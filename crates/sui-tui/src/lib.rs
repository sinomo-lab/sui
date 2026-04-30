#![forbid(unsafe_code)]

mod model;
mod render;
mod validate;

pub use model::{TuiNode, TuiSnapshot};
pub use render::{TuiFrame, TuiLayoutMode, TuiRenderOptions, render_snapshot};
pub use validate::{
    AccessibilityIssue, AccessibilityIssueSeverity, AccessibilityIssueTarget, validate_snapshot,
};

#[cfg(test)]
mod tests {
    use sui_core::{
        Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, ToggleState, WidgetId,
        WindowId,
    };
    use sui_platform::AccessibilitySnapshot;

    use crate::{
        AccessibilityIssueSeverity, TuiLayoutMode, TuiRenderOptions, render_snapshot,
        validate_snapshot,
    };

    fn snapshot(nodes: Vec<SemanticsNode>) -> AccessibilitySnapshot {
        AccessibilitySnapshot::new(WindowId::new(1), nodes)
    }

    fn node(id: u64, parent: Option<u64>, role: SemanticsRole, name: &str) -> SemanticsNode {
        let mut node = SemanticsNode::new(
            WidgetId::new(id),
            role,
            Rect::new(0.0, id as f32 * 20.0, 160.0, 20.0),
        );
        node.parent = parent.map(WidgetId::new);
        if !name.is_empty() {
            node.name = Some(name.to_string());
        }
        node
    }

    #[test]
    fn structured_render_includes_roles_names_values_and_focus() {
        let mut root = node(1, None, SemanticsRole::Window, "Harness");
        let mut button = node(2, Some(1), SemanticsRole::Button, "Save");
        button.actions = vec![SemanticsAction::Activate];
        button.state.focused = true;
        let mut input = node(3, Some(1), SemanticsRole::TextInput, "Name");
        input.value = Some(SemanticsValue::Text("Ada".to_string()));
        input.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];

        let frame = render_snapshot(
            &snapshot(vec![root.clone(), button, input]),
            TuiRenderOptions {
                width: 80,
                height: 24,
                mode: TuiLayoutMode::Structured,
                show_hidden: false,
            },
        );

        let text = frame.to_string();
        assert!(text.contains("[Window] Harness"));
        assert!(text.contains("> [Button] Save"));
        assert!(text.contains("[TextInput] Name = Ada"));
        assert!(text.contains("Issues: none"));

        root.state.hidden = true;
        let hidden_frame = render_snapshot(
            &snapshot(vec![root]),
            TuiRenderOptions {
                width: 80,
                height: 24,
                mode: TuiLayoutMode::Structured,
                show_hidden: false,
            },
        );
        assert!(!hidden_frame.to_string().contains("Harness"));
    }

    #[test]
    fn validation_reports_missing_names_actions_and_bad_parent() {
        let root = node(1, None, SemanticsRole::Window, "Harness");
        let unnamed_button = node(2, Some(1), SemanticsRole::Button, "");
        let orphan = node(3, Some(99), SemanticsRole::Switch, "Enabled");
        let issues = validate_snapshot(&snapshot(vec![root, unnamed_button, orphan]));

        assert!(issues.iter().any(|issue| {
            issue.severity == AccessibilityIssueSeverity::Error
                && issue.message.contains("missing accessible name")
        }));
        assert!(issues.iter().any(|issue| {
            issue.severity == AccessibilityIssueSeverity::Warning
                && issue.message.contains("missing expected action")
        }));
        assert!(issues.iter().any(|issue| {
            issue.severity == AccessibilityIssueSeverity::Error
                && issue.message.contains("missing parent")
        }));
    }

    #[test]
    fn validation_reports_duplicate_visible_action_names() {
        let root = node(1, None, SemanticsRole::Window, "Harness");
        let mut first = node(2, Some(1), SemanticsRole::Button, "Save");
        first.actions = vec![SemanticsAction::Activate];
        let mut second = node(3, Some(1), SemanticsRole::Button, "Save");
        second.actions = vec![SemanticsAction::Activate];

        let issues = validate_snapshot(&snapshot(vec![root, first, second]));

        assert!(issues.iter().any(|issue| {
            issue.severity == AccessibilityIssueSeverity::Warning
                && issue
                    .message
                    .contains("duplicate visible actionable role/name")
        }));
    }

    #[test]
    fn role_state_formatting_handles_toggles() {
        let root = node(1, None, SemanticsRole::Window, "Harness");
        let mut switch = node(2, Some(1), SemanticsRole::Switch, "Power");
        switch.state.checked = Some(ToggleState::Checked);
        switch.actions = vec![SemanticsAction::Activate];

        let frame = render_snapshot(&snapshot(vec![root, switch]), TuiRenderOptions::default());

        assert!(frame.to_string().contains("checked"));
    }
}
