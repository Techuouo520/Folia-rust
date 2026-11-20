// Routes companion actions and native events to the single application shell.

use std::time::{Duration, Instant};

use folia_render::{backend, desktop_window};
use iced::{Element, Point, Size, Subscription, Task, window};

use crate::remote::{self, Message as Remote, QueueAction, Tab};
use crate::{Message, PlaybackMessage, SettingsMessage, SettingsSection, Shell};

type InputHandler = fn(iced::Event, iced::event::Status, window::Id) -> Option<Message>;

impl Shell {
    pub fn boot(init: crate::Init, settings: window::Settings) -> (Self, Task<Message>) {
        let (mut shell, tasks) = Self::new(init);
        shell.remote.icon = settings.icon.clone();
        let (id, open) = window::open(settings);
        shell.window = Some(id);
        (shell, Task::batch([tasks, open.map(|id| Message::WindowReady(Some(id)))]))
    }

    pub fn title_for(&self, id: window::Id) -> String {
        if self.remote.id == Some(id) { self.i18n.t("remote.windowTitle").to_owned() } else { self.title() }
    }

    pub fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.remote.id == Some(id) {
            return self.remote_view().map(move |message| Message::RemoteInput(id, Box::new(message)));
        }
        if self.window != Some(id) {
            return iced::widget::space().into();
        }
        let fill = self.window_style(&self.iced_theme).background_color;
        iced::widget::container(self.view())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .style(move |_| iced::widget::container::Style { background: Some(fill.into()), ..Default::default() })
            .into()
    }

    pub(crate) fn main_events(&self, handler: InputHandler) -> Subscription<Message> {
        iced::event::listen_with(|event, status, id| Some((event, status, id))).with((self.window, handler)).filter_map(
            |((window, handler), (event, status, id))| {
                if window == Some(id) { handler(event, status, id) } else { None }
            },
        )
    }

    pub(crate) fn remote_events(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, id| Some((event, status, id))).with(self.remote.id).filter_map(
            |(remote, (event, status, id))| {
                (remote == Some(id)).then_some(Message::Remote(Remote::Event(id, event, status)))
            },
        )
    }

    pub(crate) fn remote_message(&mut self, message: Remote) -> Task<Message> {
        if !self.remote.loaded && !matches!(message, Remote::Loaded(_) | Remote::Open) {
            self.remote.deferred.push(message);
            return Task::none();
        }
        match message {
            Remote::Loaded(settings) => {
                if !self.remote.loaded {
                    self.remote.settings = *settings;
                    self.remote.main_border = self.remote.settings.main_border;
                    self.remote.main_pin = self.remote.settings.main_pin;
                    self.remote.main_chrome = self.remote.settings.main_chrome;
                    let [width, height] = self.remote.settings.size_presets[self.remote.settings.selected_preset];
                    self.remote.width = width.to_string();
                    self.remote.height = height.to_string();
                    self.apply_main_window();
                }
                self.remote.loaded = true;
                let pending = std::mem::take(&mut self.remote.deferred);
                let mut tasks: Vec<_> = pending.into_iter().map(|message| self.remote_message(message)).collect();
                if self.remote.open_pending {
                    tasks.push(self.open_remote());
                }
                Task::batch(tasks)
            }
            Remote::Saved(result) => {
                self.remote.saving = false;
                if let Err(error) = result {
                    self.remote_error(error);
                }
                if self.remote.dirty {
                    return self.remote.persist(&self.store);
                }
                if self.remote.close_application {
                    return self.request_close();
                }
                Task::none()
            }
            Remote::SlidersFinished => {
                self.remote.finishing_sliders = self.remote.finishing_sliders.saturating_sub(1);
                if self.remote.close_application && !self.remote.saving && !self.remote.dirty {
                    self.request_close()
                } else {
                    Task::none()
                }
            }
            Remote::Open => self.open_remote(),
            Remote::Close => self.close_remote(),
            Remote::Attached(id, raw) if self.remote.id == Some(id) => {
                self.remote.raw_id = Some(raw);
                if let Some(window) = backend::native_window(raw) {
                    let geometry = desktop_window::restore_geometry(&window, self.remote.settings.position);
                    self.remote.capabilities = geometry.capabilities;
                    let position = remote::restored_position(&self.remote.settings, &geometry);
                    desktop_window::place(&window, position, Size::from(self.remote.settings.size));
                    self.apply_remote_window();
                    window.set_visible(true);
                    window.focus_window();
                }
                Task::none()
            }
            Remote::Attached(id, _) => window::close(id),
            Remote::Event(id, event, status) if self.remote.id == Some(id) => self.remote_event(event, status),
            Remote::Event(..) => Task::none(),
            Remote::Geometry(id, serial) if self.remote.id == Some(id) && serial == self.remote.geometry_serial => {
                self.remember_remote_geometry();
                self.remote.persist(&self.store)
            }
            Remote::Geometry(..) => Task::none(),
            Remote::Idle(id, serial) => {
                if self.remote.id == Some(id)
                    && self.remote.idle_serial == serial
                    && self.remote.pointer.is_none()
                    && !(self.remote.focused && self.remote.typing)
                {
                    self.remote.sheet = None;
                    self.remote.volume = false;
                    return self.fold_remote_queue();
                }
                Task::none()
            }
            Remote::QueueFolded(id, serial) => {
                if self.remote.id == Some(id) && self.remote.queue_serial == serial && !self.remote.queue_target {
                    self.set_remote_queue(false);
                }
                Task::none()
            }
            Remote::QueueShift(id, serial, open) => {
                if self.remote.id == Some(id) && self.remote.queue_serial == serial && self.remote.queue_target == open
                {
                    self.begin_remote_shift(open)
                } else {
                    Task::none()
                }
            }
            Remote::QueueHidden(id, serial, open) => {
                if self.remote.id == Some(id) && self.remote.queue_serial == serial && self.remote.queue_target == open
                {
                    self.remote.frame(Instant::now());
                    self.remote.shift = Some(remote::Shift::Hidden { open, frames: 2 });
                }
                Task::none()
            }
            Remote::WashReady(id, serial, result) => {
                if self.remote.id == Some(id) && self.remote.wash_serial == serial {
                    match result {
                        Ok(handle) => {
                            self.remote.wash_outgoing = self.remote.wash.replace(handle);
                            if self.remote.wash_outgoing.is_some() {
                                self.remote.wash_fade.restart(0.0, Instant::now());
                            }
                        }
                        Err(error) => self.remote_error(error),
                    }
                }
                Task::none()
            }
            Remote::OpenSettings => {
                self.settings = Some(crate::Settings::open());
                let task = self.handle(Message::Settings(SettingsMessage::Section(SettingsSection::Remote)));
                Task::batch([task, self.raise_main()])
            }
            Remote::Unlock => {
                self.remote.settings.click_through = false;
                self.remote.main_through = false;
                self.apply_remote_window();
                self.apply_main_window();
                self.remote.persist(&self.store)
            }
            Remote::Background => {
                self.remote.settings.appearance.background = self.remote.settings.appearance.background.next();
                self.remote.persist(&self.store)
            }
            Remote::Pin => {
                if !self.remote.capabilities.pin {
                    return Task::none();
                }
                self.remote.settings.always_on_top = !self.remote.settings.always_on_top;
                self.apply_remote_window();
                self.remote.persist(&self.store)
            }
            Remote::Taskbar(hidden) => {
                self.remote.settings.hide_taskbar_icon = hidden;
                self.apply_remote_window();
                self.remote.persist(&self.store)
            }
            Remote::Titlebar(hidden) => {
                self.remote.settings.appearance.hide_titlebar = hidden;
                self.remote.persist(&self.store)
            }
            Remote::ClickThrough(enabled) => {
                if enabled && self.remote.main_through {
                    self.remote.main_through = false;
                    self.apply_main_window();
                }
                self.remote.settings.click_through = enabled;
                if enabled {
                    self.remote.sheet = None;
                    self.remote.queue_serial = self.remote.queue_serial.wrapping_add(1);
                    self.remote.shift = None;
                    self.remote.window_fade.snap(1.0, Instant::now());
                    self.remote.pointer = None;
                    self.remote.held = false;
                    self.remote.drag_from = None;
                    self.remote.hover = None;
                    self.set_remote_queue(false);
                }
                self.apply_remote_window();
                self.remote.frame(Instant::now());
                self.remote.persist(&self.store)
            }
            Remote::ExpandCover(enabled) => {
                self.remote.settings.appearance.expand_cover = enabled;
                self.remote.persist(&self.store)
            }
            Remote::TextShade(enabled) => {
                self.remote.settings.appearance.text_shade = enabled;
                self.remote.persist(&self.store)
            }
            Remote::Pairs(pairs) => {
                self.remote.settings.appearance.lyric_pairs = pairs.clamp(1, 2);
                self.remote.persist(&self.store)
            }
            Remote::Source(index, source) => {
                if let Some(row) = self.remote.settings.appearance.lyric_rows.get_mut(index) {
                    *row = source;
                }
                self.remote.settings.appearance.normalize();
                self.remote.persist(&self.store)
            }
            Remote::ExportAppearance => match self.remote.settings.appearance.export() {
                Ok(text) => iced::clipboard::write(text),
                Err(error) => {
                    self.remote_error(error.to_string());
                    Task::none()
                }
            },
            Remote::ImportAppearance => iced::clipboard::read().map(|text| Message::Remote(Remote::Imported(text))),
            Remote::Imported(Some(text)) => {
                match folia_store::RemoteAppearance::import(&text) {
                    Ok(appearance) => self.remote.settings.appearance = appearance,
                    Err(error) => {
                        self.remote_error(error.to_string());
                        return Task::none();
                    }
                }
                self.remote.persist(&self.store)
            }
            Remote::Imported(None) => Task::none(),
            Remote::Drag => {
                self.remote.drag_from = self.remote.pointer;
                self.remote.held = true;
                Task::none()
            }
            Remote::Resize(direction) => {
                self.remote.id.map_or_else(Task::none, |id| window::drag_resize(id, direction))
            }
            Remote::Queue => {
                if self.remote.queue_target {
                    self.fold_remote_queue()
                } else {
                    self.request_remote_queue()
                }
            }
            Remote::Query(query) => {
                self.remote.query = query;
                self.remote.queue.filter(self.playback.queue.items(), &self.remote.query);
                self.remote.typing = true;
                Task::none()
            }
            Remote::QueueList(message) => {
                let height = self.remote.expansion.map_or(0.0, |value| value.height - 52.0);
                let (task, change) = self.remote.queue.update(message, self.playback.queue.items(), height);
                let playback = change.map_or_else(Task::none, |change| {
                    self.handle(Message::Playback(match change {
                        crate::queue_island::Change::Jump(index) => PlaybackMessage::Jump(index),
                        crate::queue_island::Change::MoveNext(index) => PlaybackMessage::MoveNext(index),
                        crate::queue_island::Change::MoveEnd(index) => PlaybackMessage::MoveEnd(index),
                        crate::queue_island::Change::Remove(index) => PlaybackMessage::Remove(index),
                        crate::queue_island::Change::Reorder(from, to) => PlaybackMessage::Reorder(from, to),
                    }))
                });
                Task::batch([task.map(|message| Message::Remote(Remote::QueueList(message))), playback])
            }
            Remote::QueueAction(action, index, key) => {
                if self.playback.is_fm() && action != QueueAction::Play {
                    return Task::none();
                }
                let items = self.playback.queue.items();
                let index = if items.get(index).is_some_and(|track| track.id == key) {
                    Some(index)
                } else {
                    items.iter().position(|track| track.id == key)
                };
                let Some(index) = index else {
                    return Task::none();
                };
                let message = match action {
                    QueueAction::Play => PlaybackMessage::Jump(index),
                    QueueAction::Next => PlaybackMessage::MoveNext(index),
                    QueueAction::End => PlaybackMessage::MoveEnd(index),
                    QueueAction::Remove => PlaybackMessage::Remove(index),
                };
                self.handle(Message::Playback(message))
            }
            Remote::More => {
                self.remote.typing = false;
                self.remote.sheet = if self.remote.sheet.is_some() { None } else { Some(Tab::Window) };
                self.remote.volume = false;
                Task::none()
            }
            Remote::Tab(tab) => {
                self.remote.typing = false;
                self.remote.sheet = Some(tab);
                Task::none()
            }
            Remote::MainVisible => self.raise_main(),
            Remote::MainTransparent(enabled) => {
                if !enabled {
                    self.remote.main_through = false;
                    self.apply_main_window();
                }
                if enabled && self.playback.current().is_some() {
                    self.page = crate::Page::NowPlaying;
                }
                self.handle(Message::Panel(crate::panel::Message::TransparentBackground(enabled)))
            }
            Remote::MainBorder(visible) => {
                self.remote.main_border = visible;
                self.apply_main_window();
                self.remote.persist(&self.store)
            }
            Remote::MainThrough(enabled) => {
                self.remote.main_through = enabled && self.player_transparent();
                if self.remote.main_through {
                    self.remote.settings.click_through = false;
                    self.apply_remote_window();
                }
                self.apply_main_window();
                self.remote.persist(&self.store)
            }
            Remote::MainPin(enabled) => {
                if !self.remote.capabilities.pin {
                    return Task::none();
                }
                self.remote.main_pin = enabled;
                self.apply_main_window();
                self.remote.persist(&self.store)
            }
            Remote::MainChrome(mode) => {
                self.remote.main_chrome = mode;
                self.reveal_chrome();
                self.remote.persist(&self.store)
            }
            Remote::Preset(index) if index < 3 => {
                self.remote.settings.selected_preset = index;
                let [width, height] = self.remote.settings.size_presets[index];
                self.remote.width = width.to_string();
                self.remote.height = height.to_string();
                Task::batch([self.remote.persist(&self.store), self.resize_main(width, height)])
            }
            Remote::Preset(_) => Task::none(),
            Remote::Width(value) => {
                self.remote.typing = true;
                self.remote.width = value.chars().filter(char::is_ascii_digit).take(4).collect();
                Task::none()
            }
            Remote::Height(value) => {
                self.remote.typing = true;
                self.remote.height = value.chars().filter(char::is_ascii_digit).take(4).collect();
                Task::none()
            }
            Remote::SaveSize => {
                self.remote.typing = false;
                let (Ok(width), Ok(height)) = (self.remote.width.parse(), self.remote.height.parse()) else {
                    return Task::none();
                };
                let index = self.remote.settings.selected_preset;
                self.remote.settings.size_presets[index] = [width, height];
                self.remote.settings.normalize();
                self.remote_message(Remote::Preset(index))
            }
            Remote::Like => self.toggle_like(),
            Remote::Shuffle => self.home(crate::home::Message::Shuffle),
            Remote::Volume => {
                self.remote.volume = !self.remote.volume;
                Task::none()
            }
            Remote::Hover(value) => {
                self.remote.hover = value.map(|value| (value, Instant::now()));
                Task::none()
            }
        }
    }

    fn open_remote(&mut self) -> Task<Message> {
        if !self.remote.loaded {
            self.remote.open_pending = true;
            return Task::none();
        }
        self.remote.open_pending = false;
        if let Some(id) = self.remote.id {
            return Task::batch([
                self.remote_message(Remote::Unlock),
                window::minimize(id, false),
                window::gain_focus(id),
            ]);
        }
        let mut settings = backend::window_settings(Size::from(self.remote.settings.size), Size::new(602.0, 184.0));
        settings.exit_on_close_request = false;
        settings.icon = self.remote.icon.clone();
        settings.visible = false;
        settings.position = window::Position::Centered;
        settings.max_size = Some(Size::new(1204.0, 368.0));
        #[cfg(windows)]
        {
            settings.platform_specific.skip_taskbar = self.remote.settings.hide_taskbar_icon;
        }
        let (id, open) = window::open(settings);
        self.remote.id = Some(id);
        self.remote.left_at = Some(Instant::now());
        tracing::info!(?id, "remote window opened");
        open.then(|id| window::raw_id::<Message>(id).map(move |raw| Message::Remote(Remote::Attached(id, raw))))
    }

    pub(crate) fn close_remote(&mut self) -> Task<Message> {
        let sliders = self.finish_remote_sliders();
        self.remember_remote_geometry();
        let Some(id) = self.remote.id.take() else {
            return Task::none();
        };
        self.remote.raw_id = None;
        self.remote.expansion = None;
        self.remote.queue_target = false;
        self.remote.sheet = None;
        self.remote.outgoing = None;
        self.remote.current_face = None;
        self.remote.wash = None;
        self.remote.wash_outgoing = None;
        self.remote.wash_source = None;
        self.remote.wash_serial = self.remote.wash_serial.wrapping_add(1);
        self.remote.pointer = None;
        self.remote.shape.clear();
        self.remote.shape_scale = 0.0;
        self.remote.lyric = crate::remote_lyrics::Presentation::default();
        self.remote.main_through = false;
        self.remote.reset_presentation();
        self.apply_main_window();
        tracing::info!(?id, "remote window closed");
        Task::batch([window::close(id), sliders]).chain(self.remote.persist(&self.store))
    }

    fn remote_event(&mut self, event: iced::Event, status: iced::event::Status) -> Task<Message> {
        if matches!(event, iced::Event::Keyboard(_))
            && let Some(id) = self.remote.id
            && let Some(Message::Remote(message)) = crate::remote_keys(event.clone(), status, id)
        {
            return self.remote_message(message);
        }
        if self.remote.settings.click_through && matches!(event, iced::Event::Mouse(_) | iced::Event::Keyboard(_)) {
            return Task::none();
        }
        match event {
            iced::Event::Window(window::Event::CloseRequested) => return self.close_remote(),
            iced::Event::Window(window::Event::Resized(size)) => {
                let extra = self.remote.expansion.map_or(0.0, |value| value.height);
                if size.width >= 602.0 && size.height >= 184.0 + extra {
                    self.remote.settings.size = [size.width, size.height - extra];
                    self.remote.settings.normalize();
                }
                return self.remote_geometry_timer();
            }
            iced::Event::Window(window::Event::Moved(_)) => return self.remote_geometry_timer(),
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                self.remote.pointer = Some(position);
                self.remote.left_at = None;
                if self.remote.held && self.remote.drag_from.is_some_and(|from| from.distance(position) >= 4.0) {
                    self.remote.drag_from = None;
                    return self.remote.id.map_or_else(Task::none, window::drag);
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                self.remote.pointer = None;
                self.remote.left_at = Some(Instant::now());
                self.remote.frame(Instant::now());
                return self.remote_idle_timer();
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(_)) => {
                self.remote.held = false;
                self.remote.drag_from = None;
                return self.finish_remote_sliders();
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => self.remote.held = true,
            iced::Event::Window(window::Event::Unfocused) => {
                self.remote.focused = false;
                self.remote.held = false;
                self.remote.drag_from = None;
                if self.remote.pointer.is_none() {
                    self.remote.frame(Instant::now());
                    return Task::batch([self.finish_remote_sliders(), self.remote_idle_timer()]);
                }
                return self.finish_remote_sliders();
            }
            iced::Event::Window(window::Event::Focused) => self.remote.focused = true,
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, repeat: false, .. })
                if status == iced::event::Status::Ignored =>
            {
                use iced::keyboard::{Key, key::Named};
                match key {
                    Key::Named(Named::Space) => return self.handle(Message::Playback(PlaybackMessage::Toggle)),
                    Key::Named(Named::Escape) if self.remote.sheet.take().is_none() => {
                        if self.remote.query.is_empty() {
                            self.set_remote_queue(false);
                        } else {
                            self.remote.query.clear();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        self.remote.frame(Instant::now());
        Task::none()
    }

    fn remember_remote_geometry(&mut self) {
        let Some(window) = self.remote.raw_id.and_then(backend::native_window) else { return };
        if window.is_minimized() == Some(true) {
            return;
        }
        let geometry = desktop_window::geometry(&window);
        if let Some(position) = geometry.position {
            let offset = self
                .remote
                .expansion
                .filter(|expansion| expansion.up)
                .map_or(0.0, |expansion| expansion.height * geometry.scale);
            self.remote.settings.position = Some([position.x as i32, (position.y + offset) as i32]);
        }
        self.remote.settings.size = [
            geometry.size.width,
            geometry.size.height - self.remote.expansion.map_or(0.0, |expansion| expansion.height),
        ];
        self.remote.settings.normalize();
    }

    fn finish_remote_sliders(&mut self) -> Task<Message> {
        let seek = if std::mem::take(&mut self.remote.seek_drag) {
            self.handle(Message::Playback(PlaybackMessage::SeekDone))
        } else {
            Task::none()
        };
        let volume = if std::mem::take(&mut self.remote.volume_drag) {
            self.handle(Message::Playback(PlaybackMessage::VolumeDone))
        } else {
            Task::none()
        };
        let task = Task::batch([seek, volume]);
        if task.units() == 0 {
            return task;
        }
        self.remote.finishing_sliders = self.remote.finishing_sliders.saturating_add(1);
        task.chain(Task::done(Message::Remote(Remote::SlidersFinished)))
    }

    fn set_remote_queue(&mut self, open: bool) {
        let Some(window) = self.remote.raw_id.and_then(backend::native_window) else {
            return;
        };
        let geometry = desktop_window::geometry(&window);
        let wanted = 52.0 + 36.0 * self.playback.queue.items().len().clamp(1, 8) as f32;
        let plan = remote::queue_geometry(&geometry, self.remote.expansion, open, wanted);
        self.remote.expansion = plan.expansion;
        self.remote.queue_target = open;
        self.remote.queue_reveal.set_with(
            f32::from(open),
            if open { 340.0 } else { 200.0 },
            folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0),
            Instant::now(),
        );
        if !open {
            self.remote.query.clear();
            self.remote.queue.filter(self.playback.queue.items(), "");
        }
        let extra = plan.expansion.map_or(0.0, |value| value.height);
        desktop_window::resize_limits(&window, Size::new(602.0, 184.0 + extra), Size::new(1204.0, 368.0 + extra));
        desktop_window::place(&window, plan.position, plan.size);
    }

    fn request_remote_queue(&mut self) -> Task<Message> {
        let Some(window) = self.remote.raw_id.and_then(backend::native_window) else {
            return Task::none();
        };
        self.remote.queue_target = true;
        self.remote.queue_serial = self.remote.queue_serial.wrapping_add(1);
        let geometry = desktop_window::geometry(&window);
        let plan = remote::queue_geometry(
            &geometry,
            self.remote.expansion,
            true,
            52.0 + 36.0 * self.playback.queue.items().len().clamp(1, 8) as f32,
        );
        if plan.expansion.is_some_and(|next| next.up) && plan.expansion != self.remote.expansion {
            return self.begin_remote_shift(true);
        }
        self.remote.shift = None;
        self.remote.window_fade.set_with(1.0, 140.0, folia_visualizer::Ease::OUT, Instant::now());
        self.set_remote_queue(true);
        Task::none()
    }

    fn begin_remote_shift(&mut self, open: bool) -> Task<Message> {
        self.remote.window_fade.set_with(0.0, 90.0, folia_visualizer::Ease::OUT, Instant::now());
        self.remote.shift = Some(remote::Shift::Fading);
        let serial = self.remote.queue_serial;
        self.remote.id.map_or_else(Task::none, |id| {
            Task::perform(async { tokio::time::sleep(Duration::from_millis(90)).await }, move |()| {
                Message::Remote(Remote::QueueHidden(id, serial, open))
            })
        })
    }

    pub(crate) fn advance_remote_shift(&mut self) {
        use remote::Shift;
        match self.remote.shift {
            Some(Shift::Hidden { open, frames: 0 | 1 }) => {
                let entering = open && self.remote.expansion.is_none();
                self.set_remote_queue(open);
                if entering {
                    self.remote.queue_reveal.snap(0.0, Instant::now());
                }
                self.remote.shift = Some(Shift::Revealing(2));
            }
            Some(Shift::Hidden { open, frames }) => {
                self.remote.shift = Some(Shift::Hidden { open, frames: frames - 1 })
            }
            Some(Shift::Revealing(0 | 1)) => {
                self.remote.shift = None;
                self.remote.window_fade.set_with(1.0, 140.0, folia_visualizer::Ease::OUT, Instant::now());
                if self.remote.queue_target {
                    self.remote.queue_reveal.set_with(
                        1.0,
                        340.0,
                        folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0),
                        Instant::now(),
                    );
                }
            }
            Some(Shift::Revealing(frames)) => self.remote.shift = Some(Shift::Revealing(frames - 1)),
            Some(Shift::Fading) | None => {}
        }
    }

    fn remote_geometry_timer(&mut self) -> Task<Message> {
        self.remote.geometry_serial = self.remote.geometry_serial.wrapping_add(1);
        let serial = self.remote.geometry_serial;
        self.remote.id.map_or_else(Task::none, |id| {
            Task::perform(async { tokio::time::sleep(Duration::from_millis(350)).await }, move |()| {
                Message::Remote(Remote::Geometry(id, serial))
            })
        })
    }

    fn fold_remote_queue(&mut self) -> Task<Message> {
        self.remote.queue_target = false;
        self.remote.queue_serial = self.remote.queue_serial.wrapping_add(1);
        self.remote.queue_reveal.set_with(0.0, 200.0, folia_visualizer::Ease::OUT, Instant::now());
        self.remote.shift = None;
        self.remote.window_fade.set_with(1.0, 140.0, folia_visualizer::Ease::OUT, Instant::now());
        let serial = self.remote.queue_serial;
        let up = self.remote.expansion.is_some_and(|value| value.up);
        self.remote.id.map_or_else(Task::none, |id| {
            Task::perform(
                async move { tokio::time::sleep(Duration::from_millis(if up { 155 } else { 200 })).await },
                move |()| {
                    Message::Remote(if up {
                        Remote::QueueShift(id, serial, false)
                    } else {
                        Remote::QueueFolded(id, serial)
                    })
                },
            )
        })
    }

    fn remote_idle_timer(&mut self) -> Task<Message> {
        self.remote.idle_serial = self.remote.idle_serial.wrapping_add(1);
        let serial = self.remote.idle_serial;
        self.remote.id.map_or_else(Task::none, |id| {
            Task::perform(async { tokio::time::sleep(Duration::from_secs(6)).await }, move |()| {
                Message::Remote(Remote::Idle(id, serial))
            })
        })
    }

    fn apply_remote_window(&mut self) {
        let Some(window) = self.remote.raw_id.and_then(backend::native_window) else {
            return;
        };
        let caps = desktop_window::capabilities(&window);
        if caps.taskbar
            && let Err(error) = desktop_window::set_taskbar(&window, !self.remote.settings.hide_taskbar_icon)
        {
            self.remote_error(error);
        }
        if caps.pin
            && let Err(error) = desktop_window::set_pin(&window, self.remote.settings.always_on_top)
        {
            self.remote_error(error);
        }
        if let Err(error) = desktop_window::set_click_through(&window, self.remote.settings.click_through) {
            self.remote.settings.click_through = false;
            self.remote_error(error);
        }
        self.remote.shape.clear();
    }

    pub(crate) fn apply_main_window(&mut self) {
        if let Some(window) = self.main_raw_id.and_then(backend::native_window) {
            window.set_resizable(self.remote.main_border);
            if let Err(error) = desktop_window::set_click_through(&window, self.remote.main_through) {
                self.remote.main_through = false;
                self.remote_error(error);
            }
            if desktop_window::capabilities(&window).pin
                && let Err(error) = desktop_window::set_pin(&window, self.remote.main_pin)
            {
                self.remote_error(error);
            }
        }
    }

    fn raise_main(&mut self) -> Task<Message> {
        self.remote.main_through = false;
        self.apply_main_window();
        self.window.map_or_else(Task::none, |id| Task::batch([window::minimize(id, false), window::gain_focus(id)]))
    }

    fn resize_main(&mut self, width: u32, height: u32) -> Task<Message> {
        self.window.map_or_else(Task::none, |id| window::resize(id, Size::new(width as f32, height as f32)))
    }

    fn remote_error(&mut self, error: String) {
        tracing::warn!(%error, "remote window operation failed");
        let at = Instant::now();
        self.remote.error_at = Some(at);
        self.status = Some((false, self.i18n.t("remote.operationFailed").to_owned(), at));
    }

    pub(crate) fn sync_remote_visuals(&mut self) -> Task<Message> {
        let Some(window_id) = self.remote.id else {
            return Task::none();
        };
        self.sync_remote_shape();
        let time = self.playback.lyric_time();
        let (line, next, singing) = self
            .playback
            .lyrics
            .as_ref()
            .map_or((None, None, false), |lyrics| crate::remote_lyrics::line_at(&lyrics.lines, time));
        self.remote.lyric.sync(line, next, singing, time, Instant::now());
        let mut queue_layout = Task::none();
        if self.remote.queue_revision != self.playback.queue.revision() {
            self.remote.queue_revision = self.playback.queue.revision();
            self.remote.queue.filter(self.playback.queue.items(), &self.remote.query);
            if self.remote.queue_target {
                queue_layout = self.request_remote_queue();
            }
        }
        if let Some(expansion) = self.remote.expansion {
            self.remote.queue.follow(
                self.playback.queue.items(),
                self.playback.queue.current_index(),
                (expansion.height - 52.0).max(0.0),
                Instant::now(),
            );
        }
        if self.remote.main_through && !self.player_transparent() {
            self.remote.main_through = false;
            self.apply_main_window();
        }
        let track = self.playback.current();
        let cover = track.and_then(|track| self.art.peek(&track.id)).and_then(Option::as_ref);
        if self.remote.current_face.as_ref().map(|(track, _)| track.id) != track.map(|track| track.id) {
            self.remote.outgoing = self.remote.current_face.take();
            self.remote.current_face = track.map(|track| (track.clone(), cover.cloned()));
            if self.remote.outgoing.is_some() {
                self.remote.handoff.restart(0.0, Instant::now());
            }
        } else if let Some((_, face)) = &mut self.remote.current_face {
            *face = cover.cloned();
        }
        if self.remote.wash_source.as_ref() == cover {
            return queue_layout;
        }
        self.remote.wash_source = cover.cloned();
        self.remote.wash_serial = self.remote.wash_serial.wrapping_add(1);
        let serial = self.remote.wash_serial;
        let Some(cover) = cover.cloned() else {
            self.remote.wash = None;
            self.remote.wash_outgoing = None;
            return queue_layout;
        };
        let art = Task::perform(
            crate::background::blocking(move || {
                let source = crate::preview_art::source_of(Some(&cover))
                    .ok_or_else(|| "remote cover has no decoded pixels".to_owned())?;
                let mut thumbnail =
                    image::imageops::resize(source.as_ref(), 230, 96, image::imageops::FilterType::Triangle);
                for pixel in thumbnail.pixels_mut() {
                    for channel in 0..3 {
                        pixel[channel] = (u16::from(pixel[channel]) * u16::from(pixel[3]) / 255) as u8;
                    }
                }
                let blurred = image::imageops::blur(&thumbnail, 11.58);
                let wash = image::RgbaImage::from_fn(180, 54, |x, y| {
                    let pixel = blurred.get_pixel(x.saturating_sub(2), y + 21);
                    let alpha = if x < 2 { 0.0 } else { f32::from(pixel[3]) / 255.0 };
                    let rgb = [pixel[0], pixel[1], pixel[2]].map(|value| f32::from(value) / alpha.max(0.001));
                    let luminance = 0.213 * rgb[0] + 0.715 * rgb[1] + 0.072 * rgb[2];
                    let shade = 0.46 + 0.24 * y as f32 / 53.0;
                    let mut result = [0, 0, 0, 255];
                    for channel in 0..3 {
                        let saturated = (luminance + 1.2 * (rgb[channel] - luminance)).clamp(0.0, 255.0);
                        let behind = if channel == 2 { 16.0 } else { 13.0 };
                        let veil = if channel == 2 { 13.0 } else { 9.0 };
                        result[channel] =
                            ((saturated * alpha + behind * (1.0 - alpha)) * (1.0 - shade) + veil * shade).round() as u8;
                    }
                    image::Rgba(result)
                });
                Ok(iced::widget::image::Handle::from_rgba(180, 54, wash.into_raw()))
            }),
            move |result| {
                Message::Remote(Remote::WashReady(
                    window_id,
                    serial,
                    result.unwrap_or_else(|| Err("remote cover worker stopped".to_owned())),
                ))
            },
        );
        Task::batch([queue_layout, art])
    }

    pub(crate) fn advance_remote_queue(&mut self) -> Task<Message> {
        let Some(expansion) = self.remote.expansion else {
            return Task::none();
        };
        self.remote
            .queue
            .frame(
                self.playback.queue.items(),
                self.playback.queue.current_index(),
                (expansion.height - 52.0).max(0.0),
                Instant::now(),
            )
            .map(|message| Message::Remote(Remote::QueueList(message)))
    }

    fn sync_remote_shape(&mut self) {
        let Some(window) = self.remote.raw_id.and_then(backend::native_window) else {
            return;
        };
        let idle = self.remote.idle.value(self.remote.frame_time);
        let sleeve = if self.remote.settings.appearance.expand_cover { idle } else { 0.0 };
        let left = (178.0 - 174.0 * sleeve).floor();
        let [width, height] = self.remote.settings.size;
        let expansion = self.remote.expansion;
        let top = 4.0 + expansion.filter(|value| value.up).map_or(0.0, |value| value.height);
        let card = iced::Rectangle::new(Point::new(left, top), Size::new(width - left - 4.0, height - 8.0));
        let queue = iced::Rectangle::new(
            Point::new(178.0, if expansion.is_some_and(|value| value.up) { 4.0 } else { height + 6.0 }),
            Size::new(width - 182.0, expansion.map_or(0.0, |value| value.height - 6.0)),
        );
        let regions = [card, queue];
        let regions = &regions[..if expansion.is_some() { 2 } else { 1 }];
        let scale = window.scale_factor();
        if self.remote.shape != regions || self.remote.shape_scale != scale {
            if let Err(error) = desktop_window::set_shape(&window, regions, !self.remote.settings.click_through) {
                self.remote_error(error);
            }
            self.remote.shape.clear();
            self.remote.shape.extend_from_slice(regions);
            self.remote.shape_scale = scale;
        }
    }
}
