use std::time::{Duration, Instant};

/// Largest delta applied in a single frame. Clamping prevents the fixed-update
/// "spiral of death" after a long stall (e.g. the app was suspended).
const MAX_DELTA: Duration = Duration::from_millis(250);

/// Frame timing resource — inserted by [`App`](crate::App), updated each frame.
///
/// Tracks both a variable frame delta (for rendering / interpolation) and a
/// fixed-timestep accumulator (for deterministic simulation in
/// [`Stage::FixedUpdate`](crate::Stage::FixedUpdate)).
pub struct Time {
    last_frame: Instant,
    delta: Duration,
    elapsed: Duration,
    frame_count: u64,
    fixed_delta: Duration,
    accumulator: Duration,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            frame_count: 0,
            // 60 Hz simulation by default.
            fixed_delta: Duration::from_nanos(1_000_000_000 / 60),
            accumulator: Duration::ZERO,
        }
    }

    /// Advance using the wall clock. Called once per frame by the app loop.
    pub fn update(&mut self) {
        let now = Instant::now();
        let raw = now - self.last_frame;
        self.last_frame = now;
        self.tick(raw);
    }

    /// Advance the clock manually by `delta`, bypassing the wall clock.
    /// Useful for headless / deterministic stepping and tests.
    pub fn advance(&mut self, delta: Duration) {
        self.last_frame = Instant::now();
        self.tick(delta);
    }

    fn tick(&mut self, raw_delta: Duration) {
        self.delta = raw_delta.min(MAX_DELTA);
        self.elapsed += self.delta;
        self.frame_count += 1;
        self.accumulator += self.delta;
    }

    /// Consume one fixed step from the accumulator if enough time has built up.
    /// Returns `true` if a fixed step should run. Call in a loop.
    pub fn expend_fixed_step(&mut self) -> bool {
        if self.accumulator >= self.fixed_delta {
            self.accumulator -= self.fixed_delta;
            true
        } else {
            false
        }
    }

    /// Fraction (0.0..1.0) of a fixed step left in the accumulator — for
    /// interpolating rendered state between fixed updates.
    pub fn fixed_alpha(&self) -> f32 {
        self.accumulator.as_secs_f32() / self.fixed_delta.as_secs_f32()
    }

    /// Set the fixed-update rate in hertz (e.g. 60.0).
    pub fn set_fixed_hz(&mut self, hz: f32) {
        self.fixed_delta = Duration::from_secs_f32(1.0 / hz);
    }

    /// Variable frame delta in seconds (f32 for game math).
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Fixed timestep in seconds — use this inside `FixedUpdate` systems.
    pub fn fixed_delta_seconds(&self) -> f32 {
        self.fixed_delta.as_secs_f32()
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_steps_drain_accumulator() {
        let mut time = Time::new();
        // Advance 3.5 fixed steps -> exactly 3 full steps, 0.5 remainder.
        // Using the actual fixed delta avoids f32 rounding ambiguity.
        let fd = time.fixed_delta_seconds();
        time.advance(Duration::from_secs_f32(fd * 3.5));

        let mut steps = 0;
        while time.expend_fixed_step() {
            steps += 1;
        }
        assert_eq!(steps, 3);
    }

    #[test]
    fn delta_is_clamped() {
        let mut time = Time::new();
        time.advance(Duration::from_secs(10)); // huge stall
        assert!(time.delta() <= MAX_DELTA);
    }

    #[test]
    fn advance_increments_frame_count() {
        let mut time = Time::new();
        time.advance(Duration::from_millis(16));
        time.advance(Duration::from_millis(16));
        assert_eq!(time.frame_count(), 2);
    }
}
