# Node graph editor

`sinomo-ui-nodes` is SUI's retained node-graph editor library. It provides an
observable graph document, an interactive `NodeGraph` widget, viewport
controls, and an optional minimap. Applications can use the default visuals or
paint node and edge bodies from their own typed data.

Add the separate package alongside the SUI facade:

```toml
[dependencies]
sui = { package = "sinomo-ui", version = "0.2" }
sui-nodes = { package = "sinomo-ui-nodes", version = "0.2" }
```

## Build a graph

Nodes and edges carry generic application data. Their identifiers and handle
identifiers are strings wrapped in dedicated types, preventing accidental
node/edge ID mixing.

```rust,no_run
use sui::prelude::*;
use sui_nodes::prelude::*;

let nodes = vec![
    Node::new("load", Point::new(40.0, 80.0), ())
        .label("Load image"),
    Node::new("preview", Point::new(340.0, 180.0), ())
        .label("Preview"),
];
let edges = vec![
    Edge::new("load-preview", "load", "preview", ())
        .handles("source", "target"),
];
let state = NodeGraphState::new(nodes, edges)?;

let graph = NodeGraph::new("Pipeline", state.clone())
    .config(NodeGraphConfig {
        fit_view_on_init: true,
        background_variant: BackgroundVariant::Dots,
        snap_to_grid: Some(Size::new(16.0, 16.0)),
        ..NodeGraphConfig::default()
    })
    .on_change(|change| println!("{change:?}"));

# let _ = graph;
# Ok::<(), sui_nodes::GraphError>(())
```

`NodeGraphState` is cloneable shared state backed by a SUI observable. A graph,
`NodeControls`, and `NodeMiniMap` built from clones of the same handle remain in
sync without rebuilding their widget instances:

```rust,no_run
# use sui_nodes::prelude::*;
# let state = NodeGraphState::<(), ()>::default();
let controls = NodeControls::new("Graph controls", state.clone());
let minimap = NodeMiniMap::new("Graph overview", state.clone())
    .pannable(true)
    .zoomable(true);
# let _ = (controls, minimap);
```

Place the companions beside the graph or in an application-owned overlay/dock
surface. Their appearance types are intentionally graph-specific rather than
adding specialized roles to SUI's global theme.

## Retained custom node widgets

Set `Node::kind` and register the corresponding factory with
`NodeGraph::node_type`. The factory receives a stable `NodeId` and a
`NodeSignal<N>`. SUI retains the resulting `WidgetPod` while the node remains
present, so editable fields, menus, focus, semantics, and other widget-local
state survive graph updates. Pointer events handled by a child do not start a
node drag; unhandled node-body events bubble back to the graph.

Node customization stays in the normal SUI widget system. A retained node can
compose `Image`, `Label`, vector/icon widgets, buttons, inputs, layouts, or an
application-defined widget exactly as it would outside a graph.

Nodes are fixed-size by default. `Node::content_sized(min, max)` measures the
retained widget with those constraints and publishes the resolved size back to
the graph state. `Node::resizable(true)` instead exposes eight selection resize
handles with optional size limits.

## Canvas-compatible surface and optional events

The graph background and world transform use SUI's paint-only `CanvasSurface`.
`Viewport::to_canvas` and `Viewport::from_canvas` preserve coordinates between
the xyflow-style graph transform and an unrotated `CanvasViewport`. Rotation is
intentionally rejected by the graph model, while Canvas itself supports affine
widget transforms.

Retained node widgets use true Canvas-style uniform scaling by default. Their
layout is measured once in stable flow units; zoom transforms the complete
subtree, including text, images, icons, controls, hit testing, and semantics.
`NodeGraph::node_zoom_behavior` and `NodeWidgetRegistry::with_zoom_behavior`
allow a node kind to remain screen-space or supply a custom Canvas transform.

`NodeGraph` combines this surface with SUI's built-in graph interaction model.
Use `NodeGraphSurface` when rendering should be independent of those events.
It paints the same background, edges, overlays, semantics, and retained node
widgets, but does not interpret pointer, keyboard, or semantic actions. Child
widgets continue handling their own events normally.

Applications with a different event model can wrap `NodeGraphSurface`, call
`node_graph_hit_test` (or `NodeGraphSurface::hit_test`), and mutate the shared
`NodeGraphState` through its public operations. `into_surface` and
`into_interactive` convert between the convenience editor and paint-only forms.

## Controlled state and snapshots

`NodeGraphState::new` creates an uncontrolled state: interactions commit
directly. `NodeGraphState::controlled` creates a controlled state: interactions
send a complete proposed `GraphSnapshot` to the installed change handler, and
the owner accepts or replaces it with `replace_snapshot`.

The state API includes bulk node/edge replacement and addition, typed data
updates, deletion, incoming/outgoing and handle-connection queries,
intersection queries, subset bounds, coordinate conversion, center/zoom/fit
operations, and animated viewport transitions. `GraphDocument` is the
persistence-facing form without runtime caches or in-flight transitions.

Snapshots use a copy-on-write `Arc<GraphModel>` with separate node, edge,
viewport, and document revisions. A uniform-grid `GraphSpatialIndex` updates
only changed nodes and their affected edges. Painting and pointer hit testing
query this index instead of scanning the full document.

Large imports can build the grid incrementally on the UI thread. Creation
resolves graph geometry once, and each `advance` call limits grid insertion to
the supplied item budget:

```rust,no_run
# use sui_nodes::prelude::*;
# let graph = GraphModel::<(), ()>::empty();
let mut builder = GraphSpatialIndex::builder(&graph, 0);
while !builder.advance(2_048).is_complete() {
    // Yield to the host event loop and continue on a later turn.
}
let snapshot = GraphSnapshot::with_spatial_index(graph, builder.finish());
let state = NodeGraphState::from_snapshot(snapshot);
# let _ = state;
```

Viewport culling is enabled by default. `NodeGraphConfig::culling_margin`
retains a screen-space overscan around the viewport; offscreen retained widgets
skip arrangement, painting, and semantics until they enter that range. Set
`cull_offscreen` to `false` only when an application deliberately needs every
element mounted into those phases.

## Hierarchy and editing

`parent_id` makes a node position relative to a parent. Parent chains are
validated against missing parents and cycles. `NodeExtent::Parent` constrains
child movement to the parent, while `expand_parent(true)` grows and repositions
the parent as needed. Moving a parent automatically updates descendant absolute
bounds in the spatial index; deleting it cascades through descendants.
Negative `Node::z_index` values place container nodes below the edge layer;
non-negative nodes remain above edges. Node bodies and retained child widgets
are composited together in z/depth order, so selecting a parent does not paint
it over its descendants.

Selected resizable nodes expose constrained resize handles. Selected edges can
reconnect either endpoint according to `EdgeReconnectMode`. Every node and edge
also receives a stable generated semantic identity with focus, activate, and
delete actions, and keyboard Tab/Shift+Tab cycles graph elements.

## Interaction model

The default editor supports:

- pointer-wheel zoom anchored beneath the pointer, plus two-touch pinch zoom
  anchored beneath the gesture centroid;
- primary-button pane panning and middle/secondary-button panning;
- node selection and dragging, including optional grid snapping;
- Control/Command click multi-selection;
- Shift-drag marquee selection, with full or partial containment modes;
- source-to-target handle dragging with connection validation;
- Backspace/Delete removal, Control/Command+A selection, arrow-key nudging,
  plus/minus zoom, Escape clearing, and Home fit-to-view;
- straight, step, and cubic Bézier edges with optional labels and end markers;
- smooth-step and simple-Bézier paths, configurable curvature and corner
  radius, start/end marker shapes, animated edge particles, explicit z-order,
  and draggable endpoint reconnection;
- pan-on-scroll, double-click zoom, edge-proximity auto-pan, and animated
  viewport transitions;
- shared fit-view, zoom, and interaction-lock controls;
- a viewport-aware, optionally pannable and zoomable minimap.

Every graph mutation is applied to `NodeGraphState`. The widget's `on_change`
callback additionally emits `NodeChange`, `EdgeChange`, connection, selection,
viewport, hover, click, drag, resize, and reconnect lifecycle events for
application controllers that need an editing journal or persistence boundary.

## Validation and customization

`GraphModel::new`, `add_node`, and `add_edge` reject duplicate identifiers,
dangling endpoints, missing typed handles, and disabled handles. Removing a
node cascades its incident edges.

Use `NodeGraph::is_valid_connection` for application rules such as preventing
self-links or cycles. Use `edge_factory` to construct application-specific edge
data when a user completes a connection.

`NodeGraphAppearance`, `NodeControlsAppearance`, and `NodeMiniMapAppearance`
own optional color overrides. Unset fields resolve from semantic `DefaultTheme`
roles on every paint. For data-dependent visuals, `node_painter` and
`edge_painter` receive the typed model value plus screen-space geometry while
the editor retains hit testing and interaction behavior.

## Coordinate conversion and imperative state

`Viewport` uses the same three-part representation common to flow editors:
`x`, `y`, and `zoom`. It provides flow-to-screen and screen-to-flow conversion,
pointer-anchored zoom, visible-flow bounds, centering, and fit-view calculation.

The shared state exposes common imperative operations such as `set_viewport`,
`fit_view`, `zoom_by`, `set_node_position`, `add_node`, `add_edge`, and
interaction locking. State writes are equality-deduplicated and notify every
mounted SUI consumer.

## Comprehensive demo

The default development workspace includes a comprehensive example:

```bash
cargo run -p sinomo-ui-demo
```

Open `Node graphs`. The primary graph is controlled and automatically accepts
proposed snapshots; the sidebar exposes document/index revisions and lifecycle
events. It also includes a pannable minimap and a second uncontrolled graph
using a canvas-backed paint-only surface with retained node widgets. The demo
keeps `nodes` as a distinct feature even though it is enabled by default, so
specialized builds can disable the dependency.

## Performance diagnostics

The ignored benchmarks are optimized diagnostic workloads, not
publication-comparable results. Run them serially on an otherwise idle system:

```bash
cargo test -p sinomo-ui-runtime transformed_widget_subtree_current_status_benchmark -- --ignored --nocapture
cargo test -p sinomo-ui-nodes node_graph_current_status_benchmark -- --ignored --nocapture
cargo test -p sinomo-ui-demo retained_node_graph_gpu_zoom_current_status_benchmark -- --ignored --nocapture
```

They cover 384 flat, independently transformed, and shared-transform widgets;
a 10,000-node/19,800-edge model with budgeted spatial-index construction;
384-node retained and painted zoom frames; and the complete runtime plus WGPU
renderer path. Each zoom workload asserts that viewport-only changes do not
remeasure retained widgets.
