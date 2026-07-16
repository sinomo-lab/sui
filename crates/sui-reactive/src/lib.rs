#![forbid(unsafe_code)]

//! Architecture-neutral observable values for SUI.
//!
//! This crate owns state and change notification only. It does not depend on
//! the SUI widget runtime, so applications may adapt other stores through
//! [`Observable`] without adopting a SUI-owned application architecture.

use std::{
    fmt,
    marker::PhantomData,
    sync::{
        Arc, Mutex, RwLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

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
    _observer: Observer,
}

impl Subscription {
    pub fn new(observer: Observer) -> Self {
        Self {
            _observer: observer,
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
pub trait Observable<T> {
    fn source_id(&self) -> SourceId;
    fn source_name(&self) -> Arc<str>;
    fn get(&self) -> T;
    fn subscribe(&self, observer: Observer) -> Subscription;
}

struct SignalInner<T> {
    id: SourceId,
    name: Arc<str>,
    value: RwLock<T>,
    version: AtomicU64,
    observers: Mutex<Vec<Weak<ObserverInner>>>,
}

/// Cloneable observable storage with equality-deduplicated writes.
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
    T: fmt::Debug + Clone,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Signal")
            .field("id", &self.inner.id)
            .field("name", &self.inner.name)
            .field("value", &self.read_value())
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
                id: SourceId::new(),
                name: name.into(),
                value: RwLock::new(value),
                version: AtomicU64::new(0),
                observers: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn source_id(&self) -> SourceId {
        self.inner.id
    }

    pub fn source_name(&self) -> Arc<str> {
        Arc::clone(&self.inner.name)
    }

    pub fn version(&self) -> u64 {
        self.inner.version.load(Ordering::Acquire)
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read_value()
    }

    pub fn set(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        {
            let mut current = self
                .inner
                .value
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *current == value {
                return false;
            }
            *current = value;
        }
        self.notify();
        true
    }

    pub fn update(&self, update: impl FnOnce(&mut T)) -> bool
    where
        T: Clone + PartialEq,
    {
        {
            let mut current = self
                .inner
                .value
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = current.clone();
            update(&mut current);
            if *current == previous {
                return false;
            }
        }
        self.notify();
        true
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

    fn read_value(&self) -> T
    where
        T: Clone,
    {
        self.inner
            .value
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn notify(&self) {
        let version = self.inner.version.fetch_add(1, Ordering::AcqRel) + 1;
        let change = Change {
            source_id: self.inner.id,
            source_name: Arc::clone(&self.inner.name),
            version,
        };
        let observers = {
            let mut observers = self
                .inner
                .observers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    fn subscribe(&self, observer: Observer) -> Subscription {
        self.inner
            .observers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Arc::downgrade(&observer.inner));
        Subscription::new(observer)
    }
}

struct SelectorInner<S, I, O> {
    id: SourceId,
    name: Arc<str>,
    source: S,
    select: Arc<dyn Fn(&I) -> O + Send + Sync>,
    version: AtomicU64,
    _input: PhantomData<fn(I)>,
}

/// Equality-deduplicated derived observable.
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
        Self {
            inner: Arc::new(SelectorInner {
                id: SourceId::new(),
                name: name.into(),
                source,
                select: Arc::new(select),
                version: AtomicU64::new(0),
                _input: PhantomData,
            }),
        }
    }
}

impl<S, I, O> Observable<O> for Selector<S, I, O>
where
    S: Observable<I> + Clone + Send + Sync + 'static,
    I: Clone + Send + Sync + 'static,
    O: Clone + PartialEq + Send + Sync + 'static,
{
    fn source_id(&self) -> SourceId {
        self.inner.id
    }

    fn source_name(&self) -> Arc<str> {
        Arc::clone(&self.inner.name)
    }

    fn get(&self) -> O {
        (self.inner.select)(&self.inner.source.get())
    }

    fn subscribe(&self, observer: Observer) -> Subscription {
        let last = Arc::new(Mutex::new(self.get()));
        let source = self.inner.source.clone();
        let select = Arc::clone(&self.inner.select);
        let selector = Arc::clone(&self.inner);
        let source_observer = Observer::new(move |_change| {
            let next = select(&source.get());
            let mut previous = last
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *previous == next {
                return;
            }
            *previous = next;
            let version = selector.version.fetch_add(1, Ordering::AcqRel) + 1;
            observer.notify(Change {
                source_id: selector.id,
                source_name: Arc::clone(&selector.name),
                version,
            });
        });
        self.inner.source.subscribe(source_observer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn signal_notifies_only_for_meaningful_changes() {
        let signal = Signal::named("count", 1usize);
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let _subscription = signal.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));

        assert!(!signal.set(1));
        assert!(signal.set(2));
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(signal.version(), 1);
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
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notifications);
        let _subscription = selected.subscribe(Observer::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        }));

        state.update(|state| state.detail = "second".to_string());
        assert_eq!(notifications.load(Ordering::Relaxed), 0);
        state.update(|state| state.selected = 1);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
        assert_eq!(selected.get(), 1);
    }
}
