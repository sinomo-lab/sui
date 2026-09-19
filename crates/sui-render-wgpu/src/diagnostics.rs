#[cfg(test)]
use crate::draw::PreparedFrameBatches;
use crate::retained::RetainedCompositorFrameStats;
#[cfg(test)]
use crate::submission::{
    ANALYTIC_QUAD_INSTANCE_SIZE, COMPACT_VERTEX_SIZE, EXTENDED_QUAD_INSTANCE_SIZE,
    SOLID_VERTEX_SIZE, TEXT_ATLAS_INSTANCE_SIZE,
};
use crate::text::TextFrameStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RetainedPacketRebuildStats {
    pub new_count: usize,
    pub coordinate_space_count: usize,
    pub signature_count: usize,
    pub scene_count: usize,
    pub state_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacketRebuildReason {
    NewPacket,
    CoordinateSpace,
    Signature,
    Scene,
    State,
}

impl RetainedPacketRebuildStats {
    pub const fn new(
        new_count: usize,
        coordinate_space_count: usize,
        signature_count: usize,
        scene_count: usize,
        state_count: usize,
    ) -> Self {
        Self {
            new_count,
            coordinate_space_count,
            signature_count,
            scene_count,
            state_count,
        }
    }

    pub(crate) fn record_reason(&mut self, reason: PacketRebuildReason) {
        match reason {
            PacketRebuildReason::NewPacket => self.new_count += 1,
            PacketRebuildReason::CoordinateSpace => self.coordinate_space_count += 1,
            PacketRebuildReason::Signature => self.signature_count += 1,
            PacketRebuildReason::Scene => self.scene_count += 1,
            PacketRebuildReason::State => self.state_count += 1,
        }
    }

    pub const fn total_count(&self) -> usize {
        self.new_count
            + self.coordinate_space_count
            + self.signature_count
            + self.scene_count
            + self.state_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RendererFrameStats {
    /// Scene/output submissions; atlas resource preparation is reported separately.
    pub queue_submit_count: usize,
    pub text_atlas_allocate_time_us: u64,
    pub text_atlas_clear_time_us: u64,
    pub text_atlas_copy_time_us: u64,
    pub text_atlas_create_bind_group_time_us: u64,
    /// Adapter/device acquisition and shared renderer resources, on first use.
    pub device_prepare_time_us: u64,
    /// Offscreen color-target preparation (including resize allocations).
    pub target_prepare_time_us: u64,
    pub text_engine_init_time_us: u64,
    /// Shader and pipeline creation; also included in pass encoding time.
    pub pipeline_create_time_us: u64,
    pub pipeline_create_count: usize,
    pub pass_count: usize,
    pub draw_count: usize,
    pub uploaded_vertex_bytes: u64,
    pub text_glyph_instance_count: usize,
    pub text_vertex_bytes: u64,
    pub visible_layer_count: usize,
    pub direct_packet_count: usize,
    pub retained_state_update_time_us: u64,
    pub composition_time_us: u64,
    pub retained_scene_traversal_time_us: u64,
    pub retained_packet_build_time_us: u64,
    pub retained_packet_build_count: usize,
    pub retained_packet_rebuilds: RetainedPacketRebuildStats,
    pub retained_packet_normalize_time_us: u64,
    pub retained_packet_signature_time_us: u64,
    pub retained_packet_raster_state_init_time_us: u64,
    pub retained_packet_scene_build_time_us: u64,
    pub retained_packet_command_count: usize,
    pub retained_packet_text_command_count: usize,
    pub retained_packet_path_command_count: usize,
    pub retained_packet_clip_path_command_count: usize,
    pub retained_packet_image_command_count: usize,
    pub retained_packet_rect_command_count: usize,
    pub retained_packet_text_command_time_us: u64,
    pub retained_packet_path_command_time_us: u64,
    pub retained_packet_clip_path_command_time_us: u64,
    pub retained_packet_image_command_time_us: u64,
    pub retained_packet_rect_command_time_us: u64,
    pub text_atlas_miss_count: usize,
    pub text_atlas_miss_time_us: u64,
    pub surface_acquire_time_us: u64,
    pub resource_collection_time_us: u64,
    pub bind_group_prepare_time_us: u64,
    pub image_bind_group_time_us: u64,
    pub analytic_path_bind_group_time_us: u64,
    pub analytic_path_bind_group_miss_count: usize,
    pub analytic_path_bind_group_upload_bytes: u64,
    pub text_atlas_bind_group_time_us: u64,
    pub text_atlas_upload_copy_time_us: u64,
    pub text_atlas_upload_write_time_us: u64,
    pub text_atlas_upload_bytes: u64,
    pub batch_prepare_time_us: u64,
    pub gpu_upload_time_us: u64,
    pub pass_encode_time_us: u64,
    pub queue_submit_time_us: u64,
    pub surface_present_time_us: u64,
    pub retained_packet_hotspot: Option<RendererPacketHotspot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererPacketHotspot {
    pub container_layer_id: Option<u64>,
    pub owner_widget_id: Option<u64>,
    pub segment_index: u32,
    pub total_time_us: u64,
    pub scene_build_time_us: u64,
    pub command_count: usize,
    pub text_command_count: usize,
    pub path_command_count: usize,
    pub rect_command_count: usize,
    pub text_command_time_us: u64,
    pub path_command_time_us: u64,
    pub rect_command_time_us: u64,
    pub text_sample: Option<String>,
}

impl RendererFrameStats {
    #[cfg(test)]
    pub(crate) fn from_prepared_frame(prepared: &PreparedFrameBatches) -> Self {
        Self::from_prepared_counts(
            prepared.passes.len().max(1),
            prepared
                .passes
                .iter()
                .map(|pass| pass.clip_paths.len() + pass.draws.len())
                .sum(),
            (prepared.solid_vertices.len() as u64 + prepared.clip_vertices.len() as u64)
                * SOLID_VERTEX_SIZE
                + prepared.scene_vertices.len() as u64 * COMPACT_VERTEX_SIZE
                + prepared.analytic_vertices.len() as u64 * ANALYTIC_QUAD_INSTANCE_SIZE
                + prepared.extended_vertices.len() as u64 * EXTENDED_QUAD_INSTANCE_SIZE
                + prepared.text_instances.len() as u64 * TEXT_ATLAS_INSTANCE_SIZE,
        )
    }

    pub(crate) fn from_prepared_counts(
        pass_count: usize,
        draw_count: usize,
        uploaded_vertex_bytes: u64,
    ) -> Self {
        Self {
            queue_submit_count: 0,
            text_atlas_allocate_time_us: 0,
            text_atlas_clear_time_us: 0,
            text_atlas_copy_time_us: 0,
            text_atlas_create_bind_group_time_us: 0,
            device_prepare_time_us: 0,
            target_prepare_time_us: 0,
            text_engine_init_time_us: 0,
            pipeline_create_time_us: 0,
            pipeline_create_count: 0,
            pass_count,
            draw_count,
            uploaded_vertex_bytes,
            text_glyph_instance_count: 0,
            text_vertex_bytes: 0,
            visible_layer_count: 0,
            direct_packet_count: 0,
            retained_state_update_time_us: 0,
            composition_time_us: 0,
            retained_scene_traversal_time_us: 0,
            retained_packet_build_time_us: 0,
            retained_packet_build_count: 0,
            retained_packet_rebuilds: RetainedPacketRebuildStats::default(),
            retained_packet_normalize_time_us: 0,
            retained_packet_signature_time_us: 0,
            retained_packet_raster_state_init_time_us: 0,
            retained_packet_scene_build_time_us: 0,
            retained_packet_command_count: 0,
            retained_packet_text_command_count: 0,
            retained_packet_path_command_count: 0,
            retained_packet_clip_path_command_count: 0,
            retained_packet_image_command_count: 0,
            retained_packet_rect_command_count: 0,
            retained_packet_text_command_time_us: 0,
            retained_packet_path_command_time_us: 0,
            retained_packet_clip_path_command_time_us: 0,
            retained_packet_image_command_time_us: 0,
            retained_packet_rect_command_time_us: 0,
            text_atlas_miss_count: 0,
            text_atlas_miss_time_us: 0,
            surface_acquire_time_us: 0,
            resource_collection_time_us: 0,
            bind_group_prepare_time_us: 0,
            image_bind_group_time_us: 0,
            analytic_path_bind_group_time_us: 0,
            analytic_path_bind_group_miss_count: 0,
            analytic_path_bind_group_upload_bytes: 0,
            text_atlas_bind_group_time_us: 0,
            text_atlas_upload_copy_time_us: 0,
            text_atlas_upload_write_time_us: 0,
            text_atlas_upload_bytes: 0,
            batch_prepare_time_us: 0,
            gpu_upload_time_us: 0,
            pass_encode_time_us: 0,
            queue_submit_time_us: 0,
            surface_present_time_us: 0,
            retained_packet_hotspot: None,
        }
    }

    pub(crate) fn with_compositor_stats(mut self, stats: RetainedCompositorFrameStats) -> Self {
        self.visible_layer_count = stats.visible_layers;
        self.direct_packet_count = stats.direct_packets;
        self.retained_state_update_time_us = (stats.state_update_time_ms * 1000.0).round() as u64;
        self.composition_time_us = (stats.composition_time_ms * 1000.0).round() as u64;
        self.retained_scene_traversal_time_us =
            (stats.scene_traversal_time_ms * 1000.0).round() as u64;
        self.retained_packet_build_time_us = (stats.packet_build_time_ms * 1000.0).round() as u64;
        self.retained_packet_build_count = stats.packet_build_count;
        self.retained_packet_rebuilds = stats.packet_rebuilds;
        self.retained_packet_normalize_time_us =
            (stats.packet_normalize_time_ms * 1000.0).round() as u64;
        self.retained_packet_signature_time_us =
            (stats.packet_signature_time_ms * 1000.0).round() as u64;
        self.retained_packet_raster_state_init_time_us =
            (stats.packet_raster_state_init_time_ms * 1000.0).round() as u64;
        self.retained_packet_scene_build_time_us =
            (stats.packet_scene_build_time_ms * 1000.0).round() as u64;
        self.retained_packet_command_count = stats.packet_command_count;
        self.retained_packet_text_command_count = stats.packet_text_command_count;
        self.retained_packet_path_command_count = stats.packet_path_command_count;
        self.retained_packet_clip_path_command_count = stats.packet_clip_path_command_count;
        self.retained_packet_image_command_count = stats.packet_image_command_count;
        self.retained_packet_rect_command_count = stats.packet_rect_command_count;
        self.retained_packet_text_command_time_us =
            (stats.packet_text_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_path_command_time_us =
            (stats.packet_path_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_clip_path_command_time_us =
            (stats.packet_clip_path_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_image_command_time_us =
            (stats.packet_image_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_rect_command_time_us =
            (stats.packet_rect_command_time_ms * 1000.0).round() as u64;
        self.retained_packet_hotspot =
            stats
                .slowest_packet_build
                .map(|hotspot| RendererPacketHotspot {
                    container_layer_id: hotspot.container_layer_id,
                    owner_widget_id: hotspot.owner_widget_id,
                    segment_index: hotspot.segment_index,
                    total_time_us: (hotspot.total_time_ms * 1000.0).round() as u64,
                    scene_build_time_us: (hotspot.scene_build_time_ms * 1000.0).round() as u64,
                    command_count: hotspot.command_count,
                    text_command_count: hotspot.text_command_count,
                    path_command_count: hotspot.path_command_count,
                    rect_command_count: hotspot.rect_command_count,
                    text_command_time_us: (hotspot.text_command_time_ms * 1000.0).round() as u64,
                    path_command_time_us: (hotspot.path_command_time_ms * 1000.0).round() as u64,
                    rect_command_time_us: (hotspot.rect_command_time_ms * 1000.0).round() as u64,
                    text_sample: hotspot.text_sample,
                });
        self
    }

    pub(crate) fn with_text_stats(mut self, stats: TextFrameStats) -> Self {
        self.text_glyph_instance_count = stats.glyph_instances;
        self.text_vertex_bytes = stats.glyph_upload_bytes;
        self.text_atlas_miss_count = stats.atlas_miss_count;
        self.text_atlas_miss_time_us = stats.atlas_miss_time_us;
        self
    }
}
