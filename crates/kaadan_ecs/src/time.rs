use std::time::{Duration, Instant};

/// Frame timing resource — inserted by App, updated each frame.
pub struct Time {
    startup: Instant,
    last_frame: Instant,
    delta: Duration,
    frame_count: u64,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            startup: now,
            last_frame: now,
            delta: Duration::ZERO,
            frame_count: 0,
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.delta = now - self.last_frame;
        self.last_frame = now;
        self.frame_count += 1;
    }

    /// Delta time in seconds (f32 for game math).
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    pub fn delta(&self) -> Duration {
        self.delta
    }

    pub fn elapsed(&self) -> Duration {
        self.last_frame - self.startup
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}
