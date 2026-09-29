//! The SUI widget book: every component and its variations on one scrolling
//! page, grouped by category, with a rail that jumps to and follows the
//! current component, a filter, and a theme switch.
//!
//! Stories live in [`stories`] and are enumerated through [`registry`]; the
//! page, the rail, search, visual artifacts, and tests all read that list.

#![forbid(unsafe_code)]

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;

use crate::app::DevThemeReader;
use crate::demo_support::default_theme_reader;
use crate::live_performance::LivePerformanceRoot;

mod page;
mod registry;
mod shell;
mod specimen;
mod stories;

#[cfg(all(feature = "artifacts", not(target_arch = "wasm32")))]
pub(crate) mod visual_artifacts;

#[cfg(all(feature = "artifacts", not(target_arch = "wasm32")))]
pub use visual_artifacts::write_visual_artifacts;

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str = "Every SUI component and its variations on one page";

/// Semantics name of the scrolling page.
pub const GALLERY_SCROLL_NAME: &str = "Widget book gallery";
pub const GALLERY_SCROLL_BAR_NAME: &str = "Widget book gallery vertical scroll bar";
pub const WIDGET_BOOK_SHELL_NAME: &str = "Widget book shell";
pub const WIDGET_BOOK_SEARCH_NAME: &str = "Filter components";
pub const WIDGET_BOOK_THEME_SWITCH_NAME: &str = "Widget book theme";
pub const WIDGET_BOOK_NAV_NAME: &str = "Widget book navigation";

/// Accessible label of the image story's first specimen.
pub const DEMO_IMAGE_LABEL: &str = "Preview image";
/// Name of the color swatch story's first swatch.
pub const COLOR_SWATCH_NAME: &str = "Primary swatch";
/// Name of the split view story's horizontal splitter.
pub const SPLIT_VIEW_NAME: &str = "Editor split";

const WIDGET_BOOK_IMAGE_HANDLE: ImageHandle = ImageHandle::new(1);

/// The theme the widget book renders its stories with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeChoice {
    /// Follow the host application's theme.
    App,
    Light,
    Dark,
    Neutral,
    NeutralDark,
    Void,
}

impl ThemeChoice {
    const STANDALONE: [Self; 5] = [
        Self::Light,
        Self::Dark,
        Self::Neutral,
        Self::NeutralDark,
        Self::Void,
    ];

    const EMBEDDED: [Self; 6] = [
        Self::App,
        Self::Light,
        Self::Dark,
        Self::Neutral,
        Self::NeutralDark,
        Self::Void,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::App => "App",
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Neutral => "Neutral",
            Self::NeutralDark => "Neutral dark",
            Self::Void => "Void",
        }
    }

    fn resolve(self, application: DefaultTheme) -> DefaultTheme {
        match self {
            Self::App => application,
            Self::Light => DefaultTheme::sui(),
            Self::Dark => DefaultTheme::dark(),
            Self::Neutral => DefaultTheme::neutral(),
            Self::NeutralDark => DefaultTheme::neutral_dark(),
            Self::Void => DefaultTheme::high_contrast(),
        }
    }
}

/// Filter and theme selection shared by the page, rail, and top bar.
pub(crate) struct BookState {
    pub(crate) query: String,
    pub(crate) theme: ThemeChoice,
}

/// Register the images used by the widget book onto an application. Call this
/// while configuring app resources when you assemble the application yourself
/// rather than using [`build_widget_book_application`].
pub fn register_widget_book_images(resources: &mut ResourceRegistry<'_>) {
    resources
        .image(
            WIDGET_BOOK_IMAGE_HANDLE,
            RegisteredImage::from_rgba8(72, 72, widget_book_demo_image_pixels())
                .expect("widget-book demo image is valid RGBA data"),
        )
        .expect("widget-book demo image handle should register exactly once");
}

pub fn build_widget_book_application() -> Application {
    App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })
        .expect("widget-book image resources should be valid")
        .window(Window::new(WINDOW_TITLE).root(LivePerformanceRoot::new(
            WINDOW_TITLE,
            WINDOW_DESCRIPTION,
            build_widget_book_gallery(),
        )))
        .into_application()
}

#[cfg(feature = "native")]
pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application().run()
}

/// The standalone widget book, starting on the light theme.
pub fn build_widget_book_gallery() -> impl Widget {
    build_book(
        default_theme_reader(),
        ThemeChoice::Light,
        &ThemeChoice::STANDALONE,
    )
}

/// The widget book embedded in a host that owns the application theme. The
/// theme switch gains an "App" choice that follows `theme_reader`.
pub fn build_widget_book_gallery_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    build_book(theme_reader, ThemeChoice::App, &ThemeChoice::EMBEDDED)
}

/// One story's block, as it appears on the page, in `theme`. Other demos use
/// this to show real components; the theme editor previews with it.
///
/// # Panics
///
/// If no story has the id `id`.
pub(crate) fn story_preview(id: &str, theme: DefaultTheme) -> impl Widget + use<> {
    let story = registry::story(id).unwrap_or_else(|| panic!("no widget book story {id:?}"));
    page::story_block(story, theme)
}

/// The accessible name of a story's block, such as "Button story".
#[cfg(test)]
pub(crate) fn story_region_name(id: &str) -> String {
    registry::story(id)
        .unwrap_or_else(|| panic!("no widget book story {id:?}"))
        .region_name()
}

fn build_book(
    application_theme: DevThemeReader,
    initial: ThemeChoice,
    choices: &'static [ThemeChoice],
) -> impl Widget {
    let state = Rc::new(RefCell::new(BookState {
        query: String::new(),
        theme: initial,
    }));
    let selection = Rc::clone(&state);
    let theme_reader: DevThemeReader =
        Rc::new(move || selection.borrow().theme.resolve(application_theme()));
    shell::build_shell(state, theme_reader, choices)
}

fn widget_book_demo_image_pixels() -> Vec<u8> {
    let width = 72usize;
    let height = 72usize;
    let mut pixels = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let mut red = if checker { 228 } else { 208 };
            let mut green = if checker { 236 } else { 216 };
            let mut blue = if checker { 248 } else { 228 };
            let alpha = 255u8;

            if x > 10 && x < 62 && y > 10 && y < 62 {
                red = 38 + ((x as f32 / width as f32) * 50.0) as u8;
                green = 108 + ((y as f32 / height as f32) * 60.0) as u8;
                blue = 190;
            }

            if (x > 18 && x < 54) && (y > 18 && y < 54) {
                red = 245;
                green = 248;
                blue = 252;
            }

            if (x > 28 && x < 44) && (y > 24 && y < 48) {
                red = 255;
                green = 168;
                blue = 60;
            }

            pixels[index] = red;
            pixels[index + 1] = green;
            pixels[index + 2] = blue;
            pixels[index + 3] = alpha;
        }
    }

    pixels
}

#[cfg(test)]
mod tests;
