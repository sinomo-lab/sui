use sui_core::{
    Color, Event, KeyState, Path, PathBuilder, PathElement, Point, PointerButton, PointerEventKind,
    Rect, ScrollDelta, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
    Transform, Vector,
};
use sui_layout::Constraints;
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::StrokeStyle;

use crate::DefaultTheme;

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
        Self::Path {
            path,
            fill: None,
            stroke: Some(CanvasStroke::new(Color::rgba(0.16, 0.32, 0.72, 1.0), 2.0)),
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
    name: String,
    viewport: CanvasViewport,
    shapes: Vec<CanvasShape>,
    active_stroke: Option<Vec<Point>>,
    drag: Option<CanvasDrag>,
    draw_stroke: CanvasStroke,
    desired_size: Size,
}

impl Canvas {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            viewport: CanvasViewport::default(),
            shapes: Vec::new(),
            active_stroke: None,
            drag: None,
            draw_stroke: CanvasStroke::new(Color::rgba(0.10, 0.28, 0.78, 1.0), 2.5),
            desired_size: Size::new(520.0, 360.0),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
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
                match key.key.as_str() {
                    "=" | "+" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        1.1,
                        self.document_origin(),
                    ),
                    "-" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        1.0 / 1.1,
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
        let palette = self.theme.palette;
        ctx.fill_bounds(Color::rgba(0.955, 0.965, 0.975, 1.0));
        ctx.stroke_bounds(palette.border, StrokeStyle::new(1.0));
        ctx.push_clip_rect(ctx.bounds());
        paint_canvas_grid(ctx, self.viewport, ctx.bounds(), self.document_origin());
        paint_canvas_axes(ctx, self.viewport, ctx.bounds(), self.document_origin());
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

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
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
    },
}

pub struct PixelCanvas {
    theme: DefaultTheme,
    name: String,
    width: usize,
    height: usize,
    pixels: Vec<Color>,
    viewport: CanvasViewport,
    brush: Color,
    drag: Option<PixelCanvasDrag>,
    desired_size: Size,
}

impl PixelCanvas {
    pub fn new(name: impl Into<String>, width: usize, height: usize) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            theme: DefaultTheme::default(),
            name: name.into(),
            width,
            height,
            pixels: vec![Color::TRANSPARENT; width * height],
            viewport: CanvasViewport::new().zoom(14.0),
            brush: Color::rgba(0.12, 0.28, 0.88, 1.0),
            drag: None,
            desired_size: Size::new(520.0, 360.0),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
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

    pub fn brush_color(mut self, color: Color) -> Self {
        self.brush = color;
        self
    }

    pub fn with_pixels(mut self, pixels: Vec<Color>) -> Self {
        if pixels.len() == self.pixels.len() {
            self.pixels = pixels;
        }
        self
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) -> bool {
        let Some(index) = self.pixel_index(x, y) else {
            return false;
        };
        self.pixels[index] = color;
        true
    }

    pub fn pixel_at(&self, x: usize, y: usize) -> Option<Color> {
        self.pixel_index(x, y).map(|index| self.pixels[index])
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

    fn paint_at_position(&mut self, bounds: Rect, position: Point) -> bool {
        let world = self
            .viewport
            .screen_to_world(bounds, position, self.document_origin());
        let x = world.x.floor() as isize;
        let y = world.y.floor() as isize;
        if x < 0 || y < 0 {
            return false;
        }
        self.set_pixel(x as usize, y as usize, self.brush)
    }

    fn request_interaction_update(ctx: &mut EventCtx) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

impl Widget for PixelCanvas {
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
                self.paint_at_position(ctx.bounds(), pointer.position);
                self.drag = Some(PixelCanvasDrag::Paint {
                    pointer_id: pointer.pointer_id,
                });
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
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
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
                Some(PixelCanvasDrag::Paint { pointer_id }) if pointer_id == pointer.pointer_id => {
                    self.paint_at_position(ctx.bounds(), pointer.position);
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
                        | PixelCanvasDrag::Paint { pointer_id },
                    ) => Some(pointer_id),
                    None => None,
                };
                if active_pointer == Some(pointer.pointer_id) {
                    self.drag = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    Self::request_interaction_update(ctx);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "=" | "+" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        1.1,
                        self.document_origin(),
                    ),
                    "-" => self.viewport.zoom_around(
                        ctx.bounds(),
                        CanvasViewport::center(ctx.bounds()),
                        1.0 / 1.1,
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
        let palette = self.theme.palette;
        ctx.fill_bounds(Color::rgba(0.955, 0.965, 0.975, 1.0));
        ctx.stroke_bounds(palette.border, StrokeStyle::new(1.0));
        ctx.push_clip_rect(ctx.bounds());
        let transform = self
            .viewport
            .transform(ctx.bounds(), self.document_origin());
        paint_pixel_canvas_background(ctx, self.width, self.height, transform);
        for y in 0..self.height {
            for x in 0..self.width {
                let color = self.pixels[y * self.width + x];
                if color.alpha > 0.0 {
                    ctx.fill(
                        transform_path(
                            &Path::rect(Rect::new(x as f32, y as f32, 1.0, 1.0)),
                            transform,
                        ),
                        color,
                    );
                }
            }
        }
        if self.viewport.zoom >= 6.0 {
            paint_pixel_grid(ctx, self.width, self.height, transform);
        }
        ctx.stroke(
            transform_path(
                &Path::rect(Rect::new(0.0, 0.0, self.width as f32, self.height as f32)),
                transform,
            ),
            Color::rgba(0.08, 0.10, 0.14, 1.0),
            StrokeStyle::new(1.0),
        );
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some(self.name.clone());
        node.description = Some(format!("{} by {} pixel canvas", self.width, self.height));
        node.value = Some(SemanticsValue::Text(format!(
            "zoom {:.0}%, rotation {:.0} deg",
            self.viewport.zoom * 100.0,
            self.viewport.rotation.to_degrees()
        )));
        node.state.focused = ctx.is_focused();
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Custom("Paint".into()),
        ];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        Self::request_interaction_update(ctx);
    }
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
) {
    let visible = canvas_visible_world_rect(viewport, bounds, document_origin).inflate(80.0, 80.0);
    let transform = viewport.transform(bounds, document_origin);
    let mut x_axis = PathBuilder::new();
    x_axis
        .move_to(transform.transform_point(Point::new(visible.x(), 0.0)))
        .line_to(transform.transform_point(Point::new(visible.max_x(), 0.0)));
    ctx.stroke(
        x_axis.build(),
        Color::rgba(0.85, 0.23, 0.18, 0.55),
        StrokeStyle::new(1.0),
    );

    let mut y_axis = PathBuilder::new();
    y_axis
        .move_to(transform.transform_point(Point::new(0.0, visible.y())))
        .line_to(transform.transform_point(Point::new(0.0, visible.max_y())));
    ctx.stroke(
        y_axis.build(),
        Color::rgba(0.16, 0.52, 0.28, 0.55),
        StrokeStyle::new(1.0),
    );
}

fn paint_canvas_grid(
    ctx: &mut PaintCtx,
    viewport: CanvasViewport,
    bounds: Rect,
    document_origin: Point,
) {
    let visible = canvas_visible_world_rect(viewport, bounds, document_origin).inflate(80.0, 80.0);
    let transform = viewport.transform(bounds, document_origin);
    let step = 40.0;
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
        Color::rgba(0.40, 0.46, 0.56, 0.18),
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

fn paint_pixel_canvas_background(
    ctx: &mut PaintCtx,
    width: usize,
    height: usize,
    transform: Transform,
) {
    let light = Color::rgba(0.93, 0.94, 0.96, 1.0);
    let dark = Color::rgba(0.80, 0.82, 0.86, 1.0);
    for y in 0..height {
        for x in 0..width {
            let color = if (x + y) % 2 == 0 { light } else { dark };
            ctx.fill(
                transform_path(
                    &Path::rect(Rect::new(x as f32, y as f32, 1.0, 1.0)),
                    transform,
                ),
                color,
            );
        }
    }
}

fn paint_pixel_grid(ctx: &mut PaintCtx, width: usize, height: usize, transform: Transform) {
    let mut builder = PathBuilder::new();
    for x in 0..=width {
        let x = x as f32;
        builder
            .move_to(transform.transform_point(Point::new(x, 0.0)))
            .line_to(transform.transform_point(Point::new(x, height as f32)));
    }
    for y in 0..=height {
        let y = y as f32;
        builder
            .move_to(transform.transform_point(Point::new(0.0, y)))
            .line_to(transform.transform_point(Point::new(width as f32, y)));
    }
    ctx.stroke(
        builder.build(),
        Color::rgba(0.08, 0.10, 0.14, 0.28),
        StrokeStyle::new(1.0),
    );
}

#[cfg(test)]
mod tests {
    use super::{Canvas, CanvasShape, CanvasStroke, CanvasViewport, PixelCanvas};
    use sui_core::{
        Color, Event, Modifiers, Point, PointerButton, PointerButtons, PointerEvent,
        PointerEventKind, Rect, ScrollDelta, SemanticsRole, Size, Vector,
    };
    use sui_runtime::{Application, RenderOutput, Runtime, Widget, WindowBuilder};
    use sui_scene::{Brush, SceneCommand};

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

        let mut painted = false;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::FillPath { brush, .. } = command
                && matches!(brush, Brush::Solid(color) if color.blue > 0.8)
            {
                painted = true;
            }
        });
        assert!(painted);
        Ok(())
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
