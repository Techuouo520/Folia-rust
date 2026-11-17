// Port of Folia 8006b40f6 Beat This! final0 GPU graph, with original f32 arithmetic and packed fp16 weights.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::AnalysisError;
use crate::gpu::{Binding, Gpu, Graph, GraphBuilder, Kernel};
use crate::models::Weights;

pub const MAX_FRAMES: usize = 1500;
pub const MEL_BINS: usize = 128;
const HEAD_DIM: usize = 32;
const GELU: usize = 1;
const RESIDUAL: usize = 2;
const NORM: usize = 4;
const ROPE: usize = 8;

macro_rules! kernel {
    ($name:literal) => {
        Kernel {
            label: concat!("beat-this-", $name),
            source: include_str!(concat!("../shaders/beat_this/", $name, ".wgsl")),
        }
    };
}

pub struct Prediction {
    pub beat: Vec<f32>,
    pub downbeat: Vec<f32>,
}

pub struct BeatThis {
    graph: Graph,
    frames: usize,
}

impl BeatThis {
    pub async fn new(gpu: &Gpu, weights: &Weights, frames: usize) -> Result<Self, AnalysisError> {
        Self::new_cancellable(gpu, weights, frames, Arc::new(AtomicBool::new(false))).await
    }

    pub async fn new_cancellable(
        gpu: &Gpu,
        weights: &Weights,
        frames: usize,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, AnalysisError> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(AnalysisError::Cancelled);
        }
        if !(1..=MAX_FRAMES).contains(&frames) {
            return Err(AnalysisError::Model("Beat This requires 1 to 1500 frames per chunk".to_owned()));
        }
        let parameters = weights
            .manifest
            .hyper_parameters
            .as_ref()
            .ok_or_else(|| AnalysisError::Model("Beat This hyperparameters are missing".to_owned()))?;
        if parameters.transformer_dim != 512
            || parameters.stem_dim != 32
            || parameters.ff_mult != 4
            || parameters.n_layers != 6
        {
            return Err(AnalysisError::Model("Beat This export is not the final0 graph".to_owned()));
        }
        let mut build = Build::new(gpu, weights, frames)?;
        build.graph.cancel_with(cancelled);
        let (input, output) =
            match build.network(frames, parameters.transformer_dim, parameters.stem_dim, parameters.n_layers) {
                Ok(graph) => graph,
                Err(error) => {
                    build.graph.discard().await?;
                    return Err(error);
                }
            };
        let graph = build.graph.finish(input, output).await?;
        tracing::info!(bytes = graph.allocated_bytes(), frames, "Beat This GPU graph ready");
        Ok(Self { graph, frames })
    }

    pub fn allocated_bytes(&self) -> u64 {
        self.graph.allocated_bytes()
    }

    pub fn run(&mut self, mel: &[f32], cancelled: &AtomicBool) -> Result<Prediction, AnalysisError> {
        let result = self.graph.run(mel, cancelled)?;
        Ok(Prediction { beat: result[..self.frames].to_vec(), downbeat: result[self.frames..2 * self.frames].to_vec() })
    }
}

#[derive(Clone, Copy)]
struct Rope {
    position_divisor: usize,
    position_modulus: usize,
    model_dimension: usize,
}

impl Default for Rope {
    fn default() -> Self {
        Self { position_divisor: 1, position_modulus: 1, model_dimension: 0 }
    }
}

#[derive(Clone, Copy)]
struct Axis {
    length: usize,
    sequence_stride: usize,
    batch_stride: usize,
    batches: usize,
    position_divisor: usize,
}

struct Build<'a> {
    graph: GraphBuilder<'a>,
    rope: wgpu::Buffer,
    zeros: wgpu::Buffer,
    qkv: wgpu::Buffer,
    attention: wgpu::Buffer,
    hidden: wgpu::Buffer,
}

impl<'a> Build<'a> {
    fn new(gpu: &Gpu, weights: &'a Weights, frames: usize) -> Result<Self, AnalysisError> {
        let mut graph = GraphBuilder::new(gpu, weights);
        let zeroes = graph.storage(4096);
        let tokens = 32 * frames;
        let qkv = graph.storage((tokens * (3 * 32 + 1)).max(frames * (3 * 512 + 512 / HEAD_DIM)));
        let attention = graph.storage(tokens * 32);
        let hidden = graph.storage((tokens * 32 * 4).max(frames * 512 * 4));
        let frequencies = weights
            .manifest
            .tensors
            .get("rope.freqs")
            .ok_or_else(|| AnalysisError::Model("Beat This RoPE frequencies are missing".to_owned()))?;
        let offset = frequencies.byte_offset as usize;
        let bytes = weights
            .data
            .get(offset..offset + 64)
            .ok_or_else(|| AnalysisError::Model("Beat This RoPE frequencies exceed the weights".to_owned()))?;
        let frequencies: Vec<_> = bytes.as_chunks::<4>().0.iter().map(|bytes| f32::from_le_bytes(*bytes)).collect();
        let mut table = vec![0.0; frames * 16 * 2];
        for position in 0..frames {
            for index in 0..16 {
                let angle = f64::from(position as f32 * frequencies[index]);
                table[(position * 16 + index) * 2] = angle.cos() as f32;
                table[(position * 16 + index) * 2 + 1] = angle.sin() as f32;
            }
        }
        let rope = graph.storage(table.len());
        graph.write_f32(&rope, &table);
        Ok(Self { graph, rope, zeros: zeroes, qkv, attention, hidden })
    }

    fn dispatch(
        &mut self,
        kernel: Kernel,
        bindings: &[Binding],
        params: &[usize],
        count: [u32; 3],
    ) -> Result<(), AnalysisError> {
        let values = params
            .iter()
            .map(|value| {
                u32::try_from(*value).map_err(|_| AnalysisError::Model("Beat This uniform overflow".to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.graph.dispatch(kernel, bindings, &values, count)
    }

    fn matmul(
        &mut self,
        input: &wgpu::Buffer,
        weight: &str,
        bias: Option<&str>,
        output: &wgpu::Buffer,
        shape: (usize, usize),
        rope: Rope,
    ) -> Result<(), AnalysisError> {
        let (rows, flags) = shape;
        let dimensions = self.graph.shape(weight)?;
        let (k, n) = (dimensions[0], dimensions[1]);
        let stride = self.graph.stride(weight)?;
        let kernel = if self.graph.half(weight)? { kernel!("matmul_f16w") } else { kernel!("matmul") };
        let bias = match bias {
            Some(name) => self.graph.tensor(name)?,
            None => whole(&self.zeros),
        };
        self.dispatch(
            kernel,
            &[whole(input), self.graph.tensor(weight)?, bias, whole(output), whole(&self.rope)],
            &[rows, n, k, flags, rope.position_divisor, rope.position_modulus, rope.model_dimension, stride],
            [n.div_ceil(64) as u32, rows.div_ceil(64) as u32, 1],
        )
    }

    fn transformer_pair(
        &mut self,
        stream: &wgpu::Buffer,
        name: &str,
        rows: usize,
        channels: usize,
        axis: Axis,
    ) -> Result<(), AnalysisError> {
        let heads = channels / HEAD_DIM;
        let width = 3 * channels + heads;
        let (qkv, attention, hidden) = (self.qkv.clone(), self.attention.clone(), self.hidden.clone());
        self.matmul(
            stream,
            &format!("{name}.attn.qkvg.w"),
            Some(&format!("{name}.attn.qkvg.b")),
            &qkv,
            (rows, NORM | ROPE),
            Rope { position_divisor: axis.position_divisor, position_modulus: axis.length, model_dimension: channels },
        )?;
        self.dispatch(
            kernel!("attention"),
            &[whole(&qkv), whole(&attention)],
            &[axis.length, heads, width, channels, axis.sequence_stride, axis.batch_stride],
            [axis.length.div_ceil(64) as u32, heads as u32, axis.batches as u32],
        )?;
        self.matmul(&attention, &format!("{name}.attn.out.w"), None, stream, (rows, RESIDUAL), Rope::default())?;
        self.matmul(
            stream,
            &format!("{name}.ff.w1"),
            Some(&format!("{name}.ff.b1")),
            &hidden,
            (rows, NORM | GELU),
            Rope::default(),
        )?;
        self.matmul(
            &hidden,
            &format!("{name}.ff.w2"),
            Some(&format!("{name}.ff.b2")),
            stream,
            (rows, RESIDUAL),
            Rope::default(),
        )
    }

    fn network(
        &mut self,
        frames: usize,
        dimension: usize,
        stem: usize,
        layers: usize,
    ) -> Result<(wgpu::Buffer, wgpu::Buffer), AnalysisError> {
        let tokens = (MEL_BINS / 4) * frames;
        let mel = self.graph.storage(frames * MEL_BINS);
        let stem_input = self.graph.storage(frames * MEL_BINS);
        let x = [self.graph.storage(tokens * stem), self.graph.storage(tokens * stem)];
        let flattened = self.graph.storage(frames * 4 * stem * 8);
        let logits = self.graph.storage(frames * 2);
        let output = self.graph.storage(frames * 2);
        self.dispatch(
            kernel!("stem_bn"),
            &[
                whole(&mel),
                self.graph.tensor("stem.bn.scale")?,
                self.graph.tensor("stem.bn.shift")?,
                whole(&stem_input),
            ],
            &[frames, MEL_BINS],
            groups(frames * MEL_BINS),
        )?;
        let mut frequencies = MEL_BINS / 4;
        let mut channels = stem;
        self.dispatch(
            kernel!("conv_gelu"),
            &[whole(&stem_input), self.graph.tensor("stem.conv.w")?, self.graph.tensor("stem.conv.b")?, whole(&x[0])],
            &[frequencies, frames, 1, channels, 4],
            [channels.div_ceil(64) as u32, (frequencies * frames).div_ceil(64) as u32, 1],
        )?;
        let mut current = 0;
        for block in 0..3 {
            let rows = frequencies * frames;
            self.transformer_pair(
                &x[current],
                &format!("block{block}.freq"),
                rows,
                channels,
                Axis {
                    length: frequencies,
                    sequence_stride: frames,
                    batch_stride: 1,
                    batches: frames,
                    position_divisor: frames,
                },
            )?;
            self.transformer_pair(
                &x[current],
                &format!("block{block}.time"),
                rows,
                channels,
                Axis {
                    length: frames,
                    sequence_stride: 1,
                    batch_stride: frames,
                    batches: frequencies,
                    position_divisor: 1,
                },
            )?;
            let next = 1 - current;
            self.dispatch(
                kernel!("conv_gelu"),
                &[
                    whole(&x[current]),
                    self.graph.tensor(&format!("block{block}.conv.w"))?,
                    self.graph.tensor(&format!("block{block}.conv.b"))?,
                    whole(&x[next]),
                ],
                &[frequencies / 2, frames, channels, channels * 2, 2],
                [(channels * 2).div_ceil(64) as u32, (frequencies / 2 * frames).div_ceil(64) as u32, 1],
            )?;
            current = next;
            frequencies /= 2;
            channels *= 2;
        }
        self.dispatch(
            kernel!("flatten"),
            &[whole(&x[current]), whole(&flattened)],
            &[frequencies, frames, channels],
            groups(frequencies * frames * channels),
        )?;
        current = 1 - current;
        self.matmul(
            &flattened,
            "frontend.linear.w",
            Some("frontend.linear.b"),
            &x[current],
            (frames, 0),
            Rope::default(),
        )?;
        for layer in 0..layers {
            self.transformer_pair(
                &x[current],
                &format!("layer{layer}"),
                frames,
                dimension,
                Axis { length: frames, sequence_stride: 1, batch_stride: frames, batches: 1, position_divisor: 1 },
            )?;
        }
        self.matmul(&x[current], "head.w", Some("head.b"), &logits, (frames, NORM), Rope::default())?;
        self.dispatch(kernel!("sum_head"), &[whole(&logits), whole(&output)], &[frames], groups(frames))?;
        Ok((mel, output))
    }
}

fn whole(buffer: &wgpu::Buffer) -> Binding {
    Binding::whole(buffer)
}

fn groups(elements: usize) -> [u32; 3] {
    [elements.div_ceil(64) as u32, 1, 1]
}
