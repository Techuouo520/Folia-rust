// Serializes source mutations while playback takes only a short-lived snapshot of the active worker.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use folia_store::Store;

use super::{CustomSource, Limits, Script, SourceError, SourceSettings, UpdateAlert, parse_metadata};

#[derive(Clone)]
struct Config {
    store: Store,
    directory: PathBuf,
}

#[derive(Default)]
struct State {
    settings: SourceSettings,
    active: Vec<(String, Script)>,
    alerts: Vec<(String, UpdateAlert)>,
}

#[derive(Default)]
pub(crate) struct Sources {
    mutations: Mutex<()>,
    config: Mutex<Option<Config>>,
    state: Mutex<State>,
}

impl Sources {
    pub fn restore(&self, store: Store, data: PathBuf, client: reqwest::Client) -> Result<SourceSettings, SourceError> {
        let _operation = self.mutations.lock().unwrap_or_else(PoisonError::into_inner);
        let config = Config { store, directory: data.join("sources") };
        *self.config.lock().unwrap_or_else(PoisonError::into_inner) = Some(config.clone());
        for (_, script) in self.state.lock().unwrap_or_else(PoisonError::into_inner).active.drain(..) {
            script.stop();
        }
        let settings = SourceSettings::load(&config.store)?;
        if settings.scripts.len() > 32
            || settings.scripts.iter().any(|s| !valid_id(&s.id))
            || settings.enabled.iter().any(|id| !settings.scripts.iter().any(|s| &s.id == id))
        {
            return Err(SourceError::Protocol);
        }
        self.state.lock().unwrap_or_else(PoisonError::into_inner).settings = settings.clone();
        let mut failure = None;
        for source in settings.scripts.iter().filter(|s| settings.enabled.contains(&s.id)) {
            match start(&config, source, client.clone()) {
                Ok(script) => {
                    self.state.lock().unwrap_or_else(PoisonError::into_inner).active.push((source.id.clone(), script))
                }
                Err(err) => {
                    tracing::warn!(
                        source = source.id,
                        error = &err as &dyn std::error::Error,
                        "cannot restore custom source; continuing with others"
                    );
                    failure.get_or_insert(err);
                }
            }
        }
        if let Some(err) = failure { Err(err) } else { Ok(settings) }
    }

    pub fn list(&self) -> SourceSettings {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).settings.clone()
    }

    pub fn active(&self, platform: &str) -> Vec<(String, Script)> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .settings
            .scripts
            .iter()
            .filter_map(|source| {
                let (id, script) = state.active.iter().find(|(id, _)| id == &source.id)?;
                let supported =
                    script.sources().get(platform).is_some_and(|s| s.actions.iter().any(|a| a == "musicUrl"));
                if !supported {
                    tracing::debug!(
                        source = id,
                        platform,
                        "custom source does not support musicUrl on this platform; skipping"
                    );
                    return None;
                }
                Some((id.clone(), script.clone()))
            })
            .collect()
    }

    pub fn import(&self, path: &Path, client: reqwest::Client) -> Result<SourceSettings, SourceError> {
        self.import_text(read(path)?, client)
    }

    pub fn import_text(&self, code: String, client: reqwest::Client) -> Result<SourceSettings, SourceError> {
        if code.len() > super::SCRIPT_BYTES {
            return Err(SourceError::Limit);
        }
        let _operation = self.mutations.lock().unwrap_or_else(PoisonError::into_inner);
        let config = self.config()?;
        let id = identifier(&code);
        let mut settings = self.list();
        if settings.scripts.iter().any(|s| s.id == id) {
            return Err(SourceError::Duplicate);
        }
        if settings.scripts.len() >= 32 {
            return Err(SourceError::Limit);
        }
        let metadata = parse_metadata(&code)?;
        let script = Script::start(code.clone(), metadata.clone(), client, Limits::default())?;
        let source = CustomSource { id: id.clone(), metadata, sources: script.sources().clone() };
        std::fs::create_dir_all(&config.directory)?;
        let file = config.directory.join(format!("{id}.js"));
        let temporary = config.directory.join(format!("{id}.tmp"));
        std::fs::write(&temporary, code)?;
        std::fs::rename(&temporary, &file)?;
        settings.scripts.push(source);
        if let Err(err) = config.store.save(&settings) {
            if let Err(cleanup) = std::fs::remove_file(&file) {
                tracing::warn!(error = &cleanup as &dyn std::error::Error, "cannot remove uncommitted source");
            }
            return Err(err.into());
        }
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.settings = settings.clone();
        state.alerts.extend(script.take_update_alerts().into_iter().map(|a| (id.clone(), a)));
        tracing::info!(source = id, "custom source imported");
        Ok(settings)
    }

    pub fn enable(&self, id: &str, enabled: bool, client: reqwest::Client) -> Result<SourceSettings, SourceError> {
        let _operation = self.mutations.lock().unwrap_or_else(PoisonError::into_inner);
        let config = self.config()?;
        let mut settings = self.list();
        let source = settings.scripts.iter_mut().find(|s| s.id == id).ok_or(SourceError::NotFound)?;
        let active = if enabled {
            let script = start(&config, source, client)?;
            source.sources = script.sources().clone();
            Some(script)
        } else {
            None
        };
        settings.enabled.retain(|active| active != id);
        if enabled {
            settings.enabled.push(id.to_owned());
        }
        config.store.save(&settings)?;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.settings = settings.clone();
        if let Some(index) = state.active.iter().position(|(active, _)| active == id) {
            let (_, script) = state.active.remove(index);
            script.stop();
        }
        if let Some(script) = active {
            state.active.push((id.to_owned(), script));
        }
        tracing::info!(source = id, enabled, "custom source activation changed");
        Ok(settings)
    }

    pub fn move_to(&self, id: &str, index: usize) -> Result<SourceSettings, SourceError> {
        let _operation = self.mutations.lock().unwrap_or_else(PoisonError::into_inner);
        let config = self.config()?;
        let mut settings = self.list();
        let previous = settings.scripts.iter().position(|source| source.id == id).ok_or(SourceError::NotFound)?;
        if index >= settings.scripts.len() {
            return Err(SourceError::Protocol);
        }
        let source = settings.scripts.remove(previous);
        settings.scripts.insert(index, source);
        config.store.save(&settings)?;
        self.state.lock().unwrap_or_else(PoisonError::into_inner).settings = settings.clone();
        tracing::info!(source = id, index, "custom source priority changed");
        Ok(settings)
    }

    pub fn remove(&self, id: &str) -> Result<SourceSettings, SourceError> {
        let _operation = self.mutations.lock().unwrap_or_else(PoisonError::into_inner);
        let config = self.config()?;
        let mut settings = self.list();
        let index = settings.scripts.iter().position(|s| s.id == id).ok_or(SourceError::NotFound)?;
        settings.scripts.remove(index);
        settings.enabled.retain(|active| active != id);
        config.store.save(&settings)?;
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.settings = settings.clone();
            if let Some(index) = state.active.iter().position(|(active, _)| active == id) {
                let (_, script) = state.active.remove(index);
                script.stop();
            }
            state.alerts.retain(|(source, _)| source != id);
        }
        match std::fs::remove_file(config.directory.join(format!("{id}.js"))) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        tracing::info!(source = id, "custom source removed");
        Ok(settings)
    }

    pub fn take_alerts(&self) -> Vec<(String, UpdateAlert)> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut alerts = std::mem::take(&mut state.alerts);
        for (id, script) in &state.active {
            alerts.extend(script.take_update_alerts().into_iter().map(|a| (id.clone(), a)));
        }
        alerts
    }

    fn config(&self) -> Result<Config, SourceError> {
        self.config.lock().unwrap_or_else(PoisonError::into_inner).clone().ok_or(SourceError::NotConfigured)
    }
}

fn start(config: &Config, source: &CustomSource, client: reqwest::Client) -> Result<Script, SourceError> {
    let code = read(&config.directory.join(format!("{}.js", source.id)))?;
    if identifier(&code) != source.id {
        return Err(SourceError::Protocol);
    }
    Script::start(code, source.metadata.clone(), client, Limits::default())
}

fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn identifier(code: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, code.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn read(path: &Path) -> Result<String, SourceError> {
    let mut code = String::new();
    std::fs::File::open(path)?.take(super::SCRIPT_BYTES as u64 + 1).read_to_string(&mut code)?;
    if code.len() > super::SCRIPT_BYTES {
        return Err(SourceError::Limit);
    }
    Ok(code)
}
