#![forbid(unsafe_code)]

pub use sui_core::{
    Color, CustomEvent, DirtyRegion, Error, Event, FontHandle, ImeEvent, ImageHandle,
    InvalidationKind, KeyState, KeyboardEvent, Modifiers, Point, PointerButton,
    PointerEvent, PointerEventKind, Rect, Result, SemanticsAction, SemanticsNode,
    SemanticsRole, SemanticsState, Size, SurfaceId, WidgetId, WindowEvent, WindowId,
};
pub use sui_layout::{Alignment, Axis, Constraints, Padding};
#[cfg(feature = "desktop")]
pub use sui_platform::{DesktopPlatform, PlatformWindow};
#[cfg(feature = "wgpu")]
pub use sui_render_wgpu::{RendererCapabilities, RendererInterop, WgpuRenderer};
pub use sui_runtime::{
    Application, EventCtx, LayoutCtx, PaintCtx, RenderOutput, Runtime, SemanticsCtx,
    Widget, WindowBuilder,
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
        Application, Brush, Color, Constraints, Event, EventCtx, ImeEvent,
        KeyboardEvent, LayoutCtx, PaintCtx, PointerEvent, Result, SemanticsCtx,
        Size, Style, Theme, Widget, WindowBuilder,
    };
}