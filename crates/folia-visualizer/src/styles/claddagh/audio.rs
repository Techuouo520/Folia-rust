// Framer-motion springs on the bass and vocal bands (`VisualizerCladdagh.tsx`). Mass is 1.

/// One second-order spring. The target is held constant across `step`.
pub struct Spring {
    stiffness: f32,
    damping: f32,
    value: f32,
    velocity: f32,
}

impl Spring {
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Self { stiffness, damping, value: 0.0, velocity: 0.0 }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    /// Advances toward `target` by `dt` seconds.
    pub fn step(&mut self, target: f32, dt: f32) {
        if dt <= 0.0 || self.stiffness <= 0.0 {
            return;
        }
        let omega = self.stiffness.sqrt();
        let zeta = self.damping / (2.0 * omega);
        let offset = self.value - target;
        let (next, velocity) = if (zeta - 1.0).abs() < 1.0e-4 {
            critical(offset, self.velocity, omega, dt)
        } else if zeta < 1.0 {
            underdamped(offset, self.velocity, omega, zeta, dt)
        } else {
            overdamped(offset, self.velocity, omega, zeta, dt)
        };
        self.value = target + next;
        self.velocity = velocity;
    }
}

fn underdamped(offset: f32, velocity: f32, omega: f32, zeta: f32, dt: f32) -> (f32, f32) {
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    let envelope = (-zeta * omega * dt).exp();
    let cos = (wd * dt).cos();
    let sin = (wd * dt).sin();
    let coefficient = (velocity + zeta * omega * offset) / wd;
    let next = envelope * (offset * cos + coefficient * sin);
    let derived = envelope * (-offset * wd * sin + coefficient * wd * cos);
    (next, -zeta * omega * next + derived)
}

fn overdamped(offset: f32, velocity: f32, omega: f32, zeta: f32, dt: f32) -> (f32, f32) {
    let wd = omega * (zeta * zeta - 1.0).sqrt();
    let root_a = -zeta * omega + wd;
    let root_b = -zeta * omega - wd;
    let share = (velocity - root_b * offset) / (root_a - root_b);
    let rest = offset - share;
    let decay_a = (root_a * dt).exp();
    let decay_b = (root_b * dt).exp();
    (share * decay_a + rest * decay_b, share * root_a * decay_a + rest * root_b * decay_b)
}

fn critical(offset: f32, velocity: f32, omega: f32, dt: f32) -> (f32, f32) {
    let envelope = (-omega * dt).exp();
    let slope = velocity + omega * offset;
    let next = envelope * (offset + slope * dt);
    (next, -omega * next + envelope * slope)
}

/// The playback bridge writes 0–255. Preview clocks that wrote 0–1 are not used here.
pub fn unit(value: f32) -> f32 {
    if value.is_finite() { (value / 255.0).clamp(0.0, 1.0) } else { 0.0 }
}
