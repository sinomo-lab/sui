//! The preview: one scroll that shows the edited theme everywhere at once.
//! Contrast problems come first, then surfaces, decorative hues, motion, and
//! widget book stories with their states laid out side by side.

use sui::prelude::*;
use sui::{Border, Signal, WidgetPodMutVisitor, WidgetPodVisitor};

use super::THEME_EDITOR_PREVIEW_SCROLL_NAME;
use super::contrast::{ContrastCheck, contrast_checks};
use super::motion::MotionSample;
use super::state::ThemeEditorState;
use crate::app::{DemoTextRole, demo_text_style};
use crate::demo_support::NamedSection;
use crate::widget_book::story_preview;

/// Widget book stories in the preview: the controls and overlays whose
/// colors, states, and shapes a theme decides.
pub(super) const PREVIEW_STORIES: [&str; 15] = [
    "button",
    "icon-button",
    "checkbox",
    "switch",
    "segmented-control",
    "slider",
    "text-input",
    "select",
    "tabs",
    "list-view",
    "context-menu",
    "tooltip",
    "dialog",
    "progress-bar",
    "status-badge",
];

pub(super) const CONTRAST_REPORT_NAME: &str = "Contrast report";
pub(super) const SURFACES_SPECIMEN_NAME: &str = "Surface tiers";
pub(super) const DECORATIVE_SPECIMEN_NAME: &str = "Decorative hues";

const PAGE_PADDING: f32 = 28.0;
const SECTION_GAP: f32 = 36.0;

/// One entry in the preview's scroll view. Each rebuilds on its own when the
/// theme changes, and only entries near the viewport are laid out, so a drag
/// in the color picker repaints just what is on screen.
#[derive(Debug, Clone, Copy)]
enum PreviewItem {
    Header,
    Contrast,
    Surfaces,
    Decorative,
    Motion,
    Story(&'static str),
}

impl PreviewItem {
    fn all() -> impl Iterator<Item = Self> {
        [
            Self::Header,
            Self::Contrast,
            Self::Surfaces,
            Self::Decorative,
            Self::Motion,
        ]
        .into_iter()
        .chain(PREVIEW_STORIES.into_iter().map(Self::Story))
    }

    /// The item's widgets in `theme`, inset by `insets`.
    fn build(self, theme: DefaultTheme, insets: Insets) -> WidgetPod {
        match self {
            Self::Header => WidgetPod::new(Padding::new(insets, header(theme))),
            Self::Contrast => WidgetPod::new(Padding::new(insets, contrast_report(theme))),
            Self::Surfaces => WidgetPod::new(Padding::new(
                insets,
                section(
                    theme,
                    "Surfaces and ink",
                    "Each surface tier with every text level and a field on it.",
                    surfaces(theme),
                ),
            )),
            Self::Decorative => WidgetPod::new(Padding::new(
                insets,
                section(
                    theme,
                    "Decorative hues",
                    "Tags on soft fills and solid swatches with large labels.",
                    decorative(theme),
                ),
            )),
            Self::Motion => WidgetPod::new(Padding::new(
                insets,
                section(
                    theme,
                    "Motion",
                    "Loops continuously so the speed setting is visible.",
                    MotionSample::new(theme),
                ),
            )),
            Self::Story(id) => WidgetPod::new(Padding::new(insets, story_preview(id, theme))),
        }
    }
}

pub(super) fn build_preview(state: ThemeEditorState) -> impl Widget {
    let theme = state.theme_signal();
    let scroll_theme = theme.clone();
    let items: Vec<PreviewItem> = PreviewItem::all().collect();
    let last = items.len() - 1;
    let page = items.into_iter().enumerate().fold(
        VirtualScrollView::new()
            .name(THEME_EDITOR_PREVIEW_SCROLL_NAME)
            .padding(Insets {
                left: PAGE_PADDING,
                top: 0.0,
                right: PAGE_PADDING,
                bottom: 0.0,
            })
            .spacing(SECTION_GAP)
            .theme_when(move || scroll_theme.get()),
        |page, (index, item)| {
            // Edge gaps live in the first and last items; scroll view padding
            // would clip content short of the viewport's edges.
            let insets = Insets {
                left: 0.0,
                top: if index == 0 { PAGE_PADDING } else { 0.0 },
                right: 0.0,
                bottom: if index == last { PAGE_PADDING } else { 0.0 },
            };
            page.with_child(RebuildOnChange::new_observable(
                theme.clone(),
                move |theme| item.build(*theme, insets),
            ))
        },
    );
    ThemeBackdrop::new(theme, page)
}

fn header(theme: DefaultTheme) -> impl Widget {
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new("Preview").text_style(demo_text_style(
            theme,
            DemoTextRole::PageTitle,
            theme.palette.text,
        )))
        .with_child(
            Label::new(
                "Every edit lands here immediately. Component stories come from the widget book.",
            )
            .text_style(demo_text_style(
                theme,
                DemoTextRole::Supporting,
                theme.palette.text_muted,
            )),
        )
}

fn section<W>(
    theme: DefaultTheme,
    title: &'static str,
    summary: &'static str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new(title).text_style(demo_text_style(
            theme,
            DemoTextRole::Emphasis,
            theme.palette.text,
        )))
        .with_child(Label::new(summary).text_style(demo_text_style(
            theme,
            DemoTextRole::Supporting,
            theme.palette.text_muted,
        )))
        .with_child(SizedBox::new().height(4.0))
        .with_child(body)
}

fn contrast_report(theme: DefaultTheme) -> impl Widget {
    let mut checks = contrast_checks(&theme);
    let failing = checks.iter().filter(|check| !check.passes()).count();
    checks.sort_by_key(|check| check.passes());
    let summary = if failing == 0 {
        format!("All {} checks pass.", checks.len())
    } else {
        format!(
            "{failing} of {} checks fail; they are listed first.",
            checks.len()
        )
    };
    let cells = checks.into_iter().fold(
        Flex::horizontal()
            .gap(10.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Start),
        |cells, check| cells.with_item(contrast_cell(theme, check), FlexItem::fixed(250.0)),
    );
    NamedSection::new(
        CONTRAST_REPORT_NAME,
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(Label::new("Contrast").text_style(demo_text_style(
                theme,
                DemoTextRole::Emphasis,
                theme.palette.text,
            )))
            .with_child(Label::new(summary).text_style(demo_text_style(
                theme,
                DemoTextRole::Supporting,
                if failing == 0 {
                    theme.palette.text_muted
                } else {
                    theme.palette.danger_soft_text
                },
            )))
            .with_child(SizedBox::new().height(4.0))
            .with_child(cells),
    )
}

fn contrast_cell(theme: DefaultTheme, check: ContrastCheck) -> impl Widget {
    let palette = theme.palette;
    let (tone_fill, tone_ink) = if check.passes() {
        (palette.success_soft, palette.success_soft_text)
    } else {
        (palette.danger_soft, palette.danger_soft_text)
    };
    let sample = Tile::new(
        check.background,
        theme.colors.neutrals.border_subtle,
        6.0,
        SizedBox::new()
            .size(Size::new(44.0, 32.0))
            .with_child(Align::new(
                Alignment::Center,
                Alignment::Center,
                Label::new("Aa").text_style(demo_text_style(
                    theme,
                    DemoTextRole::Body,
                    check.foreground,
                )),
            )),
    );
    let badge = Tile::new(
        tone_fill,
        tone_fill,
        10.0,
        Padding::new(
            Insets {
                left: 8.0,
                top: 1.0,
                right: 8.0,
                bottom: 1.0,
            },
            Label::new(check.badge()).text_style(demo_text_style(
                theme,
                DemoTextRole::Metadata,
                tone_ink,
            )),
        ),
    );
    Flex::horizontal()
        .gap(10.0)
        .align_items(Alignment::Center)
        .with_child(sample)
        .with_item(
            Stack::vertical()
                .spacing(3.0)
                .alignment(Alignment::Start)
                .with_child(Label::new(check.label.clone()).text_style(demo_text_style(
                    theme,
                    DemoTextRole::Supporting,
                    palette.text,
                )))
                .with_child(badge),
            FlexItem::flex(1.0),
        )
}

fn surfaces(theme: DefaultTheme) -> impl Widget {
    let neutrals = theme.colors.neutrals;
    let tiers = [
        ("Window", neutrals.window),
        ("Subtle", neutrals.subtle),
        ("Panel", neutrals.panel),
        ("Overlay", neutrals.overlay),
    ];
    let row = tiers.into_iter().fold(
        Flex::horizontal()
            .gap(12.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Start),
        |row, (name, fill)| row.with_item(surface_tile(theme, name, fill), FlexItem::fixed(200.0)),
    );
    NamedSection::new(SURFACES_SPECIMEN_NAME, row)
}

fn surface_tile(theme: DefaultTheme, name: &'static str, fill: Color) -> impl Widget {
    let neutrals = theme.colors.neutrals;
    let line = |text: &'static str, role: DemoTextRole, color: Color| {
        Label::new(text).text_style(demo_text_style(theme, role, color))
    };
    let field = Tile::new(
        neutrals.field,
        neutrals.border_control,
        theme.metrics.corner_radius,
        Padding::new(
            Insets {
                left: 8.0,
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
            },
            line("Field", DemoTextRole::Supporting, neutrals.text_tertiary),
        ),
    );
    Tile::new(
        fill,
        neutrals.border_subtle,
        theme.metrics.corner_radius + 2.0,
        Padding::all(
            14.0,
            Stack::vertical()
                .spacing(4.0)
                .alignment(Alignment::Stretch)
                .with_child(line(name, DemoTextRole::CardTitle, neutrals.text))
                .with_child(line(
                    "Secondary text",
                    DemoTextRole::Supporting,
                    neutrals.text_secondary,
                ))
                .with_child(line(
                    "Tertiary text",
                    DemoTextRole::Supporting,
                    neutrals.text_tertiary,
                ))
                .with_child(line(
                    "Disabled text",
                    DemoTextRole::Supporting,
                    neutrals.text_disabled,
                ))
                .with_child(SizedBox::new().height(4.0))
                .with_child(Tile::new(
                    neutrals.border,
                    neutrals.border,
                    0.0,
                    SizedBox::new().height(1.0),
                ))
                .with_child(SizedBox::new().height(4.0))
                .with_child(field),
        ),
    )
}

fn decorative(theme: DefaultTheme) -> impl Widget {
    let tags = DecorativeHue::ALL.into_iter().fold(
        Flex::horizontal()
            .gap(8.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Center),
        |tags, hue| {
            let roles = theme.decorative.get(hue);
            tags.with_child(Tile::new(
                roles.soft,
                roles.border,
                12.0,
                Padding::new(
                    Insets {
                        left: 10.0,
                        top: 3.0,
                        right: 10.0,
                        bottom: 3.0,
                    },
                    Label::new(hue_name(hue)).text_style(demo_text_style(
                        theme,
                        DemoTextRole::Supporting,
                        roles.text,
                    )),
                ),
            ))
        },
    );
    let solids = DecorativeHue::ALL.into_iter().enumerate().fold(
        Flex::horizontal()
            .gap(8.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Center),
        |solids, (index, hue)| {
            let roles = theme.decorative.get(hue);
            solids.with_child(Tile::new(
                roles.solid,
                roles.solid,
                10.0,
                SizedBox::new()
                    .size(Size::new(44.0, 44.0))
                    .with_child(Align::new(
                        Alignment::Center,
                        Alignment::Center,
                        Label::new(format!("{}", index + 1)).text_style(demo_text_style(
                            theme,
                            DemoTextRole::Emphasis,
                            roles.on_solid,
                        )),
                    )),
            ))
        },
    );
    NamedSection::new(
        DECORATIVE_SPECIMEN_NAME,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(tags)
            .with_child(solids),
    )
}

fn hue_name(hue: DecorativeHue) -> &'static str {
    match hue {
        DecorativeHue::Red => "Red",
        DecorativeHue::Orange => "Orange",
        DecorativeHue::Amber => "Amber",
        DecorativeHue::Green => "Green",
        DecorativeHue::Teal => "Teal",
        DecorativeHue::Cyan => "Cyan",
        DecorativeHue::Blue => "Blue",
        DecorativeHue::Violet => "Violet",
        DecorativeHue::Magenta => "Magenta",
    }
}

/// A filled, outlined, rounded box around its child, in exact colors rather
/// than theme roles.
struct Tile {
    fill: Color,
    border: Color,
    radius: f32,
    child: SingleChild,
}

impl Tile {
    fn new<W>(fill: Color, border: Color, radius: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            fill,
            border,
            radius,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for Tile {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_rrect_bordered(
            ctx.bounds(),
            [self.radius; 4],
            self.fill,
            Border {
                width: 1.0,
                color: self.border,
            },
        );
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

/// Paints the edited theme's window color behind the preview, following the
/// theme as it changes.
struct ThemeBackdrop {
    theme: Signal<DefaultTheme>,
    child: SingleChild,
}

impl ThemeBackdrop {
    fn new<W>(theme: Signal<DefaultTheme>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for ThemeBackdrop {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = ctx.observe(&self.theme);
        ctx.fill_rect(ctx.bounds(), theme.palette.surface);
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}
