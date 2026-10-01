use sui::prelude::*;
use sui::{PlacementBadge, SemanticTone, SignalMeter, StatusBadge};

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{BoxedWidget, Section, boxed, example, grid, strip};
use super::{TONES, sized, tone_label};

pub(super) const STORIES: [Story; 7] = [
    Story {
        id: "progress-bar",
        title: "Progress bar",
        api: "ProgressBar",
        summary: "Determinate progress with an optional value label that stays readable over the fill.",
        keywords: "progress loading percent export",
        category: Category::Feedback,
        build: progress_bar,
    },
    Story {
        id: "spinner",
        title: "Spinner and busy indicator",
        api: "Spinner",
        summary: "Indeterminate activity for work without a known duration.",
        keywords: "loading busy activity indeterminate busyindicator",
        category: Category::Feedback,
        build: spinner,
    },
    Story {
        id: "status-badge",
        title: "Status badge",
        api: "StatusBadge",
        summary: "A compact semantic label for state, optionally with an icon.",
        keywords: "badge pill tag chip state",
        category: Category::Feedback,
        build: status_badge,
    },
    Story {
        id: "placement-badge",
        title: "Placement badge and coverage",
        api: "PlacementBadge, CoverageDots",
        summary: "Where an item lives and how many replicas are in place.",
        keywords: "replica coverage dots placement cluster",
        category: Category::Feedback,
        build: placement_badge,
    },
    Story {
        id: "signal-meter",
        title: "Signal meter",
        api: "SignalMeter",
        summary: "Live level bars for audio input, network quality, and similar signals.",
        keywords: "level audio meter bars",
        category: Category::Feedback,
        build: signal_meter,
    },
    Story {
        id: "status-bar",
        title: "Status bar",
        api: "StatusBar, StatusBarSegment",
        summary: "A window-bottom strip of status segments with semantic tones.",
        keywords: "footer segments status line",
        category: Category::Feedback,
        build: status_bar,
    },
    Story {
        id: "empty-state",
        title: "Empty state",
        api: "EmptyState",
        summary: "Explains an empty view and offers the next step.",
        keywords: "no results placeholder blank",
        category: Category::Feedback,
        build: empty_state,
    },
];

fn progress_bar(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let values = [0.0, 35.0, 72.0, 100.0];
    let tones = [
        SemanticTone::Accent,
        SemanticTone::Success,
        SemanticTone::Warning,
        SemanticTone::Danger,
    ];
    let rows = tones
        .iter()
        .map(|tone| {
            let row: Vec<BoxedWidget> = values
                .iter()
                .map(|value| {
                    boxed(sized(
                        112.0,
                        ProgressBar::new(format!("{} progress", tone_label(*tone)))
                            .range(0.0, 100.0)
                            .value(*value)
                            .show_value(true)
                            .tone(*tone)
                            .theme(theme),
                    ))
                })
                .collect();
            (tone_label(*tone), row)
        })
        .collect();
    vec![
        grid(
            theme,
            "Tones and values",
            &["0%", "35%", "72%", "100%"],
            rows,
        ),
        strip(
            theme,
            "Without value",
            vec![
                (
                    "Thin",
                    boxed(sized(
                        200.0,
                        ProgressBar::new("Upload progress")
                            .range(0.0, 100.0)
                            .value(60.0)
                            .theme(theme),
                    )),
                ),
                (
                    "Custom height",
                    boxed(sized(
                        200.0,
                        ProgressBar::new("Render progress")
                            .range(0.0, 100.0)
                            .value(40.0)
                            .height(10.0)
                            .theme(theme),
                    )),
                ),
            ],
        ),
    ]
}

fn spinner(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![strip(
        theme,
        "",
        vec![
            (
                "Small",
                boxed(Spinner::new("Loading small").size(16.0).theme(theme)),
            ),
            ("Default", boxed(Spinner::new("Loading").theme(theme))),
            (
                "With label",
                boxed(
                    Spinner::new("Syncing")
                        .label("Syncing library")
                        .theme(theme),
                ),
            ),
            (
                "Busy indicator",
                boxed(
                    Spinner::new("Indexing busy indicator")
                        .label("Indexing assets")
                        .theme(theme),
                ),
            ),
        ],
    )]
}

fn status_badge(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let icons = [
        IconGlyph::Hourglass,
        IconGlyph::Sparkles,
        IconGlyph::Alert,
        IconGlyph::Check,
        IconGlyph::Alert,
        IconGlyph::Close,
    ];
    let labels = ["Queued", "New", "Syncing", "Synced", "Degraded", "Failed"];
    let plain = TONES
        .iter()
        .zip(labels)
        .map(|(tone, label)| boxed(StatusBadge::new(label).tone(*tone).theme(theme)))
        .collect();
    let with_icon = TONES
        .iter()
        .zip(labels)
        .zip(icons)
        .map(|((tone, label), icon)| {
            boxed(StatusBadge::new(label).icon(icon).tone(*tone).theme(theme))
        })
        .collect();
    let columns: Vec<&str> = TONES.iter().map(|tone| tone_label(*tone)).collect();
    vec![grid(
        theme,
        "Tones",
        &columns,
        vec![("Label", plain), ("Icon", with_icon)],
    )]
}

fn placement_badge(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![
        strip(
            theme,
            "Placement badge",
            vec![
                (
                    "Info",
                    boxed(
                        PlacementBadge::new("Cluster")
                            .icon(IconGlyph::Storage)
                            .tone(SemanticTone::Info)
                            .coverage(2, 3)
                            .theme(theme),
                    ),
                ),
                (
                    "Complete",
                    boxed(
                        PlacementBadge::new("Region")
                            .icon(IconGlyph::Monitor)
                            .tone(SemanticTone::Success)
                            .coverage(3, 3)
                            .theme(theme),
                    ),
                ),
                (
                    "Label only",
                    boxed(PlacementBadge::new("Local cache").theme(theme)),
                ),
            ],
        ),
        strip(
            theme,
            "Coverage dots",
            vec![
                (
                    "Partial",
                    boxed(
                        CoverageDots::new("Replica coverage", 3, 4)
                            .tone(SemanticTone::Accent)
                            .theme(theme),
                    ),
                ),
                (
                    "Complete",
                    boxed(
                        CoverageDots::new("Complete coverage", 4, 4)
                            .tone(SemanticTone::Success)
                            .theme(theme),
                    ),
                ),
                (
                    "With label",
                    boxed(
                        CoverageDots::new("Labeled coverage", 1, 3)
                            .tone(SemanticTone::Warning)
                            .show_label(true)
                            .theme(theme),
                    ),
                ),
            ],
        ),
    ]
}

fn signal_meter(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let meter = |name: &str, level: f32, tone: SemanticTone, active: bool| {
        SignalMeter::new(name)
            .level(level)
            .tone(tone)
            .active(active)
            .bars(10)
            .size(Size::new(92.0, 30.0))
            .theme(theme)
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Strong",
                boxed(meter("Strong signal", 0.9, SemanticTone::Success, true)),
            ),
            (
                "Medium",
                boxed(meter("Medium signal", 0.55, SemanticTone::Accent, true)),
            ),
            (
                "Weak",
                boxed(meter("Weak signal", 0.2, SemanticTone::Warning, true)),
            ),
            (
                "Inactive",
                boxed(meter("Inactive signal", 0.6, SemanticTone::Neutral, false)),
            ),
        ],
    )]
}

fn status_bar(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![example(
        "",
        sized(
            520.0,
            StatusBar::new()
                .name("Document status bar")
                .description("Ready, two warnings, zoom 72 percent")
                .theme(theme)
                .segment(StatusBarSegment::new("Ready").min_width(76.0))
                .segment(
                    StatusBarSegment::new("2 warnings")
                        .tone(SemanticTone::Warning)
                        .min_width(104.0),
                )
                .segment(
                    StatusBarSegment::new("Synced")
                        .tone(SemanticTone::Success)
                        .min_width(80.0),
                )
                .segment(
                    StatusBarSegment::new("Zoom 72%")
                        .min_width(96.0)
                        .expand(true),
                ),
        ),
    )]
}

fn empty_state(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![strip(
        theme,
        "",
        vec![
            (
                "With action",
                boxed(
                    SizedBox::new().width(320.0).height(200.0).with_child(
                        EmptyState::new(
                            "No search results",
                            "Try a broader query or clear active filters.",
                        )
                        .icon(IconGlyph::Search)
                        .action(Button::primary("Clear filters").theme(theme))
                        .theme(theme),
                    ),
                ),
            ),
            (
                "Transparent",
                boxed(
                    SizedBox::new().width(280.0).height(160.0).with_child(
                        EmptyState::new("No layers", "Add a layer to start drawing.")
                            .icon(IconGlyph::File)
                            .transparent()
                            .theme(theme),
                    ),
                ),
            ),
        ],
    )]
}
