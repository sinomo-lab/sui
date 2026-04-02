use std::collections::BTreeMap;

use sui_core::{DirtyRegion, Size, WindowId};
use sui_platform::AccessibilitySnapshot;
use sui_runtime::{FocusState, WidgetGraphSnapshot};
use sui_scene::{SceneCommand, SceneFrame};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneSummary {
    pub viewport: Size,
    pub dirty_regions: Vec<DirtyRegion>,
    pub command_count: usize,
    pub command_breakdown: Vec<(String, usize)>,
}

impl SceneSummary {
    pub(crate) fn from_frame(frame: &SceneFrame) -> Self {
        let mut command_breakdown = BTreeMap::<String, usize>::new();
        for command in frame.scene.commands() {
            *command_breakdown
                .entry(command_kind(command).to_string())
                .or_default() += 1;
        }

        Self {
            viewport: frame.viewport,
            dirty_regions: frame.dirty_regions.clone(),
            command_count: frame.scene.commands().len(),
            command_breakdown: command_breakdown.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSnapshot {
    pub window_id: WindowId,
    pub title: String,
    pub accessibility: AccessibilitySnapshot,
    pub widget_graph: WidgetGraphSnapshot,
    pub focus_state: FocusState,
    pub scene_summary: Option<SceneSummary>,
}

fn command_kind(command: &SceneCommand) -> &'static str {
    match command {
        SceneCommand::Clear(_) => "Clear",
        SceneCommand::FillRect { .. } => "FillRect",
        SceneCommand::StrokeRect { .. } => "StrokeRect",
        SceneCommand::FillPath { .. } => "FillPath",
        SceneCommand::StrokePath { .. } => "StrokePath",
        SceneCommand::DrawText(_) => "DrawText",
        SceneCommand::DrawShapedText(_) => "DrawShapedText",
        SceneCommand::DrawImage { .. } => "DrawImage",
        SceneCommand::PushClip { .. } => "PushClip",
        SceneCommand::PushClipPath { .. } => "PushClipPath",
        SceneCommand::PopClip => "PopClip",
        SceneCommand::PushTransform { .. } => "PushTransform",
        SceneCommand::PopTransform => "PopTransform",
        SceneCommand::Label { .. } => "Label",
    }
}
