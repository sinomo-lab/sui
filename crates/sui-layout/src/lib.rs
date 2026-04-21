#![forbid(unsafe_code)]

use std::sync::Arc;

use sui_core::{DpiInfo, ImageHandle, Point, Size};
use sui_scene::ImageRegistry;
use sui_text::{
    FontRegistry, PersistentTextLayout, TextDocument, TextLayout, TextLayoutHandle,
    TextLayoutRequest, TextMeasurement, TextStyle, TextSystem,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Padding {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Padding {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub fn inset(self, size: Size) -> Size {
        Size::new(
            (size.width - (self.left + self.right)).max(0.0),
            (size.height - (self.top + self.bottom)).max(0.0),
        )
    }

    pub const fn offset(self) -> Point {
        Point::new(self.left, self.top)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min: Size,
    pub max: Size,
}

impl Constraints {
    pub const UNBOUNDED: Self = Self {
        min: Size::ZERO,
        max: Size::new(f32::INFINITY, f32::INFINITY),
    };

    pub const fn new(min: Size, max: Size) -> Self {
        Self { min, max }
    }

    pub const fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    pub fn loosen(self) -> Self {
        Self {
            min: Size::ZERO,
            max: self.max,
        }
    }

    pub fn clamp(self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min.width, self.max.width),
            size.height.clamp(self.min.height, self.max.height),
        )
    }
}

#[derive(Debug, Clone)]
pub struct LayoutContext {
    dpi_info: DpiInfo,
    text_system: Arc<TextSystem>,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
}

impl LayoutContext {
    pub fn new(
        dpi_info: DpiInfo,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Self {
        Self {
            dpi_info,
            text_system,
            font_registry,
            image_registry,
        }
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn measure_text(
        &self,
        text: impl Into<String>,
        style: TextStyle,
    ) -> sui_core::Result<TextMeasurement> {
        self.text_system
            .measure_text(text, style, self.font_registry.as_ref())
    }

    pub fn measure_document(&self, document: TextDocument) -> sui_core::Result<TextMeasurement> {
        self.text_system
            .measure_document(document, self.font_registry.as_ref())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
    ) -> sui_core::Result<TextLayout> {
        self.text_system
            .shape_text(text, box_size, style, self.font_registry.as_ref())
    }

    pub fn shape_text_persistent(
        &self,
        handle: Option<TextLayoutHandle>,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
    ) -> sui_core::Result<PersistentTextLayout> {
        self.text_system.shape_text_persistent(
            handle,
            text,
            box_size,
            style,
            self.font_registry.as_ref(),
        )
    }

    pub fn layout_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout> {
        self.text_system
            .layout_document(request, self.font_registry.as_ref())
    }

    pub fn layout_document_persistent(
        &self,
        handle: Option<TextLayoutHandle>,
        request: TextLayoutRequest,
    ) -> sui_core::Result<PersistentTextLayout> {
        self.text_system
            .layout_document_persistent(handle, request, self.font_registry.as_ref())
    }

    pub fn image_size(&self, image: ImageHandle) -> Option<Size> {
        self.image_registry
            .get(image)
            .map(|image| Size::new(image.width() as f32, image.height() as f32))
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::UNBOUNDED
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::LayoutContext;
    use sui_core::{Color, DpiInfo, ImageHandle, Size};
    use sui_scene::{ImageRegistry, RegisteredImage};
    use sui_text::{FontRegistry, TextStyle, TextSystem};

    #[test]
    fn layout_context_measures_text_and_images_without_runtime_widget_state() {
        let mut images = ImageRegistry::new();
        images.insert(
            ImageHandle::new(7),
            RegisteredImage::from_rgba8(4, 2, vec![255; 4 * 2 * 4]).unwrap(),
        );

        let layout = LayoutContext::new(
            DpiInfo::new(
                2.0,
                Some(192.0),
                Size::new(320.0, 180.0),
                Size::new(640.0, 360.0),
            ),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(images),
        );

        let measurement = layout
            .measure_text("hello", TextStyle::new(Color::WHITE))
            .unwrap();

        assert!(measurement.width > 0.0);
        assert_eq!(layout.dpi().effective_dpi(), 192.0);
        assert_eq!(
            layout.image_size(ImageHandle::new(7)),
            Some(Size::new(4.0, 2.0))
        );
    }
}
