// Lock-free room handoff and the original two-lane warm-up, linear fades and room limiter.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::room::RoomConvolver;

enum Command {
    Play(Box<RoomConvolver>),
    Stop(u64),
}

pub struct Control {
    commands: Producer<Command>,
    retired: Consumer<Box<RoomConvolver>>,
    pending: Option<Command>,
    fault: Arc<AtomicBool>,
    fault_reported: bool,
    rate: u32,
    enabled: bool,
    release: u64,
    released: Arc<AtomicU64>,
}

pub struct Mixer {
    commands: Consumer<Command>,
    retired: Producer<Box<RoomConvolver>>,
    active: Option<Box<RoomConvolver>>,
    incoming: Option<Box<RoomConvolver>>,
    pending: Option<Box<RoomConvolver>>,
    phase: Phase,
    desired: bool,
    wet: f32,
    target: f32,
    wet_step: f32,
    wet_left: usize,
    stop_left: Option<(usize, u64)>,
    released: Arc<AtomicU64>,
    rate: u32,
    fault: Arc<AtomicBool>,
    suspended: bool,
    limiter: Limiter,
}

#[derive(Clone, Copy)]
enum Phase {
    Stable,
    Warming(usize),
    Crossfade(usize),
}

impl Control {
    pub fn pair(rate: u32) -> (Self, Mixer) {
        let (commands, input) = RingBuffer::new(4);
        let (retired, output) = RingBuffer::new(8);
        let fault = Arc::new(AtomicBool::new(false));
        let released = Arc::new(AtomicU64::new(0));
        (
            Self {
                commands,
                retired: output,
                pending: None,
                fault: fault.clone(),
                fault_reported: false,
                rate,
                enabled: false,
                release: 0,
                released: released.clone(),
            },
            Mixer {
                commands: input,
                retired,
                active: None,
                incoming: None,
                pending: None,
                phase: Phase::Stable,
                desired: false,
                wet: 0.0,
                target: 0.0,
                wet_step: 0.0,
                wet_left: 0,
                stop_left: None,
                released,
                rate,
                fault,
                suspended: false,
                limiter: Limiter::new(rate),
            },
        )
    }

    pub fn set(&mut self, convolver: Option<Box<RoomConvolver>>) -> Result<(), crate::RoomError> {
        if convolver.as_ref().is_some_and(|convolver| convolver.sample_rate() != self.rate) {
            return Err(crate::RoomError::SampleRate);
        }
        if convolver.is_none() && !self.enabled {
            self.collect();
            return Ok(());
        }
        self.enabled = convolver.is_some();
        self.pending = Some(match convolver {
            Some(convolver) => Command::Play(convolver),
            None => {
                self.release = self.release.wrapping_add(1);
                Command::Stop(self.release)
            }
        });
        self.collect();
        Ok(())
    }

    pub fn collect(&mut self) {
        // The UI/control thread drops retired FFT storage; the real-time callback only moves boxes.
        while let Ok(convolver) = self.retired.pop() {
            drop(convolver);
        }
        if let Some(command) = self.pending.take()
            && let Err(PushError::Full(command)) = self.commands.push(command)
        {
            self.pending = Some(command);
        }
        let failed = self.fault.load(Ordering::Relaxed);
        if failed && !self.fault_reported {
            tracing::error!("room convolution failed; using dry playback");
        }
        self.fault_reported = failed;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// True after the callback has handed the disabled room's storage back for collection.
    pub fn released(&self) -> bool {
        !self.enabled && self.released.load(Ordering::Acquire) == self.release
    }
}

impl Mixer {
    pub fn drain(&mut self) {
        while self.retired.slots() >= 3 {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            match command {
                Command::Play(convolver) => {
                    if self.fault.swap(false, Ordering::Relaxed) {
                        retire(&mut self.active, &mut self.retired);
                        retire(&mut self.incoming, &mut self.retired);
                        self.phase = Phase::Stable;
                    }
                    retire(&mut self.pending, &mut self.retired);
                    self.pending = Some(convolver);
                    self.desired = true;
                    self.stop_left = None;
                }
                Command::Stop(release) => {
                    self.desired = false;
                    self.ramp(0.0);
                    self.stop_left = Some(((f64::from(self.rate) * 0.55).round() as usize, release));
                    retire(&mut self.pending, &mut self.retired);
                    if !matches!(self.phase, Phase::Crossfade(_)) {
                        retire(&mut self.incoming, &mut self.retired);
                        self.phase = Phase::Stable;
                    }
                }
            }
        }
        if self.pending.is_some() && !matches!(self.phase, Phase::Crossfade(_)) && self.retired.slots() > 0 {
            retire(&mut self.incoming, &mut self.retired);
            self.incoming = self.pending.take();
            self.phase = Phase::Warming(self.incoming.as_ref().map_or(0, |convolver| convolver.length()));
        }
    }

    pub fn reset(&mut self) {
        for convolver in [&mut self.active, &mut self.incoming, &mut self.pending].into_iter().flatten() {
            convolver.reset();
        }
        self.limiter.reset();
    }

    pub fn silent(&mut self, frames: usize) {
        if self.active.is_none() && self.incoming.is_none() && self.stop_left.is_none() {
            return;
        }
        if !self.suspended {
            self.reset();
            self.suspended = true;
        }
        for _ in 0..frames {
            self.advance();
        }
    }

    pub fn process(&mut self, left: &mut f32, right: &mut f32) {
        self.suspended = false;
        if self.active.is_none() && self.incoming.is_none() {
            if self.stop_left.is_some() {
                self.advance();
            }
            return;
        }
        if self.fault.load(Ordering::Relaxed) {
            self.advance();
            return;
        }
        let dry = [*left, *right];
        let active = process(&mut self.active, dry, &self.fault);
        let incoming = process(&mut self.incoming, dry, &self.fault);
        if self.fault.load(Ordering::Relaxed) {
            self.desired = false;
            self.wet = 0.0;
            self.target = 0.0;
            self.wet_left = 0;
            self.stop_left = Some((0, 0));
            self.advance();
            return;
        }
        let room = match self.phase {
            Phase::Crossfade(left) => {
                let amount = 1.0 - left as f32 / (self.rate as f32 * 0.1).round().max(1.0);
                [active[0] + (incoming[0] - active[0]) * amount, active[1] + (incoming[1] - active[1]) * amount]
            }
            _ => active,
        };
        let room = self.limiter.process(room);
        *left = dry[0] + (room[0] - dry[0]) * self.wet;
        *right = dry[1] + (room[1] - dry[1]) * self.wet;
        self.advance();
    }

    fn advance(&mut self) {
        match self.phase {
            Phase::Warming(remaining) if remaining > 1 => self.phase = Phase::Warming(remaining - 1),
            Phase::Warming(_) => {
                self.phase = Phase::Crossfade((self.rate as f32 * 0.1).round() as usize);
                if self.desired {
                    self.ramp(1.0);
                }
            }
            Phase::Crossfade(remaining) if remaining > 0 => self.phase = Phase::Crossfade(remaining - 1),
            Phase::Crossfade(_) if self.retired.slots() > 0 => {
                retire(&mut self.active, &mut self.retired);
                self.active = self.incoming.take();
                self.phase = Phase::Stable;
            }
            _ => {}
        }
        if self.wet_left > 0 {
            self.wet += self.wet_step;
            self.wet_left -= 1;
            if self.wet_left == 0 {
                self.wet = self.target;
            }
        }
        if let Some((remaining, release)) = self.stop_left {
            self.stop_left = Some((remaining.saturating_sub(1), release));
            if remaining == 0 && self.retired.slots() >= 2 {
                retire(&mut self.active, &mut self.retired);
                retire(&mut self.incoming, &mut self.retired);
                self.phase = Phase::Stable;
                self.stop_left = None;
                self.limiter.reset();
                self.released.store(release, Ordering::Release);
            }
        }
    }

    fn ramp(&mut self, target: f32) {
        if self.target == target {
            return;
        }
        self.target = target;
        self.wet_left = (self.rate as f32 * 0.5).round().max(1.0) as usize;
        self.wet_step = (target - self.wet) / self.wet_left as f32;
    }
}

fn retire(slot: &mut Option<Box<RoomConvolver>>, retired: &mut Producer<Box<RoomConvolver>>) {
    if let Some(convolver) = slot.take()
        && let Err(PushError::Full(convolver)) = retired.push(convolver)
    {
        *slot = Some(convolver);
    }
}

fn process(convolver: &mut Option<Box<RoomConvolver>>, input: [f32; 2], fault: &AtomicBool) -> [f32; 2] {
    let Some(convolver) = convolver else {
        return [0.0; 2];
    };
    match convolver.process(input) {
        Ok(output) if output.iter().all(|sample| sample.is_finite()) => output,
        _ => {
            fault.store(true, Ordering::Relaxed);
            input
        }
    }
}

struct Limiter {
    envelope: f32,
    attack: f32,
    release: f32,
    lookahead: Vec<[f32; 2]>,
    at: usize,
}

impl Limiter {
    fn new(rate: u32) -> Self {
        Self {
            envelope: 0.0,
            attack: (-1.0 / (rate as f32 * 0.001)).exp(),
            release: (-1.0 / (rate as f32 * 0.1)).exp(),
            lookahead: vec![[0.0; 2]; (rate as f32 * 0.006).round().max(1.0) as usize],
            at: 0,
        }
    }
    fn reset(&mut self) {
        self.envelope = 0.0;
        self.at = 0;
        self.lookahead.fill([0.0; 2]);
    }
    fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        let peak = input[0].abs().max(input[1].abs());
        let smoothing = if peak > self.envelope { self.attack } else { self.release };
        self.envelope = peak + smoothing * (self.envelope - peak);
        let threshold = 10.0_f32.powf(-1.0 / 20.0);
        let gain = if self.envelope > threshold { (self.envelope / threshold).powf(1.0 / 20.0 - 1.0) } else { 1.0 };
        let delayed = self.lookahead[self.at];
        self.lookahead[self.at] = input;
        self.at = (self.at + 1) % self.lookahead.len();
        delayed.map(|sample| sample * gain)
    }
}
