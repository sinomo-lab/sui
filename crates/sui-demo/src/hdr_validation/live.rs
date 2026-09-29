//! What the window's output does right now, read from the output
//! diagnostics the platform publishes after each presented frame.

use sui::prelude::*;
use sui::{
    DisplayColorPrimaries, InvalidationKind, OutputStrategy, Rect, RequestedToneMappingMode,
    SemanticsNode, SemanticsRole, WakeEvent, WindowId, WindowOutputDiagnostics,
    WindowRenderOptions, WindowToneMappingMode, window_output_diagnostics, window_render_options,
};

use super::controls::output_options_changes;
use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};
use crate::demo_support::DemoTextColor;

/// How many frames a widget waits for diagnostics to catch up with a change
/// before it stops asking.
const MAX_FOLLOW_FRAMES: u32 = 120;

/// The latest output diagnostics, for a widget that shows them.
///
/// Diagnostics describe the last presented frame, so after the output
/// options change they trail by a frame or two. The widget asks for
/// animation frames until they catch up, then is measured again with the
/// new values. Changes made with the output controls start this; call
/// [`refresh`](Self::refresh) from `measure` and [`on_event`](Self::on_event)
/// from `event`.
pub(crate) struct FollowedDiagnostics {
    shown: Option<WindowOutputDiagnostics>,
    frames_waited: u32,
}

impl FollowedDiagnostics {
    pub(crate) fn new() -> Self {
        Self {
            shown: None,
            frames_waited: 0,
        }
    }

    /// Take the latest diagnostics, and keep watching while they trail the
    /// window's options.
    pub(crate) fn refresh(&mut self, ctx: &mut MeasureCtx) {
        ctx.observe_with(output_options_changes(), InvalidationKind::Measure);
        let window_id = ctx.window_id();
        self.shown = window_output_diagnostics(window_id);
        if describes_window_options(window_id, self.shown.as_ref()) {
            self.frames_waited = 0;
        } else if self.frames_waited < MAX_FOLLOW_FRAMES {
            ctx.request_animation_frame();
        }
    }

    pub(crate) fn get(&self) -> Option<&WindowOutputDiagnostics> {
        self.shown.as_ref()
    }

    /// Look again on the frames [`refresh`](Self::refresh) asked for.
    pub(crate) fn on_event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !matches!(event, Event::Wake(WakeEvent::AnimationFrame { .. })) {
            return;
        }
        let window_id = ctx.window_id();
        let latest = window_output_diagnostics(window_id);
        if latest != self.shown {
            self.frames_waited = 0;
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
        } else if !describes_window_options(window_id, latest.as_ref())
            && self.frames_waited < MAX_FOLLOW_FRAMES
        {
            self.frames_waited += 1;
            ctx.request_animation_frame();
        }
    }
}

/// Whether `diagnostics` were produced with the window's current options.
fn describes_window_options(
    window_id: WindowId,
    diagnostics: Option<&WindowOutputDiagnostics>,
) -> bool {
    let Some(diagnostics) = diagnostics else {
        return false;
    };
    // Platforms fall back to these when a window has no options.
    let options =
        window_render_options(window_id).unwrap_or_else(|| WindowRenderOptions::new(false, 0.0));
    diagnostics.requested_color_management_mode == options.color_management_mode
        && diagnostics.requested_output_primaries == options.output_color_primaries
        && diagnostics.requested_dynamic_range_mode == options.dynamic_range_mode
        && diagnostics.requested_tone_mapping_mode == options.tone_mapping_mode
        && diagnostics.configured_sdr_content_brightness_nits == options.sdr_content_brightness_nits
        && diagnostics.use_system_sdr_content_brightness
            == options.use_system_sdr_content_brightness
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
                "the cards differ in color, not brightness: accents above SDR white fit to it."
                    .to_string()
            } else {
                "the cards look nearly alike: accents above SDR white fit to it and wide-gamut colors clip to sRGB.".to_string()
            }
        })
    }
}

fn primaries_name(primaries: DisplayColorPrimaries) -> &'static str {
    match primaries {
        DisplayColorPrimaries::Srgb => "sRGB",
        DisplayColorPrimaries::DisplayP3 => "Display P3",
    }
}

/// Paint `text` wrapped to `rect`'s width from its top-left corner.
pub(crate) fn paint_paragraph(ctx: &mut PaintCtx, rect: Rect, text: &str, style: TextStyle) {
    ctx.draw_text(rect, text.to_string(), style);
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
    text: String,
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
            text: String::new(),
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
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.diagnostics.on_event(ctx, event);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.diagnostics.refresh(ctx);
        self.text = (self.source)(self.diagnostics.get());
        let style = self.style();
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            640.0
        };
        let height = ctx
            .layout()
            .shape_text(
                self.text.clone(),
                Size::new(width.max(1.0), f32::INFINITY),
                style.clone(),
            )
            .map(|layout| layout.measurement().height)
            .unwrap_or(style.line_height)
            .max(style.line_height);
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        paint_paragraph(ctx, ctx.bounds(), &self.text, self.style());
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.text.clone());
        ctx.push(node);
    }
}
