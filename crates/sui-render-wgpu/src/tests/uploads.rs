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
fn vertex_allocations_are_reused_across_changes_empty_frames_and_growth() {
    let mut renderer = WgpuRenderer::new();
    let window = WindowId::new(8901);
    renderer
        .render(&frame(window, Color::srgba(1.0, 0.0, 0.0, 1.0), 1))
        .unwrap();
    let original = renderer.frame_resources.fragments[&window][0]
        .extended
        .buffer
        .clone()
        .unwrap();
    renderer
        .render(&frame(window, Color::srgba(0.0, 1.0, 0.0, 1.0), 1))
        .unwrap();
    assert_eq!(
        renderer.frame_resources.fragments[&window][0]
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
        renderer.frame_resources.fragments[&window][0]
            .extended
            .buffer
            .as_ref(),
        Some(&original)
    );
    renderer.render(&frame(window, Color::WHITE, 32)).unwrap();
    let grown = renderer.frame_resources.fragments[&window][0]
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
        renderer.frame_resources.fragments[&window][0]
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
