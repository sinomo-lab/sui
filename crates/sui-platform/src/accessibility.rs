use sui_core::{SemanticsNode, WidgetId, WindowId};

#[derive(Debug, Clone, PartialEq)]
pub struct AccessibilitySnapshot {
    pub window_id: WindowId,
    pub root: Option<WidgetId>,
    pub focused_widget: Option<WidgetId>,
    pub nodes: Vec<SemanticsNode>,
}

impl AccessibilitySnapshot {
    pub fn new(window_id: WindowId, nodes: Vec<SemanticsNode>) -> Self {
        let root = nodes
            .iter()
            .find(|node| node.parent.is_none())
            .map(|node| node.id);
        let focused_widget = nodes
            .iter()
            .find(|node| node.state.focused)
            .map(|node| node.id);

        Self {
            window_id,
            root,
            focused_widget,
            nodes,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AccessibilityBridge {
    snapshot: Option<AccessibilitySnapshot>,
}

impl AccessibilityBridge {
    pub(crate) fn snapshot(&self) -> Option<&AccessibilitySnapshot> {
        self.snapshot.as_ref()
    }

    pub(crate) fn update(&mut self, window_id: WindowId, nodes: Vec<SemanticsNode>) {
        self.snapshot = Some(AccessibilitySnapshot::new(window_id, nodes));
    }
}
