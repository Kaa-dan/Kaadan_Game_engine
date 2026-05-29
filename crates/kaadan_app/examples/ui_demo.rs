//! In-game UI demo: a centered panel containing an interactive button and an
//! animated progress bar, drawn by the engine's `UiRenderer`.
//! Run with `cargo run -p kaadan_app --example ui_demo`.
//!
//! Note: text glyphs are not drawn yet (pending a bundled font), so widgets
//! appear as colored quads. Hover/press the button to see it tint.

use kaadan_app::Engine;
use kaadan_ecs::{Resources, Time, World};
use kaadan_math::Color;
use kaadan_platform::{run, WindowConfig};
use kaadan_scene::set_parent;
use kaadan_ui::{
    AlignItems, FlexDirection, JustifyContent, UiButton, UiNode, UiProgressBar, UiScreen, UiStyle,
};

/// Marker for the animated progress bar.
struct Animated;

fn main() {
    kaadan_app::kaadan_core::init_logging();

    let config = WindowConfig {
        title: "KaadanEngine UI Demo".to_string(),
        width: 1280,
        height: 720,
        resizable: true,
    };

    let engine = Engine::new(60)
        .with_clear_color(Color::new(0.04, 0.05, 0.08, 1.0))
        .on_init(|setup| {
            let (sw, sh) = setup
                .resources
                .get::<UiScreen>()
                .map(|s| (s.size.x, s.size.y))
                .unwrap_or((1280.0, 720.0));

            // Full-screen root that centers its child panel.
            let root = setup.world.spawn((UiNode {
                style: UiStyle {
                    width: sw,
                    height: sh,
                    direction: FlexDirection::Column,
                    justify: JustifyContent::Center,
                    align: AlignItems::Center,
                    ..Default::default()
                },
                background: Color::TRANSPARENT,
                ..Default::default()
            },));

            // Panel.
            let panel = setup.world.spawn((UiNode {
                style: UiStyle {
                    width: 360.0,
                    height: 240.0,
                    padding: kaadan_ui::UiEdges::all(20.0),
                    direction: FlexDirection::Column,
                    justify: JustifyContent::SpaceBetween,
                    align: AlignItems::Center,
                    ..Default::default()
                },
                background: Color::new(0.12, 0.13, 0.18, 0.95),
                ..Default::default()
            },));

            // Interactive button.
            let button = setup.world.spawn((
                UiNode {
                    style: UiStyle {
                        width: 220.0,
                        height: 56.0,
                        ..Default::default()
                    },
                    background: Color::from_hex(0x3366CC),
                    interactive: true,
                    ..Default::default()
                },
                UiButton::new("Click me"),
            ));

            // Animated progress bar.
            let bar = setup.world.spawn((
                UiNode {
                    style: UiStyle {
                        width: 260.0,
                        height: 22.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                UiProgressBar::new(0.25),
                Animated,
            ));

            set_parent(setup.world, panel, root);
            set_parent(setup.world, button, panel);
            set_parent(setup.world, bar, panel);
        })
        .add_system("animate_bar", animate_bar);

    run(config, engine);
}

fn animate_bar(world: &mut World, resources: &mut Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(1.0 / 60.0);
    for (_e, (_marker, bar)) in world.query::<(&Animated, &mut UiProgressBar)>().iter() {
        bar.progress = (bar.progress + dt * 0.2).fract();
    }
}
