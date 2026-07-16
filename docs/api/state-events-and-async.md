# State, Events, and Background Work

[Previous: input and text editing](input-and-editing.md) · [API guide](README.md) ·
[Next: themes and resources](themes-and-resources.md)

SUI keeps widgets and their interaction state on the UI thread. Rust
application code chooses where domain state lives; the facade does not impose
a global store. The optional `Signal<T>` implementation and the
architecture-neutral `Observable<T>` trait connect changing application data
to retained widgets without requiring a particular store design.

## Two Kinds of State

Built-in widgets retain short-lived interaction state themselves: hover and
press state, focus animation, text selections, caret position, scroll offset,
and similar details. Application state usually lives outside the widget in an
`Rc<Cell<T>>`, `Rc<RefCell<T>>`, an application model, or a message queue.

Connect external state through two complementary APIs:

- An observable binding such as `Label::text_from`,
  `TabBar::selected_from`, or `SwitchView::selected_from` subscribes the
  retained widget to targeted automatic invalidation.
- A reader builder such as `Label::text_when`, `Slider::value_when`,
  `Select::selected_when`, `Button::enabled_when`, or `SwitchView::selected_when`
  reads the current value when the relevant runtime phase runs. Readers remain
  useful for adapting existing state that does not emit notifications.
- A callback such as `on_press`, `on_change`, `on_toggle`, or a
  `*_with_ctx` variant updates the application model.

The `*_with_ctx` callbacks additionally receive `&mut EventCtx`, allowing the
callback to invalidate other observable output after changing shared state.

## Tutorial: External State Without Rebuilding

```rust
use sui::prelude::*;

fn counter() -> impl Widget {
    let count = Signal::named("counter", 0_i32);
    let label_text = count.select_named("counter label", |count| {
        format!("Count: {count}")
    });

    let label = Label::new("Count: 0").text_from(label_text);

    let increment_count = count.clone();
    let increment = Button::new("Increment").on_press(move || {
        increment_count.update(|count| *count += 1);
    });

    Stack::vertical()
        .spacing(8.0)
        .with_child(label)
        .with_child(increment)
}
```

The tree remains retained: the label and button are not recreated for each
increment. The selector suppresses notifications when its derived value is
unchanged, and the label automatically requests text measurement, paint, and
semantics when the selected value changes.

Signal writes from a background thread wake the running platform event loop.
Writes made in an event callback join the current runtime turn. Repeated
changes are coalesced by the platform wake path and equality checks.

Reader closures must be fast, deterministic, and non-blocking. They run on the
UI thread and may be evaluated in more than one phase. Do not perform I/O,
network access, or expensive parsing inside them.

## Observing Values in Custom Widgets

Widget contexts can subscribe directly to any `Observable<T>`:

```rust
fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
    let document = ctx.observe_with(&self.document, InvalidationKind::Text);
    measure_document(ctx, constraints, &document)
}

fn paint(&self, ctx: &mut PaintCtx) {
    let highlight = ctx.observe(&self.highlight);
    paint_highlight(ctx, highlight);
}
```

`MeasureCtx::observe`, `ArrangeCtx::observe`, `PaintCtx::observe`, and
`SemanticsCtx::observe` infer their phase's invalidation kind.
`observe_with` declares a different consequence explicitly. `EventCtx::observe`
always accepts an explicit invalidation kind because event handling alone does
not reveal which cached output depends on the value.

Subscriptions belong to the retained `WidgetPod` identity and are released
when that pod is dropped.

## Controlled and Locally Retained Widgets

Not every widget has a reader for every property.

- `TextInput`, `PasswordInput`, `DateTimeInput`, and `TextArea` own their live
  edit buffer. Use `value(...)` for the initial text and `on_change(...)` to
  mirror edits into application state. If an owner has mutable access to the
  widget, `set_value(...)` replaces the buffer programmatically.
- Controls such as `Checkbox` and `Switch` retain their current toggle state
  and report changes with `on_toggle`.
- Controls with `value_when` or `selected_when` should use the reader as the
  authoritative value and update that same external state from the callback.

Rebuilding a subtree resets its local focus, animation, selection, and editing
state. Use `RebuildOnChange` for genuinely structural changes, not as the
default way to update a label or selected value.

When a structural key is observable, prefer
`RebuildOnChange::new_observable(selector, build)`. It subscribes the host,
wakes the runtime, rebuilds only when the selected key changes, and reports the
reason through rebuild diagnostics. The closure-based constructor remains
available for existing state adapters that are polled during a runtime pass.

For dynamic collections, `KeyedChildren<K, T>` reconciles items by stable key.
Existing `WidgetPod`s move into their new order instead of being recreated.
Each entry exposes a per-item `Signal<T>`, allowing changed row data to update
the retained row widget while preserving focus, selection, and animation.

## Reactive Diagnostics

Every `RenderOutput` includes:

- `diagnostics.reactive_invalidations`, identifying the source name, version,
  target widget, invalidation kind, and whether the target was active.
- `diagnostics.widget_rebuilds`, recording structural replacement reasons
  reported by `RebuildOnChange`, `RebuildOnConstraints`, or custom widgets
  through `ctx.record_rebuild(...)`.

Name application-facing signals and selectors with `Signal::named` and
`select_named` so diagnostics explain the state dependency rather than showing
only a generic source label.

## Event Delivery

Custom widgets receive normalized `Event` values through `Widget::event`.
Important variants include:

- `Event::Pointer` for mouse, pen, and touch movement, buttons, and scrolling.
- `Event::Keyboard` for key transitions and modifiers.
- `Event::Ime` for text composition and commits. Text editors should consume
  IME commits rather than trying to derive text solely from key names.
- `Event::Semantics` for actions requested by assistive technology.
- `Event::Wake` for timers, async wake tokens, and animation frames.
- `Event::Window` for window or embedded-viewport lifecycle changes.
- `Event::Custom` for application and runtime-defined messages.

Pointer and focus-routed events can travel through capture, target, and bubble
phases. Inspect `ctx.phase()` only when a container needs phase-specific
behavior. Call `ctx.set_handled()` after consuming an action so later routing
does not treat it as unhandled.

## Request the Narrowest Correct Invalidation

Mutating Rust state does not by itself tell the runtime which cached work is
stale. Use `EventCtx` requests to describe the consequence:

| Change | Request |
| --- | --- |
| Preferred size or child measurement | `request_measure()` |
| Child placement with unchanged measurement | `request_arrange()` |
| Drawn colors, shapes, or other pixels | `request_paint()` or `request_paint_rect(...)` |
| Accessible role, name, value, state, or actions | `request_semantics()` |
| Retained transform/effect/visibility/hit-test state | The corresponding `request_*()` method |
| Text shaping or text resource state | `request_text()` or `request_resources()` |

One state change may require more than one request. A status string that changes
both visible width and accessible text should request measurement and
semantics. A hover color usually needs paint and semantics.

The convenience methods target the widget associated with the current
`EventCtx`. That is correct for a widget changing its own retained state. When
shared state is read by a sibling or a wider subtree, submit an explicit
`InvalidationRequest` for its known `WidgetId`, or target
`InvalidationTarget::Window(ctx.window_id())` when the set of consumers is not
centrally tracked. Window invalidation is broader, so prefer a known widget
target in performance-sensitive components.

## Focus, Pointer Capture, Clipboard, and Posted Events

`EventCtx` also exposes interaction services:

- `request_focus()` and `clear_focus()` change keyboard focus.
- `request_pointer_capture(pointer_id)` keeps a drag routed to the widget;
  release it with `release_pointer_capture(pointer_id)`.
- `clipboard_text()` and `set_clipboard_text(...)` use the platform clipboard
  when available and the runtime fallback otherwise.
- `post_event(target, event)` delivers a command to a particular retained
  widget after the current dispatch completes.

Built-in text inputs already implement focus, selection, IME, and clipboard
behavior. Use these services directly only in a custom interaction.

## Timers and Animation Frames

Use a timer for a specific future deadline:

1. Call `ctx.schedule_timer_after(seconds)` or `schedule_timer_at(deadline)`.
2. Store the returned `TimerToken` in the widget.
3. Match the token in `Event::Wake(WakeEvent::Timer { .. })`.
4. Cancel an obsolete token with `ctx.cancel_timer(token)`.

For continuous motion, call `request_animation_frame()`, update animation state
from the next `WakeEvent::AnimationFrame`, invalidate the changed presentation,
and request another frame only while animation remains active. Do not run a
blocking loop inside `event` or `paint`.

## Tutorial: Deliver Background Results with `UiHandle`

Widgets are not required to be `Send`, and their methods stay synchronous.
Put long-running work on another thread, place results in thread-safe external
state, then wake the UI event loop.

```rust,no_run
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use sui::prelude::*;
use sui::{EXTERNAL_WAKE_KIND, SemanticsNode, SemanticsRole};

struct AsyncStatus {
    inbox: Arc<Mutex<VecDeque<String>>>,
    text: String,
}

impl AsyncStatus {
    fn new(inbox: Arc<Mutex<VecDeque<String>>>) -> Self {
        Self {
            inbox,
            text: "Loading…".to_string(),
        }
    }
}

impl Widget for AsyncStatus {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Custom(custom) = event else { return };
        if custom.kind != EXTERNAL_WAKE_KIND {
            return;
        }

        {
            let mut inbox = self.inbox.lock().expect("background result queue");
            while let Some(message) = inbox.pop_front() {
                self.text = message;
            }
        }

        ctx.request_measure();
        ctx.request_semantics();
        ctx.set_handled();
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(240.0, 40.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.label(ctx.bounds(), self.text.clone(), Color::BLACK);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Text,
            ctx.bounds(),
        );
        node.name = Some(self.text.clone());
        ctx.push(node);
    }
}

fn main() -> Result<()> {
    let inbox = Arc::new(Mutex::new(VecDeque::new()));
    let worker_inbox = Arc::clone(&inbox);

    App::new()
        .main_window("Background work", AsyncStatus::new(inbox))
        .run_with_handle(move |ui| {
            std::thread::spawn(move || {
                // Replace this with blocking I/O or CPU work.
                worker_inbox
                    .lock()
                    .expect("background result queue")
                    .push_back("Loaded".to_string());
                ui.wake();
            });
        })
}
```

`UiHandle::wake` is the cross-thread signal; it does not carry the result and
does not mutate widgets. The platform delivers an external-wake custom event to
window roots, and the root drains the application-owned queue. For multiple
windows, route queued work in the application model to the appropriate root.

`UiHandle` is available only with a platform event-loop feature. Headless code
can drive `Runtime` or use `sinomo-ui-testing` instead.
