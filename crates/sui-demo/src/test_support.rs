//! Helpers shared by the demo crate's unit tests: render and scene
//! inspection, theme readers, screenshot diffs, and headless benchmarks.

use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use sui::{
    Application, DefaultTheme, Event, RenderOutput, Result, SceneStatisticsDetailMode,
    SemanticsRole, Size, SizedBox, Vector, Widget, WindowBuilder, WindowEvent,
    WindowPerformanceSnapshot, set_window_scene_statistics_detail_mode,
};
use sui_scene::{Brush, SceneCommand};
use sui_testing::prelude::*;

#[cfg(feature = "artifacts")]
pub(crate) fn headless_benchmark_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn theme_reader(theme: DefaultTheme) -> crate::app::DevThemeReader {
    Rc::new(move || theme)
}

pub(crate) fn mutable_theme_reader(theme: Rc<RefCell<DefaultTheme>>) -> crate::app::DevThemeReader {
    Rc::new(move || *theme.borrow())
}

pub(crate) fn assert_widget_repaints_after_theme_change<W, B>(
    title: &str,
    size: Size,
    build: B,
) -> Result<()>
where
    W: Widget + 'static,
    B: FnOnce(crate::app::DevThemeReader) -> W,
{
    let theme = Rc::new(RefCell::new(DefaultTheme::default()));
    let child = build(mutable_theme_reader(Rc::clone(&theme)));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title(title)
                .root(SizedBox::new().size(size).with_child(child)),
        )
        .build()?;
    let window_id = runtime.window_ids()[0];
    let light = runtime.render(window_id)?;

    *theme.borrow_mut() = DefaultTheme::dark();
    runtime.handle_event(window_id, Event::Window(WindowEvent::Resized(size)))?;
    let dark = runtime.render(window_id)?;

    assert_ne!(
        light.frame.scene, dark.frame.scene,
        "{title} should repaint when the shared theme reader changes"
    );
    Ok(())
}

pub(crate) fn render_widget_with_size<W>(title: &str, size: Size, child: W) -> RenderOutput
where
    W: Widget + 'static,
{
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title(title)
                .root(SizedBox::new().size(size).with_child(child)),
        )
        .build()
        .expect("themed widget runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("themed widget should render")
}

pub(crate) fn assert_semantics_omit_live_performance_overlay(semantics: &[sui::SemanticsNode]) {
    assert!(
        semantics
            .iter()
            .all(|node| node.name.as_deref() != Some("Live performance overlay")),
        "expected semantics tree to omit the floating live performance overlay outside sui-demo"
    );
}

#[cfg(feature = "artifacts")]
pub(crate) fn unique_visual_artifact_test_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sui-demo-widget-book-artifacts-{}-{}-{}",
        std::process::id(),
        nonce,
        name
    ))
}

pub(crate) fn solid_fill_colors(output: &RenderOutput) -> Vec<sui::Color> {
    let mut colors = Vec::new();
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::FillRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::FillPath {
                brush: Brush::Solid(color),
                ..
            } => colors.push(*color),
            _ => {}
        });
    colors
}

#[cfg(feature = "artifacts")]
pub(crate) fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((sorted.len() - 1) as f64 * quantile).round() as usize;
    sorted[rank]
}

#[cfg(feature = "artifacts")]
pub(crate) fn print_headless_benchmark_summary(label: &str, samples: &[WindowPerformanceSnapshot]) {
    let frame_count = samples.len().max(1) as f64;
    let mut totals = samples
        .iter()
        .map(|sample| sample.total_time_ms)
        .collect::<Vec<_>>();
    totals.sort_by(|a, b| a.total_cmp(b));
    let avg_total_ms = totals.iter().sum::<f64>() / frame_count;
    let avg_visible_layers = samples
        .iter()
        .map(|sample| sample.renderer_submission.visible_layer_count as f64)
        .sum::<f64>()
        / frame_count;
    let avg_direct_packets = samples
        .iter()
        .map(|sample| sample.renderer_submission.direct_packet_count as f64)
        .sum::<f64>()
        / frame_count;
    let avg_packet_rebuilds = samples
        .iter()
        .map(|sample| {
            sample
                .renderer_submission
                .retained_packet_rebuilds
                .total_count() as f64
        })
        .sum::<f64>()
        / frame_count;
    let avg_scene_layers = samples
        .iter()
        .map(|sample| sample.scene.scene_layer_count as f64)
        .sum::<f64>()
        / frame_count;
    let avg_repaint_boundaries = samples
        .iter()
        .map(|sample| sample.scene.repaint_boundary_count as f64)
        .sum::<f64>()
        / frame_count;
    let avg_dirty_coverage = samples
        .iter()
        .map(|sample| sample.scene.dirty_coverage as f64)
        .sum::<f64>()
        / frame_count;
    let max_total_ms = totals.last().copied().unwrap_or(0.0);

    println!("\n=== {label} ===");
    println!("frames:                 {}", samples.len());
    println!(
        "avg frame time:         {avg_total_ms:.3} ms ({:.1} fps)",
        1000.0 / avg_total_ms.max(0.001)
    );
    println!(
        "p95 frame time:         {:.3} ms",
        percentile(&totals, 0.95)
    );
    println!("max frame time:         {max_total_ms:.3} ms");
    println!("avg visible layers:     {avg_visible_layers:.2}");
    println!("avg direct packets:     {avg_direct_packets:.2}");
    println!("avg packet rebuilds:    {avg_packet_rebuilds:.2}");
    println!("avg repaint boundaries: {avg_repaint_boundaries:.2}");
    println!("avg scene layers:       {avg_scene_layers:.2}");
    println!("avg dirty coverage:     {avg_dirty_coverage:.2}%");

    // Per-frame-phase breakdown (Event / MeasureArrange / Paint / Renderer / ...).
    // Shows where wall-clock time actually goes within a frame.
    let mut phase_totals: std::collections::BTreeMap<&'static str, f64> =
        std::collections::BTreeMap::new();
    for sample in samples {
        for timing in &sample.phase_timings {
            *phase_totals.entry(timing.phase.label()).or_default() += timing.duration_ms;
        }
    }
    if !phase_totals.is_empty() {
        let mut phases = phase_totals
            .into_iter()
            .map(|(label, total)| (label, total / frame_count))
            .collect::<Vec<_>>();
        phases.sort_by(|a, b| b.1.total_cmp(&a.1));
        println!("--- avg frame-phase breakdown ---");
        for (label, avg_ms) in phases {
            let pct = if avg_total_ms > 0.0 {
                (avg_ms / avg_total_ms) * 100.0
            } else {
                0.0
            };
            println!("  {label:<22} {avg_ms:>8.3} ms ({pct:>5.1}%)");
        }
    }

    // Per-widget measure/arrange/paint timings, only populated when the runtime
    // env var SUI_PROFILE_WIDGET_TIMINGS is set. Surfaces the hottest widgets.
    let mut widget_totals: std::collections::BTreeMap<(&'static str, &'static str), (f64, usize)> =
        std::collections::BTreeMap::new();
    for sample in samples {
        for timing in &sample.widget_timings {
            let entry = widget_totals
                .entry((timing.widget_name, timing.phase.label()))
                .or_default();
            entry.0 += timing.duration_ms;
            entry.1 += timing.calls;
        }
    }
    if !widget_totals.is_empty() {
        let mut widgets = widget_totals
            .into_iter()
            .map(|((name, phase), (total, calls))| {
                (name, phase, total / frame_count, calls as f64 / frame_count)
            })
            .collect::<Vec<_>>();
        widgets.sort_by(|a, b| b.2.total_cmp(&a.2));
        println!("--- top widget timings (avg/frame) ---");
        for (name, phase, avg_ms, avg_calls) in widgets.into_iter().take(15) {
            println!("  {name:<28} {phase:<8} {avg_ms:>8.4} ms  x{avg_calls:>6.1}");
        }
    }

    let text_requests = samples
        .iter()
        .map(|sample| sample.runtime_text_timing.request_count)
        .sum::<usize>();
    if text_requests > 0 {
        let text_hits = samples
            .iter()
            .map(|sample| sample.runtime_text_timing.cache_hit_count)
            .sum::<usize>();
        let text_misses = samples
            .iter()
            .map(|sample| sample.runtime_text_timing.cache_miss_count)
            .sum::<usize>();
        let text_total_us = samples
            .iter()
            .map(|sample| sample.runtime_text_timing.total_time_us)
            .sum::<u64>();
        let text_miss_layout_us = samples
            .iter()
            .map(|sample| sample.runtime_text_timing.miss_layout_time_us)
            .sum::<u64>();
        println!("--- text layout totals ---");
        println!("  requests: {text_requests} ({text_hits} hits, {text_misses} misses)");
        println!(
            "  total: {:.3} ms, miss layout: {:.3} ms",
            text_total_us as f64 / 1_000.0,
            text_miss_layout_us as f64 / 1_000.0,
        );
    }
}

#[cfg(feature = "artifacts")]
pub(crate) fn set_detailed_scene_statistics_mode(window: &TestWindow) -> Result<()> {
    set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
    window.run_until_idle()
}

#[cfg(feature = "artifacts")]
pub(crate) fn collect_headless_scroll_benchmark_samples(
    window: &TestWindow,
    scroll_name: &str,
    samples: usize,
) -> Result<Vec<WindowPerformanceSnapshot>> {
    let scroll = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(scroll_name);
    let mut collected = Vec::with_capacity(samples);
    let mut previous_frame_index = 0;
    let mut attempts = 0;
    let max_attempts = samples * 8;
    while collected.len() < samples && attempts < max_attempts {
        scroll.scroll_pixels(Vector::new(0.0, -180.0))?;
        let snapshot = window.performance_snapshot()?;
        if snapshot.frame_index > previous_frame_index {
            previous_frame_index = snapshot.frame_index;
            collected.push(snapshot);
        }
        attempts += 1;
    }
    assert_eq!(
        collected.len(),
        samples,
        "headless scroll benchmark collected {} frames after {} attempts",
        collected.len(),
        attempts,
    );
    Ok(collected)
}

#[cfg(feature = "artifacts")]
pub(crate) fn next_headless_benchmark_frame(
    window: &TestWindow,
    previous_frame_index: &mut u64,
    benchmark_name: &str,
    stage: &str,
    step: usize,
) -> Result<WindowPerformanceSnapshot> {
    let snapshot = window.performance_snapshot()?;
    if snapshot.frame_index <= *previous_frame_index {
        return Err(sui::Error::new(format!(
            "{benchmark_name} did not render a new frame during {stage} step {}",
            step + 1,
        )));
    }

    *previous_frame_index = snapshot.frame_index;
    Ok(snapshot)
}

#[cfg(feature = "artifacts")]
pub(crate) const SCREENSHOT_CHANNEL_TOLERANCE: u8 = 1;

#[cfg(feature = "artifacts")]
pub(crate) fn screenshot_pixels_match(left: &[u8], right: &[u8]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| left.abs_diff(*right) <= SCREENSHOT_CHANNEL_TOLERANCE)
}

#[cfg(feature = "artifacts")]
pub(crate) fn screenshot_diff_count(
    left: &sui_testing::Screenshot,
    right: &sui_testing::Screenshot,
) -> usize {
    assert_eq!(left.width(), right.width(), "screenshot widths differ");
    assert_eq!(left.height(), right.height(), "screenshot heights differ");

    left.pixels()
        .chunks_exact(4)
        .zip(right.pixels().chunks_exact(4))
        .filter(|(left_px, right_px)| !screenshot_pixels_match(left_px, right_px))
        .count()
}

#[cfg(feature = "artifacts")]
pub(crate) fn screenshot_diff_image(
    left: &sui_testing::Screenshot,
    right: &sui_testing::Screenshot,
) -> Result<sui_testing::Screenshot> {
    assert_eq!(left.width(), right.width(), "screenshot widths differ");
    assert_eq!(left.height(), right.height(), "screenshot heights differ");

    let pixels = left
        .pixels()
        .chunks_exact(4)
        .zip(right.pixels().chunks_exact(4))
        .flat_map(|(left_px, right_px)| {
            if screenshot_pixels_match(left_px, right_px) {
                [left_px[0], left_px[1], left_px[2], 96]
            } else {
                [255, 0, 0, 255]
            }
        })
        .collect::<Vec<_>>();

    sui_testing::Screenshot::new(left.width(), left.height(), pixels)
}

#[cfg(feature = "artifacts")]
#[test]
pub(crate) fn screenshot_diff_helpers_tolerate_one_channel_value_per_channel() -> Result<()> {
    let left = sui_testing::Screenshot::new(2, 1, vec![10, 20, 30, 40, 100, 110, 120, 130])?;
    let right = sui_testing::Screenshot::new(2, 1, vec![11, 19, 31, 39, 99, 111, 119, 131])?;

    assert_eq!(screenshot_diff_count(&left, &right), 0);

    let diff = screenshot_diff_image(&left, &right)?;
    assert_eq!(diff.pixels(), &[10, 20, 30, 96, 100, 110, 120, 96]);

    Ok(())
}
