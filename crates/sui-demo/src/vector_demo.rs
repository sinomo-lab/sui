use std::{cell::RefCell, rc::Rc};

use sui::{
    KeyState, PointerButton, PointerEventKind, ScrollDelta, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsValue, Vector, WidgetId, prelude::*,
};

#[cfg(test)]
use crate::app::default_dev_theme_reader;
use crate::app::{
    DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color, request_window_refresh,
};

pub(crate) const VECTOR_EDITOR_TAB_LABEL: &str = "Vector editor";
const VECTOR_DOCUMENT_NAME: &str = "Wave mark.svg";
pub(crate) const VECTOR_DOCUMENT_WIDTH: f32 = 720.0;
const VECTOR_DOCUMENT_HEIGHT: f32 = 480.0;
const VECTOR_RULER_EXTENT: f32 = 28.0;
const VECTOR_OBJECTS_NAME: &str = "Objects";
const VECTOR_PROPERTIES_NAME: &str = "Selection properties";
const VECTOR_TRANSFORM_NAME: &str = "Transform";
const VECTOR_APPEARANCE_NAME: &str = "Appearance";
const VECTOR_ALIGNMENT_NAME: &str = "Align controls";
const VECTOR_DOCUMENT_BAR_NAME: &str = "Vector document bar";
const VECTOR_STATUS_NAME: &str = "Vector editor status";
pub(crate) const VECTOR_FIT_VIEW_NAME: &str = "Fit view";
pub(crate) const VECTOR_ACTUAL_SIZE_NAME: &str = "Actual size";
pub(crate) const VECTOR_ZOOM_OUT_NAME: &str = "Zoom out";
pub(crate) const VECTOR_ZOOM_READOUT_NAME: &str = "Zoom level";
pub(crate) const VECTOR_ZOOM_IN_NAME: &str = "Zoom in";
const VECTOR_CENTER_X_NAME: &str = "Center X";
const VECTOR_CENTER_Y_NAME: &str = "Center Y";
pub(crate) const VECTOR_WIDTH_NAME: &str = "Width";
const VECTOR_HEIGHT_NAME: &str = "Height";
pub(crate) const VECTOR_ROTATION_NAME: &str = "Rotation";
pub(crate) const VECTOR_STROKE_WIDTH_NAME: &str = "Stroke width";
pub(crate) const VECTOR_OPACITY_NAME: &str = "Opacity";
const VECTOR_CORNER_RADIUS_NAME: &str = "Corner radius";
pub(crate) const VECTOR_FILL_RULE_NAME: &str = "Fill rule";
const VECTOR_HORIZONTAL_RULER_NAME: &str = "Vector horizontal ruler";
const VECTOR_VERTICAL_RULER_NAME: &str = "Vector vertical ruler";
const VECTOR_CANVAS_ZOOM: f32 = 1.05;
const VECTOR_HANDLE_SIZE: f32 = 9.0;
const VECTOR_ROTATE_HANDLE_OFFSET: f32 = 34.0;
pub(crate) const VECTOR_MIN_OBJECT_SIZE: f32 = 16.0;
const VECTOR_FILL_RULE_OPTIONS: [&str; 2] = ["Nonzero", "Even odd"];
const VECTOR_OBJECT_LABELS: [&str; 4] =
    ["Artboard", "Blue ellipse", "Amber ellipse", "Bezier stroke"];
const VECTOR_OBJECT_COLORS: [Color; 4] = [
    Color::rgba(0.97, 0.98, 0.99, 1.0),
    Color::rgba(0.18, 0.54, 0.86, 1.0),
    Color::rgba(0.94, 0.58, 0.16, 1.0),
    Color::rgba(0.12, 0.28, 0.84, 1.0),
];

#[derive(Clone, Debug)]
struct VectorCanvasViewportState {
    inner: Rc<RefCell<VectorCanvasViewportStateInner>>,
}

#[derive(Debug, Clone, Copy)]
struct VectorCanvasViewportStateInner {
    viewport: CanvasViewport,
    viewport_size: Size,
    pending_fit_view: u32,
    pending_actual_size: u32,
    pending_zoom_delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorViewportCommand {
    Fit,
    ActualSize,
    ZoomIn,
    ZoomOut,
}

impl VectorCanvasViewportState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(VectorCanvasViewportStateInner {
                viewport: vector_canvas_viewport(),
                viewport_size: Size::ZERO,
                pending_fit_view: 0,
                pending_actual_size: 0,
                pending_zoom_delta: 0,
            })),
        }
    }

    fn viewport(&self) -> CanvasViewport {
        self.inner.borrow().viewport
    }

    fn viewport_size(&self) -> Size {
        self.inner.borrow().viewport_size
    }

    fn viewport_snapshot(&self) -> (CanvasViewport, Size) {
        let inner = self.inner.borrow();
        (inner.viewport, inner.viewport_size)
    }

    fn request_fit_view(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_fit_view = inner.pending_fit_view.saturating_add(1);
    }

    fn request_actual_size_view(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_actual_size = inner.pending_actual_size.saturating_add(1);
    }

    fn request_zoom_in(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_zoom_delta = inner.pending_zoom_delta.saturating_add(1);
    }

    fn request_zoom_out(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.pending_zoom_delta = inner.pending_zoom_delta.saturating_sub(1);
    }

    fn take_viewport_command(&self) -> Option<VectorViewportCommand> {
        let mut inner = self.inner.borrow_mut();
        if inner.pending_fit_view > 0 {
            inner.pending_fit_view -= 1;
            return Some(VectorViewportCommand::Fit);
        }
        if inner.pending_actual_size > 0 {
            inner.pending_actual_size -= 1;
            return Some(VectorViewportCommand::ActualSize);
        }
        if inner.pending_zoom_delta > 0 {
            inner.pending_zoom_delta -= 1;
            return Some(VectorViewportCommand::ZoomIn);
        }
        if inner.pending_zoom_delta < 0 {
            inner.pending_zoom_delta += 1;
            return Some(VectorViewportCommand::ZoomOut);
        }
        None
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
}

#[derive(Clone)]
struct VectorDemoState {
    inner: Rc<RefCell<VectorDemoStateInner>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorObjectKind {
    Artboard,
    Ellipse,
    Path,
}

#[derive(Debug, Clone, Copy)]
struct VectorObject {
    kind: VectorObjectKind,
    center: Point,
    size: Size,
    rotation_degrees: f32,
    fill: Option<Color>,
    stroke: Color,
    stroke_width: f32,
    opacity: f32,
    corner_radius: f32,
}

impl VectorObject {
    const fn new(
        kind: VectorObjectKind,
        center: Point,
        size: Size,
        rotation_degrees: f32,
        fill: Option<Color>,
        stroke: Color,
        stroke_width: f32,
        opacity: f32,
        corner_radius: f32,
    ) -> Self {
        Self {
            kind,
            center,
            size,
            rotation_degrees,
            fill,
            stroke,
            stroke_width,
            opacity,
            corner_radius,
        }
    }

    fn bounds(self) -> Rect {
        Rect::new(
            self.center.x - self.size.width * 0.5,
            self.center.y - self.size.height * 0.5,
            self.size.width,
            self.size.height,
        )
    }

    fn local_to_document(self, local: Point) -> Point {
        let radians = self.rotation_degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        Point::new(
            self.center.x + (local.x * cos) - (local.y * sin),
            self.center.y + (local.x * sin) + (local.y * cos),
        )
    }

    fn document_to_local(self, point: Point) -> Point {
        let delta = point - self.center;
        let radians = self.rotation_degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        Point::new(
            (delta.x * cos) + (delta.y * sin),
            (-delta.x * sin) + (delta.y * cos),
        )
    }

    fn corners(self) -> [Point; 4] {
        let half_width = self.size.width * 0.5;
        let half_height = self.size.height * 0.5;
        [
            self.local_to_document(Point::new(-half_width, -half_height)),
            self.local_to_document(Point::new(half_width, -half_height)),
            self.local_to_document(Point::new(half_width, half_height)),
            self.local_to_document(Point::new(-half_width, half_height)),
        ]
    }

    fn top_center(self) -> Point {
        self.local_to_document(Point::new(0.0, -self.size.height * 0.5))
    }

    fn rotate_handle(self) -> Point {
        self.local_to_document(Point::new(
            0.0,
            -self.size.height * 0.5 - VECTOR_ROTATE_HANDLE_OFFSET / VECTOR_CANVAS_ZOOM,
        ))
    }

    fn contains(self, point: Point) -> bool {
        let local = self.document_to_local(point);
        let half_width = self.size.width * 0.5;
        let half_height = self.size.height * 0.5;
        match self.kind {
            VectorObjectKind::Ellipse => {
                if half_width <= 0.0 || half_height <= 0.0 {
                    false
                } else {
                    let x = local.x / half_width;
                    let y = local.y / half_height;
                    (x * x) + (y * y) <= 1.0
                }
            }
            VectorObjectKind::Path => {
                let tolerance = (self.stroke_width * 2.0).max(10.0);
                let start = Point::new(-self.size.width * 0.5, self.size.height * 0.32);
                let ctrl1 = Point::new(-self.size.width * 0.30, -self.size.height * 0.48);
                let ctrl2 = Point::new(self.size.width * 0.28, self.size.height * 0.56);
                let end = Point::new(self.size.width * 0.5, -self.size.height * 0.30);
                let mut previous = start;
                for step in 1..=48 {
                    let t = step as f32 / 48.0;
                    let current = vector_cubic_point(start, ctrl1, ctrl2, end, t);
                    if vector_distance_to_segment(local, previous, current) <= tolerance {
                        return true;
                    }
                    previous = current;
                }
                false
            }
            VectorObjectKind::Artboard => {
                local.x >= -half_width
                    && local.x <= half_width
                    && local.y >= -half_height
                    && local.y <= half_height
            }
        }
    }

    fn detail(self) -> String {
        match self.kind {
            VectorObjectKind::Artboard => {
                format!("{:.0} x {:.0} px", self.size.width, self.size.height)
            }
            VectorObjectKind::Ellipse => format!(
                "{:.0} x {:.0} px / {:.0}% fill",
                self.size.width,
                self.size.height,
                self.opacity * 100.0
            ),
            VectorObjectKind::Path => format!(
                "Stroke / {:.1} px / {:.0} deg",
                self.stroke_width, self.rotation_degrees
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorAlignment {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}

struct VectorDemoStateInner {
    selected_object: usize,
    // Visual layer order, frontmost first.
    object_order: Vec<usize>,
    object_visible: [bool; VECTOR_OBJECT_LABELS.len()],
    object_locked: [bool; VECTOR_OBJECT_LABELS.len()],
    objects: [VectorObject; VECTOR_OBJECT_LABELS.len()],
    fill_rule: usize,
}

impl VectorDemoState {
    fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(VectorDemoStateInner {
                selected_object: 1,
                object_order: vector_default_object_order(),
                object_visible: [true; VECTOR_OBJECT_LABELS.len()],
                object_locked: [true, false, false, false],
                objects: vector_default_objects(),
                fill_rule: 0,
            })),
        }
    }

    fn selected_object(&self) -> usize {
        self.inner.borrow().selected_object
    }

    fn selected_object_visual_index(&self) -> usize {
        let inner = self.inner.borrow();
        inner
            .object_order
            .iter()
            .position(|object| *object == inner.selected_object)
            .unwrap_or(inner.selected_object)
    }

    fn set_selected_object(&self, selected_object: usize) {
        self.inner.borrow_mut().selected_object =
            selected_object.min(VECTOR_OBJECT_LABELS.len().saturating_sub(1));
    }

    fn object_at_visual_index(&self, index: usize) -> usize {
        self.inner
            .borrow()
            .object_order
            .get(index)
            .copied()
            .unwrap_or(index)
    }

    fn set_selected_visual_object(&self, index: usize) {
        self.set_selected_object(self.object_at_visual_index(index));
    }

    fn reorder_objects(&self, from: usize, to: usize) {
        let mut inner = self.inner.borrow_mut();
        if from >= inner.object_order.len() || to >= inner.object_order.len() || from == to {
            return;
        }
        let object = inner.object_order.remove(from);
        inner.object_order.insert(to, object);
    }

    fn object_order(&self) -> Vec<usize> {
        self.inner.borrow().object_order.clone()
    }

    fn object_paint_order(&self) -> Vec<usize> {
        self.inner
            .borrow()
            .object_order
            .iter()
            .rev()
            .copied()
            .collect()
    }

    fn selected_object_label(&self) -> &'static str {
        VECTOR_OBJECT_LABELS
            .get(self.selected_object())
            .copied()
            .unwrap_or(VECTOR_OBJECT_LABELS[0])
    }

    fn object_visible(&self, index: usize) -> bool {
        self.inner
            .borrow()
            .object_visible
            .get(index)
            .copied()
            .unwrap_or(true)
    }

    fn set_object_visible(&self, index: usize, visible: bool) {
        if let Some(object_visible) = self.inner.borrow_mut().object_visible.get_mut(index) {
            *object_visible = visible;
        }
    }

    fn object_locked(&self, index: usize) -> bool {
        self.inner
            .borrow()
            .object_locked
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    fn set_object_locked(&self, index: usize, locked: bool) {
        if let Some(object_locked) = self.inner.borrow_mut().object_locked.get_mut(index) {
            *object_locked = locked;
        }
    }

    fn object_detail(&self, index: usize) -> String {
        self.object(index)
            .map(VectorObject::detail)
            .unwrap_or_else(|| "Vector object".to_string())
    }

    fn object(&self, index: usize) -> Option<VectorObject> {
        self.inner.borrow().objects.get(index).copied()
    }

    fn selected_object_snapshot(&self) -> VectorObject {
        self.object(self.selected_object())
            .unwrap_or_else(|| vector_default_objects()[0])
    }

    fn edit_selected_object<F>(&self, edit: F)
    where
        F: FnOnce(&mut VectorObject),
    {
        let mut inner = self.inner.borrow_mut();
        let selected = inner.selected_object;
        if inner.object_locked.get(selected).copied().unwrap_or(false) {
            return;
        }
        if let Some(object) = inner.objects.get_mut(selected) {
            edit(object);
        }
    }

    fn selected_center_x(&self) -> f32 {
        self.selected_object_snapshot().center.x
    }

    fn set_selected_center_x(&self, center_x: f32) {
        self.edit_selected_object(|object| object.center.x = center_x.clamp(-360.0, 360.0));
    }

    fn selected_center_y(&self) -> f32 {
        self.selected_object_snapshot().center.y
    }

    fn set_selected_center_y(&self, center_y: f32) {
        self.edit_selected_object(|object| object.center.y = center_y.clamp(-240.0, 240.0));
    }

    fn selected_width(&self) -> f32 {
        self.selected_object_snapshot().size.width
    }

    fn set_selected_width(&self, width: f32) {
        self.edit_selected_object(|object| {
            object.size.width = width.clamp(VECTOR_MIN_OBJECT_SIZE, VECTOR_DOCUMENT_WIDTH);
        });
    }

    fn selected_height(&self) -> f32 {
        self.selected_object_snapshot().size.height
    }

    fn set_selected_height(&self, height: f32) {
        self.edit_selected_object(|object| {
            object.size.height = height.clamp(VECTOR_MIN_OBJECT_SIZE, VECTOR_DOCUMENT_HEIGHT);
        });
    }

    fn selected_rotation(&self) -> f32 {
        self.selected_object_snapshot().rotation_degrees
    }

    fn set_selected_rotation(&self, rotation_degrees: f32) {
        self.edit_selected_object(|object| {
            object.rotation_degrees = normalize_vector_rotation(rotation_degrees);
        });
    }

    fn set_selected_transform(&self, center: Point, size: Size, rotation_degrees: f32) {
        self.edit_selected_object(|object| {
            object.center = Point::new(
                center
                    .x
                    .clamp(-VECTOR_DOCUMENT_WIDTH * 0.5, VECTOR_DOCUMENT_WIDTH * 0.5),
                center
                    .y
                    .clamp(-VECTOR_DOCUMENT_HEIGHT * 0.5, VECTOR_DOCUMENT_HEIGHT * 0.5),
            );
            object.size = Size::new(
                size.width
                    .clamp(VECTOR_MIN_OBJECT_SIZE, VECTOR_DOCUMENT_WIDTH),
                size.height
                    .clamp(VECTOR_MIN_OBJECT_SIZE, VECTOR_DOCUMENT_HEIGHT),
            );
            object.rotation_degrees = normalize_vector_rotation(rotation_degrees);
        });
    }

    fn align_selected(&self, alignment: VectorAlignment) {
        let artboard = self
            .object(0)
            .unwrap_or_else(|| vector_default_objects()[0]);
        self.edit_selected_object(|object| {
            let artboard_bounds = artboard.bounds();
            match alignment {
                VectorAlignment::Left => {
                    object.center.x = artboard_bounds.x() + object.size.width * 0.5;
                }
                VectorAlignment::Center => {
                    object.center.x = artboard.center.x;
                }
                VectorAlignment::Right => {
                    object.center.x = artboard_bounds.max_x() - object.size.width * 0.5;
                }
                VectorAlignment::Top => {
                    object.center.y = artboard_bounds.y() + object.size.height * 0.5;
                }
                VectorAlignment::Middle => {
                    object.center.y = artboard.center.y;
                }
                VectorAlignment::Bottom => {
                    object.center.y = artboard_bounds.max_y() - object.size.height * 0.5;
                }
            }
        });
    }

    fn reset_selected_transform(&self) {
        let selected = self.selected_object();
        let defaults = vector_default_objects();
        self.edit_selected_object(|object| {
            if let Some(default_object) = defaults.get(selected) {
                object.center = default_object.center;
                object.size = default_object.size;
                object.rotation_degrees = default_object.rotation_degrees;
            }
        });
    }

    fn hit_test(&self, point: Point) -> Option<usize> {
        let inner = self.inner.borrow();
        inner.object_order.iter().copied().find_map(|index| {
            let object = inner.objects.get(index)?;
            inner
                .object_visible
                .get(index)
                .copied()
                .unwrap_or(true)
                .then_some(())
                .filter(|_| object.contains(point))
                .map(|_| index)
        })
    }

    fn stroke_width(&self) -> f32 {
        self.selected_object_snapshot().stroke_width
    }

    fn set_stroke_width(&self, stroke_width: f32) {
        self.edit_selected_object(|object| {
            object.stroke_width = stroke_width.clamp(0.5, 24.0);
        });
    }

    fn opacity(&self) -> f32 {
        self.selected_object_snapshot().opacity
    }

    fn set_opacity(&self, opacity: f32) {
        self.edit_selected_object(|object| {
            object.opacity = opacity.clamp(0.0, 1.0);
        });
    }

    fn corner_radius(&self) -> f32 {
        self.selected_object_snapshot().corner_radius
    }

    fn set_corner_radius(&self, corner_radius: f32) {
        self.edit_selected_object(|object| {
            object.corner_radius = corner_radius.clamp(0.0, 96.0);
        });
    }

    fn fill_rule(&self) -> usize {
        self.inner.borrow().fill_rule
    }

    fn set_fill_rule(&self, fill_rule: usize) {
        self.inner.borrow_mut().fill_rule =
            fill_rule.min(VECTOR_FILL_RULE_OPTIONS.len().saturating_sub(1));
    }

    fn fill_rule_label(&self) -> &'static str {
        VECTOR_FILL_RULE_OPTIONS
            .get(self.fill_rule())
            .copied()
            .unwrap_or(VECTOR_FILL_RULE_OPTIONS[0])
    }

    fn selected_object_color(&self) -> Color {
        let object = self.selected_object_snapshot();
        object.fill.unwrap_or(object.stroke)
    }
}

fn vector_default_objects() -> [VectorObject; VECTOR_OBJECT_LABELS.len()] {
    [
        VectorObject::new(
            VectorObjectKind::Artboard,
            Point::ZERO,
            Size::new(VECTOR_DOCUMENT_WIDTH, VECTOR_DOCUMENT_HEIGHT),
            0.0,
            Some(Color::rgba(1.0, 1.0, 1.0, 1.0)),
            Color::rgba(0.12, 0.16, 0.22, 1.0),
            1.2,
            1.0,
            0.0,
        ),
        VectorObject::new(
            VectorObjectKind::Ellipse,
            Point::new(-116.0, -42.0),
            Size::new(124.0, 96.0),
            -12.0,
            Some(Color::rgba(0.18, 0.54, 0.86, 1.0)),
            Color::rgba(0.08, 0.22, 0.42, 1.0),
            3.0,
            0.78,
            18.0,
        ),
        VectorObject::new(
            VectorObjectKind::Ellipse,
            Point::new(92.0, 44.0),
            Size::new(142.0, 112.0),
            8.0,
            Some(Color::rgba(0.94, 0.58, 0.16, 1.0)),
            Color::rgba(0.42, 0.22, 0.06, 1.0),
            2.0,
            0.72,
            22.0,
        ),
        VectorObject::new(
            VectorObjectKind::Path,
            Point::new(10.0, 12.0),
            Size::new(320.0, 220.0),
            0.0,
            None,
            Color::rgba(0.12, 0.28, 0.84, 1.0),
            3.0,
            1.0,
            0.0,
        ),
    ]
}

fn vector_default_object_order() -> Vec<usize> {
    (0..VECTOR_OBJECT_LABELS.len()).rev().collect()
}

fn normalize_vector_rotation(rotation_degrees: f32) -> f32 {
    let mut rotation = rotation_degrees;
    while rotation > 180.0 {
        rotation -= 360.0;
    }
    while rotation < -180.0 {
        rotation += 360.0;
    }
    rotation
}

fn vector_cubic_point(start: Point, ctrl1: Point, ctrl2: Point, end: Point, t: f32) -> Point {
    let t = t.clamp(0.0, 1.0);
    let one_minus = 1.0 - t;
    let a = one_minus * one_minus * one_minus;
    let b = 3.0 * one_minus * one_minus * t;
    let c = 3.0 * one_minus * t * t;
    let d = t * t * t;
    Point::new(
        (start.x * a) + (ctrl1.x * b) + (ctrl2.x * c) + (end.x * d),
        (start.y * a) + (ctrl1.y * b) + (ctrl2.y * c) + (end.y * d),
    )
}

fn vector_distance_to_segment(point: Point, start: Point, end: Point) -> f32 {
    let segment = end - start;
    let length_squared = (segment.x * segment.x) + (segment.y * segment.y);
    if length_squared <= f32::EPSILON {
        let delta = point - start;
        return ((delta.x * delta.x) + (delta.y * delta.y)).sqrt();
    }
    let offset = point - start;
    let t = ((offset.x * segment.x) + (offset.y * segment.y)) / length_squared;
    let t = t.clamp(0.0, 1.0);
    let projection = Point::new(start.x + segment.x * t, start.y + segment.y * t);
    let delta = point - projection;
    ((delta.x * delta.x) + (delta.y * delta.y)).sqrt()
}

pub(crate) fn build_vector_editor_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let state = VectorDemoState::new();
    let viewport_state = VectorCanvasViewportState::new();
    Background::new(
        Color::rgba(0.925, 0.940, 0.958, 1.0),
        StatusBarHost::new(
            SplitView::horizontal(
                build_vector_canvas_stage(
                    state.clone(),
                    viewport_state.clone(),
                    Rc::clone(&theme_reader),
                ),
                build_vector_properties_panel(state.clone(), Rc::clone(&theme_reader)),
            )
            .name("Vector workspace")
            .ratio(0.76)
            .min_first(520.0)
            .min_second(292.0)
            .divider_thickness(4.0),
            build_vector_status_bar(state, viewport_state, Rc::clone(&theme_reader)),
        ),
    )
    .brush_when(move || theme_reader().palette.surface)
}

fn vector_document_size() -> Size {
    Size::new(VECTOR_DOCUMENT_WIDTH, VECTOR_DOCUMENT_HEIGHT)
}

fn vector_canvas_viewport() -> CanvasViewport {
    CanvasViewport::new().zoom(VECTOR_CANVAS_ZOOM)
}

fn vector_command_group(name: &'static str, theme_reader: &DevThemeReader) -> CommandGroup {
    CommandGroup::horizontal(name)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .padding(Insets::all(2.0))
        .spacing(2.0)
        .corner_radius(6.0)
}

fn build_vector_status_bar(
    state: VectorDemoState,
    viewport_state: VectorCanvasViewportState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let object_state = state.clone();
    let zoom_state = viewport_state;
    let stroke_state = state.clone();
    let opacity_state = state.clone();
    let fill_state = state;
    StatusBar::new()
        .name(VECTOR_STATUS_NAME)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .height(28.0)
        .segment(StatusBarSegment::new("Select / edit").min_width(120.0))
        .segment(
            StatusBarSegment::dynamic("Zoom --", move || vector_zoom_status_text(&zoom_state))
                .min_width(92.0),
        )
        .segment(
            StatusBarSegment::dynamic("Object Blue ellipse", move || {
                format!("Object {}", object_state.selected_object_label())
            })
            .min_width(176.0),
        )
        .segment(
            StatusBarSegment::dynamic("Stroke 3 px", move || {
                format!("Stroke {:.1} px", stroke_state.stroke_width())
            })
            .min_width(120.0),
        )
        .segment(
            StatusBarSegment::dynamic("Opacity 78%", move || {
                format!("Opacity {:.0}%", opacity_state.opacity() * 100.0)
            })
            .min_width(126.0),
        )
        .segment(
            StatusBarSegment::dynamic("Fill Nonzero", move || {
                format!("Fill {}", fill_state.fill_rule_label())
            })
            .expand(true),
        )
}

fn vector_zoom_status_text(state: &VectorCanvasViewportState) -> String {
    let viewport_size = state.viewport_size();
    if viewport_size.width <= 0.0 || viewport_size.height <= 0.0 {
        "Zoom --".to_string()
    } else {
        format!("Zoom {:.0}%", state.viewport().zoom * 100.0)
    }
}

fn build_vector_canvas_stage(
    state: VectorDemoState,
    viewport_state: VectorCanvasViewportState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let horizontal_ruler_state = viewport_state.clone();
    let vertical_ruler_state = viewport_state.clone();
    Background::new(
        Color::rgba(0.890, 0.905, 0.925, 1.0),
        Stack::vertical()
            .alignment(Alignment::Stretch)
            .with_child(build_vector_document_bar(
                viewport_state.clone(),
                Rc::clone(&theme_reader),
            ))
            .with_child(Padding::all(
                12.0,
                Stack::vertical()
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Stack::horizontal()
                            .with_child(vector_ruler_corner(Rc::clone(&theme_reader)))
                            .with_child(
                                CanvasRuler::horizontal(
                                    VECTOR_HORIZONTAL_RULER_NAME,
                                    vector_document_size(),
                                )
                                .theme_when(clone_dev_theme_reader(&theme_reader))
                                .extent(VECTOR_RULER_EXTENT)
                                .viewport_when(move || horizontal_ruler_state.viewport_snapshot()),
                            ),
                    )
                    .with_child(
                        Stack::horizontal()
                            .alignment(Alignment::Stretch)
                            .with_child(
                                CanvasRuler::vertical(
                                    VECTOR_VERTICAL_RULER_NAME,
                                    vector_document_size(),
                                )
                                .theme_when(clone_dev_theme_reader(&theme_reader))
                                .extent(VECTOR_RULER_EXTENT)
                                .viewport_when(move || vertical_ruler_state.viewport_snapshot()),
                            )
                            .with_child(VectorEditorCanvas::new(
                                state,
                                viewport_state,
                                Rc::clone(&theme_reader),
                            )),
                    ),
            )),
    )
    .brush_when(move || theme_reader().palette.surface_raised)
}

fn build_vector_document_bar(
    viewport_state: VectorCanvasViewportState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let fit_state = viewport_state.clone();
    let actual_size_state = viewport_state.clone();
    let zoom_out_state = viewport_state.clone();
    let zoom_reader_state = viewport_state.clone();
    let zoom_in_state = viewport_state;
    Toolbar::horizontal()
        .name(VECTOR_DOCUMENT_BAR_NAME)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .extent(34.0)
        .padding(Insets::all(6.0))
        .spacing(8.0)
        .with_child(
            Label::new(VECTOR_DOCUMENT_NAME)
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.sm,
                    theme_reader().palette.text,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(Separator::vertical().length(18.0))
        .with_child(
            Label::new(format!(
                "{:.0} x {:.0} px",
                VECTOR_DOCUMENT_WIDTH, VECTOR_DOCUMENT_HEIGHT
            ))
            .style(dev_text_style(
                theme_reader(),
                theme_reader().text.xs,
                theme_reader().palette.text_muted,
            ))
            .color_when(dev_theme_color(&theme_reader, |theme| {
                theme.palette.text_muted
            })),
        )
        .with_child(
            Label::new("SVG / Display P3")
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.xs,
                    theme_reader().palette.text_muted,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
        .with_child(Separator::vertical().length(18.0))
        .with_child(
            Label::new("1 artboard / 3 objects")
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.xs,
                    theme_reader().palette.text_muted,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
        .with_child(Separator::vertical().length(18.0))
        .with_child(
            vector_command_group("Vector view commands", &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::FitView, VECTOR_FIT_VIEW_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(24.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            fit_state.request_fit_view();
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::ActualSize, VECTOR_ACTUAL_SIZE_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(24.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            actual_size_state.request_actual_size_view();
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
        .with_child(
            vector_command_group("Vector zoom controls", &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::Remove, VECTOR_ZOOM_OUT_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(24.0)
                        .icon_size(12.0)
                        .on_press_with_ctx(move |ctx| {
                            zoom_out_state.request_zoom_out();
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    SizedBox::new().width(78.0).with_child(
                        Label::dynamic("Zoom --", move || {
                            vector_zoom_status_text(&zoom_reader_state)
                        })
                        .semantic_name(VECTOR_ZOOM_READOUT_NAME)
                        .style(dev_text_style(
                            theme_reader(),
                            theme_reader().text.xs,
                            theme_reader().palette.text,
                        ))
                        .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
                    ),
                )
                .with_child(
                    IconButton::new(IconGlyph::Add, VECTOR_ZOOM_IN_NAME)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(24.0)
                        .icon_size(12.0)
                        .on_press_with_ctx(move |ctx| {
                            zoom_in_state.request_zoom_in();
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
}

fn vector_ruler_corner(theme_reader: DevThemeReader) -> impl Widget {
    Background::new(
        Color::rgba(0.925, 0.936, 0.950, 1.0),
        SizedBox::new()
            .width(VECTOR_RULER_EXTENT)
            .height(VECTOR_RULER_EXTENT),
    )
    .brush_when(move || theme_reader().palette.surface_raised)
}

fn build_vector_properties_panel(
    state: VectorDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    DockPanel::new(
        VECTOR_PROPERTIES_NAME,
        ScrollView::vertical(Padding::all(
            8.0,
            Stack::vertical()
                .spacing(8.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    PanelSection::new(
                        VECTOR_OBJECTS_NAME,
                        build_vector_objects_panel(state.clone(), Rc::clone(&theme_reader)),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    PanelSection::new(
                        VECTOR_TRANSFORM_NAME,
                        build_vector_transform_panel(state.clone(), Rc::clone(&theme_reader)),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    PanelSection::new(
                        VECTOR_APPEARANCE_NAME,
                        build_vector_appearance_panel(state.clone(), Rc::clone(&theme_reader)),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    PanelSection::new(
                        VECTOR_ALIGNMENT_NAME,
                        build_vector_alignment_panel(state.clone(), Rc::clone(&theme_reader)),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader)),
                ),
        ))
        .name("Vector controls"),
    )
    .name(VECTOR_PROPERTIES_NAME)
    .theme_when(clone_dev_theme_reader(&theme_reader))
    .padding(Insets::ZERO)
}

fn build_vector_objects_panel(state: VectorDemoState, theme_reader: DevThemeReader) -> impl Widget {
    let selected_state = state.clone();
    let visibility_state = state.clone();
    let lock_state = state.clone();
    let reorder_state = state.clone();
    let layers = state
        .object_order()
        .into_iter()
        .map(|index| vector_object_layer_item(&state, index))
        .collect::<Vec<_>>();

    SizedBox::new().width(284.0).height(204.0).with_child(
        LayerList::new(VECTOR_OBJECTS_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .layers(layers)
            .selected(state.selected_object_visual_index())
            .selected_when(move || Some(selected_state.selected_object_visual_index()))
            .row_height(46.0)
            .on_select_with_ctx(move |ctx, index, _| {
                state.set_selected_visual_object(index);
                request_window_refresh(ctx, true);
            })
            .on_visibility_change_with_ctx(move |ctx, index, visible| {
                let object = visibility_state.object_at_visual_index(index);
                visibility_state.set_object_visible(object, visible);
                request_window_refresh(ctx, true);
            })
            .on_lock_change_with_ctx(move |ctx, index, locked| {
                let object = lock_state.object_at_visual_index(index);
                lock_state.set_object_locked(object, locked);
                request_window_refresh(ctx, true);
            })
            .on_reorder_with_ctx(move |ctx, change| {
                reorder_state.reorder_objects(change.from, change.to);
                request_window_refresh(ctx, true);
            }),
    )
}

fn vector_object_layer_item(state: &VectorDemoState, index: usize) -> LayerListItem {
    let detail_state = state.clone();
    let visible_state = state.clone();
    let locked_state = state.clone();
    LayerListItem::new(VECTOR_OBJECT_LABELS[index])
        .detail_when(move || detail_state.object_detail(index))
        .thumbnail(VECTOR_OBJECT_COLORS[index])
        .visible_when(move || visible_state.object_visible(index))
        .locked_when(move || locked_state.object_locked(index))
}

fn build_vector_transform_panel(
    state: VectorDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let x_reader_state = state.clone();
    let x_change_state = state.clone();
    let x_label_state = state.clone();
    let y_reader_state = state.clone();
    let y_change_state = state.clone();
    let y_label_state = state.clone();
    let width_reader_state = state.clone();
    let width_change_state = state.clone();
    let width_label_state = state.clone();
    let height_reader_state = state.clone();
    let height_change_state = state.clone();
    let height_label_state = state.clone();
    let rotation_reader_state = state.clone();
    let rotation_change_state = state.clone();
    let rotation_label_state = state.clone();
    let reset_state = state;

    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_CENTER_X_NAME,
            Slider::new(VECTOR_CENTER_X_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(-360.0, 360.0)
                .step(1.0)
                .value(x_reader_state.selected_center_x() as f64)
                .value_when(move || x_reader_state.selected_center_x() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    x_change_state.set_selected_center_x(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0}", x_label_state.selected_center_x()),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_CENTER_Y_NAME,
            Slider::new(VECTOR_CENTER_Y_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(-240.0, 240.0)
                .step(1.0)
                .value(y_reader_state.selected_center_y() as f64)
                .value_when(move || y_reader_state.selected_center_y() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    y_change_state.set_selected_center_y(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0}", y_label_state.selected_center_y()),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_WIDTH_NAME,
            Slider::new(VECTOR_WIDTH_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(
                    f64::from(VECTOR_MIN_OBJECT_SIZE),
                    f64::from(VECTOR_DOCUMENT_WIDTH),
                )
                .step(1.0)
                .value(width_reader_state.selected_width() as f64)
                .value_when(move || width_reader_state.selected_width() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    width_change_state.set_selected_width(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0}", width_label_state.selected_width()),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_HEIGHT_NAME,
            Slider::new(VECTOR_HEIGHT_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(
                    f64::from(VECTOR_MIN_OBJECT_SIZE),
                    f64::from(VECTOR_DOCUMENT_HEIGHT),
                )
                .step(1.0)
                .value(height_reader_state.selected_height() as f64)
                .value_when(move || height_reader_state.selected_height() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    height_change_state.set_selected_height(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0}", height_label_state.selected_height()),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_ROTATION_NAME,
            Slider::new(VECTOR_ROTATION_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(-180.0, 180.0)
                .step(1.0)
                .value(rotation_reader_state.selected_rotation() as f64)
                .value_when(move || rotation_reader_state.selected_rotation() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    rotation_change_state.set_selected_rotation(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0} deg", rotation_label_state.selected_rotation()),
        ))
        .with_child(
            vector_command_group("Transform commands", &theme_reader).with_child(
                IconButton::new(IconGlyph::Restore, "Reset transform")
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .size(28.0)
                    .icon_size(14.0)
                    .on_press_with_ctx(move |ctx| {
                        reset_state.reset_selected_transform();
                        request_window_refresh(ctx, true);
                    }),
            ),
        )
}

fn build_vector_appearance_panel(
    state: VectorDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let color_state = state.clone();
    let stroke_reader_state = state.clone();
    let stroke_change_state = state.clone();
    let stroke_label_state = state.clone();
    let opacity_reader_state = state.clone();
    let opacity_change_state = state.clone();
    let opacity_label_state = state.clone();
    let corner_reader_state = state.clone();
    let corner_change_state = state.clone();
    let corner_label_state = state.clone();
    let fill_reader_state = state.clone();
    let fill_change_state = state;

    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(vector_property_row_with_width(
            &theme_reader,
            "Object color",
            104.0,
            ColorSwatch::new("Selected object color", color_state.selected_object_color())
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .size(Size::new(104.0, 32.0))
                .color_when(move || color_state.selected_object_color())
                .read_only(),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_STROKE_WIDTH_NAME,
            Slider::new(VECTOR_STROKE_WIDTH_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.5, 24.0)
                .step(0.5)
                .value(stroke_reader_state.stroke_width() as f64)
                .value_when(move || stroke_reader_state.stroke_width() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    stroke_change_state.set_stroke_width(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.1}", stroke_label_state.stroke_width()),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_OPACITY_NAME,
            Slider::new(VECTOR_OPACITY_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.0, 1.0)
                .step(0.01)
                .value(opacity_reader_state.opacity() as f64)
                .value_when(move || opacity_reader_state.opacity() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    opacity_change_state.set_opacity(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0}%", opacity_label_state.opacity() * 100.0),
        ))
        .with_child(vector_property_slider_row(
            &theme_reader,
            VECTOR_CORNER_RADIUS_NAME,
            Slider::new(VECTOR_CORNER_RADIUS_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .range(0.0, 96.0)
                .step(1.0)
                .value(corner_reader_state.corner_radius() as f64)
                .value_when(move || corner_reader_state.corner_radius() as f64)
                .on_change_with_ctx(move |ctx, value| {
                    corner_change_state.set_corner_radius(value as f32);
                    request_window_refresh(ctx, true);
                }),
            move || format!("{:.0}", corner_label_state.corner_radius()),
        ))
        .with_child(vector_property_row(
            &theme_reader,
            VECTOR_FILL_RULE_NAME,
            Select::new(VECTOR_FILL_RULE_NAME)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .options(VECTOR_FILL_RULE_OPTIONS)
                .selected(fill_reader_state.fill_rule())
                .selected_when(move || Some(fill_reader_state.fill_rule()))
                .on_change_with_ctx(move |ctx, index, _| {
                    fill_change_state.set_fill_rule(index);
                    request_window_refresh(ctx, true);
                }),
        ))
}

fn build_vector_alignment_panel(
    state: VectorDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let left_state = state.clone();
    let center_state = state.clone();
    let right_state = state.clone();
    let top_state = state.clone();
    let middle_state = state.clone();
    let bottom_state = state;

    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            vector_command_group("Horizontal align", &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::ChevronLeft, "Align left")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            left_state.align_selected(VectorAlignment::Left);
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::Maximize, "Align center")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            center_state.align_selected(VectorAlignment::Center);
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::ChevronRight, "Align right")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            right_state.align_selected(VectorAlignment::Right);
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
        .with_child(
            vector_command_group("Vertical align", &theme_reader)
                .with_child(
                    IconButton::new(IconGlyph::ChevronUp, "Align top")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            top_state.align_selected(VectorAlignment::Top);
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::Restore, "Align middle")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            middle_state.align_selected(VectorAlignment::Middle);
                            request_window_refresh(ctx, true);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::ChevronDown, "Align bottom")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .size(28.0)
                        .icon_size(14.0)
                        .on_press_with_ctx(move |ctx| {
                            bottom_state.align_selected(VectorAlignment::Bottom);
                            request_window_refresh(ctx, true);
                        }),
                ),
        )
}

fn vector_property_row<W>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    control: W,
) -> PropertyRow
where
    W: Widget + 'static,
{
    PropertyRow::new(label, control)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .inline()
        .label_width(104.0)
}

fn vector_property_row_with_width<W>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    width: f32,
    control: W,
) -> PropertyRow
where
    W: Widget + 'static,
{
    vector_property_row(theme_reader, label, control).control_width(width)
}

fn vector_property_slider_row<W, F>(
    theme_reader: &DevThemeReader,
    label: &'static str,
    control: W,
    value_reader: F,
) -> PropertyRow
where
    W: Widget + 'static,
    F: Fn() -> String + 'static,
{
    PropertyRow::new(
        label,
        Stack::horizontal()
            .spacing(6.0)
            .alignment(Alignment::Center)
            .with_child(control)
            .with_child(
                SizedBox::new().width(44.0).height(28.0).with_child(
                    Label::dynamic("", value_reader)
                        .style(dev_text_style(
                            theme_reader(),
                            theme_reader().text.xs,
                            theme_reader().palette.text_muted,
                        ))
                        .color_when(dev_theme_color(theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                ),
            ),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .inline()
    .label_width(82.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorResizeHandle {
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VectorCanvasDragMode {
    Move,
    Resize(VectorResizeHandle),
    Rotate,
}

#[derive(Debug, Clone, Copy)]
struct VectorCanvasDrag {
    pointer_id: u64,
    mode: VectorCanvasDragMode,
    start_document: Point,
    start_object: VectorObject,
    start_angle: f32,
}

#[derive(Debug, Clone, Copy)]
struct VectorCanvasPanDrag {
    pointer_id: u64,
    last_position: Point,
}

struct VectorEditorCanvas {
    state: VectorDemoState,
    viewport_state: VectorCanvasViewportState,
    viewport: CanvasViewport,
    theme_reader: DevThemeReader,
    drag: Option<VectorCanvasDrag>,
    pan_drag: Option<VectorCanvasPanDrag>,
    hovered_object: Option<usize>,
    hovered_handle: Option<VectorCanvasDragMode>,
}

impl VectorEditorCanvas {
    fn new(
        state: VectorDemoState,
        viewport_state: VectorCanvasViewportState,
        theme_reader: DevThemeReader,
    ) -> Self {
        let viewport = viewport_state.viewport();
        Self {
            state,
            viewport_state,
            viewport,
            theme_reader,
            drag: None,
            pan_drag: None,
            hovered_object: None,
            hovered_handle: None,
        }
    }

    fn theme(&self) -> DefaultTheme {
        (self.theme_reader)()
    }

    fn screen_center(bounds: Rect) -> Point {
        Point::new(
            bounds.x() + bounds.width() * 0.5,
            bounds.y() + bounds.height() * 0.5,
        )
    }

    fn viewport_center(bounds: Rect, viewport: CanvasViewport) -> Point {
        Self::screen_center(bounds) + viewport.pan
    }

    #[cfg(test)]
    fn document_to_screen(bounds: Rect, point: Point) -> Point {
        Self::document_to_screen_with_viewport(bounds, vector_canvas_viewport(), point)
    }

    fn document_to_screen_with_viewport(
        bounds: Rect,
        viewport: CanvasViewport,
        point: Point,
    ) -> Point {
        let center = Self::viewport_center(bounds, viewport);
        let scaled = Vector::new(point.x * viewport.zoom, point.y * viewport.zoom);
        let (sin, cos) = viewport.rotation.sin_cos();
        let rotated = Vector::new(
            (scaled.x * cos) - (scaled.y * sin),
            (scaled.x * sin) + (scaled.y * cos),
        );
        center + rotated
    }

    fn screen_to_document_with_viewport(
        bounds: Rect,
        viewport: CanvasViewport,
        point: Point,
    ) -> Point {
        let center = Self::viewport_center(bounds, viewport);
        let relative = point - center;
        let (sin, cos) = (-viewport.rotation).sin_cos();
        let rotated = Vector::new(
            (relative.x * cos) - (relative.y * sin),
            (relative.x * sin) + (relative.y * cos),
        );
        Point::new(
            rotated.x / viewport.zoom.max(0.01),
            rotated.y / viewport.zoom.max(0.01),
        )
    }

    fn to_screen(&self, bounds: Rect, point: Point) -> Point {
        Self::document_to_screen_with_viewport(bounds, self.viewport, point)
    }

    fn from_screen(&self, bounds: Rect, point: Point) -> Point {
        Self::screen_to_document_with_viewport(bounds, self.viewport, point)
    }

    fn publish_viewport_state(&self, bounds: Rect) -> bool {
        self.viewport_state
            .set_viewport_state(self.viewport, bounds.size)
    }

    fn fit_view_to_bounds(&mut self, bounds: Rect) -> bool {
        if bounds.is_empty() {
            return false;
        }
        let padding = self.theme().metrics.pixel_canvas_fit_padding;
        let (sin, cos) = self.viewport.rotation.sin_cos();
        let document_size = vector_document_size();
        let rotated_width = (document_size.width * cos.abs()) + (document_size.height * sin.abs());
        let rotated_height = (document_size.width * sin.abs()) + (document_size.height * cos.abs());
        let available_width = (bounds.width() - (padding * 2.0)).max(1.0);
        let available_height = (bounds.height() - (padding * 2.0)).max(1.0);
        let next = CanvasViewport {
            pan: Vector::ZERO,
            zoom: (available_width / rotated_width.max(1.0))
                .min(available_height / rotated_height.max(1.0))
                .max(0.01),
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

    fn zoom_view_around(&mut self, bounds: Rect, anchor: Point, factor: f32) -> bool {
        if bounds.is_empty() {
            return false;
        }
        let previous = self.viewport;
        let before = Self::screen_to_document_with_viewport(bounds, self.viewport, anchor);
        self.viewport.zoom = (self.viewport.zoom * factor.max(0.01)).max(0.01);
        let after = Self::document_to_screen_with_viewport(bounds, self.viewport, before);
        self.viewport.pan += anchor - after;
        self.viewport != previous
    }

    fn apply_pending_viewport_commands(&mut self, bounds: Rect) -> bool {
        let mut changed = false;
        let zoom_step = self.theme().metrics.pixel_canvas_zoom_step;
        while let Some(command) = self.viewport_state.take_viewport_command() {
            changed |= match command {
                VectorViewportCommand::Fit => self.fit_view_to_bounds(bounds),
                VectorViewportCommand::ActualSize => self.set_actual_size_view(),
                VectorViewportCommand::ZoomIn => {
                    self.zoom_view_around(bounds, Self::screen_center(bounds), zoom_step)
                }
                VectorViewportCommand::ZoomOut => {
                    self.zoom_view_around(bounds, Self::screen_center(bounds), 1.0 / zoom_step)
                }
            };
        }
        changed
    }

    fn resize_handle_rect(point: Point) -> Rect {
        Rect::new(
            point.x - VECTOR_HANDLE_SIZE * 0.5,
            point.y - VECTOR_HANDLE_SIZE * 0.5,
            VECTOR_HANDLE_SIZE,
            VECTOR_HANDLE_SIZE,
        )
    }

    fn selection_handle_at(&self, bounds: Rect, point: Point) -> Option<VectorCanvasDragMode> {
        let object = self.state.selected_object_snapshot();
        let corners = object
            .corners()
            .map(|corner| self.to_screen(bounds, corner));
        let resize_handles = [
            (VectorResizeHandle::NorthWest, corners[0]),
            (VectorResizeHandle::NorthEast, corners[1]),
            (VectorResizeHandle::SouthEast, corners[2]),
            (VectorResizeHandle::SouthWest, corners[3]),
        ];
        for (handle, center) in resize_handles {
            if Self::resize_handle_rect(center).contains(point) {
                return Some(VectorCanvasDragMode::Resize(handle));
            }
        }

        let rotate_handle = self.to_screen(bounds, object.rotate_handle());
        if Path::circle(rotate_handle, VECTOR_HANDLE_SIZE * 0.75)
            .bounds()
            .inflate(VECTOR_HANDLE_SIZE * 0.5, VECTOR_HANDLE_SIZE * 0.5)
            .contains(point)
        {
            return Some(VectorCanvasDragMode::Rotate);
        }

        None
    }

    fn update_hover(&mut self, bounds: Rect, position: Point) {
        let handle = self.selection_handle_at(bounds, position);
        let document = self.from_screen(bounds, position);
        let object = self.state.hit_test(document);
        if self.hovered_handle != handle || self.hovered_object != object {
            self.hovered_handle = handle;
            self.hovered_object = object;
        }
    }

    fn apply_drag(&self, drag: VectorCanvasDrag, document: Point) {
        match drag.mode {
            VectorCanvasDragMode::Move => {
                let delta = document - drag.start_document;
                self.state.set_selected_transform(
                    drag.start_object.center + delta,
                    drag.start_object.size,
                    drag.start_object.rotation_degrees,
                );
            }
            VectorCanvasDragMode::Resize(handle) => {
                let local = drag.start_object.document_to_local(document);
                let (sign_x, sign_y) = match handle {
                    VectorResizeHandle::NorthWest => (-1.0, -1.0),
                    VectorResizeHandle::NorthEast => (1.0, -1.0),
                    VectorResizeHandle::SouthEast => (1.0, 1.0),
                    VectorResizeHandle::SouthWest => (-1.0, 1.0),
                };
                let width = (local.x * sign_x * 2.0)
                    .abs()
                    .clamp(VECTOR_MIN_OBJECT_SIZE, VECTOR_DOCUMENT_WIDTH);
                let height = (local.y * sign_y * 2.0)
                    .abs()
                    .clamp(VECTOR_MIN_OBJECT_SIZE, VECTOR_DOCUMENT_HEIGHT);
                self.state.set_selected_transform(
                    drag.start_object.center,
                    Size::new(width, height),
                    drag.start_object.rotation_degrees,
                );
            }
            VectorCanvasDragMode::Rotate => {
                let center = drag.start_object.center;
                let angle = (document.y - center.y).atan2(document.x - center.x);
                let delta_degrees = (angle - drag.start_angle).to_degrees();
                self.state.set_selected_transform(
                    center,
                    drag.start_object.size,
                    drag.start_object.rotation_degrees + delta_degrees,
                );
            }
        }
    }

    fn request_edit_update(ctx: &mut EventCtx) {
        request_window_refresh(ctx, true);
    }

    fn paint_grid(
        ctx: &mut PaintCtx,
        bounds: Rect,
        viewport: CanvasViewport,
        theme: &DefaultTheme,
    ) {
        let minor = (24.0 * viewport.zoom).max(4.0);
        let center = Self::viewport_center(bounds, viewport);
        let grid_minor = theme.palette.text_muted.with_alpha(0.18);
        let grid_major = theme.palette.text_muted.with_alpha(0.30);

        let mut x = center.x;
        while x > bounds.x() {
            x -= minor;
        }
        let mut column = ((x - center.x) / minor).round() as i32;
        while x < bounds.max_x() {
            let is_major = column.rem_euclid(4) == 0;
            ctx.stroke(
                vector_line_path(Point::new(x, bounds.y()), Point::new(x, bounds.max_y())),
                if is_major { grid_major } else { grid_minor },
                StrokeStyle::new(1.0),
            );
            x += minor;
            column += 1;
        }

        let mut y = center.y;
        while y > bounds.y() {
            y -= minor;
        }
        let mut row = ((y - center.y) / minor).round() as i32;
        while y < bounds.max_y() {
            let is_major = row.rem_euclid(4) == 0;
            ctx.stroke(
                vector_line_path(Point::new(bounds.x(), y), Point::new(bounds.max_x(), y)),
                if is_major { grid_major } else { grid_minor },
                StrokeStyle::new(1.0),
            );
            y += minor;
            row += 1;
        }
    }

    fn paint_object(&self, ctx: &mut PaintCtx, bounds: Rect, object: VectorObject) {
        match object.kind {
            VectorObjectKind::Artboard => self.paint_artboard(ctx, bounds, object),
            VectorObjectKind::Ellipse => self.paint_ellipse(ctx, bounds, object),
            VectorObjectKind::Path => self.paint_bezier(ctx, bounds, object),
        }
    }

    fn paint_artboard(&self, ctx: &mut PaintCtx, bounds: Rect, object: VectorObject) {
        let corners = object
            .corners()
            .map(|corner| self.to_screen(bounds, corner));
        let artboard_bounds = vector_points_bounds(&corners);
        ctx.fill(
            Path::rounded_rect(
                Rect::new(
                    artboard_bounds.x() + 8.0,
                    artboard_bounds.y() + 10.0,
                    artboard_bounds.width(),
                    artboard_bounds.height(),
                ),
                3.0,
            ),
            Color::rgba(0.04, 0.06, 0.10, 0.14),
        );
        ctx.fill(
            Path::rounded_rect(artboard_bounds, object.corner_radius * self.viewport.zoom),
            object
                .fill
                .unwrap_or(Color::WHITE)
                .with_alpha(object.opacity),
        );
        ctx.stroke(
            Path::rounded_rect(artboard_bounds, object.corner_radius * self.viewport.zoom),
            object.stroke,
            StrokeStyle::new(object.stroke_width * self.viewport.zoom),
        );
    }

    fn paint_ellipse(&self, ctx: &mut PaintCtx, bounds: Rect, object: VectorObject) {
        let path = vector_ellipse_path(bounds, self.viewport, object);
        if let Some(fill) = object.fill {
            ctx.fill(path.clone(), fill.with_alpha(object.opacity));
        }
        ctx.stroke(
            path,
            object.stroke.with_alpha(object.opacity.max(0.25)),
            StrokeStyle::new((object.stroke_width * self.viewport.zoom).max(1.0)),
        );
    }

    fn paint_bezier(&self, ctx: &mut PaintCtx, bounds: Rect, object: VectorObject) {
        let start = object.local_to_document(Point::new(
            -object.size.width * 0.5,
            object.size.height * 0.32,
        ));
        let ctrl1 = object.local_to_document(Point::new(
            -object.size.width * 0.30,
            -object.size.height * 0.48,
        ));
        let ctrl2 = object.local_to_document(Point::new(
            object.size.width * 0.28,
            object.size.height * 0.56,
        ));
        let end = object.local_to_document(Point::new(
            object.size.width * 0.5,
            -object.size.height * 0.30,
        ));

        let mut path = PathBuilder::new();
        path.move_to(self.to_screen(bounds, start)).cubic_to(
            self.to_screen(bounds, ctrl1),
            self.to_screen(bounds, ctrl2),
            self.to_screen(bounds, end),
        );
        ctx.stroke(
            path.build(),
            object.stroke.with_alpha(object.opacity),
            StrokeStyle::new((object.stroke_width * self.viewport.zoom).max(1.0)),
        );
    }

    fn paint_selection(&self, ctx: &mut PaintCtx, bounds: Rect, theme: &DefaultTheme) {
        let selected = self.state.selected_object();
        if !self.state.object_visible(selected) {
            return;
        }

        let object = self.state.selected_object_snapshot();
        let corners = object
            .corners()
            .map(|corner| self.to_screen(bounds, corner));
        let selection = vector_closed_polyline_path(&corners);
        let accent = theme.palette.accent_border_focus;
        ctx.stroke(selection, accent, StrokeStyle::new(1.5));

        let top_center = self.to_screen(bounds, object.top_center());
        let rotate_handle = self.to_screen(bounds, object.rotate_handle());
        ctx.stroke(
            vector_line_path(top_center, rotate_handle),
            accent.with_alpha(0.82),
            StrokeStyle::new(1.2),
        );
        ctx.fill(
            Path::circle(rotate_handle, VECTOR_HANDLE_SIZE * 0.62),
            Color::rgba(0.98, 0.995, 1.0, 1.0),
        );
        ctx.stroke(
            Path::circle(rotate_handle, VECTOR_HANDLE_SIZE * 0.62),
            accent,
            StrokeStyle::new(1.4),
        );

        for corner in corners {
            let handle = Self::resize_handle_rect(corner);
            ctx.fill(
                Path::rounded_rect(handle, 2.0),
                Color::rgba(0.98, 0.995, 1.0, 1.0),
            );
            ctx.stroke(
                Path::rounded_rect(handle, 2.0),
                accent,
                StrokeStyle::new(1.2),
            );
        }
    }
}

impl Widget for VectorEditorCanvas {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.apply_pending_viewport_commands(ctx.bounds()) {
            self.publish_viewport_state(ctx.bounds());
            Self::request_edit_update(ctx);
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && ctx.bounds().contains(pointer.position) =>
            {
                let delta = vector_scroll_delta_to_offset(pointer.scroll_delta, pointer.delta);
                if self.zoom_view_around(ctx.bounds(), pointer.position, (delta.y * 0.002).exp()) {
                    self.publish_viewport_state(ctx.bounds());
                }
                Self::request_edit_update(ctx);
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
                self.pan_drag = Some(VectorCanvasPanDrag {
                    pointer_id: pointer.pointer_id,
                    last_position: pointer.position,
                });
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                Self::request_edit_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                if let Some(mut pan_drag) = self.pan_drag
                    && pan_drag.pointer_id == pointer.pointer_id
                {
                    let delta = pointer.position - pan_drag.last_position;
                    self.viewport.pan += delta;
                    pan_drag.last_position = pointer.position;
                    self.pan_drag = Some(pan_drag);
                    self.publish_viewport_state(ctx.bounds());
                    Self::request_edit_update(ctx);
                    ctx.set_handled();
                    return;
                }

                if let Some(drag) = self.drag {
                    if drag.pointer_id == pointer.pointer_id {
                        let document = self.from_screen(ctx.bounds(), pointer.position);
                        self.apply_drag(drag, document);
                        Self::request_edit_update(ctx);
                        ctx.set_handled();
                        return;
                    }
                }

                if ctx.bounds().contains(pointer.position) {
                    self.update_hover(ctx.bounds(), pointer.position);
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.drag.is_none() && self.pan_drag.is_none() {
                    self.hovered_object = None;
                    self.hovered_handle = None;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position) =>
            {
                let document = self.from_screen(ctx.bounds(), pointer.position);
                let mode = if let Some(handle) =
                    self.selection_handle_at(ctx.bounds(), pointer.position)
                {
                    Some(handle)
                } else if let Some(hit_object) = self.state.hit_test(document) {
                    self.state.set_selected_object(hit_object);
                    Some(VectorCanvasDragMode::Move)
                } else {
                    None
                };

                if let Some(mode) = mode {
                    let selected = self.state.selected_object();
                    if !self.state.object_locked(selected) {
                        let object = self.state.selected_object_snapshot();
                        let start_angle =
                            (document.y - object.center.y).atan2(document.x - object.center.x);
                        self.drag = Some(VectorCanvasDrag {
                            pointer_id: pointer.pointer_id,
                            mode,
                            start_document: document,
                            start_object: object,
                            start_angle,
                        });
                    }
                }

                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                Self::request_edit_update(ctx);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    || pointer.kind == PointerEventKind::Cancel =>
            {
                if self
                    .pan_drag
                    .is_some_and(|drag| drag.pointer_id == pointer.pointer_id)
                {
                    self.pan_drag = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    Self::request_edit_update(ctx);
                    ctx.set_handled();
                    return;
                }

                if self
                    .drag
                    .is_some_and(|drag| drag.pointer_id == pointer.pointer_id)
                {
                    self.drag = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    Self::request_edit_update(ctx);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let zoom_step = self.theme().metrics.pixel_canvas_zoom_step;
                match key.key.as_str() {
                    "=" | "+" => {
                        self.zoom_view_around(
                            ctx.bounds(),
                            Self::screen_center(ctx.bounds()),
                            zoom_step,
                        );
                        self.publish_viewport_state(ctx.bounds());
                        Self::request_edit_update(ctx);
                        ctx.set_handled();
                        return;
                    }
                    "-" => {
                        self.zoom_view_around(
                            ctx.bounds(),
                            Self::screen_center(ctx.bounds()),
                            1.0 / zoom_step,
                        );
                        self.publish_viewport_state(ctx.bounds());
                        Self::request_edit_update(ctx);
                        ctx.set_handled();
                        return;
                    }
                    _ => {}
                }

                let step = 4.0;
                let mut center = self.state.selected_object_snapshot().center;
                match key.key.as_str() {
                    "ArrowLeft" => center.x -= step,
                    "ArrowRight" => center.x += step,
                    "ArrowUp" => center.y -= step,
                    "ArrowDown" => center.y += step,
                    "[" => {
                        self.state
                            .set_selected_rotation(self.state.selected_rotation() - 5.0);
                        Self::request_edit_update(ctx);
                        ctx.set_handled();
                        return;
                    }
                    "]" => {
                        self.state
                            .set_selected_rotation(self.state.selected_rotation() + 5.0);
                        Self::request_edit_update(ctx);
                        ctx.set_handled();
                        return;
                    }
                    _ => return,
                }
                let object = self.state.selected_object_snapshot();
                self.state
                    .set_selected_transform(center, object.size, object.rotation_degrees);
                Self::request_edit_update(ctx);
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
                960.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                620.0
            },
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let viewport_changed = self.apply_pending_viewport_commands(bounds);
        let state_changed = self.publish_viewport_state(bounds);
        if viewport_changed || state_changed {
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = self.theme();
        ctx.fill_rect(bounds, theme.palette.surface);
        ctx.stroke_bounds(theme.palette.border, StrokeStyle::new(1.0));
        ctx.push_clip_rect(bounds);
        Self::paint_grid(ctx, bounds, self.viewport, &theme);
        for index in self.state.object_paint_order() {
            if !self.state.object_visible(index) {
                continue;
            }
            if let Some(object) = self.state.object(index) {
                self.paint_object(ctx, bounds, object);
            }
        }
        self.paint_selection(ctx, bounds, &theme);
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let selected = self.state.selected_object_snapshot();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some(VECTOR_EDITOR_TAB_LABEL.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "Selected {}, x {:.0}, y {:.0}, width {:.0}, height {:.0}, rotation {:.0} deg, zoom {:.0}%",
            self.state.selected_object_label(),
            selected.center.x,
            selected.center.y,
            selected.size.width,
            selected.size.height,
            selected.rotation_degrees,
            self.viewport.zoom * 100.0
        )));
        node.state.focused = ctx.is_focused();
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Custom("Select and edit vector objects".into()),
        ];
        ctx.push(node);

        for index in self.state.object_order() {
            if !self.state.object_visible(index) {
                continue;
            }
            let Some(object) = self.state.object(index) else {
                continue;
            };
            let corners = object
                .corners()
                .map(|corner| self.to_screen(ctx.bounds(), corner));
            let mut object_node = SemanticsNode::new(
                vector_object_semantics_id(ctx.widget_id(), index),
                SemanticsRole::Image,
                vector_points_bounds(&corners),
            );
            object_node.parent = Some(ctx.widget_id());
            object_node.name = Some(VECTOR_OBJECT_LABELS[index].to_string());
            object_node.description = Some(object.detail());
            object_node.value = Some(SemanticsValue::Text(format!(
                "x {:.0}, y {:.0}, width {:.0}, height {:.0}, rotation {:.0} deg",
                object.center.x,
                object.center.y,
                object.size.width,
                object.size.height,
                object.rotation_degrees
            )));
            object_node.state.selected = self.state.selected_object() == index;
            object_node.state.disabled = self.state.object_locked(index);
            object_node.actions = vec![SemanticsAction::Activate];
            ctx.push(object_node);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

fn vector_object_semantics_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(397)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn vector_line_path(from: Point, to: Point) -> Path {
    let mut path = PathBuilder::new();
    path.move_to(from).line_to(to);
    path.build()
}

fn vector_closed_polyline_path(points: &[Point]) -> Path {
    let mut path = PathBuilder::new();
    if let Some(first) = points.first().copied() {
        path.move_to(first);
        for point in points.iter().skip(1).copied() {
            path.line_to(point);
        }
        path.close();
    }
    path.build()
}

fn vector_points_bounds(points: &[Point]) -> Rect {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        Rect::new(0.0, 0.0, 0.0, 0.0)
    } else {
        Rect::new(
            min_x,
            min_y,
            (max_x - min_x).max(0.0),
            (max_y - min_y).max(0.0),
        )
    }
}

fn vector_scroll_delta_to_offset(scroll_delta: Option<ScrollDelta>, fallback: Vector) -> Vector {
    match scroll_delta {
        Some(ScrollDelta::Pixels(delta)) => delta,
        Some(ScrollDelta::Lines(delta)) => Vector::new(delta.x * 16.0, delta.y * 16.0),
        None => fallback,
    }
}

fn vector_ellipse_path(bounds: Rect, viewport: CanvasViewport, object: VectorObject) -> Path {
    let mut path = PathBuilder::new();
    let segments = 56;
    for segment in 0..=segments {
        let angle = (segment as f32 / segments as f32) * std::f32::consts::TAU;
        let local = Point::new(
            angle.cos() * object.size.width * 0.5,
            angle.sin() * object.size.height * 0.5,
        );
        let point = VectorEditorCanvas::document_to_screen_with_viewport(
            bounds,
            viewport,
            object.local_to_document(local),
        );
        if segment == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close();
    path.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    use sui::{
        Application, Event, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Result,
        ScrollDelta, SemanticsRole, SemanticsValue, Vector, WindowBuilder,
    };
    use sui_testing::{TestApp, TestWindow, WindowSnapshot};

    #[test]
    fn vector_demo_state_supports_selection_and_transform_edits() {
        let state = VectorDemoState::new();
        assert_eq!(state.object_order(), vec![3, 2, 1, 0]);
        assert_eq!(state.object_paint_order(), vec![0, 1, 2, 3]);
        assert_eq!(state.hit_test(Point::new(92.0, 44.0)), Some(2));

        state.set_selected_object(2);
        state.set_selected_width(180.0);
        state.set_selected_height(128.0);
        state.set_selected_rotation(42.0);
        assert_eq!(state.selected_width(), 180.0);
        assert_eq!(state.selected_height(), 128.0);
        assert_eq!(state.selected_rotation(), 42.0);

        state.align_selected(VectorAlignment::Center);
        state.align_selected(VectorAlignment::Middle);
        assert_eq!(state.selected_center_x(), 0.0);
        assert_eq!(state.selected_center_y(), 0.0);

        state.reset_selected_transform();
        assert_eq!(state.selected_width(), 142.0);
        assert_eq!(state.selected_height(), 112.0);
        assert_eq!(state.selected_rotation(), 8.0);
    }

    #[test]
    fn vector_editor_canvas_selects_moves_and_resizes_objects() -> Result<()> {
        let app = TestApp::new(|| build_vector_canvas_test_application().build())?;
        let window = app.main_window()?;
        let snapshot = window.snapshot()?;
        let canvas = find_named_node(&snapshot, SemanticsRole::Canvas, VECTOR_EDITOR_TAB_LABEL);
        let amber = vector_default_objects()[2];
        let amber_center = VectorEditorCanvas::document_to_screen(canvas.bounds, amber.center);

        click_pointer(&window, amber_center)?;
        let selected_snapshot = window.snapshot()?;
        let selected_canvas = find_named_node(
            &selected_snapshot,
            SemanticsRole::Canvas,
            VECTOR_EDITOR_TAB_LABEL,
        );
        assert!(
            canvas_value_text(&selected_canvas).contains("Selected Amber ellipse"),
            "expected clicking the amber object to select it, value={:?}",
            selected_canvas.value
        );

        drag_pointer(
            &window,
            amber_center,
            amber_center + Vector::new(40.0, 20.0),
        )?;
        let moved_snapshot = window.snapshot()?;
        let moved_canvas = find_named_node(
            &moved_snapshot,
            SemanticsRole::Canvas,
            VECTOR_EDITOR_TAB_LABEL,
        );
        let moved_text = canvas_value_text(&moved_canvas);
        assert!(
            moved_text.contains("x 130") && moved_text.contains("y 63"),
            "expected dragging the selected vector object to move it, value={:?}",
            moved_canvas.value
        );

        let moved_object = VectorObject {
            center: Point::new(130.0, 63.0),
            ..amber
        };
        let resize_handle =
            VectorEditorCanvas::document_to_screen(canvas.bounds, moved_object.corners()[2]);
        drag_pointer(
            &window,
            resize_handle,
            resize_handle + Vector::new(52.0, 32.0),
        )?;
        let resized_snapshot = window.snapshot()?;
        let resized_canvas = find_named_node(
            &resized_snapshot,
            SemanticsRole::Canvas,
            VECTOR_EDITOR_TAB_LABEL,
        );
        let resized_text = canvas_value_text(&resized_canvas);
        assert!(
            resized_text.contains("Selected Amber ellipse"),
            "expected resizing to keep the amber object selected, value={:?}",
            resized_canvas.value
        );
        let width = vector_canvas_dimension(&resized_text, "width").unwrap_or_default();
        assert!(
            width > 142.0 && width < 720.0,
            "expected dragging a resize handle to increase selected width, value={resized_text:?}"
        );

        Ok(())
    }

    #[test]
    fn vector_editor_view_controls_update_zoom_readout() -> Result<()> {
        let app = TestApp::new(|| build_vector_editor_test_application().build())?;
        let window = app.main_window()?;

        let before = window.snapshot()?;
        let before_canvas =
            find_named_node(&before, SemanticsRole::Canvas, VECTOR_EDITOR_TAB_LABEL);
        let before_text = canvas_value_text(&before_canvas);
        assert!(before_text.contains("zoom 105%"));

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(VECTOR_ZOOM_IN_NAME)
            .click()?;
        let zoomed = window.snapshot()?;
        let zoomed_canvas =
            find_named_node(&zoomed, SemanticsRole::Canvas, VECTOR_EDITOR_TAB_LABEL);
        let zoomed_text = canvas_value_text(&zoomed_canvas);
        assert!(
            zoomed_text.contains("zoom 116%"),
            "expected zoom-in control to update canvas zoom semantics, value={zoomed_text:?}"
        );

        window
            .get_by_role(SemanticsRole::Button)
            .with_name(VECTOR_ACTUAL_SIZE_NAME)
            .click()?;
        let actual = window.snapshot()?;
        let actual_canvas =
            find_named_node(&actual, SemanticsRole::Canvas, VECTOR_EDITOR_TAB_LABEL);
        let actual_text = canvas_value_text(&actual_canvas);
        assert!(
            actual_text.contains("zoom 100%"),
            "expected actual-size control to reset canvas zoom semantics, value={actual_text:?}"
        );

        Ok(())
    }

    #[test]
    fn vector_editor_canvas_wheel_zoom_and_middle_drag_pan_view() -> Result<()> {
        let app = TestApp::new(|| build_vector_canvas_test_application().build())?;
        let window = app.main_window()?;
        let snapshot = window.snapshot()?;
        let canvas = find_named_node(&snapshot, SemanticsRole::Canvas, VECTOR_EDITOR_TAB_LABEL);
        let amber_before = find_named_node(&snapshot, SemanticsRole::Image, "Amber ellipse");
        let canvas_center = Point::new(
            canvas.bounds.x() + canvas.bounds.width() * 0.5,
            canvas.bounds.y() + canvas.bounds.height() * 0.5,
        );

        drag_pointer_with_button(
            &window,
            canvas_center,
            canvas_center + Vector::new(44.0, 28.0),
            PointerButton::Middle,
        )?;
        let panned = window.snapshot()?;
        let amber_panned = find_named_node(&panned, SemanticsRole::Image, "Amber ellipse");
        assert!(
            amber_panned.bounds.x() > amber_before.bounds.x() + 24.0
                && amber_panned.bounds.y() > amber_before.bounds.y() + 12.0,
            "expected middle-drag panning to move vector object semantics, before={:?} after={:?}",
            amber_before.bounds,
            amber_panned.bounds
        );

        scroll_pointer(&window, canvas_center, Vector::new(0.0, 120.0))?;
        let zoomed = window.snapshot()?;
        let amber_zoomed = find_named_node(&zoomed, SemanticsRole::Image, "Amber ellipse");
        assert!(
            amber_zoomed.bounds.width() > amber_panned.bounds.width(),
            "expected wheel zoom to enlarge object bounds, before={:?} after={:?}",
            amber_panned.bounds,
            amber_zoomed.bounds
        );

        Ok(())
    }

    fn build_vector_editor_test_application() -> Application {
        Application::new().window(WindowBuilder::new().title(VECTOR_EDITOR_TAB_LABEL).root(
            build_vector_editor_demo_with_theme(default_dev_theme_reader()),
        ))
    }

    fn build_vector_canvas_test_application() -> Application {
        let viewport_state = VectorCanvasViewportState::new();
        Application::new().window(WindowBuilder::new().title(VECTOR_EDITOR_TAB_LABEL).root(
            VectorEditorCanvas::new(
                VectorDemoState::new(),
                viewport_state,
                default_dev_theme_reader(),
            ),
        ))
    }

    fn click_pointer(window: &TestWindow, position: Point) -> Result<()> {
        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            position,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, position);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, position);
        up.button = Some(PointerButton::Primary);
        root.dispatch_event(Event::Pointer(up)).map(|_| ())
    }

    fn drag_pointer(window: &TestWindow, from: Point, to: Point) -> Result<()> {
        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            from,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, from);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let steps = 8;
        let total_delta = to - from;
        let mut previous = from;
        for step in 1..=steps {
            let progress = step as f32 / steps as f32;
            let position = Point::new(
                from.x + (total_delta.x * progress),
                from.y + (total_delta.y * progress),
            );
            let mut moved = PointerEvent::new(PointerEventKind::Move, position);
            moved.buttons = PointerButtons::new(1);
            moved.delta = position - previous;
            root.dispatch_event(Event::Pointer(moved))?;
            previous = position;
        }

        let mut up = PointerEvent::new(PointerEventKind::Up, to);
        up.button = Some(PointerButton::Primary);
        root.dispatch_event(Event::Pointer(up)).map(|_| ())
    }

    fn drag_pointer_with_button(
        window: &TestWindow,
        from: Point,
        to: Point,
        button: PointerButton,
    ) -> Result<()> {
        let root = window.root();

        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            from,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, from);
        down.button = Some(button);
        down.buttons = PointerButtons::new(1);
        root.dispatch_event(Event::Pointer(down))?;

        let mut moved = PointerEvent::new(PointerEventKind::Move, to);
        moved.buttons = PointerButtons::new(1);
        moved.delta = to - from;
        root.dispatch_event(Event::Pointer(moved))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, to);
        up.button = Some(button);
        root.dispatch_event(Event::Pointer(up)).map(|_| ())
    }

    fn scroll_pointer(window: &TestWindow, position: Point, delta: Vector) -> Result<()> {
        let root = window.root();
        root.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            position,
        )))?;
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, position);
        scroll.scroll_delta = Some(ScrollDelta::Pixels(delta));
        root.dispatch_event(Event::Pointer(scroll)).map(|_| ())
    }

    fn find_named_node(
        snapshot: &WindowSnapshot,
        role: SemanticsRole,
        name: &str,
    ) -> SemanticsNode {
        let matches = snapshot
            .accessibility
            .nodes
            .iter()
            .filter(|node| node.role == role && node.name.as_deref() == Some(name))
            .cloned()
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [node] => node.clone(),
            [] => panic!("missing semantics node {:?} named {:?}", role, name),
            _ => panic!(
                "expected exactly one semantics node {:?} named {:?}, found {}",
                role,
                name,
                matches.len()
            ),
        }
    }

    fn canvas_value_text(node: &SemanticsNode) -> String {
        match &node.value {
            Some(SemanticsValue::Text(text)) => text.clone(),
            other => panic!("expected text semantics value, got {other:?}"),
        }
    }

    fn vector_canvas_dimension(text: &str, name: &str) -> Option<f32> {
        let mut parts = text.split_whitespace();
        while let Some(part) = parts.next() {
            if part.trim_end_matches(',') == name {
                return parts.next()?.trim_end_matches(',').parse().ok();
            }
        }
        None
    }
}
