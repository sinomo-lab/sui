#![forbid(unsafe_code)]

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
    sync::Arc,
};

pub mod containers;
pub mod controls;

pub use containers::{Align, Background, SizedBox, Stack};
pub use controls::{
    Button, Checkbox, ControlMetrics, ControlPalette, ControlTypography, DefaultTheme, Label,
    TextInput,
};
pub use sui_core::{
    AsyncWakeToken, Color, ColorSpace, CustomEvent, DirtyRegion, DpiInfo, Error, Event, FontHandle,
    ImageHandle, ImeEvent, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState,
    KeyboardEvent, Modifiers, Path, PathBuilder, PathElement, Point, PointerButton, PointerButtons,
    PointerEvent, PointerEventKind, PointerKind, Rect, Result, ScrollDelta, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsState, SemanticsValue, Size, SurfaceId, TimerToken,
    ToggleState, Transform, Vector, WakeEvent, WidgetId, WindowEvent, WindowId,
};
pub use sui_layout::Padding as Insets;
pub use sui_layout::{Alignment, Axis, Constraints, Padding};
#[cfg(feature = "desktop")]
pub use sui_platform::{AccessibilitySnapshot, DesktopPlatform, HeadlessPlatform, PlatformWindow};
#[cfg(feature = "wgpu")]
pub use sui_render_wgpu::{RendererCapabilities, RendererInterop, WgpuRenderer};
pub use sui_runtime::{
    Application as RuntimeApplication, EventCtx, EventPhase, FocusState, FrameSchedule, LayoutCtx,
    PaintCtx, RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetChildren,
    WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
    WindowBuilder,
};
pub use sui_scene::{
    Brush, ImageRegistry, ImageSource, RegisteredImage, RegisteredImageFormat, Scene, SceneCommand,
    SceneFrame, StrokeStyle,
};
pub use sui_text::{
    FontRegistry, RegisteredFont, ResolvedTextFace, ShapedGlyph, ShapedText, TextLayout, TextLine,
    TextMeasurement, TextRun, TextStyle,
};

pub trait ThemeExtension: Any + Send + Sync {}

impl<T> ThemeExtension for T where T: Any + Send + Sync {}

#[derive(Clone, Default)]
pub struct ThemeExtensions {
    values: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ThemeExtensions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T>(&mut self, value: T) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .insert(TypeId::of::<T>(), Arc::new(value))
            .and_then(|previous| Arc::downcast::<T>(previous).ok())
    }

    pub fn get<T>(&self) -> Option<&T>
    where
        T: ThemeExtension,
    {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    pub fn get_arc<T>(&self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| Arc::clone(value).downcast::<T>().ok())
    }

    pub fn contains<T>(&self) -> bool
    where
        T: ThemeExtension,
    {
        self.values.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T>(&mut self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.values
            .remove(&TypeId::of::<T>())
            .and_then(|value| Arc::downcast::<T>(value).ok())
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

impl fmt::Debug for ThemeExtensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThemeExtensions")
            .field("len", &self.values.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub default_widgets: DefaultTheme,
    pub extensions: ThemeExtensions,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.96, 0.972, 0.988, 1.0),
            foreground: Color::rgba(0.12, 0.15, 0.20, 1.0),
            default_widgets: DefaultTheme::default(),
            extensions: ThemeExtensions::default(),
        }
    }
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_widgets(mut self, theme: DefaultTheme) -> Self {
        self.default_widgets = theme;
        self
    }

    pub fn with_extension<T>(mut self, value: T) -> Self
    where
        T: ThemeExtension,
    {
        self.extensions.insert(value);
        self
    }

    pub fn insert_extension<T>(&mut self, value: T) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.insert(value)
    }

    pub fn extension<T>(&self) -> Option<&T>
    where
        T: ThemeExtension,
    {
        self.extensions.get::<T>()
    }

    pub fn extension_arc<T>(&self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.get_arc::<T>()
    }

    pub fn has_extension<T>(&self) -> bool
    where
        T: ThemeExtension,
    {
        self.extensions.contains::<T>()
    }

    pub fn remove_extension<T>(&mut self) -> Option<Arc<T>>
    where
        T: ThemeExtension,
    {
        self.extensions.remove::<T>()
    }
}

pub struct Application {
    inner: RuntimeApplication,
    #[cfg(feature = "wgpu")]
    feather_width: f32,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            inner: RuntimeApplication::default(),
            #[cfg(feature = "wgpu")]
            feather_width: WgpuRenderer::new().feather_width(),
        }
    }
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.inner = self.inner.window(window);
        self
    }

    #[cfg(feature = "wgpu")]
    pub fn with_feather_width(mut self, feather_width: f32) -> Self {
        self.feather_width = feather_width.max(0.0);
        self
    }

    #[cfg(feature = "wgpu")]
    pub fn feather_width(&self) -> f32 {
        self.feather_width
    }

    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        self.inner.register_font(handle, font)
    }

    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        self.inner.register_font_bytes(data)
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) -> Result<()> {
        self.inner.register_image(handle, image)
    }

    pub fn register_rgba_image(
        &mut self,
        width: u32,
        height: u32,
        data: impl Into<Vec<u8>>,
    ) -> Result<ImageHandle> {
        self.inner.register_rgba_image(width, height, data)
    }

    pub fn build(self) -> Result<Runtime> {
        self.inner.build()
    }

    #[cfg(feature = "desktop")]
    pub fn run(self) -> Result<()> {
        let feather_width = self.feather_width;
        let mut runtime = self.build()?;
        let mut platform = DesktopPlatform::new().with_feather_width(feather_width);
        let _ = platform.run(&mut runtime)?;
        Ok(())
    }

    #[cfg(not(feature = "desktop"))]
    pub fn run(self) -> Result<()> {
        let _ = self;
        Err(Error::new(
            "Application::run requires the `desktop` feature to provide a platform event loop",
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub foreground: Brush,
    pub padding: Padding,
}

impl Default for Style {
    fn default() -> Self {
        let theme = Theme::default();

        Self {
            foreground: Brush::Solid(theme.foreground),
            padding: Padding::all(0.0),
        }
    }
}

pub mod prelude {
    pub use crate::{
        Align, Alignment, Application, AsyncWakeToken, Axis, Background, Brush, Button, Checkbox,
        Color, Constraints, ControlMetrics, ControlPalette, ControlTypography, DefaultTheme, Event,
        EventCtx, FontHandle, ImageHandle, ImeEvent, Insets, KeyboardEvent, Label, LayoutCtx,
        PaintCtx, Path, PathBuilder, Point, PointerEvent, Rect, RegisteredFont, RegisteredImage,
        Result, SemanticsCtx, ShapedText, SingleChild, Size, SizedBox, Stack, StrokeStyle, Style,
        TextInput, TextLayout, TextMeasurement, TextStyle, Theme, ThemeExtension, ThemeExtensions,
        TimerToken, Transform, WakeEvent, Widget, WidgetChildren, WidgetPod, WindowBuilder,
        containers::Padding,
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{DefaultTheme, Theme};
    #[cfg(feature = "wgpu")]
    use crate::Application;

    #[derive(Debug, PartialEq)]
    struct CustomWidgetTheme {
        radius: f32,
        density: u8,
    }

    #[test]
    fn theme_stores_default_widget_theme_separately_from_extensions() {
        let mut defaults = DefaultTheme::default();
        defaults.metrics.min_height = 44.0;

        let theme = Theme::new()
            .with_default_widgets(defaults)
            .with_extension(CustomWidgetTheme {
                radius: 6.0,
                density: 2,
            });

        assert_eq!(theme.default_widgets.metrics.min_height, 44.0);
        assert!(theme.has_extension::<CustomWidgetTheme>());
        assert_eq!(
            theme.extension::<CustomWidgetTheme>(),
            Some(&CustomWidgetTheme {
                radius: 6.0,
                density: 2,
            })
        );
    }

    #[test]
    fn theme_extensions_support_arc_access_and_removal() {
        let mut theme = Theme::new();
        theme.insert_extension(CustomWidgetTheme {
            radius: 12.0,
            density: 3,
        });

        let extension = theme
            .extension_arc::<CustomWidgetTheme>()
            .expect("custom widget theme present");
        assert_eq!(
            Arc::as_ref(&extension),
            &CustomWidgetTheme {
                radius: 12.0,
                density: 3,
            }
        );

        let removed = theme
            .remove_extension::<CustomWidgetTheme>()
            .expect("custom widget theme removed");
        assert_eq!(
            Arc::as_ref(&removed),
            &CustomWidgetTheme {
                radius: 12.0,
                density: 3,
            }
        );
        assert!(!theme.has_extension::<CustomWidgetTheme>());
    }

    #[cfg(feature = "wgpu")]
    #[test]
    fn application_feather_width_is_configurable() {
        let app = Application::new().with_feather_width(2.25);
        let clamped = Application::new().with_feather_width(-1.0);

        assert_eq!(app.feather_width(), 2.25);
        assert_eq!(clamped.feather_width(), 0.0);
    }
}
