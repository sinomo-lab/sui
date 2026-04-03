#![forbid(unsafe_code)]

use std::collections::HashMap;

use sui_core::{Color, DirtyRegion, Rect, SemanticsValue, Size, WidgetId, WindowId};
use sui_layout::Alignment;
use sui_platform::AccessibilitySnapshot;
use sui_runtime::{FocusState, FrameSchedule, Widget, WidgetGraphSnapshot, WidgetNodeSnapshot};
use sui_scene::{SceneCommand, SceneFrame};
use sui_widgets::{
    Background, Label, ListItem, ListView, Padding, ScrollView, Separator, SizedBox, Stack,
    Table, TableColumn, TableRow, TreeItem, TreeView,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugTone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugMetric {
    pub label: String,
    pub value: String,
    pub detail: Option<String>,
    pub tone: DebugTone,
}

impl DebugMetric {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            detail: None,
            tone: DebugTone::Neutral,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub const fn tone(mut self, tone: DebugTone) -> Self {
        self.tone = tone;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugKeyValue {
    pub label: String,
    pub value: String,
}

impl DebugKeyValue {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneDebugSummary {
    pub viewport: Size,
    pub dirty_regions: Vec<DirtyRegion>,
    pub command_count: usize,
    pub command_breakdown: Vec<(String, usize)>,
}

impl From<&SceneFrame> for SceneDebugSummary {
    fn from(frame: &SceneFrame) -> Self {
        let mut command_breakdown = HashMap::<String, usize>::new();
        for command in frame.scene.commands() {
            *command_breakdown
                .entry(command_kind(command).to_string())
                .or_default() += 1;
        }

        let mut command_breakdown: Vec<_> = command_breakdown.into_iter().collect();
        command_breakdown.sort_by(|left, right| left.0.cmp(&right.0));

        Self {
            viewport: frame.viewport,
            dirty_regions: frame.dirty_regions.clone(),
            command_count: frame.scene.commands().len(),
            command_breakdown,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowDebugSnapshot {
    pub title: String,
    pub window_id: WindowId,
    pub focus_state: FocusState,
    pub schedule: Option<FrameSchedule>,
    pub accessibility: AccessibilitySnapshot,
    pub widget_graph: WidgetGraphSnapshot,
    pub scene_summary: Option<SceneDebugSummary>,
}

impl WindowDebugSnapshot {
    pub fn new(
        title: impl Into<String>,
        window_id: WindowId,
        focus_state: FocusState,
        accessibility: AccessibilitySnapshot,
        widget_graph: WidgetGraphSnapshot,
    ) -> Self {
        Self {
            title: title.into(),
            window_id,
            focus_state,
            schedule: None,
            accessibility,
            widget_graph,
            scene_summary: None,
        }
    }

    pub const fn with_schedule(mut self, schedule: FrameSchedule) -> Self {
        self.schedule = Some(schedule);
        self
    }

    pub fn with_scene_summary(mut self, scene_summary: SceneDebugSummary) -> Self {
        self.scene_summary = Some(scene_summary);
        self
    }
}

pub fn debug_panel<W>(title: impl Into<String>, subtitle: impl Into<String>, body: W) -> impl Widget
where
    W: Widget + 'static,
{
    let title = title.into();
    let subtitle = subtitle.into();

    Background::new(
        Color::rgba(0.984, 0.989, 0.997, 1.0),
        Padding::all(
            16.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(title)
                        .font_size(19.0)
                        .line_height(23.0)
                        .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
                )
                .with_child(
                    Label::new(subtitle)
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                )
                .with_child(Separator::horizontal())
                .with_child(body),
        ),
    )
}

pub fn debug_metric_grid<I>(metrics: I) -> impl Widget
where
    I: IntoIterator<Item = DebugMetric>,
{
    let metrics: Vec<_> = metrics.into_iter().collect();
    let mut column = Stack::vertical().spacing(12.0).alignment(Alignment::Stretch);

    for chunk in metrics.chunks(3) {
        let mut row = Stack::horizontal().spacing(12.0).alignment(Alignment::Stretch);
        for metric in chunk {
            row = row.with_child(
                SizedBox::new()
                    .width(190.0)
                    .with_child(metric_card(metric.clone())),
            );
        }
        column = column.with_child(row);
    }

    column
}

pub fn debug_key_values<I>(entries: I) -> impl Widget
where
    I: IntoIterator<Item = DebugKeyValue>,
{
    let mut column = Stack::vertical().spacing(8.0).alignment(Alignment::Stretch);

    for (index, entry) in entries.into_iter().enumerate() {
        let background = if index % 2 == 0 {
            Color::rgba(0.971, 0.978, 0.989, 1.0)
        } else {
            Color::rgba(0.957, 0.968, 0.983, 1.0)
        };
        column = column.with_child(Background::new(
            background,
            Padding::all(
                10.0,
                Stack::horizontal()
                    .spacing(12.0)
                    .alignment(Alignment::Center)
                    .with_child(
                        SizedBox::new().width(180.0).with_child(
                            Label::new(entry.label)
                                .font_size(12.0)
                                .line_height(16.0)
                                .color(Color::rgba(0.38, 0.46, 0.55, 1.0)),
                        ),
                    )
                    .with_child(
                        Label::new(entry.value)
                            .font_size(13.0)
                            .line_height(17.0)
                            .color(Color::rgba(0.12, 0.17, 0.24, 1.0)),
                    ),
            ),
        ));
    }

    column
}

pub fn accessibility_snapshot_view(snapshot: AccessibilitySnapshot) -> impl Widget {
    let items = snapshot
        .nodes
        .iter()
        .map(|node| {
            let mut detail = format!(
                "role={:?} parent={} bounds={} focus={} hidden={}",
                node.role,
                format_optional_widget_id(node.parent),
                format_rect(node.bounds),
                yes_no(node.state.focused),
                yes_no(node.state.hidden),
            );

            if let Some(name) = &node.name {
                detail.push_str(&format!(" name={name}"));
            }
            if let Some(value) = &node.value {
                detail.push_str(&format!(" value={}", format_semantics_value(value)));
            }

            let mut item = ListItem::new(format!("#{} {:?}", node.id.get(), node.role)).detail(detail);
            if node.state.focused {
                item = item.accent(Color::rgba(0.88, 0.33, 0.19, 1.0));
            } else if node.state.hidden {
                item = item.accent(Color::rgba(0.54, 0.60, 0.68, 1.0));
            } else {
                item = item.accent(Color::rgba(0.16, 0.56, 0.87, 1.0));
            }
            item
        })
        .collect::<Vec<_>>();

    SizedBox::new().height(220.0).with_child(ListView::new("Accessibility snapshot").items(items))
}

pub fn widget_graph_snapshot_view(graph: WidgetGraphSnapshot) -> impl Widget {
    let items = build_graph_tree_items(&graph);
    SizedBox::new().height(240.0).with_child(TreeView::new("Widget graph").items(items))
}

pub fn scene_summary_view(scene: SceneDebugSummary) -> impl Widget {
    let metrics = debug_metric_grid([
        DebugMetric::new("Viewport", format_size(scene.viewport))
            .detail("Logical render target size")
            .tone(DebugTone::Info),
        DebugMetric::new("Dirty regions", scene.dirty_regions.len().to_string())
            .detail("Invalidated rectangles queued into the scene")
            .tone(if scene.dirty_regions.is_empty() {
                DebugTone::Success
            } else {
                DebugTone::Warning
            }),
        DebugMetric::new("Commands", scene.command_count.to_string())
            .detail("Scene commands emitted by the runtime")
            .tone(DebugTone::Neutral),
    ]);

    let dirty_region_entries = if scene.dirty_regions.is_empty() {
        vec![DebugKeyValue::new(
            "Region state",
            "No dirty regions in the captured frame",
        )]
    } else {
        scene
            .dirty_regions
            .iter()
            .enumerate()
            .map(|(index, region)| {
                DebugKeyValue::new(
                    format!("Region {}", index + 1),
                    format!("{:?} {}", region.kind, format_rect(region.area)),
                )
            })
            .collect()
    };

    let rows = scene
        .command_breakdown
        .iter()
        .map(|(kind, count)| TableRow::new([kind.clone(), count.to_string()]))
        .collect::<Vec<_>>();

    Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch)
        .with_child(metrics)
        .with_child(debug_key_values(dirty_region_entries))
        .with_child(
            SizedBox::new().height(188.0).with_child(
                Table::new("Scene command breakdown")
                    .columns([
                        TableColumn::new("Command").min_width(180.0),
                        TableColumn::new("Count").width(90.0),
                    ])
                    .rows(rows),
            ),
        )
}

pub fn window_snapshot_view(snapshot: WindowDebugSnapshot) -> impl Widget {
    let WindowDebugSnapshot {
        title,
        window_id,
        focus_state,
        schedule,
        accessibility,
        widget_graph,
        scene_summary,
    } = snapshot;

    let root_name = accessibility
        .root
        .map(|id| format!("#{}", id.get()))
        .unwrap_or_else(|| "none".to_string());
    let focused_name = focus_state
        .focused_widget
        .map(|id| format!("#{}", id.get()))
        .unwrap_or_else(|| "none".to_string());

    let summary = debug_panel(
        "Runtime summary",
        "High-level counts and identifiers for the captured SUI window state.",
        debug_metric_grid([
            DebugMetric::new("Window", title)
                .detail(format!("window id #{}", window_id.get()))
                .tone(DebugTone::Info),
            DebugMetric::new("Focused widget", focused_name)
                .detail(format!("window focused={}", yes_no(focus_state.window_focused)))
                .tone(if focus_state.focused_widget.is_some() {
                    DebugTone::Warning
                } else {
                    DebugTone::Neutral
                }),
            DebugMetric::new("Semantics nodes", accessibility.nodes.len().to_string())
                .detail(format!("root={root_name}"))
                .tone(DebugTone::Neutral),
            DebugMetric::new("Widget nodes", widget_graph.nodes.len().to_string())
                .detail(format!("graph root #{}", widget_graph.root.get()))
                .tone(DebugTone::Neutral),
            DebugMetric::new(
                "Scene frame",
                if scene_summary.is_some() { "available" } else { "missing" },
            )
            .detail("Present when a captured render summary exists")
            .tone(if scene_summary.is_some() {
                DebugTone::Success
            } else {
                DebugTone::Danger
            }),
        ]),
    );

    let mut body = Stack::vertical().spacing(14.0).alignment(Alignment::Stretch);
    body.push(summary);
    body.push(debug_panel(
        "Focus and scheduling",
        "The runtime focus state is often the fastest way to explain why keyboard input is landing in the wrong place.",
        debug_key_values([
            DebugKeyValue::new("Focused widget", format_optional_widget_id(focus_state.focused_widget)),
            DebugKeyValue::new("Window focused", yes_no(focus_state.window_focused)),
            DebugKeyValue::new(
                "Pending invalidation kinds",
                schedule
                    .map(format_schedule)
                    .unwrap_or_else(|| "not captured".to_string()),
            ),
        ]),
    ));
    body.push(debug_panel(
        "Accessibility snapshot",
        "Accessible names, roles, and state are shared infrastructure for accessibility, testing, and automation.",
        accessibility_snapshot_view(accessibility),
    ));
    body.push(debug_panel(
        "Widget graph",
        "Tree inspection should expose retained widget identity and bounds without leaking runtime internals.",
        widget_graph_snapshot_view(widget_graph),
    ));

    if let Some(scene_summary) = scene_summary {
        body.push(debug_panel(
            "Scene summary",
            "Renderer-neutral scene data makes frame generation debuggable before it reaches the backend.",
            scene_summary_view(scene_summary),
        ));
    }

    ScrollView::vertical(body)
}

fn metric_card(metric: DebugMetric) -> impl Widget {
    let (background, label_color, value_color) = tone_palette(metric.tone);
    let mut column = Stack::vertical().spacing(8.0).alignment(Alignment::Stretch);
    column.push(
        Label::new(metric.label)
            .font_size(12.0)
            .line_height(16.0)
            .color(label_color),
    );
    column.push(
        Label::new(metric.value)
            .font_size(22.0)
            .line_height(26.0)
            .color(value_color),
    );
    if let Some(detail) = metric.detail {
        column.push(
            Label::new(detail)
                .font_size(12.0)
                .line_height(16.0)
                .color(Color::rgba(0.40, 0.47, 0.56, 1.0)),
        );
    }

    Background::new(background, Padding::all(12.0, column))
}

fn tone_palette(tone: DebugTone) -> (Color, Color, Color) {
    match tone {
        DebugTone::Neutral => (
            Color::rgba(0.958, 0.968, 0.983, 1.0),
            Color::rgba(0.35, 0.43, 0.52, 1.0),
            Color::rgba(0.10, 0.15, 0.21, 1.0),
        ),
        DebugTone::Info => (
            Color::rgba(0.928, 0.962, 0.993, 1.0),
            Color::rgba(0.16, 0.42, 0.67, 1.0),
            Color::rgba(0.09, 0.27, 0.42, 1.0),
        ),
        DebugTone::Success => (
            Color::rgba(0.938, 0.978, 0.950, 1.0),
            Color::rgba(0.14, 0.44, 0.24, 1.0),
            Color::rgba(0.10, 0.30, 0.17, 1.0),
        ),
        DebugTone::Warning => (
            Color::rgba(0.995, 0.964, 0.910, 1.0),
            Color::rgba(0.67, 0.39, 0.06, 1.0),
            Color::rgba(0.48, 0.29, 0.05, 1.0),
        ),
        DebugTone::Danger => (
            Color::rgba(0.994, 0.938, 0.934, 1.0),
            Color::rgba(0.69, 0.20, 0.19, 1.0),
            Color::rgba(0.48, 0.16, 0.15, 1.0),
        ),
    }
}

fn command_kind(command: &SceneCommand) -> &'static str {
    match command {
        SceneCommand::Clear(_) => "Clear",
        SceneCommand::FillRect { .. } => "FillRect",
        SceneCommand::StrokeRect { .. } => "StrokeRect",
        SceneCommand::FillPath { .. } => "FillPath",
        SceneCommand::StrokePath { .. } => "StrokePath",
        SceneCommand::DrawText(_) => "DrawText",
        SceneCommand::DrawShapedText(_) => "DrawShapedText",
        SceneCommand::DrawImage { .. } => "DrawImage",
        SceneCommand::PushClip { .. } => "PushClip",
        SceneCommand::PushClipPath { .. } => "PushClipPath",
        SceneCommand::PopClip => "PopClip",
        SceneCommand::PushTransform { .. } => "PushTransform",
        SceneCommand::PopTransform => "PopTransform",
        SceneCommand::Label { .. } => "Label",
    }
}

fn build_graph_tree_items(graph: &WidgetGraphSnapshot) -> Vec<TreeItem> {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();

    nodes
        .get(&graph.root)
        .map(|root| vec![graph_tree_item(*root, &nodes)])
        .unwrap_or_default()
}

fn graph_tree_item(
    node: &WidgetNodeSnapshot,
    nodes: &HashMap<WidgetId, &WidgetNodeSnapshot>,
) -> TreeItem {
    let mut detail = format!(
        "bounds={} focusable={} focused={}",
        format_rect(node.bounds),
        yes_no(node.accepts_focus),
        yes_no(node.focused),
    );

    if let Some(parent) = node.parent {
        detail.push_str(&format!(" parent=#{}", parent.get()));
    }

    let mut item = TreeItem::new(format!("Widget #{}", node.id.get())).detail(detail);
    if !node.children.is_empty() {
        item = item.expanded(true);
        for child in &node.children {
            if let Some(child_node) = nodes.get(child) {
                item = item.with_child(graph_tree_item(*child_node, nodes));
            }
        }
    }

    item
}

fn format_optional_widget_id(widget_id: Option<WidgetId>) -> String {
    widget_id
        .map(|id| format!("#{}", id.get()))
        .unwrap_or_else(|| "none".to_string())
}

fn format_schedule(schedule: FrameSchedule) -> String {
    let mut pending = Vec::new();
    if schedule.layout {
        pending.push("layout");
    }
    if schedule.paint {
        pending.push("paint");
    }
    if schedule.semantics {
        pending.push("semantics");
    }
    if schedule.hit_test {
        pending.push("hit-test");
    }
    if schedule.text {
        pending.push("text");
    }
    if schedule.resources {
        pending.push("resources");
    }

    if pending.is_empty() {
        "idle".to_string()
    } else {
        pending.join(", ")
    }
}

fn format_semantics_value(value: &SemanticsValue) -> String {
    match value {
        SemanticsValue::Text(text) => text.clone(),
        SemanticsValue::Number(value) => format!("{value:.2}"),
        SemanticsValue::Range { value, min, max } => format!("{value:.2} [{min:.2}..{max:.2}]"),
    }
}

fn format_rect(rect: Rect) -> String {
    format!(
        "({:.0}, {:.0}, {:.0}, {:.0})",
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
    )
}

fn format_size(size: Size) -> String {
    format!("{:.0} x {:.0}", size.width, size.height)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::SceneDebugSummary;
    use sui_core::{Color, DirtyRegion, InvalidationKind, Rect, WindowId};
    use sui_scene::{SceneCommand, SceneFrame};

    #[test]
    fn scene_summary_counts_commands_and_dirty_regions() {
        let mut frame = SceneFrame::new(WindowId::new(7), sui_core::Size::new(640.0, 360.0));
        frame
            .dirty_regions
            .push(DirtyRegion::new(Rect::new(12.0, 24.0, 100.0, 80.0), InvalidationKind::Paint));
        frame.scene.push(SceneCommand::Clear(Color::rgba(1.0, 1.0, 1.0, 1.0)));
        frame.scene.push(SceneCommand::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)));
        frame.scene.push(SceneCommand::Label {
            rect: Rect::new(0.0, 0.0, 120.0, 24.0),
            text: "stats".to_string(),
            color: Color::rgba(0.1, 0.1, 0.1, 1.0),
        });

        let summary = SceneDebugSummary::from(&frame);

        assert_eq!(summary.viewport, sui_core::Size::new(640.0, 360.0));
        assert_eq!(summary.command_count, 3);
        assert_eq!(summary.dirty_regions.len(), 1);
        assert!(summary.command_breakdown.contains(&("Clear".to_string(), 2)));
        assert!(summary.command_breakdown.contains(&("Label".to_string(), 1)));
    }
}