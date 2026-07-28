use std::sync::Arc;

use sui_core::{Error, FontHandle, ImageHandle, Result, WindowId};
use sui_scene::{ImageRegistry, RegisteredImage};
use sui_text::{FontRegistry, RegisteredFont};

use crate::{
    Runtime, WindowState,
    command::{CommandController, CommandCtx, CommandKey, CommandListeners, CommandSender},
    logo::DEFAULT_SUI_LOGO_SVG,
    widget::{Widget, WidgetPod},
};

pub struct WindowBuilder {
    title: String,
    icon: Option<WindowIcon>,
    root: Option<WidgetPod>,
    command_listeners: CommandListeners,
}

impl WindowBuilder {
    pub fn new() -> Self {
        Self {
            title: "SUI Window".to_string(),
            icon: Some(WindowIcon::sui()),
            root: None,
            command_listeners: CommandListeners::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn icon(mut self, icon: WindowIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn icon_svg(mut self, svg: impl Into<Vec<u8>>) -> Self {
        self.icon = Some(WindowIcon::from_svg(svg));
        self
    }

    pub fn without_icon(mut self) -> Self {
        self.icon = None;
        self
    }

    pub fn root<W>(mut self, root: W) -> Self
    where
        W: Widget + 'static,
    {
        self.root = Some(WidgetPod::new(root));
        self
    }

    /// Attach a non-widget controller whose lifetime is exactly this window's
    /// lifetime.
    pub fn controller(mut self, controller: impl CommandController + 'static) -> Self {
        self.command_listeners.push_controller(controller);
        self
    }

    /// Subscribe a window-scoped handler to a typed command.
    pub fn on_command<T, F>(mut self, key: CommandKey<T>, handler: F) -> Self
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        self.command_listeners.push_subscription(key, handler);
        self
    }

    pub(crate) fn build(
        self,
        window_id: WindowId,
        command_sender: CommandSender,
    ) -> Result<WindowState> {
        let root = self
            .root
            .ok_or_else(|| Error::new("window root widget must be set before building"))?;

        Ok(WindowState::new(
            window_id,
            self.title,
            self.icon,
            root,
            self.command_listeners,
            command_sender,
        ))
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowIcon {
    Svg {
        data: Arc<[u8]>,
    },
    Rgba8 {
        width: u32,
        height: u32,
        data: Arc<[u8]>,
    },
}

impl WindowIcon {
    pub fn sui() -> Self {
        Self::from_svg(DEFAULT_SUI_LOGO_SVG)
    }

    pub fn from_svg(svg: impl Into<Vec<u8>>) -> Self {
        Self::Svg {
            data: Arc::from(svg.into()),
        }
    }

    pub fn from_rgba8(width: u32, height: u32, data: impl Into<Vec<u8>>) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::new("window icon dimensions must be non-zero"));
        }

        let data = data.into();
        let expected_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| Error::new("window icon dimensions overflow RGBA buffer length"))?;

        if data.len() != expected_len {
            return Err(Error::new(format!(
                "window icon RGBA buffer length mismatch: expected {expected_len}, got {}",
                data.len()
            )));
        }

        Ok(Self::Rgba8 {
            width,
            height,
            data: Arc::from(data),
        })
    }

    pub fn as_svg(&self) -> Option<&[u8]> {
        match self {
            Self::Svg { data } => Some(data),
            Self::Rgba8 { .. } => None,
        }
    }

    pub fn as_rgba8(&self) -> Option<(u32, u32, &[u8])> {
        match self {
            Self::Svg { .. } => None,
            Self::Rgba8 {
                width,
                height,
                data,
            } => Some((*width, *height, data)),
        }
    }
}

impl Default for WindowIcon {
    fn default() -> Self {
        Self::sui()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedSvgImageResource {
    handle: ImageHandle,
    data: &'static [u8],
    size: Option<(u32, u32)>,
}

impl EmbeddedSvgImageResource {
    pub const fn new(handle: ImageHandle, data: &'static [u8]) -> Self {
        Self {
            handle,
            data,
            size: None,
        }
    }

    pub const fn at_size(
        handle: ImageHandle,
        width: u32,
        height: u32,
        data: &'static [u8],
    ) -> Self {
        Self {
            handle,
            data,
            size: Some((width, height)),
        }
    }

    pub const fn handle(self) -> ImageHandle {
        self.handle
    }

    pub const fn data(self) -> &'static [u8] {
        self.data
    }

    pub const fn size(self) -> Option<(u32, u32)> {
        self.size
    }

    pub fn registered_image(self) -> Result<RegisteredImage> {
        match self.size {
            Some((width, height)) => RegisteredImage::from_svg_at_size(width, height, self.data),
            None => RegisteredImage::from_svg(self.data),
        }
    }
}

pub struct Application {
    windows: Vec<WindowBuilder>,
    next_font_id: u64,
    next_image_id: u64,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
    command_listeners: CommandListeners,
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.windows.push(window);
        self
    }

    /// Attach an application-scoped non-widget controller.
    pub fn controller(mut self, controller: impl CommandController + 'static) -> Self {
        self.command_listeners.push_controller(controller);
        self
    }

    /// Subscribe an application-scoped handler to a typed command.
    pub fn on_command<T, F>(mut self, key: CommandKey<T>, handler: F) -> Self
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        self.command_listeners.push_subscription(key, handler);
        self
    }

    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        if Arc::make_mut(&mut self.font_registry)
            .insert(handle, font)
            .is_some()
        {
            return Err(Error::new(format!(
                "font handle {} is already registered",
                handle.get()
            )));
        }

        self.next_font_id = self.next_font_id.max(handle.get() + 1);
        Ok(())
    }

    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        let handle = FontHandle::new(self.next_font_id.max(1));
        self.next_font_id = handle.get() + 1;
        self.register_font(handle, RegisteredFont::from_bytes(data))?;
        Ok(handle)
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        if Arc::make_mut(&mut self.image_registry)
            .insert(handle, image)
            .is_some()
        {
            return Err(Error::new(format!(
                "image handle {} is already registered",
                handle.get()
            )));
        }

        self.next_image_id = self.next_image_id.max(handle.get() + 1);
        Ok(())
    }

    pub fn register_svg_image_with_handle(
        &mut self,
        handle: ImageHandle,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.register_image(handle, RegisteredImage::from_svg(data)?)
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.next_image_id = handle.get() + 1;
        self.register_image(handle, RegisteredImage::from_rgba8(width, height, data)?)?;
        Ok(handle)
    }

    pub fn register_svg_image(&mut self, data: impl AsRef<[u8]>) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.next_image_id = handle.get() + 1;
        self.register_image(handle, RegisteredImage::from_svg(data)?)?;
        Ok(handle)
    }

    pub fn register_svg_image_at_size_with_handle(
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

    pub fn register_svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<ImageHandle> {
        let handle = ImageHandle::new(self.next_image_id.max(1));
        self.next_image_id = handle.get() + 1;
        self.register_image(
            handle,
            RegisteredImage::from_svg_at_size(width, height, data)?,
        )?;
        Ok(handle)
    }

    pub fn register_embedded_svg_image(
        &mut self,
        resource: EmbeddedSvgImageResource,
    ) -> Result<()> {
        self.register_image(resource.handle(), resource.registered_image()?)
    }

    pub fn register_embedded_svg_images(
        &mut self,
        resources: impl IntoIterator<Item = EmbeddedSvgImageResource>,
    ) -> Result<()> {
        for resource in resources {
            self.register_embedded_svg_image(resource)?;
        }
        Ok(())
    }

    pub fn build(self) -> Result<Runtime> {
        let mut runtime = Runtime::with_registries(
            self.next_font_id,
            self.font_registry,
            self.next_image_id,
            self.image_registry,
        );
        runtime.command_listeners = self.command_listeners;

        for window in self.windows {
            runtime.add_window(window)?;
        }

        Ok(runtime)
    }
}

impl Default for Application {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            next_font_id: 1,
            next_image_id: 1,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            command_listeners: CommandListeners::default(),
        }
    }
}
