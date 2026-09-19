//! Window-owned overflow cache for exact scalar measurement queries.
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use lru::LruCache;
use sui_core::{Size, WidgetId};
use sui_layout::{Axis, Constraints};

// Fixed-size keys/values plus a conservative charge for the LRU node, hash
// bucket, and allocation overhead. Storage grows on demand, never up front.
const ENTRY_BYTES: usize = 128;
const BUDGET_BYTES: usize = 8 * 1024 * 1024;
static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryContext(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum QueryKind {
    Size,
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct QueryKey {
    widget: WidgetId,
    revision: u64,
    constraints: [u32; 4],
    kind: QueryKind,
}

impl QueryKey {
    pub(crate) fn with_axis(mut self, axis: Option<Axis>) -> Self {
        self.kind = match axis {
            None => QueryKind::Size,
            Some(Axis::Horizontal) => QueryKind::Width,
            Some(Axis::Vertical) => QueryKind::Height,
        };
        self
    }

    pub(crate) fn new(widget: WidgetId, revision: u64, constraints: Constraints) -> Option<Self> {
        let values = [
            constraints.min.width,
            constraints.min.height,
            constraints.max.width,
            constraints.max.height,
        ];
        if values.iter().any(|v| v.is_nan()) {
            return None;
        }
        Some(Self {
            widget,
            revision,
            kind: QueryKind::Size,
            constraints: values.map(|v| if v == 0.0 { 0 } else { v.to_bits() }),
        })
    }
}

pub(crate) type SharedQueryCache = Rc<RefCell<MeasureQueryCache>>;

#[derive(Debug)]
pub(crate) struct MeasureQueryCache {
    entries: LruCache<QueryKey, Size>,
    limit: usize,
    context: QueryContext,
}

impl Default for MeasureQueryCache {
    fn default() -> Self {
        Self::with_budget(BUDGET_BYTES)
    }
}

impl MeasureQueryCache {
    fn with_budget(bytes: usize) -> Self {
        Self {
            entries: LruCache::unbounded(),
            limit: bytes / ENTRY_BYTES,
            context: QueryContext(NEXT_CONTEXT.fetch_add(1, Ordering::Relaxed)),
        }
    }

    pub(crate) fn shared() -> SharedQueryCache {
        Rc::new(RefCell::new(Self::default()))
    }
    pub(crate) fn context(&self) -> QueryContext {
        self.context
    }

    /// Window/font/viewport invalidation also invalidates local pod caches,
    /// including widgets that were not visited during the invalidating pass.
    pub(crate) fn invalidate_context(&mut self) {
        self.entries.clear();
        self.context = QueryContext(NEXT_CONTEXT.fetch_add(1, Ordering::Relaxed));
    }

    pub(crate) fn get(&mut self, context: QueryContext, key: &QueryKey) -> Option<Size> {
        if context != self.context {
            return None;
        }
        self.entries.get(key).copied()
    }

    pub(crate) fn put(&mut self, context: QueryContext, key: QueryKey, size: Size) {
        if context != self.context || self.limit == 0 {
            return;
        }
        if self.entries.len() >= self.limit && !self.entries.contains(&key) {
            self.entries.pop_lru();
        }
        self.entries.put(key, size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_keys_are_exact_and_revision_scoped() {
        let widget = WidgetId::new(1);
        let key = |revision, width| {
            QueryKey::new(widget, revision, Constraints::tight(Size::new(width, 10.0)))
        };
        assert_eq!(key(1, 0.0), key(1, -0.0));
        assert_ne!(key(1, 10.0), key(2, 10.0));
        assert_ne!(key(1, 10.0), key(1, f32::from_bits(10.0f32.to_bits() + 1)));
        assert!(key(1, f32::NAN).is_none());
    }

    #[test]
    fn budget_evicts_least_recent_queries_and_context_invalidation_drops_results() {
        let mut cache = MeasureQueryCache::with_budget(ENTRY_BYTES * 2);
        let key = |id| QueryKey::new(WidgetId::new(id), 1, Constraints::UNBOUNDED).unwrap();
        cache.put(cache.context(), key(1), Size::new(1.0, 1.0));
        cache.put(cache.context(), key(2), Size::new(2.0, 2.0));
        assert!(cache.get(cache.context(), &key(1)).is_some());
        cache.put(cache.context(), key(3), Size::ZERO);
        assert!(cache.get(cache.context(), &key(2)).is_none());
        assert_eq!(cache.entries.len() * ENTRY_BYTES, ENTRY_BYTES * 2);
        let context = cache.context();
        cache.invalidate_context();
        assert_ne!(cache.context(), context);
        assert!(cache.get(cache.context(), &key(1)).is_none());
        cache.put(context, key(1), Size::ZERO);
        assert!(cache.get(cache.context(), &key(1)).is_none());
    }
}
