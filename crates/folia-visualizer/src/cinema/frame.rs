// Derives one immutable cinema GPU frame from the seat, controls, clock and audio signals.

use folia_core::{AudioFrame, CinemaOptions, LyricRenderQuality, Rgb, SceneQuality, Theme};
use iced::Size;
use std::sync::Arc;

use super::camera::{Camera, Head};
use super::intermission::{CurtainPose, Intermission, IntermissionFrame, ease};
use super::layout::{SCREEN_Y, selected_seat};

#[derive(Clone, Copy)]
pub struct Config<'a> {
    pub generation: u64,
    pub options: &'a CinemaOptions,
    pub render_quality: LyricRenderQuality,
    pub theme: &'a Theme,
    pub audio: &'a AudioFrame,
    pub song_id: Option<i64>,
    pub cover: Option<&'a Arc<image::RgbaImage>>,
    pub cover_pending: bool,
    pub paused: bool,
    pub static_mode: bool,
    pub reduced_motion: bool,
    pub active: bool,
    pub queue_ended: bool,
    pub publish_blur: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub camera: Camera,
    pub intermission: IntermissionFrame,
    pub quality: SceneQuality,
    pub antialias: folia_core::LyricAntialias,
    pub time: f32,
    pub dt: f32,
    pub smoke_time: f32,
    pub dust_time: f32,
    pub power: f32,
    pub bass: f32,
    pub accent: Rgb,
    pub sample_screen: bool,
    pub static_mode: bool,
    pub reel_change: f32,
}

#[derive(Default)]
pub struct Runtime {
    pub head: Head,
    intermission: Intermission,
    previous: Option<f64>,
    time: f32,
    smoke: f32,
    dust: f32,
    power: f32,
    bass: f32,
    song_id: Option<i64>,
    cover: Option<usize>,
    animate_reel: bool,
    reel_change: f32,
}

impl Runtime {
    pub fn surface_visible(&self) -> bool {
        self.intermission.shutter() > 0.0
    }
    pub fn frame(&mut self, config: Config<'_>, size: Size, now: f64) -> Frame {
        if self.previous.is_none() {
            self.song_id = config.song_id;
            self.reel_change = 1.0;
        } else if self.song_id != config.song_id {
            self.song_id = config.song_id;
            self.animate_reel = !config.paused;
        }
        let dt = if config.active {
            self.previous.map_or(0.0, |last| ((now - last) / 1000.0).clamp(0.0, 0.1) as f32)
        } else {
            0.0
        };
        self.previous = Some(now);
        let cover = config.cover.map(|image| Arc::as_ptr(image) as usize);
        if !config.cover_pending && cover != self.cover {
            self.cover = cover;
            self.reel_change = if self.animate_reel && !config.paused { 0.0 } else { 1.0 };
            self.animate_reel = false;
        }
        self.reel_change = if config.paused { 1.0 } else { (self.reel_change + dt / 1.7).min(1.0) };
        self.time += dt;
        let calm = config.static_mode || config.reduced_motion;
        if !config.paused {
            let power = if config.reduced_motion { 0.0 } else { config.audio.power / 255.0 };
            let bass = if config.reduced_motion { 0.0 } else { config.audio.bass / 255.0 };
            self.power = ease(self.power, power, 5.0, dt);
            self.bass = ease(self.bass, bass, 7.0, dt);
        }
        self.smoke += dt * (1.0 + 1.4 * self.power);
        self.dust += dt * (1.0 + 1.8 * self.power);
        let intermission = self.intermission.step(
            dt,
            !config.paused,
            config.options.house_lights || config.queue_ended,
            calm,
            config.options.curtains_on_pause,
        );
        Frame {
            camera: Camera::new(selected_seat(config.options.viewpoint.as_deref()), &mut self.head, size),
            intermission,
            quality: config.options.quality,
            antialias: config.render_quality.antialias,
            time: self.time,
            dt,
            smoke_time: self.smoke,
            dust_time: self.dust,
            power: self.power,
            bass: self.bass,
            accent: config.theme.accent,
            sample_screen: !config.paused && !config.static_mode && intermission.shutter > 0.0,
            static_mode: config.static_mode,
            reel_change: self.reel_change,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Uniforms {
    pub eye_time: [f32; 4],
    pub right_tan: [f32; 4],
    pub up_tan: [f32; 4],
    pub forward_near: [f32; 4],
    pub render: [f32; 4],
    pub lamp: [f32; 4],
    pub atmosphere: [f32; 4],
    pub cove: [f32; 4],
    pub motion: [f32; 4],
    pub curtains: [f32; 4],
    pub post: [f32; 4],
    pub light_cover: [f32; 4],
    pub reel: [f32; 4],
    pub output: [f32; 4],
    pub reel_light_a: [[f32; 4]; 3],
    pub reel_light_b: [[f32; 4]; 3],
}

impl Uniforms {
    pub fn of(frame: Frame, size: [u32; 2], srgb: bool) -> Self {
        let camera = frame.camera;
        let d = frame.intermission.lights;
        let shutter = frame.intermission.shutter;
        let warm = [255.0, 214.0, 168.0];
        let accent = [frame.accent.r, frame.accent.g, frame.accent.b];
        let cove: [f32; 3] = std::array::from_fn(|index| {
            folia_render::to_linear_channel((warm[index] * 0.6 + f32::from(accent[index]) * 0.4) / 255.0)
        });
        let pack = |xyz: [f32; 3], w| [xyz[0], xyz[1], xyz[2], w];
        let focus = camera.eye[0].hypot(camera.eye[1] - SCREEN_Y).hypot(camera.eye[2]);
        let gap = curtain_gap(frame.intermission.left, frame.intermission.right);
        let third = super::layout::SCREEN_WIDTH / 6.0;
        let side = ((gap - third) / (2.0 * third)).clamp(0.0, 1.0);
        Self {
            eye_time: pack(camera.eye, frame.time),
            right_tan: pack(camera.right, camera.tan_horizontal),
            up_tan: pack(camera.up, camera.tan_vertical),
            forward_near: pack(camera.forward, 0.1),
            render: [size[0] as f32, size[1] as f32, 90.0, focus],
            lamp: [mix(0.085, 1.7, d), mix(1.05, 0.9, d), mix(0.7, 0.55, d), d],
            atmosphere: [1.0, shutter, frame.smoke_time, frame.dust_time],
            cove: [
                cove[0] * mix(0.32, 0.5, d),
                cove[1] * mix(0.32, 0.5, d),
                cove[2] * mix(0.32, 0.5, d),
                mix(0.78, 0.6, d),
            ],
            motion: [frame.power, frame.bass, 0.0, mix(0.35, 0.03, d) * (1.0 + 0.6 * frame.power) * shutter],
            curtains: [
                frame.intermission.left.top,
                frame.intermission.left.bottom,
                frame.intermission.right.top,
                frame.intermission.right.bottom,
            ],
            post: [mix(0.05, 0.022, d), mix(0.5, 0.22, d), mix(0.5, 0.15, d), 0.1],
            light_cover: [side, (gap / third).clamp(0.0, 1.0), side, mix(6.5, 1.1, d) * (1.0 + 0.08 * frame.bass)],
            reel: [1.0, 1.0, frame.reel_change, u8::from(frame.static_mode) as f32],
            output: [
                frame.dt,
                u8::from(frame.sample_screen) as f32,
                u8::from(srgb) as f32,
                u8::from(frame.antialias == folia_core::LyricAntialias::On) as f32,
            ],
            reel_light_a: [[0.0; 4]; 3],
            reel_light_b: [[0.0; 4]; 3],
        }
    }
}

fn mix(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn curtain_gap(left: CurtainPose, right: CurtainPose) -> f32 {
    let v: f32 = (SCREEN_Y - 0.95) / (9.8 - 0.95);
    let edge = |pose: CurtainPose| {
        let closure = (pose.top - (pose.top - pose.bottom) * (1.0 - v).powf(1.5)).min(1.0);
        let gather = (-((SCREEN_Y - 3.1) / 1.9).powi(2)).exp() * (1.0 - closure.max(0.0));
        let progress = ((SCREEN_Y - 3.1) / (9.7 - 3.1)).clamp(0.0, 1.0);
        let open = 7.12 + 0.62 * gather + 0.12 * progress * progress * (3.0 - 2.0 * progress);
        mix(open, -0.12, closure)
    };
    ((edge(left) + edge(right)) * 0.5).max(0.0)
}
