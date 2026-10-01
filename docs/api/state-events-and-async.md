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
  `TabBar::selected_from`, `Button::enabled_from`, or `SwitchView::selected_from`
  subscribes the retained widget to targeted automatic invalidation.
- A reader builder such as `Label::text_when`, `Slider::value_when`,
  `Select::selected_when`, `Button::enabled_when`, or `SwitchView::selected_when`
  reads the current value when the relevant runtime phase runs. Readers remain
  useful for adapting existing state that does not emit notifications.
- A callback such as `on_press`, `on_change`, or a
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
        .gap(8.0)
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

Dependencies are reconciled after each completed widget phase. If a conditional
reader switches from one observable to another, the old source stops scheduling
that phase. Dependencies from cached or skipped phases remain active, including
sources shared by multiple phases. Read every dependency used by the current
phase even when the widget reuses its own internal cached work. Observations
registered explicitly from event or command handlers remain until widget teardown.

Subscriptions belong to the retained `WidgetPod` identity and are released
when that pod is dropped.

## Controlled and Locally Retained Widgets

Not every widget has a reader for every property.

- `TextInput`, `PasswordInput`, `DateTimeInput`, and `TextArea` own their live
  edit buffer. Use `value(...)` for the initial text and `on_change(...)` to
  mirror edits into application state. If an owner has mutable access to the
  widget, `set_value(...)` replaces the buffer programmatically.
- Two-state controls, `Checkbox`, `Switch`, and `RadioButton`, retain their
  checked state and report changes with `on_change`.
- Controls with `value_when`, `checked_when`, or `selected_when` should use the reader as the
  authoritative value and update that same external state from the callback.

Rebuilding a subtree resets its local focus, animation, selection, and editing
state. Use `RebuildOnChange` for genuinely structural changes, not as the
default way to update a label or selected value.

When a structural key is observable, prefer
`RebuildOnChange::key_from(selector, build)`. It subscribes the host,
wakes the runtime, rebuilds only when the selected key changes, and reports the
reason through rebuild diagnostics. `RebuildOnChange::key_when(key_fn, build)`
remains available for existing state adapters that are polled during a runtime
pass.

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
- `diagnostics.command_dispatches`, recording the typed command name, payload
  type, target, delivery mode, invoked handlers, and whether delivery succeeded.
- `diagnostics.invalidations`, recording which event, reactive source, or
  command invalidated a target and an optional controller-provided reason.

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
- `Event::Custom` for explicitly widget-routed extension events. Prefer typed
  commands for application services and cross-thread messages.

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

- `request_focus()` and `clear_focus()` change keyboard focus. A pointer press
  focuses the widget pressed, or clears focus when it cannot take it;
  `keep_focus()` leaves focus where it is instead, for a control that acts on
  what has focus, as `Button::focus_on_press(false)` does.
- `request_pointer_capture(pointer_id)` keeps a drag routed to the widget;
  release it with `release_pointer_capture(pointer_id)`.
- `clipboard_text()` and `set_clipboard_text(...)` use the platform clipboard
  when available and the runtime fallback otherwise.
- `post_event(target, event)` delivers an event to a particular retained widget
  after the current dispatch completes.
- `post_command(target, key, payload)` delivers a strongly typed command to a
  particular retained widget after the current dispatch completes.
- `command_sender()` returns the runtime's typed command producer. Use it from
  a widget callback to send to window/application controllers or to start an
  application-wide broadcast without routing through an ancestor widget.

Built-in text inputs already implement focus, selection, IME, and clipboard
behavior. Use these services directly only in a custom interaction.

## Timers and Animation Frames

Use a timer for a specific future deadline:

1. Call `ctx.schedule_timer_after(seconds)` or `schedule_timer_at(deadline)`.
2. Store the returned `TimerToken` in the widget.
3. Match the token in `Event::Wake(WakeEvent::Timer { .. })`.
4. Cancel an obsolete token with `ctx.cancel_timer(token)`.

For motion that runs on its own, such as a simulation or a looping
indicator, call `request_animation_frame()`, update state from the next
`WakeEvent::AnimationFrame`, invalidate the changed presentation, and request
another frame only while the animation remains active. Transitions between
states do not need this; the runtime can drive them (see below). Do not run a
blocking loop inside `event` or `paint`.

### Animating Values

Most widget motion is a transition between states: hover, press, focus, a
toggle, a sliding indicator. Let the runtime drive it:

- `Progress` is a 0-to-1 value for widget state.
- `Motion<T>` animates any `Interpolate` value (numbers, points, vectors,
  rects, colors, transforms).

Start a transition from an event, measure, or arrange context with
`progress.animate(target, spec, ctx)` or `ctx.animate(&mut motion, target,
spec)`, and read the value with `get(ctx)` while painting. The value is a
function of time, so the widget handles no animation frames: the runtime
invalidates it every frame until the transition ends, then once more at the
end. By default that invalidation is `Paint`. A `Motion` read in
`layer_properties_at` can invalidate `Transform` or `Effect` instead
(`.invalidating(kind)`), which moves or fades a retained layer without
repainting it. Mark motion that moves content with `.movement()` so reduced
motion skips it.

The frame-driven types remain for other cases:

- `MotionScalar` and `MotionValue<T>` hold the same kind of transition but
  are advanced by the widget from animation frames.
- `AnimatedValue<T>` advances by frame deltas.
- `SpringF32` is a physics spring for interactive motion, such as a dragged
  handle that springs home with the velocity of the fling.

Each transition takes an `AnimationSpec`: either
`AnimationSpec::tween(duration, easing)` or `AnimationSpec::spring(spec)`. A
`SpringSpec` has a `duration` and a `bounce` (`SMOOTH`, `SNAPPY`, and `BOUNCY`
are presets). Springs are solved exactly, so they follow the same path at any
frame rate. `ThemeMotion` provides the theme's specs: `hover_spec()`,
`press_spec()`, `focus_spec()`, `toggle_spec()`, `entrance_spec()`,
`exit_spec()`, `layout_spec()` (content gliding to a new place), and
`tab_switch_spec()`.

Retargeting mid-flight keeps momentum: the new animation blends from the one
still running instead of restarting at zero speed, so a hover that ends halfway
through its fade turns around smoothly. Colors blend in premultiplied OKLab,
so a fade from a transparent color keeps its hue instead of passing through a
dark fringe. Transforms blend their translation, rotation, scale, and shear
separately, so a turning shape keeps its size.

```rust,no_run
use sui::prelude::*;
use sui::PointerEventKind;

/// A highlight that fades in while the pointer is over it.
struct Highlight {
    shown: Progress,
}

impl Widget for Highlight {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let hover = DefaultTheme::default().motion.hover_spec();
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.shown.animate(1.0, hover, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.shown.animate(0.0, hover, ctx);
            }
            _ => {}
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let shown = self.shown.get(ctx);
        ctx.fill_bounds(Color::rgba(0.2, 0.45, 0.95, 0.3 * shown));
    }
}
```

### Delays and Stagger

`ctx.animate_after(&mut motion, target, delay, spec)` and
`progress.animate_after(target, delay, spec, ctx)` start a transition `delay`
seconds from now; until then the value keeps doing what it was doing. Delays follow the motion policy's time scale, and are skipped when
the policy makes the transition instant.

`Stagger` computes delays that start a group one after another:
`Stagger::new(0.035).delay(index, count)` gives each item its delay, counted
from the first item, the last, the center, or any index (`from(origin)`), and
`max_delay(seconds)` keeps a long list's cascade short by shrinking the
interval. `ThemeMotion::stagger()` is the theme's cascade.

```rust,no_run
use sui::prelude::*;

/// Dots that pop in one after another when pressed.
struct Dots {
    shown: Vec<Motion<f32>>,
}

impl Widget for Dots {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Pointer(pointer) if pointer.kind == sui::PointerEventKind::Down) {
            let motion = DefaultTheme::default().motion;
            let stagger = motion.stagger().from(StaggerOrigin::Center);
            let count = self.shown.len();
            for (index, dot) in self.shown.iter_mut().enumerate() {
                dot.jump_to(0.0);
                ctx.animate_after(dot, 1.0, stagger.delay(index, count), motion.entrance_spec());
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for (index, dot) in self.shown.iter().enumerate() {
            let center = Point::new(12.0 + index as f32 * 24.0, 12.0);
            let radius = 8.0 * dot.get(ctx);
            ctx.fill(
                Path::rounded_rect(
                    Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0),
                    radius,
                ),
                Color::rgba(0.2, 0.45, 0.95, 1.0),
            );
        }
    }
}
```

### Timelines

A `Timeline` holds keyframed clips and named markers; an `AnimationPlayer` (or
the widget-side `TimelinePlayer`) plays it. `LoopMode::Once` stops at the end,
`Repeat` starts over, and `PingPong` plays forward then backward.
`PlaybackState::start_delay` waits before playing from the start, and
`loop_delay` holds at each end before looping. Markers (`Timeline::with_marker`)
name points in time; after each tick, `AnimationPlayer::passed_markers` and
`TimelineTick::passed_markers` list the markers the playhead passed, in order
and once per pass, so an app can start the next step of a sequence in time
with the animation.

### Frame Timing

Animation frames carry the time they represent. On a display that presents
with vsync, the platform starts one frame per refresh and stamps it with the
time the frame is expected on screen, so motion lines up with the display
instead of with event-loop timing. Hidden windows and headless hosts fall back
to the runtime's own timer (`FramePacing::Timer`). While painting, read that
time with `ctx.frame_time()`; `Motion::get(ctx)` uses it. Custom platform
hosts choose the pacing with `Runtime::set_frame_pacing` and start
display-paced frames with `Runtime::begin_animation_frame`.

### Reduced Motion and the Motion Policy

`motion_policy()` returns the app-wide `MotionPolicy`: a `MotionPreference`
(`Full`, `Reduced`, or `Off`) and a time scale. The platform reports the
operating system setting (Windows "Show animations in Windows" and the web's
`prefers-reduced-motion`; other platforms report full motion). An app can
override it with `set_app_motion_preference(Some(preference))`, follow the
system again with `None`, and slow every transition down for inspection with
`set_motion_time_scale(0.25)`. The policy is read when a transition starts, so
changes apply to the next animation.

- `Full` plays everything.
- `Reduced` keeps fades and color changes but drops movement: surfaces fade in
  place instead of sliding, and sliding indicators jump.
- `Off` finishes every transition immediately.

`Progress`, `Motion`, and `MotionScalar` apply the policy automatically; mark
motion that moves content with `.movement()` (or use `MotionScalar`'s
`set_movement_target` methods), and use `MotionPolicy::entrance_offset(progress)`
for the distance a surface slides in. With `MotionValue`, pass
`spec.with_policy(motion_policy())` or `spec.with_movement_policy(motion_policy())`. Delta-driven animation, such as a
timeline player or a simulation, can follow the time scale with
`motion_policy().scale_delta(delta)`.

## Tutorial: Deliver Typed Background Results with `UiHandle`

Widgets are not required to be `Send`, and their methods stay synchronous.
Put long-running work on another thread and return its result through SUI's
thread-safe typed command queue.

```rust,no_run
use sui::prelude::*;

const BACKGROUND_STATUS: CommandKey<String> =
    CommandKey::new("example.background.status");

fn main() -> Result<()> {
    let status = Signal::named("background status", "Loading…".to_string());
    let status_for_command = status.clone();

    App::new()
        .on_command(BACKGROUND_STATUS, move |_, message| {
            status_for_command.set(message.clone());
        })
        .main_window(
            "Background work",
            Label::new("Loading…").text_from(status),
        )
        .run_with_handle(move |ui| {
            std::thread::spawn(move || {
                // Replace this with blocking I/O or CPU work.
                ui.send_application(BACKGROUND_STATUS, "Loaded".to_string());
            });
        })
}
```

Sending enqueues the payload and wakes the platform event loop. The application
subscription runs on the UI thread, and the signal invalidates only widgets that
observed it. No invisible widget or root custom event is involved.

Widget callbacks use the same queue through `EventCtx::command_sender`:

```rust
#[derive(Clone, Copy)]
enum RefreshReason {
    UserAction,
}

static REFRESH_REQUESTED: CommandKey<RefreshReason> =
    CommandKey::new("example.refresh-requested");

Button::new("Refresh every window").on_press_with_ctx(|ctx| {
    ctx.command_sender().broadcast_application(
        REFRESH_REQUESTED,
        RefreshReason::UserAction,
    );
})
```

The command is delivered after the current input dispatch yields back to the
runtime. Clone the sender before moving it into worker-owned code; use
`UiHandle` when starting work from `App::run_with_handle`.

## Command Scope, Multicast, and Controllers

`CommandSender` and `UiHandle` expose four routing scopes:

| Target | Receiver |
| --- | --- |
| `Widget { window_id, widget_id }` | `Widget::command` on that retained identity |
| `FocusedWidget(window_id)` | `Widget::command` on the current focus target |
| `Window(window_id)` | `Window::on_command` and window controllers |
| `Application` | `App::on_command` and application controllers |

Each send returns the command's sequence number, which handlers see as
`Command::sequence` and dispatch diagnostics record.

A directed command stops after a handler calls `ctx.set_handled()`. A broadcast
continues through all matching handlers. `broadcast_application` first reaches
application handlers and then every live window, making it the application-wide
multicast facility. Window subscriptions and controllers are dropped with their
window; a widget command whose stable identity is no longer present is reported
as undelivered instead of being rerouted to an ancestor.

Use `App::controller` or `Window::controller` for services that need both typed
commands and a scheduler-only wake hook. `CommandController::wake` is invoked by
`UiHandle::wake`; it is intentionally separate from command delivery and never
synthesizes `Event::Custom` at a root widget. A controller can request measure,
arrange, paint, semantics, or animation work through `CommandCtx`, and can attach
a diagnostic reason with `request_window_with_reason`.

`window_command_dispatches_signal(window_id)` holds the window's latest
dispatches, up to `COMMAND_HISTORY_LENGTH`, as a signal an in-app tool can
observe. Each `CommandDispatchSample` names the command and its payload type,
its sequence, target, and delivery, the listeners that ran, whether one
handled it, and whether it was delivered at all. Controllers appear under
`CommandController::debug_name`, which is worth overriding with a readable
name; closure subscriptions appear under their type name. An application
command is recorded in every window's history.

The performance inspector shows command routing and invalidations from the
latest frame. The application inspector additionally retains bounded command,
invalidation, reactive, rebuild, and capture/target/bubble event-route history
when `Runtime::set_inspector_tracing` is enabled. Capture the unified state with
`Runtime::inspector_snapshot`; event payloads and typed text are never retained
in route history.

`UiHandle` is available only with a platform event-loop feature. Headless code
can use `Runtime::command_sender` plus `process_commands`, or drive the runtime
through `sinomo-ui-testing`.

Run the complete interactive sample with:

```bash
cargo run -p sinomo-ui --example commands
```

The `Commands` page in `sinomo-ui-demo` sends a command down each route, from
the UI thread or a worker, to named application and window controllers, and
shows what each did and the window's dispatch history as it happens. It also
has a focus-keeping edit toolbar and an export that reports its progress from
a worker thread.
