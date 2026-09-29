//! Debug captures requested by the running app.
//!
//! An app asks for a capture of a window with
//! [`request_window_debug_capture`]. After its next redraw the platform
//! renders the window's last frame again offscreen at the requested stage,
//! reads it back, and keeps the artifact for the app to collect with
//! [`take_window_debug_capture`]. If the request carried an
//! [`AsyncWakeToken`], the widget that registered it is woken then.
//!
//! The platforms here do the capturing. A host that drives its own event
//! loop redraws while [`has_pending_window_debug_captures`] and calls
//! [`service_window_debug_captures`] after each redraw.

use std::{
    collections::HashMap,
    sync::{
        Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use sui_core::{AsyncWakeToken, Result, WindowId};
use sui_render_wgpu::{DebugCaptureArtifact, DebugCaptureRequest, WgpuRenderer};

/// Identifies a requested capture until its result is collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugCaptureTicket(u64);

struct PendingCapture {
    ticket: DebugCaptureTicket,
    request: DebugCaptureRequest,
    wake: Option<AsyncWakeToken>,
}

#[derive(Default)]
struct CaptureStore {
    pending: HashMap<WindowId, Vec<PendingCapture>>,
    results: HashMap<DebugCaptureTicket, Result<DebugCaptureArtifact>>,
}

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

fn store() -> &'static Mutex<CaptureStore> {
    static STORE: OnceLock<Mutex<CaptureStore>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}

fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ask for a capture of `window_id` after its next redraw. When it is done,
/// `wake` (if any) is delivered to the widget that registered it; collect the
/// artifact with [`take_window_debug_capture`].
pub fn request_window_debug_capture(
    window_id: WindowId,
    request: DebugCaptureRequest,
    wake: Option<AsyncWakeToken>,
) -> DebugCaptureTicket {
    let ticket = DebugCaptureTicket(NEXT_TICKET.fetch_add(1, Ordering::Relaxed));
    recover_lock(store())
        .pending
        .entry(window_id)
        .or_default()
        .push(PendingCapture {
            ticket,
            request,
            wake,
        });
    ticket
}

/// The capture for `ticket`, once the platform has made it. Returns `None`
/// while it is pending; each result is handed out once.
pub fn take_window_debug_capture(
    ticket: DebugCaptureTicket,
) -> Option<Result<DebugCaptureArtifact>> {
    recover_lock(store()).results.remove(&ticket)
}

/// Whether `window_id` has captures waiting for a redraw.
pub fn has_pending_window_debug_captures(window_id: WindowId) -> bool {
    recover_lock(store())
        .pending
        .get(&window_id)
        .is_some_and(|pending| !pending.is_empty())
}

/// Make every capture waiting for `window_id` from the frame `renderer` last
/// drew for it, and return the wake tokens to deliver with
/// `Runtime::wake_async`.
pub fn service_window_debug_captures(
    renderer: &mut WgpuRenderer,
    window_id: WindowId,
) -> Vec<AsyncWakeToken> {
    service_with(window_id, |request| {
        crate::capture_window_for_app(renderer, window_id, request)
    })
}

/// Make every capture waiting for `window_id` with `capture`, keep the
/// results, and return the wake tokens to deliver.
fn service_with(
    window_id: WindowId,
    mut capture: impl FnMut(DebugCaptureRequest) -> Result<DebugCaptureArtifact>,
) -> Vec<AsyncWakeToken> {
    let pending = recover_lock(store())
        .pending
        .remove(&window_id)
        .unwrap_or_default();
    if pending.is_empty() {
        return Vec::new();
    }
    let finished = pending
        .into_iter()
        .map(|pending| (pending.ticket, capture(pending.request), pending.wake))
        .collect::<Vec<_>>();
    let mut store = recover_lock(store());
    finished
        .into_iter()
        .filter_map(|(ticket, result, wake)| {
            store.results.insert(ticket, result);
            wake
        })
        .collect()
}

/// Forget captures for a window that closed.
pub(crate) fn clear_window_debug_captures(window_id: WindowId) {
    recover_lock(store()).pending.remove(&window_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_core::Error;
    use sui_render_wgpu::RgbaImage;

    #[test]
    fn captures_wait_for_a_redraw_and_are_handed_out_once() {
        let window = WindowId::new(90_001);
        let other = WindowId::new(90_002);
        let wake = AsyncWakeToken::new(7);
        let first =
            request_window_debug_capture(window, DebugCaptureRequest::default(), Some(wake));
        let second = request_window_debug_capture(window, DebugCaptureRequest::default(), None);
        assert!(has_pending_window_debug_captures(window));
        assert!(take_window_debug_capture(first).is_none());

        assert!(service_with(other, |_| unreachable!()).is_empty());
        let mut made = 0;
        let wakes = service_with(window, |_| {
            made += 1;
            if made == 1 {
                RgbaImage::new(1, 1, vec![0, 0, 0, 255]).map(DebugCaptureArtifact::SdrRgba8)
            } else {
                Err(Error::new("no frame yet"))
            }
        });
        assert_eq!(made, 2);
        assert_eq!(wakes, vec![wake]);
        assert!(!has_pending_window_debug_captures(window));

        assert!(matches!(
            take_window_debug_capture(first),
            Some(Ok(DebugCaptureArtifact::SdrRgba8(_)))
        ));
        assert!(take_window_debug_capture(first).is_none());
        assert!(matches!(take_window_debug_capture(second), Some(Err(_))));
    }
}
