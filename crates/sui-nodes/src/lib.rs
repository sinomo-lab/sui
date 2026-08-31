#![forbid(unsafe_code)]

//! An interactive, retained node-graph editor for SUI.
//!
//! `sui-nodes` separates the graph document, observable editor state,
//! Canvas-compatible surface, retained widget presentation, and optional
//! built-in events. Use [`NodeGraph`] as a ready-to-use editor or
//! [`NodeGraphSurface`] with an application-owned event model.

mod controls;
mod minimap;
mod model;
mod node_widget;
mod spatial;
mod state;
mod viewport;
mod widget;

pub use controls::{NodeControls, NodeControlsAppearance};
pub use minimap::{NodeMiniMap, NodeMiniMapAppearance};
pub use model::{
    ConnectError, Connection, Edge, EdgeId, EdgeKind, EdgeMarker, EdgePathOptions,
    EdgeReconnectMode, GraphError, GraphModel, Handle, HandleId, HandleKind, HandlePosition, Node,
    NodeExtent, NodeId, NodeSizeMode, RemovedNode,
};
pub use node_widget::{NodeSignal, NodeWidgetRegistry};
pub use spatial::{GraphSpatialIndex, GraphSpatialIndexBuildProgress, GraphSpatialIndexBuilder};
pub use state::{
    DeletedElements, GraphDocument, GraphSnapshot, NodeGraphMode, NodeGraphState,
    SnapshotRevisions, ViewportTransition,
};
pub use viewport::{FitViewOptions, Viewport};
pub use widget::{
    BackgroundVariant, EdgeChange, EdgePaintContext, NodeChange, NodeGraph, NodeGraphAppearance,
    NodeGraphConfig, NodeGraphEvent, NodeGraphHit, NodeGraphSurface, NodePaintContext,
    ResizeDirection, SelectionMode, node_graph_hit_test,
};

pub mod prelude {
    pub use crate::{
        BackgroundVariant, Connection, DeletedElements, Edge, EdgeChange, EdgeId, EdgeKind,
        EdgeMarker, EdgePaintContext, EdgePathOptions, EdgeReconnectMode, FitViewOptions,
        GraphDocument, GraphModel, GraphSnapshot, GraphSpatialIndex,
        GraphSpatialIndexBuildProgress, GraphSpatialIndexBuilder, Handle, HandleId, HandleKind,
        HandlePosition, Node, NodeChange, NodeControls, NodeControlsAppearance, NodeExtent,
        NodeGraph, NodeGraphAppearance, NodeGraphConfig, NodeGraphEvent, NodeGraphHit,
        NodeGraphMode, NodeGraphState, NodeGraphSurface, NodeId, NodeMiniMap,
        NodeMiniMapAppearance, NodePaintContext, NodeSignal, NodeSizeMode, NodeWidgetRegistry,
        ResizeDirection, SelectionMode, SnapshotRevisions, Viewport, ViewportTransition,
        node_graph_hit_test,
    };
}
