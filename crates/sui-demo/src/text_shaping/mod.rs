//! The Scripts and shaping page: a sample of each script that checks itself
//! on this system, mixed-direction text with its runs marked, line breaking
//! at a width you choose, and a font's vertical metrics.

#![forbid(unsafe_code)]

mod probes;
#[cfg(test)]
mod tests;

use std::{cell::Cell, rc::Rc};

use sui::prelude::*;
use sui::{Rect, SemanticsNode, SemanticsRole, TextStyle, WidgetPodMutVisitor, WidgetPodVisitor};

use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style};
use crate::demo_support::*;
use crate::live_performance::LivePerformanceRoot;
use probes::{BidiProbe, MetricsProbe, SCRIPTS, ScriptTable};

pub const TEXT_SHAPING_VIEW_TITLE: &str = "SUI Scripts and Shaping";
pub const TEXT_SHAPING_SCROLL_NAME: &str = "Scripts and shaping scroll";

pub(crate) const SCRIPTS_SECTION_NAME: &str = "Scripts";
pub(crate) const SCRIPT_TABLE_NAME: &str = "Script samples";
pub(crate) const SCRIPTS_SUMMARY_NAME: &str = "Script summary";
pub(crate) const BIDI_SECTION_NAME: &str = "Mixed directions";
pub(crate) const BIDI_PROBE_NAME: &str = "Mixed-direction probe";
pub(crate) const BREAKING_SECTION_NAME: &str = "Line breaking";
pub(crate) const BREAKING_WIDTH_NAME: &str = "Line width";
pub(crate) const BREAKING_PROBE_NAME: &str = "Line breaking probe";
pub(crate) const METRICS_SECTION_NAME: &str = "Vertical metrics";
pub(crate) const METRICS_PROBE_NAME: &str = "Vertical metrics probe";

const BIDI_TEXT: &str = "Order 42 from שלום ships to مرحبا today.";
const BREAKING_TEXT: &str = "中文没有空格，可以在任意两个字之间换行。日本語も同じです。English breaks between words, and a long address like https://example.com/docs/text/shaping/line-breaking only breaks when nothing else fits.";
const BREAKING_WIDTHS: (f64, f64) = (140.0, 620.0);

pub fn build_text_shaping_surface_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let passed = Rc::new(Cell::new((0, SCRIPTS.len())));
    let content = Stack::vertical()
        .spacing(18.0)
        .alignment(Alignment::Stretch)
        .with_child(scripts_section(&theme_reader, passed))
        .with_child(bidi_section(&theme_reader))
        .with_child(breaking_section(&theme_reader))
        .with_child(metrics_section(&theme_reader));
    ScrollView::vertical(Padding::all(24.0, content))
        .name(TEXT_SHAPING_SCROLL_NAME)
        .theme_when(clone_dev_theme_reader(&theme_reader))
}

pub fn build_text_shaping_application() -> Application {
    App::new()
        .window(Window::new(TEXT_SHAPING_VIEW_TITLE).root(LivePerformanceRoot::new(
            TEXT_SHAPING_VIEW_TITLE,
            "Script samples that check themselves on this system, mixed-direction text, line breaking, and vertical metrics.",
            build_text_shaping_surface_with_theme(default_theme_reader()),
        )))
        .into_application()
}

fn section<W>(
    theme_reader: &DevThemeReader,
    name: &'static str,
    subtitle: &'static str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    NamedSection::new(
        name,
        panel_with_theme(Rc::clone(theme_reader), name, subtitle, body),
    )
}

fn body() -> Stack {
    Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch)
}

fn note(theme_reader: &DevThemeReader, text: &'static str) -> impl Widget + use<> {
    MaximumWidth::new(
        GALLERY_TEXT_MAX_WIDTH,
        demo_label(theme_reader, text, DemoTextRole::Body, DemoTextColor::Muted),
    )
}

fn scripts_section(
    theme_reader: &DevThemeReader,
    passed: Rc<Cell<(usize, usize)>>,
) -> impl Widget + use<> {
    let summary = Rc::clone(&passed);
    section(
        theme_reader,
        SCRIPTS_SECTION_NAME,
        "Each sample is laid out with the fonts this system has. A sample passes when no glyph is missing and its script shapes as it must: letters join, marks attach, conjuncts form, and emoji sequences stay together.",
        body()
            .with_child(
                demo_label(theme_reader, "", DemoTextRole::Emphasis, DemoTextColor::Text)
                    .semantic_name(SCRIPTS_SUMMARY_NAME)
                    .text_when(move || {
                        let (passed, total) = summary.get();
                        if passed == total {
                            format!("All {total} scripts shape completely on this system.")
                        } else {
                            format!(
                                "{passed} of {total} scripts shape completely on this system. Install fonts for the others to pass."
                            )
                        }
                    }),
            )
            .with_child(ScriptTable::new(SCRIPT_TABLE_NAME, theme_reader, passed)),
    )
}

fn bidi_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        BIDI_SECTION_NAME,
        "The same sentence mixes English, Hebrew, Arabic, and numbers. Blue underlines mark left-to-right runs and orange ones right-to-left runs.",
        body()
            .with_child(BidiProbe::new(BIDI_PROBE_NAME, theme_reader, BIDI_TEXT))
            .with_child(note(
                theme_reader,
                "Hebrew and Arabic read right to left, and numbers stay left to right inside them. The base direction only decides where the sentence starts and how its runs are ordered: at the left edge with English first, or at the right edge with the runs reversed.",
            )),
    )
}

fn breaking_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let width = Signal::named(BREAKING_WIDTH_NAME, 320.0_f32);
    let read = width.clone();
    let write = width.clone();
    section(
        theme_reader,
        BREAKING_SECTION_NAME,
        "Text wrapped to the width you choose. The box shows the width.",
        body()
            .with_child(
                crate::settings::controls::labeled_control(
                    theme_reader,
                    BREAKING_WIDTH_NAME,
                    360.0,
                    Slider::new(BREAKING_WIDTH_NAME)
                        .theme_when(clone_dev_theme_reader(theme_reader))
                        .range(BREAKING_WIDTHS.0, BREAKING_WIDTHS.1)
                        .step(1.0)
                        .value(f64::from(width.get()))
                        .value_when(move || f64::from(read.get()))
                        .on_change(move |value| {
                            write.set(value as f32);
                        }),
                ),
            )
            .with_child(WrapProbe::new(BREAKING_PROBE_NAME, theme_reader, width))
            .with_child(note(
                theme_reader,
                "Chinese and Japanese break between any two characters, and punctuation never starts a line. English breaks between words. The address breaks only when it is wider than the box on its own.",
            )),
    )
}

fn metrics_section(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    section(
        theme_reader,
        METRICS_SECTION_NAME,
        "A large sample with the font's vertical metrics drawn through it.",
        body()
            .with_child(MetricsProbe::new(METRICS_PROBE_NAME, theme_reader))
            .with_child(note(
                theme_reader,
                "Capital letters reach the cap height, and accents above them reach toward the ascent. Descenders such as g stop at the descent. The line box, the space a line takes, can be taller than both, and text is placed within it by its baseline.",
            )),
    )
}

/// Text wrapped to a width read from a signal, inside a box that shows it.
struct WrapProbe {
    name: &'static str,
    theme_reader: DevThemeReader,
    width: Signal<f32>,
    paragraph: Paragraph,
    box_width: f32,
}

impl WrapProbe {
    fn new(name: &'static str, theme_reader: &DevThemeReader, width: Signal<f32>) -> Self {
        Self {
            name,
            theme_reader: Rc::clone(theme_reader),
            width,
            paragraph: Paragraph::default(),
            box_width: 0.0,
        }
    }

    fn style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Body, theme.palette.text)
    }
}

const WRAP_PADDING: f32 = 10.0;

impl Widget for WrapProbe {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = ctx.observe(&self.width);
        self.box_width = width;
        self.paragraph = Paragraph::new(ctx, BREAKING_TEXT, &self.style(), TextAlign::Start, width);
        let height = self.paragraph.size().height + WRAP_PADDING * 2.0;
        let available = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            width + WRAP_PADDING * 2.0
        };
        constraints.clamp(Size::new(available, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        let frame = Rect::new(
            bounds.x(),
            bounds.y(),
            self.box_width + WRAP_PADDING * 2.0,
            bounds.height(),
        );
        ctx.stroke_rect(
            frame,
            theme.palette.border,
            StrokeStyle::new(ctx.dpi().hairline_width()),
        );
        self.paragraph.paint(
            ctx,
            Rect::new(
                frame.x() + WRAP_PADDING,
                frame.y() + WRAP_PADDING,
                self.box_width,
                frame.height() - WRAP_PADDING * 2.0,
            ),
            VerticalAlign::Top,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.to_string());
        node.description = Some(format!(
            "{} lines at {:.0} px wide.",
            self.paragraph.line_count(),
            self.box_width
        ));
        ctx.push(node);
    }

    fn visit_children(&self, _visitor: &mut dyn WidgetPodVisitor) {}

    fn visit_children_mut(&mut self, _visitor: &mut dyn WidgetPodMutVisitor) {}
}
