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
    /// An integer from a host language that distinguishes integers from
    /// floats. Numeric widget bindings accept it like [`Self::Number`].
    Integer(i64),
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
            Self::Integer(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }

    /// The value as a number, if it is numeric or boolean.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Integer(value) => Some(*value as f64),
            Self::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            Self::String(_) => None,
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

impl From<i64> for BindingValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for BindingValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
