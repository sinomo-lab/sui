#![forbid(unsafe_code)]

pub mod containers;

pub use containers::{Align, Background, SizedBox, Stack};
pub use sui_core::{
    AsyncWakeToken, Color, ColorSpace, CustomEvent, DirtyRegion, Error, Event, FontHandle,
    ImageHandle, ImeEvent, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState,
    KeyboardEvent, Modifiers, Path, PathBuilder, PathElement, Point, PointerButton, PointerButtons,
    PointerEvent, PointerEventKind, PointerKind, Rect, Result, ScrollDelta, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsState, SemanticsValue, Size, SurfaceId, TimerToken,
    ToggleState, Transform, Vector, WakeEvent, WidgetId, WindowEvent, WindowId,
};
pub use sui_layout::Padding as Insets;
pub use sui_layout::{Alignment, Axis, Constraints, Padding};
#[cfg(feature = "desktop")]
pub use sui_platform::{DesktopPlatform, HeadlessPlatform, PlatformWindow};
#[cfg(feature = "wgpu")]
pub use sui_render_wgpu::{RendererCapabilities, RendererInterop, WgpuRenderer};
pub use sui_runtime::{
    Application as RuntimeApplication, EventCtx, EventPhase, FocusState, FrameSchedule, LayoutCtx,
    PaintCtx, RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetChildren,
    WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
    WindowBuilder,
};
pub use sui_scene::{
    Brush, FontRegistry, ImageSource, RegisteredFont, Scene, SceneCommand, SceneFrame, StrokeStyle,
    TextRun, TextStyle,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.08, 0.09, 0.11, 1.0),
            foreground: Color::rgba(0.95, 0.96, 0.98, 1.0),
        }
    }
}

#[derive(Default)]
pub struct Application {
    inner: RuntimeApplication,
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: WindowBuilder) -> Self {
        self.inner = self.inner.window(window);
        self
    }

    pub fn register_font(&mut self, handle: FontHandle, font: RegisteredFont) -> Result<()> {
        self.inner.register_font(handle, font)
    }

    pub fn register_font_bytes(&mut self, data: impl Into<Vec<u8>>) -> Result<FontHandle> {
        self.inner.register_font_bytes(data)
    }

    pub fn build(self) -> Result<Runtime> {
        self.inner.build()
    }

    #[cfg(feature = "desktop")]
    pub fn run(self) -> Result<()> {
        let mut runtime = self.build()?;
        let mut platform = DesktopPlatform::new();
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
        Align, Alignment, Application, AsyncWakeToken, Axis, Background, Brush, Color, Constraints,
        Event, EventCtx, FontHandle, ImeEvent, Insets, KeyboardEvent, LayoutCtx, PaintCtx, Path,
        PathBuilder, Point, PointerEvent, Rect, RegisteredFont, Result, SemanticsCtx, SingleChild,
        Size, SizedBox, Stack, StrokeStyle, Style, TextStyle, Theme, TimerToken, Transform,
        WakeEvent, Widget, WidgetChildren, WidgetPod, WindowBuilder, containers::Padding,
    };
}
