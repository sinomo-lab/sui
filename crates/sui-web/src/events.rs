use crate::prelude::*;

#[wasm_bindgen(js_name = Event)]
#[derive(Debug, Clone)]
pub struct JsEvent {
    pub(crate) inner: BindingEvent,
}

js_class!(JsEvent => "Event");

#[wasm_bindgen(js_class = Event)]
impl JsEvent {
    #[wasm_bindgen(js_name = rawMouseMotion)]
    pub fn raw_mouse_motion(
        delta: &JsPoint,
        modifiers: Option<ClassArg<JsModifiers>>,
    ) -> Result<Self> {
        let modifiers = modifiers.map(|value| value.value()).transpose()?;
        Ok(Self {
            inner: BindingEvent::RawMouseMotion(BindingRawMouseMotionEvent {
                delta: Vector::new(delta.x as f32, delta.y as f32),
                modifiers: modifiers.unwrap_or_default().into(),
            }),
        })
    }

    #[wasm_bindgen(js_name = pointer)]
    pub fn pointer_with_modifiers(
        kind: String,
        position: &JsPoint,
        pointer_id: Option<String>,
        delta: Option<ClassArg<JsPoint>>,
        button: Option<String>,
        buttons: Option<u32>,
        pointer_kind: Option<String>,
        is_primary: Option<bool>,
        modifiers: Option<ClassArg<JsModifiers>>,
    ) -> Result<Self> {
        let delta = delta.map(|value| value.value()).transpose()?;
        let delta = delta.as_ref();
        let modifiers = modifiers.map(|value| value.value()).transpose()?;
        let modifiers = modifiers.as_ref();
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
                modifiers: modifiers.copied().map(Into::into).unwrap_or_default(),
                pointer_kind: pointer_kind_from_js(pointer_kind.as_deref().unwrap_or("mouse"))?,
                is_primary: is_primary.unwrap_or(true),
            }),
        })
    }

    #[wasm_bindgen(js_name = scroll)]
    pub fn scroll_with_modifiers(
        position: &JsPoint,
        delta: &JsPoint,
        mode: Option<String>,
        pointer_id: Option<String>,
        modifiers: Option<ClassArg<JsModifiers>>,
    ) -> Result<Self> {
        let modifiers = modifiers.map(|value| value.value()).transpose()?;
        let modifiers = modifiers.as_ref();
        let scroll_delta = match mode.as_deref().unwrap_or("pixels") {
            "pixels" | "pixel" => BindingScrollDelta::Pixels((*delta).into()),
            "lines" | "line" => BindingScrollDelta::Lines((*delta).into()),
            _ => return Err(invalid_arg("scroll mode must be 'pixels' or 'lines'")),
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
                modifiers: modifiers.copied().map(Into::into).unwrap_or_default(),
                pointer_kind: BindingPointerKind::Mouse,
                is_primary: true,
            }),
        })
    }

    #[wasm_bindgen(js_name = keyboard)]
    pub fn keyboard_with_modifiers(
        key: String,
        state: Option<String>,
        code: Option<String>,
        text: Option<String>,
        repeat: Option<bool>,
        is_composing: Option<bool>,
        modifiers: Option<ClassArg<JsModifiers>>,
    ) -> Result<Self> {
        let modifiers = modifiers.map(|value| value.value()).transpose()?;
        let modifiers = modifiers.as_ref();
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
        event.modifiers = modifiers.copied().map(Into::into).unwrap_or_default();
        Ok(Self {
            inner: BindingEvent::Keyboard(event),
        })
    }

    #[wasm_bindgen]
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
                return Err(invalid_arg(
                    "IME kind must be 'compositionStart', 'compositionUpdate', 'compositionCommit', or 'compositionEnd'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Ime(inner),
        })
    }

    #[wasm_bindgen]
    pub fn window(
        kind: String,
        value: Option<bool>,
        size: Option<ClassArg<JsSize>>,
        scale_factor: Option<f64>,
        raw_dpi: Option<f64>,
        suggested_size: Option<ClassArg<JsSize>>,
        position: Option<ClassArg<JsPoint>>,
    ) -> Result<Self> {
        let size = size.map(|value| value.value()).transpose()?;
        let size = size.as_ref();
        let suggested_size = suggested_size.map(|value| value.value()).transpose()?;
        let suggested_size = suggested_size.as_ref();
        let position = position.map(|value| value.value()).transpose()?;
        let position = position.as_ref();
        let inner = match kind.as_str() {
            "closeRequested" | "close_requested" | "close" => BindingWindowEvent::CloseRequested,
            "resized" | "resize" => BindingWindowEvent::Resized(
                size.ok_or_else(|| invalid_arg("resized window events require size"))?
                    .to_sui(),
            ),
            "moved" | "move" => BindingWindowEvent::Moved(
                position
                    .ok_or_else(|| invalid_arg("moved window events require position"))?
                    .to_owned()
                    .into(),
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
                return Err(invalid_arg(
                    "window kind must be 'closeRequested', 'resized', 'moved', 'scaleFactorChanged', 'focused', 'occluded', or 'redrawRequested'",
                ));
            }
        };
        Ok(Self {
            inner: BindingEvent::Window(inner),
        })
    }

    #[wasm_bindgen]
    pub fn custom(kind: String, payload: Option<String>) -> Self {
        Self {
            inner: BindingEvent::Custom(BindingCustomEvent { kind, payload }),
        }
    }

    #[wasm_bindgen(getter = kind)]
    pub fn kind(&self) -> String {
        self.inner.kind().to_string()
    }

    #[wasm_bindgen(getter = action)]
    pub fn action(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(pointer_event_kind_name(event.kind).to_string()),
            BindingEvent::Ime(event) => Some(ime_event_kind_name(event).to_string()),
            BindingEvent::Window(event) => Some(window_event_kind_name(event).to_string()),
            BindingEvent::Custom(_)
            | BindingEvent::Keyboard(_)
            | BindingEvent::RawMouseMotion(_)
            | BindingEvent::Unsupported { .. } => None,
        }
    }

    #[wasm_bindgen(getter = pointerId)]
    pub fn pointer_id(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.pointer_id.to_string()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = position)]
    pub fn position(&self) -> Option<JsPoint> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.position.into()),
            BindingEvent::Window(BindingWindowEvent::Moved(position)) => Some((*position).into()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = delta)]
    pub fn delta(&self) -> Option<JsPoint> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(JsPoint::new(
                f64::from(event.delta.x),
                f64::from(event.delta.y),
            )),
            BindingEvent::RawMouseMotion(event) => Some(JsPoint::new(
                f64::from(event.delta.x),
                f64::from(event.delta.y),
            )),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = scrollMode)]
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

    #[wasm_bindgen(getter = button)]
    pub fn button(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => event.button.map(pointer_button_name),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = buttons)]
    pub fn buttons(&self) -> Option<u32> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(u32::from(event.buttons)),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = modifiers)]
    pub fn modifiers(&self) -> Option<JsModifiers> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.modifiers.into()),
            BindingEvent::RawMouseMotion(event) => Some(event.modifiers.into()),
            BindingEvent::Keyboard(event) => Some(event.modifiers.into()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = deviceKind)]
    pub fn device_kind(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(pointer_kind_name(event.pointer_kind).to_owned()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = isPrimary)]
    pub fn is_primary(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Pointer(event) => Some(event.is_primary),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = key)]
    pub fn key(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.key.clone()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = code)]
    pub fn code(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.code.clone()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = text)]
    pub fn text(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => event.text.clone(),
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate { text, .. })
            | BindingEvent::Ime(BindingImeEvent::CompositionCommit { text }) => Some(text.clone()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = state)]
    pub fn state(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(key_state_name(event.state).to_owned()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = repeat)]
    pub fn repeat(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.repeat),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = isComposing)]
    pub fn is_composing(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Keyboard(event) => Some(event.is_composing),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = customKind)]
    pub fn custom_kind(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Custom(event) => Some(event.kind.clone()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = payload)]
    pub fn payload(&self) -> Option<String> {
        match &self.inner {
            BindingEvent::Custom(event) => event.payload.clone(),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = cursorStart)]
    pub fn cursor_start(&self) -> Option<u32> {
        match &self.inner {
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate { cursor_start, .. }) => {
                cursor_start.and_then(|value| u32::try_from(value).ok())
            }
            _ => None,
        }
    }

    #[wasm_bindgen(getter = cursorEnd)]
    pub fn cursor_end(&self) -> Option<u32> {
        match &self.inner {
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate { cursor_end, .. }) => {
                cursor_end.and_then(|value| u32::try_from(value).ok())
            }
            _ => None,
        }
    }

    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> Option<bool> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::Focused(value))
            | BindingEvent::Window(BindingWindowEvent::Occluded(value)) => Some(*value),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = size)]
    pub fn size(&self) -> Option<JsSize> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::Resized(size)) => Some((*size).into()),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = scaleFactor)]
    pub fn scale_factor(&self) -> Option<f64> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::ScaleFactorChanged {
                scale_factor, ..
            }) => Some(*scale_factor),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = rawDpi)]
    pub fn raw_dpi(&self) -> Option<f64> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::ScaleFactorChanged { raw_dpi, .. }) => {
                (*raw_dpi).map(f64::from)
            }
            _ => None,
        }
    }

    #[wasm_bindgen(getter = suggestedSize)]
    pub fn suggested_size(&self) -> Option<JsSize> {
        match &self.inner {
            BindingEvent::Window(BindingWindowEvent::ScaleFactorChanged {
                suggested_size, ..
            }) => (*suggested_size).map(Into::into),
            _ => None,
        }
    }

    #[wasm_bindgen(getter = filePath)]
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
    pub(crate) fn from_binding(inner: BindingEvent) -> Self {
        Self { inner }
    }

    pub(crate) fn binding_event(&self) -> BindingEvent {
        self.inner.clone()
    }
}

#[wasm_bindgen(js_name = EventContext)]
#[derive(Clone)]
pub struct JsEventContext {
    pub(crate) inner: Arc<Mutex<BindingEventContext>>,
}

js_class!(JsEventContext => "EventContext");

#[wasm_bindgen(js_class = EventContext)]
impl JsEventContext {
    #[wasm_bindgen(getter = windowId)]
    pub fn window_id(&self) -> String {
        recover_lock(&self.inner).window_id.to_string()
    }

    #[wasm_bindgen(getter = widgetId)]
    pub fn widget_id(&self) -> String {
        recover_lock(&self.inner).widget_id.to_string()
    }

    #[wasm_bindgen(getter = bounds)]
    pub fn bounds(&self) -> JsRect {
        recover_lock(&self.inner).bounds.into()
    }

    #[wasm_bindgen(getter = currentTime)]
    pub fn current_time(&self) -> f64 {
        recover_lock(&self.inner).current_time
    }

    #[wasm_bindgen(getter = phase)]
    pub fn phase(&self) -> String {
        recover_lock(&self.inner).phase.to_owned()
    }

    #[wasm_bindgen(getter = focused)]
    pub fn focused(&self) -> bool {
        recover_lock(&self.inner).focused
    }

    #[wasm_bindgen(getter = clipboardText)]
    pub fn clipboard_text(&self) -> Option<String> {
        recover_lock(&self.inner).clipboard_text.clone()
    }

    #[wasm_bindgen(js_name = setHandled)]
    pub fn set_handled(&self) {
        recover_lock(&self.inner).set_handled();
    }

    #[wasm_bindgen(js_name = requestFocus)]
    pub fn request_focus(&self) {
        recover_lock(&self.inner).request_focus();
    }

    #[wasm_bindgen(js_name = clearFocus)]
    pub fn clear_focus(&self) {
        recover_lock(&self.inner).clear_focus();
    }

    #[wasm_bindgen(js_name = requestMeasure)]
    pub fn request_measure(&self) {
        recover_lock(&self.inner).request_measure();
    }

    #[wasm_bindgen(js_name = requestArrange)]
    pub fn request_arrange(&self) {
        recover_lock(&self.inner).request_arrange();
    }

    #[wasm_bindgen(js_name = requestPaint)]
    pub fn request_paint(&self, rect: Option<ClassArg<JsRect>>) -> Result<()> {
        let rect = rect.map(|value| value.value()).transpose()?;
        let mut context = recover_lock(&self.inner);
        if let Some(rect) = rect {
            context.request_paint_rect(rect.into());
        } else {
            context.request_paint();
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = requestSemantics)]
    pub fn request_semantics(&self) {
        recover_lock(&self.inner).request_semantics();
    }

    #[wasm_bindgen(js_name = requestAnimationFrame)]
    pub fn request_animation_frame(&self) {
        recover_lock(&self.inner).request_animation_frame();
    }

    #[wasm_bindgen(js_name = capturePointer)]
    pub fn capture_pointer(&self, pointer_id: String) -> Result<()> {
        recover_lock(&self.inner).capture_pointer(parse_u64_string(&pointer_id, "pointer id")?);
        Ok(())
    }

    #[wasm_bindgen(js_name = releasePointer)]
    pub fn release_pointer(&self, pointer_id: String) -> Result<()> {
        recover_lock(&self.inner).release_pointer(parse_u64_string(&pointer_id, "pointer id")?);
        Ok(())
    }

    #[wasm_bindgen(js_name = setClipboardText)]
    pub fn set_clipboard_text(&self, text: String) {
        recover_lock(&self.inner).set_clipboard_text(text);
    }
}

impl JsEventContext {
    pub(crate) fn new(inner: BindingEventContext) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub(crate) fn apply(&self, context: &mut ForeignEventCtx<'_>) {
        recover_lock(&self.inner).apply(context);
    }
}
