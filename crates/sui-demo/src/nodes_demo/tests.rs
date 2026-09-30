use sui::{
    Event, KeyState, KeyboardEvent, PointerButtons, PointerEvent, PointerEventKind, Result,
    SemanticsNode, SemanticsRole, SemanticsValue,
};
use sui_testing::prelude::*;

use super::inspector::{INPUTS_NAME, NODE_NAME, SELECTED_EDGE_STYLE};
use super::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_nodes_application)
        .vsync(false)
        .launch()
}

fn all(window: &TestWindow) -> Vec<SemanticsNode> {
    window.snapshot().expect("snapshot").accessibility.nodes
}

fn nodes(window: &TestWindow, role: SemanticsRole, name: &str) -> Vec<SemanticsNode> {
    all(window)
        .into_iter()
        .filter(|node| node.role == role && node.name.as_deref() == Some(name))
        .collect()
}

fn text(window: &TestWindow, role: SemanticsRole, name: &str) -> String {
    let found = nodes(window, role, name);
    match &found
        .first()
        .unwrap_or_else(|| panic!("{name} is on the page"))
        .value
    {
        Some(SemanticsValue::Text(text)) => text.clone(),
        other => panic!("{name} has no text value: {other:?}"),
    }
}

fn log(window: &TestWindow) -> String {
    text(window, SemanticsRole::Text, NODES_LOG_NAME)
}

/// A graph node by its label.
fn graph_node(window: &TestWindow, label: &str) -> SemanticsNode {
    let graph = nodes(window, SemanticsRole::Canvas, NODES_MAIN_GRAPH_NAME)
        .pop()
        .expect("the graph");
    all(window)
        .into_iter()
        .find(|node| {
            node.parent == Some(graph.id)
                && node.role == SemanticsRole::GenericContainer
                && node.name.as_deref() == Some(label)
                && node
                    .description
                    .as_deref()
                    .is_some_and(|description| description.contains("node"))
        })
        .unwrap_or_else(|| panic!("{label} is in the graph"))
}

fn has_node(window: &TestWindow, label: &str) -> bool {
    let graph = nodes(window, SemanticsRole::Canvas, NODES_MAIN_GRAPH_NAME)
        .pop()
        .expect("the graph");
    all(window).into_iter().any(|node| {
        node.parent == Some(graph.id)
            && node.name.as_deref() == Some(label)
            && node
                .description
                .as_deref()
                .is_some_and(|description| description.contains("node"))
    })
}

/// Where the node labeled `label`'s input or output named `port` is.
fn handle(window: &TestWindow, label: &str, port: &str, output: bool) -> Point {
    let owner = graph_node(window, label).id;
    let direction = if output { "Output" } else { "Input" };
    let handle = all(window)
        .into_iter()
        .find(|node| {
            node.parent == Some(owner)
                && node.name.as_deref() == Some(port)
                && node
                    .description
                    .as_deref()
                    .is_some_and(|description| description.starts_with(direction))
        })
        .unwrap_or_else(|| panic!("{label} has an {direction} {port}"));
    center(handle.bounds)
}

fn output(window: &TestWindow, label: &str, port: &str) -> Point {
    handle(window, label, port, true)
}

fn input(window: &TestWindow, label: &str, port: &str) -> Point {
    handle(window, label, port, false)
}

fn center(bounds: sui::Rect) -> Point {
    Point::new(
        bounds.x() + bounds.width() * 0.5,
        bounds.y() + bounds.height() * 0.5,
    )
}

/// A point on the node's title row, clear of its controls.
fn title_of(window: &TestWindow, label: &str) -> Point {
    let bounds = graph_node(window, label).bounds;
    Point::new(bounds.x() + bounds.width() * 0.5, bounds.y() + 12.0)
}

fn pointer(kind: PointerEventKind, position: Point, button: PointerButton, pressed: bool) -> Event {
    let mut pointer = PointerEvent::new(kind, position);
    pointer.pointer_id = 41;
    pointer.button = Some(button);
    pointer.buttons = if pressed {
        let mut buttons = PointerButtons::NONE;
        buttons.insert(button);
        buttons
    } else {
        PointerButtons::NONE
    };
    Event::Pointer(pointer)
}

fn click_at(window: &TestWindow, at: Point, button: PointerButton) -> Result<()> {
    // Far enough apart in time not to be a double-click.
    window.advance_time(1.0)?;
    window.dispatch_event_now(pointer(PointerEventKind::Down, at, button, true))?;
    window.dispatch_event_now(pointer(PointerEventKind::Up, at, button, false))?;
    window.run_until_idle()
}

fn drag(window: &TestWindow, from: Point, to: Point) -> Result<()> {
    window.advance_time(1.0)?;
    window.dispatch_event_now(pointer(
        PointerEventKind::Down,
        from,
        PointerButton::Primary,
        true,
    ))?;
    let middle = Point::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5);
    for at in [middle, to] {
        window.dispatch_event_now(pointer(
            PointerEventKind::Move,
            at,
            PointerButton::Primary,
            true,
        ))?;
    }
    window.dispatch_event_now(pointer(
        PointerEventKind::Up,
        to,
        PointerButton::Primary,
        false,
    ))?;
    window.run_until_idle()
}

fn key(window: &TestWindow, name: &str, control: bool, shift: bool) -> Result<()> {
    let mut key = KeyboardEvent::new(name, KeyState::Pressed);
    key.modifiers.control = control;
    key.modifiers.shift = shift;
    window.dispatch_event_now(Event::Keyboard(key))?;
    window.run_until_idle()
}

fn press(window: &TestWindow, label: &str) -> Result<()> {
    window
        .get_by_role(SemanticsRole::Button)
        .with_name(label)
        .click()?;
    window.run_until_idle()
}

#[test]
fn values_follow_a_slider_through_the_graph() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    click_at(&window, title_of(&window, "Mix"), PointerButton::Primary)?;
    assert!(text(&window, SemanticsRole::Text, INPUTS_NAME).contains("← Blend (0.35)"));

    window
        .get_by_role(SemanticsRole::Slider)
        .with_name("Blend value")
        .press("ArrowRight")?;
    window.run_until_idle()?;
    let Some(SemanticsValue::Range { value, .. }) =
        nodes(&window, SemanticsRole::Slider, "Blend value")[0].value
    else {
        panic!("the slider has a value");
    };
    assert!((value - 0.35).abs() > 0.001, "the slider moved");
    click_at(&window, title_of(&window, "Mix"), PointerButton::Primary)?;
    let inputs = text(&window, SemanticsRole::Text, INPUTS_NAME);
    assert!(
        inputs.contains(&format!("← Blend ({value:.2})")),
        "{inputs}"
    );
    Ok(())
}

#[test]
fn a_connection_that_does_not_fit_says_why() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let edges = graph_edges(&window);

    drag(
        &window,
        output(&window, "Blend", "value"),
        input(&window, "Preview", "color"),
    )?;
    assert_eq!(graph_edges(&window), edges);
    let log = log(&window);
    assert!(
        log.contains(
            "Refused Blend › Preview color: Blend gives a number, but color takes a color"
        ),
        "{log}"
    );

    drag(
        &window,
        output(&window, "Lighten", "color"),
        input(&window, "Mix", "a"),
    )?;
    assert!(log_last(&window).contains("That would loop: Mix already feeds Lighten"));
    Ok(())
}

fn graph_edges(window: &TestWindow) -> usize {
    all(window)
        .into_iter()
        .filter(|node| {
            node.description
                .as_deref()
                .is_some_and(|description| description.ends_with(" edge"))
        })
        .count()
}

fn log_last(window: &TestWindow) -> String {
    log(window)
        .trim_end()
        .lines()
        .last()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn a_new_node_takes_a_value_when_connected() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    window
        .get_by_role(SemanticsRole::Button)
        .with_name(NODES_ADD_NODE_BUTTON)
        .click()?;
    window.run_until_idle()?;
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Preview")
        .click()?;
    window.run_until_idle()?;
    assert!(log_last(&window).starts_with("Added Preview"));
    let label = log_last(&window).trim_start_matches("Added ").to_string();

    let accent = nodes(&window, SemanticsRole::Text, "#B872CD").len();
    drag(
        &window,
        output(&window, "Accent", "color"),
        input(&window, &label, "color"),
    )?;
    assert_eq!(
        log_last(&window),
        format!("Connected Accent › {label} color")
    );
    assert!(
        nodes(&window, SemanticsRole::Text, "#B872CD").len() > accent,
        "the new preview shows Accent's color"
    );
    Ok(())
}

#[test]
fn undo_and_redo_from_the_toolbar_and_the_keyboard() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    click_at(&window, title_of(&window, "Blend"), PointerButton::Primary)?;
    key(&window, "Delete", false, false)?;
    assert!(!has_node(&window, "Blend"));

    press(&window, UNDO_LABEL)?;
    assert!(has_node(&window, "Blend"));
    press(&window, REDO_LABEL)?;
    assert!(!has_node(&window, "Blend"));
    key(&window, "z", true, false)?;
    assert!(
        has_node(&window, "Blend"),
        "the graph kept focus for Ctrl+Z"
    );
    assert_eq!(log_last(&window), "Undo");
    Ok(())
}

#[test]
fn copy_paste_and_duplicate_add_copies() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let count = || nodes(&window, SemanticsRole::GenericContainer, "Mix").len();
    let before = count();
    click_at(&window, title_of(&window, "Mix"), PointerButton::Primary)?;
    press(&window, DUPLICATE_LABEL)?;
    assert_eq!(count(), before + 1);
    assert_eq!(log_last(&window), "Duplicated 1 node");

    key(&window, "c", true, false)?;
    key(&window, "v", true, false)?;
    assert_eq!(count(), before + 2);
    assert_eq!(log_last(&window), "Pasted 1 node");
    Ok(())
}

#[test]
fn a_right_click_opens_a_menu_for_what_it_was_on() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    click_at(&window, title_of(&window, "Lift"), PointerButton::Secondary)?;
    assert_eq!(log_last(&window), "Menu for Lift");
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Disconnect")
        .expect()
        .to_be_visible()?;
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name(DELETE_LABEL)
        .click()?;
    window.run_until_idle()?;
    assert!(!has_node(&window, "Lift"));
    key(&window, "z", true, false)?;
    assert!(
        has_node(&window, "Lift"),
        "the graph has focus back after the menu"
    );

    // On the canvas, the menu adds a node where it opened.
    let graph = nodes(&window, SemanticsRole::Canvas, NODES_MAIN_GRAPH_NAME)[0].bounds;
    let empty = Point::new(graph.x() + 40.0, graph.max_y() - 30.0);
    click_at(&window, empty, PointerButton::Secondary)?;
    assert_eq!(log_last(&window), "Menu for the canvas");
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Add")
        .click()?;
    window.run_until_idle()?;
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Number")
        .click()?;
    window.run_until_idle()?;
    let added = log_last(&window).trim_start_matches("Added ").to_string();
    let node = graph_node(&window, &added).bounds;
    assert!(
        (center(node).x - empty.x).abs() < 40.0 && (center(node).y - empty.y).abs() < 40.0,
        "{added} is where the menu opened: {node:?}"
    );
    Ok(())
}

#[test]
fn the_inspector_restyles_the_selected_edge() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let from = output(&window, "Mix", "color");
    let to = input(&window, "Preview", "color");
    // A bezier between level ends passes through their midpoint.
    click_at(
        &window,
        Point::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5),
        PointerButton::Primary,
    )?;
    let style = window
        .get_by_role(SemanticsRole::ComboBox)
        .with_name(SELECTED_EDGE_STYLE);
    style.click()?;
    window.focused().press("Escape")?;
    window.focused().press("End")?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::ComboBox, SELECTED_EDGE_STYLE),
        "Straight"
    );
    assert!(
        all(&window)
            .iter()
            .any(|node| node.description.as_deref() == Some("Straight edge"))
    );
    assert_eq!(log_last(&window), "Restyled 1 edge as straight");
    Ok(())
}

#[test]
fn a_rename_is_one_undo_step() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    click_at(&window, title_of(&window, "Mix"), PointerButton::Primary)?;
    let name = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(NODE_NAME);
    name.focus()?;
    key(&window, "a", true, false)?;
    name.fill("Blended")?;
    window.run_until_idle()?;
    assert!(
        has_node(&window, "Blended"),
        "the graph shows the name as it is typed"
    );

    press(&window, UNDO_LABEL)?;
    assert!(has_node(&window, "Mix"));
    assert!(!has_node(&window, "Blended"));
    Ok(())
}
