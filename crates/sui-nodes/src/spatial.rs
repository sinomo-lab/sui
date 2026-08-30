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
        let mut index = Self {
            cell_size: DEFAULT_CELL_SIZE,
            revision,
            cells: HashMap::new(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            overflow_nodes: HashSet::new(),
            overflow_edges: HashSet::new(),
        };
        index.rebuild(graph, revision);
        index
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
        let mut ids = self
            .cells
            .get(&key)
            .map(|cell| cell.nodes.clone())
            .unwrap_or_default();
        ids.extend(self.overflow_nodes.iter().cloned());
        let mut indices = ids
            .into_iter()
            .filter_map(|id| self.nodes.get(&id))
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
        let after_node_ids = after
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<HashSet<_>>();
        let mut changed_nodes = HashSet::new();

        for id in self.nodes.keys().cloned().collect::<Vec<_>>() {
            if !after_node_ids.contains(&id) {
                self.remove_node(&id);
                changed_nodes.insert(id);
            }
        }
        for (index, node) in after.nodes.iter().enumerate() {
            let absolute_bounds = after.node_bounds(node);
            let changed = before_nodes
                .get(&node.id)
                .is_none_or(|before| *before != node)
                || self.nodes.get(&node.id).map(|entry| entry.bounds) != absolute_bounds;
            let index_changed = self
                .nodes
                .get(&node.id)
                .is_none_or(|entry| entry.index != index);
            if changed {
                self.upsert_node(after, node, index);
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
                self.upsert_edge(after, edge, index);
            } else if index_changed && let Some(entry) = self.edges.get_mut(&edge.id) {
                entry.index = index;
            }
        }
        self.revision = revision;
    }

    fn rebuild<N, E>(&mut self, graph: &GraphModel<N, E>, revision: u64) {
        self.cells.clear();
        self.nodes.clear();
        self.edges.clear();
        self.overflow_nodes.clear();
        self.overflow_edges.clear();
        for (index, node) in graph.nodes.iter().enumerate() {
            self.upsert_node(graph, node, index);
        }
        for (index, edge) in graph.edges.iter().enumerate() {
            self.upsert_edge(graph, edge, index);
        }
        self.revision = revision;
    }

    fn upsert_node<N, E>(&mut self, graph: &GraphModel<N, E>, node: &Node<N>, index: usize) {
        self.remove_node(&node.id);
        if node.hidden {
            return;
        }
        let Some(bounds) = graph.node_bounds(node) else {
            return;
        };
        let cells = self.cells_for_rect(bounds);
        if let Some(cells) = &cells {
            for key in cells {
                self.cells
                    .entry(*key)
                    .or_default()
                    .nodes
                    .push(node.id.clone());
            }
        } else {
            self.overflow_nodes.insert(node.id.clone());
        }
        self.nodes.insert(
            node.id.clone(),
            SpatialEntry {
                bounds,
                index,
                cells,
            },
        );
    }

    fn upsert_edge<N, E>(&mut self, graph: &GraphModel<N, E>, edge: &Edge<E>, index: usize) {
        self.remove_edge(&edge.id);
        if edge.hidden {
            return;
        }
        let Some(bounds) = edge_flow_bounds(graph, edge) else {
            return;
        };
        let cells = self.cells_for_rect(bounds);
        if let Some(cells) = &cells {
            for key in cells {
                self.cells
                    .entry(*key)
                    .or_default()
                    .edges
                    .push(edge.id.clone());
            }
        } else {
            self.overflow_edges.insert(edge.id.clone());
        }
        self.edges.insert(
            edge.id.clone(),
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
                        ids.extend(cell.nodes.iter().cloned());
                    }
                }
            } else {
                ids.extend(self.nodes.keys().cloned());
            }
            ids.extend(self.overflow_nodes.iter().cloned());
            let mut indices = ids
                .into_iter()
                .filter_map(|id| self.nodes.get(&id))
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
                        ids.extend(cell.edges.iter().cloned());
                    }
                }
            } else {
                ids.extend(self.edges.keys().cloned());
            }
            ids.extend(self.overflow_edges.iter().cloned());
            let mut indices = ids
                .into_iter()
                .filter_map(|id| self.edges.get(&id))
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

fn edge_flow_bounds<N, E>(graph: &GraphModel<N, E>, edge: &Edge<E>) -> Option<Rect> {
    let source_node = graph.node(&edge.source)?;
    let target_node = graph.node(&edge.target)?;
    if source_node.hidden || target_node.hidden {
        return None;
    }
    let source_bounds = graph.node_bounds(source_node)?;
    let target_bounds = graph.node_bounds(target_node)?;
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
    if matches!(edge.kind, EdgeKind::Bezier | EdgeKind::SimpleBezier) {
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

fn vector_length(vector: Vector) -> f32 {
    ((vector.x * vector.x) + (vector.y * vector.y)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Node};

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
}
