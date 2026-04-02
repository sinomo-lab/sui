use sui_core::{Error, Result, SemanticsValue};

use crate::{diagnostics::format_failure, locator::Locator};

#[derive(Clone)]
pub struct Expectation {
    locator: Locator,
    timeout: Option<f64>,
}

impl Expectation {
    pub(crate) fn new(locator: Locator) -> Self {
        Self {
            locator,
            timeout: None,
        }
    }

    pub fn with_timeout(mut self, timeout: f64) -> Self {
        self.timeout = Some(timeout.max(0.0));
        self
    }

    pub fn to_be_visible(&self) -> Result<()> {
        self.wait_for("to_be_visible", |locator, harness| {
            let nodes = locator.resolve_all(harness)?;
            Ok(nodes
                .iter()
                .find(|node| locator.selector().is_visible(node))
                .map(|_| ()))
        })
    }

    pub fn to_be_hidden(&self) -> Result<()> {
        self.wait_for("to_be_hidden", |locator, harness| {
            let nodes = locator.resolve_all(harness)?;
            if nodes.is_empty() || nodes.iter().all(|node| !locator.selector().is_visible(node)) {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        })
    }

    pub fn to_be_focused(&self) -> Result<()> {
        self.wait_for("to_be_focused", |locator, harness| {
            let node = locator.resolve_unique(harness).ok();
            Ok(node.filter(|node| node.state.focused).map(|_| ()))
        })
    }

    pub fn to_have_text(&self, expected: impl AsRef<str>) -> Result<()> {
        let expected = expected.as_ref().to_string();
        self.wait_for("to_have_text", move |locator, harness| {
            let node = locator.resolve_unique(harness).ok();
            Ok(node
                .filter(|node| locator.selector().node_text(node).as_deref() == Some(expected.as_str()))
                .map(|_| ()))
        })
    }

    pub fn to_have_value(&self, expected: impl AsRef<str>) -> Result<()> {
        let expected = expected.as_ref().to_string();
        self.wait_for("to_have_value", move |locator, harness| {
            let node = locator.resolve_unique(harness).ok();
            Ok(node
                .filter(|node| value_as_string(node.value.as_ref()).as_deref() == Some(expected.as_str()))
                .map(|_| ()))
        })
    }

    pub fn to_have_count(&self, expected: usize) -> Result<()> {
        self.wait_for("to_have_count", move |locator, harness| {
            let count = locator.resolve_all(harness)?.len();
            Ok((count == expected).then_some(()))
        })
    }

    fn wait_for<F>(&self, action: &str, mut predicate: F) -> Result<()>
    where
        F: FnMut(&Locator, &crate::harness::Harness) -> Result<Option<()>>,
    {
        let timeout = self.timeout.unwrap_or_else(|| self.locator.default_timeout());
        let mut harness = self.locator.harness().borrow_mut();
        harness
            .run_until(timeout, |harness| predicate(&self.locator, harness))
            .map_err(|_| {
                let snapshot = harness.snapshot(self.locator.window_id()).unwrap_or_else(|_| {
                    harness.fallback_snapshot(self.locator.window_id())
                });
                Error::new(format_failure(
                    action,
                    self.locator.selector(),
                    &snapshot,
                    &format!("timed out after {timeout:.3}s"),
                ))
            })
    }
}

fn value_as_string(value: Option<&SemanticsValue>) -> Option<String> {
    match value {
        Some(SemanticsValue::Text(text)) => Some(text.clone()),
        Some(SemanticsValue::Number(number)) => Some(number.to_string()),
        Some(SemanticsValue::Range { value, .. }) => Some(value.to_string()),
        None => None,
    }
}
