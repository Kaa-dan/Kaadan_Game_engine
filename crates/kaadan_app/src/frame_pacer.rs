use std::time::{Duration, Instant};

/// Device thermal pressure. Higher pressure lowers the recommended frame rate
/// to reduce heat and battery drain on mobile devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl ThermalState {
    /// Recommended target frame rate for this thermal state.
    pub fn recommended_fps(self) -> u32 {
        match self {
            ThermalState::Nominal => 60,
            ThermalState::Fair => 45,
            ThermalState::Serious => 30,
            ThermalState::Critical => 20,
        }
    }
}

/// Adaptive frame pacing for consistent frame times, with thermal awareness.
pub struct FramePacer {
    target_fps: u32,
    frame_budget: Duration,
    last_frame_start: Instant,
    thermal_state: ThermalState,
}

impl FramePacer {
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            frame_budget: Duration::from_secs_f64(1.0 / target_fps as f64),
            last_frame_start: Instant::now(),
            thermal_state: ThermalState::Nominal,
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
        self.target_fps = fps.max(1);
        self.frame_budget = Duration::from_secs_f64(1.0 / self.target_fps as f64);
    }

    pub fn thermal_state(&self) -> ThermalState {
        self.thermal_state
    }

    /// Update thermal pressure and adapt the target frame rate accordingly.
    pub fn set_thermal_state(&mut self, state: ThermalState) {
        self.thermal_state = state;
        self.set_target_fps(state.recommended_fps());
    }
}

/// Frame statistics for profiling and on-screen debug overlays.
#[derive(Debug, Default)]
pub struct FrameStats {
    pub frame_time_ms: f32,
    pub fps: f32,
    pub draw_calls: u32,
    pub triangles: u32,
    pub gpu_memory_mb: f32,
    pub cpu_memory_mb: f32,
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

    /// Record per-frame render counters (call after submitting draw work).
    pub fn record_render(&mut self, draw_calls: u32, triangles: u32) {
        self.draw_calls = draw_calls;
        self.triangles = triangles;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thermal_state_lowers_target_fps() {
        let mut pacer = FramePacer::new(60);
        assert_eq!(pacer.target_fps(), 60);
        pacer.set_thermal_state(ThermalState::Serious);
        assert_eq!(pacer.target_fps(), 30);
        assert_eq!(pacer.thermal_state(), ThermalState::Serious);
        pacer.set_thermal_state(ThermalState::Critical);
        assert_eq!(pacer.target_fps(), 20);
    }

    #[test]
    fn frame_stats_track_fps_and_render_counters() {
        let mut stats = FrameStats::new();
        for _ in 0..10 {
            stats.record_frame(1.0 / 60.0);
        }
        assert!(stats.fps > 55.0 && stats.fps < 65.0);
        stats.record_render(12, 2048);
        assert_eq!(stats.draw_calls, 12);
        assert_eq!(stats.triangles, 2048);
    }
}
