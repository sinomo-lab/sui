//! Writes reviewable screenshots of the widget book: the page in each theme,
//! every registered story in light and dark, the Themes page, and an HDR
//! validation capture.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sui::{
    Error, Event, Rect, Result, SemanticsRole, Size, Vector, WindowColorManagementMode,
    WindowDynamicRangeMode, WindowEvent, WindowOutputColorPrimaries, WindowRenderOptions,
    WindowToneMappingMode, window_output_diagnostics,
};
use sui_render_wgpu::{
    DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest, DebugCaptureStage,
    DebugSdrVisualization,
};
use sui_testing::{Screenshot, TestApp, TestWindow};

use super::registry::{Story, stories};
use super::shell::RAIL_SCROLL_NAME;
use super::{
    WIDGET_BOOK_SEARCH_NAME, WIDGET_BOOK_THEME_SWITCH_NAME, build_widget_book_application,
};
use crate::hdr_validation::report::{
    final_output_sdr_white, output_diagnostics_report, write_capture_bundle,
};
use crate::theme_demo::build_theme_demo_application;
use crate::validation::{COLOR_VALIDATION_VIEW_TITLE, build_color_validation_application};

/// Window size for story captures: wide enough for the rail and every
/// specimen grid, tall enough that most stories fit in one screenshot.
pub(crate) const CAPTURE_SIZE: Size = Size::new(1440.0, 1600.0);

/// Below the rail breakpoint, for checking the single-column fallback.
const NARROW_CAPTURE_SIZE: Size = Size::new(720.0, 1400.0);

/// Themes captured for every story, by theme-switch label.
pub(crate) const CAPTURE_THEMES: [&str; 2] = ["Light", "Dark"];

pub fn write_visual_artifacts() -> Result<PathBuf> {
    let output_root = artifact_root();
    write_visual_artifacts_to(&output_root)
}

pub(crate) fn write_visual_artifacts_to(output_root: &Path) -> Result<PathBuf> {
    reset_dir(output_root)?;

    for theme in CAPTURE_THEMES {
        let app = widget_book_capture_app(theme)?;
        let window = app.main_window()?;

        let overview_dir = output_root.join(format!("overview-{}", theme.to_lowercase()));
        create_dir(&overview_dir)?;
        window.capture_artifacts()?.write_to_dir(&overview_dir)?;
        rename_window_artifacts(&overview_dir)?;

        for story in stories() {
            let story_dir = output_root.join("stories").join(story.id);
            create_dir(&story_dir)?;
            jump_to_story(&window, story)?;
            capture_story(&window, story)?
                .write_png(story_dir.join(format!("{}.png", theme.to_lowercase())))?;
            write_text(
                story_dir.join("story.txt"),
                &format!("{}\n{}\n{}\n", story.title, story.api, story.summary),
            )?;
        }
    }

    // Narrow windows hide the rail and collapse the theme switch. Each capture
    // app is dropped before the next one starts.
    {
        let narrow_dir = output_root.join("narrow-light");
        create_dir(&narrow_dir)?;
        let narrow = TestApp::new(|| build_widget_book_application().build())?;
        let window = narrow.main_window()?;
        window
            .root()
            .dispatch_event(Event::Window(WindowEvent::Resized(NARROW_CAPTURE_SIZE)))?;
        window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(WIDGET_BOOK_SEARCH_NAME)
            .fill("button")?;
        window.run_until_idle()?;
        window.capture_artifacts()?.write_to_dir(&narrow_dir)?;
        rename_window_artifacts(&narrow_dir)?;
    }

    {
        let themes_dir = output_root.join("themes-page");
        create_dir(&themes_dir)?;
        let themes = TestApp::new(|| build_theme_demo_application().build())?;
        themes
            .main_window()?
            .capture_artifacts()?
            .write_to_dir(&themes_dir)?;
        rename_window_artifacts(&themes_dir)?;
    }

    write_hdr_validation_artifacts(output_root)?;

    Ok(output_root.to_path_buf())
}

/// A widget book window sized for captures and switched to `theme`.
pub(crate) fn widget_book_capture_app(theme: &str) -> Result<TestApp> {
    let app = TestApp::new(|| build_widget_book_application().build())?;
    let window = app.main_window()?;
    window
        .root()
        .dispatch_event(Event::Window(WindowEvent::Resized(CAPTURE_SIZE)))?;
    window.run_until_idle()?;
    select_theme(&window, theme)?;
    Ok(app)
}

/// Clicks a segment of the widget book's theme switch.
pub(crate) fn select_theme(window: &TestWindow, label: &str) -> Result<()> {
    let snapshot = window.snapshot()?;
    let switch = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(WIDGET_BOOK_THEME_SWITCH_NAME))
        .ok_or_else(|| Error::new("widget book theme switch is missing"))?;
    let segment = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| {
            node.name.as_deref() == Some(label)
                && node.bounds.intersection(switch.bounds).is_some()
                && node.role != SemanticsRole::Text
        })
        .ok_or_else(|| Error::new(format!("theme segment {label} is missing")))?;
    window
        .get_by_role(segment.role.clone())
        .with_name(label)
        .click()?;
    window.run_until_idle()
}

/// Jumps to a story through its rail link, scrolling the rail first when the
/// link is outside it.
pub(crate) fn jump_to_story(window: &TestWindow, story: &Story) -> Result<()> {
    let snapshot = window.snapshot()?;
    let find = |role: SemanticsRole, name: &str| {
        snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .map(|node| node.bounds)
    };
    let link = find(SemanticsRole::Link, story.title)
        .ok_or_else(|| Error::new(format!("rail link for {} is missing", story.title)))?;
    let rail = find(SemanticsRole::ScrollView, RAIL_SCROLL_NAME)
        .ok_or_else(|| Error::new("widget book rail is missing"))?;
    if link.y() < rail.y() || link.max_y() > rail.max_y() {
        let delta = link.y() - (rail.y() + rail.height() * 0.5);
        window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(RAIL_SCROLL_NAME)
            .scroll_pixels(Vector::new(0.0, -delta))?;
        window.run_until_idle()?;
    }
    window
        .get_by_role(SemanticsRole::Link)
        .with_name(story.title)
        .click()?;
    window.run_until_idle()
}

/// Crops the window screenshot to a story's page block.
pub(crate) fn capture_story(window: &TestWindow, story: &Story) -> Result<Screenshot> {
    let snapshot = window.snapshot()?;
    let screenshot = window.capture_screenshot()?;
    let region = story.region_name();
    let bounds = snapshot
        .accessibility
        .nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(region.as_str()))
        .map(|node| node.bounds)
        .ok_or_else(|| Error::new(format!("story block {region} is missing")))?;
    let viewport = snapshot
        .scene_summary
        .map(|scene| scene.viewport)
        .unwrap_or(Size::new(
            screenshot.width() as f32,
            screenshot.height() as f32,
        ));
    let scale_x = screenshot.width() as f32 / viewport.width.max(1.0);
    let scale_y = screenshot.height() as f32 / viewport.height.max(1.0);
    let visible = bounds
        .intersection(Rect::from_origin_size(sui::Point::ZERO, viewport))
        .ok_or_else(|| Error::new(format!("story block {region} is off screen")))?;
    screenshot.crop(Rect::new(
        visible.x() * scale_x,
        visible.y() * scale_y,
        visible.width() * scale_x,
        visible.height() * scale_y,
    ))
}

fn hdr_render_options() -> WindowRenderOptions {
    WindowRenderOptions::new(true, 1.0)
        .with_color_management_mode(WindowColorManagementMode::PreferHdr)
        .with_output_color_primaries(WindowOutputColorPrimaries::DisplayP3)
        .with_dynamic_range_mode(WindowDynamicRangeMode::HighDynamicRange)
        .with_tone_mapping_mode(WindowToneMappingMode::Automatic)
}

fn write_hdr_validation_artifacts(output_root: &Path) -> Result<()> {
    let hdr_dir = output_root.join("hdr-validation");
    create_dir(&hdr_dir)?;

    let options = hdr_render_options();
    let runtime = build_color_validation_application().build()?;
    for window_id in runtime.window_ids() {
        sui::set_window_render_options(window_id, options);
    }
    let app = TestApp::from_runtime(runtime)?;
    let window = app.main_window()?;

    let artifacts = window.capture_artifacts()?;
    artifacts.write_to_dir(&hdr_dir)?;
    rename_window_artifacts(&hdr_dir)?;
    write_text(
        hdr_dir.join("story.txt"),
        "HDR-configured color validation surface with HDR debug captures.",
    )?;

    let capture = |stage| {
        window.capture_debug_frame(DebugCaptureRequest {
            stage,
            encoding: DebugCaptureEncoding::Exr,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        })
    };
    let DebugCaptureArtifact::HdrLinearRgbaF32(image) =
        capture(DebugCaptureStage::HdrIntermediate)?
    else {
        return Err(Error::new(
            "HDR artifact capture did not produce an HDR intermediate frame",
        ));
    };
    let final_output = capture(DebugCaptureStage::FinalComposed)?;
    let diagnostics = window_output_diagnostics(window.id());
    write_capture_bundle(
        &hdr_dir,
        &image,
        &final_output,
        final_output_sdr_white(diagnostics.as_ref()),
        &output_diagnostics_report(COLOR_VALIDATION_VIEW_TITLE, diagnostics.as_ref()),
        true,
    )?;
    Ok(())
}

pub(crate) fn artifact_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("ui-artifacts")
        .join("sui-demo")
        .join("widget-book")
}

fn reset_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| Error::new(format!("failed to clear {}: {error}", path.display())))?;
    }
    create_dir(path)
}

fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| Error::new(format!("failed to create {}: {error}", path.display())))
}

fn write_text(path: PathBuf, contents: &str) -> Result<()> {
    fs::write(&path, contents)
        .map_err(|error| Error::new(format!("failed to write {}: {error}", path.display())))
}

fn rename_window_artifacts(dir: &Path) -> Result<()> {
    rename_if_exists(dir, "screenshot.png", "window.png")?;
    rename_if_exists(dir, "semantics-overlay.png", "window-semantics-overlay.png")?;
    rename_if_exists(dir, "widget-overlay.png", "window-widget-overlay.png")
}

fn rename_if_exists(dir: &Path, from: &str, to: &str) -> Result<()> {
    let from_path = dir.join(from);
    if !from_path.exists() {
        return Ok(());
    }

    let to_path = dir.join(to);
    if to_path.exists() {
        fs::remove_file(&to_path).map_err(|error| {
            Error::new(format!("failed to remove {}: {error}", to_path.display()))
        })?;
    }

    fs::rename(&from_path, &to_path)
        .map_err(|error| Error::new(format!("failed to rename {}: {error}", from_path.display())))
}
