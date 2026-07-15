# Build a Stateful Form

This tutorial builds a practical profile form with editable text, a masked
password, a local date/time field, dynamic validation, and a submit callback.
The complete runnable source is
[`crates/sui/examples/stateful_form.rs`](../../crates/sui/examples/stateful_form.rs).

Run the repository example with:

```bash
cargo run -p sinomo-ui --example stateful_form
```

## What this example teaches

The form demonstrates four important SUI patterns:

1. Editable widgets own transient editing details such as the caret and text
   selection.
2. Application data lives outside the widget in an `Rc<RefCell<T>>` on the UI
   thread.
3. Input callbacks copy completed values into that application model.
4. Dynamic readers and explicit invalidation update dependent widgets without
   rebuilding the editable inputs.

SUI widgets are UI-thread-owned and do not need to be `Send`. `Rc<RefCell<T>>`
is therefore a compact choice for local application state. For background work,
use a channel or synchronized queue and wake the UI through `UiHandle` instead.

## 1. Define application state

The example keeps only domain values and submission state in the model:

```rust
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
struct FormState {
    name: String,
    password: String,
    scheduled_for: String,
    submissions: u32,
}

impl Default for FormState {
    fn default() -> Self {
        Self {
            name: String::new(),
            password: String::new(),
            scheduled_for: "2026-07-18 10:00".to_string(),
            submissions: 0,
        }
    }
}

impl FormState {
    fn can_submit(&self) -> bool {
        !self.name.trim().is_empty()
            && self.password.chars().count() >= 8
            && !self.scheduled_for.trim().is_empty()
    }

    fn status(&self) -> String {
        if self.submissions == 0 {
            format!(
                "Draft for {} · {} password characters · scheduled {}",
                display_name(&self.name),
                self.password.chars().count(),
                self.scheduled_for
            )
        } else {
            format!(
                "Saved {} time(s) for {} at {}",
                self.submissions,
                display_name(&self.name),
                self.scheduled_for
            )
        }
    }
}

fn display_name(name: &str) -> &str {
    if name.trim().is_empty() {
        "an unnamed user"
    } else {
        name.trim()
    }
}

let state = Rc::new(RefCell::new(FormState::default()));
```

Do not add caret position, selection ranges, hover flags, or undo history to
this structure. `TextInput`, `PasswordInput`, and `DateTimeInput` already own
their editing behavior. The application model only needs the resulting values.

## 2. Connect an input callback

Each callback gets its own cheap `Rc` clone:

```rust,ignore
let name_state = Rc::clone(&state);
let name = TextInput::new("Display name")
    .placeholder("Ada Lovelace")
    .min_width(360.0)
    .theme(theme)
    .on_change_with_ctx(move |ctx, value| {
        name_state.borrow_mut().name = value;
        refresh_window(ctx);
    });
```

The constructor argument is the input's accessible name. `placeholder` is
visual guidance, not a replacement for a stable name. `on_change_with_ctx`
provides both the new value and an `EventCtx`; use the simpler `on_change` when
no other widget depends on the change.

Single-line inputs support caret movement, range selection, copy, cut, paste,
and platform IME events. Newlines pasted into a single-line input are removed.

## 3. Use specialized input wrappers

`PasswordInput` has the same callback model as `TextInput` but paints masked
graphemes and exposes password semantics to accessibility adapters:

```rust,ignore
let password_state = Rc::clone(&state);
let password = PasswordInput::new("Password")
    .placeholder("At least eight characters")
    .theme(theme)
    .on_change_with_ctx(move |ctx, value| {
        password_state.borrow_mut().password = value;
        refresh_window(ctx);
    });
```

The callback receives the real value. Treat it as sensitive application data:
do not log it, echo it in a label, or keep it longer than necessary.

`DateTimeInput` is intentionally a lightweight local text field:

```rust,ignore
let initial_schedule = state.borrow().scheduled_for.clone();
let schedule_state = Rc::clone(&state);
let scheduled_for = DateTimeInput::new("Scheduled for")
    .value(initial_schedule)
    .theme(theme)
    .on_change_with_ctx(move |ctx, value| {
        schedule_state.borrow_mut().scheduled_for = value;
        refresh_window(ctx);
    });
```

Its suggested format is `YYYY-MM-DD HH:MM`, but it stores a string. The
application owns parsing, validation, locale display, timezone selection, and
conversion to an instant. This avoids silently guessing a timezone.

## 4. Update dependent readouts

`Label::dynamic` reads current model data instead of owning a fixed string:

```rust,ignore
let status_state = Rc::clone(&state);
let status = Label::dynamic("Draft", move || status_state.borrow().status())
    .theme(theme)
    .color(theme.palette.text_muted);
```

The label may change size, and a button's enabled semantics may also change.
Request a window-level measure, paint, and semantics refresh after modifying the
shared model:

```rust,ignore
use sui::{InvalidationKind, InvalidationRequest, InvalidationTarget};

fn refresh_window(ctx: &mut EventCtx) {
    let target = InvalidationTarget::Window(ctx.window_id());
    for kind in [
        InvalidationKind::Measure,
        InvalidationKind::Paint,
        InvalidationKind::Semantics,
    ] {
        ctx.request(InvalidationRequest::new(target, kind));
    }
}
```

Widget-local helpers such as `ctx.request_paint()` are appropriate when only
the callback owner changed. A window target is deliberate here because the
input changes sibling widgets: the status label and submit button.

This approach preserves the input widget instances. Rebuilding the whole form
on each keystroke would replace their caret, selection, and focus state.

## 5. Make actions depend on model state

`enabled_when` evaluates a reader against the latest model. The activation
callback mutates the same model and refreshes the dependent status label:

```rust,ignore
let enabled_state = Rc::clone(&state);
let submit_state = Rc::clone(&state);

let submit = Button::new("Save profile")
    .theme(theme)
    .enabled_when(move || enabled_state.borrow().can_submit())
    .on_press_with_ctx(move |ctx| {
        submit_state.borrow_mut().submissions += 1;
        refresh_window(ctx);
    });
```

Keep each `RefCell` borrow short. Do not call application callbacks while a
mutable borrow is still active; update the model, let the borrow end, and then
request invalidation.

## 6. Compose the layout

A vertical `Stack` is sufficient for the form fields. A panel surface provides
visual grouping, and a window surface paints the root background:

```rust,ignore
let form = Stack::vertical()
    .spacing(12.0)
    .alignment(Alignment::Start)
    .with_child(Label::new("Create a profile").theme(theme))
    .with_child(name)
    .with_child(password)
    .with_child(scheduled_for)
    .with_child(submit)
    .with_child(status);

let card = Surface::panel(form)
    .theme(theme)
    .padding(Insets::all(20.0));

let root = Surface::window(card)
    .theme(theme)
    .padding(Insets::all(24.0))
    .fill();
```

Use `Alignment::Stretch` when every field should occupy the available cross
axis. Use `Flex` for action rows, toolbars, wrapping controls, or layouts with a
growing content region. `min_width` is a hint for desktop form density; remove
or lower it for narrow/mobile layouts.

## 7. Apply one theme value

The runnable example starts with `DefaultTheme::dark()` and passes that value
to every control and surface. `DefaultTheme` is `Copy`, so this does not require
shared ownership or theme state:

```rust,ignore
let theme = DefaultTheme::dark();
let field = TextInput::new("Name").theme(theme);
let action = Button::new("Save").theme(theme);
```

Use palette tokens such as `theme.palette.text` and
`theme.palette.text_muted` instead of hard-coded colors. Starting from one of
the built-in themes and changing semantic tokens keeps controls, focus rings,
selection, and surfaces coherent.

For a runtime-switchable theme, use the widgets' `theme_when` readers and
invalidate the affected window after replacing the shared theme value.

## Production checklist

Before adapting the example into a real account or scheduling flow:

- validate and normalize text at the domain boundary;
- store password material only in the component that needs it and never include
  it in diagnostic output;
- parse date/time text explicitly and ask the user for a timezone when an
  instant is required;
- surface validation errors with visible text and accessible descriptions;
- move blocking I/O off the UI thread and wake the UI after results arrive;
- test controls by accessible role and name rather than widget-tree position.

Check both examples whenever tutorial code changes:

```bash
cargo check -p sinomo-ui --example quickstart
cargo check -p sinomo-ui --example stateful_form
```
