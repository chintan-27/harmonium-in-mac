use std::time::Instant;

/// Settings (you'll control these with sliders in the GUI).
#[derive(Debug, Clone)]
pub struct BellowsParams {
    /// Ignore motion smaller than this (deg/sec). Helps remove jitter.
    pub deadzone_deg_per_s: f32,

    /// Motion speed (deg/sec) that should feel like "full pumping".
    pub vmax_deg_per_s: f32,

    /// Curve shaping. >1 makes it easier to play softly.
    pub gamma: f32,

    /// Exponential moving average alpha for smoothing speed.
    /// Range: 0..1. Smaller = smoother but slower response.
    pub ema_alpha: f32,

    /// How quickly the "air" rises when you start pumping (milliseconds).
    pub attack_ms: f32,

    /// How slowly the "air" falls when you stop pumping (milliseconds).
    pub release_ms: f32,
}

impl Default for BellowsParams {
    fn default() -> Self {
        Self {
            deadzone_deg_per_s: 8.0,
            vmax_deg_per_s: 50.0,
            gamma: 2.0,
            ema_alpha: 0.12,
            attack_ms: 250.0,
            release_ms: 400.0,
        }
    }
}

/// Output values you can display in the GUI (and later feed into audio).
#[derive(Debug, Clone, Copy)]
pub struct BellowsOutput {
    pub dt_sec: f32,
    pub theta_deg: f32,

    /// Angular velocity in degrees/second (can be negative).
    pub omega_deg_per_s: f32,

    /// Absolute angular speed (always >= 0).
    pub speed_raw: f32,

    /// Smoothed speed after EMA.
    pub speed_smooth: f32,

    /// Target amplitude after deadzone + normalization + curve.
    pub a_target: f32,

    /// Final amplitude after attack/release envelope.
    pub a: f32,
}

impl Default for BellowsOutput {
    fn default() -> Self {
        Self {
            dt_sec: 0.0,
            theta_deg: 0.0,
            omega_deg_per_s: 0.0,
            speed_raw: 0.0,
            speed_smooth: 0.0,
            a_target: 0.0,
            a: 0.0,
        }
    }
}

/// Internal bellows state (stores previous sample + filter memory).
#[derive(Debug, Clone)]
pub struct BellowsState {
    pub params: BellowsParams,

    prev_theta_deg: Option<f32>,
    prev_t: Option<Instant>,

    speed_smooth: f32,
    a: f32,
}

impl BellowsState {
    pub fn new(params: BellowsParams) -> Self {
        Self {
            params,
            prev_theta_deg: None,
            prev_t: None,
            speed_smooth: 0.0,
            a: 0.0,
        }
    }

    /// Update bellows using a new angle sample at time `t`.
    ///
    /// This is the "math pipeline":
    /// angle -> velocity -> abs speed -> smooth -> normalize -> curve -> envelope
    pub fn update(&mut self, theta_deg: f32, t: Instant) -> BellowsOutput {
        // First sample: we can't compute velocity yet.
        let (prev_theta, prev_t) = match (self.prev_theta_deg, self.prev_t) {
            (Some(pt), Some(ptt)) => (pt, ptt),
            _ => {
                self.prev_theta_deg = Some(theta_deg);
                self.prev_t = Some(t);

                let mut out = BellowsOutput::default();
                out.theta_deg = theta_deg;
                out.a = self.a;
                out.speed_smooth = self.speed_smooth;
                return out;
            }
        };

        let dt_sec = (t - prev_t).as_secs_f32();
        // Safety: if dt is too small (or 0), avoid division noise.
        if dt_sec <= 0.000_001 {
            let mut out = BellowsOutput::default();
            out.theta_deg = theta_deg;
            out.a = self.a;
            out.speed_smooth = self.speed_smooth;
            return out;
        }

        // 1) Angular velocity (deg/s)
        let omega = (theta_deg - prev_theta) / dt_sec;

        // 2) Bellows cares about magnitude (direction doesn't matter)
        let speed_raw = omega.abs();

        // 3) Smooth speed (EMA)
        let alpha = clamp01(self.params.ema_alpha);
        self.speed_smooth = ema(self.speed_smooth, speed_raw, alpha);

        // 4) Deadzone + normalize to 0..1
        let x = normalize_with_deadzone(
            self.speed_smooth,
            self.params.deadzone_deg_per_s,
            self.params.vmax_deg_per_s,
        );

        // 5) Curve shaping
        let gamma = if self.params.gamma <= 0.0 { 1.0 } else { self.params.gamma };
        let a_target = x.powf(gamma);

        // 6) Attack/Release envelope (smooth changes in amplitude)
        self.a = envelope_follow(self.a, a_target, dt_sec, self.params.attack_ms, self.params.release_ms);

        // Store current as previous
        self.prev_theta_deg = Some(theta_deg);
        self.prev_t = Some(t);

        BellowsOutput {
            dt_sec,
            theta_deg,
            omega_deg_per_s: omega,
            speed_raw,
            speed_smooth: self.speed_smooth,
            a_target,
            a: self.a,
        }
    }

    /// Handy for debugging / calibration buttons later.
    pub fn reset(&mut self) {
        self.prev_theta_deg = None;
        self.prev_t = None;
        self.speed_smooth = 0.0;
        self.a = 0.0;
    }
}

/* ----------------- helper functions ----------------- */

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Exponential moving average.
/// new = prev + alpha*(input - prev)
fn ema(prev: f32, input: f32, alpha: f32) -> f32 {
    prev + alpha * (input - prev)
}

/// Convert speed into [0..1] using deadzone and vmax.
/// Anything below deadzone becomes 0.
/// Anything above vmax becomes 1.
/// Between them becomes linear.
fn normalize_with_deadzone(speed: f32, deadzone: f32, vmax: f32) -> f32 {
    let dead = deadzone.max(0.0);

    // If vmax is not greater than deadzone, treat as "always max" after deadzone.
    let maxv = vmax.max(dead + 0.000_1);

    let x = (speed - dead) / (maxv - dead);
    clamp01(x)
}

/// Smoothly move current amplitude toward target using different time constants for up vs down.
/// Uses a simple one-pole filter:
///   step = 1 - exp(-dt/tau)
///   current += (target-current)*step
fn envelope_follow(current: f32, target: f32, dt_sec: f32, attack_ms: f32, release_ms: f32) -> f32 {
    let going_up = target > current;

    let ms = if going_up { attack_ms } else { release_ms };
    // If ms is 0 or negative, change immediately.
    if ms <= 0.0 {
        return target;
    }

    let tau = ms / 1000.0; // ms -> seconds
    let step = 1.0 - (-dt_sec / tau).exp();
    current + (target - current) * step
}
