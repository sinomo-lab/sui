//! Serde support for [`DockWorkspaceSnapshot`], behind the `serde` feature.
//!
//! The wire shape is owned here instead of derived from the public types, so
//! their fields can change without silently changing saved layouts. A
//! deserialized snapshot is validated like any other, so malformed input is
//! reported as a deserialization error.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use sui_core::Rect;
use sui_layout::Axis;

use super::{DockFloatingGroup, DockNode, DockPanelId, DockWorkspaceSnapshot, validate_snapshot};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AxisRepr {
    Horizontal,
    Vertical,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NodeRepr {
    Empty,
    Tabs {
        panels: Vec<u64>,
        active: u64,
    },
    Split {
        axis: AxisRepr,
        fraction: f32,
        first: Box<NodeRepr>,
        second: Box<NodeRepr>,
    },
}

#[derive(Serialize, Deserialize)]
struct BoundsRepr {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Serialize, Deserialize)]
struct FloatingRepr {
    id: u64,
    panels: Vec<u64>,
    active: u64,
    bounds: BoundsRepr,
}

#[derive(Serialize, Deserialize)]
struct SnapshotRepr {
    root: NodeRepr,
    #[serde(default)]
    floating: Vec<FloatingRepr>,
    #[serde(default)]
    hidden: Vec<u64>,
}

fn panel_ids(panels: &[DockPanelId]) -> Vec<u64> {
    panels.iter().map(|panel| panel.get()).collect()
}

impl From<&DockNode> for NodeRepr {
    fn from(node: &DockNode) -> Self {
        match node {
            DockNode::Empty => Self::Empty,
            DockNode::Tabs { panels, active } => Self::Tabs {
                panels: panel_ids(panels),
                active: active.get(),
            },
            DockNode::Split {
                axis,
                fraction,
                first,
                second,
            } => Self::Split {
                axis: match axis {
                    Axis::Horizontal => AxisRepr::Horizontal,
                    Axis::Vertical => AxisRepr::Vertical,
                },
                fraction: *fraction,
                first: Box::new(Self::from(first.as_ref())),
                second: Box::new(Self::from(second.as_ref())),
            },
        }
    }
}

impl From<NodeRepr> for DockNode {
    fn from(node: NodeRepr) -> Self {
        match node {
            NodeRepr::Empty => Self::Empty,
            NodeRepr::Tabs { panels, active } => Self::Tabs {
                panels: panels.into_iter().map(DockPanelId::new).collect(),
                active: DockPanelId::new(active),
            },
            // Built directly so an out-of-range fraction reaches validation
            // instead of being clamped into a layout the file never described.
            NodeRepr::Split {
                axis,
                fraction,
                first,
                second,
            } => Self::Split {
                axis: match axis {
                    AxisRepr::Horizontal => Axis::Horizontal,
                    AxisRepr::Vertical => Axis::Vertical,
                },
                fraction,
                first: Box::new(Self::from(*first)),
                second: Box::new(Self::from(*second)),
            },
        }
    }
}

impl Serialize for DockWorkspaceSnapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SnapshotRepr {
            root: NodeRepr::from(&self.root),
            floating: self
                .floating
                .iter()
                .map(|group| FloatingRepr {
                    id: group.id,
                    panels: panel_ids(&group.panels),
                    active: group.active.get(),
                    bounds: BoundsRepr {
                        x: group.bounds.x(),
                        y: group.bounds.y(),
                        width: group.bounds.width(),
                        height: group.bounds.height(),
                    },
                })
                .collect(),
            hidden: panel_ids(&self.hidden),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DockWorkspaceSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let repr = SnapshotRepr::deserialize(deserializer)?;
        let snapshot = DockWorkspaceSnapshot {
            root: DockNode::from(repr.root),
            floating: repr
                .floating
                .into_iter()
                .map(|group| DockFloatingGroup {
                    id: group.id,
                    panels: group.panels.into_iter().map(DockPanelId::new).collect(),
                    active: DockPanelId::new(group.active),
                    bounds: Rect::new(
                        group.bounds.x,
                        group.bounds.y,
                        group.bounds.width,
                        group.bounds.height,
                    ),
                })
                .collect(),
            hidden: repr.hidden.into_iter().map(DockPanelId::new).collect(),
        };
        validate_snapshot(&snapshot).map_err(D::Error::custom)?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENE: DockPanelId = DockPanelId::new(1);
    const VIEWPORT: DockPanelId = DockPanelId::new(2);
    const DETAILS: DockPanelId = DockPanelId::new(3);
    const LOGS: DockPanelId = DockPanelId::new(4);

    #[test]
    fn snapshot_round_trips_through_json() {
        let snapshot = DockWorkspaceSnapshot::new(DockNode::split(
            Axis::Horizontal,
            0.25,
            DockNode::tabs([SCENE], SCENE),
            DockNode::split(
                Axis::Vertical,
                0.7,
                DockNode::tabs([VIEWPORT], VIEWPORT),
                DockNode::tabs(
                    [DockPanelId::new(5), DockPanelId::new(6)],
                    DockPanelId::new(6),
                ),
            ),
        ))
        .with_floating(DockFloatingGroup::new(
            3,
            [DETAILS],
            DETAILS,
            Rect::new(40.0, 60.0, 320.0, 240.0),
        ))
        .with_hidden(LOGS);

        let json = serde_json::to_string(&snapshot).expect("snapshot serializes");
        let restored: DockWorkspaceSnapshot =
            serde_json::from_str(&json).expect("snapshot deserializes");
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn wire_shape_is_stable() {
        let snapshot = DockWorkspaceSnapshot::new(DockNode::tabs([SCENE, VIEWPORT], VIEWPORT));
        assert_eq!(
            serde_json::to_value(&snapshot).expect("snapshot serializes"),
            serde_json::json!({
                "root": { "kind": "tabs", "panels": [1, 2], "active": 2 },
                "floating": [],
                "hidden": [],
            })
        );
        let minimal: DockWorkspaceSnapshot =
            serde_json::from_str(r#"{"root":{"kind":"empty"}}"#).expect("optional lists default");
        assert_eq!(minimal, DockWorkspaceSnapshot::default());
    }

    #[test]
    fn invalid_layouts_fail_to_deserialize() {
        let duplicate = r#"{"root":{"kind":"tabs","panels":[1,1],"active":1}}"#;
        assert!(serde_json::from_str::<DockWorkspaceSnapshot>(duplicate).is_err());

        let stray_active = r#"{"root":{"kind":"tabs","panels":[1],"active":2}}"#;
        assert!(serde_json::from_str::<DockWorkspaceSnapshot>(stray_active).is_err());

        let fraction = r#"{"root":{"kind":"split","axis":"horizontal","fraction":4.0,
            "first":{"kind":"tabs","panels":[1],"active":1},
            "second":{"kind":"tabs","panels":[2],"active":2}}}"#;
        assert!(serde_json::from_str::<DockWorkspaceSnapshot>(fraction).is_err());
    }
}
