//! Story definitions, one module per category. [`ALL`] fixes the page order.

use std::sync::OnceLock;

use sui::{InteractionPreview, SemanticTone};

use super::registry::Story;

mod actions;
mod data;
mod feedback;
mod layout;
mod media;
mod navigation;
mod overlays;
mod selection;
mod text_entry;

static ALL_STORIES: OnceLock<Vec<Story>> = OnceLock::new();

/// Every story in page order: categories in [`super::registry::Category::ALL`]
/// order, stories in their module's order.
pub(crate) fn all() -> &'static [Story] {
    ALL_STORIES.get_or_init(|| {
        actions::STORIES
            .into_iter()
            .chain(selection::STORIES)
            .chain(text_entry::STORIES)
            .chain(navigation::STORIES)
            .chain(overlays::STORIES)
            .chain(feedback::STORIES)
            .chain(data::STORIES)
            .chain(layout::STORIES)
            .chain(media::STORIES)
            .collect()
    })
}

/// Interaction states compared across every stateful control.
pub(super) const PREVIEW_STATES: [InteractionPreview; 4] = [
    InteractionPreview::None,
    InteractionPreview::Hovered,
    InteractionPreview::Pressed,
    InteractionPreview::Focused,
];

pub(super) const fn preview_label(preview: InteractionPreview) -> &'static str {
    match preview {
        InteractionPreview::None => "Rest",
        InteractionPreview::Hovered => "Hovered",
        InteractionPreview::Pressed => "Pressed",
        InteractionPreview::Focused => "Focused",
    }
}

pub(super) const TONES: [SemanticTone; 6] = [
    SemanticTone::Neutral,
    SemanticTone::Accent,
    SemanticTone::Info,
    SemanticTone::Success,
    SemanticTone::Warning,
    SemanticTone::Danger,
];

pub(super) const fn tone_label(tone: SemanticTone) -> &'static str {
    match tone {
        SemanticTone::Neutral => "Neutral",
        SemanticTone::Accent => "Accent",
        SemanticTone::Info => "Info",
        SemanticTone::Success => "Success",
        SemanticTone::Warning => "Warning",
        SemanticTone::Danger => "Danger",
    }
}

/// Fixed-size box so specimens with stretchy widgets keep a compact width.
pub(super) fn sized<W>(width: f32, child: W) -> SizedBox
where
    W: sui::Widget + 'static,
{
    sui::SizedBox::new().width(width).with_child(child)
}

use sui::SizedBox;
