use crate::errors::{ForeignCallbackResult, ForeignErrorSink};
use crate::support::recover_lock;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub enum BindingValue {
    String(String),
    Number(f64),
    Bool(bool),
}

#[derive(Clone)]
pub struct BindingMessageAction {
    pub(crate) callback:
        Arc<dyn Fn(BindingValue) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingMessageAction {
    pub fn new(
        callback: impl Fn(BindingValue) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, payload: BindingValue) -> ForeignCallbackResult<()> {
        (self.callback)(payload)
    }
}

impl fmt::Debug for BindingMessageAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingMessageAction")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BindingMessageBus {
    pub(crate) handlers: Arc<Mutex<BTreeMap<String, Vec<BindingMessageAction>>>>,
    pub(crate) errors: ForeignErrorSink,
}

impl BindingMessageBus {
    pub(crate) fn on(&self, name: impl Into<String>, action: BindingMessageAction) {
        recover_lock(&self.handlers)
            .entry(name.into())
            .or_default()
            .push(action);
    }

    pub(crate) fn actions(&self, name: &str) -> Vec<BindingMessageAction> {
        recover_lock(&self.handlers)
            .get(name)
            .cloned()
            .unwrap_or_default()
    }
}

impl BindingValue {
    pub fn as_label_text(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => {
                let mut text = value.to_string();
                if text.ends_with(".0") {
                    text.truncate(text.len() - 2);
                }
                text
            }
            Self::Bool(value) => value.to_string(),
        }
    }
}

impl From<String> for BindingValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for BindingValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<f64> for BindingValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for BindingValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
