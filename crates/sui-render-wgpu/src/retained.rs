use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CompositionContainerId {
    Root,
    Layer(SceneLayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RetainedPacketId {
    pub(crate) container: CompositionContainerId,
    pub(crate) segment_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TransformNodeId(u64);

impl TransformNodeId {
    const ROOT: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClipNodeId(u64);

impl ClipNodeId {
    const ROOT: Self = Self(0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EffectNodeId(u64);

impl EffectNodeId {
    const ROOT: Self = Self(0);
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) const MAX_ANALYTIC_PATH_CONTOURS: usize = 32;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) const MAX_ANALYTIC_PATH_POINTS: usize = 512;

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct RetainedCompositorFrameStats {
    pub(crate) visible_layers: usize,
    pub(crate) direct_packets: usize,
    pub(crate) state_update_time_ms: f64,
    pub(crate) composition_time_ms: f64,
    pub(crate) scene_traversal_time_ms: f64,
    pub(crate) packet_build_count: usize,
    pub(crate) packet_build_time_ms: f64,
    pub(crate) packet_rebuilds: RetainedPacketRebuildStats,
    pub(crate) packet_normalize_time_ms: f64,
    pub(crate) packet_signature_time_ms: f64,
    pub(crate) packet_raster_state_init_time_ms: f64,
    pub(crate) packet_scene_build_time_ms: f64,
    pub(crate) packet_command_count: usize,
    pub(crate) packet_text_command_count: usize,
    pub(crate) packet_path_command_count: usize,
    pub(crate) packet_clip_path_command_count: usize,
    pub(crate) packet_image_command_count: usize,
    pub(crate) packet_rect_command_count: usize,
    pub(crate) packet_text_command_time_ms: f64,
    pub(crate) packet_path_command_time_ms: f64,
    pub(crate) packet_clip_path_command_time_ms: f64,
    pub(crate) packet_image_command_time_ms: f64,
    pub(crate) packet_rect_command_time_ms: f64,
    pub(crate) slowest_packet_build: Option<RetainedPacketBuildHotspot>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RetainedPacketBuildHotspot {
    pub(crate) container_layer_id: Option<u64>,
    pub(crate) owner_widget_id: Option<u64>,
    pub(crate) segment_index: u32,
    pub(crate) total_time_ms: f64,
    pub(crate) scene_build_time_ms: f64,
    pub(crate) command_count: usize,
    pub(crate) text_command_count: usize,
    pub(crate) path_command_count: usize,
    pub(crate) rect_command_count: usize,
    pub(crate) text_command_time_ms: f64,
    pub(crate) path_command_time_ms: f64,
    pub(crate) rect_command_time_ms: f64,
    pub(crate) text_sample: Option<String>,
}

impl RetainedCompositorFrameStats {
    fn record_packet_rebuild(&mut self, reason: PacketRebuildReason) {
        self.packet_rebuilds.record_reason(reason);
    }

    fn record_packet_build_diagnostics(
        &mut self,
        diagnostics: DirectPacketBuildDiagnostics,
        normalize_time_ms: f64,
        signature_time_ms: f64,
    ) {
        self.packet_normalize_time_ms += normalize_time_ms;
        self.packet_signature_time_ms += signature_time_ms;
        self.packet_raster_state_init_time_ms += diagnostics.raster_state_init_time_ms;
        self.packet_scene_build_time_ms += diagnostics.scene_build_time_ms;
        self.packet_command_count += diagnostics.command_count;
        self.packet_text_command_count += diagnostics.text_command_count;
        self.packet_path_command_count += diagnostics.path_command_count;
        self.packet_clip_path_command_count += diagnostics.clip_path_command_count;
        self.packet_image_command_count += diagnostics.image_command_count;
        self.packet_rect_command_count += diagnostics.rect_command_count;
        self.packet_text_command_time_ms += diagnostics.text_command_time_ms;
        self.packet_path_command_time_ms += diagnostics.path_command_time_ms;
        self.packet_clip_path_command_time_ms += diagnostics.clip_path_command_time_ms;
        self.packet_image_command_time_ms += diagnostics.image_command_time_ms;
        self.packet_rect_command_time_ms += diagnostics.rect_command_time_ms;
    }

    fn consider_packet_build_hotspot(&mut self, hotspot: RetainedPacketBuildHotspot) {
        let should_replace = self
            .slowest_packet_build
            .as_ref()
            .map(|current| hotspot.total_time_ms > current.total_time_ms)
            .unwrap_or(true);
        if should_replace {
            self.slowest_packet_build = Some(hotspot);
        }
    }
}

#[derive(Debug)]
pub(crate) struct RetainedFrameSubmission {
    pub(crate) fragments: Vec<RetainedFrameFragment>,
}

#[derive(Debug)]
pub(crate) enum RetainedFrameFragment {
    Transient(DrawOpArena),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionItem {
    Packet(RetainedPacketId),
    Layer(SceneLayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionPhase {
    Normal,
    Overlay,
    Effect,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TransformNode {
    id: TransformNodeId,
    parent: Option<TransformNodeId>,
    local: Transform,
    world: Transform,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedClipPrimitive {
    Rect(Rect),
    Path {
        path: ScenePath,
        bounds: Rect,
        signature: u64,
    },
}

#[allow(dead_code)]
impl ResolvedClipPrimitive {
    pub(crate) fn bounds(&self) -> Rect {
        match self {
            Self::Rect(rect) => *rect,
            Self::Path { bounds, .. } => *bounds,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ClipNode {
    id: ClipNodeId,
    parent: Option<ClipNodeId>,
    primitive: Option<ResolvedClipPrimitive>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct EffectNode {
    id: EffectNodeId,
    parent: Option<EffectNodeId>,
    composition_mode: sui_scene::LayerCompositionMode,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedRasterState {
    pub(crate) current_transform: Transform,
    pub(crate) clip_stack: Vec<ResolvedClipPrimitive>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    effect_node: EffectNodeId,
}

impl ResolvedRasterState {
    fn signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        hash_transform(&mut hasher, self.current_transform);
        self.transform_node.hash(&mut hasher);
        self.clip_node.hash(&mut hasher);
        self.effect_node.hash(&mut hasher);
        for clip in &self.clip_stack {
            match clip {
                ResolvedClipPrimitive::Rect(rect) => {
                    0u8.hash(&mut hasher);
                    hash_rect(&mut hasher, *rect);
                }
                ResolvedClipPrimitive::Path {
                    bounds, signature, ..
                } => {
                    1u8.hash(&mut hasher);
                    hash_rect(&mut hasher, *bounds);
                    signature.hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }
}

fn clip_stack_signature(clips: &[ResolvedClipPrimitive]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for clip in clips {
        match clip {
            ResolvedClipPrimitive::Rect(rect) => {
                0u8.hash(&mut hasher);
                hash_rect(&mut hasher, *rect);
            }
            ResolvedClipPrimitive::Path {
                bounds, signature, ..
            } => {
                1u8.hash(&mut hasher);
                hash_rect(&mut hasher, *bounds);
                signature.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

fn normalized_clip_stack_signature(
    clips: &[ResolvedClipPrimitive],
    normalization_origin: Vector,
) -> u64 {
    if normalization_origin == Vector::ZERO {
        return clip_stack_signature(clips);
    }

    let delta = Vector::new(-normalization_origin.x, -normalization_origin.y);
    let normalized = clips
        .iter()
        .cloned()
        .map(|clip| translate_resolved_clip_primitive(clip, delta))
        .collect::<Vec<_>>();
    clip_stack_signature(&normalized)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct RetainedDirectPacket {
    pub(crate) id: RetainedPacketId,
    pub(crate) scene: Scene,
    pub(crate) initial_state: ResolvedRasterState,
    pub(crate) signature: u64,
    coordinate_space: PacketCoordinateSpace,
    pub(crate) draw_ops: DrawOpArena,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketCoordinateSpace {
    World,
    LayerLocal,
}

#[derive(Debug, Clone, Default)]
struct RetainedRootNode {
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    structure_version: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct RetainedLayer {
    pub(crate) descriptor: sui_scene::SceneLayerDescriptor,
    pub(crate) parent: Option<SceneLayerId>,
    pub(crate) children: Vec<SceneLayerId>,
    items: Vec<CompositionItem>,
    pub(crate) packet_ids: Vec<RetainedPacketId>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    pub(crate) clip_signature: u64,
    effect_node: EffectNodeId,
    pub(crate) content_version: u64,
    pub(crate) structure_version: u64,
}

#[derive(Debug, Clone)]
struct PacketSnapshot {
    id: RetainedPacketId,
    scene: Scene,
    initial_state: ResolvedRasterState,
}

#[derive(Debug, Clone)]
struct LayerSnapshot {
    descriptor: sui_scene::SceneLayerDescriptor,
    parent: Option<SceneLayerId>,
    children: Vec<SceneLayerId>,
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    packets: Vec<PacketSnapshot>,
    transform_node: TransformNodeId,
    clip_node: ClipNodeId,
    clip_signature: u64,
    effect_node: EffectNodeId,
}

#[derive(Debug, Clone, Default)]
struct RootSnapshot {
    items: Vec<CompositionItem>,
    packet_ids: Vec<RetainedPacketId>,
    packets: Vec<PacketSnapshot>,
}

#[derive(Debug, Clone, Default)]
struct CompositorSnapshot {
    root: RootSnapshot,
    layers: HashMap<SceneLayerId, LayerSnapshot>,
}

#[derive(Debug, Clone)]
struct CompositionTraversalState {
    current_transform: Transform,
    transform_node: TransformNodeId,
    transform_stack: Vec<(Transform, TransformNodeId)>,
    clip_stack: Vec<(ResolvedClipPrimitive, ClipNodeId)>,
    effect_node: EffectNodeId,
}

impl Default for CompositionTraversalState {
    fn default() -> Self {
        Self {
            current_transform: Transform::IDENTITY,
            transform_node: TransformNodeId::ROOT,
            transform_stack: Vec::new(),
            clip_stack: Vec::new(),
            effect_node: EffectNodeId::ROOT,
        }
    }
}

impl CompositionTraversalState {
    fn resolved_state(&self) -> ResolvedRasterState {
        ResolvedRasterState {
            current_transform: self.current_transform,
            clip_stack: self
                .clip_stack
                .iter()
                .map(|(primitive, _)| primitive.clone())
                .collect(),
            transform_node: self.transform_node,
            clip_node: self
                .clip_stack
                .last()
                .map(|(_, node_id)| *node_id)
                .unwrap_or(ClipNodeId::ROOT),
            effect_node: self.effect_node,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RetainedCompositorState {
    root: RetainedRootNode,
    pub(crate) layers: HashMap<SceneLayerId, RetainedLayer>,
    pub(crate) packets: HashMap<RetainedPacketId, RetainedDirectPacket>,
    transforms: HashMap<TransformNodeId, TransformNode>,
    clips: HashMap<ClipNodeId, ClipNode>,
    effects: HashMap<EffectNodeId, EffectNode>,
    pub(crate) next_transform_node: u64,
    pub(crate) next_clip_node: u64,
    pub(crate) next_effect_node: u64,
    pub(crate) viewport: Size,
    pub(crate) feather_width_bits: u32,
    pub(crate) frame_index: u64,
    pub(crate) last_frame_stats: RetainedCompositorFrameStats,
    pub(crate) diagnostics_enabled: bool,
    pub(crate) path_cache: PathMeshCache,
}

impl Default for RetainedCompositorState {
    fn default() -> Self {
        Self {
            root: RetainedRootNode::default(),
            layers: HashMap::new(),
            packets: HashMap::new(),
            transforms: HashMap::new(),
            clips: HashMap::new(),
            effects: HashMap::new(),
            next_transform_node: 0,
            next_clip_node: 0,
            next_effect_node: 0,
            viewport: Size::ZERO,
            feather_width_bits: 0,
            frame_index: 0,
            last_frame_stats: RetainedCompositorFrameStats::default(),
            diagnostics_enabled: true,
            path_cache: PathMeshCache::default(),
        }
    }
}

impl RetainedCompositorState {
    pub(crate) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics_enabled = enabled;
        self.path_cache.set_diagnostics_enabled(enabled);
        if !enabled {
            self.last_frame_stats = RetainedCompositorFrameStats::default();
        }
    }

    #[cfg(test)]
    pub(crate) fn prepare_frame(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<DrawOpArena> {
        let mut frame_stats = self.refresh_frame_state(frame, text_engine, feather_width)?;
        let composition_started = self.diagnostics_enabled.then(|| Instant::now());
        let draw_ops = self.compose_draw_ops(frame.viewport, &mut frame_stats)?;
        self.finish_frame(
            frame.viewport,
            feather_width,
            &mut frame_stats,
            composition_started,
        );
        Ok(draw_ops)
    }

    pub(crate) fn prepare_frame_submission(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<RetainedFrameSubmission> {
        let mut frame_stats = self.refresh_frame_state(frame, text_engine, feather_width)?;
        let composition_started = self.diagnostics_enabled.then(|| Instant::now());
        let submission = self.compose_submission(frame.viewport, &mut frame_stats)?;
        self.finish_frame(
            frame.viewport,
            feather_width,
            &mut frame_stats,
            composition_started,
        );
        Ok(submission)
    }

    fn refresh_frame_state(
        &mut self,
        frame: &SceneFrame,
        text_engine: &mut TextEngine,
        feather_width: f32,
    ) -> Result<RetainedCompositorFrameStats> {
        let viewport_changed = self.viewport != frame.viewport;
        let feather_changed = self.feather_width_bits != feather_width.to_bits();
        self.frame_index = self.frame_index.wrapping_add(1);
        let mut frame_stats = RetainedCompositorFrameStats::default();
        let scene_traversal_started = self.diagnostics_enabled.then(|| Instant::now());
        let snapshot = self.build_snapshot(&frame.scene)?;
        if let Some(started) = scene_traversal_started {
            frame_stats.scene_traversal_time_ms = started.elapsed().as_secs_f64() * 1000.0;
        }
        let state_update_started = self.diagnostics_enabled.then(|| Instant::now());
        self.apply_snapshot(
            frame,
            snapshot,
            text_engine,
            feather_width,
            viewport_changed || feather_changed,
            &mut frame_stats,
        )?;
        if let Some(started) = state_update_started {
            frame_stats.state_update_time_ms = started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(frame_stats)
    }

    fn finish_frame(
        &mut self,
        viewport: Size,
        feather_width: f32,
        frame_stats: &mut RetainedCompositorFrameStats,
        composition_started: Option<Instant>,
    ) {
        if let Some(started) = composition_started {
            frame_stats.composition_time_ms = started.elapsed().as_secs_f64() * 1000.0;
            self.last_frame_stats = frame_stats.clone();
        } else {
            self.last_frame_stats = RetainedCompositorFrameStats::default();
        }
        self.viewport = viewport;
        self.feather_width_bits = feather_width.to_bits();
    }

    fn build_snapshot(&mut self, scene: &Scene) -> Result<CompositorSnapshot> {
        self.reset_property_trees();
        let mut snapshot = CompositorSnapshot::default();
        snapshot.root = self.build_container_snapshot(
            CompositionContainerId::Root,
            scene,
            CompositionTraversalState::default(),
            &mut snapshot,
            None,
        )?;
        Ok(snapshot)
    }

    fn build_container_snapshot(
        &mut self,
        container: CompositionContainerId,
        scene: &Scene,
        mut state: CompositionTraversalState,
        snapshot: &mut CompositorSnapshot,
        parent_layer: Option<SceneLayerId>,
    ) -> Result<RootSnapshot> {
        let mut result = RootSnapshot::default();
        let mut normal_items = Vec::new();
        let mut overlay_items = Vec::new();
        let mut effect_items = Vec::new();
        let mut segment_scene = Scene::new();
        let mut segment_start = None::<ResolvedRasterState>;

        for command in scene.commands() {
            match command {
                SceneCommand::Layer(layer) => {
                    flush_container_segment(
                        &self.effects,
                        container,
                        &mut result,
                        &mut normal_items,
                        &mut overlay_items,
                        &mut effect_items,
                        &mut segment_scene,
                        &mut segment_start,
                    );

                    let mut child_state = state.clone();
                    child_state.effect_node = self.push_effect_node(
                        Some(state.effect_node),
                        layer.descriptor.composition_mode,
                    );
                    if layer.descriptor.composition_mode == sui_scene::LayerCompositionMode::Scroll
                        || layer.descriptor.is_stack_surface
                    {
                        let clip = ResolvedClipPrimitive::Rect(layer.descriptor.presented_bounds());
                        let parent = child_state
                            .clip_stack
                            .last()
                            .map(|(_, node_id)| *node_id)
                            .unwrap_or(ClipNodeId::ROOT);
                        let node_id = self.push_clip_node(Some(parent), clip.clone());
                        child_state.clip_stack.push((clip, node_id));
                    }
                    let phase =
                        composition_phase_for_effect_node(child_state.effect_node, &self.effects);
                    let layer_snapshot =
                        self.build_layer_snapshot(layer, parent_layer, child_state, snapshot)?;
                    push_composition_item(
                        phase,
                        CompositionItem::Layer(layer.layer_id()),
                        &mut normal_items,
                        &mut overlay_items,
                        &mut effect_items,
                    );
                    snapshot.layers.insert(layer.layer_id(), layer_snapshot);
                }
                _ => {
                    if segment_start.is_none() {
                        segment_start = Some(state.resolved_state());
                    }
                    segment_scene.push(command.clone());
                    self.apply_command_to_traversal_state(command, &mut state);
                }
            }
        }

        flush_container_segment(
            &self.effects,
            container,
            &mut result,
            &mut normal_items,
            &mut overlay_items,
            &mut effect_items,
            &mut segment_scene,
            &mut segment_start,
        );
        result.items = normal_items;
        result.items.extend(overlay_items);
        result.items.extend(effect_items);
        Ok(result)
    }

    fn build_layer_snapshot(
        &mut self,
        layer: &SceneLayer,
        parent_layer: Option<SceneLayerId>,
        state: CompositionTraversalState,
        snapshot: &mut CompositorSnapshot,
    ) -> Result<LayerSnapshot> {
        let inherited_state = state.resolved_state();
        let container = self.build_container_snapshot(
            CompositionContainerId::Layer(layer.layer_id()),
            &layer.scene,
            state,
            snapshot,
            Some(layer.layer_id()),
        )?;
        let clip_signature = normalized_clip_stack_signature(
            &inherited_state.clip_stack,
            layer.descriptor.presented_bounds().origin.to_vector(),
        );
        let children = container
            .items
            .iter()
            .filter_map(|item| match item {
                CompositionItem::Layer(layer_id) => Some(*layer_id),
                CompositionItem::Packet(_) => None,
            })
            .collect();

        Ok(LayerSnapshot {
            descriptor: layer.descriptor.clone(),
            parent: parent_layer,
            children,
            items: container.items,
            packet_ids: container.packet_ids,
            packets: container.packets,
            transform_node: inherited_state.transform_node,
            clip_node: inherited_state.clip_node,
            clip_signature,
            effect_node: inherited_state.effect_node,
        })
    }

    fn apply_command_to_traversal_state(
        &mut self,
        command: &SceneCommand,
        state: &mut CompositionTraversalState,
    ) {
        match command {
            SceneCommand::PushTransform { transform } => {
                let parent_world = state.current_transform;
                let parent_node = state.transform_node;
                state.transform_stack.push((parent_world, parent_node));
                let world = parent_world.then(*transform);
                state.current_transform = world;
                state.transform_node =
                    self.push_transform_node(Some(parent_node), *transform, world);
            }
            SceneCommand::PopTransform => {
                let (world, node_id) = state
                    .transform_stack
                    .pop()
                    .unwrap_or((Transform::IDENTITY, TransformNodeId::ROOT));
                state.current_transform = world;
                state.transform_node = node_id;
            }
            SceneCommand::PushClip { rect } => {
                let clip =
                    ResolvedClipPrimitive::Rect(state.current_transform.transform_rect_bbox(*rect));
                let parent = state
                    .clip_stack
                    .last()
                    .map(|(_, node_id)| *node_id)
                    .unwrap_or(ClipNodeId::ROOT);
                let node_id = self.push_clip_node(Some(parent), clip.clone());
                state.clip_stack.push((clip, node_id));
            }
            SceneCommand::PushClipPath { path } => {
                let transformed_path = transform_scene_path(path, state.current_transform);
                let bounds = transformed_path.bounds();
                let clip = ResolvedClipPrimitive::Path {
                    signature: hash_path(&transformed_path, Transform::IDENTITY),
                    path: transformed_path,
                    bounds,
                };
                let parent = state
                    .clip_stack
                    .last()
                    .map(|(_, node_id)| *node_id)
                    .unwrap_or(ClipNodeId::ROOT);
                let node_id = self.push_clip_node(Some(parent), clip.clone());
                state.clip_stack.push((clip, node_id));
            }
            SceneCommand::PopClip => {
                let _ = state.clip_stack.pop();
            }
            SceneCommand::Clear(_)
            | SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawText(_)
            | SceneCommand::DrawShapedText(_)
            | SceneCommand::DrawShapedTextWindow(_)
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawShaderRect { .. }
            | SceneCommand::Layer(_)
            | SceneCommand::Label { .. } => {}
        }
    }

    fn apply_snapshot(
        &mut self,
        frame: &SceneFrame,
        snapshot: CompositorSnapshot,
        text_engine: &mut TextEngine,
        feather_width: f32,
        global_rebuild: bool,
        frame_stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        let previous_layers = self.layers.clone();
        let layer_translation_deltas = snapshot
            .layers
            .iter()
            .filter_map(|(layer_id, layer_snapshot)| {
                previous_layers.get(layer_id).and_then(|previous| {
                    descriptor_translation_delta(&previous.descriptor, &layer_snapshot.descriptor)
                        .map(|delta| (*layer_id, delta))
                })
            })
            .collect::<HashMap<_, _>>();
        let mut packet_dirty_layers = HashSet::new();
        let current_layers = snapshot.layers.keys().copied().collect::<HashSet<_>>();
        let mut root_dirty = global_rebuild;

        for update in &frame.layer_updates {
            if !current_layers.contains(&update.layer_id) {
                root_dirty = true;
                continue;
            }

            match update.kind {
                SceneLayerUpdateKind::Content | SceneLayerUpdateKind::Resources => {
                    packet_dirty_layers.insert(update.layer_id);
                }
                SceneLayerUpdateKind::Transform
                    if !layer_translation_deltas.contains_key(&update.layer_id) =>
                {
                    packet_dirty_layers.insert(update.layer_id);
                }
                SceneLayerUpdateKind::Ordering
                | SceneLayerUpdateKind::Clip
                | SceneLayerUpdateKind::Effect
                | SceneLayerUpdateKind::Visibility
                | SceneLayerUpdateKind::Transform => {}
            }
        }

        let mut valid_packets = HashSet::new();
        valid_packets.extend(snapshot.root.packet_ids.iter().copied());
        for layer in snapshot.layers.values() {
            valid_packets.extend(layer.packet_ids.iter().copied());
        }

        if self.root.items != snapshot.root.items
            || self.root.packet_ids != snapshot.root.packet_ids
        {
            self.root.structure_version = self.root.structure_version.wrapping_add(1);
        }
        self.root.items = snapshot.root.items.clone();
        self.root.packet_ids = snapshot.root.packet_ids.clone();

        for packet in snapshot.root.packets {
            self.upsert_packet(
                frame,
                packet,
                root_dirty,
                PacketCoordinateSpace::World,
                Vector::ZERO,
                text_engine,
                feather_width,
                frame_stats,
            )?;
        }

        self.layers
            .retain(|layer_id, _| current_layers.contains(layer_id));

        for (layer_id, layer_snapshot) in snapshot.layers {
            let translation_delta = layer_translation_deltas.get(&layer_id).copied();
            let translated_only = translation_delta.is_some();
            let structure_changed = previous_layers.get(&layer_id).is_none_or(|previous| {
                previous.parent != layer_snapshot.parent
                    || previous.children != layer_snapshot.children
                    || previous.items != layer_snapshot.items
                    || previous.packet_ids != layer_snapshot.packet_ids
                    || previous.transform_node != layer_snapshot.transform_node
                    || previous.clip_signature != layer_snapshot.clip_signature
                    || previous.effect_node != layer_snapshot.effect_node
                    || (!translated_only && previous.descriptor != layer_snapshot.descriptor)
            });

            let content_changed = packet_dirty_layers.contains(&layer_id)
                || previous_layers.get(&layer_id).is_none_or(|previous| {
                    !translated_only
                        && previous.descriptor.bounds != layer_snapshot.descriptor.bounds
                });

            let previous = previous_layers.get(&layer_id);
            let retained = self
                .layers
                .entry(layer_id)
                .or_insert_with(|| RetainedLayer {
                    descriptor: layer_snapshot.descriptor.clone(),
                    parent: layer_snapshot.parent,
                    children: layer_snapshot.children.clone(),
                    items: layer_snapshot.items.clone(),
                    packet_ids: layer_snapshot.packet_ids.clone(),
                    transform_node: layer_snapshot.transform_node,
                    clip_node: layer_snapshot.clip_node,
                    clip_signature: layer_snapshot.clip_signature,
                    effect_node: layer_snapshot.effect_node,
                    content_version: 0,
                    structure_version: 0,
                });

            if structure_changed {
                retained.structure_version = previous
                    .map_or(retained.structure_version + 1, |old| {
                        old.structure_version.wrapping_add(1)
                    });
            }
            if content_changed {
                retained.content_version = previous.map_or(retained.content_version + 1, |old| {
                    old.content_version.wrapping_add(1)
                });
            }

            retained.descriptor = layer_snapshot.descriptor.clone();
            retained.parent = layer_snapshot.parent;
            retained.children = layer_snapshot.children.clone();
            retained.items = layer_snapshot.items.clone();
            retained.packet_ids = layer_snapshot.packet_ids.clone();
            retained.transform_node = layer_snapshot.transform_node;
            retained.clip_node = layer_snapshot.clip_node;
            retained.clip_signature = layer_snapshot.clip_signature;
            retained.effect_node = layer_snapshot.effect_node;

            let packet_dirty =
                global_rebuild || structure_changed || packet_dirty_layers.contains(&layer_id);
            let coordinate_space = PacketCoordinateSpace::LayerLocal;
            let normalization_origin = layer_snapshot.descriptor.bounds.origin.to_vector();
            for packet in layer_snapshot.packets {
                self.upsert_packet(
                    frame,
                    packet,
                    packet_dirty,
                    coordinate_space,
                    normalization_origin,
                    text_engine,
                    feather_width,
                    frame_stats,
                )?;
            }
        }

        self.packets
            .retain(|packet_id, _| valid_packets.contains(packet_id));
        Ok(())
    }

    fn upsert_packet(
        &mut self,
        frame: &SceneFrame,
        snapshot: PacketSnapshot,
        _forced_dirty: bool,
        coordinate_space: PacketCoordinateSpace,
        normalization_origin: Vector,
        text_engine: &mut TextEngine,
        feather_width: f32,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        let normalize_started = self.diagnostics_enabled.then(Instant::now);
        let snapshot = normalize_packet_snapshot(snapshot, coordinate_space, normalization_origin);
        let normalize_time_ms = normalize_started
            .map(|started| started.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        let signature_started = self.diagnostics_enabled.then(Instant::now);
        let signature = packet_signature(
            &snapshot.scene,
            &snapshot.initial_state,
            frame.viewport,
            feather_width,
        );
        let signature_time_ms = signature_started
            .map(|started| started.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        let rebuild_reason = match self.packets.get(&snapshot.id) {
            None => Some(PacketRebuildReason::NewPacket),
            Some(packet) if packet.coordinate_space != coordinate_space => {
                Some(PacketRebuildReason::CoordinateSpace)
            }
            Some(packet) if packet.signature != signature => {
                if self.diagnostics_enabled {
                    if packet.scene != snapshot.scene {
                        Some(PacketRebuildReason::Scene)
                    } else if packet.initial_state != snapshot.initial_state {
                        Some(PacketRebuildReason::State)
                    } else {
                        Some(PacketRebuildReason::Signature)
                    }
                } else {
                    Some(PacketRebuildReason::Signature)
                }
            }
            Some(packet) if packet.scene != snapshot.scene => Some(PacketRebuildReason::Scene),
            Some(packet) if packet.initial_state != snapshot.initial_state => {
                Some(PacketRebuildReason::State)
            }
            Some(_) => None,
        };

        if let Some(reason) = rebuild_reason {
            let packet_build_started = self.diagnostics_enabled.then(|| Instant::now());
            let (draw_ops, diagnostics) = build_direct_packet_with_diagnostics(
                frame,
                &snapshot.scene,
                &snapshot.initial_state,
                text_engine,
                &mut self.path_cache,
                feather_width,
            )?;
            stats.record_packet_rebuild(reason);
            stats.record_packet_build_diagnostics(
                diagnostics,
                normalize_time_ms,
                signature_time_ms,
            );
            if let Some(started) = packet_build_started {
                let total_time_ms = started.elapsed().as_secs_f64() * 1000.0;
                stats.packet_build_count += 1;
                stats.packet_build_time_ms += total_time_ms;
                let (container_layer_id, owner_widget_id) = match snapshot.id.container {
                    CompositionContainerId::Root => (None, None),
                    CompositionContainerId::Layer(layer_id) => (
                        Some(layer_id.get()),
                        self.layers
                            .get(&layer_id)
                            .map(|layer| layer.descriptor.owner.get()),
                    ),
                };
                stats.consider_packet_build_hotspot(RetainedPacketBuildHotspot {
                    container_layer_id,
                    owner_widget_id,
                    segment_index: snapshot.id.segment_index,
                    total_time_ms,
                    scene_build_time_ms: diagnostics.scene_build_time_ms,
                    command_count: diagnostics.command_count,
                    text_command_count: diagnostics.text_command_count,
                    path_command_count: diagnostics.path_command_count,
                    rect_command_count: diagnostics.rect_command_count,
                    text_command_time_ms: diagnostics.text_command_time_ms,
                    path_command_time_ms: diagnostics.path_command_time_ms,
                    rect_command_time_ms: diagnostics.rect_command_time_ms,
                    text_sample: packet_text_sample(&snapshot.scene),
                });
            }
            self.packets.insert(
                snapshot.id,
                RetainedDirectPacket {
                    id: snapshot.id,
                    scene: snapshot.scene,
                    initial_state: snapshot.initial_state,
                    signature,
                    coordinate_space,
                    draw_ops,
                },
            );
        }

        Ok(())
    }

    #[cfg(test)]
    fn compose_draw_ops(
        &self,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<DrawOpArena> {
        let mut draw_ops = DrawOpArena::default();
        for phase in [
            CompositionPhase::Normal,
            CompositionPhase::Overlay,
            CompositionPhase::Effect,
        ] {
            self.append_items_for_phase(&self.root.items, phase, &mut draw_ops, viewport, stats)?;
        }
        Ok(draw_ops)
    }

    fn compose_submission(
        &self,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<RetainedFrameSubmission> {
        let mut submission = RetainedFrameSubmission {
            fragments: Vec::new(),
        };
        let mut current = DrawOpArena::default();
        for phase in [
            CompositionPhase::Normal,
            CompositionPhase::Overlay,
            CompositionPhase::Effect,
        ] {
            self.append_items_to_submission_for_phase(
                &self.root.items,
                phase,
                &mut current,
                &mut submission,
                viewport,
                stats,
            )?;
        }
        flush_transient_fragment(&mut submission, &mut current);
        Ok(submission)
    }

    #[cfg(test)]
    fn append_items_for_phase(
        &self,
        items: &[CompositionItem],
        phase: CompositionPhase,
        draw_ops: &mut DrawOpArena,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        for item in items {
            match item {
                CompositionItem::Packet(packet_id) => {
                    if let Some(packet) = self.packets.get(packet_id) {
                        if composition_phase_for_effect_node(
                            packet.initial_state.effect_node,
                            &self.effects,
                        ) != phase
                        {
                            continue;
                        }
                        match packet.coordinate_space {
                            PacketCoordinateSpace::World => {
                                draw_ops.append_fragment(&packet.draw_ops);
                            }
                            PacketCoordinateSpace::LayerLocal => {
                                let (origin, clip_stack, opacity) = match packet.id.container {
                                    CompositionContainerId::Root => (Vector::ZERO, Vec::new(), 1.0),
                                    CompositionContainerId::Layer(layer_id) => self
                                        .layers
                                        .get(&layer_id)
                                        .map(|layer| {
                                            (
                                                layer
                                                    .descriptor
                                                    .presented_bounds()
                                                    .origin
                                                    .to_vector(),
                                                resolved_clip_primitives(
                                                    layer.clip_node,
                                                    &self.clips,
                                                ),
                                                layer.descriptor.properties.opacity,
                                            )
                                        })
                                        .unwrap_or((Vector::ZERO, Vec::new(), 1.0)),
                                };
                                draw_ops.append_composed_fragment(
                                    &packet.draw_ops,
                                    origin,
                                    opacity,
                                    &clip_stack,
                                    viewport,
                                )?;
                            }
                        }
                        stats.direct_packets += 1;
                    }
                }
                CompositionItem::Layer(layer_id) => {
                    if let Some(layer) = self.layers.get(layer_id) {
                        stats.visible_layers += 1;
                        self.append_items_for_phase(
                            &layer.items,
                            phase,
                            draw_ops,
                            viewport,
                            stats,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    fn append_items_to_submission_for_phase(
        &self,
        items: &[CompositionItem],
        phase: CompositionPhase,
        current: &mut DrawOpArena,
        submission: &mut RetainedFrameSubmission,
        viewport: Size,
        stats: &mut RetainedCompositorFrameStats,
    ) -> Result<()> {
        for item in items {
            match item {
                CompositionItem::Packet(packet_id) => {
                    if let Some(packet) = self.packets.get(packet_id) {
                        if composition_phase_for_effect_node(
                            packet.initial_state.effect_node,
                            &self.effects,
                        ) != phase
                        {
                            continue;
                        }
                        match packet.coordinate_space {
                            PacketCoordinateSpace::World => {
                                current.append_fragment(&packet.draw_ops);
                            }
                            PacketCoordinateSpace::LayerLocal => {
                                let (origin, clip_stack, opacity) = match packet.id.container {
                                    CompositionContainerId::Root => (Vector::ZERO, Vec::new(), 1.0),
                                    CompositionContainerId::Layer(layer_id) => self
                                        .layers
                                        .get(&layer_id)
                                        .map(|layer| {
                                            (
                                                layer
                                                    .descriptor
                                                    .presented_bounds()
                                                    .origin
                                                    .to_vector(),
                                                resolved_clip_primitives(
                                                    layer.clip_node,
                                                    &self.clips,
                                                ),
                                                layer.descriptor.properties.opacity,
                                            )
                                        })
                                        .unwrap_or((Vector::ZERO, Vec::new(), 1.0)),
                                };
                                current.append_composed_fragment(
                                    &packet.draw_ops,
                                    origin,
                                    opacity,
                                    &clip_stack,
                                    viewport,
                                )?;
                            }
                        }
                        stats.direct_packets += 1;
                    }
                }
                CompositionItem::Layer(layer_id) => {
                    if let Some(layer) = self.layers.get(layer_id) {
                        stats.visible_layers += 1;
                        self.append_items_to_submission_for_phase(
                            &layer.items,
                            phase,
                            current,
                            submission,
                            viewport,
                            stats,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn flush_transient_fragment(submission: &mut RetainedFrameSubmission, current: &mut DrawOpArena) {
    if current.draw_ops.is_empty() {
        return;
    }

    submission
        .fragments
        .push(RetainedFrameFragment::Transient(std::mem::take(current)));
}

fn push_composition_item(
    phase: CompositionPhase,
    item: CompositionItem,
    normal_items: &mut Vec<CompositionItem>,
    overlay_items: &mut Vec<CompositionItem>,
    effect_items: &mut Vec<CompositionItem>,
) {
    match phase {
        CompositionPhase::Normal => normal_items.push(item),
        CompositionPhase::Overlay => overlay_items.push(item),
        CompositionPhase::Effect => effect_items.push(item),
    }
}

fn flush_container_segment(
    effects: &HashMap<EffectNodeId, EffectNode>,
    container: CompositionContainerId,
    result: &mut RootSnapshot,
    normal_items: &mut Vec<CompositionItem>,
    overlay_items: &mut Vec<CompositionItem>,
    effect_items: &mut Vec<CompositionItem>,
    segment_scene: &mut Scene,
    segment_start: &mut Option<ResolvedRasterState>,
) {
    if !scene_has_draw_content(segment_scene) {
        *segment_scene = Scene::new();
        *segment_start = None;
        return;
    }

    let initial_state = segment_start
        .take()
        .expect("segment state available before flush");
    let phase = composition_phase_for_effect_node(initial_state.effect_node, effects);
    let packet_id = RetainedPacketId {
        container,
        segment_index: result.packets.len() as u32,
    };
    push_composition_item(
        phase,
        CompositionItem::Packet(packet_id),
        normal_items,
        overlay_items,
        effect_items,
    );
    result.packet_ids.push(packet_id);
    result.packets.push(PacketSnapshot {
        id: packet_id,
        scene: std::mem::take(segment_scene),
        initial_state,
    });
}

fn composition_phase_for_effect_node(
    mut effect_node: EffectNodeId,
    effects: &HashMap<EffectNodeId, EffectNode>,
) -> CompositionPhase {
    let mut phase = CompositionPhase::Normal;

    while let Some(node) = effects.get(&effect_node) {
        phase = match (phase, node.composition_mode) {
            (CompositionPhase::Effect, _) | (_, sui_scene::LayerCompositionMode::Effect) => {
                CompositionPhase::Effect
            }
            (CompositionPhase::Overlay, _) | (_, sui_scene::LayerCompositionMode::Overlay) => {
                CompositionPhase::Overlay
            }
            _ => phase,
        };

        let Some(parent) = node.parent else {
            break;
        };
        effect_node = parent;
    }

    phase
}

fn resolved_clip_primitives(
    mut clip_node: ClipNodeId,
    clips: &HashMap<ClipNodeId, ClipNode>,
) -> Vec<ResolvedClipPrimitive> {
    let mut primitives = Vec::new();
    while clip_node != ClipNodeId::ROOT {
        let Some(node) = clips.get(&clip_node) else {
            break;
        };
        if let Some(primitive) = &node.primitive {
            primitives.push(primitive.clone());
        }
        clip_node = node.parent.unwrap_or(ClipNodeId::ROOT);
    }
    primitives.reverse();
    primitives
}

impl RetainedCompositorState {
    fn reset_property_trees(&mut self) {
        self.transforms.clear();
        self.clips.clear();
        self.effects.clear();
        self.transforms.insert(
            TransformNodeId::ROOT,
            TransformNode {
                id: TransformNodeId::ROOT,
                parent: None,
                local: Transform::IDENTITY,
                world: Transform::IDENTITY,
            },
        );
        self.clips.insert(
            ClipNodeId::ROOT,
            ClipNode {
                id: ClipNodeId::ROOT,
                parent: None,
                primitive: None,
            },
        );
        self.effects.insert(
            EffectNodeId::ROOT,
            EffectNode {
                id: EffectNodeId::ROOT,
                parent: None,
                composition_mode: sui_scene::LayerCompositionMode::Normal,
            },
        );
        self.next_transform_node = 1;
        self.next_clip_node = 1;
        self.next_effect_node = 1;
    }

    fn push_transform_node(
        &mut self,
        parent: Option<TransformNodeId>,
        local: Transform,
        world: Transform,
    ) -> TransformNodeId {
        let id = TransformNodeId(self.next_transform_node);
        self.next_transform_node += 1;
        self.transforms.insert(
            id,
            TransformNode {
                id,
                parent,
                local,
                world,
            },
        );
        id
    }

    fn push_clip_node(
        &mut self,
        parent: Option<ClipNodeId>,
        primitive: ResolvedClipPrimitive,
    ) -> ClipNodeId {
        let id = ClipNodeId(self.next_clip_node);
        self.next_clip_node += 1;
        self.clips.insert(
            id,
            ClipNode {
                id,
                parent,
                primitive: Some(primitive),
            },
        );
        id
    }

    fn push_effect_node(
        &mut self,
        parent: Option<EffectNodeId>,
        composition_mode: sui_scene::LayerCompositionMode,
    ) -> EffectNodeId {
        let id = EffectNodeId(self.next_effect_node);
        self.next_effect_node += 1;
        self.effects.insert(
            id,
            EffectNode {
                id,
                parent,
                composition_mode,
            },
        );
        id
    }
}

fn scene_has_draw_content(scene: &Scene) -> bool {
    scene.commands().iter().any(|command| {
        matches!(
            command,
            SceneCommand::Clear(_)
                | SceneCommand::FillRect { .. }
                | SceneCommand::StrokeRect { .. }
                | SceneCommand::FillPath { .. }
                | SceneCommand::StrokePath { .. }
                | SceneCommand::DrawText(_)
                | SceneCommand::DrawShapedText(_)
                | SceneCommand::DrawImage { .. }
                | SceneCommand::DrawShaderRect { .. }
                | SceneCommand::Label { .. }
        )
    })
}

fn normalize_packet_snapshot(
    mut snapshot: PacketSnapshot,
    coordinate_space: PacketCoordinateSpace,
    normalization_origin: Vector,
) -> PacketSnapshot {
    if coordinate_space == PacketCoordinateSpace::LayerLocal && normalization_origin != Vector::ZERO
    {
        let delta = Vector::new(-normalization_origin.x, -normalization_origin.y);
        snapshot.scene.translate(delta);
        snapshot.initial_state = translate_resolved_raster_state(&snapshot.initial_state, delta);
    }
    if coordinate_space == PacketCoordinateSpace::LayerLocal {
        snapshot.initial_state.clip_stack.clear();
        snapshot.initial_state.clip_node = ClipNodeId::ROOT;
    }
    snapshot
}

fn packet_text_sample(scene: &Scene) -> Option<String> {
    for command in scene.commands() {
        let text = match command {
            SceneCommand::Label { text, .. } => Some(text.as_str()),
            SceneCommand::DrawText(run) => Some(run.text.as_str()),
            _ => None,
        };
        if let Some(text) = text {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let mut excerpt = trimmed.chars().take(64).collect::<String>();
                if trimmed.chars().count() > 64 {
                    excerpt.push_str("...");
                }
                return Some(excerpt);
            }
        }
    }
    None
}

fn translate_resolved_raster_state(
    state: &ResolvedRasterState,
    delta: Vector,
) -> ResolvedRasterState {
    let mut translated = state.clone();
    translated.clip_stack = translated
        .clip_stack
        .into_iter()
        .map(|clip| translate_resolved_clip_primitive(clip, delta))
        .collect();
    translated
}

fn translate_resolved_clip_primitive(
    primitive: ResolvedClipPrimitive,
    delta: Vector,
) -> ResolvedClipPrimitive {
    match primitive {
        ResolvedClipPrimitive::Rect(rect) => ResolvedClipPrimitive::Rect(rect.translate(delta)),
        ResolvedClipPrimitive::Path { path, bounds, .. } => {
            let translated_path = transform_scene_path(&path, Transform::translation_vector(delta));
            let translated_bounds = bounds.translate(delta);
            ResolvedClipPrimitive::Path {
                signature: hash_path(&translated_path, Transform::IDENTITY),
                path: translated_path,
                bounds: translated_bounds,
            }
        }
    }
}

fn descriptor_translation_delta(
    previous: &sui_scene::SceneLayerDescriptor,
    current: &sui_scene::SceneLayerDescriptor,
) -> Option<Vector> {
    if previous.id != current.id
        || previous.owner != current.owner
        || previous.composition_mode != current.composition_mode
        || previous.bounds.size != current.bounds.size
        || previous.content_bounds.size != current.content_bounds.size
        || previous.paint_bounds.size != current.paint_bounds.size
    {
        return None;
    }

    let bounds_delta = current.bounds.origin - previous.bounds.origin;
    let content_delta = current.content_bounds.origin - previous.content_bounds.origin;
    let paint_delta = current.paint_bounds.origin - previous.paint_bounds.origin;
    if bounds_delta != content_delta || bounds_delta != paint_delta {
        return None;
    }

    Some(bounds_delta + (current.properties.translation - previous.properties.translation))
}

fn packet_signature(
    scene: &Scene,
    initial_state: &ResolvedRasterState,
    viewport: Size,
    feather_width: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    scene.commands().len().hash(&mut hasher);
    hash_scene(scene, &mut hasher);
    initial_state.signature().hash(&mut hasher);
    viewport.width.to_bits().hash(&mut hasher);
    viewport.height.to_bits().hash(&mut hasher);
    feather_width.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn hash_scene(scene: &Scene, hasher: &mut DefaultHasher) {
    for command in scene.commands() {
        hash_scene_command(command, hasher);
    }
}

fn hash_scene_command(command: &SceneCommand, hasher: &mut DefaultHasher) {
    match command {
        SceneCommand::Clear(color) => {
            0u8.hash(hasher);
            hash_color(*color, hasher);
        }
        SceneCommand::FillRect { rect, brush } => {
            1u8.hash(hasher);
            hash_rect(hasher, *rect);
            hash_brush(brush, hasher);
        }
        SceneCommand::StrokeRect {
            rect,
            brush,
            stroke,
        } => {
            2u8.hash(hasher);
            hash_rect(hasher, *rect);
            hash_brush(brush, hasher);
            stroke.width.to_bits().hash(hasher);
        }
        SceneCommand::FillPath { path, brush } => {
            3u8.hash(hasher);
            hash_path(path, Transform::IDENTITY).hash(hasher);
            hash_brush(brush, hasher);
        }
        SceneCommand::StrokePath {
            path,
            brush,
            stroke,
        } => {
            4u8.hash(hasher);
            hash_path(path, Transform::IDENTITY).hash(hasher);
            hash_brush(brush, hasher);
            stroke.width.to_bits().hash(hasher);
        }
        SceneCommand::DrawText(text) => {
            5u8.hash(hasher);
            hash_rect(hasher, text.rect);
            text.text.hash(hasher);
            hash_text_style(&text.style, hasher);
        }
        SceneCommand::DrawShapedText(text) => {
            6u8.hash(hasher);
            hash_point(hasher, text.origin);
            text.layout_handle.get().hash(hasher);
            text.layout_version.get().hash(hasher);
            hash_rect(hasher, text.bounds);
        }
        SceneCommand::DrawShapedTextWindow(text) => {
            15u8.hash(hasher);
            hash_point(hasher, text.origin);
            text.layout_handle.get().hash(hasher);
            text.layout_version.get().hash(hasher);
            text.line_range.start.hash(hasher);
            text.line_range.end.hash(hasher);
            hash_rect(hasher, text.bounds);
        }
        SceneCommand::DrawImage { rect, source } => {
            7u8.hash(hasher);
            hash_rect(hasher, *rect);
            source.image.get().hash(hasher);
            source
                .source_rect
                .map(|rect| {
                    1u8.hash(hasher);
                    hash_rect(hasher, rect);
                })
                .unwrap_or_else(|| 0u8.hash(hasher));
            source
                .tint
                .map(|color| {
                    1u8.hash(hasher);
                    hash_color(color, hasher);
                })
                .unwrap_or_else(|| 0u8.hash(hasher));
        }
        SceneCommand::DrawShaderRect { rect, shader } => {
            16u8.hash(hasher);
            hash_rect(hasher, *rect);
            hash_widget_shader(shader, hasher);
        }
        SceneCommand::PushClip { rect } => {
            8u8.hash(hasher);
            hash_rect(hasher, *rect);
        }
        SceneCommand::PushClipPath { path } => {
            9u8.hash(hasher);
            hash_path(path, Transform::IDENTITY).hash(hasher);
        }
        SceneCommand::PopClip => {
            10u8.hash(hasher);
        }
        SceneCommand::PushTransform { transform } => {
            11u8.hash(hasher);
            hash_transform(hasher, *transform);
        }
        SceneCommand::PopTransform => {
            12u8.hash(hasher);
        }
        SceneCommand::Layer(layer) => {
            13u8.hash(hasher);
            layer.layer_id().get().hash(hasher);
        }
        SceneCommand::Label { rect, text, color } => {
            14u8.hash(hasher);
            hash_rect(hasher, *rect);
            text.hash(hasher);
            hash_color(*color, hasher);
        }
    }
}

fn hash_widget_shader(shader: &sui_scene::WidgetShader, hasher: &mut DefaultHasher) {
    match shader {
        sui_scene::WidgetShader::ColorWheel => {
            0u8.hash(hasher);
        }
        sui_scene::WidgetShader::ColorPickerHueBar => {
            1u8.hash(hasher);
        }
        sui_scene::WidgetShader::ColorPickerSaturationValuePlane {
            color_space,
            hue,
            max_value,
        } => {
            2u8.hash(hasher);
            hash_color_space(*color_space, hasher);
            hue.to_bits().hash(hasher);
            max_value.to_bits().hash(hasher);
        }
        sui_scene::WidgetShader::ColorPickerSaturationBar {
            color_space,
            hue,
            value,
        } => {
            3u8.hash(hasher);
            hash_color_space(*color_space, hasher);
            hue.to_bits().hash(hasher);
            value.to_bits().hash(hasher);
        }
        sui_scene::WidgetShader::ColorPickerValueBar {
            color_space,
            hue,
            saturation,
            max_value,
        } => {
            4u8.hash(hasher);
            hash_color_space(*color_space, hasher);
            hue.to_bits().hash(hasher);
            saturation.to_bits().hash(hasher);
            max_value.to_bits().hash(hasher);
        }
        sui_scene::WidgetShader::ColorPickerAlphaBar { color } => {
            5u8.hash(hasher);
            hash_color(*color, hasher);
            hash_color_space(color.space, hasher);
        }
        sui_scene::WidgetShader::ColorPickerRgbChannelBar {
            color,
            channel,
            max_value,
        } => {
            6u8.hash(hasher);
            hash_color(*color, hasher);
            hash_color_space(color.space, hasher);
            channel.hash(hasher);
            max_value.to_bits().hash(hasher);
        }
    }
}

fn hash_brush(brush: &Brush, hasher: &mut DefaultHasher) {
    match brush {
        Brush::Solid(color) => {
            0u8.hash(hasher);
            hash_color(*color, hasher);
        }
    }
}

fn hash_color(color: Color, hasher: &mut DefaultHasher) {
    color.red.to_bits().hash(hasher);
    color.green.to_bits().hash(hasher);
    color.blue.to_bits().hash(hasher);
    color.alpha.to_bits().hash(hasher);
}

fn hash_color_space(space: ColorSpace, hasher: &mut DefaultHasher) {
    (match space {
        ColorSpace::Srgb => 0u8,
        ColorSpace::LinearSrgb => 1u8,
        ColorSpace::DisplayP3 => 2u8,
        ColorSpace::LinearDisplayP3 => 3u8,
    })
    .hash(hasher);
}

fn hash_text_style(style: &TextStyle, hasher: &mut DefaultHasher) {
    style.font.map(|font| font.get()).hash(hasher);
    style.font_size.to_bits().hash(hasher);
    style.line_height.to_bits().hash(hasher);
    hash_color(style.color, hasher);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use sui_core::{Rect, Size, Vector, WidgetId, WindowId};
    use sui_scene::{ImageRegistry, SceneFrame, SceneLayerUpdate, SceneLayerUpdateKind};
    use sui_text::{FontRegistry, TextLayoutRegistry};

    fn build_layer_frame(
        descriptor: sui_scene::SceneLayerDescriptor,
        update_kind: SceneLayerUpdateKind,
    ) -> SceneFrame {
        let bounds = descriptor.bounds;
        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: bounds,
            brush: Color::rgba(0.82, 0.36, 0.18, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(24),
            viewport: Size::new(160.0, 80.0),
            surface_size: Size::new(160.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![SceneLayerUpdate::from_descriptor(update_kind, descriptor)],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    }

    fn layer_packet_signature(compositor: &RetainedCompositorState, layer_id: WidgetId) -> u64 {
        let container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
        let packet = compositor
            .packets
            .values()
            .find(|packet| packet.id.container == container)
            .expect("retained packet for layer");
        packet_signature(
            &packet.scene,
            &packet.initial_state,
            compositor.viewport,
            f32::from_bits(compositor.feather_width_bits),
        )
    }

    #[test]
    fn translation_only_layer_updates_reuse_retained_content() {
        let layer_id = WidgetId::new(53);
        let descriptor = sui_scene::SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(8.0, 10.0, 80.0, 36.0),
        )
        .with_content_bounds(Rect::new(8.0, 10.0, 80.0, 36.0))
        .with_paint_bounds(Rect::new(8.0, 10.0, 80.0, 36.0));
        let mut frame = build_layer_frame(descriptor.clone(), SceneLayerUpdateKind::Content);

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = compositor
            .prepare_frame(&frame, &mut text_engine, DEFAULT_FEATHER_WIDTH)
            .unwrap();
        let first_signature = layer_packet_signature(&compositor, layer_id);
        let first_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        let translated = descriptor.with_translation(Vector::new(36.0, 0.0));
        frame = build_layer_frame(translated, SceneLayerUpdateKind::Transform);
        let second = compositor
            .prepare_frame(&frame, &mut text_engine, DEFAULT_FEATHER_WIDTH)
            .unwrap();
        let second_signature = layer_packet_signature(&compositor, layer_id);
        let second_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

        assert_eq!(first_signature, second_signature);
        assert_eq!(first_content_version, second_content_version);
        assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
        assert_eq!(compositor.last_frame_stats.direct_packets, 1);
        assert_ne!(first.scene_vertices, second.scene_vertices);
        assert_eq!(
            compositor.layers[&SceneLayerId::from_widget(layer_id)]
                .descriptor
                .properties
                .translation,
            Vector::new(36.0, 0.0)
        );
    }

    #[test]
    fn opacity_only_layer_updates_reuse_retained_content() {
        let layer_id = WidgetId::new(54);
        let descriptor = sui_scene::SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(8.0, 10.0, 80.0, 36.0),
        )
        .with_content_bounds(Rect::new(8.0, 10.0, 80.0, 36.0))
        .with_paint_bounds(Rect::new(8.0, 10.0, 80.0, 36.0));
        let mut frame = build_layer_frame(descriptor.clone(), SceneLayerUpdateKind::Content);

        let mut text_engine = TextEngine::new().unwrap();
        let mut compositor = RetainedCompositorState::default();
        let first = compositor
            .prepare_frame(&frame, &mut text_engine, DEFAULT_FEATHER_WIDTH)
            .unwrap();
        let first_signature = layer_packet_signature(&compositor, layer_id);
        let first_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;
        let first_alpha = first.scene_vertices[0].color[3];

        let faded = descriptor.with_opacity(0.5);
        frame = build_layer_frame(faded, SceneLayerUpdateKind::Effect);
        let second = compositor
            .prepare_frame(&frame, &mut text_engine, DEFAULT_FEATHER_WIDTH)
            .unwrap();
        let second_signature = layer_packet_signature(&compositor, layer_id);
        let second_content_version =
            compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;
        let second_alpha = second.scene_vertices[0].color[3];

        assert_eq!(first_signature, second_signature);
        assert_eq!(first_content_version, second_content_version);
        assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
        assert_eq!(compositor.last_frame_stats.direct_packets, 1);
        assert!(second_alpha < first_alpha);
        assert_eq!(second_alpha, first_alpha * 0.5);
        assert_eq!(
            compositor.layers[&SceneLayerId::from_widget(layer_id)]
                .descriptor
                .properties
                .opacity,
            0.5
        );
    }
}
