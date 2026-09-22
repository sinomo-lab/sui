use crate::WgpuRenderer;
use crate::tests::support::assert_rgba_images_match;
use sui_core::{Color, Path, Point, Rect, Size, Vector, WidgetId, WindowId};
use sui_scene::{
    LayerProperties, Scene, SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor,
    SceneLayerId,
};

fn layered_frame(window: WindowId, revision: usize) -> SceneFrame {
    let mut frame = SceneFrame::new(window, Size::new(200.0, 140.0));
    frame.scene.push(SceneCommand::Clear(Color::BLACK));
    frame.scene.push(SceneCommand::PushClip {
        rect: Rect::new(0.0, 0.0, if revision == 4 { 100.0 } else { 200.0 }, 140.0),
    });
    let mut content = Scene::new();
    for index in 0..80 {
        content.push(SceneCommand::FillRoundedRect {
            rect: Rect::new(
                (index % 10) as f32 * 18.0,
                (index / 10) as f32 * 16.0,
                12.0,
                12.0,
            ),
            radii: [3.0; 4],
            brush: if revision == 1 && index == 20 {
                Color::WHITE
            } else {
                Color::rgba(0.2, 0.5, 0.8, 1.0)
            }
            .into(),
            border: None,
            shadow: None,
        });
    }
    let owner = WidgetId::new(9201);
    let mut descriptor = SceneLayerDescriptor::new(
        SceneLayerId::from_widget(owner),
        owner,
        Rect::new(0.0, 0.0, 180.0, 128.0),
    );
    descriptor.properties = LayerProperties {
        translation: if revision == 2 {
            Vector::new(7.0, 3.0)
        } else {
            Vector::ZERO
        },
        opacity: if revision == 3 { 0.4 } else { 1.0 },
        ..LayerProperties::default()
    };
    frame
        .scene
        .push(SceneCommand::Layer(SceneLayer::from_descriptor(
            descriptor, content,
        )));
    frame.scene.push(SceneCommand::PopClip);
    if revision == 5 {
        frame.scale_factor = 1.5;
        frame.surface_size = Size::new(300.0, 210.0);
    }
    frame
}

#[test]
fn prepared_fragments_reuse_unchanged_data_and_invalidate_local_edits_and_properties() {
    let window = WindowId::new(9200);
    let mut retained = WgpuRenderer::new();
    let mut reference = WgpuRenderer::new();
    for revision in [0, 0, 1, 2, 3, 4, 5, 0] {
        let frame = layered_frame(window, revision);
        retained.render(&frame).unwrap();
        let stats = retained.last_frame_stats(window).unwrap();
        if revision == 1 {
            assert_eq!(
                stats.prepared_fragment_build_count, 1,
                "one edited packet should be prepared"
            );
            assert!(stats.prepared_fragment_cache_hits >= 4);
        }
        reference.compositors.remove(&window);
        reference.frame_resources.fragments.remove(&window);
        reference.render(&frame).unwrap();
        assert_rgba_images_match(
            &reference.capture_rgba(window).unwrap(),
            &retained.capture_rgba(window).unwrap(),
        );
        retained.render(&frame).unwrap();
        let warm = retained.last_frame_stats(window).unwrap();
        assert_eq!(warm.prepared_fragment_build_count, 0);
        assert_eq!(warm.prepared_fragment_cache_hits, warm.direct_packet_count);
        assert_eq!(warm.uploaded_vertex_bytes, 0);
    }
    retained
        .render(&SceneFrame::new(window, Size::new(200.0, 140.0)))
        .unwrap();
    assert!(retained.frame_resources.fragments[&window].is_empty());
    retained.remove_window(window);
    assert!(!retained.frame_resources.fragments.contains_key(&window));
}

#[test]
fn prepared_analytic_instances_refresh_after_arena_slot_remapping() {
    let mut renderer = WgpuRenderer::new();
    let make_frame = |window, radius| {
        let mut frame = SceneFrame::new(window, Size::new(100.0, 100.0));
        frame.scene.push(SceneCommand::Clear(Color::BLACK));
        frame.scene.push(SceneCommand::FillPath {
            path: Path::circle(Point::new(50.0, 50.0), radius),
            brush: Color::WHITE.into(),
        });
        frame
    };
    renderer
        .render(&make_frame(WindowId::new(9210), 10.0))
        .unwrap();
    let window = WindowId::new(9211);
    let frame = make_frame(window, 30.0);
    renderer.render(&frame).unwrap();
    let expected = renderer.capture_rgba(window).unwrap();
    let signature = renderer
        .analytic_path_cache
        .iter()
        .find(|(_, entry)| entry.slot == 1)
        .map(|(signature, _)| *signature)
        .expect("second path occupies slot one");
    // Reproduce compaction after an unrelated cached path is retired.
    renderer
        .analytic_path_cache
        .retain(|key, _| *key == signature);
    renderer.frame_resources.analytic_path_arena.bind_group = None;
    renderer.render(&frame).unwrap();
    assert_eq!(renderer.analytic_path_cache[&signature].slot, 0);
    let stats = renderer.last_frame_stats(window).unwrap();
    assert_eq!(
        stats.retained_packet_build_count, 0,
        "raster content was unchanged"
    );
    assert_eq!(
        stats.prepared_fragment_build_count, 1,
        "GPU slot references must refresh"
    );
    assert_rgba_images_match(&expected, &renderer.capture_rgba(window).unwrap());
}
