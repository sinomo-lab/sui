use std::{cell::RefCell, collections::HashSet, sync::Arc};

use crate::{SignalCore, SourceId};

#[derive(Default)]
struct BatchState {
    depth: usize,
    queued: HashSet<SourceId>,
    pending: Vec<Arc<SignalCore>>,
}

thread_local! {
    static BATCH: RefCell<BatchState> = RefCell::new(BatchState::default());
}

/// Run `writes` and notify each changed [`Signal`](crate::Signal) once when it
/// returns, instead of after every write.
///
/// Values and versions update immediately, so reads inside the batch see the
/// new state. Notifications are deferred only for writes made on the calling
/// thread; other threads are unaffected. Nested batches flush when the
/// outermost one ends. Each deferred notification carries the signal's
/// latest version, and is delivered even if `writes` panics, because the
/// values have already changed.
///
/// ```ignore
/// std::thread::spawn(move || {
///     let rows = load_rows();
///     sui_reactive::batch(|| {
///         rows_signal.set(rows);
///         status.set(Status::Loaded);
///         selection.set(None);
///     }); // observers of each signal run once, here, on this thread
/// });
/// ```
pub fn batch<R>(writes: impl FnOnce() -> R) -> R {
    BATCH.with(|batch| batch.borrow_mut().depth += 1);
    let _flush = FlushOnDrop;
    writes()
}

struct FlushOnDrop;

impl Drop for FlushOnDrop {
    fn drop(&mut self) {
        let pending = BATCH.with(|batch| {
            let mut batch = batch.borrow_mut();
            batch.depth -= 1;
            if batch.depth > 0 {
                return Vec::new();
            }
            batch.queued.clear();
            std::mem::take(&mut batch.pending)
        });
        // The batch is closed, so writes from observers notify immediately.
        for core in pending {
            core.deliver();
        }
    }
}

/// Queue `core` if this thread is inside a batch. Returns `false` when the
/// caller should deliver immediately.
pub(crate) fn defer(core: &Arc<SignalCore>) -> bool {
    BATCH
        .try_with(|batch| {
            let mut batch = batch.borrow_mut();
            if batch.depth == 0 {
                return false;
            }
            if batch.queued.insert(core.source_id()) {
                batch.pending.push(Arc::clone(core));
            }
            true
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::{Observable, Observer, Signal, batch};

    #[test]
    fn batch_notifies_each_signal_once_with_latest_version() {
        let first = Signal::new(0_u32);
        let second = Signal::new(0_u32);
        let log = Arc::new(Mutex::new(Vec::new()));
        let first_log = Arc::clone(&log);
        let second_log = Arc::clone(&log);
        let _first = first.subscribe(Observer::new(move |change| {
            first_log.lock().unwrap().push(("first", change.version));
        }));
        let _second = second.subscribe(Observer::new(move |change| {
            second_log.lock().unwrap().push(("second", change.version));
        }));

        batch(|| {
            first.set(1);
            second.set(1);
            first.set(2);
            batch(|| first.set(3));
            assert_eq!(first.get(), 3, "reads see writes inside the batch");
            assert!(log.lock().unwrap().is_empty(), "nested batch did not flush");
        });

        assert_eq!(*log.lock().unwrap(), vec![("first", 3), ("second", 1)]);
    }

    #[test]
    fn selector_over_batched_signal_recomputes_once() {
        let state = Signal::new((0_u32, 0_u32));
        let sum = state.select(|(a, b)| a + b);
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let _subscription = sum.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));

        batch(|| {
            state.update(|state| state.0 = 1);
            state.update(|state| state.1 = 2);
        });
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(sum.get(), 3);
    }

    #[test]
    fn batch_only_defers_the_calling_thread() {
        let signal = Signal::new(0_u32);
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let _subscription = signal.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));
        batch(|| {
            let writer = signal.clone();
            std::thread::spawn(move || writer.set(1)).join().unwrap();
            assert_eq!(notifications.load(Ordering::Relaxed), 1);
        });
    }

    #[test]
    fn batch_flushes_after_a_panic() {
        let signal = Signal::new(0_u32);
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let _subscription = signal.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            batch(|| {
                signal.set(1);
                panic!("worker failed");
            })
        }));
        assert!(result.is_err());
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        // The batch closed during unwinding; later writes notify immediately.
        signal.set(2);
        assert_eq!(notifications.load(Ordering::Relaxed), 2);
    }
}
