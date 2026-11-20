// Compact remote card, real transport controls, queue and main-window tools.

use folia_core::Repeat;
use folia_store::RemoteBackground;
use iced::widget::{button, canvas, column, container, image, mouse_area, row, slider, space, stack, tooltip};
use iced::{Alignment, Border, Color, Element, Length, Padding};
use lucide_static_svg::Icon;

use crate::remote::{Chrome, Message as Remote, QueueAction, Tab};
use crate::remote_face::Face;
use crate::{Message, PlaybackMessage, Shell, icons, typeface};

impl Shell {
    pub(crate) fn remote_view(&self) -> Element<'_, Message> {
        let state = &self.remote;
        let width = state.settings.size[0];
        let base = state.settings.size[1];
        let face = width - 182.0;
        let top = 4.0 + state.expansion.filter(|value| value.up).map_or(0.0, |value| value.height);
        let opacity = state.opacity();
        let ink = fade(
            remote_ink(state.settings.appearance.background, self.appearance.theme == folia_store::ThemeMode::Light),
            opacity,
        );
        let idle = state.idle.value(state.frame_time);
        let lit = state.lit.value(state.frame_time);
        let mut layers = stack![
            mouse_area(canvas(Face(self)).width(Length::Fill).height(Length::Fill))
                .on_press(Message::Remote(Remote::Drag))
        ]
        .width(Length::Fill)
        .height(Length::Fill);
        if state.sheet.is_none() {
            let (line, _, _) = self.remote_line();
            let sleeve = if state.settings.appearance.expand_cover { idle } else { 0.0 };
            let small = if line.is_some() { 22.0 } else { 56.0 };
            let side = small + (176.0 - small) * sleeve;
            let x = 200.0 - 196.0 * sleeve;
            let y = top + if line.is_some() { 11.0 } else { 35.0 } * (1.0 - sleeve) + (base - 184.0) * 0.5 * sleeve;
            let cover = self.playback.current().and_then(|track| self.art.peek(&track.id)).and_then(Option::as_ref);
            if let Some((_, Some(cover))) = state.outgoing.as_ref() {
                let old = image(cover.clone())
                    .width(side)
                    .height(side)
                    .content_fit(iced::ContentFit::Cover)
                    .border_radius(12.0)
                    .opacity((1.0 - state.handoff.value(state.frame_time)) * opacity);
                layers = layers.push(place(old.into(), x, y, side, side));
            }
            if let Some(cover) = cover {
                let cover = image(cover.clone())
                    .width(side)
                    .height(side)
                    .content_fit(iced::ContentFit::Cover)
                    .border_radius(12.0)
                    .opacity(state.handoff.value(state.frame_time) * opacity);
                layers = layers.push(place(cover.into(), x, y, side, side));
            } else {
                let placeholder =
                    container(icons::icon(Icon::Disc3, side * 0.45, fade(ink, 0.35))).center(side).style(move |_| {
                        container::Style {
                            background: Some(fade(ink, 0.08).into()),
                            border: Border { radius: 12.0.into(), ..Default::default() },
                            ..Default::default()
                        }
                    });
                layers = layers.push(place(placeholder.into(), x, y, side, side));
            }
        }
        if !state.settings.click_through {
            if !state.settings.appearance.hide_titlebar && lit > 0.01 {
                let tools = row![
                    self.remote_key(
                        Icon::Settings,
                        "remote.openSettings",
                        Remote::OpenSettings,
                        ink,
                        (24.0, lit * 0.6),
                        true
                    ),
                    self.remote_key(
                        Icon::Palette,
                        match state.settings.appearance.background {
                            RemoteBackground::Default => "remote.backgroundDefault",
                            RemoteBackground::Cover => "remote.backgroundCover",
                            RemoteBackground::Transparent => "remote.backgroundTransparent",
                        },
                        Remote::Background,
                        ink,
                        (24.0, lit * 0.6),
                        true
                    ),
                    self.remote_key(
                        if state.capabilities.pin && state.settings.always_on_top { Icon::Pin } else { Icon::PinOff },
                        if state.capabilities.pin { "remote.pinToFront" } else { "remote.pinUnavailable" },
                        Remote::Pin,
                        ink,
                        (24.0, lit * 0.6),
                        state.capabilities.pin
                    ),
                    self.remote_key(Icon::X, "remote.close", Remote::Close, ink, (24.0, lit * 0.6), true),
                ]
                .spacing(2);
                layers = layers.push(place(tools.into(), width - 122.0, top + 10.0, 102.0, 24.0));
            }
            let progress = slider(0.0..=self.playback.duration().max(0.001), self.playback.position(), |position| {
                Message::Playback(PlaybackMessage::Seeking(position))
            })
            .on_release(Message::Playback(PlaybackMessage::SeekDone))
            .step(0.05)
            .height(18)
            .style(move |_, status| iced::widget::slider::Style {
                rail: iced::widget::slider::Rail {
                    backgrounds: (fade(ink, 0.85).into(), fade(ink, 0.18).into()),
                    width: 3.0,
                    border: Border { radius: 2.0.into(), ..Default::default() },
                },
                handle: iced::widget::slider::Handle {
                    shape: iced::widget::slider::HandleShape::Circle {
                        radius: if matches!(
                            status,
                            iced::widget::slider::Status::Hovered | iced::widget::slider::Status::Dragged
                        ) {
                            4.0
                        } else {
                            3.0
                        },
                    },
                    background: ink.into(),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            });
            if self.playback.current().is_some() {
                layers = layers.push(place(progress.into(), 200.0, top + base - 66.0 + 30.0 * idle, face - 44.0, 18.0));
            }
            if idle < 0.99 {
                layers =
                    layers.push(place(self.remote_transport(ink, 1.0 - idle), 178.0, top + base - 57.0, face, 49.0));
            }
            if state.sheet.is_some() {
                layers = layers.push(place(self.remote_sheet(ink), 194.0, top + 8.0, face - 32.0, base - 76.0));
            }
            if let Some(expansion) = state.expansion {
                let queue_top = if expansion.up { 4.0 } else { base + 6.0 };
                let shown = state.queue_reveal.value(state.frame_time);
                let height = expansion.height - 6.0;
                let content = crate::design_size::DesignSize {
                    content: self.remote_queue(ink),
                    size: iced::Size::new(face, height),
                };
                let clipped = container(Element::new(content)).height(height * shown).clip(true);
                layers = layers.push(place(
                    clipped.into(),
                    178.0,
                    queue_top + if expansion.up { height * (1.0 - shown) } else { 0.0 },
                    face,
                    height * shown,
                ));
            }
            layers = layers.push(crate::chrome::resize_frame().map(|message| match message {
                crate::chrome::Message::Resize(direction) => Message::Remote(Remote::Resize(direction)),
                _ => Message::Remote(Remote::Drag),
            }));
        }
        container(layers).style(move |_| container::Style { text_color: Some(ink), ..Default::default() }).into()
    }

    fn remote_key<'a>(
        &'a self,
        icon: Icon,
        label: &str,
        action: Remote,
        ink: Color,
        metrics: (f32, f32),
        enabled: bool,
    ) -> Element<'a, Message> {
        let (side, opacity) = metrics;
        let label = self.i18n.t(label).to_owned();
        let control = key(icon, Message::Remote(action), ink, side, opacity, enabled);
        let opacity = self.remote.opacity();
        let surface = if self.remote.settings.appearance.background == RemoteBackground::Default
            && self.appearance.theme == folia_store::ThemeMode::Light
        {
            Color::WHITE
        } else {
            Color::from_rgb8(36, 36, 40)
        };
        tooltip(
            control,
            container(typeface::text(label).size(11).color(ink)).padding(5).max_width(320),
            tooltip::Position::Bottom,
        )
        .style(move |_| container::Style {
            background: Some(fade(surface, opacity).into()),
            text_color: Some(ink),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
    }

    fn remote_transport(&self, ink: Color, opacity: f32) -> Element<'_, Message> {
        let enabled = self.playback.current().is_some();
        let queue = &self.playback.queue;
        let hover = |content, preview| {
            mouse_area(content)
                .on_enter(Message::Remote(Remote::Hover(Some(preview))))
                .on_exit(Message::Remote(Remote::Hover(None)))
        };
        let play: Element<'_, Message> = if self.playback.loading.active() || self.playback.is_buffering() {
            button(
                container(icons::spinner(
                    20.0,
                    fade(ink, opacity),
                    self.playback.loading_angle(self.remote.frame_time),
                ))
                .center(34),
            )
            .padding(0)
            .style(flat(ink))
            .on_press(Message::Playback(PlaybackMessage::Toggle))
            .into()
        } else {
            key(
                if self.playback.is_playing() { Icon::Pause } else { Icon::Play },
                Message::Playback(PlaybackMessage::Toggle),
                ink,
                34.0,
                opacity,
                enabled,
            )
        };
        let transport = row![
            hover(
                key(
                    Icon::SkipBack,
                    Message::Playback(PlaybackMessage::Previous),
                    ink,
                    28.0,
                    opacity,
                    enabled && queue.previous_index().is_some()
                ),
                crate::remote::Preview::Previous
            ),
            play,
            hover(
                key(
                    Icon::SkipForward,
                    Message::Playback(PlaybackMessage::Next),
                    ink,
                    28.0,
                    opacity,
                    enabled && (queue.next_index().is_some() || self.playback.is_fm())
                ),
                crate::remote::Preview::Next
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let like_ink = if self.song_liked() { Color::from_rgb8(255, 83, 119) } else { ink };
        let keys = row![
            hover(
                self.remote_key(
                    Icon::Heart,
                    "remote.like",
                    Remote::Like,
                    like_ink,
                    (30.0, opacity * 0.7),
                    enabled && self.panel_like_reason.is_none()
                ),
                crate::remote::Preview::Like
            ),
            key(
                match queue.repeat {
                    Repeat::One => Icon::Repeat1,
                    Repeat::All => Icon::Repeat,
                    Repeat::Off => Icon::Repeat2,
                },
                Message::Playback(PlaybackMessage::CycleRepeat),
                ink,
                30.0,
                opacity * 0.7,
                enabled
            ),
            self.remote_key(Icon::ListMusic, "remote.queue", Remote::Queue, ink, (30.0, opacity * 0.7), true),
            self.remote_key(Icon::Ellipsis, "remote.more", Remote::More, ink, (30.0, opacity * 0.7), true),
        ]
        .spacing(1);
        let time = format!(
            "{} / {}",
            crate::format_time(self.playback.position()),
            crate::format_time(self.playback.duration())
        );
        let preview = self
            .remote
            .hover
            .filter(|(_, at)| self.remote.frame_time.saturating_duration_since(*at).as_millis() >= 350)
            .map(|(preview, _)| preview);
        let time = match preview {
            Some(crate::remote::Preview::Previous) => queue
                .previous_index()
                .and_then(|index| queue.items().get(index))
                .map(|track| track.title.clone())
                .unwrap_or(time),
            Some(crate::remote::Preview::Like) => self.panel_like_reason.clone().unwrap_or(time),
            _ => time,
        };
        let keys: Element<'_, Message> = if preview == Some(crate::remote::Preview::Next) {
            queue.next_index().and_then(|index| queue.items().get(index)).map_or_else(
                || keys.into(),
                |track| {
                    container(typeface::text(&track.title).size(11).color(fade(ink, opacity * 0.65)))
                        .width(123)
                        .height(30)
                        .clip(true)
                        .align_y(Alignment::Center)
                        .into()
                },
            )
        } else {
            keys.into()
        };
        let time = button(
            container(typeface::text(time).size(11).color(fade(ink, opacity * 0.5))).width(112).height(18).clip(true),
        )
        .on_press(Message::Remote(Remote::Volume))
        .padding(0)
        .style(flat(ink));
        let tokens = crate::settings_kit::Tokens::new(
            &self.theme,
            self.remote.settings.appearance.background == RemoteBackground::Default
                && self.appearance.theme == folia_store::ThemeMode::Light,
        )
        .faded(self.remote.opacity());
        let volume: Element<'_, Message> = if self.remote.volume {
            slider(0.0..=1.0, self.playback.settings.volume, |volume| {
                Message::Playback(PlaybackMessage::Volume(volume))
            })
            .style(move |_, _| crate::settings_kit::slider_style(tokens))
            .on_release(Message::Playback(PlaybackMessage::VolumeDone))
            .step(0.01_f32)
            .width(110)
            .into()
        } else {
            time.into()
        };
        stack![
            container(volume).height(49).padding([0, 22]).align_y(Alignment::Center),
            container(transport).width(Length::Fill).height(49).center_x(Length::Fill).align_y(Alignment::Center),
            container(keys)
                .width(Length::Fill)
                .height(49)
                .padding([0, 14])
                .align_x(Alignment::End)
                .align_y(Alignment::Center),
        ]
        .width(Length::Fill)
        .height(49)
        .into()
    }

    fn remote_queue(&self, ink: Color) -> Element<'_, Message> {
        let light = self.remote.settings.appearance.background == RemoteBackground::Default
            && self.appearance.theme == folia_store::ThemeMode::Light;
        let opacity = self.remote.opacity();
        let mut tokens = crate::settings_kit::Tokens::new(&self.theme, light).faded(opacity);
        tokens.accent = ink;
        tokens.t1 = ink;
        tokens.t2 = fade(ink, 0.6);
        let list_height = self.remote.expansion.map_or(0.0, |value| value.height - 52.0).max(0.0);
        let queue = self.playback.queue.items();
        let list: Element<'_, Message> = if self.remote.queue.row_count(queue.len()) == 0 {
            container(typeface::text(self.i18n.t("remote.queueNoMatch")).size(12).color(fade(ink, 0.6)))
                .center_x(Length::Fill)
                .height(list_height)
                .into()
        } else {
            self.remote
                .queue
                .view(
                    crate::queue_island::Look {
                        i18n: &self.i18n,
                        tokens,
                        ink,
                        daylight: light,
                        actions_on_current: false,
                        actions_enabled: !self.playback.is_fm(),
                        reorder: false,
                        row: crate::queue_island::RowLook {
                            title: 12.0,
                            title_weight: iced::font::Weight::Normal,
                            active_weight: iced::font::Weight::Semibold,
                            artist: 10.0,
                        },
                    },
                    queue,
                    self.playback.queue.current_index(),
                    list_height,
                    self.remote.frame_time,
                )
                .map(move |message| {
                    use crate::queue_island::Message as Q;
                    let action = match &message {
                        Q::Jump(index) => Some((QueueAction::Play, *index)),
                        Q::MoveNext(index) => Some((QueueAction::Next, *index)),
                        Q::MoveEnd(index) => Some((QueueAction::End, *index)),
                        Q::Remove(index) => Some((QueueAction::Remove, *index)),
                        _ => None,
                    };
                    if let Some((action, index)) = action
                        && let Some(track) = queue.get(index)
                    {
                        Message::Remote(Remote::QueueAction(action, index, track.id))
                    } else {
                        Message::Remote(Remote::QueueList(message))
                    }
                })
        };
        let search_label = self.i18n.format("remote.queueSearch", &[("count", &queue.len().to_string())]);
        let search = typeface::text_input(&search_label, &self.remote.query)
            .style(move |_, _| crate::settings_kit::input_style(tokens))
            .on_input(|query| Message::Remote(Remote::Query(query)))
            .size(12)
            .padding(8);
        let head = row![
            search,
            self.remote_key(Icon::Shuffle, "remote.shuffle", Remote::Shuffle, ink, (30.0, 0.7), !self.playback.is_fm()),
            self.remote_key(Icon::X, "remote.close", Remote::Queue, ink, (30.0, 0.7), true)
        ]
        .spacing(4);
        let fill = if light { Color::from_rgb8(248, 247, 244) } else { Color::from_rgba8(17, 17, 20, 0.97) };
        container(column![head, list].spacing(2))
            .padding([6, 8])
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(fade(fill, opacity).into()),
                border: Border { radius: 14.0.into(), width: 1.0, color: fade(ink, 0.1) },
                ..Default::default()
            })
            .into()
    }

    fn remote_sheet(&self, ink: Color) -> Element<'_, Message> {
        let tab = self.remote.sheet.unwrap_or(Tab::Window);
        let tokens = crate::settings_kit::Tokens::new(
            &self.theme,
            self.remote.settings.appearance.background == RemoteBackground::Default
                && self.appearance.theme == folia_store::ThemeMode::Light,
        )
        .faded(self.remote.opacity());
        let tab_button = |key, tab| {
            button(typeface::text(self.i18n.t(key)).size(11).color(fade(ink, 0.7)))
                .on_press(Message::Remote(Remote::Tab(tab)))
                .style(flat(ink))
                .padding([4, 6])
        };
        let tabs = row![
            self.remote_key(Icon::ChevronLeft, "remote.back", Remote::More, ink, (20.0, 0.6), true),
            tab_button("remote.tabMainWindow", Tab::Window),
            tab_button("remote.tabExport", Tab::Export)
        ]
        .spacing(3);
        let body: Element<'_, Message> = match tab {
            Tab::Export => typeface::text(self.i18n.t("remote.videoUnavailable")).size(12).color(fade(ink, 0.7)).into(),
            Tab::Size => {
                let mut presets = row![].spacing(4);
                for (index, [width, height]) in self.remote.settings.size_presets.iter().copied().enumerate() {
                    presets = presets.push(
                        button(typeface::text(format!("{width} × {height}")).size(11))
                            .padding(4)
                            .on_press(Message::Remote(Remote::Preset(index)))
                            .style(flat(ink)),
                    );
                }
                let fields = row![
                    typeface::text_input(self.i18n.t("remote.widthPx"), &self.remote.width)
                        .style(move |_, _| crate::settings_kit::input_style(tokens))
                        .size(12)
                        .width(70)
                        .on_input(|v| Message::Remote(Remote::Width(v))),
                    typeface::text("×"),
                    typeface::text_input(self.i18n.t("remote.heightPx"), &self.remote.height)
                        .style(move |_, _| crate::settings_kit::input_style(tokens))
                        .size(12)
                        .width(70)
                        .on_input(|v| Message::Remote(Remote::Height(v))),
                    button(typeface::text(self.i18n.t("remote.save")).size(12))
                        .on_press(Message::Remote(Remote::SaveSize))
                        .style(flat(ink))
                ]
                .spacing(4)
                .align_y(Alignment::Center);
                column![presets, fields].spacing(5).into()
            }
            Tab::Window => {
                let switch = |key, on, action| {
                    button(typeface::text(self.i18n.t(key)).size(11).color(fade(ink, if on { 1.0 } else { 0.5 })))
                        .on_press(Message::Remote(action))
                        .padding([3, 6])
                        .style(move |_, _| button::Style {
                            background: Some(fade(ink, if on { 0.12 } else { 0.03 }).into()),
                            text_color: ink,
                            border: Border { radius: 5.0.into(), ..Default::default() },
                            ..Default::default()
                        })
                };
                let first = row![
                    switch(
                        "remote.standard",
                        !self.theme_settings.transparent_player_background,
                        Remote::MainTransparent(false)
                    ),
                    switch(
                        "remote.transparent",
                        self.theme_settings.transparent_player_background,
                        Remote::MainTransparent(true)
                    ),
                    space().width(Length::Fill),
                    switch("remote.rowSize", false, Remote::Tab(Tab::Size)),
                    self.remote_key(Icon::ExternalLink, "remote.showMain", Remote::MainVisible, ink, (24.0, 0.7), true)
                ]
                .spacing(4);
                let second = row![
                    typeface::text(self.i18n.t("remote.rowChrome")).size(11),
                    switch(
                        "remote.chromeAlwaysVisible",
                        self.remote.main_chrome == Chrome::Visible,
                        Remote::MainChrome(Chrome::Visible)
                    ),
                    switch(
                        "remote.chromeAutoHide",
                        self.remote.main_chrome == Chrome::Auto,
                        Remote::MainChrome(Chrome::Auto)
                    ),
                    switch(
                        "remote.chromeAlwaysHidden",
                        self.remote.main_chrome == Chrome::Hidden,
                        Remote::MainChrome(Chrome::Hidden)
                    )
                ]
                .spacing(5)
                .align_y(Alignment::Center);
                let third = row![
                    typeface::text(self.i18n.t("remote.rowWindow")).size(11),
                    switch("remote.border", self.remote.main_border, Remote::MainBorder(!self.remote.main_border)),
                    switch("remote.through", self.remote.main_through, Remote::MainThrough(!self.remote.main_through)),
                    self.remote_key(
                        Icon::Pin,
                        if self.remote.capabilities.pin { "remote.pin" } else { "remote.pinUnavailable" },
                        Remote::MainPin(!self.remote.main_pin),
                        ink,
                        (24.0, if self.remote.main_pin { 1.0 } else { 0.5 }),
                        self.remote.capabilities.pin
                    ),
                    space().width(Length::Fill),
                    self.remote_key(Icon::Volume2, "remote.volume", Remote::Volume, ink, (24.0, 0.7), true)
                ]
                .spacing(5)
                .align_y(Alignment::Center);
                column![first, second, third].spacing(4).into()
            }
        };
        column![tabs, body].spacing(4).into()
    }

    pub(crate) fn remote_line(&self) -> (Option<&folia_core::Line>, Option<&folia_core::Line>, bool) {
        self.remote.lyric.current()
    }
}

fn key<'a>(icon: Icon, action: Message, ink: Color, side: f32, opacity: f32, enabled: bool) -> Element<'a, Message> {
    let color = fade(ink, opacity * if enabled { 1.0 } else { 0.3 });
    let size = if side > 30.0 { 21.0 } else { 14.0 };
    let glyph = match icon {
        Icon::Play => icons::variant(icons::Variant::FilledPlay, size, color),
        Icon::Pause => icons::variant(icons::Variant::FilledPause, size, color),
        Icon::Heart if ink.r > 0.9 && ink.g < 0.5 => icons::variant(icons::Variant::FilledHeart, size, color),
        _ => icons::icon(icon, size, color),
    };
    button(container(glyph).center(side))
        .padding(0)
        .on_press_maybe(enabled.then_some(action))
        .style(move |_, status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(fade(ink, 0.1).into()),
            text_color: ink,
            border: Border { radius: (side * 0.5).into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

fn place<'a>(content: Element<'a, Message>, x: f32, y: f32, width: f32, height: f32) -> Element<'a, Message> {
    container(container(content).width(width).height(height))
        .padding(Padding { left: x, top: y, right: 0.0, bottom: 0.0 })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub(crate) fn remote_ink(mode: RemoteBackground, light: bool) -> Color {
    if mode == RemoteBackground::Default && light { Color::from_rgb8(17, 17, 22) } else { Color::WHITE }
}

pub(crate) fn fade(color: Color, alpha: f32) -> Color {
    Color { a: color.a * alpha, ..color }
}

fn flat(ink: Color) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        text_color: ink,
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(fade(ink, 0.08).into()),
        border: Border { radius: 5.0.into(), ..Default::default() },
        ..Default::default()
    }
}
