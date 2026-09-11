use crate::diagnostics::{
    BindingRenderSnapshot, binding_semantics_busy, binding_semantics_checked,
    binding_semantics_descriptions, binding_semantics_disabled,
    binding_semantics_editable_multiline, binding_semantics_expanded, binding_semantics_focused,
    binding_semantics_hidden, binding_semantics_hovered, binding_semantics_names,
    binding_semantics_nodes, binding_semantics_roles, binding_semantics_selected,
    binding_semantics_values,
};
use crate::errors::ForeignErrorSink;
use crate::handles::{BindingFontHandle, BindingImageHandle, BindingWindowId};
use crate::messages::{BindingMessageAction, BindingMessageBus};
use crate::runtime::BindingRuntime;
use crate::tasks::{BindingUiHandle, UiTaskQueue};
use crate::theme::BindingTheme;
#[cfg(feature = "desktop")]
use crate::widget_adapters::BINDING_UI_TASKS_READY;
use crate::widget_adapters::BindingUiTaskRootWidget;
use crate::widget_descriptor::{BindingBuildContext, BindingWidget};
use std::io::Cursor;
#[cfg(feature = "desktop")]
use sui::App as SuiApp;
use sui::Point;
use sui::RegisteredFont;
use sui::RegisteredImage;
use sui::Runtime;
use sui::SceneCommand;
use sui::Size;
#[cfg(feature = "desktop")]
use sui::Window as SuiWindow;
use sui::WindowBuilder;
use sui::WindowColorManagementMode;
use sui::WindowDynamicRangeMode;
use sui::WindowOutputColorPrimaries;
use sui::WindowRenderOptions;
use sui::WindowToneMappingMode;

#[derive(Debug, Clone)]
pub struct BindingWindow {
    pub(crate) title: String,
    pub(crate) root: BindingWidget,
    pub(crate) initial_size: Option<Size>,
    pub(crate) initial_position: Option<Point>,
    pub(crate) icon: BindingWindowIcon,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum BindingWindowIcon {
    #[default]
    Default,
    None,
    Svg(Vec<u8>),
}

impl BindingWindow {
    pub fn new(title: impl Into<String>, root: BindingWidget) -> Self {
        Self {
            title: title.into(),
            root,
            initial_size: None,
            initial_position: None,
            icon: BindingWindowIcon::Default,
        }
    }

    pub fn with_initial_size(mut self, size: Size) -> Self {
        self.initial_size = Some(size);
        self
    }

    pub fn with_initial_position(mut self, position: Point) -> Self {
        self.initial_position = Some(position);
        self
    }

    pub fn with_icon_svg(mut self, svg: impl Into<Vec<u8>>) -> Self {
        self.icon = BindingWindowIcon::Svg(svg.into());
        self
    }

    pub fn without_icon(mut self) -> Self {
        self.icon = BindingWindowIcon::None;
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn root(&self) -> &BindingWidget {
        &self.root
    }

    pub fn initial_size(&self) -> Option<Size> {
        self.initial_size
    }

    pub fn initial_position(&self) -> Option<Point> {
        self.initial_position
    }

    pub(crate) fn configure_builder(&self, mut builder: WindowBuilder) -> WindowBuilder {
        if let Some(size) = self.initial_size {
            builder = builder.initial_size(size);
        }
        if let Some(position) = self.initial_position {
            builder = builder.initial_position(position);
        }
        match &self.icon {
            BindingWindowIcon::Default => builder,
            BindingWindowIcon::None => builder.without_icon(),
            BindingWindowIcon::Svg(svg) => builder.icon_svg(svg.clone()),
        }
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn configure_app_window(&self, mut window: SuiWindow) -> SuiWindow {
        if let Some(size) = self.initial_size {
            window = window.initial_size(size);
        }
        if let Some(position) = self.initial_position {
            window = window.initial_position(position);
        }
        match &self.icon {
            BindingWindowIcon::Default => window,
            BindingWindowIcon::None => window.without_icon(),
            BindingWindowIcon::Svg(svg) => window.icon_svg(svg.clone()),
        }
    }
}

/// Renderer-neutral window output policy shared by the Python and JavaScript APIs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingRenderOptions {
    pub(crate) inner: WindowRenderOptions,
}

impl BindingRenderOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        feathering_enabled: bool,
        feather_width: f32,
        optical_text_alignment: bool,
        output_color_primaries: &str,
        dynamic_range: &str,
        tone_mapping: &str,
        color_management: &str,
        sdr_content_brightness_nits: f32,
        use_system_sdr_brightness: bool,
    ) -> Result<Self, String> {
        let options = WindowRenderOptions::new(feathering_enabled, feather_width)
            .with_optical_vertical_text_alignment_enabled(optical_text_alignment)
            .with_output_color_primaries(parse_output_color_primaries(output_color_primaries)?)
            .with_dynamic_range_mode(parse_dynamic_range(dynamic_range)?)
            .with_tone_mapping_mode(parse_tone_mapping(tone_mapping)?)
            .with_color_management_mode(parse_color_management(color_management)?)
            .with_sdr_content_brightness_nits(sdr_content_brightness_nits)
            .with_system_sdr_content_brightness_enabled(use_system_sdr_brightness)
            .clamped();
        Ok(Self { inner: options })
    }

    pub const fn into_sui(self) -> WindowRenderOptions {
        self.inner
    }

    pub const fn feathering_enabled(self) -> bool {
        self.inner.feathering_enabled
    }

    pub const fn feather_width(self) -> f32 {
        self.inner.feather_width
    }
}

pub(crate) fn parse_output_color_primaries(
    value: &str,
) -> Result<WindowOutputColorPrimaries, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowOutputColorPrimaries::Automatic),
        "srgb" => Ok(WindowOutputColorPrimaries::Srgb),
        "displayp3" | "p3" => Ok(WindowOutputColorPrimaries::DisplayP3),
        _ => Err(format!(
            "output_color_primaries must be 'auto', 'srgb', or 'display-p3', got '{value}'"
        )),
    }
}

pub(crate) fn parse_dynamic_range(value: &str) -> Result<WindowDynamicRangeMode, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowDynamicRangeMode::Automatic),
        "sdr" | "standard" | "standarddynamicrange" => {
            Ok(WindowDynamicRangeMode::StandardDynamicRange)
        }
        "hdr" | "high" | "highdynamicrange" => Ok(WindowDynamicRangeMode::HighDynamicRange),
        _ => Err(format!(
            "dynamic_range must be 'auto', 'sdr', or 'hdr', got '{value}'"
        )),
    }
}

pub(crate) fn parse_tone_mapping(value: &str) -> Result<WindowToneMappingMode, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowToneMappingMode::Automatic),
        "clamp" => Ok(WindowToneMappingMode::Clamp),
        "reinhard" => Ok(WindowToneMappingMode::Reinhard),
        _ => Err(format!(
            "tone_mapping must be 'auto', 'clamp', or 'reinhard', got '{value}'"
        )),
    }
}

pub(crate) fn parse_color_management(value: &str) -> Result<WindowColorManagementMode, String> {
    match normalized_option_name(value).as_str() {
        "auto" | "automatic" => Ok(WindowColorManagementMode::Automatic),
        "forcesdr" | "sdr" => Ok(WindowColorManagementMode::ForceSdr),
        "preferwidegamut" | "widegamut" => Ok(WindowColorManagementMode::PreferWideGamut),
        "preferhdr" | "hdr" => Ok(WindowColorManagementMode::PreferHdr),
        _ => Err(format!(
            "color_management must be 'auto', 'force-sdr', 'prefer-wide-gamut', or 'prefer-hdr', got '{value}'"
        )),
    }
}

pub(crate) fn normalized_option_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct BindingApp {
    pub(crate) windows: Vec<BindingWindow>,
    pub(crate) theme: Option<BindingTheme>,
    pub(crate) font_resources: Vec<BindingFontResource>,
    pub(crate) next_font_slot: u64,
    pub(crate) image_resources: Vec<BindingImageResource>,
    pub(crate) next_image_slot: u64,
    pub(crate) render_options: Option<BindingRenderOptions>,
    pub(crate) messages: BindingMessageBus,
    pub(crate) errors: ForeignErrorSink,
}

#[derive(Debug, Clone)]
pub(crate) struct BindingFontResource {
    pub(crate) handle: BindingFontHandle,
    pub(crate) font: RegisteredFont,
}

#[derive(Debug, Clone)]
pub(crate) struct BindingImageResource {
    pub(crate) handle: BindingImageHandle,
    pub(crate) image: RegisteredImage,
}

impl BindingApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window(mut self, window: BindingWindow) -> Self {
        self.push_window(window);
        self
    }

    pub fn push_window(&mut self, window: BindingWindow) {
        self.windows.push(window);
    }

    pub fn set_theme(&mut self, theme: BindingTheme) {
        self.theme = Some(theme);
    }

    pub fn theme(&self) -> Option<BindingTheme> {
        self.theme.clone()
    }

    pub fn set_render_options(&mut self, options: BindingRenderOptions) {
        self.render_options = Some(options);
    }

    pub fn render_options(&self) -> Option<BindingRenderOptions> {
        self.render_options
    }

    pub fn on_message(&mut self, name: impl Into<String>, action: BindingMessageAction) {
        self.messages.on(name, action);
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn register_font_bytes(
        &mut self,
        data: impl Into<Vec<u8>>,
    ) -> Result<BindingFontHandle, String> {
        let handle = BindingFontHandle::app_resource(self.next_font_slot);
        self.next_font_slot = self.next_font_slot.saturating_add(1);
        self.font_resources.push(BindingFontResource {
            handle,
            font: RegisteredFont::from_bytes(data),
        });
        Ok(handle)
    }

    pub fn font_resource_count(&self) -> usize {
        self.font_resources.len()
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<BindingImageHandle, String> {
        let image =
            RegisteredImage::from_rgba8(width, height, data).map_err(|error| error.to_string())?;
        Ok(self.push_image_resource(image))
    }

    pub fn register_png_image(
        &mut self,
        data: impl AsRef<[u8]>,
    ) -> Result<BindingImageHandle, String> {
        let image = registered_image_from_png(data)?;
        Ok(self.push_image_resource(image))
    }

    pub fn register_svg_image(
        &mut self,
        data: impl AsRef<[u8]>,
    ) -> Result<BindingImageHandle, String> {
        let image = RegisteredImage::from_svg(data).map_err(|error| error.to_string())?;
        Ok(self.push_image_resource(image))
    }

    pub fn register_svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<BindingImageHandle, String> {
        let image = RegisteredImage::from_svg_at_size(width, height, data)
            .map_err(|error| error.to_string())?;
        Ok(self.push_image_resource(image))
    }

    pub fn image_resource_count(&self) -> usize {
        self.image_resources.len()
    }

    pub(crate) fn push_image_resource(&mut self, image: RegisteredImage) -> BindingImageHandle {
        let handle = BindingImageHandle::app_resource(self.next_image_slot);
        self.next_image_slot = self.next_image_slot.saturating_add(1);
        self.image_resources
            .push(BindingImageResource { handle, image });
        handle
    }

    pub(crate) fn register_image_resources(&self, runtime: &mut Runtime) -> Result<(), String> {
        for resource in &self.image_resources {
            runtime
                .register_image(resource.handle.into_sui(), resource.image.clone())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn register_font_resources(&self, runtime: &mut Runtime) -> Result<(), String> {
        for resource in &self.font_resources {
            runtime
                .register_font(resource.handle.into_sui(), resource.font.clone())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn error_sink(&self) -> ForeignErrorSink {
        self.errors.clone()
    }

    pub fn start(&self) -> Result<BindingRuntime, String> {
        let ui_tasks = UiTaskQueue::new();
        let ui_handle = ui_tasks.handle().with_message_bus(self.messages.clone());
        let mut runtime = Runtime::new();
        let mut window_ids = Vec::with_capacity(self.windows.len());

        self.register_font_resources(&mut runtime)?;
        self.register_image_resources(&mut runtime)?;

        for window in &self.windows {
            window.root.bind_ui_handle(&ui_handle);
            if let Some(theme) = &self.theme {
                theme.bind_ui_handle(ui_handle.clone());
            }
            let root = BindingUiTaskRootWidget::new(
                window.root.into_runtime_widget(BindingBuildContext::new(
                    self.errors.clone(),
                    self.theme.clone(),
                )),
                ui_tasks.clone(),
            );
            let builder = window
                .configure_builder(WindowBuilder::new().title(window.title.clone()).root(root));
            let window_id = runtime
                .add_window(builder)
                .map_err(|error| error.to_string())?;
            if let Some(options) = self.render_options {
                sui::set_window_render_options(window_id, options.into_sui());
            }
            window_ids.push(BindingWindowId::from(window_id));
        }

        Ok(BindingRuntime {
            runtime,
            window_ids,
            ui_tasks,
            messages: self.messages.clone(),
        })
    }

    #[cfg(feature = "desktop")]
    pub fn run(&self) -> Result<(), String> {
        self.run_with_handle(|_| {})
    }

    #[cfg(not(feature = "desktop"))]
    pub fn run(&self) -> Result<(), String> {
        Err("BindingApp::run requires the `desktop` feature".to_string())
    }

    #[cfg(feature = "desktop")]
    pub fn run_with_handle(&self, on_ready: impl FnOnce(BindingUiHandle)) -> Result<(), String> {
        let ui_tasks = UiTaskQueue::new();
        let ui_handle = ui_tasks.handle().with_message_bus(self.messages.clone());
        let mut app = SuiApp::new();
        if let Some(options) = self.render_options {
            app = app.render_options(options.into_sui());
        }

        {
            let mut resources = app.resources();
            for resource in &self.font_resources {
                resources
                    .register_font(resource.handle.into_sui(), resource.font.clone())
                    .map_err(|error| error.to_string())?;
            }
            for resource in &self.image_resources {
                resources
                    .image(resource.handle.into_sui(), resource.image.clone())
                    .map_err(|error| error.to_string())?;
            }
        }

        for window in &self.windows {
            window.root.bind_ui_handle(&ui_handle);
            if let Some(theme) = &self.theme {
                theme.bind_ui_handle(ui_handle.clone());
            }
            let root = BindingUiTaskRootWidget::new(
                window.root.into_runtime_widget(BindingBuildContext::new(
                    self.errors.clone(),
                    self.theme.clone(),
                )),
                ui_tasks.clone(),
            );
            let tasks_for_window = ui_tasks.clone();
            let app_window = window
                .configure_app_window(SuiWindow::new(window.title.clone()).root(root))
                .on_command(BINDING_UI_TASKS_READY, move |ctx, _| {
                    tasks_for_window.drain();
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                });
            app = app.window(app_window);
        }

        let tasks_for_waker = ui_tasks.clone();
        app.run_with_handle(move |native_ui| {
            tasks_for_waker.set_waker(move || {
                native_ui.broadcast_application(BINDING_UI_TASKS_READY, ());
            });
            on_ready(ui_handle);
        })
        .map_err(|error| error.to_string())
    }

    #[cfg(not(feature = "desktop"))]
    pub fn run_with_handle(&self, _on_ready: impl FnOnce(BindingUiHandle)) -> Result<(), String> {
        Err("BindingApp::run_with_handle requires the `desktop` feature".to_string())
    }

    pub fn render_window(&self, index: usize) -> Result<BindingRenderSnapshot, String> {
        let window = self
            .windows
            .get(index)
            .ok_or_else(|| format!("window index {index} is out of range"))?;
        let mut runtime = Runtime::new();
        self.register_font_resources(&mut runtime)?;
        self.register_image_resources(&mut runtime)?;
        let builder =
            window.configure_builder(WindowBuilder::new().title(window.title.clone()).root(
                window.root.into_runtime_widget(BindingBuildContext::new(
                    self.errors.clone(),
                    self.theme.clone(),
                )),
            ));
        let window_id = runtime
            .add_window(builder)
            .map_err(|error| error.to_string())?;
        if let Some(options) = self.render_options {
            sui::set_window_render_options(window_id, options.into_sui());
        }
        let output = runtime
            .render(window_id)
            .map_err(|error| error.to_string())?;
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
        Ok(BindingRenderSnapshot {
            command_count,
            semantics_count: output.semantics.len(),
            semantics_nodes: binding_semantics_nodes(&output.semantics),
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
}

pub fn registered_image_from_png(data: impl AsRef<[u8]>) -> Result<RegisteredImage, String> {
    let mut decoder = png::Decoder::new(Cursor::new(data.as_ref()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "PNG image exceeds the decoder output size limit".to_string())?;
    let mut buffer = vec![0; output_size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| error.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "expected 8-bit PNG data after decoding, got {:?}",
            info.bit_depth
        ));
    }
    let rgba = png_frame_to_rgba8(info.color_type, &buffer[..info.buffer_size()])?;
    RegisteredImage::from_rgba8(info.width, info.height, rgba).map_err(|error| error.to_string())
}

pub(crate) fn png_frame_to_rgba8(
    color_type: png::ColorType,
    data: &[u8],
) -> Result<Vec<u8>, String> {
    match color_type {
        png::ColorType::Rgba => Ok(data.to_vec()),
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((data.len() / 3) * 4);
            for chunk in data.chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            Ok(rgba)
        }
        png::ColorType::Grayscale => {
            let mut rgba = Vec::with_capacity(data.len() * 4);
            for value in data {
                rgba.extend_from_slice(&[*value, *value, *value, 255]);
            }
            Ok(rgba)
        }
        png::ColorType::GrayscaleAlpha => {
            let mut rgba = Vec::with_capacity((data.len() / 2) * 4);
            for chunk in data.chunks_exact(2) {
                rgba.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            Ok(rgba)
        }
        png::ColorType::Indexed => Err("indexed PNG data was not expanded to RGBA".to_string()),
    }
}
