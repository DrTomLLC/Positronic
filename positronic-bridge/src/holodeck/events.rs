//! Holodeck event routing.
//!
//! Keeps winit out of the widget logic. The app converts winit events into PointerEvent
//! and calls `dispatch_pointer`.

use crate::widgets::{PointerEvent, WidgetAction};
use crate::holodeck::protocol::{HolodeckDoc, HolodeckNode};

pub fn dispatch_pointer(doc: &mut HolodeckDoc, ev: PointerEvent) -> WidgetAction {
    let mut action = WidgetAction::None;

    for node in &mut doc.nodes {
        match node {
            HolodeckNode::Table(t) => {
                let a = t.on_pointer(ev);
                action = merge(action, a);
            }
            HolodeckNode::Buttons(btns) => {
                for b in btns.iter_mut() {
                    let hit = b.hit(ev.x, ev.y);
                    b.hovered = hit;
                    if hit {
                        if matches!(ev.kind, crate::widgets::PointerKind::Down) {
                            b.pressed = true;
                            action = merge(action, b.action.clone());
                        }
                        if matches!(ev.kind, crate::widgets::PointerKind::Up) {
                            b.pressed = false;
                        }
                    } else if matches!(ev.kind, crate::widgets::PointerKind::Up) {
                        b.pressed = false;
                    }
                }
            }
            _ => {}
        }
    }

    action
}

fn merge(a: WidgetAction, b: WidgetAction) -> WidgetAction {
    match (a, b) {
        (WidgetAction::None, x) => x,
        (x, WidgetAction::None) => x,
        // Prefer "stronger" actions. If two are emitted, keep the first for determinism.
        (x, _) => x,
    }
}
