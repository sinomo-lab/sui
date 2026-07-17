use std::collections::{HashMap, HashSet};

use accesskit::{
    Action, ActionData, ActionRequest, CustomAction, Live, Node, NodeId, Rect, Role, TextPosition,
    TextSelection, Toggled, Tree, TreeId, TreeUpdate,
};
use sui_core::{
    SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsTextRange,
    SemanticsValue, ToggleState, WidgetId, WindowId,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
pub(crate) struct AccessKitSnapshot {
    update: TreeUpdate,
    node_to_widget: HashMap<NodeId, WidgetId>,
    values: HashMap<NodeId, SemanticsValue>,
    text_offsets: HashMap<NodeId, Vec<usize>>,
    text_run_ids: HashMap<NodeId, NodeId>,
    custom_actions: HashMap<(NodeId, i32), SemanticsActionRequest>,
}

impl AccessKitSnapshot {
    pub(crate) fn full_update(&self) -> TreeUpdate {
        self.update.clone()
    }

    pub(crate) fn map_action(
        &self,
        request: &ActionRequest,
    ) -> Option<(WidgetId, SemanticsActionRequest)> {
        if request.target_tree != TreeId::ROOT {
            return None;
        }

        let widget_id = *self.node_to_widget.get(&request.target_node)?;
        let control_node_id = NodeId(widget_id.get());
        let action = match request.action {
            Action::Click => SemanticsActionRequest::Activate,
            Action::Focus => SemanticsActionRequest::Focus,
            Action::Blur => SemanticsActionRequest::Blur,
            Action::Collapse => SemanticsActionRequest::Collapse,
            Action::Expand => SemanticsActionRequest::Expand,
            Action::Decrement => SemanticsActionRequest::Decrement,
            Action::Increment => SemanticsActionRequest::Increment,
            Action::ReplaceSelectedText => match request.data.as_ref()? {
                ActionData::Value(value) => SemanticsActionRequest::InsertText(value.to_string()),
                _ => return None,
            },
            Action::SetTextSelection => {
                let ActionData::SetTextSelection(selection) = request.data.as_ref()? else {
                    return None;
                };
                let text_run_id = *self.text_run_ids.get(&control_node_id)?;
                if selection.anchor.node != text_run_id || selection.focus.node != text_run_id {
                    return None;
                }
                let offsets = self.text_offsets.get(&control_node_id)?;
                let anchor =
                    character_index_to_byte_offset(offsets, selection.anchor.character_index)?;
                let focus =
                    character_index_to_byte_offset(offsets, selection.focus.character_index)?;
                SemanticsActionRequest::SetSelection(SemanticsTextRange::new(anchor, focus))
            }
            Action::SetValue => match request.data.as_ref()? {
                ActionData::Value(value) => {
                    SemanticsActionRequest::SetValue(self.value_from_text(control_node_id, value)?)
                }
                ActionData::NumericValue(value) => SemanticsActionRequest::SetValue(
                    self.value_from_number(control_node_id, *value),
                ),
                _ => return None,
            },
            Action::CustomAction => {
                let ActionData::CustomAction(id) = request.data.as_ref()? else {
                    return None;
                };
                self.custom_actions.get(&(control_node_id, *id))?.clone()
            }
            _ => return None,
        };

        Some((widget_id, action))
    }

    fn value_from_text(&self, node_id: NodeId, value: &str) -> Option<SemanticsValue> {
        match self.values.get(&node_id) {
            Some(SemanticsValue::Number(_)) => value.parse().ok().map(SemanticsValue::Number),
            Some(SemanticsValue::Range { min, max, .. }) => {
                value.parse().ok().map(|value| SemanticsValue::Range {
                    value,
                    min: *min,
                    max: *max,
                })
            }
            _ => Some(SemanticsValue::Text(value.to_string())),
        }
    }

    fn value_from_number(&self, node_id: NodeId, value: f64) -> SemanticsValue {
        match self.values.get(&node_id) {
            Some(SemanticsValue::Range { min, max, .. }) => SemanticsValue::Range {
                value,
                min: *min,
                max: *max,
            },
            _ => SemanticsValue::Number(value),
        }
    }
}

pub(crate) fn build_accesskit_snapshot(
    window_id: WindowId,
    scale_factor: f64,
    title: &str,
    nodes: &[SemanticsNode],
) -> AccessKitSnapshot {
    let scale_factor = valid_scale_factor(scale_factor);
    let mut order = Vec::with_capacity(nodes.len());
    let mut by_id = HashMap::with_capacity(nodes.len());
    for node in nodes {
        if !by_id.contains_key(&node.id) {
            order.push(node.id);
        }
        by_id.insert(node.id, node);
    }

    let parents: HashMap<_, _> = order
        .iter()
        .copied()
        .map(|id| (id, normalized_parent(id, &by_id)))
        .collect();
    let mut children: HashMap<WidgetId, Vec<WidgetId>> = HashMap::new();
    let mut roots = Vec::new();
    for id in order.iter().copied() {
        if let Some(parent) = parents.get(&id).copied().flatten() {
            children.entry(parent).or_default().push(id);
        } else {
            roots.push(id);
        }
    }

    let use_semantic_root = roots.len() == 1
        && by_id
            .get(&roots[0])
            .is_some_and(|node| matches!(node.role, SemanticsRole::Window | SemanticsRole::Root));
    let mut occupied: HashSet<_> = order.iter().map(|id| NodeId(id.get())).collect();
    let tree_root = if use_semantic_root {
        NodeId(roots[0].get())
    } else {
        synthetic_root_id(window_id, &occupied)
    };
    occupied.insert(tree_root);
    let mut text_run_ids = HashMap::new();
    for id in order.iter().copied() {
        if by_id[&id].editable_text.is_some() {
            let text_run_id = synthetic_text_run_id(window_id, id, &occupied);
            occupied.insert(text_run_id);
            text_run_ids.insert(id, text_run_id);
        }
    }

    let mut accesskit_nodes =
        Vec::with_capacity(order.len() + text_run_ids.len() + usize::from(!use_semantic_root));
    let mut node_to_widget = HashMap::with_capacity(order.len() + text_run_ids.len());
    let mut values = HashMap::new();
    let mut text_offsets = HashMap::new();
    let mut control_text_run_ids = HashMap::new();
    let mut custom_actions = HashMap::new();

    for id in order.iter().copied() {
        let source = by_id[&id];
        let node_id = NodeId(id.get());
        let mut child_ids = children
            .get(&id)
            .into_iter()
            .flatten()
            .map(|child| NodeId(child.get()))
            .collect::<Vec<_>>();
        let text_run_id = text_run_ids.get(&id).copied();
        if let Some(text_run_id) = text_run_id {
            child_ids.push(text_run_id);
        }
        let mut mapped = map_semantics_node(
            node_id,
            text_run_id,
            source,
            scale_factor,
            &mut text_offsets,
            &mut custom_actions,
        );
        mapped.set_children(child_ids);
        if use_semantic_root
            && node_id == tree_root
            && source.name.as_deref().unwrap_or_default().is_empty()
            && !title.is_empty()
        {
            mapped.set_label(title);
        }
        if let Some(value) = source.value.clone() {
            values.insert(node_id, sanitize_semantics_value(value));
        }
        node_to_widget.insert(node_id, id);
        accesskit_nodes.push((node_id, mapped));
        if let Some(text_run_id) = text_run_id {
            control_text_run_ids.insert(node_id, text_run_id);
            node_to_widget.insert(text_run_id, id);
            accesskit_nodes.push((text_run_id, map_editable_text_run(source, scale_factor)));
        }
    }

    if !use_semantic_root {
        let mut root = Node::new(Role::Window);
        root.set_label(if title.is_empty() {
            "SUI window"
        } else {
            title
        });
        root.set_children(roots.iter().map(|id| NodeId(id.get())).collect::<Vec<_>>());
        root.set_bounds(tree_bounds(nodes, scale_factor));
        accesskit_nodes.push((tree_root, root));
    }

    let focus = order
        .iter()
        .copied()
        .filter(|id| {
            by_id.get(id).is_some_and(|node| {
                node.state.focused && !node.state.hidden && !node.state.disabled
            })
        })
        .max_by_key(|id| semantic_depth(*id, &parents))
        .map(|id| NodeId(id.get()))
        .unwrap_or(tree_root);
    let mut tree = Tree::new(tree_root);
    tree.toolkit_name = Some("SUI".to_string());
    tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_string());

    AccessKitSnapshot {
        update: TreeUpdate {
            nodes: accesskit_nodes,
            tree: Some(tree),
            tree_id: TreeId::ROOT,
            focus,
        },
        node_to_widget,
        values,
        text_offsets,
        text_run_ids: control_text_run_ids,
        custom_actions,
    }
}

fn semantic_depth(id: WidgetId, parents: &HashMap<WidgetId, Option<WidgetId>>) -> usize {
    let mut depth = 0;
    let mut current = id;
    while let Some(parent) = parents.get(&current).copied().flatten() {
        depth += 1;
        current = parent;
    }
    depth
}

fn normalized_parent(id: WidgetId, nodes: &HashMap<WidgetId, &SemanticsNode>) -> Option<WidgetId> {
    let parent = nodes.get(&id)?.parent?;
    if parent == id || !nodes.contains_key(&parent) {
        return None;
    }

    let mut current = parent;
    let mut visited = HashSet::new();
    loop {
        if current == id || !visited.insert(current) {
            return None;
        }
        match nodes.get(&current).and_then(|node| node.parent) {
            Some(next) if next == current => return None,
            Some(next) if nodes.contains_key(&next) => current = next,
            _ => return Some(parent),
        }
    }
}

fn synthetic_root_id(window_id: WindowId, occupied: &HashSet<NodeId>) -> NodeId {
    allocate_synthetic_id(u64::MAX ^ window_id.get().rotate_left(17), occupied)
}

fn synthetic_text_run_id(
    window_id: WindowId,
    widget_id: WidgetId,
    occupied: &HashSet<NodeId>,
) -> NodeId {
    let seed = 0xa11c_e551_b1e0_0000_u64
        ^ window_id.get().rotate_left(11)
        ^ widget_id.get().rotate_left(29);
    allocate_synthetic_id(seed, occupied)
}

fn allocate_synthetic_id(mut raw: u64, occupied: &HashSet<NodeId>) -> NodeId {
    while occupied.contains(&NodeId(raw)) {
        raw = raw.wrapping_sub(1);
    }
    NodeId(raw)
}

fn map_semantics_node(
    node_id: NodeId,
    text_run_id: Option<NodeId>,
    source: &SemanticsNode,
    scale_factor: f64,
    text_offsets: &mut HashMap<NodeId, Vec<usize>>,
    custom_actions: &mut HashMap<(NodeId, i32), SemanticsActionRequest>,
) -> Node {
    let editable = source.editable_text.as_ref();
    let multiline = editable.is_some_and(|editable| editable.multiline);
    let password = editable.is_some_and(|editable| editable.password);
    let mut node = Node::new(map_role(&source.role, multiline, password));
    node.set_bounds(scaled_bounds(source.bounds, scale_factor));

    if let Some(name) = source.name.as_deref().filter(|name| !name.is_empty()) {
        node.set_label(name);
    }
    if let Some(description) = source
        .description
        .as_deref()
        .filter(|description| !description.is_empty())
    {
        node.set_description(description);
    }
    match source.value.as_ref() {
        Some(SemanticsValue::Text(value)) => node.set_value(value.as_str()),
        Some(SemanticsValue::Number(value)) if value.is_finite() => node.set_numeric_value(*value),
        Some(SemanticsValue::Number(_)) => {}
        Some(SemanticsValue::Range { value, min, max }) => {
            let (value, min, max) = sanitize_range(*value, *min, *max);
            node.set_numeric_value(value);
            node.set_min_numeric_value(min);
            node.set_max_numeric_value(max);
        }
        None => {}
    }
    if let Some(step) = source
        .numeric_step
        .filter(|step| step.is_finite() && *step > 0.0)
    {
        node.set_numeric_value_step(step);
    }

    if source.state.disabled {
        node.set_disabled();
    }
    if source.state.hidden {
        node.set_hidden();
    }
    if source.state.busy {
        node.set_busy();
        node.set_live(Live::Polite);
    }
    if source.state.selected {
        node.set_selected(true);
    }
    if let Some(expanded) = source.state.expanded {
        node.set_expanded(expanded);
    }
    if let Some(checked) = source.state.checked {
        node.set_toggled(match checked {
            ToggleState::Unchecked => Toggled::False,
            ToggleState::Checked => Toggled::True,
            ToggleState::Mixed => Toggled::Mixed,
        });
    }
    if matches!(
        source.role,
        SemanticsRole::ProgressBar | SemanticsRole::BusyIndicator
    ) {
        node.set_live(Live::Polite);
    }

    if let Some(editable) = &source.editable_text {
        if editable.readonly {
            node.set_read_only();
        }
        if editable.scroll_x.is_finite() {
            node.set_scroll_x(editable.scroll_x.into());
        }
        if editable.scroll_y.is_finite() {
            node.set_scroll_y(editable.scroll_y.into());
        }
        let text = match source.value.as_ref() {
            Some(SemanticsValue::Text(value)) => value.as_str(),
            _ => "",
        };
        let offsets = utf8_character_offsets(text);
        let text_run_id = text_run_id.expect("editable semantic nodes have a text run");
        let (anchor, focus) = if editable.selection.start != editable.selection.end
            && editable.caret_offset == editable.selection.start
        {
            (editable.selection.end, editable.selection.start)
        } else {
            (editable.selection.start, editable.selection.end)
        };
        node.set_text_selection(TextSelection {
            anchor: TextPosition {
                node: text_run_id,
                character_index: byte_offset_to_character_index(&offsets, anchor),
            },
            focus: TextPosition {
                node: text_run_id,
                character_index: byte_offset_to_character_index(&offsets, focus),
            },
        });
        text_offsets.insert(node_id, offsets);
    }

    let mut custom_requests = Vec::new();
    for action in &source.actions {
        match action {
            SemanticsAction::Focus => node.add_action(Action::Focus),
            SemanticsAction::Blur => node.add_action(Action::Blur),
            SemanticsAction::Activate => node.add_action(Action::Click),
            SemanticsAction::Expand => node.add_action(Action::Expand),
            SemanticsAction::Collapse => node.add_action(Action::Collapse),
            SemanticsAction::Increment => node.add_action(Action::Increment),
            SemanticsAction::Decrement => node.add_action(Action::Decrement),
            SemanticsAction::SetValue => node.add_action(Action::SetValue),
            SemanticsAction::SetSelection if text_run_id.is_some() => {
                node.add_action(Action::SetTextSelection)
            }
            SemanticsAction::SetSelection => {}
            SemanticsAction::InsertText => node.add_action(Action::ReplaceSelectedText),
            SemanticsAction::Custom(name) => {
                custom_requests.push((
                    format!("custom:{name}"),
                    name.clone(),
                    SemanticsActionRequest::Custom {
                        name: name.clone(),
                        value: None,
                    },
                ));
            }
            SemanticsAction::DeleteBackward => custom_requests.push((
                "edit:delete-backward".to_string(),
                "Delete backward".to_string(),
                SemanticsActionRequest::DeleteBackward,
            )),
            SemanticsAction::DeleteForward => custom_requests.push((
                "edit:delete-forward".to_string(),
                "Delete forward".to_string(),
                SemanticsActionRequest::DeleteForward,
            )),
            SemanticsAction::Copy => custom_requests.push((
                "edit:copy".to_string(),
                "Copy".to_string(),
                SemanticsActionRequest::Copy,
            )),
            SemanticsAction::Cut => custom_requests.push((
                "edit:cut".to_string(),
                "Cut".to_string(),
                SemanticsActionRequest::Cut,
            )),
            SemanticsAction::Paste => custom_requests.push((
                "edit:paste".to_string(),
                "Paste".to_string(),
                SemanticsActionRequest::Paste,
            )),
            SemanticsAction::Undo => custom_requests.push((
                "edit:undo".to_string(),
                "Undo".to_string(),
                SemanticsActionRequest::Undo,
            )),
            SemanticsAction::Redo => custom_requests.push((
                "edit:redo".to_string(),
                "Redo".to_string(),
                SemanticsActionRequest::Redo,
            )),
        }
    }

    // Assign IDs from action identity rather than frame-local ordering. Sorting
    // also makes the collision-probing result stable when widget code reorders
    // its advertised actions.
    custom_requests.sort_by(|left, right| left.0.cmp(&right.0));
    custom_requests.dedup_by(|left, right| left.0 == right.0);
    let mut occupied_custom_ids = HashSet::new();
    for (identity, description, request) in custom_requests {
        let mut id = stable_custom_action_id(&identity);
        while !occupied_custom_ids.insert(id) {
            id = id.wrapping_add(1);
        }
        node.add_action(Action::CustomAction);
        node.push_custom_action(CustomAction {
            id,
            description: description.into_boxed_str(),
        });
        custom_actions.insert((node_id, id), request);
    }

    node
}

fn stable_custom_action_id(identity: &str) -> i32 {
    // FNV-1a is deliberately specified here instead of relying on
    // `DefaultHasher`, whose output is not a stable API.
    let mut hash = 0x811c_9dc5_u32;
    for byte in identity.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    i32::from_ne_bytes(hash.to_ne_bytes())
}

fn map_editable_text_run(source: &SemanticsNode, scale_factor: f64) -> Node {
    let text = match source.value.as_ref() {
        Some(SemanticsValue::Text(value)) => value.as_str(),
        _ => "",
    };
    let offsets = utf8_character_offsets(text);
    let lengths = offsets
        .windows(2)
        .map(|pair| {
            u8::try_from(pair[1] - pair[0])
                .expect("text character chunks are limited to 255 UTF-8 bytes")
        })
        .collect::<Vec<_>>();
    let mut node = Node::new(Role::TextRun);
    node.set_value(text);
    node.set_character_lengths(lengths);
    node.set_bounds(scaled_bounds(source.bounds, scale_factor));
    node
}

fn map_role(role: &SemanticsRole, multiline: bool, password: bool) -> Role {
    match role {
        SemanticsRole::Window | SemanticsRole::Root => Role::Window,
        SemanticsRole::GenericContainer => Role::GenericContainer,
        SemanticsRole::Separator | SemanticsRole::Splitter => Role::Splitter,
        SemanticsRole::List => Role::List,
        SemanticsRole::ListItem => Role::ListItem,
        SemanticsRole::Tree => Role::Tree,
        SemanticsRole::Table => Role::Table,
        SemanticsRole::Breadcrumb => Role::Navigation,
        SemanticsRole::TabBar => Role::TabList,
        SemanticsRole::Tabs => Role::TabPanel,
        SemanticsRole::Button => Role::Button,
        SemanticsRole::Link => Role::Link,
        SemanticsRole::CheckBox => Role::CheckBox,
        SemanticsRole::Switch => Role::Switch,
        SemanticsRole::RadioButton => Role::RadioButton,
        SemanticsRole::RadioGroup => Role::RadioGroup,
        SemanticsRole::Menu | SemanticsRole::ContextMenu => Role::Menu,
        SemanticsRole::MenuItem => Role::MenuItem,
        SemanticsRole::Tooltip => Role::Tooltip,
        SemanticsRole::Dialog | SemanticsRole::Popover => Role::Dialog,
        SemanticsRole::Slider => Role::Slider,
        SemanticsRole::ProgressBar => Role::ProgressIndicator,
        SemanticsRole::BusyIndicator => Role::Status,
        SemanticsRole::Document => Role::Document,
        SemanticsRole::Paragraph => Role::Paragraph,
        SemanticsRole::Heading => Role::Heading,
        SemanticsRole::Code => Role::Code,
        SemanticsRole::Status => Role::Status,
        SemanticsRole::Attachment => Role::Group,
        SemanticsRole::Text => Role::Label,
        SemanticsRole::TextInput if password => Role::PasswordInput,
        SemanticsRole::TextInput if multiline => Role::MultilineTextInput,
        SemanticsRole::TextInput => Role::TextInput,
        SemanticsRole::SpinBox => Role::SpinButton,
        SemanticsRole::ComboBox => Role::ComboBox,
        SemanticsRole::Image => Role::Image,
        SemanticsRole::ColorSwatch | SemanticsRole::ColorPicker => Role::ColorWell,
        SemanticsRole::Canvas => Role::Canvas,
        SemanticsRole::ScrollView => Role::ScrollView,
    }
}

fn valid_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn sanitize_semantics_value(value: SemanticsValue) -> SemanticsValue {
    match value {
        SemanticsValue::Range { value, min, max } => {
            let (value, min, max) = sanitize_range(value, min, max);
            SemanticsValue::Range { value, min, max }
        }
        SemanticsValue::Number(value) if !value.is_finite() => SemanticsValue::Number(0.0),
        value => value,
    }
}

fn sanitize_range(value: f64, min: f64, max: f64) -> (f64, f64, f64) {
    let finite_value = if value.is_finite() { value } else { 0.0 };
    let min = if min.is_finite() {
        min
    } else if min == f64::NEG_INFINITY {
        f64::MIN
    } else {
        finite_value
    };
    let max = if max.is_finite() {
        max
    } else if max == f64::INFINITY {
        f64::MAX
    } else {
        finite_value
    };
    let (min, max) = if min <= max { (min, max) } else { (max, min) };
    (finite_value.clamp(min, max), min, max)
}

fn scaled_bounds(bounds: sui_core::Rect, scale_factor: f64) -> Rect {
    let x = finite_f32(bounds.x()) * scale_factor;
    let y = finite_f32(bounds.y()) * scale_factor;
    let width = finite_f32(bounds.width()).max(0.0) * scale_factor;
    let height = finite_f32(bounds.height()).max(0.0) * scale_factor;
    Rect::new(x, y, x + width, y + height)
}

fn finite_f32(value: f32) -> f64 {
    if value.is_finite() { value.into() } else { 0.0 }
}

fn tree_bounds(nodes: &[SemanticsNode], scale_factor: f64) -> Rect {
    let mut bounds = nodes
        .iter()
        .map(|node| scaled_bounds(node.bounds, scale_factor));
    let Some(first) = bounds.next() else {
        return Rect::ZERO;
    };
    bounds.fold(first, |combined, bounds| combined.union(bounds))
}

fn utf8_character_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (grapheme_start, grapheme) in text.grapheme_indices(true) {
        let mut chunk_start = grapheme_start;
        for (relative_offset, character) in grapheme.char_indices() {
            let character_start = grapheme_start + relative_offset;
            let character_end = character_start + character.len_utf8();
            if character_end - chunk_start > usize::from(u8::MAX) {
                debug_assert!(character_start > chunk_start);
                offsets.push(character_start);
                chunk_start = character_start;
            }
        }
        let grapheme_end = grapheme_start + grapheme.len();
        if offsets.last().copied() != Some(grapheme_end) {
            offsets.push(grapheme_end);
        }
    }
    if offsets.last().copied() != Some(text.len()) {
        offsets.push(text.len());
    }
    offsets
}

fn byte_offset_to_character_index(offsets: &[usize], byte_offset: usize) -> usize {
    match offsets.binary_search(&byte_offset) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

fn character_index_to_byte_offset(offsets: &[usize], character_index: usize) -> Option<usize> {
    offsets.get(character_index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::ActionData;
    use sui_core::{EditableTextSemantics, Rect as SuiRect, SemanticsState};

    fn node(id: u64, role: SemanticsRole) -> SemanticsNode {
        SemanticsNode::new(WidgetId::new(id), role, SuiRect::new(1.0, 2.0, 30.0, 40.0))
    }

    fn mapped_node(snapshot: &AccessKitSnapshot, id: u64) -> &Node {
        mapped_accesskit_node(snapshot, NodeId(id))
    }

    fn mapped_accesskit_node(snapshot: &AccessKitSnapshot, id: NodeId) -> &Node {
        &snapshot
            .update
            .nodes
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .expect("mapped node should exist")
            .1
    }

    #[test]
    fn empty_snapshot_has_named_window_root() {
        let snapshot = build_accesskit_snapshot(WindowId::new(7), 1.0, "Sinomo", &[]);
        let update = snapshot.full_update();
        let tree = update.tree.expect("full update should initialize tree");
        assert_eq!(update.tree_id, TreeId::ROOT);
        assert_eq!(update.focus, tree.root);
        assert_eq!(update.nodes.len(), 1);
        let root = &update.nodes[0].1;
        assert_eq!(root.role(), Role::Window);
        assert_eq!(root.label(), Some("Sinomo"));
    }

    #[test]
    fn hierarchy_state_bounds_and_basic_action_are_preserved() {
        let root = node(1, SemanticsRole::Root);
        let mut button = node(2, SemanticsRole::Button);
        button.parent = Some(root.id);
        button.name = Some("Join another cluster".into());
        button.description = Some("Preview migration impact".into());
        button.state = SemanticsState {
            focused: true,
            selected: true,
            ..SemanticsState::default()
        };
        button.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];

        let snapshot = build_accesskit_snapshot(WindowId::new(1), 2.0, "Settings", &[root, button]);
        let update = snapshot.full_update();
        assert_eq!(update.tree.as_ref().unwrap().root, NodeId(1));
        assert_eq!(update.focus, NodeId(2));
        assert_eq!(mapped_node(&snapshot, 1).children(), &[NodeId(2)]);
        let button = mapped_node(&snapshot, 2);
        assert_eq!(button.role(), Role::Button);
        assert_eq!(button.label(), Some("Join another cluster"));
        assert_eq!(button.bounds(), Some(Rect::new(2.0, 4.0, 62.0, 84.0)));
        assert!(button.supports_action(Action::Focus));
        assert!(button.supports_action(Action::Click));
        assert_eq!(button.is_selected(), Some(true));

        let request = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(2),
            data: None,
        };
        assert_eq!(
            snapshot.map_action(&request),
            Some((WidgetId::new(2), SemanticsActionRequest::Activate))
        );
    }

    #[test]
    fn multiple_roots_are_wrapped_by_stable_window_root() {
        let left = node(11, SemanticsRole::List);
        let right = node(12, SemanticsRole::List);
        let first = build_accesskit_snapshot(
            WindowId::new(4),
            1.0,
            "Cluster",
            &[left.clone(), right.clone()],
        );
        let second = build_accesskit_snapshot(WindowId::new(4), 1.0, "Cluster", &[left, right]);
        let first_update = first.full_update();
        let second_update = second.full_update();
        let root_id = first_update.tree.as_ref().unwrap().root;
        assert_eq!(root_id, second_update.tree.as_ref().unwrap().root);
        assert_ne!(root_id, NodeId(11));
        assert_ne!(root_id, NodeId(12));
        let root = first_update
            .nodes
            .iter()
            .find(|(id, _)| *id == root_id)
            .unwrap();
        assert_eq!(root.1.role(), Role::Window);
        assert_eq!(root.1.children(), &[NodeId(11), NodeId(12)]);
    }

    #[test]
    fn editable_text_selection_round_trips_utf8_offsets() {
        let mut input = node(20, SemanticsRole::TextInput);
        input.value = Some(SemanticsValue::Text("á👩‍💻".into()));
        input.actions = vec![
            SemanticsAction::SetValue,
            SemanticsAction::SetSelection,
            SemanticsAction::InsertText,
        ];
        input.editable_text = Some(EditableTextSemantics {
            caret_offset: 14,
            selection: SemanticsTextRange::new(3, 14),
            multiline: false,
            password: false,
            readonly: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Editor", &[input]);
        let input = mapped_node(&snapshot, 20);
        let text_run_id = input.children()[0];
        let text_run = mapped_accesskit_node(&snapshot, text_run_id);
        assert_eq!(text_run.role(), Role::TextRun);
        assert_eq!(text_run.value(), Some("á👩‍💻"));
        assert_eq!(text_run.character_lengths(), &[3, 11]);
        assert_eq!(
            snapshot.node_to_widget.get(&text_run_id),
            Some(&WidgetId::new(20))
        );
        let consumer_tree = accesskit_consumer::Tree::new(snapshot.full_update(), true);
        assert!(
            consumer_tree
                .state()
                .node_by_tree_local_id(NodeId(20), TreeId::ROOT)
                .unwrap()
                .supports_text_ranges()
        );
        assert_eq!(
            input.text_selection(),
            Some(&TextSelection {
                anchor: TextPosition {
                    node: text_run_id,
                    character_index: 1,
                },
                focus: TextPosition {
                    node: text_run_id,
                    character_index: 2,
                },
            })
        );

        let request = ActionRequest {
            action: Action::SetTextSelection,
            target_tree: TreeId::ROOT,
            target_node: NodeId(20),
            data: Some(ActionData::SetTextSelection(TextSelection {
                anchor: TextPosition {
                    node: text_run_id,
                    character_index: 0,
                },
                focus: TextPosition {
                    node: text_run_id,
                    character_index: 2,
                },
            })),
        };
        assert_eq!(
            snapshot.map_action(&request),
            Some((
                WidgetId::new(20),
                SemanticsActionRequest::SetSelection(SemanticsTextRange::new(0, 14)),
            ))
        );
    }

    #[test]
    fn password_editable_text_maps_to_secure_accesskit_role() {
        let mut input = node(21, SemanticsRole::TextInput);
        input.value = Some(SemanticsValue::Text("secret".into()));
        input.editable_text = Some(EditableTextSemantics {
            caret_offset: 6,
            selection: SemanticsTextRange::new(6, 6),
            multiline: false,
            password: true,
            readonly: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });

        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Login", &[input]);
        assert_eq!(mapped_node(&snapshot, 21).role(), Role::PasswordInput);
    }

    #[test]
    fn range_and_custom_actions_keep_typed_payloads() {
        let mut slider = node(30, SemanticsRole::Slider);
        slider.value = Some(SemanticsValue::Range {
            value: 2.0,
            min: 0.0,
            max: 10.0,
        });
        slider.actions = vec![
            SemanticsAction::Increment,
            SemanticsAction::SetValue,
            SemanticsAction::Custom("Reset".into()),
        ];
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Range", &[slider]);
        let node = mapped_node(&snapshot, 30);
        assert_eq!(node.numeric_value(), Some(2.0));
        assert_eq!(node.min_numeric_value(), Some(0.0));
        assert_eq!(node.max_numeric_value(), Some(10.0));
        assert_eq!(node.custom_actions()[0].description.as_ref(), "Reset");

        let set_value = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: NodeId(30),
            data: Some(ActionData::NumericValue(7.0)),
        };
        assert_eq!(
            snapshot.map_action(&set_value),
            Some((
                WidgetId::new(30),
                SemanticsActionRequest::SetValue(SemanticsValue::Range {
                    value: 7.0,
                    min: 0.0,
                    max: 10.0,
                }),
            ))
        );

        let custom = ActionRequest {
            action: Action::CustomAction,
            target_tree: TreeId::ROOT,
            target_node: NodeId(30),
            data: Some(ActionData::CustomAction(node.custom_actions()[0].id)),
        };
        assert_eq!(
            snapshot.map_action(&custom),
            Some((
                WidgetId::new(30),
                SemanticsActionRequest::Custom {
                    name: "Reset".into(),
                    value: None,
                },
            ))
        );
    }

    #[test]
    fn busy_progress_is_exposed_as_a_polite_live_region() {
        let mut progress = node(35, SemanticsRole::BusyIndicator);
        progress.name = Some("Evolution refresh".into());
        progress.value = Some(SemanticsValue::Text("Loading candidate queue".into()));
        progress.state.busy = true;
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Evolution", &[progress]);
        let progress = mapped_node(&snapshot, 35);
        assert_eq!(progress.role(), Role::Status);
        assert_eq!(progress.live(), Some(Live::Polite));
        assert!(progress.is_busy());
        assert_eq!(progress.value(), Some("Loading candidate queue"));
    }

    #[test]
    fn text_selection_rejects_wrong_runs_and_stale_indices() {
        let mut input = node(36, SemanticsRole::TextInput);
        input.value = Some(SemanticsValue::Text("hello".into()));
        input.actions = vec![SemanticsAction::SetSelection];
        input.editable_text = Some(EditableTextSemantics {
            caret_offset: 5,
            selection: SemanticsTextRange::new(5, 5),
            multiline: false,
            password: false,
            readonly: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Editor", &[input]);
        let text_run_id = mapped_node(&snapshot, 36).children()[0];

        let request = |anchor_node, anchor_index, focus_index| ActionRequest {
            action: Action::SetTextSelection,
            target_tree: TreeId::ROOT,
            target_node: NodeId(36),
            data: Some(ActionData::SetTextSelection(TextSelection {
                anchor: TextPosition {
                    node: anchor_node,
                    character_index: anchor_index,
                },
                focus: TextPosition {
                    node: text_run_id,
                    character_index: focus_index,
                },
            })),
        };

        assert_eq!(snapshot.map_action(&request(NodeId(36), 0, 0)), None);
        assert_eq!(snapshot.map_action(&request(text_run_id, 6, 0)), None);
        assert!(mapped_node(&snapshot, 36).supports_action(Action::SetTextSelection));

        let mut non_editable = node(37, SemanticsRole::Text);
        non_editable.actions = vec![SemanticsAction::SetSelection];
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Text", &[non_editable]);
        assert!(!mapped_node(&snapshot, 37).supports_action(Action::SetTextSelection));
    }

    #[test]
    fn oversized_graphemes_are_split_at_utf8_scalar_boundaries() {
        let text = format!("a{}", "\u{301}".repeat(200));
        let mut input = node(38, SemanticsRole::TextInput);
        input.value = Some(SemanticsValue::Text(text.clone()));
        input.editable_text = Some(EditableTextSemantics {
            caret_offset: text.len(),
            selection: SemanticsTextRange::new(text.len(), text.len()),
            multiline: false,
            password: false,
            readonly: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Editor", &[input]);
        let text_run_id = mapped_node(&snapshot, 38).children()[0];
        let lengths = mapped_accesskit_node(&snapshot, text_run_id).character_lengths();

        assert!(lengths.len() > 1);
        assert_eq!(
            lengths
                .iter()
                .map(|length| usize::from(*length))
                .sum::<usize>(),
            text.len()
        );
        assert!(lengths.iter().all(|length| *length > 0));
        for boundary in snapshot.text_offsets[&NodeId(38)].iter().copied() {
            assert!(text.is_char_boundary(boundary));
        }
    }

    #[test]
    fn deepest_enabled_focused_semantic_node_wins() {
        let mut root = node(50, SemanticsRole::Root);
        root.state.focused = true;
        let mut child = node(51, SemanticsRole::List);
        child.parent = Some(root.id);
        child.state.focused = true;
        let mut grandchild = node(52, SemanticsRole::ListItem);
        grandchild.parent = Some(child.id);
        grandchild.state.focused = true;

        let snapshot = build_accesskit_snapshot(
            WindowId::new(1),
            1.0,
            "Queue",
            &[root.clone(), child.clone(), grandchild.clone()],
        );
        assert_eq!(snapshot.update.focus, NodeId(52));

        grandchild.state.disabled = true;
        let snapshot =
            build_accesskit_snapshot(WindowId::new(1), 1.0, "Queue", &[root, child, grandchild]);
        assert_eq!(snapshot.update.focus, NodeId(51));
    }

    #[test]
    fn custom_action_ids_are_stable_and_edit_actions_round_trip() {
        let mut first = node(60, SemanticsRole::TextInput);
        first.actions = vec![
            SemanticsAction::Custom("Reset".into()),
            SemanticsAction::Copy,
            SemanticsAction::DeleteBackward,
        ];
        let mut second = first.clone();
        second.actions.reverse();
        let first = build_accesskit_snapshot(WindowId::new(1), 1.0, "Editor", &[first]);
        let second = build_accesskit_snapshot(WindowId::new(1), 1.0, "Editor", &[second]);

        let ids = |snapshot: &AccessKitSnapshot| {
            mapped_node(snapshot, 60)
                .custom_actions()
                .iter()
                .map(|action| (action.description.to_string(), action.id))
                .collect::<HashMap<_, _>>()
        };
        assert_eq!(ids(&first), ids(&second));

        let copy_id = ids(&first)["Copy"];
        let request = ActionRequest {
            action: Action::CustomAction,
            target_tree: TreeId::ROOT,
            target_node: NodeId(60),
            data: Some(ActionData::CustomAction(copy_id)),
        };
        assert_eq!(
            first.map_action(&request),
            Some((WidgetId::new(60), SemanticsActionRequest::Copy))
        );
    }

    #[test]
    fn unbounded_numeric_range_and_step_are_finite_for_accesskit() {
        let mut spin = node(70, SemanticsRole::SpinBox);
        spin.value = Some(SemanticsValue::Range {
            value: 4.0,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        });
        spin.numeric_step = Some(0.5);
        spin.actions = vec![SemanticsAction::SetValue];
        let snapshot = build_accesskit_snapshot(WindowId::new(1), 1.0, "Count", &[spin]);
        let spin = mapped_node(&snapshot, 70);

        assert_eq!(spin.numeric_value(), Some(4.0));
        assert_eq!(spin.min_numeric_value(), Some(f64::MIN));
        assert_eq!(spin.max_numeric_value(), Some(f64::MAX));
        assert_eq!(spin.numeric_value_step(), Some(0.5));
    }

    #[test]
    fn malformed_parent_cycles_are_reparented_to_synthetic_root() {
        let mut first = node(41, SemanticsRole::ListItem);
        let mut second = node(42, SemanticsRole::ListItem);
        first.parent = Some(second.id);
        second.parent = Some(first.id);
        let snapshot = build_accesskit_snapshot(WindowId::new(2), 1.0, "Queue", &[first, second]);
        let update = snapshot.full_update();
        let root_id = update.tree.unwrap().root;
        let root = update.nodes.iter().find(|(id, _)| *id == root_id).unwrap();
        assert_eq!(root.1.children(), &[NodeId(41), NodeId(42)]);
    }
}
