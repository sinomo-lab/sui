use std::path::PathBuf;

use sui::{
    Event, KeyState, KeyboardEvent, Modifiers, Point, PointerButton, PointerButtons, PointerEvent,
    PointerEventKind, Rect, Result, SemanticsNode, SemanticsRole, SemanticsValue, Vector,
    WindowEvent,
};
use sui_testing::prelude::*;

use super::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_drag_drop_application)
        .vsync(false)
        .launch()
}

fn nodes(window: &TestWindow) -> Vec<SemanticsNode> {
    window.snapshot().expect("snapshot").accessibility.nodes
}

fn node(window: &TestWindow, role: SemanticsRole, name: &str) -> SemanticsNode {
    nodes(window)
        .into_iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("{name} is on the page"))
}

fn text(window: &TestWindow, role: SemanticsRole, name: &str) -> String {
    match node(window, role, name).value {
        Some(SemanticsValue::Text(text)) => text,
        other => panic!("{name} has no text value: {other:?}"),
    }
}

fn count(window: &TestWindow, column: Column) -> String {
    text(
        window,
        SemanticsRole::Text,
        &format!("{} count", column.name()),
    )
}

fn log(window: &TestWindow) -> String {
    text(window, SemanticsRole::Text, EVENT_LOG_NAME)
}

fn card(window: &TestWindow, title: &str) -> Rect {
    node(window, SemanticsRole::ListItem, title).bounds
}

fn center(rect: Rect) -> Point {
    Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    )
}

/// Scroll the page so `rect`'s bottom is in view, and say how far it moved.
fn scroll_into_view(window: &TestWindow, rect: Rect) -> Result<f32> {
    let page = node(
        window,
        SemanticsRole::ScrollView,
        DRAG_DROP_DEMO_SCROLL_NAME,
    )
    .bounds;
    let overflow = rect.max_y() - (page.max_y() - 24.0);
    if overflow <= 0.0 {
        return Ok(0.0);
    }
    window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(DRAG_DROP_DEMO_SCROLL_NAME)
        .scroll_pixels(Vector::new(0.0, -overflow))?;
    window.run_until_idle()?;
    Ok(overflow)
}

fn pointer(kind: PointerEventKind, at: Point, pressed: bool, modifiers: Modifiers) -> Event {
    let mut event = PointerEvent::new(kind, at);
    event.button = Some(PointerButton::Primary);
    if pressed {
        event.buttons = PointerButtons::new(1);
    }
    event.modifiers = modifiers;
    Event::Pointer(event)
}

/// Press at `from` and move to `to` in steps, holding `modifiers`.
fn drag_to(window: &TestWindow, from: Point, to: Point, modifiers: Modifiers) -> Result<()> {
    window.dispatch_event_now(pointer(PointerEventKind::Down, from, true, modifiers))?;
    for step in 1..=8 {
        let t = step as f32 / 8.0;
        let at = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        window.dispatch_event_now(pointer(PointerEventKind::Move, at, true, modifiers))?;
    }
    window.run_until_idle()
}

fn release(window: &TestWindow, at: Point, modifiers: Modifiers) -> Result<()> {
    window.dispatch_event_now(pointer(PointerEventKind::Up, at, false, modifiers))?;
    window.run_until_idle()
}

/// The top of a card's slot, where a card dropped on it lands.
fn above(rect: Rect) -> Point {
    Point::new(rect.x() + rect.width() * 0.5, rect.y() + 4.0)
}

fn copy_modifiers() -> (&'static str, Modifiers) {
    let mut modifiers = Modifiers::NONE;
    if cfg!(target_os = "macos") {
        modifiers.alt = true;
        ("Alt", modifiers)
    } else {
        modifiers.control = true;
        ("Control", modifiers)
    }
}

fn key(name: &str, modifiers: Modifiers) -> Event {
    let mut event = KeyboardEvent::new(name, KeyState::Pressed);
    event.modifiers = modifiers;
    Event::Keyboard(event)
}

#[test]
fn dropping_a_card_on_another_moves_it_before_that_card() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(count(&window, Column::Todo), "2");
    assert_eq!(count(&window, Column::Doing), "2");

    let video = card(&window, "Record the demo video");
    let review = card(&window, "Review keyboard moves");
    drag_to(&window, center(video), above(review), Modifiers::NONE)?;
    let hovering = log(&window);
    assert!(
        hovering.contains("start   “Record the demo video” · move, copy"),
        "{hovering}"
    );
    assert!(
        hovering.contains("over    before “Review keyboard moves” · move"),
        "{hovering}"
    );
    release(&window, above(review), Modifiers::NONE)?;

    assert_eq!(count(&window, Column::Todo), "1");
    assert_eq!(count(&window, Column::Doing), "3");
    let dropped = log(&window);
    assert!(
        dropped.contains("drop    “Record the demo video” before “Review keyboard moves” · move"),
        "{dropped}"
    );
    // The card sits between the other two once they finish moving.
    app.advance_time(1.0)?;
    let order = |title| card(&window, title).y();
    assert!(order("Polish the drag preview") < order("Record the demo video"));
    assert!(order("Record the demo video") < order("Review keyboard moves"));
    Ok(())
}

#[test]
fn holding_the_copy_key_copies_the_card() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let (copy_key, modifiers) = copy_modifiers();

    let video = card(&window, "Record the demo video");
    let review = card(&window, "Review keyboard moves");
    drag_to(&window, center(video), above(review), Modifiers::NONE)?;
    window.dispatch_event_now(key(copy_key, modifiers))?;
    window.run_until_idle()?;
    let hovering = log(&window);
    assert!(
        hovering.contains("over    before “Review keyboard moves” · copy"),
        "{hovering}"
    );
    release(&window, above(review), modifiers)?;

    assert_eq!(count(&window, Column::Todo), "2", "the original stays");
    assert_eq!(count(&window, Column::Doing), "3");
    node(
        &window,
        SemanticsRole::ListItem,
        "Record the demo video (copy)",
    );
    Ok(())
}

#[test]
fn escape_cancels_a_drag() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let video = card(&window, "Record the demo video");
    let review = card(&window, "Review keyboard moves");
    drag_to(&window, center(video), above(review), Modifiers::NONE)?;
    window.dispatch_event_now(key("Escape", Modifiers::NONE))?;
    window.run_until_idle()?;
    release(&window, above(review), Modifiers::NONE)?;

    assert!(log(&window).contains("cancel  Esc"), "{}", log(&window));
    assert_eq!(count(&window, Column::Todo), "2", "nothing moved");
    Ok(())
}

#[test]
fn assets_attach_and_snippets_join_the_note() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let video = card(&window, "Record the demo video");
    let sunset = node(
        &window,
        SemanticsRole::GenericContainer,
        "Shelf item Sunset",
    )
    .bounds;
    drag_to(&window, center(sunset), center(video), Modifiers::NONE)?;
    assert!(
        log(&window).contains("over    attach to “Record the demo video” · link"),
        "{}",
        log(&window)
    );
    release(&window, center(video), Modifiers::NONE)?;
    assert!(
        log(&window).contains("drop    Sunset on “Record the demo video” · link"),
        "{}",
        log(&window)
    );

    let video = card(&window, "Record the demo video");
    let snippet = node(
        &window,
        SemanticsRole::GenericContainer,
        "Shelf item Ship it Friday",
    )
    .bounds;
    let distance = scroll_into_view(&window, snippet)?;
    let snippet = snippet.translate(Vector::new(0.0, -distance));
    let video = video.translate(Vector::new(0.0, -distance));
    drag_to(&window, center(snippet), center(video), Modifiers::NONE)?;
    release(&window, center(video), Modifiers::NONE)?;
    assert_eq!(
        node(&window, SemanticsRole::ListItem, "Record the demo video").value,
        Some(SemanticsValue::Text("Ship it Friday".to_string()))
    );
    Ok(())
}

#[test]
fn the_text_field_refuses_what_is_not_text() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let field = node(&window, SemanticsRole::GenericContainer, TEXT_FIELD_NAME).bounds;
    let distance = scroll_into_view(&window, field)?;
    let field = field.translate(Vector::new(0.0, -distance));
    let hero = node(
        &window,
        SemanticsRole::GenericContainer,
        "Shelf item Hero.png",
    )
    .bounds;
    drag_to(&window, center(hero), center(field), Modifiers::NONE)?;

    assert_eq!(
        text(&window, SemanticsRole::GenericContainer, TEXT_FIELD_NAME),
        "Refusing"
    );
    assert_eq!(
        text(
            &window,
            SemanticsRole::Text,
            &format!("{TEXT_FIELD_NAME} status")
        ),
        "Only text can go here"
    );
    release(&window, center(field), Modifiers::NONE)?;
    let log = log(&window);
    assert!(log.contains("refused the text field"), "{log}");
    assert!(log.contains("cancel  "), "{log}");
    Ok(())
}

#[test]
fn alt_arrows_move_a_focused_card_and_it_keeps_focus() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let video = card(&window, "Record the demo video");
    window.dispatch_event_now(pointer(
        PointerEventKind::Down,
        center(video),
        true,
        Modifiers::NONE,
    ))?;
    release(&window, center(video), Modifiers::NONE)?;
    assert!(
        node(&window, SemanticsRole::ListItem, "Record the demo video")
            .state
            .focused
    );

    let mut alt = Modifiers::NONE;
    alt.alt = true;
    window.dispatch_event_now(key("ArrowRight", alt))?;
    window.run_until_idle()?;
    assert_eq!(count(&window, Column::Doing), "3");
    assert!(log(&window).contains("key     “Record the demo video” to Doing, 2 of 3"));
    assert!(
        node(&window, SemanticsRole::ListItem, "Record the demo video")
            .state
            .focused,
        "focus follows the card into its new column"
    );

    window.dispatch_event_now(key("ArrowUp", alt))?;
    window.run_until_idle()?;
    assert!(log(&window).contains("key     “Record the demo video” to Doing, 1 of 3"));
    Ok(())
}

#[test]
fn the_trash_deletes_and_undo_restores() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let trash = node(&window, SemanticsRole::GenericContainer, TRASH_NAME).bounds;
    let sketch = card(&window, "Sketch the board");
    let distance = scroll_into_view(&window, trash)?;
    let trash = trash.translate(Vector::new(0.0, -distance));
    let sketch = sketch.translate(Vector::new(0.0, -distance));
    drag_to(&window, center(sketch), center(trash), Modifiers::NONE)?;
    release(&window, center(trash), Modifiers::NONE)?;
    assert_eq!(count(&window, Column::Done), "0");
    assert!(log(&window).contains("drop    “Sketch the board” in the trash · move"));

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(UNDO_LABEL)
        .click()?;
    assert_eq!(count(&window, Column::Done), "1");
    assert!(log(&window).contains("undo    “Sketch the board” is back"));
    Ok(())
}

/// The page's semantics after `event`, from a runtime driven directly: the
/// live harness cannot send files from the desktop.
fn semantics_after(runtime: &mut sui::Runtime, event: Event) -> Result<Vec<SemanticsNode>> {
    let window_id = runtime.window_ids()[0];
    runtime.handle_event(window_id, event)?;
    let _ = runtime.render(window_id)?;
    Ok(runtime.render(window_id)?.semantics)
}

fn value_of(nodes: &[SemanticsNode], name: &str) -> Option<String> {
    nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(name))
        .and_then(|node| match &node.value {
            Some(SemanticsValue::Text(text)) => Some(text.clone()),
            _ => None,
        })
}

#[test]
fn files_dropped_from_the_desktop_become_cards() -> Result<()> {
    let mut runtime = build_drag_drop_application().build()?;
    let window_id = runtime.window_ids()[0];
    let _ = runtime.render(window_id)?;
    let path = PathBuf::from("notes").join("report.pdf");

    let hovering = semantics_after(
        &mut runtime,
        Event::Window(WindowEvent::ExternalFileHovered(path.clone())),
    )?;
    assert_eq!(
        value_of(&hovering, FILES_NAME).as_deref(),
        Some("Accepting")
    );

    let dropped = semantics_after(
        &mut runtime,
        Event::Window(WindowEvent::ExternalFileDropped(path)),
    )?;
    assert_eq!(value_of(&dropped, "To do count").as_deref(), Some("3"));
    assert!(
        dropped
            .iter()
            .any(|node| node.role == SemanticsRole::ListItem
                && node.name.as_deref() == Some("report.pdf"))
    );
    assert!(
        value_of(&dropped, EVENT_LOG_NAME)
            .is_some_and(|log| log.contains("file    dropped report.pdf · new card in To do"))
    );
    assert_eq!(value_of(&dropped, FILES_NAME).as_deref(), Some("Idle"));
    Ok(())
}

#[test]
fn the_locked_palette_only_lands_in_its_own_well() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let well = node(&window, SemanticsRole::GenericContainer, LOCKED_WELL_NAME).bounds;
    let distance = scroll_into_view(&window, well)?;
    let well = well.translate(Vector::new(0.0, -distance));
    let swatch = node(&window, SemanticsRole::GenericContainer, LOCKED_SWATCH_NAME).bounds;
    let field = node(&window, SemanticsRole::GenericContainer, TEXT_FIELD_NAME).bounds;

    // The text field is in the board's scope, so it neither accepts nor
    // refuses the palette.
    drag_to(&window, center(swatch), center(field), Modifiers::NONE)?;
    assert_eq!(
        text(&window, SemanticsRole::GenericContainer, TEXT_FIELD_NAME),
        "Idle"
    );
    release(&window, center(field), Modifiers::NONE)?;

    drag_to(&window, center(swatch), center(well), Modifiers::NONE)?;
    release(&window, center(well), Modifiers::NONE)?;
    assert!(
        log(&window).contains("drop    brand palette in its well · copy"),
        "{}",
        log(&window)
    );
    Ok(())
}
