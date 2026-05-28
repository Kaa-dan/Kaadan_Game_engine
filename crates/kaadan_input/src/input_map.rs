use kaadan_platform::KeyCode;
use std::collections::HashMap;

/// A physical input that can be bound to an action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputBinding {
    Key(KeyCode),
    Tap,
    DoubleTap,
    SwipeUp,
    SwipeDown,
    SwipeLeft,
    SwipeRight,
    PinchIn,
    PinchOut,
    GamepadButton(u32),
    GamepadAxis { axis: u32, positive: bool },
}

/// Maps physical inputs to named actions.
pub struct InputMap {
    bindings: HashMap<String, Vec<InputBinding>>,
}

impl Default for InputMap {
    fn default() -> Self {
        Self::new()
    }
}

impl InputMap {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, action: impl Into<String>, binding: InputBinding) -> &mut Self {
        self.bindings
            .entry(action.into())
            .or_default()
            .push(binding);
        self
    }

    pub fn bindings_for(&self, action: &str) -> &[InputBinding] {
        self.bindings.get(action).map_or(&[], |v| v.as_slice())
    }
}
