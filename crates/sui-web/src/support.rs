//! Argument conversions and name tables shared by the binding modules.

use crate::prelude::*;

/// Declare an argument type passed as a plain JavaScript value and
/// converted in the function body, where conversion errors can be returned.
macro_rules! js_value_arg {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        pub struct $name(pub(crate) JsValue);

        impl wasm_bindgen::describe::WasmDescribe for $name {
            fn describe() {
                <JsValue as wasm_bindgen::describe::WasmDescribe>::describe()
            }
        }

        impl wasm_bindgen::convert::FromWasmAbi for $name {
            type Abi = <JsValue as wasm_bindgen::convert::FromWasmAbi>::Abi;

            unsafe fn from_abi(js: Self::Abi) -> Self {
                // SAFETY: forwarded unchanged to `JsValue`'s conversion.
                Self(unsafe { <JsValue as wasm_bindgen::convert::FromWasmAbi>::from_abi(js) })
            }
        }

        impl wasm_bindgen::convert::OptionFromWasmAbi for $name {
            fn is_none(abi: &Self::Abi) -> bool {
                is_undefined_or_null(abi)
            }
        }
    };
}

js_value_arg!(
    /// A `State | string | number | boolean` argument.
    JsBindingTextArg
);
js_value_arg!(
    /// A `State | boolean | number` argument.
    JsBindingBoolArg
);
js_value_arg!(
    /// A `State | number | boolean` argument.
    JsBindingNumberArg
);

pub(crate) fn binding_text_from_js(value: JsBindingTextArg) -> Result<BindingText> {
    binding_text(&value.0)
}

pub(crate) fn binding_bool_from_js(value: JsBindingBoolArg) -> Result<BindingBool> {
    binding_bool(&value.0)
}

pub(crate) fn binding_number_from_js(value: JsBindingNumberArg) -> Result<BindingNumber> {
    binding_number(&value.0)
}

/// An instance of the exported class `T`, borrowed rather than moved out of
/// its JavaScript object. Use it for optional class arguments
/// (`Option<ClassArg<JsColor>>`), which wasm-bindgen cannot borrow.
pub struct ClassArg<T>(pub(crate) JsValue, std::marker::PhantomData<T>);

impl<T> wasm_bindgen::describe::WasmDescribe for ClassArg<T> {
    fn describe() {
        <JsValue as wasm_bindgen::describe::WasmDescribe>::describe()
    }
}

impl<T> wasm_bindgen::convert::FromWasmAbi for ClassArg<T> {
    type Abi = <JsValue as wasm_bindgen::convert::FromWasmAbi>::Abi;

    unsafe fn from_abi(js: Self::Abi) -> Self {
        // SAFETY: forwarded unchanged to `JsValue`'s conversion.
        Self(
            unsafe { <JsValue as wasm_bindgen::convert::FromWasmAbi>::from_abi(js) },
            std::marker::PhantomData,
        )
    }
}

impl<T> wasm_bindgen::convert::OptionFromWasmAbi for ClassArg<T> {
    fn is_none(abi: &Self::Abi) -> bool {
        is_undefined_or_null(abi)
    }
}

/// Like napi, optional arguments treat `null` as missing, not only
/// `undefined`.
fn is_undefined_or_null(abi: &<JsValue as wasm_bindgen::convert::FromWasmAbi>::Abi) -> bool {
    <JsValue as wasm_bindgen::convert::OptionFromWasmAbi>::is_none(abi)
        || *abi == wasm_bindgen::convert::IntoWasmAbi::into_abi(&JsValue::NULL)
}

impl<T: JsClass> ClassArg<T> {
    /// Borrow the Rust value; fails when the argument is another type.
    pub(crate) fn borrow(&self) -> Result<T::Anchor> {
        class_ref::<T>(&self.0)
    }
}

impl<T: JsClass + Clone> ClassArg<T> {
    /// A copy of the Rust value.
    pub(crate) fn value(&self) -> Result<T> {
        class_value::<T>(&self.0)
    }
}

pub(crate) fn js_image_fit(value: &str) -> Result<BindingImageFit> {
    match value {
        "fill" => Ok(BindingImageFit::Fill),
        "contain" => Ok(BindingImageFit::Contain),
        "cover" => Ok(BindingImageFit::Cover),
        "none" => Ok(BindingImageFit::None),
        _ => Err(invalid_arg(
            "image fit must be 'fill', 'contain', 'cover', or 'none'",
        )),
    }
}

pub(crate) fn icon_glyph_from_js(value: &str) -> Result<sui_crate::IconGlyph> {
    binding_icon_glyph_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown icon glyph '{value}'")))
}

pub(crate) fn semantic_tone_from_js(value: &str) -> Result<sui_crate::SemanticTone> {
    binding_semantic_tone_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown semantic tone '{value}'")))
}

pub(crate) fn table_column_alignment_from_js(
    value: &str,
) -> Result<sui_crate::TableColumnAlignment> {
    binding_table_column_alignment_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown table column alignment '{value}'")))
}

pub(crate) fn surface_role_from_js(value: &str) -> Result<sui_crate::SurfaceRole> {
    binding_surface_role_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown surface role '{value}'")))
}

pub(crate) fn surface_border_from_js(value: &str) -> Result<sui_crate::SurfaceBorder> {
    binding_surface_border_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown surface border '{value}'")))
}

pub(crate) fn surface_elevation_from_js(value: &str) -> Result<sui_crate::SurfaceElevation> {
    binding_surface_elevation_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown surface elevation '{value}'")))
}

pub(crate) fn alignment_from_js(value: &str) -> Result<sui_crate::Alignment> {
    binding_alignment_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown alignment '{value}'")))
}

pub(crate) fn tooltip_placement_from_js(value: &str) -> Result<sui_crate::TooltipPlacement> {
    binding_tooltip_placement_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown tooltip placement '{value}'")))
}

pub(crate) fn semantics_role_from_js(value: &str) -> Result<sui_crate::SemanticsRole> {
    binding_semantics_role_from_name(value)
        .ok_or_else(|| invalid_arg(format!("unknown semantics role '{value}'")))
}

pub(crate) fn js_axis(value: &str) -> Result<Axis> {
    match value {
        "horizontal" | "x" | "row" => Ok(Axis::Horizontal),
        "vertical" | "y" | "column" => Ok(Axis::Vertical),
        _ => Err(invalid_arg("axis must be 'horizontal' or 'vertical'")),
    }
}

pub(crate) fn scroll_axes_from_js(value: &str) -> Result<BindingScrollAxes> {
    match value {
        "vertical" | "y" | "column" => Ok(BindingScrollAxes::Vertical),
        "horizontal" | "x" | "row" => Ok(BindingScrollAxes::Horizontal),
        "both" | "xy" | "all" => Ok(BindingScrollAxes::Both),
        _ => Err(invalid_arg(
            "scroll axes must be 'vertical', 'horizontal', or 'both'",
        )),
    }
}

pub(crate) fn pointer_event_kind_from_js(value: &str) -> Result<BindingPointerEventKind> {
    match value {
        "down" => Ok(BindingPointerEventKind::Down),
        "up" => Ok(BindingPointerEventKind::Up),
        "move" => Ok(BindingPointerEventKind::Move),
        "scroll" => Ok(BindingPointerEventKind::Scroll),
        "enter" => Ok(BindingPointerEventKind::Enter),
        "leave" => Ok(BindingPointerEventKind::Leave),
        "cancel" => Ok(BindingPointerEventKind::Cancel),
        _ => Err(invalid_arg(
            "pointer event kind must be 'down', 'up', 'move', 'scroll', 'enter', 'leave', or 'cancel'",
        )),
    }
}

pub(crate) fn pointer_event_kind_name(value: BindingPointerEventKind) -> &'static str {
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

pub(crate) fn pointer_button_from_js(value: &str) -> Result<BindingPointerButton> {
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
                    .map_err(|_| invalid_arg("other pointer button must fit in u16"));
            }
            Err(invalid_arg(
                "pointer button must be 'primary', 'secondary', 'middle', 'back', 'forward', or 'other:<u16>'",
            ))
        }
    }
}

pub(crate) fn pointer_button_name(value: BindingPointerButton) -> String {
    match value {
        BindingPointerButton::Primary => "primary".to_owned(),
        BindingPointerButton::Secondary => "secondary".to_owned(),
        BindingPointerButton::Middle => "middle".to_owned(),
        BindingPointerButton::Back => "back".to_owned(),
        BindingPointerButton::Forward => "forward".to_owned(),
        BindingPointerButton::Other(button) => format!("other:{button}"),
    }
}

pub(crate) fn pointer_kind_from_js(value: &str) -> Result<BindingPointerKind> {
    match value {
        "mouse" => Ok(BindingPointerKind::Mouse),
        "touch" => Ok(BindingPointerKind::Touch),
        "pen" => Ok(BindingPointerKind::Pen),
        "unknown" => Ok(BindingPointerKind::Unknown),
        _ => Err(invalid_arg(
            "pointerKind must be 'mouse', 'touch', 'pen', or 'unknown'",
        )),
    }
}

pub(crate) fn pointer_kind_name(value: BindingPointerKind) -> &'static str {
    match value {
        BindingPointerKind::Mouse => "mouse",
        BindingPointerKind::Touch => "touch",
        BindingPointerKind::Pen => "pen",
        BindingPointerKind::Unknown => "unknown",
    }
}

pub(crate) fn checked_buttons(value: u32) -> Result<u8> {
    u8::try_from(value).map_err(|_| invalid_arg("buttons must fit in an unsigned byte"))
}

pub(crate) fn key_state_from_js(value: &str) -> Result<BindingKeyState> {
    match value {
        "pressed" | "down" => Ok(BindingKeyState::Pressed),
        "released" | "up" => Ok(BindingKeyState::Released),
        _ => Err(invalid_arg("key state must be 'pressed' or 'released'")),
    }
}

pub(crate) fn key_state_name(value: BindingKeyState) -> &'static str {
    match value {
        BindingKeyState::Pressed => "pressed",
        BindingKeyState::Released => "released",
    }
}

pub(crate) fn ime_event_kind_name(value: &BindingImeEvent) -> &'static str {
    match value {
        BindingImeEvent::CompositionStart => "compositionStart",
        BindingImeEvent::CompositionUpdate { .. } => "compositionUpdate",
        BindingImeEvent::CompositionCommit { .. } => "compositionCommit",
        BindingImeEvent::CompositionEnd => "compositionEnd",
    }
}

pub(crate) fn window_event_kind_name(value: &BindingWindowEvent) -> &'static str {
    match value {
        BindingWindowEvent::CloseRequested => "closeRequested",
        BindingWindowEvent::Resized(_) => "resized",
        BindingWindowEvent::Moved(_) => "moved",
        BindingWindowEvent::ScaleFactorChanged { .. } => "scaleFactorChanged",
        BindingWindowEvent::Focused(_) => "focused",
        BindingWindowEvent::Occluded(_) => "occluded",
        BindingWindowEvent::SafeAreaChanged { .. } => "safeAreaChanged",
        BindingWindowEvent::ExternalFileHovered(_) => "externalFileHovered",
        BindingWindowEvent::ExternalFileHoverCancelled => "externalFileHoverCancelled",
        BindingWindowEvent::ExternalFileDropped(_) => "externalFileDropped",
        BindingWindowEvent::RedrawRequested => "redrawRequested",
    }
}

pub(crate) fn color_space_from_js(value: Option<String>) -> Result<ColorSpace> {
    match value.as_deref().unwrap_or("srgb") {
        "srgb" | "sRGB" => Ok(ColorSpace::Srgb),
        "linear-srgb" | "linear_srgb" => Ok(ColorSpace::LinearSrgb),
        "display-p3" | "display_p3" => Ok(ColorSpace::DisplayP3),
        "linear-display-p3" | "linear_display_p3" => Ok(ColorSpace::LinearDisplayP3),
        _ => Err(invalid_arg(
            "colorSpace must be 'srgb', 'linear-srgb', 'display-p3', or 'linear-display-p3'",
        )),
    }
}

pub(crate) fn js_radii(radius: Option<f64>) -> [f32; 4] {
    [radius.unwrap_or(0.0) as f32; 4]
}

pub(crate) fn js_four_points(points: &Array) -> Result<[sui_crate::Point; 4]> {
    if points.length() != 4 {
        return Err(invalid_arg("points must contain exactly four Point values"));
    }
    let mut extracted = [sui_crate::Point::ZERO; 4];
    for (slot, point) in extracted.iter_mut().zip(points.iter()) {
        *slot = (*class_ref::<JsPoint>(&point)?).into();
    }
    Ok(extracted)
}

/// A semantics value given as a string, number, or boolean; numbers become
/// ranges when both bounds are given.
pub(crate) fn js_semantics_value(
    value: Option<JsValue>,
    min_value: Option<f64>,
    max_value: Option<f64>,
) -> Option<SemanticsValue> {
    let value = value.filter(|value| !value.is_undefined() && !value.is_null())?;
    if let Some(text) = value.as_string() {
        Some(SemanticsValue::Text(text))
    } else if let Some(value) = value.as_f64() {
        if let (Some(min), Some(max)) = (min_value, max_value) {
            Some(SemanticsValue::Range { value, min, max })
        } else {
            Some(SemanticsValue::Number(value))
        }
    } else {
        value
            .as_bool()
            .map(|value| SemanticsValue::Text(value.to_string()))
    }
}

pub(crate) fn js_toggle_state(value: Option<&str>) -> Result<Option<ToggleState>> {
    let Some(value) = value else {
        return Ok(None);
    };
    binding_toggle_state_from_name(value)
        .map(Some)
        .ok_or_else(|| invalid_arg("checked must be 'checked', 'unchecked', or 'mixed'"))
}

pub(crate) fn js_text_style(
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
            u16::try_from(weight).map_err(|_| invalid_arg("font weight must fit in u16"))?,
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

pub(crate) fn font_style_from_js(value: &str) -> Result<FontStyle> {
    match value {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(invalid_arg(
            "font style must be 'normal', 'italic', or 'oblique'",
        )),
    }
}

pub(crate) fn font_stretch_from_js(value: &str) -> Result<FontStretch> {
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
        _ => Err(invalid_arg("invalid font stretch")),
    }
}

pub(crate) fn native_backend_from_js(value: &str) -> Result<NativeGraphicsBackend> {
    match value {
        "cpu" => Ok(NativeGraphicsBackend::Cpu),
        "wgpu" => Ok(NativeGraphicsBackend::Wgpu),
        "webgpu" | "web-gpu" | "web_gpu" => Ok(NativeGraphicsBackend::WebGpu),
        "d3d12" => Ok(NativeGraphicsBackend::D3d12),
        "metal" => Ok(NativeGraphicsBackend::Metal),
        "vulkan" => Ok(NativeGraphicsBackend::Vulkan),
        "opengl" | "open-gl" | "open_gl" => Ok(NativeGraphicsBackend::OpenGl),
        "unknown" => Ok(NativeGraphicsBackend::Unknown),
        _ => Err(invalid_arg(
            "backend must be 'cpu', 'wgpu', 'webgpu', 'd3d12', 'metal', 'vulkan', 'opengl', or 'unknown'",
        )),
    }
}

pub(crate) fn native_backend_name(value: NativeGraphicsBackend) -> &'static str {
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

pub(crate) fn interop_tier_from_js(value: &str) -> Result<RendererInteropTier> {
    match value {
        "cpuUpload" | "cpu_upload" | "cpu-upload" => Ok(RendererInteropTier::CpuUpload),
        "sharedTexture" | "shared_texture" | "shared-texture" => {
            Ok(RendererInteropTier::SharedTexture)
        }
        "sharedRenderTarget" | "shared_render_target" | "shared-render-target" => {
            Ok(RendererInteropTier::SharedRenderTarget)
        }
        _ => Err(invalid_arg(
            "tier must be 'cpuUpload', 'sharedTexture', or 'sharedRenderTarget'",
        )),
    }
}

pub(crate) fn interop_tier_name(value: RendererInteropTier) -> &'static str {
    match value {
        RendererInteropTier::CpuUpload => "cpuUpload",
        RendererInteropTier::SharedTexture => "sharedTexture",
        RendererInteropTier::SharedRenderTarget => "sharedRenderTarget",
    }
}

pub(crate) fn external_texture_format_from_js(value: &str) -> Result<ExternalTextureFormat> {
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
        _ => Err(invalid_arg(
            "format must be 'rgba8unorm', 'bgra8unorm', or 'rgba16float'",
        )),
    }
}

pub(crate) fn parse_u64_string(value: &str, label: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|_| invalid_arg(format!("{label} must be an unsigned 64-bit integer string")))
}

/// The binding widgets of an array of `Widget`s.
pub(crate) fn extract_binding_widgets(children: &Array) -> Result<Vec<BindingWidget>> {
    children
        .iter()
        .map(|child| class_ref::<JsWidget>(&child)?.binding_widget())
        .collect()
}

/// The `inner` values of an array of exported class instances.
macro_rules! extract_inner {
    ($($name:ident: $class:ty => $inner:ty),* $(,)?) => {
        $(pub(crate) fn $name(items: &Array) -> Result<Vec<$inner>> {
            items
                .iter()
                .map(|item| Ok(class_ref::<$class>(&item)?.inner.clone()))
                .collect()
        })*
    };
}

extract_inner!(
    extract_text_spans: JsTextSpan => BindingTextSpan,
    extract_status_bar_segments: JsStatusBarSegment => BindingStatusBarSegment,
    extract_segmented_control_items: JsSegmentedControlItem => BindingSegmentedControlItem,
    extract_table_columns: JsTableColumn => BindingTableColumn,
    extract_table_rows: JsTableRow => BindingTableRow,
    extract_tree_items: JsTreeItem => BindingTreeItem,
    extract_layer_list_items: JsLayerListItem => BindingLayerListItem,
    extract_menu_items: JsMenuItem => BindingMenuItem,
    extract_tool_palette_items: JsToolPaletteItem => BindingToolPaletteItem,
    extract_color_palette_swatches: JsColorPaletteSwatch => BindingColorPaletteSwatch,
);

/// Grid tracks given as numbers (fixed sizes) or strings such as `"auto"`,
/// `"1fr"`, or `"minmax(120, 1fr)"`.
pub(crate) fn js_grid_tracks(tracks: &Array) -> Result<Vec<GridTrack>> {
    tracks
        .iter()
        .map(|track| {
            if let Some(points) = track.as_f64() {
                Ok(GridTrack::fixed(points.max(0.0) as f32))
            } else if let Some(name) = track.as_string() {
                binding_grid_track_from_name(&name)
                    .ok_or_else(|| invalid_arg(format!("unknown grid track '{name}'")))
            } else {
                Err(invalid_arg("grid tracks must be numbers or strings"))
            }
        })
        .collect()
}

/// List items given as strings or `ListItem` objects.
pub(crate) fn extract_list_items(items: &Array) -> Result<Vec<BindingListItem>> {
    items
        .iter()
        .map(|item| {
            if let Some(label) = item.as_string() {
                Ok(BindingListItem::new(label))
            } else if is_instance(&item, "ListItem") {
                Ok(class_ref::<JsListItem>(&item)?.inner.clone())
            } else {
                Err(invalid_arg(
                    "list items must be strings or ListItem objects",
                ))
            }
        })
        .collect()
}

pub(crate) fn value_error(error: PaintValidationError) -> JsValue {
    invalid_arg(error)
}

pub(crate) fn external_texture_error(error: ExternalTextureValidationError) -> JsValue {
    invalid_arg(error)
}

pub(crate) fn foreign_js_error(error: JsValue) -> ForeignCallbackFailure {
    callback_failure(error)
}

pub(crate) fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) trait PaintCommandBuilderIntrospection {
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
