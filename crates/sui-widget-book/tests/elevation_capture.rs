//! Headless visual capture of the elevated widgets (Dialog, Menu, Popover, and a
//! PanelSection card) in both the dark and light default themes.
//!
//! This renders a focused scene through the headless `sui-testing` harness (wgpu)
//! and writes one PNG per theme into the workspace `target/` directory so a human
//! can review the motion + elevation changes (shadows / umbrella surfaces).
#![cfg(feature = "artifacts")]
#![forbid(unsafe_code)]

use std::path::PathBuf;

use sui::{
    Alignment, Application, Background, Color, DefaultTheme, Dialog, Label, Menu, MenuItem,
    PanelSection, Popover, Result, SizedBox, Stack, WindowBuilder, containers::Padding,
};
use sui_testing::TestApp;

/// Builds the elevated-widget review scene for a single theme.
///
/// The scene intentionally places each elevated surface over a flat themed
/// background so the drop shadow / elevation around each card reads clearly.
fn build_elevation_scene(theme: DefaultTheme) -> Application {
    // A neutral mid-tone base so both the raised surface AND its soft drop shadow
    // read clearly. The natural page background (`base_100`) is pure black in the
    // dark theme and near-white in the light theme, which leaves no tonal room for
    // a drop shadow to register, so we use an explicit mid-tone per scheme.
    let background = if theme.colors.scheme == sui::ThemeColorScheme::Dark {
        Color::rgba(0.13, 0.14, 0.16, 1.0)
    } else {
        // The light theme's surfaces are near-white and clip to 255 in this
        // pipeline, so a strong mid-gray base is needed for the raised surfaces
        // and their drop shadows to register.
        Color::rgba(0.55, 0.58, 0.63, 1.0)
    };

    // A PanelSection card, themed. The PanelSection renders its own elevated surface.
    let panel_card = SizedBox::new().width(320.0).with_child(
        PanelSection::new(
            "Layers",
            Stack::vertical()
                .spacing(6.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new("Background")
                        .font_size(14.0)
                        .line_height(18.0)
                        .color(theme.palette.text),
                )
                .with_child(
                    Label::new("Paint")
                        .font_size(14.0)
                        .line_height(18.0)
                        .color(theme.palette.text),
                )
                .with_child(
                    Label::new("Adjustment")
                        .font_size(14.0)
                        .line_height(18.0)
                        .color(theme.palette.text_muted),
                ),
        )
        .theme(theme),
    );

    // A command Menu, themed and rendered with its items inline (an elevated surface).
    let menu = SizedBox::new().width(280.0).with_child(
        Menu::new("Command menu")
            .theme(theme)
            .item(MenuItem::new("New tab").shortcut("Ctrl+T"))
            .item(MenuItem::new("Duplicate panel").shortcut("Ctrl+D"))
            .item(
                MenuItem::new("Delete layer")
                    .shortcut("Del")
                    .separator_before()
                    .destructive(),
            ),
    );

    // A Popover, forced open so its floating/elevated surface is captured.
    let popover = SizedBox::new().width(360.0).with_child(
        Popover::new(
            "Inspector popover",
            sui::Button::new("Open inspector")
                .min_width(190.0)
                .theme(theme),
            Stack::vertical()
                .spacing(8.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new("Inline inspector content stays lightweight.")
                        .font_size(14.0)
                        .line_height(19.0)
                        .color(theme.palette.text),
                )
                .with_child(
                    Label::new("Blend preview: Screen @ 72%")
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(theme.palette.text_muted),
                ),
        )
        .theme(theme)
        .open(true),
    );

    let column = Stack::vertical()
        .spacing(24.0)
        .alignment(Alignment::Start)
        .with_child(panel_card)
        .with_child(menu)
        .with_child(popover);

    // A non-modal Dialog overlaying the column. `shown` defaults to true, so the
    // elevated dialog card + scrim render directly.
    let dialog = Dialog::new(
        "Project settings",
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Label::new("Autosave every 90 seconds")
                    .font_size(14.0)
                    .line_height(18.0)
                    .color(theme.palette.text),
            )
            .with_child(
                Label::new("Export color profile: Display P3")
                    .font_size(14.0)
                    .line_height(18.0)
                    .color(theme.palette.text),
            ),
    )
    .description("Compact dialog framing for confirmations and settings.")
    .modal(false)
    .theme(theme)
    .secondary_action("Cancel", || {})
    .primary_action("Apply", || {});

    let root = Background::new(
        background,
        Padding::all(
            32.0,
            Stack::new(sui::Axis::Vertical)
                .alignment(Alignment::Stretch)
                .with_child(column)
                .with_child(dialog),
        ),
    );

    Application::new().window(WindowBuilder::new().title("Elevation review").root(root))
}

fn capture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("ui-artifacts")
        .join("elevation-review")
}

fn capture_theme(name: &str, theme: DefaultTheme) -> Result<PathBuf> {
    let app = TestApp::from_runtime(build_elevation_scene(theme).build()?)?;
    let window = app.main_window()?;
    let screenshot = window.capture_screenshot()?;

    let dir = capture_root();
    std::fs::create_dir_all(&dir)
        .map_err(|error| sui::Error::new(format!("failed to create {}: {error}", dir.display())))?;
    let path = dir.join(format!("elevated-widgets-{name}.png"));
    screenshot.write_png(&path)?;

    assert!(screenshot.width() > 0 && screenshot.height() > 0);
    Ok(path)
}

#[test]
fn captures_elevated_widgets_in_dark_and_light_themes() -> Result<()> {
    let dark = capture_theme("dark", DefaultTheme::dark())?;
    let light = capture_theme("light", DefaultTheme::light())?;

    assert!(dark.exists(), "dark capture PNG should exist");
    assert!(light.exists(), "light capture PNG should exist");

    // Surface the resolved absolute paths in the test log for the reviewer.
    println!("DARK_CAPTURE={}", dark.display());
    println!("LIGHT_CAPTURE={}", light.display());

    Ok(())
}
