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

`Scroll` and `Auto` currently share the same core layout and input behavior. Scrollbar visibility is still compositional: a `ScrollBar` is a separate widget bound through `ScrollState`, so a parent layout decides whether the bar is always present, conditionally present, or omitted.

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

For a validation surface or inspector that needs a minimum readable width, wrap the content in a widget that reports at least that width. The scroll view should still provide finite width to descendants so text wraps inside the chosen content column instead of measuring as one unwrapped line.

The older `ScrollView::horizontal`, `ScrollView::vertical`, `ScrollView::both`, and `axes(...)` constructors remain compatibility conveniences. They configure scrollable axes, but they do not by themselves opt horizontal scrolling into finite wrap-width measurement. Prefer `overflow_x(...)` and `overflow_y(...)` when authoring new layout that depends on CSS-like overflow behavior.

## Non-Goals

This document is not a plan to implement full CSS. In particular, SUI does not currently model:

- the full CSS intrinsic sizing algorithm;
- `min-content`, `max-content`, or `fit-content` as first-class sizing keywords;
- block, inline, flex, grid, and absolute formatting contexts;
- CSS margin collapsing or cascade behavior.

Those ideas may inspire future sizing APIs, but the core SUI model should remain small enough for custom widgets to reason about directly.

## Open Design Space

The current overflow API solves the immediate separation between wrapping and scrolling. Future layout work may add explicit size policies, for example:

```rust
enum ContentExtent {
    Viewport,
    Natural,
    AtLeast(f32),
    AtLeastViewport { min: f32 },
}
```

That would make minimum readable widths and natural content growth first-class instead of relying on local wrapper widgets. Until then, overflow should stay focused on clipping, visibility, and scrollability.
