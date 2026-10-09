//! State, themes, windows, and apps: running in the browser and without a
//! platform for tests.

use crate::prelude::*;

#[wasm_bindgen(js_name = State)]
#[derive(Clone)]
pub struct JsState {
    pub(crate) inner: BindingState,
}

js_class!(JsState => "State");

#[wasm_bindgen(js_class = State)]
impl JsState {
    #[wasm_bindgen(constructor)]
    pub fn new(value: JsValue) -> Result<Self> {
        Ok(Self {
            inner: BindingState::new(binding_value_from_js(&value)?),
        })
    }

    #[wasm_bindgen]
    pub fn get(&self) -> JsValue {
        binding_value_to_js(self.inner.get())
    }

    #[wasm_bindgen]
    pub fn set(&self, value: JsValue) -> Result<()> {
        self.inner.set(binding_value_from_js(&value)?);
        Ok(())
    }

    #[wasm_bindgen(getter = text)]
    pub fn text(&self) -> String {
        self.inner.label_text()
    }

    #[wasm_bindgen]
    pub fn select(&self, selector: Function) -> Result<Self> {
        let initial = selector.call1(&JsValue::UNDEFINED, &self.get())?;
        let derived = BindingState::new(binding_value_from_js(&initial)?);
        let derived_for_observer = derived.clone();
        let selector = Local(selector);
        let subscription = self.inner.observe(move |value| {
            if let Ok(selected) = selector.call1(&JsValue::UNDEFINED, &binding_value_to_js(value))
                && let Ok(selected) = binding_value_from_js(&selected)
            {
                derived_for_observer.set(selected);
            }
        });
        derived.retain_subscription(subscription);
        Ok(Self { inner: derived })
    }

    #[wasm_bindgen]
    pub fn watch(&self, callback: Function) -> JsStateSubscription {
        let callback = Local(callback);
        JsStateSubscription {
            inner: Mutex::new(Some(self.inner.observe(move |value| {
                let _ = callback.call1(&JsValue::UNDEFINED, &binding_value_to_js(value));
            }))),
        }
    }
}

#[wasm_bindgen(js_name = StateSubscription)]
pub struct JsStateSubscription {
    pub(crate) inner: Mutex<Option<BindingStateSubscription>>,
}

js_class!(JsStateSubscription => "StateSubscription");

#[wasm_bindgen(js_class = StateSubscription)]
impl JsStateSubscription {
    #[wasm_bindgen]
    pub fn unsubscribe(&self) -> bool {
        recover_lock(&self.inner)
            .take()
            .is_some_and(|mut subscription| subscription.unsubscribe())
    }
}

#[wasm_bindgen(js_name = Theme)]
#[derive(Clone)]
pub struct JsTheme {
    pub(crate) inner: BindingTheme,
}

js_class!(JsTheme => "Theme");

#[wasm_bindgen(js_class = Theme)]
impl JsTheme {
    #[wasm_bindgen(constructor)]
    pub fn new(preset: Option<String>) -> Result<Self> {
        BindingTheme::preset(preset.as_deref().unwrap_or("light"))
            .map(|inner| Self { inner })
            .map_err(invalid_arg)
    }

    #[wasm_bindgen]
    pub fn light() -> Result<Self> {
        Self::new(Some("light".to_owned()))
    }

    #[wasm_bindgen]
    pub fn dark() -> Result<Self> {
        Self::new(Some("dark".to_owned()))
    }

    #[wasm_bindgen]
    pub fn neutral() -> Result<Self> {
        Self::new(Some("neutral".to_owned()))
    }

    #[wasm_bindgen(js_name = neutralDark)]
    pub fn neutral_dark() -> Result<Self> {
        Self::new(Some("neutral-dark".to_owned()))
    }

    #[wasm_bindgen(js_name = highContrast)]
    pub fn high_contrast() -> Result<Self> {
        Self::new(Some("high-contrast".to_owned()))
    }

    #[wasm_bindgen]
    pub fn oled() -> Result<Self> {
        Self::new(Some("oled".to_owned()))
    }

    #[wasm_bindgen(js_name = setPreset)]
    pub fn set_preset(&self, preset: String) -> Result<()> {
        self.inner.set_preset(&preset).map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = setAccent)]
    pub fn set_accent(&self, color: &JsColor) {
        self.inner.set_accent((*color).into());
    }

    #[wasm_bindgen(js_name = setControlSize)]
    pub fn set_control_size(&self, size: String) -> Result<()> {
        self.inner.set_control_size(&size).map_err(invalid_arg)
    }

    #[wasm_bindgen]
    pub fn color(&self, name: String) -> Result<JsColor> {
        self.inner
            .color(&name)
            .map(JsColor::from)
            .map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = setColor)]
    pub fn set_color(&self, name: String, color: &JsColor) -> Result<()> {
        self.inner
            .set_color(&name, (*color).into())
            .map_err(invalid_arg)
    }

    #[wasm_bindgen]
    pub fn number(&self, name: String) -> Result<f64> {
        self.inner.number(&name).map(f64::from).map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = setNumber)]
    pub fn set_number(&self, name: String, value: f64) -> Result<()> {
        self.inner
            .set_number(&name, value as f32)
            .map_err(invalid_arg)
    }

    #[wasm_bindgen(getter = accent)]
    pub fn accent(&self) -> JsColor {
        self.inner.accent().into()
    }
}

#[wasm_bindgen(js_name = Window)]
#[derive(Clone)]
pub struct JsWindow {
    pub(crate) title: String,
    pub(crate) root: Option<BindingWidget>,
    pub(crate) initial_size: Option<Size>,
    pub(crate) initial_position: Option<Point>,
    pub(crate) icon_svg: Option<Vec<u8>>,
    pub(crate) use_default_icon: bool,
}

js_class!(JsWindow => "Window");

#[wasm_bindgen(js_class = Window)]
impl JsWindow {
    #[wasm_bindgen(constructor)]
    pub fn new(title: String) -> Self {
        Self {
            title,
            root: None,
            initial_size: None,
            initial_position: None,
            icon_svg: None,
            use_default_icon: true,
        }
    }

    #[wasm_bindgen]
    pub fn root(&mut self, widget: &JsWidget) -> Result<()> {
        self.root = Some(widget.binding_widget()?);
        Ok(())
    }

    #[wasm_bindgen(js_name = setInitialSize)]
    pub fn set_initial_size(&mut self, size: &JsSize) {
        self.initial_size = Some((*size).into());
    }

    #[wasm_bindgen(js_name = setInitialPosition)]
    pub fn set_initial_position(&mut self, position: &JsPoint) {
        self.initial_position = Some((*position).into());
    }

    #[wasm_bindgen(js_name = setIconSvg)]
    pub fn set_icon_svg(&mut self, svg: Vec<u8>) {
        self.icon_svg = Some(svg);
        self.use_default_icon = true;
    }

    #[wasm_bindgen(js_name = removeIcon)]
    pub fn remove_icon(&mut self) {
        self.icon_svg = None;
        self.use_default_icon = false;
    }
}

impl JsWindow {
    pub(crate) fn to_binding(&self) -> Result<BindingWindow> {
        let root = self
            .root
            .clone()
            .ok_or_else(|| invalid_arg("window root has not been set"))?;
        let mut window = BindingWindow::new(self.title.clone(), root);
        if let Some(size) = self.initial_size {
            window = window.with_initial_size(size);
        }
        if let Some(position) = self.initial_position {
            window = window.with_initial_position(position);
        }
        if let Some(svg) = &self.icon_svg {
            window = window.with_icon_svg(svg.clone());
        } else if !self.use_default_icon {
            window = window.without_icon();
        }
        Ok(window)
    }
}

js_object! {
    /// Object-literal configuration for renderer and HDR output policy.
    pub struct JsRenderOptions {
        feathering: Option<bool> = "feathering",
        feather_width: Option<f64> = "featherWidth",
        optical_text_alignment: Option<bool> = "opticalTextAlignment",
        output_color_primaries: Option<String> = "outputColorPrimaries",
        dynamic_range: Option<String> = "dynamicRange",
        tone_mapping: Option<String> = "toneMapping",
        color_management: Option<String> = "colorManagement",
        sdr_content_brightness_nits: Option<f64> = "sdrContentBrightnessNits",
        use_system_sdr_brightness: Option<bool> = "useSystemSdrBrightness",
    }
}

impl TryFrom<JsRenderOptions> for BindingRenderOptions {
    type Error = JsValue;

    fn try_from(value: JsRenderOptions) -> Result<Self> {
        BindingRenderOptions::new(
            value.feathering.unwrap_or(true),
            value.feather_width.unwrap_or(1.0) as f32,
            value.optical_text_alignment.unwrap_or(true),
            value.output_color_primaries.as_deref().unwrap_or("auto"),
            value.dynamic_range.as_deref().unwrap_or("auto"),
            value.tone_mapping.as_deref().unwrap_or("auto"),
            value.color_management.as_deref().unwrap_or("auto"),
            value.sdr_content_brightness_nits.unwrap_or(203.0) as f32,
            value.use_system_sdr_brightness.unwrap_or(true),
        )
        .map_err(invalid_arg)
    }
}

#[wasm_bindgen(js_name = App)]
pub struct JsApp {
    pub(crate) inner: Mutex<BindingApp>,
}

js_class!(JsApp => "App");

#[wasm_bindgen(js_class = App)]
impl JsApp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(BindingApp::new()),
        }
    }

    #[wasm_bindgen]
    pub fn window(&self, window: &JsWindow) -> Result<()> {
        recover_lock(&self.inner).push_window(window.to_binding()?);
        Ok(())
    }

    #[wasm_bindgen(js_name = configureRendering)]
    pub fn configure_rendering(&self, options: JsValue) -> Result<()> {
        let options = <JsRenderOptions as FromJs>::from_js(&options)?;
        recover_lock(&self.inner).set_render_options(options.try_into()?);
        Ok(())
    }

    #[wasm_bindgen(js_name = setTheme)]
    pub fn set_theme(&self, theme: &JsTheme) {
        recover_lock(&self.inner).set_theme(theme.inner.clone());
    }

    #[wasm_bindgen]
    pub fn on(&self, name: String, callback: Function) {
        recover_lock(&self.inner).on_message(name, js_message_action(callback));
    }

    #[wasm_bindgen]
    pub fn render(&self, index: Option<u32>) -> Result<JsRenderSnapshot> {
        recover_lock(&self.inner)
            .render_window(index.unwrap_or(0) as usize)
            .map(JsRenderSnapshot::from)
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn start(&self) -> Result<JsRunningApp> {
        recover_lock(&self.inner)
            .start()
            .map(JsRunningApp::new)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = rgbaImage)]
    pub fn rgba_image(&self, width: u32, height: u32, pixels: Vec<u8>) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_rgba_image(width, height, pixels)
            .map(|inner| JsImageHandle { inner })
            .map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = pngImage)]
    pub fn png_image(&self, png: Vec<u8>) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_png_image(png)
            .map(|inner| JsImageHandle { inner })
            .map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = pngFile)]
    pub fn png_file(&self, path: String) -> Result<JsImageHandle> {
        Err(no_file_system(&path, "pngImage"))
    }

    #[wasm_bindgen(js_name = svgImage)]
    pub fn svg_image(&self, svg: Vec<u8>) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_svg_image(svg)
            .map(|inner| JsImageHandle { inner })
            .map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = svgFile)]
    pub fn svg_file(&self, path: String) -> Result<JsImageHandle> {
        Err(no_file_system(&path, "svgImage"))
    }

    #[wasm_bindgen(js_name = svgImageAtSize)]
    pub fn svg_image_at_size(
        &self,
        width: u32,
        height: u32,
        svg: Vec<u8>,
    ) -> Result<JsImageHandle> {
        recover_lock(&self.inner)
            .register_svg_image_at_size(width, height, svg)
            .map(|inner| JsImageHandle { inner })
            .map_err(invalid_arg)
    }

    #[wasm_bindgen(js_name = svgFileAtSize)]
    pub fn svg_file_at_size(
        &self,
        _width: u32,
        _height: u32,
        path: String,
    ) -> Result<JsImageHandle> {
        Err(no_file_system(&path, "svgImageAtSize"))
    }

    #[wasm_bindgen(js_name = fontBytes)]
    pub fn font_bytes(&self, data: Vec<u8>) -> Result<JsFontHandle> {
        recover_lock(&self.inner)
            .register_font_bytes(data)
            .map(|inner| JsFontHandle { inner })
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = fontFile)]
    pub fn font_file(&self, path: String) -> Result<JsFontHandle> {
        Err(no_file_system(&path, "fontBytes"))
    }

    /// Open the app's first window on `canvas` (or the page's element with
    /// id `sui-main-canvas`). The browser's event loop runs the app; the
    /// JavaScript layer watches the returned app to learn when it exits.
    #[wasm_bindgen(js_name = startWeb)]
    pub fn start_web(&self, canvas: Option<web_sys::HtmlCanvasElement>) -> Result<JsWebApp> {
        let status = Rc::new(RefCell::new(WebAppStatus::Running));
        let status_on_exit = Rc::clone(&status);
        let mut ui = None;
        recover_lock(&self.inner)
            .start_web_with_handle(
                canvas,
                |handle| ui = Some(handle),
                move |result| {
                    *status_on_exit.borrow_mut() = match result {
                        Ok(()) => WebAppStatus::Exited,
                        Err(error) => WebAppStatus::Failed(error),
                    };
                },
            )
            .map_err(runtime_error)?;
        let ui = ui.ok_or_else(|| runtime_error("the app did not provide a UI handle"))?;
        Ok(JsWebApp { status, ui })
    }

    #[wasm_bindgen(getter = windowCount)]
    pub fn window_count(&self) -> u32 {
        recover_lock(&self.inner).window_count() as u32
    }

    #[wasm_bindgen(getter = imageResourceCount)]
    pub fn image_resource_count(&self) -> u32 {
        recover_lock(&self.inner).image_resource_count() as u32
    }

    #[wasm_bindgen(getter = fontResourceCount)]
    pub fn font_resource_count(&self) -> u32 {
        recover_lock(&self.inner).font_resource_count() as u32
    }
}

/// The error for reading files, which browsers do not allow.
fn no_file_system(path: &str, alternative: &str) -> JsValue {
    runtime_error(format!(
        "cannot read '{path}': there is no file system in the browser; fetch the bytes and use \
         {alternative}()"
    ))
}

pub(crate) enum WebAppStatus {
    Running,
    Exited,
    Failed(String),
}

/// A started browser app. The browser's event loop runs it; `pump` reports
/// whether it still runs, so the shared JavaScript `App.run()` can watch it
/// the way it drives the Node addon. It is not exported.
#[wasm_bindgen(js_name = WebApp)]
pub struct JsWebApp {
    pub(crate) status: Rc<RefCell<WebAppStatus>>,
    pub(crate) ui: BindingUiHandle,
}

#[wasm_bindgen(js_class = WebApp)]
impl JsWebApp {
    /// Milliseconds until the next check while the app runs, or `-1` once
    /// it exited; throws the error it stopped with.
    #[wasm_bindgen]
    pub fn pump(&self, _timeout_ms: Option<f64>) -> Result<f64> {
        match &*self.status.borrow() {
            WebAppStatus::Running => Ok(Self::CHECK_INTERVAL_MS),
            WebAppStatus::Exited => Ok(-1.0),
            WebAppStatus::Failed(error) => Err(runtime_error(error)),
        }
    }

    /// The longest wait between checks.
    #[wasm_bindgen(getter = maxTurnWaitMs)]
    pub fn max_turn_wait_ms(&self) -> f64 {
        Self::CHECK_INTERVAL_MS
    }

    /// The handle for requesting exit and emitting messages.
    #[wasm_bindgen(getter = uiHandle)]
    pub fn ui_handle(&self) -> JsUiHandle {
        JsUiHandle {
            inner: self.ui.clone(),
        }
    }
}

impl JsWebApp {
    const CHECK_INTERVAL_MS: f64 = 100.0;
}

impl Default for JsApp {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_name = WindowHandle)]
#[derive(Debug, Clone, Copy)]
pub struct JsWindowHandle {
    pub(crate) inner: BindingWindowId,
}

js_class!(JsWindowHandle => "WindowHandle");

#[wasm_bindgen(js_class = WindowHandle)]
impl JsWindowHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> Result<Self> {
        let id = id.parse::<u64>().map_err(invalid_arg)?;
        Ok(Self {
            inner: BindingWindowId::new(id),
        })
    }

    #[wasm_bindgen(getter = id)]
    pub fn id(&self) -> String {
        self.inner.get().to_string()
    }
}

impl From<BindingWindowId> for JsWindowHandle {
    fn from(value: BindingWindowId) -> Self {
        Self { inner: value }
    }
}

#[wasm_bindgen(js_name = UiHandle)]
#[derive(Clone)]
pub struct JsUiHandle {
    pub(crate) inner: BindingUiHandle,
}

js_class!(JsUiHandle => "UiHandle");

js_object! {
    /// A named group of extensions for a file dialog, such as
    /// `{ name: "Images", extensions: ["png", "jpg"] }`.
    pub struct JsFileDialogFilter {
        name: String = "name",
        extensions: Vec<String> = "extensions",
    }
}

js_object! {
    pub struct JsFileDialogOptions {
        /// `"open"`, `"open-multiple"`, `"save"`, `"folder"`, or `"folders"`.
        mode: Option<String> = "mode",
        title: Option<String> = "title",
        filters: Option<Vec<JsFileDialogFilter>> = "filters",
        directory: Option<String> = "directory",
        /// Initial file name for save dialogs.
        name: Option<String> = "name",
    }
}

#[wasm_bindgen(js_class = UiHandle)]
impl JsUiHandle {
    /// Show a native file dialog. `onResult` runs with the chosen paths, or
    /// `null` when the user cancels, and an error message when the dialog
    /// could not be shown; browsers have no native file dialogs.
    #[wasm_bindgen(js_name = showFileDialog)]
    pub fn show_file_dialog(&self, options: JsValue, on_result: Function) -> Result<()> {
        let options = <Option<JsFileDialogOptions> as FromJs>::from_js(&options)?;
        let mode_name = options
            .as_ref()
            .and_then(|options| options.mode.as_deref())
            .unwrap_or("open");
        binding_file_dialog_mode_from_name(mode_name).ok_or_else(|| {
            invalid_arg(format!(
                "unknown file dialog mode '{mode_name}'; use 'open', 'open-multiple', 'save', \
                 'folder', or 'folders'"
            ))
        })?;
        on_result.call2(
            &JsValue::UNDEFINED,
            &JsValue::NULL,
            &JsValue::from_str("native file dialogs are not available in the browser"),
        )?;
        Ok(())
    }

    /// Ask the running app to exit. Returns `false` when it is not running.
    #[wasm_bindgen(js_name = requestExit)]
    pub fn request_exit(&self) -> bool {
        self.inner.request_exit()
    }

    #[wasm_bindgen]
    pub fn emit(&self, name: String, payload: JsValue) -> Result<bool> {
        Ok(self.inner.emit(name, binding_value_from_js(&payload)?))
    }
}

/// Text on the clipboard: the system clipboard on desktop hosts with a
/// display, otherwise process memory shared with the app's widgets.
#[wasm_bindgen(js_name = clipboardText)]
pub fn js_clipboard_text() -> Option<String> {
    binding_clipboard_text()
}

/// Override how much motion the app's animations use: `"full"`,
/// `"reduced"`, or `"off"`. `null` follows the system setting.
#[wasm_bindgen(js_name = setMotionPreference)]
pub fn js_set_motion_preference(preference: Option<String>) -> Result<()> {
    let preference = preference
        .map(|value| {
            binding_motion_preference_from_name(&value).ok_or_else(|| {
                invalid_arg(format!(
                    "unknown motion preference '{value}'; use 'full', 'reduced', or 'off'"
                ))
            })
        })
        .transpose()?;
    sui_crate::set_app_motion_preference(preference);
    Ok(())
}

#[wasm_bindgen(js_name = setClipboardText)]
pub fn js_set_clipboard_text(text: String) {
    binding_set_clipboard_text(&text);
}

impl From<BindingUiHandle> for JsUiHandle {
    fn from(value: BindingUiHandle) -> Self {
        Self { inner: value }
    }
}

#[wasm_bindgen(js_name = RunningApp)]
pub struct JsRunningApp {
    pub(crate) inner: Mutex<BindingRuntime>,
}

js_class!(JsRunningApp => "RunningApp");

js_object! {
    #[derive(Clone)]
    pub struct JsFrameTiming {
        phase: String = "phase",
        duration_ms: f64 = "durationMs",
    }
}

impl From<BindingFrameTiming> for JsFrameTiming {
    fn from(value: BindingFrameTiming) -> Self {
        Self {
            phase: value.phase,
            duration_ms: value.duration_ms,
        }
    }
}

js_object! {
    #[derive(Clone)]
    pub struct JsWidgetTiming {
        widget_id: String = "widgetId",
        widget_name: String = "widgetName",
        phase: String = "phase",
        duration_ms: f64 = "durationMs",
        calls: u32 = "calls",
    }
}

impl From<BindingWidgetTiming> for JsWidgetTiming {
    fn from(value: BindingWidgetTiming) -> Self {
        Self {
            widget_id: value.widget_id.to_string(),
            widget_name: value.widget_name,
            phase: value.phase,
            duration_ms: value.duration_ms,
            calls: value.calls as u32,
        }
    }
}

js_object! {
    #[derive(Clone)]
    pub struct JsEventRouteTrace {
        sequence: String = "sequence",
        event_kind: String = "eventKind",
        target_id: String = "targetId",
        path: Vec<String> = "path",
        handled: bool = "handled",
    }
}

impl From<BindingEventRouteTrace> for JsEventRouteTrace {
    fn from(value: BindingEventRouteTrace) -> Self {
        Self {
            sequence: value.sequence.to_string(),
            event_kind: value.event_kind,
            target_id: value.target_id.to_string(),
            path: value.path.into_iter().map(|id| id.to_string()).collect(),
            handled: value.handled,
        }
    }
}

js_object! {
    #[derive(Clone)]
    pub struct JsReactiveInvalidationTrace {
        widget_id: String = "widgetId",
        source_name: String = "sourceName",
        version: String = "version",
        kind: String = "kind",
        delivered: bool = "delivered",
    }
}

impl From<BindingReactiveInvalidationTrace> for JsReactiveInvalidationTrace {
    fn from(value: BindingReactiveInvalidationTrace) -> Self {
        Self {
            widget_id: value.widget_id.to_string(),
            source_name: value.source_name,
            version: value.version.to_string(),
            kind: value.kind,
            delivered: value.delivered,
        }
    }
}

js_object! {
    #[derive(Clone)]
    pub struct JsCommandDispatchTrace {
        sequence: String = "sequence",
        name: String = "name",
        payload_type: String = "payloadType",
        target: String = "target",
        delivery: String = "delivery",
        handlers: Vec<String> = "handlers",
        handled: bool = "handled",
        delivered: bool = "delivered",
    }
}

impl From<BindingCommandDispatchTrace> for JsCommandDispatchTrace {
    fn from(value: BindingCommandDispatchTrace) -> Self {
        Self {
            sequence: value.sequence.to_string(),
            name: value.name,
            payload_type: value.payload_type,
            target: value.target,
            delivery: value.delivery,
            handlers: value.handlers,
            handled: value.handled,
            delivered: value.delivered,
        }
    }
}

js_object! {
    #[derive(Clone)]
    pub struct JsInvalidationTrace {
        target: String = "target",
        kind: String = "kind",
        source: String = "source",
        reason: Option<String> = "reason",
    }
}

impl From<BindingInvalidationTrace> for JsInvalidationTrace {
    fn from(value: BindingInvalidationTrace) -> Self {
        Self {
            target: value.target,
            kind: value.kind,
            source: value.source,
            reason: value.reason,
        }
    }
}

js_object! {
    #[derive(Clone)]
    pub struct JsWidgetRebuildTrace {
        widget_id: String = "widgetId",
        widget_name: String = "widgetName",
        reason: String = "reason",
    }
}

impl From<BindingWidgetRebuildTrace> for JsWidgetRebuildTrace {
    fn from(value: BindingWidgetRebuildTrace) -> Self {
        Self {
            widget_id: value.widget_id.to_string(),
            widget_name: value.widget_name,
            reason: value.reason,
        }
    }
}

#[wasm_bindgen(js_name = InspectorSnapshot)]
pub struct JsInspectorSnapshot {
    #[wasm_bindgen(readonly, getter_with_clone, js_name = windowId)]
    pub window_id: String,
    #[wasm_bindgen(readonly, getter_with_clone, js_name = title)]
    pub title: String,
    #[wasm_bindgen(readonly, js_name = tracingEnabled)]
    pub tracing_enabled: bool,
    #[wasm_bindgen(readonly, getter_with_clone, js_name = focusedWidgetId)]
    pub focused_widget_id: Option<String>,
    #[wasm_bindgen(readonly, js_name = windowFocused)]
    pub window_focused: bool,
    #[wasm_bindgen(readonly, getter_with_clone, js_name = scheduledPhases)]
    pub scheduled_phases: Vec<String>,
    #[wasm_bindgen(readonly, js_name = semanticsCount)]
    pub semantics_count: u32,
    pub(crate) semantics_nodes: Vec<JsSemanticNode>,
    #[wasm_bindgen(readonly, js_name = widgetCount)]
    pub widget_count: u32,
    #[wasm_bindgen(readonly, js_name = stackHostCount)]
    pub stack_host_count: u32,
    #[wasm_bindgen(readonly, js_name = overlayCount)]
    pub overlay_count: u32,
    #[wasm_bindgen(readonly, js_name = timerCount)]
    pub timer_count: u32,
    #[wasm_bindgen(readonly, js_name = asyncTaskCount)]
    pub async_task_count: u32,
    #[wasm_bindgen(readonly, js_name = requestedAnimationFrameCount)]
    pub requested_animation_frame_count: u32,
    #[wasm_bindgen(readonly, js_name = widgetDiagnosticsCount)]
    pub widget_diagnostics_count: u32,
    #[wasm_bindgen(readonly, js_name = eventRouteCount)]
    pub event_route_count: u32,
    #[wasm_bindgen(readonly, js_name = reactiveInvalidationCount)]
    pub reactive_invalidation_count: u32,
    #[wasm_bindgen(readonly, js_name = commandDispatchCount)]
    pub command_dispatch_count: u32,
    #[wasm_bindgen(readonly, js_name = invalidationCount)]
    pub invalidation_count: u32,
    #[wasm_bindgen(readonly, js_name = widgetRebuildCount)]
    pub widget_rebuild_count: u32,
    pub(crate) frame_timings: Vec<JsFrameTiming>,
    pub(crate) widget_timings: Vec<JsWidgetTiming>,
    pub(crate) event_routes: Vec<JsEventRouteTrace>,
    pub(crate) reactive_invalidations: Vec<JsReactiveInvalidationTrace>,
    pub(crate) command_dispatches: Vec<JsCommandDispatchTrace>,
    pub(crate) invalidations: Vec<JsInvalidationTrace>,
    pub(crate) widget_rebuilds: Vec<JsWidgetRebuildTrace>,
}

js_class!(JsInspectorSnapshot => "InspectorSnapshot");

#[wasm_bindgen(js_class = InspectorSnapshot)]
impl JsInspectorSnapshot {
    #[wasm_bindgen(getter = semanticsNodes)]
    pub fn semantics_nodes(&self) -> Vec<JsSemanticNode> {
        self.semantics_nodes.clone()
    }

    #[wasm_bindgen(getter = frameTimings)]
    pub fn frame_timings(&self) -> JsValue {
        self.frame_timings.clone().into_js()
    }
    #[wasm_bindgen(getter = widgetTimings)]
    pub fn widget_timings(&self) -> JsValue {
        self.widget_timings.clone().into_js()
    }
    #[wasm_bindgen(getter = eventRoutes)]
    pub fn event_routes(&self) -> JsValue {
        self.event_routes.clone().into_js()
    }
    #[wasm_bindgen(getter = reactiveInvalidations)]
    pub fn reactive_invalidations(&self) -> JsValue {
        self.reactive_invalidations.clone().into_js()
    }
    #[wasm_bindgen(getter = commandDispatches)]
    pub fn command_dispatches(&self) -> JsValue {
        self.command_dispatches.clone().into_js()
    }
    #[wasm_bindgen(getter = invalidations)]
    pub fn invalidations(&self) -> JsValue {
        self.invalidations.clone().into_js()
    }
    #[wasm_bindgen(getter = widgetRebuilds)]
    pub fn widget_rebuilds(&self) -> JsValue {
        self.widget_rebuilds.clone().into_js()
    }
}

impl From<BindingInspectorSnapshot> for JsInspectorSnapshot {
    fn from(value: BindingInspectorSnapshot) -> Self {
        Self {
            window_id: value.window_id.to_string(),
            title: value.title,
            tracing_enabled: value.tracing_enabled,
            focused_widget_id: value.focused_widget_id.map(|id| id.to_string()),
            window_focused: value.window_focused,
            scheduled_phases: value.scheduled_phases,
            semantics_count: value.semantics_count as u32,
            semantics_nodes: value.semantics_nodes.into_iter().map(Into::into).collect(),
            widget_count: value.widget_count as u32,
            stack_host_count: value.stack_host_count as u32,
            overlay_count: value.overlay_count as u32,
            timer_count: value.timer_count as u32,
            async_task_count: value.async_task_count as u32,
            requested_animation_frame_count: value.requested_animation_frame_count as u32,
            widget_diagnostics_count: value.widget_diagnostics_count as u32,
            event_route_count: value.event_route_count as u32,
            reactive_invalidation_count: value.reactive_invalidation_count as u32,
            command_dispatch_count: value.command_dispatch_count as u32,
            invalidation_count: value.invalidation_count as u32,
            widget_rebuild_count: value.widget_rebuild_count as u32,
            frame_timings: value.frame_timings.into_iter().map(Into::into).collect(),
            widget_timings: value.widget_timings.into_iter().map(Into::into).collect(),
            event_routes: value.event_routes.into_iter().map(Into::into).collect(),
            reactive_invalidations: value
                .reactive_invalidations
                .into_iter()
                .map(Into::into)
                .collect(),
            command_dispatches: value
                .command_dispatches
                .into_iter()
                .map(Into::into)
                .collect(),
            invalidations: value.invalidations.into_iter().map(Into::into).collect(),
            widget_rebuilds: value.widget_rebuilds.into_iter().map(Into::into).collect(),
        }
    }
}

#[wasm_bindgen(js_class = RunningApp)]
impl JsRunningApp {
    #[wasm_bindgen(js_name = uiHandle)]
    pub fn ui_handle(&self) -> JsUiHandle {
        recover_lock(&self.inner).ui_handle().into()
    }

    #[wasm_bindgen]
    pub fn drain(&self) -> Result<u32> {
        recover_lock(&self.inner)
            .drain_ui_tasks()
            .map(|count| count as u32)
            .map_err(runtime_error)
    }

    /// Advance host-driven timers and animations to an absolute frame time in seconds.
    #[wasm_bindgen]
    pub fn tick(&self, frame_time: f64) {
        recover_lock(&self.inner).tick(frame_time);
    }

    /// The host-driven frame time in seconds.
    #[wasm_bindgen(getter = frameTime)]
    pub fn frame_time(&self) -> f64 {
        recover_lock(&self.inner).frame_time()
    }

    /// Move the frame clock forward by `seconds` and run the timers and
    /// animation frames that became due.
    #[wasm_bindgen(js_name = advanceTime)]
    pub fn advance_time(&self, seconds: f64) -> Result<()> {
        recover_lock(&self.inner)
            .advance_time(seconds)
            .map_err(runtime_error)
    }

    /// Advance time until every animation has finished and return how many
    /// seconds that took. Fails after `limit` seconds (default 10).
    #[wasm_bindgen(js_name = settleAnimations)]
    pub fn settle_animations(&self, limit: Option<f64>) -> Result<f64> {
        recover_lock(&self.inner)
            .settle_animations(limit.unwrap_or(10.0))
            .map_err(runtime_error)
    }

    /// Render a window offscreen and return it as PNG data.
    #[wasm_bindgen(js_name = screenshotPng)]
    pub fn screenshot_png(&self, index: Option<u32>) -> Result<Vec<u8>> {
        let mut runtime = recover_lock(&self.inner);
        let window_id = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(runtime_error)?;
        runtime.capture_png(window_id).map_err(runtime_error)
    }

    /// Dispatch due timer, animation-frame, and wake events and return how
    /// many were dispatched.
    #[wasm_bindgen(js_name = drainReadyEvents)]
    pub fn drain_ready_events(&self) -> Result<u32> {
        recover_lock(&self.inner)
            .drain_ready_event_count()
            .map(|count| count as u32)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = requestRedrawAll)]
    pub fn request_redraw_all(&self) -> Result<()> {
        recover_lock(&self.inner)
            .request_redraw_all()
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = wakeWindow)]
    pub fn wake_window(&self, window: &JsWindowHandle) -> Result<()> {
        recover_lock(&self.inner)
            .wake_window(window.inner)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = handleEventFor)]
    pub fn handle_event_for(&self, window: &JsWindowHandle, event: &JsEvent) -> Result<()> {
        recover_lock(&self.inner)
            .handle_event(window.inner, event.binding_event())
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = setRenderOptions)]
    pub fn set_render_options(&self, window: &JsWindowHandle, options: JsValue) -> Result<()> {
        let options = <JsRenderOptions as FromJs>::from_js(&options)?;
        recover_lock(&self.inner)
            .set_render_options(window.inner, options.try_into()?)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = setInspectorTracing)]
    pub fn set_inspector_tracing(&self, enabled: Option<bool>, index: Option<u32>) -> Result<()> {
        let mut runtime = recover_lock(&self.inner);
        let window = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(runtime_error)?;
        runtime
            .set_inspector_tracing(window, enabled.unwrap_or(true))
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn inspect(&self, index: Option<u32>) -> Result<JsInspectorSnapshot> {
        let runtime = recover_lock(&self.inner);
        let window = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(runtime_error)?;
        runtime
            .inspector_snapshot(window)
            .map(JsInspectorSnapshot::from)
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn render(&self, index: Option<u32>) -> Result<JsRenderSnapshot> {
        recover_lock(&self.inner)
            .render_window_at(index.unwrap_or(0) as usize)
            .map(JsRenderSnapshot::from)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = renderWindow)]
    pub fn render_window(&self, window: &JsWindowHandle) -> Result<JsRenderSnapshot> {
        recover_lock(&self.inner)
            .render_window(window.inner)
            .map(JsRenderSnapshot::from)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = needsRender)]
    pub fn needs_render(&self, index: Option<u32>) -> Result<bool> {
        let runtime = recover_lock(&self.inner);
        let window_id = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(runtime_error)?;
        runtime.needs_render(window_id).map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = requestRedraw)]
    pub fn request_redraw(&self, index: Option<u32>) -> Result<()> {
        let mut runtime = recover_lock(&self.inner);
        let window_id = runtime
            .window_id_at(index.unwrap_or(0) as usize)
            .map_err(runtime_error)?;
        runtime.request_redraw(window_id).map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = handleEvent)]
    pub fn handle_event(&self, event: &JsEvent, index: Option<u32>) -> Result<()> {
        recover_lock(&self.inner)
            .handle_event_at(index.unwrap_or(0) as usize, event.binding_event())
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn hover(&self, node: &JsSemanticNode, index: Option<u32>) -> Result<()> {
        recover_lock(&self.inner)
            .hover_node_at(index.unwrap_or(0) as usize, &node.inner)
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn click(&self, node: &JsSemanticNode, index: Option<u32>) -> Result<()> {
        recover_lock(&self.inner)
            .click_node_at(index.unwrap_or(0) as usize, &node.inner)
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn press(&self, node: &JsSemanticNode, key: String, index: Option<u32>) -> Result<()> {
        recover_lock(&self.inner)
            .press_node_at(index.unwrap_or(0) as usize, &node.inner, key)
            .map_err(runtime_error)
    }

    #[wasm_bindgen]
    pub fn fill(&self, node: &JsSemanticNode, text: String, index: Option<u32>) -> Result<()> {
        recover_lock(&self.inner)
            .fill_node_at(index.unwrap_or(0) as usize, &node.inner, text)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(getter = windowCount)]
    pub fn window_count(&self) -> u32 {
        recover_lock(&self.inner).window_count() as u32
    }

    #[wasm_bindgen(js_name = windowHandle)]
    pub fn window_id(&self, index: u32) -> Result<JsWindowHandle> {
        recover_lock(&self.inner)
            .window_id_at(index as usize)
            .map(JsWindowHandle::from)
            .map_err(runtime_error)
    }

    #[wasm_bindgen(js_name = windowIds)]
    pub fn window_ids(&self) -> Vec<String> {
        recover_lock(&self.inner)
            .window_ids()
            .into_iter()
            .map(|id| id.get().to_string())
            .collect()
    }

    #[wasm_bindgen(getter = pendingCount)]
    pub fn pending_count(&self) -> u32 {
        recover_lock(&self.inner).pending_ui_task_count() as u32
    }
}

impl JsRunningApp {
    pub(crate) fn new(runtime: BindingRuntime) -> Self {
        Self {
            inner: Mutex::new(runtime),
        }
    }
}
