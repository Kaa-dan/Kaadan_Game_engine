use kaadan_math::Vec2;

/// Unified input event consumed identically on all platforms.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Touch began/moved/ended (mobile) or mouse click (desktop)
    Touch(TouchEvent),
    /// Physical key press/release
    Key(KeyEvent),
    /// Window resized
    Resize { width: u32, height: u32 },
    /// App lifecycle change
    Lifecycle(LifecycleEvent),
    /// Window close requested
    CloseRequested,
}

#[derive(Debug, Clone)]
pub struct TouchEvent {
    pub id: u64,
    pub phase: TouchPhase,
    pub position: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub pressed: bool,
}

/// Subset of common key codes. Expand as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Space,
    Enter,
    Escape,
    Backspace,
    Tab,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    Unknown,
}
