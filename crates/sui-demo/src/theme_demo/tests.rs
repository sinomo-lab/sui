use std::fs;

use sui::{
    Application, DefaultTheme, Result, SemanticsRole, Size, SizedBox, Vector, WindowBuilder,
};
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;

/// Scrolls the Themes page until the preview grid is fully in view.
fn reveal_theme_preview(window: &TestWindow) -> Result<()> {
    for _ in 0..40 {
        let snapshot = window.snapshot()?;
        let viewport = viewport_size(window)?;
        let grid = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.name.as_deref() == Some(THEME_PREVIEW_NAME))
            .map(|node| node.bounds);
        if let Some(grid) = grid
            && grid.y() >= 0.0
            && (grid.max_y() <= viewport.height || grid.y() <= 24.0)
        {
            return Ok(());
        }
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(THEME_DEMO_SCROLL_NAME)
            .scroll_pixels(Vector::new(0.0, -120.0))?;
    }
    Err(sui::Error::new(
        "theme preview grid did not scroll into view",
    ))
}

fn build_default_theme_demo_app() -> Result<TestApp> {
    TestApp::new(|| build_theme_demo_application().build())
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
                            .height(super::ThemePreviewGrid::MIN_CARD_HEIGHT)
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
fn build_headless_default_theme_demo_app() -> Result<TestApp> {
    TestApp::from_runtime(build_theme_demo_application().build()?)
}

#[test]
fn hdr_theme_lab_exposes_mode_comparison_sections() {
    let mut runtime = build_theme_demo_application()
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
fn hdr_theme_lab_includes_emissive_indicator_and_popup_examples() {
    let mut runtime = build_theme_demo_application()
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

    reveal_theme_preview(&window)?;
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
fn widget_book_theme_preview_cards_fit_wrapped_descriptions() -> Result<()> {
    let app = build_default_theme_demo_app()?;
    let window = app.main_window()?;

    reveal_theme_preview(&window)?;
    let snapshot = window.snapshot()?;
    let node_bounds = |role: SemanticsRole, name: &str| {
        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("missing {role:?} {name}"))
            .bounds
    };

    // The neutral cards have the longest descriptions, which wrap onto a
    // second line; their swatches must keep full size inside the card.
    for (card_name, title) in [
        (LIGHT_THEME_PREVIEW_CARD_NAME, "SUI light"),
        (NEUTRAL_THEME_PREVIEW_CARD_NAME, "Neutral light"),
        (DARK_THEME_PREVIEW_CARD_NAME, "SUI dark"),
        (NEUTRAL_DARK_THEME_PREVIEW_CARD_NAME, "Neutral dark"),
        (TRUE_BLACK_THEME_PREVIEW_CARD_NAME, "SUI true black"),
    ] {
        let card = node_bounds(SemanticsRole::GenericContainer, card_name);
        for swatch_kind in ["base", "primary", "secondary"] {
            let swatch = node_bounds(
                SemanticsRole::ColorSwatch,
                &format!("{title} {swatch_kind} swatch"),
            );
            assert!(
                (swatch.height() - 28.0).abs() < 0.5,
                "{title} {swatch_kind} swatch was squashed to {swatch:?}"
            );
            assert!(
                swatch.max_y() <= card.max_y() + 0.5,
                "{title} {swatch_kind} swatch {swatch:?} overflows its card {card:?}"
            );
        }
    }

    // Cards in the same row still share one height.
    let first_row = [
        LIGHT_THEME_PREVIEW_CARD_NAME,
        NEUTRAL_THEME_PREVIEW_CARD_NAME,
        DARK_THEME_PREVIEW_CARD_NAME,
    ]
    .map(|name| node_bounds(SemanticsRole::GenericContainer, name).height());
    assert_eq!(first_row[0], first_row[1]);
    assert_eq!(first_row[1], first_row[2]);

    Ok(())
}

#[test]
fn widget_book_theme_preview_grid_uses_responsive_columns() {
    assert_eq!(super::ThemePreviewGrid::columns_for_width(1200.0), 3);
    assert_eq!(super::ThemePreviewGrid::columns_for_width(900.0), 2);
    assert_eq!(super::ThemePreviewGrid::columns_for_width(520.0), 1);
}

#[cfg(feature = "artifacts")]
#[test]
fn widget_book_theme_preview_switch_matches_reference_at_fractional_dpi() -> Result<()> {
    let artifact_dir =
        crate::widget_book::visual_artifacts::artifact_root().join("theme-preview-150-dpi");
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
    reveal_theme_preview(&live_window)?;

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
fn theme_demo_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        THEME_DEMO_TITLE,
        Size::new(760.0, 520.0),
        build_theme_demo_surface_with_theme,
    )
}
