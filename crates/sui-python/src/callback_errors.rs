//! Reporting for exceptions raised by Python callbacks.
//!
//! Widget, state, message, and posted-task callbacks run on the UI thread
//! with no Python caller to receive their exceptions. Every such failure is
//! reported here: through the handler installed with
//! `set_exception_handler`, or `sys.excepthook` by default.
//!
//! `KeyboardInterrupt` and `SystemExit` are not reported. They stop the
//! running event loop and are re-raised by the call that drives it:
//! `App.run`, `App.run_with_handle`, or a host-driven `RunningApp` method.

use pyo3::exceptions::{PyKeyboardInterrupt, PySystemExit, PyTypeError};
use pyo3::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use sui_bindings_core::BindingUiHandle;

use crate::recover_lock;

/// How often a running desktop loop checks for pending Python signals.
const SIGNAL_POLL_INTERVAL: Duration = Duration::from_millis(200);

static EXCEPTION_HANDLER: Mutex<Option<Py<PyAny>>> = Mutex::new(None);
static PENDING_EXIT: Mutex<Option<PyErr>> = Mutex::new(None);
static ACTIVE_LOOP: Mutex<Option<BindingUiHandle>> = Mutex::new(None);

/// Report an exception raised by a Python callback.
pub(crate) fn report_callback_error(py: Python<'_>, error: PyErr) {
    if stop_on_exit_exception(py, error.clone_ref(py)) {
        return;
    }
    let handler = recover_lock(&EXCEPTION_HANDLER)
        .as_ref()
        .map(|handler| handler.clone_ref(py));
    let Some(handler) = handler else {
        report_to_excepthook(py, error);
        return;
    };
    // `into_value` attaches the traceback as `__traceback__`; `value` alone
    // leaves it unset.
    let exception = error.clone_ref(py).into_value(py);
    if let Err(handler_error) = handler.call1(py, (exception,)) {
        // A failing handler must not hide the original exception.
        report_to_excepthook(py, error);
        if !stop_on_exit_exception(py, handler_error.clone_ref(py)) {
            report_to_excepthook(py, handler_error);
        }
    }
}

/// Raise a `KeyboardInterrupt` or `SystemExit` that a callback raised since
/// the last check.
pub(crate) fn raise_pending_exit() -> PyResult<()> {
    match recover_lock(&PENDING_EXIT).take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Track the event loop that exit exceptions should stop. `None` clears it.
pub(crate) fn set_active_loop(handle: Option<BindingUiHandle>) {
    *recover_lock(&ACTIVE_LOOP) = handle;
}

/// Check for pending Python signals, such as Ctrl+C, while a desktop loop
/// runs. Python only runs signal handlers on the main thread, which is the
/// UI thread here, so the poller posts quiet tasks that call
/// `check_signals` there. Dropping the returned guard stops the poller.
pub(crate) fn start_signal_poller(handle: BindingUiHandle) -> SignalPoller {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let in_flight = Arc::new(AtomicBool::new(false));
    thread::spawn(move || {
        loop {
            thread::sleep(SIGNAL_POLL_INTERVAL);
            if stop_for_thread.load(Ordering::Acquire) {
                break;
            }
            if in_flight.swap(true, Ordering::AcqRel) {
                continue;
            }
            let in_flight = Arc::clone(&in_flight);
            handle.post_quiet(move || {
                Python::attach(|py| {
                    if let Err(error) = py.check_signals() {
                        report_callback_error(py, error);
                    }
                });
                in_flight.store(false, Ordering::Release);
            });
        }
    });
    SignalPoller { stop }
}

pub(crate) struct SignalPoller {
    stop: Arc<AtomicBool>,
}

impl Drop for SignalPoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// Keep the first exit exception and stop the active loop. Returns `false`
/// for every other exception.
fn stop_on_exit_exception(py: Python<'_>, error: PyErr) -> bool {
    if !error.is_instance_of::<PyKeyboardInterrupt>(py) && !error.is_instance_of::<PySystemExit>(py)
    {
        return false;
    }
    {
        let mut pending = recover_lock(&PENDING_EXIT);
        if pending.is_none() {
            *pending = Some(error);
        }
    }
    let active = recover_lock(&ACTIVE_LOOP).clone();
    if let Some(active) = active {
        active.request_exit();
    }
    true
}

fn report_to_excepthook(py: Python<'_>, error: PyErr) {
    let reported = py.import("sys").and_then(|sys| {
        sys.getattr("excepthook")?
            .call1((error.get_type(py), error.value(py), error.traceback(py)))
    });
    if reported.is_err() {
        // `sys.excepthook` is missing or failed; print without it.
        error.display(py);
    }
}

/// Install a handler for exceptions raised by callbacks.
///
/// The handler receives the exception object; its traceback is available as
/// `__traceback__`. Pass `None` to restore the default, which reports through
/// `sys.excepthook`. Returns the previous handler.
#[pyfunction]
#[pyo3(signature = (handler))]
pub(crate) fn set_exception_handler(
    py: Python<'_>,
    handler: Option<Py<PyAny>>,
) -> PyResult<Option<Py<PyAny>>> {
    if let Some(handler) = &handler
        && !handler.bind(py).is_callable()
    {
        return Err(PyTypeError::new_err(
            "exception handler must be callable or None",
        ));
    }
    Ok(std::mem::replace(
        &mut *recover_lock(&EXCEPTION_HANDLER),
        handler,
    ))
}
