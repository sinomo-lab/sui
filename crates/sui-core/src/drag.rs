use std::{
    any::Any,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{ImageHandle, Modifiers, Point, Rect, WidgetId};

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

    /// The effect the held modifier keys ask for, by the platform's
    /// convention: on macOS Option copies, Command moves and both link;
    /// elsewhere Control copies, Shift moves and both link. `None` when no
    /// modifier asks for an effect.
    pub const fn for_modifiers(modifiers: Modifiers) -> Option<Self> {
        let (copy, moving) = if cfg!(target_os = "macos") {
            (modifiers.alt, modifiers.meta)
        } else {
            (modifiers.control, modifiers.shift)
        };
        match (copy, moving) {
            (true, true) => Some(Self::Link),
            (true, false) => Some(Self::Copy),
            (false, true) => Some(Self::Move),
            (false, false) => None,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Copy => 1,
            Self::Move => 2,
            Self::Link => 4,
        }
    }
}

/// A set of drop effects, such as every effect a drag source allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DropEffects(u8);

impl DropEffects {
    pub const NONE: Self = Self(0);
    pub const COPY: Self = Self(1);
    pub const MOVE: Self = Self(2);
    pub const LINK: Self = Self(4);
    pub const ALL: Self = Self(7);

    pub const fn contains(self, effect: DropEffect) -> bool {
        effect.is_some() && self.0 & effect.bit() != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// This set with `effect` added.
    pub const fn with(self, effect: DropEffect) -> Self {
        Self(self.0 | effect.bit())
    }

    /// The effect a drop takes when no modifier key asks for another: move,
    /// then copy, then link, whichever the set holds first.
    pub const fn default_effect(self) -> DropEffect {
        if self.contains(DropEffect::Move) {
            DropEffect::Move
        } else if self.contains(DropEffect::Copy) {
            DropEffect::Copy
        } else if self.contains(DropEffect::Link) {
            DropEffect::Link
        } else {
            DropEffect::None
        }
    }
}

impl From<DropEffect> for DropEffects {
    fn from(effect: DropEffect) -> Self {
        Self::NONE.with(effect)
    }
}

impl std::ops::BitOr for DropEffects {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
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
    /// The effect a drop takes when no modifier key asks for another.
    pub allowed_effect: DropEffect,
    /// Every effect the source allows, including [`Self::allowed_effect`].
    pub allowed_effects: DropEffects,
    pub accepted_effect: DropEffect,
    pub preview_label: Option<Arc<str>>,
    pub outcome: Option<DragOutcome>,
    /// The modifier keys held when the event was sent. The runtime sends
    /// targets a fresh `Over` when they change, so an `accept` callback can
    /// follow them.
    pub modifiers: Modifiers,
}

impl DragEvent {
    /// The effect the held modifier keys ask for; see
    /// [`DropEffect::for_modifiers`].
    pub const fn requested_effect(&self) -> Option<DropEffect> {
        DropEffect::for_modifiers(self.modifiers)
    }

    /// The effect to accept this drag with: the one the modifier keys ask
    /// for when the source allows it, otherwise the source's default.
    pub fn preferred_effect(&self) -> DropEffect {
        self.requested_effect()
            .filter(|effect| self.allowed_effects.contains(*effect))
            .unwrap_or(self.allowed_effect)
    }
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
    pub allowed_effects: DropEffects,
    pub preview_label: Option<Arc<str>>,
}

#[derive(Debug, Default)]
struct DragDropState {
    active: Option<DragPreview>,
}

#[derive(Clone, Debug)]
pub struct DragDropScope {
    id: DragScopeId,
    inner: Arc<Mutex<DragDropState>>,
}

impl DragDropScope {
    pub fn new() -> Self {
        Self {
            id: DragScopeId::new(NEXT_DRAG_SCOPE_ID.fetch_add(1, Ordering::Relaxed)),
            inner: Arc::new(Mutex::new(DragDropState::default())),
        }
    }

    pub const fn id(&self) -> DragScopeId {
        self.id
    }

    pub fn active_drag(&self) -> Option<DragPreview> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .clone()
    }

    pub fn set_active_drag(&self, active: DragPreview) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active = Some(active);
    }

    pub fn update_drag_position(&self, session_id: DragSessionId, position: Point) -> bool {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active = None;
    }
}

impl Default for DragDropScope {
    fn default() -> Self {
        Self::new()
    }
}
