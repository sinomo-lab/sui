//! Headless visual capture of recursive context-menu submenus in both default themes.
//!
//! The trigger sits at the right edge so the nested panels exercise viewport-aware
//! leftward placement. Captures are written under `target/ui-artifacts/` for review.
#![cfg(feature = "artifacts")]
#![forbid(unsafe_code)]

use std::path::PathBuf;

use sui::{
    Align, Alignment, Application, Background, Button, ContextMenu, DefaultTheme, Label, MenuItem,
    PointerButton, Result, SemanticsRole, SizedBox, Stack, WindowBuilder, containers::Padding,
};
use sui_testing::TestApp;

const TRIGGER_LABEL: &str = "Open actions";

fn build_submenu_scene(theme: DefaultTheme) -> Application {
    let menu = ContextMenu::new(
        "Submenu review",
        Button::new(TRIGGER_LABEL).min_width(140.0).theme(theme),
    )
    .theme(theme)
    .activation_button(PointerButton::Primary)
    .anchor_to_pointer(false)
    .items([
        MenuItem::new("Open").shortcut("Enter"),
        MenuItem::new("Rename").shortcut("F2"),
        MenuItem::new("Move to").submenu([
            MenuItem::new("Archive"),
            MenuItem::new("Shared").submenu([
                MenuItem::new("Team workspace"),
                MenuItem::new("Project workspace"),
            ]),
        ]),
        MenuItem::new("Delete")
            .shortcut("Del")
            .separator_before()
            .destructive(),
    ]);

    let content = Stack::vertical()
        .spacing(16.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new("Recursive context menu")
                .font_size(22.0)
                .line_height(28.0)
                .color(theme.palette.text),
        )
        .with_child(
            Label::new("Nested actions remain visible inside the viewport.")
                .font_size(14.0)
                .line_height(20.0)
                .color(theme.palette.text_muted),
        )
        .with_child(
            SizedBox::new()
                .width(1216.0)
                .height(64.0)
                .with_child(Align::new(Alignment::End, Alignment::Center, menu)),
        );

    let root = SizedBox::new()
        .width(1280.0)
        .height(720.0)
        .with_child(Background::new(
            theme.palette.surface,
            Padding::all(32.0, content),
        ));

    Application::new().window(
        WindowBuilder::new()
            .title("Context-menu submenu review")
            .root(root),
    )
}

fn capture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("ui-artifacts")
        .join("context-menu-submenu-review")
}

fn capture_theme(name: &str, theme: DefaultTheme) -> Result<PathBuf> {
    let app = TestApp::from_runtime(build_submenu_scene(theme).build()?)?;
    let window = app.main_window()?;

    window
        .get_by_role(SemanticsRole::Button)
        .with_name(TRIGGER_LABEL)
        .click()?;
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Move to")
        .hover()?;
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Shared")
        .hover()?;
    window
        .get_by_role(SemanticsRole::MenuItem)
        .with_name("Team workspace")
        .hover()?;
    app.advance_time(0.25)?;

    let snapshot = window.snapshot()?;
    let item_bounds = |label: &str| {
        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some(label)
            })
            .unwrap_or_else(|| panic!("missing open submenu item {label}"))
            .bounds
    };
    let move_to = item_bounds("Move to");
    let shared = item_bounds("Shared");
    let team = item_bounds("Team workspace");
    assert!(
        shared.x() < move_to.x() && team.x() < shared.x(),
        "nested panels should continue leftward inside the right viewport edge: \
         move_to={move_to:?}, shared={shared:?}, team={team:?}"
    );

    let screenshot = window.capture_screenshot()?;
    let dir = capture_root();
    std::fs::create_dir_all(&dir)
        .map_err(|error| sui::Error::new(format!("failed to create {}: {error}", dir.display())))?;
    let path = dir.join(format!("context-menu-submenus-{name}.png"));
    screenshot.write_png(&path)?;

    assert!(screenshot.width() > 0 && screenshot.height() > 0);
    Ok(path)
}

#[test]
fn captures_recursive_context_menu_in_dark_and_light_themes() -> Result<()> {
    let dark = capture_theme("dark", DefaultTheme::dark())?;
    let light = capture_theme("light", DefaultTheme::light())?;

    assert!(dark.exists(), "dark capture PNG should exist");
    assert!(light.exists(), "light capture PNG should exist");
    println!("DARK_CAPTURE={}", dark.display());
    println!("LIGHT_CAPTURE={}", light.display());

    Ok(())
}
