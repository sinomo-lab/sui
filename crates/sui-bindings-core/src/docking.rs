use crate::application::normalized_option_name;
use crate::widget_descriptor::BindingWidget;
use std::fmt;
use sui::Axis;
use sui::DockFloatingGroup;
use sui::DockNode;
use sui::DockPanelId;
use sui::DockWorkspaceSnapshot;
use sui::DockWorkspaceState;
use sui::DockZone;
use sui::FloatingViewSnapshot;
use sui::FloatingWorkspaceState;
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
    pub title: String,
    pub child: BindingWidget,
}

impl BindingDockPanel {
    pub fn new(id: u64, title: impl Into<String>, child: BindingWidget) -> Result<Self, String> {
        if id == 0 {
            return Err("dock panel id must be non-zero".to_string());
        }
        Ok(Self {
            id,
            title: title.into(),
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
