use crate::support::recover_lock;
use crate::tasks::BindingUiHandle;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use sui::FocusScopeState;
use sui::ScrollState;
use sui::Size;
use sui::Vector;

/// A thread-safe handle for scrolling a scroll view from host-language code.
///
/// Requests are applied on the UI thread during the view's next layout, and
/// the view publishes its offset and extents back after each layout. A
/// controller is meant for one scroll view at a time.
#[derive(Clone, Default)]
pub struct BindingScrollController {
    inner: Arc<Mutex<ScrollControllerInner>>,
}

#[derive(Default)]
struct ScrollControllerInner {
    pending: Option<ScrollRequest>,
    offset: Vector,
    max_offset: Vector,
    viewport: Size,
    content: Size,
    ui_handle: Option<BindingUiHandle>,
}

#[derive(Debug, Clone, Copy)]
enum ScrollRequest {
    Offset { x: Option<f32>, y: Option<f32> },
    Item(usize),
}

impl BindingScrollController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll to a content offset; `None` keeps that axis. The view clamps
    /// the offset to its scrollable range.
    pub fn scroll_to(&self, x: Option<f32>, y: Option<f32>) {
        self.request(ScrollRequest::Offset { x, y });
    }

    /// Align an item of a virtual scroll view with the viewport start.
    pub fn scroll_to_item(&self, index: usize) {
        self.request(ScrollRequest::Item(index));
    }

    /// The offset from the latest layout.
    pub fn offset(&self) -> Vector {
        recover_lock(&self.inner).offset
    }

    /// The largest offset the latest layout allows on each axis.
    pub fn max_offset(&self) -> Vector {
        recover_lock(&self.inner).max_offset
    }

    pub fn viewport_size(&self) -> Size {
        recover_lock(&self.inner).viewport
    }

    pub fn content_size(&self) -> Size {
        recover_lock(&self.inner).content
    }

    fn request(&self, request: ScrollRequest) {
        let handle = {
            let mut inner = recover_lock(&self.inner);
            inner.pending = Some(request);
            inner.ui_handle.clone()
        };
        // An empty UI task wakes the loop and lays the window out again.
        if let Some(handle) = handle {
            handle.post(|| {});
        }
    }

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        recover_lock(&self.inner).ui_handle = Some(handle.clone());
    }

    /// Apply a pending request to the view's scroll state.
    pub(crate) fn apply_pending(&self, state: &ScrollState) {
        let pending = recover_lock(&self.inner).pending.take();
        match pending {
            Some(ScrollRequest::Offset { x, y }) => {
                let current = state.current_offset();
                state.set_offset(Vector::new(x.unwrap_or(current.x), y.unwrap_or(current.y)));
            }
            Some(ScrollRequest::Item(index)) => {
                state.scroll_to_item(index);
            }
            None => {}
        }
    }

    /// Record the view's metrics after layout.
    pub(crate) fn publish(&self, state: &ScrollState) {
        let mut inner = recover_lock(&self.inner);
        inner.offset = state.current_offset();
        inner.max_offset = state.max_offset();
        inner.viewport = state.viewport_size();
        inner.content = state.content_size();
    }
}

impl fmt::Debug for BindingScrollController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = recover_lock(&self.inner);
        f.debug_struct("BindingScrollController")
            .field("offset", &inner.offset)
            .field("pending", &inner.pending)
            .finish_non_exhaustive()
    }
}

/// A thread-safe handle that moves keyboard focus into a focus scope.
#[derive(Clone, Default)]
pub struct BindingFocusController {
    state: FocusScopeState,
    ui_handle: Arc<Mutex<Option<BindingUiHandle>>>,
}

impl BindingFocusController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Focus the scope's last focused descendant, or its first focusable one,
    /// at the next frame, even when focus is currently elsewhere.
    pub fn focus(&self) {
        self.state.request_focus();
        // Wake the loop so the scope lays out and applies the request.
        if let Some(handle) = recover_lock(&self.ui_handle).clone() {
            handle.post(|| {});
        }
    }

    pub(crate) fn state(&self) -> FocusScopeState {
        self.state.clone()
    }

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        *recover_lock(&self.ui_handle) = Some(handle.clone());
    }
}

impl fmt::Debug for BindingFocusController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingFocusController")
            .finish_non_exhaustive()
    }
}
