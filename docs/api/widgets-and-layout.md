# Widgets and Layout

[Previous: getting started](getting-started.md) · [API guide](README.md) ·
[Next: input and text editing](input-and-editing.md)

SUI widgets are retained Rust values. A window owns one root widget, container
widgets own child `WidgetPod`s, and the runtime revisits that tree for events,
measurement, arrangement, painting, and accessibility semantics.

## Built-in Widget Families

The public facade groups widgets by the job they perform. These are the useful
families to learn rather than an exhaustive symbol inventory.

| Family | Common types | Use for |
| --- | --- | --- |
| Text and actions | `Label`, `RichText`, `Button`, `IconButton`, `Link` | Display text and invoke commands |
| Boolean and choice controls | `Checkbox`, `Switch`, `RadioGroup`, `SegmentedControl`, `Select`, `ComboBox` | Small finite choices |
| Text and numeric input | `TextInput`, `PasswordInput`, `DateTimeInput`, `TextArea`, `NumberInput`, `SpinBox`, `Slider` | Editable values and ranges |
| Basic layout | `Padding`, `Align`, `SizedBox`, `Stack`, `Flex`, `Background` | Size and position ordinary widget trees |
| Viewport and structure | `ScrollView`, `VirtualScrollView`, `SplitView`, `Dock`, `SwitchView` | Overflow, panes, and alternate content |
| Overlays and shells | `Dialog`, `Modal`, `Popover`, `ContextMenu`, `Tooltip`, `Drawer`, `SideSheet` | Transient or elevated interface layers |
| Data and navigation | `ListView`, `TreeView`, `Table`, `VirtualTable`, `Breadcrumb`, `TabBar`, `Tabs` | Collections and navigation state |
| Creative tools | `Canvas`, `PixelCanvas`, `ColorPicker`, `LayerList`, `BrushPreview` | Editor-style and graphics interfaces |

Constructors generally take the accessible label or name first. Choose names
that remain meaningful to screen-reader users and test locators.

## A Form Layout

Use `Stack` for a simple sequence along one axis. It measures each child in
order and places a fixed amount of spacing between children.

```rust
use sui::prelude::*;

fn account_form() -> impl Widget {
    Padding::all(
        24.0,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(Label::new("Create account").font_size(22.0))
            .with_child(
                TextInput::new("Display name")
                    .placeholder("Ada Lovelace")
                    .min_width(280.0),
            )
            .with_child(
                PasswordInput::new("Password")
                    .placeholder("At least 12 characters")
                    .min_width(280.0),
            )
            .with_child(
                DateTimeInput::new("Reminder time")
                    .value("2026-08-01 09:30")
                    .min_width(280.0),
            )
            .with_child(Button::new("Create account")),
    )
}
```

`TextInput` is single-line and owns its editing selection, caret, clipboard,
and IME state. `TextArea` (also exported as `MultilineTextInput`) is the
multiline editor. `PasswordInput` masks rendering, but its callback and stored
value are the real string; masking is not secret-memory protection.

`DateTimeInput` is deliberately a lightweight string field. Its suggested
format is `YYYY-MM-DD HH:MM`; the application owns parsing, validation,
calendar selection, timezone policy, and conversion to domain types.

## Stack Versus Flex

Use `Stack` when children keep their measured sizes. Use `Flex` when children
must grow, shrink, wrap, or share remaining space.

```rust
use sui::prelude::*;

fn search_bar() -> impl Widget {
    Flex::horizontal()
        .gap(8.0)
        .align_items(Alignment::Center)
        .with_child(Label::new("Search"))
        .with_item(
            TextInput::new("Query").placeholder("Type a query"),
            FlexItem::flex(1.0).min_width(120.0),
        )
        .with_child(Button::new("Run"))
}
```

Frequently used `FlexItem` policies are:

- `FlexItem::new()` for intrinsic sizing with normal shrinking.
- `FlexItem::fill()` to consume remaining room.
- `FlexItem::flex(weight)` to divide remaining room by weight.
- `FlexItem::fixed(points)` for a non-shrinking basis.
- `basis_gap_aware_fraction(fraction)` for fractional wrapped rows that must
  also account for the container gap.
- `min_width`, `max_width`, `min_height`, and `max_height` to bound an item.

For a wrapping card row:

```rust
use sui::prelude::*;

fn cards() -> impl Widget {
    Flex::horizontal()
        .wrap(FlexWrap::Wrap)
        .gap(12.0)
        .with_item(
            Label::new("First card"),
            FlexItem::new()
                .basis_gap_aware_fraction(0.5)
                .min_width(240.0),
        )
        .with_item(
            Label::new("Second card"),
            FlexItem::new()
                .basis_gap_aware_fraction(0.5)
                .min_width(240.0),
        )
}
```

## Sizing and Containment

- `Padding::{all, horizontal, vertical, new}` adds space around one child.
- `Align` places one child inside the space granted by its parent.
- `SizedBox` supplies an explicit width, height, or both.
- `Background` paints a brush behind one child.
- `SemanticRegion` adds an accessible grouping boundary without changing the
  visual hierarchy.

`Insets` is the layout-value type used by control builder methods such as
`Button::padding`. `Padding` in the prelude is the widget that wraps a child.

## Scrolling and Large Collections

Wrap content in `ScrollView::vertical`, `horizontal`, or `both` when it can
overflow a bounded viewport:

```rust
use sui::prelude::*;

fn log_view(lines: impl IntoIterator<Item = String>) -> impl Widget {
    let mut content = Stack::vertical().spacing(4.0);
    for line in lines {
        content.push(Label::new(line));
    }

    SizedBox::new()
        .height(320.0)
        .with_child(ScrollView::vertical(Padding::all(12.0, content)).name("Build log"))
}
```

Use `VirtualScrollView`, `VirtualTable`, or another virtualized data widget
when a collection is large enough that constructing and laying out every row
would be wasteful. Use an ordinary `ScrollView` for modest, heterogeneous
content.

`ScrollState` can be shared with `ScrollView::state` when application code
needs to inspect or control an offset. Without it, the view retains its own
scroll state.

## Responsive Structure

Most responsive changes only need `Flex` wrapping and item minimums. When the
actual widget structure must change at a breakpoint, use
`RebuildOnConstraints`:

```rust
use sui::prelude::*;

fn responsive_actions() -> impl Widget {
    RebuildOnConstraints::new(
        false,
        |constraints| constraints.max.width < 520.0,
        |narrow| {
            if *narrow {
                WidgetPod::new(
                    Stack::vertical()
                        .spacing(8.0)
                        .with_child(Button::new("Save"))
                        .with_child(Button::new("Cancel")),
                )
            } else {
                WidgetPod::new(
                    Flex::horizontal()
                        .gap(8.0)
                        .with_child(Button::new("Save"))
                        .with_child(Button::new("Cancel")),
                )
            }
        },
    )
}
```

Rebuilding replaces the child subtree, including its local interaction state.
Reserve it for structural changes. For value, selection, color, or label
changes, prefer reader callbacks and targeted invalidation as described in
[State, events, and background work](state-events-and-async.md).

## Layout Contract for Custom Widgets

The runtime uses two explicit phases:

1. `Widget::measure` receives `Constraints { min, max }` and returns a size
   clamped to that range.
2. `Widget::arrange` receives the final rectangle and positions retained
   children within it.

Painting must use the arranged bounds; it must not silently choose a different
layout. The built-in containers already honor this contract, so implement
custom layout only when the existing compositions cannot express it. See
[Custom widgets](custom-widgets.md) for the complete lifecycle.
