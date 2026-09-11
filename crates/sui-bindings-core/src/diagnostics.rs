use sui::Point;
use sui::SemanticsNode;
use sui::SemanticsRole;
use sui::SemanticsValue;
use sui::ToggleState;
use sui::WidgetId;

#[derive(Debug, Clone, PartialEq)]
pub struct BindingRenderSnapshot {
    pub command_count: usize,
    pub semantics_count: usize,
    pub semantics_nodes: Vec<BindingSemanticNode>,
    pub semantics_roles: Vec<String>,
    pub semantics_names: Vec<String>,
    pub semantics_values: Vec<String>,
    pub semantics_descriptions: Vec<String>,
    pub semantics_checked: Vec<String>,
    pub semantics_busy: Vec<bool>,
    pub semantics_editable_multiline: Vec<bool>,
    pub semantics_disabled: Vec<bool>,
    pub semantics_focused: Vec<bool>,
    pub semantics_hidden: Vec<bool>,
    pub semantics_hovered: Vec<bool>,
    pub semantics_selected: Vec<bool>,
    pub semantics_expanded: Vec<String>,
    pub fill_rect_count: usize,
    pub draw_image_count: usize,
    pub registered_font_count: usize,
    pub registered_image_count: usize,
}

impl BindingRenderSnapshot {
    pub fn find_nodes(
        &self,
        role: Option<&str>,
        name: Option<&str>,
        text: Option<&str>,
        description: Option<&str>,
        focused: Option<bool>,
        visible: Option<bool>,
    ) -> Vec<BindingSemanticNode> {
        self.semantics_nodes
            .iter()
            .filter(|node| role.is_none_or(|role| node.role == role))
            .filter(|node| name.is_none_or(|name| node.name.as_deref() == Some(name)))
            .filter(|node| {
                text.is_none_or(|text| {
                    node.name.as_deref() == Some(text) || node.value.as_deref() == Some(text)
                })
            })
            .filter(|node| {
                description
                    .is_none_or(|description| node.description.as_deref() == Some(description))
            })
            .filter(|node| focused.is_none_or(|focused| node.focused == focused))
            .filter(|node| visible.is_none_or(|visible| node.visible() == visible))
            .cloned()
            .collect()
    }

    pub fn get_one(
        &self,
        role: Option<&str>,
        name: Option<&str>,
        text: Option<&str>,
    ) -> Result<BindingSemanticNode, String> {
        let nodes = self.find_nodes(role, name, text, None, None, Some(true));
        match nodes.as_slice() {
            [node] => Ok(node.clone()),
            [] => Err("semantic query did not match any visible nodes".to_owned()),
            _ => Err(format!(
                "semantic query matched {} visible nodes instead of exactly one",
                nodes.len()
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingSemanticNode {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub actions: Vec<String>,
    pub checked: Option<String>,
    pub busy: bool,
    pub disabled: bool,
    pub focused: bool,
    pub hidden: bool,
    pub hovered: bool,
    pub selected: bool,
    pub expanded: Option<bool>,
    pub editable: bool,
    pub multiline: bool,
}

impl BindingSemanticNode {
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    pub fn visible(&self) -> bool {
        !self.hidden && self.width > 0.0 && self.height > 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingInspectorSnapshot {
    pub window_id: u64,
    pub title: String,
    pub tracing_enabled: bool,
    pub focused_widget_id: Option<u64>,
    pub window_focused: bool,
    pub scheduled_phases: Vec<String>,
    pub semantics_count: usize,
    pub semantics_nodes: Vec<BindingSemanticNode>,
    pub widget_count: usize,
    pub stack_host_count: usize,
    pub overlay_count: usize,
    pub timer_count: usize,
    pub async_task_count: usize,
    pub requested_animation_frame_count: usize,
    pub widget_diagnostics_count: usize,
    pub event_route_count: usize,
    pub reactive_invalidation_count: usize,
    pub command_dispatch_count: usize,
    pub invalidation_count: usize,
    pub widget_rebuild_count: usize,
    pub frame_timings: Vec<BindingFrameTiming>,
    pub widget_timings: Vec<BindingWidgetTiming>,
    pub event_routes: Vec<BindingEventRouteTrace>,
    pub reactive_invalidations: Vec<BindingReactiveInvalidationTrace>,
    pub command_dispatches: Vec<BindingCommandDispatchTrace>,
    pub invalidations: Vec<BindingInvalidationTrace>,
    pub widget_rebuilds: Vec<BindingWidgetRebuildTrace>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingFrameTiming {
    pub phase: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingWidgetTiming {
    pub widget_id: u64,
    pub widget_name: String,
    pub phase: String,
    pub duration_ms: f64,
    pub calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEventRouteTrace {
    pub sequence: u64,
    pub event_kind: String,
    pub target_id: u64,
    pub path: Vec<u64>,
    pub handled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingReactiveInvalidationTrace {
    pub widget_id: u64,
    pub source_name: String,
    pub version: u64,
    pub kind: String,
    pub delivered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingCommandDispatchTrace {
    pub sequence: u64,
    pub name: String,
    pub payload_type: String,
    pub target: String,
    pub delivery: String,
    pub handlers: Vec<String>,
    pub handled: bool,
    pub delivered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingInvalidationTrace {
    pub target: String,
    pub kind: String,
    pub source: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingWidgetRebuildTrace {
    pub widget_id: u64,
    pub widget_name: String,
    pub reason: String,
}

impl From<sui::WindowInspectorSnapshot> for BindingInspectorSnapshot {
    fn from(value: sui::WindowInspectorSnapshot) -> Self {
        let schedule = value.schedule;
        let mut scheduled_phases = Vec::new();
        for (name, scheduled) in [
            ("measure", schedule.measure),
            ("arrange", schedule.arrange),
            ("ordering", schedule.ordering),
            ("paint", schedule.paint),
            ("semantics", schedule.semantics),
            ("hit_test", schedule.hit_test),
            ("text", schedule.text),
            ("resources", schedule.resources),
        ] {
            if scheduled {
                scheduled_phases.push(name.to_owned());
            }
        }
        let frame_timings = value
            .last_render_diagnostics
            .phase_timings
            .iter()
            .map(|sample| BindingFrameTiming {
                phase: sample.phase.label().to_owned(),
                duration_ms: sample.duration_ms,
            })
            .collect();
        let widget_timings = value
            .last_render_diagnostics
            .widget_timings
            .iter()
            .map(|sample| BindingWidgetTiming {
                widget_id: sample.widget_id.get(),
                widget_name: sample.widget_name.to_owned(),
                phase: sample.phase.label().to_owned(),
                duration_ms: sample.duration_ms,
                calls: sample.calls,
            })
            .collect();
        let event_routes = value
            .history
            .event_routes
            .iter()
            .map(|sample| BindingEventRouteTrace {
                sequence: sample.sequence,
                event_kind: sample.event_kind.to_owned(),
                target_id: sample.target.get(),
                path: sample.path.iter().map(|id| id.get()).collect(),
                handled: sample.handled,
            })
            .collect();
        let reactive_invalidations = value
            .history
            .reactive_invalidations
            .iter()
            .map(|sample| BindingReactiveInvalidationTrace {
                widget_id: sample.widget_id.get(),
                source_name: sample.source_name.clone(),
                version: sample.version,
                kind: format!("{:?}", sample.kind),
                delivered: sample.delivered,
            })
            .collect();
        let command_dispatches = value
            .history
            .command_dispatches
            .iter()
            .map(|sample| BindingCommandDispatchTrace {
                sequence: sample.sequence,
                name: sample.name.clone(),
                payload_type: sample.payload_type.clone(),
                target: format!("{:?}", sample.target),
                delivery: format!("{:?}", sample.delivery),
                handlers: sample.handlers.clone(),
                handled: sample.handled,
                delivered: sample.delivered,
            })
            .collect();
        let invalidations = value
            .history
            .invalidations
            .iter()
            .map(|sample| BindingInvalidationTrace {
                target: format!("{:?}", sample.target),
                kind: format!("{:?}", sample.kind),
                source: sample.source.clone(),
                reason: sample.reason.clone(),
            })
            .collect();
        let widget_rebuilds = value
            .history
            .widget_rebuilds
            .iter()
            .map(|sample| BindingWidgetRebuildTrace {
                widget_id: sample.widget_id.get(),
                widget_name: sample.widget_name.to_owned(),
                reason: sample.reason.clone(),
            })
            .collect();
        Self {
            window_id: value.window_id.get(),
            title: value.title,
            tracing_enabled: value.tracing_enabled,
            focused_widget_id: value.focus_state.focused_widget.map(WidgetId::get),
            window_focused: value.focus_state.window_focused,
            scheduled_phases,
            semantics_count: value.semantics.len(),
            semantics_nodes: binding_semantics_nodes(&value.semantics),
            widget_count: value.widget_graph.nodes.len(),
            stack_host_count: value.widget_graph.stack_hosts.len(),
            overlay_count: value.overlays.overlays.len(),
            timer_count: value.scheduler.timers.len(),
            async_task_count: value.scheduler.async_tasks.len(),
            requested_animation_frame_count: value.scheduler.requested_animation_frames.len(),
            widget_diagnostics_count: value.widget_diagnostics.len(),
            event_route_count: value.history.event_routes.len(),
            reactive_invalidation_count: value.history.reactive_invalidations.len(),
            command_dispatch_count: value.history.command_dispatches.len(),
            invalidation_count: value.history.invalidations.len(),
            widget_rebuild_count: value.history.widget_rebuilds.len(),
            frame_timings,
            widget_timings,
            event_routes,
            reactive_invalidations,
            command_dispatches,
            invalidations,
            widget_rebuilds,
        }
    }
}

pub fn binding_semantics_role_name(role: &SemanticsRole) -> &'static str {
    match role {
        SemanticsRole::Window => "window",
        SemanticsRole::Root => "root",
        SemanticsRole::GenericContainer => "generic_container",
        SemanticsRole::Separator => "separator",
        SemanticsRole::List => "list",
        SemanticsRole::ListItem => "list_item",
        SemanticsRole::Tree => "tree",
        SemanticsRole::Table => "table",
        SemanticsRole::Splitter => "splitter",
        SemanticsRole::Breadcrumb => "breadcrumb",
        SemanticsRole::TabBar => "tab_bar",
        SemanticsRole::Tabs => "tabs",
        SemanticsRole::Button => "button",
        SemanticsRole::Link => "link",
        SemanticsRole::CheckBox => "checkbox",
        SemanticsRole::Switch => "switch",
        SemanticsRole::RadioButton => "radio_button",
        SemanticsRole::RadioGroup => "radio_group",
        SemanticsRole::Menu => "menu",
        SemanticsRole::MenuItem => "menu_item",
        SemanticsRole::ContextMenu => "context_menu",
        SemanticsRole::Tooltip => "tooltip",
        SemanticsRole::Dialog => "dialog",
        SemanticsRole::Popover => "popover",
        SemanticsRole::Slider => "slider",
        SemanticsRole::ProgressBar => "progress_bar",
        SemanticsRole::BusyIndicator => "busy_indicator",
        SemanticsRole::Document => "document",
        SemanticsRole::Paragraph => "paragraph",
        SemanticsRole::Heading => "heading",
        SemanticsRole::Code => "code",
        SemanticsRole::Status => "status",
        SemanticsRole::Attachment => "attachment",
        SemanticsRole::Text => "text",
        SemanticsRole::TextInput => "text_input",
        SemanticsRole::SpinBox => "spin_box",
        SemanticsRole::ComboBox => "combo_box",
        SemanticsRole::Image => "image",
        SemanticsRole::ColorSwatch => "color_swatch",
        SemanticsRole::ColorPicker => "color_picker",
        SemanticsRole::Canvas => "canvas",
        SemanticsRole::ScrollView => "scroll_view",
    }
}

pub fn binding_semantics_role_from_name(value: &str) -> Option<SemanticsRole> {
    match value {
        "window" => Some(SemanticsRole::Window),
        "root" => Some(SemanticsRole::Root),
        "generic_container" | "generic-container" | "genericContainer" | "generic" => {
            Some(SemanticsRole::GenericContainer)
        }
        "separator" => Some(SemanticsRole::Separator),
        "list" => Some(SemanticsRole::List),
        "list_item" | "list-item" | "listItem" => Some(SemanticsRole::ListItem),
        "tree" => Some(SemanticsRole::Tree),
        "table" => Some(SemanticsRole::Table),
        "splitter" => Some(SemanticsRole::Splitter),
        "breadcrumb" => Some(SemanticsRole::Breadcrumb),
        "tab_bar" | "tab-bar" | "tabBar" => Some(SemanticsRole::TabBar),
        "tabs" => Some(SemanticsRole::Tabs),
        "button" => Some(SemanticsRole::Button),
        "link" => Some(SemanticsRole::Link),
        "checkbox" | "check_box" | "check-box" | "checkBox" => Some(SemanticsRole::CheckBox),
        "switch" => Some(SemanticsRole::Switch),
        "radio_button" | "radio-button" | "radioButton" => Some(SemanticsRole::RadioButton),
        "radio_group" | "radio-group" | "radioGroup" => Some(SemanticsRole::RadioGroup),
        "menu" => Some(SemanticsRole::Menu),
        "menu_item" | "menu-item" | "menuItem" => Some(SemanticsRole::MenuItem),
        "context_menu" | "context-menu" | "contextMenu" => Some(SemanticsRole::ContextMenu),
        "tooltip" => Some(SemanticsRole::Tooltip),
        "dialog" => Some(SemanticsRole::Dialog),
        "popover" => Some(SemanticsRole::Popover),
        "slider" => Some(SemanticsRole::Slider),
        "progress_bar" | "progress-bar" | "progressBar" => Some(SemanticsRole::ProgressBar),
        "busy_indicator" | "busy-indicator" | "busyIndicator" => Some(SemanticsRole::BusyIndicator),
        "document" => Some(SemanticsRole::Document),
        "paragraph" => Some(SemanticsRole::Paragraph),
        "heading" => Some(SemanticsRole::Heading),
        "code" => Some(SemanticsRole::Code),
        "status" => Some(SemanticsRole::Status),
        "attachment" => Some(SemanticsRole::Attachment),
        "text" => Some(SemanticsRole::Text),
        "text_input" | "text-input" | "textInput" => Some(SemanticsRole::TextInput),
        "spin_box" | "spin-box" | "spinBox" => Some(SemanticsRole::SpinBox),
        "combo_box" | "combo-box" | "comboBox" => Some(SemanticsRole::ComboBox),
        "image" => Some(SemanticsRole::Image),
        "color_swatch" | "color-swatch" | "colorSwatch" => Some(SemanticsRole::ColorSwatch),
        "color_picker" | "color-picker" | "colorPicker" => Some(SemanticsRole::ColorPicker),
        "canvas" => Some(SemanticsRole::Canvas),
        "scroll_view" | "scroll-view" | "scrollView" => Some(SemanticsRole::ScrollView),
        _ => None,
    }
}

pub fn binding_toggle_state_name(state: ToggleState) -> &'static str {
    match state {
        ToggleState::Unchecked => "unchecked",
        ToggleState::Checked => "checked",
        ToggleState::Mixed => "mixed",
    }
}

pub fn binding_toggle_state_from_name(value: &str) -> Option<ToggleState> {
    match value {
        "unchecked" | "false" | "off" => Some(ToggleState::Unchecked),
        "checked" | "true" | "on" => Some(ToggleState::Checked),
        "mixed" | "indeterminate" => Some(ToggleState::Mixed),
        _ => None,
    }
}

pub fn binding_semantics_value_text(value: Option<&SemanticsValue>) -> String {
    match value {
        Some(SemanticsValue::Text(value)) => value.clone(),
        Some(SemanticsValue::Number(value)) => value.to_string(),
        Some(SemanticsValue::Range { value, min, max }) => format!("{value}:{min}:{max}"),
        None => String::new(),
    }
}

pub fn binding_semantics_roles(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| binding_semantics_role_name(&node.role).to_owned())
        .collect()
}

pub fn binding_semantics_names(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| node.name.clone().unwrap_or_default())
        .collect()
}

pub fn binding_semantics_values(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            if node
                .editable_text
                .as_ref()
                .is_some_and(|editable| editable.password)
            {
                return match node.value.as_ref() {
                    Some(SemanticsValue::Text(value)) => "•".repeat(value.chars().count()),
                    _ => String::new(),
                };
            }
            binding_semantics_value_text(node.value.as_ref())
        })
        .collect()
}

pub fn binding_semantics_nodes(nodes: &[SemanticsNode]) -> Vec<BindingSemanticNode> {
    nodes
        .iter()
        .map(|node| {
            let value = if node
                .editable_text
                .as_ref()
                .is_some_and(|editable| editable.password)
            {
                match node.value.as_ref() {
                    Some(SemanticsValue::Text(value)) => Some("•".repeat(value.chars().count())),
                    _ => None,
                }
            } else {
                node.value
                    .as_ref()
                    .map(|value| binding_semantics_value_text(Some(value)))
            };
            BindingSemanticNode {
                id: node.id.get(),
                parent_id: node.parent.map(WidgetId::get),
                role: binding_semantics_role_name(&node.role).to_owned(),
                name: node.name.clone(),
                value,
                description: node.description.clone(),
                x: node.bounds.x(),
                y: node.bounds.y(),
                width: node.bounds.width(),
                height: node.bounds.height(),
                actions: node
                    .actions
                    .iter()
                    .map(|action| format!("{action:?}"))
                    .collect(),
                checked: node
                    .state
                    .checked
                    .map(binding_toggle_state_name)
                    .map(str::to_owned),
                busy: node.state.busy,
                disabled: node.state.disabled,
                focused: node.state.focused,
                hidden: node.state.hidden,
                hovered: node.state.hovered,
                selected: node.state.selected,
                expanded: node.state.expanded,
                editable: node.editable_text.is_some(),
                multiline: node
                    .editable_text
                    .as_ref()
                    .is_some_and(|editable| editable.multiline),
            }
        })
        .collect()
}

pub fn binding_semantics_descriptions(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| node.description.clone().unwrap_or_default())
        .collect()
}

pub fn binding_semantics_checked(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            node.state
                .checked
                .map(binding_toggle_state_name)
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

pub fn binding_semantics_busy(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.busy).collect()
}

pub fn binding_semantics_editable_multiline(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes
        .iter()
        .map(|node| {
            node.editable_text
                .as_ref()
                .is_some_and(|editable| editable.multiline)
        })
        .collect()
}

pub fn binding_semantics_disabled(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.disabled).collect()
}

pub fn binding_semantics_focused(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.focused).collect()
}

pub fn binding_semantics_hidden(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.hidden).collect()
}

pub fn binding_semantics_hovered(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.hovered).collect()
}

pub fn binding_semantics_selected(nodes: &[SemanticsNode]) -> Vec<bool> {
    nodes.iter().map(|node| node.state.selected).collect()
}

pub fn binding_semantics_expanded(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node.state.expanded {
            Some(true) => "expanded",
            Some(false) => "collapsed",
            None => "",
        })
        .map(str::to_owned)
        .collect()
}
