use sui_core::WindowId;
use sui_platform::AccessibilitySnapshot;
use sui_runtime::{FocusState, WidgetGraphSnapshot};

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSnapshot {
    pub window_id: WindowId,
    pub title: String,
    pub accessibility: AccessibilitySnapshot,
    pub widget_graph: WidgetGraphSnapshot,
    pub focus_state: FocusState,
}
