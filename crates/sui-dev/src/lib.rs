mod app;

#[cfg(not(target_arch = "wasm32"))]
use app::{DesktopAutomationMode, build_dev_application_with_automation};
pub use app::{build_dev_application, build_dev_application_with_widget_book_bounds};

#[cfg(not(target_arch = "wasm32"))]
use std::env;

use sui::{Application, Rect};
use sui::{
    DesktopAutomationAction, DesktopAutomationConfig, DesktopPlatform, SceneStatisticsDetailMode,
    SemanticsRole, WindowColorManagementMode, WindowDynamicRangeMode, WindowOutputColorPrimaries,
    WindowRenderOptions, WindowToneMappingMode, set_window_render_options,
    set_window_scene_statistics_detail_mode,
};
use sui_widget_book::{
    GALLERY_SCROLL_NAME, build_button_grid_benchmark_application,
    build_color_validation_application, build_retained_text_benchmark_application,
    build_text_editing_benchmark_application, build_text_rendering_comparison_application,
    build_widget_book_application, default_widget_book_state,
};

#[cfg(not(target_arch = "wasm32"))]
const DESKTOP_NO_VSYNC_ENV: &str = "SUI_DEV_NO_VSYNC";
#[cfg(not(target_arch = "wasm32"))]
const DESKTOP_AUTOMATION_ENV: &str = "SUI_DEV_AUTOMATION";

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesktopLaunchMode {
    vsync_enabled: bool,
    automation: Option<DesktopLaunchAutomation>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for DesktopLaunchMode {
    fn default() -> Self {
        Self {
            vsync_enabled: true,
            automation: None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopLaunchAutomation {
    ButtonGridResize,
    WidgetBookScroll,
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_desktop_automation(
    raw_value: Option<&str>,
) -> sui::Result<Option<DesktopLaunchAutomation>> {
    match raw_value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        None => Ok(None),
        Some("button-grid-resize") => Ok(Some(DesktopLaunchAutomation::ButtonGridResize)),
        Some("widget-book-scroll") => Ok(Some(DesktopLaunchAutomation::WidgetBookScroll)),
        Some(other) => Err(sui::Error::new(format!(
            "unsupported sui-dev automation `{other}`; supported values: button-grid-resize, widget-book-scroll"
        ))),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn app_automation_mode(mode: Option<DesktopLaunchAutomation>) -> Option<DesktopAutomationMode> {
    match mode {
        Some(DesktopLaunchAutomation::ButtonGridResize) => {
            Some(DesktopAutomationMode::ButtonGridResize)
        }
        Some(DesktopLaunchAutomation::WidgetBookScroll) | None => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_automation_config(
    mode: Option<DesktopLaunchAutomation>,
) -> Option<DesktopAutomationConfig> {
    match mode {
        Some(DesktopLaunchAutomation::WidgetBookScroll) => Some(DesktopAutomationConfig {
            label: "widget-book-scroll".to_string(),
            target_role: SemanticsRole::ScrollView,
            target_name: GALLERY_SCROLL_NAME.to_string(),
            action: DesktopAutomationAction::ScrollPixels {
                delta: sui::Vector::new(0.0, -48.0),
            },
            step_interval: std::time::Duration::from_millis(8),
            duration: std::time::Duration::from_secs(4),
            report_interval: std::time::Duration::from_millis(500),
            startup_timeout: std::time::Duration::from_secs(2),
        }),
        Some(DesktopLaunchAutomation::ButtonGridResize) | None => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn env_requests_no_vsync(raw_value: Option<&str>) -> bool {
    raw_value.is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_desktop_launch_mode<I, S>(
    args: I,
    env_disables_vsync: bool,
    env_automation: Option<DesktopLaunchAutomation>,
) -> sui::Result<DesktopLaunchMode>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mode = DesktopLaunchMode {
        vsync_enabled: !env_disables_vsync,
        automation: env_automation,
    };

    for arg in args {
        match arg.as_ref() {
            "--no-vsync" => mode.vsync_enabled = false,
            "--vsync" => mode.vsync_enabled = true,
            value if value.starts_with("--automation=") => {
                mode.automation =
                    parse_desktop_automation(value.split_once('=').map(|(_, rhs)| rhs))?;
            }
            "" => {}
            other => {
                return Err(sui::Error::new(format!(
                    "unsupported sui-dev argument `{other}`; supported flags: --no-vsync, --vsync, --automation=<button-grid-resize|widget-book-scroll>"
                )));
            }
        }
    }

    Ok(mode)
}

#[cfg(not(target_arch = "wasm32"))]
fn current_desktop_launch_mode() -> sui::Result<DesktopLaunchMode> {
    parse_desktop_launch_mode(
        env::args().skip(1),
        env_requests_no_vsync(env::var(DESKTOP_NO_VSYNC_ENV).ok().as_deref()),
        parse_desktop_automation(env::var(DESKTOP_AUTOMATION_ENV).ok().as_deref())?,
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop_with_vsync(vsync_enabled: bool) -> sui::Result<()> {
    let app = build_dev_application();
    run_desktop_application(app, vsync_enabled)
}

#[cfg(not(target_arch = "wasm32"))]
fn run_desktop_application(app: Application, vsync_enabled: bool) -> sui::Result<()> {
    let feathering_enabled = app.feathering_enabled();
    let feather_width = app.feather_width();
    let initial_window_render_options = app.initial_window_render_options();
    let runtime = app.build()?;
    let platform = DesktopPlatform::new()
        .with_feathering_enabled(feathering_enabled)
        .with_feather_width(feather_width)
        .with_vsync_enabled(vsync_enabled);

    if let Some(options) = initial_window_render_options {
        for window_id in runtime.window_ids() {
            set_window_render_options(window_id, options);
        }
    }

    let _ = platform.run(runtime)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn run_desktop_application_with_mode(launch_mode: DesktopLaunchMode) -> sui::Result<()> {
    let app = build_dev_application_with_automation(app_automation_mode(launch_mode.automation));
    let feathering_enabled = app.feathering_enabled();
    let feather_width = app.feather_width();
    let initial_window_render_options = app.initial_window_render_options();
    let runtime = app.build()?;
    let mut platform = DesktopPlatform::new()
        .with_feathering_enabled(feathering_enabled)
        .with_feather_width(feather_width)
        .with_vsync_enabled(launch_mode.vsync_enabled);

    if let Some(options) = initial_window_render_options {
        for window_id in runtime.window_ids() {
            set_window_render_options(window_id, options);
        }
    }

    if let Some(automation) = platform_automation_config(launch_mode.automation) {
        platform = platform.with_automation(automation);
    }
    if launch_mode.automation.is_some() {
        for window_id in runtime.window_ids() {
            set_window_scene_statistics_detail_mode(window_id, SceneStatisticsDetailMode::Detailed);
        }
    }
    let _ = platform.run(runtime)?;
    Ok(())
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebBenchmarkKind {
    ButtonGrid,
    RetainedText,
    TextEditing,
    TextComparison,
    ColorValidation,
    WidgetBook,
    DevWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebCanvasFormatPreference {
    Auto,
    Rgba8UnormSrgb,
    Rgba16Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebCanvasColorSpacePreference {
    Auto,
    Srgb,
    DisplayP3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebCanvasToneMappingPreference {
    Auto,
    Standard,
    Extended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebLaunchMode {
    benchmark: Option<WebBenchmarkKind>,
    frames: usize,
    warmup_frames: usize,
    canvas_format: WebCanvasFormatPreference,
    canvas_color_space: WebCanvasColorSpacePreference,
    canvas_tone_mapping: WebCanvasToneMappingPreference,
    color_management_mode: WindowColorManagementMode,
    output_primaries: WindowOutputColorPrimaries,
    dynamic_range: WindowDynamicRangeMode,
    tone_mapping: WindowToneMappingMode,
}

impl Default for WebLaunchMode {
    fn default() -> Self {
        Self {
            benchmark: None,
            frames: 180,
            warmup_frames: 60,
            canvas_format: WebCanvasFormatPreference::Auto,
            canvas_color_space: WebCanvasColorSpacePreference::Auto,
            canvas_tone_mapping: WebCanvasToneMappingPreference::Auto,
            color_management_mode: WindowColorManagementMode::Automatic,
            output_primaries: WindowOutputColorPrimaries::Automatic,
            dynamic_range: WindowDynamicRangeMode::Automatic,
            tone_mapping: WindowToneMappingMode::Automatic,
        }
    }
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn parse_web_launch_mode(query: &str) -> WebLaunchMode {
    let mut mode = WebLaunchMode::default();

    for pair in query.trim_start_matches('?').split('&') {
        if pair.is_empty() {
            continue;
        }

        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        match key {
            "benchmark" => {
                mode.benchmark = match value {
                    "button-grid" => Some(WebBenchmarkKind::ButtonGrid),
                    "retained-text" => Some(WebBenchmarkKind::RetainedText),
                    "text-editing" => Some(WebBenchmarkKind::TextEditing),
                    "text-comparison" | "comparison-surface" => {
                        Some(WebBenchmarkKind::TextComparison)
                    }
                    "color-validation" | "wide-gamut-validation" => {
                        Some(WebBenchmarkKind::ColorValidation)
                    }
                    "widget-book" => Some(WebBenchmarkKind::WidgetBook),
                    "dev" | "workspace" => Some(WebBenchmarkKind::DevWorkspace),
                    _ => None,
                };
            }
            "frames" => {
                mode.frames = value
                    .parse::<usize>()
                    .unwrap_or(mode.frames)
                    .clamp(1, 10_000);
            }
            "warmup" | "warmup-frames" => {
                mode.warmup_frames = value
                    .parse::<usize>()
                    .unwrap_or(mode.warmup_frames)
                    .clamp(0, 2_000);
            }
            "canvas-format" => {
                mode.canvas_format = match value {
                    "rgba8unorm-srgb" | "srgb" => WebCanvasFormatPreference::Rgba8UnormSrgb,
                    "rgba16float" | "float16" | "hdr" => WebCanvasFormatPreference::Rgba16Float,
                    _ => WebCanvasFormatPreference::Auto,
                };
            }
            "canvas-color-space" => {
                mode.canvas_color_space = match value {
                    "srgb" => WebCanvasColorSpacePreference::Srgb,
                    "display-p3" | "p3" => WebCanvasColorSpacePreference::DisplayP3,
                    _ => WebCanvasColorSpacePreference::Auto,
                };
            }
            "canvas-tone-mapping" => {
                mode.canvas_tone_mapping = match value {
                    "standard" => WebCanvasToneMappingPreference::Standard,
                    "extended" | "hdr" => WebCanvasToneMappingPreference::Extended,
                    _ => WebCanvasToneMappingPreference::Auto,
                };
            }
            "color-management" => {
                mode.color_management_mode = match value {
                    "force-sdr" => WindowColorManagementMode::ForceSdr,
                    "prefer-wide-gamut" => WindowColorManagementMode::PreferWideGamut,
                    "prefer-hdr" => WindowColorManagementMode::PreferHdr,
                    _ => WindowColorManagementMode::Automatic,
                };
            }
            "output-primaries" => {
                mode.output_primaries = match value {
                    "srgb" => WindowOutputColorPrimaries::Srgb,
                    "display-p3" | "p3" => WindowOutputColorPrimaries::DisplayP3,
                    _ => WindowOutputColorPrimaries::Automatic,
                };
            }
            "dynamic-range" => {
                mode.dynamic_range = match value {
                    "sdr" | "standard" => WindowDynamicRangeMode::StandardDynamicRange,
                    "hdr" | "high" => WindowDynamicRangeMode::HighDynamicRange,
                    _ => WindowDynamicRangeMode::Automatic,
                };
            }
            "tone-mapping" => {
                mode.tone_mapping = match value {
                    "clamp" => WindowToneMappingMode::Clamp,
                    "reinhard" => WindowToneMappingMode::Reinhard,
                    _ => WindowToneMappingMode::Automatic,
                };
            }
            _ => {}
        }
    }

    mode
}

#[derive(Debug, Clone, PartialEq)]
struct WebBrowserProbe {
    current_path: String,
    user_agent: String,
    language: String,
    device_pixel_ratio: f64,
    canvas_count: u32,
    document_title: String,
}

#[derive(Debug, Clone, PartialEq)]
struct WebCanvasCapture {
    canvas_count: u32,
    first_canvas_id: String,
    first_canvas_width: u32,
    first_canvas_height: u32,
    first_canvas_data_url_len: usize,
}

fn web_benchmark_slug(benchmark: Option<WebBenchmarkKind>) -> &'static str {
    match benchmark {
        Some(WebBenchmarkKind::ButtonGrid) => "button-grid",
        Some(WebBenchmarkKind::RetainedText) => "retained-text",
        Some(WebBenchmarkKind::TextEditing) => "text-editing",
        Some(WebBenchmarkKind::TextComparison) => "text-comparison",
        Some(WebBenchmarkKind::ColorValidation) => "color-validation",
        Some(WebBenchmarkKind::WidgetBook) => "widget-book",
        Some(WebBenchmarkKind::DevWorkspace) | None => "dev",
    }
}

fn web_canvas_format_slug(format: WebCanvasFormatPreference) -> &'static str {
    match format {
        WebCanvasFormatPreference::Auto => "auto",
        WebCanvasFormatPreference::Rgba8UnormSrgb => "rgba8unorm-srgb",
        WebCanvasFormatPreference::Rgba16Float => "rgba16float",
    }
}

fn web_canvas_color_space_slug(color_space: WebCanvasColorSpacePreference) -> &'static str {
    match color_space {
        WebCanvasColorSpacePreference::Auto => "auto",
        WebCanvasColorSpacePreference::Srgb => "srgb",
        WebCanvasColorSpacePreference::DisplayP3 => "display-p3",
    }
}

fn web_canvas_tone_mapping_slug(tone_mapping: WebCanvasToneMappingPreference) -> &'static str {
    match tone_mapping {
        WebCanvasToneMappingPreference::Auto => "auto",
        WebCanvasToneMappingPreference::Standard => "standard",
        WebCanvasToneMappingPreference::Extended => "extended",
    }
}

fn web_color_management_slug(mode: WindowColorManagementMode) -> &'static str {
    match mode {
        WindowColorManagementMode::Automatic => "automatic",
        WindowColorManagementMode::ForceSdr => "force-sdr",
        WindowColorManagementMode::PreferWideGamut => "prefer-wide-gamut",
        WindowColorManagementMode::PreferHdr => "prefer-hdr",
    }
}

fn web_output_primaries_slug(primaries: WindowOutputColorPrimaries) -> &'static str {
    match primaries {
        WindowOutputColorPrimaries::Automatic => "automatic",
        WindowOutputColorPrimaries::Srgb => "srgb",
        WindowOutputColorPrimaries::DisplayP3 => "display-p3",
    }
}

fn web_dynamic_range_slug(dynamic_range: WindowDynamicRangeMode) -> &'static str {
    match dynamic_range {
        WindowDynamicRangeMode::Automatic => "automatic",
        WindowDynamicRangeMode::StandardDynamicRange => "sdr",
        WindowDynamicRangeMode::HighDynamicRange => "hdr",
    }
}

fn web_tone_mapping_slug(tone_mapping: WindowToneMappingMode) -> &'static str {
    match tone_mapping {
        WindowToneMappingMode::Automatic => "automatic",
        WindowToneMappingMode::Clamp => "clamp",
        WindowToneMappingMode::Reinhard => "reinhard",
    }
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_canvas_capture_report(mode: &WebLaunchMode, capture: &WebCanvasCapture) -> String {
    format!(
        "route={}; canvas_count={}; first_canvas_id={}; first_canvas_size={}x{}; first_canvas_data_url_len={}",
        web_benchmark_slug(mode.benchmark),
        capture.canvas_count,
        capture.first_canvas_id,
        capture.first_canvas_width,
        capture.first_canvas_height,
        capture.first_canvas_data_url_len,
    )
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_validation_url_for_path(path: &str, mode: &WebLaunchMode) -> String {
    let normalized_path = if path.is_empty() { "/" } else { path };
    format!("{}?{}", normalized_path, web_validation_query(mode))
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_browser_probe_report(mode: &WebLaunchMode, probe: &WebBrowserProbe) -> String {
    format!(
        "route={}; path={}; document_title={}; language={}; device_pixel_ratio={}; canvas_count={}; user_agent={}; validation_url={}",
        web_benchmark_slug(mode.benchmark),
        probe.current_path,
        probe.document_title,
        probe.language,
        probe.device_pixel_ratio,
        probe.canvas_count,
        probe.user_agent,
        web_validation_url_for_path(&probe.current_path, mode),
    )
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_validation_query(mode: &WebLaunchMode) -> String {
    format!(
        "benchmark={}&frames={}&warmup={}&canvas-format={}&canvas-color-space={}&canvas-tone-mapping={}&color-management={}&output-primaries={}&dynamic-range={}&tone-mapping={}",
        web_benchmark_slug(mode.benchmark),
        mode.frames,
        mode.warmup_frames,
        web_canvas_format_slug(mode.canvas_format),
        web_canvas_color_space_slug(mode.canvas_color_space),
        web_canvas_tone_mapping_slug(mode.canvas_tone_mapping),
        web_color_management_slug(mode.color_management_mode),
        web_output_primaries_slug(mode.output_primaries),
        web_dynamic_range_slug(mode.dynamic_range),
        web_tone_mapping_slug(mode.tone_mapping),
    )
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
fn web_validation_report(mode: &WebLaunchMode) -> String {
    format!(
        "route={}; canvas_format={}; canvas_color_space={}; canvas_tone_mapping={}; color_management={}; output_primaries={}; dynamic_range={}; tone_mapping={}; query=?{}",
        web_benchmark_slug(mode.benchmark),
        web_canvas_format_slug(mode.canvas_format),
        web_canvas_color_space_slug(mode.canvas_color_space),
        web_canvas_tone_mapping_slug(mode.canvas_tone_mapping),
        web_color_management_slug(mode.color_management_mode),
        web_output_primaries_slug(mode.output_primaries),
        web_dynamic_range_slug(mode.dynamic_range),
        web_tone_mapping_slug(mode.tone_mapping),
        web_validation_query(mode),
    )
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn web_window_render_options(mode: &WebLaunchMode) -> WindowRenderOptions {
    WindowRenderOptions::new(true, 1.0)
        .with_color_management_mode(mode.color_management_mode)
        .with_output_color_primaries(mode.output_primaries)
        .with_dynamic_range_mode(mode.dynamic_range)
        .with_tone_mapping_mode(mode.tone_mapping)
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn build_application_for_web_mode(mode: &WebLaunchMode) -> Application {
    let render_options = web_window_render_options(mode);
    let application = match mode.benchmark {
        Some(WebBenchmarkKind::ButtonGrid) => build_button_grid_benchmark_application(),
        Some(WebBenchmarkKind::RetainedText) => build_retained_text_benchmark_application(),
        Some(WebBenchmarkKind::TextEditing) => build_text_editing_benchmark_application(),
        Some(WebBenchmarkKind::TextComparison) => build_text_rendering_comparison_application(),
        Some(WebBenchmarkKind::ColorValidation) => build_color_validation_application(),
        Some(WebBenchmarkKind::WidgetBook) => {
            build_widget_book_application(default_widget_book_state())
        }
        Some(WebBenchmarkKind::DevWorkspace) | None => {
            app::build_dev_application_with_widget_book_bounds_and_render_options(
                Rect::new(24.0, 24.0, 680.0, 760.0),
                render_options,
            )
        }
    };
    application.with_window_render_options(render_options)
}

pub fn run_desktop() -> sui::Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        build_dev_application().run()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let launch_mode = current_desktop_launch_mode()?;
        run_desktop_application_with_mode(launch_mode)
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
fn current_web_launch_mode() -> WebLaunchMode {
    let query = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    parse_web_launch_mode(&query)
}

#[cfg(target_arch = "wasm32")]
fn current_web_browser_probe() -> WebBrowserProbe {
    let Some(window) = web_sys::window() else {
        return WebBrowserProbe {
            current_path: "/".to_string(),
            user_agent: String::new(),
            language: String::new(),
            device_pixel_ratio: 1.0,
            canvas_count: 0,
            document_title: String::new(),
        };
    };

    let location = window.location();
    let document = window.document();
    let navigator = window.navigator();
    let current_path = location
        .pathname()
        .ok()
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "/".to_string());
    let canvas_count = document
        .as_ref()
        .map(|document| document.get_elements_by_tag_name("canvas").length())
        .unwrap_or(0);

    WebBrowserProbe {
        current_path,
        user_agent: navigator.user_agent().unwrap_or_default(),
        language: navigator.language().unwrap_or_default(),
        device_pixel_ratio: window.device_pixel_ratio(),
        canvas_count,
        document_title: document
            .map(|document| document.title())
            .unwrap_or_default(),
    }
}

#[cfg(target_arch = "wasm32")]
fn current_web_canvas_capture() -> WebCanvasCapture {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return WebCanvasCapture {
            canvas_count: 0,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };
    let Some(document) = window.document() else {
        return WebCanvasCapture {
            canvas_count: 0,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };

    let canvases = document.get_elements_by_tag_name("canvas");
    let canvas_count = canvases.length();
    let Some(first_canvas) = canvases.item(0) else {
        return WebCanvasCapture {
            canvas_count,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };
    let Ok(canvas) = first_canvas.dyn_into::<web_sys::HtmlCanvasElement>() else {
        return WebCanvasCapture {
            canvas_count,
            first_canvas_id: String::new(),
            first_canvas_width: 0,
            first_canvas_height: 0,
            first_canvas_data_url_len: 0,
        };
    };
    let data_url_len = canvas.to_data_url().map(|value| value.len()).unwrap_or(0);

    WebCanvasCapture {
        canvas_count,
        first_canvas_id: canvas.id(),
        first_canvas_width: canvas.width(),
        first_canvas_height: canvas.height(),
        first_canvas_data_url_len: data_url_len,
    }
}

#[cfg(target_arch = "wasm32")]
fn current_web_validation_url() -> String {
    let probe = current_web_browser_probe();
    web_validation_url_for_path(&probe.current_path, &current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_validation_query() -> String {
    web_validation_query(&current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_validation_report() -> String {
    web_validation_report(&current_web_launch_mode())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_browser_probe_report() -> String {
    web_browser_probe_report(&current_web_launch_mode(), &current_web_browser_probe())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_canvas_capture_report() -> String {
    web_canvas_capture_report(&current_web_launch_mode(), &current_web_canvas_capture())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_canvas_capture_data_url() -> String {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return String::new();
    };
    let Some(document) = window.document() else {
        return String::new();
    };
    let Some(first_canvas) = document.get_elements_by_tag_name("canvas").item(0) else {
        return String::new();
    };
    let Ok(canvas) = first_canvas.dyn_into::<web_sys::HtmlCanvasElement>() else {
        return String::new();
    };
    canvas.to_data_url().unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn sui_web_validation_url() -> String {
    current_web_validation_url()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let mode = current_web_launch_mode();
    build_application_for_web_mode(&mode)
        .run()
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_web_launch_mode() {
        assert_eq!(parse_web_launch_mode("").benchmark, None);
        assert_eq!(parse_web_launch_mode("benchmark=unknown").benchmark, None);
    }

    #[test]
    fn parses_button_grid_web_benchmark_mode() {
        let mode = parse_web_launch_mode("benchmark=button-grid&frames=240&warmup=30");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::ButtonGrid));
        assert_eq!(mode.frames, 240);
        assert_eq!(mode.warmup_frames, 30);
    }

    #[test]
    fn parses_text_comparison_web_benchmark_mode() {
        let mode = parse_web_launch_mode("benchmark=text-comparison&frames=240&warmup=30");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::TextComparison));
        assert_eq!(mode.frames, 240);
        assert_eq!(mode.warmup_frames, 30);
    }

    #[test]
    fn parses_comparison_surface_alias() {
        let mode = parse_web_launch_mode("benchmark=comparison-surface");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::TextComparison));
    }

    #[test]
    fn parses_color_validation_and_web_output_preferences() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=reinhard",
        );
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::ColorValidation));
        assert_eq!(mode.canvas_format, WebCanvasFormatPreference::Rgba16Float);
        assert_eq!(
            mode.canvas_color_space,
            WebCanvasColorSpacePreference::DisplayP3
        );
        assert_eq!(
            mode.canvas_tone_mapping,
            WebCanvasToneMappingPreference::Extended
        );
        assert_eq!(
            mode.color_management_mode,
            WindowColorManagementMode::PreferHdr
        );
        assert_eq!(mode.output_primaries, WindowOutputColorPrimaries::DisplayP3);
        assert_eq!(mode.dynamic_range, WindowDynamicRangeMode::HighDynamicRange);
        assert_eq!(mode.tone_mapping, WindowToneMappingMode::Reinhard);
    }

    #[test]
    fn web_window_render_options_reflect_launch_mode_preferences() {
        let mode = parse_web_launch_mode(
            "color-management=prefer-wide-gamut&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=clamp",
        );
        let options = web_window_render_options(&mode);

        assert_eq!(
            options.color_management_mode,
            WindowColorManagementMode::PreferWideGamut
        );
        assert_eq!(
            options.output_color_primaries,
            WindowOutputColorPrimaries::DisplayP3
        );
        assert_eq!(
            options.dynamic_range_mode,
            WindowDynamicRangeMode::HighDynamicRange
        );
        assert_eq!(options.tone_mapping_mode, WindowToneMappingMode::Clamp);
    }

    #[test]
    fn web_validation_query_normalizes_phase4_preferences() {
        let mode = parse_web_launch_mode(
            "benchmark=wide-gamut-validation&canvas-format=hdr&canvas-color-space=p3&canvas-tone-mapping=hdr&color-management=prefer-hdr&output-primaries=p3&dynamic-range=high&tone-mapping=reinhard&frames=240&warmup=24",
        );

        assert_eq!(
            web_validation_query(&mode),
            "benchmark=color-validation&frames=240&warmup=24&canvas-format=rgba16float&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=reinhard"
        );
    }

    #[test]
    fn web_validation_report_summarizes_browser_validation_configuration() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-wide-gamut&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=clamp",
        );
        let report = web_validation_report(&mode);

        assert!(report.contains("route=color-validation"));
        assert!(report.contains("canvas_format=rgba16float"));
        assert!(report.contains("canvas_color_space=display-p3"));
        assert!(report.contains("canvas_tone_mapping=extended"));
        assert!(report.contains("color_management=prefer-wide-gamut"));
        assert!(report.contains("output_primaries=display-p3"));
        assert!(report.contains("dynamic_range=hdr"));
        assert!(report.contains("tone_mapping=clamp"));
    }

    #[test]
    fn web_browser_probe_report_includes_live_browser_context() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended&color-management=prefer-hdr&output-primaries=display-p3&dynamic-range=hdr&tone-mapping=reinhard",
        );
        let probe = WebBrowserProbe {
            current_path: "/sui-dev".to_string(),
            user_agent: "ExampleBrowser/1.0".to_string(),
            language: "en-US".to_string(),
            device_pixel_ratio: 2.0,
            canvas_count: 2,
            document_title: "SUI Dev Validation".to_string(),
        };
        let report = web_browser_probe_report(&mode, &probe);

        assert!(report.contains("path=/sui-dev"));
        assert!(report.contains("document_title=SUI Dev Validation"));
        assert!(report.contains("language=en-US"));
        assert!(report.contains("device_pixel_ratio=2"));
        assert!(report.contains("canvas_count=2"));
        assert!(report.contains("user_agent=ExampleBrowser/1.0"));
        assert!(report.contains("validation_url=/sui-dev?benchmark=color-validation"));
        assert!(report.contains("canvas-format=rgba16float"));
    }

    #[test]
    fn web_canvas_capture_report_describes_first_canvas_snapshot() {
        let mode = parse_web_launch_mode(
            "benchmark=color-validation&canvas-format=float16&canvas-color-space=display-p3&canvas-tone-mapping=extended",
        );
        let capture = WebCanvasCapture {
            canvas_count: 2,
            first_canvas_id: "sui-main-canvas".to_string(),
            first_canvas_width: 1920,
            first_canvas_height: 1080,
            first_canvas_data_url_len: 128,
        };
        let report = web_canvas_capture_report(&mode, &capture);

        assert!(report.contains("route=color-validation"));
        assert!(report.contains("canvas_count=2"));
        assert!(report.contains("first_canvas_id=sui-main-canvas"));
        assert!(report.contains("first_canvas_size=1920x1080"));
        assert!(report.contains("first_canvas_data_url_len=128"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_defaults_to_vsync() {
        let mode = parse_desktop_launch_mode(Vec::<&str>::new(), false, None).unwrap();
        assert!(mode.vsync_enabled);
        assert_eq!(mode.automation, None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_honors_no_vsync_sources() {
        let cli_mode = parse_desktop_launch_mode(["--no-vsync"], false, None).unwrap();
        let env_mode = parse_desktop_launch_mode(Vec::<&str>::new(), true, None).unwrap();

        assert!(!cli_mode.vsync_enabled);
        assert!(!env_mode.vsync_enabled);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_allows_cli_override_back_to_vsync() {
        let mode = parse_desktop_launch_mode(["--vsync"], true, None).unwrap();
        assert!(mode.vsync_enabled);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_accepts_automation_sources() {
        let cli_mode =
            parse_desktop_launch_mode(["--automation=button-grid-resize"], false, None).unwrap();
        let env_mode = parse_desktop_launch_mode(
            Vec::<&str>::new(),
            false,
            Some(DesktopLaunchAutomation::ButtonGridResize),
        )
        .unwrap();

        assert_eq!(
            cli_mode.automation,
            Some(DesktopLaunchAutomation::ButtonGridResize)
        );
        assert_eq!(
            env_mode.automation,
            Some(DesktopLaunchAutomation::ButtonGridResize)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_accepts_widget_book_scroll_automation() {
        let mode =
            parse_desktop_launch_mode(["--automation=widget-book-scroll"], false, None).unwrap();
        assert_eq!(
            mode.automation,
            Some(DesktopLaunchAutomation::WidgetBookScroll)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_rejects_unknown_flags() {
        let error = parse_desktop_launch_mode(["--bogus"], false, None).unwrap_err();
        assert!(error.to_string().contains("unsupported sui-dev argument"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_launch_mode_rejects_unknown_automation() {
        let error = parse_desktop_automation(Some("bogus")).unwrap_err();
        assert!(error.to_string().contains("unsupported sui-dev automation"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_no_vsync_env_parser_understands_falsey_values() {
        assert!(!env_requests_no_vsync(None));
        assert!(!env_requests_no_vsync(Some("0")));
        assert!(!env_requests_no_vsync(Some("false")));
        assert!(env_requests_no_vsync(Some("1")));
        assert!(env_requests_no_vsync(Some("true")));
    }

    #[test]
    fn clamps_invalid_frame_counts() {
        let mode = parse_web_launch_mode("benchmark=retained-text&frames=0&warmup=999999");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::RetainedText));
        assert_eq!(mode.frames, 1);
        assert_eq!(mode.warmup_frames, 2000);
    }
}
