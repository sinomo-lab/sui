//! The single ordered list of widget book stories. The page, the navigation
//! rail, search, visual artifacts, and tests all enumerate this registry, so
//! rail order always matches page order.

use sui::prelude::*;

use super::specimen::Section;
use super::stories;
use crate::app::{DemoTextRole, demo_mono_text_style, demo_text_style};

/// Page sections, in page order. Each category owns one decorative hue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Category {
    Actions,
    Selection,
    TextEntry,
    Navigation,
    Overlays,
    Feedback,
    Data,
    Layout,
    Media,
}

impl Category {
    pub(crate) const ALL: [Self; 9] = [
        Self::Actions,
        Self::Selection,
        Self::TextEntry,
        Self::Navigation,
        Self::Overlays,
        Self::Feedback,
        Self::Data,
        Self::Layout,
        Self::Media,
    ];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Actions => "Actions",
            Self::Selection => "Selection",
            Self::TextEntry => "Text entry",
            Self::Navigation => "Navigation",
            Self::Overlays => "Menus and overlays",
            Self::Feedback => "Feedback and status",
            Self::Data => "Data views",
            Self::Layout => "Layout and surfaces",
            Self::Media => "Text, color, and media",
        }
    }

    pub(crate) const fn summary(self) -> &'static str {
        match self {
            Self::Actions => "Buttons and command surfaces that trigger work.",
            Self::Selection => "Boolean, exclusive, and ranged choices.",
            Self::TextEntry => "Fields for typed, numeric, and chosen values.",
            Self::Navigation => "Tabs and paths that move between views.",
            Self::Overlays => "Menus, tooltips, popovers, dialogs, and notifications.",
            Self::Feedback => "Progress, status, and empty states.",
            Self::Data => "Lists, trees, and tables for collections.",
            Self::Layout => "Surfaces, grouping, and pane layouts for app shells.",
            Self::Media => "Typography, icons, color tools, images, and canvases.",
        }
    }

    pub(crate) const fn hue(self) -> DecorativeHue {
        match self {
            Self::Actions => DecorativeHue::Blue,
            Self::Selection => DecorativeHue::Violet,
            Self::TextEntry => DecorativeHue::Cyan,
            Self::Navigation => DecorativeHue::Teal,
            Self::Overlays => DecorativeHue::Magenta,
            Self::Feedback => DecorativeHue::Amber,
            Self::Data => DecorativeHue::Green,
            Self::Layout => DecorativeHue::Orange,
            Self::Media => DecorativeHue::Red,
        }
    }
}

/// Builds a story's specimen sections for one resolved theme.
pub(crate) type StoryBuilder = fn(&StoryCtx) -> Vec<Section>;

/// One component entry: its metadata and the specimens that show it.
pub(crate) struct Story {
    /// Stable identifier used by tests and artifact folders.
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    /// The public type path shown next to the title.
    pub(crate) api: &'static str,
    pub(crate) summary: &'static str,
    /// Extra search terms, such as aliases and related concepts.
    pub(crate) keywords: &'static str,
    pub(crate) category: Category,
    pub(crate) build: StoryBuilder,
}

impl Story {
    /// Semantics name of the story's page block.
    pub(crate) fn region_name(&self) -> String {
        format!("{} story", self.title)
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        [
            self.title,
            self.api,
            self.summary,
            self.keywords,
            self.category.title(),
        ]
        .iter()
        .any(|field| field.to_lowercase().contains(&query))
    }
}

/// Every story, in page order.
pub(crate) fn stories() -> &'static [Story] {
    stories::all()
}

pub(crate) fn stories_in(category: Category) -> impl Iterator<Item = &'static Story> {
    stories()
        .iter()
        .filter(move |story| story.category == category)
}

#[cfg(test)]
pub(crate) fn story(id: &str) -> Option<&'static Story> {
    stories().iter().find(|story| story.id == id)
}

/// The theme and text helpers a story builds its specimens with. Story
/// blocks are rebuilt when the book's theme changes, so specimens take the
/// resolved theme directly.
#[derive(Clone, Copy)]
pub(crate) struct StoryCtx {
    pub(crate) theme: DefaultTheme,
}

impl StoryCtx {
    pub(crate) fn new(theme: DefaultTheme) -> Self {
        Self { theme }
    }

    /// The theme resized to a control size preset.
    pub(crate) fn sized(&self, size: ControlSize) -> DefaultTheme {
        self.theme.with_size(size)
    }

    pub(crate) fn text(&self, text: impl Into<String>) -> Label {
        Label::new(text).style(demo_text_style(
            self.theme,
            DemoTextRole::Body,
            self.theme.palette.text,
        ))
    }

    pub(crate) fn muted(&self, text: impl Into<String>) -> Label {
        Label::new(text).style(demo_text_style(
            self.theme,
            DemoTextRole::Supporting,
            self.theme.palette.text_muted,
        ))
    }

    pub(crate) fn styled(&self, text: impl Into<String>, role: DemoTextRole) -> Label {
        Label::new(text).style(demo_text_style(self.theme, role, self.theme.palette.text))
    }

    pub(crate) fn mono(&self, text: impl Into<String>) -> Label {
        Label::new(text).style(demo_mono_text_style(
            self.theme,
            DemoTextRole::Supporting,
            self.theme.palette.text,
        ))
    }
}
