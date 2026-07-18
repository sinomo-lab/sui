# Widgets and Layout

[Previous: getting started](getting-started.md) · [API guide](README.md) ·
[Next: overlays and desktop interaction](overlays-and-desktop.md)

SUI widgets are retained Rust values. A window owns one root widget, container
widgets own child `WidgetPod`s, and the runtime revisits that tree for events,
measurement, arrangement, painting, and accessibility semantics.

## Built-in Widget Families

The public facade groups widgets by the job they perform. These are the useful
families to learn rather than an exhaustive symbol inventory.

| Family | Common types | Use for |
| --- | --- | --- |
| Text, documents, and actions | `Label`, `RichText`, `RichDocumentView`, `Button`, `IconButton`, `Link` | Display text or streaming Markdown and invoke commands |
| Boolean and choice controls | `Checkbox`, `Switch`, `RadioGroup`, `SegmentedControl`, `Select`, `ComboBox` | Small finite choices |
| Text and numeric input | `TextInput`, `PasswordInput`, `DateTimeInput`, `TextArea`, `NumberInput`, `SpinBox`, `Slider` | Editable values and ranges |
| Basic layout | `Padding`, `Align`, `SizedBox`, `Stack`, `Flex`, `Grid`, `AspectRatio`, `Background` | Size and position ordinary widget trees |
| Viewport and structure | `ScrollView`, `VirtualScrollView`, `SplitView`, `AdaptiveView`, `ConstraintView`, `ResponsiveSidebar`, `MasterDetail`, `SafeArea` | Overflow, panes, and adaptive workspace structure |
| Overlays and shells | `Dialog`, `Modal`, `CommandPalette`, `Popover`, `ContextMenu`, `Tooltip`, `Drawer`, `SideSheet`, `BottomSheet` | Managed transient or elevated interface layers |
| Data and navigation | `ListView`, `VirtualList`, `TreeView`, `Table`, `VirtualTable`, `Breadcrumb`, `TabBar`, `Tabs` | Collections and navigation state |
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
            .with_child(Button::primary("Create account")),
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
        .with_child(Button::primary("Run"))
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

`AspectRatio` constrains any retained child, not only an image. `Contain` is
the default; `AspectRatioFit::Cover` fills and clips the available rectangle.

Use `Grid` when rows and columns must share track measurements:

```rust
use sui::prelude::*;

fn workspace_grid(sidebar: impl Widget + 'static, content: impl Widget + 'static) -> impl Widget {
    Grid::new([
        GridTrack::Fixed(240.0),
        GridTrack::MinMax {
            min: 320.0,
            max: GridTrackMax::Fraction(1.0),
        },
    ])
    .rows([GridTrack::Auto, GridTrack::Fraction(1.0)])
    .gap(12.0)
    .with_cell(GridCell::new(0, 0).span(2, 1), sidebar)
    .with_cell(GridCell::new(0, 1), Toolbar::horizontal())
    .with_cell(GridCell::new(1, 1), content)
}
```

`Auto` tracks consume natural child extents, fractional tracks share remaining
finite room, and `MinMax` supplies an explicit floor and automatic, point, or
fractional ceiling. Cells retain normal widget identities and support row and
column spans plus per-axis alignment.

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

Use `VirtualList` or `VirtualTable` when a collection is large enough that
constructing and laying out every row would be wasteful. `VirtualScrollView`
keeps a static retained child set and virtualizes arrangement, visitation, and
paint, but still measures every child to obtain exact heterogeneous heights.
Use an ordinary `ScrollView` for modest content. See
[Virtual collections](virtual-collections.md) for keyed incremental data,
anchoring, follow-end, and retained-row policies.

`ScrollState` can be shared with `ScrollView::state` when application code
needs to inspect or control an offset. Without it, the view retains its own
scroll state.

Use `content_width` and `content_height` to separate the viewport from the
child's content offer. `ContentExtent::Viewport` enables wrapping at viewport
width, `Natural` allows growth, `AtLeastViewport` fills short viewports,
`AtLeast(points)` establishes an application minimum, and `MinContent` uses
the child's intrinsic minimum. The older `viewport_*_hint` methods remain
compatibility shims.

`ScrollView` and `VirtualScrollView` show interactive overlay scroll bars only
when their content exceeds the viewport. The bars do not consume layout space,
follow the view's static or dynamic theme, and support pointer dragging and
accessibility range actions. Scrollable containers also support direct touch
panning with nested inner-to-outer handoff at a scroll boundary.

For a traditional gutter, bind a standalone `ScrollBar` to the same
`ScrollState` and disable the built-in overlay:

```rust
let state = ScrollState::new();
let content = ScrollView::vertical(log_rows)
    .state(state.clone())
    .overlay_scroll_bars(false);
let gutter = ScrollBar::vertical(state);
```

## Responsive Structure

Most responsive changes only need `Flex` wrapping and item minimums. When the
presentation really changes at a breakpoint, use `AdaptiveView`. It derives
its class from its own incoming width constraints rather than global window
size, and retains all three variant subtrees while only visiting the active
one:

```rust
use sui::prelude::*;

fn adaptive_actions() -> impl Widget {
    AdaptiveView::new(
        Stack::vertical()
            .spacing(8.0)
            .with_child(Button::primary("Save"))
            .with_child(Button::new("Cancel")),
        Flex::horizontal()
            .gap(8.0)
            .with_child(Button::primary("Save"))
            .with_child(Button::new("Cancel")),
        Flex::horizontal()
            .gap(12.0)
            .with_child(Button::primary("Save changes"))
            .with_child(Button::new("Cancel")),
    )
    .breakpoints(AdaptiveBreakpoints::new(520.0, 900.0))
}
```

For more specific local policies, use `ConstraintView`. Its ordered,
declarative queries can combine minimum and maximum width or height, aspect
ratio, and portrait/landscape orientation. The first match wins; the fallback
and every query branch stay retained:

```rust
let responsive = ConstraintView::new(compact)
    .when(
        ConstraintQuery::new()
            .min_width(720.0)
            .orientation(ConstraintOrientation::Landscape),
        wide,
    )
    .when(ConstraintQuery::new().min_height(600.0), tall);
```

Queries inspect incoming constraints, so a component embedded in a split pane
adapts to the pane rather than the outer window.

Each adaptive variant has a stable widget identity and focus scope. Returning
to a variant restores its last focused descendant, with first-focusable
fallback. A variant's state is retained, but the three variants are still
distinct trees. Do not copy one stateful editor into every variant when the
same logical pane should survive every presentation.

For common workspace structures, prefer the policy widgets that retain one
copy of each logical pane:

- `ResponsiveSidebar` keeps sidebar and content pods stable. Compact width
  turns the sidebar into a dismissible overlay; wider widths use a rail or
  inline pane. `ResponsiveSidebarState` controls overlay visibility and
  collapse, while `SplitState` supplies the persisted inline width.
- `MasterDetail` keeps master and detail pods stable. Compact width shows the
  route selected by `MasterDetailState`; wider widths show both. Escape moves
  from detail back to master in compact mode.
- `FocusScope` and `FocusScopeState` are available for custom adaptive
  containers that need the same last-focused-or-first-focusable restoration.

`RebuildOnConstraints` remains available for genuinely disposable structure.
It replaces its child subtree and therefore resets local editor, focus,
selection, and animation state.

## Toolbars, Safe Areas, and Layout Motion

`Toolbar::wrapping()` flows retained actions onto additional rows or columns.
`line_spacing` controls the cross-line gap; logical child order and keyboard
navigation do not change when wrapping changes.

`SafeArea` consumes `DpiInfo::safe_area` on selected `SafeAreaEdges`. Platform
or embedding adapters update it with `WindowEvent::SafeAreaChanged`; the
runtime invalidates layout without rebuilding. Use `minimum` when an edge also
needs application-defined padding:

```rust
let mobile_shell = SafeArea::new(content)
    .edges(SafeAreaEdges::TOP.union(SafeAreaEdges::HORIZONTAL))
    .minimum(SafeAreaInsets::new(12.0, 8.0, 12.0, 0.0));
```

Wrap a stable child in `LayoutTransition` when a retained layout policy moves
it between origins. Arrangement immediately settles at the destination while
the compositor animates translation from the previous visual origin:

```rust
let inspector = LayoutTransition::new(inspector)
    .duration(0.20)
    .easing(Easing::EaseOut);
```

Transition continuity follows the `WidgetPod` identity. When a keyed parent
reconciles around it, focus, selection, editor state, and scroll position stay
with the same child. Size changes currently snap; only origin movement is
animated because it can use transform-only invalidation without re-painting.

## Split Pane State and Persistence

`SplitView` accepts either fractional or pixel sizing through `SplitExtent`.
Share a `SplitState` when application controls need to collapse a pane, update
its size, or persist it:

```rust
use sui::prelude::*;

let split = SplitState::pixels(320.0);
let layout = SplitView::horizontal(sidebar, content)
    .state(split.clone())
    .min_first(220.0)
    .min_second(360.0);

split.collapse(SplitPaneSide::First);
split.expand();

// Store this plain value in the application's settings format.
let persisted: SplitStateSnapshot = split.snapshot();
split.apply_snapshot(persisted);
```

SUI deliberately does not choose a settings store or serialization format.
The snapshot is the persistence boundary. Pointer dragging preserves the
state's authored unit: a pixel split remains pixel-sized and a fractional
split remains proportional.

## Drawers and Bottom Sheets

`SideSheet` (also exported as `Drawer`) anchors to the left or right edge.
`BottomSheet` uses the same modal scrim, actions, dismissal, entrance motion,
dialog semantics, and focus-return contract while exposing height rather than
width. Use `SheetState` to let sibling controls present either sheet without
rebuilding it:

```rust
use sui::prelude::*;

let sheet = SheetState::default();
let open_sheet = sheet.clone();

let open = Button::new("Filters").on_press(move || {
    open_sheet.show();
});
let panel = BottomSheet::new("Filters", filter_form)
    .state(sheet)
    .height(360.0);
```

When a state-backed sheet is dismissed by Escape or its scrim, it hides itself
and invokes `on_dismiss`. Focus returns to the widget that owned focus before
the sheet opened when that widget is still present.

These APIs cover stable application-shell composition; they are not a docking
framework. Use `FloatingWorkspace` for independent floating panels today.
Tab docking, drop zones, detachable windows, and serialized dock graphs should
remain a separate subsystem until more than one application needs the same
policy.

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
