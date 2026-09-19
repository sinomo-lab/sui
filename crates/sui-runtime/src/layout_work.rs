//! Opt-in, thread-local work counters for benchmark/diagnostic replays.
//! Calls compile away unless the layout-diagnostics feature is enabled.
use std::time::Duration;
use web_time::Instant;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LayoutWorkSnapshot {
    pub constructed: u64,
    pub dropped: u64,
    pub measure_requests: u64,
    pub measure_executions: u64,
    pub size_measure_executions: u64,
    pub measure_cache_hits: u64,
    pub forced_measures: u64,
    pub constraint_changes: u64,
    pub first_measures: u64,
    pub probe_requests: u64,
    pub probe_cache_hits: u64,
    pub shared_probe_cache_hits: u64,
    pub intrinsic_executions: u64,
    pub intrinsic_cache_hits: u64,
    pub intrinsic_horizontal: u64,
    pub intrinsic_vertical: u64,
    pub arrange_requests: u64,
    pub arrange_executions: u64,
    pub arrange_cache_hits: u64,
    pub translations: u64,
    pub paint_executions: u64,
    pub semantics_executions: u64,
    pub layout_passes: u64,
    pub gutter_iterations: u64,
    pub measure_us: f64,
    pub arrange_us: f64,
    pub graph_us: f64,
}

/// Instrumentation seam used by built-in scrollbar layout.
#[doc(hidden)]
#[inline]
pub fn record_scrollbar_gutter_iteration() {
    record(|work| work.gutter_iterations += 1);
}

#[cfg(feature = "layout-diagnostics")]
thread_local! {
    static WORK: std::cell::RefCell<Option<LayoutWorkSnapshot>> = const { std::cell::RefCell::new(None) };
}

/// Start a collector on the current UI thread. No per-widget allocation occurs.
/// Returns false when this build does not include layout-diagnostics.
pub fn begin_layout_work_collection() -> bool {
    #[cfg(feature = "layout-diagnostics")]
    {
        WORK.with(|work| *work.borrow_mut() = Some(LayoutWorkSnapshot::default()));
        true
    }
    #[cfg(not(feature = "layout-diagnostics"))]
    {
        false
    }
}

pub fn take_layout_work_collection() -> Option<LayoutWorkSnapshot> {
    #[cfg(feature = "layout-diagnostics")]
    {
        WORK.with(|work| work.borrow_mut().take())
    }
    #[cfg(not(feature = "layout-diagnostics"))]
    {
        None
    }
}

#[inline]
pub(crate) fn record(update: impl FnOnce(&mut LayoutWorkSnapshot)) {
    #[cfg(feature = "layout-diagnostics")]
    WORK.with(|work| {
        if let Some(work) = work.borrow_mut().as_mut() {
            update(work);
        }
    });
    #[cfg(not(feature = "layout-diagnostics"))]
    let _ = update;
}

#[inline]
pub(crate) fn started() -> Option<Instant> {
    #[cfg(feature = "layout-diagnostics")]
    {
        WORK.with(|work| work.borrow().is_some().then(Instant::now))
    }
    #[cfg(not(feature = "layout-diagnostics"))]
    {
        None
    }
}

#[inline]
pub(crate) fn elapsed(start: Option<Instant>, update: impl FnOnce(&mut LayoutWorkSnapshot, f64)) {
    if let Some(start) = start {
        let elapsed: Duration = start.elapsed();
        record(|work| update(work, elapsed.as_secs_f64() * 1e6));
    }
}

#[cfg(all(test, feature = "layout-diagnostics"))]
mod tests {
    use super::*;
    #[test]
    fn collection_is_scoped_resettable_and_thread_local() {
        assert!(begin_layout_work_collection());
        record(|work| work.constructed += 2);
        assert!(
            std::thread::spawn(take_layout_work_collection)
                .join()
                .unwrap()
                .is_none()
        );
        assert_eq!(take_layout_work_collection().unwrap().constructed, 2);
        assert!(take_layout_work_collection().is_none());
        record(|work| work.constructed += 10);
        begin_layout_work_collection();
        assert_eq!(
            take_layout_work_collection().unwrap(),
            LayoutWorkSnapshot::default()
        );
    }
}
