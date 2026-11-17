// One cancellable analysis worker keeps playback I/O, model construction, GPU jobs and cache writes off the UI thread.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use folia_core::{AudioInput, BeatEvent, DrumHits, StemAnalysis};

use crate::audio::AudioReader;
use crate::beat_this::{BeatThis, MAX_FRAMES};
use crate::cache::Cache;
use crate::gpu::Gpu;
use crate::htdemucs::{Htdemucs, SEGMENT};
use crate::models::{ModelKind, Weights};
use crate::signal::{MelStream, StemFeatures};
use crate::{AnalysisData, AnalysisError};

pub use folia_core::AnalysisSources as Needs;

pub struct Request {
    pub ticket: u64,
    pub input: AudioInput,
    pub local_file: bool,
    pub needs: Needs,
    pub cached_only: bool,
}

#[derive(Clone, Debug)]
pub struct Update {
    pub ticket: u64,
    pub event: Event,
}

#[derive(Clone, Debug)]
pub enum Event {
    Ready(AnalysisData),
    PartialDrums(Arc<DrumHits>),
    Analysing(ModelKind),
    Progress(f64),
    Finished,
    Failed(Arc<AnalysisError>),
}

enum Command {
    Run(Request, Arc<AtomicBool>),
    Release,
    Stop,
}

pub struct Worker {
    sender: mpsc::Sender<Command>,
    current: Option<Arc<AtomicBool>>,
}

impl Worker {
    pub fn spawn(models: PathBuf, database: PathBuf) -> Result<(Self, mpsc::Receiver<Update>), AnalysisError> {
        let (sender, receiver) = mpsc::channel();
        let (updates, results) = mpsc::channel();
        std::thread::Builder::new().name("folia-analysis".to_owned()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(error = &error as &dyn std::error::Error, "analysis runtime failed");
                    return;
                }
            };
            let mut models = Models::new(models);
            loop {
                let received = match models.idle_wait() {
                    Some(wait) => receiver.recv_timeout(wait),
                    None => receiver.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                };
                let mut command = match received {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        models.trim();
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                for latest in receiver.try_iter() {
                    command = latest;
                }
                models.trim();
                match command {
                    Command::Stop => break,
                    Command::Release => models.release(),
                    Command::Run(request, cancelled) => {
                        if cancelled.load(Ordering::Acquire) {
                            continue;
                        }
                        let ticket = request.ticket;
                        let publish = |event| updates.send(Update { ticket, event }).is_ok();
                        if let Err(error) = runtime.block_on(models.run(&database, request, &cancelled, &publish))
                            && !cancelled.load(Ordering::Acquire)
                            && !matches!(error, AnalysisError::Cancelled)
                        {
                            tracing::warn!(error = &error as &dyn std::error::Error, "music analysis failed");
                            if matches!(error, AnalysisError::Gpu(_)) {
                                models.release();
                            }
                            if !publish(Event::Failed(Arc::new(error))) {
                                break;
                            }
                        }
                    }
                }
            }
        })?;
        Ok((Self { sender, current: None }, results))
    }

    pub fn submit(&mut self, request: Request) -> Result<(), AnalysisError> {
        if let Some(cancelled) = self.current.take() {
            cancelled.store(true, Ordering::Release);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        self.current = Some(cancelled.clone());
        self.sender
            .send(Command::Run(request, cancelled))
            .map_err(|_| AnalysisError::Gpu("analysis worker stopped".to_owned()))
    }

    pub fn cancel(&mut self) {
        if let Some(cancelled) = self.current.take() {
            cancelled.store(true, Ordering::Release);
        }
        if self.sender.send(Command::Release).is_err() {
            tracing::debug!("analysis worker already stopped");
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(cancelled) = self.current.take() {
            cancelled.store(true, Ordering::Release);
        }
        if self.sender.send(Command::Stop).is_err() {
            tracing::debug!("analysis worker already stopped");
        }
    }
}

struct Models {
    directory: PathBuf,
    gpu: Option<Gpu>,
    stems: Option<(Htdemucs, Instant)>,
    beats: Option<(usize, BeatThis, Instant)>,
    last: Instant,
}

impl Models {
    fn new(directory: PathBuf) -> Self {
        Self { directory, gpu: None, stems: None, beats: None, last: Instant::now() }
    }

    fn trim(&mut self) {
        if self.stems.as_ref().is_some_and(|(_, used)| used.elapsed() >= Duration::from_secs(30)) {
            self.stems = None;
        }
        if self.beats.as_ref().is_some_and(|(_, _, used)| used.elapsed() >= Duration::from_secs(60)) {
            self.beats = None;
        }
        if self.last.elapsed() >= Duration::from_secs(120) {
            self.release();
        }
    }

    fn idle_wait(&self) -> Option<Duration> {
        [
            self.stems.as_ref().map(|(_, used)| Duration::from_secs(30).saturating_sub(used.elapsed())),
            self.beats.as_ref().map(|(_, _, used)| Duration::from_secs(60).saturating_sub(used.elapsed())),
            self.gpu.as_ref().map(|_| Duration::from_secs(120).saturating_sub(self.last.elapsed())),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn release(&mut self) {
        self.stems = None;
        self.beats = None;
        self.gpu = None;
    }

    async fn ensure_gpu(&mut self) -> Result<(), AnalysisError> {
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::from_runtime(&self.directory.with_file_name("analysis")).await?);
        }
        Ok(())
    }

    async fn run(
        &mut self,
        database: &Path,
        request: Request,
        cancelled: &Arc<AtomicBool>,
        publish: &impl Fn(Event) -> bool,
    ) -> Result<(), AnalysisError> {
        if request.cached_only {
            self.release();
        }
        let cache = Cache::at(database)?;
        let key = representation_key(&request.input, request.local_file)?;
        for model in [ModelKind::BeatThis, ModelKind::Htdemucs] {
            let wanted = match model {
                ModelKind::BeatThis => request.needs.beats,
                ModelKind::Htdemucs => request.needs.stems,
            };
            if !wanted {
                continue;
            }
            if cancelled.load(Ordering::Acquire) {
                return Err(AnalysisError::Cancelled);
            }
            if let Some(data) = cache.get(&key, model)? {
                if !publish(Event::Ready(data)) {
                    return Err(AnalysisError::Cancelled);
                }
                continue;
            }
            if request.cached_only {
                continue;
            }
            if !publish(Event::Analysing(model)) {
                return Err(AnalysisError::Cancelled);
            }
            let data = match model {
                ModelKind::BeatThis => {
                    AnalysisData::Beats(Arc::new(self.analyse_beats(&request.input, cancelled, publish).await?))
                }
                ModelKind::Htdemucs => {
                    AnalysisData::Stems(Arc::new(self.analyse_stems(&request.input, cancelled, publish).await?))
                }
            };
            if cancelled.load(Ordering::Acquire) {
                return Err(AnalysisError::Cancelled);
            }
            self.wait_for_audio(&request.input, cancelled).await?;
            if representation_key(&request.input, request.local_file)? != key {
                return Err(AnalysisError::Model("audio changed during analysis".to_owned()));
            }
            if let Err(error) = cache.put(&key, &data) {
                tracing::warn!(error = &error as &dyn std::error::Error, "analysis result was not cached");
            }
            tracing::info!(model = model.file(), "music analysis completed");
            if !publish(Event::Ready(data)) {
                return Err(AnalysisError::Cancelled);
            }
        }
        if !publish(Event::Finished) {
            return Err(AnalysisError::Cancelled);
        }
        Ok(())
    }

    async fn wait_for_audio(&mut self, input: &AudioInput, cancelled: &AtomicBool) -> Result<(), AnalysisError> {
        let Some(download) = &input.download else {
            return Ok(());
        };
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(AnalysisError::Cancelled);
            }
            let (_, done, error) = download.progress();
            if let Some(error) = error {
                return Err(AnalysisError::Model(format!("analysis audio download failed: {error}")));
            }
            if done {
                return Ok(());
            }
            self.trim();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn analyse_stems(
        &mut self,
        input: &AudioInput,
        cancelled: &Arc<AtomicBool>,
        publish: &impl Fn(Event) -> bool,
    ) -> Result<StemAnalysis, AnalysisError> {
        if self.stems.is_none() {
            let weights = Weights::load(&self.directory.join(ModelKind::Htdemucs.file()), ModelKind::Htdemucs)?;
            if self
                .beats
                .as_ref()
                .is_some_and(|(_, model, _)| model.allocated_bytes() + 350 * 1024 * 1024 > 512 * 1024 * 1024)
            {
                self.beats = None;
            }
            self.ensure_gpu().await?;
            let Some(gpu) = &self.gpu else {
                return Err(AnalysisError::Gpu("analysis device was released".to_owned()));
            };
            self.stems = Some((Htdemucs::new_cancellable(gpu, &weights, cancelled.clone()).await?, Instant::now()));
        }
        let Some((model, used)) = &mut self.stems else {
            return Err(AnalysisError::Gpu("stem model was released".to_owned()));
        };
        let result = separate(input, model, cancelled, publish);
        *used = Instant::now();
        self.last = *used;
        result
    }

    async fn analyse_beats(
        &mut self,
        input: &AudioInput,
        cancelled: &Arc<AtomicBool>,
        publish: &impl Fn(Event) -> bool,
    ) -> Result<Vec<BeatEvent>, AnalysisError> {
        let mel = read_mel(input, cancelled)?;
        if mel.frames == 0 {
            return Ok(Vec::new());
        }
        let frames = (mel.frames + 12).min(MAX_FRAMES);
        if self.beats.as_ref().is_none_or(|(count, _, _)| *count != frames) {
            self.beats = None;
            let weights = Weights::load(&self.directory.join(ModelKind::BeatThis.file()), ModelKind::BeatThis)?;
            if self
                .stems
                .as_ref()
                .is_some_and(|(model, _)| model.allocated_bytes() + 106 * 1024 * 1024 > 512 * 1024 * 1024)
            {
                self.stems = None;
            }
            self.ensure_gpu().await?;
            let Some(gpu) = &self.gpu else {
                return Err(AnalysisError::Gpu("analysis device was released".to_owned()));
            };
            self.beats = Some((
                frames,
                BeatThis::new_cancellable(gpu, &weights, frames, cancelled.clone()).await?,
                Instant::now(),
            ));
        }
        let Some((_, model, used)) = &mut self.beats else {
            return Err(AnalysisError::Gpu("beat model was released".to_owned()));
        };
        let result = crate::rhythm::predict(model, &mel, cancelled, |progress| publish(Event::Progress(progress)));
        *used = Instant::now();
        self.last = *used;
        let predicted = result?;
        let Some(gpu) = &self.gpu else { return Err(AnalysisError::Gpu("analysis device was released".to_owned())) };
        let energy = match crate::percussive::energy(gpu, &mel, cancelled).await {
            Ok(energy) => Some(energy),
            Err(AnalysisError::Cancelled) => return Err(AnalysisError::Cancelled),
            Err(error) => {
                tracing::warn!(error = &error as &dyn std::error::Error, "beat rhythm gate is unavailable");
                None
            }
        };
        crate::accents::events(&mel, &predicted.grid, &predicted.logits, energy.as_ref())
    }
}

pub fn representation_key(input: &AudioInput, local_file: bool) -> Result<String, AnalysisError> {
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    hash.update(input.path.as_os_str().as_encoded_bytes());
    if local_file {
        let metadata = std::fs::metadata(&input.path)?;
        hash.update(&metadata.len().to_le_bytes());
        let modified = metadata.modified()?;
        let (before, time) = match modified.duration_since(std::time::UNIX_EPOCH) {
            Ok(time) => (false, time),
            Err(error) => (true, error.duration()),
        };
        hash.update(&[u8::from(before)]);
        hash.update(&time.as_nanos().to_le_bytes());
    }
    Ok(hash.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect())
}

fn read_mel(input: &AudioInput, cancelled: &Arc<AtomicBool>) -> Result<crate::signal::MelSpectrogram, AnalysisError> {
    let mut reader = AudioReader::open(input, 22050, cancelled.clone())?;
    let mut spectrum = MelStream::new();
    let (mut left, mut right, mut mono) = (Vec::new(), Vec::new(), Vec::new());
    loop {
        left.clear();
        right.clear();
        mono.clear();
        if reader.read(8192, &mut left, &mut right)? == 0 {
            break;
        }
        mono.extend(left.iter().zip(&right).map(|(left, right)| (*left + *right) * 0.5));
        spectrum.push(&mono)?;
    }
    spectrum.into_spectrogram()
}

fn separate(
    input: &AudioInput,
    model: &mut Htdemucs,
    cancelled: &Arc<AtomicBool>,
    publish: &impl Fn(Event) -> bool,
) -> Result<StemAnalysis, AnalysisError> {
    let stride = SEGMENT - SEGMENT / 4;
    let overlap = SEGMENT / 4;
    let mut reader = AudioReader::open(input, 44100, cancelled.clone())?;
    let (mut left, mut right) = (Vec::with_capacity(SEGMENT), Vec::with_capacity(SEGMENT));
    let mut count = reader.read(SEGMENT, &mut left, &mut right)?;
    let mut features = StemFeatures::new()?;
    let mut sums = [vec![0.0f32; SEGMENT], vec![0.0f32; SEGMENT]];
    let mut weights = vec![0.0f32; SEGMENT];
    let window: Vec<_> = (0..SEGMENT)
        .map(|index| {
            if index < overlap {
                index as f32 / (overlap - 1) as f32
            } else if index >= SEGMENT - overlap {
                (SEGMENT - 1 - index) as f32 / (overlap - 1) as f32
            } else {
                1.0
            }
        })
        .collect();
    let (mut drums, mut voice, mut mix) =
        (Vec::with_capacity(stride), Vec::with_capacity(stride), Vec::with_capacity(stride));
    let mut last_progress = Instant::now();
    let mut completed = 0;
    while count > 0 {
        left.resize(SEGMENT, 0.0);
        right.resize(SEGMENT, 0.0);
        let stems = model.run(&left, &right, cancelled)?;
        let (drum_l, drum_r) = stems.drums();
        let (voice_l, voice_r) = stems.vocals();
        for index in 0..count {
            sums[0][index] += (drum_l[index] + drum_r[index]) * 0.5 * window[index];
            sums[1][index] += (voice_l[index] + voice_r[index]) * 0.5 * window[index];
            weights[index] += window[index];
        }
        let final_frames = count.min(stride);
        drums.clear();
        voice.clear();
        mix.clear();
        for index in 0..final_frames {
            let weight = weights[index].max(1e-8);
            drums.push(sums[0][index] / weight);
            voice.push(sums[1][index] / weight);
            mix.push((left[index] + right[index]) * 0.5);
        }
        features.push(&drums, &voice, &mix)?;
        completed += final_frames;
        if last_progress.elapsed() >= Duration::from_secs(1) {
            if !publish(Event::Progress(completed as f64 / 44100.0)) {
                return Err(AnalysisError::Cancelled);
            }
            if let Some(hits) = features.live_drums()
                && !publish(Event::PartialDrums(Arc::new(hits)))
            {
                return Err(AnalysisError::Cancelled);
            }
            last_progress = Instant::now();
        }
        if count <= stride {
            break;
        }
        for sum in &mut sums {
            sum.copy_within(stride.., 0);
            sum[overlap..].fill(0.0);
        }
        weights.copy_within(stride.., 0);
        weights[overlap..].fill(0.0);
        left.truncate(count);
        right.truncate(count);
        left.drain(..stride);
        right.drain(..stride);
        reader.read(SEGMENT - left.len(), &mut left, &mut right)?;
        count = left.len();
    }
    features.finish()
}
