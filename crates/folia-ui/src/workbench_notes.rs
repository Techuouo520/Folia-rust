// Information notes in the workbench, animated by the shell's shared UI frame clock.
use std::collections::BTreeMap;
use std::time::Instant;

use iced::widget::{button, container, tooltip};
use iced::{Length, Padding};
use lucide_static_svg::Icon;

use crate::settings_kit::{self as kit, Note, Tokens};
use crate::workbench::Message;

pub(crate) struct Notes {
    entries: BTreeMap<&'static str, Entry>,
    sampled: Instant,
}

struct Entry {
    height: f32,
    progress: crate::motion::Tween,
}

impl Default for Notes {
    fn default() -> Self {
        Self { entries: BTreeMap::new(), sampled: Instant::now() }
    }
}

impl Notes {
    fn entry(&mut self, key: &'static str) -> &mut Entry {
        self.entries.entry(key).or_insert_with(|| Entry {
            height: 0.0,
            progress: crate::motion::Tween::new(
                0.0,
                300.0,
                folia_visualizer::Ease::new(0.22, 1.0, 0.36, 1.0),
                self.sampled,
            ),
        })
    }

    pub fn toggle(&mut self, key: &'static str) {
        self.sampled = Instant::now();
        let now = self.sampled;
        let entry = self.entry(key);
        entry.progress.set(1.0 - entry.progress.goal(), now);
    }

    pub fn measure(&mut self, key: &'static str, height: f32) {
        self.entry(key).height = height;
    }

    pub fn frame(&mut self) {
        self.sampled = Instant::now();
    }

    pub fn sampled(&self) -> Instant {
        self.sampled
    }

    pub fn animating(&self) -> bool {
        self.entries.values().any(|entry| entry.progress.running(self.sampled))
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn view<'a>(
        &self,
        i18n: &'a folia_i18n::I18n,
        tokens: Tokens,
        key: &'static str,
        description: String,
    ) -> Note<'a, crate::Message> {
        let entry = self.entries.get(key);
        let height = entry.map_or(0.0, |entry| entry.height);
        let progress = entry.map_or(0.0, |entry| entry.progress.value(self.sampled));
        let open = entry.is_some_and(|entry| entry.progress.goal() > 0.0);
        let trigger = |ink| {
            button(container(crate::icons::icon(Icon::Info, 15.0, ink)).center(18))
                .padding(0)
                .on_press(crate::Message::Workbench(Message::ToggleDetails(key)))
                .style(|_, _| button::Style::default())
                .into()
        };
        let trigger = kit::hover(trigger(if open { tokens.t1 } else { tokens.t3 }), trigger(tokens.t1));
        let trigger = tooltip(
            trigger,
            crate::typeface::HINT.text(i18n.t("options.settingsRowDetails")),
            tooltip::Position::Bottom,
        )
        .delay(std::time::Duration::from_millis(400))
        .style(move |_| container::Style { text_color: Some(tokens.t1), ..kit::rounded(tokens.menu, 6.0) });
        let text = container(
            crate::typeface::HINT
                .text(description)
                .line_height(iced::widget::text::LineHeight::Absolute(19.0.into()))
                .color(tokens.t2),
        )
        .width(Length::Fill)
        .padding(Padding::ZERO.top(6).bottom(2));
        let content = crate::measured::measured(text, height, move |height| {
            crate::Message::Workbench(Message::DetailsMeasured(key, height))
        });
        Note {
            trigger: trigger.into(),
            content: container(content).height(height * progress).width(Length::Fill).clip(true).into(),
        }
    }
}
