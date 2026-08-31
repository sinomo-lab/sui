use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use sui_core::{Point, Rect, Size};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

string_id!(NodeId);
string_id!(EdgeId);
string_id!(HandleId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleKind {
    Source,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandlePosition {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Handle {
    pub id: HandleId,
    pub kind: HandleKind,
    pub position: HandlePosition,
    /// Normalized position along the selected side.
    pub offset: f32,
    pub connectable: bool,
}

impl Handle {
    pub fn new(id: impl Into<HandleId>, kind: HandleKind, position: HandlePosition) -> Self {
        Self {
            id: id.into(),
            kind,
            position,
            offset: 0.5,
            connectable: true,
        }
    }

    pub fn source(id: impl Into<HandleId>, position: HandlePosition) -> Self {
        Self::new(id, HandleKind::Source, position)
    }

    pub fn target(id: impl Into<HandleId>, position: HandlePosition) -> Self {
        Self::new(id, HandleKind::Target, position)
    }

    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset.clamp(0.0, 1.0);
        self
    }

    pub fn connectable(mut self, connectable: bool) -> Self {
        self.connectable = connectable;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node<N = ()> {
    pub id: NodeId,
    pub kind: String,
    pub position: Point,
    pub size: Size,
    pub size_mode: NodeSizeMode,
    pub parent_id: Option<NodeId>,
    pub extent: NodeExtent,
    pub expand_parent: bool,
    /// Normalized anchor inside the node used by [`Self::position`].
    pub origin: Point,
    pub z_index: i32,
    pub connectable: bool,
    pub focusable: bool,
    pub resizable: bool,
    pub min_size: Size,
    pub max_size: Size,
    pub aria_label: Option<String>,
    pub label: String,
    pub data: N,
    pub handles: Vec<Handle>,
    pub selected: bool,
    pub draggable: bool,
    pub selectable: bool,
    pub deletable: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NodeSizeMode {
    #[default]
    Fixed,
    Content {
        min: Size,
        max: Size,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NodeExtent {
    #[default]
    Unbounded,
    Parent,
    Rect(Rect),
}

impl<N> Node<N> {
    pub fn new(id: impl Into<NodeId>, position: Point, data: N) -> Self {
        let id = id.into();
        Self {
            label: id.to_string(),
            id,
            kind: "default".to_string(),
            position,
            size: Size::new(180.0, 72.0),
            size_mode: NodeSizeMode::Fixed,
            parent_id: None,
            extent: NodeExtent::Unbounded,
            expand_parent: false,
            origin: Point::ZERO,
            z_index: 0,
            connectable: true,
            focusable: true,
            resizable: false,
            min_size: Size::new(24.0, 24.0),
            max_size: Size::new(f32::INFINITY, f32::INFINITY),
            aria_label: None,
            data,
            handles: vec![
                Handle::target("target", HandlePosition::Left),
                Handle::source("source", HandlePosition::Right),
            ],
            selected: false,
            draggable: true,
            selectable: true,
            deletable: true,
            hidden: false,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = Size::new(size.width.max(1.0), size.height.max(1.0));
        self.size_mode = NodeSizeMode::Fixed;
        self
    }

    pub fn content_sized(mut self, min: Size, max: Size) -> Self {
        let min = Size::new(min.width.max(0.0), min.height.max(0.0));
        let max = Size::new(
            max.width.max(min.width).max(1.0),
            max.height.max(min.height).max(1.0),
        );
        self.size_mode = NodeSizeMode::Content { min, max };
        self
    }

    pub fn parent(mut self, parent: impl Into<NodeId>) -> Self {
        self.parent_id = Some(parent.into());
        self
    }

    pub fn extent(mut self, extent: NodeExtent) -> Self {
        self.extent = extent;
        self
    }

    pub fn expand_parent(mut self, expand: bool) -> Self {
        self.expand_parent = expand;
        self
    }

    pub fn origin(mut self, origin: Point) -> Self {
        self.origin = Point::new(origin.x.clamp(0.0, 1.0), origin.y.clamp(0.0, 1.0));
        self
    }

    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    pub fn connectable(mut self, connectable: bool) -> Self {
        self.connectable = connectable;
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn size_limits(mut self, min: Size, max: Size) -> Self {
        self.min_size = Size::new(min.width.max(1.0), min.height.max(1.0));
        self.max_size = Size::new(
            max.width.max(self.min_size.width),
            max.height.max(self.min_size.height),
        );
        self
    }

    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn handles(mut self, handles: impl IntoIterator<Item = Handle>) -> Self {
        self.handles = handles.into_iter().collect();
        self
    }

    pub fn handle(mut self, handle: Handle) -> Self {
        self.handles.push(handle);
        self
    }

    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    pub fn deletable(mut self, deletable: bool) -> Self {
        self.deletable = deletable;
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(
            self.position.x - (self.size.width * self.origin.x),
            self.position.y - (self.size.height * self.origin.y),
            self.size.width,
            self.size.height,
        )
    }

    pub fn handle_by_id(&self, id: &HandleId, kind: HandleKind) -> Option<&Handle> {
        self.handles
            .iter()
            .find(|handle| handle.id == *id && handle.kind == kind)
    }

    pub fn first_handle(&self, kind: HandleKind) -> Option<&Handle> {
        self.handles.iter().find(|handle| handle.kind == kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeKind {
    Straight,
    Step,
    SmoothStep,
    SimpleBezier,
    #[default]
    Bezier,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgePathOptions {
    pub curvature: f32,
    pub step_offset: f32,
    pub border_radius: f32,
}

impl Default for EdgePathOptions {
    fn default() -> Self {
        Self {
            curvature: 0.5,
            step_offset: 20.0,
            border_radius: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMarker {
    Arrow,
    ArrowClosed,
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeReconnectMode {
    None,
    Source,
    Target,
    #[default]
    Both,
}

impl EdgeReconnectMode {
    pub const fn allows(self, endpoint: HandleKind) -> bool {
        matches!(
            (self, endpoint),
            (Self::Both, _)
                | (Self::Source, HandleKind::Source)
                | (Self::Target, HandleKind::Target)
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge<E = ()> {
    pub id: EdgeId,
    pub source: NodeId,
    pub source_handle: Option<HandleId>,
    pub target: NodeId,
    pub target_handle: Option<HandleId>,
    pub label: Option<String>,
    pub data: E,
    pub kind: EdgeKind,
    pub path_options: EdgePathOptions,
    pub animated: bool,
    pub animation_speed: f32,
    pub selected: bool,
    pub selectable: bool,
    pub deletable: bool,
    pub hidden: bool,
    pub marker_start: Option<EdgeMarker>,
    pub marker_end: Option<EdgeMarker>,
    pub reconnectable: EdgeReconnectMode,
    pub focusable: bool,
    pub z_index: i32,
    pub aria_label: Option<String>,
}

impl<E> Edge<E> {
    pub fn new(
        id: impl Into<EdgeId>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        data: E,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            source_handle: None,
            target: target.into(),
            target_handle: None,
            label: None,
            data,
            kind: EdgeKind::Bezier,
            path_options: EdgePathOptions::default(),
            animated: false,
            animation_speed: 1.0,
            selected: false,
            selectable: true,
            deletable: true,
            hidden: false,
            marker_start: None,
            marker_end: Some(EdgeMarker::ArrowClosed),
            reconnectable: EdgeReconnectMode::Both,
            focusable: true,
            z_index: 0,
            aria_label: None,
        }
    }

    pub fn handles(mut self, source: impl Into<HandleId>, target: impl Into<HandleId>) -> Self {
        self.source_handle = Some(source.into());
        self.target_handle = Some(target.into());
        self
    }

    pub fn source_handle(mut self, handle: impl Into<HandleId>) -> Self {
        self.source_handle = Some(handle.into());
        self
    }

    pub fn target_handle(mut self, handle: impl Into<HandleId>) -> Self {
        self.target_handle = Some(handle.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn kind(mut self, kind: EdgeKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn path_options(mut self, options: EdgePathOptions) -> Self {
        self.path_options = options;
        self
    }

    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }

    pub fn animation_speed(mut self, speed: f32) -> Self {
        self.animation_speed = speed.max(0.01);
        self
    }

    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    pub fn deletable(mut self, deletable: bool) -> Self {
        self.deletable = deletable;
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    pub fn marker_end(mut self, marker_end: bool) -> Self {
        self.marker_end = marker_end.then_some(EdgeMarker::ArrowClosed);
        self
    }

    pub fn start_marker(mut self, marker: Option<EdgeMarker>) -> Self {
        self.marker_start = marker;
        self
    }

    pub fn end_marker(mut self, marker: Option<EdgeMarker>) -> Self {
        self.marker_end = marker;
        self
    }

    pub fn reconnectable(mut self, reconnectable: EdgeReconnectMode) -> Self {
        self.reconnectable = reconnectable;
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn reconnect(&mut self, connection: Connection) {
        self.source = connection.source;
        self.source_handle = connection.source_handle;
        self.target = connection.target;
        self.target_handle = connection.target_handle;
    }

    pub fn connection(&self) -> Connection {
        Connection {
            source: self.source.clone(),
            source_handle: self.source_handle.clone(),
            target: self.target.clone(),
            target_handle: self.target_handle.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub source: NodeId,
    pub source_handle: Option<HandleId>,
    pub target: NodeId,
    pub target_handle: Option<HandleId>,
}

impl Connection {
    pub fn new(source: impl Into<NodeId>, target: impl Into<NodeId>) -> Self {
        Self {
            source: source.into(),
            source_handle: None,
            target: target.into(),
            target_handle: None,
        }
    }

    pub fn handles(mut self, source: impl Into<HandleId>, target: impl Into<HandleId>) -> Self {
        self.source_handle = Some(source.into());
        self.target_handle = Some(target.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    MissingSource(NodeId),
    MissingTarget(NodeId),
    MissingSourceHandle { node: NodeId, handle: HandleId },
    MissingTargetHandle { node: NodeId, handle: HandleId },
    SourceHandleDisabled { node: NodeId, handle: HandleId },
    TargetHandleDisabled { node: NodeId, handle: HandleId },
}

impl fmt::Display for ConnectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(node) => write!(formatter, "source node `{node}` does not exist"),
            Self::MissingTarget(node) => write!(formatter, "target node `{node}` does not exist"),
            Self::MissingSourceHandle { node, handle } => {
                write!(
                    formatter,
                    "source handle `{handle}` does not exist on node `{node}`"
                )
            }
            Self::MissingTargetHandle { node, handle } => {
                write!(
                    formatter,
                    "target handle `{handle}` does not exist on node `{node}`"
                )
            }
            Self::SourceHandleDisabled { node, handle } => {
                write!(
                    formatter,
                    "source handle `{handle}` on node `{node}` is disabled"
                )
            }
            Self::TargetHandleDisabled { node, handle } => {
                write!(
                    formatter,
                    "target handle `{handle}` on node `{node}` is disabled"
                )
            }
        }
    }
}

impl Error for ConnectError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    DuplicateNode(NodeId),
    DuplicateEdge(EdgeId),
    DuplicateHandle {
        node: NodeId,
        handle: HandleId,
        kind: HandleKind,
    },
    DuplicateConnection(Connection),
    MissingParent {
        node: NodeId,
        parent: NodeId,
    },
    ParentCycle(NodeId),
    InvalidConnection(ConnectError),
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(formatter, "node id `{id}` is duplicated"),
            Self::DuplicateEdge(id) => write!(formatter, "edge id `{id}` is duplicated"),
            Self::DuplicateHandle { node, handle, kind } => write!(
                formatter,
                "{kind:?} handle id `{handle}` is duplicated on node `{node}`"
            ),
            Self::DuplicateConnection(connection) => write!(
                formatter,
                "connection from `{}` to `{}` already exists",
                connection.source, connection.target
            ),
            Self::MissingParent { node, parent } => {
                write!(
                    formatter,
                    "parent node `{parent}` for `{node}` does not exist"
                )
            }
            Self::ParentCycle(node) => {
                write!(formatter, "node `{node}` participates in a parent cycle")
            }
            Self::InvalidConnection(error) => error.fmt(formatter),
        }
    }
}

impl Error for GraphError {}

impl From<ConnectError> for GraphError {
    fn from(value: ConnectError) -> Self {
        Self::InvalidConnection(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemovedNode<N, E> {
    pub node: Node<N>,
    pub descendants: Vec<Node<N>>,
    pub edges: Vec<Edge<E>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphModel<N = (), E = ()> {
    pub nodes: Vec<Node<N>>,
    pub edges: Vec<Edge<E>>,
}

impl<N, E> GraphModel<N, E> {
    pub fn new(nodes: Vec<Node<N>>, edges: Vec<Edge<E>>) -> Result<Self, GraphError> {
        let model = Self { nodes, edges };
        model.validate()?;
        Ok(model)
    }

    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        let mut node_indices = HashMap::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            if node_indices.insert(&node.id, index).is_some() {
                return Err(GraphError::DuplicateNode(node.id.clone()));
            }
            let mut handles = HashSet::with_capacity(node.handles.len());
            for handle in &node.handles {
                if !handles.insert((&handle.id, handle.kind)) {
                    return Err(GraphError::DuplicateHandle {
                        node: node.id.clone(),
                        handle: handle.id.clone(),
                        kind: handle.kind,
                    });
                }
            }
        }

        for node in &self.nodes {
            if let Some(parent) = &node.parent_id
                && !node_indices.contains_key(parent)
            {
                return Err(GraphError::MissingParent {
                    node: node.id.clone(),
                    parent: parent.clone(),
                });
            }
        }

        let mut parent_states = vec![0_u8; self.nodes.len()];
        for start in 0..self.nodes.len() {
            if parent_states[start] != 0 {
                continue;
            }
            let mut path = Vec::new();
            let mut current = Some(start);
            while let Some(index) = current {
                match parent_states[index] {
                    0 => {
                        parent_states[index] = 1;
                        path.push(index);
                        current = self.nodes[index]
                            .parent_id
                            .as_ref()
                            .and_then(|parent| node_indices.get(parent).copied());
                    }
                    1 => return Err(GraphError::ParentCycle(self.nodes[index].id.clone())),
                    _ => break,
                }
            }
            for index in path {
                parent_states[index] = 2;
            }
        }

        let mut edge_ids = HashSet::with_capacity(self.edges.len());
        for edge in &self.edges {
            if !edge_ids.insert(&edge.id) {
                return Err(GraphError::DuplicateEdge(edge.id.clone()));
            }
            let source = node_indices
                .get(&edge.source)
                .map(|index| &self.nodes[*index])
                .ok_or_else(|| ConnectError::MissingSource(edge.source.clone()))?;
            let target = node_indices
                .get(&edge.target)
                .map(|index| &self.nodes[*index])
                .ok_or_else(|| ConnectError::MissingTarget(edge.target.clone()))?;
            validate_handle(source, edge.source_handle.as_ref(), HandleKind::Source)?;
            validate_handle(target, edge.target_handle.as_ref(), HandleKind::Target)?;
        }
        Ok(())
    }

    pub fn node(&self, id: &NodeId) -> Option<&Node<N>> {
        self.nodes.iter().find(|node| node.id == *id)
    }

    pub fn node_mut(&mut self, id: &NodeId) -> Option<&mut Node<N>> {
        self.nodes.iter_mut().find(|node| node.id == *id)
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&Edge<E>> {
        self.edges.iter().find(|edge| edge.id == *id)
    }

    pub fn edge_mut(&mut self, id: &EdgeId) -> Option<&mut Edge<E>> {
        self.edges.iter_mut().find(|edge| edge.id == *id)
    }

    pub fn add_node(&mut self, node: Node<N>) -> Result<(), GraphError> {
        if self.node(&node.id).is_some() {
            return Err(GraphError::DuplicateNode(node.id));
        }
        if let Some(parent) = &node.parent_id
            && self.node(parent).is_none()
        {
            return Err(GraphError::MissingParent {
                node: node.id,
                parent: parent.clone(),
            });
        }
        for (index, handle) in node.handles.iter().enumerate() {
            if node.handles[..index]
                .iter()
                .any(|other| other.id == handle.id && other.kind == handle.kind)
            {
                return Err(GraphError::DuplicateHandle {
                    node: node.id.clone(),
                    handle: handle.id.clone(),
                    kind: handle.kind,
                });
            }
        }
        self.nodes.push(node);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: Edge<E>) -> Result<(), GraphError> {
        if self.edge(&edge.id).is_some() {
            return Err(GraphError::DuplicateEdge(edge.id));
        }
        self.validate_connection(&edge.connection())?;
        self.edges.push(edge);
        Ok(())
    }

    pub fn add_edge_unique(&mut self, edge: Edge<E>) -> Result<(), GraphError> {
        let connection = edge.connection();
        if self.connection_exists(&connection) {
            return Err(GraphError::DuplicateConnection(connection));
        }
        self.add_edge(edge)
    }

    pub fn remove_node(&mut self, id: &NodeId) -> Option<RemovedNode<N, E>> {
        self.node(id)?;
        let removed_ids = std::iter::once(id.clone())
            .chain(self.descendants(id).into_iter().map(|node| node.id.clone()))
            .collect::<HashSet<_>>();
        let mut removed_nodes = Vec::new();
        let mut retained_nodes = Vec::with_capacity(self.nodes.len());
        for node in self.nodes.drain(..) {
            if removed_ids.contains(&node.id) {
                removed_nodes.push(node);
            } else {
                retained_nodes.push(node);
            }
        }
        self.nodes = retained_nodes;
        let root_index = removed_nodes.iter().position(|node| node.id == *id)?;
        let node = removed_nodes.remove(root_index);
        let mut removed_edges = Vec::new();
        let mut retained_edges = Vec::with_capacity(self.edges.len());
        for edge in self.edges.drain(..) {
            if removed_ids.contains(&edge.source) || removed_ids.contains(&edge.target) {
                removed_edges.push(edge);
            } else {
                retained_edges.push(edge);
            }
        }
        self.edges = retained_edges;
        Some(RemovedNode {
            node,
            descendants: removed_nodes,
            edges: removed_edges,
        })
    }

    pub fn remove_edge(&mut self, id: &EdgeId) -> Option<Edge<E>> {
        let index = self.edges.iter().position(|edge| edge.id == *id)?;
        Some(self.edges.remove(index))
    }

    pub fn validate_connection(&self, connection: &Connection) -> Result<(), ConnectError> {
        let source = self
            .node(&connection.source)
            .ok_or_else(|| ConnectError::MissingSource(connection.source.clone()))?;
        let target = self
            .node(&connection.target)
            .ok_or_else(|| ConnectError::MissingTarget(connection.target.clone()))?;

        validate_handle(
            source,
            connection.source_handle.as_ref(),
            HandleKind::Source,
        )?;
        validate_handle(
            target,
            connection.target_handle.as_ref(),
            HandleKind::Target,
        )?;
        Ok(())
    }

    pub fn connection_exists(&self, connection: &Connection) -> bool {
        self.edges
            .iter()
            .any(|edge| edge.connection() == *connection)
    }

    pub fn incoming_edges(&self, node: &NodeId) -> Vec<&Edge<E>> {
        self.edges
            .iter()
            .filter(|edge| edge.target == *node)
            .collect()
    }

    pub fn outgoing_edges(&self, node: &NodeId) -> Vec<&Edge<E>> {
        self.edges
            .iter()
            .filter(|edge| edge.source == *node)
            .collect()
    }

    pub fn connected_edges(&self, nodes: &[NodeId]) -> Vec<&Edge<E>> {
        let nodes = nodes.iter().collect::<HashSet<_>>();
        self.edges
            .iter()
            .filter(|edge| nodes.contains(&edge.source) || nodes.contains(&edge.target))
            .collect()
    }

    pub fn handle_connections(
        &self,
        node: &NodeId,
        kind: HandleKind,
        handle: Option<&HandleId>,
    ) -> Vec<&Edge<E>> {
        self.edges
            .iter()
            .filter(|edge| match kind {
                HandleKind::Source => {
                    edge.source == *node
                        && handle.is_none_or(|handle| edge.source_handle.as_ref() == Some(handle))
                }
                HandleKind::Target => {
                    edge.target == *node
                        && handle.is_none_or(|handle| edge.target_handle.as_ref() == Some(handle))
                }
            })
            .collect()
    }

    pub fn incomers(&self, node: &NodeId) -> Vec<&Node<N>> {
        let ids = self
            .incoming_edges(node)
            .into_iter()
            .map(|edge| &edge.source)
            .collect::<HashSet<_>>();
        self.nodes
            .iter()
            .filter(|candidate| ids.contains(&candidate.id))
            .collect()
    }

    pub fn outgoers(&self, node: &NodeId) -> Vec<&Node<N>> {
        let ids = self
            .outgoing_edges(node)
            .into_iter()
            .map(|edge| &edge.target)
            .collect::<HashSet<_>>();
        self.nodes
            .iter()
            .filter(|candidate| ids.contains(&candidate.id))
            .collect()
    }

    pub fn intersecting_nodes(&self, area: Rect, partially: bool) -> Vec<&Node<N>> {
        self.nodes
            .iter()
            .filter(|node| {
                if node.hidden {
                    return false;
                }
                let Some(bounds) = self.node_bounds(node) else {
                    return false;
                };
                if partially {
                    bounds.intersection(area).is_some()
                } else {
                    area.contains(bounds.origin)
                        && area.contains(Point::new(bounds.max_x(), bounds.max_y()))
                }
            })
            .collect()
    }

    pub fn bounds_for_nodes(&self, ids: &[NodeId]) -> Option<Rect> {
        let ids = ids.iter().collect::<HashSet<_>>();
        self.nodes
            .iter()
            .filter(|node| ids.contains(&node.id) && !node.hidden)
            .filter_map(|node| self.node_bounds(node))
            .reduce(Rect::union)
    }

    pub fn bounds(&self) -> Option<Rect> {
        self.nodes
            .iter()
            .filter(|node| !node.hidden)
            .filter_map(|node| self.node_bounds(node))
            .reduce(Rect::union)
    }

    pub fn node_bounds(&self, node: &Node<N>) -> Option<Rect> {
        let mut origin = node.bounds().origin;
        let mut current = node.parent_id.as_ref();
        let mut seen = HashSet::new();
        while let Some(parent_id) = current {
            if !seen.insert(parent_id) {
                return None;
            }
            let parent = self.node(parent_id)?;
            origin += parent.bounds().origin.to_vector();
            current = parent.parent_id.as_ref();
        }
        Some(Rect::from_origin_size(origin, node.size))
    }

    pub fn absolute_node_bounds(&self, id: &NodeId) -> Option<Rect> {
        self.node(id).and_then(|node| self.node_bounds(node))
    }

    pub fn descendants(&self, id: &NodeId) -> Vec<&Node<N>> {
        self.nodes
            .iter()
            .filter(|candidate| {
                let mut current = candidate.parent_id.as_ref();
                let mut seen = HashSet::new();
                while let Some(parent) = current {
                    if parent == id {
                        return true;
                    }
                    if !seen.insert(parent) {
                        return false;
                    }
                    current = self.node(parent).and_then(|node| node.parent_id.as_ref());
                }
                false
            })
            .collect()
    }

    pub fn node_depth(&self, id: &NodeId) -> usize {
        let mut depth = 0;
        let mut current = self.node(id).and_then(|node| node.parent_id.as_ref());
        let mut seen = HashSet::new();
        while let Some(parent) = current {
            if !seen.insert(parent) {
                break;
            }
            depth += 1;
            current = self.node(parent).and_then(|node| node.parent_id.as_ref());
        }
        depth
    }

    pub fn move_node(&mut self, id: &NodeId, proposed: Point) -> Option<Point> {
        let child_index = self.nodes.iter().position(|node| node.id == *id)?;
        let child_size = self.nodes[child_index].size;
        let child_origin = self.nodes[child_index].origin;
        let parent_id = self.nodes[child_index].parent_id.clone();
        let expand_parent = self.nodes[child_index].expand_parent;
        let extent = self.nodes[child_index].extent;
        let mut adjusted = proposed;

        if expand_parent
            && let Some(parent_id) = &parent_id
            && let Some(parent_index) = self.nodes.iter().position(|node| node.id == *parent_id)
        {
            let child_left = proposed.x - (child_size.width * child_origin.x);
            let child_top = proposed.y - (child_size.height * child_origin.y);
            let shift_x = child_left.min(0.0);
            let shift_y = child_top.min(0.0);
            adjusted = Point::new(proposed.x - shift_x, proposed.y - shift_y);

            let parent = &self.nodes[parent_index];
            let parent_top_left = parent.bounds().origin;
            let mut parent_size = parent.size;
            parent_size.width = parent_size
                .width
                .max((adjusted.x - child_size.width * child_origin.x) + child_size.width)
                - shift_x;
            parent_size.height = parent_size
                .height
                .max((adjusted.y - child_size.height * child_origin.y) + child_size.height)
                - shift_y;
            let parent_origin = parent.origin;

            if shift_x != 0.0 || shift_y != 0.0 {
                for node in self
                    .nodes
                    .iter_mut()
                    .filter(|node| node.parent_id.as_ref() == Some(parent_id))
                {
                    node.position.x -= shift_x;
                    node.position.y -= shift_y;
                }
            }
            let parent = &mut self.nodes[parent_index];
            parent.size = parent_size;
            parent.position = Point::new(
                parent_top_left.x + shift_x + (parent_size.width * parent_origin.x),
                parent_top_left.y + shift_y + (parent_size.height * parent_origin.y),
            );
        }

        let constraint = match extent {
            NodeExtent::Unbounded => None,
            NodeExtent::Rect(rect) => Some(rect),
            NodeExtent::Parent if !expand_parent => parent_id
                .as_ref()
                .and_then(|parent| self.node(parent))
                .map(|parent| Rect::from_origin_size(Point::ZERO, parent.size)),
            NodeExtent::Parent => None,
        };
        if let Some(rect) = constraint {
            let min_x = rect.x() + (child_size.width * child_origin.x);
            let min_y = rect.y() + (child_size.height * child_origin.y);
            let max_x = (rect.max_x() - child_size.width * (1.0 - child_origin.x)).max(min_x);
            let max_y = (rect.max_y() - child_size.height * (1.0 - child_origin.y)).max(min_y);
            adjusted.x = adjusted.x.clamp(min_x, max_x);
            adjusted.y = adjusted.y.clamp(min_y, max_y);
        }
        self.nodes[child_index].position = adjusted;
        Some(adjusted)
    }

    pub fn resize_node(
        &mut self,
        id: &NodeId,
        position: Point,
        size: Size,
    ) -> Option<(Point, Size)> {
        let node = self.node_mut(id)?;
        let size = Size::new(
            size.width.clamp(node.min_size.width, node.max_size.width),
            size.height
                .clamp(node.min_size.height, node.max_size.height),
        );
        node.position = position;
        node.size = size;
        node.size_mode = NodeSizeMode::Fixed;
        Some((position, size))
    }

    pub fn clear_selection(&mut self) -> bool {
        let mut changed = false;
        for node in &mut self.nodes {
            changed |= std::mem::take(&mut node.selected);
        }
        for edge in &mut self.edges {
            changed |= std::mem::take(&mut edge.selected);
        }
        changed
    }

    pub fn selected_node_ids(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|node| node.selected)
            .map(|node| node.id.clone())
            .collect()
    }

    pub fn selected_edge_ids(&self) -> Vec<EdgeId> {
        self.edges
            .iter()
            .filter(|edge| edge.selected)
            .map(|edge| edge.id.clone())
            .collect()
    }
}

fn validate_handle<N>(
    node: &Node<N>,
    requested: Option<&HandleId>,
    kind: HandleKind,
) -> Result<(), ConnectError> {
    let handle = if let Some(id) = requested {
        node.handle_by_id(id, kind).ok_or_else(|| match kind {
            HandleKind::Source => ConnectError::MissingSourceHandle {
                node: node.id.clone(),
                handle: id.clone(),
            },
            HandleKind::Target => ConnectError::MissingTargetHandle {
                node: node.id.clone(),
                handle: id.clone(),
            },
        })?
    } else if let Some(handle) = node.first_handle(kind) {
        handle
    } else {
        return Ok(());
    };

    if handle.connectable {
        Ok(())
    } else {
        Err(match kind {
            HandleKind::Source => ConnectError::SourceHandleDisabled {
                node: node.id.clone(),
                handle: handle.id.clone(),
            },
            HandleKind::Target => ConnectError::TargetHandleDisabled {
                node: node.id.clone(),
                handle: handle.id.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, x: f32) -> Node<()> {
        Node::new(id, Point::new(x, 20.0), ()).label(format!("Node {id}"))
    }

    #[test]
    fn model_rejects_dangling_edges() {
        let error = GraphModel::new(vec![node("a", 0.0)], vec![Edge::new("a-b", "a", "b", ())])
            .unwrap_err();

        assert_eq!(
            error,
            GraphError::InvalidConnection(ConnectError::MissingTarget(NodeId::from("b")))
        );
    }

    #[test]
    fn removing_a_node_cascades_connected_edges() {
        let mut model = GraphModel::new(
            vec![node("a", 0.0), node("b", 240.0), node("c", 480.0)],
            vec![
                Edge::new("a-b", "a", "b", ()),
                Edge::new("b-c", "b", "c", ()),
                Edge::new("a-c", "a", "c", ()),
            ],
        )
        .unwrap();

        let removed = model.remove_node(&NodeId::from("b")).unwrap();

        assert_eq!(removed.edges.len(), 2);
        assert_eq!(model.edges.len(), 1);
        assert_eq!(model.edges[0].id, EdgeId::from("a-c"));
    }

    #[test]
    fn removing_parent_cascades_descendants() {
        let mut model = GraphModel::<(), ()>::new(
            vec![
                Node::new("parent", Point::ZERO, ()),
                Node::new("child", Point::ZERO, ()).parent("parent"),
                Node::new("grandchild", Point::ZERO, ()).parent("child"),
            ],
            Vec::new(),
        )
        .unwrap();

        let removed = model.remove_node(&NodeId::from("parent")).unwrap();

        assert_eq!(removed.descendants.len(), 2);
        assert!(model.nodes.is_empty());
    }

    #[test]
    fn connection_checks_handle_direction_and_connectability() {
        let disabled = Handle::target("input", HandlePosition::Left).connectable(false);
        let model = GraphModel::new(
            vec![
                node("source", 0.0),
                node("target", 240.0).handles([disabled]),
            ],
            Vec::<Edge<()>>::new(),
        )
        .unwrap();

        let error = model
            .validate_connection(&Connection::new("source", "target").handles("source", "input"))
            .unwrap_err();

        assert_eq!(
            error,
            ConnectError::TargetHandleDisabled {
                node: NodeId::from("target"),
                handle: HandleId::from("input"),
            }
        );
    }

    #[test]
    fn graph_bounds_ignore_hidden_nodes() {
        let model = GraphModel::<(), ()> {
            nodes: vec![node("a", 10.0), node("b", 400.0).hidden(true)],
            edges: Vec::new(),
        };

        assert_eq!(model.bounds(), Some(Rect::new(10.0, 20.0, 180.0, 72.0)));
    }

    #[test]
    fn graph_queries_connections_and_intersections() {
        let model = GraphModel::new(
            vec![node("a", 0.0), node("b", 240.0), node("c", 480.0)],
            vec![
                Edge::new("a-b", "a", "b", ()),
                Edge::new("b-c", "b", "c", ()),
            ],
        )
        .unwrap();

        assert_eq!(model.incoming_edges(&NodeId::from("b")).len(), 1);
        assert_eq!(model.outgoing_edges(&NodeId::from("b")).len(), 1);
        assert_eq!(model.incomers(&NodeId::from("b"))[0].id, NodeId::from("a"));
        assert_eq!(model.outgoers(&NodeId::from("b"))[0].id, NodeId::from("c"));
        assert_eq!(
            model
                .intersecting_nodes(Rect::new(0.0, 0.0, 430.0, 120.0), false)
                .len(),
            2
        );
    }

    #[test]
    fn subflow_children_use_parent_relative_coordinates() {
        let model = GraphModel::<(), ()>::new(
            vec![
                Node::new("group", Point::new(100.0, 80.0), ()).size(Size::new(400.0, 300.0)),
                Node::new("child", Point::new(24.0, 36.0), ())
                    .parent("group")
                    .extent(NodeExtent::Parent),
            ],
            Vec::new(),
        )
        .unwrap();

        assert_eq!(
            model.absolute_node_bounds(&NodeId::from("child")),
            Some(Rect::new(124.0, 116.0, 180.0, 72.0))
        );
        assert_eq!(model.descendants(&NodeId::from("group")).len(), 1);
    }

    #[test]
    fn model_rejects_parent_cycles() {
        let error = GraphModel::<(), ()>::new(
            vec![
                Node::new("a", Point::ZERO, ()).parent("b"),
                Node::new("b", Point::ZERO, ()).parent("a"),
            ],
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(error, GraphError::ParentCycle(NodeId::from("a")));
    }

    #[test]
    fn parent_extent_clamps_and_expand_parent_grows() {
        let mut clamped = GraphModel::<(), ()>::new(
            vec![
                Node::new("parent", Point::ZERO, ()).size(Size::new(300.0, 200.0)),
                Node::new("child", Point::ZERO, ())
                    .size(Size::new(100.0, 80.0))
                    .parent("parent")
                    .extent(NodeExtent::Parent),
            ],
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            clamped.move_node(&NodeId::from("child"), Point::new(280.0, 180.0)),
            Some(Point::new(200.0, 120.0))
        );

        let mut expanded = GraphModel::<(), ()>::new(
            vec![
                Node::new("parent", Point::ZERO, ()).size(Size::new(300.0, 200.0)),
                Node::new("child", Point::ZERO, ())
                    .size(Size::new(100.0, 80.0))
                    .parent("parent")
                    .extent(NodeExtent::Parent)
                    .expand_parent(true),
            ],
            Vec::new(),
        )
        .unwrap();
        expanded.move_node(&NodeId::from("child"), Point::new(280.0, 180.0));
        assert_eq!(
            expanded.node(&NodeId::from("parent")).unwrap().size,
            Size::new(380.0, 260.0)
        );
    }
}
