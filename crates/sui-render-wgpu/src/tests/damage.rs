use crate::WgpuRenderer;
use crate::damage::RedrawRegion;
use crate::submission::LAST_REDRAW_REGION;
use crate::tests::support::assert_rgba_images_match;
use std::sync::Arc;
use sui_core::{Color, Rect, Size, Vector, WidgetId, WindowId};
use sui_scene::{
    ImageRegistry, Scene, SceneCommand, SceneFrame, SceneLayer, SceneLayerDescriptor, SceneLayerId,
};

const CELLS: usize = 256;

/// A grid of clipped cells, `hidden` left out, plus a small layer at `badge`.
fn grid(
    window: WindowId,
    images: &Arc<ImageRegistry>,
    highlighted: usize,
    hidden: Option<usize>,
    badge: Vector,
) -> SceneFrame {
    let mut frame = SceneFrame::new(window, Size::new(256.0, 272.0));
    frame.image_registry = Arc::clone(images);
    frame.scene.push(SceneCommand::Clear(Color::WHITE));
    for index in (0..CELLS).filter(|index| Some(*index) != hidden) {
        let rect = Rect::new(
            (index % 16) as f32 * 16.0,
            (index / 16) as f32 * 16.0,
            14.0,
            14.0,
        );
        frame.scene.push(SceneCommand::PushClip {
            rect: rect.inflate(-2.0, -2.0),
        });
        frame.scene.push(SceneCommand::FillRect {
            rect,
            brush: if index == highlighted {
                Color::srgba(1.0, 0.8, 0.1, 1.0)
            } else {
                Color::srgba(0.2, 0.4, 0.9, 0.8)
            }
            .into(),
        });
        frame.scene.push(SceneCommand::PopClip);
    }
    let id = WidgetId::new(77);
    let bounds = Rect::new(8.0 + badge.x, 258.0 + badge.y, 24.0, 10.0);
    let mut layer = Scene::new();
    layer.push(SceneCommand::FillRect {
        rect: bounds,
        brush: Color::srgba(0.9, 0.1, 0.3, 0.7).into(),
    });
    frame
        .scene
        .push(SceneCommand::Layer(SceneLayer::from_descriptor(
            SceneLayerDescriptor::new(SceneLayerId::from_widget(id), id, bounds)
                .with_content_bounds(bounds)
                .with_paint_bounds(bounds),
            layer,
        )));
    frame
}

fn last_region() -> RedrawRegion {
    LAST_REDRAW_REGION
        .with(|last| last.get())
        .expect("a frame was encoded")
}

#[test]
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn partial_redraw_matches_full_redraw_through_edits_removals_and_moves() {
    let window = WindowId::new(9300);
    let images = Arc::new(ImageRegistry::new());
    let mut renderer = WgpuRenderer::new();
    // Cells away from the first packet, which also holds the full-window
    // background and so damages everything when it changes.
    let frames = [
        (grid(window, &images, 100, None, Vector::ZERO), None),
        (
            grid(window, &images, 100, None, Vector::ZERO),
            Some("nothing"),
        ),
        (
            grid(window, &images, 37, None, Vector::ZERO),
            Some("partial"),
        ),
        // Removing a cell shifts later packet boundaries, so it may redraw
        // more; only the pixels are checked.
        (
            grid(window, &images, 37, Some(120), Vector::ZERO),
            Some("any"),
        ),
        (
            grid(window, &images, 37, Some(120), Vector::new(40.0, 0.0)),
            Some("partial"),
        ),
        (
            grid(window, &images, 37, None, Vector::new(40.0, 0.0)),
            Some("any"),
        ),
    ];
    for (index, (frame, expected)) in frames.iter().enumerate() {
        renderer.render(frame).unwrap();
        let region = last_region();
        match expected {
            None => assert_eq!(region, RedrawRegion::Full, "frame {index}"),
            Some("nothing") => assert_eq!(region, RedrawRegion::Nothing, "frame {index}"),
            Some("any") => {}
            Some(_) => assert!(
                matches!(region, RedrawRegion::Partial(damage)
                    if u64::from(damage.max_x - damage.min_x)
                        * u64::from(damage.max_y - damage.min_y)
                        < 256 * 272 / 2),
                "frame {index} redrew {region:?}"
            ),
        }
        let pixels = renderer.capture_last_frame_rgba(window).unwrap();
        let mut fresh = WgpuRenderer::new();
        fresh.render(frame).unwrap();
        assert_rgba_images_match(&fresh.capture_last_frame_rgba(window).unwrap(), &pixels);
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn partial_redraw_falls_back_to_full_redraw_when_inputs_change() {
    let window = WindowId::new(9301);
    let images = Arc::new(ImageRegistry::new());
    let mut renderer = WgpuRenderer::new();
    renderer
        .render(&grid(window, &images, 0, None, Vector::ZERO))
        .unwrap();

    // A different image registry may have changed pixels behind any image.
    let updated_images = Arc::new(ImageRegistry::new());
    renderer
        .render(&grid(window, &updated_images, 0, None, Vector::ZERO))
        .unwrap();
    assert_eq!(last_region(), RedrawRegion::Full);

    // A different size needs a new target.
    let mut resized = grid(window, &updated_images, 0, None, Vector::ZERO);
    resized.viewport = Size::new(240.0, 272.0);
    resized.surface_size = resized.viewport;
    renderer.render(&resized).unwrap();
    assert_eq!(last_region(), RedrawRegion::Full);
}
