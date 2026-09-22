use std::{ops::Range, time::Instant};

use sui::{
    EventPhase, FontWeight, KeyState, PointerButton, PointerEventKind, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Vector, WidgetId,
    WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor, prelude::*,
};
use sui_text::{FontFamilyStack, PersistentTextLayout, TextDocument, TextLayoutRequest};

use crate::app::{DevThemeReader, clone_dev_theme_reader};

pub(crate) const EDITORIAL_TAB_LABEL: &str = "Editorial engine";
const NAME: &str = "Editorial text flow";
const LINE_HEIGHT: f32 = 26.0;
const MIN_SLOT: f32 = 54.0;
const TITLE: &str = "Room for\nthe unexpected";
const QUOTE: &str = "A good place leaves room for something unplanned.";
const PAPER: Color = Color::rgba(0.89, 0.87, 0.82, 1.0);
const GOLD: Color = Color::rgba(0.78, 0.64, 0.38, 1.0);

// Original article copy; paragraphs bound the work performed by the rectangular
// text-layout adapter below. This is a demo compositor, not an exclusion API.
const ARTICLE: &[&str] = &[
    "The city wakes before its streets become busy. A shopkeeper lifts a shutter, a bicycle crosses the square, and the first light finds a narrow garden between two buildings. Nothing here is perfectly still. The useful spaces are the ones that can make room for change.",
    "A garden begins with boundaries, but its character comes from what happens inside them. Paths bend around a tree. A bench catches the afternoon sun. People gather where a wall offers shelter, then drift toward an opening when the weather changes. The plan becomes a conversation with its surroundings.",
    "On a printed page, columns provide a similar kind of order. They give the eye a comfortable distance to travel and a reliable place to begin again. An illustration can interrupt that rhythm without breaking it. The words find another route, and the reader follows the movement almost without noticing.",
    "Here the obstacles are allowed to move. Each circle borrows a little space from the lines beside it. A wide passage may divide into two narrow ones; a blocked row may become open again. The story continues through those openings and carries on into the next column.",
    "There is a difference between filling a space and inhabiting it. A crowded page can contain every necessary detail and still be difficult to read. A quieter arrangement offers pauses: a generous margin, a short quotation, a change of scale at the beginning of a story.",
    "At the corner cafe\u{301}, a visitor writes a note in the margin: 文字也需要呼吸的空间. Words need room to breathe. The letters differ, but the idea is familiar. Good typography respects the shape of language as well as the shape of the page that holds it.",
    "Try moving a circle across the gutter. The left column and the right column respond independently, but they still share one continuous passage. No sentence is meant to disappear into the obstacle. The available space changes; the order of the story does not.",
    "Resize the window and a different composition emerges. Three columns become two, and two become one. The headline grows quieter on a small screen. The important question is not whether every version looks identical, but whether each version remains comfortable to explore.",
    "A useful experiment makes its rules visible. You can pause the motion, place two obstacles together, or move one against an edge. These small interventions reveal where the layout is generous and where it becomes fragile. They turn a polished picture into something that can be tested.",
    "The most interesting designs often begin with an ordinary constraint. A room has a door in an inconvenient place. A path meets an old tree. A paragraph reaches a shape that was not there a moment ago. Instead of erasing the obstacle, the design discovers a new way around it.",
    "As evening arrives, the square changes once more. The garden becomes a shortcut, the bench becomes a meeting place, and the open doorway gathers a small crowd. The original plan still matters, but it has learned to share the space with events it could not predict.",
    "The page is another small place to practice that generosity. Give the text a clear direction, allow the objects to move, and watch how a familiar story finds its way through unfamiliar space. There is usually more than one useful arrangement waiting to be discovered.",
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Orb {
    position: Point,
    velocity: Vector,
    radius: f32,
    color: Color,
    paused: bool,
    dragging: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Motion {
    playing: bool,
    orbs: [Orb; 3],
}

impl Default for Motion {
    fn default() -> Self {
        let orb = |x, y, vx, vy, radius, color| Orb {
            position: Point::new(x, y),
            velocity: Vector::new(vx, vy),
            radius,
            color,
            paused: false,
            dragging: false,
        };
        Self {
            playing: true,
            orbs: [
                orb(0.25, 0.3, 22.0, 17.0, 66.0, GOLD),
                orb(
                    0.62,
                    0.6,
                    -19.0,
                    14.0,
                    78.0,
                    Color::rgba(0.40, 0.60, 0.95, 1.0),
                ),
                orb(
                    0.86,
                    0.25,
                    -14.0,
                    -19.0,
                    56.0,
                    Color::rgba(0.84, 0.40, 0.53, 1.0),
                ),
            ],
        }
    }
}

impl Motion {
    fn active(&self) -> bool {
        self.playing && self.orbs.iter().any(|orb| !orb.paused && !orb.dragging)
    }
    fn advance(&mut self, delta: f64, body: Rect) {
        if !self.playing || !delta.is_finite() || body.width() <= 1.0 || body.height() <= 1.0 {
            return;
        }
        let dt = delta.clamp(0.0, 0.05) as f32;
        for orb in &mut self.orbs {
            if orb.paused || orb.dragging {
                continue;
            }
            orb.position.x += orb.velocity.x * dt / body.width();
            orb.position.y += orb.velocity.y * dt / body.height();
            let radius = orb_circle(*orb, body).radius;
            for (position, velocity, extent) in [
                (&mut orb.position.x, &mut orb.velocity.x, body.width()),
                (&mut orb.position.y, &mut orb.velocity.y, body.height()),
            ] {
                let inset = radius / extent;
                if *position < inset {
                    *position = inset;
                    *velocity = velocity.abs();
                }
                if *position > 1.0 - inset {
                    *position = 1.0 - inset;
                    *velocity = -velocity.abs();
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Circle {
    center: Point,
    radius: f32,
}

fn orb_circle(orb: Orb, body: Rect) -> Circle {
    let radius = orb
        .radius
        .min(body.width().max(0.0) * 0.22)
        .min(body.height().max(0.0) * 0.22);
    Circle {
        center: Point::new(
            (body.x() + orb.position.x * body.width())
                .clamp(body.x() + radius, body.max_x() - radius),
            (body.y() + orb.position.y * body.height())
                .clamp(body.y() + radius, body.max_y() - radius),
        ),
        radius,
    }
}

fn line_slots(column: Rect, y: f32, circles: &[Circle], rectangles: &[Rect]) -> Vec<Range<f32>> {
    let mut slots = vec![column.x()..column.max_x()];
    let mut blocked = Vec::new();
    for circle in circles {
        let distance = if circle.center.y < y - 4.0 {
            y - 4.0 - circle.center.y
        } else if circle.center.y > y + LINE_HEIGHT + 4.0 {
            circle.center.y - y - LINE_HEIGHT - 4.0
        } else {
            0.0
        };
        if distance < circle.radius {
            let half = (circle.radius * circle.radius - distance * distance)
                .max(0.0)
                .sqrt()
                + 12.0;
            blocked.push(circle.center.x - half..circle.center.x + half);
        }
    }
    for rect in rectangles {
        if rect.max_y() > y && rect.y() < y + LINE_HEIGHT {
            blocked.push(rect.x()..rect.max_x());
        }
    }
    for block in blocked {
        let mut next = Vec::new();
        for slot in slots {
            if block.end <= slot.start || block.start >= slot.end {
                next.push(slot);
            } else {
                if block.start > slot.start {
                    next.push(slot.start..block.start);
                }
                if block.end < slot.end {
                    next.push(block.end..slot.end);
                }
            }
        }
        slots = next;
    }
    slots.retain(|slot| slot.end - slot.start >= MIN_SLOT);
    slots
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Cursor {
    paragraph: usize,
    byte: usize,
}

struct FlowLine {
    bounds: Rect,
    layout: PersistentTextLayout,
    paragraph: usize,
    consumed: Range<usize>,
}

fn flow_article(
    columns: &[Rect],
    circles: &[Circle],
    rectangles: &[Rect],
    previous: &[FlowLine],
    mut layout: impl FnMut(
        &str,
        f32,
        Option<sui::TextLayoutHandle>,
    ) -> sui::Result<PersistentTextLayout>,
) -> sui::Result<(Vec<FlowLine>, Cursor)> {
    // SUI currently lays out rectangular paragraphs. Shape the unconsumed
    // suffix at this slot's width, display only its first line, and advance
    // by that line's UTF-8 byte range. This deliberately simple adapter may
    // lay out more lines than it displays; it is not a next-line engine API.
    // The first ASCII letter is painted separately as the drop cap.
    let mut cursor = Cursor {
        paragraph: 0,
        byte: 1,
    };
    let mut lines = Vec::new();
    for column in columns {
        let mut y = column.y();
        while y + LINE_HEIGHT <= column.max_y() && cursor.paragraph < ARTICLE.len() {
            let mut paragraph_ended = false;
            for slot in line_slots(*column, y, circles, rectangles) {
                let text = &ARTICLE[cursor.paragraph][cursor.byte..];
                let width = (slot.end - slot.start).floor().max(1.0);
                let shaped = layout(
                    text,
                    width,
                    previous.get(lines.len()).map(|line| line.layout.handle()),
                )?;
                let consumed = shaped.lines().first().map_or(0, |line| line.byte_range.end);
                if consumed == 0 || consumed > text.len() || !text.is_char_boundary(consumed) {
                    return Err(sui::Error::new(
                        "editorial line layout did not advance at a UTF-8 boundary",
                    ));
                }
                let start = cursor.byte;
                cursor.byte += consumed;
                lines.push(FlowLine {
                    bounds: Rect::new(slot.start, y, width, LINE_HEIGHT),
                    layout: shaped,
                    paragraph: cursor.paragraph,
                    consumed: start..cursor.byte,
                });
                if cursor.byte == ARTICLE[cursor.paragraph].len() {
                    cursor.paragraph += 1;
                    cursor.byte = 0;
                    paragraph_ended = true;
                    break;
                }
            }
            y += LINE_HEIGHT + if paragraph_ended { 9.0 } else { 0.0 };
        }
    }
    Ok((lines, cursor))
}

fn serif(size: f32, line_height: f32, color: Color) -> TextStyle {
    TextStyle {
        font_size: size,
        line_height,
        color,
        font_families: Some(FontFamilyStack::new(
            "Georgia",
            &["Palatino Linotype", "ui-serif"],
        )),
        ..TextStyle::default()
    }
}

pub(crate) fn build_editorial_demo_with_theme(theme: DevThemeReader) -> impl Widget {
    Editorial::new(Signal::named("Editorial motion", Motion::default()), theme)
}

struct Drag {
    index: usize,
    pointer: u64,
    down: Point,
    original: Point,
    moved: bool,
}

struct Editorial {
    state: Signal<Motion>,
    controls: WidgetPod,
    controls_size: Size,
    size: Size,
    body: Rect,
    columns: Vec<Rect>,
    circles: Vec<Circle>,
    blocked_rects: Vec<Rect>,
    title: Option<PersistentTextLayout>,
    drop_cap: Option<PersistentTextLayout>,
    quote: Option<PersistentTextLayout>,
    quote_bounds: Option<Rect>,
    static_size: Option<Size>,
    lines: Vec<FlowLine>,
    cursor: Cursor,
    reflow_ms: f64,
    drag: Option<Drag>,
}

impl Editorial {
    fn new(state: Signal<Motion>, theme: DevThemeReader) -> Self {
        let play = state.clone();
        let pause = state.clone();
        let reset = state.clone();
        let controls = Flex::horizontal()
            .gap(8.0)
            .wrap(FlexWrap::Wrap)
            .with_item(
                Button::new("Play all")
                    .theme_when(clone_dev_theme_reader(&theme))
                    .on_press(move || {
                        play.update(|state| {
                            state.playing = true;
                            for orb in &mut state.orbs {
                                orb.paused = false;
                            }
                        });
                    }),
                FlexItem::fixed(90.0),
            )
            .with_item(
                Button::new("Pause all")
                    .theme_when(clone_dev_theme_reader(&theme))
                    .on_press(move || {
                        pause.update(|state| state.playing = false);
                    }),
                FlexItem::fixed(90.0),
            )
            .with_item(
                Button::new("Reset")
                    .theme_when(clone_dev_theme_reader(&theme))
                    .on_press(move || {
                        reset.set(Motion::default());
                    }),
                FlexItem::fixed(80.0),
            );
        Self {
            state,
            controls: WidgetPod::new(controls),
            controls_size: Size::ZERO,
            size: Size::ZERO,
            body: Rect::ZERO,
            columns: Vec::new(),
            circles: Vec::new(),
            blocked_rects: Vec::new(),
            title: None,
            drop_cap: None,
            quote: None,
            quote_bounds: None,
            static_size: None,
            lines: Vec::new(),
            cursor: Cursor::default(),
            reflow_ms: 0.0,
            drag: None,
        }
    }

    fn margin(&self) -> f32 {
        if self.size.width < 600.0 { 18.0 } else { 32.0 }
    }
    fn orb_id(owner: WidgetId, index: usize) -> WidgetId {
        WidgetId::new((7_u64 << 56) | (owner.get() << 3) | (index as u64 + 1))
    }
    fn finish_drag(&mut self, ctx: &mut EventCtx, toggle: bool) {
        if let Some(drag) = self.drag.take() {
            self.state.update(|state| {
                state.orbs[drag.index].dragging = false;
                if toggle && !drag.moved {
                    state.orbs[drag.index].paused = !state.orbs[drag.index].paused;
                }
            });
            ctx.release_pointer_capture(drag.pointer);
        }
    }
}

impl Widget for Editorial {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() == EventPhase::Capture {
            return;
        }
        match event {
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                if self.state.get().active() {
                    self.state.update(|state| state.advance(*delta, self.body));
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let point = Point::new(
                    pointer.position.x - ctx.bounds().x(),
                    pointer.position.y - ctx.bounds().y(),
                );
                if let Some(index) = self.circles.iter().rposition(|circle| {
                    let d = point - circle.center;
                    d.x * d.x + d.y * d.y <= circle.radius * circle.radius
                }) {
                    self.drag = Some(Drag {
                        index,
                        pointer: pointer.pointer_id,
                        down: point,
                        original: self.state.get().orbs[index].position,
                        moved: false,
                    });
                    self.state.update(|state| state.orbs[index].dragging = true);
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_focus();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if let Some(drag) = &mut self.drag
                    && drag.pointer == pointer.pointer_id
                {
                    let point = Point::new(
                        pointer.position.x - ctx.bounds().x(),
                        pointer.position.y - ctx.bounds().y(),
                    );
                    let delta = point - drag.down;
                    drag.moved |= delta.x * delta.x + delta.y * delta.y > 16.0;
                    self.state.update(|state| {
                        let orb = &mut state.orbs[drag.index];
                        let radius = orb_circle(*orb, self.body).radius;
                        let x = radius / self.body.width().max(1.0);
                        let y = radius / self.body.height().max(1.0);
                        orb.position = Point::new(
                            (drag.original.x + delta.x / self.body.width().max(1.0))
                                .clamp(x, 1.0 - x),
                            (drag.original.y + delta.y / self.body.height().max(1.0))
                                .clamp(y, 1.0 - y),
                        );
                    });
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Up | PointerEventKind::Cancel
                ) =>
            {
                if self
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.pointer == pointer.pointer_id)
                {
                    self.finish_drag(ctx, pointer.kind == PointerEventKind::Up);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if key.state == KeyState::Pressed => match key.key.as_str() {
                " " | "Space" => {
                    self.state.update(|state| state.playing = !state.playing);
                    ctx.set_handled();
                }
                "r" | "R" => {
                    self.finish_drag(ctx, false);
                    self.state.set(Motion::default());
                    ctx.set_handled();
                }
                "Escape" => {
                    self.finish_drag(ctx, false);
                }
                _ => {}
            },
            Event::Semantics(event) if event.action == SemanticsActionRequest::Activate => {
                if let Some(index) =
                    (0..3).find(|index| Self::orb_id(ctx.widget_id(), *index) == event.target)
                {
                    self.state
                        .update(|state| state.orbs[index].paused = !state.orbs[index].paused);
                    ctx.set_handled();
                }
            }
            Event::Window(sui::WindowEvent::Focused(false) | sui::WindowEvent::Resized(_))
                if self.drag.is_some() =>
            {
                self.finish_drag(ctx, false)
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                1100.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                780.0
            },
        ));
        let margin = self.margin();
        let width = (self.size.width - margin * 2.0).max(1.0);
        self.controls_size = self.controls.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(width, f32::INFINITY)),
        );
        let started = Instant::now();
        if self.static_size != Some(self.size) {
            let font = (width * 0.068).clamp(24.0, 64.0);
            let style = TextStyle {
                weight: FontWeight::BOLD,
                ..serif(font, font * 1.05, PAPER)
            };
            self.title = ctx
                .layout()
                .shape_text_persistent(
                    self.title.as_ref().map(|v| v.handle()),
                    TITLE,
                    Size::new(width, 1.0),
                    style,
                )
                .ok();
            self.drop_cap = ctx
                .layout()
                .shape_text_persistent(
                    self.drop_cap.as_ref().map(|v| v.handle()),
                    "T",
                    Size::new(100.0, 78.0),
                    serif(68.0, 78.0, GOLD),
                )
                .ok();
            self.static_size = Some(self.size);
        }
        let title_height = self
            .title
            .as_ref()
            .map_or(80.0, |layout| layout.measurement().height);
        let body_top = margin + self.controls_size.height + 16.0 + title_height + 20.0;
        self.body = Rect::new(
            margin,
            body_top,
            width,
            (self.size.height - body_top - 48.0).max(0.0),
        );
        let count = if self.size.width >= 1080.0 {
            3
        } else if self.size.width >= 700.0 {
            2
        } else {
            1
        };
        let gap = 28.0;
        let column_width = ((width - gap * (count - 1) as f32) / count as f32).max(1.0);
        self.columns = (0..count)
            .map(|index| {
                Rect::new(
                    margin + index as f32 * (column_width + gap),
                    body_top,
                    column_width,
                    self.body.height(),
                )
            })
            .collect();
        self.blocked_rects.clear();
        let cap_width = self
            .drop_cap
            .as_ref()
            .map_or(46.0, |layout| layout.measurement().bounds.width())
            + 12.0;
        self.blocked_rects
            .push(Rect::new(margin, body_top, cap_width, LINE_HEIGHT * 3.0));
        self.quote_bounds = None;
        if count > 1 && self.body.height() > 260.0 {
            let quote_width = column_width * 0.68;
            self.quote = ctx
                .layout()
                .shape_text_persistent(
                    self.quote.as_ref().map(|v| v.handle()),
                    QUOTE,
                    Size::new((quote_width - 20.0).max(1.0), 1.0),
                    serif(18.0, 25.0, GOLD),
                )
                .ok();
            let height = self
                .quote
                .as_ref()
                .map_or(90.0, |layout| layout.measurement().height + 20.0);
            let rect = Rect::new(
                self.columns[1].max_x() - quote_width,
                body_top + self.body.height() * 0.42,
                quote_width,
                height,
            );
            if rect.max_y() < self.body.max_y() {
                self.quote_bounds = Some(rect);
                self.blocked_rects.push(rect.inflate(8.0, 6.0));
            }
        }
        let motion = ctx.observe(&self.state);
        self.circles = motion
            .orbs
            .iter()
            .map(|orb| orb_circle(*orb, self.body))
            .collect();
        let result = flow_article(
            &self.columns,
            &self.circles,
            &self.blocked_rects,
            &self.lines,
            |text, width, handle| {
                ctx.layout().layout_document_persistent(
                    handle,
                    TextLayoutRequest::new(TextDocument::from_plain_text(
                        text,
                        serif(17.0, LINE_HEIGHT, PAPER),
                    ))
                    .with_box_size(Size::new(width, LINE_HEIGHT)),
                )
            },
        );
        if let Ok((lines, cursor)) = result {
            self.lines = lines;
            self.cursor = cursor;
        } else {
            self.lines.clear();
            self.cursor = Cursor::default();
        }
        self.reflow_ms = started.elapsed().as_secs_f64() * 1000.0;
        if motion.active() && self.body.height() > 0.0 {
            ctx.request_animation_frame();
        }
        self.size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.controls.arrange(
            ctx,
            Rect::new(
                bounds.x() + self.margin(),
                bounds.y() + self.margin(),
                (bounds.width() - self.margin() * 2.0).max(0.0),
                self.controls_size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let offset = ctx.bounds().origin.to_vector();
        ctx.fill_bounds(Color::rgba(0.045, 0.05, 0.065, 1.0));
        self.controls.paint(ctx);
        ctx.push_clip_rect(ctx.bounds());
        if let Some(title) = &self.title {
            ctx.draw_persistent_text_layout(
                Point::new(
                    offset.x + self.margin(),
                    offset.y + self.margin() + self.controls_size.height + 16.0,
                ),
                title,
            );
        }
        let motion = self.state.get();
        for (index, circle) in self.circles.iter().enumerate() {
            let color = motion.orbs[index].color;
            let alpha = if motion.orbs[index].paused || !motion.playing {
                0.5
            } else {
                1.0
            };
            for (extra, opacity) in [(10.0, 0.035), (5.0, 0.065), (0.0, 0.22)] {
                let r = circle.radius + extra;
                ctx.fill_rrect(
                    Rect::new(
                        offset.x + circle.center.x - r,
                        offset.y + circle.center.y - r,
                        r * 2.0,
                        r * 2.0,
                    ),
                    [r; 4],
                    color.with_alpha(opacity * alpha),
                );
            }
        }
        for line in &self.lines {
            let rect = line.bounds.translate(offset);
            ctx.push_clip_rect(rect);
            ctx.draw_persistent_text_layout_window(rect.origin, &line.layout, 0..1);
            ctx.pop_clip();
        }
        if let Some(cap) = &self.drop_cap {
            let ink = cap.measurement().bounds;
            ctx.draw_persistent_text_layout(
                Point::new(
                    offset.x + self.body.x() - ink.x(),
                    offset.y + self.body.y() + 3.0 - ink.y(),
                ),
                cap,
            );
        }
        if let (Some(rect), Some(quote)) = (self.quote_bounds, &self.quote) {
            let rect = rect.translate(offset);
            ctx.fill_rect(rect, Color::rgba(0.065, 0.065, 0.075, 1.0));
            ctx.fill_rect(
                Rect::new(rect.x(), rect.y(), 2.0, rect.height()),
                GOLD.with_alpha(0.6),
            );
            ctx.draw_persistent_text_layout(Point::new(rect.x() + 12.0, rect.y() + 10.0), quote);
        }
        ctx.fill_rect(
            Rect::new(
                offset.x,
                offset.y + self.size.height - 36.0,
                self.size.width,
                36.0,
            ),
            Color::rgba(0.035, 0.04, 0.05, 1.0),
        );
        let mut status = format!(
            "{} columns · {} lines · Reflow {:.2} ms",
            self.columns.len(),
            self.lines.len(),
            self.reflow_ms
        );
        if self.size.width >= 700.0 {
            status.push_str(" · Drag circles · Click to pause · Space: play/pause · R: reset");
        } else {
            status.push_str(" · Drag / tap circles");
        }
        ctx.draw_text(
            Rect::new(
                offset.x + self.margin(),
                offset.y + self.size.height - 27.0,
                (self.size.width - self.margin() * 2.0).max(1.0),
                24.0,
            ),
            status,
            TextStyle {
                font_size: 12.0,
                line_height: 18.0,
                color: GOLD,
                ..TextStyle::default()
            },
        );
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(NAME.into());
        node.description=Some("Drag a circle to reshape the text. Click a circle to pause it. Space pauses or resumes motion; R resets.".into());
        let mut text = String::from("T");
        for line in &self.lines {
            text.push_str(&ARTICLE[line.paragraph][line.consumed.clone()]);
        }
        node.value = Some(SemanticsValue::Text(text));
        ctx.push(node);
        self.controls.semantics(ctx);
        for (index, circle) in self.circles.iter().enumerate() {
            let mut orb = SemanticsNode::new(
                Self::orb_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                Rect::new(
                    ctx.bounds().x() + circle.center.x - circle.radius,
                    ctx.bounds().y() + circle.center.y - circle.radius,
                    circle.radius * 2.0,
                    circle.radius * 2.0,
                ),
            );
            orb.parent = Some(ctx.widget_id());
            orb.name = Some(format!("Toggle circle {}", index + 1));
            orb.actions = vec![SemanticsAction::Activate];
            orb.state.selected = self.state.get().orbs[index].paused;
            ctx.push(orb);
        }
    }
    fn accepts_focus(&self) -> bool {
        true
    }
    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        visitor.visit(&self.controls);
    }
    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        visitor.visit(&mut self.controls);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui::{Application, PointerEvent, WgpuRenderer, WindowBuilder, WindowEvent};
    use sui_text::{FontRegistry, TextSystem};

    fn runtime(state: Signal<Motion>, width: f32) -> sui::Result<(sui::Runtime, sui::WindowId)> {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Editorial engine")
                    .root(Editorial::new(
                        state,
                        crate::app::default_dev_theme_reader(),
                    )),
            )
            .build()?;
        let window = runtime.window_ids()[0];
        runtime.handle_event(
            window,
            Event::Window(WindowEvent::Resized(Size::new(width, 900.0))),
        )?;
        Ok((runtime, window))
    }

    #[test]
    fn editorial_slots_subtract_overlapping_obstacles() {
        let column = Rect::new(0.0, 0.0, 500.0, 500.0);
        let circles = [Circle {
            center: Point::new(250.0, 100.0),
            radius: 50.0,
        }];
        assert_eq!(
            line_slots(column, 90.0, &circles, &[]),
            vec![0.0..188.0, 312.0..500.0]
        );
        assert_eq!(
            line_slots(
                column,
                90.0,
                &circles,
                &[Rect::new(300.0, 80.0, 140.0, 80.0)]
            ),
            vec![0.0..188.0, 440.0..500.0]
        );
        assert!(line_slots(column, 90.0, &circles, &[column]).is_empty());
        assert_eq!(line_slots(column, 200.0, &circles, &[]), vec![0.0..500.0]);
    }

    #[test]
    fn editorial_flow_preserves_every_byte_across_slots_and_columns() -> sui::Result<()> {
        let system = TextSystem::new();
        let fonts = FontRegistry::new();
        for width in [140.0, 250.0, 390.0] {
            let columns: Vec<_> = (0..30)
                .map(|i| Rect::new(i as f32 * (width + 28.0), 0.0, width, 500.0))
                .collect();
            let circles = [Circle {
                center: Point::new(width * 0.5, 210.0),
                radius: 36.0,
            }];
            let (lines, end) =
                flow_article(&columns, &circles, &[], &[], |text, width, handle| {
                    system.layout_document_persistent(
                        handle,
                        TextLayoutRequest::new(TextDocument::from_plain_text(
                            text,
                            serif(17.0, LINE_HEIGHT, PAPER),
                        ))
                        .with_box_size(Size::new(width, LINE_HEIGHT)),
                        &fonts,
                    )
                })?;
            let mut cursor = Cursor {
                paragraph: 0,
                byte: 1,
            };
            let mut actual = String::from("T");
            for line in lines {
                assert_eq!(line.paragraph, cursor.paragraph);
                assert_eq!(line.consumed.start, cursor.byte);
                assert!(line.consumed.end > line.consumed.start);
                actual.push_str(&ARTICLE[line.paragraph][line.consumed.clone()]);
                cursor.byte = line.consumed.end;
                if cursor.byte == ARTICLE[cursor.paragraph].len() {
                    cursor.paragraph += 1;
                    cursor.byte = 0;
                }
                let column = columns
                    .iter()
                    .find(|column| {
                        line.bounds.x() >= column.x() && line.bounds.max_x() <= column.max_x()
                    })
                    .unwrap();
                assert!(
                    line_slots(*column, line.bounds.y(), &circles, &[])
                        .iter()
                        .any(
                            |slot| line.bounds.x() >= slot.start && line.bounds.max_x() <= slot.end
                        )
                );
            }
            assert_eq!(cursor, end);
            assert_eq!(end.paragraph, ARTICLE.len());
            assert_eq!(actual, ARTICLE.concat());
        }
        Ok(())
    }

    #[test]
    fn editorial_scheduled_motion_controls_and_drag_work() -> sui::Result<()> {
        let state = Signal::new(Motion::default());
        let (mut runtime, window) = runtime(state.clone(), 1280.0)?;
        runtime.render(window)?;
        let initial = state.get();
        for i in 1..=3 {
            runtime.tick(i as f64 / 60.0);
            for (id, event) in runtime.drain_ready_events() {
                runtime.handle_event(id, event)?;
            }
            runtime.render(window)?;
        }
        assert_ne!(initial.orbs[0].position, state.get().orbs[0].position);
        let output = runtime.render(window)?;
        let pause = output
            .semantics
            .iter()
            .find(|n| n.name.as_deref() == Some("Pause all"))
            .unwrap()
            .id;
        runtime.handle_semantics_action(window, pause, SemanticsActionRequest::Activate)?;
        let stopped = state.get();
        for i in 4..=30 {
            runtime.tick(i as f64 / 60.0);
            for (id, event) in runtime.drain_ready_events() {
                runtime.handle_event(id, event)?;
            }
            runtime.render(window)?;
        }
        assert_eq!(stopped, state.get());
        assert!(runtime.next_wakeup_time(window)?.is_none());
        let output = runtime.render(window)?;
        let orb = output
            .semantics
            .iter()
            .find(|n| n.name.as_deref() == Some("Toggle circle 1"))
            .unwrap();
        let center = Point::new(
            orb.bounds.x() + orb.bounds.width() * 0.5,
            orb.bounds.y() + orb.bounds.height() * 0.5,
        );
        let mut down = PointerEvent::new(PointerEventKind::Down, center);
        down.button = Some(PointerButton::Primary);
        runtime.handle_event(window, Event::Pointer(down))?;
        assert!(state.get().orbs[0].dragging);
        runtime.handle_event(window, Event::Window(WindowEvent::RedrawRequested))?;
        runtime.render(window)?;
        assert!(state.get().orbs[0].dragging);
        let destination = Point::new(center.x + 40.0, center.y + 20.0);
        runtime.handle_event(
            window,
            Event::Pointer(PointerEvent::new(PointerEventKind::Move, destination)),
        )?;
        runtime.handle_event(
            window,
            Event::Pointer(PointerEvent::new(PointerEventKind::Up, destination)),
        )?;
        assert!(!state.get().orbs[0].dragging);
        assert!(!state.get().orbs[0].paused);
        assert_ne!(stopped.orbs[0].position, state.get().orbs[0].position);
        runtime.handle_semantics_action(window, orb.id, SemanticsActionRequest::Activate)?;
        assert!(state.get().orbs[0].paused);
        for width in [360.0, 800.0, 1280.0] {
            runtime.handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(width, 900.0))),
            )?;
            let output = runtime.render(window)?;
            let node = output
                .semantics
                .iter()
                .find(|n| n.name.as_deref() == Some(NAME))
                .unwrap();
            let Some(SemanticsValue::Text(text)) = &node.value else {
                panic!("article missing")
            };
            assert!(text.len() > 100);
            assert!(ARTICLE.concat().starts_with(text));
        }
        assert_eq!(
            crate::app::dev_demo_label_for_slug("editorial"),
            Some(EDITORIAL_TAB_LABEL)
        );
        Ok(())
    }

    #[test]
    #[ignore = "writes narrow, medium and wide GPU-rendered screenshots"]
    fn editorial_visual_capture() -> sui::Result<()> {
        let state = Signal::new(Motion {
            playing: false,
            ..Motion::default()
        });
        let (mut runtime, window) = runtime(state, 1280.0)?;
        let mut renderer = WgpuRenderer::new();
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/editorial-demo");
        std::fs::create_dir_all(&directory).unwrap();
        for width in [360.0, 800.0, 1280.0] {
            runtime.handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(width, 900.0))),
            )?;
            renderer.render(&runtime.render(window)?.frame)?;
            let image = renderer.capture_rgba(window)?;
            let path = directory.join(format!("width-{width:.0}.png"));
            let mut encoder = png::Encoder::new(
                std::io::BufWriter::new(std::fs::File::create(&path).unwrap()),
                image.width(),
                image.height(),
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(image.pixels())
                .unwrap();
            println!("{}", path.display());
        }
        Ok(())
    }

    #[test]
    #[ignore = "paced CPU frame benchmark; run serially on an idle machine"]
    fn editorial_frame_profile() -> sui::Result<()> {
        let state = Signal::new(Motion::default());
        let (mut runtime, window) = runtime(state.clone(), 1280.0)?;
        let mut renderer = WgpuRenderer::new();
        renderer.render(&runtime.render(window)?.frame)?;
        let mut time = 0.0;
        for playing in [false, true] {
            state.set(Motion {
                playing,
                ..Motion::default()
            });
            let mut samples = Vec::new();
            let (mut runtime_total, mut renderer_total, mut text_us) = (0.0, 0.0, 0.0);
            for frame in 0..330 {
                std::thread::sleep(std::time::Duration::from_millis(17));
                time += 1.0 / 60.0;
                let start = Instant::now();
                runtime.tick(time);
                runtime.handle_event(window, Event::Window(WindowEvent::RedrawRequested))?;
                for (id, event) in runtime.drain_ready_events() {
                    runtime.handle_event(id, event)?;
                }
                let output = runtime.render(window)?;
                let runtime_ms = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                renderer.render(&output.frame)?;
                let renderer_ms = start.elapsed().as_secs_f64() * 1000.0;
                if frame >= 30 {
                    runtime_total += runtime_ms;
                    renderer_total += renderer_ms;
                    text_us += output.diagnostics.runtime_text_timing.total_time_us as f64;
                    samples.push(runtime_ms + renderer_ms);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "EDITORIAL playing={playing} viewport=1280x900 frames=300 runtime_mean_ms={:.3} renderer_mean_ms={:.3} text_mean_ms={:.3} cpu_p50_ms={:.3} cpu_p95_ms={:.3} text_timing={}",
                runtime_total / 300.0,
                renderer_total / 300.0,
                text_us / 300000.0,
                samples[150],
                samples[285],
                std::env::var_os("SUI_PROFILE_TEXT_TIMINGS").is_some()
            );
        }
        Ok(())
    }
}
