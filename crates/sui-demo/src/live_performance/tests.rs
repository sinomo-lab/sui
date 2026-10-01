use std::{cell::RefCell, rc::Rc};

use sui::diagnostics::{
    FramePhase, FramePhaseSample, PresentationLatencyDiagnostics, RendererSubmissionDiagnostics,
    SceneStatistics, SceneStatisticsDetailMode, TextCacheDeltaDiagnostics, TextCacheDiagnostics,
    WindowPerformanceSnapshot, window_scene_statistics_detail_mode,
};
use sui::{
    App, Application, DefaultTheme, Event, Result, SemanticsRole, SemanticsValue, Size, Vector,
    Widget, WidgetPod, WidgetPodVisitor, Window, WindowBuilder, WindowEvent, WindowId,
};
use sui_runtime::publish_window_performance_snapshot;
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;
use crate::widget_book::{
    GALLERY_SCROLL_NAME, WINDOW_DESCRIPTION, WINDOW_TITLE, build_widget_book_application,
    build_widget_book_gallery, register_widget_book_images,
};

fn build_default_widget_book_app() -> Result<TestApp> {
    TestApp::new(|| build_widget_book_application_with_overlay().build())
}

fn build_widget_book_application_with_overlay() -> Application {
    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(
            Window::new(WINDOW_TITLE).root(
                LivePerformanceRoot::new(
                    WINDOW_TITLE,
                    WINDOW_DESCRIPTION,
                    build_widget_book_gallery(),
                )
                .show_performance_overlay(),
            ),
        )
        .into_application()
}

fn build_overlay_placeholder_app() -> Result<TestApp> {
    TestApp::new(|| {
        Application::new()
            .window(
                WindowBuilder::new()
                    .title("Overlay")
                    .root(LivePerformancePanel::new()),
            )
            .build()
    })
}

#[test]
fn widget_book_application_omits_live_performance_overlay() {
    let mut runtime = build_widget_book_application()
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("widget book should render");

    let semantics = runtime
        .semantics(window_id)
        .expect("widget book semantics should exist");
    assert_semantics_omit_live_performance_overlay(semantics);
}

#[test]
fn live_performance_frame_sample_records_snapshot_phase_costs() {
    let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
    assert!(display.borrow().samples.is_empty());

    let snapshot = sample_detailed_window_performance_snapshot_record(WindowId::new(11));
    display
        .borrow_mut()
        .samples
        .push(LivePerformanceFrameSample::from_snapshot(&snapshot));
    let sample = display.borrow().samples[0].clone();

    assert_eq!(sample.frame_index, snapshot.frame_index);
    assert_eq!(
        sample.stage_costs[frame_phase_index(FramePhase::Paint)],
        0.8
    );
    assert_eq!(
        sample.stage_costs[frame_phase_index(FramePhase::Renderer)],
        1.9
    );
}

#[test]
fn live_performance_panel_does_not_create_child_widgets_when_snapshot_updates() {
    struct CountingVisitor {
        count: usize,
    }

    impl WidgetPodVisitor for CountingVisitor {
        fn visit(&mut self, _child: &WidgetPod) {
            self.count += 1;
        }
    }

    let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
    let panel = LivePerformancePanel::with_display(Rc::clone(&display));
    let mut visitor = CountingVisitor { count: 0 };
    Widget::visit_children(&panel, &mut visitor);
    assert_eq!(visitor.count, 0);

    display.borrow_mut().snapshot =
        Some(sample_window_performance_snapshot_record(WindowId::new(11)));

    let mut visitor = CountingVisitor { count: 0 };
    Widget::visit_children(&panel, &mut visitor);
    assert_eq!(visitor.count, 0);
}

#[test]
fn live_performance_panel_measures_to_compact_width() {
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Overlay")
                .root(LivePerformancePanel::new()),
        )
        .build()
        .expect("runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime.render(window_id).expect("panel should render");
    let graph = runtime
        .widget_graph(window_id)
        .expect("widget graph should exist");
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .expect("panel root node present");

    assert!(root.bounds.width() <= LivePerformancePanel::WIDTH);
    assert!(root.bounds.height() > 0.0);
}

#[test]
fn live_performance_panel_uses_theme_text_tokens_and_font_stack() {
    let theme = DefaultTheme::default();
    let caption = LivePerformancePanel::caption_text_style(sui::Color::WHITE);
    let headline = LivePerformancePanel::headline_text_style(sui::Color::WHITE);

    assert_eq!(caption.font_size, theme.text.xs.size);
    assert_eq!(caption.line_height, theme.text.xs.line_height);
    assert_eq!(headline.font_size, theme.text._2xl.size);
    assert_eq!(headline.line_height, theme.text._2xl.line_height);
    assert_eq!(caption.font_families, theme.body_text_style().font_families);
    assert_eq!(
        headline.font_families,
        theme.body_text_style().font_families
    );
}

#[test]
fn live_performance_panel_reports_frame_cadence_separately_from_work() {
    let mut snapshot = sample_window_performance_snapshot_record(WindowId::new(11));
    snapshot.frame_interval_ms = Some(50.0);
    let display = Rc::new(RefCell::new(LivePerformanceDisplay {
        snapshot: Some(snapshot.clone()),
        idle: false,
        samples: vec![LivePerformanceFrameSample::from_snapshot(&snapshot)],
    }));
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Overlay")
                .root(LivePerformancePanel::with_display(display)),
        )
        .build()
        .unwrap();
    let output = runtime.render(runtime.window_ids()[0]).unwrap();
    let overlay = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("Live performance overlay"))
        .unwrap();
    assert_eq!(
        overlay.value,
        Some(SemanticsValue::Text(
            "20 fps | 1.5 ms | 1 samples".to_string()
        ))
    );
    assert_eq!(
        super::format_fps(None),
        "-- fps",
        "the first frame has no measured cadence"
    );
}

#[test]
fn live_performance_panel_reports_zero_fps_when_idle() {
    let snapshot = sample_window_performance_snapshot_record(WindowId::new(11));
    let display = Rc::new(RefCell::new(LivePerformanceDisplay {
        snapshot: Some(snapshot.clone()),
        idle: true,
        samples: vec![LivePerformanceFrameSample::from_snapshot(&snapshot)],
    }));
    let panel = LivePerformancePanel::with_display(display);
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Overlay").root(panel))
        .build()
        .expect("runtime should build");
    let window_id = runtime.window_ids()[0];

    runtime.render(window_id).expect("panel should render");
    let semantics = runtime
        .semantics(window_id)
        .expect("semantics snapshot should exist");
    let overlay = semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Live performance overlay")
        })
        .expect("overlay semantics node present");

    assert_eq!(
        overlay.value,
        Some(SemanticsValue::Text(
            "0 fps | 1.5 ms | 1 samples".to_string()
        ))
    );
}

#[test]
fn widget_book_root_requests_paint_when_a_published_snapshot_arrives() {
    let mut runtime = build_widget_book_application()
        .build()
        .expect("runtime should build");
    let window_id = runtime.window_ids()[0];

    runtime
        .render(window_id)
        .expect("initial render should succeed");
    assert!(
        !runtime
            .needs_render(window_id)
            .expect("window should be idle after initial render")
    );

    publish_window_performance_snapshot(sample_window_performance_snapshot_record(window_id));
    runtime
        .handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))
        .expect("redraw event should be handled");

    assert!(runtime.needs_render(window_id).expect(
        "widget-book root should request a paint when the published performance snapshot changes"
    ));
}

#[test]
fn widget_book_startup_bootstraps_live_performance_overlay() -> Result<()> {
    let placeholder_image = {
        let placeholder = build_overlay_placeholder_app()?;
        let placeholder_window = placeholder.main_window()?;
        placeholder_window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay")
            .capture_screenshot()?
    };

    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let overlay = window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name("Live performance overlay");

    let live_image = overlay.capture_screenshot()?;
    let performance = window.performance_snapshot()?;

    assert_ne!(live_image, placeholder_image);
    assert!(performance.frame_index >= 2);

    Ok(())
}

#[test]
fn widget_book_overlay_enables_detail_mode_while_visible() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let overlay = window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name("Live performance overlay");
    let before = overlay.capture_screenshot()?;
    window
        .root()
        .dispatch_event(Event::Window(WindowEvent::RedrawRequested))?;
    let after = overlay.capture_screenshot()?;
    assert_eq!(
        window_scene_statistics_detail_mode(window.id()),
        SceneStatisticsDetailMode::Detailed,
        "visible overlay should enable detailed scene statistics mode"
    );
    assert!(
        before != after,
        "overlay screenshot did not change after publishing detailed diagnostics"
    );

    Ok(())
}

#[test]
fn widget_book_scroll_updates_performance_overlay_without_extra_frame() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);
    let overlay = window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name("Live performance overlay");
    let before = overlay.capture_screenshot()?;
    gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
    let after = overlay.capture_screenshot()?;
    assert_ne!(after, before);

    Ok(())
}

#[test]
fn widget_book_scroll_updates_performance_overlay_visuals() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);
    let overlay = window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name("Live performance overlay");

    let before = overlay.capture_screenshot()?;
    gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
    let after = overlay.capture_screenshot()?;

    assert_ne!(before, after);

    Ok(())
}

#[test]
fn widget_book_exposes_compact_performance_overlay_semantics() {
    let mut runtime = build_widget_book_application_with_overlay()
        .build()
        .expect("runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("widget book should render");
    let semantics = runtime
        .semantics(window_id)
        .expect("semantics snapshot should exist");

    let overlay = semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some("Live performance overlay")
        })
        .expect("overlay semantics node present");

    let expected_left_edge =
        1280.0 - super::LivePerformanceRoot::OVERLAY_MARGIN.right - LivePerformancePanel::WIDTH;
    assert!(overlay.bounds.width() <= LivePerformancePanel::WIDTH);
    assert!(overlay.bounds.x() >= expected_left_edge);
    assert!(
        overlay.bounds.max_x() <= 1280.0 - super::LivePerformanceRoot::OVERLAY_MARGIN.right + 1.0
    );
    assert!(overlay.bounds.y() <= 24.0);
}

fn sample_window_performance_snapshot_record(window_id: WindowId) -> WindowPerformanceSnapshot {
    WindowPerformanceSnapshot::new(
        window_id,
        7,
        vec![FramePhaseSample::new(FramePhase::Renderer, 1.5)],
        RendererSubmissionDiagnostics::new(
            2,
            6,
            2048,
            24,
            1536,
            3,
            6,
            420,
            160,
            210,
            120,
            3,
            sui_runtime::RetainedPacketRebuildDiagnostics::new(1, 0, 1, 1, 0),
            4,
            90,
            440,
            210,
            130,
            15,
            95,
            4,
            32768,
            115,
            85,
            22,
            16384,
            920,
            640,
            180,
            70,
            560,
        ),
        TextCacheDiagnostics::default(),
        TextCacheDeltaDiagnostics::default(),
        SceneStatistics {
            detail_mode: Default::default(),
            viewport: Size::new(1280.0, 720.0),
            total_widget_count: 4,
            active_animated_widget_count: 0,
            animation_frame_wake_count: 0,
            animation_repaint_frame_count: 0,
            animation_transform_effect_only_frame_count: 0,
            dirty_region_count: 0,
            dirty_regions: Vec::new(),
            dirty_area: 0.0,
            dirty_coverage: 0.0,
            command_count: 0,
            command_breakdown: Vec::new(),
            repaint_boundary_count: 0,
            scene_layer_count: 0,
            stack_surface_count: 0,
            overlay_layer_count: 0,
            layer_update_count: 0,
            layer_update_breakdown: Vec::new(),
            text_command_count: 0,
            image_command_count: 0,
            clip_command_count: 0,
            transform_command_count: 0,
        },
    )
    .with_presentation_latency(PresentationLatencyDiagnostics::new(1.1, 4.8, 3.2))
}

fn sample_detailed_window_performance_snapshot_record(
    window_id: WindowId,
) -> WindowPerformanceSnapshot {
    WindowPerformanceSnapshot::new(
        window_id,
        8,
        vec![
            FramePhaseSample::new(FramePhase::Paint, 0.8),
            FramePhaseSample::new(FramePhase::Renderer, 1.9),
        ],
        RendererSubmissionDiagnostics::new(
            2,
            6,
            2048,
            24,
            1536,
            3,
            6,
            420,
            160,
            210,
            120,
            3,
            sui_runtime::RetainedPacketRebuildDiagnostics::new(1, 0, 1, 1, 0),
            4,
            90,
            440,
            210,
            130,
            15,
            95,
            4,
            32768,
            115,
            85,
            22,
            16384,
            920,
            640,
            180,
            70,
            560,
        ),
        TextCacheDiagnostics::default(),
        TextCacheDeltaDiagnostics::default(),
        SceneStatistics {
            detail_mode: SceneStatisticsDetailMode::Detailed,
            viewport: Size::new(1280.0, 720.0),
            total_widget_count: 9,
            active_animated_widget_count: 3,
            animation_frame_wake_count: 2,
            animation_repaint_frame_count: 1,
            animation_transform_effect_only_frame_count: 1,
            dirty_region_count: 2,
            dirty_regions: Vec::new(),
            dirty_area: 128.0,
            dirty_coverage: 3.0,
            command_count: 14,
            command_breakdown: vec![("FillRect".to_string(), 8), ("Layer".to_string(), 6)],
            repaint_boundary_count: 6,
            scene_layer_count: 6,
            stack_surface_count: 2,
            overlay_layer_count: 1,
            layer_update_count: 4,
            layer_update_breakdown: vec![("Repaint".to_string(), 4)],
            text_command_count: 3,
            image_command_count: 1,
            clip_command_count: 2,
            transform_command_count: 1,
        },
    )
    .with_presentation_latency(PresentationLatencyDiagnostics::new(2.4, 7.1, 4.5))
}
