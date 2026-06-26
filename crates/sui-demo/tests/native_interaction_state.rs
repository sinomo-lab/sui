#![cfg(feature = "artifacts")]
#![forbid(unsafe_code)]

use sui::{
    Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect, Result,
    SemanticsNode, SemanticsRole, ToggleState,
};
use sui_testing::{Screenshot, TestApp, TestWindow};

const PIXEL_TOLERANCE: u8 = 1;

fn find_node(
    snapshot: &sui_testing::WindowSnapshot,
    role: SemanticsRole,
    name: &str,
) -> SemanticsNode {
    snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("missing semantics node {role:?} named {name}"))
}

fn crop(window: &TestWindow, logical_bounds: Rect) -> Result<Screenshot> {
    let screenshot = window.capture_screenshot()?;
    let snapshot = window.snapshot()?;
    let root = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.parent.is_none())
        .expect("window snapshot should contain a root node");
    let scale_x = screenshot.width() as f32 / root.bounds.width();
    let scale_y = screenshot.height() as f32 / root.bounds.height();
    let padding = 8.0;
    let logical_bounds = Rect::new(
        logical_bounds.x() - padding,
        logical_bounds.y() - padding,
        logical_bounds.width() + (padding * 2.0),
        logical_bounds.height() + (padding * 2.0),
    );
    screenshot.crop(Rect::new(
        logical_bounds.x() * scale_x,
        logical_bounds.y() * scale_y,
        logical_bounds.width() * scale_x,
        logical_bounds.height() * scale_y,
    ))
}

fn crop_diff_count(left: &Screenshot, right: &Screenshot) -> usize {
    assert_eq!(
        (left.width(), left.height()),
        (right.width(), right.height())
    );
    left.pixels()
        .chunks_exact(4)
        .zip(right.pixels().chunks_exact(4))
        .filter(|(left, right)| {
            left.iter()
                .zip(right.iter())
                .any(|(left, right)| left.abs_diff(*right) > PIXEL_TOLERANCE)
        })
        .count()
}

fn assert_crop_changed(label: &str, baseline: &Screenshot, changed: &Screenshot) {
    let diff = crop_diff_count(baseline, changed);
    assert!(
        diff > 0,
        "{label} did not change any rendered pixels after interaction"
    );
}

fn assert_crop_returned(label: &str, baseline: &Screenshot, restored: &Screenshot) {
    let diff = crop_diff_count(baseline, restored);
    assert!(
        diff == 0,
        "{label} remained visually highlighted after outside click; diff pixels: {diff}"
    );
}

fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
    let mut event = PointerEvent::new(kind, position);
    event.button = Some(PointerButton::Primary);
    event.buttons = if pressed {
        PointerButtons::new(1)
    } else {
        PointerButtons::NONE
    };
    Event::Pointer(event)
}

fn click_at(window: &TestWindow, point: Point) -> Result<()> {
    let root = window.root();
    root.dispatch_event(Event::Pointer(PointerEvent::new(
        PointerEventKind::Move,
        point,
    )))?;
    root.dispatch_event(primary_pointer(PointerEventKind::Down, point, true))?;
    root.dispatch_event(primary_pointer(PointerEventKind::Up, point, false))
}

fn advance_interaction_motion(app: &TestApp) -> Result<()> {
    for _ in 0..18 {
        app.advance_time(1.0 / 60.0)?;
    }
    Ok(())
}

fn outside_gallery_point(snapshot: &sui_testing::WindowSnapshot) -> Point {
    let gallery = find_node(
        snapshot,
        SemanticsRole::ScrollView,
        sui_demo_app::widget_book::GALLERY_SCROLL_NAME,
    );
    Point::new(gallery.bounds.max_x() - 32.0, gallery.bounds.y() + 32.0)
}

#[test]
fn native_widget_states_clear_hover_focus_and_visual_highlight_after_outside_click() -> Result<()> {
    let app = TestApp::new_no_vsync(|| {
        sui_demo_app::widget_book::build_widget_book_application(
            sui_demo_app::widget_book::default_widget_book_state(),
        )
    })?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let initial = window.snapshot()?;
    let outside = outside_gallery_point(&initial);
    click_at(&window, outside)?;
    advance_interaction_motion(&app)?;
    let initial = window.snapshot()?;

    let button = window
        .get_by_role(SemanticsRole::Button)
        .with_name(sui_demo_app::widget_book::WIDGET_STATES_BUTTON_LABEL);
    let button_node = find_node(
        &initial,
        SemanticsRole::Button,
        sui_demo_app::widget_book::WIDGET_STATES_BUTTON_LABEL,
    );
    let button_baseline = crop(&window, button_node.bounds)?;

    button.hover()?;
    advance_interaction_motion(&app)?;
    let button_hovered = find_node(
        &window.snapshot()?,
        SemanticsRole::Button,
        sui_demo_app::widget_book::WIDGET_STATES_BUTTON_LABEL,
    );
    assert!(button_hovered.state.hovered);

    button.click()?;
    advance_interaction_motion(&app)?;
    let button_focused = find_node(
        &window.snapshot()?,
        SemanticsRole::Button,
        sui_demo_app::widget_book::WIDGET_STATES_BUTTON_LABEL,
    );
    assert!(button_focused.state.focused);
    let button_focused_crop = crop(&window, button_node.bounds)?;
    assert_crop_changed("button focus", &button_baseline, &button_focused_crop);

    click_at(&window, outside)?;
    advance_interaction_motion(&app)?;
    let button_cleared = find_node(
        &window.snapshot()?,
        SemanticsRole::Button,
        sui_demo_app::widget_book::WIDGET_STATES_BUTTON_LABEL,
    );
    assert!(!button_cleared.state.hovered);
    assert!(!button_cleared.state.focused);
    assert_crop_changed(
        "button outside focus clear",
        &button_focused_crop,
        &crop(&window, button_node.bounds)?,
    );

    let input = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(sui_demo_app::widget_book::WIDGET_STATES_TEXT_INPUT_LABEL);
    let before_input = window.snapshot()?;
    let input_node = find_node(
        &before_input,
        SemanticsRole::TextInput,
        sui_demo_app::widget_book::WIDGET_STATES_TEXT_INPUT_LABEL,
    );
    let input_baseline = crop(&window, input_node.bounds)?;

    input.hover()?;
    advance_interaction_motion(&app)?;
    let input_hovered = find_node(
        &window.snapshot()?,
        SemanticsRole::TextInput,
        sui_demo_app::widget_book::WIDGET_STATES_TEXT_INPUT_LABEL,
    );
    assert!(input_hovered.state.hovered);

    input.click()?;
    window.run_until_idle()?;
    let input_immediate_crop = crop(&window, input_node.bounds)?;
    let input_focused = find_node(
        &window.snapshot()?,
        SemanticsRole::TextInput,
        sui_demo_app::widget_book::WIDGET_STATES_TEXT_INPUT_LABEL,
    );
    assert!(input_focused.state.focused);
    assert_crop_changed("text input focus", &input_baseline, &input_immediate_crop);

    click_at(&window, outside)?;
    advance_interaction_motion(&app)?;
    let input_cleared = find_node(
        &window.snapshot()?,
        SemanticsRole::TextInput,
        sui_demo_app::widget_book::WIDGET_STATES_TEXT_INPUT_LABEL,
    );
    assert!(!input_cleared.state.hovered);
    assert!(!input_cleared.state.focused);
    assert_crop_returned(
        "text input",
        &input_baseline,
        &crop(&window, input_node.bounds)?,
    );

    let checkbox = window
        .get_by_role(SemanticsRole::CheckBox)
        .with_name(sui_demo_app::widget_book::WIDGET_STATES_CHECKBOX_LABEL);
    let before_checkbox = window.snapshot()?;
    let checkbox_node = find_node(
        &before_checkbox,
        SemanticsRole::CheckBox,
        sui_demo_app::widget_book::WIDGET_STATES_CHECKBOX_LABEL,
    );
    let checkbox_baseline = crop(&window, checkbox_node.bounds)?;

    checkbox.hover()?;
    advance_interaction_motion(&app)?;
    let checkbox_hovered = find_node(
        &window.snapshot()?,
        SemanticsRole::CheckBox,
        sui_demo_app::widget_book::WIDGET_STATES_CHECKBOX_LABEL,
    );
    assert!(checkbox_hovered.state.hovered);
    assert_eq!(checkbox_hovered.state.checked, Some(ToggleState::Unchecked));
    assert_crop_changed(
        "checkbox hover",
        &checkbox_baseline,
        &crop(&window, checkbox_node.bounds)?,
    );

    checkbox.click()?;
    advance_interaction_motion(&app)?;
    let checkbox_focused = find_node(
        &window.snapshot()?,
        SemanticsRole::CheckBox,
        sui_demo_app::widget_book::WIDGET_STATES_CHECKBOX_LABEL,
    );
    assert!(checkbox_focused.state.focused);
    assert_eq!(checkbox_focused.state.checked, Some(ToggleState::Checked));
    let checkbox_checked_focused = crop(&window, checkbox_node.bounds)?;
    assert_crop_changed(
        "checkbox checked focus",
        &checkbox_baseline,
        &checkbox_checked_focused,
    );

    click_at(&window, outside)?;
    advance_interaction_motion(&app)?;
    let checkbox_cleared = find_node(
        &window.snapshot()?,
        SemanticsRole::CheckBox,
        sui_demo_app::widget_book::WIDGET_STATES_CHECKBOX_LABEL,
    );
    assert!(!checkbox_cleared.state.hovered);
    assert!(!checkbox_cleared.state.focused);
    assert_eq!(checkbox_cleared.state.checked, Some(ToggleState::Checked));
    assert_crop_changed(
        "checkbox outside focus clear",
        &checkbox_checked_focused,
        &crop(&window, checkbox_node.bounds)?,
    );

    Ok(())
}
