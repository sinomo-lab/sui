use sui::prelude::*;

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{BoxedWidget, Section, boxed, example, grid};
use super::selection::size_strip;
use super::{preview_label, sized};

pub(super) const STORIES: [Story; 6] = [
    Story {
        id: "text-input",
        title: "Text input",
        api: "TextInput",
        summary: "Single-line text entry with placeholder, leading icon, and read-only variants.",
        keywords: "field entry search type",
        category: Category::TextEntry,
        build: text_input,
    },
    Story {
        id: "password-input",
        title: "Password input",
        api: "PasswordInput",
        summary: "Masks its value while keeping editing and selection behavior.",
        keywords: "secret masked credential field",
        category: Category::TextEntry,
        build: password_input,
    },
    Story {
        id: "date-time-input",
        title: "Date and time input",
        api: "DateTimeInput",
        summary: "Accepts a date and time in YYYY-MM-DD HH:MM form.",
        keywords: "date time schedule field",
        category: Category::TextEntry,
        build: date_time_input,
    },
    Story {
        id: "number-input",
        title: "Number input",
        api: "NumberInput",
        summary: "Numeric entry with steppers, a range, and fixed precision.",
        keywords: "spin box stepper numeric value spinbox",
        category: Category::TextEntry,
        build: number_input,
    },
    Story {
        id: "text-area",
        title: "Text area",
        api: "TextArea",
        summary: "Multiline text entry for notes and longer values.",
        keywords: "multiline notes text field multilinetextinput",
        category: Category::TextEntry,
        build: text_area,
    },
    Story {
        id: "select",
        title: "Select",
        api: "Select",
        summary: "Chooses one option from a list that opens below the field.",
        keywords: "combo box dropdown options picker combobox",
        category: Category::TextEntry,
        build: select,
    },
];

/// Hover and focus are the only interaction states fields paint.
const FIELD_STATES: [InteractionPreview; 3] = [
    InteractionPreview::None,
    InteractionPreview::Hovered,
    InteractionPreview::Focused,
];

fn field_columns() -> Vec<&'static str> {
    FIELD_STATES
        .iter()
        .map(|preview| preview_label(*preview))
        .chain(["Disabled"])
        .collect()
}

/// Rows of `(label, build)` across the field states, then disabled. `build`
/// takes the state to preview and whether the field is enabled.
fn field_rows<W, F>(rows: Vec<(&'static str, F)>) -> Vec<(&'static str, Vec<BoxedWidget>)>
where
    W: Widget + 'static,
    F: Fn(InteractionPreview, bool) -> W,
{
    rows.into_iter()
        .map(|(label, make)| {
            let cells = FIELD_STATES
                .iter()
                .map(|preview| boxed(make(*preview, true)))
                .chain([boxed(make(InteractionPreview::None, false))])
                .collect();
            (label, cells)
        })
        .collect()
}

const FIELD_WIDTH: f32 = 180.0;

fn text_input(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |value: &'static str, placeholder: &'static str, icon: bool, read_only: bool| {
        move |preview, enabled: bool| {
            let mut input = TextInput::new("Layer name")
                .placeholder(placeholder)
                .value(value)
                .interaction_preview(preview)
                .enabled(enabled)
                .theme(theme);
            if icon {
                input = input.leading_icon(IconGlyph::Search);
            }
            if read_only {
                input = input.read_only(true);
            }
            sized(FIELD_WIDTH, input)
        }
    };
    vec![
        grid(
            theme,
            "Content and states",
            &field_columns(),
            field_rows(vec![
                ("Placeholder", make("", "Search layers", false, false)),
                ("Value", make("Layer 08 / mask", "", false, false)),
                ("Leading icon", make("", "Search", true, false)),
                ("Read only", make("sui-build-042", "", false, true)),
            ]),
        ),
        size_strip(ctx, |size_theme| {
            sized(
                FIELD_WIDTH,
                TextInput::new("Sized layer name")
                    .value("Layer name")
                    .theme(size_theme),
            )
        }),
    ]
}

fn password_input(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |value: &'static str| {
        move |preview, enabled: bool| {
            sized(
                FIELD_WIDTH,
                PasswordInput::new("Password")
                    .placeholder("Enter a password")
                    .value(value)
                    .interaction_preview(preview)
                    .enabled(enabled)
                    .theme(theme),
            )
        }
    };
    vec![grid(
        theme,
        "",
        &field_columns(),
        field_rows(vec![("Empty", make("")), ("Filled", make("sui-demo"))]),
    )]
}

fn date_time_input(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |value: &'static str| {
        move |preview, enabled: bool| {
            sized(
                FIELD_WIDTH,
                DateTimeInput::new("Scheduled for")
                    .value(value)
                    .interaction_preview(preview)
                    .enabled(enabled)
                    .theme(theme),
            )
        }
    };
    vec![grid(
        theme,
        "",
        &field_columns(),
        field_rows(vec![
            ("Empty", make("")),
            ("Filled", make("2026-07-15 14:30")),
        ]),
    )]
}

fn number_input(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |precision: usize, value: f64| {
        move |preview, enabled: bool| {
            sized(
                140.0,
                NumberInput::new("Brush size")
                    .range(0.0, 256.0)
                    .step(1.0)
                    .precision(precision)
                    .value(value)
                    .interaction_preview(preview)
                    .enabled(enabled)
                    .theme(theme),
            )
        }
    };
    let states = [
        InteractionPreview::None,
        InteractionPreview::Hovered,
        InteractionPreview::Pressed,
        InteractionPreview::Focused,
    ];
    let rows = [("Integer", make(0, 24.0)), ("Two decimals", make(2, 0.75))]
        .into_iter()
        .map(|(label, make)| {
            (
                label,
                states
                    .iter()
                    .map(|preview| boxed(make(*preview, true)))
                    .chain([boxed(make(InteractionPreview::None, false))])
                    .collect::<Vec<BoxedWidget>>(),
            )
        })
        .collect();
    let columns: Vec<&str> = states
        .iter()
        .map(|preview| preview_label(*preview))
        .chain(["Disabled"])
        .collect();
    vec![
        grid(theme, "Precision and states", &columns, rows),
        size_strip(ctx, |size_theme| {
            sized(
                140.0,
                NumberInput::new("Sized brush size")
                    .range(0.0, 64.0)
                    .value(12.0)
                    .theme(size_theme),
            )
        }),
    ]
}

fn text_area(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let make = |value: &'static str| {
        move |preview, enabled: bool| {
            SizedBox::new().width(200.0).height(88.0).with_child(
                TextArea::new("Notes")
                    .placeholder("Write notes")
                    .value(value)
                    .interaction_preview(preview)
                    .enabled(enabled)
                    .theme(theme),
            )
        }
    };
    vec![grid(
        theme,
        "",
        &field_columns(),
        field_rows(vec![
            ("Placeholder", make("")),
            ("Value", make("Frame notes\nOpacity ramp is locked")),
        ]),
    )]
}

fn select(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let modes = ["Normal", "Multiply", "Screen", "Overlay"];
    let make = |selected: Option<usize>| {
        move |preview, enabled: bool| {
            let select = Select::new("Blend mode")
                .placeholder("Choose blend mode")
                .options(modes)
                .interaction_preview(preview)
                .enabled(enabled)
                .theme(theme);
            sized(
                FIELD_WIDTH,
                match selected {
                    Some(index) => select.selected(index),
                    None => select,
                },
            )
        }
    };
    let states = [
        InteractionPreview::None,
        InteractionPreview::Hovered,
        InteractionPreview::Pressed,
        InteractionPreview::Focused,
    ];
    let rows = [("Placeholder", make(None)), ("Selected", make(Some(2)))]
        .into_iter()
        .map(|(label, make)| {
            (
                label,
                states
                    .iter()
                    .map(|preview| boxed(make(*preview, true)))
                    .chain([boxed(make(InteractionPreview::None, false))])
                    .collect::<Vec<BoxedWidget>>(),
            )
        })
        .collect();
    let columns: Vec<&str> = states
        .iter()
        .map(|preview| preview_label(*preview))
        .chain(["Disabled"])
        .collect();
    vec![
        grid(theme, "Content and states", &columns, rows),
        example(
            "Open",
            sized(
                FIELD_WIDTH,
                Select::new("Open blend mode")
                    .options(modes)
                    .selected(1)
                    .theme(theme)
                    .show_inline(),
            ),
        ),
        size_strip(ctx, |size_theme| {
            sized(
                FIELD_WIDTH,
                Select::new("Sized blend mode")
                    .options(modes)
                    .selected(0)
                    .theme(size_theme),
            )
        }),
    ]
}
