# Layout Rework Proposal

## Goal

Replace the current single-call `Widget::layout` contract with a two-phase layout model that keeps the existing retained widget tree and explicit invalidation model, while making room for:

- intrinsic sizing
- text-driven measurement without ad hoc widget-local hacks
- arrange-only updates such as scrolling and viewport panning
- future viewport and virtualization containers
- stronger layout caching and diagnostics

This proposal does not introduce a CSS engine or a full constraint solver. The target is a graphics-native retained layout system that stays predictable under profiling.

## Current Pain Points

The current runtime contract is:

1. parent sends `Constraints`
2. child returns `Size`
3. parent mutates child bounds directly

That is simple, but it forces several concerns into one step:

- measurement and placement are coupled
- widgets must compute child size and child position in the same call
- scrolling and similar offset-only changes still look like generic layout work
- there is no clean framework hook for intrinsic or baseline queries
- runtime caching can only key off final bounds, not measured intent

## Proposed Model

Adopt a measure/arrange layout pipeline.

1. `measure`: a widget receives constraints and returns its desired size.
2. `arrange`: the runtime gives the widget its final rect, and the widget positions children inside that rect.

This keeps `Constraints` as the main layout currency. The main change is separating "how big do you want to be" from "where do your children end up".

## API Shape

### Trait split

`WidgetLayout` becomes the layout participation surface. `Widget` keeps event, paint, and semantics behavior.

```rust
pub trait WidgetLayout {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.max
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let _ = (ctx, bounds);
    }

    fn intrinsic_size(
        &mut self,
        ctx: &mut IntrinsicCtx,
        axis: Axis,
        cross_extent: Option<f32>,
    ) -> Option<f32> {
        let _ = (ctx, axis, cross_extent);
        None
    }

    fn baseline(&mut self, _ctx: &mut BaselineCtx, _bounds: Rect) -> Option<f32> {
        None
    }
}

pub trait Widget: WidgetLayout {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn paint(&self, _ctx: &mut PaintCtx) {}

    fn semantics(&self, _ctx: &mut SemanticsCtx) {}

    fn accepts_focus(&self) -> bool {
        false
    }

    fn focus_changed(&mut self, _ctx: &mut EventCtx, _focused: bool) {}

    fn visit_children(&self, _visitor: &mut dyn WidgetPodVisitor) {}

    fn visit_children_mut(&mut self, _visitor: &mut dyn WidgetPodMutVisitor) {}
}
```

### Context split

The current `LayoutCtx` should be replaced by narrower contexts.

```rust
pub struct MeasureCtx {
    // current widget id, dpi, text system, font registry, image registry
}

pub struct ArrangeCtx {
    // current widget id, dpi, invalidation sink
}

pub struct IntrinsicCtx {
    // current widget id, dpi, text and image measurement helpers
}

pub struct BaselineCtx {
    // current widget id, dpi
}
```

`MeasureCtx` should keep the current useful helpers:

- `measure_text`
- `shape_text`
- `image_size`
- `request_layout`
- `request_paint`
- `request_semantics`

`ArrangeCtx` should support child placement and arrange-only invalidation:

- `request_arrange`
- `request_paint`
- `request_semantics`
- `dpi`

The main rule is:

- expensive content sizing happens in `measure`
- child placement happens in `arrange`
- paint reads final arranged bounds only

## WidgetPod Changes

`WidgetPod` should stop using bounds as both measured output and arranged state. It should track a small generic layout state.

```rust
struct LayoutState {
    measured_size: Size,
    arranged_bounds: Rect,
    last_constraints: Constraints,
    measure_valid: bool,
    arrange_valid: bool,
}
```

Proposed `WidgetPod` entry points:

```rust
impl WidgetPod {
    pub fn measure(&mut self, parent_ctx: &mut MeasureCtx, constraints: Constraints) -> Size;

    pub fn arrange(&mut self, parent_ctx: &mut ArrangeCtx, bounds: Rect);

    pub fn measured_size(&self) -> Size;

    pub fn bounds(&self) -> Rect;
}
```

Behavior:

- `measure` calls the widget's `measure`, caches the result, and does not reposition descendants.
- `arrange` updates final bounds and calls the widget's `arrange` so the widget can place children.
- descendant translation becomes an arrange concern, not a side effect of measurement.

## Runtime Changes

### New layout pass

The runtime's current `run_layout_pass` should become two explicit phases.

```rust
fn run_layout_pass(&mut self, ...) -> Vec<InvalidationRequest> {
    let root_constraints = self.layout_constraints();

    let mut measure_ctx = MeasureCtx::new(...);
    let measured_root = self.root.measure(&mut measure_ctx, root_constraints);

    let viewport = root_constraints.clamp(measured_root);

    let mut arrange_ctx = ArrangeCtx::new(...);
    self.root.arrange(&mut arrange_ctx, Rect::from_origin_size(Point::ZERO, viewport));

    self.viewport = Some(viewport);
    self.refresh_graph();

    let mut invalidations = measure_ctx.take_invalidations();
    invalidations.extend(arrange_ctx.take_invalidations());
    invalidations
}
```

### Scheduling changes

`FrameSchedule` should split the current `layout` bit into `measure` and `arrange`.

```rust
pub struct FrameSchedule {
    pub measure: bool,
    pub arrange: bool,
    pub paint: bool,
    pub semantics: bool,
    pub hit_test: bool,
    pub text: bool,
    pub resources: bool,
}
```

Suggested invalidation behavior:

- `InvalidationKind::Layout` marks `measure`, `arrange`, `paint`, `semantics`, and `hit_test`
- new `InvalidationKind::Arrange` marks `arrange`, `paint`, `semantics`, and `hit_test`
- `InvalidationKind::Text` marks `measure`, `arrange`, `paint`, and `semantics`
- pure visual changes continue to mark only `paint`

This is the main runtime win. Scroll offsets, viewport shifts, and similar geometry-only moves can avoid full remeasurement when content size is unchanged.

### Graph changes

The retained widget graph should continue to store final arranged bounds, because hit testing, semantics, dirty layer generation, and paint all need arranged geometry.

What should change is the dirty comparison source:

- compare `arranged_bounds`, not transient measurement state
- optionally record `measured_size` and `baseline` in graph snapshots for diagnostics

`WidgetNodeSnapshot` can grow like this:

```rust
pub struct WidgetNodeSnapshot {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
    pub measured_size: Size,
    pub bounds: Rect,
    pub baseline: Option<f32>,
    pub accepts_focus: bool,
    pub focused: bool,
}
```

That gives debug tools a direct view of "desired size" versus "final rect".

## Container Helpers

The helper types in `SingleChild` and `WidgetChildren` should expose separate measure and arrange helpers.

```rust
impl SingleChild {
    pub fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size;

    pub fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect);
}

impl WidgetChildren {
    pub fn measure_child(
        &mut self,
        index: usize,
        ctx: &mut MeasureCtx,
        constraints: Constraints,
    ) -> Size;

    pub fn arrange_child(
        &mut self,
        index: usize,
        ctx: &mut ArrangeCtx,
        bounds: Rect,
    );
}
```

This should replace the current pattern where a parent measures a child and also mutates the child's bounds inside the same call.

## Example Migrations

### Label

Current behavior:

- text measurement happens in `layout`
- returned size is text measurement clamped to constraints

Proposed behavior:

```rust
impl WidgetLayout for Label {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let measurement = ctx.measure_text(self.text.clone(), self.style.clone())
            .unwrap_or(TextMeasurement {
                width: 0.0,
                height: self.style.line_height,
                bounds: Rect::new(0.0, 0.0, 0.0, self.style.line_height),
            });

        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measurement.width,
            measurement.height.max(self.style.line_height),
        ))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {}
}
```

Label does not need child arrangement, so its arrange phase is trivial.

### Padding

Proposed behavior:

```rust
impl WidgetLayout for Padding {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let child_constraints = inset_constraints(constraints, self.insets);
        let child_size = self.child.measure(ctx, child_constraints);
        constraints.clamp(expand_size(child_size, self.insets))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let child_bounds = Rect::from_origin_size(
            bounds.origin + self.insets.offset().to_vector(),
            self.child.measured_size(),
        );
        self.child.arrange(ctx, child_bounds);
    }
}
```

Padding becomes clearer because size computation and child placement are now separate.

### ScrollView

This is the most important migration target.

Proposed behavior:

- `measure` computes content size and viewport size
- `arrange` positions the child at `-offset`
- scrolling usually requests `Arrange`, `Paint`, and `Semantics`, not full `Layout`

That should reduce layout churn in scroll-heavy views and make retained layer invalidation easier to reason about.

## Intrinsic Sizing

Intrinsic queries should remain optional. Most widgets should not pay for them unless a parent asks.

Proposed rules:

- default `intrinsic_size` returns `None`
- text widgets, icons, images, and data cells can implement it directly
- flex/grid or table-like containers may query children for preferred sizes before final measurement

The runtime should not invoke intrinsic queries during ordinary layout unless a container explicitly chooses that path.

## Text Handling

The current text APIs are already good enough to support the new model.

Use the following split:

- `measure_text` for rough size contribution
- `shape_text` during `measure` when final line wrapping depends on current width constraints
- `paint` consumes shaped results already cached on the widget or in the text system

This keeps text shaping under layout control without requiring paint to perform layout work.

## Virtualization Hook

The first proposal should not try to solve full viewport virtualization for every container. It should, however, make it possible.

Add an optional specialized trait later:

```rust
pub trait ViewportLayout: WidgetLayout {
    fn measure_viewport(&mut self, ctx: &mut MeasureCtx, viewport: ViewportConstraints)
        -> ViewportMetrics;

    fn arrange_viewport(&mut self, ctx: &mut ArrangeCtx, viewport: Rect);
}
```

This should be phase two. The immediate goal is making `ScrollView` and future list/table widgets avoid unnecessary remeasurement.

## Diagnostics

The debug and profiling story should improve as part of this change.

Add the following per-frame diagnostics:

- measure pass duration
- arrange pass duration
- number of widgets remeasured
- number of widgets rearranged
- intrinsic query count
- cache hit and miss counts for layout state reuse

The widget graph and debug panels should expose:

- measured size
- arranged bounds
- baseline where available
- last constraints

## Migration Strategy

### Phase 1: Introduce types and compatibility bridge

- add `WidgetLayout`, `MeasureCtx`, `ArrangeCtx`, and `LayoutState`
- keep the existing `Widget::layout` temporarily as a compatibility layer
- provide a blanket adapter for legacy widgets

Compatibility bridge sketch:

```rust
pub trait LegacyLayout {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size;
}

impl<T: LegacyLayout> WidgetLayout for T {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.layout(&mut ctx.as_legacy_layout_ctx(), constraints)
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {}
}
```

This keeps the tree running while core containers migrate.

### Phase 2: Migrate framework containers

Convert first:

- `Padding`
- `Align`
- `SizedBox`
- `Stack`
- `ScrollView`

These give the runtime enough real coverage to validate the phase split.

### Phase 3: Migrate text-heavy controls

Convert:

- `Label`
- `Button`
- text inputs
- data widgets that currently perform manual text measurement

This is where intrinsic sizing and arrange-only updates start paying off.

### Phase 4: Remove legacy layout path

- delete `Widget::layout`
- remove legacy compatibility contexts
- update docs and profiling tools to reflect the new pipeline

## Non-Goals

This proposal does not aim to:

- adopt HTML or CSS layout semantics wholesale
- make every widget use intrinsic measurement
- force a solver-based layout system into common controls
- solve virtualization in the first migration step

## Recommendation

Implement this in runtime first, with only the minimum new core types added to `sui-layout`.

Suggested crate ownership:

- `sui-layout`: `Constraints`, intrinsic size enums, viewport helper structs, generic algorithms
- `sui-runtime`: `WidgetLayout`, `MeasureCtx`, `ArrangeCtx`, `LayoutState`, runtime scheduling
- `sui-widgets`: migrated containers and controls

That keeps the architecture aligned with the existing crate split while giving SUI a path from the current one-pass layout model to a more capable retained layout system.
