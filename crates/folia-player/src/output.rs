// The cpal output stream: the real-time callback copies decoded samples from the ring buffer to the device.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::PlayerError;
use crate::analyser::push_samples;
use crate::equalizer::{Parameters, Processor};
use crate::shared::{PlayerState, Shared};

pub struct Output {
    pub stream: Stream,
    pub rate: u32,
    pub channels: usize,
}

/// Seconds of audio the ring buffer between decoder and device holds.
const BUFFER_SECONDS: f64 = 0.5;

/// Opens the default output device and returns the stream with the producer end of its ring buffer.
pub fn open(
    shared: Arc<Shared>,
    tap: Producer<f32>,
    equalizer: rtrb::Consumer<Parameters>,
) -> Result<(Output, Producer<f32>), PlayerError> {
    let device = cpal::default_host().default_output_device().ok_or(PlayerError::NoDevice)?;
    let supported = device.default_output_config().map_err(PlayerError::Device)?;
    let config = supported.config();
    let channels = usize::from(config.channels);
    let capacity = (f64::from(config.sample_rate) * BUFFER_SECONDS) as usize * channels;
    let (producer, consumer) = RingBuffer::new(capacity);

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, consumer, tap, shared, equalizer),
        SampleFormat::F64 => build::<f64>(&device, config, consumer, tap, shared, equalizer),
        SampleFormat::I16 => build::<i16>(&device, config, consumer, tap, shared, equalizer),
        SampleFormat::I32 => build::<i32>(&device, config, consumer, tap, shared, equalizer),
        SampleFormat::U16 => build::<u16>(&device, config, consumer, tap, shared, equalizer),
        other => return Err(PlayerError::UnsupportedFormat(format!("{other:?}"))),
    }?;
    Ok((Output { stream, rate: config.sample_rate, channels }, producer))
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut consumer: Consumer<f32>,
    mut tap: Producer<f32>,
    shared: Arc<Shared>,
    mut equalizer: rtrb::Consumer<Parameters>,
) -> Result<Stream, PlayerError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut processor = Processor::new(config.sample_rate as f32);
    let callback = move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
        processor.drain(&mut equalizer);
        if shared.take_flush() {
            let stale = consumer.slots();
            if let Ok(chunk) = consumer.read_chunk(stale) {
                chunk.commit_all();
            }
        }
        if shared.state() != PlayerState::Playing {
            out.fill(T::EQUILIBRIUM);
            return;
        }

        let volume = shared.volume();
        let available = consumer.slots().min(out.len());
        let available = available - available % channels;
        if let Ok(chunk) = consumer.read_chunk(available) {
            let (first, second) = chunk.as_slices();
            let mut source = first.iter().chain(second);
            let mut mono = [0.0f32; 256];
            let mut mono_len = 0usize;
            for frame in out[..available].chunks_exact_mut(channels) {
                let mut values = [0.0f32; 32];
                for value in values.iter_mut().take(channels) {
                    *value = *source.next().unwrap_or(&0.0);
                }
                let (mut left, mut right) = (values[0], values[if channels > 1 { 1 } else { 0 }]);
                processor.process(&mut left, &mut right);
                frame[0] = T::from_sample(left * volume);
                if channels > 1 {
                    frame[1] = T::from_sample(right * volume);
                }
                for (slot, value) in frame.iter_mut().zip(values.iter()).skip(2) {
                    *slot = T::from_sample(*value * volume);
                }
                mono[mono_len] = (left + right) * 0.5 * volume;
                mono_len += 1;
                if mono_len == mono.len() {
                    push_samples(&mut tap, &mono);
                    mono_len = 0;
                }
            }
            if mono_len > 0 {
                push_samples(&mut tap, &mono[..mono_len]);
            }
            chunk.commit_all();
        }
        out[available..].fill(T::EQUILIBRIUM);
        shared.add_played((available / channels) as u64);

        if available == 0 && shared.decoder_done.load(std::sync::atomic::Ordering::Acquire) {
            shared.set_state(PlayerState::Ended);
        }
    };
    let on_error = |err: cpal::Error| tracing::error!(error = &err as &dyn std::error::Error, "audio output error");
    device.build_output_stream::<T, _, _>(config, callback, on_error, None).map_err(PlayerError::Device)
}
