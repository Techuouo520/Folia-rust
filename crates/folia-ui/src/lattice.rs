// Queue posters and radio browsing share the wall renderer and the active playback queue.

use crate::typeface::text;
use crate::{focus_card, home, library, playback, wall};
use folia_core::{Queue, Theme, Track};
use folia_i18n::I18n;
use folia_omni::Omni;
use iced::widget::{column, container, stack};
use iced::{Element, Length, Size, Task};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum Message {
    Wall(wall::Message),
    Key(focus_card::Key),
    Card(focus_card::Action),
    Frame,
    Locate,
}

#[derive(Default)]
pub struct Lattice {
    wall: wall::Wall,
    tiles: Vec<wall::Tile>,
    snapshot: Option<(u64, Option<usize>, bool, bool)>,
    selected: usize,
    wrap: bool,
    drag: f32,
    clicked: Option<(usize, Instant)>,
}

impl Lattice {
    pub fn set_randomize_on_entry(&mut self, enabled: bool) {
        if self.wall.order_seed().is_some() != enabled {
            self.wall.begin_visit(enabled);
            self.snapshot = None;
        }
    }

    pub fn sync(
        &mut self,
        queue: &Queue<Track>,
        fm: bool,
        appearance: folia_store::WallAppearance,
        i18n: &I18n,
        omni: Option<&Omni>,
    ) -> Task<Message> {
        self.wall.appearance(folia_store::WallAppearance { look: folia_store::WallLook::Solid, ..appearance });
        let radio = fm || appearance.radio;
        self.wrap = !fm && queue.repeat == folia_core::Repeat::All;
        let hidden = radio && (fm || appearance.hide_upcoming);
        let key = (queue.revision(), queue.current_index(), radio, hidden);
        if self.snapshot == Some(key) {
            return Task::none();
        }
        let current = queue.current_index().unwrap_or(0);
        self.snapshot = Some(key);
        self.selected = current;
        self.clicked = None;
        self.tiles = queue
            .items()
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let mut tile = library::track_tile(track, None, i18n);
                if hidden && index > current {
                    tile.title = i18n.t("home.latticeRadioHidden").to_owned();
                    tile.subtitle.clear();
                    tile.cover = wall::CoverSource::None;
                }
                tile
            })
            .collect();
        let layout =
            self.wall.relayout(&self.tiles, if radio { wall::Shape::Radio } else { wall::Shape::Blocks }, omni);
        let focus = if radio {
            self.arrange(omni)
        } else {
            self.wall.center_on(current);
            self.wall.show_visible(&self.tiles, omni)
        };
        Task::batch([layout, focus]).map(Message::Wall)
    }

    fn range(&self) -> std::ops::Range<usize> {
        self.selected.saturating_sub(2)..self.selected.saturating_add(4).min(self.tiles.len())
    }

    fn arrange(&mut self, omni: Option<&Omni>) -> Task<wall::Message> {
        self.wall.radio(self.selected, self.range(), &self.tiles, omni)
    }

    pub fn pointer(&mut self, at: Option<iced::Point>, now: Instant) {
        self.wall.pointer(at, &self.tiles, now);
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.wall.coasting() || self.wall.focus.animating(now)
    }

    pub fn update(&mut self, message: Message, omni: Option<&Omni>) -> (Task<Message>, Option<crate::Message>) {
        let now = Instant::now();
        let (_, current, radio, _) = self.snapshot.unwrap_or_default();
        let mut effect = None;
        let play = |i| {
            crate::Message::Playback(if current == Some(i) {
                playback::Message::Toggle
            } else {
                playback::Message::Jump(i)
            })
        };
        let task = match message {
            Message::Frame => {
                if self.wall.tick_focus(now) {
                    self.wall.show_visible(&self.tiles, omni)
                } else {
                    self.wall.tick_coast(now, &self.tiles, omni)
                }
            }
            Message::Locate => {
                self.selected = current.unwrap_or(0);
                if radio {
                    self.arrange(omni)
                } else {
                    self.wall.center_on(self.selected);
                    self.wall.show_visible(&self.tiles, omni)
                }
            }
            Message::Wall(wall::Message::Pan(delta) | wall::Message::Wheel(delta)) if radio => {
                self.drag += if delta.x.abs() > delta.y.abs() { delta.x } else { delta.y };
                if self.drag.abs() < 32.0 {
                    return (Task::none(), None);
                }
                let forward = self.drag > 0.0;
                self.drag = 0.0;
                return self.step(forward, omni);
            }
            Message::Wall(wall::Message::Settled(_)) if radio => Task::none(),
            Message::Wall(message) => {
                let resized = matches!(message, wall::Message::Viewport(_));
                let (task, hit) = self.wall.update(message, &self.tiles, omni);
                if let Some(Some((index, slot))) = hit {
                    let double = self
                        .clicked
                        .is_some_and(|(last, at)| last == index && now.duration_since(at) < Duration::from_millis(400));
                    self.clicked = Some((index, now));
                    if double {
                        effect = Some(play(index));
                    }
                    self.selected = index;
                    if radio {
                        return (Task::batch([task, self.arrange(omni)]).map(Message::Wall), effect);
                    }
                    self.wall.focus_slot(Some(slot), false, 0.0);
                    self.wall.expand(slot, index, 80.0);
                    return (Task::batch([task, self.wall.show_visible(&self.tiles, omni)]).map(Message::Wall), effect);
                } else if hit == Some(None) {
                    self.wall.collapse();
                }
                if resized && radio { Task::batch([task, self.arrange(omni)]) } else { task }
            }
            Message::Card(action) => {
                effect = Some(match action {
                    focus_card::Action::Play => play(self.selected),
                    focus_card::Action::Enter => crate::Message::Home(home::Message::Player),
                    focus_card::Action::Enqueue => crate::Message::Playback(playback::Message::MoveEnd(self.selected)),
                    focus_card::Action::Catalog(message) => crate::Message::Catalog(message),
                });
                Task::none()
            }
            Message::Key(key) => {
                use focus_card::{Direction, Key};
                match key {
                    Key::Move(direction) if radio => {
                        return self.step(matches!(direction, Direction::Right | Direction::Down), omni);
                    }
                    Key::Move(direction) => {
                        let slot = self.wall.neighbour(direction, &self.tiles);
                        self.wall.focus_slot(slot, true, 80.0);
                    }
                    Key::Enter if radio && !self.tiles.is_empty() => effect = Some(play(self.selected)),
                    Key::Enter => {
                        if let Some((index, slot)) = self.wall.keyboard_tile() {
                            if self.wall.focus.expanded() == Some((slot, index)) {
                                effect = Some(play(index));
                            } else {
                                self.selected = index;
                                self.wall.expand(slot, index, 80.0);
                            }
                        }
                    }
                    Key::Escape if self.wall.focus.expanded().is_some() => self.wall.collapse(),
                    Key::Escape => effect = Some(crate::Message::Back),
                    Key::First | Key::Last if !radio => {
                        self.wall.center_on(if key == Key::First { 0 } else { self.tiles.len().saturating_sub(1) })
                    }
                    Key::Page(direction) if !radio => self.wall.page(direction),
                    _ => {}
                }
                self.wall.show_visible(&self.tiles, omni)
            }
        };
        (task.map(Message::Wall), effect)
    }

    fn step(&mut self, forward: bool, omni: Option<&Omni>) -> (Task<Message>, Option<crate::Message>) {
        let count = self.tiles.len();
        if count == 0 {
            return (Task::none(), None);
        }
        self.selected = match (forward, self.wrap) {
            (true, true) => (self.selected + 1) % count,
            (false, true) => (self.selected + count - 1) % count,
            (true, false) => (self.selected + 1).min(count - 1),
            (false, false) => self.selected.saturating_sub(1),
        };
        (self.arrange(omni).map(Message::Wall), None)
    }

    pub fn view<'a>(
        &'a self,
        i18n: &'a I18n,
        theme: &'a Theme,
        day: bool,
        player: &'a playback::Playback,
        size: Size,
    ) -> Element<'a, Message> {
        let current = player.current().map(|track| track.id);
        let now = Instant::now();
        let mut layers = stack![
            library::ground(theme, [0.0; 3]),
            iced::widget::shader(folia_render::Backdrop::new(self.wall.glass_scene(
                &self.tiles,
                theme,
                day,
                current,
                now
            )))
            .width(Length::Fill)
            .height(Length::Fill),
            self.wall.surface(&self.tiles, theme, day, current, None).map(Message::Wall)
        ];
        if self.tiles.is_empty() {
            layers = layers.push(
                container(
                    column![
                        text(i18n.t("home.latticeEmptyTitle")).size(32),
                        text(i18n.t("home.latticeEmptyText")).size(16)
                    ]
                    .spacing(10)
                    .align_x(iced::Alignment::Center),
                )
                .center(Length::Fill),
            );
        } else if let Some((index, rect, scale)) = self.wall.expanded_screen(now)
            && let Some(track) = player.queue.items().get(index)
        {
            let card = focus_card::Card {
                track,
                catalog: Default::default(),
                queued: true,
                current: Some(track.id) == current,
                playing: player.is_playing(),
                rect,
                scale,
                ink: folia_visualizer::color(theme.primary, 1.0),
                window_width: size.width,
            };
            layers = layers.push(focus_card::body(i18n, card, self.wall.focus.actions_reveal(now)).map(Message::Card));
        }
        layers.into()
    }
}
