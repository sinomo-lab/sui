#![forbid(unsafe_code)]

pub use sui_core::{
    Color, ColorSpace, CustomEvent, DirtyRegion, Error, Event, FontHandle, ImageHandle, ImeEvent,
    InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, KeyboardEvent, Modifiers,
    Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, PointerKind, Rect,
    Result, ScrollDelta, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState,
    SemanticsValue, Size, SurfaceId, ToggleState, Vector, WidgetId, WindowEvent, WindowId,
};
pub use sui_layout::{Alignment, Axis, Constraints, Padding};
#[cfg(feature = "desktop")]
pub use sui_platform::{DesktopPlatform, PlatformWindow};
#[cfg(feature = "wgpu")]
pub use sui_render_wgpu::{RendererCapabilities, RendererInterop, WgpuRenderer};
pub use sui_runtime::{
    Application as RuntimeApplication, EventCtx, EventPhase, FocusState, FrameSchedule, LayoutCtx,
    PaintCtx, RenderOutput, Runtime, SemanticsCtx, SingleChild, Widget, WidgetChildren,
    WidgetGraphSnapshot, WidgetNodeSnapshot, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
    WindowBuilder,
};
pub use sui_scene::{Brush, Scene, SceneCommand, SceneFrame};

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
        Application, Brush, Color, Constraints, Event, EventCtx, ImeEvent, KeyboardEvent,
        LayoutCtx, PaintCtx, Point, PointerEvent, Rect, Result, SemanticsCtx, SingleChild, Size,
        Style, Theme, Widget, WidgetChildren, WidgetPod, WindowBuilder,
    };
}
