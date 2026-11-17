// Port of Folia 8006b40f6 htdemucsWgsl/runtime.ts, using the pinned MIT model and original WGSL graph.
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::AnalysisError;
use crate::gpu::{Binding, Gpu, Graph, GraphBuilder, Kernel};
use crate::models::Weights;

pub const SEGMENT: usize = 172_032;
const FRAMES: usize = 168;
const BINS: usize = 2048;
const DIM: usize = 512;
const GEMM_GELU: usize = 1;
const GEMM_RESIDUAL: usize = 2;

macro_rules! kernel {
    ($name:literal) => {
        Kernel {
            label: concat!("htdemucs-", $name),
            source: include_str!(concat!("../shaders/htdemucs/", $name, ".wgsl")),
        }
    };
}

pub struct Htdemucs {
    graph: Graph,
    input: Vec<f32>,
}

pub struct Stems {
    samples: Vec<f32>,
}

impl Stems {
    pub fn drums(&self) -> (&[f32], &[f32]) {
        (&self.samples[..SEGMENT], &self.samples[SEGMENT..2 * SEGMENT])
    }

    pub fn vocals(&self) -> (&[f32], &[f32]) {
        (&self.samples[2 * SEGMENT..3 * SEGMENT], &self.samples[3 * SEGMENT..4 * SEGMENT])
    }
}

impl Htdemucs {
    pub async fn new(gpu: &Gpu, weights: &Weights) -> Result<Self, AnalysisError> {
        Self::new_cancellable(gpu, weights, Arc::new(AtomicBool::new(false))).await
    }

    pub async fn new_cancellable(
        gpu: &Gpu,
        weights: &Weights,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, AnalysisError> {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(AnalysisError::Cancelled);
        }
        if weights.manifest.segment != Some(SEGMENT) {
            return Err(AnalysisError::Model("htdemucs segment must contain 172032 samples".to_owned()));
        }
        let mut build = Build::new(gpu, weights);
        build.graph.cancel_with(cancelled);
        let (input, output) = match build.network() {
            Ok(graph) => graph,
            Err(error) => {
                build.graph.discard().await?;
                return Err(error);
            }
        };
        let graph = build.graph.finish(input, output).await?;
        tracing::info!(bytes = graph.allocated_bytes(), "htdemucs GPU graph ready");
        Ok(Self { graph, input: vec![0.0; 2 * SEGMENT] })
    }

    pub fn allocated_bytes(&self) -> u64 {
        self.graph.allocated_bytes()
    }

    pub fn run(&mut self, left: &[f32], right: &[f32], cancelled: &AtomicBool) -> Result<Stems, AnalysisError> {
        if left.len() != SEGMENT || right.len() != SEGMENT {
            return Err(AnalysisError::Model("htdemucs requires one complete stereo segment".to_owned()));
        }
        self.input[..SEGMENT].copy_from_slice(left);
        self.input[SEGMENT..].copy_from_slice(right);
        let samples = self.graph.run(&self.input, cancelled)?;
        Ok(Stems { samples })
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    h: usize,
    w: usize,
    kh: usize,
    kw: usize,
    sh: usize,
    sw: usize,
    ph: usize,
    pw: usize,
    dh: usize,
    dw: usize,
}

impl Geometry {
    fn point(h: usize, w: usize) -> Self {
        Self { h, w, kh: 1, kw: 1, sh: 1, sw: 1, ph: 0, pw: 0, dh: 1, dw: 1 }
    }
}

struct Build<'a> {
    graph: GraphBuilder<'a>,
    stream: wgpu::Buffer,
    hidden: wgpu::Buffer,
    wide: wgpu::Buffer,
    scratch_stats: wgpu::Buffer,
    partial: wgpu::Buffer,
    zeros: wgpu::Buffer,
}

impl<'a> Build<'a> {
    fn new(gpu: &Gpu, weights: &'a Weights) -> Self {
        let mut graph = GraphBuilder::new(gpu, weights);
        let top = 48 * (BINS / 4) * FRAMES;
        let stream = graph.storage(top);
        let hidden = graph.storage(top / 8);
        let wide = graph.storage(2 * top);
        let scratch_stats = graph.storage(4 * BINS);
        let partial = graph.storage(4096);
        let zeros = graph.storage(64);
        Self { graph, stream, hidden, wide, scratch_stats, partial, zeros }
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
                u32::try_from(*value).map_err(|_| AnalysisError::Model("htdemucs uniform overflow".to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.graph.dispatch(kernel, bindings, &values, count)
    }

    fn stats(
        &mut self,
        input: &wgpu::Buffer,
        target: &wgpu::Buffer,
        geometry: [usize; 5],
        final_kind: usize,
    ) -> Result<(), AnalysisError> {
        let [groups, channels, inner, channel_stride, group_stride] = geometry;
        let n = channels * inner;
        let parts = 256.min(4096 / groups).min(n.div_ceil(16384)).max(1);
        for (variance, mode) in [(0, 0), (1, final_kind)] {
            self.dispatch(
                kernel!("stats_partial"),
                &[whole(input), whole(target), whole(&self.partial)],
                &[parts, channels, inner, channel_stride, group_stride, variance],
                [parts as u32, groups as u32, 1],
            )?;
            self.dispatch(
                kernel!("stats_final"),
                &[whole(&self.partial), whole(target)],
                &[parts, n, mode],
                [groups as u32, 1, 1],
            )?;
        }
        Ok(())
    }

    fn group_stats(
        &mut self,
        input: &wgpu::Buffer,
        c: usize,
        h: usize,
        w: usize,
        per_h: bool,
    ) -> Result<(), AnalysisError> {
        let geometry = if per_h { [h, c, w, h * w, w] } else { [1, 1, c * h * w, 0, 0] };
        self.stats(input, &self.scratch_stats.clone(), geometry, 1)
    }

    fn conv(
        &mut self,
        input: &wgpu::Buffer,
        name: &str,
        output: &wgpu::Buffer,
        g: Geometry,
        flags: usize,
        side: Option<Binding>,
    ) -> Result<(usize, usize), AnalysisError> {
        let weights = format!("{name}.w");
        let shape = self.graph.shape(&weights)?;
        let (m, k) = (shape[0], shape[1]);
        let channels = k / (g.kh * g.kw);
        let hout = (g.h + 2 * g.ph - g.dh * (g.kh - 1) - 1) / g.sh + 1;
        let wout = (g.w + 2 * g.pw - g.dw * (g.kw - 1) - 1) / g.sw + 1;
        let code = if self.graph.half(&weights)? { kernel!("gemm_f16w") } else { kernel!("gemm_f32") };
        self.dispatch(
            code,
            &[
                whole(input),
                self.graph.tensor(&weights)?,
                self.graph.tensor(&format!("{name}.b"))?,
                side.unwrap_or_else(|| whole(&self.zeros)),
                whole(output),
            ],
            &[m, k, channels, g.kh, g.kw, g.sh, g.sw, g.ph, g.pw, g.dh, g.dw, g.h, g.w, hout, wout, flags],
            [(hout * wout).div_ceil(64) as u32, m.div_ceil(64) as u32, 1],
        )?;
        Ok((hout, wout))
    }

    fn linear(
        &mut self,
        input: &wgpu::Buffer,
        name: &str,
        output: &wgpu::Buffer,
        n: usize,
        flags: usize,
        side: Option<Binding>,
    ) -> Result<(), AnalysisError> {
        self.conv(input, name, output, Geometry::point(1, n), flags, side).map(|_| ())
    }

    fn conv_transpose(
        &mut self,
        input: &wgpu::Buffer,
        name: &str,
        output: &wgpu::Buffer,
        shape: [usize; 2],
        axis: usize,
        flags: usize,
    ) -> Result<(), AnalysisError> {
        let weights = format!("{name}.w");
        let dimensions = self.graph.shape(&weights)?;
        let (m, k) = (dimensions[0], dimensions[1]);
        let [h, w] = shape;
        let n = if axis == 0 { (h + 1) * w } else { h * (w + 1) };
        let code = if self.graph.half(&weights)? { kernel!("gemm_tr_f16w") } else { kernel!("gemm_tr_f32") };
        self.dispatch(
            code,
            &[whole(input), self.graph.tensor(&weights)?, self.graph.tensor(&format!("{name}.b"))?, whole(output)],
            &[m, k, k / 2, h, w, axis, flags],
            [n.div_ceil(64) as u32, m.div_ceil(64) as u32, 1],
        )
    }

    fn dconv(&mut self, prefix: &str, c: usize, h: usize, w: usize, per_h: bool) -> Result<(), AnalysisError> {
        let (stream, hidden, wide) = (self.stream.clone(), self.hidden.clone(), self.wide.clone());
        for index in 0..2 {
            let prefix = format!("{prefix}.dc{index}");
            let dilation = 1 << index;
            let channels = self.graph.shape(&format!("{prefix}.c1.w"))?[0];
            self.conv(
                &stream,
                &format!("{prefix}.c1"),
                &hidden,
                Geometry { kw: 3, pw: dilation, dw: dilation, ..Geometry::point(h, w) },
                0,
                None,
            )?;
            self.group_stats(&hidden, channels, h, w, per_h)?;
            self.dispatch(
                kernel!("gn_inplace"),
                &[
                    whole(&hidden),
                    whole(&self.scratch_stats),
                    self.graph.tensor(&format!("{prefix}.n1.g"))?,
                    self.graph.tensor(&format!("{prefix}.n1.b"))?,
                ],
                &[channels, h, w, usize::from(per_h), 1],
                flat(channels * h * w),
            )?;
            self.conv(&hidden, &format!("{prefix}.c2"), &wide, Geometry::point(h, w), 0, None)?;
            self.group_stats(&wide, 2 * c, h, w, per_h)?;
            self.dispatch(
                kernel!("gn_glu_residual"),
                &[
                    whole(&wide),
                    whole(&self.scratch_stats),
                    self.graph.tensor(&format!("{prefix}.n2.g"))?,
                    self.graph.tensor(&format!("{prefix}.n2.b"))?,
                    self.graph.tensor(&format!("{prefix}.ls"))?,
                    whole(&stream),
                ],
                &[c, h, w, usize::from(per_h)],
                flat(c * h * w),
            )?;
        }
        Ok(())
    }

    fn layer_norm(
        &mut self,
        input: &wgpu::Buffer,
        name: &str,
        output: &wgpu::Buffer,
        n: usize,
    ) -> Result<(), AnalysisError> {
        self.dispatch(
            kernel!("layer_norm"),
            &[
                whole(input),
                self.graph.tensor(&format!("{name}.g"))?,
                self.graph.tensor(&format!("{name}.b"))?,
                whole(&self.zeros),
                whole(output),
            ],
            &[DIM, n, 0],
            [n.div_ceil(64) as u32, 1, 1],
        )
    }

    fn attention(
        &mut self,
        q: &wgpu::Buffer,
        kv: &wgpu::Buffer,
        out: &wgpu::Buffer,
        scores: &wgpu::Buffer,
        lengths: [usize; 2],
        offsets: [usize; 3],
    ) -> Result<(), AnalysisError> {
        let [nq, nk] = lengths;
        let [rows_q, rows_k, rows_v] = offsets;
        for head in [0, 4] {
            self.dispatch(
                kernel!("attn_scores"),
                &[whole(q), whole(kv), whole(scores)],
                &[nq, nk, rows_q, rows_k, head],
                [nk.div_ceil(64) as u32, nq.div_ceil(64) as u32, 4],
            )?;
            let rows = 4 * nq;
            let x = rows.min(32768);
            self.dispatch(kernel!("attn_softmax"), &[whole(scores)], &[nk], [x as u32, rows.div_ceil(x) as u32, 1])?;
            self.dispatch(
                kernel!("attn_out"),
                &[whole(scores), whole(kv), whole(out)],
                &[nq, nk, rows_v, head],
                [1, nq.div_ceil(64) as u32, 4],
            )?;
        }
        Ok(())
    }

    fn feed_forward(
        &mut self,
        x: &wgpu::Buffer,
        name: &str,
        n: usize,
        norm: &wgpu::Buffer,
        ff: &wgpu::Buffer,
    ) -> Result<(), AnalysisError> {
        self.layer_norm(x, &format!("{name}.nf"), norm, n)?;
        self.linear(norm, &format!("{name}.ff1"), ff, n, GEMM_GELU, None)?;
        self.linear(ff, &format!("{name}.ff2"), x, n, GEMM_RESIDUAL, Some(self.graph.tensor(&format!("{name}.g2"))?))?;
        self.stats(x, &self.scratch_stats.clone(), [1, 1, DIM * n, 0, 0], 1)?;
        self.dispatch(
            kernel!("gn_inplace"),
            &[
                whole(x),
                whole(&self.scratch_stats),
                self.graph.tensor(&format!("{name}.no.g"))?,
                self.graph.tensor(&format!("{name}.no.b"))?,
            ],
            &[DIM, 1, n, 0, 0],
            flat(DIM * n),
        )
    }

    fn network(&mut self) -> Result<(wgpu::Buffer, wgpu::Buffer), AnalysisError> {
        let mix = self.graph.storage(2 * SEGMENT);
        let output = self.graph.storage(4 * SEGMENT);
        let mix_stats = self.graph.storage(4);
        let spec_stats = self.graph.storage(4);
        let dft = self.graph.storage(2 * FRAMES * 4096 * 2);
        let xin = self.graph.storage(4 * BINS * FRAMES);
        let ping = [self.graph.storage(16 * BINS * FRAMES), self.graph.storage(16 * BINS * FRAMES)];
        let freq_out = self.graph.storage(16 * BINS * FRAMES);
        let time_out = self.graph.storage(8 * SEGMENT);
        let frames = self.graph.storage(2 * FRAMES * 4096);
        let skips: Vec<_> =
            (0..4).map(|i| self.graph.storage(48 * (1 << i) * (BINS / 4usize.pow(i + 1)) * FRAMES)).collect();
        let time_skips: Vec<_> =
            (0..4).map(|i| self.graph.storage(48 * (1 << i) * (SEGMENT / 4usize.pow(i + 1)))).collect();
        let twiddle: Vec<_> = (0..4096)
            .flat_map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / 4096.0;
                [angle.cos() as f32, angle.sin() as f32]
            })
            .collect();
        let tw = self.graph.storage(twiddle.len());
        self.graph.write_f32(&tw, &twiddle);
        self.stats(&mix, &mix_stats, [1, 1, 2 * SEGMENT, 0, 0], 2)?;
        self.graph.copy(&mix, &time_out, (2 * SEGMENT * 4) as u64);
        self.dispatch(kernel!("normalize"), &[whole(&time_out), whole(&mix_stats)], &[2 * SEGMENT], flat(2 * SEGMENT))?;
        self.dispatch(
            kernel!("stft_a"),
            &[whole(&mix), self.graph.tensor("stft.window")?, whole(&tw), whole(&dft)],
            &[FRAMES, SEGMENT],
            [1, 64, 2 * FRAMES as u32],
        )?;
        self.dispatch(
            kernel!("stft_b"),
            &[whole(&dft), whole(&tw), whole(&xin)],
            &[FRAMES],
            [1, 64, 2 * FRAMES as u32],
        )?;
        self.stats(&xin, &spec_stats, [1, 1, 4 * BINS * FRAMES, 0, 0], 2)?;
        self.dispatch(
            kernel!("normalize"),
            &[whole(&xin), whole(&spec_stats)],
            &[4 * BINS * FRAMES],
            flat(4 * BINS * FRAMES),
        )?;
        self.encoders(&xin, &time_out, &skips, &time_skips)?;
        let (xf, xt) = self.transformer(&skips[3], &time_skips[3])?;
        self.decoders([&xf, &xt], &ping, [&skips, &time_skips], [&freq_out, &time_out])?;
        for (index, source) in [0, 3].into_iter().enumerate() {
            self.dispatch(
                kernel!("istft_a"),
                &[whole(&freq_out), whole(&spec_stats), whole(&tw), whole(&dft)],
                &[FRAMES, source],
                [1, 64, 2 * FRAMES as u32],
            )?;
            self.dispatch(
                kernel!("istft_b"),
                &[whole(&dft), whole(&tw), self.graph.tensor("istft.window")?, whole(&frames)],
                &[FRAMES],
                [1, 64, 2 * FRAMES as u32],
            )?;
            self.dispatch(
                kernel!("overlap_add"),
                &[whole(&frames), self.graph.tensor("istft.env")?, whole(&time_out), whole(&mix_stats), whole(&output)],
                &[FRAMES, SEGMENT, source, index * 2 * SEGMENT],
                flat(2 * SEGMENT),
            )?;
        }
        Ok((mix, output))
    }

    fn encoders(
        &mut self,
        xin: &wgpu::Buffer,
        time_input: &wgpu::Buffer,
        skips: &[wgpu::Buffer],
        time_skips: &[wgpu::Buffer],
    ) -> Result<(), AnalysisError> {
        let (stream, wide) = (self.stream.clone(), self.wide.clone());
        let mut time_current = time_input.clone();
        let mut frequency_current = xin.clone();
        let mut time_length = SEGMENT;
        let mut bins = BINS;
        for i in 0..4 {
            let channels = 48 * (1 << i);
            (_, time_length) = self.conv(
                &time_current,
                &format!("tenc{i}.conv"),
                &stream,
                Geometry { kw: 8, sw: 4, pw: 2, ..Geometry::point(1, time_length) },
                GEMM_GELU,
                None,
            )?;
            self.dconv(&format!("tenc{i}"), channels, 1, time_length, false)?;
            self.conv(&stream, &format!("tenc{i}.rw"), &wide, Geometry::point(1, time_length), 0, None)?;
            self.dispatch(
                kernel!("glu"),
                &[whole(&wide), whole(&self.zeros), whole(&time_skips[i])],
                &[channels, 1, time_length, 0],
                flat(channels * time_length),
            )?;
            time_current = time_skips[i].clone();
            (bins, _) = self.conv(
                &frequency_current,
                &format!("enc{i}.conv"),
                &stream,
                Geometry { kh: 8, sh: 4, ph: 2, ..Geometry::point(bins, FRAMES) },
                GEMM_GELU,
                None,
            )?;
            self.dconv(&format!("enc{i}"), channels, bins, FRAMES, true)?;
            self.conv(&stream, &format!("enc{i}.rw"), &wide, Geometry::point(bins, FRAMES), 0, None)?;
            let embedding = if i == 0 { self.graph.tensor("freq_emb")? } else { whole(&self.zeros) };
            self.dispatch(
                kernel!("glu"),
                &[whole(&wide), embedding, whole(&skips[i])],
                &[channels, bins, FRAMES, usize::from(i == 0)],
                flat(channels * bins * FRAMES),
            )?;
            frequency_current = skips[i].clone();
        }
        Ok(())
    }

    fn transformer(
        &mut self,
        frequency: &wgpu::Buffer,
        time: &wgpu::Buffer,
    ) -> Result<(wgpu::Buffer, wgpu::Buffer), AnalysisError> {
        let nf = 8 * FRAMES;
        let nt = SEGMENT / 256;
        let xf = self.graph.storage(DIM * nf);
        let xt = self.graph.storage(DIM * nt);
        let norm_f = self.graph.storage(DIM * nf);
        let norm_t = self.graph.storage(DIM * nt);
        let other_f = self.graph.storage(DIM * nf);
        let other_t = self.graph.storage(DIM * nt);
        let qkv_f = self.graph.storage(3 * DIM * nf);
        let qkv_t = self.graph.storage(3 * DIM * nt);
        let kv_f = self.graph.storage(2 * DIM * nt);
        let kv_t = self.graph.storage(2 * DIM * nf);
        let att_f = self.graph.storage(DIM * nf);
        let att_t = self.graph.storage(DIM * nt);
        let ff = self.graph.storage(4 * DIM * nf);
        let scores = self.graph.storage(4 * nf * nf);
        self.linear(frequency, "up", &norm_f, nf, 0, None)?;
        self.linear(time, "up_t", &norm_t, nt, 0, None)?;
        for (norm, name, pos, x, n) in [(&norm_f, "norm_in", "pos", &xf, nf), (&norm_t, "norm_in_t", "pos_t", &xt, nt)]
        {
            self.dispatch(
                kernel!("layer_norm"),
                &[
                    whole(norm),
                    self.graph.tensor(&format!("{name}.g"))?,
                    self.graph.tensor(&format!("{name}.b"))?,
                    self.graph.tensor(pos)?,
                    whole(x),
                ],
                &[DIM, n, 1],
                [n.div_ceil(64) as u32, 1, 1],
            )?;
        }
        for layer in 0..5 {
            let pf = format!("tf{layer}");
            let pt = format!("tft{layer}");
            if layer % 2 == 0 {
                for (x, name, n, norm, qkv, att) in
                    [(&xf, &pf, nf, &norm_f, &qkv_f, &att_f), (&xt, &pt, nt, &norm_t, &qkv_t, &att_t)]
                {
                    self.layer_norm(x, &format!("{name}.n1"), norm, n)?;
                    self.linear(norm, &format!("{name}.qkv"), qkv, n, 0, None)?;
                    self.attention(qkv, qkv, att, &scores, [n, n], [0, DIM, 2 * DIM])?;
                    self.linear(
                        att,
                        &format!("{name}.out"),
                        x,
                        n,
                        GEMM_RESIDUAL,
                        Some(self.graph.tensor(&format!("{name}.g1"))?),
                    )?;
                    self.feed_forward(x, name, n, norm, &ff)?;
                }
            } else {
                self.layer_norm(&xf, &format!("{pf}.n1"), &norm_f, nf)?;
                self.linear(&norm_f, &format!("{pf}.q"), &qkv_f, nf, 0, None)?;
                self.layer_norm(&xt, &format!("{pf}.nkv"), &other_t, nt)?;
                self.linear(&other_t, &format!("{pf}.kv"), &kv_f, nt, 0, None)?;
                self.layer_norm(&xt, &format!("{pt}.n1"), &norm_t, nt)?;
                self.linear(&norm_t, &format!("{pt}.q"), &qkv_t, nt, 0, None)?;
                self.layer_norm(&xf, &format!("{pt}.nkv"), &other_f, nf)?;
                self.linear(&other_f, &format!("{pt}.kv"), &kv_t, nf, 0, None)?;
                self.attention(&qkv_f, &kv_f, &att_f, &scores, [nf, nt], [0, 0, DIM])?;
                self.attention(&qkv_t, &kv_t, &att_t, &scores, [nt, nf], [0, 0, DIM])?;
                self.linear(
                    &att_f,
                    &format!("{pf}.out"),
                    &xf,
                    nf,
                    GEMM_RESIDUAL,
                    Some(self.graph.tensor(&format!("{pf}.g1"))?),
                )?;
                self.linear(
                    &att_t,
                    &format!("{pt}.out"),
                    &xt,
                    nt,
                    GEMM_RESIDUAL,
                    Some(self.graph.tensor(&format!("{pt}.g1"))?),
                )?;
                self.feed_forward(&xf, &pf, nf, &norm_f, &ff)?;
                self.feed_forward(&xt, &pt, nt, &norm_t, &ff)?;
            }
        }
        Ok((xf, xt))
    }

    fn decoders(
        &mut self,
        input: [&wgpu::Buffer; 2],
        ping: &[wgpu::Buffer; 2],
        skips: [&[wgpu::Buffer]; 2],
        output: [&wgpu::Buffer; 2],
    ) -> Result<(), AnalysisError> {
        let (stream, wide) = (self.stream.clone(), self.wide.clone());
        for (input, down, prefix, skips, output, frequency) in [
            (input[0], "down", "dec", skips[0], output[0], true),
            (input[1], "down_t", "tdec", skips[1], output[1], false),
        ] {
            let mut bins = if frequency { 8 } else { 1 };
            let mut width = if frequency { FRAMES } else { SEGMENT / 256 };
            let mut current = ping[0].clone();
            self.linear(input, down, &current, bins * width, 0, None)?;
            for i in 0..4 {
                let channels = 48 * (1 << (3 - i));
                self.dispatch(
                    kernel!("add"),
                    &[whole(&current), whole(&skips[3 - i])],
                    &[channels * bins * width],
                    flat(channels * bins * width),
                )?;
                let geometry = Geometry {
                    kh: if frequency { 3 } else { 1 },
                    kw: 3,
                    ph: usize::from(frequency),
                    pw: 1,
                    ..Geometry::point(bins, width)
                };
                self.conv(&current, &format!("{prefix}{i}.rw"), &wide, geometry, 0, None)?;
                self.dispatch(
                    kernel!("glu"),
                    &[whole(&wide), whole(&self.zeros), whole(&stream)],
                    &[channels, bins, width, 0],
                    flat(channels * bins * width),
                )?;
                self.dconv(&format!("{prefix}{i}"), channels, bins, width, frequency)?;
                let next = if i == 3 { output } else { &ping[(i + 1) % 2] };
                self.conv_transpose(
                    &stream,
                    &format!("{prefix}{i}.tr"),
                    next,
                    [bins, width],
                    usize::from(!frequency),
                    if i == 3 { 0 } else { GEMM_GELU },
                )?;
                if frequency {
                    bins *= 4;
                } else {
                    width *= 4;
                }
                current = next.clone();
            }
        }
        Ok(())
    }
}

fn whole(buffer: &wgpu::Buffer) -> Binding {
    Binding::whole(buffer)
}

fn flat(elements: usize) -> [u32; 3] {
    let groups = elements.div_ceil(256).max(1);
    let x = groups.min(32768);
    [x as u32, groups.div_ceil(x) as u32, 1]
}
