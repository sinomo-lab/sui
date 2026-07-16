use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    hash::Hash,
    ops::Range,
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use sui_core::{
    Event, InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, Point,
    PointerButton, PointerEvent, PointerEventKind, PointerKind, Rect, ScrollDelta, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Size, Vector, WidgetId,
};
use sui_layout::{Constraints, Padding as Insets};
use sui_reactive::{Observable, Observer, Signal, SourceId, Subscription};
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, LayerOptions, MeasureCtx, PaintBoundaryMode, PaintCtx,
    SemanticsCtx, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::LayerCompositionMode;

use crate::{
    DefaultTheme,
    containers::{
        OverlayScrollBars, ScrollAxes, ScrollInvalidationCtx, ScrollState, ScrollWidgetCtx,
        request_scroll_bar_refresh,
    },
    data::{draw_surface, paint_data_row_state},
};

const COLLECTION_JOURNAL_LIMIT: usize = 512;
const DEFAULT_CACHE_CAPACITY: usize = 64;
const DEFAULT_OVERSCAN_VIEWPORTS: f32 = 0.75;
const DEFAULT_UNBOUNDED_VIEWPORT_HEIGHT: f32 = 420.0;
const FOLLOW_END_THRESHOLD: f32 = 24.0;
const SYNTHETIC_COLLECTION_ITEM_TAG: u64 = 6_u64 << 60;

static NEXT_COLLECTION_ITEM_ID: AtomicU64 = AtomicU64::new(1);

/// Incremental mutation accepted by [`VirtualCollectionModel`].
#[derive(Clone, Debug, PartialEq)]
pub enum CollectionChange<K, T> {
    Insert { index: usize, items: Vec<(K, T)> },
    Remove { keys: Vec<K> },
    Move { key: K, index: usize },
    Update { key: K, item: T },
    Reset { items: Vec<(K, T)> },
}

/// Key-only change journal consumed by virtual collection widgets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionDelta<K> {
    Inserted { index: usize, keys: Vec<K> },
    Removed { keys: Vec<K> },
    Moved { key: K, index: usize },
    Updated { key: K },
}

/// Result of asking a collection source for changes after a known revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionSync<K> {
    Unchanged {
        revision: u64,
    },
    Incremental {
        revision: u64,
        changes: Vec<CollectionDelta<K>>,
    },
    Reset {
        revision: u64,
        keys: Vec<K>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionModelError {
    DuplicateKey,
    MissingKey,
    IndexOutOfBounds { index: usize, len: usize },
}

impl fmt::Display for CollectionModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey => formatter.write_str("collection keys must be unique"),
            Self::MissingKey => formatter.write_str("collection key was not found"),
            Self::IndexOutOfBounds { index, len } => {
                write!(formatter, "collection index {index} exceeds length {len}")
            }
        }
    }
}

impl std::error::Error for CollectionModelError {}

/// Architecture-neutral data source consumed by [`VirtualList`].
///
/// Sources without an incremental journal may always return
/// [`CollectionSync::Reset`]. The default [`VirtualCollectionModel`] keeps a
/// bounded key-only journal so normal insert, remove, move, and update
/// operations do not require rebuilding collection metrics.
pub trait VirtualCollectionSource<K, T>: Observable<u64> {
    fn revision(&self) -> u64;
    fn keys(&self) -> Vec<K>;
    fn item(&self, key: &K) -> Option<T>;
    fn changes_since(&self, revision: u64) -> CollectionSync<K>;
}

#[derive(Clone)]
struct CollectionJournalEntry<K> {
    revision: u64,
    delta: Option<CollectionDelta<K>>,
    reset: bool,
}

struct CollectionModelInner<K, T> {
    entries: Vec<(K, T)>,
    index_by_key: HashMap<K, usize>,
    revision: u64,
    journal: VecDeque<CollectionJournalEntry<K>>,
}

/// Cloneable observable collection with stable keys and incremental changes.
pub struct VirtualCollectionModel<K, T> {
    inner: Arc<RwLock<CollectionModelInner<K, T>>>,
    revision: Signal<u64>,
}

impl<K, T> Clone for VirtualCollectionModel<K, T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            revision: self.revision.clone(),
        }
    }
}

impl<K, T> fmt::Debug for VirtualCollectionModel<K, T>
where
    K: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        formatter
            .debug_struct("VirtualCollectionModel")
            .field("entries", &inner.entries)
            .field("revision", &inner.revision)
            .finish()
    }
}

impl<K, T> VirtualCollectionModel<K, T>
where
    K: Clone + Eq + Hash,
    T: Clone + PartialEq,
{
    pub fn new() -> Self {
        Self::named("Virtual collection")
    }

    pub fn named(name: impl Into<Arc<str>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(CollectionModelInner {
                entries: Vec::new(),
                index_by_key: HashMap::new(),
                revision: 0,
                journal: VecDeque::new(),
            })),
            revision: Signal::named(name, 0),
        }
    }

    pub fn from_items(
        name: impl Into<Arc<str>>,
        items: impl IntoIterator<Item = (K, T)>,
    ) -> Result<Self, CollectionModelError> {
        let model = Self::named(name);
        model.apply(CollectionChange::Reset {
            items: items.into_iter().collect(),
        })?;
        Ok(model)
    }

    pub fn len(&self) -> usize {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn append(&self, key: K, item: T) -> Result<bool, CollectionModelError> {
        let index = self.len();
        self.apply(CollectionChange::Insert {
            index,
            items: vec![(key, item)],
        })
    }

    pub fn prepend(
        &self,
        items: impl IntoIterator<Item = (K, T)>,
    ) -> Result<bool, CollectionModelError> {
        self.apply(CollectionChange::Insert {
            index: 0,
            items: items.into_iter().collect(),
        })
    }

    pub fn remove(&self, key: K) -> Result<bool, CollectionModelError> {
        self.apply(CollectionChange::Remove { keys: vec![key] })
    }

    pub fn update(&self, key: K, item: T) -> Result<bool, CollectionModelError> {
        self.apply(CollectionChange::Update { key, item })
    }

    pub fn move_to(&self, key: K, index: usize) -> Result<bool, CollectionModelError> {
        self.apply(CollectionChange::Move { key, index })
    }

    pub fn replace(
        &self,
        items: impl IntoIterator<Item = (K, T)>,
    ) -> Result<bool, CollectionModelError> {
        self.apply(CollectionChange::Reset {
            items: items.into_iter().collect(),
        })
    }

    pub fn apply(&self, change: CollectionChange<K, T>) -> Result<bool, CollectionModelError> {
        let next_revision = {
            let mut inner = self
                .inner
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            let journal = match change {
                CollectionChange::Insert { index, items } => {
                    if index > inner.entries.len() {
                        return Err(CollectionModelError::IndexOutOfBounds {
                            index,
                            len: inner.entries.len(),
                        });
                    }
                    if items.is_empty() {
                        return Ok(false);
                    }
                    let mut inserted = HashSet::with_capacity(items.len());
                    if items.iter().any(|(key, _)| {
                        !inserted.insert(key.clone()) || inner.index_by_key.contains_key(key)
                    }) {
                        return Err(CollectionModelError::DuplicateKey);
                    }
                    let keys = items.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
                    inner.entries.splice(index..index, items);
                    Self::reindex(&mut inner);
                    CollectionJournalEntry {
                        revision: 0,
                        delta: Some(CollectionDelta::Inserted { index, keys }),
                        reset: false,
                    }
                }
                CollectionChange::Remove { keys } => {
                    if keys.is_empty() {
                        return Ok(false);
                    }
                    let unique = keys.iter().cloned().collect::<HashSet<_>>();
                    if unique.len() != keys.len() {
                        return Err(CollectionModelError::DuplicateKey);
                    }
                    if keys.iter().any(|key| !inner.index_by_key.contains_key(key)) {
                        return Err(CollectionModelError::MissingKey);
                    }
                    inner.entries.retain(|(key, _)| !unique.contains(key));
                    Self::reindex(&mut inner);
                    CollectionJournalEntry {
                        revision: 0,
                        delta: Some(CollectionDelta::Removed { keys }),
                        reset: false,
                    }
                }
                CollectionChange::Move { key, index } => {
                    let Some(from) = inner.index_by_key.get(&key).copied() else {
                        return Err(CollectionModelError::MissingKey);
                    };
                    let target_len = inner.entries.len().saturating_sub(1);
                    if index > target_len {
                        return Err(CollectionModelError::IndexOutOfBounds {
                            index,
                            len: target_len,
                        });
                    }
                    if from == index {
                        return Ok(false);
                    }
                    let entry = inner.entries.remove(from);
                    inner.entries.insert(index, entry);
                    Self::reindex(&mut inner);
                    CollectionJournalEntry {
                        revision: 0,
                        delta: Some(CollectionDelta::Moved { key, index }),
                        reset: false,
                    }
                }
                CollectionChange::Update { key, item } => {
                    let Some(index) = inner.index_by_key.get(&key).copied() else {
                        return Err(CollectionModelError::MissingKey);
                    };
                    if inner.entries[index].1 == item {
                        return Ok(false);
                    }
                    inner.entries[index].1 = item;
                    CollectionJournalEntry {
                        revision: 0,
                        delta: Some(CollectionDelta::Updated { key }),
                        reset: false,
                    }
                }
                CollectionChange::Reset { items } => {
                    let mut keys = HashSet::with_capacity(items.len());
                    if items.iter().any(|(key, _)| !keys.insert(key.clone())) {
                        return Err(CollectionModelError::DuplicateKey);
                    }
                    if inner.entries == items {
                        return Ok(false);
                    }
                    inner.entries = items;
                    Self::reindex(&mut inner);
                    CollectionJournalEntry {
                        revision: 0,
                        delta: None,
                        reset: true,
                    }
                }
            };

            inner.revision = inner.revision.wrapping_add(1);
            let revision = inner.revision;
            inner.journal.push_back(CollectionJournalEntry {
                revision,
                ..journal
            });
            while inner.journal.len() > COLLECTION_JOURNAL_LIMIT {
                inner.journal.pop_front();
            }
            revision
        };

        let _ = self.revision.set(next_revision);
        Ok(true)
    }

    fn reindex(inner: &mut CollectionModelInner<K, T>) {
        inner.index_by_key.clear();
        inner.index_by_key.extend(
            inner
                .entries
                .iter()
                .enumerate()
                .map(|(index, (key, _))| (key.clone(), index)),
        );
    }
}

impl<K, T> Default for VirtualCollectionModel<K, T>
where
    K: Clone + Eq + Hash,
    T: Clone + PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, T> Observable<u64> for VirtualCollectionModel<K, T>
where
    K: Clone + Eq + Hash + 'static,
    T: Clone + PartialEq + 'static,
{
    fn source_id(&self) -> SourceId {
        self.revision.source_id()
    }

    fn source_name(&self) -> Arc<str> {
        self.revision.source_name()
    }

    fn get(&self) -> u64 {
        self.revision.get()
    }

    fn subscribe(&self, observer: Observer) -> Subscription {
        self.revision.subscribe(observer)
    }
}

impl<K, T> VirtualCollectionSource<K, T> for VirtualCollectionModel<K, T>
where
    K: Clone + Eq + Hash + 'static,
    T: Clone + PartialEq + 'static,
{
    fn revision(&self) -> u64 {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .revision
    }

    fn keys(&self) -> Vec<K> {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .iter()
            .map(|(key, _)| key.clone())
            .collect()
    }

    fn item(&self, key: &K) -> Option<T> {
        let inner = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner
            .index_by_key
            .get(key)
            .and_then(|index| inner.entries.get(*index))
            .map(|(_, item)| item.clone())
    }

    fn changes_since(&self, revision: u64) -> CollectionSync<K> {
        let inner = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if revision == inner.revision {
            return CollectionSync::Unchanged {
                revision: inner.revision,
            };
        }

        let Some(oldest) = inner.journal.front().map(|entry| entry.revision) else {
            return CollectionSync::Reset {
                revision: inner.revision,
                keys: inner.entries.iter().map(|(key, _)| key.clone()).collect(),
            };
        };
        if revision.wrapping_add(1) < oldest {
            return CollectionSync::Reset {
                revision: inner.revision,
                keys: inner.entries.iter().map(|(key, _)| key.clone()).collect(),
            };
        }

        let entries = inner
            .journal
            .iter()
            .filter(|entry| entry.revision > revision)
            .collect::<Vec<_>>();
        if entries.iter().any(|entry| entry.reset) {
            return CollectionSync::Reset {
                revision: inner.revision,
                keys: inner.entries.iter().map(|(key, _)| key.clone()).collect(),
            };
        }

        CollectionSync::Incremental {
            revision: inner.revision,
            changes: entries
                .into_iter()
                .filter_map(|entry| entry.delta.clone())
                .collect(),
        }
    }
}

type ExtentLink = Option<Box<ExtentNode>>;

struct ExtentNode {
    extent: f32,
    priority: u64,
    len: usize,
    sum: f32,
    left: ExtentLink,
    right: ExtentLink,
}

impl ExtentNode {
    fn new(extent: f32, priority: u64) -> Box<Self> {
        Box::new(Self {
            extent: extent.max(0.0),
            priority,
            len: 1,
            sum: extent.max(0.0),
            left: None,
            right: None,
        })
    }

    fn update(&mut self) {
        self.len = 1 + link_len(&self.left) + link_len(&self.right);
        self.sum = self.extent + link_sum(&self.left) + link_sum(&self.right);
    }
}

fn link_len(link: &ExtentLink) -> usize {
    link.as_ref().map_or(0, |node| node.len)
}

fn link_sum(link: &ExtentLink) -> f32 {
    link.as_ref().map_or(0.0, |node| node.sum)
}

fn split_extent(root: ExtentLink, left_len: usize) -> (ExtentLink, ExtentLink) {
    let Some(mut root) = root else {
        return (None, None);
    };
    let root_left_len = link_len(&root.left);
    if left_len <= root_left_len {
        let (left, right) = split_extent(root.left.take(), left_len);
        root.left = right;
        root.update();
        (left, Some(root))
    } else {
        let (left, right) = split_extent(
            root.right.take(),
            left_len.saturating_sub(root_left_len + 1),
        );
        root.right = left;
        root.update();
        (Some(root), right)
    }
}

fn merge_extent(left: ExtentLink, right: ExtentLink) -> ExtentLink {
    match (left, right) {
        (None, right) => right,
        (left, None) => left,
        (Some(mut left), Some(mut right)) => {
            if left.priority <= right.priority {
                left.right = merge_extent(left.right.take(), Some(right));
                left.update();
                Some(left)
            } else {
                right.left = merge_extent(Some(left), right.left.take());
                right.update();
                Some(right)
            }
        }
    }
}

/// Variable-height prefix-sum index used by virtual collection policies.
///
/// It supports offset lookup, prefix sums, insertion, removal, movement, and
/// extent correction in expected logarithmic time.
#[derive(Default)]
pub struct CollectionExtentIndex {
    root: ExtentLink,
    next_priority: u64,
}

impl fmt::Debug for CollectionExtentIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionExtentIndex")
            .field("len", &self.len())
            .field("total", &self.total())
            .finish()
    }
}

impl CollectionExtentIndex {
    pub const fn new() -> Self {
        Self {
            root: None,
            next_priority: 0,
        }
    }

    pub fn len(&self) -> usize {
        link_len(&self.root)
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn total(&self) -> f32 {
        link_sum(&self.root)
    }

    pub fn clear(&mut self) {
        self.root = None;
    }

    pub fn rebuild(&mut self, extents: impl IntoIterator<Item = f32>) {
        self.clear();
        for extent in extents {
            self.insert(self.len(), extent);
        }
    }

    pub fn insert(&mut self, index: usize, extent: f32) {
        let index = index.min(self.len());
        let priority = splitmix64(self.next_priority);
        self.next_priority = self.next_priority.wrapping_add(1);
        let (left, right) = split_extent(self.root.take(), index);
        self.root = merge_extent(
            merge_extent(left, Some(ExtentNode::new(extent, priority))),
            right,
        );
    }

    pub fn insert_many(&mut self, index: usize, extents: impl IntoIterator<Item = f32>) {
        let index = index.min(self.len());
        for (offset, extent) in extents.into_iter().enumerate() {
            self.insert(index + offset, extent);
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<f32> {
        if index >= self.len() {
            return None;
        }
        let (left, rest) = split_extent(self.root.take(), index);
        let (removed, right) = split_extent(rest, 1);
        self.root = merge_extent(left, right);
        removed.map(|node| node.extent)
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.len() || to >= self.len() || from == to {
            return false;
        }
        let Some(extent) = self.remove(from) else {
            return false;
        };
        self.insert(to, extent);
        true
    }

    pub fn update(&mut self, index: usize, extent: f32) -> bool {
        fn update_at(node: &mut ExtentLink, index: usize, extent: f32) -> bool {
            let Some(node) = node else {
                return false;
            };
            let left_len = link_len(&node.left);
            let changed = if index < left_len {
                update_at(&mut node.left, index, extent)
            } else if index == left_len {
                let extent = extent.max(0.0);
                if (node.extent - extent).abs() <= f32::EPSILON {
                    false
                } else {
                    node.extent = extent;
                    true
                }
            } else {
                update_at(&mut node.right, index - left_len - 1, extent)
            };
            if changed {
                node.update();
            }
            changed
        }

        update_at(&mut self.root, index, extent)
    }

    pub fn extent(&self, index: usize) -> Option<f32> {
        let mut node = self.root.as_deref();
        let mut index = index;
        while let Some(current) = node {
            let left_len = link_len(&current.left);
            if index < left_len {
                node = current.left.as_deref();
            } else if index == left_len {
                return Some(current.extent);
            } else {
                index -= left_len + 1;
                node = current.right.as_deref();
            }
        }
        None
    }

    pub fn offset_of(&self, index: usize) -> f32 {
        fn prefix(node: &ExtentLink, index: usize) -> f32 {
            let Some(node) = node else {
                return 0.0;
            };
            let left_len = link_len(&node.left);
            if index <= left_len {
                prefix(&node.left, index)
            } else {
                link_sum(&node.left) + node.extent + prefix(&node.right, index - left_len - 1)
            }
        }

        prefix(&self.root, index.min(self.len()))
    }

    pub fn index_at_offset(&self, offset: f32) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        let mut offset = offset.max(0.0).min((self.total() - f32::EPSILON).max(0.0));
        let mut base = 0;
        let mut node = self.root.as_deref();
        while let Some(current) = node {
            let left_sum = link_sum(&current.left);
            let left_len = link_len(&current.left);
            if offset < left_sum {
                node = current.left.as_deref();
            } else if offset < left_sum + current.extent {
                return Some(base + left_len);
            } else {
                offset -= left_sum + current.extent;
                base += left_len + 1;
                node = current.right.as_deref();
            }
        }
        Some(self.len() - 1)
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollectionAnchorGravity {
    #[default]
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollectionAnchor<K> {
    pub key: K,
    pub offset_within_item: f32,
    pub gravity: CollectionAnchorGravity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAlignment {
    #[default]
    Nearest,
    Start,
    Center,
    End,
}

#[derive(Clone)]
pub struct VirtualListState<K> {
    inner: Rc<RefCell<VirtualListStateInner<K>>>,
    revision: Signal<u64>,
    scroll: ScrollState,
}

struct VirtualListStateInner<K> {
    selected: Option<K>,
    pending_scroll: Option<(K, ScrollAlignment)>,
    pinned: HashSet<K>,
    follow_end_enabled: bool,
    following_end: bool,
}

impl<K> VirtualListState<K>
where
    K: Clone + Eq + Hash + PartialEq + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(VirtualListStateInner {
                selected: None,
                pending_scroll: None,
                pinned: HashSet::new(),
                follow_end_enabled: false,
                following_end: false,
            })),
            revision: Signal::named("Virtual list state", 0),
            scroll: ScrollState::new(),
        }
    }

    pub fn selected_key(&self) -> Option<K> {
        self.inner.borrow().selected.clone()
    }

    pub fn select(&self, key: Option<K>) -> bool {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            if inner.selected == key {
                false
            } else {
                inner.selected = key;
                true
            }
        };
        if changed {
            self.bump();
        }
        changed
    }

    pub fn scroll_to(&self, key: K, alignment: ScrollAlignment) {
        self.inner.borrow_mut().pending_scroll = Some((key, alignment));
        self.bump();
    }

    pub fn pin(&self, key: K) -> bool {
        let changed = self.inner.borrow_mut().pinned.insert(key);
        if changed {
            self.bump();
        }
        changed
    }

    pub fn unpin(&self, key: &K) -> bool {
        let changed = self.inner.borrow_mut().pinned.remove(key);
        if changed {
            self.bump();
        }
        changed
    }

    pub fn set_follow_end(&self, enabled: bool) {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            if inner.follow_end_enabled == enabled && (!enabled || inner.following_end == enabled) {
                false
            } else {
                inner.follow_end_enabled = enabled;
                inner.following_end = enabled;
                true
            }
        };
        if changed {
            self.bump();
        }
    }

    pub fn resume_follow_end(&self) {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            if !inner.follow_end_enabled || inner.following_end {
                false
            } else {
                inner.following_end = true;
                true
            }
        };
        if changed {
            self.bump();
        }
    }

    pub fn is_following_end(&self) -> bool {
        let inner = self.inner.borrow();
        inner.follow_end_enabled && inner.following_end
    }

    pub fn scroll_state(&self) -> ScrollState {
        self.scroll.clone()
    }

    fn bump(&self) {
        let _ = self.revision.update(|revision| {
            *revision = revision.wrapping_add(1);
        });
    }

    fn take_pending_scroll(&self) -> Option<(K, ScrollAlignment)> {
        self.inner.borrow_mut().pending_scroll.take()
    }

    fn pinned(&self) -> HashSet<K> {
        self.inner.borrow().pinned.clone()
    }

    fn update_following_for_offset(&self, offset: f32, max_offset: f32) {
        let mut inner = self.inner.borrow_mut();
        if inner.follow_end_enabled {
            inner.following_end = max_offset - offset <= FOLLOW_END_THRESHOLD;
        }
    }

    fn clear_removed_selection(&self, live_keys: &HashSet<K>) {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            if inner
                .selected
                .as_ref()
                .is_some_and(|selected| !live_keys.contains(selected))
            {
                inner.selected = None;
                true
            } else {
                false
            }
        };
        if changed {
            self.bump();
        }
    }
}

impl<K> Default for VirtualListState<K>
where
    K: Clone + Eq + Hash + PartialEq + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

struct RealizedRow<T> {
    value: Signal<T>,
    pod: WidgetPod,
    last_used: u64,
}

#[derive(Clone, Copy)]
struct TouchGesture {
    pointer_id: u64,
    origin: Point,
    dragging: bool,
}

impl TouchGesture {
    fn new(pointer: &PointerEvent) -> Self {
        Self {
            pointer_id: pointer.pointer_id,
            origin: pointer.position,
            dragging: false,
        }
    }

    fn matches(self, pointer: &PointerEvent) -> bool {
        self.pointer_id == pointer.pointer_id
    }

    fn passed_threshold(self, pointer: &PointerEvent) -> bool {
        (pointer.position.y - self.origin.y).abs() >= 6.0
    }
}

struct CapturedAnchor<K> {
    anchor: CollectionAnchor<K>,
    fallback_index: usize,
}

/// Data-backed retained virtual list with stable keyed row identity.
///
/// Only the visible and overscanned rows are realized and measured. Recently
/// hidden rows are retained in a bounded keyed cache; focused and explicitly
/// pinned rows are never recycled.
pub struct VirtualList<K, T> {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Rc<dyn Fn() -> DefaultTheme>>,
    overlay_theme: Rc<RefCell<DefaultTheme>>,
    name: String,
    source: Arc<dyn VirtualCollectionSource<K, T>>,
    state: VirtualListState<K>,
    row_builder: Box<dyn Fn(&K, Signal<T>) -> WidgetPod>,
    row_name: Option<Box<dyn Fn(&K, &T) -> String>>,
    row_description: Option<Box<dyn Fn(&K, &T) -> String>>,
    on_change: Option<Box<dyn FnMut(K)>>,
    on_near_start: Option<Box<dyn FnMut()>>,
    on_near_end: Option<Box<dyn FnMut()>>,
    keys: Vec<K>,
    index_by_key: HashMap<K, usize>,
    semantic_ids: HashMap<K, WidgetId>,
    realized: HashMap<K, RealizedRow<T>>,
    extents: CollectionExtentIndex,
    initialized: bool,
    source_revision: u64,
    offset_y: f32,
    active_range: Range<usize>,
    estimated_row_height: Option<f32>,
    spacing: f32,
    padding: Option<Insets>,
    overscan_viewports: f32,
    cache_capacity: usize,
    epoch: u64,
    hovered: Option<K>,
    pressed: Option<K>,
    focused_row: RefCell<Option<K>>,
    near_start_notified: bool,
    near_end_notified: bool,
    overlay_scroll_bars: bool,
    overlay_bars: Option<OverlayScrollBars>,
    touch_scroll: Option<TouchGesture>,
}

impl<K, T> VirtualList<K, T>
where
    K: Clone + Eq + Hash + PartialEq + 'static,
    T: Clone + PartialEq + 'static,
{
    pub fn new<S, F, W>(name: impl Into<String>, source: S, row_builder: F) -> VirtualList<K, T>
    where
        S: VirtualCollectionSource<K, T> + 'static,
        F: Fn(&K, Signal<T>) -> W + 'static,
        W: Widget + 'static,
    {
        let theme = DefaultTheme::default();
        Self {
            theme: Box::new(theme),
            theme_reader: None,
            overlay_theme: Rc::new(RefCell::new(theme)),
            name: name.into(),
            source: Arc::new(source),
            state: VirtualListState::new(),
            row_builder: Box::new(move |key, value| WidgetPod::new(row_builder(key, value))),
            row_name: None,
            row_description: None,
            on_change: None,
            on_near_start: None,
            on_near_end: None,
            keys: Vec::new(),
            index_by_key: HashMap::new(),
            semantic_ids: HashMap::new(),
            realized: HashMap::new(),
            extents: CollectionExtentIndex::new(),
            initialized: false,
            source_revision: 0,
            offset_y: 0.0,
            active_range: 0..0,
            estimated_row_height: None,
            spacing: 0.0,
            padding: None,
            overscan_viewports: DEFAULT_OVERSCAN_VIEWPORTS,
            cache_capacity: DEFAULT_CACHE_CAPACITY,
            epoch: 0,
            hovered: None,
            pressed: None,
            focused_row: RefCell::new(None),
            near_start_notified: false,
            near_end_notified: false,
            overlay_scroll_bars: true,
            overlay_bars: None,
            touch_scroll: None,
        }
    }

    pub fn state(mut self, state: VirtualListState<K>) -> Self {
        self.state = state;
        self.overlay_bars = None;
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        *self.overlay_theme.borrow_mut() = theme;
        self
    }

    pub fn theme_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Rc::new(reader));
        self
    }

    pub fn estimated_row_height(mut self, height: f32) -> Self {
        self.estimated_row_height = Some(height.max(1.0));
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn overscan_viewports(mut self, viewports: f32) -> Self {
        self.overscan_viewports = viewports.max(0.0);
        self
    }

    pub fn cache_capacity(mut self, capacity: usize) -> Self {
        self.cache_capacity = capacity;
        self
    }

    pub fn stick_to_end(self, enabled: bool) -> Self {
        self.state.set_follow_end(enabled);
        self
    }

    pub fn overlay_scroll_bars(mut self, enabled: bool) -> Self {
        self.overlay_scroll_bars = enabled;
        if !enabled {
            self.overlay_bars = None;
        }
        self
    }

    pub fn row_name<F>(mut self, name: F) -> Self
    where
        F: Fn(&K, &T) -> String + 'static,
    {
        self.row_name = Some(Box::new(name));
        self
    }

    pub fn row_description<F>(mut self, description: F) -> Self
    where
        F: Fn(&K, &T) -> String + 'static,
    {
        self.row_description = Some(Box::new(description));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(K) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_near_start<F>(mut self, on_near_start: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_near_start = Some(Box::new(on_near_start));
        self
    }

    pub fn on_near_end<F>(mut self, on_near_end: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_near_end = Some(Box::new(on_near_end));
        self
    }

    pub fn current_offset(&self) -> Vector {
        Vector::new(0.0, self.offset_y)
    }

    pub fn realized_count(&self) -> usize {
        self.realized.len()
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(*self.theme)
    }

    fn sync_overlay_theme(&self) -> DefaultTheme {
        let theme = self.resolved_theme();
        *self.overlay_theme.borrow_mut() = theme;
        theme
    }

    fn ensure_overlay_bars(&mut self) {
        if self.overlay_scroll_bars && self.overlay_bars.is_none() {
            self.overlay_bars = Some(OverlayScrollBars::new(
                self.state.scroll.clone(),
                Rc::clone(&self.overlay_theme),
                Some(&self.name),
                ScrollAxes::Vertical,
            ));
        }
    }

    fn resolved_padding(&self) -> Insets {
        self.padding
            .unwrap_or(self.resolved_theme().metrics.data_viewport_padding)
    }

    fn estimated_extent(&self) -> f32 {
        self.estimated_row_height
            .unwrap_or(self.resolved_theme().metrics.list_row_height)
            .max(1.0)
            + self.spacing
    }

    fn content_height(&self) -> f32 {
        if self.extents.is_empty() {
            0.0
        } else {
            (self.extents.total() - self.spacing).max(0.0)
        }
    }

    fn viewport_rect(&self, bounds: Rect) -> Rect {
        inset_rect(bounds, self.resolved_padding())
    }

    fn rebuild_index(&mut self) {
        self.index_by_key.clear();
        self.index_by_key.extend(
            self.keys
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, key)| (key, index)),
        );
    }

    fn next_semantic_id() -> WidgetId {
        WidgetId::new(
            SYNTHETIC_COLLECTION_ITEM_TAG | NEXT_COLLECTION_ITEM_ID.fetch_add(1, Ordering::Relaxed),
        )
    }

    fn ensure_semantic_ids(&mut self) {
        for key in &self.keys {
            self.semantic_ids
                .entry(key.clone())
                .or_insert_with(Self::next_semantic_id);
        }
    }

    fn reset_keys(&mut self, keys: Vec<K>) {
        let old_extents = self
            .keys
            .iter()
            .enumerate()
            .filter_map(|(index, key)| {
                self.extents
                    .extent(index)
                    .map(|extent| (key.clone(), extent))
            })
            .collect::<HashMap<_, _>>();
        let live = keys.iter().cloned().collect::<HashSet<_>>();
        self.realized.retain(|key, row| {
            if !live.contains(key) {
                return false;
            }
            if let Some(item) = self.source.item(key) {
                let _ = row.value.set(item);
            }
            true
        });
        self.semantic_ids.retain(|key, _| live.contains(key));
        self.keys = keys;
        let estimate = self.estimated_extent();
        self.extents.rebuild(
            self.keys
                .iter()
                .map(|key| old_extents.get(key).copied().unwrap_or(estimate)),
        );
        self.rebuild_index();
        self.ensure_semantic_ids();
        self.state.clear_removed_selection(&live);
        self.hovered.take_if(|hovered| !live.contains(hovered));
        self.pressed.take_if(|pressed| !live.contains(pressed));
        self.focused_row
            .borrow_mut()
            .take_if(|focused| !live.contains(focused));
    }

    fn apply_delta(&mut self, delta: CollectionDelta<K>) -> bool {
        match delta {
            CollectionDelta::Inserted { index, keys } => {
                if index > self.keys.len()
                    || keys.iter().any(|key| self.index_by_key.contains_key(key))
                {
                    return false;
                }
                let estimate = self.estimated_extent();
                self.keys.splice(index..index, keys.iter().cloned());
                self.extents
                    .insert_many(index, std::iter::repeat_n(estimate, keys.len()));
                for key in keys {
                    self.semantic_ids
                        .entry(key)
                        .or_insert_with(Self::next_semantic_id);
                }
                self.rebuild_index();
            }
            CollectionDelta::Removed { keys } => {
                let removed = keys.iter().cloned().collect::<HashSet<_>>();
                if keys.iter().any(|key| !self.index_by_key.contains_key(key)) {
                    return false;
                }
                let mut indices = keys
                    .iter()
                    .filter_map(|key| self.index_by_key.get(key).copied())
                    .collect::<Vec<_>>();
                indices.sort_unstable_by(|left, right| right.cmp(left));
                for index in indices {
                    self.keys.remove(index);
                    let _ = self.extents.remove(index);
                }
                for key in &keys {
                    self.realized.remove(key);
                    self.semantic_ids.remove(key);
                }
                self.rebuild_index();
                let live = self.keys.iter().cloned().collect::<HashSet<_>>();
                self.state.clear_removed_selection(&live);
                self.hovered.take_if(|key| removed.contains(key));
                self.pressed.take_if(|key| removed.contains(key));
                self.focused_row
                    .borrow_mut()
                    .take_if(|key| removed.contains(key));
            }
            CollectionDelta::Moved { key, index } => {
                let Some(from) = self.index_by_key.get(&key).copied() else {
                    return false;
                };
                if index >= self.keys.len() {
                    return false;
                }
                if from != index {
                    let key = self.keys.remove(from);
                    self.keys.insert(index, key);
                    let _ = self.extents.move_item(from, index);
                    self.rebuild_index();
                }
            }
            CollectionDelta::Updated { key } => {
                let Some(row) = self.realized.get_mut(&key) else {
                    return true;
                };
                let Some(item) = self.source.item(&key) else {
                    return false;
                };
                let _ = row.value.set(item);
            }
        }
        true
    }

    fn sync_source(&mut self) -> bool {
        let sync = if self.initialized {
            self.source.changes_since(self.source_revision)
        } else {
            CollectionSync::Reset {
                revision: self.source.revision(),
                keys: self.source.keys(),
            }
        };

        match sync {
            CollectionSync::Unchanged { revision } => {
                self.source_revision = revision;
                self.initialized = true;
                false
            }
            CollectionSync::Incremental { revision, changes } => {
                let mut valid = true;
                for change in changes {
                    valid &= self.apply_delta(change);
                }
                if !valid {
                    self.reset_keys(self.source.keys());
                }
                self.source_revision = revision;
                self.initialized = true;
                true
            }
            CollectionSync::Reset { revision, keys } => {
                self.reset_keys(keys);
                self.source_revision = revision;
                self.initialized = true;
                true
            }
        }
    }

    fn capture_anchor(&self) -> Option<CapturedAnchor<K>> {
        let index = self.extents.index_at_offset(self.offset_y)?;
        let key = self.keys.get(index)?.clone();
        let item_start = self.extents.offset_of(index);
        Some(CapturedAnchor {
            anchor: CollectionAnchor {
                key,
                offset_within_item: self.offset_y - item_start,
                gravity: CollectionAnchorGravity::Start,
            },
            fallback_index: index,
        })
    }

    fn restore_anchor(&mut self, captured: &CapturedAnchor<K>) {
        let offset = if let Some(index) = self.index_by_key.get(&captured.anchor.key).copied() {
            let start = self.extents.offset_of(index);
            match captured.anchor.gravity {
                CollectionAnchorGravity::Start => {
                    start + captured.anchor.offset_within_item.max(0.0)
                }
                CollectionAnchorGravity::End => {
                    let extent = self.extents.extent(index).unwrap_or(0.0);
                    start + extent - captured.anchor.offset_within_item.max(0.0)
                }
            }
        } else if self.keys.is_empty() {
            0.0
        } else {
            self.extents
                .offset_of(captured.fallback_index.min(self.keys.len() - 1))
        };
        self.offset_y = offset.max(0.0);
    }

    fn visible_range(&self, viewport_height: f32, offset_y: f32) -> Range<usize> {
        if self.keys.is_empty() {
            return 0..0;
        }
        let overdraw = viewport_height * self.overscan_viewports;
        let top = (offset_y - overdraw).max(0.0);
        let bottom = offset_y + viewport_height + overdraw;
        let start = self.extents.index_at_offset(top).unwrap_or(0);
        let end = self
            .extents
            .index_at_offset(bottom)
            .map_or(self.keys.len(), |index| index + 1)
            .min(self.keys.len());
        start..end.max(start + 1).min(self.keys.len())
    }

    fn ensure_realized(&mut self, index: usize) -> bool {
        let Some(key) = self.keys.get(index).cloned() else {
            return false;
        };
        self.epoch = self.epoch.wrapping_add(1);
        if let Some(row) = self.realized.get_mut(&key) {
            row.last_used = self.epoch;
            return true;
        }
        let Some(item) = self.source.item(&key) else {
            return false;
        };
        let value = Signal::named("Virtual collection row", item);
        let pod = (self.row_builder)(&key, value.clone());
        self.realized.insert(
            key,
            RealizedRow {
                value,
                pod,
                last_used: self.epoch,
            },
        );
        true
    }

    fn measure_active_rows(
        &mut self,
        ctx: &mut MeasureCtx,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let next_range = self.visible_range(viewport_height, self.offset_y);
        let mut extent_changed = false;
        let constraints = Constraints::new(
            Size::new(viewport_width.max(0.0), 0.0),
            Size::new(viewport_width.max(0.0), f32::INFINITY),
        );
        for index in next_range.clone() {
            if !self.ensure_realized(index) {
                continue;
            }
            let key = self.keys[index].clone();
            let Some(row) = self.realized.get_mut(&key) else {
                continue;
            };
            let size = row.pod.measure(ctx, constraints);
            extent_changed |= self
                .extents
                .update(index, size.height.max(1.0) + self.spacing);
        }
        self.active_range = next_range;
        extent_changed
    }

    fn sync_scroll_state<C>(&mut self, ctx: &mut C, viewport: Size)
    where
        C: ScrollInvalidationCtx + ScrollWidgetCtx,
    {
        let content = Size::new(viewport.width, self.content_height());
        self.state
            .scroll
            .bind_scroll_view(ctx.widget_id(), ctx.widget_id());
        let mut changed = self
            .state
            .scroll
            .sync_metrics(ScrollAxes::Vertical, viewport, content);
        let max_offset = self.state.scroll.max_offset().y;
        self.offset_y = self.offset_y.clamp(0.0, max_offset);
        changed |= self
            .state
            .scroll
            .set_offset(Vector::new(0.0, self.offset_y));
        if changed {
            for scroll_bar_id in self.state.scroll.subscribers().scroll_bar_ids {
                request_scroll_bar_refresh(ctx, scroll_bar_id);
            }
        }
    }

    fn offset_for_alignment(
        &self,
        index: usize,
        viewport_height: f32,
        alignment: ScrollAlignment,
    ) -> f32 {
        let top = self.extents.offset_of(index);
        let extent = self
            .extents
            .extent(index)
            .unwrap_or(self.estimated_extent())
            .saturating_sub(self.spacing);
        let bottom = top + extent;
        match alignment {
            ScrollAlignment::Start => top,
            ScrollAlignment::Center => top - (viewport_height - extent) * 0.5,
            ScrollAlignment::End => bottom - viewport_height,
            ScrollAlignment::Nearest => {
                if top < self.offset_y {
                    top
                } else if bottom > self.offset_y + viewport_height {
                    bottom - viewport_height
                } else {
                    self.offset_y
                }
            }
        }
        .max(0.0)
    }

    fn apply_pending_scroll(&mut self, viewport_height: f32) {
        if let Some((key, alignment)) = self.state.take_pending_scroll()
            && let Some(index) = self.index_by_key.get(&key).copied()
        {
            self.offset_y = self.offset_for_alignment(index, viewport_height, alignment);
            self.state
                .update_following_for_offset(self.offset_y, self.state.scroll.max_offset().y);
        }
        if let Some(index) = self.state.scroll.take_pending_virtual_item()
            && index < self.keys.len()
        {
            self.offset_y =
                self.offset_for_alignment(index, viewport_height, ScrollAlignment::Start);
        }
    }

    fn clamp_offset(&mut self, viewport_height: f32) {
        let max_offset = (self.content_height() - viewport_height).max(0.0);
        self.offset_y = self.offset_y.clamp(0.0, max_offset);
        let _ = self
            .state
            .scroll
            .set_offset(Vector::new(0.0, self.offset_y));
    }

    fn row_at_position(&self, bounds: Rect, position: Point) -> Option<usize> {
        let viewport = self.viewport_rect(bounds);
        if !viewport.contains(position) {
            return None;
        }
        let offset = position.y - viewport.y() + self.offset_y;
        self.extents.index_at_offset(offset)
    }

    fn row_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        let viewport = self.viewport_rect(bounds);
        let top = self.extents.offset_of(index);
        let extent = self.extents.extent(index)?;
        Some(Rect::new(
            viewport.x(),
            viewport.y() + top - self.offset_y,
            viewport.width(),
            (extent - self.spacing).max(0.0),
        ))
    }

    fn activate_index(&mut self, index: usize, ctx: &mut EventCtx) {
        let Some(key) = self.keys.get(index).cloned() else {
            return;
        };
        let changed = self.state.select(Some(key.clone()));
        if changed {
            if let Some(on_change) = &mut self.on_change {
                on_change(key);
            }
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn ensure_index_visible(&mut self, index: usize, viewport_height: f32) {
        self.offset_y = self.offset_for_alignment(index, viewport_height, ScrollAlignment::Nearest);
        self.clamp_offset(viewport_height);
    }

    fn move_selection(&mut self, delta: isize, viewport_height: f32, ctx: &mut EventCtx) {
        if self.keys.is_empty() {
            return;
        }
        let current = self
            .state
            .selected_key()
            .as_ref()
            .and_then(|key| self.index_by_key.get(key).copied())
            .unwrap_or(0);
        let next = (current as isize + delta).clamp(0, self.keys.len() as isize - 1) as usize;
        self.activate_index(next, ctx);
        self.ensure_index_visible(next, viewport_height);
    }

    fn scroll_by(&mut self, viewport: Rect, delta_y: f32, ctx: &mut EventCtx) -> bool {
        let previous = self.offset_y;
        let max_offset = (self.content_height() - viewport.height()).max(0.0);
        let next = (previous + delta_y).clamp(0.0, max_offset);
        if (next - previous).abs() <= f32::EPSILON {
            return false;
        }
        self.offset_y = next;
        let _ = self
            .state
            .scroll
            .set_offset(Vector::new(0.0, self.offset_y));
        self.state
            .update_following_for_offset(self.offset_y, max_offset);
        ctx.request_measure();
        ctx.request_arrange();
        ctx.request_paint();
        ctx.request_semantics();
        for index in self.active_range.clone() {
            if let Some(key) = self.keys.get(index)
                && let Some(row) = self.realized.get(key)
            {
                ctx.request(InvalidationRequest::new(
                    InvalidationTarget::Widget(row.pod.id()),
                    InvalidationKind::Transform,
                ));
            }
        }
        for scroll_bar_id in self.state.scroll.subscribers().scroll_bar_ids {
            request_scroll_bar_refresh(ctx, scroll_bar_id);
        }
        self.notify_near_edges(viewport.height());
        true
    }

    fn notify_near_edges(&mut self, viewport_height: f32) {
        let threshold = viewport_height.max(self.estimated_extent() * 8.0);
        let near_start = self.offset_y <= threshold;
        let remaining = (self.content_height() - viewport_height - self.offset_y).max(0.0);
        let near_end = remaining <= threshold;

        if near_start && !self.near_start_notified {
            self.near_start_notified = true;
            if let Some(on_near_start) = &mut self.on_near_start {
                on_near_start();
            }
        } else if !near_start {
            self.near_start_notified = false;
        }

        if near_end && !self.near_end_notified {
            self.near_end_notified = true;
            if let Some(on_near_end) = &mut self.on_near_end {
                on_near_end();
            }
        } else if !near_end {
            self.near_end_notified = false;
        }
    }

    fn sync_focused_row(&self, focused: Option<WidgetId>) {
        let Some(focused) = focused else {
            *self.focused_row.borrow_mut() = None;
            return;
        };
        *self.focused_row.borrow_mut() = self
            .realized
            .iter()
            .find_map(|(key, row)| pod_contains_widget(&row.pod, focused).then(|| key.clone()));
    }

    fn retained_keys(&self) -> HashSet<K> {
        let mut retained = self.state.pinned();
        if let Some(focused) = self.focused_row.borrow().as_ref() {
            retained.insert(focused.clone());
        }
        retained
    }

    fn evict_rows(&mut self) {
        let retained = self.retained_keys();
        let active = self
            .active_range
            .clone()
            .filter_map(|index| self.keys.get(index).cloned())
            .collect::<HashSet<_>>();
        let maximum = active.len() + retained.len() + self.cache_capacity;
        if self.realized.len() <= maximum {
            return;
        }
        let mut candidates = self
            .realized
            .iter()
            .filter(|(key, _)| !active.contains(*key) && !retained.contains(*key))
            .map(|(key, row)| (key.clone(), row.last_used))
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|(_, last_used)| *last_used);
        let remove_count = self.realized.len().saturating_sub(maximum);
        for (key, _) in candidates.into_iter().take(remove_count) {
            self.realized.remove(&key);
        }
    }

    fn semantic_key(&self, target: WidgetId) -> Option<K> {
        self.semantic_ids
            .iter()
            .find_map(|(key, id)| (*id == target).then(|| key.clone()))
    }

    fn handle_touch_pointer(&mut self, ctx: &mut EventCtx, pointer: &PointerEvent, viewport: Rect) {
        if pointer.pointer_kind != PointerKind::Touch || ctx.phase() == EventPhase::Capture {
            return;
        }
        match pointer.kind {
            PointerEventKind::Down
                if pointer.is_primary
                    && pointer.button == Some(PointerButton::Primary)
                    && viewport.contains(pointer.position)
                    && self.content_height() > viewport.height() =>
            {
                self.touch_scroll = Some(TouchGesture::new(pointer));
            }
            PointerEventKind::Move => {
                let Some(gesture) = self.touch_scroll else {
                    return;
                };
                if !gesture.matches(pointer)
                    || (!gesture.dragging && !gesture.passed_threshold(pointer))
                {
                    return;
                }
                if !gesture.dragging {
                    if let Some(gesture) = &mut self.touch_scroll {
                        gesture.dragging = true;
                    }
                    self.pressed = None;
                    ctx.request_pointer_capture(pointer.pointer_id);
                }
                if self.scroll_by(viewport, -pointer.delta.y, ctx) {
                    ctx.set_handled();
                }
            }
            PointerEventKind::Up | PointerEventKind::Cancel => {
                let Some(gesture) = self.touch_scroll else {
                    return;
                };
                if gesture.matches(pointer) {
                    self.touch_scroll = None;
                    if gesture.dragging {
                        ctx.release_pointer_capture(pointer.pointer_id);
                        ctx.set_handled();
                    }
                }
            }
            _ => {}
        }
    }

    fn child_keys_for_graph(&self) -> Vec<K> {
        let mut keys = self
            .active_range
            .clone()
            .filter_map(|index| self.keys.get(index).cloned())
            .collect::<Vec<_>>();
        let active = keys.iter().cloned().collect::<HashSet<_>>();
        for key in self.retained_keys() {
            if !active.contains(&key) && self.realized.contains_key(&key) {
                keys.push(key);
            }
        }
        keys
    }
}

impl<K, T> Widget for VirtualList<K, T>
where
    K: Clone + Eq + Hash + PartialEq + 'static,
    T: Clone + PartialEq + 'static,
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_focused_row(ctx.focused_widget_id());
        let viewport = self.viewport_rect(ctx.bounds());
        if let Event::Pointer(pointer) = event {
            self.handle_touch_pointer(ctx, pointer, viewport);
        }
        if ctx.phase() == EventPhase::Capture {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && viewport.contains(pointer.position) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                if self.scroll_by(viewport, -delta.y, ctx) {
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .and_then(|index| self.keys.get(index).cloned());
                if self.hovered != hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && viewport.contains(pointer.position)
                    && !ctx.is_handled() =>
            {
                self.pressed = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .and_then(|index| self.keys.get(index).cloned());
                if self.pressed.is_some() {
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pressed.is_some() =>
            {
                let hovered = self
                    .row_at_position(ctx.bounds(), pointer.position)
                    .and_then(|index| self.keys.get(index).cloned());
                let activate = self
                    .pressed
                    .as_ref()
                    .zip(hovered.as_ref())
                    .filter(|(pressed, hovered)| pressed == hovered)
                    .and_then(|(_, key)| self.index_by_key.get(key).copied());
                self.pressed = None;
                self.hovered = hovered;
                ctx.release_pointer_capture(pointer.pointer_id);
                if let Some(index) = activate {
                    self.activate_index(index, ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Leave | PointerEventKind::Cancel
                ) =>
            {
                let changed = self.hovered.take().is_some() || self.pressed.take().is_some();
                if pointer.kind == PointerEventKind::Cancel {
                    ctx.release_pointer_capture(pointer.pointer_id);
                }
                if changed {
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowUp" => self.move_selection(-1, viewport.height(), ctx),
                    "ArrowDown" => self.move_selection(1, viewport.height(), ctx),
                    "Home" if !self.keys.is_empty() => {
                        self.activate_index(0, ctx);
                        self.ensure_index_visible(0, viewport.height());
                    }
                    "End" if !self.keys.is_empty() => {
                        let last = self.keys.len() - 1;
                        self.activate_index(last, ctx);
                        self.ensure_index_visible(last, viewport.height());
                    }
                    "PageUp" => {
                        let _ = self.scroll_by(viewport, -viewport.height() * 0.85, ctx);
                    }
                    "PageDown" => {
                        let _ = self.scroll_by(viewport, viewport.height() * 0.85, ctx);
                    }
                    _ => return,
                }
                let _ = self
                    .state
                    .scroll
                    .set_offset(Vector::new(0.0, self.offset_y));
                ctx.request_measure();
                ctx.request_arrange();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Semantics(semantics) => {
                if semantics.target == ctx.widget_id() {
                    let delta = match semantics.action {
                        SemanticsActionRequest::Increment => Some(viewport.height() * 0.85),
                        SemanticsActionRequest::Decrement => Some(-viewport.height() * 0.85),
                        _ => None,
                    };
                    if let Some(delta) = delta
                        && self.scroll_by(viewport, delta, ctx)
                    {
                        ctx.set_handled();
                    }
                    return;
                }
                let Some(key) = self.semantic_key(semantics.target) else {
                    return;
                };
                let Some(index) = self.index_by_key.get(&key).copied() else {
                    return;
                };
                match semantics.action {
                    SemanticsActionRequest::Activate | SemanticsActionRequest::Focus => {
                        self.activate_index(index, ctx);
                        self.ensure_index_visible(index, viewport.height());
                        ctx.request_focus();
                        ctx.request_measure();
                        ctx.request_arrange();
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.sync_overlay_theme();
        self.ensure_overlay_bars();
        let _ = ctx.observe_with(self.source.as_ref(), InvalidationKind::Measure);
        let _ = ctx.observe(&self.state.revision);
        self.offset_y = self.state.scroll.current_offset().y;

        let was_following = self.state.is_following_end();
        let anchor = (!was_following).then(|| self.capture_anchor()).flatten();
        let source_changed = self.sync_source();

        let padding = self.resolved_padding();
        let estimated_height = self.content_height() + padding.top + padding.bottom;
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            320.0
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            estimated_height.min(DEFAULT_UNBOUNDED_VIEWPORT_HEIGHT)
        };
        let size = constraints.clamp(Size::new(width, height));
        let viewport = self.viewport_rect(Rect::from_origin_size(Point::ZERO, size));

        if source_changed {
            if was_following {
                self.offset_y = (self.content_height() - viewport.height()).max(0.0);
            } else if let Some(anchor) = &anchor {
                self.restore_anchor(anchor);
            }
        }

        self.sync_scroll_state(ctx, viewport.size);
        self.apply_pending_scroll(viewport.height());
        if was_following {
            self.offset_y = (self.content_height() - viewport.height()).max(0.0);
        }
        self.clamp_offset(viewport.height());

        let measurement_anchor = (!was_following)
            .then(|| self.capture_anchor())
            .flatten()
            .or(anchor);
        for _ in 0..3 {
            let changed = self.measure_active_rows(ctx, viewport.width(), viewport.height());
            if !changed {
                break;
            }
            if was_following {
                self.offset_y = (self.content_height() - viewport.height()).max(0.0);
            } else if let Some(anchor) = &measurement_anchor {
                self.restore_anchor(anchor);
            }
            self.clamp_offset(viewport.height());
        }

        self.sync_scroll_state(ctx, viewport.size);
        self.apply_pending_scroll(viewport.height());
        if self.state.is_following_end() {
            self.offset_y = (self.content_height() - viewport.height()).max(0.0);
        }
        self.clamp_offset(viewport.height());
        self.active_range = self.visible_range(viewport.height(), self.offset_y);
        self.evict_rows();

        let max_offset = self.state.scroll.max_offset();
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.set_visibility(ScrollAxes::Vertical, max_offset);
            overlay_bars.measure(ctx, size, theme.metrics.scroll_bar_thickness);
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.sync_overlay_theme();
        let viewport = self.viewport_rect(bounds);
        self.offset_y = self.state.scroll.current_offset().y;
        let previous_offset = self.offset_y;
        self.sync_scroll_state(ctx, viewport.size);
        self.apply_pending_scroll(viewport.height());
        if self.state.is_following_end() {
            self.offset_y = (self.content_height() - viewport.height()).max(0.0);
        }
        self.clamp_offset(viewport.height());
        let next_range = self.visible_range(viewport.height(), self.offset_y);
        if next_range != self.active_range
            && next_range.clone().any(|index| {
                self.keys
                    .get(index)
                    .is_some_and(|key| !self.realized.contains_key(key))
            })
        {
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Widget(ctx.widget_id()),
                InvalidationKind::Measure,
            ));
        }
        self.active_range = next_range;
        if (previous_offset - self.offset_y).abs() > f32::EPSILON {
            ctx.request_paint();
            ctx.request_semantics();
        }

        for key in self.child_keys_for_graph() {
            let Some(index) = self.index_by_key.get(&key).copied() else {
                continue;
            };
            let Some(rect) = self.row_rect(bounds, index) else {
                continue;
            };
            if let Some(row) = self.realized.get_mut(&key) {
                row.pod.arrange(ctx, rect);
            }
        }

        let max_offset = self.state.scroll.max_offset();
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.set_visibility(ScrollAxes::Vertical, max_offset);
            overlay_bars.arrange(ctx, bounds, theme.metrics.scroll_bar_thickness);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.sync_focused_row(ctx.focused_widget_id());
        self.sync_overlay_theme();
        let theme = self.resolved_theme();
        let viewport = self.viewport_rect(ctx.bounds());
        let selected = self.state.selected_key();
        draw_surface(ctx, ctx.bounds(), &theme, 0.0);
        ctx.push_clip_rect(viewport);
        for index in self.active_range.clone() {
            let Some(key) = self.keys.get(index) else {
                continue;
            };
            let Some(row) = self.realized.get(key) else {
                continue;
            };
            let Some(rect) = self.row_rect(ctx.bounds(), index) else {
                continue;
            };
            paint_data_row_state(
                ctx,
                rect,
                viewport,
                &theme,
                selected.as_ref() == Some(key),
                (self.hovered.as_ref() == Some(key)) as u8 as f32,
                (self.pressed.as_ref() == Some(key)) as u8 as f32,
            );
            row.pod.paint(ctx);
        }
        ctx.pop_clip();
        if let Some(overlay_bars) = &self.overlay_bars {
            ctx.push_clip_rect(ctx.bounds());
            overlay_bars.paint(ctx);
            ctx.pop_clip();
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Scroll,
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let total = self.keys.len();
        let visible_start = self.active_range.start.min(total);
        let visible_end = self.active_range.end.min(total);
        let mut list = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        list.name = Some(self.name.clone());
        list.state.focused = ctx.is_focused();
        list.value = Some(SemanticsValue::Text(format!("{total} items")));
        list.description = Some(if total == 0 {
            "Empty collection".to_string()
        } else {
            format!(
                "Showing items {} through {} of {}; {} before and {} after the realized range",
                visible_start + 1,
                visible_end.max(visible_start + 1),
                total,
                visible_start,
                total.saturating_sub(visible_end),
            )
        });
        list.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Increment,
            SemanticsAction::Decrement,
        ];
        ctx.push(list);

        let selected = self.state.selected_key();
        for index in self.active_range.clone() {
            let Some(key) = self.keys.get(index) else {
                continue;
            };
            let Some(row) = self.realized.get(key) else {
                continue;
            };
            let Some(id) = self.semantic_ids.get(key).copied() else {
                continue;
            };
            let Some(rect) = self.row_rect(ctx.bounds(), index) else {
                continue;
            };
            let item = row.value.get();
            let mut node = SemanticsNode::new(id, SemanticsRole::ListItem, rect);
            node.parent = Some(ctx.widget_id());
            node.name = Some(
                self.row_name
                    .as_ref()
                    .map(|name| name(key, &item))
                    .unwrap_or_else(|| format!("Item {}", index + 1)),
            );
            let position = format!("Item {} of {total}", index + 1);
            node.description = Some(
                self.row_description
                    .as_ref()
                    .map(|description| format!("{}; {position}", description(key, &item)))
                    .unwrap_or(position),
            );
            node.state.selected = selected.as_ref() == Some(key);
            node.state.hovered = self.hovered.as_ref() == Some(key);
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
            ctx.push(node);
            row.pod.semantics(ctx);
        }
        if let Some(overlay_bars) = &self.overlay_bars {
            overlay_bars.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for key in self.child_keys_for_graph() {
            if let Some(row) = self.realized.get(&key) {
                visitor.visit(&row.pod);
            }
        }
        if let Some(overlay_bars) = &self.overlay_bars {
            overlay_bars.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        let keys = self.child_keys_for_graph();
        for key in keys {
            if let Some(row) = self.realized.get_mut(&key) {
                visitor.visit(&mut row.pod);
            }
        }
        if let Some(overlay_bars) = &mut self.overlay_bars {
            overlay_bars.visit_children_mut(visitor);
        }
    }
}

fn inset_rect(rect: Rect, padding: Insets) -> Rect {
    Rect::new(
        rect.x() + padding.left,
        rect.y() + padding.top,
        (rect.width() - padding.left - padding.right).max(0.0),
        (rect.height() - padding.top - padding.bottom).max(0.0),
    )
}

fn scroll_delta_to_offset(delta: ScrollDelta) -> Vector {
    match delta {
        ScrollDelta::Lines(delta) => Vector::new(delta.x * 40.0, delta.y * 40.0),
        ScrollDelta::Pixels(delta) => delta,
    }
}

fn pod_contains_widget(pod: &WidgetPod, target: WidgetId) -> bool {
    struct Finder {
        target: WidgetId,
        found: bool,
    }

    impl WidgetPodVisitor for Finder {
        fn visit(&mut self, child: &WidgetPod) {
            if self.found {
                return;
            }
            if child.id() == self.target {
                self.found = true;
            } else {
                child.visit_children(self);
            }
        }
    }

    if pod.id() == target {
        return true;
    }
    let mut finder = Finder {
        target,
        found: false,
    };
    pod.visit_children(&mut finder);
    finder.found
}

trait SaturatingSubF32 {
    fn saturating_sub(self, other: Self) -> Self;
}

impl SaturatingSubF32 for f32 {
    fn saturating_sub(self, other: Self) -> Self {
        (self - other).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::{
        CollectionChange, CollectionDelta, CollectionExtentIndex, CollectionSync, ScrollAlignment,
        VirtualCollectionModel, VirtualCollectionSource, VirtualList, VirtualListState,
    };
    use crate::SizedBox;
    use sui_core::{
        Color, Event, KeyState, KeyboardEvent, Point, PointerEvent, PointerEventKind, Rect, Result,
        ScrollDelta, SemanticsActionRequest, SemanticsNode, SemanticsRole, Size, Vector,
    };
    use sui_layout::Constraints;
    use sui_runtime::{
        Application, MeasureCtx, PaintCtx, Runtime, SemanticsCtx, Widget, WindowBuilder,
    };

    struct RowBox {
        value: sui_reactive::Signal<(u64, f32)>,
    }

    impl Widget for RowBox {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let (_, height) = ctx.observe(&self.value);
            constraints.clamp(Size::new(constraints.max.width, height))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            let (index, _) = self.value.get();
            let color = if index.is_multiple_of(2) {
                Color::rgba(0.2, 0.4, 0.8, 1.0)
            } else {
                Color::rgba(0.3, 0.6, 0.4, 1.0)
            };
            ctx.fill_rect(ctx.bounds(), color);
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let (index, _) = self.value.get();
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(format!("Row {index}"));
            ctx.push(node);
        }
    }

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Virtual list").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    #[test]
    fn extent_index_supports_incremental_prefix_operations() {
        let mut index = CollectionExtentIndex::new();
        index.rebuild([10.0, 20.0, 30.0]);
        assert_eq!(index.total(), 60.0);
        assert_eq!(index.offset_of(2), 30.0);
        assert_eq!(index.index_at_offset(29.0), Some(1));

        index.insert(1, 5.0);
        assert_eq!(index.total(), 65.0);
        assert_eq!(index.offset_of(2), 15.0);
        assert_eq!(index.remove(2), Some(20.0));
        assert_eq!(index.total(), 45.0);
        assert!(index.update(1, 15.0));
        assert_eq!(index.total(), 55.0);
        assert!(index.move_item(0, 2));
        assert_eq!(index.offset_of(2), 45.0);
    }

    #[test]
    fn extent_index_matches_vec_under_mixed_edits() {
        let mut index = CollectionExtentIndex::new();
        let mut expected = Vec::<f32>::new();
        let mut random = 0x5eed_cafe_f00d_u64;
        let mut next_random = || {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            random
        };

        for _ in 0..2_000 {
            match next_random() % 4 {
                0 | 1 => {
                    let insertion = (next_random() as usize) % (expected.len() + 1);
                    let extent = (next_random() % 96 + 1) as f32;
                    expected.insert(insertion, extent);
                    index.insert(insertion, extent);
                }
                2 if !expected.is_empty() => {
                    let removal = (next_random() as usize) % expected.len();
                    assert_eq!(index.remove(removal), Some(expected.remove(removal)));
                }
                3 if !expected.is_empty() => {
                    let changed = (next_random() as usize) % expected.len();
                    let extent = (next_random() % 96 + 1) as f32;
                    let expected_changed = expected[changed] != extent;
                    expected[changed] = extent;
                    assert_eq!(index.update(changed, extent), expected_changed);
                }
                _ => {}
            }

            if expected.len() > 1 && next_random().is_multiple_of(5) {
                let from = (next_random() as usize) % expected.len();
                let to = (next_random() as usize) % expected.len();
                if from != to {
                    let extent = expected.remove(from);
                    expected.insert(to, extent);
                    assert!(index.move_item(from, to));
                }
            }

            assert_eq!(index.len(), expected.len());
            assert_eq!(index.total(), expected.iter().sum::<f32>());
            for offset_index in 0..=expected.len() {
                assert_eq!(
                    index.offset_of(offset_index),
                    expected[..offset_index].iter().sum::<f32>()
                );
            }
            for (item_index, extent) in expected.iter().enumerate() {
                let midpoint = expected[..item_index].iter().sum::<f32>() + (*extent * 0.5);
                assert_eq!(index.index_at_offset(midpoint), Some(item_index));
            }
        }
    }

    #[test]
    fn model_reports_incremental_key_changes() {
        let model =
            VirtualCollectionModel::from_items("items", [(1_u64, "one"), (2, "two")]).unwrap();
        let revision = model.revision();
        assert!(model.append(3, "three").unwrap());
        assert!(model.update(2, "second").unwrap());
        assert!(model.move_to(3, 0).unwrap());

        assert_eq!(
            model.changes_since(revision),
            CollectionSync::Incremental {
                revision: model.revision(),
                changes: vec![
                    CollectionDelta::Inserted {
                        index: 2,
                        keys: vec![3],
                    },
                    CollectionDelta::Updated { key: 2 },
                    CollectionDelta::Moved { key: 3, index: 0 },
                ],
            }
        );
    }

    #[test]
    fn model_rejects_duplicate_keys_atomically() {
        let model = VirtualCollectionModel::<u64, String>::new();
        assert!(
            model
                .apply(CollectionChange::Insert {
                    index: 0,
                    items: vec![(1, "one".into()), (1, "duplicate".into())],
                })
                .is_err()
        );
        assert!(model.is_empty());
    }

    #[test]
    fn model_resets_consumers_that_fall_behind_the_bounded_journal() {
        let model = VirtualCollectionModel::<u64, u64>::new();
        let revision = model.revision();
        for key in 0..=super::COLLECTION_JOURNAL_LIMIT as u64 {
            model.append(key, key).unwrap();
        }

        assert!(matches!(
            model.changes_since(revision),
            CollectionSync::Reset { keys, .. }
                if keys.len() == super::COLLECTION_JOURNAL_LIMIT + 1
        ));
    }

    #[test]
    fn virtual_list_realizes_only_a_bounded_visible_window() {
        let model = VirtualCollectionModel::from_items(
            "rows",
            (0_u64..1_000).map(|index| (index, (index, 20.0))),
        )
        .unwrap();
        let builds = Rc::new(Cell::new(0));
        let build_count = Rc::clone(&builds);
        let list = VirtualList::new("Rows", model, move |_key, value| {
            build_count.set(build_count.get() + 1);
            RowBox { value }
        })
        .estimated_row_height(20.0)
        .overscan_viewports(0.5);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );

        runtime.render(window_id).unwrap();
        assert!(
            builds.get() < 30,
            "virtual list should not realize all rows; realized {}",
            builds.get()
        );
    }

    #[test]
    fn prepend_preserves_the_visible_key_anchor() {
        let model = VirtualCollectionModel::from_items(
            "rows",
            (10_u64..30).map(|index| (index, (index, 20.0))),
        )
        .unwrap();
        let state = VirtualListState::new();
        state.scroll_to(15, ScrollAlignment::Start);
        let list = VirtualList::new("Rows", model.clone(), |_key, value| RowBox { value })
            .state(state.clone())
            .estimated_row_height(20.0)
            .overscan_viewports(0.0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );
        runtime.render(window_id).unwrap();
        let before = state.scroll_state().current_offset().y;
        assert_eq!(before, 100.0);

        model
            .prepend((0_u64..10).map(|index| (index, (index, 20.0))))
            .unwrap();
        runtime.render(window_id).unwrap();

        assert_eq!(state.scroll_state().current_offset().y, before + 200.0);
    }

    #[test]
    fn follow_end_tracks_appends_until_user_scrolls_away() -> Result<()> {
        let model = VirtualCollectionModel::from_items(
            "rows",
            (0_u64..20).map(|index| (index, (index, 20.0))),
        )
        .unwrap();
        let state = VirtualListState::new();
        let list = VirtualList::new("Rows", model.clone(), |_key, value| RowBox { value })
            .state(state.clone())
            .stick_to_end(true)
            .estimated_row_height(20.0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );
        runtime.render(window_id)?;
        assert_eq!(
            state.scroll_state().current_offset().y,
            state.scroll_state().max_offset().y
        );

        model.append(20, (20, 20.0)).unwrap();
        runtime.render(window_id)?;
        assert_eq!(
            state.scroll_state().current_offset().y,
            state.scroll_state().max_offset().y
        );

        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, Point::new(100.0, 50.0));
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(0.0, 80.0)));
        runtime.handle_event(window_id, Event::Pointer(scroll))?;
        runtime.render(window_id)?;
        assert!(!state.is_following_end());
        let away = state.scroll_state().current_offset().y;

        model.append(21, (21, 20.0)).unwrap();
        runtime.render(window_id)?;
        assert_eq!(state.scroll_state().current_offset().y, away);
        Ok(())
    }

    #[test]
    fn keyboard_selection_uses_stable_keys_and_semantics_reports_range() -> Result<()> {
        let model = VirtualCollectionModel::from_items(
            "rows",
            (0_u64..20).map(|index| (index, (index, 20.0))),
        )
        .unwrap();
        let state = VirtualListState::new();
        let list = VirtualList::new("Rows", model, |_key, value| RowBox { value })
            .state(state.clone())
            .estimated_row_height(20.0)
            .row_name(|key, _| format!("Row {key}"));
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );
        let output = runtime.render(window_id)?;
        let list_id = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::List)
            .expect("list semantics")
            .id;
        runtime.handle_semantics_action(window_id, list_id, SemanticsActionRequest::Focus)?;
        runtime.handle_event(
            window_id,
            Event::Keyboard(KeyboardEvent::new("ArrowDown", KeyState::Pressed)),
        )?;
        assert_eq!(state.selected_key(), Some(1));

        let output = runtime.render(window_id)?;
        let list = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::List)
            .expect("list semantics");
        assert_eq!(
            list.value,
            Some(sui_core::SemanticsValue::Text("20 items".into()))
        );
        assert!(
            list.description
                .as_deref()
                .is_some_and(|description| description.contains("of 20"))
        );
        Ok(())
    }

    #[test]
    fn row_update_preserves_realized_widget_identity() {
        let model = VirtualCollectionModel::from_items("rows", [(1_u64, (1_u64, 20.0))]).unwrap();
        let list = VirtualList::new("Rows", model.clone(), |_key, value| RowBox { value })
            .estimated_row_height(20.0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );
        let before_output = runtime.render(window_id).unwrap();
        let before = before_output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Row 1"))
            .expect("row widget")
            .id;

        model.update(1, (1, 36.0)).unwrap();
        let output = runtime.render(window_id).unwrap();
        let after = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Row 1"))
            .expect("row widget")
            .id;
        assert_eq!(before, after);
    }

    #[test]
    fn pinned_rows_survive_cache_eviction() {
        let model = VirtualCollectionModel::from_items(
            "rows",
            (0_u64..100).map(|index| (index, (index, 20.0))),
        )
        .unwrap();
        let state = VirtualListState::new();
        state.pin(0);
        let list = VirtualList::new("Rows", model, |_key, value| RowBox { value })
            .state(state.clone())
            .estimated_row_height(20.0)
            .cache_capacity(0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );
        let before = runtime.render(window_id).unwrap();
        let before_id = before
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Row 0"))
            .expect("pinned row initially visible")
            .id;
        state.scroll_to(90, ScrollAlignment::Start);
        runtime.render(window_id).unwrap();
        state.scroll_to(0, ScrollAlignment::Start);
        let after = runtime.render(window_id).unwrap();
        let after_id = after
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Row 0"))
            .expect("pinned row visible again")
            .id;
        assert_eq!(before_id, after_id);
    }

    #[test]
    fn row_rects_use_variable_measured_heights() {
        let model = VirtualCollectionModel::from_items(
            "rows",
            [(0_u64, (0, 20.0)), (1, (1, 40.0)), (2, (2, 30.0))],
        )
        .unwrap();
        let list = VirtualList::new("Rows", model, |_key, value| RowBox { value })
            .estimated_row_height(24.0)
            .overscan_viewports(2.0);
        let (mut runtime, window_id) = build_runtime(
            SizedBox::new()
                .size(Size::new(240.0, 100.0))
                .with_child(list),
        );
        runtime.render(window_id).unwrap();
        let mut rows = runtime
            .render(window_id)
            .unwrap()
            .semantics
            .into_iter()
            .filter(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| name.starts_with("Row "))
            })
            .map(|node| node.bounds)
            .collect::<Vec<Rect>>();
        rows.sort_by(|left, right| left.y().total_cmp(&right.y()));
        assert_eq!(rows[0].height(), 20.0);
        assert_eq!(rows[1].height(), 40.0);
        assert_eq!(rows[2].height(), 30.0);
    }
}
