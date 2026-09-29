//! How far each window's output reaches beyond sRGB and SDR white, for
//! widgets whose styling depends on it (HDR theme modes). The platform sets
//! it before rendering each frame.

use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

use sui_core::WindowId;
use sui_reactive::Signal;

/// How far a window's output reaches beyond sRGB and SDR white.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutputColorRange {
    /// sRGB colors up to SDR white.
    Standard,
    /// Colors beyond sRGB, up to SDR white.
    WideGamut,
    /// Light beyond SDR white, and colors beyond sRGB where the display has
    /// them.
    HighDynamicRange,
}

type RangeSignal = Signal<Option<OutputColorRange>>;

fn store() -> &'static RwLock<HashMap<WindowId, RangeSignal>> {
    static STORE: OnceLock<RwLock<HashMap<WindowId, RangeSignal>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Record what `window_id`'s output can show. Widgets observing
/// [`window_output_color_range_signal`] are invalidated when it changes.
pub fn set_window_output_color_range(window_id: WindowId, range: OutputColorRange) {
    // Set outside the lock: observers run when the value changes.
    window_output_color_range_signal(window_id).set(Some(range));
}

/// What `window_id`'s output can show, once the platform has said.
pub fn window_output_color_range(window_id: WindowId) -> Option<OutputColorRange> {
    store()
        .read()
        .expect("output color range store lock should not be poisoned")
        .get(&window_id)
        .and_then(Signal::get)
}

/// What `window_id`'s output can show, as a signal to observe. It holds
/// `None` until the platform has said, as it does without a platform.
pub fn window_output_color_range_signal(window_id: WindowId) -> RangeSignal {
    if let Some(signal) = store()
        .read()
        .expect("output color range store lock should not be poisoned")
        .get(&window_id)
    {
        return signal.clone();
    }
    store()
        .write()
        .expect("output color range store lock should not be poisoned")
        .entry(window_id)
        .or_insert_with(|| Signal::named("Window output color range", None))
        .clone()
}

pub(crate) fn clear_window_output_color_range(window_id: WindowId) {
    let removed = store()
        .write()
        .expect("output color range store lock should not be poisoned")
        .remove(&window_id);
    if let Some(signal) = removed {
        signal.set(None);
    }
}

pub(crate) fn clear_window_output_color_ranges() {
    let removed = std::mem::take(
        &mut *store()
            .write()
            .expect("output color range store lock should not be poisoned"),
    );
    for signal in removed.into_values() {
        signal.set(None);
    }
}
