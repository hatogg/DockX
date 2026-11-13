//! Springs and easing — the same maths DockX 1.1 used, so the dock feels identical.

/// Critically damped spring (frame-rate independent, no overshoot).
pub fn smooth_damp(current: f32, target: f32, vel: &mut f32, smooth_time: f32, dt: f32) -> f32 {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*vel + omega * change) * dt;
    *vel = (*vel - omega * temp) * exp;
    let mut output = target + (change + temp) * exp;
    if (target - current > 0.0) == (output > target) {
        output = target;
        *vel = 0.0;
    }
    output
}

/// A value with velocity. `step` returns true while it is still moving.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    pub v: f32,
    pub vel: f32,
}

impl Spring {
    pub const fn new(v: f32) -> Spring {
        Spring { v, vel: 0.0 }
    }
    pub fn step(&mut self, target: f32, time: f32, dt: f32, eps: f32) -> bool {
        if self.v == target && self.vel == 0.0 {
            return false;
        }
        self.v = smooth_damp(self.v, target, &mut self.vel, time, dt);
        if (self.v - target).abs() < eps && self.vel.abs() < eps * 8.0 {
            self.v = target;
            self.vel = 0.0;
        }
        true
    }
    pub fn snap(&mut self, v: f32) {
        self.v = v;
        self.vel = 0.0;
    }
}

/// Under-damped spring with a little bounce — used for the island morph (iPhone-like).
#[derive(Clone, Copy, Debug, Default)]
pub struct Bouncy {
    pub v: f32,
    pub vel: f32,
}

impl Bouncy {
    pub const fn new(v: f32) -> Bouncy {
        Bouncy { v, vel: 0.0 }
    }
    /// `freq` in Hz, `damping` ratio (1 = critical).
    pub fn step(&mut self, target: f32, freq: f32, damping: f32, dt: f32, eps: f32) -> bool {
        if self.v == target && self.vel == 0.0 {
            return false;
        }
        let w = std::f32::consts::TAU * freq;
        // semi-implicit Euler in small sub-steps for stability
        let n = ((dt / 0.004).ceil() as i32).max(1);
        let h = dt / n as f32;
        for _ in 0..n {
            let acc = -w * w * (self.v - target) - 2.0 * damping * w * self.vel;
            self.vel += acc * h;
            self.v += self.vel * h;
        }
        if (self.v - target).abs() < eps && self.vel.abs() < eps * 6.0 {
            self.v = target;
            self.vel = 0.0;
        }
        true
    }
    pub fn snap(&mut self, v: f32) {
        self.v = v;
        self.vel = 0.0;
    }
}

pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
