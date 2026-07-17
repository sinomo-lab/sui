use crate::{
    Application, CommandController, CommandCtx, CommandKey, CommandSender, CommandTarget,
    EmbeddedSvgImageResource, FontHandle, ImageHandle, RegisteredFont, RegisteredImage, Result,
    Runtime, Widget, WindowBuilder, WindowIcon, WindowRenderOptions,
};

#[cfg(all(target_os = "android", feature = "mobile"))]
use crate::AndroidApp;

/// User-facing SUI application builder.
///
/// `App` is the recommended entrypoint for Rust applications. It keeps the
/// public surface small while still producing the same runtime used by lower
/// level integration and debug tools. The builder is intentionally owned and
/// value-oriented: construct it on any thread, register resources up front,
/// add windows, then build or run it on the UI thread.
pub struct App {
    application: Application,
}

impl Default for App {
    fn default() -> Self {
        Self {
            application: Application::new(),
        }
    }
}

impl App {
    /// Create an empty application with SUI's built-in widget resources
    /// already registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a window to the application.
    pub fn window(mut self, window: Window) -> Self {
        self.application = self.application.window(window.into_window_builder());
        self
    }

    /// Attach an application-scoped service outside the widget tree.
    pub fn controller(mut self, controller: impl CommandController + 'static) -> Self {
        self.application = self.application.controller(controller);
        self
    }

    /// Subscribe an application-scoped typed command handler.
    pub fn on_command<T, F>(mut self, key: CommandKey<T>, handler: F) -> Self
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        self.application = self.application.on_command(key, handler);
        self
    }

    /// Add a window by title and root widget.
    pub fn main_window<W>(self, title: impl Into<String>, root: W) -> Self
    where
        W: Widget + 'static,
    {
        self.window(Window::new(title).root(root))
    }

    /// Mutably access the application's resource registry.
    pub fn resources(&mut self) -> ResourceRegistry<'_> {
        ResourceRegistry {
            application: &mut self.application,
        }
    }

    /// Configure resources while preserving builder-style application setup.
    pub fn with_resources(
        mut self,
        configure: impl FnOnce(&mut ResourceRegistry<'_>) -> Result<()>,
    ) -> Result<Self> {
        configure(&mut self.resources())?;
        Ok(self)
    }

    /// Set initial render options for every window when the app starts.
    pub fn render_options(mut self, options: WindowRenderOptions) -> Self {
        self.application = self.application.with_window_render_options(options);
        self
    }

    /// Configure renderer feathering when the WGPU renderer is enabled.
    #[cfg(feature = "wgpu")]
    pub fn feathering(mut self, enabled: bool) -> Self {
        self.application = self.application.with_feathering_enabled(enabled);
        self
    }

    /// Configure renderer feather width when the WGPU renderer is enabled.
    #[cfg(feature = "wgpu")]
    pub fn feather_width(mut self, width: f32) -> Self {
        self.application = self.application.with_feather_width(width);
        self
    }

    /// Attach a shared registry for app-owned WGPU textures rendered through
    /// SUI's normal image composition path.
    #[cfg(feature = "wgpu")]
    pub fn external_texture_registry(
        mut self,
        registry: crate::WgpuExternalTextureRegistry,
    ) -> Self {
        self.application = self.application.with_external_texture_registry(registry);
        self
    }

    /// Return the configured app-owned texture registry, if one is attached.
    #[cfg(feature = "wgpu")]
    pub fn configured_external_texture_registry(
        &self,
    ) -> Option<&crate::WgpuExternalTextureRegistry> {
        self.application.external_texture_registry()
    }

    /// Build the runtime without starting a platform event loop.
    ///
    /// This is the right entrypoint for tests, headless rendering, embedding,
    /// and custom platform integrations.
    pub fn build(self) -> Result<Runtime> {
        self.application.build()
    }

    /// Run the app on the default desktop/web platform event loop.
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub fn run(self) -> Result<()> {
        self.application.run()
    }

    /// Run the app and receive a cloneable, thread-safe command handle once the
    /// event loop is ready.
    ///
    /// Background tasks normally send typed window/application commands. A bare
    /// [`UiHandle::wake`] only schedules controller wake hooks and never creates
    /// a widget event.
    #[cfg(any(feature = "desktop", feature = "web"))]
    pub fn run_with_handle(self, on_ready: impl FnOnce(UiHandle)) -> Result<()> {
        self.application
            .run_with(|commands| on_ready(UiHandle::new(commands)))
    }

    #[cfg(all(target_os = "android", feature = "mobile"))]
    pub fn run_android(self, android_app: AndroidApp) -> Result<()> {
        self.application.run_android(android_app)
    }

    #[cfg(all(target_os = "android", feature = "mobile"))]
    pub fn run_android_with_handle(
        self,
        android_app: AndroidApp,
        on_ready: impl FnOnce(UiHandle),
    ) -> Result<()> {
        self.application
            .run_android_with(android_app, |commands| on_ready(UiHandle::new(commands)))
    }

    /// Convert back to the lower-level application builder.
    ///
    /// This is intended for debug tooling and migration code. Regular demos and
    /// applications should prefer [`App`] methods directly.
    pub fn into_application(self) -> Application {
        self.application
    }
}

impl From<App> for Application {
    fn from(app: App) -> Self {
        app.into_application()
    }
}

/// User-facing window builder.
pub struct Window {
    builder: WindowBuilder,
}

impl Window {
    /// Create a window with a title and the default SUI icon.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            builder: WindowBuilder::new().title(title),
        }
    }

    /// Set the window root widget.
    pub fn root<W>(mut self, root: W) -> Self
    where
        W: Widget + 'static,
    {
        self.builder = self.builder.root(root);
        self
    }

    /// Attach a controller owned by this window rather than by its widget tree.
    pub fn controller(mut self, controller: impl CommandController + 'static) -> Self {
        self.builder = self.builder.controller(controller);
        self
    }

    /// Subscribe a lifecycle-safe handler owned by this window.
    pub fn on_command<T, F>(mut self, key: CommandKey<T>, handler: F) -> Self
    where
        T: Send + Sync + 'static,
        F: FnMut(&mut CommandCtx, &T) + 'static,
    {
        self.builder = self.builder.on_command(key, handler);
        self
    }

    /// Set a platform icon.
    pub fn icon(mut self, icon: WindowIcon) -> Self {
        self.builder = self.builder.icon(icon);
        self
    }

    /// Set an SVG platform icon.
    pub fn icon_svg(mut self, svg: impl Into<Vec<u8>>) -> Self {
        self.builder = self.builder.icon_svg(svg);
        self
    }

    /// Remove the platform icon.
    pub fn without_icon(mut self) -> Self {
        self.builder = self.builder.without_icon();
        self
    }

    pub(crate) fn into_window_builder(self) -> WindowBuilder {
        self.builder
    }
}

/// Resource registration facade for user code.
///
/// Resource handles are stable, cheap to copy, and safe to store in widget
/// state. Register resources during app construction and pass handles through
/// UI state instead of moving raw image/font blobs through render paths.
pub struct ResourceRegistry<'a> {
    application: &'a mut Application,
}

impl ResourceRegistry<'_> {
    /// Register a font with an explicit stable handle.
    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        self.application.register_font(handle, font)
    }

    /// Register font bytes and allocate a stable handle.
    pub fn font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        self.application.register_font_bytes(data)
    }

    /// Register an image with an explicit stable handle.
    pub fn image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        self.application.register_image(handle, image)
    }

    /// Register RGBA8 pixels and allocate an image handle.
    pub fn rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        self.application.register_rgba_image(width, height, data)
    }

    /// Register SVG bytes at their intrinsic size and allocate an image handle.
    pub fn svg_image(&mut self, data: impl AsRef<[u8]>) -> Result<ImageHandle> {
        self.application.register_svg_image(data)
    }

    /// Register SVG bytes at their intrinsic size with an explicit handle.
    pub fn svg_image_with_handle(
        &mut self,
        handle: ImageHandle,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.application
            .register_svg_image_with_handle(handle, data)
    }

    /// Rasterize SVG bytes at an explicit size and allocate an image handle.
    pub fn svg_image_at_size(
        &mut self,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<ImageHandle> {
        self.application
            .register_svg_image_at_size(width, height, data)
    }

    /// Rasterize SVG bytes at an explicit size and handle.
    pub fn svg_image_at_size_with_handle(
        &mut self,
        handle: ImageHandle,
        width: u32,
        height: u32,
        data: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.application
            .register_svg_image_at_size_with_handle(handle, width, height, data)
    }

    /// Register one compile-time embedded SVG resource.
    pub fn embedded_svg_image(&mut self, resource: EmbeddedSvgImageResource) -> Result<()> {
        self.application.register_embedded_svg_image(resource)
    }

    /// Register multiple compile-time embedded SVG resources.
    pub fn embedded_svg_images(
        &mut self,
        resources: impl IntoIterator<Item = EmbeddedSvgImageResource>,
    ) -> Result<()> {
        self.application.register_embedded_svg_images(resources)
    }
}

/// Cloneable, thread-safe UI command handle for background work.
#[cfg(any(feature = "desktop", feature = "web", feature = "mobile"))]
#[derive(Clone)]
pub struct UiHandle {
    commands: CommandSender,
}

#[cfg(any(feature = "desktop", feature = "web", feature = "mobile"))]
impl UiHandle {
    #[cfg_attr(
        not(any(
            feature = "desktop",
            feature = "web",
            all(target_os = "android", feature = "mobile")
        )),
        allow(dead_code)
    )]
    fn new(commands: CommandSender) -> Self {
        Self { commands }
    }

    /// Schedule controller wake hooks without synthesizing a widget event.
    pub fn wake(&self) {
        self.commands.wake();
    }

    pub fn send<T>(&self, target: CommandTarget, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.commands.send(target, key, payload);
    }

    pub fn send_widget<T>(
        &self,
        window_id: crate::WindowId,
        widget_id: crate::WidgetId,
        key: CommandKey<T>,
        payload: T,
    ) where
        T: Send + Sync + 'static,
    {
        self.commands
            .send_widget(window_id, widget_id, key, payload);
    }

    pub fn send_focused<T>(&self, window_id: crate::WindowId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.commands.send_focused(window_id, key, payload);
    }

    pub fn send_window<T>(&self, window_id: crate::WindowId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.commands.send_window(window_id, key, payload);
    }

    pub fn send_application<T>(&self, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.commands.send_application(key, payload);
    }

    pub fn broadcast_window<T>(&self, window_id: crate::WindowId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.commands.broadcast_window(window_id, key, payload);
    }

    pub fn broadcast_application<T>(&self, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.commands.broadcast_application(key, payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Constraints, MeasureCtx, Size};

    struct TestWidget;

    impl Widget for TestWidget {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
            Size::new(32.0, 24.0)
        }
    }

    #[test]
    fn app_builds_window_after_registering_resources() -> Result<()> {
        let mut app = App::new();
        let image = {
            let mut resources = app.resources();
            resources.rgba_image(1, 1, vec![255, 0, 0, 255])?
        };

        let runtime = app
            .main_window(format!("Image {image:?}"), TestWidget)
            .build()?;

        assert_eq!(runtime.window_ids().len(), 1);
        Ok(())
    }

    #[test]
    fn app_with_resources_preserves_builder_style() -> Result<()> {
        let runtime = App::new()
            .with_resources(|resources| {
                resources.rgba_image(1, 1, vec![0, 255, 0, 255])?;
                Ok(())
            })?
            .window(Window::new("Resource setup").root(TestWidget))
            .build()?;

        assert_eq!(runtime.window_ids().len(), 1);
        Ok(())
    }
}
