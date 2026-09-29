use sui::{
    Application, DefaultTheme, Event, ImeEvent, Result, SemanticsRole, SemanticsValue, Size,
    SizedBox, TextSurfaceOverlayKind, Vector, WindowBuilder, WindowPerformanceSnapshot,
};
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;

#[test]
fn text_editing_benchmark_exercises_rich_code_style_ranges() {
    let document = text_editing_benchmark_document();
    let spans = text_editing_benchmark_style_spans(&document);
    let overlays = text_editing_benchmark_style_overlays(&document);

    assert!(spans.len() > 500);
    assert!(
        overlays
            .iter()
            .any(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::SearchMatch))
    );
    assert!(
        overlays
            .iter()
            .any(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::Diagnostic))
    );
    let emoji_previews = overlays
        .iter()
        .filter(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::RichTextPreview))
        .collect::<Vec<_>>();
    assert!(!emoji_previews.is_empty());
    assert!(
        emoji_previews
            .iter()
            .all(|overlay| document.get(overlay.range.clone()) == Some("\u{1f642}"))
    );
    assert!(spans.iter().all(
        |span| span.range.start < span.range.end && document.get(span.range.clone()).is_some()
    ));
    assert!(
        overlays
            .iter()
            .all(|overlay| overlay.range.start < overlay.range.end
                && document.get(overlay.range.clone()).is_some())
    );

    let (preview, preview_spans) = text_editing_syntax_preview_content(DefaultTheme::default());
    assert_eq!(preview.lines().count(), 220);
    assert!(preview_spans.len() > 1_500);
    assert!(
        preview_spans
            .iter()
            .all(|span| span.range.start < span.range.end && span.range.end <= preview.len())
    );
}

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

#[test]
fn text_editing_benchmark_exposes_named_splitter() -> Result<()> {
    let mut runtime = build_text_editing_benchmark_runtime()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;

    let splitter = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Splitter
                && node.name.as_deref() == Some(TEXT_EDITING_BENCHMARK_SPLIT_NAME)
        })
        .expect("text editing splitter should be present");
    let editor = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(TEXT_EDITING_BENCHMARK_EDITOR_NAME)
        })
        .expect("text editing editor should be present");
    let syntax_preview = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME)
        })
        .expect("text editing syntax preview should be present");

    assert!(matches!(
        splitter.value,
        Some(SemanticsValue::Number(value)) if (value - 0.54).abs() < 0.01
    ));
    assert!(editor.bounds.max_x() <= syntax_preview.bounds.x());
    Ok(())
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

fn build_text_editing_benchmark_runtime() -> Result<sui::Runtime> {
    Application::new()
        .window(
            WindowBuilder::new()
                .title(TEXT_EDITING_BENCHMARK_TITLE)
                .root(
                    SizedBox::new()
                        .size(Size::new(900.0, 520.0))
                        .with_child(super::build_text_editing_benchmark()),
                ),
        )
        .build()
}

#[cfg(feature = "artifacts")]
fn collect_headless_text_editing_benchmark_samples(
    window: &TestWindow,
) -> Result<Vec<WindowPerformanceSnapshot>> {
    const EDIT_COMMITS: [&str; 10] = [
        " // typed atlas reuse",
        "\nlet pending_frame = cache_hits + 1;",
        "\n// bidi check: abc אבג 123 مرحبا",
        "\nlet emoji = \"🙂✅🎨\";",
        "\nlet ime_probe = \"候補\";",
        "\nlet syntax_band = highlight_rows.len();",
        "\n// fallback sample: Ж 中 नमस्ते",
        "\nrecord_selection_delta(cursor, viewport);",
        "\nlet scroll_budget_ms = 16.67;",
        "\ncommit_overlay_sample(frame_index);",
    ];
    const IME_PREEDIT_UPDATES: [(&str, Option<(usize, usize)>); 3] = [
        ("候", Some((0, 1))),
        ("候補", Some((1, 2))),
        ("候補を", Some((2, 3))),
    ];
    const EDITOR_SCROLL_FRAMES: usize = 18;
    const SYNTAX_SCROLL_FRAMES: usize = 28;
    const SCROLL_STEP_PX: f32 = -34.0;

    let editor = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_EDITING_BENCHMARK_EDITOR_NAME);
    let syntax_scroll = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME);
    editor.focus()?;

    let mut collected = Vec::with_capacity(
        IME_PREEDIT_UPDATES.len()
            + 1
            + EDIT_COMMITS.len()
            + EDITOR_SCROLL_FRAMES
            + SYNTAX_SCROLL_FRAMES,
    );
    let mut previous_frame_index = window.performance_snapshot()?.frame_index;

    editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
    for (step, (text, cursor_range)) in IME_PREEDIT_UPDATES.iter().enumerate() {
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
            text: (*text).to_string(),
            cursor_range: cursor_range.map(|(start, end)| start..end),
        }))?;
        collected.push(next_headless_benchmark_frame(
            window,
            &mut previous_frame_index,
            "headless text editing benchmark",
            "composition preedit",
            step,
        )?);
    }
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit {
        text: "候補を".to_string(),
    }))?;
    collected.push(next_headless_benchmark_frame(
        window,
        &mut previous_frame_index,
        "headless text editing benchmark",
        "composition commit",
        IME_PREEDIT_UPDATES.len(),
    )?);
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;

    for (step, text) in EDIT_COMMITS.iter().enumerate() {
        let text = (*text).to_string();
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
            text: text.clone(),
            cursor_range: None,
        }))?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit { text }))?;
        editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;
        collected.push(next_headless_benchmark_frame(
            window,
            &mut previous_frame_index,
            "headless text editing benchmark",
            "typing",
            step,
        )?);
    }

    for step in 0..EDITOR_SCROLL_FRAMES {
        editor.scroll_pixels(Vector::new(0.0, SCROLL_STEP_PX))?;
        collected.push(next_headless_benchmark_frame(
            window,
            &mut previous_frame_index,
            "headless text editing benchmark",
            "editor scroll",
            step,
        )?);
    }

    for step in 0..SYNTAX_SCROLL_FRAMES {
        syntax_scroll.scroll_pixels(Vector::new(0.0, SCROLL_STEP_PX))?;
        collected.push(next_headless_benchmark_frame(
            window,
            &mut previous_frame_index,
            "headless text editing benchmark",
            "syntax scroll",
            step,
        )?);
    }

    Ok(collected)
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
#[ignore = "diagnostic benchmark for current headless text editing status"]
fn text_editing_headless_current_status_benchmark() -> Result<()> {
    let _guard = headless_benchmark_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let app = TestApp::from_runtime(build_text_editing_benchmark_application().build()?)?;
    let window = app.main_window()?;
    let snapshot = window.snapshot()?;
    assert_eq!(snapshot.title, TEXT_EDITING_BENCHMARK_TITLE);
    set_detailed_scene_statistics_mode(&window)?;
    let samples = collect_headless_text_editing_benchmark_samples(&window)?;

    print_headless_benchmark_summary("Text Editing Headless Benchmark", &samples);
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

#[test]
fn text_editing_benchmark_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        TEXT_EDITING_BENCHMARK_TITLE,
        Size::new(900.0, 520.0),
        build_text_editing_benchmark_with_theme,
    )
}
