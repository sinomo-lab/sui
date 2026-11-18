use crate::application::normalized_option_name;
use crate::errors::ForeignCallbackResult;
use crate::widget_descriptor::BindingWidget;
use std::fmt;
use std::sync::Arc;
use sui::Axis;
use sui::DockCloseAction;
use sui::DockFloatingGroup;
use sui::DockNode;
use sui::DockPanelId;
use sui::DockPanelOptions;
use sui::DockPlacement;
use sui::DockWorkspaceSnapshot;
use sui::DockWorkspaceState;
use sui::DockZone;
use sui::FloatingViewSnapshot;
use sui::FloatingWorkspaceState;
use sui::IconGlyph;
use sui::Rect;
use sui::Size;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BindingDockNode {
    pub(crate) inner: DockNode,
}

impl BindingDockNode {
    pub fn empty() -> Self {
        Self {
            inner: DockNode::empty(),
        }
    }

    pub fn tabs(
        panel_ids: impl IntoIterator<Item = u64>,
        active: Option<u64>,
    ) -> Result<Self, String> {
        let panel_ids = panel_ids.into_iter().collect::<Vec<_>>();
        if panel_ids.is_empty() {
            return Err("dock tabs require at least one panel id".to_string());
        }
        if panel_ids.contains(&0) {
            return Err("dock panel ids must be non-zero".to_string());
        }
        let active = active.unwrap_or(panel_ids[0]);
        if !panel_ids.contains(&active) {
            return Err(format!(
                "active dock panel {active} is not present in the tab group"
            ));
        }
        Ok(Self {
            inner: DockNode::tabs(
                panel_ids.into_iter().map(DockPanelId::new),
                DockPanelId::new(active),
            ),
        })
    }

    pub fn split(axis: Axis, fraction: f32, first: Self, second: Self) -> Result<Self, String> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err("dock split fraction must be finite and between 0 and 1".to_string());
        }
        Ok(Self {
            inner: DockNode::split(axis, fraction, first.inner, second.inner),
        })
    }

    pub(crate) fn into_sui(self) -> DockNode {
        self.inner
    }

    pub(crate) fn from_sui(inner: DockNode) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingDockFloatingGroup {
    pub id: u64,
    pub panel_ids: Vec<u64>,
    pub active: u64,
    pub bounds: Rect,
}

impl BindingDockFloatingGroup {
    pub fn new(
        id: u64,
        panel_ids: impl IntoIterator<Item = u64>,
        active: u64,
        bounds: Rect,
    ) -> Self {
        Self {
            id,
            panel_ids: panel_ids.into_iter().collect(),
            active,
            bounds,
        }
    }

    pub(crate) fn into_sui(self) -> DockFloatingGroup {
        DockFloatingGroup::new(
            self.id,
            self.panel_ids.into_iter().map(DockPanelId::new),
            DockPanelId::new(self.active),
            self.bounds,
        )
    }

    pub(crate) fn from_sui(value: DockFloatingGroup) -> Self {
        Self {
            id: value.id,
            panel_ids: value.panels.into_iter().map(DockPanelId::get).collect(),
            active: value.active.get(),
            bounds: value.bounds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BindingDockLayout {
    pub root: BindingDockNode,
    pub floating: Vec<BindingDockFloatingGroup>,
    pub hidden: Vec<u64>,
}

impl BindingDockLayout {
    pub fn new(
        root: BindingDockNode,
        floating: impl IntoIterator<Item = BindingDockFloatingGroup>,
        hidden: impl IntoIterator<Item = u64>,
    ) -> Self {
        Self {
            root,
            floating: floating.into_iter().collect(),
            hidden: hidden.into_iter().collect(),
        }
    }

    pub(crate) fn into_sui(self) -> DockWorkspaceSnapshot {
        DockWorkspaceSnapshot {
            root: self.root.into_sui(),
            floating: self
                .floating
                .into_iter()
                .map(BindingDockFloatingGroup::into_sui)
                .collect(),
            hidden: self.hidden.into_iter().map(DockPanelId::new).collect(),
        }
    }

    /// A copy of this layout without the panels missing from `panel_ids`,
    /// with the groups and splits left empty collapsed.
    pub fn retain_panels(&self, panel_ids: impl IntoIterator<Item = u64>) -> Self {
        let keep = panel_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let mut snapshot = self.clone().into_sui();
        snapshot.retain_panels(|panel| keep.contains(&panel.get()));
        Self::from_sui(snapshot)
    }

    pub(crate) fn from_sui(value: DockWorkspaceSnapshot) -> Self {
        Self {
            root: BindingDockNode::from_sui(value.root),
            floating: value
                .floating
                .into_iter()
                .map(BindingDockFloatingGroup::from_sui)
                .collect(),
            hidden: value.hidden.into_iter().map(DockPanelId::get).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingDockState {
    pub(crate) inner: DockWorkspaceState,
}

impl BindingDockState {
    pub fn new(layout: BindingDockLayout) -> Result<Self, String> {
        DockWorkspaceState::new(layout.into_sui())
            .map(|inner| Self { inner })
            .map_err(|error| error.to_string())
    }

    pub fn empty() -> Self {
        Self {
            inner: DockWorkspaceState::empty(),
        }
    }

    pub fn snapshot(&self) -> BindingDockLayout {
        BindingDockLayout::from_sui(self.inner.snapshot())
    }

    pub fn apply(&self, layout: BindingDockLayout) -> Result<bool, String> {
        self.inner
            .apply_snapshot(layout.into_sui())
            .map_err(|error| error.to_string())
    }

    pub fn dock(&self, panel: u64, target: u64, zone: &str) -> Result<bool, String> {
        self.inner
            .dock(
                DockPanelId::new(panel),
                DockPanelId::new(target),
                binding_dock_zone_from_name(zone)?,
            )
            .map_err(|error| error.to_string())
    }

    pub fn dock_to_root(&self, panel: u64, zone: &str) -> Result<bool, String> {
        self.inner
            .dock_to_root(DockPanelId::new(panel), binding_dock_zone_from_name(zone)?)
            .map_err(|error| error.to_string())
    }

    pub fn float_panel(&self, panel: u64, bounds: Rect) -> Result<u64, String> {
        self.inner
            .float_panel(DockPanelId::new(panel), bounds)
            .map_err(|error| error.to_string())
    }

    pub fn hide(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .hide(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }

    pub fn show(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .show(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }

    pub fn activate(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .activate(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }

    /// Moves `panel` beside `anchor`'s tab, in the anchor's tab group.
    pub fn dock_tab(&self, panel: u64, anchor: u64, after: bool) -> Result<bool, String> {
        self.inner
            .dock_tab(DockPanelId::new(panel), DockPanelId::new(anchor), after)
            .map_err(|error| error.to_string())
    }

    /// Registers a panel on a mounted workspace, whose panel builder then
    /// creates the panel's widget.
    pub fn add_panel(
        &self,
        panel: u64,
        options: impl Into<BindingDockPanelOptions>,
        placement: BindingDockPlacement,
    ) -> Result<(), String> {
        self.inner
            .add_panel(
                DockPanelId::new(panel),
                options.into().inner,
                placement.inner,
            )
            .map_err(|error| error.to_string())
    }

    pub fn remove_panel(&self, panel: u64) -> Result<bool, String> {
        self.inner
            .remove_panel(DockPanelId::new(panel))
            .map_err(|error| error.to_string())
    }

    /// Replaces a registered panel's tab options.
    pub fn update_panel(&self, panel: u64, options: BindingDockPanelOptions) -> Result<(), String> {
        self.inner
            .update_panel(DockPanelId::new(panel), move |current| {
                *current = options.inner;
            })
            .map_err(|error| error.to_string())
    }

    pub fn is_registered(&self, panel: u64) -> bool {
        self.inner.is_registered(DockPanelId::new(panel))
    }
}

/// A dock panel's tab title, icon, close button, unsaved-changes marker, and
/// minimum content size.
#[derive(Debug, Clone, PartialEq)]
pub struct BindingDockPanelOptions {
    pub(crate) inner: DockPanelOptions,
}

impl BindingDockPanelOptions {
    pub fn new(
        title: impl Into<String>,
        icon: Option<IconGlyph>,
        closable: bool,
        dirty: bool,
        min_size: Option<Size>,
    ) -> Result<Self, String> {
        let min_size = min_size.unwrap_or(Size::ZERO);
        if !min_size.width.is_finite()
            || !min_size.height.is_finite()
            || min_size.width < 0.0
            || min_size.height < 0.0
        {
            return Err("dock panel minimum size must be finite and non-negative".to_string());
        }
        let mut inner = DockPanelOptions::new(title)
            .closable(closable)
            .dirty(dirty)
            .min_size(min_size);
        inner.icon = icon;
        Ok(Self { inner })
    }

    pub fn title(&self) -> &str {
        &self.inner.title
    }

    pub fn icon(&self) -> Option<IconGlyph> {
        self.inner.icon
    }

    pub fn closable(&self) -> bool {
        self.inner.closable
    }

    pub fn dirty(&self) -> bool {
        self.inner.dirty
    }

    pub fn min_size(&self) -> Size {
        self.inner.min_size
    }
}

impl From<&str> for BindingDockPanelOptions {
    fn from(title: &str) -> Self {
        Self {
            inner: DockPanelOptions::new(title),
        }
    }
}

impl From<String> for BindingDockPanelOptions {
    fn from(title: String) -> Self {
        Self {
            inner: DockPanelOptions::new(title),
        }
    }
}

/// Where a panel added to a mounted workspace first appears.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingDockPlacement {
    pub(crate) inner: DockPlacement,
}

impl BindingDockPlacement {
    pub fn root(zone: &str) -> Result<Self, String> {
        Ok(Self {
            inner: DockPlacement::Root(binding_dock_zone_from_name(zone)?),
        })
    }

    pub fn beside(target: u64, zone: &str) -> Result<Self, String> {
        if target == 0 {
            return Err("dock target panel id must be non-zero".to_string());
        }
        Ok(Self {
            inner: DockPlacement::Beside {
                target: DockPanelId::new(target),
                zone: binding_dock_zone_from_name(zone)?,
            },
        })
    }

    pub fn floating(bounds: Rect) -> Self {
        Self {
            inner: DockPlacement::Floating(bounds),
        }
    }

    pub fn hidden() -> Self {
        Self {
            inner: DockPlacement::Hidden,
        }
    }
}

impl Default for BindingDockPlacement {
    fn default() -> Self {
        Self {
            inner: DockPlacement::Root(DockZone::Center),
        }
    }
}

/// Creates the widget of a panel added through
/// [`BindingDockState::add_panel`], from a foreign callback on the UI thread.
#[derive(Clone)]
pub struct BindingDockPanelBuilder {
    callback: Arc<dyn Fn(u64) -> ForeignCallbackResult<BindingWidget> + Send + Sync + 'static>,
}

impl BindingDockPanelBuilder {
    pub fn new(
        callback: impl Fn(u64) -> ForeignCallbackResult<BindingWidget> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub(crate) fn build(&self, panel: u64) -> ForeignCallbackResult<BindingWidget> {
        (self.callback)(panel)
    }
}

impl fmt::Debug for BindingDockPanelBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingDockPanelBuilder")
            .finish_non_exhaustive()
    }
}

/// Decides what a press on a tab's close button does, from a foreign
/// callback on the UI thread.
///
/// The callback returns `"hide"`, `"remove"`, or `"keep"`; `None` hides the
/// panel, as a workspace without a handler does.
#[derive(Clone)]
pub struct BindingDockCloseHandler {
    callback: Arc<dyn Fn(u64) -> ForeignCallbackResult<Option<String>> + Send + Sync + 'static>,
}

impl BindingDockCloseHandler {
    pub fn new(
        callback: impl Fn(u64) -> ForeignCallbackResult<Option<String>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub(crate) fn close_action(&self, panel: u64) -> ForeignCallbackResult<DockCloseAction> {
        match (self.callback)(panel)? {
            None => Ok(DockCloseAction::Hide),
            Some(action) => binding_dock_close_action_from_name(&action).map_err(Into::into),
        }
    }
}

impl fmt::Debug for BindingDockCloseHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingDockCloseHandler")
            .finish_non_exhaustive()
    }
}

fn binding_dock_close_action_from_name(value: &str) -> Result<DockCloseAction, String> {
    match normalized_option_name(value).as_str() {
        "hide" => Ok(DockCloseAction::Hide),
        "remove" => Ok(DockCloseAction::Remove),
        "keep" => Ok(DockCloseAction::Keep),
        _ => Err(format!(
            "dock close action must be 'hide', 'remove', or 'keep', got '{value}'"
        )),
    }
}

pub(crate) fn binding_dock_zone_from_name(value: &str) -> Result<DockZone, String> {
    match normalized_option_name(value).as_str() {
        "center" | "tab" => Ok(DockZone::Center),
        "left" => Ok(DockZone::Left),
        "right" => Ok(DockZone::Right),
        "top" => Ok(DockZone::Top),
        "bottom" => Ok(DockZone::Bottom),
        _ => Err(format!(
            "dock zone must be 'center', 'left', 'right', 'top', or 'bottom', got '{value}'"
        )),
    }
}

#[derive(Debug, Clone)]
pub struct BindingDockPanel {
    pub id: u64,
    pub options: BindingDockPanelOptions,
    pub child: BindingWidget,
}

impl BindingDockPanel {
    pub fn new(
        id: u64,
        options: impl Into<BindingDockPanelOptions>,
        child: BindingWidget,
    ) -> Result<Self, String> {
        if id == 0 {
            return Err("dock panel id must be non-zero".to_string());
        }
        Ok(Self {
            id,
            options: options.into(),
            child,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BindingFloatingView {
    pub(crate) id: Option<u64>,
    pub title: String,
    pub bounds: Rect,
    pub min_size: Size,
    pub visible: bool,
    pub child: BindingWidget,
}

impl BindingFloatingView {
    pub fn new(
        title: impl Into<String>,
        bounds: Rect,
        min_size: Size,
        visible: bool,
        child: BindingWidget,
    ) -> Self {
        Self {
            id: None,
            title: title.into(),
            bounds,
            min_size,
            visible,
            child,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingFloatingViewSnapshot {
    pub id: u64,
    pub title: String,
    pub bounds: Rect,
    pub min_size: Size,
    pub visible: bool,
    pub maximized: bool,
}

impl From<FloatingViewSnapshot> for BindingFloatingViewSnapshot {
    fn from(value: FloatingViewSnapshot) -> Self {
        Self {
            id: value.id,
            title: value.title,
            bounds: value.bounds,
            min_size: value.min_size,
            visible: value.visible,
            maximized: value.maximized,
        }
    }
}

#[derive(Clone)]
pub struct BindingFloatingWorkspaceState {
    pub(crate) inner: FloatingWorkspaceState,
}

impl fmt::Debug for BindingFloatingWorkspaceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingFloatingWorkspaceState")
            .field("views", &self.views())
            .finish()
    }
}

impl BindingFloatingWorkspaceState {
    pub fn new() -> Self {
        Self {
            inner: FloatingWorkspaceState::new(),
        }
    }

    pub fn views(&self) -> Vec<BindingFloatingViewSnapshot> {
        self.inner.snapshots().into_iter().map(Into::into).collect()
    }

    pub fn set_visible(&self, id: u64, visible: bool) -> bool {
        self.inner.set_view_visible(id, visible)
    }

    pub fn set_bounds(&self, id: u64, bounds: Rect) -> bool {
        self.inner.set_view_bounds(id, bounds)
    }

    pub fn bring_to_front(&self, id: u64) -> bool {
        self.inner.bring_to_front(id)
    }

    pub fn set_maximized(&self, id: u64, maximized: bool) -> bool {
        self.inner.set_view_maximized(id, maximized)
    }
}

impl Default for BindingFloatingWorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}
