use std::{cell::Cell, rc::Rc};

use sui::{SemanticRegion, SingleChild, WidgetPodMutVisitor, WidgetPodVisitor, prelude::*};
use sui_nodes::{
    BackgroundVariant, Connection, Edge, EdgeKind, EdgeMarker, EdgePathOptions, EdgeReconnectMode,
    FitViewOptions, GraphDocument, GraphModel, GraphSnapshot, Node, NodeControls,
    NodeControlsAppearance, NodeExtent, NodeGraph, NodeGraphAppearance, NodeGraphConfig,
    NodeGraphEvent, NodeGraphState, NodeGraphSurface, NodeMiniMap, NodeMiniMapAppearance,
    NodeSignal, SelectionMode, Viewport,
};

use crate::app::{
    DEV_SHELL_LOGO_IMAGE_HANDLE, DemoTextRole, DevThemeReader, clone_dev_theme_reader,
    demo_text_style_when, request_window_refresh,
};

pub(crate) const NODES_TAB_LABEL: &str = "Node graphs";
pub(crate) const NODES_DEMO_NAME: &str = "Comprehensive node graph workspace";
pub(crate) const NODES_MAIN_GRAPH_NAME: &str = "Controlled workflow graph";
pub(crate) const NODES_SCRATCH_GRAPH_NAME: &str = "Paint-only retained widget graph";
pub(crate) const NODES_MINIMAP_NAME: &str = "Interactive workflow minimap";
pub(crate) const NODES_SIDEBAR_SCROLL_NAME: &str = "Node graph inspector";
pub(crate) const NODES_STATUS_NAME: &str = "Node graph event status";
pub(crate) const NODES_STATS_NAME: &str = "Node graph snapshot statistics";
pub(crate) const NODES_ADD_NODE_BUTTON: &str = "Add operation node";
pub(crate) const NODES_RESET_BUTTON: &str = "Reset node graph";
pub(crate) const NODES_DOCUMENT_BUTTON: &str = "Round-trip graph document";
pub(crate) const NODES_QUERY_BUTTON: &str = "Query selected connections";
const NODES_CONTROLS_WIDTH: f32 = 68.0;

type DemoGraphState = NodeGraphState<DemoNodeData, DemoEdgeData>;

#[derive(Debug, Clone, PartialEq, Default)]
struct DemoNodeData {
    detail: String,
    runs: u32,
}

impl DemoNodeData {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            runs: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct DemoEdgeData {
    note: String,
}

impl DemoEdgeData {
    fn new(note: impl Into<String>) -> Self {
        Self { note: note.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DemoTelemetry {
    proposals: u64,
    last_event: String,
}

struct ResponsiveNodesWorkspace {
    primary: SingleChild,
    sidebar: SingleChild,
    graph_state: DemoGraphState,
    theme_reader: DevThemeReader,
    breakpoint: f32,
    sidebar_width: f32,
    last_primary_size: Option<Size>,
}

impl ResponsiveNodesWorkspace {
    fn new<P, S>(
        primary: P,
        sidebar: S,
        graph_state: DemoGraphState,
        theme_reader: DevThemeReader,
    ) -> Self
    where
        P: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            primary: SingleChild::new(primary),
            sidebar: SingleChild::new(sidebar),
            graph_state,
            theme_reader,
            breakpoint: 1120.0,
            sidebar_width: 320.0,
            last_primary_size: None,
        }
    }

    fn compact_sidebar_height(&self, bounds: Rect) -> f32 {
        (bounds.height() * 0.20).clamp(120.0, 200.0)
    }
}

impl Widget for ResponsiveNodesWorkspace {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1100.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                680.0
            },
        ));
        if size.width >= self.breakpoint {
            let sidebar_width = self.sidebar_width.min(size.width * 0.42);
            self.primary.measure(
                ctx,
                Constraints::tight(Size::new(
                    (size.width - sidebar_width - 1.0).max(1.0),
                    size.height,
                )),
            );
            self.sidebar.measure(
                ctx,
                Constraints::tight(Size::new(sidebar_width, size.height)),
            );
        } else {
            let sidebar_height =
                self.compact_sidebar_height(Rect::from_origin_size(Point::ZERO, size));
            self.primary.measure(
                ctx,
                Constraints::tight(Size::new(
                    size.width,
                    (size.height - sidebar_height - 1.0).max(1.0),
                )),
            );
            self.sidebar.measure(
                ctx,
                Constraints::tight(Size::new(size.width, sidebar_height)),
            );
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let (primary, sidebar) = if bounds.width() >= self.breakpoint {
            let sidebar_width = self.sidebar_width.min(bounds.width() * 0.42);
            let primary = Rect::new(
                bounds.x(),
                bounds.y(),
                (bounds.width() - sidebar_width - 1.0).max(1.0),
                bounds.height(),
            );
            let sidebar = Rect::new(
                primary.max_x() + 1.0,
                bounds.y(),
                sidebar_width,
                bounds.height(),
            );
            (primary, sidebar)
        } else {
            let sidebar_height = self.compact_sidebar_height(bounds);
            let primary = Rect::new(
                bounds.x(),
                bounds.y(),
                bounds.width(),
                (bounds.height() - sidebar_height - 1.0).max(1.0),
            );
            let sidebar = Rect::new(
                bounds.x(),
                primary.max_y() + 1.0,
                bounds.width(),
                sidebar_height,
            );
            (primary, sidebar)
        };
        let graph_size = Size::new(
            (primary.width() - NODES_CONTROLS_WIDTH - 1.0).max(1.0),
            primary.height(),
        );
        if self
            .last_primary_size
            .is_some_and(|previous| previous != graph_size)
        {
            self.graph_state.fit_view(
                graph_size,
                FitViewOptions::default()
                    .padding(12.0)
                    .zoom_range(0.35, 2.2),
            );
        }
        self.last_primary_size = Some(graph_size);
        self.primary.arrange(ctx, primary);
        self.sidebar.arrange(ctx, sidebar);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.primary.paint(ctx);
        self.sidebar.paint(ctx);
        let color = (self.theme_reader)().palette.border;
        if ctx.bounds().width() >= self.breakpoint {
            let x = self.primary.child().bounds().max_x();
            ctx.fill_rect(
                Rect::new(x, ctx.bounds().y(), 1.0, ctx.bounds().height()),
                color,
            );
        } else {
            let y = self.primary.child().bounds().max_y();
            ctx.fill_rect(
                Rect::new(ctx.bounds().x(), y, ctx.bounds().width(), 1.0),
                color,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.primary.semantics(ctx);
        self.sidebar.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.primary.visit_children(visitor);
        self.sidebar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.primary.visit_children_mut(visitor);
        self.sidebar.visit_children_mut(visitor);
    }
}

impl Default for DemoTelemetry {
    fn default() -> Self {
        Self {
            proposals: 0,
            last_event:
                "Ready — drag nodes, resize the selected node, or reconnect the selected edge."
                    .to_string(),
        }
    }
}

pub(crate) fn build_nodes_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let telemetry = Signal::named("Node graph demo telemetry", DemoTelemetry::default());
    let initial_document = comprehensive_document();
    let graph = GraphModel::new(
        initial_document.nodes.clone(),
        initial_document.edges.clone(),
    )
    .expect("node demo graph should be valid");
    let controlled =
        NodeGraphState::controlled(GraphSnapshot::new(graph).viewport(initial_document.viewport));
    let accept = controlled.clone();
    let controlled_telemetry = telemetry.clone();
    controlled.set_change_handler(move |snapshot| {
        controlled_telemetry.update(|telemetry| {
            telemetry.proposals = telemetry.proposals.saturating_add(1);
        });
        accept
            .replace_snapshot(snapshot)
            .expect("controlled demo should accept valid proposals");
    });

    let event_telemetry = telemetry.clone();
    let edge_sequence = Rc::new(Cell::new(100_u64));
    let edge_ids = Rc::clone(&edge_sequence);
    let operation_state = controlled.clone();
    let operation_status = telemetry.clone();
    let operation_theme = Rc::clone(&theme_reader);
    let decision_state = controlled.clone();
    let decision_status = telemetry.clone();
    let decision_theme = Rc::clone(&theme_reader);
    let group_theme = Rc::clone(&theme_reader);
    let media_theme = Rc::clone(&theme_reader);
    let main_graph = NodeGraph::new(NODES_MAIN_GRAPH_NAME, controlled.clone())
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .appearance(NodeGraphAppearance {
            grid: Some(Color::rgba(0.18, 0.50, 0.74, 0.18)),
            source_handle: Some(Color::rgba(0.18, 0.66, 0.88, 1.0)),
            target_handle: Some(Color::rgba(0.96, 0.60, 0.18, 1.0)),
            ..NodeGraphAppearance::default()
        })
        .config(NodeGraphConfig {
            fit_view_on_init: true,
            fit_view: FitViewOptions::default()
                .padding(24.0)
                .zoom_range(0.35, 2.2),
            min_zoom: 0.35,
            max_zoom: 2.2,
            background_variant: BackgroundVariant::Dots,
            selection_mode: SelectionMode::Partial,
            snap_to_grid: Some(Size::new(16.0, 16.0)),
            connection_line_kind: EdgeKind::SmoothStep,
            ..NodeGraphConfig::default()
        })
        .is_valid_connection(|connection, graph| {
            connection.source != connection.target && !graph.connection_exists(connection)
        })
        .edge_factory(move |connection| {
            let sequence = edge_ids.get();
            edge_ids.set(sequence.saturating_add(1));
            edge_from_connection(
                format!("user-edge-{sequence}"),
                connection,
                EdgeKind::SmoothStep,
                "Created interactively",
            )
        })
        .node_type("group", move |_id, node| {
            build_group_node(node, Rc::clone(&group_theme))
        })
        .node_type("operation", move |id, node| {
            build_operation_node(
                id.clone(),
                node,
                operation_state.clone(),
                operation_status.clone(),
                Rc::clone(&operation_theme),
            )
        })
        .node_type("decision", move |id, node| {
            build_decision_node(
                id.clone(),
                node,
                decision_state.clone(),
                decision_status.clone(),
                Rc::clone(&decision_theme),
            )
        })
        .node_type("media", move |_id, node| {
            build_media_node(node, Rc::clone(&media_theme))
        })
        .on_change(move |event| {
            event_telemetry.update(|telemetry| {
                telemetry.last_event = describe_event(&event);
            });
        });

    let controls = NodeControls::new("Workflow viewport controls", controlled.clone())
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .appearance(NodeControlsAppearance {
            active: Some(Color::rgba(0.18, 0.64, 0.88, 1.0)),
            ..NodeControlsAppearance::default()
        })
        .zoom_range(0.35, 2.2)
        .transition_duration(0.28);
    let controls_surface = Background::new(
        theme_reader().palette.surface_raised,
        Padding::all(7.0, controls).fill_child_width(),
    )
    .brush_when({
        let theme = Rc::clone(&theme_reader);
        move || theme().palette.surface_raised
    });
    let graph_stage = FixedPaneSplit::horizontal(
        controls_surface,
        Separator::vertical().theme_when(clone_dev_theme_reader(&theme_reader)),
        main_graph,
    )
    .fixed_first(NODES_CONTROLS_WIDTH)
    .divider_extent(1.0);

    let sidebar = build_sidebar(
        controlled.clone(),
        telemetry.clone(),
        initial_document.clone(),
        Rc::clone(&theme_reader),
    );
    let workspace = ResponsiveNodesWorkspace::new(
        graph_stage,
        sidebar,
        controlled.clone(),
        Rc::clone(&theme_reader),
    );

    let header = build_header(
        controlled,
        telemetry,
        initial_document,
        Rc::clone(&theme_reader),
    );
    SemanticRegion::new(
        NODES_DEMO_NAME,
        Background::new(
            theme_reader().palette.surface,
            Stack::vertical()
                .alignment(Alignment::Stretch)
                .with_child(header)
                .with_child(workspace),
        )
        .brush_when({
            let theme = Rc::clone(&theme_reader);
            move || theme().palette.surface
        }),
    )
    .description(
        "Controlled and uncontrolled node graphs demonstrating retained nodes, subflows, indexing, semantics, editing, and viewport behavior.",
    )
}

fn build_header(
    state: DemoGraphState,
    telemetry: Signal<DemoTelemetry>,
    initial_document: GraphDocument<DemoNodeData, DemoEdgeData>,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let next_node = Rc::new(Cell::new(1_u64));
    let add_state = state.clone();
    let add_telemetry = telemetry.clone();
    let add_sequence = Rc::clone(&next_node);
    let add = Button::primary(NODES_ADD_NODE_BUTTON)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_press_with_ctx(move |ctx| {
            let sequence = add_sequence.get();
            add_sequence.set(sequence.saturating_add(1));
            let id = format!("added-{sequence}");
            let position = Point::new(44.0 + (sequence as f32 * 36.0), 250.0);
            add_state
                .add_node(
                    Node::new(
                        id.clone(),
                        position,
                        DemoNodeData::new("Created through the controlled state API"),
                    )
                    .kind("operation")
                    .parent("pipeline")
                    .extent(NodeExtent::Parent)
                    .expand_parent(true)
                    .content_sized(Size::new(180.0, 92.0), Size::new(300.0, 160.0))
                    .resizable(true),
                )
                .expect("new demo node should be valid");
            add_telemetry.update(|telemetry| {
                telemetry.last_event = format!("Added {id} through NodeGraphState::add_node");
            });
            request_window_refresh(ctx, true);
        });

    let reset_state = state.clone();
    let reset_telemetry = telemetry.clone();
    let reset_document = initial_document.clone();
    let reset = Button::new(NODES_RESET_BUTTON)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_press_with_ctx(move |ctx| {
            reset_state
                .restore_document(reset_document.clone())
                .expect("demo reset document should be valid");
            reset_telemetry.update(|telemetry| {
                telemetry.last_event = "Restored the portable GraphDocument".to_string();
            });
            request_window_refresh(ctx, true);
        });

    let document_state = state.clone();
    let document_telemetry = telemetry.clone();
    let document = Button::new(NODES_DOCUMENT_BUTTON)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_press_with_ctx(move |ctx| {
            let document = document_state.to_document();
            document_state
                .restore_document(document)
                .expect("round-tripped document should remain valid");
            document_telemetry.update(|telemetry| {
                telemetry.last_event =
                    "Round-tripped nodes, edges, viewport, and interaction mode".to_string();
            });
            request_window_refresh(ctx, true);
        });

    let query_state = state;
    let query_telemetry = telemetry;
    let query = Button::new(NODES_QUERY_BUTTON)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_press_with_ctx(move |ctx| {
            let snapshot = query_state.snapshot();
            let selected = snapshot.graph.selected_node_ids();
            let connected = query_state.connected_edges(&selected);
            let bounds = query_state.nodes_bounds(&selected);
            query_telemetry.update(|telemetry| {
                telemetry.last_event = format!(
                    "Query: {} selected nodes, {} connected edges, bounds {bounds:?}",
                    selected.len(),
                    connected.len()
                );
            });
            request_window_refresh(ctx, false);
        });

    Background::new(
        theme_reader().palette.surface_raised,
        Padding::symmetric(
            12.0,
            9.0,
            Flex::horizontal()
                .gap(8.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Center)
                .with_item(add, FlexItem::new())
                .with_item(reset, FlexItem::new())
                .with_item(document, FlexItem::new())
                .with_item(query, FlexItem::new()),
        ),
    )
    .brush_when({
        let theme = Rc::clone(&theme_reader);
        move || theme().palette.surface_raised
    })
}

fn build_sidebar(
    state: DemoGraphState,
    telemetry: Signal<DemoTelemetry>,
    _initial_document: GraphDocument<DemoNodeData, DemoEdgeData>,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let minimap = NodeMiniMap::new(NODES_MINIMAP_NAME, state.clone())
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .appearance(NodeMiniMapAppearance {
            viewport_border: Some(Color::rgba(0.18, 0.64, 0.88, 1.0)),
            ..NodeMiniMapAppearance::default()
        })
        .desired_size(Size::new(300.0, 160.0))
        .pannable(true)
        .zoomable(true)
        .zoom_range(0.35, 2.2);

    let stats = state.observable().select_named(
        "Node graph demo statistics",
        |snapshot| {
            format!(
                "{} nodes · {} edges · selected {} · zoom {:.0}%\ndoc r{} · nodes r{} · edges r{} · viewport r{} · spatial r{}",
                snapshot.graph.nodes.len(),
                snapshot.graph.edges.len(),
                snapshot.graph.selected_node_ids().len()
                    + snapshot.graph.selected_edge_ids().len(),
                snapshot.viewport.zoom * 100.0,
                snapshot.revisions.document,
                snapshot.revisions.nodes,
                snapshot.revisions.edges,
                snapshot.revisions.viewport,
                snapshot.spatial.revision(),
            )
        },
    );
    let status = telemetry.select_named("Node graph demo event summary", |telemetry| {
        format!(
            "Controlled proposals accepted: {}\nLast lifecycle event: {}",
            telemetry.proposals, telemetry.last_event
        )
    });

    let scratch = build_uncontrolled_scratch(Rc::clone(&theme_reader));
    ScrollView::vertical(Padding::all(
        14.0,
        Flex::horizontal()
            .gap(14.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Start)
            .with_item(
                demo_section(
                "Controlled state + incremental index",
                Stack::vertical()
                    .spacing(6.0)
                    .with_child(
                        Label::new("")
                            .text_from(stats)
                            .semantic_name(NODES_STATS_NAME)
                            .style_when(demo_text_style_when(
                                &theme_reader,
                                DemoTextRole::Supporting,
                                |theme| theme.palette.text_muted,
                            )),
                    )
                    .with_child(
                        Label::new("")
                            .text_from(status)
                            .semantic_name(NODES_STATUS_NAME)
                            .style_when(demo_text_style_when(
                                &theme_reader,
                                DemoTextRole::Supporting,
                                |theme| theme.palette.text,
                            )),
                    ),
                Rc::clone(&theme_reader),
                ),
                FlexItem::new().basis(300.0).grow(1.0).min_width(260.0),
            )
            .with_item(
                demo_section(
                    "Pannable + zoomable minimap",
                    minimap,
                    Rc::clone(&theme_reader),
                ),
                FlexItem::new().basis(300.0).grow(1.0).min_width(260.0),
            )
            .with_item(
                demo_section(
                    "Canvas-backed surface + retained widgets",
                    SizedBox::new()
                        .size(Size::new(304.0, 200.0))
                        .with_child(scratch),
                    Rc::clone(&theme_reader),
                ),
                FlexItem::new().basis(300.0).grow(1.0).min_width(260.0),
            )
            .with_item(
                demo_section(
                    "Interaction checklist",
                    Label::new(
                        "• drag and multi-select nodes\n• resize the selected Normalize node\n• reconnect the selected smooth-step edge\n• drag handles to add a validated edge\n• pan, pointer-anchored wheel/pinch zoom, and auto-pan\n• Tab through element semantics; Enter selects; Delete removes",
                    )
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Supporting,
                        |theme| theme.palette.text_muted,
                    )),
                    Rc::clone(&theme_reader),
                ),
                FlexItem::new().basis(300.0).grow(1.0).min_width(260.0),
            ),
    ))
    .name(NODES_SIDEBAR_SCROLL_NAME)
    .theme_when(clone_dev_theme_reader(&theme_reader))
}

fn build_uncontrolled_scratch(theme_reader: DevThemeReader) -> impl Widget {
    let state = NodeGraphState::<DemoNodeData, DemoEdgeData>::new(
        vec![
            Node::new(
                "scratch-a",
                Point::new(10.0, 38.0),
                DemoNodeData::new("Direct state"),
            )
            .kind("scratch-widget")
            .label("Scratch A")
            .size(Size::new(112.0, 72.0)),
            Node::new(
                "scratch-b",
                Point::new(145.0, 102.0),
                DemoNodeData::new("Retained widget"),
            )
            .kind("scratch-widget")
            .label("Scratch B")
            .size(Size::new(112.0, 72.0)),
        ],
        vec![
            Edge::new(
                "scratch-edge",
                "scratch-a",
                "scratch-b",
                DemoEdgeData::new("Painter callback"),
            )
            .kind(EdgeKind::SimpleBezier),
        ],
    )
    .expect("scratch graph should be valid");
    let scratch_theme = Rc::clone(&theme_reader);
    NodeGraphSurface::new(NODES_SCRATCH_GRAPH_NAME, state)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .appearance(NodeGraphAppearance {
            edge: Some(Color::rgba(0.92, 0.48, 0.18, 1.0)),
            ..NodeGraphAppearance::default()
        })
        .config(NodeGraphConfig {
            fit_view_on_init: true,
            fit_view: FitViewOptions::default().padding(12.0).zoom_range(0.5, 2.0),
            min_zoom: 0.5,
            max_zoom: 2.0,
            background_variant: BackgroundVariant::Cross,
            grid_spacing: 20.0,
            ..NodeGraphConfig::default()
        })
        .node_type("scratch-widget", move |_id, node| {
            build_scratch_node(node, Rc::clone(&scratch_theme))
        })
}

fn build_group_node(node: NodeSignal<DemoNodeData>, theme_reader: DevThemeReader) -> impl Widget {
    let title = node.select_named("Pipeline group title", |node| node.label.clone());
    Padding::all(
        12.0,
        Align::new(
            Alignment::Stretch,
            Alignment::Start,
            Label::new("")
                .text_from(title)
                .style_when(demo_text_style_when(
                    &theme_reader,
                    DemoTextRole::CardTitle,
                    |theme| theme.palette.text_muted,
                )),
        ),
    )
    .fill_child()
}

fn build_media_node(node: NodeSignal<DemoNodeData>, theme_reader: DevThemeReader) -> impl Widget {
    let title = node.select_named("Media node title", |node| node.label.clone());
    let detail = node.select_named("Media node detail", |node| node.data.detail.clone());
    let icon_theme = Rc::clone(&theme_reader);
    Padding::all(
        10.0,
        Stack::horizontal()
            .spacing(8.0)
            .alignment(Alignment::Center)
            .with_child(
                Stack::vertical()
                    .spacing(2.0)
                    .alignment(Alignment::Center)
                    .with_child(
                        Image::new(DEV_SHELL_LOGO_IMAGE_HANDLE)
                            .size(Size::new(22.0, 22.0))
                            .fit(ImageFit::Contain)
                            .theme_when(clone_dev_theme_reader(&theme_reader)),
                    )
                    .with_child(
                        Icon::new(IconGlyph::Send)
                            .size(14.0)
                            .label("Publish vector icon")
                            .theme(theme_reader())
                            .color_when(move || icon_theme().palette.accent),
                    ),
            )
            .with_child(
                Stack::vertical()
                    .spacing(2.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new("")
                            .text_from(title)
                            .style_when(demo_text_style_when(
                                &theme_reader,
                                DemoTextRole::CardTitle,
                                |theme| theme.palette.text,
                            )),
                    )
                    .with_child(
                        Label::new("")
                            .text_from(detail)
                            .style_when(demo_text_style_when(
                                &theme_reader,
                                DemoTextRole::Metadata,
                                |theme| theme.palette.text_muted,
                            )),
                    ),
            ),
    )
    .fill_child()
}

fn build_scratch_node(node: NodeSignal<DemoNodeData>, theme_reader: DevThemeReader) -> impl Widget {
    let title = node.select_named("Scratch node title", |node| node.label.clone());
    let detail = node.select_named("Scratch node detail", |node| node.data.detail.clone());
    Padding::all(
        9.0,
        Stack::vertical()
            .spacing(2.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new("")
                    .text_from(title)
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::CardTitle,
                        |theme| theme.palette.text,
                    )),
            )
            .with_child(
                Label::new("")
                    .text_from(detail)
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Metadata,
                        |theme| theme.palette.text_muted,
                    )),
            ),
    )
    .fill_child()
}

fn build_operation_node(
    id: sui_nodes::NodeId,
    node: NodeSignal<DemoNodeData>,
    graph: DemoGraphState,
    telemetry: Signal<DemoTelemetry>,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let title = node.select_named(format!("{id} title"), |node| node.label.clone());
    let details = node.select_named(format!("{id} details"), |node| {
        format!("{} · executions {}", node.data.detail, node.data.runs)
    });
    let action_id = id;
    let action_state = graph;
    let action_telemetry = telemetry;
    Padding::all(
        10.0,
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new("")
                    .text_from(title)
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::CardTitle,
                        |theme| theme.palette.text,
                    )),
            )
            .with_child(
                Label::new("")
                    .text_from(details)
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Metadata,
                        |theme| theme.palette.text_muted,
                    )),
            )
            .with_child(
                Button::new("Run node")
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .on_press_with_ctx(move |ctx| {
                        action_state.update_node_data(&action_id, |data| {
                            data.runs = data.runs.saturating_add(1);
                        });
                        action_telemetry.update(|telemetry| {
                            telemetry.last_event =
                                format!("Interactive retained child ran node {}", action_id);
                        });
                        request_window_refresh(ctx, true);
                    }),
            ),
    )
    .fill_child_width()
}

fn build_decision_node(
    id: sui_nodes::NodeId,
    node: NodeSignal<DemoNodeData>,
    graph: DemoGraphState,
    telemetry: Signal<DemoTelemetry>,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let title = node.select_named(format!("{id} decision title"), |node| node.label.clone());
    let details = node.select_named(format!("{id} decision details"), |node| {
        format!("{} · toggles {}", node.data.detail, node.data.runs)
    });
    let action_id = id;
    Padding::all(
        10.0,
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new("")
                    .text_from(title)
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::CardTitle,
                        |theme| theme.palette.text,
                    )),
            )
            .with_child(
                Label::new("")
                    .text_from(details)
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Metadata,
                        |theme| theme.palette.warning_text,
                    )),
            )
            .with_child(
                Button::new("Toggle route")
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .on_press_with_ctx(move |ctx| {
                        graph.update_node_data(&action_id, |data| {
                            data.runs = data.runs.saturating_add(1);
                        });
                        telemetry.update(|telemetry| {
                            telemetry.last_event =
                                format!("Decision node {} handled its own button event", action_id);
                        });
                        request_window_refresh(ctx, true);
                    }),
            ),
    )
    .fill_child_width()
}

fn demo_section<W>(title: &'static str, body: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Background::new(
        theme_reader().palette.surface_raised,
        Padding::all(
            12.0,
            Stack::vertical()
                .spacing(8.0)
                .alignment(Alignment::Stretch)
                .with_child(Label::new(title).style_when(demo_text_style_when(
                    &theme_reader,
                    DemoTextRole::CardTitle,
                    |theme| theme.palette.text,
                )))
                .with_child(body),
        ),
    )
    .brush_when({
        let theme = Rc::clone(&theme_reader);
        move || theme().palette.surface_raised
    })
}

fn comprehensive_document() -> GraphDocument<DemoNodeData, DemoEdgeData> {
    let mut pipeline = Node::new(
        "pipeline",
        Point::new(20.0, 40.0),
        DemoNodeData::new("Subflow parent"),
    )
    .label("Pipeline subflow")
    .kind("group")
    .size(Size::new(560.0, 330.0))
    .handles(Vec::new())
    .resizable(true)
    .z_index(-10)
    .aria_label("Resizable pipeline group");
    pipeline.selectable = true;

    let source = Node::new(
        "source",
        Point::new(20.0, 58.0),
        DemoNodeData::new("Retained input"),
    )
    .label("Load source")
    .kind("operation")
    .parent("pipeline")
    .extent(NodeExtent::Parent)
    .content_sized(Size::new(190.0, 100.0), Size::new(240.0, 170.0))
    .resizable(true)
    .aria_label("Load source operation");

    let mut normalize = Node::new(
        "normalize",
        Point::new(280.0, 58.0),
        DemoNodeData::new("Eight resize handles"),
    )
    .label("Normalize")
    .kind("operation")
    .parent("pipeline")
    .extent(NodeExtent::Parent)
    .content_sized(Size::new(210.0, 105.0), Size::new(270.0, 180.0))
    .resizable(true)
    .size_limits(Size::new(170.0, 90.0), Size::new(360.0, 220.0));
    normalize.selected = true;

    let decision = Node::new(
        "decision",
        Point::new(280.0, 240.0),
        DemoNodeData::new("Centered-origin decision node"),
    )
    .label("Quality gate")
    .kind("decision")
    .parent("pipeline")
    .extent(NodeExtent::Parent)
    .origin(Point::new(0.5, 0.5))
    .content_sized(Size::new(190.0, 105.0), Size::new(250.0, 180.0))
    .resizable(true);

    let output = Node::new(
        "output",
        Point::new(600.0, 166.0),
        DemoNodeData::new("Outside the subflow"),
    )
    .label("Publish output")
    .kind("media")
    .size(Size::new(165.0, 84.0))
    .resizable(true)
    .aria_label("Publish workflow output");

    let mut smooth = Edge::new(
        "normalize-decision",
        "normalize",
        "decision",
        DemoEdgeData::new("Selected and reconnectable"),
    )
    .kind(EdgeKind::SmoothStep)
    .path_options(EdgePathOptions {
        border_radius: 14.0,
        step_offset: 28.0,
        ..EdgePathOptions::default()
    })
    .start_marker(Some(EdgeMarker::Circle))
    .end_marker(Some(EdgeMarker::ArrowClosed))
    .reconnectable(EdgeReconnectMode::Both);
    smooth.selected = true;

    GraphDocument::new(
        vec![pipeline, source, normalize, decision, output],
        vec![
            Edge::new(
                "source-normalize",
                "source",
                "normalize",
                DemoEdgeData::new("Animated bezier"),
            )
            .kind(EdgeKind::Bezier)
            .animated(true)
            .animation_speed(0.75),
            smooth,
            Edge::new(
                "decision-output",
                "decision",
                "output",
                DemoEdgeData::new("Stepped cross-subflow edge"),
            )
            .kind(EdgeKind::Step)
            .end_marker(Some(EdgeMarker::Arrow)),
            Edge::new(
                "source-decision",
                "source",
                "decision",
                DemoEdgeData::new("Straight comparison edge"),
            )
            .kind(EdgeKind::Straight)
            .end_marker(None),
            Edge::new(
                "normalize-output",
                "normalize",
                "output",
                DemoEdgeData::new("Simple bezier comparison edge"),
            )
            .kind(EdgeKind::SimpleBezier)
            .path_options(EdgePathOptions {
                curvature: 0.32,
                ..EdgePathOptions::default()
            }),
        ],
    )
    .viewport(Viewport::new(20.0, 20.0, 0.88))
}

fn edge_from_connection(
    id: impl Into<sui_nodes::EdgeId>,
    connection: Connection,
    kind: EdgeKind,
    note: impl Into<String>,
) -> Edge<DemoEdgeData> {
    let mut edge = Edge::new(
        id,
        connection.source.clone(),
        connection.target.clone(),
        DemoEdgeData::new(note),
    )
    .kind(kind);
    edge.source_handle = connection.source_handle;
    edge.target_handle = connection.target_handle;
    edge
}

fn describe_event(event: &NodeGraphEvent) -> String {
    let text = format!("{event:?}");
    const LIMIT: usize = 180;
    if text.chars().count() <= LIMIT {
        text
    } else {
        format!("{}…", text.chars().take(LIMIT).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui::WgpuRenderer;

    #[test]
    fn comprehensive_document_covers_every_edge_kind_and_subflows() {
        let document = comprehensive_document();
        let kinds = document
            .edges
            .iter()
            .map(|edge| edge.kind)
            .collect::<Vec<_>>();

        for kind in [
            EdgeKind::Bezier,
            EdgeKind::SimpleBezier,
            EdgeKind::Straight,
            EdgeKind::Step,
            EdgeKind::SmoothStep,
        ] {
            assert!(kinds.contains(&kind));
        }
        assert!(document.nodes.iter().any(|node| node.parent_id.is_some()));
        assert!(document.nodes.iter().any(|node| node.resizable));
        assert!(document.edges.iter().any(|edge| edge.animated));
        assert!(document.edges.iter().any(|edge| edge.selected));
    }

    #[test]
    #[ignore = "diagnostic benchmark for retained node graph runtime and GPU zoom frames"]
    fn retained_node_graph_gpu_zoom_current_status_benchmark() -> Result<()> {
        const COLUMNS: usize = 24;
        const ROWS: usize = 16;
        const FRAMES: usize = 120;
        let mut nodes = Vec::with_capacity(COLUMNS * ROWS);
        let mut edges = Vec::new();
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let index = row * COLUMNS + column;
                nodes.push(
                    Node::new(
                        format!("gpu-node-{index}"),
                        Point::new(column as f32 * 120.0, row as f32 * 64.0),
                        DemoNodeData::new(format!("item {index}")),
                    )
                    .kind("gpu-benchmark")
                    .label(format!("Node {index}"))
                    .size(Size::new(100.0, 48.0)),
                );
                if column > 0 {
                    edges.push(Edge::new(
                        format!("gpu-edge-{index}"),
                        format!("gpu-node-{}", index - 1),
                        format!("gpu-node-{index}"),
                        DemoEdgeData::new("benchmark"),
                    ));
                }
            }
        }
        let state = NodeGraphState::new(nodes, edges).expect("valid GPU benchmark graph");
        let graph = NodeGraph::new("GPU node benchmark", state.clone())
            .config(NodeGraphConfig {
                min_zoom: 0.1,
                max_zoom: 2.0,
                background_variant: BackgroundVariant::Dots,
                ..NodeGraphConfig::default()
            })
            .node_type("gpu-benchmark", |_id, node| {
                let title =
                    node.select_named("GPU benchmark node title", |node| node.label.clone());
                Padding::all(6.0, Label::new("").text_from(title)).fill_child()
            });
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("GPU node benchmark").root(graph))
            .build()?;
        let window_id = runtime.window_ids()[0];
        let detailed_profile = std::env::var_os("SUI_NODE_BENCH_PROFILE").is_some();
        if detailed_profile {
            sui::set_window_scene_statistics_detail_mode(
                window_id,
                sui::SceneStatisticsDetailMode::Detailed,
            );
        }
        let initial = runtime.render(window_id)?;
        let mut renderer = WgpuRenderer::new();
        renderer.render(&initial.frame)?;
        let viewport_size = state.viewport_size();
        let center = Point::new(1_430.0, 510.0);
        let mut runtime_time = std::time::Duration::ZERO;
        let mut renderer_time = std::time::Duration::ZERO;
        let mut draw_count = 0usize;
        let mut path_misses = 0usize;
        let mut path_upload_bytes = 0u64;
        let mut scene_traversal_us = 0u64;
        let mut packet_build_us = 0u64;
        let mut packet_scene_build_us = 0u64;
        let mut packet_text_us = 0u64;
        let mut packet_path_us = 0u64;
        let mut packet_rect_us = 0u64;
        let mut resource_collection_us = 0u64;
        let mut bind_group_us = 0u64;
        let mut batch_prepare_us = 0u64;
        let mut gpu_upload_us = 0u64;
        let mut encode_us = 0u64;
        let mut queue_submit_us = 0u64;
        let mut vertex_upload_bytes = 0u64;
        let mut text_vertex_bytes = 0u64;
        let mut text_glyph_instances = 0usize;
        let mut visible_layers = 0usize;
        let mut packet_builds = 0usize;
        let mut packet_new = 0usize;
        let mut packet_signature = 0usize;
        let mut packet_scene = 0usize;
        let mut packet_state = 0usize;
        let mut measure_arrange_ms = 0.0_f64;
        let mut hit_test_ms = 0.0_f64;
        let mut paint_ms = 0.0_f64;
        let mut semantics_ms = 0.0_f64;
        let mut widget_arrange_ms = 0.0_f64;
        let mut widget_paint_ms = 0.0_f64;
        let mut widget_semantics_ms = 0.0_f64;

        for frame in 0..FRAMES {
            let zoom = 0.32 + ((frame % 17) as f32 * 0.006);
            if detailed_profile {
                runtime
                    .handle_event(window_id, Event::Window(sui::WindowEvent::RedrawRequested))?;
            }
            state.set_viewport(Viewport::centered_on(center, viewport_size, zoom, 0.1, 2.0));
            let runtime_started = std::time::Instant::now();
            let output = runtime.render(window_id)?;
            runtime_time += runtime_started.elapsed();
            for phase in &output.diagnostics.phase_timings {
                match phase.phase {
                    sui::FramePhase::MeasureArrange => measure_arrange_ms += phase.duration_ms,
                    sui::FramePhase::HitTest => hit_test_ms += phase.duration_ms,
                    sui::FramePhase::Paint => paint_ms += phase.duration_ms,
                    sui::FramePhase::Semantics => semantics_ms += phase.duration_ms,
                    _ => {}
                }
            }
            for timing in &output.diagnostics.widget_timings {
                match timing.phase.label() {
                    "Arrange" => widget_arrange_ms += timing.duration_ms,
                    "Paint" => widget_paint_ms += timing.duration_ms,
                    "Semantics" => {
                        widget_semantics_ms += timing.duration_ms;
                    }
                    _ => {}
                }
            }
            let renderer_started = std::time::Instant::now();
            renderer.render(&output.frame)?;
            renderer_time += renderer_started.elapsed();
            let stats = renderer
                .last_frame_stats(window_id)
                .expect("renderer frame stats");
            draw_count += stats.draw_count;
            path_misses += stats.analytic_path_bind_group_miss_count;
            path_upload_bytes += stats.analytic_path_bind_group_upload_bytes;
            scene_traversal_us += stats.retained_scene_traversal_time_us;
            packet_build_us += stats.retained_packet_build_time_us;
            packet_scene_build_us += stats.retained_packet_scene_build_time_us;
            packet_text_us += stats.retained_packet_text_command_time_us;
            packet_path_us += stats.retained_packet_path_command_time_us;
            packet_rect_us += stats.retained_packet_rect_command_time_us;
            resource_collection_us += stats.resource_collection_time_us;
            bind_group_us += stats.bind_group_prepare_time_us;
            batch_prepare_us += stats.batch_prepare_time_us;
            gpu_upload_us += stats.gpu_upload_time_us;
            encode_us += stats.pass_encode_time_us;
            queue_submit_us += stats.queue_submit_time_us;
            vertex_upload_bytes += stats.uploaded_vertex_bytes;
            text_vertex_bytes += stats.text_vertex_bytes;
            text_glyph_instances += stats.text_glyph_instance_count;
            visible_layers += stats.visible_layer_count;
            packet_builds += stats.retained_packet_build_count;
            packet_new += stats.retained_packet_rebuilds.new_count;
            packet_signature += stats.retained_packet_rebuilds.signature_count;
            packet_scene += stats.retained_packet_rebuilds.scene_count;
            packet_state += stats.retained_packet_rebuilds.state_count;
        }

        println!(
            "NODE_GRAPH_GPU_ZOOM_BENCHMARK nodes=384 edges=368 frames={FRAMES} detailed={detailed_profile} runtime_avg_ms={:.3} renderer_avg_ms={:.3} total_avg_ms={:.3} draw_avg={:.1} path_miss_avg={:.1} path_upload_avg_bytes={:.1} scene_traversal_avg_us={:.1} packet_build_avg_us={:.1} packet_scene_build_avg_us={:.1} packet_text_avg_us={:.1} packet_path_avg_us={:.1} packet_rect_avg_us={:.1} resource_collection_avg_us={:.1} bind_group_avg_us={:.1} batch_prepare_avg_us={:.1} gpu_upload_avg_us={:.1} encode_avg_us={:.1} queue_submit_avg_us={:.1} vertex_upload_avg_bytes={:.1} text_vertex_avg_bytes={:.1} text_glyph_avg={:.1} visible_layer_avg={:.1} packet_build_avg={:.1} packet_new={packet_new} packet_signature={packet_signature} packet_scene={packet_scene} packet_state={packet_state} measure_arrange_avg_ms={:.3} hit_test_avg_ms={:.3} paint_avg_ms={:.3} semantics_avg_ms={:.3} widget_arrange_avg_ms={:.3} widget_paint_avg_ms={:.3} widget_semantics_avg_ms={:.3}",
            runtime_time.as_secs_f64() * 1000.0 / FRAMES as f64,
            renderer_time.as_secs_f64() * 1000.0 / FRAMES as f64,
            (runtime_time + renderer_time).as_secs_f64() * 1000.0 / FRAMES as f64,
            draw_count as f64 / FRAMES as f64,
            path_misses as f64 / FRAMES as f64,
            path_upload_bytes as f64 / FRAMES as f64,
            scene_traversal_us as f64 / FRAMES as f64,
            packet_build_us as f64 / FRAMES as f64,
            packet_scene_build_us as f64 / FRAMES as f64,
            packet_text_us as f64 / FRAMES as f64,
            packet_path_us as f64 / FRAMES as f64,
            packet_rect_us as f64 / FRAMES as f64,
            resource_collection_us as f64 / FRAMES as f64,
            bind_group_us as f64 / FRAMES as f64,
            batch_prepare_us as f64 / FRAMES as f64,
            gpu_upload_us as f64 / FRAMES as f64,
            encode_us as f64 / FRAMES as f64,
            queue_submit_us as f64 / FRAMES as f64,
            vertex_upload_bytes as f64 / FRAMES as f64,
            text_vertex_bytes as f64 / FRAMES as f64,
            text_glyph_instances as f64 / FRAMES as f64,
            visible_layers as f64 / FRAMES as f64,
            packet_builds as f64 / FRAMES as f64,
            measure_arrange_ms / FRAMES as f64,
            hit_test_ms / FRAMES as f64,
            paint_ms / FRAMES as f64,
            semantics_ms / FRAMES as f64,
            widget_arrange_ms / FRAMES as f64,
            widget_paint_ms / FRAMES as f64,
            widget_semantics_ms / FRAMES as f64,
        );
        Ok(())
    }

    fn run_edge_world_benchmark(retained: bool) -> (f64, f64, f64, f64) {
        const EDGE_COUNT: usize = 1_024;
        const FRAMES: usize = 60;
        let mut nodes = Vec::with_capacity(EDGE_COUNT + 1);
        let mut edges = Vec::with_capacity(EDGE_COUNT);
        for index in 0..=EDGE_COUNT {
            let mut node = Node::new(
                format!("edge-world-node-{index}"),
                Point::new((index % 64) as f32 * 32.0, (index / 64) as f32 * 32.0),
                DemoNodeData::new("edge world"),
            )
            .size(Size::new(24.0, 20.0));
            node.handles.clear();
            nodes.push(node);
            if index > 0 {
                edges.push(Edge::new(
                    format!("edge-world-{index}"),
                    format!("edge-world-node-{}", index - 1),
                    format!("edge-world-node-{index}"),
                    DemoEdgeData::new("retained edge world"),
                ));
            }
        }
        let state = NodeGraphState::new(nodes, edges).expect("valid edge-world benchmark graph");
        let graph = NodeGraph::new("Edge world benchmark", state.clone())
            .config(NodeGraphConfig {
                min_zoom: 0.05,
                max_zoom: 2.0,
                retain_edge_world: retained,
                retained_edge_world_min: 0,
                ..NodeGraphConfig::default()
            })
            .node_painter(|_ctx, _node, _paint| {});
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Edge world benchmark")
                    .root(graph),
            )
            .build()
            .expect("edge-world runtime");
        let window_id = runtime.window_ids()[0];
        let initial = runtime.render(window_id).expect("initial edge-world frame");
        let mut renderer = WgpuRenderer::new();
        renderer
            .render(&initial.frame)
            .expect("initial edge-world GPU frame");
        let viewport_size = state.viewport_size();
        let center = Point::new(1_008.0, 256.0);
        let mut runtime_time = std::time::Duration::ZERO;
        let mut renderer_time = std::time::Duration::ZERO;
        let mut packet_builds = 0usize;
        let mut path_time_us = 0u64;
        for frame in 0..FRAMES {
            let zoom = 0.42 + ((frame % 13) as f32 * 0.008);
            state.set_viewport(Viewport::centered_on(
                center,
                viewport_size,
                zoom,
                0.05,
                2.0,
            ));
            let runtime_started = std::time::Instant::now();
            let output = runtime.render(window_id).expect("edge-world runtime frame");
            runtime_time += runtime_started.elapsed();
            let renderer_started = std::time::Instant::now();
            renderer
                .render(&output.frame)
                .expect("edge-world GPU frame");
            renderer_time += renderer_started.elapsed();
            let stats = renderer
                .last_frame_stats(window_id)
                .expect("edge-world renderer stats");
            packet_builds += stats.retained_packet_build_count;
            path_time_us += stats.retained_packet_path_command_time_us;
        }
        (
            runtime_time.as_secs_f64() * 1000.0 / FRAMES as f64,
            renderer_time.as_secs_f64() * 1000.0 / FRAMES as f64,
            packet_builds as f64 / FRAMES as f64,
            path_time_us as f64 / FRAMES as f64,
        )
    }

    #[test]
    #[ignore = "diagnostic benchmark for retained flow-space edge layers"]
    fn retained_edge_world_gpu_zoom_benchmark() {
        let direct = run_edge_world_benchmark(false);
        let retained = run_edge_world_benchmark(true);
        println!(
            "EDGE_WORLD_BENCHMARK edges=1024 frames=60 direct_runtime_ms={:.3} direct_renderer_ms={:.3} direct_packet_build_avg={:.2} direct_path_us={:.1} retained_runtime_ms={:.3} retained_renderer_ms={:.3} retained_packet_build_avg={:.2} retained_path_us={:.1} total_ratio={:.3}",
            direct.0,
            direct.1,
            direct.2,
            direct.3,
            retained.0,
            retained.1,
            retained.2,
            retained.3,
            (retained.0 + retained.1) / (direct.0 + direct.1).max(0.001),
        );
    }
}
