//! Layout primitives for component specimens: labeled grids that compare
//! variations side by side, wrapping strips, and the stage each story sits on.

use sui::prelude::*;
use sui::{Rect, WidgetPodMutVisitor, WidgetPodVisitor};

use crate::app::{DemoTextRole, demo_text_style};

pub(crate) type BoxedWidget = Box<dyn Widget>;

pub(crate) fn boxed<W>(widget: W) -> BoxedWidget
where
    W: Widget + 'static,
{
    Box::new(widget)
}

/// One captioned block of specimens inside a story's stage.
pub(crate) struct Section {
    pub(crate) caption: Option<String>,
    pub(crate) content: BoxedWidget,
}

impl Section {
    pub(crate) fn new<W>(caption: impl Into<String>, content: W) -> Self
    where
        W: Widget + 'static,
    {
        let caption = caption.into();
        Self {
            caption: (!caption.is_empty()).then_some(caption),
            content: boxed(content),
        }
    }
}

/// A matrix of variations: one row per variant, one column per state or tone.
pub(crate) fn grid(
    theme: DefaultTheme,
    caption: &str,
    columns: &[&str],
    rows: Vec<(&str, Vec<BoxedWidget>)>,
) -> Section {
    Section::new(caption, SpecimenGrid::new(theme, columns, rows, false))
}

/// A wrapping row of captioned specimens.
pub(crate) fn strip(
    theme: DefaultTheme,
    caption: &str,
    items: Vec<(&str, BoxedWidget)>,
) -> Section {
    let (labels, widgets): (Vec<&str>, Vec<BoxedWidget>) = items.into_iter().unzip();
    Section::new(
        caption,
        SpecimenGrid::new(theme, &labels, vec![("", widgets)], true),
    )
}

/// A single free-form specimen, such as a composed example or a data view.
pub(crate) fn example<W>(caption: &str, content: W) -> Section
where
    W: Widget + 'static,
{
    Section::new(caption, content)
}

fn caption_label(theme: DefaultTheme, text: &str) -> Label {
    Label::new(text).style(demo_text_style(
        theme,
        DemoTextRole::Metadata,
        theme.palette.text_muted,
    ))
}

fn row_label(theme: DefaultTheme, text: &str) -> Label {
    Label::new(text).style(demo_text_style(
        theme,
        DemoTextRole::Supporting,
        theme.palette.text_muted,
    ))
}

/// Lays specimens out as a labeled grid when there is room and falls back to
/// wrapping rows of captioned cells when the grid would overflow. Flow mode
/// always wraps, which suits single-row strips.
///
/// Children are stored per row as `[row label, (caption, cell) × columns]`.
/// Grid mode shows the first row's captions as column headers; the stacked
/// fallback captions every cell so states stay identifiable.
pub(crate) struct SpecimenGrid {
    columns: usize,
    rows: usize,
    flow: bool,
    has_row_labels: bool,
    has_captions: bool,
    children: WidgetChildren,
    placements: Vec<Option<Rect>>,
}

impl SpecimenGrid {
    const COLUMN_GAP: f32 = 24.0;
    const ROW_GAP: f32 = 16.0;
    const CAPTION_GAP: f32 = 8.0;
    const LABEL_GAP: f32 = 20.0;
    const MAX_CELL_WIDTH: f32 = 420.0;

    pub(crate) fn new(
        theme: DefaultTheme,
        columns: &[&str],
        rows: Vec<(&str, Vec<BoxedWidget>)>,
        flow: bool,
    ) -> Self {
        let column_count = rows
            .iter()
            .map(|(_, cells)| cells.len())
            .max()
            .unwrap_or(0)
            .max(columns.len());
        let has_row_labels = rows.iter().any(|(label, _)| !label.is_empty());
        let has_captions = columns.iter().any(|caption| !caption.is_empty());
        let mut children = WidgetChildren::new();
        let row_count = rows.len();
        for (label, cells) in rows {
            children.push(row_label(theme, label));
            let mut cells = cells.into_iter();
            for column in 0..column_count {
                children.push(caption_label(
                    theme,
                    columns.get(column).copied().unwrap_or(""),
                ));
                match cells.next() {
                    Some(cell) => children.push_pod(WidgetPod::new_boxed(cell)),
                    None => children.push(SizedBox::new()),
                }
            }
        }
        Self {
            columns: column_count,
            rows: row_count,
            flow,
            has_row_labels,
            has_captions,
            placements: vec![None; children.len()],
            children,
        }
    }

    fn stride(&self) -> usize {
        1 + self.columns * 2
    }

    fn label_index(&self, row: usize) -> usize {
        row * self.stride()
    }

    fn caption_index(&self, row: usize, column: usize) -> usize {
        row * self.stride() + 1 + column * 2
    }

    fn cell_index(&self, row: usize, column: usize) -> usize {
        row * self.stride() + 2 + column * 2
    }

    fn size_of(&self, index: usize) -> Size {
        self.children.as_slice()[index].measured_size()
    }

    /// Places every child in grid mode, returning the grid size.
    fn layout_grid(&mut self) -> Size {
        let label_width = if self.has_row_labels {
            (0..self.rows)
                .map(|row| self.size_of(self.label_index(row)).width)
                .fold(0.0, f32::max)
        } else {
            0.0
        };
        let label_extent = if self.has_row_labels {
            label_width + Self::LABEL_GAP
        } else {
            0.0
        };
        let column_widths: Vec<f32> = (0..self.columns)
            .map(|column| {
                let caption = if self.has_captions {
                    self.size_of(self.caption_index(0, column)).width
                } else {
                    0.0
                };
                (0..self.rows)
                    .map(|row| self.size_of(self.cell_index(row, column)).width)
                    .fold(caption, f32::max)
            })
            .collect();
        let header_height = if self.has_captions {
            (0..self.columns)
                .map(|column| self.size_of(self.caption_index(0, column)).height)
                .fold(0.0, f32::max)
                + Self::CAPTION_GAP
        } else {
            0.0
        };

        let mut x = label_extent;
        let column_x: Vec<f32> = column_widths
            .iter()
            .map(|width| {
                let start = x;
                x += width + Self::COLUMN_GAP;
                start
            })
            .collect();
        let width = (x - Self::COLUMN_GAP).max(label_extent);

        for placement in &mut self.placements {
            *placement = None;
        }
        if self.has_captions {
            for (column, x) in column_x.iter().enumerate() {
                let index = self.caption_index(0, column);
                self.placements[index] = Some(Rect::from_origin_size(
                    Point::new(*x, 0.0),
                    self.size_of(index),
                ));
            }
        }

        let mut y = header_height;
        for row in 0..self.rows {
            let label = self.label_index(row);
            let row_height = (0..self.columns)
                .map(|column| self.size_of(self.cell_index(row, column)).height)
                .fold(self.size_of(label).height, f32::max);
            if self.has_row_labels {
                let size = self.size_of(label);
                self.placements[label] = Some(Rect::from_origin_size(
                    Point::new(0.0, y + (row_height - size.height) * 0.5),
                    size,
                ));
            }
            for (column, x) in column_x.iter().enumerate() {
                let index = self.cell_index(row, column);
                let size = self.size_of(index);
                self.placements[index] = Some(Rect::from_origin_size(
                    Point::new(*x, y + (row_height - size.height) * 0.5),
                    size,
                ));
            }
            y += row_height + Self::ROW_GAP;
        }
        Size::new(width, (y - Self::ROW_GAP).max(header_height))
    }

    /// Places every child as wrapping rows of captioned cells.
    fn layout_flow(&mut self, available_width: f32) -> Size {
        for placement in &mut self.placements {
            *placement = None;
        }
        let mut y = 0.0;
        let mut widest: f32 = 0.0;
        for row in 0..self.rows {
            let label = self.label_index(row);
            if self.has_row_labels && self.size_of(label).width > 0.0 {
                let size = self.size_of(label);
                self.placements[label] = Some(Rect::from_origin_size(Point::new(0.0, y), size));
                y += size.height + Self::CAPTION_GAP;
            }
            let mut x = 0.0;
            let mut line_height: f32 = 0.0;
            for column in 0..self.columns {
                let caption = self.caption_index(row, column);
                let cell = self.cell_index(row, column);
                let caption_size = self.size_of(caption);
                let cell_size = self.size_of(cell);
                let captioned = caption_size.width > 0.0;
                let unit_width =
                    cell_size
                        .width
                        .max(if captioned { caption_size.width } else { 0.0 });
                let unit_height = cell_size.height
                    + if captioned {
                        caption_size.height + Self::CAPTION_GAP
                    } else {
                        0.0
                    };
                if x > 0.0 && x + unit_width > available_width {
                    x = 0.0;
                    y += line_height + Self::ROW_GAP;
                    line_height = 0.0;
                }
                let mut cell_y = y;
                if captioned {
                    self.placements[caption] =
                        Some(Rect::from_origin_size(Point::new(x, y), caption_size));
                    cell_y += caption_size.height + Self::CAPTION_GAP;
                }
                self.placements[cell] =
                    Some(Rect::from_origin_size(Point::new(x, cell_y), cell_size));
                x += unit_width + Self::COLUMN_GAP;
                widest = widest.max(x - Self::COLUMN_GAP);
                line_height = line_height.max(unit_height);
            }
            y += line_height + Self::ROW_GAP * 1.5;
        }
        Size::new(widest, (y - Self::ROW_GAP * 1.5).max(0.0))
    }
}

impl Widget for SpecimenGrid {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let available_width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            f32::INFINITY
        };
        let cell_constraints = Constraints::new(
            Size::ZERO,
            Size::new(available_width.min(Self::MAX_CELL_WIDTH), f32::INFINITY),
        );
        for child in self.children.as_mut_slice() {
            child.measure(ctx, cell_constraints);
        }
        let size = if self.flow {
            self.layout_flow(available_width)
        } else {
            let grid = self.layout_grid();
            if grid.width <= available_width + 0.5 {
                grid
            } else {
                self.layout_flow(available_width)
            }
        };
        constraints.clamp(size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let origin = bounds.origin.to_vector();
        for (child, placement) in self
            .children
            .as_mut_slice()
            .iter_mut()
            .zip(&self.placements)
        {
            let rect = placement
                .map(|rect| rect.translate(origin))
                .unwrap_or_else(|| Rect::from_origin_size(bounds.origin, Size::ZERO));
            child.arrange(ctx, rect);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for (child, placement) in self.children.as_slice().iter().zip(&self.placements) {
            if placement.is_some() {
                child.paint(ctx);
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        for (child, placement) in self.children.as_slice().iter().zip(&self.placements) {
            if placement.is_some() {
                child.semantics(ctx);
            }
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for (child, placement) in self.children.as_slice().iter().zip(&self.placements) {
            if placement.is_some() {
                visitor.visit(child);
            }
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for (child, placement) in self
            .children
            .as_mut_slice()
            .iter_mut()
            .zip(&self.placements)
        {
            if placement.is_some() {
                visitor.visit(child);
            }
        }
    }
}

/// The quiet, bordered well every story's specimens sit on.
pub(crate) struct Stage {
    theme: DefaultTheme,
    padding: Insets,
    child: SingleChild,
}

impl Stage {
    const RADIUS: f32 = 10.0;

    pub(crate) fn new<W>(theme: DefaultTheme, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme,
            padding: Insets::all(20.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for Stage {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let horizontal = self.padding.left + self.padding.right;
        let vertical = self.padding.top + self.padding.bottom;
        let max_width = if constraints.max.width.is_finite() {
            (constraints.max.width - horizontal).max(0.0)
        } else {
            f32::INFINITY
        };
        let child = self.child.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(max_width, f32::INFINITY)),
        );
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            child.width + horizontal
        };
        constraints.clamp(Size::new(width, child.height + vertical))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let size = self.child.child().measured_size();
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x() + self.padding.left,
                bounds.y() + self.padding.top,
                size.width
                    .min((bounds.width() - self.padding.left - self.padding.right).max(0.0)),
                size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let neutrals = self.theme.colors.neutrals;
        let path = Path::rounded_rect(bounds.inflate(-0.5, -0.5), Self::RADIUS);
        ctx.fill(path.clone(), neutrals.subtle);
        ctx.stroke(path, neutrals.border_subtle, StrokeStyle::new(1.0));
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

/// A filled circle used as a category marker.
pub(crate) struct ColorDot {
    color: Box<dyn Fn() -> Color>,
    diameter: f32,
}

impl ColorDot {
    pub(crate) fn new<F>(diameter: f32, color: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        Self {
            color: Box::new(color),
            diameter,
        }
    }
}

impl Widget for ColorDot {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.diameter, self.diameter))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let center = Point::new(
            bounds.x() + bounds.width() * 0.5,
            bounds.y() + bounds.height() * 0.5,
        );
        ctx.fill(
            Path::circle(center, bounds.width().min(bounds.height()) * 0.5),
            (self.color)(),
        );
    }
}
