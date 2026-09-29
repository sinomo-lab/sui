use std::rc::Rc;

use sui::prelude::*;
use sui::{
    Brush, Event, PointerButton, PointerButtons, PointerEvent, PointerEventKind, RenderOutput,
    Runtime, SceneCommand, ScrollDelta, SemanticsActionRequest, SemanticsEvent, SemanticsNode,
    SemanticsRole, SemanticsValue, Vector, WindowEvent, WindowId, WindowRenderOptions,
};

use super::contrast::{ContrastUse, contrast_checks};
use super::export::{hex, parse_hex, rust_source};
use super::preview::{
    CONTRAST_REPORT_NAME, DECORATIVE_SPECIMEN_NAME, PREVIEW_STORIES, SURFACES_SPECIMEN_NAME,
};
use super::state::{EditOrigin, Preset, ThemeEditorState, ThemeRecipe};
use super::tokens::{RoleToken, SourceToken, Token, TokenGroup};
use super::*;
use crate::app::{DEV_SHELL_THEME_TOGGLE_NAME, DEV_THEME_CUSTOM_LABEL, DevThemeReader};

const PRIMARY: Token = Token::Source(SourceToken::Primary);

fn editor_runtime(state: ThemeEditorState) -> (Runtime, WindowId) {
    let shell: DevThemeReader = Rc::new(DefaultTheme::sui);
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Theme editor")
                .root(build_theme_editor(state, shell, None)),
        )
        .build()
        .expect("the theme editor builds");
    let window_id = runtime.window_ids()[0];
    resize(&mut runtime, window_id);
    (runtime, window_id)
}

fn dev_shell_runtime() -> (Runtime, WindowId) {
    let mut runtime = crate::app::build_dev_application_with_initial_demo_and_render_options(
        Some(THEME_EDITOR_TAB_LABEL),
        WindowRenderOptions::new(true, 1.0),
    )
    .build()
    .expect("the dev shell builds");
    let window_id = runtime.window_ids()[0];
    resize(&mut runtime, window_id);
    (runtime, window_id)
}

fn resize(runtime: &mut Runtime, window_id: WindowId) {
    runtime
        .handle_event(
            window_id,
            Event::Window(WindowEvent::Resized(Size::new(1440.0, 960.0))),
        )
        .unwrap();
}

fn find(output: &RenderOutput, role: SemanticsRole, name: &str) -> Option<SemanticsNode> {
    output
        .semantics
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
        .cloned()
}

fn node(output: &RenderOutput, role: SemanticsRole, name: &str) -> SemanticsNode {
    find(output, role.clone(), name).unwrap_or_else(|| panic!("{role:?} {name:?} is present"))
}

fn picker_slider(output: &RenderOutput, channel: &str) -> SemanticsNode {
    let picker = node(output, SemanticsRole::ColorPicker, THEME_COLOR_PICKER_NAME);
    output
        .semantics
        .iter()
        .find(|node| node.parent == Some(picker.id) && node.name.as_deref() == Some(channel))
        .cloned()
        .unwrap_or_else(|| panic!("the picker has a {channel} slider"))
}

fn range_value(node: &SemanticsNode) -> f64 {
    match node.value {
        Some(SemanticsValue::Range { value, .. }) => value,
        ref other => panic!("{:?} has a range value, got {other:?}", node.name),
    }
}

fn pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
    let mut event = PointerEvent::new(kind, position);
    event.pointer_id = 7;
    event.button = Some(PointerButton::Primary);
    event.buttons = if pressed {
        PointerButtons::new(1)
    } else {
        PointerButtons::NONE
    };
    Event::Pointer(event)
}

fn center(bounds: Rect) -> Point {
    Point::new(
        bounds.x() + bounds.width() * 0.5,
        bounds.y() + bounds.height() * 0.5,
    )
}

fn click(runtime: &mut Runtime, window_id: WindowId, bounds: Rect) {
    drag(runtime, window_id, center(bounds), center(bounds));
}

fn drag(runtime: &mut Runtime, window_id: WindowId, from: Point, to: Point) {
    for (kind, position, pressed) in [
        (PointerEventKind::Down, from, true),
        (PointerEventKind::Move, to, true),
        (PointerEventKind::Up, to, false),
    ] {
        runtime
            .handle_event(window_id, pointer(kind, position, pressed))
            .unwrap();
    }
}

fn set_value(runtime: &mut Runtime, window_id: WindowId, node: &SemanticsNode, value: &str) {
    runtime
        .handle_event(
            window_id,
            Event::Semantics(SemanticsEvent::new(
                node.id,
                SemanticsActionRequest::SetValue(SemanticsValue::Text(value.into())),
            )),
        )
        .unwrap();
}

fn scroll(runtime: &mut Runtime, window_id: WindowId, over: Rect, delta: f32) {
    let mut wheel = PointerEvent::new(PointerEventKind::Scroll, center(over));
    wheel.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, delta)));
    runtime
        .handle_event(window_id, Event::Pointer(wheel))
        .unwrap();
}

/// Scrolls the token panel until the button named `name` is well inside it.
fn reveal_row(runtime: &mut Runtime, window_id: WindowId, name: &str) -> RenderOutput {
    for _ in 0..40 {
        let output = runtime.render(window_id).unwrap();
        let panel = node(
            &output,
            SemanticsRole::ScrollView,
            THEME_EDITOR_CONTROLS_SCROLL_NAME,
        )
        .bounds;
        let row = node(&output, SemanticsRole::Button, name).bounds;
        if row.y() >= panel.y() + 40.0 && row.max_y() <= panel.max_y() - 40.0 {
            return output;
        }
        scroll(
            runtime,
            window_id,
            panel,
            if row.y() < panel.y() + 40.0 {
                120.0
            } else {
                -120.0
            },
        );
    }
    panic!("{name} never scrolled into view");
}

fn rgba8(color: Color) -> [u8; 4] {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        byte(color.red),
        byte(color.green),
        byte(color.blue),
        byte(color.alpha),
    ]
}

/// Solid fill colors painted inside `area`, as 8-bit RGBA.
fn fills_in(output: &RenderOutput, area: Rect) -> Vec<[u8; 4]> {
    let mut colors = Vec::new();
    output.frame.scene.visit_commands(&mut |command| {
        let (bounds, brush) = match command {
            SceneCommand::FillRect { rect, brush } => (*rect, brush),
            SceneCommand::FillRoundedRect { rect, brush, .. } => (*rect, brush),
            SceneCommand::FillPath { path, brush } => (path.bounds(), brush),
            _ => return,
        };
        if let Brush::Solid(color) = brush
            && area.contains(center(bounds))
        {
            colors.push(rgba8(*color));
        }
    });
    colors
}

#[test]
fn unedited_recipes_rebuild_their_presets() {
    for preset in Preset::ALL {
        assert!(
            ThemeRecipe::from_preset(preset).build() == preset.theme(),
            "{} rebuilds unchanged",
            preset.label()
        );
    }
}

#[test]
fn source_edits_rederive_roles_and_keep_on_colors() {
    let state = ThemeEditorState::new();
    let on_primary = state.theme().colors.on_primary;
    let yellow = Color::rgba(0.95, 0.85, 0.2, 1.0);
    state.set_color(PRIMARY, yellow, EditOrigin::Picker);

    let theme = state.theme();
    assert_eq!(theme.colors.primary, yellow);
    assert_eq!(theme.palette.accent, yellow);
    assert_eq!(theme.palette.accent_text, on_primary);
    assert_ne!(
        theme.palette.accent_hover,
        DefaultTheme::sui().palette.accent_hover
    );
    assert!(state.is_modified(PRIMARY));
    assert!(!state.is_modified(Token::Source(SourceToken::OnPrimary)));
}

#[test]
fn role_overrides_survive_source_and_shape_edits_until_removed() {
    let state = ThemeEditorState::new();
    let hover = Color::rgba(0.88, 0.18, 0.52, 0.42);
    let role = Token::Role(RoleToken::ControlHover);
    state.add_override(RoleToken::ControlHover);
    assert_eq!(state.selected_signal().get(), Some(role));
    state.set_color(role, hover, EditOrigin::Hex);
    state.set_color(
        PRIMARY,
        Color::rgba(0.75, 0.2, 0.16, 1.0),
        EditOrigin::Picker,
    );
    state.set_spacing(7.0);
    state.set_radius_scale(1.25);
    state.set_text_scale(1.1);
    assert_eq!(state.theme().palette.control_hover, hover);
    assert!(!state.available_roles().contains(&RoleToken::ControlHover));

    state.reset(role);
    assert_ne!(state.theme().palette.control_hover, hover);
    assert!(state.overrides().is_empty());
}

#[test]
fn presets_and_reset_all_drop_every_edit() {
    let state = ThemeEditorState::new();
    state.set_color(PRIMARY, Color::rgba(0.1, 0.8, 0.3, 1.0), EditOrigin::Picker);
    state.add_override(RoleToken::Caret);
    state.set_control_size(ControlSize::Large);
    state.set_motion_scale(0.5);

    state.reset_all();
    assert!(state.theme() == DefaultTheme::sui());
    assert!(state.overrides().is_empty());
    assert_eq!(state.selected_signal().get(), None);

    state.set_preset(Preset::NeutralDark);
    assert!(state.theme() == DefaultTheme::neutral_dark());
    assert_eq!(state.preset(), Preset::NeutralDark);
}

#[test]
fn edit_summary_describes_what_changed() {
    let state = ThemeEditorState::new();
    assert_eq!(state.edit_summary(), "SUI light with no changes");
    state.set_color(PRIMARY, Color::rgba(0.1, 0.8, 0.3, 1.0), EditOrigin::Picker);
    state.set_color(
        Token::Source(SourceToken::Panel),
        Color::rgba(0.98, 0.97, 0.95, 1.0),
        EditOrigin::Hex,
    );
    state.add_override(RoleToken::Caret);
    state.set_spacing(6.0);
    assert_eq!(
        state.edit_summary(),
        "SUI light: 2 colors changed, 1 role override, shape and type changed"
    );
    state.set_status("copied Rust source");
    assert!(
        state
            .summary_signal()
            .get()
            .ends_with(" · copied Rust source")
    );
    state.set_spacing(5.0);
    assert!(!state.summary_signal().get().contains("copied"));
}

#[test]
fn token_inventory_covers_every_source_color_and_role() {
    assert_eq!(SourceToken::ALL.len(), 40);
    assert_eq!(RoleToken::ALL.len(), 53);
    assert_eq!(
        TokenGroup::ALL
            .into_iter()
            .map(|group| group.tokens().count())
            .sum::<usize>(),
        SourceToken::ALL.len()
    );
    let marker = Color::rgba(0.123, 0.456, 0.789, 0.625);
    for token in SourceToken::ALL {
        let mut colors = ThemeColors::sui();
        let before = SourceToken::ALL
            .iter()
            .map(|other| other.get(&colors))
            .collect::<Vec<_>>();
        token.set(&mut colors, marker);
        for (other, previous) in SourceToken::ALL.iter().zip(before) {
            let expected = if other == token { marker } else { previous };
            assert_eq!(
                other.get(&colors),
                expected,
                "editing {} leaves {} alone",
                token.label(),
                other.label()
            );
        }
    }
}

#[test]
fn built_in_presets_pass_every_contrast_check() {
    for preset in Preset::ALL {
        for check in contrast_checks(&preset.theme()) {
            assert!(
                check.passes(),
                "{}: {} is {}",
                preset.label(),
                check.label,
                check.badge()
            );
        }
    }
}

#[test]
fn contrast_ratings_follow_wcag_thresholds() {
    let theme = DefaultTheme::sui();
    let mut checks = contrast_checks(&theme);
    assert_eq!(checks.len(), 18);
    let mut text = checks.remove(0);
    assert_eq!(text.usage, ContrastUse::Text);
    text.background = Color::rgba(1.0, 1.0, 1.0, 1.0);
    for (foreground, rating) in [
        (Color::rgba(0.0, 0.0, 0.0, 1.0), "AAA"),
        (Color::rgba(0.45, 0.45, 0.45, 1.0), "AA"),
        (Color::rgba(0.6, 0.6, 0.6, 1.0), "Fail"),
    ] {
        text.foreground = foreground;
        assert_eq!(text.rating(), rating, "{:.2}", text.ratio());
    }
    let outline = checks
        .iter()
        .find(|check| check.usage == ContrastUse::Graphic)
        .expect("outline checks exist");
    assert_eq!(outline.minimum(), 3.0);
}

#[test]
fn hex_values_round_trip() {
    let color = Color::rgba(23.0 / 255.0, 98.0 / 255.0, 244.0 / 255.0, 1.0);
    assert_eq!(hex(color), "#1762F4");
    assert_eq!(parse_hex("#1762F4"), Some(color));
    assert_eq!(parse_hex("1762f4"), Some(color));
    assert_eq!(parse_hex("#fff"), Some(Color::rgba(1.0, 1.0, 1.0, 1.0)));
    assert_eq!(hex(parse_hex("#1762F480").unwrap()), "#1762F480");
    for invalid in ["", "#12", "#12345", "#GGGGGG", "#1762F4801"] {
        assert_eq!(parse_hex(invalid), None, "{invalid:?}");
    }
}

#[test]
fn rust_export_spells_out_every_source_and_edit() {
    let state = ThemeEditorState::new();
    let plain = rust_source(&state.recipe());
    assert!(
        plain
            .starts_with("// Exported from the SUI theme editor, based on the SUI light preset.\n")
    );
    assert!(plain.contains("pub fn custom_theme() -> DefaultTheme {"));
    for token in SourceToken::ALL {
        let field = format!(" {}: Color::rgba(", token.field());
        assert_eq!(plain.matches(&field).count(), 1, "{field} appears once");
    }
    assert!(!plain.contains("sync_derived_fields"));
    assert!(plain.contains("theme.with_size(ControlSize::Medium)"));

    state.set_color(PRIMARY, Color::rgba(0.1, 0.8, 0.3, 1.0), EditOrigin::Picker);
    state.add_override(RoleToken::Caret);
    state.set_color(
        Token::Role(RoleToken::Caret),
        Color::rgba(1.0, 0.0, 0.0, 1.0),
        EditOrigin::Hex,
    );
    state.set_radius_scale(1.5);
    state.set_control_size(ControlSize::Large);
    let edited = rust_source(&state.recipe());
    assert!(edited.contains("        primary: Color::rgba(0.1, 0.8, 0.3, 1.0), // #1ACC4D\n"));
    assert!(
        edited.contains("    theme.palette.caret = Color::rgba(1.0, 0.0, 0.0, 1.0); // #FF0000\n")
    );
    assert!(edited.contains("    theme.radius = ThemeRadii {\n"));
    assert!(edited.contains("    theme.sync_derived_fields();\n"));
    assert!(edited.contains("theme.with_size(ControlSize::Large)"));
    assert!(edited.contains(
        "use sui::{Color, ControlSize, DecorativeColors, DefaultTheme, NeutralRamp, \
         ThemeColorScheme, ThemeColors, ThemeMotion, ThemeRadii, ThemeTextScale};"
    ));
}

#[test]
fn pressing_a_token_opens_its_editor_beneath_it() {
    let (mut runtime, window_id) = editor_runtime(ThemeEditorState::new());
    let output = runtime.render(window_id).unwrap();
    assert!(find(&output, SemanticsRole::ColorPicker, THEME_COLOR_PICKER_NAME).is_none());
    let primary = node(&output, SemanticsRole::Button, "Primary");
    assert_eq!(primary.state.expanded, Some(false));
    assert_eq!(primary.value, Some(SemanticsValue::Text("#1762F4".into())));
    click(&mut runtime, window_id, primary.bounds);

    let output = runtime.render(window_id).unwrap();
    let primary = node(&output, SemanticsRole::Button, "Primary");
    let on_primary = node(&output, SemanticsRole::Button, "On primary");
    let picker = node(&output, SemanticsRole::ColorPicker, THEME_COLOR_PICKER_NAME);
    assert_eq!(primary.state.expanded, Some(true));
    assert!(picker.bounds.y() > primary.bounds.max_y());
    assert!(picker.bounds.max_y() < on_primary.bounds.y());
    assert!(
        picker
            .description
            .as_deref()
            .is_some_and(|text| text.starts_with("OKLCH sliders")),
        "the editor opens in OKLCH: {:?}",
        picker.description
    );
    assert!(
        on_primary
            .description
            .as_deref()
            .unwrap_or("")
            .contains(":1 AA")
    );

    click(&mut runtime, window_id, primary.bounds);
    let output = runtime.render(window_id).unwrap();
    assert!(find(&output, SemanticsRole::ColorPicker, THEME_COLOR_PICKER_NAME).is_none());
}

#[test]
fn picker_drags_repaint_the_preview_and_the_token_row() {
    let state = ThemeEditorState::new();
    let (mut runtime, window_id) = editor_runtime(state.clone());
    let output = runtime.render(window_id).unwrap();
    click(
        &mut runtime,
        window_id,
        node(&output, SemanticsRole::Button, "Primary").bounds,
    );
    let output = runtime.render(window_id).unwrap();
    let preview = node(
        &output,
        SemanticsRole::ScrollView,
        THEME_EDITOR_PREVIEW_SCROLL_NAME,
    )
    .bounds;
    let original = state.theme().palette.accent;
    assert!(fills_in(&output, preview).contains(&rgba8(original)));

    let lightness = picker_slider(&output, "Lightness");
    let before = range_value(&lightness);
    let row = lightness.bounds;
    drag(
        &mut runtime,
        window_id,
        Point::new(row.x() + row.width() * 0.5, row.y() + row.height() * 0.5),
        Point::new(row.x() + row.width() * 0.3, row.y() + row.height() * 0.5),
    );

    let output = runtime.render(window_id).unwrap();
    let edited = state.theme().palette.accent;
    assert!(range_value(&picker_slider(&output, "Lightness")) < before - 5.0);
    assert!(edited.to_oklch().lightness < original.to_oklch().lightness - 0.05);
    let fills = fills_in(&output, preview);
    assert!(fills.contains(&rgba8(edited)), "the preview repaints");
    assert!(!fills.contains(&rgba8(original)), "the old primary is gone");
    assert_eq!(
        node(&output, SemanticsRole::Button, "Primary").value,
        Some(SemanticsValue::Text(hex(edited)))
    );
    let hex_field = node(&output, SemanticsRole::TextInput, THEME_HEX_NAME);
    assert_eq!(hex_field.value, Some(SemanticsValue::Text(hex(edited))));
}

#[test]
fn hex_entry_updates_the_color_and_the_picker() {
    let state = ThemeEditorState::new();
    let (mut runtime, window_id) = editor_runtime(state.clone());
    let output = runtime.render(window_id).unwrap();
    click(
        &mut runtime,
        window_id,
        node(&output, SemanticsRole::Button, "Primary").bounds,
    );
    let output = runtime.render(window_id).unwrap();
    let field = node(&output, SemanticsRole::TextInput, THEME_HEX_NAME);
    set_value(&mut runtime, window_id, &field, "#E0115F");

    let output = runtime.render(window_id).unwrap();
    let ruby = parse_hex("#E0115F").unwrap();
    assert_eq!(state.theme().colors.primary, ruby);
    let hue = range_value(&picker_slider(&output, "Hue"));
    assert!(
        (hue - f64::from(ruby.to_oklch().hue)).abs() < 0.5,
        "the picker follows the hex value: {hue}"
    );
}

#[test]
fn role_override_select_adds_a_row_that_can_be_removed() {
    let state = ThemeEditorState::new();
    let (mut runtime, window_id) = editor_runtime(state.clone());
    let output = runtime.render(window_id).unwrap();
    let select = node(&output, SemanticsRole::ComboBox, THEME_OVERRIDE_ROLE_NAME);
    set_value(&mut runtime, window_id, &select, "Focus ring");

    let output = runtime.render(window_id).unwrap();
    assert_eq!(state.overrides(), vec![RoleToken::FocusRing]);
    let row = node(&output, SemanticsRole::Button, "Focus ring role");
    assert_eq!(row.state.expanded, Some(true), "a new override opens");
    let output = reveal_row(&mut runtime, window_id, "Remove override");
    let remove = node(&output, SemanticsRole::Button, "Remove override");
    click(&mut runtime, window_id, remove.bounds);

    let output = runtime.render(window_id).unwrap();
    assert!(state.overrides().is_empty());
    assert!(find(&output, SemanticsRole::Button, "Focus ring role").is_none());
}

#[test]
fn token_selection_keeps_the_panel_scroll_position() {
    let (mut runtime, window_id) = editor_runtime(ThemeEditorState::new());
    let output = reveal_row(&mut runtime, window_id, "Text tertiary");
    let row = node(&output, SemanticsRole::Button, "Text tertiary").bounds;
    click(&mut runtime, window_id, row);

    let output = runtime.render(window_id).unwrap();
    let after = node(&output, SemanticsRole::Button, "Text tertiary").bounds;
    assert!(
        (after.y() - row.y()).abs() <= 1.0,
        "the row stays put while its editor opens: {row:?} -> {after:?}"
    );
    let picker = node(&output, SemanticsRole::ColorPicker, THEME_COLOR_PICKER_NAME);
    assert!(picker.bounds.y() > after.max_y());
}

#[test]
fn scale_slider_drag_updates_its_label_and_the_theme() {
    let state = ThemeEditorState::new();
    let (mut runtime, window_id) = editor_runtime(state.clone());
    let output = runtime.render(window_id).unwrap();
    let panel = node(
        &output,
        SemanticsRole::ScrollView,
        THEME_EDITOR_CONTROLS_SCROLL_NAME,
    )
    .bounds;
    scroll(&mut runtime, window_id, panel, -5000.0);
    let output = runtime.render(window_id).unwrap();
    let slider = node(&output, SemanticsRole::Slider, THEME_SPACING_NAME).bounds;
    assert!(find(&output, SemanticsRole::Text, "4.0 px").is_some());
    drag(
        &mut runtime,
        window_id,
        center(slider),
        Point::new(slider.max_x() - 1.0, slider.y() + slider.height() * 0.5),
    );

    let output = runtime.render(window_id).unwrap();
    assert_eq!(state.theme().spacing, 12.0);
    assert!(find(&output, SemanticsRole::Text, "12.0 px").is_some());
    assert!(state.edit_summary().ends_with("shape and type changed"));
}

#[test]
fn preview_shows_contrast_surfaces_and_every_story() {
    let (mut runtime, window_id) = editor_runtime(ThemeEditorState::new());
    let output = runtime.render(window_id).unwrap();
    for name in [
        CONTRAST_REPORT_NAME,
        SURFACES_SPECIMEN_NAME,
        DECORATIVE_SPECIMEN_NAME,
    ] {
        assert!(
            output
                .semantics
                .iter()
                .any(|node| node.name.as_deref() == Some(name)),
            "{name} is in the preview"
        );
    }
    let preview = node(
        &output,
        SemanticsRole::ScrollView,
        THEME_EDITOR_PREVIEW_SCROLL_NAME,
    )
    .bounds;
    for id in PREVIEW_STORIES {
        let story = crate::widget_book::story_region_name(id);
        let mut found = false;
        for _ in 0..60 {
            let output = runtime.render(window_id).unwrap();
            if output
                .semantics
                .iter()
                .any(|node| node.name.as_deref() == Some(story.as_str()))
            {
                found = true;
                break;
            }
            scroll(&mut runtime, window_id, preview, -400.0);
        }
        assert!(found, "{story} is in the preview");
    }
}

#[test]
fn copy_as_rust_puts_the_theme_on_the_clipboard() {
    let (mut runtime, window_id) = dev_shell_runtime();
    let output = runtime.render(window_id).unwrap();
    let copy = node(&output, SemanticsRole::Button, THEME_COPY_RUST_NAME);
    click(&mut runtime, window_id, copy.bounds);

    let output = runtime.render(window_id).unwrap();
    let text = runtime.clipboard().text().expect("the export is copied");
    assert!(text.contains("pub fn custom_theme() -> DefaultTheme {"));
    assert!(
        output.semantics.iter().any(|node| node
            .name
            .as_deref()
            .is_some_and(|name| name.ends_with("copied Rust source"))),
        "the summary confirms the copy"
    );
}

#[test]
fn use_as_app_theme_restyles_the_whole_app() {
    let (mut runtime, window_id) = dev_shell_runtime();
    let output = runtime.render(window_id).unwrap();
    click(
        &mut runtime,
        window_id,
        node(&output, SemanticsRole::Button, "Primary").bounds,
    );
    let output = runtime.render(window_id).unwrap();
    let field = node(&output, SemanticsRole::TextInput, THEME_HEX_NAME);
    set_value(&mut runtime, window_id, &field, "#0F8A5F");
    let output = runtime.render(window_id).unwrap();
    let apply = node(&output, SemanticsRole::Button, THEME_USE_AS_APP_THEME_NAME);
    let green = parse_hex("#0F8A5F").unwrap();
    assert!(
        !fills_in(&output, apply.bounds).contains(&rgba8(green)),
        "the shell still uses its own primary"
    );
    click(&mut runtime, window_id, apply.bounds);

    let output = runtime.render(window_id).unwrap();
    let toggle = node(&output, SemanticsRole::Switch, DEV_SHELL_THEME_TOGGLE_NAME);
    assert_eq!(
        toggle.value,
        Some(SemanticsValue::Text(DEV_THEME_CUSTOM_LABEL.into()))
    );
    let apply = node(&output, SemanticsRole::Button, THEME_USE_AS_APP_THEME_NAME);
    assert!(
        fills_in(&output, apply.bounds).contains(&rgba8(green)),
        "the shell's own primary button now uses the edited primary"
    );

    click(&mut runtime, window_id, toggle.bounds);
    let output = runtime.render(window_id).unwrap();
    let toggle = node(&output, SemanticsRole::Switch, DEV_SHELL_THEME_TOGGLE_NAME);
    assert_eq!(toggle.value, Some(SemanticsValue::Text("Dark".into())));
}

/// A recipe that exercises every part of the export.
fn export_fixture_state() -> ThemeEditorState {
    let state = ThemeEditorState::new();
    state.set_preset(Preset::SuiDark);
    state.set_color(PRIMARY, parse_hex("#0F8A5F").unwrap(), EditOrigin::Hex);
    state.set_color(
        Token::Source(SourceToken::Panel),
        parse_hex("#1C1F26").unwrap(),
        EditOrigin::Hex,
    );
    state.add_override(RoleToken::Caret);
    state.set_color(
        Token::Role(RoleToken::Caret),
        parse_hex("#FF5A1F").unwrap(),
        EditOrigin::Hex,
    );
    state.set_control_size(ControlSize::Large);
    state.set_spacing(5.0);
    state.set_radius_scale(1.5);
    state.set_text_scale(1.1);
    state.set_motion_scale(0.5);
    state
}

/// `export_fixture.rs` is the export of [`export_fixture_state`], checked in
/// so the compiler proves exported code builds.
mod exported {
    include!("export_fixture.rs");
}

#[test]
fn rust_export_compiles_and_rebuilds_the_edited_theme() {
    let state = export_fixture_state();
    assert_eq!(
        rust_source(&state.recipe()),
        include_str!("export_fixture.rs").replace("\r\n", "\n"),
        "regenerate export_fixture.rs after changing the export format"
    );
    let mut rebuilt = exported::custom_theme();
    assert_eq!(rebuilt.colors.name, "custom");
    rebuilt.colors.name = state.theme().colors.name;
    assert!(
        rebuilt == state.theme(),
        "the export rebuilds the same theme"
    );
}
