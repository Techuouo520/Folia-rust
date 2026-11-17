// Original room-IR delay alignment, reflection bass rolloff and default-seat loudness calibration.

use folia_core::RoomImpulseResponse;

pub(crate) struct SpeakerImpulse {
    pub wet: [Vec<f32>; 2],
    pub dry: [Vec<f32>; 2],
    pub onset: usize,
    pub delay: usize,
}

pub(crate) fn assemble(speakers: &[SpeakerImpulse], sample_rate: u32) -> RoomImpulseResponse {
    let size = sample_rate as usize * 2;
    let mut channels = std::array::from_fn(|_| vec![0.0_f32; size]);
    let shift = speakers.iter().map(|speaker| speaker.delay).min().unwrap_or(0);
    let lead = speakers.iter().map(|speaker| speaker.onset).min().unwrap_or(0);
    for (index, speaker) in speakers.iter().enumerate().take(2) {
        let at = speaker.delay as isize - shift as isize - lead as isize;
        for ear in 0..2 {
            let wet = high_pass(&speaker.wet[ear], sample_rate);
            add(&mut channels[index * 2 + ear], &wet, at);
            add(&mut channels[index * 2 + ear], &speaker.dry[ear], at);
        }
    }
    let fade = (f64::from(sample_rate) * 0.05).round() as usize;
    for channel in &mut channels {
        for index in 0..fade {
            channel[size - fade + index] *=
                (0.5 * (1.0 + (std::f64::consts::PI * (index + 1) as f64 / fade as f64).cos())) as f32;
        }
    }
    RoomImpulseResponse { sample_rate, channels, gain: 1.0 }
}

fn add(output: &mut [f32], input: &[f32], at: isize) {
    let start = (-at).max(0) as usize;
    for (index, sample) in input.iter().enumerate().skip(start) {
        let target = (at + index as isize) as usize;
        if target >= output.len() {
            break;
        }
        output[target] += sample;
    }
}

fn high_pass(input: &[f32], sample_rate: u32) -> Vec<f32> {
    let omega = std::f64::consts::TAU * 200.0 / f64::from(sample_rate);
    let cos = omega.cos();
    let alpha = omega.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let a0 = 1.0 + alpha;
    biquad(
        input,
        [(1.0 + cos) / (2.0 * a0), -(1.0 + cos) / a0, (1.0 + cos) / (2.0 * a0)],
        [-2.0 * cos / a0, (1.0 - alpha) / a0],
    )
}

fn biquad(input: &[f32], b: [f64; 3], a: [f64; 2]) -> Vec<f32> {
    let mut filter = Biquad { b, a, state: [0.0; 4] };
    input.iter().map(|sample| filter.process(f64::from(*sample)) as f32).collect()
}

struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    state: [f64; 4],
}

impl Biquad {
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.state[0] + self.b[2] * self.state[1]
            - self.a[0] * self.state[2]
            - self.a[1] * self.state[3];
        self.state = [x, self.state[0], y, self.state[2]];
        y
    }
}

fn pink(input: &[f32]) -> Vec<f32> {
    let b = [0.049922035, -0.095993537, 0.050612699, -0.004408786];
    let a = [-2.494956002, 2.017265875, -0.522189400];
    let mut x = [0.0; 3];
    let mut y = [0.0; 3];
    input
        .iter()
        .map(|sample| {
            let value = b[0] * f64::from(*sample) + b[1] * x[0] + b[2] * x[1] + b[3] * x[2]
                - a[0] * y[0]
                - a[1] * y[1]
                - a[2] * y[2];
            x = [f64::from(*sample), x[0], x[1]];
            y = [value, y[0], y[1]];
            value as f32
        })
        .collect()
}

fn k_weight(input: &[f32], sample_rate: u32) -> Vec<f32> {
    let rate = f64::from(sample_rate);
    let k = (std::f64::consts::PI * 1681.974450955533 / rate).tan();
    let vh = 10.0_f64.powf(3.999843853973347 / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let q = 0.7071752369554196;
    let a0 = 1.0 + k / q + k * k;
    let mut shelf = Biquad {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        state: [0.0; 4],
    };
    let k = (std::f64::consts::PI * 38.13547087602444 / rate).tan();
    let q = 0.5003270373238773;
    let a0 = 1.0 + k / q + k * k;
    let mut high =
        Biquad { b: [1.0, -2.0, 1.0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0], state: [0.0; 4] };
    input.iter().map(|sample| high.process(shelf.process(f64::from(*sample))) as f32).collect()
}

fn pink_k_energy(channel: &[f32], sample_rate: u32) -> f64 {
    let mut padded = vec![0.0_f32; channel.len() + (f64::from(sample_rate) * 0.1).round() as usize];
    padded[..channel.len()].copy_from_slice(channel);
    k_weight(&pink(&padded), sample_rate).iter().map(|value| f64::from(*value).powi(2)).sum()
}

pub fn loudness_gain(channels: &[Vec<f32>; 4], sample_rate: u32) -> f32 {
    if sample_rate == 0 {
        return 1.0;
    }
    let dry = 2.0 * pink_k_energy(&[1.0], sample_rate);
    let wet: f64 = channels.iter().map(|channel| pink_k_energy(channel, sample_rate)).sum();
    if wet > 0.0 { (dry / wet).sqrt() as f32 } else { 1.0 }
}
