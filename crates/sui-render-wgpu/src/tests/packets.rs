use crate::WgpuRenderer;
use crate::diagnostics::PacketRebuildReason;
use crate::diagnostics::RendererFrameStats;
use crate::diagnostics::RetainedPacketRebuildStats;
use crate::draw::DrawOpKind;
use crate::retained::CompositionContainerId;
use crate::retained::RetainedCompositorFrameStats;
use crate::retained::RetainedCompositorState;
use crate::retained::RetainedPacketId;
use crate::tests::support::TestSceneLayerDescriptorExt;
use crate::tests::support::{
    LayerCachePolicy, assert_rgba_images_match, content_updates, load_test_font, packet_signature,
    prepare_with_compositor,
};
use crate::text_engine::TextEngine;
use std::sync::Arc;
use sui_core::Color;
use sui_core::FontHandle;
use sui_core::Path;
use sui_core::Point;
use sui_core::Rect;
use sui_core::Size;
use sui_core::WidgetId;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
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
use sui_text::TextLayoutRegistry;
use sui_text::TextRun;
use sui_text::TextStyle;

#[test]
fn packet_chunks_match_unsplit_rendering_through_state_and_content_changes() {
    use sui_core::Transform;
    use sui_scene::{TextRenderCoveragePolicy, TextRenderPolicy};
    let window = WindowId::new(8954);
    let mut chunked = WgpuRenderer::new();
    let mut reference = WgpuRenderer::new();
    reference
        .compositors
        .entry(window)
        .or_default()
        .packet_draw_limit = usize::MAX;
    for layer in [false, true] {
        for revision in 0..4 {
            let mut content = Scene::new();
            for index in 0..48 {
                let x = (index % 8) as f32 * 24.0;
                let y = (index / 8) as f32 * 20.0;
                content.push(SceneCommand::PushTransform {
                    transform: Transform::translation(revision as f32 * 0.25, 0.5),
                });
                content.push(SceneCommand::PushClipPath {
                    path: Path::rect(Rect::new(x, y, 23.0, 19.0)),
                });
                content.push(SceneCommand::PushTextRenderPolicy {
                    policy: TextRenderPolicy {
                        coverage_policy: Some(TextRenderCoveragePolicy::Gamma(1.6)),
                        ..Default::default()
                    },
                });
                content.push(SceneCommand::FillRect {
                    rect: Rect::new(x, y, 22.0, 18.0),
                    brush: Color::srgba(0.2, 0.1, 0.3, 0.5).into(),
                });
                content.push(SceneCommand::Label {
                    rect: Rect::new(x, y, 22.0, 18.0),
                    text: if index == 19 && revision % 2 == 1 {
                        "B"
                    } else {
                        "A"
                    }
                    .into(),
                    color: Color::WHITE,
                });
                content.push(SceneCommand::PopTextRenderPolicy);
                content.push(SceneCommand::PopClip);
                content.push(SceneCommand::PopTransform);
            }
            let mut frame = SceneFrame::new(window, Size::new(200.0, 128.0));
            frame.scale_factor = if revision < 2 { 1.0 } else { 1.25 };
            frame.surface_size = Size::new(200.0 * frame.scale_factor, 128.0 * frame.scale_factor);
            frame.scene.push(SceneCommand::Clear(Color::BLACK));
            if layer {
                let owner = WidgetId::new(8955);
                let descriptor = SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(owner),
                    owner,
                    Rect::new(1.5, 2.5, 190.0, 116.0),
                )
                .with_composition_mode(LayerCompositionMode::Scroll);
                frame
                    .scene
                    .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                        descriptor, content,
                    )));
            } else {
                frame.scene.append(content);
            }
            chunked.render(&frame).unwrap();
            reference.render(&frame).unwrap();
            assert_eq!(reference.compositors[&window].packet_draw_limit, usize::MAX);
            assert!(
                chunked.compositors[&window].packets.len()
                    > reference.compositors[&window].packets.len()
            );
            assert_rgba_images_match(
                &chunked.capture_rgba(window).unwrap(),
                &reference.capture_rgba(window).unwrap(),
            );
        }
    }
    let empty = SceneFrame::new(window, Size::new(200.0, 128.0));
    chunked.render(&empty).unwrap();
    assert!(
        chunked.compositors[&window].packets.is_empty(),
        "obsolete packets must be released"
    );
    assert!(chunked.frame_resources.fragments[&window].is_empty());
}

#[test]
fn optimization_regression_local_change_rebuilds_only_a_small_packet() {
    let make_scene = |changed: bool| {
        let mut scene = Scene::new();
        for i in 0..96 {
            scene.push(SceneCommand::FillRect {
                rect: Rect::new((i % 12) as f32 * 8.0, (i / 12) as f32 * 8.0, 7.0, 7.0),
                brush: if changed && i == 32 {
                    Color::BLACK
                } else {
                    Color::WHITE
                }
                .into(),
            });
        }
        scene
    };
    let mut frame = SceneFrame::new(WindowId::new(8952), Size::new(96.0, 64.0));
    frame.scene = make_scene(false);
    let mut engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    prepare_with_compositor(&frame, &mut engine, &mut compositor).unwrap();
    frame.scene = make_scene(true);
    prepare_with_compositor(&frame, &mut engine, &mut compositor).unwrap();
    assert!(
        compositor.last_frame_stats.packet_command_count <= 16,
        "a local edit rebuilt {} commands",
        compositor.last_frame_stats.packet_command_count
    );
}

#[test]
fn optimization_regression_recycled_atlas_invalidates_retained_text() {
    let mut renderer = WgpuRenderer::new();
    let window = WindowId::new(8953);
    let mut frame = SceneFrame::new(window, Size::new(200.0, 60.0));
    frame.scene.push(SceneCommand::Clear(Color::BLACK));
    frame.scene.push(SceneCommand::Label {
        rect: Rect::new(8.0, 8.0, 180.0, 40.0),
        text: "Retained glyphs".into(),
        color: Color::WHITE,
    });
    renderer.render(&frame).unwrap();
    let original = renderer.capture_rgba(window).unwrap();
    // Reproduce the invalidation boundary of page eviction (including eviction
    // caused by another window sharing the renderer's text atlas).
    let engine = renderer.text_engine.as_mut().unwrap();
    engine.atlas.pages[0].clear_for_reuse();
    engine.glyph_cache.clear();
    renderer.render(&frame).unwrap();
    let restored = renderer.capture_rgba(window).unwrap();
    assert!(
        original.pixels() == restored.pixels(),
        "atlas recycling corrupted retained text"
    );
}

#[test]
fn cached_packets_keep_their_atlas_pages_live_without_rasterizing_glyphs() {
    let mut frame = SceneFrame::new(WindowId::new(8957), Size::new(200.0, 60.0));
    frame.scene.push(SceneCommand::Label {
        rect: Rect::new(0.0, 0.0, 200.0, 60.0),
        text: "Keep this page".into(),
        color: Color::WHITE,
    });
    let mut engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    engine.begin_frame();
    prepare_with_compositor(&frame, &mut engine, &mut compositor).unwrap();
    engine.begin_frame();
    prepare_with_compositor(&frame, &mut engine, &mut compositor).unwrap();
    assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
    assert_eq!(engine.atlas.pages[0].last_used_frame, engine.frame_counter);
    assert_eq!(engine.frame_stats.atlas_miss_count, 0);
}

#[test]
pub(crate) fn retained_packet_rebuild_stats_record_each_reason_and_preserve_grouping() {
    let mut rebuilds = RetainedPacketRebuildStats::default();

    rebuilds.record_reason(PacketRebuildReason::NewPacket);
    rebuilds.record_reason(PacketRebuildReason::CoordinateSpace);
    rebuilds.record_reason(PacketRebuildReason::Signature);
    rebuilds.record_reason(PacketRebuildReason::Scene);
    rebuilds.record_reason(PacketRebuildReason::State);

    assert_eq!(rebuilds, RetainedPacketRebuildStats::new(1, 1, 1, 1, 1));
    assert_eq!(rebuilds.total_count(), 5);
}

#[test]
pub(crate) fn renderer_frame_stats_with_compositor_stats_preserves_grouped_packet_rebuilds() {
    let stats = RendererFrameStats::from_prepared_counts(1, 2, 3).with_compositor_stats(
        RetainedCompositorFrameStats {
            packet_rebuilds: RetainedPacketRebuildStats::new(2, 3, 5, 7, 11),
            ..Default::default()
        },
    );

    assert_eq!(
        stats.retained_packet_rebuilds,
        RetainedPacketRebuildStats::new(2, 3, 5, 7, 11)
    );
}

#[test]
pub(crate) fn retained_compositor_reuses_cached_path_meshes_across_retained_packets() {
    let layer_id = WidgetId::new(70);
    let descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(layer_id),
        layer_id,
        Rect::new(0.0, 0.0, 512.0, 128.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_cache_policy(LayerCachePolicy::Cached);

    let mut layer_scene = Scene::new();
    layer_scene.push(SceneCommand::FillPath {
        path: Path::rect(Rect::new(0.0, 0.0, 512.0, 128.0)),
        brush: Color::rgba(0.24, 0.48, 0.72, 1.0).into(),
    });

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        descriptor.clone(),
        layer_scene,
    )));

    let frame = SceneFrame {
        window_id: WindowId::new(30),
        viewport: Size::new(512.0, 128.0),
        surface_size: Size::new(512.0, 128.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert!(!draw_ops.draw_ops.is_empty());
    assert_eq!(compositor.path_cache.stats(), (0, 0, 0));
    assert_eq!(draw_ops.analytic_paths.len(), 1);
    assert!(
        draw_ops
            .draw_ops
            .iter()
            .any(|draw| matches!(draw.kind, DrawOpKind::AnalyticPath { .. }))
    );
}

#[test]
pub(crate) fn retained_compositor_uses_analytic_stroke_paths_across_retained_packets() {
    let layer_id = WidgetId::new(71);
    let descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(layer_id),
        layer_id,
        Rect::new(0.0, 0.0, 512.0, 128.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_cache_policy(LayerCachePolicy::Cached);

    let mut stroke_path = Path::builder();
    stroke_path
        .move_to(Point::new(8.0, 24.0))
        .line_to(Point::new(180.0, 92.0))
        .line_to(Point::new(340.0, 20.0))
        .line_to(Point::new(500.0, 92.0));

    let mut layer_scene = Scene::new();
    layer_scene.push(SceneCommand::StrokePath {
        path: stroke_path.build(),
        brush: Color::rgba(0.92, 0.46, 0.18, 1.0).into(),
        stroke: StrokeStyle::new(12.0),
    });

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        descriptor.clone(),
        layer_scene,
    )));

    let frame = SceneFrame {
        window_id: WindowId::new(31),
        viewport: Size::new(512.0, 128.0),
        surface_size: Size::new(512.0, 128.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let draw_ops = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert!(!draw_ops.draw_ops.is_empty());
    assert_eq!(compositor.path_cache.stats(), (0, 0, 0));
    assert_eq!(draw_ops.analytic_paths.len(), 1);
    assert!(
        draw_ops
            .draw_ops
            .iter()
            .any(|draw| matches!(draw.kind, DrawOpKind::AnalyticPath { .. }))
    );
}

#[test]
pub(crate) fn retained_compositor_reuses_layer_packets_until_content_changes() {
    let layer_id = WidgetId::new(41);
    let mut child_scene = Scene::new();
    child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(4.0, 6.0, 32.0, 24.0),
        brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
    });

    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::BLACK));
    scene.push(SceneCommand::Layer(SceneLayer::new(
        layer_id,
        Rect::new(4.0, 6.0, 32.0, 24.0),
        child_scene,
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(21),
        viewport: Size::new(96.0, 64.0),
        surface_size: Size::new(96.0, 64.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: content_updates([layer_id]),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert!(compositor.last_frame_stats.packet_build_count > 0);
    let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
    let first_signature = packet_signature(&compositor, layer_container);
    let first_content_version =
        compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

    frame.layer_updates.clear();
    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
    assert_eq!(first.scene_vertices, second.scene_vertices);
    assert_eq!(
        first_signature,
        packet_signature(&compositor, layer_container)
    );

    frame.layer_updates = content_updates([layer_id]);
    let mut updated_child_scene = Scene::new();
    updated_child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(4.0, 6.0, 32.0, 24.0),
        brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
    });
    updated_child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(12.0, 10.0, 8.0, 8.0),
        brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
    });
    assert!(frame.scene.replace_layer(
        layer_id,
        SceneLayer::new(
            layer_id,
            Rect::new(4.0, 6.0, 32.0, 24.0),
            updated_child_scene
        ),
    ));
    let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert!(compositor.last_frame_stats.packet_build_count > 0);
    let third_signature = packet_signature(&compositor, layer_container);
    let third_content_version =
        compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

    assert!(third_content_version > first_content_version);
    assert_ne!(first_signature, third_signature);
    assert_ne!(first.scene_vertices, third.scene_vertices);
}

#[test]
pub(crate) fn retained_compositor_skips_packet_rebuild_for_unchanged_content_updates() {
    let layer_id = WidgetId::new(411);
    let mut child_scene = Scene::new();
    child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(4.0, 6.0, 32.0, 24.0),
        brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
    });

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::new(
        layer_id,
        Rect::new(4.0, 6.0, 32.0, 24.0),
        child_scene,
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(211),
        viewport: Size::new(96.0, 64.0),
        surface_size: Size::new(96.0, 64.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: content_updates([layer_id]),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert!(compositor.last_frame_stats.packet_build_count > 0);
    let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
    let first_signature = packet_signature(&compositor, layer_container);

    frame.layer_updates = content_updates([layer_id]);
    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
    assert_eq!(
        first_signature,
        packet_signature(&compositor, layer_container)
    );
    assert_eq!(first.scene_vertices, second.scene_vertices);
}

#[test]
pub(crate) fn retained_compositor_reuses_parent_packets_when_only_child_content_changes() {
    let parent_id = WidgetId::new(51);
    let child_id = WidgetId::new(52);

    let mut child_scene = Scene::new();
    child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(8.0, 8.0, 10.0, 10.0),
        brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
    });

    let mut parent_scene = Scene::new();
    parent_scene.push(SceneCommand::FillRect {
        rect: Rect::new(4.0, 4.0, 24.0, 24.0),
        brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
    });
    parent_scene.push(SceneCommand::Layer(SceneLayer::new(
        child_id,
        Rect::new(8.0, 8.0, 10.0, 10.0),
        child_scene,
    )));

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::new(
        parent_id,
        Rect::new(4.0, 4.0, 24.0, 24.0),
        parent_scene,
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(22),
        viewport: Size::new(64.0, 64.0),
        surface_size: Size::new(64.0, 64.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: content_updates([parent_id, child_id]),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let parent_container = CompositionContainerId::Layer(SceneLayerId::from_widget(parent_id));
    let child_container = CompositionContainerId::Layer(SceneLayerId::from_widget(child_id));
    let parent_signature = packet_signature(&compositor, parent_container);
    let child_signature = packet_signature(&compositor, child_container);

    frame.layer_updates.clear();
    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert_eq!(first.scene_vertices, second.scene_vertices);
    assert_eq!(
        parent_signature,
        packet_signature(&compositor, parent_container)
    );
    assert_eq!(
        child_signature,
        packet_signature(&compositor, child_container)
    );

    let mut updated_child_scene = Scene::new();
    updated_child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(8.0, 8.0, 10.0, 10.0),
        brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
    });

    let mut updated_parent_scene = Scene::new();
    updated_parent_scene.push(SceneCommand::FillRect {
        rect: Rect::new(4.0, 4.0, 24.0, 24.0),
        brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
    });
    updated_parent_scene.push(SceneCommand::Layer(SceneLayer::new(
        child_id,
        Rect::new(8.0, 8.0, 10.0, 10.0),
        updated_child_scene,
    )));
    assert!(frame.scene.replace_layer(
        parent_id,
        SceneLayer::new(
            parent_id,
            Rect::new(4.0, 4.0, 24.0, 24.0),
            updated_parent_scene
        ),
    ));

    frame.layer_updates = content_updates([child_id]);
    let third = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert_eq!(
        parent_signature,
        packet_signature(&compositor, parent_container)
    );
    assert_ne!(
        child_signature,
        packet_signature(&compositor, child_container)
    );
    assert_ne!(first.scene_vertices, third.scene_vertices);
}

#[test]
pub(crate) fn retained_compositor_reuses_direct_packets_across_layer_translation() {
    let layer_id = WidgetId::new(53);

    let build_layer = |x: f32| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(x, 10.0, 80.0, 36.0),
        )
        .with_content_bounds(Rect::new(x, 10.0, 80.0, 36.0))
        .with_paint_bounds(Rect::new(x, 10.0, 80.0, 36.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(x, 10.0, 80.0, 36.0),
            brush: Color::rgba(0.82, 0.36, 0.18, 1.0).into(),
        });

        SceneLayer::from_descriptor(descriptor, layer_scene)
    };

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(build_layer(8.0)));

    let mut frame = SceneFrame {
        window_id: WindowId::new(24),
        viewport: Size::new(160.0, 80.0),
        surface_size: Size::new(160.0, 80.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: content_updates([layer_id]),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    assert!(compositor.last_frame_stats.packet_build_count > 0);
    let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
    let first_signature = packet_signature(&compositor, layer_container);
    let first_content_version =
        compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

    frame.scene = {
        let mut next = Scene::new();
        next.push(SceneCommand::Layer(build_layer(44.0)));
        next
    };
    frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
        SceneLayerUpdateKind::Transform,
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(44.0, 10.0, 80.0, 36.0),
        )
        .with_content_bounds(Rect::new(44.0, 10.0, 80.0, 36.0))
        .with_paint_bounds(Rect::new(44.0, 10.0, 80.0, 36.0))
        .with_cache_policy(LayerCachePolicy::Direct),
    )];

    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let second_signature = packet_signature(&compositor, layer_container);
    let second_content_version =
        compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;

    assert_eq!(first_signature, second_signature);
    assert_eq!(first_content_version, second_content_version);
    assert_eq!(compositor.last_frame_stats.direct_packets, 1);
    assert_eq!(compositor.last_frame_stats.packet_build_count, 0);
    assert_ne!(first.scene_vertices, second.scene_vertices);
}

#[test]
pub(crate) fn retained_compositor_reuses_direct_packets_across_clip_only_updates() {
    let layer_id = WidgetId::new(54);
    let descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(layer_id),
        layer_id,
        Rect::new(8.0, 8.0, 96.0, 48.0),
    )
    .with_content_bounds(Rect::new(8.0, 8.0, 96.0, 48.0))
    .with_paint_bounds(Rect::new(8.0, 8.0, 96.0, 48.0))
    .with_cache_policy(LayerCachePolicy::Direct);

    let build_layer = || {
        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(8.0, 8.0, 96.0, 48.0),
            brush: Color::rgba(0.16, 0.52, 0.84, 1.0).into(),
        });
        SceneLayer::from_descriptor(descriptor.clone(), layer_scene)
    };

    let build_scene = |clip: Rect| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::PushClip { rect: clip });
        scene.push(SceneCommand::Layer(build_layer()));
        scene.push(SceneCommand::PopClip);
        scene
    };

    let mut frame = SceneFrame {
        window_id: WindowId::new(25),
        viewport: Size::new(160.0, 96.0),
        surface_size: Size::new(160.0, 96.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: content_updates([layer_id]),
        scene: build_scene(Rect::new(0.0, 0.0, 160.0, 96.0)),
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let layer_container = CompositionContainerId::Layer(SceneLayerId::from_widget(layer_id));
    let first_signature = packet_signature(&compositor, layer_container);
    let first_content_version =
        compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;
    let first_clip_rects = first
        .draw_ops
        .iter()
        .map(|draw_op| draw_op.clip_rect)
        .collect::<Vec<_>>();

    frame.scene = build_scene(Rect::new(24.0, 8.0, 64.0, 48.0));
    frame.layer_updates = vec![SceneLayerUpdate::from_descriptor(
        SceneLayerUpdateKind::Clip,
        descriptor.clone(),
    )];

    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let second_signature = packet_signature(&compositor, layer_container);
    let second_content_version =
        compositor.layers[&SceneLayerId::from_widget(layer_id)].content_version;
    let second_clip_rects = second
        .draw_ops
        .iter()
        .map(|draw_op| draw_op.clip_rect)
        .collect::<Vec<_>>();

    assert_eq!(first_signature, second_signature);
    assert_eq!(first_content_version, second_content_version);
    assert_eq!(compositor.last_frame_stats.direct_packets, 1);
    assert_eq!(first.scene_vertices, second.scene_vertices);
    assert_ne!(first_clip_rects, second_clip_rects);
}

#[test]
pub(crate) fn retained_compositor_rebuilds_retained_packets_across_ancestor_clip_updates() {
    let shell_id = WidgetId::new(55);
    let scroll_id = WidgetId::new(56);
    let content_id = WidgetId::new(57);

    let shell_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(shell_id),
        shell_id,
        Rect::new(0.0, 0.0, 220.0, 180.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 180.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 180.0))
    .with_cache_policy(LayerCachePolicy::Direct);

    let scroll_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(scroll_id),
        scroll_id,
        Rect::new(0.0, 0.0, 220.0, 360.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
    .with_cache_policy(LayerCachePolicy::Cached)
    .with_composition_mode(LayerCompositionMode::Scroll);

    let content_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(content_id),
        content_id,
        Rect::new(0.0, 0.0, 220.0, 360.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 220.0, 360.0))
    .with_cache_policy(LayerCachePolicy::Direct);

    let build_scene = |clip: Rect| {
        let mut content_scene = Scene::new();
        content_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 220.0, 360.0),
            brush: Color::WHITE.into(),
        });
        content_scene.push(SceneCommand::FillRect {
            rect: Rect::new(16.0, 16.0, 188.0, 120.0),
            brush: Color::rgba(0.28, 0.20, 0.86, 1.0).into(),
        });
        content_scene.push(SceneCommand::FillRect {
            rect: Rect::new(16.0, 220.0, 188.0, 120.0),
            brush: Color::rgba(0.14, 0.55, 0.82, 1.0).into(),
        });

        let mut scroll_scene = Scene::new();
        scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            content_descriptor.clone(),
            content_scene,
        )));

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::PushClip { rect: clip });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            scroll_descriptor.clone(),
            scroll_scene,
        )));
        shell_scene.push(SceneCommand::PopClip);

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));
        scene
    };

    let mut frame = SceneFrame {
        window_id: WindowId::new(26),
        viewport: Size::new(220.0, 180.0),
        surface_size: Size::new(220.0, 180.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                scroll_descriptor.clone(),
            )
            .with_damage(scroll_descriptor.paint_bounds),
        ],
        scene: build_scene(Rect::new(0.0, 0.0, 220.0, 180.0)),
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let first_clip_rects = first
        .draw_ops
        .iter()
        .map(|draw_op| draw_op.clip_rect)
        .collect::<Vec<_>>();

    frame.scene = build_scene(Rect::new(0.0, 0.0, 220.0, 96.0));
    frame.layer_updates = vec![
        SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Clip, shell_descriptor.clone())
            .with_damage(shell_descriptor.paint_bounds),
    ];

    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let second_clip_rects = second
        .draw_ops
        .iter()
        .map(|draw_op| draw_op.clip_rect)
        .collect::<Vec<_>>();

    assert_ne!(first_clip_rects, second_clip_rects);
}

#[test]
pub(crate) fn retained_compositor_prunes_removed_layers_and_packets() {
    let removed_id = WidgetId::new(61);
    let replacement_id = WidgetId::new(62);

    let mut first_scene = Scene::new();
    first_scene.push(SceneCommand::Layer(SceneLayer::new(
        removed_id,
        Rect::new(0.0, 0.0, 24.0, 24.0),
        {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(0.0, 0.0, 24.0, 24.0),
                brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
            });
            scene
        },
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(23),
        viewport: Size::new(64.0, 64.0),
        surface_size: Size::new(64.0, 64.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: content_updates([removed_id]),
        scene: first_scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
    let removed_layer_id = SceneLayerId::from_widget(removed_id);
    let replacement_layer_id = SceneLayerId::from_widget(replacement_id);
    let removed_packet_id = RetainedPacketId {
        container: CompositionContainerId::Layer(removed_layer_id),
        segment_index: 0,
    };
    assert!(compositor.layers.contains_key(&removed_layer_id));
    assert!(compositor.packets.contains_key(&removed_packet_id));

    let mut second_scene = Scene::new();
    second_scene.push(SceneCommand::Layer(SceneLayer::new(
        replacement_id,
        Rect::new(8.0, 8.0, 24.0, 24.0),
        {
            let mut scene = Scene::new();
            scene.push(SceneCommand::FillRect {
                rect: Rect::new(8.0, 8.0, 24.0, 24.0),
                brush: Color::rgba(0.0, 1.0, 0.0, 1.0).into(),
            });
            scene
        },
    )));
    frame.scene = second_scene;
    frame.layer_updates = content_updates([replacement_id]);

    let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert!(!compositor.layers.contains_key(&removed_layer_id));
    assert!(!compositor.packets.contains_key(&removed_packet_id));
    assert!(compositor.layers.contains_key(&replacement_layer_id));
    assert!(compositor.packets.contains_key(&RetainedPacketId {
        container: CompositionContainerId::Layer(replacement_layer_id),
        segment_index: 0,
    }));
}

#[test]
pub(crate) fn retained_compositor_routes_descendant_damage_into_cached_parent_packets() {
    let parent_id = WidgetId::new(81);
    let child_id = WidgetId::new(82);

    let parent_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(parent_id),
        parent_id,
        Rect::new(0.0, 0.0, 512.0, 128.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_cache_policy(LayerCachePolicy::Cached);
    let child_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(child_id),
        child_id,
        Rect::new(300.0, 24.0, 48.0, 48.0),
    );

    let build_parent_scene = |child_brush: Color| {
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(300.0, 24.0, 48.0, 48.0),
            brush: child_brush.into(),
        });

        let mut parent_scene = Scene::new();
        parent_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 512.0, 128.0),
            brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
        });
        parent_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            child_descriptor.clone(),
            child_scene,
        )));
        parent_scene
    };

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        parent_descriptor.clone(),
        build_parent_scene(Color::rgba(1.0, 0.0, 0.0, 1.0)),
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(32),
        viewport: Size::new(512.0, 128.0),
        surface_size: Size::new(512.0, 128.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                parent_descriptor.clone(),
            )
            .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                child_descriptor.clone(),
            )
            .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert!(frame.scene.replace_layer(
        parent_id,
        SceneLayer::from_descriptor(
            parent_descriptor.clone(),
            build_parent_scene(Color::rgba(0.0, 1.0, 0.0, 1.0)),
        ),
    ));
    frame.layer_updates = vec![
        SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor.clone())
            .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
    ];

    let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
}

#[test]
pub(crate) fn retained_compositor_routes_descendant_transform_into_cached_parent_packets() {
    let parent_id = WidgetId::new(181);
    let child_id = WidgetId::new(182);

    let parent_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(parent_id),
        parent_id,
        Rect::new(0.0, 0.0, 512.0, 128.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_cache_policy(LayerCachePolicy::Cached);

    let child_descriptor = |x: f32| {
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            Rect::new(x, 24.0, 48.0, 48.0),
        )
        .with_content_bounds(Rect::new(x, 24.0, 48.0, 48.0))
        .with_paint_bounds(Rect::new(x, 24.0, 48.0, 48.0))
        .with_cache_policy(LayerCachePolicy::Direct)
    };

    let build_parent_scene = |child_x: f32| {
        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(child_x, 24.0, 48.0, 48.0),
            brush: Color::rgba(0.84, 0.32, 0.18, 1.0).into(),
        });

        let mut parent_scene = Scene::new();
        parent_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 512.0, 128.0),
            brush: Color::rgba(0.1, 0.1, 0.1, 1.0).into(),
        });
        parent_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            child_descriptor(child_x),
            child_scene,
        )));
        parent_scene
    };

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        parent_descriptor.clone(),
        build_parent_scene(300.0),
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(132),
        viewport: Size::new(512.0, 128.0),
        surface_size: Size::new(512.0, 128.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                parent_descriptor.clone(),
            )
            .with_damage(Rect::new(0.0, 0.0, 512.0, 128.0)),
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                child_descriptor(300.0),
            )
            .with_damage(Rect::new(300.0, 24.0, 48.0, 48.0)),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert!(frame.scene.replace_layer(
        parent_id,
        SceneLayer::from_descriptor(parent_descriptor.clone(), build_parent_scene(340.0)),
    ));
    frame.layer_updates = vec![
        SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Transform, child_descriptor(340.0))
            .with_damage(Rect::new(300.0, 24.0, 88.0, 48.0)),
    ];

    let _ = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();
}

#[test]
pub(crate) fn retained_compositor_routes_nested_cached_descendant_damage_into_packet_owner() {
    let outer_id = WidgetId::new(83);
    let inner_id = WidgetId::new(84);
    let leaf_id = WidgetId::new(85);

    let outer_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(outer_id),
        outer_id,
        Rect::new(0.0, 0.0, 512.0, 128.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 128.0))
    .with_cache_policy(LayerCachePolicy::Cached);
    let inner_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(inner_id),
        inner_id,
        Rect::new(256.0, 0.0, 256.0, 128.0),
    )
    .with_content_bounds(Rect::new(256.0, 0.0, 256.0, 128.0))
    .with_paint_bounds(Rect::new(256.0, 0.0, 256.0, 128.0))
    .with_cache_policy(LayerCachePolicy::Cached);
    let leaf_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(leaf_id),
        leaf_id,
        Rect::new(300.0, 24.0, 48.0, 48.0),
    )
    .with_content_bounds(Rect::new(300.0, 24.0, 48.0, 48.0))
    .with_paint_bounds(Rect::new(300.0, 24.0, 48.0, 48.0));

    let build_scene = |leaf_brush: Color| {
        let mut leaf_scene = Scene::new();
        leaf_scene.push(SceneCommand::FillRect {
            rect: leaf_descriptor.bounds,
            brush: leaf_brush.into(),
        });

        let mut inner_scene = Scene::new();
        inner_scene.push(SceneCommand::FillRect {
            rect: inner_descriptor.bounds,
            brush: Color::rgba(0.12, 0.12, 0.16, 1.0).into(),
        });
        inner_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            leaf_descriptor.clone(),
            leaf_scene,
        )));

        let mut outer_scene = Scene::new();
        outer_scene.push(SceneCommand::FillRect {
            rect: outer_descriptor.bounds,
            brush: Color::rgba(0.08, 0.08, 0.10, 1.0).into(),
        });
        outer_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            inner_descriptor.clone(),
            inner_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            outer_descriptor.clone(),
            outer_scene,
        )));
        scene
    };

    let mut frame = SceneFrame {
        window_id: WindowId::new(36),
        viewport: Size::new(512.0, 128.0),
        surface_size: Size::new(512.0, 128.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                outer_descriptor.clone(),
            )
            .with_damage(outer_descriptor.paint_bounds),
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                inner_descriptor.clone(),
            )
            .with_damage(inner_descriptor.paint_bounds),
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                leaf_descriptor.clone(),
            )
            .with_damage(leaf_descriptor.paint_bounds),
        ],
        scene: build_scene(Color::rgba(1.0, 0.0, 0.0, 1.0)),
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    frame.scene = build_scene(Color::rgba(0.0, 1.0, 0.0, 1.0));
    frame.layer_updates = vec![
        SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, leaf_descriptor.clone())
            .with_damage(leaf_descriptor.paint_bounds),
    ];

    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert_ne!(first.scene_vertices, second.scene_vertices);
}

#[test]
pub(crate) fn retained_packets_match_direct_text_across_packet_boundaries() {
    let handle = FontHandle::new(27);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let layer_id = WidgetId::new(92);
    let build_frame = |cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(layer_id),
            layer_id,
            Rect::new(0.0, 0.0, 512.0, 72.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 72.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 72.0))
        .with_cache_policy(cache_policy);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 512.0, 72.0),
            brush: Color::rgba(0.08, 0.09, 0.11, 1.0).into(),
        });
        layer_scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(332.0, 18.0, 156.0, 28.0),
            text: "boundary glyph sample".to_string(),
            style: TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(92),
            viewport: Size::new(512.0, 72.0),
            surface_size: Size::new(512.0, 72.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 72.0)),
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

#[test]
pub(crate) fn retained_ancestors_match_direct_for_child_layer_text_across_packet_boundaries() {
    let handle = FontHandle::new(28);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let shell_id = WidgetId::new(93);
    let child_id = WidgetId::new(94);
    let build_frame = |shell_cache_policy| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 512.0, 84.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
        .with_cache_policy(shell_cache_policy);

        let child_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            Rect::new(0.0, 0.0, 512.0, 84.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 512.0, 84.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 512.0, 84.0),
            brush: Color::rgba(0.08, 0.09, 0.11, 1.0).into(),
        });
        child_scene.push(SceneCommand::DrawText(TextRun {
            rect: Rect::new(326.0, 22.0, 164.0, 28.0),
            text: "tab boundary sample".to_string(),
            style: TextStyle {
                font: Some(handle),
                ..TextStyle::new(Color::WHITE)
            },
        }));

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            child_descriptor.clone(),
            child_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));

        SceneFrame {
            window_id: WindowId::new(93),
            viewport: Size::new(512.0, 84.0),
            surface_size: Size::new(512.0, 84.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, shell_descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 84.0)),
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 512.0, 84.0)),
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

#[test]
pub(crate) fn retained_packets_match_direct_for_theme_preview_style_cards() {
    let handle = FontHandle::new(151);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let widget_id = WidgetId::new(152);
    let build_frame = |window_id, cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(widget_id),
            widget_id,
            Rect::new(0.0, 0.0, 640.0, 220.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 640.0, 220.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 640.0, 220.0))
        .with_cache_policy(cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 640.0, 220.0),
            brush: Color::rgba(0.94, 0.95, 0.98, 1.0).into(),
        });

        let card_specs = [
            (
                56.0,
                Color::rgba(0.99, 0.99, 1.0, 1.0),
                Color::rgba(0.19, 0.46, 0.91, 1.0),
                Color::rgba(0.15, 0.73, 0.70, 1.0),
                Color::rgba(0.10, 0.13, 0.19, 1.0),
                Color::rgba(0.39, 0.45, 0.54, 1.0),
                Color::rgba(0.82, 0.85, 0.91, 1.0),
                "Light theme",
            ),
            (
                344.0,
                Color::rgba(0.14, 0.16, 0.21, 1.0),
                Color::rgba(0.45, 0.60, 0.98, 1.0),
                Color::rgba(0.96, 0.54, 0.31, 1.0),
                Color::rgba(0.94, 0.95, 0.98, 1.0),
                Color::rgba(0.68, 0.72, 0.80, 1.0),
                Color::rgba(0.28, 0.31, 0.38, 1.0),
                "Dark theme",
            ),
        ];

        for (card_x, surface, accent, secondary, text_color, subtle_text, border, title) in
            card_specs
        {
            let card_rect = Rect::new(card_x, 24.0, 240.0, 172.0);
            layer_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(card_rect, 18.0),
                brush: surface.into(),
            });
            layer_scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(card_rect, 18.0),
                brush: border.into(),
                stroke: StrokeStyle::new(1.0),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 20.0, 44.0, 172.0, 24.0),
                text: title.to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 18.0,
                    line_height: 22.0,
                    color: text_color,
                    ..Default::default()
                },
            }));
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 20.0, 76.0, 188.0, 20.0),
                text: "Retained packets must match direct".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 13.0,
                    line_height: 18.0,
                    color: subtle_text,
                    ..Default::default()
                },
            }));

            let swatch_colors = [Color::rgba(0.84, 0.87, 0.92, 1.0), accent, secondary];
            for (index, swatch_color) in swatch_colors.into_iter().enumerate() {
                let swatch_rect =
                    Rect::new(card_x + 24.0 + (index as f32 * 72.0), 124.0, 60.0, 32.0);
                layer_scene.push(SceneCommand::FillPath {
                    path: Path::rounded_rect(swatch_rect, 10.0),
                    brush: swatch_color.into(),
                });
                layer_scene.push(SceneCommand::StrokePath {
                    path: Path::rounded_rect(swatch_rect, 10.0),
                    brush: border.into(),
                    stroke: StrokeStyle::new(1.0),
                });
            }
        }

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(640.0, 220.0),
            surface_size: Size::new(640.0, 220.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 640.0, 220.0)),
            ],
            scene,
            font_registry: Arc::new(fonts.clone()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(WindowId::new(152), LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(WindowId::new(153), LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn retained_packets_match_direct_for_theme_preview_style_cards_at_fractional_scale() {
    let handle = FontHandle::new(153);
    let mut fonts = FontRegistry::new();
    fonts.insert(handle, load_test_font());

    let widget_id = WidgetId::new(154);
    let build_frame = |window_id, cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(widget_id),
            widget_id,
            Rect::new(0.0, 0.0, 640.0, 220.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 640.0, 220.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 640.0, 220.0))
        .with_cache_policy(cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 640.0, 220.0),
            brush: Color::rgba(0.94, 0.95, 0.98, 1.0).into(),
        });

        let card_specs = [
            (
                56.0,
                Color::rgba(0.99, 0.99, 1.0, 1.0),
                Color::rgba(0.19, 0.46, 0.91, 1.0),
                Color::rgba(0.15, 0.73, 0.70, 1.0),
                Color::rgba(0.10, 0.13, 0.19, 1.0),
                Color::rgba(0.39, 0.45, 0.54, 1.0),
                Color::rgba(0.82, 0.85, 0.91, 1.0),
                "Light theme",
            ),
            (
                344.0,
                Color::rgba(0.14, 0.16, 0.21, 1.0),
                Color::rgba(0.45, 0.60, 0.98, 1.0),
                Color::rgba(0.96, 0.54, 0.31, 1.0),
                Color::rgba(0.94, 0.95, 0.98, 1.0),
                Color::rgba(0.68, 0.72, 0.80, 1.0),
                Color::rgba(0.28, 0.31, 0.38, 1.0),
                "Dark theme",
            ),
        ];

        for (card_x, surface, accent, secondary, text_color, subtle_text, border, title) in
            card_specs
        {
            let card_rect = Rect::new(card_x, 24.0, 240.0, 172.0);
            layer_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(card_rect, 18.0),
                brush: surface.into(),
            });
            layer_scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(card_rect, 18.0),
                brush: border.into(),
                stroke: StrokeStyle::new(1.0),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 20.0, 44.0, 172.0, 24.0),
                text: title.to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 18.0,
                    line_height: 22.0,
                    color: text_color,
                    ..Default::default()
                },
            }));
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 20.0, 76.0, 188.0, 20.0),
                text: format!(
                    "{} base surface with {} accent for primary actions.",
                    title.split_whitespace().next().unwrap().to_lowercase(),
                    title.split_whitespace().next().unwrap().to_lowercase(),
                ),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 13.0,
                    line_height: 18.0,
                    color: subtle_text,
                    ..Default::default()
                },
            }));
            layer_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(Rect::new(card_x + 20.0, 108.0, 220.0, 36.0), 10.0),
                brush: surface.into(),
            });
            layer_scene.push(SceneCommand::StrokePath {
                path: Path::rounded_rect(Rect::new(card_x + 20.0, 108.0, 220.0, 36.0), 10.0),
                brush: border.into(),
                stroke: StrokeStyle::new(1.0),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 36.0, 118.0, 172.0, 16.0),
                text: "Find layer, panel, or asset".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 13.0,
                    line_height: 18.0,
                    color: subtle_text,
                    ..Default::default()
                },
            }));
            layer_scene.push(SceneCommand::FillPath {
                path: Path::rounded_rect(Rect::new(card_x + 20.0, 156.0, 86.0, 28.0), 14.0),
                brush: accent.into(),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 38.0, 163.0, 48.0, 16.0),
                text: "Inspect".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 13.0,
                    line_height: 18.0,
                    color: Color::rgba(1.0, 1.0, 1.0, 1.0),
                    ..Default::default()
                },
            }));
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(card_x + 128.0, 160.0, 28.0, 16.0),
                brush: secondary.into(),
            });
            layer_scene.push(SceneCommand::DrawText(TextRun {
                rect: Rect::new(card_x + 164.0, 158.33333, 68.0, 20.0),
                text: "Live updates".to_string(),
                style: TextStyle {
                    font: Some(handle),
                    font_size: 14.0,
                    line_height: 20.0,
                    color: text_color,
                    ..Default::default()
                },
            }));
        }

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(640.0, 220.0),
            surface_size: Size::new(960.0, 330.0),
            scale_factor: 1.5,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, 0.0, 640.0, 220.0)),
            ],
            scene,
            font_registry: Arc::new(fonts.clone()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(WindowId::new(153), LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(WindowId::new(154), LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}
