//! Painted probes. Each draws known colors so the output's handling of
//! them can be judged by eye and in a capture.

use sui::prelude::*;
use sui::{
    Brush, ColorSpace, DisplayColorPrimaries, GradientStop, Rect, SemanticsNode, SemanticsRole,
    SemanticsValue, TextStyle, WidgetId, fit_to_sdr, paint_text_line,
};

use super::live::{FollowedDiagnostics, HighlightFit, OutputSummary};
use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};

const LABEL_GAP: f32 = 6.0;
const TILE_RADIUS: f32 = 6.0;

fn label_style(theme_reader: &DevThemeReader) -> TextStyle {
    let theme = theme_reader();
    demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text_muted)
}

/// Opaque, so it still shows around tiles brighter than SDR white: a
/// translucent stroke over 4× white stays above white.
fn outline_color(theme_reader: &DevThemeReader) -> Color {
    theme_reader().palette.text_muted
}

fn available_width(constraints: Constraints, preferred: f32) -> f32 {
    if constraints.max.width.is_finite() {
        constraints.max.width
    } else {
        preferred
    }
}

/// An id for a part of a painted probe, for its semantics node.
fn part_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 19_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(521)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

fn push_container(ctx: &mut SemanticsCtx, name: &str, description: Option<String>) {
    let mut node = SemanticsNode::new(
        ctx.widget_id(),
        SemanticsRole::GenericContainer,
        ctx.bounds(),
    );
    node.name = Some(name.to_string());
    node.description = description;
    ctx.push(node);
}

fn push_swatch(ctx: &mut SemanticsCtx, index: usize, rect: Rect, name: &str, color: Color) {
    let linear = color.to_linear_srgb();
    let mut node = SemanticsNode::new(
        part_id(ctx.widget_id(), index),
        SemanticsRole::ColorSwatch,
        rect,
    );
    node.parent = Some(ctx.widget_id());
    node.name = Some(name.to_string());
    node.value = Some(SemanticsValue::Text(format!(
        "linear sRGB {:.3}, {:.3}, {:.3}",
        linear.red, linear.green, linear.blue
    )));
    ctx.push(node);
}

fn linear(red: f32, green: f32, blue: f32) -> Color {
    Color::linear_rgba(red, green, blue, 1.0)
}

fn scaled(color: [f32; 3], scale: f32) -> [f32; 3] {
    color.map(|channel| channel * scale)
}

/// `color` as `summary`'s output fits it, computed on the CPU. The result
/// is within SDR range, so the GPU passes it through unchanged; painting it
/// beside the unfitted color shows whether the GPU fit agrees.
pub(crate) fn fitted_on_cpu(color: [f32; 3], summary: Option<OutputSummary>) -> Color {
    let Some(summary) = summary else {
        return linear(color[0], color[1], color[2]);
    };
    let Some(mode) = summary.fit.tone_mapping() else {
        return linear(color[0], color[1], color[2]);
    };
    // Outputs fit after converting to their primaries.
    let space = match summary.primaries {
        DisplayColorPrimaries::Srgb => ColorSpace::LinearSrgb,
        DisplayColorPrimaries::DisplayP3 => ColorSpace::LinearDisplayP3,
    };
    let in_output = linear(color[0], color[1], color[2]).to_space(space);
    let [red, green, blue] = fit_to_sdr(
        [
            in_output.red.max(0.0),
            in_output.green.max(0.0),
            in_output.blue.max(0.0),
        ],
        mode,
    );
    Color::new(space, red, green, blue, 1.0)
}

/// A row of tiles, each a named color with a label under it.
pub(crate) struct SwatchStrip {
    name: &'static str,
    theme_reader: DevThemeReader,
    swatches: Vec<(String, String, Color)>,
    tile: Size,
    gap: f32,
}

impl SwatchStrip {
    pub(crate) fn new(name: &'static str, theme_reader: &DevThemeReader) -> Self {
        Self {
            name,
            theme_reader: std::rc::Rc::clone(theme_reader),
            swatches: Vec::new(),
            tile: Size::new(88.0, 56.0),
            gap: 10.0,
        }
    }

    /// Add a tile: `name` for accessibility, `label` under the tile.
    pub(crate) fn swatch(
        mut self,
        name: impl Into<String>,
        label: impl Into<String>,
        color: Color,
    ) -> Self {
        self.swatches.push((name.into(), label.into(), color));
        self
    }

    fn tile_rect(&self, bounds: Rect, index: usize) -> Rect {
        Rect::new(
            bounds.x() + index as f32 * (self.tile.width + self.gap),
            bounds.y(),
            self.tile.width,
            self.tile.height,
        )
    }
}

impl Widget for SwatchStrip {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let count = self.swatches.len() as f32;
        let width = count * self.tile.width + (count - 1.0).max(0.0) * self.gap;
        let height = self.tile.height + LABEL_GAP + label_style(&self.theme_reader).line_height;
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let style = label_style(&self.theme_reader);
        let outline = outline_color(&self.theme_reader);
        for (index, (_, label, color)) in self.swatches.iter().enumerate() {
            let tile = self.tile_rect(bounds, index);
            ctx.fill_rrect(tile, [TILE_RADIUS; 4], *color);
            // Wide enough to cover the antialiased fringe, which glows past
            // SDR white around very bright tiles.
            ctx.stroke(
                {
                    let mut path = PathBuilder::new();
                    path.push_rounded_rect(tile, TILE_RADIUS);
                    path.build()
                },
                outline,
                StrokeStyle::new(2.0),
            );
            paint_text_line(
                ctx,
                Rect::new(
                    tile.x() - self.gap * 0.5,
                    tile.max_y() + LABEL_GAP,
                    tile.width() + self.gap,
                    style.line_height,
                ),
                label,
                &style,
                TextAlign::Center,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(ctx, self.name, None);
        let bounds = ctx.bounds();
        for (index, (name, _, color)) in self.swatches.iter().enumerate() {
            let tile = self.tile_rect(bounds, index);
            push_swatch(ctx, index, tile, name, *color);
        }
    }
}

pub(crate) const HEADROOM_RAMP_NAME: &str = "Headroom ramp";

/// Log2 range of the headroom ramp: a quarter of SDR white to 16 times it.
const RAMP_OCTAVES: std::ops::RangeInclusive<i32> = -2..=4;

/// White from a quarter of SDR white to 16 times it, on a log scale, with
/// SDR white and the display's reported peak marked.
pub(crate) struct HeadroomRamp {
    theme_reader: DevThemeReader,
    diagnostics: FollowedDiagnostics,
}

impl HeadroomRamp {
    const BAR_HEIGHT: f32 = 48.0;
    const MARKER_HEIGHT: f32 = 10.0;

    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            diagnostics: FollowedDiagnostics::new(),
        }
    }

    fn octave_span() -> (f32, f32) {
        (*RAMP_OCTAVES.start() as f32, *RAMP_OCTAVES.end() as f32)
    }

    /// Where `multiple` of SDR white falls along `bar`.
    fn x_for(bar: Rect, multiple: f32) -> f32 {
        let (low, high) = Self::octave_span();
        let t = ((multiple.max(f32::MIN_POSITIVE).log2() - low) / (high - low)).clamp(0.0, 1.0);
        bar.x() + t * bar.width()
    }
}

/// A slot `width` wide for a label under `x` on `bar`: centered on `x`, or
/// against the end of the bar it would run past.
fn label_under(bar: Rect, x: f32, width: f32) -> (f32, f32, TextAlign) {
    let centered = x - width * 0.5;
    if centered < bar.x() {
        (bar.x(), width, TextAlign::Start)
    } else if centered + width > bar.max_x() {
        (bar.max_x() - width, width, TextAlign::End)
    } else {
        (centered, width, TextAlign::Center)
    }
}

fn multiple_label(multiple: f32) -> String {
    match multiple {
        value if (value - 0.25).abs() < f32::EPSILON => "¼×".to_string(),
        value if (value - 0.5).abs() < f32::EPSILON => "½×".to_string(),
        value => format!("{value}×"),
    }
}

impl Widget for HeadroomRamp {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        let style = label_style(&self.theme_reader);
        let height = Self::BAR_HEIGHT + Self::MARKER_HEIGHT + LABEL_GAP + style.line_height * 2.0;
        constraints.clamp(Size::new(available_width(constraints, 720.0), height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let bar = Rect::new(bounds.x(), bounds.y(), bounds.width(), Self::BAR_HEIGHT);
        let octaves = RAMP_OCTAVES.clone().collect::<Vec<_>>();
        let last = (octaves.len() - 1) as f32;
        let stops = octaves
            .iter()
            .enumerate()
            .map(|(index, octave)| {
                let value = 2.0_f32.powi(*octave);
                GradientStop {
                    offset: index as f32 / last,
                    color: linear(value, value, value),
                }
            })
            .collect();
        ctx.fill_rect(
            bar,
            Brush::LinearGradient {
                start: Point::new(bar.x(), bar.y()),
                end: Point::new(bar.max_x(), bar.y()),
                stops,
            },
        );
        let outline = outline_color(&self.theme_reader);
        ctx.stroke_rect(bar, outline, StrokeStyle::new(1.0));

        let style = label_style(&self.theme_reader);
        let text = (self.theme_reader)().palette.text;
        let tick_top = bar.max_y();
        for octave in RAMP_OCTAVES {
            let multiple = 2.0_f32.powi(octave);
            let x = Self::x_for(bar, multiple);
            let strong = octave == 0;
            ctx.fill_rect(
                Rect::new(
                    x - 0.5,
                    tick_top,
                    1.0,
                    Self::MARKER_HEIGHT * if strong { 1.0 } else { 0.6 },
                ),
                if strong { text } else { outline },
            );
            let label = if strong {
                "1× SDR white".to_string()
            } else {
                multiple_label(multiple)
            };
            let (left, width, align) = label_under(bar, x, 96.0);
            paint_text_line(
                ctx,
                Rect::new(
                    left,
                    tick_top + Self::MARKER_HEIGHT + LABEL_GAP * 0.5,
                    width,
                    style.line_height,
                ),
                &label,
                &style,
                align,
            );
        }

        let summary = self.diagnostics.get().map(OutputSummary::of);
        if let Some(summary) = summary
            && summary.fit == HighlightFit::Extended
            && let Some(headroom) = summary.headroom
        {
            let x = Self::x_for(bar, headroom);
            let accent = (self.theme_reader)().palette.accent;
            ctx.fill_rect(
                Rect::new(x - 1.0, bar.y() - 2.0, 2.0, bar.height() + 4.0),
                accent,
            );
            let (left, width, align) = label_under(bar, x, 200.0);
            paint_text_line(
                ctx,
                Rect::new(
                    left,
                    tick_top + Self::MARKER_HEIGHT + LABEL_GAP * 0.5 + style.line_height,
                    width,
                    style.line_height,
                ),
                &format!("display peak ≈ {headroom:.1}×"),
                &style,
                align,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(
            ctx,
            HEADROOM_RAMP_NAME,
            Some("White from a quarter of SDR white to 16 times it, on a log scale.".to_string()),
        );
    }
}

pub(crate) const HIGHLIGHT_CURVE_NAME: &str = "Highlight fit curve";

/// The orange whose brightness the curve plot sweeps.
const CURVE_BASE: [f32; 3] = [1.0, 0.45, 0.1];
/// The sweep's log2 range, 1/8 to 16 times the base.
const CURVE_OCTAVES: (f32, f32) = (-3.0, 4.0);

/// Each output channel of an orange as it brightens, for the output's fit,
/// with the per-channel clamp for comparison, and strips of the colors sent
/// and the colors expected.
pub(crate) struct HighlightCurvePlot {
    theme_reader: DevThemeReader,
    diagnostics: FollowedDiagnostics,
}

impl HighlightCurvePlot {
    const PLOT_HEIGHT: f32 = 200.0;
    const STRIP_HEIGHT: f32 = 22.0;
    const AXIS_GUTTER: f32 = 40.0;

    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            diagnostics: FollowedDiagnostics::new(),
        }
    }

    fn multiple_at(t: f32) -> f32 {
        let (low, high) = CURVE_OCTAVES;
        2.0_f32.powf(low + t * (high - low))
    }

    /// The output channels for input `color`, relative to SDR white.
    fn output(color: [f32; 3], summary: Option<OutputSummary>) -> [f32; 3] {
        match summary.and_then(|summary| summary.fit.tone_mapping()) {
            Some(mode) => fit_to_sdr(color, mode),
            None => {
                let peak = summary
                    .and_then(|summary| summary.headroom)
                    .unwrap_or(f32::INFINITY);
                color.map(|channel| channel.min(peak))
            }
        }
    }

    fn y_range(summary: Option<OutputSummary>) -> f32 {
        match summary {
            Some(summary) if summary.fit == HighlightFit::Extended => {
                summary.headroom.unwrap_or(4.0).clamp(2.0, 16.0) * 1.05
            }
            _ => 1.05,
        }
    }
}

impl Widget for HighlightCurvePlot {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        let style = label_style(&self.theme_reader);
        let width = available_width(constraints, 640.0).min(720.0);
        let height = Self::PLOT_HEIGHT
            + LABEL_GAP
            + style.line_height
            + (Self::STRIP_HEIGHT + LABEL_GAP + style.line_height) * 2.0;
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let style = label_style(&self.theme_reader);
        let summary = self.diagnostics.get().map(OutputSummary::of);
        let bounds = ctx.bounds();
        let plot = Rect::new(
            bounds.x() + Self::AXIS_GUTTER,
            bounds.y(),
            (bounds.width() - Self::AXIS_GUTTER).max(1.0),
            Self::PLOT_HEIGHT,
        );
        let y_range = Self::y_range(summary);
        let to_point = |t: f32, value: f32| {
            Point::new(
                plot.x() + t * plot.width(),
                plot.max_y() - (value / y_range).clamp(0.0, 1.0) * plot.height(),
            )
        };

        ctx.fill_rect(plot, theme.palette.surface_raised);
        ctx.stroke_rect(plot, theme.palette.border, StrokeStyle::new(1.0));
        // SDR white on both axes.
        let (low, high) = CURVE_OCTAVES;
        let white_t = -low / (high - low);
        let white_x = plot.x() + white_t * plot.width();
        ctx.fill_rect(
            Rect::new(white_x - 0.5, plot.y(), 1.0, plot.height()),
            theme.palette.border,
        );
        let white_y = to_point(0.0, 1.0).y;
        ctx.fill_rect(
            Rect::new(plot.x(), white_y - 0.5, plot.width(), 1.0),
            theme.palette.border,
        );
        paint_text_line(
            ctx,
            Rect::new(
                bounds.x(),
                white_y - style.line_height * 0.5,
                Self::AXIS_GUTTER - 6.0,
                style.line_height,
            ),
            "1×",
            &style,
            TextAlign::End,
        );
        if y_range > 1.5 {
            paint_text_line(
                ctx,
                Rect::new(
                    bounds.x(),
                    plot.y(),
                    Self::AXIS_GUTTER - 6.0,
                    style.line_height,
                ),
                &format!("{:.0}×", y_range / 1.05),
                &style,
                TextAlign::End,
            );
        }

        const SAMPLES: usize = 96;
        let channel_colors = [
            Color::rgba(0.93, 0.26, 0.21, 1.0),
            Color::rgba(0.20, 0.72, 0.33, 1.0),
            Color::rgba(0.26, 0.47, 0.96, 1.0),
        ];
        if summary.is_some_and(|summary| summary.fit != HighlightFit::Extended) {
            // What clamping each channel on its own would do: green keeps
            // rising after red stops, so the orange turns yellow.
            for (channel, color) in channel_colors.iter().enumerate() {
                let mut path = PathBuilder::new();
                for sample in 0..=SAMPLES {
                    let t = sample as f32 / SAMPLES as f32;
                    let value = (CURVE_BASE[channel] * Self::multiple_at(t)).min(1.0);
                    let point = to_point(t, value);
                    if sample == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                ctx.stroke(path.build(), color.with_alpha(0.45), StrokeStyle::new(1.0));
            }
        }
        for (channel, color) in channel_colors.iter().enumerate() {
            let mut path = PathBuilder::new();
            for sample in 0..=SAMPLES {
                let t = sample as f32 / SAMPLES as f32;
                let value =
                    Self::output(scaled(CURVE_BASE, Self::multiple_at(t)), summary)[channel];
                let point = to_point(t, value);
                if sample == 0 {
                    path.move_to(point);
                } else {
                    path.line_to(point);
                }
            }
            ctx.stroke(path.build(), *color, StrokeStyle::new(2.0));
        }
        paint_text_line(
            ctx,
            Rect::new(
                plot.x(),
                plot.max_y() + LABEL_GAP,
                plot.width(),
                style.line_height,
            ),
            "Input brightness, ⅛× to 16× (log scale). Thick: output channels. Faint: clamping each channel instead.",
            &style,
            TextAlign::Start,
        );

        // The same sweep as colors: sent unfitted, and fitted on the CPU.
        let mut y = plot.max_y() + LABEL_GAP + style.line_height + LABEL_GAP;
        for (title, fitted) in [
            ("Sent as is; the output fits it", false),
            ("Fitted on the CPU; should match the strip above", true),
        ] {
            let strip = Rect::new(plot.x(), y, plot.width(), Self::STRIP_HEIGHT);
            const CELLS: usize = 28;
            let cell_width = strip.width() / CELLS as f32;
            for cell in 0..CELLS {
                let t = (cell as f32 + 0.5) / CELLS as f32;
                let color = scaled(CURVE_BASE, Self::multiple_at(t));
                let fill = if fitted {
                    fitted_on_cpu(color, summary)
                } else {
                    linear(color[0], color[1], color[2])
                };
                // Each cell runs to the strip's end and the next covers the
                // rest, so no background shows between them.
                let x = strip.x() + cell as f32 * cell_width;
                ctx.fill_rect(
                    Rect::new(x, strip.y(), strip.max_x() - x, strip.height()),
                    fill,
                );
            }
            paint_text_line(
                ctx,
                Rect::new(
                    strip.x(),
                    strip.max_y() + 2.0,
                    strip.width(),
                    style.line_height,
                ),
                title,
                &style,
                TextAlign::Start,
            );
            y = strip.max_y() + 2.0 + style.line_height + LABEL_GAP;
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(
            ctx,
            HIGHLIGHT_CURVE_NAME,
            Some(
                "Output channels of an orange brightened from one eighth to 16 times SDR white."
                    .to_string(),
            ),
        );
    }
}

pub(crate) const HUE_GRID_NAME: &str = "Highlight hue grid";

const HUE_GRID_HUES: [(&str, [f32; 3]); 7] = [
    ("Red", [1.0, 0.05, 0.05]),
    ("Orange", [1.0, 0.45, 0.08]),
    ("Yellow", [1.0, 0.9, 0.1]),
    ("Green", [0.1, 1.0, 0.15]),
    ("Cyan", [0.08, 0.8, 1.0]),
    ("Blue", [0.1, 0.2, 1.0]),
    ("Magenta", [0.9, 0.1, 1.0]),
];
const HUE_GRID_MULTIPLES: [f32; 6] = [0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// Saturated colors at rising brightness. The top half of each cell is sent
/// as is; the bottom half is fitted on the CPU the way the output fits.
pub(crate) struct HueFitGrid {
    theme_reader: DevThemeReader,
    diagnostics: FollowedDiagnostics,
}

impl HueFitGrid {
    const ROW_LABEL_WIDTH: f32 = 72.0;
    const CELL: Size = Size::new(72.0, 40.0);
    const GAP: f32 = 4.0;

    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            diagnostics: FollowedDiagnostics::new(),
        }
    }
}

impl Widget for HueFitGrid {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        let style = label_style(&self.theme_reader);
        let columns = HUE_GRID_MULTIPLES.len() as f32;
        let rows = HUE_GRID_HUES.len() as f32;
        constraints.clamp(Size::new(
            Self::ROW_LABEL_WIDTH + columns * (Self::CELL.width + Self::GAP),
            style.line_height + LABEL_GAP + rows * (Self::CELL.height + Self::GAP),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let style = label_style(&self.theme_reader);
        let summary = self.diagnostics.get().map(OutputSummary::of);
        let top = bounds.y() + style.line_height + LABEL_GAP;
        for (column, multiple) in HUE_GRID_MULTIPLES.iter().enumerate() {
            let x =
                bounds.x() + Self::ROW_LABEL_WIDTH + column as f32 * (Self::CELL.width + Self::GAP);
            paint_text_line(
                ctx,
                Rect::new(x, bounds.y(), Self::CELL.width, style.line_height),
                &multiple_label(*multiple),
                &style,
                TextAlign::Center,
            );
        }
        for (row, (name, hue)) in HUE_GRID_HUES.iter().enumerate() {
            let y = top + row as f32 * (Self::CELL.height + Self::GAP);
            paint_text_line(
                ctx,
                Rect::new(
                    bounds.x(),
                    y,
                    Self::ROW_LABEL_WIDTH - 8.0,
                    Self::CELL.height,
                ),
                name,
                &style,
                TextAlign::Start,
            );
            for (column, multiple) in HUE_GRID_MULTIPLES.iter().enumerate() {
                let x = bounds.x()
                    + Self::ROW_LABEL_WIDTH
                    + column as f32 * (Self::CELL.width + Self::GAP);
                let color = scaled(*hue, *multiple);
                let half = Self::CELL.height * 0.5;
                // The bottom half covers the whole cell's lower part, so no
                // background shows between the halves.
                ctx.fill_rect(
                    Rect::new(x, y, Self::CELL.width, Self::CELL.height),
                    linear(color[0], color[1], color[2]),
                );
                ctx.fill_rect(
                    Rect::new(x, y + half, Self::CELL.width, half),
                    fitted_on_cpu(color, summary),
                );
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(
            ctx,
            HUE_GRID_NAME,
            Some("Seven hues from half to 16 times SDR white; each cell's top half is sent as is and its bottom half is fitted on the CPU.".to_string()),
        );
    }
}

pub(crate) const GAMUT_TILES_NAME: &str = "Wide-gamut split tiles";

/// Display P3 colors outside sRGB, as `(name, encoded Display P3 channels)`.
pub(crate) const GAMUT_PROBES: [(&str, [f32; 3]); 6] = [
    ("red", [1.0, 0.0, 0.0]),
    ("orange", [1.0, 0.5, 0.0]),
    ("yellow", [1.0, 1.0, 0.0]),
    ("green", [0.0, 1.0, 0.0]),
    ("cyan", [0.0, 1.0, 1.0]),
    ("magenta", [1.0, 0.0, 1.0]),
];

/// `color` as an sRGB output with `summary`'s highlight fit shows it: the
/// channels below zero (the part outside sRGB) clipped, and any overshoot
/// above SDR white fitted like a highlight.
pub(crate) fn clipped_to_srgb(color: Color, summary: Option<OutputSummary>) -> Color {
    let linear_srgb = color.to_linear_srgb();
    let inside = [
        linear_srgb.red.max(0.0),
        linear_srgb.green.max(0.0),
        linear_srgb.blue.max(0.0),
    ];
    let fit = summary.map_or(HighlightFit::Clip, |summary| summary.fit);
    let [red, green, blue] = match fit.tone_mapping() {
        Some(mode) => fit_to_sdr(inside, mode),
        None => inside,
    };
    linear(red, green, blue)
}

/// Tiles whose left half is a Display P3 color clipped to sRGB and whose
/// right half is the color itself. The seam shows only on wide-gamut output.
pub(crate) struct GamutSplitTiles {
    theme_reader: DevThemeReader,
    diagnostics: FollowedDiagnostics,
}

impl GamutSplitTiles {
    const TILE: Size = Size::new(104.0, 64.0);
    const GAP: f32 = 10.0;

    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            diagnostics: FollowedDiagnostics::new(),
        }
    }

    fn tile_rect(bounds: Rect, index: usize) -> Rect {
        Rect::new(
            bounds.x() + index as f32 * (Self::TILE.width + Self::GAP),
            bounds.y(),
            Self::TILE.width,
            Self::TILE.height,
        )
    }

    fn halves(tile: Rect) -> (Rect, Rect) {
        let half = tile.width() * 0.5;
        (
            Rect::new(tile.x(), tile.y(), half, tile.height()),
            Rect::new(tile.x() + half, tile.y(), half, tile.height()),
        )
    }
}

impl Widget for GamutSplitTiles {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        let count = GAMUT_PROBES.len() as f32;
        constraints.clamp(Size::new(
            count * Self::TILE.width + (count - 1.0) * Self::GAP,
            Self::TILE.height + LABEL_GAP + label_style(&self.theme_reader).line_height,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let style = label_style(&self.theme_reader);
        let summary = self.diagnostics.get().map(OutputSummary::of);
        for (index, (name, [red, green, blue])) in GAMUT_PROBES.iter().enumerate() {
            let tile = Self::tile_rect(bounds, index);
            let (_, right) = Self::halves(tile);
            let p3 = Color::display_p3(*red, *green, *blue, 1.0);
            // A seam between separately filled halves would show on any
            // output; the only seam should be the gamut difference.
            ctx.fill_rect(tile, clipped_to_srgb(p3, summary));
            ctx.fill_rect(right, p3);
            paint_text_line(
                ctx,
                Rect::new(
                    tile.x(),
                    tile.max_y() + LABEL_GAP,
                    tile.width(),
                    style.line_height,
                ),
                name,
                &style,
                TextAlign::Center,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(ctx, GAMUT_TILES_NAME, None);
        let bounds = ctx.bounds();
        let summary = self.diagnostics.get().map(OutputSummary::of);
        for (index, (name, [red, green, blue])) in GAMUT_PROBES.iter().enumerate() {
            let (left, right) = Self::halves(Self::tile_rect(bounds, index));
            let p3 = Color::display_p3(*red, *green, *blue, 1.0);
            let clipped = clipped_to_srgb(p3, summary);
            push_swatch(
                ctx,
                index * 2,
                left,
                &format!("sRGB clipped {name}"),
                clipped,
            );
            push_swatch(ctx, index * 2 + 1, right, &format!("Display P3 {name}"), p3);
        }
    }
}

pub(crate) const CHROMATICITY_NAME: &str = "Chromaticity diagram";

/// CIE 1931 2° spectral locus, 380 nm to 700 nm in 10 nm steps.
const SPECTRAL_LOCUS: [(f32, f32); 33] = [
    (0.1741, 0.0050),
    (0.1738, 0.0049),
    (0.1733, 0.0048),
    (0.1726, 0.0048),
    (0.1714, 0.0051),
    (0.1689, 0.0069),
    (0.1644, 0.0109),
    (0.1566, 0.0177),
    (0.1440, 0.0297),
    (0.1241, 0.0578),
    (0.0913, 0.1327),
    (0.0454, 0.2950),
    (0.0082, 0.5384),
    (0.0139, 0.7502),
    (0.0743, 0.8338),
    (0.1547, 0.8059),
    (0.2296, 0.7543),
    (0.3016, 0.6923),
    (0.3731, 0.6245),
    (0.4441, 0.5547),
    (0.5125, 0.4866),
    (0.5752, 0.4242),
    (0.6270, 0.3725),
    (0.6658, 0.3340),
    (0.6915, 0.3083),
    (0.7079, 0.2920),
    (0.7190, 0.2809),
    (0.7260, 0.2740),
    (0.7300, 0.2700),
    (0.7320, 0.2680),
    (0.7334, 0.2666),
    (0.7344, 0.2656),
    (0.7347, 0.2653),
];
const SRGB_PRIMARIES: [(f32, f32); 3] = [(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)];
const DISPLAY_P3_PRIMARIES: [(f32, f32); 3] = [(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)];
const D65_WHITE: (f32, f32) = (0.3127, 0.3290);

/// CIE xy chromaticity of a color.
pub(crate) fn chromaticity(color: Color) -> Option<(f32, f32)> {
    let c = color.to_linear_srgb();
    let x = 0.412_456_4 * c.red + 0.357_576_1 * c.green + 0.180_437_5 * c.blue;
    let y = 0.212_672_9 * c.red + 0.715_152_2 * c.green + 0.072_175 * c.blue;
    let z = 0.019_333_9 * c.red + 0.119_192 * c.green + 0.950_304_1 * c.blue;
    let sum = x + y + z;
    (sum > f32::EPSILON).then(|| (x / sum, y / sum))
}

/// The sRGB and Display P3 gamuts on the CIE 1931 diagram, the output's
/// gamut drawn strongest, with the wide-gamut probes plotted.
pub(crate) struct ChromaticityDiagram {
    theme_reader: DevThemeReader,
    diagnostics: FollowedDiagnostics,
}

impl ChromaticityDiagram {
    const SIZE: Size = Size::new(280.0, 300.0);
    const X_RANGE: f32 = 0.8;
    const Y_RANGE: f32 = 0.9;

    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            diagnostics: FollowedDiagnostics::new(),
        }
    }

    fn polygon(plot: Rect, points: &[(f32, f32)]) -> sui::Path {
        let to_point = |(x, y): (f32, f32)| {
            Point::new(
                plot.x() + x / Self::X_RANGE * plot.width(),
                plot.max_y() - y / Self::Y_RANGE * plot.height(),
            )
        };
        let mut path = PathBuilder::new();
        for (index, point) in points.iter().enumerate() {
            if index == 0 {
                path.move_to(to_point(*point));
            } else {
                path.line_to(to_point(*point));
            }
        }
        path.close();
        path.build()
    }
}

impl Widget for ChromaticityDiagram {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        constraints.clamp(Self::SIZE)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let style = label_style(&self.theme_reader);
        let bounds = ctx.bounds();
        let plot = Rect::new(
            bounds.x(),
            bounds.y(),
            bounds.width(),
            bounds.height() - style.line_height,
        );
        ctx.fill_rect(plot, theme.palette.surface_raised);
        ctx.stroke_rect(plot, theme.palette.border, StrokeStyle::new(1.0));
        ctx.stroke(
            Self::polygon(plot, &SPECTRAL_LOCUS),
            theme.palette.text_muted,
            StrokeStyle::new(1.5),
        );

        let wide = self
            .diagnostics
            .get()
            .map(OutputSummary::of)
            .is_some_and(OutputSummary::shows_wide_gamut);
        for (primaries, active) in [(SRGB_PRIMARIES, !wide), (DISPLAY_P3_PRIMARIES, wide)] {
            let (color, width) = if active {
                (theme.palette.accent, 2.5)
            } else {
                (theme.palette.text_muted.with_alpha(0.6), 1.0)
            };
            ctx.stroke(
                Self::polygon(plot, &primaries),
                color,
                StrokeStyle::new(width),
            );
        }

        let to_point = |(x, y): (f32, f32)| {
            Point::new(
                plot.x() + x / Self::X_RANGE * plot.width(),
                plot.max_y() - y / Self::Y_RANGE * plot.height(),
            )
        };
        let dot = |ctx: &mut PaintCtx, center: Point, fill: Color, radius: f32| {
            let mut path = PathBuilder::new();
            path.push_circle(center, radius);
            let path = path.build();
            ctx.fill(path.clone(), fill);
            ctx.stroke(path, theme.palette.text, StrokeStyle::new(1.0));
        };
        dot(ctx, to_point(D65_WHITE), Color::WHITE, 3.0);
        for (_, [red, green, blue]) in GAMUT_PROBES {
            let p3 = Color::display_p3(red, green, blue, 1.0);
            if let Some(xy) = chromaticity(p3) {
                dot(ctx, to_point(xy), clipped_to_srgb(p3, None), 4.0);
            }
        }

        paint_text_line(
            ctx,
            Rect::new(
                bounds.x(),
                plot.max_y() + 2.0,
                bounds.width(),
                style.line_height,
            ),
            if wide {
                "Bold: Display P3, this output's gamut. Thin: sRGB."
            } else {
                "Bold: sRGB, this output's gamut. Thin: Display P3."
            },
            &style,
            TextAlign::Start,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(
            ctx,
            CHROMATICITY_NAME,
            Some("The sRGB and Display P3 gamuts on the CIE 1931 xy diagram, with the wide-gamut probes plotted.".to_string()),
        );
    }
}

pub(crate) const RAMPS_NAME: &str = "Gradient ramps";

enum RampFill {
    /// Evenly spaced gradient stops.
    Smooth(Vec<Color>),
    /// Solid steps.
    Stepped(Vec<Color>),
}

/// Labeled ramps for judging smoothness and banding.
pub(crate) struct GradientRamps {
    theme_reader: DevThemeReader,
    ramps: Vec<(&'static str, RampFill)>,
}

impl GradientRamps {
    const LABEL_WIDTH: f32 = 200.0;
    const RAMP_HEIGHT: f32 = 32.0;
    const GAP: f32 = 10.0;

    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        let srgb_steps = |count: usize, top: f32| {
            (0..count)
                .map(|step| {
                    let value = top * step as f32 / (count - 1) as f32;
                    Color::srgba(value, value, value, 1.0)
                })
                .collect::<Vec<_>>()
        };
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            ramps: vec![
                ("Black to SDR white", RampFill::Smooth(srgb_steps(8, 1.0))),
                (
                    "Shadows, black to 25% gray",
                    RampFill::Smooth(srgb_steps(8, 0.25)),
                ),
                (
                    "32 steps, for comparison",
                    RampFill::Stepped(srgb_steps(32, 1.0)),
                ),
                (
                    "SDR white to 8×",
                    RampFill::Smooth(vec![
                        linear(1.0, 1.0, 1.0),
                        linear(2.0, 2.0, 2.0),
                        linear(4.0, 4.0, 4.0),
                        linear(8.0, 8.0, 8.0),
                    ]),
                ),
                (
                    "Display P3 red to green",
                    RampFill::Smooth(vec![
                        Color::display_p3(1.0, 0.0, 0.0, 1.0),
                        Color::display_p3(1.0, 1.0, 0.0, 1.0),
                        Color::display_p3(0.0, 1.0, 0.0, 1.0),
                    ]),
                ),
            ],
        }
    }
}

impl Widget for GradientRamps {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let count = self.ramps.len() as f32;
        constraints.clamp(Size::new(
            available_width(constraints, 720.0),
            count * Self::RAMP_HEIGHT + (count - 1.0) * Self::GAP,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let style = label_style(&self.theme_reader);
        let outline = outline_color(&self.theme_reader);
        for (index, (label, fill)) in self.ramps.iter().enumerate() {
            let y = bounds.y() + index as f32 * (Self::RAMP_HEIGHT + Self::GAP);
            paint_text_line(
                ctx,
                Rect::new(bounds.x(), y, Self::LABEL_WIDTH - 12.0, Self::RAMP_HEIGHT),
                label,
                &style,
                TextAlign::Start,
            );
            let ramp = Rect::new(
                bounds.x() + Self::LABEL_WIDTH,
                y,
                (bounds.width() - Self::LABEL_WIDTH).max(1.0),
                Self::RAMP_HEIGHT,
            );
            match fill {
                RampFill::Smooth(colors) => {
                    let last = (colors.len() - 1) as f32;
                    ctx.fill_rect(
                        ramp,
                        Brush::LinearGradient {
                            start: Point::new(ramp.x(), ramp.y()),
                            end: Point::new(ramp.max_x(), ramp.y()),
                            stops: colors
                                .iter()
                                .enumerate()
                                .map(|(stop, color)| GradientStop {
                                    offset: stop as f32 / last,
                                    color: *color,
                                })
                                .collect(),
                        },
                    );
                }
                RampFill::Stepped(colors) => {
                    let step = ramp.width() / colors.len() as f32;
                    for (index, color) in colors.iter().enumerate() {
                        let x = ramp.x() + index as f32 * step;
                        ctx.fill_rect(
                            Rect::new(x, ramp.y(), ramp.max_x() - x, ramp.height()),
                            *color,
                        );
                    }
                }
            }
            ctx.stroke_rect(ramp, outline, StrokeStyle::new(1.0));
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        push_container(
            ctx,
            RAMPS_NAME,
            Some(
                self.ramps
                    .iter()
                    .map(|(label, _)| *label)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
    }
}
