// Nebbia glass, ring-cached buildings, static noise, quarter/eighth-size fog and the window composite.
// Pipelines live as long as the renderer; textures change with the size, instance buffers only grow.

use bytemuck::{Pod, Zeroable};
use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Rectangle, Size};
use std::sync::Weak;
#[cfg(feature = "dev")]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "dev")]
static LIVE_SLOTS: AtomicUsize = AtomicUsize::new(0);

/// Renderers owned by widgets that are still alive. Release builds omit the counter.
#[cfg(feature = "dev")]
pub fn live_slots() -> usize {
    LIVE_SLOTS.load(Ordering::Relaxed)
}

use folia_core::Theme;

use super::cache::Buildings;
use super::palette::palette;
use super::place::{self, Knobs, MASK_SCALE};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Uniforms {
    pub resolution: [f32; 2],
    pub mask_size: [f32; 2],
    pub time: f32,
    pub bus: f32,
    pub neon: f32,
    pub mist: f32,
    pub spot_i: f32,
    pub spot_r: f32,
    /// Physical pixels per CSS pixel; set by the renderer.
    pub scale: f32,
    /// 1 when the target stores sRGB and the shader must hand back linear color; set by the renderer.
    pub encode: f32,
    pub spot_tgt: [f32; 2],
    /// The two spotlight sources' x in scene units; both stand at y 0.595.
    pub spot_src: [f32; 2],
    pub sky0: [f32; 4],
    pub sky1: [f32; 4],
    pub la: [f32; 4],
    pub lb: [f32; 4],
    pub lc: [f32; 4],
    /// Up to eight breath ellipses, xy centre, zw radii. A non-positive radius is unused.
    pub lobes: [[f32; 4]; 8],
    /// Eight (shape, dense) pairs, two per vec4.
    pub lobe_sd: [[f32; 4]; 4],
    /// Scene, glass style, scenery/rain bit mask, and transparent glass.
    pub environment: [f32; 4],
    pub water: [f32; 4],
    pub mist_tint: [f32; 4],
    pub jelly: [[f32; 4]; 24],
    pub jelly_c: [[f32; 4]; 24],
    pub bursts: [[f32; 4]; 12],
    pub sway: [f32; 4],
    pub aqua: [f32; 4],
    pub aqua_motion: [f32; 4],
    pub snow: [f32; 4],
    pub motion: [f32; 4],
}

/// One frame for the window shader. Segments and drops belong to this frame, not a process-wide buffer.
#[derive(Clone)]
pub struct Pass {
    pub uniforms: Uniforms,
    pub segments: Vec<super::place::Segment>,
    pub drops: Vec<super::place::Drop>,
    /// Local environment time advanced this frame, independent of lyric seeks, clamped to 100 ms.
    pub elapsed: f32,
    /// The playing page publishes into the shared glass source. A second scene must not replace it.
    pub publish_blur: bool,
    pub(super) knobs: Knobs,
    pub(super) ice: Vec<[f32; 6]>,
    pub(super) sweeps: Vec<place::WetSegment>,
    pub(super) wet: Vec<place::WetSegment>,
    motion: super::beats::DropsMotion,
}

impl std::fmt::Debug for Pass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pass")
            .field("uniforms", &self.uniforms)
            .field("segments", &self.segments.len())
            .field("drops", &self.drops.len())
            .finish_non_exhaustive()
    }
}

pub(super) struct FrameIn<'a> {
    pub render_scale: f32,
    pub static_mode: bool,
    pub lyrics: &'a folia_core::Lyrics,
    pub time: f64,
    pub size: Size,
    pub theme: &'a Theme,
    pub font_scale: f32,
    pub tuning: &'a crate::tuning::Values,
    pub energy: f32,
    pub elapsed: f32,
    pub publish_blur: bool,
    pub see_through: bool,
    pub daylight: bool,
    pub smart: folia_core::SmartBeat<'a>,
    pub voice: &'a mut super::beats::VocalRain,
    pub motion: &'a mut super::beats::Motion,
}

/// One frame: lays out the song (cached), collects this frame's wipes and drops, fills the uniforms.
pub fn frame(
    lyrics: &folia_core::Lyrics,
    time: f64,
    size: Size,
    theme: &Theme,
    font_scale: f32,
    tuning: &crate::tuning::Values,
    energy: f32,
) -> Pass {
    let mut paint = place::Paint::default();
    let mut voice = super::beats::VocalRain::default();
    let mut motion = super::beats::Motion::default();
    frame_in(
        FrameIn {
            render_scale: 1.0,
            static_mode: false,
            lyrics,
            time,
            size,
            theme,
            font_scale,
            tuning,
            energy,
            elapsed: 0.0,
            publish_blur: false,
            see_through: false,
            daylight: false,
            smart: Default::default(),
            voice: &mut voice,
            motion: &mut motion,
        },
        &mut paint,
    )
}

pub(super) fn frame_in(frame: FrameIn<'_>, paint: &mut place::Paint) -> Pass {
    let FrameIn {
        render_scale,
        static_mode,
        lyrics,
        time,
        size,
        theme,
        font_scale,
        tuning,
        energy,
        elapsed,
        publish_blur,
        see_through,
        daylight,
        smart,
        voice,
        motion,
    } = frame;
    super::ink::warm(lyrics);
    let list = super::tuning::manifest();
    let (width, height) = (size.width.max(1.0), size.height.max(1.0));
    let scene = super::scene::scene(lyrics, width, height, font_scale, tuning);
    let dew = tuning.choice(list, "glass") == "dew";
    let snowy = tuning.choice(list, "glass") == "snow";
    let rainy = !dew && !snowy;
    let aquarium = tuning.choice(list, "behind") == "aquarium";
    let knobs = Knobs {
        stroke_width: tuning.number(list, "strokeWidth"),
        write_speed: tuning.number(list, "writeSpeed").max(0.4),
        refog: f64::from(tuning.number(list, "refog")).max(0.5),
        rain_hit: if rainy && !static_mode { tuning.number(list, "rainHit") * smart.rain_strength } else { 0.0 },
        rain_big: if rainy { tuning.number(list, "rainBig") * smart.rain_strength } else { 0.0 },
        ice: tuning.choice(list, "glass") == "snow",
        ice_scale: height / 1000.0,
        water: !rainy,
    };
    voice.update(smart.vocal, &scene, [width, height]);
    let drive = place::paint_with_voice(
        &scene,
        time,
        [width, height],
        &knobs,
        (energy, voice.active().then_some(&*voice)),
        paint,
    );
    let (mut segments, mut drops, ice) = paint.take_sprites();
    if render_scale != 1.0 {
        for segment in &mut segments {
            for coordinate in &mut segment[..5] {
                *coordinate *= render_scale;
            }
        }
    }
    let colors = palette(theme);
    let beat = motion.step(smart, time, [width, height], tuning, &colors);
    beat.drops.apply(&mut drops);
    let aspect = width / height;
    let (jelly, jelly_c) = if let Some(jelly) = beat.jelly {
        jelly
    } else if aquarium {
        super::aquarium::jellies(time, aspect, tuning.number(list, "jellySize"), &colors)
    } else {
        ([[0.0; 4]; 24], [[0.0; 4]; 24])
    };
    let spot_x = tuning.number(list, "spotX");
    let pad = |rgb: [f32; 3]| [rgb[0], rgb[1], rgb[2], 0.0];
    let [sx, sy, sr, si] = drive.spot;
    Pass {
        uniforms: Uniforms {
            resolution: [width, height],
            mask_size: mask_size(width * render_scale, height * render_scale).map(|v| v as f32),
            time: time as f32,
            bus: tuning.number(list, "busSpeed"),
            neon: tuning.number(list, "neon"),
            mist: tuning.number(list, "mist"),
            spot_i: if tuning.member(list, "scenery", "spot") { si * tuning.number(list, "spot") } else { 0.0 },
            spot_r: (sr / height * 0.75).max(0.02) * beat.spot_scale,
            scale: 1.0,
            encode: 0.0,
            spot_tgt: [sx / width * aspect, sy / height],
            spot_src: [spot_x * aspect, (1.0 - spot_x) * aspect],
            sky0: {
                let mut color = pad(colors.sky0);
                color[3] = if aquarium || static_mode { 0.0 } else { tuning.number(list, "bump") };
                color
            },
            sky1: pad(colors.sky1),
            la: pad(colors.la),
            lb: pad(colors.lb),
            lc: pad(colors.lc),
            lobes: drive.lobes,
            lobe_sd: drive.lobe_sd,
            environment: [
                f32::from(aquarium),
                if snowy { 2.0 } else { f32::from(dew) },
                super::tuning::scenery(tuning) as f32,
                f32::from(see_through || tuning.choice(list, "behind") == "backdrop"),
            ],
            water: [tuning.number(list, "dew"), tuning.number(list, "dewSize"), tuning.number(list, "dewGlint"), 0.0],
            mist_tint: if daylight { [0.86, 0.89, 0.92, 0.0] } else { [0.07, 0.08, 0.1, 0.0] },
            jelly,
            jelly_c,
            bursts: beat.bursts,
            sway: beat.sway,
            aqua: [
                tuning.number(list, "jellies").round(),
                tuning.number(list, "bubbles"),
                beat.burst_count,
                beat.beat_rows,
            ],
            aqua_motion: [beat.weed_idle, 0.0, 0.0, 0.0],
            snow: [
                tuning.number(list, "snow"),
                tuning.number(list, "snowSize"),
                tuning.number(list, "snowMelt"),
                tuning.number(list, "hold"),
            ],
            motion: [beat.drops.shift[0] / width, beat.drops.shift[1] / height, f32::from(beat.drops.driven), 0.0],
        },
        segments,
        drops,
        publish_blur,
        elapsed: elapsed.clamp(0.0, 0.1),
        knobs,
        ice,
        sweeps: std::mem::take(&mut paint.sweeps),
        wet: std::mem::take(&mut paint.wet),
        motion: beat.drops,
    }
}

fn mask_size(width: f32, height: f32) -> [u32; 2] {
    [(width * MASK_SCALE).round().max(1.0) as u32, (height * MASK_SCALE).round().max(1.0) as u32]
}

const INSTANCE: u64 = std::mem::size_of::<[f32; 6]>() as u64;

/// A growing instance buffer and how many instances it holds this frame.
struct Instances {
    buffer: wgpu::Buffer,
    capacity: u64,
    count: u32,
}

impl Instances {
    fn new(device: &wgpu::Device) -> Self {
        Self { buffer: instance_buffer(device, 256), capacity: 256, count: 0 }
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, items: &[[f32; 6]]) {
        let n = items.len() as u64;
        if n > self.capacity {
            self.capacity = n.next_power_of_two();
            self.buffer = instance_buffer(device, self.capacity);
        }
        if n > 0 {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(items));
        }
        self.count = n as u32;
    }
}

struct Targets {
    css: [u32; 2],
    canvas: [u32; 2],
    mask_size: [u32; 2],
    mask: wgpu::TextureView,
    drops: wgpu::TextureView,
    field: wgpu::TextureView,
    field_dirty: AtomicBool,
    bind: wgpu::BindGroup,
    buildings: Buildings,
    fog: [wgpu::TextureView; 3],
    dew: bool,
    fog_bind: wgpu::BindGroup,
    noise: wgpu::TextureView,
    noise_dirty: AtomicBool,
}

/// Every nebbia pass for one target format. The app draws through it; anything with a wgpu device can too.
pub struct Renderer {
    window: wgpu::RenderPipeline,
    fill: wgpu::RenderPipeline,
    fog: [wgpu::RenderPipeline; 3],
    noise: wgpu::RenderPipeline,
    noise_bind: wgpu::BindGroup,
    mask: wgpu::RenderPipeline,
    drop: wgpu::RenderPipeline,
    window_layout: wgpu::BindGroupLayout,
    fog_layout: wgpu::BindGroupLayout,
    ring_sampler: wgpu::Sampler,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    glass_bind: wgpu::BindGroup,
    targets: Option<Targets>,
    segments: Instances,
    drops: Instances,
    field: Instances,
    field_key: u64,
    base_field: Vec<super::place::Drop>,
    field_sprites: Vec<super::place::Drop>,
    field_was_moving: bool,
    moving_field: super::drops::MotionField,
    combined_drops: Vec<super::place::Drop>,
    condensation: super::condensation::Condensation,
    snow: super::snow::Snow,
    particles: super::particles::Gpu,
    fireflies: super::condensation::fireflies::Fireflies,
    active_snow: bool,
    active_dew: bool,
    mask_segments: Vec<super::place::Segment>,
    stamp: u64,
    environment: [f32; 2],
    environment_frame: u64,
    encode: bool,
    bounds: [f32; 4],
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let window_source = [
            include_str!("../../../shaders/nebbia/window.wgsl"),
            include_str!("../../../shaders/nebbia/aquarium.wgsl"),
        ]
        .concat();
        let window_module = shader_module(device, &window_source);
        let glass_module = shader_module(device, include_str!("../../../shaders/nebbia/glass.wgsl"));
        let mut window_entries = vec![
            uniform_entry(0),
            texture_entry(1),
            sampler_entry(2),
            texture_entry(3),
            texture_entry(4),
            uniform_entry(5),
        ];
        window_entries.extend((6..12).map(texture_entry));
        window_entries.extend([
            sampler_entry(12),
            texture_entry(13),
            texture_entry(14),
            texture_entry(15),
            texture_entry(17),
        ]);
        let window_layout = layout(device, &window_entries);
        let mut fog_entries = vec![uniform_entry(0), uniform_entry(5), sampler_entry(12)];
        fog_entries.extend((6..12).map(texture_entry));
        let fog_layout = layout(device, &fog_entries);
        let fill_layout = layout(device, &[uniform_entry(0), uniform_entry(16)]);
        let noise_layout = layout(device, &[]);
        let noise_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-noise"),
            layout: &noise_layout,
            entries: &[],
        });
        let glass_layout = layout(device, &[uniform_entry(0), texture_entry(1), sampler_entry(2)]);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("folia-nebbia-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-nebbia"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let sprite = sprite_view(device, queue);
        let ring_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-nebbia-ring"),
            address_mode_u: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let glass_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-glass"),
            layout: &glass_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&sprite) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let max = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Max,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        // raindrop-fx's "smoother": screen-like on color, over on alpha.
        let smoother = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::OneMinusDst,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let glass = Stage { device, layout: &glass_layout, module: &glass_module };
        let (particles, lumps) = super::particles::Gpu::new(device, queue, format);
        Self {
            fill: Stage { device, layout: &fill_layout, module: &window_module }.pipeline(
                "vs",
                "fs_fill",
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::BlendState::REPLACE,
                false,
            ),
            fog: ["fs_fog_blur", "fs_fog_glow", "fs_fog_sharp"].map(|entry| {
                Stage { device, layout: &fog_layout, module: &window_module }.pipeline(
                    "vs",
                    entry,
                    wgpu::TextureFormat::Rgba16Float,
                    wgpu::BlendState::REPLACE,
                    false,
                )
            }),
            noise: Stage { device, layout: &noise_layout, module: &window_module }.pipeline(
                "vs",
                "fs_noise",
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::BlendState::REPLACE,
                false,
            ),
            noise_bind,
            fog_layout,
            ring_sampler,
            window: Stage { device, layout: &window_layout, module: &window_module }.pipeline(
                "vs",
                "fs",
                format,
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
                false,
            ),
            mask: glass.pipeline("vs_mask", "fs_mask", wgpu::TextureFormat::R8Unorm, max, true),
            drop: glass.pipeline("vs_drop", "fs_drop", wgpu::TextureFormat::Rgba8Unorm, smoother, true),
            window_layout,
            sampler,
            uniform,
            glass_bind,
            targets: None,
            segments: Instances::new(device),
            drops: Instances::new(device),
            field: Instances::new(device),
            field_key: 0,
            base_field: Vec::new(),
            field_sprites: Vec::new(),
            field_was_moving: false,
            moving_field: Default::default(),
            combined_drops: Vec::new(),
            condensation: super::condensation::Condensation::default(),
            snow: super::snow::Snow::new(lumps),
            particles,
            fireflies: Default::default(),
            active_snow: false,
            active_dew: false,
            mask_segments: Vec::new(),
            stamp: 0,
            environment: [-1.0; 2],
            environment_frame: 0,
            encode: format.is_srgb(),
            bounds: [0.0; 4],
            device: device.clone(),
            queue: queue.clone(),
            format,
        }
    }

    /// Uploads this frame. `bounds` is the drawing area in physical pixels, `scale` physical per CSS pixel.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pass: &Pass, bounds: [f32; 4], scale: f32) {
        let uniforms = Uniforms { scale, encode: if self.encode { 1.0 } else { 0.0 }, ..pass.uniforms };
        let environment = [uniforms.environment[0], uniforms.environment[1]];
        if environment != self.environment {
            self.environment = environment;
            self.snow.clear();
            self.condensation = Default::default();
            self.fireflies = Default::default();
        }
        if pass.elapsed > 0.0 {
            self.environment_frame = self.environment_frame.wrapping_add(1);
        }
        use std::hash::Hasher;
        let mut hash = std::hash::DefaultHasher::new();
        hash.write(bytemuck::bytes_of(&uniforms));
        hash.write(bytemuck::cast_slice(&pass.segments));
        hash.write(bytemuck::cast_slice(&pass.drops));
        hash.write_u64(self.environment_frame);
        self.stamp = hash.finish();
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniforms));
        self.bounds = bounds;
        let [width, height] = uniforms.resolution;
        let css = [width.round().max(1.0) as u32, height.round().max(1.0) as u32];
        let canvas = [(width * scale).round().max(1.0) as u32, (height * scale).round().max(1.0) as u32];
        let limit = device.limits().max_texture_dimension_2d as f32;
        let mask = uniforms.mask_size.map(|side| side.round().clamp(1.0, limit) as u32);
        let dew = uniforms.environment[1] == 1.0;
        if self.targets.as_ref().is_none_or(|targets| {
            targets.css != css || targets.canvas != canvas || targets.mask_size != mask || targets.dew != dew
        }) {
            self.targets = Some(self.targets_for(device, css, canvas, mask, dew));
            self.condensation.invalidate();
            if let Some(targets) = &self.targets {
                self.particles.bind(device, &targets.fog[1], &targets.fog[0]);
            }
        }
        if let Some(targets) = &mut self.targets {
            targets.buildings.prepare(queue, &uniforms);
        }
        self.drops.write(device, queue, &pass.drops);
        self.active_snow = uniforms.environment[1] == 2.0;
        self.active_dew = dew;
        if self.active_snow {
            self.snow.prepare(pass);
            self.particles.snow(device, queue, &uniforms, self.encode, &self.snow.flakes);
            self.mask_segments.clear();
            self.mask_segments.extend_from_slice(&pass.segments);
            let mask_scale = uniforms.mask_size[0] / width;
            self.mask_segments.extend(self.snow.fog.iter().map(|segment| {
                let mut segment = *segment;
                for value in &mut segment[..5] {
                    *value *= mask_scale;
                }
                segment
            }));
            self.segments.write(device, queue, &self.mask_segments);
            self.field.write(device, queue, &self.snow.drops);
            if let Some(targets) = &self.targets {
                targets.field_dirty.store(true, Ordering::Relaxed);
            }
            self.field_key = 0;
            self.condensation.invalidate();
            return;
        }
        self.segments.write(device, queue, &pass.segments);
        if uniforms.environment[1] == 1.0 {
            let rgb = |color: [f32; 4]| [color[0], color[1], color[2]];
            let colors = super::palette::Palette {
                sky0: rgb(uniforms.sky0),
                sky1: rgb(uniforms.sky1),
                la: rgb(uniforms.la),
                lb: rgb(uniforms.lb),
                lc: rgb(uniforms.lc),
            };
            let flies = self.fireflies.update([width, height], pass.elapsed, &colors);
            self.particles.fireflies(device, queue, &uniforms, self.encode, flies);
            if let Some(drops) = self.condensation.update(
                [width, height],
                pass.elapsed,
                [uniforms.water[0], uniforms.water[1]],
                &pass.sweeps,
                &pass.wet,
            ) {
                self.field.write(device, queue, drops);
                if let Some(targets) = &self.targets {
                    targets.field_dirty.store(true, Ordering::Relaxed);
                }
            }
            self.field_key = 0;
            return;
        }
        self.condensation.invalidate();
        let base = &mut self.base_field;
        let field_key = super::drops::field(width, height, self.field_key, |drops| {
            base.clear();
            base.extend_from_slice(drops);
        });
        if field_key != self.field_key {
            self.moving_field.clear();
        }
        if self.moving_field.update(
            &self.base_field,
            &pass.motion,
            uniforms.bus,
            uniforms.snow[3],
            pass.knobs.refog as f32,
            height,
        ) {
            self.field.write(device, queue, &self.moving_field.sprites);
            if !self.moving_field.drops.is_empty() {
                self.combined_drops.clear();
                self.combined_drops.extend_from_slice(&pass.drops);
                self.combined_drops.extend_from_slice(&self.moving_field.drops);
                self.drops.write(device, queue, &self.combined_drops);
            }
            if !self.moving_field.fog.is_empty() {
                let scale = uniforms.mask_size[0] / width;
                self.mask_segments.clear();
                self.mask_segments.extend_from_slice(&pass.segments);
                self.mask_segments.extend(self.moving_field.fog.iter().map(|segment| {
                    let mut segment = *segment;
                    for value in &mut segment[..5] {
                        *value *= scale;
                    }
                    segment
                }));
                self.segments.write(device, queue, &self.mask_segments);
            }
            if let Some(targets) = &self.targets {
                targets.field_dirty.store(true, Ordering::Relaxed);
            }
            self.field_key = field_key;
            self.field_was_moving = true;
            return;
        }
        if field_key != self.field_key || pass.motion.active || self.field_was_moving {
            self.field_sprites.clear();
            self.field_sprites.extend_from_slice(&self.base_field);
            pass.motion.apply(&mut self.field_sprites);
            self.field.write(device, queue, &self.field_sprites);
            if let Some(targets) = &self.targets {
                targets.field_dirty.store(true, Ordering::Relaxed);
            }
        }
        self.field_was_moving = pass.motion.active;
        self.field_key = field_key;
    }

    /// Draws the glass passes, then the window into `target` within the prepared bounds, clipped to `clip`.
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, clip: [u32; 4]) {
        let Some(targets) = self.targets.as_ref() else { return };
        self.instanced(encoder, &targets.mask, &self.mask, &self.segments);
        self.instanced(encoder, &targets.drops, &self.drop, &self.drops);
        if targets.field_dirty.swap(false, Ordering::Relaxed) {
            self.instanced(encoder, &targets.field, &self.drop, &self.field);
        }
        targets.buildings.render(encoder, &self.fill);
        if targets.noise_dirty.swap(false, Ordering::Relaxed) {
            fullscreen(encoder, &targets.noise, &self.noise, &self.noise_bind);
        }
        for (view, pipeline) in targets.fog.iter().zip(&self.fog).take(if targets.dew { 3 } else { 2 }) {
            fullscreen(encoder, view, pipeline, &targets.fog_bind);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-nebbia-window"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let [x, y, w, h] = self.bounds;
        pass.set_viewport(x, y, w, h, 0.0, 1.0);
        pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
        pass.set_pipeline(&self.window);
        pass.set_bind_group(0, &targets.bind, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        if self.active_snow || self.active_dew {
            self.particles.render(encoder, target, self.bounds, clip);
        }
    }

    /// The same window pass, into the glass blur's quarter-resolution picture.
    fn paint_blur(&self, encoder: &mut wgpu::CommandEncoder, window: [f32; 2]) {
        let stamp = self.stamp;
        let drew =
            folia_render::draw_source(&self.device, &self.queue, self.format, window, stamp, |view, width, height| {
                let Some(targets) = self.targets.as_ref() else { return };
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("folia-nebbia-blur"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
                pass.set_scissor_rect(0, 0, width, height);
                pass.set_pipeline(&self.window);
                pass.set_bind_group(0, &targets.bind, &[]);
                pass.draw(0..3, 0..1);
                drop(pass);
                if self.active_snow || self.active_dew {
                    self.particles.render(
                        encoder,
                        view,
                        [0.0, 0.0, width as f32, height as f32],
                        [0, 0, width, height],
                    );
                }
            });
        if drew {
            folia_render::blur_source(encoder);
        }
    }

    fn instanced(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        pipeline: &wgpu::RenderPipeline,
        items: &Instances,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-nebbia-glass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if items.count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.glass_bind, &[]);
        pass.set_vertex_buffer(0, items.buffer.slice(..u64::from(items.count) * INSTANCE));
        pass.draw(0..4, 0..items.count);
    }

    fn targets_for(
        &self,
        device: &wgpu::Device,
        css: [u32; 2],
        canvas: [u32; 2],
        mask: [u32; 2],
        dew: bool,
    ) -> Targets {
        let mask_size = mask;
        let mask = target_view(device, mask, wgpu::TextureFormat::R8Unorm);
        let drops = target_view(device, mask_size, wgpu::TextureFormat::Rgba8Unorm);
        let field = target_view(device, mask_size, wgpu::TextureFormat::Rgba8Unorm);
        let buildings = Buildings::new(device, &self.uniform, canvas);
        let fog = [4, 8, if dew { 4 } else { 0 }].map(|div| {
            target_view(
                device,
                if div == 0 { [1, 1] } else { canvas.map(|v| v.div_ceil(div)) },
                wgpu::TextureFormat::Rgba16Float,
            )
        });
        let noise = target_view(device, canvas, wgpu::TextureFormat::Rgba8Unorm);
        let mut fog_entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 12, resource: wgpu::BindingResource::Sampler(&self.ring_sampler) },
        ];
        fog_entries.extend(buildings.entries());
        let fog_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-fog"),
            layout: &self.fog_layout,
            entries: &fog_entries,
        });
        let mut entries = fog_entries;
        entries.extend([
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&mask) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&drops) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&field) },
            wgpu::BindGroupEntry { binding: 13, resource: wgpu::BindingResource::TextureView(&fog[0]) },
            wgpu::BindGroupEntry { binding: 14, resource: wgpu::BindingResource::TextureView(&fog[1]) },
            wgpu::BindGroupEntry { binding: 15, resource: wgpu::BindingResource::TextureView(&noise) },
            wgpu::BindGroupEntry { binding: 17, resource: wgpu::BindingResource::TextureView(&fog[2]) },
        ]);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-nebbia-window"),
            layout: &self.window_layout,
            entries: &entries,
        });
        Targets {
            dew,
            css,
            canvas,
            mask_size,
            mask,
            drops,
            field,
            field_dirty: AtomicBool::new(true),
            bind,
            buildings,
            fog,
            fog_bind,
            noise,
            noise_dirty: AtomicBool::new(true),
        }
    }
}

fn fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("folia-nebbia-prepass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.draw(0..3, 0..1);
}

struct Stage<'a> {
    device: &'a wgpu::Device,
    layout: &'a wgpu::BindGroupLayout,
    module: &'a wgpu::ShaderModule,
}

impl Stage<'_> {
    fn pipeline(
        &self,
        vs: &str,
        fs: &str,
        format: wgpu::TextureFormat,
        blend: wgpu::BlendState,
        instanced: bool,
    ) -> wgpu::RenderPipeline {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x2];
        let instances = [wgpu::VertexBufferLayout {
            array_stride: INSTANCE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }];
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-nebbia"),
            bind_group_layouts: &[self.layout],
            push_constant_ranges: &[],
        });
        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-nebbia"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: self.module,
                entry_point: Some(vs),
                buffers: if instanced { &instances } else { &[] },
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: self.module,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: if instanced {
                    wgpu::PrimitiveTopology::TriangleStrip
                } else {
                    wgpu::PrimitiveTopology::TriangleList
                },
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
}

fn shader_module(device: &wgpu::Device, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("folia-nebbia"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

pub(super) fn layout(device: &wgpu::Device, entries: &[wgpu::BindGroupLayoutEntry]) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("folia-nebbia"), entries })
}

pub(super) fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(super) fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-nebbia-instances"),
        size: capacity * INSTANCE,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(super) fn target_view(device: &wgpu::Device, size: [u32; 2], format: wgpu::TextureFormat) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-nebbia-glass"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// The raindrop sprite with its mip chain; a 1x1 clear texel when the image cannot be read.
fn sprite_view(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let empty = [(1, 1, vec![0u8; 4])];
    let levels: &[(u32, u32, Vec<u8>)] = super::drops::sprite().map_or(&empty, |sprite| &sprite.levels);
    let (width, height, _) = &levels[0];
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-nebbia-sprite"),
        size: wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, (w, h, pixels)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(*h) },
            wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

#[derive(Debug)]
pub struct LivePass {
    pub(super) frame: Pass,
    pub(super) owner: Weak<std::sync::atomic::AtomicBool>,
    pub(super) pool: std::sync::Arc<std::sync::Mutex<place::Paint>>,
}

impl Drop for LivePass {
    fn drop(&mut self) {
        let mut pooled = self.pool.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.frame.segments.capacity() > pooled.segments.capacity() {
            std::mem::swap(&mut pooled.segments, &mut self.frame.segments);
        }
        if self.frame.drops.capacity() > pooled.drops.capacity() {
            std::mem::swap(&mut pooled.drops, &mut self.frame.drops);
        }
        if self.frame.ice.capacity() > pooled.ice.capacity() {
            std::mem::swap(&mut pooled.ice, &mut self.frame.ice);
        }
        if self.frame.sweeps.capacity() > pooled.sweeps.capacity() {
            std::mem::swap(&mut pooled.sweeps, &mut self.frame.sweeps);
        }
        if self.frame.wet.capacity() > pooled.wet.capacity() {
            std::mem::swap(&mut pooled.wet, &mut self.frame.wet);
        }
    }
}

impl ShaderPrimitive for LivePass {
    type Pipeline = NebbiaGpu;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let physical = [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale];
        let key = self.owner.as_ptr() as usize;
        let Some(renderer) = pipeline.slot(key, self.owner.clone(), device, queue) else { return };
        renderer.prepare(device, queue, &self.frame, physical, scale);
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let key = self.owner.as_ptr() as usize;
        let Some(renderer) = pipeline.renderer(key) else { return };
        renderer.render(encoder, target, [clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height]);
        if self.frame.publish_blur {
            let window = self.frame.uniforms.resolution;
            renderer.paint_blur(encoder, window);
        }
    }
}

struct SceneSlot {
    key: usize,
    owner: Weak<std::sync::atomic::AtomicBool>,
    renderer: folia_render::Prepared<Renderer>,
}

impl Drop for SceneSlot {
    fn drop(&mut self) {
        #[cfg(feature = "dev")]
        LIVE_SLOTS.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Shared pipelines would also share the only uniforms and textures. Each live widget keeps its own renderer.
pub struct NebbiaGpu {
    slots: Vec<SceneSlot>,
    format: wgpu::TextureFormat,
}

impl NebbiaGpu {
    fn slot(
        &mut self,
        key: usize,
        owner: Weak<std::sync::atomic::AtomicBool>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<&mut Renderer> {
        if let Some(index) = self.slots.iter().position(|slot| slot.key == key) {
            self.slots[index].owner = owner;
            return self.slots[index].renderer.poll();
        }
        #[cfg(feature = "dev")]
        LIVE_SLOTS.fetch_add(1, Ordering::Relaxed);
        let (device, queue, format) = (device.clone(), queue.clone(), self.format);
        let renderer = folia_render::Prepared::new(owner.clone(), move || Renderer::new(&device, &queue, format));
        self.slots.push(SceneSlot { key, owner, renderer });
        let index = self.slots.len() - 1;
        self.slots[index].renderer.poll()
    }

    fn renderer(&self, key: usize) -> Option<&Renderer> {
        self.slots.iter().find(|slot| slot.key == key).and_then(|slot| slot.renderer.get())
    }
}

impl shader::Pipeline for NebbiaGpu {
    fn new(_device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self { slots: Vec::new(), format }
    }

    fn trim(&mut self) {
        // The widget owns the lifetime, so another window's present cannot evict an active style.
        self.slots.retain(|slot| slot.owner.strong_count() > 0);
    }
}
