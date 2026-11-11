// One opened audio file: demuxes and decodes packets into interleaved f32 samples.

use folia_core::{AudioInput, DownloadReader};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::PlayerError;

/// A decoded packet: interleaved samples with their channel count and rate.
pub struct Block<'a> {
    pub samples: &'a [f32],
    pub channels: usize,
    pub rate: u32,
}

pub struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    pub duration: f64,
    samples: Vec<f32>,
    streaming: bool,
    linear_seek: Arc<AtomicBool>,
}

impl Source {
    pub fn open(path: &Path) -> Result<Self, PlayerError> {
        Self::open_input(&path.to_owned().into(), Arc::new(AtomicBool::new(false)))
    }

    pub fn open_input(input: &AudioInput, cancelled: Arc<AtomicBool>) -> Result<Self, PlayerError> {
        Self::with_status(input, cancelled, Default::default())
    }

    pub(crate) fn with_status(
        input: &AudioInput,
        cancelled: Arc<AtomicBool>,
        waiting: Arc<AtomicBool>,
    ) -> Result<Self, PlayerError> {
        let path = &input.path;
        let probing = Arc::new(AtomicBool::new(true));
        let linear_seek = Arc::new(AtomicBool::new(false));
        let file: Box<dyn symphonia::core::io::MediaSource> = match &input.download {
            Some(download) => Box::new(Streaming {
                reader: download.reader_with_status(cancelled, waiting),
                probing: probing.clone(),
                linear_seek: linear_seek.clone(),
            }),
            None => Box::new(File::open(path).map_err(|source| PlayerError::Open { path: path.to_owned(), source })?),
        };
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        let stream = MediaSourceStream::new(file, Default::default());
        let format = symphonia::default::get_probe().probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;
        probing.store(false, Ordering::Release);

        let track = format.default_track(TrackType::Audio).ok_or(PlayerError::NoAudioTrack)?;
        let params = track.codec_params.as_ref().and_then(|params| params.audio()).ok_or(PlayerError::NoAudioTrack)?;
        let decoder = symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default())?;
        let duration = match (track.num_frames, params.sample_rate) {
            (Some(frames), Some(rate)) if rate > 0 => frames as f64 / f64::from(rate),
            _ => 0.0,
        };
        let (track_id, time_base) = (track.id, track.time_base);
        Ok(Self {
            format,
            decoder,
            track_id,
            time_base,
            duration,
            samples: Vec::new(),
            streaming: input.download.as_ref().is_some_and(|download| !download.progress().1),
            linear_seek,
        })
    }

    /// Decodes the next packet of the track; `None` at the end of the stream.
    pub fn next_packet(&mut self) -> Result<Option<Block<'_>>, PlayerError> {
        loop {
            let Some(packet) = self.format.next_packet()? else { return Ok(None) };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    let channels = buffer.spec().channels().count();
                    let rate = buffer.spec().rate();
                    self.samples.resize(buffer.samples_interleaved(), 0.0);
                    buffer.copy_to_slice_interleaved(&mut self.samples);
                    return Ok(Some(Block { samples: &self.samples, channels, rate }));
                }
                Err(Error::DecodeError(err)) => tracing::debug!(error = err, "skipping undecodable packet"),
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// Seeks to `seconds` and returns the time actually reached, which may be slightly earlier.
    pub fn seek(&mut self, seconds: f64) -> Result<f64, PlayerError> {
        let time = Time::try_from_secs_f64(seconds.max(0.0)).unwrap_or_default();
        let mode = if self.streaming { SeekMode::Coarse } else { SeekMode::Accurate };
        // Without ranges, FLAC's binary search would wait in the file middle even when seeking back near zero.
        self.linear_seek.store(self.format.format_info().short_name == "flac", Ordering::Release);
        let result = self.format.seek(mode, SeekTo::Time { time, track_id: Some(self.track_id) });
        self.linear_seek.store(false, Ordering::Release);
        let seeked = result?;
        self.decoder.reset();
        let reached = self.time_base.and_then(|base| base.calc_time(seeked.actual_ts)).map(|time| time.as_secs_f64());
        Ok(reached.unwrap_or(seconds))
    }
}

struct Streaming {
    reader: DownloadReader,
    probing: Arc<AtomicBool>,
    linear_seek: Arc<AtomicBool>,
}
impl Read for Streaming {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buffer)
    }
}
impl Seek for Streaming {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(from)
    }
}
impl symphonia::core::io::MediaSource for Streaming {
    fn is_seekable(&self) -> bool {
        !self.linear_seek.load(Ordering::Acquire) || self.reader.random_access()
    }
    fn byte_len(&self) -> Option<u64> {
        if self.probing.load(Ordering::Acquire) { self.reader.byte_len() } else { self.reader.content_len() }
    }
}
