// Latent (隐现): a mesh gradient under a warp dither, cover colours falling back to the theme.

use std::cell::Cell;
#[cfg(feature = "dev")]
use std::sync::atomic::AtomicU32;
#[cfg(feature = "dev")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use folia_core::{AudioFrame, Rgb, Theme};
use iced::widget::shader::{self, Primitive as ShaderPrimitive};
use iced::{Element, Length, Rectangle, mouse};

use crate::Ground;
use crate::clock;

const SHADER: &str = include_str!("../shaders/backgrounds/latent.wgsl");

/// `displayMode`: pixel, fluid, or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerMode {
    Dither,
    Mesh,
    Both,
}

/// `colorSource`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    CoverTheme,
    CoverOnly,
}

/// Defaults from `DEFAULT_LATENT_BACKGROUND_TUNING`. Beat-clock pulses stay off until that model exists.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    pub layers: LayerMode,
    pub colors: ColorMode,
    pub dynamic_only: bool,
    pub mesh_speed: f32,
    pub mesh_audio: f32,
    pub dither_speed: f32,
    pub dither_audio: f32,
    pub distortion: f32,
    pub swirl: f32,
    pub dither_size: f32,
    pub dither_opacity: f32,
    pub overlay_on: bool,
    pub overlay: f32,
    pub enhanced: bool,
}

impl Tuning {
    /// `latent_background_tuning`. Missing keys keep the original defaults.
    pub fn read(value: Option<&toml::Value>) -> Self {
        let Some(toml::Value::Table(table)) = value else { return Self::DEFAULT };
        let number = |key: &str, fallback: f32| match table.get(key) {
            Some(toml::Value::Float(value)) => *value as f32,
            Some(toml::Value::Integer(value)) => *value as f32,
            _ => fallback,
        };
        let flag = |key: &str, fallback: bool| match table.get(key) {
            Some(toml::Value::Boolean(value)) => *value,
            _ => fallback,
        };
        let text = |key: &str| match table.get(key) {
            Some(toml::Value::String(value)) => value.as_str(),
            _ => "",
        };
        Self {
            layers: match text("displayMode") {
                "dithering" => LayerMode::Dither,
                "mesh" => LayerMode::Mesh,
                _ => Self::DEFAULT.layers,
            },
            colors: if text("colorSource") == "cover-only" { ColorMode::CoverOnly } else { Self::DEFAULT.colors },
            dynamic_only: flag("dynamicOnlyInPlayer", Self::DEFAULT.dynamic_only),
            mesh_speed: number("meshSpeed", Self::DEFAULT.mesh_speed),
            mesh_audio: number("meshAudioSpeed", Self::DEFAULT.mesh_audio),
            dither_speed: number("ditheringSpeed", Self::DEFAULT.dither_speed),
            dither_audio: number("ditheringAudioSpeed", Self::DEFAULT.dither_audio),
            distortion: number("meshDistortion", Self::DEFAULT.distortion),
            swirl: number("meshSwirl", Self::DEFAULT.swirl),
            dither_size: number("ditheringSize", Self::DEFAULT.dither_size),
            dither_opacity: number("ditheringOpacity", Self::DEFAULT.dither_opacity),
            overlay_on: flag("overlayEnabled", Self::DEFAULT.overlay_on),
            overlay: number("overlayOpacity", Self::DEFAULT.overlay),
            enhanced: flag("enhancedBeatResponse", Self::DEFAULT.enhanced),
        }
        .clamped()
    }

    /// Keeps each slider inside the range the original card exposes.
    pub fn clamped(self) -> Self {
        Self {
            mesh_speed: self.mesh_speed.clamp(0.0, 2.0),
            mesh_audio: self.mesh_audio.clamp(0.0, 2.0),
            dither_speed: self.dither_speed.clamp(0.0, 2.0),
            dither_audio: self.dither_audio.clamp(0.0, 2.0),
            distortion: self.distortion.clamp(0.0, 2.0),
            swirl: self.swirl.clamp(0.0, 1.0),
            dither_size: self.dither_size.clamp(0.5, 8.0),
            dither_opacity: self.dither_opacity.clamp(0.0, 1.0),
            overlay: self.overlay.clamp(0.0, 1.0),
            ..self
        }
    }

    pub fn table(self) -> toml::map::Map<String, toml::Value> {
        let mut table = toml::map::Map::new();
        let store = |table: &mut toml::map::Map<String, toml::Value>, key: &str, value: f32| {
            table.insert(key.to_owned(), toml::Value::Float(f64::from(value)));
        };
        table.insert(
            "displayMode".to_owned(),
            toml::Value::String(
                match self.layers {
                    LayerMode::Dither => "dithering",
                    LayerMode::Mesh => "mesh",
                    LayerMode::Both => "both",
                }
                .to_owned(),
            ),
        );
        table.insert(
            "colorSource".to_owned(),
            toml::Value::String(
                match self.colors {
                    ColorMode::CoverTheme => "cover-theme",
                    ColorMode::CoverOnly => "cover-only",
                }
                .to_owned(),
            ),
        );
        table.insert("dynamicOnlyInPlayer".to_owned(), toml::Value::Boolean(self.dynamic_only));
        table.insert("overlayEnabled".to_owned(), toml::Value::Boolean(self.overlay_on));
        store(&mut table, "meshSpeed", self.mesh_speed);
        store(&mut table, "meshAudioSpeed", self.mesh_audio);
        store(&mut table, "ditheringSpeed", self.dither_speed);
        store(&mut table, "ditheringAudioSpeed", self.dither_audio);
        store(&mut table, "meshDistortion", self.distortion);
        store(&mut table, "meshSwirl", self.swirl);
        store(&mut table, "ditheringSize", self.dither_size);
        store(&mut table, "ditheringOpacity", self.dither_opacity);
        store(&mut table, "overlayOpacity", self.overlay);
        table.insert("enhancedBeatResponse".to_owned(), toml::Value::Boolean(self.enhanced));
        table
    }

    pub const DEFAULT: Tuning = Tuning {
        layers: LayerMode::Both,
        colors: ColorMode::CoverTheme,
        dynamic_only: true,
        mesh_speed: 0.3,
        mesh_audio: 2.0,
        dither_speed: 0.1,
        dither_audio: 1.2,
        distortion: 0.8,
        swirl: 0.1,
        dither_size: 2.5,
        dither_opacity: 0.55,
        overlay_on: true,
        overlay: 0.35,
        enhanced: true,
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    pixel_ratio: f32,
    mesh_time: f32,
    dither_time: f32,
    distortion: f32,
    swirl: f32,
    dither_size: f32,
    dither_opacity: f32,
    show_mesh: f32,
    show_dither: f32,
    overlay: f32,
    mesh_scale: f32,
    dither_scale: f32,
    saturate: f32,
    brightness: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    dither_back: [f32; 4],
    dither_front: [f32; 4],
    overlay_color: [f32; 4],
    mesh: [[f32; 4]; 6],
}

/// Paper's render size for latent: at least 1 device pixel per CSS pixel, and at most 1280×720 pixels.
pub fn shader_budget(css_width: f32, css_height: f32, device_scale: f32) -> (f32, f32, f32) {
    let css_w = css_width.max(1.0);
    let css_h = css_height.max(1.0);
    let dpr = device_scale.max(1.0);
    let meet = (1.0 / dpr).max(1.0);
    let target_w = css_w * dpr * meet;
    let target_h = css_h * dpr * meet;
    let headroom = (MAX_SHADER_PIXELS / (target_w * target_h).max(1.0)).sqrt().min(1.0);
    let width = (target_w * headroom).round().max(1.0);
    let height = (target_h * headroom).round().max(1.0);
    (width / css_w, width, height)
}

const MAX_SHADER_PIXELS: f32 = 1280.0 * 720.0;

#[cfg(feature = "dev")]
static BUDGET_W: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "dev")]
static BUDGET_H: AtomicU32 = AtomicU32::new(0);

/// Width and height of the latent texture the last frame actually allocated.
#[cfg(feature = "dev")]
pub fn budget_texture_size() -> (u32, u32) {
    (BUDGET_W.load(Ordering::Relaxed), BUDGET_H.load(Ordering::Relaxed))
}

fn note_budget(width: u32, height: u32) {
    #[cfg(feature = "dev")]
    {
        BUDGET_W.store(width, Ordering::Relaxed);
        BUDGET_H.store(height, Ordering::Relaxed);
    }
    let _ = (width, height);
}

const BLIT: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var budget_tex: texture_2d<f32>;
@group(0) @binding(1) var budget_samp: sampler;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(budget_tex, budget_samp, in.uv);
}
"#;

pub struct Layer<'a, Message> {
    ground: Ground<'a>,
    _message: std::marker::PhantomData<Message>,
}

pub fn view<'a, Message: 'a>(ground: Ground<'a>) -> Element<'a, Message> {
    iced::widget::shader(Layer { ground, _message: std::marker::PhantomData })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub struct Motion {
    owner: Arc<()>,
    last_ms: Cell<f64>,
    mesh_time: Cell<f32>,
    dither_time: Cell<f32>,
    playing: Cell<f32>,
    power: Cell<f32>,
    bass: Cell<f32>,
    mid: Cell<f32>,
    energy: Cell<f32>,
    onset: Cell<f32>,
    beat_speed: Cell<f32>,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            owner: Arc::new(()),
            last_ms: Cell::new(0.0),
            mesh_time: Cell::new(0.0),
            dither_time: Cell::new(0.0),
            playing: Cell::new(0.0),
            power: Cell::new(0.0),
            bass: Cell::new(0.0),
            mid: Cell::new(0.0),
            energy: Cell::new(0.0),
            onset: Cell::new(0.0),
            beat_speed: Cell::new(0.0),
        }
    }
}

#[cfg(feature = "dev")]
static LIVE_TARGETS: AtomicUsize = AtomicUsize::new(0);

/// Budget textures still held by a living latent widget.
#[cfg(feature = "dev")]
pub fn live_targets() -> usize {
    LIVE_TARGETS.load(Ordering::Relaxed)
}

impl<Message> shader::Program<Message> for Layer<'_, Message> {
    type State = Motion;
    type Primitive = Pass;

    fn draw(&self, state: &Motion, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let now = clock::wall_ms();
        let still = self.ground.latent.dynamic_only && !self.ground.player;
        let driven = state.drive(&self.ground.latent, self.ground.audio, self.ground.paused, now, still);
        let colors = resolve_colors(self.ground.theme, self.ground.cover, self.ground.latent.colors);
        let show_mesh = if self.ground.latent.layers == LayerMode::Dither { 0.0 } else { 1.0 };
        let show_dither = if self.ground.latent.layers == LayerMode::Mesh { 0.0 } else { 1.0 };
        let overlay = if self.ground.latent.overlay_on { self.ground.latent.overlay } else { 0.0 };
        Pass {
            css: [bounds.width.max(1.0), bounds.height.max(1.0)],
            uniforms: Uniforms {
                resolution: [1.0, 1.0],
                pixel_ratio: 1.0,
                mesh_time: driven.mesh_time,
                dither_time: driven.dither_time,
                distortion: self.ground.latent.distortion + driven.power * 0.62,
                swirl: self.ground.latent.swirl + driven.mid * 0.38,
                dither_size: (self.ground.latent.dither_size - driven.bass * self.ground.latent.dither_size * 0.34)
                    .max(0.5),
                dither_opacity: (self.ground.latent.dither_opacity + driven.bass * 0.25).min(1.0),
                show_mesh,
                show_dither,
                overlay,
                mesh_scale: 1.025 + driven.power * 0.018,
                dither_scale: 1.015 + driven.bass * 0.025,
                saturate: 1.04 + driven.mid * 0.34,
                brightness: 0.94 + driven.power * 0.16,
                _pad0: 0.0,
                _pad1: 0.0,
                _pad2: 0.0,
                _pad3: 0.0,
                dither_back: colors.dither_back,
                dither_front: colors.dither_front,
                overlay_color: colors.overlay,
                mesh: colors.mesh,
            },
            slot: folia_render::Slot::default(),
            publish: self.ground.publish,
            owner: Arc::downgrade(&state.owner),
        }
    }
}

struct Driven {
    mesh_time: f32,
    dither_time: f32,
    power: f32,
    bass: f32,
    mid: f32,
}

impl Motion {
    /// Advances in 16ms frames. A gap is several of those frames, not one jump.
    fn drive(&self, tuning: &Tuning, audio: &AudioFrame, paused: bool, now: f64, still: bool) -> Driven {
        let previous = self.last_ms.get();
        let first = previous == 0.0;
        if first {
            self.playing.set(if paused { 0.0 } else { 1.0 });
        }
        let elapsed = if first { 16.0 } else { (now - previous).max(0.0) };
        self.last_ms.set(now);
        if !first && elapsed == 0.0 {
            return self.snapshot();
        }
        let frames = ((elapsed / 16.0).round() as u32).clamp(1, 180);
        let slice = elapsed / f64::from(frames);
        for _ in 0..frames {
            self.step(tuning, audio, paused, slice, still);
        }
        self.snapshot()
    }

    fn step(&self, tuning: &Tuning, audio: &AudioFrame, paused: bool, dt_ms: f64, still: bool) {
        let pause_step = 1.0 - (-dt_ms / 300.0).exp();
        let playing = ease(self.playing.get(), if paused { 0.0 } else { 1.0 }, pause_step as f32);
        self.playing.set(playing);
        let level = |value: f32| if paused { 0.0 } else { (value / 255.0).clamp(0.0, 1.0) };
        let power = ease(self.power.get(), level(audio.power), 0.12);
        let bass = ease(self.bass.get(), level(audio.bass), 0.16);
        let mid = ease(self.mid.get(), level(audio.mid.max(audio.vocal)), 0.13);
        self.power.set(power);
        self.bass.set(bass);
        self.mid.set(mid);
        let broadband = if paused {
            0.0
        } else {
            (level(audio.bass) * 0.22
                + level(audio.low_mid) * 0.18
                + level(audio.mid) * 0.22
                + level(audio.vocal) * 0.28
                + level(audio.treble) * 0.1)
                .powf(0.55)
        };
        let rise = ((broadband - self.energy.get()) * 7.0).clamp(0.0, 1.0);
        let onset = if tuning.enhanced { (self.onset.get() * 0.84).max(rise) } else { 0.0 };
        self.energy.set(broadband);
        self.onset.set(onset);
        let target = if tuning.enhanced { (broadband * 0.42 + onset * 0.85).clamp(0.0, 1.0) } else { broadband };
        let follow = if target > self.beat_speed.get() { 0.42 } else { 0.14 };
        let beat_speed = ease(self.beat_speed.get(), target, follow);
        self.beat_speed.set(beat_speed);
        let mesh_speed =
            if still { 0.0 } else { shader_speed(tuning.mesh_speed, tuning.mesh_audio, beat_speed, playing) };
        let dither_speed =
            if still { 0.0 } else { shader_speed(tuning.dither_speed, tuning.dither_audio, beat_speed, playing) };
        let dt = (dt_ms / 1000.0) as f32;
        self.mesh_time.set(self.mesh_time.get() + dt * mesh_speed);
        self.dither_time.set(self.dither_time.get() + dt * dither_speed);
    }

    fn snapshot(&self) -> Driven {
        Driven {
            mesh_time: self.mesh_time.get(),
            dither_time: self.dither_time.get(),
            power: self.power.get(),
            bass: self.bass.get(),
            mid: self.mid.get(),
        }
    }
}

fn ease(current: f32, target: f32, amount: f32) -> f32 {
    current + (target - current) * amount
}

fn shader_speed(base: f32, audio: f32, amount: f32, playing: f32) -> f32 {
    let running = base + (audio - base) * amount;
    let paused = base * 0.12;
    (paused + (running - paused) * playing).clamp(0.0, 2.0)
}

pub struct ShaderColors {
    pub dither_back: [f32; 4],
    pub dither_front: [f32; 4],
    pub overlay: [f32; 4],
    pub mesh: [[f32; 4]; 6],
}

/// `resolveLatentShaderColors`. Cover-only walks the cover and falls back inside it; cover-theme keeps the theme veil.
pub fn resolve_colors(theme: &Theme, cover: &[Rgb], source: ColorMode) -> ShaderColors {
    let pick = |index: usize, fallback: Rgb| cover.get(index).copied().unwrap_or(fallback);
    let spot = |rgb: Rgb| [f32::from(rgb.r) / 255.0, f32::from(rgb.g) / 255.0, f32::from(rgb.b) / 255.0, 1.0];
    let primary = pick(0, theme.secondary);
    if source == ColorMode::CoverOnly {
        let secondary = pick(1, primary);
        let tertiary = pick(2, secondary);
        let quaternary = pick(3, primary);
        let quinary = pick(4, secondary);
        let senary = pick(5, tertiary);
        return ShaderColors {
            dither_back: spot(tertiary),
            dither_front: spot(primary),
            overlay: spot(theme.background),
            mesh: [spot(primary), spot(secondary), spot(tertiary), spot(quaternary), spot(quinary), spot(senary)],
        };
    }
    let secondary = pick(1, theme.primary);
    let tertiary = pick(2, primary);
    let quaternary = pick(3, secondary);
    ShaderColors {
        dither_back: spot(theme.background),
        dither_front: spot(primary),
        overlay: spot(theme.background),
        mesh: [
            spot(primary),
            spot(secondary),
            spot(tertiary),
            spot(quaternary),
            spot(theme.background),
            spot(theme.accent),
        ],
    }
}

#[derive(Debug)]
pub struct Pass {
    css: [f32; 2],
    uniforms: Uniforms,
    slot: folia_render::Slot,
    publish: bool,
    owner: Weak<()>,
}

struct BudgetTarget {
    owner: Weak<()>,
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    blit: wgpu::BindGroup,
    place: [f32; 4],
    publish: bool,
}

pub struct Gpu {
    device: wgpu::Device,
    shade: folia_render::Fullscreen,
    blit: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: Vec<BudgetTarget>,
    format: wgpu::TextureFormat,
    _guard: folia_render::PublisherGuard,
}

impl Gpu {
    fn ensure(
        &mut self,
        device: &wgpu::Device,
        owner: &Weak<()>,
        width: u32,
        height: u32,
        place: [f32; 4],
        publish: bool,
    ) {
        let key = owner.as_ptr() as usize;
        if let Some(target) = self.targets.iter_mut().find(|target| target.owner.as_ptr() as usize == key) {
            target.place = place;
            target.publish = publish;
            if target.width == width && target.height == height {
                note_budget(width, height);
                return;
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("folia-latent-budget"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blit = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("folia-latent-blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let target = BudgetTarget { owner: owner.clone(), width, height, texture, view, blit, place, publish };
        if let Some(slot) = self.targets.iter_mut().find(|target| target.owner.as_ptr() as usize == key) {
            *slot = target;
        } else {
            self.targets.push(target);
        }
        note_budget(width, height);
    }
}

impl ShaderPrimitive for Pass {
    type Pipeline = Gpu;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let (pixel_ratio, width, height) = shader_budget(self.css[0], self.css[1], scale);
        let uniforms = Uniforms { resolution: [width, height], pixel_ratio, ..self.uniforms };
        pipeline.shade.write(device, queue, &self.slot, bytemuck::bytes_of(&uniforms));
        let place = [bounds.x * scale, bounds.y * scale, bounds.width * scale, bounds.height * scale];
        pipeline.ensure(
            device,
            &self.owner,
            width.round().max(1.0) as u32,
            height.round().max(1.0) as u32,
            place,
            self.publish,
        );
    }

    fn draw(&self, _pipeline: &Self::Pipeline, _pass: &mut wgpu::RenderPass<'_>) -> bool {
        false
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: &Rectangle<u32>,
    ) {
        if clip.width == 0 || clip.height == 0 {
            return;
        }
        let key = self.owner.as_ptr() as usize;
        let Some(slot) = pipeline.targets.iter().find(|target| target.owner.as_ptr() as usize == key) else {
            return;
        };
        let place = slot.place;
        if place[2] < 1.0 || place[3] < 1.0 {
            return;
        }
        let view = slot.view.clone();
        let blit = slot.blit.clone();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("folia-latent-budget"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pipeline.shade.draw(&mut pass, &self.slot);
        }
        if self.publish {
            let texture = slot.texture.clone();
            let (width, height) = (slot.width, slot.height);
            folia_render::copy_underlay(
                &pipeline.device,
                encoder,
                &texture,
                width,
                height,
                picture_key(&self.uniforms, width, height),
            );
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("folia-latent-blit"),
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
        pass.set_viewport(place[0], place[1], place[2], place[3], 0.0, 1.0);
        pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
        pass.set_pipeline(&pipeline.blit);
        pass.set_bind_group(0, &blit, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl shader::Pipeline for Gpu {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("folia-latent-blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT.into()),
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("folia-latent-blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("folia-latent-blit"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("folia-latent-blit"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("folia-latent-blit"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            device: device.clone(),
            shade: folia_render::Fullscreen::new(
                device,
                format,
                "folia-latent",
                SHADER,
                "fragment",
                std::mem::size_of::<Uniforms>(),
            ),
            blit,
            blit_layout,
            sampler,
            targets: Vec::new(),
            format,
            _guard: folia_render::PublisherGuard::register(),
        }
    }

    fn trim(&mut self) {
        self.targets.retain(|target| target.owner.strong_count() > 0);
        #[cfg(feature = "dev")]
        LIVE_TARGETS.store(self.targets.len(), Ordering::Relaxed);
        self._guard.report(self.targets.iter().any(|target| target.publish));
        self.shade.trim();
    }
}

fn picture_key(uniforms: &Uniforms, width: u32, height: u32) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytemuck::bytes_of(uniforms) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^= u64::from(width);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    hash ^ u64::from(height)
}
