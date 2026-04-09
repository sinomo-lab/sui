# SUI Stack Hosts, Popup Resolution, and Ordering

## Companion Documents

- [Documentation Index](./README.md)
- [Architecture Overview](./architecture.md)
- [Crate Guide](./crate-architecture.md)
- [Rendering Architecture](./renderer-architecture.md)
- [SUI Design](./design.md)

## Current Contract

The current stack model is an explicit runtime contract, not a design-only direction.

The implemented behavior is:

- stack hosts are widget subtrees that own local surface ordering
- stack surfaces are widget subtrees that participate in host-local z order
- popup-like surfaces resolve to the nearest host when active
- transient popups track an owning surface
- runtime graph hit testing uses input bounds, not only layout bounds
- scene layer descriptors carry host and ordering metadata
- ordering-only updates can be emitted without forcing content repaint

This keeps floating and popup behavior in the retained widget/runtime model rather than introducing platform child windows.

## Core Model

### Stack host

A stack host is any widget that exposes host options through the runtime widget contract.

Host policy is currently:

- Stable: preserve declared surface order
- FocusFronted: allow focused surface fronting behavior

The root widget is always treated as a host. Non-root hosts can be introduced by widgets, for example FloatingStack.

### Stack surface

A stack surface is any widget that is either:

- explicitly marked as a surface by stack surface options, or
- a direct child surface in a host-local stacking world

Each surface resolves to one nearest host and one resolved surface identity in the runtime graph.

### Transient popup ownership

Active popup-like widgets mark themselves as transient stack surfaces. The runtime records the owning surface for transient surfaces so ordering and diagnostics preserve popup-to-owner relationship.

## Geometry Contract

Each widget node exposes three bounds in runtime graph snapshots:

- layout bounds
- input bounds
- paint bounds

Current defaults are:

- input bounds defaults to layout bounds
- paint bounds defaults to scene paint bounds when available, otherwise layout bounds

Hit testing uses input bounds, while paint invalidation and damage defaulting use paint bounds.

## Runtime Implementation

Primary implementation is in sui-runtime.

### Widget-side stack hooks

Widget now supports:

- stack_host_options
- stack_surface_options

Host and surface options are evaluated during graph rebuild and propagated into WidgetGraphSnapshot.

### Graph snapshots

WidgetNodeSnapshot now includes:

- stack_host
- stack_surface
- stack_surface_order
- transient_owner_surface
- is_stack_host
- is_stack_surface
- stack_order_policy

WidgetGraphSnapshot also includes stack_hosts, which provides host-level surface lists and policy.

### Host-local hit testing

For host nodes, hit testing is ordered by host-local surface order before non-surface child traversal. This allows host-managed z order to drive pointer targeting directly.

### Ordering invalidation path

InvalidationKind includes Ordering and frame scheduling tracks ordering work separately from content paint work.

This allows widgets to request ordering changes through request_ordering when only z order changes.

## Scene and Compositor Integration

Primary implementation spans sui-scene, sui-runtime, and sui-render-wgpu.

### Scene layer metadata

SceneLayerDescriptor and SceneLayerUpdate include stack metadata:

- stack_host
- stack_order
- transient_owner_surface
- is_stack_surface

### Ordering updates

SceneLayerUpdateKind includes Ordering.

Runtime collects ordering updates when descriptor stack metadata changes, and layer update priority now recognizes Ordering as a first-class update category.

### Scene ordering sync

Before frame finalization, runtime syncs stack metadata from the graph into scene layer descriptors and reorders stack surfaces in scene command order with scene.reorder_stack_surfaces.

This keeps scene ordering aligned with host-local runtime ordering.

### Retained compositor behavior

Retained compositor logic treats Ordering updates as non-content updates, so z-order changes do not force content packet regeneration by default.

## Popup Host Resolution

Current popup-like widgets in sui-widgets opt into transient surface behavior when active:

- Tooltip when hovered
- Popover when open
- ContextMenu when open
- Dialog when shown
- Select and ComboBox when expanded

These widgets still use composition mode hints, but their host membership and ownership are now explicit runtime and scene metadata.

## Diagnostics Surfaces

### Runtime diagnostics

Scene statistics include layer update breakdowns, including Ordering updates.

### Debug widgets

sui-debug surfaces stack metadata directly:

- widget graph rows show host, surface, owner, order, and geometry bounds
- window snapshot includes a stack-host summary panel
- scene summary includes layer update breakdown including Ordering

This makes ordering-only frames and popup ownership chains observable without reading renderer internals.

## First Non-root Host User

FloatingStack in sui-widgets is the first non-root host user with FocusFronted policy.

Its bring-to-front path requests ordering invalidation rather than repaint invalidation, and host-local surface order is reflected through runtime graph and scene updates.

## Testing Guidance

When touching stack host behavior, validate at three levels:

1. Runtime graph snapshots
- host membership
- surface order
- transient ownership

2. Scene and update stream
- descriptor stack metadata
- layer update kinds include Ordering when appropriate

3. Widget behavior
- focus-front and pointer-fronting in host widgets
- popup ownership and nearest-host membership

Recommended targeted runs:

1. cargo test -p sui-runtime
2. cargo test -p sui-scene
3. cargo test -p sui-render-wgpu
4. cargo test -p sui-widgets with relevant popup and floating host tests

## Current Constraints

- Stack host clipping policy is still conservative and widget-specific; there is no full generalized clip policy API yet.
- Composition modes remain renderer hints and are still used by some widget behavior, but ordering semantics should come from stack metadata.
- Popup lifecycle policy is currently local to each popup-style widget; ownership metadata is available for tighter centralized policies in future changes.

## Where To Work

- stack graph membership and ordering: sui-runtime
- scene descriptor and update metadata: sui-scene and sui-runtime
- retained compositor update handling: sui-render-wgpu
- popup and floating widgets: sui-widgets
- debug panels and inspection widgets: sui-debug

This document should be treated as implementation guidance for current code paths, not as a speculative design note.
