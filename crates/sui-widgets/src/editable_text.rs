use std::ops::{Deref, DerefMut};

use sui_core::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, KeyboardEvent,
    SemanticsAction, SemanticsActionRequest, SemanticsValue, WidgetId,
};
use sui_runtime::EventCtx;

use crate::{
    editor::{EditorCommand, EditorCommandResult, EditorState},
    selection::{SelectionChange, SelectionClipboardBehavior, SelectionOwnerId, SelectionScope},
    text_command::TextCommand,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditableTextLineMode {
    SingleLine,
    MultiLine,
}

impl EditableTextLineMode {
    fn normalize(self, text: impl Into<String>) -> String {
        match self {
            Self::SingleLine => single_line_text(text),
            Self::MultiLine => text.into(),
        }
    }

    fn is_multiline(self) -> bool {
        matches!(self, Self::MultiLine)
    }
}

pub(crate) struct EditableTextController {
    editor: EditorState,
    selection_scope: Option<SelectionScope>,
    clipboard_behavior: Option<SelectionClipboardBehavior>,
}

impl EditableTextController {
    pub(crate) fn new() -> Self {
        Self {
            editor: EditorState::new(),
            selection_scope: None,
            clipboard_behavior: None,
        }
    }

    pub(crate) fn selectable(&mut self, selection_scope: SelectionScope) {
        self.selection_scope = Some(selection_scope);
    }

    pub(crate) fn selection_scope(&mut self, selection_scope: SelectionScope) {
        self.selection_scope = Some(selection_scope);
    }

    pub(crate) fn clipboard_behavior(&mut self, behavior: SelectionClipboardBehavior) {
        self.clipboard_behavior = Some(behavior);
    }

    pub(crate) fn handles_implicit_clipboard(&self) -> bool {
        self.clipboard_behavior
            .unwrap_or(if self.selection_scope.is_some() {
                SelectionClipboardBehavior::AppManaged
            } else {
                SelectionClipboardBehavior::WidgetManaged
            })
            .is_widget_managed()
    }

    pub(crate) fn apply_result_common(
        &mut self,
        ctx: &mut EventCtx,
        result: &mut EditorCommandResult,
    ) {
        if let Some(text) = result.clipboard_text.take() {
            ctx.set_clipboard_text(text);
        }
        if result.text_changed || result.selection_changed || result.composition_changed {
            self.sync_selection_scope(ctx);
        }
        if result.handled {
            ctx.set_handled();
        }
    }

    pub(crate) fn semantic_actions(&self, read_only: bool) -> Vec<SemanticsAction> {
        let mut actions = vec![SemanticsAction::Focus, SemanticsAction::SetSelection];
        if !read_only {
            actions.extend([
                SemanticsAction::SetValue,
                SemanticsAction::InsertText,
                SemanticsAction::DeleteBackward,
                SemanticsAction::DeleteForward,
            ]);
        }
        if self.handles_implicit_clipboard() {
            actions.push(SemanticsAction::Copy);
            if !read_only {
                actions.push(SemanticsAction::Cut);
            }
        }
        if !read_only {
            actions.extend([
                SemanticsAction::Paste,
                SemanticsAction::Undo,
                SemanticsAction::Redo,
            ]);
        }
        actions
    }

    pub(crate) fn semantics_commands(
        &self,
        ctx: &EventCtx,
        action: &SemanticsActionRequest,
        read_only: bool,
        line_mode: EditableTextLineMode,
    ) -> Option<Vec<EditorCommand>> {
        match action {
            SemanticsActionRequest::SetValue(SemanticsValue::Text(text)) if !read_only => {
                Some(vec![
                    EditorCommand::SelectAll,
                    EditorCommand::InsertText(line_mode.normalize(text.clone())),
                ])
            }
            SemanticsActionRequest::SetSelection(selection) => {
                Some(vec![EditorCommand::SetSelection {
                    anchor: selection.start,
                    focus: selection.end,
                }])
            }
            SemanticsActionRequest::InsertText(text) if !read_only => {
                Some(vec![EditorCommand::InsertText(
                    line_mode.normalize(text.clone()),
                )])
            }
            SemanticsActionRequest::DeleteBackward if !read_only => {
                Some(vec![EditorCommand::DeleteBackward])
            }
            SemanticsActionRequest::DeleteForward if !read_only => {
                Some(vec![EditorCommand::DeleteForward])
            }
            SemanticsActionRequest::Copy if self.handles_implicit_clipboard() => {
                Some(vec![EditorCommand::Copy])
            }
            SemanticsActionRequest::Cut if !read_only && self.handles_implicit_clipboard() => {
                Some(vec![EditorCommand::Cut])
            }
            SemanticsActionRequest::Paste if !read_only => {
                Some(vec![paste_command(ctx, line_mode)])
            }
            SemanticsActionRequest::Undo if !read_only => Some(vec![EditorCommand::Undo]),
            SemanticsActionRequest::Redo if !read_only => Some(vec![EditorCommand::Redo]),
            _ => None,
        }
    }

    pub(crate) fn text_command(
        &self,
        ctx: &EventCtx,
        command: TextCommand,
        read_only: bool,
        line_mode: EditableTextLineMode,
    ) -> Option<EditorCommand> {
        match command {
            TextCommand::SelectAll => Some(EditorCommand::SelectAll),
            TextCommand::Copy => Some(EditorCommand::Copy),
            TextCommand::Cut if !read_only => Some(EditorCommand::Cut),
            TextCommand::Paste if !read_only => Some(paste_command(ctx, line_mode)),
            TextCommand::Cut | TextCommand::Paste => None,
        }
    }

    pub(crate) fn keyboard_command(
        &self,
        ctx: &EventCtx,
        key: &KeyboardEvent,
        read_only: bool,
        line_mode: EditableTextLineMode,
    ) -> Option<EditorCommand> {
        if key.state != KeyState::Pressed {
            return None;
        }

        let command_modifier = key.modifiers.control || key.modifiers.meta;
        match key.key.as_str() {
            "a" | "A" if command_modifier => Some(EditorCommand::SelectAll),
            "c" | "C" if command_modifier && self.handles_implicit_clipboard() => {
                Some(EditorCommand::Copy)
            }
            "x" | "X" if command_modifier && !read_only && self.handles_implicit_clipboard() => {
                Some(EditorCommand::Cut)
            }
            "v" | "V" if command_modifier && !read_only => Some(paste_command(ctx, line_mode)),
            "z" | "Z" if command_modifier && key.modifiers.shift && !read_only => {
                Some(EditorCommand::Redo)
            }
            "z" | "Z" if command_modifier && !read_only => Some(EditorCommand::Undo),
            "y" | "Y" if command_modifier && !read_only => Some(EditorCommand::Redo),
            "ArrowLeft" if command_modifier => Some(EditorCommand::MoveWordLeft {
                extend: key.modifiers.shift,
            }),
            "ArrowRight" if command_modifier => Some(EditorCommand::MoveWordRight {
                extend: key.modifiers.shift,
            }),
            "ArrowLeft" => Some(EditorCommand::MoveLeft {
                extend: key.modifiers.shift,
            }),
            "ArrowRight" => Some(EditorCommand::MoveRight {
                extend: key.modifiers.shift,
            }),
            "ArrowUp" if line_mode.is_multiline() => Some(EditorCommand::MoveUp {
                extend: key.modifiers.shift,
            }),
            "ArrowDown" if line_mode.is_multiline() => Some(EditorCommand::MoveDown {
                extend: key.modifiers.shift,
            }),
            "Home" => Some(EditorCommand::MoveLineStart {
                extend: key.modifiers.shift,
            }),
            "End" => Some(EditorCommand::MoveLineEnd {
                extend: key.modifiers.shift,
            }),
            "PageUp" if line_mode.is_multiline() => Some(EditorCommand::PageUp {
                extend: key.modifiers.shift,
                lines: 8,
            }),
            "PageDown" if line_mode.is_multiline() => Some(EditorCommand::PageDown {
                extend: key.modifiers.shift,
                lines: 8,
            }),
            "Backspace" if !read_only => Some(EditorCommand::DeleteBackward),
            "Delete" if !read_only => Some(EditorCommand::DeleteForward),
            "Enter" if !read_only && line_mode.is_multiline() => {
                Some(EditorCommand::InsertText("\n".to_string()))
            }
            _ if !read_only && self.editor.composition().is_none() => keyboard_text(key)
                .map(|text| EditorCommand::InsertText(line_mode.normalize(text.to_string()))),
            _ => None,
        }
    }

    fn sync_selection_scope(&self, ctx: &mut EventCtx) {
        let Some(scope) = &self.selection_scope else {
            return;
        };
        let owner = SelectionOwnerId::from(ctx.widget_id());
        let range = self.editor.selection_range();
        let selected = self.editor.selected_text().to_string();
        let change =
            scope.replace_text(owner, owner, range, self.editor.document().len(), selected);
        request_selection_change(ctx, change);
    }
}

impl Default for EditableTextController {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for EditableTextController {
    type Target = EditorState;

    fn deref(&self) -> &Self::Target {
        &self.editor
    }
}

impl DerefMut for EditableTextController {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.editor
    }
}

pub(crate) fn single_line_text(text: impl Into<String>) -> String {
    text.into()
        .chars()
        .filter(|ch| *ch != '\r' && *ch != '\n')
        .collect()
}

pub(crate) fn keyboard_text(event: &KeyboardEvent) -> Option<&str> {
    if event.state != KeyState::Pressed
        || event.is_composing
        || event.modifiers.control
        || event.modifiers.alt
        || event.modifiers.meta
    {
        return None;
    }

    if let Some(text) = event
        .text
        .as_deref()
        .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
    {
        return Some(text);
    }

    let key = event.key.as_str();
    (key.chars().count() == 1 && !key.chars().any(char::is_control)).then_some(key)
}

pub(crate) fn paste_command(ctx: &EventCtx, line_mode: EditableTextLineMode) -> EditorCommand {
    ctx.clipboard_text()
        .filter(|text| !text.is_empty())
        .map(|text| EditorCommand::Paste(line_mode.normalize(text)))
        .unwrap_or(EditorCommand::Noop)
}

fn request_selection_change(ctx: &mut EventCtx, change: SelectionChange) {
    for owner in change.affected_owners() {
        let widget_id = WidgetId::new(owner.get());
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            InvalidationKind::Paint,
        ));
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Widget(widget_id),
            InvalidationKind::Semantics,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_core::{KeyState, KeyboardEvent};

    #[test]
    fn keyboard_text_falls_back_to_printable_key_when_text_payload_is_missing() {
        let mut event = KeyboardEvent::new("h", KeyState::Pressed);
        event.text = None;
        assert_eq!(keyboard_text(&event), Some("h"));

        event.modifiers.control = true;
        assert_eq!(keyboard_text(&event), None);
    }

    #[test]
    fn single_line_mode_removes_line_breaks() {
        assert_eq!(
            EditableTextLineMode::SingleLine.normalize("alpha\r\nbeta"),
            "alphabeta"
        );
    }
}
