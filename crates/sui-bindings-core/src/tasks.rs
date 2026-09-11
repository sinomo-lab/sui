use crate::errors::{ForeignCallbackError, ForeignCallbackPhase, ForeignWidgetId, UiTask, UiWake};
use crate::messages::{BindingMessageBus, BindingValue};
use crate::support::recover_lock;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Clone, Default)]
pub struct UiTaskQueue {
    pub(crate) inner: Arc<UiTaskQueueInner>,
}

#[derive(Default)]
pub(crate) struct UiTaskQueueInner {
    pub(crate) tasks: Mutex<VecDeque<UiTask>>,
    pub(crate) wake: Mutex<Option<UiWake>>,
    pub(crate) draining_depth: AtomicUsize,
}

impl UiTaskQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_waker(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let queue = Self::new();
        queue.set_waker(wake);
        queue
    }

    pub fn handle(&self) -> BindingUiHandle {
        BindingUiHandle {
            inner: Arc::clone(&self.inner),
            messages: None,
        }
    }

    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        *recover_lock(&self.inner.wake) = Some(Arc::new(wake));
    }

    pub fn clear_waker(&self) {
        *recover_lock(&self.inner.wake) = None;
    }

    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        self.handle().post(task);
    }

    pub fn drain(&self) -> usize {
        self.inner.draining_depth.fetch_add(1, Ordering::SeqCst);
        let _guard = UiTaskDrainGuard { inner: &self.inner };
        let mut drained = 0;
        loop {
            let task = recover_lock(&self.inner.tasks).pop_front();
            let Some(task) = task else {
                break;
            };
            task();
            drained += 1;
        }
        drained
    }

    pub fn pending_count(&self) -> usize {
        recover_lock(&self.inner.tasks).len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }
}

pub(crate) struct UiTaskDrainGuard<'a> {
    pub(crate) inner: &'a UiTaskQueueInner,
}

impl Drop for UiTaskDrainGuard<'_> {
    fn drop(&mut self) {
        self.inner.draining_depth.fetch_sub(1, Ordering::SeqCst);
    }
}

impl fmt::Debug for UiTaskQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiTaskQueue")
            .field("pending_count", &self.pending_count())
            .finish()
    }
}

#[derive(Clone)]
pub struct BindingUiHandle {
    pub(crate) inner: Arc<UiTaskQueueInner>,
    pub(crate) messages: Option<BindingMessageBus>,
}

impl BindingUiHandle {
    pub(crate) fn with_message_bus(mut self, messages: BindingMessageBus) -> Self {
        self.messages = Some(messages);
        self
    }

    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        recover_lock(&self.inner.tasks).push_back(Box::new(task));
        let wake = recover_lock(&self.inner.wake).clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    pub fn pending_count(&self) -> usize {
        recover_lock(&self.inner.tasks).len()
    }

    pub fn emit(&self, name: impl Into<String>, payload: BindingValue) -> bool {
        let Some(messages) = &self.messages else {
            return false;
        };
        let name = name.into();
        let actions = messages.actions(&name);
        if actions.is_empty() {
            return false;
        }
        let errors = messages.errors.clone();
        self.post(move || {
            for action in actions {
                if let Err(error) = action.run(payload.clone()) {
                    errors.push(ForeignCallbackError::new(
                        ForeignWidgetId::new(0),
                        ForeignCallbackPhase::Event,
                        error.message,
                    ));
                }
            }
        });
        true
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.inner.draining_depth.load(Ordering::SeqCst) > 0
    }
}

impl fmt::Debug for BindingUiHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingUiHandle")
            .field("pending_count", &self.pending_count())
            .finish()
    }
}
