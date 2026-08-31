use std::collections::{HashMap, HashSet};

use sui_core::{Point, Rect, Vector};

use crate::{Edge, EdgeId, EdgeKind, GraphModel, Handle, HandleKind, HandlePosition, Node, NodeId};

const DEFAULT_CELL_SIZE: f32 = 256.0;
const MAX_CELLS_PER_ITEM: usize = 1024;
const MAX_QUERY_CELLS: usize = 4096;

type CellKey = (i32, i32);

#[derive(Debug, Clone, Default)]
struct SpatialCell {
    nodes: Vec<NodeId>,
    edges: Vec<EdgeId>,
}

#[derive(Debug, Clone)]
struct SpatialEntry {
    bounds: Rect,
    index: usize,
    cells: Option<Vec<CellKey>>,
}

#[derive(Debug, Clone)]
struct PendingSpatialEntry<I> {
    id: I,
    bounds: Rect,
    index: usize,
}

/// Progress returned by a budgeted spatial-index build step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphSpatialIndexBuildProgress {
    pub completed: usize,
    pub total: usize,
}

impl GraphSpatialIndexBuildProgress {
    pub const fn is_complete(self) -> bool {
        self.completed >= self.total
    }
}

/// UI-thread-friendly incremental builder for a graph spatial index.
///
/// Geometry is resolved once when the builder is created; grid insertion can
/// then be limited to a caller-selected number of nodes or edges per turn.
#[derive(Debug)]
pub struct GraphSpatialIndexBuilder {
    index: GraphSpatialIndex,
    nodes: std::vec::IntoIter<PendingSpatialEntry<NodeId>>,
    edges: std::vec::IntoIter<PendingSpatialEntry<EdgeId>>,
    completed: usize,
    total: usize,
}

/// Revisioned uniform-grid index used for graph culling and hit testing.
#[derive(Debug, Clone)]
pub struct GraphSpatialIndex {
    cell_size: f32,
    revision: u64,
    cells: HashMap<CellKey, SpatialCell>,
    nodes: HashMap<NodeId, SpatialEntry>,
    edges: HashMap<EdgeId, SpatialEntry>,
    overflow_nodes: HashSet<NodeId>,
    overflow_edges: HashSet<EdgeId>,
}

impl GraphSpatialIndex {
    pub fn new<N, E>(graph: &GraphModel<N, E>, revision: u64) -> Self {
        Self::builder(graph, revision).finish()
    }

    pub fn builder<N, E>(graph: &GraphModel<N, E>, revision: u64) -> GraphSpatialIndexBuilder {
        GraphSpatialIndexBuilder::new(graph, revision)
    }

    fn empty(revision: u64) -> Self {
        Self {
            cell_size: DEFAULT_CELL_SIZE,
            revision,
            cells: HashMap::new(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            overflow_nodes: HashSet::new(),
            overflow_edges: HashSet::new(),
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn node_bounds(&self, id: &NodeId) -> Option<Rect> {
        self.nodes.get(id).map(|entry| entry.bounds)
    }

    pub fn edge_bounds(&self, id: &EdgeId) -> Option<Rect> {
        self.edges.get(id).map(|entry| entry.bounds)
    }

    pub fn query_node_indices(&self, area: Rect) -> Vec<usize> {
        self.query_indices(area, true)
    }

    pub fn query_edge_indices(&self, area: Rect) -> Vec<usize> {
        self.query_indices(area, false)
    }

    pub fn query_node_indices_at(&self, point: Point) -> Vec<usize> {
        let key = self.cell_for(point);
        let mut ids = HashSet::new();
        if let Some(cell) = self.cells.get(&key) {
            ids.extend(&cell.nodes);
        }
        ids.extend(&self.overflow_nodes);
        let mut indices = ids
            .into_iter()
            .filter_map(|id| self.nodes.get(id))
            .filter(|entry| entry.bounds.contains(point))
            .map(|entry| entry.index)
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    pub fn update<N, E>(
        &mut self,
        before: &GraphModel<N, E>,
        after: &GraphModel<N, E>,
        revision: u64,
    ) where
        N: PartialEq,
        E: PartialEq,
    {
        let before_nodes = before
            .nodes
            .iter()
            .map(|node| (&node.id, node))
            .collect::<HashMap<_, _>>();
        let absolute_bounds = absolute_node_bounds(after);
        let after_nodes = after
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                absolute_bounds[index].map(|bounds| (&node.id, (node, bounds)))
            })
            .collect::<HashMap<_, _>>();
        let mut changed_nodes = HashSet::new();

        for id in self.nodes.keys().cloned().collect::<Vec<_>>() {
            if !after_nodes.contains_key(&id) {
                self.remove_node(&id);
                changed_nodes.insert(id);
            }
        }
        for (index, node) in after.nodes.iter().enumerate() {
            let node_bounds = absolute_bounds[index];
            let changed = before_nodes
                .get(&node.id)
                .is_none_or(|before| *before != node)
                || self.nodes.get(&node.id).map(|entry| entry.bounds) != node_bounds;
            let index_changed = self
                .nodes
                .get(&node.id)
                .is_none_or(|entry| entry.index != index);
            if changed {
                self.upsert_node_bounds(node, index, node_bounds);
                changed_nodes.insert(node.id.clone());
            } else if index_changed && let Some(entry) = self.nodes.get_mut(&node.id) {
                entry.index = index;
            }
        }

        let before_edges = before
            .edges
            .iter()
            .map(|edge| (&edge.id, edge))
            .collect::<HashMap<_, _>>();
        let after_edge_ids = after
            .edges
            .iter()
            .map(|edge| edge.id.clone())
            .collect::<HashSet<_>>();
        for id in self.edges.keys().cloned().collect::<Vec<_>>() {
            if !after_edge_ids.contains(&id) {
                self.remove_edge(&id);
            }
        }
        for (index, edge) in after.edges.iter().enumerate() {
            let edge_changed = before_edges
                .get(&edge.id)
                .is_none_or(|before| *before != edge);
            let endpoint_changed =
                changed_nodes.contains(&edge.source) || changed_nodes.contains(&edge.target);
            let index_changed = self
                .edges
                .get(&edge.id)
                .is_none_or(|entry| entry.index != index);
            if edge_changed || endpoint_changed {
                let bounds = edge_flow_bounds_from_nodes(&after_nodes, edge);
                self.upsert_edge_bounds(edge, index, bounds);
            } else if index_changed && let Some(entry) = self.edges.get_mut(&edge.id) {
                entry.index = index;
            }
        }
        self.revision = revision;
    }

    fn upsert_node_bounds<N>(&mut self, node: &Node<N>, index: usize, bounds: Option<Rect>) {
        self.remove_node(&node.id);
        if node.hidden {
            return;
        }
        let Some(bounds) = bounds else {
            return;
        };
        self.insert_node_entry(node.id.clone(), bounds, index);
    }

    fn insert_node_entry(&mut self, id: NodeId, bounds: Rect, index: usize) {
        let cells = self.cells_for_rect(bounds);
        if let Some(cells) = &cells {
            for key in cells {
                self.cells.entry(*key).or_default().nodes.push(id.clone());
            }
        } else {
            self.overflow_nodes.insert(id.clone());
        }
        self.nodes.insert(
            id,
            SpatialEntry {
                bounds,
                index,
                cells,
            },
        );
    }

    fn upsert_edge_bounds<E>(&mut self, edge: &Edge<E>, index: usize, bounds: Option<Rect>) {
        self.remove_edge(&edge.id);
        if edge.hidden {
            return;
        }
        let Some(bounds) = bounds else {
            return;
        };
        self.insert_edge_entry(edge.id.clone(), bounds, index);
    }

    fn insert_edge_entry(&mut self, id: EdgeId, bounds: Rect, index: usize) {
        let cells = self.cells_for_rect(bounds);
        if let Some(cells) = &cells {
            for key in cells {
                self.cells.entry(*key).or_default().edges.push(id.clone());
            }
        } else {
            self.overflow_edges.insert(id.clone());
        }
        self.edges.insert(
            id,
            SpatialEntry {
                bounds,
                index,
                cells,
            },
        );
    }

    fn remove_node(&mut self, id: &NodeId) {
        let Some(entry) = self.nodes.remove(id) else {
            return;
        };
        self.overflow_nodes.remove(id);
        if let Some(cells) = entry.cells {
            for key in cells {
                if let Some(cell) = self.cells.get_mut(&key) {
                    cell.nodes.retain(|candidate| candidate != id);
                }
                self.remove_empty_cell(key);
            }
        }
    }

    fn remove_edge(&mut self, id: &EdgeId) {
        let Some(entry) = self.edges.remove(id) else {
            return;
        };
        self.overflow_edges.remove(id);
        if let Some(cells) = entry.cells {
            for key in cells {
                if let Some(cell) = self.cells.get_mut(&key) {
                    cell.edges.retain(|candidate| candidate != id);
                }
                self.remove_empty_cell(key);
            }
        }
    }

    fn remove_empty_cell(&mut self, key: CellKey) {
        if self
            .cells
            .get(&key)
            .is_some_and(|cell| cell.nodes.is_empty() && cell.edges.is_empty())
        {
            self.cells.remove(&key);
        }
    }

    fn query_indices(&self, area: Rect, nodes: bool) -> Vec<usize> {
        let keys = self.cells_for_query(area);
        if nodes {
            let mut ids = HashSet::new();
            if let Some(keys) = keys {
                for key in keys {
                    if let Some(cell) = self.cells.get(&key) {
                        ids.extend(&cell.nodes);
                    }
                }
            } else {
                ids.extend(self.nodes.keys());
            }
            ids.extend(&self.overflow_nodes);
            let mut indices = ids
                .into_iter()
                .filter_map(|id| self.nodes.get(id))
                .filter(|entry| entry.bounds.intersection(area).is_some())
                .map(|entry| entry.index)
                .collect::<Vec<_>>();
            indices.sort_unstable();
            indices.dedup();
            indices
        } else {
            let mut ids = HashSet::new();
            if let Some(keys) = keys {
                for key in keys {
                    if let Some(cell) = self.cells.get(&key) {
                        ids.extend(&cell.edges);
                    }
                }
            } else {
                ids.extend(self.edges.keys());
            }
            ids.extend(&self.overflow_edges);
            let mut indices = ids
                .into_iter()
                .filter_map(|id| self.edges.get(id))
                .filter(|entry| entry.bounds.intersection(area).is_some())
                .map(|entry| entry.index)
                .collect::<Vec<_>>();
            indices.sort_unstable();
            indices.dedup();
            indices
        }
    }

    fn cell_for(&self, point: Point) -> CellKey {
        (
            (point.x / self.cell_size).floor() as i32,
            (point.y / self.cell_size).floor() as i32,
        )
    }

    fn cells_for_rect(&self, rect: Rect) -> Option<Vec<CellKey>> {
        self.cells_for_rect_limited(rect, MAX_CELLS_PER_ITEM)
    }

    fn cells_for_query(&self, rect: Rect) -> Option<Vec<CellKey>> {
        self.cells_for_rect_limited(rect, MAX_QUERY_CELLS)
    }

    fn cells_for_rect_limited(&self, rect: Rect, limit: usize) -> Option<Vec<CellKey>> {
        let min = self.cell_for(rect.origin);
        let max = self.cell_for(Point::new(rect.max_x(), rect.max_y()));
        let width = (i64::from(max.0) - i64::from(min.0) + 1).max(0) as usize;
        let height = (i64::from(max.1) - i64::from(min.1) + 1).max(0) as usize;
        if width.saturating_mul(height) > limit {
            return None;
        }
        let mut cells = Vec::with_capacity(width.saturating_mul(height));
        for y in min.1..=max.1 {
            for x in min.0..=max.0 {
                cells.push((x, y));
            }
        }
        Some(cells)
    }
}

impl GraphSpatialIndexBuilder {
    fn new<N, E>(graph: &GraphModel<N, E>, revision: u64) -> Self {
        let absolute_bounds = absolute_node_bounds(graph);
        let nodes_by_id = graph
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                absolute_bounds[index].map(|bounds| (&node.id, (node, bounds)))
            })
            .collect::<HashMap<_, _>>();
        let nodes = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| !node.hidden)
            .filter_map(|(index, node)| {
                absolute_bounds[index].map(|bounds| PendingSpatialEntry {
                    id: node.id.clone(),
                    bounds,
                    index,
                })
            })
            .collect::<Vec<_>>();
        let edges = graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| !edge.hidden)
            .filter_map(|(index, edge)| {
                edge_flow_bounds_from_nodes(&nodes_by_id, edge).map(|bounds| PendingSpatialEntry {
                    id: edge.id.clone(),
                    bounds,
                    index,
                })
            })
            .collect::<Vec<_>>();
        let total = nodes.len() + edges.len();
        Self {
            index: GraphSpatialIndex::empty(revision),
            nodes: nodes.into_iter(),
            edges: edges.into_iter(),
            completed: 0,
            total,
        }
    }

    pub const fn progress(&self) -> GraphSpatialIndexBuildProgress {
        GraphSpatialIndexBuildProgress {
            completed: self.completed,
            total: self.total,
        }
    }

    pub const fn index(&self) -> &GraphSpatialIndex {
        &self.index
    }

    /// Insert at most `max_items` nodes or edges and return current progress.
    pub fn advance(&mut self, max_items: usize) -> GraphSpatialIndexBuildProgress {
        let mut remaining = max_items;
        while remaining > 0 {
            if let Some(entry) = self.nodes.next() {
                self.index
                    .insert_node_entry(entry.id, entry.bounds, entry.index);
            } else if let Some(entry) = self.edges.next() {
                self.index
                    .insert_edge_entry(entry.id, entry.bounds, entry.index);
            } else {
                break;
            }
            self.completed += 1;
            remaining -= 1;
        }
        self.progress()
    }

    pub fn finish(mut self) -> GraphSpatialIndex {
        while !self.progress().is_complete() {
            self.advance(usize::MAX);
        }
        self.index
    }
}

fn absolute_node_bounds<N, E>(graph: &GraphModel<N, E>) -> Vec<Option<Rect>> {
    let node_indices = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (&node.id, index))
        .collect::<HashMap<_, _>>();
    let mut resolved: Vec<Option<Rect>> = vec![None; graph.nodes.len()];

    for start in 0..graph.nodes.len() {
        if resolved[start].is_some() {
            continue;
        }
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut current = Some(start);
        let base = loop {
            let Some(index) = current else {
                break Some(Point::ZERO);
            };
            if let Some(bounds) = resolved[index] {
                break Some(bounds.origin);
            }
            if !seen.insert(index) {
                break None;
            }
            chain.push(index);
            current = match graph.nodes[index].parent_id.as_ref() {
                Some(parent) => node_indices.get(parent).copied(),
                None => None,
            };
            if graph.nodes[index].parent_id.is_some() && current.is_none() {
                break None;
            }
        };
        let Some(mut origin) = base else {
            continue;
        };
        while let Some(index) = chain.pop() {
            let local = graph.nodes[index].bounds().origin;
            origin = Point::new(origin.x + local.x, origin.y + local.y);
            resolved[index] = Some(Rect::from_origin_size(origin, graph.nodes[index].size));
        }
    }
    resolved
}

fn edge_flow_bounds_from_nodes<N, E>(
    nodes: &HashMap<&NodeId, (&Node<N>, Rect)>,
    edge: &Edge<E>,
) -> Option<Rect> {
    let (source_node, source_bounds) = *nodes.get(&edge.source)?;
    let (target_node, target_bounds) = *nodes.get(&edge.target)?;
    if source_node.hidden || target_node.hidden {
        return None;
    }
    let (source, source_side) = endpoint(
        source_node,
        source_bounds,
        edge.source_handle.as_ref(),
        HandleKind::Source,
    );
    let (target, target_side) = endpoint(
        target_node,
        target_bounds,
        edge.target_handle.as_ref(),
        HandleKind::Target,
    );
    let mut bounds =
        Rect::new(source.x, source.y, 0.01, 0.01).union(Rect::new(target.x, target.y, 0.01, 0.01));
    if matches!(edge.kind, EdgeKind::Step | EdgeKind::SmoothStep)
        && (dot(target - source, side_direction(source_side)) < 0.0
            || dot(source - target, side_direction(target_side)) < 0.0)
    {
        let clearance = edge
            .path_options
            .step_offset
            .max(edge.path_options.border_radius + 8.0)
            .max(20.0);
        bounds = source_bounds
            .union(target_bounds)
            .inflate(clearance + 12.0, clearance + 12.0);
    } else if matches!(edge.kind, EdgeKind::Bezier | EdgeKind::SimpleBezier) {
        let distance = vector_length(target - source);
        let bend = (distance * 0.5).clamp(36.0, 180.0);
        let control_1 = source + scale(side_direction(source_side), bend);
        let control_2 = target + scale(side_direction(target_side), bend);
        bounds = bounds
            .union(Rect::new(control_1.x, control_1.y, 0.01, 0.01))
            .union(Rect::new(control_2.x, control_2.y, 0.01, 0.01));
    }
    Some(bounds.inflate(12.0, 12.0))
}

fn endpoint<N>(
    node: &Node<N>,
    bounds: Rect,
    requested: Option<&crate::HandleId>,
    kind: HandleKind,
) -> (Point, HandlePosition) {
    let handle = requested
        .and_then(|id| node.handle_by_id(id, kind))
        .or_else(|| node.first_handle(kind));
    handle.map_or_else(
        || match kind {
            HandleKind::Source => (
                Point::new(bounds.max_x(), bounds.y() + node.size.height * 0.5),
                HandlePosition::Right,
            ),
            HandleKind::Target => (
                Point::new(bounds.x(), bounds.y() + node.size.height * 0.5),
                HandlePosition::Left,
            ),
        },
        |handle| (handle_position(bounds, handle), handle.position),
    )
}

fn handle_position(bounds: Rect, handle: &Handle) -> Point {
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

fn side_direction(side: HandlePosition) -> Vector {
    match side {
        HandlePosition::Left => Vector::new(-1.0, 0.0),
        HandlePosition::Right => Vector::new(1.0, 0.0),
        HandlePosition::Top => Vector::new(0.0, -1.0),
        HandlePosition::Bottom => Vector::new(0.0, 1.0),
    }
}

fn scale(vector: Vector, factor: f32) -> Vector {
    Vector::new(vector.x * factor, vector.y * factor)
}

fn dot(first: Vector, second: Vector) -> f32 {
    (first.x * second.x) + (first.y * second.y)
}

fn vector_length(vector: Vector) -> f32 {
    ((vector.x * vector.x) + (vector.y * vector.y)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Node};
    use sui_core::Size;

    #[test]
    fn incremental_update_moves_only_changed_node_and_incident_edge() {
        let before = GraphModel::new(
            vec![
                Node::new("a", Point::new(0.0, 0.0), ()),
                Node::new("b", Point::new(600.0, 0.0), ()),
            ],
            vec![Edge::new("a-b", "a", "b", ())],
        )
        .unwrap();
        let mut after = before.clone();
        after.node_mut(&NodeId::from("a")).unwrap().position = Point::new(900.0, 500.0);
        let mut index = GraphSpatialIndex::new(&before, 1);

        index.update(&before, &after, 2);

        assert_eq!(index.revision(), 2);
        assert!(
            index
                .query_node_indices(Rect::new(850.0, 450.0, 300.0, 200.0))
                .contains(&0)
        );
        assert!(
            !index
                .query_node_indices(Rect::new(-20.0, -20.0, 300.0, 200.0))
                .contains(&0)
        );
        assert!(
            index
                .query_edge_indices(Rect::new(500.0, -100.0, 600.0, 700.0))
                .contains(&0)
        );
    }

    #[test]
    fn backward_step_edge_bounds_include_the_outside_detour() {
        let graph = GraphModel::new(
            vec![
                Node::new("source", Point::new(220.0, 20.0), ()).size(Size::new(100.0, 80.0)),
                Node::new("target", Point::new(20.0, 180.0), ()).size(Size::new(100.0, 80.0)),
            ],
            vec![Edge::new("backward", "source", "target", ()).kind(EdgeKind::SmoothStep)],
        )
        .unwrap();
        let index = GraphSpatialIndex::new(&graph, 1);

        assert!(
            index
                .query_edge_indices(Rect::new(140.0, -20.0, 40.0, 24.0))
                .contains(&0),
            "the spatial index must include the detour above the two nodes"
        );
    }

    #[test]
    fn budgeted_builder_exposes_monotonic_partial_progress() {
        let graph = GraphModel::new(
            vec![
                Node::new("a", Point::new(0.0, 0.0), ()),
                Node::new("b", Point::new(600.0, 0.0), ()),
            ],
            vec![Edge::new("a-b", "a", "b", ())],
        )
        .unwrap();
        let mut builder = GraphSpatialIndex::builder(&graph, 7);

        assert_eq!(
            builder.progress(),
            GraphSpatialIndexBuildProgress {
                completed: 0,
                total: 3,
            }
        );
        assert_eq!(builder.advance(1).completed, 1);
        assert_eq!(
            builder
                .index()
                .query_node_indices(Rect::new(-20.0, -20.0, 300.0, 200.0)),
            vec![0]
        );
        assert!(!builder.progress().is_complete());

        let index = builder.finish();
        assert_eq!(index.revision(), 7);
        assert_eq!(
            index.query_node_indices(Rect::new(-20.0, -20.0, 900.0, 200.0)),
            vec![0, 1]
        );
        assert_eq!(
            index.query_edge_indices(Rect::new(-20.0, -20.0, 900.0, 200.0)),
            vec![0]
        );
    }
}
