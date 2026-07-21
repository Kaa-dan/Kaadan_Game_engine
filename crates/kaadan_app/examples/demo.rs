//! KaadanEngine demo: a windowed scene of batched sprites with a player sprite
//! moved by the arrow keys. Run with `cargo run -p kaadan_app --example demo`.

use kaadan_app::Engine;
use kaadan_ecs::{Resources, Time, World};
use kaadan_input::InputState;
use kaadan_math::{Color, Quat, Transform, Vec2, Vec3};
use kaadan_platform::{run, KeyCode, WindowConfig};
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, Sprite};

/// Marker component for the player-controlled sprite.
struct Player;

/// Marker component for the spinning 3D cube.
struct Spinner;

fn checkerboard(size: u32, cells: u32) -> Vec<u8> {
    let cell = (size / cells).max(1);
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let on = ((x / cell) + (y / cell)) % 2 == 0;
            let (r, g, b) = if on { (235, 235, 245) } else { (70, 70, 95) };
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    pixels
}

fn main() {
    kaadan_app::kaadan_core::init_logging();

    let config = WindowConfig {
        title: "KaadanEngine Demo".to_string(),
        width: 1280,
        height: 720,
        resizable: true,
    };

    let engine = Engine::new(60)
        .with_clear_color(Color::new(0.02, 0.02, 0.04, 1.0))
        .on_init(|setup| {
            // 3D: a lit cube at the origin, a directional light, and a camera
            // pulled back to frame it.
            let cube = setup.create_cube(1.0);
            setup.world.spawn((
                Mesh3D::new(cube),
                Transform::from_position(Vec3::ZERO),
                PbrMaterial {
                    base_color: Color::from_hex(0x4477FF),
                    metallic: 0.1,
                    roughness: 0.35,
                    ..Default::default()
                },
                Spinner,
            ));
            setup.world.spawn((DirectionalLight {
                direction: Vec3::new(-0.4, -1.0, -0.6).normalize(),
                color: Color::WHITE,
                intensity: 1.2,
            },));
            if let Some(camera) = setup.camera3d() {
                camera.position = Vec3::new(0.0, 2.5, 6.0);
                camera.target = Vec3::ZERO;
            }

            // 2D overlay: a grid of HUD sprites plus a movable player sprite.
            let texture = setup.create_texture_rgba(&checkerboard(64, 8), 64, 64, "checker");
            for i in 0..12 {
                let col = (i % 6) as f32 - 2.5;
                let mut sprite = Sprite::new(texture);
                sprite.size = Some(Vec2::splat(48.0));
                sprite.color = Color::from_hex(0x66FFAA);
                setup
                    .world
                    .spawn((sprite, Transform::from_position_2d(col * 70.0, -300.0)));
            }
            let mut player = Sprite::new(texture);
            player.size = Some(Vec2::splat(72.0));
            player.color = Color::WHITE;
            player.z_order = 10;
            setup
                .world
                .spawn((player, Transform::from_position_2d(0.0, 280.0), Player));
        })
        .add_system("spin_cube", spin_cube)
        .add_system("move_player", move_player)
        .add_system("log_fps", log_fps);

    run(config, engine);
}

fn move_player(world: &mut World, resources: &mut Resources) {
    let mut dir = Vec2::ZERO;
    if let Some(input) = resources.get::<InputState>() {
        if input.key_pressed(KeyCode::ArrowLeft) {
            dir.x -= 1.0;
        }
        if input.key_pressed(KeyCode::ArrowRight) {
            dir.x += 1.0;
        }
        if input.key_pressed(KeyCode::ArrowUp) {
            dir.y += 1.0;
        }
        if input.key_pressed(KeyCode::ArrowDown) {
            dir.y -= 1.0;
        }
    }

    if dir == Vec2::ZERO {
        return;
    }

    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(1.0 / 60.0);
    let velocity = dir.normalize() * 350.0 * dt;

    for (_e, (_player, transform)) in world.query::<(&Player, &mut Transform)>().iter() {
        transform.position.x += velocity.x;
        transform.position.y += velocity.y;
    }
}

fn spin_cube(world: &mut World, resources: &mut Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(1.0 / 60.0);
    for (_e, (_spinner, transform)) in world.query::<(&Spinner, &mut Transform)>().iter() {
        transform.rotation *= Quat::from_rotation_y(0.8 * dt) * Quat::from_rotation_x(0.3 * dt);
    }
}

fn log_fps(_world: &mut World, resources: &mut Resources) {
    if let Some(time) = resources.get::<Time>() {
        if time.frame_count() % 120 == 0 {
            tracing::info!(
                "frame {} — {:.1}ms",
                time.frame_count(),
                time.delta_seconds() * 1000.0
            );
        }
    }
}
