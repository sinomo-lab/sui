# Stack Hosts, Floating Order, and Multi-Bounds

## Companion Documents

- [Documentation Index](./README.md)
- [Architecture Overview](./architecture.md)
- [Crate Guide](./crate-architecture.md)
- [Rendering Architecture](./renderer-architecture.md)
- [SUI Design](./design.md)

## Status

This document is a design note for the next iteration of runtime composition. It describes the intended direction for floating windows, popup containment, editor-style layers, and widget geometry contracts. It is not a description of the current implementation.

## Problem

The current runtime uses a small set of composition modes such as `Normal`, `Overlay`, and `Effect` to influence both rendering order and hit testing. That model is sufficient for simple popovers, modal overlays, and direct layers, but it does not scale well to the following cases:

- floating windows that must move to the front when focused without scrambling the order of other windows
- editor-style layer stacks that need explicit stable ordering independent of focus
- popups and tooltips that should stay inside a containing surface rather than always behaving like root-window overlays
- widgets whose layout size, interactive size, and painted size must differ

The framework needs a more explicit model for stacking and a more explicit model for widget geometry.

## Decisions

The design in this note assumes the following decisions.

1. Floating windows remain normal widgets. They do not become a separate platform or runtime surface type.
2. Widgets may opt into becoming a stack host. A stack host controls the ordering, hit testing, and popup placement of descendant stack surfaces.
3. Popups use the nearest containing stack host by default. A tooltip, menu, dropdown, or popover opened inside a floating window stays within that floating window's host unless a widget explicitly asks for different behavior.
4. The runtime tracks three bounds per widget: layout bounds, input bounds, and paint bounds.
5. Ordering becomes a first-class runtime concern rather than an implicit side effect of `Overlay` or `Effect` composition modes.

## Goals

- Support floating desktop-style panels and windows with stable bring-to-front behavior.
- Support explicit layer stacks for graphics-editor and content-creation workflows.
- Allow parent widgets to control popup containment, clipping, and ordering.
- Allow widgets to paint or receive input outside their layout footprint without abusing global overlays.
- Keep the model compatible with retained widgets, explicit invalidation, semantics generation, and the retained compositor.

## Non-Goals

- This design does not introduce platform-native child windows.
- This design does not try to replace the existing scene graph with a separate surface tree.
- This design does not solve docking, tabbed hosts, or window persistence policy by itself.
- This design does not define application-level shortcuts for cycling or managing floating windows.

## Core Terms

### Stack host

A stack host is a widget subtree that owns an ordered set of descendant stack surfaces. The host is responsible for:

- maintaining local z-order
- resolving pointer hit testing among stacked descendants
- determining popup placement and containment for descendant widgets
- exposing clipping and paint constraints that apply to hosted surfaces

The root window is the default stack host. Widgets such as floating-window containers, canvas overlays, or editor layer panels may create nested hosts.

### Stack surface

A stack surface is a widget subtree that participates in explicit ordering inside a host. Floating windows, popups, menus, tooltips, and editor layers are all stack surfaces. A stack surface remains a normal widget subtree with a normal `WidgetId` and retained child ownership.

Each stack surface belongs to exactly one nearest resolved stack host.

### Owner surface

When a popup is opened by a widget inside a stack surface, the runtime records the owning surface. Ownership is used to keep transient content ordered relative to its source and to clean it up when the owner closes or disappears.

## Geometry Model

Each widget exposes a geometry snapshot with three rectangles.

### Layout bounds

Layout bounds are the widget's measured and arranged rectangle. They are the bounds used for measure and arrange, and they define how the widget participates in parent layout.

### Input bounds

Input bounds define the area where the widget can intercept pointer interaction. Input bounds may be larger than layout bounds for controls such as:

- thin splitters with larger drag handles
- invisible edge and corner resize zones
- small visible affordances that need touch-friendly interaction

Input bounds may also be smaller than layout bounds when a widget intentionally wants a narrower active region.

### Paint bounds

Paint bounds define the rectangle that the runtime and renderer treat as the widget's visible output area. Paint bounds may extend beyond layout bounds for controls such as:

- tooltips and menus
- shadows, glow, or other visual effects
- handles or previews that paint outside their measured footprint

Paint bounds are used for culling, layer visibility, damage tracking, and retained-compositor layer descriptors.

## Host and Surface Model

### Why hosts exist

Ordering is not global policy. A floating-window workspace, a root desktop surface, and a graphics layer panel each want different rules. The host abstraction keeps those rules local.

### Host responsibilities

Each stack host maintains:

- a stable ordered list of child stack surfaces
- surface metadata such as owner, transient status, and clipping policy
- a hit-test view ordered from front to back
- a popup resolution policy for descendants

### Surface responsibilities

Each stack surface exposes:

- its host membership
- its geometry snapshot
- whether it participates in dynamic ordering
- whether it is transient relative to another surface

### Nested hosts

A nested host creates a local stacking world inside its subtree. Descendant popups resolve to the nearest host by default. This keeps standalone surfaces such as floating windows self-contained.

## Ordering Rules

### Focus-driven floating order

Floating windows use stable local ordering inside a host.

- Focusing a window brings that window to the front of the host.
- The relative order of all other hosted floating windows is preserved.
- Clicking a front-most window does not change the order unless policy says it should.
- Keyboard-driven focus changes may also bring a window forward when the focused widget belongs to that window surface.

This behaves like taking one item out of an ordered list and reinserting it at the front.

### Editor layers

Editor layers use the same host and surface infrastructure but a different policy.

- Layer order is explicit application state.
- Focus does not reorder layers unless the application requests it.
- Reorder operations are direct host mutations rather than focus side effects.

The host model is shared; only the ordering policy differs.

### Popup ordering

Popups are inserted into the same stack host as their owner. The host applies the following default rules.

- A popup is always ordered above its owning surface.
- A popup is ordered above older transient popups owned by the same surface unless policy says otherwise.
- Reordering a floating window also carries its owned transient surfaces with it.
- Dismissing an owner surface dismisses its owned transient surfaces.

This keeps menus and tooltips visually attached to their source without requiring a separate root overlay path.

## Popup Containment and Clipping

Each stack host defines popup containment behavior for its descendants.

The default host behavior is:

- resolve popups to the nearest host
- clip popup paint to the host's configured clip region when clipping is enabled
- keep popup hit testing inside the same host ordering model

This supports both of the important cases:

- a floating window can keep its menus and tooltips inside its own surface bounds for a standalone feel
- a permissive container can allow descendants to paint and receive input beyond local layout bounds by expanding or merging host-visible paint and input regions

## Runtime Changes

### Widget geometry snapshot

The runtime should stop treating a single arranged rectangle as the full widget geometry contract. Instead it should store a geometry snapshot per widget, roughly consisting of:

- layout bounds
- input bounds
- paint bounds

Default behavior remains simple:

- layout bounds default to the arranged bounds
- input bounds default to layout bounds
- paint bounds default to scene paint bounds or layout bounds when the scene has no wider paint output

This keeps existing widgets working while allowing opt-in overrides.

### Widget graph

The retained widget graph should use input bounds for hit testing rather than the single stored bounds rectangle. Layout bounds remain available for diagnostics and layout-driven invalidation.

### Focus integration

Focus requests remain widget-driven, but focus changes may now trigger host-local reorder operations when the focused widget belongs to a focus-fronted surface. Reordering should be explicit runtime work rather than an incidental consequence of paint order.

### Invalidation

The runtime should add an explicit ordering invalidation or update path.

Reordering should not be forced through generic paint invalidation when the surface content itself is unchanged. The compositor and scene snapshot need enough information to distinguish:

- content changes
- transform changes
- clip changes
- effect changes
- visibility changes
- ordering changes

## Scene and Compositor Changes

### Scene descriptors

Scene layer descriptors already carry layout-facing bounds, content bounds, and paint bounds. The next step is to make scene snapshots capable of expressing host-local ordering explicitly instead of inferring all floating behavior from composition mode.

The scene and compositor should learn enough metadata to answer:

- which host a layer belongs to
- the layer's local order inside that host
- whether a layer is transient and who owns it

### Composition modes

Composition modes should remain compositor hints, not the sole source of z-order semantics. Existing modes such as `Scroll`, `Overlay`, and `Effect` can continue to express retained-compositor behavior, but stack ordering should not depend on them.

## Semantics and Accessibility

Semantics should continue to be generated from the retained widget tree. The important changes are:

- popup semantics remain inside the owning host subtree by default
- geometry used for semantic nodes should no longer assume a single widget bounds rect is always correct
- floating-window focus changes must keep semantic focus and visual fronting in sync

This design does not require a separate semantics tree for stacked surfaces.

## Testing and Diagnostics

The debug and testing surfaces should expose the new model directly.

Diagnostics should eventually show:

- stack host membership
- local surface order
- layout, input, and paint bounds per widget
- popup ownership chains

The test suite should add coverage for:

- focus-driven bring-to-front preserving the order of other windows
- explicit editor-layer reorder operations
- oversized input bounds for splitters and resize handles
- popups clipped to a floating host
- popups dismissed with their owner
- ordering-only updates that do not require content repaint

## Staged Rollout

### Stage 1: geometry separation

- Introduce widget geometry snapshots.
- Keep existing behavior by defaulting input and paint bounds from layout and scene bounds.
- Move graph hit testing to input bounds.

### Stage 2: stack hosts and surfaces

- Add host membership and host-local order tracking.
- Make the root window the default host.
- Add a focus-fronted floating-window container as the first non-root host user.

### Stage 3: popup host resolution

- Convert tooltip, popover, dropdown, menu, and dialog-style widgets to resolve through the nearest stack host.
- Record popup ownership and transient ordering.

### Stage 4: compositor and diagnostics

- Add explicit ordering updates to scene snapshots and retained-compositor updates.
- Surface host/order/bounds data in debugging and testing tools.

## Initial API Direction

The exact public API can evolve, but the intended shape is:

- widgets can opt into becoming a stack host
- widgets can opt into becoming a stack surface
- widgets can override input bounds and paint bounds without replacing the layout contract
- popup-producing widgets resolve their surface placement through the nearest host rather than hard-coding root overlay behavior

The key constraint is that these remain widget-level capabilities layered on the existing retained runtime rather than a separate special-case subsystem.

## Summary

The design direction is:

- floating windows are ordinary widgets hosted in an explicit stack host
- popups use the same nearest host by default rather than escaping to the root window
- layout, input, and paint geometry are tracked separately
- ordering becomes explicit runtime and compositor metadata rather than being inferred entirely from `Overlay` and `Effect`

This is the minimum model that supports floating windows, editor layers, and host-controlled popup behavior without breaking the retained-widget architecture.
