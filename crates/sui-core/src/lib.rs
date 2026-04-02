#![forbid(unsafe_code)]

mod color;
mod error;
mod event;
mod geometry;
mod id;
mod invalidation;
mod semantics;

pub use color::{Color, ColorSpace};
pub use error::{Error, Result};
pub use event::{
    CustomEvent, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, ScrollDelta, WindowEvent,
};
pub use geometry::{Point, Rect, Size, Vector};
pub use id::{FontHandle, ImageHandle, SurfaceId, WidgetId, WindowId};
pub use invalidation::{DirtyRegion, InvalidationKind, InvalidationRequest, InvalidationTarget};
pub use semantics::{
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState, SemanticsValue, ToggleState,
};
