use crate::paint::PaintValidationError;
use crate::support::recover_lock;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub(crate) static NEXT_FOREIGN_WIDGET_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) const BINDING_APP_FONT_HANDLE_NAMESPACE: u64 = 1 << 60;
pub(crate) const BINDING_APP_FONT_SLOT_MASK: u64 = BINDING_APP_FONT_HANDLE_NAMESPACE - 1;
pub(crate) const BINDING_APP_IMAGE_HANDLE_NAMESPACE: u64 = 1 << 61;
pub(crate) const BINDING_APP_IMAGE_SLOT_MASK: u64 = BINDING_APP_IMAGE_HANDLE_NAMESPACE - 1;
pub(crate) const BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE: u64 = 1 << 62;
pub(crate) const BINDING_LOCAL_IMAGE_SLOT_MASK: u64 = BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE - 1;

pub(crate) type UiTask = Box<dyn FnOnce() + Send + 'static>;
pub(crate) type UiWake = Arc<dyn Fn() + Send + Sync + 'static>;

macro_rules! themed_widget {
    ($widget:expr, $context:expr) => {{
        let widget = $widget;
        if let Some(theme) = $context.theme.clone() {
            widget.theme_when(move || theme.snapshot())
        } else {
            widget
        }
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForeignWidgetId(pub(crate) u64);

impl ForeignWidgetId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Default for ForeignWidgetId {
    fn default() -> Self {
        Self::new(NEXT_FOREIGN_WIDGET_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackPhase {
    DebugName,
    Event,
    Measure,
    Arrange,
    Paint,
    Semantics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallbackFailure {
    pub(crate) message: String,
}

impl ForeignCallbackFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ForeignCallbackFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ForeignCallbackFailure {}

impl From<String> for ForeignCallbackFailure {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ForeignCallbackFailure {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<PaintValidationError> for ForeignCallbackFailure {
    fn from(value: PaintValidationError) -> Self {
        Self::new(value.to_string())
    }
}

pub type ForeignCallbackResult<T> = std::result::Result<T, ForeignCallbackFailure>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCallbackError {
    pub widget_id: ForeignWidgetId,
    pub phase: ForeignCallbackPhase,
    pub message: String,
}

impl ForeignCallbackError {
    pub fn new(
        widget_id: ForeignWidgetId,
        phase: ForeignCallbackPhase,
        message: impl Into<String>,
    ) -> Self {
        Self {
            widget_id,
            phase,
            message: message.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ForeignErrorSink {
    pub(crate) errors: Arc<Mutex<Vec<ForeignCallbackError>>>,
}

impl ForeignErrorSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, error: ForeignCallbackError) {
        recover_lock(&self.errors).push(error);
    }

    pub fn drain(&self) -> Vec<ForeignCallbackError> {
        std::mem::take(&mut *recover_lock(&self.errors))
    }

    pub fn snapshot(&self) -> Vec<ForeignCallbackError> {
        recover_lock(&self.errors).clone()
    }

    pub fn is_empty(&self) -> bool {
        recover_lock(&self.errors).is_empty()
    }
}

impl fmt::Debug for ForeignErrorSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForeignErrorSink")
            .field("len", &recover_lock(&self.errors).len())
            .finish()
    }
}

pub(crate) use themed_widget;
