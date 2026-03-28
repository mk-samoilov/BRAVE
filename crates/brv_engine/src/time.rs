use std::time::Instant;

pub struct Time {
    delta: f32,
    fixed_delta: f32,
    elapsed: f64,
    fps: f32,
    target_fps: f32,
    last_frame: Instant,
    start: Instant,
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            delta: 0.0,
            fixed_delta: 1.0 / 60.0,
            elapsed: 0.0,
            fps: 0.0,
            target_fps: 144.0,
            last_frame: now,
            start: now,
        }
    }

    pub(crate) fn tick(&mut self) -> f32 {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.elapsed = now.duration_since(self.start).as_secs_f64();
        self.delta = dt;
        self.fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        dt
    }

    pub(crate) fn target_fps(&self) -> f32 {
        self.target_fps
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }

    pub fn fixed_delta(&self) -> f32 {
        self.fixed_delta
    }

    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.target_fps = fps;
    }

    pub fn set_physics_rate(&mut self, rate: f32) {
        self.fixed_delta = 1.0 / rate;
    }
}
