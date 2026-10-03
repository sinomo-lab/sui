#![forbid(unsafe_code)]

//! Architecture-neutral observable values for SUI.
//!
//! This crate owns state and change notification only. It does not depend on
//! the SUI widget runtime, so applications may adapt other stores through
//! [`Observable`] without adopting a SUI-owned application architecture.
//!
//! # Threads
//!
//! [`Signal`] is `Send + Sync` when its value is, and may be written from any
//! thread. Observer callbacks run synchronously **on the thread that wrote the
//! value**, after the value lock is released and before the write returns.
//! Inside [`batch`], they run on the batching thread when the outermost batch
//! ends. This crate never moves an observer call to another thread.
//!
//! Observers therefore must be cheap, non-blocking, and safe to run on a
//! worker. The SUI runtime's widget observers only enqueue an invalidation and
//! wake the event loop; widgets re-read the value later on the UI thread.
//! Custom observers that need UI-thread state should forward a message (for
//! example through SUI's `UiHandle`) instead of touching that state directly.
//!
//! A value read inside an observer may already be newer than the
//! [`Change::version`] it received if another thread wrote concurrently.
//! Observers may write back to the signal that notified them.
//!
//! # Large values
//!
//! [`Signal::get`] clones the value and [`Signal::update`] clones it to
//! compare. For large state, read with [`Signal::with`], mutate with
//! [`Signal::modify`] or [`Signal::mark_changed`], or store an `Arc<T>` and
//! use [`Signal::set_arc`] and [`Signal::modify_arc`], which compare pointers
//! and copy only while a reader still holds the previous snapshot.

mod batch;
mod combine;
mod task;

use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{
        Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll, Waker},
};

pub use batch::batch;
pub use combine::{Zip, combine, combine_named};
pub use task::{Task, TaskHandle, TaskState};

static NEXT_SOURCE_ID: AtomicU64 = AtomicU64::new(1);

/// Stable identity for one observable source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(u64);

impl SourceId {
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Allocate a process-unique observable source identifier.
    pub fn new() -> Self {
        Self(NEXT_SOURCE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for SourceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata emitted when an observable value changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub source_id: SourceId,
    pub source_name: Arc<str>,
    pub version: u64,
}

type ObserverCallback = dyn Fn(Change) + Send + Sync + 'static;

struct ObserverInner {
    callback: Box<ObserverCallback>,
}

/// Cloneable change observer used by observable implementations.
///
/// The callback runs on the thread that changed the observed value; see the
/// [crate-level threading notes](crate#threads).
#[derive(Clone)]
pub struct Observer {
    inner: Arc<ObserverInner>,
}

impl Observer {
    pub fn new(callback: impl Fn(Change) + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(ObserverInner {
                callback: Box::new(callback),
            }),
        }
    }

    pub fn notify(&self, change: Change) {
        (self.inner.callback)(change);
    }

    pub fn downgrade(&self) -> WeakObserver {
        WeakObserver {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl fmt::Debug for Observer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Observer").finish_non_exhaustive()
    }
}

/// Weak observer reference suitable for storage inside custom observables.
#[derive(Clone)]
pub struct WeakObserver {
    inner: Weak<ObserverInner>,
}

impl WeakObserver {
    /// Notify the observer, returning `false` after its subscription was
    /// dropped.
    pub fn notify(&self, change: Change) -> bool {
        let Some(observer) = self.inner.upgrade() else {
            return false;
        };
        (observer.callback)(change);
        true
    }
}

impl fmt::Debug for WeakObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeakObserver")
            .finish_non_exhaustive()
    }
}

/// Keeps an observable subscription alive.
///
/// Dropping the guard releases the strong observer reference. Sources retain
/// only weak references and prune them during later notifications.
pub struct Subscription {
    // Dropped before `_release`, so the release hook sees the observer gone.
    _observer: Observer,
    _release: Option<Box<dyn Send + Sync>>,
}

impl Subscription {
    pub fn new(observer: Observer) -> Self {
        Self {
            _observer: observer,
            _release: None,
        }
    }

    /// Keep `release` alive with the subscription and drop it right after the
    /// observer, e.g. to tear down upstream work once nobody is listening.
    pub fn with_release(observer: Observer, release: impl Send + Sync + 'static) -> Self {
        Self {
            _observer: observer,
            _release: Some(Box::new(release)),
        }
    }
}

impl fmt::Debug for Subscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Subscription")
            .finish_non_exhaustive()
    }
}

/// Readable value that can notify subscribers after meaningful changes.
///
/// SUI's runtime consumes this trait, so applications may provide adapters for
/// reducer stores, channels, actor snapshots, or other state architectures.
///
/// The simplest custom implementation keeps a `Signal<u64>` revision next to
/// its data, calls [`Signal::mark_changed`] after each change, and delegates
/// `source_id`, `source_name`, `subscribe`, and `value_version` to it. That
/// gives it batching and observer pruning for free.
pub trait Observable<T> {
    fn source_id(&self) -> SourceId;
    fn source_name(&self) -> Arc<str>;
    fn get(&self) -> T;
    fn subscribe(&self, observer: Observer) -> Subscription;

    /// Call `reader` once with a borrow of the current value.
    ///
    /// The default clones through [`get`](Self::get). Implementations that
    /// store the value should override it to avoid the copy; selectors read
    /// their source through this method.
    fn read(&self, reader: &mut dyn FnMut(&T)) {
        reader(&self.get());
    }

    /// A counter identifying the current value, or `None` if unknown.
    ///
    /// Selectors use it to reuse a derived value while the source has not
    /// changed. It must increase *after* every change to the value, so that a
    /// version read before [`read`](Self::read) never claims a newer value
    /// than the one read. Return `None` if that cannot be guaranteed.
    fn value_version(&self) -> Option<u64> {
        None
    }

    /// A future that resolves with the next change notification after this
    /// call. It subscribes immediately, so a change made before the first
    /// poll is not missed. It needs no particular executor.
    fn changed(&self) -> Changed
    where
        Self: Sized,
    {
        Changed::new(|observer| self.subscribe(observer))
    }
}

/// Future returned by [`Observable::changed`] and [`Signal::changed`].
///
/// Resolves with the most recent [`Change`] once at least one has arrived.
/// Dropping it unsubscribes.
#[must_use = "futures do nothing unless awaited"]
pub struct Changed {
    state: Arc<Mutex<ChangedState>>,
    _subscription: Subscription,
}

#[derive(Default)]
struct ChangedState {
    change: Option<Change>,
    waker: Option<Waker>,
}

impl Changed {
    fn new(subscribe: impl FnOnce(Observer) -> Subscription) -> Self {
        let state = Arc::new(Mutex::new(ChangedState::default()));
        let observed = Arc::clone(&state);
        let subscription = subscribe(Observer::new(move |change| {
            let waker = {
                let mut state = lock(&observed);
                state.change = Some(change);
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }));
        Self {
            state,
            _subscription: subscription,
        }
    }
}

impl Future for Changed {
    type Output = Change;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Change> {
        let mut state = lock(&self.state);
        if let Some(change) = state.change.take() {
            return Poll::Ready(change);
        }
        if !state
            .waker
            .as_ref()
            .is_some_and(|waker| waker.will_wake(context.waker()))
        {
            state.waker = Some(context.waker().clone());
        }
        Poll::Pending
    }
}

impl fmt::Debug for Changed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Changed").finish_non_exhaustive()
    }
}

fn read_with<T, R, O>(observable: &O, read: impl FnOnce(&T) -> R) -> R
where
    O: Observable<T> + ?Sized,
{
    let mut read = Some(read);
    let mut result = None;
    observable.read(&mut |value| {
        if let Some(read) = read.take() {
            result = Some(read(value));
        }
    });
    result.expect("Observable::read must call its reader")
}

/// Identity, revision, and observers of one signal. Kept apart from the value
/// so [`batch`] can queue a deferred notification without knowing its type.
pub(crate) struct SignalCore {
    id: SourceId,
    name: Arc<str>,
    version: AtomicU64,
    observers: Mutex<Vec<Weak<ObserverInner>>>,
}

impl SignalCore {
    fn new(name: Arc<str>) -> Arc<Self> {
        Arc::new(Self {
            id: SourceId::new(),
            name,
            version: AtomicU64::new(0),
            observers: Mutex::new(Vec::new()),
        })
    }

    fn changed(self: &Arc<Self>) {
        self.version.fetch_add(1, Ordering::AcqRel);
        self.notify();
    }

    fn notify(self: &Arc<Self>) {
        if !batch::defer(self) {
            self.deliver();
        }
    }

    fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    fn add_observer(&self, observer: &Observer) {
        lock(&self.observers).push(Arc::downgrade(&observer.inner));
    }

    /// Prune dropped observers and report whether any remain.
    fn has_observers(&self) -> bool {
        let mut observers = lock(&self.observers);
        observers.retain(|observer| observer.strong_count() > 0);
        !observers.is_empty()
    }

    pub(crate) fn source_id(&self) -> SourceId {
        self.id
    }

    pub(crate) fn deliver(&self) {
        let change = Change {
            source_id: self.id,
            source_name: Arc::clone(&self.name),
            version: self.version.load(Ordering::Acquire),
        };
        let observers = {
            let mut observers = lock(&self.observers);
            let mut live = Vec::with_capacity(observers.len());
            observers.retain(|observer| {
                if let Some(observer) = observer.upgrade() {
                    live.push(observer);
                    true
                } else {
                    false
                }
            });
            live
        };
        for observer in observers {
            (observer.callback)(change.clone());
        }
    }
}

struct SignalInner<T> {
    core: Arc<SignalCore>,
    value: RwLock<T>,
}

/// Cloneable observable storage with equality-deduplicated writes.
///
/// Every write that changes the value increments [`version`](Self::version)
/// and notifies observers on the writing thread (see the
/// [crate-level threading notes](crate#threads)).
///
/// Closures passed to [`with`](Self::with), [`update`](Self::update), and
/// [`modify`](Self::modify) run while the value is locked, so they must not
/// write to the same signal. Debug builds panic, naming the signal, instead
/// of deadlocking.
pub struct Signal<T> {
    inner: Arc<SignalInner<T>>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> fmt::Debug for Signal<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Signal")
            .field("id", &self.inner.core.id)
            .field("name", &self.inner.core.name)
            .field("value", &*self.read_lock())
            .field("version", &self.version())
            .finish()
    }
}

impl<T> Signal<T> {
    pub fn new(value: T) -> Self {
        Self::named("Signal", value)
    }

    pub fn named(name: impl Into<Arc<str>>, value: T) -> Self {
        Self {
            inner: Arc::new(SignalInner {
                core: SignalCore::new(name.into()),
                value: RwLock::new(value),
            }),
        }
    }

    pub fn source_id(&self) -> SourceId {
        self.inner.core.id
    }

    pub fn source_name(&self) -> Arc<str> {
        Arc::clone(&self.inner.core.name)
    }

    /// Revision counter, incremented by every write that notifies.
    pub fn version(&self) -> u64 {
        self.inner.core.version()
    }

    /// Clone the current value. Prefer [`with`](Self::with) for large values.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read_lock().clone()
    }

    /// Read the current value by reference without cloning it.
    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.read_lock())
    }

    pub fn set(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        {
            let mut current = self.write_lock();
            if *current == value {
                return false;
            }
            *current = value;
        }
        self.inner.core.changed();
        true
    }

    /// Mutate the value and notify if it differs from before.
    ///
    /// This clones the value to compare it. For large values use
    /// [`modify`](Self::modify), which lets the closure report the change.
    pub fn update(&self, update: impl FnOnce(&mut T)) -> bool
    where
        T: Clone + PartialEq,
    {
        {
            let mut current = self.write_lock();
            let previous = current.clone();
            update(&mut current);
            if *current == previous {
                return false;
            }
        }
        self.inner.core.changed();
        true
    }

    /// Mutate the value in place without cloning or comparing it.
    ///
    /// Observers are notified only when `modify` returns `true`.
    pub fn modify(&self, modify: impl FnOnce(&mut T) -> bool) -> bool {
        let changed = modify(&mut self.write_lock());
        if changed {
            self.inner.core.changed();
        }
        changed
    }

    /// Increment the version and notify observers without touching the value.
    ///
    /// Use after changing state the signal cannot see, such as data behind
    /// interior mutability, or as a revision bump for values without cheap
    /// equality.
    pub fn mark_changed(&self) {
        self.inner.core.changed();
    }

    /// Observe changes. Unlike [`Observable::subscribe`], this does not
    /// require `T: Clone`.
    pub fn subscribe(&self, observer: Observer) -> Subscription {
        self.inner.core.add_observer(&observer);
        Subscription::new(observer)
    }

    /// A future that resolves at the next change; see [`Observable::changed`].
    /// Unlike the trait method, this does not require `T: Clone`.
    pub fn changed(&self) -> Changed {
        Changed::new(|observer| self.subscribe(observer))
    }

    pub fn select<U>(
        &self,
        select: impl Fn(&T) -> U + Send + Sync + 'static,
    ) -> Selector<Self, T, U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        self.select_named("Selector", select)
    }

    pub fn select_named<U>(
        &self,
        name: impl Into<Arc<str>>,
        select: impl Fn(&T) -> U + Send + Sync + 'static,
    ) -> Selector<Self, T, U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        Selector::new(name, self.clone(), select)
    }

    fn read_lock(&self) -> Held<RwLockReadGuard<'_, T>> {
        let held = HeldLock::acquire(&self.inner.core, false);
        Held {
            guard: self
                .inner
                .value
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            _held: held,
        }
    }

    fn write_lock(&self) -> Held<RwLockWriteGuard<'_, T>> {
        let held = HeldLock::acquire(&self.inner.core, true);
        Held {
            guard: self
                .inner
                .value
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            _held: held,
        }
    }
}

/// A signal value guard plus its debug-build lock record.
struct Held<G> {
    // Unlock before forgetting the record.
    guard: G,
    _held: HeldLock,
}

impl<G: Deref> Deref for Held<G> {
    type Target = G::Target;

    fn deref(&self) -> &G::Target {
        &self.guard
    }
}

impl<G: DerefMut> DerefMut for Held<G> {
    fn deref_mut(&mut self) -> &mut G::Target {
        &mut self.guard
    }
}

#[cfg(debug_assertions)]
thread_local! {
    /// Signals whose value this thread has locked, and whether for writing.
    static HELD_LOCKS: std::cell::RefCell<Vec<(SourceId, bool)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// In debug builds, turns a same-thread re-entry that would deadlock on the
/// value lock into a panic naming the signal. Free in release builds.
struct HeldLock {
    #[cfg(debug_assertions)]
    id: SourceId,
}

impl HeldLock {
    #[cfg_attr(not(debug_assertions), allow(unused_variables))]
    fn acquire(core: &SignalCore, write: bool) -> Self {
        #[cfg(debug_assertions)]
        {
            let conflict = HELD_LOCKS
                .try_with(|held| {
                    let mut held = held.borrow_mut();
                    let conflict = held
                        .iter()
                        .any(|&(id, held_write)| id == core.id && (write || held_write));
                    if !conflict {
                        held.push((core.id, write));
                    }
                    conflict
                })
                .unwrap_or(false);
            if conflict {
                panic!(
                    "signal `{}` was {} while this thread already holds its value inside \
                     `with`, `update`, `modify`, or a selector; this would deadlock",
                    core.name,
                    if write { "written" } else { "read" },
                );
            }
            Self { id: core.id }
        }
        #[cfg(not(debug_assertions))]
        Self {}
    }
}

#[cfg(debug_assertions)]
impl Drop for HeldLock {
    fn drop(&mut self) {
        let _ = HELD_LOCKS.try_with(|held| {
            let mut held = held.borrow_mut();
            if let Some(index) = held.iter().rposition(|&(id, _)| id == self.id) {
                held.remove(index);
            }
        });
    }
}

impl<T> Signal<Arc<T>> {
    /// Replace the shared value, deduplicating by pointer rather than by
    /// value equality. [`get`](Self::get) then only clones the `Arc`.
    pub fn set_arc(&self, value: Arc<T>) -> bool {
        {
            let mut current = self.write_lock();
            if Arc::ptr_eq(&current, &value) {
                return false;
            }
            *current = value;
        }
        self.inner.core.changed();
        true
    }

    /// Mutate the shared value through [`Arc::make_mut`]: in place when no
    /// reader holds the current snapshot, otherwise on a fresh copy so the
    /// snapshots readers hold stay unchanged.
    ///
    /// Observers are notified only when `modify` returns `true`.
    pub fn modify_arc(&self, modify: impl FnOnce(&mut T) -> bool) -> bool
    where
        T: Clone,
    {
        self.modify(|value| modify(Arc::make_mut(value)))
    }
}

impl<T> Observable<T> for Signal<T>
where
    T: Clone + 'static,
{
    fn source_id(&self) -> SourceId {
        self.source_id()
    }

    fn source_name(&self) -> Arc<str> {
        self.source_name()
    }

    fn get(&self) -> T {
        self.get()
    }

    fn read(&self, reader: &mut dyn FnMut(&T)) {
        self.with(|value| reader(value));
    }

    fn value_version(&self) -> Option<u64> {
        Some(self.version())
    }

    fn subscribe(&self, observer: Observer) -> Subscription {
        self.subscribe(observer)
    }
}

struct SelectorInner<S, I, O> {
    name: Arc<str>,
    source: S,
    select: Box<dyn Fn(&I) -> O + Send + Sync>,
    /// Downstream observers and this selector's own change version.
    core: Arc<SignalCore>,
    /// Last derived value, keyed by the source's value version.
    cached: Mutex<Option<(u64, O)>>,
    tracking: Mutex<Tracking<O>>,
    _input: PhantomData<fn(I)>,
}

/// The single upstream subscription shared by every downstream observer.
struct Tracking<O> {
    upstream: Option<Subscription>,
    /// Value most recently delivered to observers, for deduplication.
    delivered: Option<O>,
}

/// Equality-deduplicated derived observable.
///
/// Clones share one upstream subscription and one cached value: however many
/// widgets observe a selector, `select` runs once per source change, and
/// [`get`](Observable::get) reuses the last result while the source's
/// [`value_version`](Observable::value_version) is unchanged. The upstream
/// subscription is released when the last downstream subscription drops.
///
/// The source is read by reference through [`Observable::read`], so selecting
/// a small field from a large [`Signal`] does not clone the whole value. The
/// select closure runs while the source is locked and must not write to it.
pub struct Selector<S, I, O> {
    inner: Arc<SelectorInner<S, I, O>>,
}

impl<S, I, O> Clone for Selector<S, I, O> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<S, I, O> Selector<S, I, O> {
    pub fn new(
        name: impl Into<Arc<str>>,
        source: S,
        select: impl Fn(&I) -> O + Send + Sync + 'static,
    ) -> Self {
        let name = name.into();
        Self {
            inner: Arc::new(SelectorInner {
                core: SignalCore::new(Arc::clone(&name)),
                name,
                source,
                select: Box::new(select),
                cached: Mutex::new(None),
                tracking: Mutex::new(Tracking {
                    upstream: None,
                    delivered: None,
                }),
                _input: PhantomData,
            }),
        }
    }
}

impl<S, I, O> SelectorInner<S, I, O>
where
    S: Observable<I>,
    O: Clone + PartialEq,
{
    fn compute(&self) -> O {
        // Read the version first: the value read after it is at least as new.
        let version = self.source.value_version();
        if let Some(version) = version
            && let Some((cached_version, value)) = &*lock(&self.cached)
            && *cached_version == version
        {
            return value.clone();
        }
        let value = read_with(&self.source, |input| (self.select)(input));
        if let Some(version) = version {
            let mut cached = lock(&self.cached);
            if cached
                .as_ref()
                .is_none_or(|(cached_version, _)| *cached_version <= version)
            {
                *cached = Some((version, value.clone()));
            }
        }
        value
    }

    fn source_changed(&self) {
        {
            let mut tracking = lock(&self.tracking);
            if tracking.upstream.is_none() {
                return; // Released while this notification was in flight.
            }
            // Derive and compare under one lock so concurrent writers cannot
            // store an older value after a newer one.
            let next = self.compute();
            if tracking.delivered.as_ref() == Some(&next) {
                return;
            }
            tracking.delivered = Some(next);
            self.core.version.fetch_add(1, Ordering::AcqRel);
        }
        // Observers may write back to the source and reenter this selector.
        self.core.notify();
    }
}

/// Drops the shared upstream subscription with the last downstream one.
struct SelectorRelease<S, I, O> {
    inner: Arc<SelectorInner<S, I, O>>,
}

impl<S, I, O> Drop for SelectorRelease<S, I, O> {
    fn drop(&mut self) {
        let upstream = {
            let mut tracking = lock(&self.inner.tracking);
            if self.inner.core.has_observers() {
                return;
            }
            tracking.delivered = None;
            tracking.upstream.take()
        };
        drop(upstream);
    }
}

impl<S, I, O> Observable<O> for Selector<S, I, O>
where
    S: Observable<I> + Send + Sync + 'static,
    I: Send + Sync + 'static,
    O: Clone + PartialEq + Send + Sync + 'static,
{
    fn source_id(&self) -> SourceId {
        self.inner.core.id
    }

    fn source_name(&self) -> Arc<str> {
        Arc::clone(&self.inner.name)
    }

    fn get(&self) -> O {
        self.inner.compute()
    }

    fn value_version(&self) -> Option<u64> {
        // A pure function of the source changes only when the source does.
        self.inner.source.value_version()
    }

    fn subscribe(&self, observer: Observer) -> Subscription {
        {
            let mut tracking = lock(&self.inner.tracking);
            self.inner.core.add_observer(&observer);
            if tracking.upstream.is_none() {
                let selector = Arc::downgrade(&self.inner);
                tracking.upstream = Some(self.inner.source.subscribe(Observer::new(move |_| {
                    if let Some(selector) = selector.upgrade() {
                        selector.source_changed();
                    }
                })));
                // Subscribed first, so a write racing this read is either
                // included here or delivered afterwards.
                tracking.delivered = Some(self.inner.compute());
            }
        }
        Subscription::with_release(
            observer,
            SelectorRelease {
                inner: Arc::clone(&self.inner),
            },
        )
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn counter<O: Observable<T>, T>(observable: &O) -> (Arc<AtomicUsize>, Subscription) {
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let subscription = observable.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));
        (notifications, subscription)
    }

    #[test]
    fn signal_notifies_only_for_meaningful_changes() {
        let signal = Signal::named("count", 1usize);
        let (notifications, _subscription) = counter(&signal);

        assert!(!signal.set(1));
        assert!(signal.set(2));
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(signal.version(), 1);
    }

    #[test]
    fn modify_and_mark_changed_notify_without_clone_or_comparison() {
        // Neither Clone nor PartialEq.
        struct Big(Vec<u8>);

        let signal = Signal::new(Big(vec![0; 1024]));
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let _subscription = signal.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));

        assert!(!signal.modify(|big| {
            big.0[0] = 0;
            false
        }));
        assert!(signal.modify(|big| {
            big.0[0] = 7;
            true
        }));
        signal.mark_changed();
        assert_eq!(signal.with(|big| big.0[0]), 7);
        assert_eq!(notifications.load(Ordering::Relaxed), 2);
        assert_eq!(signal.version(), 2);
    }

    #[test]
    fn arc_signal_uses_pointer_identity_and_copies_on_write() {
        let signal = Signal::new(Arc::new(vec![1, 2, 3]));
        let (notifications, _subscription) = counter(&signal);

        let same = signal.get();
        assert!(!signal.set_arc(Arc::clone(&same)));
        // Equal contents, different allocation: a deliberate replacement.
        assert!(signal.set_arc(Arc::new(vec![1, 2, 3])));
        assert_eq!(notifications.load(Ordering::Relaxed), 1);

        let snapshot = signal.get();
        assert!(signal.modify_arc(|values| {
            values.push(4);
            true
        }));
        assert_eq!(*snapshot, vec![1, 2, 3]);
        assert_eq!(*signal.get(), vec![1, 2, 3, 4]);

        drop(snapshot);
        let before = Arc::as_ptr(&signal.get());
        signal.modify_arc(|values| {
            values.push(5);
            true
        });
        assert_eq!(Arc::as_ptr(&signal.get()), before, "mutated in place");
        assert_eq!(notifications.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn selector_reads_source_without_cloning_it() {
        static CLONES: AtomicUsize = AtomicUsize::new(0);

        #[derive(PartialEq)]
        struct State {
            selected: usize,
        }
        impl Clone for State {
            fn clone(&self) -> Self {
                CLONES.fetch_add(1, Ordering::Relaxed);
                Self {
                    selected: self.selected,
                }
            }
        }

        let state = Signal::new(State { selected: 0 });
        let selected = state.select(|state| state.selected);
        let (notifications, _subscription) = counter(&selected);
        state.modify(|state| {
            state.selected = 3;
            true
        });
        assert_eq!(selected.get(), 3);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(CLONES.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn selector_deduplicates_unrelated_source_changes() {
        #[derive(Clone, PartialEq)]
        struct State {
            selected: usize,
            detail: String,
        }

        let state = Signal::new(State {
            selected: 0,
            detail: "first".to_string(),
        });
        let selected = state.select_named("selected", |state| state.selected);
        let (notifications, _subscription) = counter(&selected);

        state.update(|state| state.detail = "second".to_string());
        assert_eq!(notifications.load(Ordering::Relaxed), 0);
        state.update(|state| state.selected = 1);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(selected.get(), 1);
    }

    #[test]
    fn selector_observer_can_write_back_to_its_source() {
        let (done, completed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let source = Signal::new(0_u32);
            let selected = source.select(|value| *value);
            let writer = source.clone();
            let observed = Arc::new(Mutex::new(Vec::new()));
            let values = Arc::clone(&observed);
            let _subscription = selected.subscribe(Observer::new(move |_| {
                let value = writer.get();
                values.lock().unwrap().push(value);
                if value == 1 {
                    writer.set(2);
                }
            }));
            source.set(1);
            // An equal write must still be deduplicated after reentrant delivery.
            assert!(!source.set(2));
            done.send((source.get(), observed.lock().unwrap().clone()))
                .unwrap();
        });
        let (value, observed) = completed
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("selector source write-back deadlocked");
        worker.join().unwrap();
        assert_eq!(value, 2);
        assert_eq!(observed, vec![1, 2]);
    }

    #[test]
    fn observers_run_on_the_writing_thread() {
        let signal = Signal::new(0_u32);
        let threads = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&threads);
        let _subscription = signal.subscribe(Observer::new(move |_| {
            seen.lock().unwrap().push(std::thread::current().id());
        }));
        let writer = signal.clone();
        let worker = std::thread::spawn(move || {
            writer.set(1);
            std::thread::current().id()
        })
        .join()
        .unwrap();
        signal.set(2);
        assert_eq!(
            *threads.lock().unwrap(),
            vec![worker, std::thread::current().id()]
        );
    }

    fn counted_selector(
        source: &Signal<u32>,
    ) -> (Selector<Signal<u32>, u32, u32>, Arc<AtomicUsize>) {
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let selector = source.select(move |value| {
            counter.fetch_add(1, Ordering::Relaxed);
            value % 3
        });
        (selector, runs)
    }

    #[test]
    fn selector_runs_once_per_change_for_all_subscribers() {
        let source = Signal::new(0_u32);
        let (selected, runs) = counted_selector(&source);
        let subscriptions = (0..10)
            .map(|_| counter(&selected.clone()))
            .collect::<Vec<_>>();
        assert_eq!(runs.load(Ordering::Relaxed), 1, "seeded once");

        source.set(1);
        assert_eq!(runs.load(Ordering::Relaxed), 2);
        for _ in 0..5 {
            assert_eq!(selected.get(), 1);
        }
        assert_eq!(
            runs.load(Ordering::Relaxed),
            2,
            "get reuses the cached value"
        );
        for (notifications, _) in &subscriptions {
            assert_eq!(notifications.load(Ordering::Relaxed), 1);
        }

        source.set(4); // Same residue: recomputed once, nobody notified.
        assert_eq!(runs.load(Ordering::Relaxed), 3);
        assert_eq!(subscriptions[0].0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn selector_releases_its_source_with_the_last_subscriber() {
        let source = Signal::new(0_u32);
        let (selected, runs) = counted_selector(&source);
        let first = counter(&selected);
        let second = counter(&selected);
        assert!(source.inner.core.has_observers());

        drop(first);
        assert!(source.inner.core.has_observers(), "second still listening");
        drop(second);
        assert!(!source.inner.core.has_observers());

        let before = runs.load(Ordering::Relaxed);
        source.set(1);
        assert_eq!(
            runs.load(Ordering::Relaxed),
            before,
            "no work while unobserved"
        );
        assert_eq!(selected.get(), 1);

        let (notifications, _again) = counter(&selected);
        source.set(2);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn chained_selectors_see_writes_made_inside_a_batch() {
        let source = Signal::new(1_u32);
        let doubled = source.select(|value| value * 2);
        let label = Selector::new("label", doubled.clone(), |value: &u32| format!("{value}"));
        let (_notifications, _subscription) = counter(&label);
        batch(|| {
            source.set(5);
            assert_eq!(label.get(), "10", "cache keyed by the source version");
        });
    }

    /// A source that parks the notification reading `HELD` after it has
    /// derived a value but before the selector records it.
    #[derive(Clone)]
    struct GatedSource {
        signal: Signal<u32>,
        gate: Arc<(Mutex<(bool, bool)>, std::sync::Condvar)>,
    }

    impl GatedSource {
        const HELD: u32 = 1;

        fn wait(&self, until: impl Fn(&(bool, bool)) -> bool) {
            let (state, changed) = &*self.gate;
            let state = state.lock().unwrap();
            drop(changed.wait_while(state, |state| !until(state)).unwrap());
        }

        fn update(&self, update: impl FnOnce(&mut (bool, bool))) {
            let (state, changed) = &*self.gate;
            update(&mut state.lock().unwrap());
            changed.notify_all();
        }
    }

    impl Observable<u32> for GatedSource {
        fn source_id(&self) -> SourceId {
            self.signal.source_id()
        }

        fn source_name(&self) -> Arc<str> {
            self.signal.source_name()
        }

        fn get(&self) -> u32 {
            self.signal.get()
        }

        fn read(&self, reader: &mut dyn FnMut(&u32)) {
            let value = self.signal.get();
            reader(&value);
            if value == Self::HELD {
                self.update(|(parked, _)| *parked = true);
                self.wait(|&(_, open)| open);
            }
        }

        fn value_version(&self) -> Option<u64> {
            Some(self.signal.version())
        }

        fn subscribe(&self, observer: Observer) -> Subscription {
            self.signal.subscribe(observer)
        }
    }

    #[test]
    fn slow_notification_cannot_overwrite_a_newer_selection() {
        let signal = Signal::new(0_u32);
        let source = GatedSource {
            signal: signal.clone(),
            gate: Arc::new((Mutex::new((false, false)), std::sync::Condvar::new())),
        };
        let selected = Selector::new("residue", source.clone(), |value: &u32| value % 3);
        let (notifications, _subscription) = counter(&selected);

        // A derives 1 from its write, then stalls before recording it.
        let slow = {
            let signal = signal.clone();
            std::thread::spawn(move || signal.set(GatedSource::HELD))
        };
        source.wait(|&(parked, _)| parked);
        // B writes 2 meanwhile. It either records 2 first or waits for A.
        let fast = {
            let signal = signal.clone();
            std::thread::spawn(move || signal.set(2))
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        source.update(|(_, open)| *open = true);
        slow.join().unwrap();
        fast.join().unwrap();

        assert_eq!(lock(&selected.inner.tracking).delivered, Some(2));
        // A stale baseline of 1 would swallow this change.
        let before = notifications.load(Ordering::Relaxed);
        signal.set(4);
        assert_eq!(notifications.load(Ordering::Relaxed), before + 1);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "signal `count` was written while this thread already holds")]
    fn writing_a_signal_inside_its_own_read_panics_in_debug() {
        let signal = Signal::named("count", 0_u32);
        signal.with(|_| signal.set(1));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn nested_reads_and_other_signals_are_allowed() {
        let first = Signal::new(1_u32);
        let second = Signal::new(2_u32);
        let sum = first.with(|a| first.with(|b| a + b) + second.with(|c| *c));
        assert_eq!(sum, 4);
        first.modify(|value| {
            second.set(*value);
            true
        });
        assert_eq!(second.get(), 1);
        // The lock record is cleared after a caught panic.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            first.with(|_| first.set(9));
        }));
        assert!(caught.is_err());
        assert!(first.set(3));
    }

    #[test]
    fn mixed_concurrent_use_neither_deadlocks_nor_loses_the_final_value() {
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let a = Signal::named("a", 0_u32);
            let b = Signal::named("b", 0_u32);
            let parity = a.select(|value| value % 2);
            let sum = combine((parity.clone(), b.clone()), |(parity, b)| parity + b);
            let task: Task<u32> = Task::new();
            let workers = (0..4_u32)
                .map(|thread| {
                    let (a, b, parity, sum, task) = (
                        a.clone(),
                        b.clone(),
                        parity.clone(),
                        sum.clone(),
                        task.clone(),
                    );
                    std::thread::spawn(move || {
                        for step in 0..300_u32 {
                            match (thread + step) % 5 {
                                0 => {
                                    a.set(step);
                                }
                                1 => batch(|| {
                                    a.update(|value| *value += 1);
                                    b.set(step % 7);
                                }),
                                2 => {
                                    // Subscription churn on shared selectors.
                                    let _parity = parity.subscribe(Observer::new(|_| {}));
                                    let _sum = sum.subscribe(Observer::new(|_| {}));
                                    let _ = sum.get();
                                }
                                3 => {
                                    let handle = task.refresh();
                                    handle.set_progress(0.5);
                                    handle.finish(step);
                                }
                                _ => {
                                    let _ = (parity.get(), sum.get(), task.progress());
                                }
                            }
                        }
                    })
                })
                .collect::<Vec<_>>();
            let latest = Arc::new(Mutex::new(None));
            let observed = Arc::clone(&latest);
            let reader = sum.clone();
            let _watch = sum.subscribe(Observer::new(move |_| {
                *observed.lock().unwrap() = Some(reader.get());
            }));
            for worker in workers {
                worker.join().unwrap();
            }
            // Quiescent: a final write must reach a live observer.
            a.set(1_001);
            b.set(41);
            let expected = 1 + 41;
            done.send((sum.get(), *latest.lock().unwrap(), expected))
                .unwrap();
        });
        let (sum, latest, expected) = finished
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("concurrent signal use deadlocked");
        assert_eq!(sum, expected);
        assert_eq!(latest, Some(expected));
    }
}
