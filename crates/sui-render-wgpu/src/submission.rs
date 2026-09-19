use crate::WgpuRenderer;
use crate::diagnostics::RendererFrameStats;
use crate::draw::CachedDrawBatch;
use crate::draw::CachedPassBatch;
#[cfg(test)]
use crate::draw::ClipState;
use crate::draw::DrawOp;
use crate::draw::DrawOpArena;
use crate::draw::DrawOpKind;
use crate::draw::EncodablePassBatch;
use crate::draw::ImageBindGroupKey;
use crate::draw::PreparedAnalyticPathResources;
use crate::draw::PreparedClipPath;
use crate::draw::PreparedDrawBatch;
use crate::draw::PreparedDrawKind;
use crate::draw::PreparedDrawPipelineKind;
use crate::draw::PreparedFragmentSubmission;
use crate::draw::PreparedFrameBatches;
use crate::draw::PreparedPassBatch;
use crate::draw::PreparedSceneSubmission;
use crate::draw::PreparedVertices;
use crate::draw::ScissorRect;
use crate::draw::resolve_submission_clip_rect;
use crate::geometry::AnalyticPathCpuData;
use crate::gpu::AnalyticQuadInstance;
use crate::gpu::CompactVertex;
use crate::gpu::ExtendedQuadInstance;
use crate::gpu::SharedRenderer;
use crate::gpu::SolidVertex;
use crate::gpu::TextAtlasInstance;
#[cfg(test)]
use crate::gpu::Vertex;
use crate::retained::RetainedFrameFragment;
use crate::scene::translation_to_viewport_origin;
use crate::surface::normalize_framebuffer_size;
use crate::text::TextFrameStats;
use crate::text_engine::TextEngine;
#[cfg(test)]
use crate::text_engine::pack_unorm16;
#[cfg(test)]
#[cfg(test)]
use crate::text_engine::unpack_unorm16;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use sui_core::Error;
use sui_core::Rect;
use sui_core::Result;
use sui_core::Size;
use sui_core::Vector;
use sui_scene::SceneFrame;
use web_time::Instant;

impl WgpuRenderer {
    pub(crate) fn prepare_scene_submission(
        &mut self,
        frame: &SceneFrame,
    ) -> Result<PreparedSceneSubmission> {
        // Drive completed staging-buffer map callbacks without waiting for GPU
        // work, so the upload belt can reuse its storage on this frame.
        self.shared
            .as_ref()
            .expect("renderer shared state initialized")
            .device
            .poll(wgpu::PollType::Poll)
            .map_err(|error| Error::new(format!("failed to poll frame uploads: {error}")))?;
        let diagnostics_enabled = self.runtime_diagnostics_enabled;
        let feather_width = self.active_feather_width();
        let text_render_mode = self.text_render_mode();
        let text_subpixel_order = self.active_text_subpixel_order();
        let text_hinting = self.active_text_hinting();
        let stem_darkening = self.active_stem_darkening();
        let text_coverage_policy = self.active_text_coverage_policy();
        if self.text_engine.is_none() {
            self.text_engine = Some(TextEngine::new()?);
        }
        // The multi-page atlas grows and evicts on demand, so a glyph that cannot be placed is
        // simply dropped for the frame -- there is no longer an "atlas full" error to recover
        // from, hence no retry loop.
        let (submission, compositor_stats, text_frame_stats) = {
            let text_engine = self
                .text_engine
                .as_mut()
                .expect("text engine initialized before draw-op construction");
            text_engine.set_text_render_mode(text_render_mode);
            text_engine.set_text_subpixel_order(text_subpixel_order);
            text_engine.set_text_hinting(text_hinting);
            text_engine.set_stem_darkening(stem_darkening);
            text_engine.set_text_coverage_policy(text_coverage_policy);
            text_engine.set_diagnostics_enabled(diagnostics_enabled);
            text_engine.begin_frame();
            let compositor = self.compositors.entry(frame.window_id).or_default();
            compositor.set_diagnostics_enabled(diagnostics_enabled);
            let submission =
                compositor.prepare_frame_submission(frame, text_engine, feather_width)?;
            let compositor_stats = compositor.last_frame_stats.clone();
            let text_frame_stats = if diagnostics_enabled {
                text_engine.frame_stats()
            } else {
                TextFrameStats::default()
            };
            (submission, compositor_stats, text_frame_stats)
        };
        let framebuffer_size = normalize_framebuffer_size(frame.surface_size).unwrap_or((1, 1));
        let mut analytic_paths = HashMap::new();
        let mut image_resources = HashSet::new();
        let mut uses_text_atlas = false;
        let resource_collection_started = diagnostics_enabled.then(Instant::now);
        for fragment in &submission.fragments {
            let RetainedFrameFragment::Transient(draw_ops) = fragment;
            uses_text_atlas |=
                collect_draw_op_resources(draw_ops, &mut analytic_paths, &mut image_resources);
        }
        let resource_collection_time_us = resource_collection_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let bind_group_prepare_started = diagnostics_enabled.then(Instant::now);
        let (analytic_path_resources, analytic_path_stats) =
            self.prepare_analytic_path_resources(analytic_paths, diagnostics_enabled)?;
        let analytic_path_bind_group_time_us = analytic_path_stats.total_time_us;
        let analytic_path_bind_group_miss_count = analytic_path_stats.miss_count;
        let analytic_path_bind_group_upload_bytes = analytic_path_stats.upload_bytes;

        let image_bind_group_started = diagnostics_enabled.then(Instant::now);
        let mut image_bind_groups = HashMap::new();
        for key in image_resources {
            let bind_group = if let Some(image) = frame.image_registry.get_external(key.handle) {
                self.ensure_external_image_bind_group(key.handle, key.sampling, *image)?
            } else {
                let image = frame.image_registry.get(key.handle).ok_or_else(|| {
                    Error::new(format!(
                        "image handle {} is not registered",
                        key.handle.get()
                    ))
                })?;
                self.ensure_image_bind_group(key, image)?
            };
            image_bind_groups.insert(key, bind_group);
        }
        let image_bind_group_time_us = image_bind_group_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        let mut text_atlas_bind_group_time_us = 0u64;
        let mut text_atlas_upload_copy_time_us = 0u64;
        let mut text_atlas_upload_write_time_us = 0u64;
        let mut text_atlas_upload_bytes = 0u64;
        let text_atlas_bind_group = if uses_text_atlas {
            let mut text_engine = self
                .text_engine
                .take()
                .expect("text engine initialized before text atlas upload");
            let (bind_group, stats) =
                self.ensure_text_atlas_bind_group(&mut text_engine, diagnostics_enabled)?;
            self.text_engine = Some(text_engine);
            text_atlas_bind_group_time_us = stats.total_time_us;
            text_atlas_upload_copy_time_us = stats.upload_copy_time_us;
            text_atlas_upload_write_time_us = stats.upload_write_time_us;
            text_atlas_upload_bytes = stats.upload_bytes;
            Some(bind_group)
        } else {
            None
        };
        let bind_group_prepare_time_us = bind_group_prepare_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        let mut prepared_fragments = Vec::new();
        let mut draw_count = 0usize;
        let upload_bytes_before = self.frame_resources.uploads.bytes_written;
        let mut needs_stencil = false;
        let mut batch_prepare_time_us = 0u64;
        let mut gpu_upload_time_us = 0u64;

        let buffers = self
            .frame_resources
            .fragments
            .entry(frame.window_id)
            .or_default();
        buffers.resize_with(submission.fragments.len(), Default::default);
        for (fragment_index, fragment) in submission.fragments.into_iter().enumerate() {
            let RetainedFrameFragment::Transient(draw_ops) = fragment;
            let batch_prepare_started = diagnostics_enabled.then(Instant::now);
            let prepared = prepare_frame_batches_with_analytic_slots(
                draw_ops,
                frame.viewport,
                framebuffer_size,
                analytic_path_resources
                    .as_ref()
                    .map(|resources| &resources.slots),
            );
            if let Some(started) = batch_prepare_started {
                batch_prepare_time_us += started.elapsed().as_micros() as u64;
            }
            if diagnostics_enabled {
                let (_, fragment_draw_count) = prepared_batch_counts(&prepared.passes);
                draw_count += fragment_draw_count;
            }

            if prepared.passes.is_empty() {
                continue;
            }

            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            needs_stencil |= prepared
                .passes
                .iter()
                .any(|pass| !pass.clip_paths.is_empty());
            let gpu_upload_started = diagnostics_enabled.then(Instant::now);
            let buffers = &mut self
                .frame_resources
                .fragments
                .get_mut(&frame.window_id)
                .expect("frame buffers allocated")[fragment_index];
            let uploads = &mut self.frame_resources.uploads;
            prepared_fragments.push(PreparedFragmentSubmission {
                passes: prepared.passes,
                solid_buffer: buffers.solid.upload(
                    &shared.device,
                    uploads,
                    "SUI transient fragment solid vertices",
                    &prepared.solid_vertices,
                ),
                scene_buffer: buffers.scene.upload(
                    &shared.device,
                    uploads,
                    "SUI transient fragment scene",
                    &prepared.scene_vertices,
                ),
                analytic_buffer: buffers.analytic.upload(
                    &shared.device,
                    uploads,
                    "SUI transient fragment analytic instances",
                    &prepared.analytic_vertices,
                ),
                extended_buffer: buffers.extended.upload(
                    &shared.device,
                    uploads,
                    "SUI transient fragment extended vertices",
                    &prepared.extended_vertices,
                ),
                clip_buffer: buffers.clip.upload(
                    &shared.device,
                    uploads,
                    "SUI transient fragment clip",
                    &prepared.clip_vertices,
                ),
                text_instance_buffer: buffers.text.upload(
                    &shared.device,
                    uploads,
                    "SUI transient fragment text instances",
                    &prepared.text_instances,
                ),
                translation: Vector::ZERO,
            });
            if let Some(started) = gpu_upload_started {
                gpu_upload_time_us += started.elapsed().as_micros() as u64;
            }
        }

        if needs_stencil {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            let gpu_upload_started = diagnostics_enabled.then(Instant::now);
            self.frame_resources
                .ensure_stencil(&shared.device, framebuffer_size);
            if let Some(started) = gpu_upload_started {
                gpu_upload_time_us += started.elapsed().as_micros() as u64;
            }
        }

        let batch_prepare_started = diagnostics_enabled.then(Instant::now);
        let encodable_passes = flatten_fragment_passes(&prepared_fragments);
        if let Some(started) = batch_prepare_started {
            batch_prepare_time_us += started.elapsed().as_micros() as u64;
        }
        let uploaded_vertex_bytes = self
            .frame_resources
            .uploads
            .bytes_written
            .wrapping_sub(upload_bytes_before);
        let mut frame_stats = if diagnostics_enabled {
            RendererFrameStats::from_prepared_counts(0, draw_count, uploaded_vertex_bytes)
                .with_text_stats(text_frame_stats)
                .with_compositor_stats(compositor_stats)
        } else {
            RendererFrameStats::default()
        };
        frame_stats.resource_collection_time_us = resource_collection_time_us;
        frame_stats.bind_group_prepare_time_us = bind_group_prepare_time_us;
        frame_stats.image_bind_group_time_us = image_bind_group_time_us;
        frame_stats.analytic_path_bind_group_time_us = analytic_path_bind_group_time_us;
        frame_stats.analytic_path_bind_group_miss_count = analytic_path_bind_group_miss_count;
        frame_stats.analytic_path_bind_group_upload_bytes = analytic_path_bind_group_upload_bytes;
        frame_stats.text_atlas_bind_group_time_us = text_atlas_bind_group_time_us;
        frame_stats.text_atlas_upload_copy_time_us = text_atlas_upload_copy_time_us;
        frame_stats.text_atlas_upload_write_time_us = text_atlas_upload_write_time_us;
        frame_stats.text_atlas_upload_bytes = text_atlas_upload_bytes;
        frame_stats.batch_prepare_time_us = batch_prepare_time_us;
        frame_stats.gpu_upload_time_us = gpu_upload_time_us;
        Ok(PreparedSceneSubmission {
            viewport: frame.viewport,
            framebuffer_size,
            encodable_passes,
            image_bind_groups,
            text_atlas_bind_group,
            analytic_path_resources,
            frame_stats,
        })
    }

    pub(crate) fn submit_prepared_scene(
        &mut self,
        prepared: PreparedSceneSubmission,
        target_format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
    ) -> Result<RendererFrameStats> {
        let mut encoder = {
            let shared = self
                .shared
                .as_ref()
                .expect("renderer shared state initialized");
            shared
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SUI scene encoder"),
                })
        };

        let pass_encode_started = self.runtime_diagnostics_enabled.then(Instant::now);
        let pass_count = if prepared.encodable_passes.is_empty() {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SUI scene clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            1
        } else {
            let shared = self
                .shared
                .as_mut()
                .expect("renderer shared state initialized");
            let stencil_view = self.frame_resources.stencil.as_ref().map(|target| {
                let _ = &target.texture;
                &target.view
            });
            encode_fragment_passes(
                shared,
                &mut encoder,
                view,
                target_format,
                prepared.viewport,
                prepared.framebuffer_size,
                &prepared.encodable_passes,
                stencil_view,
                &prepared.image_bind_groups,
                prepared.text_atlas_bind_group.as_ref(),
                prepared.analytic_path_resources.as_ref(),
            )?
        };
        let pass_encode_time_us = pass_encode_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let queue_submit_started = self.runtime_diagnostics_enabled.then(Instant::now);
        let uploads = self.frame_resources.uploads.finish();
        self.shared
            .as_ref()
            .expect("renderer shared state initialized")
            .queue
            .submit(uploads.into_iter().chain(std::iter::once(encoder.finish())));
        let queue_submit_time_us = queue_submit_started
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let mut frame_stats = prepared.frame_stats;
        frame_stats.pass_encode_time_us = pass_encode_time_us;
        frame_stats.queue_submit_time_us = queue_submit_time_us;
        frame_stats.pass_count = pass_count.max(1);
        Ok(frame_stats)
    }
}

#[cfg(test)]
pub(crate) fn prepare_frame_batches(
    draw_ops: DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> PreparedFrameBatches {
    prepare_frame_batches_with_analytic_slots(draw_ops, viewport, framebuffer_size, None)
}

pub(crate) fn prepare_frame_batches_with_analytic_slots(
    mut draw_ops: DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
    analytic_path_slots: Option<&HashMap<u64, u32>>,
) -> PreparedFrameBatches {
    let batch_analytic_paths = analytic_path_slots.is_some();
    if let Some(slots) = analytic_path_slots {
        stamp_draw_op_analytic_path_slots(&mut draw_ops, slots);
    }
    let cached_passes = cache_draw_ops_internal(&draw_ops, batch_analytic_paths);
    let mut passes = prepare_cached_passes(
        &cached_passes,
        viewport,
        framebuffer_size,
        Vector::ZERO,
        None,
        0,
        0,
        0,
    );
    let mut solid_vertices = Vec::new();
    let mut scene_vertices = Vec::new();
    let mut analytic_vertices = Vec::new();
    let mut extended_vertices = Vec::new();
    for pass in &mut passes {
        for draw in &mut pass.draws {
            if draw.kind == PreparedDrawKind::TextAtlas {
                continue;
            }
            let source = &draw_ops.scene_vertices
                [draw.vertices.start as usize..(draw.vertices.start + draw.vertices.len) as usize];
            if draw.kind == PreparedDrawKind::Solid {
                draw.vertices.start = solid_vertices.len() as u32;
                solid_vertices.extend(source.iter().copied().map(SolidVertex::from));
            } else if matches!(draw.kind, PreparedDrawKind::AnalyticPath { .. }) {
                draw.vertices.start = analytic_vertices.len() as u32;
                let mut chunks = source.chunks_exact(6);
                analytic_vertices.extend(chunks.by_ref().map(|vertices| AnalyticQuadInstance {
                    ndc_min: vertices[0].position,
                    ndc_max: vertices[5].position,
                    scene_min: vertices[0].tex_coords,
                    scene_max: vertices[5].tex_coords,
                    color: vertices[0].color,
                    path_index: vertices[0].shader_params[0].round().max(0.0) as u32,
                }));
                assert!(
                    chunks.remainder().is_empty(),
                    "analytic batches must contain complete six-vertex quads"
                );
                draw.vertices.len /= 6;
            } else if draw.kind.uses_extended_vertices() {
                draw.vertices.start = extended_vertices.len() as u32;
                let mut chunks = source.chunks_exact(6);
                extended_vertices.extend(chunks.by_ref().map(|vertices| ExtendedQuadInstance {
                    ndc_min: vertices[0].position,
                    ndc_max: vertices[5].position,
                    local_min: vertices[0].tex_coords,
                    local_max: vertices[5].tex_coords,
                    color: vertices[0].color,
                    shader_params: vertices[0].shader_params,
                    shader_params2: vertices[0].shader_params2,
                    shader_params3: vertices[0].shader_params3,
                    shader_params4: vertices[0].shader_params4,
                }));
                assert!(
                    chunks.remainder().is_empty(),
                    "extended rectangle batches must contain complete six-vertex quads"
                );
                draw.vertices.len /= 6;
            } else {
                draw.vertices.start = scene_vertices.len() as u32;
                scene_vertices.extend(source.iter().copied().map(CompactVertex::from));
            }
        }
    }
    PreparedFrameBatches {
        solid_vertices,
        scene_vertices,
        analytic_vertices,
        extended_vertices,
        clip_vertices: draw_ops
            .clip_vertices
            .into_iter()
            .map(SolidVertex::from)
            .collect(),
        text_instances: draw_ops.text_instances,
        passes,
    }
}

#[cfg(test)]
pub(crate) fn batch_draw_ops(
    draw_ops: &DrawOpArena,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Vec<PreparedPassBatch> {
    let cached_passes = cache_draw_ops(draw_ops);
    prepare_cached_passes(
        &cached_passes,
        viewport,
        framebuffer_size,
        Vector::ZERO,
        None,
        0,
        0,
        0,
    )
}

#[cfg(test)]
pub(crate) fn cache_draw_ops(draw_ops: &DrawOpArena) -> Vec<CachedPassBatch> {
    cache_draw_ops_internal(draw_ops, false)
}

pub(crate) fn cache_draw_ops_internal(
    draw_ops: &DrawOpArena,
    batch_analytic_paths: bool,
) -> Vec<CachedPassBatch> {
    let mut passes = Vec::new();

    for op in &draw_ops.draw_ops {
        let share_pass = passes.last().is_some_and(|pass: &CachedPassBatch| {
            let op_clip = &draw_ops.clip_states[op.clip_state_index];
            pass.clip_paths.len() == op_clip.clip_paths.len()
                && pass
                    .clip_paths
                    .iter()
                    .zip(op_clip.clip_paths.iter())
                    .all(|(a, b)| a.vertices.start == b.start && a.vertices.len == b.len)
        });
        if !share_pass {
            let clip_state = &draw_ops.clip_states[op.clip_state_index];
            passes.push(CachedPassBatch {
                clip_paths: clip_state
                    .clip_paths
                    .iter()
                    .copied()
                    .map(|vertices| PreparedClipPath { vertices })
                    .collect(),
                draws: Vec::new(),
            });
        }

        let pass = passes
            .last_mut()
            .expect("cached pass created before draw insertion");
        let mut kind = prepared_draw_kind(draw_ops, op);
        if batch_analytic_paths && matches!(kind, PreparedDrawKind::AnalyticPath { .. }) {
            kind = PreparedDrawKind::AnalyticPath {
                resource_signature: 0,
            };
        }
        let clip_rect = op.clip_rect;
        if let Some(previous) = pass.draws.last_mut() {
            let previous_end = previous.vertices.start + previous.vertices.len;
            if previous.kind == kind
                && previous.clip_rect == clip_rect
                && previous_end == op.vertices.start
            {
                previous.vertices.len += op.vertices.len;
                continue;
            }
        }

        pass.draws.push(CachedDrawBatch {
            kind,
            clip_rect,
            vertices: op.vertices,
        });
    }

    passes
}

pub(crate) fn prepare_cached_passes(
    cached_passes: &[CachedPassBatch],
    viewport: Size,
    framebuffer_size: (u32, u32),
    translation: Vector,
    external_clip_rect: Option<Rect>,
    scene_vertex_offset: u32,
    clip_vertex_offset: u32,
    text_instance_offset: u32,
) -> Vec<PreparedPassBatch> {
    cached_passes
        .iter()
        .map(|pass| PreparedPassBatch {
            clip_paths: pass
                .clip_paths
                .iter()
                .copied()
                .map(|clip_path| PreparedClipPath {
                    vertices: clip_path.vertices.offset(clip_vertex_offset),
                })
                .collect(),
            draws: pass
                .draws
                .iter()
                .filter_map(|draw| {
                    let clip_rect = resolve_submission_clip_rect(
                        draw.clip_rect.map(|rect| rect.translate(translation)),
                        external_clip_rect,
                    )?;
                    let clip_rect = match clip_rect {
                        Some(rect) => {
                            if rect.is_empty() {
                                return None;
                            }
                            rect_to_scissor(rect, viewport, framebuffer_size)
                        }
                        None => None,
                    };
                    Some(PreparedDrawBatch {
                        kind: draw.kind,
                        clip_rect,
                        vertices: match draw.kind {
                            PreparedDrawKind::TextAtlas => {
                                draw.vertices.offset(text_instance_offset)
                            }
                            _ => draw.vertices.offset(scene_vertex_offset),
                        },
                    })
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn prepared_draw_kind(draw_ops: &DrawOpArena, op: &DrawOp) -> PreparedDrawKind {
    match op.kind {
        DrawOpKind::Solid => PreparedDrawKind::Solid,
        DrawOpKind::Image {
            handle,
            sampling,
            raster_size,
        } => PreparedDrawKind::Image {
            handle,
            sampling,
            raster_size,
        },
        DrawOpKind::TextAtlas => PreparedDrawKind::TextAtlas,
        DrawOpKind::AnalyticPath { id } => PreparedDrawKind::AnalyticPath {
            resource_signature: draw_ops.analytic_paths[&id].resource_signature,
        },
        DrawOpKind::WidgetShader => PreparedDrawKind::WidgetShader,
        DrawOpKind::RoundedRect => PreparedDrawKind::RoundedRect,
        DrawOpKind::GradientRect => PreparedDrawKind::GradientRect,
    }
}

pub(crate) fn collect_draw_op_resources(
    draw_ops: &DrawOpArena,
    analytic_paths: &mut HashMap<u64, Arc<AnalyticPathCpuData>>,
    image_resources: &mut HashSet<ImageBindGroupKey>,
) -> bool {
    let mut uses_text_atlas = false;
    for draw in &draw_ops.draw_ops {
        match draw.kind {
            DrawOpKind::Solid => {}
            DrawOpKind::Image {
                handle,
                sampling,
                raster_size,
            } => {
                image_resources.insert(ImageBindGroupKey {
                    handle,
                    sampling,
                    raster_size,
                });
            }
            DrawOpKind::TextAtlas => {
                uses_text_atlas = true;
            }
            DrawOpKind::AnalyticPath { id } => {
                let path = &draw_ops.analytic_paths[&id];
                analytic_paths
                    .entry(path.resource_signature)
                    .or_insert_with(|| path.clone());
            }
            DrawOpKind::WidgetShader => {}
            DrawOpKind::RoundedRect => {}
            DrawOpKind::GradientRect => {}
        }
    }
    uses_text_atlas
}

pub(crate) fn prepared_batch_counts(passes: &[PreparedPassBatch]) -> (usize, usize) {
    (
        passes.len(),
        passes
            .iter()
            .map(|pass| pass.clip_paths.len() + pass.draws.len())
            .sum(),
    )
}

pub(crate) fn stamp_draw_op_analytic_path_slots(
    draw_ops: &mut DrawOpArena,
    analytic_path_slots: &HashMap<u64, u32>,
) {
    for draw in &draw_ops.draw_ops {
        let DrawOpKind::AnalyticPath { id } = draw.kind else {
            continue;
        };
        let signature = draw_ops.analytic_paths[&id].resource_signature;
        let Some(slot) = analytic_path_slots.get(&signature).copied() else {
            continue;
        };
        let start = draw.vertices.start as usize;
        let end = start + draw.vertices.len as usize;
        for vertex in &mut draw_ops.scene_vertices[start..end] {
            vertex.shader_params[0] = slot as f32;
        }
    }
}

pub(crate) fn flatten_fragment_passes(
    fragments: &[PreparedFragmentSubmission],
) -> Vec<EncodablePassBatch> {
    let mut flattened = Vec::new();
    for fragment in fragments {
        for pass in &fragment.passes {
            flattened.push(EncodablePassBatch {
                pass: pass.clone(),
                solid_buffer: fragment.solid_buffer.clone(),
                scene_buffer: fragment.scene_buffer.clone(),
                analytic_buffer: fragment.analytic_buffer.clone(),
                extended_buffer: fragment.extended_buffer.clone(),
                clip_buffer: fragment.clip_buffer.clone(),
                text_instance_buffer: fragment.text_instance_buffer.clone(),
                translation: fragment.translation,
            });
        }
    }
    flattened
}

pub(crate) fn encode_fragment_passes(
    shared: &mut SharedRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    passes: &[EncodablePassBatch],
    stencil_view: Option<&wgpu::TextureView>,
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
) -> Result<usize> {
    let mut cleared = false;
    let mut index = 0;
    let mut render_pass_count = 0;

    while index < passes.len() {
        if passes[index].pass.clip_paths.is_empty() {
            let start = index;
            while index < passes.len() && passes[index].pass.clip_paths.is_empty() {
                index += 1;
            }
            encode_unclipped_pass_run(
                shared,
                encoder,
                view,
                target_format,
                viewport,
                framebuffer_size,
                &passes[start..index],
                image_bind_groups,
                text_atlas_bind_group,
                analytic_path_resources,
                &mut cleared,
            )?;
            render_pass_count += 1;
        } else {
            encode_clipped_pass(
                shared,
                encoder,
                view,
                target_format,
                viewport,
                framebuffer_size,
                &passes[index],
                stencil_view,
                image_bind_groups,
                text_atlas_bind_group,
                analytic_path_resources,
                &mut cleared,
            )?;
            render_pass_count += 1;
            index += 1;
        }
    }

    Ok(render_pass_count)
}

pub(crate) fn encode_unclipped_pass_run(
    shared: &mut SharedRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    passes: &[EncodablePassBatch],
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
    cleared: &mut bool,
) -> Result<()> {
    let load_op = next_pass_load_op(cleared);
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SUI scene unclipped batch pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });
    let mut current_kind = None;
    for batch in passes {
        encode_draws_for_pass(
            &mut render_pass,
            shared,
            target_format,
            viewport,
            framebuffer_size,
            &batch.pass,
            batch.solid_buffer.as_ref(),
            batch.scene_buffer.as_ref(),
            batch.analytic_buffer.as_ref(),
            batch.extended_buffer.as_ref(),
            batch.text_instance_buffer.as_ref(),
            batch.translation,
            false,
            image_bind_groups,
            text_atlas_bind_group,
            analytic_path_resources,
            &mut current_kind,
        )?;
    }

    Ok(())
}

pub(crate) fn encode_clipped_pass(
    shared: &mut SharedRenderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    batch: &EncodablePassBatch,
    stencil_view: Option<&wgpu::TextureView>,
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
    cleared: &mut bool,
) -> Result<()> {
    let load_op = next_pass_load_op(cleared);
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("SUI scene clipped batch pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: stencil_view.expect("stencil view available for path-clipped pass"),
            depth_ops: None,
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0),
                store: wgpu::StoreOp::Store,
            }),
        }),
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });

    let clip_pipeline = shared.clip_pipeline(target_format);
    render_pass.set_pipeline(clip_pipeline);
    render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1);
    let (viewport_x, viewport_y) =
        translation_to_viewport_origin(batch.translation, viewport, framebuffer_size);
    render_pass.set_viewport(
        viewport_x,
        viewport_y,
        framebuffer_size.0 as f32,
        framebuffer_size.1 as f32,
        0.0,
        1.0,
    );
    let clip_buffer = batch
        .clip_buffer
        .as_ref()
        .expect("clip buffer available for path-clipped pass");
    for (clip_index, clip_path) in batch.pass.clip_paths.iter().enumerate() {
        render_pass.set_stencil_reference(clip_index as u32);
        render_pass.set_vertex_buffer(
            0,
            solid_vertex_buffer_slice(clip_buffer, clip_path.vertices),
        );
        render_pass.draw(0..clip_path.vertices.len, 0..1);
    }

    let mut current_kind = None;
    encode_draws_for_pass(
        &mut render_pass,
        shared,
        target_format,
        viewport,
        framebuffer_size,
        &batch.pass,
        batch.solid_buffer.as_ref(),
        batch.scene_buffer.as_ref(),
        batch.analytic_buffer.as_ref(),
        batch.extended_buffer.as_ref(),
        batch.text_instance_buffer.as_ref(),
        batch.translation,
        true,
        image_bind_groups,
        text_atlas_bind_group,
        analytic_path_resources,
        &mut current_kind,
    )?;

    Ok(())
}

pub(crate) fn next_pass_load_op(cleared: &mut bool) -> wgpu::LoadOp<wgpu::Color> {
    if *cleared {
        wgpu::LoadOp::Load
    } else {
        *cleared = true;
        wgpu::LoadOp::Clear(wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        })
    }
}

pub(crate) fn encode_draws_for_pass(
    render_pass: &mut wgpu::RenderPass<'_>,
    shared: &mut SharedRenderer,
    target_format: wgpu::TextureFormat,
    viewport: Size,
    framebuffer_size: (u32, u32),
    pass: &PreparedPassBatch,
    solid_buffer: Option<&wgpu::Buffer>,
    scene_buffer: Option<&wgpu::Buffer>,
    analytic_buffer: Option<&wgpu::Buffer>,
    extended_buffer: Option<&wgpu::Buffer>,
    text_instance_buffer: Option<&wgpu::Buffer>,
    translation: Vector,
    clipped: bool,
    image_bind_groups: &HashMap<ImageBindGroupKey, wgpu::BindGroup>,
    text_atlas_bind_group: Option<&wgpu::BindGroup>,
    analytic_path_resources: Option<&PreparedAnalyticPathResources>,
    current_kind: &mut Option<PreparedDrawPipelineKind>,
) -> Result<()> {
    let (viewport_x, viewport_y) =
        translation_to_viewport_origin(translation, viewport, framebuffer_size);
    render_pass.set_viewport(
        viewport_x,
        viewport_y,
        framebuffer_size.0 as f32,
        framebuffer_size.1 as f32,
        0.0,
        1.0,
    );

    for draw in &pass.draws {
        match draw.clip_rect {
            Some(scissor) => {
                render_pass.set_scissor_rect(scissor.x, scissor.y, scissor.width, scissor.height)
            }
            None => render_pass.set_scissor_rect(0, 0, framebuffer_size.0, framebuffer_size.1),
        }

        let pipeline_kind = draw.kind.pipeline_kind();
        if *current_kind != Some(pipeline_kind) {
            let pipeline = match (pipeline_kind, clipped) {
                (PreparedDrawPipelineKind::Solid, true) => shared.clipped_pipeline(target_format),
                (PreparedDrawPipelineKind::Solid, false) => shared.pipeline(target_format),
                (PreparedDrawPipelineKind::Image, true) => {
                    shared.clipped_image_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::Image, false) => shared.image_pipeline(target_format),
                (PreparedDrawPipelineKind::TextAtlas, true) => {
                    shared.clipped_text_atlas_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::TextAtlas, false) => {
                    shared.text_atlas_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::AnalyticPath, true) => {
                    shared.clipped_analytic_path_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::AnalyticPath, false) => {
                    shared.analytic_path_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::WidgetShader, true) => {
                    shared.clipped_widget_shader_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::WidgetShader, false) => {
                    shared.widget_shader_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::RoundedRect, true) => {
                    shared.clipped_rounded_rect_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::RoundedRect, false) => {
                    shared.rounded_rect_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::GradientRect, true) => {
                    shared.clipped_gradient_rect_pipeline(target_format)
                }
                (PreparedDrawPipelineKind::GradientRect, false) => {
                    shared.gradient_rect_pipeline(target_format)
                }
            };
            render_pass.set_pipeline(pipeline);
            if pipeline_kind == PreparedDrawPipelineKind::AnalyticPath {
                let bind_group = &analytic_path_resources
                    .expect("analytic path resources prepared before retained render pass")
                    .bind_group;
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            *current_kind = Some(pipeline_kind);
        }

        if clipped {
            render_pass.set_stencil_reference(pass.clip_paths.len() as u32);
        }

        match draw.kind {
            PreparedDrawKind::Solid => {}
            PreparedDrawKind::Image {
                handle,
                sampling,
                raster_size,
            } => {
                let bind_group = image_bind_groups
                    .get(&ImageBindGroupKey {
                        handle,
                        sampling,
                        raster_size,
                    })
                    .expect("image bind group prepared before retained render pass");
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            PreparedDrawKind::TextAtlas => {
                let bind_group = text_atlas_bind_group
                    .expect("text atlas bind group prepared before retained render pass");
                render_pass.set_bind_group(0, bind_group, &[]);
            }
            PreparedDrawKind::AnalyticPath { .. } => {}
            PreparedDrawKind::WidgetShader => {}
            PreparedDrawKind::RoundedRect => {}
            PreparedDrawKind::GradientRect => {}
        }

        let (vertex_range, instances) = match draw.kind {
            PreparedDrawKind::TextAtlas => {
                let text_instance_buffer = text_instance_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a text instance buffer")
                })?;
                render_pass.set_vertex_buffer(0, shared.text_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1,
                    text_instance_buffer_slice(text_instance_buffer, draw.vertices),
                );
                (0..6, 0..draw.vertices.len)
            }
            PreparedDrawKind::AnalyticPath { .. } => {
                let analytic_buffer = analytic_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing an analytic instance buffer")
                })?;
                render_pass.set_vertex_buffer(0, shared.text_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1,
                    analytic_vertex_buffer_slice(analytic_buffer, draw.vertices),
                );
                (0..6, 0..draw.vertices.len)
            }
            PreparedDrawKind::Solid => {
                let solid_buffer = solid_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a solid vertex buffer")
                })?;
                render_pass
                    .set_vertex_buffer(0, solid_vertex_buffer_slice(solid_buffer, draw.vertices));
                (0..draw.vertices.len, 0..1)
            }
            PreparedDrawKind::RoundedRect | PreparedDrawKind::GradientRect => {
                let extended_buffer = extended_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing an extended vertex buffer")
                })?;
                render_pass.set_vertex_buffer(0, shared.text_quad_buffer.slice(..));
                render_pass.set_vertex_buffer(
                    1,
                    extended_vertex_buffer_slice(extended_buffer, draw.vertices),
                );
                (0..6, 0..draw.vertices.len)
            }
            _ => {
                let scene_buffer = scene_buffer.ok_or_else(|| {
                    Error::new("prepared render batch is missing a scene vertex buffer")
                })?;
                render_pass
                    .set_vertex_buffer(0, compact_vertex_buffer_slice(scene_buffer, draw.vertices));
                (0..draw.vertices.len, 0..1)
            }
        };
        render_pass.draw(vertex_range, instances);
    }

    Ok(())
}

pub(crate) const COMPACT_VERTEX_SIZE: u64 = std::mem::size_of::<CompactVertex>() as u64;
pub(crate) const SOLID_VERTEX_SIZE: u64 = std::mem::size_of::<SolidVertex>() as u64;
pub(crate) const ANALYTIC_QUAD_INSTANCE_SIZE: u64 =
    std::mem::size_of::<AnalyticQuadInstance>() as u64;
pub(crate) const EXTENDED_QUAD_INSTANCE_SIZE: u64 =
    std::mem::size_of::<ExtendedQuadInstance>() as u64;
pub(crate) const TEXT_ATLAS_INSTANCE_SIZE: u64 = std::mem::size_of::<TextAtlasInstance>() as u64;

pub(crate) fn extended_vertex_buffer_slice(
    buffer: &wgpu::Buffer,
    vertices: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * EXTENDED_QUAD_INSTANCE_SIZE;
    let end = start + vertices.len as u64 * EXTENDED_QUAD_INSTANCE_SIZE;
    buffer.slice(start..end)
}

pub(crate) fn analytic_vertex_buffer_slice(
    buffer: &wgpu::Buffer,
    vertices: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * ANALYTIC_QUAD_INSTANCE_SIZE;
    let end = start + vertices.len as u64 * ANALYTIC_QUAD_INSTANCE_SIZE;
    buffer.slice(start..end)
}

pub(crate) fn compact_vertex_buffer_slice(
    buffer: &wgpu::Buffer,
    vertices: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * COMPACT_VERTEX_SIZE;
    let end = start + vertices.len as u64 * COMPACT_VERTEX_SIZE;
    buffer.slice(start..end)
}

pub(crate) fn solid_vertex_buffer_slice(
    buffer: &wgpu::Buffer,
    vertices: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = vertices.start as u64 * SOLID_VERTEX_SIZE;
    let end = start + vertices.len as u64 * SOLID_VERTEX_SIZE;
    buffer.slice(start..end)
}

pub(crate) fn text_instance_buffer_slice(
    buffer: &wgpu::Buffer,
    instances: PreparedVertices,
) -> wgpu::BufferSlice<'_> {
    let start = instances.start as u64 * TEXT_ATLAS_INSTANCE_SIZE;
    let end = start + instances.len as u64 * TEXT_ATLAS_INSTANCE_SIZE;
    buffer.slice(start..end)
}

pub(crate) fn rect_to_scissor(
    rect: Rect,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Option<ScissorRect> {
    if rect.is_empty() || viewport.is_empty() {
        return None;
    }

    let framebuffer_width = framebuffer_size.0.max(1);
    let framebuffer_height = framebuffer_size.1.max(1);
    let scale_x = framebuffer_width as f32 / viewport.width.max(1.0);
    let scale_y = framebuffer_height as f32 / viewport.height.max(1.0);

    let min_x = quantize_scissor_edge(rect.x().max(0.0) * scale_x, framebuffer_width);
    let min_y = quantize_scissor_edge(rect.y().max(0.0) * scale_y, framebuffer_height);
    let max_x = quantize_scissor_edge(
        (rect.x() + rect.width()).min(viewport.width) * scale_x,
        framebuffer_width,
    );
    let max_y = quantize_scissor_edge(
        (rect.y() + rect.height()).min(viewport.height) * scale_y,
        framebuffer_height,
    );

    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    let scissor = ScissorRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    if scissor.x == 0
        && scissor.y == 0
        && scissor.width == framebuffer_width
        && scissor.height == framebuffer_height
    {
        None
    } else {
        Some(scissor)
    }
}

pub(crate) fn quantize_scissor_edge(edge: f32, limit: u32) -> u32 {
    edge.round().clamp(0.0, limit as f32) as u32
}

#[cfg(test)]
pub(crate) mod transient_vertex_tests {
    use super::*;

    #[test]
    fn prepared_batches_compact_basic_vertices_and_instance_rectangle_quads() {
        let vertex = Vertex::basic([0.0; 2], [1.0; 4], [0.0; 2], [0.0; 4]);
        let draw_ops = DrawOpArena {
            scene_vertices: vec![vertex; 18],
            clip_states: vec![ClipState {
                clip_paths: Vec::new(),
            }],
            draw_ops: vec![
                DrawOp {
                    kind: DrawOpKind::Solid,
                    vertices: PreparedVertices { start: 0, len: 6 },
                    clip_rect: None,
                    clip_state_index: 0,
                    image: None,
                },
                DrawOp {
                    kind: DrawOpKind::RoundedRect,
                    vertices: PreparedVertices { start: 6, len: 6 },
                    clip_rect: None,
                    clip_state_index: 0,
                    image: None,
                },
                DrawOp {
                    kind: DrawOpKind::GradientRect,
                    vertices: PreparedVertices { start: 12, len: 6 },
                    clip_rect: None,
                    clip_state_index: 0,
                    image: None,
                },
            ],
            ..DrawOpArena::default()
        };

        let prepared = prepare_frame_batches(draw_ops, Size::new(100.0, 100.0), (100, 100));

        assert_eq!(std::mem::size_of::<CompactVertex>(), 48);
        assert_eq!(std::mem::size_of::<SolidVertex>(), 24);
        assert_eq!(std::mem::size_of::<AnalyticQuadInstance>(), 52);
        assert_eq!(std::mem::size_of::<ExtendedQuadInstance>(), 112);
        assert_eq!(std::mem::size_of::<TextAtlasInstance>(), 60);
        assert_eq!(prepared.solid_vertices.len(), 6);
        assert_eq!(prepared.scene_vertices.len(), 0);
        assert_eq!(prepared.analytic_vertices.len(), 0);
        assert_eq!(prepared.extended_vertices.len(), 2);
        assert_eq!(prepared.passes[0].draws[0].vertices.len, 6);
        assert_eq!(prepared.passes[0].draws[1].vertices.len, 1);
        assert_eq!(prepared.passes[0].draws[2].vertices.len, 1);
    }

    #[test]
    fn packed_text_uvs_preserve_normalized_coordinates() {
        for value in [0.0_f32, 0.125, 0.5, 0.875, 1.0] {
            let unpacked = unpack_unorm16(pack_unorm16(value));
            assert!((unpacked - value).abs() <= (1.0 / u16::MAX as f32));
        }
    }
}
