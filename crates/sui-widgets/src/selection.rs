use std::{any::Any, cell::RefCell, collections::BTreeMap, fmt, ops::Range, rc::Rc, sync::Arc};

use sui_core::{ImageHandle, Rect, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SelectionOwnerId(u64);

impl SelectionOwnerId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<WidgetId> for SelectionOwnerId {
    fn from(value: WidgetId) -> Self {
        Self(value.get())
    }
}

impl From<u64> for SelectionOwnerId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SelectionOrder(u64);

impl SelectionOrder {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<SelectionOwnerId> for SelectionOrder {
    fn from(value: SelectionOwnerId) -> Self {
        Self(value.get())
    }
}

impl From<WidgetId> for SelectionOrder {
    fn from(value: WidgetId) -> Self {
        Self(value.get())
    }
}

impl From<u64> for SelectionOrder {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionIntent {
    Replace,
    Extend,
    Toggle,
}

#[derive(Clone)]
pub enum SelectionPayload {
    Text(String),
    Image {
        handle: ImageHandle,
        region: Option<Rect>,
    },
    Custom {
        kind: Arc<str>,
        data: Arc<dyn Any + Send + Sync>,
    },
}

impl SelectionPayload {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn image(handle: ImageHandle) -> Self {
        Self::Image {
            handle,
            region: None,
        }
    }

    pub fn image_region(handle: ImageHandle, region: Rect) -> Self {
        Self::Image {
            handle,
            region: Some(region),
        }
    }

    pub fn custom<T>(kind: impl Into<Arc<str>>, data: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        Self::Custom {
            kind: kind.into(),
            data: Arc::new(data),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Image { .. } | Self::Custom { .. } => None,
        }
    }

    pub fn custom_kind(&self) -> Option<&str> {
        match self {
            Self::Custom { kind, .. } => Some(kind.as_ref()),
            Self::Text(_) | Self::Image { .. } => None,
        }
    }

    pub fn custom_data<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        match self {
            Self::Custom { data, .. } => data.downcast_ref(),
            Self::Text(_) | Self::Image { .. } => None,
        }
    }
}

impl fmt::Debug for SelectionPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => f.debug_tuple("Text").field(text).finish(),
            Self::Image { handle, region } => f
                .debug_struct("Image")
                .field("handle", handle)
                .field("region", region)
                .finish(),
            Self::Custom { kind, .. } => f
                .debug_struct("Custom")
                .field("kind", kind)
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelectionEntry {
    pub owner: SelectionOwnerId,
    pub order: SelectionOrder,
    pub payload: SelectionPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionChange {
    affected_owners: Vec<SelectionOwnerId>,
}

impl SelectionChange {
    pub fn new(affected_owners: Vec<SelectionOwnerId>) -> Self {
        Self { affected_owners }
    }

    pub fn is_empty(&self) -> bool {
        self.affected_owners.is_empty()
    }

    pub fn affected_owners(&self) -> &[SelectionOwnerId] {
        &self.affected_owners
    }
}

impl SelectionEntry {
    pub fn new(
        owner: impl Into<SelectionOwnerId>,
        order: impl Into<SelectionOrder>,
        payload: SelectionPayload,
    ) -> Self {
        Self {
            owner: owner.into(),
            order: order.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub owner: SelectionOwnerId,
    pub offset: usize,
    pub order: SelectionOrder,
}

impl SelectionPoint {
    pub fn new(
        owner: impl Into<SelectionOwnerId>,
        offset: usize,
        order: impl Into<SelectionOrder>,
    ) -> Self {
        Self {
            owner: owner.into(),
            offset,
            order: order.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSelectionInfo {
    pub owner: SelectionOwnerId,
    pub range: Range<usize>,
    pub text_len: usize,
}

#[derive(Debug, Clone)]
struct OwnerSelection {
    entries: Vec<SelectionEntry>,
    text: Option<TextSelectionInfo>,
}

#[derive(Debug, Default)]
struct SelectionState {
    revision: u64,
    active_owner: Option<SelectionOwnerId>,
    anchor: Option<SelectionPoint>,
    focus: Option<SelectionPoint>,
    owners: BTreeMap<SelectionOwnerId, OwnerSelection>,
}

#[derive(Clone, Debug, Default)]
pub struct SelectionScope {
    inner: Rc<RefCell<SelectionState>>,
}

impl SelectionScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revision(&self) -> u64 {
        self.inner.borrow().revision
    }

    pub fn active_owner(&self) -> Option<SelectionOwnerId> {
        self.inner.borrow().active_owner
    }

    pub fn anchor(&self) -> Option<SelectionPoint> {
        self.inner.borrow().anchor
    }

    pub fn focus(&self) -> Option<SelectionPoint> {
        self.inner.borrow().focus
    }

    pub fn set_points(
        &self,
        anchor: Option<SelectionPoint>,
        focus: Option<SelectionPoint>,
    ) -> SelectionChange {
        let mut affected = Vec::new();
        if let Some(point) = anchor {
            affected.push(point.owner);
        }
        if let Some(point) = focus {
            affected.push(point.owner);
        }
        affected.sort();
        affected.dedup();

        let mut state = self.inner.borrow_mut();
        state.anchor = anchor;
        state.focus = focus;
        state.revision = state.revision.saturating_add(1);
        SelectionChange::new(affected)
    }

    pub fn selected_items(&self) -> Vec<SelectionEntry> {
        let mut entries = self
            .inner
            .borrow()
            .owners
            .values()
            .flat_map(|owner| owner.entries.iter().cloned())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| (entry.order, entry.owner));
        entries
    }

    pub fn selected_items_for_owner(
        &self,
        owner: impl Into<SelectionOwnerId>,
    ) -> Vec<SelectionEntry> {
        let owner = owner.into();
        self.inner
            .borrow()
            .owners
            .get(&owner)
            .map(|selection| selection.entries.clone())
            .unwrap_or_default()
    }

    pub fn selected_text(&self) -> Option<String> {
        let text = self
            .selected_items()
            .into_iter()
            .filter_map(|entry| match entry.payload {
                SelectionPayload::Text(text) => Some(text),
                SelectionPayload::Image { .. } | SelectionPayload::Custom { .. } => None,
            })
            .collect::<String>();
        (!text.is_empty()).then_some(text)
    }

    pub fn text_selection_for_owner(
        &self,
        owner: impl Into<SelectionOwnerId>,
    ) -> Option<TextSelectionInfo> {
        let owner = owner.into();
        self.inner
            .borrow()
            .owners
            .get(&owner)
            .and_then(|selection| selection.text.clone())
    }

    pub fn has_owner_selection(&self, owner: impl Into<SelectionOwnerId>) -> bool {
        let owner = owner.into();
        self.inner.borrow().owners.contains_key(&owner)
    }

    pub fn replace(
        &self,
        owner: impl Into<SelectionOwnerId>,
        entries: Vec<SelectionEntry>,
    ) -> SelectionChange {
        self.select(owner, SelectionIntent::Replace, entries, None)
    }

    pub fn replace_text(
        &self,
        owner: impl Into<SelectionOwnerId>,
        order: impl Into<SelectionOrder>,
        range: Range<usize>,
        text_len: usize,
        text: impl Into<String>,
    ) -> SelectionChange {
        let owner = owner.into();
        let order = order.into();
        let selected_text = text.into();
        let entries = (!selected_text.is_empty()).then(|| {
            vec![SelectionEntry::new(
                owner,
                order,
                SelectionPayload::Text(selected_text),
            )]
        });
        self.select(
            owner,
            SelectionIntent::Replace,
            entries.unwrap_or_default(),
            Some(TextSelectionInfo {
                owner,
                range,
                text_len,
            }),
        )
    }

    pub fn extend(
        &self,
        owner: impl Into<SelectionOwnerId>,
        entries: Vec<SelectionEntry>,
    ) -> SelectionChange {
        self.select(owner, SelectionIntent::Extend, entries, None)
    }

    pub fn toggle(
        &self,
        owner: impl Into<SelectionOwnerId>,
        entries: Vec<SelectionEntry>,
    ) -> SelectionChange {
        self.select(owner, SelectionIntent::Toggle, entries, None)
    }

    pub fn clear_owner(&self, owner: impl Into<SelectionOwnerId>) -> SelectionChange {
        let owner = owner.into();
        let mut state = self.inner.borrow_mut();
        let had_owner = state.owners.remove(&owner).is_some();
        let had_active_owner = state.active_owner == Some(owner);
        let had_anchor = state.anchor.is_some_and(|point| point.owner == owner);
        let had_focus = state.focus.is_some_and(|point| point.owner == owner);
        if had_owner || had_active_owner || had_anchor || had_focus {
            if had_active_owner {
                state.active_owner = None;
            }
            if had_anchor {
                state.anchor = None;
            }
            if had_focus {
                state.focus = None;
            }
            state.revision = state.revision.saturating_add(1);
            SelectionChange::new(vec![owner])
        } else {
            SelectionChange::default()
        }
    }

    pub fn clear(&self) -> SelectionChange {
        let mut state = self.inner.borrow_mut();
        let affected = state.owners.keys().copied().collect::<Vec<_>>();
        if !state.owners.is_empty()
            || state.active_owner.is_some()
            || state.anchor.is_some()
            || state.focus.is_some()
        {
            state.owners.clear();
            state.active_owner = None;
            state.anchor = None;
            state.focus = None;
            state.revision = state.revision.saturating_add(1);
            SelectionChange::new(affected)
        } else {
            SelectionChange::default()
        }
    }

    fn select(
        &self,
        owner: impl Into<SelectionOwnerId>,
        intent: SelectionIntent,
        entries: Vec<SelectionEntry>,
        text: Option<TextSelectionInfo>,
    ) -> SelectionChange {
        let owner = owner.into();
        let mut state = self.inner.borrow_mut();
        let affected = match intent {
            SelectionIntent::Replace => state
                .owners
                .keys()
                .copied()
                .chain(std::iter::once(owner))
                .collect::<Vec<_>>(),
            SelectionIntent::Extend | SelectionIntent::Toggle => vec![owner],
        };
        match intent {
            SelectionIntent::Replace => {
                state.owners.clear();
                insert_owner_selection(&mut state, owner, entries, text);
            }
            SelectionIntent::Extend => {
                insert_owner_selection(&mut state, owner, entries, text);
            }
            SelectionIntent::Toggle => {
                if state.owners.contains_key(&owner) {
                    state.owners.remove(&owner);
                    if state.active_owner == Some(owner) {
                        state.active_owner = None;
                    }
                } else {
                    insert_owner_selection(&mut state, owner, entries, text);
                }
            }
        }
        state.revision = state.revision.saturating_add(1);
        let mut affected = affected;
        affected.sort();
        affected.dedup();
        SelectionChange::new(affected)
    }
}

fn insert_owner_selection(
    state: &mut SelectionState,
    owner: SelectionOwnerId,
    entries: Vec<SelectionEntry>,
    text: Option<TextSelectionInfo>,
) {
    state.active_owner = Some(owner);
    if entries.is_empty() && text.as_ref().is_none_or(|text| text.range.is_empty()) {
        state.owners.remove(&owner);
    } else {
        state.owners.insert(owner, OwnerSelection { entries, text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_text_updates_selected_text_and_clears_previous_owner() {
        let scope = SelectionScope::new();
        let first = SelectionOwnerId::new(1);
        let second = SelectionOwnerId::new(2);

        let change = scope.replace_text(first, first, 0..5, 5, "hello");
        assert_eq!(change.affected_owners(), &[first]);
        assert_eq!(scope.selected_text().as_deref(), Some("hello"));
        assert!(scope.has_owner_selection(first));

        let change = scope.replace_text(second, second, 0..5, 5, "world");
        assert_eq!(change.affected_owners(), &[first, second]);
        assert_eq!(scope.selected_text().as_deref(), Some("world"));
        assert!(!scope.has_owner_selection(first));
        assert!(scope.has_owner_selection(second));
    }

    #[test]
    fn extend_keeps_multiple_payload_kinds_in_order() {
        let scope = SelectionScope::new();
        let first = SelectionOwnerId::new(10);
        let second = SelectionOwnerId::new(20);

        scope.extend(
            second,
            vec![SelectionEntry::new(
                second,
                SelectionOrder::new(2),
                SelectionPayload::text("B"),
            )],
        );
        scope.extend(
            first,
            vec![SelectionEntry::new(
                first,
                SelectionOrder::new(1),
                SelectionPayload::text("A"),
            )],
        );

        assert_eq!(scope.selected_text().as_deref(), Some("AB"));
        assert_eq!(
            scope
                .selected_items()
                .iter()
                .map(|entry| entry.owner)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
    }
}
