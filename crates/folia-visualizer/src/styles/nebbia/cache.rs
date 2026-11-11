// Ring-buffer building textures and dirty-column scheduling from nebbiaBuildingCache.ts.

use std::sync::atomic::{AtomicBool, Ordering};

use bytemuck::{Pod, Zeroable};

use super::gpu::{Uniforms, layout, target_view, uniform_entry};

const LEVELS: [[f64; 3]; 3] = [[0.0012, 0.03, 0.63], [0.03, 0.0, 0.66], [0.09, -0.06, 0.73]];
const SPEED: [f64; 2] = [0.02, 0.035];
const MARGIN: f64 = 0.15;
const SLACK: f64 = 0.2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Fill {
    geo: [f32; 4],
    params: [f32; 4],
}

struct Layer {
    view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    size: [u32; 2],
    lo: f64,
    hi: f64,
    valid: bool,
    drawn: AtomicBool,
    runs: [[u32; 2]; 2],
}

pub(super) struct Buildings {
    layers: [Layer; 6],
    span: f64,
    aspect: f64,
    palette: [[f32; 3]; 5],
    state: wgpu::Buffer,
}

impl Buildings {
    pub fn new(device: &wgpu::Device, uniform: &wgpu::Buffer, canvas: [u32; 2]) -> Self {
        let aspect = f64::from(canvas[0]) / f64::from(canvas[1]);
        let span = aspect + 2.0 * MARGIN + SLACK;
        let fill_layout = layout(device, &[uniform_entry(0), uniform_entry(16)]);
        let layers = std::array::from_fn(|index| {
            let level = index / 2;
            let [_, y0, y1] = LEVELS[level];
            let density = if level == 0 { f64::from(canvas[1]) * 0.5 } else { f64::from(canvas[1].div_ceil(4)) };
            let density = density.min(f64::from(device.limits().max_texture_dimension_2d) / span);
            let size = [(span * density).ceil().max(1.0) as u32, ((y1 - y0) * density).ceil().max(1.0) as u32];
            let fill = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("folia-nebbia-building-fill"),
                size: std::mem::size_of::<Fill>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("folia-nebbia-building-fill"),
                layout: &fill_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 16, resource: fill.as_entire_binding() },
                ],
            });
            Layer {
                view: target_view(device, size, wgpu::TextureFormat::Rgba8Unorm),
                uniform: fill,
                bind,
                size,
                lo: 0.0,
                hi: 0.0,
                valid: false,
                drawn: AtomicBool::new(false),
                runs: [[0; 2]; 2],
            }
        });
        let state = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-nebbia-building-state"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { layers, span, aspect, palette: [[0.0; 3]; 5], state }
    }

    pub fn entries(&self) -> [wgpu::BindGroupEntry<'_>; 7] {
        std::array::from_fn(|i| wgpu::BindGroupEntry {
            binding: 5 + i as u32,
            resource: if i == 0 {
                self.state.as_entire_binding()
            } else {
                wgpu::BindingResource::TextureView(&self.layers[i - 1].view)
            },
        })
    }

    // Schedules at most one full texture per frame; a level is usable only after both layers are filled.
    pub fn prepare(&mut self, queue: &wgpu::Queue, uniforms: &Uniforms) {
        let colors = [uniforms.sky0, uniforms.sky1, uniforms.la, uniforms.lb, uniforms.lc].map(|c| [c[0], c[1], c[2]]);
        if colors != self.palette {
            for layer in &mut self.layers {
                layer.valid = false;
            }
            self.palette = colors;
        }
        let t = f64::from(uniforms.time * uniforms.bus);
        let mut full = false;
        for (index, layer) in self.layers.iter_mut().enumerate() {
            layer.runs = [[0; 2]; 2];
            layer.valid &= layer.drawn.load(Ordering::Relaxed);
            let off = t * SPEED[index % 2];
            let (lo, hi) = (off - MARGIN, off + self.aspect + MARGIN);
            if layer.valid && lo >= layer.lo && hi <= layer.hi {
                continue;
            }
            let ahead = layer.valid && lo >= layer.lo && hi - layer.hi < self.span / 2.0;
            let (a, b) = if ahead {
                let next = hi + SLACK / 4.0;
                let previous = layer.hi;
                layer.hi = next;
                layer.lo = next - self.span;
                (previous, next)
            } else {
                layer.valid = false;
                if full {
                    continue;
                }
                full = true;
                layer.lo = lo - SLACK / 4.0;
                layer.hi = layer.lo + self.span;
                (layer.lo, layer.hi)
            };
            let (x0, runs) = ring_columns(a, b, self.span, layer.size[0]);
            layer.runs = runs;
            layer.valid = true;
            layer.drawn.store(false, Ordering::Relaxed);
            let [blur, y0, y1] = LEVELS[index / 2];
            let fill = Fill {
                geo: [y0 as f32, y1 as f32, self.span as f32, blur as f32],
                params: [x0 as f32, (index % 2) as f32, layer.size[0] as f32, layer.size[1] as f32],
            };
            queue.write_buffer(&layer.uniform, 0, bytemuck::bytes_of(&fill));
        }
        let geo: [[f32; 4]; 3] = std::array::from_fn(|i| {
            let on = self.layers[i * 2].valid && self.layers[i * 2 + 1].valid;
            [LEVELS[i][1] as f32, LEVELS[i][2] as f32, self.span as f32, f32::from(on)]
        });
        queue.write_buffer(&self.state, 0, bytemuck::cast_slice(&geo));
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, pipeline: &wgpu::RenderPipeline) {
        for layer in &self.layers {
            if layer.runs[0][1] == 0 {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-nebbia-building-fill"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &layer.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &layer.bind, &[]);
            for [start, count] in layer.runs {
                if count == 0 {
                    continue;
                }
                pass.set_scissor_rect(start, 0, count, layer.size[1]);
                pass.draw(0..3, 0..1);
            }
            layer.drawn.store(true, Ordering::Relaxed);
        }
    }
}

fn ring_columns(a: f64, b: f64, span: f64, width: u32) -> (f64, [[u32; 2]; 2]) {
    let ca = (a / span * f64::from(width)).floor();
    let count = ((b / span * f64::from(width)).ceil() - ca).clamp(0.0, f64::from(width)) as u32;
    let start = (ca as i64).rem_euclid(i64::from(width)) as u32;
    let first = count.min(width - start);
    (ca * span / f64::from(width), [[start, first], [0, count - first]])
}
