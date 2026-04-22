# SUI Animation System Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task when parallelization is helpful, but keep the runtime policy-light, keep widget animation state widget-owned, and do not expose raw `wgpu` objects directly to ordinary widgets.

**Goal:** Add a concrete animation foundation to SUI that supports efficient animated widgets, preserves SUI's widget-owned control model, enables smooth 120 fps paths where appropriate, and ships a reusable `sui-widgets` animation utility layer plus first-wave built-in widget animations.

**Architecture:** Add only low-level frame-time and frame-wake support in `sui-runtime` / `sui-platform`, add renderer-neutral dynamic layer properties in `sui-scene`, route efficient retained animation through explicit paint boundaries and retained compositor updates in `sui-render-wgpu`, and keep transition/spring/easing policy in a new `sui-widgets::animation` module. Built-in widgets should animate by either repainting or updating retained layer properties, but SUI itself should not enforce a framework-owned animation model.

**Tech Stack:** Rust 2024, `sui-core` events and invalidation, `sui-runtime` widget contexts and frame scheduling, `sui-scene` layer descriptors and scene frames, `sui-render-wgpu` retained compositor, `sui-platform` desktop/headless redraw loops, `sui-widgets` built-in widgets and theme model, `sui-testing` headless harness, and `sui-widget-book` visual/performance validation surfaces.

---

## Scope guardrails

- Do **not** turn `sui-runtime` into a framework-owned animation engine with built-in transition policy.
- Do **not** expose raw `wgpu::Device`, `wgpu::Queue`, pipeline handles, or shader module handles directly to ordinary widgets.
- Do **not** start with blur, backdrop-filter, arbitrary shader injection, or generic path morphing.
- Do **not** require all widgets to become explicit paint boundaries just to animate.
- Do **not** regress the existing flat-by-default architectural direction; explicit retained boundaries should remain opt-in.
- Do **not** make built-in animation utilities mandatory for custom widgets.
- For the first rollout, keep the retained fast path to **translation + opacity + simple effect parameters** only.
- Keep caret blink and similarly tiny local effects on the repaint path unless a later benchmark proves a retained path is materially better.

---

## Current grounded facts from the repo

These facts should shape the implementation:

1. `EventCtx` already exposes `current_time()` and timer scheduling through `schedule_timer_at()` / `schedule_timer_after()` in `crates/sui-runtime/src/widget.rs`.
2. `WakeEvent::Timer` and `WakeEvent::Async` already exist in `crates/sui-core/src/event.rs`; there is no animation-frame wake yet.
3. `FrameSchedule` in `crates/sui-runtime/src/lib.rs` already distinguishes `Transform`, `Effect`, `Visibility`, `Paint`, `Semantics`, `Resources`, and other work kinds.
4. `PaintCtx` paints only into `Scene`; widgets do not currently emit raw GPU commands.
5. `SceneLayerDescriptor` in `crates/sui-scene/src/lib.rs` already carries owner, bounds, content bounds, paint bounds, stack metadata, and composition mode, but not dynamic layer properties such as opacity.
6. `sui-render-wgpu` already has retained transform / clip / effect nodes and already treats explicit layer transforms as a cheaper path than repainting content.
7. Built-in widgets in `crates/sui-widgets/src/controls.rs` currently store booleans like `hovered`, `pressed`, and `focused` state but generally switch visuals immediately rather than animating.
8. `Tooltip`, `Popover`, and related composites in `crates/sui-widgets/src/composites.rs` already opt into `PaintBoundaryMode::Explicit`, which makes them good first candidates for retained animations.
9. `sui-testing` already supports deterministic `advance_time(...)` through the headless harness, which is important for animation validation.
10. The current docs already say that widgets own state and explicit invalidation, while the runtime owns scheduling and orchestration.

Implication:

- the first runtime slice should add **frame delivery**, not a full animation API
- the first scene/renderer slice should add **dynamic layer properties**, not raw GPU access
- the first widget-library slice should add **optional helper types**, not a mandatory animation subsystem

---

## Recommended first-wave feature boundary

Phase 1 is complete when all of the following are true:

- widgets can request an animation-frame wake separate from timers
- desktop and headless platforms continue redrawing while animation-frame subscriptions are active
- `sui-scene` can represent dynamic layer properties for explicit paint-boundary widgets
- `sui-render-wgpu` can apply retained translation and opacity updates without rebuilding content every frame when the underlying content is unchanged
- `sui-widgets` exposes reusable helpers for easing, interpolation, time-based transitions, and blink/pulse utilities
- at least these built-in widgets use the new system:
  - `Button`
  - `IconButton`
  - `Checkbox`
  - `Switch`
  - `Slider`
  - `TextInput` and `TextArea`
  - one explicit-layer composite path such as `Tooltip` or `Popover`
- widget-book includes animation demo/validation coverage
- headless tests can deterministically advance animations and assert intermediate states
- runtime / renderer diagnostics make it obvious whether an animation frame repainted content or only updated retained properties

---

## Architectural targets

### Runtime responsibility
Add only low-level animation support in `sui-runtime` and `sui-platform`:
- monotonic frame time
- frame delta
- next-frame wake delivery
- continued redraw scheduling while animation-frame work is pending
- invalidation plumbing for paint / transform / effect / visibility updates
- cleanup when animated widgets disappear or stop requesting frames

### Widget responsibility
Keep animation policy and state widget-owned:
- start and target values
- easing / spring choice
- hover / press / focus transition logic
- decisions about repaint-driven vs retained-property animation
- repeated frame requests while the animation is still active

### GPU / renderer boundary
Keep widgets renderer-neutral:
- repaint-driven animation stays ordinary `PaintCtx` / `Scene` work
- retained animation flows through explicit boundaries plus renderer-neutral layer metadata
- future advanced renderer effects should use abstract scene/resource/effect handles rather than raw `wgpu`

### Default rule
- flat widgets animate by repaint
- explicit paint-boundary widgets may animate through retained transform / opacity / effect-property updates

---

## Task 1: Add animation-frame wake primitives to `sui-core` and `sui-runtime` (test-first)

**Objective:** Give widgets a first-class way to request per-frame wakes without abusing one-shot timers.

**Files:**
- Modify: `crates/sui-core/src/event.rs`
- Modify: `crates/sui-runtime/src/widget.rs`
- Modify: `crates/sui-runtime/src/lib.rs`
- Test: `crates/sui-core/src/event.rs`
- Test: `crates/sui-runtime/src/widget.rs`
- Test: `crates/sui-runtime/src/lib.rs`

**Step 1: Write failing tests for the new wake contract**

Add tests covering at least:
- `WakeEvent` includes a new animation-frame variant carrying `time`, `delta`, and a stable per-window frame index or equivalent monotonic counter
- `EventCtx::request_animation_frame()` registers an animation-frame wake for the current widget
- repeated requests do not leak duplicate registrations for the same widget within the same frame
- widgets removed from the tree do not keep their window redrawing forever

**Suggested API shape:**
```rust
pub enum WakeEvent {
    Timer { token: TimerToken, time: f64, deadline: f64 },
    Async { token: AsyncWakeToken, time: f64 },
    AnimationFrame { time: f64, delta: f64, frame_index: u64 },
}
```

```rust
impl EventCtx {
    pub fn request_animation_frame(&mut self);
}
```

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-core --lib event::tests::wake_event_animation_frame_carries_time_and_delta -- --exact
cargo test -p sui-runtime --lib widget::tests::request_animation_frame_enqueues_widget_for_next_frame -- --exact
cargo test -p sui-runtime --lib tests::animated_widget_registration_is_cleaned_up_when_widget_disappears -- --exact
```
Expected: FAIL

**Step 3: Implement the low-level runtime plumbing**

Implementation notes:
- prefer a window-local registry of widgets requesting animation frames
- treat animation-frame delivery like a wake source, not like a synthetic pointer/window event
- keep the registration cheap and idempotent per widget per frame
- update `wake_target(...)`, `drain_ready_events(...)`, and related bookkeeping so `AnimationFrame` routes to the correct widget
- define clear semantics for `delta` when the window was just bootstrapped or resumed after inactivity

**Step 4: Add ergonomic invalidation helpers while touching `EventCtx`**

Add widget-facing helpers such as:
```rust
pub fn request_transform(&mut self);
pub fn request_effect(&mut self);
pub fn request_visibility(&mut self);
```

These should simply map to existing invalidation kinds and keep widget animation code readable.

**Step 5: Re-run the exact tests and a broader runtime pass**

Run:
```bash
cargo test -p sui-core --lib event::tests::wake_event_animation_frame_carries_time_and_delta -- --exact
cargo test -p sui-runtime --lib widget::tests::request_animation_frame_enqueues_widget_for_next_frame -- --exact
cargo test -p sui-runtime --lib tests::animated_widget_registration_is_cleaned_up_when_widget_disappears -- --exact
cargo check -p sui-runtime
```

**Step 6: Commit**
```bash
git add crates/sui-core/src/event.rs crates/sui-runtime/src/widget.rs crates/sui-runtime/src/lib.rs
git commit -m "feat: add animation frame wake support"
```

---

## Task 2: Teach desktop and headless platforms to keep redraws flowing for active animation frames (test-first)

**Objective:** Make the platform redraw loop continue while animation-frame work is pending, without special-casing specific widgets.

**Files:**
- Modify: `crates/sui-platform/src/desktop.rs`
- Modify: `crates/sui-platform/src/headless.rs`
- Modify: `crates/sui-testing/src/harness.rs`
- Modify: `crates/sui-testing/src/window.rs`
- Test: `crates/sui-platform/src/headless.rs`
- Test: `crates/sui-testing/src/harness.rs`

**Step 1: Write failing platform and harness tests**

Add tests covering:
- a widget that repeatedly requests animation frames causes headless `pump()` to continue producing redraws until the animation stops
- `advance_time(...)` deterministically advances animation state in headless mode
- desktop redraw scheduling consults animation-frame wakeups through the normal `needs_render()` / next-wakeup path rather than a separate hardcoded loop

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-platform --lib headless::tests::animation_frame_request_keeps_headless_pumping_until_completion -- --exact
cargo test -p sui-testing --lib harness::tests::advance_time_drives_animation_frame_delivery -- --exact
```
Expected: FAIL

**Step 3: Implement the redraw-flow updates**

Implementation notes:
- prefer using `Runtime::next_wakeup_time(...)` and normal redraw plumbing rather than a second animation-only scheduler
- headless should keep deterministic behavior; do not add wall-clock sleeps or background polling
- desktop should request redraw only when runtime state says work is pending
- define how animation-frame deadlines interact with timer deadlines; the earliest ready wake should continue to win

**Step 4: Re-run focused tests and a broader check**

Run:
```bash
cargo test -p sui-platform --lib headless::tests::animation_frame_request_keeps_headless_pumping_until_completion -- --exact
cargo test -p sui-testing --lib harness::tests::advance_time_drives_animation_frame_delivery -- --exact
cargo check -p sui-platform
cargo check -p sui-testing
```

**Step 5: Commit**
```bash
git add crates/sui-platform/src/desktop.rs crates/sui-platform/src/headless.rs crates/sui-testing/src/harness.rs crates/sui-testing/src/window.rs
git commit -m "feat: route animation frame wakes through platform redraw loops"
```

---

## Task 3: Add renderer-neutral dynamic layer properties to `sui-scene` (test-first)

**Objective:** Create the scene-level contract for efficient retained animation without exposing raw `wgpu` to widgets.

**Files:**
- Modify: `crates/sui-scene/src/lib.rs`
- Modify: `crates/sui-runtime/src/widget.rs`
- Modify: `crates/sui-runtime/src/lib.rs`
- Test: `crates/sui-scene/src/lib.rs`
- Test: `crates/sui-runtime/src/lib.rs`

**Step 1: Write failing tests for dynamic layer properties**

Add tests covering:
- `SceneLayerDescriptor` can carry dynamic layer properties such as opacity and translation
- layer descriptors default to identity / fully opaque behavior
- descriptor changes map to `SceneLayerUpdateKind::Transform` or `SceneLayerUpdateKind::Effect` instead of forcing content rebuilds
- only explicit paint-boundary widgets can use the retained property fast path

**Suggested MVP types:**
```rust
pub struct LayerProperties {
    pub opacity: f32,
    pub translation: Vector,
    pub effect: LayerEffect,
}

pub enum LayerEffect {
    None,
    Glow { intensity: f32, color: Color },
}
```

If `Glow` feels too early, land only `None` first and defer parameterized effects to a follow-up task in the same file.

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-scene --lib tests::scene_layer_descriptor_defaults_to_identity_layer_properties -- --exact
cargo test -p sui-runtime --lib tests::explicit_boundary_layer_property_change_marks_transform_or_effect_without_content_rebuild -- --exact
```
Expected: FAIL

**Step 3: Implement the scene contract**

Implementation notes:
- store dynamic properties in `SceneLayerDescriptor`, not as ordinary draw commands
- do not make layer properties mandatory for flat widgets
- keep the first property set intentionally small: translation, opacity, and optionally a tiny effect enum
- ensure `SceneLayerDescriptor::translate(...)` and related helpers stay coherent with the new fields

**Step 4: Add a widget/runtime extraction hook for explicit layers**

Add a widget-facing hook that keeps policy widget-owned while exposing retained state to the runtime. One workable shape is:
```rust
fn layer_state(&self) -> LayerState {
    LayerState::default()
}
```

Or a context-taking variant if current time / DPI is needed. Keep the hook optional and no-op by default.

**Step 5: Re-run focused tests and `cargo check`**

Run:
```bash
cargo test -p sui-scene --lib tests::scene_layer_descriptor_defaults_to_identity_layer_properties -- --exact
cargo test -p sui-runtime --lib tests::explicit_boundary_layer_property_change_marks_transform_or_effect_without_content_rebuild -- --exact
cargo check -p sui-scene
cargo check -p sui-runtime
```

**Step 6: Commit**
```bash
git add crates/sui-scene/src/lib.rs crates/sui-runtime/src/widget.rs crates/sui-runtime/src/lib.rs
git commit -m "feat: add dynamic scene layer properties"
```

---

## Task 4: Apply retained layer-property updates in `sui-render-wgpu` (test-first)

**Objective:** Make explicit paint-boundary widgets animate via retained compositor updates when only properties changed.

**Files:**
- Modify: `crates/sui-render-wgpu/src/retained.rs`
- Modify: `crates/sui-render-wgpu/src/scene.rs`
- Possibly modify: `crates/sui-render-wgpu/src/gpu.rs`
- Test: `crates/sui-render-wgpu/src/retained.rs`
- Test: `crates/sui-render-wgpu/src/scene.rs`

**Step 1: Write failing retained-path tests**

Add tests covering:
- translation-only updates on an explicit layer do not rebuild packet content
- opacity-only updates stay on the retained fast path
- layer-property changes update effect-node / transform-node state correctly
- content rebuild counts do not increase on pure property animation frames

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-render-wgpu --lib retained::tests::translation_only_layer_updates_reuse_retained_content -- --exact
cargo test -p sui-render-wgpu --lib retained::tests::opacity_only_layer_updates_reuse_retained_content -- --exact
```
Expected: FAIL

**Step 3: Implement the retained-property path**

Implementation notes:
- extend retained effect-node state only as far as the MVP requires
- keep property updates separate from content signature / structure signature changes
- preserve the existing composition-only translation optimization where possible
- make diagnostics attribute the frame correctly as transform/effect work rather than content rebuild work

**Step 4: Re-run focused tests and renderer checks**

Run:
```bash
cargo test -p sui-render-wgpu --lib retained::tests::translation_only_layer_updates_reuse_retained_content -- --exact
cargo test -p sui-render-wgpu --lib retained::tests::opacity_only_layer_updates_reuse_retained_content -- --exact
cargo check -p sui-render-wgpu
```

**Step 5: Commit**
```bash
git add crates/sui-render-wgpu/src/retained.rs crates/sui-render-wgpu/src/scene.rs crates/sui-render-wgpu/src/gpu.rs
git commit -m "feat: add retained layer property animation path"
```

---

## Task 5: Add optional widget-side animation utilities in `sui-widgets` (test-first)

**Objective:** Ship a small reusable animation helper library for built-in widgets and custom widgets that want it, without moving policy into runtime.

**Files:**
- Create: `crates/sui-widgets/src/animation.rs`
- Modify: `crates/sui-widgets/src/lib.rs`
- Test: `crates/sui-widgets/src/animation.rs`

**Step 1: Write failing unit tests for the utility surface**

Add tests covering:
- interpolation of `f32`, `Color`, and `Vector`
- easing curves such as `linear`, `ease_in_out`, and cubic-bezier helpers
- `Transition<T>` sampling from start/end values over time
- `Blink` / `Pulse` helpers producing stable deterministic outputs from `time` / `delta`
- spring helpers converging toward target values without requiring runtime-global state

**Suggested initial API surface:**
```rust
pub trait Interpolate: Sized {
    fn interpolate(from: Self, to: Self, t: f32) -> Self;
}

pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
}

pub struct Transition<T> { /* start, end, start_time, duration, easing */ }
pub struct SpringF32 { /* value, velocity, stiffness, damping */ }
pub struct Blink { /* period, duty_cycle, phase */ }
```

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-widgets --lib animation::tests::transition_samples_ease_in_out_curve -- --exact
cargo test -p sui-widgets --lib animation::tests::blink_is_deterministic_for_same_time_inputs -- --exact
```
Expected: FAIL

**Step 3: Implement the minimal helper set**

Implementation notes:
- keep the API small and copy-friendly
- avoid introducing a central animation registry in `sui-widgets`
- keep everything driven by explicit `time` / `delta` input from widgets
- do not add path morphing in this first file

**Step 4: Export the module through `sui-widgets`**

Update `crates/sui-widgets/src/lib.rs` so built-in widgets and downstream users can consume the helpers directly.

**Step 5: Re-run focused tests and `cargo check -p sui-widgets`**

Run:
```bash
cargo test -p sui-widgets --lib animation::tests::transition_samples_ease_in_out_curve -- --exact
cargo test -p sui-widgets --lib animation::tests::blink_is_deterministic_for_same_time_inputs -- --exact
cargo check -p sui-widgets
```

**Step 6: Commit**
```bash
git add crates/sui-widgets/src/animation.rs crates/sui-widgets/src/lib.rs
git commit -m "feat: add widget animation utility module"
```

---

## Task 6: Animate the first wave of simple controls by repaint or retained-property updates (test-first)

**Objective:** Prove the new animation system in the core built-in controls that currently flip hover/press/focus state immediately.

**Files:**
- Modify: `crates/sui-widgets/src/controls.rs`
- Test: `crates/sui-widgets/src/controls.rs`
- Possibly modify: `crates/sui-widget-book/src/visual_artifacts.rs`

**Step 1: Write failing widget tests for animated state transitions**

Add tests covering at least:
- `Button` hover transitions interpolate rather than snap
- `IconButton` pressed state decays back smoothly after release
- `Checkbox` check indicator and focus ring animate deterministically
- `Switch` thumb slide and track color transition animate across multiple frames
- `Slider` thumb hover/press visuals animate without disturbing semantics

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-widgets --lib controls::tests::button_hover_animation_advances_over_multiple_frames -- --exact
cargo test -p sui-widgets --lib controls::tests::switch_thumb_animation_tracks_progress_and_completion -- --exact
cargo test -p sui-widgets --lib controls::tests::slider_thumb_hover_animation_requests_followup_frames_until_complete -- --exact
```
Expected: FAIL

**Step 3: Implement the first-wave widget rollout**

Recommended first-wave breakdown:
- repaint-driven transitions for `Button`, `IconButton`, `Checkbox`, and `Slider` where only tiny local visuals change
- explicit retained-property path for `Switch` thumb translation if it measurably reduces repaint churn; if not, keep `Switch` repaint-driven in the first commit and defer retained usage to the overlay/composite task

Implementation notes:
- widgets should own animation state fields directly
- on pointer/focus events, update target values and call `request_animation_frame()`
- on `WakeEvent::AnimationFrame`, sample transitions and request only the minimal invalidation kind needed
- keep semantics tied to logical state, not intermediate visual interpolation values

**Step 4: Re-run focused tests and `cargo check -p sui-widgets`**

Run:
```bash
cargo test -p sui-widgets --lib controls::tests::button_hover_animation_advances_over_multiple_frames -- --exact
cargo test -p sui-widgets --lib controls::tests::switch_thumb_animation_tracks_progress_and_completion -- --exact
cargo test -p sui-widgets --lib controls::tests::slider_thumb_hover_animation_requests_followup_frames_until_complete -- --exact
cargo check -p sui-widgets
```

**Step 5: Commit**
```bash
git add crates/sui-widgets/src/controls.rs crates/sui-widget-book/src/visual_artifacts.rs
git commit -m "feat: animate built-in control state transitions"
```

---

## Task 7: Add focused text-input animation behavior without overengineering it (test-first)

**Objective:** Improve text-editing affordances with simple, correct animation behavior while keeping tiny local effects repaint-driven.

**Files:**
- Modify: `crates/sui-widgets/src/controls.rs`
- Test: `crates/sui-widgets/src/controls.rs`
- Possibly modify: `crates/sui-widget-book/src/visual_artifacts.rs`

**Step 1: Write failing tests for text input animation behavior**

Add tests covering:
- `TextInput` and `TextArea` caret blink uses the new time/frame contract rather than permanent always-on drawing
- focus ring transitions animate in and out
- caret blink can be deterministically validated by advancing headless time
- IME composition behavior keeps the caret/composition rect correct while blinking

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-widgets --lib controls::tests::text_input_caret_blink_toggles_visibility_as_time_advances -- --exact
cargo test -p sui-widgets --lib controls::tests::text_area_focus_ring_animation_progresses_without_losing_ime_rect -- --exact
```
Expected: FAIL

**Step 3: Implement the text-input rollout**

Implementation notes:
- keep caret blink repaint-driven in this first slice
- do not move text measurement or text layout policy into the animation helpers
- use a tiny `Blink` helper from `sui-widgets::animation` rather than bespoke timer math in each text control
- focus-ring fade may still use repaint unless profiling shows a retained path is worthwhile

**Step 4: Re-run focused tests and `cargo check -p sui-widgets`**

Run:
```bash
cargo test -p sui-widgets --lib controls::tests::text_input_caret_blink_toggles_visibility_as_time_advances -- --exact
cargo test -p sui-widgets --lib controls::tests::text_area_focus_ring_animation_progresses_without_losing_ime_rect -- --exact
cargo check -p sui-widgets
```

**Step 5: Commit**
```bash
git add crates/sui-widgets/src/controls.rs crates/sui-widget-book/src/visual_artifacts.rs
git commit -m "feat: animate focus and caret behavior in text inputs"
```

---

## Task 8: Animate explicit-layer composites through retained properties (test-first)

**Objective:** Use explicit paint boundaries where they already exist to prove efficient retained animation on overlays and popup-like surfaces.

**Files:**
- Modify: `crates/sui-widgets/src/composites.rs`
- Test: `crates/sui-widgets/src/composites.rs`
- Modify: `crates/sui-widget-book/src/lib.rs`
- Modify: `crates/sui-widget-book/src/visual_artifacts.rs`

**Step 1: Write failing composite tests**

Add tests covering:
- `Tooltip` reveal/hide animation progresses across frames without content rebuild churn when only translation/opacity changes
- `Popover` open/close animation uses explicit-layer properties instead of repainting the content every frame
- animation completion stops requesting further frames
- interaction semantics still follow logical open/hover state, not intermediate visual opacity

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-widgets --lib composites::tests::tooltip_reveal_animation_updates_layer_properties_until_complete -- --exact
cargo test -p sui-widgets --lib composites::tests::popover_open_animation_stops_requesting_frames_after_completion -- --exact
```
Expected: FAIL

**Step 3: Implement the overlay/composite rollout**

Implementation notes:
- these widgets already opt into `PaintBoundaryMode::Explicit`, so prefer retained translation and opacity here
- keep arrival pulse / glow subtle and optional; do not introduce permanent HDR glow as a default
- if the first slice still needs repaint for shadow or outline details, keep the retained property set limited to the core motion and opacity path

**Step 4: Add widget-book demo coverage**

Add or update a widget-book surface showing:
- button hover/press animation
- switch motion
- text-input focus/caret blink
- tooltip/popover entry motion

Prefer an explicit animation demo panel rather than relying only on static artifact crops.

**Step 5: Re-run focused tests and widget-book checks**

Run:
```bash
cargo test -p sui-widgets --lib composites::tests::tooltip_reveal_animation_updates_layer_properties_until_complete -- --exact
cargo test -p sui-widgets --lib composites::tests::popover_open_animation_stops_requesting_frames_after_completion -- --exact
cargo check -p sui-widget-book
```

**Step 6: Commit**
```bash
git add crates/sui-widgets/src/composites.rs crates/sui-widget-book/src/lib.rs crates/sui-widget-book/src/visual_artifacts.rs
git commit -m "feat: animate popup surfaces with retained layer properties"
```

---

## Task 9: Add diagnostics and benchmark coverage for animation behavior (test-first)

**Objective:** Make it easy to understand whether animation frames are repaint-heavy or retained-fast-path frames.

**Files:**
- Modify: `crates/sui-runtime/src/diagnostics.rs`
- Modify: `crates/sui-platform/src/lib.rs`
- Possibly modify: `crates/sui-render-wgpu/src/retained.rs`
- Modify: `crates/sui-widget-book/src/lib.rs`
- Test: `crates/sui-runtime/src/diagnostics.rs`
- Test: `crates/sui-widget-book/src/lib.rs`

**Step 1: Write failing diagnostics tests**

Add tests covering:
- active animated widget counts are surfaced in runtime/window diagnostics
- frames driven only by transform/effect updates are distinguishable from repaint-driven frames
- widget-book benchmark/demo surfaces can report useful animation-related counters

**Recommended counters:**
- active animated widget count
- animation-frame wake count
- animation repaint frame count
- animation transform/effect-only frame count
- retained packet rebuild count during animation surfaces

**Step 2: Run focused tests to confirm failure**

Run:
```bash
cargo test -p sui-runtime --lib diagnostics::tests::animation_frame_counters_distinguish_repaint_from_transform_only_frames -- --exact
cargo test -p sui-widget-book --lib widget_book_animation_demo_reports_animation_counters -- --exact
```
Expected: FAIL

**Step 3: Implement the diagnostics slice**

Implementation notes:
- keep these counters cheap enough for live diagnostics use
- avoid adding animation-only instrumentation paths that bypass existing frame publication plumbing
- wire the counters through the same window-performance publication route used by existing renderer statistics

**Step 4: Add benchmark / demo coverage**

Add a widget-book animation demo or ignored benchmark that can be used to compare:
- repaint-driven hover/focus animations
- retained overlay motion
- mixed animation scenarios

**Step 5: Re-run focused tests and `cargo check`**

Run:
```bash
cargo test -p sui-runtime --lib diagnostics::tests::animation_frame_counters_distinguish_repaint_from_transform_only_frames -- --exact
cargo test -p sui-widget-book --lib widget_book_animation_demo_reports_animation_counters -- --exact
cargo check -p sui-runtime
cargo check -p sui-widget-book
```

**Step 6: Commit**
```bash
git add crates/sui-runtime/src/diagnostics.rs crates/sui-platform/src/lib.rs crates/sui-widget-book/src/lib.rs crates/sui-render-wgpu/src/retained.rs
git commit -m "feat: add animation diagnostics and widget-book coverage"
```

---

## Task 10: Update architecture docs to capture the animation boundary model

**Objective:** Document the new runtime/widget/renderer split so future work does not drift into a framework-owned animation engine or raw-widget `wgpu` access.

**Files:**
- Modify: `docs/design.md`
- Modify: `docs/architecture.md`
- Modify: `docs/renderer-architecture.md`
- Modify: `docs/crate-architecture.md`
- Optionally modify: `docs/README.md`

**Step 1: Update the docs as a set**

Capture all three clearly:
- current implementation reality after the animation slice lands
- what remains intentionally widget-owned
- what the runtime and renderer now provide

**Step 2: Add the animation boundary explicitly**

The docs should say, in substance:
- runtime provides time, frame scheduling, invalidation routing, and retained-property plumbing
- widgets own animation policy and state
- `sui-widgets` provides optional shared helpers
- `sui-render-wgpu` remains the only crate that owns `wgpu`
- efficient retained animation requires explicit paint boundaries rather than reintroducing broad default layerization

**Step 3: Verify the doc pass**

Run:
```bash
git diff --stat -- docs/
git status --short -- docs/
```

**Step 4: Commit**
```bash
git add docs/design.md docs/architecture.md docs/renderer-architecture.md docs/crate-architecture.md docs/README.md
git commit -m "docs: describe animation system boundaries"
```

---

## Suggested implementation order if you want the safest first slice

If this needs to be split across multiple sessions, the safest order is:

1. Task 1 — runtime animation-frame wakes
2. Task 2 — platform/headless redraw continuation
3. Task 5 — widget-side animation helpers
4. Task 6 — first-wave simple controls
5. Task 7 — text inputs and caret blink
6. Task 3 — dynamic scene layer properties
7. Task 4 — retained property application in renderer
8. Task 8 — explicit-layer overlay/composite rollout
9. Task 9 — diagnostics / benchmark coverage
10. Task 10 — docs refresh

This order gives a useful repaint-driven animation foundation before the retained-property path is fully complete.

---

## Validation checklist for the whole rollout

Before calling the feature complete, verify all of the following:

- widgets can animate purely by repaint without special retained-layer requirements
- explicit paint-boundary widgets can animate by retained translation/opacity updates without rebuilding content every frame
- headless tests can deterministically advance animations with `advance_time(...)`
- animation completion stops redraw churn promptly
- focus, hover, pressed, and text-input semantics remain correct during animation
- widget-book exposes at least one dedicated animation demo/validation surface
- runtime/renderer diagnostics show whether frames were repaint-driven or retained-property-driven
- docs describe the runtime/widget/renderer split accurately

---

## Open questions to resolve during implementation

These should be decided while working through the tasks, not deferred indefinitely:

1. Should `AnimationFrame` be one-shot only, or should runtime also expose an explicit subscribe/unsubscribe API for continuous animation sources?
2. Should `LayerState` be polled during scene generation or captured earlier during event handling?
3. Is `opacity` enough for the first retained-property slice, or is a tiny effect enum worth landing immediately for popup arrival glow/pulse work?
4. Do any first-wave controls actually benefit enough from retained-property motion to justify explicit boundaries, or should the retained path remain overlay/composite-only in phase 1?
5. Should animation-related diagnostics live only in runtime/platform snapshots, or also appear in widget-book overlay summaries by default?

Record the chosen answers in the touched docs so the architecture remains explicit.

---

## Final recommendation

Land the repaint-driven animation foundation first, then add retained layer-property animation for the widgets that already have meaningful explicit boundaries. That gets SUI to a useful and flexible animation model quickly, preserves the widget-owned design philosophy, and avoids prematurely turning the runtime or renderer contract into a heavyweight effect framework.
