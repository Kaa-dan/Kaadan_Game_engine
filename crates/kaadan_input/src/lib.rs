//! Input handling for keyboard, mouse, touch, and gamepad.
//!
//! Provides a unified input abstraction with action mapping and gesture recognition.

mod gesture;
mod input_map;
mod input_state;

pub use gesture::{Gesture, GestureRecognizer, SwipeDirection};
pub use input_map::{InputBinding, InputMap};
pub use input_state::{InputState, TouchState};
