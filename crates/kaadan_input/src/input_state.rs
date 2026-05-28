use kaadan_math::Vec2;
use kaadan_platform::{InputEvent, KeyCode, TouchEvent, TouchPhase};
use std::collections::HashSet;

use crate::gesture::{Gesture, GestureRecognizer};
use crate::input_map::{InputBinding, InputMap};

/// Tracked state for an active touch.
#[derive(Debug, Clone)]
pub struct TouchState {
    pub id: u64,
    pub position: Vec2,
    pub start_position: Vec2,
    pub phase: TouchPhase,
}

/// Per-frame input state, updated from platform events.
/// Inserted as an ECS Resource.
pub struct InputState {
    keys_pressed: HashSet<KeyCode>,
    keys_just_pressed: HashSet<KeyCode>,
    keys_just_released: HashSet<KeyCode>,
    touches: Vec<TouchState>,
    gestures: Vec<Gesture>,
    gesture_recognizer: GestureRecognizer,
    pointer_position: Vec2,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            keys_just_pressed: HashSet::new(),
            keys_just_released: HashSet::new(),
            touches: Vec::new(),
            gestures: Vec::new(),
            gesture_recognizer: GestureRecognizer::new(),
            pointer_position: Vec2::ZERO,
        }
    }

    /// Call at the start of each frame before processing events.
    pub fn begin_frame(&mut self) {
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.gestures.clear();
    }

    /// Process a platform input event.
    pub fn process_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Key(key_event) => {
                if key_event.pressed {
                    if self.keys_pressed.insert(key_event.key) {
                        self.keys_just_pressed.insert(key_event.key);
                    }
                } else {
                    self.keys_pressed.remove(&key_event.key);
                    self.keys_just_released.insert(key_event.key);
                }
            }
            InputEvent::Touch(touch) => {
                self.process_touch(touch);
            }
            _ => {}
        }
    }

    fn process_touch(&mut self, touch: &TouchEvent) {
        let recognized = self.gesture_recognizer.process_touch(touch);
        self.gestures.extend(recognized);

        match touch.phase {
            TouchPhase::Started => {
                self.touches.push(TouchState {
                    id: touch.id,
                    position: touch.position,
                    start_position: touch.position,
                    phase: touch.phase,
                });
            }
            TouchPhase::Moved => {
                if let Some(t) = self.touches.iter_mut().find(|t| t.id == touch.id) {
                    t.position = touch.position;
                    t.phase = touch.phase;
                }
                self.pointer_position = touch.position;
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.touches.retain(|t| t.id != touch.id);
            }
        }
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    pub fn key_just_released(&self, key: KeyCode) -> bool {
        self.keys_just_released.contains(&key)
    }

    pub fn touches(&self) -> &[TouchState] {
        &self.touches
    }

    pub fn gestures(&self) -> &[Gesture] {
        &self.gestures
    }

    pub fn pointer_position(&self) -> Vec2 {
        self.pointer_position
    }

    /// Check if an action is currently active (key held, gesture recognized).
    pub fn action_pressed(&self, action: &str, map: &InputMap) -> bool {
        map.bindings_for(action)
            .iter()
            .any(|binding| match binding {
                InputBinding::Key(key) => self.key_pressed(*key),
                InputBinding::Tap => self
                    .gestures
                    .iter()
                    .any(|g| matches!(g, Gesture::Tap { .. })),
                _ => false,
            })
    }

    /// Check if an action was just triggered this frame.
    pub fn action_just_pressed(&self, action: &str, map: &InputMap) -> bool {
        map.bindings_for(action)
            .iter()
            .any(|binding| match binding {
                InputBinding::Key(key) => self.key_just_pressed(*key),
                InputBinding::Tap => self
                    .gestures
                    .iter()
                    .any(|g| matches!(g, Gesture::Tap { .. })),
                _ => false,
            })
    }
}
