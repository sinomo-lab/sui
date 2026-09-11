use crate::application::normalized_option_name;
use sui::DragDropScope;
use sui::DragEvent;
use sui::DragPayload;
use sui::DropEffect;

#[derive(Debug, Clone)]
pub struct BindingDragScope {
    pub(crate) inner: DragDropScope,
}

impl BindingDragScope {
    pub fn new() -> Self {
        Self {
            inner: DragDropScope::new(),
        }
    }

    pub fn active(&self) -> bool {
        self.inner.active_drag().is_some()
    }
}

impl Default for BindingDragScope {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn binding_drop_effect(value: &str) -> Result<DropEffect, String> {
    match normalized_option_name(value).as_str() {
        "none" | "reject" => Ok(DropEffect::None),
        "copy" => Ok(DropEffect::Copy),
        "move" => Ok(DropEffect::Move),
        "link" => Ok(DropEffect::Link),
        _ => Err(format!(
            "drop effect must be 'none', 'copy', 'move', or 'link', got '{value}'"
        )),
    }
}

pub(crate) fn binding_drag_payload_text(event: &DragEvent) -> String {
    match &event.payload {
        DragPayload::Text(text) => text.clone(),
        DragPayload::Image { handle, .. } => format!("image:{}", handle.get()),
        DragPayload::Custom { kind, .. } => kind.to_string(),
    }
}
