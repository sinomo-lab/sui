use std::ops::Range;

use sui_text::{TextCursor, TextSelection};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorTextEdit {
    pub range: Range<usize>,
    pub replacement_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EditorDocument {
    text: String,
    line_starts: Vec<usize>,
    revision: u64,
    dirty_line_range: Range<usize>,
    line_offsets_dirty_from: Option<usize>,
    pending_text_edit: Option<EditorTextEdit>,
}

impl EditorDocument {
    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let line_starts = line_starts_for(&text);
        let line_count = line_starts.len();
        Self {
            text,
            line_starts,
            revision: 0,
            dirty_line_range: 0..line_count,
            line_offsets_dirty_from: Some(0),
            pending_text_edit: None,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub(crate) fn line_range(&self, line_index: usize) -> Range<usize> {
        let line_index = line_index.min(self.line_count().saturating_sub(1));
        let start = self.line_starts[line_index];
        let end = self
            .line_starts
            .get(line_index + 1)
            .map(|next| next.saturating_sub(1))
            .unwrap_or(self.text.len());
        start..end
    }

    pub(crate) fn line_text(&self, line_index: usize) -> &str {
        &self.text[self.line_range(line_index)]
    }

    pub(crate) fn line_index_for_offset(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn dirty_line_range(&self) -> Range<usize> {
        self.dirty_line_range.clone()
    }

    pub(crate) fn line_offsets_dirty_from(&self) -> Option<usize> {
        self.line_offsets_dirty_from
    }

    pub(crate) fn clear_dirty(&mut self) {
        let line_index = self.line_index_for_offset(self.text.len());
        self.dirty_line_range = line_index..line_index;
        self.line_offsets_dirty_from = None;
    }

    pub(crate) fn take_text_edit(&mut self) -> Option<EditorTextEdit> {
        self.pending_text_edit.take()
    }

    fn replace_range(&mut self, range: Range<usize>, replacement: &str) {
        let old_line_count = self.line_count();
        let pending_dirty = self.dirty_line_range.clone();
        let start_line = self.line_index_for_offset(range.start);
        let old_end_line = self.line_index_for_offset(range.end);
        let retained_prefix_end = start_line + 1;
        let shifted_suffix_start = self
            .line_starts
            .partition_point(|line_start| *line_start <= range.end);
        let replacement_line_starts = replacement
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(range.start + index + 1))
            .collect::<Vec<_>>();
        let removed_len = range.end - range.start;

        self.text.replace_range(range.clone(), replacement);
        self.pending_text_edit = Some(EditorTextEdit {
            range: range.clone(),
            replacement_len: replacement.len(),
        });
        self.line_starts.splice(
            retained_prefix_end..shifted_suffix_start,
            replacement_line_starts.iter().copied(),
        );
        let shifted_suffix_start = retained_prefix_end + replacement_line_starts.len();
        if replacement.len() >= removed_len {
            let delta = replacement.len() - removed_len;
            for line_start in &mut self.line_starts[shifted_suffix_start..] {
                *line_start += delta;
            }
        } else {
            let delta = removed_len - replacement.len();
            for line_start in &mut self.line_starts[shifted_suffix_start..] {
                *line_start -= delta;
            }
        }
        self.revision = self.revision.saturating_add(1);

        let replacement_end = range.start + replacement.len();
        let new_end_line = self.line_index_for_offset(replacement_end);
        let dirty_start = start_line.min(self.line_count().saturating_sub(1));
        let dirty_end = old_end_line.max(new_end_line).saturating_add(1);
        let edit_dirty = dirty_start..dirty_end.min(self.line_count());
        let shifted_suffix_start = edit_dirty.end;
        self.dirty_line_range = if pending_dirty.is_empty() {
            edit_dirty
        } else if old_line_count == self.line_count() {
            pending_dirty.start.min(edit_dirty.start)..pending_dirty.end.max(edit_dirty.end)
        } else {
            // Pending dirty indices after a line-count-changing edit are costly to
            // remap precisely. Conservatively retain every line from the earliest
            // affected line until the next successful measure clears the state.
            pending_dirty.start.min(edit_dirty.start)..self.line_count()
        };
        if replacement.len() != removed_len && old_line_count == self.line_count() {
            self.line_offsets_dirty_from = Some(
                self.line_offsets_dirty_from
                    .map_or(shifted_suffix_start, |pending| {
                        pending.min(shifted_suffix_start)
                    }),
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EditorComposition {
    pub text: String,
    pub replacement_range: Range<usize>,
    pub cursor_range: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
struct EditorEdit {
    range: Range<usize>,
    removed: String,
    inserted: String,
    selection_before: TextSelection,
    selection_after: TextSelection,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EditorState {
    document: EditorDocument,
    selection: TextSelection,
    composition: Option<EditorComposition>,
    preferred_x: Option<f32>,
    preferred_column: Option<usize>,
    scroll_x: f32,
    scroll_y: f32,
    undo_stack: Vec<EditorEdit>,
    redo_stack: Vec<EditorEdit>,
}

impl EditorState {
    pub(crate) fn new() -> Self {
        Self::from_text("")
    }

    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        let document = EditorDocument::from_text(text);
        let cursor = TextCursor::new(document.len());
        Self {
            document,
            selection: TextSelection::new(cursor, cursor),
            composition: None,
            preferred_x: None,
            preferred_column: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub(crate) fn document(&self) -> &EditorDocument {
        &self.document
    }

    pub(crate) fn clear_document_dirty(&mut self) {
        self.document.clear_dirty();
    }

    pub(crate) fn take_text_edit(&mut self) -> Option<EditorTextEdit> {
        self.document.take_text_edit()
    }

    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        self.document = EditorDocument::from_text(text);
        let cursor = TextCursor::new(self.document.len());
        self.selection = TextSelection::new(cursor, cursor);
        self.composition = None;
        self.preferred_x = None;
        self.preferred_column = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub(crate) fn selection(&self) -> &TextSelection {
        &self.selection
    }

    pub(crate) fn composition(&self) -> Option<&EditorComposition> {
        self.composition.as_ref()
    }

    pub(crate) fn preferred_x(&self) -> Option<f32> {
        self.preferred_x
    }

    pub(crate) fn set_preferred_x(&mut self, preferred_x: Option<f32>) {
        self.preferred_x = preferred_x;
    }

    pub(crate) fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    pub(crate) fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    pub(crate) fn set_scroll(&mut self, scroll_x: f32, scroll_y: f32) {
        self.scroll_x = scroll_x.max(0.0);
        self.scroll_y = scroll_y.max(0.0);
    }

    pub(crate) fn selection_is_collapsed(&self) -> bool {
        self.selection.anchor.utf8_offset == self.selection.focus.utf8_offset
    }

    pub(crate) fn selection_range(&self) -> Range<usize> {
        selection_range(&self.selection, self.document.len())
    }

    pub(crate) fn selected_text(&self) -> &str {
        &self.document.text()[self.selection_range()]
    }

    pub(crate) fn display_text(&self) -> String {
        let Some(composition) = &self.composition else {
            return self.document.text().to_string();
        };

        let mut text = self.document.text().to_string();
        text.replace_range(composition.replacement_range.clone(), &composition.text);
        text
    }

    pub(crate) fn display_selection(&self) -> TextSelection {
        if let Some(composition) = &self.composition {
            let cursor_offset = composition
                .cursor_range
                .as_ref()
                .map(|range| range.end.min(composition.text.len()))
                .unwrap_or(composition.text.len());
            let offset = composition.replacement_range.start + cursor_offset;
            let cursor = TextCursor::new(offset);
            TextSelection::new(cursor, cursor)
        } else {
            self.selection.clone()
        }
    }

    pub(crate) fn execute(&mut self, command: EditorCommand) -> EditorCommandResult {
        let before_revision = self.document.revision();
        let before_selection = self.selection.clone();
        let before_composition = self.composition.clone();
        let mut handled = true;
        let mut clipboard_text = None;

        match command {
            EditorCommand::InsertText(text) => {
                self.clear_composition();
                self.replace_selection(&text, before_selection.clone());
            }
            EditorCommand::DeleteBackward => {
                self.clear_composition();
                self.delete_backward(before_selection.clone());
            }
            EditorCommand::DeleteForward => {
                self.clear_composition();
                self.delete_forward(before_selection.clone());
            }
            EditorCommand::MoveLeft { extend } => {
                self.clear_composition();
                self.move_horizontal(true, extend);
            }
            EditorCommand::MoveRight { extend } => {
                self.clear_composition();
                self.move_horizontal(false, extend);
            }
            EditorCommand::MoveWordLeft { extend } => {
                self.clear_composition();
                self.move_word(true, extend);
            }
            EditorCommand::MoveWordRight { extend } => {
                self.clear_composition();
                self.move_word(false, extend);
            }
            EditorCommand::MoveLineStart { extend } => {
                self.clear_composition();
                self.move_line_boundary(true, extend);
            }
            EditorCommand::MoveLineEnd { extend } => {
                self.clear_composition();
                self.move_line_boundary(false, extend);
            }
            EditorCommand::MoveUp { extend } => {
                self.clear_composition();
                self.move_logical_vertical(-1, extend);
            }
            EditorCommand::MoveDown { extend } => {
                self.clear_composition();
                self.move_logical_vertical(1, extend);
            }
            EditorCommand::PageUp { extend, lines } => {
                self.clear_composition();
                self.move_logical_vertical(-(lines.max(1) as isize), extend);
            }
            EditorCommand::PageDown { extend, lines } => {
                self.clear_composition();
                self.move_logical_vertical(lines.max(1) as isize, extend);
            }
            EditorCommand::MoveTo { offset, extend } => {
                self.clear_composition();
                self.move_to_offset(offset, extend);
            }
            EditorCommand::SetSelection { anchor, focus } => {
                self.clear_composition();
                self.set_selection(anchor, focus);
            }
            EditorCommand::SelectAll => {
                self.clear_composition();
                self.selection =
                    TextSelection::new(TextCursor::new(0), TextCursor::new(self.document.len()));
                self.clear_preferred_position();
            }
            EditorCommand::Copy => {
                if !self.selection_is_collapsed() {
                    clipboard_text = Some(self.selected_text().to_string());
                }
            }
            EditorCommand::Cut => {
                if !self.selection_is_collapsed() {
                    clipboard_text = Some(self.selected_text().to_string());
                    self.clear_composition();
                    self.replace_selection("", before_selection.clone());
                }
            }
            EditorCommand::Paste(text) => {
                self.clear_composition();
                self.replace_selection(&text, before_selection.clone());
            }
            EditorCommand::Undo => {
                self.clear_composition();
                self.undo();
            }
            EditorCommand::Redo => {
                self.clear_composition();
                self.redo();
            }
            EditorCommand::StartComposition => {
                self.composition = Some(EditorComposition {
                    text: String::new(),
                    replacement_range: self.selection_range(),
                    cursor_range: None,
                });
            }
            EditorCommand::UpdateComposition { text, cursor_range } => {
                let replacement_range = self
                    .composition
                    .as_ref()
                    .map(|composition| composition.replacement_range.clone())
                    .unwrap_or_else(|| self.selection_range());
                self.composition = Some(EditorComposition {
                    text,
                    replacement_range,
                    cursor_range,
                });
            }
            EditorCommand::CommitComposition(text) => {
                let selection_before = self.selection.clone();
                if let Some(composition) = self.composition.take() {
                    self.set_selection(
                        composition.replacement_range.start,
                        composition.replacement_range.end,
                    );
                }
                self.replace_selection(&text, selection_before);
            }
            EditorCommand::EndComposition => {
                self.clear_composition();
            }
            EditorCommand::ClearComposition => {
                self.clear_composition();
            }
            EditorCommand::Noop => {
                handled = false;
            }
        }

        EditorCommandResult {
            handled,
            text_changed: self.document.revision() != before_revision,
            selection_changed: self.selection != before_selection,
            composition_changed: self.composition != before_composition,
            clipboard_text,
        }
    }

    fn replace_selection(&mut self, replacement: &str, selection_before: TextSelection) -> bool {
        let range = self.selection_range();
        if range.is_empty() && replacement.is_empty() {
            return false;
        }

        let removed = self.document.text()[range.clone()].to_string();
        self.document.replace_range(range.clone(), replacement);
        let cursor_offset = range.start + replacement.len();
        self.selection = TextSelection::new(
            TextCursor::new(cursor_offset),
            TextCursor::new(cursor_offset),
        );
        self.clear_preferred_position();
        self.undo_stack.push(EditorEdit {
            range,
            removed,
            inserted: replacement.to_string(),
            selection_before,
            selection_after: self.selection.clone(),
        });
        self.redo_stack.clear();
        true
    }

    fn delete_backward(&mut self, selection_before: TextSelection) -> bool {
        if !self.selection_is_collapsed() {
            return self.replace_selection("", selection_before);
        }

        let focus =
            clamp_to_grapheme_boundary(self.document.text(), self.selection.focus.utf8_offset);
        let previous = previous_grapheme_boundary(self.document.text(), focus);
        if previous == focus {
            return false;
        }
        self.selection = TextSelection::new(TextCursor::new(previous), TextCursor::new(focus));
        self.replace_selection("", selection_before)
    }

    fn delete_forward(&mut self, selection_before: TextSelection) -> bool {
        if !self.selection_is_collapsed() {
            return self.replace_selection("", selection_before);
        }

        let focus =
            clamp_to_grapheme_boundary(self.document.text(), self.selection.focus.utf8_offset);
        let next = next_grapheme_boundary(self.document.text(), focus);
        if next == focus {
            return false;
        }
        self.selection = TextSelection::new(TextCursor::new(focus), TextCursor::new(next));
        self.replace_selection("", selection_before)
    }

    fn move_horizontal(&mut self, backward: bool, extend: bool) {
        let range = self.selection_range();
        let focus =
            clamp_to_grapheme_boundary(self.document.text(), self.selection.focus.utf8_offset);
        let target = if !extend && !range.is_empty() {
            if backward { range.start } else { range.end }
        } else if backward {
            previous_grapheme_boundary(self.document.text(), focus)
        } else {
            next_grapheme_boundary(self.document.text(), focus)
        };
        self.move_to_offset(target, extend);
    }

    fn move_word(&mut self, backward: bool, extend: bool) {
        let focus =
            clamp_to_grapheme_boundary(self.document.text(), self.selection.focus.utf8_offset);
        let target = if backward {
            previous_word_boundary(self.document.text(), focus)
        } else {
            next_word_boundary(self.document.text(), focus)
        };
        self.move_to_offset(target, extend);
    }

    fn move_line_boundary(&mut self, to_start: bool, extend: bool) {
        let focus = self.selection.focus.utf8_offset.min(self.document.len());
        let line = self.document.line_index_for_offset(focus);
        let range = self.document.line_range(line);
        self.move_to_offset(if to_start { range.start } else { range.end }, extend);
    }

    fn move_logical_vertical(&mut self, delta_lines: isize, extend: bool) {
        let focus = self.selection.focus.utf8_offset.min(self.document.len());
        let line_index = self.document.line_index_for_offset(focus);
        let line_range = self.document.line_range(line_index);
        let column = self
            .preferred_column
            .unwrap_or_else(|| focus.saturating_sub(line_range.start));
        let target_line = (line_index as isize + delta_lines)
            .clamp(0, self.document.line_count().saturating_sub(1) as isize)
            as usize;
        let target_range = self.document.line_range(target_line);
        let target = clamp_to_grapheme_boundary(
            self.document.text(),
            target_range.start + column.min(target_range.len()),
        );
        self.move_to_offset(target, extend);
        self.preferred_column = Some(column);
    }

    fn move_to_offset(&mut self, offset: usize, extend: bool) {
        let target = clamp_to_grapheme_boundary(self.document.text(), offset);
        if extend {
            self.selection = TextSelection::new(self.selection.anchor, TextCursor::new(target));
        } else {
            self.selection = TextSelection::new(TextCursor::new(target), TextCursor::new(target));
        }
        self.preferred_x = None;
        if !extend {
            self.preferred_column = None;
        }
    }

    fn set_selection(&mut self, anchor: usize, focus: usize) {
        self.selection = TextSelection::new(
            TextCursor::new(clamp_to_grapheme_boundary(self.document.text(), anchor)),
            TextCursor::new(clamp_to_grapheme_boundary(self.document.text(), focus)),
        );
        self.clear_preferred_position();
    }

    fn clear_composition(&mut self) -> bool {
        let had_composition = self.composition.is_some();
        self.composition = None;
        had_composition
    }

    fn clear_preferred_position(&mut self) {
        self.preferred_x = None;
        self.preferred_column = None;
    }

    fn undo(&mut self) -> bool {
        let Some(edit) = self.undo_stack.pop() else {
            return false;
        };
        let inserted_range = edit.range.start..edit.range.start + edit.inserted.len();
        self.document.replace_range(inserted_range, &edit.removed);
        self.selection = edit.selection_before.clone();
        self.clear_preferred_position();
        self.redo_stack.push(edit);
        true
    }

    fn redo(&mut self) -> bool {
        let Some(edit) = self.redo_stack.pop() else {
            return false;
        };
        let removed_range = edit.range.start..edit.range.start + edit.removed.len();
        self.document.replace_range(removed_range, &edit.inserted);
        self.selection = edit.selection_after.clone();
        self.clear_preferred_position();
        self.undo_stack.push(edit);
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EditorCommand {
    InsertText(String),
    DeleteBackward,
    DeleteForward,
    MoveLeft {
        extend: bool,
    },
    MoveRight {
        extend: bool,
    },
    MoveWordLeft {
        extend: bool,
    },
    MoveWordRight {
        extend: bool,
    },
    MoveLineStart {
        extend: bool,
    },
    MoveLineEnd {
        extend: bool,
    },
    MoveUp {
        extend: bool,
    },
    MoveDown {
        extend: bool,
    },
    PageUp {
        extend: bool,
        lines: usize,
    },
    PageDown {
        extend: bool,
        lines: usize,
    },
    MoveTo {
        offset: usize,
        extend: bool,
    },
    SetSelection {
        anchor: usize,
        focus: usize,
    },
    SelectAll,
    Copy,
    Cut,
    Paste(String),
    Undo,
    Redo,
    StartComposition,
    UpdateComposition {
        text: String,
        cursor_range: Option<Range<usize>>,
    },
    CommitComposition(String),
    EndComposition,
    ClearComposition,
    Noop,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct EditorCommandResult {
    pub handled: bool,
    pub text_changed: bool,
    pub selection_changed: bool,
    pub composition_changed: bool,
    pub clipboard_text: Option<String>,
}

impl EditorCommandResult {
    pub(crate) fn layout_changed(&self) -> bool {
        self.text_changed || self.composition_changed
    }

    pub(crate) fn overlay_changed(&self) -> bool {
        self.selection_changed || self.composition_changed
    }
}

pub(crate) fn selection_range(selection: &TextSelection, text_len: usize) -> Range<usize> {
    let start = selection.anchor.utf8_offset.min(text_len);
    let end = selection.focus.utf8_offset.min(text_len);
    if start <= end { start..end } else { end..start }
}

pub(crate) fn clamp_to_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() || text.is_empty() {
        return offset;
    }

    let mut previous = 0;
    for (index, _) in text.grapheme_indices(true) {
        if index == offset {
            return offset;
        }
        if index > offset {
            return previous;
        }
        previous = index;
    }
    text.len()
}

pub(crate) fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_to_grapheme_boundary(text, offset);
    if offset == 0 {
        return 0;
    }

    text[..offset]
        .grapheme_indices(true)
        .next_back()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

pub(crate) fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_to_grapheme_boundary(text, offset);
    if offset >= text.len() {
        return text.len();
    }

    text[offset..]
        .graphemes(true)
        .next()
        .map(|grapheme| offset + grapheme.len())
        .unwrap_or(text.len())
}

fn previous_word_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_to_grapheme_boundary(text, offset);
    let mut previous_start = 0;
    for (start, word) in text.unicode_word_indices() {
        if start >= offset {
            break;
        }
        let end = start + word.len();
        if offset > start && offset <= end {
            return start;
        }
        previous_start = start;
    }
    previous_start
}

fn next_word_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_to_grapheme_boundary(text, offset);
    for (start, word) in text.unicode_word_indices() {
        let end = start + word.len();
        if offset < start {
            return start;
        }
        if offset >= start && offset < end {
            return end;
        }
    }
    text.len()
}

fn line_starts_for(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_tracks_line_offsets_and_dirty_ranges() {
        let mut document = EditorDocument::from_text("alpha\nbeta\ngamma");
        assert_eq!(document.line_count(), 3);
        assert_eq!(document.line_range(1), 6..10);
        assert_eq!(document.line_text(2), "gamma");

        document.clear_dirty();
        document.replace_range(6..10, "beta\ninserted");

        assert_eq!(document.text(), "alpha\nbeta\ninserted\ngamma");
        assert_eq!(document.line_count(), 4);
        assert_eq!(document.line_index_for_offset(17), 2);
        assert_eq!(document.dirty_line_range(), 1..3);
    }

    #[test]
    fn document_accumulates_dirty_coverage_until_cleared() {
        let mut document = EditorDocument::from_text("alpha\nbeta\ngamma");
        document.clear_dirty();

        document.replace_range(1..1, "XYZ");
        assert_eq!(document.dirty_line_range(), 0..1);
        assert_eq!(document.line_offsets_dirty_from(), Some(1));

        let gamma = document.line_range(2);
        document.replace_range(gamma.start..gamma.start + 1, "G");
        assert_eq!(document.dirty_line_range(), 0..3);
        assert_eq!(document.line_offsets_dirty_from(), Some(1));

        document.clear_dirty();
        assert!(document.dirty_line_range().is_empty());
        assert_eq!(document.line_offsets_dirty_from(), None);
    }

    #[test]
    fn batched_line_count_changes_keep_conservative_dirty_coverage() {
        let mut document = EditorDocument::from_text("alpha\nbeta\ngamma");
        document.clear_dirty();

        let gamma = document.line_range(2);
        document.replace_range(gamma.start..gamma.start + 1, "G");
        assert_eq!(document.line_offsets_dirty_from(), None);
        document.replace_range(1..1, "one\ntwo\n");

        assert_eq!(document.dirty_line_range(), 0..document.line_count());
        assert_eq!(document.line_offsets_dirty_from(), None);
    }

    #[test]
    fn incremental_line_index_matches_full_rebuild_across_boundary_edits() {
        let mut document = EditorDocument::from_text("alpha\nbeta\ngamma\ndelta");
        let edits = [(8..8, "X\nY"), (5..12, "\nreplacement\n"), (0..5, "α")];

        for (range, replacement) in edits {
            document.replace_range(range, replacement);
            assert_eq!(document.line_starts, line_starts_for(document.text()));
            for line_index in 0..document.line_count() {
                let range = document.line_range(line_index);
                assert!(document.text().is_char_boundary(range.start));
                assert!(document.text().is_char_boundary(range.end));
            }
        }

        let end = document.len();
        document.replace_range(end..end, "\n終端");
        assert_eq!(document.line_starts, line_starts_for(document.text()));
    }

    #[test]
    fn incremental_line_index_stays_correct_for_large_documents() {
        let text = (0..4_096)
            .map(|index| format!("line-{index:04}-{}", "x".repeat(index % 31)))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = EditorDocument::from_text(text);

        for replacement in ["候補", "候補\nsecond line", ""] {
            let line = 3_900;
            let range = document.line_range(line);
            let edit_start = range.start + (range.len() / 2);
            document.replace_range(edit_start..edit_start, replacement);
            assert_eq!(document.line_starts, line_starts_for(document.text()));
        }
    }

    #[test]
    fn editing_uses_grapheme_boundaries_for_delete_and_movement() {
        let mut state = EditorState::from_text("a🇯🇵e\u{301}z");
        state.execute(EditorCommand::MoveLeft { extend: false });
        state.execute(EditorCommand::DeleteBackward);

        assert_eq!(state.document().text(), "a🇯🇵z");

        state.execute(EditorCommand::DeleteBackward);

        assert_eq!(state.document().text(), "az");
    }

    #[test]
    fn transactions_support_undo_redo_and_clipboard_commands() {
        let mut state = EditorState::from_text("hello world");
        state.execute(EditorCommand::SetSelection {
            anchor: 6,
            focus: 11,
        });
        let cut = state.execute(EditorCommand::Cut);
        assert_eq!(cut.clipboard_text.as_deref(), Some("world"));
        assert_eq!(state.document().text(), "hello ");

        state.execute(EditorCommand::Undo);
        assert_eq!(state.document().text(), "hello world");

        state.execute(EditorCommand::Redo);
        assert_eq!(state.document().text(), "hello ");

        state.execute(EditorCommand::Paste("there".to_string()));
        assert_eq!(state.document().text(), "hello there");
    }

    #[test]
    fn composition_is_overlay_until_commit() {
        let mut state = EditorState::from_text("hello world");
        state.execute(EditorCommand::SetSelection {
            anchor: 6,
            focus: 11,
        });
        state.execute(EditorCommand::StartComposition);
        state.execute(EditorCommand::UpdateComposition {
            text: "世界".to_string(),
            cursor_range: Some(3..6),
        });

        assert_eq!(state.document().text(), "hello world");
        assert_eq!(state.display_text(), "hello 世界");
        assert_eq!(
            state.display_selection().focus.utf8_offset,
            "hello 世界".len()
        );

        state.execute(EditorCommand::CommitComposition("世界".to_string()));
        assert_eq!(state.document().text(), "hello 世界");
    }

    #[test]
    fn command_layer_supports_word_and_logical_line_movement() {
        let mut state = EditorState::from_text("one two\nthree four");
        state.execute(EditorCommand::MoveLineStart { extend: false });
        assert_eq!(state.selection().focus.utf8_offset, 8);

        state.execute(EditorCommand::MoveWordLeft { extend: false });
        assert_eq!(state.selection().focus.utf8_offset, 4);

        state.execute(EditorCommand::MoveDown { extend: false });
        assert_eq!(state.selection().focus.utf8_offset, 12);

        state.execute(EditorCommand::PageUp {
            extend: true,
            lines: 10,
        });
        assert_eq!(state.selection_range(), 4..12);
    }
}
