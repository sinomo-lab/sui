use std::{collections::HashMap, fmt};

use sui_reactive::Signal;
use sui_runtime::{
    PaintCtx, SemanticsCtx, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};

use crate::{Node, NodeId};

/// Observable model supplied to a retained custom node widget.
pub type NodeSignal<N> = Signal<Node<N>>;

type NodeWidgetFactory<N> = dyn FnMut(&NodeId, NodeSignal<N>) -> Box<dyn Widget> + 'static;

/// Registry of retained widget factories keyed by [`Node::kind`].
pub struct NodeWidgetRegistry<N> {
    factories: HashMap<String, Box<NodeWidgetFactory<N>>>,
}

impl<N> NodeWidgetRegistry<N> {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    pub fn register<F, W>(&mut self, kind: impl Into<String>, mut factory: F) -> bool
    where
        F: FnMut(&NodeId, NodeSignal<N>) -> W + 'static,
        W: Widget + 'static,
    {
        self.factories
            .insert(
                kind.into(),
                Box::new(move |id, node| Box::new(factory(id, node))),
            )
            .is_none()
    }

    pub fn with<F, W>(mut self, kind: impl Into<String>, factory: F) -> Self
    where
        F: FnMut(&NodeId, NodeSignal<N>) -> W + 'static,
        W: Widget + 'static,
    {
        self.register(kind, factory);
        self
    }

    pub fn contains(&self, kind: &str) -> bool {
        self.factories.contains_key(kind)
    }

    pub fn remove(&mut self, kind: &str) -> bool {
        self.factories.remove(kind).is_some()
    }

    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }

    fn build(&mut self, node: &Node<N>, signal: NodeSignal<N>) -> Option<WidgetPod> {
        self.factories
            .get_mut(&node.kind)
            .map(|factory| WidgetPod::new_boxed(factory(&node.id, signal)))
    }
}

impl<N> Default for NodeWidgetRegistry<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N> fmt::Debug for NodeWidgetRegistry<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut kinds = self
            .factories
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        kinds.sort_unstable();
        formatter
            .debug_struct("NodeWidgetRegistry")
            .field("kinds", &kinds)
            .finish()
    }
}

pub(crate) struct RetainedNodeWidget<N> {
    pub id: NodeId,
    kind: String,
    pub signal: NodeSignal<N>,
    pub pod: WidgetPod,
}

pub(crate) struct RetainedNodeWidgets<N> {
    entries: Vec<RetainedNodeWidget<N>>,
}

impl<N> Default for RetainedNodeWidgets<N> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<N> RetainedNodeWidgets<N>
where
    N: Clone + PartialEq + 'static,
{
    pub fn reconcile(&mut self, nodes: &[Node<N>], registry: &mut NodeWidgetRegistry<N>) -> bool {
        let previous_ids = self
            .entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.pod.id()))
            .collect::<Vec<_>>();
        let mut previous = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect::<HashMap<_, _>>();
        let mut next = Vec::new();

        for node in nodes {
            if !registry.contains(&node.kind) {
                continue;
            }
            if let Some(entry) = previous.remove(&node.id)
                && entry.kind == node.kind
            {
                entry.signal.set(node.clone());
                next.push(entry);
                continue;
            }

            let signal = Signal::named(format!("Node {}", node.id), node.clone());
            if let Some(pod) = registry.build(node, signal.clone()) {
                next.push(RetainedNodeWidget {
                    id: node.id.clone(),
                    kind: node.kind.clone(),
                    signal,
                    pod,
                });
            }
        }

        self.entries = next;
        let next_ids = self
            .entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.pod.id()))
            .collect::<Vec<_>>();
        previous_ids != next_ids
    }

    pub fn get(&self, id: &NodeId) -> Option<&RetainedNodeWidget<N>> {
        self.entries.iter().find(|entry| entry.id == *id)
    }

    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut RetainedNodeWidget<N>> {
        self.entries.iter_mut().find(|entry| entry.id == *id)
    }

    pub fn contains(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn paint_node(&self, ctx: &mut PaintCtx, id: &NodeId) {
        if let Some(entry) = self.get(id) {
            entry.pod.paint(ctx);
        }
    }

    pub fn semantics(&self, ctx: &mut SemanticsCtx) {
        for entry in &self.entries {
            entry.pod.semantics(ctx);
        }
    }

    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for entry in &self.entries {
            visitor.visit(&entry.pod);
        }
    }

    pub fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for entry in &mut self.entries {
            visitor.visit(&mut entry.pod);
        }
    }
}
