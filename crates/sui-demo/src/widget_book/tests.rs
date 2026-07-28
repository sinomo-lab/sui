use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use super::visual_artifacts::{
    StoryCase, artifact_root, configured_widget_book_state, scroll_to_story_target,
};
use super::{
    ANIMATION_BENCHMARK_REPAINT_NAME, ANIMATION_BENCHMARK_RETAINED_NAME,
    ANIMATION_BENCHMARK_SCALE_NAME, ANIMATION_BENCHMARK_TITLE, COLOR_PICKER_NAME,
    DARK_THEME_PREVIEW_CARD_NAME, DATETIME_INPUT_LABEL, DIALOG_TITLE, DIALOG_TRIGGER_LABEL,
    GALLERY_SCROLL_BAR_NAME, GALLERY_SCROLL_NAME, LIGHT_PREVIEW_ACTION_LABEL,
    LIGHT_PREVIEW_INPUT_LABEL, LIGHT_THEME_PREVIEW_CARD_NAME, LivePerformanceDisplay,
    LivePerformanceFrameSample, LivePerformancePanel, NAME_INPUT_LABEL,
    NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME, NEUTRAL_THEME_PREVIEW_CARD_NAME, NUMBER_INPUT_NAME,
    PASSWORD_INPUT_LABEL, POPOVER_NAME, POPOVER_TRIGGER_LABEL, RADIO_BUTTON_LABEL,
    RETAINED_TEXT_BENCHMARK_SCROLL_BAR_NAME, RETAINED_TEXT_BENCHMARK_SCROLL_NAME,
    RETAINED_TEXT_BENCHMARK_TITLE, SELECT_NAME, SLIDER_NAME, SUMMARY_NAME, SWITCH_LABEL,
    TEXT_AREA_LABEL, TEXT_EDITING_BENCHMARK_EDITOR_NAME, TEXT_EDITING_BENCHMARK_SPLIT_NAME,
    TEXT_EDITING_BENCHMARK_SYNTAX_SCROLL_NAME, TEXT_EDITING_BENCHMARK_TITLE,
    TEXT_RENDERING_COMPARISON_SCROLL_NAME, TEXT_RENDERING_COMPARISON_TITLE,
    TEXT_VALIDATION_EDITOR_NAME, TEXT_VALIDATION_SCROLL_NAME, TEXT_VALIDATION_VIEW_TITLE,
    THEME_DEMO_SCROLL_NAME, THEME_DEMO_TITLE, TOOLTIP_TEXT, TOOLTIP_TRIGGER_LABEL,
    TRUE_BLACK_THEME_PREVIEW_CARD_NAME, WIDGET_STATES_BUTTON_LABEL, WIDGET_STATES_CHECKBOX_LABEL,
    WIDGET_STATES_GALLERY_NAME, WIDGET_STATES_MENU_NAME, WIDGET_STATES_POPOVER_NAME,
    WIDGET_STATES_SELECT_NAME, WIDGET_STATES_SLIDER_NAME, WIDGET_STATES_SWITCH_LABEL,
    WIDGET_STATES_TABS_NAME, WIDGET_STATES_TEXT_AREA_LABEL, WIDGET_STATES_TEXT_INPUT_LABEL,
    WINDOW_TITLE, build_animation_benchmark_application, build_color_and_imagery_story,
    build_retained_text_benchmark_application, build_text_editing_benchmark_application,
    build_text_rendering_comparison_application, build_text_validation_surface,
    build_theme_demo_application, build_widget_book_application, build_widget_book_gallery,
    default_widget_book_state, frame_phase_index, register_widget_book_images,
    text_editing_benchmark_document, text_editing_benchmark_style_overlays,
    text_editing_benchmark_style_spans, text_editing_syntax_preview_content, theme_preview_card,
};
use sui::{
    App, Application, DefaultTheme, Event, FramePhase, FramePhaseSample, ImeEvent, KeyState,
    KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
    PresentationLatencyDiagnostics, Rect, RenderOutput, RendererSubmissionDiagnostics, Result,
    SceneStatistics, SceneStatisticsDetailMode, ScrollDelta, SemanticsRole, SemanticsValue, Size,
    SizedBox, TextCacheDeltaDiagnostics, TextCacheDiagnostics, TextSurfaceOverlayKind, Vector,
    Widget, WidgetPod, WidgetPodVisitor, Window, WindowBuilder, WindowEvent, WindowId,
    WindowPerformanceSnapshot, set_window_scene_statistics_detail_mode,
    window_scene_statistics_detail_mode,
};
use sui_runtime::publish_window_performance_snapshot;
use sui_scene::{Brush, SceneCommand, SceneLayerUpdateKind};
use sui_testing::prelude::*;

fn build_default_widget_book_app() -> Result<TestApp> {
    TestApp::new(|| build_widget_book_application_with_overlay(default_widget_book_state()).build())
}

fn build_default_theme_demo_app() -> Result<TestApp> {
    TestApp::new(|| build_theme_demo_application(default_widget_book_state()).build())
}

fn build_configured_widget_book_app() -> Result<TestApp> {
    TestApp::new(|| build_widget_book_application(configured_widget_book_state()).build())
}

fn combo_box_text_value(window: &TestWindow, name: &str) -> Result<String> {
    window
        .snapshot()?
        .accessibility
        .nodes
        .into_iter()
        .find(|node| node.role == SemanticsRole::ComboBox && node.name.as_deref() == Some(name))
        .and_then(|node| match node.value {
            Some(SemanticsValue::Text(value)) => Some(value),
            _ => None,
        })
        .ok_or_else(|| sui::Error::new(format!("missing {name} combo box text value")))
}

fn build_widget_book_application_with_overlay(
    state: Rc<RefCell<super::WidgetBookState>>,
) -> Application {
    super::set_widget_book_hdr_theme_mode(sui::HdrThemeMode::Disabled);

    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(
            Window::new(WINDOW_TITLE).root(
                super::LivePerformanceRoot::new(
                    WINDOW_TITLE,
                    super::WINDOW_DESCRIPTION,
                    build_widget_book_gallery(Rc::clone(&state)),
                )
                .show_performance_overlay()
                .watch_widget_book_state(state),
            ),
        )
        .into_application()
}

#[cfg(feature = "artifacts")]
fn build_gallery_only_widget_book_app() -> Result<TestApp> {
    TestApp::from_runtime(
        App::new()
            .with_resources(|resources| {
                register_widget_book_images(resources);
                Ok(())
            })?
            .window(
                Window::new(WINDOW_TITLE)
                    .root(build_widget_book_gallery(default_widget_book_state())),
            )
            .build()?,
    )
}

#[cfg(feature = "artifacts")]
fn headless_benchmark_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn build_text_validation_app() -> Result<TestApp> {
    TestApp::new(|| {
        Application::new()
            .window(
                WindowBuilder::new()
                    .title(TEXT_VALIDATION_VIEW_TITLE)
                    .root(build_text_validation_surface()),
            )
            .build()
    })
}

fn semantics_contains(semantics: &[sui::SemanticsNode], role: &SemanticsRole, name: &str) -> bool {
    semantics
        .iter()
        .any(|node| &node.role == role && node.name.as_deref() == Some(name))
}

#[test]
fn widget_book_new_sections_cover_exported_widget_families() -> Result<()> {
    let theme_reader = super::default_widget_book_theme_reader();
    let root = SizedBox::new().width(1280.0).height(3600.0).with_child(
        sui::Stack::vertical()
            .spacing(18.0)
            .alignment(sui::Alignment::Stretch)
            .with_child(super::build_composite_widgets_gallery_with_theme(
                Rc::clone(&theme_reader),
            ))
            .with_child(super::build_layout_widgets_gallery_with_theme(Rc::clone(
                &theme_reader,
            )))
            .with_child(super::build_text_widgets_gallery_with_theme(Rc::clone(
                &theme_reader,
            )))
            .with_child(super::build_data_and_interaction_gallery_with_theme(
                Rc::clone(&theme_reader),
            ))
            .with_child(super::build_canvas_and_media_gallery_with_theme(Rc::clone(
                &theme_reader,
            ))),
    );
    let mut runtime = App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })?
        .window(Window::new("Widget family coverage").root(root))
        .build()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;

    let expected = [
        (
            SemanticsRole::GenericContainer,
            super::COMPOSITE_WIDGETS_GALLERY_NAME,
        ),
        (
            SemanticsRole::GenericContainer,
            super::LAYOUT_WIDGETS_GALLERY_NAME,
        ),
        (
            SemanticsRole::GenericContainer,
            super::TEXT_WIDGETS_GALLERY_NAME,
        ),
        (
            SemanticsRole::GenericContainer,
            super::DATA_WIDGETS_GALLERY_NAME,
        ),
        (
            SemanticsRole::GenericContainer,
            super::CANVAS_WIDGETS_GALLERY_NAME,
        ),
        (SemanticsRole::GenericContainer, super::SURFACE_SAMPLE_NAME),
        (SemanticsRole::Button, super::ACTION_CARD_NAME),
        (SemanticsRole::Text, super::SECTION_LABEL_NAME),
        (SemanticsRole::Text, super::STATUS_BADGE_NAME),
        (SemanticsRole::Text, super::COVERAGE_DOTS_NAME),
        (SemanticsRole::Text, super::PLACEMENT_BADGE_NAME),
        (SemanticsRole::GenericContainer, super::TOOLBAR_NAME),
        (SemanticsRole::GenericContainer, super::COMMAND_GROUP_NAME),
        (SemanticsRole::GenericContainer, super::TOOL_PALETTE_NAME),
        (SemanticsRole::GenericContainer, super::PRESET_STRIP_NAME),
        (SemanticsRole::RadioGroup, super::SEGMENTED_CONTROL_NAME),
        (SemanticsRole::BusyIndicator, super::BUSY_INDICATOR_NAME),
        (SemanticsRole::GenericContainer, super::FORM_SECTION_NAME),
        (SemanticsRole::GenericContainer, super::PROPERTY_ROW_NAME),
        (SemanticsRole::GenericContainer, super::DETAIL_ROW_NAME),
        (SemanticsRole::GenericContainer, super::PANEL_SECTION_NAME),
        (SemanticsRole::GenericContainer, super::DOCK_PANEL_NAME),
        (SemanticsRole::GenericContainer, super::EMPTY_STATE_NAME),
        (SemanticsRole::GenericContainer, super::STATUS_BAR_NAME),
        (SemanticsRole::GenericContainer, super::LAYOUT_REGION_NAME),
        (SemanticsRole::GenericContainer, super::DOCK_LAYOUT_NAME),
        (
            SemanticsRole::GenericContainer,
            super::MEASURED_BOTTOM_DOCK_NAME,
        ),
        (SemanticsRole::GenericContainer, super::SWITCH_VIEW_NAME),
        (
            SemanticsRole::GenericContainer,
            super::TRAILING_SLOT_ROW_NAME,
        ),
        (
            SemanticsRole::GenericContainer,
            super::FIXED_PANE_SPLIT_NAME,
        ),
        (SemanticsRole::ScrollView, super::SCROLL_VIEW_NAME),
        (SemanticsRole::ScrollView, super::VIRTUAL_SCROLL_SAMPLE_NAME),
        (SemanticsRole::Text, super::RICH_TEXT_NAME),
        (SemanticsRole::Link, super::LINK_NAME),
        (SemanticsRole::ComboBox, super::COMBO_BOX_ALIAS_NAME),
        (SemanticsRole::SpinBox, super::SPIN_BOX_ALIAS_NAME),
        (SemanticsRole::TextInput, super::MULTILINE_ALIAS_NAME),
        (SemanticsRole::Separator, super::DIVIDER_ALIAS_NAME),
        (SemanticsRole::Breadcrumb, super::PATH_BAR_NAME),
        (SemanticsRole::List, super::LAYER_LIST_NAME),
        (SemanticsRole::Table, super::DATA_GRID_NAME),
        (SemanticsRole::Table, super::VIRTUAL_TABLE_NAME),
        (SemanticsRole::List, super::REORDERABLE_LIST_NAME),
        (SemanticsRole::Text, super::DRAG_SOURCE_NAME),
        (SemanticsRole::Text, super::DROP_TARGET_NAME),
        (SemanticsRole::GenericContainer, super::CANVAS_RULER_NAME),
        (SemanticsRole::Canvas, super::CANVAS_NAME),
        (SemanticsRole::Canvas, super::PIXEL_CANVAS_NAME),
        (SemanticsRole::GenericContainer, super::COLOR_PALETTE_NAME),
        (SemanticsRole::Image, super::BRUSH_PREVIEW_NAME),
        (SemanticsRole::GenericContainer, super::SIGNAL_METER_NAME),
    ];

    for (role, name) in expected {
        assert!(
            semantics_contains(&output.semantics, &role, name),
            "missing {role:?} named {name:?}"
        );
    }

    Ok(())
}

#[test]
fn widget_book_virtual_table_sample_paints_cells() {
    let output = render_widget_with_size(
        "Widget book data gallery",
        Size::new(1280.0, 1800.0),
        super::build_data_and_interaction_gallery_with_theme(
            super::default_widget_book_theme_reader(),
        ),
    );

    for text in ["#0000", "asset_00000.png"] {
        assert!(
            scene_contains_text(&output, text),
            "virtual table sample should paint visible cell text {text:?}"
        );
    }
}

#[test]
fn widget_book_empty_state_sample_keeps_action_inside_bounds() -> Result<()> {
    let theme_reader = widget_book_theme_reader(DefaultTheme::dark());
    let root = SizedBox::new().width(1280.0).height(1200.0).with_child(
        super::build_composite_widgets_gallery_with_theme(theme_reader),
    );
    let mut runtime = App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })?
        .window(Window::new("Widget book empty state").root(root))
        .build()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;

    let empty_state = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(super::EMPTY_STATE_NAME)
        })
        .expect("widget-book empty state semantics should exist");
    let action = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some("Clear filters")
        })
        .expect("widget-book empty state action semantics should exist");

    assert!(empty_state.bounds.height() >= 180.0);
    assert!(action.bounds.y() >= empty_state.bounds.y());
    assert!(action.bounds.max_y() <= empty_state.bounds.max_y());
    assert!(action.bounds.x() >= empty_state.bounds.x());
    assert!(action.bounds.max_x() <= empty_state.bounds.max_x());
    assert!(
        action.bounds.width() < empty_state.bounds.width() * 0.75,
        "empty-state action should not stretch across the sample: state={:?}, action={:?}",
        empty_state.bounds,
        action.bounds
    );

    Ok(())
}

#[test]
fn widget_book_exported_sections_keep_second_column_stories_sized() -> Result<()> {
    let theme_reader = super::default_widget_book_theme_reader();
    let root = SizedBox::new().width(1280.0).height(3600.0).with_child(
        sui::Stack::vertical()
            .spacing(18.0)
            .alignment(sui::Alignment::Stretch)
            .with_child(super::build_layout_widgets_gallery_with_theme(Rc::clone(
                &theme_reader,
            )))
            .with_child(super::build_data_and_interaction_gallery_with_theme(
                Rc::clone(&theme_reader),
            ))
            .with_child(super::build_canvas_and_media_gallery_with_theme(Rc::clone(
                &theme_reader,
            ))),
    );
    let mut runtime = App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })?
        .window(Window::new("Widget exported sections").root(root))
        .build()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;
    let semantics = output.semantics;

    for (role, name, min_width) in [
        (
            SemanticsRole::GenericContainer,
            super::TRAILING_SLOT_ROW_NAME,
            300.0,
        ),
        (
            SemanticsRole::ScrollView,
            super::VIRTUAL_SCROLL_SAMPLE_NAME,
            300.0,
        ),
        (SemanticsRole::Table, super::DATA_GRID_NAME, 400.0),
        (SemanticsRole::Text, super::DROP_TARGET_NAME, 80.0),
        (
            SemanticsRole::GenericContainer,
            super::COLOR_PALETTE_NAME,
            100.0,
        ),
        (SemanticsRole::ColorSwatch, "Canvas accent swatch", 40.0),
        (
            SemanticsRole::GenericContainer,
            super::SIGNAL_METER_NAME,
            80.0,
        ),
    ] {
        let node = semantics
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("missing {role:?} named {name:?}"));
        assert!(
            node.bounds.width() >= min_width,
            "{role:?} named {name:?} should keep a usable width: {:?}",
            node.bounds
        );
    }

    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    scroll_to_story_target(&window, StoryCase::ColorSwatch, 64)?;
    let snapshot = window.snapshot()?;
    let scrolled_semantics = &snapshot.accessibility.nodes;

    for (section_name, max_height) in [
        // The data gallery includes the 132px VirtualList story added alongside
        // the collection framework. Keep headroom for its surrounding spacing
        // while still catching an accidentally stretched, viewport-sized tail.
        (super::DATA_WIDGETS_GALLERY_NAME, 1_100.0),
        (super::CANVAS_WIDGETS_GALLERY_NAME, 750.0),
    ] {
        let section = scrolled_semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(section_name)
            })
            .unwrap_or_else(|| panic!("missing exported section {section_name:?}"));
        assert!(
            section.bounds.height() <= max_height,
            "{section_name} should not reserve a large blank tail: {:?}",
            section.bounds
        );
    }

    Ok(())
}

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
    assert!(
        overlays
            .iter()
            .any(|overlay| matches!(overlay.kind, TextSurfaceOverlayKind::RichTextPreview))
    );
    assert!(
        spans
            .iter()
            .all(|span| span.range.start < span.range.end && span.range.end <= document.len())
    );
    assert!(
        overlays
            .iter()
            .all(|overlay| overlay.range.start < overlay.range.end
                && overlay.range.end <= document.len())
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
        super::build_retained_text_benchmark_with_theme(widget_book_theme_reader(theme)),
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

fn build_text_validation_runtime() -> Result<sui::Runtime> {
    Application::new()
        .window(
            WindowBuilder::new().title(TEXT_VALIDATION_VIEW_TITLE).root(
                SizedBox::new()
                    .size(Size::new(460.0, 380.0))
                    .with_child(build_text_validation_surface()),
            ),
        )
        .build()
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

fn build_text_rendering_comparison_runtime() -> Result<sui::Runtime> {
    build_text_rendering_comparison_application().build()
}

fn build_narrow_text_rendering_comparison_runtime() -> Result<sui::Runtime> {
    Application::new()
        .window(
            WindowBuilder::new()
                .title(TEXT_RENDERING_COMPARISON_TITLE)
                .root(
                    SizedBox::new()
                        .size(Size::new(430.0, 320.0))
                        .with_child(super::build_text_rendering_comparison_surface()),
                ),
        )
        .build()
}

fn build_color_validation_runtime() -> Result<sui::Runtime> {
    super::build_color_validation_application().build()
}

fn build_narrow_color_validation_runtime() -> Result<sui::Runtime> {
    Application::new()
        .window(
            WindowBuilder::new()
                .title(super::COLOR_VALIDATION_VIEW_TITLE)
                .root(
                    SizedBox::new()
                        .size(Size::new(430.0, 320.0))
                        .with_child(super::build_color_validation_surface()),
                ),
        )
        .build()
}

fn widget_book_theme_reader(theme: DefaultTheme) -> super::WidgetBookThemeReader {
    Rc::new(move || theme)
}

fn mutable_widget_book_theme_reader(
    theme: Rc<RefCell<DefaultTheme>>,
) -> super::WidgetBookThemeReader {
    Rc::new(move || *theme.borrow())
}

fn assert_widget_repaints_after_theme_change<W, B>(title: &str, size: Size, build: B) -> Result<()>
where
    W: Widget + 'static,
    B: FnOnce(super::WidgetBookThemeReader) -> W,
{
    let theme = Rc::new(RefCell::new(DefaultTheme::default()));
    let child = build(mutable_widget_book_theme_reader(Rc::clone(&theme)));
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

fn render_widget_with_size<W>(title: &str, size: Size, child: W) -> RenderOutput
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

fn scene_contains_text(output: &RenderOutput, text: &str) -> bool {
    let registry = output.frame.text_layout_registry.as_ref();
    let mut found = false;
    output.frame.scene.visit_commands(&mut |command| {
        if found {
            return;
        }
        found = match command {
            SceneCommand::DrawText(run) => run.text == text,
            SceneCommand::DrawShapedText(run) => run
                .resolve(registry)
                .is_some_and(|layout| layout.text() == text),
            SceneCommand::DrawShapedTextWindow(run) => run
                .resolve(registry)
                .is_some_and(|layout| layout.text() == text),
            _ => false,
        };
    });
    found
}

#[test]
fn focused_demo_surfaces_repaint_when_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        super::WINDOW_TITLE,
        Size::new(1280.0, 760.0),
        |theme_reader| {
            super::build_widget_book_gallery_with_theme(default_widget_book_state(), theme_reader)
        },
    )?;
    assert_widget_repaints_after_theme_change(
        super::THEME_DEMO_TITLE,
        Size::new(760.0, 520.0),
        |theme_reader| {
            super::build_theme_demo_surface_with_theme(default_widget_book_state(), theme_reader)
        },
    )?;
    assert_widget_repaints_after_theme_change(
        super::TEXT_VALIDATION_VIEW_TITLE,
        Size::new(520.0, 420.0),
        super::build_text_validation_surface_with_theme,
    )?;
    assert_widget_repaints_after_theme_change(
        super::TEXT_EDITING_BENCHMARK_TITLE,
        Size::new(900.0, 520.0),
        super::build_text_editing_benchmark_with_theme,
    )?;
    assert_widget_repaints_after_theme_change(
        super::TEXT_RENDERING_COMPARISON_TITLE,
        Size::new(520.0, 360.0),
        super::build_text_rendering_comparison_surface_with_theme,
    )?;
    assert_widget_repaints_after_theme_change(
        super::COLOR_VALIDATION_VIEW_TITLE,
        Size::new(520.0, 360.0),
        super::build_color_validation_surface_with_theme,
    )
}

fn assert_semantics_omit_live_performance_overlay(semantics: &[sui::SemanticsNode]) {
    assert!(
        semantics
            .iter()
            .all(|node| node.name.as_deref() != Some("Live performance overlay")),
        "expected semantics tree to omit the floating live performance overlay outside sui-demo"
    );
}

#[cfg(feature = "artifacts")]
fn unique_visual_artifact_test_dir(name: &str) -> PathBuf {
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

fn solid_fill_max_channel(output: &RenderOutput) -> f32 {
    let mut max_channel = 0.0_f32;
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
            } => {
                max_channel = max_channel.max(color.red.max(color.green.max(color.blue)));
            }
            _ => {}
        });
    max_channel
}

fn solid_fill_colors(output: &RenderOutput) -> Vec<sui::Color> {
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

fn solid_fill_bounds(output: &RenderOutput, expected: sui::Color) -> Vec<sui::Rect> {
    let mut bounds = Vec::new();
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::FillRect {
                rect,
                brush: Brush::Solid(color),
            }
            | SceneCommand::FillRoundedRect {
                rect,
                brush: Brush::Solid(color),
                ..
            } if *color == expected => bounds.push(*rect),
            SceneCommand::FillPath {
                path,
                brush: Brush::Solid(color),
            } if *color == expected => bounds.push(path.bounds()),
            _ => {}
        });
    bounds
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

#[cfg(feature = "artifacts")]
fn build_light_theme_preview_reference_app(card_width: f32) -> Result<TestApp> {
    TestApp::from_runtime(
        Application::new()
            .window(
                WindowBuilder::new().title("Theme preview reference").root(
                    sui::containers::Padding::all(
                        24.0,
                        SizedBox::new()
                            .width(card_width)
                            .height(super::ThemePreviewGrid::CARD_HEIGHT)
                            .with_child(super::NamedSection::new(
                                LIGHT_THEME_PREVIEW_CARD_NAME,
                                theme_preview_card(
                                    DefaultTheme::sui(),
                                    "SUI light",
                                    LIGHT_PREVIEW_ACTION_LABEL,
                                    LIGHT_PREVIEW_INPUT_LABEL,
                                ),
                            )),
                    ),
                ),
            )
            .build()?,
    )
}

#[cfg(feature = "artifacts")]
fn build_headless_default_widget_book_app() -> Result<TestApp> {
    TestApp::from_runtime(build_widget_book_application(default_widget_book_state()).build()?)
}

#[cfg(feature = "artifacts")]
fn build_headless_default_theme_demo_app() -> Result<TestApp> {
    TestApp::from_runtime(build_theme_demo_application(default_widget_book_state()).build()?)
}

#[cfg(feature = "artifacts")]
fn viewport_size(window: &TestWindow) -> Result<Size> {
    let snapshot = window.snapshot()?;
    if let Some(scene) = snapshot.scene_summary {
        return Ok(scene.viewport);
    }

    snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.role == SemanticsRole::Window)
        .map(|node| node.bounds.size)
        .ok_or_else(|| sui::Error::new("window viewport is missing from snapshot"))
}

#[cfg(feature = "artifacts")]
fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((sorted.len() - 1) as f64 * quantile).round() as usize;
    sorted[rank]
}

#[cfg(feature = "artifacts")]
fn print_widget_book_headless_scroll_benchmark_summary(
    label: &str,
    samples: &[WindowPerformanceSnapshot],
) {
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
fn set_detailed_scene_statistics_mode(window: &TestWindow) -> Result<()> {
    set_window_scene_statistics_detail_mode(window.id(), SceneStatisticsDetailMode::Detailed);
    window.run_until_idle()
}

#[cfg(feature = "artifacts")]
fn collect_headless_scroll_benchmark_samples(
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
fn next_headless_benchmark_frame(
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
fn collect_headless_text_editing_benchmark_samples(
    window: &TestWindow,
) -> Result<Vec<WindowPerformanceSnapshot>> {
    const EDIT_COMMITS: [&str; 10] = [
        " // typed atlas reuse",
        "\nlet pending_frame = cache_hits + 1;",
        "\n// bidi check: abc ××‘×’ 123 Ù…Ø±Ø­Ø¨Ø§",
        "\nlet emoji = \"ðŸ™‚âœ…ðŸŽ¨\";",
        "\nlet ime_probe = \"å€™è£œ\";",
        "\nlet syntax_band = highlight_rows.len();",
        "\n// fallback sample: Ð– ä¸­ à¤¨à¤®à¤¸à¥à¤¤à¥‡",
        "\nrecord_selection_delta(cursor, viewport);",
        "\nlet scroll_budget_ms = 16.67;",
        "\ncommit_overlay_sample(frame_index);",
    ];
    const IME_PREEDIT_UPDATES: [(&str, Option<(usize, usize)>); 3] = [
        ("å€™", Some((0, 1))),
        ("å€™è£œ", Some((1, 2))),
        ("å€™è£œã‚’", Some((2, 3))),
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
        text: "å€™è£œã‚’".to_string(),
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
fn set_window_scale_factor(window: &TestWindow, scale_factor: f64, raw_dpi: f32) -> Result<()> {
    let viewport = viewport_size(window)?;
    window
        .root()
        .dispatch_event(Event::Window(WindowEvent::ScaleFactorChanged {
            scale_factor,
            raw_dpi: Some(raw_dpi),
            suggested_size: Some(viewport),
        }))?;
    window
        .root()
        .dispatch_event(Event::Window(WindowEvent::Resized(viewport)))?;
    window.run_until_idle()
}

#[cfg(feature = "artifacts")]
fn write_screenshot(path: impl AsRef<Path>, screenshot: &sui_testing::Screenshot) -> Result<()> {
    screenshot.write_png(path)
}

#[cfg(feature = "artifacts")]
const SCREENSHOT_CHANNEL_TOLERANCE: u8 = 1;

#[cfg(feature = "artifacts")]
fn screenshot_pixels_match(left: &[u8], right: &[u8]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| left.abs_diff(*right) <= SCREENSHOT_CHANNEL_TOLERANCE)
}

#[cfg(feature = "artifacts")]
fn screenshot_diff_count(left: &sui_testing::Screenshot, right: &sui_testing::Screenshot) -> usize {
    assert_eq!(left.width(), right.width(), "screenshot widths differ");
    assert_eq!(left.height(), right.height(), "screenshot heights differ");

    left.pixels()
        .chunks_exact(4)
        .zip(right.pixels().chunks_exact(4))
        .filter(|(left_px, right_px)| !screenshot_pixels_match(left_px, right_px))
        .count()
}

#[cfg(feature = "artifacts")]
fn screenshot_diff_image(
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
fn normalize_screenshot_pair(
    left: &sui_testing::Screenshot,
    right: &sui_testing::Screenshot,
) -> Result<(sui_testing::Screenshot, sui_testing::Screenshot)> {
    let width = left.width().min(right.width()) as f32;
    let height = left.height().min(right.height()) as f32;
    let crop = sui::Rect::new(0.0, 0.0, width, height);
    Ok((left.crop(crop)?, right.crop(crop)?))
}

#[cfg(feature = "artifacts")]
#[test]
fn screenshot_diff_helpers_tolerate_one_channel_value_per_channel() -> Result<()> {
    let left = sui_testing::Screenshot::new(2, 1, vec![10, 20, 30, 40, 100, 110, 120, 130])?;
    let right = sui_testing::Screenshot::new(2, 1, vec![11, 19, 31, 39, 99, 111, 119, 131])?;

    assert_eq!(screenshot_diff_count(&left, &right), 0);

    let diff = screenshot_diff_image(&left, &right)?;
    assert_eq!(diff.pixels(), &[10, 20, 30, 96, 100, 110, 120, 96]);

    Ok(())
}

#[test]
fn text_rendering_comparison_surface_exposes_all_render_modes() {
    let mut runtime =
        build_text_rendering_comparison_runtime().expect("comparison runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("comparison surface should render");

    let semantics = runtime
        .semantics(window_id)
        .expect("comparison semantics should exist");

    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Window
            && node.name.as_deref() == Some(TEXT_RENDERING_COMPARISON_TITLE)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::ScrollView
            && node.name.as_deref() == Some(TEXT_RENDERING_COMPARISON_SCROLL_NAME)
    }));

    for spec in super::TEXT_RENDERING_MODE_DATA {
        let mode_name = spec.title;
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some(mode_name)
        }));

        for dark in [false, true] {
            let sample_name = super::text_rendering_sample_name(spec.title, dark);
            assert!(semantics.iter().any(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(sample_name.as_str())
            }));
        }
    }
}

#[test]
fn text_rendering_comparison_surface_uses_direct_policy_overrides() {
    let mut runtime =
        build_text_rendering_comparison_runtime().expect("comparison runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("comparison surface should render");

    let mut image_commands = 0usize;
    let mut push_policy_commands = 0usize;
    let mut pop_policy_commands = 0usize;
    let mut text_commands = 0usize;
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                image_commands += 1;
            }
            SceneCommand::PushTextRenderPolicy { .. } => {
                push_policy_commands += 1;
            }
            SceneCommand::PopTextRenderPolicy => {
                pop_policy_commands += 1;
            }
            SceneCommand::DrawText(_)
            | SceneCommand::DrawShapedText(_)
            | SceneCommand::DrawShapedTextWindow(_) => {
                text_commands += 1;
            }
            _ => {}
        });

    assert_eq!(image_commands, 0);
    assert_eq!(
        push_policy_commands,
        super::TEXT_RENDERING_MODE_DATA.len() * 2
    );
    assert_eq!(pop_policy_commands, push_policy_commands);
    assert!(text_commands > push_policy_commands);
}

#[test]
fn text_rendering_comparison_surface_uses_two_axis_scroll_when_narrow() {
    let mut runtime = build_narrow_text_rendering_comparison_runtime()
        .expect("narrow comparison runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("narrow comparison surface should render");

    let scroll = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(TEXT_RENDERING_COMPARISON_SCROLL_NAME)
        })
        .expect("text comparison scroll view should be present");
    let horizontal_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref()
                    == Some(super::TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME)
        })
        .expect("horizontal text comparison scroll bar should be present");
    let vertical_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref()
                    == Some(super::TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME)
        })
        .expect("vertical text comparison scroll bar should be present");

    let horizontal_max = match horizontal_scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };
    let vertical_max = match vertical_scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };

    assert!(horizontal_max > 0.0);
    assert!(vertical_max > 0.0);
    assert!(horizontal_scroll_bar.bounds.y() >= scroll.bounds.max_y());
    assert!(vertical_scroll_bar.bounds.x() >= scroll.bounds.max_x());
}

#[test]
fn text_rendering_comparison_scroll_bars_use_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        TEXT_RENDERING_COMPARISON_TITLE,
        Size::new(430.0, 320.0),
        super::build_text_rendering_comparison_surface_with_theme(widget_book_theme_reader(theme)),
    );
    let horizontal_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref()
                    == Some(super::TEXT_RENDERING_COMPARISON_HORIZONTAL_SCROLL_BAR_NAME)
        })
        .expect("horizontal text comparison scroll bar should be present");
    let vertical_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref()
                    == Some(super::TEXT_RENDERING_COMPARISON_VERTICAL_SCROLL_BAR_NAME)
        })
        .expect("vertical text comparison scroll bar should be present");

    assert_eq!(
        vertical_scroll_bar.bounds.width(),
        theme.metrics.scroll_bar_thickness
    );
    assert_eq!(
        horizontal_scroll_bar.bounds.height(),
        theme.metrics.scroll_bar_thickness
    );
}

#[test]
fn text_validation_scroll_repaints_visible_content() -> Result<()> {
    let mut runtime = build_text_validation_runtime()?;
    let window_id = runtime.window_ids()[0];
    let before = runtime.render(window_id)?;
    let scroll_node = before
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(TEXT_VALIDATION_SCROLL_NAME)
        })
        .expect("text validation scroll semantics present");
    let scroll_point = Point::new(
        scroll_node.bounds.x() + 24.0,
        scroll_node.bounds.y() + (scroll_node.bounds.height() * 0.5),
    );

    let mut scroll = PointerEvent::new(PointerEventKind::Scroll, scroll_point);
    scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, -220.0)));
    runtime.handle_event(window_id, Event::Pointer(scroll))?;
    let after = runtime.render(window_id)?;

    assert_ne!(before.frame.scene, after.frame.scene);
    assert!(after.frame.layer_updates.iter().any(|update| {
        update.owner == scroll_node.id && update.kind == SceneLayerUpdateKind::Content
    }));

    Ok(())
}

#[test]
fn color_validation_surface_exposes_wide_gamut_reference_swatches() {
    let mut runtime =
        build_color_validation_runtime().expect("color validation runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("color validation surface should render");

    let semantics = runtime
        .semantics(window_id)
        .expect("color validation semantics should exist");

    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Window
            && node.name.as_deref() == Some(super::COLOR_VALIDATION_VIEW_TITLE)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::ScrollView
            && node.name.as_deref() == Some(super::COLOR_VALIDATION_SCROLL_NAME)
    }));

    for swatch_name in [
        "sRGB reference red",
        "Display P3 reference red",
        "sRGB clipped lime",
        "Display P3 vivid lime",
        "sRGB accent cyan",
        "Display P3 accent cyan",
        "Reference white 1.0",
        "Highlight white 2.0",
        "Highlight white 4.0",
        "Highlight white 8.0",
        "Orange highlight 1.0",
        "Orange highlight 2.0",
        "Cyan highlight 1.0",
        "Cyan highlight 2.0",
        "SDR white baseline",
        "SDR clipped white 2.0",
    ] {
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some(swatch_name)
        }));
    }
}

#[test]
fn color_validation_surface_keeps_swatch_labels_readable_when_narrow() {
    let mut runtime = build_narrow_color_validation_runtime()
        .expect("narrow color validation runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("narrow color validation surface should render");

    let scroll = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_SCROLL_NAME)
        })
        .expect("color validation scroll view should be present");
    let horizontal_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
        })
        .expect("horizontal color validation scroll bar should be present");
    let vertical_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME)
        })
        .expect("vertical color validation scroll bar should be present");
    let cyan_label = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text && node.name.as_deref() == Some("Cyan highlight 2.0")
        })
        .expect("final HDR color label should be present");
    let hdr_description = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text
                && node
                    .name
                    .as_deref()
                    .is_some_and(|name| name.starts_with("Colored highlights help catch cases"))
        })
        .expect("HDR color description should be present");

    let horizontal_max = match horizontal_scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };
    let vertical_max = match vertical_scroll_bar.value {
        Some(SemanticsValue::Range { max, .. }) => max,
        _ => 0.0,
    };

    assert!(horizontal_max > 0.0);
    assert!(vertical_max > 0.0);
    assert!(horizontal_scroll_bar.bounds.y() >= scroll.bounds.max_y());
    assert!(vertical_scroll_bar.bounds.x() >= scroll.bounds.max_x());
    assert!(cyan_label.bounds.width() >= 80.0);
    assert!(cyan_label.bounds.height() <= 40.0);
    assert!(hdr_description.bounds.height() > 20.0);
    assert!(hdr_description.bounds.width() < 900.0);
}

#[test]
fn color_validation_scroll_bars_use_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        super::COLOR_VALIDATION_VIEW_TITLE,
        Size::new(430.0, 320.0),
        super::build_color_validation_surface_with_theme(widget_book_theme_reader(theme)),
    );
    let horizontal_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_HORIZONTAL_SCROLL_BAR_NAME)
        })
        .expect("horizontal color validation scroll bar should be present");
    let vertical_scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(super::COLOR_VALIDATION_VERTICAL_SCROLL_BAR_NAME)
        })
        .expect("vertical color validation scroll bar should be present");

    assert_eq!(
        vertical_scroll_bar.bounds.width(),
        theme.metrics.scroll_bar_thickness
    );
    assert_eq!(
        horizontal_scroll_bar.bounds.height(),
        theme.metrics.scroll_bar_thickness
    );
}

#[test]
fn color_validation_surface_omits_live_performance_overlay() {
    let mut runtime =
        build_color_validation_runtime().expect("color validation runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("color validation surface should render");

    let semantics = runtime
        .semantics(window_id)
        .expect("color validation semantics should exist");
    assert_semantics_omit_live_performance_overlay(semantics);
}

#[test]
fn widget_book_application_omits_live_performance_overlay() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
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
fn widget_book_shell_exposes_sidebar_navigation_without_duplicate_view_controls() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("widget book should render with shell chrome");

    for (role, name) in [
        (
            SemanticsRole::GenericContainer,
            super::WIDGET_BOOK_SHELL_NAME,
        ),
        (SemanticsRole::TextInput, super::WIDGET_BOOK_SEARCH_NAME),
        (
            SemanticsRole::ComboBox,
            super::WIDGET_BOOK_THEME_SELECT_NAME,
        ),
        (SemanticsRole::List, super::WIDGET_BOOK_CATEGORY_NAV_NAME),
    ] {
        assert!(
            output
                .semantics
                .iter()
                .any(|node| { node.role == role && node.name.as_deref() == Some(name) })
        );
    }
    assert!(output.semantics.iter().any(|node| {
        node.role == SemanticsRole::ComboBox
            && node.name.as_deref() == Some(super::WIDGET_BOOK_THEME_SELECT_NAME)
            && node.value == Some(SemanticsValue::Text("SUI light".to_string()))
    }));
    for removed_control in ["Component category", "Preview theme", "Control size"] {
        assert!(output.semantics.iter().all(|node| {
            node.role != SemanticsRole::ComboBox || node.name.as_deref() != Some(removed_control)
        }));
    }
    assert!(output.semantics.iter().all(|node| {
        node.name.as_deref() != Some("Browse")
            && !node
                .name
                .as_deref()
                .is_some_and(|name| name.starts_with("Choose a category or type above"))
    }));

    let category_rail = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::List
                && node.name.as_deref() == Some(super::WIDGET_BOOK_CATEGORY_NAV_NAME)
        })
        .expect("category rail should be present at desktop width");
    let gallery = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
        })
        .expect("gallery should remain available beside shell chrome");

    assert!(category_rail.bounds.max_x() <= gallery.bounds.x());
}

#[test]
fn widget_book_sidebar_theme_selector_switches_the_live_theme() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let selector = window
        .get_by_role(SemanticsRole::ComboBox)
        .with_name(super::WIDGET_BOOK_THEME_SELECT_NAME);

    assert_eq!(
        combo_box_text_value(&window, super::WIDGET_BOOK_THEME_SELECT_NAME)?,
        "SUI light"
    );
    let before = window.capture_screenshot()?;

    for _ in 0..3 {
        selector.press("ArrowDown")?;
    }
    selector.press("Enter")?;

    assert_eq!(
        combo_box_text_value(&window, super::WIDGET_BOOK_THEME_SELECT_NAME)?,
        "Neutral dark"
    );
    let after = window.capture_screenshot()?;
    assert_ne!(
        before, after,
        "changing the sidebar theme should repaint the full widget book"
    );

    Ok(())
}

#[test]
fn widget_book_demo_text_roles_use_semantic_theme_weights() {
    let theme = DefaultTheme::default();
    assert_eq!(
        super::DemoTextRole::PageTitle.weight(theme).value(),
        theme.font_weights.semibold
    );
    assert_eq!(
        super::DemoTextRole::SectionTitle.weight(theme).value(),
        theme.font_weights.semibold
    );
    assert_eq!(
        super::DemoTextRole::CardTitle.weight(theme).value(),
        theme.font_weights.medium
    );
    assert_eq!(
        super::DemoTextRole::Body.weight(theme).value(),
        theme.font_weights.normal
    );
}

#[test]
fn widget_book_category_rail_navigates_without_filtering_stories() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;

    let gallery_locator = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);
    for (category_name, target_role, target_name) in [
        ("All components", SemanticsRole::Text, WINDOW_TITLE),
        (
            "Foundations",
            SemanticsRole::GenericContainer,
            WIDGET_STATES_GALLERY_NAME,
        ),
        ("Controls", SemanticsRole::Text, "Common controls"),
        ("Text", SemanticsRole::Text, "Typography"),
        ("Navigation", SemanticsRole::Text, "Navigation surfaces"),
        (
            "Data views",
            SemanticsRole::Text,
            "Collections and hierarchy",
        ),
        ("Layout", SemanticsRole::Text, "Layout and pathing"),
        (
            "Canvas & media",
            SemanticsRole::GenericContainer,
            super::CANVAS_WIDGETS_GALLERY_NAME,
        ),
    ] {
        window
            .get_by_role(SemanticsRole::ListItem)
            .with_name(category_name)
            .click()?;

        let snapshot = window.snapshot()?;
        let gallery = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView
                    && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
            })
            .expect("gallery should remain present after navigation");
        let target = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == target_role && node.name.as_deref() == Some(target_name))
            .unwrap_or_else(|| panic!("{category_name} should expose {target_name}"));
        assert!(
            target.bounds.y() - gallery.bounds.y() < 96.0,
            "{target_name} should align near the gallery top: gallery={:?}, target={:?}",
            gallery.bounds,
            target.bounds
        );
    }

    gallery_locator.scroll_pixels(Vector::new(0.0, 100_000.0))?;
    let reset = window.snapshot()?;
    assert!(reset.accessibility.nodes.iter().any(|node| {
        node.role == SemanticsRole::GenericContainer
            && node.name.as_deref() == Some(WIDGET_STATES_GALLERY_NAME)
    }));

    Ok(())
}

#[test]
fn hdr_theme_lab_exposes_mode_comparison_sections() {
    let mut runtime = build_theme_demo_application(default_widget_book_state())
        .build()
        .expect("theme demo runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("theme demo should render for HDR lab semantics");
    let semantics = runtime
        .semantics(window_id)
        .expect("theme demo semantics should exist");

    for section_name in [
        super::HDR_THEME_LAB_NAME,
        super::HDR_THEME_LAB_ACTIVE_PREVIEW_NAME,
        super::hdr_theme_lab_section_name(super::HdrThemeMode::Disabled),
        super::hdr_theme_lab_section_name(super::HdrThemeMode::WideGamutOnly),
        super::hdr_theme_lab_section_name(super::HdrThemeMode::ConstrainedHdr),
        super::hdr_theme_lab_section_name(super::HdrThemeMode::FullHdr),
    ] {
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(section_name)
        }));
    }

    for (button_name, switch_name) in [
        (
            format!(
                "{} sample action",
                super::hdr_theme_mode_title(super::HdrThemeMode::Disabled)
            ),
            format!(
                "{} sample live indicator",
                super::hdr_theme_mode_title(super::HdrThemeMode::Disabled)
            ),
        ),
        (
            format!(
                "{} sample action",
                super::hdr_theme_mode_title(super::HdrThemeMode::WideGamutOnly)
            ),
            format!(
                "{} sample live indicator",
                super::hdr_theme_mode_title(super::HdrThemeMode::WideGamutOnly)
            ),
        ),
        (
            format!(
                "{} sample action",
                super::hdr_theme_mode_title(super::HdrThemeMode::ConstrainedHdr)
            ),
            format!(
                "{} sample live indicator",
                super::hdr_theme_mode_title(super::HdrThemeMode::ConstrainedHdr)
            ),
        ),
        (
            format!(
                "{} sample action",
                super::hdr_theme_mode_title(super::HdrThemeMode::FullHdr)
            ),
            format!(
                "{} sample live indicator",
                super::hdr_theme_mode_title(super::HdrThemeMode::FullHdr)
            ),
        ),
    ] {
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Button && node.name.as_deref() == Some(button_name.as_str())
        }));
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::Switch && node.name.as_deref() == Some(switch_name.as_str())
        }));
    }
}

#[test]
fn widget_book_gallery_omits_theme_demo_sections() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("widget book should render");
    let semantics = runtime
        .semantics(window_id)
        .expect("widget book semantics should exist");

    for removed_section in [
        super::THEME_PREVIEW_NAME,
        super::HDR_THEME_LAB_NAME,
        crate::animation_demo::ANIMATION_DEMO_NAME,
    ] {
        assert!(
            semantics.iter().all(|node| {
                node.role != SemanticsRole::GenericContainer
                    || node.name.as_deref() != Some(removed_section)
            }),
            "expected the main widget book gallery to omit {removed_section:?}"
        );
    }
}

#[test]
fn widget_book_exposes_widget_states_gallery() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("widget book should render for widget states semantics");
    let semantics = runtime
        .semantics(window_id)
        .expect("widget book semantics should exist");

    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::GenericContainer
            && node.name.as_deref() == Some(WIDGET_STATES_GALLERY_NAME)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Button
            && node.name.as_deref() == Some(WIDGET_STATES_BUTTON_LABEL)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Button
            && node.name.as_deref() == Some(super::WIDGET_STATES_ICON_BUTTON_LABEL)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::TextInput
            && node.name.as_deref() == Some(WIDGET_STATES_TEXT_INPUT_LABEL)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::TextInput
            && node.name.as_deref() == Some(WIDGET_STATES_TEXT_AREA_LABEL)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::ComboBox
            && node.name.as_deref() == Some(WIDGET_STATES_SELECT_NAME)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::CheckBox
            && node.name.as_deref() == Some(WIDGET_STATES_CHECKBOX_LABEL)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Switch
            && node.name.as_deref() == Some(WIDGET_STATES_SWITCH_LABEL)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Slider
            && node.name.as_deref() == Some(WIDGET_STATES_SLIDER_NAME)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Tabs && node.name.as_deref() == Some(WIDGET_STATES_TABS_NAME)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Menu && node.name.as_deref() == Some(WIDGET_STATES_MENU_NAME)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Popover
            && node.name.as_deref() == Some(WIDGET_STATES_POPOVER_NAME)
    }));
}

#[test]
fn widget_book_popup_samples_start_collapsed_to_keep_gallery_compact() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("widget book should render for popup sample state");
    let semantics = runtime
        .semantics(window_id)
        .expect("widget book semantics should exist");

    let select = semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ComboBox
                && node.name.as_deref() == Some("States select expandable")
        })
        .expect("state matrix expandable select should exist");
    assert_eq!(select.state.expanded, Some(false));

    let popover = semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Popover
                && node.name.as_deref() == Some("States popover details")
        })
        .expect("state matrix popover sample should exist");
    assert_eq!(popover.state.expanded, Some(false));
}

#[test]
fn widget_book_state_matrix_rows_share_a_single_surface() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("widget book should render for widget state row surfaces");

    let button = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(WIDGET_STATES_BUTTON_LABEL)
        })
        .expect("state action button should be visible");
    let text_input = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(WIDGET_STATES_TEXT_INPUT_LABEL)
        })
        .expect("state text input should be visible");
    let button_center = Point::new(
        button.bounds.x() + button.bounds.width() * 0.5,
        button.bounds.y() + button.bounds.height() * 0.5,
    );
    let input_center = Point::new(
        text_input.bounds.x() + text_input.bounds.width() * 0.5,
        text_input.bounds.y() + text_input.bounds.height() * 0.5,
    );
    let raised_surfaces =
        solid_fill_bounds(&output, DefaultTheme::default().palette.surface_raised);

    assert!(
        raised_surfaces
            .iter()
            .any(|bounds| { bounds.contains(button_center) && bounds.contains(input_center) }),
        "the action and text-entry state columns should share one raised row surface"
    );
}

#[test]
fn widget_book_size_presets_section_exposes_contextual_size_samples() {
    let root = SizedBox::new().width(1040.0).height(760.0).with_child(
        super::build_size_presets_gallery_with_theme(super::default_widget_book_theme_reader()),
    );
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Size presets").root(root))
        .build()
        .expect("size preset section runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("size preset section should render");
    let semantics = runtime
        .semantics(window_id)
        .expect("size preset section semantics should exist");

    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::GenericContainer
            && node.name.as_deref() == Some(super::SIZE_PRESETS_GALLERY_NAME)
    }));

    let button_height = |name: &str| {
        semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some(name))
            .map(|node| node.bounds.height())
            .unwrap_or_else(|| panic!("missing {name} preset action button"))
    };
    let small_button = button_height(super::SIZE_PRESET_SMALL_ACTION_LABEL);
    let medium_button = button_height(super::SIZE_PRESET_MEDIUM_ACTION_LABEL);
    let large_button = button_height(super::SIZE_PRESET_LARGE_ACTION_LABEL);

    assert!(small_button < medium_button);
    assert!(medium_button < large_button);
    for (actual, size) in [
        (small_button, super::ControlSize::Small),
        (medium_button, super::ControlSize::Medium),
        (large_button, super::ControlSize::Large),
    ] {
        let expected = DefaultTheme::default().with_size(size).metrics.min_height;
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {size:?} action height {expected}, got {actual}"
        );
    }

    for (name, size) in [
        (
            super::SIZE_PRESET_SMALL_INPUT_LABEL,
            super::ControlSize::Small,
        ),
        (
            super::SIZE_PRESET_MEDIUM_INPUT_LABEL,
            super::ControlSize::Medium,
        ),
        (
            super::SIZE_PRESET_LARGE_INPUT_LABEL,
            super::ControlSize::Large,
        ),
    ] {
        let node = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput && node.name.as_deref() == Some(name)
            })
            .unwrap_or_else(|| panic!("missing {name}"));
        let expected = DefaultTheme::default().with_size(size).metrics.min_height;
        assert!(
            (node.bounds.height() - expected).abs() < 0.01,
            "expected {size:?} input height {expected}, got {:?}",
            node.bounds
        );
    }
}

#[test]
fn widget_book_size_presets_wrap_without_overflow_at_narrow_width() {
    const VIEW_WIDTH: f32 = 420.0;
    let root = SizedBox::new().width(VIEW_WIDTH).height(1800.0).with_child(
        super::build_size_presets_gallery_with_theme(super::default_widget_book_theme_reader()),
    );
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Narrow size presets").root(root))
        .build()
        .expect("narrow size preset runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("narrow size presets should render");
    let semantics = runtime
        .semantics(window_id)
        .expect("narrow size preset semantics should exist");

    let action_bounds = |name: &str| {
        semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Button && node.name.as_deref() == Some(name))
            .map(|node| node.bounds)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let small = action_bounds(super::SIZE_PRESET_SMALL_ACTION_LABEL);
    let medium = action_bounds(super::SIZE_PRESET_MEDIUM_ACTION_LABEL);
    let large = action_bounds(super::SIZE_PRESET_LARGE_ACTION_LABEL);

    for bounds in [small, medium, large] {
        assert!(
            bounds.x() >= 0.0 && bounds.max_x() <= VIEW_WIDTH + 0.01,
            "size preset action should remain inside the narrow viewport: {bounds:?}"
        );
    }
    assert!(
        small.y() < medium.y() && medium.y() < large.y(),
        "narrow size preset cards should wrap into separate rows: {small:?}, {medium:?}, {large:?}"
    );
}

#[test]
fn control_story_pairs_keep_two_columns_wide_and_wrap_narrow() {
    const FIRST_ACTION: &str = "First responsive story action";
    const SECOND_ACTION: &str = "Second responsive story action";

    let render_actions = |width: f32| {
        let theme_reader = super::default_widget_book_theme_reader();
        let row = super::responsive_control_story_pair(
            super::control_story_with_theme(
                Rc::clone(&theme_reader),
                "First story",
                "Responsive story caption",
                sui::Button::new(FIRST_ACTION),
            ),
            super::control_story_with_theme(
                Rc::clone(&theme_reader),
                "Second story",
                "Responsive story caption",
                sui::Button::new(SECOND_ACTION),
            ),
        );
        let root = SizedBox::new().width(width).height(800.0).with_child(row);
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Responsive stories").root(root))
            .build()
            .expect("responsive story runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .expect("responsive stories should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("responsive story semantics should exist");
        [FIRST_ACTION, SECOND_ACTION].map(|name| {
            semantics
                .iter()
                .find(|node| {
                    node.role == SemanticsRole::Button && node.name.as_deref() == Some(name)
                })
                .map(|node| node.bounds)
                .unwrap_or_else(|| panic!("missing {name}"))
        })
    };

    let [wide_first, wide_second] = render_actions(1040.0);
    assert!((wide_first.y() - wide_second.y()).abs() < 0.01);
    assert!(wide_first.x() < wide_second.x());

    let [narrow_first, narrow_second] = render_actions(420.0);
    assert!(narrow_first.y() < narrow_second.y());
    for bounds in [narrow_first, narrow_second] {
        assert!(
            bounds.x() >= 0.0 && bounds.max_x() <= 420.01,
            "responsive story action should remain inside the narrow viewport: {bounds:?}"
        );
    }
}

#[test]
fn widget_book_choices_ranges_and_selects_use_consistent_heights() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    scroll_to_story_target(&window, StoryCase::Slider, 12)?;
    let snapshot = window.snapshot()?;
    let semantics = &snapshot.accessibility.nodes;
    let theme = DefaultTheme::default();
    let style = theme.body_text_style();
    let padding = theme.metrics.text_input_padding;
    let expected_height =
        (style.line_height + padding.top + padding.bottom).max(theme.metrics.min_height);

    for (role, name) in [
        (SemanticsRole::Switch, SWITCH_LABEL),
        (SemanticsRole::RadioButton, RADIO_BUTTON_LABEL),
        (SemanticsRole::Slider, SLIDER_NAME),
        (SemanticsRole::SpinBox, NUMBER_INPUT_NAME),
        (SemanticsRole::ComboBox, SELECT_NAME),
    ] {
        let node = semantics
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("missing {role:?} named {name:?}"));
        assert!(
            (node.bounds.height() - expected_height).abs() < 0.01,
            "expected {role:?} named {name:?} to use the theme control height {expected_height}, got {:?}",
            node.bounds
        );
    }

    Ok(())
}

#[test]
fn hdr_theme_lab_includes_emissive_indicator_and_popup_examples() {
    let mut runtime = build_theme_demo_application(default_widget_book_state())
        .build()
        .expect("theme demo runtime should build");
    let window_id = runtime.window_ids()[0];
    runtime
        .render(window_id)
        .expect("theme demo should render for HDR lab semantics");
    let semantics = runtime
        .semantics(window_id)
        .expect("theme demo semantics should exist");
    let full_hdr_title = super::hdr_theme_mode_title(super::HdrThemeMode::FullHdr);
    let swatch_name = format!("{full_hdr_title} emissive indicator");
    let popover_name = format!("{full_hdr_title} attention popover");
    let popover_trigger = format!("{full_hdr_title} attention trigger");

    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::ColorSwatch
            && node.name.as_deref() == Some(swatch_name.as_str())
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Button && node.name.as_deref() == Some(popover_trigger.as_str())
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Popover && node.name.as_deref() == Some(popover_name.as_str())
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::Popover
            && node.name.as_deref() == Some(popover_name.as_str())
            && node.state.expanded == Some(false)
    }));
    assert!(semantics.iter().any(|node| {
        node.role == SemanticsRole::GenericContainer
            && node.description.as_deref().is_some_and(|description| {
                description.contains("button, switch, emissive indicator, and popup trigger")
            })
            && node.name.as_deref() == Some(super::HDR_THEME_LAB_NAME)
    }));
}

#[test]
fn hdr_theme_lab_full_hdr_emits_stronger_headroom_than_constrained() {
    let mut constrained_runtime =
        Application::new()
            .window(WindowBuilder::new().title("Constrained HDR lab").root(
                super::hdr_theme_lab_card(
                    "Constrained HDR isolated",
                    super::HdrThemeMode::ConstrainedHdr,
                    "Constrained HDR isolated",
                    "Constrained HDR isolated preview",
                ),
            ))
            .build()
            .expect("constrained HDR lab runtime should build");
    let constrained_window = constrained_runtime.window_ids()[0];
    let constrained_output = constrained_runtime
        .render(constrained_window)
        .expect("constrained HDR lab should render");

    let mut full_runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Full HDR lab")
                .root(super::hdr_theme_lab_card(
                    "Full HDR isolated",
                    super::HdrThemeMode::FullHdr,
                    "Full HDR isolated",
                    "Full HDR isolated preview",
                )),
        )
        .build()
        .expect("full HDR lab runtime should build");
    let full_window = full_runtime.window_ids()[0];
    let full_output = full_runtime
        .render(full_window)
        .expect("full HDR lab should render");

    let constrained_max = solid_fill_max_channel(&constrained_output);
    let full_max = solid_fill_max_channel(&full_output);

    assert!(
        constrained_max > 1.0,
        "constrained HDR lab should emit above-reference-white colors, got {constrained_max}"
    );
    assert!(
        full_max > constrained_max,
        "full HDR lab should exceed constrained HDR scene headroom, got full={full_max} constrained={constrained_max}"
    );
    assert!(
        full_max >= 2.0,
        "full HDR lab should emit clearly HDR-bright values, got {full_max}"
    );
}

#[test]
fn widget_book_theme_preview_grid_exposes_all_builtin_themes() -> Result<()> {
    let app = build_default_theme_demo_app()?;
    let window = app.main_window()?;

    scroll_to_story_target(&window, StoryCase::ThemePreview, 2)?;
    let snapshot = window.snapshot()?;
    let card_bounds = [
        LIGHT_THEME_PREVIEW_CARD_NAME,
        NEUTRAL_THEME_PREVIEW_CARD_NAME,
        DARK_THEME_PREVIEW_CARD_NAME,
        NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME,
        TRUE_BLACK_THEME_PREVIEW_CARD_NAME,
    ]
    .map(|name| {
        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer && node.name.as_deref() == Some(name)
            })
            .unwrap_or_else(|| panic!("missing theme preview card {name}"))
            .bounds
    });

    assert_eq!(card_bounds[0].y(), card_bounds[1].y());
    assert_eq!(card_bounds[1].y(), card_bounds[2].y());
    assert!(card_bounds[0].x() < card_bounds[1].x());
    assert!(card_bounds[1].x() < card_bounds[2].x());
    assert!(card_bounds[3].y() > card_bounds[0].y());
    assert_eq!(card_bounds[3].y(), card_bounds[4].y());

    Ok(())
}

#[test]
fn widget_book_theme_preview_grid_uses_responsive_columns() {
    assert_eq!(super::ThemePreviewGrid::columns_for_width(1200.0), 3);
    assert_eq!(super::ThemePreviewGrid::columns_for_width(900.0), 2);
    assert_eq!(super::ThemePreviewGrid::columns_for_width(520.0), 1);
}

#[test]
fn widget_book_popover_click_repaints_gallery() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;

    scroll_to_story_target(&window, StoryCase::PopoverOpen, 12)?;
    let before = window.capture_screenshot()?;

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(POPOVER_TRIGGER_LABEL)
        .click()?;

    window
        .get_by_role(SemanticsRole::Popover)
        .with_name(POPOVER_NAME)
        .capture_screenshot()?;
    let after = window.capture_screenshot()?;

    assert_ne!(before, after);

    Ok(())
}

#[test]
fn widget_book_project_settings_click_repaints_gallery() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;

    scroll_to_story_target(&window, StoryCase::Dialog, 12)?;
    let before = window.capture_screenshot()?;

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(DIALOG_TRIGGER_LABEL)
        .click()?;

    window
        .get_by_role(SemanticsRole::Dialog)
        .with_name(DIALOG_TITLE)
        .capture_screenshot()?;
    let after = window.capture_screenshot()?;

    assert_ne!(before, after);

    Ok(())
}

#[test]
fn widget_book_tooltip_hides_after_pointer_moves_to_another_control() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;

    scroll_to_story_target(&window, StoryCase::TooltipVisible, 12)?;

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(TOOLTIP_TRIGGER_LABEL)
        .hover()?;
    assert_eq!(
        window
            .get_by_role(SemanticsRole::Tooltip)
            .with_name(TOOLTIP_TEXT)
            .count()?,
        1
    );

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(POPOVER_TRIGGER_LABEL)
        .hover()?;
    assert_eq!(
        window
            .get_by_role(SemanticsRole::Tooltip)
            .with_name(TOOLTIP_TEXT)
            .count()?,
        0
    );

    Ok(())
}

#[test]
fn widget_book_text_input_accepts_plain_keyboard_typing() -> Result<()> {
    let baseline_summary = {
        let baseline_app = build_default_widget_book_app()?;
        let baseline_window = baseline_app.main_window()?;
        scroll_to_story_target(&baseline_window, StoryCase::Summary, 12)?;
        baseline_window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(SUMMARY_NAME)
            .capture_screenshot()?
    };

    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;

    scroll_to_story_target(&window, StoryCase::FilledInput, 12)?;
    let input = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(NAME_INPUT_LABEL);
    input.focus()?;
    input.press("Z")?;
    let input_value = window
        .snapshot()?
        .accessibility
        .nodes
        .into_iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput && node.name.as_deref() == Some(NAME_INPUT_LABEL)
        })
        .and_then(|node| match node.value {
            Some(SemanticsValue::Text(value)) => Some(value),
            _ => None,
        })
        .expect("text input semantics value present after typing");
    assert_eq!(input_value, "AdaZ");

    scroll_to_story_target(&window, StoryCase::Summary, 12)?;
    let summary_description = window
        .snapshot()?
        .accessibility
        .nodes
        .into_iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(SUMMARY_NAME)
        })
        .and_then(|node| node.description)
        .expect("summary semantics description present after typing");
    assert!(
        summary_description.contains("AdaZ"),
        "summary semantics did not reflect the typed name: {summary_description}"
    );
    let edited_summary = window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name(SUMMARY_NAME)
        .capture_screenshot()?;

    assert!(
        edited_summary != baseline_summary,
        "summary screenshot did not change after typing"
    );

    Ok(())
}

#[test]
fn widget_book_password_and_datetime_inputs_are_editable() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;

    scroll_to_story_target(&window, StoryCase::FilledInput, 12)?;
    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME)
        .scroll_pixels(Vector::new(0.0, -120.0))?;
    let password = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(PASSWORD_INPUT_LABEL);
    password.focus()?;
    password.press("Z")?;

    let snapshot = window.snapshot()?;
    let password = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(PASSWORD_INPUT_LABEL)
        })
        .expect("widget-book password input semantics present");
    assert_eq!(
        password.value,
        Some(SemanticsValue::Text("sui-demoZ".to_string()))
    );
    assert!(password.editable_text.as_ref().unwrap().password);

    let datetime = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(DATETIME_INPUT_LABEL);
    datetime.focus()?;
    datetime.press("Z")?;

    let snapshot = window.snapshot()?;
    let datetime_value = snapshot
        .accessibility
        .nodes
        .into_iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(DATETIME_INPUT_LABEL)
        })
        .and_then(|node| match node.value {
            Some(SemanticsValue::Text(value)) => Some(value),
            _ => None,
        })
        .expect("widget-book date/time input semantics value present after typing");
    assert_eq!(datetime_value, "2026-07-15 14:30Z");

    Ok(())
}

#[test]
fn widget_book_summary_uses_live_dark_theme_tokens() -> Result<()> {
    let theme = DefaultTheme::dark();
    let theme_reader: super::WidgetBookThemeReader = Rc::new(move || theme);
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Widget book summary").root(
            super::WidgetBookSummary::new(default_widget_book_state(), theme_reader),
        ))
        .build()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;
    let fills = solid_fill_colors(&output);

    assert!(fills.contains(&theme.palette.surface_raised));
    assert!(
        !fills.contains(&sui::Color::rgba(0.985, 0.99, 1.0, 1.0)),
        "dark live summary should not use the old hardcoded light panel fill"
    );
    Ok(())
}

#[test]
fn text_validation_surface_supports_ime_and_selection() -> Result<()> {
    let app = build_text_validation_app()?;
    let window = app.main_window()?;
    let editor = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_VALIDATION_EDITOR_NAME);

    editor.focus()?;
    let before_selection = editor.capture_screenshot()?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
        text: " // validatedðŸ™‚".to_string(),
        cursor_range: None,
    }))?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit {
        text: " // validatedðŸ™‚".to_string(),
    }))?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;

    let mut shift_left = KeyboardEvent::new("ArrowLeft", KeyState::Pressed);
    shift_left.modifiers.shift = true;
    for _ in 0..6 {
        editor.dispatch_event(Event::Keyboard(shift_left.clone()))?;
    }

    let after_selection = editor.capture_screenshot()?;
    assert_ne!(before_selection, after_selection);

    let editor_value = window
        .snapshot()?
        .accessibility
        .nodes
        .into_iter()
        .find(|node| {
            node.role == SemanticsRole::TextInput
                && node.name.as_deref() == Some(TEXT_VALIDATION_EDITOR_NAME)
        })
        .and_then(|node| match node.value {
            Some(SemanticsValue::Text(value)) => Some(value),
            _ => None,
        })
        .expect("validation editor semantics value present after IME commit");
    assert!(editor_value.contains("validatedðŸ™‚"));

    Ok(())
}

#[test]
fn widget_book_gallery_wheel_scroll_updates_screenshot_and_reveals_lower_story() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);

    let before = gallery.capture_screenshot()?;

    gallery.scroll_pixels(Vector::new(0.0, -360.0))?;

    let after = gallery.capture_screenshot()?;

    assert_ne!(before, after);

    Ok(())
}

#[test]
fn widget_book_gallery_scroll_bar_drag_repaints_content_immediately() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);
    let snapshot = window.snapshot()?;
    let scroll_bar = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(GALLERY_SCROLL_BAR_NAME)
        })
        .expect("widget book gallery scroll bar should be present");
    let before_value = scroll_bar.value.clone();
    let start = Point::new(
        scroll_bar.bounds.x() + scroll_bar.bounds.width() * 0.5,
        scroll_bar.bounds.y() + 24.0,
    );
    let end = Point::new(
        start.x,
        (start.y + 300.0).min(scroll_bar.bounds.max_y() - 24.0),
    );

    let root = window.root();
    let mut down = PointerEvent::new(PointerEventKind::Down, start);
    down.pointer_id = 73;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    root.dispatch_event(Event::Pointer(down))?;

    let before = gallery.capture_screenshot()?;
    let content_crop = Rect::new(
        16.0,
        16.0,
        (before.width() as f32 - 64.0).max(1.0),
        (before.height() as f32 - 32.0).max(1.0),
    );
    let before_content = before.crop(content_crop)?;

    let mut moved = PointerEvent::new(PointerEventKind::Move, end);
    moved.pointer_id = 73;
    moved.buttons = PointerButtons::new(1);
    moved.delta = end - start;
    root.dispatch_event(Event::Pointer(moved))?;

    let after_value = window
        .snapshot()?
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(GALLERY_SCROLL_BAR_NAME)
        })
        .and_then(|node| node.value.clone());
    let after_content = gallery.capture_screenshot()?.crop(content_crop)?;

    assert_ne!(before_value, after_value, "the drag should move the thumb");
    assert_ne!(
        before_content, after_content,
        "one captured mouse move must redraw gallery content, not only the thumb"
    );

    Ok(())
}

#[test]
fn widget_book_gallery_scroll_redraws_when_split_view_is_visible() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    scroll_to_story_target(&window, StoryCase::SplitView, 12)?;

    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);

    let before = gallery.capture_screenshot()?;

    gallery.scroll_pixels(Vector::new(0.0, -48.0))?;

    let after = gallery.capture_screenshot()?;

    assert_ne!(before, after);

    Ok(())
}

#[test]
fn widget_book_text_area_focus_does_not_trap_gallery_wheel_scroll() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    scroll_to_story_target(&window, StoryCase::TextArea, 12)?;
    let text_area = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_AREA_LABEL);

    text_area.click()?;
    let before = window.capture_screenshot()?;
    text_area.scroll_pixels(Vector::new(0.0, -240.0))?;
    let after = window.capture_screenshot()?;

    assert_ne!(
        before, after,
        "wheel scrolling over the focused multiline editor should still move the gallery"
    );
    Ok(())
}

#[test]
fn widget_book_gallery_small_wheel_scroll_updates_screenshot() -> Result<()> {
    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);

    let before = gallery.capture_screenshot()?;

    gallery.scroll_pixels(Vector::new(0.0, -12.0))?;

    let after = gallery.capture_screenshot()?;

    assert_ne!(before, after);

    Ok(())
}

#[test]
fn widget_book_gallery_exposes_visible_scroll_bar() {
    let mut runtime = build_widget_book_application(default_widget_book_state())
        .build()
        .expect("widget book runtime should build");
    let window_id = runtime.window_ids()[0];
    let output = runtime
        .render(window_id)
        .expect("widget book should render");
    let gallery = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
        })
        .expect("widget book gallery scroll view should be present");
    let scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(GALLERY_SCROLL_BAR_NAME)
        })
        .expect("widget book gallery scroll bar should be present");

    assert!(scroll_bar.bounds.x() >= gallery.bounds.x());
    assert!(scroll_bar.bounds.max_x() <= gallery.bounds.max_x());
    assert!(scroll_bar.bounds.y() >= gallery.bounds.y());
    assert!(scroll_bar.bounds.max_y() <= gallery.bounds.max_y());
    assert!(scroll_bar.bounds.height() >= gallery.bounds.height() - 8.0);
}

#[test]
fn widget_book_gallery_scroll_bar_uses_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        WINDOW_TITLE,
        Size::new(520.0, 420.0),
        super::build_widget_book_gallery_with_theme(
            default_widget_book_state(),
            widget_book_theme_reader(theme),
        ),
    );
    let scroll_bar = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Slider
                && node.name.as_deref() == Some(GALLERY_SCROLL_BAR_NAME)
        })
        .expect("widget book gallery scroll bar should be present");

    assert_eq!(
        scroll_bar.bounds.width(),
        theme.metrics.scroll_bar_thickness
    );
}

#[test]
fn widget_book_title_scrolls_with_gallery_and_theme_root_starts_at_scroll_top() -> Result<()> {
    fn assert_title_flush_with_scroll(output: &RenderOutput, scroll_name: &str, title: &str) {
        let scroll = output
            .semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ScrollView && node.name.as_deref() == Some(scroll_name)
            })
            .expect("root scroll view should be present");
        let title_node = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some(title))
            .expect("root title text should be present");

        assert!(
            (title_node.bounds.y() - scroll.bounds.y()).abs() < 0.01,
            "{title} should start at the scroll viewport top: title={:?}, scroll={:?}",
            title_node.bounds,
            scroll.bounds
        );
    }

    let app = build_default_widget_book_app()?;
    let window = app.main_window()?;
    let initial = window.snapshot()?;
    let gallery = initial
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ScrollView
                && node.name.as_deref() == Some(GALLERY_SCROLL_NAME)
        })
        .expect("widget-book gallery should be present");
    let title = initial
        .accessibility
        .nodes
        .iter()
        .find(|node| node.role == SemanticsRole::Text && node.name.as_deref() == Some(WINDOW_TITLE))
        .expect("widget-book title should be present in the gallery intro");

    assert!(title.bounds.y() >= gallery.bounds.y());
    assert!(title.bounds.max_y() <= gallery.bounds.max_y());

    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME)
        .scroll_pixels(Vector::new(0.0, -3_000.0))?;
    let scrolled = window.snapshot()?;
    assert!(scrolled.accessibility.nodes.iter().all(|node| {
        node.role != SemanticsRole::Text || node.name.as_deref() != Some(WINDOW_TITLE)
    }));

    let mut theme_runtime = build_theme_demo_application(default_widget_book_state()).build()?;
    let theme_window = theme_runtime.window_ids()[0];
    let theme_output = theme_runtime.render(theme_window)?;
    assert_title_flush_with_scroll(&theme_output, THEME_DEMO_SCROLL_NAME, THEME_DEMO_TITLE);

    Ok(())
}

#[test]
fn widget_book_gallery_exposes_color_picker_story() -> Result<()> {
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Color story")
                .root(build_color_and_imagery_story()),
        )
        .build()?;
    let window_id = runtime.window_ids()[0];
    let output = runtime.render(window_id)?;
    let picker = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ColorPicker
                && node.name.as_deref() == Some(COLOR_PICKER_NAME)
        })
        .expect("widget book gallery should expose the color picker story");

    assert!(picker.bounds.width() >= 420.0);
    assert!(picker.bounds.height() >= 424.0);
    Ok(())
}

#[cfg(feature = "artifacts")]
#[test]
fn widget_book_visual_artifacts_include_hdr_widget_book_capture() -> Result<()> {
    let artifact_root = unique_visual_artifact_test_dir("hdr-widget-book");
    let output_root = super::visual_artifacts::write_visual_artifacts_to(&artifact_root)?;
    let hdr_dir = output_root.join("hdr-widget-book");

    assert!(hdr_dir.join("window.png").exists());
    assert!(hdr_dir.join("hdr-intermediate.exr").exists());
    assert!(hdr_dir.join("hdr-intermediate.avif").exists());
    assert!(hdr_dir.join("luminance-map.png").exists());
    assert!(hdr_dir.join("headroom-map.png").exists());
    assert!(hdr_dir.join("clip-mask.png").exists());
    assert!(hdr_dir.join("output-diagnostics.txt").exists());
    assert!(hdr_dir.join("capture-metrics.txt").exists());
    assert!(
        hdr_dir.join("final-composed.exr").exists()
            || hdr_dir.join("final-composed.avif").exists()
            || hdr_dir.join("final-composed.png").exists()
    );

    fs::remove_dir_all(&artifact_root).ok();
    Ok(())
}

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "slow; run `cargo run -p sinomo-ui-demo --bin sui-demo-artifacts` to generate artifacts"]
fn widget_book_generates_visual_artifacts() -> Result<()> {
    let artifact_root = super::write_visual_artifacts()?;

    for story in StoryCase::ALL {
        assert!(
            artifact_root
                .join(story.id())
                .join("screenshot.png")
                .exists()
        );
    }

    Ok(())
}

#[cfg(feature = "artifacts")]
#[test]
fn widget_book_theme_preview_switch_matches_reference_at_fractional_dpi() -> Result<()> {
    let artifact_dir = artifact_root().join("theme-preview-150-dpi");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir).map_err(|error| {
            sui::Error::new(format!(
                "failed to clear {}: {error}",
                artifact_dir.display()
            ))
        })?;
    }
    fs::create_dir_all(&artifact_dir).map_err(|error| {
        sui::Error::new(format!(
            "failed to create {}: {error}",
            artifact_dir.display()
        ))
    })?;

    let live_app = build_headless_default_theme_demo_app()?;
    let live_window = live_app.main_window()?;
    set_window_scale_factor(&live_window, 1.5, 144.0)?;
    scroll_to_story_target(&live_window, StoryCase::ThemePreview, 12)?;

    let live_artifacts = live_window.capture_artifacts()?;
    live_artifacts.write_to_dir(artifact_dir.join("live-window"))?;

    let live_light_card_locator = live_window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name(LIGHT_THEME_PREVIEW_CARD_NAME);
    let live_light_card = live_light_card_locator.capture_screenshot()?;
    let live_switch = live_window
        .get_by_role(SemanticsRole::Switch)
        .with_name("SUI light preview live updates")
        .capture_screenshot()?;
    write_screenshot(artifact_dir.join("live-light-card.png"), &live_light_card)?;
    write_screenshot(artifact_dir.join("live-light-switch.png"), &live_switch)?;

    let live_snapshot = live_window.snapshot()?;
    let live_card_bounds = live_snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(LIGHT_THEME_PREVIEW_CARD_NAME)
        })
        .map(|node| node.bounds)
        .ok_or_else(|| sui::Error::new("light theme preview card is missing"))?;

    let reference_app = build_light_theme_preview_reference_app(live_card_bounds.width())?;
    let reference_window = reference_app.main_window()?;
    set_window_scale_factor(&reference_window, 1.5, 144.0)?;

    let reference_artifacts = reference_window.capture_artifacts()?;
    reference_artifacts.write_to_dir(artifact_dir.join("reference-window"))?;

    let reference_light_card = reference_window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name(LIGHT_THEME_PREVIEW_CARD_NAME)
        .capture_screenshot()?;
    let reference_switch = reference_window
        .get_by_role(SemanticsRole::Switch)
        .with_name("SUI light preview live updates")
        .capture_screenshot()?;
    write_screenshot(
        artifact_dir.join("reference-light-card.png"),
        &reference_light_card,
    )?;
    write_screenshot(
        artifact_dir.join("reference-light-switch.png"),
        &reference_switch,
    )?;

    let (normalized_live_switch, normalized_reference_switch) =
        normalize_screenshot_pair(&live_switch, &reference_switch)?;
    write_screenshot(
        artifact_dir.join("live-light-switch-normalized.png"),
        &normalized_live_switch,
    )?;
    write_screenshot(
        artifact_dir.join("reference-light-switch-normalized.png"),
        &normalized_reference_switch,
    )?;

    let diff = screenshot_diff_image(&normalized_live_switch, &normalized_reference_switch)?;
    write_screenshot(artifact_dir.join("switch-diff.png"), &diff)?;
    let diff_count = screenshot_diff_count(&normalized_live_switch, &normalized_reference_switch);
    let switch_control_crop = sui::Rect::new(
        0.0,
        0.0,
        56.0_f32.min(normalized_live_switch.width() as f32),
        normalized_live_switch.height() as f32,
    );
    let live_switch_control = normalized_live_switch.crop(switch_control_crop)?;
    let reference_switch_control = normalized_reference_switch.crop(switch_control_crop)?;
    write_screenshot(
        artifact_dir.join("live-light-switch-control.png"),
        &live_switch_control,
    )?;
    write_screenshot(
        artifact_dir.join("reference-light-switch-control.png"),
        &reference_switch_control,
    )?;
    let control_diff = screenshot_diff_image(&live_switch_control, &reference_switch_control)?;
    write_screenshot(artifact_dir.join("switch-control-diff.png"), &control_diff)?;
    let control_diff_count = screenshot_diff_count(&live_switch_control, &reference_switch_control);
    fs::write(
            artifact_dir.join("comparison.txt"),
            format!(
                "live card: {}\nreference card: isolated {}\nlive switch: {}x{}\nreference switch: {}x{}\nnormalized switch: {}x{}\nfull-row diff pixels: {}\nswitch-control diff pixels: {}\n",
                LIGHT_THEME_PREVIEW_CARD_NAME,
                LIGHT_THEME_PREVIEW_CARD_NAME,
                live_switch.width(),
                live_switch.height(),
                reference_switch.width(),
                reference_switch.height(),
                normalized_live_switch.width(),
                normalized_live_switch.height(),
                diff_count,
                control_diff_count,
            ),
        )
        .map_err(|error| {
            sui::Error::new(format!(
                "failed to write comparison metadata in {}: {error}",
                artifact_dir.display()
            ))
        })?;

    assert!(
        control_diff_count <= 550,
        "theme preview switch control differed from isolated reference at 150% DPI; diff pixels={control_diff_count}; see {}",
        artifact_dir.display()
    );

    Ok(())
}

#[test]
fn widget_book_configured_story_renders_expected_visual_state() -> Result<()> {
    let (
        _default_slider,
        default_number_value,
        default_select_value,
        default_summary,
        default_slider_value,
    ) = {
        let default_app = build_default_widget_book_app()?;
        let default_window = default_app.main_window()?;
        scroll_to_story_target(&default_window, StoryCase::Slider, 12)?;
        let default_slider = default_window
            .get_by_role(SemanticsRole::Slider)
            .with_name(SLIDER_NAME)
            .capture_screenshot()?;
        let default_slider_value = default_window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::Slider && node.name.as_deref() == Some(SLIDER_NAME)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Range { value, .. }) => Some(value),
                _ => None,
            })
            .expect("default slider semantics value present");
        scroll_to_story_target(&default_window, StoryCase::NumberInput, 12)?;
        let default_number_value = default_window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Range { value, .. }) => Some(value),
                _ => None,
            })
            .expect("default number input semantics value present");
        scroll_to_story_target(&default_window, StoryCase::SelectExpanded, 12)?;
        let default_select_value = combo_box_text_value(&default_window, SELECT_NAME)?;
        scroll_to_story_target(&default_window, StoryCase::Summary, 12)?;
        let default_summary = default_window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(SUMMARY_NAME)
            .capture_screenshot()?;
        (
            default_slider,
            default_number_value,
            default_select_value,
            default_summary,
            default_slider_value,
        )
    };

    let (
        _configured_slider,
        configured_number_value,
        configured_select_value,
        configured_summary,
        configured_slider_value,
    ) = {
        let configured_app = build_configured_widget_book_app()?;
        let configured_window = configured_app.main_window()?;
        scroll_to_story_target(&configured_window, StoryCase::Slider, 12)?;
        let configured_slider = configured_window
            .get_by_role(SemanticsRole::Slider)
            .with_name(SLIDER_NAME)
            .capture_screenshot()?;
        let configured_slider_value = configured_window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::Slider && node.name.as_deref() == Some(SLIDER_NAME)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Range { value, .. }) => Some(value),
                _ => None,
            })
            .expect("configured slider semantics value present");
        scroll_to_story_target(&configured_window, StoryCase::NumberInput, 12)?;
        let configured_number_value = configured_window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Range { value, .. }) => Some(value),
                _ => None,
            })
            .expect("configured number input semantics value present");
        scroll_to_story_target(&configured_window, StoryCase::SelectExpanded, 12)?;
        let configured_select_value = combo_box_text_value(&configured_window, SELECT_NAME)?;
        scroll_to_story_target(&configured_window, StoryCase::Summary, 12)?;
        let configured_summary = configured_window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(SUMMARY_NAME)
            .capture_screenshot()?;
        (
            configured_slider,
            configured_number_value,
            configured_select_value,
            configured_summary,
            configured_slider_value,
        )
    };

    assert_eq!(default_slider_value, 72.0);
    assert_eq!(configured_slider_value, 35.0);
    assert_eq!(default_number_value, 12.0);
    assert_eq!(configured_number_value, 24.0);
    assert_eq!(default_select_value, "Normal");
    assert_eq!(configured_select_value, "Multiply");

    assert!(
        configured_summary != default_summary,
        "configured summary screenshot matched default state"
    );

    Ok(())
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
    let mut runtime = build_widget_book_application(default_widget_book_state())
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

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "diagnostic benchmark for current headless widget-book scroll status"]
fn widget_book_headless_scroll_current_status_benchmark() -> Result<()> {
    let _guard = headless_benchmark_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let app = build_headless_default_widget_book_app()?;
    let window = app.main_window()?;
    set_detailed_scene_statistics_mode(&window)?;
    let samples = collect_headless_scroll_benchmark_samples(&window, GALLERY_SCROLL_NAME, 24)?;

    print_widget_book_headless_scroll_benchmark_summary(
        "Widget Book Headless Scroll Benchmark",
        &samples,
    );
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for widget-book tree construction and destruction"]
fn widget_book_tree_creation_current_status_benchmark() {
    let mut creation_ms = Vec::with_capacity(7);
    let mut destruction_ms = Vec::with_capacity(7);
    for _ in 0..7 {
        let started = Instant::now();
        let gallery = std::hint::black_box(build_widget_book_gallery(default_widget_book_state()));
        creation_ms.push(started.elapsed().as_secs_f64() * 1_000.0);

        let started = Instant::now();
        drop(gallery);
        destruction_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    creation_ms.sort_by(f64::total_cmp);
    destruction_ms.sort_by(f64::total_cmp);
    println!(
        "widget-book tree creation: median={:.3} ms min={:.3} ms max={:.3} ms",
        creation_ms[creation_ms.len() / 2],
        creation_ms[0],
        creation_ms[creation_ms.len() - 1]
    );
    println!(
        "widget-book tree destruction: median={:.3} ms min={:.3} ms max={:.3} ms",
        destruction_ms[destruction_ms.len() / 2],
        destruction_ms[0],
        destruction_ms[destruction_ms.len() - 1]
    );
}

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "diagnostic benchmark for current headless overlay-free widget-book gallery status"]
fn widget_book_headless_gallery_only_scroll_current_status_benchmark() -> Result<()> {
    let _guard = headless_benchmark_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let app = build_gallery_only_widget_book_app()?;
    let window = app.main_window()?;
    set_detailed_scene_statistics_mode(&window)?;
    let samples = collect_headless_scroll_benchmark_samples(&window, GALLERY_SCROLL_NAME, 24)?;

    print_widget_book_headless_scroll_benchmark_summary(
        "Widget Book Headless Gallery-Only Scroll Benchmark",
        &samples,
    );
    Ok(())
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

    print_widget_book_headless_scroll_benchmark_summary(
        "Retained Text Headless Scroll Benchmark",
        &samples,
    );
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

    print_widget_book_headless_scroll_benchmark_summary(
        "Text Editing Headless Benchmark",
        &samples,
    );
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

    print_widget_book_headless_scroll_benchmark_summary("Animation Headless Benchmark", &samples);
    Ok(())
}

#[test]
fn widget_book_exposes_compact_performance_overlay_semantics() {
    let mut runtime = build_widget_book_application_with_overlay(default_widget_book_state())
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
