use crate::prelude::*;

#[wasm_bindgen(js_name = FontHandle)]
#[derive(Debug, Clone, Copy)]
pub struct JsFontHandle {
    pub(crate) inner: BindingFontHandle,
}

js_class!(JsFontHandle => "FontHandle");

#[wasm_bindgen(js_class = FontHandle)]
impl JsFontHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> Result<Self> {
        Ok(Self {
            inner: BindingFontHandle::new(parse_u64_string(&id, "font handle id")?),
        })
    }

    #[wasm_bindgen(getter = id)]
    pub fn id(&self) -> String {
        self.inner.get().to_string()
    }
}

#[wasm_bindgen(js_name = ImageHandle)]
#[derive(Debug, Clone, Copy)]
pub struct JsImageHandle {
    pub(crate) inner: BindingImageHandle,
}

js_class!(JsImageHandle => "ImageHandle");

#[wasm_bindgen(js_class = ImageHandle)]
impl JsImageHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> Result<Self> {
        Ok(Self {
            inner: BindingImageHandle::new(parse_u64_string(&id, "image handle id")?),
        })
    }

    #[wasm_bindgen]
    pub fn local(slot: u32) -> Self {
        Self {
            inner: BindingImageHandle::local(u64::from(slot)),
        }
    }

    #[wasm_bindgen(getter = id)]
    pub fn id(&self) -> String {
        self.inner.get().to_string()
    }

    #[wasm_bindgen(getter = localSlot)]
    pub fn local_slot(&self) -> Option<u32> {
        self.inner
            .local_slot()
            .and_then(|slot| u32::try_from(slot).ok())
    }
}

#[derive(Clone)]
pub(crate) struct PendingPaintImage {
    slot: u64,
    image: RegisteredImage,
}

#[wasm_bindgen(js_name = Paint)]
#[derive(Clone)]
pub struct JsPaint {
    pub(crate) builder: Arc<Mutex<PaintCommandBuilder>>,
    pub(crate) images: Arc<Mutex<Vec<PendingPaintImage>>>,
    pub(crate) bounds: JsRect,
}

js_class!(JsPaint => "Paint");

#[wasm_bindgen(js_class = Paint)]
impl JsPaint {
    #[wasm_bindgen(getter = bounds)]
    pub fn bounds(&self) -> JsRect {
        self.bounds
    }

    #[wasm_bindgen]
    pub fn clear(&self, color: &JsColor) -> Result<()> {
        self.with_builder(|builder| builder.clear((*color).into()).map(|_| ()))
    }

    #[wasm_bindgen(js_name = fillRect)]
    pub fn fill_rect(&self, rect: &JsRect, color: &JsColor) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .fill_rect((*rect).into(), Color::from(*color))
                .map(|_| ())
        })
    }

    #[wasm_bindgen(js_name = strokeRect)]
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

    #[wasm_bindgen(js_name = fillPath)]
    pub fn fill_path(&self, path: &JsPath, color: &JsColor) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .fill_path(path.inner.clone(), Color::from(*color))
                .map(|_| ())
        })
    }

    #[wasm_bindgen(js_name = strokePath)]
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

    #[wasm_bindgen(js_name = fillRoundedRect)]
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

    #[wasm_bindgen(js_name = drawShadow)]
    pub fn draw_shadow(&self, rect: &JsRect, shadow: &JsShadow, radius: Option<f64>) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .draw_shadow((*rect).into(), js_radii(radius), (*shadow).into())
                .map(|_| ())
        })
    }

    #[wasm_bindgen(js_name = fillRoundedRectWithShadow)]
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

    #[wasm_bindgen(js_name = fillBounds)]
    pub fn fill_bounds(&self, color: &JsColor) -> Result<()> {
        self.fill_rect(&self.bounds, color)
    }

    #[wasm_bindgen(js_name = drawText)]
    pub fn draw_text(
        &self,
        rect: &JsRect,
        text: String,
        color: Option<ClassArg<JsColor>>,
        font_size: Option<f64>,
        line_height: Option<f64>,
        font: Option<ClassArg<JsFontHandle>>,
        weight: Option<u32>,
        style: Option<String>,
        stretch: Option<String>,
    ) -> Result<()> {
        let color = color.map(|value| value.value()).transpose()?;
        let color = color.as_ref();
        let font = font.map(|value| value.value()).transpose()?;
        let font = font.as_ref();
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

    #[wasm_bindgen(js_name = drawShaderRect)]
    pub fn draw_shader_rect(&self, rect: &JsRect, shader: &JsShader) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .draw_binding_shader_rect((*rect).into(), shader.inner)
                .map(|_| ())
        })
    }

    #[wasm_bindgen(js_name = rgbaImage)]
    pub fn rgba_image(
        &self,
        slot: u32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<JsImageHandle> {
        let image = RegisteredImage::from_rgba8(width, height, pixels).map_err(invalid_arg)?;
        recover_lock(&self.images).push(PendingPaintImage {
            slot: u64::from(slot),
            image,
        });
        Ok(JsImageHandle {
            inner: BindingImageHandle::local(u64::from(slot)),
        })
    }

    #[wasm_bindgen(js_name = drawImage)]
    pub fn draw_image(&self, rect: &JsRect, image: &JsImageHandle) -> Result<()> {
        self.with_builder(|builder| {
            builder
                .draw_binding_image((*rect).into(), image.inner)
                .map(|_| ())
        })
    }

    #[wasm_bindgen(js_name = drawImageQuad)]
    pub fn draw_image_quad(&self, points: Array, image: &JsImageHandle) -> Result<()> {
        let points = js_four_points(&points)?;
        self.with_builder(|builder| {
            builder
                .draw_binding_image_quad(points, image.inner)
                .map(|_| ())
        })
    }

    #[wasm_bindgen(js_name = pushClipRect)]
    pub fn push_clip_rect(&self, rect: &JsRect) -> Result<()> {
        self.with_builder(|builder| builder.push_clip_rect((*rect).into()).map(|_| ()))
    }

    #[wasm_bindgen(js_name = pushClipPath)]
    pub fn push_clip_path(&self, path: &JsPath) -> Result<()> {
        self.with_builder(|builder| builder.push_clip_path(path.inner.clone()).map(|_| ()))
    }

    #[wasm_bindgen(js_name = popClip)]
    pub fn pop_clip(&self) -> Result<()> {
        self.with_builder(|builder| builder.pop_clip().map(|_| ()))
    }

    #[wasm_bindgen(js_name = pushTransform)]
    pub fn push_transform(&self, transform: &JsTransform) -> Result<()> {
        self.with_builder(|builder| builder.push_transform((*transform).into()).map(|_| ()))
    }

    #[wasm_bindgen(js_name = popTransform)]
    pub fn pop_transform(&self) -> Result<()> {
        self.with_builder(|builder| builder.pop_transform().map(|_| ()))
    }

    #[wasm_bindgen(getter = commandCount)]
    pub fn command_count(&self) -> u32 {
        recover_lock(&self.builder).command_count() as u32
    }
}

impl JsPaint {
    pub(crate) fn new(bounds: Rect) -> Self {
        Self {
            builder: Arc::new(Mutex::new(PaintCommandBuilder::new())),
            images: Arc::new(Mutex::new(Vec::new())),
            bounds: bounds.into(),
        }
    }

    pub(crate) fn with_builder(
        &self,
        f: impl FnOnce(&mut PaintCommandBuilder) -> std::result::Result<(), PaintValidationError>,
    ) -> Result<()> {
        f(&mut recover_lock(&self.builder)).map_err(value_error)
    }

    pub(crate) fn finish(&self) -> std::result::Result<Vec<PaintCommand>, PaintValidationError> {
        std::mem::take(&mut *recover_lock(&self.builder)).finish()
    }

    pub(crate) fn take_images(&self) -> Vec<PendingPaintImage> {
        std::mem::take(&mut *recover_lock(&self.images))
    }
}

#[derive(Clone)]
pub(crate) enum JsSemanticsCommand {
    Node(SemanticsNode),
    Child(usize),
}

#[wasm_bindgen(js_name = Semantics)]
#[derive(Clone)]
pub struct JsSemantics {
    pub(crate) widget_id: WidgetId,
    pub(crate) commands: Arc<Mutex<Vec<JsSemanticsCommand>>>,
    pub(crate) bounds: JsRect,
    pub(crate) focused: bool,
    pub(crate) child_count: u32,
}

js_class!(JsSemantics => "Semantics");

#[wasm_bindgen(js_class = Semantics)]
impl JsSemantics {
    #[wasm_bindgen(getter = bounds)]
    pub fn bounds(&self) -> JsRect {
        self.bounds
    }

    #[wasm_bindgen(getter = focused)]
    pub fn focused(&self) -> bool {
        self.focused
    }

    #[wasm_bindgen(getter = childCount)]
    pub fn child_count(&self) -> u32 {
        self.child_count
    }

    #[wasm_bindgen]
    pub fn node(
        &self,
        role: Option<String>,
        name: Option<String>,
        value: Option<JsValue>,
        description: Option<String>,
        bounds: Option<ClassArg<JsRect>>,
        disabled: Option<bool>,
        checked: Option<String>,
        selected: Option<bool>,
        expanded: Option<bool>,
        busy: Option<bool>,
        min_value: Option<f64>,
        max_value: Option<f64>,
    ) -> Result<()> {
        let bounds = bounds.map(|value| value.value()).transpose()?;
        let bounds = bounds.as_ref();
        let role_name = role.unwrap_or_else(|| "generic_container".to_owned());
        let role = binding_semantics_role_from_name(&role_name)
            .ok_or_else(|| invalid_arg(format!("unknown semantics role '{role_name}'")))?;
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

    #[wasm_bindgen]
    pub fn child(&self, index: u32) -> bool {
        if index >= self.child_count {
            return false;
        }
        recover_lock(&self.commands).push(JsSemanticsCommand::Child(index as usize));
        true
    }
}

impl JsSemantics {
    pub(crate) fn new(
        widget_id: WidgetId,
        bounds: Rect,
        focused: bool,
        child_count: usize,
    ) -> Self {
        Self {
            widget_id,
            commands: Arc::new(Mutex::new(Vec::new())),
            bounds: bounds.into(),
            focused,
            child_count: child_count as u32,
        }
    }

    pub(crate) fn take_commands(&self) -> Vec<JsSemanticsCommand> {
        std::mem::take(&mut *recover_lock(&self.commands))
    }
}

#[wasm_bindgen(js_name = Shader)]
#[derive(Debug, Clone, Copy)]
pub struct JsShader {
    pub(crate) inner: BindingShader,
}

js_class!(JsShader => "Shader");

#[wasm_bindgen(js_class = Shader)]
impl JsShader {
    #[wasm_bindgen(js_name = colorWheel)]
    pub fn color_wheel() -> Self {
        Self {
            inner: BindingShader::color_wheel(),
        }
    }

    #[wasm_bindgen(js_name = hueBar)]
    pub fn hue_bar() -> Self {
        Self {
            inner: BindingShader::hue_bar(),
        }
    }

    #[wasm_bindgen(js_name = saturationValuePlane)]
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
        .map_err(value_error)
    }

    #[wasm_bindgen(js_name = saturationBar)]
    pub fn saturation_bar(hue: f64, value: f64, color_space: Option<String>) -> Result<Self> {
        BindingShader::saturation_bar(color_space_from_js(color_space)?, hue as f32, value as f32)
            .map(Self::from)
            .map_err(value_error)
    }

    #[wasm_bindgen(js_name = valueBar)]
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
        .map_err(value_error)
    }

    #[wasm_bindgen(js_name = alphaBar)]
    pub fn alpha_bar(color: &JsColor) -> Result<Self> {
        BindingShader::alpha_bar(Color::from(*color))
            .map(Self::from)
            .map_err(value_error)
    }

    #[wasm_bindgen(js_name = rgbChannelBar)]
    pub fn rgb_channel_bar(color: &JsColor, channel: u32, max_value: Option<f64>) -> Result<Self> {
        BindingShader::rgb_channel_bar(
            Color::from(*color),
            channel,
            max_value.unwrap_or(1.0) as f32,
        )
        .map(Self::from)
        .map_err(value_error)
    }
}

impl From<BindingShader> for JsShader {
    fn from(value: BindingShader) -> Self {
        Self { inner: value }
    }
}

pub(crate) struct JsObjectCallbacks {
    object: Local<Object>,
}

impl JsObjectCallbacks {
    pub(crate) fn new(object: Object) -> Result<Self> {
        Ok(Self {
            object: Local(object),
        })
    }

    pub(crate) fn clone_ref(&self) -> Result<Self> {
        Ok(Self {
            object: self.object.clone(),
        })
    }

    /// The object's method `name`, or `None` when the object has no such property.
    fn method(&self, name: &str) -> Result<Option<Function>> {
        let key = JsValue::from_str(name);
        if !Reflect::has(&self.object, &key)? {
            return Ok(None);
        }
        Reflect::get(&self.object, &key)?
            .dyn_into::<Function>()
            .map(Some)
            .map_err(|_| invalid_arg(format!("{name} must be a function")))
    }

    fn call(&self, method: &Function, args: &[JsValue]) -> Result<JsValue> {
        method.apply(&self.object, &args.iter().collect::<Array>())
    }
}

pub(crate) struct JsWidgetCallbacks {
    pub(crate) callbacks: Mutex<Option<JsObjectCallbacks>>,
    pub(crate) child_sizes: Mutex<Vec<Size>>,
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
        let context = JsEventContext::new(BindingEventContext::from_foreign(ctx));
        let handled = callbacks
            .call_event(event, context.clone())
            .map_err(foreign_js_error)?;
        if handled {
            let mut requests = recover_lock(&context.inner);
            requests.set_handled();
            requests.request_paint();
        }
        context.apply(ctx);
        Ok(())
    }

    fn measure(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        let child_constraints = constraints.loosen();
        let child_sizes = (0..ctx.child_count())
            .map(|index| {
                ctx.measure_child(index, child_constraints)
                    .unwrap_or(Size::ZERO)
            })
            .collect::<Vec<_>>();
        *recover_lock(&self.child_sizes) = child_sizes.clone();
        let callbacks = recover_lock(&self.callbacks);
        let Some(callbacks) = callbacks.as_ref() else {
            return Err(ForeignCallbackFailure::new(
                "JavaScript widget callbacks were released",
            ));
        };
        callbacks
            .call_measure(constraints, &child_sizes)
            .map_err(foreign_js_error)
    }

    fn arrange(
        &self,
        _id: sui_bindings_core::ForeignWidgetId,
        ctx: &mut ForeignArrangeCtx<'_>,
        bounds: Rect,
    ) -> ForeignCallbackResult<()> {
        let child_sizes = recover_lock(&self.child_sizes).clone();
        let callbacks = recover_lock(&self.callbacks);
        let Some(callbacks) = callbacks.as_ref() else {
            return Err(ForeignCallbackFailure::new(
                "JavaScript widget callbacks were released",
            ));
        };
        let custom_bounds = callbacks
            .call_arrange(bounds, &child_sizes)
            .map_err(foreign_js_error)?;
        if let Some(custom_bounds) = custom_bounds {
            if custom_bounds.len() != ctx.child_count() {
                return Err(ForeignCallbackFailure::new(format!(
                    "arrange returned {} child bounds for {} children",
                    custom_bounds.len(),
                    ctx.child_count()
                )));
            }
            for (index, bounds) in custom_bounds.into_iter().enumerate() {
                ctx.arrange_child(index, bounds);
            }
        } else {
            let mut y = bounds.y();
            for index in 0..ctx.child_count() {
                let size = child_sizes.get(index).copied().unwrap_or(Size::ZERO);
                ctx.arrange_child(index, Rect::new(bounds.x(), y, bounds.width(), size.height));
                y += size.height;
            }
        }
        Ok(())
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
                    .map_err(ForeignCallbackFailure::from)?;
                for index in 0..ctx.child_count() {
                    ctx.paint_child(index);
                }
                Ok(())
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
        let mut included_children = vec![false; ctx.child_count()];
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
                        if let Some(included) = included_children.get_mut(index) {
                            *included = true;
                        }
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
        for (index, included) in included_children.into_iter().enumerate() {
            if !included {
                ctx.semantics_child(index);
            }
        }
        Ok(())
    }
}

impl JsObjectCallbacks {
    pub(crate) fn call_event(&self, event: &Event, context: JsEventContext) -> Result<bool> {
        let Some(event_fn) = self.method("event")? else {
            return Ok(false);
        };
        // Anything but `true` or `false` (such as no return value) means the
        // event was not handled.
        let handled = self.call(
            &event_fn,
            &[
                JsEvent::from_binding(BindingEvent::from(event)).into(),
                context.into(),
            ],
        )?;
        Ok(handled.as_bool().unwrap_or(false))
    }

    pub(crate) fn call_measure(
        &self,
        constraints: Constraints,
        child_sizes: &[Size],
    ) -> Result<Size> {
        if let Some(measure) = self.method("measureWithChildren")? {
            let size = self.call(
                &measure,
                &[
                    JsConstraints::from(constraints).into(),
                    js_sizes(child_sizes).into(),
                ],
            )?;
            return Ok(Size::from(class_value::<JsSize>(&size)?));
        }
        let Some(measure) = self.method("measure")? else {
            let natural = child_sizes.iter().fold(Size::ZERO, |size, child| {
                Size::new(size.width.max(child.width), size.height + child.height)
            });
            return Ok(constraints.clamp(natural));
        };
        let size = self.call(&measure, &[JsConstraints::from(constraints).into()])?;
        Ok(Size::from(class_value::<JsSize>(&size)?))
    }

    pub(crate) fn call_arrange(
        &self,
        bounds: Rect,
        child_sizes: &[Size],
    ) -> Result<Option<Vec<Rect>>> {
        let Some(arrange) = self.method("arrange")? else {
            return Ok(None);
        };
        let bounds = self.call(
            &arrange,
            &[JsRect::from(bounds).into(), js_sizes(child_sizes).into()],
        )?;
        Ok(Some(
            class_array::<JsRect>(&bounds)?
                .into_iter()
                .map(Rect::from)
                .collect(),
        ))
    }

    pub(crate) fn call_paint(
        &self,
        bounds: Rect,
    ) -> Result<(Vec<PaintCommand>, Vec<PendingPaintImage>)> {
        let Some(paint_fn) = self.method("paint")? else {
            return Ok((Vec::new(), Vec::new()));
        };
        let paint = JsPaint::new(bounds);
        self.call(&paint_fn, &[paint.clone().into()])?;
        let commands = paint.finish().map_err(value_error)?;
        Ok((commands, paint.take_images()))
    }

    pub(crate) fn call_semantics(
        &self,
        widget_id: WidgetId,
        bounds: Rect,
        focused: bool,
        child_count: usize,
    ) -> Result<Option<Vec<JsSemanticsCommand>>> {
        let Some(semantics_fn) = self.method("semantics")? else {
            return Ok(None);
        };
        let semantics = JsSemantics::new(widget_id, bounds, focused, child_count);
        self.call(&semantics_fn, &[semantics.clone().into()])?;
        Ok(Some(semantics.take_commands()))
    }

    pub(crate) fn name(&self) -> Result<Option<String>> {
        let name = Reflect::get(&self.object, &JsValue::from_str("name"))?;
        if name.is_undefined() || name.is_null() {
            return Ok(None);
        }
        name.as_string()
            .map(Some)
            .ok_or_else(|| invalid_arg("name must be a string"))
    }
}

fn js_sizes(sizes: &[Size]) -> Array {
    to_array(sizes.iter().copied().map(JsSize::from))
}
