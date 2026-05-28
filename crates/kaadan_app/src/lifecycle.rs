/// Application lifecycle state, especially important for mobile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppLifecycleState {
    /// App is running in the foreground.
    #[default]
    Active,
    /// App is in the background — release GPU resources, pause audio.
    Suspended,
    /// App is about to be terminated.
    Exiting,
}
