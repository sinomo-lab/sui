# Custom Widgets

[Previous: themes and resources](themes-and-resources.md) · [API guide](README.md) ·
[Next: testing and accessibility](testing-and-accessibility.md)

Compose built-in widgets first. Implement `Widget` when a component needs
custom drawing, input behavior, layout, semantics, retained layers, or direct
cooperation with the scene and text systems.

## The Widget Lifecycle

`Widget` has safe defaults, so a leaf implements only the phases it needs:

| Method | Responsibility |
| --- | --- |
| `event(&mut self, EventCtx, &Event)` | Mutate interaction state and request invalidation |
| `measure(&mut self, MeasureCtx, Constraints) -> Size` | Choose a size within parent constraints and measure children |
| `arrange(&mut self, ArrangeCtx, Rect)` | Place retained children in final bounds |
| `paint(&self, PaintCtx)` | Emit renderer-neutral scene commands |
| `semantics(&self, SemanticsCtx)` | Emit accessible roles, names, values, state, and actions |
| `accepts_focus()` | Opt into keyboard focus |
| `focus_changed(...)` | Invalidate presentation or semantics after focus changes |
| `visit_children(_mut)` | Expose retained child pods to runtime traversal |

The runtime owns phase ordering and caching. Widget methods must stay
synchronous and should not call platform or `wgpu` APIs directly.

## Tutorial: an Accessible Toggle Pill

This complete leaf widget handles mouse/touch activation, pointer capture,
keyboard activation, assistive-technology activation, focus, paint, and
semantics.

```rust
use sui::prelude::*;
use sui::{
    KeyState, PointerButton, PointerEventKind, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, ToggleState,
};

struct TogglePill {
    name: String,
    on: bool,
    pressed_pointer: Option<u64>,
}

impl TogglePill {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            on: false,
            pressed_pointer: None,
        }
    }

    fn toggle(&mut self, ctx: &mut EventCtx) {
        self.on = !self.on;
        ctx.request_paint();
        ctx.request_semantics();
    }
}

impl Widget for TogglePill {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.pressed_pointer = Some(pointer.pointer_id);
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && self.pressed_pointer == Some(pointer.pointer_id) =>
            {
                self.pressed_pointer = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                if ctx.bounds().contains(pointer.position) {
                    self.toggle(ctx);
                } else {
                    ctx.request_paint();
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.pressed_pointer == Some(pointer.pointer_id) =>
            {
                self.pressed_pointer = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.toggle(ctx);
                ctx.set_handled();
            }
            Event::Semantics(action)
                if matches!(&action.action, SemanticsActionRequest::Activate) =>
            {
                self.toggle(ctx);
                ctx.set_handled();
            }
            Event::Semantics(action)
                if matches!(&action.action, SemanticsActionRequest::Focus) =>
            {
                ctx.request_focus();
                ctx.set_handled();
            }
            Event::Semantics(action)
                if matches!(&action.action, SemanticsActionRequest::Blur) =>
            {
                ctx.clear_focus();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(152.0, 36.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let fill = if self.on {
            Color::rgba(0.18, 0.55, 0.34, 1.0)
        } else {
            Color::rgba(0.32, 0.34, 0.38, 1.0)
        };
        ctx.fill_rrect(ctx.bounds(), [18.0; 4], fill);
        if ctx.is_focused() {
            ctx.stroke_rect(
                ctx.bounds().inflate(2.0, 2.0),
                Color::rgba(0.35, 0.75, 1.0, 1.0),
                StrokeStyle::new(2.0),
            );
        }
        ctx.label(ctx.bounds(), self.name.clone(), Color::WHITE);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Switch,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.state.checked = Some(if self.on {
            ToggleState::Checked
        } else {
            ToggleState::Unchecked
        });
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}
```

Production controls may add hover tracking, animation, disabled state, and a
callback. The important contract is already present: focus has a visible
treatment, while pointer, keyboard, and semantic activation change the same
state and emit the same invalidations.

## Measurement and Arrangement

Always return a size accepted by the parent constraints:

```rust,ignore
fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
    constraints.clamp(self.preferred_size)
}
```

A parent measures each child before arranging it. A custom container should:

1. Store children as `SingleChild`, `WidgetChildren`, or explicit `WidgetPod`
   fields.
2. Call each pod's measure helper with derived child constraints.
3. Return its own clamped aggregate size.
4. Arrange every measured child inside the final bounds.

Do not paint a child at a location that differs from its arranged bounds. Use
retained transforms for presentation-only movement when that distinction is
intentional.

`MeasureCtx::layout()` exposes text, DPI, font, and image measurement services.
Cache shaped or persistent text layout during measurement when paint will
reuse it; avoid reshaping unchanged text every frame.

## Owning Children Correctly

`WidgetChildren` provides indexed measurement and arrangement plus bulk paint
and semantics delegation. A custom container must also expose its children:

```rust,ignore
fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
    self.children.visit_children(visitor);
}

fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
    self.children.visit_children_mut(visitor);
}
```

Traversal is how the runtime finds stable identities for event routing, focus,
invalidation, retained caches, and semantics. Painting a child but omitting it
from traversal creates a visually present subtree the runtime cannot fully
address.

Use `SingleChild` for a wrapper and `WidgetChildren` for a normal fixed
collection. Use `KeyedChildren<K, T>` when collection membership or ordering
changes and existing child `WidgetPod`s must retain focus, animation,
selection, or editor state across reconciliation.
Virtualized widgets may expose only the mounted subset, but their generated
semantics still need stable IDs and correct parent relationships.

## Painting Contract

`PaintCtx` emits a renderer-neutral `Scene`. Frequently used operations are:

- `fill`, `fill_rect`, `fill_rrect`, and bordered/shadow variants.
- `stroke` and `stroke_rect` with `StrokeStyle`.
- `draw_text_layout` or persistent-layout variants.
- `draw_image`, `draw_image_source`, and image-quad variants.
- balanced clip and transform pushes/pops.
- `draw_shader_rect` for scene-level shader content.

Use local widget bounds and logical pixels. The platform and renderer own DPI,
surface formats, color management, and presentation. Keep clip, transform, and
text-policy stacks balanced within the widget's paint call.

`paint(&self, ...)` is intentionally non-mutating. Prepare caches during
measurement or use purpose-built interior caches only when their mutation is
an implementation detail that cannot change observable widget state.

## Semantics Are Part of the Widget

Every interactive custom widget should publish:

- The closest `SemanticsRole`.
- A durable accessible name and optional description.
- Current value, checked/selected/expanded/busy state where applicable.
- Only the actions it actually implements.
- Bounds matching the actionable visual target.

Handling pointer clicks without `SemanticsActionRequest::Activate`, or
advertising `Activate` without handling it, creates an inaccessible control.
Similarly, `accepts_focus()` should agree with the advertised focus action and
the visible focus treatment.

## Invalidation and Retained Layers

Request the narrowest work that makes a mutation observable. See the
[invalidation table](state-events-and-async.md#request-the-narrowest-correct-invalidation).

Advanced widgets can override `layer_options`, `layer_properties`, and stack
host/surface options. Use these only for explicit retained paint boundaries,
compositor transforms/effects, or overlay ordering. Ordinary widgets should
keep the defaults and emit normal scene commands.
