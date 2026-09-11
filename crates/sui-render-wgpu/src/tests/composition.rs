use crate::WgpuRenderer;
use crate::draw::CachedDrawBatch;
use crate::draw::CachedPassBatch;
use crate::draw::ClipState;
use crate::draw::DrawOp;
use crate::draw::DrawOpArena;
use crate::draw::DrawOpKind;
use crate::draw::PreparedDrawKind;
use crate::draw::PreparedVertices;
use crate::draw::ScissorRect;
use crate::gpu::Vertex;
use crate::primitives::to_ndc;
use crate::resources::DEFAULT_FEATHER_WIDTH;
use crate::retained::RetainedCompositorState;
use crate::scene::build_vertices;
use crate::submission::prepare_cached_passes;
use crate::submission::prepare_frame_batches;
use crate::tests::support::TestSceneLayerDescriptorExt;
use crate::tests::support::{
    LayerCachePolicy, RGBA_CHANNEL_TOLERANCE, assert_rgba_images_match, assert_rgba_pixel_near,
    is_physically_pixel_aligned, load_test_font, logical_x_from_ndc, logical_y_from_ndc,
    non_white_pixel_count, prepare_with_compositor, rgba_pixel,
};
use crate::text::GlyphSubpixelOffsetKey;
use crate::text_engine::TextEngine;
use crate::text_engine::glyph_subpixel_offset;
use std::sync::Arc;
use sui_core::Color;
use sui_core::FontHandle;
use sui_core::ImageHandle;
use sui_core::Path;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_core::WidgetId;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
use sui_scene::ImageSampling;
use sui_scene::LayerCompositionMode;
use sui_scene::Scene;
use sui_scene::SceneCommand;
use sui_scene::SceneFrame;
use sui_scene::SceneLayer;
use sui_scene::SceneLayerDescriptor;
use sui_scene::SceneLayerId;
use sui_scene::SceneLayerUpdate;
use sui_scene::SceneLayerUpdateKind;
use sui_scene::StrokeStyle;
use sui_text::FontRegistry;
use sui_text::ShapedGlyph;
use sui_text::TextLayoutRegistry;
use sui_text::TextRun;
use sui_text::TextStyle;

#[test]
pub(crate) fn headless_intermediate_accepts_stencil_clip_mask_pipeline() {
    let window_id = WindowId::new(4521);
    let viewport = Size::new(32.0, 24.0);
    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::BLACK));
    scene.push(SceneCommand::PushClipPath {
        path: Path::rounded_rect(Rect::new(4.0, 4.0, 24.0, 16.0), 6.0),
    });
    scene.push(SceneCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 32.0, 24.0),
        brush: Color::WHITE.into(),
    });
    scene.push(SceneCommand::PopClip);

    let frame = SceneFrame {
        window_id,
        viewport,
        surface_size: viewport,
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::new();
    renderer.render(&frame).unwrap();
    let image = renderer.capture_last_frame_rgba(window_id).unwrap();
    assert_rgba_pixel_near(&image, 16, 12, [255, 255, 255, 255], RGBA_CHANNEL_TOLERANCE);
    assert_rgba_pixel_near(&image, 0, 0, [0, 0, 0, 255], RGBA_CHANNEL_TOLERANCE);
}

#[test]
pub(crate) fn build_vertices_applies_clip_and_transform_to_fill_rects() {
    let mut scene = Scene::new();
    scene.push(SceneCommand::PushTransform {
        transform: Transform::translation(10.0, 5.0),
    });
    scene.push(SceneCommand::PushClip {
        rect: Rect::new(0.0, 0.0, 16.0, 12.0),
    });
    scene.push(SceneCommand::FillRect {
        rect: Rect::new(4.0, 3.0, 20.0, 10.0),
        brush: Color::WHITE.into(),
    });

    let mut text_engine = TextEngine::new().unwrap();
    let vertices = build_vertices(
        &SceneFrame {
            window_id: WindowId::new(1),
            viewport: Size::new(100.0, 100.0),
            surface_size: Size::new(100.0, 100.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
    )
    .unwrap();

    let expected_min = to_ndc(13.0, 7.0, Size::new(100.0, 100.0));
    let expected_max = to_ndc(26.0, 17.0, Size::new(100.0, 100.0));

    assert_eq!(vertices.len(), 6);
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.position == expected_min)
    );
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.position == expected_max)
    );
    assert!(
        vertices
            .iter()
            .all(|vertex| vertex.shader_params == [10.0, 5.0, 0.0, DEFAULT_FEATHER_WIDTH])
    );
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.tex_coords == [-11.0, -6.0])
    );
    assert!(
        vertices
            .iter()
            .any(|vertex| vertex.tex_coords == [2.0, 4.0])
    );
}

#[test]
pub(crate) fn glyph_subpixel_offset_includes_layer_pixel_snap_phase() {
    let glyph = ShapedGlyph {
        glyph_id: 42,
        cluster: 0,
        span_id: sui_text::TextSpanId {
            paragraph_index: 0,
            span_index: 0,
        },
        run_index: 0,
        line_index: 0,
        face_index: 0,
        origin_x: 14.0,
        origin_y: 20.0,
        advance: Vector::new(8.0, 0.0),
        scale: 12.0,
        bounds: None,
    };

    assert_eq!(
        glyph_subpixel_offset(
            Transform::IDENTITY,
            Vector::new(1.0 / 3.0, 0.0),
            &glyph,
            1.5
        ),
        GlyphSubpixelOffsetKey::new(2, 0)
    );
}

#[test]
pub(crate) fn retained_compositor_carries_active_path_clips() {
    let mut clip = Path::builder();
    clip.move_to(Point::new(8.0, 8.0))
        .line_to(Point::new(32.0, 8.0))
        .line_to(Point::new(20.0, 28.0))
        .close();

    let mut scene = Scene::new();
    scene.push(SceneCommand::PushClipPath { path: clip.build() });
    scene.push(SceneCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 40.0, 40.0),
        brush: Color::WHITE.into(),
    });

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let ops = prepare_with_compositor(
        &SceneFrame {
            window_id: WindowId::new(6),
            viewport: Size::new(64.0, 64.0),
            surface_size: Size::new(64.0, 64.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: Vec::new(),
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        },
        &mut text_engine,
        &mut compositor,
    )
    .unwrap();

    assert_eq!(ops.draw_ops.len(), 1);
    let op = &ops.draw_ops[0];
    let clip_state = &ops.clip_states[op.clip_state_index];
    assert!(op.clip_state_index > 0);
    assert_eq!(clip_state.clip_paths.len(), 1);
    assert!(clip_state.clip_paths[0].len > 0);
    assert_eq!(op.clip_rect, Some(Rect::new(8.0, 8.0, 24.0, 20.0)));
}

#[test]
pub(crate) fn prepare_frame_batches_converts_clip_rects_to_scissors() {
    let prepared = prepare_frame_batches(
        DrawOpArena {
            scene_vertices: vec![
                Vertex::basic(
                    [0.0, 0.0],
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0],
                    [0.0; 4],
                );
                6
            ],
            clip_vertices: Vec::new(),
            text_instances: Vec::new(),
            clip_states: vec![ClipState {
                clip_paths: Vec::new(),
            }],
            draw_ops: vec![DrawOp {
                kind: DrawOpKind::Solid,
                clip_rect: Some(Rect::new(5.0, 8.0, 20.0, 10.0)),
                vertices: PreparedVertices { start: 0, len: 6 },
                clip_state_index: 0,
                image: None,
            }],
            analytic_paths: std::collections::HashMap::new(),
            next_analytic_path_id: 0,
        },
        Size::new(50.0, 40.0),
        (100, 80),
    );

    assert_eq!(prepared.passes.len(), 1);
    assert_eq!(
        prepared.passes[0].draws[0].clip_rect,
        Some(ScissorRect {
            x: 10,
            y: 16,
            width: 40,
            height: 20,
        })
    );
}

#[test]
pub(crate) fn prepare_cached_passes_snap_translated_adjacent_clip_edges_without_overlap() {
    let passes = prepare_cached_passes(
        &[
            CachedPassBatch {
                clip_paths: Vec::new(),
                draws: vec![CachedDrawBatch {
                    kind: PreparedDrawKind::Solid,
                    clip_rect: Some(Rect::new(0.0, 0.0, 384.0, 128.0)),
                    vertices: PreparedVertices { start: 0, len: 6 },
                }],
            },
            CachedPassBatch {
                clip_paths: Vec::new(),
                draws: vec![CachedDrawBatch {
                    kind: PreparedDrawKind::Solid,
                    clip_rect: Some(Rect::new(384.0, 0.0, 128.0, 128.0)),
                    vertices: PreparedVertices { start: 6, len: 6 },
                }],
            },
        ],
        Size::new(512.0, 128.0),
        (768, 192),
        Vector::new(0.25, 0.0),
        None,
        0,
        0,
        0,
    );

    let first = passes[0].draws[0].clip_rect.expect("first scissor");
    let second = passes[1].draws[0].clip_rect.expect("second scissor");

    assert_eq!(first.x + first.width, second.x);
    assert!(first.x + first.width <= second.x);
}

#[test]
pub(crate) fn prepare_cached_passes_uses_external_clip_for_unclipped_draws() {
    let passes = prepare_cached_passes(
        &[CachedPassBatch {
            clip_paths: Vec::new(),
            draws: vec![CachedDrawBatch {
                kind: PreparedDrawKind::Image {
                    handle: ImageHandle::new(99),
                    sampling: ImageSampling::Linear,
                    raster_size: crate::draw::ImageRasterSize {
                        width: 1,
                        height: 1,
                    },
                },
                clip_rect: None,
                vertices: PreparedVertices { start: 0, len: 6 },
            }],
        }],
        Size::new(100.0, 100.0),
        (100, 100),
        Vector::ZERO,
        Some(Rect::new(20.0, 30.0, 40.0, 50.0)),
        0,
        0,
        0,
    );

    assert_eq!(passes.len(), 1);
    assert_eq!(passes[0].draws.len(), 1);
    assert_eq!(
        passes[0].draws[0].clip_rect,
        Some(ScissorRect {
            x: 20,
            y: 30,
            width: 40,
            height: 50,
        })
    );
}

#[test]
pub(crate) fn prepare_cached_passes_drops_draws_fully_outside_external_clip() {
    let passes = prepare_cached_passes(
        &[CachedPassBatch {
            clip_paths: Vec::new(),
            draws: vec![CachedDrawBatch {
                kind: PreparedDrawKind::Image {
                    handle: ImageHandle::new(100),
                    sampling: ImageSampling::Linear,
                    raster_size: crate::draw::ImageRasterSize {
                        width: 1,
                        height: 1,
                    },
                },
                clip_rect: Some(Rect::new(70.0, 70.0, 20.0, 20.0)),
                vertices: PreparedVertices { start: 0, len: 6 },
            }],
        }],
        Size::new(100.0, 100.0),
        (100, 100),
        Vector::ZERO,
        Some(Rect::new(0.0, 0.0, 20.0, 20.0)),
        0,
        0,
        0,
    );

    assert_eq!(passes.len(), 1);
    assert!(passes[0].draws.is_empty());
}

#[test]
pub(crate) fn overlay_layers_paint_above_later_normal_siblings() {
    let normal_id = WidgetId::new(205);
    let overlay_id = WidgetId::new(206);

    let build_frame = || {
        let normal_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(normal_id),
            normal_id,
            Rect::new(32.0, 12.0, 72.0, 48.0),
        )
        .with_content_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
        .with_paint_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
        .with_cache_policy(LayerCachePolicy::Direct);
        let overlay_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(overlay_id),
            overlay_id,
            Rect::new(12.0, 12.0, 72.0, 48.0),
        )
        .with_content_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
        .with_paint_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Overlay);

        let mut overlay_scene = Scene::new();
        overlay_scene.push(SceneCommand::FillRect {
            rect: overlay_descriptor.bounds,
            brush: Color::rgba(0.90, 0.16, 0.16, 1.0).into(),
        });

        let mut normal_scene = Scene::new();
        normal_scene.push(SceneCommand::FillRect {
            rect: normal_descriptor.bounds,
            brush: Color::rgba(0.16, 0.72, 0.24, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            overlay_descriptor.clone(),
            overlay_scene,
        )));
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            normal_descriptor.clone(),
            normal_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(143),
            viewport: Size::new(128.0, 80.0),
            surface_size: Size::new(128.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    overlay_descriptor,
                )
                .with_damage(Rect::new(12.0, 12.0, 72.0, 48.0)),
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, normal_descriptor)
                    .with_damage(Rect::new(32.0, 12.0, 72.0, 48.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let frame = build_frame();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    let overlap = rgba_pixel(&pixels, 48, 24);

    assert!(
        overlap[0] > overlap[1],
        "expected overlay pixel to dominate overlap, got rgba={overlap:?}"
    );
}

#[test]
pub(crate) fn nested_overlay_layers_paint_above_later_root_normal_siblings() {
    let shell_id = WidgetId::new(207);
    let nested_overlay_id = WidgetId::new(208);
    let blocker_id = WidgetId::new(209);

    let build_frame = || {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 128.0, 80.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 128.0, 80.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 128.0, 80.0))
        .with_cache_policy(LayerCachePolicy::Direct);
        let nested_overlay_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(nested_overlay_id),
            nested_overlay_id,
            Rect::new(12.0, 12.0, 72.0, 48.0),
        )
        .with_content_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
        .with_paint_bounds(Rect::new(12.0, 12.0, 72.0, 48.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Overlay);
        let blocker_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(blocker_id),
            blocker_id,
            Rect::new(32.0, 12.0, 72.0, 48.0),
        )
        .with_content_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
        .with_paint_bounds(Rect::new(32.0, 12.0, 72.0, 48.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut nested_overlay_scene = Scene::new();
        nested_overlay_scene.push(SceneCommand::FillRect {
            rect: nested_overlay_descriptor.bounds,
            brush: Color::rgba(0.90, 0.16, 0.16, 1.0).into(),
        });

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: shell_descriptor.bounds,
            brush: Color::rgba(0.97, 0.98, 1.0, 1.0).into(),
        });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            nested_overlay_descriptor.clone(),
            nested_overlay_scene,
        )));

        let mut blocker_scene = Scene::new();
        blocker_scene.push(SceneCommand::FillRect {
            rect: blocker_descriptor.bounds,
            brush: Color::rgba(0.16, 0.72, 0.24, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            blocker_descriptor.clone(),
            blocker_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(144),
            viewport: Size::new(128.0, 80.0),
            surface_size: Size::new(128.0, 80.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, shell_descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 128.0, 80.0)),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    nested_overlay_descriptor,
                )
                .with_damage(Rect::new(12.0, 12.0, 72.0, 48.0)),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    blocker_descriptor,
                )
                .with_damage(Rect::new(32.0, 12.0, 72.0, 48.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let frame = build_frame();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    let overlap = rgba_pixel(&pixels, 48, 24);

    assert!(
        overlap[0] > overlap[1],
        "expected nested overlay pixel to dominate overlap, got rgba={overlap:?}"
    );
}

#[test]
pub(crate) fn cached_ancestor_overlay_layers_paint_above_later_root_normal_siblings() {
    let shell_id = WidgetId::new(210);
    let nested_overlay_id = WidgetId::new(211);
    let blocker_id = WidgetId::new(212);

    let build_frame = || {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 512.0, 120.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 120.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 120.0))
        .with_cache_policy(LayerCachePolicy::Cached);
        let nested_overlay_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(nested_overlay_id),
            nested_overlay_id,
            Rect::new(12.0, 12.0, 180.0, 80.0),
        )
        .with_content_bounds(Rect::new(12.0, 12.0, 180.0, 80.0))
        .with_paint_bounds(Rect::new(12.0, 12.0, 180.0, 80.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Overlay);
        let blocker_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(blocker_id),
            blocker_id,
            Rect::new(96.0, 40.0, 220.0, 44.0),
        )
        .with_content_bounds(Rect::new(96.0, 40.0, 220.0, 44.0))
        .with_paint_bounds(Rect::new(96.0, 40.0, 220.0, 44.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut nested_overlay_scene = Scene::new();
        nested_overlay_scene.push(SceneCommand::FillRect {
            rect: nested_overlay_descriptor.bounds,
            brush: Color::rgba(0.90, 0.16, 0.16, 1.0).into(),
        });

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: shell_descriptor.bounds,
            brush: Color::rgba(0.97, 0.98, 1.0, 1.0).into(),
        });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            nested_overlay_descriptor.clone(),
            nested_overlay_scene,
        )));

        let mut blocker_scene = Scene::new();
        blocker_scene.push(SceneCommand::FillRect {
            rect: blocker_descriptor.bounds,
            brush: Color::rgba(0.16, 0.72, 0.24, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            blocker_descriptor.clone(),
            blocker_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(145),
            viewport: Size::new(512.0, 120.0),
            surface_size: Size::new(512.0, 120.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, shell_descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 120.0)),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    nested_overlay_descriptor,
                )
                .with_damage(Rect::new(12.0, 12.0, 180.0, 80.0)),
                SceneLayerUpdate::from_descriptor(
                    SceneLayerUpdateKind::Content,
                    blocker_descriptor,
                )
                .with_damage(Rect::new(96.0, 40.0, 220.0, 44.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let frame = build_frame();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();
    let overlap = rgba_pixel(&pixels, 120, 52);

    assert!(
        overlap[0] > overlap[1],
        "expected cached-ancestor overlay pixel to dominate overlap, got rgba={overlap:?}"
    );
}

#[test]
pub(crate) fn retained_layer_local_text_snaps_after_fractional_origin_composition() {
    let handle = FontHandle::new(127);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());
    let viewport = Size::new(240.0, 92.0);
    let layer_id = WidgetId::new(127);
    let layer_bounds = Rect::new(58.333_332, 18.333_334, 158.0, 54.0);
    let text_rect = Rect::new(
        layer_bounds.x() + 14.0,
        layer_bounds.y() + 14.0,
        132.0,
        24.0,
    );
    let text_style = TextStyle {
        font: Some(handle),
        font_size: 14.0,
        line_height: 19.0,
        color: Color::rgba(0.92, 0.94, 0.98, 1.0),
        ..TextStyle::default()
    };
    let text = "Popup text snaps".to_string();

    let descriptor =
        SceneLayerDescriptor::new(SceneLayerId::from_widget(layer_id), layer_id, layer_bounds)
            .with_content_bounds(layer_bounds)
            .with_paint_bounds(layer_bounds);
    let mut layer_scene = Scene::new();
    layer_scene.push(SceneCommand::FillRect {
        rect: layer_bounds,
        brush: Color::rgba(0.12, 0.15, 0.20, 1.0).into(),
    });
    layer_scene.push(SceneCommand::DrawText(TextRun {
        rect: text_rect,
        text,
        style: text_style,
    }));
    let mut retained_scene = Scene::new();
    retained_scene.push(SceneCommand::FillRect {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        brush: Color::rgba(0.06, 0.07, 0.09, 1.0).into(),
    });
    retained_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        descriptor.clone(),
        layer_scene,
    )));

    let retained = SceneFrame {
        window_id: WindowId::new(128),
        viewport,
        surface_size: Size::new(360.0, 138.0),
        scale_factor: 1.5,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                .with_damage(layer_bounds),
        ],
        scene: retained_scene,
        font_registry: Arc::new(fonts),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = prepare_with_compositor(&retained, &mut text_engine, &mut compositor)
        .expect("retained frame should prepare");
    let text_op = draw_ops
        .draw_ops
        .iter()
        .find(|op| matches!(op.kind, DrawOpKind::TextAtlas))
        .expect("retained layer should emit atlas text");
    let start = text_op.vertices.start as usize;
    let end = start + text_op.vertices.len as usize;
    let instances = &draw_ops.text_instances[start..end];
    assert!(!instances.is_empty());
    for (index, instance) in instances.iter().enumerate() {
        let x = logical_x_from_ndc(instance.top_left[0], viewport);
        let y = logical_y_from_ndc(instance.top_left[1], viewport);
        assert!(
            is_physically_pixel_aligned(x, retained.scale_factor),
            "text instance {index} x was not aligned after layer composition: {x}"
        );
        assert!(
            is_physically_pixel_aligned(y, retained.scale_factor),
            "text instance {index} y was not aligned after layer composition: {y}"
        );
    }
}

#[test]
pub(crate) fn transformed_layer_keeps_analytic_path_at_translated_location() {
    let layer_id = WidgetId::new(104);
    let window_id = WindowId::new(104);
    let build_layer = |x: f32| {
        let bounds = Rect::new(x, 12.0, 72.0, 52.0);
        let descriptor =
            SceneLayerDescriptor::new(SceneLayerId::from_widget(layer_id), layer_id, bounds)
                .with_content_bounds(bounds)
                .with_paint_bounds(bounds)
                .with_cache_policy(LayerCachePolicy::Direct);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(Rect::new(x + 14.0, 24.0, 44.0, 24.0), 7.0),
            brush: Color::rgba(0.18, 0.32, 0.86, 1.0).into(),
        });

        (
            descriptor.clone(),
            SceneLayer::from_descriptor(descriptor, layer_scene),
        )
    };

    let build_frame = |descriptor: SceneLayerDescriptor, layer: SceneLayer, update_kind| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 240.0, 92.0),
            brush: Color::WHITE.into(),
        });
        scene.push(SceneCommand::Layer(layer));

        SceneFrame {
            window_id,
            viewport: Size::new(240.0, 92.0),
            surface_size: Size::new(240.0, 92.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![SceneLayerUpdate::from_descriptor(update_kind, descriptor)],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let (initial_descriptor, initial_layer) = build_layer(28.0);
    let (moved_descriptor, moved_layer) = build_layer(136.0);

    let mut renderer = WgpuRenderer::default();
    let first = build_frame(
        initial_descriptor,
        initial_layer,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&first).unwrap();

    let second = build_frame(
        moved_descriptor,
        moved_layer,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&second).unwrap();
    let pixels = renderer.capture_last_frame_rgba(window_id).unwrap();

    let old_location_pixels = non_white_pixel_count(&pixels, Rect::new(34.0, 18.0, 60.0, 40.0));
    let moved_location_pixels = non_white_pixel_count(&pixels, Rect::new(142.0, 18.0, 60.0, 40.0));

    assert_eq!(
        old_location_pixels, 0,
        "translated feathered path left pixels at its previous layer location"
    );
    assert!(
        moved_location_pixels > 400,
        "translated feathered path did not render at its moved layer location (moved_location_pixels={moved_location_pixels})"
    );
}

#[test]
pub(crate) fn cached_layer_matches_direct_for_tight_rounded_border_at_fractional_scale() {
    let widget_id = WidgetId::new(100);
    let build_frame = |cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(widget_id),
            widget_id,
            Rect::new(0.0, 0.0, 220.0, 64.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 64.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 64.0))
        .with_cache_policy(cache_policy);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 220.0, 64.0),
            brush: Color::WHITE.into(),
        });
        layer_scene.push(SceneCommand::StrokePath {
            path: Path::rounded_rect(Rect::new(0.0, 0.0, 220.0, 64.0), 10.0),
            brush: Color::rgba(0.18, 0.33, 0.85, 1.0).into(),
            stroke: StrokeStyle::new(1.0 / 1.5),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(100),
            viewport: Size::new(220.0, 64.0),
            surface_size: Size::new(330.0, 96.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 220.0, 64.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_layer_matches_direct_for_tight_tab_text_at_fractional_scale() {
    let handle = FontHandle::new(30);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let widget_id = WidgetId::new(101);
    let build_frame = |cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(widget_id),
            widget_id,
            Rect::new(0.0, 0.0, 236.0, 44.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 236.0, 44.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 236.0, 44.0))
        .with_cache_policy(cache_policy);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 236.0, 44.0),
            brush: Color::rgba(0.96, 0.97, 0.99, 1.0).into(),
        });
        layer_scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(10.0, 10.0, 216.0, 20.0),
            text: "Inspector".to_string(),
            style: TextStyle {
                font: Some(handle),
                font_size: 12.0,
                line_height: 16.0,
                color: Color::rgba(0.15, 0.19, 0.26, 1.0),
                ..TextStyle::default()
            },
        }));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(101),
            viewport: Size::new(236.0, 44.0),
            surface_size: Size::new(354.0, 66.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 236.0, 44.0)),
            ],
            scene,
            font_registry: Arc::new(fonts.clone()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}
