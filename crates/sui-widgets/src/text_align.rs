//! Painting text in a rect: one line, a wrapped paragraph, or a paragraph
//! laid out during measurement.

use sui_core::{Color, Point, Rect, Size};
use sui_layout::LayoutContext;
use sui_runtime::{MeasureCtx, PaintCtx, window_render_options};
use sui_text::{
    TextAlign, TextDocument, TextLayout, TextLayoutRequest, TextMeasurement, TextStyle, TextWrap,
};

pub(crate) struct AlignedTextLayout {
    pub(crate) rect: Rect,
    origin: Point,
    layout: TextLayout,
    color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HorizontalTextAlignmentMode {
    Advance,
    Optical,
}

#[derive(Debug, Clone, Copy)]
struct HorizontalPlacement {
    rect_x: f32,
    rect_width: f32,
    origin_x: f32,
}

pub(crate) fn aligned_text_rect_for_text(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
) -> Rect {
    aligned_text_rect_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        HorizontalTextAlignmentMode::Advance,
    )
}

pub(crate) fn aligned_text_rect_for_text_with_mode(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> Rect {
    if let Some(aligned) = aligned_text_layout_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    ) {
        return aligned.rect;
    }

    let fallback_measurement = || TextMeasurement {
        width: text.chars().count() as f32 * style.font_size * 0.56,
        height: style.line_height,
        bounds: Rect::ZERO,
        ascent: style.font_size,
        descent: 0.0,
        cap_height: Some(style.font_size),
    };
    let measurement = ctx
        .measure_text(text.to_string(), style.clone())
        .ok()
        .unwrap_or_else(fallback_measurement);
    let placement = horizontal_placement(rect, measurement, horizontal_alignment, horizontal_mode);
    let height = line_height.max(measurement.height).min(rect.height());
    let y = vertically_centered_text_rect_y(ctx, rect, measurement, height);

    Rect::new(placement.rect_x, y, placement.rect_width, height)
}

pub(crate) fn aligned_text_layout_for_text_with_mode(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> Option<AlignedTextLayout> {
    aligned_text_layout_for_text_with_mode_and_wrap(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
        TextWrap::Word,
    )
}

fn aligned_text_layout_for_text_with_mode_and_wrap(
    ctx: &PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
    wrap: TextWrap,
) -> Option<AlignedTextLayout> {
    let color = style.color;
    let mut layout_style = style.clone();
    layout_style.color = Color::WHITE;
    let mut document = TextDocument::from_plain_text(text.to_string(), layout_style);
    let paragraph_align = paragraph_alignment(horizontal_alignment, horizontal_mode);
    for paragraph in &mut document.paragraphs {
        paragraph.style.align = paragraph_align;
        paragraph.style.wrap = wrap;
    }
    let layout = ctx
        .layout_text_document(
            TextLayoutRequest::new(document)
                .with_box_size(Size::new(rect.width().max(1.0), rect.height().max(1.0))),
        )
        .ok()?;
    let measurement = layout.measurement();
    let placement = horizontal_placement(rect, measurement, horizontal_alignment, horizontal_mode);
    let aligned_rect = aligned_text_rect_for_layout_with_mode(
        ctx,
        rect,
        &layout,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    );
    Some(AlignedTextLayout {
        origin: Point::new(placement.origin_x, aligned_rect.y()),
        rect: aligned_rect,
        layout,
        color,
    })
}

/// Where text goes vertically in the rect it is painted into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    /// The first line starts at the top.
    Top,
    /// A single line centers on its capital letters, or on its ascent when
    /// optical text centering is off. Several lines center their line boxes,
    /// so a rect exactly as tall as the text leaves it where it was laid out.
    #[default]
    Center,
    /// The last line ends at the bottom.
    Bottom,
}

/// Where text goes in the rect it is painted into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPlacement {
    pub horizontal: TextAlign,
    pub vertical: VerticalAlign,
}

impl TextPlacement {
    pub const fn new(horizontal: TextAlign, vertical: VerticalAlign) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    /// At the start, centered vertically: labels in controls.
    pub const START: Self = Self::new(TextAlign::Start, VerticalAlign::Center);
    /// Centered both ways.
    pub const CENTER: Self = Self::new(TextAlign::Center, VerticalAlign::Center);
    /// At the end, centered vertically.
    pub const END: Self = Self::new(TextAlign::End, VerticalAlign::Center);
    /// From the top start corner: running text.
    pub const TOP_START: Self = Self::new(TextAlign::Start, VerticalAlign::Top);
}

impl From<TextAlign> for TextPlacement {
    /// Centered vertically.
    fn from(horizontal: TextAlign) -> Self {
        Self::new(horizontal, VerticalAlign::Center)
    }
}

/// Where along the rect a line or block goes, from 0 (left) to 1 (right).
pub(crate) const fn alignment_fraction(align: TextAlign) -> f32 {
    match align {
        TextAlign::Start | TextAlign::Left | TextAlign::Justified => 0.0,
        TextAlign::Center => 0.5,
        TextAlign::End | TextAlign::Right => 1.0,
    }
}

/// Lays text out: implemented by the measure and paint contexts, and by
/// [`LayoutContext`].
pub trait TextShaper {
    fn shape_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout>;
}

impl TextShaper for LayoutContext {
    fn shape_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout> {
        self.layout_document(request)
    }
}

impl TextShaper for MeasureCtx {
    fn shape_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout> {
        self.layout().layout_document(request)
    }
}

impl TextShaper for PaintCtx {
    fn shape_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout> {
        self.layout_text_document(request)
    }
}

/// Text wrapped to a width and laid out once, to measure and then paint.
///
/// Lay a paragraph out in `measure` and keep it on the widget. `paint` draws
/// the layout that was measured, so the painted lines always fit the size the
/// widget reported.
///
/// ```ignore
/// fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
///     self.paragraph = Paragraph::new(
///         ctx,
///         &self.text,
///         &self.style,
///         TextAlign::Start,
///         constraints.max.width,
///     );
///     constraints.clamp(self.paragraph.size())
/// }
///
/// fn paint(&self, ctx: &mut PaintCtx) {
///     self.paragraph.paint(ctx, ctx.bounds(), VerticalAlign::Top);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Paragraph {
    text: String,
    style: TextStyle,
    align: TextAlign,
    layout: Option<TextLayout>,
    /// The width lines were aligned in.
    box_width: f32,
}

impl Paragraph {
    /// Lay out `text` wrapped to `max_width`, or unwrapped when it is
    /// infinite, with each line aligned by `align`.
    pub fn new(
        shaper: &(impl TextShaper + ?Sized),
        text: impl Into<String>,
        style: &TextStyle,
        align: TextAlign,
        max_width: f32,
    ) -> Self {
        let text = text.into();
        let width = if max_width.is_finite() {
            max_width.max(1.0)
        } else {
            f32::INFINITY
        };
        let request = |paragraph_align| {
            // Shape in white and paint in the style's color, so the layout is
            // shared across colors.
            let mut layout_style = style.clone();
            layout_style.color = Color::WHITE;
            let mut document = TextDocument::from_plain_text(text.clone(), layout_style);
            for paragraph in &mut document.paragraphs {
                paragraph.style.align = paragraph_align;
                paragraph.style.wrap = TextWrap::Word;
            }
            TextLayoutRequest::new(document).with_box_size(Size::new(width, f32::INFINITY))
        };
        // Lines laid out from the left edge are placed by their ink, which
        // keeps a single line optically aligned. Several lines aligned
        // otherwise are aligned by the layout itself.
        let mut layout = shaper.shape_document(request(TextAlign::Left)).ok();
        if !matches!(align, TextAlign::Left | TextAlign::Start)
            && layout
                .as_ref()
                .is_some_and(|layout| layout.lines().len() > 1)
        {
            layout = shaper.shape_document(request(align)).ok().or(layout);
        }
        let box_width = layout
            .as_ref()
            .map_or(0.0, |layout| layout.box_size().width);
        Self {
            text,
            style: style.clone(),
            align,
            layout,
            box_width,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The size the text takes: its widest line by the height of its lines.
    pub fn size(&self) -> Size {
        self.layout.as_ref().map_or_else(
            || Size::new(0.0, self.style.line_height),
            |layout| {
                let measurement = layout.measurement();
                Size::new(measurement.width, measurement.height)
            },
        )
    }

    pub fn line_count(&self) -> usize {
        self.layout
            .as_ref()
            .map_or(0, |layout| layout.lines().len())
    }

    /// Paint into `rect`, placed vertically by `vertical` and horizontally by
    /// the alignment the paragraph was laid out with.
    pub fn paint(&self, ctx: &mut PaintCtx, rect: Rect, vertical: VerticalAlign) {
        self.paint_with_color(ctx, rect, vertical, self.style.color);
    }

    /// Paint like [`paint`](Self::paint), in `color` instead of the style's,
    /// for a color that can change without laying the text out again.
    pub fn paint_with_color(
        &self,
        ctx: &mut PaintCtx,
        rect: Rect,
        vertical: VerticalAlign,
        color: Color,
    ) {
        let Some(layout) = &self.layout else {
            if !self.text.is_empty() {
                let style = TextStyle {
                    color,
                    ..self.style.clone()
                };
                ctx.draw_text(rect, self.text.clone(), style);
            }
            return;
        };
        let origin = paragraph_origin(
            ctx,
            rect,
            layout,
            alignment_fraction(self.align),
            self.box_width,
            vertical,
        );
        ctx.draw_text_layout_with_color(origin, layout, color);
    }
}

/// Where to draw `layout` so it sits in `rect`. Lines laid out from the left
/// edge are placed by their ink, a single line centers on its capitals, and a
/// block of lines is placed by its line boxes. A block taller than `rect`
/// starts at its top, and in a rect with room for one line its first line is
/// placed like a single line.
pub(crate) fn paragraph_origin(
    ctx: &PaintCtx,
    rect: Rect,
    layout: &TextLayout,
    horizontal: f32,
    box_width: f32,
    vertical: VerticalAlign,
) -> Point {
    let measurement = layout.measurement();
    let lines = layout.lines();
    let left_aligned = lines.len() <= 1
        || layout
            .paragraphs()
            .iter()
            .all(|paragraph| matches!(paragraph.style.align, TextAlign::Left | TextAlign::Start));
    let x = if left_aligned {
        horizontal_placement(
            rect,
            measurement,
            horizontal,
            HorizontalTextAlignmentMode::Optical,
        )
        .origin_x
    } else {
        rect.x() + (rect.width() - box_width) * horizontal
    };
    // A rect with room for one line shows the first line, placed as if it
    // were alone, so a one-line slot places text the same whether it wraps
    // or not.
    let (height, lone_line) = match lines {
        [line] => (measurement.height, Some(line)),
        [first, second, ..] if rect.height() < second.rect.max_y() => {
            (first.rect.max_y(), Some(first))
        }
        _ => (measurement.height, None),
    };
    let room = (rect.height() - height).max(0.0);
    let y = match vertical {
        VerticalAlign::Top => rect.y(),
        VerticalAlign::Bottom => rect.y() + room,
        VerticalAlign::Center => match lone_line {
            Some(line) => {
                rect.y() + rect.height() * 0.5 - line.baseline - visual_center(ctx, measurement)
            }
            None => rect.y() + room * 0.5,
        },
    };
    Point::new(x, y)
}

/// Paint `text` in `rect`, wrapped to its width and placed by `placement`.
///
/// Pass a [`TextAlign`] for text centered vertically, or a
/// [`TextPlacement`] to place it at the top or bottom. To measure text
/// before painting it, lay it out as a [`Paragraph`] instead.
pub fn paint_text(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    placement: impl Into<TextPlacement>,
) {
    let placement = placement.into();
    let paragraph = Paragraph::new(ctx, text, style, placement.horizontal, rect.width());
    paragraph.paint(ctx, rect, placement.vertical);
}

/// Paint `text` on one line in `rect`, never wrapping: centered vertically on
/// its capital letters and placed horizontally by `align`.
pub fn paint_text_line(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    align: TextAlign,
) {
    let horizontal = alignment_fraction(align);
    let horizontal_mode = HorizontalTextAlignmentMode::Optical;
    if let Some(aligned) = aligned_text_layout_for_text_with_mode_and_wrap(
        ctx,
        rect,
        text,
        style,
        style.line_height,
        horizontal,
        horizontal_mode,
        TextWrap::NoWrap,
    ) {
        ctx.draw_text_layout_with_color(aligned.origin, &aligned.layout, aligned.color);
        return;
    }

    let fallback_rect = aligned_text_rect_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        style.line_height,
        horizontal,
        horizontal_mode,
    );
    ctx.draw_text(fallback_rect, text.to_string(), style.clone());
}

pub(crate) fn paint_aligned_text_contained(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    line_height: f32,
    horizontal_alignment: f32,
) {
    let horizontal_mode = HorizontalTextAlignmentMode::Optical;
    if let Some(aligned) = aligned_text_layout_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    ) {
        let painted = painted_layout_rect(aligned.origin, &aligned.layout);
        let offset = containment_offset(painted, rect);
        ctx.draw_text_layout_with_color(aligned.origin + offset, &aligned.layout, aligned.color);
        return;
    }

    let fallback_rect = aligned_text_rect_for_text_with_mode(
        ctx,
        rect,
        text,
        style,
        line_height,
        horizontal_alignment,
        horizontal_mode,
    );
    ctx.draw_text(
        fallback_rect.translate(containment_offset(fallback_rect, rect)),
        text.to_string(),
        style.clone(),
    );
}

fn painted_layout_rect(origin: Point, layout: &TextLayout) -> Rect {
    let measurement = layout.measurement();
    let bounds = measurement.bounds;
    let width = if bounds.width().is_finite() && bounds.width() > 0.0 {
        bounds.width()
    } else {
        measurement.width
    };
    Rect::new(
        origin.x + bounds.x(),
        origin.y + ((layout.box_size().height - measurement.height).max(0.0) * 0.5),
        width,
        layout.style().line_height.max(measurement.height),
    )
}

fn containment_offset(inner: Rect, outer: Rect) -> sui_core::Vector {
    fn axis_offset(inner_min: f32, inner_max: f32, outer_min: f32, outer_max: f32) -> f32 {
        let inner_size = inner_max - inner_min;
        let outer_size = outer_max - outer_min;
        if inner_size > outer_size || inner_min < outer_min {
            outer_min - inner_min
        } else if inner_max > outer_max {
            outer_max - inner_max
        } else {
            0.0
        }
    }

    sui_core::Vector::new(
        axis_offset(inner.x(), inner.max_x(), outer.x(), outer.max_x()),
        axis_offset(inner.y(), inner.max_y(), outer.y(), outer.max_y()),
    )
}

/// Paint one unwrapped line using SUI's shared optical baseline alignment path.
/// Greedy word-wrap `text` to `max_width`, using `measure` for run widths.
///
/// Explicit newlines are preserved. Words longer than `max_width` are kept on
/// their own line rather than split, which matches common UI metadata wrapping.
pub fn wrap_text_lines(
    text: &str,
    max_width: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let space_width = measure(" ");

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0.0_f32;
        for word in paragraph.split(' ') {
            let word_width = measure(word);
            if current.is_empty() {
                current.push_str(word);
                current_width = word_width;
            } else if current_width + space_width + word_width <= max_width {
                current.push(' ');
                current.push_str(word);
                current_width += space_width + word_width;
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
                current_width = word_width;
            }
        }
        lines.push(current);
    }

    lines
}

fn paragraph_alignment(
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> TextAlign {
    if horizontal_mode == HorizontalTextAlignmentMode::Optical {
        return TextAlign::Left;
    }

    let horizontal_alignment = horizontal_alignment.clamp(0.0, 1.0);
    if horizontal_alignment <= 0.0 {
        TextAlign::Left
    } else if horizontal_alignment >= 1.0 {
        TextAlign::Right
    } else {
        TextAlign::Center
    }
}

pub(crate) fn aligned_text_rect_for_layout(
    ctx: &PaintCtx,
    rect: Rect,
    layout: &TextLayout,
    line_height: f32,
    horizontal_alignment: f32,
) -> Rect {
    aligned_text_rect_for_layout_with_mode(
        ctx,
        rect,
        layout,
        line_height,
        horizontal_alignment,
        HorizontalTextAlignmentMode::Advance,
    )
}

pub(crate) fn aligned_text_rect_for_layout_with_mode(
    ctx: &PaintCtx,
    rect: Rect,
    layout: &TextLayout,
    line_height: f32,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> Rect {
    let measurement = layout.measurement();
    let placement = horizontal_placement(rect, measurement, horizontal_alignment, horizontal_mode);
    let height = line_height.max(measurement.height).min(rect.height());
    let y = layout
        .lines()
        .first()
        .map(|line| {
            rect.y() + (rect.height() * 0.5) - line.baseline - visual_center(ctx, measurement)
        })
        .unwrap_or_else(|| vertically_centered_text_rect_y(ctx, rect, measurement, height));

    Rect::new(placement.rect_x, y, placement.rect_width, height)
}

fn horizontal_placement(
    rect: Rect,
    measurement: TextMeasurement,
    horizontal_alignment: f32,
    horizontal_mode: HorizontalTextAlignmentMode,
) -> HorizontalPlacement {
    let horizontal_alignment = horizontal_alignment.clamp(0.0, 1.0);
    let (width, origin_shift) = match horizontal_mode {
        HorizontalTextAlignmentMode::Advance => (measurement.width, 0.0),
        HorizontalTextAlignmentMode::Optical => {
            let bounds = measurement.bounds;
            if bounds.width().is_finite() && bounds.width() > 0.0 {
                (bounds.width(), -bounds.x())
            } else {
                (measurement.width, 0.0)
            }
        }
    };
    let rect_width = width.min(rect.width()).max(0.0);
    let rect_x = rect.x() + ((rect.width() - rect_width).max(0.0) * horizontal_alignment);

    HorizontalPlacement {
        rect_x,
        rect_width,
        origin_x: rect_x + origin_shift,
    }
}

pub(crate) fn vertically_centered_text_rect_y(
    ctx: &PaintCtx,
    rect: Rect,
    measurement: TextMeasurement,
    height: f32,
) -> f32 {
    let visual_center = visual_center(ctx, measurement);
    let baseline = rect.y() + (rect.height() * 0.5) - visual_center;
    let leading_above = ((height - (measurement.ascent + measurement.descent)).max(0.0)) * 0.5;

    baseline - measurement.ascent - leading_above
}

fn visual_center(ctx: &PaintCtx, measurement: TextMeasurement) -> f32 {
    let optical_centering = window_render_options(ctx.window_id())
        .map(|options| options.optical_vertical_text_alignment_enabled)
        .unwrap_or(true);
    let top = if optical_centering {
        -measurement.cap_height.unwrap_or(measurement.ascent)
    } else {
        -measurement.ascent
    };
    let bottom = if optical_centering {
        measurement.descent * 0.5
    } else {
        measurement.descent
    };
    (top + bottom) * 0.5
}

#[cfg(test)]
mod tests {
    use super::wrap_text_lines;
    use std::{cell::RefCell, rc::Rc};

    use sui_core::{Color, Point, Rect, Size};
    use sui_layout::Constraints;
    use sui_runtime::{Application, MeasureCtx, PaintCtx, Widget, WindowBuilder};
    use sui_scene::SceneCommand;
    use sui_text::{TextAlign, TextStyle};

    use super::{
        HorizontalTextAlignmentMode, Paragraph, TextPlacement, VerticalAlign,
        aligned_text_layout_for_text_with_mode, paint_text,
    };

    type Paint = Box<dyn Fn(&mut PaintCtx)>;
    type Measure = Box<dyn Fn(&mut MeasureCtx)>;

    /// A widget that runs `measure` and `paint` callbacks in a 400 × 600 window.
    struct Probe {
        measure: Option<Measure>,
        paint: Paint,
    }

    impl Widget for Probe {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            if let Some(measure) = &self.measure {
                measure(ctx);
            }
            constraints.clamp(Size::new(400.0, 600.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            (self.paint)(ctx);
        }
    }

    /// Where each drawn text ended up: its text, its first baseline, the x of
    /// its layout origin, and its size.
    #[derive(Debug, Clone, PartialEq)]
    struct Drawn {
        text: String,
        origin: Point,
        first_baseline: f32,
        size: Size,
        line_starts: Vec<f32>,
    }

    fn render(probe: Probe) -> Vec<Drawn> {
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Text").root(probe))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        let output = runtime.render(window_id).unwrap();
        let registry = output.frame.text_layout_registry.as_ref();
        let mut drawn = Vec::new();
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command
                && let Some(layout) = text.resolve(registry)
            {
                drawn.push(Drawn {
                    text: layout.text().to_string(),
                    origin: text.origin,
                    first_baseline: text.origin.y + layout.lines()[0].baseline,
                    size: Size::new(layout.measurement().width, layout.measurement().height),
                    line_starts: layout.lines().iter().map(|line| line.rect.x()).collect(),
                });
            }
        });
        drawn
    }

    fn style() -> TextStyle {
        TextStyle::new(Color::BLACK)
    }

    const PARAGRAPH: &str = "several words that wrap onto more than one line of text here";

    #[test]
    fn a_single_line_is_placed_as_before() {
        // The previous single-line path, kept for text fields and buttons,
        // against the new one: same baseline, same optical horizontal place.
        for (align, fraction) in [
            (TextAlign::Start, 0.0),
            (TextAlign::Center, 0.5),
            (TextAlign::End, 1.0),
        ] {
            let drawn = render(Probe {
                measure: None,
                paint: Box::new(move |ctx| {
                    let rect = Rect::new(20.0, 30.0, 300.0, 44.0);
                    paint_text(ctx, rect, "Label", &style(), align);
                    let before = aligned_text_layout_for_text_with_mode(
                        ctx,
                        rect.translate(sui_core::Vector::new(0.0, 100.0)),
                        "Label",
                        &style(),
                        style().line_height,
                        fraction,
                        HorizontalTextAlignmentMode::Optical,
                    )
                    .unwrap();
                    ctx.draw_text_layout_with_color(before.origin, &before.layout, Color::BLACK);
                }),
            });
            let [now, before] = &drawn[..] else {
                panic!("two texts are drawn: {drawn:?}");
            };
            assert!(
                (now.origin.x - before.origin.x).abs() < 0.001,
                "{align:?}: {now:?} vs {before:?}"
            );
            assert!(
                (now.first_baseline + 100.0 - before.first_baseline).abs() < 0.001,
                "{align:?}: {now:?} vs {before:?}"
            );
        }
    }

    #[test]
    fn several_lines_are_placed_as_a_block() {
        let rect = Rect::new(10.0, 20.0, 160.0, 300.0);
        let placed = |vertical| {
            render(Probe {
                measure: None,
                paint: Box::new(move |ctx| {
                    paint_text(
                        ctx,
                        rect,
                        PARAGRAPH,
                        &style(),
                        TextPlacement::new(TextAlign::Start, vertical),
                    );
                }),
            })
            .remove(0)
        };

        let top = placed(VerticalAlign::Top);
        assert!(top.line_starts.len() > 2, "{top:?}");
        let height = top.size.height;
        assert!((top.origin.y - rect.y()).abs() < 0.001, "{top:?}");
        let center = placed(VerticalAlign::Center);
        assert!(
            (center.origin.y - (rect.y() + (rect.height() - height) * 0.5)).abs() < 0.001,
            "{center:?}"
        );
        let bottom = placed(VerticalAlign::Bottom);
        assert!(
            (bottom.origin.y - (rect.max_y() - height)).abs() < 0.001,
            "{bottom:?}"
        );

        // A block taller than its rect starts at the top instead of rising
        // above it.
        let shorter = Rect::new(10.0, 20.0, 160.0, height * 0.9);
        let overflowing = render(Probe {
            measure: None,
            paint: Box::new(move |ctx| {
                paint_text(ctx, shorter, PARAGRAPH, &style(), TextAlign::Start);
            }),
        })
        .remove(0);
        assert!(
            (overflowing.origin.y - shorter.y()).abs() < 0.001,
            "{overflowing:?}"
        );

        // In a rect with room for one line, the first line sits where it
        // would on its own.
        let one_line = Rect::new(10.0, 20.0, 160.0, 12.0);
        let drawn = render(Probe {
            measure: None,
            paint: Box::new(move |ctx| {
                paint_text(ctx, one_line, PARAGRAPH, &style(), TextAlign::Start);
                paint_text(ctx, one_line, "several", &style(), TextAlign::Start);
            }),
        });
        let [wrapped, alone] = &drawn[..] else {
            panic!("two texts are drawn: {drawn:?}");
        };
        assert!(wrapped.line_starts.len() > 1, "{wrapped:?}");
        assert!(
            (wrapped.first_baseline - alone.first_baseline).abs() < 0.001,
            "{wrapped:?} vs {alone:?}"
        );
    }

    #[test]
    fn centered_lines_are_centered_one_by_one() {
        let drawn = render(Probe {
            measure: None,
            paint: Box::new(|ctx| {
                paint_text(
                    ctx,
                    Rect::new(0.0, 0.0, 160.0, 300.0),
                    PARAGRAPH,
                    &style(),
                    TextAlign::Center,
                );
            }),
        })
        .remove(0);
        let first = drawn.line_starts[0];
        assert!(
            drawn
                .line_starts
                .iter()
                .any(|start| (start - first).abs() > 1.0),
            "lines of different widths start at different places: {drawn:?}"
        );
    }

    #[test]
    fn a_paragraph_paints_the_layout_it_measured() {
        let measured = Rc::new(RefCell::new(Paragraph::default()));
        let laid_out = Rc::clone(&measured);
        let painted = Rc::clone(&measured);
        let drawn = render(Probe {
            measure: Some(Box::new(move |ctx| {
                *laid_out.borrow_mut() =
                    Paragraph::new(ctx, PARAGRAPH, &style(), TextAlign::Start, 160.0);
            })),
            paint: Box::new(move |ctx| {
                let paragraph = painted.borrow();
                let rect = Rect::from_origin_size(Point::new(5.0, 7.0), paragraph.size());
                paragraph.paint(ctx, rect, VerticalAlign::Top);
            }),
        })
        .remove(0);

        let paragraph = measured.borrow();
        assert!(paragraph.line_count() > 2);
        assert_eq!(drawn.text, PARAGRAPH);
        assert_eq!(drawn.size, paragraph.size());
        assert!((drawn.origin.y - 7.0).abs() < 0.001, "{drawn:?}");
    }

    #[test]
    fn wrap_text_lines_preserves_newlines_and_keeps_long_words() {
        let lines = wrap_text_lines("alpha beta\nsuperlongword gamma", 10.0, |text| {
            text.chars().count() as f32
        });

        assert_eq!(
            lines,
            vec![
                "alpha beta".to_string(),
                "superlongword".to_string(),
                "gamma".to_string(),
            ]
        );
    }
}
