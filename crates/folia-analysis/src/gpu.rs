// Executes fixed native WGSL model graphs on a dedicated device, yielding between dispatches.
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AnalysisError;
use crate::models::{Manifest, Weights};

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub async fn new() -> Result<Self, AnalysisError> {
        let executable = std::env::current_exe()?;
        let directory = executable.parent().ok_or_else(|| AnalysisError::Gpu("executable has no directory".into()))?;
        Self::from_runtime(&directory.join("assets").join("analysis")).await
    }

    pub async fn from_runtime(directory: &std::path::Path) -> Result<Self, AnalysisError> {
        let options = runtime_options(directory)?;
        let (adapter, device, queue) =
            folia_render::backend::device_with_options(options).await.map_err(AnalysisError::Gpu)?;
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err(AnalysisError::Gpu("software adapters cannot run live music analysis".to_owned()));
        }
        tracing::info!(adapter = %adapter.get_info().name, backend = ?adapter.get_info().backend, "music analysis GPU ready");
        Ok(Self { device, queue })
    }
}

fn runtime_options(directory: &std::path::Path) -> Result<wgpu::BackendOptions, AnalysisError> {
    #[cfg(windows)]
    if folia_render::backend::configured().iter().any(|backends| backends.contains(wgpu::Backends::DX12)) {
        #[derive(serde::Deserialize)]
        struct RuntimeFile {
            bytes: u64,
            sha256: String,
        }
        #[derive(serde::Deserialize)]
        struct RuntimeManifest {
            windows: BTreeMap<String, RuntimeFile>,
        }
        let manifest: RuntimeManifest = toml::from_str(include_str!("../../../assets/analysis/manifest.toml"))?;
        let file = manifest
            .windows
            .get(std::env::consts::ARCH)
            .ok_or_else(|| AnalysisError::Gpu("analysis compiler does not support this Windows architecture".into()))?;
        let path = directory.join("dxcompiler.dll");
        verify_runtime(&path, file.bytes, &file.sha256)
            .map_err(|source| AnalysisError::Runtime { path: path.clone(), source })?;
        let mut options = wgpu::BackendOptions::default();
        options.dx12.shader_compiler = wgpu::Dx12Compiler::DynamicDxc {
            dxc_path: path.to_string_lossy().into_owned(),
            max_shader_model: wgpu::DxcShaderModel::V6_7,
        };
        return Ok(options);
    }
    let _ = directory;
    Ok(wgpu::BackendOptions::default())
}

#[cfg(windows)]
fn verify_runtime(path: &std::path::Path, bytes: u64, expected: &str) -> std::io::Result<()> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    if file.metadata()?.len() != bytes {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "analysis compiler has an unexpected length"));
    }
    let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
    let mut block = [0u8; 65536];
    loop {
        let count = file.read(&mut block)?;
        if count == 0 {
            break;
        }
        digest.update(&block[..count]);
    }
    let actual: String = digest.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
    if actual != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "analysis compiler failed SHA256 verification",
        ));
    }
    Ok(())
}

#[derive(Clone)]
pub struct Binding {
    pub buffer: wgpu::Buffer,
    pub offset: u64,
    pub size: Option<NonZeroU64>,
}

impl Binding {
    pub fn whole(buffer: &wgpu::Buffer) -> Self {
        Self { buffer: buffer.clone(), offset: 0, size: None }
    }
}

#[derive(Clone, Copy)]
pub struct Kernel {
    pub label: &'static str,
    pub source: &'static str,
}

enum Step {
    Dispatch { pipeline: wgpu::ComputePipeline, group: wgpu::BindGroup, count: [u32; 3] },
    Copy { from: wgpu::Buffer, to: wgpu::Buffer, bytes: u64 },
}

pub struct GraphBuilder<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    weights: wgpu::Buffer,
    manifest: &'a Manifest,
    pipelines: HashMap<&'static str, wgpu::ComputePipeline>,
    steps: Vec<Step>,
    allocated: u64,
    cancelled: Option<Arc<AtomicBool>>,
}

impl<'a> GraphBuilder<'a> {
    pub fn new(gpu: &Gpu, weights: &'a Weights) -> Self {
        Self::from_parts(gpu, &weights.manifest, &weights.data)
    }

    fn from_parts(gpu: &Gpu, manifest: &'a Manifest, data: &[u8]) -> Self {
        gpu.device.push_error_scope(wgpu::ErrorFilter::Internal);
        gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("analysis weights"),
            size: data.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&buffer, 0, data);
        Self {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            weights: buffer,
            manifest,
            pipelines: HashMap::new(),
            steps: Vec::new(),
            allocated: data.len() as u64,
            cancelled: None,
        }
    }

    pub fn cancel_with(&mut self, cancelled: Arc<AtomicBool>) {
        self.cancelled = Some(cancelled);
    }

    pub fn storage(&mut self, floats: usize) -> wgpu::Buffer {
        let size = (floats as u64 * 4).max(16);
        self.allocated += size;
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("analysis activation"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn write_f32(&self, buffer: &wgpu::Buffer, values: &[f32]) {
        self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(values));
    }

    pub fn tensor(&self, name: &str) -> Result<Binding, AnalysisError> {
        let tensor =
            self.manifest.tensors.get(name).ok_or_else(|| AnalysisError::Model(format!("missing tensor {name}")))?;
        let bytes = tensor
            .bytes()
            .and_then(NonZeroU64::new)
            .ok_or_else(|| AnalysisError::Model(format!("empty tensor {name}")))?;
        let alignment = u64::from(self.device.limits().min_storage_buffer_offset_alignment);
        if !tensor.byte_offset.is_multiple_of(alignment) {
            return Err(AnalysisError::Model(format!("unaligned tensor {name}")));
        }
        Ok(Binding { buffer: self.weights.clone(), offset: tensor.byte_offset, size: Some(bytes) })
    }

    pub fn shape(&self, name: &str) -> Result<&[usize], AnalysisError> {
        self.manifest
            .tensors
            .get(name)
            .map(|tensor| tensor.shape.as_slice())
            .ok_or_else(|| AnalysisError::Model(format!("missing tensor {name}")))
    }

    pub fn half(&self, name: &str) -> Result<bool, AnalysisError> {
        self.manifest
            .tensors
            .get(name)
            .map(|tensor| tensor.dtype == crate::models::Dtype::F16)
            .ok_or_else(|| AnalysisError::Model(format!("missing tensor {name}")))
    }

    pub fn stride(&self, name: &str) -> Result<usize, AnalysisError> {
        self.manifest
            .tensors
            .get(name)
            .and_then(|tensor| tensor.stride)
            .filter(|stride| *stride > 0)
            .ok_or_else(|| AnalysisError::Model(format!("missing tensor row stride for {name}")))
    }

    pub fn dispatch(
        &mut self,
        kernel: Kernel,
        bindings: &[Binding],
        params: &[u32],
        count: [u32; 3],
    ) -> Result<(), AnalysisError> {
        if self.cancelled.as_ref().is_some_and(|cancelled| cancelled.load(Ordering::Acquire)) {
            return Err(AnalysisError::Cancelled);
        }
        if count.contains(&0)
            || count.iter().any(|count| *count > self.device.limits().max_compute_workgroups_per_dimension)
        {
            return Err(AnalysisError::Model(format!("invalid dispatch size for {}", kernel.label)));
        }
        let pipeline = self
            .pipelines
            .entry(kernel.label)
            .or_insert_with(|| {
                let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(kernel.label),
                    source: wgpu::ShaderSource::Wgsl(kernel.source.into()),
                });
                self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(kernel.label),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .clone();
        let mut values = vec![0u32; params.len().div_ceil(4).max(1) * 4];
        values[..params.len()].copy_from_slice(params);
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(kernel.label),
            size: values.len() as u64 * 4,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.allocated += uniform.size();
        self.queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&values));
        let mut entries: Vec<_> = bindings
            .iter()
            .enumerate()
            .map(|(index, binding)| wgpu::BindGroupEntry {
                binding: index as u32,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &binding.buffer,
                    offset: binding.offset,
                    size: binding.size,
                }),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry { binding: entries.len() as u32, resource: uniform.as_entire_binding() });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(kernel.label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        self.steps.push(Step::Dispatch { pipeline, group, count });
        Ok(())
    }

    pub fn copy(&mut self, from: &wgpu::Buffer, to: &wgpu::Buffer, bytes: u64) {
        self.steps.push(Step::Copy { from: from.clone(), to: to.clone(), bytes });
    }

    pub async fn finish(self, input: wgpu::Buffer, output: wgpu::Buffer) -> Result<Graph, AnalysisError> {
        close_scopes(&self.device).await?;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("analysis readback"),
            size: output.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Graph {
            device: self.device,
            queue: self.queue,
            steps: self.steps,
            input,
            output,
            readback,
            allocated: self.allocated,
        })
    }

    pub async fn discard(self) -> Result<(), AnalysisError> {
        // Flush pending uploads before releasing a cancelled graph's staging buffers.
        let submitted = self.queue.submit([]);
        let waited = self
            .device
            .poll(wgpu::PollType::Wait { submission_index: Some(submitted), timeout: None })
            .map_err(|error| AnalysisError::Gpu(error.to_string()));
        close_scopes(&self.device).await?;
        waited.map(|_| ())
    }
}

impl GraphBuilder<'static> {
    pub fn empty(gpu: &Gpu) -> Self {
        static EMPTY: Manifest =
            Manifest { bytes: 16, tensors: BTreeMap::new(), segment: None, hyper_parameters: None };
        Self::from_parts(gpu, &EMPTY, &[0u8; 16])
    }
}

pub struct Graph {
    device: wgpu::Device,
    queue: wgpu::Queue,
    steps: Vec<Step>,
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    allocated: u64,
}

impl Graph {
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated + self.readback.size()
    }

    pub fn run(&mut self, input: &[f32], cancelled: &AtomicBool) -> Result<Vec<f32>, AnalysisError> {
        if cancelled.load(Ordering::Acquire) {
            return Err(AnalysisError::Cancelled);
        }
        if input.len() as u64 * 4 != self.input.size() {
            return Err(AnalysisError::Model("input length does not match the model graph".to_owned()));
        }
        self.queue.write_buffer(&self.input, 0, bytemuck::cast_slice(input));
        for step in &self.steps {
            if cancelled.load(Ordering::Acquire) {
                return Err(AnalysisError::Cancelled);
            }
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            match step {
                Step::Dispatch { pipeline, group, count } => {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, group, &[]);
                    pass.dispatch_workgroups(count[0], count[1], count[2]);
                }
                Step::Copy { from, to, bytes } => encoder.copy_buffer_to_buffer(from, 0, to, 0, *bytes),
            }
            let submitted = self.queue.submit([encoder.finish()]);
            self.device
                .poll(wgpu::PollType::Wait { submission_index: Some(submitted), timeout: None })
                .map_err(|error| AnalysisError::Gpu(error.to_string()))?;
        }
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(&self.output, 0, &self.readback, 0, self.output.size());
        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            if sender.send(result).is_err() {
                tracing::debug!("analysis readback receiver was released");
            }
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|error| AnalysisError::Gpu(error.to_string()))?;
        receiver
            .recv()
            .map_err(|error| AnalysisError::Gpu(error.to_string()))?
            .map_err(|error| AnalysisError::Gpu(error.to_string()))?;
        let mapped = slice.get_mapped_range();
        let values = bytemuck::try_cast_slice::<u8, f32>(&mapped)
            .map(|values| values.to_vec())
            .map_err(|error| AnalysisError::Gpu(error.to_string()));
        drop(mapped);
        self.readback.unmap();
        let values = values?;
        if values.iter().any(|value| !value.is_finite()) {
            return Err(AnalysisError::Gpu("model output contains non-finite samples".to_owned()));
        }
        Ok(values)
    }
}

async fn close_scopes(device: &wgpu::Device) -> Result<(), AnalysisError> {
    let mut errors = Vec::new();
    for _ in 0..3 {
        if let Some(error) = device.pop_error_scope().await {
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(AnalysisError::Gpu(errors.join("\n"))) }
}
