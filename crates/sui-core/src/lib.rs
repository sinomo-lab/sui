#![forbid(unsafe_code)]

mod color;
mod dpi;
mod drag;
mod error;
mod event;
mod geometry;
mod id;
mod invalidation;
mod semantics;

pub use color::{Color, ColorSpace};
pub use dpi::DpiInfo;
pub use drag::{
    DragDropScope, DragEvent, DragEventKind, DragOutcome, DragPayload, DragPreview, DragScopeId,
    DragSessionId, DropEffect,
};
pub use error::{Error, Result};
pub use event::{
    CustomEvent, Event, ImeEvent, KeyState, KeyboardEvent, Modifiers, PointerButton,
    PointerButtons, PointerEvent, PointerEventKind, PointerKind, ScrollDelta, WakeEvent,
    WindowEvent,
};
pub use geometry::{Path, PathBuilder, PathElement, Point, Rect, Size, Transform, Vector};
pub use id::{AsyncWakeToken, FontHandle, ImageHandle, SurfaceId, TimerToken, WidgetId, WindowId};
pub use invalidation::{DirtyRegion, InvalidationKind, InvalidationRequest, InvalidationTarget};
pub use semantics::{
    EditableTextSemantics, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState,
    SemanticsTextRange, SemanticsValue, ToggleState,
};
