// GPU median-filter HPSS energy used by the original beat accent rhythm gate.
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AnalysisError;
use crate::gpu::{Binding, Gpu, GraphBuilder, Kernel};
use crate::signal::{BANDS, MelSpectrogram};

pub struct Energy {
    pub percussive: Vec<f32>,
    pub total: Vec<f32>,
}

pub async fn energy(gpu: &Gpu, mel: &MelSpectrogram, cancelled: &AtomicBool) -> Result<Energy, AnalysisError> {
    if mel.frames == 0 || mel.data.len() != mel.frames * BANDS {
        return Err(AnalysisError::Model("HPSS requires a nonempty complete spectrogram".to_owned()));
    }
    let mut builder = GraphBuilder::empty(gpu);
    let input = builder.storage(mel.data.len());
    let output = builder.storage(mel.frames * 2);
    let shader = Kernel { label: "beat-this-percussive", source: include_str!("../shaders/beat_this/percussive.wgsl") };
    for first in (0..mel.frames).step_by(1024) {
        if cancelled.load(Ordering::Acquire) {
            builder.discard().await?;
            return Err(AnalysisError::Cancelled);
        }
        let count = (mel.frames - first).min(1024);
        builder.dispatch(
            shader,
            &[Binding::whole(&input), Binding::whole(&output)],
            &[mel.frames as u32, first as u32, count as u32],
            [count.div_ceil(64) as u32, 1, 1],
        )?;
    }
    let mut graph = builder.finish(input, output).await?;
    let result = graph.run(&mel.data, cancelled)?;
    let mut percussive = Vec::with_capacity(mel.frames);
    let mut total = Vec::with_capacity(mel.frames);
    for pair in result[..mel.frames * 2].as_chunks::<2>().0 {
        percussive.push(pair[0]);
        total.push(pair[1]);
    }
    Ok(Energy { percussive, total })
}
