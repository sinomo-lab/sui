use sui::{Result, SemanticsRole, Size};
use sui_testing::prelude::*;

use super::*;
use crate::test_support::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_text_shaping_application)
        .vsync(false)
        .launch()
}

fn node(window: &TestWindow, name: &str) -> sui::SemanticsNode {
    window
        .snapshot()
        .expect("snapshot")
        .accessibility
        .nodes
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("{name} is on the page"))
}

#[test]
fn every_script_checks_itself_and_the_summary_counts_them() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let mut passed = 0;
    for sample in probes::SCRIPTS {
        let row = node(&window, &format!("{} probe", sample.script));
        let description = row.description.unwrap_or_default();
        assert!(
            description.starts_with("Passed.") || description.starts_with("Failed."),
            "{}: {description}",
            sample.script
        );
        if description.starts_with("Passed.") {
            passed += 1;
        }
    }
    // A bundled font covers Latin, so it passes everywhere.
    let latin = node(&window, "Latin probe").description.unwrap_or_default();
    assert!(latin.starts_with("Passed."), "{latin}");
    assert!(latin.contains("Drawn with "), "{latin}");

    let summary = match node(&window, SCRIPTS_SUMMARY_NAME).value {
        Some(sui::SemanticsValue::Text(text)) => text,
        other => panic!("the summary has text: {other:?}"),
    };
    let total = probes::SCRIPTS.len();
    if passed == total {
        assert_eq!(
            summary,
            format!("All {total} scripts shape completely on this system.")
        );
    } else {
        assert!(
            summary.starts_with(&format!("{passed} of {total} scripts")),
            "{summary}"
        );
    }
    Ok(())
}

#[test]
fn mixed_directions_describe_their_runs() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let description = node(&window, BIDI_PROBE_NAME)
        .description
        .unwrap_or_default();
    assert!(
        description.starts_with("Left-to-right paragraph: "),
        "{description}"
    );
    assert!(
        description.contains(" Right-to-left paragraph: "),
        "{description}"
    );
    // Hebrew and Arabic make right-to-left runs in both paragraphs.
    assert!(!description.contains(" 0 right to left"), "{description}");

    let metrics = node(&window, METRICS_PROBE_NAME)
        .description
        .unwrap_or_default();
    assert!(metrics.contains("ascent"), "{metrics}");
    let cap_height = metrics
        .split_once(", cap height ")
        .expect("cap height is described")
        .1
        .trim_end_matches('.');
    // Cap height is optional font metadata; system fonts may omit it.
    assert!(
        cap_height == "unknown"
            || cap_height
                .parse::<f32>()
                .is_ok_and(|height| height.is_finite() && height > 0.0),
        "{metrics}"
    );
    Ok(())
}

#[test]
fn line_breaking_follows_the_width() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let lines = |window: &TestWindow| -> (usize, String) {
        let description = node(window, BREAKING_PROBE_NAME)
            .description
            .unwrap_or_default();
        let count = description
            .split(' ')
            .next()
            .and_then(|count| count.parse().ok())
            .unwrap_or_else(|| panic!("{description}"));
        (count, description)
    };
    // The section is below the fold.
    let page = node(&window, TEXT_SHAPING_SCROLL_NAME).bounds;
    let section = node(&window, BREAKING_SECTION_NAME).bounds;
    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(TEXT_SHAPING_SCROLL_NAME)
        .scroll_pixels(sui::Vector::new(0.0, page.y() - section.y()))?;
    window.run_until_idle()?;
    let (wide, description) = lines(&window);
    assert!(description.ends_with("at 320 px wide."), "{description}");

    let slider = window
        .get_by_role(SemanticsRole::Slider)
        .with_name(BREAKING_WIDTH_NAME);
    slider.press("Home")?;
    window.run_until_idle()?;
    let (narrow, description) = lines(&window);
    assert!(description.ends_with("at 140 px wide."), "{description}");
    assert!(narrow > wide, "{narrow} lines at 140 px, {wide} at 320 px");

    let probe = node(&window, BREAKING_PROBE_NAME);
    assert!(probe.bounds.height() > 0.0);
    Ok(())
}

#[test]
fn text_shaping_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        TEXT_SHAPING_VIEW_TITLE,
        Size::new(900.0, 700.0),
        build_text_shaping_surface_with_theme,
    )
}
