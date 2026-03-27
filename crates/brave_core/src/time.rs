use std::time::{Duration, Instant};

pub const FIXED_DT: f32 = 1.0 / 60.0;

pub struct Time {
    last_frame: Instant,
    start: Instant,
    pub(crate) delta: f32,
    pub(crate) elapsed: f64,
    frame_budget: Duration,
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_frame: now,
            start: now,
            delta: 0.0,
            elapsed: 0.0,
            frame_budget: Duration::from_secs_f64(1.0 / 165.0),
        }
    }

    pub(crate) fn tick(&mut self) -> f32 {
        let now = Instant::now();
        self.delta = now.duration_since(self.last_frame).as_secs_f32();
        self.elapsed = now.duration_since(self.start).as_secs_f64();
        self.last_frame = now;
        self.delta
    }

    pub(crate) fn throttle(&self) {
        let elapsed = self.last_frame.elapsed();
        if elapsed < self.frame_budget {
            std::thread::sleep(self.frame_budget - elapsed);
        }
    }

    pub fn set_fps_limit(&mut self, fps: f64) {
        self.frame_budget = Duration::from_secs_f64(1.0 / fps);
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }

    pub fn fixed_delta(&self) -> f32 {
        FIXED_DT
    }

    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}
