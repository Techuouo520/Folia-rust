// Queue posters and radio browsing share the wall renderer and the active playback queue.

use crate::typeface::text;
use crate::{focus_card, home, library, playback, wall};
use folia_core::Theme;
use folia_i18n::I18n;
use folia_omni::Omni;
use iced::widget::{column, container, pin, stack};
use iced::{Element, Length, Size, Task};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum Message {
    Wall(wall::Message),
    Key(focus_card::Key),
    Card(focus_card::Action),
    Frame,
    Locate,
    Playback(playback::Message),
    Timeline,
    Details,
    Radio(crate::radio::Input),
    RadioArt(i64, iced::advanced::image::Id, Option<crate::radio_art::Artwork>),
}

#[derive(Default)]
pub struct Lattice {
    wall: wall::Wall,
    tiles: Vec<wall::Tile>,
    snapshot: Option<(u64, Option<usize>, bool, bool)>,
    selected: usize,
    radio: Option<crate::radio::State>,
    clicked: Option<(focus_card::SlotId, iced::Point, Instant)>,
    playing: bool,
    current_id: Option<i64>,
    pending_focus: bool,
    details: Option<crate::motion::Tween>,
    handoff: Option<wall::Handoff>,
    opened: bool,
    keyboard_details: bool,
    pointer_at: Option<iced::Point>,
}

impl Lattice {
    pub(crate) fn enter(&mut self, handoff: wall::Handoff) {
        self.handoff = Some(handoff);
    }

    pub(crate) fn leave(&mut self) -> wall::Handoff {
        if let Some(state) = &self.radio {
            self.wall.take_radio_handoff(state.clone(), self.tiles.clone())
        } else {
            self.wall.take_handoff(&self.tiles)
        }
    }

    pub fn set_randomize_on_entry(&mut self, enabled: bool) {
        if self.wall.order_seed().is_some() != enabled {
            self.wall.begin_visit(enabled);
            self.snapshot = None;
        }
    }

    pub fn sync(
        &mut self,
        player: &playback::Playback,
        appearance: folia_store::WallAppearance,
        i18n: &I18n,
        omni: Option<&Omni>,
    ) -> Task<Message> {
        let queue = &player.queue;
        let fm = player.is_fm();
        self.playing = player.is_playing();
        self.wall.appearance(folia_store::WallAppearance { look: folia_store::WallLook::Solid, ..appearance });
        let radio = fm || appearance.radio;
        let wrap = !fm && queue.repeat == folia_core::Repeat::All;
        let hidden = radio && (fm || appearance.hide_upcoming);
        let key = (queue.revision(), queue.current_index(), radio, hidden);
        let previous = self.snapshot;
        let changed = previous.is_none_or(|before| before.0 != key.0 || before.2 != radio || before.3 != hidden);
        let current = queue.current_index().unwrap_or(0);
        self.snapshot = Some(key);
        let song = queue.current().map(|track| track.id);
        let follow = self.current_id != song;
        self.current_id = song;
        let mut tasks = Vec::new();
        if previous.is_some_and(|before| before.2 != radio) {
            self.handoff = Some(self.leave());
        }
        if radio && self.radio.is_none() {
            self.radio = Some(crate::radio::State::new(Instant::now()));
        } else if !radio {
            self.radio = None;
        }
        if changed {
            self.clicked = None;
            let selected = self.tiles.get(self.selected).map(|tile| tile.id);
            let grown = self
                .wall
                .expanded_screen(Instant::now())
                .and_then(|(index, rect, _)| self.tiles.get(index).map(|tile| (tile.id, rect)));
            self.tiles = queue
                .items()
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    let badge = (Some(index) == queue.current_index())
                        .then(|| format!("{} · {:02}", i18n.t("home.latticeBadgeNow"), index + 1));
                    library::track_tile(track, badge, i18n)
                })
                .collect();
            self.selected = selected.and_then(|id| self.tiles.iter().position(|tile| tile.id == id)).unwrap_or(current);
            tasks.push(self.wall.layout_tiles(
                &self.tiles,
                if radio { wall::Shape::Radio } else { wall::Shape::Blocks },
                omni,
                previous.is_some(),
            ));
            if let Some((id, rect)) = grown
                && let Some(index) = self.tiles.iter().position(|tile| tile.id == id)
                && let Some(slot) = self.wall.slot_of(index)
            {
                self.wall.restore_poster(slot, index, rect);
            }
        }
        if follow && !changed {
            for (index, tile) in self.tiles.iter_mut().enumerate() {
                tile.badge = (Some(index) == queue.current_index())
                    .then(|| format!("{} · {:02}", i18n.t("home.latticeBadgeNow"), index + 1));
            }
            self.wall.relabel(&self.tiles);
        }
        if (follow || previous.is_none()) && appearance.auto_focus {
            self.pending_focus = song.is_some();
        }
        if let Some(state) = &mut self.radio {
            state.fm = fm;
            let word = i18n.t(if fm { "home.latticeRadioBackdropWord" } else { "home.latticeRadioBackdropQueueWord" });
            if state.word != word {
                state.word = word.to_owned();
            }
            if follow || changed || state.wrap != wrap {
                state.sync(&self.tiles, queue.current_index(), wrap, hidden, Instant::now());
                self.selected = state.selected().unwrap_or(current);
                tasks.push(self.arrange(omni));
            }
        } else if self.pending_focus && self.wall.viewport().width > 0.0 {
            tasks.push(self.focus_current(previous.is_none(), omni));
        }
        if let Some(handoff) = self.handoff.take() {
            tasks.push(self.wall.update(wall::Message::Viewport(handoff.viewport), &self.tiles, omni).0);
            if radio {
                tasks.push(self.arrange(omni));
            } else if self.pending_focus {
                tasks.push(self.focus_current(true, omni));
            } else if !self.opened {
                self.wall.home_camera();
            }
            self.opened = true;
            self.wall.accept_handoff(handoff, &self.tiles);
            if let Some(state) = &mut self.radio {
                state.opened = Instant::now();
                state.hold = self.wall.radio_handoff_delay();
            }
        }
        let artwork = self.radio.as_mut().map_or_else(Task::none, |state| self.wall.radio_artwork(state));
        Task::batch([Task::batch(tasks).map(Message::Wall), artwork])
    }

    fn focus_current(&mut self, instant: bool, omni: Option<&Omni>) -> Task<wall::Message> {
        self.pending_focus = false;
        let Some(index) = self.current_id.and_then(|id| self.tiles.iter().position(|tile| tile.id == id)) else {
            return Task::none();
        };
        self.selected = index;
        if self.wall.slot_of(index).is_none() {
            self.wall.center_on(index);
        }
        if let Some(slot) = self.wall.slot_of(index) {
            self.wall.expand_centered(slot, index, instant);
        }
        self.wall.show_visible(&self.tiles, omni)
    }

    fn arrange(&mut self, omni: Option<&Omni>) -> Task<wall::Message> {
        let Some(state) = &mut self.radio else { return Task::none() };
        state.metrics = crate::radio::Metrics::new(self.wall.viewport());
        let changed = state.rebuild(&self.tiles, Instant::now());
        self.selected = state.selected().unwrap_or(0);
        if changed { self.wall.radio_layout(state, &self.tiles, omni) } else { Task::none() }
    }

    pub fn pointer(&mut self, at: Option<iced::Point>, now: Instant) {
        if let Some(state) = &mut self.radio {
            state.pointer(at, now);
            if self.pointer_at != at {
                self.keyboard_details = false;
            }
            self.pointer_at = at;
            let shown = state.current.is_some()
                && state.browse == 0
                && (self.keyboard_details || at.is_some_and(|p| state.metrics.rect(0.0).contains(p)));
            self.reveal_details(shown, now);
            return;
        }
        self.wall.pointer(at, &self.tiles, now);
        if self.pointer_at != at {
            self.keyboard_details = false;
        }
        self.pointer_at = at;
        let shown = self
            .wall
            .expanded_screen(now)
            .is_some_and(|(_, rect, _)| self.keyboard_details || at.is_some_and(|p| rect.contains(p)));
        self.reveal_details(shown, now);
    }

    fn reveal_details(&mut self, shown: bool, now: Instant) {
        if self.radio.is_some() && crate::radio::reduced() {
            self.details = Some(crate::motion::Tween::new(
                if shown { 1.0 } else { 0.0 },
                0.0,
                folia_visualizer::Ease::LINEAR,
                now,
            ));
            return;
        }
        let motion = self.details.get_or_insert_with(|| {
            crate::motion::Tween::new(0.0, 180.0, folia_visualizer::Ease::new(0.25, 0.1, 0.25, 1.0), now)
        });
        motion.set(if shown { 1.0 } else { 0.0 }, now);
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.radio.as_ref().is_some_and(|state| state.animating(now, self.playing))
            || self.wall.coasting()
            || self.wall.focus.animating(now)
            || self.details.as_ref().is_some_and(|motion| motion.running(now))
    }

    pub fn lyrics_visible(&self, player: &playback::Playback) -> bool {
        if let Some(state) = &self.radio {
            return state.current.is_some()
                && state.browse == 0
                && state.resting(Instant::now())
                && player.lyrics.is_some();
        }
        player.lyrics.as_ref().is_some_and(|lyrics| !lyrics.lines.is_empty())
            && self.wall.expanded_screen(Instant::now()).is_some_and(|(index, rect, _)| {
                self.tiles.get(index).is_some_and(|tile| Some(tile.id) == self.current_id)
                    && rect.intersects(&iced::Rectangle::with_size(self.wall.viewport()))
            })
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
            Message::RadioArt(id, source, art) => {
                if let Some(state) = &mut self.radio
                    && state.pending_art.get(&id) == Some(&source)
                {
                    state.pending_art.remove(&id);
                    if let Some(art) = art {
                        state.art.insert(id, art);
                    }
                }
                Task::none()
            }
            Message::Radio(input) => return self.radio_input(input, now, omni),
            Message::Details if radio => {
                return (Task::none(), Some(crate::Message::Playback(playback::Message::Toggle)));
            }
            Message::Details => {
                if let Some((index, slot)) = self.wall.keyboard_tile() {
                    if self.wall.focus.expanded() == Some((slot, index)) {
                        self.keyboard_details = !self.keyboard_details;
                        self.reveal_details(self.keyboard_details, now);
                    } else {
                        self.selected = index;
                        self.wall.expand_centered(slot, index, false);
                    }
                }
                Task::none()
            }
            Message::Playback(message) => return (Task::none(), Some(crate::Message::Playback(message))),
            Message::Timeline => {
                return (Task::none(), Some(crate::Message::Bar(crate::player_bar::Message::Timeline)));
            }
            Message::Frame => {
                if self.radio.is_some() {
                    let clock = self.wall.tick_coast(now, &self.tiles, omni);
                    let layout = self.arrange(omni);
                    let art = self.radio.as_mut().map_or_else(Task::none, |state| self.wall.radio_artwork(state));
                    return (Task::batch([Task::batch([clock, layout]).map(Message::Wall), art]), None);
                }
                if self.wall.tick_focus(now) {
                    self.wall.show_visible(&self.tiles, omni)
                } else {
                    self.wall.tick_coast(now, &self.tiles, omni)
                }
            }
            Message::Locate => {
                self.selected = current.unwrap_or(0);
                if let Some(state) = &mut self.radio {
                    state.browse_to(0, now);
                }
                if radio { self.arrange(omni) } else { self.focus_current(false, omni) }
            }
            Message::Wall(wall::Message::Pan(delta) | wall::Message::Wheel(delta)) if radio => {
                return self.radio_input(
                    crate::radio::Input::Wheel(if delta.x.abs() > delta.y.abs() { delta.x } else { delta.y }),
                    now,
                    omni,
                );
            }
            Message::Wall(wall::Message::Settled(_)) if radio => Task::none(),
            Message::Wall(message) => {
                let image_changed =
                    matches!(message, wall::Message::CoverLoaded { .. } | wall::Message::CoverAllocated { .. });
                let resized = matches!(message, wall::Message::Viewport(_));
                let point = match &message {
                    wall::Message::Press(_, point) => *point,
                    _ => iced::Point::ORIGIN,
                };
                let (task, hit) = self.wall.update(message, &self.tiles, omni);
                if image_changed && let Some(state) = &mut self.radio {
                    return (Task::batch([task.map(Message::Wall), self.wall.radio_artwork(state)]), None);
                }
                if let Some(Some((index, slot))) = hit {
                    if self.clicked.is_some_and(|(last, at, when)| {
                        last != slot
                            && at.distance(point) < 5.0
                            && now.duration_since(when) < Duration::from_millis(400)
                    }) {
                        return (task.map(Message::Wall), None);
                    }
                    let double = self.clicked.is_some_and(|(last, _, at)| {
                        last == slot && now.duration_since(at) < Duration::from_millis(400)
                    });
                    self.clicked = Some((slot, point, now));
                    if double && (current != Some(index) || !self.playing) {
                        effect = Some(play(index));
                    }
                    self.selected = index;
                    if radio {
                        if let Some(state) = &mut self.radio {
                            state.browse_to(index as i32 - current.map_or(-1, |i| i as i32), now);
                        }
                        return (Task::batch([task, self.arrange(omni)]).map(Message::Wall), effect);
                    }
                    if self.wall.focus.expanded() != Some((slot, index)) {
                        self.wall.expand_centered(slot, index, false);
                    }
                    return (Task::batch([task, self.wall.show_visible(&self.tiles, omni)]).map(Message::Wall), effect);
                } else if hit == Some(None) {
                    self.wall.collapse();
                }
                let first = resized && !self.opened;
                self.opened |= resized;
                if resized && radio {
                    Task::batch([task, self.arrange(omni)])
                } else if resized && self.pending_focus {
                    Task::batch([task, self.focus_current(true, omni)])
                } else if first {
                    self.wall.home_camera();
                    Task::batch([task, self.wall.show_visible(&self.tiles, omni)])
                } else {
                    task
                }
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
                self.keyboard_details = key != focus_card::Key::Escape;
                self.reveal_details(true, now);
                use focus_card::{Direction, Key};
                match key {
                    Key::Move(direction) if radio => {
                        if matches!(direction, Direction::Left | Direction::Right) {
                            return self.step(direction == Direction::Right, omni);
                        }
                    }
                    Key::Move(direction) => {
                        let slot = self.wall.neighbour(direction, &self.tiles);
                        self.wall.focus_slot(slot, true, 80.0);
                    }
                    Key::Enter if radio => effect = self.radio.as_ref().and_then(|state| state.selected()).map(play),
                    Key::Escape if radio => {
                        if let Some(state) = &mut self.radio {
                            if state.browse != 0 {
                                state.browse_to(0, now);
                                return (self.arrange(omni).map(Message::Wall), None);
                            }
                            effect = Some(crate::Message::Back);
                        }
                    }
                    Key::Enter => {
                        if let Some((index, slot)) = self.wall.keyboard_tile() {
                            if self.wall.focus.expanded() == Some((slot, index)) {
                                effect = Some(play(index));
                            } else {
                                self.selected = index;
                                self.wall.expand_centered(slot, index, false);
                            }
                        }
                    }
                    Key::Escape if self.wall.focus.expanded().is_some() => self.wall.collapse(),
                    Key::Escape => effect = Some(crate::Message::Back),
                    Key::First | Key::Last if !radio => {
                        let index = if key == Key::First { 0 } else { self.tiles.len().saturating_sub(1) };
                        self.wall.center_on(index);
                        self.wall.focus_slot(self.wall.slot_of(index), false, 0.0);
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
        if let Some(state) = &mut self.radio {
            state.browse_to(state.browse + if forward { 1 } else { -1 }, Instant::now());
        }
        (self.arrange(omni).map(Message::Wall), None)
    }

    /// Feeds radio decorations from the host's existing limited playback frame clock.
    pub fn sample_audio(&mut self, audio: &folia_core::AudioFrame, now: Instant) {
        if let Some(state) = &mut self.radio {
            state.sample_audio(audio, self.playing, now);
        }
    }

    pub(crate) fn radio_capsule(&self) -> Option<bool> {
        self.radio.as_ref().map(|state| state.current.is_none() || state.browse != 0)
    }

    pub(crate) fn release_radio(&mut self, point: Option<iced::Point>, omni: Option<&Omni>) -> Task<Message> {
        let Some(state) = &mut self.radio else {
            return Task::none();
        };
        if !state.dragging() {
            return Task::none();
        }
        state.release(point, Instant::now());
        self.arrange(omni).map(Message::Wall)
    }

    fn radio_input(
        &mut self,
        input: crate::radio::Input,
        now: Instant,
        omni: Option<&Omni>,
    ) -> (Task<Message>, Option<crate::Message>) {
        use crate::radio::Input;
        let Some(state) = &mut self.radio else { return (Task::none(), None) };
        let mut effect = None;
        match input {
            Input::Down(point) => state.down(point, now),
            Input::Drag(point) => {
                self.clicked = None;
                state.drag(point, now);
            }
            Input::Up(point) => state.release(Some(point), now),
            Input::Cancel => state.release(None, now),
            Input::Wheel(delta) => state.wheel(delta, now),
            Input::Click(offset, point) => {
                state.release(None, now);
                let slot = focus_card::SlotId { column: i64::from(offset), row: 0, index: 0, lot: false };
                let recent = self.clicked.filter(|(_, at, when)| {
                    at.distance(point) < 5.0 && now.saturating_duration_since(*when) < Duration::from_millis(400)
                });
                if recent.is_some_and(|(last, _, _)| last != slot) {
                    return (Task::none(), None);
                }
                if offset != state.browse {
                    self.clicked = Some((focus_card::SlotId { column: i64::MIN, ..slot }, point, now));
                    state.browse_to(offset, now);
                } else {
                    if recent.is_some() && (state.current != state.selected() || !self.playing) {
                        effect = state.selected().map(|i| {
                            crate::Message::Playback(if state.current == Some(i) {
                                playback::Message::Toggle
                            } else {
                                playback::Message::Jump(i)
                            })
                        });
                    }
                    self.clicked = Some((slot, point, now));
                }
            }
        }
        (self.arrange(omni).map(Message::Wall), effect)
    }

    pub fn view<'a>(
        &'a self,
        i18n: &'a I18n,
        theme: &'a Theme,
        appearance: &'a folia_store::AppearanceSettings,
        player: &'a playback::Playback,
        size: Size,
    ) -> Element<'a, Message> {
        if let Some(state) = &self.radio {
            return self.radio_view(state, i18n, theme, appearance, player, size);
        }
        let current = player.current().map(|track| track.id);
        let day = appearance.theme == folia_store::ThemeMode::Light;
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
            && self.wall.poster_ready(rect, track.id, now)
        {
            let card = crate::lattice_controls::Poster {
                track,
                index,
                player,
                rect,
                scale,
                ink: folia_visualizer::color(theme.primary, 1.0),
                width: size.width,
                details: self.details.as_ref().map_or(0.0, |motion| motion.value(now)),
                radio: false,
            };
            let lyrics = (Some(track.id) == current && self.wall.focus.expansion_settled(now))
                .then(|| self.lyrics(player, appearance, theme, rect, scale, size.width))
                .flatten();
            layers = layers.push(crate::lattice_controls::view(card, lyrics, self.wall.focus.actions_reveal(now)));
        }
        layers = layers.push(library::veil(day, appearance.wall.vignette));
        layers.into()
    }

    fn radio_view<'a>(
        &'a self,
        state: &'a crate::radio::State,
        i18n: &'a I18n,
        theme: &'a Theme,
        appearance: &'a folia_store::AppearanceSettings,
        player: &'a playback::Playback,
        size: Size,
    ) -> Element<'a, Message> {
        use iced::widget::button;
        let now = Instant::now();
        let day = appearance.theme == folia_store::ThemeMode::Light;
        let word =
            i18n.t(if state.fm { "home.latticeRadioBackdropWord" } else { "home.latticeRadioBackdropQueueWord" });
        let mut layers = stack![self.wall.radio_surface(state, &self.tiles, theme, day, word)];
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
        }
        if state.resting(now)
            && !state.entering(now)
            && let Some(index) = state.selected()
            && let Some(track) = player.queue.items().get(index)
        {
            let rect = state.metrics.rect(0.0).bounds;
            let ink = folia_visualizer::color(theme.primary, 1.0);
            if state.current == Some(index) && state.browse == 0 {
                let card = crate::lattice_controls::Poster {
                    track,
                    index,
                    player,
                    rect,
                    scale: 1.0,
                    ink,
                    width: size.width,
                    details: self.details.as_ref().map_or(0.0, |m| m.value(now)),
                    radio: true,
                };
                let lyrics = self.lyrics(player, appearance, theme, rect, 1.0, size.width);
                layers = layers.push(crate::lattice_controls::view(card, lyrics, (1.0, 0.0)));
            } else if state.cards.iter().any(|card| card.offset == state.browse && card.revealed) {
                let side = (rect.width * 0.12).clamp(36.0, 64.0);
                let paper = folia_visualizer::color(theme.background, 1.0);
                let play = button(
                    container(crate::icons::variant(crate::icons::Variant::FilledPlay, side * 0.42, paper))
                        .center(Length::Fixed(side)),
                )
                .padding(0)
                .on_press(Message::Playback(playback::Message::Jump(index)))
                .style(move |_, _| button::Style {
                    background: Some(ink.into()),
                    border: iced::Border { radius: (side / 2.0).into(), ..Default::default() },
                    ..Default::default()
                });
                layers =
                    layers.push(pin(play).position(iced::Point::new(
                        rect.x + rect.width * 0.94 - side,
                        rect.y + rect.height * 0.94 - side,
                    )));
            }
        }
        let accent = folia_visualizer::color(theme.accent, 1.0);
        let locate = button(
            container(crate::icons::icon(lucide_static_svg::Icon::Crosshair, 18.0, accent)).center(Length::Fixed(40.0)),
        )
        .padding(0)
        .on_press_maybe(state.current.map(|_| Message::Locate))
        .style(move |_, status| button::Style {
            background: Some(
                iced::Color {
                    a: if matches!(status, button::Status::Hovered) { 0.95 } else { 0.8 },
                    ..if day { iced::Color::WHITE } else { iced::Color::BLACK }
                }
                .into(),
            ),
            border: iced::Border { radius: 20.0.into(), ..Default::default() },
            ..Default::default()
        });
        let (planes, images) = self.wall.radio_projection(state, now);
        let faces = if self.tiles.is_empty() {
            layers.into()
        } else {
            folia_render::radio::surface(layers.into(), &state.projection, planes, images)
        };
        stack![
            self.wall.radio_decoration(state, &self.tiles, theme, day, word, true),
            faces,
            self.wall.radio_decoration(state, &self.tiles, theme, day, word, false)
        ]
        .push(library::veil(day, appearance.wall.vignette))
        .push(pin(locate).position(iced::Point::new(size.width - 68.0, size.height - 124.0)))
        .into()
    }

    fn lyrics<'a>(
        &self,
        player: &'a playback::Playback,
        appearance: &'a folia_store::AppearanceSettings,
        theme: &'a Theme,
        rect: iced::Rectangle,
        scale: f32,
        width: f32,
    ) -> Option<Element<'a, Message>> {
        let lyrics = player.lyrics.as_ref().filter(|lyrics| !lyrics.lines.is_empty())?;
        let (top, bottom) = if self.radio.is_some() && width < 640.0 {
            (104.0, 96.0)
        } else if width < 640.0 {
            (160.0, 164.0)
        } else {
            (128.0, 110.0)
        };
        let mut typography = crate::fonts::typography(appearance);
        typography.subtitle_scale = 1.0;
        let scene = folia_visualizer::Scene {
            lyrics,
            theme,
            time: player.lyric_time(),
            typography,
            render_quality: Default::default(),
            static_mode: crate::motion::reduced(folia_store::MotionSurface::CollectionMorph),
            paused: !player.is_playing(),
            daylight: appearance.theme == folia_store::ThemeMode::Light,
            style: folia_visualizer::Style::Monet,
            smart_beat: Default::default(),
            font_scale: 1.0,
            seed: "",
            stage: None,
            tuning: Default::default(),
            backdrop: false,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            portrait: None,
            preview: false,
            audio: &folia_core::AudioFrame::SILENT,
        };
        let mut view = stack![folia_visualizer::styles::monet::lyric_card(scene, scale)];
        if appearance.subtitle.show_harmony {
            view = view.push(crate::harmony::view(
                lyrics,
                player.lyric_time(),
                theme,
                &appearance.subtitle,
                [typography.font(iced::Font::DEFAULT), typography.subtitle_font(iced::Font::DEFAULT)],
                1.0,
            ));
        }
        let inset = if self.radio.is_some() && width < 640.0 { 16.0 } else { 24.0 };
        Some(
            pin(container(view)
                .width((rect.width - inset * 2.0 * scale).max(1.0))
                .height((rect.height - (top + bottom) * scale).max(1.0))
                .clip(true))
            .position(iced::Point::new(inset * scale, top * scale))
            .into(),
        )
    }
}
