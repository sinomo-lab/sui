//! What the window's output does right now, read from the output
//! diagnostics the platform publishes after each presented frame.

use sui::prelude::*;
use sui::{
    DisplayColorPrimaries, OutputStrategy, RequestedToneMappingMode, SemanticsNode, SemanticsRole,
    WindowOutputDiagnostics, WindowToneMappingMode, window_output_diagnostics_signal,
};

use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};
use crate::demo_support::DemoTextColor;

/// The window's output diagnostics, for a widget that shows them. The
/// platform publishes them after each presented frame; a widget that calls
/// [`refresh`](Self::refresh) from `measure` is measured again whenever they
/// change.
pub(crate) struct FollowedDiagnostics {
    shown: Option<WindowOutputDiagnostics>,
}

impl FollowedDiagnostics {
    pub(crate) fn new() -> Self {
        Self { shown: None }
    }

    pub(crate) fn refresh(&mut self, ctx: &mut MeasureCtx) {
        self.shown = ctx.observe(&window_output_diagnostics_signal(ctx.window_id()));
    }

    pub(crate) fn get(&self) -> Option<&WindowOutputDiagnostics> {
        self.shown.as_ref()
    }
}

/// What an output does with colors beyond SDR white.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HighlightFit {
    /// The display receives extended range.
    Extended,
    /// Clipped to SDR white, keeping hue.
    Clip,
    /// Clipped, then turned toward white.
    RollOff,
}

impl HighlightFit {
    /// The renderer's mode for this fit; `None` when nothing is fitted.
    pub(crate) const fn tone_mapping(self) -> Option<RequestedToneMappingMode> {
        match self {
            Self::Extended => None,
            Self::Clip => Some(RequestedToneMappingMode::Clamp),
            Self::RollOff => Some(RequestedToneMappingMode::Reinhard),
        }
    }
}

/// The window's output, reduced to what the probes depend on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OutputSummary {
    pub(crate) kind: OutputKind,
    pub(crate) fit: HighlightFit,
    /// The primaries colors are fitted in and clipped to.
    pub(crate) primaries: DisplayColorPrimaries,
    /// Whether colors outside sRGB reach the display.
    pub(crate) wide_gamut: bool,
    /// How many times brighter than SDR white the display can go, when it
    /// says. Always 1 for SDR outputs.
    pub(crate) headroom: Option<f32>,
    pub(crate) peak_nits: Option<f32>,
    pub(crate) sdr_white_nits: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputKind {
    Sdr,
    WideGamut,
    NativeHdr,
    ToneMappedHdr,
}

impl OutputSummary {
    pub(crate) fn of(diagnostics: &WindowOutputDiagnostics) -> Self {
        let clip_or_roll_off = match diagnostics.requested_tone_mapping_mode {
            WindowToneMappingMode::Reinhard => HighlightFit::RollOff,
            WindowToneMappingMode::Automatic | WindowToneMappingMode::Clamp => HighlightFit::Clip,
        };
        let capabilities = &diagnostics.display_capabilities;
        let sdr_white_nits = diagnostics.requested_sdr_content_brightness_nits;
        let (kind, fit, primaries) = match diagnostics.active_output_strategy {
            OutputStrategy::SdrSurface { .. } => (
                OutputKind::Sdr,
                clip_or_roll_off,
                DisplayColorPrimaries::Srgb,
            ),
            OutputStrategy::WideGamutSurface { primaries, .. } => {
                (OutputKind::WideGamut, clip_or_roll_off, primaries)
            }
            OutputStrategy::HdrNativeSurface { primaries, .. } => {
                (OutputKind::NativeHdr, HighlightFit::Extended, primaries)
            }
            OutputStrategy::HdrIntermediateThenToneMap { primaries, .. } => {
                (OutputKind::ToneMappedHdr, clip_or_roll_off, primaries)
            }
        };
        let headroom = if kind == OutputKind::NativeHdr {
            capabilities.max_content_headroom.or_else(|| {
                capabilities
                    .max_luminance_nits
                    .filter(|_| sdr_white_nits > 0.0)
                    .map(|peak| peak / sdr_white_nits)
            })
        } else {
            Some(1.0)
        };
        // Native HDR output is extended linear sRGB (scRGB), which carries
        // colors outside sRGB as negative channels to a wide-gamut display.
        let wide_gamut = primaries == DisplayColorPrimaries::DisplayP3
            || (kind == OutputKind::NativeHdr && capabilities.supports_wide_gamut);
        Self {
            kind,
            fit,
            primaries,
            wide_gamut,
            headroom: headroom.filter(|headroom| headroom.is_finite() && *headroom > 0.0),
            peak_nits: capabilities.max_luminance_nits,
            sdr_white_nits,
        }
    }

    pub(crate) fn shows_wide_gamut(self) -> bool {
        self.wide_gamut
    }

    /// A few words on what the output is.
    pub(crate) fn label(self) -> String {
        let gamut = if self.wide_gamut { ", wide gamut" } else { "" };
        match self.kind {
            OutputKind::Sdr => "SDR, sRGB".to_string(),
            OutputKind::WideGamut => {
                format!("Wide-gamut SDR, {}", primaries_name(self.primaries))
            }
            OutputKind::ToneMappedHdr => {
                format!("HDR tone mapped to SDR, {}", primaries_name(self.primaries))
            }
            OutputKind::NativeHdr => match self.headroom {
                Some(headroom) => format!("Native HDR, up to {headroom:.1}× SDR white{gamut}"),
                None => format!("Native HDR{gamut}"),
            },
        }
    }

    /// One sentence on what the output is and does with highlights.
    pub(crate) fn headline(self) -> String {
        let highlights = match self.fit {
            HighlightFit::Extended => String::new(),
            HighlightFit::Clip => {
                " Highlights above SDR white are clipped to it, keeping their hue.".to_string()
            }
            HighlightFit::RollOff => {
                " Highlights above SDR white roll off toward white, so brighter ones still read brighter.".to_string()
            }
        };
        let gamut = if self.wide_gamut {
            " Colors outside sRGB reach it too."
        } else {
            ""
        };
        match self.kind {
            OutputKind::Sdr => format!("SDR output in sRGB.{highlights}"),
            OutputKind::WideGamut => {
                format!(
                    "Wide-gamut SDR output in {}.{highlights}",
                    primaries_name(self.primaries)
                )
            }
            OutputKind::ToneMappedHdr => format!(
                "HDR scene tone mapped to SDR in {} (debug path).{highlights}",
                primaries_name(self.primaries)
            ),
            OutputKind::NativeHdr => match (self.headroom, self.peak_nits) {
                (Some(headroom), Some(peak)) => format!(
                    "Native HDR output. Highlights reach the display, up to about {headroom:.1}× SDR white ({peak:.0} nits peak, SDR white at {:.0} nits).{gamut}",
                    self.sdr_white_nits
                ),
                (Some(headroom), None) => format!(
                    "Native HDR output. Highlights reach the display, up to about {headroom:.1}× SDR white.{gamut}"
                ),
                _ => format!(
                    "Native HDR output. Highlights reach the display; it did not report its peak, so the headroom above SDR white ({:.0} nits) is unknown.{gamut}",
                    self.sdr_white_nits
                ),
            },
        }
    }
}

/// What each probe should show on the window's output right now.
pub(crate) mod expect {
    use super::{HighlightFit, OutputKind, OutputSummary, WAITING_FOR_OUTPUT};

    fn on_this_output(
        summary: Option<OutputSummary>,
        text: impl FnOnce(OutputSummary) -> String,
    ) -> String {
        summary.map_or_else(
            || WAITING_FOR_OUTPUT.to_string(),
            |summary| format!("On this output: {}", text(summary)),
        )
    }

    pub(crate) fn headroom(summary: Option<OutputSummary>) -> String {
        on_this_output(summary, |summary| {
            match (summary.fit, summary.headroom) {
            (HighlightFit::Extended, Some(headroom)) => format!(
                "the ramp and the ladder keep brightening up to about {headroom:.1}× and look the same beyond it. 1.05× and 1.1× read slightly brighter than 1×."
            ),
            (HighlightFit::Extended, None) => "the ramp and the ladder keep brightening until the display's peak. 1.05× and 1.1× read slightly brighter than 1×.".to_string(),
            _ => "everything from 1× up is the same white, and only 0.9× and 0.95× are dimmer. White has no hue to keep, so Clamp and Reinhard look alike here.".to_string(),
        }
        })
    }

    pub(crate) fn highlights(summary: Option<OutputSummary>) -> String {
        on_this_output(summary, |summary| {
            let halves = "The two strips, and the two halves of each cell, should match; a seam means the GPU fits differently from the CPU.";
            match summary.fit {
                HighlightFit::Extended => format!(
                    "highlights are sent as they are and keep getting brighter while staying saturated, up to the display's peak. {halves}"
                ),
                HighlightFit::Clip => format!(
                    "cells from 1× up keep their hue at full brightness and look alike; the curves go flat together at 1×. {halves}"
                ),
                HighlightFit::RollOff => format!(
                    "brighter cells turn paler toward white, so each step still reads brighter; the curves meet at 1× as they rise. {halves}"
                ),
            }
        })
    }

    pub(crate) fn gamut(summary: Option<OutputSummary>) -> String {
        on_this_output(summary, |summary| {
            if summary.shows_wide_gamut() {
                "the right half of each tile is more saturated than the left, so a seam runs down the middle.".to_string()
            } else {
                "both halves of each tile look the same, because this output clips Display P3 colors to sRGB.".to_string()
            }
        })
    }

    pub(crate) fn ramps(summary: Option<OutputSummary>) -> String {
        on_this_output(summary, |summary| {
            let hdr_ramp = if summary.kind == OutputKind::NativeHdr {
                "The SDR white to 8× ramp brightens across."
            } else {
                "The SDR white to 8× ramp is flat white."
            };
            let gamut_ramp = if summary.shows_wide_gamut() {
                "The Display P3 ramp stays vivid end to end."
            } else {
                "The Display P3 ramp is clipped to sRGB but still smooth."
            };
            format!("{hdr_ramp} {gamut_ramp}")
        })
    }

    pub(crate) fn ui_modes(summary: Option<OutputSummary>) -> String {
        on_this_output(summary, |summary| {
            if summary.kind == OutputKind::NativeHdr {
                "accents in the Constrained and Full HDR cards glow above SDR white, Full HDR the most; the SDR baseline and wide-gamut cards stay at SDR white.".to_string()
            } else if summary.shows_wide_gamut() {
                "Constrained and Full HDR fall back to wide gamut, so the three cards after the SDR baseline match and differ from it in color only.".to_string()
            } else {
                "every card falls back to the SDR baseline and looks the same, because this output shows neither wide gamut nor HDR.".to_string()
            }
        })
    }
}

/// The SDR content brightness in use, where it comes from, and both values
/// it could come from.
pub(crate) fn sdr_content_brightness_line(diagnostics: &WindowOutputDiagnostics) -> String {
    let source = if diagnostics.use_system_sdr_content_brightness
        && diagnostics.system_sdr_content_brightness_nits.is_some()
    {
        "system"
    } else if diagnostics.use_system_sdr_content_brightness {
        "manual fallback"
    } else {
        "manual"
    };
    let system = diagnostics
        .system_sdr_content_brightness_nits
        .map(|nits| format!("{nits:.0} nits"))
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "SDR content brightness: {:.0} nits ({source}; system {system}, manual {:.0} nits)",
        diagnostics.requested_sdr_content_brightness_nits,
        diagnostics.configured_sdr_content_brightness_nits,
    )
}

fn primaries_name(primaries: DisplayColorPrimaries) -> &'static str {
    match primaries {
        DisplayColorPrimaries::Srgb => "sRGB",
        DisplayColorPrimaries::DisplayP3 => "Display P3",
    }
}

/// Text that depends on the window's output, rewritten as it changes.
pub(crate) type LiveTextSource = Box<dyn Fn(Option<&WindowOutputDiagnostics>) -> String>;

pub(crate) const WAITING_FOR_OUTPUT: &str = "Waiting for the first presented frame…";

/// A paragraph describing the window's output, kept current as it changes.
pub(crate) struct LiveText {
    theme_reader: DevThemeReader,
    role: DemoTextRole,
    color: DemoTextColor,
    source: LiveTextSource,
    diagnostics: FollowedDiagnostics,
    paragraph: Paragraph,
}

impl LiveText {
    pub(crate) fn new(
        theme_reader: &DevThemeReader,
        role: DemoTextRole,
        color: DemoTextColor,
        source: impl Fn(Option<&WindowOutputDiagnostics>) -> String + 'static,
    ) -> Self {
        Self {
            theme_reader: std::rc::Rc::clone(theme_reader),
            role,
            color,
            source: Box::new(source),
            diagnostics: FollowedDiagnostics::new(),
            paragraph: Paragraph::default(),
        }
    }

    fn style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        let color = match self.color {
            DemoTextColor::Text => theme.palette.text,
            DemoTextColor::Muted => theme.palette.text_muted,
        };
        demo_text_style(theme, self.role, color)
    }
}

impl Widget for LiveText {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        let text = (self.source)(self.diagnostics.get());
        let style = self.style();
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            640.0
        };
        self.paragraph = Paragraph::new(ctx, text, &style, TextAlign::Start, width);
        let height = self.paragraph.size().height.max(style.line_height);
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let color = self.style().color;
        self.paragraph
            .paint_with_color(ctx, ctx.bounds(), VerticalAlign::Top, color);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.paragraph.text().to_string());
        ctx.push(node);
    }
}
