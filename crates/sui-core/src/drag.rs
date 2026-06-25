use std::{
    any::Any,
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{ImageHandle, Point, Rect, WidgetId};

static NEXT_DRAG_SCOPE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DragScopeId(u64);

impl DragScopeId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for DragScopeId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<DragScopeId> for u64 {
    fn from(value: DragScopeId) -> Self {
        value.get()
    }
}

impl fmt::Display for DragScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DragSessionId(u64);

impl DragSessionId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for DragSessionId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<DragSessionId> for u64 {
    fn from(value: DragSessionId) -> Self {
        value.get()
    }
}

impl fmt::Display for DragSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropEffect {
    #[default]
    None,
    Copy,
    Move,
    Link,
}

impl DropEffect {
    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub const fn is_some(self) -> bool {
        !self.is_none()
    }
}

#[derive(Clone)]
pub enum DragPayload {
    Text(String),
    Image {
        handle: ImageHandle,
        region: Option<Rect>,
    },
    Custom {
        kind: Arc<str>,
        data: Arc<dyn Any + Send + Sync>,
    },
}

impl DragPayload {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn image(handle: ImageHandle) -> Self {
        Self::Image {
            handle,
            region: None,
        }
    }

    pub fn image_region(handle: ImageHandle, region: Rect) -> Self {
        Self::Image {
            handle,
            region: Some(region),
        }
    }

    pub fn custom<T>(kind: impl Into<Arc<str>>, data: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        Self::Custom {
            kind: kind.into(),
            data: Arc::new(data),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Image { .. } | Self::Custom { .. } => None,
        }
    }

    pub fn custom_kind(&self) -> Option<&str> {
        match self {
            Self::Custom { kind, .. } => Some(kind.as_ref()),
            Self::Text(_) | Self::Image { .. } => None,
        }
    }

    pub fn custom_data<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        match self {
            Self::Custom { data, .. } => data.downcast_ref(),
            Self::Text(_) | Self::Image { .. } => None,
        }
    }
}

impl fmt::Debug for DragPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => f.debug_tuple("Text").field(text).finish(),
            Self::Image { handle, region } => f
                .debug_struct("Image")
                .field("handle", handle)
                .field("region", region)
                .finish(),
            Self::Custom { kind, .. } => f
                .debug_struct("Custom")
                .field("kind", kind)
                .finish_non_exhaustive(),
        }
    }
}

impl PartialEq for DragPayload {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(left), Self::Text(right)) => left == right,
            (
                Self::Image {
                    handle: left_handle,
                    region: left_region,
                },
                Self::Image {
                    handle: right_handle,
                    region: right_region,
                },
            ) => left_handle == right_handle && left_region == right_region,
            (Self::Custom { kind: left, .. }, Self::Custom { kind: right, .. }) => left == right,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragOutcome {
    Dropped {
        target: WidgetId,
        effect: DropEffect,
    },
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEventKind {
    Enter,
    Over,
    Leave,
    Drop,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DragEvent {
    pub kind: DragEventKind,
    pub session_id: DragSessionId,
    pub scope_id: DragScopeId,
    pub pointer_id: u64,
    pub source: WidgetId,
    pub target: Option<WidgetId>,
    pub position: Point,
    pub start_position: Point,
    pub payload: DragPayload,
    pub allowed_effect: DropEffect,
    pub accepted_effect: DropEffect,
    pub preview_label: Option<Arc<str>>,
    pub outcome: Option<DragOutcome>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DragPreview {
    pub session_id: DragSessionId,
    pub scope_id: DragScopeId,
    pub pointer_id: u64,
    pub source: WidgetId,
    pub position: Point,
    pub start_position: Point,
    pub payload: DragPayload,
    pub allowed_effect: DropEffect,
    pub preview_label: Option<Arc<str>>,
}

#[derive(Debug, Default)]
struct DragDropState {
    active: Option<DragPreview>,
}

#[derive(Clone, Debug)]
pub struct DragDropScope {
    id: DragScopeId,
    inner: Rc<RefCell<DragDropState>>,
}

impl DragDropScope {
    pub fn new() -> Self {
        Self {
            id: DragScopeId::new(NEXT_DRAG_SCOPE_ID.fetch_add(1, Ordering::Relaxed)),
            inner: Rc::new(RefCell::new(DragDropState::default())),
        }
    }

    pub const fn id(&self) -> DragScopeId {
        self.id
    }

    pub fn active_drag(&self) -> Option<DragPreview> {
        self.inner.borrow().active.clone()
    }

    pub fn set_active_drag(&self, active: DragPreview) {
        self.inner.borrow_mut().active = Some(active);
    }

    pub fn update_drag_position(&self, session_id: DragSessionId, position: Point) -> bool {
        let mut state = self.inner.borrow_mut();
        let Some(active) = &mut state.active else {
            return false;
        };
        if active.session_id != session_id {
            return false;
        }
        active.position = position;
        true
    }

    pub fn finish_drag(&self, session_id: DragSessionId) -> bool {
        let mut state = self.inner.borrow_mut();
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.session_id == session_id)
        {
            state.active = None;
            true
        } else {
            false
        }
    }

    pub fn clear(&self) {
        self.inner.borrow_mut().active = None;
    }
}

impl Default for DragDropScope {
    fn default() -> Self {
        Self::new()
    }
}
