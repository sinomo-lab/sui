use std::rc::Rc;

use sui::{WidgetPodMutVisitor, WidgetPodVisitor, prelude::*};

use crate::app::{DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color};

pub(crate) const LAYOUT_TAB_LABEL: &str = "Layout";
pub(crate) const LAYOUT_DEMO_SCROLL_NAME: &str = "Layout demo scroll";

pub(crate) fn build_layout_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(
            18.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(section(
                    "Stack",
                    "Linear layout with spacing and cross-axis alignment.",
                    build_stack_examples(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Align",
                    "Place one child inside the available space.",
                    build_align_examples(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Flex grow",
                    "Mix fixed, growing, capped, and spacer items in one row.",
                    build_flex_grow_example(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Flex wrap",
                    "Use wrapping and fractional basis values for responsive groups.",
                    build_flex_wrap_example(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Grid and aspect ratio",
                    "Resolve fixed, intrinsic, and fractional tracks while media keeps a stable ratio.",
                    build_grid_example(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Local container query",
                    "This retained view changes policy from its own width, not the window width.",
                    build_constraint_query_example(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Wrapping toolbar",
                    "Toolbar actions retain identity and keyboard order when they flow onto more lines.",
                    build_wrapping_toolbar_example(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                )),
        ))
        .name(LAYOUT_DEMO_SCROLL_NAME),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn section<W>(title: &str, description: &str, body: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new(title)
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.lg,
                    theme_reader().palette.text,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(
            Label::new(description)
                .style(dev_text_style(
                    theme_reader(),
                    theme_reader().text.sm,
                    theme_reader().palette.text_muted,
                ))
                .color_when(dev_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
        .with_child(body)
        .with_child(
            Separator::horizontal()
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .inset(0.0),
        )
}

fn build_stack_examples(theme_reader: DevThemeReader) -> impl Widget {
    Flex::horizontal()
        .gap(12.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            demo_frame(
                Stack::horizontal()
                    .spacing(8.0)
                    .alignment(Alignment::Start)
                    .with_child(tile("A", palette(0), Color::WHITE).height(46.0))
                    .with_child(tile("B", palette(1), Color::WHITE).height(78.0))
                    .with_child(tile("C", palette(2), Color::WHITE).height(58.0)),
                Rc::clone(&theme_reader),
            )
            .height(132.0),
            FlexItem::new().basis_fraction(0.5).min_width(280.0),
        )
        .with_item(
            demo_frame(
                Stack::horizontal()
                    .spacing(8.0)
                    .alignment(Alignment::Center)
                    .with_child(tile("A", palette(3), Color::WHITE).height(46.0))
                    .with_child(tile("B", palette(4), Color::BLACK).height(78.0))
                    .with_child(tile("C", palette(5), Color::WHITE).height(58.0)),
                theme_reader,
            )
            .height(132.0),
            FlexItem::new().basis_fraction(0.5).min_width(280.0),
        )
}

fn build_align_examples(theme_reader: DevThemeReader) -> impl Widget {
    Flex::horizontal()
        .gap(12.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(
            demo_frame(
                Align::new(
                    Alignment::Start,
                    Alignment::Center,
                    tile("start / center", palette(0), Color::WHITE)
                        .width(120.0)
                        .height(44.0),
                ),
                Rc::clone(&theme_reader),
            )
            .height(120.0),
            FlexItem::new().basis_fraction(0.333).min_width(210.0),
        )
        .with_item(
            demo_frame(
                Align::center(
                    tile("center", palette(1), Color::WHITE)
                        .width(96.0)
                        .height(44.0),
                ),
                Rc::clone(&theme_reader),
            )
            .height(120.0),
            FlexItem::new().basis_fraction(0.333).min_width(210.0),
        )
        .with_item(
            demo_frame(
                Align::new(
                    Alignment::Stretch,
                    Alignment::End,
                    tile("stretch / end", palette(2), Color::WHITE).height(44.0),
                ),
                theme_reader,
            )
            .height(120.0),
            FlexItem::new().basis_fraction(0.333).min_width(210.0),
        )
}

fn build_flex_grow_example(theme_reader: DevThemeReader) -> impl Widget {
    demo_frame(
        Flex::horizontal()
            .gap(8.0)
            .align_items(Alignment::Stretch)
            .with_item(
                tile("nav\n180 fixed", palette(3), Color::WHITE),
                FlexItem::fixed(180.0),
            )
            .with_item(
                tile("content\ngrow 2", palette(0), Color::WHITE),
                FlexItem::flex(2.0).min_width(160.0),
            )
            .with_item(
                tile("inspector\ngrow 1 max 220", palette(1), Color::WHITE),
                FlexItem::flex(1.0).min_width(130.0).max_width(220.0),
            )
            .spacer()
            .with_item(
                tile("action\nauto", palette(5), Color::WHITE),
                FlexItem::new().no_shrink(),
            ),
        theme_reader,
    )
    .height(96.0)
}

fn build_flex_wrap_example(theme_reader: DevThemeReader) -> impl Widget {
    demo_frame(
        Flex::horizontal()
            .gap(10.0)
            .wrap(FlexWrap::Wrap)
            .align_items(Alignment::Stretch)
            .align_content(FlexAlignContent::Start)
            .with_item(
                tile("gap-aware\n1/3", palette(0), Color::WHITE),
                FlexItem::new()
                    .basis_gap_aware_fraction(1.0 / 3.0)
                    .min_width(160.0)
                    .min_height(66.0),
            )
            .with_item(
                tile("gap-aware\n1/3", palette(1), Color::WHITE),
                FlexItem::new()
                    .basis_gap_aware_fraction(1.0 / 3.0)
                    .min_width(160.0)
                    .min_height(66.0),
            )
            .with_item(
                tile("gap-aware\n1/3", palette(2), Color::WHITE),
                FlexItem::new()
                    .basis_gap_aware_fraction(1.0 / 3.0)
                    .min_width(160.0)
                    .min_height(66.0),
            )
            .with_item(
                tile("gap-aware\n1/2", palette(3), Color::WHITE),
                FlexItem::new()
                    .basis_gap_aware_fraction(0.5)
                    .min_width(220.0)
                    .min_height(66.0),
            )
            .with_item(
                tile("gap-aware\n1/2", palette(4), Color::BLACK),
                FlexItem::new()
                    .basis_gap_aware_fraction(0.5)
                    .min_width(220.0)
                    .min_height(66.0),
            ),
        theme_reader,
    )
}

fn build_grid_example(theme_reader: DevThemeReader) -> impl Widget {
    demo_frame(
        Grid::new([
            GridTrack::Fixed(132.0),
            GridTrack::Fraction(1.0),
            GridTrack::Fraction(1.0),
        ])
        .rows([GridTrack::Auto, GridTrack::Auto])
        .gap(10.0)
        .with_cell(
            GridCell::new(0, 0).span(2, 1),
            AspectRatio::new(
                1.0,
                tile("1 : 1", palette(5), Color::WHITE).min_size(Size::new(80.0, 80.0)),
            ),
        )
        .with_cell(
            GridCell::new(0, 1).span(1, 2),
            tile("spans two fractional columns", palette(0), Color::WHITE),
        )
        .with_cell(GridCell::new(1, 1), tile("1fr", palette(1), Color::WHITE))
        .with_cell(GridCell::new(1, 2), tile("1fr", palette(2), Color::WHITE)),
        theme_reader,
    )
}

fn build_constraint_query_example(theme_reader: DevThemeReader) -> impl Widget {
    let compact_theme = Rc::clone(&theme_reader);
    demo_frame(
        ConstraintView::new(
            Stack::vertical()
                .spacing(8.0)
                .with_child(tile("compact header", palette(3), Color::WHITE))
                .with_child(tile("stacked content", palette(0), Color::WHITE)),
        )
        .when(
            ConstraintQuery::new().min_width(680.0),
            Grid::new([GridTrack::Fixed(180.0), GridTrack::Fraction(1.0)])
                .gap(10.0)
                .with_child(tile("wide sidebar", palette(3), Color::WHITE))
                .with_child(tile("wide content", palette(0), Color::WHITE)),
        ),
        compact_theme,
    )
}

fn build_wrapping_toolbar_example(theme_reader: DevThemeReader) -> impl Widget {
    demo_frame(
        Toolbar::horizontal()
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .wrapping()
            .line_spacing(8.0)
            .divider(false)
            .with_child(Button::primary("Run"))
            .with_child(Button::new("Format"))
            .with_child(Button::new("Inspect"))
            .with_child(Button::new("Share"))
            .with_child(Button::new("More actions")),
        theme_reader,
    )
}

fn demo_frame<W>(child: W, theme_reader: DevThemeReader) -> LayoutTile
where
    W: Widget + 'static,
{
    LayoutTile::new(
        dev_theme_color(&theme_reader, |theme| theme.surfaces.panel),
        Some(dev_theme_color(&theme_reader, |theme| theme.palette.border)),
        child,
    )
    .padding(12.0)
}

fn tile(label: &'static str, fill: Color, text: Color) -> LayoutTile {
    let theme = DefaultTheme::default();
    LayoutTile::new(
        move || fill,
        Some(move || Color::WHITE.with_alpha(0.26)),
        Align::center(Padding::all(
            6.0,
            Label::new(label).style(dev_text_style(theme, theme.text.xs, text)),
        )),
    )
    .min_size(Size::new(72.0, 38.0))
    .padding(0.0)
}

fn palette(index: usize) -> Color {
    const COLORS: [Color; 6] = [
        Color::rgba(0.16, 0.48, 0.86, 1.0),
        Color::rgba(0.08, 0.58, 0.42, 1.0),
        Color::rgba(0.68, 0.26, 0.32, 1.0),
        Color::rgba(0.75, 0.42, 0.12, 1.0),
        Color::rgba(0.96, 0.76, 0.20, 1.0),
        Color::rgba(0.35, 0.38, 0.82, 1.0),
    ];

    COLORS[index % COLORS.len()]
}

struct LayoutTile {
    fill: Box<dyn Fn() -> Color>,
    border: Option<Box<dyn Fn() -> Color>>,
    radius: f32,
    padding: f32,
    width: Option<f32>,
    height: Option<f32>,
    min_size: Size,
    child: SingleChild,
}

impl LayoutTile {
    fn new<W, F, B>(fill: F, border: Option<B>, child: W) -> Self
    where
        W: Widget + 'static,
        F: Fn() -> Color + 'static,
        B: Fn() -> Color + 'static,
    {
        Self {
            fill: Box::new(fill),
            border: border.map(|border| Box::new(border) as Box<dyn Fn() -> Color>),
            radius: 8.0,
            padding: 0.0,
            width: None,
            height: None,
            min_size: Size::new(96.0, 52.0),
            child: SingleChild::new(child),
        }
    }

    fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    fn min_size(mut self, min_size: Size) -> Self {
        self.min_size = Size::new(min_size.width.max(0.0), min_size.height.max(0.0));
        self
    }

    fn content_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + self.padding,
            bounds.y() + self.padding,
            (bounds.width() - (self.padding * 2.0)).max(0.0),
            (bounds.height() - (self.padding * 2.0)).max(0.0),
        )
    }
}

impl Widget for LayoutTile {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let inset = self.padding * 2.0;
        let child_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                (constraints.max.width - inset).max(0.0),
                (constraints.max.height - inset).max(0.0),
            ),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        let measured = Size::new(
            self.width
                .unwrap_or((child_size.width + inset).max(self.min_size.width)),
            self.height
                .unwrap_or((child_size.height + inset).max(self.min_size.height)),
        );

        constraints.clamp(measured)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, self.content_bounds(bounds));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let path = Path::rounded_rect(bounds, self.radius);
        ctx.fill(path.clone(), (self.fill)());
        if let Some(border) = &self.border {
            ctx.stroke(path, border(), StrokeStyle::new(1.0));
        }
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
