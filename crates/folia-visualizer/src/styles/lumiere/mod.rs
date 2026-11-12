// Lumiere (绘光): a dark stage, volumetric shafts and a small window of lyrics lit by those shafts.

mod art;
mod audio;
mod beams;
pub mod bloom;
mod catalog;
mod decor;
mod field;
mod gpu;
mod more;
mod palette;
mod profiles;
mod program;
mod text;
mod tuning;

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;

use folia_lyrics::active_line;
use folia_render::glyphs::Batch;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::shader;
use iced::{Color, Point, Radians, Rectangle, Renderer, Size, Vector, mouse};

use crate::Scene;

#[cfg(feature = "dev")]
pub use gpu::{front_slots, stage_slots, stage_textures};
pub use program::{Program, Shot, Span};
pub use tuning::{Quality, Tuning, manifest};

use field::View;

/// Shader-widget state. The owner keeps this scene's GPU slot alive across other windows' presents.
#[derive(Default)]
pub struct LiveState {
    pub cache: RefCell<Cache>,
    owner: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Default)]
pub struct Cache {
    seed: u32,
    program: program::Program,
    decor: decor::Store,
    text: text::Text,
    glyphs: Batch,
    audio: audio::Sampler,
}

/// The graphics and text groups for one frame. Only the foreground bokeh and the frame are drawn above it.
pub struct Field<'a, Message> {
    scene: Scene<'a>,
    _message: PhantomData<Message>,
}

pub fn field_view<'a, Message>(scene: Scene<'a>) -> Field<'a, Message> {
    Field { scene, _message: PhantomData }
}

/// The dark-field sprite under the lights: theme background × 0.06 at the resolved alpha.
pub struct Veil {
    pub color: Color,
}

impl<Message> canvas::Program<Message> for Veil {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.color);
        vec![frame.into_geometry()]
    }
}

pub fn veil_color(theme: &folia_core::Theme, amount: f32) -> Color {
    let field = palette::dark_field(theme, amount);
    Color::from_rgba(field.rgb[0], field.rgb[1], field.rgb[2], field.alpha)
}

impl<Message> shader::Program<Message> for Field<'_, Message> {
    type State = LiveState;
    type Primitive = gpu::Pass;

    fn update(
        &self,
        state: &mut Self::State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        (!state.owner.load(std::sync::atomic::Ordering::Acquire)).then(|| {
            shader::Action::request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16))
        })
    }

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let mut cache = state.cache.borrow_mut();
        if !state.owner.load(std::sync::atomic::Ordering::Acquire) {
            cache.decor.invalidate();
        }
        let size = bounds.size();
        let tuning = Tuning::read(&self.scene.tuning);
        cache.sync(self.scene.lyrics, self.scene.seed, tuning.seamless);
        let heard = cache.audio.sample(audio::wall_ms(), self.scene.audio);
        let view = field::prepare(&cache.program, self.scene.time, size, self.scene.theme, tuning, heard);
        let mut pass = stage_pass(&mut cache, &self.scene, &view, tuning, heard);
        pass.slot = Arc::as_ptr(&state.owner) as u64;
        pass.owner = Arc::downgrade(&state.owner);
        pass.publish_blur = !self.scene.preview;
        pass
    }
}

/// Foreground bokeh, drawn above the lyrics.
pub struct Front<'a, Message> {
    scene: Scene<'a>,
    _message: PhantomData<Message>,
}

pub fn front_view<'a, Message>(scene: Scene<'a>) -> Front<'a, Message> {
    Front { scene, _message: PhantomData }
}

impl<Message> shader::Program<Message> for Front<'_, Message> {
    type State = LiveState;
    type Primitive = gpu::FrontPass;

    fn update(
        &self,
        state: &mut Self::State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        (!state.owner.load(std::sync::atomic::Ordering::Acquire)).then(|| {
            shader::Action::request_redraw_at(std::time::Instant::now() + std::time::Duration::from_millis(16))
        })
    }

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let mut cache = state.cache.borrow_mut();
        if !state.owner.load(std::sync::atomic::Ordering::Acquire) {
            cache.decor.invalidate();
        }
        let size = bounds.size();
        let tuning = Tuning::read(&self.scene.tuning);
        cache.sync(self.scene.lyrics, self.scene.seed, tuning.seamless);
        let heard = cache.audio.sample(audio::wall_ms(), self.scene.audio);
        let view = field::prepare(&cache.program, self.scene.time, size, self.scene.theme, tuning, heard);
        let particles = {
            let Cache { decor, program, .. } = &mut *cache;
            decor.front(program, &view, self.scene.time, tuning)
        };
        gpu::FrontPass {
            bytes: field::pack(&view),
            decor: decor::uniform(&cache.program, self.scene.time, tuning, mote_lift(heard, tuning)),
            particles,
            slot: Arc::as_ptr(&state.owner) as u64,
            owner: Arc::downgrade(&state.owner),
        }
    }
}

/// The coverage images the sprite pipeline is built with (light marks of the lyric window).
pub fn sprite_images() -> Vec<folia_render::glyphs::Image> {
    text::images()
}

fn mote_lift(heard: audio::Heard, tuning: Tuning) -> f32 {
    1.0 + 0.5 * heard.treble.min(1.0) * tuning.audio_response
}

fn light<'a>(view: &'a View, tuning: Tuning, intensity: f32) -> text::Light<'a> {
    text::Light {
        beams: &view.beams[..view.count as usize],
        light: view.palette.light,
        lit: view.palette.lit,
        unlit: view.palette.unlit,
        unlit_alpha: tuning.unlit_opacity,
        intensity,
        hide_trails: tuning.hide_trails,
        camera: &view.camera,
        height: view.height,
    }
}

fn stage_pass(cache: &mut Cache, scene: &Scene<'_>, view: &View, tuning: Tuning, heard: audio::Heard) -> gpu::Pass {
    let Cache { decor, program, seed, text, glyphs, .. } = cache;
    text.sync(program, scene, *seed, view, tuning);
    let light = light(view, tuning, text.window_intensity(scene.time));
    let echo = {
        let mut sprites = glyphs.lock();
        sprites.clear();
        text.paint_echo(scene.lyrics, scene.time, &light, &mut sprites);
        let echo = sprites.len() as u32;
        text.paint_window(scene.lyrics, scene.time, &light, &mut sprites);
        echo
    };
    decor.ensure(*seed, scene.lyrics, program);
    let bursts = decor.active(scene.time);
    let (lines, particles) = decor.rebuild(program, view, scene.time, tuning, &scene.theme.lyrics_icons);
    let mut decor_bytes = decor::uniform(program, scene.time, tuning, mote_lift(heard, tuning));
    for (slot, burst) in bursts.iter().enumerate() {
        let Some(burst) = burst else { continue };
        let Some([x, y, font_px]) = text.glyph_anchor(scene.lyrics, burst.line, burst.grapheme, scene.time) else {
            continue;
        };
        let age = scene.time as f32 - burst.time;
        let at = [x, y + font_px * burst.dy, font_px * burst.size];
        let tint = text.keyword(burst.line, burst.grapheme).map_or(burst.tint, |keyword| {
            let light = self::text::keyword_light(view.palette.light, keyword, 0.7);
            std::array::from_fn(|i| burst.tint[i] + (light[i] - burst.tint[i]) * 0.8)
        });
        decor::stamp(&mut decor_bytes, slot, at, age, tint, view.palette.light);
    }
    gpu::Pass {
        bytes: field::pack(view),
        decor: decor_bytes,
        lines,
        particles,
        split: decor.split(),
        kind: profiles::PROFILES[view.profile].kind,
        bloom: tuning.bloom,
        text_bloom: tuning.text_bloom,
        glyphs: glyphs.clone(),
        echo,
        text_only: tuning.text_only,
        base: view.base,
        veil: {
            let color = veil_color(scene.theme, tuning.dark_field);
            [color.r, color.g, color.b, color.a]
        },
        graphics_scale: tuning.graphics_scale(),
        slot: 0,
        owner: std::sync::Weak::new(),
        publish_blur: false,
    }
}

/// Draws the corner frame, the top layer (the original `overlayLayer` sits above the scenes and front bokeh).
/// A canvas with its own backdrop is a style picture with no shader under it, so it also sets the window in canvas text.
pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size) {
    let tuning = Tuning::read(&scene.tuning);
    let framed = tuning.overlay_frame && !tuning.text_only;
    if !framed && !scene.backdrop {
        return;
    }
    cache.sync(scene.lyrics, scene.seed, tuning.seamless);
    let heard = cache.audio.sample(audio::wall_ms(), scene.audio);
    let view = field::prepare(&cache.program, scene.time, size, scene.theme, tuning, heard);
    if scene.backdrop && active_line(&scene.lyrics.lines, scene.time).is_some() {
        let Cache { program, seed, text, .. } = cache;
        text.sync(program, scene, *seed, &view, tuning);
        let light = light(&view, tuning, text.window_intensity(scene.time));
        frame.with_save(|frame| {
            apply_camera(frame, &view.camera);
            text.each_glyph(scene.lyrics, scene.time, &light, |grapheme, at, rotation, font_px, color| {
                frame.with_save(|frame| {
                    frame.translate(Vector::new(at[0], at[1]));
                    frame.rotate(Radians(rotation));
                    frame.fill_text(Text {
                        content: grapheme.to_owned(),
                        position: Point::ORIGIN,
                        color: Color::from_rgba(color[0], color[1], color[2], color[3]),
                        size: font_px.into(),
                        font: scene.typography.font(text::LYRIC_FONT),
                        align_x: iced::widget::text::Alignment::Center,
                        align_y: iced::alignment::Vertical::Center,
                        ..Text::default()
                    });
                });
            });
        });
    }
    if framed {
        paint_overlay(frame, size, view.palette.light);
    }
}

impl Cache {
    fn sync(&mut self, lyrics: &folia_core::Lyrics, seed: &str, seamless: bool) {
        let lyric = program::song_seed(lyrics);
        let lyric_changed = self.seed != lyric;
        self.seed = lyric;
        let seamless_changed = self.program.seamless != (seamless && self.program.spans.len() >= 2);
        if lyric_changed || seamless_changed || self.program.shots.is_empty() || self.program.seed != seed {
            self.program = program::Program::compile(lyrics, seed, seamless);
        }
    }
}

fn paint_overlay(frame: &mut Frame, size: Size, light: [f32; 3]) {
    let color = Color::from_rgba(light[0], light[1], light[2], 0.5);
    let unit = size.width.min(size.height);
    let (pad_x, pad_y) = text::frame_insets(size.width, size.height);
    let arm = unit * 0.045;
    let stroke = Stroke::default().with_width((unit / 600.0).max(1.2)).with_color(color);
    let corners = [
        (pad_x, pad_y, 1.0, 1.0),
        (size.width - pad_x, pad_y, -1.0, 1.0),
        (size.width - pad_x, size.height - pad_y, -1.0, -1.0),
        (pad_x, size.height - pad_y, 1.0, -1.0),
    ];
    for (x, y, sx, sy) in corners {
        let path = Path::new(|builder| {
            builder.move_to(Point::new(x, y + sy * arm));
            builder.line_to(Point::new(x, y));
            builder.line_to(Point::new(x + sx * arm, y));
        });
        frame.stroke(&path, stroke);
    }
}

fn apply_camera(frame: &mut Frame, camera: &field::Camera) {
    frame.translate(Vector::new(camera.x, camera.y));
    frame.rotate(Radians(camera.rotation));
    frame.scale(camera.scale);
    frame.translate(Vector::new(-camera.pivot_x, -camera.pivot_y));
}
