use crate::config::Config;
use sui_core::{
    Color, Event, FontHandle, Point, PointerEvent, PointerEventKind, Rect, ScrollDelta,
    SemanticsNode, SemanticsRole, Size, Vector, WindowEvent, WindowId,
};
use sui_layout::{Constraints, GridTrack};
use sui_reactive::Signal;
use sui_runtime::{
    Application, ArrangeCtx, MeasureCtx, PaintCtx, Runtime, SemanticsCtx, SingleChild, Widget,
    WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
};
use sui_text::{BUNDLED_NOTO_SANS_REGULAR_FONT, RegisteredFont, TextStyle};
use sui_widgets::{
    Button, Dialog, Flex, Grid, Label, Padding, RichDocumentModel, RichDocumentView, ScrollView,
    SizedBox, SplitView, Stack, SwitchView, TextInput, VirtualCollectionModel, VirtualList,
};

pub const MARKER: &str = "SUI benchmark content";

pub struct Fixture {
    pub revision: Signal<usize>,
    labels: Vec<Signal<String>>,
    color: Signal<Color>,
    selected: Signal<usize>,
    collection: VirtualCollectionModel<u64, String>,
    document: RichDocumentModel,
    next_key: u64,
    pub expected_label: Option<String>,
}

pub struct Input {
    labels: Vec<String>,
}

pub fn input(config: &Config) -> Input {
    Input {
        labels: (0..config.size)
            .map(|index| {
                format!(
                    "Item {index:06} seed {}: office layout and wrapping text",
                    config.seed
                )
            })
            .collect(),
    }
}

fn boxed(widget: impl Widget + 'static) -> SingleChild {
    SingleChild::from_pod(WidgetPod::new(widget))
}

struct Root {
    child: SingleChild,
    revision: Signal<usize>,
    color: Signal<Color>,
    rebuild: Option<Box<dyn Fn() -> SingleChild>>,
    built_revision: usize,
}

impl Widget for Root {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(build) = &self.rebuild {
            let revision = ctx.observe(&self.revision);
            if revision != self.built_revision {
                self.child = build();
                self.built_revision = revision;
            }
        }
        self.child.measure(ctx, constraints)
    }
    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }
    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_bounds(ctx.observe(&self.color));
        self.child.paint(ctx);
    }
    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(MARKER.into());
        node.value = Some(sui_core::SemanticsValue::Text(
            ctx.observe(&self.revision).to_string(),
        ));
        ctx.push(node);
        self.child.semantics(ctx);
    }
    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }
    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

// Type erasure at fixture composition seams uses the same SingleChild contract
// as built-in containers, rather than a second layout model.
struct Child(SingleChild);
impl Widget for Child {
    fn measure_axis(
        &mut self,
        c: &mut MeasureCtx,
        constraints: Constraints,
        axis: sui_layout::Axis,
    ) -> f32 {
        self.0.measure_axis(c, constraints, axis)
    }

    fn measure_size(&mut self, c: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.0.measure_size(c, constraints)
    }
    fn measure(&mut self, c: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.0.measure(c, constraints)
    }
    fn arrange(&mut self, c: &mut ArrangeCtx, bounds: Rect) {
        self.0.arrange(c, bounds);
    }
    fn paint(&self, c: &mut PaintCtx) {
        self.0.paint(c);
    }
    fn semantics(&self, c: &mut SemanticsCtx) {
        self.0.semantics(c);
    }
    fn visit_children(&self, v: &mut dyn WidgetPodVisitor) {
        self.0.visit_children(v);
    }
    fn visit_children_mut(&mut self, v: &mut dyn WidgetPodMutVisitor) {
        self.0.visit_children_mut(v);
    }
}

fn controls_grid(labels: &[Signal<String>], style: &TextStyle) -> Grid {
    let mut grid = Grid::new([
        GridTrack::Fraction(1.0),
        GridTrack::Fraction(1.0),
        GridTrack::Fraction(1.0),
    ])
    .gap(4.0);
    for (i, text) in labels.iter().enumerate() {
        grid.push(
            Stack::horizontal()
                .spacing(4.0)
                .with_child(Label::new("").text_from(text.clone()).style(style.clone()))
                .with_child(Button::new(format!("Action {i}"))),
        );
    }
    grid
}

pub fn build(config: &Config, input: Input) -> Result<(Application, Fixture), String> {
    let is_collection = matches!(
        config.fixture.as_str(),
        "keyed-collection" | "virtual-collection"
    );
    let is_document = config.fixture == "streaming-document";
    let labels: Vec<_> = if is_collection || is_document {
        Vec::new()
    } else {
        input
            .labels
            .iter()
            .map(|text| Signal::new(text.clone()))
            .collect()
    };
    let revision = Signal::new(0);
    let color = Signal::new(Color::rgba(0.1, 0.12, 0.15, 1.0));
    let selected = Signal::new(0);
    let collection = if is_collection {
        VirtualCollectionModel::from_items(
            "Benchmark rows",
            input
                .labels
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, v)| (i as u64, v)),
        )
        .map_err(|e| e.to_string())?
    } else {
        VirtualCollectionModel::new()
    };
    let document = RichDocumentModel::new();
    let style = TextStyle {
        font: Some(FontHandle::new(1)),
        font_size: 16.0,
        line_height: 20.0,
        ..TextStyle::default()
    };
    let grid = || controls_grid(&labels, &style);
    let child = match config.fixture.as_str() {
        "controls-grid" | "scene-properties" => boxed(grid()),
        "nested-flex" => {
            let mut leaf = Flex::vertical().gap(2.0);
            for value in &labels {
                leaf =
                    leaf.with_child(Label::new("").text_from(value.clone()).style(style.clone()));
            }
            let mut child = boxed(leaf);
            for depth in 0..config.depth {
                child = boxed(
                    Flex::horizontal()
                        .gap(2.0)
                        .with_child(Label::new(format!("Depth {depth}")))
                        .with_child(Padding::all(1.0, Child(child))),
                );
            }
            child
        }
        "scrollbar-thresholds" => {
            let mut stack = Stack::vertical().spacing(1.0);
            for text in &labels {
                stack.push(Label::new("").text_from(text.clone()).style(style.clone()));
            }
            boxed(ScrollView::both(
                SizedBox::new().width(config.width - 8.0).with_child(stack),
            ))
        }
        "keyed-collection" | "virtual-collection" => boxed(
            VirtualList::new("Benchmark rows", collection.clone(), {
                let style = style.clone();
                move |_: &u64, value: Signal<String>| {
                    Label::new("").text_from(value).style(style.clone())
                }
            })
            .estimated_row_height(24.0)
            .overscan_viewports(0.5),
        ),
        "streaming-document" => {
            document.set_markdown(input.labels.join("\n\n"));
            boxed(ScrollView::vertical(RichDocumentView::new(
                document.clone(),
            )))
        }
        "overlays-and-dialogs" => boxed(
            SwitchView::new()
                .selected_from(selected.clone())
                .with_child(Label::new("Closed dialog"))
                .with_child(Dialog::new("Benchmark dialog", grid())),
        ),
        "application-shell" => boxed(
            SplitView::horizontal(
                Stack::vertical()
                    .spacing(4.0)
                    .with_child(Label::new("Workspace"))
                    .with_child(TextInput::new("Search"))
                    .with_child(Button::new("Open")),
                ScrollView::vertical(grid()),
            )
            .ratio(0.2),
        ),
        _ => return Err(format!("unknown fixture {}", config.fixture)),
    };
    let rebuild = if config.mutation == "rebuild" {
        let labels = labels.clone();
        let style = style.clone();
        Some(Box::new(move || boxed(controls_grid(&labels, &style))) as Box<dyn Fn() -> SingleChild>)
    } else {
        None
    };
    let root = Root {
        rebuild,
        built_revision: 0,
        child,
        revision: revision.clone(),
        color: color.clone(),
    };
    let mut app = Application::new().window(
        WindowBuilder::new()
            .title(MARKER)
            .initial_size(Size::new(config.width, config.height))
            .root(root),
    );
    app.register_font(
        FontHandle::new(1),
        RegisteredFont::from_bytes(BUNDLED_NOTO_SANS_REGULAR_FONT),
    )
    .map_err(|e| e.to_string())?;
    Ok((
        app,
        Fixture {
            revision,
            labels,
            color,
            selected,
            collection,
            document,
            next_key: config.size as u64,
            expected_label: None,
        },
    ))
}

impl Fixture {
    pub fn mutate(
        &mut self,
        config: &Config,
        step: usize,
        runtime: &mut Runtime,
        window: WindowId,
    ) -> Result<usize, String> {
        let default = match config.fixture.as_str() {
            "scrollbar-thresholds" => "resize",
            "keyed-collection" => "reorder",
            "virtual-collection" => "scroll",
            "scene-properties" => "paint",
            _ => "local",
        };
        let mutation = if config.mutation == "default" {
            default
        } else {
            config.mutation.as_str()
        };
        if mutation == "idle" {
            return Ok(0);
        }
        self.expected_label = None;
        let mut changed = 1;
        match mutation {
            "resize" => {
                let width = if config.fixture == "scrollbar-thresholds" {
                    config.width + if step.is_multiple_of(2) { -16.0 } else { 16.0 }
                } else {
                    config.width + (step % 101) as f32 + 0.25
                };
                runtime
                    .handle_event(
                        window,
                        Event::Window(WindowEvent::Resized(Size::new(
                            width.max(1.0),
                            config.height,
                        ))),
                    )
                    .map_err(|e| e.to_string())?;
            }
            "scroll" => {
                let mut pointer = PointerEvent::new(
                    PointerEventKind::Scroll,
                    Point::new(config.width * 0.5, config.height * 0.5),
                );
                pointer.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(
                    0.0,
                    if (step / 8).is_multiple_of(2) {
                        -48.0
                    } else {
                        48.0
                    },
                )));
                runtime
                    .handle_event(window, Event::Pointer(pointer))
                    .map_err(|e| e.to_string())?;
            }
            "reorder" => {
                if step.is_multiple_of(3) {
                    self.collection
                        .move_to((step as u64 / 3) % config.size as u64, 0)
                        .map_err(|e| e.to_string())?;
                } else if step % 3 == 1 {
                    self.collection
                        .prepend([(self.next_key, format!("Inserted revision {step}"))])
                        .map_err(|e| e.to_string())?;
                    self.next_key += 1;
                } else {
                    self.collection
                        .remove(self.next_key - 1)
                        .map_err(|e| e.to_string())?;
                }
            }
            "paint" => {
                self.color.set(if step.is_multiple_of(2) {
                    Color::BLACK
                } else {
                    Color::rgba(0.15, 0.1, 0.2, 1.0)
                });
            }
            _ if config.fixture == "streaming-document" => {
                self.document.append_markdown(&format!(
                    " revision-{step}{}",
                    if step.is_multiple_of(8) { "\n\n" } else { "" }
                ));
            }
            _ if config.fixture == "overlays-and-dialogs" => {
                self.selected.set(1 - self.selected.get());
            }
            _ => {
                changed = if mutation == "all" {
                    config.size
                } else if mutation == "distributed" {
                    ((config.size as f64 * config.fraction).ceil() as usize).max(1)
                } else {
                    1
                };
                for j in 0..changed {
                    let index =
                        (step + config.seed as usize + j * config.size / changed) % config.size;
                    let value = format!(
                        "Item {index:06} revision {step:06}: {}",
                        if step.is_multiple_of(2) {
                            "short"
                        } else {
                            "longer text that wraps across available layout constraints"
                        }
                    );
                    if config.fixture == "virtual-collection"
                        || config.fixture == "keyed-collection"
                    {
                        self.collection
                            .update(index as u64, value.clone())
                            .map_err(|e| e.to_string())?;
                    } else {
                        self.labels[index].set(value.clone());
                        self.expected_label = Some(value);
                    }
                }
            }
        }
        self.revision.set(step + 1);
        Ok(changed)
    }
}
