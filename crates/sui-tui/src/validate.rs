use std::collections::{HashMap, HashSet};

use sui_core::{SemanticsAction, SemanticsNode, SemanticsRole, WidgetId};
use sui_platform::AccessibilitySnapshot;

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

pub fn validate_snapshot(snapshot: &AccessibilitySnapshot) -> Vec<AccessibilityIssue> {
    let mut issues = Vec::new();
    validate_roots(snapshot, &mut issues);
    validate_ids_and_parents(snapshot, &mut issues);
    validate_nodes(snapshot, &mut issues);
    validate_duplicate_action_names(snapshot, &mut issues);
    issues
}

fn validate_roots(snapshot: &AccessibilitySnapshot, issues: &mut Vec<AccessibilityIssue>) {
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
}

fn validate_ids_and_parents(
    snapshot: &AccessibilitySnapshot,
    issues: &mut Vec<AccessibilityIssue>,
) {
    let mut counts = HashMap::<WidgetId, usize>::new();
    for node in &snapshot.nodes {
        *counts.entry(node.id).or_default() += 1;
    }

    for (id, count) in counts.iter().filter(|(_, count)| **count > 1) {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Error,
            AccessibilityIssueTarget::Node(*id),
            format!("duplicate accessibility node id #{id} appears {count} times"),
        ));
    }

    let ids = counts.keys().copied().collect::<HashSet<_>>();
    for node in &snapshot.nodes {
        if let Some(parent) = node.parent {
            if !ids.contains(&parent) {
                issues.push(AccessibilityIssue::new(
                    AccessibilityIssueSeverity::Error,
                    AccessibilityIssueTarget::Node(node.id),
                    format!("node #{} references missing parent #{}", node.id, parent),
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
    }

    let parent_by_id = snapshot
        .nodes
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<HashMap<_, _>>();
    for node in &snapshot.nodes {
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
            current = parent_by_id.get(&id).copied().flatten();
        }
    }
}

fn validate_nodes(snapshot: &AccessibilitySnapshot, issues: &mut Vec<AccessibilityIssue>) {
    let ids = snapshot
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();

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
            if !has_action(node, &action) {
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

        if !node.state.hidden && node.bounds.is_empty() && interactive_role(node) {
            issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Warning,
                AccessibilityIssueTarget::Node(node.id),
                format!("visible actionable node #{} has empty bounds", node.id),
            ));
        }

        if node
            .parent
            .is_some_and(|parent| ids.contains(&parent) && hidden_parent(snapshot, parent))
            && !node.state.hidden
        {
            issues.push(AccessibilityIssue::new(
                AccessibilityIssueSeverity::Warning,
                AccessibilityIssueTarget::Node(node.id),
                format!("visible node #{} has a hidden parent", node.id),
            ));
        }
    }
}

fn validate_duplicate_action_names(
    snapshot: &AccessibilitySnapshot,
    issues: &mut Vec<AccessibilityIssue>,
) {
    let mut names = HashMap::<(String, String), Vec<WidgetId>>::new();
    for node in snapshot.nodes.iter().filter(|node| {
        !node.state.hidden
            && interactive_role(node)
            && node
                .name
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty())
    }) {
        names
            .entry((
                format!("{:?}", node.role),
                node.name.clone().unwrap_or_default(),
            ))
            .or_default()
            .push(node.id);
    }

    for ((role, name), ids) in names.into_iter().filter(|(_, ids)| ids.len() > 1) {
        issues.push(AccessibilityIssue::new(
            AccessibilityIssueSeverity::Warning,
            AccessibilityIssueTarget::Snapshot,
            format!(
                "duplicate visible actionable role/name {role} {name:?} on nodes {:?}",
                ids.iter().map(|id| id.get()).collect::<Vec<_>>()
            ),
        ));
    }
}

fn hidden_parent(snapshot: &AccessibilitySnapshot, parent: WidgetId) -> bool {
    snapshot
        .nodes
        .iter()
        .find(|node| node.id == parent)
        .is_some_and(|node| node.state.hidden)
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

fn has_action(node: &SemanticsNode, expected: &SemanticsAction) -> bool {
    node.actions.iter().any(|action| action == expected)
}
