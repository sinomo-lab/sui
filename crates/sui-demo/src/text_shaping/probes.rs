//! Samples that lay themselves out and check the result: whether any glyph
//! is missing, which fonts drew them, and what a script needs from shaping.

use std::rc::Rc;

use sui::prelude::*;
use sui::{
    Rect, SemanticsNode, SemanticsRole, TextDirection, TextDocument, TextFlowDirection, TextLayout,
    TextLayoutRequest, TextStyle, TextWrap, WidgetId, paint_text_line,
};

use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};

/// What a sample must shape correctly besides having every glyph.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Check {
    /// Every glyph is present.
    Glyphs,
    /// `letter` takes a different form between two of itself than alone,
    /// as joining scripts need.
    Joins { letter: &'static str },
    /// `sequence` shapes as one cluster: marks attach to their letter,
    /// conjuncts form, and emoji sequences stay together.
    OneCluster {
        sequence: &'static str,
        what: &'static str,
    },
}

/// A script sample and the check it must pass.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScriptSample {
    pub(crate) script: &'static str,
    pub(crate) sample: &'static str,
    pub(crate) check: Check,
}

pub(crate) const SCRIPTS: &[ScriptSample] = &[
    ScriptSample {
        script: "Latin",
        sample: "Façade naïve — Ångström, Łódź, 1½",
        check: Check::Glyphs,
    },
    ScriptSample {
        script: "Greek",
        sample: "Καλημέρα κόσμε",
        check: Check::Glyphs,
    },
    ScriptSample {
        script: "Cyrillic",
        sample: "Съешь же ещё этих булок",
        check: Check::Glyphs,
    },
    ScriptSample {
        script: "Arabic",
        sample: "مرحبا بالعالم",
        check: Check::Joins { letter: "ب" },
    },
    ScriptSample {
        script: "Hebrew",
        sample: "שָׁלוֹם עוֹלָם",
        check: Check::OneCluster {
            sequence: "שָׁ",
            what: "the letter and its points",
        },
    },
    ScriptSample {
        script: "Devanagari",
        sample: "नमस्ते क्षत्रिय",
        check: Check::OneCluster {
            sequence: "क्ष",
            what: "the conjunct क्ष",
        },
    },
    ScriptSample {
        script: "Thai",
        sample: "สวัสดีชาวโลก",
        check: Check::Glyphs,
    },
    ScriptSample {
        script: "Chinese, Japanese, Korean",
        sample: "你好世界 こんにちは 안녕하세요",
        check: Check::Glyphs,
    },
    ScriptSample {
        script: "Emoji",
        sample: "👋🏽 👨‍👩‍👧 🇯🇵 ✅",
        check: Check::OneCluster {
            sequence: "👨‍👩‍👧",
            what: "the family sequence",
        },
    },
];

pub(crate) const SAMPLE_SIZE: f32 = 22.0;

/// How a sample laid out on this system.
#[derive(Debug, Clone, Default)]
pub(crate) struct Result {
    pub(crate) passed: bool,
    /// What passed or failed, in a sentence.
    pub(crate) verdict: String,
    /// Which fonts drew the sample.
    pub(crate) fonts: Vec<String>,
}

fn sample_style(theme: DefaultTheme) -> TextStyle {
    let mut style = demo_text_style(theme, DemoTextRole::Body, theme.palette.text);
    style.font_size = SAMPLE_SIZE;
    style.line_height = (SAMPLE_SIZE * 1.4).ceil();
    style
}

/// Lay `text` out on one line.
fn shape(ctx: &MeasureCtx, text: &str, style: &TextStyle) -> Option<TextLayout> {
    let mut document = TextDocument::from_plain_text(text.to_string(), style.clone());
    for paragraph in &mut document.paragraphs {
        paragraph.style.wrap = TextWrap::NoWrap;
    }
    ctx.layout()
        .layout_document(
            TextLayoutRequest::new(document).with_box_size(Size::new(f32::INFINITY, f32::INFINITY)),
        )
        .ok()
}

fn fonts(layout: &TextLayout) -> Vec<String> {
    let mut fonts = Vec::new();
    for index in 0..layout.runs().len() {
        let name = layout
            .run_face(index)
            .family_name()
            .unwrap_or_else(|| "an unnamed font".to_string());
        if !fonts.contains(&name) {
            fonts.push(name);
        }
    }
    fonts
}

fn missing_glyphs(layout: &TextLayout) -> usize {
    layout
        .glyphs()
        .iter()
        .filter(|glyph| glyph.glyph_id == 0)
        .count()
}

fn check(ctx: &MeasureCtx, sample: ScriptSample, style: &TextStyle, layout: &TextLayout) -> Result {
    let fonts = fonts(layout);
    let missing = missing_glyphs(layout);
    if missing > 0 {
        return Result {
            passed: false,
            verdict: format!(
                "{missing} {} missing: no font on this system covers them.",
                if missing == 1 {
                    "glyph is"
                } else {
                    "glyphs are"
                }
            ),
            fonts,
        };
    }
    let (passed, verdict) = match sample.check {
        Check::Glyphs => (true, "Every glyph is present.".to_string()),
        Check::Joins { letter } => {
            let alone = shape(ctx, letter, style);
            let joined = shape(ctx, &letter.repeat(3), style);
            let alone =
                alone.and_then(|layout| layout.glyphs().first().map(|glyph| glyph.glyph_id));
            let middle =
                joined.and_then(|layout| layout.glyphs().get(1).map(|glyph| glyph.glyph_id));
            match (alone, middle) {
                (Some(alone), Some(middle)) if alone != middle => (
                    true,
                    format!("Letters join: {letter} between two others takes its medial form."),
                ),
                _ => (
                    false,
                    format!(
                        "Letters do not join: {letter} keeps its isolated form between two others."
                    ),
                ),
            }
        }
        Check::OneCluster { sequence, what } => {
            let clusters = shape(ctx, sequence, style).map_or(0, |layout| layout.clusters().len());
            if clusters == 1 {
                (
                    true,
                    format!("Every glyph is present, and {what} shapes as one cluster."),
                )
            } else {
                (
                    false,
                    format!("{what} shapes as {clusters} clusters instead of one."),
                )
            }
        }
    };
    Result {
        passed,
        verdict,
        fonts,
    }
}

const SCRIPT_WIDTH: f32 = 200.0;
const GAP: f32 = 16.0;
const ROW_GAP: f32 = 6.0;

/// A row per script: its name, its sample as laid out here, and whether it
/// passed its check.
pub(crate) struct ScriptTable {
    name: &'static str,
    theme_reader: DevThemeReader,
    rows: Vec<(ScriptSample, Option<TextLayout>, Result)>,
    /// How many passed, shared with the summary above the table.
    passed: Rc<std::cell::Cell<(usize, usize)>>,
}

impl ScriptTable {
    pub(crate) fn new(
        name: &'static str,
        theme_reader: &DevThemeReader,
        passed: Rc<std::cell::Cell<(usize, usize)>>,
    ) -> Self {
        Self {
            name,
            theme_reader: Rc::clone(theme_reader),
            rows: Vec::new(),
            passed,
        }
    }

    fn detail_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text_muted)
    }

    fn label_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Body, theme.palette.text)
    }

    fn row_height(&self, layout: Option<&TextLayout>) -> f32 {
        let sample = layout.map_or(SAMPLE_SIZE * 1.4, |layout| layout.measurement().height);
        let details = self.detail_style().line_height * 2.0;
        sample.max(self.label_style().line_height) + ROW_GAP + details
    }
}

impl Widget for ScriptTable {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = (self.theme_reader)();
        let style = sample_style(theme);
        self.rows = SCRIPTS
            .iter()
            .map(|sample| {
                let layout = shape(ctx, sample.sample, &style);
                let result = layout.as_ref().map_or_else(
                    || Result {
                        passed: false,
                        verdict: "The sample could not be laid out.".to_string(),
                        fonts: Vec::new(),
                    },
                    |layout| check(ctx, *sample, &style, layout),
                );
                (*sample, layout, result)
            })
            .collect();
        let passed = self
            .rows
            .iter()
            .filter(|(_, _, result)| result.passed)
            .count();
        self.passed.set((passed, self.rows.len()));
        let height = self
            .rows
            .iter()
            .map(|(_, layout, _)| self.row_height(layout.as_ref()) + GAP)
            .sum::<f32>();
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            900.0
        };
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = (self.theme_reader)();
        let label = self.label_style();
        let detail = self.detail_style();
        let mut y = bounds.y();
        for (sample, layout, result) in &self.rows {
            let height = self.row_height(layout.as_ref());
            paint_text_line(
                ctx,
                Rect::new(bounds.x(), y, SCRIPT_WIDTH, label.line_height),
                sample.script,
                &label,
                TextAlign::Start,
            );
            let sample_x = bounds.x() + SCRIPT_WIDTH + GAP;
            if let Some(layout) = layout {
                ctx.push_clip_rect(Rect::new(
                    sample_x,
                    y,
                    (bounds.max_x() - sample_x).max(0.0),
                    layout.measurement().height,
                ));
                ctx.draw_text_layout(Point::new(sample_x, y), layout);
                ctx.pop_clip();
            }
            let sample_height = layout
                .as_ref()
                .map_or(SAMPLE_SIZE * 1.4, |layout| layout.measurement().height)
                .max(label.line_height);
            let (mark, color) = if result.passed {
                ("✓", theme.palette.success)
            } else {
                ("✗", theme.palette.danger)
            };
            let verdict_style = TextStyle {
                color,
                ..detail.clone()
            };
            let detail_y = y + sample_height + ROW_GAP;
            let detail_width = (bounds.max_x() - sample_x).max(0.0);
            paint_text_line(
                ctx,
                Rect::new(sample_x, detail_y, detail_width, detail.line_height),
                &format!("{mark} {}", result.verdict),
                &verdict_style,
                TextAlign::Start,
            );
            paint_text_line(
                ctx,
                Rect::new(
                    sample_x,
                    detail_y + detail.line_height,
                    detail_width,
                    detail.line_height,
                ),
                &drawn_with(&result.fonts),
                &detail,
                TextAlign::Start,
            );
            y += height + GAP;
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        ctx.push(node);
        let bounds = ctx.bounds();
        let mut y = bounds.y();
        for (index, (sample, layout, result)) in self.rows.iter().enumerate() {
            let height = self.row_height(layout.as_ref());
            let mut row = SemanticsNode::new(
                row_semantics_id(ctx.widget_id(), index),
                SemanticsRole::GenericContainer,
                Rect::new(bounds.x(), y, bounds.width(), height),
            );
            row.name = Some(format!("{} probe", sample.script));
            row.value = Some(sui::SemanticsValue::Text(sample.sample.to_string()));
            row.description = Some(format!(
                "{} {} {}",
                if result.passed { "Passed." } else { "Failed." },
                result.verdict,
                drawn_with(&result.fonts)
            ));
            ctx.push(row);
            y += height + GAP;
        }
    }
}

/// An id for the accessibility node of the table's row `index`.
fn row_semantics_id(table: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 6_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(TAG | (table.get().wrapping_mul(131).wrapping_add(index as u64 + 1) & LOW_MASK))
}

fn drawn_with(fonts: &[String]) -> String {
    match fonts {
        [] => "No font drew it.".to_string(),
        [font] => format!("Drawn with {font}."),
        [rest @ .., last] => format!("Drawn with {} and {last}.", rest.join(", ")),
    }
}

/// The same mixed-direction text laid out left to right and right to left,
/// with each run underlined by its direction.
pub(crate) struct BidiProbe {
    name: &'static str,
    theme_reader: DevThemeReader,
    text: &'static str,
    layouts: Vec<(TextDirection, TextLayout)>,
}

impl BidiProbe {
    pub(crate) fn new(
        name: &'static str,
        theme_reader: &DevThemeReader,
        text: &'static str,
    ) -> Self {
        Self {
            name,
            theme_reader: Rc::clone(theme_reader),
            text,
            layouts: Vec::new(),
        }
    }

    fn caption_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text_muted)
    }

    /// The runs in visual order, each with its text and whether it goes
    /// right to left.
    fn runs(&self, layout: &TextLayout) -> Vec<(String, bool, Rect)> {
        let mut runs = layout
            .runs()
            .iter()
            .map(|run| {
                (
                    self.text
                        .get(run.byte_range.clone())
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    run.direction == TextFlowDirection::RightToLeft,
                    run.rect,
                )
            })
            .filter(|(text, _, _)| !text.is_empty())
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| left.2.x().total_cmp(&right.2.x()));
        runs
    }
}

const BIDI_WIDTH: f32 = 560.0;
/// Space between a paragraph and its frame.
const BIDI_INSET: f32 = 10.0;

fn paragraph_name(direction: TextDirection) -> &'static str {
    match direction {
        TextDirection::RightToLeft => "Right-to-left paragraph",
        _ => "Left-to-right paragraph",
    }
}

impl Widget for BidiProbe {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = (self.theme_reader)();
        let style = sample_style(theme);
        self.layouts = [TextDirection::LeftToRight, TextDirection::RightToLeft]
            .into_iter()
            .filter_map(|direction| {
                let mut document =
                    TextDocument::from_plain_text(self.text.to_string(), style.clone());
                for paragraph in &mut document.paragraphs {
                    paragraph.style.wrap = TextWrap::NoWrap;
                    paragraph.style.direction = direction;
                    paragraph.style.align = sui::TextAlign::Start;
                }
                ctx.layout()
                    .layout_document(
                        TextLayoutRequest::new(document)
                            .with_box_size(Size::new(BIDI_WIDTH - BIDI_INSET * 2.0, f32::INFINITY)),
                    )
                    .ok()
                    .map(|layout| (direction, layout))
            })
            .collect();
        let caption = self.caption_style().line_height;
        let height = self
            .layouts
            .iter()
            .map(|(_, layout)| caption * 2.0 + layout.measurement().height + BIDI_INSET * 2.0 + GAP)
            .sum::<f32>();
        constraints.clamp(Size::new(BIDI_WIDTH, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let caption = self.caption_style();
        let bounds = ctx.bounds();
        let mut y = bounds.y();
        for (direction, layout) in &self.layouts {
            paint_text_line(
                ctx,
                Rect::new(bounds.x(), y, bounds.width(), caption.line_height),
                paragraph_name(*direction),
                &caption,
                TextAlign::Start,
            );
            y += caption.line_height;
            let frame = Rect::new(
                bounds.x(),
                y,
                BIDI_WIDTH,
                layout.measurement().height + BIDI_INSET * 2.0,
            );
            let text = Point::new(frame.x() + BIDI_INSET, frame.y() + BIDI_INSET);
            ctx.stroke_rect(
                frame,
                theme.palette.border,
                StrokeStyle::new(ctx.dpi().hairline_width()),
            );
            ctx.draw_text_layout(text, layout);
            let runs = self.runs(layout);
            for (_, rtl, rect) in &runs {
                let color = if *rtl {
                    theme.palette.warning
                } else {
                    theme.palette.info
                };
                ctx.fill_rect(
                    Rect::new(
                        text.x + rect.x(),
                        text.y + rect.max_y() - 2.0,
                        rect.width(),
                        2.0,
                    ),
                    color,
                );
            }
            y += frame.height();
            let order = runs
                .iter()
                .map(|(text, rtl, _)| format!("{} {text}", if *rtl { "←" } else { "→" }))
                .collect::<Vec<_>>()
                .join("   ");
            paint_text_line(
                ctx,
                Rect::new(bounds.x(), y, bounds.width(), caption.line_height),
                &format!("Runs, left to right: {order}"),
                &caption,
                TextAlign::Start,
            );
            y += caption.line_height + GAP;
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
            self.layouts
                .iter()
                .map(|(direction, layout)| {
                    let runs = self.runs(layout);
                    let rtl = runs.iter().filter(|(_, rtl, _)| *rtl).count();
                    format!(
                        "{}: {} runs, {rtl} right to left.",
                        paragraph_name(*direction),
                        runs.len()
                    )
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
        ctx.push(node);
    }
}

/// A large sample with its vertical metrics drawn as guide lines.
pub(crate) struct MetricsProbe {
    name: &'static str,
    theme_reader: DevThemeReader,
    layout: Option<TextLayout>,
}

impl MetricsProbe {
    const TEXT: &'static str = "Hxgé Ág";
    const SIZE: f32 = 72.0;
    const LABEL_WIDTH: f32 = 110.0;

    pub(crate) fn new(name: &'static str, theme_reader: &DevThemeReader) -> Self {
        Self {
            name,
            theme_reader: Rc::clone(theme_reader),
            layout: None,
        }
    }
}

impl Widget for MetricsProbe {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = (self.theme_reader)();
        let mut style = demo_text_style(theme, DemoTextRole::Body, theme.palette.text);
        style.font_size = Self::SIZE;
        style.line_height = (Self::SIZE * 1.3).ceil();
        self.layout = shape(ctx, Self::TEXT, &style);
        let width = self.layout.as_ref().map_or(400.0, |layout| {
            layout.measurement().width + (Self::LABEL_WIDTH + GAP) * 2.0
        });
        let height = self
            .layout
            .as_ref()
            .map_or(100.0, |layout| layout.measurement().height);
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(layout) = &self.layout else {
            return;
        };
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        let text_x = bounds.x() + Self::LABEL_WIDTH + GAP;
        let origin = Point::new(text_x, bounds.y());
        let Some(line) = layout.lines().first() else {
            return;
        };
        let measurement = layout.measurement();
        let baseline = origin.y + line.baseline;
        let hairline = ctx.dpi().hairline_width();
        let label = demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text_muted);
        // The font's metrics are labeled on the left and the line box on the
        // right, as they can coincide.
        let mut guides = vec![
            (
                "Ascent",
                baseline - measurement.ascent,
                theme.palette.info,
                true,
            ),
            ("Baseline", baseline, theme.palette.danger, true),
            (
                "Descent",
                baseline + measurement.descent,
                theme.palette.info,
                true,
            ),
            (
                "Line top",
                origin.y + line.rect.y(),
                theme.palette.text_muted,
                false,
            ),
            (
                "Line bottom",
                origin.y + line.rect.max_y(),
                theme.palette.text_muted,
                false,
            ),
        ];
        if let Some(cap_height) = measurement.cap_height {
            guides.insert(
                1,
                (
                    "Cap height",
                    baseline - cap_height,
                    theme.palette.success,
                    true,
                ),
            );
        }
        let width = measurement.width;
        for (name, y, color, left) in guides {
            ctx.fill_rect(
                Rect::new(text_x, y - hairline * 0.5, width, hairline),
                color,
            );
            let (x, align) = if left {
                (bounds.x(), TextAlign::End)
            } else {
                (text_x + width + GAP, TextAlign::Start)
            };
            paint_text_line(
                ctx,
                Rect::new(
                    x,
                    y - label.line_height * 0.5,
                    Self::LABEL_WIDTH,
                    label.line_height,
                ),
                name,
                &TextStyle {
                    color,
                    ..label.clone()
                },
                align,
            );
        }
        ctx.draw_text_layout(origin, layout);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        if let Some(layout) = &self.layout {
            let measurement = layout.measurement();
            node.description = Some(format!(
                "\"{}\" at {} px: ascent {:.1}, descent {:.1}, cap height {}.",
                Self::TEXT,
                Self::SIZE,
                measurement.ascent,
                measurement.descent,
                measurement
                    .cap_height
                    .map_or_else(|| "unknown".to_string(), |cap| format!("{cap:.1}")),
            ));
        }
        ctx.push(node);
    }
}
