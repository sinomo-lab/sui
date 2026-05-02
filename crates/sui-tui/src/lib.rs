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

        let frame = render_snapshot(
            &snapshot(vec![root, switch]),
            TuiRenderOptions {
                mode: TuiLayoutMode::Structured,
                ..TuiRenderOptions::default()
            },
        );

        assert!(frame.to_string().contains("checked"));
    }

    #[test]
    fn spatial_render_draws_canvas_from_bounds() {
        let mut root = node(1, None, SemanticsRole::Window, "Harness");
        root.bounds = Rect::new(0.0, 0.0, 120.0, 80.0);
        let mut button = node(2, Some(1), SemanticsRole::Button, "Save");
        button.bounds = Rect::new(12.0, 12.0, 36.0, 20.0);
        button.actions = vec![SemanticsAction::Activate];
        let mut input = node(3, Some(1), SemanticsRole::TextInput, "Name");
        input.bounds = Rect::new(72.0, 48.0, 36.0, 20.0);
        input.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        input.state.focused = true;

        let frame = render_snapshot(
            &snapshot(vec![root, button, input]),
            TuiRenderOptions {
                width: 64,
                height: 28,
                mode: TuiLayoutMode::Spatial,
                show_hidden: false,
            },
        );
        let text = frame.to_string();

        assert!(text.contains("Spatial canvas bounds=(0,0,120,80)"));
        assert!(text.contains("Button:Save"));
        assert!(text.contains("Input:Name"));
        assert!(text.contains("Spatial legend:"));
        assert!(text.contains("@"));
    }

    #[test]
    fn spatial_render_uses_root_bounds_and_omits_offscreen_nodes() {
        let mut root = node(1, None, SemanticsRole::Window, "Harness");
        root.bounds = Rect::new(0.0, 0.0, 100.0, 80.0);
        let mut visible = node(2, Some(1), SemanticsRole::Button, "Visible");
        visible.bounds = Rect::new(10.0, 10.0, 24.0, 18.0);
        visible.actions = vec![SemanticsAction::Activate];
        let mut offscreen = node(3, Some(1), SemanticsRole::Button, "Offscreen");
        offscreen.bounds = Rect::new(0.0, 400.0, 80.0, 30.0);
        offscreen.actions = vec![SemanticsAction::Activate];

        let frame = render_snapshot(
            &snapshot(vec![root, visible, offscreen]),
            TuiRenderOptions {
                width: 64,
                height: 28,
                mode: TuiLayoutMode::Spatial,
                show_hidden: false,
            },
        )
        .to_string();

        assert!(frame.contains("Spatial canvas bounds=(0,0,100,80)"));
        assert!(frame.contains("Button:Visible"));
        assert!(!frame.contains("Offscreen"));
    }
}
