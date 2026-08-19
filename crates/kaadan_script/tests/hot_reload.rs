//! End-to-end hot-reload test.
//!
//! This builds the `game_template` cdylib with `cargo`, loads it through a
//! [`ScriptHost`], and verifies the plugin's `Spinner` behaviour actually runs
//! and rotates the `Mesh3D` + `Transform` entity it is attached to.
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
use kaadan_script::{BehaviourRegistry, ScriptComponent, ScriptHost};

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

    // 3. Spawn a Mesh3D + Transform entity BEFORE loading (no GPU needed: the
    //    handle is a plain id, and the test never dereferences the mesh). The
    //    plugin needs it to exist when the behaviour is attached, so
    //    the entity must exist at load time.
    let mut app = App::new();
    let mut alloc: HandleAllocator<Mesh3DGpu> = HandleAllocator::new();
    let e = app
        .world
        .spawn((Mesh3D::new(alloc.allocate()), Transform::IDENTITY));

    // 4. Load the plugin: passes the ABI check, then registers the behaviour
    //    driver system and the plugin's behaviour *factories*.
    let mut host = ScriptHost::new(&dylib);
    host.load(&mut app).expect("ScriptHost::load failed");
    assert!(
        !host.plugin_systems().is_empty(),
        "plugin registered no systems"
    );

    // 4b. Attach `Spinner` from the registry. `build` deliberately only
    //     registers factories — attachment is data-driven, done by the editor or
    //     a scene loader from per-entity script assignments — so the test plays
    //     that role here.
    let spinner = app
        .resources
        .get::<BehaviourRegistry>()
        .expect(
            "plugin's BehaviourRegistry is not visible to the host.\n\
             The plugin inserts it through `ScriptContext`, but `Resources` is keyed by \
             `TypeId`, and `TypeId::of::<BehaviourRegistry>()` differs between the host \
             binary and the cdylib (verified: kaadan_math's `Transform` matches across the \
             boundary and round-trips, kaadan_script's types do not).\n\
             Consequence: `probe_behaviour_names` returns empty and `attach_scripts` never \
             finds a factory, so no user behaviour attaches in Play mode.\n\
             Fix: hand factories back explicitly through the `ScriptContext` return path \
             (as `take_registered` already does for system names) instead of relying on \
             cross-image `TypeId` equality.",
        )
        .create("Spinner")
        .expect("plugin registered a 'Spinner' factory");
    app.world
        .inner_mut()
        .insert_one(e, ScriptComponent::from_box(spinner))
        .expect("entity is alive");

    // 5. Drive a frame with a known delta so the Spinner produces a
    //    deterministic, non-identity rotation.
    app.resources
        .get_mut::<Time>()
        .unwrap()
        .advance(Duration::from_millis(16));
    app.tick();

    let rot = app.world.get::<Transform>(e).unwrap().rotation;
    assert_ne!(
        rot,
        Quat::IDENTITY,
        "Spinner behaviour did not rotate the entity"
    );
}
