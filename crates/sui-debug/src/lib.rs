#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use sui_core::{Color, DirtyRegion, Rect, SemanticsValue, Size, WidgetId, WindowId};
use sui_layout::Alignment;
use sui_platform::AccessibilitySnapshot;
use sui_runtime::{
    CacheMetrics, CacheMetricsDelta, FocusState, FramePhase, FrameSchedule, SceneStatistics,
    Widget, WidgetGraphSnapshot, WidgetNodeSnapshot, WindowPerformanceSnapshot,
};
use sui_scene::{SceneCommand, SceneFrame};
use sui_widgets::{
    Background, Label, ListItem, ListView, Padding, ScrollView, Separator, SizedBox, Stack, Table,
    TableColumn, TableRow, TreeItem, TreeView,
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
    pub dirty_region_count: usize,
    pub dirty_regions: Vec<DirtyRegion>,
    pub command_count: usize,
    pub command_breakdown: Vec<(String, usize)>,
    pub layer_update_count: usize,
    pub layer_update_breakdown: Vec<(String, usize)>,
    pub stack_host_count: usize,
    pub stack_surface_count: usize,
    pub transient_surface_count: usize,
    pub detail_collected: bool,
}

impl From<&SceneFrame> for SceneDebugSummary {
    fn from(frame: &SceneFrame) -> Self {
        let mut command_breakdown = HashMap::<String, usize>::new();
        let mut command_count = 0usize;
        frame.scene.visit_commands(&mut |command| {
            command_count += 1;
            *command_breakdown
                .entry(command_kind(command).to_string())
                .or_default() += 1;
        });

        let mut command_breakdown: Vec<_> = command_breakdown.into_iter().collect();
        command_breakdown.sort_by(|left, right| left.0.cmp(&right.0));

        let mut layer_update_breakdown = HashMap::<String, usize>::new();
        for update in &frame.layer_updates {
            *layer_update_breakdown
                .entry(layer_update_kind(update.kind).to_string())
                .or_default() += 1;
        }
        let mut layer_update_breakdown: Vec<_> = layer_update_breakdown.into_iter().collect();
        layer_update_breakdown.sort_by(|left, right| left.0.cmp(&right.0));

        let mut stack_hosts = HashSet::new();
        let mut stack_surface_count = 0usize;
        let mut transient_surface_count = 0usize;
        frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.is_stack_surface {
                stack_surface_count += 1;
                stack_hosts.insert(layer.descriptor.stack_host);
                if layer.descriptor.transient_owner_surface.is_some() {
                    transient_surface_count += 1;
                }
            }
        });

        Self {
            viewport: frame.viewport,
            dirty_region_count: frame.dirty_regions.len(),
            dirty_regions: frame.dirty_regions.clone(),
            command_count,
            command_breakdown,
            layer_update_count: frame.layer_updates.len(),
            layer_update_breakdown,
            stack_host_count: stack_hosts.len(),
            stack_surface_count,
            transient_surface_count,
            detail_collected: true,
        }
    }
}

impl From<&SceneStatistics> for SceneDebugSummary {
    fn from(scene: &SceneStatistics) -> Self {
        Self {
            viewport: scene.viewport,
            dirty_region_count: scene.dirty_region_count,
            dirty_regions: scene.dirty_regions.clone(),
            command_count: scene.command_count,
            command_breakdown: scene.command_breakdown.clone(),
            layer_update_count: scene.layer_update_count,
            layer_update_breakdown: scene.layer_update_breakdown.clone(),
            stack_host_count: 0,
            stack_surface_count: 0,
            transient_surface_count: 0,
            detail_collected: scene.detail_mode.is_detailed(),
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
            14.0,
            Stack::vertical()
                .spacing(8.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(title)
                        .font_size(18.0)
                        .line_height(22.0)
                        .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
                )
                .with_child(
                    Label::new(subtitle)
                        .font_size(12.0)
                        .line_height(16.0)
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
    let mut column = Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch);

    for chunk in metrics.chunks(4) {
        let mut row = Stack::horizontal()
            .spacing(10.0)
            .alignment(Alignment::Stretch);
        for metric in chunk {
            row = row.with_child(
                SizedBox::new()
                    .width(156.0)
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
    let mut column = Stack::vertical().spacing(6.0).alignment(Alignment::Stretch);

    for (index, entry) in entries.into_iter().enumerate() {
        let background = if index % 2 == 0 {
            Color::rgba(0.971, 0.978, 0.989, 1.0)
        } else {
            Color::rgba(0.957, 0.968, 0.983, 1.0)
        };
        column = column.with_child(Background::new(
            background,
            Padding::all(
                8.0,
                Stack::horizontal()
                    .spacing(10.0)
                    .alignment(Alignment::Center)
                    .with_child(
                        SizedBox::new().width(160.0).with_child(
                            Label::new(entry.label)
                                .font_size(11.0)
                                .line_height(15.0)
                                .color(Color::rgba(0.38, 0.46, 0.55, 1.0)),
                        ),
                    )
                    .with_child(
                        Label::new(entry.value)
                            .font_size(12.0)
                            .line_height(16.0)
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

            let mut item =
                ListItem::new(format!("#{} {:?}", node.id.get(), node.role)).detail(detail);
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

    SizedBox::new()
        .height(220.0)
        .with_child(ListView::new("Accessibility snapshot").items(items))
}

pub fn widget_graph_snapshot_view(graph: WidgetGraphSnapshot) -> impl Widget {
    let items = build_graph_tree_items(&graph);
    SizedBox::new()
        .height(240.0)
        .with_child(TreeView::new("Widget graph").items(items))
}

pub fn scene_summary_view(scene: SceneDebugSummary) -> impl Widget {
    let metrics = debug_metric_grid([
        DebugMetric::new("Viewport", format_size(scene.viewport))
            .detail("Logical render target size")
            .tone(DebugTone::Info),
        DebugMetric::new("Dirty regions", scene.dirty_region_count.to_string())
            .detail("Invalidated rectangles queued into the scene")
            .tone(if scene.dirty_region_count == 0 {
                DebugTone::Success
            } else {
                DebugTone::Warning
            }),
        DebugMetric::new("Commands", scene.command_count.to_string())
            .detail("Scene commands emitted by the runtime")
            .tone(DebugTone::Neutral),
        DebugMetric::new("Layer updates", scene.layer_update_count.to_string())
            .detail("Per-layer update records for this frame")
            .tone(if scene.layer_update_count == 0 {
                DebugTone::Success
            } else {
                DebugTone::Info
            }),
        DebugMetric::new("Stack surfaces", scene.stack_surface_count.to_string())
            .detail(format!(
                "{} hosts, {} transient surfaces",
                scene.stack_host_count, scene.transient_surface_count
            ))
            .tone(if scene.stack_surface_count == 0 {
                DebugTone::Neutral
            } else {
                DebugTone::Info
            }),
    ]);

    let dirty_region_entries = if !scene.detail_collected {
        vec![DebugKeyValue::new(
            "Region state",
            "Detailed dirty-region rectangles were not collected for this frame",
        )]
    } else if scene.dirty_regions.is_empty() {
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
    let rows = if rows.is_empty() && !scene.detail_collected {
        vec![TableRow::new([
            "detail disabled".to_string(),
            "n/a".to_string(),
        ])]
    } else {
        rows
    };

    let layer_update_rows = scene
        .layer_update_breakdown
        .iter()
        .map(|(kind, count)| TableRow::new([kind.clone(), count.to_string()]))
        .collect::<Vec<_>>();
    let layer_update_rows = if layer_update_rows.is_empty() && !scene.detail_collected {
        vec![TableRow::new([
            "detail disabled".to_string(),
            "n/a".to_string(),
        ])]
    } else if layer_update_rows.is_empty() {
        vec![TableRow::new(["none".to_string(), "0".to_string()])]
    } else {
        layer_update_rows
    };

    Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(metrics)
        .with_child(debug_key_values(dirty_region_entries))
        .with_child(
            SizedBox::new().height(140.0).with_child(
                Table::new("Scene layer update breakdown")
                    .columns([
                        TableColumn::new("Update kind").min_width(180.0),
                        TableColumn::new("Count").width(90.0),
                    ])
                    .rows(layer_update_rows),
            ),
        )
        .with_child(
            SizedBox::new().height(172.0).with_child(
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
                .detail(format!(
                    "window focused={}",
                    yes_no(focus_state.window_focused)
                ))
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
            DebugMetric::new("Stack hosts", widget_graph.stack_hosts.len().to_string())
                .detail("Host-local surface ordering groups")
                .tone(DebugTone::Info),
            DebugMetric::new(
                "Scene frame",
                if scene_summary.is_some() {
                    "available"
                } else {
                    "missing"
                },
            )
            .detail("Present when a captured render summary exists")
            .tone(if scene_summary.is_some() {
                DebugTone::Success
            } else {
                DebugTone::Danger
            }),
        ]),
    );

    let mut body = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch);
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
        "Stack hosts",
        "Host-local ordering metadata makes floating and popup behavior inspectable without guessing from overlay layers.",
        stack_host_snapshot_view(widget_graph.clone()),
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
    let mut column = Stack::vertical().spacing(6.0).alignment(Alignment::Stretch);
    column.push(
        Label::new(metric.label)
            .font_size(11.0)
            .line_height(15.0)
            .color(label_color),
    );
    column.push(
        Label::new(metric.value)
            .font_size(19.0)
            .line_height(23.0)
            .color(value_color),
    );
    if let Some(detail) = metric.detail {
        column.push(
            Label::new(detail)
                .font_size(11.0)
                .line_height(15.0)
                .color(Color::rgba(0.40, 0.47, 0.56, 1.0)),
        );
    }

    Background::new(background, Padding::all(10.0, column))
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
        SceneCommand::DrawShapedTextWindow(_) => "DrawShapedTextWindow",
        SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => "DrawImage",
        SceneCommand::DrawShaderRect { .. } => "DrawShaderRect",
        SceneCommand::PushClip { .. } => "PushClip",
        SceneCommand::PushClipPath { .. } => "PushClipPath",
        SceneCommand::PopClip => "PopClip",
        SceneCommand::PushTransform { .. } => "PushTransform",
        SceneCommand::PopTransform => "PopTransform",
        SceneCommand::PushTextRenderPolicy { .. } => "PushTextRenderPolicy",
        SceneCommand::PopTextRenderPolicy => "PopTextRenderPolicy",
        SceneCommand::Layer(_) => "Layer",
        SceneCommand::FillRoundedRect { .. } => "FillRoundedRect",
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
        "layout={} input={} paint={} host=#{} surface=#{} owner={} order={} host_node={} surface_node={} policy={:?} focusable={} focused={}",
        format_rect(node.geometry.layout_bounds),
        format_rect(node.geometry.input_bounds),
        format_rect(node.geometry.paint_bounds),
        node.stack_host.get(),
        node.stack_surface.get(),
        format_optional_widget_id(node.transient_owner_surface),
        node.stack_surface_order,
        yes_no(node.is_stack_host),
        yes_no(node.is_stack_surface),
        node.stack_order_policy,
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
    if schedule.measure {
        pending.push("measure");
    }
    if schedule.arrange {
        pending.push("arrange");
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
        frame.dirty_regions.push(DirtyRegion::new(
            Rect::new(12.0, 24.0, 100.0, 80.0),
            InvalidationKind::Paint,
        ));
        frame
            .scene
            .push(SceneCommand::Clear(Color::rgba(1.0, 1.0, 1.0, 1.0)));
        frame
            .scene
            .push(SceneCommand::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)));
        frame.scene.push(SceneCommand::Label {
            rect: Rect::new(0.0, 0.0, 120.0, 24.0),
            text: "stats".to_string(),
            color: Color::rgba(0.1, 0.1, 0.1, 1.0),
        });

        let summary = SceneDebugSummary::from(&frame);

        assert_eq!(summary.viewport, sui_core::Size::new(640.0, 360.0));
        assert_eq!(summary.command_count, 3);
        assert_eq!(summary.dirty_regions.len(), 1);
        assert!(
            summary
                .command_breakdown
                .contains(&("Clear".to_string(), 2))
        );
        assert!(
            summary
                .command_breakdown
                .contains(&("Label".to_string(), 1))
        );
    }
}

pub fn performance_snapshot_view(snapshot: WindowPerformanceSnapshot) -> impl Widget {
    if !snapshot.scene.detail_mode.is_detailed() {
        let fps = if snapshot.total_time_ms > 0.0 {
            format!("{:.0} fps", 1000.0 / snapshot.total_time_ms)
        } else {
            "0 fps".to_string()
        };

        return Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(debug_metric_grid([
                DebugMetric::new("Frame", format_duration_ms(snapshot.total_time_ms))
                    .detail("Wall time across event handling, runtime, and renderer")
                    .tone(duration_tone(snapshot.total_time_ms)),
                DebugMetric::new("FPS", fps)
                    .detail("Live overlay detail is off; detailed analytics are disabled")
                    .tone(DebugTone::Neutral),
                DebugMetric::new(
                    "Event -> present",
                    format_latency_ms(snapshot.presentation_latency.event_to_present_ms),
                )
                .detail("Time from the latest non-redraw event to the end of the present call")
                .tone(latency_tone(
                    snapshot.presentation_latency.event_to_present_ms,
                )),
                DebugMetric::new(
                    "Redraw wait",
                    format_latency_ms(snapshot.presentation_latency.redraw_request_to_callback_ms),
                )
                .detail("Time spent waiting between request_redraw and RedrawRequested")
                .tone(latency_tone(
                    snapshot.presentation_latency.redraw_request_to_callback_ms,
                )),
            ]))
            .with_child(debug_key_values([
                DebugKeyValue::new("Frame index", snapshot.frame_index.to_string()),
                DebugKeyValue::new("Scene detail", snapshot.scene.detail_mode.label()),
            ]));
    }

    let slowest_phase = snapshot.slowest_phase();
    let slowest_label = slowest_phase
        .map(|sample| sample.phase.label())
        .unwrap_or("No measured work");
    let slowest_duration_ms = slowest_phase
        .map(|sample| sample.duration_ms)
        .unwrap_or(0.0);
    let renderer_submission = snapshot.renderer_submission;
    let renderer_work_ms: f64 = snapshot
        .phase_timings
        .iter()
        .filter(|sample| sample.phase == FramePhase::Renderer)
        .map(|sample| sample.duration_ms)
        .sum();
    let surface_wait_ms: f64 = snapshot
        .phase_timings
        .iter()
        .filter(|sample| sample.phase == FramePhase::SurfaceWait)
        .map(|sample| sample.duration_ms)
        .sum();

    let metrics = debug_metric_grid([
        DebugMetric::new("Frame", format_duration_ms(snapshot.total_time_ms))
            .detail("Wall time across event handling, runtime, and renderer")
            .tone(duration_tone(snapshot.total_time_ms)),
        DebugMetric::new(
            "Event -> render",
            format_latency_ms(snapshot.presentation_latency.event_to_render_start_ms),
        )
        .detail("Time from the latest non-redraw event to the start of runtime render work")
        .tone(latency_tone(
            snapshot.presentation_latency.event_to_render_start_ms,
        )),
        DebugMetric::new(
            "Event -> present",
            format_latency_ms(snapshot.presentation_latency.event_to_present_ms),
        )
        .detail("Time from the latest non-redraw event to the end of the present call")
        .tone(latency_tone(
            snapshot.presentation_latency.event_to_present_ms,
        )),
        DebugMetric::new(
            "Redraw wait",
            format_latency_ms(snapshot.presentation_latency.redraw_request_to_callback_ms),
        )
        .detail("Time spent waiting between request_redraw and RedrawRequested")
        .tone(latency_tone(
            snapshot.presentation_latency.redraw_request_to_callback_ms,
        )),
        DebugMetric::new("Slowest phase", slowest_label)
            .detail(format_duration_ms(slowest_duration_ms))
            .tone(duration_tone(slowest_duration_ms)),
        DebugMetric::new("Renderer work", format_duration_ms(renderer_work_ms))
            .detail("Renderer wall time after subtracting surface acquire and present waits")
            .tone(duration_tone(renderer_work_ms)),
        DebugMetric::new("Surface wait", format_duration_ms(surface_wait_ms))
            .detail(format!(
                "acquire {} | present {}",
                format_duration_ms(renderer_submission.surface_acquire_time_us as f64 / 1000.0),
                format_duration_ms(renderer_submission.surface_present_time_us as f64 / 1000.0),
            ))
            .tone(latency_tone(surface_wait_ms)),
        DebugMetric::new("Commands", snapshot.scene.command_count.to_string())
            .detail("Renderer-neutral draw commands in the current scene")
            .tone(DebugTone::Neutral),
        DebugMetric::new(
            "Dirty coverage",
            format!("{:.1}%", snapshot.scene.dirty_coverage),
        )
        .detail(format!(
            "{:.0} px^2 invalidated in this frame",
            snapshot.scene.dirty_area
        ))
        .tone(if snapshot.scene.dirty_regions.is_empty() {
            DebugTone::Success
        } else {
            DebugTone::Warning
        }),
        DebugMetric::new("Render passes", renderer_submission.pass_count.to_string())
            .detail("Render passes submitted for the frame")
            .tone(DebugTone::Neutral),
        DebugMetric::new("Draw calls", renderer_submission.draw_count.to_string())
            .detail("All GPU draw calls, including clip-mask submission")
            .tone(DebugTone::Neutral),
        DebugMetric::new(
            "Vertex upload",
            format_byte_size(renderer_submission.uploaded_vertex_bytes),
        )
        .detail(format!(
            "{} bytes written into scene and clip vertex buffers",
            renderer_submission.uploaded_vertex_bytes,
        ))
        .tone(upload_tone(renderer_submission.uploaded_vertex_bytes)),
        DebugMetric::new(
            "Visible layers",
            renderer_submission.visible_layer_count.to_string(),
        )
        .detail(format!(
            "{} retained direct packets composed across visible widget layers this frame",
            renderer_submission.direct_packet_count,
        ))
        .tone(DebugTone::Neutral),
        DebugMetric::new(
            "Direct packets",
            renderer_submission.direct_packet_count.to_string(),
        )
        .detail("Retained packet fragments submitted directly from explicit scene-layer boundaries")
        .tone(DebugTone::Neutral),
        DebugMetric::new(
            "State update",
            format_duration_ms(renderer_submission.retained_state_update_time_us as f64 / 1000.0),
        )
        .detail("Time spent reconciling retained compositor state before composition")
        .tone(duration_tone(
            renderer_submission.retained_state_update_time_us as f64 / 1000.0,
        )),
        DebugMetric::new(
            "Compose",
            format_duration_ms(renderer_submission.composition_time_us as f64 / 1000.0),
        )
        .detail("Time spent assembling the final retained composition draw list")
        .tone(duration_tone(
            renderer_submission.composition_time_us as f64 / 1000.0,
        )),
    ]);

    let phase_rows = snapshot
        .phase_timings
        .iter()
        .map(|sample| {
            let share = if snapshot.total_time_ms > 0.0 {
                (sample.duration_ms / snapshot.total_time_ms) * 100.0
            } else {
                0.0
            };

            TableRow::new([
                sample.phase.label().to_string(),
                format_duration_ms(sample.duration_ms),
                format!("{share:.1}%"),
            ])
        })
        .collect::<Vec<_>>();

    Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(metrics)
        .with_child(debug_key_values([
            DebugKeyValue::new("Frame index", snapshot.frame_index.to_string()),
            DebugKeyValue::new(
                "Dirty regions",
                snapshot.scene.dirty_region_count.to_string(),
            ),
            DebugKeyValue::new("Scene detail", snapshot.scene.detail_mode.label()),
            DebugKeyValue::new(
                "Text commands",
                snapshot.scene.text_command_count.to_string(),
            ),
            DebugKeyValue::new(
                "Image commands",
                snapshot.scene.image_command_count.to_string(),
            ),
            DebugKeyValue::new(
                "Clip commands",
                snapshot.scene.clip_command_count.to_string(),
            ),
            DebugKeyValue::new(
                "Transform commands",
                snapshot.scene.transform_command_count.to_string(),
            ),
        ]))
        .with_child(debug_key_values([
            DebugKeyValue::new(
                "Runtime text layout cache total",
                format_cache_metrics(snapshot.text_caches.runtime_layout),
            ),
            DebugKeyValue::new(
                "Renderer text layout cache total",
                format_cache_metrics(snapshot.text_caches.renderer_layout),
            ),
            DebugKeyValue::new(
                "Renderer glyph cache total",
                format_cache_metrics(snapshot.text_caches.renderer_glyph),
            ),
            DebugKeyValue::new(
                "Renderer path cache total",
                format_cache_metrics(snapshot.text_caches.renderer_path),
            ),
        ]))
        .with_child(debug_key_values([
            DebugKeyValue::new(
                "Runtime text layout cache frame delta",
                format_cache_delta_metrics(snapshot.text_cache_deltas.runtime_layout),
            ),
            DebugKeyValue::new(
                "Renderer text layout cache frame delta",
                format_cache_delta_metrics(snapshot.text_cache_deltas.renderer_layout),
            ),
            DebugKeyValue::new(
                "Renderer glyph cache frame delta",
                format_cache_delta_metrics(snapshot.text_cache_deltas.renderer_glyph),
            ),
            DebugKeyValue::new(
                "Renderer path cache frame delta",
                format_cache_delta_metrics(snapshot.text_cache_deltas.renderer_path),
            ),
        ]))
        .with_child(
            SizedBox::new().height(156.0).with_child(
                Table::new("Frame phase timings")
                    .columns([
                        TableColumn::new("Phase").min_width(180.0),
                        TableColumn::new("Duration").width(110.0),
                        TableColumn::new("Share").width(90.0),
                    ])
                    .rows(phase_rows),
            ),
        )
        .with_child(scene_summary_view(SceneDebugSummary::from(&snapshot.scene)))
}

fn duration_tone(duration_ms: f64) -> DebugTone {
    if duration_ms >= 33.0 {
        DebugTone::Danger
    } else if duration_ms >= 16.7 {
        DebugTone::Warning
    } else if duration_ms > 0.0 {
        DebugTone::Success
    } else {
        DebugTone::Neutral
    }
}

fn latency_tone(duration_ms: f64) -> DebugTone {
    if duration_ms <= 0.0 {
        DebugTone::Neutral
    } else {
        duration_tone(duration_ms)
    }
}

fn upload_tone(bytes: u64) -> DebugTone {
    if bytes >= 1_048_576 {
        DebugTone::Danger
    } else if bytes >= 262_144 {
        DebugTone::Warning
    } else if bytes > 0 {
        DebugTone::Info
    } else {
        DebugTone::Neutral
    }
}

fn format_duration_ms(duration_ms: f64) -> String {
    format!("{duration_ms:.2} ms")
}

fn format_latency_ms(duration_ms: f64) -> String {
    if duration_ms <= 0.0 {
        "n/a".to_string()
    } else {
        format_duration_ms(duration_ms)
    }
}

fn format_byte_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;

    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn format_cache_metrics(metrics: CacheMetrics) -> String {
    let requests = metrics.requests();
    if requests == 0 {
        format!("{} entries, no lookups yet", metrics.entries)
    } else {
        format!(
            "{} entries, {:.1}% hit rate ({} hits / {} lookups)",
            metrics.entries,
            metrics.hit_rate() * 100.0,
            metrics.hits,
            requests,
        )
    }
}

fn format_cache_delta_metrics(delta: CacheMetricsDelta) -> String {
    let requests = delta.requests();
    let entry_delta = match delta.entries_delta.cmp(&0) {
        std::cmp::Ordering::Greater => format!("+{} entries", delta.entries_delta),
        std::cmp::Ordering::Less => format!("{} entries", delta.entries_delta),
        std::cmp::Ordering::Equal => "no entry change".to_string(),
    };

    if requests == 0 {
        format!("{}, no cache lookups this frame", entry_delta)
    } else {
        format!(
            "{}, {:.1}% hit rate this frame ({} hits / {} lookups)",
            entry_delta,
            delta.hit_rate() * 100.0,
            delta.hits,
            requests,
        )
    }
}

fn stack_host_snapshot_view(graph: WidgetGraphSnapshot) -> impl Widget {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();

    let rows = graph
        .stack_hosts
        .iter()
        .map(|host| {
            let transient_count = host
                .surfaces
                .iter()
                .filter(|surface| {
                    nodes
                        .get(surface)
                        .is_some_and(|node| node.transient_owner_surface.is_some())
                })
                .count();
            TableRow::new([
                format!("#{}", host.host.get()),
                format!("{:?}", host.order_policy),
                host.surfaces.len().to_string(),
                transient_count.to_string(),
            ])
        })
        .collect::<Vec<_>>();
    let rows = if rows.is_empty() {
        vec![TableRow::new([
            "none".to_string(),
            "n/a".to_string(),
            "0".to_string(),
            "0".to_string(),
        ])]
    } else {
        rows
    };

    SizedBox::new().height(156.0).with_child(
        Table::new("Stack hosts")
            .columns([
                TableColumn::new("Host").min_width(100.0),
                TableColumn::new("Policy").min_width(120.0),
                TableColumn::new("Surfaces").width(84.0),
                TableColumn::new("Transient").width(84.0),
            ])
            .rows(rows),
    )
}

fn layer_update_kind(kind: sui_scene::SceneLayerUpdateKind) -> &'static str {
    match kind {
        sui_scene::SceneLayerUpdateKind::Ordering => "Ordering",
        sui_scene::SceneLayerUpdateKind::Content => "Content",
        sui_scene::SceneLayerUpdateKind::Transform => "Transform",
        sui_scene::SceneLayerUpdateKind::Clip => "Clip",
        sui_scene::SceneLayerUpdateKind::Effect => "Effect",
        sui_scene::SceneLayerUpdateKind::Visibility => "Visibility",
        sui_scene::SceneLayerUpdateKind::Resources => "Resources",
    }
}
