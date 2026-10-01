//! The inspector: what is selected, and its settings. A node can be renamed
//! and shows what reaches each input; an edge can be restyled and animated.

use sui::prelude::*;
use sui_nodes::{EdgeId, GraphSnapshot, NodeId};

use super::lab::{LabNode, Op, evaluate};
use super::nodes::describe_value;
use super::{EDGE_STYLES, Page, count_of, style_index};
use crate::app::{DemoTextRole, clone_dev_theme_reader};
use crate::demo_support::{DemoTextColor, demo_label, demo_mono_label};

pub(super) const NODE_NAME: &str = "Node name";
pub(super) const INPUTS_NAME: &str = "Node inputs";
pub(super) const SELECTED_EDGE_STYLE: &str = "Selected edge style";
pub(super) const ANIMATED_LABEL: &str = "Animated";
pub(super) const SELECTION_NAME: &str = "Selection";

type Snapshot = GraphSnapshot<LabNode, ()>;

pub(super) fn inspector(page: &Page) -> impl Widget + use<> {
    let selection = page
        .state
        .observable()
        .select_named("Lab selection", |snapshot: &Snapshot| {
            (
                snapshot.graph.selected_node_ids(),
                snapshot.graph.selected_edge_ids(),
            )
        });
    let page = page.clone();
    RebuildOnChange::key_from(selection, move |(nodes, edges)| {
        match (nodes.as_slice(), edges.as_slice()) {
            ([], []) => WidgetPod::new(nothing_selected(&page)),
            ([node], []) => WidgetPod::new(node_inspector(&page, node)),
            ([], [edge]) => WidgetPod::new(edge_inspector(&page, edge)),
            (nodes, edges) => WidgetPod::new(many_selected(&page, nodes.len(), edges.len())),
        }
    })
}

fn nothing_selected(page: &Page) -> impl Widget + use<> {
    let summary = page
        .state
        .observable()
        .select_named("Lab summary", |snapshot: &Snapshot| {
            format!(
                "{} in the graph.",
                count_of(snapshot.graph.nodes.len(), snapshot.graph.edges.len())
            )
        });
    Stack::vertical()
        .gap(6.0)
        .alignment(Alignment::Stretch)
        .with_child(
            demo_label(
                &page.theme_reader,
                "Nothing selected. Click a node or an edge to edit it here.",
                DemoTextRole::Supporting,
                DemoTextColor::Muted,
            )
            .semantic_name(SELECTION_NAME),
        )
        .with_child(
            demo_label(
                &page.theme_reader,
                "",
                DemoTextRole::Metadata,
                DemoTextColor::Muted,
            )
            .text_from(summary),
        )
}

fn node_inspector(page: &Page, id: &NodeId) -> impl Widget + use<> {
    let Some(node) = page.state.node(id) else {
        return Stack::vertical();
    };
    let rename = page.state.clone();
    let rename_id = id.clone();
    let name = TextInput::new(NODE_NAME)
        .theme_when(clone_dev_theme_reader(&page.theme_reader))
        .value(node.label.clone())
        .on_change(move |text| {
            // Typing a name is one undo step, however many keys it takes.
            rename.merge_undo(format!("{rename_id} name"), || {
                let _ = rename.update_node(&rename_id, |node| node.label = text.to_string());
            });
        });
    let inputs = {
        let id = id.clone();
        page.state
            .observable()
            .select_named("Lab node inputs", move |snapshot: &Snapshot| {
                inputs_text(snapshot, &id)
            })
    };
    Stack::vertical()
        .gap(10.0)
        .alignment(Alignment::Stretch)
        .with_child(name)
        .with_child(demo_label(
            &page.theme_reader,
            match node.data.op {
                Op::Color { .. } => "Color: a hue and a lightness, set on the node",
                Op::Number { .. } => "Number: a value from 0 to 1, set on the node",
                Op::Mix => "Mix: blends a toward b by t",
                Op::Lighten => "Lighten: moves a color toward white",
                Op::Contrast => "Contrast: how well text reads on a background",
                Op::Preview => "Preview: shows a color large",
                Op::Gradient => "Gradient: the colors between two ends",
                Op::Group => "Group: holds the nodes inside it",
            },
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
        .with_child(
            demo_mono_label(&page.theme_reader, "", DemoTextRole::Metadata, |theme| {
                theme.palette.text
            })
            .semantic_name(INPUTS_NAME)
            .text_from(inputs),
        )
}

/// What reaches each of `id`'s inputs, and what it gives.
fn inputs_text(snapshot: &Snapshot, id: &NodeId) -> String {
    let graph = &snapshot.graph;
    let Some(node) = graph.node(id) else {
        return String::new();
    };
    let evaluation = evaluate(graph);
    let mut lines = node
        .data
        .op
        .inputs()
        .iter()
        .map(|port| {
            let source = graph
                .incoming_edges(id)
                .into_iter()
                .find(|edge| {
                    edge.target_handle.as_ref().map(|handle| handle.as_str()) == Some(port.id)
                })
                .and_then(|edge| graph.node(&edge.source))
                .map(|source| source.label.clone());
            match source {
                Some(source) => format!(
                    "{:<10} ← {source} ({})",
                    port.label,
                    describe_value(evaluation.input(id, port.id))
                ),
                None => format!("{:<10} not connected", port.label),
            }
        })
        .collect::<Vec<_>>();
    if let Some(output) = node.data.op.output() {
        lines.push(format!(
            "{:<10} → {}",
            output.label,
            describe_value(evaluation.output(id))
        ));
    }
    lines.join("\n")
}

fn edge_inspector(page: &Page, id: &EdgeId) -> impl Widget + use<> {
    let title = {
        let id = id.clone();
        page.state
            .observable()
            .select_named("Lab edge title", move |snapshot: &Snapshot| {
                let graph = &snapshot.graph;
                let Some(edge) = graph.edge(&id) else {
                    return String::new();
                };
                let label = |node: &NodeId| {
                    graph
                        .node(node)
                        .map_or_else(|| node.to_string(), |node| node.label.clone())
                };
                format!(
                    "{} › {} {}",
                    label(&edge.source),
                    label(&edge.target),
                    edge.target_handle
                        .as_ref()
                        .map_or("", |handle| handle.as_str())
                )
            })
    };
    let style = {
        let read = page.state.clone();
        let read_id = id.clone();
        let page = page.clone();
        Select::new(SELECTED_EDGE_STYLE)
            .theme_when(clone_dev_theme_reader(&page.theme_reader))
            .options(EDGE_STYLES.map(|(label, _)| label))
            .selected_when(move || read.edge(&read_id).map(|edge| style_index(edge.kind)))
            .on_change(move |index, _| page.set_edge_style(EDGE_STYLES[index].1))
    };
    let animated = {
        let read = page.state.clone();
        let read_id = id.clone();
        let write = page.state.clone();
        let write_id = id.clone();
        Switch::new(ANIMATED_LABEL)
            .theme_when(clone_dev_theme_reader(&page.theme_reader))
            .checked_when(move || read.edge(&read_id).is_some_and(|edge| edge.animated))
            .on_change(move |on| {
                let _ = write.update_edge(&write_id, |edge| edge.animated = on);
            })
    };
    Stack::vertical()
        .gap(10.0)
        .alignment(Alignment::Stretch)
        .with_child(
            demo_label(
                &page.theme_reader,
                "",
                DemoTextRole::Body,
                DemoTextColor::Text,
            )
            .semantic_name(SELECTION_NAME)
            .text_from(title),
        )
        .with_child(style)
        .with_child(animated)
}

fn many_selected(page: &Page, nodes: usize, edges: usize) -> impl Widget + use<> {
    demo_label(
        &page.theme_reader,
        format!(
            "{} selected. The toolbar and the context menu act on all of them.",
            count_of(nodes, edges)
        ),
        DemoTextRole::Body,
        DemoTextColor::Text,
    )
    .semantic_name(SELECTION_NAME)
}
