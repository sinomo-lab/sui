//! The Text rendering page: the window's text settings beside samples drawn
//! with them, two render policies side by side and under a magnifier, and
//! probes that say what each setting should do.

#![forbid(unsafe_code)]

mod magnifier;
mod samples;
#[cfg(test)]
mod tests;

use std::{cell::Cell, rc::Rc};

use sui::Application;
use sui::prelude::*;
use sui::{
    FlexItem, TextRenderCoveragePolicy, TextRenderHinting, TextRenderMode, TextRenderPolicy,
    TextRenderStemDarkening, TextSubpixelOrder,
};

use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;
use crate::settings::controls::{Place, labeled_control};
use crate::settings::{RenderOptions, RenderOptionsScope, text_controls};
#[cfg(test)]
pub(crate) use magnifier::{MAGNIFIER_NAME, MAGNIFY_LABEL};
use magnifier::{Magnifier, Source};
use samples::{
    BLUE, CenteringBoxes, DARK, LIGHT, PolicyPair, PolicySource, Specimen, SubpixelDrift, Surface,
};

pub const TEXT_RENDERING_VIEW_TITLE: &str = "SUI Text Rendering";
pub const TEXT_RENDERING_SCROLL_NAME: &str = "Text rendering scroll";

pub(crate) const SETTINGS_SECTION_NAME: &str = "Window text settings";
pub(crate) const SIZES_SECTION_NAME: &str = "Sizes and backgrounds";
pub(crate) const COMPARE_SECTION_NAME: &str = "Compare two policies";
pub(crate) const PROBES_SECTION_NAME: &str = "What to look for";
pub(crate) const POLICIES_SECTION_NAME: &str = "Per-text policies";
pub(crate) const SIZES_SPECIMEN_NAME: &str = "Specimen with the window's settings";
pub(crate) const POLICY_A_NAME: &str = "Policy A";
pub(crate) const POLICY_B_NAME: &str = "Policy B";
pub(crate) const SPECIMEN_A_NAME: &str = "Specimen A";
pub(crate) const SPECIMEN_B_NAME: &str = "Specimen B";

const SPECIMEN_SIZES: &[f32] = &[10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 20.0];
const SPECIMEN_SURFACES: &[Surface] = &[LIGHT, DARK, BLUE];
const COMPARE_SIZES: &[f32] = &[11.0, 13.0, 16.0];
const COMPARE_SURFACES: &[Surface] = &[LIGHT, DARK];
const CONTROLS_MAX_WIDTH: f32 = 560.0;
const SELECT_WIDTH: f32 = 280.0;

/// A text render policy to compare, and the code that sets it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NamedPolicy {
    pub(crate) name: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) code: &'static str,
    /// `None` draws with the window's settings.
    pub(crate) policy: Option<TextRenderPolicy>,
}

const SMALL_TEXT_DARKENING: TextRenderStemDarkening = TextRenderStemDarkening::Enabled {
    max_ppem: 18.0,
    amount: 0.20,
};

pub(crate) const POLICIES: [NamedPolicy; 10] = [
    NamedPolicy {
        name: "Window settings",
        summary: "No policy of its own: whatever the settings above say.",
        code: "// No override: the window's render options apply.",
        policy: None,
    },
    NamedPolicy {
        name: "Linear coverage",
        summary: "Coverage as sampled, without perceptual compensation. Light text on dark looks thin.",
        code: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Linear)",
        policy: Some(
            TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Linear),
        ),
    },
    NamedPolicy {
        name: "Perceptual coverage",
        summary: "SUI's default: coverage compensated for the text and background colors.",
        code: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)",
        policy: Some(
            TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
        ),
    },
    NamedPolicy {
        name: "Perceptual, no hinting",
        summary: "The default without snapping small text to the pixel grid, for free positioning.",
        code: "TextRenderPolicy::new()\n    .with_hinting(TextRenderHinting::None)\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)",
        policy: Some(
            TextRenderPolicy::new()
                .with_hinting(TextRenderHinting::None)
                .with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
        ),
    },
    NamedPolicy {
        name: "LCD subpixel",
        summary: "Separate coverage per color channel for pixel-aligned text on RGB panels.",
        code: "TextRenderPolicy::new()\n    .with_render_mode(TextRenderMode::LcdSubpixel)\n    .with_subpixel_order(TextSubpixelOrder::Rgb)\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)",
        policy: Some(
            TextRenderPolicy::new()
                .with_render_mode(TextRenderMode::LcdSubpixel)
                .with_subpixel_order(TextSubpixelOrder::Rgb)
                .with_coverage_policy(TextRenderCoveragePolicy::Perceptual),
        ),
    },
    NamedPolicy {
        name: "Gamma 1.8",
        summary: "A diagnostic curve that lightens partly covered edge pixels.",
        code: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Gamma(1.8))",
        policy: Some(
            TextRenderPolicy::new().with_coverage_policy(TextRenderCoveragePolicy::Gamma(1.8)),
        ),
    },
    NamedPolicy {
        name: "Coverage boost 0.5",
        summary: "A fixed boost that does not depend on the colors.",
        code: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::CoverageBoost(0.5))",
        policy: Some(
            TextRenderPolicy::new()
                .with_coverage_policy(TextRenderCoveragePolicy::CoverageBoost(0.5)),
        ),
    },
    NamedPolicy {
        name: "2c − c²",
        summary: "The strongest built-in boost, for deliberately heavy small text.",
        code: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq)",
        policy: Some(
            TextRenderPolicy::new()
                .with_coverage_policy(TextRenderCoveragePolicy::TwoCoverageMinusCoverageSq),
        ),
    },
    NamedPolicy {
        name: "Perceptual + stem darkening",
        summary: "The default plus extra stem weight below 18 ppem, without changing layout.",
        code: "TextRenderPolicy::new()\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)\n    .with_stem_darkening(TextRenderStemDarkening::Enabled { max_ppem: 18.0, amount: 0.20 })",
        policy: Some(
            TextRenderPolicy::new()
                .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)
                .with_stem_darkening(SMALL_TEXT_DARKENING),
        ),
    },
    NamedPolicy {
        name: "LCD + stem darkening",
        summary: "Subpixel rendering with extra small-text stem weight.",
        code: "TextRenderPolicy::new()\n    .with_render_mode(TextRenderMode::LcdSubpixel)\n    .with_subpixel_order(TextSubpixelOrder::Rgb)\n    .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)\n    .with_stem_darkening(TextRenderStemDarkening::Enabled { max_ppem: 18.0, amount: 0.20 })",
        policy: Some(
            TextRenderPolicy::new()
                .with_render_mode(TextRenderMode::LcdSubpixel)
                .with_subpixel_order(TextSubpixelOrder::Rgb)
                .with_coverage_policy(TextRenderCoveragePolicy::Perceptual)
                .with_stem_darkening(SMALL_TEXT_DARKENING),
        ),
    },
];

/// Which of [`POLICIES`] each side of the comparison starts with.
const POLICY_A: usize = 1;
const POLICY_B: usize = 2;

/// The page for a window of its own: its settings start as the window's.
pub fn build_text_rendering_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    build_text_rendering_page(theme_reader, RenderOptions::from_window())
}

pub fn build_text_rendering_application() -> Application {
    App::new()
        .window(Window::new(TEXT_RENDERING_VIEW_TITLE).root(LivePerformanceRoot::new(
            TEXT_RENDERING_VIEW_TITLE,
            "The window's text settings beside samples drawn with them, and render policies compared under a magnifier.",
            build_text_rendering_surface_with_theme(default_theme_reader()),
        )))
        .into_application()
}

/// The page, editing `options`.
pub(crate) fn build_text_rendering_page(
    theme_reader: DevThemeReader,
    options: RenderOptions,
) -> impl Widget {
    let content = Stack::vertical()
        .gap(18.0)
        .alignment(Alignment::Stretch)
        .with_child(settings_section(&theme_reader, &options))
        .with_child(sizes_section(&theme_reader))
        .with_child(compare_section(&theme_reader))
        .with_child(probes_section(&theme_reader))
        .with_child(policies_section(&theme_reader));
    RenderOptionsScope::new(
        options,
        ScrollView::vertical(Padding::all(24.0, content))
            .name(TEXT_RENDERING_SCROLL_NAME)
            .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
}

fn section<W>(
    theme_reader: &DevThemeReader,
    name: &'static str,
    subtitle: &'static str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    NamedSection::new(
        name,
        panel_with_theme(Rc::clone(theme_reader), name, subtitle, body),
    )
}

fn body() -> Stack {
    Stack::vertical().gap(12.0).alignment(Alignment::Stretch)
}

fn note(theme_reader: &DevThemeReader, text: &'static str) -> impl Widget + use<> {
    MaximumWidth::new(
        GALLERY_TEXT_MAX_WIDTH,
        demo_label(theme_reader, text, DemoTextRole::Body, DemoTextColor::Muted),
    )
}

fn settings_section(theme_reader: &DevThemeReader, options: &RenderOptions) -> impl Widget + use<> {
    section(
        theme_reader,
        SETTINGS_SECTION_NAME,
        "How the window draws text. Every sample on this page without a policy of its own follows these, and Settings edits the same options.",
        MaximumWidth::new(
            CONTROLS_MAX_WIDTH,
            text_controls(theme_reader, options, Place::Page),
        ),
    )
}

fn sizes_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        SIZES_SECTION_NAME,
        "The same sample from 10 to 20 px on light, dark, and colored backgrounds, drawn with the settings above.",
        body()
            .with_child(Specimen::new(SIZES_SPECIMEN_NAME, SPECIMEN_SIZES, SPECIMEN_SURFACES))
            .with_child(note(
                theme_reader,
                "Every row should look even in weight and sharp, on every background. Small sizes are the ones settings change most: watch 10 to 13 px as you change them.",
            )),
    )
}

/// A select choosing one of [`POLICIES`] for one side of the comparison.
fn policy_select(
    theme_reader: &DevThemeReader,
    name: &'static str,
    choice: &Signal<usize>,
) -> Select {
    let read = choice.clone();
    let write = choice.clone();
    Select::new(name)
        .theme_when(clone_dev_theme_reader(theme_reader))
        .options(POLICIES.map(|policy| policy.name))
        .selected_when(move || Some(read.get()))
        .on_change(move |index, _| {
            write.set(index);
        })
}

/// A side of the comparison: its select, its specimen, and what its policy
/// does.
fn comparison_side(
    theme_reader: &DevThemeReader,
    select_name: &'static str,
    specimen_name: &'static str,
    choice: &Signal<usize>,
    zoom: samples::ZoomRegion,
) -> impl Widget + use<> {
    let summary = Selector::new(
        format!("{select_name} summary"),
        choice.clone(),
        |index: &usize| {
            POLICIES
                .get(*index)
                .map_or("", |policy| policy.summary)
                .to_string()
        },
    );
    let code = Selector::new(
        format!("{select_name} code"),
        choice.clone(),
        |index: &usize| {
            POLICIES
                .get(*index)
                .map_or("", |policy| policy.code)
                .to_string()
        },
    );
    Stack::vertical()
        .gap(10.0)
        .alignment(Alignment::Stretch)
        .with_child(labeled_control(
            theme_reader,
            select_name,
            SELECT_WIDTH,
            policy_select(theme_reader, select_name, choice),
        ))
        .with_child(
            Specimen::new(specimen_name, COMPARE_SIZES, COMPARE_SURFACES)
                .policy(PolicySource::Chosen(choice.clone()))
                .zoom(zoom),
        )
        .with_child(
            demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Muted)
                .text_from(summary),
        )
        .with_child(
            demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
                theme.palette.text
            })
            .text_from(code),
        )
}

fn compare_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let choice_a = Signal::named(POLICY_A_NAME, POLICY_A);
    let choice_b = Signal::named(POLICY_B_NAME, POLICY_B);
    let zoom_a = Rc::new(Cell::new(None));
    let zoom_b = Rc::new(Cell::new(None));
    let caption = |label: &'static str, choice: &Signal<usize>| -> Box<dyn Fn() -> String> {
        let choice = choice.clone();
        Box::new(move || {
            let name = POLICIES.get(choice.get()).map_or("", |policy| policy.name);
            format!("{label}: {name}")
        })
    };
    let magnifier = Magnifier::new(
        theme_reader,
        vec![
            Source {
                region: Rc::clone(&zoom_a),
                caption: caption("A", &choice_a),
            },
            Source {
                region: Rc::clone(&zoom_b),
                caption: caption("B", &choice_b),
            },
        ],
    );
    section(
        theme_reader,
        COMPARE_SECTION_NAME,
        "Text can carry a render policy of its own. Pick two, see them side by side, and magnify the marked corners to see what 1× hides.",
        body()
            .with_child(
                // Each side takes half, however long its text.
                Flex::horizontal()
                    .gap(20.0)
                    .align_items(Alignment::Start)
                    .with_item(
                        comparison_side(
                            theme_reader,
                            POLICY_A_NAME,
                            SPECIMEN_A_NAME,
                            &choice_a,
                            zoom_a,
                        ),
                        FlexItem::fill(),
                    )
                    .with_item(
                        comparison_side(
                            theme_reader,
                            POLICY_B_NAME,
                            SPECIMEN_B_NAME,
                            &choice_b,
                            zoom_b,
                        ),
                        FlexItem::fill(),
                    ),
            )
            .with_child(magnifier),
    )
}

/// A probe: what it shows, the sample, and what to look for.
fn probe<W>(
    theme_reader: &DevThemeReader,
    title: &'static str,
    sample: W,
    expectation: &'static str,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    Stack::vertical()
        .gap(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            title,
            DemoTextRole::Emphasis,
            DemoTextColor::Text,
        ))
        .with_child(sample)
        .with_child(note(theme_reader, expectation))
}

fn probes_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let caption_reader = Rc::clone(theme_reader);
    let caption: Rc<dyn Fn() -> Color> = Rc::new(move || caption_reader().palette.text_muted);
    let fixed = |index: usize| PolicySource::Fixed(POLICIES[index].policy.unwrap_or_default());
    section(
        theme_reader,
        PROBES_SECTION_NAME,
        "Each probe isolates one setting, draws it both ways, and says what should differ.",
        body()
            .gap(22.0)
            .with_child(probe(
                theme_reader,
                "Stem darkening thickens small text only",
                PolicyPair::new(
                    "Stem darkening probe",
                    ("Perceptual", fixed(2)),
                    ("Perceptual + stem darkening", fixed(8)),
                    &[11.0, 24.0],
                    LIGHT,
                    Rc::clone(&caption),
                ),
                "At 11 px the right side is a little bolder. At 24 px, above the 18 ppem limit, both sides look the same.",
            ))
            .with_child(probe(
                theme_reader,
                "Hinting keeps small text sharp",
                PolicyPair::new(
                    "Hinting probe",
                    ("No hinting", fixed(3)),
                    ("Slight hinting", fixed(2)),
                    &[11.0, 12.0],
                    LIGHT,
                    Rc::clone(&caption),
                ),
                "With hinting, horizontal strokes such as the tops of x-height letters land on whole pixels and look crisper. Without it they can blur across two pixel rows. Magnify a comparison of the same two policies to see it clearly.",
            ))
            .with_child(probe(
                theme_reader,
                "Light text on dark keeps its weight",
                PolicyPair::new(
                    "Coverage on dark probe",
                    ("Linear coverage", fixed(1)),
                    ("Perceptual coverage", fixed(2)),
                    &[12.0, 14.0],
                    DARK,
                    Rc::clone(&caption),
                ),
                "Linear coverage makes light text on a dark background look thinner than dark text on light. Perceptual coverage compensates, so the right side matches the weight of the light specimens above.",
            ))
            .with_child(probe(
                theme_reader,
                "Positions between pixels",
                SubpixelDrift::new("Subpixel position probe", LIGHT),
                "Each copy is a quarter pixel further right. All four should look equally sharp and bold: glyphs are rasterized at quarter-pixel positions, so only their place changes.",
            ))
            .with_child(probe(
                theme_reader,
                "Optical vertical centering",
                CenteringBoxes::new("Optical centering probe", LIGHT),
                "The pink line is each box's center. With optical vertical centering on, capital letters are centered on it. With it off, the line box is centered instead, so text sits slightly low. Toggle it in the settings above.",
            )),
    )
}

fn policies_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let mut rows = Stack::vertical().gap(14.0).alignment(Alignment::Stretch);
    for policy in POLICIES.iter().filter(|policy| policy.policy.is_some()) {
        rows = rows.with_child(
            Stack::vertical()
                .gap(4.0)
                .alignment(Alignment::Stretch)
                .with_child(demo_label(
                    theme_reader,
                    policy.name,
                    DemoTextRole::Emphasis,
                    DemoTextColor::Text,
                ))
                .with_child(note(theme_reader, policy.summary))
                .with_child(demo_mono_label(
                    theme_reader,
                    policy.code,
                    DemoTextRole::Metadata,
                    |theme| theme.palette.text,
                )),
        );
    }
    section(
        theme_reader,
        POLICIES_SECTION_NAME,
        "Push a policy while painting to draw text differently from the window's settings, as the comparison above does: ctx.push_text_render_policy(policy), then pop_text_render_policy().",
        rows,
    )
}
