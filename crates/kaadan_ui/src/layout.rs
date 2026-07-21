use std::collections::{HashMap, HashSet};

use kaadan_ecs::{Entity, Resources, World};
use kaadan_math::{Rect, Vec2};
use kaadan_scene::Children;

use crate::node::{AlignItems, FlexDirection, JustifyContent, UiEdges, UiNode, UiStyle};
use crate::screen::UiScreen;

struct NodeInfo {
    style: UiStyle,
    children: Vec<Entity>,
}

/// Compute screen-space rectangles for every [`UiNode`] using a simplified
/// flexbox model (direction / justify / align + padding + margins), respecting
/// the scene [`Children`] hierarchy and the [`UiScreen`] safe area.
pub fn ui_layout_system(world: &mut World, resources: &mut Resources) {
    let (screen_size, safe_area) = resources
        .get::<UiScreen>()
        .map(|s| (s.size, s.safe_area))
        .unwrap_or((Vec2::new(800.0, 600.0), UiEdges::ZERO));

    // Collect node styles + UI children into owned data so we can recurse
    // without holding ECS borrows.
    let mut nodes: HashMap<Entity, NodeInfo> = HashMap::new();
    for (entity, node) in world.query::<&UiNode>().iter() {
        nodes.insert(
            entity,
            NodeInfo {
                style: node.style.clone(),
                children: Vec::new(),
            },
        );
    }
    let mut all_children: HashSet<Entity> = HashSet::new();
    for (entity, children) in world.query::<&Children>().iter() {
        if !nodes.contains_key(&entity) {
            continue;
        }
        let ui_children: Vec<Entity> = children
            .0
            .iter()
            .copied()
            .filter(|c| nodes.contains_key(c))
            .collect();
        for c in &ui_children {
            all_children.insert(*c);
        }
        nodes.get_mut(&entity).unwrap().children = ui_children;
    }

    // Roots are UI nodes that are no other UI node's child.
    let roots: Vec<Entity> = nodes
        .keys()
        .copied()
        .filter(|e| !all_children.contains(e))
        .collect();

    let root_rect = Rect::from_position_size(
        Vec2::new(safe_area.left, safe_area.top),
        Vec2::new(
            (screen_size.x - safe_area.left - safe_area.right).max(0.0),
            (screen_size.y - safe_area.top - safe_area.bottom).max(0.0),
        ),
    );

    let mut computed: HashMap<Entity, Rect> = HashMap::new();
    for root in roots {
        layout_node(root, root_rect, &nodes, &mut computed);
    }

    for (entity, rect) in computed {
        if let Ok(mut node) = world.get_mut::<UiNode>(entity) {
            node.computed_rect = rect;
        }
    }
}

fn inset(rect: Rect, edges: UiEdges) -> Rect {
    Rect::new(
        Vec2::new(rect.min.x + edges.left, rect.min.y + edges.top),
        Vec2::new(rect.max.x - edges.right, rect.max.y - edges.bottom),
    )
}

fn layout_node(
    entity: Entity,
    rect: Rect,
    nodes: &HashMap<Entity, NodeInfo>,
    out: &mut HashMap<Entity, Rect>,
) {
    out.insert(entity, rect);
    let info = &nodes[&entity];
    if info.children.is_empty() {
        return;
    }

    let content = inset(rect, info.style.padding);
    let column = info.style.direction == FlexDirection::Column;
    let main_extent = if column {
        content.height()
    } else {
        content.width()
    };
    let cross_extent = if column {
        content.width()
    } else {
        content.height()
    };

    // Per-child main-axis outer sizes (inner + main margins).
    let child_sizes: Vec<(f32, f32)> = info
        .children
        .iter()
        .map(|c| {
            let s = &nodes[c].style;
            let (inner, m0, m1) = if column {
                (s.height, s.margin.top, s.margin.bottom)
            } else {
                (s.width, s.margin.left, s.margin.right)
            };
            (inner, m0 + m1)
        })
        .collect();

    let total_main: f32 = child_sizes.iter().map(|(inner, m)| inner + m).sum();
    let free = main_extent - total_main;
    let n = info.children.len();
    let (mut cursor, gap) = match info.style.justify {
        JustifyContent::Start => (0.0, 0.0),
        JustifyContent::Center => (free * 0.5, 0.0),
        JustifyContent::End => (free, 0.0),
        JustifyContent::SpaceBetween => {
            if n > 1 {
                (0.0, free / (n - 1) as f32)
            } else {
                (free * 0.5, 0.0)
            }
        }
    };

    let content_main_min = if column { content.min.y } else { content.min.x };
    let content_cross_min = if column { content.min.x } else { content.min.y };

    for (i, child) in info.children.iter().enumerate() {
        let style = &nodes[child].style;
        let (main_inner, main_m0) = if column {
            (style.height, style.margin.top)
        } else {
            (style.width, style.margin.left)
        };
        let (cross_dim, cross_m0, cross_m1) = if column {
            (style.width, style.margin.left, style.margin.right)
        } else {
            (style.height, style.margin.top, style.margin.bottom)
        };
        let cross_inner = if cross_dim > 0.0 {
            cross_dim
        } else {
            (cross_extent - cross_m0 - cross_m1).max(0.0)
        };

        let main_pos = content_main_min + cursor + main_m0;
        let cross_pos = match info.style.align {
            AlignItems::Start => content_cross_min + cross_m0,
            AlignItems::Center => content_cross_min + (cross_extent - cross_inner) * 0.5,
            AlignItems::End => content_cross_min + cross_extent - cross_m1 - cross_inner,
        };

        let child_rect = if column {
            Rect::from_position_size(
                Vec2::new(cross_pos, main_pos),
                Vec2::new(cross_inner, main_inner),
            )
        } else {
            Rect::from_position_size(
                Vec2::new(main_pos, cross_pos),
                Vec2::new(main_inner, cross_inner),
            )
        };

        layout_node(*child, child_rect, nodes, out);
        cursor += child_sizes[i].0 + child_sizes[i].1 + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::UiNode;
    use kaadan_scene::Children;

    #[test]
    fn column_center_layout() {
        let mut world = World::new();
        let mut resources = Resources::new();
        resources.insert(UiScreen::new(300.0, 400.0));

        let root_node = UiNode {
            style: UiStyle {
                width: 300.0,
                height: 400.0,
                direction: FlexDirection::Column,
                justify: JustifyContent::Center,
                align: AlignItems::Center,
                ..Default::default()
            },
            ..Default::default()
        };
        let child = || UiNode {
            style: UiStyle {
                width: 100.0,
                height: 50.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let c1 = world.spawn((child(),));
        let c2 = world.spawn((child(),));
        let root = world.spawn((root_node, Children(vec![c1, c2])));

        ui_layout_system(&mut world, &mut resources);

        let root_rect = world.get::<UiNode>(root).unwrap().computed_rect;
        assert_eq!(root_rect.size(), Vec2::new(300.0, 400.0));

        // Two 50px children -> 100px content; centered in 400 -> start y=150.
        let r1 = world.get::<UiNode>(c1).unwrap().computed_rect;
        let r2 = world.get::<UiNode>(c2).unwrap().computed_rect;
        assert!((r1.min.y - 150.0).abs() < 0.01);
        assert!((r2.min.y - 200.0).abs() < 0.01);
        // Centered horizontally: (300-100)/2 = 100.
        assert!((r1.min.x - 100.0).abs() < 0.01);
        assert_eq!(r1.size(), Vec2::new(100.0, 50.0));
    }
}
