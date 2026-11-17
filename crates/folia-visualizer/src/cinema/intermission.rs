// The original pause shutter, curtain motor, pendulum hems and house-light transitions.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CurtainPose {
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntermissionFrame {
    pub shutter: f32,
    pub lights: f32,
    pub left: CurtainPose,
    pub right: CurtainPose,
}

impl Default for IntermissionFrame {
    fn default() -> Self {
        Self { shutter: 1.0, lights: 0.0, left: CurtainPose::default(), right: CurtainPose::default() }
    }
}

pub struct Intermission {
    paused_for: f32,
    lit: bool,
    first: bool,
    carriage: f32,
    carriage_speed: f32,
    hems: [[f32; 2]; 2],
    frame: IntermissionFrame,
}

impl Default for Intermission {
    fn default() -> Self {
        Self {
            paused_for: 0.0,
            lit: false,
            first: true,
            carriage: 0.0,
            carriage_speed: 0.0,
            hems: [[0.0; 2]; 2],
            frame: IntermissionFrame::default(),
        }
    }
}

impl Intermission {
    pub fn shutter(&self) -> f32 {
        self.frame.shutter
    }
    pub fn step(&mut self, dt: f32, playing: bool, lights_up: bool, still: bool, on_pause: bool) -> IntermissionFrame {
        let dt = dt.clamp(0.0, 0.1);
        if still {
            self.paused_for = 0.0;
            self.lit = false;
            self.carriage = 0.0;
            self.carriage_speed = 0.0;
            self.hems = [[0.0; 2]; 2];
            self.frame.shutter = 1.0;
            self.frame.lights = if lights_up { 1.0 } else { 0.0 };
        } else {
            self.paused_for = if playing || !on_pause { 0.0 } else { self.paused_for + dt };
            self.frame.shutter = if self.paused_for > 3.0 {
                (self.frame.shutter - dt / 0.7).max(0.0)
            } else {
                (self.frame.shutter + dt / 0.25).min(1.0)
            };
            self.move_curtains(dt);
            self.lit = !playing && (self.lit || self.carriage > 0.85);
            let lights = if lights_up {
                1.0
            } else if self.lit {
                0.5
            } else {
                0.0
            };
            let rate = if lights > self.frame.lights { 0.9 } else { 4.5 };
            self.frame.lights = if self.first { lights } else { ease(self.frame.lights, lights, rate, dt) };
        }
        self.first = false;
        self.frame.left = CurtainPose { top: self.carriage, bottom: self.hems[0][0] };
        self.frame.right = CurtainPose { top: self.carriage, bottom: self.hems[1][0] };
        self.frame
    }

    fn move_curtains(&mut self, dt: f32) {
        let target = if self.paused_for > 3.8 { 1.0 } else { 0.0 };
        let steps = (dt * 120.0).ceil().max(1.0) as u32;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let wanted = (0.8 * (target - self.carriage)).clamp(-0.17, 0.17);
            self.carriage_speed = ease(self.carriage_speed, wanted, 2.5, h);
            self.carriage += self.carriage_speed * h;
            for (hem, omega) in self.hems.iter_mut().zip([1.6, 1.72]) {
                hem[1] += (omega * omega * (self.carriage - hem[0]) - 2.0 * 0.12 * omega * hem[1]) * h;
                hem[0] += hem[1] * h;
            }
        }
    }
}

pub(super) fn ease(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-rate * dt).exp())
}
