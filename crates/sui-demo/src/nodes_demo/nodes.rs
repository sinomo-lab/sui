//! The widgets the Color lab's nodes are drawn with. The graph draws each
//! node's frame and its ports' names; these draw its title, its result, and
//! its controls, which edit the node's data in the graph's state.

use std::rc::Rc;

use sui::{Observable, Rect, Selector, Signal, prelude::*};
use sui_nodes::{GraphSnapshot, NodeId, NodeSignal};

use super::lab::{Evaluation, HEADER, LabNode, LabState, Op, ROW, Value, contrast_grade, hex, mix};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::{DemoTextColor, demo_label, demo_mono_label};

type EvaluationSource =
    Selector<Signal<GraphSnapshot<LabNode, ()>>, GraphSnapshot<LabNode, ()>, Evaluation>;

/// What every node widget reads: the theme, the graph's state to edit, and
/// the lab's computed values.
#[derive(Clone)]
pub(super) struct LabContext {
    pub(super) theme_reader: DevThemeReader,
    pub(super) state: LabState,
    pub(super) evaluation: EvaluationSource,
}

impl LabContext {
    /// `node`'s output, as it changes.
    fn output(
        &self,
        node: &NodeId,
    ) -> impl Observable<Option<Value>> + Clone + Send + Sync + use<> {
        let node = node.clone();
        Selector::new(
            format!("{node} output"),
            self.evaluation.clone(),
            move |evaluation: &Evaluation| evaluation.output(&node),
        )
    }

    /// The value arriving at `node`'s `port`, as it changes.
    fn input(
        &self,
        node: &NodeId,
        port: &'static str,
    ) -> impl Observable<Option<Value>> + Clone + Send + Sync + use<> {
        let node = node.clone();
        Selector::new(
            format!("{node} {port}"),
            self.evaluation.clone(),
            move |evaluation: &Evaluation| evaluation.input(&node, port),
        )
    }
}

fn as_color(value: &Option<Value>) -> Option<Color> {
    match value {
        Some(Value::Color(color)) => Some(*color),
        _ => None,
    }
}

/// The node's title, in the row its output handle sits beside.
fn header(lab: &LabContext, node: &NodeSignal<LabNode>, muted: bool) -> impl Widget + use<> {
    let title = node.select_named("Lab node title", |node| node.label.clone());
    SizedBox::new().height(HEADER).with_child(Align::new(
        Alignment::Start,
        Alignment::Center,
        demo_label(
            &lab.theme_reader,
            "",
            DemoTextRole::CardTitle,
            if muted {
                DemoTextColor::Muted
            } else {
                DemoTextColor::Text
            },
        )
        .text_from(title),
    ))
}

/// A column of the header and `body`, inset from the node's frame.
fn framed<W>(lab: &LabContext, node: &NodeSignal<LabNode>, body: W) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    Padding::symmetric(
        12.0,
        0.0,
        Stack::vertical()
            .alignment(Alignment::Stretch)
            .with_child(header(lab, node, false))
            .with_child(body),
    )
}

/// A swatch and its hex code, from `value`.
fn result<O>(lab: &LabContext, value: O) -> impl Widget + use<O>
where
    O: Observable<Option<Value>> + Clone + Send + Sync + 'static,
{
    let text = Selector::new("Result text", value.clone(), |value: &Option<Value>| {
        value.map_or_else(|| "—".to_string(), Value::text)
    });
    Flex::horizontal()
        .gap(8.0)
        .align_items(Alignment::Center)
        .with_child(Swatch::new(
            &lab.theme_reader,
            Selector::new("Result color", value, as_color),
            Some(Size::new(22.0, 22.0)),
        ))
        .with_child(
            demo_mono_label(&lab.theme_reader, "", DemoTextRole::Metadata, |theme| {
                theme.palette.text
            })
            .text_from(text),
        )
}

/// A body beside the input rows, leaving the ports' names room on the left.
fn beside_inputs<W>(rows: usize, body: W) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    SizedBox::new()
        .height(rows as f32 * ROW)
        .with_child(Padding::new(
            Insets {
                left: 76.0,
                ..Insets::all(0.0)
            },
            Align::new(Alignment::End, Alignment::Center, body),
        ))
}

pub(super) fn color_node(
    id: &NodeId,
    node: NodeSignal<LabNode>,
    lab: &LabContext,
) -> impl Widget + use<> {
    let read = node.clone();
    let write = lab.state.clone();
    let edit = id.clone();
    let hue = Slider::new(format!("{} hue", node.get().label))
        .theme_when(clone_dev_theme_reader(&lab.theme_reader))
        .range(0.0, 360.0)
        .step(1.0)
        .value_when(move || match read.get().data.op {
            Op::Color { hue, .. } => f64::from(hue),
            _ => 0.0,
        })
        .on_change(move |value| {
            // A drag along the slider is one undo step.
            write.merge_undo(format!("{edit} hue"), || {
                write.update_node_data(&edit, |data| {
                    if let Op::Color { hue, .. } = &mut data.op {
                        *hue = value as f32;
                    }
                })
            });
        });
    framed(
        lab,
        &node,
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(result(lab, lab.output(id)))
            .with_child(hue),
    )
}

pub(super) fn number_node(
    id: &NodeId,
    node: NodeSignal<LabNode>,
    lab: &LabContext,
) -> impl Widget + use<> {
    let read = node.clone();
    let write = lab.state.clone();
    let edit = id.clone();
    let value = Slider::new(format!("{} value", node.get().label))
        .theme_when(clone_dev_theme_reader(&lab.theme_reader))
        .range(0.0, 1.0)
        .step(0.01)
        .value_when(move || match read.get().data.op {
            Op::Number { value } => f64::from(value),
            _ => 0.0,
        })
        .on_change(move |value| {
            write.merge_undo(format!("{edit} value"), || {
                write.update_node_data(&edit, |data| {
                    if let Op::Number { value: number } = &mut data.op {
                        *number = value as f32;
                    }
                })
            });
        });
    let text = Selector::new("Number text", lab.output(id), |value: &Option<Value>| {
        value.map_or_else(|| "—".to_string(), Value::text)
    });
    framed(
        lab,
        &node,
        Stack::vertical()
            .spacing(4.0)
            .alignment(Alignment::Stretch)
            .with_child(
                demo_mono_label(&lab.theme_reader, "", DemoTextRole::Body, |theme| {
                    theme.palette.text
                })
                .text_from(text),
            )
            .with_child(value),
    )
}

/// Mix and Lighten: their result beside their inputs.
pub(super) fn blend_node(
    id: &NodeId,
    node: NodeSignal<LabNode>,
    lab: &LabContext,
) -> impl Widget + use<> {
    let rows = node.get().data.op.inputs().len();
    framed(lab, &node, beside_inputs(rows, result(lab, lab.output(id))))
}

pub(super) fn contrast_node(
    id: &NodeId,
    node: NodeSignal<LabNode>,
    lab: &LabContext,
) -> impl Widget + use<> {
    let ratio = Selector::new(
        "Contrast ratio",
        lab.output(id),
        |value: &Option<Value>| match value {
            Some(Value::Number(ratio)) => format!("{ratio:.1} : 1"),
            _ => "—".to_string(),
        },
    );
    let grade = Selector::new(
        "Contrast grade",
        lab.output(id),
        |value: &Option<Value>| match value {
            Some(Value::Number(ratio)) => contrast_grade(*ratio).to_string(),
            _ => String::new(),
        },
    );
    framed(
        lab,
        &node,
        beside_inputs(
            2,
            Stack::vertical()
                .spacing(2.0)
                .alignment(Alignment::End)
                .with_child(
                    demo_mono_label(&lab.theme_reader, "", DemoTextRole::Body, |theme| {
                        theme.palette.text
                    })
                    .text_from(ratio),
                )
                .with_child(
                    demo_label(
                        &lab.theme_reader,
                        "",
                        DemoTextRole::Metadata,
                        DemoTextColor::Muted,
                    )
                    .text_from(grade),
                ),
        ),
    )
}

pub(super) fn preview_node(
    id: &NodeId,
    node: NodeSignal<LabNode>,
    lab: &LabContext,
) -> impl Widget + use<> {
    let color = lab.input(id, "color");
    let text = Selector::new("Preview text", color.clone(), |value: &Option<Value>| {
        value.map_or_else(|| "Connect a color".to_string(), Value::text)
    });
    framed(
        lab,
        &node,
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(SizedBox::new().height(ROW))
            .with_child(Swatch::new(
                &lab.theme_reader,
                Selector::new("Preview color", color, as_color),
                Some(Size::new(156.0, 50.0)),
            ))
            .with_child(
                demo_mono_label(&lab.theme_reader, "", DemoTextRole::Metadata, |theme| {
                    theme.palette.text_muted
                })
                .text_from(text),
            ),
    )
}

pub(super) fn gradient_node(
    id: &NodeId,
    node: NodeSignal<LabNode>,
    lab: &LabContext,
) -> impl Widget + use<> {
    let (from, to) = (id.clone(), id.clone());
    let ends = Selector::new(
        format!("{id} gradient"),
        lab.evaluation.clone(),
        move |evaluation: &Evaluation| {
            let from = as_color(&evaluation.input(&from, "from"))?;
            let to = as_color(&evaluation.input(&to, "to"))?;
            Some((from, to))
        },
    );
    framed(
        lab,
        &node,
        Stack::vertical()
            .alignment(Alignment::Stretch)
            .with_child(SizedBox::new().height(2.0 * ROW + 4.0))
            .with_child(GradientBar::new(&lab.theme_reader, ends)),
    )
}

pub(super) fn group_node(node: NodeSignal<LabNode>, lab: &LabContext) -> impl Widget + use<> {
    Padding::symmetric(12.0, 0.0, header(lab, &node, true))
}

/// A rounded patch of a color, or an empty outline while there is none.
struct Swatch {
    theme_reader: DevThemeReader,
    color: Box<dyn Observable<Option<Color>>>,
    size: Option<Size>,
}

impl Swatch {
    fn new<O>(theme_reader: &DevThemeReader, color: O, size: Option<Size>) -> Self
    where
        O: Observable<Option<Color>> + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            color: Box::new(color),
            size,
        }
    }
}

impl Widget for Swatch {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(self.size.unwrap_or(constraints.max))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        let shape = Path::rounded_rect(bounds, 6.0);
        match ctx.observe(self.color.as_ref()) {
            Some(color) => {
                ctx.fill(shape.clone(), color);
                ctx.stroke(shape, theme.palette.border, StrokeStyle::new(1.0));
            }
            None => ctx.stroke(shape, theme.palette.border_strong, StrokeStyle::new(1.0)),
        }
    }
}

/// The colors between two ends, blended in OKLab.
struct GradientBar {
    theme_reader: DevThemeReader,
    ends: Box<dyn Observable<Option<(Color, Color)>>>,
}

impl GradientBar {
    fn new<O>(theme_reader: &DevThemeReader, ends: O) -> Self
    where
        O: Observable<Option<(Color, Color)>> + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            ends: Box::new(ends),
        }
    }
}

impl Widget for GradientBar {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(constraints.max.width, 28.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        const STEPS: usize = 24;
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        let shape = Path::rounded_rect(bounds, 6.0);
        let Some((from, to)) = ctx.observe(self.ends.as_ref()) else {
            ctx.stroke(shape, theme.palette.border_strong, StrokeStyle::new(1.0));
            return;
        };
        ctx.push_clip(shape.clone());
        let width = bounds.width() / STEPS as f32;
        for step in 0..STEPS {
            let t = step as f32 / (STEPS - 1) as f32;
            ctx.fill_rect(
                Rect::new(
                    bounds.x() + step as f32 * width,
                    bounds.y(),
                    width + 0.5,
                    bounds.height(),
                ),
                mix(from, to, t),
            );
        }
        ctx.pop_clip();
        ctx.stroke(shape, theme.palette.border, StrokeStyle::new(1.0));
    }
}

/// A value as the inspector shows it.
pub(super) fn describe_value(value: Option<Value>) -> String {
    match value {
        Some(Value::Color(color)) => hex(color),
        Some(Value::Number(number)) => format!("{number:.2}"),
        None => "nothing yet".to_string(),
    }
}
