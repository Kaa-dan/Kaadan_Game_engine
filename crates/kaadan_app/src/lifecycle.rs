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

/// Tracks application lifecycle transitions (foreground/background/low-memory),
/// which on mobile must drive surface recreation and resource release.
#[derive(Debug, Default)]
pub struct LifecycleManager {
    state: AppLifecycleState,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> AppLifecycleState {
        self.state
    }

    /// App moved to the background. Callers should pause audio and release the
    /// GPU surface (it becomes invalid on Android).
    pub fn on_suspend(&mut self) {
        self.state = AppLifecycleState::Suspended;
        tracing::info!("app suspended");
    }

    /// App returned to the foreground. Callers should recreate the surface.
    pub fn on_resume(&mut self) {
        self.state = AppLifecycleState::Active;
        tracing::info!("app resumed");
    }

    /// OS low-memory warning. Callers should drop non-essential caches.
    pub fn on_low_memory(&mut self) {
        tracing::warn!("low memory warning — free caches");
    }

    pub fn on_exit(&mut self) {
        self.state = AppLifecycleState::Exiting;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_transitions() {
        let mut manager = LifecycleManager::new();
        assert_eq!(manager.state(), AppLifecycleState::Active);
        manager.on_suspend();
        assert_eq!(manager.state(), AppLifecycleState::Suspended);
        manager.on_resume();
        assert_eq!(manager.state(), AppLifecycleState::Active);
        manager.on_exit();
        assert_eq!(manager.state(), AppLifecycleState::Exiting);
    }
}
