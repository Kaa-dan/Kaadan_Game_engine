//! Android entry point for a KaadanEngine game.
//!
//! `android_main` is the symbol the `android-activity` native-activity glue
//! calls once the Android `NativeActivity` starts. It builds an [`Engine`],
//! spawns a minimal 3D scene (a lit cube), statically links the gameplay crate
//! ([`game_template`], which attaches a `Spinner` behaviour to mesh entities),
//! and hands control to the Android platform backend.
//!
//! This uses the **static-link** gameplay path (`with_static_game`) — the
//! shipping model for mobile, with no dylib hot-reload (iOS forbids it and it is
//! unnecessary on a device).
//!
//! On non-Android targets this crate is an empty library so it can participate
//! in the desktop `cargo build --workspace` and catch breakage early.

#[cfg(target_os = "android")]
use kaadan_app::Engine;
#[cfg(target_os = "android")]
use kaadan_math::{Color, Transform, Vec3};
#[cfg(target_os = "android")]
use kaadan_platform::{AndroidApp, WindowConfig};
#[cfg(target_os = "android")]
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial};
#[cfg(target_os = "android")]
use kaadan_ui::{UiNode, UiText};

/// Entry point invoked by the Android NativeActivity glue.
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    // Route logs to logcat via the android tracing subscriber.
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .try_init();

    let engine = Engine::new(60)
        .with_clear_color(Color::new(0.05, 0.05, 0.08, 1.0))
        .on_init(|setup| {
            // A unit cube at the origin, lit by a single directional light. The
            // default Camera3D sits at (0, 2, 5) looking at the origin, so the
            // cube is centred on screen.
            let cube = setup.create_cube(0.5);
            setup.world.spawn((
                Mesh3D::new(cube),
                Transform::from_position(Vec3::ZERO),
                PbrMaterial::default(),
            ));
            setup.world.spawn((DirectionalLight::default(),));

            // A UiText label proves the built-in bitmap font renders on-device.
            // A parent-less UiNode lays out as a full-screen root, so the label
            // anchors at the top-left safe area.
            setup
                .world
                .spawn((UiNode::default(), UiText::new("KaadanEngine", 32.0)));
        })
        // game_template::build registers the `Spinner` behaviour and attaches it
        // to every Mesh3D entity, so the cube spins.
        .with_static_game(game_template::build);

    kaadan_platform::run_android(app, WindowConfig::default(), engine);
}
