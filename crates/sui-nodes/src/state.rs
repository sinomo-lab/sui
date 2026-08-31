use std::{cell::RefCell, fmt, rc::Rc, sync::Arc};

use sui_core::{Point, Rect, Size};
use sui_reactive::Signal;

use crate::{
    Edge, EdgeId, FitViewOptions, GraphError, GraphModel, GraphSpatialIndex, HandleId, HandleKind,
    Node, NodeId, Viewport,
};

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

pub struct NodeGraphState<N = (), E = ()> {
    pub(crate) signal: Signal<GraphSnapshot<N, E>>,
    mode: NodeGraphMode,
    change_handler: SharedChangeHandler<N, E>,
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
        Self {
            signal: Signal::named("NodeGraphState", snapshot),
            mode: NodeGraphMode::Uncontrolled,
            change_handler: Rc::new(RefCell::new(None)),
        }
    }

    /// Create a controlled graph state.
    ///
    /// User interactions propose a complete next snapshot to the registered
    /// change handler. The owner accepts or replaces that proposal with
    /// [`Self::replace_snapshot`].
    pub fn controlled(snapshot: GraphSnapshot<N, E>) -> Self {
        Self {
            signal: Signal::named("ControlledNodeGraphState", snapshot),
            mode: NodeGraphMode::Controlled,
            change_handler: Rc::new(RefCell::new(None)),
        }
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

    fn request_snapshot(&self, mut snapshot: GraphSnapshot<N, E>) -> bool {
        let before = self.snapshot();
        if snapshot == before {
            return false;
        }
        snapshot.refresh_from(&before);
        match self.mode {
            NodeGraphMode::Uncontrolled => self.signal.set(snapshot),
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
        self.signal.set(snapshot)
    }
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
}
