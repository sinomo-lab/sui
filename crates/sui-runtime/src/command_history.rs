//! The commands each window recently delivered, as a signal in-app tools can
//! observe. The inspector and the performance snapshot keep the same
//! samples; this history is for widgets, which cannot reach either.

use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

use sui_core::WindowId;
use sui_reactive::Signal;

use crate::diagnostics::CommandDispatchSample;

/// How many dispatches each window's history keeps.
pub const COMMAND_HISTORY_LENGTH: usize = 64;

type HistorySignal = Signal<Vec<CommandDispatchSample>>;

fn store() -> &'static RwLock<HashMap<WindowId, HistorySignal>> {
    static STORE: OnceLock<RwLock<HashMap<WindowId, HistorySignal>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The commands `window_id` delivered most recently, oldest first, up to
/// [`COMMAND_HISTORY_LENGTH`] of them. Each sample is one attempt to deliver
/// a command in the window: to a widget, to the focused widget, or to the
/// window's or the application's listeners, with the handlers that ran and
/// whether one handled it. Observe it to follow commands as they arrive.
pub fn window_command_dispatches_signal(window_id: WindowId) -> HistorySignal {
    if let Some(signal) = store()
        .read()
        .expect("command history store lock should not be poisoned")
        .get(&window_id)
    {
        return signal.clone();
    }
    store()
        .write()
        .expect("command history store lock should not be poisoned")
        .entry(window_id)
        .or_insert_with(|| Signal::named("Window command dispatches", Vec::new()))
        .clone()
}

pub(crate) fn record_window_command_dispatch(window_id: WindowId, sample: CommandDispatchSample) {
    // Updated outside the lock: observers run when the history changes.
    window_command_dispatches_signal(window_id).update(|history| {
        if history.len() == COMMAND_HISTORY_LENGTH {
            history.remove(0);
        }
        history.push(sample);
    });
}

pub(crate) fn clear_window_command_dispatches(window_id: WindowId) {
    let removed = store()
        .write()
        .expect("command history store lock should not be poisoned")
        .remove(&window_id);
    if let Some(signal) = removed {
        signal.set(Vec::new());
    }
}
