use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    rc::Rc,
    sync::Arc,
};

use sui_core::{Point, Rect, Size, Vector};
use sui_reactive::Signal;

use crate::{
    Edge, EdgeId, FitViewOptions, GraphError, GraphModel, GraphSpatialIndex, HandleId, HandleKind,
    Node, NodeExtent, NodeId, Viewport,
};

/// How far each paste lands from the last, in flow units.
pub const PASTE_OFFSET: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeGraphMode {
    #[default]
    Uncontrolled,
    Controlled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeletedElements<N, E> {
    pub nodes: Vec<Node<N>>,
    pub edges: Vec<Edge<E>>,
}

/// Persistence-friendly graph data without runtime caches or transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphDocument<N = (), E = ()> {
    pub nodes: Vec<Node<N>>,
    pub edges: Vec<Edge<E>>,
    pub viewport: Viewport,
    pub interactive: bool,
}

impl<N, E> GraphDocument<N, E> {
    pub fn new(nodes: Vec<Node<N>>, edges: Vec<Edge<E>>) -> Self {
        Self {
            nodes,
            edges,
            viewport: Viewport::default(),
            interactive: true,
        }
    }

    pub fn viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = viewport;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SnapshotRevisions {
    pub document: u64,
    pub nodes: u64,
    pub edges: u64,
    pub viewport: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportTransition {
    pub from: Viewport,
    pub to: Viewport,
    pub duration: f64,
    pub elapsed: f64,
}

impl ViewportTransition {
    pub fn new(from: Viewport, to: Viewport, duration: f64) -> Self {
        Self {
            from,
            to,
            duration: duration.max(0.001),
            elapsed: 0.0,
        }
    }

    pub fn advance(&mut self, delta: f64) -> (Viewport, bool) {
        self.elapsed = (self.elapsed + delta.max(0.0)).min(self.duration);
        let linear = (self.elapsed / self.duration) as f32;
        let t = linear * linear * (3.0 - (2.0 * linear));
        (
            Viewport::new(
                self.from.x + ((self.to.x - self.from.x) * t),
                self.from.y + ((self.to.y - self.from.y) * t),
                self.from.zoom + ((self.to.zoom - self.from.zoom) * t),
            ),
            self.elapsed >= self.duration,
        )
    }
}

#[derive(Debug, Clone)]
pub struct GraphSnapshot<N = (), E = ()> {
    pub graph: Arc<GraphModel<N, E>>,
    pub spatial: Arc<GraphSpatialIndex>,
    pub revisions: SnapshotRevisions,
    pub viewport: Viewport,
    pub viewport_transition: Option<ViewportTransition>,
    pub viewport_size: Size,
    pub interactive: bool,
}

impl<N, E> GraphSnapshot<N, E> {
    pub fn new(graph: GraphModel<N, E>) -> Self {
        let spatial = GraphSpatialIndex::new(&graph, 0);
        Self::with_spatial_index(graph, spatial)
    }

    /// Create a snapshot from a graph and a previously completed spatial
    /// index, including one produced by [`GraphSpatialIndex::builder`].
    pub fn with_spatial_index(graph: GraphModel<N, E>, spatial: GraphSpatialIndex) -> Self {
        Self {
            graph: Arc::new(graph),
            spatial: Arc::new(spatial),
            revisions: SnapshotRevisions::default(),
            viewport: Viewport::default(),
            viewport_transition: None,
            viewport_size: Size::ZERO,
            interactive: true,
        }
    }

    pub fn viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = viewport;
        self
    }

    pub fn graph(&self) -> &GraphModel<N, E> {
        self.graph.as_ref()
    }

    pub fn spatial(&self) -> &GraphSpatialIndex {
        self.spatial.as_ref()
    }
}

impl<N, E> GraphSnapshot<N, E>
where
    N: Clone + PartialEq,
    E: Clone + PartialEq,
{
    pub fn graph_mut(&mut self) -> &mut GraphModel<N, E> {
        Arc::make_mut(&mut self.graph)
    }

    fn refresh_from(&mut self, before: &Self) {
        let nodes_changed = self.graph.nodes != before.graph.nodes;
        let edges_changed = self.graph.edges != before.graph.edges;
        let viewport_changed = self.viewport != before.viewport
            || self.viewport_size != before.viewport_size
            || self.viewport_transition != before.viewport_transition;
        let interactive_changed = self.interactive != before.interactive;
        self.revisions = before.revisions;

        if nodes_changed || edges_changed {
            let after_graph = Arc::clone(&self.graph);
            self.spatial = Arc::clone(&before.spatial);
            let next_spatial_revision = before.spatial.revision().wrapping_add(1);
            Arc::make_mut(&mut self.spatial).update(
                before.graph.as_ref(),
                after_graph.as_ref(),
                next_spatial_revision,
            );
            if nodes_changed {
                self.revisions.nodes = self.revisions.nodes.wrapping_add(1);
            }
            if edges_changed {
                self.revisions.edges = self.revisions.edges.wrapping_add(1);
            }
        } else {
            self.graph = Arc::clone(&before.graph);
            self.spatial = Arc::clone(&before.spatial);
        }
        if viewport_changed {
            self.revisions.viewport = self.revisions.viewport.wrapping_add(1);
        }
        if nodes_changed || edges_changed || viewport_changed || interactive_changed {
            self.revisions.document = self.revisions.document.wrapping_add(1);
        }
    }
}

impl<N, E> PartialEq for GraphSnapshot<N, E>
where
    N: PartialEq,
    E: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.graph == other.graph
            && self.viewport == other.viewport
            && self.viewport_transition == other.viewport_transition
            && self.viewport_size == other.viewport_size
            && self.interactive == other.interactive
    }
}

impl<N, E> Default for GraphSnapshot<N, E> {
    fn default() -> Self {
        Self::new(GraphModel::empty())
    }
}

/// How many steps undo and redo can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryStatus {
    pub undo_steps: usize,
    pub redo_steps: usize,
}

impl HistoryStatus {
    pub const fn can_undo(self) -> bool {
        self.undo_steps > 0
    }

    pub const fn can_redo(self) -> bool {
        self.redo_steps > 0
    }
}

/// The graphs undo and redo return to. Each entry is a whole graph, shared
/// with the snapshot it came from rather than copied.
struct History<N, E> {
    /// How many steps undo keeps; none when zero.
    limit: usize,
    undo: Vec<Arc<GraphModel<N, E>>>,
    redo: Vec<Arc<GraphModel<N, E>>>,
    /// Open undo groups: their changes undo as one step.
    group_depth: u32,
    /// Whether the open group has recorded its step.
    group_recorded: bool,
    /// Commits that are not edits: undo and redo themselves, and sizes the
    /// graph measured.
    suspended: u32,
    /// The key of the [`NodeGraphState::merge_undo`] in progress.
    merge_key: Option<String>,
    /// The key the last step was recorded under, while nothing else has
    /// been recorded since.
    last_merge: Option<String>,
}

impl<N, E> History<N, E> {
    fn new() -> Self {
        Self {
            limit: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            group_depth: 0,
            group_recorded: false,
            suspended: 0,
            merge_key: None,
            last_merge: None,
        }
    }

    fn status(&self) -> HistoryStatus {
        HistoryStatus {
            undo_steps: self.undo.len(),
            redo_steps: self.redo.len(),
        }
    }

    fn trim(&mut self) {
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(..excess);
        }
        self.redo.truncate(self.limit);
    }
}

/// Nodes, with the edges between them, copied out of a graph to paste back
/// in. Copying a node copies its descendants too.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphClipboard<N = (), E = ()> {
    pub nodes: Vec<Node<N>>,
    pub edges: Vec<Edge<E>>,
    /// Where the copied nodes whose parent was not copied were in the
    /// flow, for pasting them after that parent is gone.
    absolute_positions: HashMap<NodeId, Point>,
}

impl<N, E> GraphClipboard<N, E> {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// The state's own clipboard, and how many times it was pasted.
type ClipboardSlot<N, E> = Rc<RefCell<Option<(GraphClipboard<N, E>, u32)>>>;

pub struct NodeGraphState<N = (), E = ()> {
    pub(crate) signal: Signal<GraphSnapshot<N, E>>,
    mode: NodeGraphMode,
    change_handler: SharedChangeHandler<N, E>,
    history: Rc<RefCell<History<N, E>>>,
    history_status: Signal<HistoryStatus>,
    clipboard: ClipboardSlot<N, E>,
}

type ChangeRequest<N, E> = dyn FnMut(GraphSnapshot<N, E>) + 'static;
type SharedChangeHandler<N, E> = Rc<RefCell<Option<Box<ChangeRequest<N, E>>>>>;

impl<N, E> fmt::Debug for NodeGraphState<N, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeGraphState")
            .field("source_id", &self.signal.source_id())
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl<N, E> Clone for NodeGraphState<N, E> {
    fn clone(&self) -> Self {
        Self {
            signal: self.signal.clone(),
            mode: self.mode,
            change_handler: Rc::clone(&self.change_handler),
            history: Rc::clone(&self.history),
            history_status: self.history_status.clone(),
            clipboard: Rc::clone(&self.clipboard),
        }
    }
}

impl<N, E> NodeGraphState<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    pub fn new(nodes: Vec<Node<N>>, edges: Vec<Edge<E>>) -> Result<Self, GraphError> {
        Ok(Self::from_model(GraphModel::new(nodes, edges)?))
    }

    pub fn from_model(graph: GraphModel<N, E>) -> Self {
        Self::from_snapshot(GraphSnapshot::new(graph))
    }

    pub fn from_snapshot(snapshot: GraphSnapshot<N, E>) -> Self {
        Self::with_mode(
            Signal::named("NodeGraphState", snapshot),
            NodeGraphMode::Uncontrolled,
        )
    }

    fn with_mode(signal: Signal<GraphSnapshot<N, E>>, mode: NodeGraphMode) -> Self {
        Self {
            signal,
            mode,
            change_handler: Rc::new(RefCell::new(None)),
            history: Rc::new(RefCell::new(History::new())),
            history_status: Signal::named("NodeGraphState history", HistoryStatus::default()),
            clipboard: Rc::new(RefCell::new(None)),
        }
    }

    /// Create a controlled graph state.
    ///
    /// User interactions propose a complete next snapshot to the registered
    /// change handler. The owner accepts or replaces that proposal with
    /// [`Self::replace_snapshot`].
    pub fn controlled(snapshot: GraphSnapshot<N, E>) -> Self {
        Self::with_mode(
            Signal::named("ControlledNodeGraphState", snapshot),
            NodeGraphMode::Controlled,
        )
    }

    /// Keep up to `limit` steps of undo. Every edit to the graph is a step:
    /// adding, removing, moving, resizing, connecting, and changing data,
    /// labels, or styles. Selection and the viewport are not. A drag or a
    /// resize is one step, as is anything between [`Self::begin_undo_group`]
    /// and [`Self::end_undo_group`]. History is off by default.
    pub fn with_history(self, limit: usize) -> Self {
        self.set_history_limit(limit);
        self
    }

    /// Change how many steps of undo are kept; zero turns history off.
    pub fn set_history_limit(&self, limit: usize) {
        let status = {
            let mut history = self.history.borrow_mut();
            history.limit = limit;
            history.trim();
            history.status()
        };
        self.history_status.set(status);
    }

    pub fn history_status(&self) -> HistoryStatus {
        self.history_status.get()
    }

    /// [`Self::history_status`] as a value to observe, for undo and redo
    /// buttons.
    pub fn history_observable(&self) -> Signal<HistoryStatus> {
        self.history_status.clone()
    }

    pub fn can_undo(&self) -> bool {
        self.history_status().can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history_status().can_redo()
    }

    /// Return the graph to before the last step, keeping the viewport.
    /// Undo and redo replace the graph directly, in a controlled state too:
    /// they return to graphs the owner already accepted.
    pub fn undo(&self) -> bool {
        let current = self.snapshot();
        let previous = {
            let mut history = self.history.borrow_mut();
            if history.group_depth > 0 {
                return false;
            }
            let Some(previous) = history.undo.pop() else {
                return false;
            };
            history.last_merge = None;
            history.redo.push(Arc::clone(&current.graph));
            previous
        };
        self.restore_graph(current, previous)
    }

    /// Take the step the last undo took back.
    pub fn redo(&self) -> bool {
        let current = self.snapshot();
        let next = {
            let mut history = self.history.borrow_mut();
            if history.group_depth > 0 {
                return false;
            }
            let Some(next) = history.redo.pop() else {
                return false;
            };
            history.last_merge = None;
            history.undo.push(Arc::clone(&current.graph));
            next
        };
        self.restore_graph(current, next)
    }

    pub fn clear_history(&self) {
        let status = {
            let mut history = self.history.borrow_mut();
            history.undo.clear();
            history.redo.clear();
            history.last_merge = None;
            history.status()
        };
        self.history_status.set(status);
    }

    /// Start a group of changes that undo as one step, such as a drag, or
    /// an application command that adds a node and connects it. Groups
    /// nest; the step ends with the outermost [`Self::end_undo_group`].
    pub fn begin_undo_group(&self) {
        let mut history = self.history.borrow_mut();
        if history.group_depth == 0 {
            history.group_recorded = false;
        }
        history.group_depth += 1;
    }

    pub fn end_undo_group(&self) {
        let mut history = self.history.borrow_mut();
        history.group_depth = history.group_depth.saturating_sub(1);
    }

    /// Run `changes` as one undo step.
    pub fn undo_group<R>(&self, changes: impl FnOnce() -> R) -> R {
        self.begin_undo_group();
        let result = changes();
        self.end_undo_group();
        result
    }

    /// Run `changes`, merging them into the last undo step when that step
    /// was recorded by `merge_undo` with the same `key` and nothing has been
    /// recorded since. A name typed a key at a time, or a value dragged
    /// along a slider, then undoes in one step.
    pub fn merge_undo<R>(&self, key: impl Into<String>, changes: impl FnOnce() -> R) -> R {
        let outer = self.history.borrow_mut().merge_key.replace(key.into());
        let result = changes();
        self.history.borrow_mut().merge_key = outer;
        result
    }

    pub const fn mode(&self) -> NodeGraphMode {
        self.mode
    }

    pub const fn is_controlled(&self) -> bool {
        matches!(self.mode, NodeGraphMode::Controlled)
    }

    pub fn set_change_handler<F>(&self, handler: F)
    where
        F: FnMut(GraphSnapshot<N, E>) + 'static,
    {
        *self.change_handler.borrow_mut() = Some(Box::new(handler));
    }

    pub fn clear_change_handler(&self) {
        self.change_handler.borrow_mut().take();
    }

    pub fn snapshot(&self) -> GraphSnapshot<N, E> {
        self.signal.get()
    }

    /// Clone the underlying observable snapshot source for derived UI.
    pub fn observable(&self) -> Signal<GraphSnapshot<N, E>> {
        self.signal.clone()
    }

    pub fn graph(&self) -> GraphModel<N, E> {
        self.snapshot().graph.as_ref().clone()
    }

    pub fn nodes(&self) -> Vec<Node<N>> {
        self.snapshot().graph.nodes.clone()
    }

    pub fn edges(&self) -> Vec<Edge<E>> {
        self.snapshot().graph.edges.clone()
    }

    pub fn node(&self, id: &NodeId) -> Option<Node<N>> {
        self.snapshot().graph.node(id).cloned()
    }

    pub fn edge(&self, id: &EdgeId) -> Option<Edge<E>> {
        self.snapshot().graph.edge(id).cloned()
    }

    pub fn incoming_edges(&self, id: &NodeId) -> Vec<Edge<E>> {
        self.snapshot()
            .graph
            .incoming_edges(id)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn outgoing_edges(&self, id: &NodeId) -> Vec<Edge<E>> {
        self.snapshot()
            .graph
            .outgoing_edges(id)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn connected_edges(&self, ids: &[NodeId]) -> Vec<Edge<E>> {
        self.snapshot()
            .graph
            .connected_edges(ids)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn handle_connections(
        &self,
        node: &NodeId,
        kind: HandleKind,
        handle: Option<&HandleId>,
    ) -> Vec<Edge<E>> {
        self.snapshot()
            .graph
            .handle_connections(node, kind, handle)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn incomers(&self, id: &NodeId) -> Vec<Node<N>> {
        self.snapshot()
            .graph
            .incomers(id)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn outgoers(&self, id: &NodeId) -> Vec<Node<N>> {
        self.snapshot()
            .graph
            .outgoers(id)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn intersecting_nodes(&self, area: Rect, partially: bool) -> Vec<Node<N>> {
        self.snapshot()
            .graph
            .intersecting_nodes(area, partially)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn nodes_bounds(&self, ids: &[NodeId]) -> Option<Rect> {
        self.snapshot().graph.bounds_for_nodes(ids)
    }

    pub fn viewport(&self) -> Viewport {
        self.snapshot().viewport
    }

    pub fn viewport_size(&self) -> Size {
        self.snapshot().viewport_size
    }

    /// Convert a graph-local screen position into flow coordinates.
    pub fn screen_to_flow_position(&self, position: Point, snap: Option<Size>) -> Point {
        let snapshot = self.snapshot();
        let bounds = Rect::from_origin_size(Point::ZERO, snapshot.viewport_size);
        let mut position = snapshot.viewport.screen_to_flow(bounds, position);
        if let Some(grid) = snap {
            let width = grid.width.max(1.0);
            let height = grid.height.max(1.0);
            position.x = (position.x / width).round() * width;
            position.y = (position.y / height).round() * height;
        }
        position
    }

    /// Convert flow coordinates into graph-local screen coordinates.
    pub fn flow_to_screen_position(&self, position: Point) -> Point {
        let snapshot = self.snapshot();
        snapshot.viewport.flow_to_screen(
            Rect::from_origin_size(Point::ZERO, snapshot.viewport_size),
            position,
        )
    }

    pub fn set_center(&self, position: Point, zoom: f32, min_zoom: f32, max_zoom: f32) -> bool {
        let viewport =
            Viewport::centered_on(position, self.viewport_size(), zoom, min_zoom, max_zoom);
        self.set_viewport(viewport)
    }

    pub fn zoom_to(&self, zoom: f32, min_zoom: f32, max_zoom: f32) -> bool {
        let snapshot = self.snapshot();
        let bounds = Rect::from_origin_size(Point::ZERO, snapshot.viewport_size);
        let mut viewport = snapshot.viewport;
        let factor = zoom / viewport.zoom.max(0.001);
        viewport.zoom_at(
            bounds,
            Point::new(
                snapshot.viewport_size.width * 0.5,
                snapshot.viewport_size.height * 0.5,
            ),
            factor,
            min_zoom,
            max_zoom,
        );
        self.set_viewport(viewport)
    }

    pub fn fit_bounds(&self, bounds: Rect, options: FitViewOptions) -> bool {
        Viewport::fit(bounds, self.viewport_size(), options)
            .is_some_and(|viewport| self.set_viewport(viewport))
    }

    pub fn is_interactive(&self) -> bool {
        self.snapshot().interactive
    }

    pub fn set_viewport(&self, viewport: Viewport) -> bool {
        self.update(|snapshot| {
            snapshot.viewport = viewport;
            snapshot.viewport_transition = None;
        })
    }

    /// Replace the authoritative viewport even when this state is controlled.
    pub fn replace_viewport(&self, viewport: Viewport) -> bool {
        self.update_authoritative(|snapshot| {
            snapshot.viewport = viewport;
            snapshot.viewport_transition = None;
        })
    }

    pub fn animate_viewport(&self, viewport: Viewport, duration: f64) -> bool {
        self.update(|snapshot| {
            snapshot.viewport_transition = Some(ViewportTransition::new(
                snapshot.viewport,
                viewport,
                duration,
            ));
        })
    }

    pub fn set_viewport_size(&self, viewport_size: Size) -> bool {
        let viewport_size = Size::new(viewport_size.width.max(0.0), viewport_size.height.max(0.0));
        self.update_authoritative(|snapshot| snapshot.viewport_size = viewport_size)
    }

    pub fn set_interactive(&self, interactive: bool) -> bool {
        self.update(|snapshot| snapshot.interactive = interactive)
    }

    pub fn toggle_interactive(&self) -> bool {
        self.update(|snapshot| snapshot.interactive = !snapshot.interactive)
    }

    pub fn zoom_by(&self, factor: f32, min_zoom: f32, max_zoom: f32) -> bool {
        self.update(|snapshot| {
            snapshot.viewport_transition = None;
            let bounds = sui_core::Rect::from_origin_size(Point::ZERO, snapshot.viewport_size);
            snapshot.viewport.zoom_at(
                bounds,
                Point::new(
                    snapshot.viewport_size.width * 0.5,
                    snapshot.viewport_size.height * 0.5,
                ),
                factor,
                min_zoom,
                max_zoom,
            );
        })
    }

    pub fn add_node(&self, node: Node<N>) -> Result<bool, GraphError> {
        let mut result = Ok(());
        let changed = self.update(|snapshot| {
            result = snapshot.graph_mut().add_node(node);
        });
        result.map(|()| changed)
    }

    pub fn add_nodes(&self, nodes: impl IntoIterator<Item = Node<N>>) -> Result<bool, GraphError> {
        let mut next = self.snapshot();
        for node in nodes {
            next.graph_mut().add_node(node)?;
        }
        Ok(self.request_snapshot(next))
    }

    pub fn add_edge(&self, edge: Edge<E>) -> Result<bool, GraphError> {
        let mut result = Ok(());
        let changed = self.update(|snapshot| {
            result = snapshot.graph_mut().add_edge(edge);
        });
        result.map(|()| changed)
    }

    pub fn add_edges(&self, edges: impl IntoIterator<Item = Edge<E>>) -> Result<bool, GraphError> {
        let mut next = self.snapshot();
        for edge in edges {
            next.graph_mut().add_edge(edge)?;
        }
        Ok(self.request_snapshot(next))
    }

    pub fn set_nodes(&self, nodes: Vec<Node<N>>) -> Result<bool, GraphError> {
        let mut next = self.snapshot();
        let graph = GraphModel::new(nodes, next.graph.edges.clone())?;
        next.graph = Arc::new(graph);
        Ok(self.request_snapshot(next))
    }

    pub fn set_edges(&self, edges: Vec<Edge<E>>) -> Result<bool, GraphError> {
        let mut next = self.snapshot();
        let graph = GraphModel::new(next.graph.nodes.clone(), edges)?;
        next.graph = Arc::new(graph);
        Ok(self.request_snapshot(next))
    }

    pub fn set_graph(&self, graph: GraphModel<N, E>) -> Result<bool, GraphError> {
        graph.validate()?;
        let mut next = self.snapshot();
        next.graph = Arc::new(graph);
        Ok(self.request_snapshot(next))
    }

    /// Replace the authoritative graph even when this state is controlled.
    pub fn replace_graph(&self, graph: GraphModel<N, E>) -> Result<bool, GraphError> {
        graph.validate()?;
        Ok(self.update_authoritative(|snapshot| snapshot.graph = Arc::new(graph)))
    }

    pub fn remove_node(&self, id: &NodeId) -> bool {
        self.update(|snapshot| {
            snapshot.graph_mut().remove_node(id);
        })
    }

    pub fn remove_edge(&self, id: &EdgeId) -> bool {
        self.update(|snapshot| {
            snapshot.graph_mut().remove_edge(id);
        })
    }

    pub fn set_node_position(&self, id: &NodeId, position: Point) -> bool {
        self.update(|snapshot| {
            snapshot.graph_mut().move_node(id, position);
        })
    }

    pub fn resize_node(&self, id: &NodeId, position: Point, size: Size) -> bool {
        self.update(|snapshot| {
            snapshot.graph_mut().resize_node(id, position, size);
        })
    }

    pub fn update_node<F>(&self, id: &NodeId, update: F) -> Result<bool, GraphError>
    where
        F: FnOnce(&mut Node<N>),
    {
        let mut next = self.snapshot();
        let Some(node) = next.graph_mut().node_mut(id) else {
            return Ok(false);
        };
        let stable_id = node.id.clone();
        update(node);
        if node.id != stable_id {
            node.id = stable_id;
        }
        next.graph.validate()?;
        Ok(self.request_snapshot(next))
    }

    pub fn update_node_data<F>(&self, id: &NodeId, update: F) -> bool
    where
        F: FnOnce(&mut N),
    {
        let mut next = self.snapshot();
        let Some(node) = next.graph_mut().node_mut(id) else {
            return false;
        };
        update(&mut node.data);
        self.request_snapshot(next)
    }

    pub fn update_edge<F>(&self, id: &EdgeId, update: F) -> Result<bool, GraphError>
    where
        F: FnOnce(&mut Edge<E>),
    {
        let mut next = self.snapshot();
        let Some(edge) = next.graph_mut().edge_mut(id) else {
            return Ok(false);
        };
        let stable_id = edge.id.clone();
        update(edge);
        if edge.id != stable_id {
            edge.id = stable_id;
        }
        next.graph.validate()?;
        Ok(self.request_snapshot(next))
    }

    pub fn update_edge_data<F>(&self, id: &EdgeId, update: F) -> bool
    where
        F: FnOnce(&mut E),
    {
        let mut next = self.snapshot();
        let Some(edge) = next.graph_mut().edge_mut(id) else {
            return false;
        };
        update(&mut edge.data);
        self.request_snapshot(next)
    }

    pub fn clear_selection(&self) -> bool {
        self.update(|snapshot| {
            snapshot.graph_mut().clear_selection();
        })
    }

    pub fn fit_view(&self, viewport_size: Size, options: FitViewOptions) -> bool {
        self.update(|snapshot| {
            if let Some(bounds) = snapshot.graph.bounds()
                && let Some(viewport) = Viewport::fit(bounds, viewport_size, options)
            {
                snapshot.viewport = viewport;
                snapshot.viewport_transition = None;
            }
        })
    }

    pub fn delete_elements(
        &self,
        node_ids: &[NodeId],
        edge_ids: &[EdgeId],
    ) -> DeletedElements<N, E> {
        let mut next = self.snapshot();
        let mut deleted = DeletedElements {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        for id in edge_ids {
            if let Some(edge) = next.graph_mut().remove_edge(id) {
                deleted.edges.push(edge);
            }
        }
        for id in node_ids {
            if let Some(removed) = next.graph_mut().remove_node(id) {
                deleted.nodes.push(removed.node);
                deleted.nodes.extend(removed.descendants);
                for edge in removed.edges {
                    if !deleted
                        .edges
                        .iter()
                        .any(|candidate| candidate.id == edge.id)
                    {
                        deleted.edges.push(edge);
                    }
                }
            }
        }
        if !deleted.nodes.is_empty() || !deleted.edges.is_empty() {
            self.request_snapshot(next);
        }
        deleted
    }

    /// The selected nodes, their descendants, and the edges between them;
    /// `None` when no node is selected.
    pub fn copy_selection(&self) -> Option<GraphClipboard<N, E>> {
        let snapshot = self.snapshot();
        let graph = snapshot.graph.as_ref();
        let mut ids = HashSet::new();
        for node in graph.nodes.iter().filter(|node| node.selected) {
            ids.insert(node.id.clone());
            ids.extend(
                graph
                    .descendants(&node.id)
                    .into_iter()
                    .map(|node| node.id.clone()),
            );
        }
        if ids.is_empty() {
            return None;
        }
        // Parents first, so pasting can add each node after its parent.
        let mut nodes = graph
            .nodes
            .iter()
            .filter(|node| ids.contains(&node.id))
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| graph.node_depth(&node.id));
        let absolute_positions = nodes
            .iter()
            .filter(|node| {
                node.parent_id
                    .as_ref()
                    .is_some_and(|parent| !ids.contains(parent))
            })
            .filter_map(|node| {
                let bounds = graph.node_bounds(node)?;
                Some((
                    node.id.clone(),
                    bounds.origin + (node.position - node.bounds().origin),
                ))
            })
            .collect();
        Some(GraphClipboard {
            nodes: nodes.into_iter().cloned().collect(),
            edges: graph
                .edges
                .iter()
                .filter(|edge| ids.contains(&edge.source) && ids.contains(&edge.target))
                .cloned()
                .collect(),
            absolute_positions,
        })
    }

    /// Add `clipboard`'s nodes and edges, `offset` from where they were
    /// copied, with fresh ids, as one undo step. The pasted elements are
    /// selected as they were when copied, and nothing else is. Returns the
    /// new nodes' ids.
    pub fn paste_clipboard(
        &self,
        clipboard: &GraphClipboard<N, E>,
        offset: Vector,
    ) -> Result<Vec<NodeId>, GraphError> {
        if clipboard.is_empty() {
            return Ok(Vec::new());
        }
        let mut next = self.snapshot();
        let graph = next.graph_mut();
        graph.clear_selection();
        let mut taken = HashSet::new();
        let node_ids = clipboard
            .nodes
            .iter()
            .map(|node| {
                let id = fresh_id(node.id.as_str(), |candidate| {
                    taken.contains(candidate) || graph.node(&NodeId::from(candidate)).is_some()
                });
                taken.insert(id.clone());
                (node.id.clone(), NodeId::from(id))
            })
            .collect::<HashMap<_, _>>();
        for node in &clipboard.nodes {
            let mut pasted = node.clone();
            pasted.id = node_ids[&node.id].clone();
            match &node.parent_id {
                // Children move with their pasted parent.
                Some(parent) if node_ids.contains_key(parent) => {
                    pasted.parent_id = Some(node_ids[parent].clone());
                }
                Some(parent) if graph.node(parent).is_some() => {
                    pasted.position = node.position + offset;
                }
                Some(_) => {
                    let position = clipboard
                        .absolute_positions
                        .get(&node.id)
                        .copied()
                        .unwrap_or(node.position);
                    pasted.parent_id = None;
                    pasted.position = position + offset;
                    if pasted.extent == NodeExtent::Parent {
                        pasted.extent = NodeExtent::Unbounded;
                    }
                }
                None => pasted.position = node.position + offset,
            }
            graph.add_node(pasted)?;
        }
        let mut taken = HashSet::new();
        for edge in &clipboard.edges {
            let mut pasted = edge.clone();
            let id = fresh_id(edge.id.as_str(), |candidate| {
                taken.contains(candidate) || graph.edge(&EdgeId::from(candidate)).is_some()
            });
            taken.insert(id.clone());
            pasted.id = EdgeId::from(id);
            pasted.source = node_ids[&edge.source].clone();
            pasted.target = node_ids[&edge.target].clone();
            graph.add_edge(pasted)?;
        }
        self.request_snapshot(next);
        Ok(clipboard
            .nodes
            .iter()
            .map(|node| node_ids[&node.id].clone())
            .collect())
    }

    /// Copy the selection to this state's clipboard, which every graph and
    /// companion sharing the state uses. Returns whether anything was
    /// selected to copy.
    pub fn copy(&self) -> bool {
        let Some(clipboard) = self.copy_selection() else {
            return false;
        };
        *self.clipboard.borrow_mut() = Some((clipboard, 0));
        true
    }

    /// Copy the selection, then delete it.
    pub fn cut(&self) -> bool {
        if !self.copy() {
            return false;
        }
        let snapshot = self.snapshot();
        self.delete_elements(
            &snapshot.graph.selected_node_ids(),
            &snapshot.graph.selected_edge_ids(),
        );
        true
    }

    /// Paste the clipboard, each time a step further from where it was
    /// copied.
    pub fn paste(&self) -> Result<Vec<NodeId>, GraphError> {
        let Some((clipboard, pastes)) = self.clipboard.borrow().clone() else {
            return Ok(Vec::new());
        };
        let pastes = pastes + 1;
        let step = PASTE_OFFSET * pastes as f32;
        let pasted = self.paste_clipboard(&clipboard, Vector::new(step, step))?;
        if let Some((_, count)) = self.clipboard.borrow_mut().as_mut() {
            *count = pastes;
        }
        Ok(pasted)
    }

    pub fn can_paste(&self) -> bool {
        self.clipboard
            .borrow()
            .as_ref()
            .is_some_and(|(clipboard, _)| !clipboard.is_empty())
    }

    /// Paste a copy of the selection beside it, leaving the clipboard alone.
    pub fn duplicate(&self) -> Result<Vec<NodeId>, GraphError> {
        match self.copy_selection() {
            Some(clipboard) => {
                self.paste_clipboard(&clipboard, Vector::new(PASTE_OFFSET, PASTE_OFFSET))
            }
            None => Ok(Vec::new()),
        }
    }

    pub fn to_object(&self) -> GraphSnapshot<N, E> {
        self.snapshot()
    }

    pub fn to_document(&self) -> GraphDocument<N, E> {
        let snapshot = self.snapshot();
        GraphDocument {
            nodes: snapshot.graph.nodes.clone(),
            edges: snapshot.graph.edges.clone(),
            viewport: snapshot.viewport,
            interactive: snapshot.interactive,
        }
    }

    pub fn restore_document(&self, document: GraphDocument<N, E>) -> Result<bool, GraphError> {
        let graph = GraphModel::new(document.nodes, document.edges)?;
        let mut snapshot = GraphSnapshot::new(graph).viewport(document.viewport);
        snapshot.interactive = document.interactive;
        self.replace_snapshot(snapshot)
    }

    /// Accept an authoritative snapshot, bypassing controlled change requests.
    pub fn replace_snapshot(&self, snapshot: GraphSnapshot<N, E>) -> Result<bool, GraphError> {
        snapshot.graph.validate()?;
        Ok(self.commit_authoritative(snapshot))
    }

    pub fn restore(&self, snapshot: GraphSnapshot<N, E>) -> Result<bool, GraphError> {
        self.replace_snapshot(snapshot)
    }

    pub(crate) fn update(&self, update: impl FnOnce(&mut GraphSnapshot<N, E>)) -> bool {
        let mut next = self.snapshot();
        update(&mut next);
        self.request_snapshot(next)
    }

    pub(crate) fn update_authoritative(
        &self,
        update: impl FnOnce(&mut GraphSnapshot<N, E>),
    ) -> bool {
        let mut next = self.snapshot();
        update(&mut next);
        self.commit_authoritative(next)
    }

    /// Commit a change that is not an edit, such as a size the graph
    /// measured, without recording an undo step.
    pub(crate) fn update_untracked(&self, update: impl FnOnce(&mut GraphSnapshot<N, E>)) -> bool {
        self.history.borrow_mut().suspended += 1;
        let changed = self.update_authoritative(update);
        self.history.borrow_mut().suspended -= 1;
        changed
    }

    fn request_snapshot(&self, mut snapshot: GraphSnapshot<N, E>) -> bool {
        let before = self.snapshot();
        if snapshot == before {
            return false;
        }
        snapshot.refresh_from(&before);
        match self.mode {
            NodeGraphMode::Uncontrolled => self.commit(&before, snapshot),
            NodeGraphMode::Controlled => {
                if let Some(handler) = self.change_handler.borrow_mut().as_mut() {
                    handler(snapshot);
                }
                true
            }
        }
    }

    fn commit_authoritative(&self, mut snapshot: GraphSnapshot<N, E>) -> bool {
        let before = self.snapshot();
        if snapshot == before {
            return false;
        }
        snapshot.refresh_from(&before);
        self.commit(&before, snapshot)
    }

    /// Publish `snapshot`, recording `before`'s graph for undo when the
    /// graph's content changed.
    fn commit(&self, before: &GraphSnapshot<N, E>, snapshot: GraphSnapshot<N, E>) -> bool {
        let edited = !Arc::ptr_eq(&before.graph, &snapshot.graph)
            && !same_content(&before.graph, &snapshot.graph);
        let changed = self.signal.set(snapshot);
        if changed && edited {
            self.record(Arc::clone(&before.graph));
        }
        changed
    }

    fn record(&self, before: Arc<GraphModel<N, E>>) {
        let status = {
            let mut history = self.history.borrow_mut();
            if history.limit == 0 || history.suspended > 0 {
                return;
            }
            if history.group_depth > 0 {
                if history.group_recorded {
                    return;
                }
                history.group_recorded = true;
                history.last_merge = None;
            } else if history.merge_key.is_some() && history.merge_key == history.last_merge {
                // Part of the step the last change with this key recorded.
                return;
            } else {
                history.last_merge = history.merge_key.clone();
            }
            history.undo.push(before);
            history.redo.clear();
            history.trim();
            history.status()
        };
        self.history_status.set(status);
    }

    fn restore_graph(&self, current: GraphSnapshot<N, E>, graph: Arc<GraphModel<N, E>>) -> bool {
        let mut next = current;
        next.graph = graph;
        self.history.borrow_mut().suspended += 1;
        let changed = self.commit_authoritative(next);
        let status = {
            let mut history = self.history.borrow_mut();
            history.suspended -= 1;
            history.status()
        };
        self.history_status.set(status);
        changed
    }
}

/// Whether two graphs hold the same elements, ignoring which are selected.
fn same_content<N, E>(first: &GraphModel<N, E>, second: &GraphModel<N, E>) -> bool
where
    N: Clone + PartialEq,
    E: Clone + PartialEq,
{
    first.nodes.len() == second.nodes.len()
        && first.edges.len() == second.edges.len()
        && first
            .nodes
            .iter()
            .zip(&second.nodes)
            .all(|(first, second)| {
                first == second
                    || (first.selected != second.selected && {
                        let mut first = first.clone();
                        first.selected = second.selected;
                        first == *second
                    })
            })
        && first
            .edges
            .iter()
            .zip(&second.edges)
            .all(|(first, second)| {
                first == second
                    || (first.selected != second.selected && {
                        let mut first = first.clone();
                        first.selected = second.selected;
                        first == *second
                    })
            })
}

/// An id based on `id` that `taken` does not hold: `id-copy`, then
/// `id-copy-2`, and so on, from the id without an earlier copy suffix.
fn fresh_id(id: &str, taken: impl Fn(&str) -> bool) -> String {
    let stem = strip_copy_suffix(id);
    (1..)
        .map(|count| {
            if count == 1 {
                format!("{stem}-copy")
            } else {
                format!("{stem}-copy-{count}")
            }
        })
        .find(|candidate| !taken(candidate))
        .expect("an unused id")
}

fn strip_copy_suffix(id: &str) -> &str {
    if let Some(index) = id.rfind("-copy") {
        let rest = &id[index + "-copy".len()..];
        let numbered = rest
            .strip_prefix('-')
            .is_some_and(|count| !count.is_empty() && count.chars().all(|c| c.is_ascii_digit()));
        if rest.is_empty() || numbered {
            return &id[..index];
        }
    }
    id
}

impl<N, E> Default for NodeGraphState<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    fn default() -> Self {
        Self::from_snapshot(GraphSnapshot::default())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use sui_core::Point;

    use super::*;

    #[test]
    fn cloned_state_is_a_shared_editor_handle() {
        let state = NodeGraphState::<(), ()>::new(
            vec![Node::new("node", Point::new(10.0, 20.0), ())],
            Vec::new(),
        )
        .unwrap();
        let clone = state.clone();

        assert!(clone.set_node_position(&NodeId::from("node"), Point::new(40.0, 80.0)));

        assert_eq!(
            state.graph().node(&NodeId::from("node")).unwrap().position,
            Point::new(40.0, 80.0)
        );
    }

    #[test]
    fn failed_model_updates_leave_state_unchanged() {
        let state =
            NodeGraphState::<(), ()>::new(vec![Node::new("node", Point::ZERO, ())], Vec::new())
                .unwrap();

        let error = state
            .add_node(Node::new("node", Point::new(20.0, 20.0), ()))
            .unwrap_err();

        assert_eq!(error, GraphError::DuplicateNode(NodeId::from("node")));
        assert_eq!(state.graph().nodes.len(), 1);
    }

    #[test]
    fn controlled_state_proposes_before_authoritative_acceptance() {
        let initial = GraphSnapshot::new(
            GraphModel::<(), ()>::new(
                vec![Node::new("node", Point::new(10.0, 20.0), ())],
                Vec::new(),
            )
            .unwrap(),
        );
        let state = NodeGraphState::controlled(initial);
        let proposals = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&proposals);
        state.set_change_handler(move |snapshot| captured.borrow_mut().push(snapshot));

        assert!(state.set_node_position(&NodeId::from("node"), Point::new(90.0, 120.0)));
        assert_eq!(
            state.node(&NodeId::from("node")).unwrap().position,
            Point::new(10.0, 20.0)
        );

        let proposal = proposals.borrow_mut().pop().expect("change proposal");
        assert_eq!(
            proposal.graph.node(&NodeId::from("node")).unwrap().position,
            Point::new(90.0, 120.0)
        );
        state.replace_snapshot(proposal).unwrap();
        assert_eq!(
            state.node(&NodeId::from("node")).unwrap().position,
            Point::new(90.0, 120.0)
        );
    }

    #[test]
    fn state_exposes_bulk_updates_and_connection_queries() {
        let state = NodeGraphState::<u32, u32>::new(
            vec![
                Node::new("a", Point::ZERO, 1),
                Node::new("b", Point::new(200.0, 0.0), 2),
                Node::new("c", Point::new(400.0, 0.0), 3),
            ],
            vec![
                Edge::new("a-b", "a", "b", 10),
                Edge::new("b-c", "b", "c", 20),
            ],
        )
        .unwrap();

        state.update_node_data(&NodeId::from("b"), |data| *data = 22);
        state.update_edge_data(&EdgeId::from("b-c"), |data| *data = 30);

        assert_eq!(state.node(&NodeId::from("b")).unwrap().data, 22);
        assert_eq!(state.edge(&EdgeId::from("b-c")).unwrap().data, 30);
        assert_eq!(state.incomers(&NodeId::from("b"))[0].id, NodeId::from("a"));
        assert_eq!(state.outgoers(&NodeId::from("b"))[0].id, NodeId::from("c"));
        assert_eq!(state.connected_edges(&[NodeId::from("b")]).len(), 2);
    }

    #[test]
    fn viewport_changes_reuse_graph_and_spatial_allocations() {
        let state =
            NodeGraphState::<(), ()>::new(vec![Node::new("node", Point::ZERO, ())], Vec::new())
                .unwrap();
        let before = state.snapshot();

        state.set_viewport(Viewport::new(40.0, 20.0, 1.5));
        let after = state.snapshot();

        assert!(Arc::ptr_eq(&before.graph, &after.graph));
        assert!(Arc::ptr_eq(&before.spatial, &after.spatial));
        assert_eq!(after.revisions.nodes, before.revisions.nodes);
        assert_eq!(after.revisions.viewport, before.revisions.viewport + 1);
    }

    #[test]
    fn moving_parent_incrementally_reindexes_child_absolute_bounds() {
        let state = NodeGraphState::<(), ()>::new(
            vec![
                Node::new("parent", Point::new(100.0, 100.0), ()).size(Size::new(400.0, 300.0)),
                Node::new("child", Point::new(20.0, 30.0), ()).parent("parent"),
            ],
            Vec::new(),
        )
        .unwrap();
        let before = state.snapshot();

        state.set_node_position(&NodeId::from("parent"), Point::new(500.0, 250.0));
        let after = state.snapshot();

        assert_eq!(
            before.spatial.node_bounds(&NodeId::from("child")),
            Some(Rect::new(120.0, 130.0, 180.0, 72.0))
        );
        assert_eq!(
            after.spatial.node_bounds(&NodeId::from("child")),
            Some(Rect::new(520.0, 280.0, 180.0, 72.0))
        );
        assert_eq!(after.revisions.nodes, before.revisions.nodes + 1);
    }

    #[test]
    fn viewport_transition_uses_smooth_interpolation_and_settles_exactly() {
        let mut transition = ViewportTransition::new(
            Viewport::new(0.0, 0.0, 1.0),
            Viewport::new(100.0, -40.0, 2.0),
            0.5,
        );

        let (middle, finished) = transition.advance(0.25);
        assert!(!finished);
        assert_eq!(middle, Viewport::new(50.0, -20.0, 1.5));

        let (end, finished) = transition.advance(0.25);
        assert!(finished);
        assert_eq!(end, Viewport::new(100.0, -40.0, 2.0));
    }

    #[test]
    fn portable_document_round_trips_without_runtime_caches() {
        let state = NodeGraphState::<u32, u32>::new(
            vec![Node::new("node", Point::new(20.0, 30.0), 7)],
            Vec::new(),
        )
        .unwrap();
        state.set_viewport(Viewport::new(80.0, 40.0, 1.5));
        let document = state.to_document();
        let restored = NodeGraphState::<u32, u32>::default();

        restored.restore_document(document).unwrap();

        assert_eq!(restored.node(&NodeId::from("node")).unwrap().data, 7);
        assert_eq!(restored.viewport(), Viewport::new(80.0, 40.0, 1.5));
        let restored = restored.snapshot();
        assert_eq!(restored.spatial.revision(), 1);
        assert_eq!(
            restored.spatial.node_bounds(&NodeId::from("node")),
            Some(Rect::new(20.0, 30.0, 180.0, 72.0))
        );
    }

    fn id(value: &str) -> NodeId {
        NodeId::from(value)
    }

    #[test]
    fn history_is_off_until_asked_for() {
        let state =
            NodeGraphState::<(), ()>::new(vec![Node::new("a", Point::ZERO, ())], Vec::new())
                .unwrap();
        state.set_node_position(&id("a"), Point::new(40.0, 0.0));
        assert!(!state.can_undo());
        assert!(!state.undo());
    }

    #[test]
    fn undo_and_redo_step_through_edits_but_not_selection_or_the_viewport() {
        let state =
            NodeGraphState::<u32, ()>::new(vec![Node::new("a", Point::ZERO, 1)], Vec::new())
                .unwrap()
                .with_history(10);
        state.set_viewport(Viewport::new(10.0, 0.0, 2.0));
        state
            .update_node(&id("a"), |node| node.selected = true)
            .unwrap();
        assert!(!state.can_undo(), "selecting and panning are not steps");

        state
            .add_node(Node::new("b", Point::new(300.0, 0.0), 2))
            .unwrap();
        state.update_node_data(&id("a"), |data| *data = 5);
        assert_eq!(
            state.history_status(),
            HistoryStatus {
                undo_steps: 2,
                redo_steps: 0
            }
        );

        assert!(state.undo());
        assert_eq!(state.node(&id("a")).unwrap().data, 1);
        assert!(state.undo());
        assert!(state.node(&id("b")).is_none());
        assert!(!state.undo());
        assert_eq!(
            state.viewport(),
            Viewport::new(10.0, 0.0, 2.0),
            "undo keeps the viewport"
        );
        assert_eq!(state.snapshot().spatial.node_bounds(&id("b")), None);

        assert!(state.redo());
        assert_eq!(
            state.snapshot().spatial.node_bounds(&id("b")),
            Some(Rect::new(300.0, 0.0, 180.0, 72.0)),
            "the spatial index follows the restored graph"
        );
        assert!(state.can_redo());
        state.remove_node(&id("b"));
        assert!(!state.can_redo(), "a new edit drops what could be redone");
    }

    #[test]
    fn an_undo_group_is_one_step_and_history_keeps_its_limit() {
        let state =
            NodeGraphState::<(), ()>::new(vec![Node::new("a", Point::ZERO, ())], Vec::new())
                .unwrap()
                .with_history(2);
        state.undo_group(|| {
            for x in [10.0, 20.0, 30.0] {
                state.set_node_position(&id("a"), Point::new(x, 0.0));
            }
        });
        assert_eq!(state.history_status().undo_steps, 1);
        assert!(state.undo());
        assert_eq!(state.node(&id("a")).unwrap().position, Point::ZERO);

        for x in [10.0, 20.0, 30.0] {
            state.set_node_position(&id("a"), Point::new(x, 0.0));
        }
        assert_eq!(state.history_status().undo_steps, 2);
        assert!(state.undo() && state.undo());
        assert_eq!(
            state.node(&id("a")).unwrap().position,
            Point::new(10.0, 0.0)
        );
    }

    #[test]
    fn merged_changes_are_one_step_until_something_else_is_recorded() {
        let state = NodeGraphState::<u32, ()>::new(
            vec![
                Node::new("a", Point::ZERO, 0),
                Node::new("b", Point::new(300.0, 0.0), 0),
            ],
            Vec::new(),
        )
        .unwrap()
        .with_history(10);
        for value in 1..=3 {
            state.merge_undo("a value", || {
                state.update_node_data(&id("a"), |data| *data = value)
            });
        }
        assert_eq!(state.history_status().undo_steps, 1);
        state.merge_undo("b value", || {
            state.update_node_data(&id("b"), |data| *data = 9)
        });
        state.merge_undo("a value", || {
            state.update_node_data(&id("a"), |data| *data = 4)
        });
        assert_eq!(
            state.history_status().undo_steps,
            3,
            "other changes end a merge"
        );

        assert!(state.undo() && state.undo());
        assert_eq!(state.node(&id("a")).unwrap().data, 3);
        state.merge_undo("a value", || {
            state.update_node_data(&id("a"), |data| *data = 5)
        });
        assert_eq!(
            state.history_status().undo_steps,
            2,
            "a change after undo is a new step"
        );
    }

    #[test]
    fn a_controlled_state_records_the_proposals_it_accepts() {
        let state = NodeGraphState::controlled(GraphSnapshot::new(
            GraphModel::<(), ()>::new(vec![Node::new("a", Point::ZERO, ())], Vec::new()).unwrap(),
        ))
        .with_history(10);
        let accept = state.clone();
        state.set_change_handler(move |proposal| {
            accept.replace_snapshot(proposal).unwrap();
        });

        state.set_node_position(&id("a"), Point::new(90.0, 0.0));
        assert_eq!(state.history_status().undo_steps, 1);
        assert!(state.undo());
        assert_eq!(state.node(&id("a")).unwrap().position, Point::ZERO);
    }

    fn clipboard_graph() -> NodeGraphState<u32, u32> {
        let mut a = Node::new("a", Point::new(500.0, 0.0), 1);
        a.selected = true;
        let mut b = Node::new("b", Point::new(800.0, 0.0), 2);
        b.selected = true;
        NodeGraphState::new(
            vec![
                Node::new("group", Point::ZERO, 0).size(Size::new(400.0, 300.0)),
                Node::new("child", Point::new(20.0, 30.0), 3).parent("group"),
                a,
                b,
            ],
            vec![
                Edge::new("a-b", "a", "b", 10),
                Edge::new("child-a", "child", "a", 20),
            ],
        )
        .unwrap()
        .with_history(10)
    }

    #[test]
    fn pasting_adds_fresh_copies_of_the_selection_and_the_edges_between_them() {
        let state = clipboard_graph();
        assert!(state.copy());
        let pasted = state.paste().unwrap();
        assert_eq!(pasted, vec![id("a-copy"), id("b-copy")]);

        let graph = state.graph();
        let copy = graph.node(&id("a-copy")).unwrap();
        assert_eq!(copy.position, Point::new(532.0, 32.0));
        assert_eq!(copy.data, 1);
        let edge = graph
            .edge(&EdgeId::from("a-b-copy"))
            .expect("the edge between them");
        assert_eq!(
            (edge.source.as_str(), edge.target.as_str()),
            ("a-copy", "b-copy")
        );
        assert_eq!(
            graph.edges.len(),
            3,
            "an edge to a node left behind is not copied"
        );
        assert_eq!(graph.selected_node_ids(), vec![id("a-copy"), id("b-copy")]);
        assert_eq!(state.history_status().undo_steps, 1, "one step");

        let again = state.paste().unwrap();
        assert_eq!(again, vec![id("a-copy-2"), id("b-copy-2")]);
        assert_eq!(
            state.node(&id("a-copy-2")).unwrap().position,
            Point::new(564.0, 64.0),
            "each paste lands a step further"
        );

        // A copy of a copy is named from the original.
        assert_eq!(state.duplicate().unwrap()[0], id("a-copy-3"));
    }

    #[test]
    fn copying_a_parent_copies_its_children_and_a_child_alone_stays_in_its_parent() {
        let state = clipboard_graph();
        state.clear_selection();
        state
            .update_node(&id("group"), |node| node.selected = true)
            .unwrap();
        let pasted = state.duplicate().unwrap();
        assert_eq!(pasted, vec![id("group-copy"), id("child-copy")]);
        let child = state.node(&id("child-copy")).unwrap();
        assert_eq!(child.parent_id, Some(id("group-copy")));
        assert_eq!(
            child.position,
            Point::new(20.0, 30.0),
            "it moves with its parent"
        );

        state.clear_selection();
        state
            .update_node(&id("child"), |node| node.selected = true)
            .unwrap();
        let pasted = state.duplicate().unwrap();
        let child = state.node(&pasted[0]).unwrap();
        assert_eq!(child.parent_id, Some(id("group")));
        assert_eq!(child.position, Point::new(52.0, 62.0));
    }

    #[test]
    fn a_child_whose_parent_is_gone_pastes_where_it_was() {
        let state = clipboard_graph();
        state.clear_selection();
        state
            .update_node(&id("child"), |node| node.selected = true)
            .unwrap();
        assert!(state.copy());
        state.delete_elements(&[id("group")], &[]);

        let pasted = state.paste().unwrap();
        let child = state.node(&pasted[0]).unwrap();
        assert_eq!(child.parent_id, None);
        assert_eq!(child.position, Point::new(52.0, 62.0));
    }

    #[test]
    fn cutting_copies_the_selection_then_deletes_it() {
        let state = clipboard_graph();
        assert!(state.cut());
        assert!(state.node(&id("a")).is_none());
        assert!(state.can_paste());
        assert_eq!(state.paste().unwrap(), vec![id("a-copy"), id("b-copy")]);
    }
}
