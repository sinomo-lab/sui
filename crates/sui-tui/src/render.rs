use std::fmt::{self, Write};

use sui_core::{SemanticsNode, SemanticsRole, SemanticsValue, ToggleState};
use sui_platform::AccessibilitySnapshot;

use crate::{
    model::{TuiNode, TuiSnapshot},
    validate::{AccessibilityIssueSeverity, validate_snapshot},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiLayoutMode {
    Structured,
    Spatial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiRenderOptions {
    pub width: u16,
    pub height: u16,
    pub mode: TuiLayoutMode,
    pub show_hidden: bool,
}

impl Default for TuiRenderOptions {
    fn default() -> Self {
        Self {
            width: 100,
            height: 36,
            mode: TuiLayoutMode::Structured,
            show_hidden: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiFrame {
    lines: Vec<String>,
}

impl TuiFrame {
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

impl fmt::Display for TuiFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                f.write_char('\n')?;
            }
            f.write_str(line)?;
        }
        Ok(())
    }
}

pub fn render_snapshot(snapshot: &AccessibilitySnapshot, options: TuiRenderOptions) -> TuiFrame {
    let tui_snapshot = TuiSnapshot::from_accessibility(snapshot, options.show_hidden);
    let issues = validate_snapshot(snapshot);

    let mut lines = Vec::new();
    lines.push(fit_line(
        format!(
            "SUI TUI window=#{} root={} focused={} nodes={}",
            snapshot.window_id.get(),
            snapshot
                .root
                .map(|id| format!("#{id}"))
                .unwrap_or_else(|| "none".to_string()),
            snapshot
                .focused_widget
                .map(|id| format!("#{id}"))
                .unwrap_or_else(|| "none".to_string()),
            tui_snapshot.flat.len(),
        ),
        options.width,
    ));
    lines.push(fit_line("=".repeat(options.width as usize), options.width));

    match options.mode {
        TuiLayoutMode::Structured => render_structured(&tui_snapshot.roots, &mut lines, options),
        TuiLayoutMode::Spatial => render_spatial(&tui_snapshot.flat, &mut lines, options),
    }

    lines.push(fit_line("-".repeat(options.width as usize), options.width));
    render_issues(&issues, &mut lines, options.width);

    if options.height > 0 {
        lines.truncate(options.height as usize);
    }

    TuiFrame::new(lines)
}

fn render_structured(roots: &[TuiNode], lines: &mut Vec<String>, options: TuiRenderOptions) {
    if roots.is_empty() {
        lines.push(fit_line("<empty accessibility tree>", options.width));
        return;
    }

    for root in roots {
        render_node(root, lines, options.width);
    }
}

fn render_node(node: &TuiNode, lines: &mut Vec<String>, width: u16) {
    let indent = "  ".repeat(node.depth);
    let focus = if node.node.state.focused { "> " } else { "  " };
    lines.push(fit_line(
        format!("{indent}{focus}{}", format_node_summary(&node.node)),
        width,
    ));

    for child in &node.children {
        render_node(child, lines, width);
    }
}

fn render_spatial(nodes: &[SemanticsNode], lines: &mut Vec<String>, options: TuiRenderOptions) {
    if nodes.is_empty() {
        lines.push(fit_line("<empty accessibility tree>", options.width));
        return;
    }

    let mut sorted = nodes.to_vec();
    sorted.sort_by(|left, right| {
        left.bounds
            .y()
            .total_cmp(&right.bounds.y())
            .then_with(|| left.bounds.x().total_cmp(&right.bounds.x()))
            .then_with(|| left.id.cmp(&right.id))
    });

    for node in &sorted {
        lines.push(fit_line(
            format!(
                "{:>4},{:>4} {:>4}x{:<4} {}",
                node.bounds.x().round() as i32,
                node.bounds.y().round() as i32,
                node.bounds.width().round() as i32,
                node.bounds.height().round() as i32,
                format_node_summary(node)
            ),
            options.width,
        ));
    }
}

fn render_issues(issues: &[crate::AccessibilityIssue], lines: &mut Vec<String>, width: u16) {
    if issues.is_empty() {
        lines.push(fit_line("Issues: none", width));
        return;
    }

    let errors = issues
        .iter()
        .filter(|issue| issue.severity == AccessibilityIssueSeverity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|issue| issue.severity == AccessibilityIssueSeverity::Warning)
        .count();
    lines.push(fit_line(
        format!("Issues: {errors} error(s), {warnings} warning(s)"),
        width,
    ));

    for issue in issues.iter().take(8) {
        lines.push(fit_line(
            format!("- {:?}: {}", issue.severity, issue.message),
            width,
        ));
    }
}

fn format_node_summary(node: &SemanticsNode) -> String {
    let mut summary = String::new();
    let _ = write!(
        summary,
        "[{}] {}",
        role_label(&node.role),
        node.name.as_deref().unwrap_or("<unnamed>")
    );

    if let Some(value) = &node.value {
        let _ = write!(summary, " = {}", format_value(value));
    }

    let states = format_states(node);
    if !states.is_empty() {
        let _ = write!(summary, " ({states})");
    }

    if !node.actions.is_empty() {
        let actions = node
            .actions
            .iter()
            .map(|action| format!("{action:?}"))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(summary, " actions={actions}");
    }

    summary
}

fn format_value(value: &SemanticsValue) -> String {
    match value {
        SemanticsValue::Text(text) => text.clone(),
        SemanticsValue::Number(number) => format!("{number:.2}"),
        SemanticsValue::Range { value, min, max } => format!("{value:.2} [{min:.2}..{max:.2}]"),
    }
}

fn format_states(node: &SemanticsNode) -> String {
    let mut states = Vec::new();
    if node.state.disabled {
        states.push("disabled".to_string());
    }
    if node.state.focused {
        states.push("focused".to_string());
    }
    if node.state.hidden {
        states.push("hidden".to_string());
    }
    if node.state.hovered {
        states.push("hovered".to_string());
    }
    if let Some(checked) = node.state.checked {
        states.push(
            match checked {
                ToggleState::Unchecked => "unchecked",
                ToggleState::Checked => "checked",
                ToggleState::Mixed => "mixed",
            }
            .to_string(),
        );
    }
    if node.state.selected {
        states.push("selected".to_string());
    }
    if let Some(expanded) = node.state.expanded {
        states.push(if expanded { "expanded" } else { "collapsed" }.to_string());
    }
    if node.state.busy {
        states.push("busy".to_string());
    }
    states.join(",")
}

fn role_label(role: &SemanticsRole) -> &'static str {
    match role {
        SemanticsRole::Window => "Window",
        SemanticsRole::Root => "Root",
        SemanticsRole::GenericContainer => "Group",
        SemanticsRole::Separator => "Separator",
        SemanticsRole::List => "List",
        SemanticsRole::Tree => "Tree",
        SemanticsRole::Table => "Table",
        SemanticsRole::Splitter => "Splitter",
        SemanticsRole::Breadcrumb => "Breadcrumb",
        SemanticsRole::TabBar => "TabBar",
        SemanticsRole::Tabs => "Tabs",
        SemanticsRole::Button => "Button",
        SemanticsRole::CheckBox => "CheckBox",
        SemanticsRole::Switch => "Switch",
        SemanticsRole::RadioButton => "RadioButton",
        SemanticsRole::RadioGroup => "RadioGroup",
        SemanticsRole::Menu => "Menu",
        SemanticsRole::MenuItem => "MenuItem",
        SemanticsRole::ContextMenu => "ContextMenu",
        SemanticsRole::Tooltip => "Tooltip",
        SemanticsRole::Dialog => "Dialog",
        SemanticsRole::Popover => "Popover",
        SemanticsRole::Slider => "Slider",
        SemanticsRole::ProgressBar => "ProgressBar",
        SemanticsRole::BusyIndicator => "BusyIndicator",
        SemanticsRole::Text => "Text",
        SemanticsRole::TextInput => "TextInput",
        SemanticsRole::SpinBox => "SpinBox",
        SemanticsRole::ComboBox => "ComboBox",
        SemanticsRole::Image => "Image",
        SemanticsRole::ColorSwatch => "ColorSwatch",
        SemanticsRole::ColorPicker => "ColorPicker",
        SemanticsRole::Canvas => "Canvas",
        SemanticsRole::ScrollView => "ScrollView",
    }
}

fn fit_line(line: impl Into<String>, width: u16) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }

    let mut line = line.into();
    if line.len() > width {
        line.truncate(width.saturating_sub(1));
        line.push('~');
    }
    line
}
