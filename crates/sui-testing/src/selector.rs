use sui_core::{SemanticsNode, SemanticsRole, SemanticsValue};
use sui_platform::AccessibilitySnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selector {
    role: Option<SemanticsRole>,
    name: Option<String>,
    text: Option<String>,
    description: Option<String>,
    focused: Option<bool>,
    root_only: bool,
}

impl Selector {
    pub fn by_role(role: SemanticsRole) -> Self {
        Self {
            role: Some(role),
            ..Self::default()
        }
    }

    pub fn by_text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            ..Self::default()
        }
    }

    pub fn by_description(text: impl Into<String>) -> Self {
        Self {
            description: Some(text.into()),
            ..Self::default()
        }
    }

    pub fn focused() -> Self {
        Self {
            focused: Some(true),
            ..Self::default()
        }
    }

    pub fn root() -> Self {
        Self {
            root_only: true,
            ..Self::default()
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn matches(&self, snapshot: &AccessibilitySnapshot, node: &SemanticsNode) -> bool {
        if self.root_only && snapshot.root != Some(node.id) {
            return false;
        }

        if self.role.as_ref().is_some_and(|role| role != &node.role) {
            return false;
        }

        if self
            .name
            .as_ref()
            .is_some_and(|name| node.name.as_ref() != Some(name))
        {
            return false;
        }

        if self
            .description
            .as_ref()
            .is_some_and(|description| node.description.as_ref() != Some(description))
        {
            return false;
        }

        if self
            .focused
            .is_some_and(|focused| node.state.focused != focused)
        {
            return false;
        }

        if self
            .text
            .as_ref()
            .is_some_and(|text| self.node_text(node).as_deref() != Some(text.as_str()))
        {
            return false;
        }

        true
    }

    pub fn is_visible(&self, node: &SemanticsNode) -> bool {
        !node.state.hidden && !node.bounds.is_empty()
    }

    pub fn node_text(&self, node: &SemanticsNode) -> Option<String> {
        node.name.clone().or_else(|| match node.value.as_ref() {
            Some(SemanticsValue::Text(text)) => Some(text.clone()),
            _ => None,
        })
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        if let Some(role) = &self.role {
            parts.push(format!("role={role:?}"));
        }
        if let Some(name) = &self.name {
            parts.push(format!("name={name:?}"));
        }
        if let Some(text) = &self.text {
            parts.push(format!("text={text:?}"));
        }
        if let Some(description) = &self.description {
            parts.push(format!("description={description:?}"));
        }
        if let Some(focused) = self.focused {
            parts.push(format!("focused={focused}"));
        }
        if self.root_only {
            parts.push("root=true".to_string());
        }

        if parts.is_empty() {
            "<all nodes>".to_string()
        } else {
            parts.join(", ")
        }
    }
}
