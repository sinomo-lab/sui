#![allow(
    clippy::large_enum_variant,
    clippy::too_many_arguments,
    clippy::wrong_self_convention
)]

use std::{
    fs,
    sync::{Arc, Mutex},
};

use ::sui as sui_crate;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use sui_bindings_core::{
    BindingAction, BindingApp, BindingBool, BindingBoolAction, BindingColorAction,
    BindingColorPaletteSwatch, BindingColorSelectAction, BindingCustomEvent, BindingEvent,
    BindingFontHandle, BindingImageFit, BindingImageHandle, BindingImeEvent, BindingKeyState,
    BindingKeyboardEvent, BindingLayerListItem, BindingMenuItem, BindingModifiers, BindingNumber,
    BindingNumberAction, BindingPointerButton, BindingPointerEvent, BindingPointerEventKind,
    BindingPointerKind, BindingRenderSnapshot, BindingRuntime, BindingScrollAxes,
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

#[napi(js_name = "Point")]
#[derive(Debug, Clone, Copy)]
pub struct JsPoint {
    pub x: f64,
    pub y: f64,
}

#[napi]
impl JsPoint {
    #[napi(constructor)]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<sui_crate::Point> for JsPoint {
    fn from(value: sui_crate::Point) -> Self {
        Self::new(f64::from(value.x), f64::from(value.y))
    }
}

impl From<JsPoint> for sui_crate::Point {
    fn from(value: JsPoint) -> Self {
        Self::new(value.x as f32, value.y as f32)
    }
}

impl From<JsPoint> for Vector {
    fn from(value: JsPoint) -> Self {
        Self::new(value.x as f32, value.y as f32)
    }
}

#[napi(js_name = "Modifiers")]
#[derive(Debug, Clone, Copy, Default)]
pub struct JsModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[napi]
impl JsModifiers {
    #[napi(constructor)]
    pub fn new(
        shift: Option<bool>,
        control: Option<bool>,
        alt: Option<bool>,
        meta: Option<bool>,
    ) -> Self {
        Self {
            shift: shift.unwrap_or(false),
            control: control.unwrap_or(false),
            alt: alt.unwrap_or(false),
            meta: meta.unwrap_or(false),
        }
    }
}

impl From<BindingModifiers> for JsModifiers {
    fn from(value: BindingModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

impl From<JsModifiers> for BindingModifiers {
    fn from(value: JsModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

#[napi(js_name = "Event")]
#[derive(Debug, Clone)]
pub struct JsEvent {
    inner: BindingEvent,
}

#[napi]
impl JsEvent {
    #[napi(factory)]
    pub fn pointer(
        kind: String,
        position: &JsPoint,
        pointer_id: Option<String>,
        delta: Option<&JsPoint>,
        button: Option<String>,
        buttons: Option<u32>,
        pointer_kind: Option<String>,
        is_primary: Option<bool>,
    ) -> Result<Self> {
        Ok(Self {
            inner: BindingEvent::Pointer(BindingPointerEvent {
                pointer_id: pointer_id
                    .as_deref()
                    .map(|id| parse_u64_string(id, "pointer id"))
                    .transpose()?
                    .unwrap_or(0),
                kind: pointer_event_kind_from_js(&kind)?,
                position: (*position).into(),
                delta: delta.copied().unwrap_or(JsPoint::new(0.0, 0.0)).into(),
                scroll_delta: None,
                button: button.as_deref().map(pointer_button_from_js).transpose()?,
                buttons: checked_buttons(buttons.unwrap_or(0))?,
                modifiers: BindingModifiers::default(),
                pointer_kind: pointer_kind_from_js(pointer_kind.as_deref().unwrap_or("mouse"))?,
                is_primary: is_primary.unwrap_or(true),
            }),
        })
    }

    #[napi(factory)]
    pub fn scroll(
        position: &JsPoint,
        delta: &JsPoint,
        mode: Option<String>,
        pointer_id: Option<String>,
    ) -> Result<Self> {
        let scroll_delta = match mode.as_deref().unwrap_or("pixels") {
            "pixels" | "pixel" => BindingScrollDelta::Pixels((*delta).into()),
            "lines" | "line" => BindingScrollDelta::Lines((*delta).into()),
            _ => return Err(napi_invalid_arg("scroll mode must be 'pixels' or 'lines'")),
        };
        Ok(Self {
            inner: BindingEvent::Pointer(BindingPointerEvent {
                pointer_id: pointer_id
                    .as_deref()
                    .map(|id| parse_u64_string(id, "pointer id"))
                    .transpose()?
                    .unwrap_or(0),
                kind: BindingPointerEventKind::Scroll,
                position: (*position).into(),
                delta: (*delta).into(),
                scroll_delta: Some(scroll_delta),
                button: None,
                buttons: 0,
                modifiers: BindingModifiers::default(),
                pointer_kind: BindingPointerKind::Mouse,
                is_primary: true,
            }),
        })
    }

    #[napi(factory)]
    pub fn keyboard(
        key: String,
        state: Option<String>,
        code: Option<String>,
        text: Option<String>,
        repeat: Option<bool>,
        is_composing: Option<bool>,
    ) -> Result<Self> {
        let mut event = BindingKeyboardEvent::new(
            key,
            key_state_from_js(state.as_deref().unwrap_or("pressed"))?,
        );
        if let Some(code) = code {
            event.code = code;
        }
        if text.is_some() {
            event.text = text;
        }
        event.repeat = repeat.unwrap_or(false);
        event.is_composing = is_composing.unwrap_or(false);
        Ok(Self {
            inner: BindingEvent::Keyboard(event),
        })
    }

    #[napi(factory)]
    pub fn ime(
        kind: String,
        text: Option<String>,
        cursor_start: Option<u32>,
        cursor_end: Option<u32>,
    ) -> Result<Self> {
        let inner = match kind.as_str() {
            "compositionStart" | "composition_start" | "start" => BindingImeEvent::CompositionStart,
            "compositionUpdate" | "composition_update" | "update" => {
                BindingImeEvent::CompositionUpdate {
                    text: text.unwrap_or_default(),
                    cursor_start: cursor_start.map(|value| value as usize),
                    cursor_end: cursor_end.map(|value| value as usize),
                }
            }
            "compositionCommit" | "composition_commit" | "commit" => {
                BindingImeEvent::CompositionCommit {
                    text: text.unwrap_or_default(),
                }
            }
            "compositionEnd" | "composition_end" | "end" => BindingImeEvent::CompositionEnd,
            _ => {
                return Err(napi_invalid_arg(
                    "IME kind must be 'compositionStart', 'compositionUpdate', 'compositionCommit', or 'compositionEnd'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Ime(inner),
        })
    }

    #[napi(factory)]
    pub fn window(
        kind: String,
        value: Option<bool>,
        size: Option<&JsSize>,
        scale_factor: Option<f64>,
        raw_dpi: Option<f64>,
        suggested_size: Option<&JsSize>,
    ) -> Result<Self> {
        let inner = match kind.as_str() {
            "closeRequested" | "close_requested" | "close" => BindingWindowEvent::CloseRequested,
            "resized" | "resize" => BindingWindowEvent::Resized(
                size.ok_or_else(|| napi_invalid_arg("resized window events require size"))?
                    .to_sui(),
            ),
            "scaleFactorChanged" | "scale_factor_changed" => {
                BindingWindowEvent::ScaleFactorChanged {
                    scale_factor: scale_factor.unwrap_or(1.0),
                    raw_dpi: raw_dpi.map(|value| value as f32),
                    suggested_size: suggested_size.map(JsSize::to_sui),
                }
            }
            "focused" | "focus" => BindingWindowEvent::Focused(value.unwrap_or(false)),
            "occluded" => BindingWindowEvent::Occluded(value.unwrap_or(false)),
            "redrawRequested" | "redraw_requested" | "redraw" => {
                BindingWindowEvent::RedrawRequested
            }
            _ => {
                return Err(napi_invalid_arg(
                    "window kind must be 'closeRequested', 'resized', 'scaleFactorChanged', 'focused', 'occluded', or 'redrawRequested'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Window(inner),
        })
    }

    #[napi(factory)]
    pub fn custom(kind: String, payload: Option<String>) -> Self {
        Self {
            inner: BindingEvent::Custom(BindingCustomEvent { kind, payload }),
        }
    }

    #[napi(getter)]
    pub fn kind(&self) -> String {
        self.inner.kind().to_string()
    }

    #[napi(getter)]
    pub fn action(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(pointer_event_kind_name(event.kind).to_string()),
            BindingEvent::Ime(event) => Some(ime_event_kind_name(event).to_string()),
            BindingEvent::Window(event) => Some(window_event_kind_name(event).to_string()),
            BindingEvent::Custom(_)
            | BindingEvent::Keyboard(_)
            | BindingEvent::Unsupported { .. } => None,
        }
    }

    #[napi(getter)]
    pub fn pointer_id(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.pointer_id.to_string()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn position(&self) -> Option<JsPoint> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.position.into()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn delta(&self) -> Option<JsPoint> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(JsPoint::new(
                f64::from(event.delta.x),
                f64::from(event.delta.y),
            )),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn scroll_mode(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => match event.scroll_delta {
                Some(BindingScrollDelta::Lines(_)) => Some("lines".to_owned()),
                Some(BindingScrollDelta::Pixels(_)) => Some("pixels".to_owned()),
                None => None,
            },
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn button(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => event.button.map(pointer_button_name),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn buttons(&self) -> Option<u32> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(u32::from(event.buttons)),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn modifiers(&self) -> Option<JsModifiers> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.modifiers.into()),
            BindingEvent::Keyboard(event) => Some(event.modifiers.into()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn device_kind(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(pointer_kind_name(event.pointer_kind).to_owned()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn is_primary(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.is_primary),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn key(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.key.clone()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn code(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.code.clone()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn text(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => event.text.clone(),
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate { text, .. })
            | BindingEvent::Ime(BindingImeEvent::CompositionCommit { text }) => Some(text.clone()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn state(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(key_state_name(event.state).to_owned()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn repeat(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.repeat),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn is_composing(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.is_composing),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn custom_kind(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Custom(event) => Some(event.kind.clone()),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn payload(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Custom(event) => event.payload.clone(),
            _ => None,
        }
    }

    #[napi(getter)]
    pub fn file_path(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::ExternalFileHovered(path))
            | BindingEvent::Window(BindingWindowEvent::ExternalFileDropped(path)) => {
                Some(path.clone())
            }
            _ => None,
        }
    }
}

impl JsEvent {
    fn from_binding(inner: BindingEvent) -> Self {
        Self { inner }
    }

    fn binding_event(&self) -> BindingEvent {
        self.inner.clone()
    }
}

#[napi(js_name = "Size")]
#[derive(Debug, Clone, Copy)]
pub struct JsSize {
    pub width: f64,
    pub height: f64,
}

#[napi]
impl JsSize {
    #[napi(constructor)]
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

impl JsSize {
    fn to_sui(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }
}

impl From<Size> for JsSize {
    fn from(value: Size) -> Self {
        Self::new(f64::from(value.width), f64::from(value.height))
    }
}

impl From<JsSize> for Size {
    fn from(value: JsSize) -> Self {
        Self::new(value.width as f32, value.height as f32)
    }
}

#[napi(js_name = "Rect")]
#[derive(Debug, Clone, Copy)]
pub struct JsRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[napi]
impl JsRect {
    #[napi(constructor)]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[napi(getter)]
    pub fn origin(&self) -> JsPoint {
        JsPoint::new(self.x, self.y)
    }

    #[napi(getter)]
    pub fn size(&self) -> JsSize {
        JsSize::new(self.width, self.height)
    }
}

impl From<Rect> for JsRect {
    fn from(value: Rect) -> Self {
        Self::new(
            f64::from(value.x()),
            f64::from(value.y()),
            f64::from(value.width()),
            f64::from(value.height()),
        )
    }
}

impl From<JsRect> for Rect {
    fn from(value: JsRect) -> Self {
        Self::new(
            value.x as f32,
            value.y as f32,
            value.width as f32,
            value.height as f32,
        )
    }
}

#[napi(js_name = "Path")]
#[derive(Debug, Clone)]
pub struct JsPath {
    inner: Path,
}

#[napi]
impl JsPath {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: Path::new() }
    }

    #[napi(factory)]
    pub fn rect(rect: &JsRect) -> Self {
        Self {
            inner: Path::rect((*rect).into()),
        }
    }

    #[napi(factory)]
    pub fn circle(center: &JsPoint, radius: f64) -> Self {
        Self {
            inner: Path::circle((*center).into(), radius as f32),
        }
    }

    #[napi(factory, js_name = "roundedRect")]
    pub fn rounded_rect(rect: &JsRect, radius: f64) -> Self {
        Self {
            inner: Path::rounded_rect((*rect).into(), radius as f32),
        }
    }

    #[napi(factory)]
    pub fn arc(center: &JsPoint, radius: f64, start_angle: f64, sweep_angle: f64) -> Self {
        Self {
            inner: Path::arc(
                (*center).into(),
                radius as f32,
                start_angle as f32,
                sweep_angle as f32,
            ),
        }
    }

    #[napi(getter)]
    pub fn bounds(&self) -> JsRect {
        self.inner.bounds().into()
    }

    #[napi(getter)]
    pub fn element_count(&self) -> u32 {
        self.inner.elements().len() as u32
    }

    #[napi(js_name = "isEmpty")]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for JsPath {
    fn default() -> Self {
        Self::new()
    }
}

#[napi(js_name = "PathBuilder")]
#[derive(Debug, Clone, Default)]
pub struct JsPathBuilder {
    inner: PathBuilder,
}

#[napi]
impl JsPathBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: PathBuilder::new(),
        }
    }

    #[napi(js_name = "moveTo")]
    pub fn move_to(&mut self, point: &JsPoint) {
        self.inner.move_to((*point).into());
    }

    #[napi(js_name = "lineTo")]
    pub fn line_to(&mut self, point: &JsPoint) {
        self.inner.line_to((*point).into());
    }

    #[napi(js_name = "quadTo")]
    pub fn quad_to(&mut self, ctrl: &JsPoint, to: &JsPoint) {
        self.inner.quad_to((*ctrl).into(), (*to).into());
    }

    #[napi(js_name = "cubicTo")]
    pub fn cubic_to(&mut self, ctrl1: &JsPoint, ctrl2: &JsPoint, to: &JsPoint) {
        self.inner
            .cubic_to((*ctrl1).into(), (*ctrl2).into(), (*to).into());
    }

    #[napi]
    pub fn close(&mut self) {
        self.inner.close();
    }

    #[napi(js_name = "pushRect")]
    pub fn push_rect(&mut self, rect: &JsRect) {
        self.inner.push_rect((*rect).into());
    }

    #[napi(js_name = "pushCircle")]
    pub fn push_circle(&mut self, center: &JsPoint, radius: f64) {
        self.inner.push_circle((*center).into(), radius as f32);
    }

    #[napi(js_name = "pushRoundedRect")]
    pub fn push_rounded_rect(&mut self, rect: &JsRect, radius: f64) {
        self.inner.push_rounded_rect((*rect).into(), radius as f32);
    }

    #[napi(js_name = "pushArc")]
    pub fn push_arc(&mut self, center: &JsPoint, radius: f64, start_angle: f64, sweep_angle: f64) {
        self.inner.push_arc(
            (*center).into(),
            radius as f32,
            start_angle as f32,
            sweep_angle as f32,
        );
    }

    #[napi]
    pub fn build(&self) -> JsPath {
        JsPath {
            inner: self.inner.clone().build(),
        }
    }
}

#[napi(js_name = "Transform")]
#[derive(Debug, Clone, Copy)]
pub struct JsTransform {
    pub xx: f64,
    pub yx: f64,
    pub xy: f64,
    pub yy: f64,
    pub dx: f64,
    pub dy: f64,
}

#[napi]
impl JsTransform {
    #[napi(constructor)]
    pub fn new(xx: f64, yx: f64, xy: f64, yy: f64, dx: f64, dy: f64) -> Self {
        Self {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        }
    }

    #[napi(factory)]
    pub fn identity() -> Self {
        Self::from(Transform::IDENTITY)
    }

    #[napi(factory)]
    pub fn translation(x: f64, y: f64) -> Self {
        Self::from(Transform::translation(x as f32, y as f32))
    }

    #[napi(factory)]
    pub fn scale(x: f64, y: f64) -> Self {
        Self::from(Transform::scale(x as f32, y as f32))
    }

    #[napi(factory)]
    pub fn rotation(radians: f64) -> Self {
        Self::from(Transform::rotation(radians as f32))
    }

    #[napi]
    pub fn then(&self, next: &JsTransform) -> Self {
        Transform::from(*self).then((*next).into()).into()
    }
}

impl From<Transform> for JsTransform {
    fn from(value: Transform) -> Self {
        Self::new(
            f64::from(value.xx),
            f64::from(value.yx),
            f64::from(value.xy),
            f64::from(value.yy),
            f64::from(value.dx),
            f64::from(value.dy),
        )
    }
}

impl From<JsTransform> for Transform {
    fn from(value: JsTransform) -> Self {
        Self::new(
            value.xx as f32,
            value.yx as f32,
            value.xy as f32,
            value.yy as f32,
            value.dx as f32,
            value.dy as f32,
        )
    }
}

#[napi(js_name = "Color")]
#[derive(Debug, Clone, Copy)]
pub struct JsColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[napi]
impl JsColor {
    #[napi(constructor)]
    pub fn new(red: f64, green: f64, blue: f64, alpha: Option<f64>) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: alpha.unwrap_or(1.0),
        }
    }
}

impl From<JsColor> for Color {
    fn from(value: JsColor) -> Self {
        Self::rgba(
            value.red as f32,
            value.green as f32,
            value.blue as f32,
            value.alpha as f32,
        )
    }
}

impl From<Color> for JsColor {
    fn from(value: Color) -> Self {
        Self::new(
            f64::from(value.red),
            f64::from(value.green),
            f64::from(value.blue),
            Some(f64::from(value.alpha)),
        )
    }
}

#[napi(js_name = "Shadow")]
#[derive(Debug, Clone, Copy)]
pub struct JsShadow {
    offset_x: f64,
    offset_y: f64,
    blur: f64,
    spread: f64,
    color: JsColor,
}

#[napi]
impl JsShadow {
    #[napi(constructor)]
    pub fn new(offset_x: f64, offset_y: f64, blur: f64, spread: f64, color: &JsColor) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            spread,
            color: *color,
        }
    }

    #[napi(getter, js_name = "offsetX")]
    pub fn offset_x(&self) -> f64 {
        self.offset_x
    }

    #[napi(getter, js_name = "offsetY")]
    pub fn offset_y(&self) -> f64 {
        self.offset_y
    }

    #[napi(getter)]
    pub fn blur(&self) -> f64 {
        self.blur
    }

    #[napi(getter)]
    pub fn spread(&self) -> f64 {
        self.spread
    }

    #[napi(getter)]
    pub fn color(&self) -> JsColor {
        self.color
    }
}

impl From<JsShadow> for ShadowParams {
    fn from(value: JsShadow) -> Self {
        Self {
            offset_x: value.offset_x as f32,
            offset_y: value.offset_y as f32,
            blur: value.blur as f32,
            spread: value.spread as f32,
            color: value.color.into(),
        }
    }
}

#[napi(js_name = "Constraints")]
#[derive(Debug, Clone, Copy)]
pub struct JsConstraints {
    min: JsSize,
    max: JsSize,
}

#[napi]
impl JsConstraints {
    #[napi(constructor)]
    pub fn new(min: &JsSize, max: &JsSize) -> Self {
        Self {
            min: *min,
            max: *max,
        }
    }

    #[napi(getter)]
    pub fn min(&self) -> JsSize {
        self.min
    }

    #[napi(getter)]
    pub fn max(&self) -> JsSize {
        self.max
    }

    #[napi]
    pub fn clamp(&self, size: &JsSize) -> JsSize {
        self.to_sui().clamp((*size).into()).into()
    }

    #[napi]
    pub fn loosen(&self) -> Self {
        Self::from(self.to_sui().loosen())
    }
}

impl JsConstraints {
    fn to_sui(&self) -> Constraints {
        Constraints::new(self.min.into(), self.max.into())
    }
}

impl From<Constraints> for JsConstraints {
    fn from(value: Constraints) -> Self {
        Self {
            min: value.min.into(),
            max: value.max.into(),
        }
    }
}

#[napi(js_name = "FontHandle")]
#[derive(Debug, Clone, Copy)]
pub struct JsFontHandle {
    inner: BindingFontHandle,
}

#[napi]
impl JsFontHandle {
    #[napi(constructor)]
    pub fn new(id: String) -> Result<Self> {
        Ok(Self {
            inner: BindingFontHandle::new(parse_u64_string(&id, "font handle id")?),
        })
    }

    #[napi(getter)]
    pub fn id(&self) -> String {
        self.inner.get().to_string()
    }
}

include!("generated_widgets.rs");

#[napi(js_name = "ImageHandle")]
#[derive(Debug, Clone, Copy)]
pub struct JsImageHandle {
    inner: BindingImageHandle,
}

#[napi]
impl JsImageHandle {
    #[napi(constructor)]
    pub fn new(id: String) -> Result<Self> {
        Ok(Self {
            inner: BindingImageHandle::new(parse_u64_string(&id, "image handle id")?),
        })
    }

    #[napi(factory)]
    pub fn local(slot: u32) -> Self {
        Self {
            inner: BindingImageHandle::local(u64::from(slot)),
        }
    }

    #[napi(getter)]
    pub fn id(&self) -> String {
        self.inner.get().to_string()
    }

    #[napi(getter, js_name = "localSlot")]
    pub fn local_slot(&self) -> Option<u32> {
        self.inner
            .local_slot()
            .and_then(|slot| u32::try_from(slot).ok())
    }
}

#[derive(Clone)]
struct PendingPaintImage {
    slot: u64,
    image: RegisteredImage,
}

#[napi(js_name = "Paint")]
#[derive(Clone)]
pub struct JsPaint {
    builder: Arc<Mutex<PaintCommandBuilder>>,
    images: Arc<Mutex<Vec<PendingPaintImage>>>,
    bounds: JsRect,
}

#[napi]
impl JsPaint {
    #[napi(getter)]
    pub fn bounds(&self) -> JsRect {
        self.bounds
    }

    #[napi]
    pub fn clear(&self, color: &JsColor) -> Result<()> {
        self.with_builder(|builder| builder.clear((*color).into()).map(|_| ()))
    }

    #[napi(js_name = "fillRect")]
    pub fn fill_rect(&self, rect: &JsRect, color: &JsColor) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .fill_rect((*rect).into(), Color::from(*color))
                .map(|_| ())
        })
    }

    #[napi(js_name = "strokeRect")]
    pub fn stroke_rect(&self, rect: &JsRect, color: &JsColor, width: Option<f64>) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .stroke_rect(
                    (*rect).into(),
                    Color::from(*color),
                    StrokeStyle::new(width.unwrap_or(1.0) as f32),
                )
                .map(|_| ())
        })
    }

    #[napi(js_name = "fillPath")]
    pub fn fill_path(&self, path: &JsPath, color: &JsColor) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .fill_path(path.inner.clone(), Color::from(*color))
                .map(|_| ())
        })
    }

    #[napi(js_name = "strokePath")]
    pub fn stroke_path(&self, path: &JsPath, color: &JsColor, width: Option<f64>) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .stroke_path(
                    path.inner.clone(),
                    Color::from(*color),
                    StrokeStyle::new(width.unwrap_or(1.0) as f32),
                )
                .map(|_| ())
        })
    }

    #[napi(js_name = "fillRoundedRect")]
    pub fn fill_rounded_rect(
        &self,
        rect: &JsRect,
        color: &JsColor,
        radius: Option<f64>,
    ) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .fill_rrect((*rect).into(), js_radii(radius), Color::from(*color))
                .map(|_| ())
        })
    }

    #[napi(js_name = "drawShadow")]
    pub fn draw_shadow(&self, rect: &JsRect, shadow: &JsShadow, radius: Option<f64>) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .draw_shadow((*rect).into(), js_radii(radius), (*shadow).into())
                .map(|_| ())
        })
    }

    #[napi(js_name = "fillRoundedRectWithShadow")]
    pub fn fill_rounded_rect_with_shadow(
        &self,
        rect: &JsRect,
        color: &JsColor,
        shadow: &JsShadow,
        radius: Option<f64>,
    ) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .fill_rrect_with_shadow(
                    (*rect).into(),
                    js_radii(radius),
                    Color::from(*color),
                    (*shadow).into(),
                )
                .map(|_| ())
        })
    }

    #[napi(js_name = "fillBounds")]
    pub fn fill_bounds(&self, color: &JsColor) -> Result<()> {
        self.fill_rect(&self.bounds, color)
    }

    #[napi(js_name = "drawText")]
    pub fn draw_text(
        &self,
        rect: &JsRect,
        text: String,
        color: Option<&JsColor>,
        font_size: Option<f64>,
        line_height: Option<f64>,
        font: Option<&JsFontHandle>,
        weight: Option<u32>,
        style: Option<String>,
        stretch: Option<String>,
    ) -> Result<()> {
        let style = js_text_style(
            color,
            font_size,
            line_height,
            font,
            weight,
            style.as_deref(),
            stretch.as_deref(),
        )?;
        self.with_builder(|builder| builder.draw_text((*rect).into(), text, style).map(|_| ()))
    }

    #[napi(js_name = "drawShaderRect")]
    pub fn draw_shader_rect(&self, rect: &JsRect, shader: &JsShader) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .draw_binding_shader_rect((*rect).into(), shader.inner)
                .map(|_| ())
        })
    }

    #[napi(js_name = "rgbaImage")]
    pub fn rgba_image(
        &self,
        slot: u32,
        width: u32,
        height: u32,
        pixels: Buffer,
    ) -> Result<JsImageHandle> {
        let image = RegisteredImage::from_rgba8(width, height, pixels.as_ref().to_vec())
            .map_err(napi_invalid_arg)?;
        recover_lock(&self.images).push(PendingPaintImage {
            slot: u64::from(slot),
            image,
        });
        Ok(JsImageHandle {
            inner: BindingImageHandle::local(u64::from(slot)),
        })
    }

    #[napi(js_name = "drawImage")]
    pub fn draw_image(&self, rect: &JsRect, image: &JsImageHandle) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .draw_binding_image((*rect).into(), image.inner)
                .map(|_| ())
        })
    }

    #[napi(js_name = "drawImageQuad")]
    pub fn draw_image_quad(&self, points: Array<'_>, image: &JsImageHandle) -> Result<()> {
        let points = js_four_points(&points)?;
        self.with_builder(|builder| {
            builder
                .draw_binding_image_quad(points, image.inner)
                .map(|_| ())
        })
    }

    #[napi(js_name = "pushClipRect")]
    pub fn push_clip_rect(&self, rect: &JsRect) -> Result<()> {
        self.with_builder(|builder| builder.push_clip_rect((*rect).into()).map(|_| ()))
    }

    #[napi(js_name = "pushClipPath")]
    pub fn push_clip_path(&self, path: &JsPath) -> Result<()> {
        self.with_builder(|builder| builder.push_clip_path(path.inner.clone()).map(|_| ()))
    }

    #[napi(js_name = "popClip")]
    pub fn pop_clip(&self) -> Result<()> {
        self.with_builder(|builder| builder.pop_clip().map(|_| ()))
    }

    #[napi(js_name = "pushTransform")]
    pub fn push_transform(&self, transform: &JsTransform) -> Result<()> {
        self.with_builder(|builder| builder.push_transform((*transform).into()).map(|_| ()))
    }

    #[napi(js_name = "popTransform")]
    pub fn pop_transform(&self) -> Result<()> {
        self.with_builder(|builder| builder.pop_transform().map(|_| ()))
    }

    #[napi(getter)]
    pub fn command_count(&self) -> u32 {
        recover_lock(&self.builder).command_count() as u32
    }
}

impl JsPaint {
    fn new(bounds: Rect) -> Self {
        Self {
            builder: Arc::new(Mutex::new(PaintCommandBuilder::new())),
            images: Arc::new(Mutex::new(Vec::new())),
            bounds: bounds.into(),
        }
    }

    fn with_builder(
        &self,
        f: impl FnOnce(&mut PaintCommandBuilder) -> std::result::Result<(), PaintValidationError>,
    ) -> Result<()> {
        f(&mut recover_lock(&self.builder)).map_err(napi_value_error)
    }

    fn finish(&self) -> std::result::Result<Vec<PaintCommand>, PaintValidationError> {
        std::mem::take(&mut *recover_lock(&self.builder)).finish()
    }

    fn take_images(&self) -> Vec<PendingPaintImage> {
        std::mem::take(&mut *recover_lock(&self.images))
    }
}

#[derive(Clone)]
enum JsSemanticsCommand {
    Node(SemanticsNode),
    Child(usize),
}

#[napi(js_name = "Semantics")]
#[derive(Clone)]
pub struct JsSemantics {
    widget_id: WidgetId,
    commands: Arc<Mutex<Vec<JsSemanticsCommand>>>,
    bounds: JsRect,
    focused: bool,
    child_count: u32,
}

#[napi]
impl JsSemantics {
    #[napi(getter)]
    pub fn bounds(&self) -> JsRect {
        self.bounds
    }

    #[napi(getter)]
    pub fn focused(&self) -> bool {
        self.focused
    }

    #[napi(getter, js_name = "childCount")]
    pub fn child_count(&self) -> u32 {
        self.child_count
    }

    #[napi]
    pub fn node(
        &self,
        role: Option<String>,
        name: Option<String>,
        value: Option<Either3<String, f64, bool>>,
        description: Option<String>,
        bounds: Option<&JsRect>,
        disabled: Option<bool>,
        checked: Option<String>,
        selected: Option<bool>,
        expanded: Option<bool>,
        busy: Option<bool>,
        min_value: Option<f64>,
        max_value: Option<f64>,
    ) -> Result<()> {
        let role_name = role.unwrap_or_else(|| "generic_container".to_owned());
        let role = binding_semantics_role_from_name(&role_name)
            .ok_or_else(|| napi_invalid_arg(format!("unknown semantics role '{role_name}'")))?;
        let mut node = SemanticsNode::new(
            self.widget_id,
            role,
            bounds.copied().unwrap_or(self.bounds).into(),
        );
        node.name = name;
        node.description = description;
        node.value = js_semantics_value(value, min_value, max_value);
        node.state.disabled = disabled.unwrap_or(false);
        node.state.focused = self.focused;
        node.state.checked = js_toggle_state(checked.as_deref())?;
        node.state.selected = selected.unwrap_or(false);
        node.state.expanded = expanded;
        node.state.busy = busy.unwrap_or(false);
        recover_lock(&self.commands).push(JsSemanticsCommand::Node(node));
        Ok(())
    }

    #[napi]
    pub fn child(&self, index: u32) -> bool {
        if index >= self.child_count {
            return false;
        }
        recover_lock(&self.commands).push(JsSemanticsCommand::Child(index as usize));
        true
    }
}

impl JsSemantics {
    fn new(widget_id: WidgetId, bounds: Rect, focused: bool, child_count: usize) -> Self {
        Self {
            widget_id,
            commands: Arc::new(Mutex::new(Vec::new())),
            bounds: bounds.into(),
            focused,
            child_count: child_count as u32,
        }
    }

    fn take_commands(&self) -> Vec<JsSemanticsCommand> {
        std::mem::take(&mut *recover_lock(&self.commands))
    }
}

#[napi(js_name = "Shader")]
#[derive(Debug, Clone, Copy)]
pub struct JsShader {
    inner: BindingShader,
}

#[napi]
impl JsShader {
    #[napi(factory, js_name = "colorWheel")]
    pub fn color_wheel() -> Self {
        Self {
            inner: BindingShader::color_wheel(),
        }
    }

    #[napi(factory, js_name = "hueBar")]
    pub fn hue_bar() -> Self {
        Self {
            inner: BindingShader::hue_bar(),
        }
    }

    #[napi(factory, js_name = "saturationValuePlane")]
    pub fn saturation_value_plane(
        hue: f64,
        max_value: Option<f64>,
        color_space: Option<String>,
    ) -> Result<Self> {
        BindingShader::saturation_value_plane(
            color_space_from_js(color_space)?,
            hue as f32,
            max_value.unwrap_or(1.0) as f32,
        )
        .map(Self::from)
        .map_err(napi_value_error)
    }

    #[napi(factory, js_name = "saturationBar")]
    pub fn saturation_bar(hue: f64, value: f64, color_space: Option<String>) -> Result<Self> {
        BindingShader::saturation_bar(color_space_from_js(color_space)?, hue as f32, value as f32)
            .map(Self::from)
            .map_err(napi_value_error)
    }

    #[napi(factory, js_name = "valueBar")]
    pub fn value_bar(
        hue: f64,
        saturation: f64,
        max_value: Option<f64>,
        color_space: Option<String>,
    ) -> Result<Self> {
        BindingShader::value_bar(
            color_space_from_js(color_space)?,
            hue as f32,
            saturation as f32,
            max_value.unwrap_or(1.0) as f32,
        )
        .map(Self::from)
        .map_err(napi_value_error)
    }

    #[napi(factory, js_name = "alphaBar")]
    pub fn alpha_bar(color: &JsColor) -> Result<Self> {
        BindingShader::alpha_bar(Color::from(*color))
            .map(Self::from)
            .map_err(napi_value_error)
    }

    #[napi(factory, js_name = "rgbChannelBar")]
    pub fn rgb_channel_bar(color: &JsColor, channel: u32, max_value: Option<f64>) -> Result<Self> {
        BindingShader::rgb_channel_bar(
            Color::from(*color),
            channel,
            max_value.unwrap_or(1.0) as f32,
        )
        .map(Self::from)
        .map_err(napi_value_error)
    }
}

impl From<BindingShader> for JsShader {
    fn from(value: BindingShader) -> Self {
        Self { inner: value }
    }
}

#[napi(js_name = "Widget")]
pub struct JsWidget {
    kind: JsWidgetKind,
}

enum JsWidgetKind {
    Foreign {
        callbacks: Mutex<Option<JsObjectCallbacks>>,
    },
    Binding(BindingWidget),
}

#[napi]
impl JsWidget {
    #[napi(constructor)]
    pub fn new(env: Env, callbacks: Object<'_>) -> Result<Self> {
        Ok(Self {
            kind: JsWidgetKind::Foreign {
                callbacks: Mutex::new(Some(JsObjectCallbacks::new(env, callbacks)?)),
            },
        })
    }
}

impl JsWidget {
    fn from_binding(widget: BindingWidget) -> Self {
        Self {
            kind: JsWidgetKind::Binding(widget),
        }
    }

    fn binding_widget(&self) -> Result<BindingWidget> {
        match &self.kind {
            JsWidgetKind::Binding(widget) => Ok(widget.clone()),
            JsWidgetKind::Foreign { callbacks } => {
                let callbacks = recover_lock(callbacks)
                    .as_ref()
                    .ok_or_else(|| napi_runtime_error("JavaScript widget callbacks were released"))?
                    .clone_ref()?;
                Ok(BindingWidget::foreign(JsWidgetCallbacks {
                    callbacks: Mutex::new(Some(callbacks)),
                }))
            }
        }
    }

    fn into_sui_widget(&self) -> Result<ForeignWidget> {
        match &self.kind {
            JsWidgetKind::Foreign { callbacks } => {
                let callbacks = recover_lock(callbacks)
                    .as_ref()
                    .ok_or_else(|| napi_runtime_error("JavaScript widget callbacks were released"))?
                    .clone_ref()?;
                Ok(ForeignWidget::new(JsWidgetCallbacks {
                    callbacks: Mutex::new(Some(callbacks)),
                }))
            }
            JsWidgetKind::Binding(_) => Err(napi_invalid_arg(
                "declarative JavaScript widgets are rendered through BindingApp",
            )),
        }
    }
}

#[napi(js_name = "State")]
#[derive(Clone)]
pub struct JsState {
    inner: BindingState,
}

#[napi]
impl JsState {
    #[napi(constructor)]
    pub fn new(value: Either3<String, f64, bool>) -> Self {
        Self {
            inner: BindingState::new(binding_value_from_js(value)),
        }
    }

    #[napi]
    pub fn get(&self) -> Either3<String, f64, bool> {
        binding_value_to_js(self.inner.get())
    }

    #[napi]
    pub fn set(&self, value: Either3<String, f64, bool>) {
        self.inner.set(binding_value_from_js(value));
    }

    #[napi(getter)]
    pub fn text(&self) -> String {
        self.inner.label_text()
    }
}

#[napi(js_name = "Window")]
#[derive(Clone)]
pub struct JsWindow {
    title: String,
    root: Option<BindingWidget>,
}

#[napi]
impl JsWindow {
    #[napi(constructor)]
    pub fn new(title: String) -> Self {
        Self { title, root: None }
    }

    #[napi]
    pub fn root(&mut self, widget: &JsWidget) -> Result<()> {
        self.root = Some(widget.binding_widget()?);
        Ok(())
    }
}

impl JsWindow {
    fn to_binding(&self) -> Result<BindingWindow> {
        let root = self
            .root
            .clone()
            .ok_or_else(|| napi_invalid_arg("window root has not been set"))?;
        Ok(BindingWindow::new(self.title.clone(), root))
    }
}

#[napi(js_name = "App")]
pub struct JsApp {
    inner: Mutex<BindingApp>,
}

#[napi]
impl JsApp {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(BindingApp::new()),
        }
    }

    #[napi]
    pub fn window(&self, window: &JsWindow) -> Result<()> {
        recover_lock(&self.inner).push_window(window.to_binding()?);
        Ok(())
    }

    #[napi]
    pub fn render(&self, index: Option<u32>) -> Result<JsRenderSnapshot> {
        recover_lock(&self.inner)
            .render_window(index.unwrap_or(0) as usize)
            .map(JsRenderSnapshot::from)
            .map_err(napi_runtime_error)
    }

    #[napi]
    pub fn start(&self) -> Result<JsRunningApp> {
        recover_lock(&self.inner)
            .start()
            .map(JsRunningApp::new)
            .map_err(napi_runtime_error)
    }

    #[napi(js_name = "rgbaImage")]
    pub fn rgba_image(&self, width: u32, height: u32, pixels: Buffer) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_rgba_image(width, height, pixels.as_ref().to_vec())
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "pngImage")]
    pub fn png_image(&self, png: Buffer) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_png_image(png.as_ref())
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "pngFile")]
    pub fn png_file(&self, path: String) -> Result<JsImageHandle> {
        let data = fs::read(&path).map_err(|error| {
            napi_runtime_error(format!("failed to read PNG file '{path}': {error}"))
        })?;
        recover_lock(&self.inner)
            .register_png_image(data)
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "svgImage")]
    pub fn svg_image(&self, svg: Buffer) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_svg_image(svg.as_ref())
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "svgFile")]
    pub fn svg_file(&self, path: String) -> Result<JsImageHandle> {
        let data = fs::read(&path).map_err(|error| {
            napi_runtime_error(format!("failed to read SVG file '{path}': {error}"))
        })?;
        recover_lock(&self.inner)
            .register_svg_image(data)
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "svgImageAtSize")]
    pub fn svg_image_at_size(&self, width: u32, height: u32, svg: Buffer) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_svg_image_at_size(width, height, svg.as_ref())
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "svgFileAtSize")]
    pub fn svg_file_at_size(&self, width: u32, height: u32, path: String) -> Result<JsImageHandle> {
        let data = fs::read(&path).map_err(|error| {
            napi_runtime_error(format!("failed to read SVG file '{path}': {error}"))
        })?;
        recover_lock(&self.inner)
            .register_svg_image_at_size(width, height, data)
            .map(|inner| JsImageHandle { inner })
            .map_err(napi_invalid_arg)
    }

    #[napi(js_name = "fontBytes")]
    pub fn font_bytes(&self, data: Buffer) -> Result<JsFontHandle> {
        recover_lock(&self.inner)
            .register_font_bytes(data.as_ref().to_vec())
            .map(|inner| JsFontHandle { inner })
            .map_err(napi_runtime_error)
    }

    #[napi(js_name = "fontFile")]
    pub fn font_file(&self, path: String) -> Result<JsFontHandle> {
        let data = fs::read(&path).map_err(|error| {
            napi_runtime_error(format!("failed to read font file '{path}': {error}"))
        })?;
        recover_lock(&self.inner)
            .register_font_bytes(data)
            .map(|inner| JsFontHandle { inner })
            .map_err(napi_runtime_error)
    }

    #[napi]
    pub fn run(&self) -> Result<()> {
        recover_lock(&self.inner).run().map_err(napi_runtime_error)
    }

    #[napi(js_name = "runWithHandle")]
    pub fn run_with_handle(&self, callback: Function<'_, JsUiHandle, ()>) -> Result<()> {
        recover_lock(&self.inner)
            .run_with_handle(move |handle| {
                let _ = callback.call(JsUiHandle { inner: handle });
            })
            .map_err(napi_runtime_error)
    }

    #[napi(getter)]
    pub fn window_count(&self) -> u32 {
        recover_lock(&self.inner).window_count() as u32
    }

    #[napi(getter)]
    pub fn image_resource_count(&self) -> u32 {
        recover_lock(&self.inner).image_resource_count() as u32
    }

    #[napi(getter)]
    pub fn font_resource_count(&self) -> u32 {
        recover_lock(&self.inner).font_resource_count() as u32
    }
}

impl Default for JsApp {
    fn default() -> Self {
        Self::new()
    }
}

#[napi(js_name = "WindowHandle")]
#[derive(Debug, Clone, Copy)]
pub struct JsWindowHandle {
    inner: BindingWindowId,
}

#[napi]
impl JsWindowHandle {
    #[napi(constructor)]
    pub fn new(id: String) -> Result<Self> {
        let id = id.parse::<u64>().map_err(napi_invalid_arg)?;
        Ok(Self {
            inner: BindingWindowId::new(id),
        })
    }

    #[napi(getter)]
    pub fn id(&self) -> String {
        self.inner.get().to_string()
    }
}

impl From<BindingWindowId> for JsWindowHandle {
    fn from(value: BindingWindowId) -> Self {
        Self { inner: value }
    }
}

#[napi(js_name = "UiHandle")]
#[derive(Clone)]
pub struct JsUiHandle {
    inner: BindingUiHandle,
}

#[napi]
impl JsUiHandle {
    #[napi]
    pub fn post(&self, env: Env, callback: Function<'_, (), ()>) -> Result<()> {
        let env = JsEnvHandle::from_env(env);
        let callback = callback.create_ref()?;
        self.inner.post(move || {
            let env = env.to_env();
            if let Ok(callback) = callback.borrow_back(&env) {
                let _ = callback.call(());
            }
        });
        Ok(())
    }

    #[napi(getter)]
    pub fn pending_count(&self) -> u32 {
        self.inner.pending_count() as u32
    }
}

impl From<BindingUiHandle> for JsUiHandle {
    fn from(value: BindingUiHandle) -> Self {
        Self { inner: value }
    }
}

#[napi(js_name = "RunningApp")]
pub struct JsRunningApp {
    inner: Mutex<BindingRuntime>,
}

#[napi]
impl JsRunningApp {
    #[napi(js_name = "uiHandle")]
    pub fn ui_handle(&self) -> JsUiHandle {
        recover_lock(&self.inner).ui_handle().into()
    }

    #[napi]
    pub fn drain(&self) -> Result<u32> {
        recover_lock(&self.inner)
            .drain_ui_tasks()
            .map(|count| count as u32)
            .map_err(napi_runtime_error)
    }

    #[napi]
    pub fn render(&self, index: Option<u32>) -> Result<JsRenderSnapshot> {
        recover_lock(&self.inner)
            .render_window_at(index.unwrap_or(0) as usize)
            .map(JsRenderSnapshot::from)
            .map_err(napi_runtime_error)
    }

    #[napi(js_name = "renderWindow")]
    pub fn render_window(&self, window: &JsWindowHandle) -> Result<JsRenderSnapshot> {
        recover_lock(&self.inner)
            .render_window(window.inner)
            .map(JsRenderSnapshot::from)
            .map_err(napi_runtime_error)
    }

    #[napi(js_name = "needsRender")]
    pub fn needs_render(&self, index: Option<u32>) -> Result<bool> {
        let runtime = recover_lock(&self.inner);
        let window_id = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(napi_runtime_error)?;
        runtime.needs_render(window_id).map_err(napi_runtime_error)
    }

    #[napi(js_name = "requestRedraw")]
    pub fn request_redraw(&self, index: Option<u32>) -> Result<()> {
        let mut runtime = recover_lock(&self.inner);
        let window_id = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(napi_runtime_error)?;
        runtime
            .request_redraw(window_id)
            .map_err(napi_runtime_error)
    }

    #[napi(js_name = "handleEvent")]
    pub fn handle_event(&self, event: &JsEvent, index: Option<u32>) -> Result<()> {
        recover_lock(&self.inner)
            .handle_event_at(index.unwrap_or(0) as usize, event.binding_event())
            .map_err(napi_runtime_error)
    }

    #[napi(getter)]
    pub fn window_count(&self) -> u32 {
        recover_lock(&self.inner).window_count() as u32
    }

    #[napi(js_name = "windowId")]
    pub fn window_id(&self, index: u32) -> Result<JsWindowHandle> {
        recover_lock(&self.inner)
            .window_id_at(index as usize)
            .map(JsWindowHandle::from)
            .map_err(napi_runtime_error)
    }

    #[napi(js_name = "windowIds")]
    pub fn window_ids(&self) -> Vec<String> {
        recover_lock(&self.inner)
            .window_ids()
            .into_iter()
            .map(|id| id.get().to_string())
            .collect()
    }

    #[napi(getter)]
    pub fn pending_count(&self) -> u32 {
        recover_lock(&self.inner).pending_ui_task_count() as u32
    }
}

impl JsRunningApp {
    fn new(runtime: BindingRuntime) -> Self {
        Self {
            inner: Mutex::new(runtime),
        }
    }
}

#[napi(js_name = "RendererInteropCapabilities")]
#[derive(Debug, Clone, Copy)]
pub struct JsRendererInteropCapabilities {
    inner: RendererInteropCapabilities,
}

#[napi]
impl JsRendererInteropCapabilities {
    #[napi(constructor)]
    pub fn new(
        backend: String,
        cpu_upload: Option<bool>,
        shared_texture: Option<bool>,
        shared_render_target: Option<bool>,
    ) -> Result<Self> {
        Ok(Self {
            inner: RendererInteropCapabilities {
                backend: native_backend_from_js(&backend)?,
                cpu_upload: cpu_upload.unwrap_or(true),
                shared_texture: shared_texture.unwrap_or(false),
                shared_render_target: shared_render_target.unwrap_or(false),
            },
        })
    }

    #[napi(factory, js_name = "cpuOnly")]
    pub fn cpu_only(backend: String) -> Result<Self> {
        Ok(Self {
            inner: RendererInteropCapabilities::cpu_only(native_backend_from_js(&backend)?),
        })
    }

    #[napi]
    pub fn supports(&self, tier: String) -> Result<bool> {
        Ok(self.inner.supports(interop_tier_from_js(&tier)?))
    }

    #[napi(getter)]
    pub fn backend(&self) -> String {
        native_backend_name(self.inner.backend).to_owned()
    }

    #[napi(getter, js_name = "cpuUpload")]
    pub fn cpu_upload(&self) -> bool {
        self.inner.cpu_upload
    }

    #[napi(getter, js_name = "sharedTexture")]
    pub fn shared_texture(&self) -> bool {
        self.inner.shared_texture
    }

    #[napi(getter, js_name = "sharedRenderTarget")]
    pub fn shared_render_target(&self) -> bool {
        self.inner.shared_render_target
    }
}

#[napi(js_name = "ExternalBackendHandle")]
#[derive(Debug, Clone, Copy)]
pub struct JsExternalBackendHandle {
    inner: ExternalBackendHandle,
}

#[napi]
impl JsExternalBackendHandle {
    #[napi(constructor)]
    pub fn new(id: String) -> Result<Self> {
        Ok(Self {
            inner: ExternalBackendHandle::new(parse_u64_string(&id, "external backend handle id")?),
        })
    }

    #[napi(getter)]
    pub fn id(&self) -> String {
        self.inner.id().to_string()
    }

    #[napi(getter, js_name = "isEmpty")]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[napi(js_name = "ExternalSync")]
#[derive(Debug, Clone, Copy)]
pub struct JsExternalSync {
    inner: ExternalSync,
}

#[napi]
impl JsExternalSync {
    #[napi(factory)]
    pub fn none() -> Self {
        Self {
            inner: ExternalSync::None,
        }
    }

    #[napi(factory)]
    pub fn generation(generation: String) -> Result<Self> {
        Ok(Self {
            inner: ExternalSync::Generation(parse_u64_string(
                &generation,
                "external sync generation",
            )?),
        })
    }

    #[napi(factory, js_name = "timelineValue")]
    pub fn timeline_value(handle: &JsExternalBackendHandle, value: String) -> Result<Self> {
        Ok(Self {
            inner: ExternalSync::TimelineValue {
                handle: handle.inner,
                value: parse_u64_string(&value, "external sync timeline value")?,
            },
        })
    }

    #[napi(factory)]
    pub fn fence(handle: &JsExternalBackendHandle) -> Self {
        Self {
            inner: ExternalSync::Fence {
                handle: handle.inner,
            },
        }
    }

    #[napi(getter)]
    pub fn kind(&self) -> String {
        match self.inner {
            ExternalSync::None => "none",
            ExternalSync::Generation(_) => "generation",
            ExternalSync::TimelineValue { .. } => "timelineValue",
            ExternalSync::Fence { .. } => "fence",
        }
        .to_owned()
    }
}

#[napi(js_name = "ExternalTextureDescriptor")]
#[derive(Debug, Clone)]
pub struct JsExternalTextureDescriptor {
    inner: ExternalTextureDescriptor,
}

#[napi]
impl JsExternalTextureDescriptor {
    #[napi(factory, js_name = "cpuRgba8")]
    pub fn cpu_rgba8(
        size: &JsSize,
        pixels: Uint8Array,
        generation: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::cpu_rgba8(
                (*size).into(),
                pixels.to_vec(),
                generation
                    .as_deref()
                    .map(|value| parse_u64_string(value, "external texture generation"))
                    .transpose()?
                    .unwrap_or(0),
            ),
        })
    }

    #[napi(factory, js_name = "sharedTexture")]
    pub fn shared_texture(
        backend: String,
        size: &JsSize,
        format: String,
        handle: &JsExternalBackendHandle,
        sync: &JsExternalSync,
        color_space: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::SharedTexture {
                backend: native_backend_from_js(&backend)?,
                size: (*size).into(),
                format: external_texture_format_from_js(&format)?,
                color_space: color_space_from_js(color_space)?,
                handle: handle.inner,
                sync: sync.inner,
            },
        })
    }

    #[napi(factory, js_name = "sharedRenderTarget")]
    pub fn shared_render_target(
        backend: String,
        size: &JsSize,
        format: String,
        handle: &JsExternalBackendHandle,
        sync: &JsExternalSync,
        color_space: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::SharedRenderTarget {
                backend: native_backend_from_js(&backend)?,
                size: (*size).into(),
                format: external_texture_format_from_js(&format)?,
                color_space: color_space_from_js(color_space)?,
                handle: handle.inner,
                sync: sync.inner,
            },
        })
    }

    #[napi]
    pub fn validate(&self) -> Result<()> {
        self.inner.validate().map_err(napi_external_texture_error)
    }

    #[napi(getter)]
    pub fn tier(&self) -> String {
        interop_tier_name(self.inner.tier()).to_owned()
    }

    #[napi(getter)]
    pub fn size(&self) -> JsSize {
        self.inner.size().into()
    }
}

struct JsObjectCallbacks {
    env: JsEnvHandle,
    object: Option<ObjectRef<false>>,
}

impl JsObjectCallbacks {
    fn new(env: Env, object: Object<'_>) -> Result<Self> {
        Ok(Self {
            env: JsEnvHandle::from_env(env),
            object: Some(object.create_ref::<false>()?),
        })
    }

    fn clone_ref(&self) -> Result<Self> {
        let env = self.env.to_env();
        let object = self.object_ref()?.get_value(&env)?.create_ref::<false>()?;
        Ok(Self {
            env: self.env,
            object: Some(object),
        })
    }

    fn object<'env>(&self, env: &'env Env) -> Result<Object<'env>> {
        self.object_ref()?.get_value(env)
    }

    fn object_ref(&self) -> Result<&ObjectRef<false>> {
        self.object
            .as_ref()
            .ok_or_else(|| napi_runtime_error("JavaScript widget callbacks were released"))
    }
}

impl Drop for JsObjectCallbacks {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            let env = self.env.to_env();
            let _ = object.unref(&env);
        }
    }
}

struct JsWidgetCallbacks {
    callbacks: Mutex<Option<JsObjectCallbacks>>,
}

impl ForeignWidgetCallbacks for JsWidgetCallbacks {
    fn debug_name(&self, _id: sui_bindings_core::ForeignWidgetId) -> &'static str {
        "sui_js::Widget"
    }

    fn event(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignEventCtx<'_>,
        event: &Event,
    ) -> ForeignCallbackResult<()> {
        let callbacks = recover_lock(&self.callbacks);
        let Some(callbacks) = callbacks.as_ref() else {
            return Err(ForeignCallbackFailure::new(
                "JavaScript widget callbacks were released",
            ));
        };
        let handled = callbacks.call_event(event).map_err(foreign_js_error)?;
        if handled {
            ctx.set_handled();
            ctx.request_paint();
        }
        Ok(())
    }

    fn measure(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        let callbacks = recover_lock(&self.callbacks);
        let Some(callbacks) = callbacks.as_ref() else {
            return Err(ForeignCallbackFailure::new(
                "JavaScript widget callbacks were released",
            ));
        };
        callbacks
            .call_measure(constraints)
            .map_err(foreign_js_error)
    }

    fn paint(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        let callbacks = recover_lock(&self.callbacks);
        let Some(callbacks) = callbacks.as_ref() else {
            return Err(ForeignCallbackFailure::new(
                "JavaScript widget callbacks were released",
            ));
        };
        callbacks
            .call_paint(ctx.bounds())
            .map_err(foreign_js_error)
            .and_then(|(mut commands, images)| {
                resolve_binding_image_slots(&mut commands, |slot| ctx.widget_image_handle(slot));
                for pending in images {
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
        let callbacks = recover_lock(&self.callbacks);
        let Some(callbacks) = callbacks.as_ref() else {
            return Err(ForeignCallbackFailure::new(
                "JavaScript widget callbacks were released",
            ));
        };
        if let Some(commands) = callbacks
            .call_semantics(
                ctx.widget_id(),
                ctx.bounds(),
                ctx.is_focused(),
                ctx.child_count(),
            )
            .map_err(foreign_js_error)?
        {
            for command in commands {
                match command {
                    JsSemanticsCommand::Node(node) => ctx.push(node),
                    JsSemanticsCommand::Child(index) => {
                        ctx.semantics_child(index);
                    }
                }
            }
        } else if let Some(name) = callbacks.name().map_err(foreign_js_error)? {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(name);
            ctx.push(node);
        }
        Ok(())
    }
}

impl JsObjectCallbacks {
    fn call_event(&self, event: &Event) -> Result<bool> {
        let env = self.env.to_env();
        let object = self.object(&env)?;
        if !object.has_named_property("event")? {
            return Ok(false);
        }
        let event_fn: Function<'_, (JsEvent,), bool> = object.get_named_property("event")?;
        event_fn.apply(object, (JsEvent::from_binding(BindingEvent::from(event)),))
    }

    fn call_measure(&self, constraints: Constraints) -> Result<Size> {
        let env = self.env.to_env();
        let object = self.object(&env)?;
        if !object.has_named_property("measure")? {
            return Ok(constraints.clamp(Size::ZERO));
        }
        let measure: Function<'_, (JsConstraints,), ClassInstance<'_, JsSize>> =
            object.get_named_property("measure")?;
        let size = measure.apply(object, (JsConstraints::from(constraints),))?;
        Ok(Size::from(*size))
    }

    fn call_paint(&self, bounds: Rect) -> Result<(Vec<PaintCommand>, Vec<PendingPaintImage>)> {
        let env = self.env.to_env();
        let object = self.object(&env)?;
        if !object.has_named_property("paint")? {
            return Ok((Vec::new(), Vec::new()));
        }
        let paint = JsPaint::new(bounds);
        let paint_for_js = paint.clone();
        let paint_fn: Function<'_, (JsPaint,), Unknown<'_>> = object.get_named_property("paint")?;
        let _ = paint_fn.apply(object, (paint_for_js,))?;
        let commands = paint.finish().map_err(napi_value_error)?;
        Ok((commands, paint.take_images()))
    }

    fn call_semantics(
        &self,
        widget_id: WidgetId,
        bounds: Rect,
        focused: bool,
        child_count: usize,
    ) -> Result<Option<Vec<JsSemanticsCommand>>> {
        let env = self.env.to_env();
        let object = self.object(&env)?;
        if !object.has_named_property("semantics")? {
            return Ok(None);
        }
        let semantics = JsSemantics::new(widget_id, bounds, focused, child_count);
        let semantics_for_js = semantics.clone();
        let semantics_fn: Function<'_, (JsSemantics,), Unknown<'_>> =
            object.get_named_property("semantics")?;
        let _ = semantics_fn.apply(object, (semantics_for_js,))?;
        Ok(Some(semantics.take_commands()))
    }

    fn name(&self) -> Result<Option<String>> {
        let env = self.env.to_env();
        let object = self.object(&env)?;
        object.get("name")
    }
}

#[napi(js_name = "UiTaskQueue")]
#[derive(Clone)]
pub struct JsUiTaskQueue {
    inner: UiTaskQueue,
}

#[napi]
impl JsUiTaskQueue {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: UiTaskQueue::new(),
        }
    }

    #[napi]
    pub fn post(&self, env: Env, callback: Function<'_, (), ()>) -> Result<()> {
        let env = JsEnvHandle::from_env(env);
        let callback = callback.create_ref()?;
        self.inner.post(move || {
            let env = env.to_env();
            if let Ok(callback) = callback.borrow_back(&env) {
                let _ = callback.call(());
            }
        });
        Ok(())
    }

    #[napi]
    pub fn drain(&self) -> u32 {
        self.inner.drain() as u32
    }

    #[napi(getter)]
    pub fn pending_count(&self) -> u32 {
        self.inner.pending_count() as u32
    }
}

impl Default for JsUiTaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl JsUiTaskQueue {
    #[cfg(test)]
    fn post_rust_for_test(&self, task: impl FnOnce() + Send + 'static) {
        self.inner.post(task);
    }
}

#[napi(js_name = "RenderSnapshot")]
#[derive(Debug, Clone)]
pub struct JsRenderSnapshot {
    pub command_count: u32,
    pub semantics_count: u32,
    pub semantics_roles: Vec<String>,
    pub semantics_names: Vec<String>,
    pub semantics_values: Vec<String>,
    pub semantics_descriptions: Vec<String>,
    pub semantics_checked: Vec<String>,
    pub semantics_busy: Vec<bool>,
    pub semantics_editable_multiline: Vec<bool>,
    pub semantics_disabled: Vec<bool>,
    pub semantics_focused: Vec<bool>,
    pub semantics_hidden: Vec<bool>,
    pub semantics_hovered: Vec<bool>,
    pub semantics_selected: Vec<bool>,
    pub semantics_expanded: Vec<String>,
    pub fill_rect_count: u32,
    pub draw_image_count: u32,
    pub registered_font_count: u32,
    pub registered_image_count: u32,
}

impl From<BindingRenderSnapshot> for JsRenderSnapshot {
    fn from(value: BindingRenderSnapshot) -> Self {
        Self {
            command_count: value.command_count as u32,
            semantics_count: value.semantics_count as u32,
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
            fill_rect_count: value.fill_rect_count as u32,
            draw_image_count: value.draw_image_count as u32,
            registered_font_count: value.registered_font_count as u32,
            registered_image_count: value.registered_image_count as u32,
        }
    }
}

#[napi(js_name = "renderWidget")]
pub fn render_widget(widget: &JsWidget, event: Option<&JsEvent>) -> Result<JsRenderSnapshot> {
    if let Ok(binding) = widget.binding_widget() {
        return render_binding_widget(binding, event);
    }
    render_foreign_widget(widget.into_sui_widget()?, event)
}

fn render_binding_widget(
    widget: BindingWidget,
    event: Option<&JsEvent>,
) -> Result<JsRenderSnapshot> {
    let app = BindingApp::new().with_window(BindingWindow::new("JavaScript widget", widget));
    if let Some(event) = event {
        let mut runtime = app.start().map_err(napi_runtime_error)?;
        let window_id = runtime.window_id_at(0).map_err(napi_runtime_error)?;
        let _ = runtime
            .render_window(window_id)
            .map_err(napi_runtime_error)?;
        runtime
            .handle_event(window_id, event.binding_event())
            .map_err(napi_runtime_error)?;
        return runtime
            .render_window(window_id)
            .map(JsRenderSnapshot::from)
            .map_err(napi_runtime_error);
    }
    app.render_window(0)
        .map(JsRenderSnapshot::from)
        .map_err(napi_runtime_error)
}

fn render_foreign_widget(
    widget: ForeignWidget,
    event: Option<&JsEvent>,
) -> Result<JsRenderSnapshot> {
    let mut runtime = RuntimeApplication::new()
        .window(WindowBuilder::new().title("JavaScript widget").root(widget))
        .build()
        .map_err(napi_runtime_error)?;
    let window_id = runtime.window_ids()[0];
    let mut output = runtime.render(window_id).map_err(napi_runtime_error)?;
    if let Some(event) = event {
        runtime
            .handle_event(
                window_id,
                event
                    .binding_event()
                    .into_sui_event()
                    .map_err(napi_runtime_error)?,
            )
            .map_err(napi_runtime_error)?;
        output = runtime.render(window_id).map_err(napi_runtime_error)?;
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
    Ok(JsRenderSnapshot {
        command_count,
        semantics_count: output.semantics.len() as u32,
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
        registered_font_count: output.frame.font_registry.len() as u32,
        registered_image_count: output.frame.image_registry.len() as u32,
    })
}

#[derive(Debug, Clone, Copy)]
struct JsEnvHandle(usize);

impl JsEnvHandle {
    fn from_env(env: Env) -> Self {
        Self(env.raw() as usize)
    }

    fn to_env(self) -> Env {
        Env::from_raw(self.0 as napi::sys::napi_env)
    }
}

fn binding_value_from_js(value: Either3<String, f64, bool>) -> BindingValue {
    match value {
        Either3::A(value) => BindingValue::String(value),
        Either3::B(value) => BindingValue::Number(value),
        Either3::C(value) => BindingValue::Bool(value),
    }
}

fn binding_value_to_js(value: BindingValue) -> Either3<String, f64, bool> {
    match value {
        BindingValue::String(value) => Either3::A(value),
        BindingValue::Number(value) => Either3::B(value),
        BindingValue::Bool(value) => Either3::C(value),
    }
}

fn binding_bool_from_js(value: Either3<ClassInstance<'_, JsState>, bool, f64>) -> BindingBool {
    match value {
        Either3::A(state) => BindingBool::State(state.inner.clone()),
        Either3::B(value) => BindingBool::Static(value),
        Either3::C(value) => BindingBool::Static(value != 0.0),
    }
}

fn binding_number_from_js(value: Either3<ClassInstance<'_, JsState>, f64, bool>) -> BindingNumber {
    match value {
        Either3::A(state) => BindingNumber::State(state.inner.clone()),
        Either3::B(value) => BindingNumber::Static(value),
        Either3::C(value) => BindingNumber::Static(if value { 1.0 } else { 0.0 }),
    }
}

fn js_image_fit(value: &str) -> Result<BindingImageFit> {
    match value {
        "fill" => Ok(BindingImageFit::Fill),
        "contain" => Ok(BindingImageFit::Contain),
        "cover" => Ok(BindingImageFit::Cover),
        "none" => Ok(BindingImageFit::None),
        _ => Err(napi_invalid_arg(
            "image fit must be 'fill', 'contain', 'cover', or 'none'",
        )),
    }
}

fn icon_glyph_from_js(value: &str) -> Result<sui_crate::IconGlyph> {
    binding_icon_glyph_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown icon glyph '{value}'")))
}

fn semantic_tone_from_js(value: &str) -> Result<sui_crate::SemanticTone> {
    binding_semantic_tone_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown semantic tone '{value}'")))
}

fn table_column_alignment_from_js(value: &str) -> Result<sui_crate::TableColumnAlignment> {
    binding_table_column_alignment_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown table column alignment '{value}'")))
}

fn surface_role_from_js(value: &str) -> Result<sui_crate::SurfaceRole> {
    binding_surface_role_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown surface role '{value}'")))
}

fn surface_border_from_js(value: &str) -> Result<sui_crate::SurfaceBorder> {
    binding_surface_border_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown surface border '{value}'")))
}

fn surface_elevation_from_js(value: &str) -> Result<sui_crate::SurfaceElevation> {
    binding_surface_elevation_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown surface elevation '{value}'")))
}

fn alignment_from_js(value: &str) -> Result<sui_crate::Alignment> {
    binding_alignment_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown alignment '{value}'")))
}

fn tooltip_placement_from_js(value: &str) -> Result<sui_crate::TooltipPlacement> {
    binding_tooltip_placement_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown tooltip placement '{value}'")))
}

fn semantics_role_from_js(value: &str) -> Result<sui_crate::SemanticsRole> {
    binding_semantics_role_from_name(value)
        .ok_or_else(|| napi_invalid_arg(format!("unknown semantics role '{value}'")))
}

fn js_axis(value: &str) -> Result<Axis> {
    match value {
        "horizontal" | "x" | "row" => Ok(Axis::Horizontal),
        "vertical" | "y" | "column" => Ok(Axis::Vertical),
        _ => Err(napi_invalid_arg("axis must be 'horizontal' or 'vertical'")),
    }
}

fn scroll_axes_from_js(value: &str) -> Result<BindingScrollAxes> {
    match value {
        "vertical" | "y" | "column" => Ok(BindingScrollAxes::Vertical),
        "horizontal" | "x" | "row" => Ok(BindingScrollAxes::Horizontal),
        "both" | "xy" | "all" => Ok(BindingScrollAxes::Both),
        _ => Err(napi_invalid_arg(
            "scroll axes must be 'vertical', 'horizontal', or 'both'",
        )),
    }
}

fn pointer_event_kind_from_js(value: &str) -> Result<BindingPointerEventKind> {
    match value {
        "down" => Ok(BindingPointerEventKind::Down),
        "up" => Ok(BindingPointerEventKind::Up),
        "move" => Ok(BindingPointerEventKind::Move),
        "scroll" => Ok(BindingPointerEventKind::Scroll),
        "enter" => Ok(BindingPointerEventKind::Enter),
        "leave" => Ok(BindingPointerEventKind::Leave),
        "cancel" => Ok(BindingPointerEventKind::Cancel),
        _ => Err(napi_invalid_arg(
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

fn pointer_button_from_js(value: &str) -> Result<BindingPointerButton> {
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
                    .map_err(|_| napi_invalid_arg("other pointer button must fit in u16"));
            }
            Err(napi_invalid_arg(
                "pointer button must be 'primary', 'secondary', 'middle', 'back', 'forward', or 'other:<u16>'",
            ))
        }
    }
}

fn pointer_button_name(value: BindingPointerButton) -> String {
    match value {
        BindingPointerButton::Primary => "primary".to_owned(),
        BindingPointerButton::Secondary => "secondary".to_owned(),
        BindingPointerButton::Middle => "middle".to_owned(),
        BindingPointerButton::Back => "back".to_owned(),
        BindingPointerButton::Forward => "forward".to_owned(),
        BindingPointerButton::Other(button) => format!("other:{button}"),
    }
}

fn pointer_kind_from_js(value: &str) -> Result<BindingPointerKind> {
    match value {
        "mouse" => Ok(BindingPointerKind::Mouse),
        "touch" => Ok(BindingPointerKind::Touch),
        "pen" => Ok(BindingPointerKind::Pen),
        "unknown" => Ok(BindingPointerKind::Unknown),
        _ => Err(napi_invalid_arg(
            "pointerKind must be 'mouse', 'touch', 'pen', or 'unknown'",
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

fn checked_buttons(value: u32) -> Result<u8> {
    u8::try_from(value).map_err(|_| napi_invalid_arg("buttons must fit in an unsigned byte"))
}

fn key_state_from_js(value: &str) -> Result<BindingKeyState> {
    match value {
        "pressed" | "down" => Ok(BindingKeyState::Pressed),
        "released" | "up" => Ok(BindingKeyState::Released),
        _ => Err(napi_invalid_arg(
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
        BindingImeEvent::CompositionStart => "compositionStart",
        BindingImeEvent::CompositionUpdate { .. } => "compositionUpdate",
        BindingImeEvent::CompositionCommit { .. } => "compositionCommit",
        BindingImeEvent::CompositionEnd => "compositionEnd",
    }
}

fn window_event_kind_name(value: &BindingWindowEvent) -> &'static str {
    match value {
        BindingWindowEvent::CloseRequested => "closeRequested",
        BindingWindowEvent::Resized(_) => "resized",
        BindingWindowEvent::ScaleFactorChanged { .. } => "scaleFactorChanged",
        BindingWindowEvent::Focused(_) => "focused",
        BindingWindowEvent::Occluded(_) => "occluded",
        BindingWindowEvent::ExternalFileHovered(_) => "externalFileHovered",
        BindingWindowEvent::ExternalFileHoverCancelled => "externalFileHoverCancelled",
        BindingWindowEvent::ExternalFileDropped(_) => "externalFileDropped",
        BindingWindowEvent::RedrawRequested => "redrawRequested",
    }
}

fn color_space_from_js(value: Option<String>) -> Result<ColorSpace> {
    match value.as_deref().unwrap_or("srgb") {
        "srgb" | "sRGB" => Ok(ColorSpace::Srgb),
        "linear-srgb" | "linear_srgb" => Ok(ColorSpace::LinearSrgb),
        "display-p3" | "display_p3" => Ok(ColorSpace::DisplayP3),
        "linear-display-p3" | "linear_display_p3" => Ok(ColorSpace::LinearDisplayP3),
        _ => Err(napi_invalid_arg(
            "colorSpace must be 'srgb', 'linear-srgb', 'display-p3', or 'linear-display-p3'",
        )),
    }
}

fn js_radii(radius: Option<f64>) -> [f32; 4] {
    [radius.unwrap_or(0.0) as f32; 4]
}

fn js_four_points(points: &Array<'_>) -> Result<[sui_crate::Point; 4]> {
    if points.len() != 4 {
        return Err(napi_invalid_arg(
            "points must contain exactly four Point values",
        ));
    }

    let mut extracted = Vec::with_capacity(4);
    for index in 0_u32..4 {
        let point = points
            .get::<ClassInstance<'_, JsPoint>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("point index {index} is out of range")))?;
        extracted.push(JsPoint::new(point.x, point.y).into());
    }

    Ok(<[sui_crate::Point; 4]>::try_from(extracted)
        .expect("length checked before converting points"))
}

fn js_semantics_value(
    value: Option<Either3<String, f64, bool>>,
    min_value: Option<f64>,
    max_value: Option<f64>,
) -> Option<SemanticsValue> {
    match value {
        Some(Either3::A(value)) => Some(SemanticsValue::Text(value)),
        Some(Either3::B(value)) => {
            if let (Some(min), Some(max)) = (min_value, max_value) {
                Some(SemanticsValue::Range { value, min, max })
            } else {
                Some(SemanticsValue::Number(value))
            }
        }
        Some(Either3::C(value)) => Some(SemanticsValue::Text(value.to_string())),
        None => None,
    }
}

fn js_toggle_state(value: Option<&str>) -> Result<Option<ToggleState>> {
    let Some(value) = value else {
        return Ok(None);
    };
    binding_toggle_state_from_name(value)
        .map(Some)
        .ok_or_else(|| napi_invalid_arg("checked must be 'checked', 'unchecked', or 'mixed'"))
}

fn js_text_style(
    color: Option<&JsColor>,
    font_size: Option<f64>,
    line_height: Option<f64>,
    font: Option<&JsFontHandle>,
    weight: Option<u32>,
    style: Option<&str>,
    stretch: Option<&str>,
) -> Result<TextStyle> {
    let mut text_style = TextStyle::new(color.map(|color| (*color).into()).unwrap_or(Color::WHITE));
    if let Some(font_size) = font_size {
        text_style.font_size = font_size as f32;
    }
    if let Some(line_height) = line_height {
        text_style.line_height = line_height as f32;
    }
    if let Some(font) = font {
        text_style.font = Some(font.inner.into_sui());
    }
    if let Some(weight) = weight {
        text_style.weight = FontWeight::new(
            u16::try_from(weight).map_err(|_| napi_invalid_arg("font weight must fit in u16"))?,
        );
    }
    if let Some(style) = style {
        text_style.style = font_style_from_js(style)?;
    }
    if let Some(stretch) = stretch {
        text_style.stretch = font_stretch_from_js(stretch)?;
    }
    Ok(text_style)
}

fn font_style_from_js(value: &str) -> Result<FontStyle> {
    match value {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(napi_invalid_arg(
            "font style must be 'normal', 'italic', or 'oblique'",
        )),
    }
}

fn font_stretch_from_js(value: &str) -> Result<FontStretch> {
    match value {
        "ultraCondensed" | "ultra_condensed" | "ultra-condensed" => Ok(FontStretch::UltraCondensed),
        "extraCondensed" | "extra_condensed" | "extra-condensed" => Ok(FontStretch::ExtraCondensed),
        "condensed" => Ok(FontStretch::Condensed),
        "semiCondensed" | "semi_condensed" | "semi-condensed" => Ok(FontStretch::SemiCondensed),
        "normal" => Ok(FontStretch::Normal),
        "semiExpanded" | "semi_expanded" | "semi-expanded" => Ok(FontStretch::SemiExpanded),
        "expanded" => Ok(FontStretch::Expanded),
        "extraExpanded" | "extra_expanded" | "extra-expanded" => Ok(FontStretch::ExtraExpanded),
        "ultraExpanded" | "ultra_expanded" | "ultra-expanded" => Ok(FontStretch::UltraExpanded),
        _ => Err(napi_invalid_arg("invalid font stretch")),
    }
}

fn native_backend_from_js(value: &str) -> Result<NativeGraphicsBackend> {
    match value {
        "cpu" => Ok(NativeGraphicsBackend::Cpu),
        "wgpu" => Ok(NativeGraphicsBackend::Wgpu),
        "webgpu" | "web-gpu" | "web_gpu" => Ok(NativeGraphicsBackend::WebGpu),
        "d3d12" => Ok(NativeGraphicsBackend::D3d12),
        "metal" => Ok(NativeGraphicsBackend::Metal),
        "vulkan" => Ok(NativeGraphicsBackend::Vulkan),
        "opengl" | "open-gl" | "open_gl" => Ok(NativeGraphicsBackend::OpenGl),
        "unknown" => Ok(NativeGraphicsBackend::Unknown),
        _ => Err(napi_invalid_arg(
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

fn interop_tier_from_js(value: &str) -> Result<RendererInteropTier> {
    match value {
        "cpuUpload" | "cpu_upload" | "cpu-upload" => Ok(RendererInteropTier::CpuUpload),
        "sharedTexture" | "shared_texture" | "shared-texture" => {
            Ok(RendererInteropTier::SharedTexture)
        }
        "sharedRenderTarget" | "shared_render_target" | "shared-render-target" => {
            Ok(RendererInteropTier::SharedRenderTarget)
        }
        _ => Err(napi_invalid_arg(
            "tier must be 'cpuUpload', 'sharedTexture', or 'sharedRenderTarget'",
        )),
    }
}

fn interop_tier_name(value: RendererInteropTier) -> &'static str {
    match value {
        RendererInteropTier::CpuUpload => "cpuUpload",
        RendererInteropTier::SharedTexture => "sharedTexture",
        RendererInteropTier::SharedRenderTarget => "sharedRenderTarget",
    }
}

fn external_texture_format_from_js(value: &str) -> Result<ExternalTextureFormat> {
    match value {
        "rgba8unorm" | "rgba8Unorm" | "rgba8_unorm" | "rgba8" => {
            Ok(ExternalTextureFormat::Rgba8Unorm)
        }
        "bgra8unorm" | "bgra8Unorm" | "bgra8_unorm" | "bgra8" => {
            Ok(ExternalTextureFormat::Bgra8Unorm)
        }
        "rgba16float" | "rgba16Float" | "rgba16_float" | "rgba16f" => {
            Ok(ExternalTextureFormat::Rgba16Float)
        }
        _ => Err(napi_invalid_arg(
            "format must be 'rgba8unorm', 'bgra8unorm', or 'rgba16float'",
        )),
    }
}

fn parse_u64_string(value: &str, label: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|_| napi_invalid_arg(format!("{label} must be an unsigned 64-bit integer string")))
}

fn binding_text_from_js(
    value: Either4<ClassInstance<'_, JsState>, String, f64, bool>,
) -> BindingText {
    match value {
        Either4::A(state) => BindingText::State(state.inner.clone()),
        Either4::B(value) => BindingText::Static(value),
        Either4::C(value) => BindingText::Static(BindingValue::Number(value).as_label_text()),
        Either4::D(value) => BindingText::Static(BindingValue::Bool(value).as_label_text()),
    }
}

fn extract_binding_widgets(children: &Array<'_>) -> Result<Vec<BindingWidget>> {
    let mut widgets = Vec::with_capacity(children.len() as usize);
    for index in 0..children.len() {
        let child = children
            .get::<ClassInstance<'_, JsWidget>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("child index {index} is out of range")))?;
        widgets.push(child.binding_widget()?);
    }
    Ok(widgets)
}

fn extract_text_spans(spans: &Array<'_>) -> Result<Vec<BindingTextSpan>> {
    let mut out = Vec::with_capacity(spans.len() as usize);
    for index in 0..spans.len() {
        let span = spans
            .get::<ClassInstance<'_, JsTextSpan>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("span index {index} is out of range")))?;
        out.push(span.inner.clone());
    }
    Ok(out)
}

fn extract_status_bar_segments(segments: &Array<'_>) -> Result<Vec<BindingStatusBarSegment>> {
    let mut out = Vec::with_capacity(segments.len() as usize);
    for index in 0..segments.len() {
        let segment = segments
            .get::<ClassInstance<'_, JsStatusBarSegment>>(index)?
            .ok_or_else(|| {
                napi_invalid_arg(format!("status bar segment {index} is out of range"))
            })?;
        out.push(segment.inner.clone());
    }
    Ok(out)
}

fn extract_segmented_control_items(items: &Array<'_>) -> Result<Vec<BindingSegmentedControlItem>> {
    let mut out = Vec::with_capacity(items.len() as usize);
    for index in 0..items.len() {
        let item = items
            .get::<ClassInstance<'_, JsSegmentedControlItem>>(index)?
            .ok_or_else(|| {
                napi_invalid_arg(format!("segmented control item {index} is out of range"))
            })?;
        out.push(item.inner.clone());
    }
    Ok(out)
}

fn extract_table_columns(columns: &Array<'_>) -> Result<Vec<BindingTableColumn>> {
    let mut out = Vec::with_capacity(columns.len() as usize);
    for index in 0..columns.len() {
        let column = columns
            .get::<ClassInstance<'_, JsTableColumn>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("table column {index} is out of range")))?;
        out.push(column.inner.clone());
    }
    Ok(out)
}

fn extract_table_rows(rows: &Array<'_>) -> Result<Vec<BindingTableRow>> {
    let mut out = Vec::with_capacity(rows.len() as usize);
    for index in 0..rows.len() {
        let row = rows
            .get::<ClassInstance<'_, JsTableRow>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("table row {index} is out of range")))?;
        out.push(row.inner.clone());
    }
    Ok(out)
}

fn extract_tree_items(items: &Array<'_>) -> Result<Vec<BindingTreeItem>> {
    let mut out = Vec::with_capacity(items.len() as usize);
    for index in 0..items.len() {
        let item = items
            .get::<ClassInstance<'_, JsTreeItem>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("tree item {index} is out of range")))?;
        out.push(item.inner.clone());
    }
    Ok(out)
}

fn extract_layer_list_items(items: &Array<'_>) -> Result<Vec<BindingLayerListItem>> {
    let mut out = Vec::with_capacity(items.len() as usize);
    for index in 0..items.len() {
        let item = items
            .get::<ClassInstance<'_, JsLayerListItem>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("layer list item {index} is out of range")))?;
        out.push(item.inner.clone());
    }
    Ok(out)
}

fn extract_menu_items(items: &Array<'_>) -> Result<Vec<BindingMenuItem>> {
    let mut out = Vec::with_capacity(items.len() as usize);
    for index in 0..items.len() {
        let item = items
            .get::<ClassInstance<'_, JsMenuItem>>(index)?
            .ok_or_else(|| napi_invalid_arg(format!("menu item {index} is out of range")))?;
        out.push(item.inner.clone());
    }
    Ok(out)
}

fn extract_tool_palette_items(items: &Array<'_>) -> Result<Vec<BindingToolPaletteItem>> {
    let mut out = Vec::with_capacity(items.len() as usize);
    for index in 0..items.len() {
        let item = items
            .get::<ClassInstance<'_, JsToolPaletteItem>>(index)?
            .ok_or_else(|| {
                napi_invalid_arg(format!("tool palette item {index} is out of range"))
            })?;
        out.push(item.inner.clone());
    }
    Ok(out)
}

fn extract_color_palette_swatches(swatches: &Array<'_>) -> Result<Vec<BindingColorPaletteSwatch>> {
    let mut out = Vec::with_capacity(swatches.len() as usize);
    for index in 0..swatches.len() {
        let swatch = swatches
            .get::<ClassInstance<'_, JsColorPaletteSwatch>>(index)?
            .ok_or_else(|| {
                napi_invalid_arg(format!("color palette swatch {index} is out of range"))
            })?;
        out.push(swatch.inner.clone());
    }
    Ok(out)
}

fn napi_value_error(error: PaintValidationError) -> Error {
    Error::new(Status::InvalidArg, error.to_string())
}

fn napi_external_texture_error(error: ExternalTextureValidationError) -> Error {
    Error::new(Status::InvalidArg, error.to_string())
}

fn napi_invalid_arg(error: impl ToString) -> Error {
    Error::new(Status::InvalidArg, error.to_string())
}

fn napi_runtime_error(error: impl ToString) -> Error {
    Error::new(Status::GenericFailure, error.to_string())
}

fn foreign_js_error(error: Error) -> ForeignCallbackFailure {
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
    use std::{
        fs,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn unique_temp_path(label: &str, extension: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sui-js-{label}-{}-{nanos}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn paint_surface_records_valid_commands() {
        let paint = JsPaint::new(Rect::new(0.0, 0.0, 100.0, 20.0));
        paint
            .fill_rect(
                &JsRect::new(0.0, 0.0, 50.0, 20.0),
                &JsColor::new(0.25, 0.68, 0.46, Some(1.0)),
            )
            .unwrap();
        paint
            .stroke_rect(
                &JsRect::new(0.0, 0.0, 100.0, 20.0),
                &JsColor::new(1.0, 1.0, 1.0, Some(1.0)),
                Some(2.0),
            )
            .unwrap();

        assert_eq!(paint.command_count(), 2);
        assert_eq!(paint.finish().unwrap().len(), 2);
    }

    #[test]
    fn paint_surface_records_styled_text_command() {
        let paint = JsPaint::new(Rect::new(0.0, 0.0, 160.0, 32.0));
        let font = JsFontHandle {
            inner: BindingFontHandle::new(7),
        };

        paint
            .draw_text(
                &JsRect::new(0.0, 0.0, 160.0, 32.0),
                "Styled".to_owned(),
                Some(&JsColor::new(0.9, 0.95, 1.0, Some(1.0))),
                Some(18.0),
                Some(22.0),
                Some(&font),
                Some(700),
                Some("italic".to_owned()),
                Some("condensed".to_owned()),
            )
            .unwrap();

        assert_eq!(paint.command_count(), 1);
        assert!(matches!(
            paint.finish().unwrap().as_slice(),
            [PaintCommand::DrawText { .. }]
        ));
    }

    #[test]
    fn text_span_builds_rich_text_style() {
        let span = JsTextSpan::new(
            "Warm".to_owned(),
            Some(&JsColor::new(0.9, 0.35, 0.2, Some(1.0))),
            Some(18.0),
            Some(22.0),
            None,
            Some(700),
            Some("italic".to_owned()),
            Some("normal".to_owned()),
        )
        .unwrap();

        assert_eq!(span.text(), "Warm");
        assert_eq!(span.inner.style.font_size, 18.0);
        assert_eq!(span.inner.style.line_height, 22.0);
        assert_eq!(span.inner.style.weight, FontWeight::new(700));
        assert_eq!(span.inner.style.style, FontStyle::Italic);
    }

    #[test]
    fn paint_surface_records_rich_low_level_commands() {
        let paint = JsPaint::new(Rect::new(0.0, 0.0, 96.0, 64.0));
        let path = JsPath::circle(&JsPoint::new(24.0, 24.0), 14.0);
        let mut builder = JsPathBuilder::new();
        builder.move_to(&JsPoint::new(8.0, 48.0));
        builder.line_to(&JsPoint::new(36.0, 48.0));
        builder.quad_to(&JsPoint::new(48.0, 56.0), &JsPoint::new(60.0, 48.0));
        builder.close();
        let custom_path = builder.build();
        let shadow = JsShadow::new(2.0, 3.0, 4.0, 1.0, &JsColor::new(0.0, 0.0, 0.0, Some(0.35)));

        paint
            .push_clip_path(&JsPath::rect(&paint.bounds()))
            .unwrap();
        paint
            .push_transform(&JsTransform::translation(4.0, 2.0))
            .unwrap();
        paint
            .fill_path(&path, &JsColor::new(0.2, 0.5, 0.9, Some(1.0)))
            .unwrap();
        paint
            .stroke_path(
                &custom_path,
                &JsColor::new(1.0, 1.0, 1.0, Some(1.0)),
                Some(1.5),
            )
            .unwrap();
        paint
            .draw_shadow(&JsRect::new(8.0, 8.0, 48.0, 28.0), &shadow, Some(6.0))
            .unwrap();
        paint
            .fill_rounded_rect_with_shadow(
                &JsRect::new(12.0, 12.0, 40.0, 20.0),
                &JsColor::new(0.9, 0.4, 0.2, Some(1.0)),
                &shadow,
                Some(5.0),
            )
            .unwrap();
        paint.pop_transform().unwrap();
        paint.pop_clip().unwrap();

        assert_eq!(paint.command_count(), 8);
        assert!(matches!(
            paint.finish().unwrap().as_slice(),
            [
                PaintCommand::PushClipPath(_),
                PaintCommand::PushTransform(_),
                PaintCommand::FillPath { .. },
                PaintCommand::StrokePath { .. },
                PaintCommand::FillRoundedRect {
                    shadow: Some(_),
                    ..
                },
                PaintCommand::FillRoundedRect {
                    shadow: Some(_),
                    ..
                },
                PaintCommand::PopTransform,
                PaintCommand::PopClip,
            ]
        ));
    }

    #[test]
    fn semantics_surface_records_accessibility_nodes() {
        let semantics =
            JsSemantics::new(WidgetId::new(9), Rect::new(0.0, 0.0, 160.0, 28.0), false, 0);

        semantics
            .node(
                Some("progressBar".to_owned()),
                Some("CPU meter".to_owned()),
                Some(Either3::B(0.62)),
                Some("Processor utilization".to_owned()),
                None,
                None,
                None,
                None,
                None,
                Some(true),
                Some(0.0),
                Some(1.0),
            )
            .unwrap();
        let commands = semantics.take_commands();

        assert!(matches!(
            commands.as_slice(),
            [JsSemanticsCommand::Node(node)]
                if node.role == SemanticsRole::ProgressBar
                    && node.name.as_deref() == Some("CPU meter")
                    && node.value == Some(SemanticsValue::Range {
                        value: 0.62,
                        min: 0.0,
                        max: 1.0,
                    })
                    && node.state.busy
        ));
    }

    #[test]
    fn render_snapshot_counts_foreign_widget_output() {
        let snapshot = render_foreign_widget(ForeignWidget::new(MockCallbacks), None).unwrap();

        assert!(snapshot.command_count >= 2);
        assert_eq!(snapshot.fill_rect_count, 2);
        assert_eq!(snapshot.semantics_count, 1);
        assert!(snapshot.semantics_disabled.iter().any(|value| *value));
        assert!(snapshot.semantics_selected.iter().any(|value| *value));
        assert!(
            snapshot
                .semantics_expanded
                .iter()
                .any(|value| value == "expanded")
        );
    }

    #[test]
    fn binding_tree_renders_foreign_widget_output() {
        let root = BindingWidget::column(
            [
                BindingWidget::foreign(MockCallbacks),
                BindingWidget::label("Tail"),
            ],
            4.0,
        );
        let snapshot = render_binding_widget(root, None).unwrap();

        assert!(snapshot.command_count > 0);
        assert!(snapshot.fill_rect_count >= 1);
        assert!(snapshot.semantics_count >= 2);
    }

    #[test]
    fn binding_tree_renders_form_controls_and_updates_checkbox_state() {
        let checked = JsState::new(Either3::C(false));
        let slider_value = JsState::new(Either3::B(0.25));
        let selected = JsState::new(Either3::C(false));
        let enabled = JsState::new(Either3::C(true));
        let root = BindingWidget::column(
            [
                BindingWidget::checkbox("Enabled", checked.inner.clone(), None),
                BindingWidget::switch("Airplane mode", false, None),
                BindingWidget::slider("Opacity", slider_value.inner.clone(), 0.0, 1.0, 0.05, None),
                BindingWidget::icon(
                    sui_crate::IconGlyph::Search,
                    Some("Search icon".to_owned()),
                    None,
                    None,
                ),
                BindingWidget::icon_button(
                    sui_crate::IconGlyph::Download,
                    "Download",
                    selected.inner.clone(),
                    enabled.inner.clone(),
                    None,
                    None,
                    Some("Download file".to_owned()),
                    None,
                ),
            ],
            8.0,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Controls", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();
        let snapshot = runtime.render_window(window_id).unwrap();

        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 5);
        assert!(snapshot.semantics_roles.iter().any(|role| role == "image"));
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Search icon")
        );
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Download")
        );
        assert_eq!(checked.text(), "false");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(32.0, 18.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();

        let up = JsEvent::pointer(
            "up".to_owned(),
            &JsPoint::new(32.0, 18.0),
            None,
            None,
            Some("primary".to_owned()),
            None,
            None,
            None,
        )
        .unwrap();
        runtime.handle_event(window_id, up.binding_event()).unwrap();
        let snapshot = runtime.render_window(window_id).unwrap();

        assert!(snapshot.command_count > 0);
        assert_eq!(checked.text(), "true");

        selected.set(Either3::C(true));
        enabled.set(Either3::C(false));
        assert_eq!(runtime.pending_ui_task_count(), 2);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 2);
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.semantics_selected.iter().any(|value| *value));
        assert!(snapshot.semantics_disabled.iter().any(|value| *value));
    }

    #[test]
    fn binding_tree_radio_button_updates_state_from_pointer() {
        let selected = JsState::new(Either3::C(false));
        let root = BindingWidget::radio_button("Manual", selected.inner.clone(), None);
        let app = BindingApp::new().with_window(BindingWindow::new("Radio", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "radio_button")
        );
        assert_eq!(selected.text(), "false");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(32.0, 18.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();

        let up = JsEvent::pointer(
            "up".to_owned(),
            &JsPoint::new(32.0, 18.0),
            None,
            None,
            Some("primary".to_owned()),
            None,
            None,
            None,
        )
        .unwrap();
        runtime.handle_event(window_id, up.binding_event()).unwrap();

        assert_eq!(selected.text(), "true");
    }

    #[test]
    fn binding_tree_radio_group_updates_state_from_pointer() {
        let selected = JsState::new(Either3::B(0.0));
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let root = BindingWidget::radio_group(
            "Priority",
            ["Low", "Medium", "High"],
            Some(BindingNumber::State(selected.inner.clone())),
            Some(action),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Radio group", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "radio_group")
        );
        assert!(snapshot.semantics_values.iter().any(|value| value == "Low"));
        assert_eq!(selected.text(), "0");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(20.0, 52.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();

        let up = JsEvent::pointer(
            "up".to_owned(),
            &JsPoint::new(20.0, 52.0),
            None,
            None,
            Some("primary".to_owned()),
            None,
            None,
            None,
        )
        .unwrap();
        runtime.handle_event(window_id, up.binding_event()).unwrap();

        assert_eq!(selected.text(), "1");
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Medium".to_owned())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Medium")
        );
    }

    #[test]
    fn binding_tree_list_view_updates_state_from_pointer() {
        let selected = JsState::new(Either3::B(0.0));
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let root = BindingWidget::list_view(
            "Assets",
            ["Brush", "Canvas", "Export"],
            Some(BindingNumber::State(selected.inner.clone())),
            Some(action),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("List view", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.semantics_roles.iter().any(|role| role == "list"));
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "list_item")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Brush")
        );
        assert_eq!(selected.text(), "0");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(44.0, 44.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();

        let up = JsEvent::pointer(
            "up".to_owned(),
            &JsPoint::new(44.0, 44.0),
            None,
            None,
            Some("primary".to_owned()),
            None,
            None,
            None,
        )
        .unwrap();
        runtime.handle_event(window_id, up.binding_event()).unwrap();

        assert_eq!(selected.text(), "1");
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Canvas".to_owned())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Canvas")
        );
        assert!(
            snapshot.semantics_selected.iter().any(|selected| *selected),
            "missing selected list item state in {:?}",
            snapshot.semantics_selected
        );
    }

    #[test]
    fn binding_tree_signal_meter_reads_bound_state() {
        let active = JsState::new(Either3::C(true));
        let root = BindingWidget::signal_meter(
            "Input signal",
            active.inner.clone(),
            Some("Live audio input".to_owned()),
            8,
            Some(Size::new(76.0, 16.0)),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Signal", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "generic_container")
        );
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Input signal")
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|description| description == "Live audio input")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "active")
        );

        active.set(Either3::C(false));
        assert_eq!(runtime.pending_ui_task_count(), 1);
        assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "idle")
        );
    }

    #[test]
    fn binding_tree_select_updates_state_from_keyboard() {
        let selected = JsState::new(Either3::B(0.0));
        let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
        let action = BindingSelectAction::new({
            let changes = Arc::clone(&changes);
            move |index, value| {
                changes.lock().unwrap().push((index, value));
                Ok(())
            }
        });
        let root = BindingWidget::select(
            "Mode",
            ["Draft", "Final", "Review"],
            Some(BindingNumber::State(selected.inner.clone())),
            Some("Choose mode".to_owned()),
            Some(action),
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Select", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_roles
                .iter()
                .any(|role| role == "combo_box")
        );
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Draft")
        );
        assert_eq!(selected.text(), "0");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(20.0, 20.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();

        let up = JsEvent::pointer(
            "up".to_owned(),
            &JsPoint::new(20.0, 20.0),
            None,
            None,
            Some("primary".to_owned()),
            None,
            None,
            None,
        )
        .unwrap();
        runtime.handle_event(window_id, up.binding_event()).unwrap();

        let arrow_down =
            JsEvent::keyboard("ArrowDown".to_owned(), None, None, None, None, None).unwrap();
        runtime
            .handle_event(window_id, arrow_down.binding_event())
            .unwrap();
        let enter = JsEvent::keyboard("Enter".to_owned(), None, None, None, None, None).unwrap();
        runtime
            .handle_event(window_id, enter.binding_event())
            .unwrap();

        assert_eq!(selected.text(), "1");
        assert_eq!(
            changes.lock().unwrap().as_slice(),
            &[(1, "Final".to_owned())]
        );
        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(
            snapshot
                .semantics_values
                .iter()
                .any(|value| value == "Final")
        );
    }

    #[test]
    fn binding_tree_text_input_updates_state_from_keyboard() {
        let text = JsState::new(Either3::A(String::new()));
        let root = BindingWidget::text_input(
            "Name",
            text.inner.clone(),
            Some("Type here".to_owned()),
            None,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Text", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert_eq!(text.text(), "");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(32.0, 18.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();
        let key = JsEvent::keyboard("a".to_owned(), None, None, None, None, None).unwrap();
        runtime
            .handle_event(window_id, key.binding_event())
            .unwrap();

        assert_eq!(text.text(), "a");
    }

    #[test]
    fn binding_tree_text_area_updates_state_from_keyboard() {
        let text = JsState::new(Either3::A(String::new()));
        let root = BindingWidget::text_area(
            "Notes",
            text.inner.clone(),
            Some("Type notes".to_owned()),
            None,
        );
        let app = BindingApp::new().with_window(BindingWindow::new("Text", root));
        let mut runtime = app.start().unwrap();
        let window_id = runtime.window_id_at(0).unwrap();

        let snapshot = runtime.render_window(window_id).unwrap();
        assert!(snapshot.command_count > 0);
        assert!(
            snapshot
                .semantics_editable_multiline
                .iter()
                .any(|value| *value)
        );
        assert_eq!(text.text(), "");

        let down = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(32.0, 18.0),
            None,
            None,
            Some("primary".to_owned()),
            Some(1),
            None,
            None,
        )
        .unwrap();
        runtime
            .handle_event(window_id, down.binding_event())
            .unwrap();
        let key = JsEvent::keyboard("a".to_owned(), None, None, None, None, None).unwrap();
        runtime
            .handle_event(window_id, key.binding_event())
            .unwrap();

        assert_eq!(text.text(), "a");
    }

    #[test]
    fn ui_task_queue_drains_posted_tasks() {
        let queue = JsUiTaskQueue::new();
        let count = Arc::new(AtomicUsize::new(0));
        let count_for_task = Arc::clone(&count);
        queue.post_rust_for_test(move || {
            count_for_task.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(queue.pending_count(), 1);
        assert_eq!(queue.drain(), 1);
        assert_eq!(queue.pending_count(), 0);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn paint_surface_records_shader_command() {
        let paint = JsPaint::new(Rect::new(0.0, 0.0, 100.0, 20.0));
        let shader = JsShader::hue_bar();

        paint
            .draw_shader_rect(&JsRect::new(0.0, 0.0, 100.0, 20.0), &shader)
            .unwrap();

        assert_eq!(paint.command_count(), 1);
        assert_eq!(paint.finish().unwrap().len(), 1);
        assert!(
            JsShader::rgb_channel_bar(&JsColor::new(1.0, 0.0, 0.0, Some(1.0)), 4, None).is_err()
        );
    }

    #[test]
    fn paint_surface_records_image_command() {
        let paint = JsPaint::new(Rect::new(0.0, 0.0, 100.0, 20.0));
        let image = paint
            .rgba_image(0, 2, 1, Buffer::from(vec![255, 0, 0, 255, 0, 0, 255, 255]))
            .unwrap();

        assert_eq!(image.local_slot(), Some(0));
        paint
            .draw_image(&JsRect::new(0.0, 0.0, 100.0, 20.0), &image)
            .unwrap();

        assert_eq!(paint.command_count(), 1);
        assert_eq!(paint.take_images().len(), 1);
        assert!(matches!(
            paint.finish().unwrap().as_slice(),
            [PaintCommand::DrawImage { .. }]
        ));
    }

    #[test]
    fn interop_descriptors_validate_external_texture_inputs() {
        let caps =
            JsRendererInteropCapabilities::new("wgpu".to_owned(), None, Some(true), None).unwrap();
        assert_eq!(caps.backend(), "wgpu");
        assert!(caps.supports("cpuUpload".to_owned()).unwrap());
        assert!(caps.supports("sharedTexture".to_owned()).unwrap());
        assert!(!caps.supports("sharedRenderTarget".to_owned()).unwrap());

        let cpu = JsExternalTextureDescriptor {
            inner: ExternalTextureDescriptor::cpu_rgba8(Size::new(2.0, 2.0), vec![0_u8; 16], 7),
        };
        assert_eq!(cpu.tier(), "cpuUpload");
        cpu.validate().unwrap();

        let invalid_cpu = JsExternalTextureDescriptor {
            inner: ExternalTextureDescriptor::cpu_rgba8(Size::new(2.0, 2.0), vec![0_u8; 15], 0),
        };
        assert!(invalid_cpu.validate().is_err());

        let handle = JsExternalBackendHandle::new("42".to_owned()).unwrap();
        let sync = JsExternalSync::generation("3".to_owned()).unwrap();
        let shared = JsExternalTextureDescriptor::shared_texture(
            "wgpu".to_owned(),
            &JsSize::new(4.0, 4.0),
            "rgba8unorm".to_owned(),
            &handle,
            &sync,
            None,
        )
        .unwrap();
        assert_eq!(shared.tier(), "sharedTexture");
        shared.validate().unwrap();

        let empty_handle = JsExternalBackendHandle::new("0".to_owned()).unwrap();
        let empty = JsExternalTextureDescriptor::shared_texture(
            "wgpu".to_owned(),
            &JsSize::new(4.0, 4.0),
            "rgba8unorm".to_owned(),
            &empty_handle,
            &JsExternalSync::none(),
            None,
        )
        .unwrap();
        assert!(empty.validate().is_err());
    }

    #[test]
    fn external_surface_draws_cpu_fallback() {
        let texture = JsExternalTextureDescriptor {
            inner: ExternalTextureDescriptor::cpu_rgba8(
                Size::new(2.0, 1.0),
                vec![255, 0, 0, 255, 0, 0, 255, 255],
                1,
            ),
        };
        let widget = js_external_surface(
            &texture,
            Some(&JsSize::new(64.0, 32.0)),
            Some("Preview".to_owned()),
        )
        .unwrap();

        let snapshot = render_widget(&widget, None).unwrap();

        assert_eq!(snapshot.draw_image_count, 1);
        assert!(snapshot.registered_image_count >= 1);
        assert!(snapshot.semantics_count >= 1);
    }

    #[test]
    fn event_descriptors_expose_pointer_and_keyboard_fields() {
        let pointer = JsEvent::pointer(
            "down".to_owned(),
            &JsPoint::new(8.0, 9.0),
            Some("17".to_owned()),
            None,
            Some("primary".to_owned()),
            Some(1),
            Some("mouse".to_owned()),
            Some(true),
        )
        .unwrap();

        assert_eq!(pointer.kind(), "pointer");
        assert_eq!(pointer.action().as_deref(), Some("down"));
        assert_eq!(pointer.pointer_id().as_deref(), Some("17"));
        assert_eq!(pointer.position().unwrap().x, 8.0);
        assert_eq!(pointer.button().as_deref(), Some("primary"));
        assert_eq!(pointer.buttons(), Some(1));

        let keyboard = JsEvent::keyboard("Enter".to_owned(), None, None, None, None, None).unwrap();
        assert_eq!(keyboard.kind(), "keyboard");
        assert_eq!(keyboard.key().as_deref(), Some("Enter"));
        assert_eq!(keyboard.state().as_deref(), Some("pressed"));
    }

    #[test]
    fn high_level_app_renders_basic_widget_tree() {
        let state = JsState::new(Either3::A("Ready".to_owned()));
        let root = JsWidget::from_binding(BindingWidget::column(
            [
                BindingWidget::label_state(state.inner.clone()),
                BindingWidget::button("Apply", None),
            ],
            8.0,
        ));
        let mut window = JsWindow::new("Bindings".to_owned());
        window.root(&root).unwrap();
        let app = JsApp::new();
        app.window(&window).unwrap();

        let snapshot = app.render(None).unwrap();

        assert_eq!(app.window_count(), 1);
        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 2);
        assert_eq!(state.text(), "Ready");
        state.set(Either3::A("Updated".to_owned()));
        match state.get() {
            Either3::A(value) => assert_eq!(value, "Updated"),
            _ => panic!("state should keep its string value"),
        }
    }

    fn assert_cross_language_snapshot_signature(snapshot: &JsRenderSnapshot) {
        assert!(snapshot.command_count > 0);
        assert!(snapshot.semantics_count >= 30);

        for role in [
            "generic_container",
            "text",
            "button",
            "link",
            "checkbox",
            "switch",
            "radio_button",
            "radio_group",
            "breadcrumb",
            "list",
            "list_item",
            "table",
            "slider",
            "spin_box",
            "combo_box",
            "progress_bar",
            "busy_indicator",
            "text_input",
            "image",
            "scroll_view",
            "color_swatch",
            "separator",
        ] {
            assert!(
                snapshot.semantics_roles.iter().any(|value| value == role),
                "missing semantics role {role:?} in {:?}",
                snapshot.semantics_roles
            );
        }

        for name in [
            "Ready",
            "Apply",
            "Search icon",
            "Download",
            "Main surface",
            "Surface content",
            "Main toolbar",
            "Toolbar action",
            "Toolbar search",
            "Documentation",
            "Enabled",
            "Airplane mode",
            "Manual",
            "Priority",
            "View mode",
            "Show list view",
            "Gallery",
            "Show map view",
            "Workspace path",
            "Assets",
            "Brush",
            "Canvas",
            "Export",
            "Build table",
            "Input signal",
            "Online",
            "Editor status",
            "Ln 12",
            "Writable",
            "UTF-8",
            "Build",
            "Opacity",
            "Count",
            "Mode",
            "Load progress",
            "Background work",
            "Name",
            "Notes",
            "Scrollable content",
            "Rich summary",
            "Accent",
            "Section divider",
            "Projects empty",
            "New project",
        ] {
            assert!(
                snapshot.semantics_names.iter().any(|value| value == name),
                "missing semantics name {name:?} in {:?}",
                snapshot.semantics_names
            );
        }

        for value in [
            "https://example.invalid/docs",
            "0.5:0:1",
            "3:0:10",
            "Medium",
            "Gallery",
            "List",
            "Map",
            "sui",
            "Canvas",
            "Bindings",
            "active",
            "Online",
            "All systems nominal",
            "Ln 12",
            "Writable",
            "UTF-8",
            "Debug profile with local bindings",
            "Final",
            "0.25:0:1",
            "Ada",
            "Line one\nLine two",
            "Warm cool",
            "#4080BFFF",
        ] {
            assert!(
                snapshot.semantics_values.iter().any(|found| found == value),
                "missing semantics value {value:?} in {:?}",
                snapshot.semantics_values
            );
        }

        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Loading assets"),
            "missing busy indicator description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Download file"),
            "missing icon button description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Live audio input"),
            "missing signal meter description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Compact rows"),
            "missing segmented control description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "All systems nominal"),
            "missing status bar description in {:?}",
            snapshot.semantics_descriptions
        );
        assert!(
            snapshot
                .semantics_descriptions
                .iter()
                .any(|value| value == "Create a project to get started. Templates are available"),
            "missing empty state description in {:?}",
            snapshot.semantics_descriptions
        );
        for checked in ["checked", "unchecked"] {
            assert!(
                snapshot
                    .semantics_checked
                    .iter()
                    .any(|value| value == checked),
                "missing checked state {checked:?} in {:?}",
                snapshot.semantics_checked
            );
        }
        assert!(
            snapshot.semantics_busy.iter().any(|value| *value),
            "missing busy semantics state in {:?}",
            snapshot.semantics_busy
        );
        assert!(
            snapshot
                .semantics_editable_multiline
                .iter()
                .any(|value| *value),
            "missing multiline editable semantics in {:?}",
            snapshot.semantics_editable_multiline
        );
        assert!(
            snapshot.semantics_selected.iter().any(|value| *value),
            "missing selected semantics state in {:?}",
            snapshot.semantics_selected
        );
    }

    #[test]
    fn high_level_app_renders_cross_language_compatibility_signature() {
        let opacity = JsState::new(Either3::B(0.5));
        let count = JsState::new(Either3::B(3.0));
        let progress = JsState::new(Either3::B(0.25));
        let text = JsState::new(Either3::A("Ada".to_owned()));
        let notes = JsState::new(Either3::A("Line one\nLine two".to_owned()));
        let root = JsWidget::from_binding(BindingWidget::column(
            [
                BindingWidget::label("Ready"),
                BindingWidget::button("Apply", None),
                BindingWidget::icon(
                    sui_crate::IconGlyph::Search,
                    Some("Search icon".to_owned()),
                    None,
                    None,
                ),
                BindingWidget::icon_button(
                    sui_crate::IconGlyph::Download,
                    "Download",
                    true,
                    true,
                    Some(28.0),
                    Some(16.0),
                    Some("Download file".to_owned()),
                    None,
                ),
                BindingWidget::surface(
                    BindingWidget::label("Surface content"),
                    sui_crate::SurfaceRole::Panel,
                    Some("Main surface".to_owned()),
                    None,
                    Some(sui_crate::SurfaceElevation::Small),
                    None,
                    Some(6.0),
                    false,
                    false,
                ),
                BindingWidget::toolbar(
                    [
                        BindingWidget::button("Toolbar action", None),
                        BindingWidget::icon(
                            sui_crate::IconGlyph::Search,
                            Some("Toolbar search".to_owned()),
                            None,
                            None,
                        ),
                    ],
                    Axis::Horizontal,
                    Some("Main toolbar".to_owned()),
                    Some(32.0),
                    Some(4.0),
                    Some(4.0),
                    None,
                    true,
                ),
                BindingWidget::link(
                    "Documentation",
                    "https://example.invalid/docs",
                    None,
                    true,
                    None,
                ),
                BindingWidget::checkbox("Enabled", true, None),
                BindingWidget::switch("Airplane mode", false, None),
                BindingWidget::radio_button("Manual", true, None),
                BindingWidget::radio_group(
                    "Priority",
                    ["Low", "Medium", "High"],
                    Some(BindingNumber::Static(1.0)),
                    None,
                ),
                BindingWidget::segmented_control(
                    "View mode",
                    [
                        BindingSegmentedControlItem::new(
                            "List",
                            Some("Show list view".to_owned()),
                            Some("Compact rows".to_owned()),
                            false,
                        ),
                        BindingSegmentedControlItem::new("Gallery", None, None, false),
                        BindingSegmentedControlItem::new(
                            "Map",
                            Some("Show map view".to_owned()),
                            None,
                            true,
                        ),
                    ],
                    Some(BindingNumber::Static(1.0)),
                    None,
                ),
                BindingWidget::breadcrumb(
                    "Workspace path",
                    ["D:", "Workspace", "sui"],
                    Some(BindingNumber::Static(2.0)),
                    None,
                ),
                BindingWidget::list_view(
                    "Assets",
                    ["Brush", "Canvas", "Export"],
                    Some(BindingNumber::Static(1.0)),
                    None,
                ),
                BindingWidget::table(
                    "Build table",
                    [
                        BindingTableColumn::new(
                            "Task",
                            Some(160.0),
                            None,
                            sui_crate::TableColumnAlignment::Start,
                            false,
                        ),
                        BindingTableColumn::new(
                            "Owner",
                            Some(96.0),
                            None,
                            sui_crate::TableColumnAlignment::Center,
                            false,
                        ),
                    ],
                    [
                        BindingTableRow::new(["Bindings", "IX"]),
                        BindingTableRow::new(["Renderer", "Core"]),
                    ],
                    Some(BindingNumber::Static(0.0)),
                    None,
                ),
                BindingWidget::signal_meter(
                    "Input signal",
                    true,
                    Some("Live audio input".to_owned()),
                    8,
                    Some(Size::new(76.0, 16.0)),
                ),
                BindingWidget::status_badge(
                    "Online",
                    sui_crate::SemanticTone::Success,
                    Some(sui_crate::IconGlyph::Check),
                    Some(72.0),
                ),
                BindingWidget::status_bar(
                    [
                        BindingStatusBarSegment::new(
                            "Ln 12",
                            sui_crate::SemanticTone::Neutral,
                            None,
                            false,
                        ),
                        BindingStatusBarSegment::new(
                            "Writable",
                            sui_crate::SemanticTone::Success,
                            Some(84.0),
                            false,
                        ),
                        BindingStatusBarSegment::new(
                            "UTF-8",
                            sui_crate::SemanticTone::Info,
                            None,
                            true,
                        ),
                    ],
                    Some("Editor status".to_owned()),
                    Some("All systems nominal".into()),
                    Some(24.0),
                ),
                BindingWidget::detail_row("Build", "Debug profile with local bindings", Some(2)),
                BindingWidget::slider("Opacity", opacity.inner.clone(), 0.0, 1.0, 0.25, None),
                BindingWidget::number_input("Count", count.inner.clone(), 0.0, 10.0, 1.0, 0, None),
                BindingWidget::select(
                    "Mode",
                    ["Draft", "Final", "Review"],
                    Some(BindingNumber::Static(1.0)),
                    Some("Choose mode".to_owned()),
                    None,
                ),
                BindingWidget::progress_bar(
                    "Load progress",
                    progress.inner.clone(),
                    0.0,
                    1.0,
                    true,
                ),
                BindingWidget::busy_indicator(
                    "Background work",
                    Some("Loading assets".into()),
                    20.0,
                ),
                BindingWidget::text_input(
                    "Name",
                    text.inner.clone(),
                    Some("Type a name".to_owned()),
                    None,
                ),
                BindingWidget::text_area(
                    "Notes",
                    notes.inner.clone(),
                    Some("Type notes".to_owned()),
                    None,
                ),
                BindingWidget::scroll_view(
                    BindingWidget::rich_text(
                        [
                            BindingTextSpan::new(
                                "Warm",
                                TextStyle::new(Color::rgba(0.9, 0.35, 0.2, 1.0)),
                            ),
                            BindingTextSpan::new(
                                " cool",
                                TextStyle::new(Color::rgba(0.25, 0.55, 0.9, 1.0)),
                            ),
                        ],
                        Some("Rich summary".to_owned()),
                        0.0,
                        0.0,
                    ),
                    BindingScrollAxes::Vertical,
                    Some("Scrollable content".to_owned()),
                ),
                BindingWidget::color_swatch(
                    "Accent",
                    Color::rgba(0.25, 0.5, 0.75, 1.0),
                    Some(Size::new(24.0, 24.0)),
                    false,
                    None,
                ),
                BindingWidget::separator(
                    Axis::Horizontal,
                    Some("Section divider".to_owned()),
                    0.0,
                    None,
                    Some(24.0),
                ),
                BindingWidget::empty_state(
                    "No projects",
                    "Create a project to get started.",
                    Some("Projects empty".to_owned()),
                    Some("Templates are available".to_owned()),
                    Some(sui_crate::IconGlyph::Folder),
                    Some(BindingWidget::button("New project", None)),
                    None,
                    true,
                ),
            ],
            6.0,
        ));
        let mut window = JsWindow::new("Compatibility".to_owned());
        window.root(&root).unwrap();
        let app = JsApp::new();
        app.window(&window).unwrap();

        let snapshot = app.render(None).unwrap();

        assert_cross_language_snapshot_signature(&snapshot);
    }

    #[test]
    fn app_registers_image_resources() {
        let app = JsApp::new();
        let image = app
            .rgba_image(2, 1, Buffer::from(vec![255, 0, 0, 255, 0, 0, 255, 255]))
            .unwrap();
        let widget = js_image(
            &image,
            Some("Preview".to_owned()),
            Some("contain".to_owned()),
            Some(&JsSize::new(32.0, 16.0)),
        )
        .unwrap();
        let mut window = JsWindow::new("Image".to_owned());
        window.root(&widget).unwrap();
        app.window(&window).unwrap();

        assert_eq!(app.image_resource_count(), 1);
        assert!(image.id().parse::<u64>().unwrap() > 0);
        assert_eq!(image.local_slot(), None);
        assert!(app.rgba_image(2, 1, Buffer::from(vec![0_u8; 7])).is_err());
        assert!(js_image(&image, None, Some("stretch".to_owned()), None).is_err());

        let snapshot = app.render(None).unwrap();
        assert_eq!(snapshot.draw_image_count, 1);
        assert!(snapshot.registered_image_count >= 1);
        assert!(snapshot.semantics_roles.iter().any(|role| role == "image"));
        assert!(
            snapshot
                .semantics_names
                .iter()
                .any(|name| name == "Preview")
        );
    }

    #[test]
    fn app_registers_font_resources() {
        let app = JsApp::new();
        let font = app.font_bytes(Buffer::from(vec![0, 1, 2, 3])).unwrap();
        let mut window = JsWindow::new("Font".to_owned());
        let label = JsWidget::from_binding(BindingWidget::label("Text"));
        window.root(&label).unwrap();
        app.window(&window).unwrap();

        assert_eq!(app.font_resource_count(), 1);
        assert!(font.id().parse::<u64>().unwrap() > 0);
        assert_eq!(app.render(None).unwrap().registered_font_count, 1);
    }

    #[test]
    fn app_registers_file_resources() {
        let svg_path = unique_temp_path("icon", "svg");
        let png_path = unique_temp_path("icon", "png");
        let font_path = unique_temp_path("font", "bin");
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        fs::write(
            &svg_path,
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#,
        )
        .unwrap();
        fs::write(&png_path, &png).unwrap();
        fs::write(&font_path, vec![0, 1, 2, 3]).unwrap();

        let app = JsApp::new();
        let image = app
            .svg_file(svg_path.to_string_lossy().to_string())
            .unwrap();
        let resized = app
            .svg_file_at_size(16, 16, svg_path.to_string_lossy().to_string())
            .unwrap();
        let png_bytes = app.png_image(Buffer::from(png)).unwrap();
        let png_file = app
            .png_file(png_path.to_string_lossy().to_string())
            .unwrap();
        let font = app
            .font_file(font_path.to_string_lossy().to_string())
            .unwrap();

        assert_eq!(app.image_resource_count(), 4);
        assert_eq!(app.font_resource_count(), 1);
        assert!(image.id().parse::<u64>().unwrap() > 0);
        assert!(resized.id().parse::<u64>().unwrap() > 0);
        assert!(png_bytes.id().parse::<u64>().unwrap() > 0);
        assert!(png_file.id().parse::<u64>().unwrap() > 0);
        assert!(font.id().parse::<u64>().unwrap() > 0);

        let _ = fs::remove_file(svg_path);
        let _ = fs::remove_file(png_path);
        let _ = fs::remove_file(font_path);
    }

    #[test]
    fn running_app_drains_bound_state_updates() {
        let state = JsState::new(Either3::A("Ready".to_owned()));
        let root = JsWidget::from_binding(BindingWidget::label_state(state.inner.clone()));
        let mut window = JsWindow::new("Runtime".to_owned());
        window.root(&root).unwrap();
        let app = JsApp::new();
        app.window(&window).unwrap();
        let running = app.start().unwrap();
        let window = running.window_id(0).unwrap();

        assert_eq!(running.window_count(), 1);
        assert_eq!(running.window_ids(), vec![window.id()]);
        assert!(running.render_window(&window).unwrap().command_count > 0);

        state.set(Either3::A("Queued".to_owned()));
        assert_eq!(state.text(), "Ready");
        assert_eq!(running.pending_count(), 1);
        assert_eq!(running.drain().unwrap(), 1);
        assert_eq!(state.text(), "Queued");
        assert!(running.needs_render(None).unwrap());
        assert!(running.render(None).unwrap().command_count > 0);
    }

    #[cfg(not(feature = "desktop"))]
    #[test]
    fn app_run_reports_missing_desktop_feature() {
        let mut window = JsWindow::new("Headless".to_owned());
        let label = JsWidget::from_binding(BindingWidget::label("No desktop"));
        window.root(&label).unwrap();
        let app = JsApp::new();
        app.window(&window).unwrap();

        assert!(app.run().unwrap_err().to_string().contains("desktop"));
    }

    struct MockCallbacks;

    impl ForeignWidgetCallbacks for MockCallbacks {
        fn measure(
            &self,
            _id: sui_bindings_core::ForeignWidgetId,
            _ctx: &mut ForeignMeasureCtx<'_>,
            constraints: Constraints,
        ) -> ForeignCallbackResult<Size> {
            Ok(constraints.clamp(Size::new(160.0, 28.0)))
        }

        fn paint(
            &self,
            _id: sui_bindings_core::ForeignWidgetId,
            ctx: &mut ForeignPaintCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            let bounds = ctx.bounds();
            let mut builder = PaintCommandBuilder::new();
            builder.fill_rect(bounds, Color::rgba(0.11, 0.12, 0.14, 1.0))?;
            builder.fill_rect(
                Rect::new(
                    bounds.x(),
                    bounds.y(),
                    bounds.width() * 0.5,
                    bounds.height(),
                ),
                Color::rgba(0.25, 0.68, 0.46, 1.0),
            )?;
            ctx.apply_all(builder.finish()?)?;
            Ok(())
        }

        fn semantics(
            &self,
            _id: sui_bindings_core::ForeignWidgetId,
            ctx: &mut ForeignSemanticsCtx<'_>,
        ) -> ForeignCallbackResult<()> {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some("JS meter".to_owned());
            node.state.disabled = true;
            node.state.selected = true;
            node.state.expanded = Some(true);
            ctx.push(node);
            Ok(())
        }
    }
}
