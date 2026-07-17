use std::hash::{Hash, Hasher};

use sui_core::{
    EditableTextSemantics, Rect, SemanticsAction, SemanticsNode, SemanticsRole, SemanticsState,
    SemanticsValue, ToggleState, WindowId,
};
use sui_runtime::WindowPerformanceSnapshot;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::HashMap};

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Function, Reflect};
#[cfg(target_arch = "wasm32")]
use sui_core::WidgetId;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

#[derive(Debug, Clone, PartialEq)]
#[cfg(target_arch = "wasm32")]
pub(crate) enum WebInteropCommand {
    Click {
        target: WebInteropTarget,
    },
    Scroll {
        target: WebInteropTarget,
        delta_x: f32,
        delta_y: f32,
    },
    Key {
        target: WebInteropTarget,
        key: String,
    },
    Text {
        target: WebInteropTarget,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(target_arch = "wasm32")]
pub(crate) struct WebInteropTarget {
    pub widget_id: Option<WidgetId>,
    pub role: Option<String>,
    pub name: Option<String>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SNAPSHOT_CACHE: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn publish_snapshot(
    window_id: WindowId,
    frame_index: u64,
    scale_factor: f64,
    viewport: sui_core::Size,
    nodes: &[SemanticsNode],
    performance: Option<&WindowPerformanceSnapshot>,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(publish) = Reflect::get(&window, &JsValue::from_str("__suiPublishSnapshot")) else {
        return;
    };
    let Some(publish) = publish.dyn_ref::<Function>() else {
        return;
    };

    let semantics_hash = semantics_hash(nodes);
    let nodes_changed = SNAPSHOT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let previous = cache.insert(window_id.get(), semantics_hash);
        previous != Some(semantics_hash)
    });
    let snapshot = serialize_snapshot(
        window_id,
        frame_index,
        scale_factor,
        viewport,
        if nodes_changed { Some(nodes) } else { None },
        performance,
        semantics_hash,
    );
    let _ = publish.call1(&window, &JsValue::from_str(&snapshot));
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn drain_commands() -> Vec<WebInteropCommand> {
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };
    let Ok(drain) = Reflect::get(&window, &JsValue::from_str("__suiDrainCommands")) else {
        return Vec::new();
    };
    let Some(drain) = drain.dyn_ref::<Function>() else {
        return Vec::new();
    };
    let Ok(value) = drain.call0(&window) else {
        return Vec::new();
    };

    Array::from(&value)
        .iter()
        .filter_map(parse_command)
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn parse_command(value: JsValue) -> Option<WebInteropCommand> {
    let kind = get_string(&value, "type")?;
    let target = WebInteropTarget {
        widget_id: get_number(&value, "id").map(|id| WidgetId::new(id as u64)),
        role: get_string(&value, "role"),
        name: get_string(&value, "name"),
    };
    match kind.as_str() {
        "click" => Some(WebInteropCommand::Click { target }),
        "scroll" => Some(WebInteropCommand::Scroll {
            target,
            delta_x: get_number(&value, "deltaX").unwrap_or(0.0) as f32,
            delta_y: get_number(&value, "deltaY").unwrap_or(0.0) as f32,
        }),
        "key" => Some(WebInteropCommand::Key {
            target,
            key: get_string(&value, "key")?,
        }),
        "text" => Some(WebInteropCommand::Text {
            target,
            text: get_string(&value, "text")?,
        }),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn get_string(value: &JsValue, property: &str) -> Option<String> {
    Reflect::get(value, &JsValue::from_str(property))
        .ok()
        .and_then(|value| value.as_string())
}

#[cfg(target_arch = "wasm32")]
fn get_number(value: &JsValue, property: &str) -> Option<f64> {
    Reflect::get(value, &JsValue::from_str(property))
        .ok()
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
}

fn serialize_snapshot(
    window_id: WindowId,
    frame_index: u64,
    scale_factor: f64,
    viewport: sui_core::Size,
    nodes: Option<&[SemanticsNode]>,
    performance: Option<&WindowPerformanceSnapshot>,
    semantics_hash: u64,
) -> String {
    let mut json = String::new();
    json.push('{');
    push_json_number_field(&mut json, "windowId", window_id.get() as f64);
    json.push(',');
    push_json_number_field(&mut json, "frameIndex", frame_index as f64);
    json.push(',');
    push_json_number_field(&mut json, "devicePixelRatio", scale_factor);
    json.push(',');
    push_json_string_field(
        &mut json,
        "semanticsHash",
        &format!("{semantics_hash:016x}"),
    );
    json.push(',');
    push_json_bool_field(&mut json, "nodesChanged", nodes.is_some());
    json.push(',');
    json.push_str("\"viewport\":{");
    push_json_number_field(&mut json, "width", viewport.width as f64);
    json.push(',');
    push_json_number_field(&mut json, "height", viewport.height as f64);
    json.push('}');
    json.push(',');
    json.push_str("\"performance\":");
    if let Some(performance) = performance {
        serialize_performance(&mut json, performance);
    } else {
        json.push_str("null");
    }
    json.push(',');
    json.push_str("\"nodes\":");
    if let Some(nodes) = nodes {
        json.push('[');
        for (index, node) in nodes.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            serialize_node(&mut json, node);
        }
        json.push(']');
    } else {
        json.push_str("null");
    }
    json.push('}');
    json
}

fn semantics_hash(nodes: &[SemanticsNode]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    nodes.len().hash(&mut hasher);
    for node in nodes {
        hash_node(&mut hasher, node);
    }
    hasher.finish()
}

fn hash_node(hasher: &mut impl Hasher, node: &SemanticsNode) {
    node.id.get().hash(hasher);
    node.parent.map(|id| id.get()).hash(hasher);
    semantic_role_name(&node.role).hash(hasher);
    node.name.hash(hasher);
    node.description.hash(hasher);
    hash_value(hasher, node.value.as_ref());
    hash_state(hasher, &node.state);
    node.actions.len().hash(hasher);
    for action in &node.actions {
        semantic_action_name(action).hash(hasher);
    }
    hash_editable_text(hasher, node.editable_text.as_ref());
    hash_rect(hasher, node.bounds);
}

fn hash_value(hasher: &mut impl Hasher, value: Option<&SemanticsValue>) {
    match value {
        Some(SemanticsValue::Text(text)) => {
            1_u8.hash(hasher);
            text.hash(hasher);
        }
        Some(SemanticsValue::Number(number)) => {
            2_u8.hash(hasher);
            number.to_bits().hash(hasher);
        }
        Some(SemanticsValue::Range { value, min, max }) => {
            3_u8.hash(hasher);
            value.to_bits().hash(hasher);
            min.to_bits().hash(hasher);
            max.to_bits().hash(hasher);
        }
        None => 0_u8.hash(hasher),
    }
}

fn hash_state(hasher: &mut impl Hasher, state: &SemanticsState) {
    state.disabled.hash(hasher);
    state.focused.hash(hasher);
    state.hidden.hash(hasher);
    state.hovered.hash(hasher);
    match state.checked {
        Some(ToggleState::Unchecked) => 1_u8.hash(hasher),
        Some(ToggleState::Checked) => 2_u8.hash(hasher),
        Some(ToggleState::Mixed) => 3_u8.hash(hasher),
        None => 0_u8.hash(hasher),
    }
    state.selected.hash(hasher);
    state.expanded.hash(hasher);
    state.busy.hash(hasher);
}

fn hash_editable_text(hasher: &mut impl Hasher, editable: Option<&EditableTextSemantics>) {
    let Some(editable) = editable else {
        0_u8.hash(hasher);
        return;
    };
    1_u8.hash(hasher);
    editable.caret_offset.hash(hasher);
    editable.selection.start.hash(hasher);
    editable.selection.end.hash(hasher);
    editable.multiline.hash(hasher);
    editable.password.hash(hasher);
    editable.readonly.hash(hasher);
    editable.scroll_x.to_bits().hash(hasher);
    editable.scroll_y.to_bits().hash(hasher);
}

fn hash_rect(hasher: &mut impl Hasher, rect: Rect) {
    rect.x().to_bits().hash(hasher);
    rect.y().to_bits().hash(hasher);
    rect.width().to_bits().hash(hasher);
    rect.height().to_bits().hash(hasher);
}

fn serialize_performance(json: &mut String, performance: &WindowPerformanceSnapshot) {
    json.push('{');
    push_json_number_field(json, "frameIndex", performance.frame_index as f64);
    json.push(',');
    push_json_number_field(json, "totalTimeMs", performance.total_time_ms);
    json.push(',');
    push_json_number_field(
        json,
        "drawCount",
        performance.renderer_submission.draw_count as f64,
    );
    json.push(',');
    push_json_number_field(
        json,
        "visibleLayerCount",
        performance.renderer_submission.visible_layer_count as f64,
    );
    json.push(',');
    push_json_number_field(json, "commandCount", performance.scene.command_count as f64);
    json.push('}');
}

fn serialize_node(json: &mut String, node: &SemanticsNode) {
    json.push('{');
    push_json_number_field(json, "id", node.id.get() as f64);
    json.push(',');
    json.push_str("\"parent\":");
    match node.parent {
        Some(parent) => json.push_str(&parent.get().to_string()),
        None => json.push_str("null"),
    }
    json.push(',');
    push_json_string_field(json, "role", semantic_role_name(&node.role));
    json.push(',');
    json.push_str("\"dom\":");
    serialize_dom_descriptor(json, node);
    json.push(',');
    push_json_optional_string_field(json, "name", node.name.as_deref());
    json.push(',');
    push_json_optional_string_field(json, "description", node.description.as_deref());
    json.push(',');
    json.push_str("\"value\":");
    serialize_value(json, node.value.as_ref());
    json.push(',');
    json.push_str("\"state\":");
    serialize_state(json, &node.state);
    json.push(',');
    json.push_str("\"actions\":[");
    for (index, action) in node.actions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_json_string(json, semantic_action_name(action));
    }
    json.push(']');
    json.push(',');
    json.push_str("\"editableText\":");
    serialize_editable_text(json, node.editable_text.as_ref());
    json.push(',');
    json.push_str("\"bounds\":");
    serialize_rect(json, node.bounds);
    json.push('}');
}

fn serialize_dom_descriptor(json: &mut String, node: &SemanticsNode) {
    let tag = dom_tag_name(node);
    json.push('{');
    push_json_string_field(json, "tag", tag);
    json.push(',');
    json.push_str("\"role\":");
    if let Some(role) = dom_aria_role(node, tag) {
        push_json_string(json, role);
    } else {
        json.push_str("null");
    }
    json.push(',');
    push_json_bool_field(json, "interactive", interactive_node(node));
    if node
        .editable_text
        .as_ref()
        .is_some_and(|editable| editable.password)
    {
        json.push(',');
        push_json_string_field(json, "inputType", "password");
    }
    json.push('}');
}

fn dom_tag_name(node: &SemanticsNode) -> &'static str {
    match node.role {
        SemanticsRole::Button => "button",
        SemanticsRole::Link => "a",
        SemanticsRole::TextInput => {
            if node
                .editable_text
                .as_ref()
                .is_some_and(|editable| editable.multiline)
            {
                "textarea"
            } else {
                "input"
            }
        }
        SemanticsRole::CheckBox | SemanticsRole::RadioButton | SemanticsRole::Slider => "input",
        SemanticsRole::Switch => "button",
        SemanticsRole::Document => "article",
        SemanticsRole::Paragraph => "p",
        SemanticsRole::Heading => "h2",
        SemanticsRole::Code => "pre",
        SemanticsRole::Text => "span",
        _ => "div",
    }
}

fn dom_aria_role(node: &SemanticsNode, tag: &str) -> Option<&'static str> {
    if matches!(
        (&node.role, tag),
        (SemanticsRole::Button, "button")
            | (SemanticsRole::Link, "a")
            | (SemanticsRole::TextInput, _)
            | (SemanticsRole::CheckBox, "input")
            | (SemanticsRole::RadioButton, "input")
            | (SemanticsRole::Slider, "input")
    ) {
        return None;
    }

    match node.role {
        SemanticsRole::Window => Some("main"),
        SemanticsRole::Root => Some("group"),
        SemanticsRole::GenericContainer => Some("group"),
        SemanticsRole::Separator => Some("separator"),
        SemanticsRole::List => Some("list"),
        SemanticsRole::ListItem => Some("listitem"),
        SemanticsRole::Tree => Some("tree"),
        SemanticsRole::Table => Some("table"),
        SemanticsRole::Splitter => Some("separator"),
        SemanticsRole::Breadcrumb => Some("navigation"),
        SemanticsRole::TabBar => Some("tablist"),
        SemanticsRole::Tabs => Some("group"),
        SemanticsRole::Switch => Some("switch"),
        SemanticsRole::RadioGroup => Some("radiogroup"),
        SemanticsRole::Menu => Some("menu"),
        SemanticsRole::MenuItem => Some("menuitem"),
        SemanticsRole::ContextMenu => Some("menu"),
        SemanticsRole::Tooltip => Some("tooltip"),
        SemanticsRole::Dialog | SemanticsRole::Popover => Some("dialog"),
        SemanticsRole::ProgressBar => Some("progressbar"),
        SemanticsRole::BusyIndicator => Some("status"),
        SemanticsRole::Document => Some("document"),
        SemanticsRole::Paragraph => Some("paragraph"),
        SemanticsRole::Heading => Some("heading"),
        SemanticsRole::Code => Some("code"),
        SemanticsRole::Status => Some("status"),
        SemanticsRole::Attachment => Some("group"),
        SemanticsRole::Image | SemanticsRole::ColorSwatch | SemanticsRole::Canvas => Some("img"),
        SemanticsRole::ColorPicker => Some("button"),
        SemanticsRole::ScrollView => Some("region"),
        SemanticsRole::ComboBox => Some("combobox"),
        SemanticsRole::SpinBox => Some("spinbutton"),
        SemanticsRole::Button
        | SemanticsRole::Link
        | SemanticsRole::CheckBox
        | SemanticsRole::RadioButton
        | SemanticsRole::Slider
        | SemanticsRole::Text
        | SemanticsRole::TextInput => None,
    }
}

fn interactive_node(node: &SemanticsNode) -> bool {
    interactive_role(&node.role)
        || node.actions.iter().any(|action| {
            matches!(
                action,
                SemanticsAction::Activate
                    | SemanticsAction::SetValue
                    | SemanticsAction::SetSelection
                    | SemanticsAction::Increment
                    | SemanticsAction::Decrement
                    | SemanticsAction::InsertText
                    | SemanticsAction::Custom(_)
            )
        })
}

fn interactive_role(role: &SemanticsRole) -> bool {
    matches!(
        role,
        SemanticsRole::Button
            | SemanticsRole::Link
            | SemanticsRole::CheckBox
            | SemanticsRole::Switch
            | SemanticsRole::RadioButton
            | SemanticsRole::MenuItem
            | SemanticsRole::Slider
            | SemanticsRole::TextInput
            | SemanticsRole::SpinBox
            | SemanticsRole::ComboBox
            | SemanticsRole::ColorPicker
            | SemanticsRole::ScrollView
    )
}

fn serialize_value(json: &mut String, value: Option<&SemanticsValue>) {
    match value {
        Some(SemanticsValue::Text(text)) => {
            json.push_str("{\"kind\":\"text\",\"text\":");
            push_json_string(json, text);
            json.push('}');
        }
        Some(SemanticsValue::Number(number)) => {
            json.push_str("{\"kind\":\"number\",\"number\":");
            json.push_str(&json_number(*number));
            json.push('}');
        }
        Some(SemanticsValue::Range { value, min, max }) => {
            json.push_str("{\"kind\":\"range\",");
            push_json_number_field(json, "value", *value);
            json.push(',');
            push_json_number_field(json, "min", *min);
            json.push(',');
            push_json_number_field(json, "max", *max);
            json.push('}');
        }
        None => json.push_str("null"),
    }
}

fn serialize_state(json: &mut String, state: &SemanticsState) {
    json.push('{');
    push_json_bool_field(json, "disabled", state.disabled);
    json.push(',');
    push_json_bool_field(json, "focused", state.focused);
    json.push(',');
    push_json_bool_field(json, "hidden", state.hidden);
    json.push(',');
    push_json_bool_field(json, "hovered", state.hovered);
    json.push(',');
    json.push_str("\"checked\":");
    match state.checked {
        Some(ToggleState::Unchecked) => push_json_string(json, "unchecked"),
        Some(ToggleState::Checked) => push_json_string(json, "checked"),
        Some(ToggleState::Mixed) => push_json_string(json, "mixed"),
        None => json.push_str("null"),
    }
    json.push(',');
    push_json_bool_field(json, "selected", state.selected);
    json.push(',');
    json.push_str("\"expanded\":");
    match state.expanded {
        Some(expanded) => json.push_str(if expanded { "true" } else { "false" }),
        None => json.push_str("null"),
    }
    json.push(',');
    push_json_bool_field(json, "busy", state.busy);
    json.push('}');
}

fn serialize_editable_text(json: &mut String, editable: Option<&EditableTextSemantics>) {
    let Some(editable) = editable else {
        json.push_str("null");
        return;
    };
    json.push('{');
    push_json_number_field(json, "caretOffset", editable.caret_offset as f64);
    json.push(',');
    push_json_number_field(json, "selectionStart", editable.selection.start as f64);
    json.push(',');
    push_json_number_field(json, "selectionEnd", editable.selection.end as f64);
    json.push(',');
    push_json_bool_field(json, "multiline", editable.multiline);
    json.push(',');
    push_json_bool_field(json, "password", editable.password);
    json.push(',');
    push_json_bool_field(json, "readonly", editable.readonly);
    json.push(',');
    push_json_number_field(json, "scrollX", editable.scroll_x as f64);
    json.push(',');
    push_json_number_field(json, "scrollY", editable.scroll_y as f64);
    json.push('}');
}

fn serialize_rect(json: &mut String, rect: Rect) {
    json.push('{');
    push_json_number_field(json, "x", rect.x() as f64);
    json.push(',');
    push_json_number_field(json, "y", rect.y() as f64);
    json.push(',');
    push_json_number_field(json, "width", rect.width() as f64);
    json.push(',');
    push_json_number_field(json, "height", rect.height() as f64);
    json.push('}');
}

fn push_json_string_field(json: &mut String, field: &str, value: &str) {
    push_json_string(json, field);
    json.push(':');
    push_json_string(json, value);
}

fn push_json_optional_string_field(json: &mut String, field: &str, value: Option<&str>) {
    push_json_string(json, field);
    json.push(':');
    if let Some(value) = value {
        push_json_string(json, value);
    } else {
        json.push_str("null");
    }
}

fn push_json_number_field(json: &mut String, field: &str, value: f64) {
    push_json_string(json, field);
    json.push(':');
    json.push_str(&json_number(value));
}

fn push_json_bool_field(json: &mut String, field: &str, value: bool) {
    push_json_string(json, field);
    json.push(':');
    json.push_str(if value { "true" } else { "false" });
}

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            '\u{08}' => json.push_str("\\b"),
            '\u{0c}' => json.push_str("\\f"),
            ch if ch.is_control() => {
                json.push_str("\\u");
                json.push_str(&format!("{:04x}", ch as u32));
            }
            ch => json.push(ch),
        }
    }
    json.push('"');
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "0".to_string()
    }
}

fn semantic_role_name(role: &SemanticsRole) -> &'static str {
    match role {
        SemanticsRole::Window => "Window",
        SemanticsRole::Root => "Root",
        SemanticsRole::GenericContainer => "GenericContainer",
        SemanticsRole::Separator => "Separator",
        SemanticsRole::List => "List",
        SemanticsRole::ListItem => "ListItem",
        SemanticsRole::Tree => "Tree",
        SemanticsRole::Table => "Table",
        SemanticsRole::Splitter => "Splitter",
        SemanticsRole::Breadcrumb => "Breadcrumb",
        SemanticsRole::TabBar => "TabBar",
        SemanticsRole::Tabs => "Tabs",
        SemanticsRole::Button => "Button",
        SemanticsRole::Link => "Link",
        SemanticsRole::CheckBox => "CheckBox",
        SemanticsRole::Switch => "Switch",
        SemanticsRole::RadioButton => "RadioButton",
        SemanticsRole::RadioGroup => "RadioGroup",
        SemanticsRole::Menu => "Menu",
        SemanticsRole::MenuItem => "MenuItem",
        SemanticsRole::ContextMenu => "ContextMenu",
        SemanticsRole::Tooltip => "Tooltip",
        SemanticsRole::Dialog => "Dialog",
        SemanticsRole::Popover => "Popover",
        SemanticsRole::Slider => "Slider",
        SemanticsRole::ProgressBar => "ProgressBar",
        SemanticsRole::BusyIndicator => "BusyIndicator",
        SemanticsRole::Document => "Document",
        SemanticsRole::Paragraph => "Paragraph",
        SemanticsRole::Heading => "Heading",
        SemanticsRole::Code => "Code",
        SemanticsRole::Status => "Status",
        SemanticsRole::Attachment => "Attachment",
        SemanticsRole::Text => "Text",
        SemanticsRole::TextInput => "TextInput",
        SemanticsRole::SpinBox => "SpinBox",
        SemanticsRole::ComboBox => "ComboBox",
        SemanticsRole::Image => "Image",
        SemanticsRole::ColorSwatch => "ColorSwatch",
        SemanticsRole::ColorPicker => "ColorPicker",
        SemanticsRole::Canvas => "Canvas",
        SemanticsRole::ScrollView => "ScrollView",
    }
}

fn semantic_action_name(action: &SemanticsAction) -> &str {
    match action {
        SemanticsAction::Focus => "Focus",
        SemanticsAction::Blur => "Blur",
        SemanticsAction::Activate => "Activate",
        SemanticsAction::Expand => "Expand",
        SemanticsAction::Collapse => "Collapse",
        SemanticsAction::Increment => "Increment",
        SemanticsAction::Decrement => "Decrement",
        SemanticsAction::SetValue => "SetValue",
        SemanticsAction::SetSelection => "SetSelection",
        SemanticsAction::InsertText => "InsertText",
        SemanticsAction::DeleteBackward => "DeleteBackward",
        SemanticsAction::DeleteForward => "DeleteForward",
        SemanticsAction::Copy => "Copy",
        SemanticsAction::Cut => "Cut",
        SemanticsAction::Paste => "Paste",
        SemanticsAction::Undo => "Undo",
        SemanticsAction::Redo => "Redo",
        SemanticsAction::Custom(name) => name.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::{semantics_hash, serialize_snapshot};
    use sui_core::{
        EditableTextSemantics, Rect, SemanticsAction, SemanticsNode, SemanticsRole,
        SemanticsTextRange, SemanticsValue, Size, WidgetId, WindowId,
    };

    #[test]
    fn serializes_button_node_for_dom_mirror() {
        let mut node = SemanticsNode::new(
            WidgetId::new(7),
            SemanticsRole::Button,
            Rect::new(10.0, 20.0, 120.0, 32.0),
        );
        node.parent = Some(WidgetId::new(1));
        node.name = Some("Run \"demo\"".to_string());
        node.description = Some("Starts the selected demo\nnow".to_string());
        node.actions.push(SemanticsAction::Activate);
        node.state.focused = true;
        let nodes = [node];

        let json = serialize_snapshot(
            WindowId::new(3),
            42,
            2.0,
            Size::new(800.0, 600.0),
            Some(&nodes),
            None,
            semantics_hash(&nodes),
        );

        assert!(json.contains("\"windowId\":3"));
        assert!(json.contains("\"frameIndex\":42"));
        assert!(json.contains("\"devicePixelRatio\":2"));
        assert!(json.contains("\"semanticsHash\":"));
        assert!(json.contains("\"nodesChanged\":true"));
        assert!(json.contains("\"role\":\"Button\""));
        assert!(json.contains("\"dom\":{\"tag\":\"button\",\"role\":null,\"interactive\":true}"));
        assert!(json.contains("\"parent\":1"));
        assert!(json.contains("\"name\":\"Run \\\"demo\\\"\""));
        assert!(json.contains("\"description\":\"Starts the selected demo\\nnow\""));
        assert!(json.contains("\"actions\":[\"Activate\"]"));
        assert!(json.contains("\"focused\":true"));
        assert!(json.contains("\"bounds\":{\"x\":10,\"y\":20,\"width\":120,\"height\":32}"));
    }

    #[test]
    fn serializes_editable_multiline_text_for_textarea_mapping() {
        let mut node = SemanticsNode::new(
            WidgetId::new(8),
            SemanticsRole::TextInput,
            Rect::new(2.0, 4.0, 240.0, 80.0),
        );
        node.name = Some("Notes".to_string());
        node.value = Some(SemanticsValue::Text("hello".to_string()));
        node.editable_text = Some(EditableTextSemantics {
            caret_offset: 5,
            selection: SemanticsTextRange::new(1, 5),
            multiline: true,
            password: false,
            readonly: false,
            scroll_x: 0.0,
            scroll_y: 12.0,
        });
        node.actions.push(SemanticsAction::InsertText);
        let nodes = [node];

        let json = serialize_snapshot(
            WindowId::new(1),
            1,
            1.0,
            Size::new(320.0, 240.0),
            Some(&nodes),
            None,
            semantics_hash(&nodes),
        );

        assert!(json.contains("\"role\":\"TextInput\""));
        assert!(json.contains("\"dom\":{\"tag\":\"textarea\",\"role\":null,\"interactive\":true}"));
        assert!(json.contains("\"value\":{\"kind\":\"text\",\"text\":\"hello\"}"));
        assert!(json.contains("\"editableText\":{\"caretOffset\":5"));
        assert!(json.contains("\"selectionStart\":1"));
        assert!(json.contains("\"selectionEnd\":5"));
        assert!(json.contains("\"multiline\":true"));
        assert!(json.contains("\"password\":false"));
        assert!(json.contains("\"scrollY\":12"));
        assert!(json.contains("\"actions\":[\"InsertText\"]"));
    }

    #[test]
    fn serializes_password_editable_text_for_secure_dom_mapping() {
        let mut node = SemanticsNode::new(
            WidgetId::new(11),
            SemanticsRole::TextInput,
            Rect::new(2.0, 4.0, 240.0, 32.0),
        );
        node.name = Some("Password".to_string());
        node.value = Some(SemanticsValue::Text("secret".to_string()));
        node.editable_text = Some(EditableTextSemantics {
            caret_offset: 6,
            selection: SemanticsTextRange::new(6, 6),
            multiline: false,
            password: true,
            readonly: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
        let nodes = [node];

        let json = serialize_snapshot(
            WindowId::new(1),
            1,
            1.0,
            Size::new(320.0, 240.0),
            Some(&nodes),
            None,
            semantics_hash(&nodes),
        );

        assert!(json.contains("\"inputType\":\"password\""));
        assert!(json.contains("\"password\":true"));
    }

    #[test]
    fn serializes_link_and_switch_dom_mappings() {
        let mut link = SemanticsNode::new(
            WidgetId::new(9),
            SemanticsRole::Link,
            Rect::new(0.0, 0.0, 100.0, 20.0),
        );
        link.name = Some("Docs".to_string());

        let mut switch = SemanticsNode::new(
            WidgetId::new(10),
            SemanticsRole::Switch,
            Rect::new(0.0, 24.0, 48.0, 24.0),
        );
        switch.name = Some("Power".to_string());
        let nodes = [link, switch];

        let json = serialize_snapshot(
            WindowId::new(1),
            1,
            1.0,
            Size::new(320.0, 240.0),
            Some(&nodes),
            None,
            semantics_hash(&nodes),
        );

        assert!(json.contains("\"role\":\"Link\""));
        assert!(json.contains("\"dom\":{\"tag\":\"a\",\"role\":null,\"interactive\":true}"));
        assert!(json.contains("\"role\":\"Switch\""));
        assert!(
            json.contains("\"dom\":{\"tag\":\"button\",\"role\":\"switch\",\"interactive\":true}")
        );
    }

    #[test]
    fn serializes_list_item_dom_mapping_and_action_interactivity() {
        let mut row = SemanticsNode::new(
            WidgetId::new(11),
            SemanticsRole::ListItem,
            Rect::new(8.0, 12.0, 160.0, 32.0),
        );
        row.parent = Some(WidgetId::new(3));
        row.name = Some("Paint".to_string());
        row.description = Some("Normal / 100%".to_string());
        row.value = Some(SemanticsValue::Text("Normal / 100%".to_string()));
        row.state.selected = true;
        row.actions.push(SemanticsAction::Activate);
        let nodes = [row];

        let json = serialize_snapshot(
            WindowId::new(1),
            1,
            1.0,
            Size::new(320.0, 240.0),
            Some(&nodes),
            None,
            semantics_hash(&nodes),
        );

        assert!(json.contains("\"role\":\"ListItem\""));
        assert!(
            json.contains("\"dom\":{\"tag\":\"div\",\"role\":\"listitem\",\"interactive\":true}")
        );
        assert!(json.contains("\"description\":\"Normal / 100%\""));
        assert!(json.contains("\"selected\":true"));
        assert!(json.contains("\"actions\":[\"Activate\"]"));
    }

    #[test]
    fn serializes_reused_nodes_as_lightweight_frame_update() {
        let json = serialize_snapshot(
            WindowId::new(1),
            2,
            1.0,
            Size::new(320.0, 240.0),
            None,
            None,
            0x1234,
        );

        assert!(json.contains("\"semanticsHash\":\"0000000000001234\""));
        assert!(json.contains("\"nodesChanged\":false"));
        assert!(json.contains("\"nodes\":null"));
    }
}
