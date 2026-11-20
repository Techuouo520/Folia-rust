// Cached remote-card painting and lyric word coverage.

use crate::remote_view::{fade, remote_ink};
use crate::{Message, Shell, remote_lyrics, typeface};
use folia_store::RemoteBackground;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Size};
use std::cell::RefCell;

pub(crate) struct Face<'a>(pub &'a Shell);

#[derive(Default)]
pub(crate) struct TextLayout {
    text: String,
    words: Vec<folia_core::Word>,
    width: f32,
    size: f32,
    height: f32,
    large_lines: usize,
    segments: Vec<Segment>,
}

struct Segment {
    rect: Rectangle,
    start: f64,
    end: f64,
    before: f32,
    total: f32,
}

impl TextLayout {
    fn prepare(&mut self, line: &folia_core::Line, text: &str, width: f32) {
        if self.text == text && self.width == width && self.words == line.words {
            return;
        }
        self.text = text.to_owned();
        self.words.clone_from(&line.words);
        self.width = width;
        let make = |size| {
            folia_render::text_run::Run::new(
                text,
                folia_render::text_run::Style {
                    font: typeface::adapt(typeface::SEMIBOLD),
                    size,
                    line_height: size * 1.3,
                    tracking: 0.0,
                    vertical: false,
                    upright: false,
                    width: Some(width),
                },
            )
        };
        let mut run = make(20.0);
        self.large_lines = (run.size.height / 26.0).round().max(1.0) as usize;
        self.size = if run.size.height > 52.5 { 17.0 } else { 20.0 };
        if self.size == 17.0 {
            run = make(17.0);
        }
        self.height = run.size.height.min(self.size * 2.6);
        self.segments.clear();
        let mut byte = 0;
        for word in &line.words {
            let Some(relative) = text.get(byte..).and_then(|rest| rest.find(&word.text)) else {
                continue;
            };
            let from = byte + relative;
            byte = from + word.text.len();
            let first = text[..from].chars().count();
            let last = first + word.text.chars().count();
            let mut pieces: Vec<Rectangle> = Vec::new();
            for rect in run
                .clusters
                .get(first..last)
                .unwrap_or_default()
                .iter()
                .copied()
                .filter(|rect| rect.width > 0.0 && rect.y < self.height)
            {
                if let Some(previous) = pieces.last_mut()
                    && (previous.y - rect.y).abs() < 0.1
                {
                    let left = previous.x.min(rect.x);
                    previous.width = (previous.x + previous.width).max(rect.x + rect.width) - left;
                    previous.x = left;
                } else {
                    pieces.push(rect);
                }
            }
            let total = pieces.iter().map(|rect| rect.width).sum();
            let mut before = 0.0;
            for rect in pieces {
                self.segments.push(Segment { rect, start: word.start, end: word.end, before, total });
                before += rect.width;
            }
        }
    }
}

impl canvas::Program<Message> for Face<'_> {
    type State = RefCell<TextLayout>;
    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let shell = self.0;
        let remote = &shell.remote;
        let mode = remote.settings.appearance.background;
        let light = shell.appearance.theme == folia_store::ThemeMode::Light;
        let opacity = remote.opacity();
        let ink = fade(remote_ink(mode, light), opacity);
        let idle = remote.idle.value(remote.frame_time);
        let sleeve = if remote.settings.appearance.expand_cover { idle } else { 0.0 };
        let top = 4.0 + remote.expansion.filter(|e| e.up).map_or(0.0, |e| e.height);
        let base = remote.settings.size[1];
        let left = 178.0 - 174.0 * sleeve;
        let rect = Rectangle::new(Point::new(left, top), Size::new(bounds.width - left - 4.0, base - 8.0));
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let fill = match mode {
            RemoteBackground::Default if light => Color::from_rgb8(248, 247, 244),
            RemoteBackground::Default => Color::from_rgb8(20, 20, 24),
            RemoteBackground::Cover => Color::from_rgb8(13, 13, 16),
            RemoteBackground::Transparent => Color::from_rgba8(13, 13, 17, 0.8 * (1.0 - idle)),
        };
        let shape = canvas::Path::rounded_rectangle(rect.position(), rect.size(), 18.0.into());
        if mode == RemoteBackground::Default {
            let (top_color, bottom_color) = if light {
                (Color::from_rgb8(251, 250, 248), Color::from_rgb8(243, 242, 238))
            } else {
                (Color::from_rgb8(24, 24, 28), Color::from_rgb8(15, 15, 18))
            };
            let gradient = canvas::gradient::Linear::new(rect.position(), Point::new(rect.x, rect.y + rect.height))
                .add_stop(0.0, fade(top_color, opacity))
                .add_stop(1.0, fade(bottom_color, opacity));
            frame.fill(&shape, canvas::Fill { style: canvas::Style::Gradient(gradient.into()), ..Default::default() });
        } else {
            frame.fill(&shape, fade(fill, opacity));
        }
        if mode == RemoteBackground::Cover {
            let fade = remote.wash_fade.value(remote.frame_time);
            frame.with_clip(rect, |frame| {
                for (cover, share) in [(remote.wash_outgoing.as_ref(), 1.0 - fade), (remote.wash.as_ref(), fade)] {
                    if let Some(cover) = cover {
                        let mut image = canvas::Image::new(cover.clone()).opacity(share * opacity);
                        image.border_radius = 18.0.into();
                        frame.draw_image(
                            Rectangle::new(Point::new(4.0, top), Size::new(bounds.width - 8.0, base - 8.0)),
                            image,
                        );
                    }
                }
            });
        }
        if remote.sheet.is_none() {
            let incoming = remote.handoff.value(remote.frame_time);
            if let Some((track, _)) = &remote.outgoing {
                let has_line = shell.remote_line().0.is_some();
                let x = if has_line { 231.0 - 31.0 * sleeve } else { 272.0 - 72.0 * sleeve };
                let y = top + if has_line { 11.0 } else { 38.0 };
                let width = bounds.width - x - 26.0;
                label(
                    &mut frame,
                    &track.title,
                    Point::new(x, y),
                    Size::new(width, 28.0),
                    if has_line { 13.0 } else { 20.0 },
                    fade(ink, 1.0 - incoming),
                    !has_line,
                );
                if !has_line && let Some(artist) = &track.artist {
                    label(
                        &mut frame,
                        artist,
                        Point::new(x, top + 70.0),
                        Size::new(width, 19.0),
                        13.0,
                        fade(ink, 0.55 * (1.0 - incoming)),
                        false,
                    );
                }
            }
            self.song(&mut frame, &mut state.borrow_mut(), fade(ink, incoming), top, idle, sleeve);
        }
        if shell.playback.current().is_none() || remote.settings.click_through {
            let y = top + base - 58.0 + idle * 30.0;
            let width = bounds.width - 226.0;
            frame.fill_rectangle(Point::new(200.0, y), Size::new(width, 2.0), fade(ink, 0.16));
            let ratio = (shell.playback.position() / shell.playback.duration().max(0.001)).clamp(0.0, 1.0) as f32;
            frame.fill_rectangle(Point::new(200.0, y), Size::new(width * ratio, 2.0), fade(ink, 0.8));
        }
        vec![frame.into_geometry()]
    }
}

impl Face<'_> {
    fn song(&self, frame: &mut canvas::Frame, layout: &mut TextLayout, ink: Color, top: f32, idle: f32, sleeve: f32) {
        let shell = self.0;
        let (line, next, singing) = shell.remote_line();
        let track = shell.playback.current();
        let title = track.map_or_else(
            || shell.i18n.t("remote.noTrack").to_owned(),
            |track| if track.title.trim().is_empty() { track.display_name() } else { track.title.clone() },
        );
        let artist = track.and_then(|track| track.artist.as_deref()).unwrap_or(shell.i18n.t("remote.unknownArtist"));
        let x = if line.is_some() { 231.0 - 31.0 * sleeve } else { 272.0 - 72.0 * sleeve };
        let width = shell.remote.settings.size[0] - x - 26.0;
        if shell.remote.settings.appearance.background == RemoteBackground::Transparent
            && shell.remote.settings.appearance.text_shade
            && idle > 0.0
        {
            let rect =
                Rectangle::new(Point::new(190.0, top + 30.0), Size::new(shell.remote.settings.size[0] - 212.0, 100.0));
            frame.fill(
                &canvas::Path::rounded_rectangle(rect.position(), rect.size(), 20.0.into()),
                Color::from_rgba(0.0, 0.0, 0.0, idle * 0.6),
            );
        }
        if let Some(line) = line {
            let header = format!("{title} · {artist}");
            let header_width = shell.remote.settings.size[0] - 602.0
                + if shell.remote.settings.appearance.hide_titlebar {
                    345.0
                } else {
                    257.0 - 26.0 * shell.remote.lit.value(shell.remote.frame_time)
                };
            label(
                frame,
                &header,
                Point::new(x, top + 11.0),
                Size::new(header_width, 22.0),
                13.0,
                fade(ink, 0.7),
                false,
            );
            let rows = remote_lyrics::rows(
                &shell.remote.settings.appearance,
                shell.appearance.subtitle.content_mode,
                line,
                next,
            );
            let (opacity, rise) = shell.remote.lyric.pose(shell.remote.frame_time);
            let ink = fade(ink, opacity);
            let y = top + 40.0 + rise;
            let width = shell.remote.settings.size[0] - 222.0;
            layout.prepare(line, rows.main, width);
            let size = layout.size;
            let height = layout.height;
            let sung = if singing && shell.playback.is_playing() { 1.0 } else { 0.62 };
            label(
                frame,
                rows.main,
                Point::new(200.0, y),
                Size::new(width, height),
                size,
                fade(ink, if rows.timed && !line.words.is_empty() { 0.4 } else { sung }),
                true,
            );
            if rows.timed && !line.words.is_empty() {
                let time = shell.remote.lyric.time;
                for segment in &layout.segments {
                    let share = if segment.end > segment.start {
                        ((time - segment.start) / (segment.end - segment.start)).clamp(0.0, 1.0) as f32
                    } else {
                        f32::from(time >= segment.start)
                    };
                    let width = (segment.total * share - segment.before).clamp(0.0, segment.rect.width);
                    if width <= 0.0 {
                        continue;
                    }
                    let rect = Rectangle {
                        x: 200.0 + segment.rect.x,
                        y: y + segment.rect.y,
                        width,
                        height: segment.rect.height,
                    };
                    frame.with_clip(rect, |clip| {
                        clip.fill_text(canvas::Text {
                            content: rows.main.into(),
                            position: Point::new(200.0, y),
                            max_width: layout.width,
                            color: fade(ink, sung),
                            size: size.into(),
                            font: typeface::adapt(typeface::SEMIBOLD),
                            line_height: iced::widget::text::LineHeight::Relative(1.3),
                            ..typeface::canvas_defaults()
                        })
                    });
                }
            }
            let show_roman = rows.romanization.is_some() && (layout.large_lines <= 1 || rows.sub.is_none());
            let sub_lines = if rows.sub.is_none() {
                0
            } else if layout.large_lines <= 1 && !show_roman && shell.remote.settings.appearance.lyric_pairs == 1 {
                2
            } else {
                1
            };
            let mut under = height;
            if let Some(roman) = rows.romanization.filter(|_| show_roman) {
                label(
                    frame,
                    roman,
                    Point::new(200.0, y + under + 1.0),
                    Size::new(width, 15.0),
                    11.0,
                    fade(ink, 0.55),
                    false,
                );
                under += 16.0;
            }
            if let Some(sub) = rows.sub {
                label(
                    frame,
                    sub,
                    Point::new(200.0, y + under + 4.0),
                    Size::new(width, 19.0 * sub_lines as f32),
                    13.0,
                    fade(ink, 0.64),
                    false,
                );
                under += 4.0 + 19.0 * sub_lines as f32;
            }
            if let Some((main, sub)) = rows.upcoming
                && idle > 0.0
            {
                let room = 110.0 - under - 6.0;
                let sub = sub.filter(|_| room >= 36.0);
                if room >= 20.0 {
                    let next_y = top + 150.0 - 26.0 - if sub.is_some() { 16.0 } else { 0.0 } + rise;
                    right_label(
                        frame,
                        main,
                        Point::new(220.0, next_y),
                        Size::new(width - 20.0, 20.0),
                        15.0,
                        fade(ink, 0.5 * idle),
                    );
                    if let Some(sub) = sub {
                        right_label(
                            frame,
                            sub,
                            Point::new(220.0, next_y + 20.0),
                            Size::new(width - 20.0, 16.0),
                            12.0,
                            fade(ink, 0.4 * idle),
                        );
                    }
                }
            }
        } else {
            label(frame, &title, Point::new(x, top + 38.0), Size::new(width, 28.0), 20.0, ink, true);
            if track.is_some() {
                label(frame, artist, Point::new(x, top + 70.0), Size::new(width, 19.0), 13.0, fade(ink, 0.55), false);
            }
        }
    }
}

fn label(frame: &mut canvas::Frame, content: &str, at: Point, bounds: Size, size: f32, color: Color, bold: bool) {
    let Size { width, height } = bounds;
    if width <= 0.0 {
        return;
    }
    frame.with_clip(Rectangle::new(at, Size::new(width, height)), |frame| {
        if color.r > 0.9 && color.g > 0.9 && color.b > 0.9 {
            frame.fill_text(canvas::Text {
                content: content.into(),
                position: Point::new(at.x, at.y + 1.0),
                max_width: width,
                color: Color::from_rgba(0.0, 0.0, 0.0, (color.a * 2.0).min(0.45)),
                size: size.into(),
                font: typeface::adapt(if bold { typeface::SEMIBOLD } else { typeface::SYSTEM }),
                line_height: iced::widget::text::LineHeight::Relative(1.3),
                ..typeface::canvas_defaults()
            });
        }
        frame.fill_text(canvas::Text {
            content: content.into(),
            position: at,
            max_width: width,
            color,
            size: size.into(),
            font: typeface::adapt(if bold { typeface::SEMIBOLD } else { typeface::SYSTEM }),
            line_height: iced::widget::text::LineHeight::Relative(1.3),
            ..typeface::canvas_defaults()
        })
    });
}

fn right_label(frame: &mut canvas::Frame, content: &str, at: Point, bounds: Size, size: f32, color: Color) {
    let width =
        typeface::measure(content, size, typeface::adapt(typeface::SYSTEM), bounds.width).width.min(bounds.width);
    label(
        frame,
        content,
        Point::new(at.x + bounds.width - width, at.y),
        Size::new(width, bounds.height),
        size,
        color,
        false,
    );
}
