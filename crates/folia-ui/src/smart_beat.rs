// SmartBeat preferences, generation-filtered analysis messages and live settings controls.
use std::path::PathBuf;
use std::sync::Arc;

use folia_analysis::{
    AnalysisData,
    worker::{Event, Needs, Request, Update, Worker},
};
use folia_core::{BeatEvent, DrumHits, SmartBeat, StemAnalysis};
use folia_store::{
    BeatEffectSetting, NEBBIA_AQUA_BEAT, NEBBIA_DRIZZLE_RIDE, SMART_BEAT_KEY, SmartBeatMode, SmartBeatSettings,
    VisualizerSettings,
};
use iced::Task;

#[derive(Clone, Copy, Debug)]
pub enum Change {
    Mode(SmartBeatMode),
    Enabled(&'static str, bool),
    Strength(&'static str, &'static str, f32),
    Reset,
}

impl Change {
    pub fn commits(self) -> bool {
        !matches!(self, Self::Strength(..))
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Updated(Update),
    Stopped,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Status {
    #[default]
    Idle,
    Analysing(folia_analysis::models::ModelKind, f64),
    Ready,
    CachedMiss,
    Failed,
    MissingModel,
    MissingRuntime,
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub settings: &'a SmartBeatSettings,
    pub status: Status,
    pub sample: SmartBeat<'a>,
}

#[derive(Clone, Default)]
pub struct Snapshot {
    beats: Option<Arc<Vec<BeatEvent>>>,
    drums: Option<Arc<DrumHits>>,
    vocal: Option<Arc<folia_core::VocalEnvelope>>,
    heard: f64,
}

impl Snapshot {
    pub fn capture(sample: SmartBeat<'_>) -> Self {
        Self {
            beats: sample.beats.map(|events| Arc::new(events.to_vec())),
            drums: sample.drums.map(|hits| Arc::new(hits.clone())),
            vocal: sample.vocal.map(|envelope| Arc::new(envelope.clone())),
            heard: sample.heard,
        }
    }

    pub fn frame(&self, settings: &SmartBeatSettings) -> SmartBeat<'_> {
        if !settings.mode.active() {
            return SmartBeat { heard: self.heard, ..Default::default() };
        }
        let drizzle = settings.effect(NEBBIA_DRIZZLE_RIDE);
        let aqua = settings.effect(NEBBIA_AQUA_BEAT);
        SmartBeat {
            heard: self.heard,
            beats: self.beats.as_deref().filter(|beats| aqua.on && !beats.is_empty()).map(Vec::as_slice),
            drums: self.drums.as_deref().filter(|_| drizzle.on),
            vocal: self.vocal.as_deref().filter(|_| drizzle.on),
            rain_strength: if drizzle.on { drizzle.part("rain") } else { 1.0 },
            bump_strength: if drizzle.on { drizzle.part("bump") } else { 1.0 },
            aqua_strength: if aqua.on {
                [aqua.part("jelly"), aqua.part("weed"), aqua.part("bubble"), aqua.part("spot")]
            } else {
                [0.0; 4]
            },
        }
    }
}

impl Default for View<'_> {
    fn default() -> Self {
        static DEFAULT: SmartBeatSettings =
            SmartBeatSettings { mode: SmartBeatMode::Off, effects: std::collections::BTreeMap::new() };
        Self { settings: &DEFAULT, status: Status::Idle, sample: SmartBeat::default() }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Active {
    playback: u64,
    mode: SmartBeatMode,
    needs: Needs,
}

pub struct State {
    settings: SmartBeatSettings,
    stored: Option<toml::Value>,
    models: PathBuf,
    database: PathBuf,
    worker: Option<Worker>,
    active: Option<Active>,
    ticket: u64,
    beats: Option<Arc<Vec<BeatEvent>>>,
    stems: Option<Arc<StemAnalysis>>,
    partial: Option<Arc<DrumHits>>,
    status: Status,
}

impl State {
    pub fn new(visualizer: &VisualizerSettings, models: PathBuf, database: PathBuf) -> Self {
        let stored = visualizer.tables.get(SMART_BEAT_KEY).cloned();
        let settings = SmartBeatSettings::read(stored.as_ref());
        Self {
            settings,
            stored,
            models,
            database,
            worker: None,
            active: None,
            ticket: 0,
            beats: None,
            stems: None,
            partial: None,
            status: Status::Idle,
        }
    }

    pub fn change(&mut self, change: Change, visualizer: &mut VisualizerSettings) {
        match change {
            Change::Mode(mode) => self.settings.mode = mode,
            Change::Enabled(effect, on) => self.settings.set_enabled(effect, on),
            Change::Strength(effect, part, value) => self.settings.set_part(effect, part, value),
            Change::Reset => self.settings = SmartBeatSettings::default(),
        }
        let table = toml::Value::Table(self.settings.table());
        visualizer.tables.insert(SMART_BEAT_KEY.to_owned(), table.clone());
        self.stored = Some(table);
    }

    pub fn reload(&mut self, visualizer: &VisualizerSettings) {
        let stored = visualizer.tables.get(SMART_BEAT_KEY);
        if stored != self.stored.as_ref() {
            self.settings = SmartBeatSettings::read(stored);
            self.stored = stored.cloned();
        }
    }

    pub fn active(&self) -> bool {
        self.settings.mode.active()
    }

    pub fn needs(&self, sources: folia_core::AnalysisSources) -> Needs {
        Needs {
            beats: sources.beats && self.settings.effect(NEBBIA_AQUA_BEAT).on,
            stems: sources.stems && self.settings.effect(NEBBIA_DRIZZLE_RIDE).on,
        }
    }

    pub fn cancel(&mut self) {
        if self.active.take().is_none() {
            return;
        }
        self.ticket = self.ticket.wrapping_add(1);
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
        self.beats = None;
        self.stems = None;
        self.partial = None;
        self.status = Status::Idle;
    }

    pub fn sync(&mut self, playback: &crate::Playback, needs: Needs) -> Task<crate::Message> {
        if !self.settings.mode.active() || !needs.any() {
            self.cancel();
            return Task::none();
        }
        let active = Active { playback: playback.request_id(), mode: self.settings.mode, needs };
        if self.active == Some(active) {
            return Task::none();
        }
        self.cancel();
        let Some(input) = playback.analysis_input() else { return Task::none() };
        let local_file = playback.current().is_some_and(|track| track.online.is_none());
        self.ticket = self.ticket.wrapping_add(1);
        self.active = Some(active);
        let mut stream = Task::none();
        if self.worker.is_none() {
            match Worker::spawn(self.models.clone(), self.database.clone()) {
                Ok((worker, receiver)) => {
                    self.worker = Some(worker);
                    let updates = iced::futures::stream::unfold(Some(receiver), |receiver| async move {
                        let receiver = receiver?;
                        crate::background::blocking(move || match receiver.recv() {
                            Ok(message) => (Message::Updated(message), Some(receiver)),
                            Err(_) => (Message::Stopped, None),
                        })
                        .await
                    });
                    stream = Task::run(updates, crate::Message::SmartBeat);
                }
                Err(error) => {
                    tracing::error!(error = &error as &dyn std::error::Error, "cannot start music analysis");
                    self.status = Status::Failed;
                    return Task::none();
                }
            }
        }
        if let Some(worker) = &mut self.worker
            && let Err(error) = worker.submit(Request {
                ticket: self.ticket,
                input,
                local_file,
                needs,
                cached_only: !self.settings.mode.may_analyse(),
            })
        {
            tracing::error!(error = &error as &dyn std::error::Error, "cannot request music analysis");
            self.status = Status::Failed;
        }
        stream
    }

    pub fn update(&mut self, message: Message) {
        let Message::Updated(update) = message else {
            self.worker = None;
            if self.active.is_some() {
                self.status = Status::Failed;
            }
            return;
        };
        if update.ticket != self.ticket || self.active.is_none() {
            return;
        }
        match update.event {
            Event::Ready(AnalysisData::Beats(data)) => self.beats = Some(data),
            Event::Ready(AnalysisData::Stems(data)) => {
                self.stems = Some(data);
                self.partial = None;
            }
            Event::PartialDrums(data) => self.partial = Some(data),
            Event::Analysing(model) => self.status = Status::Analysing(model, 0.0),
            Event::Progress(seconds) => {
                if let Status::Analysing(model, _) = self.status {
                    self.status = Status::Analysing(model, seconds);
                }
            }
            Event::Finished => {
                self.status =
                    if self.beats.is_some() || self.stems.is_some() { Status::Ready } else { Status::CachedMiss }
            }
            Event::Failed(error) => {
                self.status = match error.as_ref() {
                    folia_analysis::AnalysisError::Runtime { .. } => Status::MissingRuntime,
                    folia_analysis::AnalysisError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Status::MissingModel
                    }
                    _ => Status::Failed,
                }
            }
        }
    }

    pub fn view(&self, heard: f64) -> View<'_> {
        View { settings: &self.settings, status: self.status, sample: self.frame(heard) }
    }

    pub fn frame(&self, heard: f64) -> SmartBeat<'_> {
        let Some(active) = self.active.filter(|_| self.settings.mode.active()) else {
            return SmartBeat { heard, ..Default::default() };
        };
        let drizzle = self.settings.effect(NEBBIA_DRIZZLE_RIDE);
        let aqua = self.settings.effect(NEBBIA_AQUA_BEAT);
        let stems = self.stems.as_deref().filter(|_| active.needs.stems && drizzle.on);
        SmartBeat {
            heard,
            beats: self
                .beats
                .as_deref()
                .filter(|beats| active.needs.beats && aqua.on && !beats.is_empty())
                .map(Vec::as_slice),
            drums: stems
                .and_then(|value| value.drums.as_ref())
                .or_else(|| self.partial.as_deref().filter(|_| active.needs.stems && drizzle.on)),
            vocal: stems.map(|value| &value.vocal).filter(|value| !value.db.is_empty()),
            rain_strength: if active.needs.stems && drizzle.on { drizzle.part("rain") } else { 1.0 },
            bump_strength: if active.needs.stems && drizzle.on { drizzle.part("bump") } else { 1.0 },
            aqua_strength: if active.needs.beats && aqua.on {
                [aqua.part("jelly"), aqua.part("weed"), aqua.part("bubble"), aqua.part("spot")]
            } else {
                [0.0; 4]
            },
        }
    }
}

impl crate::Shell {
    pub(crate) fn sync_smart_beat(&mut self, frame_tick: bool) -> Task<crate::Message> {
        self.smart_beat.reload(&self.visualizer);
        let visible = self.page == crate::Page::NowPlaying
            || self.workbench.as_ref().is_some_and(crate::workbench::Workbench::uses_live_audio)
            || self.theme_park.as_ref().is_some_and(|park| park.preview.uses_live_audio());
        if !self.smart_beat.active() || !visible {
            self.smart_beat.cancel();
            return Task::none();
        }
        if frame_tick
            && self.smart_beat.active.is_some_and(|active| active.playback == self.playback.request_id())
            && matches!(
                self.playback.status.state,
                folia_player::PlayerState::Playing | folia_player::PlayerState::Paused
            )
        {
            return Task::none();
        }
        let sources = self.style().analysis_sources(&self.visualizer.tables);
        let needs = self.smart_beat.needs(sources);
        self.smart_beat.sync(&self.playback, needs)
    }
}

pub(crate) fn global_page<'a>(
    i18n: &'a folia_i18n::I18n,
    tokens: crate::settings_kit::Tokens,
    view: View<'a>,
    notes: &crate::workbench_notes::Notes,
) -> iced::Element<'a, crate::Message> {
    use crate::settings_kit::{self as kit, Group, Row};
    use iced::widget::{column, space};
    let settings = view.settings;
    let key = match settings.mode {
        SmartBeatMode::Off => "options.smartBeatDesc_off",
        SmartBeatMode::Cached => "options.smartBeatDesc_cached",
        SmartBeatMode::On => "options.smartBeatDesc_on",
    };
    let choices = SmartBeatMode::ALL
        .into_iter()
        .zip(["options.smartBeatOff", "options.smartBeatCached", "options.smartBeatOn"])
        .map(|(value, key)| (value, i18n.t(key).to_owned()))
        .collect();
    let mut rows = vec![Row {
        note: Some(notes.view(i18n, tokens, "options.smartBeatMode", i18n.t(key).to_owned())),
        label: i18n.t("options.smartBeatMode").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: kit::segmented(tokens, choices, settings.mode, None, |mode| {
            crate::Message::Workbench(crate::workbench::Message::SmartBeat(Change::Mode(mode)))
        }),
    }];
    if settings.mode.active()
        && let Some(status) = status_text(i18n, view.status)
    {
        rows.push(kit::notice_row(tokens, &status, space().into()));
    }
    column![kit::group(
        tokens,
        Group { title: i18n.t("options.smartBeat").to_owned(), bare: false, rows, foot: None },
        true
    )]
    .padding(iced::Padding { left: 24.0, right: 12.0, bottom: 40.0, top: 0.0 })
    .into()
}

pub(crate) fn effects_page<'a>(
    i18n: &'a folia_i18n::I18n,
    tokens: crate::settings_kit::Tokens,
    style: folia_visualizer::Style,
    view: View<'a>,
    notes: &crate::workbench_notes::Notes,
) -> Option<iced::Element<'a, crate::Message>> {
    use crate::settings_kit::{self as kit, Group, Row};
    use iced::widget::column;
    if style != folia_visualizer::Style::Nebbia {
        return None;
    }
    let settings = view.settings;
    let mut rows = Vec::new();
    if !settings.mode.active() {
        rows.push(kit::notice_row(
            tokens,
            i18n.t("options.beatEffectsNeedSmartBeat"),
            kit::sheet_button(
                tokens,
                i18n.t("options.beatEffectsOpenSmartBeat"),
                false,
                Some(crate::Message::Workbench(crate::workbench::Message::Tab(crate::workbench::Tab::Global))),
            ),
        ));
    }
    for (id, name, description, parts) in [
        (
            NEBBIA_DRIZZLE_RIDE,
            "options.beatEffect_nebbiaDrizzleRide",
            "options.beatEffect_nebbiaDrizzleRideDesc",
            &[("rain", "options.beatEffectPart_nebbiaRain"), ("bump", "options.beatEffectPart_nebbiaBump")][..],
        ),
        (
            NEBBIA_AQUA_BEAT,
            "options.beatEffect_nebbiaAquaBeat",
            "options.beatEffect_nebbiaAquaBeatDesc",
            &[
                ("jelly", "options.beatEffectPart_nebbiaJelly"),
                ("weed", "options.beatEffectPart_nebbiaWeed"),
                ("bubble", "options.beatEffectPart_nebbiaBubble"),
                ("spot", "options.beatEffectPart_nebbiaSpot"),
            ][..],
        ),
    ] {
        let effect = settings.effect(id);
        rows.push(Row {
            note: Some(notes.view(i18n, tokens, description, i18n.t(description).to_owned())),
            label: i18n.t(name).to_owned(),
            hint: None,
            sub: false,
            below: None,
            control: kit::toggle(
                tokens,
                f32::from(effect.on),
                crate::Message::Workbench(crate::workbench::Message::SmartBeat(Change::Enabled(id, !effect.on))),
            ),
        });
        if settings.mode.active() && effect.on {
            rows.extend(parts.iter().map(|(part, label)| strength_row(i18n, tokens, id, part, label, effect)));
        }
    }
    Some(
        column![kit::group(
            tokens,
            Group { title: i18n.t("options.beatEffects").to_owned(), bare: false, rows, foot: None },
            true
        )]
        .padding(iced::Padding { left: 24.0, right: 12.0, bottom: 0.0, top: 0.0 })
        .into(),
    )
}

fn strength_row<'a>(
    i18n: &folia_i18n::I18n,
    tokens: crate::settings_kit::Tokens,
    effect: &'static str,
    part: &'static str,
    label: &str,
    setting: &BeatEffectSetting,
) -> crate::settings_kit::Row<'a, crate::Message> {
    use crate::settings_kit::{self as kit, Row};
    use iced::widget::slider;
    let value = setting.part(part);
    let control = slider(0.0..=2.0, value, move |value| {
        crate::Message::Workbench(crate::workbench::Message::SmartBeat(Change::Strength(effect, part, value)))
    })
    .step(0.05_f32)
    .on_release(crate::Message::Workbench(crate::workbench::Message::Commit))
    .width(iced::Length::Fill)
    .height(32)
    .style(move |_, _| kit::slider_style(tokens));
    Row {
        note: None,
        label: i18n.t(label).to_owned(),
        hint: None,
        sub: true,
        below: Some(control.into()),
        control: kit::value(tokens, format!("{value:.2}x")),
    }
}

fn status_text(i18n: &folia_i18n::I18n, status: Status) -> Option<String> {
    match status {
        Status::Idle => None,
        Status::Analysing(model, seconds) => Some(i18n.format(
            "options.smartBeatAnalysing",
            &[
                (
                    "model",
                    match model {
                        folia_analysis::models::ModelKind::BeatThis => "Beat This!",
                        folia_analysis::models::ModelKind::Htdemucs => "HTDemucs",
                    },
                ),
                ("seconds", &format!("{seconds:.0}")),
            ],
        )),
        Status::Ready => Some(i18n.t("options.smartBeatReady").to_owned()),
        Status::CachedMiss => Some(i18n.t("options.smartBeatCachedMiss").to_owned()),
        Status::Failed => Some(i18n.t("options.smartBeatFailed").to_owned()),
        Status::MissingModel => Some(i18n.t("options.smartBeatMissingModel").to_owned()),
        Status::MissingRuntime => Some(i18n.t("options.smartBeatMissingRuntime").to_owned()),
    }
}
