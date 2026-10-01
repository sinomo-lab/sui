use sui::prelude::*;

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{BoxedWidget, Section, boxed, grid, strip};
use super::{PREVIEW_STATES, preview_label, sized};

pub(super) const STORIES: [Story; 6] = [
    Story {
        id: "checkbox",
        title: "Checkbox",
        api: "Checkbox",
        summary: "Toggles one independent option. Plain rows sit in lists; framed rows suit inspectors.",
        keywords: "check toggle boolean option framed",
        category: Category::Selection,
        build: checkbox,
    },
    Story {
        id: "radio-button",
        title: "Radio button",
        api: "RadioButton",
        summary: "One option in a set where exactly one is chosen.",
        keywords: "radio exclusive option framed",
        category: Category::Selection,
        build: radio_button,
    },
    Story {
        id: "radio-group",
        title: "Radio group",
        api: "RadioGroup",
        summary: "A labeled set of radio options with keyboard navigation built in.",
        keywords: "radio exclusive options list",
        category: Category::Selection,
        build: radio_group,
    },
    Story {
        id: "switch",
        title: "Switch",
        api: "Switch",
        summary: "Turns a setting on or off immediately.",
        keywords: "toggle boolean on off",
        category: Category::Selection,
        build: switch,
    },
    Story {
        id: "segmented-control",
        title: "Segmented control",
        api: "SegmentedControl",
        summary: "A compact exclusive choice between two to five views or modes.",
        keywords: "segments view mode toggle group",
        category: Category::Selection,
        build: segmented_control,
    },
    Story {
        id: "slider",
        title: "Slider",
        api: "Slider",
        summary: "Picks a value from a continuous or stepped range.",
        keywords: "range value opacity scrub",
        category: Category::Selection,
        build: slider,
    },
];

fn state_columns() -> Vec<&'static str> {
    PREVIEW_STATES
        .iter()
        .map(|preview| preview_label(*preview))
        .chain(["Disabled"])
        .collect()
}

/// Rows of `(label, build)` compared across every interaction state, then
/// disabled. `build` takes the state to preview and whether the control is
/// enabled.
fn state_rows<W, F>(rows: Vec<(&'static str, F)>) -> Vec<(&'static str, Vec<BoxedWidget>)>
where
    W: Widget + 'static,
    F: Fn(InteractionPreview, bool) -> W,
{
    rows.into_iter()
        .map(|(label, make)| {
            let cells = PREVIEW_STATES
                .iter()
                .map(|preview| boxed(make(*preview, true)))
                .chain([boxed(make(InteractionPreview::None, false))])
                .collect();
            (label, cells)
        })
        .collect()
}

fn checkbox(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |checked: bool, framed: bool| {
        move |preview, enabled: bool| {
            let checkbox = Checkbox::new("Snap to grid")
                .checked(checked)
                .interaction_preview(preview)
                .enabled(enabled)
                .theme(theme);
            if framed { checkbox.framed() } else { checkbox }
        }
    };
    vec![
        grid(
            theme,
            "Plain",
            &state_columns(),
            state_rows(vec![
                ("Unchecked", make(false, false)),
                ("Checked", make(true, false)),
            ]),
        ),
        grid(
            theme,
            "Framed",
            &state_columns(),
            state_rows(vec![
                ("Unchecked", make(false, true)),
                ("Checked", make(true, true)),
            ]),
        ),
        size_strip(ctx, |size_theme| {
            Checkbox::new("Snap to grid")
                .checked(true)
                .theme(size_theme)
        }),
    ]
}

fn radio_button(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |selected: bool, framed: bool| {
        move |preview, enabled: bool| {
            let radio = RadioButton::new("High quality")
                .checked(selected)
                .interaction_preview(preview)
                .enabled(enabled)
                .theme(theme);
            if framed { radio.framed() } else { radio }
        }
    };
    vec![
        grid(
            theme,
            "Plain",
            &state_columns(),
            state_rows(vec![
                ("Unselected", make(false, false)),
                ("Selected", make(true, false)),
            ]),
        ),
        grid(
            theme,
            "Framed",
            &state_columns(),
            state_rows(vec![
                ("Unselected", make(false, true)),
                ("Selected", make(true, true)),
            ]),
        ),
    ]
}

fn radio_group(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![strip(
        theme,
        "",
        vec![
            (
                "First selected",
                boxed(sized(
                    220.0,
                    RadioGroup::new("Render quality")
                        .options(["Balanced", "High", "Fast"])
                        .selected(0)
                        .theme(theme),
                )),
            ),
            (
                "Last selected",
                boxed(sized(
                    220.0,
                    RadioGroup::new("Export format")
                        .options(["PNG", "JPEG", "WebP", "AVIF"])
                        .selected(3)
                        .theme(theme),
                )),
            ),
            (
                "Small",
                boxed(sized(
                    220.0,
                    RadioGroup::new("Small render quality")
                        .options(["Balanced", "High", "Fast"])
                        .selected(1)
                        .theme(ctx.sized(ControlSize::Small)),
                )),
            ),
        ],
    )]
}

fn switch(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |on: bool, framed: bool| {
        move |preview, enabled: bool| {
            let switch = Switch::new("Live preview")
                .checked(on)
                .interaction_preview(preview)
                .enabled(enabled)
                .theme(theme);
            if framed { switch.framed() } else { switch }
        }
    };
    vec![
        grid(
            theme,
            "Plain",
            &state_columns(),
            state_rows(vec![("Off", make(false, false)), ("On", make(true, false))]),
        ),
        grid(
            theme,
            "Framed",
            &state_columns(),
            state_rows(vec![("Off", make(false, true)), ("On", make(true, true))]),
        ),
        size_strip(ctx, |size_theme| {
            Switch::new("Live preview").checked(true).theme(size_theme)
        }),
    ]
}

fn segmented_control(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![
        strip(
            theme,
            "Segments",
            vec![
                (
                    "Two",
                    boxed(
                        SegmentedControl::new("Layout mode")
                            .segments(["List", "Grid"])
                            .selected(0)
                            .theme(theme),
                    ),
                ),
                (
                    "Three",
                    boxed(
                        SegmentedControl::new("Preview mode")
                            .segments(["Preview", "Inspect", "Compare"])
                            .selected(1)
                            .theme(theme),
                    ),
                ),
                (
                    "Disabled segment",
                    boxed(
                        SegmentedControl::new("Channel")
                            .items([
                                SegmentedControlItem::new("RGB"),
                                SegmentedControlItem::new("Alpha"),
                                SegmentedControlItem::new("Depth").enabled(false),
                            ])
                            .selected(0)
                            .theme(theme),
                    ),
                ),
            ],
        ),
        size_strip(ctx, |size_theme| {
            SegmentedControl::new("Sized preview mode")
                .segments(["Day", "Week", "Month"])
                .selected(1)
                .theme(size_theme)
        }),
    ]
}

fn slider(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |value: f64| {
        move |preview, enabled: bool| {
            sized(
                132.0,
                Slider::new("Opacity")
                    .range(0.0, 100.0)
                    .value(value)
                    .interaction_preview(preview)
                    .enabled(enabled)
                    .theme(theme),
            )
        }
    };
    vec![
        grid(
            theme,
            "Values and states",
            &state_columns(),
            state_rows(vec![
                ("0%", make(0.0)),
                ("35%", make(35.0)),
                ("100%", make(100.0)),
            ]),
        ),
        size_strip(ctx, |size_theme| {
            sized(
                180.0,
                Slider::new("Sized opacity")
                    .range(0.0, 100.0)
                    .value(60.0)
                    .theme(size_theme),
            )
        }),
    ]
}

/// The same control at every size preset.
pub(super) fn size_strip<W, F>(ctx: &StoryCtx, make: F) -> Section
where
    W: Widget + 'static,
    F: Fn(DefaultTheme) -> W,
{
    strip(
        ctx.theme,
        "Sizes",
        vec![
            ("Small", boxed(make(ctx.sized(ControlSize::Small)))),
            ("Medium", boxed(make(ctx.sized(ControlSize::Medium)))),
            ("Large", boxed(make(ctx.sized(ControlSize::Large)))),
        ],
    )
}
