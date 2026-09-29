use sui::{
    ColorSpace, Event, ImeEvent, KeyState, KeyboardEvent, Result, SemanticsNode, SemanticsRole,
    SemanticsValue, Size, ToggleState, Vector,
};
use sui_testing::prelude::*;

use super::documents::highlight;
use super::*;
use crate::test_support::*;

fn page_app() -> Result<TestApp> {
    TestApp::builder(build_text_editor_application)
        .vsync(false)
        .launch()
}

fn node(window: &TestWindow, role: SemanticsRole, name: &str) -> SemanticsNode {
    window
        .snapshot()
        .expect("snapshot")
        .accessibility
        .nodes
        .into_iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("{name} is on the page"))
}

fn text_value(node: &SemanticsNode) -> String {
    match &node.value {
        Some(SemanticsValue::Text(text)) => text.clone(),
        other => panic!("{:?} has no text value: {other:?}", node.name),
    }
}

/// What the inspector's row `name` says.
fn inspector(window: &TestWindow, name: &str) -> String {
    text_value(&node(window, SemanticsRole::GenericContainer, name))
}

fn editor_text(window: &TestWindow) -> String {
    text_value(&node(window, SemanticsRole::TextInput, TEXT_EDITOR_NAME))
}

fn wraps(window: &TestWindow) -> bool {
    node(window, SemanticsRole::Switch, WRAP_LABEL)
        .state
        .checked
        == Some(ToggleState::Checked)
}

/// Choose the option `steps` below the current one in the select `name`.
fn choose(window: &TestWindow, name: &str, steps: usize) -> Result<()> {
    window
        .get_by_role(SemanticsRole::ComboBox)
        .with_name(name)
        .click()?;
    for _ in 0..steps {
        window.focused().press("ArrowDown")?;
    }
    window.focused().press("Enter")?;
    window.run_until_idle()
}

fn key(name: &str, shift: bool, control: bool) -> Event {
    let mut event = KeyboardEvent::new(name, KeyState::Pressed);
    event.modifiers.shift = shift;
    event.modifiers.control = control;
    Event::Keyboard(event)
}

/// Focus the editor with the caret at the start of the document.
fn focus_at_start(window: &TestWindow) -> Result<Locator> {
    let editor = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_EDITOR_NAME);
    editor.focus()?;
    editor.dispatch_event(key("Home", false, true))?;
    window.run_until_idle()?;
    Ok(editor)
}

#[test]
fn opens_code_and_the_inspector_describes_it() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    let code = Document::Code.text();
    assert_eq!(editor_text(&window), code);
    assert!(!wraps(&window), "code opens without wrapping");
    assert_eq!(
        node(&window, SemanticsRole::ComboBox, DIRECTION_NAME).value,
        Some(SemanticsValue::Text("Automatic".to_string()))
    );
    assert_eq!(inspector(&window, CARET_NAME), "Line 1, column 1");
    assert_eq!(inspector(&window, SELECTION_NAME), "Nothing selected");
    assert_eq!(inspector(&window, COMPOSITION_NAME), "Not composing");
    assert_eq!(
        inspector(&window, SIZE_NAME),
        format!("{} bytes", grouped(code.len()))
    );
    let on_screen = inspector(&window, ON_SCREEN_NAME);
    // The line after the last line break counts, as the caret can go there.
    let lines = code.split('\n').count();
    assert!(on_screen.starts_with("Lines 1–"), "{on_screen}");
    assert!(on_screen.ends_with(&format!(" of {lines}")), "{on_screen}");
    Ok(())
}

#[test]
fn typing_composing_and_selecting_show_in_the_inspector() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    let editor = focus_at_start(&window)?;
    let size = Document::Code.text().len();

    editor.fill("let ")?;
    window.run_until_idle()?;
    assert!(editor_text(&window).starts_with("let //! A small layout cache"));
    assert_eq!(inspector(&window, CARET_NAME), "Line 1, column 5");
    assert_eq!(
        inspector(&window, SIZE_NAME),
        format!("{} bytes", grouped(size + 4))
    );

    editor.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate {
        text: "にほ".to_string(),
        cursor_range: Some(2..2),
    }))?;
    window.run_until_idle()?;
    assert_eq!(
        inspector(&window, COMPOSITION_NAME),
        "Composing \u{201c}にほ\u{201d}"
    );
    // The composition is not part of the document yet.
    assert_eq!(
        inspector(&window, SIZE_NAME),
        format!("{} bytes", grouped(size + 4))
    );

    editor.dispatch_event(Event::Ime(ImeEvent::CompositionCommit {
        text: "日本".to_string(),
    }))?;
    editor.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))?;
    window.run_until_idle()?;
    assert_eq!(inspector(&window, COMPOSITION_NAME), "Not composing");
    assert_eq!(inspector(&window, CARET_NAME), "Line 1, column 7");

    for _ in 0..2 {
        editor.dispatch_event(key("ArrowLeft", true, false))?;
    }
    window.run_until_idle()?;
    // "日本" is two characters and six bytes, after the four of "let ".
    assert_eq!(
        inspector(&window, SELECTION_NAME),
        "2 characters, bytes 4–10"
    );
    Ok(())
}

/// Pixels in `screenshot` close to `color`.
fn pixels_like(screenshot: &Screenshot, color: Color) -> usize {
    let srgb = color.to_space(ColorSpace::Srgb);
    let target = [srgb.red, srgb.green, srgb.blue]
        .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
    screenshot
        .pixels()
        .chunks_exact(4)
        .filter(|pixel| (0..3).all(|channel| pixel[channel].abs_diff(target[channel]) <= 24))
        .count()
}

#[test]
fn highlighting_follows_typing() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    let editor = focus_at_start(&window)?;
    let accent = DefaultTheme::default().palette.accent;
    let before = pixels_like(&editor.capture_screenshot()?, accent);

    // The first line is a comment; a keyword typed before it turns accent.
    editor.fill("impl ")?;
    window.run_until_idle()?;
    let after = pixels_like(&editor.capture_screenshot()?, accent);
    assert!(
        after > before,
        "{after} accent pixels after typing, {before} before"
    );
    Ok(())
}

#[test]
fn documents_choose_wrapping_and_the_large_one_scrolls() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;

    choose(&window, DOCUMENT_NAME, 1)?;
    assert!(editor_text(&window).starts_with("Mixed-direction prose"));
    assert!(wraps(&window), "prose opens wrapped");

    choose(&window, DOCUMENT_NAME, 2)?;
    assert_eq!(
        node(&window, SemanticsRole::ComboBox, DOCUMENT_NAME).value,
        Some(SemanticsValue::Text(Document::Large.name().to_string()))
    );
    assert!(!wraps(&window), "the large document opens without wrapping");
    let on_screen = inspector(&window, ON_SCREEN_NAME);
    assert!(on_screen.starts_with("Lines 1–"), "{on_screen}");
    assert!(
        on_screen.ends_with(&format!(" of {}", grouped(LARGE_LINES + 1))),
        "{on_screen}"
    );

    window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_EDITOR_NAME)
        .scroll_pixels(Vector::new(0.0, -4_000.0))?;
    window.run_until_idle()?;
    let scrolled = inspector(&window, ON_SCREEN_NAME);
    assert!(!scrolled.starts_with("Lines 1–"), "{scrolled}");
    Ok(())
}

#[test]
fn wrapping_and_direction_change_the_layout() -> Result<()> {
    let app = page_app()?;
    let window = app.main_window()?;
    window.run_until_idle()?;
    let editor = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(TEXT_EDITOR_NAME);
    let unwrapped = inspector(&window, ON_SCREEN_NAME);

    window
        .get_by_role(SemanticsRole::Switch)
        .with_name(WRAP_LABEL)
        .click()?;
    window.run_until_idle()?;
    assert!(wraps(&window));
    // Long lines wrap, so fewer document lines fit on screen.
    let wrapped = inspector(&window, ON_SCREEN_NAME);
    assert_ne!(wrapped, unwrapped);

    let left_to_right = editor.capture_screenshot()?;
    choose(&window, DIRECTION_NAME, 2)?;
    assert_eq!(
        node(&window, SemanticsRole::ComboBox, DIRECTION_NAME).value,
        Some(SemanticsValue::Text("Right to left".to_string()))
    );
    assert_ne!(
        left_to_right,
        editor.capture_screenshot()?,
        "right-to-left paragraphs start at the right edge"
    );
    Ok(())
}

#[test]
fn highlighting_colors_rust() {
    let theme = DefaultTheme::default();
    let base = text_style(theme);
    let text = "// note\npub fn cache(width: f32) -> Option<Line> { \"ok\" 42 }";
    let spans = highlight(text, &base, theme);
    let color_of = |word: &str| {
        let start = text.find(word).expect("word in text");
        spans
            .iter()
            .find(|span| span.range.start == start)
            .map(|span| span.style.color)
    };
    assert_eq!(color_of("// note"), Some(theme.palette.text_muted));
    assert_eq!(color_of("pub"), Some(theme.palette.accent));
    assert_eq!(color_of("fn"), Some(theme.palette.accent));
    assert_eq!(color_of("Option"), Some(theme.palette.info));
    assert_eq!(color_of("\"ok\""), Some(theme.palette.success));
    assert_eq!(color_of("42"), Some(theme.palette.warning));
    // Plain identifiers keep the base style.
    assert_eq!(color_of("cache"), None);
    assert!(
        spans
            .iter()
            .all(|span| text.get(span.range.clone()).is_some())
    );
}

#[test]
fn text_editor_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        TEXT_EDITOR_VIEW_TITLE,
        Size::new(900.0, 700.0),
        build_text_editor_surface_with_theme,
    )
}
