use kaadan_math::Vec2;
use kaadan_platform::{TouchEvent, TouchPhase};
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Gesture {
    Tap {
        position: Vec2,
    },
    DoubleTap {
        position: Vec2,
    },
    Swipe {
        direction: SwipeDirection,
        velocity: f32,
        start: Vec2,
        end: Vec2,
    },
    Pinch {
        scale: f32,
        center: Vec2,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

struct TrackedTouch {
    id: u64,
    start_position: Vec2,
    start_time: Instant,
    current_position: Vec2,
}

/// Recognizes gestures from raw touch events.
pub struct GestureRecognizer {
    swipe_threshold: f32,
    double_tap_window: f32,
    active_touches: Vec<TrackedTouch>,
    last_tap: Option<(Instant, Vec2)>,
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureRecognizer {
    pub fn new() -> Self {
        Self {
            swipe_threshold: 50.0,
            double_tap_window: 0.3,
            active_touches: Vec::new(),
            last_tap: None,
        }
    }

    /// Process a touch event. Returns recognized gestures (if any).
    pub fn process_touch(&mut self, touch: &TouchEvent) -> Vec<Gesture> {
        let mut gestures = Vec::new();
        match touch.phase {
            TouchPhase::Started => {
                self.active_touches.push(TrackedTouch {
                    id: touch.id,
                    start_position: touch.position,
                    start_time: Instant::now(),
                    current_position: touch.position,
                });
            }
            TouchPhase::Moved => {
                if let Some(t) = self.active_touches.iter_mut().find(|t| t.id == touch.id) {
                    t.current_position = touch.position;
                }
            }
            TouchPhase::Ended => {
                if let Some(t) = self.active_touches.iter().find(|t| t.id == touch.id) {
                    let delta = t.current_position - t.start_position;
                    let distance = delta.length();
                    let duration = t.start_time.elapsed().as_secs_f32();

                    if distance < self.swipe_threshold && duration < 0.5 {
                        self.recognize_tap(t.current_position, &mut gestures);
                    } else if distance >= self.swipe_threshold && duration > 0.0 {
                        let direction = if delta.x.abs() > delta.y.abs() {
                            if delta.x > 0.0 {
                                SwipeDirection::Right
                            } else {
                                SwipeDirection::Left
                            }
                        } else if delta.y > 0.0 {
                            SwipeDirection::Down
                        } else {
                            SwipeDirection::Up
                        };
                        gestures.push(Gesture::Swipe {
                            direction,
                            velocity: distance / duration,
                            start: t.start_position,
                            end: t.current_position,
                        });
                    }
                }
                self.active_touches.retain(|t| t.id != touch.id);
            }
            TouchPhase::Cancelled => {
                self.active_touches.retain(|t| t.id != touch.id);
            }
        }
        gestures
    }

    fn recognize_tap(&mut self, position: Vec2, gestures: &mut Vec<Gesture>) {
        if let Some((last_time, last_pos)) = &self.last_tap {
            if last_time.elapsed().as_secs_f32() < self.double_tap_window
                && (position - *last_pos).length() < 30.0
            {
                gestures.push(Gesture::DoubleTap { position });
                self.last_tap = None;
                return;
            }
        }
        gestures.push(Gesture::Tap { position });
        self.last_tap = Some((Instant::now(), position));
    }
}
