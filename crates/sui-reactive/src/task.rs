use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{Observable, Observer, Selector, Signal, SourceId, Subscription};

/// Lifecycle of a background operation observed through a [`Task`].
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState<T, E = String> {
    Idle,
    /// Running. `progress` is `None` while indeterminate, otherwise in `0..=1`.
    Loading {
        progress: Option<f32>,
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
            Self::Loading { progress } => *progress,
            _ => None,
        }
    }

    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
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
/// the worker. Starting again, [`reset`](Self::reset), or setting a result
/// directly retires every earlier handle, whose later reports are ignored.
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
    /// worker reports through. Retires earlier handles.
    pub fn start(&self) -> TaskHandle<T, E> {
        let mut generation = 0;
        self.state.modify(|state| {
            generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
            let changed = !matches!(state, TaskState::Loading { progress: None });
            *state = TaskState::Loading { progress: None };
            changed
        });
        TaskHandle {
            task: self.clone(),
            generation,
        }
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
/// again, reset, or completed by this or another handle. Dropping a handle
/// without completing leaves the task `Loading`.
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
        self.task.write_if_current(self.generation, false, |state| {
            let next = TaskState::Loading { progress };
            if matches!(state, TaskState::Loading { progress: current } if *current == progress) {
                return false;
            }
            *state = next;
            true
        })
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
        assert_eq!(task.state(), TaskState::Loading { progress: None });

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
}
