use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use sui_core::{
    Color, Event, InvalidationKind, KeyState, Path, Point, PointerButton, PointerEvent,
    PointerEventKind, PointerKind, Rect, ScrollDelta, SemanticsAction, SemanticsActionRequest,
    SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector, WakeEvent, WidgetId,
};
use sui_layout::Constraints;
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx,
    SemanticsCtx, StackSurfaceOptions, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{Border, Brush, Scene, SceneCommand, StrokeStyle};
use sui_text::TextStyle;
use sui_widgets::{
    CanvasGridStyle, CanvasSurface, CanvasZoomBehavior, CanvasZoomContext, DefaultTheme,
};

use crate::node_widget::RetainedNodeWidgets;
use crate::{
    Connection, Edge, EdgeId, EdgeKind, EdgeMarker, EdgePathOptions, FitViewOptions, GraphModel,
    GraphSnapshot, Handle, HandleId, HandleKind, HandlePosition, Node, NodeGraphState, NodeId,
    NodeSizeMode, NodeWidgetRegistry, Viewport,
};

const EDGE_HIT_RADIUS: f32 = 8.0;
const EDGE_HIT_STEPS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundVariant {
    Dots,
    Cross,
    #[default]
    Lines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Full,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChange {
    Selected {
        id: NodeId,
        selected: bool,
    },
    Position {
        id: NodeId,
        position: Point,
        dragging: bool,
    },
    Dimensions {
        id: NodeId,
        position: Point,
        size: Size,
        resizing: bool,
    },
    Removed {
        id: NodeId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeChange {
    Selected { id: EdgeId, selected: bool },
    Added { id: EdgeId },
    Reconnected { id: EdgeId, connection: Connection },
    Removed { id: EdgeId },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeGraphEvent {
    NodesChanged(Vec<NodeChange>),
    EdgesChanged(Vec<EdgeChange>),
    ViewportChanged(Viewport),
    Connect(Connection),
    ConnectionStarted {
        source: NodeId,
        handle: HandleId,
    },
    ConnectionEnded {
        connection: Option<Connection>,
    },
    NodeClicked {
        id: NodeId,
        position: Point,
    },
    EdgeClicked {
        id: EdgeId,
        position: Point,
    },
    PaneClicked {
        position: Point,
    },
    NodeDoubleClicked {
        id: NodeId,
        position: Point,
    },
    PaneDoubleClicked {
        position: Point,
    },
    NodeEntered(NodeId),
    NodeLeft(NodeId),
    EdgeEntered(EdgeId),
    EdgeLeft(EdgeId),
    NodeDragStarted(Vec<NodeId>),
    NodeDragStopped(Vec<NodeId>),
    NodeResizeStarted {
        id: NodeId,
        direction: ResizeDirection,
    },
    NodeResizeStopped {
        id: NodeId,
        position: Point,
        size: Size,
    },
    EdgeReconnectStarted {
        id: EdgeId,
        endpoint: HandleKind,
    },
    EdgeReconnectEnded {
        id: EdgeId,
        connection: Option<Connection>,
    },
    SelectionStarted,
    SelectionEnded,
    ViewportChangeStarted(Viewport),
    ViewportChangeEnded(Viewport),
    SelectionChanged {
        nodes: Vec<NodeId>,
        edges: Vec<EdgeId>,
    },
}

/// Renderer-independent hit result for applications that attach their own
/// event model to [`NodeGraphSurface`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeGraphHit {
    Handle {
        node: NodeId,
        handle: HandleId,
        kind: HandleKind,
    },
    Node(NodeId),
    Edge(EdgeId),
    Pane,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodePaintContext {
    pub bounds: Rect,
    pub viewport: Viewport,
    pub hovered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgePaintContext {
    pub path: Path,
    pub source: Point,
    pub target: Point,
    pub midpoint: Point,
    pub viewport: Viewport,
    pub hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeGraphConfig {
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub zoom_speed: f32,
    pub zoom_on_scroll: bool,
    pub zoom_on_pinch: bool,
    pub pan_on_scroll: bool,
    pub pan_on_scroll_speed: f32,
    pub zoom_on_double_click: bool,
    pub double_click_interval: f64,
    pub pan_on_drag: bool,
    pub selection_on_drag: bool,
    pub nodes_draggable: bool,
    pub nodes_connectable: bool,
    pub edges_reconnectable: bool,
    pub nodes_resizable: bool,
    pub resize_keep_aspect_ratio: bool,
    pub auto_pan_on_node_drag: bool,
    pub auto_pan_on_connect: bool,
    pub auto_pan_on_selection: bool,
    pub auto_pan_margin: f32,
    pub auto_pan_speed: f32,
    pub connection_line_kind: EdgeKind,
    pub elements_selectable: bool,
    pub delete_key_enabled: bool,
    pub snap_to_grid: Option<Size>,
    pub fit_view_on_init: bool,
    pub fit_view: FitViewOptions,
    pub grid_spacing: f32,
    pub background_variant: BackgroundVariant,
    pub selection_mode: SelectionMode,
    /// Skip retained widget layout, painting, and semantics outside the viewport.
    pub cull_offscreen: bool,
    /// Screen-space overscan retained around the viewport when culling.
    pub culling_margin: f32,
    /// Cache eligible edges in one flow-space retained layer.
    pub retain_edge_world: bool,
    /// Minimum edge count at which the retained layer amortizes its boundary.
    pub retained_edge_world_min: usize,
    /// Maximum edge count retained in one layer before falling back to culling.
    pub retained_edge_world_max: usize,
}

impl Default for NodeGraphConfig {
    fn default() -> Self {
        Self {
            min_zoom: 0.1,
            max_zoom: 4.0,
            zoom_speed: 0.002,
            zoom_on_scroll: true,
            zoom_on_pinch: true,
            pan_on_scroll: false,
            pan_on_scroll_speed: 0.5,
            zoom_on_double_click: true,
            double_click_interval: 0.32,
            pan_on_drag: true,
            selection_on_drag: false,
            nodes_draggable: true,
            nodes_connectable: true,
            edges_reconnectable: true,
            nodes_resizable: true,
            resize_keep_aspect_ratio: false,
            auto_pan_on_node_drag: true,
            auto_pan_on_connect: true,
            auto_pan_on_selection: true,
            auto_pan_margin: 32.0,
            auto_pan_speed: 12.0,
            connection_line_kind: EdgeKind::Bezier,
            elements_selectable: true,
            delete_key_enabled: true,
            snap_to_grid: None,
            fit_view_on_init: false,
            fit_view: FitViewOptions::default(),
            grid_spacing: 24.0,
            background_variant: BackgroundVariant::Lines,
            selection_mode: SelectionMode::Full,
            cull_offscreen: true,
            culling_margin: 64.0,
            retain_edge_world: true,
            retained_edge_world_min: 1_024,
            retained_edge_world_max: 4_096,
        }
    }
}

impl NodeGraphConfig {
    fn normalized(self) -> Self {
        let min_zoom = self.min_zoom.max(0.001);
        Self {
            min_zoom,
            max_zoom: self.max_zoom.max(min_zoom),
            zoom_speed: self.zoom_speed.max(0.0001),
            pan_on_scroll_speed: self.pan_on_scroll_speed.max(0.01),
            double_click_interval: self.double_click_interval.max(0.05),
            auto_pan_margin: self.auto_pan_margin.max(4.0),
            auto_pan_speed: self.auto_pan_speed.max(0.1),
            grid_spacing: self.grid_spacing.max(1.0),
            culling_margin: self.culling_margin.max(0.0),
            retained_edge_world_max: self
                .retained_edge_world_max
                .max(self.retained_edge_world_min),
            snap_to_grid: self
                .snap_to_grid
                .map(|size| Size::new(size.width.max(1.0), size.height.max(1.0))),
            ..self
        }
    }
}

/// Widget-owned colors for node graph presentation.
///
/// Every unset field resolves from SUI's semantic theme at paint time, so
/// applications can switch themes without rebuilding the graph.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NodeGraphAppearance {
    pub background: Option<Color>,
    pub grid: Option<Color>,
    pub node: Option<Color>,
    pub node_hovered: Option<Color>,
    pub node_border: Option<Color>,
    pub node_text: Option<Color>,
    pub selection: Option<Color>,
    pub edge: Option<Color>,
    pub edge_selected: Option<Color>,
    pub source_handle: Option<Color>,
    pub target_handle: Option<Color>,
    pub marquee_fill: Option<Color>,
    pub marquee_border: Option<Color>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedAppearance {
    background: Color,
    grid: Color,
    node: Color,
    node_hovered: Color,
    node_border: Color,
    node_text: Color,
    selection: Color,
    edge: Color,
    edge_selected: Color,
    source_handle: Color,
    target_handle: Color,
    marquee_fill: Color,
    marquee_border: Color,
}

impl NodeGraphAppearance {
    fn resolve(self, theme: &DefaultTheme) -> ResolvedAppearance {
        let selection = self.selection.unwrap_or(theme.palette.accent);
        ResolvedAppearance {
            background: self.background.unwrap_or(theme.palette.surface),
            grid: self.grid.unwrap_or(theme.palette.border.with_alpha(0.28)),
            node: self.node.unwrap_or(theme.palette.surface_raised),
            node_hovered: self.node_hovered.unwrap_or(theme.palette.surface_hover),
            node_border: self.node_border.unwrap_or(theme.palette.border),
            node_text: self.node_text.unwrap_or(theme.palette.text),
            selection,
            edge: self
                .edge
                .unwrap_or(theme.palette.text_muted.with_alpha(0.74)),
            edge_selected: self.edge_selected.unwrap_or(selection),
            source_handle: self.source_handle.unwrap_or(theme.palette.accent),
            target_handle: self.target_handle.unwrap_or(theme.palette.text_muted),
            marquee_fill: self.marquee_fill.unwrap_or(selection.with_alpha(0.13)),
            marquee_border: self.marquee_border.unwrap_or(selection.with_alpha(0.84)),
        }
    }
}

#[derive(Debug, Clone)]
enum Interaction {
    Pan {
        pointer_id: u64,
        last_position: Point,
    },
    DragNodes {
        pointer_id: u64,
        start: Point,
        origins: Vec<(NodeId, Point)>,
    },
    ResizeNode {
        pointer_id: u64,
        node: NodeId,
        direction: ResizeDirection,
        start: Point,
        original_bounds: Rect,
        origin: Point,
    },
    Marquee {
        pointer_id: u64,
        start: Point,
        current: Point,
        additive: bool,
    },
    Connect {
        pointer_id: u64,
        source: NodeId,
        source_handle: HandleId,
        source_position: Point,
        source_side: HandlePosition,
        current: Point,
    },
    ReconnectEdge {
        pointer_id: u64,
        edge: EdgeId,
        endpoint: HandleKind,
        fixed_position: Point,
        fixed_side: HandlePosition,
        current: Point,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FocusedElement {
    Node(NodeId),
    Edge(EdgeId),
}

#[derive(Debug, Clone, Copy)]
struct PinchGesture {
    pointers: [u64; 2],
    last_center: Point,
    last_distance: f32,
}

struct GraphWorldLayerMarker;

impl Widget for GraphWorldLayerMarker {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::ZERO)
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            ..LayerOptions::default()
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        Some(StackSurfaceOptions {
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EdgeWorldCacheKey {
    nodes_revision: u64,
    edges_revision: u64,
    color: Color,
    hovered: Option<EdgeId>,
    focused: Option<EdgeId>,
}

#[derive(Default)]
struct EdgeWorldCache {
    key: Option<EdgeWorldCacheKey>,
    scene: Arc<Scene>,
    bounds: Rect,
    retained_edges: Arc<HashSet<EdgeId>>,
}

impl Interaction {
    fn pointer_id(&self) -> u64 {
        match self {
            Self::Pan { pointer_id, .. }
            | Self::DragNodes { pointer_id, .. }
            | Self::ResizeNode { pointer_id, .. }
            | Self::Marquee { pointer_id, .. }
            | Self::Connect { pointer_id, .. }
            | Self::ReconnectEdge { pointer_id, .. } => *pointer_id,
        }
    }
}

type ChangeCallback = Box<dyn FnMut(NodeGraphEvent)>;
type ConnectionValidator<N, E> = Box<dyn Fn(&Connection, &GraphModel<N, E>) -> bool>;
type EdgeFactory<E> = Box<dyn FnMut(Connection) -> Edge<E>>;
type NodePaintFn<N> = dyn Fn(&mut PaintCtx, &Node<N>, NodePaintContext);
type EdgePaintFn<E> = dyn Fn(&mut PaintCtx, &Edge<E>, EdgePaintContext);
type NodePainter<N> = Box<NodePaintFn<N>>;
type EdgePainter<E> = Box<EdgePaintFn<E>>;

pub struct NodeGraph<N = (), E = ()> {
    name: String,
    state: NodeGraphState<N, E>,
    last_measured_nodes_revision: u64,
    theme: DefaultTheme,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    appearance: NodeGraphAppearance,
    config: NodeGraphConfig,
    desired_size: Size,
    interaction: Option<Interaction>,
    hovered_node: Option<NodeId>,
    hovered_edge: Option<EdgeId>,
    hovered_handle: Option<(NodeId, HandleId, HandleKind)>,
    focused_element: Option<FocusedElement>,
    edge_animation_time: f32,
    viewport_transition_running: bool,
    last_primary_down: Option<(f64, Point)>,
    active_touches: BTreeMap<u64, Point>,
    pinch_gesture: Option<PinchGesture>,
    built_in_events: bool,
    arranged_once: bool,
    next_edge_id: u64,
    on_change: Option<ChangeCallback>,
    connection_validator: Option<ConnectionValidator<N, E>>,
    edge_factory: Option<EdgeFactory<E>>,
    node_painter: Option<NodePainter<N>>,
    edge_painter: Option<EdgePainter<E>>,
    node_widget_registry: NodeWidgetRegistry<N>,
    node_widgets: RetainedNodeWidgets<N>,
    world_layer: WidgetPod,
    world_layer_active: bool,
    edge_world_cache: RefCell<EdgeWorldCache>,
    active_node_widgets: HashSet<NodeId>,
    visible_node_indices: Vec<usize>,
    visible_edge_indices: Vec<usize>,
}

impl<N, E> NodeGraph<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + Default + 'static,
{
    pub fn new(name: impl Into<String>, state: NodeGraphState<N, E>) -> Self {
        let name = name.into();
        Self {
            name,
            state,
            last_measured_nodes_revision: u64::MAX,
            theme: DefaultTheme::default(),
            theme_reader: None,
            appearance: NodeGraphAppearance::default(),
            config: NodeGraphConfig::default(),
            desired_size: Size::new(720.0, 480.0),
            interaction: None,
            hovered_node: None,
            hovered_edge: None,
            hovered_handle: None,
            focused_element: None,
            edge_animation_time: 0.0,
            viewport_transition_running: false,
            last_primary_down: None,
            active_touches: BTreeMap::new(),
            pinch_gesture: None,
            built_in_events: true,
            arranged_once: false,
            next_edge_id: 1,
            on_change: None,
            connection_validator: None,
            edge_factory: None,
            node_painter: None,
            edge_painter: None,
            node_widget_registry: NodeWidgetRegistry::new(),
            node_widgets: RetainedNodeWidgets::default(),
            world_layer: WidgetPod::new(GraphWorldLayerMarker),
            world_layer_active: false,
            edge_world_cache: RefCell::new(EdgeWorldCache::default()),
            active_node_widgets: HashSet::new(),
            visible_node_indices: Vec::new(),
            visible_edge_indices: Vec::new(),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn appearance(mut self, appearance: NodeGraphAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn config(mut self, config: NodeGraphConfig) -> Self {
        self.config = config.normalized();
        self
    }

    pub fn desired_size(mut self, size: Size) -> Self {
        self.desired_size = Size::new(size.width.max(1.0), size.height.max(1.0));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(NodeGraphEvent) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn is_valid_connection<F>(mut self, validator: F) -> Self
    where
        F: Fn(&Connection, &GraphModel<N, E>) -> bool + 'static,
    {
        self.connection_validator = Some(Box::new(validator));
        self
    }

    pub fn edge_factory<F>(mut self, factory: F) -> Self
    where
        F: FnMut(Connection) -> Edge<E> + 'static,
    {
        self.edge_factory = Some(Box::new(factory));
        self
    }

    /// Override node body painting while retaining graph hit testing,
    /// selection, dragging, and handle rendering.
    pub fn node_painter<F>(mut self, painter: F) -> Self
    where
        F: Fn(&mut PaintCtx, &Node<N>, NodePaintContext) + 'static,
    {
        self.node_painter = Some(Box::new(painter));
        self
    }

    /// Override complete edge painting while retaining edge hit testing and
    /// selection behavior.
    pub fn edge_painter<F>(mut self, painter: F) -> Self
    where
        F: Fn(&mut PaintCtx, &Edge<E>, EdgePaintContext) + 'static,
    {
        self.edge_painter = Some(Box::new(painter));
        self
    }

    /// Register a retained custom widget for nodes whose [`Node::kind`]
    /// matches `kind`.
    ///
    /// Existing nodes keep their [`sui_runtime::WidgetPod`] identity while
    /// their observable [`crate::NodeSignal`] is updated.
    pub fn node_type<F, W>(mut self, kind: impl Into<String>, factory: F) -> Self
    where
        F: FnMut(&NodeId, crate::NodeSignal<N>) -> W + 'static,
        W: Widget + 'static,
    {
        self.node_widget_registry.register(kind, factory);
        self
    }

    pub fn node_types(mut self, registry: NodeWidgetRegistry<N>) -> Self {
        self.node_widget_registry = registry;
        self
    }

    /// Override how retained widgets of `kind` respond to Canvas zoom.
    /// Uniform subtree scaling is the default.
    pub fn node_zoom_behavior(
        mut self,
        kind: impl Into<String>,
        behavior: CanvasZoomBehavior,
    ) -> Self {
        self.node_widget_registry.set_zoom_behavior(kind, behavior);
        self
    }

    pub fn state_handle(&self) -> NodeGraphState<N, E> {
        self.state.clone()
    }

    pub fn hit_test(&self, bounds: Rect, position: Point) -> NodeGraphHit {
        node_graph_hit_test(&self.state.snapshot(), bounds, position)
    }

    /// Convert this interactive graph into a paint-only surface. Retained
    /// child widgets continue receiving their own events.
    pub fn into_surface(mut self) -> NodeGraphSurface<N, E> {
        self.built_in_events = false;
        NodeGraphSurface { graph: self }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.theme)
    }

    fn emit(&mut self, event: NodeGraphEvent) {
        if let Some(on_change) = &mut self.on_change {
            on_change(event);
        }
    }

    fn emit_selection(&mut self) {
        let snapshot = self.state.snapshot();
        self.emit(NodeGraphEvent::SelectionChanged {
            nodes: snapshot.graph.selected_node_ids(),
            edges: snapshot.graph.selected_edge_ids(),
        });
    }

    fn cancel_interaction_for_pinch(&mut self) {
        let Some(interaction) = self.interaction.take() else {
            return;
        };
        match interaction {
            Interaction::Pan { .. } => {
                self.emit(NodeGraphEvent::ViewportChangeEnded(self.state.viewport()));
            }
            Interaction::DragNodes { origins, .. } => {
                let snapshot = self.state.snapshot();
                let dragged_ids = origins.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
                let changes = origins
                    .iter()
                    .filter_map(|(id, _)| {
                        snapshot.graph.node(id).map(|node| NodeChange::Position {
                            id: id.clone(),
                            position: node.position,
                            dragging: false,
                        })
                    })
                    .collect::<Vec<_>>();
                if !changes.is_empty() {
                    self.emit(NodeGraphEvent::NodesChanged(changes));
                }
                self.emit(NodeGraphEvent::NodeDragStopped(dragged_ids));
            }
            Interaction::ResizeNode { node, .. } => {
                let snapshot = self.state.snapshot();
                if let Some(node) = snapshot.graph.node(&node) {
                    let position = node.position;
                    let size = node.size;
                    let id = node.id.clone();
                    self.emit(NodeGraphEvent::NodesChanged(vec![NodeChange::Dimensions {
                        id: id.clone(),
                        position,
                        size,
                        resizing: false,
                    }]));
                    self.emit(NodeGraphEvent::NodeResizeStopped { id, position, size });
                }
            }
            Interaction::Marquee { .. } => self.emit(NodeGraphEvent::SelectionEnded),
            Interaction::Connect { .. } => {
                self.emit(NodeGraphEvent::ConnectionEnded { connection: None });
            }
            Interaction::ReconnectEdge { edge, .. } => {
                self.emit(NodeGraphEvent::EdgeReconnectEnded {
                    id: edge,
                    connection: None,
                });
            }
        }
        self.hovered_handle = None;
    }

    fn handle_touch_gesture(
        &mut self,
        ctx: &mut EventCtx,
        pointer: &PointerEvent,
        config: NodeGraphConfig,
    ) -> bool {
        match pointer.kind {
            PointerEventKind::Down if ctx.bounds().contains(pointer.position) => {
                self.active_touches
                    .insert(pointer.pointer_id, pointer.position);
                if self.pinch_gesture.is_some() {
                    ctx.request_pointer_capture(pointer.pointer_id);
                    return true;
                }
                if self.active_touches.len() < 2 {
                    return false;
                }
                let mut touches = self.active_touches.iter();
                let Some((&first_id, &first)) = touches.next() else {
                    return false;
                };
                let Some((&second_id, &second)) = touches.next() else {
                    return false;
                };
                let center = midpoint(first, second);
                self.cancel_interaction_for_pinch();
                self.last_primary_down = None;
                self.pinch_gesture = Some(PinchGesture {
                    pointers: [first_id, second_id],
                    last_center: center,
                    last_distance: vector_length(second - first).max(1.0),
                });
                ctx.request_pointer_capture(first_id);
                ctx.request_pointer_capture(second_id);
                self.emit(NodeGraphEvent::ViewportChangeStarted(self.state.viewport()));
                self.request_update(ctx);
                true
            }
            PointerEventKind::Move => {
                let Some(position) = self.active_touches.get_mut(&pointer.pointer_id) else {
                    return false;
                };
                *position = pointer.position;
                let Some(gesture) = self.pinch_gesture else {
                    return false;
                };
                let Some(&first) = self.active_touches.get(&gesture.pointers[0]) else {
                    return false;
                };
                let Some(&second) = self.active_touches.get(&gesture.pointers[1]) else {
                    return false;
                };
                let center = midpoint(first, second);
                let distance = vector_length(second - first).max(1.0);
                let mut viewport = self.state.viewport();
                viewport.pan_by(center - gesture.last_center);
                viewport.zoom_at(
                    ctx.bounds(),
                    center,
                    distance / gesture.last_distance,
                    config.min_zoom,
                    config.max_zoom,
                );
                if let Some(gesture) = &mut self.pinch_gesture {
                    gesture.last_center = center;
                    gesture.last_distance = distance;
                }
                if self.state.set_viewport(viewport) {
                    self.emit(NodeGraphEvent::ViewportChanged(viewport));
                }
                self.request_update(ctx);
                true
            }
            PointerEventKind::Up | PointerEventKind::Cancel => {
                if self.active_touches.remove(&pointer.pointer_id).is_none() {
                    return false;
                }
                let Some(gesture) = self.pinch_gesture else {
                    return false;
                };
                ctx.release_pointer_capture(pointer.pointer_id);
                if !gesture.pointers.contains(&pointer.pointer_id) {
                    return true;
                }
                self.pinch_gesture = None;
                self.emit(NodeGraphEvent::ViewportChangeEnded(self.state.viewport()));
                if pointer.kind == PointerEventKind::Cancel {
                    for pointer_id in self.active_touches.keys().copied().collect::<Vec<_>>() {
                        ctx.release_pointer_capture(pointer_id);
                    }
                    self.active_touches.clear();
                } else if config.pan_on_drag {
                    if let Some((&pointer_id, &position)) = self.active_touches.iter().next() {
                        for extra in self
                            .active_touches
                            .keys()
                            .copied()
                            .filter(|id| *id != pointer_id)
                            .collect::<Vec<_>>()
                        {
                            ctx.release_pointer_capture(extra);
                        }
                        self.active_touches.retain(|id, _| *id == pointer_id);
                        self.interaction = Some(Interaction::Pan {
                            pointer_id,
                            last_position: position,
                        });
                        ctx.request_pointer_capture(pointer_id);
                        self.emit(NodeGraphEvent::ViewportChangeStarted(self.state.viewport()));
                    }
                } else {
                    for pointer_id in self.active_touches.keys().copied().collect::<Vec<_>>() {
                        ctx.release_pointer_capture(pointer_id);
                    }
                    self.active_touches.clear();
                }
                self.request_update(ctx);
                true
            }
            _ => self.pinch_gesture.is_some(),
        }
    }

    fn request_update(&self, ctx: &mut EventCtx) {
        if !self.node_widgets.is_empty() {
            ctx.request_transform();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn next_default_edge(&mut self, connection: Connection) -> Edge<E> {
        loop {
            let id = EdgeId::new(format!("edge-{}", self.next_edge_id));
            self.next_edge_id = self.next_edge_id.saturating_add(1);
            if self.state.snapshot().graph.edge(&id).is_none() {
                let mut edge = Edge::new(
                    id,
                    connection.source.clone(),
                    connection.target.clone(),
                    E::default(),
                );
                edge.source_handle = connection.source_handle.clone();
                edge.target_handle = connection.target_handle.clone();
                return edge;
            }
        }
    }

    fn paint_retained_edge_world(
        &self,
        ctx: &mut PaintCtx,
        snapshot: &GraphSnapshot<N, E>,
        bounds: Rect,
        color: Color,
        focused_edge: Option<&EdgeId>,
    ) -> Option<Arc<HashSet<EdgeId>>> {
        if !self.world_layer_active {
            return None;
        }

        let key = EdgeWorldCacheKey {
            nodes_revision: snapshot.revisions.nodes,
            edges_revision: snapshot.revisions.edges,
            color,
            hovered: self.hovered_edge.clone(),
            focused: focused_edge.cloned(),
        };
        let mut cache = self.edge_world_cache.borrow_mut();
        if cache.key.as_ref() != Some(&key) {
            let mut scene = Scene::new();
            let mut retained_edges = HashSet::new();
            let node_lookup = snapshot
                .graph
                .nodes
                .iter()
                .filter_map(|node| {
                    snapshot
                        .spatial
                        .node_bounds(&node.id)
                        .map(|bounds| (&node.id, (node, bounds)))
                })
                .collect::<HashMap<_, _>>();
            for edge in &snapshot.graph.edges {
                if !edge_is_retained_world_candidate(edge, self.hovered_edge.as_ref(), focused_edge)
                {
                    continue;
                }
                let Some(geometry) = edge_geometry_from_lookup(&node_lookup, edge) else {
                    continue;
                };
                scene.push(SceneCommand::StrokePath {
                    path: geometry.path.clone(),
                    brush: Brush::Solid(color),
                    stroke: StrokeStyle::new(1.5),
                });
                if let Some(marker) = edge.marker_start {
                    append_marker_scene(
                        &mut scene,
                        geometry.source,
                        scale_vector(geometry.source_tangent, -1.0),
                        marker,
                        color,
                    );
                }
                if let Some(marker) = edge.marker_end {
                    append_marker_scene(
                        &mut scene,
                        geometry.target,
                        geometry.target_tangent,
                        marker,
                        color,
                    );
                }
                retained_edges.insert(edge.id.clone());
            }
            cache.bounds = snapshot
                .graph
                .bounds()
                .unwrap_or(Rect::ZERO)
                .inflate(16.0, 16.0);
            cache.scene = Arc::new(scene);
            cache.retained_edges = Arc::new(retained_edges);
            cache.key = Some(key);
        }

        let retained = Arc::clone(&cache.retained_edges);
        if !cache.scene.commands().is_empty() {
            let scene = cache.scene.clone();
            let layer_bounds = cache.bounds;
            let transform = snapshot
                .viewport
                .to_canvas(bounds.size)
                .transform(bounds, Point::ZERO);
            let owner = self.world_layer.id();
            drop(cache);
            ctx.with_transform(transform, |ctx| {
                ctx.push_retained_scene_layer(owner, layer_bounds, scene);
            });
        }
        Some(retained)
    }
}

/// Paint-only node graph UI with no built-in graph interaction model.
///
/// The surface observes [`NodeGraphState`] and renders the same graph, canvas
/// content, retained child widgets, semantics, and viewport as [`NodeGraph`].
/// It does not interpret pointer, keyboard, or semantic actions. Retained child
/// widgets remain normal independent SUI widgets, and an application can wrap
/// the surface with its own event controller that mutates the shared state.
pub struct NodeGraphSurface<N = (), E = ()> {
    graph: NodeGraph<N, E>,
}

impl<N, E> NodeGraphSurface<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + Default + 'static,
{
    pub fn new(name: impl Into<String>, state: NodeGraphState<N, E>) -> Self {
        NodeGraph::new(name, state).into_surface()
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.graph = self.graph.theme(theme);
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.graph = self.graph.theme_when(theme);
        self
    }

    pub fn appearance(mut self, appearance: NodeGraphAppearance) -> Self {
        self.graph = self.graph.appearance(appearance);
        self
    }

    pub fn config(mut self, config: NodeGraphConfig) -> Self {
        self.graph = self.graph.config(config);
        self
    }

    pub fn desired_size(mut self, size: Size) -> Self {
        self.graph = self.graph.desired_size(size);
        self
    }

    pub fn node_painter<F>(mut self, painter: F) -> Self
    where
        F: Fn(&mut PaintCtx, &Node<N>, NodePaintContext) + 'static,
    {
        self.graph = self.graph.node_painter(painter);
        self
    }

    pub fn edge_painter<F>(mut self, painter: F) -> Self
    where
        F: Fn(&mut PaintCtx, &Edge<E>, EdgePaintContext) + 'static,
    {
        self.graph = self.graph.edge_painter(painter);
        self
    }

    pub fn node_type<F, W>(mut self, kind: impl Into<String>, factory: F) -> Self
    where
        F: FnMut(&NodeId, crate::NodeSignal<N>) -> W + 'static,
        W: Widget + 'static,
    {
        self.graph = self.graph.node_type(kind, factory);
        self
    }

    pub fn node_types(mut self, registry: NodeWidgetRegistry<N>) -> Self {
        self.graph = self.graph.node_types(registry);
        self
    }

    pub fn node_zoom_behavior(
        mut self,
        kind: impl Into<String>,
        behavior: CanvasZoomBehavior,
    ) -> Self {
        self.graph = self.graph.node_zoom_behavior(kind, behavior);
        self
    }

    pub fn state_handle(&self) -> NodeGraphState<N, E> {
        self.graph.state_handle()
    }

    pub fn hit_test(&self, bounds: Rect, position: Point) -> NodeGraphHit {
        self.graph.hit_test(bounds, position)
    }

    pub fn into_interactive(mut self) -> NodeGraph<N, E> {
        self.graph.built_in_events = true;
        self.graph
    }
}

impl<N, E> From<NodeGraph<N, E>> for NodeGraphSurface<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + Default + 'static,
{
    fn from(graph: NodeGraph<N, E>) -> Self {
        graph.into_surface()
    }
}

impl<N, E> Widget for NodeGraphSurface<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + Default + 'static,
{
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        <NodeGraph<N, E> as Widget>::measure(&mut self.graph, ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        <NodeGraph<N, E> as Widget>::arrange(&mut self.graph, ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        <NodeGraph<N, E> as Widget>::paint(&self.graph, ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        <NodeGraph<N, E> as Widget>::semantics(&self.graph, ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        <NodeGraph<N, E> as Widget>::visit_children(&self.graph, visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        <NodeGraph<N, E> as Widget>::visit_children_mut(&mut self.graph, visitor);
    }
}

#[derive(Debug, Clone, Copy)]
struct HandleHit<'a, N> {
    node: &'a Node<N>,
    handle: &'a Handle,
    position: Point,
}

#[derive(Debug, Clone, Copy)]
struct ResizeHit<'a, N> {
    node: &'a Node<N>,
    direction: ResizeDirection,
}

#[derive(Debug, Clone, Copy)]
struct ReconnectHit<'a, E> {
    edge: &'a Edge<E>,
    endpoint: HandleKind,
    fixed_position: Point,
    fixed_side: HandlePosition,
}

#[derive(Debug, Clone)]
struct EdgeGeometry {
    path: Path,
    source: Point,
    target: Point,
    control_1: Point,
    control_2: Point,
    midpoint: Point,
    source_tangent: Vector,
    target_tangent: Vector,
    source_side: HandlePosition,
    target_side: HandlePosition,
    kind: EdgeKind,
}

impl<N, E> Widget for NodeGraph<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + Default + 'static,
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        // Descendant controls get first refusal. If a retained node child does
        // not handle the event, normal bubbling reaches the graph here.
        if !self.built_in_events {
            return;
        }
        let snapshot = ctx.observe(&self.state.signal, InvalidationKind::Paint);
        let config = self.config.normalized();
        if ctx.phase() == EventPhase::Capture {
            if snapshot.interactive
                && config.zoom_on_pinch
                && let Event::Pointer(pointer) = event
                && pointer.pointer_kind == PointerKind::Touch
                && self.handle_touch_gesture(ctx, pointer, config)
            {
                ctx.set_handled();
            }
            return;
        }
        if snapshot.interactive
            && config.zoom_on_pinch
            && let Event::Pointer(pointer) = event
            && pointer.pointer_kind == PointerKind::Touch
            && self.handle_touch_gesture(ctx, pointer, config)
        {
            ctx.set_handled();
            return;
        }

        if !snapshot.interactive
            && matches!(
                event,
                Event::Pointer(_) | Event::Keyboard(_) | Event::RawMouseMotion(_)
            )
        {
            if let Event::Pointer(pointer) = event
                && pointer.kind == PointerEventKind::Down
                && ctx.bounds().contains(pointer.position)
            {
                ctx.request_focus();
                ctx.set_handled();
            }
            return;
        }

        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Down
            && pointer.button == Some(PointerButton::Primary)
        {
            let is_double_click = self.last_primary_down.is_some_and(|(time, position)| {
                ctx.current_time() - time <= config.double_click_interval
                    && vector_length(pointer.position - position) <= 4.0
            });
            self.last_primary_down =
                (!is_double_click).then_some((ctx.current_time(), pointer.position));
            if is_double_click && config.zoom_on_double_click {
                if let Some(node) = hit_node(&snapshot, ctx.bounds(), pointer.position) {
                    self.emit(NodeGraphEvent::NodeDoubleClicked {
                        id: node.id.clone(),
                        position: pointer.position,
                    });
                } else {
                    self.emit(NodeGraphEvent::PaneDoubleClicked {
                        position: pointer.position,
                    });
                }
                let mut viewport = snapshot.viewport;
                viewport.zoom_at(
                    ctx.bounds(),
                    pointer.position,
                    1.2,
                    config.min_zoom,
                    config.max_zoom,
                );
                self.emit(NodeGraphEvent::ViewportChangeStarted(snapshot.viewport));
                if self.state.set_viewport(viewport) {
                    self.emit(NodeGraphEvent::ViewportChanged(viewport));
                }
                self.emit(NodeGraphEvent::ViewportChangeEnded(viewport));
                self.request_update(ctx);
                ctx.set_handled();
                return;
            }
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && (config.zoom_on_scroll || config.pan_on_scroll)
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = scroll_offset(pointer.scroll_delta, pointer.delta);
                let amount = if delta.y.abs() >= delta.x.abs() {
                    delta.y
                } else {
                    delta.x
                };
                let mut next = snapshot.viewport;
                if config.pan_on_scroll && !(pointer.modifiers.control || pointer.modifiers.meta) {
                    next.pan_by(Vector::new(
                        delta.x * config.pan_on_scroll_speed,
                        delta.y * config.pan_on_scroll_speed,
                    ));
                } else {
                    next.zoom_at(
                        ctx.bounds(),
                        pointer.position,
                        (amount * config.zoom_speed).exp(),
                        config.min_zoom,
                        config.max_zoom,
                    );
                }
                if self.state.set_viewport(next) {
                    self.emit(NodeGraphEvent::ViewportChangeStarted(snapshot.viewport));
                    self.emit(NodeGraphEvent::ViewportChanged(next));
                    self.emit(NodeGraphEvent::ViewportChangeEnded(next));
                    self.request_update(ctx);
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position)
                    && matches!(
                        pointer.button,
                        Some(PointerButton::Middle | PointerButton::Secondary)
                    ) =>
            {
                self.interaction = Some(Interaction::Pan {
                    pointer_id: pointer.pointer_id,
                    last_position: pointer.position,
                });
                self.emit(NodeGraphEvent::ViewportChangeStarted(snapshot.viewport));
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                let bounds = ctx.bounds();
                let resize_hit = hit_resize_handle(&snapshot, bounds, pointer.position);
                let reconnect_hit = hit_reconnect_handle(&snapshot, bounds, pointer.position);
                let source_hit = hit_handle(
                    &snapshot,
                    bounds,
                    pointer.position,
                    Some(HandleKind::Source),
                );
                if config.nodes_resizable
                    && let Some(hit) = resize_hit
                {
                    self.interaction = Some(Interaction::ResizeNode {
                        pointer_id: pointer.pointer_id,
                        node: hit.node.id.clone(),
                        direction: hit.direction,
                        start: pointer.position,
                        original_bounds: hit.node.bounds(),
                        origin: hit.node.origin,
                    });
                    self.emit(NodeGraphEvent::NodeResizeStarted {
                        id: hit.node.id.clone(),
                        direction: hit.direction,
                    });
                } else if config.edges_reconnectable
                    && let Some(hit) = reconnect_hit
                {
                    self.interaction = Some(Interaction::ReconnectEdge {
                        pointer_id: pointer.pointer_id,
                        edge: hit.edge.id.clone(),
                        endpoint: hit.endpoint,
                        fixed_position: hit.fixed_position,
                        fixed_side: hit.fixed_side,
                        current: pointer.position,
                    });
                    self.emit(NodeGraphEvent::EdgeReconnectStarted {
                        id: hit.edge.id.clone(),
                        endpoint: hit.endpoint,
                    });
                } else if config.nodes_connectable
                    && let Some(hit) =
                        source_hit.filter(|hit| hit.node.connectable && hit.handle.connectable)
                {
                    self.interaction = Some(Interaction::Connect {
                        pointer_id: pointer.pointer_id,
                        source: hit.node.id.clone(),
                        source_handle: hit.handle.id.clone(),
                        source_position: hit.position,
                        source_side: hit.handle.position,
                        current: pointer.position,
                    });
                    self.hovered_handle =
                        Some((hit.node.id.clone(), hit.handle.id.clone(), hit.handle.kind));
                    self.emit(NodeGraphEvent::ConnectionStarted {
                        source: hit.node.id.clone(),
                        handle: hit.handle.id.clone(),
                    });
                } else if let Some(node) = hit_node(&snapshot, bounds, pointer.position) {
                    let id = node.id.clone();
                    self.emit(NodeGraphEvent::NodeClicked {
                        id: id.clone(),
                        position: pointer.position,
                    });
                    let draggable = node.draggable && config.nodes_draggable;
                    let mut changes = ElementChanges::default();
                    if config.elements_selectable && node.selectable {
                        self.state.update(|snapshot| {
                            apply_node_selection(
                                snapshot.graph_mut(),
                                &id,
                                pointer.modifiers.control || pointer.modifiers.meta,
                                &mut changes,
                            );
                        });
                    }
                    emit_element_changes(self, changes);

                    if draggable {
                        let current = self.state.snapshot();
                        let origins = current
                            .graph
                            .nodes
                            .iter()
                            .filter(|candidate| {
                                candidate.selected
                                    && candidate.draggable
                                    && !has_selected_ancestor(&current.graph, candidate)
                            })
                            .map(|candidate| (candidate.id.clone(), candidate.position))
                            .collect::<Vec<_>>();
                        if !origins.is_empty() {
                            self.emit(NodeGraphEvent::NodeDragStarted(
                                origins.iter().map(|(id, _)| id.clone()).collect(),
                            ));
                            self.interaction = Some(Interaction::DragNodes {
                                pointer_id: pointer.pointer_id,
                                start: pointer.position,
                                origins,
                            });
                        }
                    }
                } else if let Some(edge) = hit_edge(&snapshot, bounds, pointer.position) {
                    let id = edge.id.clone();
                    self.emit(NodeGraphEvent::EdgeClicked {
                        id: id.clone(),
                        position: pointer.position,
                    });
                    let mut changes = ElementChanges::default();
                    if config.elements_selectable && edge.selectable {
                        self.state.update(|snapshot| {
                            apply_edge_selection(
                                snapshot.graph_mut(),
                                &id,
                                pointer.modifiers.control || pointer.modifiers.meta,
                                &mut changes,
                            );
                        });
                    }
                    emit_element_changes(self, changes);
                } else if config.selection_on_drag || pointer.modifiers.shift {
                    self.emit(NodeGraphEvent::SelectionStarted);
                    self.interaction = Some(Interaction::Marquee {
                        pointer_id: pointer.pointer_id,
                        start: pointer.position,
                        current: pointer.position,
                        additive: pointer.modifiers.control || pointer.modifiers.meta,
                    });
                } else if config.pan_on_drag {
                    self.emit(NodeGraphEvent::PaneClicked {
                        position: pointer.position,
                    });
                    self.emit(NodeGraphEvent::ViewportChangeStarted(snapshot.viewport));
                    let mut changes = ElementChanges::default();
                    if config.elements_selectable {
                        self.state.update(|snapshot| {
                            clear_selection(snapshot.graph_mut(), &mut changes);
                        });
                    }
                    emit_element_changes(self, changes);
                    self.interaction = Some(Interaction::Pan {
                        pointer_id: pointer.pointer_id,
                        last_position: pointer.position,
                    });
                }

                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                self.request_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let bounds = ctx.bounds();
                let Some(interaction) = self.interaction.clone() else {
                    let next_handle = hit_handle(&snapshot, bounds, pointer.position, None)
                        .map(|hit| (hit.node.id.clone(), hit.handle.id.clone(), hit.handle.kind));
                    let next_node =
                        hit_node(&snapshot, bounds, pointer.position).map(|node| node.id.clone());
                    let next_edge = if next_node.is_none() {
                        hit_edge(&snapshot, bounds, pointer.position).map(|edge| edge.id.clone())
                    } else {
                        None
                    };
                    if self.hovered_handle != next_handle
                        || self.hovered_node != next_node
                        || self.hovered_edge != next_edge
                    {
                        let previous_node = self.hovered_node.clone();
                        let previous_edge = self.hovered_edge.clone();
                        self.hovered_handle = next_handle;
                        self.hovered_node = next_node.clone();
                        self.hovered_edge = next_edge.clone();
                        if previous_node != next_node {
                            if let Some(id) = previous_node {
                                self.emit(NodeGraphEvent::NodeLeft(id));
                            }
                            if let Some(id) = next_node {
                                self.emit(NodeGraphEvent::NodeEntered(id));
                            }
                        }
                        if previous_edge != next_edge {
                            if let Some(id) = previous_edge {
                                self.emit(NodeGraphEvent::EdgeLeft(id));
                            }
                            if let Some(id) = next_edge {
                                self.emit(NodeGraphEvent::EdgeEntered(id));
                            }
                        }
                        ctx.request_paint();
                    }
                    return;
                };

                if interaction.pointer_id() != pointer.pointer_id {
                    return;
                }
                let auto_pan_enabled = match &interaction {
                    Interaction::DragNodes { .. } | Interaction::ResizeNode { .. } => {
                        config.auto_pan_on_node_drag
                    }
                    Interaction::Connect { .. } | Interaction::ReconnectEdge { .. } => {
                        config.auto_pan_on_connect
                    }
                    Interaction::Marquee { .. } => config.auto_pan_on_selection,
                    Interaction::Pan { .. } => false,
                };
                match interaction {
                    Interaction::Pan {
                        pointer_id,
                        last_position,
                    } => {
                        let mut viewport = snapshot.viewport;
                        viewport.pan_by(pointer.position - last_position);
                        if self.state.set_viewport(viewport) {
                            self.emit(NodeGraphEvent::ViewportChanged(viewport));
                        }
                        self.interaction = Some(Interaction::Pan {
                            pointer_id,
                            last_position: pointer.position,
                        });
                    }
                    Interaction::DragNodes {
                        pointer_id,
                        start,
                        origins,
                    } => {
                        let delta = pointer.position - start;
                        let flow_delta = Vector::new(
                            delta.x / snapshot.viewport.zoom.max(0.001),
                            delta.y / snapshot.viewport.zoom.max(0.001),
                        );
                        let mut changes = Vec::new();
                        self.state.update(|snapshot| {
                            for (id, origin) in &origins {
                                let mut position = *origin + flow_delta;
                                if let Some(grid) = config.snap_to_grid {
                                    position = snap_point(position, grid);
                                }
                                let graph = snapshot.graph_mut();
                                if graph.node(id).is_some_and(|node| node.position != position)
                                    && let Some(position) = graph.move_node(id, position)
                                {
                                    changes.push(NodeChange::Position {
                                        id: id.clone(),
                                        position,
                                        dragging: true,
                                    });
                                }
                            }
                        });
                        if !changes.is_empty() {
                            self.emit(NodeGraphEvent::NodesChanged(changes));
                        }
                        self.interaction = Some(Interaction::DragNodes {
                            pointer_id,
                            start,
                            origins,
                        });
                    }
                    Interaction::ResizeNode {
                        pointer_id,
                        node,
                        direction,
                        start,
                        original_bounds,
                        origin,
                    } => {
                        let delta = pointer.position - start;
                        let flow_delta = Vector::new(
                            delta.x / snapshot.viewport.zoom.max(0.001),
                            delta.y / snapshot.viewport.zoom.max(0.001),
                        );
                        if let Some(model) = snapshot.graph.node(&node) {
                            let (position, size) = resized_node_geometry(
                                original_bounds,
                                origin,
                                direction,
                                flow_delta,
                                model.min_size,
                                model.max_size,
                                config.resize_keep_aspect_ratio || pointer.modifiers.shift,
                            );
                            if self.state.resize_node(&node, position, size) {
                                self.emit(NodeGraphEvent::NodesChanged(vec![
                                    NodeChange::Dimensions {
                                        id: node.clone(),
                                        position,
                                        size,
                                        resizing: true,
                                    },
                                ]));
                            }
                        }
                        self.interaction = Some(Interaction::ResizeNode {
                            pointer_id,
                            node,
                            direction,
                            start,
                            original_bounds,
                            origin,
                        });
                    }
                    Interaction::Marquee {
                        pointer_id,
                        start,
                        additive,
                        ..
                    } => {
                        self.interaction = Some(Interaction::Marquee {
                            pointer_id,
                            start,
                            current: pointer.position,
                            additive,
                        });
                    }
                    Interaction::Connect {
                        pointer_id,
                        source,
                        source_handle,
                        source_position,
                        source_side,
                        ..
                    } => {
                        self.hovered_handle = hit_handle(
                            &snapshot,
                            bounds,
                            pointer.position,
                            Some(HandleKind::Target),
                        )
                        .map(|hit| (hit.node.id.clone(), hit.handle.id.clone(), hit.handle.kind));
                        self.interaction = Some(Interaction::Connect {
                            pointer_id,
                            source,
                            source_handle,
                            source_position,
                            source_side,
                            current: pointer.position,
                        });
                    }
                    Interaction::ReconnectEdge {
                        pointer_id,
                        edge,
                        endpoint,
                        fixed_position,
                        fixed_side,
                        ..
                    } => {
                        self.hovered_handle =
                            hit_handle(&snapshot, bounds, pointer.position, Some(endpoint)).map(
                                |hit| (hit.node.id.clone(), hit.handle.id.clone(), hit.handle.kind),
                            );
                        self.interaction = Some(Interaction::ReconnectEdge {
                            pointer_id,
                            edge,
                            endpoint,
                            fixed_position,
                            fixed_side,
                            current: pointer.position,
                        });
                    }
                }
                if auto_pan_enabled
                    && let Some(delta) = auto_pan_delta(
                        bounds,
                        pointer.position,
                        config.auto_pan_margin,
                        config.auto_pan_speed,
                    )
                {
                    let mut viewport = self.state.viewport();
                    viewport.pan_by(delta);
                    if self.state.set_viewport(viewport) {
                        self.emit(NodeGraphEvent::ViewportChanged(viewport));
                    }
                }
                self.request_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    || pointer.kind == PointerEventKind::Cancel =>
            {
                let Some(interaction) = self.interaction.clone() else {
                    return;
                };
                if interaction.pointer_id() != pointer.pointer_id {
                    return;
                }
                let cancelled = pointer.kind == PointerEventKind::Cancel;
                match interaction {
                    Interaction::Pan { .. } => {
                        self.emit(NodeGraphEvent::ViewportChangeEnded(self.state.viewport()));
                    }
                    Interaction::DragNodes { origins, .. } => {
                        let current = self.state.snapshot();
                        let dragged_ids =
                            origins.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
                        let changes = origins
                            .iter()
                            .filter_map(|(id, _)| {
                                current.graph.node(id).map(|node| NodeChange::Position {
                                    id: id.clone(),
                                    position: node.position,
                                    dragging: false,
                                })
                            })
                            .collect::<Vec<_>>();
                        if !changes.is_empty() {
                            self.emit(NodeGraphEvent::NodesChanged(changes));
                        }
                        self.emit(NodeGraphEvent::NodeDragStopped(dragged_ids));
                    }
                    Interaction::ResizeNode { node, .. } => {
                        let current = self.state.snapshot();
                        if let Some(node) = current.graph.node(&node) {
                            self.emit(NodeGraphEvent::NodesChanged(vec![NodeChange::Dimensions {
                                id: node.id.clone(),
                                position: node.position,
                                size: node.size,
                                resizing: false,
                            }]));
                            self.emit(NodeGraphEvent::NodeResizeStopped {
                                id: node.id.clone(),
                                position: node.position,
                                size: node.size,
                            });
                        }
                    }
                    Interaction::Marquee {
                        start,
                        current,
                        additive,
                        ..
                    } if !cancelled => {
                        let selection = screen_rect_from_points(start, current);
                        let flow_selection = snapshot
                            .viewport
                            .screen_rect_to_flow(ctx.bounds(), selection);
                        let mut changes = ElementChanges::default();
                        self.state.update(|snapshot| {
                            apply_marquee_selection(
                                snapshot.graph_mut(),
                                flow_selection,
                                additive,
                                config.selection_mode,
                                &mut changes,
                            );
                        });
                        emit_element_changes(self, changes);
                        self.emit(NodeGraphEvent::SelectionEnded);
                    }
                    Interaction::Marquee { .. } => {
                        self.emit(NodeGraphEvent::SelectionEnded);
                    }
                    Interaction::Connect {
                        source,
                        source_handle,
                        ..
                    } if !cancelled => {
                        let mut completed = None;
                        let target = hit_handle(
                            &snapshot,
                            ctx.bounds(),
                            pointer.position,
                            Some(HandleKind::Target),
                        );
                        if let Some(target) =
                            target.filter(|hit| hit.node.connectable && hit.handle.connectable)
                        {
                            let connection = Connection {
                                source,
                                source_handle: Some(source_handle),
                                target: target.node.id.clone(),
                                target_handle: Some(target.handle.id.clone()),
                            };
                            let valid = snapshot.graph.validate_connection(&connection).is_ok()
                                && self.connection_validator.as_ref().is_none_or(|validator| {
                                    validator(&connection, &snapshot.graph)
                                });
                            if valid {
                                completed = Some(connection.clone());
                                self.emit(NodeGraphEvent::Connect(connection.clone()));
                                let edge = if let Some(factory) = &mut self.edge_factory {
                                    factory(connection)
                                } else {
                                    self.next_default_edge(connection)
                                };
                                let id = edge.id.clone();
                                if self.state.add_edge(edge).is_ok() {
                                    self.emit(NodeGraphEvent::EdgesChanged(vec![
                                        EdgeChange::Added { id },
                                    ]));
                                }
                            }
                        }
                        self.emit(NodeGraphEvent::ConnectionEnded {
                            connection: completed,
                        });
                    }
                    Interaction::Connect { .. } => {
                        self.emit(NodeGraphEvent::ConnectionEnded { connection: None });
                    }
                    Interaction::ReconnectEdge { edge, endpoint, .. } if !cancelled => {
                        let mut completed = None;
                        let handle =
                            hit_handle(&snapshot, ctx.bounds(), pointer.position, Some(endpoint));
                        if let (Some(old_edge), Some(handle)) = (
                            snapshot.graph.edge(&edge),
                            handle.filter(|hit| hit.node.connectable && hit.handle.connectable),
                        ) {
                            let mut connection = old_edge.connection();
                            match endpoint {
                                HandleKind::Source => {
                                    connection.source = handle.node.id.clone();
                                    connection.source_handle = Some(handle.handle.id.clone());
                                }
                                HandleKind::Target => {
                                    connection.target = handle.node.id.clone();
                                    connection.target_handle = Some(handle.handle.id.clone());
                                }
                            }
                            let valid = snapshot.graph.validate_connection(&connection).is_ok()
                                && self.connection_validator.as_ref().is_none_or(|validator| {
                                    validator(&connection, &snapshot.graph)
                                });
                            if valid {
                                let replacement = connection.clone();
                                if self
                                    .state
                                    .update_edge(&edge, move |edge| edge.reconnect(replacement))
                                    .unwrap_or(false)
                                {
                                    completed = Some(connection.clone());
                                    self.emit(NodeGraphEvent::EdgesChanged(vec![
                                        EdgeChange::Reconnected {
                                            id: edge.clone(),
                                            connection,
                                        },
                                    ]));
                                }
                            }
                        }
                        self.emit(NodeGraphEvent::EdgeReconnectEnded {
                            id: edge,
                            connection: completed,
                        });
                    }
                    Interaction::ReconnectEdge { edge, .. } => {
                        self.emit(NodeGraphEvent::EdgeReconnectEnded {
                            id: edge,
                            connection: None,
                        });
                    }
                }
                self.interaction = None;
                self.hovered_handle = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                self.request_update(ctx);
                ctx.set_handled();
            }
            Event::Semantics(semantics) => {
                if semantics.target == ctx.widget_id() {
                    match &semantics.action {
                        SemanticsActionRequest::Focus => ctx.request_focus(),
                        SemanticsActionRequest::Blur => ctx.clear_focus(),
                        SemanticsActionRequest::Custom { name, .. } if name == "Fit view" => {
                            self.state.fit_view(ctx.bounds().size, config.fit_view);
                        }
                        _ => return,
                    }
                    self.request_update(ctx);
                    ctx.set_handled();
                    return;
                }
                let Some(element) =
                    semantics_element_for(ctx.widget_id(), &snapshot, semantics.target)
                else {
                    return;
                };
                match &semantics.action {
                    SemanticsActionRequest::Focus => {
                        self.focused_element = Some(element);
                        ctx.request_focus();
                    }
                    SemanticsActionRequest::Blur => {
                        if self.focused_element.as_ref() == Some(&element) {
                            self.focused_element = None;
                        }
                    }
                    SemanticsActionRequest::Activate => {
                        let mut changes = ElementChanges::default();
                        self.state.update(|snapshot| match &element {
                            FocusedElement::Node(id) => {
                                apply_node_selection(snapshot.graph_mut(), id, false, &mut changes)
                            }
                            FocusedElement::Edge(id) => {
                                apply_edge_selection(snapshot.graph_mut(), id, false, &mut changes)
                            }
                        });
                        self.focused_element = Some(element);
                        emit_element_changes(self, changes);
                    }
                    SemanticsActionRequest::Custom { name, .. } if name == "Delete" => {
                        let deleted = match &element {
                            FocusedElement::Node(id) => {
                                self.state.delete_elements(std::slice::from_ref(id), &[])
                            }
                            FocusedElement::Edge(id) => {
                                self.state.delete_elements(&[], std::slice::from_ref(id))
                            }
                        };
                        let changes = ElementChanges {
                            nodes: deleted
                                .nodes
                                .into_iter()
                                .map(|node| NodeChange::Removed { id: node.id })
                                .collect(),
                            edges: deleted
                                .edges
                                .into_iter()
                                .map(|edge| EdgeChange::Removed { id: edge.id })
                                .collect(),
                        };
                        self.focused_element = None;
                        emit_element_changes(self, changes);
                    }
                    _ => return,
                }
                self.request_update(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                if let Some(mut transition) = snapshot.viewport_transition {
                    if !self.viewport_transition_running {
                        self.viewport_transition_running = true;
                        self.emit(NodeGraphEvent::ViewportChangeStarted(transition.from));
                    }
                    let (viewport, finished) = transition.advance(*delta);
                    self.state.update(|snapshot| {
                        snapshot.viewport = viewport;
                        snapshot.viewport_transition = (!finished).then_some(transition);
                    });
                    self.emit(NodeGraphEvent::ViewportChanged(viewport));
                    if finished {
                        self.viewport_transition_running = false;
                        self.emit(NodeGraphEvent::ViewportChangeEnded(viewport));
                    } else {
                        ctx.request_animation_frame();
                    }
                    self.request_update(ctx);
                }
                if snapshot
                    .graph
                    .edges
                    .iter()
                    .any(|edge| edge.animated && !edge.hidden)
                {
                    self.edge_animation_time =
                        (self.edge_animation_time + *delta as f32).rem_euclid(1.0);
                    ctx.request_paint();
                    ctx.request_animation_frame();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.interaction.is_none() {
                    let node = self.hovered_node.take();
                    let edge = self.hovered_edge.take();
                    let handle = self.hovered_handle.take();
                    let changed = node.is_some() || edge.is_some() || handle.is_some();
                    if let Some(id) = node {
                        self.emit(NodeGraphEvent::NodeLeft(id));
                    }
                    if let Some(id) = edge {
                        self.emit(NodeGraphEvent::EdgeLeft(id));
                    }
                    if changed {
                        ctx.request_paint();
                    }
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "Tab" => {
                        let elements = focusable_elements(&snapshot);
                        if elements.is_empty() {
                            return;
                        }
                        let current = self
                            .focused_element
                            .as_ref()
                            .and_then(|focused| elements.iter().position(|item| item == focused));
                        let next = if key.modifiers.shift {
                            current.map_or(elements.len() - 1, |index| {
                                index.checked_sub(1).unwrap_or(elements.len() - 1)
                            })
                        } else {
                            current.map_or(0, |index| (index + 1) % elements.len())
                        };
                        self.focused_element = Some(elements[next].clone());
                        ctx.request_focus();
                    }
                    "Enter" | " " if self.focused_element.is_some() => {
                        let element = self.focused_element.clone().unwrap();
                        let mut changes = ElementChanges::default();
                        self.state.update(|snapshot| match &element {
                            FocusedElement::Node(id) => {
                                apply_node_selection(snapshot.graph_mut(), id, false, &mut changes)
                            }
                            FocusedElement::Edge(id) => {
                                apply_edge_selection(snapshot.graph_mut(), id, false, &mut changes)
                            }
                        });
                        emit_element_changes(self, changes);
                    }
                    "Delete" | "Backspace" if config.delete_key_enabled => {
                        let mut changes = ElementChanges::default();
                        self.state.update(|snapshot| {
                            delete_selected(snapshot.graph_mut(), &mut changes);
                        });
                        emit_element_changes(self, changes);
                    }
                    "a" | "A" if key.modifiers.control || key.modifiers.meta => {
                        let mut changes = ElementChanges::default();
                        self.state.update(|snapshot| {
                            select_all(snapshot.graph_mut(), &mut changes);
                        });
                        emit_element_changes(self, changes);
                    }
                    "Escape" => {
                        self.interaction = None;
                        let mut changes = ElementChanges::default();
                        self.state.update(|snapshot| {
                            clear_selection(snapshot.graph_mut(), &mut changes);
                        });
                        emit_element_changes(self, changes);
                    }
                    "Home" => {
                        if self.state.fit_view(ctx.bounds().size, config.fit_view) {
                            self.emit(NodeGraphEvent::ViewportChanged(self.state.viewport()));
                        }
                    }
                    "=" | "+" | "-" => {
                        let mut viewport = snapshot.viewport;
                        let factor: f32 = if key.key == "-" { 1.0 / 1.2 } else { 1.2 };
                        viewport.zoom_at(
                            ctx.bounds(),
                            center(ctx.bounds()),
                            factor,
                            config.min_zoom,
                            config.max_zoom,
                        );
                        if self.state.set_viewport(viewport) {
                            self.emit(NodeGraphEvent::ViewportChanged(viewport));
                        }
                    }
                    "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" => {
                        let step = if key.modifiers.shift { 10.0 } else { 1.0 };
                        let delta = match key.key.as_str() {
                            "ArrowLeft" => Vector::new(-step, 0.0),
                            "ArrowRight" => Vector::new(step, 0.0),
                            "ArrowUp" => Vector::new(0.0, -step),
                            _ => Vector::new(0.0, step),
                        };
                        let mut changes = Vec::new();
                        self.state.update(|snapshot| {
                            let graph = snapshot.graph_mut();
                            let ids = graph
                                .nodes
                                .iter()
                                .filter(|node| {
                                    node.selected
                                        && node.draggable
                                        && config.nodes_draggable
                                        && !has_selected_ancestor(graph, node)
                                })
                                .map(|node| node.id.clone())
                                .collect::<Vec<_>>();
                            for id in ids {
                                let mut position = graph.node(&id).unwrap().position + delta;
                                if let Some(grid) = config.snap_to_grid {
                                    position = snap_point(position, grid);
                                }
                                if let Some(position) = graph.move_node(&id, position) {
                                    changes.push(NodeChange::Position {
                                        id,
                                        position,
                                        dragging: false,
                                    });
                                }
                            }
                        });
                        if !changes.is_empty() {
                            self.emit(NodeGraphEvent::NodesChanged(changes));
                        }
                    }
                    _ => return,
                }
                self.request_update(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let snapshot = self.state.snapshot();
        self.world_layer_active = self.edge_painter.is_none()
            && self.config.retain_edge_world
            && (self.config.retained_edge_world_min..=self.config.retained_edge_world_max)
                .contains(&snapshot.graph.edges.len());
        if self.world_layer_active {
            self.world_layer
                .measure(ctx, Constraints::tight(Size::ZERO));
        }
        self.last_measured_nodes_revision = snapshot.revisions.nodes;
        let structure_changed = self
            .node_widgets
            .reconcile(&snapshot.graph.nodes, &mut self.node_widget_registry);
        let mut measured_sizes = Vec::new();
        for node in snapshot.graph.nodes.iter().filter(|node| !node.hidden) {
            let Some(entry) = self.node_widgets.get_mut(&node.id) else {
                continue;
            };
            let child_constraints = match node.size_mode {
                NodeSizeMode::Fixed => Constraints::tight(node.size),
                NodeSizeMode::Content { min, max } => Constraints::new(min, max),
            };
            let measured = entry.pod.measure(ctx, child_constraints);
            if matches!(node.size_mode, NodeSizeMode::Content { .. }) && measured != node.size {
                measured_sizes.push((node.id.clone(), measured));
            }
        }

        if !measured_sizes.is_empty() {
            self.state.update_authoritative(|snapshot| {
                for (id, size) in &measured_sizes {
                    if let Some(node) = snapshot.graph_mut().node_mut(id) {
                        node.size = *size;
                    }
                }
            });
        }
        if structure_changed || !measured_sizes.is_empty() {
            ctx.request_arrange();
            ctx.request_paint();
            ctx.request_semantics();
        }

        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                self.desired_size.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                self.desired_size.height
            },
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.state.set_viewport_size(bounds.size);
        if !self.arranged_once && self.config.fit_view_on_init {
            self.state.fit_view(bounds.size, self.config.fit_view);
            ctx.request_paint();
            ctx.request_semantics();
        }
        let snapshot = ctx.observe_with(&self.state.signal, InvalidationKind::Transform);
        self.world_layer_active = self.edge_painter.is_none()
            && self.config.retain_edge_world
            && (self.config.retained_edge_world_min..=self.config.retained_edge_world_max)
                .contains(&snapshot.graph.edges.len());
        if self.world_layer_active {
            self.world_layer.arrange(ctx, bounds);
        }
        if snapshot.revisions.nodes != self.last_measured_nodes_revision {
            ctx.request_measure();
        }
        let visible = snapshot.viewport.visible_flow_rect(bounds).inflate(
            self.config.culling_margin / snapshot.viewport.zoom.max(0.001),
            self.config.culling_margin / snapshot.viewport.zoom.max(0.001),
        );
        self.visible_node_indices = if self.config.cull_offscreen {
            snapshot.spatial.query_node_indices(visible)
        } else {
            (0..snapshot.graph.nodes.len()).collect()
        };
        self.visible_edge_indices = if self.config.cull_offscreen {
            snapshot.spatial.query_edge_indices(visible)
        } else {
            (0..snapshot.graph.edges.len()).collect()
        };
        sort_node_indices(&snapshot, &mut self.visible_node_indices);
        sort_edge_indices(&snapshot, &mut self.visible_edge_indices);

        let next_active_node_widgets = self
            .visible_node_indices
            .iter()
            .filter_map(|index| snapshot.graph.nodes.get(*index))
            .filter(|node| !node.hidden && self.node_widgets.contains(&node.id))
            .map(|node| node.id.clone())
            .collect::<HashSet<_>>();
        let leaving_node_widgets = self
            .active_node_widgets
            .difference(&next_active_node_widgets)
            .cloned()
            .collect::<Vec<_>>();
        for id in leaving_node_widgets {
            if let Some(entry) = self.node_widgets.get_mut(&id) {
                entry.pod.arrange(ctx, Rect::ZERO);
            }
        }

        let canvas_viewport = snapshot.viewport.to_canvas(bounds.size);
        for index in &self.visible_node_indices {
            let Some(node) = snapshot.graph.nodes.get(*index) else {
                continue;
            };
            let Some(entry) = self.node_widgets.get_mut(&node.id) else {
                continue;
            };
            let child_bounds = graph_node_bounds(&snapshot.graph, node);
            let transform = self
                .node_widget_registry
                .zoom_behavior(&node.kind)
                .transform(CanvasZoomContext {
                    viewport: canvas_viewport,
                    canvas_bounds: bounds,
                    document_origin: Point::ZERO,
                    content_bounds: child_bounds,
                });
            entry.pod.arrange_transformed(ctx, child_bounds, transform);
        }
        self.active_node_widgets = next_active_node_widgets;
        if snapshot
            .graph
            .edges
            .iter()
            .any(|edge| edge.animated && !edge.hidden)
        {
            ctx.request_animation_frame();
        }
        if snapshot.viewport_transition.is_some() {
            ctx.request_animation_frame();
        }
        self.arranged_once = true;
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let snapshot = ctx.observe(&self.state.signal);
        let theme = self.resolved_theme();
        let appearance = self.appearance.resolve(&theme);
        let bounds = ctx.bounds();
        let focused_node = match &self.focused_element {
            Some(FocusedElement::Node(id)) => Some(id),
            _ => None,
        };
        let focused_edge = match &self.focused_element {
            Some(FocusedElement::Edge(id)) => Some(id),
            _ => None,
        };

        CanvasSurface::new(snapshot.viewport.to_canvas(bounds.size))
            .grid_spacing(self.config.grid_spacing)
            .grid_style(match self.config.background_variant {
                BackgroundVariant::Dots => CanvasGridStyle::Dots,
                BackgroundVariant::Cross => CanvasGridStyle::Cross,
                BackgroundVariant::Lines => CanvasGridStyle::Lines,
            })
            .paint_background(ctx, bounds, appearance.background, appearance.grid);
        ctx.push_clip_rect(bounds);
        let foreground_start = self
            .visible_node_indices
            .iter()
            .position(|index| {
                snapshot
                    .graph
                    .nodes
                    .get(*index)
                    .is_none_or(|node| node.z_index >= 0)
            })
            .unwrap_or(self.visible_node_indices.len());
        let (background_node_indices, foreground_node_indices) =
            self.visible_node_indices.split_at(foreground_start);
        paint_nodes(
            ctx,
            &snapshot,
            bounds,
            background_node_indices,
            NodePaintOptions {
                appearance,
                hovered_node: self.hovered_node.as_ref(),
                theme: &theme,
                painter: self.node_painter.as_deref(),
                custom_nodes: &self.node_widgets,
                registry: &self.node_widget_registry,
            },
        );
        let retained_edges =
            self.paint_retained_edge_world(ctx, &snapshot, bounds, appearance.edge, focused_edge);
        paint_edges(
            ctx,
            &snapshot,
            bounds,
            &self.visible_edge_indices,
            retained_edges.as_deref(),
            EdgePaintOptions {
                appearance,
                hovered_edge: self.hovered_edge.as_ref(),
                theme: &theme,
                painter: self.edge_painter.as_deref(),
                edges_reconnectable: self.config.edges_reconnectable,
                focused_edge,
                animation_time: self.edge_animation_time,
            },
        );
        paint_connection(
            ctx,
            self.interaction.as_ref(),
            appearance,
            self.config.connection_line_kind,
        );
        paint_nodes(
            ctx,
            &snapshot,
            bounds,
            foreground_node_indices,
            NodePaintOptions {
                appearance,
                hovered_node: self.hovered_node.as_ref(),
                theme: &theme,
                painter: self.node_painter.as_deref(),
                custom_nodes: &self.node_widgets,
                registry: &self.node_widget_registry,
            },
        );
        paint_node_overlays(
            ctx,
            &snapshot,
            bounds,
            NodeOverlayOptions {
                appearance,
                hovered_handle: self.hovered_handle.as_ref(),
                nodes_resizable: self.config.nodes_resizable,
                focused_node,
                indices: &self.visible_node_indices,
            },
        );
        if let Some(Interaction::Marquee { start, current, .. }) = self.interaction {
            let rect = screen_rect_from_points(start, current);
            ctx.fill_rect(rect, appearance.marquee_fill);
            ctx.stroke_rect(rect, appearance.marquee_border, StrokeStyle::new(1.0));
        }
        ctx.pop_clip();

        let border = if ctx.is_focused() {
            theme.palette.focus_ring
        } else {
            theme.palette.border
        };
        ctx.stroke_rect(bounds, border, StrokeStyle::new(theme.metrics.border_width));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let snapshot = ctx.observe(&self.state.signal);
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some(if self.built_in_events {
            "Node graph editor. Arrow keys move selected nodes; Delete removes selected elements; Home fits the graph."
                .to_string()
        } else {
            "Paint-only node graph surface controlled by application state.".to_string()
        });
        node.value = Some(SemanticsValue::Text(format!(
            "{} nodes, {} edges, {} selected, zoom {:.0}%",
            snapshot.graph.nodes.len(),
            snapshot.graph.edges.len(),
            snapshot.graph.selected_node_ids().len() + snapshot.graph.selected_edge_ids().len(),
            snapshot.viewport.zoom * 100.0
        )));
        node.state.focused = ctx.is_focused();
        if self.built_in_events {
            node.actions = vec![
                SemanticsAction::Focus,
                SemanticsAction::Custom("Fit view".into()),
            ];
        }
        ctx.push(node);

        for index in &self.visible_node_indices {
            let Some(graph_node) = snapshot.graph.nodes.get(*index) else {
                continue;
            };
            if graph_node.hidden {
                continue;
            }
            let element = FocusedElement::Node(graph_node.id.clone());
            let Some(flow_bounds) = snapshot.graph.node_bounds(graph_node) else {
                continue;
            };
            let mut node = SemanticsNode::new(
                element_semantics_id(ctx.widget_id(), &element),
                SemanticsRole::GenericContainer,
                snapshot
                    .viewport
                    .flow_rect_to_screen(ctx.bounds(), flow_bounds),
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some(
                graph_node
                    .aria_label
                    .clone()
                    .unwrap_or_else(|| graph_node.label.clone()),
            );
            node.description = Some(format!(
                "{} node with {} connection handles",
                graph_node.kind,
                graph_node.handles.len()
            ));
            node.state.selected = graph_node.selected;
            node.state.disabled = !graph_node.selectable;
            node.state.focused = self.focused_element.as_ref() == Some(&element);
            if self.built_in_events && graph_node.focusable {
                node.actions.push(SemanticsAction::Focus);
            }
            if self.built_in_events && graph_node.selectable {
                node.actions.push(SemanticsAction::Activate);
            }
            if self.built_in_events && graph_node.deletable {
                node.actions.push(SemanticsAction::Custom("Delete".into()));
            }
            ctx.push(node);
        }

        for index in &self.visible_edge_indices {
            let Some(edge) = snapshot.graph.edges.get(*index) else {
                continue;
            };
            if edge.hidden {
                continue;
            }
            let element = FocusedElement::Edge(edge.id.clone());
            let Some(flow_bounds) = snapshot.spatial.edge_bounds(&edge.id) else {
                continue;
            };
            let mut node = SemanticsNode::new(
                element_semantics_id(ctx.widget_id(), &element),
                SemanticsRole::GenericContainer,
                snapshot
                    .viewport
                    .flow_rect_to_screen(ctx.bounds(), flow_bounds),
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some(edge.aria_label.clone().unwrap_or_else(|| {
                edge.label
                    .clone()
                    .unwrap_or_else(|| format!("{} to {}", edge.source, edge.target))
            }));
            node.description = Some(format!("{:?} edge", edge.kind));
            node.state.selected = edge.selected;
            node.state.disabled = !edge.selectable;
            node.state.focused = self.focused_element.as_ref() == Some(&element);
            if self.built_in_events && edge.focusable {
                node.actions.push(SemanticsAction::Focus);
            }
            if self.built_in_events && edge.selectable {
                node.actions.push(SemanticsAction::Activate);
            }
            if self.built_in_events && edge.deletable {
                node.actions.push(SemanticsAction::Custom("Delete".into()));
            }
            ctx.push(node);
        }
        for index in &self.visible_node_indices {
            let Some(graph_node) = snapshot.graph.nodes.get(*index) else {
                continue;
            };
            self.node_widgets.semantics_node(ctx, &graph_node.id);
        }
    }

    fn accepts_focus(&self) -> bool {
        self.built_in_events
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        self.request_update(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.world_layer_active {
            visitor.visit(&self.world_layer);
        }
        self.node_widgets.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.world_layer_active {
            visitor.visit(&mut self.world_layer);
        }
        self.node_widgets.visit_children_mut(visitor);
    }
}

#[derive(Debug, Default)]
struct ElementChanges {
    nodes: Vec<NodeChange>,
    edges: Vec<EdgeChange>,
}

fn element_semantics_id(owner: WidgetId, element: &FocusedElement) -> WidgetId {
    let (tag, value) = match element {
        FocusedElement::Node(id) => (4_u64, id.as_str()),
        FocusedElement::Edge(id) => (5_u64, id.as_str()),
    };
    let mut hash = 0xcbf29ce484222325_u64 ^ owner.get();
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    const LOW_MASK: u64 = (1_u64 << 49) - 1;
    WidgetId::new((tag << 49) | (hash & LOW_MASK))
}

fn semantics_element_for<N, E>(
    owner: WidgetId,
    snapshot: &GraphSnapshot<N, E>,
    target: WidgetId,
) -> Option<FocusedElement> {
    snapshot
        .graph
        .nodes
        .iter()
        .filter(|node| !node.hidden && node.focusable)
        .map(|node| FocusedElement::Node(node.id.clone()))
        .chain(
            snapshot
                .graph
                .edges
                .iter()
                .filter(|edge| !edge.hidden && edge.focusable)
                .map(|edge| FocusedElement::Edge(edge.id.clone())),
        )
        .find(|element| element_semantics_id(owner, element) == target)
}

fn focusable_elements<N, E>(snapshot: &GraphSnapshot<N, E>) -> Vec<FocusedElement> {
    snapshot
        .graph
        .nodes
        .iter()
        .filter(|node| !node.hidden && node.focusable)
        .map(|node| FocusedElement::Node(node.id.clone()))
        .chain(
            snapshot
                .graph
                .edges
                .iter()
                .filter(|edge| !edge.hidden && edge.focusable)
                .map(|edge| FocusedElement::Edge(edge.id.clone())),
        )
        .collect()
}

impl ElementChanges {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }
}

fn emit_element_changes<N, E>(graph: &mut NodeGraph<N, E>, changes: ElementChanges)
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + Default + 'static,
{
    if changes.is_empty() {
        return;
    }
    if !changes.nodes.is_empty() {
        graph.emit(NodeGraphEvent::NodesChanged(changes.nodes));
    }
    if !changes.edges.is_empty() {
        graph.emit(NodeGraphEvent::EdgesChanged(changes.edges));
    }
    graph.emit_selection();
}

fn apply_node_selection<N, E>(
    graph: &mut GraphModel<N, E>,
    id: &NodeId,
    toggle: bool,
    changes: &mut ElementChanges,
) {
    if toggle {
        if let Some(node) = graph.node_mut(id) {
            node.selected = !node.selected;
            changes.nodes.push(NodeChange::Selected {
                id: id.clone(),
                selected: node.selected,
            });
        }
        return;
    }

    for node in &mut graph.nodes {
        let selected = node.id == *id;
        if node.selected != selected {
            node.selected = selected;
            changes.nodes.push(NodeChange::Selected {
                id: node.id.clone(),
                selected,
            });
        }
    }
    for edge in &mut graph.edges {
        if edge.selected {
            edge.selected = false;
            changes.edges.push(EdgeChange::Selected {
                id: edge.id.clone(),
                selected: false,
            });
        }
    }
}

fn apply_edge_selection<N, E>(
    graph: &mut GraphModel<N, E>,
    id: &EdgeId,
    toggle: bool,
    changes: &mut ElementChanges,
) {
    if toggle {
        if let Some(edge) = graph.edge_mut(id) {
            edge.selected = !edge.selected;
            changes.edges.push(EdgeChange::Selected {
                id: id.clone(),
                selected: edge.selected,
            });
        }
        return;
    }

    for node in &mut graph.nodes {
        if node.selected {
            node.selected = false;
            changes.nodes.push(NodeChange::Selected {
                id: node.id.clone(),
                selected: false,
            });
        }
    }
    for edge in &mut graph.edges {
        let selected = edge.id == *id;
        if edge.selected != selected {
            edge.selected = selected;
            changes.edges.push(EdgeChange::Selected {
                id: edge.id.clone(),
                selected,
            });
        }
    }
}

fn clear_selection<N, E>(graph: &mut GraphModel<N, E>, changes: &mut ElementChanges) {
    for node in &mut graph.nodes {
        if node.selected {
            node.selected = false;
            changes.nodes.push(NodeChange::Selected {
                id: node.id.clone(),
                selected: false,
            });
        }
    }
    for edge in &mut graph.edges {
        if edge.selected {
            edge.selected = false;
            changes.edges.push(EdgeChange::Selected {
                id: edge.id.clone(),
                selected: false,
            });
        }
    }
}

fn select_all<N, E>(graph: &mut GraphModel<N, E>, changes: &mut ElementChanges) {
    for node in graph
        .nodes
        .iter_mut()
        .filter(|node| node.selectable && !node.hidden)
    {
        if !node.selected {
            node.selected = true;
            changes.nodes.push(NodeChange::Selected {
                id: node.id.clone(),
                selected: true,
            });
        }
    }
    for edge in graph
        .edges
        .iter_mut()
        .filter(|edge| edge.selectable && !edge.hidden)
    {
        if !edge.selected {
            edge.selected = true;
            changes.edges.push(EdgeChange::Selected {
                id: edge.id.clone(),
                selected: true,
            });
        }
    }
}

fn apply_marquee_selection<N, E>(
    graph: &mut GraphModel<N, E>,
    selection: Rect,
    additive: bool,
    mode: SelectionMode,
    changes: &mut ElementChanges,
) {
    for index in 0..graph.nodes.len() {
        let bounds = {
            let node = &graph.nodes[index];
            if !node.selectable || node.hidden {
                continue;
            }
            graph_node_bounds(graph, node)
        };
        let node = &mut graph.nodes[index];
        if !node.selectable || node.hidden {
            continue;
        }
        let intersects = match mode {
            SelectionMode::Full => rect_contains_rect(selection, bounds),
            SelectionMode::Partial => bounds.intersection(selection).is_some(),
        };
        let selected = if additive {
            node.selected || intersects
        } else {
            intersects
        };
        if node.selected != selected {
            node.selected = selected;
            changes.nodes.push(NodeChange::Selected {
                id: node.id.clone(),
                selected,
            });
        }
    }
    let selected_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.selectable && !edge.hidden)
        .map(|edge| {
            let selected = edge_intersects_rect(graph, edge, selection, mode);
            (edge.id.clone(), selected)
        })
        .collect::<Vec<_>>();
    for (id, intersects) in selected_edges {
        let Some(edge) = graph.edge_mut(&id) else {
            continue;
        };
        let selected = if additive {
            edge.selected || intersects
        } else {
            intersects
        };
        if edge.selected != selected {
            edge.selected = selected;
            changes.edges.push(EdgeChange::Selected { id, selected });
        }
    }
}

fn edge_intersects_rect<N, E>(
    graph: &GraphModel<N, E>,
    edge: &Edge<E>,
    selection: Rect,
    mode: SelectionMode,
) -> bool {
    let bounds = Rect::new(0.0, 0.0, 1.0, 1.0);
    let Some(geometry) = edge_geometry(graph, edge, Viewport::default(), bounds) else {
        return false;
    };
    match mode {
        SelectionMode::Full => {
            selection.contains(geometry.source) && selection.contains(geometry.target)
        }
        SelectionMode::Partial => match geometry.kind {
            EdgeKind::Straight => {
                segment_intersects_rect(geometry.source, geometry.target, selection)
            }
            EdgeKind::Step | EdgeKind::SmoothStep => {
                let horizontal = geometry.target_tangent.x.abs() > 0.0;
                let (first, second) = if horizontal {
                    (
                        Point::new(geometry.midpoint.x, geometry.source.y),
                        Point::new(geometry.midpoint.x, geometry.target.y),
                    )
                } else {
                    (
                        Point::new(geometry.source.x, geometry.midpoint.y),
                        Point::new(geometry.target.x, geometry.midpoint.y),
                    )
                };
                segment_intersects_rect(geometry.source, first, selection)
                    || segment_intersects_rect(first, second, selection)
                    || segment_intersects_rect(second, geometry.target, selection)
            }
            EdgeKind::Bezier | EdgeKind::SimpleBezier => (0..=EDGE_HIT_STEPS).any(|step| {
                selection.contains(cubic_point(
                    geometry.source,
                    geometry.control_1,
                    geometry.control_2,
                    geometry.target,
                    step as f32 / EDGE_HIT_STEPS as f32,
                ))
            }),
        },
    }
}

fn delete_selected<N, E>(graph: &mut GraphModel<N, E>, changes: &mut ElementChanges) {
    let mut removed_nodes = graph
        .nodes
        .iter()
        .filter(|node| node.selected && node.deletable)
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let selected_roots = removed_nodes.iter().cloned().collect::<Vec<_>>();
    for id in selected_roots {
        removed_nodes.extend(
            graph
                .descendants(&id)
                .into_iter()
                .map(|node| node.id.clone()),
        );
    }
    let removed_edges = graph
        .edges
        .iter()
        .filter(|edge| {
            (edge.selected && edge.deletable)
                || removed_nodes.contains(&edge.source)
                || removed_nodes.contains(&edge.target)
        })
        .map(|edge| edge.id.clone())
        .collect::<HashSet<_>>();

    graph.nodes.retain(|node| !removed_nodes.contains(&node.id));
    graph.edges.retain(|edge| !removed_edges.contains(&edge.id));
    changes.nodes.extend(
        removed_nodes
            .into_iter()
            .map(|id| NodeChange::Removed { id }),
    );
    changes.edges.extend(
        removed_edges
            .into_iter()
            .map(|id| EdgeChange::Removed { id }),
    );
}

fn has_selected_ancestor<N, E>(graph: &GraphModel<N, E>, node: &Node<N>) -> bool {
    let mut current = node.parent_id.as_ref();
    let mut seen = HashSet::new();
    while let Some(parent) = current {
        if !seen.insert(parent) {
            return false;
        }
        let Some(parent_node) = graph.node(parent) else {
            return false;
        };
        if parent_node.selected {
            return true;
        }
        current = parent_node.parent_id.as_ref();
    }
    false
}

fn hit_node<N, E>(
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    position: Point,
) -> Option<&Node<N>> {
    let flow_position = snapshot.viewport.screen_to_flow(bounds, position);
    let mut indices = snapshot.spatial.query_node_indices_at(flow_position);
    sort_node_indices(snapshot, &mut indices);
    indices
        .into_iter()
        .rev()
        .filter_map(|index| snapshot.graph.nodes.get(index))
        .find(|node| {
            snapshot
                .viewport
                .flow_rect_to_screen(bounds, graph_node_bounds(&snapshot.graph, node))
                .contains(position)
        })
}

fn hit_resize_handle<N, E>(
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    position: Point,
) -> Option<ResizeHit<'_, N>> {
    let radius = 8.0;
    let flow_position = snapshot.viewport.screen_to_flow(bounds, position);
    let flow_radius = radius / snapshot.viewport.zoom.max(0.001);
    let query = Rect::new(
        flow_position.x - flow_radius,
        flow_position.y - flow_radius,
        flow_radius * 2.0,
        flow_radius * 2.0,
    );
    let mut indices = snapshot.spatial.query_node_indices(query);
    sort_node_indices(snapshot, &mut indices);
    indices
        .into_iter()
        .rev()
        .filter_map(|index| snapshot.graph.nodes.get(index))
        .filter(|node| node.selected && node.resizable && !node.hidden)
        .find_map(|node| {
            let rect = snapshot
                .viewport
                .flow_rect_to_screen(bounds, graph_node_bounds(&snapshot.graph, node));
            resize_handle_points(rect)
                .into_iter()
                .find(|(_, handle)| vector_length(position - *handle) <= radius)
                .map(|(direction, _)| ResizeHit { node, direction })
        })
}

fn hit_reconnect_handle<N, E>(
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    position: Point,
) -> Option<ReconnectHit<'_, E>> {
    let radius = 10.0;
    let flow_position = snapshot.viewport.screen_to_flow(bounds, position);
    let flow_radius = radius / snapshot.viewport.zoom.max(0.001);
    let query = Rect::new(
        flow_position.x - flow_radius,
        flow_position.y - flow_radius,
        flow_radius * 2.0,
        flow_radius * 2.0,
    );
    let mut indices = snapshot.spatial.query_edge_indices(query);
    sort_edge_indices(snapshot, &mut indices);
    indices
        .into_iter()
        .rev()
        .filter_map(|index| snapshot.graph.edges.get(index))
        .filter(|edge| edge.selected && !edge.hidden)
        .find_map(|edge| {
            let geometry = edge_geometry(&snapshot.graph, edge, snapshot.viewport, bounds)?;
            if edge.reconnectable.allows(HandleKind::Source)
                && vector_length(position - geometry.source) <= radius
            {
                Some(ReconnectHit {
                    edge,
                    endpoint: HandleKind::Source,
                    fixed_position: geometry.target,
                    fixed_side: geometry.target_side,
                })
            } else if edge.reconnectable.allows(HandleKind::Target)
                && vector_length(position - geometry.target) <= radius
            {
                Some(ReconnectHit {
                    edge,
                    endpoint: HandleKind::Target,
                    fixed_position: geometry.source,
                    fixed_side: geometry.source_side,
                })
            } else {
                None
            }
        })
}

fn sort_node_indices<N, E>(snapshot: &GraphSnapshot<N, E>, indices: &mut [usize]) {
    let mut keys = vec![(i32::MIN, 0, false, 0); snapshot.graph.nodes.len()];
    for index in indices.iter().copied() {
        if let Some(node) = snapshot.graph.nodes.get(index) {
            keys[index] = (
                node.z_index,
                node_depth_from_node(&snapshot.graph, node),
                node.selected,
                index,
            );
        }
    }
    indices.sort_by_key(|index| keys.get(*index).copied().unwrap_or_default());
}

fn node_depth_from_node<N, E>(graph: &GraphModel<N, E>, node: &Node<N>) -> usize {
    let Some(mut current) = node.parent_id.as_ref() else {
        return 0;
    };
    let mut depth = 0;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(current) {
            break;
        }
        depth += 1;
        let Some(parent) = graph.node(current) else {
            break;
        };
        let Some(next) = parent.parent_id.as_ref() else {
            break;
        };
        current = next;
    }
    depth
}

fn sort_edge_indices<N, E>(snapshot: &GraphSnapshot<N, E>, indices: &mut [usize]) {
    indices.sort_by_key(|index| {
        snapshot
            .graph
            .edges
            .get(*index)
            .map_or((false, i32::MIN, *index), |edge| {
                (edge.selected, edge.z_index, *index)
            })
    });
}

fn hit_handle<'a, N, E>(
    snapshot: &'a GraphSnapshot<N, E>,
    bounds: Rect,
    position: Point,
    kind: Option<HandleKind>,
) -> Option<HandleHit<'a, N>> {
    let radius = handle_hit_radius(snapshot.viewport.zoom);
    let flow_position = snapshot.viewport.screen_to_flow(bounds, position);
    let flow_radius = radius / snapshot.viewport.zoom.max(0.001);
    let query = Rect::new(
        flow_position.x - flow_radius,
        flow_position.y - flow_radius,
        flow_radius * 2.0,
        flow_radius * 2.0,
    );
    let mut indices = snapshot.spatial.query_node_indices(query);
    sort_node_indices(snapshot, &mut indices);
    indices
        .into_iter()
        .rev()
        .filter_map(|index| snapshot.graph.nodes.get(index))
        .find_map(|node| {
            if node.hidden {
                return None;
            }
            node.handles.iter().find_map(|handle| {
                if kind.is_some_and(|kind| kind != handle.kind) {
                    return None;
                }
                let handle_position = snapshot
                    .viewport
                    .flow_to_screen(bounds, handle_position(&snapshot.graph, node, handle));
                (vector_length(position - handle_position) <= radius).then_some(HandleHit {
                    node,
                    handle,
                    position: handle_position,
                })
            })
        })
}

/// Hit-test graph elements without invoking the built-in interaction system.
pub fn node_graph_hit_test<N, E>(
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    position: Point,
) -> NodeGraphHit {
    if let Some(hit) = hit_handle(snapshot, bounds, position, None) {
        return NodeGraphHit::Handle {
            node: hit.node.id.clone(),
            handle: hit.handle.id.clone(),
            kind: hit.handle.kind,
        };
    }
    if let Some(node) = hit_node(snapshot, bounds, position) {
        return NodeGraphHit::Node(node.id.clone());
    }
    if let Some(edge) = hit_edge(snapshot, bounds, position) {
        return NodeGraphHit::Edge(edge.id.clone());
    }
    NodeGraphHit::Pane
}

fn hit_edge<N, E>(
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    position: Point,
) -> Option<&Edge<E>> {
    let flow_position = snapshot.viewport.screen_to_flow(bounds, position);
    let flow_radius = EDGE_HIT_RADIUS / snapshot.viewport.zoom.max(0.001);
    let query = Rect::new(
        flow_position.x - flow_radius,
        flow_position.y - flow_radius,
        flow_radius * 2.0,
        flow_radius * 2.0,
    );
    let mut indices = snapshot.spatial.query_edge_indices(query);
    sort_edge_indices(snapshot, &mut indices);
    indices
        .into_iter()
        .rev()
        .filter_map(|index| snapshot.graph.edges.get(index))
        .find(|edge| {
            if edge.hidden || !edge.selectable {
                return false;
            }
            edge_geometry(&snapshot.graph, edge, snapshot.viewport, bounds)
                .is_some_and(|geometry| edge_distance(&geometry, position) <= EDGE_HIT_RADIUS)
        })
}

fn handle_position<N, E>(graph: &GraphModel<N, E>, node: &Node<N>, handle: &Handle) -> Point {
    let bounds = graph_node_bounds(graph, node);
    handle_position_in_bounds(bounds, handle)
}

fn handle_position_in_bounds(bounds: Rect, handle: &Handle) -> Point {
    match handle.position {
        HandlePosition::Left => {
            Point::new(bounds.x(), bounds.y() + bounds.height() * handle.offset)
        }
        HandlePosition::Right => {
            Point::new(bounds.max_x(), bounds.y() + bounds.height() * handle.offset)
        }
        HandlePosition::Top => Point::new(bounds.x() + bounds.width() * handle.offset, bounds.y()),
        HandlePosition::Bottom => {
            Point::new(bounds.x() + bounds.width() * handle.offset, bounds.max_y())
        }
    }
}

fn graph_node_bounds<N, E>(graph: &GraphModel<N, E>, node: &Node<N>) -> Rect {
    graph.node_bounds(node).unwrap_or_else(|| node.bounds())
}

fn resize_handle_points(rect: Rect) -> [(ResizeDirection, Point); 8] {
    let center_x = rect.x() + rect.width() * 0.5;
    let center_y = rect.y() + rect.height() * 0.5;
    [
        (ResizeDirection::NorthWest, rect.origin),
        (ResizeDirection::North, Point::new(center_x, rect.y())),
        (
            ResizeDirection::NorthEast,
            Point::new(rect.max_x(), rect.y()),
        ),
        (ResizeDirection::East, Point::new(rect.max_x(), center_y)),
        (
            ResizeDirection::SouthEast,
            Point::new(rect.max_x(), rect.max_y()),
        ),
        (ResizeDirection::South, Point::new(center_x, rect.max_y())),
        (
            ResizeDirection::SouthWest,
            Point::new(rect.x(), rect.max_y()),
        ),
        (ResizeDirection::West, Point::new(rect.x(), center_y)),
    ]
}

#[allow(clippy::too_many_arguments)]
fn resized_node_geometry(
    original: Rect,
    origin: Point,
    direction: ResizeDirection,
    delta: Vector,
    min_size: Size,
    max_size: Size,
    keep_aspect_ratio: bool,
) -> (Point, Size) {
    let west = matches!(
        direction,
        ResizeDirection::West | ResizeDirection::NorthWest | ResizeDirection::SouthWest
    );
    let east = matches!(
        direction,
        ResizeDirection::East | ResizeDirection::NorthEast | ResizeDirection::SouthEast
    );
    let north = matches!(
        direction,
        ResizeDirection::North | ResizeDirection::NorthEast | ResizeDirection::NorthWest
    );
    let south = matches!(
        direction,
        ResizeDirection::South | ResizeDirection::SouthEast | ResizeDirection::SouthWest
    );
    let mut left = original.x();
    let mut right = original.max_x();
    let mut top = original.y();
    let mut bottom = original.max_y();
    if west {
        left += delta.x;
    }
    if east {
        right += delta.x;
    }
    if north {
        top += delta.y;
    }
    if south {
        bottom += delta.y;
    }

    let min_width = min_size.width.max(1.0);
    let min_height = min_size.height.max(1.0);
    let max_width = max_size.width.max(min_width);
    let max_height = max_size.height.max(min_height);
    let mut width = (right - left).clamp(min_width, max_width);
    let mut height = (bottom - top).clamp(min_height, max_height);
    if keep_aspect_ratio {
        let aspect = original.width() / original.height().max(1.0);
        if matches!(direction, ResizeDirection::North | ResizeDirection::South) {
            width = (height * aspect).clamp(min_width, max_width);
        } else {
            height = (width / aspect.max(0.001)).clamp(min_height, max_height);
        }
    }
    if west {
        left = right - width;
    } else {
        right = left + width;
    }
    if north {
        top = bottom - height;
    } else {
        bottom = top + height;
    }
    let _ = (right, bottom);
    (
        Point::new(left + width * origin.x, top + height * origin.y),
        Size::new(width, height),
    )
}

fn endpoint<N, E>(
    graph: &GraphModel<N, E>,
    node: &Node<N>,
    requested: Option<&HandleId>,
    kind: HandleKind,
) -> (Point, HandlePosition) {
    endpoint_in_bounds(node, graph_node_bounds(graph, node), requested, kind)
}

fn endpoint_in_bounds<N>(
    node: &Node<N>,
    bounds: Rect,
    requested: Option<&HandleId>,
    kind: HandleKind,
) -> (Point, HandlePosition) {
    let handle = requested
        .and_then(|id| node.handle_by_id(id, kind))
        .or_else(|| node.first_handle(kind));
    handle.map_or_else(
        || match kind {
            HandleKind::Source => (
                Point::new(bounds.max_x(), bounds.y() + bounds.height() * 0.5),
                HandlePosition::Right,
            ),
            HandleKind::Target => (
                Point::new(bounds.x(), bounds.y() + bounds.height() * 0.5),
                HandlePosition::Left,
            ),
        },
        |handle| (handle_position_in_bounds(bounds, handle), handle.position),
    )
}

fn edge_geometry<N, E>(
    graph: &GraphModel<N, E>,
    edge: &Edge<E>,
    viewport: Viewport,
    bounds: Rect,
) -> Option<EdgeGeometry> {
    let source_node = graph.node(&edge.source)?;
    let target_node = graph.node(&edge.target)?;
    if source_node.hidden || target_node.hidden {
        return None;
    }
    let (source, source_side) = endpoint(
        graph,
        source_node,
        edge.source_handle.as_ref(),
        HandleKind::Source,
    );
    let (target, target_side) = endpoint(
        graph,
        target_node,
        edge.target_handle.as_ref(),
        HandleKind::Target,
    );
    Some(make_edge_geometry(
        viewport.flow_to_screen(bounds, source),
        source_side,
        viewport.flow_to_screen(bounds, target),
        target_side,
        edge.kind,
        edge.path_options,
    ))
}

fn edge_geometry_from_lookup<N, E>(
    nodes: &HashMap<&NodeId, (&Node<N>, Rect)>,
    edge: &Edge<E>,
) -> Option<EdgeGeometry> {
    let (source_node, source_bounds) = *nodes.get(&edge.source)?;
    let (target_node, target_bounds) = *nodes.get(&edge.target)?;
    if source_node.hidden || target_node.hidden {
        return None;
    }
    let (source, source_side) = endpoint_in_bounds(
        source_node,
        source_bounds,
        edge.source_handle.as_ref(),
        HandleKind::Source,
    );
    let (target, target_side) = endpoint_in_bounds(
        target_node,
        target_bounds,
        edge.target_handle.as_ref(),
        HandleKind::Target,
    );
    Some(make_edge_geometry(
        source,
        source_side,
        target,
        target_side,
        edge.kind,
        edge.path_options,
    ))
}

fn make_edge_geometry(
    source: Point,
    source_side: HandlePosition,
    target: Point,
    target_side: HandlePosition,
    kind: EdgeKind,
    options: EdgePathOptions,
) -> EdgeGeometry {
    let source_direction = side_direction(source_side);
    let target_direction = side_direction(target_side);
    let distance = vector_length(target - source);
    let curvature = if kind == EdgeKind::SimpleBezier {
        options.curvature * 0.6
    } else {
        options.curvature
    };
    let bend = (distance * curvature.clamp(0.0, 1.5)).clamp(24.0, 240.0);
    let control_1 = source + scale_vector(source_direction, bend);
    let control_2 = target + scale_vector(target_direction, bend);
    let mut builder = Path::builder();
    builder.move_to(source);
    let (midpoint, source_tangent, target_tangent) = match kind {
        EdgeKind::Straight => {
            builder.line_to(target);
            (
                lerp_point(source, target, 0.5),
                target - source,
                target - source,
            )
        }
        EdgeKind::Step | EdgeKind::SmoothStep => {
            let horizontal_source =
                matches!(source_side, HandlePosition::Left | HandlePosition::Right);
            if horizontal_source {
                let mid_x = ((source.x + target.x) * 0.5)
                    + (source_direction.x * options.step_offset.max(0.0) * 0.25);
                let points = [
                    source,
                    Point::new(mid_x, source.y),
                    Point::new(mid_x, target.y),
                    target,
                ];
                if kind == EdgeKind::SmoothStep {
                    append_rounded_polyline(&mut builder, &points, options.border_radius);
                } else {
                    for point in points.into_iter().skip(1) {
                        builder.line_to(point);
                    }
                }
                (
                    Point::new(mid_x, (source.y + target.y) * 0.5),
                    Point::new(mid_x, source.y) - source,
                    target - Point::new(mid_x, target.y),
                )
            } else {
                let mid_y = ((source.y + target.y) * 0.5)
                    + (source_direction.y * options.step_offset.max(0.0) * 0.25);
                let points = [
                    source,
                    Point::new(source.x, mid_y),
                    Point::new(target.x, mid_y),
                    target,
                ];
                if kind == EdgeKind::SmoothStep {
                    append_rounded_polyline(&mut builder, &points, options.border_radius);
                } else {
                    for point in points.into_iter().skip(1) {
                        builder.line_to(point);
                    }
                }
                (
                    Point::new((source.x + target.x) * 0.5, mid_y),
                    Point::new(source.x, mid_y) - source,
                    target - Point::new(target.x, mid_y),
                )
            }
        }
        EdgeKind::Bezier | EdgeKind::SimpleBezier => {
            builder.cubic_to(control_1, control_2, target);
            (
                cubic_point(source, control_1, control_2, target, 0.5),
                control_1 - source,
                target - control_2,
            )
        }
    };
    EdgeGeometry {
        path: builder.build(),
        source,
        target,
        control_1,
        control_2,
        midpoint,
        source_tangent,
        target_tangent,
        source_side,
        target_side,
        kind,
    }
}

fn append_rounded_polyline(builder: &mut sui_core::PathBuilder, points: &[Point], radius: f32) {
    if points.len() < 2 {
        return;
    }
    let radius = radius.max(0.0);
    for index in 1..points.len() - 1 {
        let previous = points[index - 1];
        let corner = points[index];
        let next = points[index + 1];
        let incoming = corner - previous;
        let outgoing = next - corner;
        let incoming_length = vector_length(incoming);
        let outgoing_length = vector_length(outgoing);
        let local_radius = radius.min(incoming_length * 0.5).min(outgoing_length * 0.5);
        let before =
            corner + scale_vector(incoming, -local_radius / incoming_length.max(f32::EPSILON));
        let after =
            corner + scale_vector(outgoing, local_radius / outgoing_length.max(f32::EPSILON));
        builder.line_to(before).quad_to(corner, after);
    }
    builder.line_to(*points.last().expect("polyline has points"));
}

fn side_direction(side: HandlePosition) -> Vector {
    match side {
        HandlePosition::Left => Vector::new(-1.0, 0.0),
        HandlePosition::Right => Vector::new(1.0, 0.0),
        HandlePosition::Top => Vector::new(0.0, -1.0),
        HandlePosition::Bottom => Vector::new(0.0, 1.0),
    }
}

fn edge_distance(geometry: &EdgeGeometry, point: Point) -> f32 {
    match geometry.kind {
        EdgeKind::Straight => point_segment_distance(point, geometry.source, geometry.target),
        EdgeKind::Step | EdgeKind::SmoothStep => {
            let horizontal = (geometry.target_tangent.x).abs() > 0.0;
            if horizontal {
                let mid_x = geometry.midpoint.x;
                point_segment_distance(point, geometry.source, Point::new(mid_x, geometry.source.y))
                    .min(point_segment_distance(
                        point,
                        Point::new(mid_x, geometry.source.y),
                        Point::new(mid_x, geometry.target.y),
                    ))
                    .min(point_segment_distance(
                        point,
                        Point::new(mid_x, geometry.target.y),
                        geometry.target,
                    ))
            } else {
                let mid_y = geometry.midpoint.y;
                point_segment_distance(point, geometry.source, Point::new(geometry.source.x, mid_y))
                    .min(point_segment_distance(
                        point,
                        Point::new(geometry.source.x, mid_y),
                        Point::new(geometry.target.x, mid_y),
                    ))
                    .min(point_segment_distance(
                        point,
                        Point::new(geometry.target.x, mid_y),
                        geometry.target,
                    ))
            }
        }
        EdgeKind::Bezier | EdgeKind::SimpleBezier => {
            let mut distance = f32::INFINITY;
            let mut previous = geometry.source;
            for step in 1..=EDGE_HIT_STEPS {
                let current = cubic_point(
                    geometry.source,
                    geometry.control_1,
                    geometry.control_2,
                    geometry.target,
                    step as f32 / EDGE_HIT_STEPS as f32,
                );
                distance = distance.min(point_segment_distance(point, previous, current));
                previous = current;
            }
            distance
        }
    }
}

fn edge_point_at(geometry: &EdgeGeometry, t: f32) -> Point {
    let t = t.clamp(0.0, 1.0);
    match geometry.kind {
        EdgeKind::Straight => lerp_point(geometry.source, geometry.target, t),
        EdgeKind::Bezier | EdgeKind::SimpleBezier => cubic_point(
            geometry.source,
            geometry.control_1,
            geometry.control_2,
            geometry.target,
            t,
        ),
        EdgeKind::Step | EdgeKind::SmoothStep => {
            let horizontal = geometry.target_tangent.x.abs() > 0.0;
            let (first, second) = if horizontal {
                (
                    Point::new(geometry.midpoint.x, geometry.source.y),
                    Point::new(geometry.midpoint.x, geometry.target.y),
                )
            } else {
                (
                    Point::new(geometry.source.x, geometry.midpoint.y),
                    Point::new(geometry.target.x, geometry.midpoint.y),
                )
            };
            if t < 1.0 / 3.0 {
                lerp_point(geometry.source, first, t * 3.0)
            } else if t < 2.0 / 3.0 {
                lerp_point(first, second, (t - 1.0 / 3.0) * 3.0)
            } else {
                lerp_point(second, geometry.target, (t - 2.0 / 3.0) * 3.0)
            }
        }
    }
}

struct EdgePaintOptions<'a, E> {
    appearance: ResolvedAppearance,
    hovered_edge: Option<&'a EdgeId>,
    theme: &'a DefaultTheme,
    painter: Option<&'a EdgePaintFn<E>>,
    edges_reconnectable: bool,
    focused_edge: Option<&'a EdgeId>,
    animation_time: f32,
}

fn edge_is_retained_world_candidate<E>(
    edge: &Edge<E>,
    hovered_edge: Option<&EdgeId>,
    focused_edge: Option<&EdgeId>,
) -> bool {
    !edge.hidden
        && !edge.selected
        && !edge.animated
        && edge.label.is_none()
        && hovered_edge != Some(&edge.id)
        && focused_edge != Some(&edge.id)
}

fn paint_edges<N, E>(
    ctx: &mut PaintCtx,
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    indices: &[usize],
    retained_edges: Option<&HashSet<EdgeId>>,
    options: EdgePaintOptions<'_, E>,
) {
    let EdgePaintOptions {
        appearance,
        hovered_edge,
        theme,
        painter,
        edges_reconnectable,
        focused_edge,
        animation_time,
    } = options;
    for index in indices {
        let Some(edge) = snapshot.graph.edges.get(*index) else {
            continue;
        };
        if edge.hidden {
            continue;
        }
        if retained_edges.is_some_and(|retained| retained.contains(&edge.id)) {
            continue;
        }
        let Some(geometry) = edge_geometry(&snapshot.graph, edge, snapshot.viewport, bounds) else {
            continue;
        };
        let hovered = hovered_edge == Some(&edge.id);
        let focused = focused_edge == Some(&edge.id);
        if let Some(painter) = painter {
            painter(
                ctx,
                edge,
                EdgePaintContext {
                    path: geometry.path.clone(),
                    source: geometry.source,
                    target: geometry.target,
                    midpoint: geometry.midpoint,
                    viewport: snapshot.viewport,
                    hovered,
                },
            );
            continue;
        }
        let color = if edge.selected || focused {
            appearance.edge_selected
        } else if hovered {
            appearance.edge_selected.with_alpha(0.72)
        } else {
            appearance.edge
        };
        let width = if edge.selected || focused {
            2.5
        } else if hovered {
            2.0
        } else {
            1.5
        };
        ctx.stroke(geometry.path.clone(), color, StrokeStyle::new(width));
        if edge.animated {
            for offset in [0.0_f32, 0.333, 0.666] {
                let t = (animation_time * edge.animation_speed + offset).rem_euclid(1.0);
                ctx.fill(Path::circle(edge_point_at(&geometry, t), 2.4), color);
            }
        }
        if edge.selected && edges_reconnectable {
            if edge.reconnectable.allows(HandleKind::Source) {
                ctx.fill(Path::circle(geometry.source, 5.5), appearance.background);
                ctx.stroke(
                    Path::circle(geometry.source, 5.5),
                    appearance.edge_selected,
                    StrokeStyle::new(1.5),
                );
            }
            if edge.reconnectable.allows(HandleKind::Target) {
                ctx.fill(Path::circle(geometry.target, 5.5), appearance.background);
                ctx.stroke(
                    Path::circle(geometry.target, 5.5),
                    appearance.edge_selected,
                    StrokeStyle::new(1.5),
                );
            }
        }
        if let Some(marker) = edge.marker_start {
            paint_marker(
                ctx,
                geometry.source,
                scale_vector(geometry.source_tangent, -1.0),
                marker,
                color,
            );
        }
        if let Some(marker) = edge.marker_end {
            paint_marker(ctx, geometry.target, geometry.target_tangent, marker, color);
        }
        if let Some(label) = &edge.label {
            let text_style = TextStyle {
                font_size: theme.text.xs.size,
                line_height: theme.text.xs.line_height,
                color: theme.palette.text_muted,
                ..theme.body_text_style()
            };
            let measured = ctx
                .measure_text(label.clone(), text_style.clone())
                .ok()
                .map(|measurement| measurement.bounds.size)
                .unwrap_or(Size::new(label.len() as f32 * 7.0, 16.0));
            let label_rect = Rect::new(
                geometry.midpoint.x - (measured.width * 0.5) - 5.0,
                geometry.midpoint.y - (measured.height * 0.5) - 2.0,
                measured.width + 10.0,
                measured.height + 4.0,
            );
            ctx.fill(
                Path::rounded_rect(label_rect, 4.0),
                appearance.background.with_alpha(0.94),
            );
            ctx.draw_text(
                Rect::new(
                    label_rect.x() + 5.0,
                    label_rect.y() + 2.0,
                    measured.width,
                    measured.height,
                ),
                label.clone(),
                text_style,
            );
        }
    }
}

fn paint_connection(
    ctx: &mut PaintCtx,
    interaction: Option<&Interaction>,
    appearance: ResolvedAppearance,
    kind: EdgeKind,
) {
    let geometry = match interaction {
        Some(Interaction::Connect {
            source_position,
            source_side,
            current,
            ..
        }) => make_edge_geometry(
            *source_position,
            *source_side,
            *current,
            opposite_side(*source_side),
            kind,
            EdgePathOptions::default(),
        ),
        Some(Interaction::ReconnectEdge {
            endpoint,
            fixed_position,
            fixed_side,
            current,
            ..
        }) => match endpoint {
            HandleKind::Source => make_edge_geometry(
                *current,
                opposite_side(*fixed_side),
                *fixed_position,
                *fixed_side,
                kind,
                EdgePathOptions::default(),
            ),
            HandleKind::Target => make_edge_geometry(
                *fixed_position,
                *fixed_side,
                *current,
                opposite_side(*fixed_side),
                kind,
                EdgePathOptions::default(),
            ),
        },
        _ => return,
    };
    ctx.stroke(
        geometry.path,
        appearance.edge_selected.with_alpha(0.86),
        StrokeStyle::new(2.0),
    );
}

struct NodePaintOptions<'a, N> {
    appearance: ResolvedAppearance,
    hovered_node: Option<&'a NodeId>,
    theme: &'a DefaultTheme,
    painter: Option<&'a NodePaintFn<N>>,
    custom_nodes: &'a RetainedNodeWidgets<N>,
    registry: &'a NodeWidgetRegistry<N>,
}

fn paint_nodes<N, E>(
    ctx: &mut PaintCtx,
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    indices: &[usize],
    options: NodePaintOptions<'_, N>,
) where
    N: Clone + PartialEq + 'static,
{
    let NodePaintOptions {
        appearance,
        hovered_node,
        theme,
        painter,
        custom_nodes,
        registry,
    } = options;
    let label_style = TextStyle {
        font_size: theme.text.sm.size,
        line_height: theme.text.sm.line_height,
        color: appearance.node_text,
        ..theme.body_text_style()
    };
    let canvas_viewport = snapshot.viewport.to_canvas(bounds.size);
    let shared_transform = canvas_viewport.transform(bounds, Point::ZERO);
    let mut cursor = 0;
    while cursor < indices.len() {
        let uniform_custom = indices
            .get(cursor)
            .and_then(|index| snapshot.graph.nodes.get(*index))
            .is_some_and(|node| {
                custom_nodes.contains(&node.id) && registry.zoom_behavior(&node.kind).is_uniform()
            });
        if uniform_custom {
            let start = cursor;
            while indices
                .get(cursor)
                .and_then(|index| snapshot.graph.nodes.get(*index))
                .is_some_and(|node| {
                    custom_nodes.contains(&node.id)
                        && registry.zoom_behavior(&node.kind).is_uniform()
                })
            {
                cursor += 1;
            }
            ctx.with_transform(shared_transform, |ctx| {
                for index in &indices[start..cursor] {
                    let Some(node) = snapshot.graph.nodes.get(*index) else {
                        continue;
                    };
                    let rect = graph_node_bounds(&snapshot.graph, node);
                    paint_default_node_body(
                        ctx,
                        node,
                        rect,
                        snapshot.viewport,
                        snapshot.viewport.zoom,
                        appearance,
                        theme,
                        hovered_node == Some(&node.id),
                        false,
                        &label_style,
                    );
                    custom_nodes.paint_node(ctx, &node.id);
                }
            });
            continue;
        }

        let Some(index) = indices.get(cursor) else {
            break;
        };
        cursor += 1;
        let Some(node) = snapshot.graph.nodes.get(*index) else {
            continue;
        };
        if node.hidden {
            continue;
        }
        let rect = snapshot
            .viewport
            .flow_rect_to_screen(bounds, graph_node_bounds(&snapshot.graph, node));
        if rect.intersection(bounds).is_none() {
            continue;
        }
        let hovered = hovered_node == Some(&node.id);
        if custom_nodes.contains(&node.id) {
            paint_default_node_body(
                ctx,
                node,
                rect,
                snapshot.viewport,
                1.0,
                appearance,
                theme,
                hovered,
                false,
                &label_style,
            );
            custom_nodes.paint_node(ctx, &node.id);
        } else if let Some(painter) = painter {
            painter(
                ctx,
                node,
                NodePaintContext {
                    bounds: rect,
                    viewport: snapshot.viewport,
                    hovered,
                },
            );
        } else {
            paint_default_node_body(
                ctx,
                node,
                rect,
                snapshot.viewport,
                1.0,
                appearance,
                theme,
                hovered,
                true,
                &label_style,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_default_node_body<N>(
    ctx: &mut PaintCtx,
    node: &Node<N>,
    rect: Rect,
    viewport: Viewport,
    command_scale: f32,
    appearance: ResolvedAppearance,
    theme: &DefaultTheme,
    hovered: bool,
    paint_label: bool,
    label_style: &TextStyle,
) {
    let fill = if hovered {
        appearance.node_hovered
    } else {
        appearance.node
    };
    let command_scale = command_scale.max(0.001);
    let corner_radius = (theme.metrics.corner_radius * viewport.zoom)
        .clamp(3.0, theme.metrics.corner_radius * 1.5)
        / command_scale;
    let border = if node.selected {
        appearance.selection
    } else {
        appearance.node_border
    };
    ctx.fill_rrect_bordered(
        rect,
        [corner_radius; 4],
        fill,
        Border {
            width: (if node.selected { 2.25 } else { 1.0 }) / command_scale,
            color: border,
        },
    );

    if paint_label && rect.width() >= 28.0 && rect.height() >= 18.0 {
        let padding = (12.0 * viewport.zoom).clamp(6.0, 14.0);
        ctx.push_clip_rect(rect);
        ctx.draw_text(
            Rect::new(
                rect.x() + padding,
                rect.y() + padding,
                (rect.width() - (padding * 2.0)).max(1.0),
                (rect.height() - (padding * 2.0)).max(1.0),
            ),
            node.label.clone(),
            label_style.clone(),
        );
        ctx.pop_clip();
    }
}

struct NodeOverlayOptions<'a> {
    appearance: ResolvedAppearance,
    hovered_handle: Option<&'a (NodeId, HandleId, HandleKind)>,
    nodes_resizable: bool,
    focused_node: Option<&'a NodeId>,
    indices: &'a [usize],
}

fn paint_node_overlays<N, E>(
    ctx: &mut PaintCtx,
    snapshot: &GraphSnapshot<N, E>,
    bounds: Rect,
    options: NodeOverlayOptions<'_>,
) {
    let NodeOverlayOptions {
        appearance,
        hovered_handle,
        nodes_resizable,
        focused_node,
        indices,
    } = options;
    for index in indices {
        let Some(node) = snapshot.graph.nodes.get(*index) else {
            continue;
        };
        let rect = snapshot
            .viewport
            .flow_rect_to_screen(bounds, graph_node_bounds(&snapshot.graph, node));
        if rect.intersection(bounds).is_none() {
            continue;
        }
        if node.selected || focused_node == Some(&node.id) {
            ctx.stroke(
                Path::rounded_rect(rect, 5.0),
                appearance.selection,
                StrokeStyle::new(2.25),
            );
            if nodes_resizable && node.resizable {
                for (_, point) in resize_handle_points(rect) {
                    let handle = Rect::new(point.x - 3.5, point.y - 3.5, 7.0, 7.0);
                    ctx.fill_rrect_bordered(
                        handle,
                        [0.0; 4],
                        appearance.background,
                        Border {
                            width: 1.5,
                            color: appearance.selection,
                        },
                    );
                }
            }
        }
        for handle in &node.handles {
            let position = snapshot
                .viewport
                .flow_to_screen(bounds, handle_position(&snapshot.graph, node, handle));
            let radius = handle_paint_radius(snapshot.viewport.zoom);
            let hovered = hovered_handle.is_some_and(|(node_id, handle_id, kind)| {
                node_id == &node.id && handle_id == &handle.id && *kind == handle.kind
            });
            let color = match handle.kind {
                HandleKind::Source => appearance.source_handle,
                HandleKind::Target => appearance.target_handle,
            };
            if hovered {
                ctx.fill(
                    Path::circle(position, radius + 3.0),
                    appearance.selection.with_alpha(0.22),
                );
            }
            ctx.fill_rrect_bordered(
                Rect::new(
                    position.x - radius,
                    position.y - radius,
                    radius * 2.0,
                    radius * 2.0,
                ),
                [radius; 4],
                if handle.connectable {
                    color
                } else {
                    color.with_alpha(0.38)
                },
                Border {
                    width: 1.25,
                    color: appearance.background,
                },
            );
        }
    }
}

fn paint_marker(
    ctx: &mut PaintCtx,
    target: Point,
    tangent: Vector,
    marker: EdgeMarker,
    color: Color,
) {
    let length = vector_length(tangent);
    if length <= 0.001 {
        return;
    }
    let direction = Vector::new(tangent.x / length, tangent.y / length);
    let perpendicular = Vector::new(-direction.y, direction.x);
    let base = target + scale_vector(direction, -9.0);
    match marker {
        EdgeMarker::Circle => ctx.fill(Path::circle(target, 4.5), color),
        EdgeMarker::Arrow | EdgeMarker::ArrowClosed => {
            let mut builder = Path::builder();
            builder
                .move_to(base + scale_vector(perpendicular, 4.5))
                .line_to(target)
                .line_to(base + scale_vector(perpendicular, -4.5));
            if marker == EdgeMarker::ArrowClosed {
                builder.close();
                ctx.fill(builder.build(), color);
            } else {
                ctx.stroke(builder.build(), color, StrokeStyle::new(1.5));
            }
        }
    }
}

fn append_marker_scene(
    scene: &mut Scene,
    target: Point,
    tangent: Vector,
    marker: EdgeMarker,
    color: Color,
) {
    let length = vector_length(tangent);
    if length <= 0.001 {
        return;
    }
    let direction = Vector::new(tangent.x / length, tangent.y / length);
    let perpendicular = Vector::new(-direction.y, direction.x);
    let base = target + scale_vector(direction, -9.0);
    match marker {
        EdgeMarker::Circle => scene.push(SceneCommand::FillPath {
            path: Path::circle(target, 4.5),
            brush: Brush::Solid(color),
        }),
        EdgeMarker::Arrow | EdgeMarker::ArrowClosed => {
            let mut builder = Path::builder();
            builder
                .move_to(base + scale_vector(perpendicular, 4.5))
                .line_to(target)
                .line_to(base + scale_vector(perpendicular, -4.5));
            if marker == EdgeMarker::ArrowClosed {
                builder.close();
                scene.push(SceneCommand::FillPath {
                    path: builder.build(),
                    brush: Brush::Solid(color),
                });
            } else {
                scene.push(SceneCommand::StrokePath {
                    path: builder.build(),
                    brush: Brush::Solid(color),
                    stroke: StrokeStyle::new(1.5),
                });
            }
        }
    }
}

fn cubic_point(from: Point, control_1: Point, control_2: Point, to: Point, t: f32) -> Point {
    let t = t.clamp(0.0, 1.0);
    let inverse = 1.0 - t;
    Point::new(
        (inverse.powi(3) * from.x)
            + (3.0 * inverse.powi(2) * t * control_1.x)
            + (3.0 * inverse * t.powi(2) * control_2.x)
            + (t.powi(3) * to.x),
        (inverse.powi(3) * from.y)
            + (3.0 * inverse.powi(2) * t * control_1.y)
            + (3.0 * inverse * t.powi(2) * control_2.y)
            + (t.powi(3) * to.y),
    )
}

fn point_segment_distance(point: Point, start: Point, end: Point) -> f32 {
    let segment = end - start;
    let length_squared = (segment.x * segment.x) + (segment.y * segment.y);
    if length_squared <= f32::EPSILON {
        return vector_length(point - start);
    }
    let relative = point - start;
    let t = ((relative.x * segment.x) + (relative.y * segment.y)) / length_squared;
    let closest = start + scale_vector(segment, t.clamp(0.0, 1.0));
    vector_length(point - closest)
}

fn scroll_offset(delta: Option<ScrollDelta>, fallback: Vector) -> Vector {
    match delta {
        Some(ScrollDelta::Lines(delta)) => Vector::new(delta.x * 48.0, delta.y * 48.0),
        Some(ScrollDelta::Pixels(delta)) => delta,
        None => fallback,
    }
}

fn screen_rect_from_points(first: Point, second: Point) -> Rect {
    Rect::from_points(
        Point::new(first.x.min(second.x), first.y.min(second.y)),
        Point::new(first.x.max(second.x), first.y.max(second.y)),
    )
}

fn auto_pan_delta(bounds: Rect, position: Point, margin: f32, speed: f32) -> Option<Vector> {
    let margin = margin.max(1.0);
    let axis = |position: f32, min: f32, max: f32| {
        if position < min + margin {
            speed * (1.0 - ((position - min).max(0.0) / margin))
        } else if position > max - margin {
            -speed * (1.0 - ((max - position).max(0.0) / margin))
        } else {
            0.0
        }
    };
    let delta = Vector::new(
        axis(position.x, bounds.x(), bounds.max_x()),
        axis(position.y, bounds.y(), bounds.max_y()),
    );
    (delta != Vector::ZERO).then_some(delta)
}

fn rect_contains_rect(outer: Rect, inner: Rect) -> bool {
    outer.contains(inner.origin) && outer.contains(Point::new(inner.max_x(), inner.max_y()))
}

fn segment_intersects_rect(start: Point, end: Point, rect: Rect) -> bool {
    if rect.contains(start) || rect.contains(end) {
        return true;
    }
    let top_left = rect.origin;
    let top_right = Point::new(rect.max_x(), rect.y());
    let bottom_right = Point::new(rect.max_x(), rect.max_y());
    let bottom_left = Point::new(rect.x(), rect.max_y());
    segments_intersect(start, end, top_left, top_right)
        || segments_intersect(start, end, top_right, bottom_right)
        || segments_intersect(start, end, bottom_right, bottom_left)
        || segments_intersect(start, end, bottom_left, top_left)
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ab = b - a;
    let cd = d - c;
    let denominator = cross(ab, cd);
    if denominator.abs() <= f32::EPSILON {
        return false;
    }
    let offset = c - a;
    let t = cross(offset, cd) / denominator;
    let u = cross(offset, ab) / denominator;
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

fn cross(first: Vector, second: Vector) -> f32 {
    (first.x * second.y) - (first.y * second.x)
}

fn snap_point(point: Point, grid: Size) -> Point {
    Point::new(
        (point.x / grid.width).round() * grid.width,
        (point.y / grid.height).round() * grid.height,
    )
}

fn handle_paint_radius(zoom: f32) -> f32 {
    (5.0 * zoom).clamp(3.5, 8.0)
}

fn handle_hit_radius(zoom: f32) -> f32 {
    handle_paint_radius(zoom) + 5.0
}

fn opposite_side(side: HandlePosition) -> HandlePosition {
    match side {
        HandlePosition::Left => HandlePosition::Right,
        HandlePosition::Right => HandlePosition::Left,
        HandlePosition::Top => HandlePosition::Bottom,
        HandlePosition::Bottom => HandlePosition::Top,
    }
}

fn center(rect: Rect) -> Point {
    Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    )
}

fn lerp_point(from: Point, to: Point, t: f32) -> Point {
    Point::new(
        from.x + ((to.x - from.x) * t),
        from.y + ((to.y - from.y) * t),
    )
}

fn scale_vector(vector: Vector, factor: f32) -> Vector {
    Vector::new(vector.x * factor, vector.y * factor)
}

fn midpoint(first: Point, second: Point) -> Point {
    Point::new(
        first.x + ((second.x - first.x) * 0.5),
        first.y + ((second.y - first.y) * 0.5),
    )
}

fn vector_length(vector: Vector) -> f32 {
    ((vector.x * vector.x) + (vector.y * vector.y)).sqrt()
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use super::*;
    use sui_core::{PointerButtons, PointerEvent};
    use sui_runtime::{Application, Runtime, WindowBuilder};

    use crate::NodeSignal;

    struct RetainedTestNode {
        node: NodeSignal<()>,
    }

    struct InteractiveTestNode {
        presses: Rc<Cell<u32>>,
    }

    struct PaintOrderTestNode {
        name: &'static str,
        order: Rc<RefCell<Vec<&'static str>>>,
    }

    struct MeasureCountingNode {
        measures: Rc<Cell<u32>>,
    }

    struct LifecycleCountingNode {
        arranges: Rc<Cell<u32>>,
        paints: Rc<Cell<u32>>,
        semantics: Rc<Cell<u32>>,
    }

    impl Widget for InteractiveTestNode {
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Pointer(pointer) = event
                && pointer.kind == PointerEventKind::Down
                && pointer.button == Some(PointerButton::Primary)
            {
                self.presses.set(self.presses.get() + 1);
                ctx.set_handled();
            }
        }

        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(180.0, 72.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.22, 0.32, 0.48, 1.0));
        }
    }

    impl Widget for PaintOrderTestNode {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 64.0))
        }

        fn paint(&self, _ctx: &mut PaintCtx) {
            self.order.borrow_mut().push(self.name);
        }
    }

    impl Widget for MeasureCountingNode {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            self.measures.set(self.measures.get().saturating_add(1));
            constraints.clamp(Size::new(120.0, 64.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.2, 0.3, 0.4, 1.0));
        }
    }

    impl Widget for LifecycleCountingNode {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(120.0, 64.0))
        }

        fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {
            self.arranges.set(self.arranges.get().saturating_add(1));
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            self.paints.set(self.paints.get().saturating_add(1));
            ctx.fill_bounds(Color::rgba(0.2, 0.3, 0.4, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            self.semantics.set(self.semantics.get().saturating_add(1));
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some("lifecycle widget".to_string());
            ctx.push(node);
        }
    }

    impl Widget for RetainedTestNode {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let node = ctx.observe(&self.node);
            constraints.clamp(Size::new(
                48.0 + (node.label.chars().count() as f32 * 8.0),
                84.0,
            ))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.18, 0.28, 0.42, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let node = ctx.observe(&self.node);
            let mut semantics =
                SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
            semantics.name = Some(node.label);
            semantics.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
            ctx.push(semantics);
        }

        fn accepts_focus(&self) -> bool {
            true
        }
    }

    fn graph() -> GraphModel<(), ()> {
        GraphModel::new(
            vec![
                Node::new("source", Point::new(20.0, 40.0), ()),
                Node::new("target", Point::new(360.0, 180.0), ()),
            ],
            vec![Edge::new("edge", "source", "target", ())],
        )
        .unwrap()
    }

    fn build_runtime(state: NodeGraphState<(), ()>) -> (Runtime, sui_core::WindowId) {
        build_runtime_with_graph(NodeGraph::new("Graph", state))
    }

    fn build_runtime_with_widget<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Node graph").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn build_runtime_with_graph(graph: NodeGraph<(), ()>) -> (Runtime, sui_core::WindowId) {
        build_runtime_with_widget(graph)
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }
        let mut pointer = PointerEvent::new(kind, position);
        pointer.pointer_id = 7;
        pointer.button = Some(PointerButton::Primary);
        pointer.buttons = buttons;
        Event::Pointer(pointer)
    }

    fn touch_pointer(
        pointer_id: u64,
        kind: PointerEventKind,
        position: Point,
        pressed: bool,
    ) -> Event {
        let mut pointer = match primary_pointer(kind, position, pressed) {
            Event::Pointer(pointer) => pointer,
            _ => unreachable!(),
        };
        pointer.pointer_id = pointer_id;
        pointer.pointer_kind = PointerKind::Touch;
        Event::Pointer(pointer)
    }

    #[test]
    fn default_navigation_zooms_in_place_instead_of_wheel_panning() {
        let config = NodeGraphConfig::default();

        assert!(config.zoom_on_scroll);
        assert!(config.zoom_on_pinch);
        assert!(!config.pan_on_scroll);
    }

    #[test]
    fn default_wheel_zoom_keeps_pointer_flow_position_anchored() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::default();
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let pointer_position = Point::new(bounds.x() + 180.0, bounds.y() + 140.0);
        let before_viewport = state.viewport();
        let anchored_flow = before_viewport.screen_to_flow(bounds, pointer_position);
        let mut wheel = PointerEvent::new(PointerEventKind::Scroll, pointer_position);
        wheel.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, 120.0)));

        runtime.handle_event(window_id, Event::Pointer(wheel))?;

        let after_viewport = state.viewport();
        let after_flow = after_viewport.screen_to_flow(bounds, pointer_position);
        assert!(after_viewport.zoom > before_viewport.zoom);
        assert!(vector_length(after_flow - anchored_flow) < 0.001);
        Ok(())
    }

    #[test]
    fn touch_pinch_zooms_around_the_gesture_centroid() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::default();
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let center = Point::new(bounds.x() + 300.0, bounds.y() + 220.0);
        let first = Point::new(center.x - 50.0, center.y);
        let second = Point::new(center.x + 50.0, center.y);
        let anchored_flow = state.viewport().screen_to_flow(bounds, center);

        runtime.handle_event(
            window_id,
            touch_pointer(31, PointerEventKind::Down, first, true),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(47, PointerEventKind::Down, second, true),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(
                31,
                PointerEventKind::Move,
                Point::new(center.x - 100.0, center.y),
                true,
            ),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(
                47,
                PointerEventKind::Move,
                Point::new(center.x + 100.0, center.y),
                true,
            ),
        )?;

        let viewport = state.viewport();
        assert!(
            (viewport.zoom - 2.0).abs() < 0.001,
            "expected 2x pinch zoom, got {viewport:?}"
        );
        assert!(vector_length(viewport.screen_to_flow(bounds, center) - anchored_flow) < 0.001);

        runtime.handle_event(
            window_id,
            touch_pointer(47, PointerEventKind::Up, second, false),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(31, PointerEventKind::Up, first, false),
        )?;
        Ok(())
    }

    #[test]
    fn touch_pinch_preempts_retained_child_pointer_handling() -> sui_core::Result<()> {
        let presses = Rc::new(Cell::new(0));
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("custom", Point::new(40.0, 60.0), ())
                    .kind("interactive")
                    .size(Size::new(300.0, 200.0)),
            ],
            Vec::new(),
        )
        .expect("valid retained graph");
        let counter = Rc::clone(&presses);
        let graph =
            NodeGraph::new("Graph", state.clone()).node_type("interactive", move |_id, _node| {
                InteractiveTestNode {
                    presses: Rc::clone(&counter),
                }
            });
        let (mut runtime, window_id) = build_runtime_with_graph(graph);
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let viewport = state.viewport();
        let first = viewport.flow_to_screen(bounds, Point::new(100.0, 130.0));
        let second = viewport.flow_to_screen(bounds, Point::new(260.0, 130.0));

        runtime.handle_event(
            window_id,
            touch_pointer(51, PointerEventKind::Down, first, true),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(52, PointerEventKind::Down, second, true),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(
                51,
                PointerEventKind::Move,
                Point::new(first.x - 40.0, first.y),
                true,
            ),
        )?;
        runtime.handle_event(
            window_id,
            touch_pointer(
                52,
                PointerEventKind::Move,
                Point::new(second.x + 40.0, second.y),
                true,
            ),
        )?;

        assert!(state.viewport().zoom > viewport.zoom);
        assert_eq!(presses.get(), 1, "second touch should start the pinch");
        Ok(())
    }

    #[test]
    fn node_hit_testing_respects_viewport_transform_and_z_order() {
        let mut graph = graph();
        graph
            .nodes
            .push(Node::new("front", Point::new(20.0, 40.0), ()).size(Size::new(80.0, 40.0)));
        let bounds = Rect::new(10.0, 20.0, 800.0, 600.0);
        let viewport = Viewport::new(50.0, 30.0, 2.0);
        let position = viewport.flow_to_screen(bounds, Point::new(40.0, 50.0));
        let snapshot = GraphSnapshot::new(graph).viewport(viewport);

        assert_eq!(
            hit_node(&snapshot, bounds, position).map(|node| node.id.clone()),
            Some(NodeId::from("front"))
        );
        assert_eq!(
            node_graph_hit_test(&snapshot, bounds, position),
            NodeGraphHit::Node(NodeId::from("front"))
        );
    }

    #[test]
    fn marquee_adds_intersecting_nodes_without_dropping_existing_selection() {
        let mut graph = graph();
        graph.nodes[1].selected = true;
        let mut changes = ElementChanges::default();

        apply_marquee_selection(
            &mut graph,
            Rect::new(0.0, 0.0, 240.0, 160.0),
            true,
            SelectionMode::Partial,
            &mut changes,
        );

        assert!(graph.nodes[0].selected);
        assert!(graph.nodes[1].selected);
        assert_eq!(changes.nodes.len(), 1);
    }

    #[test]
    fn deleting_a_selected_node_reports_its_incident_edge() {
        let mut graph = graph();
        graph.nodes[0].selected = true;
        let mut changes = ElementChanges::default();

        delete_selected(&mut graph, &mut changes);

        assert!(
            graph
                .nodes
                .iter()
                .all(|node| node.id != NodeId::from("source"))
        );
        assert!(graph.edges.is_empty());
        assert_eq!(changes.nodes.len(), 1);
        assert_eq!(changes.edges.len(), 1);
    }

    #[test]
    fn bezier_hit_distance_is_small_on_the_curve() {
        let geometry = make_edge_geometry(
            Point::new(20.0, 40.0),
            HandlePosition::Right,
            Point::new(320.0, 180.0),
            HandlePosition::Left,
            EdgeKind::Bezier,
            EdgePathOptions::default(),
        );

        assert!(edge_distance(&geometry, geometry.midpoint) < 0.01);
        assert!(edge_distance(&geometry, Point::new(20.0, 300.0)) > 40.0);
    }

    #[test]
    fn smooth_step_uses_rounded_path_corners() {
        let geometry = make_edge_geometry(
            Point::new(20.0, 40.0),
            HandlePosition::Right,
            Point::new(320.0, 180.0),
            HandlePosition::Left,
            EdgeKind::SmoothStep,
            EdgePathOptions {
                border_radius: 12.0,
                ..EdgePathOptions::default()
            },
        );

        assert!(
            geometry
                .path
                .elements()
                .iter()
                .any(|element| matches!(element, sui_core::PathElement::QuadTo { .. }))
        );
    }

    #[test]
    fn auto_pan_points_back_toward_visible_content() {
        let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        assert_eq!(
            auto_pan_delta(bounds, Point::new(2.0, 150.0), 32.0, 12.0),
            Some(Vector::new(11.25, 0.0))
        );
        assert_eq!(
            auto_pan_delta(bounds, Point::new(398.0, 298.0), 32.0, 12.0),
            Some(Vector::new(-11.25, -11.25))
        );
    }

    #[test]
    fn snapping_uses_independent_axis_intervals() {
        assert_eq!(
            snap_point(Point::new(24.0, 39.0), Size::new(10.0, 16.0)),
            Point::new(20.0, 32.0)
        );
    }

    #[test]
    fn runtime_pointer_drag_moves_a_node_in_flow_coordinates() -> sui_core::Result<()> {
        let state = NodeGraphState::from_model(graph());
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let start = state
            .viewport()
            .flow_to_screen(bounds, Point::new(80.0, 70.0));
        let end = start + Vector::new(45.0, 30.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        assert_eq!(
            state
                .graph()
                .node(&NodeId::from("source"))
                .expect("source node")
                .position,
            Point::new(65.0, 70.0)
        );
        Ok(())
    }

    #[test]
    fn runtime_handle_drag_creates_a_valid_edge() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("source", Point::new(20.0, 40.0), ()),
                Node::new("target", Point::new(360.0, 180.0), ()),
            ],
            Vec::new(),
        )
        .expect("valid graph");
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let snapshot = state.snapshot();
        let source = snapshot.graph.node(&NodeId::from("source")).unwrap();
        let source_handle = source.first_handle(HandleKind::Source).unwrap();
        let target = snapshot.graph.node(&NodeId::from("target")).unwrap();
        let target_handle = target.first_handle(HandleKind::Target).unwrap();
        let start = snapshot.viewport.flow_to_screen(
            bounds,
            handle_position(&snapshot.graph, source, source_handle),
        );
        let end = snapshot.viewport.flow_to_screen(
            bounds,
            handle_position(&snapshot.graph, target, target_handle),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let graph = state.graph();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, NodeId::from("source"));
        assert_eq!(graph.edges[0].target, NodeId::from("target"));
        Ok(())
    }

    #[test]
    fn retained_custom_node_keeps_identity_and_publishes_measured_size() -> sui_core::Result<()> {
        let builds = Rc::new(Cell::new(0));
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("custom", Point::new(30.0, 40.0), ())
                    .kind("retained")
                    .label("Retained custom node")
                    .content_sized(Size::new(80.0, 40.0), Size::new(400.0, 200.0)),
            ],
            Vec::new(),
        )
        .expect("valid graph");
        let build_counter = Rc::clone(&builds);
        let graph =
            NodeGraph::new("Graph", state.clone()).node_type("retained", move |_id, node| {
                build_counter.set(build_counter.get() + 1);
                RetainedTestNode { node }
            });
        let (mut runtime, window_id) = build_runtime_with_graph(graph);

        let first = runtime.render(window_id)?;
        let first_semantics = first
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Retained custom node"))
            .expect("custom node semantics");
        let first_id = first_semantics.id;
        assert_eq!(builds.get(), 1);
        assert_eq!(
            state.graph().node(&NodeId::from("custom")).unwrap().size,
            Size::new(208.0, 84.0)
        );

        state.set_node_position(&NodeId::from("custom"), Point::new(180.0, 120.0));
        let second = runtime.render(window_id)?;
        let second_semantics = second
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Retained custom node"))
            .expect("custom node semantics after update");

        assert_eq!(builds.get(), 1);
        assert_eq!(second_semantics.id, first_id);
        assert_ne!(
            second_semantics.bounds.origin,
            first_semantics.bounds.origin
        );
        Ok(())
    }

    #[test]
    fn retained_node_widgets_uniformly_scale_with_canvas_zoom() -> sui_core::Result<()> {
        let make_state = || {
            NodeGraphState::from_snapshot(
                GraphSnapshot::new(
                    GraphModel::new(
                        vec![
                            Node::new("custom", Point::new(30.0, 40.0), ())
                                .kind("retained")
                                .label("Retained custom node")
                                .content_sized(Size::new(80.0, 40.0), Size::new(400.0, 200.0)),
                        ],
                        Vec::new(),
                    )
                    .expect("valid graph"),
                )
                .viewport(Viewport::new(0.0, 0.0, 2.0)),
            )
        };

        let uniform_state = make_state();
        let uniform = NodeGraph::new("Uniform graph", uniform_state.clone())
            .node_type("retained", |_id, node| RetainedTestNode { node });
        let (mut runtime, window_id) = build_runtime_with_graph(uniform);
        let output = runtime.render(window_id)?;
        let uniform_bounds = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Retained custom node")
            })
            .expect("uniform retained semantics")
            .bounds;
        assert_eq!(uniform_bounds.size, Size::new(416.0, 168.0));

        let screen_state = make_state();
        let screen_space = NodeGraph::new("Screen-space graph", screen_state)
            .node_type("retained", |_id, node| RetainedTestNode { node })
            .node_zoom_behavior("retained", CanvasZoomBehavior::ScreenSpace);
        let (mut runtime, window_id) = build_runtime_with_graph(screen_space);
        let output = runtime.render(window_id)?;
        let screen_bounds = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some("Retained custom node")
            })
            .expect("screen-space retained semantics")
            .bounds;
        assert_eq!(screen_bounds.size, Size::new(208.0, 84.0));
        Ok(())
    }

    #[test]
    fn viewport_only_zoom_does_not_remeasure_retained_nodes() -> sui_core::Result<()> {
        let measures = Rc::new(Cell::new(0));
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("measured", Point::new(30.0, 40.0), ())
                    .kind("measured")
                    .size(Size::new(120.0, 64.0)),
            ],
            Vec::new(),
        )
        .expect("valid graph");
        let counter = Rc::clone(&measures);
        let graph = NodeGraph::new("Measured graph", state.clone()).node_type(
            "measured",
            move |_id, _node| MeasureCountingNode {
                measures: Rc::clone(&counter),
            },
        );
        let (mut runtime, window_id) = build_runtime_with_graph(graph);
        runtime.render(window_id)?;
        let initial_measures = measures.get();

        state.set_viewport(Viewport::new(40.0, 30.0, 1.75));
        runtime.render(window_id)?;

        assert_eq!(measures.get(), initial_measures);
        Ok(())
    }

    #[test]
    fn offscreen_retained_nodes_skip_arrange_paint_and_semantics() -> sui_core::Result<()> {
        let arranges = Rc::new(Cell::new(0));
        let paints = Rc::new(Cell::new(0));
        let semantics = Rc::new(Cell::new(0));
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("near", Point::new(30.0, 40.0), ())
                    .kind("counted")
                    .size(Size::new(120.0, 64.0)),
                Node::new("far", Point::new(10_000.0, 40.0), ())
                    .kind("counted")
                    .size(Size::new(120.0, 64.0)),
            ],
            Vec::new(),
        )
        .expect("valid graph");
        let arrange_counter = Rc::clone(&arranges);
        let paint_counter = Rc::clone(&paints);
        let semantics_counter = Rc::clone(&semantics);
        let graph = NodeGraph::new("Culled graph", state.clone()).node_type(
            "counted",
            move |_id, _node| LifecycleCountingNode {
                arranges: Rc::clone(&arrange_counter),
                paints: Rc::clone(&paint_counter),
                semantics: Rc::clone(&semantics_counter),
            },
        );
        let (mut runtime, window_id) = build_runtime_with_graph(graph);

        let initial = runtime.render(window_id)?;
        assert_eq!(arranges.get(), 1);
        assert_eq!(paints.get(), 1);
        assert_eq!(semantics.get(), 1);
        let initial_widget_bounds = initial
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("lifecycle widget"))
            .expect("visible retained widget semantics")
            .bounds;

        state.set_viewport(Viewport::new(0.0, 0.0, 0.9));
        let zoomed = runtime.render(window_id)?;
        assert_eq!(
            arranges.get(),
            1,
            "viewport-only projection should reuse retained child arrangement"
        );
        assert_eq!(paints.get(), 2);
        assert_eq!(semantics.get(), 2);
        let zoomed_widget_bounds = zoomed
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("lifecycle widget"))
            .expect("reprojected retained widget semantics")
            .bounds;
        assert!(
            (zoomed_widget_bounds.width() - (initial_widget_bounds.width() * 0.9)).abs() < 0.01
        );
        assert!(
            (zoomed_widget_bounds.height() - (initial_widget_bounds.height() * 0.9)).abs() < 0.01
        );

        state.set_viewport(Viewport::new(-9_950.0, 0.0, 1.0));
        runtime.render(window_id)?;
        assert_eq!(arranges.get(), 3, "one node leaves and one enters");
        assert_eq!(paints.get(), 3);
        assert_eq!(semantics.get(), 3);
        Ok(())
    }

    #[test]
    fn large_static_edge_world_reuses_flow_space_scene_across_zoom() -> sui_core::Result<()> {
        let edge_count = NodeGraphConfig::default().retained_edge_world_min;
        let nodes = (0..=edge_count)
            .map(|index| {
                Node::new(
                    format!("world-node-{index}"),
                    Point::new(index as f32 * 32.0, (index % 7) as f32 * 48.0),
                    (),
                )
                .size(Size::new(24.0, 20.0))
            })
            .collect::<Vec<_>>();
        let edges = (0..edge_count)
            .map(|index| {
                Edge::new(
                    format!("world-edge-{index}"),
                    format!("world-node-{index}"),
                    format!("world-node-{}", index + 1),
                    (),
                )
            })
            .collect::<Vec<_>>();
        let state = NodeGraphState::new(nodes, edges).expect("valid retained edge world");
        let (mut runtime, window_id) = build_runtime(state.clone());

        let first = runtime.render(window_id)?;
        let mut first_layers = Vec::new();
        first
            .frame
            .scene
            .visit_layers(&mut |layer| first_layers.push(Arc::clone(&layer.scene)));
        let first_world = first_layers
            .into_iter()
            .find(|scene| scene.commands().len() == edge_count * 2)
            .expect("static strokes and markers should occupy one retained world layer");

        state.set_viewport(Viewport::new(-120.0, 48.0, 0.72));
        let second = runtime.render(window_id)?;
        let mut second_layers = Vec::new();
        second
            .frame
            .scene
            .visit_layers(&mut |layer| second_layers.push(Arc::clone(&layer.scene)));
        let second_world = second_layers
            .into_iter()
            .find(|scene| scene.commands().len() == edge_count * 2)
            .expect("retained edge world should survive viewport changes");

        assert!(Arc::ptr_eq(&first_world, &second_world));
        Ok(())
    }

    #[test]
    fn selected_background_node_paints_below_edges_and_descendants() -> sui_core::Result<()> {
        let mut group = Node::new("group", Point::new(20.0, 20.0), ())
            .kind("background")
            .size(Size::new(320.0, 220.0))
            .z_index(-10);
        group.selected = true;
        let child = Node::new("child", Point::new(70.0, 70.0), ())
            .kind("foreground")
            .parent("group");
        let state = NodeGraphState::new(
            vec![group, child],
            vec![Edge::new("edge", "group", "child", ())],
        )
        .expect("valid nested graph");
        let order = Rc::new(RefCell::new(Vec::new()));
        let background_order = Rc::clone(&order);
        let foreground_order = Rc::clone(&order);
        let edge_order = Rc::clone(&order);
        let graph = NodeGraph::new("Graph", state)
            .node_type("background", move |_id, _node| PaintOrderTestNode {
                name: "background",
                order: Rc::clone(&background_order),
            })
            .node_type("foreground", move |_id, _node| PaintOrderTestNode {
                name: "foreground",
                order: Rc::clone(&foreground_order),
            })
            .edge_painter(move |_ctx, _edge, _paint| {
                edge_order.borrow_mut().push("edge");
            });
        let (mut runtime, window_id) = build_runtime_with_graph(graph);

        runtime.render(window_id)?;

        assert_eq!(&*order.borrow(), &["background", "edge", "foreground"]);
        Ok(())
    }

    #[test]
    fn retained_child_control_consumes_pointer_without_dragging_node() -> sui_core::Result<()> {
        let presses = Rc::new(Cell::new(0));
        let state = NodeGraphState::<(), ()>::new(
            vec![Node::new("custom", Point::new(40.0, 60.0), ()).kind("interactive")],
            Vec::new(),
        )
        .unwrap();
        let counter = Rc::clone(&presses);
        let graph =
            NodeGraph::new("Graph", state.clone()).node_type("interactive", move |_id, _node| {
                InteractiveTestNode {
                    presses: Rc::clone(&counter),
                }
            });
        let (mut runtime, window_id) = build_runtime_with_graph(graph);
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .unwrap()
            .bounds;
        let start = state
            .viewport()
            .flow_to_screen(bounds, Point::new(120.0, 90.0));
        let end = start + Vector::new(60.0, 30.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        assert_eq!(presses.get(), 1);
        assert_eq!(
            state.node(&NodeId::from("custom")).unwrap().position,
            Point::new(40.0, 60.0)
        );
        Ok(())
    }

    #[test]
    fn paint_only_surface_ignores_graph_drag_events() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::new(
            vec![Node::new("node", Point::new(40.0, 60.0), ())],
            Vec::new(),
        )
        .expect("valid graph");
        let surface = NodeGraphSurface::new("Paint-only graph", state.clone());
        let (mut runtime, window_id) = build_runtime_with_widget(surface);
        let output = runtime.render(window_id)?;
        let graph_semantics = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("surface semantics");
        let bounds = graph_semantics.bounds;
        let start = state
            .viewport()
            .flow_to_screen(bounds, Point::new(90.0, 90.0));
        let end = start + Vector::new(80.0, 40.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        assert_eq!(
            state.node(&NodeId::from("node")).unwrap().position,
            Point::new(40.0, 60.0)
        );
        assert!(graph_semantics.actions.is_empty());
        Ok(())
    }

    #[test]
    fn retained_widgets_remain_interactive_on_paint_only_surface() -> sui_core::Result<()> {
        let presses = Rc::new(Cell::new(0));
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("custom", Point::new(40.0, 60.0), ())
                    .kind("interactive")
                    .size(Size::new(180.0, 72.0)),
            ],
            Vec::new(),
        )
        .expect("valid graph");
        let counter = Rc::clone(&presses);
        let surface = NodeGraphSurface::new("Paint-only graph", state.clone()).node_type(
            "interactive",
            move |_id, _node| InteractiveTestNode {
                presses: Rc::clone(&counter),
            },
        );
        let (mut runtime, window_id) = build_runtime_with_widget(surface);
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("surface semantics")
            .bounds;
        let position = state
            .viewport()
            .flow_to_screen(bounds, Point::new(100.0, 90.0));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;

        assert_eq!(presses.get(), 1);
        assert_eq!(
            state.node(&NodeId::from("custom")).unwrap().position,
            Point::new(40.0, 60.0)
        );
        Ok(())
    }

    #[test]
    fn runtime_resize_handle_updates_node_dimensions() -> sui_core::Result<()> {
        let mut node = Node::new("node", Point::new(40.0, 60.0), ())
            .size(Size::new(180.0, 80.0))
            .resizable(true)
            .size_limits(Size::new(100.0, 50.0), Size::new(400.0, 300.0));
        node.selected = true;
        let state = NodeGraphState::<(), ()>::new(vec![node], Vec::new()).unwrap();
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let start = state
            .viewport()
            .flow_to_screen(bounds, Point::new(220.0, 140.0));
        let end = start + Vector::new(50.0, 30.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let node = state.node(&NodeId::from("node")).unwrap();
        assert_eq!(node.position, Point::new(40.0, 60.0));
        assert_eq!(node.size, Size::new(230.0, 110.0));
        Ok(())
    }

    #[test]
    fn runtime_reconnects_selected_edge_endpoint() -> sui_core::Result<()> {
        let mut edge = Edge::new("edge", "a", "b", ());
        edge.selected = true;
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("a", Point::new(20.0, 40.0), ()),
                Node::new("b", Point::new(340.0, 40.0), ()),
                Node::new("c", Point::new(340.0, 220.0), ()),
            ],
            vec![edge],
        )
        .unwrap();
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("graph semantics")
            .bounds;
        let snapshot = state.snapshot();
        let b = snapshot.graph.node(&NodeId::from("b")).unwrap();
        let b_target = b.first_handle(HandleKind::Target).unwrap();
        let c = snapshot.graph.node(&NodeId::from("c")).unwrap();
        let c_target = c.first_handle(HandleKind::Target).unwrap();
        let start = snapshot
            .viewport
            .flow_to_screen(bounds, handle_position(&snapshot.graph, b, b_target));
        let end = snapshot
            .viewport
            .flow_to_screen(bounds, handle_position(&snapshot.graph, c, c_target));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, end, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let edge = state.edge(&EdgeId::from("edge")).unwrap();
        assert_eq!(edge.source, NodeId::from("a"));
        assert_eq!(edge.target, NodeId::from("c"));
        assert_eq!(edge.target_handle, Some(HandleId::from("target")));
        Ok(())
    }

    #[test]
    fn per_element_semantics_focus_activate_and_delete_node() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::new(
            vec![Node::new("node", Point::new(40.0, 60.0), ()).label("Accessible node")],
            Vec::new(),
        )
        .unwrap();
        let (mut runtime, window_id) = build_runtime(state.clone());
        let first = runtime.render(window_id)?;
        let node_id = first
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Accessible node"))
            .expect("node semantic")
            .id;

        assert!(runtime.handle_semantics_action(
            window_id,
            node_id,
            SemanticsActionRequest::Focus,
        )?);
        assert!(runtime.handle_semantics_action(
            window_id,
            node_id,
            SemanticsActionRequest::Activate,
        )?);
        assert!(state.node(&NodeId::from("node")).unwrap().selected);
        assert!(
            runtime
                .render(window_id)?
                .semantics
                .iter()
                .find(|node| node.id == node_id)
                .unwrap()
                .state
                .focused
        );

        assert!(runtime.handle_semantics_action(
            window_id,
            node_id,
            SemanticsActionRequest::Custom {
                name: "Delete".to_string(),
                value: None,
            },
        )?);
        assert!(state.node(&NodeId::from("node")).is_none());
        Ok(())
    }

    #[test]
    fn connection_interaction_emits_start_connect_and_end_lifecycle() -> sui_core::Result<()> {
        let events = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&events);
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("source", Point::new(20.0, 40.0), ()),
                Node::new("target", Point::new(360.0, 180.0), ()),
            ],
            Vec::new(),
        )
        .unwrap();
        let graph = NodeGraph::new("Graph", state.clone())
            .on_change(move |event| captured.borrow_mut().push(event));
        let (mut runtime, window_id) = build_runtime_with_graph(graph);
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .unwrap()
            .bounds;
        let snapshot = state.snapshot();
        let source = snapshot.graph.node(&NodeId::from("source")).unwrap();
        let target = snapshot.graph.node(&NodeId::from("target")).unwrap();
        let start = snapshot.viewport.flow_to_screen(
            bounds,
            handle_position(
                &snapshot.graph,
                source,
                source.first_handle(HandleKind::Source).unwrap(),
            ),
        );
        let end = snapshot.viewport.flow_to_screen(
            bounds,
            handle_position(
                &snapshot.graph,
                target,
                target.first_handle(HandleKind::Target).unwrap(),
            ),
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let events = events.borrow();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, NodeGraphEvent::ConnectionStarted { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, NodeGraphEvent::Connect(_)))
        );
        assert!(events.iter().any(|event| matches!(
            event,
            NodeGraphEvent::ConnectionEnded {
                connection: Some(_)
            }
        )));
        Ok(())
    }

    #[test]
    fn animated_edge_requests_follow_up_animation_frames() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("a", Point::new(20.0, 40.0), ()),
                Node::new("b", Point::new(340.0, 180.0), ()),
            ],
            vec![Edge::new("edge", "a", "b", ()).animated(true)],
        )
        .unwrap();
        let (mut runtime, window_id) = build_runtime(state);

        runtime.render(window_id)?;
        let ready = runtime.drain_ready_events();

        assert!(
            ready
                .iter()
                .any(|(_, event)| matches!(event, Event::Wake(WakeEvent::AnimationFrame { .. })))
        );
        Ok(())
    }

    fn node_benchmark_document() -> (Vec<Node<()>>, Vec<Edge<()>>) {
        const COLUMNS: usize = 24;
        const ROWS: usize = 16;
        let mut nodes = Vec::with_capacity(COLUMNS * ROWS);
        let mut edges = Vec::new();
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let index = row * COLUMNS + column;
                nodes.push(
                    Node::new(
                        format!("node-{index}"),
                        Point::new(column as f32 * 50.0, row as f32 * 34.0),
                        (),
                    )
                    .kind("benchmark")
                    .size(Size::new(44.0, 28.0)),
                );
                if column > 0 {
                    edges.push(Edge::new(
                        format!("edge-{index}"),
                        format!("node-{}", index - 1),
                        format!("node-{index}"),
                        (),
                    ));
                }
            }
        }
        (nodes, edges)
    }

    fn node_benchmark_percentile(samples: &mut [f64], quantile: f64) -> f64 {
        samples.sort_by(f64::total_cmp);
        let index = ((samples.len().saturating_sub(1)) as f64 * quantile).round() as usize;
        samples.get(index).copied().unwrap_or(0.0)
    }

    fn run_node_render_benchmark(retained: bool) -> (f64, f64, f64, u32, usize) {
        const WARMUP: usize = 20;
        const FRAMES: usize = 180;
        let (nodes, edges) = node_benchmark_document();
        let state = NodeGraphState::new(nodes, edges).expect("valid benchmark graph");
        let measures = Rc::new(Cell::new(0_u32));
        let graph = NodeGraph::new("Node benchmark", state.clone()).config(NodeGraphConfig {
            min_zoom: 0.1,
            max_zoom: 2.0,
            background_variant: BackgroundVariant::Dots,
            ..NodeGraphConfig::default()
        });
        let graph = if retained {
            let counter = Rc::clone(&measures);
            graph.node_type("benchmark", move |_id, _node| MeasureCountingNode {
                measures: Rc::clone(&counter),
            })
        } else {
            graph
        };
        let (mut runtime, window_id) = build_runtime_with_graph(graph);
        runtime.render(window_id).expect("initial benchmark render");
        let initial_measures = measures.get();
        let viewport_size = state.viewport_size();
        let center = Point::new(597.0, 269.0);

        for frame in 0..WARMUP {
            let zoom = 0.55 + ((frame % 11) as f32 * 0.035);
            state.set_viewport(Viewport::centered_on(center, viewport_size, zoom, 0.1, 2.0));
            runtime.render(window_id).expect("warm benchmark frame");
        }

        let mut samples = Vec::with_capacity(FRAMES);
        let mut command_count = 0usize;
        for frame in 0..FRAMES {
            let zoom = 0.50 + ((frame % 29) as f32 * 0.015);
            state.set_viewport(Viewport::centered_on(center, viewport_size, zoom, 0.1, 2.0));
            let started = std::time::Instant::now();
            let output = runtime.render(window_id).expect("node benchmark frame");
            samples.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            command_count = output.frame.scene.commands().len();
            std::hint::black_box(command_count);
        }
        assert_eq!(
            measures.get(),
            initial_measures,
            "viewport-only benchmark frames must not remeasure retained nodes"
        );
        let average = samples.iter().sum::<f64>() / samples.len() as f64;
        let mut ordered = samples.clone();
        let p50 = node_benchmark_percentile(&mut ordered, 0.50);
        let p95 = node_benchmark_percentile(&mut samples, 0.95);
        (average, p50, p95, initial_measures, command_count)
    }

    #[test]
    #[ignore = "diagnostic benchmark for node graph indexing and retained zoom frames"]
    fn node_graph_current_status_benchmark() {
        const COLUMNS: usize = 100;
        const ROWS: usize = 100;
        const QUERIES: usize = 2_000;
        let mut nodes = Vec::with_capacity(COLUMNS * ROWS);
        let mut edges = Vec::with_capacity((COLUMNS - 1) * ROWS + (ROWS - 1) * COLUMNS);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let index = row * COLUMNS + column;
                nodes.push(
                    Node::new(
                        format!("large-{index}"),
                        Point::new(column as f32 * 24.0, row as f32 * 18.0),
                        (),
                    )
                    .size(Size::new(20.0, 14.0)),
                );
                if column > 0 {
                    edges.push(Edge::new(
                        format!("large-h-{index}"),
                        format!("large-{}", index - 1),
                        format!("large-{index}"),
                        (),
                    ));
                }
                if row > 0 {
                    edges.push(Edge::new(
                        format!("large-v-{index}"),
                        format!("large-{}", index - COLUMNS),
                        format!("large-{index}"),
                        (),
                    ));
                }
            }
        }
        let model_started = std::time::Instant::now();
        let graph = GraphModel::new(nodes, edges).expect("valid large benchmark graph");
        let model_us = model_started.elapsed().as_secs_f64() * 1_000_000.0;
        let index_started = std::time::Instant::now();
        let mut builder = crate::GraphSpatialIndex::builder(&graph, 1);
        let index_prepare_us = index_started.elapsed().as_secs_f64() * 1_000_000.0;
        let mut index_steps = 0usize;
        let mut index_max_step_us = 0.0_f64;
        while !builder.progress().is_complete() {
            let step_started = std::time::Instant::now();
            builder.advance(512);
            index_max_step_us =
                index_max_step_us.max(step_started.elapsed().as_secs_f64() * 1_000_000.0);
            index_steps += 1;
        }
        let spatial = builder.finish();
        let index_us = index_started.elapsed().as_secs_f64() * 1_000_000.0;
        let query_started = std::time::Instant::now();
        let mut candidates = 0usize;
        for query in 0..QUERIES {
            let x = (query % 97) as f32 * 23.0;
            let y = (query % 89) as f32 * 17.0;
            let area = Rect::new(x, y, 240.0, 180.0);
            candidates += spatial.query_node_indices(area).len();
            candidates += spatial.query_edge_indices(area).len();
        }
        let query_us = query_started.elapsed().as_secs_f64() * 1_000_000.0;
        std::hint::black_box(candidates);

        let (painted_avg, painted_p50, painted_p95, _, painted_commands) =
            run_node_render_benchmark(false);
        let (retained_avg, retained_p50, retained_p95, retained_measures, retained_commands) =
            run_node_render_benchmark(true);
        println!(
            "NODE_GRAPH_BENCHMARK model_nodes=10000 model_edges={} model_build_us={model_us:.2} spatial_build_us={index_us:.2} spatial_prepare_us={index_prepare_us:.2} spatial_steps={index_steps} spatial_max_step_us={index_max_step_us:.2} queries={QUERIES} query_total_us={query_us:.2} query_avg_us={:.3} query_candidates={candidates} render_nodes=384 render_edges=368 frames=180 painted_avg_us={painted_avg:.2} painted_p50_us={painted_p50:.2} painted_p95_us={painted_p95:.2} retained_avg_us={retained_avg:.2} retained_p50_us={retained_p50:.2} retained_p95_us={retained_p95:.2} retained_ratio={:.3} retained_measures={retained_measures} painted_commands={painted_commands} retained_commands={retained_commands}",
            graph.edges.len(),
            query_us / QUERIES as f64,
            retained_avg / painted_avg.max(0.001),
        );
    }

    #[test]
    fn double_click_zoom_is_anchored_on_the_pane() -> sui_core::Result<()> {
        let state = NodeGraphState::<(), ()>::default();
        let (mut runtime, window_id) = build_runtime(state.clone());
        let output = runtime.render(window_id)?;
        let bounds = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .unwrap()
            .bounds;
        let position = Point::new(bounds.x() + 300.0, bounds.y() + 220.0);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, position, false),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, position, true),
        )?;

        assert!((state.viewport().zoom - 1.2).abs() < 0.001);
        Ok(())
    }
}
