use kaadan_math::Handle;

use crate::server::AssetServer;

type LoadedCheck = Box<dyn Fn(&AssetServer) -> bool + Send + Sync>;

/// Tracks a set of pending asset loads so a loading screen can report progress.
#[derive(Default)]
pub struct AssetGroup {
    checks: Vec<LoadedCheck>,
}

impl AssetGroup {
    pub fn new() -> Self {
        Self::default()
    }

    /// Track a handle; counts toward progress once its asset is loaded.
    pub fn add<T: Send + Sync + 'static>(&mut self, handle: Handle<T>) {
        self.checks
            .push(Box::new(move |server| server.is_loaded::<T>(handle)));
    }

    /// Fraction of tracked assets that are loaded, in `0.0..=1.0`.
    pub fn progress(&self, server: &AssetServer) -> f32 {
        if self.checks.is_empty() {
            return 1.0;
        }
        let loaded = self.checks.iter().filter(|c| c(server)).count();
        loaded as f32 / self.checks.len() as f32
    }

    pub fn is_complete(&self, server: &AssetServer) -> bool {
        self.checks.iter().all(|c| c(server))
    }
}
