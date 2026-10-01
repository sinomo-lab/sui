use sui::diagnostics::WindowPerformanceSnapshot;
use sui::{
    Application, DefaultTheme, Result, SemanticsRole, SemanticsValue, Size, SizedBox, WindowBuilder,
};
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;

#[test]
fn retained_text_benchmark_exposes_vertical_scroll_bar() -> Result<()> {
    let mut runtime = build_retained_text_benchmark_runtime()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;

    let scroll = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(RETAINED_TEXT_BENCHMARK_SCROLL_NAME)
        })
        .expect("retained text scroll view should be present");
    let scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME)
        })
        .expect("retained text vertical scroll bar should be present");
    let max = match scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };

    assert!(max > 0.0);
    assert!(scroll_bar.bounds.x() >= scroll.bounds.max_x());
    Ok(())
}

#[test]
fn retained_text_benchmark_scroll_bar_uses_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        RETAINED_TEXT_BENCHMARK_TITLE,
        Size::new(520.0, 360.0),
        super::build_retained_text_benchmark_with_theme(theme_reader(theme)),
    );
    let scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME)
        })
        .expect("retained text vertical scroll bar should be present");

    assert_eq!(
        scroll_bar.bounds.width(),
        theme.metrics.scroll_bar_thickness
    );
}

fn build_retained_text_benchmark_runtime() -> Result<sui::Runtime> {
    Application::new()
        .window(
            WindowBuilder::new()
                .title(RETAINED_TEXT_BENCHMARK_TITLE)
                .root(
                    SizedBox::new()
                        .size(Size::new(520.0, 360.0))
                        .with_child(super::build_retained_text_benchmark()),
                ),
        )
        .build()
}

#[cfg(feature = "artifacts")]
fn collect_headless_animation_benchmark_samples(
    window: &TestWindow,
) -> Result<Vec<WindowPerformanceSnapshot>> {
    const WARMUP_FRAMES: usize = 12;
    const MEASURED_FRAMES: usize = 120;
    const FRAME_DELTA_SECONDS: f64 = 1.0 / 60.0;

    for name in [
        ANIMATION_BENCHMARK_RETAINED_NAME,
        ANIMATION_BENCHMARK_REPAINT_NAME,
        ANIMATION_BENCHMARK_SCALE_NAME,
    ] {
        window
            .get_by_role(SemanticsRole::Button)
            .with_name(name)
            .click()?;
    }

    let mut collected = Vec::with_capacity(MEASURED_FRAMES);
    let mut previous_frame_index = window.performance_snapshot()?.frame_index;
    for step in 0..(WARMUP_FRAMES + MEASURED_FRAMES) {
        window.advance_time(FRAME_DELTA_SECONDS)?;
        let snapshot = next_headless_benchmark_frame(
            window,
            &mut previous_frame_index,
            "headless animation benchmark",
            "animation frame",
            step,
        )?;
        if step >= WARMUP_FRAMES {
            collected.push(snapshot);
        }
    }

    Ok(collected)
}

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "diagnostic benchmark for current headless retained text scroll status"]
fn retained_text_headless_scroll_current_status_benchmark() -> Result<()> {
    let _guard = headless_benchmark_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let app = TestApp::from_runtime(build_retained_text_benchmark_application().build()?)?;
    let window = app.main_window()?;
    let snapshot = window.snapshot()?;
    assert_eq!(snapshot.title, RETAINED_TEXT_BENCHMARK_TITLE);
    set_detailed_scene_statistics_mode(&window)?;
    let samples = collect_headless_scroll_benchmark_samples(
        &window,
        RETAINED_TEXT_BENCHMARK_SCROLL_NAME,
        24,
    )?;

    print_headless_benchmark_summary("Retained Text Headless Scroll Benchmark", &samples);
    Ok(())
}

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "diagnostic benchmark for current headless animation status"]
fn animation_headless_current_status_benchmark() -> Result<()> {
    let _guard = headless_benchmark_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let app = TestApp::from_runtime(build_animation_benchmark_application().build()?)?;
    let window = app.main_window()?;
    let snapshot = window.snapshot()?;
    assert_eq!(snapshot.title, ANIMATION_BENCHMARK_TITLE);
    set_detailed_scene_statistics_mode(&window)?;
    let samples = collect_headless_animation_benchmark_samples(&window)?;

    print_headless_benchmark_summary("Animation Headless Benchmark", &samples);
    Ok(())
}
