//! Captures the window and shows two specimens' corners several times
//! bigger, pixel for pixel, so rendering differences too small to see at
//! 1× become visible.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use sui::prelude::*;
use sui::{
    AsyncWakeToken, DebugCaptureArtifact, DebugCaptureEncoding, DebugCaptureRequest,
    DebugCaptureStage, DebugCaptureTicket, DebugSdrVisualization, ImageSampling, ImageSource,
    InvalidationKind, InvalidationRequest, InvalidationTarget, Rect, RegisteredImage, RgbaImage,
    SemanticsNode, SemanticsRole, WakeEvent, WidgetId, WidgetPodMutVisitor, WidgetPodVisitor,
    paint_text_line, request_window_debug_capture, take_window_debug_capture,
};

use super::samples::ZoomRegion;
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style};

pub(crate) const MAGNIFY_LABEL: &str = "Magnify";
pub(crate) const MAGNIFIER_NAME: &str = "Magnifier";

const GAP: f32 = 12.0;
const CAPTION_GAP: f32 = 4.0;
const MAX_ZOOM: u32 = 8;

/// One specimen to magnify: where it is, and what to call it.
pub(crate) struct Source {
    pub(crate) region: ZoomRegion,
    pub(crate) caption: Box<dyn Fn() -> String>,
}

/// A captured corner of a specimen.
struct Crop {
    image: RegisteredImage,
    /// In physical pixels.
    size: (u32, u32),
    caption: String,
}

enum State {
    Idle,
    Pending(DebugCaptureTicket),
    Done(Vec<Crop>),
    Failed(String),
}

struct Shared {
    sources: Vec<Source>,
    state: RefCell<State>,
    wake: Cell<Option<AsyncWakeToken>>,
    widget: Cell<Option<WidgetId>>,
}

impl Shared {
    fn start(&self, ctx: &mut EventCtx) {
        let Some(wake) = self.wake.get() else {
            *self.state.borrow_mut() =
                State::Failed("The magnifier could not be woken for the capture.".to_string());
            self.refresh(ctx);
            return;
        };
        let ticket = request_window_debug_capture(
            ctx.window_id(),
            DebugCaptureRequest {
                stage: DebugCaptureStage::FinalComposed,
                encoding: DebugCaptureEncoding::Png,
                sdr_visualization: DebugSdrVisualization::ToneMappedColor,
            },
            Some(wake),
        );
        *self.state.borrow_mut() = State::Pending(ticket);
        // Captures follow the next redraw.
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            InvalidationKind::Paint,
        ));
        self.refresh(ctx);
    }

    fn finish(&self) {
        let ticket = match &*self.state.borrow() {
            State::Pending(ticket) => *ticket,
            _ => return,
        };
        let finished = match take_window_debug_capture(ticket) {
            Some(Ok(DebugCaptureArtifact::SdrRgba8(image))) => self.crop(&image),
            Some(Ok(DebugCaptureArtifact::HdrLinearRgbaF32(_))) => {
                State::Failed("The capture came back as floating point, not 8-bit.".to_string())
            }
            Some(Err(error)) => State::Failed(format!("The capture failed: {error}")),
            None => return,
        };
        *self.state.borrow_mut() = finished;
    }

    fn crop(&self, image: &RgbaImage) -> State {
        let mut crops = Vec::new();
        for source in &self.sources {
            let Some(region) = source.region.get() else {
                continue;
            };
            let Some((size, pixels)) = crop_pixels(image, region) else {
                continue;
            };
            let Ok(image) = RegisteredImage::from_rgba8(size.0, size.1, pixels) else {
                continue;
            };
            crops.push(Crop {
                image,
                size,
                caption: (source.caption)(),
            });
        }
        if crops.is_empty() {
            State::Failed(
                "The samples were not on screen when the window was captured.".to_string(),
            )
        } else {
            State::Done(crops)
        }
    }

    fn refresh(&self, ctx: &mut EventCtx) {
        let Some(widget) = self.widget.get() else {
            return;
        };
        for kind in [
            InvalidationKind::Measure,
            InvalidationKind::Paint,
            InvalidationKind::Semantics,
        ] {
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Widget(widget),
                kind,
            ));
        }
    }

    fn status(&self) -> String {
        match &*self.state.borrow() {
            State::Idle => format!(
                "{MAGNIFY_LABEL} captures the window and shows the marked corner of each sample above, pixel for pixel."
            ),
            State::Pending(_) => "Capturing the next frame…".to_string(),
            State::Done(crops) => format!(
                "Captured {} samples. Each screen pixel is drawn as a square; compare edge softness, stem weight, and color fringes.",
                crops.len()
            ),
            State::Failed(message) => message.clone(),
        }
    }
}

/// The pixels of `region`, clamped to `image`, as tightly packed RGBA8.
fn crop_pixels(image: &RgbaImage, region: Rect) -> Option<((u32, u32), Vec<u8>)> {
    let x0 = region.x().max(0.0) as u32;
    let y0 = region.y().max(0.0) as u32;
    let x1 = (region.max_x().max(0.0) as u32).min(image.width());
    let y1 = (region.max_y().max(0.0) as u32).min(image.height());
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (width, height) = (x1 - x0, y1 - y0);
    let stride = image.width() as usize * 4;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in y0..y1 {
        let start = row as usize * stride + x0 as usize * 4;
        pixels.extend_from_slice(&image.pixels()[start..start + width as usize * 4]);
    }
    Some(((width, height), pixels))
}

/// A button that captures the window, and the magnified corners of the
/// specimens it was given.
pub(crate) struct Magnifier {
    shared: Rc<Shared>,
    theme_reader: DevThemeReader,
    button: SingleChild,
    status: Paragraph,
    /// Where each crop is drawn, as of the last layout, and how big.
    layout: Vec<(Rect, Rect)>,
}

impl Magnifier {
    pub(crate) fn new(theme_reader: &DevThemeReader, sources: Vec<Source>) -> Self {
        let shared = Rc::new(Shared {
            sources,
            state: RefCell::new(State::Idle),
            wake: Cell::new(None),
            widget: Cell::new(None),
        });
        let start = Rc::clone(&shared);
        Self {
            button: SingleChild::new(
                Button::new(MAGNIFY_LABEL)
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .on_press_with_ctx(move |ctx| start.start(ctx)),
            ),
            shared,
            theme_reader: Rc::clone(theme_reader),
            status: Paragraph::default(),
            layout: Vec::new(),
        }
    }

    fn text_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Body, theme.palette.text_muted)
    }

    fn caption_style(&self) -> TextStyle {
        let theme = (self.theme_reader)();
        demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text)
    }
}

impl Widget for Magnifier {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.shared.wake.get().is_none() {
            self.shared.wake.set(Some(ctx.register_async_wakeup()));
        }
        if let Event::Wake(WakeEvent::Async { token, .. }) = event
            && self.shared.wake.get() == Some(*token)
        {
            self.shared.finish();
            self.shared.refresh(ctx);
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.shared.widget.set(Some(ctx.widget_id()));
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            720.0
        };
        let button = self.button.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(width, f32::INFINITY)),
        );
        let style = self.text_style();
        let status_width = (width - button.width - GAP).max(1.0);
        self.status = Paragraph::new(
            ctx,
            self.shared.status(),
            &style,
            TextAlign::Start,
            status_width,
        );
        let header = button.height.max(self.status.size().height);

        // Show each crop as big as a whole-pixel zoom lets it fit its share
        // of the width.
        self.layout.clear();
        let scale = ctx.dpi().scale_factor.max(0.01);
        let caption = self.caption_style().line_height;
        let mut crops_height: f32 = 0.0;
        if let State::Done(crops) = &*self.shared.state.borrow() {
            let share =
                (width - GAP * (crops.len().saturating_sub(1)) as f32) / crops.len().max(1) as f32;
            let mut x = 0.0;
            for crop in crops {
                let zoom = ((share * scale) / crop.size.0.max(1) as f32)
                    .floor()
                    .clamp(1.0, MAX_ZOOM as f32);
                let size = Size::new(
                    crop.size.0 as f32 * zoom / scale,
                    crop.size.1 as f32 * zoom / scale,
                );
                let caption_rect = Rect::new(x, 0.0, share, caption);
                let image_rect = Rect::new(x, caption + CAPTION_GAP, size.width, size.height);
                crops_height = crops_height.max(image_rect.max_y());
                self.layout.push((caption_rect, image_rect));
                x += share + GAP;
            }
        }
        let height = header
            + if crops_height > 0.0 {
                GAP + crops_height
            } else {
                0.0
            };
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let button = self.button.child().measured_size();
        self.button.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), button.width, button.height),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        self.button.paint(ctx);
        let button = self.button.child().measured_size();
        let header = button.height.max(self.status.size().height);
        self.status.paint(
            ctx,
            Rect::new(
                bounds.x() + button.width + GAP,
                bounds.y(),
                (bounds.width() - button.width - GAP).max(0.0),
                header,
            ),
            VerticalAlign::Center,
        );

        let State::Done(crops) = &*self.shared.state.borrow() else {
            return;
        };
        let origin = Point::new(bounds.x(), bounds.y() + header + GAP);
        let caption_style = self.caption_style();
        for (slot, (crop, (caption, image))) in crops.iter().zip(&self.layout).enumerate() {
            paint_text_line(
                ctx,
                caption.translate(origin.to_vector()),
                &crop.caption,
                &caption_style,
                TextAlign::Start,
            );
            let handle = ctx.widget_image_handle(slot as u64);
            ctx.register_image(handle, crop.image.clone());
            ctx.draw_image_source(
                image.translate(origin.to_vector()),
                ImageSource::new(handle).with_sampling(ImageSampling::Nearest),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(MAGNIFIER_NAME.to_string());
        let mut description = self.shared.status();
        if let State::Done(crops) = &*self.shared.state.borrow() {
            for crop in crops {
                description.push_str(&format!(
                    " {} ({} × {} pixels).",
                    crop.caption, crop.size.0, crop.size.1
                ));
            }
        }
        node.description = Some(description);
        ctx.push(node);
        self.button.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.button.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.button.visit_children_mut(visitor);
    }
}
