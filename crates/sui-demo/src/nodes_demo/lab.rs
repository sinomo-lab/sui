//! The Color lab's model: the kinds of node, the typed values that flow
//! between them, how each node computes its value, and which connections the
//! graph accepts.

use std::collections::{HashMap, HashSet};

use sui::{Color, Point, Size};
use sui_nodes::{
    Connection, Edge, EdgeKind, GraphDocument, GraphModel, GraphSnapshot, Handle, HandlePosition,
    Node, NodeExtent, NodeGraphState, NodeId, Viewport,
};

pub(super) type LabState = NodeGraphState<LabNode, ()>;
pub(super) type LabGraph = GraphModel<LabNode, ()>;

/// The height of a node's title row.
pub(super) const HEADER: f32 = 34.0;
/// The height of each input row below the title.
pub(super) const ROW: f32 = 26.0;
/// Chroma of the colors the Color node makes.
const INPUT_CHROMA: f32 = 0.15;

/// What flows along an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PortType {
    Color,
    Number,
}

impl PortType {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Number => "number",
        }
    }

    /// The handle color for the type, the same in every theme.
    pub(super) fn tint(self) -> Color {
        match self {
            Self::Color => Color::rgba(0.55, 0.36, 0.96, 1.0),
            Self::Number => Color::rgba(0.93, 0.6, 0.12, 1.0),
        }
    }
}

/// A value an output gives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Value {
    Color(Color),
    Number(f32),
}

impl Value {
    pub(super) fn text(self) -> String {
        match self {
            Self::Color(color) => hex(color),
            Self::Number(number) => format!("{number:.2}"),
        }
    }

    fn color(self) -> Option<Color> {
        match self {
            Self::Color(color) => Some(color),
            Self::Number(_) => None,
        }
    }

    fn number(self) -> Option<f32> {
        match self {
            Self::Number(number) => Some(number),
            Self::Color(_) => None,
        }
    }
}

/// `#RRGGBB` for a color.
pub(super) fn hex(color: Color) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(color.red),
        channel(color.green),
        channel(color.blue)
    )
}

/// An input or output on a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Port {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) ty: PortType,
}

const fn port(id: &'static str, label: &'static str, ty: PortType) -> Port {
    Port { id, label, ty }
}

/// What a node does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Op {
    /// A color from a hue and a lightness.
    Color { hue: f32, lightness: f32 },
    /// A number from 0 to 1.
    Number { value: f32 },
    /// Two colors blended by `t`.
    Mix,
    /// A color lightened by an amount.
    Lighten,
    /// The contrast ratio between text and background colors.
    Contrast,
    /// A color shown large.
    Preview,
    /// A gradient between two colors.
    Gradient,
    /// A frame that holds other nodes.
    Group,
}

pub(super) const KINDS: [Op; 7] = [
    Op::Color {
        hue: 200.0,
        lightness: 0.6,
    },
    Op::Number { value: 0.5 },
    Op::Mix,
    Op::Lighten,
    Op::Contrast,
    Op::Preview,
    Op::Gradient,
];

impl Op {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Color { .. } => "Color",
            Self::Number { .. } => "Number",
            Self::Mix => "Mix",
            Self::Lighten => "Lighten",
            Self::Contrast => "Contrast",
            Self::Preview => "Preview",
            Self::Gradient => "Gradient",
            Self::Group => "Group",
        }
    }

    /// The widget kind the graph builds the node with.
    pub(super) fn kind(self) -> &'static str {
        match self {
            Self::Color { .. } => "lab-color",
            Self::Number { .. } => "lab-number",
            Self::Mix => "lab-mix",
            Self::Lighten => "lab-lighten",
            Self::Contrast => "lab-contrast",
            Self::Preview => "lab-preview",
            Self::Gradient => "lab-gradient",
            Self::Group => "lab-group",
        }
    }

    pub(super) fn inputs(self) -> &'static [Port] {
        const MIX: [Port; 3] = [
            port("a", "a", PortType::Color),
            port("b", "b", PortType::Color),
            port("t", "t", PortType::Number),
        ];
        const LIGHTEN: [Port; 2] = [
            port("color", "color", PortType::Color),
            port("amount", "amount", PortType::Number),
        ];
        const CONTRAST: [Port; 2] = [
            port("text", "text", PortType::Color),
            port("background", "background", PortType::Color),
        ];
        const PREVIEW: [Port; 1] = [port("color", "color", PortType::Color)];
        const GRADIENT: [Port; 2] = [
            port("from", "from", PortType::Color),
            port("to", "to", PortType::Color),
        ];
        match self {
            Self::Color { .. } | Self::Number { .. } | Self::Group => &[],
            Self::Mix => &MIX,
            Self::Lighten => &LIGHTEN,
            Self::Contrast => &CONTRAST,
            Self::Preview => &PREVIEW,
            Self::Gradient => &GRADIENT,
        }
    }

    pub(super) fn output(self) -> Option<Port> {
        match self {
            Self::Color { .. } | Self::Mix | Self::Lighten => {
                Some(port("out", "color", PortType::Color))
            }
            Self::Number { .. } => Some(port("out", "value", PortType::Number)),
            Self::Contrast => Some(port("out", "ratio", PortType::Number)),
            Self::Preview | Self::Gradient | Self::Group => None,
        }
    }

    pub(super) fn size(self) -> Size {
        match self {
            Self::Color { .. } => Size::new(210.0, 112.0),
            Self::Number { .. } => Size::new(190.0, 100.0),
            Self::Mix => Size::new(200.0, HEADER + 3.0 * ROW + 12.0),
            Self::Lighten => Size::new(200.0, HEADER + 2.0 * ROW + 12.0),
            Self::Contrast => Size::new(220.0, HEADER + 2.0 * ROW + 12.0),
            Self::Preview => Size::new(180.0, 150.0),
            Self::Gradient => Size::new(230.0, HEADER + 2.0 * ROW + 40.0),
            Self::Group => Size::new(260.0, 290.0),
        }
    }

    /// Handles for the ports: inputs on the left, one per row below the
    /// title, and the output on the right beside the title.
    fn handles(self) -> Vec<Handle> {
        let size = self.size();
        let mut handles = self
            .inputs()
            .iter()
            .enumerate()
            .map(|(row, port)| {
                Handle::target(port.id, HandlePosition::Left)
                    .offset((HEADER + (row as f32 + 0.5) * ROW) / size.height)
                    .label(port.label)
                    .color(port.ty.tint())
            })
            .collect::<Vec<_>>();
        if let Some(port) = self.output() {
            handles.push(
                Handle::source(port.id, HandlePosition::Right)
                    .offset(HEADER * 0.5 / size.height)
                    .label(port.label)
                    .color(port.ty.tint()),
            );
        }
        handles
    }

    fn input(self, id: &str) -> Option<Port> {
        self.inputs().iter().copied().find(|port| port.id == id)
    }
}

/// A node's data: what it does.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LabNode {
    pub(super) op: Op,
}

/// A node of kind `op` at `position`.
pub(super) fn lab_node(
    id: impl Into<NodeId>,
    label: &str,
    op: Op,
    position: Point,
) -> Node<LabNode> {
    Node::new(id, position, LabNode { op })
        .label(label)
        .kind(op.kind())
        .size(op.size())
        .handles(op.handles())
}

/// An edge from `source`'s output to `target`'s `input`.
pub(super) fn lab_edge(source: &str, target: &str, input: &str, kind: EdgeKind) -> Edge<()> {
    Edge::new(format!("{source}-{target}-{input}"), source, target, ())
        .handles("out", input)
        .kind(kind)
}

/// Whether the lab accepts `connection`, and why not when it does not.
pub(super) fn connection_rule(connection: &Connection, graph: &LabGraph) -> Result<(), String> {
    let (Some(source), Some(target)) = (
        graph.node(&connection.source),
        graph.node(&connection.target),
    ) else {
        return Err("That node is gone".to_string());
    };
    let Some(output) = source.data.op.output() else {
        return Err(format!("{} has no output", source.label));
    };
    let input_id = connection
        .target_handle
        .as_ref()
        .map_or("", |handle| handle.as_str());
    let Some(input) = target.data.op.input(input_id) else {
        return Err(format!("{} has no input {input_id}", target.label));
    };
    if output.ty != input.ty {
        return Err(format!(
            "{} gives a {}, but {} takes a {}",
            source.label,
            output.ty.name(),
            input.label,
            input.ty.name()
        ));
    }
    if connection.source == connection.target {
        return Err("A node can't feed itself".to_string());
    }
    if feeds(graph, &connection.target, &connection.source) {
        return Err(format!(
            "That would loop: {} already feeds {}",
            target.label, source.label
        ));
    }
    if graph.edges.iter().any(|edge| {
        edge.target == connection.target && edge.target_handle == connection.target_handle
    }) {
        return Err(format!("{} already has an input", input.label));
    }
    Ok(())
}

/// Whether `from` feeds `to`, directly or through other nodes.
fn feeds(graph: &LabGraph, from: &NodeId, to: &NodeId) -> bool {
    let mut seen = HashSet::new();
    let mut pending = vec![from.clone()];
    while let Some(node) = pending.pop() {
        if node == *to {
            return true;
        }
        if seen.insert(node.clone()) {
            pending.extend(
                graph
                    .outgoing_edges(&node)
                    .into_iter()
                    .map(|edge| edge.target.clone()),
            );
        }
    }
    false
}

/// Every node's output and every connected input, computed from the graph.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Evaluation {
    outputs: HashMap<NodeId, Value>,
    inputs: HashMap<(NodeId, &'static str), Value>,
}

impl Evaluation {
    pub(super) fn output(&self, node: &NodeId) -> Option<Value> {
        self.outputs.get(node).copied()
    }

    pub(super) fn input(&self, node: &NodeId, port: &str) -> Option<Value> {
        self.inputs
            .iter()
            .find(|((id, input), _)| id == node && *input == port)
            .map(|(_, value)| *value)
    }
}

pub(super) fn evaluate_snapshot(snapshot: &GraphSnapshot<LabNode, ()>) -> Evaluation {
    evaluate(&snapshot.graph)
}

pub(super) fn evaluate(graph: &LabGraph) -> Evaluation {
    let mut evaluation = Evaluation::default();
    let mut visiting = HashSet::new();
    for node in &graph.nodes {
        output(graph, &node.id, &mut evaluation, &mut visiting);
    }
    evaluation
}

fn output(
    graph: &LabGraph,
    id: &NodeId,
    evaluation: &mut Evaluation,
    visiting: &mut HashSet<NodeId>,
) -> Option<Value> {
    if let Some(value) = evaluation.outputs.get(id) {
        return Some(*value);
    }
    let node = graph.node(id)?;
    // A loop the rule missed computes nothing rather than recursing forever.
    if !visiting.insert(id.clone()) {
        return None;
    }
    let op = node.data.op;
    let mut inputs = HashMap::new();
    for port in op.inputs() {
        let value = graph
            .incoming_edges(id)
            .into_iter()
            .find(|edge| edge.target_handle.as_ref().map(|handle| handle.as_str()) == Some(port.id))
            .and_then(|edge| output(graph, &edge.source, evaluation, visiting))
            .filter(|value| match port.ty {
                PortType::Color => value.color().is_some(),
                PortType::Number => value.number().is_some(),
            });
        if let Some(value) = value {
            evaluation.inputs.insert((id.clone(), port.id), value);
            inputs.insert(port.id, value);
        }
    }
    visiting.remove(id);
    let color = |port: &str| inputs.get(port).and_then(|value| value.color());
    let number = |port: &str| inputs.get(port).and_then(|value| value.number());
    let value = match op {
        Op::Color { hue, lightness } => {
            Some(Value::Color(Color::oklch(lightness, INPUT_CHROMA, hue)))
        }
        Op::Number { value } => Some(Value::Number(value)),
        Op::Mix => match (color("a"), color("b")) {
            (Some(a), Some(b)) => Some(Value::Color(mix(a, b, number("t").unwrap_or(0.5)))),
            _ => None,
        },
        Op::Lighten => color("color")
            .map(|color| Value::Color(lighten(color, number("amount").unwrap_or(0.2)))),
        Op::Contrast => match (color("text"), color("background")) {
            (Some(text), Some(background)) => Some(Value::Number(text.contrast_ratio(background))),
            _ => None,
        },
        Op::Preview | Op::Gradient | Op::Group => None,
    };
    if let Some(value) = value {
        evaluation.outputs.insert(id.clone(), value);
    }
    value
}

/// `a` blended toward `b` by `t`, in OKLab.
pub(super) fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (a, b) = (a.to_oklch(), b.to_oklch());
    let lab = |color: sui::Oklch| {
        let hue = color.hue.to_radians();
        (
            color.lightness,
            color.chroma * hue.cos(),
            color.chroma * hue.sin(),
        )
    };
    let (la, aa, ba) = lab(a);
    let (lb, ab, bb) = lab(b);
    let (l, x, y) = (la + (lb - la) * t, aa + (ab - aa) * t, ba + (bb - ba) * t);
    Color::oklch(
        l,
        (x * x + y * y).sqrt(),
        y.atan2(x).to_degrees().rem_euclid(360.0),
    )
}

/// `color` taken `amount` of the way to white in lightness.
fn lighten(color: Color, amount: f32) -> Color {
    let color = color.to_oklch();
    let lightness = color.lightness + (1.0 - color.lightness) * amount.clamp(0.0, 1.0);
    Color::oklch(lightness, color.chroma, color.hue)
}

/// How well text reads on a background, by the WCAG ratio.
pub(super) fn contrast_grade(ratio: f32) -> &'static str {
    if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3.0 {
        "AA large"
    } else {
        "Fails"
    }
}

/// The lab as it opens: a brand palette mixed, lightened, checked for
/// contrast, and previewed.
pub(super) fn lab_document() -> GraphDocument<LabNode, ()> {
    let color = |hue: f32, lightness: f32| Op::Color { hue, lightness };
    let palette = lab_node("palette", "Brand palette", Op::Group, Point::ZERO)
        .z_index(-10)
        .aria_label("Brand palette group");
    let inside = |node: Node<LabNode>| {
        node.parent("palette")
            .extent(NodeExtent::Parent)
            .expand_parent(true)
    };
    let nodes = vec![
        palette,
        inside(lab_node(
            "brand",
            "Brand",
            color(262.0, 0.52),
            Point::new(24.0, 44.0),
        )),
        inside(lab_node(
            "accent",
            "Accent",
            color(318.0, 0.66),
            Point::new(24.0, 166.0),
        )),
        lab_node(
            "blend",
            "Blend",
            Op::Number { value: 0.35 },
            Point::new(34.0, 330.0),
        ),
        lab_node("mix", "Mix", Op::Mix, Point::new(340.0, 96.0)),
        lab_node(
            "lift",
            "Lift",
            Op::Number { value: 0.8 },
            Point::new(340.0, 330.0),
        ),
        lab_node("lighten", "Lighten", Op::Lighten, Point::new(600.0, 360.0)),
        lab_node("preview", "Preview", Op::Preview, Point::new(900.0, 0.0)),
        lab_node(
            "contrast",
            "Contrast",
            Op::Contrast,
            Point::new(900.0, 190.0),
        ),
        lab_node(
            "gradient",
            "Gradient",
            Op::Gradient,
            Point::new(900.0, 340.0),
        ),
    ];
    let bezier = EdgeKind::Bezier;
    let edges = vec![
        lab_edge("brand", "mix", "a", bezier),
        lab_edge("accent", "mix", "b", bezier),
        lab_edge("blend", "mix", "t", bezier),
        lab_edge("mix", "preview", "color", bezier),
        lab_edge("mix", "lighten", "color", bezier),
        lab_edge("lift", "lighten", "amount", bezier),
        lab_edge("lighten", "contrast", "text", bezier),
        lab_edge("mix", "contrast", "background", bezier),
        lab_edge("mix", "gradient", "from", bezier),
        lab_edge("lighten", "gradient", "to", bezier),
    ];
    GraphDocument::new(nodes, edges).viewport(Viewport::new(20.0, 20.0, 0.9))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> LabGraph {
        let document = lab_document();
        GraphModel::new(document.nodes, document.edges).expect("the lab document is valid")
    }

    #[test]
    fn values_flow_through_the_lab() {
        let graph = graph();
        let evaluation = evaluate(&graph);
        let brand = Color::oklch(0.52, INPUT_CHROMA, 262.0);
        let accent = Color::oklch(0.66, INPUT_CHROMA, 318.0);
        assert_eq!(
            evaluation.output(&NodeId::from("mix")),
            Some(Value::Color(mix(brand, accent, 0.35)))
        );
        assert_eq!(
            evaluation.input(&NodeId::from("mix"), "t"),
            Some(Value::Number(0.35))
        );
        let Some(Value::Number(ratio)) = evaluation.output(&NodeId::from("contrast")) else {
            panic!("contrast computes a ratio");
        };
        assert!(ratio > 1.0);
        assert_eq!(evaluation.output(&NodeId::from("preview")), None);
    }

    #[test]
    fn the_rule_refuses_mismatched_types_loops_and_busy_inputs() {
        let graph = graph();
        let connect = |source: &str, target: &str, input: &str| {
            connection_rule(
                &Connection {
                    source: NodeId::from(source),
                    source_handle: Some("out".into()),
                    target: NodeId::from(target),
                    target_handle: Some(input.into()),
                },
                &graph,
            )
        };
        assert_eq!(
            connect("blend", "preview", "color"),
            Err("Blend gives a number, but color takes a color".to_string())
        );
        assert_eq!(
            connect("lighten", "mix", "a"),
            Err("That would loop: Mix already feeds Lighten".to_string())
        );
        assert_eq!(
            connect("accent", "mix", "a"),
            Err("a already has an input".to_string())
        );
        assert_eq!(
            connect("accent", "gradient", "from"),
            Err("from already has an input".to_string())
        );
        let mut free = graph.clone();
        free.remove_edge(&"mix-gradient-from".into());
        assert_eq!(
            connection_rule(
                &Connection {
                    source: NodeId::from("accent"),
                    source_handle: Some("out".into()),
                    target: NodeId::from("gradient"),
                    target_handle: Some("from".into()),
                },
                &free,
            ),
            Ok(())
        );
    }

    #[test]
    fn hex_names_colors() {
        assert_eq!(hex(Color::rgba(0.0, 0.5, 1.0, 1.0)), "#0080FF");
    }
}
