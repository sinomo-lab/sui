use crate::WgpuRenderer;
use crate::retained::RetainedCompositorState;
use crate::tests::support::RGBA_CHANNEL_TOLERANCE;
use crate::tests::support::TestSceneLayerDescriptorExt;
use crate::tests::support::{
    LayerCachePolicy, assert_rgba_images_match, assert_rgba_pixel_near,
    build_translucent_scroll_child_frame, prepare_with_compositor, rgba_image_diff_count,
};
use crate::text_engine::TextEngine;
use std::sync::Arc;
use sui_core::Color;
use sui_core::ImageHandle;
use sui_core::Path;
use sui_core::Rect;
use sui_core::Size;
use sui_core::Vector;
use sui_core::WidgetId;
use sui_core::WindowId;
use sui_scene::ImageRegistry;
use sui_scene::ImageSource;
use sui_scene::LayerCompositionMode;
use sui_scene::RegisteredImage;
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

#[test]
pub(crate) fn retained_compositor_updates_nested_retained_scroll_layer_after_child_transform() {
    let shell_id = WidgetId::new(191);
    let scroll_id = WidgetId::new(192);
    let content_id = WidgetId::new(193);
    let first_id = WidgetId::new(194);
    let second_id = WidgetId::new(195);
    let third_id = WidgetId::new(196);

    let shell_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(shell_id),
        shell_id,
        Rect::new(0.0, 0.0, 360.0, 220.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 360.0, 220.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 360.0, 220.0))
    .with_cache_policy(LayerCachePolicy::Direct);

    let scroll_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(scroll_id),
        scroll_id,
        Rect::new(0.0, 0.0, 240.0, 220.0),
    )
    .with_content_bounds(Rect::new(0.0, 0.0, 240.0, 220.0))
    .with_paint_bounds(Rect::new(0.0, 0.0, 240.0, 220.0))
    .with_cache_policy(LayerCachePolicy::Cached)
    .with_composition_mode(LayerCompositionMode::Scroll);

    let content_descriptor = |y: f32| {
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(content_id),
            content_id,
            Rect::new(0.0, y, 360.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, y, 220.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, y, 220.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct)
    };

    let child_layer = |id: WidgetId, y: f32, brush: Color| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, y, 220.0, 120.0),
            brush: brush.into(),
        });
        SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(id),
                id,
                Rect::new(0.0, y, 220.0, 120.0),
            )
            .with_content_bounds(Rect::new(0.0, y, 220.0, 120.0))
            .with_paint_bounds(Rect::new(0.0, y, 220.0, 120.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            scene,
        )
    };

    let build_content_scene = |y: f32| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(child_layer(
            first_id,
            y,
            Color::rgba(0.82, 0.36, 0.18, 1.0),
        )));
        scene.push(SceneCommand::Layer(child_layer(
            second_id,
            y + 120.0,
            Color::rgba(0.18, 0.54, 0.82, 1.0),
        )));
        scene.push(SceneCommand::Layer(child_layer(
            third_id,
            y + 240.0,
            Color::rgba(0.24, 0.72, 0.36, 1.0),
        )));
        scene
    };

    let build_shell_scene = |content_y: f32| {
        let mut scroll_scene = Scene::new();
        scroll_scene.push(SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 230.0, 220.0),
        });
        scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            content_descriptor(content_y),
            build_content_scene(content_y),
        )));
        scroll_scene.push(SceneCommand::PopClip);

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            scroll_descriptor.clone(),
            scroll_scene,
        )));
        shell_scene.push(SceneCommand::FillRect {
            rect: Rect::new(240.0, 0.0, 120.0, 220.0),
            brush: Color::rgba(0.94, 0.95, 0.97, 1.0).into(),
        });
        shell_scene
    };

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        shell_descriptor.clone(),
        build_shell_scene(0.0),
    )));

    let mut frame = SceneFrame {
        window_id: WindowId::new(142),
        viewport: Size::new(360.0, 220.0),
        surface_size: Size::new(360.0, 220.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                scroll_descriptor.clone(),
            )
            .with_damage(scroll_descriptor.paint_bounds),
            SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                content_descriptor(0.0),
            )
            .with_damage(Rect::new(0.0, 0.0, 220.0, 360.0)),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut text_engine = TextEngine::new().unwrap();
    let mut compositor = RetainedCompositorState::default();
    let first_frame = frame.clone();
    let first = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert!(
        frame
            .scene
            .translate_layer(content_id, Vector::new(0.0, -72.0))
    );
    frame.layer_updates = vec![
        SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Transform,
            content_descriptor(-72.0),
        )
        .with_damage(Rect::new(0.0, -72.0, 220.0, 432.0)),
        SceneLayerUpdate::from_descriptor(
            SceneLayerUpdateKind::Transform,
            scroll_descriptor.clone(),
        )
        .with_damage(Rect::new(0.0, 0.0, 220.0, 360.0)),
    ];

    let second = prepare_with_compositor(&frame, &mut text_engine, &mut compositor).unwrap();

    assert_ne!(first.scene_vertices, second.scene_vertices);
    let mut renderer = WgpuRenderer::default();
    renderer.render(&first_frame).unwrap();
    let before = renderer
        .capture_last_frame_rgba(first_frame.window_id)
        .unwrap();
    renderer.render(&frame).unwrap();
    let after = renderer.capture_last_frame_rgba(frame.window_id).unwrap();

    assert!(
        rgba_image_diff_count(&before, &after) > 0,
        "translated retained layer did not change the captured image beyond {RGBA_CHANNEL_TOLERANCE} channel value"
    );
}

#[test]
pub(crate) fn retained_scroll_layer_matches_direct_at_fractional_packet_boundaries() {
    let widget_id = WidgetId::new(95);
    let build_frame = |window_id, cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(widget_id),
            widget_id,
            Rect::new(0.0, -150.5, 420.0, 700.0),
        )
        .with_content_bounds(Rect::new(0.0, -150.5, 420.0, 700.0))
        .with_paint_bounds(Rect::new(0.0, -150.5, 420.0, 700.0))
        .with_cache_policy(cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, -150.5, 420.0, 700.0),
            brush: Color::WHITE.into(),
        });
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(32.0, 372.0, 340.0, 18.0),
            brush: Color::rgba(0.18, 0.24, 0.34, 1.0).into(),
        });
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(350.0, 356.0, 44.0, 58.0),
            brush: Color::rgba(0.12, 0.35, 0.78, 1.0).into(),
        });
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(334.0, 392.0, 64.0, 16.0),
            brush: Color::rgba(0.88, 0.52, 0.18, 1.0).into(),
        });

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(420.0, 240.0),
            surface_size: Size::new(420.0, 240.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(0.0, -150.5, 420.0, 700.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(WindowId::new(94), LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(WindowId::new(95), LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn retained_scroll_layer_matches_direct_for_clipped_rows_across_packet_boundary() {
    let widget_id = WidgetId::new(96);
    let clip_rect = Rect::new(42.0, 628.0, 360.0, 220.0);
    let build_frame = |window_id, cache_policy| {
        let descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(widget_id),
            widget_id,
            Rect::new(24.0, -478.0, 1232.0, 2046.0),
        )
        .with_content_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
        .with_paint_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
        .with_cache_policy(cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut layer_scene = Scene::new();
        layer_scene.push(SceneCommand::FillRect {
            rect: Rect::new(24.0, -478.0, 1232.0, 2046.0),
            brush: Color::WHITE.into(),
        });
        layer_scene.push(SceneCommand::PushClip { rect: clip_rect });
        for (index, y) in [636.0, 668.0, 700.0, 732.0].into_iter().enumerate() {
            let brush = if index == 1 {
                Color::rgba(0.79, 0.86, 0.98, 1.0)
            } else {
                Color::rgba(0.90, 0.93, 0.97, 1.0)
            };
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(42.0, y, 360.0, 28.0),
                brush: brush.into(),
            });
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(58.0, y + 8.0, 172.0, 12.0),
                brush: Color::rgba(0.17, 0.21, 0.29, 1.0).into(),
            });
            layer_scene.push(SceneCommand::FillRect {
                rect: Rect::new(248.0, y + 8.0, 96.0, 12.0),
                brush: Color::rgba(0.41, 0.48, 0.58, 1.0).into(),
            });
        }
        layer_scene.push(SceneCommand::PopClip);

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor.clone(),
            layer_scene,
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(430.0, 900.0),
            surface_size: Size::new(430.0, 900.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, descriptor)
                    .with_damage(Rect::new(24.0, -478.0, 1232.0, 2046.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(WindowId::new(96), LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(WindowId::new(97), LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn retained_scroll_layer_clips_direct_packet_after_child_layer_boundary() {
    let scroll_id = WidgetId::new(209);
    let child_id = WidgetId::new(210);
    let scroll_bounds = Rect::new(0.0, 0.0, 200.0, 160.0);
    let internal_clip = Rect::new(20.0, 24.0, 120.0, 80.0);
    let scroll_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(scroll_id),
        scroll_id,
        scroll_bounds,
    )
    .with_content_bounds(scroll_bounds)
    .with_paint_bounds(scroll_bounds)
    .with_composition_mode(LayerCompositionMode::Scroll);
    let child_descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(child_id),
        child_id,
        Rect::new(32.0, 36.0, 48.0, 36.0),
    )
    .with_content_bounds(Rect::new(32.0, 36.0, 48.0, 36.0))
    .with_paint_bounds(Rect::new(32.0, 36.0, 48.0, 36.0));

    let mut child_scene = Scene::new();
    child_scene.push(SceneCommand::FillRect {
        rect: Rect::new(32.0, 36.0, 48.0, 36.0),
        brush: Color::rgba(0.0, 0.0, 1.0, 1.0).into(),
    });

    let mut scroll_scene = Scene::new();
    scroll_scene.push(SceneCommand::FillRect {
        rect: scroll_bounds,
        brush: Color::WHITE.into(),
    });
    scroll_scene.push(SceneCommand::PushClip {
        rect: internal_clip,
    });
    scroll_scene.push(SceneCommand::FillRect {
        rect: Rect::new(28.0, 36.0, 52.0, 20.0),
        brush: Color::rgba(0.90, 0.94, 0.98, 1.0).into(),
    });
    scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        child_descriptor.clone(),
        child_scene,
    )));
    scroll_scene.push(SceneCommand::FillRect {
        rect: Rect::new(28.0, 12.0, 96.0, 40.0),
        brush: Color::rgba(1.0, 0.0, 0.0, 1.0).into(),
    });
    scroll_scene.push(SceneCommand::PopClip);

    let mut scene = Scene::new();
    scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
        scroll_descriptor.clone(),
        scroll_scene,
    )));

    let frame = SceneFrame {
        window_id: WindowId::new(209),
        viewport: Size::new(200.0, 160.0),
        surface_size: Size::new(200.0, 160.0),
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: vec![
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, scroll_descriptor)
                .with_damage(scroll_bounds),
            SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor),
        ],
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };

    let mut renderer = WgpuRenderer::default();
    renderer.render(&frame).unwrap();
    let pixels = renderer.capture_last_frame_rgba(frame.window_id).unwrap();

    assert_rgba_pixel_near(&pixels, 32, 18, [255, 255, 255, 255], 2);
    assert_rgba_pixel_near(&pixels, 32, 30, [255, 0, 0, 255], 2);
}

#[test]
pub(crate) fn cached_scroll_ancestor_matches_direct_for_clipped_child_layer_rows() {
    let shell_id = WidgetId::new(97);
    let child_id = WidgetId::new(98);
    let clip_rect = Rect::new(42.0, 628.0, 360.0, 220.0);
    let build_frame = |window_id, shell_cache_policy| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(24.0, -478.0, 1232.0, 2046.0),
        )
        .with_content_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
        .with_paint_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
        .with_cache_policy(shell_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let child_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            Rect::new(41.5, 627.5, 361.0, 221.0),
        )
        .with_content_bounds(Rect::new(41.5, 627.5, 361.0, 221.0))
        .with_paint_bounds(Rect::new(41.5, 627.5, 361.0, 221.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(41.5, 627.5, 361.0, 221.0),
            brush: Color::WHITE.into(),
        });
        child_scene.push(SceneCommand::PushClip { rect: clip_rect });
        for (index, y) in [636.0, 668.0, 700.0, 732.0].into_iter().enumerate() {
            let brush = if index == 1 {
                Color::rgba(0.79, 0.86, 0.98, 1.0)
            } else {
                Color::rgba(0.90, 0.93, 0.97, 1.0)
            };
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(42.0, y, 360.0, 28.0),
                brush: brush.into(),
            });
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(58.0, y + 8.0, 172.0, 12.0),
                brush: Color::rgba(0.17, 0.21, 0.29, 1.0).into(),
            });
            child_scene.push(SceneCommand::FillRect {
                rect: Rect::new(248.0, y + 8.0, 96.0, 12.0),
                brush: Color::rgba(0.41, 0.48, 0.58, 1.0).into(),
            });
        }
        child_scene.push(SceneCommand::PopClip);

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
            window_id,
            viewport: Size::new(430.0, 900.0),
            surface_size: Size::new(430.0, 900.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, shell_descriptor)
                    .with_damage(Rect::new(24.0, -478.0, 1232.0, 2046.0)),
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor)
                    .with_damage(Rect::new(41.5, 627.5, 361.0, 221.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(WindowId::new(98), LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(WindowId::new(99), LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_scroll_ancestor_clips_fully_outside_child_layer() {
    let shell_id = WidgetId::new(100);
    let child_id = WidgetId::new(101);
    let clip_rect = Rect::new(42.0, 628.0, 360.0, 220.0);
    let build_frame = |window_id, shell_cache_policy| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(24.0, -478.0, 1232.0, 2046.0),
        )
        .with_content_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
        .with_paint_bounds(Rect::new(24.0, -478.0, 1232.0, 2046.0))
        .with_cache_policy(shell_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let child_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(child_id),
            child_id,
            Rect::new(96.0, 904.0, 180.0, 96.0),
        )
        .with_content_bounds(Rect::new(96.0, 904.0, 180.0, 96.0))
        .with_paint_bounds(Rect::new(96.0, 904.0, 180.0, 96.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut child_scene = Scene::new();
        child_scene.push(SceneCommand::FillRect {
            rect: Rect::new(96.0, 904.0, 180.0, 96.0),
            brush: Color::rgba(0.82, 0.16, 0.18, 1.0).into(),
        });

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: Rect::new(24.0, -478.0, 1232.0, 2046.0),
            brush: Color::WHITE.into(),
        });
        shell_scene.push(SceneCommand::PushClip { rect: clip_rect });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            child_descriptor.clone(),
            child_scene,
        )));
        shell_scene.push(SceneCommand::PopClip);

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(430.0, 900.0),
            surface_size: Size::new(430.0, 900.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, shell_descriptor)
                    .with_damage(Rect::new(24.0, -478.0, 1232.0, 2046.0)),
                SceneLayerUpdate::from_descriptor(SceneLayerUpdateKind::Content, child_descriptor)
                    .with_damage(Rect::new(96.0, 904.0, 180.0, 96.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();
    let direct = build_frame(WindowId::new(100), LayerCachePolicy::Direct);
    renderer.render(&direct).unwrap();
    let direct_pixels = renderer.capture_last_frame_rgba(direct.window_id).unwrap();

    let cached = build_frame(WindowId::new(101), LayerCachePolicy::Cached);
    renderer.render(&cached).unwrap();
    let cached_pixels = renderer.capture_last_frame_rgba(cached.window_id).unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_scroll_ancestor_matches_direct_after_child_translation() {
    let shell_id = WidgetId::new(102);
    let scroll_id = WidgetId::new(103);
    let content_id = WidgetId::new(104);
    let first_id = WidgetId::new(105);
    let second_id = WidgetId::new(106);
    let third_id = WidgetId::new(107);
    let clip_rect = Rect::new(42.0, 60.0, 360.0, 220.0);

    let content_descriptor = |y: f32| {
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(content_id),
            content_id,
            Rect::new(42.0, y, 360.0, 360.0),
        )
        .with_content_bounds(Rect::new(42.0, y, 360.0, 360.0))
        .with_paint_bounds(Rect::new(42.0, y, 360.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct)
    };

    let child_layer = |id: WidgetId, y: f32, brush: Color| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(42.0, y, 360.0, 96.0),
            brush: brush.into(),
        });
        SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(id),
                id,
                Rect::new(42.0, y, 360.0, 96.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 360.0, 96.0))
            .with_paint_bounds(Rect::new(42.0, y, 360.0, 96.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            scene,
        )
    };

    let build_content_scene = |y: f32| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(child_layer(
            first_id,
            y,
            Color::rgba(0.82, 0.36, 0.18, 1.0),
        )));
        scene.push(SceneCommand::Layer(child_layer(
            second_id,
            y + 120.0,
            Color::rgba(0.18, 0.54, 0.82, 1.0),
        )));
        scene.push(SceneCommand::Layer(child_layer(
            third_id,
            y + 240.0,
            Color::rgba(0.24, 0.72, 0.36, 1.0),
        )));
        scene
    };

    let build_frame = |window_id, scroll_cache_policy, content_y: f32, update_kind| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 430.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(scroll_id),
            scroll_id,
            Rect::new(24.0, 24.0, 382.0, 292.0),
        )
        .with_content_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
        .with_paint_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
        .with_cache_policy(scroll_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut scroll_scene = Scene::new();
        scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            content_descriptor(content_y),
            build_content_scene(content_y),
        )));

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 430.0, 360.0),
            brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
        });
        shell_scene.push(SceneCommand::PushClip { rect: clip_rect });
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

        let update = if update_kind == SceneLayerUpdateKind::Content {
            SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                .with_damage(Rect::new(42.0, content_y, 360.0, 360.0))
        } else {
            SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                .with_damage(Rect::new(42.0, 0.0, 360.0, 432.0))
        };

        SceneFrame {
            window_id,
            viewport: Size::new(430.0, 360.0),
            surface_size: Size::new(430.0, 360.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![update],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_frame(
        WindowId::new(110),
        LayerCachePolicy::Direct,
        0.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_frame(
        WindowId::new(110),
        LayerCachePolicy::Direct,
        72.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_frame(
        WindowId::new(111),
        LayerCachePolicy::Cached,
        0.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_frame(
        WindowId::new(111),
        LayerCachePolicy::Cached,
        72.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_scroll_internal_clip_matches_direct_after_child_translation() {
    let shell_id = WidgetId::new(108);
    let scroll_id = WidgetId::new(109);
    let content_id = WidgetId::new(110);
    let first_id = WidgetId::new(111);
    let second_id = WidgetId::new(112);
    let third_id = WidgetId::new(113);
    let clip_rect = Rect::new(42.0, 60.0, 360.0, 220.0);

    let content_descriptor = |y: f32| {
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(content_id),
            content_id,
            Rect::new(42.0, y, 360.0, 360.0),
        )
        .with_content_bounds(Rect::new(42.0, y, 360.0, 360.0))
        .with_paint_bounds(Rect::new(42.0, y, 360.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct)
    };

    let child_layer = |id: WidgetId, y: f32, brush: Color| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(42.0, y, 360.0, 96.0),
            brush: brush.into(),
        });
        SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(id),
                id,
                Rect::new(42.0, y, 360.0, 96.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 360.0, 96.0))
            .with_paint_bounds(Rect::new(42.0, y, 360.0, 96.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            scene,
        )
    };

    let build_content_scene = |y: f32| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(child_layer(
            first_id,
            y,
            Color::rgba(0.82, 0.36, 0.18, 1.0),
        )));
        scene.push(SceneCommand::Layer(child_layer(
            second_id,
            y + 120.0,
            Color::rgba(0.18, 0.54, 0.82, 1.0),
        )));
        scene.push(SceneCommand::Layer(child_layer(
            third_id,
            y + 240.0,
            Color::rgba(0.24, 0.72, 0.36, 1.0),
        )));
        scene
    };

    let build_frame = |window_id, scroll_cache_policy, content_y: f32, update_kind| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 430.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(scroll_id),
            scroll_id,
            Rect::new(24.0, 24.0, 382.0, 292.0),
        )
        .with_content_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
        .with_paint_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
        .with_cache_policy(scroll_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut scroll_scene = Scene::new();
        scroll_scene.push(SceneCommand::PushClip { rect: clip_rect });
        scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            content_descriptor(content_y),
            build_content_scene(content_y),
        )));
        scroll_scene.push(SceneCommand::PopClip);

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 430.0, 360.0),
            brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
        });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            scroll_descriptor.clone(),
            scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));

        let update = if update_kind == SceneLayerUpdateKind::Content {
            SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                .with_damage(Rect::new(42.0, content_y, 360.0, 360.0))
        } else {
            SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                .with_damage(Rect::new(42.0, 0.0, 360.0, 432.0))
        };

        SceneFrame {
            window_id,
            viewport: Size::new(430.0, 360.0),
            surface_size: Size::new(430.0, 360.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![update],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::new(ImageRegistry::new()),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_frame(
        WindowId::new(112),
        LayerCachePolicy::Direct,
        0.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_frame(
        WindowId::new(112),
        LayerCachePolicy::Direct,
        72.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_frame(
        WindowId::new(113),
        LayerCachePolicy::Cached,
        0.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_frame(
        WindowId::new(113),
        LayerCachePolicy::Cached,
        72.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_scroll_translucent_auto_child_matches_direct_after_translation() {
    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_translucent_scroll_child_frame(
        WindowId::new(214),
        LayerCachePolicy::Direct,
        LayerCachePolicy::Direct,
        60.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_translucent_scroll_child_frame(
        WindowId::new(214),
        LayerCachePolicy::Direct,
        LayerCachePolicy::Direct,
        132.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_translucent_scroll_child_frame(
        WindowId::new(215),
        LayerCachePolicy::Cached,
        LayerCachePolicy::Auto,
        60.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_translucent_scroll_child_frame(
        WindowId::new(215),
        LayerCachePolicy::Cached,
        LayerCachePolicy::Auto,
        132.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_scroll_translucent_direct_child_matches_direct_after_translation() {
    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_translucent_scroll_child_frame(
        WindowId::new(216),
        LayerCachePolicy::Direct,
        LayerCachePolicy::Direct,
        60.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_translucent_scroll_child_frame(
        WindowId::new(216),
        LayerCachePolicy::Direct,
        LayerCachePolicy::Direct,
        132.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_translucent_scroll_child_frame(
        WindowId::new(217),
        LayerCachePolicy::Cached,
        LayerCachePolicy::Direct,
        60.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_translucent_scroll_child_frame(
        WindowId::new(217),
        LayerCachePolicy::Cached,
        LayerCachePolicy::Direct,
        132.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn cached_scroll_internal_clip_image_matches_direct_after_child_translation() {
    let shell_id = WidgetId::new(114);
    let scroll_id = WidgetId::new(115);
    let content_id = WidgetId::new(116);
    let image_layer_id = WidgetId::new(117);
    let filler_id = WidgetId::new(118);
    let image_handle = ImageHandle::new(41);
    let clip_rect = Rect::new(42.0, 60.0, 360.0, 220.0);

    let mut images = ImageRegistry::new();
    images.insert(
        image_handle,
        RegisteredImage::from_rgba8(
            2,
            2,
            vec![
                220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64, 255,
            ],
        )
        .unwrap(),
    );
    let images = Arc::new(images);

    let content_descriptor = |y: f32| {
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(content_id),
            content_id,
            Rect::new(42.0, y, 360.0, 360.0),
        )
        .with_content_bounds(Rect::new(42.0, y, 360.0, 360.0))
        .with_paint_bounds(Rect::new(42.0, y, 360.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct)
    };

    let image_layer = |y: f32| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawImage {
            rect: Rect::new(42.0, y, 220.0, 220.0),
            source: ImageSource::new(image_handle),
        });
        SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(image_layer_id),
                image_layer_id,
                Rect::new(42.0, y, 220.0, 220.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 220.0, 220.0))
            .with_paint_bounds(Rect::new(42.0, y, 220.0, 220.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            scene,
        )
    };

    let filler_layer = |y: f32| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(42.0, y, 360.0, 96.0),
            brush: Color::rgba(0.24, 0.72, 0.36, 1.0).into(),
        });
        SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(filler_id),
                filler_id,
                Rect::new(42.0, y, 360.0, 96.0),
            )
            .with_content_bounds(Rect::new(42.0, y, 360.0, 96.0))
            .with_paint_bounds(Rect::new(42.0, y, 360.0, 96.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            scene,
        )
    };

    let build_content_scene = |y: f32| {
        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(image_layer(y)));
        scene.push(SceneCommand::Layer(filler_layer(y + 240.0)));
        scene
    };

    let build_frame = |window_id, scroll_cache_policy, content_y: f32, update_kind| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 430.0, 360.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 430.0, 360.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(scroll_id),
            scroll_id,
            Rect::new(24.0, 24.0, 382.0, 292.0),
        )
        .with_content_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
        .with_paint_bounds(Rect::new(24.0, 24.0, 382.0, 292.0))
        .with_cache_policy(scroll_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let mut scroll_scene = Scene::new();
        scroll_scene.push(SceneCommand::PushClip { rect: clip_rect });
        scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            content_descriptor(content_y),
            build_content_scene(content_y),
        )));
        scroll_scene.push(SceneCommand::PopClip);

        let mut shell_scene = Scene::new();
        shell_scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 430.0, 360.0),
            brush: Color::rgba(0.95, 0.97, 0.99, 1.0).into(),
        });
        shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            scroll_descriptor.clone(),
            scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor.clone(),
            shell_scene,
        )));

        let update = if update_kind == SceneLayerUpdateKind::Content {
            SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                .with_damage(Rect::new(42.0, content_y, 360.0, 360.0))
        } else {
            SceneLayerUpdate::from_descriptor(update_kind, content_descriptor(content_y))
                .with_damage(Rect::new(42.0, 0.0, 360.0, 460.0))
        };

        SceneFrame {
            window_id,
            viewport: Size::new(430.0, 360.0),
            surface_size: Size::new(430.0, 360.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![update],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_frame(
        WindowId::new(114),
        LayerCachePolicy::Direct,
        0.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_frame(
        WindowId::new(114),
        LayerCachePolicy::Direct,
        72.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_frame(
        WindowId::new(115),
        LayerCachePolicy::Cached,
        0.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_frame(
        WindowId::new(115),
        LayerCachePolicy::Cached,
        72.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn nested_cached_scroll_image_outside_parent_bounds_matches_direct_render() {
    let shell_id = WidgetId::new(119);
    let outer_scroll_id = WidgetId::new(120);
    let inner_scroll_id = WidgetId::new(121);
    let image_layer_id = WidgetId::new(122);
    let image_handle = ImageHandle::new(42);

    let mut images = ImageRegistry::new();
    images.insert(
        image_handle,
        RegisteredImage::from_rgba8(
            2,
            2,
            vec![
                220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64, 255,
            ],
        )
        .unwrap(),
    );
    let images = Arc::new(images);

    let build_frame = |window_id, inner_cache_policy| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 1280.0, 720.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let outer_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(outer_scroll_id),
            outer_scroll_id,
            Rect::new(320.0, 28.0, 428.0, 336.0),
        )
        .with_content_bounds(Rect::new(320.0, 28.0, 428.0, 336.0))
        .with_paint_bounds(Rect::new(320.0, 28.0, 428.0, 336.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let inner_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(inner_scroll_id),
            inner_scroll_id,
            Rect::new(321.0, 60.0, 426.0, 303.0),
        )
        .with_content_bounds(Rect::new(321.0, 60.0, 426.0, 303.0))
        .with_paint_bounds(Rect::new(321.0, 60.0, 426.0, 303.0))
        .with_cache_policy(inner_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll)
        .with_is_stack_surface(true);

        let mut image_scene = Scene::new();
        image_scene.push(SceneCommand::DrawImage {
            rect: Rect::new(363.0, 376.0, 220.0, 220.0),
            source: ImageSource::new(image_handle),
        });
        let image_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(image_layer_id),
                image_layer_id,
                Rect::new(363.0, 376.0, 220.0, 220.0),
            )
            .with_content_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
            .with_paint_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            image_scene,
        );

        let mut inner_scroll_scene = Scene::new();
        inner_scroll_scene.push(SceneCommand::FillRect {
            rect: Rect::new(321.0, 60.0, 426.0, 303.0),
            brush: Color::rgba(0.96, 0.97, 0.99, 1.0).into(),
        });
        inner_scroll_scene.push(SceneCommand::Layer(image_layer));

        let mut outer_scroll_scene = Scene::new();
        outer_scroll_scene.push(SceneCommand::FillRect {
            rect: Rect::new(320.0, 28.0, 428.0, 336.0),
            brush: Color::rgba(0.96, 0.97, 0.99, 1.0).into(),
        });
        outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            inner_scroll_descriptor,
            inner_scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
            brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
        });
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor,
            {
                let mut shell_scene = Scene::new();
                shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                    outer_scroll_descriptor,
                    outer_scroll_scene,
                )));
                shell_scene
            },
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(1280.0, 720.0),
            surface_size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(363.0, 376.0, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
                .with_paint_bounds(Rect::new(363.0, 376.0, 220.0, 220.0))
                .with_cache_policy(LayerCachePolicy::Direct),
            )],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_frame = build_frame(WindowId::new(116), LayerCachePolicy::Direct);
    renderer.render(&direct_frame).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_frame.window_id)
        .unwrap();

    let cached_frame = build_frame(WindowId::new(117), LayerCachePolicy::Cached);
    renderer.render(&cached_frame).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_frame.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn nested_cached_scroll_large_paint_bounds_image_matches_direct_render() {
    let shell_id = WidgetId::new(123);
    let outer_scroll_id = WidgetId::new(124);
    let inner_scroll_id = WidgetId::new(125);
    let image_layer_id = WidgetId::new(126);
    let image_handle = ImageHandle::new(43);

    let mut images = ImageRegistry::new();
    images.insert(
        image_handle,
        RegisteredImage::from_rgba8(
            2,
            2,
            vec![
                220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64, 255,
            ],
        )
        .unwrap(),
    );
    let images = Arc::new(images);

    let build_frame = |window_id, inner_cache_policy| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 1280.0, 720.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let outer_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(outer_scroll_id),
            outer_scroll_id,
            Rect::new(320.0, 28.0, 428.0, 336.0),
        )
        .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let inner_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(inner_scroll_id),
            inner_scroll_id,
            Rect::new(321.0, 60.0, 426.0, 303.0),
        )
        .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_cache_policy(inner_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll)
        .with_is_stack_surface(true);

        let mut image_scene = Scene::new();
        image_scene.push(SceneCommand::DrawImage {
            rect: Rect::new(363.0, 376.0, 220.0, 220.0),
            source: ImageSource::new(image_handle),
        });
        let image_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(image_layer_id),
                image_layer_id,
                Rect::new(363.0, 376.0, 220.0, 220.0),
            )
            .with_content_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
            .with_paint_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            image_scene,
        );

        let mut inner_scroll_scene = Scene::new();
        inner_scroll_scene.push(SceneCommand::Layer(image_layer));

        let mut outer_scroll_scene = Scene::new();
        outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            inner_scroll_descriptor,
            inner_scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
            brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
        });
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor,
            {
                let mut shell_scene = Scene::new();
                shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                    outer_scroll_descriptor,
                    outer_scroll_scene,
                )));
                shell_scene
            },
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(1280.0, 720.0),
            surface_size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![SceneLayerUpdate::from_descriptor(
                SceneLayerUpdateKind::Content,
                SceneLayerDescriptor::new(
                    SceneLayerId::from_widget(image_layer_id),
                    image_layer_id,
                    Rect::new(363.0, 376.0, 220.0, 220.0),
                )
                .with_content_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
                .with_paint_bounds(Rect::new(362.5, 375.5, 221.0, 221.0))
                .with_cache_policy(LayerCachePolicy::Direct),
            )],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_frame = build_frame(WindowId::new(118), LayerCachePolicy::Direct);
    renderer.render(&direct_frame).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_frame.window_id)
        .unwrap();

    let cached_frame = build_frame(WindowId::new(119), LayerCachePolicy::Cached);
    renderer.render(&cached_frame).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_frame.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn nested_cached_scroll_internal_clip_matches_direct_after_image_scroll() {
    let shell_id = WidgetId::new(127);
    let outer_scroll_id = WidgetId::new(128);
    let inner_scroll_id = WidgetId::new(129);
    let image_layer_id = WidgetId::new(130);
    let image_handle = ImageHandle::new(44);

    let mut images = ImageRegistry::new();
    images.insert(
        image_handle,
        RegisteredImage::from_rgba8(
            2,
            2,
            vec![
                220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64, 255,
            ],
        )
        .unwrap(),
    );
    let images = Arc::new(images);

    let build_frame = |window_id, inner_cache_policy, image_y, update_kind| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 1280.0, 720.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let outer_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(outer_scroll_id),
            outer_scroll_id,
            Rect::new(320.0, 28.0, 428.0, 336.0),
        )
        .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let inner_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(inner_scroll_id),
            inner_scroll_id,
            Rect::new(321.0, 60.0, 426.0, 303.0),
        )
        .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_cache_policy(inner_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll)
        .with_is_stack_surface(true);

        let image_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(image_layer_id),
            image_layer_id,
            Rect::new(363.0, image_y, 220.0, 220.0),
        )
        .with_content_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
        .with_paint_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let mut image_scene = Scene::new();
        image_scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
            brush: Color::rgba(0.965, 0.975, 0.99, 1.0).into(),
        });
        image_scene.push(SceneCommand::PushClip {
            rect: Rect::new(363.0, image_y, 220.0, 220.0),
        });
        image_scene.push(SceneCommand::DrawImage {
            rect: Rect::new(363.0, image_y, 220.0, 220.0),
            source: ImageSource::new(image_handle),
        });
        image_scene.push(SceneCommand::PopClip);
        image_scene.push(SceneCommand::StrokePath {
            path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
            brush: Color::rgba(0.8335978, 0.8335974, 0.835042, 1.0).into(),
            stroke: StrokeStyle::new(1.0),
        });
        let image_layer = SceneLayer::from_descriptor(image_descriptor.clone(), image_scene);

        let mut inner_scroll_scene = Scene::new();
        inner_scroll_scene.push(SceneCommand::PushClip {
            rect: Rect::new(321.0, 60.0, 426.0, 303.0),
        });
        inner_scroll_scene.push(SceneCommand::Layer(image_layer));
        inner_scroll_scene.push(SceneCommand::PopClip);

        let mut outer_scroll_scene = Scene::new();
        outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            inner_scroll_descriptor,
            inner_scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
            brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
        });
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor,
            {
                let mut shell_scene = Scene::new();
                shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                    outer_scroll_descriptor,
                    outer_scroll_scene,
                )));
                shell_scene
            },
        )));

        let update = SceneLayerUpdate::from_descriptor(update_kind, image_descriptor.clone())
            .with_damage(Rect::new(362.5, 139.5, 221.0, 457.0));

        SceneFrame {
            window_id,
            viewport: Size::new(1280.0, 720.0),
            surface_size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![update],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_frame(
        WindowId::new(120),
        LayerCachePolicy::Direct,
        140.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_frame(
        WindowId::new(120),
        LayerCachePolicy::Direct,
        376.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_frame(
        WindowId::new(121),
        LayerCachePolicy::Cached,
        140.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_frame(
        WindowId::new(121),
        LayerCachePolicy::Cached,
        376.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn nested_cached_scroll_sibling_transforms_match_direct_after_image_scroll() {
    let shell_id = WidgetId::new(131);
    let outer_scroll_id = WidgetId::new(132);
    let inner_scroll_id = WidgetId::new(133);
    let top_section_id = WidgetId::new(134);
    let bottom_section_id = WidgetId::new(135);
    let image_layer_id = WidgetId::new(136);
    let image_handle = ImageHandle::new(45);

    let mut images = ImageRegistry::new();
    images.insert(
        image_handle,
        RegisteredImage::from_rgba8(
            2,
            2,
            vec![
                220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64, 255,
            ],
        )
        .unwrap(),
    );
    let images = Arc::new(images);

    let build_frame = |window_id, inner_cache_policy, top_y, bottom_y, update_kind| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 1280.0, 720.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let outer_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(outer_scroll_id),
            outer_scroll_id,
            Rect::new(320.0, 28.0, 428.0, 336.0),
        )
        .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let inner_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(inner_scroll_id),
            inner_scroll_id,
            Rect::new(321.0, 60.0, 426.0, 303.0),
        )
        .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_cache_policy(inner_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll)
        .with_is_stack_surface(true);

        let mut top_scene = Scene::new();
        top_scene.push(SceneCommand::FillRect {
            rect: Rect::new(345.0, top_y, 378.0, 379.0),
            brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
        });
        top_scene.push(SceneCommand::FillRect {
            rect: Rect::new(363.0, top_y + 81.0, 440.2, 240.0),
            brush: Color::rgba(0.97, 0.981, 0.992, 1.0).into(),
        });
        let top_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(top_section_id),
                top_section_id,
                Rect::new(345.0, top_y, 378.0, 379.0),
            )
            .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
            .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            top_scene,
        );

        let image_y = bottom_y + 133.0;
        let mut image_scene = Scene::new();
        image_scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
            brush: Color::rgba(0.965, 0.975, 0.99, 1.0).into(),
        });
        image_scene.push(SceneCommand::PushClip {
            rect: Rect::new(363.0, image_y, 220.0, 220.0),
        });
        image_scene.push(SceneCommand::DrawImage {
            rect: Rect::new(363.0, image_y, 220.0, 220.0),
            source: ImageSource::new(image_handle),
        });
        image_scene.push(SceneCommand::PopClip);
        let image_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(image_layer_id),
                image_layer_id,
                Rect::new(363.0, image_y, 220.0, 220.0),
            )
            .with_content_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
            .with_paint_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            image_scene,
        );

        let mut bottom_scene = Scene::new();
        bottom_scene.push(SceneCommand::FillRect {
            rect: Rect::new(345.0, bottom_y, 378.0, 371.0),
            brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
        });
        bottom_scene.push(SceneCommand::Layer(image_layer));
        let bottom_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(bottom_section_id),
                bottom_section_id,
                Rect::new(345.0, bottom_y, 378.0, 371.0),
            )
            .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
            .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
            .with_cache_policy(LayerCachePolicy::Direct),
            bottom_scene,
        );

        let mut inner_scroll_scene = Scene::new();
        inner_scroll_scene.push(SceneCommand::PushClip {
            rect: Rect::new(321.0, 60.0, 426.0, 303.0),
        });
        inner_scroll_scene.push(SceneCommand::Layer(top_layer));
        inner_scroll_scene.push(SceneCommand::Layer(bottom_layer));
        inner_scroll_scene.push(SceneCommand::PopClip);

        let mut outer_scroll_scene = Scene::new();
        outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            inner_scroll_descriptor,
            inner_scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
            brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
        });
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor,
            {
                let mut shell_scene = Scene::new();
                shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                    outer_scroll_descriptor,
                    outer_scroll_scene,
                )));
                shell_scene
            },
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(1280.0, 720.0),
            surface_size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    update_kind,
                    SceneLayerDescriptor::new(
                        SceneLayerId::from_widget(top_section_id),
                        top_section_id,
                        Rect::new(345.0, top_y, 378.0, 379.0),
                    )
                    .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                    .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                    .with_cache_policy(LayerCachePolicy::Direct),
                )
                .with_damage(Rect::new(345.0, -178.0, 1172.2, 403.0)),
                SceneLayerUpdate::from_descriptor(
                    update_kind,
                    SceneLayerDescriptor::new(
                        SceneLayerId::from_widget(bottom_section_id),
                        bottom_section_id,
                        Rect::new(345.0, bottom_y, 378.0, 371.0),
                    )
                    .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                    .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                    .with_cache_policy(LayerCachePolicy::Direct),
                )
                .with_damage(Rect::new(345.0, 219.0, 378.0, 395.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_frame(
        WindowId::new(122),
        LayerCachePolicy::Direct,
        -178.0,
        219.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_frame(
        WindowId::new(122),
        LayerCachePolicy::Direct,
        -154.0,
        243.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_frame(
        WindowId::new(123),
        LayerCachePolicy::Cached,
        -178.0,
        219.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_frame(
        WindowId::new(123),
        LayerCachePolicy::Cached,
        -154.0,
        243.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}

#[test]
pub(crate) fn nested_cached_scroll_auto_sibling_transforms_match_direct_after_image_scroll() {
    let shell_id = WidgetId::new(137);
    let outer_scroll_id = WidgetId::new(138);
    let inner_scroll_id = WidgetId::new(139);
    let top_section_id = WidgetId::new(140);
    let bottom_section_id = WidgetId::new(141);
    let image_layer_id = WidgetId::new(142);
    let image_handle = ImageHandle::new(46);

    let mut images = ImageRegistry::new();
    images.insert(
        image_handle,
        RegisteredImage::from_rgba8(
            2,
            2,
            vec![
                220, 232, 246, 255, 64, 156, 232, 255, 64, 156, 232, 255, 255, 175, 64, 255,
            ],
        )
        .unwrap(),
    );
    let images = Arc::new(images);

    let build_frame = |window_id, inner_cache_policy, top_y, bottom_y, update_kind| {
        let shell_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(shell_id),
            shell_id,
            Rect::new(0.0, 0.0, 1280.0, 720.0),
        )
        .with_content_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_paint_bounds(Rect::new(0.0, 0.0, 1280.0, 720.0))
        .with_cache_policy(LayerCachePolicy::Direct);

        let outer_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(outer_scroll_id),
            outer_scroll_id,
            Rect::new(320.0, 28.0, 428.0, 336.0),
        )
        .with_content_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_paint_bounds(Rect::new(345.0, 84.0, 378.0, 781.0))
        .with_cache_policy(LayerCachePolicy::Direct)
        .with_composition_mode(LayerCompositionMode::Scroll);

        let inner_scroll_descriptor = SceneLayerDescriptor::new(
            SceneLayerId::from_widget(inner_scroll_id),
            inner_scroll_id,
            Rect::new(321.0, 60.0, 426.0, 303.0),
        )
        .with_content_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_paint_bounds(Rect::new(345.0, -130.0, 1172.2, 768.0))
        .with_cache_policy(inner_cache_policy)
        .with_composition_mode(LayerCompositionMode::Scroll)
        .with_is_stack_surface(true);

        let mut top_scene = Scene::new();
        top_scene.push(SceneCommand::FillRect {
            rect: Rect::new(345.0, top_y, 378.0, 379.0),
            brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
        });
        top_scene.push(SceneCommand::FillRect {
            rect: Rect::new(363.0, top_y + 81.0, 440.2, 240.0),
            brush: Color::rgba(0.97, 0.981, 0.992, 1.0).into(),
        });
        let top_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(top_section_id),
                top_section_id,
                Rect::new(345.0, top_y, 378.0, 379.0),
            )
            .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
            .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0)),
            top_scene,
        );

        let image_y = bottom_y + 133.0;
        let mut image_scene = Scene::new();
        image_scene.push(SceneCommand::FillPath {
            path: Path::rounded_rect(Rect::new(363.0, image_y, 220.0, 220.0), 12.0),
            brush: Color::rgba(0.965, 0.975, 0.99, 1.0).into(),
        });
        image_scene.push(SceneCommand::PushClip {
            rect: Rect::new(363.0, image_y, 220.0, 220.0),
        });
        image_scene.push(SceneCommand::DrawImage {
            rect: Rect::new(363.0, image_y, 220.0, 220.0),
            source: ImageSource::new(image_handle),
        });
        image_scene.push(SceneCommand::PopClip);
        let image_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(image_layer_id),
                image_layer_id,
                Rect::new(363.0, image_y, 220.0, 220.0),
            )
            .with_content_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0))
            .with_paint_bounds(Rect::new(362.5, image_y - 0.5, 221.0, 221.0)),
            image_scene,
        );

        let mut bottom_scene = Scene::new();
        bottom_scene.push(SceneCommand::FillRect {
            rect: Rect::new(345.0, bottom_y, 378.0, 371.0),
            brush: Color::rgba(0.985, 0.99, 1.0, 1.0).into(),
        });
        bottom_scene.push(SceneCommand::Layer(image_layer));
        let bottom_layer = SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(
                SceneLayerId::from_widget(bottom_section_id),
                bottom_section_id,
                Rect::new(345.0, bottom_y, 378.0, 371.0),
            )
            .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
            .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0)),
            bottom_scene,
        );

        let mut inner_scroll_scene = Scene::new();
        inner_scroll_scene.push(SceneCommand::PushClip {
            rect: Rect::new(321.0, 60.0, 426.0, 303.0),
        });
        inner_scroll_scene.push(SceneCommand::Layer(top_layer));
        inner_scroll_scene.push(SceneCommand::Layer(bottom_layer));
        inner_scroll_scene.push(SceneCommand::PopClip);

        let mut outer_scroll_scene = Scene::new();
        outer_scroll_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            inner_scroll_descriptor,
            inner_scroll_scene,
        )));

        let mut scene = Scene::new();
        scene.push(SceneCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
            brush: Color::rgba(0.92, 0.94, 0.97, 1.0).into(),
        });
        scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
            shell_descriptor,
            {
                let mut shell_scene = Scene::new();
                shell_scene.push(SceneCommand::Layer(SceneLayer::from_descriptor(
                    outer_scroll_descriptor,
                    outer_scroll_scene,
                )));
                shell_scene
            },
        )));

        SceneFrame {
            window_id,
            viewport: Size::new(1280.0, 720.0),
            surface_size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            dirty_regions: Vec::new(),
            layer_updates: vec![
                SceneLayerUpdate::from_descriptor(
                    update_kind,
                    SceneLayerDescriptor::new(
                        SceneLayerId::from_widget(top_section_id),
                        top_section_id,
                        Rect::new(345.0, top_y, 378.0, 379.0),
                    )
                    .with_content_bounds(Rect::new(345.0, top_y, 1172.2, 379.0))
                    .with_paint_bounds(Rect::new(345.0, top_y, 1172.2, 379.0)),
                )
                .with_damage(Rect::new(345.0, -178.0, 1172.2, 403.0)),
                SceneLayerUpdate::from_descriptor(
                    update_kind,
                    SceneLayerDescriptor::new(
                        SceneLayerId::from_widget(bottom_section_id),
                        bottom_section_id,
                        Rect::new(345.0, bottom_y, 378.0, 371.0),
                    )
                    .with_content_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0))
                    .with_paint_bounds(Rect::new(345.0, bottom_y, 378.0, 371.0)),
                )
                .with_damage(Rect::new(345.0, 219.0, 378.0, 395.0)),
            ],
            scene,
            font_registry: Arc::new(FontRegistry::new()),
            image_registry: Arc::clone(&images),
            text_layout_registry: Arc::new(TextLayoutRegistry::default()),
        }
    };

    let mut renderer = WgpuRenderer::default();

    let direct_initial = build_frame(
        WindowId::new(124),
        LayerCachePolicy::Direct,
        -178.0,
        219.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&direct_initial).unwrap();
    let direct_updated = build_frame(
        WindowId::new(124),
        LayerCachePolicy::Direct,
        -154.0,
        243.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&direct_updated).unwrap();
    let direct_pixels = renderer
        .capture_last_frame_rgba(direct_updated.window_id)
        .unwrap();

    let cached_initial = build_frame(
        WindowId::new(125),
        LayerCachePolicy::Cached,
        -178.0,
        219.0,
        SceneLayerUpdateKind::Content,
    );
    renderer.render(&cached_initial).unwrap();
    let cached_updated = build_frame(
        WindowId::new(125),
        LayerCachePolicy::Cached,
        -154.0,
        243.0,
        SceneLayerUpdateKind::Transform,
    );
    renderer.render(&cached_updated).unwrap();
    let cached_pixels = renderer
        .capture_last_frame_rgba(cached_updated.window_id)
        .unwrap();

    assert_rgba_images_match(&direct_pixels, &cached_pixels);
}
