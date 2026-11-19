// Radio cards reuse the wall's asynchronous cover cache and ordered canvas passes.

use super::*;
use crate::lattice::Message as LatticeMessage;
use crate::radio::{Card, Input, Role, State};
use crate::radio_backdrop::{alpha, line};

pub(crate) struct Departure {
    wall: Wall,
    state: State,
    tiles: Vec<Tile>,
    at: Instant,
}

impl Departure {
    pub fn project(&self, renderer: &mut Renderer, now: Instant) {
        if self.running(now) {
            let opacity = 1.0
                - folia_visualizer::Ease::new(0.25, 0.1, 0.25, 1.0)
                    .at(f64::from(now.saturating_duration_since(self.at).as_secs_f32() / 0.26))
                    as f32;
            self.state.projection.fade(renderer, opacity);
        }
    }
    pub fn running(&self, now: Instant) -> bool {
        !crate::radio::reduced() && now.saturating_duration_since(self.at).as_secs_f32() < 0.26
    }

    pub fn draw(&self, renderer: &Renderer, theme: &Theme, day: bool, size: Size, now: Instant) -> Vec<Geometry> {
        use canvas::Program as _;
        if !self.running(now) {
            return Vec::new();
        }
        let opacity = 1.0
            - folia_visualizer::Ease::new(0.25, 0.1, 0.25, 1.0)
                .at(f64::from(now.saturating_duration_since(self.at).as_secs_f32() / 0.26)) as f32;
        Program {
            wall: &self.wall,
            state: &self.state,
            tiles: &self.tiles,
            theme,
            day,
            word: &self.state.word,
            opacity,
            ground: false,
            flat: false,
            backdrop_only: self.state.projection.has_faces(),
            marks_only: false,
        }
        .draw(
            &PointerState::default(),
            renderer,
            &iced::Theme::Light,
            Rectangle::with_size(size),
            mouse::Cursor::Unavailable,
        )
    }
}

impl Wall {
    pub(crate) fn radio_artwork(&self, state: &mut State) -> Task<LatticeMessage> {
        state.art.retain(|id, _| state.cards.iter().chain(&state.outgoing).any(|c| c.id == *id));
        state.pending_art.retain(|id, _| state.cards.iter().any(|c| c.id == *id));
        let mut tasks = Vec::new();
        for card in &state.cards {
            let Some(handle) = self.cover_ref(card.id).and_then(Option::as_ref) else { continue };
            let key = handle.id();
            if state.art.get(&card.id).is_some_and(|art| art.source == key)
                || state.pending_art.get(&card.id) == Some(&key)
            {
                continue;
            }
            state.pending_art.insert(card.id, key);
            let id = card.id;
            let handle = handle.clone();
            tasks.push(Task::perform(blocking(move || crate::radio_art::prepare(handle)), move |art| {
                if art.is_none() {
                    tracing::warn!("radio cover worker stopped before returning its result");
                }
                LatticeMessage::RadioArt(id, key, art.flatten())
            }));
        }
        Task::batch(tasks)
    }
    pub(crate) fn take_radio_handoff(&mut self, state: State, tiles: Vec<Tile>) -> Handoff {
        let origin = state.metrics.rect(state.anchor - state.at(Instant::now())).bounds.center();
        let viewport = self.viewport;
        let mut wall = std::mem::take(self);
        self.viewport = viewport;
        self.order_seed = wall.order_seed;
        self.appearance = wall.appearance;
        self.lights = wall.lights;
        self.tint = wall.tint;
        self.next_cover_ticket = wall.next_cover_ticket;
        self.closed_ticket = wall.next_cover_ticket;
        wall.pending.clear();
        wall.cover_retry = None;
        wall.radio_departure = None;
        wall.shift = None;
        wall.next_shift = None;
        wall.ghosts.clear();
        Handoff {
            viewport,
            ghosts: Vec::new(),
            origin,
            radio: Some(Box::new(Departure { wall, state, tiles, at: Instant::now() })),
        }
    }
    pub(crate) fn radio_layout(&mut self, state: &State, tiles: &[Tile], omni: Option<&Omni>) -> Task<Message> {
        let scale = wall_scale(self.viewport.width).max(0.01);
        let view_center = Point::new(self.viewport.width / 2.0, self.viewport.height / 2.0);
        let mut places = Vec::with_capacity(state.cards.len());
        let title_size = (self.viewport.width * 0.02).clamp(22.0, 36.0);
        for card in &state.cards {
            let rect = state.metrics.rect((card.offset - state.browse) as f32).bounds;
            let world = Rectangle::new(
                Point::new((rect.x - view_center.x) / scale, (rect.y - view_center.y) / scale),
                Size::new(rect.width / scale, rect.height / scale),
            );
            places.push(Placement {
                rect: world,
                track: card.track,
                slot: SlotId { column: i64::from(card.offset), row: 0, index: 0, lot: false },
            });
            let Some(tile) = tiles.get(card.track) else { continue };
            let key = (card.track, rect.width as u32);
            self.labels.entry(key).or_insert_with(|| Label {
                title: folia_visualizer::clamp_lines(
                    &tile.title,
                    title_size,
                    TITLE_FONT,
                    (rect.width - 34.0).max(1.0),
                    3,
                ),
                artist: folia_visualizer::clamp_lines(
                    &tile.subtitle,
                    13.0,
                    ARTIST_FONT,
                    (rect.width - 34.0).max(1.0),
                    1,
                ),
                badge: None,
            });
        }
        self.layout = Layout::Finite(places);
        self.pan = Vector::ZERO;
        self.refresh();
        self.load_visible(tiles, omni)
    }

    pub(crate) fn radio_surface<'a>(
        &'a self,
        state: &'a State,
        tiles: &'a [Tile],
        theme: &'a Theme,
        day: bool,
        word: &'a str,
    ) -> iced::Element<'a, LatticeMessage> {
        iced::Element::new(crate::wall_canvas::WallCanvas::new(Program {
            wall: self,
            state,
            tiles,
            theme,
            day,
            word,
            opacity: 1.0,
            ground: false,
            flat: true,
            backdrop_only: false,
            marks_only: false,
        }))
    }

    pub(crate) fn radio_decoration<'a>(
        &'a self,
        state: &'a State,
        tiles: &'a [Tile],
        theme: &'a Theme,
        day: bool,
        word: &'a str,
        background: bool,
    ) -> iced::Element<'a, LatticeMessage> {
        iced::Element::new(crate::wall_canvas::WallCanvas::new(Program {
            wall: self,
            state,
            tiles,
            theme,
            day,
            word,
            opacity: 1.0,
            ground: background,
            flat: false,
            backdrop_only: background,
            marks_only: !background,
        }))
    }

    pub(crate) fn radio_projection(
        &self,
        state: &State,
        now: Instant,
    ) -> (Vec<folia_render::radio::Plane>, Vec<Handle>) {
        let mut planes = Vec::with_capacity(state.cards.len() + state.outgoing.len());
        let mut images = Vec::new();
        let mut add = |card: &Card, exit| {
            let rect = state.metrics.rect(card.position - state.at(now));
            if !rect.bounds.intersects(&Rectangle::with_size(state.metrics.size)) {
                return;
            }
            planes.push(plane(state, card, now, exit));
            let layers = self.cover_layers_at(card.id, now);
            for image in [layers.image, layers.previous, state.art.get(&card.id).map(|a| &a.veil)].into_iter().flatten()
            {
                if !images.iter().any(|added: &Handle| added.id() == image.id()) {
                    images.push(image.clone());
                }
            }
        };
        for c in &state.outgoing {
            add(c, true);
        }
        for distance in (0..=6).rev() {
            for c in &state.cards {
                if (c.position - state.at(now)).abs().floor() as i32 == distance {
                    add(c, false);
                }
            }
        }
        (planes, images)
    }

    pub(crate) fn radio_handoff_delay(&self) -> std::time::Duration {
        let Some(shift) = self.shift else { return std::time::Duration::ZERO };
        let now = Instant::now();
        let mut ms = 0;
        while ms < 570
            && self.ghosts.iter().any(|g| shift.leaving(g.rect, now + std::time::Duration::from_millis(ms)).is_some())
        {
            ms += 10;
        }
        std::time::Duration::from_millis(ms)
    }
}

#[derive(Default)]
pub(crate) struct PointerState {
    start: Option<Point>,
    last: Option<Point>,
    dragged: bool,
    ctrl: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Program<'a> {
    wall: &'a Wall,
    state: &'a State,
    tiles: &'a [Tile],
    theme: &'a Theme,
    day: bool,
    word: &'a str,
    opacity: f32,
    ground: bool,
    flat: bool,
    backdrop_only: bool,
    marks_only: bool,
}

impl crate::wall_canvas::Layered<LatticeMessage> for Program<'_> {}

impl canvas::Program<LatticeMessage> for Program<'_> {
    type State = PointerState;

    fn update(
        &self,
        pointer: &mut PointerState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<LatticeMessage>> {
        if self.backdrop_only || self.marks_only {
            return None;
        }
        let emit = |input| Some(Action::publish(LatticeMessage::Radio(input)).and_capture());
        match event {
            Event::Window(window::Event::RedrawRequested(_)) => (bounds.size() != self.wall.viewport)
                .then(|| Action::publish(LatticeMessage::Wall(Message::Viewport(bounds.size())))),
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                pointer.ctrl = m.control();
                None
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let point = cursor.position_in(bounds)?;
                pointer.start = Some(point);
                pointer.last = Some(point);
                pointer.dragged = false;
                emit(Input::Down(point))
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if pointer.start.is_some() => {
                let point = Point::new(position.x - bounds.x, position.y - bounds.y);
                pointer.last = Some(point);
                pointer.dragged |= pointer.start.is_some_and(|from| from.distance(point) >= 8.0);
                if pointer.dragged { emit(Input::Drag(point)) } else { None }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let start = pointer.start.take()?;
                let point = pointer.last.unwrap_or(start);
                if pointer.dragged {
                    return emit(Input::Up(point));
                }
                let now = Instant::now();
                let hit = self
                    .state
                    .cards
                    .iter()
                    .filter(|c| pose(self.state, c, now, false).alpha > 0.2)
                    .filter(|c| plane(self.state, c, now, false).contains(start))
                    .min_by(|a, b| {
                        (a.position - self.state.at(now)).abs().total_cmp(&(b.position - self.state.at(now)).abs())
                    });
                emit(hit.map_or(Input::Cancel, |card| Input::Click(card.offset, start)))
            }
            Event::Window(window::Event::Unfocused) => {
                pointer.start = None;
                emit(Input::Cancel)
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if !pointer.ctrl && pointer.start.is_none() => {
                cursor.position_in(bounds)?;
                let (x, y) = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => (x * 100.0, y * 100.0),
                    mouse::ScrollDelta::Pixels { x, y } => (x, y),
                };
                emit(Input::Wheel(-if x.abs() > y.abs() { x } else { y }))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _: &PointerState,
        renderer: &Renderer,
        _: &iced::Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let now = Instant::now();
        let mut background = Frame::new(renderer, bounds.size());
        if self.ground {
            background.fill_rectangle(Point::ORIGIN, bounds.size(), rgb(self.theme.background, 1.0));
        }
        if self.ground || !self.flat && !self.marks_only {
            crate::radio_backdrop::draw(&mut background, self.state, self.theme, self.word, now, self.opacity);
        }
        let mut layers = vec![background.into_geometry()];
        if self.ground
            && let Some(shift) = self.wall.shift.filter(|s| s.running(now))
        {
            let mut ghosts = Frame::new(renderer, bounds.size());
            let typeset = Typeset::for_width(bounds.width);
            let palette = Palette::new(self.theme, self.day, &self.wall.shades);
            self.wall.view(self.tiles, self.theme, self.day, None, None).draw_ghosts(
                &mut ghosts,
                shift,
                now,
                &typeset,
                &palette,
            );
            layers.push(ghosts.into_geometry());
        }
        if self.backdrop_only {
            return layers;
        }
        for card in &self.state.outgoing {
            self.draw_card(card, true, renderer, bounds.size(), now, &mut layers);
        }
        for distance in (0..=6).rev() {
            for card in &self.state.cards {
                if (card.position - self.state.at(now)).abs().floor() as i32 == distance {
                    self.draw_card(card, false, renderer, bounds.size(), now, &mut layers);
                }
            }
        }
        layers
    }

    fn mouse_interaction(
        &self,
        pointer: &PointerState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.backdrop_only || self.marks_only {
            return mouse::Interaction::None;
        }
        if pointer.dragged && pointer.start.is_some() {
            return mouse::Interaction::Grabbing;
        }
        if let Some(point) = cursor.position_in(bounds)
            && self
                .state
                .cards
                .iter()
                .any(|c| self.state.metrics.rect(c.position - self.state.at(Instant::now())).contains(point))
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl Program<'_> {
    fn draw_card(
        &self,
        card: &Card,
        exiting: bool,
        renderer: &Renderer,
        size: Size,
        now: Instant,
        layers: &mut Vec<Geometry>,
    ) {
        let rect = self.state.metrics.rect(card.position - self.state.at(now));
        let mut pose = pose(self.state, card, now, exiting);
        if self.flat {
            pose = Pose::REST;
        }
        pose.alpha *= self.opacity;
        if pose.alpha < 0.001 || pose.squeeze < 0.001 || !rect.bounds.intersects(&Rectangle::with_size(size)) {
            return;
        }
        let Some(tile) = self.tiles.get(card.track).filter(|t| t.id == card.id) else { return };
        let focused = card.offset == self.state.browse && !exiting;
        let expanded = card.role == Role::Now && focused;
        let hovered = !self.state.dragging() && self.state.pointer.is_some_and(|p| rect.contains(p));
        if self.marks_only {
            self.marks(card, rect, pose, renderer, size, layers);
            return;
        }
        let make_frame = || {
            let mut frame = Frame::new(renderer, size);
            frame.translate(rect.bounds.center() - Point::ORIGIN + Vector::new(0.0, pose.lift));
            if !self.flat {
                frame.rotate(rect.rotate.to_radians());
            }
            frame.scale_nonuniform(Vector::new(pose.scale * pose.squeeze, pose.scale));
            frame.translate(Vector::new(-rect.bounds.width / 2.0, -rect.bounds.height / 2.0));
            frame
        };
        let mut cover = make_frame();
        let mut shade = make_frame();
        let mut copy = make_frame();
        let local = Rectangle::with_size(rect.bounds.size());
        let ink = rgb(self.theme.primary, pose.alpha);
        let mut revealed = card.revealed;
        if !crate::radio::reduced() && (exiting || card.flip.is_some_and(|(_, hidden)| hidden || !card.revealed)) {
            revealed = flip_angle(card, now, exiting).cos() >= 0.0;
        }
        if revealed {
            let palette = Palette::new(self.theme, self.day, &self.wall.shades);
            let loaded = self.wall.cover_ref(card.id);
            let images = self.wall.cover_layers_at(card.id, now);
            cover.fill_rectangle(Point::ORIGIN, local.size(), alpha(palette.poster, pose.alpha));
            if let Some(image) = images.previous {
                cover.draw_image(local, canvas::Image::new(image.clone()).opacity(pose.alpha));
            }
            if let Some(image) = images.image {
                cover.draw_image(local, canvas::Image::new(image.clone()).opacity(pose.alpha * images.opacity));
            } else if shows_fallback(tile, loaded) {
                cover.fill_rectangle(
                    Point::ORIGIN,
                    local.size(),
                    gradient::Linear::new(Point::ORIGIN, Point::new(local.width, local.height))
                        .add_stop(0.0, alpha(hsl(tile.hue, 0.68, 0.58), pose.alpha))
                        .add_stop(1.0, alpha(hsl((tile.hue + 52.0) % 360.0, 0.62, 0.18), pose.alpha)),
                );
            }
            let base = if self.day { Color::WHITE } else { Color::BLACK };
            let p = if crate::radio::reduced() { card.expanded.goal() } else { card.expanded.value(now) };
            if p < 0.999 {
                shade.fill_rectangle(
                    Point::ORIGIN,
                    local.size(),
                    gradient::Linear::new(Point::ORIGIN, Point::new(0.0, local.height))
                        .add_stop(0.34, alpha(base, 0.05 * pose.alpha * (1.0 - p)))
                        .add_stop(0.56, alpha(base, 0.24 * pose.alpha * (1.0 - p)))
                        .add_stop(1.0, alpha(base, 0.92 * pose.alpha * (1.0 - p))),
                );
            }
            if p > 0.001 {
                shade.fill_rectangle(
                    Point::ORIGIN,
                    local.size(),
                    gradient::Linear::new(Point::ORIGIN, Point::new(local.width, 0.0))
                        .add_stop(0.0, alpha(base, 0.88 * p * pose.alpha))
                        .add_stop(0.52, alpha(base, 0.5 * p * pose.alpha))
                        .add_stop(1.0, alpha(base, 0.08 * p * pose.alpha)),
                );
                shade.fill_rectangle(
                    Point::ORIGIN,
                    local.size(),
                    gradient::Linear::new(Point::ORIGIN, Point::new(0.0, local.height))
                        .add_stop(0.54, Color::TRANSPARENT)
                        .add_stop(0.7, alpha(base, 0.26 * p * pose.alpha))
                        .add_stop(0.86, alpha(base, 0.62 * p * pose.alpha))
                        .add_stop(1.0, alpha(base, 0.86 * p * pose.alpha)),
                );
            }
            if expanded && (!self.state.resting(now) || self.state.entering(now) || !self.flat) {
                for (text, size, y) in [
                    (&tile.title, 22.0, if self.state.metrics.size.width < 640.0 { 56.0 } else { 74.0 }),
                    (&tile.subtitle, 13.0, if self.state.metrics.size.width < 640.0 { 84.4 } else { 102.4 }),
                ] {
                    copy.fill_text(Text {
                        content: text.clone(),
                        position: Point::new(if self.state.metrics.size.width < 640.0 { 20.0 } else { 32.0 }, y),
                        max_width: (local.width - if self.state.metrics.size.width < 640.0 { 92.0 } else { 132.0 })
                            .max(1.0),
                        size: size.into(),
                        color: ink,
                        font: crate::typeface::adapt(TITLE_FONT),
                        ..crate::typeface::canvas_defaults()
                    });
                }
            }
            if !expanded {
                self.draw_title(&mut copy, card, local, ink);
                let (a, b, dim) = self.wall.overlays(
                    self.theme,
                    self.day,
                    now,
                    if focused || hovered || card.role == Role::Now { 1.0 } else { 0.0 },
                );
                let (start, end) = tint_axis(local);
                shade.fill_rectangle(
                    Point::ORIGIN,
                    local.size(),
                    gradient::Linear::new(start, end)
                        .add_stop(0.0, alpha(a, pose.alpha))
                        .add_stop(1.0, alpha(b, pose.alpha)),
                );
                if dim.a > 0.001 {
                    copy.fill_rectangle(Point::ORIGIN, local.size(), alpha(dim, pose.alpha));
                }
            }
        } else {
            self.draw_back(&mut cover, &mut copy, tile, local, pose.alpha);
        }
        layers.extend([cover.into_geometry(), shade.into_geometry(), copy.into_geometry()]);
        if !self.flat {
            self.marks(card, rect, pose, renderer, size, layers);
        }
    }

    fn marks(
        &self,
        card: &Card,
        rect: crate::radio::Rect,
        pose: Pose,
        renderer: &Renderer,
        size: Size,
        layers: &mut Vec<Geometry>,
    ) {
        let local = Rectangle::with_size(rect.bounds.size());
        let ink = rgb(self.theme.primary, pose.alpha);
        let mut marks = Frame::new(renderer, size);
        marks.translate(rect.bounds.center() - Point::ORIGIN + Vector::new(0.0, pose.lift));
        marks.rotate(rect.rotate.to_radians());
        marks.scale(pose.scale);
        marks.translate(Vector::new(-rect.bounds.width / 2.0, -rect.bounds.height / 2.0));
        for (x, dx) in [(-8.0, 10.0), (local.width + 8.0, -10.0)] {
            for (y, dy) in [(-8.0, 10.0), (local.height + 8.0, -10.0)] {
                line(&mut marks, Point::new(x, y), Point::new(x + dx, y), alpha(ink, 0.4), 1.0);
                line(&mut marks, Point::new(x, y), Point::new(x, y + dy), alpha(ink, 0.4), 1.0);
            }
        }
        let drop = self.state.metrics.station_drop(rect);
        let opacity = (drop / 6.0).min(1.0);
        line(
            &mut marks,
            Point::new(local.width / 2.0, local.height),
            Point::new(local.width / 2.0, local.height + drop),
            alpha(ink, 0.3 * opacity),
            1.0,
        );
        marks.fill(
            &Path::circle(Point::new(local.width / 2.0, local.height + drop), 3.0),
            alpha(if card.role == Role::Now { rgb(self.theme.accent, pose.alpha) } else { alpha(ink, 0.62) }, opacity),
        );
        layers.push(marks.into_geometry());
    }

    fn draw_title(&self, frame: &mut Frame, card: &Card, local: Rectangle, ink: Color) {
        let rest = self.state.metrics.rect((card.offset - self.state.browse) as f32).bounds.width;
        let Some(label) = self.wall.labels.get(&(card.track, rest as u32)) else { return };
        let scale = local.width / rest;
        draw_copy(
            frame,
            local,
            Place { around: local, pose: Pose::REST },
            label,
            &Typeset {
                scale,
                title: (self.state.metrics.size.width * 0.02).clamp(22.0, 36.0) * scale,
                artist: 13.0 * scale,
                badge: 0.0,
                badge_box: Size::ZERO,
            },
            ink,
        );
    }

    fn draw_back(&self, frame: &mut Frame, copy: &mut Frame, tile: &Tile, rect: Rectangle, opacity: f32) {
        let tint = hsl(tile.hue, 0.52, 0.4);
        frame.fill_rectangle(
            Point::ORIGIN,
            rect.size(),
            gradient::Linear::new(Point::ORIGIN, Point::new(rect.width, rect.height))
                .add_stop(0.0, alpha(tint, opacity))
                .add_stop(1.0, alpha(Color { r: tint.r * 0.32, g: tint.g * 0.32, b: tint.b * 0.32, a: 1.0 }, opacity)),
        );
        if let Some(art) = self.state.art.get(&tile.id) {
            frame.draw_image(rect, canvas::Image::new(art.veil.clone()).opacity(opacity));
        }
        for i in 0..(rect.width / 14.0).ceil() as i32 {
            let x = i as f32 * 14.0;
            frame.fill_rectangle(Point::new(x, 0.0), Size::new(1.0, rect.height), alpha(Color::WHITE, 0.1 * opacity));
            frame.fill_rectangle(
                Point::new(x + 7.0, 0.0),
                Size::new(6.0_f32.min((rect.width - x - 7.0).max(0.0)), rect.height),
                alpha(Color::BLACK, 0.12 * opacity),
            );
        }
        frame.fill_rectangle(
            Point::ORIGIN,
            rect.size(),
            gradient::Linear::new(Point::ORIGIN, Point::new(0.0, rect.height))
                .add_stop(0.45, Color::TRANSPARENT)
                .add_stop(1.0, alpha(Color::BLACK, 0.45 * opacity)),
        );
        let (title, artist) = crate::radio::redaction(&tile.title, &tile.subtitle);
        copy.fill_rectangle(
            Point::new(rect.width * 0.085, rect.height * 0.764),
            Size::new(rect.width * title, rect.height * 0.08),
            alpha(Color::WHITE, 0.9 * opacity),
        );
        copy.fill_rectangle(
            Point::new(rect.width * 0.085, rect.height * 0.874),
            Size::new(rect.width * artist, rect.height * 0.036),
            alpha(Color::WHITE, 0.5 * opacity),
        );
    }
}

fn plane(state: &State, card: &Card, now: Instant, exit: bool) -> folia_render::radio::Plane {
    let rect = state.metrics.rect(card.position - state.at(now));
    let pose = pose(state, card, now, exit);
    let tilt = state.tilt(card, now);
    let colour = if crate::radio::reduced() { card.colour.goal() } else { card.colour.value(now) };
    let hovered = !state.dragging() && state.pointer.is_some_and(|p| rect.contains(p));
    let filter = if !card.revealed {
        if hovered { [0.0, 1.08, 1.06, 0.0] } else { [0.0, 1.0, 1.0, 0.0] }
    } else {
        [0.85 * (1.0 - colour), 0.8 + 0.2 * colour, 1.0, 0.0]
    };
    folia_render::radio::Plane {
        rect: [rect.bounds.x, rect.bounds.y, rect.bounds.width, rect.bounds.height],
        rotation: [tilt[0].to_radians(), tilt[1].to_radians(), rect.rotate.to_radians(), pose.scale * tilt[2]],
        effect: [pose.lift, pose.alpha, 900.0, flip_angle(card, now, exit)],
        color: filter,
    }
}

fn flip_angle(card: &Card, now: Instant, exiting: bool) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    if crate::radio::reduced() {
        return 0.0;
    }
    if exiting {
        return (if card.offset < 0 { -1.0 } else { 1.0 })
            * PI
            * crate::motion::CSS_TRANSITION
                .at(f64::from((now.saturating_duration_since(card.born).as_secs_f32() / 0.5).clamp(0.0, 1.0)))
                as f32;
    }
    let Some((at, hidden)) = card.flip else {
        return 0.0;
    };
    let t = now.saturating_duration_since(at).as_secs_f32();
    if hidden || !card.revealed {
        let p = crate::motion::CSS_TRANSITION.at(f64::from((t / 0.55).min(1.0))) as f32;
        return if hidden { (1.0 - p) * PI } else { p * PI };
    }
    if t < 0.26 {
        card.spin_direction
            * FRAC_PI_2
            * folia_visualizer::Ease::new(0.42, 0.0, 1.0, 1.0).at(f64::from(t / 0.26)) as f32
    } else if t < 0.56 {
        -card.spin_direction * FRAC_PI_2 * (1.0 - folia_visualizer::Ease::OUT.at(f64::from((t - 0.26) / 0.3)) as f32)
    } else {
        0.0
    }
}

fn pose(state: &State, card: &Card, now: Instant, exiting: bool) -> Pose {
    if crate::radio::reduced() {
        return Pose::REST;
    }
    if exiting {
        let t = (now.saturating_duration_since(card.born).as_secs_f32() / 0.5).clamp(0.0, 1.0);
        let eased = crate::motion::CSS_TRANSITION.at(t.into()) as f32;
        return Pose {
            squeeze: (eased * std::f32::consts::PI).cos().abs(),
            scale: 1.0 - 0.18 * eased,
            alpha: ((1.0 - t) / 0.4).min(1.0),
            lift: 0.0,
        };
    }
    let (at, delay) = card.opening.map_or((card.born, 0.0), |i| (state.opened + state.hold, i.min(5) as f32 * 0.08));
    let t = now.saturating_duration_since(at).as_secs_f32() - delay;
    let entered = folia_visualizer::spring(0.0, 1.0, t.into(), 240.0 / 0.9, 30.0 / 0.9) as f32;
    let mut pose = Pose { alpha: (t / 0.28).clamp(0.0, 1.0), ..Pose::REST };
    if card.opening.is_some() {
        pose.scale = 0.92 + 0.08 * entered;
        pose.lift = 48.0 * (1.0 - entered);
    }
    if now < at {
        pose.alpha = 0.0;
    }
    pose.squeeze = flip_angle(card, now, exiting).cos().abs();
    pose
}
