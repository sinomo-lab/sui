use std::collections::HashMap;

use sui_core::{SemanticsNode, WidgetId};
use sui_platform::AccessibilitySnapshot;

#[derive(Debug, Clone)]
pub struct TuiNode {
    pub node: SemanticsNode,
    pub depth: usize,
    pub children: Vec<TuiNode>,
}

#[derive(Debug, Clone)]
pub struct TuiSnapshot {
    pub roots: Vec<TuiNode>,
    pub flat: Vec<SemanticsNode>,
}

impl TuiSnapshot {
    pub fn from_accessibility(snapshot: &AccessibilitySnapshot, show_hidden: bool) -> Self {
        let mut nodes = snapshot
            .nodes
            .iter()
            .filter(|node| show_hidden || !node.state.hidden)
            .cloned()
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| node.id);

        let visible_ids = nodes.iter().map(|node| node.id).collect::<Vec<_>>();
        let nodes_by_id = nodes
            .iter()
            .cloned()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>();

        let mut children_by_parent = HashMap::<WidgetId, Vec<WidgetId>>::new();
        for node in &nodes {
            if let Some(parent) = node
                .parent
                .filter(|parent| nodes_by_id.contains_key(parent))
            {
                children_by_parent.entry(parent).or_default().push(node.id);
            }
        }
        for children in children_by_parent.values_mut() {
            children.sort();
        }

        let root_ids = visible_ids
            .iter()
            .copied()
            .filter(|id| {
                let parent = nodes_by_id.get(id).and_then(|node| node.parent);
                parent.is_none_or(|parent| !nodes_by_id.contains_key(&parent))
            })
            .collect::<Vec<_>>();

        let roots = root_ids
            .into_iter()
            .filter_map(|id| build_node(id, 0, &nodes_by_id, &children_by_parent))
            .collect();

        Self { roots, flat: nodes }
    }
}

fn build_node(
    id: WidgetId,
    depth: usize,
    nodes_by_id: &HashMap<WidgetId, SemanticsNode>,
    children_by_parent: &HashMap<WidgetId, Vec<WidgetId>>,
) -> Option<TuiNode> {
    let node = nodes_by_id.get(&id)?.clone();
    let children = children_by_parent
        .get(&id)
        .into_iter()
        .flat_map(|children| children.iter())
        .filter_map(|child| build_node(*child, depth + 1, nodes_by_id, children_by_parent))
        .collect();

    Some(TuiNode {
        node,
        depth,
        children,
    })
}
