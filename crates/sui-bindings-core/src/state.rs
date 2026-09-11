use crate::messages::BindingValue;
use crate::support::recover_lock;
use crate::tasks::BindingUiHandle;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone)]
pub struct BindingState {
    pub(crate) inner: Arc<BindingStateInner>,
}

#[derive(Debug)]
pub(crate) struct BindingStateInner {
    pub(crate) value: Mutex<BindingValue>,
    pub(crate) ui_handle: Mutex<Option<BindingUiHandle>>,
    pub(crate) observers: Mutex<BTreeMap<u64, BindingStateObserver>>,
    pub(crate) next_observer_id: AtomicU64,
    pub(crate) retained_subscriptions: Mutex<Vec<BindingStateSubscription>>,
}

#[derive(Clone)]
pub(crate) struct BindingStateObserver {
    pub(crate) callback: Arc<dyn Fn(BindingValue) + Send + Sync + 'static>,
}

impl fmt::Debug for BindingStateObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingStateObserver")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct BindingStateSubscription {
    pub(crate) source: Weak<BindingStateInner>,
    pub(crate) id: u64,
    pub(crate) active: bool,
}

impl BindingStateSubscription {
    pub fn unsubscribe(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.source
            .upgrade()
            .is_some_and(|source| recover_lock(&source.observers).remove(&self.id).is_some())
    }
}

impl Drop for BindingStateSubscription {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

impl BindingState {
    pub fn new(value: impl Into<BindingValue>) -> Self {
        Self {
            inner: Arc::new(BindingStateInner {
                value: Mutex::new(value.into()),
                ui_handle: Mutex::new(None),
                observers: Mutex::new(BTreeMap::new()),
                next_observer_id: AtomicU64::new(1),
                retained_subscriptions: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn get(&self) -> BindingValue {
        recover_lock(&self.inner.value).clone()
    }

    pub fn set(&self, value: impl Into<BindingValue>) {
        let value = value.into();
        if let Some(handle) = recover_lock(&self.inner.ui_handle).clone()
            && !handle.is_draining()
        {
            let state = self.clone();
            handle.post(move || state.set_immediate(value));
        } else {
            self.set_immediate(value);
        }
    }

    pub fn label_text(&self) -> String {
        self.get().as_label_text()
    }

    pub fn bind_ui_handle(&self, handle: BindingUiHandle) {
        *recover_lock(&self.inner.ui_handle) = Some(handle);
    }

    pub fn unbind_ui_handle(&self) {
        *recover_lock(&self.inner.ui_handle) = None;
    }

    pub fn is_ui_bound(&self) -> bool {
        recover_lock(&self.inner.ui_handle).is_some()
    }

    pub fn observe(
        &self,
        callback: impl Fn(BindingValue) + Send + Sync + 'static,
    ) -> BindingStateSubscription {
        let id = self
            .inner
            .next_observer_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        recover_lock(&self.inner.observers).insert(
            id,
            BindingStateObserver {
                callback: Arc::new(callback),
            },
        );
        BindingStateSubscription {
            source: Arc::downgrade(&self.inner),
            id,
            active: true,
        }
    }

    pub fn retain_subscription(&self, subscription: BindingStateSubscription) {
        recover_lock(&self.inner.retained_subscriptions).push(subscription);
    }

    pub(crate) fn set_immediate(&self, value: BindingValue) {
        {
            let mut current = recover_lock(&self.inner.value);
            if *current == value {
                return;
            }
            *current = value.clone();
        }
        let observers = recover_lock(&self.inner.observers)
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for observer in observers {
            (observer.callback)(value.clone());
        }
    }
}
