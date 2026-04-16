mod app;

pub use app::{build_dev_application, build_dev_application_with_widget_book_bounds};

use sui::Application;
use sui_widget_book::{
    build_button_grid_benchmark_application, build_retained_text_benchmark_application,
    build_text_editing_benchmark_application, build_widget_book_application,
    default_widget_book_state,
};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebBenchmarkKind {
    ButtonGrid,
    RetainedText,
    TextEditing,
    WidgetBook,
    DevWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebLaunchMode {
    benchmark: Option<WebBenchmarkKind>,
    frames: usize,
    warmup_frames: usize,
}

impl Default for WebLaunchMode {
    fn default() -> Self {
        Self {
            benchmark: None,
            frames: 180,
            warmup_frames: 60,
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
                    "widget-book" => Some(WebBenchmarkKind::WidgetBook),
                    "dev" | "workspace" => Some(WebBenchmarkKind::DevWorkspace),
                    _ => None,
                };
            }
            "frames" => {
                mode.frames = value.parse::<usize>().unwrap_or(mode.frames).clamp(1, 10_000);
            }
            "warmup" | "warmup-frames" => {
                mode.warmup_frames = value
                    .parse::<usize>()
                    .unwrap_or(mode.warmup_frames)
                    .clamp(0, 2_000);
            }
            _ => {}
        }
    }

    mode
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn build_application_for_web_mode(mode: &WebLaunchMode) -> Application {
    match mode.benchmark {
        Some(WebBenchmarkKind::ButtonGrid) => build_button_grid_benchmark_application(),
        Some(WebBenchmarkKind::RetainedText) => build_retained_text_benchmark_application(),
        Some(WebBenchmarkKind::TextEditing) => build_text_editing_benchmark_application(),
        Some(WebBenchmarkKind::WidgetBook) => {
            build_widget_book_application(default_widget_book_state())
        }
        Some(WebBenchmarkKind::DevWorkspace) | None => build_dev_application(),
    }
}

pub fn run_desktop() -> sui::Result<()> {
    build_dev_application().run()
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
    fn clamps_invalid_frame_counts() {
        let mode = parse_web_launch_mode("benchmark=retained-text&frames=0&warmup=999999");
        assert_eq!(mode.benchmark, Some(WebBenchmarkKind::RetainedText));
        assert_eq!(mode.frames, 1);
        assert_eq!(mode.warmup_frames, 2000);
    }
}
