//! Play mode: runs the real gameplay runtime against the editor's scene.
//!
//! Pressing Play builds a [`PlaySession`] that owns a runtime [`App`] and a
//! [`ScriptHost`] pointed at the gameplay crate's built cdylib. The session
//! loads the plugin — which attaches Unity-style behaviours to the scene's
//! entities — and then ticks it each frame, hot-reloading the dylib whenever a
//! rebuild lands. Stop drops the session and restores the pre-play snapshot.
//!
//! The scene world lives in the editor's [`Viewport`](crate::viewport::Viewport)
//! so all the panels/gizmos keep operating on it. The session borrows that world
//! by swapping it into its `App` only for the duration of `load`/`tick`, then
//! swaps it back — the world is never copied and the panels never see it move.

use std::path::{Path, PathBuf};

use kaadan_ecs::{App, World};
use kaadan_script::ScriptHost;

#[derive(Clone, Copy)]
pub enum PlayRequest {
    Start,
    Stop,
}

/// The live runtime backing Play mode: a runtime [`App`] (schedule + resources)
/// plus an optional hot-reload [`ScriptHost`]. `None` host means the gameplay
/// dylib wasn't built yet — play still runs, just without user scripts.
pub struct PlaySession {
    app: App,
    host: Option<ScriptHost>,
}

impl PlaySession {
    /// Start a session against `world`. Builds a runtime `App`, then (if the
    /// gameplay dylib for `crate_dir` exists) loads it, which registers the
    /// behaviour driver and attaches behaviours to the scene's current entities.
    ///
    /// `world` is swapped into the `App` for the load and swapped back out, so
    /// on return the editor's world holds the freshly attached behaviours.
    pub fn start(world: &mut World, crate_dir: &Path) -> Self {
        let mut app = App::new();
        std::mem::swap(world, &mut app.world);

        let host = match game_dylib_path(crate_dir) {
            Some(path) if path.exists() => {
                let mut host = ScriptHost::new(path);
                match host.load(&mut app) {
                    Ok(()) => {
                        tracing::info!(
                            "Play: loaded gameplay plugin ({} systems)",
                            host.plugin_systems().len()
                        );
                        Some(host)
                    }
                    Err(e) => {
                        tracing::error!("Play: failed to load gameplay plugin: {e}");
                        None
                    }
                }
            }
            Some(path) => {
                tracing::warn!(
                    "Play: gameplay dylib not built yet ({}); press Build. Running without scripts.",
                    path.display()
                );
                None
            }
            None => {
                tracing::warn!(
                    "Play: could not resolve gameplay dylib path; running without scripts"
                );
                None
            }
        };

        std::mem::swap(world, &mut app.world);
        Self { app, host }
    }

    /// Advance the running scene by one frame: hot-reload the gameplay dylib if
    /// it changed on disk, then run the runtime's systems (behaviour `start` /
    /// `update`) against `world`.
    pub fn tick(&mut self, world: &mut World) {
        std::mem::swap(world, &mut self.app.world);
        if let Some(host) = self.host.as_mut() {
            host.poll(&mut self.app);
        }
        self.app.tick();
        std::mem::swap(world, &mut self.app.world);
    }
}

/// Resolve the built cdylib path for the gameplay crate at `crate_dir`.
///
/// Mirrors cargo's default layout: `<workspace>/target/debug/<platform-name>`,
/// where the workspace root is the crate dir's grandparent (crate lives at
/// `<workspace>/templates/<pkg>`) and the file name is the platform's cdylib
/// convention for the package name.
fn game_dylib_path(crate_dir: &Path) -> Option<PathBuf> {
    let package = crate_dir.file_name()?.to_str()?;
    let workspace_root = crate_dir.parent()?.parent()?;
    let file_name = if cfg!(target_os = "windows") {
        format!("{package}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{package}.dylib")
    } else {
        format!("lib{package}.so")
    };
    Some(workspace_root.join("target").join("debug").join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dylib_path_follows_cargo_layout() {
        let crate_dir = Path::new("/ws/templates/game_template");
        let path = game_dylib_path(crate_dir).expect("path resolves");
        assert!(path.starts_with("/ws/target/debug"));
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name.contains("game_template"),
            "file name should mention the package: {name}"
        );
    }

    /// With no gameplay dylib present the session starts host-less, and the
    /// world-swap round-trip in `start`/`tick` leaves the caller's world intact
    /// (entities preserved, world handed back). Guards the `mem::swap` logic.
    #[test]
    fn session_preserves_world_without_dylib() {
        use kaadan_math::Transform;

        let mut world = World::new();
        let e = world.spawn((Transform::IDENTITY,));

        // A crate dir whose dylib can't exist -> host is None.
        let mut session = PlaySession::start(&mut world, Path::new("/no/such/templates/game"));
        assert!(session.host.is_none());
        assert!(world.is_alive(e), "world must be swapped back after start");

        session.tick(&mut world);
        assert!(world.is_alive(e), "world must be swapped back after tick");
        assert_eq!(world.len(), 1, "no entities gained or lost");
    }
}
