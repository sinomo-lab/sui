use crate::EmbeddedSvgImageResource;
use std::sync::Arc;
use sui_core::{Error, FontHandle, ImageHandle, Result};
use sui_scene::{ImageRegistry, RegisteredImage};
use sui_text::{FontRegistry, RegisteredFont};

/// Authoritative resource registration and handle allocation, transferred intact
/// from an application builder to its runtime.
pub(crate) struct ResourceStore {
    next_font_id: u64,
    next_image_id: u64,
    fonts: Arc<FontRegistry>,
    images: Arc<ImageRegistry>,
}

impl Default for ResourceStore {
    fn default() -> Self {
        Self {
            next_font_id: 1,
            next_image_id: 1,
            fonts: Arc::new(FontRegistry::new()),
            images: Arc::new(ImageRegistry::new()),
        }
    }
}

impl ResourceStore {
    pub(crate) fn fonts(&self) -> &Arc<FontRegistry> {
        &self.fonts
    }
    pub(crate) fn images(&self) -> &Arc<ImageRegistry> {
        &self.images
    }

    pub(crate) fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        if self.fonts.get(handle).is_some() {
            return Err(Error::new(format!(
                "font handle {} is already registered",
                handle.get()
            )));
        }

        Arc::make_mut(&mut self.fonts).insert(handle, font);
        self.next_font_id = self.next_font_id.max(handle.get().saturating_add(1));
        Ok(())
    }

    pub(crate) fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        let handle = FontHandle::new(self.next_font_id.max(1));
        self.register_font(handle, RegisteredFont::from_bytes(data))?;
        Ok(handle)
    }

    pub(crate) fn register_image(
        &mut self,
        handle: ImageHandle,
        image: RegisteredImage,
    ) -> Result<()> {
        if self.images.get(handle).is_some() {
            return Err(Error::new(format!(
                "image handle {} is already registered",
                handle.get()
            )));
        }

        Arc::make_mut(&mut self.images).insert(handle, image);
        self.next_image_id = self.next_image_id.max(handle.get().saturating_add(1));
        Ok(())
    }

    pub(crate) fn register_svg_image_with_handle(
        &mut self,
        handle: ImageHandle,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.register_image(handle, RegisteredImage::from_svg(data)?)
    }

    pub(crate) fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.register_image(handle, RegisteredImage::from_rgba8(width, height, data)?)?;
        Ok(handle)
    }

    pub(crate) fn register_svg_image(&mut self, data: impl AsRef<[u8]>) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.register_image(handle, RegisteredImage::from_svg(data)?)?;
        Ok(handle)
    }

    pub(crate) fn register_svg_image_at_size_with_handle(
        &mut self,
        handle: ImageHandle,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.register_image(
            handle,
            RegisteredImage::from_svg_at_size(width, height, data)?,
        )
    }

    pub(crate) fn register_svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.register_image(
            handle,
            RegisteredImage::from_svg_at_size(width, height, data)?,
        )?;
        Ok(handle)
    }

    pub(crate) fn register_embedded_svg_image(
        &mut self,
        resource: EmbeddedSvgImageResource,
    ) -> Result<()> {
        self.register_image(resource.handle(), resource.registered_image()?)
    }

    pub(crate) fn register_embedded_svg_images(
        &mut self,
        resources: impl IntoIterator<Item = EmbeddedSvgImageResource>,
    ) -> Result<()> {
        for resource in resources {
            self.register_embedded_svg_image(resource)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Application, Runtime};

    #[test]
    fn duplicate_images_leave_builder_and_runtime_resources_unchanged() {
        let id = ImageHandle::new(41);
        let original = RegisteredImage::from_rgba8(1, 1, vec![255, 0, 0, 255]).unwrap();
        let replacement = RegisteredImage::from_rgba8(1, 1, vec![0, 0, 255, 255]).unwrap();
        let mut app = Application::new();
        app.register_image(id, original.clone()).unwrap();
        assert!(app.register_image(id, replacement.clone()).is_err());
        let mut runtime = app.build().unwrap();
        assert_eq!(runtime.image_registry().get(id), Some(&original));
        let snapshot = Arc::clone(runtime.image_registry());
        assert!(runtime.register_image(id, replacement).is_err());
        assert!(Arc::ptr_eq(&snapshot, runtime.image_registry()));
        assert_eq!(runtime.image_registry().get(id), Some(&original));
        assert_eq!(
            runtime.register_rgba_image(1, 1, vec![0; 4]).unwrap().get(),
            42
        );
    }

    #[test]
    fn invalid_auto_registered_images_do_not_consume_handles() {
        let mut runtime = Runtime::new();
        assert!(runtime.register_rgba_image(2, 2, vec![0; 4]).is_err());
        assert!(runtime.image_registry().is_empty());
        assert_eq!(
            runtime.register_rgba_image(1, 1, vec![0; 4]).unwrap().get(),
            1
        );
    }

    #[test]
    fn duplicate_fonts_preserve_registered_bytes_and_handle_allocation() {
        let id = FontHandle::new(17);
        let mut app = Application::new();
        app.register_font(id, RegisteredFont::from_bytes(vec![1, 2, 3]))
            .unwrap();
        assert!(
            app.register_font(id, RegisteredFont::from_bytes(vec![4, 5]))
                .is_err()
        );
        let mut runtime = app.build().unwrap();
        assert!(
            runtime
                .register_font(id, RegisteredFont::from_bytes(vec![6, 7]))
                .is_err()
        );
        assert_eq!(runtime.font_registry().get(id).unwrap().bytes(), &[1, 2, 3]);
        assert_eq!(runtime.register_font_bytes(vec![8, 9]).unwrap().get(), 18);
    }
}
