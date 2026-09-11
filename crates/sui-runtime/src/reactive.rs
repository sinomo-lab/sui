use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, RwLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use sui_core::{InvalidationKind, InvalidationRequest, InvalidationTarget, WidgetId, WindowId};
use sui_reactive::{Change, Observable, Observer, SourceId, Subscription};

use crate::ReactiveInvalidationSample;

type ExternalWaker = dyn Fn() + Send + Sync + 'static;

struct PendingReactiveInvalidation {
    request: InvalidationRequest,
    sample: ReactiveInvalidationSample,
}

pub(crate) struct ReactiveInvalidationHub {
    pending: Mutex<Vec<PendingReactiveInvalidation>>,
    wake_pending: AtomicBool,
    waker: RwLock<Option<Arc<ExternalWaker>>>,
}

impl ReactiveInvalidationHub {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            pending: Mutex::new(Vec::new()),
            wake_pending: AtomicBool::new(false),
            waker: RwLock::new(None),
        })
    }

    pub(crate) fn set_waker(&self, waker: Option<Arc<ExternalWaker>>) {
        *self
            .waker
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = waker;
    }

    fn enqueue(&self, widget_id: WidgetId, kind: InvalidationKind, change: Change) {
        let should_wake = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let sample = ReactiveInvalidationSample {
                widget_id,
                source_id: change.source_id,
                source_name: change.source_name.to_string(),
                version: change.version,
                kind,
                delivered: true,
            };
            if let Some(existing) = pending.iter_mut().find(|pending| {
                pending.sample.widget_id == widget_id
                    && pending.sample.source_id == change.source_id
                    && pending.sample.kind == kind
            }) {
                existing.sample = sample;
            } else {
                pending.push(PendingReactiveInvalidation {
                    request: InvalidationRequest::new(InvalidationTarget::Widget(widget_id), kind),
                    sample,
                });
            }
            !self.wake_pending.swap(true, Ordering::AcqRel)
        };

        if should_wake
            && let Some(waker) = self
                .waker
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        {
            waker();
        }
    }

    pub(crate) fn drain(&self) -> Vec<(InvalidationRequest, ReactiveInvalidationSample)> {
        let pending = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let pending = std::mem::take(&mut *pending);
            self.wake_pending.store(false, Ordering::Release);
            pending
        };
        pending
            .into_iter()
            .map(|pending| (pending.request, pending.sample))
            .collect()
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }
}

fn window_hubs() -> &'static RwLock<HashMap<WindowId, Weak<ReactiveInvalidationHub>>> {
    static WINDOW_HUBS: OnceLock<RwLock<HashMap<WindowId, Weak<ReactiveInvalidationHub>>>> =
        OnceLock::new();
    WINDOW_HUBS.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WidgetSubscriptionKey {
    window_id: WindowId,
    source_id: SourceId,
    kind: InvalidationKind,
}

#[derive(Clone, Copy)]
pub(crate) enum ObservationPhase {
    Measure,
    IntrinsicHorizontal,
    IntrinsicVertical,
    Arrange,
    Paint,
    Semantics,
}

impl ObservationPhase {
    fn bit(self) -> u8 {
        1 << self as u8
    }
}

// Event/command observers are explicit registrations rather than dependencies
// of a replayable frame phase. Keep them until the widget is dropped.
const EXPLICIT_OBSERVATION: u8 = 1 << 6;

struct WidgetSubscription {
    _subscription: Subscription,
    phases: u8,
}

struct ObservationFrame {
    window_id: WindowId,
    widget_id: WidgetId,
    phase: u8,
    reads: Vec<WidgetSubscriptionKey>,
}

thread_local! {
    static OBSERVATIONS: RefCell<Vec<ObservationFrame>> = const { RefCell::new(Vec::new()) };
}

/// Reconcile only a phase that actually executed. Cached measure/arrange and
/// retained paint paths never start a scope, so their dependencies stay live.
pub(crate) struct ObservationScope<'a> {
    observed_phases: &'a Cell<u8>,
}

impl<'a> ObservationScope<'a> {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        phase: ObservationPhase,
        observed_phases: &'a Cell<u8>,
    ) -> Self {
        OBSERVATIONS.with(|frames| {
            frames.borrow_mut().push(ObservationFrame {
                window_id,
                widget_id,
                phase: phase.bit(),
                reads: Vec::new(),
            })
        });
        Self { observed_phases }
    }
}

impl Drop for ObservationScope<'_> {
    fn drop(&mut self) {
        let frame =
            OBSERVATIONS.with(|frames| frames.borrow_mut().pop().expect("observation scope"));
        let previous = self.observed_phases.get();
        if !frame.reads.is_empty() {
            self.observed_phases.set(previous | frame.phase);
        }
        if std::thread::panicking() {
            return;
        }
        if frame.reads.is_empty() {
            self.observed_phases.set(previous & !frame.phase);
            if previous & frame.phase == 0 {
                return; // No dependency registry work for non-observing widgets.
            }
        }
        let removed = {
            let mut all = widget_subscriptions()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(subscriptions) = all.get_mut(&frame.widget_id) else {
                return;
            };
            let mut stale = Vec::new();
            for (key, subscription) in subscriptions.iter_mut() {
                if key.window_id == frame.window_id && !frame.reads.contains(key) {
                    subscription.phases &= !frame.phase;
                }
                if subscription.phases == 0 {
                    stale.push(*key);
                }
            }
            let removed = stale
                .into_iter()
                .filter_map(|key| subscriptions.remove(&key))
                .collect::<Vec<_>>();
            if subscriptions.is_empty() {
                all.remove(&frame.widget_id);
            }
            removed
        };
        // Observer closures can own user values with destructors. Drop them
        // outside the registry lock, just like invoking callbacks.
        drop(removed);
    }
}

fn record_observation(key: WidgetSubscriptionKey, widget_id: WidgetId) -> u8 {
    OBSERVATIONS.with(|frames| {
        let mut frames = frames.borrow_mut();
        let Some(frame) = frames
            .iter_mut()
            .rev()
            .find(|frame| frame.window_id == key.window_id && frame.widget_id == widget_id)
        else {
            return EXPLICIT_OBSERVATION;
        };
        if !frame.reads.contains(&key) {
            frame.reads.push(key);
        }
        frame.phase
    })
}

fn widget_subscriptions()
-> &'static Mutex<HashMap<WidgetId, HashMap<WidgetSubscriptionKey, WidgetSubscription>>> {
    static WIDGET_SUBSCRIPTIONS: OnceLock<
        Mutex<HashMap<WidgetId, HashMap<WidgetSubscriptionKey, WidgetSubscription>>>,
    > = OnceLock::new();
    WIDGET_SUBSCRIPTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn register_window(window_id: WindowId, hub: &Arc<ReactiveInvalidationHub>) {
    window_hubs()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(window_id, Arc::downgrade(hub));
}

pub(crate) fn unregister_window(window_id: WindowId) {
    window_hubs()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&window_id);
}

pub(crate) fn clear_widget(widget_id: WidgetId) {
    let removed = widget_subscriptions()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&widget_id);
    drop(removed);
}

#[cfg(test)]
pub(crate) fn subscription_count(widget_id: WidgetId) -> usize {
    widget_subscriptions()
        .lock()
        .unwrap()
        .get(&widget_id)
        .map_or(0, HashMap::len)
}

pub(crate) fn observe<T, O>(
    window_id: WindowId,
    widget_id: WidgetId,
    observable: &O,
    kind: InvalidationKind,
) -> T
where
    O: Observable<T> + ?Sized,
{
    let key = WidgetSubscriptionKey {
        window_id,
        source_id: observable.source_id(),
        kind,
    };
    let phase = record_observation(key, widget_id);
    let subscribed = {
        let mut all_subscriptions = widget_subscriptions()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let subscriptions = all_subscriptions.entry(widget_id).or_default();
        subscriptions.retain(|candidate, _| {
            candidate.window_id == window_id
                || candidate.source_id != key.source_id
                || candidate.kind != kind
        });
        if let Some(subscription) = subscriptions.get_mut(&key) {
            subscription.phases |= phase;
            true
        } else {
            false
        }
    };

    if !subscribed {
        let hub = window_hubs()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&window_id)
            .and_then(Weak::upgrade);
        if let Some(hub) = hub {
            let weak_hub = Arc::downgrade(&hub);
            let subscription = observable.subscribe(Observer::new(move |change| {
                if let Some(hub) = weak_hub.upgrade() {
                    hub.enqueue(widget_id, kind, change);
                }
            }));
            widget_subscriptions()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .entry(widget_id)
                .or_default()
                .entry(key)
                .and_modify(|existing| existing.phases |= phase)
                .or_insert(WidgetSubscription {
                    _subscription: subscription,
                    phases: phase,
                });
        }
    }

    observable.get()
}
