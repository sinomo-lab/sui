#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::too_many_arguments,
    clippy::wrong_self_convention
)]

use std::{
    cell::RefCell,
    fs,
    sync::{Arc, Mutex},
};

use ::sui as sui_crate;
use pyo3::{
    Bound, Py, PyAny, PyErr, PyResult, Python,
    exceptions::{PyOSError, PyRuntimeError, PyValueError},
    prelude::*,
    types::PyModule,
};
use sui_bindings_core::{
    BindingAction, BindingApp, BindingBool, BindingBoolAction, BindingBrushPreviewSpec,
    BindingColorAction, BindingColorPaletteSwatch, BindingColorSelectAction, BindingCustomEvent,
    BindingEvent, BindingFloatingStackWindow, BindingFontHandle, BindingImageFit,
    BindingImageHandle, BindingImeEvent, BindingKeyState, BindingKeyboardEvent,
    BindingLayerListItem, BindingMenuItem, BindingModifiers, BindingNumber, BindingNumberAction,
    BindingPointerButton, BindingPointerEvent, BindingPointerEventKind, BindingPointerKind,
    BindingRenderSnapshot, BindingReorderAction, BindingRuntime, BindingScrollAxes,
    BindingScrollDelta, BindingSegmentedControlItem, BindingSelectAction, BindingShader,
    BindingState, BindingStatusBarSegment, BindingStringAction, BindingTableColumn,
    BindingTableRow, BindingText, BindingTextSpan, BindingToolPaletteItem, BindingTreeItem,
    BindingUiHandle, BindingValue, BindingWidget, BindingWindow, BindingWindowEvent,
    BindingWindowId, ExternalBackendHandle, ExternalSync, ExternalTextureDescriptor,
    ExternalTextureFormat, ExternalTextureValidationError, ForeignCallbackFailure,
    ForeignCallbackResult, ForeignEventCtx, ForeignMeasureCtx, ForeignPaintCtx,
    ForeignSemanticsCtx, ForeignWidget, ForeignWidgetCallbacks, NativeGraphicsBackend,
    PaintCommand, PaintCommandBuilder, PaintValidationError, RendererInteropCapabilities,
    RendererInteropTier, UiTaskQueue, binding_alignment_from_name, binding_icon_glyph_from_name,
    binding_semantic_tone_from_name, binding_semantics_busy, binding_semantics_checked,
    binding_semantics_descriptions, binding_semantics_disabled,
    binding_semantics_editable_multiline, binding_semantics_expanded, binding_semantics_focused,
    binding_semantics_hidden, binding_semantics_hovered, binding_semantics_names,
    binding_semantics_role_from_name, binding_semantics_roles, binding_semantics_selected,
    binding_semantics_values, binding_surface_border_from_name,
    binding_surface_elevation_from_name, binding_surface_role_from_name,
    binding_table_column_alignment_from_name, binding_toggle_state_from_name,
    binding_tooltip_placement_from_name, resolve_binding_image_slots,
};
use sui_crate::{
    Axis, Color, ColorSpace, Constraints, Event, FontStretch, FontStyle, FontWeight, Path,
    PathBuilder, Rect, RegisteredImage, RuntimeApplication, SceneCommand, SemanticsNode,
    SemanticsRole, SemanticsValue, ShadowParams, Size, StrokeStyle, TextStyle, ToggleState,
    Transform, Vector, WidgetId, WindowBuilder,
};

#[pyclass(name = "Point", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyPoint {
    #[pyo3(get)]
    pub x: f32,
    #[pyo3(get)]
    pub y: f32,
}

#[pymethods]
impl PyPoint {
    #[new]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn __repr__(&self) -> String {
        format!("Point({}, {})", self.x, self.y)
    }
}

impl From<sui_crate::Point> for PyPoint {
    fn from(value: sui_crate::Point) -> Self {
        Self::new(value.x, value.y)
    }
}

impl From<PyPoint> for sui_crate::Point {
    fn from(value: PyPoint) -> Self {
        Self::new(value.x, value.y)
    }
}

impl From<PyPoint> for Vector {
    fn from(value: PyPoint) -> Self {
        Self::new(value.x, value.y)
    }
}

#[pyclass(name = "Modifiers", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyModifiers {
    #[pyo3(get)]
    pub shift: bool,
    #[pyo3(get)]
    pub control: bool,
    #[pyo3(get)]
    pub alt: bool,
    #[pyo3(get)]
    pub meta: bool,
}

#[pymethods]
impl PyModifiers {
    #[new]
    #[pyo3(signature = (shift=false, control=false, alt=false, meta=false))]
    pub const fn new(shift: bool, control: bool, alt: bool, meta: bool) -> Self {
        Self {
            shift,
            control,
            alt,
            meta,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Modifiers(shift={}, control={}, alt={}, meta={})",
            self.shift, self.control, self.alt, self.meta
        )
    }
}

impl From<BindingModifiers> for PyModifiers {
    fn from(value: BindingModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

impl From<PyModifiers> for BindingModifiers {
    fn from(value: PyModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

#[pyclass(name = "Event", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyEvent {
    inner: BindingEvent,
}

#[pymethods]
impl PyEvent {
    #[staticmethod]
    #[pyo3(signature = (kind, position, pointer_id=0, delta=None, button=None, buttons=0, modifiers=None, pointer_kind="mouse", is_primary=true))]
    pub fn pointer(
        kind: &str,
        position: PyPoint,
        pointer_id: u64,
        delta: Option<PyPoint>,
        button: Option<String>,
        buttons: u8,
        modifiers: Option<PyModifiers>,
        pointer_kind: &str,
        is_primary: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: BindingEvent::Pointer(BindingPointerEvent {
                pointer_id,
                kind: parse_pointer_event_kind(kind)?,
                position: position.into(),
                delta: delta.unwrap_or(PyPoint::new(0.0, 0.0)).into(),
                scroll_delta: None,
                button: button.as_deref().map(parse_pointer_button).transpose()?,
                buttons,
                modifiers: modifiers.unwrap_or_default().into(),
                pointer_kind: parse_pointer_kind(pointer_kind)?,
                is_primary,
            }),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (position, delta, mode="pixels", pointer_id=0, modifiers=None))]
    pub fn scroll(
        position: PyPoint,
        delta: PyPoint,
        mode: &str,
        pointer_id: u64,
        modifiers: Option<PyModifiers>,
    ) -> PyResult<Self> {
        let scroll_delta = match mode {
            "pixels" | "pixel" => BindingScrollDelta::Pixels(delta.into()),
            "lines" | "line" => BindingScrollDelta::Lines(delta.into()),
            _ => {
                return Err(PyValueError::new_err(
                    "scroll mode must be 'pixels' or 'lines'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Pointer(BindingPointerEvent {
                pointer_id,
                kind: BindingPointerEventKind::Scroll,
                position: position.into(),
                delta: delta.into(),
                scroll_delta: Some(scroll_delta),
                button: None,
                buttons: 0,
                modifiers: modifiers.unwrap_or_default().into(),
                pointer_kind: BindingPointerKind::Mouse,
                is_primary: true,
            }),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (key, state="pressed", code=None, text=None, modifiers=None, repeat=false, is_composing=false))]
    pub fn keyboard(
        key: String,
        state: &str,
        code: Option<String>,
        text: Option<String>,
        modifiers: Option<PyModifiers>,
        repeat: bool,
        is_composing: bool,
    ) -> PyResult<Self> {
        let mut event = BindingKeyboardEvent::new(key, parse_key_state(state)?);
        if let Some(code) = code {
            event.code = code;
        }
        if text.is_some() {
            event.text = text;
        }
        event.modifiers = modifiers.unwrap_or_default().into();
        event.repeat = repeat;
        event.is_composing = is_composing;
        Ok(Self {
            inner: BindingEvent::Keyboard(event),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (kind, text=None, cursor_start=None, cursor_end=None))]
    pub fn ime(
        kind: &str,
        text: Option<String>,
        cursor_start: Option<usize>,
        cursor_end: Option<usize>,
    ) -> PyResult<Self> {
        let inner = match kind {
            "composition_start" | "composition-start" | "start" => {
                BindingImeEvent::CompositionStart
            }
            "composition_update" | "composition-update" | "update" => {
                BindingImeEvent::CompositionUpdate {
                    text: text.unwrap_or_default(),
                    cursor_start,
                    cursor_end,
                }
            }
            "composition_commit" | "composition-commit" | "commit" => {
                BindingImeEvent::CompositionCommit {
                    text: text.unwrap_or_default(),
                }
            }
            "composition_end" | "composition-end" | "end" => BindingImeEvent::CompositionEnd,
            _ => {
                return Err(PyValueError::new_err(
                    "IME kind must be 'composition_start', 'composition_update', 'composition_commit', or 'composition_end'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Ime(inner),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (kind, value=None, size=None, scale_factor=None, raw_dpi=None, suggested_size=None))]
    pub fn window(
        kind: &str,
        value: Option<bool>,
        size: Option<PySize>,
        scale_factor: Option<f64>,
        raw_dpi: Option<f32>,
        suggested_size: Option<PySize>,
    ) -> PyResult<Self> {
        let inner = match kind {
            "close_requested" | "close-requested" | "close" => BindingWindowEvent::CloseRequested,
            "resized" | "resize" => BindingWindowEvent::Resized(
                size.ok_or_else(|| PyValueError::new_err("resized window events require size"))?
                    .into(),
            ),
            "scale_factor_changed" | "scale-factor-changed" => {
                BindingWindowEvent::ScaleFactorChanged {
                    scale_factor: scale_factor.unwrap_or(1.0),
                    raw_dpi,
                    suggested_size: suggested_size.map(Into::into),
                }
            }
            "focused" | "focus" => BindingWindowEvent::Focused(value.unwrap_or(false)),
            "occluded" => BindingWindowEvent::Occluded(value.unwrap_or(false)),
            "redraw_requested" | "redraw-requested" | "redraw" => {
                BindingWindowEvent::RedrawRequested
            }
            _ => {
                return Err(PyValueError::new_err(
                    "window kind must be 'close_requested', 'resized', 'scale_factor_changed', 'focused', 'occluded', or 'redraw_requested'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Window(inner),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (kind, payload=None))]
    pub fn custom(kind: String, payload: Option<String>) -> Self {
        Self {
            inner: BindingEvent::Custom(BindingCustomEvent { kind, payload }),
        }
    }

    #[getter]
    pub fn kind(&self) -> String {
        self.inner.kind().to_string()
    }

    #[getter]
    pub fn action(&self) -> Option<&'static str> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(pointer_event_kind_name(event.kind)),
            BindingEvent::Ime(event) => Some(ime_event_kind_name(event)),
            BindingEvent::Window(event) => Some(window_event_kind_name(event)),
            BindingEvent::Custom(_)
            | BindingEvent::Keyboard(_)
            | BindingEvent::Unsupported { .. } => None,
        }
    }

    #[getter]
    pub fn pointer_id(&self) -> Option<u64> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.pointer_id),
            _ => None,
        }
    }

    #[getter]
    pub fn position(&self) -> Option<PyPoint> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.position.into()),
            _ => None,
        }
    }

    #[getter]
    pub fn delta(&self) -> Option<PyPoint> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(PyPoint::new(event.delta.x, event.delta.y)),
            _ => None,
        }
    }

    #[getter]
    pub fn scroll_mode(&self) -> Option<&'static str> {
        match &self.inner {
            BindingEvent::Pointer(event) => match event.scroll_delta {
                Some(BindingScrollDelta::Lines(_)) => Some("lines"),
                Some(BindingScrollDelta::Pixels(_)) => Some("pixels"),
                None => None,
            },
            _ => None,
        }
    }

    #[getter]
    pub fn button(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => event.button.map(pointer_button_name),
            _ => None,
        }
    }

    #[getter]
    pub fn buttons(&self) -> Option<u8> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.buttons),
            _ => None,
        }
    }

    #[getter]
    pub fn modifiers(&self) -> Option<PyModifiers> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.modifiers.into()),
            BindingEvent::Keyboard(event) => Some(event.modifiers.into()),
            _ => None,
        }
    }

    #[getter]
    pub fn device_kind(&self) -> Option<&'static str> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(pointer_kind_name(event.pointer_kind)),
            _ => None,
        }
    }

    #[getter]
    pub fn is_primary(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.is_primary),
            _ => None,
        }
    }

    #[getter]
    pub fn key(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.key.clone()),
            _ => None,
        }
    }

    #[getter]
    pub fn code(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.code.clone()),
            _ => None,
        }
    }

    #[getter]
    pub fn text(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => event.text.clone(),
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate { text, .. })
            | BindingEvent::Ime(BindingImeEvent::CompositionCommit { text }) => Some(text.clone()),
            _ => None,
        }
    }

    #[getter]
    pub fn state(&self) -> Option<&'static str> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(key_state_name(event.state)),
            _ => None,
        }
    }

    #[getter]
    pub fn repeat(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.repeat),
            _ => None,
        }
    }

    #[getter]
    pub fn is_composing(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.is_composing),
            _ => None,
        }
    }

    #[getter]
    pub fn custom_kind(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Custom(event) => Some(event.kind.clone()),
            _ => None,
        }
    }

    #[getter]
    pub fn payload(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Custom(event) => event.payload.clone(),
            _ => None,
        }
    }

    #[getter]
    pub fn file_path(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::ExternalFileHovered(path))
            | BindingEvent::Window(BindingWindowEvent::ExternalFileDropped(path)) => {
                Some(path.clone())
            }
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("Event(kind='{}')", self.inner.kind())
    }
}

impl PyEvent {
    fn from_binding(inner: BindingEvent) -> Self {
        Self { inner }
    }

    fn binding_event(&self) -> BindingEvent {
        self.inner.clone()
    }
}

#[pyclass(name = "Size", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PySize {
    #[pyo3(get)]
    pub width: f32,
    #[pyo3(get)]
    pub height: f32,
}

#[pymethods]
impl PySize {
    #[new]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    fn __repr__(&self) -> String {
        format!("Size({}, {})", self.width, self.height)
    }
}

impl From<Size> for PySize {
    fn from(value: Size) -> Self {
        Self::new(value.width, value.height)
    }
}

impl From<PySize> for Size {
    fn from(value: PySize) -> Self {
        Self::new(value.width, value.height)
    }
}

#[pyclass(name = "Rect", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyRect {
    #[pyo3(get)]
    pub x: f32,
    #[pyo3(get)]
    pub y: f32,
    #[pyo3(get)]
    pub width: f32,
    #[pyo3(get)]
    pub height: f32,
}

#[pymethods]
impl PyRect {
    #[new]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[getter]
    pub const fn origin(&self) -> PyPoint {
        PyPoint::new(self.x, self.y)
    }

    #[getter]
    pub const fn size(&self) -> PySize {
        PySize::new(self.width, self.height)
    }

    fn __repr__(&self) -> String {
        format!(
            "Rect({}, {}, {}, {})",
            self.x, self.y, self.width, self.height
        )
    }
}

impl From<Rect> for PyRect {
    fn from(value: Rect) -> Self {
        Self::new(value.x(), value.y(), value.width(), value.height())
    }
}

impl From<PyRect> for Rect {
    fn from(value: PyRect) -> Self {
        Self::new(value.x, value.y, value.width, value.height)
    }
}

#[pyclass(name = "Path", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyPath {
    inner: Path,
}

#[pymethods]
impl PyPath {
    #[new]
    pub fn new() -> Self {
        Self { inner: Path::new() }
    }

    #[staticmethod]
    pub fn rect(rect: PyRect) -> Self {
        Self {
            inner: Path::rect(rect.into()),
        }
    }

    #[staticmethod]
    pub fn circle(center: PyPoint, radius: f32) -> Self {
        Self {
            inner: Path::circle(center.into(), radius),
        }
    }

    #[staticmethod]
    pub fn rounded_rect(rect: PyRect, radius: f32) -> Self {
        Self {
            inner: Path::rounded_rect(rect.into(), radius),
        }
    }

    #[staticmethod]
    pub fn arc(center: PyPoint, radius: f32, start_angle: f32, sweep_angle: f32) -> Self {
        Self {
            inner: Path::arc(center.into(), radius, start_angle, sweep_angle),
        }
    }

    #[getter]
    pub fn bounds(&self) -> PyRect {
        self.inner.bounds().into()
    }

    #[getter]
    pub fn element_count(&self) -> usize {
        self.inner.elements().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!("Path(elements={})", self.inner.elements().len())
    }
}

impl Default for PyPath {
    fn default() -> Self {
        Self::new()
    }
}

#[pyclass(name = "PathBuilder", module = "sui", skip_from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct PyPathBuilder {
    inner: PathBuilder,
}

#[pymethods]
impl PyPathBuilder {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: PathBuilder::new(),
        }
    }

    pub fn move_to(&mut self, point: PyPoint) {
        self.inner.move_to(point.into());
    }

    pub fn line_to(&mut self, point: PyPoint) {
        self.inner.line_to(point.into());
    }

    pub fn quad_to(&mut self, ctrl: PyPoint, to: PyPoint) {
        self.inner.quad_to(ctrl.into(), to.into());
    }

    pub fn cubic_to(&mut self, ctrl1: PyPoint, ctrl2: PyPoint, to: PyPoint) {
        self.inner.cubic_to(ctrl1.into(), ctrl2.into(), to.into());
    }

    pub fn close(&mut self) {
        self.inner.close();
    }

    pub fn push_rect(&mut self, rect: PyRect) {
        self.inner.push_rect(rect.into());
    }

    pub fn push_circle(&mut self, center: PyPoint, radius: f32) {
        self.inner.push_circle(center.into(), radius);
    }

    pub fn push_rounded_rect(&mut self, rect: PyRect, radius: f32) {
        self.inner.push_rounded_rect(rect.into(), radius);
    }

    pub fn push_arc(&mut self, center: PyPoint, radius: f32, start_angle: f32, sweep_angle: f32) {
        self.inner
            .push_arc(center.into(), radius, start_angle, sweep_angle);
    }

    pub fn build(&self) -> PyPath {
        PyPath {
            inner: self.inner.clone().build(),
        }
    }
}

#[pyclass(name = "Transform", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyTransform {
    #[pyo3(get)]
    pub xx: f32,
    #[pyo3(get)]
    pub yx: f32,
    #[pyo3(get)]
    pub xy: f32,
    #[pyo3(get)]
    pub yy: f32,
    #[pyo3(get)]
    pub dx: f32,
    #[pyo3(get)]
    pub dy: f32,
}

#[pymethods]
impl PyTransform {
    #[new]
    pub const fn new(xx: f32, yx: f32, xy: f32, yy: f32, dx: f32, dy: f32) -> Self {
        Self {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        }
    }

    #[staticmethod]
    pub const fn identity() -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    }

    #[staticmethod]
    pub const fn translation(x: f32, y: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, x, y)
    }

    #[staticmethod]
    pub const fn scale(x: f32, y: f32) -> Self {
        Self::new(x, 0.0, 0.0, y, 0.0, 0.0)
    }

    #[staticmethod]
    pub fn rotation(radians: f32) -> Self {
        Transform::rotation(radians).into()
    }

    pub fn then(&self, next: PyTransform) -> Self {
        Transform::from(*self).then(next.into()).into()
    }

    fn __repr__(&self) -> String {
        format!(
            "Transform({}, {}, {}, {}, {}, {})",
            self.xx, self.yx, self.xy, self.yy, self.dx, self.dy
        )
    }
}

impl From<Transform> for PyTransform {
    fn from(value: Transform) -> Self {
        Self::new(value.xx, value.yx, value.xy, value.yy, value.dx, value.dy)
    }
}

impl From<PyTransform> for Transform {
    fn from(value: PyTransform) -> Self {
        Self::new(value.xx, value.yx, value.xy, value.yy, value.dx, value.dy)
    }
}

#[pyclass(name = "Color", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyColor {
    #[pyo3(get)]
    pub red: f32,
    #[pyo3(get)]
    pub green: f32,
    #[pyo3(get)]
    pub blue: f32,
    #[pyo3(get)]
    pub alpha: f32,
}

#[pymethods]
impl PyColor {
    #[new]
    #[pyo3(signature = (red, green, blue, alpha=1.0))]
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    #[staticmethod]
    #[pyo3(signature = (red, green, blue, alpha=1.0))]
    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::new(red, green, blue, alpha)
    }

    #[staticmethod]
    pub const fn white() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0)
    }

    #[staticmethod]
    pub const fn black() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    fn __repr__(&self) -> String {
        format!(
            "Color({}, {}, {}, {})",
            self.red, self.green, self.blue, self.alpha
        )
    }
}

impl From<PyColor> for Color {
    fn from(value: PyColor) -> Self {
        Self::rgba(value.red, value.green, value.blue, value.alpha)
    }
}

impl From<Color> for PyColor {
    fn from(value: Color) -> Self {
        Self::new(value.red, value.green, value.blue, value.alpha)
    }
}

#[pyclass(name = "Shadow", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyShadow {
    #[pyo3(get)]
    pub offset_x: f32,
    #[pyo3(get)]
    pub offset_y: f32,
    #[pyo3(get)]
    pub blur: f32,
    #[pyo3(get)]
    pub spread: f32,
    #[pyo3(get)]
    pub color: PyColor,
}

#[pymethods]
impl PyShadow {
    #[new]
    #[pyo3(signature = (offset_x, offset_y, blur, spread, color))]
    pub const fn new(offset_x: f32, offset_y: f32, blur: f32, spread: f32, color: PyColor) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            spread,
            color,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Shadow({}, {}, {}, {}, {:?})",
            self.offset_x, self.offset_y, self.blur, self.spread, self.color
        )
    }
}

impl From<PyShadow> for ShadowParams {
    fn from(value: PyShadow) -> Self {
        Self {
            offset_x: value.offset_x,
            offset_y: value.offset_y,
            blur: value.blur,
            spread: value.spread,
            color: value.color.into(),
        }
    }
}

#[pyclass(name = "Constraints", frozen, module = "sui", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyConstraints {
    #[pyo3(get)]
    pub min: PySize,
    #[pyo3(get)]
    pub max: PySize,
}

#[pymethods]
impl PyConstraints {
    #[new]
    pub const fn new(min: PySize, max: PySize) -> Self {
        Self { min, max }
    }

    pub fn clamp(&self, size: PySize) -> PySize {
        self.to_sui().clamp(size.into()).into()
    }

    pub fn loosen(&self) -> Self {
        Self::from(self.to_sui().loosen())
    }
}

impl PyConstraints {
    fn to_sui(self) -> Constraints {
        Constraints::new(self.min.into(), self.max.into())
    }
}

impl From<Constraints> for PyConstraints {
    fn from(value: Constraints) -> Self {
        Self {
            min: value.min.into(),
            max: value.max.into(),
        }
    }
}

#[pyclass(name = "FontHandle", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyFontHandle {
    inner: BindingFontHandle,
}

#[pymethods]
impl PyFontHandle {
    #[new]
    pub const fn new(raw: u64) -> Self {
        Self {
            inner: BindingFontHandle::new(raw),
        }
    }

    #[getter]
    pub const fn id(&self) -> u64 {
        self.inner.get()
    }

    fn __repr__(&self) -> String {
        format!("FontHandle({})", self.inner.get())
    }
}

include!("generated_widgets.rs");

#[pyclass(name = "ImageHandle", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyImageHandle {
    inner: BindingImageHandle,
}

#[pymethods]
impl PyImageHandle {
    #[new]
    pub const fn new(raw: u64) -> Self {
        Self {
            inner: BindingImageHandle::new(raw),
        }
    }

    #[staticmethod]
    pub const fn local(slot: u64) -> Self {
        Self {
            inner: BindingImageHandle::local(slot),
        }
    }

    #[getter]
    pub const fn id(&self) -> u64 {
        self.inner.get()
    }

    #[getter]
    pub const fn local_slot(&self) -> Option<u64> {
        self.inner.local_slot()
    }

    fn __repr__(&self) -> String {
        format!("ImageHandle({})", self.inner.get())
    }
}

#[derive(Clone)]
struct PendingPaintImage {
    slot: u64,
    image: RegisteredImage,
}

#[pyclass(name = "Paint", module = "sui", skip_from_py_object)]
#[derive(Clone)]
pub struct PyPaint {
    builder: Arc<Mutex<PaintCommandBuilder>>,
    images: Arc<Mutex<Vec<PendingPaintImage>>>,
    bounds: PyRect,
}

#[pymethods]
impl PyPaint {
    #[getter]
    pub const fn bounds(&self) -> PyRect {
        self.bounds
    }

    pub fn clear(&self, color: PyColor) -> PyResult<()> {
        self.with_builder(|builder| builder.clear(color.into()).map(|_| ()))
    }

    pub fn fill_rect(&self, rect: PyRect, color: PyColor) -> PyResult<()> {
        self.with_builder(|builder| {
            builder
                .fill_rect(rect.into(), Color::from(color))
                .map(|_| ())
        })
    }

    #[pyo3(signature = (rect, color, width=1.0))]
    pub fn stroke_rect(&self, rect: PyRect, color: PyColor, width: f32) -> PyResult<()> {
        self.with_builder(|builder| {
            builder
                .stroke_rect(rect.into(), Color::from(color), StrokeStyle::new(width))
                .map(|_| ())
        })
    }

    pub fn fill_path(&self, path: PyRef<'_, PyPath>, color: PyColor) -> PyResult<()> {
        self.with_builder(|builder| {
            builder
                .fill_path(path.inner.clone(), Color::from(color))
                .map(|_| ())
        })
    }

    #[pyo3(signature = (path, color, width=1.0))]
    pub fn stroke_path(&self, path: PyRef<'_, PyPath>, color: PyColor, width: f32) -> PyResult<()> {
        self.with_builder(|builder| {
            builder
                .stroke_path(
                    path.inner.clone(),
                    Color::from(color),
                    StrokeStyle::new(width),
                )
                .map(|_| ())
        })
    }

    #[pyo3(signature = (rect, color, radii=None))]
    pub fn fill_rounded_rect(
        &self,
        rect: PyRect,
        color: PyColor,
        radii: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let radii = py_radii(radii)?;
        self.with_builder(|builder| {
            builder
                .fill_rrect(rect.into(), radii, Color::from(color))
                .map(|_| ())
        })
    }

    #[pyo3(signature = (rect, shadow, radii=None))]
    pub fn draw_shadow(
        &self,
        rect: PyRect,
        shadow: PyShadow,
        radii: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let radii = py_radii(radii)?;
        self.with_builder(|builder| {
            builder
                .draw_shadow(rect.into(), radii, shadow.into())
                .map(|_| ())
        })
    }

    #[pyo3(signature = (rect, color, shadow, radii=None))]
    pub fn fill_rounded_rect_with_shadow(
        &self,
        rect: PyRect,
        color: PyColor,
        shadow: PyShadow,
        radii: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let radii = py_radii(radii)?;
        self.with_builder(|builder| {
            builder
                .fill_rrect_with_shadow(rect.into(), radii, Color::from(color), shadow.into())
                .map(|_| ())
        })
    }

    pub fn fill_bounds(&self, color: PyColor) -> PyResult<()> {
        self.fill_rect(self.bounds, color)
    }

    #[pyo3(signature = (
        rect,
        text,
        color=None,
        font_size=None,
        line_height=None,
        font=None,
        weight=None,
        style=None,
        stretch=None
    ))]
    pub fn draw_text(
        &self,
        rect: PyRect,
        text: String,
        color: Option<PyColor>,
        font_size: Option<f32>,
        line_height: Option<f32>,
        font: Option<PyRef<'_, PyFontHandle>>,
        weight: Option<u16>,
        style: Option<&str>,
        stretch: Option<&str>,
    ) -> PyResult<()> {
        let style = py_text_style(color, font_size, line_height, font, weight, style, stretch)?;
        self.with_builder(|builder| builder.draw_text(rect.into(), text, style).map(|_| ()))
    }

    pub fn draw_shader_rect(&self, rect: PyRect, shader: PyRef<'_, PyShader>) -> PyResult<()> {
        self.with_builder(|builder| {
            builder
                .draw_binding_shader_rect(rect.into(), shader.inner)
                .map(|_| ())
        })
    }

    pub fn rgba_image(
        &self,
        slot: u64,
        width: u32,
        height: u32,
        pixels: &Bound<'_, PyAny>,
    ) -> PyResult<PyImageHandle> {
        let image = RegisteredImage::from_rgba8(width, height, pixels.extract::<Vec<u8>>()?)
            .map_err(py_runtime_error)?;
        recover_lock(&self.images).push(PendingPaintImage { slot, image });
        Ok(PyImageHandle {
            inner: BindingImageHandle::local(slot),
        })
    }

    pub fn draw_image(&self, rect: PyRect, image: PyRef<'_, PyImageHandle>) -> PyResult<()> {
        self.with_builder(|builder| {
            builder
                .draw_binding_image(rect.into(), image.inner)
                .map(|_| ())
        })
    }

    pub fn draw_image_quad(
        &self,
        points: &Bound<'_, PyAny>,
        image: PyRef<'_, PyImageHandle>,
    ) -> PyResult<()> {
        let points = py_four_points(points)?;
        self.with_builder(|builder| {
            builder
                .draw_binding_image_quad(points, image.inner)
                .map(|_| ())
        })
    }

    pub fn push_clip_rect(&self, rect: PyRect) -> PyResult<()> {
        self.with_builder(|builder| builder.push_clip_rect(rect.into()).map(|_| ()))
    }

    pub fn push_clip_path(&self, path: PyRef<'_, PyPath>) -> PyResult<()> {
        self.with_builder(|builder| builder.push_clip_path(path.inner.clone()).map(|_| ()))
    }

    pub fn pop_clip(&self) -> PyResult<()> {
        self.with_builder(|builder| builder.pop_clip().map(|_| ()))
    }

    pub fn push_transform(&self, transform: PyTransform) -> PyResult<()> {
        self.with_builder(|builder| builder.push_transform(transform.into()).map(|_| ()))
    }

    pub fn pop_transform(&self) -> PyResult<()> {
        self.with_builder(|builder| builder.pop_transform().map(|_| ()))
    }

    pub fn command_count(&self) -> usize {
        recover_lock(&self.builder).command_count()
    }
}

impl PyPaint {
    fn new(bounds: Rect) -> Self {
        Self {
            builder: Arc::new(Mutex::new(PaintCommandBuilder::new())),
            images: Arc::new(Mutex::new(Vec::new())),
            bounds: bounds.into(),
        }
    }

    fn with_builder(
        &self,
        f: impl FnOnce(&mut PaintCommandBuilder) -> Result<(), PaintValidationError>,
    ) -> PyResult<()> {
        f(&mut recover_lock(&self.builder)).map_err(py_value_error)
    }

    fn finish(&self) -> Result<Vec<PaintCommand>, PaintValidationError> {
        std::mem::take(&mut *recover_lock(&self.builder)).finish()
    }

    fn take_images(&self) -> Vec<PendingPaintImage> {
        std::mem::take(&mut *recover_lock(&self.images))
    }
}

#[derive(Clone)]
enum PySemanticsCommand {
    Node(SemanticsNode),
    Child(usize),
}

#[pyclass(name = "Semantics", module = "sui", skip_from_py_object)]
#[derive(Clone)]
pub struct PySemantics {
    widget_id: WidgetId,
    commands: Arc<Mutex<Vec<PySemanticsCommand>>>,
    bounds: PyRect,
    focused: bool,
    child_count: usize,
}

#[pymethods]
impl PySemantics {
    #[getter]
    pub const fn bounds(&self) -> PyRect {
        self.bounds
    }

    #[getter]
    pub const fn focused(&self) -> bool {
        self.focused
    }

    #[getter]
    pub const fn child_count(&self) -> usize {
        self.child_count
    }

    #[pyo3(signature = (
        role="generic_container",
        name=None,
        value=None,
        description=None,
        bounds=None,
        disabled=false,
        focused=None,
        hidden=false,
        hovered=false,
        checked=None,
        selected=false,
        expanded=None,
        busy=false,
        min_value=None,
        max_value=None
    ))]
    pub fn node(
        &self,
        role: &str,
        name: Option<String>,
        value: Option<&Bound<'_, PyAny>>,
        description: Option<String>,
        bounds: Option<PyRect>,
        disabled: bool,
        focused: Option<bool>,
        hidden: bool,
        hovered: bool,
        checked: Option<&Bound<'_, PyAny>>,
        selected: bool,
        expanded: Option<bool>,
        busy: bool,
        min_value: Option<f64>,
        max_value: Option<f64>,
    ) -> PyResult<()> {
        let role = binding_semantics_role_from_name(role)
            .ok_or_else(|| PyValueError::new_err(format!("unknown semantics role '{role}'")))?;
        let mut node =
            SemanticsNode::new(self.widget_id, role, bounds.unwrap_or(self.bounds).into());
        node.name = name;
        node.description = description;
        node.value = py_semantics_value(value, min_value, max_value)?;
        node.state.disabled = disabled;
        node.state.focused = focused.unwrap_or(self.focused);
        node.state.hidden = hidden;
        node.state.hovered = hovered;
        node.state.checked = py_toggle_state(checked)?;
        node.state.selected = selected;
        node.state.expanded = expanded;
        node.state.busy = busy;
        recover_lock(&self.commands).push(PySemanticsCommand::Node(node));
        Ok(())
    }

    pub fn child(&self, index: usize) -> bool {
        if index >= self.child_count {
            return false;
        }
        recover_lock(&self.commands).push(PySemanticsCommand::Child(index));
        true
    }
}

impl PySemantics {
    fn new(widget_id: WidgetId, bounds: Rect, focused: bool, child_count: usize) -> Self {
        Self {
            widget_id,
            commands: Arc::new(Mutex::new(Vec::new())),
            bounds: bounds.into(),
            focused,
            child_count,
        }
    }

    fn take_commands(&self) -> Vec<PySemanticsCommand> {
        std::mem::take(&mut *recover_lock(&self.commands))
    }
}

#[pyclass(name = "Shader", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyShader {
    inner: BindingShader,
}

#[pymethods]
impl PyShader {
    #[staticmethod]
    pub fn color_wheel() -> Self {
        Self {
            inner: BindingShader::color_wheel(),
        }
    }

    #[staticmethod]
    pub fn hue_bar() -> Self {
        Self {
            inner: BindingShader::hue_bar(),
        }
    }

    #[staticmethod]
    #[pyo3(signature = (hue, max_value=1.0, color_space="srgb"))]
    pub fn saturation_value_plane(hue: f32, max_value: f32, color_space: &str) -> PyResult<Self> {
        BindingShader::saturation_value_plane(parse_color_space(color_space)?, hue, max_value)
            .map(Self::from)
            .map_err(py_value_error)
    }

    #[staticmethod]
    #[pyo3(signature = (hue, value, color_space="srgb"))]
    pub fn saturation_bar(hue: f32, value: f32, color_space: &str) -> PyResult<Self> {
        BindingShader::saturation_bar(parse_color_space(color_space)?, hue, value)
            .map(Self::from)
            .map_err(py_value_error)
    }

    #[staticmethod]
    #[pyo3(signature = (hue, saturation, max_value=1.0, color_space="srgb"))]
    pub fn value_bar(
        hue: f32,
        saturation: f32,
        max_value: f32,
        color_space: &str,
    ) -> PyResult<Self> {
        BindingShader::value_bar(parse_color_space(color_space)?, hue, saturation, max_value)
            .map(Self::from)
            .map_err(py_value_error)
    }

    #[staticmethod]
    pub fn alpha_bar(color: PyColor) -> PyResult<Self> {
        BindingShader::alpha_bar(color.into())
            .map(Self::from)
            .map_err(py_value_error)
    }

    #[staticmethod]
    #[pyo3(signature = (color, channel, max_value=1.0))]
    pub fn rgb_channel_bar(color: PyColor, channel: u32, max_value: f32) -> PyResult<Self> {
        BindingShader::rgb_channel_bar(color.into(), channel, max_value)
            .map(Self::from)
            .map_err(py_value_error)
    }
}

impl From<BindingShader> for PyShader {
    fn from(value: BindingShader) -> Self {
        Self { inner: value }
    }
}

#[pyclass(name = "Widget", module = "sui")]
pub struct PyWidget {
    kind: PyWidgetKind,
}

enum PyWidgetKind {
    Foreign { callbacks: Py<PyAny> },
    Binding(BindingWidget),
}

#[pymethods]
impl PyWidget {
    #[new]
    pub fn new(callbacks: Py<PyAny>) -> Self {
        Self {
            kind: PyWidgetKind::Foreign { callbacks },
        }
    }
}

impl PyWidget {
    fn from_binding(widget: BindingWidget) -> Self {
        Self {
            kind: PyWidgetKind::Binding(widget),
        }
    }

    fn binding_widget(&self) -> PyResult<BindingWidget> {
        match &self.kind {
            PyWidgetKind::Binding(widget) => Ok(widget.clone()),
            PyWidgetKind::Foreign { callbacks } => {
                let callbacks = Python::attach(|py| callbacks.clone_ref(py));
                Ok(BindingWidget::foreign(PyWidgetCallbacks {
                    callbacks: Mutex::new(callbacks),
                }))
            }
        }
    }

    fn into_sui_widget(&self) -> PyResult<ForeignWidget> {
        match &self.kind {
            PyWidgetKind::Foreign { callbacks } => {
                let callbacks = Python::attach(|py| callbacks.clone_ref(py));
                Ok(ForeignWidget::new(PyWidgetCallbacks {
                    callbacks: Mutex::new(callbacks),
                }))
            }
            PyWidgetKind::Binding(_) => Err(PyValueError::new_err(
                "binding widgets should be rendered through sui.App",
            )),
        }
    }
}

#[pyclass(name = "State", module = "sui", from_py_object)]
#[derive(Clone)]
pub struct PyState {
    inner: BindingState,
}

#[pymethods]
impl PyState {
    #[new]
    pub fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: BindingState::new(binding_value_from_py(value)?),
        })
    }

    pub fn get(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        binding_value_to_py(py, self.inner.get())
    }

    pub fn set(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.set(binding_value_from_py(value)?);
        Ok(())
    }
}

#[pyclass(name = "Window", module = "sui", from_py_object)]
#[derive(Clone)]
pub struct PyWindow {
    title: String,
    root: Option<BindingWidget>,
}

#[pymethods]
impl PyWindow {
    #[new]
    pub fn new(title: String) -> Self {
        Self { title, root: None }
    }

    pub fn root(&self, widget: PyRef<'_, PyWidget>) -> PyResult<Self> {
        Ok(Self {
            title: self.title.clone(),
            root: Some(widget.binding_widget()?),
        })
    }
}

impl PyWindow {
    fn to_binding(&self) -> PyResult<BindingWindow> {
        let root = self
            .root
            .clone()
            .ok_or_else(|| PyValueError::new_err("window root has not been set"))?;
        Ok(BindingWindow::new(self.title.clone(), root))
    }
}

#[pyclass(name = "App", module = "sui")]
pub struct PyApp {
    inner: BindingApp,
}

#[pymethods]
impl PyApp {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: BindingApp::new(),
        }
    }

    pub fn window(&mut self, window: PyRef<'_, PyWindow>) -> PyResult<()> {
        self.inner.push_window(window.to_binding()?);
        Ok(())
    }

    #[pyo3(signature = (index=0))]
    pub fn render(&self, index: usize) -> PyResult<PyRenderSnapshot> {
        self.inner
            .render_window(index)
            .map(PyRenderSnapshot::from)
            .map_err(py_runtime_error)
    }

    pub fn start(&self) -> PyResult<PyRunningApp> {
        self.inner
            .start()
            .map(PyRunningApp::new)
            .map_err(py_runtime_error)
    }

    pub fn rgba_image(
        &mut self,
        width: u32,
        height: u32,
        pixels: &Bound<'_, PyAny>,
    ) -> PyResult<PyImageHandle> {
        self.inner
            .register_rgba_image(width, height, pixels.extract::<Vec<u8>>()?)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn png_image(&mut self, png: &Bound<'_, PyAny>) -> PyResult<PyImageHandle> {
        self.inner
            .register_png_image(png.extract::<Vec<u8>>()?)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn png_file(&mut self, path: &str) -> PyResult<PyImageHandle> {
        let data = fs::read(path).map_err(|error| {
            PyOSError::new_err(format!("failed to read PNG file '{path}': {error}"))
        })?;
        self.inner
            .register_png_image(data)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn svg_image(&mut self, svg: &Bound<'_, PyAny>) -> PyResult<PyImageHandle> {
        self.inner
            .register_svg_image(svg.extract::<Vec<u8>>()?)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn svg_file(&mut self, path: &str) -> PyResult<PyImageHandle> {
        let data = fs::read(path).map_err(|error| {
            PyOSError::new_err(format!("failed to read SVG file '{path}': {error}"))
        })?;
        self.inner
            .register_svg_image(data)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        svg: &Bound<'_, PyAny>,
    ) -> PyResult<PyImageHandle> {
        self.inner
            .register_svg_image_at_size(width, height, svg.extract::<Vec<u8>>()?)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn svg_file_at_size(
        &mut self,
        width: u32,
        height: u32,
        path: &str,
    ) -> PyResult<PyImageHandle> {
        let data = fs::read(path).map_err(|error| {
            PyOSError::new_err(format!("failed to read SVG file '{path}': {error}"))
        })?;
        self.inner
            .register_svg_image_at_size(width, height, data)
            .map(|inner| PyImageHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn font_bytes(&mut self, data: &Bound<'_, PyAny>) -> PyResult<PyFontHandle> {
        self.inner
            .register_font_bytes(data.extract::<Vec<u8>>()?)
            .map(|inner| PyFontHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn font_file(&mut self, path: &str) -> PyResult<PyFontHandle> {
        let data = fs::read(path).map_err(|error| {
            PyOSError::new_err(format!("failed to read font file '{path}': {error}"))
        })?;
        self.inner
            .register_font_bytes(data)
            .map(|inner| PyFontHandle { inner })
            .map_err(py_runtime_error)
    }

    pub fn run(&self, py: Python<'_>) -> PyResult<()> {
        let app = self.inner.clone();
        py.detach(move || app.run()).map_err(py_runtime_error)
    }

    pub fn run_with_handle(&self, py: Python<'_>, callback: Py<PyAny>) -> PyResult<()> {
        let app = self.inner.clone();
        let callback_error = Arc::new(Mutex::new(None::<String>));
        let callback_error_for_run = Arc::clone(&callback_error);

        let result = py.detach(move || {
            app.run_with_handle(move |handle| {
                Python::attach(|py| {
                    let call_result = Py::new(py, PyUiHandle { inner: handle })
                        .and_then(|handle| callback.call1(py, (handle,)).map(|_| ()));
                    if let Err(error) = call_result {
                        *recover_lock(&callback_error_for_run) = Some(error.to_string());
                    }
                });
            })
        });

        result.map_err(py_runtime_error)?;
        if let Some(error) = recover_lock(&callback_error).take() {
            return Err(PyRuntimeError::new_err(error));
        }
        Ok(())
    }

    pub fn window_count(&self) -> usize {
        self.inner.window_count()
    }

    pub fn image_resource_count(&self) -> usize {
        self.inner.image_resource_count()
    }

    pub fn font_resource_count(&self) -> usize {
        self.inner.font_resource_count()
    }
}

impl Default for PyApp {
    fn default() -> Self {
        Self::new()
    }
}

#[pyclass(name = "WindowHandle", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyWindowHandle {
    inner: BindingWindowId,
}

#[pymethods]
impl PyWindowHandle {
    #[new]
    pub fn new(raw: u64) -> Self {
        Self {
            inner: BindingWindowId::new(raw),
        }
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.inner.get()
    }

    fn __repr__(&self) -> String {
        format!("WindowHandle({})", self.inner.get())
    }
}

impl From<BindingWindowId> for PyWindowHandle {
    fn from(value: BindingWindowId) -> Self {
        Self { inner: value }
    }
}

#[pyclass(name = "UiHandle", module = "sui", skip_from_py_object)]
#[derive(Clone)]
pub struct PyUiHandle {
    inner: BindingUiHandle,
}

#[pymethods]
impl PyUiHandle {
    pub fn post(&self, callback: Py<PyAny>) {
        self.inner.post(move || {
            Python::attach(|py| {
                let _ = callback.call0(py);
            });
        });
    }

    pub fn pending_count(&self) -> usize {
        self.inner.pending_count()
    }
}

impl From<BindingUiHandle> for PyUiHandle {
    fn from(value: BindingUiHandle) -> Self {
        Self { inner: value }
    }
}

#[pyclass(name = "RunningApp", module = "sui", unsendable, skip_from_py_object)]
pub struct PyRunningApp {
    inner: RefCell<BindingRuntime>,
}

#[pymethods]
impl PyRunningApp {
    pub fn ui_handle(&self) -> PyUiHandle {
        self.inner.borrow().ui_handle().into()
    }

    pub fn drain(&self) -> PyResult<usize> {
        self.inner
            .borrow_mut()
            .drain_ui_tasks()
            .map_err(py_runtime_error)
    }

    #[pyo3(signature = (index=0))]
    pub fn render(&self, index: usize) -> PyResult<PyRenderSnapshot> {
        self.inner
            .borrow_mut()
            .render_window_at(index)
            .map(PyRenderSnapshot::from)
            .map_err(py_runtime_error)
    }

    pub fn render_window(&self, window: PyRef<'_, PyWindowHandle>) -> PyResult<PyRenderSnapshot> {
        self.inner
            .borrow_mut()
            .render_window(window.inner)
            .map(PyRenderSnapshot::from)
            .map_err(py_runtime_error)
    }

    #[pyo3(signature = (index=0))]
    pub fn needs_render(&self, index: usize) -> PyResult<bool> {
        let runtime = self.inner.borrow();
        let window_id = runtime.window_id_at(index).map_err(py_runtime_error)?;
        runtime.needs_render(window_id).map_err(py_runtime_error)
    }

    #[pyo3(signature = (index=0))]
    pub fn request_redraw(&self, index: usize) -> PyResult<()> {
        let mut runtime = self.inner.borrow_mut();
        let window_id = runtime.window_id_at(index).map_err(py_runtime_error)?;
        runtime.request_redraw(window_id).map_err(py_runtime_error)
    }

    #[pyo3(signature = (event, index=0))]
    pub fn handle_event(&self, event: PyRef<'_, PyEvent>, index: usize) -> PyResult<()> {
        self.inner
            .borrow_mut()
            .handle_event_at(index, event.binding_event())
            .map_err(py_runtime_error)
    }

    pub fn window_count(&self) -> usize {
        self.inner.borrow().window_count()
    }

    pub fn window_ids(&self) -> Vec<u64> {
        self.inner
            .borrow()
            .window_ids()
            .into_iter()
            .map(BindingWindowId::get)
            .collect()
    }

    pub fn window_handle(&self, index: usize) -> PyResult<PyWindowHandle> {
        self.inner
            .borrow()
            .window_id_at(index)
            .map(PyWindowHandle::from)
            .map_err(py_runtime_error)
    }

    pub fn pending_count(&self) -> usize {
        self.inner.borrow().pending_ui_task_count()
    }
}

impl PyRunningApp {
    fn new(runtime: BindingRuntime) -> Self {
        Self {
            inner: RefCell::new(runtime),
        }
    }
}

#[pyclass(
    name = "RendererInteropCapabilities",
    frozen,
    module = "sui",
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub struct PyRendererInteropCapabilities {
    inner: RendererInteropCapabilities,
}

#[pymethods]
impl PyRendererInteropCapabilities {
    #[new]
    #[pyo3(signature = (backend, cpu_upload=true, shared_texture=false, shared_render_target=false))]
    pub fn new(
        backend: &str,
        cpu_upload: bool,
        shared_texture: bool,
        shared_render_target: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: RendererInteropCapabilities {
                backend: parse_native_backend(backend)?,
                cpu_upload,
                shared_texture,
                shared_render_target,
            },
        })
    }

    #[staticmethod]
    pub fn cpu_only(backend: &str) -> PyResult<Self> {
        Ok(Self {
            inner: RendererInteropCapabilities::cpu_only(parse_native_backend(backend)?),
        })
    }

    pub fn supports(&self, tier: &str) -> PyResult<bool> {
        Ok(self.inner.supports(parse_interop_tier(tier)?))
    }

    #[getter]
    pub fn backend(&self) -> &'static str {
        native_backend_name(self.inner.backend)
    }

    #[getter]
    pub fn cpu_upload(&self) -> bool {
        self.inner.cpu_upload
    }

    #[getter]
    pub fn shared_texture(&self) -> bool {
        self.inner.shared_texture
    }

    #[getter]
    pub fn shared_render_target(&self) -> bool {
        self.inner.shared_render_target
    }
}

#[pyclass(
    name = "ExternalBackendHandle",
    frozen,
    module = "sui",
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub struct PyExternalBackendHandle {
    inner: ExternalBackendHandle,
}

#[pymethods]
impl PyExternalBackendHandle {
    #[new]
    pub fn new(id: u64) -> Self {
        Self {
            inner: ExternalBackendHandle::new(id),
        }
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.inner.id()
    }

    #[getter]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[pyclass(name = "ExternalSync", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyExternalSync {
    inner: ExternalSync,
}

#[pymethods]
impl PyExternalSync {
    #[staticmethod]
    pub fn none() -> Self {
        Self {
            inner: ExternalSync::None,
        }
    }

    #[staticmethod]
    pub fn generation(generation: u64) -> Self {
        Self {
            inner: ExternalSync::Generation(generation),
        }
    }

    #[staticmethod]
    pub fn timeline_value(handle: PyRef<'_, PyExternalBackendHandle>, value: u64) -> Self {
        Self {
            inner: ExternalSync::TimelineValue {
                handle: handle.inner,
                value,
            },
        }
    }

    #[staticmethod]
    pub fn fence(handle: PyRef<'_, PyExternalBackendHandle>) -> Self {
        Self {
            inner: ExternalSync::Fence {
                handle: handle.inner,
            },
        }
    }

    #[getter]
    pub fn kind(&self) -> &'static str {
        match self.inner {
            ExternalSync::None => "none",
            ExternalSync::Generation(_) => "generation",
            ExternalSync::TimelineValue { .. } => "timeline_value",
            ExternalSync::Fence { .. } => "fence",
        }
    }
}

#[pyclass(
    name = "ExternalTextureDescriptor",
    frozen,
    module = "sui",
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyExternalTextureDescriptor {
    inner: ExternalTextureDescriptor,
}

#[pymethods]
impl PyExternalTextureDescriptor {
    #[staticmethod]
    #[pyo3(signature = (size, pixels, generation=0))]
    pub fn cpu_rgba8(size: PySize, pixels: &Bound<'_, PyAny>, generation: u64) -> PyResult<Self> {
        let pixels = pixels.extract::<Vec<u8>>().map_err(|_| {
            PyValueError::new_err("pixels must be a bytes-like object or sequence of bytes")
        })?;
        Ok(Self {
            inner: ExternalTextureDescriptor::cpu_rgba8(size.into(), pixels, generation),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (backend, size, format, handle, sync, color_space="srgb"))]
    pub fn shared_texture(
        backend: &str,
        size: PySize,
        format: &str,
        handle: PyRef<'_, PyExternalBackendHandle>,
        sync: PyRef<'_, PyExternalSync>,
        color_space: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::SharedTexture {
                backend: parse_native_backend(backend)?,
                size: size.into(),
                format: parse_external_texture_format(format)?,
                color_space: parse_color_space(color_space)?,
                handle: handle.inner,
                sync: sync.inner,
            },
        })
    }

    #[staticmethod]
    #[pyo3(signature = (backend, size, format, handle, sync, color_space="srgb"))]
    pub fn shared_render_target(
        backend: &str,
        size: PySize,
        format: &str,
        handle: PyRef<'_, PyExternalBackendHandle>,
        sync: PyRef<'_, PyExternalSync>,
        color_space: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::SharedRenderTarget {
                backend: parse_native_backend(backend)?,
                size: size.into(),
                format: parse_external_texture_format(format)?,
                color_space: parse_color_space(color_space)?,
                handle: handle.inner,
                sync: sync.inner,
            },
        })
    }

    pub fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(py_external_texture_error)
    }

    #[getter]
    pub fn tier(&self) -> &'static str {
        interop_tier_name(self.inner.tier())
    }

    #[getter]
    pub fn size(&self) -> PySize {
        self.inner.size().into()
    }
}

struct PyWidgetCallbacks {
    callbacks: Mutex<Py<PyAny>>,
}

impl ForeignWidgetCallbacks for PyWidgetCallbacks {
    fn debug_name(&self, _id: sui_bindings_core::ForeignWidgetId) -> &'static str {
        "sui_python::Widget"
    }

    fn event(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignEventCtx<'_>,
        event: &Event,
    ) -> ForeignCallbackResult<()> {
        Python::attach(|py| {
            let callbacks = recover_lock(&self.callbacks);
            let object = callbacks.bind(py);
            if !object.hasattr("event").map_err(foreign_py_error)? {
                return Ok(());
            }
            let py_event = Py::new(py, PyEvent::from_binding(BindingEvent::from(event)))
                .map_err(foreign_py_error)?;
            let result = object
                .call_method1("event", (py_event,))
                .map_err(foreign_py_error)?;
            if result.extract::<bool>().unwrap_or(false) {
                ctx.set_handled();
                ctx.request_paint();
            }
            Ok(())
        })
    }

    fn measure(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        Python::attach(|py| {
            let callbacks = recover_lock(&self.callbacks);
            let object = callbacks.bind(py);
            if !object.hasattr("measure").map_err(foreign_py_error)? {
                return Ok(constraints.clamp(Size::new(0.0, 0.0)));
            }
            let value = object
                .call_method1("measure", (PyConstraints::from(constraints),))
                .map_err(foreign_py_error)?;
            extract_size(&value).map_err(foreign_py_error)
        })
    }

    fn paint(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        Python::attach(|py| {
            let callbacks = recover_lock(&self.callbacks);
            let object = callbacks.bind(py);
            if !object.hasattr("paint").map_err(foreign_py_error)? {
                return Ok(());
            }
            let paint = PyPaint::new(ctx.bounds());
            let py_paint = Py::new(py, paint.clone()).map_err(foreign_py_error)?;
            object
                .call_method1("paint", (py_paint.clone_ref(py),))
                .map_err(foreign_py_error)?;
            let mut commands = paint.finish().map_err(ForeignCallbackFailure::from)?;
            resolve_binding_image_slots(&mut commands, |slot| ctx.widget_image_handle(slot));
            for pending in paint.take_images() {
                ctx.register_image(ctx.widget_image_handle(pending.slot), pending.image);
            }
            ctx.apply_all(commands)
                .map_err(ForeignCallbackFailure::from)
        })
    }

    fn semantics(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignSemanticsCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        Python::attach(|py| {
            let callbacks = recover_lock(&self.callbacks);
            let object = callbacks.bind(py);
            if object.hasattr("semantics").map_err(foreign_py_error)? {
                let semantics = PySemantics::new(
                    ctx.widget_id(),
                    ctx.bounds(),
                    ctx.is_focused(),
                    ctx.child_count(),
                );
                let py_semantics = Py::new(py, semantics.clone()).map_err(foreign_py_error)?;
                object
                    .call_method1("semantics", (py_semantics,))
                    .map_err(foreign_py_error)?;
                for command in semantics.take_commands() {
                    match command {
                        PySemanticsCommand::Node(node) => ctx.push(node),
                        PySemanticsCommand::Child(index) => {
                            ctx.semantics_child(index);
                        }
                    }
                }
                return Ok(());
            }

            let name = if object.hasattr("name").map_err(foreign_py_error)? {
                Some(
                    object
                        .getattr("name")
                        .and_then(|value| value.extract::<String>())
                        .map_err(foreign_py_error)?,
                )
            } else {
                None
            };
            if name.is_some() {
                let mut node = SemanticsNode::new(
                    ctx.widget_id(),
                    SemanticsRole::GenericContainer,
                    ctx.bounds(),
                );
                node.name = name;
                ctx.push(node);
            }
            Ok(())
        })
    }
}

#[pyclass(name = "UiTaskQueue", module = "sui", skip_from_py_object)]
#[derive(Clone)]
pub struct PyUiTaskQueue {
    inner: UiTaskQueue,
}

#[pymethods]
impl PyUiTaskQueue {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: UiTaskQueue::new(),
        }
    }

    pub fn post(&self, callback: Py<PyAny>) {
        self.inner.post(move || {
            Python::attach(|py| {
                let _ = callback.call0(py);
            });
        });
    }

    pub fn drain(&self) -> usize {
        self.inner.drain()
    }

    pub fn pending_count(&self) -> usize {
        self.inner.pending_count()
    }
}

impl Default for PyUiTaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[pyclass(name = "RenderSnapshot", frozen, module = "sui", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRenderSnapshot {
    #[pyo3(get)]
    pub command_count: usize,
    #[pyo3(get)]
    pub semantics_count: usize,
    #[pyo3(get)]
    pub semantics_roles: Vec<String>,
    #[pyo3(get)]
    pub semantics_names: Vec<String>,
    #[pyo3(get)]
    pub semantics_values: Vec<String>,
    #[pyo3(get)]
    pub semantics_descriptions: Vec<String>,
    #[pyo3(get)]
    pub semantics_checked: Vec<String>,
    #[pyo3(get)]
    pub semantics_busy: Vec<bool>,
    #[pyo3(get)]
    pub semantics_editable_multiline: Vec<bool>,
    #[pyo3(get)]
    pub semantics_disabled: Vec<bool>,
    #[pyo3(get)]
    pub semantics_focused: Vec<bool>,
    #[pyo3(get)]
    pub semantics_hidden: Vec<bool>,
    #[pyo3(get)]
    pub semantics_hovered: Vec<bool>,
    #[pyo3(get)]
    pub semantics_selected: Vec<bool>,
    #[pyo3(get)]
    pub semantics_expanded: Vec<String>,
    #[pyo3(get)]
    pub fill_rect_count: usize,
    #[pyo3(get)]
    pub draw_image_count: usize,
    #[pyo3(get)]
    pub registered_font_count: usize,
    #[pyo3(get)]
    pub registered_image_count: usize,
}

impl From<BindingRenderSnapshot> for PyRenderSnapshot {
    fn from(value: BindingRenderSnapshot) -> Self {
        Self {
            command_count: value.command_count,
            semantics_count: value.semantics_count,
            semantics_roles: value.semantics_roles,
            semantics_names: value.semantics_names,
            semantics_values: value.semantics_values,
            semantics_descriptions: value.semantics_descriptions,
            semantics_checked: value.semantics_checked,
            semantics_busy: value.semantics_busy,
            semantics_editable_multiline: value.semantics_editable_multiline,
            semantics_disabled: value.semantics_disabled,
            semantics_focused: value.semantics_focused,
            semantics_hidden: value.semantics_hidden,
            semantics_hovered: value.semantics_hovered,
            semantics_selected: value.semantics_selected,
            semantics_expanded: value.semantics_expanded,
            fill_rect_count: value.fill_rect_count,
            draw_image_count: value.draw_image_count,
            registered_font_count: value.registered_font_count,
            registered_image_count: value.registered_image_count,
        }
    }
}

#[pyfunction]
#[pyo3(signature = (widget, event=None))]
pub fn render_widget(
    widget: PyRef<'_, PyWidget>,
    event: Option<PyRef<'_, PyEvent>>,
) -> PyResult<PyRenderSnapshot> {
    if let Ok(binding) = widget.binding_widget() {
        let app = BindingApp::new().with_window(BindingWindow::new("Python widget", binding));
        if let Some(event) = event {
            let mut runtime = app.start().map_err(py_runtime_error)?;
            let window_id = runtime.window_id_at(0).map_err(py_runtime_error)?;
            let _ = runtime.render_window(window_id).map_err(py_runtime_error)?;
            runtime
                .handle_event(window_id, event.binding_event())
                .map_err(py_runtime_error)?;
            return runtime
                .render_window(window_id)
                .map(PyRenderSnapshot::from)
                .map_err(py_runtime_error);
        }
        return app
            .render_window(0)
            .map(PyRenderSnapshot::from)
            .map_err(py_runtime_error);
    }
    let mut runtime = RuntimeApplication::new()
        .window(
            WindowBuilder::new()
                .title("Python widget")
                .root(widget.into_sui_widget()?),
        )
        .build()
        .map_err(py_runtime_error)?;
    let window_id = runtime.window_ids()[0];
    let mut output = runtime.render(window_id).map_err(py_runtime_error)?;
    if let Some(event) = event {
        runtime
            .handle_event(
                window_id,
                event
                    .binding_event()
                    .into_sui_event()
                    .map_err(py_runtime_error)?,
            )
            .map_err(py_runtime_error)?;
        output = runtime.render(window_id).map_err(py_runtime_error)?;
    }
    let mut command_count = 0;
    let mut fill_rect_count = 0;
    let mut draw_image_count = 0;
    output.frame.scene.visit_commands(&mut |command| {
        command_count += 1;
        match command {
            SceneCommand::FillRect { .. } => fill_rect_count += 1,
            SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                draw_image_count += 1;
            }
            _ => {}
        }
    });
    Ok(PyRenderSnapshot {
        command_count,
        semantics_count: output.semantics.len(),
        semantics_roles: binding_semantics_roles(&output.semantics),
        semantics_names: binding_semantics_names(&output.semantics),
        semantics_values: binding_semantics_values(&output.semantics),
        semantics_descriptions: binding_semantics_descriptions(&output.semantics),
        semantics_checked: binding_semantics_checked(&output.semantics),
        semantics_busy: binding_semantics_busy(&output.semantics),
        semantics_editable_multiline: binding_semantics_editable_multiline(&output.semantics),
        semantics_disabled: binding_semantics_disabled(&output.semantics),
        semantics_focused: binding_semantics_focused(&output.semantics),
        semantics_hidden: binding_semantics_hidden(&output.semantics),
        semantics_hovered: binding_semantics_hovered(&output.semantics),
        semantics_selected: binding_semantics_selected(&output.semantics),
        semantics_expanded: binding_semantics_expanded(&output.semantics),
        fill_rect_count,
        draw_image_count,
        registered_font_count: output.frame.font_registry.len(),
        registered_image_count: output.frame.image_registry.len(),
    })
}

#[pymodule]
fn sui(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPoint>()?;
    m.add_class::<PyModifiers>()?;
    m.add_class::<PyEvent>()?;
    m.add_class::<PySize>()?;
    m.add_class::<PyRect>()?;
    m.add_class::<PyPath>()?;
    m.add_class::<PyPathBuilder>()?;
    m.add_class::<PyTransform>()?;
    m.add_class::<PyColor>()?;
    m.add_class::<PyShadow>()?;
    m.add_class::<PyConstraints>()?;
    m.add_class::<PyFontHandle>()?;
    m.add_class::<PyImageHandle>()?;
    m.add_class::<PyPaint>()?;
    m.add_class::<PySemantics>()?;
    m.add_class::<PyShader>()?;
    m.add_class::<PyWidget>()?;
    m.add_class::<PyState>()?;
    m.add_class::<PyWindow>()?;
    m.add_class::<PyApp>()?;
    m.add_class::<PyWindowHandle>()?;
    m.add_class::<PyUiHandle>()?;
    m.add_class::<PyRunningApp>()?;
    m.add_class::<PyRendererInteropCapabilities>()?;
    m.add_class::<PyExternalBackendHandle>()?;
    m.add_class::<PyExternalSync>()?;
    m.add_class::<PyExternalTextureDescriptor>()?;
    m.add_class::<PyUiTaskQueue>()?;
    m.add_class::<PyRenderSnapshot>()?;
    m.add_class::<PyTextSpan>()?;
    m.add_class::<PyStatusBarSegment>()?;
    m.add_class::<PySegmentedControlItem>()?;
    m.add_class::<PyTableColumn>()?;
    m.add_class::<PyTableRow>()?;
    m.add_class::<PyTreeItem>()?;
    m.add_class::<PyLayerListItem>()?;
    m.add_class::<PyMenuItem>()?;
    m.add_class::<PyToolPaletteItem>()?;
    m.add_class::<PyColorPaletteSwatch>()?;
    m.add_class::<PyBrushPreviewSpec>()?;
    m.add_class::<PyFloatingStackWindow>()?;
    m.add_function(wrap_pyfunction!(py_label, m)?)?;
    m.add_function(wrap_pyfunction!(py_button, m)?)?;
    m.add_function(wrap_pyfunction!(py_icon, m)?)?;
    m.add_function(wrap_pyfunction!(py_icon_button, m)?)?;
    m.add_function(wrap_pyfunction!(py_link, m)?)?;
    m.add_function(wrap_pyfunction!(py_checkbox, m)?)?;
    m.add_function(wrap_pyfunction!(py_switch, m)?)?;
    m.add_function(wrap_pyfunction!(py_radio_button, m)?)?;
    m.add_function(wrap_pyfunction!(py_radio_group, m)?)?;
    m.add_function(wrap_pyfunction!(py_segmented_control, m)?)?;
    m.add_function(wrap_pyfunction!(py_breadcrumb, m)?)?;
    m.add_function(wrap_pyfunction!(py_path_bar, m)?)?;
    m.add_function(wrap_pyfunction!(py_list_view, m)?)?;
    m.add_function(wrap_pyfunction!(py_table, m)?)?;
    m.add_function(wrap_pyfunction!(py_data_grid, m)?)?;
    m.add_function(wrap_pyfunction!(py_slider, m)?)?;
    m.add_function(wrap_pyfunction!(py_number_input, m)?)?;
    m.add_function(wrap_pyfunction!(py_select, m)?)?;
    m.add_function(wrap_pyfunction!(py_progress_bar, m)?)?;
    m.add_function(wrap_pyfunction!(py_signal_meter, m)?)?;
    m.add_function(wrap_pyfunction!(py_status_badge, m)?)?;
    m.add_function(wrap_pyfunction!(py_status_bar, m)?)?;
    m.add_function(wrap_pyfunction!(py_detail_row, m)?)?;
    m.add_function(wrap_pyfunction!(py_busy_indicator, m)?)?;
    m.add_function(wrap_pyfunction!(py_action_card, m)?)?;
    m.add_function(wrap_pyfunction!(py_brush_preview, m)?)?;
    m.add_function(wrap_pyfunction!(py_command_group, m)?)?;
    m.add_function(wrap_pyfunction!(py_coverage_dots, m)?)?;
    m.add_function(wrap_pyfunction!(py_dock, m)?)?;
    m.add_function(wrap_pyfunction!(py_fixed_pane_split, m)?)?;
    m.add_function(wrap_pyfunction!(py_framed_field, m)?)?;
    m.add_function(wrap_pyfunction!(py_measured_bottom_dock, m)?)?;
    m.add_function(wrap_pyfunction!(py_placement_badge, m)?)?;
    m.add_function(wrap_pyfunction!(py_property_row, m)?)?;
    m.add_function(wrap_pyfunction!(py_section_label, m)?)?;
    m.add_function(wrap_pyfunction!(py_side_sheet, m)?)?;
    m.add_function(wrap_pyfunction!(py_split_view, m)?)?;
    m.add_function(wrap_pyfunction!(py_switch_view, m)?)?;
    m.add_function(wrap_pyfunction!(py_trailing_slot_row, m)?)?;
    m.add_function(wrap_pyfunction!(py_floating_stack, m)?)?;
    m.add_function(wrap_pyfunction!(py_virtual_scroll_view, m)?)?;
    m.add_function(wrap_pyfunction!(py_reorderable_list, m)?)?;
    m.add_function(wrap_pyfunction!(py_text_input, m)?)?;
    m.add_function(wrap_pyfunction!(py_password_input, m)?)?;
    m.add_function(wrap_pyfunction!(py_datetime_input, m)?)?;
    m.add_function(wrap_pyfunction!(py_text_area, m)?)?;
    m.add_function(wrap_pyfunction!(py_rich_text, m)?)?;
    m.add_function(wrap_pyfunction!(py_image, m)?)?;
    m.add_function(wrap_pyfunction!(py_color_swatch, m)?)?;
    m.add_function(wrap_pyfunction!(py_separator, m)?)?;
    m.add_function(wrap_pyfunction!(py_empty_state, m)?)?;
    m.add_function(wrap_pyfunction!(py_surface, m)?)?;
    m.add_function(wrap_pyfunction!(py_toolbar, m)?)?;
    m.add_function(wrap_pyfunction!(py_column, m)?)?;
    m.add_function(wrap_pyfunction!(py_row, m)?)?;
    m.add_function(wrap_pyfunction!(py_scroll_view, m)?)?;
    m.add_function(wrap_pyfunction!(py_external_surface, m)?)?;
    m.add_function(wrap_pyfunction!(py_tree_view, m)?)?;
    m.add_function(wrap_pyfunction!(py_layer_list, m)?)?;
    m.add_function(wrap_pyfunction!(py_menu, m)?)?;
    m.add_function(wrap_pyfunction!(py_context_menu, m)?)?;
    m.add_function(wrap_pyfunction!(py_tab_bar, m)?)?;
    m.add_function(wrap_pyfunction!(py_tabs, m)?)?;
    m.add_function(wrap_pyfunction!(py_dialog, m)?)?;
    m.add_function(wrap_pyfunction!(py_padding, m)?)?;
    m.add_function(wrap_pyfunction!(py_align, m)?)?;
    m.add_function(wrap_pyfunction!(py_background, m)?)?;
    m.add_function(wrap_pyfunction!(py_sized_box, m)?)?;
    m.add_function(wrap_pyfunction!(py_stack, m)?)?;
    m.add_function(wrap_pyfunction!(py_semantic_region, m)?)?;
    m.add_function(wrap_pyfunction!(py_form_row, m)?)?;
    m.add_function(wrap_pyfunction!(py_field_group, m)?)?;
    m.add_function(wrap_pyfunction!(py_form_section, m)?)?;
    m.add_function(wrap_pyfunction!(py_panel_section, m)?)?;
    m.add_function(wrap_pyfunction!(py_dock_panel, m)?)?;
    m.add_function(wrap_pyfunction!(py_status_bar_host, m)?)?;
    m.add_function(wrap_pyfunction!(py_tooltip, m)?)?;
    m.add_function(wrap_pyfunction!(py_popover, m)?)?;
    m.add_function(wrap_pyfunction!(py_tool_palette, m)?)?;
    m.add_function(wrap_pyfunction!(py_preset_strip, m)?)?;
    m.add_function(wrap_pyfunction!(py_browser_tab_bar, m)?)?;
    m.add_function(wrap_pyfunction!(py_color_palette, m)?)?;
    m.add_function(wrap_pyfunction!(py_color_picker, m)?)?;
    m.add_function(wrap_pyfunction!(render_widget, m)?)?;
    Ok(())
}

fn binding_value_from_py(value: &Bound<'_, PyAny>) -> PyResult<BindingValue> {
    if let Ok(value) = value.extract::<bool>() {
        return Ok(BindingValue::Bool(value));
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(BindingValue::String(value));
    }
    if let Ok(value) = value.extract::<f64>() {
        return Ok(BindingValue::Number(value));
    }
    Err(PyValueError::new_err(
        "state values must be bool, str, int, or float",
    ))
}

fn binding_value_to_py(py: Python<'_>, value: BindingValue) -> PyResult<Py<PyAny>> {
    match value {
        BindingValue::String(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        BindingValue::Number(value) => Ok(value.into_pyobject(py)?.into_any().unbind()),
        BindingValue::Bool(value) => Ok(value.into_pyobject(py)?.to_owned().into_any().unbind()),
    }
}

fn binding_text_from_py(value: &Bound<'_, PyAny>) -> PyResult<BindingText> {
    if let Ok(state) = value.extract::<PyRef<'_, PyState>>() {
        return Ok(BindingText::State(state.inner.clone()));
    }
    Ok(BindingText::Static(
        binding_value_from_py(value)?.as_label_text(),
    ))
}

fn binding_bool_from_py(value: &Bound<'_, PyAny>) -> PyResult<BindingBool> {
    if let Ok(state) = value.extract::<PyRef<'_, PyState>>() {
        return Ok(BindingBool::State(state.inner.clone()));
    }
    if let Ok(value) = value.extract::<bool>() {
        return Ok(BindingBool::Static(value));
    }
    Err(PyValueError::new_err(
        "toggle value must be bool or sui.State",
    ))
}

fn binding_number_from_py(value: &Bound<'_, PyAny>) -> PyResult<BindingNumber> {
    if let Ok(state) = value.extract::<PyRef<'_, PyState>>() {
        return Ok(BindingNumber::State(state.inner.clone()));
    }
    if let Ok(value) = value.extract::<f64>() {
        return Ok(BindingNumber::Static(value));
    }
    Err(PyValueError::new_err(
        "numeric value must be int, float, or sui.State",
    ))
}

fn py_image_fit(value: &str) -> PyResult<BindingImageFit> {
    match value {
        "fill" => Ok(BindingImageFit::Fill),
        "contain" => Ok(BindingImageFit::Contain),
        "cover" => Ok(BindingImageFit::Cover),
        "none" => Ok(BindingImageFit::None),
        _ => Err(PyValueError::new_err(
            "image fit must be 'fill', 'contain', 'cover', or 'none'",
        )),
    }
}

fn py_icon_glyph(value: &str) -> PyResult<sui_crate::IconGlyph> {
    binding_icon_glyph_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown icon glyph '{value}'")))
}

fn py_semantic_tone(value: &str) -> PyResult<sui_crate::SemanticTone> {
    binding_semantic_tone_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown semantic tone '{value}'")))
}

fn py_table_column_alignment(value: &str) -> PyResult<sui_crate::TableColumnAlignment> {
    binding_table_column_alignment_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown table column alignment '{value}'")))
}

fn py_surface_role(value: &str) -> PyResult<sui_crate::SurfaceRole> {
    binding_surface_role_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown surface role '{value}'")))
}

fn py_surface_border(value: &str) -> PyResult<sui_crate::SurfaceBorder> {
    binding_surface_border_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown surface border '{value}'")))
}

fn py_surface_elevation(value: &str) -> PyResult<sui_crate::SurfaceElevation> {
    binding_surface_elevation_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown surface elevation '{value}'")))
}

fn py_alignment(value: &str) -> PyResult<sui_crate::Alignment> {
    binding_alignment_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown alignment '{value}'")))
}

fn py_tooltip_placement(value: &str) -> PyResult<sui_crate::TooltipPlacement> {
    binding_tooltip_placement_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown tooltip placement '{value}'")))
}

fn py_semantics_role(value: &str) -> PyResult<sui_crate::SemanticsRole> {
    binding_semantics_role_from_name(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown semantics role '{value}'")))
}

fn py_axis(value: &str) -> PyResult<Axis> {
    match value {
        "horizontal" | "x" | "row" => Ok(Axis::Horizontal),
        "vertical" | "y" | "column" => Ok(Axis::Vertical),
        _ => Err(PyValueError::new_err(
            "axis must be 'horizontal' or 'vertical'",
        )),
    }
}

fn py_scroll_axes(value: &str) -> PyResult<BindingScrollAxes> {
    match value {
        "vertical" | "y" | "column" => Ok(BindingScrollAxes::Vertical),
        "horizontal" | "x" | "row" => Ok(BindingScrollAxes::Horizontal),
        "both" | "xy" | "all" => Ok(BindingScrollAxes::Both),
        _ => Err(PyValueError::new_err(
            "scroll axes must be 'vertical', 'horizontal', or 'both'",
        )),
    }
}

fn extract_binding_widgets(children: &Bound<'_, PyAny>) -> PyResult<Vec<BindingWidget>> {
    children
        .extract::<Vec<PyRef<'_, PyWidget>>>()?
        .iter()
        .map(|widget| widget.binding_widget())
        .collect()
}

fn extract_text_spans(spans: &Bound<'_, PyAny>) -> PyResult<Vec<BindingTextSpan>> {
    Ok(spans
        .extract::<Vec<PyRef<'_, PyTextSpan>>>()?
        .iter()
        .map(|span| span.inner.clone())
        .collect())
}

fn extract_status_bar_segments(
    segments: &Bound<'_, PyAny>,
) -> PyResult<Vec<BindingStatusBarSegment>> {
    Ok(segments
        .extract::<Vec<PyRef<'_, PyStatusBarSegment>>>()?
        .iter()
        .map(|segment| segment.inner.clone())
        .collect())
}

fn extract_segmented_control_items(
    items: &Bound<'_, PyAny>,
) -> PyResult<Vec<BindingSegmentedControlItem>> {
    Ok(items
        .extract::<Vec<PyRef<'_, PySegmentedControlItem>>>()?
        .iter()
        .map(|item| item.inner.clone())
        .collect())
}

fn extract_table_columns(columns: &Bound<'_, PyAny>) -> PyResult<Vec<BindingTableColumn>> {
    Ok(columns
        .extract::<Vec<PyRef<'_, PyTableColumn>>>()?
        .iter()
        .map(|column| column.inner.clone())
        .collect())
}

fn extract_table_rows(rows: &Bound<'_, PyAny>) -> PyResult<Vec<BindingTableRow>> {
    Ok(rows
        .extract::<Vec<PyRef<'_, PyTableRow>>>()?
        .iter()
        .map(|row| row.inner.clone())
        .collect())
}

fn extract_tree_items(items: &Bound<'_, PyAny>) -> PyResult<Vec<BindingTreeItem>> {
    Ok(items
        .extract::<Vec<PyRef<'_, PyTreeItem>>>()?
        .iter()
        .map(|item| item.inner.clone())
        .collect())
}

fn extract_layer_list_items(items: &Bound<'_, PyAny>) -> PyResult<Vec<BindingLayerListItem>> {
    Ok(items
        .extract::<Vec<PyRef<'_, PyLayerListItem>>>()?
        .iter()
        .map(|item| item.inner.clone())
        .collect())
}

fn extract_menu_items(items: &Bound<'_, PyAny>) -> PyResult<Vec<BindingMenuItem>> {
    Ok(items
        .extract::<Vec<PyRef<'_, PyMenuItem>>>()?
        .iter()
        .map(|item| item.inner.clone())
        .collect())
}

fn extract_tool_palette_items(items: &Bound<'_, PyAny>) -> PyResult<Vec<BindingToolPaletteItem>> {
    Ok(items
        .extract::<Vec<PyRef<'_, PyToolPaletteItem>>>()?
        .iter()
        .map(|item| item.inner.clone())
        .collect())
}

fn extract_color_palette_swatches(
    swatches: &Bound<'_, PyAny>,
) -> PyResult<Vec<BindingColorPaletteSwatch>> {
    Ok(swatches
        .extract::<Vec<PyRef<'_, PyColorPaletteSwatch>>>()?
        .iter()
        .map(|swatch| swatch.inner.clone())
        .collect())
}

fn extract_size(value: &Bound<'_, PyAny>) -> PyResult<Size> {
    if let Ok(size) = value.extract::<PyRef<'_, PySize>>() {
        return Ok(Size::new(size.width, size.height));
    }
    if let Ok((width, height)) = value.extract::<(f32, f32)>() {
        return Ok(Size::new(width, height));
    }
    Err(PyValueError::new_err(
        "measure must return sui.Size or a (width, height) tuple",
    ))
}

fn py_value_error(error: PaintValidationError) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn parse_pointer_event_kind(value: &str) -> PyResult<BindingPointerEventKind> {
    match value {
        "down" => Ok(BindingPointerEventKind::Down),
        "up" => Ok(BindingPointerEventKind::Up),
        "move" => Ok(BindingPointerEventKind::Move),
        "scroll" => Ok(BindingPointerEventKind::Scroll),
        "enter" => Ok(BindingPointerEventKind::Enter),
        "leave" => Ok(BindingPointerEventKind::Leave),
        "cancel" => Ok(BindingPointerEventKind::Cancel),
        _ => Err(PyValueError::new_err(
            "pointer event kind must be 'down', 'up', 'move', 'scroll', 'enter', 'leave', or 'cancel'",
        )),
    }
}

fn pointer_event_kind_name(value: BindingPointerEventKind) -> &'static str {
    match value {
        BindingPointerEventKind::Down => "down",
        BindingPointerEventKind::Up => "up",
        BindingPointerEventKind::Move => "move",
        BindingPointerEventKind::Scroll => "scroll",
        BindingPointerEventKind::Enter => "enter",
        BindingPointerEventKind::Leave => "leave",
        BindingPointerEventKind::Cancel => "cancel",
    }
}

fn parse_pointer_button(value: &str) -> PyResult<BindingPointerButton> {
    match value {
        "primary" | "left" => Ok(BindingPointerButton::Primary),
        "secondary" | "right" => Ok(BindingPointerButton::Secondary),
        "middle" => Ok(BindingPointerButton::Middle),
        "back" => Ok(BindingPointerButton::Back),
        "forward" => Ok(BindingPointerButton::Forward),
        _ => {
            if let Some(value) = value.strip_prefix("other:") {
                return value
                    .parse::<u16>()
                    .map(BindingPointerButton::Other)
                    .map_err(|_| PyValueError::new_err("other pointer button must fit in u16"));
            }
            Err(PyValueError::new_err(
                "pointer button must be 'primary', 'secondary', 'middle', 'back', 'forward', or 'other:<u16>'",
            ))
        }
    }
}

fn pointer_button_name(value: BindingPointerButton) -> String {
    match value {
        BindingPointerButton::Primary => "primary".to_string(),
        BindingPointerButton::Secondary => "secondary".to_string(),
        BindingPointerButton::Middle => "middle".to_string(),
        BindingPointerButton::Back => "back".to_string(),
        BindingPointerButton::Forward => "forward".to_string(),
        BindingPointerButton::Other(button) => format!("other:{button}"),
    }
}

fn parse_pointer_kind(value: &str) -> PyResult<BindingPointerKind> {
    match value {
        "mouse" => Ok(BindingPointerKind::Mouse),
        "touch" => Ok(BindingPointerKind::Touch),
        "pen" => Ok(BindingPointerKind::Pen),
        "unknown" => Ok(BindingPointerKind::Unknown),
        _ => Err(PyValueError::new_err(
            "pointer_kind must be 'mouse', 'touch', 'pen', or 'unknown'",
        )),
    }
}

fn pointer_kind_name(value: BindingPointerKind) -> &'static str {
    match value {
        BindingPointerKind::Mouse => "mouse",
        BindingPointerKind::Touch => "touch",
        BindingPointerKind::Pen => "pen",
        BindingPointerKind::Unknown => "unknown",
    }
}

fn parse_key_state(value: &str) -> PyResult<BindingKeyState> {
    match value {
        "pressed" | "down" => Ok(BindingKeyState::Pressed),
        "released" | "up" => Ok(BindingKeyState::Released),
        _ => Err(PyValueError::new_err(
            "key state must be 'pressed' or 'released'",
        )),
    }
}

fn key_state_name(value: BindingKeyState) -> &'static str {
    match value {
        BindingKeyState::Pressed => "pressed",
        BindingKeyState::Released => "released",
    }
}

fn ime_event_kind_name(value: &BindingImeEvent) -> &'static str {
    match value {
        BindingImeEvent::CompositionStart => "composition_start",
        BindingImeEvent::CompositionUpdate { .. } => "composition_update",
        BindingImeEvent::CompositionCommit { .. } => "composition_commit",
        BindingImeEvent::CompositionEnd => "composition_end",
    }
}

fn window_event_kind_name(value: &BindingWindowEvent) -> &'static str {
    match value {
        BindingWindowEvent::CloseRequested => "close_requested",
        BindingWindowEvent::Resized(_) => "resized",
        BindingWindowEvent::ScaleFactorChanged { .. } => "scale_factor_changed",
        BindingWindowEvent::Focused(_) => "focused",
        BindingWindowEvent::Occluded(_) => "occluded",
        BindingWindowEvent::SafeAreaChanged { .. } => "safe_area_changed",
        BindingWindowEvent::ExternalFileHovered(_) => "external_file_hovered",
        BindingWindowEvent::ExternalFileHoverCancelled => "external_file_hover_cancelled",
        BindingWindowEvent::ExternalFileDropped(_) => "external_file_dropped",
        BindingWindowEvent::RedrawRequested => "redraw_requested",
    }
}

fn parse_color_space(value: &str) -> PyResult<ColorSpace> {
    match value {
        "srgb" | "sRGB" => Ok(ColorSpace::Srgb),
        "linear-srgb" | "linear_srgb" => Ok(ColorSpace::LinearSrgb),
        "display-p3" | "display_p3" => Ok(ColorSpace::DisplayP3),
        "linear-display-p3" | "linear_display_p3" => Ok(ColorSpace::LinearDisplayP3),
        _ => Err(PyValueError::new_err(
            "color_space must be 'srgb', 'linear-srgb', 'display-p3', or 'linear-display-p3'",
        )),
    }
}

fn py_radii(value: Option<&Bound<'_, PyAny>>) -> PyResult<[f32; 4]> {
    let Some(value) = value else {
        return Ok([0.0; 4]);
    };
    if value.is_none() {
        return Ok([0.0; 4]);
    }
    if let Ok(radius) = value.extract::<f32>() {
        return Ok([radius; 4]);
    }
    let radii = value.extract::<Vec<f32>>()?;
    <[f32; 4]>::try_from(radii)
        .map_err(|_| PyValueError::new_err("radii must be a number or a sequence of four numbers"))
}

fn py_four_points(value: &Bound<'_, PyAny>) -> PyResult<[sui_crate::Point; 4]> {
    let points = value.extract::<Vec<PyPoint>>()?;
    let points = <[PyPoint; 4]>::try_from(points)
        .map_err(|_| PyValueError::new_err("points must contain exactly four sui.Point values"))?;
    Ok(points.map(Into::into))
}

fn py_semantics_value(
    value: Option<&Bound<'_, PyAny>>,
    min_value: Option<f64>,
    max_value: Option<f64>,
) -> PyResult<Option<SemanticsValue>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(boolean) = value.extract::<bool>() {
        return Ok(Some(SemanticsValue::Text(boolean.to_string())));
    }
    if let Ok(number) = value.extract::<f64>() {
        if let (Some(min), Some(max)) = (min_value, max_value) {
            return Ok(Some(SemanticsValue::Range {
                value: number,
                min,
                max,
            }));
        }
        return Ok(Some(SemanticsValue::Number(number)));
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(Some(SemanticsValue::Text(text)));
    }
    Err(PyValueError::new_err(
        "semantics value must be a string, number, bool, or None",
    ))
}

fn py_toggle_state(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<ToggleState>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(checked) = value.extract::<bool>() {
        return Ok(Some(if checked {
            ToggleState::Checked
        } else {
            ToggleState::Unchecked
        }));
    }
    let state = value.extract::<String>()?;
    binding_toggle_state_from_name(&state)
        .map(Some)
        .ok_or_else(|| PyValueError::new_err("checked must be 'checked', 'unchecked', or 'mixed'"))
}

fn py_text_style(
    color: Option<PyColor>,
    font_size: Option<f32>,
    line_height: Option<f32>,
    font: Option<PyRef<'_, PyFontHandle>>,
    weight: Option<u16>,
    style: Option<&str>,
    stretch: Option<&str>,
) -> PyResult<TextStyle> {
    let mut text_style = TextStyle::new(color.map(Into::into).unwrap_or(Color::WHITE));
    if let Some(font_size) = font_size {
        text_style.font_size = font_size;
    }
    if let Some(line_height) = line_height {
        text_style.line_height = line_height;
    }
    if let Some(font) = font {
        text_style.font = Some(font.inner.into_sui());
    }
    if let Some(weight) = weight {
        text_style.weight = FontWeight::new(weight);
    }
    if let Some(style) = style {
        text_style.style = parse_font_style(style)?;
    }
    if let Some(stretch) = stretch {
        text_style.stretch = parse_font_stretch(stretch)?;
    }
    Ok(text_style)
}

fn parse_font_style(value: &str) -> PyResult<FontStyle> {
    match value {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(PyValueError::new_err(
            "font style must be 'normal', 'italic', or 'oblique'",
        )),
    }
}

fn parse_font_stretch(value: &str) -> PyResult<FontStretch> {
    match value {
        "ultra_condensed" | "ultra-condensed" => Ok(FontStretch::UltraCondensed),
        "extra_condensed" | "extra-condensed" => Ok(FontStretch::ExtraCondensed),
        "condensed" => Ok(FontStretch::Condensed),
        "semi_condensed" | "semi-condensed" => Ok(FontStretch::SemiCondensed),
        "normal" => Ok(FontStretch::Normal),
        "semi_expanded" | "semi-expanded" => Ok(FontStretch::SemiExpanded),
        "expanded" => Ok(FontStretch::Expanded),
        "extra_expanded" | "extra-expanded" => Ok(FontStretch::ExtraExpanded),
        "ultra_expanded" | "ultra-expanded" => Ok(FontStretch::UltraExpanded),
        _ => Err(PyValueError::new_err("invalid font stretch")),
    }
}

fn parse_native_backend(value: &str) -> PyResult<NativeGraphicsBackend> {
    match value {
        "cpu" => Ok(NativeGraphicsBackend::Cpu),
        "wgpu" => Ok(NativeGraphicsBackend::Wgpu),
        "webgpu" | "web-gpu" | "web_gpu" => Ok(NativeGraphicsBackend::WebGpu),
        "d3d12" => Ok(NativeGraphicsBackend::D3d12),
        "metal" => Ok(NativeGraphicsBackend::Metal),
        "vulkan" => Ok(NativeGraphicsBackend::Vulkan),
        "opengl" | "open-gl" | "open_gl" => Ok(NativeGraphicsBackend::OpenGl),
        "unknown" => Ok(NativeGraphicsBackend::Unknown),
        _ => Err(PyValueError::new_err(
            "backend must be 'cpu', 'wgpu', 'webgpu', 'd3d12', 'metal', 'vulkan', 'opengl', or 'unknown'",
        )),
    }
}

fn native_backend_name(value: NativeGraphicsBackend) -> &'static str {
    match value {
        NativeGraphicsBackend::Cpu => "cpu",
        NativeGraphicsBackend::Wgpu => "wgpu",
        NativeGraphicsBackend::WebGpu => "webgpu",
        NativeGraphicsBackend::D3d12 => "d3d12",
        NativeGraphicsBackend::Metal => "metal",
        NativeGraphicsBackend::Vulkan => "vulkan",
        NativeGraphicsBackend::OpenGl => "opengl",
        NativeGraphicsBackend::Unknown => "unknown",
    }
}

fn parse_interop_tier(value: &str) -> PyResult<RendererInteropTier> {
    match value {
        "cpu_upload" | "cpu-upload" => Ok(RendererInteropTier::CpuUpload),
        "shared_texture" | "shared-texture" => Ok(RendererInteropTier::SharedTexture),
        "shared_render_target" | "shared-render-target" => {
            Ok(RendererInteropTier::SharedRenderTarget)
        }
        _ => Err(PyValueError::new_err(
            "tier must be 'cpu_upload', 'shared_texture', or 'shared_render_target'",
        )),
    }
}

fn interop_tier_name(value: RendererInteropTier) -> &'static str {
    match value {
        RendererInteropTier::CpuUpload => "cpu_upload",
        RendererInteropTier::SharedTexture => "shared_texture",
        RendererInteropTier::SharedRenderTarget => "shared_render_target",
    }
}

fn parse_external_texture_format(value: &str) -> PyResult<ExternalTextureFormat> {
    match value {
        "rgba8unorm" | "rgba8_unorm" | "rgba8" => Ok(ExternalTextureFormat::Rgba8Unorm),
        "bgra8unorm" | "bgra8_unorm" | "bgra8" => Ok(ExternalTextureFormat::Bgra8Unorm),
        "rgba16float" | "rgba16_float" | "rgba16f" => Ok(ExternalTextureFormat::Rgba16Float),
        _ => Err(PyValueError::new_err(
            "format must be 'rgba8unorm', 'bgra8unorm', or 'rgba16float'",
        )),
    }
}

fn py_external_texture_error(error: ExternalTextureValidationError) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn py_runtime_error(error: impl ToString) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

fn foreign_py_error(error: PyErr) -> ForeignCallbackFailure {
    ForeignCallbackFailure::new(error.to_string())
}

fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

trait PaintCommandBuilderIntrospection {
    fn command_count(&self) -> usize;
}

impl PaintCommandBuilderIntrospection for PaintCommandBuilder {
    fn command_count(&self) -> usize {
        self.clone()
            .finish()
            .map(|commands| commands.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyDict, PyModule};

    #[test]
    fn python_module_links() {
        assert_eq!(
            std::any::type_name::<super::PyWidget>(),
            "sui_python::PyWidget"
        );
    }

    #[test]
    fn renders_python_custom_widget() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            let module = PyModule::from_code(
                py,
                c"
import sui

class Meter:
    name = 'CPU meter'

    def measure(self, constraints):
        return constraints.clamp(sui.Size(160.0, 28.0))

    def paint(self, paint):
        r = paint.bounds
        paint.fill_rect(r, sui.Color.rgba(0.11, 0.12, 0.14, 1.0))
        paint.fill_rect(
            sui.Rect(r.x, r.y, r.width * 0.5, r.height),
            sui.Color.rgba(0.25, 0.68, 0.46, 1.0),
        )
",
                c"meter.py",
                c"meter",
            )?;
            let callbacks = module.getattr("Meter")?.call0()?.unbind();
            let widget = Py::new(py, PyWidget::new(callbacks))?;
            let snapshot = render_widget(widget.bind(py).borrow(), None)?;

            assert!(snapshot.command_count >= 2);
            assert_eq!(snapshot.fill_rect_count, 2);
            assert_eq!(snapshot.semantics_count, 1);
            Ok(())
        })
    }

    #[test]
    fn python_custom_widget_receives_event_descriptor() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class Probe:
    def __init__(self):
        self.events = []

    def measure(self, constraints):
        return constraints.clamp(sui.Size(80, 24))

    def event(self, event):
        self.events.append((
            event.kind,
            event.action,
            event.position.x,
            event.button,
            event.modifiers.shift,
        ))
        return True

    def paint(self, paint):
        paint.fill_rect(paint.bounds, sui.Color.rgba(0.1, 0.2, 0.3, 1.0))

probe = Probe()
widget = sui.Widget(probe)
event = sui.Event.pointer(
    'down',
    sui.Point(8, 8),
    button='primary',
    buttons=1,
    modifiers=sui.Modifiers(shift=True),
)
snapshot = sui.render_widget(widget, event)

assert snapshot.command_count >= 1
assert probe.events == [('pointer', 'down', 8.0, 'primary', True)]

key = sui.Event.keyboard('Enter')
assert key.kind == 'keyboard'
assert key.key == 'Enter'
assert key.state == 'pressed'
",
                c"events.py",
                c"events",
            )?;
            Ok(())
        })
    }

    #[test]
    fn drains_python_ui_tasks() -> PyResult<()> {
        Python::attach(|py| {
            let module = PyModule::from_code(
                py,
                c"
calls = 0

def bump():
    global calls
    calls += 1
",
                c"tasks.py",
                c"tasks",
            )?;
            let queue = PyUiTaskQueue::new();
            queue.post(module.getattr("bump")?.unbind());

            assert_eq!(queue.pending_count(), 1);
            assert_eq!(queue.drain(), 1);
            assert_eq!(queue.pending_count(), 0);
            assert_eq!(module.getattr("calls")?.extract::<usize>()?, 1);
            Ok(())
        })
    }

    #[test]
    fn python_custom_widget_emits_semantics_nodes() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class Meter:
    def measure(self, constraints):
        return constraints.clamp(sui.Size(160, 28))

    def semantics(self, semantics):
        semantics.node(
            role='progress_bar',
            name='CPU meter',
            value=0.62,
            min_value=0.0,
            max_value=1.0,
            disabled=True,
            focused=True,
            hidden=True,
            hovered=True,
            selected=True,
            expanded=False,
            busy=True,
        )

snapshot = sui.render_widget(sui.Widget(Meter()))

assert 'progress_bar' in snapshot.semantics_roles
assert 'CPU meter' in snapshot.semantics_names
assert '0.62:0:1' in snapshot.semantics_values
assert True in snapshot.semantics_disabled, snapshot.semantics_disabled
assert True in snapshot.semantics_hidden, snapshot.semantics_hidden
assert True in snapshot.semantics_hovered, snapshot.semantics_hovered
assert True in snapshot.semantics_selected, snapshot.semantics_selected
assert 'collapsed' in snapshot.semantics_expanded, snapshot.semantics_expanded
assert True in snapshot.semantics_busy, snapshot.semantics_busy
",
                c"semantics.py",
                c"semantics",
            )?;
            Ok(())
        })
    }

    #[test]
    fn renders_python_shader_paint_command() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class HueWidget:
    def measure(self, constraints):
        return constraints.clamp(sui.Size(120, 20))

    def paint(self, paint):
        paint.draw_shader_rect(paint.bounds, sui.Shader.hue_bar())

widget = sui.Widget(HueWidget())
snapshot = sui.render_widget(widget)

assert snapshot.command_count >= 1

try:
    sui.Shader.rgb_channel_bar(sui.Color.rgba(1, 0, 0, 1), 4)
    raise AssertionError('invalid channel should fail')
except ValueError:
    pass
",
                c"shader_widget.py",
                c"shader_widget",
            )?;
            Ok(())
        })
    }

    #[test]
    fn renders_python_registered_image_paint_command() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class ImageWidget:
    def measure(self, constraints):
        return constraints.clamp(sui.Size(32, 16))

    def paint(self, paint):
        image = paint.rgba_image(
            0,
            2,
            1,
            bytes([
                255, 0, 0, 255,
                0, 0, 255, 255,
            ]),
        )
        assert image.local_slot == 0
        paint.draw_image(paint.bounds, image)

snapshot = sui.render_widget(sui.Widget(ImageWidget()))
assert snapshot.draw_image_count == 1
assert snapshot.registered_image_count >= 1
",
                c"image_paint.py",
                c"image_paint",
            )?;
            Ok(())
        })
    }

    #[test]
    fn renders_python_styled_text_paint_command() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class TextWidget:
    def measure(self, constraints):
        return constraints.clamp(sui.Size(160, 32))

    def paint(self, paint):
        paint.draw_text(
            paint.bounds,
            'Styled',
            sui.Color.rgba(0.9, 0.95, 1.0, 1.0),
            font_size=18,
            line_height=22,
            weight=700,
            style='italic',
            stretch='condensed',
        )

snapshot = sui.render_widget(sui.Widget(TextWidget()))
assert snapshot.command_count >= 1
",
                c"styled_text.py",
                c"styled_text",
            )?;
            Ok(())
        })
    }

    #[test]
    fn renders_python_rich_low_level_paint_commands() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class RichPaintWidget:
    def measure(self, constraints):
        return constraints.clamp(sui.Size(96, 64))

    def paint(self, paint):
        path = sui.Path.circle(sui.Point(24, 24), 14)
        builder = sui.PathBuilder()
        builder.move_to(sui.Point(8, 48))
        builder.line_to(sui.Point(36, 48))
        builder.quad_to(sui.Point(48, 56), sui.Point(60, 48))
        builder.close()
        custom_path = builder.build()

        image = paint.rgba_image(
            1,
            1,
            1,
            bytes([255, 255, 255, 255]),
        )
        shadow = sui.Shadow(2, 3, 4, 1, sui.Color.rgba(0, 0, 0, 0.35))

        paint.push_clip_path(sui.Path.rect(paint.bounds))
        paint.push_transform(sui.Transform.translation(4, 2))
        paint.fill_path(path, sui.Color.rgba(0.2, 0.5, 0.9, 1))
        paint.stroke_path(custom_path, sui.Color.white(), width=1.5)
        paint.draw_shadow(sui.Rect(8, 8, 48, 28), shadow, radii=6)
        paint.fill_rounded_rect_with_shadow(
            sui.Rect(12, 12, 40, 20),
            sui.Color.rgba(0.9, 0.4, 0.2, 1),
            shadow,
            radii=(5, 5, 3, 3),
        )
        paint.draw_image_quad(
            [
                sui.Point(56, 8),
                sui.Point(80, 12),
                sui.Point(76, 36),
                sui.Point(52, 32),
            ],
            image,
        )
        paint.pop_transform()
        paint.pop_clip()

widget = sui.Widget(RichPaintWidget())
snapshot = sui.render_widget(widget)

assert snapshot.command_count >= 8
assert snapshot.draw_image_count == 1
assert snapshot.registered_image_count >= 1
",
                c"rich_paint.py",
                c"rich_paint",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_app_level_image_resource_can_be_drawn() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

app = sui.App()
image = app.rgba_image(
    2,
    1,
    bytes([
        255, 0, 0, 255,
        0, 0, 255, 255,
    ]),
)

app.window(sui.Window('Image').root(
    sui.Image(image, label='Preview', fit='contain', size=sui.Size(32, 16))
))

assert app.image_resource_count() == 1
snapshot = app.render()
assert snapshot.draw_image_count == 1
assert snapshot.registered_image_count >= 1
assert 'image' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Preview' in snapshot.semantics_names, snapshot.semantics_names
",
                c"app_image_resource.py",
                c"app_image_resource",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_app_level_font_resource_is_registered() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

app = sui.App()
font = app.font_bytes(bytes([0, 1, 2, 3]))
app.window(sui.Window('Font').root(sui.Label('Text')))

assert font.id > 0
assert app.font_resource_count() == 1
snapshot = app.render()
assert snapshot.registered_font_count == 1
",
                c"app_font_resource.py",
                c"app_font_resource",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_app_level_resources_can_be_loaded_from_files() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import os
import struct
import tempfile
import zlib
import sui

svg = b'<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"><rect width=\"2\" height=\"2\" fill=\"red\"/></svg>'

def png_rgba(width, height, pixels):
    def chunk(kind, data):
        return (
            struct.pack('>I', len(data))
            + kind
            + data
            + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
        )

    rows = []
    stride = width * 4
    for y in range(height):
        start = y * stride
        rows.append(bytes([0]) + pixels[start:start + stride])
    return (
        bytes([137, 80, 78, 71, 13, 10, 26, 10])
        + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
        + chunk(b'IDAT', zlib.compress(b''.join(rows)))
        + chunk(b'IEND', b'')
    )

png = png_rgba(1, 1, bytes([255, 0, 0, 255]))

with tempfile.TemporaryDirectory() as directory:
    svg_path = os.path.join(directory, 'icon.svg')
    png_path = os.path.join(directory, 'icon.png')
    font_path = os.path.join(directory, 'font.bin')
    with open(svg_path, 'wb') as file:
        file.write(svg)
    with open(png_path, 'wb') as file:
        file.write(png)
    with open(font_path, 'wb') as file:
        file.write(bytes([0, 1, 2, 3]))

    app = sui.App()
    image = app.svg_file(svg_path)
    resized = app.svg_file_at_size(16, 16, svg_path)
    png_bytes = app.png_image(png)
    png_file = app.png_file(png_path)
    font = app.font_file(font_path)

    assert image.id > 0
    assert resized.id > 0
    assert png_bytes.id > 0
    assert png_file.id > 0
    assert font.id > 0
    assert app.image_resource_count() == 4
    assert app.font_resource_count() == 1
",
                c"app_file_resources.py",
                c"app_file_resources",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_external_texture_descriptors_validate_interop_inputs() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

caps = sui.RendererInteropCapabilities('wgpu', shared_texture=True)
assert caps.backend == 'wgpu'
assert caps.supports('cpu_upload')
assert caps.supports('shared_texture')
assert not caps.supports('shared_render_target')

cpu = sui.ExternalTextureDescriptor.cpu_rgba8(sui.Size(2, 2), bytes(16), generation=7)
assert cpu.tier == 'cpu_upload'
assert cpu.size.width == 2
cpu.validate()

bad = sui.ExternalTextureDescriptor.cpu_rgba8(sui.Size(2, 2), bytes(15))
try:
    bad.validate()
    raise AssertionError('invalid CPU byte length should fail')
except ValueError:
    pass

handle = sui.ExternalBackendHandle(42)
sync = sui.ExternalSync.generation(3)
shared = sui.ExternalTextureDescriptor.shared_texture(
    'wgpu',
    sui.Size(4, 4),
    'rgba8unorm',
    handle,
    sync,
)
assert shared.tier == 'shared_texture'
shared.validate()

empty = sui.ExternalTextureDescriptor.shared_texture(
    'wgpu',
    sui.Size(4, 4),
    'rgba8unorm',
    sui.ExternalBackendHandle(0),
    sui.ExternalSync.none(),
)
try:
    empty.validate()
    raise AssertionError('empty handle should fail')
except ValueError:
    pass
",
                c"interop.py",
                c"interop",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_external_surface_draws_cpu_fallback() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

texture = sui.ExternalTextureDescriptor.cpu_rgba8(
    sui.Size(2, 1),
    bytes([
        255, 0, 0, 255,
        0, 0, 255, 255,
    ]),
    generation=1,
)
surface = sui.ExternalSurface(texture, desired_size=sui.Size(64, 32), name='Preview')
snapshot = sui.render_widget(surface)

assert snapshot.draw_image_count == 1
assert snapshot.registered_image_count >= 1
assert snapshot.semantics_count >= 1
",
                c"external_surface.py",
                c"external_surface",
            )?;
            Ok(())
        })
    }

    #[test]
    fn renders_python_high_level_app_tree() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

state = sui.State('Ready')
app = sui.App()
root = sui.Column([
    sui.Label(state),
    sui.Button('Apply'),
], gap=8.0)
app.window(sui.Window('Bindings').root(root))
snapshot = app.render()

assert app.window_count() == 1
assert snapshot.command_count > 0
assert snapshot.semantics_count >= 2
assert state.get() == 'Ready'
state.set('Updated')
assert state.get() == 'Updated'
",
                c"high_level.py",
                c"high_level",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_renders_cross_language_compatibility_signature() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

opacity = sui.State(0.5)
count = sui.State(3.0)
progress = sui.State(0.25)
text = sui.State('Ada')
password = sui.State('sëcret')
scheduled_for = sui.State('2026-07-15 09:30')
notes = sui.State('Line one' + chr(10) + 'Line two')
card_enabled = sui.State(True)
field_focused = sui.State(False)
field_invalid = sui.State(False)
placement = sui.State('Primary')
sheet_shown = sui.State(False)
split_ratio = sui.State(0.4)
selected_view = sui.State(1)
brush = sui.BrushPreviewSpec(
    sui.Color.rgba(0.8, 0.2, 0.3, 1.0),
    size=22,
    opacity=0.75,
    shape='round',
)
app = sui.App()
root = sui.Column([
    sui.Label('Ready'),
    sui.Button('Apply'),
    sui.Icon('search', label='Search icon'),
    sui.IconButton('download', 'Download', selected=True, enabled=True, size=28, icon_size=16, description='Download file'),
    sui.Surface(
        sui.Label('Surface content'),
        role='panel',
        name='Main surface',
        elevation='small',
        padding=6.0,
    ),
    sui.Toolbar([
        sui.Button('Toolbar action'),
        sui.Icon('search', label='Toolbar search'),
    ], name='Main toolbar', extent=32.0, padding=4.0, spacing=4.0),
    sui.Link('Documentation', 'https://example.invalid/docs'),
    sui.Checkbox('Enabled', True),
    sui.Switch('Airplane mode', False),
    sui.RadioButton('Manual', True),
    sui.RadioGroup('Priority', ['Low', 'Medium', 'High'], selected=1),
    sui.SegmentedControl('View mode', [
        sui.SegmentedControlItem('List', semantic_name='Show list view', description='Compact rows'),
        sui.SegmentedControlItem('Gallery'),
        sui.SegmentedControlItem('Map', semantic_name='Show map view', disabled=True),
    ], selected=1),
    sui.Breadcrumb('Workspace path', ['D:', 'Workspace', 'sui'], current=2),
    sui.ListView('Assets', ['Brush', 'Canvas', 'Export'], selected=1),
    sui.Table('Build table', [
        sui.TableColumn('Task', width=160.0),
        sui.TableColumn('Owner', width=96.0, alignment='center'),
    ], [
        sui.TableRow(['Bindings', 'IX']),
        sui.TableRow(['Renderer', 'Core']),
    ], selected=0),
    sui.SignalMeter('Input signal', True, description='Live audio input', bars=8, size=sui.Size(76, 16)),
    sui.StatusBadge('Online', tone='success', icon='check', min_width=72.0),
    sui.StatusBar([
        sui.StatusBarSegment('Ln 12'),
        sui.StatusBarSegment('Writable', tone='success', min_width=84.0),
        sui.StatusBarSegment('UTF-8', tone='info', expand=True),
    ], name='Editor status', description='All systems nominal', height=24.0),
    sui.DetailRow('Build', 'Debug profile with local bindings', max_value_lines=2),
    sui.Slider('Opacity', opacity, min_value=0.0, max_value=1.0, step=0.25),
    sui.NumberInput('Count', count, min_value=0.0, max_value=10.0, step=1.0, precision=0),
    sui.Select('Mode', ['Draft', 'Final', 'Review'], selected=1, placeholder='Choose mode'),
    sui.ProgressBar('Load progress', progress, min_value=0.0, max_value=1.0, show_value=True),
    sui.BusyIndicator('Background work', label='Loading assets', size=20),
    sui.ActionCard('Create project', 'Start from a template', icon='plus', tone='accent', enabled=card_enabled),
    sui.BrushPreview('Current brush', brush, kind='ink', size=sui.Size(72, 36)),
    sui.CommandGroup('Editing commands', [
        sui.Button('Cut'),
        sui.Button('Copy'),
    ], axis='horizontal', padding=4, spacing=2, corner_radius=5),
    sui.CoverageDots('Coverage', 3, 4, tone='success', max_dots=4, min_width=72),
    sui.TextInput('Name', text, placeholder='Type a name'),
    sui.PasswordInput('Password', password, placeholder='Enter a password'),
    sui.DateTimeInput('Scheduled for', scheduled_for, placeholder='YYYY-MM-DD HH:MM'),
    sui.TextArea('Notes', notes, placeholder='Type notes'),
    sui.Dock(
        sui.Label('Dock body'),
        top=sui.Label('Dock top'),
        top_height=20,
        bottom=sui.Label('Dock bottom'),
        bottom_height=20,
    ),
    sui.FixedPaneSplit(
        sui.Label('Fixed pane'),
        sui.Separator('vertical'),
        sui.Label('Flexible pane'),
        fixed='first',
        fixed_extent=72,
    ),
    sui.FramedField(
        sui.TextInput('Framed editor'),
        name='Framed field',
        description='Compound editor frame',
        padding=4,
        min_height=32,
        fill_width=True,
        focused=field_focused,
        invalid=field_invalid,
    ),
    sui.MeasuredBottomDock(
        sui.Label('Measured body'),
        sui.Label('Measured footer'),
        fallback_size=sui.Size(240, 120),
    ),
    sui.PlacementBadge(placement, icon='brush', tone='info', current=2, target=3, min_width=96),
    sui.PropertyRow('Property', sui.TextInput('Property value'), stacked=True, gap=3),
    sui.SectionLabel('Advanced', semantic_name='Advanced section'),
    sui.SideSheet(
        'Inspector',
        sui.Label('Sheet body'),
        description='Selection details',
        shown=sheet_shown,
        placement='right',
        header_action=sui.Button('Close inspector'),
        actions=[sui.Button('Save inspector')],
    ),
    sui.SplitView(
        sui.Label('Split first'),
        sui.Label('Split second'),
        axis='horizontal',
        name='Workspace split',
        ratio=split_ratio,
        min_first=40,
        min_second=40,
        divider_thickness=4,
    ),
    sui.SwitchView([
        sui.Label('Inactive view'),
        sui.Label('Active view'),
    ], selected=selected_view),
    sui.TrailingSlotRow(
        sui.Label('Trailing body'),
        sui.Button('More'),
        trailing_width=56,
        trailing_height=24,
        gap=4,
    ),
    sui.FloatingStack([
        sui.FloatingStackWindow(
            sui.Rect(4, 4, 120, 36),
            sui.Label('Floating window'),
        ),
    ], name='Floating workspace'),
    sui.VirtualScrollView([
        sui.Label('Virtual row one'),
        sui.Label('Virtual row two'),
    ], name='Virtual results', padding=4, spacing=2),
    sui.ReorderableList(
        'Tasks',
        [sui.Label('Task one'), sui.Label('Task two')],
        spacing=4,
        drag_threshold=4,
        preview_label='Move task',
    ),
    sui.ScrollView(
        sui.RichText([
            sui.TextSpan('Warm', color=sui.Color.rgba(0.9, 0.35, 0.2, 1.0)),
            sui.TextSpan(' cool', color=sui.Color.rgba(0.25, 0.55, 0.9, 1.0)),
        ], semantic_name='Rich summary'),
        name='Scrollable content',
    ),
    sui.ColorSwatch('Accent', sui.Color.rgba(0.25, 0.5, 0.75, 1.0), size=sui.Size(24, 24)),
    sui.Separator('horizontal', name='Section divider', length=24.0),
    sui.EmptyState(
        'No projects',
        'Create a project to get started.',
        name='Projects empty',
        detail='Templates are available',
        icon='folder',
        action=sui.Button('New project'),
        transparent=True,
    ),
], gap=6.0)
app.window(sui.Window('Compatibility').root(root))
snapshot = app.render()

assert snapshot.command_count > 0
assert snapshot.semantics_count >= 30

for role in ('generic_container', 'text', 'button', 'link', 'checkbox', 'switch', 'radio_button', 'radio_group', 'breadcrumb', 'list', 'list_item', 'table', 'slider', 'spin_box', 'combo_box', 'progress_bar', 'busy_indicator', 'text_input', 'image', 'scroll_view', 'color_swatch', 'separator'):
    assert role in snapshot.semantics_roles, (role, snapshot.semantics_roles)

for name in ('Ready', 'Apply', 'Search icon', 'Download', 'Main surface', 'Surface content', 'Main toolbar', 'Toolbar action', 'Toolbar search', 'Documentation', 'Enabled', 'Airplane mode', 'Manual', 'Priority', 'View mode', 'Show list view', 'Gallery', 'Show map view', 'Workspace path', 'Assets', 'Brush', 'Canvas', 'Export', 'Build table', 'Input signal', 'Online', 'Editor status', 'Ln 12', 'Writable', 'UTF-8', 'Build', 'Opacity', 'Count', 'Mode', 'Load progress', 'Background work', 'Name', 'Password', 'Scheduled for', 'Notes', 'Scrollable content', 'Rich summary', 'Accent', 'Section divider', 'Projects empty', 'New project'):
    assert name in snapshot.semantics_names, (name, snapshot.semantics_names)

for value in ('https://example.invalid/docs', '0.5:0:1', '3:0:10', 'Medium', 'Gallery', 'List', 'Map', 'sui', 'Canvas', 'Bindings', 'active', 'Online', 'All systems nominal', 'Ln 12', 'Writable', 'UTF-8', 'Debug profile with local bindings', 'Final', '0.25:0:1', 'Ada', '••••••', '2026-07-15 09:30', 'Line one' + chr(10) + 'Line two', 'Warm cool', '#4080BFFF'):
    assert value in snapshot.semantics_values, (value, snapshot.semantics_values)

assert 'Loading assets' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'Download file' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'Live audio input' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'Compact rows' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'All systems nominal' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'Create a project to get started. Templates are available' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'checked' in snapshot.semantics_checked, snapshot.semantics_checked
assert 'unchecked' in snapshot.semantics_checked, snapshot.semantics_checked
assert True in snapshot.semantics_busy, snapshot.semantics_busy
assert True in snapshot.semantics_editable_multiline, snapshot.semantics_editable_multiline
assert True in snapshot.semantics_selected, snapshot.semantics_selected
",
                c"compatibility_signature.py",
                c"compatibility_signature",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_rich_text_exposes_plain_text_semantics() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

app = sui.App()
app.window(sui.Window('Rich text').root(
    sui.RichText([
        sui.TextSpan('Warm', color=sui.Color.rgba(0.9, 0.35, 0.2, 1.0), weight=700),
        sui.TextSpan(' cool', color=sui.Color.rgba(0.25, 0.55, 0.9, 1.0), style='italic'),
    ], semantic_name='Rich summary', min_width=80)
))
snapshot = app.render()

assert snapshot.command_count > 0
assert 'text' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Rich summary' in snapshot.semantics_names, snapshot.semantics_names
assert 'Warm cool' in snapshot.semantics_values, snapshot.semantics_values
",
                c"rich_text.py",
                c"rich_text",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_scroll_view_exposes_container_and_child_semantics() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

app = sui.App()
app.window(sui.Window('Scroll').root(
    sui.ScrollView(sui.Label('Inside'), name='Scrollable content')
))
snapshot = app.render()

assert snapshot.command_count > 0
assert 'scroll_view' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Scrollable content' in snapshot.semantics_names, snapshot.semantics_names
assert 'Inside' in snapshot.semantics_names, snapshot.semantics_names
",
                c"scroll_view.py",
                c"scroll_view",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_high_level_app_accepts_custom_widget() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

class Probe:
    name = 'Probe widget'

    def __init__(self):
        self.events = []

    def measure(self, constraints):
        return constraints.clamp(sui.Size(96, 24))

    def event(self, event):
        self.events.append(event.action)
        return True

    def paint(self, paint):
        paint.fill_rect(paint.bounds, sui.Color.rgba(0.2, 0.4, 0.6, 1))

probe = Probe()
app = sui.App()
app.window(sui.Window('Custom').root(
    sui.Column([
        sui.Widget(probe),
        sui.Label('Tail'),
    ], gap=4)
))

snapshot = app.render()
assert snapshot.command_count > 0
assert snapshot.fill_rect_count >= 1
assert snapshot.semantics_count >= 2

running = app.start()
assert running.render().command_count > 0
running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(8, 8),
    button='primary',
    buttons=1,
))
assert probe.events == ['down']
",
                c"custom_app.py",
                c"custom_app",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_high_level_controls_render_and_update_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

checked = sui.State(False)
opacity = sui.State(0.25)
selected = sui.State(False)
enabled = sui.State(True)
changes = []

app = sui.App()
app.window(sui.Window('Controls').root(
    sui.Column([
        sui.Checkbox('Enabled', checked, on_toggle=changes.append),
        sui.Switch('Airplane mode', False),
        sui.Slider('Opacity', opacity, min_value=0.0, max_value=1.0, step=0.05),
        sui.Icon('search', label='Search icon'),
        sui.IconButton('download', 'Download', selected=selected, enabled=enabled, description='Download file'),
    ], gap=8)
))

running = app.start()
snapshot = running.render()
assert snapshot.command_count > 0
assert snapshot.semantics_count >= 5
assert 'image' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Search icon' in snapshot.semantics_names, snapshot.semantics_names
assert 'Download' in snapshot.semantics_names, snapshot.semantics_names
assert checked.get() is False

running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(32, 18),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(32, 18),
    button='primary',
))

assert checked.get() is True
assert changes == [True]

selected.set(True)
enabled.set(False)
assert running.pending_count() == 2
assert running.drain() == 2
snapshot = running.render()
assert True in snapshot.semantics_selected, snapshot.semantics_selected
assert True in snapshot.semantics_disabled, snapshot.semantics_disabled
",
                c"controls.py",
                c"controls",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_link_and_color_swatch_callbacks_fire() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

opened = []
pressed = []
app = sui.App()
app.window(sui.Window('Callbacks').root(
    sui.Column([
        sui.Link('Documentation', 'https://example.invalid/docs', on_open=opened.append),
        sui.ColorSwatch(
            'Accent',
            sui.Color.rgba(0.25, 0.5, 0.75, 1.0),
            size=sui.Size(24, 24),
            on_press=lambda: pressed.append(True),
        ),
    ], gap=8)
))

running = app.start()
snapshot = running.render()
assert 'link' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'color_swatch' in snapshot.semantics_roles, snapshot.semantics_roles
assert '#4080BFFF' in snapshot.semantics_values, snapshot.semantics_values

running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(4, 4),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(4, 4),
    button='primary',
))

running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(12, 36),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(12, 36),
    button='primary',
))

assert opened == ['https://example.invalid/docs']
assert pressed == [True]
",
                c"link_color_swatch.py",
                c"link_color_swatch",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_select_updates_bound_state_from_keyboard() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

selected = sui.State(0)
changes = []
app = sui.App()
app.window(sui.Window('Select').root(
    sui.Select(
        'Mode',
        ['Draft', 'Final', 'Review'],
        selected=selected,
        placeholder='Choose mode',
        on_change=lambda index, value: changes.append((index, value)),
    )
))

running = app.start()
snapshot = running.render()
assert 'combo_box' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Draft' in snapshot.semantics_values, snapshot.semantics_values
assert selected.get() == 0

running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(20, 20),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(20, 20),
    button='primary',
))
running.handle_event(sui.Event.keyboard('ArrowDown'))
running.handle_event(sui.Event.keyboard('Enter'))

assert selected.get() == 1
assert changes == [(1, 'Final')]
snapshot = running.render()
assert 'Final' in snapshot.semantics_values, snapshot.semantics_values
",
                c"select.py",
                c"select",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_radio_button_updates_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

selected = sui.State(False)
calls = []
app = sui.App()
app.window(sui.Window('Radio').root(
    sui.RadioButton('Manual', selected, on_select=lambda: calls.append('selected'))
))

running = app.start()
snapshot = running.render()
assert 'radio_button' in snapshot.semantics_roles, snapshot.semantics_roles
assert selected.get() is False
running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(32, 18),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(32, 18),
    button='primary',
))

assert selected.get() is True
assert calls == ['selected']
",
                c"radio_button.py",
                c"radio_button",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_radio_group_updates_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

selected = sui.State(0)
changes = []
app = sui.App()
app.window(sui.Window('Radio').root(
    sui.RadioGroup(
        'Priority',
        ['Low', 'Medium', 'High'],
        selected=selected,
        on_change=lambda index, value: changes.append((index, value)),
    )
))

running = app.start()
snapshot = running.render()
assert 'radio_group' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Low' in snapshot.semantics_values, snapshot.semantics_values
assert selected.get() == 0
running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(20, 52),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(20, 52),
    button='primary',
))

assert selected.get() == 1
assert changes == [(1, 'Medium')]
snapshot = running.render()
assert 'Medium' in snapshot.semantics_values, snapshot.semantics_values
",
                c"radio_group.py",
                c"radio_group",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_list_view_updates_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

selected = sui.State(0)
changes = []
app = sui.App()
app.window(sui.Window('List').root(
    sui.ListView(
        'Assets',
        ['Brush', 'Canvas', 'Export'],
        selected=selected,
        on_change=lambda index, value: changes.append((index, value)),
    )
))

running = app.start()
snapshot = running.render()
assert 'list' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'list_item' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Brush' in snapshot.semantics_values, snapshot.semantics_values
assert selected.get() == 0
running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(44, 44),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up',
    sui.Point(44, 44),
    button='primary',
))

assert selected.get() == 1
assert changes == [(1, 'Canvas')]
snapshot = running.render()
assert 'Canvas' in snapshot.semantics_values, snapshot.semantics_values
assert True in snapshot.semantics_selected, snapshot.semantics_selected
",
                c"list_view.py",
                c"list_view",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_signal_meter_reads_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

active = sui.State(True)
app = sui.App()
app.window(sui.Window('Signal').root(
    sui.SignalMeter(
        'Input signal',
        active,
        description='Live audio input',
        bars=8,
        size=sui.Size(76, 16),
    )
))

running = app.start()
snapshot = running.render()
assert 'generic_container' in snapshot.semantics_roles, snapshot.semantics_roles
assert 'Input signal' in snapshot.semantics_names, snapshot.semantics_names
assert 'Live audio input' in snapshot.semantics_descriptions, snapshot.semantics_descriptions
assert 'active' in snapshot.semantics_values, snapshot.semantics_values

active.set(False)
assert running.pending_count() == 1
assert running.drain() == 1
snapshot = running.render()
assert 'idle' in snapshot.semantics_values, snapshot.semantics_values
",
                c"signal_meter.py",
                c"signal_meter",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_text_input_updates_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

text = sui.State('')
changes = []
app = sui.App()
app.window(sui.Window('Text').root(
    sui.TextInput('Name', text, placeholder='Type here', on_change=changes.append)
))

running = app.start()
assert running.render().command_count > 0
running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(32, 18),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.keyboard('a'))

assert text.get() == 'a'
assert changes == ['a']
",
                c"text_input.py",
                c"text_input",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_password_and_datetime_inputs_update_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

password = sui.State('')
password_changes = []
password_app = sui.App()
password_app.window(sui.Window('Password').root(
    sui.PasswordInput(
        'Password',
        password,
        placeholder='Enter a password',
        on_change=password_changes.append,
    )
))

password_running = password_app.start()
password_running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(32, 18),
    button='primary',
    buttons=1,
))
password_running.handle_event(sui.Event.keyboard('s'))

assert password.get() == 's'
assert password_changes == ['s']
password_snapshot = password_running.render()
assert 's' not in password_snapshot.semantics_values
assert '•' in password_snapshot.semantics_values

scheduled_for = sui.State('')
datetime_changes = []
datetime_app = sui.App()
datetime_app.window(sui.Window('Date/time').root(
    sui.DateTimeInput(
        'Scheduled for',
        scheduled_for,
        placeholder='YYYY-MM-DD HH:MM',
        on_change=datetime_changes.append,
    )
))

datetime_running = datetime_app.start()
datetime_running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(32, 18),
    button='primary',
    buttons=1,
))
datetime_running.handle_event(sui.Event.keyboard('2'))

assert scheduled_for.get() == '2'
assert datetime_changes == ['2']
datetime_snapshot = datetime_running.render()
assert '2' in datetime_snapshot.semantics_values
",
                c"password_datetime_input.py",
                c"password_datetime_input",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_reorderable_list_reports_positional_callback_arguments() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

changes = []
app = sui.App()
app.window(sui.Window('Reorder').root(
    sui.ReorderableList(
        'Tasks',
        [
            sui.SizedBox(width=120, height=30),
            sui.SizedBox(width=120, height=30),
            sui.SizedBox(width=120, height=30),
        ],
        spacing=0,
        drag_threshold=4,
        on_reorder=lambda item, from_index, to_index: changes.append(
            (item, from_index, to_index)
        ),
    )
))

running = app.start()
running.render()
running.handle_event(sui.Event.pointer(
    'down', sui.Point(10, 15), pointer_id=1, button='primary', buttons=1,
))
running.handle_event(sui.Event.pointer(
    'move', sui.Point(10, 48), pointer_id=1, button='primary', buttons=1,
))
running.handle_event(sui.Event.pointer(
    'move', sui.Point(10, 78), pointer_id=1, button='primary', buttons=1,
))
running.handle_event(sui.Event.pointer(
    'up', sui.Point(10, 78), pointer_id=1, button='primary', buttons=0,
))

assert changes == [(0, 0, 2)], changes
",
                c"reorderable_list.py",
                c"reorderable_list",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_text_area_updates_bound_state() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

text = sui.State('')
changes = []
app = sui.App()
app.window(sui.Window('Text').root(
    sui.TextArea('Notes', text, placeholder='Type notes', on_change=changes.append)
))

running = app.start()
snapshot = running.render()
assert snapshot.command_count > 0
assert True in snapshot.semantics_editable_multiline, snapshot.semantics_editable_multiline
running.handle_event(sui.Event.pointer(
    'down',
    sui.Point(32, 18),
    button='primary',
    buttons=1,
))
running.handle_event(sui.Event.keyboard('a'))

assert text.get() == 'a'
assert changes == ['a']
",
                c"text_area.py",
                c"text_area",
            )?;
            Ok(())
        })
    }

    #[test]
    fn python_running_app_drains_bound_state_updates() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

state = sui.State('Ready')
app = sui.App()
app.window(sui.Window('Runtime').root(sui.Label(state)))
running = app.start()
window = running.window_handle(0)

assert running.window_count() == 1
assert window.id in running.window_ids()
assert running.render_window(window).command_count > 0

state.set('Queued')
assert state.get() == 'Ready'
assert running.pending_count() == 1
assert running.drain() == 1
assert state.get() == 'Queued'
assert running.needs_render()

def update_on_ui():
    state.set('UI callback')

running.ui_handle().post(update_on_ui)
assert running.drain() == 1
assert running.pending_count() == 0
assert state.get() == 'UI callback'
assert running.render().command_count > 0
",
                c"running_app.py",
                c"running_app",
            )?;
            Ok(())
        })
    }

    #[cfg(not(feature = "desktop"))]
    #[test]
    fn python_app_run_reports_missing_desktop_feature() -> PyResult<()> {
        Python::attach(|py| {
            install_sui_module(py)?;
            PyModule::from_code(
                py,
                c"
import sui

app = sui.App()
app.window(sui.Window('Headless').root(sui.Label('No desktop')))

try:
    app.run()
    raise AssertionError('run should require the desktop feature')
except RuntimeError as error:
    assert 'desktop' in str(error)
",
                c"run_feature.py",
                c"run_feature",
            )?;
            Ok(())
        })
    }

    fn install_sui_module(py: Python<'_>) -> PyResult<()> {
        let module = PyModule::new(py, "sui")?;
        sui(&module)?;
        let sys_modules = py
            .import("sys")?
            .getattr("modules")?
            .cast_into::<PyDict>()?;
        sys_modules.set_item("sui", module)?;
        Ok(())
    }
}
