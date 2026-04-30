use std::fmt::{self, Write};

use sui_core::{Rect, SemanticsNode, SemanticsRole, SemanticsValue, ToggleState};
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
            mode: TuiLayoutMode::Spatial,
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
    let spatial_nodes = nodes
        .iter()
        .filter(|node| spatial_bounds(node).is_some())
        .cloned()
        .collect::<Vec<_>>();

    if spatial_nodes.is_empty() {
        lines.push(fit_line("<empty accessibility tree>", options.width));
        return;
    }

    let Some(bounds) = spatial_world_bounds(&spatial_nodes) else {
        lines.push(fit_line("<no spatial bounds>", options.width));
        return;
    };
    let spatial_nodes = spatial_nodes
        .into_iter()
        .filter(|node| node.bounds.intersection(bounds).is_some())
        .collect::<Vec<_>>();

    let canvas_height = spatial_canvas_height(lines.len(), options);
    let canvas_width = options.width.max(20) as usize;
    lines.push(fit_line(
        format!(
            "Spatial canvas bounds=({:.0},{:.0},{:.0},{:.0})",
            bounds.x(),
            bounds.y(),
            bounds.width(),
            bounds.height()
        ),
        options.width,
    ));

    let mut canvas = SpatialCanvas::new(canvas_width, canvas_height);
    let mut draw_order = spatial_nodes
        .iter()
        .filter(|node| canvas_node(node))
        .cloned()
        .collect::<Vec<_>>();
    draw_order.sort_by(|left, right| {
        node_area(right)
            .total_cmp(&node_area(left))
            .then_with(|| left.id.cmp(&right.id))
    });
    for node in &draw_order {
        canvas.draw_node(node, bounds);
    }
    lines.extend(
        canvas
            .into_lines()
            .into_iter()
            .map(|line| fit_line(line, options.width)),
    );

    lines.push(fit_line("Spatial legend:", options.width));
    let mut sorted = spatial_nodes;
    sorted.sort_by(|left, right| {
        left.bounds
            .y()
            .total_cmp(&right.bounds.y())
            .then_with(|| left.bounds.x().total_cmp(&right.bounds.x()))
            .then_with(|| left.id.cmp(&right.id))
    });

    for node in sorted.iter().filter(|node| legend_node(node)).take(10) {
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

fn spatial_world_bounds(nodes: &[SemanticsNode]) -> Option<Rect> {
    nodes
        .iter()
        .find(|node| {
            node.parent.is_none()
                && matches!(node.role, SemanticsRole::Window | SemanticsRole::Root)
                && spatial_bounds(node).is_some()
        })
        .and_then(spatial_bounds)
        .or_else(|| {
            nodes
                .iter()
                .filter_map(spatial_bounds)
                .reduce(|left, right| left.union(right))
        })
}

struct SpatialCanvas {
    width: usize,
    height: usize,
    cells: Vec<Vec<char>>,
}

impl SpatialCanvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![vec![' '; width]; height],
        }
    }

    fn draw_node(&mut self, node: &SemanticsNode, world: Rect) {
        let Some((x0, y0, x1, y1)) =
            map_rect_to_canvas(node.bounds, world, self.width, self.height)
        else {
            return;
        };

        let border = node_border(node);
        for x in x0..=x1 {
            self.put(x, y0, border);
            self.put(x, y1, border);
        }
        for y in y0..=y1 {
            self.put(x0, y, border);
            self.put(x1, y, border);
        }

        if x1 > x0 + 3 && label_node(node) {
            let label = spatial_label(node);
            let label_y = if y1 > y0 + 1 { y0 + 1 } else { y0 };
            let max_len = x1.saturating_sub(x0 + 1);
            for (offset, ch) in label.chars().take(max_len).enumerate() {
                self.put(x0 + 1 + offset, label_y, ch);
            }
        }
    }

    fn put(&mut self, x: usize, y: usize, value: char) {
        if y < self.height && x < self.width {
            self.cells[y][x] = value;
        }
    }

    fn into_lines(self) -> Vec<String> {
        self.cells
            .into_iter()
            .map(|row| row.into_iter().collect::<String>())
            .collect()
    }
}

fn spatial_canvas_height(existing_lines: usize, options: TuiRenderOptions) -> usize {
    if options.height == 0 {
        return 18;
    }

    (options.height as usize)
        .saturating_sub(existing_lines + 14)
        .clamp(6, 24)
}

fn map_rect_to_canvas(
    rect: Rect,
    world: Rect,
    width: usize,
    height: usize,
) -> Option<(usize, usize, usize, usize)> {
    if width == 0 || height == 0 || world.width() <= 0.0 || world.height() <= 0.0 {
        return None;
    }

    let max_x = width.saturating_sub(1) as f32;
    let max_y = height.saturating_sub(1) as f32;
    let scale_x = max_x / world.width().max(f32::EPSILON);
    let scale_y = max_y / world.height().max(f32::EPSILON);

    let mut x0 = ((rect.x() - world.x()) * scale_x).floor().max(0.0) as usize;
    let mut y0 = ((rect.y() - world.y()) * scale_y).floor().max(0.0) as usize;
    let mut x1 = ((rect.max_x() - world.x()) * scale_x).ceil().max(0.0) as usize;
    let mut y1 = ((rect.max_y() - world.y()) * scale_y).ceil().max(0.0) as usize;

    x0 = x0.min(width.saturating_sub(1));
    x1 = x1.min(width.saturating_sub(1));
    y0 = y0.min(height.saturating_sub(1));
    y1 = y1.min(height.saturating_sub(1));

    if x1 <= x0 {
        x1 = (x0 + 1).min(width.saturating_sub(1));
    }
    if y1 <= y0 {
        y1 = (y0 + 1).min(height.saturating_sub(1));
    }

    Some((x0, y0, x1, y1))
}

fn spatial_bounds(node: &SemanticsNode) -> Option<Rect> {
    let bounds = node.bounds;
    if bounds.is_empty()
        || !bounds.x().is_finite()
        || !bounds.y().is_finite()
        || !bounds.width().is_finite()
        || !bounds.height().is_finite()
    {
        return None;
    }
    Some(bounds)
}

fn node_area(node: &SemanticsNode) -> f32 {
    node.bounds.width().max(0.0) * node.bounds.height().max(0.0)
}

fn node_border(node: &SemanticsNode) -> char {
    if node.state.focused {
        '@'
    } else if interactive_role(node) {
        '*'
    } else if matches!(node.role, SemanticsRole::Window | SemanticsRole::Root) {
        '#'
    } else if matches!(
        node.role,
        SemanticsRole::GenericContainer | SemanticsRole::ScrollView
    ) {
        '+'
    } else {
        '.'
    }
}

fn canvas_node(node: &SemanticsNode) -> bool {
    node.state.focused
        || interactive_role(node)
        || matches!(
            node.role,
            SemanticsRole::Window
                | SemanticsRole::Root
                | SemanticsRole::GenericContainer
                | SemanticsRole::List
                | SemanticsRole::Tree
                | SemanticsRole::Table
                | SemanticsRole::TabBar
                | SemanticsRole::Tabs
                | SemanticsRole::Menu
                | SemanticsRole::ContextMenu
                | SemanticsRole::Dialog
                | SemanticsRole::Popover
                | SemanticsRole::ScrollView
                | SemanticsRole::Image
                | SemanticsRole::Canvas
        )
}

fn label_node(node: &SemanticsNode) -> bool {
    node.state.focused
        || interactive_role(node)
        || matches!(
            node.role,
            SemanticsRole::Window
                | SemanticsRole::Root
                | SemanticsRole::List
                | SemanticsRole::Dialog
                | SemanticsRole::Popover
                | SemanticsRole::ScrollView
        )
}

fn spatial_label(node: &SemanticsNode) -> String {
    let name = node.name.as_deref().unwrap_or("<unnamed>");
    format!("{}:{name}", compact_role_label(&node.role))
}

fn compact_role_label(role: &SemanticsRole) -> &'static str {
    match role {
        SemanticsRole::GenericContainer => "Group",
        SemanticsRole::TextInput => "Input",
        SemanticsRole::ScrollView => "Scroll",
        SemanticsRole::ColorSwatch => "Swatch",
        SemanticsRole::RadioButton => "Radio",
        SemanticsRole::ProgressBar => "Progress",
        SemanticsRole::BusyIndicator => "Busy",
        _ => role_label(role),
    }
}

fn legend_node(node: &SemanticsNode) -> bool {
    node.state.focused
        || interactive_role(node)
        || matches!(
            node.role,
            SemanticsRole::Window
                | SemanticsRole::Root
                | SemanticsRole::GenericContainer
                | SemanticsRole::ScrollView
        )
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

fn interactive_role(node: &SemanticsNode) -> bool {
    matches!(
        node.role,
        SemanticsRole::Button
            | SemanticsRole::CheckBox
            | SemanticsRole::Switch
            | SemanticsRole::RadioButton
            | SemanticsRole::MenuItem
            | SemanticsRole::Slider
            | SemanticsRole::TextInput
            | SemanticsRole::SpinBox
            | SemanticsRole::ComboBox
            | SemanticsRole::ColorPicker
            | SemanticsRole::ScrollView
    )
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
