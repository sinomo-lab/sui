# SUI Testing Guide

This document describes the testing surfaces that exist in the repo today.

The goal is to describe the current test layers, the harness behavior, and the properties of a reliable SUI test.

## Testing Layers

There are three main layers in the current workspace.

### 1. Unit-style runtime tests with `sui-testing`

This layer covers most interaction tests.

It provides:

- `TestApp` to boot a runtime from an application builder
- `TestWindow` to scope interactions to a window
- semantics-first `Locator` queries
- auto-waiting `Expectation` helpers
- screenshot and artifact capture helpers when needed

This is the default choice for widget behavior, semantics behavior, focus handling, IME-oriented input, and most user flows.

### 2. Headless platform validation

`sui-platform::HeadlessPlatform` drives the real runtime with deterministic event pumping and manual time advancement.

This matters because `sui-testing` is not a fake UI model. It runs against the same runtime boundaries used by the real application path.

### 3. Desktop harness and visual tests

`sui-widget-book` includes desktop-oriented tests and visual artifact generation. This layer covers cases that depend on the real desktop event loop, real surface presentation, or visual output reviewed as images.

## Core Testing Model

The repo is deliberately semantics-first.

That means tests should usually interact with the UI through:

- role
- accessible name
- text
- description
- value

The semantics tree is the stable observable surface shared by accessibility, automation, and testing. Widget graph internals are available for debugging, but they are not the main test API.

## `sui-testing` Object Model

The high-level object model is:

- `TestApp` for process-level setup
- `TestWindow` for per-window interactions
- `Locator` for lazy queries that re-resolve against the latest snapshot
- `Expectation` for retrying assertions

This is intentionally similar to Playwright's ergonomics, but it targets SUI semantics and normalized events instead of the DOM.

## What The Harness Actually Does

After an action or while waiting on an expectation, the harness repeatedly:

1. pumps queued events
2. lets the runtime handle those events
3. processes any required redraws
4. advances timers and async wakeups when needed
5. re-checks the latest semantics snapshot

This is why tests should not use ad hoc sleeps. The harness already knows how to drive the real runtime to a stable state.

## Common Test Style

The expected style is:

```rust
use sui::prelude::*;
use sui_testing::prelude::*;

#[test]
fn save_flow() -> Result<()> {
    let app = TestApp::new(|| {
        Application::new().window(
            WindowBuilder::new()
                .title("Editor")
                .root(build_root()),
        )
    })?;

    let window = app.main_window()?;

    window
        .get_by_role(SemanticsRole::TextInput)
        .with_name("Name")
        .fill("Ada")?;

    window
        .get_by_role(SemanticsRole::Button)
        .with_name("Save")
        .click()?;

    window.get_by_text("Saved").expect().to_be_visible()?;
    Ok(())
}
```

That style is already used by examples and tests in the repo.

## When To Use Which Layer

`sui-testing` fits cases such as:

- you are testing widget behavior
- you are testing focus, keyboard, pointer, or IME flows
- you are asserting semantics or visible text
- you want deterministic tests that run quickly in-process

Widget-book desktop tests fit cases such as:

- the issue reproduces only on the real desktop path
- scrolling, clipping, or rendering differs from headless output
- you need visual artifacts for review

Manual `sui-dev` runs fit cases such as:

- the problem is exploratory
- you need the performance overlay
- the failure depends on real interaction feel rather than a single assertion

## Common Commands

```bash
cargo test
cargo test -p sui-testing
cargo test -p sui-widget-book -- --nocapture
cargo run -p sui-dev
```

`cargo test -p sui-widget-book -- --nocapture` writes visual artifacts under `target/ui-artifacts/sui-widget-book`.

## Testing Guidelines

### Prefer semantics over internals

Assert what a user or automation system can observe. Roles, names, text, values, and focus state are better than direct widget graph inspection.

### Use unique semantics for gallery and story content

The widget-book tests rely on unique role and accessible-name combinations. If multiple nodes expose the same role and name in one story, locators become ambiguous.

### Prefer high-level actions

Use `click()`, `fill()`, `press()`, and related helpers unless the test is specifically about low-level event dispatch.

### Do not add sleeps

If a test needs waiting, it should usually be expressed as an expectation so the harness can keep pumping the runtime deterministically.

### Keep visual tests focused

Only use screenshot-style validation when behavior cannot be expressed cleanly through semantics and state assertions.

## Debugging Failing Tests

When a test fails, the useful sources are usually:

- the latest semantics snapshot
- widget-book visual artifacts
- runtime and renderer diagnostics
- targeted `sui-dev` reproduction in the desktop host

If a change affects rendering, also check whether the semantics tree still matches what the image suggests. Many regressions are not purely visual or purely semantic; they often touch both.

## Files Worth Reading

If you are changing the test surface itself, start here:

- `crates/sui-testing/src/app.rs`
- `crates/sui-testing/src/window.rs`
- `crates/sui-testing/src/locator.rs`
- `crates/sui-testing/src/expect.rs`
- `crates/sui-platform/src/headless.rs`
- `crates/sui-widget-book/tests/desktop_e2e.rs`

Those files show the current expected testing style more accurately than the old proposal docs did.
