# Accessibility-Generated SUI TUI Implementation Plan

Goal: Add a terminal UI path for SUI where usable TUIs are automatically generated from the accessibility tree. The first integration target is `sui-dev --tui`, backed by a new `sui-tui` crate that renders and drives SUI applications through their accessibility snapshots.

Architecture: Treat the accessibility tree as the source of truth for terminal rendering and interaction. The TUI does not need to duplicate the GUI's exact layout, drawing, animation, or pointer interaction model. Instead, it should preserve the same user-facing functionality for supported widgets by interpreting roles, names, values, state, actions, hierarchy, and bounds from `AccessibilitySnapshot`.

Tech Stack: Rust 2024, existing SUI runtime/platform/testing architecture, `sui-core::SemanticsNode`, `sui-platform::AccessibilitySnapshot`, `sui-testing` style locator/action semantics, a new `sui-tui` crate, and a `sui-dev --tui` launch path.

---

## Core idea

SUI already has a semantics-first architecture:

- widgets publish `SemanticsNode` values with roles, names, descriptions, values, state, actions, bounds, and parent links
- the runtime assembles those nodes during render work
- `sui-platform` stores them as `AccessibilitySnapshot`
- `sui-testing` already uses them as the stable interaction surface

The TUI should build on that existing contract. If a UI is composed entirely from supported semantic roles and actions, `sui-tui` should be able to generate a usable terminal interface automatically.

This means:

- the GUI remains the visual renderer
- the accessibility tree becomes a parallel functional renderer
- basic widgets and layouts get terminal support without hand-writing a second UI
- tests can verify that the accessibility tree is complete enough to drive real interaction
- `sui-dev --tui` becomes both a developer tool and an accessibility-tree validation surface

---

## Non-goals

- Do not make the TUI match GUI pixels or exact spatial layout.
- Do not duplicate widget implementation logic inside the TUI.
- Do not require every custom widget to provide a custom terminal renderer.
- Do not treat terminal support as a manually authored second frontend for `sui-dev`.
- Do not block the MVP on rich terminal graphics, mouse support, or full desktop live-bridge integration.

---

## Design principles

### Accessibility tree first

The TUI is generated from `AccessibilitySnapshot`, not from widget internals, scene commands, or layout objects.

Useful input fields are:

- `role`
- `name`
- `description`
- `value`
- `state`
- `actions`
- `bounds`
- `parent`

The widget graph can remain a debug fallback, but it must not be required for ordinary TUI rendering.

### Functional parity over layout parity

The terminal view should expose the same practical functionality as the GUI for supported widgets, even when the terminal structure is different.

Examples:

- a floating GUI workspace can become a list of views plus a focused content panel
- a visual tab bar can become a terminal tab selector
- a button grid can become a selectable list or grid
- sliders and spin boxes can become value rows with increment/decrement keys
- scroll views can become independently scrollable terminal regions

The standard should be: "Can the user discover, inspect, and operate the same controls?" not "Does it look like the GUI?"

### Automatic for supported roles

If every important node in an app uses roles that `sui-tui` understands, no app-specific TUI code should be needed.

Unsupported roles should degrade gracefully:

- render as named generic content when possible
- expose available actions if present
- report validation warnings for missing names, values, actions, or hierarchy

### Validation is a feature

Because the TUI depends on accessibility quality, it should make missing or ambiguous semantics visible. `sui-dev --tui` should help developers notice when the GUI works visually but the accessibility tree is not functional enough.

---

## Proposed crate layout

Add:

```text
crates/sui-tui/
  Cargo.toml
  src/lib.rs
  src/model.rs
  src/render.rs
  src/validate.rs
  src/interact.rs
```

Responsibilities:

- `model`: normalize `AccessibilitySnapshot` into a terminal-friendly tree/model
- `render`: produce terminal frames from the model
- `validate`: report accessibility issues that affect generated TUI quality
- `interact`: map terminal commands to semantic actions

The first version should keep terminal rendering mostly pure:

```rust
pub struct TuiRenderOptions {
    pub width: u16,
    pub height: u16,
    pub mode: TuiLayoutMode,
    pub show_hidden: bool,
}

pub enum TuiLayoutMode {
    Structured,
    Spatial,
}

pub fn render_snapshot(
    snapshot: &AccessibilitySnapshot,
    options: TuiRenderOptions,
) -> TuiFrame;
```

The exact types can evolve, but the important boundary is that snapshot-to-frame rendering can be tested without launching a terminal.

---

## `sui-dev --tui`

Extend `sui-dev` launch parsing with:

```text
cargo run -p sui-dev -- --tui
```

Behavior:

1. Build the normal `sui-dev` application.
2. Run it through a headless or terminal-compatible platform loop.
3. Capture the current `AccessibilitySnapshot`.
4. Render that snapshot through `sui-tui`.
5. Map terminal input back to semantic actions.
6. Pump the runtime and redraw the TUI after changes.

The GUI path should remain the default. `--tui` is an alternate host for the same app, not a separate app.

Suggested initial flags:

```text
--tui
--tui-layout=structured|spatial
--tui-show-hidden
--tui-dump-accessibility
```

`--tui-dump-accessibility` can be a useful non-interactive mode for CI and debugging.

---

## Terminal views

### Structured mode

This should be the default MVP mode.

It renders the accessibility tree as a task-oriented interface:

- top-level landmarks/windows/views as sections
- interactive controls as selectable rows
- text content as read-only rows
- containers as groups
- scroll views as navigable regions

This mode is robust even when GUI bounds do not map well to terminal cells.

### Spatial mode

This mode uses node bounds to approximate the GUI layout in terminal cells.

It is useful for debugging:

- whether semantic bounds are sane
- whether hidden/offscreen nodes are published correctly
- whether focus and hover state match expectations
- whether scroll content moves through the accessibility tree

Spatial mode is not required to be the most usable mode.

### Details and issues panel

Both modes should support inspecting the selected node:

- id
- role
- name
- description
- value
- actions
- state
- bounds
- parent

The same panel should show validation issues for the whole snapshot and for the selected node.

---

## Supported role behavior

Initial role support should focus on the widgets already used throughout `sui-dev` and `sui-widget-book`.

| Role | TUI representation | Expected actions |
| --- | --- | --- |
| `Window` / `Root` | top-level document | navigate children |
| `GenericContainer` | group/section | navigate children |
| `Text` | read-only text row | none |
| `Button` | selectable command | activate |
| `CheckBox` | toggle row | activate |
| `Switch` | toggle row | activate |
| `RadioButton` / `RadioGroup` | choice list | activate selected option |
| `Tabs` / `TabBar` | tab selector | activate tab |
| `TextInput` | editable field | focus, set value |
| `SpinBox` | numeric field | increment, decrement, set value |
| `Slider` | value control | increment, decrement, set value |
| `ComboBox` | selectable menu | expand, collapse, choose |
| `List` / `Tree` | navigable collection | select/expand when actions exist |
| `Table` | row/column browser | navigate cells/rows |
| `ScrollView` | scrollable region | scroll, focus |
| `Dialog` / `Popover` / `Menu` | modal or overlay section | navigate/activate |
| `Tooltip` | contextual text | read-only |
| `Image` / `Canvas` / `ColorSwatch` | described visual item | read name/description/value |
| `ColorPicker` | value editor | set value if supported |

Unsupported roles should still appear with their name, description, value, and actions.

---

## Interaction model

The TUI interaction layer should operate on semantic concepts, not widget internals.

Suggested keys:

```text
Up/Down          move selection
Left/Right       collapse/expand or decrement/increment
Enter/Space      activate selected node
Tab/Shift+Tab    move focus among focusable/actionable nodes
/                filter by role/name/text
Esc              close popup/filter or back out
PgUp/PgDn        scroll current scroll view
Home/End         jump within current list/region
?                show key help
q                quit TUI host
```

For text input, the first implementation can use a simple prompt/edit mode instead of trying to emulate every GUI editing gesture.

The interaction layer should prefer declared actions:

- `Activate` for buttons/toggles/menu items
- `Focus` for focusable nodes
- `Expand` / `Collapse` for disclosure controls
- `Increment` / `Decrement` for sliders/spin boxes
- `SetValue` for text/value controls

If a role implies an action but the node does not publish it, the TUI should warn rather than silently invent behavior.

---

## Accessibility validation checks

Add reusable validation in `sui-tui::validate`.

Initial checks:

- no root node
- multiple root nodes
- duplicate node ids
- parent points to missing node
- parent cycle
- focused widget missing from nodes
- focused widget is hidden
- actionable control has no accessible name
- interactive role has no expected action
- value role has no value
- duplicate `role + name` combinations among visible actionable nodes
- non-finite bounds
- empty bounds on visible actionable nodes
- hidden parent with visible child

Validation severity:

- `Error`: TUI cannot faithfully operate this part of the UI
- `Warning`: usable but likely ambiguous or incomplete
- `Info`: useful diagnostic detail

These checks should later feed `sui-testing` diagnostics too, but `sui-tui` can own the first implementation.

---

## Implementation tasks

### Task 1: Create the `sui-tui` crate with pure snapshot rendering

Objective: Add a crate that can render a small hand-built `AccessibilitySnapshot` into deterministic text output.

Files:

- Add: `crates/sui-tui/Cargo.toml`
- Add: `crates/sui-tui/src/lib.rs`
- Add: `crates/sui-tui/src/model.rs`
- Add: `crates/sui-tui/src/render.rs`
- Modify: workspace `Cargo.toml` only if needed for shared dependencies

Steps:

1. Build a normalized model from `AccessibilitySnapshot`.
2. Implement structured text rendering.
3. Add tests for buttons, text, containers, and focus markers.
4. Verify:

```powershell
cargo test -p sui-tui
cargo check -p sui-tui
```

---

### Task 2: Add accessibility validation

Objective: Make the TUI reveal semantics problems that prevent automatic generation from being reliable.

Files:

- Add: `crates/sui-tui/src/validate.rs`
- Test: `crates/sui-tui/src/validate.rs`

Steps:

1. Add `AccessibilityIssue`, `AccessibilityIssueSeverity`, and `validate_snapshot`.
2. Test missing roots, duplicate ids, missing names, missing actions, bad parents, and ambiguous controls.
3. Render validation summaries in the TUI frame.
4. Verify:

```powershell
cargo test -p sui-tui
```

---

### Task 3: Add role adapters for common widgets

Objective: Give supported SUI widgets useful automatic terminal representations.

Files:

- Modify: `crates/sui-tui/src/model.rs`
- Modify: `crates/sui-tui/src/render.rs`
- Add: `crates/sui-tui/src/interact.rs`

Steps:

1. Add role-to-terminal-control mapping for core roles.
2. Add value formatting for `SemanticsValue`.
3. Add state formatting for focused, disabled, hidden, checked, selected, expanded, and busy.
4. Add tests for each supported role group.
5. Verify:

```powershell
cargo test -p sui-tui
```

---

### Task 4: Integrate `sui-dev --tui`

Objective: Launch the normal `sui-dev` app through the generated TUI path.

Files:

- Modify: `crates/sui-dev/Cargo.toml`
- Modify: `crates/sui-dev/src/lib.rs`
- Possibly modify: `crates/sui-dev/src/main.rs`

Steps:

1. Add dependency on `sui-tui`.
2. Extend desktop launch parsing to accept `--tui`.
3. Add tests for launch parsing.
4. Implement a first non-interactive TUI render path that boots `build_dev_application()`, captures the main accessibility snapshot, and prints a deterministic TUI frame.
5. Verify:

```powershell
cargo test -p sui-dev --lib
cargo run -p sui-dev -- --tui-dump-accessibility
```

---

### Task 5: Add interactive terminal host

Objective: Make `sui-dev --tui` usable as an interactive terminal app.

Files:

- Modify: `crates/sui-tui/src/interact.rs`
- Modify: `crates/sui-dev/src/lib.rs`
- Possibly add: `crates/sui-tui/src/terminal.rs`

Steps:

1. Choose a terminal backend dependency after the pure renderer is stable.
2. Run the SUI runtime on the headless platform loop.
3. Render the latest snapshot after each input/action.
4. Route terminal commands through semantic actions.
5. Keep the event loop deterministic enough for tests.
6. Verify manually:

```powershell
cargo run -p sui-dev -- --tui
```

---

### Task 6: Add semantic action execution

Objective: Make generated TUI controls operate the underlying SUI app.

Files:

- Modify: `crates/sui-tui/src/interact.rs`
- Possibly modify: `crates/sui-testing/src/locator.rs`
- Possibly expose reusable action helpers from `sui-testing`

Steps:

1. Reuse `sui-testing` concepts where possible rather than inventing a separate action system.
2. Map selected semantic nodes to focus, activate, scroll, increment/decrement, and set-value actions.
3. Add integration tests using a simple app with button, switch, text input, slider, and scroll view.
4. Verify:

```powershell
cargo test -p sui-tui
cargo test -p sui-testing
```

---

### Task 7: Validate `sui-dev` automatic TUI coverage

Objective: Prove that the current dev workspace can be represented usefully without app-specific TUI code.

Files:

- Test: `crates/sui-dev/src/app.rs` or a new integration test
- Possibly modify widget semantics where validation exposes gaps

Steps:

1. Boot `build_dev_application()` in a test.
2. Render the generated TUI.
3. Assert that major controls/views appear:
   - `Widget book`
   - `64 buttons`
   - `Retained text`
   - `Text comparison`
   - `Text validation`
   - `Text editing`
   - `HDR validation`
   - `Settings`
4. Assert that there are no validation `Error` issues for visible supported controls.
5. Fix missing semantics in widgets if the TUI exposes real accessibility gaps.
6. Verify:

```powershell
cargo test -p sui-dev --lib
cargo test -p sui-widget-book
```

---

### Task 8: Document generated TUI authoring expectations

Objective: Make widget authors understand how to get automatic TUI support.

Files:

- Modify: `docs/testing.md`
- Modify: `docs/design.md`
- Optional: Add `docs/tui.md`

Document:

- accessibility tree as the source of automatic TUI generation
- supported roles and expected actions
- how to make custom widgets TUI-compatible
- validation issues and how to fix them
- when custom terminal adapters might be needed

Verify:

```powershell
cargo check -p sui-dev
cargo test -p sui-tui
```

---

## MVP definition of done

- `sui-tui` exists as a crate.
- It can render `AccessibilitySnapshot` values into deterministic terminal output.
- It validates common accessibility problems that affect automatic TUI generation.
- `sui-dev --tui-dump-accessibility` or equivalent can render the dev app's current accessibility tree.
- `sui-dev --tui` can run an interactive terminal host for basic navigation and activation.
- Basic widgets and layouts used by `sui-dev` produce a usable TUI automatically.
- The TUI does not rely on GUI-specific layout or widget internals.

---

## Longer-term direction

After the MVP, `sui-tui` can grow into a general alternate host for SUI apps:

- richer terminal layouts
- terminal mouse support
- modal/dialog handling polish
- table/tree specific navigation
- better text editing
- role-specific adapters for complex widgets
- CI snapshots of generated TUIs
- shared validation in `sui-testing` failure output
- optional live sidecar mode attached to a desktop SUI process

The long-term success condition is simple: a SUI app built from accessible, supported widgets should get a functional TUI for free.
