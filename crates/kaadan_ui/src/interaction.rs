use kaadan_ecs::{Resources, World};
use kaadan_input::InputState;

use crate::node::{InteractionState, UiNode};
use crate::widgets::UiButton;

/// Hit-test the pointer against laid-out [`UiNode`] rects, updating each
/// interactive node's [`InteractionState`] and mirroring hover/press/click
/// onto any [`UiButton`]. Run after [`crate::ui_layout_system`].
pub fn ui_interaction_system(world: &mut World, resources: &mut Resources) {
    let (pointer, pointer_down) = match resources.get::<InputState>() {
        Some(input) => (input.pointer_position(), !input.touches().is_empty()),
        None => return,
    };

    for (_entity, node) in world.query::<&mut UiNode>().iter() {
        if !node.interactive {
            node.state = InteractionState::None;
            continue;
        }
        let hovered = node.visible && node.computed_rect.contains(pointer);
        node.state = if hovered {
            if pointer_down {
                InteractionState::Pressed
            } else {
                InteractionState::Hovered
            }
        } else {
            InteractionState::None
        };
    }

    for (_entity, (node, button)) in world.query::<(&UiNode, &mut UiButton)>().iter() {
        let hovered = node.visible && node.computed_rect.contains(pointer);
        let was_pressed = button.pressed;
        button.hovered = hovered;
        // Click registers on release while still hovering the button.
        button.clicked = was_pressed && hovered && !pointer_down;
        button.pressed = hovered && pointer_down;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_math::{Rect, Vec2};
    use kaadan_platform::{InputEvent, TouchEvent, TouchPhase};

    fn touch(phase: TouchPhase, x: f32, y: f32) -> InputEvent {
        InputEvent::Touch(TouchEvent {
            id: 0,
            phase,
            position: Vec2::new(x, y),
        })
    }

    #[test]
    fn button_hover_press_click_cycle() {
        let mut world = World::new();
        let mut resources = Resources::new();
        resources.insert(InputState::new());

        let node = UiNode {
            interactive: true,
            computed_rect: Rect::from_position_size(Vec2::ZERO, Vec2::splat(100.0)),
            ..Default::default()
        };
        let entity = world.spawn((node, UiButton::new("Play")));

        // Frame 1: pointer hovers the button, not pressed.
        resources
            .get_mut::<InputState>()
            .unwrap()
            .process_event(&touch(TouchPhase::Moved, 50.0, 50.0));
        ui_interaction_system(&mut world, &mut resources);
        {
            let button = world.get::<UiButton>(entity).unwrap();
            assert!(button.hovered);
            assert!(!button.pressed);
            assert!(!button.clicked);
        }

        // Frame 2: pointer pressed down over the button.
        resources
            .get_mut::<InputState>()
            .unwrap()
            .process_event(&touch(TouchPhase::Started, 50.0, 50.0));
        ui_interaction_system(&mut world, &mut resources);
        assert!(world.get::<UiButton>(entity).unwrap().pressed);

        // Frame 3: release over the button -> click.
        resources
            .get_mut::<InputState>()
            .unwrap()
            .process_event(&touch(TouchPhase::Ended, 50.0, 50.0));
        ui_interaction_system(&mut world, &mut resources);
        {
            let button = world.get::<UiButton>(entity).unwrap();
            assert!(button.clicked);
            assert!(!button.pressed);
        }
    }
}
