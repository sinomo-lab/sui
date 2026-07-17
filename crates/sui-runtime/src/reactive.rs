use std::{
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

fn widget_subscriptions()
-> &'static Mutex<HashMap<WidgetId, HashMap<WidgetSubscriptionKey, Subscription>>> {
    static WIDGET_SUBSCRIPTIONS: OnceLock<
        Mutex<HashMap<WidgetId, HashMap<WidgetSubscriptionKey, Subscription>>>,
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
    widget_subscriptions()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&widget_id);
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
        subscriptions.contains_key(&key)
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
                .or_insert(subscription);
        }
    }

    observable.get()
}
