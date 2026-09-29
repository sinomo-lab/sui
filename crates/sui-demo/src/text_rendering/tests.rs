use sui::{Result, SemanticsRole, SemanticsValue, Size, Vector};
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_text_rendering_application)
        .vsync(false)
        .launch()
}

fn description(window: &TestWindow, name: &str) -> String {
    window
        .snapshot()
        .expect("snapshot")
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(name))
        .and_then(|node| node.description.clone())
        .unwrap_or_else(|| panic!("{name} has a description"))
}

fn node_bounds(window: &TestWindow, name: &str) -> sui::Rect {
    window
        .snapshot()
        .expect("snapshot")
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(name))
        .map(|node| node.bounds)
        .unwrap_or_else(|| panic!("{name} is on the page"))
}

/// Scroll the page until the section `name` starts at the top.
fn scroll_to(window: &TestWindow, name: &str) -> Result<()> {
    let page = node_bounds(window, TEXT_RENDERING_SCROLL_NAME);
    let section = node_bounds(window, name);
    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(TEXT_RENDERING_SCROLL_NAME)
        .scroll_pixels(Vector::new(0.0, page.y() - section.y()))?;
    window.run_until_idle()
}

#[test]
fn page_shows_each_section_and_what_draws_each_specimen() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let snapshot = window.snapshot()?;
    for name in [
        SETTINGS_SECTION_NAME,
        SIZES_SECTION_NAME,
        COMPARE_SECTION_NAME,
        PROBES_SECTION_NAME,
        POLICIES_SECTION_NAME,
        "Stem darkening probe",
        "Hinting probe",
        "Coverage on dark probe",
        "Subpixel position probe",
        "Optical centering probe",
    ] {
        assert!(
            snapshot
                .accessibility
                .nodes
                .iter()
                .any(|node| node.name.as_deref() == Some(name)),
            "{name} is on the page"
        );
    }
    // The page edits the window's text settings, like Settings does.
    assert!(snapshot.accessibility.nodes.iter().any(|node| {
        node.role == SemanticsRole::ComboBox
            && node.value.is_some()
            && node
                .name
                .as_deref()
                .is_some_and(|name| name.contains("coverage") || name.contains("Coverage"))
    }));

    let sizes = description(&window, SIZES_SPECIMEN_NAME);
    assert!(sizes.contains("10, 11, 12, 13, 14, 16, 20 px"), "{sizes}");
    assert!(
        sizes.ends_with("drawn with the window's settings."),
        "{sizes}"
    );
    assert!(description(&window, SPECIMEN_A_NAME).ends_with("drawn with Linear coverage."));
    assert!(description(&window, SPECIMEN_B_NAME).ends_with("drawn with Perceptual coverage."));
    Ok(())
}

#[test]
fn choosing_a_policy_redraws_its_specimen_and_explains_it() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    scroll_to(&window, COMPARE_SECTION_NAME)?;
    let specimen = window
        .get_by_role(SemanticsRole::GenericContainer)
        .with_name(SPECIMEN_A_NAME);
    let before = specimen.capture_screenshot()?;

    // From Linear coverage down to LCD subpixel.
    let select = window
        .get_by_role(SemanticsRole::ComboBox)
        .with_name(POLICY_A_NAME);
    select.click()?;
    for _ in 0..3 {
        window.focused().press("ArrowDown")?;
    }
    window.focused().press("Enter")?;
    window.run_until_idle()?;

    let value = window
        .snapshot()?
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.role == SemanticsRole::ComboBox && node.name.as_deref() == Some(POLICY_A_NAME)
        })
        .and_then(|node| node.value.clone());
    assert_eq!(
        value,
        Some(SemanticsValue::Text("LCD subpixel".to_string()))
    );
    assert!(description(&window, SPECIMEN_A_NAME).ends_with("drawn with LCD subpixel."));
    let texts = window
        .snapshot()?
        .accessibility
        .nodes
        .iter()
        .filter_map(|node| node.name.clone())
        .collect::<Vec<_>>();
    assert!(texts.iter().any(|text| text == POLICIES[4].summary));
    assert!(texts.iter().any(|text| text == POLICIES[4].code));
    // Side B is left alone.
    assert!(description(&window, SPECIMEN_B_NAME).ends_with("drawn with Perceptual coverage."));

    let after = specimen.capture_screenshot()?;
    assert_ne!(before, after, "the specimen is drawn with the new policy");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn magnify_shows_both_specimens_pixel_for_pixel() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    scroll_to(&window, COMPARE_SECTION_NAME)?;
    let idle = description(&window, MAGNIFIER_NAME);
    assert!(idle.starts_with(MAGNIFY_LABEL), "{idle}");

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(MAGNIFY_LABEL)
        .click()?;
    window.run_until_idle()?;
    window.pump_frames(2)?;

    let status = description(&window, MAGNIFIER_NAME);
    assert!(status.starts_with("Captured 2 samples."), "{status}");
    assert!(status.contains("A: Linear coverage ("), "{status}");
    assert!(status.contains("B: Perceptual coverage ("), "{status}");
    Ok(())
}

#[test]
fn text_rendering_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        TEXT_RENDERING_VIEW_TITLE,
        Size::new(900.0, 700.0),
        build_text_rendering_surface_with_theme,
    )
}
