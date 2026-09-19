use crate::output::{ColorManagementMode, OutputStrategy, RequestedToneMappingMode};
use crate::tests::support::assert_rgba_pixel_near;
use crate::{RendererFrameStats, WgpuRenderer};
use sui_core::{Color, Rect, Size, WindowId};
use sui_scene::{SceneCommand, SceneFrame};

fn frame(window: WindowId, color: Color, rect_count: usize) -> SceneFrame {
    let mut frame = SceneFrame::new(window, Size::new(32.0, 32.0));
    frame.scene.push(SceneCommand::Clear(Color::BLACK));
    for index in 0..rect_count {
        let width = 32.0 / rect_count as f32;
        frame.scene.push(SceneCommand::FillRect {
            rect: Rect::new(index as f32 * width, 0.0, width, 32.0),
            brush: color.into(),
        });
    }
    frame
}

#[test]
fn optimization_regression_unchanged_vertices_are_not_uploaded_again() {
    let mut renderer = WgpuRenderer::new();
    let window = WindowId::new(8951);
    let content = frame(window, Color::WHITE, 64);
    renderer.render(&content).unwrap();
    assert!(
        renderer
            .last_frame_stats(window)
            .unwrap()
            .uploaded_vertex_bytes
            > 0
    );
    let original = renderer.capture_rgba(window).unwrap();
    renderer.render(&content).unwrap();
    assert_eq!(
        renderer
            .last_frame_stats(window)
            .unwrap()
            .uploaded_vertex_bytes,
        0
    );
    assert!(original.pixels() == renderer.capture_rgba(window).unwrap().pixels());
}

#[test]
fn local_vertex_changes_upload_a_subrange_and_preserve_other_geometry() {
    let window = WindowId::new(8956);
    let mut renderer = WgpuRenderer::new();
    let mut scene = frame(window, Color::WHITE, 8);
    renderer.render(&scene).unwrap();
    let full_bytes = renderer
        .last_frame_stats(window)
        .unwrap()
        .uploaded_vertex_bytes;
    let mut updated = frame(window, Color::WHITE, 8);
    // Rebuild the same geometry with one changed color in the middle.
    updated.scene.clear();
    updated.scene.push(SceneCommand::Clear(Color::BLACK));
    for i in 0..8 {
        updated.scene.push(SceneCommand::FillRect {
            rect: Rect::new(i as f32 * 4.0, 0.0, 4.0, 32.0),
            brush: if i == 4 { Color::BLACK } else { Color::WHITE }.into(),
        });
    }
    renderer.render(&updated).unwrap();
    let changed_bytes = renderer
        .last_frame_stats(window)
        .unwrap()
        .uploaded_vertex_bytes;
    assert!(changed_bytes > 0 && changed_bytes < full_bytes);
    assert_rgba_pixel_near(
        &renderer.capture_rgba(window).unwrap(),
        18,
        16,
        [0, 0, 0, 255],
        1,
    );
    let mut reference = WgpuRenderer::new();
    reference.render(&updated).unwrap();
    crate::tests::support::assert_rgba_images_match(
        &renderer.capture_rgba(window).unwrap(),
        &reference.capture_rgba(window).unwrap(),
    );
    // Growth and shrink must also update bytes beyond the old logical length.
    for count in [2, 64, 8] {
        scene = frame(window, Color::WHITE, count);
        renderer.render(&scene).unwrap();
        reference.render(&scene).unwrap();
        crate::tests::support::assert_rgba_images_match(
            &renderer.capture_rgba(window).unwrap(),
            &reference.capture_rgba(window).unwrap(),
        );
    }
}

#[test]
fn vertex_allocations_are_reused_across_changes_empty_frames_and_growth() {
    let mut renderer = WgpuRenderer::new();
    let window = WindowId::new(8901);
    renderer
        .render(&frame(window, Color::srgba(1.0, 0.0, 0.0, 1.0), 1))
        .unwrap();
    let original = renderer.frame_resources.fragments[&window]
        [&crate::retained::RetainedPacketId {
            container: crate::retained::CompositionContainerId::Root,
            segment_index: 0,
        }]
        .extended
        .buffer
        .clone()
        .unwrap();
    renderer
        .render(&frame(window, Color::srgba(0.0, 1.0, 0.0, 1.0), 1))
        .unwrap();
    assert_eq!(
        renderer.frame_resources.fragments[&window][&crate::retained::RetainedPacketId {
            container: crate::retained::CompositionContainerId::Root,
            segment_index: 0
        }]
            .extended
            .buffer
            .as_ref(),
        Some(&original)
    );
    assert_rgba_pixel_near(
        &renderer.capture_rgba(window).unwrap(),
        16,
        16,
        [0, 255, 0, 255],
        1,
    );

    renderer.render(&frame(window, Color::BLACK, 0)).unwrap();
    renderer.render(&frame(window, Color::WHITE, 1)).unwrap();
    assert_eq!(
        renderer.frame_resources.fragments[&window][&crate::retained::RetainedPacketId {
            container: crate::retained::CompositionContainerId::Root,
            segment_index: 0
        }]
            .extended
            .buffer
            .as_ref(),
        Some(&original)
    );
    renderer.render(&frame(window, Color::WHITE, 32)).unwrap();
    let grown = renderer.frame_resources.fragments[&window][&crate::retained::RetainedPacketId {
        container: crate::retained::CompositionContainerId::Root,
        segment_index: 0,
    }]
        .extended
        .buffer
        .clone()
        .unwrap();
    assert_ne!(original, grown);
    assert!(grown.size() > original.size());
    renderer
        .render(&frame(window, Color::srgba(0.0, 0.0, 1.0, 1.0), 1))
        .unwrap();
    assert_eq!(
        renderer.frame_resources.fragments[&window][&crate::retained::RetainedPacketId {
            container: crate::retained::CompositionContainerId::Root,
            segment_index: 0
        }]
            .extended
            .buffer
            .as_ref(),
        Some(&grown)
    );
    assert_rgba_pixel_near(
        &renderer.capture_rgba(window).unwrap(),
        16,
        16,
        [0, 0, 255, 255],
        1,
    );
}

#[test]
fn queued_window_uploads_remain_independent_and_are_released_on_close() {
    let mut renderer = WgpuRenderer::new();
    let first = WindowId::new(8902);
    let second = WindowId::new(8903);
    renderer
        .render(&frame(first, Color::srgba(1.0, 0.0, 0.0, 1.0), 1))
        .unwrap();
    renderer
        .render(&frame(second, Color::srgba(0.0, 0.0, 1.0, 1.0), 1))
        .unwrap();
    renderer
        .render(&frame(first, Color::srgba(0.0, 1.0, 0.0, 1.0), 1))
        .unwrap();
    assert_rgba_pixel_near(
        &renderer.capture_rgba(first).unwrap(),
        16,
        16,
        [0, 255, 0, 255],
        1,
    );
    assert_rgba_pixel_near(
        &renderer.capture_rgba(second).unwrap(),
        16,
        16,
        [0, 0, 255, 255],
        1,
    );
    renderer.remove_window(first);
    assert!(!renderer.frame_resources.fragments.contains_key(&first));
    assert!(
        !renderer
            .frame_resources
            .output_transforms
            .contains_key(&first)
    );
    assert!(renderer.frame_resources.fragments.contains_key(&second));
}

#[test]
fn output_resources_reuse_bindings_but_update_color_policy_and_resized_source() {
    let mut renderer = WgpuRenderer::new();
    let window = WindowId::new(8904);
    let initial = frame(window, Color::linear_rgba(4.0, 0.0, 0.0, 1.0), 1);
    renderer.render(&initial).unwrap();
    let uniform_buffer = renderer.frame_resources.output_transforms[&window]
        .buffer
        .clone();
    let bind_group = renderer.frame_resources.output_transforms[&window]
        .bind_group
        .clone();
    renderer.render(&initial).unwrap();
    assert_eq!(
        renderer.frame_resources.output_transforms[&window].buffer,
        uniform_buffer
    );
    assert_eq!(
        renderer.frame_resources.output_transforms[&window].bind_group,
        bind_group
    );

    let source = renderer.intermediate_targets[&window].view.clone();
    let destination = renderer.offscreen_targets[&window].view.clone();
    renderer
        .submit_output_transform_pass(
            window,
            &source,
            &destination,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            OutputStrategy::SdrSurface {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
            },
            RequestedToneMappingMode::Reinhard,
            ColorManagementMode::default().sdr_content_brightness_nits,
            None,
            &mut RendererFrameStats::default(),
        )
        .unwrap();
    assert_eq!(
        renderer.frame_resources.output_transforms[&window].buffer,
        uniform_buffer
    );
    assert_eq!(
        renderer.frame_resources.output_transforms[&window].bind_group,
        bind_group
    );
    let captured = renderer.capture_rgba(window).unwrap();
    let pixel = &captured.pixels()[0..4];
    assert!(
        pixel[0] < 255 && pixel[0] > 200,
        "tone mapping must update the reused uniform: {pixel:?}"
    );

    let mut resized = frame(window, Color::WHITE, 1);
    resized.viewport = Size::new(64.0, 64.0);
    resized.surface_size = resized.viewport;
    renderer.render(&resized).unwrap();
    assert_ne!(
        renderer.frame_resources.output_transforms[&window].bind_group,
        bind_group
    );
    assert_rgba_pixel_near(&renderer.capture_rgba(window).unwrap(), 16, 16, [255; 4], 1);
}

#[test]
fn growing_text_keeps_other_packet_allocations_and_uploads_local() {
    use crate::retained::{CompositionContainerId, RetainedPacketId};
    use crate::tests::support::assert_rgba_images_match;
    let window = WindowId::new(8962);
    let mut renderer = WgpuRenderer::new();
    let mut reference = WgpuRenderer::new();
    reference
        .compositors
        .entry(window)
        .or_default()
        .packet_draw_limit = usize::MAX;
    let make = |grow| {
        let mut frame = SceneFrame::new(window, Size::new(480.0, 160.0));
        frame.scene.push(SceneCommand::Clear(Color::BLACK));
        for index in 0..64 {
            frame.scene.push(SceneCommand::Label {
                rect: Rect::new(
                    (index % 8) as f32 * 60.0,
                    (index / 8) as f32 * 20.0,
                    59.0,
                    20.0,
                ),
                text: if grow && index == 0 { "AAAA" } else { "A" }.into(),
                color: Color::WHITE,
            });
        }
        frame
    };
    renderer.render(&make(false)).unwrap();
    let full_upload = renderer
        .last_frame_stats(window)
        .unwrap()
        .uploaded_vertex_bytes;
    let stable = RetainedPacketId {
        container: CompositionContainerId::Root,
        segment_index: 2,
    };
    let buffer = renderer.frame_resources.fragments[&window][&stable]
        .text
        .buffer
        .clone();
    for grow in [true, false, true] {
        let frame = make(grow);
        renderer.render(&frame).unwrap();
        reference.render(&frame).unwrap();
        assert_eq!(
            renderer.frame_resources.fragments[&window][&stable]
                .text
                .buffer,
            buffer
        );
        let stats = renderer.last_frame_stats(window).unwrap();
        assert!(
            stats.uploaded_vertex_bytes < full_upload / 2,
            "{} of {} bytes uploaded",
            stats.uploaded_vertex_bytes,
            full_upload
        );
        assert_rgba_images_match(
            &renderer.capture_rgba(window).unwrap(),
            &reference.capture_rgba(window).unwrap(),
        );
    }
    renderer
        .render(&SceneFrame::new(window, Size::new(480.0, 160.0)))
        .unwrap();
    assert!(renderer.frame_resources.fragments[&window].is_empty());
}

#[test]
fn initialization_diagnostics_distinguish_cold_and_reused_resources() {
    let window = WindowId::new(8963);
    let mut renderer = WgpuRenderer::new();
    let content = frame(window, Color::WHITE, 1);
    renderer.render(&content).unwrap();
    let cold = renderer.last_frame_stats(window).unwrap();
    assert!(cold.device_prepare_time_us > 0);
    assert!(cold.pipeline_create_count > 0);
    assert!(cold.pipeline_create_time_us > 0);
    renderer.render(&content).unwrap();
    let warm = renderer.last_frame_stats(window).unwrap();
    assert_eq!(warm.device_prepare_time_us, 0);
    assert_eq!(warm.pipeline_create_count, 0);
    assert_eq!(warm.pipeline_create_time_us, 0);
    assert_eq!(warm.text_engine_init_time_us, 0);
    let mut disabled = WgpuRenderer::new();
    disabled.set_runtime_diagnostics_enabled(false);
    disabled.render(&content).unwrap();
    let disabled = disabled.last_frame_stats(window).unwrap();
    assert_eq!(disabled.device_prepare_time_us, 0);
    assert_eq!(disabled.pipeline_create_time_us, 0);
    assert_eq!(disabled.pipeline_create_count, 0);
    assert_eq!(disabled.text_engine_init_time_us, 0);
    assert_eq!(disabled.target_prepare_time_us, 0);
}
