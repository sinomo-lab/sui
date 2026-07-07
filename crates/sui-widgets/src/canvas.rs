use std::{cell::RefCell, rc::Rc};

use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, PathElement, Point, PointerButton, PointerEventKind,
    Rect, ScrollDelta, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
    Transform, Vector, WakeEvent,
};
use sui_layout::Constraints;
use sui_runtime::{ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::{ImageSampling, ImageSource, RegisteredImage, StrokeStyle};
use sui_text::{FontFeature, TextMeasurement, TextStyle};

use crate::{DefaultTheme, animation::MotionScalar, text_align::paint_aligned_text};

const AXIS_ALIGNED_EPSILON: f32 = 0.0001;
const PIXEL_CANVAS_HISTORY_LIMIT: usize = 32;
const CANVAS_RULER_MAX_TICKS: usize = 400;

type AnimatedScalar = MotionScalar;

fn set_focus_animation_target(
    animation: &mut AnimatedScalar,
    target: f32,
    theme: &DefaultTheme,
    ctx: &mut EventCtx,
) {
    animation.set_target_event(
        target,
        theme.motion.focus_duration(),
        theme.motion.focus_easing(),
        ctx,
    );
}

fn draw_focus_ring(ctx: &mut PaintCtx, bounds: Rect, theme: &DefaultTheme, progress: f32) {
    if progress <= AnimatedScalar::EPSILON || bounds.is_empty() {
        return;
    }

    let metrics = theme.metrics;
    let outset = physical_pixels(ctx, metrics.focus_ring_outset);
    ctx.stroke(
        Path::rounded_rect(
            bounds.inflate(outset, outset),
            metrics.corner_radius + outset,
        ),
        theme
            .palette
            .focus_ring
            .with_alpha(theme.palette.focus_ring.alpha * progress),
        StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
    );
}

fn physical_pixels(ctx: &PaintCtx, value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    ctx.dpi().physical_pixels_to_logical(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasRulerAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasViewport {
    pub pan: Vector,
    pub zoom: f32,
    pub rotation: f32,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            pan: Vector::ZERO,
            zoom: 1.0,
            rotation: 0.0,
        }
    }
}

impl CanvasViewport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pan(mut self, pan: Vector) -> Self {
        self.pan = pan;
        self
    }

    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom.max(0.01);
        self
    }

    pub fn rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }

    fn center(bounds: Rect) -> Point {
        Point::new(
            bounds.x() + (bounds.width() * 0.5),
            bounds.y() + (bounds.height() * 0.5),
        )
    }

    fn transform(self, bounds: Rect, document_origin: Point) -> Transform {
        let center = Self::center(bounds) + self.pan;
        Transform::translation(-document_origin.x, -document_origin.y)
            .then(Transform::scale(self.zoom, self.zoom))
            .then(Transform::rotation(self.rotation))
            .then(Transform::translation(center.x, center.y))
    }

    fn screen_to_world(self, bounds: Rect, point: Point, document_origin: Point) -> Point {
        let center = Self::center(bounds) + self.pan;
        let relative = point - center;
        let (sin, cos) = (-self.rotation).sin_cos();
        let rotated = Vector::new(
            (relative.x * cos) + (relative.y * -sin),
            (relative.x * sin) + (relative.y * cos),
        );
        Point::new(
            document_origin.x + (rotated.x / self.zoom),
            document_origin.y + (rotated.y / self.zoom),
        )
    }

    fn world_to_screen(self, bounds: Rect, point: Point, document_origin: Point) -> Point {
        self.transform(bounds, document_origin)
            .transform_point(point)
    }

    fn zoom_around(&mut self, bounds: Rect, anchor: Point, factor: f32, document_origin: Point) {
        let before = self.screen_to_world(bounds, anchor, document_origin);
        self.zoom = (self.zoom * factor.max(0.01)).max(0.01);
        let after = self.world_to_screen(bounds, before, document_origin);
        self.pan += anchor - after;
    }

    fn rotate_around(&mut self, bounds: Rect, anchor: Point, radians: f32, document_origin: Point) {
        let before = self.screen_to_world(bounds, anchor, document_origin);
        self.rotation += radians;
        let after = self.world_to_screen(bounds, before, document_origin);
        self.pan += anchor - after;
    }
}

pub struct CanvasRuler {
    theme: DefaultTheme,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    axis: CanvasRulerAxis,
    name: String,
    document_size: Size,
    viewport: CanvasViewport,
    viewport_size: Size,
    viewport_reader: Option<Box<dyn Fn() -> (CanvasViewport, Size)>>,
    extent: Option<f32>,
}

impl CanvasRuler {
    pub fn new(axis: CanvasRulerAxis, name: impl Into<String>, document_size: Size) -> Self {
        Self {
            theme: DefaultTheme::default(),
            theme_reader: None,
            axis,
            name: name.into(),
            document_size: Size::new(document_size.width.max(1.0), document_size.height.max(1.0)),
            viewport: CanvasViewport::default(),
            viewport_size: Size::ZERO,
            viewport_reader: None,
            extent: None,
        }
    }

    pub fn horizontal(name: impl Into<String>, document_size: Size) -> Self {
        Self::new(CanvasRulerAxis::Horizontal, name, document_size)
    }

    pub fn vertical(name: impl Into<String>, document_size: Size) -> Self {
        Self::new(CanvasRulerAxis::Vertical, name, document_size)
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn viewport(mut self, viewport: CanvasViewport, viewport_size: Size) -> Self {
        self.viewport = viewport;
        self.viewport_size = viewport_size;
        self.viewport_reader = None;
        self
    }

    pub fn viewport_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> (CanvasViewport, Size) + 'static,
    {
        self.viewport_reader = Some(Box::new(reader));
        self
    }

    pub fn extent(mut self, extent: f32) -> Self {
        self.extent = Some(extent.max(0.0));
        self
    }

    fn viewport_snapshot(&self) -> (CanvasViewport, Size) {
        self.viewport_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or((self.viewport, self.viewport_size))
    }

    fn document_axis_length(&self) -> f32 {
        match self.axis {
            CanvasRulerAxis::Horizontal => self.document_size.width,
            CanvasRulerAxis::Vertical => self.document_size.height,
        }
    }

    fn axis_label(&self) -> &'static str {
        match self.axis {
            CanvasRulerAxis::Horizontal => "horizontal",
            CanvasRulerAxis::Vertical => "vertical",
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(self.theme)
    }

    fn resolved_extent(&self, theme: &DefaultTheme) -> f32 {
        self.extent
            .unwrap_or(theme.metrics.canvas_ruler_extent)
            .max(0.0)
    }
}

impl Widget for CanvasRuler {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let extent = self.resolved_extent(&theme);
        let natural = match self.axis {
            CanvasRulerAxis::Horizontal => Size::new(
                if constraints.max.width.is_finite() {
                    constraints.max.width
                } else {
                    320.0
                },
                extent,
            ),
            CanvasRulerAxis::Vertical => Size::new(
                extent,
                if constraints.max.height.is_finite() {
                    constraints.max.height
                } else {
                    240.0
                },
            ),
        };

        constraints.clamp(natural)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let (viewport, viewport_size) = self.viewport_snapshot();
        let text_style = TextStyle {
            font_size: theme.text.xs.size,
            line_height: theme.text.xs.line_height,
            color: theme.surfaces.canvas_ruler_text,
            ..theme.body_text_style()
        };

        ctx.fill_rect(bounds, theme.surfaces.canvas_ruler);
        paint_canvas_ruler_divider(ctx, bounds, self.axis, theme.surfaces.canvas_ruler_border);
        ctx.push_clip_rect(bounds);
        paint_canvas_ruler_ticks(
            ctx,
            bounds,
            self.axis,
            self.document_size,
            viewport,
            viewport_size,
            &theme,
            text_style,
        );
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(format!(
            "{} ruler, {:.0} px document axis",
            self.axis_label(),
            self.document_axis_length()
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasStroke {
    pub color: Color,
    pub width: f32,
}

impl CanvasStroke {
    pub const fn new(color: Color, width: f32) -> Self {
        Self { color, width }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasShape {
    Path {
        path: Path,
        fill: Option<Color>,
        stroke: Option<CanvasStroke>,
    },
}

impl CanvasShape {
    pub fn path(path: Path) -> Self {
        let accent = DefaultTheme::default().palette.accent;
        Self::Path {
            path,
            fill: None,
            stroke: Some(CanvasStroke::new(accent, 2.0)),
        }
    }

    pub fn rect(rect: Rect, fill: Option<Color>, stroke: Option<CanvasStroke>) -> Self {
        Self::Path {
            path: Path::rect(rect),
            fill,
            stroke,
        }
    }

    pub fn circle(
        center: Point,
        radius: f32,
        fill: Option<Color>,
        stroke: Option<CanvasStroke>,
    ) -> Self {
        Self::Path {
            path: Path::circle(center, radius),
            fill,
            stroke,
        }
    }

    pub fn polyline(points: &[Point], stroke: CanvasStroke) -> Option<Self> {
        path_from_points(points).map(|path| Self::Path {
            path,
            fill: None,
            stroke: Some(stroke),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CanvasDrag {
    Pan {
        pointer_id: u64,
        last_position: Point,
    },
    Draw {
        pointer_id: u64,
    },
}

pub struct Canvas {
    theme: DefaultTheme,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    viewport: CanvasViewport,
    shapes: Vec<CanvasShape>,
    active_stroke: Option<Vec<Point>>,
    drag: Option<CanvasDrag>,
    draw_stroke: CanvasStroke,
    focus_animation: AnimatedScalar,
    desired_size: Size,
}

impl Canvas {
    pub fn new(name: impl Into<String>) -> Self {
        let theme = DefaultTheme::default();
        Self {
            theme,
            theme_reader: None,
            name: name.into(),
            viewport: CanvasViewport::default(),
            shapes: Vec::new(),
            active_stroke: None,
            drag: None,
            draw_stroke: CanvasStroke::new(theme.palette.accent, 2.5),
            focus_animation: AnimatedScalar::new(0.0),
            desired_size: Size::new(520.0, 360.0),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn viewport(mut self, viewport: CanvasViewport) -> Self {
        self.viewport = viewport;
        self
    }

    pub fn desired_size(mut self, size: Size) -> Self {
        self.desired_size = Size::new(size.width.max(1.0), size.height.max(1.0));
        self
    }

    pub fn shape(mut self, shape: CanvasShape) -> Self {
        self.shapes.push(shape);
        self
    }

    pub fn shapes<I>(mut self, shapes: I) -> Self
    where
        I: IntoIterator<Item = CanvasShape>,
    {
        self.shapes.extend(shapes);
        self
    }

    pub fn draw_stroke(mut self, stroke: CanvasStroke) -> Self {
        self.draw_stroke = CanvasStroke::new(stroke.color, stroke.width.max(0.1));
        self
    }

    pub fn viewport_state(&self) -> CanvasViewport {
        self.viewport
    }

    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    fn document_origin(&self) -> Point {
        Point::ZERO
    }

    fn world_position(&self, bounds: Rect, position: Point) -> Point {
        self.viewport
            .screen_to_world(bounds, position, self.document_origin())
    }

    fn push_active_point(&mut self, point: Point) {
        let Some(points) = &mut self.active_stroke else {
            return;
        };
        if points
            .last()
            .is_none_or(|last| vector_length(point - *last) >= 1.5)
        {
            points.push(point);
        }
    }

    fn finish_active_stroke(&mut self) {
        let Some(points) = self.active_stroke.take() else {
            return;
        };
        if let Some(shape) = CanvasShape::polyline(&points, self.draw_stroke) {
            self.shapes.push(shape);
        } else if let Some(point) = points.first().copied() {
            self.shapes.push(CanvasShape::circle(
                point,
                self.draw_stroke.width.max(1.0),
                Some(self.draw_stroke.color),
                None,
            ));
        }
    }

    fn request_interaction_update(ctx: &mut EventCtx) {
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(self.theme)
    }
}

impl Widget for Canvas {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = scroll_delta_to_offset(pointer.scroll_delta, pointer.delta);
                if pointer.modifiers.shift {
                    self.viewport.rotate_around(
                        ctx.bounds(),
                        pointer.position,
                        delta.y * 0.01,
                        self.document_origin(),
                    );
                } else {
                    self.viewport.zoom_around(
                        ctx.bounds(),
                        pointer.position,
                        (delta.y * 0.002).exp(),
                        self.document_origin(),
                    );
                }
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position)
                    && matches!(
                        pointer.button,
                        Some(PointerButton::Middle | PointerButton::Secondary)
                    ) =>
            {
                self.drag = Some(CanvasDrag::Pan {
                    pointer_id: pointer.pointer_id,
                    last_position: pointer.position,
                });
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                let point = self.world_position(ctx.bounds(), pointer.position);
                self.active_stroke = Some(vec![point]);
                self.drag = Some(CanvasDrag::Draw {
                    pointer_id: pointer.pointer_id,
                });
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => match self.drag {
                Some(CanvasDrag::Pan {
                    pointer_id,
                    mut last_position,
                }) if pointer_id == pointer.pointer_id => {
                    let delta = pointer.position - last_position;
                    self.viewport.pan += delta;
                    last_position = pointer.position;
                    self.drag = Some(CanvasDrag::Pan {
                        pointer_id,
                        last_position,
                    });
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
                Some(CanvasDrag::Draw { pointer_id }) if pointer_id == pointer.pointer_id => {
                    let point = self.world_position(ctx.bounds(), pointer.position);
                    self.push_active_point(point);
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
                _ => {}
            },
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    || pointer.kind == PointerEventKind::Cancel =>
            {
                let active_pointer = match self.drag {
                    Some(CanvasDrag::Pan { pointer_id, .. } | CanvasDrag::Draw { pointer_id }) => {
                        Some(pointer_id)
                    }
                    None => None,
                };
                if active_pointer == Some(pointer.pointer_id) {
                    if matches!(self.drag, Some(CanvasDrag::Draw { .. })) {
                        self.finish_active_stroke();
                    }
                    self.drag = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let zoom_step = self.resolved_theme().metrics.pixel_canvas_zoom_step;
                match key.key.as_str() {
                    "=" | "+" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        zoom_step,
                        self.document_origin(),
                    ),
                    "-" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        1.0 / zoom_step,
                        self.document_origin(),
                    ),
                    "[" => self.viewport.rotate_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        -0.1,
                        self.document_origin(),
                    ),
                    "]" => self.viewport.rotate_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        0.1,
                        self.document_origin(),
                    ),
                    "ArrowLeft" => self.viewport.pan.x += 24.0,
                    "ArrowRight" => self.viewport.pan.x -= 24.0,
                    "ArrowUp" => self.viewport.pan.y += 24.0,
                    "ArrowDown" => self.viewport.pan.y -= 24.0,
                    _ => return,
                }
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.focus_animation.advance(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                self.desired_size.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                self.desired_size.height
            },
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        ctx.fill_bounds(theme.surfaces.canvas);
        ctx.stroke_bounds(
            theme.surfaces.border,
            StrokeStyle::new(theme.metrics.border_width),
        );
        ctx.push_clip_rect(ctx.bounds());
        paint_canvas_grid(
            ctx,
            self.viewport,
            ctx.bounds(),
            self.document_origin(),
            &theme,
        );
        paint_canvas_axes(
            ctx,
            self.viewport,
            ctx.bounds(),
            self.document_origin(),
            &theme,
        );
        let transform = self
            .viewport
            .transform(ctx.bounds(), self.document_origin());
        for shape in &self.shapes {
            paint_canvas_shape(ctx, shape, transform, self.viewport.zoom);
        }
        if let Some(points) = &self.active_stroke {
            if let Some(shape) = CanvasShape::polyline(points, self.draw_stroke) {
                paint_canvas_shape(ctx, &shape, transform, self.viewport.zoom);
            }
        }
        ctx.pop_clip();
        draw_focus_ring(ctx, ctx.bounds(), &theme, self.focus_animation.value);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = Some(SemanticsValue::Text(format!(
            "zoom {:.0}%, rotation {:.0} deg",
            self.viewport.zoom * 100.0,
            self.viewport.rotation.to_degrees()
        )));
        node.state.focused = ctx.is_focused();
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Custom("Pan".into()),
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        Self::request_interaction_update(ctx);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PixelCanvasDrag {
    Pan {
        pointer_id: u64,
        last_position: Point,
    },
    Paint {
        pointer_id: u64,
        last_position: Point,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PixelCanvasHistoryCommand {
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PixelCanvasViewportCommand {
    Fit,
    ActualSize,
    ZoomIn,
    ZoomOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelEdit {
    index: usize,
    before: PixelColor,
    after: PixelColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl PixelColor {
    const TRANSPARENT: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
    };

    fn from_color(color: Color) -> Self {
        let color = color.clamped();
        Self {
            red: channel_to_u8(color.red),
            green: channel_to_u8(color.green),
            blue: channel_to_u8(color.blue),
            alpha: channel_to_u8(color.alpha),
        }
    }

    fn to_color(self) -> Color {
        Color::rgba(
            self.red as f32 / 255.0,
            self.green as f32 / 255.0,
            self.blue as f32 / 255.0,
            self.alpha as f32 / 255.0,
        )
    }

    fn compose(self, source: Color, opacity: f32, blend_mode: PixelCanvasBlendMode) -> Self {
        let destination = self.to_color();
        let source = source.clamped();
        let opacity = opacity.clamp(0.0, 1.0);
        let source_alpha = (source.alpha * opacity).clamp(0.0, 1.0);
        if source_alpha <= 0.0 {
            return self;
        }

        let destination_alpha = destination.alpha.clamp(0.0, 1.0);
        let blend_red = blend_channel(source.red, destination.red, destination_alpha, blend_mode);
        let blend_green = blend_channel(
            source.green,
            destination.green,
            destination_alpha,
            blend_mode,
        );
        let blend_blue =
            blend_channel(source.blue, destination.blue, destination_alpha, blend_mode);
        let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
        if output_alpha <= 0.0 {
            return Self::TRANSPARENT;
        }

        let destination_weight = destination_alpha * (1.0 - source_alpha);
        Self::from_color(Color::rgba(
            ((blend_red * source_alpha) + (destination.red * destination_weight)) / output_alpha,
            ((blend_green * source_alpha) + (destination.green * destination_weight))
                / output_alpha,
            ((blend_blue * source_alpha) + (destination.blue * destination_weight)) / output_alpha,
            output_alpha,
        ))
    }

    fn erased(self, opacity: f32) -> Self {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return self;
        }
        let scale = 1.0 - opacity;
        Self {
            alpha: channel_to_u8((self.alpha as f32 / 255.0) * scale),
            ..self
        }
    }
}

fn blend_channel(
    source: f32,
    destination: f32,
    destination_alpha: f32,
    mode: PixelCanvasBlendMode,
) -> f32 {
    if destination_alpha <= 0.0 {
        return source.clamp(0.0, 1.0);
    }

    match mode {
        PixelCanvasBlendMode::Normal => source,
        PixelCanvasBlendMode::Multiply => source * destination,
        PixelCanvasBlendMode::Screen => 1.0 - ((1.0 - source) * (1.0 - destination)),
        PixelCanvasBlendMode::Overlay => {
            if destination <= 0.5 {
                2.0 * source * destination
            } else {
                1.0 - (2.0 * (1.0 - source) * (1.0 - destination))
            }
        }
    }
    .clamp(0.0, 1.0)
}

fn brush_shape_contains_pixel(
    shape: PixelCanvasBrushShape,
    size: isize,
    start_x: isize,
    start_y: isize,
    px: isize,
    py: isize,
) -> bool {
    match shape {
        PixelCanvasBrushShape::Square => true,
        PixelCanvasBrushShape::Round => {
            let size = size.max(1) as f32;
            let center = (size - 1.0) * 0.5;
            let local_x = (px - start_x) as f32 - center;
            let local_y = (py - start_y) as f32 - center;
            let radius = (size * 0.5 - 0.25).max(0.0);
            (local_x * local_x) + (local_y * local_y) <= radius * radius
        }
    }
}

fn brush_stroke_spacing(brush_size: f32) -> f32 {
    (brush_size.round().max(1.0) * 0.25).max(1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelCanvasTool {
    #[default]
    Brush,
    Eraser,
    Fill,
    Pan,
}

impl PixelCanvasTool {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Pan => "Pan",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelCanvasBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
}

impl PixelCanvasBlendMode {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Multiply, Self::Screen, Self::Overlay];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelCanvasBrushShape {
    #[default]
    Square,
    Round,
}

impl PixelCanvasBrushShape {
    pub const ALL: [Self; 2] = [Self::Square, Self::Round];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Square => "Square",
            Self::Round => "Round",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PixelCanvasStateInner {
    tool: PixelCanvasTool,
    brush: Color,
    brush_size: f32,
    brush_opacity: f32,
    brush_shape: PixelCanvasBrushShape,
    blend_mode: PixelCanvasBlendMode,
    display_visible: bool,
    display_opacity: f32,
    display_blend_mode: PixelCanvasBlendMode,
    display_above_paper: bool,
    paper_visible: bool,
    paper_opacity: f32,
    pending_undo: u32,
    pending_redo: u32,
    pending_fit_view: u32,
    pending_actual_size: u32,
    pending_zoom_delta: i32,
    pending_export: u32,
    pending_clear: u32,
    export_revision: u64,
    latest_export: Option<PixelCanvasExportSnapshot>,
    editable: bool,
    can_undo: bool,
    can_redo: bool,
    can_clear: bool,
    viewport: CanvasViewport,
    viewport_size: Size,
    cursor_position: Option<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelCanvasExportSnapshot {
    revision: u64,
    name: String,
    width: usize,
    height: usize,
    rgba8: Rc<[u8]>,
}

impl PixelCanvasExportSnapshot {
    fn new(revision: u64, name: String, width: usize, height: usize, rgba8: Vec<u8>) -> Self {
        Self {
            revision,
            name,
            width,
            height,
            rgba8: Rc::from(rgba8.into_boxed_slice()),
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    pub fn rgba8(&self) -> &[u8] {
        self.rgba8.as_ref()
    }

    pub fn byte_len(&self) -> usize {
        self.rgba8.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PixelCanvasBrushSettings {
    tool: PixelCanvasTool,
    brush: Color,
    brush_size: f32,
    brush_opacity: f32,
    brush_shape: PixelCanvasBrushShape,
    blend_mode: PixelCanvasBlendMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PixelCanvasDisplaySettings {
    visible: bool,
    opacity: f32,
    blend_mode: PixelCanvasBlendMode,
}

impl PixelCanvasDisplaySettings {
    const DEFAULT: Self = Self {
        visible: true,
        opacity: 1.0,
        blend_mode: PixelCanvasBlendMode::Normal,
    };

    fn requires_compositing(self) -> bool {
        !self.visible || self.opacity < 0.999 || self.blend_mode != PixelCanvasBlendMode::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PixelCanvasPaperSettings {
    visible: bool,
    opacity: f32,
}

impl PixelCanvasPaperSettings {
    const DEFAULT: Self = Self {
        visible: true,
        opacity: 1.0,
    };

    fn requires_compositing(self) -> bool {
        !self.visible || self.opacity < 0.999
    }

    fn pixel(self, paper_color: Color) -> PixelColor {
        if self.visible {
            PixelColor::from_color(paper_color.with_alpha(self.opacity))
        } else {
            PixelColor::TRANSPARENT
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PixelCanvasImageSource {
    Raw,
    Composited {
        display: PixelCanvasDisplaySettings,
        paper: PixelCanvasPaperSettings,
        display_above_paper: bool,
        paper_color: Color,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PixelCanvasImageCacheKey {
    pixel_revision: u64,
    source: PixelCanvasImageSource,
}

#[derive(Debug, Clone)]
struct PixelCanvasImageCache {
    key: PixelCanvasImageCacheKey,
    image: RegisteredImage,
}

#[derive(Clone, Debug)]
pub struct PixelCanvasState {
    inner: Rc<RefCell<PixelCanvasStateInner>>,
}

impl PixelCanvasState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn brush_color(&self) -> Color {
        self.inner.borrow().brush
    }

    pub fn tool(&self) -> PixelCanvasTool {
        self.inner.borrow().tool
    }

    pub fn set_tool(&self, tool: PixelCanvasTool) {
        self.inner.borrow_mut().tool = tool;
    }

    pub fn request_undo(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_undo = inner.pending_undo.saturating_add(1);
    }

    pub fn request_redo(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_redo = inner.pending_redo.saturating_add(1);
    }

    pub fn request_fit_view(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_fit_view = inner.pending_fit_view.saturating_add(1);
    }

    pub fn request_actual_size_view(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_actual_size = inner.pending_actual_size.saturating_add(1);
    }

    pub fn request_zoom_in(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_zoom_delta = inner.pending_zoom_delta.saturating_add(1);
    }

    pub fn request_zoom_out(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_zoom_delta = inner.pending_zoom_delta.saturating_sub(1);
    }

    pub fn request_export_snapshot(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_export = inner.pending_export.saturating_add(1);
    }

    pub fn request_clear(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_clear = inner.pending_clear.saturating_add(1);
    }

    pub fn is_editable(&self) -> bool {
        self.inner.borrow().editable
    }

    pub fn set_editable(&self, editable: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.editable == editable {
            return false;
        }
        inner.editable = editable;
        if !editable {
            inner.can_undo = false;
            inner.can_redo = false;
            inner.can_clear = false;
        }
        true
    }

    pub fn latest_export_snapshot(&self) -> Option<PixelCanvasExportSnapshot> {
        self.inner.borrow().latest_export.clone()
    }

    pub fn can_undo(&self) -> bool {
        self.inner.borrow().can_undo
    }

    pub fn can_redo(&self) -> bool {
        self.inner.borrow().can_redo
    }

    pub fn can_clear(&self) -> bool {
        self.inner.borrow().can_clear
    }

    pub fn viewport(&self) -> CanvasViewport {
        self.inner.borrow().viewport
    }

    pub fn viewport_size(&self) -> Size {
        self.inner.borrow().viewport_size
    }

    pub fn cursor_position(&self) -> Option<Point> {
        self.inner.borrow().cursor_position
    }

    pub fn set_brush_color(&self, color: Color) {
        self.inner.borrow_mut().brush = color;
    }

    pub fn brush_size(&self) -> f32 {
        self.inner.borrow().brush_size
    }

    pub fn set_brush_size(&self, size: f32) {
        self.inner.borrow_mut().brush_size = size.max(1.0);
    }

    pub fn brush_opacity(&self) -> f32 {
        self.inner.borrow().brush_opacity
    }

    pub fn set_brush_opacity(&self, opacity: f32) {
        self.inner.borrow_mut().brush_opacity = opacity.clamp(0.0, 1.0);
    }

    pub fn brush_shape(&self) -> PixelCanvasBrushShape {
        self.inner.borrow().brush_shape
    }

    pub fn set_brush_shape(&self, brush_shape: PixelCanvasBrushShape) {
        self.inner.borrow_mut().brush_shape = brush_shape;
    }

    pub fn blend_mode(&self) -> PixelCanvasBlendMode {
        self.inner.borrow().blend_mode
    }

    pub fn set_blend_mode(&self, blend_mode: PixelCanvasBlendMode) {
        self.inner.borrow_mut().blend_mode = blend_mode;
    }

    pub fn display_visible(&self) -> bool {
        self.inner.borrow().display_visible
    }

    pub fn set_display_visible(&self, visible: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.display_visible == visible {
            return false;
        }
        inner.display_visible = visible;
        true
    }

    pub fn display_opacity(&self) -> f32 {
        self.inner.borrow().display_opacity
    }

    pub fn set_display_opacity(&self, opacity: f32) -> bool {
        let opacity = opacity.clamp(0.0, 1.0);
        let mut inner = self.inner.borrow_mut();
        if (inner.display_opacity - opacity).abs() < f32::EPSILON {
            return false;
        }
        inner.display_opacity = opacity;
        true
    }

    pub fn display_blend_mode(&self) -> PixelCanvasBlendMode {
        self.inner.borrow().display_blend_mode
    }

    pub fn set_display_blend_mode(&self, blend_mode: PixelCanvasBlendMode) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.display_blend_mode == blend_mode {
            return false;
        }
        inner.display_blend_mode = blend_mode;
        true
    }

    pub fn display_above_paper(&self) -> bool {
        self.inner.borrow().display_above_paper
    }

    pub fn set_display_above_paper(&self, above: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.display_above_paper == above {
            return false;
        }
        inner.display_above_paper = above;
        true
    }

    pub fn paper_visible(&self) -> bool {
        self.inner.borrow().paper_visible
    }

    pub fn set_paper_visible(&self, visible: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.paper_visible == visible {
            return false;
        }
        inner.paper_visible = visible;
        true
    }

    pub fn paper_opacity(&self) -> f32 {
        self.inner.borrow().paper_opacity
    }

    pub fn set_paper_opacity(&self, opacity: f32) -> bool {
        let opacity = opacity.clamp(0.0, 1.0);
        let mut inner = self.inner.borrow_mut();
        if (inner.paper_opacity - opacity).abs() < f32::EPSILON {
            return false;
        }
        inner.paper_opacity = opacity;
        true
    }

    fn brush(&self) -> PixelCanvasBrushSettings {
        let inner = self.inner.borrow();
        PixelCanvasBrushSettings {
            tool: inner.tool,
            brush: inner.brush,
            brush_size: inner.brush_size,
            brush_opacity: inner.brush_opacity,
            brush_shape: inner.brush_shape,
            blend_mode: inner.blend_mode,
        }
    }

    fn display(&self) -> PixelCanvasDisplaySettings {
        let inner = self.inner.borrow();
        PixelCanvasDisplaySettings {
            visible: inner.display_visible,
            opacity: inner.display_opacity,
            blend_mode: inner.display_blend_mode,
        }
    }

    fn paper(&self) -> PixelCanvasPaperSettings {
        let inner = self.inner.borrow();
        PixelCanvasPaperSettings {
            visible: inner.paper_visible,
            opacity: inner.paper_opacity,
        }
    }

    fn take_history_command(&self) -> Option<PixelCanvasHistoryCommand> {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_undo > 0 {
            inner.pending_undo -= 1;
            return Some(PixelCanvasHistoryCommand::Undo);
        }
        if inner.pending_redo > 0 {
            inner.pending_redo -= 1;
            return Some(PixelCanvasHistoryCommand::Redo);
        }
        None
    }

    fn take_viewport_command(&self) -> Option<PixelCanvasViewportCommand> {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_fit_view > 0 {
            inner.pending_fit_view -= 1;
            return Some(PixelCanvasViewportCommand::Fit);
        }
        if inner.pending_actual_size > 0 {
            inner.pending_actual_size -= 1;
            return Some(PixelCanvasViewportCommand::ActualSize);
        }
        if inner.pending_zoom_delta > 0 {
            inner.pending_zoom_delta -= 1;
            return Some(PixelCanvasViewportCommand::ZoomIn);
        }
        if inner.pending_zoom_delta < 0 {
            inner.pending_zoom_delta += 1;
            return Some(PixelCanvasViewportCommand::ZoomOut);
        }
        None
    }

    fn take_export_request(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_export == 0 {
            return false;
        }
        inner.pending_export = 0;
        true
    }

    fn take_clear_request(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_clear == 0 {
            return false;
        }
        inner.pending_clear = 0;
        true
    }

    fn publish_export_snapshot(&self, name: String, width: usize, height: usize, rgba8: Vec<u8>) {
        let mut inner = self.inner.borrow_mut();
        inner.export_revision = inner.export_revision.saturating_add(1);
        inner.latest_export = Some(PixelCanvasExportSnapshot::new(
            inner.export_revision,
            name,
            width,
            height,
            rgba8,
        ));
    }

    fn set_canvas_availability(&self, can_undo: bool, can_redo: bool, can_clear: bool) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.can_undo == can_undo && inner.can_redo == can_redo && inner.can_clear == can_clear
        {
            return false;
        }
        inner.can_undo = can_undo;
        inner.can_redo = can_redo;
        inner.can_clear = can_clear;
        true
    }

    fn set_viewport_state(&self, viewport: CanvasViewport, viewport_size: Size) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.viewport == viewport && inner.viewport_size == viewport_size {
            return false;
        }
        inner.viewport = viewport;
        inner.viewport_size = viewport_size;
        true
    }

    fn set_cursor_position(&self, cursor_position: Option<Point>) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.cursor_position == cursor_position {
            return false;
        }
        inner.cursor_position = cursor_position;
        true
    }
}

impl Default for PixelCanvasState {
    fn default() -> Self {
        let accent = DefaultTheme::default().palette.accent;
        Self {
            inner: Rc::new(RefCell::new(PixelCanvasStateInner {
                tool: PixelCanvasTool::Brush,
                brush: accent,
                brush_size: 1.0,
                brush_opacity: 1.0,
                brush_shape: PixelCanvasBrushShape::Square,
                blend_mode: PixelCanvasBlendMode::Normal,
                display_visible: PixelCanvasDisplaySettings::DEFAULT.visible,
                display_opacity: PixelCanvasDisplaySettings::DEFAULT.opacity,
                display_blend_mode: PixelCanvasDisplaySettings::DEFAULT.blend_mode,
                display_above_paper: true,
                paper_visible: PixelCanvasPaperSettings::DEFAULT.visible,
                paper_opacity: PixelCanvasPaperSettings::DEFAULT.opacity,
                pending_undo: 0,
                pending_redo: 0,
                pending_fit_view: 0,
                pending_actual_size: 0,
                pending_zoom_delta: 0,
                pending_export: 0,
                pending_clear: 0,
                export_revision: 0,
                latest_export: None,
                editable: true,
                can_undo: false,
                can_redo: false,
                can_clear: false,
                viewport: CanvasViewport::default(),
                viewport_size: Size::ZERO,
                cursor_position: None,
            })),
        }
    }
}

pub struct PixelCanvas {
    theme: DefaultTheme,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    name: String,
    width: usize,
    height: usize,
    pixels: Vec<PixelColor>,
    pixel_revision: u64,
    paint_image_cache: RefCell<Option<PixelCanvasImageCache>>,
    viewport: CanvasViewport,
    state: PixelCanvasState,
    drag: Option<PixelCanvasDrag>,
    active_edits: Vec<PixelEdit>,
    undo_stack: Vec<Vec<PixelEdit>>,
    redo_stack: Vec<Vec<PixelEdit>>,
    has_visible_pixels: bool,
    focus_animation: AnimatedScalar,
    desired_size: Size,
    fit_on_first_layout: bool,
    initial_fit_applied: bool,
}

impl PixelCanvas {
    pub fn new(name: impl Into<String>, width: usize, height: usize) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            theme: DefaultTheme::default(),
            theme_reader: None,
            name: name.into(),
            width,
            height,
            pixels: vec![PixelColor::TRANSPARENT; width * height],
            pixel_revision: 0,
            paint_image_cache: RefCell::new(None),
            viewport: CanvasViewport::new().zoom(14.0),
            state: PixelCanvasState::new(),
            drag: None,
            active_edits: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            has_visible_pixels: false,
            focus_animation: AnimatedScalar::new(0.0),
            desired_size: Size::new(520.0, 360.0),
            fit_on_first_layout: false,
            initial_fit_applied: false,
        }
    }

    pub fn from_fn<F>(name: impl Into<String>, width: usize, height: usize, mut pixel: F) -> Self
    where
        F: FnMut(usize, usize) -> Color,
    {
        let mut canvas = Self::new(name, width, height);
        for y in 0..canvas.height {
            for x in 0..canvas.width {
                let index = y * canvas.width + x;
                canvas.pixels[index] = PixelColor::from_color(pixel(x, y));
            }
        }
        canvas.has_visible_pixels = canvas.pixels.iter().any(|pixel| pixel.alpha > 0);
        canvas
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn desired_size(mut self, size: Size) -> Self {
        self.desired_size = Size::new(size.width.max(1.0), size.height.max(1.0));
        self
    }

    pub fn viewport(mut self, viewport: CanvasViewport) -> Self {
        self.viewport = viewport;
        self
    }

    pub fn fit_on_first_layout(mut self) -> Self {
        self.fit_on_first_layout = true;
        self.initial_fit_applied = false;
        self
    }

    pub fn state(mut self, state: PixelCanvasState) -> Self {
        self.state = state;
        self.publish_canvas_availability();
        self.state.set_viewport_state(self.viewport, Size::ZERO);
        self
    }

    pub fn brush_color(self, color: Color) -> Self {
        self.state.set_brush_color(color);
        self
    }

    pub fn brush_size(self, size: f32) -> Self {
        self.state.set_brush_size(size);
        self
    }

    pub fn with_pixels(mut self, pixels: Vec<Color>) -> Self {
        if pixels.len() == self.pixels.len() {
            self.pixels = pixels.into_iter().map(PixelColor::from_color).collect();
            self.mark_pixels_changed();
            self.has_visible_pixels = self.pixels.iter().any(|pixel| pixel.alpha > 0);
            self.publish_canvas_availability();
        }
        self
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) -> bool {
        let Some(index) = self.pixel_index(x, y) else {
            return false;
        };
        let next = PixelColor::from_color(color);
        if self.pixels[index] == next {
            return false;
        }
        self.pixels[index] = next;
        self.mark_pixels_changed();
        if next.alpha > 0 {
            self.has_visible_pixels = true;
        } else if self.has_visible_pixels {
            self.has_visible_pixels = self.pixels.iter().any(|pixel| pixel.alpha > 0);
        }
        self.publish_canvas_availability();
        true
    }

    pub fn pixel_at(&self, x: usize, y: usize) -> Option<Color> {
        self.pixel_index(x, y)
            .map(|index| self.pixels[index].to_color())
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    pub fn viewport_state(&self) -> CanvasViewport {
        self.viewport
    }

    fn pixel_index(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }

    fn document_origin(&self) -> Point {
        Point::new(self.width as f32 * 0.5, self.height as f32 * 0.5)
    }

    fn mark_pixels_changed(&mut self) {
        self.pixel_revision = self.pixel_revision.wrapping_add(1);
        if self.pixel_revision == 0 {
            self.paint_image_cache.borrow_mut().take();
        }
    }

    fn set_pixel_with_history(
        &mut self,
        x: usize,
        y: usize,
        color: Color,
        opacity: f32,
        blend_mode: PixelCanvasBlendMode,
        edits: &mut Vec<PixelEdit>,
    ) -> bool {
        let Some(index) = self.pixel_index(x, y) else {
            return false;
        };
        let before = self.pixels[index];
        let after = before.compose(color, opacity, blend_mode);
        if before == after {
            return false;
        }
        self.pixels[index] = after;
        edits.push(PixelEdit {
            index,
            before,
            after,
        });
        true
    }

    fn erase_pixel_with_history(
        &mut self,
        x: usize,
        y: usize,
        opacity: f32,
        edits: &mut Vec<PixelEdit>,
    ) -> bool {
        let Some(index) = self.pixel_index(x, y) else {
            return false;
        };
        let before = self.pixels[index];
        let after = before.erased(opacity);
        if before == after {
            return false;
        }
        self.pixels[index] = after;
        edits.push(PixelEdit {
            index,
            before,
            after,
        });
        true
    }

    fn paint_at_world(&mut self, world: Point, edits: &mut Vec<PixelEdit>) -> bool {
        let x = world.x.floor() as isize;
        let y = world.y.floor() as isize;
        let brush = self.state.brush();
        let color = match brush.tool {
            PixelCanvasTool::Brush => brush.brush,
            PixelCanvasTool::Eraser => PixelColor::TRANSPARENT.to_color(),
            PixelCanvasTool::Fill | PixelCanvasTool::Pan => return false,
        };
        let size = brush.brush_size.round().max(1.0) as isize;
        let half = size / 2;
        let start_x = x - half;
        let start_y = y - half;
        let mut painted = false;
        for py in start_y..start_y + size {
            for px in start_x..start_x + size {
                if px < 0 || py < 0 {
                    continue;
                }
                if !brush_shape_contains_pixel(brush.brush_shape, size, start_x, start_y, px, py) {
                    continue;
                }
                painted |= match brush.tool {
                    PixelCanvasTool::Brush => self.set_pixel_with_history(
                        px as usize,
                        py as usize,
                        color,
                        brush.brush_opacity,
                        brush.blend_mode,
                        edits,
                    ),
                    PixelCanvasTool::Eraser => self.erase_pixel_with_history(
                        px as usize,
                        py as usize,
                        brush.brush_opacity,
                        edits,
                    ),
                    PixelCanvasTool::Fill | PixelCanvasTool::Pan => false,
                };
            }
        }
        if painted {
            self.mark_pixels_changed();
        }
        painted
    }

    fn paint_at_position(
        &mut self,
        bounds: Rect,
        position: Point,
        edits: &mut Vec<PixelEdit>,
    ) -> bool {
        let world = self
            .viewport
            .screen_to_world(bounds, position, self.document_origin());
        self.paint_at_world(world, edits)
    }

    fn paint_between_positions(
        &mut self,
        bounds: Rect,
        start: Point,
        end: Point,
        edits: &mut Vec<PixelEdit>,
    ) -> bool {
        let document_origin = self.document_origin();
        let start = self
            .viewport
            .screen_to_world(bounds, start, document_origin);
        let end = self.viewport.screen_to_world(bounds, end, document_origin);
        let delta = end - start;
        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
        if distance <= f32::EPSILON {
            return false;
        }

        let spacing = brush_stroke_spacing(self.state.brush_size());
        let steps = (distance / spacing).ceil().max(1.0) as usize;
        let mut painted = false;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            painted |= self.paint_at_world(
                Point::new(start.x + (delta.x * t), start.y + (delta.y * t)),
                edits,
            );
        }
        painted
    }

    fn fill_at_position(
        &mut self,
        bounds: Rect,
        position: Point,
        edits: &mut Vec<PixelEdit>,
    ) -> bool {
        let world = self
            .viewport
            .screen_to_world(bounds, position, self.document_origin());
        let x = world.x.floor() as isize;
        let y = world.y.floor() as isize;
        if x < 0 || y < 0 {
            return false;
        }
        let Some(start) = self.pixel_index(x as usize, y as usize) else {
            return false;
        };
        let brush = self.state.brush();
        let target = self.pixels[start];
        let replacement = target.compose(brush.brush, brush.brush_opacity, brush.blend_mode);
        if target == replacement {
            return false;
        }

        let mut stack = vec![start];
        while let Some(index) = stack.pop() {
            if self.pixels[index] != target {
                continue;
            }
            self.pixels[index] = replacement;
            edits.push(PixelEdit {
                index,
                before: target,
                after: replacement,
            });
            let px = index % self.width;
            let py = index / self.width;
            if px > 0 {
                stack.push(index - 1);
            }
            if px + 1 < self.width {
                stack.push(index + 1);
            }
            if py > 0 {
                stack.push(index - self.width);
            }
            if py + 1 < self.height {
                stack.push(index + self.width);
            }
        }
        self.mark_pixels_changed();
        true
    }

    fn push_history(&mut self, edits: Vec<PixelEdit>) {
        if edits.is_empty() {
            return;
        }
        self.undo_stack.push(edits);
        if self.undo_stack.len() > PIXEL_CANVAS_HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        self.has_visible_pixels = self.pixels.iter().any(|pixel| pixel.alpha > 0);
        self.publish_canvas_availability();
    }

    fn undo(&mut self) -> bool {
        if !self.state.is_editable() {
            self.publish_canvas_availability();
            return false;
        }
        let Some(edits) = self.undo_stack.pop() else {
            self.publish_canvas_availability();
            return false;
        };
        for edit in edits.iter().rev() {
            self.pixels[edit.index] = edit.before;
        }
        self.mark_pixels_changed();
        self.redo_stack.push(edits);
        self.has_visible_pixels = self.pixels.iter().any(|pixel| pixel.alpha > 0);
        self.publish_canvas_availability();
        true
    }

    fn redo(&mut self) -> bool {
        if !self.state.is_editable() {
            self.publish_canvas_availability();
            return false;
        }
        let Some(edits) = self.redo_stack.pop() else {
            self.publish_canvas_availability();
            return false;
        };
        for edit in &edits {
            self.pixels[edit.index] = edit.after;
        }
        self.mark_pixels_changed();
        self.undo_stack.push(edits);
        self.has_visible_pixels = self.pixels.iter().any(|pixel| pixel.alpha > 0);
        self.publish_canvas_availability();
        true
    }

    fn publish_canvas_availability(&self) -> bool {
        let editable = self.state.is_editable();
        self.state.set_canvas_availability(
            editable && !self.undo_stack.is_empty(),
            editable && !self.redo_stack.is_empty(),
            editable && self.has_visible_pixels,
        )
    }

    fn publish_viewport_state(&self, bounds: Rect) -> bool {
        self.state.set_viewport_state(self.viewport, bounds.size)
    }

    fn cursor_position_for_pointer(&self, bounds: Rect, position: Point) -> Option<Point> {
        if !bounds.contains(position) {
            return None;
        }
        let world = self
            .viewport
            .screen_to_world(bounds, position, self.document_origin());
        let x = world.x.floor();
        let y = world.y.floor();
        (x >= 0.0 && y >= 0.0 && x < self.width as f32 && y < self.height as f32)
            .then_some(Point::new(x, y))
    }

    fn publish_cursor_position(&self, bounds: Rect, position: Point) -> bool {
        self.state
            .set_cursor_position(self.cursor_position_for_pointer(bounds, position))
    }

    fn clear_cursor_position(&self) -> bool {
        self.state.set_cursor_position(None)
    }

    fn clear_pixels(&mut self) -> bool {
        if !self.state.is_editable() {
            self.publish_canvas_availability();
            return false;
        }
        let mut edits = Vec::new();
        for (index, pixel) in self.pixels.iter_mut().enumerate() {
            if *pixel == PixelColor::TRANSPARENT {
                continue;
            }
            let before = *pixel;
            *pixel = PixelColor::TRANSPARENT;
            edits.push(PixelEdit {
                index,
                before,
                after: PixelColor::TRANSPARENT,
            });
        }
        if edits.is_empty() {
            self.publish_canvas_availability();
            return false;
        }
        self.has_visible_pixels = false;
        self.mark_pixels_changed();
        self.push_history(edits);
        true
    }

    fn apply_pending_history_commands(&mut self) -> bool {
        let mut changed = false;
        while let Some(command) = self.state.take_history_command() {
            changed |= match command {
                PixelCanvasHistoryCommand::Undo => self.undo(),
                PixelCanvasHistoryCommand::Redo => self.redo(),
            };
        }
        changed
    }

    fn apply_pending_clear_requests(&mut self) -> bool {
        if self.state.take_clear_request() {
            self.clear_pixels()
        } else {
            false
        }
    }

    fn fit_view_to_bounds(&mut self, bounds: Rect) -> bool {
        if bounds.is_empty() {
            return false;
        }
        let padding = self.resolved_theme().metrics.pixel_canvas_fit_padding;
        let (sin, cos) = self.viewport.rotation.sin_cos();
        let rotated_width = (self.width as f32 * cos.abs()) + (self.height as f32 * sin.abs());
        let rotated_height = (self.width as f32 * sin.abs()) + (self.height as f32 * cos.abs());
        let available_width = (bounds.width() - (padding * 2.0)).max(1.0);
        let available_height = (bounds.height() - (padding * 2.0)).max(1.0);
        let zoom = (available_width / rotated_width.max(1.0))
            .min(available_height / rotated_height.max(1.0))
            .max(0.01);
        let next = CanvasViewport {
            pan: Vector::ZERO,
            zoom,
            rotation: self.viewport.rotation,
        };
        if self.viewport == next {
            return false;
        }
        self.viewport = next;
        true
    }

    fn set_actual_size_view(&mut self) -> bool {
        let next = CanvasViewport {
            pan: Vector::ZERO,
            zoom: 1.0,
            rotation: self.viewport.rotation,
        };
        if self.viewport == next {
            return false;
        }
        self.viewport = next;
        true
    }

    fn zoom_view_around_center(&mut self, bounds: Rect, factor: f32) -> bool {
        if bounds.is_empty() {
            return false;
        }
        let previous = self.viewport;
        self.viewport.zoom_around(
            bounds,
            CanvasViewport::center(bounds),
            factor,
            self.document_origin(),
        );
        self.viewport != previous
    }

    fn apply_pending_viewport_commands(&mut self, bounds: Rect) -> bool {
        let mut changed = false;
        let zoom_step = self.resolved_theme().metrics.pixel_canvas_zoom_step;
        while let Some(command) = self.state.take_viewport_command() {
            changed |= match command {
                PixelCanvasViewportCommand::Fit => self.fit_view_to_bounds(bounds),
                PixelCanvasViewportCommand::ActualSize => self.set_actual_size_view(),
                PixelCanvasViewportCommand::ZoomIn => {
                    self.zoom_view_around_center(bounds, zoom_step)
                }
                PixelCanvasViewportCommand::ZoomOut => {
                    self.zoom_view_around_center(bounds, 1.0 / zoom_step)
                }
            };
        }
        changed
    }

    fn apply_initial_fit(&mut self, bounds: Rect) -> bool {
        if !self.fit_on_first_layout || self.initial_fit_applied || bounds.is_empty() {
            return false;
        }
        self.initial_fit_applied = true;
        self.fit_view_to_bounds(bounds)
    }

    fn apply_pending_export_requests(&self) -> bool {
        if !self.state.take_export_request() {
            return false;
        }

        self.state.publish_export_snapshot(
            self.name.clone(),
            self.width,
            self.height,
            self.export_image_data(),
        );
        true
    }

    fn export_image_data(&self) -> Vec<u8> {
        self.paint_image().bytes().to_vec()
    }

    fn paint_image(&self) -> RegisteredImage {
        let display = self.state.display();
        let paper = self.state.paper();
        let display_above_paper = self.state.display_above_paper();
        let paper_color = self.resolved_theme().surfaces.pixel_canvas_paper;
        self.paint_image_for_source(Self::image_source(
            display,
            paper,
            display_above_paper,
            paper_color,
        ))
    }

    fn raw_paint_image(&self) -> RegisteredImage {
        self.paint_image_for_source(PixelCanvasImageSource::Raw)
    }

    fn image_source(
        display: PixelCanvasDisplaySettings,
        paper: PixelCanvasPaperSettings,
        display_above_paper: bool,
        paper_color: Color,
    ) -> PixelCanvasImageSource {
        if display.requires_compositing() || paper.requires_compositing() || !display_above_paper {
            PixelCanvasImageSource::Composited {
                display,
                paper,
                display_above_paper,
                paper_color,
            }
        } else {
            PixelCanvasImageSource::Raw
        }
    }

    fn can_paint_with_image_opacity(
        display: PixelCanvasDisplaySettings,
        paper: PixelCanvasPaperSettings,
        display_above_paper: bool,
    ) -> bool {
        display_above_paper
            && display.visible
            && display.blend_mode == PixelCanvasBlendMode::Normal
            && paper.visible
            && paper.opacity >= 0.999
    }

    fn paint_image_for_source(&self, source: PixelCanvasImageSource) -> RegisteredImage {
        let key = PixelCanvasImageCacheKey {
            pixel_revision: self.pixel_revision,
            source,
        };

        if let Some(cache) = self.paint_image_cache.borrow().as_ref()
            && cache.key == key
        {
            return cache.image.clone();
        }

        let data = match source {
            PixelCanvasImageSource::Raw => self.image_data(),
            PixelCanvasImageSource::Composited {
                display,
                paper,
                display_above_paper,
                paper_color,
            } => self.display_image_data(display, paper, display_above_paper, paper_color),
        };
        let image = RegisteredImage::from_rgba8(self.width as u32, self.height as u32, data)
            .expect("pixel canvas image data should match its dimensions");
        *self.paint_image_cache.borrow_mut() = Some(PixelCanvasImageCache {
            key,
            image: image.clone(),
        });
        image
    }

    fn image_data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in &self.pixels {
            data.extend_from_slice(&[pixel.red, pixel.green, pixel.blue, pixel.alpha]);
        }
        data
    }

    fn display_image_data(
        &self,
        display: PixelCanvasDisplaySettings,
        paper: PixelCanvasPaperSettings,
        display_above_paper: bool,
        paper_color: Color,
    ) -> Vec<u8> {
        let paper = paper.pixel(paper_color);
        let mut data = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in &self.pixels {
            let mut output = PixelColor::TRANSPARENT;
            if display_above_paper {
                output = output.compose(paper.to_color(), 1.0, PixelCanvasBlendMode::Normal);
                if display.visible {
                    output = output.compose(pixel.to_color(), display.opacity, display.blend_mode);
                }
            } else {
                if display.visible {
                    output = output.compose(pixel.to_color(), display.opacity, display.blend_mode);
                }
                output = output.compose(paper.to_color(), 1.0, PixelCanvasBlendMode::Normal);
            }
            data.extend_from_slice(&[output.red, output.green, output.blue, output.alpha]);
        }
        data
    }

    fn request_interaction_update(ctx: &mut EventCtx) {
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(self.theme)
    }
}

impl Widget for PixelCanvas {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let history_changed = self.apply_pending_history_commands();
        let viewport_changed = self.apply_pending_viewport_commands(ctx.bounds());
        let clear_changed = self.apply_pending_clear_requests();
        let export_changed = self.apply_pending_export_requests();
        let availability_changed = self.publish_canvas_availability();
        if viewport_changed {
            self.publish_viewport_state(ctx.bounds());
        }
        if history_changed
            || viewport_changed
            || clear_changed
            || export_changed
            || availability_changed
        {
            Self::request_interaction_update(ctx);
        }
        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Move
                        | PointerEventKind::Down
                        | PointerEventKind::Scroll
                        | PointerEventKind::Up
                ) =>
            {
                if self.publish_cursor_position(ctx.bounds(), pointer.position) {
                    Self::request_interaction_update(ctx);
                }
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Leave | PointerEventKind::Cancel
                ) =>
            {
                if self.clear_cursor_position() {
                    Self::request_interaction_update(ctx);
                }
            }
            _ => {}
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = scroll_delta_to_offset(pointer.scroll_delta, pointer.delta);
                if pointer.modifiers.shift {
                    self.viewport.rotate_around(
                        ctx.bounds(),
                        pointer.position,
                        delta.y * 0.01,
                        self.document_origin(),
                    );
                } else {
                    self.viewport.zoom_around(
                        ctx.bounds(),
                        pointer.position,
                        (delta.y * 0.002).exp(),
                        self.document_origin(),
                    );
                }
                self.publish_viewport_state(ctx.bounds());
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position)
                    && matches!(
                        pointer.button,
                        Some(PointerButton::Middle | PointerButton::Secondary)
                    ) =>
            {
                self.drag = Some(PixelCanvasDrag::Pan {
                    pointer_id: pointer.pointer_id,
                    last_position: pointer.position,
                });
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                let editable = self.state.is_editable();
                match self.state.tool() {
                    PixelCanvasTool::Brush | PixelCanvasTool::Eraser if editable => {
                        let mut edits = Vec::new();
                        self.paint_at_position(ctx.bounds(), pointer.position, &mut edits);
                        self.active_edits = edits;
                        self.drag = Some(PixelCanvasDrag::Paint {
                            pointer_id: pointer.pointer_id,
                            last_position: pointer.position,
                        });
                    }
                    PixelCanvasTool::Fill if editable => {
                        let mut edits = Vec::new();
                        self.fill_at_position(ctx.bounds(), pointer.position, &mut edits);
                        self.push_history(edits);
                    }
                    PixelCanvasTool::Brush | PixelCanvasTool::Eraser | PixelCanvasTool::Fill => {}
                    PixelCanvasTool::Pan => {
                        self.drag = Some(PixelCanvasDrag::Pan {
                            pointer_id: pointer.pointer_id,
                            last_position: pointer.position,
                        });
                    }
                }
                ctx.request_focus();
                if self.drag.is_some() {
                    ctx.request_pointer_capture(pointer.pointer_id);
                }
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => match self.drag {
                Some(PixelCanvasDrag::Pan {
                    pointer_id,
                    mut last_position,
                }) if pointer_id == pointer.pointer_id => {
                    let delta = pointer.position - last_position;
                    self.viewport.pan += delta;
                    last_position = pointer.position;
                    self.drag = Some(PixelCanvasDrag::Pan {
                        pointer_id,
                        last_position,
                    });
                    self.publish_viewport_state(ctx.bounds());
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
                Some(PixelCanvasDrag::Paint {
                    pointer_id,
                    last_position,
                }) if pointer_id == pointer.pointer_id && self.state.is_editable() => {
                    let mut edits = std::mem::take(&mut self.active_edits);
                    self.paint_between_positions(
                        ctx.bounds(),
                        last_position,
                        pointer.position,
                        &mut edits,
                    );
                    self.active_edits = edits;
                    self.drag = Some(PixelCanvasDrag::Paint {
                        pointer_id,
                        last_position: pointer.position,
                    });
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
                _ => {}
            },
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    || pointer.kind == PointerEventKind::Cancel =>
            {
                let active_pointer = match self.drag {
                    Some(
                        PixelCanvasDrag::Pan { pointer_id, .. }
                        | PixelCanvasDrag::Paint { pointer_id, .. },
                    ) => Some(pointer_id),
                    None => None,
                };
                if active_pointer == Some(pointer.pointer_id) {
                    if let Some(PixelCanvasDrag::Paint { last_position, .. }) = self.drag {
                        let mut edits = std::mem::take(&mut self.active_edits);
                        if pointer.kind == PointerEventKind::Up && self.state.is_editable() {
                            self.paint_between_positions(
                                ctx.bounds(),
                                last_position,
                                pointer.position,
                                &mut edits,
                            );
                        }
                        self.push_history(edits);
                    }
                    self.drag = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let command_modifier = key.modifiers.control || key.modifiers.meta;
                if command_modifier && matches!(key.key.as_str(), "z" | "Z") {
                    let changed = if key.modifiers.shift {
                        self.redo()
                    } else {
                        self.undo()
                    };
                    if changed {
                        Self::request_interaction_update(ctx);
                    }
                    ctx.set_handled();
                    return;
                }
                if command_modifier && matches!(key.key.as_str(), "y" | "Y") {
                    if self.redo() {
                        Self::request_interaction_update(ctx);
                    }
                    ctx.set_handled();
                    return;
                }
                let zoom_step = self.resolved_theme().metrics.pixel_canvas_zoom_step;
                match key.key.as_str() {
                    "=" | "+" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        zoom_step,
                        self.document_origin(),
                    ),
                    "-" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        1.0 / zoom_step,
                        self.document_origin(),
                    ),
                    "[" => self.viewport.rotate_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        -0.1,
                        self.document_origin(),
                    ),
                    "]" => self.viewport.rotate_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        0.1,
                        self.document_origin(),
                    ),
                    "ArrowLeft" => self.viewport.pan.x += 24.0,
                    "ArrowRight" => self.viewport.pan.x -= 24.0,
                    "ArrowUp" => self.viewport.pan.y += 24.0,
                    "ArrowDown" => self.viewport.pan.y -= 24.0,
                    _ => return,
                }
                self.publish_viewport_state(ctx.bounds());
                Self::request_interaction_update(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.focus_animation.advance(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.apply_pending_history_commands() {
            ctx.request_paint();
            ctx.request_semantics();
        }
        if self.apply_pending_clear_requests() {
            ctx.request_paint();
            ctx.request_semantics();
        }
        if self.apply_pending_export_requests() {
            ctx.request_semantics();
        }
        if self.publish_canvas_availability() {
            ctx.request_semantics();
        }

        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                self.desired_size.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                self.desired_size.height
            },
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let viewport_changed =
            self.apply_initial_fit(bounds) || self.apply_pending_viewport_commands(bounds);
        let state_changed = self.publish_viewport_state(bounds);
        if viewport_changed || state_changed {
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        ctx.fill_bounds(theme.surfaces.canvas);
        ctx.stroke_bounds(
            theme.surfaces.border,
            StrokeStyle::new(theme.metrics.border_width),
        );
        ctx.push_clip_rect(ctx.bounds());
        let transform = self
            .viewport
            .transform(ctx.bounds(), self.document_origin());
        let image_bounds = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);
        paint_pixel_canvas_document_shadow(ctx, image_bounds, transform, &theme);
        let display = self.state.display();
        let paper = self.state.paper();
        let display_above_paper = self.state.display_above_paper();
        let image_handle = ctx.widget_image_handle(0);
        let sampling = if self.viewport.zoom >= theme.metrics.pixel_canvas_nearest_sampling_zoom {
            ImageSampling::Nearest
        } else {
            ImageSampling::Linear
        };
        if Self::can_paint_with_image_opacity(display, paper, display_above_paper) {
            fill_transformed_rect(
                ctx,
                image_bounds,
                transform,
                theme.surfaces.pixel_canvas_paper,
            );
            ctx.register_image(image_handle, self.raw_paint_image());
            ctx.draw_image_quad_source(
                transformed_rect_points(image_bounds, transform),
                ImageSource::new(image_handle)
                    .with_sampling(sampling)
                    .with_tint(Color::WHITE.with_alpha(display.opacity)),
            );
        } else {
            let baked_image = display.requires_compositing()
                || paper.requires_compositing()
                || !display_above_paper;
            if !baked_image {
                fill_transformed_rect(
                    ctx,
                    image_bounds,
                    transform,
                    theme.surfaces.pixel_canvas_paper,
                );
            }
            ctx.register_image(image_handle, self.paint_image());
            ctx.draw_image_quad_source(
                transformed_rect_points(image_bounds, transform),
                ImageSource::new(image_handle).with_sampling(sampling),
            );
        }
        if self.viewport.zoom >= theme.metrics.pixel_canvas_grid_zoom
            && let Some(range) = pixel_visible_range(
                self.viewport,
                ctx.bounds(),
                self.document_origin(),
                self.width,
                self.height,
            )
        {
            paint_pixel_grid(ctx, range, transform, theme.surfaces.pixel_canvas_grid);
        }
        ctx.stroke(
            transformed_rect_path(image_bounds, transform),
            theme.surfaces.pixel_canvas_document_edge,
            StrokeStyle::new(1.0),
        );
        ctx.pop_clip();
        draw_focus_ring(ctx, ctx.bounds(), &theme, self.focus_animation.value);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some(format!("{} by {} pixel canvas", self.width, self.height));
        let tool = self.state.tool();
        let brush_size = self.state.brush_size();
        let brush_opacity = self.state.brush_opacity();
        let brush_shape = self.state.brush_shape();
        let blend_mode = self.state.blend_mode();
        let editable = self.state.is_editable();
        let display = self.state.display();
        let paper = self.state.paper();
        node.value = Some(SemanticsValue::Text(format!(
            "tool {}, zoom {:.0}%, rotation {:.0} deg, brush {:.0} px, shape {}, opacity {:.0}%, blend {}, paint layer {}, paint opacity {:.0}%, paint blend {}, paper layer {}, paper opacity {:.0}%, {}",
            tool.label(),
            self.viewport.zoom * 100.0,
            self.viewport.rotation.to_degrees(),
            brush_size,
            brush_shape.label(),
            brush_opacity * 100.0,
            blend_mode.label(),
            if display.visible { "visible" } else { "hidden" },
            display.opacity * 100.0,
            display.blend_mode.label(),
            if paper.visible { "visible" } else { "hidden" },
            paper.opacity * 100.0,
            if editable { "editable" } else { "read only" }
        )));
        node.state.focused = ctx.is_focused();
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Custom("Pan".into()),
        ];
        if editable {
            node.actions.push(SemanticsAction::Custom("Paint".into()));
        }
        if editable && self.state.can_undo() {
            node.actions.push(SemanticsAction::Undo);
        }
        if editable && self.state.can_redo() {
            node.actions.push(SemanticsAction::Redo);
        }
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        Self::request_interaction_update(ctx);
    }
}

fn fill_transformed_rect(ctx: &mut PaintCtx, rect: Rect, transform: Transform, color: Color) {
    if transform.yx.abs() < AXIS_ALIGNED_EPSILON && transform.xy.abs() < AXIS_ALIGNED_EPSILON {
        ctx.fill_rect(transform.transform_rect_bbox(rect), color);
    } else {
        ctx.fill(transformed_rect_path(rect, transform), color);
    }
}

fn paint_pixel_canvas_document_shadow(
    ctx: &mut PaintCtx,
    rect: Rect,
    transform: Transform,
    theme: &DefaultTheme,
) {
    fill_transformed_rect(
        ctx,
        rect,
        transform.then(Transform::translation(0.0, 7.0)),
        theme.surfaces.pixel_canvas_shadow_far,
    );
    fill_transformed_rect(
        ctx,
        rect,
        transform.then(Transform::translation(0.0, 3.0)),
        theme.surfaces.pixel_canvas_shadow_near,
    );
}

fn paint_canvas_ruler_divider(
    ctx: &mut PaintCtx,
    bounds: Rect,
    axis: CanvasRulerAxis,
    color: Color,
) {
    match axis {
        CanvasRulerAxis::Horizontal => {
            stroke_line(
                ctx,
                Point::new(bounds.x(), bounds.max_y()),
                Point::new(bounds.max_x(), bounds.max_y()),
                color,
                1.0,
            );
        }
        CanvasRulerAxis::Vertical => {
            stroke_line(
                ctx,
                Point::new(bounds.max_x(), bounds.y()),
                Point::new(bounds.max_x(), bounds.max_y()),
                color,
                1.0,
            );
        }
    }
}

fn paint_canvas_ruler_ticks(
    ctx: &mut PaintCtx,
    bounds: Rect,
    axis: CanvasRulerAxis,
    document_size: Size,
    viewport: CanvasViewport,
    viewport_size: Size,
    theme: &DefaultTheme,
    text_style: TextStyle,
) {
    let canvas_bounds = ruler_canvas_bounds(bounds, axis, viewport_size);
    let document_origin = Point::new(document_size.width * 0.5, document_size.height * 0.5);
    let visible = ruler_visible_range(bounds, axis, document_size, viewport, canvas_bounds);
    let major_step = ruler_major_step(
        viewport.zoom,
        theme.metrics.canvas_ruler_target_major_spacing,
    );
    let minor_step = (major_step / 5.0).max(1.0);
    let start = (visible.0 / minor_step).floor() * minor_step;
    let end = visible.1 + minor_step;
    let mut value = start;
    let mut count = 0;

    while value <= end && count < CANVAS_RULER_MAX_TICKS {
        if value >= 0.0 && value <= ruler_document_length(axis, document_size) {
            let major = is_major_ruler_tick(value, major_step);
            let position = ruler_tick_screen_position(
                axis,
                value,
                document_size,
                viewport,
                canvas_bounds,
                document_origin,
            );
            if ruler_position_in_bounds(position, bounds, axis) {
                paint_canvas_ruler_tick(
                    ctx,
                    bounds,
                    axis,
                    position,
                    major,
                    theme.surfaces.canvas_ruler_tick,
                    theme.metrics.canvas_ruler_major_tick,
                    theme.metrics.canvas_ruler_minor_tick,
                );
                if major {
                    paint_canvas_ruler_label(
                        ctx,
                        bounds,
                        axis,
                        position,
                        value,
                        text_style.clone(),
                        theme.metrics.canvas_ruler_label_padding,
                        theme.metrics.canvas_ruler_label_max_width,
                    );
                }
            }
        }

        value += minor_step;
        count += 1;
    }
}

fn ruler_canvas_bounds(bounds: Rect, axis: CanvasRulerAxis, viewport_size: Size) -> Rect {
    match axis {
        CanvasRulerAxis::Horizontal => Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            viewport_size.height.max(bounds.height()),
        ),
        CanvasRulerAxis::Vertical => Rect::new(
            bounds.x(),
            bounds.y(),
            viewport_size.width.max(bounds.width()),
            bounds.height(),
        ),
    }
}

fn ruler_visible_range(
    bounds: Rect,
    axis: CanvasRulerAxis,
    document_size: Size,
    viewport: CanvasViewport,
    canvas_bounds: Rect,
) -> (f32, f32) {
    let document_origin = Point::new(document_size.width * 0.5, document_size.height * 0.5);
    let center = CanvasViewport::center(canvas_bounds);
    let (start, end) = match axis {
        CanvasRulerAxis::Horizontal => (
            viewport
                .screen_to_world(
                    canvas_bounds,
                    Point::new(bounds.x(), center.y),
                    document_origin,
                )
                .x,
            viewport
                .screen_to_world(
                    canvas_bounds,
                    Point::new(bounds.max_x(), center.y),
                    document_origin,
                )
                .x,
        ),
        CanvasRulerAxis::Vertical => (
            viewport
                .screen_to_world(
                    canvas_bounds,
                    Point::new(center.x, bounds.y()),
                    document_origin,
                )
                .y,
            viewport
                .screen_to_world(
                    canvas_bounds,
                    Point::new(center.x, bounds.max_y()),
                    document_origin,
                )
                .y,
        ),
    };

    (start.min(end), start.max(end))
}

fn ruler_major_step(zoom: f32, target_major_spacing: f32) -> f32 {
    let target_world = (target_major_spacing.max(1.0) / zoom.max(0.01)).max(1.0);
    let magnitude = 10.0_f32.powf(target_world.log10().floor());
    for multiplier in [1.0, 2.0, 5.0, 10.0] {
        let step = multiplier * magnitude;
        if step >= target_world {
            return step;
        }
    }
    10.0 * magnitude
}

fn ruler_document_length(axis: CanvasRulerAxis, document_size: Size) -> f32 {
    match axis {
        CanvasRulerAxis::Horizontal => document_size.width,
        CanvasRulerAxis::Vertical => document_size.height,
    }
}

fn is_major_ruler_tick(value: f32, major_step: f32) -> bool {
    let nearest = (value / major_step).round() * major_step;
    (value - nearest).abs() < 0.01
}

fn ruler_tick_screen_position(
    axis: CanvasRulerAxis,
    value: f32,
    document_size: Size,
    viewport: CanvasViewport,
    canvas_bounds: Rect,
    document_origin: Point,
) -> f32 {
    match axis {
        CanvasRulerAxis::Horizontal => {
            viewport
                .world_to_screen(
                    canvas_bounds,
                    Point::new(value, document_size.height * 0.5),
                    document_origin,
                )
                .x
        }
        CanvasRulerAxis::Vertical => {
            viewport
                .world_to_screen(
                    canvas_bounds,
                    Point::new(document_size.width * 0.5, value),
                    document_origin,
                )
                .y
        }
    }
}

fn ruler_position_in_bounds(position: f32, bounds: Rect, axis: CanvasRulerAxis) -> bool {
    match axis {
        CanvasRulerAxis::Horizontal => position >= bounds.x() && position <= bounds.max_x(),
        CanvasRulerAxis::Vertical => position >= bounds.y() && position <= bounds.max_y(),
    }
}

fn paint_canvas_ruler_tick(
    ctx: &mut PaintCtx,
    bounds: Rect,
    axis: CanvasRulerAxis,
    position: f32,
    major: bool,
    color: Color,
    major_length: f32,
    minor_length: f32,
) {
    let length = if major { major_length } else { minor_length };
    match axis {
        CanvasRulerAxis::Horizontal => stroke_line(
            ctx,
            Point::new(position, bounds.max_y()),
            Point::new(position, bounds.max_y() - length),
            color,
            1.0,
        ),
        CanvasRulerAxis::Vertical => stroke_line(
            ctx,
            Point::new(bounds.max_x(), position),
            Point::new(bounds.max_x() - length, position),
            color,
            1.0,
        ),
    }
}

fn paint_canvas_ruler_label(
    ctx: &mut PaintCtx,
    bounds: Rect,
    axis: CanvasRulerAxis,
    position: f32,
    value: f32,
    mut style: TextStyle,
    padding: sui_layout::Padding,
    max_width: f32,
) {
    style.features.enable(FontFeature::TABULAR_FIGURES);

    let label = format!("{value:.0}");
    let measurement = paint_text_measurement(ctx, &label, &style);
    let available_height = (bounds.height() - padding.top - padding.bottom).max(0.0);
    let slot = match axis {
        CanvasRulerAxis::Horizontal => Rect::new(
            position + padding.left,
            bounds.y() + padding.top,
            max_width.min((bounds.max_x() - position - padding.left).max(0.0)),
            available_height,
        ),
        CanvasRulerAxis::Vertical => {
            let height = style
                .line_height
                .max(measurement.height)
                .min(available_height);
            let min_y = bounds.y() + padding.top;
            let max_y = (bounds.max_y() - padding.bottom - height).max(min_y);
            Rect::new(
                bounds.x() + padding.left,
                (position - (height * 0.5)).clamp(min_y, max_y),
                max_width.min((bounds.width() - padding.left - padding.right).max(0.0)),
                height,
            )
        }
    };

    if slot.width() <= 0.0 || slot.height() <= 0.0 {
        return;
    }

    if measurement.width <= slot.width() {
        let width = measurement.width.min(slot.width()).max(0.0);
        paint_aligned_text(
            ctx,
            Rect::new(slot.x(), slot.y(), width, slot.height()),
            &label,
            &style,
            style.line_height,
            0.0,
        );
    }
}

fn paint_text_measurement(ctx: &PaintCtx, text: &str, style: &TextStyle) -> TextMeasurement {
    ctx.measure_text(text.to_string(), style.clone())
        .unwrap_or(TextMeasurement {
            width: text.chars().count() as f32 * style.font_size * 0.58,
            height: style.line_height,
            bounds: Rect::new(
                0.0,
                0.0,
                text.chars().count() as f32 * style.font_size * 0.58,
                style.line_height,
            ),
            ascent: style.font_size * 0.8,
            descent: style.font_size * 0.2,
            cap_height: Some(style.font_size),
        })
}

fn stroke_line(ctx: &mut PaintCtx, from: Point, to: Point, color: Color, width: f32) {
    let mut path = PathBuilder::new();
    path.move_to(from).line_to(to);
    ctx.stroke(path.build(), color, StrokeStyle::new(width));
}

fn scroll_delta_to_offset(scroll_delta: Option<ScrollDelta>, fallback: Vector) -> Vector {
    match scroll_delta {
        Some(ScrollDelta::Pixels(delta)) => delta,
        Some(ScrollDelta::Lines(delta)) => Vector::new(delta.x * 24.0, delta.y * 24.0),
        None => fallback,
    }
}

fn vector_length(vector: Vector) -> f32 {
    ((vector.x * vector.x) + (vector.y * vector.y)).sqrt()
}

fn path_from_points(points: &[Point]) -> Option<Path> {
    let first = points.first().copied()?;
    let mut builder = PathBuilder::new();
    builder.move_to(first);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    Some(builder.build())
}

fn paint_canvas_shape(
    ctx: &mut PaintCtx,
    shape: &CanvasShape,
    transform: Transform,
    stroke_scale: f32,
) {
    match shape {
        CanvasShape::Path { path, fill, stroke } => {
            let path = transform_path(path, transform);
            if let Some(fill) = fill {
                ctx.fill(path.clone(), *fill);
            }
            if let Some(stroke) = stroke {
                ctx.stroke(
                    path,
                    stroke.color,
                    StrokeStyle::new((stroke.width * stroke_scale).max(0.5)),
                );
            }
        }
    }
}

fn paint_canvas_axes(
    ctx: &mut PaintCtx,
    viewport: CanvasViewport,
    bounds: Rect,
    document_origin: Point,
    theme: &DefaultTheme,
) {
    let overscan = theme.metrics.canvas_axis_overscan;
    let visible =
        canvas_visible_world_rect(viewport, bounds, document_origin).inflate(overscan, overscan);
    let transform = viewport.transform(bounds, document_origin);
    let mut x_axis = PathBuilder::new();
    x_axis
        .move_to(transform.transform_point(Point::new(visible.x(), 0.0)))
        .line_to(transform.transform_point(Point::new(visible.max_x(), 0.0)));
    ctx.stroke(
        x_axis.build(),
        theme.surfaces.canvas_axis_x,
        StrokeStyle::new(1.0),
    );

    let mut y_axis = PathBuilder::new();
    y_axis
        .move_to(transform.transform_point(Point::new(0.0, visible.y())))
        .line_to(transform.transform_point(Point::new(0.0, visible.max_y())));
    ctx.stroke(
        y_axis.build(),
        theme.surfaces.canvas_axis_y,
        StrokeStyle::new(1.0),
    );
}

fn paint_canvas_grid(
    ctx: &mut PaintCtx,
    viewport: CanvasViewport,
    bounds: Rect,
    document_origin: Point,
    theme: &DefaultTheme,
) {
    let overscan = theme.metrics.canvas_axis_overscan;
    let visible =
        canvas_visible_world_rect(viewport, bounds, document_origin).inflate(overscan, overscan);
    let transform = viewport.transform(bounds, document_origin);
    let step = theme.metrics.canvas_grid_step.max(1.0);
    let min_x = (visible.x() / step).floor() as i32 - 1;
    let max_x = (visible.max_x() / step).ceil() as i32 + 1;
    let min_y = (visible.y() / step).floor() as i32 - 1;
    let max_y = (visible.max_y() / step).ceil() as i32 + 1;
    let mut builder = PathBuilder::new();
    for x in min_x..=max_x {
        let x = x as f32 * step;
        builder
            .move_to(transform.transform_point(Point::new(x, visible.y())))
            .line_to(transform.transform_point(Point::new(x, visible.max_y())));
    }
    for y in min_y..=max_y {
        let y = y as f32 * step;
        builder
            .move_to(transform.transform_point(Point::new(visible.x(), y)))
            .line_to(transform.transform_point(Point::new(visible.max_x(), y)));
    }
    ctx.stroke(
        builder.build(),
        theme.surfaces.canvas_grid,
        StrokeStyle::new(1.0),
    );
}

fn canvas_visible_world_rect(
    viewport: CanvasViewport,
    bounds: Rect,
    document_origin: Point,
) -> Rect {
    let points = [
        viewport.screen_to_world(bounds, bounds.origin, document_origin),
        viewport.screen_to_world(
            bounds,
            Point::new(bounds.max_x(), bounds.y()),
            document_origin,
        ),
        viewport.screen_to_world(
            bounds,
            Point::new(bounds.x(), bounds.max_y()),
            document_origin,
        ),
        viewport.screen_to_world(
            bounds,
            Point::new(bounds.max_x(), bounds.max_y()),
            document_origin,
        ),
    ];
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Rect::from_points(Point::new(min_x, min_y), Point::new(max_x, max_y))
}

fn transform_path(path: &Path, transform: Transform) -> Path {
    let mut builder = PathBuilder::new();
    for element in path.elements() {
        match *element {
            PathElement::MoveTo(point) => {
                builder.move_to(transform.transform_point(point));
            }
            PathElement::LineTo(point) => {
                builder.line_to(transform.transform_point(point));
            }
            PathElement::QuadTo { ctrl, to } => {
                builder.quad_to(
                    transform.transform_point(ctrl),
                    transform.transform_point(to),
                );
            }
            PathElement::CubicTo { ctrl1, ctrl2, to } => {
                builder.cubic_to(
                    transform.transform_point(ctrl1),
                    transform.transform_point(ctrl2),
                    transform.transform_point(to),
                );
            }
            PathElement::Close => {
                builder.close();
            }
        }
    }
    builder.build()
}

fn transformed_rect_path(rect: Rect, transform: Transform) -> Path {
    let points = transformed_rect_points(rect, transform);
    let mut builder = PathBuilder::new();
    builder
        .move_to(points[0])
        .line_to(points[1])
        .line_to(points[3])
        .line_to(points[2])
        .close();
    builder.build()
}

fn transformed_rect_points(rect: Rect, transform: Transform) -> [Point; 4] {
    [
        transform.transform_point(rect.origin),
        transform.transform_point(Point::new(rect.max_x(), rect.y())),
        transform.transform_point(Point::new(rect.x(), rect.max_y())),
        transform.transform_point(Point::new(rect.max_x(), rect.max_y())),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelRenderRange {
    start_x: usize,
    end_x: usize,
    start_y: usize,
    end_y: usize,
}

fn pixel_visible_range(
    viewport: CanvasViewport,
    bounds: Rect,
    document_origin: Point,
    width: usize,
    height: usize,
) -> Option<PixelRenderRange> {
    let visible = canvas_visible_world_rect(viewport, bounds, document_origin)
        .intersection(Rect::new(0.0, 0.0, width as f32, height as f32))?;
    let start_x = visible.x().floor().max(0.0) as usize;
    let start_y = visible.y().floor().max(0.0) as usize;
    let end_x = visible.max_x().ceil().min(width as f32) as usize;
    let end_y = visible.max_y().ceil().min(height as f32) as usize;
    if start_x >= end_x || start_y >= end_y {
        return None;
    }

    Some(PixelRenderRange {
        start_x,
        end_x,
        start_y,
        end_y,
    })
}

fn paint_pixel_grid(
    ctx: &mut PaintCtx,
    range: PixelRenderRange,
    transform: Transform,
    color: Color,
) {
    let mut builder = PathBuilder::new();
    for x in range.start_x..=range.end_x {
        let x = x as f32;
        builder
            .move_to(transform.transform_point(Point::new(x, range.start_y as f32)))
            .line_to(transform.transform_point(Point::new(x, range.end_y as f32)));
    }
    for y in range.start_y..=range.end_y {
        let y = y as f32;
        builder
            .move_to(transform.transform_point(Point::new(range.start_x as f32, y)))
            .line_to(transform.transform_point(Point::new(range.end_x as f32, y)));
    }
    ctx.stroke(builder.build(), color, StrokeStyle::new(1.0));
}

fn channel_to_u8(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::{
        Canvas, CanvasRuler, CanvasShape, CanvasStroke, CanvasViewport, PixelCanvas,
        PixelCanvasBlendMode, PixelCanvasBrushShape, PixelCanvasState, PixelCanvasTool, PixelColor,
    };
    use crate::{CanvasRulerAxis, DefaultTheme, ThemeTextToken};
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Modifiers, Point, PointerButton, PointerButtons,
        PointerEvent, PointerEventKind, Rect, ScrollDelta, SemanticsAction, SemanticsRole,
        SemanticsValue, Size, Vector, WindowEvent,
    };
    use sui_runtime::{Application, RenderOutput, Runtime, Widget, WindowBuilder};
    use sui_scene::{Brush, ImageSampling, RegisteredImage, SceneCommand};
    use sui_text::{FontFeature, FontRegistry, TextSystem};

    const RGBA_CHANNEL_TOLERANCE: u8 = 1;

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Canvas widgets").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn render<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let (mut runtime, window_id) = build_runtime(root);
        runtime.render(window_id).expect("render should succeed")
    }

    fn handle_ready_events(runtime: &mut Runtime) -> usize {
        let ready = runtime.drain_ready_events();
        let count = ready.len();
        for (window_id, event) in ready {
            runtime
                .handle_event(window_id, event)
                .expect("ready event should be handled");
        }
        count
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }
        let mut pointer = PointerEvent::new(kind, position);
        pointer.pointer_id = 1;
        pointer.button = Some(PointerButton::Primary);
        pointer.buttons = buttons;
        Event::Pointer(pointer)
    }

    fn command_key(key: &str) -> Event {
        let mut event = KeyboardEvent::new(key, KeyState::Pressed);
        event.modifiers.control = true;
        Event::Keyboard(event)
    }

    fn rendered_pixel_bytes(output: &RenderOutput) -> Vec<u8> {
        rendered_pixel_image(output).bytes().to_vec()
    }

    fn rendered_pixel_image(output: &RenderOutput) -> &RegisteredImage {
        let source = rendered_pixel_image_source(output);
        output
            .frame
            .image_registry
            .get(source.image)
            .expect("pixel canvas image should be registered")
    }

    fn rendered_pixel_image_source(output: &RenderOutput) -> sui_scene::ImageSource {
        let mut image_handle = None;
        let mut image_source = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                image_handle = Some(source.image);
                image_source = Some(source.clone());
            }
        });
        let source = image_source.expect("pixel canvas should draw an image");
        assert_eq!(
            Some(source.image),
            image_handle,
            "captured image source should match the image handle"
        );
        source
    }

    fn pixel_canvas_zoom_percent(output: &RenderOutput) -> f32 {
        let node = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some("Paint")
            })
            .expect("pixel canvas semantics should be present");
        let value = match node.value.as_ref() {
            Some(SemanticsValue::Text(value)) => value,
            _ => panic!("pixel canvas should expose zoom in text semantics"),
        };
        let start = value.find("zoom ").expect("zoom label should be present") + "zoom ".len();
        let end = value[start..]
            .find('%')
            .expect("zoom value should include percent")
            + start;
        value[start..end]
            .parse::<f32>()
            .expect("zoom percent should parse")
    }

    fn solid_fill_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::FillPath {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                colors.push(*color);
            }
        });
        colors
    }

    fn solid_stroke_colors(output: &RenderOutput) -> Vec<Color> {
        let mut colors = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::StrokeRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::StrokePath {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                colors.push(*color);
            }
        });
        colors
    }

    fn contains_approx_color(colors: &[Color], expected: Color) -> bool {
        const CHANNEL_TOLERANCE: f32 = 1.0 / 255.0;

        colors.iter().any(|color| {
            color.space == expected.space
                && (color.red - expected.red).abs() <= CHANNEL_TOLERANCE
                && (color.green - expected.green).abs() <= CHANNEL_TOLERANCE
                && (color.blue - expected.blue).abs() <= CHANNEL_TOLERANCE
                && (color.alpha - expected.alpha).abs() <= CHANNEL_TOLERANCE
        })
    }

    fn numeric_text_runs(output: &RenderOutput) -> Vec<sui_text::TextRun> {
        let mut runs = Vec::new();
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::DrawText(text_run)
                    if text_run
                        .text
                        .chars()
                        .all(|character| character.is_ascii_digit()) =>
                {
                    runs.push(text_run.clone());
                }
                SceneCommand::DrawShapedText(text) => {
                    if let Some(layout) = text.resolve(output.frame.text_layout_registry.as_ref())
                        && layout
                            .text()
                            .chars()
                            .all(|character| character.is_ascii_digit())
                    {
                        runs.push(sui_text::TextRun {
                            rect: Rect::new(
                                text.origin.x,
                                text.origin.y,
                                layout.box_size().width,
                                layout.box_size().height,
                            ),
                            text: layout.text().to_string(),
                            style: layout.style().clone(),
                        });
                    }
                }
                _ => {}
            });

        runs
    }

    fn optical_visual_center(measurement: sui_text::TextMeasurement) -> f32 {
        let top = -measurement.cap_height.unwrap_or(measurement.ascent);
        let bottom = measurement.descent * 0.5;

        (top + bottom) * 0.5
    }

    fn rgba_channels_match(actual: &[u8], expected: [u8; 4]) -> bool {
        actual.len() == 4
            && actual
                .iter()
                .zip(expected.iter())
                .all(|(actual, expected)| actual.abs_diff(*expected) <= RGBA_CHANNEL_TOLERANCE)
    }

    fn assert_rgba_channels_near(actual: &[u8], expected: [u8; 4]) {
        assert_eq!(
            actual.len(),
            4,
            "expected exactly one RGBA pixel, got {} channels",
            actual.len()
        );
        for channel in 0..4 {
            assert!(
                actual[channel].abs_diff(expected[channel]) <= RGBA_CHANNEL_TOLERANCE,
                "channel {channel} differed by more than {RGBA_CHANNEL_TOLERANCE}: got {}, expected {}",
                actual[channel],
                expected[channel]
            );
        }
    }

    fn assert_rgba_buffers_near(actual: &[u8], expected: &[u8]) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "RGBA buffer length mismatch: got {}, expected {}",
            actual.len(),
            expected.len()
        );
        assert_eq!(
            actual.len() % 4,
            0,
            "RGBA buffer length must be divisible by 4"
        );

        for (pixel_index, (actual_pixel, expected_pixel)) in actual
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .enumerate()
        {
            for channel in 0..4 {
                assert!(
                    actual_pixel[channel].abs_diff(expected_pixel[channel])
                        <= RGBA_CHANNEL_TOLERANCE,
                    "pixel {pixel_index} channel {channel} differed by more than {RGBA_CHANNEL_TOLERANCE}: got {}, expected {}",
                    actual_pixel[channel],
                    expected_pixel[channel]
                );
            }
        }
    }

    fn alpha_matches(pixel: &[u8], expected: u8) -> bool {
        pixel
            .get(3)
            .is_some_and(|alpha| alpha.abs_diff(expected) <= RGBA_CHANNEL_TOLERANCE)
    }

    fn pixel_matches_color(pixel: &[u8], color: PixelColor) -> bool {
        rgba_channels_match(pixel, [color.red, color.green, color.blue, color.alpha])
    }

    #[test]
    fn viewport_screen_world_mapping_round_trips_nonzero_bounds() {
        let viewport = CanvasViewport::new()
            .pan(Vector::new(24.0, -18.0))
            .zoom(2.35)
            .rotation(0.42);
        let bounds = Rect::new(180.0, 96.0, 420.0, 260.0);
        let document_origin = Point::new(16.0, 12.0);
        let cursor = Point::new(412.0, 214.0);

        let world = viewport.screen_to_world(bounds, cursor, document_origin);
        let screen = viewport.world_to_screen(bounds, world, document_origin);

        assert!((screen.x - cursor.x).abs() < 0.001);
        assert!((screen.y - cursor.y).abs() < 0.001);
    }

    #[test]
    fn canvas_ruler_exposes_semantics_and_draws_ticks() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(420.0, theme.metrics.canvas_ruler_extent))
                .with_child(
                    CanvasRuler::horizontal("Horizontal ruler", Size::new(1920.0, 1080.0))
                        .viewport(CanvasViewport::new().zoom(0.5), Size::new(420.0, 260.0)),
                ),
        );

        let ruler = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Horizontal ruler")
            })
            .expect("ruler semantics should exist");
        assert_eq!(
            ruler.bounds,
            Rect::new(0.0, 0.0, 420.0, theme.metrics.canvas_ruler_extent)
        );
        assert_eq!(
            ruler.value,
            Some(SemanticsValue::Text(
                "horizontal ruler, 1920 px document axis".to_string(),
            ))
        );

        let mut stroke_count = 0;
        let mut label_count = 0;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokePath { .. } => stroke_count += 1,
                SceneCommand::DrawText(_) | SceneCommand::DrawShapedText(_) => label_count += 1,
                _ => {}
            });

        assert!(stroke_count > 2);
        assert!(label_count > 0);
    }

    #[test]
    fn canvas_ruler_defaults_follow_theme_density_and_extent_override() {
        let compact = DefaultTheme::compact();
        let touch = DefaultTheme::touch();

        let compact_output = render(crate::SizedBox::new().width(420.0).with_child(
            CanvasRuler::horizontal("Compact ruler", Size::new(1920.0, 1080.0)).theme(compact),
        ));
        let touch_output = render(crate::SizedBox::new().width(420.0).with_child(
            CanvasRuler::horizontal("Touch ruler", Size::new(1920.0, 1080.0)).theme(touch),
        ));
        let override_output = render(
            crate::SizedBox::new().width(420.0).with_child(
                CanvasRuler::horizontal("Override ruler", Size::new(1920.0, 1080.0))
                    .theme(touch)
                    .extent(18.0),
            ),
        );

        let ruler_height = |output: &RenderOutput, name: &str| {
            output
                .semantics
                .iter()
                .find(|node| node.name.as_deref() == Some(name))
                .expect("ruler semantics should exist")
                .bounds
                .height()
        };

        assert_eq!(
            ruler_height(&compact_output, "Compact ruler"),
            compact.metrics.canvas_ruler_extent
        );
        assert_eq!(
            ruler_height(&touch_output, "Touch ruler"),
            touch.metrics.canvas_ruler_extent
        );
        assert_eq!(ruler_height(&override_output, "Override ruler"), 18.0);
    }

    #[test]
    fn canvas_ruler_numeric_labels_use_tabular_figures_and_optical_centering() {
        let theme = DefaultTheme::default();
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(420.0, theme.metrics.canvas_ruler_extent))
                .with_child(
                    CanvasRuler::horizontal("Horizontal ruler", Size::new(1920.0, 1080.0))
                        .viewport(
                            CanvasViewport::new().pan(Vector::new(750.0, 0.0)),
                            Size::new(420.0, 260.0),
                        ),
                ),
        );
        let text = numeric_text_runs(&output)
            .into_iter()
            .next()
            .expect("ruler should draw at least one numeric label");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("ruler label should shape");
        let line = layout
            .lines()
            .first()
            .expect("ruler label should contain one line");
        let measurement = layout.measurement();
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(measurement);
        let slot_center = theme.metrics.canvas_ruler_label_padding.top
            + (theme.metrics.canvas_ruler_extent
                - theme.metrics.canvas_ruler_label_padding.top
                - theme.metrics.canvas_ruler_label_padding.bottom)
                * 0.5;

        assert!(
            text.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!(
            (text.rect.width() - measurement.width).abs() < 0.75,
            "ruler label rect should use measured width: rect={:?}, measurement={measurement:?}",
            text.rect
        );
        assert!(
            (actual_visual_center - slot_center).abs() < 0.75,
            "ruler label visual center {actual_visual_center} did not match slot center {slot_center}; text rect {:?}",
            text.rect
        );
    }

    #[test]
    fn horizontal_canvas_ruler_preserves_tall_label_measurements() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 32.0,
            line_height: 12.0,
        };
        theme.metrics.canvas_ruler_label_max_width = 120.0;

        let extent = 96.0;
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(420.0, extent))
                .with_child(
                    CanvasRuler::horizontal("Tall horizontal ruler", Size::new(1920.0, 1080.0))
                        .theme(theme)
                        .extent(extent)
                        .viewport(
                            CanvasViewport::new().pan(Vector::new(750.0, 0.0)),
                            Size::new(420.0, 260.0),
                        ),
                ),
        );
        let text = numeric_text_runs(&output)
            .into_iter()
            .next()
            .expect("horizontal ruler should draw at least one numeric label");
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("ruler label should shape");
        let measurement = layout.measurement();
        let line = layout
            .lines()
            .first()
            .expect("ruler label should contain one line");
        let slot_center = theme.metrics.canvas_ruler_label_padding.top
            + (extent
                - theme.metrics.canvas_ruler_label_padding.top
                - theme.metrics.canvas_ruler_label_padding.bottom)
                * 0.5;
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(measurement);

        assert_eq!(text.style.font_size, theme.text.xs.size);
        assert_eq!(text.style.line_height, theme.text.xs.line_height);
        assert!(
            text.rect.height() >= measurement.height - 0.01,
            "horizontal ruler label rect should preserve measured glyph height: rect={:?}, measurement={measurement:?}",
            text.rect
        );
        assert!(
            text.rect.height() > text.style.line_height,
            "test should exercise a measurement taller than line-height: rect={:?}, style={:?}",
            text.rect,
            text.style
        );
        assert!(
            (actual_visual_center - slot_center).abs() < 0.75,
            "horizontal ruler tall label visual center {actual_visual_center} did not match slot center {slot_center}; text rect {:?}",
            text.rect
        );
    }

    #[test]
    fn vertical_canvas_ruler_numeric_labels_center_on_tick_position() {
        let theme = DefaultTheme::default();
        let document_size = Size::new(1920.0, 1080.0);
        let viewport = CanvasViewport::new();
        let viewport_size = Size::new(260.0, 420.0);
        let extent = 72.0;
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(extent, 420.0))
                .with_child(
                    CanvasRuler::vertical("Vertical ruler", document_size)
                        .extent(extent)
                        .viewport(viewport, viewport_size),
                ),
        );
        let bounds = Rect::new(0.0, 0.0, extent, 420.0);
        let canvas_bounds =
            super::ruler_canvas_bounds(bounds, CanvasRulerAxis::Vertical, viewport_size);
        let document_origin = Point::new(document_size.width * 0.5, document_size.height * 0.5);
        let padding = theme.metrics.canvas_ruler_label_padding;
        let text = numeric_text_runs(&output)
            .into_iter()
            .find(|run| {
                let Ok(value) = run.text.parse::<f32>() else {
                    return false;
                };
                let tick_y = super::ruler_tick_screen_position(
                    CanvasRulerAxis::Vertical,
                    value,
                    document_size,
                    viewport,
                    canvas_bounds,
                    document_origin,
                );

                tick_y > bounds.y() + padding.top + run.style.line_height
                    && tick_y < bounds.max_y() - padding.bottom - run.style.line_height
            })
            .expect("vertical ruler should draw an unclamped numeric label");
        let value = text
            .text
            .parse::<f32>()
            .expect("numeric ruler label should parse");
        let expected_tick_y = super::ruler_tick_screen_position(
            CanvasRulerAxis::Vertical,
            value,
            document_size,
            viewport,
            canvas_bounds,
            document_origin,
        );
        let layout = TextSystem::new()
            .shape_text_run(&text, &FontRegistry::new())
            .expect("ruler label should shape");
        let line = layout
            .lines()
            .first()
            .expect("ruler label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(layout.measurement());

        assert!(
            text.style
                .features
                .iter()
                .any(|feature| feature.tag == FontFeature::TABULAR_FIGURES && feature.value == 1)
        );
        assert!(
            (actual_visual_center - expected_tick_y).abs() < 0.75,
            "vertical ruler label visual center {actual_visual_center} did not match tick {expected_tick_y}; text rect {:?}",
            text.rect
        );
    }

    #[test]
    fn vertical_canvas_ruler_preserves_tall_label_measurements() {
        let mut theme = DefaultTheme::default();
        theme.text.xs = ThemeTextToken {
            size: 32.0,
            line_height: 12.0,
        };
        theme.metrics.canvas_ruler_label_max_width = 120.0;

        let document_size = Size::new(1920.0, 1080.0);
        let viewport = CanvasViewport::new();
        let viewport_size = Size::new(260.0, 420.0);
        let extent = 96.0;
        let output = render(
            crate::SizedBox::new()
                .size(Size::new(extent, 420.0))
                .with_child(
                    CanvasRuler::vertical("Tall vertical ruler", document_size)
                        .theme(theme)
                        .extent(extent)
                        .viewport(viewport, viewport_size),
                ),
        );
        let bounds = Rect::new(0.0, 0.0, extent, 420.0);
        let canvas_bounds =
            super::ruler_canvas_bounds(bounds, CanvasRulerAxis::Vertical, viewport_size);
        let document_origin = Point::new(document_size.width * 0.5, document_size.height * 0.5);
        let padding = theme.metrics.canvas_ruler_label_padding;
        let text_system = TextSystem::new();
        let text = numeric_text_runs(&output)
            .into_iter()
            .find(|run| {
                let Ok(value) = run.text.parse::<f32>() else {
                    return false;
                };
                let Ok(layout) = text_system.shape_text_run(run, &FontRegistry::new()) else {
                    return false;
                };
                let tick_y = super::ruler_tick_screen_position(
                    CanvasRulerAxis::Vertical,
                    value,
                    document_size,
                    viewport,
                    canvas_bounds,
                    document_origin,
                );
                let height = run.style.line_height.max(layout.measurement().height);

                tick_y > bounds.y() + padding.top + height
                    && tick_y < bounds.max_y() - padding.bottom - height
            })
            .expect("vertical ruler should draw an unclamped tall numeric label");
        let value = text
            .text
            .parse::<f32>()
            .expect("numeric ruler label should parse");
        let expected_tick_y = super::ruler_tick_screen_position(
            CanvasRulerAxis::Vertical,
            value,
            document_size,
            viewport,
            canvas_bounds,
            document_origin,
        );
        let layout = text_system
            .shape_text_run(&text, &FontRegistry::new())
            .expect("ruler label should shape");
        let measurement = layout.measurement();
        let line = layout
            .lines()
            .first()
            .expect("ruler label should contain one line");
        let actual_visual_center =
            text.rect.y() + line.baseline + optical_visual_center(measurement);

        assert!(
            text.rect.height() >= measurement.height - 0.01,
            "vertical ruler label rect should preserve measured glyph height: rect={:?}, measurement={measurement:?}",
            text.rect
        );
        assert!(
            text.rect.height() > text.style.line_height,
            "test should exercise a measurement taller than line-height: rect={:?}, style={:?}",
            text.rect,
            text.style
        );
        assert!(
            (actual_visual_center - expected_tick_y).abs() < 0.75,
            "vertical ruler tall label visual center {actual_visual_center} did not match tick {expected_tick_y}; text rect {:?}",
            text.rect
        );
    }

    #[test]
    fn canvas_surface_tokens_drive_default_canvas_paint() {
        let mut theme = DefaultTheme::default();
        theme.surfaces.canvas = Color::rgba(0.91, 0.83, 0.72, 1.0);
        theme.surfaces.canvas_grid = Color::rgba(0.24, 0.33, 0.44, 0.40);
        theme.surfaces.canvas_axis_x = Color::rgba(0.80, 0.20, 0.30, 0.70);
        theme.surfaces.canvas_axis_y = Color::rgba(0.20, 0.60, 0.40, 0.70);

        let output = render(Canvas::new("Vector").theme(theme));
        let fills = solid_fill_colors(&output);
        let strokes = solid_stroke_colors(&output);

        assert!(fills.contains(&theme.surfaces.canvas));
        assert!(strokes.contains(&theme.surfaces.canvas_grid));
        assert!(strokes.contains(&theme.surfaces.canvas_axis_x));
        assert!(strokes.contains(&theme.surfaces.canvas_axis_y));
    }

    #[test]
    fn canvas_focus_ring_uses_theme_motion() {
        let theme = DefaultTheme::default();
        let focus_duration = theme.motion.focus_duration();
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(160.0, 120.0))
                .with_child(Canvas::new("Vector").theme(theme)),
        );

        let _ = runtime.render(window_id).expect("render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(30.0, 30.0), true),
            )
            .expect("focus event should be handled");

        runtime.tick(focus_duration * 0.5);
        assert!(handle_ready_events(&mut runtime) >= 1);
        let mid_focus = runtime.render(window_id).expect("render should succeed");
        assert!(
            !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
            "canvas focus ring should not snap to the settled focus color"
        );

        runtime.tick(focus_duration + 0.01);
        assert!(handle_ready_events(&mut runtime) >= 1);
        let settled_focus = runtime.render(window_id).expect("render should succeed");
        let settled_strokes = solid_stroke_colors(&settled_focus);
        assert!(
            contains_approx_color(&settled_strokes, theme.palette.focus_ring),
            "canvas focus ring should settle to the theme focus color; strokes={settled_strokes:?}"
        );
    }

    #[test]
    fn pixel_canvas_focus_ring_uses_theme_motion() {
        let theme = DefaultTheme::default();
        let focus_duration = theme.motion.focus_duration();
        let (mut runtime, window_id) = build_runtime(
            crate::SizedBox::new()
                .size(Size::new(160.0, 120.0))
                .with_child(PixelCanvas::new("Paint", 8, 8).theme(theme)),
        );

        let _ = runtime.render(window_id).expect("render should succeed");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(30.0, 30.0), true),
            )
            .expect("focus event should be handled");

        runtime.tick(focus_duration * 0.5);
        assert!(handle_ready_events(&mut runtime) >= 1);
        let mid_focus = runtime.render(window_id).expect("render should succeed");
        assert!(
            !contains_approx_color(&solid_stroke_colors(&mid_focus), theme.palette.focus_ring),
            "pixel canvas focus ring should not snap to the settled focus color"
        );

        runtime.tick(focus_duration + 0.01);
        assert!(handle_ready_events(&mut runtime) >= 1);
        let settled_focus = runtime.render(window_id).expect("render should succeed");
        let settled_strokes = solid_stroke_colors(&settled_focus);
        assert!(
            contains_approx_color(&settled_strokes, theme.palette.focus_ring),
            "pixel canvas focus ring should settle to the theme focus color; strokes={settled_strokes:?}"
        );
    }

    #[test]
    fn canvas_renders_shapes_inside_a_transform() {
        let output = render(
            Canvas::new("Vector")
                .viewport(CanvasViewport::new().zoom(1.5).rotation(0.2))
                .shape(CanvasShape::rect(
                    Rect::new(-20.0, -20.0, 40.0, 40.0),
                    Some(Color::rgba(0.2, 0.4, 0.8, 1.0)),
                    Some(CanvasStroke::new(Color::rgba(0.0, 0.0, 0.0, 1.0), 1.0)),
                )),
        );

        let has_canvas = output.semantics.iter().any(|node| {
            node.role == SemanticsRole::Canvas && node.name.as_deref() == Some("Vector")
        });
        assert!(has_canvas);
        let mut path_count = 0;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillPath { .. } | SceneCommand::StrokePath { .. } => path_count += 1,
                _ => {}
            });
        assert!(path_count >= 1);
    }

    #[test]
    fn canvas_primary_drag_adds_vector_stroke() -> sui_core::Result<()> {
        let (mut runtime, window_id) = build_runtime(Canvas::new("Vector"));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(280.0, 190.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(280.0, 190.0), false),
        )?;
        let output = runtime.render(window_id)?;

        let mut stroke_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            if matches!(command, SceneCommand::StrokePath { .. }) {
                stroke_count += 1;
            }
        });
        assert!(stroke_count >= 3);
        Ok(())
    }

    #[test]
    fn pixel_canvas_primary_drag_paints_pixel() -> sui_core::Result<()> {
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 8, 8));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        let output = runtime.render(window_id)?;

        let mut image_handle = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                image_handle = Some(source.image);
            }
        });
        let image = output
            .frame
            .image_registry
            .get(image_handle.expect("pixel canvas should draw an image"))
            .expect("pixel canvas image should be registered");
        let accent = PixelColor::from_color(DefaultTheme::default().palette.accent);
        let painted = image
            .bytes()
            .chunks_exact(4)
            .any(|pixel| pixel_matches_color(pixel, accent));
        assert!(painted);
        Ok(())
    }

    #[test]
    fn pixel_canvas_fast_brush_drag_interpolates_sparse_pointer_samples() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_brush_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
        state.set_brush_size(1.0);
        let (mut runtime, window_id) =
            build_runtime(PixelCanvas::new("Paint", 8, 8).state(state.clone()));

        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("pixel canvas semantics present");
        let document_origin = Point::new(4.0, 4.0);
        let start =
            state
                .viewport()
                .world_to_screen(canvas.bounds, Point::new(1.5, 4.5), document_origin);
        let middle =
            state
                .viewport()
                .world_to_screen(canvas.bounds, Point::new(4.5, 4.5), document_origin);
        let end =
            state
                .viewport()
                .world_to_screen(canvas.bounds, Point::new(6.5, 4.5), document_origin);

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, start, true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, middle, true),
        )?;
        runtime.handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))?;

        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        for x in 1..=6 {
            let index = ((4 * 8) + x) * 4;
            assert_rgba_channels_near(&pixels[index..index + 4], [255, 0, 0, 255]);
        }
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_controls_brush_color_and_size() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_brush_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
        state.set_brush_size(3.0);
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 8, 8).state(state));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        let output = runtime.render(window_id)?;

        let mut image_handle = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                image_handle = Some(source.image);
            }
        });
        let image = output
            .frame
            .image_registry
            .get(image_handle.expect("pixel canvas should draw an image"))
            .expect("pixel canvas image should be registered");
        let painted = image
            .bytes()
            .chunks_exact(4)
            .filter(|pixel| rgba_channels_match(pixel, [255, 0, 0, 255]))
            .count();
        assert_eq!(painted, 9);
        Ok(())
    }

    #[test]
    fn pixel_canvas_blend_mode_composes_brush_with_existing_pixels() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_blend_mode(PixelCanvasBlendMode::Screen);
        state.set_brush_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
        state.set_brush_size(1.0);
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 1, 1)
                .state(state)
                .with_pixels(vec![Color::rgba(0.0, 0.0, 1.0, 1.0)]),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);

        assert_rgba_channels_near(&pixels[0..4], [255, 0, 255, 255]);
        Ok(())
    }

    #[test]
    fn pixel_canvas_round_brush_uses_circular_stamp() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_brush_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
        state.set_brush_size(3.0);
        state.set_brush_shape(PixelCanvasBrushShape::Round);
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 8, 8).state(state));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        let painted = pixels
            .chunks_exact(4)
            .filter(|pixel| rgba_channels_match(pixel, [255, 0, 0, 255]))
            .count();

        assert_eq!(painted, 5);
        Ok(())
    }

    #[test]
    fn pixel_canvas_eraser_tool_clears_painted_pixels() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_tool(PixelCanvasTool::Eraser);
        let pixels = vec![Color::rgba(0.0, 0.2, 1.0, 1.0); 64];
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 8, 8)
                .state(state)
                .with_pixels(pixels),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        let output = runtime.render(window_id)?;

        let mut image_handle = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                image_handle = Some(source.image);
            }
        });
        let image = output
            .frame
            .image_registry
            .get(image_handle.expect("pixel canvas should draw an image"))
            .expect("pixel canvas image should be registered");
        let transparent = image
            .bytes()
            .chunks_exact(4)
            .filter(|pixel| alpha_matches(pixel, 0))
            .count();
        assert_eq!(transparent, 1);
        Ok(())
    }

    #[test]
    fn pixel_canvas_fill_tool_flood_fills_matching_pixels() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_tool(PixelCanvasTool::Fill);
        state.set_brush_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 8, 8).state(state));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        let output = runtime.render(window_id)?;

        let mut image_handle = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                image_handle = Some(source.image);
            }
        });
        let image = output
            .frame
            .image_registry
            .get(image_handle.expect("pixel canvas should draw an image"))
            .expect("pixel canvas image should be registered");
        let red = image
            .bytes()
            .chunks_exact(4)
            .filter(|pixel| rgba_channels_match(pixel, [255, 0, 0, 255]))
            .count();
        assert_eq!(red, 64);
        Ok(())
    }

    #[test]
    fn pixel_canvas_keyboard_undo_redo_restores_pixel_edits() -> sui_core::Result<()> {
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 8, 8));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        let accent = PixelColor::from_color(DefaultTheme::default().palette.accent);
        let painted = rendered_pixel_bytes(&runtime.render(window_id)?)
            .chunks_exact(4)
            .any(|pixel| pixel_matches_color(pixel, accent));
        assert!(painted);

        runtime.handle_event(window_id, command_key("z"))?;
        let painted_after_undo = rendered_pixel_bytes(&runtime.render(window_id)?)
            .chunks_exact(4)
            .any(|pixel| !alpha_matches(pixel, 0));
        assert!(!painted_after_undo);

        runtime.handle_event(window_id, command_key("y"))?;
        let painted_after_redo = rendered_pixel_bytes(&runtime.render(window_id)?)
            .chunks_exact(4)
            .any(|pixel| pixel_matches_color(pixel, accent));
        assert!(painted_after_redo);
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_undo_request_is_consumed_during_measure() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        let (mut runtime, window_id) =
            build_runtime(PixelCanvas::new("Paint", 8, 8).state(state.clone()));

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;
        assert!(state.can_undo());

        state.request_undo();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let painted_after_undo = rendered_pixel_bytes(&runtime.render(window_id)?)
            .chunks_exact(4)
            .any(|pixel| !alpha_matches(pixel, 0));
        assert!(!painted_after_undo);
        assert!(state.can_redo());
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_clear_request_clears_pixels_and_supports_undo() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        let pixels = vec![Color::rgba(0.2, 0.4, 0.8, 1.0); 4];
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 2, 2)
                .state(state.clone())
                .with_pixels(pixels),
        );

        let _ = runtime.render(window_id)?;
        state.request_clear();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let cleared = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert!(cleared.chunks_exact(4).all(|pixel| alpha_matches(pixel, 0)));
        assert!(state.can_undo());

        state.request_undo();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(522.0, 362.0))),
        )?;
        let restored = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert!(
            restored
                .chunks_exact(4)
                .all(|pixel| alpha_matches(pixel, 255))
        );
        assert!(state.can_redo());
        Ok(())
    }

    #[test]
    fn pixel_canvas_read_only_blocks_edit_commands_and_reports_state() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_editable(false);
        state.set_tool(PixelCanvasTool::Fill);
        state.set_brush_color(Color::rgba(1.0, 0.0, 0.0, 1.0));
        let pixels = vec![Color::rgba(0.2, 0.4, 0.8, 1.0); 4];
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 2, 2)
                .state(state.clone())
                .with_pixels(pixels),
        );

        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("pixel canvas semantics present");
        let value = match canvas.value.as_ref() {
            Some(SemanticsValue::Text(value)) => value,
            _ => panic!("pixel canvas should expose text value"),
        };
        assert!(value.contains("read only"));
        assert!(
            canvas
                .actions
                .contains(&SemanticsAction::Custom("Pan".into()))
        );
        assert!(
            !canvas
                .actions
                .contains(&SemanticsAction::Custom("Paint".into()))
        );
        assert!(!state.can_clear());

        let before = rendered_pixel_bytes(&output);
        state.request_clear();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(260.0, 180.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(260.0, 180.0), false),
        )?;

        let after = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert_rgba_buffers_near(&after, &before);
        assert!(!state.can_undo());
        Ok(())
    }

    #[test]
    fn pixel_canvas_display_visibility_affects_render_and_export() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_display_visible(false);
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 1, 1)
                .state(state.clone())
                .with_pixels(vec![Color::rgba(1.0, 0.0, 0.0, 1.0)]),
        );
        let paper = PixelColor::from_color(DefaultTheme::default().surfaces.pixel_canvas_paper);
        let expected = [paper.red, paper.green, paper.blue, paper.alpha];

        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert_rgba_channels_near(&pixels[0..4], expected);

        state.request_export_snapshot();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let snapshot = state
            .latest_export_snapshot()
            .expect("hidden layer export should publish a snapshot");
        assert_rgba_channels_near(&snapshot.rgba8()[0..4], expected);
        Ok(())
    }

    #[test]
    fn pixel_canvas_uses_theme_paper_for_render_and_export() -> sui_core::Result<()> {
        let mut theme = DefaultTheme::default();
        theme.surfaces.pixel_canvas_paper = Color::rgba(0.80, 0.62, 0.36, 1.0);
        let state = PixelCanvasState::new();
        state.set_display_visible(false);
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 1, 1)
                .theme(theme)
                .state(state.clone())
                .with_pixels(vec![Color::rgba(1.0, 0.0, 0.0, 1.0)]),
        );
        let paper = PixelColor::from_color(theme.surfaces.pixel_canvas_paper);
        let expected = [paper.red, paper.green, paper.blue, paper.alpha];

        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert_rgba_channels_near(&pixels[0..4], expected);

        state.request_export_snapshot();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let snapshot = state
            .latest_export_snapshot()
            .expect("hidden layer export should publish a snapshot");
        assert_rgba_channels_near(&snapshot.rgba8()[0..4], expected);
        Ok(())
    }

    #[test]
    fn pixel_canvas_display_opacity_and_blend_affect_rendered_image() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_display_opacity(0.5);
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 1, 1)
                .state(state.clone())
                .with_pixels(vec![Color::rgba(1.0, 0.0, 0.0, 1.0)]),
        );
        let paper = PixelColor::from_color(DefaultTheme::default().surfaces.pixel_canvas_paper);
        let half = paper.compose(
            Color::rgba(1.0, 0.0, 0.0, 1.0),
            0.5,
            PixelCanvasBlendMode::Normal,
        );

        let output = runtime.render(window_id)?;
        let source = rendered_pixel_image_source(&output);
        assert_eq!(source.tint, Some(Color::WHITE.with_alpha(0.5)));
        let pixels = rendered_pixel_bytes(&output);
        assert_rgba_channels_near(&pixels[0..4], [255, 0, 0, 255]);

        state.request_export_snapshot();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let snapshot = state
            .latest_export_snapshot()
            .expect("display opacity export should publish composited pixels");
        assert_rgba_channels_near(
            &snapshot.rgba8()[0..4],
            [half.red, half.green, half.blue, half.alpha],
        );

        state.set_display_opacity(1.0);
        state.set_display_blend_mode(PixelCanvasBlendMode::Multiply);
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let multiply = paper.compose(
            Color::rgba(1.0, 0.0, 0.0, 1.0),
            1.0,
            PixelCanvasBlendMode::Multiply,
        );
        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert_rgba_channels_near(
            &pixels[0..4],
            [multiply.red, multiply.green, multiply.blue, multiply.alpha],
        );
        Ok(())
    }

    #[test]
    fn pixel_canvas_layer_opacity_reuses_registered_data_between_repaints() -> sui_core::Result<()>
    {
        let state = PixelCanvasState::new();
        state.set_display_opacity(0.5);
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::new("Paint", 1, 1)
                .state(state)
                .with_pixels(vec![Color::rgba(1.0, 0.0, 0.0, 1.0)]),
        );

        let first = runtime.render(window_id)?;
        let second = runtime.render(window_id)?;

        assert!(std::ptr::addr_eq(
            rendered_pixel_image(&first).bytes().as_ptr(),
            rendered_pixel_image(&second).bytes().as_ptr()
        ));
        Ok(())
    }

    #[test]
    fn pixel_canvas_paper_visibility_affects_render_and_export() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_paper_visible(false);
        let (mut runtime, window_id) =
            build_runtime(PixelCanvas::new("Paint", 1, 1).state(state.clone()));

        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert_rgba_channels_near(&pixels[0..4], [0, 0, 0, 0]);

        state.request_export_snapshot();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let snapshot = state
            .latest_export_snapshot()
            .expect("hidden paper export should publish a snapshot");
        assert_rgba_channels_near(&snapshot.rgba8()[0..4], [0, 0, 0, 0]);
        Ok(())
    }

    #[test]
    fn pixel_canvas_paper_opacity_affects_composited_background() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        state.set_paper_opacity(0.5);
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 1, 1).state(state));
        let paper = PixelColor::from_color(
            DefaultTheme::default()
                .surfaces
                .pixel_canvas_paper
                .with_alpha(0.5),
        );
        let expected = [paper.red, paper.green, paper.blue, paper.alpha];

        let pixels = rendered_pixel_bytes(&runtime.render(window_id)?);
        assert_rgba_channels_near(&pixels[0..4], expected);
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_export_request_publishes_rgba_snapshot() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        let (mut runtime, window_id) = build_runtime(
            PixelCanvas::from_fn("Export", 2, 2, |x, y| {
                if x == 0 && y == 0 {
                    Color::rgba(1.0, 0.0, 0.0, 1.0)
                } else {
                    Color::rgba(0.0, 0.0, 0.0, 0.0)
                }
            })
            .state(state.clone()),
        );

        let _ = runtime.render(window_id)?;
        assert!(state.latest_export_snapshot().is_none());

        state.request_export_snapshot();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(521.0, 361.0))),
        )?;
        let _ = runtime.render(window_id)?;

        let snapshot = state
            .latest_export_snapshot()
            .expect("export request should publish a snapshot");
        assert_eq!(snapshot.name(), "Export");
        assert_eq!(snapshot.width(), 2);
        assert_eq!(snapshot.height(), 2);
        assert_eq!(snapshot.byte_len(), 16);
        assert_rgba_channels_near(&snapshot.rgba8()[0..4], [255, 0, 0, 255]);
        assert_eq!(snapshot.revision(), 1);
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_tracks_cursor_document_position() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        let (mut runtime, window_id) =
            build_runtime(PixelCanvas::new("Paint", 8, 8).state(state.clone()));
        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Canvas)
            .expect("canvas semantics should exist");
        let center = Point::new(
            canvas.bounds.x() + canvas.bounds.width() * 0.5,
            canvas.bounds.y() + canvas.bounds.height() * 0.5,
        );

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, center, false),
        )?;
        assert_eq!(state.cursor_position(), Some(Point::new(4.0, 4.0)));

        runtime.handle_event(
            window_id,
            primary_pointer(
                PointerEventKind::Move,
                Point::new(canvas.bounds.max_x() + 12.0, canvas.bounds.max_y() + 12.0),
                false,
            ),
        )?;
        assert_eq!(state.cursor_position(), None);
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_view_requests_update_zoom_during_arrange() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        let canvas = PixelCanvas::new("Paint", 8, 8)
            .viewport(
                CanvasViewport::new()
                    .pan(Vector::new(40.0, 16.0))
                    .zoom(0.25),
            )
            .state(state.clone());
        let (mut runtime, window_id) = build_runtime(canvas);

        let _ = runtime.render(window_id)?;
        state.request_actual_size_view();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 240.0))),
        )?;
        let actual_size = runtime.render(window_id)?;
        assert_eq!(pixel_canvas_zoom_percent(&actual_size), 100.0);

        state.request_fit_view();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(400.0, 300.0))),
        )?;
        let fit = runtime.render(window_id)?;
        assert!(pixel_canvas_zoom_percent(&fit) > 1000.0);
        Ok(())
    }

    #[test]
    fn pixel_canvas_state_zoom_requests_update_view_during_arrange() -> sui_core::Result<()> {
        let state = PixelCanvasState::new();
        let canvas = PixelCanvas::new("Paint", 8, 8)
            .viewport(CanvasViewport::new().zoom(1.0))
            .state(state.clone());
        let (mut runtime, window_id) = build_runtime(canvas);

        let _ = runtime.render(window_id)?;
        state.request_zoom_in();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 240.0))),
        )?;
        let zoomed_in = runtime.render(window_id)?;
        assert_eq!(pixel_canvas_zoom_percent(&zoomed_in), 110.0);

        state.request_zoom_out();
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(320.0, 240.0))),
        )?;
        let zoomed_out = runtime.render(window_id)?;
        assert_eq!(pixel_canvas_zoom_percent(&zoomed_out), 100.0);
        Ok(())
    }

    #[test]
    fn pixel_canvas_can_fit_initial_view_on_first_layout() {
        let state = PixelCanvasState::new();
        let output = render(
            PixelCanvas::new("Paint", 1920, 1080)
                .state(state.clone())
                .fit_on_first_layout(),
        );

        assert_eq!(pixel_canvas_zoom_percent(&output), 25.0);
        assert_eq!(state.viewport_size(), Size::new(520.0, 360.0));
        let padding = DefaultTheme::default().metrics.pixel_canvas_fit_padding;
        let expected_zoom = ((state.viewport_size().width - padding * 2.0) / 1920.0)
            .min((state.viewport_size().height - padding * 2.0) / 1080.0);
        assert!((state.viewport().zoom - expected_zoom).abs() < 0.001);
    }

    #[test]
    fn pixel_canvas_draws_one_image_instead_of_per_pixel_rects() {
        let output = render(
            PixelCanvas::from_fn("Large paint", 1920, 1080, |x, y| {
                Color::rgba(x as f32 / 1919.0, y as f32 / 1079.0, 0.5, 1.0)
            })
            .viewport(CanvasViewport::new().zoom(0.28)),
        );

        let mut image_handle = None;
        let mut image_command_count = 0;
        let mut fill_command_count = 0;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::DrawImage { source, .. }
                | SceneCommand::DrawImageQuad { source, .. } => {
                    image_handle = Some(source.image);
                    image_command_count += 1;
                }
                SceneCommand::FillPath { .. } | SceneCommand::FillRect { .. } => {
                    fill_command_count += 1;
                }
                _ => {}
            });

        assert_eq!(image_command_count, 1);
        assert!(
            fill_command_count <= 5,
            "pixel canvas should only issue bounded workbench, shadow, and paper fills"
        );
        let mut sampling = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                sampling = Some(source.sampling);
            }
        });
        assert_eq!(sampling, Some(ImageSampling::Linear));
        let image = output
            .frame
            .image_registry
            .get(image_handle.expect("pixel canvas should draw an image"))
            .expect("pixel canvas image should be registered");
        assert_eq!(image.width(), 1920);
        assert_eq!(image.height(), 1080);
    }

    #[test]
    fn pixel_canvas_uses_nearest_sampling_at_pixel_zoom() {
        let output = render(PixelCanvas::new("Paint", 8, 8));

        let mut sampling = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawImage { source, .. }
            | SceneCommand::DrawImageQuad { source, .. } = command
            {
                sampling = Some(source.sampling);
            }
        });

        assert_eq!(sampling, Some(ImageSampling::Nearest));
    }

    #[test]
    fn shift_wheel_rotates_pixel_canvas() -> sui_core::Result<()> {
        let (mut runtime, window_id) = build_runtime(PixelCanvas::new("Paint", 8, 8));
        let _ = runtime.render(window_id)?;

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(260.0, 180.0));
        scroll.modifiers = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, 24.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll))?;
        let output = runtime.render(window_id)?;
        let canvas = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Canvas && node.name.as_deref() == Some("Paint")
            })
            .expect("pixel canvas semantics present");

        assert!(
            canvas
                .value
                .as_ref()
                .is_some_and(|value| format!("{value:?}").contains("rotation"))
        );
        Ok(())
    }

    #[test]
    fn pixel_canvas_builder_accepts_resolution_pixels() {
        let pixels = vec![Color::rgba(1.0, 0.0, 0.0, 1.0); 4];
        let canvas = PixelCanvas::new("Paint", 2, 2)
            .with_pixels(pixels)
            .desired_size(Size::new(64.0, 64.0));

        assert_eq!(canvas.width(), 2);
        assert_eq!(canvas.height(), 2);
        assert_eq!(canvas.pixel_at(1, 1), Some(Color::rgba(1.0, 0.0, 0.0, 1.0)));
    }
}
