# Input and Text Editing

[Previous: widgets and layout](widgets-and-layout.md) · [API guide](README.md) ·
[Next: state and events](state-events-and-async.md)

SUI's input widgets share the same retained editor behavior: keyboard and IME
text entry, pointer and keyboard selection, a visible caret and selection,
clipboard operations, and editable accessibility semantics. Applications own
the resulting domain values; widgets own the transient editing mechanics.

## Choose the Right Field

| Widget | Value contract | Important behavior |
| --- | --- | --- |
| `TextInput` | One `String` line | Newlines are removed from initial, pasted, and programmatic values |
| `PasswordInput` | One real `String` line | Rendering is masked and semantics mark it as a password |
| `DateTimeInput` | One `String` line | Suggested `YYYY-MM-DD HH:MM`; parsing and timezone policy are application-owned |
| `TextArea` | Multiline `String` | Supports line breaks and optional submit-on-Enter behavior |
| `TextSurface` | Multiline text/editor surface | Use for richer editor overlays, spans, and document-style behavior |
| `NumberInput` / `SpinBox` | `f64` | Range, step, precision, keyboard editing, and steppers |

`MultilineTextInput` is an alias of `TextArea`.

The first constructor argument is the accessible name. Placeholder text is a
visual editing hint; it is not a replacement for a durable name.

## Tutorial: Capture a Form Draft

```rust
use std::{cell::RefCell, rc::Rc};
use sui::prelude::*;

#[derive(Default)]
struct Draft {
    name: String,
    password: String,
    reminder: String,
    notes: String,
}

fn profile_form() -> impl Widget {
    let draft = Rc::new(RefCell::new(Draft::default()));

    let name_draft = Rc::clone(&draft);
    let name = TextInput::new("Display name")
        .placeholder("Ada Lovelace")
        .on_change(move |value| name_draft.borrow_mut().name = value);

    let password_draft = Rc::clone(&draft);
    let password = PasswordInput::new("Password")
        .placeholder("At least 12 characters")
        .on_change(move |value| password_draft.borrow_mut().password = value);

    let reminder_draft = Rc::clone(&draft);
    let reminder = DateTimeInput::new("Reminder time")
        .on_change(move |value| reminder_draft.borrow_mut().reminder = value);

    let notes_draft = Rc::clone(&draft);
    let notes = TextArea::new("Notes")
        .placeholder("Optional notes")
        .min_height(120.0)
        .on_change(move |value| notes_draft.borrow_mut().notes = value);

    let submit_draft = Rc::clone(&draft);
    let submit = Button::new("Save").on_press(move || {
        let draft = submit_draft.borrow();
        // Validate and hand the draft to application code. Do not log the
        // password or keep extra plaintext copies.
        let _is_valid = !draft.name.trim().is_empty()
            && draft.password.chars().count() >= 12;
    });

    Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(name)
        .with_child(password)
        .with_child(reminder)
        .with_child(notes)
        .with_child(submit)
}
```

`value(...)` sets the initial edit buffer. `current_value()` returns the
widget's current buffer, and `set_value(...)` replaces it when an owning custom
widget has mutable access. `on_change` receives an owned string after edits;
`on_change_with_ctx` additionally lets the callback invalidate other output.

## Selection and Clipboard

Focused text fields support ordinary platform editing gestures:

- Pointer click and drag place the caret or select a range.
- Shift-modified navigation extends keyboard selection.
- Select all, copy, cut, and paste use the runtime clipboard service.
- Clipboard and semantic edit operations honor `read_only`.
- Selection and caret offsets use UTF-8 byte positions internally while text
  movement respects grapheme boundaries.

The concrete input types expose `selected_text`, `select_all`, `copy`, `cut`,
and `paste` for a composite widget that directly owns the field and has an
`EventCtx`. Application menus should normally route a command to the retained
editor instead of reaching into it:

```rust,ignore
// `editor_id` is the WidgetId of the retained input or text surface.
ctx.post_event(editor_id, TextCommand::Paste.into_event());
```

`TextCommand::{Cut, Copy, Paste, SelectAll}` is understood by `TextInput`,
`TextArea`, and `TextSurface`. Posting the event after a context-menu action
keeps focus and editor ownership in one place.

## Shared Selection Scope

Each editable field works without a `SelectionScope`. Create and clone a scope
only when labels, rich text, or editor surfaces must participate in one
coordinated application selection:

```rust
use sui::prelude::*;

fn selectable_document() -> impl Widget {
    let selection = SelectionScope::new();

    Stack::vertical()
        .spacing(8.0)
        .with_child(Label::new("Selectable heading").selectable(selection.clone()))
        .with_child(
            TextArea::new("Document body")
                .value("Select text across the document surface.")
                .selection_scope(selection),
        )
}
```

The scope tracks selection owners, ordering, payloads, and the active text
range. It can also represent image or application-defined selections, so it is
broader than an edit-buffer cursor.

## IME and Keyboard Text

The platform normalizes composed text into `ImeEvent` values. Built-in fields
handle composition start, update, commit, and end, including the composition
rectangle used by native candidate windows. Do not implement text entry by
concatenating `KeyboardEvent.key`; that fails for dead keys, input methods, and
many international layouts.

Keyboard events remain important for navigation and commands. Custom editors
should separate navigation/shortcut handling from committed text insertion in
the same way.

## Multiline Submit Behavior

By default, Enter inserts a newline in `TextArea`. Adding `on_submit` changes
plain Enter into a submit action while modified Enter, including Shift+Enter,
continues to insert a line break:

```rust
use sui::prelude::*;

fn message_composer() -> impl Widget {
    TextArea::new("Message")
        .placeholder("Write a message")
        .min_height(96.0)
        .on_submit(|message| {
            if !message.trim().is_empty() {
                // Send through the application's command layer.
            }
        })
}
```

The callback borrows the current text. Clear or replace the field from its
owner after the application accepts the message.

## Read-only and Selectable Content

Call `read_only()` when users may focus and copy a value but must not modify
it. Use a selectable `Label`, `RichText`, or `TextSurface` when the content is
display-oriented rather than a form field. Read-only state should also remain
visible in semantics so assistive technologies do not advertise unavailable
edit actions.

## Password Security Boundary

`PasswordInput` masks graphemes on screen and sets the editable password flag
used by platform accessibility adapters. Its Rust value and change callback
still contain plaintext. The internal semantics snapshot also retains the
actual editable value so the runtime and testing layer can represent the field
consistently; platform adapters are responsible for secure presentation.

Therefore:

- Do not log, serialize, or include password state in ordinary debug output.
- Treat screenshots as visual masking, not proof that memory or automation
  snapshots contain no plaintext.
- Move accepted credentials promptly into the application's secure handling
  path and avoid redundant copies.

## Date and Time Boundary

`DateTimeInput` does not claim a locale, calendar, UTC offset, or timezone. It
is a text-editing convenience with a suggested placeholder. Validate the
string on change or submission and convert it to the domain type chosen by the
application. If the product needs a calendar popover, locale-specific
formatting, or ambiguity handling, compose that UI around the field rather
than relying on implicit parsing.
