use std::sync::Arc;

use crate::{Observable, Observer, Selector, SourceId, Subscription};

/// Several observables viewed as one tuple-valued source. Built by
/// [`combine`]; observing it directly notifies on a change to any input.
pub struct Zip<S> {
    id: SourceId,
    name: Arc<str>,
    sources: S,
}

/// Derive a value from up to four observables.
///
/// ```ignore
/// let summary = combine((items.clone(), filter.clone()), |(items, filter)| {
///     items.iter().filter(|item| filter.matches(item)).count()
/// });
/// ```
///
/// The result is a [`Selector`]: it notifies only when the derived value
/// changes, runs `combine` once per input change however many observers it
/// has, and caches the result while no input changes. Unlike a selector over
/// one source, each input is cloned to build the tuple, because holding
/// several inputs' locks at once could deadlock against writers. Combine
/// narrow selectors or `Arc` values rather than large state.
pub fn combine<S, I, O>(
    sources: S,
    combine: impl Fn(&I) -> O + Send + Sync + 'static,
) -> Selector<Zip<S>, I, O>
where
    Zip<S>: Observable<I>,
{
    combine_named("Combined", sources, combine)
}

/// [`combine`] with a diagnostics name.
pub fn combine_named<S, I, O>(
    name: impl Into<Arc<str>>,
    sources: S,
    combine: impl Fn(&I) -> O + Send + Sync + 'static,
) -> Selector<Zip<S>, I, O>
where
    Zip<S>: Observable<I>,
{
    let name = name.into();
    let zip = Zip {
        id: SourceId::new(),
        name: Arc::clone(&name),
        sources,
    };
    Selector::new(name, zip, combine)
}

macro_rules! impl_zip {
    ($(($source:ident, $value:ident, $index:tt)),+) => {
        impl<$($source, $value),+> Observable<($($value,)+)> for Zip<($($source,)+)>
        where
            $($source: Observable<$value>,)+
        {
            fn source_id(&self) -> SourceId {
                self.id
            }

            fn source_name(&self) -> Arc<str> {
                Arc::clone(&self.name)
            }

            fn get(&self) -> ($($value,)+) {
                ($(self.sources.$index.get(),)+)
            }

            fn value_version(&self) -> Option<u64> {
                // Every input change raises the sum, and each input's version
                // is read before its value, so the sum is a valid version.
                let mut version = 0_u64;
                $(version = version.checked_add(self.sources.$index.value_version()?)?;)+
                Some(version)
            }

            fn subscribe(&self, observer: Observer) -> Subscription {
                let inputs = vec![$(self.sources.$index.subscribe(observer.clone())),+];
                Subscription::with_release(observer, inputs)
            }
        }
    };
}

impl_zip!((SA, A, 0), (SB, B, 1));
impl_zip!((SA, A, 0), (SB, B, 1), (SC, C, 2));
impl_zip!((SA, A, 0), (SB, B, 1), (SC, C, 2), (SD, D, 3));

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::{Observable, Observer, Signal, batch, combine, combine_named};

    #[test]
    fn combine_derives_from_several_sources() {
        let count = Signal::new(2_u32);
        let label = Signal::new("apples".to_string());
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let summary = combine((count.clone(), label.clone()), move |(count, label)| {
            counter.fetch_add(1, Ordering::Relaxed);
            format!("{count} {label}")
        });
        let notifications = Arc::new(AtomicUsize::new(0));
        let notified = Arc::clone(&notifications);
        let _subscription = summary.subscribe(Observer::new(move |_| {
            notified.fetch_add(1, Ordering::Relaxed);
        }));

        assert_eq!(summary.get(), "2 apples");
        count.set(3);
        label.set("pears".to_string());
        assert_eq!(summary.get(), "3 pears");
        assert_eq!(notifications.load(Ordering::Relaxed), 2);

        let before = runs.load(Ordering::Relaxed);
        batch(|| {
            count.set(4);
            label.set("plums".to_string());
        });
        // Each changed input notifies the selector, but both inputs had
        // changed by the first notification: the second hits the cache and is
        // deduplicated.
        assert_eq!(notifications.load(Ordering::Relaxed), 3);
        assert_eq!(summary.get(), "4 plums");
        assert_eq!(runs.load(Ordering::Relaxed) - before, 1);
    }

    #[test]
    fn combine_accepts_selectors_and_four_inputs() {
        let a = Signal::new((1_u8, "unused"));
        let first = a.select(|(value, _)| *value);
        let b = Signal::new(2_u8);
        let c = Signal::new(3_u8);
        let d = Signal::new(4_u8);
        let sum = combine_named("sum", (first, b, c, d.clone()), |(a, b, c, d)| {
            a + b + c + d
        });
        assert_eq!(sum.get(), 10);
        d.set(5);
        assert_eq!(sum.get(), 11);
        assert_eq!(&*sum.source_name(), "sum");
    }
}
