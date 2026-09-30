//! Shadows of rounded boxes: a Gaussian blur of the box, drawn behind it,
//! outside it only, or inside it, and scaled with the transform.
//!
//! Expected values come from reference fills of the same color at a known
//! alpha, so the checks hold whatever space blending happens in.

use std::sync::Arc;

use sui_core::{Color, Rect, Size, Transform, WindowId};
use sui_scene::{
    Brush, ImageRegistry, Scene, SceneCommand, SceneFrame, ShadowParams, ShadowPlacement,
};
use sui_text::{FontRegistry, TextLayoutRegistry};

use crate::WgpuRenderer;
use crate::capture::RgbaImage;
use crate::tests::support::rgba_pixel;

const VIEWPORT: Size = Size::new(240.0, 240.0);
/// Where reference swatches go, out of the way of the shadows.
const SWATCH_Y: f32 = 220.0;
const TOLERANCE: u8 = 4;

fn render(commands: Vec<SceneCommand>) -> RgbaImage {
    let window_id = WindowId::new(7320);
    let mut scene = Scene::new();
    scene.push(SceneCommand::Clear(Color::rgba(0.0, 0.0, 0.0, 1.0)));
    for command in commands {
        scene.push(command);
    }
    let frame = SceneFrame {
        window_id,
        viewport: VIEWPORT,
        surface_size: VIEWPORT,
        scale_factor: 1.0,
        dirty_regions: Vec::new(),
        layer_updates: Vec::new(),
        scene,
        font_registry: Arc::new(FontRegistry::new()),
        image_registry: Arc::new(ImageRegistry::new()),
        text_layout_registry: Arc::new(TextLayoutRegistry::default()),
    };
    let mut renderer = WgpuRenderer::new();
    renderer.render(&frame).expect("headless shadow render");
    renderer
        .capture_last_frame_rgba(window_id)
        .expect("the frame can be captured")
}

fn white(alpha: f32) -> Color {
    Color::rgba(1.0, 1.0, 1.0, alpha)
}

/// Just the shadow of `rect`: a transparent fill that casts it.
fn shadow_of(rect: Rect, radius: f32, shadow: ShadowParams) -> SceneCommand {
    SceneCommand::FillRoundedRect {
        rect,
        radii: [radius; 4],
        brush: Brush::Solid(Color::TRANSPARENT),
        border: None,
        shadow: Some(shadow),
    }
}

/// A small square of white at `alpha`, the `index`th along the bottom.
fn swatch(index: usize, alpha: f32) -> SceneCommand {
    SceneCommand::FillRect {
        rect: Rect::new(4.0 + index as f32 * 12.0, SWATCH_Y, 8.0, 8.0),
        brush: Brush::Solid(white(alpha)),
    }
}

fn swatch_value(image: &RgbaImage, index: usize) -> u8 {
    rgba_pixel(image, 8 + index as u32 * 12, SWATCH_Y as u32 + 4)[0]
}

fn value(image: &RgbaImage, x: u32, y: u32) -> u8 {
    rgba_pixel(image, x, y)[0]
}

fn assert_near(actual: u8, expected: u8, what: &str) {
    assert!(
        actual.abs_diff(expected) <= TOLERANCE,
        "{what}: got {actual}, expected {expected}"
    );
}

/// The error function, to about 1e-7 (Abramowitz and Stegun 7.1.26).
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t
        * (0.254_829_6
            + t * (-0.284_496_7 + t * (1.421_413_8 + t * (-1.453_152 + t * 1.061_405_4))));
    let y = 1.0 - poly * (-x * x).exp();
    y.copysign(x)
}

/// Coverage at `distance` past the straight edge of a large box blurred with
/// deviation `sigma`.
fn edge_coverage(distance: f32, sigma: f32) -> f32 {
    0.5 * (1.0 - erf(distance / (std::f32::consts::SQRT_2 * sigma)))
}

#[test]
fn a_shadow_is_its_box_blurred_by_a_gaussian_of_half_the_blur_radius() {
    // A 120 px box, blurred with blur radius 16: deviation 8. Samples along
    // the middle of its right edge, where the far edges add nothing.
    let sigma = 8.0;
    let distances = [0.5, 8.5, 16.5];
    let mut commands = vec![shadow_of(
        Rect::new(40.0, 40.0, 120.0, 120.0),
        0.0,
        ShadowParams::new(0.0, 0.0, 16.0, 0.0, white(1.0)),
    )];
    for (index, distance) in distances.iter().enumerate() {
        commands.push(swatch(index, edge_coverage(*distance, sigma)));
    }
    let image = render(commands);
    for (index, distance) in distances.iter().enumerate() {
        let x = 160 + (*distance - 0.5) as u32;
        assert_near(
            value(&image, x, 100),
            swatch_value(&image, index),
            &format!("{distance} px past the edge"),
        );
    }
    assert_eq!(value(&image, 100, 100), 255, "the middle is fully covered");
}

#[test]
fn a_glow_around_a_small_box_stays_as_faint_as_its_blur_makes_it() {
    // A 4 px box blurred with deviation 16 spreads its little coverage thin:
    // the middle gets erf(2 / (sqrt(2) 16))^2 of the color, about 1%.
    let expected = erf(2.0 / (std::f32::consts::SQRT_2 * 16.0)).powi(2);
    let image = render(vec![
        shadow_of(
            Rect::new(98.0, 98.0, 4.0, 4.0),
            0.0,
            ShadowParams::new(0.0, 0.0, 32.0, 0.0, white(1.0)),
        ),
        swatch(0, expected),
    ]);
    assert_near(value(&image, 100, 100), swatch_value(&image, 0), "middle");
}

#[test]
fn an_outside_shadow_leaves_its_box_clear() {
    let rect = Rect::new(60.0, 60.0, 120.0, 120.0);
    let shadow = ShadowParams::glow(16.0, 0.0, white(1.0));
    let outside = render(vec![shadow_of(rect, 12.0, shadow)]);
    let behind = render(vec![shadow_of(
        rect,
        12.0,
        shadow.with_placement(ShadowPlacement::Behind),
    )]);
    assert_eq!(value(&outside, 120, 120), 0, "clear inside the box");
    assert_eq!(value(&behind, 120, 120), 255, "covered behind the box");
    for x in [184, 190, 200] {
        assert_near(
            value(&outside, x, 120),
            value(&behind, x, 120),
            &format!("the same outside the box at x {x}"),
        );
    }
}

#[test]
fn an_inside_shadow_darkens_only_the_inside_of_its_edges() {
    let sigma = 4.0;
    let image = render(vec![
        shadow_of(
            Rect::new(40.0, 40.0, 160.0, 160.0),
            0.0,
            ShadowParams::inset(0.0, 0.0, 8.0, 0.0, white(1.0)),
        ),
        // Inside, the shadow is what the blurred hole leaves uncovered.
        swatch(0, 1.0 - edge_coverage(-0.5, sigma)),
        swatch(1, 1.0 - edge_coverage(-6.5, sigma)),
    ]);
    assert_near(
        value(&image, 40, 120),
        swatch_value(&image, 0),
        "at the edge",
    );
    assert_near(value(&image, 46, 120), swatch_value(&image, 1), "inward");
    assert_eq!(value(&image, 120, 120), 0, "clear in the middle");
    assert_eq!(value(&image, 36, 120), 0, "nothing outside the box");
}

#[test]
fn an_inside_shadow_goes_over_its_fill() {
    let image = render(vec![SceneCommand::FillRoundedRect {
        rect: Rect::new(40.0, 40.0, 160.0, 160.0),
        radii: [0.0; 4],
        brush: Brush::Solid(Color::rgba(0.0, 0.0, 1.0, 1.0)),
        border: None,
        shadow: Some(ShadowParams::inset(0.0, 0.0, 8.0, 0.0, white(1.0))),
    }]);
    let edge = rgba_pixel(&image, 40, 120);
    assert!(edge[0] > 100, "the shadow shows over the fill: {edge:?}");
    let middle = rgba_pixel(&image, 120, 120);
    assert_eq!(middle[0], 0, "the fill shows in the middle: {middle:?}");
    assert_eq!(middle[2], 255, "the fill shows in the middle: {middle:?}");
}

#[test]
fn shadows_scale_with_their_transform() {
    let shadow = ShadowParams::new(4.0, 2.0, 8.0, 2.0, white(0.8));
    let scaled = render(vec![
        SceneCommand::PushTransform {
            transform: Transform::scale(2.0, 2.0),
        },
        shadow_of(Rect::new(20.0, 20.0, 50.0, 50.0), 6.0, shadow),
        SceneCommand::PopTransform,
    ]);
    let direct = render(vec![shadow_of(
        Rect::new(40.0, 40.0, 100.0, 100.0),
        12.0,
        ShadowParams::new(8.0, 4.0, 16.0, 4.0, white(0.8)),
    )]);
    for (x, y) in [(150, 90), (160, 90), (90, 150), (150, 150), (30, 30)] {
        assert_near(
            value(&scaled, x, y),
            value(&direct, x, y),
            &format!("({x}, {y})"),
        );
    }
}
