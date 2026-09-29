//! The theme editor demo. Color tokens sit on the left, grouped by job with
//! their contrast shown; pressing one opens an OKLCH, RGB, or HSL editor
//! beneath it. The right side previews the edited theme across surfaces,
//! decorative hues, motion, and widget book stories, all at once. The theme
//! can be copied as Rust or applied to the whole demo app.

mod contrast;
mod export;
mod motion;
mod panel;
mod preview;
mod state;
mod token_row;
mod tokens;

#[cfg(test)]
mod tests;

use std::rc::Rc;

use sui::prelude::*;

use self::panel::build_panel;
use self::preview::build_preview;
use self::state::ThemeEditorState;
use crate::app::{DevAppTheme, DevThemeReader, clone_dev_theme_reader, dev_theme_color};

pub(crate) const THEME_EDITOR_TAB_LABEL: &str = "Theme editor";
pub(crate) const THEME_EDITOR_CONTROLS_SCROLL_NAME: &str = "Theme editor tokens";
pub(crate) const THEME_EDITOR_PREVIEW_SCROLL_NAME: &str = "Theme editor preview";
pub(crate) const THEME_COLOR_PICKER_NAME: &str = "Token color picker";
pub(crate) const THEME_HEX_NAME: &str = "Token hex value";
pub(crate) const THEME_COLOR_MODEL_NAME: &str = "Color model";
pub(crate) const THEME_PRESET_NAME: &str = "Theme preset";
pub(crate) const THEME_CONTROL_SIZE_NAME: &str = "Control size";
pub(crate) const THEME_OVERRIDE_ROLE_NAME: &str = "Override a role";
pub(crate) const THEME_SPACING_NAME: &str = "Base spacing";
pub(crate) const THEME_RADIUS_SCALE_NAME: &str = "Corner radius scale";
pub(crate) const THEME_TEXT_SCALE_NAME: &str = "Type size scale";
pub(crate) const THEME_MOTION_SCALE_NAME: &str = "Motion duration scale";
pub(crate) const THEME_RESET_ALL_NAME: &str = "Reset all";
pub(crate) const THEME_COPY_RUST_NAME: &str = "Copy as Rust";
pub(crate) const THEME_USE_AS_APP_THEME_NAME: &str = "Use as app theme";

/// The editor, styled by the shell theme. `app_theme` enables "Use as app
/// theme"; without it the button is hidden.
pub(crate) fn build_theme_editor_demo(
    shell: DevThemeReader,
    app_theme: Option<DevAppTheme>,
) -> impl Widget {
    build_theme_editor(ThemeEditorState::new(), shell, app_theme)
}

fn build_theme_editor(
    state: ThemeEditorState,
    shell: DevThemeReader,
    app_theme: Option<DevAppTheme>,
) -> impl Widget {
    Background::new(
        shell().palette.surface,
        SplitView::horizontal(
            build_panel(state.clone(), Rc::clone(&shell), app_theme),
            build_preview(state),
        )
        .name("Theme editor workspace")
        .theme_when(clone_dev_theme_reader(&shell))
        .ratio(0.36)
        .min_first(400.0)
        .min_second(520.0),
    )
    .brush_when(dev_theme_color(&shell, |theme| theme.palette.surface))
}
