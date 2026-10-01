# Widget API Conventions

[API guide](README.md)

Built-in widgets are configured the same way, so what you learn on one carries
to the next. This page states the conventions. New and changed widgets follow
them; older widgets are moving to them through deprecations. The
[migration plan](../plans/widget-api-conventions-plan.md) lists the widgets that
still differ.

## Builders and Getters

A widget is configured with builder methods that take `self`, set one
property, and return `Self`. A builder is named for its property, without a
prefix: `text`, `selected`, `enabled`, `gap`.

```rust,no_run
use sui::prelude::*;

let save = Button::primary("Save").enabled(false).focus_on_press(false);
```

Builder order never matters. A widget stores what it is given and resolves
values that depend on others, such as theme defaults, when it measures and
paints. A later `.theme(...)` does not undo an earlier `.color(...)`.

A getter reads state and never shares a name with a builder on any widget:

- Booleans read as `is_<property>()`: `is_checked()`, `is_enabled()`.
- Values that change while the widget runs read as `current_<property>()`,
  except for the selection, which reads as `selected_index()`.
- A setter that changes a built widget in place is `set_<property>(...)`.

Methods without arguments are presets that set several properties at once,
such as `Button::primary_action()` or `Checkbox::plain()`. A single on/off
setting is a flag that takes a `bool`.

## Static, Polled, and Observed Values

A property that can change after the widget is built comes in up to three
forms, always spelled the same way:

| Form | Signature | Use when |
| --- | --- | --- |
| `p(value)` | `p(T)` | The value is fixed once built |
| `p_when(reader)` | `p_when(Fn() -> T)` | The value lives elsewhere and is read when the widget needs it |
| `p_from(observable)` | `p_from(impl Observable<T>)` | The value is an `Observable`, such as a `Signal`, and the widget should repaint when it changes |

A reader passed to `_when` takes no arguments. `_from` subscribes the widget,
so it updates without anything else asking it to repaint; `_when` relies on
something else invalidating the widget, as described in
[state, events, and background work](state-events-and-async.md). Not every
property offers all three forms, but none is spelled any other way.

## Events

A callback is named `on_<event>` and receives the event's payload. Its
`on_<event>_with_ctx` twin receives the `EventCtx` first, then the same
payload, so the handler can invalidate other widgets or send commands:

```rust,no_run
use sui::prelude::*;

let files = ListView::new("Files")
    .on_change(|index, label| println!("{index}: {label}"))
    .on_change_with_ctx(|ctx, _index, _label| ctx.request_paint());
```

Every `on_<event>` has a `_with_ctx` twin, with two exceptions. Drag and drop
handlers need the context to answer a drag, so `on_drag_start`, `on_drop`, and
the other drag events take it directly, first. Callbacks that fire while the
widget lays out, such as `AdaptiveView::on_class_change` and
`ResponsiveSidebar::on_mode_change`, have no event context to pass, so they
have no twin.

A widget keeps both callbacks of a pair and calls both, the plain one first.
`RichDocumentView` is the exception: its `on_link`, `on_image`, and
`on_attachment` share one slot with their twins, so setting either replaces
the other.

The `on_` prefix is reserved for callbacks. Events use these names:

| Event | Fires when | Payload |
| --- | --- | --- |
| `on_change` | The user changed the widget's value: text, number, color, checked state, or selection | The new value. A selection reports the index first, then the item's label |
| `on_activate` | The user committed an item: Enter, a double click, a menu choice | The item's index or path, then the item |
| `on_press` | A button was pressed | Nothing, or the button's value |
| `on_submit` | A text field was submitted with Enter | The text |
| `on_<state>_change` | Another piece of state changed, such as `on_open_change` or `on_focus_change` | The new state |
| `on_dismiss` | The user dismissed an overlay | Nothing |

## State Vocabulary

The same kind of state has the same name on every widget:

| State | Builders | Getter | Event |
| --- | --- | --- | --- |
| Two-state input: `Checkbox`, `Switch`, `RadioButton` | `checked`, `checked_when`, `checked_from` | `is_checked` | `on_change(bool)` |
| Which item of a collection is selected | `selected(impl Into<Option<usize>>)`, `selected_when`, `selected_from` with `Option<usize>` | `selected_index() -> Option<usize>` | `on_change` |
| Whether one item or button is selected within a group | `selected(bool)`, `selected_when` | `is_selected` | The group's `on_change` |
| Availability of an interactive widget or item | `enabled(bool)`, `enabled_when`, `enabled_from` | `is_enabled` | |
| Editability of an input | `read_only(bool)` | `is_read_only` | |
| Whether an overlay is open | `open(bool)`, `open_when`, `open_from` | `is_open` | `on_open_change(bool)` |

`selected` takes `impl Into<Option<usize>>`, so `selected(2)` and
`selected(None)` both work. A widget that always shows one choice, such as
`TabBar`, `Tabs`, or `SegmentedControl`, treats `None` as its first item, and
its `selected_index()` is `None` only when it has no items. `TreeView` also
takes `selected_path(...)` for nested rows, in the form its `on_change`
reports.

A few widgets keep their own words where the meaning differs:
`SwitchView` is a container that shows one of its children, so its `selected`
takes that child's index; `Breadcrumb` marks the `current` location, as
`aria-current` does, rather than a selection; and `Presence` animates content in
and out rather than opening an overlay, so it keeps `shown_when` and
`shown_from`.

## Themes

Every themed widget takes `theme(DefaultTheme)` for a fixed theme and
`theme_when(Fn() -> DefaultTheme)` for a live one. The theme supplies defaults
only: properties set explicitly on the widget win over it, whichever was set
first.

A disabled built-in control paints with `DefaultTheme::for_disabled_control()`,
which turns text to the disabled ink, fades fills, and weakens borders and
accents. A custom widget can paint its disabled state the same way.

## Names and Labels

- `label` is visible text, nothing else.
- A widget with visible text uses it as its accessible name. Override the name
  with `semantic_name(...)` when the visible text alone is ambiguous.
- An interactive or landmark widget without visible text, such as `Slider` or
  `ListView`, takes its accessible name as the first argument of `new`.

## Layout and Appearance

| Concept | Name |
| --- | --- |
| Space between children | `gap`; `main_gap` and `cross_gap` when the two axes differ |
| Space inside the widget's edge | `padding(Insets)` |
| Size | `size`, taking a `Size` for rectangles and an `f32` for square glyphs such as icons and spinners |
| Rounded corners | `corner_radius(f32)` |
| Text style | `text_style(TextStyle)` |
| Preset look | `appearance(...)` with an enum of looks |
| Color overrides | `colors(...)` with a struct of colors |

## Children and Items

- `child(widget)` sets a widget's single child, replacing any earlier one.
- `with_child(widget)` appends a child to a widget that holds several.
- `item(...)` and `items(...)` add data entries, such as list items, menu
  items, or tree nodes, rather than child widgets.

## Value Types

A configuration type is either plain data or encapsulated, never both:

- Plain data has public fields and `Default`, with no builders that validate,
  so what you write is what the widget gets.
- An encapsulated type keeps its fields private and offers a constructor,
  builders, and getters, so it can validate and clamp its input.
