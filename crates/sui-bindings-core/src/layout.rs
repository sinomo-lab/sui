use crate::application::normalized_option_name;
use crate::widget_descriptor::BindingWidget;
use sui::ConstraintOrientation;
use sui::ConstraintQuery;
use sui::MasterDetailRoute;
use sui::MasterDetailState;
use sui::ResponsiveSidebarState;

#[derive(Debug, Clone)]
pub struct BindingConstraintCase {
    pub(crate) query: ConstraintQuery,
    pub(crate) child: BindingWidget,
}

impl BindingConstraintCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        child: BindingWidget,
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        min_aspect_ratio: Option<f32>,
        max_aspect_ratio: Option<f32>,
        orientation: &str,
    ) -> Result<Self, String> {
        let mut query = ConstraintQuery::new();
        if let Some(value) = min_width {
            query = query.min_width(value);
        }
        if let Some(value) = max_width {
            query = query.max_width(value);
        }
        if let Some(value) = min_height {
            query = query.min_height(value);
        }
        if let Some(value) = max_height {
            query = query.max_height(value);
        }
        if let Some(value) = min_aspect_ratio {
            query = query.min_aspect_ratio(value);
        }
        if let Some(value) = max_aspect_ratio {
            query = query.max_aspect_ratio(value);
        }
        query = query.orientation(match normalized_option_name(orientation).as_str() {
            "any" => ConstraintOrientation::Any,
            "portrait" => ConstraintOrientation::Portrait,
            "landscape" => ConstraintOrientation::Landscape,
            _ => {
                return Err(format!(
                    "constraint orientation must be 'any', 'portrait', or 'landscape', got '{orientation}'"
                ));
            }
        });
        Ok(Self { query, child })
    }
}

#[derive(Debug, Clone)]
pub struct BindingResponsiveSidebarState {
    pub(crate) inner: ResponsiveSidebarState,
}

impl BindingResponsiveSidebarState {
    pub fn new(expanded: bool, overlay_open: bool) -> Self {
        let inner = ResponsiveSidebarState::new();
        inner.set_expanded(expanded);
        if overlay_open {
            inner.open_overlay();
        }
        Self { inner }
    }

    pub fn expanded(&self) -> bool {
        self.inner.snapshot().expanded
    }

    pub fn overlay_open(&self) -> bool {
        self.inner.snapshot().overlay_open
    }

    pub fn set_expanded(&self, expanded: bool) -> bool {
        self.inner.set_expanded(expanded)
    }

    pub fn toggle_expanded(&self) -> bool {
        self.inner.toggle_expanded()
    }

    pub fn open_overlay(&self) -> bool {
        self.inner.open_overlay()
    }

    pub fn close_overlay(&self) -> bool {
        self.inner.close_overlay()
    }

    pub fn toggle_overlay(&self) -> bool {
        self.inner.toggle_overlay()
    }
}

#[derive(Debug, Clone)]
pub struct BindingMasterDetailState {
    pub(crate) inner: MasterDetailState,
}

impl BindingMasterDetailState {
    pub fn new(route: &str) -> Result<Self, String> {
        Ok(Self {
            inner: MasterDetailState::new(binding_master_detail_route(route)?),
        })
    }

    pub fn route(&self) -> &'static str {
        match self.inner.route() {
            MasterDetailRoute::Master => "master",
            MasterDetailRoute::Detail => "detail",
        }
    }

    pub fn set_route(&self, route: &str) -> Result<bool, String> {
        Ok(self.inner.set_route(binding_master_detail_route(route)?))
    }

    pub fn show_master(&self) -> bool {
        self.inner.show_master()
    }

    pub fn show_detail(&self) -> bool {
        self.inner.show_detail()
    }
}

pub(crate) fn binding_master_detail_route(value: &str) -> Result<MasterDetailRoute, String> {
    match normalized_option_name(value).as_str() {
        "master" | "list" => Ok(MasterDetailRoute::Master),
        "detail" => Ok(MasterDetailRoute::Detail),
        _ => Err(format!(
            "master-detail route must be 'master' or 'detail', got '{value}'"
        )),
    }
}
