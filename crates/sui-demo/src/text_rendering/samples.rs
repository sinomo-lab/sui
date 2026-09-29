//! Text drawn on fixed backgrounds, with or without a render policy of its
//! own, for the page to compare.

use std::{cell::Cell, rc::Rc};

use sui::prelude::*;
use sui::{Rect, SemanticsNode, SemanticsRole, TextRenderPolicy, TextStyle, paint_text_line};

use super::POLICIES;

/// The sample every specimen shows: round and straight stems, figures, and
/// the thin, close strokes of "ill minimum".
pub(crate) const SAMPLE_TEXT: &str = "Hamburgefonstiv 0123 ill minimum";

/// A background and the text color drawn on it. Specimens fix their colors,
/// so a sample looks the same in every theme.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Surface {
    pub(crate) name: &'static str,
    pub(crate) background: Color,
    pub(crate) text: Color,
}

pub(crate) const LIGHT: Surface = Surface {
    name: "light",
    background: Color::rgba(0.99, 0.99, 1.0, 1.0),
    text: Color::rgba(0.10, 0.12, 0.16, 1.0),
};
pub(crate) const DARK: Surface = Surface {
    name: "dark",
    background: Color::rgba(0.11, 0.13, 0.17, 1.0),
    text: Color::rgba(0.93, 0.95, 0.98, 1.0),
};
pub(crate) const BLUE: Surface = Surface {
    name: "blue",
    background: Color::rgba(0.13, 0.36, 0.86, 1.0),
    text: Color::rgba(1.0, 1.0, 1.0, 1.0),
};

const PADDING: f32 = 12.0;
const ROW_GAP: f32 = 6.0;
const COLUMN_GAP: f32 = 8.0;
/// The part of a specimen the magnifier shows, from its top left.
const ZOOM_SIZE: Size = Size::new(220.0, 56.0);

fn line_height(size: f32) -> f32 {
    (size * 1.35).ceil()
}

fn text_style(size: f32, color: Color) -> TextStyle {
    let mut style = TextStyle::new(color);
    style.font_size = size;
    style.line_height = line_height(size);
    style
}

/// Where a specimen's text policy comes from.
#[derive(Clone)]
pub(crate) enum PolicySource {
    /// The window's settings.
    Window,
    /// A policy of its own.
    Fixed(TextRenderPolicy),
    /// The one chosen in a select, as an index into [`POLICIES`].
    Chosen(Signal<usize>),
}

impl PolicySource {
    fn policy(&self, ctx: &PaintCtx) -> Option<TextRenderPolicy> {
        match self {
            Self::Window => None,
            Self::Fixed(policy) => Some(*policy),
            Self::Chosen(choice) => POLICIES
                .get(ctx.observe(choice))
                .and_then(|policy| policy.policy),
        }
    }

    fn describe(&self) -> &'static str {
        match self {
            Self::Window => "the window's settings",
            Self::Fixed(_) => "a policy of its own",
            Self::Chosen(choice) => POLICIES
                .get(choice.get())
                .map_or("the window's settings", |policy| policy.name),
        }
    }
}

/// Where the magnifier should look: a specimen's top left corner, in
/// physical pixels of the window, as of its last paint.
pub(crate) type ZoomRegion = Rc<Cell<Option<Rect>>>;

/// The sample at each of `sizes`, in a column per surface.
pub(crate) struct Specimen {
    name: String,
    sizes: &'static [f32],
    surfaces: &'static [Surface],
    policy: PolicySource,
    zoom: Option<ZoomRegion>,
    /// How many surfaces fit side by side, as of the last measure.
    per_row: usize,
}

impl Specimen {
    pub(crate) fn new(
        name: impl Into<String>,
        sizes: &'static [f32],
        surfaces: &'static [Surface],
    ) -> Self {
        Self {
            name: name.into(),
            sizes,
            surfaces,
            policy: PolicySource::Window,
            zoom: None,
            per_row: 1,
        }
    }

    pub(crate) fn policy(mut self, policy: PolicySource) -> Self {
        self.policy = policy;
        self
    }

    /// Record the region the magnifier shows in `zoom` as it paints.
    pub(crate) fn zoom(mut self, zoom: ZoomRegion) -> Self {
        self.zoom = Some(zoom);
        self
    }

    /// The height of one surface's column.
    fn column_height(&self) -> f32 {
        let rows = self
            .sizes
            .iter()
            .map(|size| line_height(*size))
            .sum::<f32>();
        PADDING * 2.0 + rows + ROW_GAP * self.sizes.len().saturating_sub(1) as f32
    }

    /// Where surface `index` is drawn in `bounds`.
    fn column(&self, bounds: Rect, index: usize) -> Rect {
        let per_row = self.per_row.max(1);
        let width =
            ((bounds.width() - COLUMN_GAP * (per_row - 1) as f32) / per_row as f32).max(0.0);
        let height = self.column_height();
        Rect::new(
            bounds.x() + (width + COLUMN_GAP) * (index % per_row) as f32,
            bounds.y() + (height + COLUMN_GAP) * (index / per_row) as f32,
            width,
            height,
        )
    }
}

impl Widget for Specimen {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        // Surfaces sit side by side as long as their widest line fits, and
        // wrap onto more rows when it does not.
        let widest = self
            .sizes
            .iter()
            .filter_map(|size| {
                ctx.layout()
                    .measure_text(
                        format!("{size} px  {SAMPLE_TEXT}"),
                        text_style(*size, Color::BLACK),
                    )
                    .ok()
            })
            .map(|measurement| measurement.width)
            .fold(0.0, f32::max)
            + PADDING * 2.0;
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            (widest + COLUMN_GAP) * self.surfaces.len() as f32
        };
        let count = self.surfaces.len().max(1);
        self.per_row =
            (((width + COLUMN_GAP) / (widest + COLUMN_GAP)).floor() as usize).clamp(1, count);
        let rows = count.div_ceil(self.per_row);
        let height = self.column_height() * rows as f32 + COLUMN_GAP * (rows - 1) as f32;
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let policy = self.policy.policy(ctx);
        for (index, surface) in self.surfaces.iter().enumerate() {
            let column = self.column(bounds, index);
            ctx.fill_rect(column, surface.background);
            ctx.push_clip_rect(column);
            if let Some(policy) = policy {
                ctx.push_text_render_policy(policy);
            }
            let mut y = column.y() + PADDING;
            for size in self.sizes {
                let height = line_height(*size);
                paint_text_line(
                    ctx,
                    Rect::new(
                        column.x() + PADDING,
                        y,
                        (column.width() - PADDING * 2.0).max(0.0),
                        height,
                    ),
                    &format!("{size} px  {SAMPLE_TEXT}"),
                    &text_style(*size, surface.text),
                    TextAlign::Start,
                );
                y += height + ROW_GAP;
            }
            if policy.is_some() {
                ctx.pop_text_render_policy();
            }
            ctx.pop_clip();
        }

        if let Some(zoom) = &self.zoom {
            let first = self.column(bounds, 0);
            let region = Rect::new(
                first.x(),
                first.y(),
                ZOOM_SIZE.width.min(first.width()),
                ZOOM_SIZE.height.min(first.height()),
            );
            let scale = ctx.dpi().scale_factor;
            let window = ctx.presentation_transform().transform_rect_bbox(region);
            zoom.set(Some(Rect::new(
                (window.x() * scale).round(),
                (window.y() * scale).round(),
                (window.width() * scale).round(),
                (window.height() * scale).round(),
            )));
            ctx.stroke_rect(
                region,
                Color::rgba(0.9, 0.2, 0.5, 0.9),
                StrokeStyle::new(ctx.dpi().hairline_width()),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        let sizes = self
            .sizes
            .iter()
            .map(|size| format!("{size}"))
            .collect::<Vec<_>>()
            .join(", ");
        let surfaces = self
            .surfaces
            .iter()
            .map(|surface| surface.name)
            .collect::<Vec<_>>()
            .join(" and ");
        node.description = Some(format!(
            "\"{SAMPLE_TEXT}\" at {sizes} px on {surfaces}, drawn with {}.",
            self.policy.describe()
        ));
        ctx.push(node);
    }
}

/// The same sample drawn twice side by side, each half with its own policy
/// and caption.
pub(crate) struct PolicyPair {
    name: &'static str,
    sides: [(&'static str, PolicySource); 2],
    sizes: &'static [f32],
    surface: Surface,
    caption_color: Rc<dyn Fn() -> Color>,
}

impl PolicyPair {
    pub(crate) fn new(
        name: &'static str,
        left: (&'static str, PolicySource),
        right: (&'static str, PolicySource),
        sizes: &'static [f32],
        surface: Surface,
        caption_color: Rc<dyn Fn() -> Color>,
    ) -> Self {
        Self {
            name,
            sides: [left, right],
            sizes,
            surface,
            caption_color,
        }
    }

    fn caption_style(&self) -> TextStyle {
        text_style(12.0, (self.caption_color)())
    }
}

impl Widget for PolicyPair {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            640.0
        };
        let rows = self
            .sizes
            .iter()
            .map(|size| line_height(*size) + ROW_GAP)
            .sum::<f32>();
        let caption = self.caption_style().line_height + ROW_GAP;
        constraints.clamp(Size::new(width, caption + PADDING * 2.0 + rows))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let caption = self.caption_style();
        let half = ((bounds.width() - COLUMN_GAP) * 0.5).max(0.0);
        for (index, (label, source)) in self.sides.iter().enumerate() {
            let x = bounds.x() + (half + COLUMN_GAP) * index as f32;
            paint_text_line(
                ctx,
                Rect::new(x, bounds.y(), half, caption.line_height),
                label,
                &caption,
                TextAlign::Start,
            );
            let panel = Rect::new(
                x,
                bounds.y() + caption.line_height + ROW_GAP,
                half,
                (bounds.height() - caption.line_height - ROW_GAP).max(0.0),
            );
            ctx.fill_rect(panel, self.surface.background);
            ctx.push_clip_rect(panel);
            let policy = source.policy(ctx);
            if let Some(policy) = policy {
                ctx.push_text_render_policy(policy);
            }
            let mut y = panel.y() + PADDING;
            for size in self.sizes {
                let height = line_height(*size);
                paint_text_line(
                    ctx,
                    Rect::new(
                        panel.x() + PADDING,
                        y,
                        panel.width() - PADDING * 2.0,
                        height,
                    ),
                    &format!("{size} px  {SAMPLE_TEXT}"),
                    &text_style(*size, self.surface.text),
                    TextAlign::Start,
                );
                y += height + ROW_GAP;
            }
            if policy.is_some() {
                ctx.pop_text_render_policy();
            }
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        node.description = Some(format!(
            "Left: {}. Right: {}.",
            self.sides[0].0, self.sides[1].0
        ));
        ctx.push(node);
    }
}

/// One word drawn four times, a quarter pixel further right each time.
pub(crate) struct SubpixelDrift {
    name: &'static str,
    surface: Surface,
}

impl SubpixelDrift {
    const SIZE: f32 = 13.0;
    const COPIES: usize = 4;

    pub(crate) fn new(name: &'static str, surface: Surface) -> Self {
        Self { name, surface }
    }
}

impl Widget for SubpixelDrift {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let height = PADDING * 2.0 + (line_height(Self::SIZE) + ROW_GAP) * Self::COPIES as f32;
        constraints.clamp(Size::new(360.0, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill_rect(bounds, self.surface.background);
        let style = text_style(Self::SIZE, self.surface.text);
        let mut y = bounds.y() + PADDING;
        for copy in 0..Self::COPIES {
            let offset = copy as f32 * 0.25;
            paint_text_line(
                ctx,
                Rect::new(
                    bounds.x() + PADDING + offset,
                    y,
                    bounds.width() - PADDING * 2.0,
                    style.line_height,
                ),
                &format!("minimum illumination  +{offset:.2} px"),
                &style,
                TextAlign::Start,
            );
            y += style.line_height + ROW_GAP;
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        node.description =
            Some("\"minimum illumination\" drawn at +0, +0.25, +0.5, and +0.75 px.".to_string());
        ctx.push(node);
    }
}

/// Labels centered in boxes of different heights, with each box's center
/// line drawn through them.
pub(crate) struct CenteringBoxes {
    name: &'static str,
    surface: Surface,
}

impl CenteringBoxes {
    const HEIGHTS: [f32; 3] = [24.0, 32.0, 44.0];
    const LABELS: [&'static str; 3] = ["Compact", "Label Hx", "Title Ag"];
    const SIZES: [f32; 3] = [12.0, 14.0, 20.0];

    pub(crate) fn new(name: &'static str, surface: Surface) -> Self {
        Self { name, surface }
    }
}

impl Widget for CenteringBoxes {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(420.0, Self::HEIGHTS[2] + PADDING * 2.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        ctx.fill_rect(bounds, self.surface.background);
        let guide = Color::rgba(0.9, 0.2, 0.5, 0.8);
        let hairline = ctx.dpi().hairline_width();
        let mut x = bounds.x() + PADDING;
        for ((height, label), size) in Self::HEIGHTS.iter().zip(Self::LABELS).zip(Self::SIZES) {
            let slot = Rect::new(x, bounds.y() + PADDING, 120.0, *height);
            ctx.stroke_rect(
                slot,
                self.surface.text.with_alpha(0.35),
                StrokeStyle::new(hairline),
            );
            let center = slot.y() + slot.height() * 0.5;
            ctx.fill_rect(
                Rect::new(slot.x(), center - hairline * 0.5, slot.width(), hairline),
                guide,
            );
            paint_text_line(
                ctx,
                slot,
                label,
                &text_style(size, self.surface.text),
                TextAlign::Center,
            );
            x += slot.width() + COLUMN_GAP * 2.0;
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        node.description = Some(
            "Three labels centered in boxes 24, 32, and 44 px tall, with each box's center line."
                .to_string(),
        );
        ctx.push(node);
    }
}
