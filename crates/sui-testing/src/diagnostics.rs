use std::fmt::Write;

use sui_core::{SemanticsNode, SemanticsValue};

use crate::snapshot::WindowSnapshot;

pub(crate) fn format_failure(
    action: &str,
    selector_description: &str,
    snapshot: &WindowSnapshot,
    detail: &str,
) -> String {
    let mut message = String::new();
    let _ = writeln!(message, "{action} failed: {detail}");
    let _ = writeln!(message, "selector: {selector_description}");
    let _ = writeln!(
        message,
        "window: {} ({})",
        snapshot.title,
        snapshot.window_id.get()
    );
    let _ = writeln!(
        message,
        "focus: {:?}, semantics_root: {:?}",
        snapshot.focus_state.focused_widget, snapshot.accessibility.root
    );
    if let Some(scene) = &snapshot.scene_summary {
        let _ = writeln!(
            message,
            "scene: viewport=({}, {}), dirty_regions={}, commands={}",
            scene.viewport.width,
            scene.viewport.height,
            scene.dirty_regions.len(),
            scene.command_count,
        );
        let _ = writeln!(
            message,
            "scene command breakdown: {:?}",
            scene.command_breakdown
        );
    }
    let _ = writeln!(message, "Semantics snapshot:");
    for node in &snapshot.accessibility.nodes {
        let _ = writeln!(message, "  {}", format_semantics_node(node));
    }
    let _ = writeln!(message, "Widget graph:");
    for node in &snapshot.widget_graph.nodes {
        let _ = writeln!(
            message,
            "  id={} parent={:?} children={:?} bounds=({}, {}, {}, {}) focusable={} focused={}",
            node.id.get(),
            node.parent.map(|id| id.get()),
            node.children.iter().map(|id| id.get()).collect::<Vec<_>>(),
            node.bounds.x(),
            node.bounds.y(),
            node.bounds.width(),
            node.bounds.height(),
            node.accepts_focus,
            node.focused,
        );
    }
    message
}

fn format_semantics_node(node: &SemanticsNode) -> String {
    format!(
        "id={} parent={:?} role={:?} name={:?} description={:?} value={} bounds=({}, {}, {}, {}) state={{disabled:{}, focused:{}, hidden:{}, hovered:{}, selected:{}, busy:{}}}",
        node.id.get(),
        node.parent.map(|id| id.get()),
        node.role,
        node.name,
        node.description,
        format_value(node.value.as_ref()),
        node.bounds.x(),
        node.bounds.y(),
        node.bounds.width(),
        node.bounds.height(),
        node.state.disabled,
        node.state.focused,
        node.state.hidden,
        node.state.hovered,
        node.state.selected,
        node.state.busy,
    )
}

fn format_value(value: Option<&SemanticsValue>) -> String {
    match value {
        Some(SemanticsValue::Text(text)) => format!("Text({text:?})"),
        Some(SemanticsValue::Number(number)) => format!("Number({number})"),
        Some(SemanticsValue::Range { value, min, max }) => {
            format!("Range(value={value}, min={min}, max={max})")
        }
        None => "None".to_string(),
    }
}
