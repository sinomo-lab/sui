use std::{
    fmt,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{Changed, Observable, Observer, Selector, Signal, SourceId, Subscription};

/// Lifecycle of a background operation observed through a [`Task`].
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState<T, E = String> {
    Idle,
    /// Running. `progress` is `None` while indeterminate, otherwise in `0..=1`.
    /// `previous` holds the last result while a [`Task::refresh`] runs.
    Loading {
        progress: Option<f32>,
        previous: Option<T>,
    },
    Ready(T),
    Failed(E),
}

impl<T, E> TaskState<T, E> {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Progress while loading; `None` when indeterminate or not loading.
    pub fn progress(&self) -> Option<f32> {
        match self {
            Self::Loading { progress, .. } => *progress,
            _ => None,
        }
    }

    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }

    /// The current result, or while refreshing, the previous one.
    pub fn latest(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            Self::Loading { previous, .. } => previous.as_ref(),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<&E> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
}

/// Observable state of a background operation: a [`Signal`] holding a
/// [`TaskState`], plus a generation counter that keeps stale workers from
/// overwriting newer results.
///
/// [`start`](Self::start) moves to `Loading` and returns a [`TaskHandle`] for
/// the worker; [`refresh`](Self::refresh) does the same but keeps the last
/// result visible. Starting again, [`reset`](Self::reset), or setting a result
/// directly retires every earlier handle, whose later reports are ignored.
///
/// For async work, [`TaskHandle::run`] wraps a future that you spawn on any
/// executor; dropping that future cancels the run.
///
/// ```ignore
/// let thumbnails: Task<Arc<Vec<Image>>> = Task::named("thumbnails");
/// let handle = thumbnails.start();
/// std::thread::spawn(move || {
///     for (index, path) in paths.iter().enumerate() {
///         handle.set_progress(index as f32 / paths.len() as f32);
///         // ...
///     }
///     handle.complete(load_all(&paths).map(Arc::new));
/// });
/// ```
///
/// Writes need no `Clone` or `PartialEq` on `T` or `E`. Observers run on the
/// reporting thread, as with any [`Signal`]. Wrap large results in `Arc` so
/// [`state`](Self::state) stays cheap, or read with [`with`](Self::with).
pub struct Task<T, E = String> {
    state: Signal<TaskState<T, E>>,
    // Changed only while `state` is write-locked, so a check and the write it
    // guards are atomic with respect to other transitions.
    generation: Arc<AtomicU64>,
}

impl<T, E> Clone for Task<T, E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            generation: Arc::clone(&self.generation),
        }
    }
}

impl<T, E> fmt::Debug for Task<T, E>
where
    T: fmt::Debug,
    E: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Task")
            .field("state", &self.state)
            .field("generation", &self.generation.load(Ordering::Acquire))
            .finish()
    }
}

impl<T, E> Default for Task<T, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, E> Task<T, E> {
    pub fn new() -> Self {
        Self::named("Task")
    }

    pub fn named(name: impl Into<Arc<str>>) -> Self {
        Self {
            state: Signal::named(name, TaskState::Idle),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Enter `Loading` with indeterminate progress and return the handle the
    /// worker reports through. Retires earlier handles and drops any result.
    pub fn start(&self) -> TaskHandle<T, E> {
        self.begin(false)
    }

    /// Like [`start`](Self::start), but keep the current result as
    /// `Loading { previous }` so the UI can show it until the new one lands.
    /// Cancelling the run restores it as `Ready`.
    pub fn refresh(&self) -> TaskHandle<T, E> {
        self.begin(true)
    }

    /// Return to `Idle`, retiring every outstanding handle.
    pub fn reset(&self) -> bool {
        self.retire_and_set(TaskState::Idle)
    }

    /// Store a result directly, retiring every outstanding handle.
    pub fn finish(&self, value: T) -> bool {
        self.retire_and_set(TaskState::Ready(value))
    }

    /// Store a failure directly, retiring every outstanding handle.
    pub fn fail(&self, error: E) -> bool {
        self.retire_and_set(TaskState::Failed(error))
    }

    pub fn complete(&self, result: Result<T, E>) -> bool {
        self.retire_and_set(result.into())
    }

    pub fn state(&self) -> TaskState<T, E>
    where
        T: Clone,
        E: Clone,
    {
        self.state.get()
    }

    /// Read the state by reference without cloning a result.
    pub fn with<R>(&self, read: impl FnOnce(&TaskState<T, E>) -> R) -> R {
        self.state.with(read)
    }

    pub fn is_loading(&self) -> bool {
        self.with(TaskState::is_loading)
    }

    pub fn progress(&self) -> Option<f32> {
        self.with(TaskState::progress)
    }

    /// The underlying signal, for APIs that take a [`Signal`].
    pub fn signal(&self) -> &Signal<TaskState<T, E>> {
        &self.state
    }

    pub fn source_id(&self) -> SourceId {
        self.state.source_id()
    }

    pub fn version(&self) -> u64 {
        self.state.version()
    }

    pub fn subscribe(&self, observer: Observer) -> Subscription {
        self.state.subscribe(observer)
    }

    /// A future that resolves at the next state change.
    pub fn changed(&self) -> Changed {
        self.state.changed()
    }

    pub fn select<U>(
        &self,
        select: impl Fn(&TaskState<T, E>) -> U + Send + Sync + 'static,
    ) -> Selector<Signal<TaskState<T, E>>, TaskState<T, E>, U>
    where
        T: Clone + Send + Sync + 'static,
        E: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        self.state.select_named(self.state.source_name(), select)
    }

    pub fn select_named<U>(
        &self,
        name: impl Into<Arc<str>>,
        select: impl Fn(&TaskState<T, E>) -> U + Send + Sync + 'static,
    ) -> Selector<Signal<TaskState<T, E>>, TaskState<T, E>, U>
    where
        T: Clone + Send + Sync + 'static,
        E: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        self.state.select_named(name, select)
    }

    fn begin(&self, keep_previous: bool) -> TaskHandle<T, E> {
        let mut generation = 0;
        self.state.modify(|state| {
            generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
            let unchanged = matches!(
                state,
                TaskState::Loading {
                    progress: None,
                    previous: None
                }
            );
            let previous = match std::mem::replace(state, TaskState::Idle) {
                TaskState::Ready(value) if keep_previous => Some(value),
                TaskState::Loading { previous, .. } if keep_previous => previous,
                _ => None,
            };
            let changed = !(unchanged && previous.is_none());
            *state = TaskState::Loading {
                progress: None,
                previous,
            };
            changed
        });
        TaskHandle {
            task: self.clone(),
            generation,
        }
    }

    fn retire_and_set(&self, next: TaskState<T, E>) -> bool {
        self.state.modify(|state| {
            self.generation.fetch_add(1, Ordering::AcqRel);
            let changed = !(state.is_idle() && next.is_idle());
            *state = next;
            changed
        })
    }

    /// Apply `write` only while `generation` is still current.
    fn write_if_current(
        &self,
        generation: u64,
        retire: bool,
        write: impl FnOnce(&mut TaskState<T, E>) -> bool,
    ) -> bool {
        self.state.modify(|state| {
            if self.generation.load(Ordering::Acquire) != generation {
                return false;
            }
            if retire {
                self.generation.fetch_add(1, Ordering::AcqRel);
            }
            write(state)
        })
    }
}

impl<T, E> From<Result<T, E>> for TaskState<T, E> {
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self::Ready(value),
            Err(error) => Self::Failed(error),
        }
    }
}

impl<T, E> Observable<TaskState<T, E>> for Task<T, E>
where
    T: Clone + 'static,
    E: Clone + 'static,
{
    fn source_id(&self) -> SourceId {
        self.state.source_id()
    }

    fn source_name(&self) -> Arc<str> {
        self.state.source_name()
    }

    fn get(&self) -> TaskState<T, E> {
        self.state.get()
    }

    fn read(&self, reader: &mut dyn FnMut(&TaskState<T, E>)) {
        self.state.with(|state| reader(state));
    }

    fn value_version(&self) -> Option<u64> {
        Some(self.state.version())
    }

    fn subscribe(&self, observer: Observer) -> Subscription {
        self.state.subscribe(observer)
    }
}

/// A worker's reporting channel for one run of a [`Task`].
///
/// Every report is ignored once the run is retired: after the task is started
/// again, reset, or completed or cancelled by this or another handle. Dropping
/// a handle without completing leaves the task `Loading`; call
/// [`cancel`](Self::cancel) to abandon a run.
pub struct TaskHandle<T, E = String> {
    task: Task<T, E>,
    generation: u64,
}

impl<T, E> Clone for TaskHandle<T, E> {
    fn clone(&self) -> Self {
        Self {
            task: self.task.clone(),
            generation: self.generation,
        }
    }
}

impl<T, E> fmt::Debug for TaskHandle<T, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskHandle")
            .field("source_id", &self.task.source_id())
            .field("generation", &self.generation)
            .finish()
    }
}

impl<T, E> TaskHandle<T, E> {
    /// Whether this run is still the task's current one. A worker may poll
    /// this to stop early after being superseded.
    pub fn is_current(&self) -> bool {
        self.task.generation.load(Ordering::Acquire) == self.generation
    }

    /// Report determinate progress, clamped to `0..=1`. Repeating the current
    /// value does not notify. NaN reports indeterminate progress.
    pub fn set_progress(&self, progress: f32) -> bool {
        let progress = (!progress.is_nan()).then(|| progress.clamp(0.0, 1.0));
        self.task
            .write_if_current(self.generation, false, |state| match state {
                TaskState::Loading {
                    progress: current, ..
                } => {
                    let changed = *current != progress;
                    *current = progress;
                    changed
                }
                _ => {
                    *state = TaskState::Loading {
                        progress,
                        previous: None,
                    };
                    true
                }
            })
    }

    /// Abandon the run: back to `Idle`, or to the previous result after a
    /// [`Task::refresh`]. Ignored if the run was already retired.
    pub fn cancel(self) -> bool {
        self.task.write_if_current(self.generation, true, |state| {
            *state = match std::mem::replace(state, TaskState::Idle) {
                TaskState::Loading {
                    previous: Some(value),
                    ..
                } => TaskState::Ready(value),
                _ => TaskState::Idle,
            };
            true
        })
    }

    /// Wrap `work` so its output completes this run. Spawn the returned
    /// future on any executor; it resolves to whether the result was stored.
    /// Dropping it before `work` finishes [cancels](Self::cancel) the run.
    ///
    /// ```ignore
    /// let handle = results.refresh();
    /// let progress = handle.clone();
    /// wasm_bindgen_futures::spawn_local(async move {
    ///     handle
    ///         .run(async move {
    ///             progress.set_progress(0.5);
    ///             fetch_results().await
    ///         })
    ///         .await;
    /// });
    /// ```
    pub fn run<Work>(self, work: Work) -> impl Future<Output = bool> + use<T, E, Work>
    where
        Work: Future<Output = Result<T, E>>,
    {
        let mut run = CancelOnDrop(Some(self));
        async move {
            let result = work.await;
            run.0.take().is_some_and(|handle| handle.complete(result))
        }
    }

    pub fn finish(self, value: T) -> bool {
        self.complete(Ok(value))
    }

    pub fn fail(self, error: E) -> bool {
        self.complete(Err(error))
    }

    /// Store the run's result unless the run was retired. Returns whether it
    /// was stored.
    pub fn complete(self, result: Result<T, E>) -> bool {
        self.task.write_if_current(self.generation, true, |state| {
            *state = result.into();
            true
        })
    }

    pub fn task(&self) -> &Task<T, E> {
        &self.task
    }
}

/// Cancels the run if [`TaskHandle::run`]'s future is dropped unfinished.
struct CancelOnDrop<T, E>(Option<TaskHandle<T, E>>);

impl<T, E> Drop for CancelOnDrop<T, E> {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    fn count_notifications<T, E>(task: &Task<T, E>) -> (Arc<AtomicUsize>, Subscription) {
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let subscription = task.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));
        (notifications, subscription)
    }

    #[test]
    fn task_reports_progress_and_result_from_a_worker() {
        let task: Task<u32> = Task::named("load");
        let (notifications, _subscription) = count_notifications(&task);
        let handle = task.start();
        assert_eq!(
            task.state(),
            TaskState::Loading {
                progress: None,
                previous: None
            }
        );

        std::thread::spawn(move || {
            assert!(handle.set_progress(0.5));
            assert!(!handle.set_progress(0.5), "equal progress is deduplicated");
            assert!(handle.set_progress(2.0));
            assert!(handle.finish(7));
        })
        .join()
        .unwrap();

        assert_eq!(task.state(), TaskState::Ready(7));
        // start, 0.5, 1.0 (clamped), ready
        assert_eq!(notifications.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn restarting_retires_the_previous_handle() {
        let task: Task<&'static str> = Task::new();
        let stale = task.start();
        let current = task.start();

        assert!(!stale.is_current());
        assert!(!stale.set_progress(0.9));
        assert!(!stale.clone().finish("stale"));
        assert!(task.is_loading());

        let progress = current.clone();
        assert!(current.finish("fresh"));
        assert!(!progress.set_progress(0.1), "completion retires clones too");
        assert_eq!(task.state(), TaskState::Ready("fresh"));
    }

    #[test]
    fn reset_cancels_a_running_worker() {
        let task: Task<u32> = Task::new();
        let handle = task.start();
        assert!(task.reset());
        assert!(!task.reset(), "already idle");
        assert!(!handle.fail("late".to_string()));
        assert!(task.with(TaskState::is_idle));
    }

    #[test]
    fn task_state_needs_no_clone_or_equality_to_write() {
        struct Opaque;
        struct OpaqueError;

        let task: Task<Opaque, OpaqueError> = Task::new();
        let handle = task.start();
        assert!(handle.complete(Err(OpaqueError)));
        assert!(task.with(|state| state.error().is_some()));
        assert!(task.finish(Opaque));
        assert!(task.with(|state| state.ready().is_some()));
    }

    #[test]
    fn selector_tracks_task_progress() {
        let task: Task<u32> = Task::new();
        let percent = task.select(|state| state.progress().map(|p| (p * 100.0) as u32));
        let handle = task.start();
        handle.set_progress(0.25);
        assert_eq!(percent.get(), Some(25));
    }

    #[test]
    fn refresh_keeps_the_previous_result_until_replaced_or_cancelled() {
        let task: Task<&'static str> = Task::new();
        task.finish("first");

        let handle = task.refresh();
        assert_eq!(task.with(|state| state.latest().copied()), Some("first"));
        assert!(handle.set_progress(0.5));
        assert_eq!(task.progress(), Some(0.5));
        assert_eq!(task.with(|state| state.latest().copied()), Some("first"));
        assert!(handle.cancel());
        assert_eq!(task.state(), TaskState::Ready("first"));

        let handle = task.refresh();
        assert!(handle.finish("second"));
        assert_eq!(task.state(), TaskState::Ready("second"));

        // A plain start drops the result.
        task.start();
        assert_eq!(task.with(|state| state.latest().copied()), None);
    }

    /// Poll a future to completion on this thread, parking between polls.
    fn block_on<F: Future>(future: F) -> F::Output {
        struct ThreadWaker(std::thread::Thread);
        impl std::task::Wake for ThreadWaker {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = std::task::Waker::from(Arc::new(ThreadWaker(std::thread::current())));
        let mut context = std::task::Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
            std::thread::park();
        }
    }

    #[test]
    fn run_completes_the_task_from_a_future() {
        let task: Task<u32> = Task::new();
        let handle = task.start();
        let progress = handle.clone();
        let stored = block_on(handle.run(async move {
            progress.set_progress(0.5);
            Ok(42)
        }));
        assert!(stored);
        assert_eq!(task.state(), TaskState::Ready(42));
    }

    #[test]
    fn dropping_a_run_future_cancels_the_run() {
        let task: Task<u32> = Task::new();
        task.finish(1);
        let run = task.refresh().run(std::future::pending());
        assert!(task.is_loading());
        drop(run);
        assert_eq!(task.state(), TaskState::Ready(1));

        // A superseded run's drop leaves the newer run alone.
        let stale = task.start().run(std::future::pending());
        let current = task.start();
        drop(stale);
        assert!(task.is_loading());
        assert!(current.finish(2));
    }

    #[test]
    fn changed_resolves_when_a_worker_finishes() {
        let task: Task<u32> = Task::new();
        let handle = task.start();
        let changed = task.changed();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            handle.finish(5);
        });
        let change = block_on(changed);
        assert_eq!(change.source_id, task.source_id());
        assert_eq!(task.state(), TaskState::Ready(5));
        worker.join().unwrap();
    }
}
