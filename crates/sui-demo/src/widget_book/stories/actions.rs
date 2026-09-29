use sui::SemanticTone;
use sui::prelude::*;

use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{BoxedWidget, Section, boxed, example, grid, strip};
use super::{PREVIEW_STATES, TONES, preview_label, tone_label};

pub(super) const STORIES: [Story; 6] = [
    Story {
        id: "button",
        title: "Button",
        api: "Button",
        summary: "Triggers an action. Filled for the primary action, tonal by default, outline and ghost for quieter commands.",
        keywords: "primary danger action command press",
        category: Category::Actions,
        build: button,
    },
    Story {
        id: "icon-button",
        title: "Icon button",
        api: "IconButton",
        summary: "A compact, square action labeled by its icon and an accessible name.",
        keywords: "toolbar glyph toggle selected",
        category: Category::Actions,
        build: icon_button,
    },
    Story {
        id: "action-card",
        title: "Action card",
        api: "ActionCard",
        summary: "A large, pressable card for launchers and onboarding entry points.",
        keywords: "tile launcher decorative hue",
        category: Category::Actions,
        build: action_card,
    },
    Story {
        id: "toolbar",
        title: "Toolbar and command group",
        api: "Toolbar, CommandGroup",
        summary: "Rows of related commands. Command groups frame a tight cluster inside a toolbar.",
        keywords: "divider command bar actions",
        category: Category::Actions,
        build: toolbar,
    },
    Story {
        id: "tool-palette",
        title: "Tool palette",
        api: "ToolPalette",
        summary: "An exclusive set of editing tools, horizontal or vertical.",
        keywords: "brush eraser select tools",
        category: Category::Actions,
        build: tool_palette,
    },
    Story {
        id: "preset-strip",
        title: "Preset strip",
        api: "PresetStrip",
        summary: "A row of named presets with one active choice.",
        keywords: "presets chips quick choice",
        category: Category::Actions,
        build: preset_strip,
    },
];

/// A labeled way to construct one button style.
type ButtonStyle = (&'static str, fn() -> Button);

fn button(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let styles: [ButtonStyle; 5] = [
        ("Primary", || Button::primary("Export")),
        ("Default", || Button::new("Export")),
        ("Outline", || {
            Button::new("Export").appearance(ButtonAppearance::Outline)
        }),
        ("Ghost", || {
            Button::new("Export").appearance(ButtonAppearance::Ghost)
        }),
        ("Danger", || Button::danger("Delete")),
    ];
    let state_rows = styles
        .iter()
        .map(|(label, make)| {
            let mut row: Vec<BoxedWidget> = PREVIEW_STATES
                .iter()
                .map(|preview| boxed(make().interaction_preview(*preview).theme(theme)))
                .collect();
            row.push(boxed(make().enabled(false).theme(theme)));
            (*label, row)
        })
        .collect();
    let mut state_columns: Vec<&str> = PREVIEW_STATES.iter().map(|p| preview_label(*p)).collect();
    state_columns.push("Disabled");

    let appearances = [
        ("Filled", ButtonAppearance::Filled),
        ("Tonal", ButtonAppearance::Tonal),
        ("Outline", ButtonAppearance::Outline),
        ("Ghost", ButtonAppearance::Ghost),
    ];
    let tone_rows = appearances
        .iter()
        .map(|(label, appearance)| {
            let row = TONES
                .iter()
                .map(|tone| {
                    boxed(
                        Button::new(tone_label(*tone))
                            .appearance(*appearance)
                            .tone(*tone)
                            .theme(theme),
                    )
                })
                .collect();
            (*label, row)
        })
        .collect();
    let tone_columns: Vec<&str> = TONES.iter().map(|tone| tone_label(*tone)).collect();

    vec![
        grid(theme, "Styles and states", &state_columns, state_rows),
        grid(theme, "Appearances and tones", &tone_columns, tone_rows),
        strip(
            theme,
            "Sizes",
            vec![
                (
                    "Small",
                    boxed(Button::primary("Export").theme(ctx.sized(ControlSize::Small))),
                ),
                (
                    "Medium",
                    boxed(Button::primary("Export").theme(ctx.sized(ControlSize::Medium))),
                ),
                (
                    "Large",
                    boxed(Button::primary("Export").theme(ctx.sized(ControlSize::Large))),
                ),
            ],
        ),
        strip(
            theme,
            "Content",
            vec![
                ("Label", boxed(Button::new("Share").theme(theme))),
                (
                    "Icon and label",
                    boxed(
                        Button::new("Download")
                            .icon(IconGlyph::Download)
                            .theme(theme),
                    ),
                ),
                (
                    "Minimum width",
                    boxed(Button::new("OK").min_width(120.0).theme(theme)),
                ),
            ],
        ),
    ]
}

fn icon_button(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let styles: [(&str, ButtonAppearance, SemanticTone); 4] = [
        ("Tonal", ButtonAppearance::Tonal, SemanticTone::Neutral),
        ("Ghost", ButtonAppearance::Ghost, SemanticTone::Neutral),
        ("Outline", ButtonAppearance::Outline, SemanticTone::Neutral),
        ("Filled", ButtonAppearance::Filled, SemanticTone::Accent),
    ];
    let rows = styles
        .iter()
        .map(|(label, appearance, tone)| {
            let make = || {
                IconButton::new(IconGlyph::Search, "Search")
                    .appearance(*appearance)
                    .tone(*tone)
                    .theme(theme)
            };
            let mut row: Vec<BoxedWidget> = PREVIEW_STATES
                .iter()
                .map(|preview| boxed(make().interaction_preview(*preview)))
                .collect();
            row.push(boxed(make().selected(true)));
            row.push(boxed(make().enabled(false)));
            (*label, row)
        })
        .collect();
    let mut columns: Vec<&str> = PREVIEW_STATES.iter().map(|p| preview_label(*p)).collect();
    columns.extend(["Selected", "Disabled"]);

    let glyphs = [
        IconGlyph::Undo,
        IconGlyph::Redo,
        IconGlyph::Brush,
        IconGlyph::Eraser,
        IconGlyph::Trash,
        IconGlyph::MoreHorizontal,
    ];
    vec![
        grid(theme, "Styles and states", &columns, rows),
        strip(
            theme,
            "Sizes",
            vec![
                (
                    "Small",
                    boxed(
                        IconButton::new(IconGlyph::Add, "Add small")
                            .theme(ctx.sized(ControlSize::Small)),
                    ),
                ),
                (
                    "Medium",
                    boxed(
                        IconButton::new(IconGlyph::Add, "Add medium")
                            .theme(ctx.sized(ControlSize::Medium)),
                    ),
                ),
                (
                    "Large",
                    boxed(
                        IconButton::new(IconGlyph::Add, "Add large")
                            .theme(ctx.sized(ControlSize::Large)),
                    ),
                ),
            ],
        ),
        example(
            "Glyphs",
            glyphs.iter().fold(
                Stack::horizontal()
                    .spacing(8.0)
                    .alignment(Alignment::Center),
                |row, glyph| {
                    row.with_child(IconButton::new(*glyph, format!("{glyph:?}")).theme(theme))
                },
            ),
        ),
    ]
}

fn action_card(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let card = |title: &str, description: &str| {
        ActionCard::new(title, description)
            .min_width(240.0)
            .theme(theme)
    };
    vec![
        strip(
            theme,
            "Decorative hues",
            vec![
                (
                    "Blue",
                    boxed(
                        card("Open canvas", "Start a drawing workspace.")
                            .icon(IconGlyph::Brush)
                            .decorative(DecorativeHue::Blue),
                    ),
                ),
                (
                    "Violet",
                    boxed(
                        card("Ask the assistant", "Draft with suggestions.")
                            .icon(IconGlyph::Sparkles)
                            .decorative(DecorativeHue::Violet),
                    ),
                ),
                (
                    "Green",
                    boxed(
                        card("Import files", "Bring in images and documents.")
                            .icon(IconGlyph::Folder)
                            .decorative(DecorativeHue::Green),
                    ),
                ),
            ],
        ),
        strip(
            theme,
            "States",
            vec![
                (
                    "Semantic tone",
                    boxed(
                        card("Publish", "Push the build to staging.")
                            .icon(IconGlyph::Send)
                            .tone(SemanticTone::Success),
                    ),
                ),
                (
                    "Without icon",
                    boxed(card("Recent files", "Reopen your latest work.").without_icon()),
                ),
                (
                    "Disabled",
                    boxed(
                        card("Sync library", "Connect storage to enable.")
                            .icon(IconGlyph::Storage)
                            .enabled(false),
                    ),
                ),
            ],
        ),
    ]
}

fn toolbar(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let icon = |glyph: IconGlyph, label: &str| IconButton::new(glyph, label).theme(theme);
    let document_toolbar = Toolbar::horizontal()
        .name("Document toolbar")
        .theme(theme)
        .with_child(icon(IconGlyph::Undo, "Undo"))
        .with_child(icon(IconGlyph::Redo, "Redo"))
        .with_child(Divider::vertical().theme(theme))
        .with_child(
            CommandGroup::horizontal("Zoom commands")
                .theme(theme)
                .with_child(icon(IconGlyph::Remove, "Zoom out"))
                .with_child(icon(IconGlyph::FitView, "Fit canvas"))
                .with_child(icon(IconGlyph::Add, "Zoom in")),
        )
        .with_child(Divider::vertical().theme(theme))
        .with_child(Button::new("Share").theme(theme))
        .with_child(Button::primary("Publish").theme(theme));
    vec![
        example(
            "Horizontal toolbar",
            SizedBox::new().width(520.0).with_child(document_toolbar),
        ),
        strip(
            theme,
            "Command groups",
            vec![
                (
                    "Horizontal",
                    boxed(
                        CommandGroup::horizontal("Alignment commands")
                            .theme(theme)
                            .with_child(icon(IconGlyph::ChevronLeft, "Align left"))
                            .with_child(icon(IconGlyph::ActualSize, "Align center"))
                            .with_child(icon(IconGlyph::ChevronRight, "Align right")),
                    ),
                ),
                (
                    "Vertical",
                    boxed(
                        CommandGroup::vertical("Layer commands")
                            .theme(theme)
                            .with_child(icon(IconGlyph::ArrowUp, "Raise layer"))
                            .with_child(icon(IconGlyph::ChevronDown, "Lower layer"))
                            .with_child(icon(IconGlyph::Trash, "Delete layer")),
                    ),
                ),
                (
                    "Vertical toolbar",
                    boxed(
                        SizedBox::new().height(150.0).with_child(
                            Toolbar::vertical()
                                .name("Side toolbar")
                                .theme(theme)
                                .with_child(icon(IconGlyph::Hand, "Pan"))
                                .with_child(icon(IconGlyph::Search, "Zoom"))
                                .with_child(icon(IconGlyph::MoreVertical, "More tools")),
                        ),
                    ),
                ),
            ],
        ),
    ]
}

fn tool_palette(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let tools = || {
        [
            ToolPaletteItem::new(IconGlyph::Hand, "Move"),
            ToolPaletteItem::new(IconGlyph::Brush, "Brush"),
            ToolPaletteItem::new(IconGlyph::Eraser, "Eraser"),
            ToolPaletteItem::new(IconGlyph::PaintBucket, "Fill"),
        ]
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Horizontal",
                boxed(
                    ToolPalette::horizontal("Paint tools")
                        .items(tools())
                        .selected(1)
                        .theme(theme),
                ),
            ),
            (
                "Vertical",
                boxed(
                    ToolPalette::vertical("Vertical paint tools")
                        .items(tools())
                        .selected(0)
                        .theme(theme),
                ),
            ),
            (
                "Disabled tool",
                boxed(
                    ToolPalette::horizontal("Limited tools")
                        .items([
                            ToolPaletteItem::new(IconGlyph::Hand, "Move"),
                            ToolPaletteItem::new(IconGlyph::Brush, "Brush"),
                            ToolPaletteItem::new(IconGlyph::Eraser, "Eraser").disabled(),
                        ])
                        .selected(0)
                        .theme(theme),
                ),
            ),
        ],
    )]
}

fn preset_strip(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![strip(
        theme,
        "",
        vec![
            (
                "First selected",
                boxed(
                    PresetStrip::new("Export presets")
                        .presets(["Draft", "Review", "Final"])
                        .selected(0)
                        .theme(theme),
                ),
            ),
            (
                "Brush sizes",
                boxed(
                    PresetStrip::new("Brush size presets")
                        .presets(["4 px", "8 px", "16 px", "32 px"])
                        .selected(2)
                        .theme(theme),
                ),
            ),
            (
                "Small",
                boxed(
                    PresetStrip::new("Small presets")
                        .presets(["1x", "2x", "3x"])
                        .selected(1)
                        .theme(ctx.sized(ControlSize::Small)),
                ),
            ),
        ],
    )]
}
