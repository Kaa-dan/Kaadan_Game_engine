use std::time::{Duration, Instant};

/// Adaptive frame pacing for consistent frame times.
pub struct FramePacer {
    target_fps: u32,
    frame_budget: Duration,
    last_frame_start: Instant,
}

impl FramePacer {
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            frame_budget: Duration::from_secs_f64(1.0 / target_fps as f64),
            last_frame_start: Instant::now(),
        }
    }

    /// Call at the start of each frame.
    pub fn begin_frame(&mut self) {
        self.last_frame_start = Instant::now();
    }

    /// Call at the end of each frame. Returns how long until the next frame should start.
    pub fn end_frame(&self) -> Duration {
        let elapsed = self.last_frame_start.elapsed();
        self.frame_budget.saturating_sub(elapsed)
    }

    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }

    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps;
        self.frame_budget = Duration::from_secs_f64(1.0 / fps as f64);
    }
}

/// Frame statistics for profiling.
#[derive(Debug, Default)]
pub struct FrameStats {
    pub frame_time_ms: f32,
    pub fps: f32,
    frame_times: Vec<f32>,
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(120),
            ..Default::default()
        }
    }

    pub fn record_frame(&mut self, dt: f32) {
        self.frame_time_ms = dt * 1000.0;
        self.frame_times.push(dt);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }
        let avg = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        self.fps = if avg > 0.0 { 1.0 / avg } else { 0.0 };
    }

    pub fn average_fps(&self) -> f32 {
        self.fps
    }

    pub fn average_frame_time_ms(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        (self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32) * 1000.0
    }
}
