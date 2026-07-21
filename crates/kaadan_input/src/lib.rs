//! Input handling for keyboard, mouse, touch, and gamepad.
//!
//! Provides a unified input abstraction with action mapping and gesture recognition.

mod gesture;
mod input_map;
mod input_state;

pub use gesture::{Gesture, GestureRecognizer, SwipeDirection};
pub use input_map::{InputBinding, InputMap};
pub use input_state::{InputState, TouchState};

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_math::Vec2;
    use kaadan_platform::{InputEvent, KeyCode, KeyEvent, TouchEvent, TouchPhase};

    fn touch(id: u64, phase: TouchPhase, x: f32, y: f32) -> TouchEvent {
        TouchEvent {
            id,
            phase,
            position: Vec2::new(x, y),
        }
    }

    #[test]
    fn recognizes_tap() {
        let mut recognizer = GestureRecognizer::new();
        recognizer.process_touch(&touch(1, TouchPhase::Started, 10.0, 10.0));
        let gestures = recognizer.process_touch(&touch(1, TouchPhase::Ended, 12.0, 11.0));
        assert!(gestures.iter().any(|g| matches!(g, Gesture::Tap { .. })));
    }

    #[test]
    fn recognizes_rightward_swipe() {
        let mut recognizer = GestureRecognizer::new();
        recognizer.process_touch(&touch(1, TouchPhase::Started, 0.0, 0.0));
        recognizer.process_touch(&touch(1, TouchPhase::Moved, 120.0, 5.0));
        let gestures = recognizer.process_touch(&touch(1, TouchPhase::Ended, 120.0, 5.0));
        assert!(gestures.iter().any(|g| matches!(
            g,
            Gesture::Swipe {
                direction: SwipeDirection::Right,
                ..
            }
        )));
    }

    #[test]
    fn input_map_binds_action() {
        let mut map = InputMap::new();
        map.bind("jump", InputBinding::Key(KeyCode::Space));
        assert_eq!(map.bindings_for("jump").len(), 1);
        assert!(map.bindings_for("missing").is_empty());
    }

    #[test]
    fn action_pressed_via_key() {
        let mut map = InputMap::new();
        map.bind("jump", InputBinding::Key(KeyCode::Space));
        let mut state = InputState::new();
        state.process_event(&InputEvent::Key(KeyEvent {
            key: KeyCode::Space,
            pressed: true,
        }));
        assert!(state.action_pressed("jump", &map));
        assert!(state.action_just_pressed("jump", &map));
    }
}
