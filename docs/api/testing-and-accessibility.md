# Testing and Accessibility

[Previous: custom widgets](custom-widgets.md) · [API guide](README.md) ·
[Next: platforms and features](platforms-and-features.md)

SUI uses one semantics tree for accessibility adapters, UI automation, and
high-level tests. Treat that tree as a public user-facing contract: roles,
names, values, state, and actions should describe what a user can perceive and
operate.

## Add the Test Harness

`sinomo-ui-testing` is a separate package rather than a `sui` module:

```toml
[dev-dependencies]
sinomo-ui-testing = "0.2"
```

Import application types and test helpers independently:

```rust,ignore
use sui::prelude::*;
use sui_testing::prelude::*;
```

`TestApp` accepts a closure that builds a `Runtime`, `Application`, or another
`IntoTestRuntime` result. `App::build()` is the normal facade-level bridge.

## Tutorial: Test a Form Through Semantics

```rust
use std::sync::{Arc, Mutex};

use sui::prelude::*;
use sui::{SemanticsRole};
use sui_testing::prelude::*;

#[derive(Default)]
struct FormState {
    name: String,
    saved_name: Option<String>,
}

fn form(state: Arc<Mutex<FormState>>) -> impl Widget {
    let input_state = Arc::clone(&state);
    let input = TextInput::new("Name").on_change(move |value| {
        input_state.lock().expect("form state").name = value;
    });

    let save_state = Arc::clone(&state);
    let save = Button::new("Save").on_press(move || {
        let mut state = save_state.lock().expect("form state");
        state.saved_name = Some(state.name.clone());
    });

    Stack::vertical()
        .spacing(8.0)
        .with_child(input)
        .with_child(save)
}

#[test]
fn saves_a_name() -> Result<()> {
    let state = Arc::new(Mutex::new(FormState::default()));
    let app_state = Arc::clone(&state);
    let app = TestApp::new(move || {
        App::new()
            .main_window("Profile", form(app_state))
            .build()
    })?;
    let window = app.main_window()?;

    let name = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name("Name");
    name.fill("Ada")?;
    name.expect().to_have_value("Ada")?;

    window
        .get_by_role(SemanticsRole::Button)
        .with_name("Save")
        .click()?;

    assert_eq!(
        state.lock().expect("form state").saved_name.as_deref(),
        Some("Ada")
    );
    Ok(())
}
```

This test does not know the widget's Rust type, tree index, or pixel
coordinates. The locator re-resolves against the latest semantics snapshot,
and the expectation pumps events and redraws until it succeeds or reaches its
timeout.

## Locator API

Start from a `TestWindow`:

- `get_by_role(role)` queries semantic role.
- `get_by_text(text)` queries user-visible semantic text.
- `get_by_description(text)` queries accessible description.
- `focused()` locates the current focus target.
- `locator(Selector)` supports a fully constructed selector.

Refine and scope a `Locator` with:

- `with_name(...)` and `with_description(...)`.
- Nested `get_by_*` calls, which search within the prior locator's semantics
  subtree.
- `count()` when multiple matches are intentional.

Actions require one unique, visible, enabled target. Common actions are
`click`, `touch_tap`, `hover`, `focus`, `press`, `fill`, `scroll_pixels`, and
`scroll_lines`. `fill` sends an IME composition sequence, so it exercises the
same committed-text boundary used by real platform input. It inserts at the
current selection; it does not first select all and clear a non-empty field.

## Auto-waiting Expectations

`locator.expect()` produces an `Expectation` with retrying assertions:

- `to_be_visible()` and `to_be_hidden()`.
- `to_be_focused()`.
- `to_have_text(...)` and `to_have_value(...)`.
- `to_have_count(...)`.
- `to_match_screenshot(path)`.

Set a focused timeout with `with_timeout(seconds)`, or configure the app-wide
default through `TestApp::set_default_timeout`. Do not add sleeps: expectations
already pump the runtime, timers, wakeups, and redraw work while waiting.

## Deterministic Time and Raw Events

`TestApp` and `TestWindow` expose:

- `run_until_idle()` to process queued work.
- `pump_frames(count)` to advance a known number of rendered frames.
- `advance_time(seconds)` to drive timers and time-based behavior.
- `dispatch_event_now(event)` for tests specifically about normalized event
  details.

Use high-level locator actions for normal user flows. Direct event dispatch is
appropriate for a custom pointer kind, exact modifier state, window lifecycle
event, or runtime boundary that no high-level action represents.

`TestApp::new` uses the live backend when a display is available and otherwise
falls back to the headless harness. `TestApp::from_runtime` is the explicit
headless path. `new_no_vsync` and `new_visible_no_vsync` are useful when a live
test needs controlled presentation timing.

## Screenshots and Diagnostic Artifacts

Semantics should be the default assertion surface. Capture pixels only for
visual behavior that roles, state, and values cannot describe.

```rust,ignore
let screenshot = window.capture_screenshot()?;
screenshot.write_png("target/ui-artifacts/profile.png")?;

window
    .get_by_role(SemanticsRole::Dialog)
    .with_name("Delete account")
    .expect()
    .to_match_screenshot("tests/baselines/delete-dialog.png")?;
```

`capture_artifacts()` returns the current window snapshot plus available
screenshot, semantics overlay, and widget overlay. `performance_snapshot()`
captures runtime timing diagnostics. Keep baseline tests small and focused;
when a screenshot differs, the matcher writes actual and diff images next to
the expected baseline.

## Live Application Inspector

`sinomo-ui-runtime` exposes one renderer-neutral snapshot for development
tools:

```rust,ignore
runtime.set_inspector_tracing(window_id, true)?;
let snapshot = runtime.inspector_snapshot(window_id)?;
```

`WindowInspectorSnapshot` includes the semantic tree, retained widget tree and
lifetime-stable IDs, focus, pending frame work, overlay ownership, timers,
animation-frame requests, async registrations, the most recent render
diagnostics, paint damage, and bounded histories for event routes,
invalidations, commands, reactive changes, and widget rebuild reasons. Event
route history records only event categories and widget IDs; it deliberately
does not retain typed text or event payloads.

Tracing is opt-in because route and history capture adds work to event and
render paths. Structural snapshots remain available while tracing is off, and
disabling tracing clears the retained histories:

```rust,ignore
runtime.set_inspector_tracing(window_id, false)?;
```

Render a point-in-time snapshot with `sui_debug::inspector_snapshot_view`. For
a separate live inspector window, create `sui_debug::InspectorState`, use
`live_inspector_view(state.clone())` as that window's root, and publish a fresh
snapshot after the inspected window processes work. Keeping the inspector in a
separate window prevents its own widget and paint activity from obscuring the
application being diagnosed.

Widget IDs are stable for the lifetime of a retained `WidgetPod`; they are not
persistent application or document IDs. Widget implementations may override
`Widget::diagnostics` to expose compact operational counters:

```rust,ignore
fn diagnostics(&self, ctx: &mut WidgetDiagnosticsCtx) {
    ctx.record("cached rows", self.cache.len().to_string());
}
```

The hook runs only when `inspector_snapshot` is called. Do not expose user
content, secrets, or large state dumps through it. `VirtualList` uses this hook
for loaded and visible ranges, realized and cached row counts, scroll state,
follow-end state, and source revision.

## Accessibility Contract

Built-in widgets emit semantics automatically. A custom interactive widget
must keep these fields coherent:

| Semantic field | Contract |
| --- | --- |
| `role` | Closest available user-facing role |
| `name` | Stable, concise accessible label |
| `description` | Optional supporting instruction, not duplicate name |
| `value` | Current text, number, or range value |
| `state` | Disabled, focused, checked, selected, expanded, busy, and similar state |
| `actions` | Only operations the widget handles |
| `bounds` | Actionable visual target in window coordinates |
| `editable_text` | Caret, selection, multiline/password/read-only flags, and scroll offsets |

The platform routes `SemanticsActionRequest` back to the retained widget that
owns a node. A widget that advertises `Activate`, `SetValue`, `Increment`, or
another action must implement the equivalent event path. Keyboard, pointer,
and assistive-technology activation should converge on the same behavior.

## Accessible Names and Grouping

- Button labels and field constructor names become accessible names by
  default.
- Placeholder text is transient and is not a field label.
- Icon-only actions need explicit semantic names.
- Use `SemanticRegion` to add a named group when visual containment alone does
  not express the relationship.
- Use unique role/name pairs inside repeated demo or test surfaces so locators
  remain unambiguous.
- Keep hidden, disabled, selected, expanded, and focus state synchronized with
  what the user sees.

Text fields expose selection and edit actions through editable semantics.
Password fields set the password flag for platform adapters; see the
[password security boundary](input-and-editing.md#password-security-boundary)
before inspecting or persisting internal snapshots.

## What to Test

For each new interactive widget, cover at least:

1. Pointer or touch activation.
2. Keyboard focus and activation/navigation.
3. Its semantic role, name, value, and state.
4. Its `SemanticsActionRequest` path.
5. Disabled or read-only behavior where applicable.
6. One state transition that proves invalidation updates the visible and
   semantic outputs.

Add a visual test only when the behavior cannot be asserted reliably through
those contracts.
