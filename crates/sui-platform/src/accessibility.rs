use std::collections::{HashMap, HashSet};

use sui_core::{SemanticsAction, SemanticsNode, SemanticsRole, WidgetId, WindowId};

#[derive(Debug, Clone, PartialEq)]
pub struct AccessibilitySnapshot {
    pub window_id: WindowId,
    pub root: Option<WidgetId>,
    pub focused_widget: Option<WidgetId>,
    pub nodes: Vec<SemanticsNode>,
}

impl AccessibilitySnapshot {
    pub fn new(window_id: WindowId, nodes: Vec<SemanticsNode>) -> Self {
        let root = nodes
            .iter()
            .find(|node| node.parent.is_none())
            .map(|node| node.id);
        let focused_widget = nodes
            .iter()
            .find(|node| node.state.focused)
            .map(|node| node.id);

        Self {
            window_id,
            root,
            focused_widget,
            nodes,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AccessibilityBridge {
    snapshot: Option<AccessibilitySnapshot>,
}

impl AccessibilityBridge {
    pub(crate) fn snapshot(&self) -> Option<&AccessibilitySnapshot> {
        self.snapshot.as_ref()
    }

    pub(crate) fn update(&mut self, window_id: WindowId, nodes: Vec<SemanticsNode>) {
        self.snapshot = Some(AccessibilitySnapshot::new(window_id, nodes));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityIssueSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityIssueTarget {
    Snapshot,
    Node(WidgetId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityIssue {
    pub severity: AccessibilityIssueSeverity,
    pub target: AccessibilityIssueTarget,
    pub message: String,
}

impl AccessibilityIssue {
    fn new(
        severity: AccessibilityIssueSeverity,
        target: AccessibilityIssueTarget,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            target,
            message: message.into(),
        }
    }
}

/// Validate a semantic snapshot using the shared accessibility contract used
/// by inspectors, terminal rendering, and test tooling.
pub fn validate_accessibility_snapshot(
    snapshot: &AccessibilitySnapshot,
) -> Vec<AccessibilityIssue> {
    let mut issues = Vec::new();
    let roots = snapshot
        .nodes
        .iter()
        .filter(|node| node.parent.is_none())
        .collect::<Vec<_>>();
    if roots.is_empty() {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Error,
            AccessibilityIssueTarget::Snapshot,
            "accessibility snapshot has no root node",
        ));
    } else if roots.len() > 1 {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Error,
            AccessibilityIssueTarget::Snapshot,
            format!("accessibility snapshot has {} root nodes", roots.len()),
        ));
    }
    if let Some(root) = snapshot.root
        && !snapshot.nodes.iter().any(|node| node.id == root)
    {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Error,
            AccessibilityIssueTarget::Snapshot,
            format!("snapshot root #{root} is missing from nodes"),
        ));
    }

    let mut counts = HashMap::<WidgetId, usize>::new();
    for node in &snapshot.nodes {
        *counts.entry(node.id).or_default() += 1;
    }
    for (&id, &count) in counts.iter().filter(|(_, count)| **count > 1) {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Error,
            AccessibilityIssueTarget::Node(id),
            format!("duplicate accessibility node id #{id} appears {count} times"),
        ));
    }
    let ids = counts.keys().copied().collect::<HashSet<_>>();
    let parents = snapshot
        .nodes
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<HashMap<_, _>>();
    for node in &snapshot.nodes {
        if let Some(parent) = node.parent {
            if !ids.contains(&parent) {
                issues.push(AccessibilityIssue::new(
                    AccessibilityIssueSeverity::Error,
                    AccessibilityIssueTarget::Node(node.id),
                    format!("node #{} references missing parent #{parent}", node.id),
                ));
            }
            if parent == node.id {
                issues.push(AccessibilityIssue::new(
                    AccessibilityIssueSeverity::Error,
                    AccessibilityIssueTarget::Node(node.id),
                    format!("node #{} cannot parent itself", node.id),
                ));
            }
        }
        let mut seen = HashSet::new();
        let mut current = Some(node.id);
        while let Some(id) = current {
            if !seen.insert(id) {
                issues.push(AccessibilityIssue::new(
                    AccessibilityIssueSeverity::Error,
                    AccessibilityIssueTarget::Node(node.id),
                    format!("node #{} has a cyclic parent chain", node.id),
                ));
                break;
            }
            current = parents.get(&id).copied().flatten();
        }
    }

    if let Some(focused) = snapshot.focused_widget {
        match snapshot.nodes.iter().find(|node| node.id == focused) {
            Some(node) if node.state.hidden => issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Warning,
                AccessibilityIssueTarget::Node(focused),
                format!("focused node #{focused} is hidden"),
            )),
            Some(_) => {}
            None => issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Error,
                AccessibilityIssueTarget::Snapshot,
                format!("focused widget #{focused} is missing from nodes"),
            )),
        }
    }

    for node in &snapshot.nodes {
        if interactive_role(node)
            && !node.state.hidden
            && node.name.as_deref().unwrap_or_default().trim().is_empty()
        {
            issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Error,
                AccessibilityIssueTarget::Node(node.id),
                format!(
                    "{:?} node #{} is missing accessible name",
                    node.role, node.id
                ),
            ));
        }
        for action in expected_actions(node) {
            if !node.actions.contains(&action) {
                issues.push(AccessibilityIssue::new(
                    AccessibilityIssueSeverity::Warning,
                    AccessibilityIssueTarget::Node(node.id),
                    format!(
                        "{:?} node #{} is missing expected action {:?}",
                        node.role, node.id, action
                    ),
                ));
            }
        }
        if value_role(node) && node.value.is_none() {
            issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Warning,
                AccessibilityIssueTarget::Node(node.id),
                format!("{:?} node #{} is missing value", node.role, node.id),
            ));
        }
        if !node.state.hidden && interactive_role(node) && node.bounds.is_empty() {
            issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Warning,
                AccessibilityIssueTarget::Node(node.id),
                format!("visible actionable node #{} has empty bounds", node.id),
            ));
        }
        if node.parent.is_some_and(|parent| {
            snapshot
                .nodes
                .iter()
                .find(|candidate| candidate.id == parent)
                .is_some_and(|parent| parent.state.hidden)
        }) && !node.state.hidden
        {
            issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Warning,
                AccessibilityIssueTarget::Node(node.id),
                format!("visible node #{} has a hidden parent", node.id),
            ));
        }
    }

    let mut visible_names = HashMap::<(String, String), Vec<WidgetId>>::new();
    for node in snapshot.nodes.iter().filter(|node| {
        !node.state.hidden
            && interactive_role(node)
            && node
                .name
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty())
    }) {
        visible_names
            .entry((
                format!("{:?}", node.role),
                node.name.clone().unwrap_or_default(),
            ))
            .or_default()
            .push(node.id);
    }
    for ((role, name), ids) in visible_names.into_iter().filter(|(_, ids)| ids.len() > 1) {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Warning,
            AccessibilityIssueTarget::Snapshot,
            format!(
                "duplicate visible actionable role/name {role} {name:?} on nodes {:?}",
                ids.iter().map(|id| id.get()).collect::<Vec<_>>()
            ),
        ));
    }

    issues
}

fn interactive_role(node: &SemanticsNode) -> bool {
    matches!(
        node.role,
        SemanticsRole::Button
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

fn value_role(node: &SemanticsNode) -> bool {
    matches!(
        node.role,
        SemanticsRole::Slider
            | SemanticsRole::ProgressBar
            | SemanticsRole::TextInput
            | SemanticsRole::SpinBox
            | SemanticsRole::ComboBox
            | SemanticsRole::ColorPicker
    )
}

fn expected_actions(node: &SemanticsNode) -> Vec<SemanticsAction> {
    match node.role {
        SemanticsRole::Button
        | SemanticsRole::CheckBox
        | SemanticsRole::Switch
        | SemanticsRole::RadioButton
        | SemanticsRole::MenuItem => vec![SemanticsAction::Activate],
        SemanticsRole::TextInput | SemanticsRole::ComboBox | SemanticsRole::ColorPicker => {
            vec![SemanticsAction::Focus, SemanticsAction::SetValue]
        }
        SemanticsRole::Slider | SemanticsRole::SpinBox => vec![
            SemanticsAction::Focus,
            SemanticsAction::Increment,
            SemanticsAction::Decrement,
        ],
        SemanticsRole::ScrollView => vec![SemanticsAction::Focus],
        _ => Vec::new(),
    }
}
