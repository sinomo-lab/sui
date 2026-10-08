use crate::errors::{ForeignCallbackError, ForeignCallbackPhase, ForeignWidgetId, UiTask, UiWake};
use crate::messages::{BindingMessageBus, BindingValue};
use crate::support::recover_lock;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct UiTaskQueue {
    pub(crate) inner: Arc<UiTaskQueueInner>,
}

#[derive(Default)]
pub(crate) struct UiTaskQueueInner {
    pub(crate) tasks: Mutex<VecDeque<QueuedUiTask>>,
    pub(crate) wake: Mutex<Option<UiWake>>,
    pub(crate) exit: Mutex<Option<UiWake>>,
    pub(crate) draining_depth: AtomicUsize,
}

pub(crate) struct QueuedUiTask {
    task: UiTask,
    /// Quiet tasks are housekeeping that does not change the widget tree, so
    /// draining them alone does not count as UI work.
    quiet: bool,
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

    /// Install the callback that [`BindingUiHandle::request_exit`] forwards
    /// to while a platform event loop is running.
    pub fn set_exit_hook(&self, exit: impl Fn() + Send + Sync + 'static) {
        *recover_lock(&self.inner.exit) = Some(Arc::new(exit));
    }

    pub fn clear_exit_hook(&self) {
        *recover_lock(&self.inner.exit) = None;
    }

    pub fn post(&self, task: impl FnOnce() + Send + 'static) {
        self.handle().post(task);
    }

    /// Run every queued task and return how many were not quiet.
    pub fn drain(&self) -> usize {
        self.inner.draining_depth.fetch_add(1, Ordering::SeqCst);
        let _guard = UiTaskDrainGuard { inner: &self.inner };
        let mut drained = 0;
        loop {
            let queued = recover_lock(&self.inner.tasks).pop_front();
            let Some(queued) = queued else {
                break;
            };
            (queued.task)();
            if !queued.quiet {
                drained += 1;
            }
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
        self.enqueue(Box::new(task), false);
    }

    /// Post housekeeping that does not change the widget tree. Draining only
    /// quiet tasks does not request layout, paint, or semantics updates.
    pub fn post_quiet(&self, task: impl FnOnce() + Send + 'static) {
        self.enqueue(Box::new(task), true);
    }

    fn enqueue(&self, task: UiTask, quiet: bool) {
        recover_lock(&self.inner.tasks).push_back(QueuedUiTask { task, quiet });
        let wake = recover_lock(&self.inner.wake).clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    /// Ask the running platform event loop to exit. Returns `false` when no
    /// event loop is running, such as for host-driven runtimes.
    pub fn request_exit(&self) -> bool {
        let exit = recover_lock(&self.inner.exit).clone();
        match exit {
            Some(exit) => {
                exit();
                true
            }
            None => false,
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

/// A pending or repeating callback scheduled through
/// [`BindingUiHandle::call_later`] or [`BindingUiHandle::call_every`].
#[derive(Clone, Debug, Default)]
pub struct BindingTimer {
    stopped: Arc<AtomicBool>,
}

impl BindingTimer {
    /// Stop the timer. A callback already posted to the UI thread still
    /// runs.
    pub fn cancel(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    /// Whether the timer can still fire.
    pub fn is_active(&self) -> bool {
        !self.stopped.load(Ordering::Acquire)
    }
}

impl BindingUiHandle {
    /// Run `task` on the UI thread once `delay` has passed.
    pub fn call_later(
        &self,
        delay: Duration,
        task: impl FnOnce() + Send + 'static,
    ) -> BindingTimer {
        let timer = BindingTimer::default();
        let stopped = Arc::clone(&timer.stopped);
        let queue = Arc::downgrade(&self.inner);
        thread::spawn(move || {
            thread::sleep(delay);
            // Stop once the app that owns the queue is gone.
            let Some(inner) = queue.upgrade() else {
                return;
            };
            if !stopped.swap(true, Ordering::AcqRel) {
                BindingUiHandle {
                    inner,
                    messages: None,
                }
                .post(task);
            }
        });
        timer
    }

    /// Run `task` on the UI thread every `interval` until the timer is
    /// cancelled or its app is gone. A run that is still waiting for the UI
    /// thread is not queued again.
    pub fn call_every(
        &self,
        interval: Duration,
        task: impl Fn() + Send + Sync + 'static,
    ) -> BindingTimer {
        let timer = BindingTimer::default();
        let stopped = Arc::clone(&timer.stopped);
        let queue = Arc::downgrade(&self.inner);
        let task = Arc::new(task);
        let in_flight = Arc::new(AtomicBool::new(false));
        thread::spawn(move || {
            loop {
                thread::sleep(interval);
                if stopped.load(Ordering::Acquire) {
                    return;
                }
                let Some(inner) = queue.upgrade() else {
                    stopped.store(true, Ordering::Release);
                    return;
                };
                if in_flight.swap(true, Ordering::AcqRel) {
                    continue;
                }
                let task = Arc::clone(&task);
                let in_flight = Arc::clone(&in_flight);
                let stopped = Arc::clone(&stopped);
                BindingUiHandle {
                    inner,
                    messages: None,
                }
                .post(move || {
                    if !stopped.load(Ordering::Acquire) {
                        task();
                    }
                    in_flight.store(false, Ordering::Release);
                });
            }
        });
        timer
    }
}
