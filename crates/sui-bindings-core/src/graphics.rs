use crate::application::normalized_option_name;
use crate::widget_descriptor::BindingWidget;
use sui::BrushPreviewShape;
use sui::BrushPreviewSpec;
use sui::CanvasShape;
use sui::CanvasStroke;
use sui::CanvasViewport;
use sui::Color;
use sui::ImageFit;
use sui::Path;
use sui::PixelCanvasBlendMode;
use sui::PixelCanvasBrushShape;
use sui::PixelCanvasExportSnapshot;
use sui::PixelCanvasState;
use sui::PixelCanvasTool;
use sui::Point;
use sui::Rect;
use sui::Vector;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingImageFit {
    Fill,
    #[default]
    Contain,
    Cover,
    None,
}

impl From<BindingImageFit> for ImageFit {
    fn from(value: BindingImageFit) -> Self {
        match value {
            BindingImageFit::Fill => Self::Fill,
            BindingImageFit::Contain => Self::Contain,
            BindingImageFit::Cover => Self::Cover,
            BindingImageFit::None => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingScrollAxes {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

/// Portable brush metadata used by [`BindingWidget::brush_preview`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingBrushPreviewSpec {
    pub(crate) color: Color,
    pub(crate) size: f32,
    pub(crate) opacity: f32,
    pub(crate) shape: BrushPreviewShape,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingCanvasViewport {
    pub pan: Vector,
    pub zoom: f32,
    pub rotation: f32,
}

impl BindingCanvasViewport {
    pub fn new(pan: Vector, zoom: f32, rotation: f32) -> Self {
        Self {
            pan,
            zoom: zoom.max(0.01),
            rotation,
        }
    }

    pub(crate) fn into_sui(self) -> CanvasViewport {
        CanvasViewport::new()
            .pan(self.pan)
            .zoom(self.zoom)
            .rotation(self.rotation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingCanvasStroke {
    pub color: Color,
    pub width: f32,
}

impl BindingCanvasStroke {
    pub fn new(color: Color, width: f32) -> Self {
        Self {
            color,
            width: width.max(0.1),
        }
    }

    pub(crate) fn into_sui(self) -> CanvasStroke {
        CanvasStroke::new(self.color, self.width)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingCanvasShape {
    pub(crate) inner: CanvasShape,
}

impl BindingCanvasShape {
    pub fn path(path: Path, fill: Option<Color>, stroke: Option<BindingCanvasStroke>) -> Self {
        Self {
            inner: CanvasShape::Path {
                path,
                fill,
                stroke: stroke.map(BindingCanvasStroke::into_sui),
            },
        }
    }

    pub fn rect(rect: Rect, fill: Option<Color>, stroke: Option<BindingCanvasStroke>) -> Self {
        Self {
            inner: CanvasShape::rect(rect, fill, stroke.map(BindingCanvasStroke::into_sui)),
        }
    }

    pub fn circle(
        center: Point,
        radius: f32,
        fill: Option<Color>,
        stroke: Option<BindingCanvasStroke>,
    ) -> Self {
        Self {
            inner: CanvasShape::circle(
                center,
                radius,
                fill,
                stroke.map(BindingCanvasStroke::into_sui),
            ),
        }
    }

    pub fn polyline(points: &[Point], stroke: BindingCanvasStroke) -> Result<Self, String> {
        CanvasShape::polyline(points, stroke.into_sui())
            .map(|inner| Self { inner })
            .ok_or_else(|| "canvas polyline requires at least two distinct points".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct BindingPixelCanvasExport {
    pub revision: u64,
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub rgba8: Vec<u8>,
}

impl From<PixelCanvasExportSnapshot> for BindingPixelCanvasExport {
    fn from(value: PixelCanvasExportSnapshot) -> Self {
        Self {
            revision: value.revision(),
            name: value.name().to_owned(),
            width: value.width(),
            height: value.height(),
            rgba8: value.rgba8().to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingPixelCanvasState {
    pub(crate) inner: PixelCanvasState,
}

impl BindingPixelCanvasState {
    pub fn new() -> Self {
        Self {
            inner: PixelCanvasState::new(),
        }
    }

    pub fn tool(&self) -> &'static str {
        match self.inner.tool() {
            PixelCanvasTool::Brush => "brush",
            PixelCanvasTool::Eraser => "eraser",
            PixelCanvasTool::Fill => "fill",
            PixelCanvasTool::Pan => "pan",
        }
    }

    pub fn set_tool(&self, value: &str) -> Result<(), String> {
        self.inner
            .set_tool(match normalized_option_name(value).as_str() {
                "brush" | "paint" => PixelCanvasTool::Brush,
                "eraser" | "erase" => PixelCanvasTool::Eraser,
                "fill" | "bucket" => PixelCanvasTool::Fill,
                "pan" | "hand" => PixelCanvasTool::Pan,
                _ => {
                    return Err(format!(
                        "pixel canvas tool must be brush, eraser, fill, or pan; got '{value}'"
                    ));
                }
            });
        Ok(())
    }

    pub fn brush_color(&self) -> Color {
        self.inner.brush_color()
    }

    pub fn set_brush_color(&self, color: Color) {
        self.inner.set_brush_color(color);
    }

    pub fn brush_size(&self) -> f32 {
        self.inner.brush_size()
    }

    pub fn set_brush_size(&self, size: f32) {
        self.inner.set_brush_size(size);
    }

    pub fn brush_opacity(&self) -> f32 {
        self.inner.brush_opacity()
    }

    pub fn set_brush_opacity(&self, opacity: f32) {
        self.inner.set_brush_opacity(opacity);
    }

    pub fn brush_shape(&self) -> &'static str {
        match self.inner.brush_shape() {
            PixelCanvasBrushShape::Square => "square",
            PixelCanvasBrushShape::Round => "round",
        }
    }

    pub fn set_brush_shape(&self, value: &str) -> Result<(), String> {
        self.inner
            .set_brush_shape(match normalized_option_name(value).as_str() {
                "square" => PixelCanvasBrushShape::Square,
                "round" | "circle" => PixelCanvasBrushShape::Round,
                _ => {
                    return Err(format!(
                        "pixel brush shape must be square or round; got '{value}'"
                    ));
                }
            });
        Ok(())
    }

    pub fn blend_mode(&self) -> &'static str {
        binding_pixel_blend_mode_name(self.inner.blend_mode())
    }

    pub fn set_blend_mode(&self, value: &str) -> Result<(), String> {
        self.inner.set_blend_mode(binding_pixel_blend_mode(value)?);
        Ok(())
    }

    pub fn editable(&self) -> bool {
        self.inner.is_editable()
    }

    pub fn set_editable(&self, editable: bool) -> bool {
        self.inner.set_editable(editable)
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub fn can_clear(&self) -> bool {
        self.inner.can_clear()
    }

    pub fn request_undo(&self) {
        self.inner.request_undo();
    }

    pub fn request_redo(&self) {
        self.inner.request_redo();
    }

    pub fn request_clear(&self) {
        self.inner.request_clear();
    }

    pub fn request_fit_view(&self) {
        self.inner.request_fit_view();
    }

    pub fn request_actual_size(&self) {
        self.inner.request_actual_size_view();
    }

    pub fn request_zoom_in(&self) {
        self.inner.request_zoom_in();
    }

    pub fn request_zoom_out(&self) {
        self.inner.request_zoom_out();
    }

    pub fn request_export(&self) {
        self.inner.request_export_snapshot();
    }

    pub fn latest_export(&self) -> Option<BindingPixelCanvasExport> {
        self.inner.latest_export_snapshot().map(Into::into)
    }
}

impl Default for BindingPixelCanvasState {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn binding_pixel_blend_mode(value: &str) -> Result<PixelCanvasBlendMode, String> {
    match normalized_option_name(value).as_str() {
        "normal" => Ok(PixelCanvasBlendMode::Normal),
        "multiply" => Ok(PixelCanvasBlendMode::Multiply),
        "screen" => Ok(PixelCanvasBlendMode::Screen),
        "overlay" => Ok(PixelCanvasBlendMode::Overlay),
        _ => Err(format!(
            "pixel blend mode must be normal, multiply, screen, or overlay; got '{value}'"
        )),
    }
}

pub(crate) fn binding_pixel_blend_mode_name(value: PixelCanvasBlendMode) -> &'static str {
    match value {
        PixelCanvasBlendMode::Normal => "normal",
        PixelCanvasBlendMode::Multiply => "multiply",
        PixelCanvasBlendMode::Screen => "screen",
        PixelCanvasBlendMode::Overlay => "overlay",
    }
}

impl BindingBrushPreviewSpec {
    pub fn new(color: Color, size: f32, opacity: f32, shape: BrushPreviewShape) -> Self {
        Self {
            color,
            size,
            opacity,
            shape,
        }
    }

    pub(crate) fn into_sui(self) -> BrushPreviewSpec {
        BrushPreviewSpec::new(self.color, self.size, self.opacity, self.shape)
    }
}

#[derive(Debug, Clone)]
pub struct BindingFloatingStackWindow {
    pub(crate) bounds: Rect,
    pub(crate) child: BindingWidget,
}

impl BindingFloatingStackWindow {
    pub fn new(bounds: Rect, child: BindingWidget) -> Self {
        Self { bounds, child }
    }
}
