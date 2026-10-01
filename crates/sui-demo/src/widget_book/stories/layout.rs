use sui::prelude::*;
use sui::{GlowTone, SemanticTone, ShadowBox, ShadowParams, ShadowPlacement, StatusBadge};

use super::super::SPLIT_VIEW_NAME;
use super::super::registry::{Category, Story, StoryCtx};
use super::super::specimen::{Section, boxed, example, strip};
use super::sized;
use crate::app::{DemoTextRole, demo_text_style};

pub(super) const STORIES: [Story; 8] = [
    Story {
        id: "surface",
        title: "Surface",
        api: "Surface",
        summary: "Themed backgrounds for app regions, with elevation, appearance, and tone.",
        keywords: "panel card background elevation shadow",
        category: Category::Layout,
        build: surface,
    },
    Story {
        id: "shadows-and-glows",
        title: "Shadows and glows",
        api: "Surface::shadow, Surface::glow, ShadowBox, paint_theme_glow",
        summary: "Theme shadows around and inside boxes, and the glows live signals wear.",
        keywords: "shadow elevation inset glow halo box-shadow",
        category: Category::Layout,
        build: shadows_and_glows,
    },
    Story {
        id: "separator",
        title: "Separator and section label",
        api: "Separator, SectionLabel",
        summary: "Hairlines between groups and small labels that title a group.",
        keywords: "divider rule hairline heading",
        category: Category::Layout,
        build: separator,
    },
    Story {
        id: "form-section",
        title: "Form section",
        api: "FormSection, FieldGroup, FormRow, PropertyRow, DetailRow",
        summary: "Grouped settings rows with stacked or inline labels and read-only details.",
        keywords: "form settings rows inspector property",
        category: Category::Layout,
        build: form_section,
    },
    Story {
        id: "panel-section",
        title: "Panel section and dock panel",
        api: "PanelSection, DockPanel",
        summary: "Collapsible inspector groups inside a titled dock panel.",
        keywords: "inspector collapsible dock",
        category: Category::Layout,
        build: panel_section,
    },
    Story {
        id: "split-view",
        title: "Split view",
        api: "SplitView",
        summary: "Two panes separated by a draggable divider.",
        keywords: "splitter resizable panes divider resizablepane",
        category: Category::Layout,
        build: split_view,
    },
    Story {
        id: "pane-layouts",
        title: "Pane layouts",
        api: "FixedPaneSplit, Dock, MeasuredBottomDock, TrailingSlotRow",
        summary: "Fixed-size panes, docked header and footer slots, and trailing slots.",
        keywords: "dock fixed pane slots shell",
        category: Category::Layout,
        build: pane_layouts,
    },
    Story {
        id: "scroll-view",
        title: "Scroll view",
        api: "ScrollView, VirtualScrollView",
        summary: "Bounded, scrollable content; the virtual variant lays out only visible children.",
        keywords: "scroll overflow virtual",
        category: Category::Layout,
        build: scroll_view,
    },
];

fn shadows_and_glows(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let card = |label: &str| {
        SizedBox::new()
            .width(104.0)
            .height(64.0)
            .child(Align::center(Label::new(label).text_style(
                demo_text_style(theme, DemoTextRole::Supporting, theme.palette.text_muted),
            )))
    };
    type Token = fn(&DefaultTheme) -> ThemeShadow;
    let box_shadows: [(&str, Token); 7] = [
        ("2xs", |theme| theme.shadows.box_shadow._2xs),
        ("xs", |theme| theme.shadows.box_shadow.xs),
        ("sm", |theme| theme.shadows.box_shadow.sm),
        ("md", |theme| theme.shadows.box_shadow.md),
        ("lg", |theme| theme.shadows.box_shadow.lg),
        ("xl", |theme| theme.shadows.box_shadow.xl),
        ("2xl", |theme| theme.shadows.box_shadow._2xl),
    ];
    let inset_shadows: [(&str, Token); 4] = [
        ("2xs", |theme| theme.shadows.inset._2xs),
        ("xs", |theme| theme.shadows.inset.xs),
        ("sm", |theme| theme.shadows.inset.sm),
        ("Deeper", |_| {
            ThemeShadow::single(ThemeShadowLayer {
                offset_x: 0.0,
                offset_y: 3.0,
                blur: 10.0,
                spread: 0.0,
                color: Color::BLACK.with_alpha(0.35),
                inset: true,
            })
        }),
    ];
    let glow_caption = if theme.glows.accent.first.is_some() {
        "Glows"
    } else {
        "Glows: none in light themes"
    };
    vec![
        strip(
            theme,
            "Box shadows",
            box_shadows
                .into_iter()
                .map(|(label, token)| {
                    (
                        label,
                        boxed(
                            Surface::panel(card(label))
                                .corner_radius(10.0)
                                .shadow(token)
                                .theme(theme),
                        ),
                    )
                })
                .collect(),
        ),
        strip(
            theme,
            "Inset shadows",
            inset_shadows
                .into_iter()
                .map(|(label, token)| {
                    (
                        label,
                        boxed(
                            Surface::field(card(label))
                                .corner_radius(10.0)
                                .shadow(token)
                                .theme(theme),
                        ),
                    )
                })
                .collect(),
        ),
        strip(
            theme,
            glow_caption,
            vec![
                (
                    "Accent",
                    boxed(
                        Surface::panel(card("Live"))
                            .corner_radius(10.0)
                            .glow(GlowTone::Accent)
                            .theme(theme),
                    ),
                ),
                (
                    "Secondary",
                    boxed(
                        Surface::panel(card("Voice"))
                            .corner_radius(32.0)
                            .glow(GlowTone::Secondary)
                            .theme(theme),
                    ),
                ),
                (
                    "Primary action",
                    boxed(Button::primary("Publish").theme(theme)),
                ),
                (
                    "Busy",
                    boxed(Spinner::new("Syncing").label("Syncing").theme(theme)),
                ),
                (
                    "Shadow box",
                    boxed(
                        ShadowBox::new(ColorSwatch(
                            theme.decorative.get(DecorativeHue::Violet).solid,
                        ))
                        .corner_radius(12.0)
                        .shadow(|theme| theme.shadows.box_shadow.md)
                        .glow(GlowTone::Secondary)
                        .theme(theme),
                    ),
                ),
            ],
        ),
        strip(
            theme,
            "Under a translucent fill",
            vec![
                (
                    "Behind",
                    boxed(Translucent::new(theme, ShadowPlacement::Behind)),
                ),
                (
                    "Outside",
                    boxed(Translucent::new(theme, ShadowPlacement::Outside)),
                ),
            ],
        ),
    ]
}

/// A block of one color that paints its own face, for [`ShadowBox`].
struct ColorSwatch(Color);

impl Widget for ColorSwatch {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(64.0, 64.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill(Path::rounded_rect(ctx.bounds(), 12.0), self.0);
    }
}

/// A see-through box casting a shadow with `placement`: behind shows through
/// the fill, outside leaves the box clear. The shadow takes the accent color
/// so it shows on dark backgrounds too, where a black one would not.
struct Translucent {
    theme: DefaultTheme,
    placement: ShadowPlacement,
}

impl Translucent {
    fn new(theme: DefaultTheme, placement: ShadowPlacement) -> Self {
        Self { theme, placement }
    }
}

impl Widget for Translucent {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(104.0, 64.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let palette = self.theme.palette;
        let shadow = ShadowParams::new(0.0, 8.0, 18.0, 0.0, palette.accent.with_alpha(0.6))
            .with_placement(self.placement);
        ctx.draw_shadow(bounds, [10.0; 4], shadow);
        ctx.fill(
            Path::rounded_rect(bounds, 10.0),
            palette.text.with_alpha(0.12),
        );
    }
}

fn surface(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let tile = |label: &str, color: Color| {
        SizedBox::new()
            .width(116.0)
            .height(64.0)
            .child(Align::center(Label::new(label).text_style(
                demo_text_style(theme, DemoTextRole::Supporting, color),
            )))
    };
    let muted = theme.palette.text_muted;
    let roles = [
        ("Window", SurfaceRole::Window),
        ("Sidebar", SurfaceRole::Sidebar),
        ("Panel", SurfaceRole::Panel),
        ("Titlebar", SurfaceRole::Titlebar),
        ("Field", SurfaceRole::Field),
    ];
    let elevations = [
        ("None", SurfaceElevation::None),
        ("Small", SurfaceElevation::Small),
        ("Medium", SurfaceElevation::Medium),
        ("Large", SurfaceElevation::Large),
    ];
    let appearances = [
        (
            "Standard",
            SurfaceAppearance::Standard,
            SemanticTone::Neutral,
        ),
        ("Raised", SurfaceAppearance::Raised, SemanticTone::Neutral),
        ("Soft accent", SurfaceAppearance::Soft, SemanticTone::Accent),
        (
            "Soft success",
            SurfaceAppearance::Soft,
            SemanticTone::Success,
        ),
        (
            "Filled accent",
            SurfaceAppearance::Filled,
            SemanticTone::Accent,
        ),
    ];
    vec![
        strip(
            theme,
            "Roles",
            roles
                .into_iter()
                .map(|(label, role)| {
                    (
                        label,
                        boxed(
                            Surface::new(role, tile(label, muted))
                                .border(SurfaceBorder::All)
                                .corner_radius(8.0)
                                .theme(theme),
                        ),
                    )
                })
                .collect(),
        ),
        strip(
            theme,
            "Elevation",
            elevations
                .into_iter()
                .map(|(label, elevation)| {
                    (
                        label,
                        boxed(
                            Surface::panel(tile(label, muted))
                                .elevation(elevation)
                                .corner_radius(8.0)
                                .theme(theme),
                        ),
                    )
                })
                .collect(),
        ),
        strip(
            theme,
            "Appearance and tone",
            appearances
                .into_iter()
                .map(|(label, appearance, tone)| {
                    (
                        label,
                        boxed(
                            Surface::panel(tile(
                                label,
                                if appearance == SurfaceAppearance::Filled {
                                    theme.palette.accent_text
                                } else {
                                    muted
                                },
                            ))
                            .appearance(appearance)
                            .tone(tone)
                            .corner_radius(8.0)
                            .theme(theme),
                        ),
                    )
                })
                .collect(),
        ),
    ]
}

fn separator(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![
        strip(
            theme,
            "Separators",
            vec![
                (
                    "Horizontal",
                    boxed(sized(200.0, Separator::horizontal().theme(theme))),
                ),
                (
                    "Inset",
                    boxed(sized(
                        200.0,
                        Separator::horizontal().inset(24.0).theme(theme),
                    )),
                ),
                (
                    "Vertical",
                    boxed(
                        SizedBox::new()
                            .height(48.0)
                            .child(Separator::vertical().theme(theme)),
                    ),
                ),
            ],
        ),
        strip(
            theme,
            "Section label",
            vec![
                (
                    "Default",
                    boxed(SectionLabel::new("Inspector").theme(theme)),
                ),
                (
                    "Accent",
                    boxed(
                        SectionLabel::new("Recent")
                            .color(theme.palette.accent)
                            .theme(theme),
                    ),
                ),
            ],
        ),
    ]
}

fn form_section(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    vec![example(
        "",
        sized(
            420.0,
            FormSection::new(
                "Publish settings",
                FieldGroup::new()
                    .fill_width(true)
                    .gap(10.0)
                    .with_child(
                        FormRow::new(
                            "Target",
                            TextInput::new("Publish target")
                                .value("staging")
                                .theme(theme),
                        )
                        .theme(theme),
                    )
                    .with_child(
                        FormRow::new(
                            "Channel",
                            Select::new("Publish channel")
                                .options(["Stable", "Beta", "Nightly"])
                                .selected(1)
                                .theme(theme),
                        )
                        .theme(theme),
                    )
                    .with_child(
                        PropertyRow::new(
                            "Opacity",
                            Slider::new("Opacity property value")
                                .range(0.0, 100.0)
                                .value(72.0)
                                .theme(theme),
                        )
                        .inline()
                        .theme(theme),
                    )
                    .with_child(DetailRow::new("Last publish", "2 min ago").theme(theme))
                    .with_child(DetailRow::new("Build", "sui-042 (release)").theme(theme)),
            )
            .description("Applies to every environment in this cluster.")
            .header_action(Button::new("Reset").theme(theme))
            .theme(theme),
        ),
    )]
}

fn panel_section(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let details = |first: &str, second: &str| {
        Stack::vertical()
            .gap(6.0)
            .alignment(Alignment::Stretch)
            .with_child(ctx.muted(first))
            .with_child(ctx.muted(second))
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Dock panel",
                boxed(sized(
                    300.0,
                    DockPanel::new(
                        "Inspector",
                        Stack::vertical()
                            .gap(8.0)
                            .alignment(Alignment::Stretch)
                            .with_child(
                                PanelSection::new(
                                    "Layer",
                                    details("Blend: Screen", "Mask feather: 8 px"),
                                )
                                .collapsible(true)
                                .theme(theme),
                            )
                            .with_child(
                                PanelSection::new("Effects", details("Glow", "Drop shadow"))
                                    .collapsible(true)
                                    .expanded(false)
                                    .theme(theme),
                            ),
                    )
                    .name("Inspector dock panel")
                    .theme(theme),
                )),
            ),
            (
                "Header action",
                boxed(sized(
                    260.0,
                    PanelSection::new("Transform", details("X 120  Y 48", "Rotation 0°"))
                        .header_action(
                            IconButton::new(IconGlyph::MoreHorizontal, "Transform options")
                                .theme(theme),
                        )
                        .theme(theme),
                )),
            ),
        ],
    )]
}

fn split_view(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let pane = |title: &str, detail: &str, role: SurfaceRole| {
        Surface::new(
            role,
            Stack::vertical()
                .gap(6.0)
                .alignment(Alignment::Stretch)
                .with_child(ctx.text(title))
                .with_child(ctx.muted(detail)),
        )
        .padding(Insets::all(14.0))
        .fill()
        .theme(theme)
    };
    vec![
        example(
            "Horizontal",
            SizedBox::new().width(560.0).height(160.0).child(
                SplitView::horizontal(
                    pane(
                        "Viewport",
                        "Drag the divider to resize.",
                        SurfaceRole::Window,
                    ),
                    pane(
                        "Inspector",
                        "Keeps its minimum width.",
                        SurfaceRole::Sidebar,
                    ),
                )
                .name(SPLIT_VIEW_NAME)
                .ratio(0.62)
                .min_second(160.0)
                .theme(theme),
            ),
        ),
        example(
            "Vertical",
            SizedBox::new().width(360.0).height(200.0).child(
                SplitView::vertical(
                    pane("Editor", "Source text", SurfaceRole::Window),
                    pane("Console", "Build output", SurfaceRole::Panel),
                )
                .name("Console split")
                .ratio(0.6)
                .theme(theme),
            ),
        ),
    ]
}

/// Outlines a layout sample so panes that share the stage color stay visible.
fn framed<W>(theme: DefaultTheme, child: W) -> Surface
where
    W: Widget + 'static,
{
    Surface::window(child)
        .border(SurfaceBorder::All)
        .fill()
        .theme(theme)
}

fn pane_layouts(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let fill = |role: SurfaceRole, label: &str| {
        Surface::new(role, ctx.muted(label))
            .padding(Insets::all(10.0))
            .fill()
            .theme(theme)
    };
    vec![strip(
        theme,
        "",
        vec![
            (
                "Fixed pane split",
                boxed(
                    SizedBox::new().width(280.0).height(130.0).child(framed(
                        theme,
                        FixedPaneSplit::horizontal(
                            fill(SurfaceRole::Sidebar, "Fixed 96 px"),
                            Separator::vertical().theme(theme),
                            fill(SurfaceRole::Window, "Flexible"),
                        )
                        .fixed_first(96.0)
                        .divider_extent(1.0),
                    )),
                ),
            ),
            (
                "Dock",
                boxed(
                    SizedBox::new().width(280.0).height(130.0).child(framed(
                        theme,
                        Dock::new(fill(SurfaceRole::Window, "Body fills the rest"))
                            .top(32.0, fill(SurfaceRole::Titlebar, "Top slot"))
                            .bottom(32.0, fill(SurfaceRole::Titlebar, "Bottom slot")),
                    )),
                ),
            ),
            (
                "Measured bottom dock",
                boxed(
                    SizedBox::new().width(280.0).height(130.0).child(framed(
                        theme,
                        MeasuredBottomDock::new(
                            fill(SurfaceRole::Window, "Body"),
                            StatusBar::new()
                                .text_segment("Measured footer")
                                .theme(theme),
                        ),
                    )),
                ),
            ),
            (
                "Trailing slot row",
                boxed(
                    SizedBox::new().width(280.0).height(34.0).child(
                        TrailingSlotRow::new(
                            ctx.text("Replication"),
                            StatusBadge::new("Active")
                                .tone(SemanticTone::Success)
                                .theme(theme),
                        )
                        .trailing_width(96.0)
                        .trailing_height(28.0)
                        .gap(10.0),
                    ),
                ),
            ),
        ],
    )]
}

fn scroll_view(ctx: &StoryCtx) -> Vec<Section> {
    let theme = ctx.theme;
    let rows = |prefix: &str| {
        (1..=12).fold(
            Stack::vertical().gap(8.0).alignment(Alignment::Stretch),
            |stack, index| stack.with_child(ctx.text(format!("{prefix} {index}"))),
        )
    };
    let virtual_rows = (1..=40).fold(
        VirtualScrollView::new()
            .name("Virtual scroll sample")
            .padding(Insets::all(10.0))
            .gap(8.0)
            .theme(theme),
        |view, index| view.with_child(ctx.text(format!("Virtual row {index}"))),
    );
    vec![strip(
        theme,
        "",
        vec![
            (
                "Scroll view",
                boxed(
                    SizedBox::new().width(240.0).height(140.0).child(
                        Surface::field(
                            ScrollView::vertical(Padding::all(10.0, rows("Scroll item")))
                                .name("Inner scroll view")
                                .theme(theme),
                        )
                        .fill()
                        .theme(theme),
                    ),
                ),
            ),
            (
                "Virtual scroll view",
                boxed(
                    SizedBox::new()
                        .width(240.0)
                        .height(140.0)
                        .child(Surface::field(virtual_rows).fill().theme(theme)),
                ),
            ),
        ],
    )]
}
