//! End-to-end hot-reload test.
//!
//! This builds the `game_template` cdylib with `cargo`, loads it through a
//! [`ScriptHost`], and verifies the plugin's `spin` system actually runs and
//! rotates a `Mesh3D` + `Transform` entity.
//!
//! It is `#[ignore]`d because building a crate from inside a test is slow and
//! fragile (it shells out to `cargo`, depends on the workspace layout, and
//! contends for the target dir). CI runs the *static-link* equivalent in
//! `templates/game_template` instead. Run this manually with:
//!
//! ```sh
//! cargo test -p kaadan_script -- --ignored
//! ```

#![cfg(feature = "hot_reload")]

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use kaadan_ecs::{App, Time};
use kaadan_math::{HandleAllocator, Quat, Transform};
use kaadan_renderer::{Mesh3D, Mesh3DGpu};
use kaadan_script::ScriptHost;

/// Platform-specific cdylib file name for `game_template`.
fn cdylib_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "game_template.dll"
    } else if cfg!(target_os = "macos") {
        "libgame_template.dylib"
    } else {
        "libgame_template.so"
    }
}

/// Workspace root = two levels up from this crate's manifest dir.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate is nested under <workspace>/crates/")
        .to_path_buf()
}

#[test]
#[ignore = "builds game_template via cargo; slow/fragile for CI. Run with --ignored"]
fn hot_reload_loads_and_runs_plugin() {
    let root = workspace_root();

    // 1. Build the gameplay cdylib.
    let status = Command::new(env!("CARGO"))
        .current_dir(&root)
        .args(["build", "-p", "game_template"])
        .status()
        .expect("failed to spawn cargo");
    assert!(status.success(), "cargo build -p game_template failed");

    // 2. Locate the produced cdylib under target/debug/.
    let dylib = root.join("target").join("debug").join(cdylib_name());
    assert!(
        dylib.exists(),
        "expected cdylib at {} — adjust target dir if CARGO_TARGET_DIR is set",
        dylib.display()
    );

    // 3. Load it into a fresh App.
    let mut app = App::new();
    let mut host = ScriptHost::new(&dylib);
    host.load(&mut app).expect("ScriptHost::load failed");
    assert!(
        !host.plugin_systems().is_empty(),
        "plugin registered no systems"
    );

    // 4. Spawn a Mesh3D + Transform entity (no GPU needed: the handle is a
    //    plain id, and the test never dereferences the mesh).
    let mut alloc: HandleAllocator<Mesh3DGpu> = HandleAllocator::new();
    let e = app
        .world
        .spawn((Mesh3D::new(alloc.allocate()), Transform::IDENTITY));

    // 5. Drive a frame with a known delta so `spin` produces a deterministic,
    //    non-identity rotation.
    app.resources
        .get_mut::<Time>()
        .unwrap()
        .advance(Duration::from_millis(16));
    app.tick();

    let rot = app.world.get::<Transform>(e).unwrap().rotation;
    assert_ne!(rot, Quat::IDENTITY, "spin did not rotate the entity");
}
