use crate::draw::DrawOpArena;
use crate::draw::DrawOpKind;
use crate::draw::ImageDrawMetadata;
use crate::draw::SceneRasterState;
use crate::draw::push_draw_op;
use crate::draw::push_image_draw_op;
use crate::draw::push_text_draw_op;
use crate::gpu::TextAtlasInstance;
use crate::gpu::Vertex;
use crate::path_cache::PathMeshCache;
use crate::paths::append_painted_path;
use crate::paths::append_stroked_path;
use crate::primitives::append_gradient_rect;
use crate::primitives::append_image;
use crate::primitives::append_image_quad;
use crate::primitives::append_rect;
use crate::primitives::append_rounded_rect_fill;
use crate::primitives::append_rounded_rect_shadow;
use crate::primitives::append_widget_shader_rect;
use crate::primitives::brush_fallback_color;
use crate::primitives::image_quad_raster_metadata;
use crate::primitives::image_rect_raster_metadata;
#[cfg(test)]
use crate::resources::DEFAULT_FEATHER_WIDTH;
use crate::retained::ResolvedRasterState;
#[cfg(test)]
use crate::retained::RetainedCompositorState;
use crate::text_engine::TextEngine;
#[cfg(test)]
#[cfg(test)]
use crate::text_engine::append_text_instance_vertices;
use sui_core::Color;
use sui_core::Error;
use sui_core::Rect;
use sui_core::Result;
use sui_core::Size;
use sui_core::Vector;
use sui_scene::Brush;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_text::TextRun;
use sui_text::TextStyle;
use web_time::Instant;

#[cfg(test)]
pub(crate) fn build_vertices(
    frame: &SceneFrame,
    text_engine: &mut TextEngine,
) -> Result<Vec<Vertex>> {
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = compositor.prepare_frame(frame, text_engine, DEFAULT_FEATHER_WIDTH)?;
    let mut vertices = Vec::new();
    for op in &draw_ops.draw_ops {
        match op.kind {
            DrawOpKind::TextAtlas => {
                append_text_instance_vertices(&mut vertices, draw_ops.text_instances(op.vertices));
            }
            _ => vertices.extend_from_slice(draw_ops.scene_vertices(op.vertices)),
        }
    }
    Ok(vertices)
}

pub(crate) fn translation_to_viewport_origin(
    translation: Vector,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> (f32, f32) {
    if translation == Vector::ZERO || viewport.is_empty() {
        return (0.0, 0.0);
    }

    let scale_x = framebuffer_size.0 as f32 / viewport.width.max(1.0);
    let scale_y = framebuffer_size.1 as f32 / viewport.height.max(1.0);
    (translation.x * scale_x, translation.y * scale_y)
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DirectPacketBuildDiagnostics {
    pub(crate) command_count: usize,
    pub(crate) text_command_count: usize,
    pub(crate) path_command_count: usize,
    pub(crate) clip_path_command_count: usize,
    pub(crate) image_command_count: usize,
    pub(crate) rect_command_count: usize,
    pub(crate) raster_state_init_time_ms: f64,
    pub(crate) scene_build_time_ms: f64,
    pub(crate) text_command_time_ms: f64,
    pub(crate) path_command_time_ms: f64,
    pub(crate) clip_path_command_time_ms: f64,
    pub(crate) image_command_time_ms: f64,
    pub(crate) rect_command_time_ms: f64,
}

pub(crate) fn build_direct_packet_with_diagnostics(
    frame: &SceneFrame,
    scene: &Scene,
    initial_state: &ResolvedRasterState,
    text_engine: &mut TextEngine,
    path_cache: &mut PathMeshCache,
    feather_width: f32,
) -> Result<(DrawOpArena, DirectPacketBuildDiagnostics)> {
    let mut diagnostics = DirectPacketBuildDiagnostics::default();
    let mut draw_ops = DrawOpArena::default();
    let state_init_started = Instant::now();
    let mut state = SceneRasterState::from_resolved(initial_state, &mut draw_ops, frame.viewport)?;
    diagnostics.raster_state_init_time_ms = state_init_started.elapsed().as_secs_f64() * 1000.0;
    let mut builder = SceneDrawOpBuilder {
        frame,
        text_engine,
        path_cache,
        feather_width,
        scratch_vertices: Vec::new(),
        scratch_text_instances: Vec::new(),
        overlay_scratch_vertices: Vec::new(),
        clip_scratch_vertices: Vec::new(),
    };
    let scene_build_started = Instant::now();
    builder.build_scene(scene, &mut draw_ops, &mut state, &mut diagnostics)?;
    diagnostics.scene_build_time_ms = scene_build_started.elapsed().as_secs_f64() * 1000.0;
    Ok((draw_ops, diagnostics))
}

pub(crate) struct SceneDrawOpBuilder<'a> {
    pub(crate) frame: &'a SceneFrame,
    pub(crate) text_engine: &'a mut TextEngine,
    pub(crate) path_cache: &'a mut PathMeshCache,
    pub(crate) feather_width: f32,
    pub(crate) scratch_vertices: Vec<Vertex>,
    pub(crate) scratch_text_instances: Vec<TextAtlasInstance>,
    pub(crate) overlay_scratch_vertices: Vec<Vertex>,
    pub(crate) clip_scratch_vertices: Vec<Vertex>,
}

pub(crate) enum FillPathRenderMode {
    SolidOnly,
    SolidPlusAnalytic { id: u64 },
}

pub(crate) const ANALYTIC_AA_OUTSET: f32 = 1.0;
pub(crate) const ANALYTIC_PATH_COORDINATE_PRECISION: f32 = 1024.0;

pub(crate) fn analytic_coverage_outset(soft_width: f32) -> f32 {
    (soft_width.max(0.0) * 0.5).max(ANALYTIC_AA_OUTSET)
}

impl SceneDrawOpBuilder<'_> {
    pub(crate) fn build_scene(
        &mut self,
        scene: &Scene,
        draw_ops: &mut DrawOpArena,
        state: &mut SceneRasterState,
        diagnostics: &mut DirectPacketBuildDiagnostics,
    ) -> Result<()> {
        for command in scene.commands() {
            self.build_command(command, draw_ops, state, diagnostics)?;
        }

        Ok(())
    }

    pub(crate) fn build_command(
        &mut self,
        command: &SceneCommand,
        draw_ops: &mut DrawOpArena,
        state: &mut SceneRasterState,
        diagnostics: &mut DirectPacketBuildDiagnostics,
    ) -> Result<()> {
        let viewport = self.frame.viewport;
        diagnostics.command_count += 1;
        let command_started = Instant::now();
        let clip = state
            .clip_stack
            .iter()
            .map(|clip| clip.bounds())
            .reduce(|a, b| a.intersection(b).unwrap_or(Rect::ZERO));
        let rectangular_clip = state
            .clip_stack
            .iter()
            .all(|clip| matches!(clip, crate::draw::ClipPrimitive::Rect(_)));
        state
            .text_background
            .observe(command, state.current_transform, clip, rectangular_clip);

        let result = match command {
            SceneCommand::Clear(color) => {
                self.scratch_vertices.clear();
                append_rect(
                    &mut self.scratch_vertices,
                    Rect::new(0.0, 0.0, viewport.width, viewport.height),
                    *color,
                    viewport,
                );
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::FillRect { rect, brush } => {
                self.scratch_vertices.clear();
                match brush {
                    Brush::Solid(color) => {
                        append_rounded_rect_fill(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            [0.0; 4],
                            *color,
                            None,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::RoundedRect,
                            &self.scratch_vertices,
                            state,
                        );
                    }
                    Brush::LinearGradient { start, end, stops } => {
                        let stop0 = stops.first().map(|s| s.color).unwrap_or(Color::TRANSPARENT);
                        let stop1 = stops.last().map(|s| s.color).unwrap_or(stop0);
                        append_gradient_rect(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            [0.0; 4],
                            *start,
                            *end,
                            stop0,
                            stop1,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::GradientRect,
                            &self.scratch_vertices,
                            state,
                        );
                    }
                }
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::StrokeRect {
                rect,
                brush,
                stroke,
            } => {
                let color = brush_fallback_color(brush);
                self.scratch_vertices.clear();
                append_rounded_rect_fill(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    [0.0; 4],
                    Color::TRANSPARENT,
                    Some(sui_scene::Border {
                        width: stroke.width,
                        color,
                    }),
                    viewport,
                    self.feather_width,
                );
                push_draw_op(
                    draw_ops,
                    DrawOpKind::RoundedRect,
                    &self.scratch_vertices,
                    state,
                );
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::FillPath { path, brush } => {
                let color = brush_fallback_color(brush);
                self.scratch_vertices.clear();
                self.overlay_scratch_vertices.clear();
                let render_mode = append_painted_path(
                    &mut self.scratch_vertices,
                    &mut self.overlay_scratch_vertices,
                    draw_ops,
                    state,
                    path,
                    color,
                    self.path_cache,
                    viewport,
                    self.feather_width,
                )?;
                push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                if let FillPathRenderMode::SolidPlusAnalytic { id } = render_mode {
                    push_draw_op(
                        draw_ops,
                        DrawOpKind::AnalyticPath { id },
                        &self.overlay_scratch_vertices,
                        state,
                    );
                }
                diagnostics.path_command_count += 1;
                Ok(())
            }
            SceneCommand::StrokePath {
                path,
                brush,
                stroke,
            } => {
                let color = brush_fallback_color(brush);
                self.scratch_vertices.clear();
                self.overlay_scratch_vertices.clear();
                let analytic_id = append_stroked_path(
                    &mut self.scratch_vertices,
                    &mut self.overlay_scratch_vertices,
                    draw_ops,
                    state,
                    path,
                    color,
                    *stroke,
                    self.path_cache,
                    viewport,
                    self.feather_width,
                )?;
                if !self.scratch_vertices.is_empty() {
                    push_draw_op(draw_ops, DrawOpKind::Solid, &self.scratch_vertices, state);
                }
                if let Some(id) = analytic_id {
                    push_draw_op(
                        draw_ops,
                        DrawOpKind::AnalyticPath { id },
                        &self.overlay_scratch_vertices,
                        state,
                    );
                }
                diagnostics.path_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawText(text) => {
                self.scratch_text_instances.clear();
                self.text_engine.append_text_run(
                    &mut self.scratch_text_instances,
                    state,
                    text,
                    self.frame.font_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawShapedText(text) => {
                self.scratch_text_instances.clear();
                self.text_engine.append_shaped_text(
                    &mut self.scratch_text_instances,
                    state,
                    text,
                    self.frame.text_layout_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawShapedTextWindow(text) => {
                self.scratch_text_instances.clear();
                self.text_engine.append_shaped_text_window(
                    &mut self.scratch_text_instances,
                    state,
                    text,
                    self.frame.text_layout_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawImage { rect, source } => {
                self.scratch_vertices.clear();
                let image_size = self
                    .frame
                    .image_registry
                    .dimensions(source.image)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "image handle {} is not registered",
                            source.image.get()
                        ))
                    })?;
                append_image(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    source,
                    image_size,
                    viewport,
                );
                let (bounds, raster_size) = image_rect_raster_metadata(
                    state.current_transform,
                    *rect,
                    source.source_rect,
                    image_size,
                    viewport,
                    self.frame.surface_size,
                );
                push_image_draw_op(
                    draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                        sampling: source.sampling,
                        raster_size,
                    },
                    &self.scratch_vertices,
                    state,
                    ImageDrawMetadata {
                        bounds,
                        pixel_snap: if state.current_transform.xy.abs() < 0.0001
                            && state.current_transform.yx.abs() < 0.0001
                            && state.current_transform.xx >= 0.0
                            && state.current_transform.yy >= 0.0
                        {
                            source.pixel_snap
                        } else {
                            sui_scene::ImagePixelSnap::None
                        },
                    },
                );
                diagnostics.image_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawImageQuad { points, source } => {
                self.scratch_vertices.clear();
                let image_size = self
                    .frame
                    .image_registry
                    .dimensions(source.image)
                    .ok_or_else(|| {
                        Error::new(format!(
                            "image handle {} is not registered",
                            source.image.get()
                        ))
                    })?;
                append_image_quad(
                    &mut self.scratch_vertices,
                    state,
                    *points,
                    source,
                    image_size,
                    viewport,
                );
                let (bounds, raster_size) = image_quad_raster_metadata(
                    state.current_transform,
                    *points,
                    source.source_rect,
                    image_size,
                    viewport,
                    self.frame.surface_size,
                );
                push_image_draw_op(
                    draw_ops,
                    DrawOpKind::Image {
                        handle: source.image,
                        sampling: source.sampling,
                        raster_size,
                    },
                    &self.scratch_vertices,
                    state,
                    ImageDrawMetadata {
                        bounds,
                        pixel_snap: sui_scene::ImagePixelSnap::None,
                    },
                );
                diagnostics.image_command_count += 1;
                Ok(())
            }
            SceneCommand::DrawShaderRect { rect, shader } => {
                self.scratch_vertices.clear();
                append_widget_shader_rect(
                    &mut self.scratch_vertices,
                    state,
                    *rect,
                    *shader,
                    viewport,
                );
                push_draw_op(
                    draw_ops,
                    DrawOpKind::WidgetShader,
                    &self.scratch_vertices,
                    state,
                );
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::PushClip { rect } => {
                state.push_clip(*rect);
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::PushClipPath { path } => {
                state.push_clip_path(path, viewport, draw_ops, &mut self.clip_scratch_vertices)?;
                diagnostics.clip_path_command_count += 1;
                Ok(())
            }
            SceneCommand::PopClip => {
                state.pop_clip(draw_ops);
                Ok(())
            }
            SceneCommand::PushTransform { transform } => {
                state.push_transform(*transform);
                Ok(())
            }
            SceneCommand::PopTransform => {
                state.pop_transform();
                Ok(())
            }
            SceneCommand::PushTextRenderPolicy { policy } => {
                state.push_text_render_policy(*policy);
                Ok(())
            }
            SceneCommand::PopTextRenderPolicy => {
                state.pop_text_render_policy();
                Ok(())
            }
            SceneCommand::Layer(layer) => Err(Error::new(format!(
                "retained direct packet compiler encountered nested layer {}",
                layer.layer_id().get()
            ))),
            SceneCommand::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                shadow,
            } => {
                // Submission order is z-order: paint the soft shadow first so the fill
                // (and its border) draw on top of it.
                if let Some(shadow) = shadow {
                    self.scratch_vertices.clear();
                    append_rounded_rect_shadow(
                        &mut self.scratch_vertices,
                        state,
                        *rect,
                        *radii,
                        *shadow,
                        viewport,
                        self.feather_width,
                    );
                    push_draw_op(
                        draw_ops,
                        DrawOpKind::RoundedRect,
                        &self.scratch_vertices,
                        state,
                    );
                }

                self.scratch_vertices.clear();
                match brush {
                    Brush::Solid(color) => {
                        append_rounded_rect_fill(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            *radii,
                            *color,
                            *border,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::RoundedRect,
                            &self.scratch_vertices,
                            state,
                        );
                    }
                    Brush::LinearGradient { start, end, stops } => {
                        let stop0 = stops.first().map(|s| s.color).unwrap_or(Color::TRANSPARENT);
                        let stop1 = stops.last().map(|s| s.color).unwrap_or(stop0);
                        append_gradient_rect(
                            &mut self.scratch_vertices,
                            state,
                            *rect,
                            *radii,
                            *start,
                            *end,
                            stop0,
                            stop1,
                            viewport,
                            self.feather_width,
                        );
                        push_draw_op(
                            draw_ops,
                            DrawOpKind::GradientRect,
                            &self.scratch_vertices,
                            state,
                        );
                        // A gradient fill ignores any border here (documented limitation);
                        // borders are only honored for solid rounded-rect fills.
                        let _ = border;
                    }
                }
                diagnostics.rect_command_count += 1;
                Ok(())
            }
            SceneCommand::Label { rect, text, color } => {
                self.scratch_text_instances.clear();
                self.text_engine.append_text_run(
                    &mut self.scratch_text_instances,
                    state,
                    &TextRun {
                        rect: *rect,
                        text: text.clone(),
                        style: TextStyle::new(*color),
                    },
                    self.frame.font_registry.as_ref(),
                    viewport,
                    self.frame.scale_factor,
                )?;
                push_text_draw_op(draw_ops, &self.scratch_text_instances, state);
                diagnostics.text_command_count += 1;
                Ok(())
            }
        };

        let elapsed_ms = command_started.elapsed().as_secs_f64() * 1000.0;
        match command {
            SceneCommand::Clear(_)
            | SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::DrawShaderRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::PushClip { .. } => {
                diagnostics.rect_command_time_ms += elapsed_ms;
            }
            SceneCommand::FillPath { .. } | SceneCommand::StrokePath { .. } => {
                diagnostics.path_command_time_ms += elapsed_ms;
            }
            SceneCommand::DrawText(_)
            | SceneCommand::DrawShapedText(_)
            | SceneCommand::DrawShapedTextWindow(_)
            | SceneCommand::Label { .. } => {
                diagnostics.text_command_time_ms += elapsed_ms;
            }
            SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                diagnostics.image_command_time_ms += elapsed_ms;
            }
            SceneCommand::PushClipPath { .. } => {
                diagnostics.clip_path_command_time_ms += elapsed_ms;
            }
            SceneCommand::PopClip
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PopTransform
            | SceneCommand::PushTextRenderPolicy { .. }
            | SceneCommand::PopTextRenderPolicy
            | SceneCommand::Layer(_) => {}
        }

        result
    }
}
