// Times one Analyser::frame call: drain a playback quantum, then the 2048-point FFT.

use criterion::{Criterion, criterion_group, criterion_main};
use folia_player::{Analyser, push_samples};

fn frame(c: &mut Criterion) {
    let (mut producer, mut analyser) = Analyser::pair();
    let warmup = vec![0.2f32; 2048];
    push_samples(&mut producer, &warmup);
    let _ = analyser.frame(true, 0.0);
    let quantum = vec![0.2f32; 735];
    c.bench_function("analyser_frame", |bencher| {
        bencher.iter(|| {
            push_samples(&mut producer, &quantum);
            std::hint::black_box(analyser.frame(true, 0.0).power)
        });
    });
}

criterion_group!(benches, frame);
criterion_main!(benches);
