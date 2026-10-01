//! Capture the window from inside the app and report what it held.
//!
//! A capture asks the platform for both stages of the next frame: the scene
//! before output conversion and the final output. Natively the result is
//! written as a bundle (see [`super::report`]) and summarized on the page.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use sui::diagnostics::{
    DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest, DebugCaptureStage,
    DebugCaptureTicket, DebugSdrVisualization, HdrRgbaImage, fit_to_sdr,
    request_window_debug_capture, take_window_debug_capture, window_output_diagnostics,
};
use sui::prelude::*;
use sui::{
    AsyncWakeToken, InvalidationKind, InvalidationRequest, InvalidationTarget, Rect,
    RegisteredImage, RequestedToneMappingMode, SemanticsNode, SemanticsRole, TextStyle, WakeEvent,
    WidgetId, WidgetPodMutVisitor, WidgetPodVisitor, WindowId, paint_text_line,
};

use super::report::{
    CaptureMetrics, LightMetrics, final_output_sdr_white, output_diagnostics_report,
};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style};

pub(crate) const CAPTURE_BUTTON_LABEL: &str = "Capture frame";
pub(crate) const COPY_REPORT_BUTTON_LABEL: &str = "Copy report";
pub(crate) const CAPTURE_STATUS_NAME: &str = "Capture status";

/// The box thumbnails are scaled to fit.
const THUMBNAIL_SIZE: (u32, u32) = (280, 200);
const LINE_GAP: f32 = 4.0;
const BLOCK_GAP: f32 = 14.0;

struct Thumbnail {
    caption: &'static str,
    image: RegisteredImage,
    size: Size,
}

enum CaptureState {
    Idle,
    Pending {
        intermediate: DebugCaptureTicket,
        final_output: DebugCaptureTicket,
    },
    Done {
        lines: Vec<String>,
        metrics_report: String,
        thumbnails: Vec<Thumbnail>,
    },
    Failed(String),
}

/// What the panel and its buttons share. The buttons act with their own
/// event context; the panel is woken when a capture is ready.
struct PanelState {
    view: &'static str,
    capture: RefCell<CaptureState>,
    copied: Cell<bool>,
    /// Wake events reach only the widget that registered them, so the panel
    /// registers this one; every event to its buttons passes it first.
    wake: Cell<Option<AsyncWakeToken>>,
    panel: Cell<Option<WidgetId>>,
}

impl PanelState {
    fn start_capture(&self, ctx: &mut EventCtx) {
        let Some(wake) = self.wake.get() else {
            *self.capture.borrow_mut() =
                CaptureState::Failed("the page could not be woken for the result".to_string());
            self.relayout(ctx);
            return;
        };
        let window_id = ctx.window_id();
        let request = |stage| DebugCaptureRequest {
            stage,
            encoding: DebugCaptureEncoding::Exr,
            sdr_visualization: DebugSdrVisualization::ToneMappedColor,
        };
        // Captures are made in order after the next redraw, so waking on
        // the second means both are ready.
        let intermediate = request_window_debug_capture(
            window_id,
            request(DebugCaptureStage::HdrIntermediate),
            None,
        );
        let final_output = request_window_debug_capture(
            window_id,
            request(DebugCaptureStage::FinalComposed),
            Some(wake),
        );
        *self.capture.borrow_mut() = CaptureState::Pending {
            intermediate,
            final_output,
        };
        self.copied.set(false);
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(window_id),
            InvalidationKind::Paint,
        ));
        self.relayout(ctx);
    }

    fn finish_capture(&self, window_id: WindowId) {
        let (intermediate, final_output) = match &*self.capture.borrow() {
            CaptureState::Pending {
                intermediate,
                final_output,
            } => (*intermediate, *final_output),
            _ => return,
        };
        let finished = match (
            take_window_debug_capture(intermediate),
            take_window_debug_capture(final_output),
        ) {
            (Some(Ok(DebugCaptureArtifact::HdrLinearRgbaF32(image))), Some(Ok(final_artifact))) => {
                self.captured(window_id, &image, &final_artifact)
            }
            (Some(Ok(DebugCaptureArtifact::SdrRgba8(_))), _) => CaptureState::Failed(
                "the scene capture came back as 8-bit SDR instead of linear HDR".to_string(),
            ),
            (Some(Err(error)), _) | (_, Some(Err(error))) => {
                CaptureState::Failed(error.to_string())
            }
            _ => CaptureState::Failed("the platform did not return the capture".to_string()),
        };
        *self.capture.borrow_mut() = finished;
    }

    fn captured(
        &self,
        window_id: WindowId,
        intermediate: &HdrRgbaImage,
        final_output: &DebugCaptureArtifact,
    ) -> CaptureState {
        let diagnostics = window_output_diagnostics(window_id);
        let final_sdr_white = final_output_sdr_white(diagnostics.as_ref());
        let report = output_diagnostics_report(self.view, diagnostics.as_ref());
        let preview = Thumbnail::new("Scene, fitted to SDR", scene_preview(intermediate));
        let (metrics, files, maps) =
            write_bundle(intermediate, final_output, final_sdr_white, &report);
        let mut lines = metrics.summary_lines();
        lines.push(files);
        let mut thumbnails = vec![preview];
        thumbnails.extend(maps);
        CaptureState::Done {
            lines,
            metrics_report: metrics.report(),
            thumbnails: thumbnails.into_iter().flatten().collect(),
        }
    }

    fn copy_report(&self, ctx: &mut EventCtx) {
        let mut report = output_diagnostics_report(
            self.view,
            window_output_diagnostics(ctx.window_id()).as_ref(),
        );
        if let CaptureState::Done { metrics_report, .. } = &*self.capture.borrow() {
            report.push_str(metrics_report);
        }
        ctx.set_clipboard_text(report);
        self.copied.set(true);
        self.relayout(ctx);
    }

    fn status_lines(&self) -> Vec<String> {
        let mut lines = match &*self.capture.borrow() {
            CaptureState::Idle => vec![
                "Captures the scene before output conversion and the final output of the next frame."
                    .to_string(),
            ],
            CaptureState::Pending { .. } => vec!["Capturing the next frame…".to_string()],
            CaptureState::Done { lines, .. } => lines.clone(),
            CaptureState::Failed(error) => vec![format!("Capture failed: {error}")],
        };
        if self.copied.get() {
            lines.push("Report copied to the clipboard.".to_string());
        }
        lines
    }

    /// Have the panel lay out its status again, from a button's context.
    fn relayout(&self, ctx: &mut EventCtx) {
        let Some(panel) = self.panel.get() else {
            return;
        };
        for kind in [
            InvalidationKind::Measure,
            InvalidationKind::Paint,
            InvalidationKind::Semantics,
        ] {
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Widget(panel),
                kind,
            ));
        }
    }
}

/// Buttons to capture the frame and copy a report, and what the last
/// capture found.
pub(crate) struct CapturePanel {
    theme_reader: DevThemeReader,
    state: Rc<PanelState>,
    buttons: SingleChild,
    /// The status lines, laid out when measured.
    lines: Vec<Paragraph>,
}

impl CapturePanel {
    pub(crate) fn new(theme_reader: &DevThemeReader, view: &'static str) -> Self {
        let state = Rc::new(PanelState {
            view,
            capture: RefCell::new(CaptureState::Idle),
            copied: Cell::new(false),
            wake: Cell::new(None),
            panel: Cell::new(None),
        });
        let buttons = Stack::horizontal()
            .spacing(10.0)
            .alignment(Alignment::Center)
            .with_child({
                let state = Rc::clone(&state);
                Button::primary(CAPTURE_BUTTON_LABEL)
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .on_press_with_ctx(move |ctx| state.start_capture(ctx))
            })
            .with_child({
                let state = Rc::clone(&state);
                Button::new(COPY_REPORT_BUTTON_LABEL)
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .on_press_with_ctx(move |ctx| state.copy_report(ctx))
            });
        Self {
            theme_reader: Rc::clone(theme_reader),
            state,
            buttons: SingleChild::new(buttons),
            lines: Vec::new(),
        }
    }

    fn text_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Supporting, theme.palette.text)
    }

    fn caption_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text_muted)
    }

    fn thumbnail_sizes(&self) -> Vec<Size> {
        match &*self.state.capture.borrow() {
            CaptureState::Done { thumbnails, .. } => {
                thumbnails.iter().map(|thumbnail| thumbnail.size).collect()
            }
            _ => Vec::new(),
        }
    }
}

impl Thumbnail {
    fn new(caption: &'static str, pixels: Option<(u32, u32, Vec<u8>)>) -> Option<Self> {
        let (width, height, pixels) = pixels?;
        let image = RegisteredImage::from_rgba8(width, height, pixels).ok()?;
        Some(Self {
            caption,
            image,
            size: Size::new(width as f32, height as f32),
        })
    }
}

/// The height a status line takes: at least one line.
fn line_height(line: &Paragraph, style: &TextStyle) -> f32 {
    line.size().height.max(style.line_height)
}

/// Nearest-neighbour samples of a `width` × `height` image scaled to fit
/// [`THUMBNAIL_SIZE`]: the thumbnail's size and the source pixel index of
/// each of its pixels.
fn thumbnail_samples(width: u32, height: u32) -> Option<(u32, u32, Vec<usize>)> {
    if width == 0 || height == 0 {
        return None;
    }
    let scale = (THUMBNAIL_SIZE.0 as f32 / width as f32)
        .min(THUMBNAIL_SIZE.1 as f32 / height as f32)
        .min(1.0);
    let out_width = ((width as f32 * scale).round() as u32).max(1);
    let out_height = ((height as f32 * scale).round() as u32).max(1);
    let mut samples = Vec::with_capacity((out_width * out_height) as usize);
    for y in 0..out_height {
        let source_y = ((y as f32 + 0.5) / scale) as u32;
        for x in 0..out_width {
            let source_x = ((x as f32 + 0.5) / scale) as u32;
            samples.push((source_y.min(height - 1) * width + source_x.min(width - 1)) as usize);
        }
    }
    Some((out_width, out_height, samples))
}

fn linear_to_srgb_u8(channel: f32) -> u8 {
    let channel = channel.clamp(0.0, 1.0);
    let encoded = if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// A small SDR view of a linear capture, highlights clipped keeping hue.
fn scene_preview(image: &HdrRgbaImage) -> Option<(u32, u32, Vec<u8>)> {
    let (width, height, samples) = thumbnail_samples(image.width(), image.height())?;
    let source = image.pixels();
    let mut pixels = Vec::with_capacity(samples.len() * 4);
    for index in samples {
        let rgba = &source[index * 4..index * 4 + 4];
        let fitted = fit_to_sdr(
            [rgba[0].max(0.0), rgba[1].max(0.0), rgba[2].max(0.0)],
            RequestedToneMappingMode::Clamp,
        );
        pixels.extend(fitted.map(linear_to_srgb_u8));
        pixels.push(255);
    }
    Some((width, height, pixels))
}

#[cfg(not(target_arch = "wasm32"))]
fn thumbnail_of(screenshot: &sui_testing::Screenshot) -> Option<(u32, u32, Vec<u8>)> {
    let (width, height, samples) = thumbnail_samples(screenshot.width(), screenshot.height())?;
    let source = screenshot.pixels();
    let mut pixels = Vec::with_capacity(samples.len() * 4);
    for index in samples {
        pixels.extend_from_slice(&source[index * 4..index * 4 + 4]);
    }
    Some((width, height, pixels))
}

/// Where in-app captures go: beside the other demo artifacts.
#[cfg(not(target_arch = "wasm32"))]
fn capture_dir() -> std::path::PathBuf {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map_or_else(std::env::temp_dir, std::path::Path::to_path_buf);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    workspace
        .join("target")
        .join("ui-artifacts")
        .join("sui-demo")
        .join("hdr-validation")
        .join(format!("capture-{stamp}"))
}

/// Write the bundle; returns the measurements, a line about the files, and
/// thumbnails of the maps.
#[cfg(not(target_arch = "wasm32"))]
fn write_bundle(
    intermediate: &HdrRgbaImage,
    final_output: &DebugCaptureArtifact,
    final_sdr_white: f32,
    diagnostics: &str,
) -> (CaptureMetrics, String, Vec<Option<Thumbnail>>) {
    let dir = capture_dir();
    match super::report::write_capture_bundle(
        &dir,
        intermediate,
        final_output,
        final_sdr_white,
        diagnostics,
        false,
    ) {
        Ok((metrics, maps)) => (
            metrics,
            format!("Files written to {}", dir.display()),
            vec![
                Thumbnail::new("Headroom above SDR white", thumbnail_of(&maps.headroom)),
                Thumbnail::new("Pixels above SDR white", thumbnail_of(&maps.clip_mask)),
            ],
        ),
        Err(error) => (
            measure_only(intermediate, final_output, final_sdr_white),
            format!("Could not write files: {error}"),
            Vec::new(),
        ),
    }
}

#[cfg(target_arch = "wasm32")]
fn write_bundle(
    intermediate: &HdrRgbaImage,
    final_output: &DebugCaptureArtifact,
    final_sdr_white: f32,
    _diagnostics: &str,
) -> (CaptureMetrics, String, Vec<Option<Thumbnail>>) {
    (
        measure_only(intermediate, final_output, final_sdr_white),
        "Browsers cannot write capture files.".to_string(),
        Vec::new(),
    )
}

fn measure_only(
    intermediate: &HdrRgbaImage,
    final_output: &DebugCaptureArtifact,
    final_sdr_white: f32,
) -> CaptureMetrics {
    let (final_is_hdr, final_metrics) = match final_output {
        DebugCaptureArtifact::HdrLinearRgbaF32(image) => {
            (true, LightMetrics::relative_to(image, final_sdr_white))
        }
        DebugCaptureArtifact::SdrRgba8(_) => (false, LightMetrics::SDR),
    };
    CaptureMetrics {
        width: intermediate.width(),
        height: intermediate.height(),
        intermediate: LightMetrics::of(intermediate),
        final_is_hdr,
        final_sdr_white,
        final_output: final_metrics,
    }
}

impl Widget for CapturePanel {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.state.wake.get().is_none() {
            self.state.wake.set(Some(ctx.register_async_wakeup()));
        }
        if let Event::Wake(WakeEvent::Async { token, .. }) = event
            && self.state.wake.get() == Some(*token)
        {
            self.state.finish_capture(ctx.window_id());
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            720.0
        };
        self.state.panel.set(Some(ctx.widget_id()));
        let buttons = self.buttons.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(width, f32::INFINITY)),
        );
        let style = self.text_style();
        self.lines = self
            .state
            .status_lines()
            .into_iter()
            .map(|line| Paragraph::new(ctx, line, &style, TextAlign::Start, width))
            .collect();
        let text_height = self
            .lines
            .iter()
            .map(|line| line_height(line, &style) + LINE_GAP)
            .sum::<f32>();
        let thumbnail_height = self
            .thumbnail_sizes()
            .iter()
            .map(|size| size.height)
            .fold(0.0, f32::max);
        let thumbnails = if thumbnail_height > 0.0 {
            BLOCK_GAP + thumbnail_height + LINE_GAP + self.caption_style().line_height
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            width,
            buttons.height + BLOCK_GAP + text_height + thumbnails,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let size = self.buttons.child().measured_size();
        self.buttons.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y(),
                size.width.min(bounds.width()),
                size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.buttons.paint(ctx);
        let bounds = ctx.bounds();
        let style = self.text_style();
        let mut y = bounds.y() + self.buttons.child().measured_size().height + BLOCK_GAP;
        for line in &self.lines {
            let height = line_height(line, &style);
            line.paint_with_color(
                ctx,
                Rect::new(bounds.x(), y, bounds.width(), height),
                VerticalAlign::Top,
                style.color,
            );
            y += height + LINE_GAP;
        }

        let caption = self.caption_style();
        // Each thumbnail gets the same slot, so captions never overlap.
        let slot_width = THUMBNAIL_SIZE.0 as f32;
        let mut x = bounds.x();
        let top = y + BLOCK_GAP - LINE_GAP;
        let capture = self.state.capture.borrow();
        let thumbnails = match &*capture {
            CaptureState::Done { thumbnails, .. } => thumbnails.as_slice(),
            _ => &[],
        };
        for (slot, thumbnail) in thumbnails.iter().enumerate() {
            let rect = Rect::new(x, top, thumbnail.size.width, thumbnail.size.height);
            let handle = ctx.widget_image_handle(slot as u64);
            ctx.register_image(handle, thumbnail.image.clone());
            ctx.draw_image(rect, handle);
            let theme = (self.theme_reader)();
            ctx.stroke_rect(rect, theme.palette.border, StrokeStyle::new(1.0));
            paint_text_line(
                ctx,
                Rect::new(x, rect.max_y() + LINE_GAP, slot_width, caption.line_height),
                thumbnail.caption,
                &caption,
                TextAlign::Start,
            );
            x += slot_width + BLOCK_GAP;
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(CAPTURE_STATUS_NAME.to_string());
        node.description = Some(
            self.lines
                .iter()
                .map(Paragraph::text)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        ctx.push(node);
        self.buttons.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.buttons.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.buttons.visit_children_mut(visitor);
    }
}
