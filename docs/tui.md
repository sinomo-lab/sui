# Accessibility-Generated Terminal UI

SUI can project an application's accessibility tree into a terminal view. The
terminal renderer is useful for accessibility auditing, headless inspection,
and operating semantics-complete interfaces without reproducing their visual
renderer.

The implementation is split between:

- `sinomo-ui-tui`, which turns an `AccessibilitySnapshot` into structured or spatial
  terminal text and validates the snapshot;
- `sinomo-ui-demo`, which provides an interactive Ratatui host over the real demo
  runtime.

The accessibility tree remains the source of truth. The TUI does not inspect
private widget state or scene drawing commands.

## Run The Demo TUI

The default `sinomo-ui-demo` features include TUI support:

```bash
cargo run -p sinomo-ui-demo -- --tui
```

The default spatial layout projects semantic bounds into a terminal canvas and
also exposes an actionable tree. Use the structured layout for a hierarchy-only
view:

```bash
cargo run -p sinomo-ui-demo -- --tui --tui-layout=structured
```

Print a single snapshot without entering the interactive host:

```bash
cargo run -p sinomo-ui-demo -- --tui-dump-accessibility
```

Hidden semantic nodes are omitted by default. Include them when diagnosing
visibility or lifecycle problems:

```bash
cargo run -p sinomo-ui-demo -- --tui-dump-accessibility --tui-show-hidden
```

## Interactive Controls

Inside the interactive host:

| Key | Action |
| --- | --- |
| `Down`, `j`, `Tab` | Select the next actionable node |
| `Up`, `k`, `Shift+Tab` | Select the previous actionable node |
| `Enter`, `Space`, `a` | Activate the selected node |
| `e` | Edit a value-capable node |
| `Right`, `+` | Increment or move right |
| `Left`, `-` | Decrement or move left |
| `PageDown` / `PageUp` | Scroll the selected scroll surface |
| `q`, `Esc`, `Ctrl+C` | Exit |

Mouse selection and scrolling are also supported in terminals that report
mouse events.

## Library API

Applications and tools can use `sinomo-ui-tui` directly:

```rust,no_run
use sui_platform::AccessibilitySnapshot;
use sui_tui::{TuiLayoutMode, TuiRenderOptions, render_snapshot, validate_snapshot};

fn inspect(snapshot: &AccessibilitySnapshot) {
    let issues = validate_snapshot(snapshot);
    let frame = render_snapshot(
        snapshot,
        TuiRenderOptions {
            width: 100,
            height: 36,
            mode: TuiLayoutMode::Structured,
            show_hidden: false,
        },
    );

    println!("{frame}");
    println!("{} accessibility issue(s)", issues.len());
}
```

The primary public types are:

- `TuiSnapshot` and `TuiNode` for a hierarchy derived from semantic parents;
- `TuiRenderOptions` and `TuiLayoutMode` for output configuration;
- `TuiFrame` for rendered terminal lines;
- `validate_snapshot` and `AccessibilityIssue` for accessibility auditing.

## What Validation Checks

`validate_snapshot` reports errors, warnings, and informational findings for:

- missing or multiple roots;
- duplicate node IDs, missing parents, and cyclic parent chains;
- focus pointing to a missing or hidden node;
- unnamed interactive controls;
- roles missing expected actions or values;
- actionable nodes with empty bounds;
- visible children beneath hidden parents;
- duplicate visible role/name pairs that make automation ambiguous.

These checks are intentionally useful beyond the TUI. The same semantics tree
drives platform accessibility and `sinomo-ui-testing`, so a clean TUI validation pass
usually improves all three surfaces.

## Authoring Widgets For Automatic TUI Support

A widget does not implement a separate terminal renderer. Instead, its
`Widget::semantics` implementation should provide:

- a correct `SemanticsRole`;
- a concise, unique accessible name for interactive controls;
- the current value and state when the role carries one;
- supported `SemanticsAction` values;
- the correct parent relationship and logical bounds;
- `hidden` state when the widget is not available to the user.

Prefer standard semantic actions such as activation, focus, value changes,
selection, increment/decrement, and scrolling. Custom actions can remain
available to specialized hosts, but standard actions give accessibility,
testing, and generated TUI consumers the best interoperability.

See the [testing guide](./testing.md) for semantics-first interaction tests and
the [custom-widget API guide](./api/custom-widgets.md) for implementing a
widget's event, paint, and semantics contracts.
