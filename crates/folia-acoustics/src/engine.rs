// One native simulation worker owns all SDK handles; bounded requests and cancelled replies never block the UI.

use crate::{Error, Simulator, cache};
use folia_core::{RoomImpulseResponse, RoomListener, RoomRequest};
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};

pub fn library_path() -> Result<PathBuf, Error> {
    let executable = std::env::current_exe()?;
    let root = executable.parent().ok_or(Error::Invalid("executable has no directory"))?;
    let file = if cfg!(target_os = "windows") {
        "phonon.dll"
    } else if cfg!(target_os = "macos") {
        "libphonon.dylib"
    } else {
        "libphonon.so"
    };
    Ok(root.join("assets").join("acoustics").join(file))
}

enum Job {
    CachePath(PathBuf, oneshot::Sender<()>),
    Probe(oneshot::Sender<Result<(), Error>>),
    Compute {
        request: RoomRequest,
        reference: RoomListener,
        reply: oneshot::Sender<Result<RoomImpulseResponse, Error>>,
    },
}

#[derive(Clone)]
pub struct Engine {
    sender: mpsc::Sender<Job>,
}

impl Engine {
    pub fn new(library: PathBuf, mut database: PathBuf) -> Result<Self, Error> {
        let (sender, mut receiver) = mpsc::channel::<Job>(2);
        std::thread::Builder::new().name("folia-room-sound".into()).spawn(move || {
            let mut cache = None;
            while let Some(job) = receiver.blocking_recv() {
                let cancelled = match &job {
                    Job::CachePath(_, reply) => reply.is_closed(),
                    Job::Probe(reply) => reply.is_closed(),
                    Job::Compute { reply, .. } => reply.is_closed(),
                };
                if cancelled {
                    continue;
                }
                match job {
                    Job::CachePath(path, reply) => {
                        cache = None;
                        database = path;
                        let _ = reply.send(());
                    }
                    Job::Probe(reply) => {
                        let _ = reply.send(Simulator::open(&library).map(drop));
                    }
                    Job::Compute { request, reference, reply } => {
                        if cache.is_none() {
                            match cache::Cache::open(&database) {
                                Ok(value) => cache = Some(value),
                                Err(error) => {
                                    tracing::warn!(error = &error as &dyn std::error::Error, "room cache unavailable")
                                }
                            }
                        }
                        let result = calibrated(&library, cache.as_ref(), request, reference, &reply);
                        let _ = reply.send(result);
                    }
                }
            }
        })?;
        Ok(Self { sender })
    }

    pub async fn available(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Job::Probe(sender)).await.map_err(|_| Error::Worker)?;
        receiver.await.map_err(|_| Error::Worker)?
    }

    /// Releases the SQLite connection before acknowledging a cache location change.
    pub async fn set_cache_path(&self, path: PathBuf) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(Job::CachePath(path, sender)).await.map_err(|_| Error::Worker)?;
        receiver.await.map_err(|_| Error::Worker)
    }

    pub async fn render(&self, request: RoomRequest, reference: RoomListener) -> Result<RoomImpulseResponse, Error> {
        let (reply, receiver) = oneshot::channel();
        self.sender.send(Job::Compute { request, reference, reply }).await.map_err(|_| Error::Worker)?;
        receiver.await.map_err(|_| Error::Worker)?
    }
}

fn calibrated(
    library: &Path,
    cache: Option<&cache::Cache>,
    request: RoomRequest,
    reference: RoomListener,
    reply: &oneshot::Sender<Result<RoomImpulseResponse, Error>>,
) -> Result<RoomImpulseResponse, Error> {
    let mut simulator = None;
    let baseline = RoomRequest { listener: reference, ..request.clone() };
    let mut impulse = load(&mut simulator, library, cache, &baseline)?;
    if reply.is_closed() {
        return Err(Error::Worker);
    }
    let gain = crate::loudness_gain(&impulse.channels, request.sample_rate);
    if request.listener != reference {
        impulse = load(&mut simulator, library, cache, &request)?;
    }
    impulse.gain = gain;
    Ok(impulse)
}

fn load(
    simulator: &mut Option<Simulator>,
    library: &Path,
    cache: Option<&cache::Cache>,
    request: &RoomRequest,
) -> Result<RoomImpulseResponse, Error> {
    let key = cache::key(request);
    if let Some(cache) = cache {
        match cache.get(&key) {
            Ok(Some(impulse)) => return Ok(impulse),
            Ok(None) => {}
            Err(error) => tracing::warn!(error = &error as &dyn std::error::Error, "room cache entry unavailable"),
        }
    }
    if simulator.is_none() {
        *simulator = Some(Simulator::open(library)?);
    }
    let Some(simulator) = simulator.as_mut() else {
        return Err(Error::Worker);
    };
    let impulse = simulator.compute(request)?;
    if let Some(cache) = cache
        && let Err(error) = cache.put(&key, &impulse)
    {
        tracing::warn!(error = &error as &dyn std::error::Error, "room response not cached");
    }
    Ok(impulse)
}
