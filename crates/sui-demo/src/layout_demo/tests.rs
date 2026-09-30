use sui::{
    Event, PointerButtons, PointerEvent, PointerEventKind, Result, SemanticsNode, SemanticsRole,
    SemanticsValue,
};
use sui_testing::prelude::*;

use super::flex::FlexSettings;
use super::frame::{EXAMPLE_FRAME_NAME, HANDLE_WIDTH};
use super::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_layout_application)
        .vsync(false)
        .launch()
}

fn nodes(window: &TestWindow, role: SemanticsRole, name: &str) -> Vec<SemanticsNode> {
    window
        .snapshot()
        .expect("snapshot")
        .accessibility
        .nodes
        .into_iter()
        .filter(|node| node.role == role && node.name.as_deref() == Some(name))
        .collect()
}

fn text(window: &TestWindow, role: SemanticsRole, name: &str) -> String {
    let found = nodes(window, role, name);
    let node = found
        .first()
        .unwrap_or_else(|| panic!("{name} is on the page"));
    match &node.value {
        Some(SemanticsValue::Text(text)) => text.clone(),
        other => panic!("{name} has no text value: {other:?}"),
    }
}

fn readout(window: &TestWindow) -> String {
    text(window, SemanticsRole::Text, FRAME_READOUT_NAME)
}

fn choose_preset(window: &TestWindow, preset: &str) -> Result<()> {
    window
        .get_by_role(SemanticsRole::RadioButton)
        .with_name(preset)
        .click()?;
    window.run_until_idle()
}

/// Focus `name`, which clicks it unless it has focus, and press `key`.
fn press(window: &TestWindow, role: SemanticsRole, name: &str, key: &str) -> Result<()> {
    window.get_by_role(role).with_name(name).press(key)?;
    window.run_until_idle()
}

/// Scroll the page until `name` is in view.
fn reveal(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
    for _ in 0..24 {
        let page = nodes(window, SemanticsRole::ScrollView, LAYOUT_DEMO_SCROLL_NAME)
            .pop()
            .expect("the page scrolls");
        let visible = nodes(window, role.clone(), name)
            .first()
            .is_some_and(|node| {
                node.bounds.y() >= page.bounds.y() && node.bounds.max_y() <= page.bounds.max_y()
            });
        if visible {
            return Ok(());
        }
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(LAYOUT_DEMO_SCROLL_NAME)
            .scroll_pixels(sui::Vector::new(0.0, -240.0))?;
        window.run_until_idle()?;
    }
    panic!("{name} never came into view");
}

#[test]
fn presets_drive_the_responsive_examples() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    // Tablet width: the sidebar docks, the query is wide, and master and
    // detail sit side by side with no way back to need.
    assert_eq!(readout(&window), "768 px");
    assert_eq!(
        text(&window, SemanticsRole::Text, SIDEBAR_MODE_NAME),
        "Inline"
    );
    assert_eq!(
        text(&window, SemanticsRole::Text, QUERY_RULE_NAME),
        "min_width(680): side by side"
    );
    assert!(nodes(&window, SemanticsRole::Button, BACK_LABEL).is_empty());

    choose_preset(&window, "Phone")?;
    assert_eq!(readout(&window), "360 px");
    assert_eq!(
        text(&window, SemanticsRole::Text, SIDEBAR_MODE_NAME),
        "Overlay, closed"
    );
    assert_eq!(
        text(&window, SemanticsRole::Text, QUERY_RULE_NAME),
        "Default rule: stacked, below 680 px"
    );
    reveal(&window, SemanticsRole::Button, OPEN_DETAIL_LABEL)?;
    window
        .get_by_role(SemanticsRole::Button)
        .with_name(OPEN_DETAIL_LABEL)
        .click()?;
    window.run_until_idle()?;
    assert_eq!(
        nodes(&window, SemanticsRole::Button, BACK_LABEL).len(),
        1,
        "one pane at a time needs a way back"
    );
    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(LAYOUT_DEMO_SCROLL_NAME)
        .scroll_pixels(sui::Vector::new(0.0, 10_000.0))?;
    window.run_until_idle()?;

    choose_preset(&window, "Desktop")?;
    assert_eq!(readout(&window), "1180 px");
    assert_eq!(
        text(&window, SemanticsRole::Text, SIDEBAR_MODE_NAME),
        "Inline"
    );

    // Wider than the page: the frames fit and say so.
    press(&window, SemanticsRole::Slider, FRAME_WIDTH_NAME, "End")?;
    let widest = readout(&window);
    assert!(widest.starts_with("1280 px, "), "{widest}");
    assert!(widest.ends_with(" px fit"), "{widest}");
    Ok(())
}

#[test]
fn flex_controls_and_tiles_rewrite_the_code() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::Text, FLEX_CODE_NAME),
        flex::flex_code(&FlexSettings::default())
    );

    window
        .get_by_role(SemanticsRole::Button)
        .with_name("Tile A")
        .click()?;
    window.run_until_idle()?;
    assert_eq!(text(&window, SemanticsRole::Button, "Tile A"), "grows");
    let code = text(&window, SemanticsRole::Text, FLEX_CODE_NAME);
    assert!(
        code.contains("tile(\"A\"), FlexItem::new().grow(1.0).basis(110.0))"),
        "{code}"
    );
    // The press focused it, so the keyboard carries on.
    press(&window, SemanticsRole::Button, "Tile A", "Enter")?;
    assert_eq!(text(&window, SemanticsRole::Button, "Tile A"), "capped");
    assert!(
        nodes(&window, SemanticsRole::Button, "Tile A")[0]
            .state
            .focused,
        "the tile keeps focus while the flex restyles"
    );

    window
        .get_by_role(SemanticsRole::RadioButton)
        .with_name("Column")
        .click()?;
    window.run_until_idle()?;
    let code = text(&window, SemanticsRole::Text, FLEX_CODE_NAME);
    assert!(code.starts_with("Flex::vertical()"), "{code}");
    assert!(code.contains(".basis(36.0)"), "{code}");
    Ok(())
}

#[test]
fn the_grid_ruler_reads_resolved_track_widths() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::GenericContainer, "Fixed 160"),
        "Fixed 160 · 160 px"
    );
    let fraction = text(&window, SemanticsRole::GenericContainer, "1fr");
    assert!(fraction.starts_with("1fr · "), "{fraction}");
    assert!(
        text(&window, SemanticsRole::Text, GRID_CODE_NAME).contains("GridTrack::Fraction(1.0)")
    );
    Ok(())
}

#[test]
fn dragging_a_frame_edge_changes_the_width() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let frame = nodes(&window, SemanticsRole::GenericContainer, EXAMPLE_FRAME_NAME)
        .into_iter()
        .min_by(|a, b| a.bounds.y().total_cmp(&b.bounds.y()))
        .expect("a frame");
    let start = sui::Point::new(
        frame.bounds.max_x() + HANDLE_WIDTH * 0.5,
        frame.bounds.y() + frame.bounds.height() * 0.5,
    );
    let pointer = |kind, x: f32, pressed: bool| {
        let mut event = PointerEvent::new(kind, sui::Point::new(x, start.y));
        event.button = Some(sui::PointerButton::Primary);
        if pressed {
            event.buttons = PointerButtons::new(1);
        }
        Event::Pointer(event)
    };
    window.dispatch_event_now(pointer(PointerEventKind::Down, start.x, true))?;
    window.dispatch_event_now(pointer(PointerEventKind::Move, start.x - 60.0, true))?;
    window.dispatch_event_now(pointer(PointerEventKind::Move, start.x - 100.0, true))?;
    window.dispatch_event_now(pointer(PointerEventKind::Up, start.x - 100.0, false))?;
    window.run_until_idle()?;
    assert_eq!(readout(&window), "668 px");
    Ok(())
}

#[test]
fn the_keyboard_raises_the_safe_area() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::Text, INSETS_NAME),
        "Top 44 · bottom 34 · left 0 · right 0"
    );
    reveal(&window, SemanticsRole::Switch, KEYBOARD_LABEL)?;
    window
        .get_by_role(SemanticsRole::Switch)
        .with_name(KEYBOARD_LABEL)
        .click()?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::Text, INSETS_NAME),
        "Top 44 · bottom 250 · left 0 · right 0"
    );
    assert_eq!(
        text(&window, SemanticsRole::GenericContainer, "Phone preview"),
        "Top 44 · bottom 250 · left 0 · right 0"
    );
    Ok(())
}
