# SUI Testing Architecture

This document describes the proposed architecture for `sui-testing`, a Playwright-style automated UI testing surface for SUI.

It should be read alongside [design.md](./design.md), [architecture.md](./architecture.md), and [crate-architecture.md](./crate-architecture.md).

The intent is not to copy browser automation literally. SUI is not a DOM runtime, and desktop or headless native applications have different constraints than a web page. The goal is to provide the same testing ergonomics that make Playwright effective:

- tests written against observable behavior rather than internal implementation details
- locator-based queries instead of brittle one-shot element handles
- automatic waiting and retrying around asynchronous UI behavior
- high-level user actions such as click, press, fill, and drag
- strong debugging artifacts when assertions fail

## Goals

`sui-testing` should provide:

- a deterministic in-process test harness for SUI applications and widgets
- a semantics-first locator model that feels similar to Playwright's role/name/text/test-id queries
- high-level actions that drive the runtime through normalized SUI events
- auto-waiting expectations that poll against the latest UI snapshot until timeout
- optional screenshot and image-diff testing for visual regressions
- an architecture that can later support Python and JavaScript bindings without redesigning the core test model

## Non-Goals

`sui-testing` should not try to:

- emulate the DOM, CSS selectors, or browser networking behavior
- make pixel comparison the default assertion strategy
- expose internal widget storage as the primary testing API
- require a real desktop event loop for most automated tests
- couple tests to `wgpu` internals or platform-specific accessibility APIs

## Existing Foundation

The current workspace already contains most of the low-level building blocks needed for a first version of `sui-testing`.

### Runtime and platform pieces that already exist

- `sui-platform::HeadlessPlatform` provides deterministic event pumping, queued event dispatch, and manual time advancement.
- `sui-runtime::Runtime` already exposes semantics snapshots, widget graph snapshots, focus state, pointer capture state, and explicit render scheduling.
- `sui-platform::AccessibilitySnapshot` already packages window-level semantic data with the focused widget and root node.
- `sui-render-wgpu::WgpuRenderer` already supports offscreen rendering for headless windows.

### Why this matters

This means `sui-testing` does not need its own runtime, renderer, or fake widget system. It should be a thin testing layer built on top of:

- `Runtime`
- `HeadlessPlatform`
- semantics snapshots
- normalized `Event` injection
- optional renderer readback utilities

That keeps the testing surface aligned with how real applications execute.

## Design Principles

### 1. Semantics-first, not tree-internals-first

The default testing surface should query the semantics tree, because that is the most stable representation of what the user can observe and interact with. The semantics layer is already a first-class SUI output for accessibility, automation, and future tooling.

Widget graph inspection remains valuable for debugging, but it should be a secondary surface.

### 2. Locator objects, not stale node handles

Like Playwright, SUI tests should prefer locators that re-resolve on each action or expectation rather than storing one-shot node handles that become stale after a render.

This is especially important for a retained widget runtime where a widget may move, change bounds, gain focus, or update text between frames.

### 3. Deterministic execution by default

The harness should run against the headless platform first. Time advancement, event delivery, render pumping, and asynchronous wakeups should be explicit and reproducible.

Wall-clock sleeps should not be the main synchronization mechanism.

### 4. High-level actions map to real SUI events

Actions like `click()` and `press()` should ultimately flow through the same normalized event model used by real applications. This keeps tests honest and exercises routing, focus, invalidation, and semantics updates.

### 5. Auto-waiting is part of the API contract

Assertions and actions should wait for the right state instead of forcing users to manually pump frames around every interaction.

### 6. Bindings-ready architecture

The core test model should be transport-agnostic and data-oriented so it can later be wrapped by Rust, Python, or JavaScript clients.

## Proposed Crate Role

`sui-testing` should be a dedicated workspace crate with this responsibility:

> deterministic UI automation and inspection for SUI applications, centered on locators, actions, expectations, and diagnostics.

It should depend on:

- `sui-core`
- `sui-runtime`
- `sui-platform`
- `sui-scene`
- `sui-render-wgpu` optionally for screenshot capture

It should not own:

- platform windowing logic
- renderer execution logic
- widget tree implementation details

## Proposed Crate Structure

```text
crates/
  sui-testing/
    src/
      lib.rs
      harness.rs
      app.rs
      window.rs
      selector.rs
      locator.rs
      action.rs
      expect.rs
      snapshot.rs
      diagnostics.rs
      clock.rs
      protocol.rs      # optional later phase
      screenshot.rs
```

### Module responsibilities

`harness.rs`

- owns `Runtime` plus `HeadlessPlatform`
- pumps events, frames, timers, and async wakeups
- exposes idle detection and timeout handling

`app.rs`

- top-level test entry point
- boots applications from builders or factory closures
- manages one or more windows

`window.rs`

- window-scoped automation surface
- exposes locators and snapshot access
- manages viewport configuration and per-window actions

`selector.rs`

- selector model and matching rules
- role, name, text, test-id, focused, and custom predicate queries

`locator.rs`

- lazy query objects that resolve against the latest snapshot
- strict matching, scoping, chaining, and filtering

`action.rs`

- click, hover, press, fill, focus, blur, drag, and custom event helpers
- translation of high-level actions into SUI `Event` sequences

`expect.rs`

- retrying expectations with timeout policy
- visible, hidden, focused, text, value, count, and screenshot assertions

`snapshot.rs`

- immutable test-facing snapshots of semantics, widget graph, and scene metadata
- debug dumps for failed assertions

`diagnostics.rs`

- structured failure reporting
- artifact capture: semantics tree, widget graph, scene summary, screenshots, overlays, and event trace

`screenshot.rs`

- screenshot capture helpers and PNG encode/decode utilities
- screenshot diffing and overlay generation for semantics and widget bounds
- structured artifact bundle writing for failed or exploratory test runs

`clock.rs`

- deterministic time control
- virtual-time stepping and timeout bookkeeping

`protocol.rs`

- optional transport-neutral command model for external drivers
- future bridge for JavaScript and Python test clients

## Core Object Model

### `TestApp`

`TestApp` is the process-level automation entry point.

Responsibilities:

- build the application runtime
- own the headless platform
- expose window enumeration and selection
- provide deterministic pumping utilities
- collect global diagnostics on failure

Illustrative API:

```rust
use sui::prelude::*;
use sui_testing::prelude::*;

#[test]
fn save_flow() -> Result<()> {
    let mut app = TestApp::new(|| {
        Application::new()
            .window(WindowBuilder::new().title("Editor").root(build_root()))
    })?;

    let window = app.main_window()?;
    window.get_by_role(SemanticsRole::TextInput).with_name("Name").fill("Ada")?;
    window.get_by_role(SemanticsRole::Button).with_name("Save").click()?;
    window.get_by_text("Saved").expect().to_be_visible()?;

    Ok(())
}
```

The exact names may change, but the shape should stay locator-driven and high-level.

### `TestWindow`

`TestWindow` represents one window inside the harness.

Responsibilities:

- query by semantics within a specific window
- dispatch pointer, keyboard, and IME interactions in window coordinates
- retrieve current snapshots and diagnostics
- control viewport, time, and render stabilization for that window

### `Locator`

A `Locator` is a reusable query plan, not a resolved node.

Each action on a locator should:

1. pump the harness to a stable point
2. fetch the latest accessibility snapshot
3. resolve the selector in the current tree
4. apply any scoped ancestor locators and then enforce strictness rules
5. perform the action or evaluate the expectation

This avoids stale-element behavior and makes retries natural.

### `Expectation`

Expectations should wrap a locator or snapshot predicate and retry until:

- the condition becomes true
- the harness reaches timeout
- the condition becomes impossible in a way that should fail immediately

## Selector Model

The selector model should look familiar to Playwright users but be rooted in SUI semantics rather than DOM attributes.

### Required first-wave selectors

- `get_by_role(role)`
- `get_by_role(role).with_name(name)`
- `get_by_text(text)`
- `get_by_description(text)`
- descendant scoping through locator chaining such as `row.get_by_role(role)`
- `focused()`
- `root()`
- `locator(filter)` for advanced matching

### Matching sources

Selectors should resolve primarily from `SemanticsNode` fields:

- `role`
- `name`
- `description`
- `value`
- `state`
- `bounds`
- `actions`

### Future selector extensions

These are valuable, but not required for the first slice:

- `get_by_label()` once semantic label relationships exist
- `nth()` and `first()` helpers
- regex or predicate-based matching
- spatial matching such as `below()`, `near()`, or `inside()`

## Action Model

The action layer should provide user-intent operations rather than just raw event dispatch.

### First-wave actions

- `click()`
- `double_click()`
- `hover()`
- `press(key)`
- `fill(text)`
- `focus()`
- `blur()`
- `drag_to(target)`
- `dispatch_event(event)` for low-level escape hatches

### Action execution rules

`click()` should:

1. resolve a unique target node
2. ensure the node is actionable
3. compute a target point from semantic bounds, usually the center
4. dispatch pointer move, down, and up events through the runtime
5. pump until the resulting work reaches a stable state

`press(key)` should:

1. ensure a focused target exists or resolve focus through the locator
2. dispatch normalized keyboard events
3. pump until idle or timeout

`fill(text)` should prefer a high-level text-input path rather than synthesizing hundreds of low-level key events.

Recommended behavior:

1. focus the target
2. clear current text through a defined text-input strategy
3. dispatch text input through IME commit or a dedicated text-input testing path
4. pump until the semantics value reflects the new content

This keeps text-entry tests deterministic and closer to how modern input systems actually behave.

### Semantics actions as a second execution path

SUI already models semantic actions like `Activate`, `Increment`, and `SetValue`. `sui-testing` should eventually support dispatching actions through semantics as well as pointer or keyboard events.

That implies a future runtime hook similar to:

```rust
Runtime::invoke_semantics_action(window_id, widget_id, action, payload)
```

This is especially useful for:

- accessibility-driven testing
- widgets whose behavior is not naturally expressible as a mouse click
- bindings and protocol-driven automation

The first release can still rely primarily on pointer and keyboard synthesis if needed.

## Auto-Waiting and Determinism

Auto-waiting is one of the most important pieces of the design.

### What the harness should wait for

After actions and during expectations, the harness should repeatedly:

1. pump queued runtime events
2. render pending redraws
3. drain ready timers and async wakeups
4. observe fresh semantics snapshots
5. stop when the condition is satisfied or timeout is reached

### Idle definition

For testing, a window is idle when all of the following are true:

- no pending headless platform events remain
- `Runtime::needs_render(window_id)` is false
- no immediate timer or async wakeup is due
- the current assertion predicate no longer requires waiting for a new snapshot

The exact implementation may need a small amount of new API on `HeadlessPlatform`, but the underlying data already exists.

### Virtual time

The harness should own a virtual test clock rather than relying on wall clock.

This enables:

- deterministic timer tests
- reproducible animation tests
- fast-forwarding delayed work without sleeping the test process

Recommended helpers:

- `advance_time(delta)`
- `run_until_idle()`
- `run_for(delta)`
- `wait_for(locator, timeout)`

## Snapshots and Diagnostics

When tests fail, the framework should surface enough information to debug the issue without rerunning under a debugger.

### Minimum failure artifacts

- current accessibility snapshot
- current widget graph snapshot
- last scene frame summary
- event trace since the last user action
- current window title, size, focus, and pointer capture state

### Optional visual artifacts

- rendered screenshot
- screenshot diff against baseline
- semantic-bounds overlay screenshot

### Why both semantics and graph snapshots matter

Semantics tell the user-facing story. The widget graph helps explain structural mistakes such as wrong bounds, unexpected focus ownership, or a parent swallowing input.

Both should be available in diagnostics, but semantics should remain the main assertion surface.

## Visual Regression Testing

Visual testing should exist, but it should be layered on top of the semantics-first core rather than replacing it.

### Proposed screenshot architecture

`sui-render-wgpu` already renders headless frames into offscreen textures. To make screenshot assertions possible, it should grow a readback API such as:

```rust
WgpuRenderer::capture_rgba(window_id) -> Result<RgbaImage>
```

or a testing-oriented equivalent owned by `sui-testing`.

That API should:

- copy the offscreen texture into a CPU-visible buffer
- normalize row padding
- return RGBA pixels or encode a PNG artifact

### Assertion surface

Recommended visual assertions:

- `to_match_screenshot("save-dialog")`
- `to_match_screenshot_with_threshold(...)`
- `capture_screenshot()` for manual debugging

Phase 2 can reasonably expose these through the Rust API as methods such as:

- `window.capture_screenshot()`
- `window.capture_artifacts()`
- `locator.expect().to_match_screenshot(path)`

Pixel assertions should be reserved for:

- renderer regressions
- paint-order bugs
- font and text rendering validation
- layout and clip regressions that semantics alone cannot detect

## Required Runtime and Platform Changes

The first version of `sui-testing` can be built mostly from existing code, but a few additions will materially improve the design.

### 1. Expose better harness idle state

`HeadlessPlatform` should expose whether queued events remain so `sui-testing` can implement precise auto-waiting without guessing.

Examples:

- `has_pending_events()`
- `pending_event_count()`
- `pump_until(predicate)` helper in the testing layer

### 2. Add screenshot readback

Needed for screenshot testing and failure artifacts.

### 3. Add semantics action dispatch

Not required for the first milestone, but important for long-term accessibility-driven automation.

### 4. Add snapshot revision tracking

The runtime or testing layer should track a monotonic snapshot revision so expectations can cheaply detect whether a new UI state was produced.

## Multi-Window Support

SUI already treats each window as an independent runtime island. `sui-testing` should keep that model.

Recommended surface:

- `app.main_window()`
- `app.window_by_title("Preferences")`
- `app.windows()`

Locators are always window-scoped unless a test explicitly chooses a cross-window query.

## Rust-First API, Protocol-Ready Core

The first implementation should be a Rust-native crate used directly from `cargo test`.

That should be the canonical implementation because:

- it is the cheapest path to real value
- it uses the current in-process runtime directly
- it avoids inventing a remote protocol before the object model is stable

However, the internal command model should be shaped so a later transport can expose it to JavaScript or Python.

### Future protocol direction

Once the Rust API is stable, a protocol layer can expose commands like:

- create app session
- list windows
- query locator
- perform action
- wait for expectation
- fetch snapshots
- capture screenshot
- advance time

This could back:

- a JavaScript client with Playwright-like method names
- a Python test client
- inspector or recorder tooling

The important point is that the protocol should mirror the Rust testing concepts rather than define a second mental model.

## Recommended API Style

The public API should bias toward the same authoring style that makes Playwright productive.

### Characteristics

- locator chaining over raw widget identifiers
- strict matching by default
- retrying expectations by default
- narrow escape hatches for raw events and internal snapshots
- concise success path, rich failure output

### Example shape

```rust
let save = window.get_by_role(SemanticsRole::Button).with_name("Save");

save.expect().to_be_visible()?;
save.click()?;
save.expect().to_be_focused()?;

window
    .get_by_role(SemanticsRole::Text)
    .with_name("Saved")
    .expect()
    .to_be_visible()?;
```

This style is preferable to resolving a widget id once and manually orchestrating redraws around each assertion.

## Incremental Rollout Plan

### Phase 1: deterministic in-process harness

Deliver:

- `sui-testing` crate skeleton
- `TestApp`, `TestWindow`, `Locator`, and `Expectation`
- selectors by role, name, text, and description
- actions for click, hover, press, focus, and fill
- auto-waiting driven by `HeadlessPlatform`
- semantics and widget-graph diagnostics

This phase provides real end-to-end value without waiting on protocol work.

### Phase 2: richer diagnostics and screenshots

Deliver:

- renderer readback
- screenshot assertions
- structured failure artifact bundles
- optional scene overlays for debugging hit testing and semantics bounds

Concrete Rust-facing APIs for this phase can include:

- `WgpuRenderer::capture_rgba(window_id)` for offscreen readback
- `TestWindow::capture_screenshot()`
- `TestWindow::capture_artifacts()`
- `Expectation::to_match_screenshot(path)`

### Phase 3: semantics action dispatch and recorder tooling

Deliver:

- runtime semantics action invocation
- test recorder hooks based on the same selector model
- improved text-input testing paths

### Phase 4: external automation protocol

Deliver:

- transport-neutral command schema
- JavaScript and Python client layers
- attach or spawn workflow for app-under-test processes

## Recommended First Milestone

The first implementation should stay narrow and ship the smallest coherent slice:

1. create the `sui-testing` crate
2. build a `TestApp` around `Runtime` plus `HeadlessPlatform`
3. implement locator resolution over `AccessibilitySnapshot`
4. implement `click`, `press`, `fill`, and core expectations
5. add strong failure dumps for semantics and widget graphs

That milestone will already support Playwright-style automated UI testing in the sense that matters most: high-level, locator-based, deterministic behavioral tests.

## Summary

SUI should treat automated UI testing as a first-class tooling surface built on the same retained runtime, semantics tree, and normalized event model used by real applications.

The correct architecture is a dedicated `sui-testing` crate that:

- builds on the existing headless platform rather than replacing it
- uses semantics-first locators rather than widget internals
- auto-waits against deterministic runtime progression
- keeps screenshots as an optional layer, not the default testing strategy
- leaves room for future JavaScript and Python clients without compromising the Rust-first design

That approach fits the existing SUI architecture, matches the intent already captured in the design notes, and creates a practical path toward Playwright-style automation for desktop and headless SUI applications.
