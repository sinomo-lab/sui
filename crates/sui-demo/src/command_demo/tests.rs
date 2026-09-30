use std::time::Duration;

use sui::{
    Event, KeyState, KeyboardEvent, Modifiers, Result, SemanticsNode, SemanticsRole, SemanticsValue,
};
use sui_testing::prelude::*;

use super::editing::{ACTS_ON_NAME, BODY_NAME, TITLE_NAME};
#[cfg(not(target_arch = "wasm32"))]
use super::export::{CANCEL_LABEL, EXPORT_LABEL, EXPORT_PROGRESS_NAME, EXPORT_STATUS_NAME};
use super::routes::{
    DELIVERY_NAME, Lane, NOTES_NAME, Origin, ROUTE_CODE_NAME, ROUTE_NAME, ROUTES, Route,
    SENDER_NAME,
};
use super::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_command_application)
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

/// What a lane on the map says it did.
fn status(window: &TestWindow, lane: Lane) -> String {
    text(window, SemanticsRole::GenericContainer, lane.name())
}

fn delivery(window: &TestWindow) -> String {
    text(window, SemanticsRole::Text, DELIVERY_NAME)
}

fn trace(window: &TestWindow) -> String {
    text(window, SemanticsRole::Text, TRACE_NAME)
}

/// Scroll the page until `name` is in view.
fn reveal(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
    for _ in 0..24 {
        let page = nodes(window, SemanticsRole::ScrollView, COMMAND_DEMO_SCROLL_NAME)
            .pop()
            .expect("the page scrolls");
        let node = nodes(window, role.clone(), name)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{name} is on the page"));
        if node.bounds.y() < page.bounds.y() {
            window
                .get_by_role(SemanticsRole::ScrollView)
                .with_name(COMMAND_DEMO_SCROLL_NAME)
                .scroll_pixels(sui::Vector::new(0.0, 240.0))?;
        } else if node.bounds.max_y() > page.bounds.max_y() {
            window
                .get_by_role(SemanticsRole::ScrollView)
                .with_name(COMMAND_DEMO_SCROLL_NAME)
                .scroll_pixels(sui::Vector::new(0.0, -240.0))?;
        } else {
            return Ok(());
        }
        window.run_until_idle()?;
    }
    panic!("{name} never came into view");
}

/// Scroll `name` into view and click it.
fn click(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
    reveal(window, role.clone(), name)?;
    window.get_by_role(role).with_name(name).click()?;
    window.run_until_idle()
}

fn choose_route(window: &TestWindow, route: Route) -> Result<()> {
    // Clicking opens the list; the keys pick from it closed.
    click(window, SemanticsRole::ComboBox, ROUTE_NAME)?;
    window.focused().press("Escape")?;
    window.focused().press("Home")?;
    let index = ROUTES
        .iter()
        .position(|candidate| *candidate == route)
        .unwrap();
    for _ in 0..index {
        window.focused().press("ArrowDown")?;
    }
    window.run_until_idle()?;
    assert_eq!(
        text(window, SemanticsRole::ComboBox, ROUTE_NAME),
        route.label()
    );
    Ok(())
}

fn send(window: &TestWindow) -> Result<()> {
    click(window, SemanticsRole::Button, SEND_LABEL)
}

fn turn_off(window: &TestWindow, lane: Lane) -> Result<()> {
    click(window, SemanticsRole::Switch, &lane.switch_name())
}

fn key(window: &TestWindow, key: &str, shift: bool) -> Result<()> {
    let mut event = KeyboardEvent::new(key, KeyState::Pressed);
    if shift {
        event.modifiers = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
    }
    window.dispatch_event_now(Event::Keyboard(event))?;
    window.run_until_idle()
}

fn focused_name(window: &TestWindow) -> Option<String> {
    window
        .snapshot()
        .expect("snapshot")
        .accessibility
        .nodes
        .into_iter()
        .find(|node| node.state.focused)
        .and_then(|node| node.name)
}

/// Run the window until `done` holds, for work arriving from other threads.
#[cfg(not(target_arch = "wasm32"))]
fn wait_until(window: &TestWindow, done: impl Fn() -> bool) -> Result<()> {
    for _ in 0..400 {
        window.run_until_idle()?;
        if done() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("gave up waiting");
}

#[test]
fn a_window_command_stops_at_the_first_listener_that_handles_it() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(
        status(&window, Lane::WindowController),
        "Waiting for a ping"
    );

    send(&window)?;
    let first = status(&window, Lane::WindowController);
    assert!(first.ends_with("handled it"), "{first}");
    assert!(
        status(&window, Lane::StatusHandler).ends_with("skipped, already handled"),
        "{}",
        status(&window, Lane::StatusHandler)
    );
    assert_eq!(status(&window, Lane::Settings), "Not on this route");
    let verdict = delivery(&window);
    assert!(
        verdict.ends_with("reached 1 listener and was handled."),
        "{verdict}"
    );

    turn_off(&window, Lane::WindowController)?;
    send(&window)?;
    assert!(
        status(&window, Lane::WindowController).ends_with("ran, passed it on"),
        "{}",
        status(&window, Lane::WindowController)
    );
    assert!(status(&window, Lane::StatusHandler).ends_with("handled it"));
    assert!(delivery(&window).ends_with("reached 2 listeners and was handled."));
    Ok(())
}

#[test]
fn broadcasts_reach_every_listener_in_their_scope() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    choose_route(&window, Route::WindowBroadcast)?;
    send(&window)?;
    assert!(status(&window, Lane::WindowController).ends_with("handled it"));
    assert!(status(&window, Lane::StatusHandler).ends_with("handled it"));

    choose_route(&window, Route::ApplicationBroadcast)?;
    send(&window)?;
    assert!(status(&window, Lane::Settings).ends_with("handled it"));
    assert!(status(&window, Lane::Analytics).ends_with("ran, passed it on"));
    assert!(status(&window, Lane::WindowController).ends_with("handled it"));
    assert!(status(&window, Lane::StatusHandler).ends_with("handled it"));
    assert_eq!(status(&window, Lane::TargetCard), "Not on this route");
    assert!(delivery(&window).ends_with("reached 4 listeners and was handled."));
    Ok(())
}

#[test]
fn an_application_command_skips_the_windows() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    choose_route(&window, Route::Application)?;
    send(&window)?;
    assert!(status(&window, Lane::Settings).ends_with("handled it"));
    assert!(status(&window, Lane::Analytics).ends_with("skipped, already handled"));
    assert_eq!(status(&window, Lane::WindowController), "Not on this route");

    turn_off(&window, Lane::Settings)?;
    send(&window)?;
    assert!(status(&window, Lane::Analytics).ends_with("ran, passed it on"));
    assert!(delivery(&window).ends_with("reached 2 listeners, and none handled it."));
    Ok(())
}

#[test]
fn a_widget_command_goes_straight_to_the_card() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    choose_route(&window, Route::Widget)?;
    send(&window)?;
    assert!(status(&window, Lane::TargetCard).ends_with("handled it"));
    assert_eq!(status(&window, Lane::Notes), "Not on this route");
    assert_eq!(status(&window, Lane::WindowController), "Not on this route");
    let trace = trace(&window);
    assert!(
        trace.contains("demo.ping → widget ") && trace.contains("widget target · handled"),
        "{trace}"
    );
    Ok(())
}

#[test]
fn a_focused_widget_command_goes_wherever_focus_is() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    choose_route(&window, Route::Focused)?;

    // Send leaves focus on the card clicked before it.
    click(
        &window,
        SemanticsRole::GenericContainer,
        Lane::TargetCard.name(),
    )?;
    send(&window)?;
    assert_eq!(
        focused_name(&window).as_deref(),
        Some(Lane::TargetCard.name())
    );
    assert!(status(&window, Lane::TargetCard).ends_with("handled it"));
    assert_eq!(status(&window, Lane::Notes), "Not on this route");

    click(&window, SemanticsRole::TextInput, NOTES_NAME)?;
    send(&window)?;
    assert!(status(&window, Lane::Notes).ends_with("got it, ignored it"));
    assert!(delivery(&window).ends_with("reached 1 listener, and none handled it."));

    // Pressing plain text focuses the page, which scrolls from the keyboard
    // but does nothing with pings.
    click(&window, SemanticsRole::Text, "Widgets")?;
    assert_eq!(
        focused_name(&window).as_deref(),
        Some(COMMAND_DEMO_SCROLL_NAME)
    );
    send(&window)?;
    assert_eq!(status(&window, Lane::TargetCard), "Not on this route");
    assert_eq!(status(&window, Lane::Notes), "Not on this route");
    assert!(delivery(&window).ends_with("reached 1 listener, and none handled it."));
    assert!(trace(&window).contains("demo.ping → focused widget · focused widget · not handled"));
    Ok(())
}

#[test]
fn a_command_to_a_closed_widget_is_not_delivered() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    choose_route(&window, Route::Closed)?;
    send(&window)?;
    let verdict = delivery(&window);
    assert!(
        verdict.ends_with("was not delivered: no widget has that id."),
        "{verdict}"
    );
    for lane in [Lane::TargetCard, Lane::WindowController, Lane::Settings] {
        assert_eq!(status(&window, lane), "Not on this route");
    }
    assert!(trace(&window).contains("· no listeners · not delivered"));
    Ok(())
}

#[test]
fn a_wake_runs_every_controller_hook_and_delivers_nothing() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    choose_route(&window, Route::Wake)?;
    send(&window)?;
    for lane in [
        Lane::Settings,
        Lane::Analytics,
        Lane::WindowController,
        Lane::StatusHandler,
    ] {
        assert_eq!(status(&window, lane), "Wake hook ran");
    }
    assert_eq!(status(&window, Lane::TargetCard), "Not on this route");
    assert_eq!(
        delivery(&window),
        "Wake: 4 controller wake hooks ran. No command was delivered."
    );
    assert!(
        trace(&window).contains(
            "scheduler wake → window, broadcast · Window controller, Status handler · woke"
        )
    );
    Ok(())
}

#[test]
fn the_code_follows_the_route_and_the_sender() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::Text, ROUTE_CODE_NAME),
        "let sender = ctx.command_sender();\nsender.send_window(window_id, PING, ping);"
    );

    choose_route(&window, Route::Application)?;
    click(&window, SemanticsRole::RadioButton, Origin::Worker.label())?;
    let code = text(&window, SemanticsRole::Text, ROUTE_CODE_NAME);
    assert!(
        code.contains("sender.send_application(PING, ping)"),
        "{code}"
    );
    assert!(code.contains("clone()"), "{code}");
    assert_eq!(
        text(&window, SemanticsRole::RadioGroup, SENDER_NAME),
        Origin::Worker.label()
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_ping_from_a_worker_thread_arrives_like_any_other() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    click(&window, SemanticsRole::RadioButton, Origin::Worker.label())?;
    send(&window)?;
    wait_until(&window, || {
        status(&window, Lane::WindowController).ends_with("handled it")
    })?;
    assert!(status(&window, Lane::StatusHandler).ends_with("skipped, already handled"));
    Ok(())
}

#[test]
fn the_toolbar_acts_on_the_last_editor_without_taking_focus() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    assert_eq!(
        text(&window, SemanticsRole::Text, ACTS_ON_NAME),
        "Acts on: nothing yet. Click into an editor."
    );

    click(&window, SemanticsRole::TextInput, BODY_NAME)?;
    assert_eq!(
        text(&window, SemanticsRole::Text, ACTS_ON_NAME),
        "Acts on: Body"
    );
    let body = text(&window, SemanticsRole::TextInput, BODY_NAME);

    for tool in ["Select all", "Cut"] {
        click(&window, SemanticsRole::Button, tool)?;
        assert_eq!(focused_name(&window).as_deref(), Some(BODY_NAME));
    }
    assert_eq!(text(&window, SemanticsRole::TextInput, BODY_NAME), "");

    click(&window, SemanticsRole::Button, "Paste")?;
    assert_eq!(text(&window, SemanticsRole::TextInput, BODY_NAME), body);
    Ok(())
}

#[test]
fn the_toolbar_works_from_the_keyboard_and_hands_focus_back() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    click(&window, SemanticsRole::TextInput, TITLE_NAME)?;

    // Back out of the editor to the toolbar's last button.
    key(&window, "Tab", true)?;
    assert_eq!(focused_name(&window).as_deref(), Some("Select all"));
    assert_eq!(
        text(&window, SemanticsRole::Text, ACTS_ON_NAME),
        "Acts on: Title"
    );

    key(&window, "Enter", false)?;
    assert_eq!(focused_name(&window).as_deref(), Some(TITLE_NAME));
    key(&window, "Backspace", false)?;
    assert_eq!(text(&window, SemanticsRole::TextInput, TITLE_NAME), "");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn an_export_reports_progress_and_its_end_through_commands() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    click(&window, SemanticsRole::Button, EXPORT_LABEL)?;
    wait_until(&window, || {
        text(&window, SemanticsRole::Text, EXPORT_STATUS_NAME) == "Exported 24 thumbnails."
    })?;
    let meter = nodes(&window, SemanticsRole::ProgressBar, EXPORT_PROGRESS_NAME);
    assert!(
        matches!(meter[0].value, Some(SemanticsValue::Range { value, .. }) if value == 24.0),
        "{:?}",
        meter[0].value
    );
    let trace = trace(&window);
    assert!(
        trace
            .lines()
            .any(|line| line.contains("×25") && line.contains("demo.export.progress → widget")),
        "{trace}"
    );
    assert!(
        trace.contains(
            "demo.export.finished → window · Window controller, Status handler · handled"
        ),
        "{trace}"
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_cancelled_export_stops_between_thumbnails() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    click(&window, SemanticsRole::Button, EXPORT_LABEL)?;
    click(&window, SemanticsRole::Button, CANCEL_LABEL)?;
    wait_until(&window, || {
        text(&window, SemanticsRole::Text, EXPORT_STATUS_NAME).starts_with("Cancelled after")
    })?;
    Ok(())
}
