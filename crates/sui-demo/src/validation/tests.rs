use sui::{
    Application, DefaultTheme, Event, ImeEvent, KeyState, KeyboardEvent, Point, PointerEvent,
    PointerEventKind, Result, ScrollDelta, SemanticsRole, SemanticsValue, Size, SizedBox, Vector,
    WindowBuilder,
};
use sui_scene::{SceneCommand, SceneLayerUpdateKind};
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;

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
        super::build_text_rendering_comparison_surface_with_theme(theme_reader(theme)),
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
fn color_validation_surface_exposes_its_reference_swatches() {
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
        "sRGB clipped red",
        "Display P3 red",
        "sRGB clipped green",
        "Display P3 green",
        "sRGB clipped cyan",
        "Display P3 cyan",
        "White 1×",
        "White 2×",
        "White 4×",
        "White 8×",
        "White 16×",
        "White 0.9×",
        "White 1.05×",
    ] {
        assert!(semantics.iter().any(|node| {
            node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some(swatch_name)
        }));
    }
}

#[test]
fn color_validation_surface_keeps_swatches_and_text_readable_when_narrow() {
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
    let brightest_swatch = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ColorSwatch && node.name.as_deref() == Some("White 16×")
        })
        .expect("the brightest ladder swatch should be present");
    let hdr_description = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::Text
                && node
                    .name
                    .as_deref()
                    .is_some_and(|name| name.starts_with("White from a quarter of SDR white"))
        })
        .expect("the headroom description should be present");

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
    // The page keeps its width and scrolls instead of squeezing probes.
    assert!(brightest_swatch.bounds.width() >= 80.0);
    assert!(brightest_swatch.bounds.height() >= 40.0);
    assert!(hdr_description.bounds.height() > 20.0);
    assert!(hdr_description.bounds.width() < 1000.0);
}

#[test]
fn color_validation_scroll_bars_use_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        super::COLOR_VALIDATION_VIEW_TITLE,
        Size::new(430.0, 320.0),
        super::build_color_validation_surface_with_theme(theme_reader(theme)),
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
        text: " // validated🙂".to_string(),
        cursor_range: None,
    }))?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit {
        text: " // validated🙂".to_string(),
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
    assert!(editor_value.contains("validated\u{1f642}"));

    Ok(())
}

#[test]
fn validation_surfaces_repaint_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        TEXT_VALIDATION_VIEW_TITLE,
        Size::new(520.0, 420.0),
        build_text_validation_surface_with_theme,
    )?;
    assert_widget_repaints_after_theme_change(
        TEXT_RENDERING_COMPARISON_TITLE,
        Size::new(520.0, 360.0),
        build_text_rendering_comparison_surface_with_theme,
    )?;
    assert_widget_repaints_after_theme_change(
        COLOR_VALIDATION_VIEW_TITLE,
        Size::new(520.0, 360.0),
        build_color_validation_surface_with_theme,
    )
}
