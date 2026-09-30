//! The rows at the top of Settings summing up what the window's output
//! shows.

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{
    HdrThemeMode, OutputColorRange, WidgetPodMutVisitor, WidgetPodVisitor, WindowOutputDiagnostics,
    window_output_color_range_signal, window_output_diagnostics_signal,
};

use super::controls::hdr_theme_mode_label;
use crate::app::{DevThemeReader, clone_dev_theme_reader};
use crate::hdr_theme_mode::hdr_theme_mode_signal;
use crate::hdr_validation::OutputSummary;

pub(crate) const OUTPUT_ROW_NAME: &str = "Output";
pub(crate) const SDR_WHITE_ROW_NAME: &str = "SDR white";
pub(crate) const HDR_THEME_ROW_NAME: &str = "HDR theme";

const WAITING_FOR_OUTPUT: &str = "Waiting for the first frame";

#[derive(Default)]
struct Rows {
    output: String,
    sdr_white: String,
    hdr_theme: String,
}

impl Rows {
    fn of(
        diagnostics: Option<&WindowOutputDiagnostics>,
        range: Option<OutputColorRange>,
        mode: HdrThemeMode,
    ) -> Self {
        Self {
            output: diagnostics.map_or_else(
                || WAITING_FOR_OUTPUT.to_string(),
                |diagnostics| OutputSummary::of(diagnostics).label(),
            ),
            sdr_white: diagnostics.map_or_else(|| WAITING_FOR_OUTPUT.to_string(), sdr_white),
            hdr_theme: hdr_theme(mode, range),
        }
    }
}

fn sdr_white(diagnostics: &WindowOutputDiagnostics) -> String {
    let nits = diagnostics.requested_sdr_content_brightness_nits;
    match (
        diagnostics.use_system_sdr_content_brightness,
        diagnostics.system_sdr_content_brightness_nits,
    ) {
        (true, Some(_)) => format!("{nits:.0} nits, from the system"),
        (true, None) => format!("{nits:.0} nits, set here: the system does not report its own"),
        (false, _) => format!("{nits:.0} nits, set here"),
    }
}

fn hdr_theme(mode: HdrThemeMode, range: Option<OutputColorRange>) -> String {
    let shown = mode.limited_to(range);
    if shown == mode {
        return hdr_theme_mode_label(mode).to_string();
    }
    let fallback = match shown {
        HdrThemeMode::Disabled => "the SDR baseline",
        HdrThemeMode::WideGamutOnly => "wide gamut only",
        HdrThemeMode::ConstrainedHdr => "constrained HDR",
        HdrThemeMode::FullHdr => "full HDR",
    };
    format!(
        "{}, shown as {fallback} on this output",
        hdr_theme_mode_label(mode)
    )
}

/// What the window's output shows: what it presents, its SDR white, and the
/// HDR theme mode widgets style for it. Follows every presented frame.
pub(crate) struct OutputSummaryRows {
    rows: Rc<RefCell<Rows>>,
    content: SingleChild,
}

impl OutputSummaryRows {
    pub(crate) fn new(theme_reader: &DevThemeReader) -> Self {
        let rows = Rc::new(RefCell::new(Rows::default()));
        let row = |label: &'static str, value: fn(&Rows) -> String| {
            let rows = Rc::clone(&rows);
            DetailRow::new(label, "")
                .theme_when(clone_dev_theme_reader(theme_reader))
                .value_when(move || value(&rows.borrow()))
        };
        let content = Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(row(OUTPUT_ROW_NAME, |rows| rows.output.clone()))
            .with_child(row(SDR_WHITE_ROW_NAME, |rows| rows.sdr_white.clone()))
            .with_child(row(HDR_THEME_ROW_NAME, |rows| rows.hdr_theme.clone()));
        Self {
            rows,
            content: SingleChild::new(content),
        }
    }
}

impl Widget for OutputSummaryRows {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let window_id = ctx.window_id();
        let diagnostics = ctx.observe(&window_output_diagnostics_signal(window_id));
        let range = ctx.observe(&window_output_color_range_signal(window_id));
        let mode = ctx.observe(&hdr_theme_mode_signal());
        *self.rows.borrow_mut() = Rows::of(diagnostics.as_ref(), range, mode);
        self.content.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hdr_theme_row_says_when_the_output_limits_the_mode() {
        assert_eq!(
            hdr_theme(
                HdrThemeMode::FullHdr,
                Some(OutputColorRange::HighDynamicRange)
            ),
            "Full HDR"
        );
        assert_eq!(
            hdr_theme(HdrThemeMode::FullHdr, Some(OutputColorRange::WideGamut)),
            "Full HDR, shown as wide gamut only on this output"
        );
        assert_eq!(
            hdr_theme(
                HdrThemeMode::WideGamutOnly,
                Some(OutputColorRange::Standard)
            ),
            "Wide-gamut only, shown as the SDR baseline on this output"
        );
        assert_eq!(
            hdr_theme(HdrThemeMode::Disabled, None),
            "Disabled (SDR baseline)"
        );
    }
}
