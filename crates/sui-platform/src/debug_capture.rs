//! Debug captures requested by the running app.
//!
//! An app asks for a capture of a window with
//! [`request_window_debug_capture`]. After its next redraw the platform
//! renders the window's last frame again offscreen at the requested stage
//! and starts copying it back. Native platforms wait for the copy; browsers,
//! which cannot wait, collect it on a later redraw. Either way the artifact
//! is kept for the app to collect with [`take_window_debug_capture`], and if
//! the request carried an [`AsyncWakeToken`], the widget that registered it
//! is woken then.
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
use sui_render_wgpu::{DebugCaptureArtifact, DebugCaptureId, DebugCaptureRequest, WgpuRenderer};

/// Identifies a requested capture until its result is collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugCaptureTicket(u64);

struct PendingCapture {
    ticket: DebugCaptureTicket,
    request: DebugCaptureRequest,
    wake: Option<AsyncWakeToken>,
}

/// A capture the renderer began and is copying back.
struct InFlightCapture {
    ticket: DebugCaptureTicket,
    wake: Option<AsyncWakeToken>,
}

#[derive(Default)]
struct CaptureStore {
    pending: HashMap<WindowId, Vec<PendingCapture>>,
    in_flight: HashMap<(WindowId, DebugCaptureId), InFlightCapture>,
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

/// Whether `window_id` has captures waiting for a redraw or being copied
/// back.
pub fn has_pending_window_debug_captures(window_id: WindowId) -> bool {
    let store = recover_lock(store());
    store
        .pending
        .get(&window_id)
        .is_some_and(|pending| !pending.is_empty())
        || store
            .in_flight
            .keys()
            .any(|(window, _)| *window == window_id)
}

/// Begin every capture waiting for `window_id` from the frame `renderer` last
/// drew for it, collect the ones that are back, and return the wake tokens to
/// deliver with `Runtime::wake_async`.
pub fn service_window_debug_captures(
    renderer: &mut WgpuRenderer,
    window_id: WindowId,
) -> Vec<AsyncWakeToken> {
    service_with(renderer, window_id)
}

/// What makes captures: the renderer, or a stand-in in tests.
trait CaptureSource {
    fn begin(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureId>;

    /// The captures of `window_id` that are back.
    fn finished(
        &mut self,
        window_id: WindowId,
    ) -> Vec<(DebugCaptureId, Result<DebugCaptureArtifact>)>;
}

impl CaptureSource for WgpuRenderer {
    fn begin(
        &mut self,
        window_id: WindowId,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureId> {
        self.begin_debug_capture(window_id, request)
    }

    fn finished(
        &mut self,
        window_id: WindowId,
    ) -> Vec<(DebugCaptureId, Result<DebugCaptureArtifact>)> {
        // Native devices can wait for the copies. Browsers cannot, so their
        // captures come back on a later redraw, as they do here if waiting
        // fails.
        #[cfg(not(target_arch = "wasm32"))]
        if self.has_debug_captures_in_flight(window_id) {
            let _ = self.wait_for_debug_captures();
        }
        self.take_finished_debug_captures(window_id)
    }
}

/// Begin every capture waiting for `window_id` with `source`, keep the
/// results that are back, and return the wake tokens to deliver.
fn service_with(source: &mut impl CaptureSource, window_id: WindowId) -> Vec<AsyncWakeToken> {
    let pending = recover_lock(store())
        .pending
        .remove(&window_id)
        .unwrap_or_default();
    let mut finished = Vec::new();
    let mut began = Vec::new();
    for pending in pending {
        match source.begin(window_id, pending.request) {
            Ok(id) => began.push((
                id,
                InFlightCapture {
                    ticket: pending.ticket,
                    wake: pending.wake,
                },
            )),
            Err(error) => finished.push((pending.ticket, Err(error), pending.wake)),
        }
    }
    let in_flight = {
        let mut store = recover_lock(store());
        for (id, capture) in began {
            store.in_flight.insert((window_id, id), capture);
        }
        store
            .in_flight
            .keys()
            .any(|(window, _)| *window == window_id)
    };
    let back = if in_flight {
        source.finished(window_id)
    } else {
        Vec::new()
    };

    let mut store = recover_lock(store());
    for (id, result) in back {
        if let Some(capture) = store.in_flight.remove(&(window_id, id)) {
            finished.push((capture.ticket, result, capture.wake));
        }
    }
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
    let mut store = recover_lock(store());
    store.pending.remove(&window_id);
    store
        .in_flight
        .retain(|(window, _), _| *window != window_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_core::Error;
    use sui_render_wgpu::RgbaImage;

    /// Makes captures that come back after `delay` services, the first
    /// succeeding and the rest failing.
    #[derive(Default)]
    struct StandIn {
        delay: usize,
        began: Vec<(DebugCaptureId, usize)>,
        services: usize,
    }

    impl CaptureSource for StandIn {
        fn begin(
            &mut self,
            _window_id: WindowId,
            _request: DebugCaptureRequest,
        ) -> Result<DebugCaptureId> {
            let id = DebugCaptureId::new(self.began.len() as u64 + 1);
            self.began.push((id, self.services));
            Ok(id)
        }

        fn finished(
            &mut self,
            _window_id: WindowId,
        ) -> Vec<(DebugCaptureId, Result<DebugCaptureArtifact>)> {
            self.services += 1;
            let (ready, waiting) = std::mem::take(&mut self.began)
                .into_iter()
                .partition::<Vec<_>, _>(|(_, began)| self.services - began > self.delay);
            self.began = waiting;
            ready
                .into_iter()
                .map(|(id, _)| {
                    let artifact = if id.get() == 1 {
                        RgbaImage::new(1, 1, vec![0, 0, 0, 255]).map(DebugCaptureArtifact::SdrRgba8)
                    } else {
                        Err(Error::new("no frame yet"))
                    };
                    (id, artifact)
                })
                .collect()
        }
    }

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

        let mut source = StandIn::default();
        assert!(service_with(&mut source, other).is_empty());
        let wakes = service_with(&mut source, window);
        assert_eq!(wakes, vec![wake]);
        assert!(!has_pending_window_debug_captures(window));

        assert!(matches!(
            take_window_debug_capture(first),
            Some(Ok(DebugCaptureArtifact::SdrRgba8(_)))
        ));
        assert!(take_window_debug_capture(first).is_none());
        assert!(matches!(take_window_debug_capture(second), Some(Err(_))));
    }

    #[test]
    fn captures_copied_back_later_stay_pending_until_they_are_back() {
        let window = WindowId::new(90_003);
        let wake = AsyncWakeToken::new(8);
        let ticket =
            request_window_debug_capture(window, DebugCaptureRequest::default(), Some(wake));

        // Like a browser: the copy is not back on the redraw that began it.
        let mut source = StandIn {
            delay: 1,
            ..StandIn::default()
        };
        assert!(service_with(&mut source, window).is_empty());
        assert!(has_pending_window_debug_captures(window));
        assert!(take_window_debug_capture(ticket).is_none());

        assert_eq!(service_with(&mut source, window), vec![wake]);
        assert!(!has_pending_window_debug_captures(window));
        assert!(matches!(
            take_window_debug_capture(ticket),
            Some(Ok(DebugCaptureArtifact::SdrRgba8(_)))
        ));
    }
}
