//! Retained, same-window docking workspace for editor-style applications.
//!
//! The workspace owns every registered panel widget in one stable registry.
//! Docking, tab selection, and floating only change the logical layout graph;
//! a panel's [`WidgetId`] and widget-local state therefore survive layout
//! changes. Native operating-system window tear-out is intentionally outside
//! this widget's scope.

use crate::draw::inset_rect_evenly;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};
use sui_text::TextAlign;

use sui_core::{
    Event, KeyState, Point, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Size, WidgetId,
};
use sui_layout::{Axis, Constraints};
use sui_reactive::Signal;
use sui_runtime::{
    ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, StackHostOptions, StackOrderPolicy,
    Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};

use crate::controls::draw_icon_glyph;
#[cfg(feature = "serde")]
#[path = "docking_serde.rs"]
mod serde_support;

use crate::frame::{snap_to_pixels, stroke_border};
use crate::{DefaultTheme, IconGlyph, text_align::paint_text_line};

const MAX_DOCK_DEPTH: usize = 64;
const MAX_DOCK_NODES: usize = 4_096;
const MAX_FLOATING_GROUPS: usize = 256;
const MIN_SPLIT_FRACTION: f32 = 0.05;
const MAX_SPLIT_FRACTION: f32 = 0.95;
const DEFAULT_SPLITTER_THICKNESS: f32 = 4.0;
const MIN_DOCK_PANE_EXTENT: f32 = 96.0;
const MIN_FLOATING_WIDTH: f32 = 180.0;
const MIN_FLOATING_HEIGHT: f32 = 120.0;
const FLOATING_RESIZE_HANDLE: f32 = 16.0;
const FLOATING_GRIP_WIDTH: f32 = 32.0;
const TAB_DRAG_THRESHOLD: f32 = 5.0;
const DROP_EDGE_FRACTION: f32 = 0.24;
const DROP_ZONE_INSET: f32 = 6.0;
const SEMANTIC_SPLIT_STEP: f32 = 0.05;
const TAB_TEXT_INSET: f32 = 8.0;
const TAB_ICON_EXTENT: f32 = 14.0;
const TAB_CLOSE_EXTENT: f32 = 16.0;
const TAB_DIRTY_EXTENT: f32 = 6.0;
const TAB_PART_GAP: f32 = 6.0;
const TAB_MIN_WIDTH: f32 = 72.0;
const TAB_MAX_TITLE_WIDTH: f32 = 180.0;
const TAB_MIN_CLOSE_WIDTH: f32 = 44.0;
const TAB_MIN_ICON_WIDTH: f32 = 64.0;
const TAB_INSERT_INDICATOR: f32 = 2.0;

/// Stable application-defined identity for one dockable panel.
///
/// Persist this value in application settings. It is deliberately independent
/// from the runtime-generated [`WidgetId`]. Zero is reserved as an invalid ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DockPanelId(u64);

impl DockPanelId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl From<u64> for DockPanelId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<DockPanelId> for u64 {
    fn from(value: DockPanelId) -> Self {
        value.get()
    }
}

impl fmt::Display for DockPanelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// One node in the persisted docking graph.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DockNode {
    #[default]
    Empty,
    Tabs {
        panels: Vec<DockPanelId>,
        active: DockPanelId,
    },
    Split {
        axis: Axis,
        fraction: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
}

impl DockNode {
    pub const fn empty() -> Self {
        Self::Empty
    }

    pub fn tabs(panels: impl IntoIterator<Item = DockPanelId>, active: DockPanelId) -> Self {
        Self::Tabs {
            panels: panels.into_iter().collect(),
            active,
        }
    }

    pub fn split(axis: Axis, fraction: f32, first: Self, second: Self) -> Self {
        Self::Split {
            axis,
            fraction: normalize_fraction(fraction),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

/// A tab group floating above the docked graph inside the same SUI window.
#[derive(Debug, Clone, PartialEq)]
pub struct DockFloatingGroup {
    pub id: u64,
    pub panels: Vec<DockPanelId>,
    pub active: DockPanelId,
    pub bounds: Rect,
}

impl DockFloatingGroup {
    pub fn new(
        id: u64,
        panels: impl IntoIterator<Item = DockPanelId>,
        active: DockPanelId,
        bounds: Rect,
    ) -> Self {
        Self {
            id,
            panels: panels.into_iter().collect(),
            active,
            bounds,
        }
    }
}

/// Plain persistence boundary for a docking workspace.
///
/// SUI does not prescribe a serializer or settings location. Applications may
/// map this value into their own versioned settings format and later restore it
/// through [`DockWorkspaceState::apply_snapshot`]. Floating groups are ordered
/// back-to-front.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DockWorkspaceSnapshot {
    pub root: DockNode,
    pub floating: Vec<DockFloatingGroup>,
    pub hidden: Vec<DockPanelId>,
}

impl DockWorkspaceSnapshot {
    pub fn new(root: DockNode) -> Self {
        Self {
            root,
            floating: Vec::new(),
            hidden: Vec::new(),
        }
    }

    pub fn with_floating(mut self, group: DockFloatingGroup) -> Self {
        self.floating.push(group);
        self
    }

    pub fn with_hidden(mut self, panel: DockPanelId) -> Self {
        self.hidden.push(panel);
        self
    }

    /// Drops every panel `keep` rejects and collapses the groups and splits
    /// left empty.
    ///
    /// Use it before restoring a saved layout whose panels may no longer all
    /// exist, such as documents that were deleted or extensions that were
    /// removed.
    pub fn retain_panels(&mut self, keep: impl Fn(DockPanelId) -> bool) {
        let mut panels = HashSet::new();
        collect_snapshot_panels(self, &mut panels);
        for panel in panels {
            if !keep(panel) {
                remove_panel_from_snapshot(self, panel);
            }
        }
    }
}

/// Destination zone used when docking a panel relative to a tab group or the
/// workspace root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// Presentation and behavior of one dock panel's tab.
///
/// Options are owned by the workspace's panel registry. Change them after
/// registration with [`DockWorkspaceState::update_panel`].
#[derive(Debug, Clone, PartialEq)]
pub struct DockPanelOptions {
    pub title: String,
    pub icon: Option<IconGlyph>,
    /// Shows a close button on the tab. A close press is routed through
    /// [`DockWorkspace::on_close_request`].
    pub closable: bool,
    /// Marks the tab as holding unsaved changes.
    pub dirty: bool,
    /// Smallest content size the panel remains usable at. Splits keep every
    /// docked group at least this large while the workspace has room.
    pub min_size: Size,
}

impl DockPanelOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            closable: false,
            dirty: false,
            min_size: Size::ZERO,
        }
    }

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn min_size(mut self, min_size: Size) -> Self {
        self.min_size = min_size;
        self
    }
}

impl From<&str> for DockPanelOptions {
    fn from(title: &str) -> Self {
        Self::new(title)
    }
}

impl From<String> for DockPanelOptions {
    fn from(title: String) -> Self {
        Self::new(title)
    }
}

/// Where a panel added at runtime first appears.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockPlacement {
    /// Relative to the whole docked graph.
    Root(DockZone),
    /// Relative to the tab group that contains `target`.
    Beside { target: DockPanelId, zone: DockZone },
    /// In a new same-window floating group.
    Floating(Rect),
    /// Registered but not shown until [`DockWorkspaceState::show`].
    Hidden,
}

/// What the workspace does after a tab's close button is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockCloseAction {
    /// Keep the panel widget and move it to the hidden list.
    #[default]
    Hide,
    /// Drop the panel widget and forget its ID.
    Remove,
    /// Leave the panel open, for example while the application asks about
    /// unsaved changes.
    Keep,
}

/// Validation or mutation failure for a docking layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockLayoutError {
    message: String,
}

impl DockLayoutError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DockLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DockLayoutError {}

/// Cloneable observable state for [`DockWorkspace`].
#[derive(Clone)]
pub struct DockWorkspaceState {
    snapshot: Signal<DockWorkspaceSnapshot>,
    registry: Arc<Mutex<DockRegistryQueue>>,
    registry_revision: Signal<u64>,
}

impl fmt::Debug for DockWorkspaceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockWorkspaceState")
            .field("snapshot", &self.snapshot)
            .field("pending_registry_ops", &self.registry().ops.len())
            .finish()
    }
}

/// Registry changes requested through the state and applied by the workspace
/// widget, which owns the panel widgets, on its next measure pass.
///
/// The queue holds plain data only, so the state stays `Send + Sync` and can
/// be driven from another thread like the layout signal it sits beside.
#[derive(Default)]
struct DockRegistryQueue {
    registered: HashSet<DockPanelId>,
    ops: Vec<DockRegistryOp>,
}

enum DockRegistryOp {
    Add {
        id: DockPanelId,
        options: DockPanelOptions,
    },
    Remove(DockPanelId),
    Update {
        id: DockPanelId,
        update: Box<dyn FnOnce(&mut DockPanelOptions) + Send>,
    },
}

impl DockWorkspaceState {
    pub fn new(snapshot: DockWorkspaceSnapshot) -> Result<Self, DockLayoutError> {
        validate_snapshot(&snapshot)?;
        Ok(Self {
            snapshot: Signal::named("Dock workspace state", snapshot),
            registry: Arc::new(Mutex::new(DockRegistryQueue::default())),
            registry_revision: Signal::named("Dock workspace panel registry", 0),
        })
    }

    pub fn empty() -> Self {
        Self::new(DockWorkspaceSnapshot::default())
            .expect("the empty dock workspace snapshot is valid")
    }

    pub fn snapshot(&self) -> DockWorkspaceSnapshot {
        self.snapshot.get()
    }

    /// Applies a structurally valid dock graph.
    ///
    /// Panel widgets are registered by [`DockWorkspace`], not by this state
    /// object, so this method cannot reject otherwise-valid snapshots that
    /// reference an unregistered panel ID. Prefer
    /// [`DockWorkspace::apply_snapshot`] when restoring application-owned
    /// settings after the panel registry has been assembled.
    pub fn apply_snapshot(&self, snapshot: DockWorkspaceSnapshot) -> Result<bool, DockLayoutError> {
        validate_snapshot(&snapshot)?;
        Ok(self.snapshot.set(snapshot))
    }

    pub fn dock(
        &self,
        panel: DockPanelId,
        target: DockPanelId,
        zone: DockZone,
    ) -> Result<bool, DockLayoutError> {
        validate_panel_id(panel)?;
        validate_panel_id(target)?;
        if panel == target {
            return self.activate(panel);
        }

        self.mutate(|snapshot| {
            if !visible_snapshot_contains_panel(snapshot, target) {
                return Err(DockLayoutError::new(format!(
                    "dock target panel {target} is not visible in the workspace"
                )));
            }
            remove_panel_from_snapshot(snapshot, panel);

            if dock_into_root_node(&mut snapshot.root, panel, target, zone) {
                return Ok(());
            }

            let Some(group) = snapshot
                .floating
                .iter_mut()
                .find(|group| group.panels.contains(&target))
            else {
                return Err(DockLayoutError::new(format!(
                    "dock target panel {target} disappeared during mutation"
                )));
            };
            if zone != DockZone::Center {
                return Err(DockLayoutError::new(
                    "floating tab groups accept center docking only",
                ));
            }
            insert_tab(&mut group.panels, &mut group.active, panel);
            Ok(())
        })
    }

    pub fn dock_to_root(
        &self,
        panel: DockPanelId,
        zone: DockZone,
    ) -> Result<bool, DockLayoutError> {
        validate_panel_id(panel)?;
        self.mutate(|snapshot| {
            remove_panel_from_snapshot(snapshot, panel);
            let incoming = DockNode::tabs([panel], panel);
            if snapshot.root.is_empty() {
                snapshot.root = incoming;
                return Ok(());
            }

            if zone == DockZone::Center {
                if insert_into_first_tab_group(&mut snapshot.root, panel) {
                    return Ok(());
                }
                snapshot.root = incoming;
                return Ok(());
            }

            let previous = std::mem::take(&mut snapshot.root);
            snapshot.root = split_for_zone(previous, incoming, zone);
            Ok(())
        })
    }

    pub fn float_panel(&self, panel: DockPanelId, bounds: Rect) -> Result<u64, DockLayoutError> {
        validate_panel_id(panel)?;
        validate_floating_bounds(bounds)?;
        let mut assigned = 0;
        self.mutate(|snapshot| {
            remove_panel_from_snapshot(snapshot, panel);
            assigned = next_floating_group_id(snapshot)?;
            snapshot.floating.push(DockFloatingGroup::new(
                assigned,
                [panel],
                panel,
                normalized_floating_bounds(bounds),
            ));
            Ok(())
        })?;
        Ok(assigned)
    }

    pub fn hide(&self, panel: DockPanelId) -> Result<bool, DockLayoutError> {
        validate_panel_id(panel)?;
        self.mutate(|snapshot| {
            remove_panel_from_snapshot(snapshot, panel);
            if !snapshot.hidden.contains(&panel) {
                snapshot.hidden.push(panel);
            }
            Ok(())
        })
    }

    pub fn show(&self, panel: DockPanelId) -> Result<bool, DockLayoutError> {
        validate_panel_id(panel)?;
        self.mutate(|snapshot| {
            snapshot.hidden.retain(|candidate| *candidate != panel);
            if snapshot_contains_panel(snapshot, panel) {
                activate_panel(snapshot, panel);
            } else if !insert_into_first_tab_group(&mut snapshot.root, panel) {
                snapshot.root = DockNode::tabs([panel], panel);
            }
            Ok(())
        })
    }

    pub fn activate(&self, panel: DockPanelId) -> Result<bool, DockLayoutError> {
        validate_panel_id(panel)?;
        self.mutate(|snapshot| {
            if !activate_panel(snapshot, panel) {
                return Err(DockLayoutError::new(format!(
                    "panel {panel} is not visible in the workspace"
                )));
            }
            Ok(())
        })
    }

    pub fn set_floating_bounds(
        &self,
        group_id: u64,
        bounds: Rect,
    ) -> Result<bool, DockLayoutError> {
        validate_floating_bounds(bounds)?;
        self.mutate(|snapshot| {
            let Some(group) = snapshot
                .floating
                .iter_mut()
                .find(|group| group.id == group_id)
            else {
                return Err(DockLayoutError::new(format!(
                    "floating group {group_id} does not exist"
                )));
            };
            group.bounds = normalized_floating_bounds(bounds);
            Ok(())
        })
    }

    pub fn bring_floating_to_front(&self, group_id: u64) -> Result<bool, DockLayoutError> {
        self.mutate(|snapshot| {
            let Some(index) = snapshot
                .floating
                .iter()
                .position(|group| group.id == group_id)
            else {
                return Err(DockLayoutError::new(format!(
                    "floating group {group_id} does not exist"
                )));
            };
            if index + 1 != snapshot.floating.len() {
                let group = snapshot.floating.remove(index);
                snapshot.floating.push(group);
            }
            Ok(())
        })
    }

    /// Moves `panel` into the tab group that contains `anchor`, immediately
    /// before or after the anchor's tab, and activates it.
    ///
    /// Use it to reorder tabs within a group as well as to insert a tab at a
    /// chosen position in another group.
    pub fn dock_tab(
        &self,
        panel: DockPanelId,
        anchor: DockPanelId,
        after: bool,
    ) -> Result<bool, DockLayoutError> {
        validate_panel_id(panel)?;
        validate_panel_id(anchor)?;
        if panel == anchor {
            return self.activate(panel);
        }
        self.mutate(|snapshot| {
            if !visible_snapshot_contains_panel(snapshot, anchor) {
                return Err(DockLayoutError::new(format!(
                    "dock anchor panel {anchor} is not visible in the workspace"
                )));
            }
            remove_panel_from_snapshot(snapshot, panel);
            if insert_tab_beside_in_node(&mut snapshot.root, panel, anchor, after) {
                return Ok(());
            }
            let Some(group) = snapshot
                .floating
                .iter_mut()
                .find(|group| group.panels.contains(&anchor))
            else {
                return Err(DockLayoutError::new(format!(
                    "dock anchor panel {anchor} disappeared during mutation"
                )));
            };
            insert_tab_beside(&mut group.panels, &mut group.active, panel, anchor, after);
            Ok(())
        })
    }

    /// Registers a panel while the workspace is mounted and places it.
    ///
    /// The workspace creates the panel's widget on its next measure pass by
    /// calling its [`DockWorkspace::panel_builder`] with `id`. The ID must not
    /// belong to a panel that is already registered.
    pub fn add_panel(
        &self,
        id: DockPanelId,
        options: impl Into<DockPanelOptions>,
        placement: DockPlacement,
    ) -> Result<(), DockLayoutError> {
        validate_panel_id(id)?;
        if self.registry().registered.contains(&id) {
            return Err(DockLayoutError::new(format!(
                "dock panel {id} is already registered"
            )));
        }
        match placement {
            DockPlacement::Root(zone) => {
                self.dock_to_root(id, zone)?;
            }
            DockPlacement::Beside { target, zone } => {
                self.dock(id, target, zone)?;
            }
            DockPlacement::Floating(bounds) => {
                self.float_panel(id, bounds)?;
            }
            DockPlacement::Hidden => {
                self.hide(id)?;
            }
        }
        {
            let mut registry = self.registry();
            registry.registered.insert(id);
            registry.ops.push(DockRegistryOp::Add {
                id,
                options: options.into(),
            });
        }
        self.bump_registry();
        Ok(())
    }

    /// Removes a panel from the layout and drops its widget.
    ///
    /// Returns whether the panel was registered. Unlike [`Self::hide`], the
    /// panel's widget-local state is discarded and its ID may be reused.
    pub fn remove_panel(&self, id: DockPanelId) -> Result<bool, DockLayoutError> {
        validate_panel_id(id)?;
        self.mutate(|snapshot| {
            remove_panel_from_snapshot(snapshot, id);
            Ok(())
        })?;
        let was_registered = {
            let mut registry = self.registry();
            let was_registered = registry.registered.remove(&id);
            if was_registered {
                registry.ops.push(DockRegistryOp::Remove(id));
            }
            was_registered
        };
        if was_registered {
            self.bump_registry();
        }
        Ok(was_registered)
    }

    /// Changes a registered panel's tab options, such as its title or its
    /// unsaved-changes marker.
    pub fn update_panel(
        &self,
        id: DockPanelId,
        update: impl FnOnce(&mut DockPanelOptions) + Send + 'static,
    ) -> Result<(), DockLayoutError> {
        validate_panel_id(id)?;
        {
            let mut registry = self.registry();
            if !registry.registered.contains(&id) {
                return Err(DockLayoutError::new(format!(
                    "dock panel {id} is not registered"
                )));
            }
            registry.ops.push(DockRegistryOp::Update {
                id,
                update: Box::new(update),
            });
        }
        self.bump_registry();
        Ok(())
    }

    /// Whether a panel widget is registered or queued for registration.
    pub fn is_registered(&self, id: DockPanelId) -> bool {
        self.registry().registered.contains(&id)
    }

    fn registry(&self) -> MutexGuard<'_, DockRegistryQueue> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn bump_registry(&self) {
        self.registry_revision
            .set(self.registry_revision.get().wrapping_add(1));
    }

    fn mutate(
        &self,
        mutate: impl FnOnce(&mut DockWorkspaceSnapshot) -> Result<(), DockLayoutError>,
    ) -> Result<bool, DockLayoutError> {
        let mut snapshot = self.snapshot();
        let previous = snapshot.clone();
        mutate(&mut snapshot)?;
        validate_snapshot(&snapshot)?;
        if snapshot == previous {
            return Ok(false);
        }
        Ok(self.snapshot.set(snapshot))
    }

    fn set_split_fraction(
        &self,
        path: &[DockBranch],
        fraction: f32,
    ) -> Result<bool, DockLayoutError> {
        self.mutate(|snapshot| {
            let Some(node) = node_at_path_mut(&mut snapshot.root, path) else {
                return Err(DockLayoutError::new("split path is no longer valid"));
            };
            let DockNode::Split {
                fraction: current, ..
            } = node
            else {
                return Err(DockLayoutError::new(
                    "split path does not identify a split node",
                ));
            };
            *current = normalize_fraction(fraction);
            Ok(())
        })
    }
}

impl Default for DockWorkspaceState {
    fn default() -> Self {
        Self::empty()
    }
}

fn validate_panel_id(panel: DockPanelId) -> Result<(), DockLayoutError> {
    if panel.is_valid() {
        Ok(())
    } else {
        Err(DockLayoutError::new("dock panel ID zero is reserved"))
    }
}

fn validate_snapshot(snapshot: &DockWorkspaceSnapshot) -> Result<(), DockLayoutError> {
    if snapshot.floating.len() > MAX_FLOATING_GROUPS {
        return Err(DockLayoutError::new(format!(
            "dock snapshot contains more than {MAX_FLOATING_GROUPS} floating groups"
        )));
    }

    let mut panels = HashSet::new();
    let mut node_count = 0;
    validate_node(&snapshot.root, 0, &mut node_count, &mut panels)?;

    let mut floating_ids = HashSet::new();
    for group in &snapshot.floating {
        if group.id == 0 || !floating_ids.insert(group.id) {
            return Err(DockLayoutError::new(format!(
                "floating group ID {} is zero or duplicated",
                group.id
            )));
        }
        if group.panels.is_empty() {
            return Err(DockLayoutError::new(format!(
                "floating group {} has no panels",
                group.id
            )));
        }
        if !group.panels.contains(&group.active) {
            return Err(DockLayoutError::new(format!(
                "floating group {} active panel {} is not a member",
                group.id, group.active
            )));
        }
        validate_floating_bounds(group.bounds)?;
        for panel in &group.panels {
            validate_panel_id(*panel)?;
            if !panels.insert(*panel) {
                return Err(DockLayoutError::new(format!(
                    "panel {panel} appears more than once in the dock snapshot"
                )));
            }
        }
    }

    for panel in &snapshot.hidden {
        validate_panel_id(*panel)?;
        if !panels.insert(*panel) {
            return Err(DockLayoutError::new(format!(
                "panel {panel} appears more than once in the dock snapshot"
            )));
        }
    }
    Ok(())
}

fn validate_node(
    node: &DockNode,
    depth: usize,
    node_count: &mut usize,
    panels: &mut HashSet<DockPanelId>,
) -> Result<(), DockLayoutError> {
    if depth > MAX_DOCK_DEPTH {
        return Err(DockLayoutError::new(format!(
            "dock graph exceeds maximum depth {MAX_DOCK_DEPTH}"
        )));
    }
    *node_count += 1;
    if *node_count > MAX_DOCK_NODES {
        return Err(DockLayoutError::new(format!(
            "dock graph exceeds maximum node count {MAX_DOCK_NODES}"
        )));
    }

    match node {
        DockNode::Empty => Ok(()),
        DockNode::Tabs {
            panels: members,
            active,
        } => {
            if members.is_empty() {
                return Err(DockLayoutError::new(
                    "dock tab nodes must contain at least one panel",
                ));
            }
            if !members.contains(active) {
                return Err(DockLayoutError::new(format!(
                    "active panel {active} is not a member of its dock tab node"
                )));
            }
            for panel in members {
                validate_panel_id(*panel)?;
                if !panels.insert(*panel) {
                    return Err(DockLayoutError::new(format!(
                        "panel {panel} appears more than once in the dock snapshot"
                    )));
                }
            }
            Ok(())
        }
        DockNode::Split {
            fraction,
            first,
            second,
            ..
        } => {
            if !fraction.is_finite()
                || *fraction < MIN_SPLIT_FRACTION
                || *fraction > MAX_SPLIT_FRACTION
            {
                return Err(DockLayoutError::new(format!(
                    "dock split fraction {fraction} is outside {MIN_SPLIT_FRACTION}..={MAX_SPLIT_FRACTION}"
                )));
            }
            if first.is_empty() || second.is_empty() {
                return Err(DockLayoutError::new(
                    "dock split nodes cannot contain an empty branch",
                ));
            }
            validate_node(first, depth + 1, node_count, panels)?;
            validate_node(second, depth + 1, node_count, panels)
        }
    }
}

fn validate_floating_bounds(bounds: Rect) -> Result<(), DockLayoutError> {
    if !bounds.x().is_finite()
        || !bounds.y().is_finite()
        || !bounds.width().is_finite()
        || !bounds.height().is_finite()
        || bounds.width() <= 0.0
        || bounds.height() <= 0.0
    {
        return Err(DockLayoutError::new(
            "floating bounds must have finite coordinates and positive finite size",
        ));
    }
    Ok(())
}

fn normalized_floating_bounds(bounds: Rect) -> Rect {
    Rect::new(
        bounds.x(),
        bounds.y(),
        bounds.width().max(MIN_FLOATING_WIDTH),
        bounds.height().max(MIN_FLOATING_HEIGHT),
    )
}

fn normalize_fraction(fraction: f32) -> f32 {
    if fraction.is_finite() {
        fraction.clamp(MIN_SPLIT_FRACTION, MAX_SPLIT_FRACTION)
    } else {
        0.5
    }
}

fn snapshot_contains_panel(snapshot: &DockWorkspaceSnapshot, panel: DockPanelId) -> bool {
    visible_snapshot_contains_panel(snapshot, panel) || snapshot.hidden.contains(&panel)
}

fn visible_snapshot_contains_panel(snapshot: &DockWorkspaceSnapshot, panel: DockPanelId) -> bool {
    node_contains_panel(&snapshot.root, panel)
        || snapshot
            .floating
            .iter()
            .any(|group| group.panels.contains(&panel))
}

fn node_contains_panel(node: &DockNode, panel: DockPanelId) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, .. } => panels.contains(&panel),
        DockNode::Split { first, second, .. } => {
            node_contains_panel(first, panel) || node_contains_panel(second, panel)
        }
    }
}

fn remove_panel_from_snapshot(snapshot: &mut DockWorkspaceSnapshot, panel: DockPanelId) -> bool {
    let mut removed = remove_panel_from_node(&mut snapshot.root, panel);
    for group in &mut snapshot.floating {
        if let Some(index) = group
            .panels
            .iter()
            .position(|candidate| *candidate == panel)
        {
            group.panels.remove(index);
            if group.active == panel
                && let Some(next) = group.panels.first().copied()
            {
                group.active = next;
            }
            removed = true;
        }
    }
    snapshot.floating.retain(|group| !group.panels.is_empty());
    let hidden_len = snapshot.hidden.len();
    snapshot.hidden.retain(|candidate| *candidate != panel);
    removed || snapshot.hidden.len() != hidden_len
}

fn remove_panel_from_node(node: &mut DockNode, panel: DockPanelId) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, active } => {
            let Some(index) = panels.iter().position(|candidate| *candidate == panel) else {
                return false;
            };
            panels.remove(index);
            if panels.is_empty() {
                *node = DockNode::Empty;
            } else if *active == panel {
                *active = panels[index.min(panels.len() - 1)];
            }
            true
        }
        DockNode::Split { first, second, .. } => {
            let removed =
                remove_panel_from_node(first, panel) || remove_panel_from_node(second, panel);
            if removed {
                compact_split(node);
            }
            removed
        }
    }
}

fn compact_split(node: &mut DockNode) {
    let replacement = match node {
        DockNode::Split { first, second, .. } if first.is_empty() => {
            Some(std::mem::take(second.as_mut()))
        }
        DockNode::Split { first, second, .. } if second.is_empty() => {
            Some(std::mem::take(first.as_mut()))
        }
        _ => None,
    };
    if let Some(replacement) = replacement {
        *node = replacement;
    }
}

fn insert_tab(panels: &mut Vec<DockPanelId>, active: &mut DockPanelId, panel: DockPanelId) {
    if !panels.contains(&panel) {
        panels.push(panel);
    }
    *active = panel;
}

fn insert_tab_beside(
    panels: &mut Vec<DockPanelId>,
    active: &mut DockPanelId,
    panel: DockPanelId,
    anchor: DockPanelId,
    after: bool,
) {
    let index = panels
        .iter()
        .position(|candidate| *candidate == anchor)
        .map_or(panels.len(), |index| index + usize::from(after));
    panels.insert(index, panel);
    *active = panel;
}

fn insert_tab_beside_in_node(
    node: &mut DockNode,
    panel: DockPanelId,
    anchor: DockPanelId,
    after: bool,
) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, active } if panels.contains(&anchor) => {
            insert_tab_beside(panels, active, panel, anchor, after);
            true
        }
        DockNode::Tabs { .. } => false,
        DockNode::Split { first, second, .. } => {
            insert_tab_beside_in_node(first, panel, anchor, after)
                || insert_tab_beside_in_node(second, panel, anchor, after)
        }
    }
}

fn insert_into_first_tab_group(node: &mut DockNode, panel: DockPanelId) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, active } => {
            insert_tab(panels, active, panel);
            true
        }
        DockNode::Split { first, second, .. } => {
            insert_into_first_tab_group(first, panel) || insert_into_first_tab_group(second, panel)
        }
    }
}

fn dock_into_root_node(
    node: &mut DockNode,
    panel: DockPanelId,
    target: DockPanelId,
    zone: DockZone,
) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, active } if panels.contains(&target) => {
            if zone == DockZone::Center {
                insert_tab(panels, active, panel);
            } else {
                let previous = std::mem::take(node);
                *node = split_for_zone(previous, DockNode::tabs([panel], panel), zone);
            }
            true
        }
        DockNode::Tabs { .. } => false,
        DockNode::Split { first, second, .. } => {
            dock_into_root_node(first, panel, target, zone)
                || dock_into_root_node(second, panel, target, zone)
        }
    }
}

fn split_for_zone(existing: DockNode, incoming: DockNode, zone: DockZone) -> DockNode {
    match zone {
        DockZone::Left => DockNode::split(Axis::Horizontal, 0.28, incoming, existing),
        DockZone::Right => DockNode::split(Axis::Horizontal, 0.72, existing, incoming),
        DockZone::Top => DockNode::split(Axis::Vertical, 0.28, incoming, existing),
        DockZone::Bottom => DockNode::split(Axis::Vertical, 0.72, existing, incoming),
        DockZone::Center => existing,
    }
}

fn activate_panel(snapshot: &mut DockWorkspaceSnapshot, panel: DockPanelId) -> bool {
    if activate_in_node(&mut snapshot.root, panel) {
        return true;
    }
    let Some(index) = snapshot
        .floating
        .iter()
        .position(|group| group.panels.contains(&panel))
    else {
        return false;
    };
    snapshot.floating[index].active = panel;
    if index + 1 != snapshot.floating.len() {
        let group = snapshot.floating.remove(index);
        snapshot.floating.push(group);
    }
    true
}

fn activate_in_node(node: &mut DockNode, panel: DockPanelId) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, active } if panels.contains(&panel) => {
            *active = panel;
            true
        }
        DockNode::Tabs { .. } => false,
        DockNode::Split { first, second, .. } => {
            activate_in_node(first, panel) || activate_in_node(second, panel)
        }
    }
}

fn next_floating_group_id(snapshot: &DockWorkspaceSnapshot) -> Result<u64, DockLayoutError> {
    snapshot
        .floating
        .iter()
        .map(|group| group.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|id| *id != 0)
        .ok_or_else(|| DockLayoutError::new("floating group ID space is exhausted"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockBranch {
    First,
    Second,
}

fn node_at_path_mut<'a>(
    mut node: &'a mut DockNode,
    path: &[DockBranch],
) -> Option<&'a mut DockNode> {
    for branch in path {
        let DockNode::Split { first, second, .. } = node else {
            return None;
        };
        node = match branch {
            DockBranch::First => first,
            DockBranch::Second => second,
        };
    }
    Some(node)
}

fn node_at_path<'a>(mut node: &'a DockNode, path: &[DockBranch]) -> Option<&'a DockNode> {
    for branch in path {
        let DockNode::Split { first, second, .. } = node else {
            return None;
        };
        node = match branch {
            DockBranch::First => first,
            DockBranch::Second => second,
        };
    }
    Some(node)
}

struct DockPanelEntry {
    id: DockPanelId,
    options: DockPanelOptions,
    child: WidgetPod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DockGroupLocation {
    Docked(Vec<DockBranch>),
    Floating(u64),
}

#[derive(Debug, Clone)]
struct DockTabLayout {
    panel: DockPanelId,
    bounds: Rect,
    label: Rect,
    icon: Option<Rect>,
    dirty: Option<Rect>,
    close: Option<Rect>,
}

/// A position between two tabs of one group that a dragged tab can take.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DockTabInsert {
    anchor: DockPanelId,
    after: bool,
    indicator: Rect,
}

#[derive(Debug, Clone)]
struct DockGroupLayout {
    location: DockGroupLocation,
    bounds: Rect,
    header: Rect,
    content: Rect,
    panels: Vec<DockPanelId>,
    active: DockPanelId,
    tabs: Vec<DockTabLayout>,
}

impl DockGroupLayout {
    fn is_floating(&self) -> bool {
        matches!(self.location, DockGroupLocation::Floating(_))
    }

    fn tab_at(&self, position: Point) -> Option<DockPanelId> {
        self.tabs
            .iter()
            .find(|tab| tab.bounds.contains(position))
            .map(|tab| tab.panel)
    }

    fn close_at(&self, position: Point) -> Option<DockPanelId> {
        self.tabs
            .iter()
            .find(|tab| tab.close.is_some_and(|close| close.contains(position)))
            .map(|tab| tab.panel)
    }

    /// The slot a tab dragged over this group's header would be inserted at.
    fn tab_insert_at(&self, position: Point, dragged: DockPanelId) -> Option<DockTabInsert> {
        if !self.header.contains(position) || self.tabs.iter().all(|tab| tab.panel == dragged) {
            return None;
        }
        let slot = |tab: &DockTabLayout, after: bool| {
            let x = if after {
                tab.bounds.max_x() - TAB_INSERT_INDICATOR
            } else {
                tab.bounds.x()
            };
            DockTabInsert {
                anchor: tab.panel,
                after,
                indicator: Rect::new(
                    x.max(self.header.x()),
                    self.header.y(),
                    TAB_INSERT_INDICATOR,
                    self.header.height(),
                ),
            }
        };
        self.tabs
            .iter()
            .find(|tab| position.x < tab.bounds.x() + tab.bounds.width() * 0.5)
            .map(|tab| slot(tab, false))
            .or_else(|| self.tabs.last().map(|tab| slot(tab, true)))
    }
}

#[derive(Debug, Clone)]
struct DockSplitterLayout {
    path: Vec<DockBranch>,
    axis: Axis,
    fraction: f32,
    bounds: Rect,
    split_bounds: Rect,
}

#[derive(Debug, Clone, Default)]
struct DockLayoutCache {
    bounds: Rect,
    groups: Vec<DockGroupLayout>,
    splitters: Vec<DockSplitterLayout>,
}

impl DockLayoutCache {
    fn visible_panel_ids(&self) -> impl Iterator<Item = DockPanelId> + '_ {
        self.groups.iter().map(|group| group.active)
    }

    fn floating_groups_front_to_back(&self) -> impl Iterator<Item = &DockGroupLayout> {
        self.groups.iter().rev().filter(|group| group.is_floating())
    }
}

enum DockWorkspaceGesture {
    ClosePress {
        pointer_id: u64,
        panel: DockPanelId,
    },
    TabPress {
        pointer_id: u64,
        panel: DockPanelId,
        start_position: Point,
        floating_size: Size,
    },
    TabDrag {
        pointer_id: u64,
        panel: DockPanelId,
        position: Point,
        floating_size: Size,
        candidate: Option<DockDropCandidate>,
    },
    Split {
        pointer_id: u64,
        path: Vec<DockBranch>,
        axis: Axis,
        split_bounds: Rect,
    },
    FloatingMove {
        pointer_id: u64,
        group_id: u64,
        pointer_origin: Point,
        initial_bounds: Rect,
    },
    FloatingResize {
        pointer_id: u64,
        group_id: u64,
        pointer_origin: Point,
        initial_bounds: Rect,
    },
}

impl DockWorkspaceGesture {
    fn pointer_id(&self) -> u64 {
        match self {
            Self::ClosePress { pointer_id, .. }
            | Self::TabPress { pointer_id, .. }
            | Self::TabDrag { pointer_id, .. }
            | Self::Split { pointer_id, .. }
            | Self::FloatingMove { pointer_id, .. }
            | Self::FloatingResize { pointer_id, .. } => *pointer_id,
        }
    }
}

#[derive(Debug, Clone)]
struct DockDropCandidate {
    target: Option<DockPanelId>,
    zone: DockZone,
    host_bounds: Rect,
    zone_bounds: Rect,
    center_only: bool,
    insert: Option<DockTabInsert>,
}

impl DockDropCandidate {
    fn insert(host_bounds: Rect, insert: DockTabInsert) -> Self {
        Self {
            target: Some(insert.anchor),
            zone: DockZone::Center,
            host_bounds,
            zone_bounds: insert.indicator,
            center_only: true,
            insert: Some(insert),
        }
    }
}

type DockCloseRequest = Box<dyn FnMut(DockPanelId) -> DockCloseAction>;
type DockPanelBuilder = Box<dyn FnMut(DockPanelId) -> Box<dyn Widget>>;

type ThemeReader = std::rc::Rc<dyn Fn() -> DefaultTheme>;

/// Retained editor workspace with a split/tab dock graph and same-window
/// floating panels.
///
/// Every panel widget remains owned by this workspace until it is removed
/// through [`DockWorkspaceState::remove_panel`]. Layout changes only alter
/// which registered panel is measured, arranged, and visited at each logical
/// location.
pub struct DockWorkspace {
    theme: Box<DefaultTheme>,
    theme_reader: Option<ThemeReader>,
    name: String,
    state: DockWorkspaceState,
    panels: Vec<DockPanelEntry>,
    layout: DockLayoutCache,
    gesture: Option<DockWorkspaceGesture>,
    focused_group: Option<DockGroupLocation>,
    hovered_close: Option<DockPanelId>,
    on_close_request: Option<DockCloseRequest>,
    panel_builder: Option<DockPanelBuilder>,
}

impl DockWorkspace {
    pub fn new(state: DockWorkspaceState) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: "Dock workspace".to_string(),
            state,
            panels: Vec::new(),
            layout: DockLayoutCache::default(),
            gesture: None,
            focused_group: None,
            hovered_close: None,
            on_close_request: None,
            panel_builder: None,
        }
    }

    /// Creates the widget for a panel added through
    /// [`DockWorkspaceState::add_panel`].
    ///
    /// The builder runs once per added panel, on the UI thread. Without a
    /// builder an added panel has a tab and empty content.
    pub fn panel_builder<F>(mut self, panel_builder: F) -> Self
    where
        F: FnMut(DockPanelId) -> Box<dyn Widget> + 'static,
    {
        self.panel_builder = Some(Box::new(panel_builder));
        self
    }

    /// Decides what a press on a tab's close button does.
    ///
    /// Without a handler a closed panel is hidden, which keeps its widget and
    /// lets [`DockWorkspaceState::show`] bring it back.
    pub fn on_close_request<F>(mut self, on_close_request: F) -> Self
    where
        F: FnMut(DockPanelId) -> DockCloseAction + 'static,
    {
        self.on_close_request = Some(Box::new(on_close_request));
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(std::rc::Rc::new(theme));
        self
    }

    /// Registers a panel. `options` is a title or a full [`DockPanelOptions`].
    pub fn with_panel<W>(
        mut self,
        id: DockPanelId,
        options: impl Into<DockPanelOptions>,
        child: W,
    ) -> Self
    where
        W: Widget + 'static,
    {
        self.push_panel(id, options, child)
            .expect("dock panel registration must be valid and unique");
        self
    }

    pub fn push_panel<W>(
        &mut self,
        id: DockPanelId,
        options: impl Into<DockPanelOptions>,
        child: W,
    ) -> Result<(), DockLayoutError>
    where
        W: Widget + 'static,
    {
        validate_panel_id(id)?;
        if !self.state.registry().registered.insert(id) {
            return Err(DockLayoutError::new(format!(
                "dock panel {id} is already registered"
            )));
        }
        self.panels.push(DockPanelEntry {
            id,
            options: options.into(),
            child: WidgetPod::new(child),
        });
        Ok(())
    }

    /// Applies registry changes queued through [`DockWorkspaceState`].
    fn apply_registry_ops(&mut self) {
        let ops = std::mem::take(&mut self.state.registry().ops);
        for op in ops {
            match op {
                DockRegistryOp::Add { id, options } => {
                    let child = match &mut self.panel_builder {
                        Some(build) => build(id),
                        None => Box::new(crate::SizedBox::new()),
                    };
                    self.panels.retain(|panel| panel.id != id);
                    self.panels.push(DockPanelEntry {
                        id,
                        options,
                        child: WidgetPod::new_boxed(child),
                    });
                }
                DockRegistryOp::Remove(id) => {
                    self.panels.retain(|panel| panel.id != id);
                    if self.hovered_close == Some(id) {
                        self.hovered_close = None;
                    }
                }
                DockRegistryOp::Update { id, update } => {
                    if let Some(panel) = self.panel_mut(id) {
                        update(&mut panel.options);
                    }
                }
            }
        }
    }

    pub fn shared_state(&self) -> DockWorkspaceState {
        self.state.clone()
    }

    pub fn panel_widget_id(&self, panel: DockPanelId) -> Option<WidgetId> {
        self.panel(panel).map(|entry| entry.child.id())
    }

    pub fn validate_registered_panels(&self) -> Result<(), DockLayoutError> {
        self.validate_snapshot_panels(&self.state.snapshot())
    }

    /// Validates that every panel referenced by `snapshot` has a widget in
    /// this workspace's stable registry.
    ///
    /// This complements the structural graph validation performed by
    /// [`DockWorkspaceState::apply_snapshot`].
    pub fn validate_snapshot_panels(
        &self,
        snapshot: &DockWorkspaceSnapshot,
    ) -> Result<(), DockLayoutError> {
        let registered = self.state.registry().registered.clone();
        let mut referenced = HashSet::new();
        collect_snapshot_panels(snapshot, &mut referenced);
        if let Some(missing) = referenced
            .into_iter()
            .find(|panel| !registered.contains(panel))
        {
            return Err(DockLayoutError::new(format!(
                "dock snapshot references unregistered panel {missing}"
            )));
        }
        Ok(())
    }

    /// Applies a restored snapshot only after validating both its graph and
    /// all panel IDs against this workspace's registry.
    pub fn apply_snapshot(&self, snapshot: DockWorkspaceSnapshot) -> Result<bool, DockLayoutError> {
        validate_snapshot(&snapshot)?;
        self.validate_snapshot_panels(&snapshot)?;
        self.state.apply_snapshot(snapshot)
    }

    fn panel(&self, id: DockPanelId) -> Option<&DockPanelEntry> {
        self.panels.iter().find(|panel| panel.id == id)
    }

    fn panel_mut(&mut self, id: DockPanelId) -> Option<&mut DockPanelEntry> {
        self.panels.iter_mut().find(|panel| panel.id == id)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| *self.theme)
    }

    fn options_map(&self) -> HashMap<DockPanelId, DockPanelOptions> {
        self.panels
            .iter()
            .map(|panel| (panel.id, panel.options.clone()))
            .collect()
    }

    fn header_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height.max(22.0)
    }

    fn rebuild_layout(&self, snapshot: &DockWorkspaceSnapshot, bounds: Rect) -> DockLayoutCache {
        build_workspace_layout(
            snapshot,
            bounds,
            self.header_height(),
            DEFAULT_SPLITTER_THICKNESS,
            &self.options_map(),
        )
    }

    fn close_rect(&self, panel: DockPanelId) -> Option<Rect> {
        self.layout
            .groups
            .iter()
            .flat_map(|group| group.tabs.iter())
            .find(|tab| tab.panel == panel)
            .and_then(|tab| tab.close)
    }

    fn close_at(&self, position: Point) -> Option<DockPanelId> {
        for group in self.layout.floating_groups_front_to_back() {
            if group.bounds.contains(position) {
                return group.close_at(position);
            }
        }
        self.layout
            .groups
            .iter()
            .filter(|group| !group.is_floating())
            .find_map(|group| group.close_at(position))
    }

    fn set_hovered_close(&mut self, ctx: &mut EventCtx, hovered: Option<DockPanelId>) {
        if self.hovered_close != hovered {
            self.hovered_close = hovered;
            ctx.request_paint();
        }
    }

    fn request_close(&mut self, ctx: &mut EventCtx, panel: DockPanelId) {
        let action = self
            .on_close_request
            .as_mut()
            .map_or(DockCloseAction::Hide, |handler| handler(panel));
        let changed = match action {
            DockCloseAction::Hide => self.state.hide(panel).unwrap_or(false),
            DockCloseAction::Remove => self.state.remove_panel(panel).unwrap_or(false),
            DockCloseAction::Keep => false,
        };
        if changed {
            self.hovered_close = None;
            ctx.request_ordering();
            ctx.request_hit_test();
            ctx.request_measure();
            ctx.request_semantics();
        }
        ctx.request_paint();
    }

    fn begin_pointer_gesture(
        &mut self,
        ctx: &mut EventCtx,
        pointer_id: u64,
        position: Point,
    ) -> bool {
        for group in self.layout.floating_groups_front_to_back() {
            let DockGroupLocation::Floating(group_id) = group.location else {
                continue;
            };
            if !group.bounds.contains(position) {
                continue;
            }

            if self
                .state
                .bring_floating_to_front(group_id)
                .unwrap_or(false)
            {
                ctx.request_ordering();
                ctx.request_hit_test();
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
            }
            if floating_resize_rect(group.bounds).contains(position) {
                self.gesture = Some(DockWorkspaceGesture::FloatingResize {
                    pointer_id,
                    group_id,
                    pointer_origin: position,
                    initial_bounds: group.bounds,
                });
                ctx.request_pointer_capture(pointer_id);
                return true;
            }

            if let Some(panel) = group.close_at(position) {
                self.gesture = Some(DockWorkspaceGesture::ClosePress { pointer_id, panel });
                ctx.request_pointer_capture(pointer_id);
                return true;
            }

            if let Some(panel) = group.tab_at(position) {
                let _ = self.state.activate(panel);
                self.focused_group = Some(group.location.clone());
                self.gesture = Some(DockWorkspaceGesture::TabPress {
                    pointer_id,
                    panel,
                    start_position: position,
                    floating_size: group.bounds.size,
                });
                ctx.request_focus();
                ctx.request_pointer_capture(pointer_id);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                return true;
            }

            if group.header.contains(position) {
                self.gesture = Some(DockWorkspaceGesture::FloatingMove {
                    pointer_id,
                    group_id,
                    pointer_origin: position,
                    initial_bounds: group.bounds,
                });
                self.focused_group = Some(group.location.clone());
                ctx.request_focus();
                ctx.request_pointer_capture(pointer_id);
                return true;
            }

            // Body presses front the floating group but remain available to
            // the active panel child.
            ctx.request_paint();
            return false;
        }

        if let Some(splitter) = self
            .layout
            .splitters
            .iter()
            .find(|splitter| splitter.bounds.contains(position))
            .cloned()
        {
            self.gesture = Some(DockWorkspaceGesture::Split {
                pointer_id,
                path: splitter.path,
                axis: splitter.axis,
                split_bounds: splitter.split_bounds,
            });
            ctx.request_pointer_capture(pointer_id);
            return true;
        }

        if let Some(panel) = self
            .layout
            .groups
            .iter()
            .filter(|group| !group.is_floating())
            .find_map(|group| group.close_at(position))
        {
            self.gesture = Some(DockWorkspaceGesture::ClosePress { pointer_id, panel });
            ctx.request_pointer_capture(pointer_id);
            return true;
        }

        if let Some(group) = self
            .layout
            .groups
            .iter()
            .filter(|group| !group.is_floating())
            .find(|group| group.header.contains(position))
            && let Some(panel) = group.tab_at(position)
        {
            let location = group.location.clone();
            let floating_size = Size::new(320.0, 260.0);
            let _ = self.state.activate(panel);
            self.focused_group = Some(location);
            self.gesture = Some(DockWorkspaceGesture::TabPress {
                pointer_id,
                panel,
                start_position: position,
                floating_size,
            });
            ctx.request_focus();
            ctx.request_pointer_capture(pointer_id);
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
            return true;
        }

        false
    }

    fn drop_candidate_at(
        &self,
        position: Point,
        dragged_panel: DockPanelId,
    ) -> Option<DockDropCandidate> {
        for group in self.layout.floating_groups_front_to_back() {
            if !group.bounds.contains(position) {
                continue;
            }
            if let Some(insert) = group.tab_insert_at(position, dragged_panel) {
                return Some(DockDropCandidate::insert(group.bounds, insert));
            }
            let target = group
                .panels
                .iter()
                .copied()
                .find(|panel| *panel != dragged_panel)
                .unwrap_or(group.active);
            let zone_bounds = inset_rect_evenly(group.bounds, DROP_ZONE_INSET);
            return Some(DockDropCandidate {
                target: Some(target),
                zone: DockZone::Center,
                host_bounds: group.bounds,
                zone_bounds,
                center_only: true,
                insert: None,
            });
        }

        for group in self
            .layout
            .groups
            .iter()
            .filter(|group| !group.is_floating())
        {
            if !group.bounds.contains(position) {
                continue;
            }
            if let Some(insert) = group.tab_insert_at(position, dragged_panel) {
                return Some(DockDropCandidate::insert(group.bounds, insert));
            }
            let target = group
                .panels
                .iter()
                .copied()
                .find(|panel| *panel != dragged_panel)
                .unwrap_or(group.active);
            let zone = drop_zone_at(group.bounds, position);
            return Some(DockDropCandidate {
                target: Some(target),
                zone,
                host_bounds: group.bounds,
                zone_bounds: drop_zone_rect(group.bounds, zone),
                center_only: false,
                insert: None,
            });
        }

        if self.state.snapshot().root.is_empty()
            && !self.layout.groups.iter().any(|group| !group.is_floating())
        {
            let bounds = self.layout.bounds;
            if bounds.contains(position) {
                return Some(DockDropCandidate {
                    target: None,
                    zone: DockZone::Center,
                    host_bounds: bounds,
                    zone_bounds: inset_rect_evenly(bounds, DROP_ZONE_INSET),
                    center_only: true,
                    insert: None,
                });
            }
        }
        None
    }

    fn update_pointer_gesture(&mut self, ctx: &mut EventCtx, position: Point) {
        let host_bounds = ctx.bounds();
        if let Some(DockWorkspaceGesture::TabPress {
            pointer_id,
            panel,
            start_position,
            floating_size,
        }) = &self.gesture
        {
            let distance = position - *start_position;
            if (distance.x * distance.x) + (distance.y * distance.y)
                >= TAB_DRAG_THRESHOLD * TAB_DRAG_THRESHOLD
            {
                let pointer_id = *pointer_id;
                let panel = *panel;
                let floating_size = *floating_size;
                let candidate = self.drop_candidate_at(position, panel);
                self.gesture = Some(DockWorkspaceGesture::TabDrag {
                    pointer_id,
                    panel,
                    position,
                    floating_size,
                    candidate,
                });
            }
        }

        let dragged_panel = match &self.gesture {
            Some(DockWorkspaceGesture::TabDrag { panel, .. }) => Some(*panel),
            _ => None,
        };
        let next_candidate =
            dragged_panel.and_then(|panel| self.drop_candidate_at(position, panel));
        if let Some(DockWorkspaceGesture::TabDrag {
            position: current,
            candidate,
            ..
        }) = &mut self.gesture
        {
            *current = position;
            *candidate = next_candidate;
            ctx.request_paint();
            return;
        }

        let Some(gesture) = &self.gesture else {
            return;
        };
        match gesture {
            DockWorkspaceGesture::ClosePress { .. } => return,
            DockWorkspaceGesture::TabPress { .. } | DockWorkspaceGesture::TabDrag { .. } => {}
            DockWorkspaceGesture::Split {
                path,
                axis,
                split_bounds,
                ..
            } => {
                let total = axis_extent(*axis, split_bounds.size);
                let available = (total - DEFAULT_SPLITTER_THICKNESS.min(total)).max(1.0);
                let offset = match axis {
                    Axis::Horizontal => position.x - split_bounds.x(),
                    Axis::Vertical => position.y - split_bounds.y(),
                };
                let snapshot = self.state.snapshot();
                let (first_minimum, second_minimum) = match node_at_path(&snapshot.root, path) {
                    Some(DockNode::Split { first, second, .. }) => split_minimums(
                        first,
                        second,
                        *axis,
                        available,
                        self.header_height(),
                        DEFAULT_SPLITTER_THICKNESS,
                        &self.options_map(),
                    ),
                    _ => (0.0, 0.0),
                };
                let min_fraction =
                    (first_minimum / available).clamp(MIN_SPLIT_FRACTION, MAX_SPLIT_FRACTION);
                let max_fraction =
                    (1.0 - second_minimum / available).clamp(min_fraction, MAX_SPLIT_FRACTION);
                let fraction = (offset / available).clamp(min_fraction, max_fraction);
                let _ = self.state.set_split_fraction(path, fraction);
            }
            DockWorkspaceGesture::FloatingMove {
                group_id,
                pointer_origin,
                initial_bounds,
                ..
            } => {
                let delta = position - *pointer_origin;
                let proposed = initial_bounds.translate(delta);
                let _ = self
                    .state
                    .set_floating_bounds(*group_id, clamp_floating_bounds(host_bounds, proposed));
            }
            DockWorkspaceGesture::FloatingResize {
                group_id,
                pointer_origin,
                initial_bounds,
                ..
            } => {
                let delta = position - *pointer_origin;
                let proposed = Rect::new(
                    initial_bounds.x(),
                    initial_bounds.y(),
                    initial_bounds.width() + delta.x,
                    initial_bounds.height() + delta.y,
                );
                let _ = self
                    .state
                    .set_floating_bounds(*group_id, clamp_floating_bounds(host_bounds, proposed));
            }
        }
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn finish_pointer_gesture(&mut self, ctx: &mut EventCtx, release: Point, cancelled: bool) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        if cancelled {
            ctx.request_paint();
            return;
        }

        if let DockWorkspaceGesture::ClosePress { panel, .. } = gesture {
            if self
                .close_rect(panel)
                .is_some_and(|close| close.contains(release))
            {
                self.request_close(ctx, panel);
            }
            return;
        }

        if let DockWorkspaceGesture::TabDrag {
            panel,
            position,
            floating_size,
            candidate,
            ..
        } = gesture
        {
            if let Some(insert) = candidate.as_ref().and_then(|candidate| candidate.insert) {
                let _ = self.state.dock_tab(panel, insert.anchor, insert.after);
            } else if let Some(candidate) = candidate {
                match candidate.target {
                    Some(target) if target != panel => {
                        let _ = self.state.dock(panel, target, candidate.zone);
                    }
                    Some(_) => {
                        let _ = self.state.activate(panel);
                    }
                    None => {
                        let _ = self.state.dock_to_root(panel, DockZone::Center);
                    }
                }
            } else {
                let bounds = clamp_floating_bounds(
                    ctx.bounds(),
                    Rect::new(
                        position.x - floating_size.width * 0.5,
                        position.y - 18.0,
                        floating_size.width,
                        floating_size.height,
                    ),
                );
                let _ = self.state.float_panel(panel, bounds);
            }
            self.focused_group = panel_location(&self.state.snapshot(), panel);
            ctx.request_ordering();
            ctx.request_hit_test();
            ctx.request_measure();
            ctx.request_semantics();
        }
        ctx.request_paint();
    }

    fn activate_adjacent_tab(&mut self, delta: isize) -> bool {
        let Some(location) = self.focused_group.clone() else {
            return false;
        };
        let snapshot = self.state.snapshot();
        let Some((panels, active)) = group_members(&snapshot, &location) else {
            return false;
        };
        if panels.is_empty() {
            return false;
        }
        let current = panels
            .iter()
            .position(|panel| *panel == active)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(panels.len() as isize) as usize;
        self.state.activate(panels[next]).unwrap_or(false)
    }
}

impl Widget for DockWorkspace {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if self.begin_pointer_gesture(ctx, pointer.pointer_id, pointer.position) {
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self
                        .gesture
                        .as_ref()
                        .is_some_and(|gesture| gesture.pointer_id() == pointer.pointer_id) =>
            {
                self.update_pointer_gesture(ctx, pointer.position);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Up | PointerEventKind::Cancel
                ) && self
                    .gesture
                    .as_ref()
                    .is_some_and(|gesture| gesture.pointer_id() == pointer.pointer_id) =>
            {
                self.finish_pointer_gesture(
                    ctx,
                    pointer.position,
                    pointer.kind == PointerEventKind::Cancel,
                );
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Move | PointerEventKind::Enter
                ) =>
            {
                let hovered = self.close_at(pointer.position);
                self.set_hovered_close(ctx, hovered);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered_close(ctx, None);
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let handled = match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.activate_adjacent_tab(-1),
                    "ArrowRight" | "ArrowDown" => self.activate_adjacent_tab(1),
                    _ => false,
                };
                if handled {
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Semantics(semantics) => {
                if matches!(semantics.action, SemanticsActionRequest::Activate) {
                    let closing = self
                        .layout
                        .groups
                        .iter()
                        .flat_map(|group| group.tabs.iter())
                        .find(|tab| {
                            tab.close.is_some()
                                && dock_close_semantics_id(ctx.widget_id(), tab.panel)
                                    == semantics.target
                        })
                        .map(|tab| tab.panel);
                    if let Some(panel) = closing {
                        self.request_close(ctx, panel);
                        ctx.set_handled();
                        return;
                    }
                }
                if matches!(
                    semantics.action,
                    SemanticsActionRequest::Activate | SemanticsActionRequest::Focus
                ) {
                    let panel = self.layout.groups.iter().find_map(|group| {
                        group.tabs.iter().find_map(|tab| {
                            (dock_tab_semantics_id(ctx.widget_id(), tab.panel) == semantics.target)
                                .then_some((tab.panel, group.location.clone()))
                        })
                    });
                    if let Some((panel, location)) = panel {
                        let _ = self.state.activate(panel);
                        self.focused_group = Some(location);
                        ctx.request_focus();
                        ctx.request_measure();
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                        return;
                    }
                }

                let splitter_path = self.layout.splitters.iter().find_map(|splitter| {
                    (dock_splitter_semantics_id(ctx.widget_id(), &splitter.path)
                        == semantics.target)
                        .then(|| splitter.path.clone())
                });
                let Some(path) = splitter_path else {
                    return;
                };
                if matches!(semantics.action, SemanticsActionRequest::Focus) {
                    self.focused_group = None;
                    ctx.request_focus();
                    ctx.request_semantics();
                    ctx.set_handled();
                    return;
                }

                let snapshot = self.state.snapshot();
                let Some(DockNode::Split { fraction, .. }) = node_at_path(&snapshot.root, &path)
                else {
                    return;
                };
                let current = f64::from(*fraction);
                let next = match &semantics.action {
                    SemanticsActionRequest::Increment => {
                        Some(current + f64::from(SEMANTIC_SPLIT_STEP))
                    }
                    SemanticsActionRequest::Decrement => {
                        Some(current - f64::from(SEMANTIC_SPLIT_STEP))
                    }
                    SemanticsActionRequest::SetValue(SemanticsValue::Number(value)) => Some(*value),
                    SemanticsActionRequest::SetValue(SemanticsValue::Range { value, .. }) => {
                        Some(*value)
                    }
                    _ => None,
                };
                let Some(next) = next.filter(|value| value.is_finite()) else {
                    return;
                };
                let next = next as f32;
                if !next.is_finite() {
                    return;
                }
                let _ = self.state.set_split_fraction(&path, next);
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let _registry_revision = ctx.observe(&self.state.registry_revision);
        self.apply_registry_ops();
        let snapshot = ctx.observe(&self.state.snapshot);
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(1_024.0)
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            constraints.min.height.max(720.0)
        };
        let size = constraints.clamp(Size::new(width, height));
        self.layout = self.rebuild_layout(&snapshot, Rect::from_origin_size(Point::ZERO, size));

        let active = self.layout.visible_panel_ids().collect::<Vec<_>>();
        for panel_id in active {
            let bounds = self
                .layout
                .groups
                .iter()
                .find(|group| group.active == panel_id)
                .map(|group| group.content)
                .unwrap_or(Rect::ZERO);
            if let Some(panel) = self.panel_mut(panel_id) {
                panel.child.measure(ctx, Constraints::tight(bounds.size));
            }
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.layout = self.rebuild_layout(&self.state.snapshot(), bounds);
        let placements = self
            .layout
            .groups
            .iter()
            .map(|group| (group.active, group.content))
            .collect::<Vec<_>>();
        for (panel_id, panel_bounds) in placements {
            if let Some(panel) = self.panel_mut(panel_id) {
                panel.child.arrange(ctx, panel_bounds);
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        ctx.fill_bounds(theme.palette.surface);

        for group in self
            .layout
            .groups
            .iter()
            .filter(|group| !group.is_floating())
        {
            self.paint_group(ctx, group, &theme);
        }
        for splitter in &self.layout.splitters {
            ctx.fill_rect(splitter.bounds, theme.palette.border.with_alpha(0.88));
        }
        for group in self
            .layout
            .groups
            .iter()
            .filter(|group| group.is_floating())
        {
            self.paint_group(ctx, group, &theme);
        }
        if let Some(DockWorkspaceGesture::TabDrag {
            panel,
            position,
            candidate,
            ..
        }) = &self.gesture
        {
            match candidate {
                Some(DockDropCandidate {
                    insert: Some(insert),
                    ..
                }) => ctx.fill_rect(insert.indicator, theme.palette.accent),
                Some(candidate) => paint_drop_targets(ctx, candidate, &theme),
                None => {}
            }
            self.paint_drag_preview(ctx, *panel, *position, &theme);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut workspace = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        workspace.name = Some(self.name.clone());
        workspace.state.focused = ctx.is_focused();
        workspace.actions = vec![SemanticsAction::Focus];
        ctx.push(workspace);

        for group in &self.layout.groups {
            let group_id = dock_group_semantics_id(ctx.widget_id(), &group.location);
            let mut tabs = SemanticsNode::new(group_id, SemanticsRole::TabBar, group.header);
            tabs.parent = Some(ctx.widget_id());
            tabs.name = Some(if group.is_floating() {
                "Floating panel tabs".to_string()
            } else {
                "Docked panel tabs".to_string()
            });
            tabs.value = self
                .panel(group.active)
                .map(|panel| SemanticsValue::Text(panel.options.title.clone()));
            ctx.push(tabs);
            for tab in &group.tabs {
                let mut node = SemanticsNode::new(
                    dock_tab_semantics_id(ctx.widget_id(), tab.panel),
                    SemanticsRole::Button,
                    tab.bounds,
                );
                let options = self.panel(tab.panel).map(|panel| &panel.options);
                node.parent = Some(group_id);
                node.name = options.map(|options| options.title.clone());
                node.description = options
                    .filter(|options| options.dirty)
                    .map(|_| "Unsaved changes".to_string());
                node.state.selected = tab.panel == group.active;
                node.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
                ctx.push(node);

                if let Some(close) = tab.close {
                    let mut node = SemanticsNode::new(
                        dock_close_semantics_id(ctx.widget_id(), tab.panel),
                        SemanticsRole::Button,
                        close,
                    );
                    node.parent = Some(group_id);
                    node.name = Some(match options {
                        Some(options) => format!("Close {}", options.title),
                        None => "Close panel".to_string(),
                    });
                    node.actions = vec![SemanticsAction::Activate];
                    ctx.push(node);
                }
            }
            if let Some(panel) = self.panel(group.active) {
                panel.child.semantics(ctx);
            }
        }

        for splitter in &self.layout.splitters {
            let mut node = SemanticsNode::new(
                dock_splitter_semantics_id(ctx.widget_id(), &splitter.path),
                SemanticsRole::Splitter,
                splitter.bounds,
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some("Dock divider".to_string());
            node.value = Some(SemanticsValue::Range {
                value: f64::from(splitter.fraction),
                min: f64::from(MIN_SPLIT_FRACTION),
                max: f64::from(MAX_SPLIT_FRACTION),
            });
            node.actions = vec![
                SemanticsAction::Focus,
                SemanticsAction::Increment,
                SemanticsAction::Decrement,
                SemanticsAction::SetValue,
            ];
            ctx.push(node);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        Some(StackHostOptions {
            order_policy: StackOrderPolicy::FocusFronted,
        })
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for panel_id in self.layout.visible_panel_ids() {
            if let Some(panel) = self.panel(panel_id) {
                visitor.visit(&panel.child);
            }
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        let visible = self.layout.visible_panel_ids().collect::<Vec<_>>();
        for panel_id in visible {
            if let Some(panel) = self.panel_mut(panel_id) {
                visitor.visit(&mut panel.child);
            }
        }
    }
}

impl DockWorkspace {
    fn paint_group(&self, ctx: &mut PaintCtx, group: &DockGroupLayout, theme: &DefaultTheme) {
        let palette = theme.palette;
        ctx.fill_rect(group.bounds, palette.surface_raised);
        ctx.push_clip_rect(group.content);
        if let Some(panel) = self.panel(group.active) {
            panel.child.paint(ctx);
        }
        ctx.pop_clip();

        ctx.fill_rect(group.header, palette.control);
        for tab in &group.tabs {
            let selected = tab.panel == group.active;
            if selected {
                ctx.fill_rect(tab.bounds, palette.control_active);
                ctx.fill_rect(
                    Rect::new(
                        tab.bounds.x(),
                        tab.bounds.max_y() - 2.0,
                        tab.bounds.width(),
                        2.0,
                    ),
                    palette.accent,
                );
            }
            let options = self.panel(tab.panel).map(|panel| &panel.options);
            let label = options
                .map(|options| options.title.as_str())
                .unwrap_or("Missing panel");
            let ink = if selected {
                palette.text
            } else {
                palette.text_muted
            };
            if let (Some(icon), Some(glyph)) = (tab.icon, options.and_then(|options| options.icon))
            {
                draw_icon_glyph(ctx, glyph, icon, ink);
            }
            let style = theme.text_style(ink);
            ctx.push_clip_rect(tab.label);
            paint_text_line(ctx, tab.label, label, &style, TextAlign::Start);
            ctx.pop_clip();
            if let Some(dirty) = tab.dirty {
                ctx.fill_rrect(dirty, [dirty.width() * 0.5; 4], ink);
            }
            if let Some(close) = tab.close {
                if self.hovered_close == Some(tab.panel) {
                    ctx.fill_rrect(close, [3.0; 4], palette.control_hover);
                }
                draw_icon_glyph(ctx, IconGlyph::Close, close.inflate(-3.0, -3.0), ink);
            }
        }

        if group.is_floating() {
            stroke_border(ctx, group.bounds, 0.0, 1.0, palette.border);
            let handle = floating_resize_rect(group.bounds);
            ctx.fill_rect(handle, palette.border.with_alpha(0.72));
            ctx.fill_rect(
                Rect::new(
                    group.header.max_x() - FLOATING_GRIP_WIDTH,
                    group.header.y(),
                    FLOATING_GRIP_WIDTH,
                    group.header.height(),
                ),
                palette.control_hover.with_alpha(0.34),
            );
        }
    }

    fn paint_drag_preview(
        &self,
        ctx: &mut PaintCtx,
        panel: DockPanelId,
        position: Point,
        theme: &DefaultTheme,
    ) {
        let width = 168.0;
        let height = theme.metrics.tab_height.max(26.0);
        let max_x = (ctx.bounds().max_x() - width).max(ctx.bounds().x());
        let max_y = (ctx.bounds().max_y() - height).max(ctx.bounds().y());
        let bounds = Rect::new(
            (position.x + 12.0).clamp(ctx.bounds().x(), max_x),
            (position.y + 12.0).clamp(ctx.bounds().y(), max_y),
            width,
            height,
        );
        let bounds = snap_to_pixels(ctx, bounds);
        ctx.fill_rect(bounds, theme.palette.surface_raised.with_alpha(0.96));
        stroke_border(ctx, bounds, 0.0, 1.5, theme.palette.accent);
        let text = self
            .panel(panel)
            .map(|entry| entry.options.title.as_str())
            .unwrap_or("Panel");
        let style = theme.text_style(theme.palette.text);
        let text_bounds = inset_rect_evenly(bounds, 8.0);
        ctx.push_clip_rect(text_bounds);
        paint_text_line(ctx, text_bounds, text, &style, TextAlign::Start);
        ctx.pop_clip();
    }
}

fn drop_zone_at(bounds: Rect, position: Point) -> DockZone {
    let edge_x = bounds.width() * DROP_EDGE_FRACTION;
    let edge_y = bounds.height() * DROP_EDGE_FRACTION;
    let horizontal_distance = (position.x - bounds.x()).min(bounds.max_x() - position.x);
    let vertical_distance = (position.y - bounds.y()).min(bounds.max_y() - position.y);
    let center_x = bounds.x() + bounds.width() * 0.5;
    let center_y = bounds.y() + bounds.height() * 0.5;

    if horizontal_distance < edge_x && horizontal_distance <= vertical_distance {
        if position.x < center_x {
            DockZone::Left
        } else {
            DockZone::Right
        }
    } else if vertical_distance < edge_y {
        if position.y < center_y {
            DockZone::Top
        } else {
            DockZone::Bottom
        }
    } else {
        DockZone::Center
    }
}

fn drop_zone_rect(bounds: Rect, zone: DockZone) -> Rect {
    let bounds = inset_rect_evenly(bounds, DROP_ZONE_INSET);
    let edge_width = bounds.width() * DROP_EDGE_FRACTION;
    let edge_height = bounds.height() * DROP_EDGE_FRACTION;

    match zone {
        DockZone::Left => Rect::new(bounds.x(), bounds.y(), edge_width, bounds.height()),
        DockZone::Right => Rect::new(
            bounds.max_x() - edge_width,
            bounds.y(),
            edge_width,
            bounds.height(),
        ),
        DockZone::Top => Rect::new(bounds.x(), bounds.y(), bounds.width(), edge_height),
        DockZone::Bottom => Rect::new(
            bounds.x(),
            bounds.max_y() - edge_height,
            bounds.width(),
            edge_height,
        ),
        DockZone::Center => Rect::new(
            bounds.x() + edge_width,
            bounds.y() + edge_height,
            (bounds.width() - edge_width * 2.0).max(0.0),
            (bounds.height() - edge_height * 2.0).max(0.0),
        ),
    }
}

fn paint_drop_targets(ctx: &mut PaintCtx, candidate: &DockDropCandidate, theme: &DefaultTheme) {
    const ALL_ZONES: [DockZone; 5] = [
        DockZone::Left,
        DockZone::Right,
        DockZone::Top,
        DockZone::Bottom,
        DockZone::Center,
    ];
    const CENTER_ZONE: [DockZone; 1] = [DockZone::Center];
    let zones = if candidate.center_only {
        &CENTER_ZONE[..]
    } else {
        &ALL_ZONES[..]
    };

    for zone in zones {
        let bounds = if *zone == candidate.zone {
            candidate.zone_bounds
        } else {
            drop_zone_rect(candidate.host_bounds, *zone)
        };
        let selected = *zone == candidate.zone;
        ctx.fill_rect(
            bounds,
            if selected {
                theme.palette.accent.with_alpha(0.30)
            } else {
                theme.palette.control_active.with_alpha(0.20)
            },
        );
        stroke_border(
            ctx,
            bounds,
            0.0,
            if selected { 2.0 } else { 1.0 },
            if selected {
                theme.palette.accent
            } else {
                theme.palette.border_focus.with_alpha(0.72)
            },
        );
    }
}

fn collect_snapshot_panels(snapshot: &DockWorkspaceSnapshot, output: &mut HashSet<DockPanelId>) {
    collect_node_panels(&snapshot.root, output);
    for group in &snapshot.floating {
        output.extend(group.panels.iter().copied());
    }
    output.extend(snapshot.hidden.iter().copied());
}

fn collect_node_panels(node: &DockNode, output: &mut HashSet<DockPanelId>) {
    match node {
        DockNode::Empty => {}
        DockNode::Tabs { panels, .. } => output.extend(panels.iter().copied()),
        DockNode::Split { first, second, .. } => {
            collect_node_panels(first, output);
            collect_node_panels(second, output);
        }
    }
}

fn build_workspace_layout(
    snapshot: &DockWorkspaceSnapshot,
    bounds: Rect,
    header_height: f32,
    splitter_thickness: f32,
    panels: &HashMap<DockPanelId, DockPanelOptions>,
) -> DockLayoutCache {
    let mut layout = DockLayoutCache {
        bounds,
        ..DockLayoutCache::default()
    };
    let mut path = Vec::new();
    layout_dock_node(
        &snapshot.root,
        bounds,
        header_height,
        splitter_thickness,
        panels,
        &mut path,
        &mut layout,
    );

    for floating in &snapshot.floating {
        let resolved = clamp_floating_bounds(bounds, floating.bounds);
        layout.groups.push(make_group_layout(
            DockGroupLocation::Floating(floating.id),
            resolved,
            header_height,
            &floating.panels,
            floating.active,
            panels,
            true,
        ));
    }
    layout
}

#[allow(clippy::too_many_arguments)]
fn layout_dock_node(
    node: &DockNode,
    bounds: Rect,
    header_height: f32,
    splitter_thickness: f32,
    panels: &HashMap<DockPanelId, DockPanelOptions>,
    path: &mut Vec<DockBranch>,
    layout: &mut DockLayoutCache,
) {
    match node {
        DockNode::Empty => {}
        DockNode::Tabs {
            panels: members,
            active,
        } => layout.groups.push(make_group_layout(
            DockGroupLocation::Docked(path.clone()),
            bounds,
            header_height,
            members,
            *active,
            panels,
            false,
        )),
        DockNode::Split {
            axis,
            fraction,
            first,
            second,
        } => {
            let total = axis_extent(*axis, bounds.size);
            let divider = splitter_thickness.min(total).max(0.0);
            let available = (total - divider).max(0.0);
            let (first_minimum, second_minimum) = split_minimums(
                first,
                second,
                *axis,
                available,
                header_height,
                splitter_thickness,
                panels,
            );
            let first_extent = (available * *fraction).clamp(
                first_minimum,
                (available - second_minimum).max(first_minimum),
            );
            let second_extent = (available - first_extent).max(0.0);
            let first_bounds = axis_rect(*axis, bounds, 0.0, first_extent);
            let splitter_bounds = axis_rect(*axis, bounds, first_extent, divider);
            let second_bounds = axis_rect(*axis, bounds, first_extent + divider, second_extent);
            layout.splitters.push(DockSplitterLayout {
                path: path.clone(),
                axis: *axis,
                fraction: *fraction,
                bounds: splitter_bounds,
                split_bounds: bounds,
            });
            path.push(DockBranch::First);
            layout_dock_node(
                first,
                first_bounds,
                header_height,
                splitter_thickness,
                panels,
                path,
                layout,
            );
            path.pop();
            path.push(DockBranch::Second);
            layout_dock_node(
                second,
                second_bounds,
                header_height,
                splitter_thickness,
                panels,
                path,
                layout,
            );
            path.pop();
        }
    }
}

fn make_group_layout(
    location: DockGroupLocation,
    bounds: Rect,
    header_height: f32,
    members: &[DockPanelId],
    active: DockPanelId,
    panels: &HashMap<DockPanelId, DockPanelOptions>,
    floating: bool,
) -> DockGroupLayout {
    let header_height = header_height.min(bounds.height()).max(0.0);
    let header = Rect::new(bounds.x(), bounds.y(), bounds.width(), header_height);
    let content = Rect::new(
        bounds.x(),
        bounds.y() + header_height,
        bounds.width(),
        (bounds.height() - header_height).max(0.0),
    );
    let tab_area = if floating {
        Rect::new(
            header.x(),
            header.y(),
            (header.width() - FLOATING_GRIP_WIDTH).max(0.0),
            header.height(),
        )
    } else {
        header
    };
    let tabs = tab_rects(tab_area, members, panels);
    DockGroupLayout {
        location,
        bounds,
        header,
        content,
        panels: members.to_vec(),
        active,
        tabs,
    }
}

/// The smallest extent a dock node needs along `axis` so that every panel
/// in it keeps its minimum content size.
fn node_min_extent(
    node: &DockNode,
    axis: Axis,
    header_height: f32,
    splitter_thickness: f32,
    panels: &HashMap<DockPanelId, DockPanelOptions>,
) -> f32 {
    match node {
        DockNode::Empty => 0.0,
        DockNode::Tabs {
            panels: members, ..
        } => {
            let content = members
                .iter()
                .filter_map(|panel| panels.get(panel))
                .map(|options| axis_extent(axis, options.min_size))
                .filter(|extent| extent.is_finite())
                .fold(0.0, f32::max);
            let chrome = match axis {
                Axis::Horizontal => 0.0,
                Axis::Vertical => header_height,
            };
            (content + chrome).max(MIN_DOCK_PANE_EXTENT)
        }
        DockNode::Split {
            axis: split_axis,
            first,
            second,
            ..
        } => {
            let first = node_min_extent(first, axis, header_height, splitter_thickness, panels);
            let second = node_min_extent(second, axis, header_height, splitter_thickness, panels);
            if *split_axis == axis {
                first + second + splitter_thickness
            } else {
                first.max(second)
            }
        }
    }
}

/// Minimum extents for the two branches of a split with `available` room.
///
/// When both minimums cannot be met they shrink in proportion, leaving the
/// divider a little travel so the user can still trade space between them.
fn split_minimums(
    first: &DockNode,
    second: &DockNode,
    axis: Axis,
    available: f32,
    header_height: f32,
    splitter_thickness: f32,
    panels: &HashMap<DockPanelId, DockPanelOptions>,
) -> (f32, f32) {
    let first = node_min_extent(first, axis, header_height, splitter_thickness, panels);
    let second = node_min_extent(second, axis, header_height, splitter_thickness, panels);
    let budget = (available * 0.9).max(0.0);
    if first + second > budget {
        let scale = budget / (first + second);
        (first * scale, second * scale)
    } else {
        (first, second)
    }
}

fn tab_rects(
    bounds: Rect,
    members: &[DockPanelId],
    panels: &HashMap<DockPanelId, DockPanelOptions>,
) -> Vec<DockTabLayout> {
    if members.is_empty() || bounds.width() <= 0.0 {
        return Vec::new();
    }
    let natural = members
        .iter()
        .map(|panel| {
            let options = panels.get(panel);
            let characters = options.map_or(12, |options| options.title.chars().count());
            let mut width =
                (characters as f32 * 7.0 + 28.0).clamp(TAB_MIN_WIDTH, TAB_MAX_TITLE_WIDTH);
            if options.is_some_and(|options| options.icon.is_some()) {
                width += TAB_ICON_EXTENT + TAB_PART_GAP;
            }
            if options.is_some_and(|options| options.dirty) {
                width += TAB_DIRTY_EXTENT + TAB_PART_GAP;
            }
            if options.is_some_and(|options| options.closable) {
                width += TAB_CLOSE_EXTENT + TAB_PART_GAP;
            }
            width
        })
        .collect::<Vec<_>>();
    let total = natural.iter().sum::<f32>();
    let scale = if total > bounds.width() {
        bounds.width() / total
    } else {
        1.0
    };
    let mut x = bounds.x();
    natural
        .into_iter()
        .zip(members.iter().copied())
        .map(|(width, panel)| {
            let width = (width * scale).max(0.0).min(bounds.max_x() - x);
            let tab_bounds = Rect::new(x, bounds.y(), width, bounds.height());
            x += width;
            tab_layout(panel, tab_bounds, panels.get(&panel))
        })
        .collect()
}

/// Places a tab's icon, title, unsaved marker, and close button. Parts that
/// do not fit a squeezed tab are dropped, the title last.
fn tab_layout(
    panel: DockPanelId,
    bounds: Rect,
    options: Option<&DockPanelOptions>,
) -> DockTabLayout {
    let center_y = bounds.y() + bounds.height() * 0.5;
    let mut left = bounds.x() + TAB_TEXT_INSET;
    let mut right = bounds.max_x() - TAB_TEXT_INSET;

    let close = options
        .filter(|options| options.closable && bounds.width() >= TAB_MIN_CLOSE_WIDTH)
        .map(|_| {
            let extent = TAB_CLOSE_EXTENT.min(bounds.height());
            let rect = Rect::new(
                bounds.max_x() - TAB_TEXT_INSET * 0.5 - extent,
                center_y - extent * 0.5,
                extent,
                extent,
            );
            right = rect.x() - TAB_PART_GAP;
            rect
        });
    let icon = options
        .filter(|options| options.icon.is_some() && bounds.width() >= TAB_MIN_ICON_WIDTH)
        .map(|_| {
            let extent = TAB_ICON_EXTENT.min(bounds.height());
            let rect = Rect::new(left, center_y - extent * 0.5, extent, extent);
            left = rect.max_x() + TAB_PART_GAP;
            rect
        });
    let dirty = options
        .filter(|options| options.dirty && right - left >= TAB_DIRTY_EXTENT + TAB_PART_GAP)
        .map(|_| {
            let rect = Rect::new(
                right - TAB_DIRTY_EXTENT,
                center_y - TAB_DIRTY_EXTENT * 0.5,
                TAB_DIRTY_EXTENT,
                TAB_DIRTY_EXTENT,
            );
            right = rect.x() - TAB_PART_GAP;
            rect
        });
    let label = Rect::new(
        left,
        bounds.y() + 2.0,
        (right - left).max(0.0),
        (bounds.height() - 4.0).max(0.0),
    );
    DockTabLayout {
        panel,
        bounds,
        label,
        icon,
        dirty,
        close,
    }
}

fn axis_extent(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn axis_rect(axis: Axis, bounds: Rect, offset: f32, extent: f32) -> Rect {
    match axis {
        Axis::Horizontal => Rect::new(
            bounds.x() + offset,
            bounds.y(),
            extent.max(0.0),
            bounds.height(),
        ),
        Axis::Vertical => Rect::new(
            bounds.x(),
            bounds.y() + offset,
            bounds.width(),
            extent.max(0.0),
        ),
    }
}

fn floating_resize_rect(bounds: Rect) -> Rect {
    Rect::new(
        bounds.max_x() - FLOATING_RESIZE_HANDLE,
        bounds.max_y() - FLOATING_RESIZE_HANDLE,
        FLOATING_RESIZE_HANDLE,
        FLOATING_RESIZE_HANDLE,
    )
}

fn clamp_floating_bounds(host: Rect, proposed: Rect) -> Rect {
    let host_width = host.width().max(0.0);
    let host_height = host.height().max(0.0);
    let width = proposed
        .width()
        .max(MIN_FLOATING_WIDTH.min(host_width))
        .min(host_width);
    let height = proposed
        .height()
        .max(MIN_FLOATING_HEIGHT.min(host_height))
        .min(host_height);
    let min_x = host.x();
    let min_y = host.y();
    let max_x = (host.max_x() - width).max(min_x);
    let max_y = (host.max_y() - height).max(min_y);
    Rect::new(
        proposed.x().clamp(min_x, max_x),
        proposed.y().clamp(min_y, max_y),
        width,
        height,
    )
}

fn group_members(
    snapshot: &DockWorkspaceSnapshot,
    location: &DockGroupLocation,
) -> Option<(Vec<DockPanelId>, DockPanelId)> {
    match location {
        DockGroupLocation::Docked(path) => {
            let DockNode::Tabs { panels, active } = node_at_path(&snapshot.root, path)? else {
                return None;
            };
            Some((panels.clone(), *active))
        }
        DockGroupLocation::Floating(id) => snapshot
            .floating
            .iter()
            .find(|group| group.id == *id)
            .map(|group| (group.panels.clone(), group.active)),
    }
}

fn panel_location(
    snapshot: &DockWorkspaceSnapshot,
    panel: DockPanelId,
) -> Option<DockGroupLocation> {
    let mut path = Vec::new();
    if find_panel_path(&snapshot.root, panel, &mut path) {
        return Some(DockGroupLocation::Docked(path));
    }
    snapshot
        .floating
        .iter()
        .find(|group| group.panels.contains(&panel))
        .map(|group| DockGroupLocation::Floating(group.id))
}

fn find_panel_path(node: &DockNode, panel: DockPanelId, path: &mut Vec<DockBranch>) -> bool {
    match node {
        DockNode::Empty => false,
        DockNode::Tabs { panels, .. } => panels.contains(&panel),
        DockNode::Split { first, second, .. } => {
            path.push(DockBranch::First);
            if find_panel_path(first, panel, path) {
                return true;
            }
            path.pop();
            path.push(DockBranch::Second);
            if find_panel_path(second, panel, path) {
                return true;
            }
            path.pop();
            false
        }
    }
}

fn dock_tab_semantics_id(parent: WidgetId, panel: DockPanelId) -> WidgetId {
    const TAG: u64 = 5_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(421)
            .wrapping_add(panel.get().wrapping_mul(17))
            & LOW_MASK),
    )
}

fn dock_close_semantics_id(parent: WidgetId, panel: DockPanelId) -> WidgetId {
    const TAG: u64 = 9_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(443)
            .wrapping_add(panel.get().wrapping_mul(19))
            & LOW_MASK),
    )
}

fn dock_group_semantics_id(parent: WidgetId, location: &DockGroupLocation) -> WidgetId {
    const TAG: u64 = 6_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;
    let location_hash = match location {
        DockGroupLocation::Docked(path) => path.iter().fold(1_u64, |hash, branch| {
            hash.wrapping_mul(31).wrapping_add(match branch {
                DockBranch::First => 1,
                DockBranch::Second => 2,
            })
        }),
        DockGroupLocation::Floating(id) => id.wrapping_mul(97).wrapping_add(3),
    };
    WidgetId::new(TAG | (parent.get().wrapping_mul(433).wrapping_add(location_hash) & LOW_MASK))
}

fn dock_splitter_semantics_id(parent: WidgetId, path: &[DockBranch]) -> WidgetId {
    const TAG: u64 = 7_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;
    let path_hash = path.iter().fold(1_u64, |hash, branch| {
        hash.wrapping_mul(37).wrapping_add(match branch {
            DockBranch::First => 1,
            DockBranch::Second => 2,
        })
    });
    WidgetId::new(TAG | (parent.get().wrapping_mul(439).wrapping_add(path_hash) & LOW_MASK))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Label, SizedBox};
    use sui_core::{
        Event, KeyboardEvent, Modifiers, PointerButtons, PointerEvent, PointerKind, Vector,
        WindowEvent,
    };
    use sui_runtime::{Application, Runtime, WindowBuilder};
    use sui_scene::SceneCommand;

    const SCENE: DockPanelId = DockPanelId::new(1);
    const VIEWPORT: DockPanelId = DockPanelId::new(2);
    const DETAILS: DockPanelId = DockPanelId::new(3);

    fn root_snapshot() -> DockWorkspaceSnapshot {
        DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.25,
            DockNode::tabs([SCENE], SCENE),
            DockNode::split(
                Axis::Horizontal,
                0.72,
                DockNode::tabs([VIEWPORT], VIEWPORT),
                DockNode::tabs([DETAILS], DETAILS),
            ),
        ))
    }

    fn panel(title: &str) -> SizedBox {
        SizedBox::new()
            .width(200.0)
            .height(160.0)
            .child(Label::new(title))
    }

    fn build_runtime(root: impl Widget + 'static) -> (Runtime, sui_core::WindowId) {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Docking").root(root))
            .build()
            .expect("runtime builds");
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn resize(runtime: &mut Runtime, window_id: sui_core::WindowId, width: f32, height: f32) {
        runtime
            .handle_event(
                window_id,
                Event::Window(WindowEvent::Resized(Size::new(width, height))),
            )
            .expect("window resizes");
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }
        Event::Pointer(PointerEvent {
            pointer_id: 1,
            kind,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: Some(PointerButton::Primary),
            buttons,
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        })
    }

    #[test]
    fn snapshot_rejects_duplicate_panels_and_invalid_splits() {
        let duplicate = DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.5,
            DockNode::tabs([SCENE], SCENE),
            DockNode::tabs([SCENE], SCENE),
        ));
        assert!(DockWorkspaceState::new(duplicate).is_err());

        let invalid = DockWorkspaceSnapshot::new(DockNode::Split {
            axis: Axis::Horizontal,
            fraction: f32::NAN,
            first: Box::new(DockNode::tabs([SCENE], SCENE)),
            second: Box::new(DockNode::tabs([VIEWPORT], VIEWPORT)),
        });
        assert!(DockWorkspaceState::new(invalid).is_err());

        let empty_branch = DockWorkspaceSnapshot::new(DockNode::Split {
            axis: Axis::Horizontal,
            fraction: 0.5,
            first: Box::new(DockNode::Empty),
            second: Box::new(DockNode::tabs([VIEWPORT], VIEWPORT)),
        });
        assert!(DockWorkspaceState::new(empty_branch).is_err());
    }

    #[test]
    fn state_round_trips_hide_float_and_dock_without_duplicates() {
        let state = DockWorkspaceState::new(root_snapshot()).expect("valid root");
        let floating = state
            .float_panel(DETAILS, Rect::new(80.0, 60.0, 300.0, 240.0))
            .expect("details floats");
        assert_eq!(floating, 1);
        assert_eq!(state.snapshot().floating[0].panels, vec![DETAILS]);

        assert!(state.hide(SCENE).expect("scene hides"));
        assert!(state.snapshot().hidden.contains(&SCENE));
        assert!(state.show(SCENE).expect("scene shows"));
        assert!(!state.snapshot().hidden.contains(&SCENE));

        assert!(
            state
                .dock(DETAILS, VIEWPORT, DockZone::Center)
                .expect("details docks")
        );
        let snapshot = state.snapshot();
        assert!(snapshot.floating.is_empty());
        validate_snapshot(&snapshot).expect("mutated snapshot stays canonical");
    }

    #[test]
    fn docking_and_floating_preserve_panel_widget_identity() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs(
            [VIEWPORT, DETAILS],
            VIEWPORT,
        )))
        .expect("valid tabs");
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(VIEWPORT, "Viewport", panel("Viewport content"))
            .with_panel(DETAILS, "Details", panel("Details content"));
        let details_widget = workspace
            .panel_widget_id(DETAILS)
            .expect("details widget registered");
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        runtime.render(window_id).expect("initial frame");

        state
            .float_panel(DETAILS, Rect::new(400.0, 80.0, 280.0, 320.0))
            .expect("details floats");
        runtime.render(window_id).expect("floating frame");
        assert!(
            runtime
                .widget_graph(window_id)
                .expect("floating graph")
                .nodes
                .iter()
                .any(|node| node.id == details_widget)
        );

        state
            .dock(DETAILS, VIEWPORT, DockZone::Center)
            .expect("details docks again");
        state.activate(DETAILS).expect("details activates");
        runtime.render(window_id).expect("redocked frame");
        assert!(
            runtime
                .widget_graph(window_id)
                .expect("redocked graph")
                .nodes
                .iter()
                .any(|node| node.id == details_widget),
            "the original retained details widget must be reused"
        );
    }

    #[test]
    fn dock_tab_titles_are_shaped_as_one_line() {
        const WORKFLOW: DockPanelId = DockPanelId::new(10);
        const MODELS: DockPanelId = DockPanelId::new(11);
        const WORKFLOW_TITLE: &str = "Image-to-3D Workflow";
        const MODELS_TITLE: &str = "Model Manager";

        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs(
            [WORKFLOW, MODELS],
            WORKFLOW,
        )))
        .expect("valid tabs");
        let workspace = DockWorkspace::new(state)
            .with_panel(WORKFLOW, WORKFLOW_TITLE, panel("Workflow body"))
            .with_panel(MODELS, MODELS_TITLE, panel("Models body"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 340.0, 240.0);
        let output = runtime.render(window_id).expect("dock tabs render");

        for title in [WORKFLOW_TITLE, MODELS_TITLE] {
            let mut line_counts = Vec::new();
            output.frame.scene.visit_commands(&mut |command| {
                let SceneCommand::DrawShapedText(run) = command else {
                    return;
                };
                if let Some(layout) = run
                    .resolve(output.frame.text_layout_registry.as_ref())
                    .filter(|layout| layout.text() == title)
                {
                    line_counts.push(layout.lines().len());
                }
            });
            assert_eq!(
                line_counts,
                vec![1],
                "dock tab title {title:?} must remain a single line"
            );
        }
    }

    #[test]
    fn splitter_pointer_drag_updates_persisted_fraction() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.5,
            DockNode::tabs([SCENE], SCENE),
            DockNode::tabs([VIEWPORT], VIEWPORT),
        )))
        .expect("valid split");
        let root = DockWorkspace::new(state.clone())
            .with_panel(SCENE, "Scene", panel("Scene"))
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"));
        let (mut runtime, window_id) = build_runtime(root);
        resize(&mut runtime, window_id, 800.0, 600.0);
        runtime.render(window_id).expect("initial frame");

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(400.0, 300.0), true),
            )
            .expect("split press");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, Point::new(600.0, 300.0), true),
            )
            .expect("split move");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, Point::new(600.0, 300.0), false),
            )
            .expect("split release");

        let DockNode::Split { fraction, .. } = state.snapshot().root else {
            panic!("root remains split");
        };
        assert!(fraction > 0.7, "dragged fraction was {fraction}");
    }

    #[test]
    fn pointer_tab_drag_floats_redocks_and_preserves_widget_identity() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs(
            [SCENE, VIEWPORT],
            SCENE,
        )))
        .expect("valid tabs");
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(SCENE, "Scene", panel("Scene"))
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"));
        let scene_widget = workspace
            .panel_widget_id(SCENE)
            .expect("scene widget registered");
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        runtime.render(window_id).expect("initial frame");

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(20.0, 10.0), true),
            )
            .expect("scene tab press");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, Point::new(840.0, 40.0), true),
            )
            .expect("scene tab drag outside");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, Point::new(840.0, 40.0), false),
            )
            .expect("scene tab float drop");

        let floated = state.snapshot();
        assert_eq!(floated.floating.len(), 1);
        assert_eq!(floated.floating[0].panels, vec![SCENE]);
        assert!(node_contains_panel(&floated.root, VIEWPORT));
        let floating_bounds = floated.floating[0].bounds;

        runtime.render(window_id).expect("floating frame");
        assert!(
            runtime
                .widget_graph(window_id)
                .expect("floating graph")
                .nodes
                .iter()
                .any(|node| node.id == scene_widget)
        );

        let floating_tab = Point::new(floating_bounds.x() + 20.0, floating_bounds.y() + 10.0);
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, floating_tab, true),
            )
            .expect("floating scene tab press");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, Point::new(8.0, 300.0), true),
            )
            .expect("floating scene tab drag to dock edge");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, Point::new(8.0, 300.0), false),
            )
            .expect("floating scene tab dock drop");

        let redocked = state.snapshot();
        assert!(redocked.floating.is_empty());
        assert!(matches!(
            redocked.root,
            DockNode::Split {
                axis: Axis::Horizontal,
                ..
            }
        ));
        validate_snapshot(&redocked).expect("pointer-mutated snapshot stays canonical");

        runtime.render(window_id).expect("redocked frame");
        assert!(
            runtime
                .widget_graph(window_id)
                .expect("redocked graph")
                .nodes
                .iter()
                .any(|node| node.id == scene_widget),
            "the same retained scene widget must survive float and dock gestures"
        );
    }

    #[test]
    fn tab_drag_updates_keyboard_navigation_to_the_destination_group() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.5,
            DockNode::tabs([SCENE, DETAILS], SCENE),
            DockNode::tabs([VIEWPORT], VIEWPORT),
        )))
        .expect("valid split tabs");
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(SCENE, "Scene", panel("Scene"))
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"))
            .with_panel(DETAILS, "Details", panel("Details"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        runtime.render(window_id).expect("initial frame");

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, Point::new(20.0, 10.0), true),
            )
            .expect("source tab press");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, Point::new(600.0, 300.0), true),
            )
            .expect("drag to destination center");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, Point::new(600.0, 300.0), false),
            )
            .expect("drop into destination tabs");
        runtime
            .handle_event(
                window_id,
                Event::Keyboard(KeyboardEvent::new("ArrowRight", KeyState::Pressed)),
            )
            .expect("cycle destination tabs");

        let DockNode::Split { first, second, .. } = state.snapshot().root else {
            panic!("workspace remains split");
        };
        let DockNode::Tabs {
            active: source_active,
            ..
        } = *first
        else {
            panic!("source remains a tab group");
        };
        let DockNode::Tabs {
            panels,
            active: destination_active,
        } = *second
        else {
            panic!("destination remains a tab group");
        };
        assert_eq!(source_active, DETAILS);
        assert_eq!(panels, vec![VIEWPORT, SCENE]);
        assert_eq!(
            destination_active, VIEWPORT,
            "ArrowRight must cycle the destination group after the drag"
        );
    }

    #[test]
    fn splitter_semantics_expose_and_mutate_the_persisted_fraction() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.5,
            DockNode::tabs([SCENE], SCENE),
            DockNode::tabs([VIEWPORT], VIEWPORT),
        )))
        .expect("valid split");
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(SCENE, "Scene", panel("Scene"))
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        let frame = runtime.render(window_id).expect("initial frame");
        let splitter = frame
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Splitter)
            .expect("dock splitter semantics");
        assert_eq!(
            splitter.value,
            Some(SemanticsValue::Range {
                value: 0.5,
                min: f64::from(MIN_SPLIT_FRACTION),
                max: f64::from(MAX_SPLIT_FRACTION),
            })
        );
        for action in [
            SemanticsAction::Increment,
            SemanticsAction::Decrement,
            SemanticsAction::SetValue,
        ] {
            assert!(splitter.actions.contains(&action));
        }
        let splitter_id = splitter.id;

        assert!(
            runtime
                .handle_semantics_action(window_id, splitter_id, SemanticsActionRequest::Increment,)
                .expect("increment semantics routes")
        );
        let DockNode::Split { fraction, .. } = state.snapshot().root else {
            panic!("root remains split");
        };
        assert!((fraction - 0.55).abs() < f32::EPSILON);

        runtime.render(window_id).expect("incremented frame");
        assert!(
            runtime
                .handle_semantics_action(
                    window_id,
                    splitter_id,
                    SemanticsActionRequest::SetValue(SemanticsValue::Number(0.8)),
                )
                .expect("set-value semantics routes")
        );
        let DockNode::Split { fraction, .. } = state.snapshot().root else {
            panic!("root remains split");
        };
        assert!((fraction - 0.8).abs() < f32::EPSILON);
    }

    fn click(runtime: &mut Runtime, window_id: sui_core::WindowId, position: Point) {
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, position, true),
            )
            .expect("press");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, position, false),
            )
            .expect("release");
    }

    fn named<'a>(semantics: &'a [SemanticsNode], name: &str) -> Option<&'a SemanticsNode> {
        semantics
            .iter()
            .find(|node| node.name.as_deref() == Some(name))
    }

    fn center(rect: Rect) -> Point {
        Point::new(
            rect.x() + rect.width() * 0.5,
            rect.y() + rect.height() * 0.5,
        )
    }

    #[test]
    fn panels_can_be_added_and_removed_while_mounted() {
        let state =
            DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs([SCENE], SCENE)))
                .expect("valid tabs");
        let mut builds = 0;
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(SCENE, "Scene", panel("Scene body"))
            .panel_builder(move |id| {
                assert_eq!(id, DETAILS);
                builds += 1;
                Box::new(panel(&format!("Details build {builds}")))
            });
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        runtime.render(window_id).expect("initial frame");

        state
            .add_panel(
                DETAILS,
                "Details",
                DockPlacement::Beside {
                    target: SCENE,
                    zone: DockZone::Right,
                },
            )
            .expect("details registers while mounted");
        assert!(state.is_registered(DETAILS));
        assert!(
            state
                .add_panel(DETAILS, "Again", DockPlacement::Hidden)
                .is_err(),
            "a registered ID cannot be registered twice"
        );
        assert!(
            state
                .add_panel(SCENE, "Scene", DockPlacement::Hidden)
                .is_err(),
            "builder-registered IDs are known to the state"
        );

        let frame = runtime.render(window_id).expect("frame with added panel");
        assert!(named(&frame.semantics, "Details").is_some());
        assert!(named(&frame.semantics, "Details build 1").is_some());

        assert!(state.remove_panel(DETAILS).expect("details removes"));
        assert!(!state.is_registered(DETAILS));
        assert!(
            !state
                .remove_panel(DETAILS)
                .expect("second removal is a no-op")
        );
        let frame = runtime.render(window_id).expect("frame without panel");
        assert!(named(&frame.semantics, "Details").is_none());
        assert!(named(&frame.semantics, "Details build 1").is_none());
        assert_eq!(state.snapshot().root, DockNode::tabs([SCENE], SCENE));

        state
            .add_panel(DETAILS, "Details", DockPlacement::Root(DockZone::Bottom))
            .expect("a removed ID can be reused");
        let frame = runtime.render(window_id).expect("frame with reused ID");
        assert!(named(&frame.semantics, "Details build 2").is_some());
    }

    #[test]
    fn state_stays_shareable_across_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DockWorkspaceState>();

        let state =
            DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs([SCENE], SCENE)))
                .expect("valid tabs");
        let workspace =
            DockWorkspace::new(state.clone()).with_panel(SCENE, "Scene", panel("Scene"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        runtime.render(window_id).expect("initial frame");

        let worker_state = state.clone();
        std::thread::spawn(move || {
            worker_state
                .add_panel(DETAILS, "Details", DockPlacement::Root(DockZone::Right))
                .expect("a worker thread may queue a panel");
        })
        .join()
        .expect("worker finishes");

        let frame = runtime.render(window_id).expect("frame with queued panel");
        assert!(
            named(&frame.semantics, "Details").is_some(),
            "a panel without a builder still gets its tab"
        );
    }

    #[test]
    fn dock_tab_reorders_within_a_group_and_inserts_into_another() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.5,
            DockNode::tabs([SCENE, VIEWPORT], SCENE),
            DockNode::tabs([DETAILS], DETAILS),
        )))
        .expect("valid split");

        assert!(state.dock_tab(VIEWPORT, SCENE, false).expect("reorders"));
        let DockNode::Split { first, .. } = state.snapshot().root else {
            panic!("root remains split");
        };
        assert_eq!(*first, DockNode::tabs([VIEWPORT, SCENE], VIEWPORT));

        assert!(state.dock_tab(DETAILS, VIEWPORT, true).expect("inserts"));
        assert_eq!(
            state.snapshot().root,
            DockNode::tabs([VIEWPORT, DETAILS, SCENE], DETAILS),
            "the emptied group collapses and the tab lands after its anchor"
        );
        assert!(state.dock_tab(SCENE, DockPanelId::new(99), true).is_err());
    }

    #[test]
    fn dragging_a_tab_along_its_header_reorders_the_group() {
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs(
            [SCENE, VIEWPORT],
            SCENE,
        )))
        .expect("valid tabs");
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(SCENE, "Scene", panel("Scene"))
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        let frame = runtime.render(window_id).expect("initial frame");
        let scene_tab = named(&frame.semantics, "Scene").expect("scene tab").bounds;
        let viewport_tab = named(&frame.semantics, "Viewport")
            .expect("viewport tab")
            .bounds;
        let drop = Point::new(viewport_tab.max_x() - 4.0, center(viewport_tab).y);

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, center(scene_tab), true),
            )
            .expect("scene tab press");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, drop, true),
            )
            .expect("drag past the viewport tab");
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Up, drop, false),
            )
            .expect("drop");

        assert_eq!(
            state.snapshot().root,
            DockNode::tabs([VIEWPORT, SCENE], SCENE)
        );
        assert!(state.snapshot().floating.is_empty());
    }

    #[test]
    fn close_button_hides_by_default_and_follows_the_close_handler() {
        let closable = |title: &str| DockPanelOptions::new(title).closable(true);
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs(
            [SCENE, VIEWPORT, DETAILS],
            SCENE,
        )))
        .expect("valid tabs");
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(SCENE, closable("Scene"), panel("Scene"))
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"))
            .with_panel(DETAILS, closable("Details"), panel("Details"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        let frame = runtime.render(window_id).expect("initial frame");
        assert!(
            named(&frame.semantics, "Close Viewport").is_none(),
            "only closable panels expose a close button"
        );
        let close = named(&frame.semantics, "Close Scene").expect("scene close button");
        assert_eq!(close.role, SemanticsRole::Button);
        click(&mut runtime, window_id, center(close.bounds));
        assert!(state.snapshot().hidden.contains(&SCENE));
        assert!(state.is_registered(SCENE), "hiding keeps the panel widget");

        let frame = runtime.render(window_id).expect("frame after hide");
        let close_id = named(&frame.semantics, "Close Details")
            .expect("details close button")
            .id;
        assert!(
            runtime
                .handle_semantics_action(window_id, close_id, SemanticsActionRequest::Activate)
                .expect("close semantics routes")
        );
        assert!(state.snapshot().hidden.contains(&DETAILS));
    }

    #[test]
    fn close_handler_can_keep_or_remove_a_panel() {
        let action = std::rc::Rc::new(std::cell::Cell::new(DockCloseAction::Keep));
        let state = DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs(
            [SCENE, VIEWPORT],
            SCENE,
        )))
        .expect("valid tabs");
        let handler_action = std::rc::Rc::clone(&action);
        let workspace = DockWorkspace::new(state.clone())
            .with_panel(
                SCENE,
                DockPanelOptions::new("Scene").closable(true).dirty(true),
                panel("Scene"),
            )
            .with_panel(VIEWPORT, "Viewport", panel("Viewport"))
            .on_close_request(move |panel| {
                assert_eq!(panel, SCENE);
                handler_action.get()
            });
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        let frame = runtime.render(window_id).expect("initial frame");
        let close = center(
            named(&frame.semantics, "Close Scene")
                .expect("scene close button")
                .bounds,
        );

        click(&mut runtime, window_id, close);
        assert_eq!(
            state.snapshot().root,
            DockNode::tabs([SCENE, VIEWPORT], SCENE),
            "a vetoed close leaves the layout untouched"
        );

        action.set(DockCloseAction::Remove);
        click(&mut runtime, window_id, close);
        assert_eq!(state.snapshot().root, DockNode::tabs([VIEWPORT], VIEWPORT));
        assert!(state.snapshot().hidden.is_empty());
        assert!(!state.is_registered(SCENE));
        let frame = runtime.render(window_id).expect("frame after removal");
        assert!(named(&frame.semantics, "Scene").is_none());
    }

    #[test]
    fn update_panel_changes_the_tab_title_and_unsaved_marker() {
        let state =
            DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs([SCENE], SCENE)))
                .expect("valid tabs");
        let workspace = DockWorkspace::new(state.clone()).with_panel(SCENE, "Scene", panel("Body"));
        let (mut runtime, window_id) = build_runtime(workspace);
        resize(&mut runtime, window_id, 800.0, 600.0);
        let frame = runtime.render(window_id).expect("initial frame");
        assert_eq!(
            named(&frame.semantics, "Scene")
                .expect("scene tab")
                .description,
            None
        );

        state
            .update_panel(SCENE, |options| {
                options.title = "Renamed".to_string();
                options.dirty = true;
            })
            .expect("registered panel updates");
        let frame = runtime.render(window_id).expect("updated frame");
        assert!(named(&frame.semantics, "Scene").is_none());
        assert_eq!(
            named(&frame.semantics, "Renamed")
                .expect("renamed tab")
                .description
                .as_deref(),
            Some("Unsaved changes")
        );
        assert!(state.update_panel(VIEWPORT, |_| {}).is_err());
    }

    #[test]
    fn splits_keep_panels_at_their_minimum_size() {
        let snapshot = DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.2,
            DockNode::tabs([SCENE], SCENE),
            DockNode::tabs([VIEWPORT], VIEWPORT),
        ));
        let width_of = |layout: &DockLayoutCache, panel: DockPanelId| {
            layout
                .groups
                .iter()
                .find(|group| group.active == panel)
                .expect("group is laid out")
                .bounds
                .width()
        };
        let mut panels = HashMap::new();
        panels.insert(
            SCENE,
            DockPanelOptions::new("Scene").min_size(Size::new(420.0, 0.0)),
        );
        panels.insert(VIEWPORT, DockPanelOptions::new("Viewport"));
        let bounds = Rect::new(0.0, 0.0, 804.0, 600.0);

        let layout = build_workspace_layout(&snapshot, bounds, 28.0, 4.0, &panels);
        assert!((width_of(&layout, SCENE) - 420.0).abs() < 0.5);
        assert!((width_of(&layout, VIEWPORT) - 380.0).abs() < 0.5);

        panels.insert(
            VIEWPORT,
            DockPanelOptions::new("Viewport").min_size(Size::new(1_260.0, 0.0)),
        );
        let layout = build_workspace_layout(&snapshot, bounds, 28.0, 4.0, &panels);
        let scene = width_of(&layout, SCENE);
        let viewport = width_of(&layout, VIEWPORT);
        assert!((scene + viewport - 800.0).abs() < 0.5);
        assert!(
            scene >= 150.0 && viewport > scene * 2.5,
            "minimums that cannot both fit shrink in proportion: {scene} / {viewport}"
        );
    }

    #[test]
    fn retain_panels_prunes_missing_panels_and_stays_valid() {
        let mut snapshot = root_snapshot().with_floating(DockFloatingGroup::new(
            1,
            [DockPanelId::new(7)],
            DockPanelId::new(7),
            Rect::new(40.0, 40.0, 300.0, 240.0),
        ));
        snapshot.hidden.push(DockPanelId::new(8));

        snapshot.retain_panels(|panel| panel == SCENE || panel == DETAILS);

        validate_snapshot(&snapshot).expect("pruned snapshot stays canonical");
        assert!(snapshot.floating.is_empty());
        assert!(snapshot.hidden.is_empty());
        assert_eq!(
            snapshot.root,
            DockNode::split(
                Axis::Horizontal,
                0.25,
                DockNode::tabs([SCENE], SCENE),
                DockNode::tabs([DETAILS], DETAILS),
            )
        );
    }

    #[test]
    fn floating_layout_collapses_minimum_size_to_a_tiny_host() {
        let host = Rect::new(10.0, 20.0, 100.0, 80.0);
        let resolved = clamp_floating_bounds(host, Rect::new(-400.0, 500.0, 320.0, 260.0));
        assert_eq!(resolved, host);
    }

    #[test]
    fn registered_panel_validation_reports_unknown_snapshot_ids() {
        let state =
            DockWorkspaceState::new(DockWorkspaceSnapshot::new(DockNode::tabs([SCENE], SCENE)))
                .expect("valid state");
        let workspace =
            DockWorkspace::new(state.clone()).with_panel(SCENE, "Scene", panel("Scene"));
        let restored = DockWorkspaceSnapshot::new(DockNode::tabs([SCENE, VIEWPORT], SCENE));
        let error = workspace
            .apply_snapshot(restored.clone())
            .expect_err("viewport is not registered");
        assert!(error.to_string().contains("panel 2"));
        assert!(!node_contains_panel(&state.snapshot().root, VIEWPORT));

        state
            .apply_snapshot(restored)
            .expect("state-only restore validates graph structure");
        assert!(workspace.validate_registered_panels().is_err());
    }
}
