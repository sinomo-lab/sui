# Layout And Overflow

SUI uses a simple measure/arrange layout pipeline, not a CSS formatting engine. The goal is to keep common widget layout predictable while borrowing the parts of CSS sizing that solve real UI problems: finite wrap widths, minimum readable sizes, clipping, visible overflow, and scrollable overflow.

## Core Model

Every widget still participates in the same two-phase contract:

1. `measure(ctx, constraints) -> Size`
2. `arrange(ctx, bounds)`

`Constraints` are the parent's layout offer. The measured `Size` is the child's desired content extent under that offer. The arranged `Rect` is the viewport the parent actually gives the child.

Overflow is the policy for what happens when the measured content extent is larger than the arranged viewport. It should not be confused with the layout offer itself.

```text
layout offer -> measured content size -> arranged viewport -> overflow behavior
```

The important design rule is:

```text
scrollability must not imply infinite inline layout width
```

Text and other wrapping content should usually receive a finite width so it can wrap. A child can still create horizontal overflow by reporting a measured width larger than the viewport, commonly because it has a minimum readable width.

## Overflow Modes

`sui_widgets::Overflow` intentionally mirrors the small, familiar part of CSS overflow:

- `Visible`: content may paint outside the viewport and does not scroll on that axis.
- `Clip`: content is clipped to the viewport and does not scroll on that axis.
- `Scroll`: content is scrollable on that axis.
- `Auto`: content is scrollable on that axis when the measured content extent exceeds the viewport.

`Scroll` and `Auto` currently share the same core layout and input behavior. When content actually exceeds the viewport, `ScrollView` paints interactive scroll bars over the content without reserving layout space. Mouse wheels, trackpad scrolling, keyboard navigation, scroll-bar dragging, accessibility range actions, and direct touch panning all update the same scroll state. Touch panning waits for a short drag threshold and hands an exhausted gesture to an enclosing scroll view, so nested scrolling remains inner-first.

`VirtualScrollView` provides the same built-in vertical overlay and touch
behavior while arranging, visiting, and painting only its visible rows. It
still owns and measures every static child. Use `VirtualList` for data-backed
keyed realization where off-screen rows should not be constructed. To use a
permanently allocated gutter instead, share a `ScrollState` with a standalone
`ScrollBar` and call `.overlay_scroll_bars(false)` on the scroll view to avoid
duplicate controls.

## ScrollView Sizing

When a caller configures `ScrollView` through the explicit overflow API, `ScrollView` treats the horizontal and vertical axes differently because most UI content behaves like block layout:

- horizontal scrollable overflow gives children a finite width equal to the viewport width, so labels and paragraphs wrap;
- vertical scrollable overflow gives children unbounded height by default, so content can grow naturally and produce a vertical scroll extent.

This produces the common CSS-like pattern:

```text
content width = finite viewport width, unless a child reports a larger minimum width
content height = natural height
overflow-x = auto
overflow-y = auto
```

`ContentExtent` makes the content offer explicit on either axis:

- `Natural` gives the child an unbounded content axis.
- `Viewport` gives the child a finite maximum equal to the viewport.
- `AtLeastViewport` allows natural growth but requires at least the viewport.
- `AtLeast(points)` allows natural growth above an explicit minimum.
- `MinContent` asks the child for its intrinsic minimum and measures at that
  exact extent.

For example, a settings page can fill a short viewport while continuing to
grow vertically, and a horizontally scrollable inspector can choose the
smallest readable text width:

```rust
use sui::prelude::*;

let settings = ScrollView::vertical(form)
    .content_width(ContentExtent::Viewport)
    .content_height(ContentExtent::AtLeastViewport);

let inspector = ScrollView::both(details)
    .content_width(ContentExtent::MinContent)
    .content_height(ContentExtent::Natural);
```

The compatibility methods `viewport_size_hint`, `viewport_width_hint`, and
`viewport_height_hint` map to `Viewport` and `Natural`. New code should prefer
the content-extent methods because they state the actual layout policy.

The older `ScrollView::horizontal`, `ScrollView::vertical`, `ScrollView::both`, and `axes(...)` constructors remain compatibility conveniences. They configure scrollable axes, but they do not by themselves opt horizontal scrolling into finite wrap-width measurement. Prefer `overflow_x(...)` and `overflow_y(...)` when authoring new layout that depends on CSS-like overflow behavior.

## Intrinsic Sizing

Widgets may override `Widget::intrinsic_size` to report a minimum readable
extent and a preferred natural extent on one axis. The default is conservative:
it treats an ordinary unbounded measurement as both minimum and natural, so
custom widgets remain correct without implementing a second sizing contract.

`Label` reports its full line as the horizontal natural extent and its widest
whitespace-delimited segment as the horizontal minimum. `Grid` and
`ContentExtent::MinContent` consume this information. Intrinsic queries do not
replace normal measurement and do not mutate a pod's cached arranged bounds.

## Grid

`Grid` provides explicit retained two-dimensional layout. Tracks can be
`Fixed`, `Auto`, `Fraction`, or `MinMax`; cells may span rows and columns and
choose independent alignment:

```rust
use sui::prelude::*;

let grid = Grid::new([
    GridTrack::Fixed(220.0),
    GridTrack::MinMax {
        min: 280.0,
        max: GridTrackMax::Fraction(1.0),
    },
])
.rows([GridTrack::Auto, GridTrack::Fraction(1.0)])
.gap(12.0)
.with_cell(GridCell::new(0, 0).span(2, 1), navigation)
.with_cell(GridCell::new(0, 1), toolbar)
.with_cell(GridCell::new(1, 1), content);
```

The renderer-independent `grid_layout` solver is also public for custom
widgets. SUI's grid intentionally omits CSS named lines, implicit placement
algorithms beyond row-major `with_child`, and dense reordering.

## Non-Goals

This document is not a plan to implement full CSS. In particular, SUI does not currently model:

- the full CSS intrinsic-sizing algorithm and `fit-content` grammar;
- block and inline formatting contexts or absolute positioning;
- CSS margin collapsing or cascade behavior.

Those ideas may inspire future sizing APIs, but the core SUI model should remain small enough for custom widgets to reason about directly.
