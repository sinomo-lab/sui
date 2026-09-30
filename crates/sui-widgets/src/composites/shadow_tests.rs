//! Shadows and glows on surfaces, shadow boxes, primary buttons, and spinners.

use sui_core::{Color, Size};
use sui_layout::Constraints;
use sui_runtime::{Application, MeasureCtx, PaintCtx, RenderOutput, Widget, WindowBuilder};
use sui_scene::{Brush, SceneCommand, ShadowParams, ShadowPlacement};

use super::{ShadowBox, Spinner, Surface};
use crate::{Button, DefaultTheme, GlowTone, SizedBox, ThemeShadow, ThemeShadowLayer};

fn render<W>(root: W) -> RenderOutput
where
    W: Widget + 'static,
{
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("Shadows").root(root))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];
    runtime.render(window_id).unwrap()
}

/// What was painted, in order, as the renderer draws it: a shadow around a
/// box goes before the box's fill, one inside it after.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Painted {
    Fill(Color),
    Shadow(ShadowParams),
}

fn painted(output: &RenderOutput) -> Vec<Painted> {
    let mut painted = Vec::new();
    output
        .frame
        .scene
        .visit_commands(&mut |command| match command {
            SceneCommand::FillRoundedRect { brush, shadow, .. } => {
                let fill = match brush {
                    Brush::Solid(color) if color.alpha > 0.0 => Some(Painted::Fill(*color)),
                    _ => None,
                };
                match shadow {
                    Some(shadow) if shadow.placement == ShadowPlacement::Inside => {
                        painted.extend(fill);
                        painted.push(Painted::Shadow(*shadow));
                    }
                    Some(shadow) => {
                        painted.push(Painted::Shadow(*shadow));
                        painted.extend(fill);
                    }
                    None => painted.extend(fill),
                }
            }
            SceneCommand::FillRect {
                brush: Brush::Solid(color),
                ..
            }
            | SceneCommand::FillPath {
                brush: Brush::Solid(color),
                ..
            } => painted.push(Painted::Fill(*color)),
            _ => {}
        });
    painted
}

fn shadows(output: &RenderOutput) -> Vec<ShadowParams> {
    painted(output)
        .into_iter()
        .filter_map(|painted| match painted {
            Painted::Shadow(shadow) => Some(shadow),
            Painted::Fill(_) => None,
        })
        .collect()
}

fn position(painted: &[Painted], wanted: impl Fn(&Painted) -> bool) -> usize {
    painted
        .iter()
        .position(wanted)
        .unwrap_or_else(|| panic!("not painted: {painted:?}"))
}

/// A block of one color, standing in for content that paints its own face.
struct Block(Color);

impl Widget for Block {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(80.0, 40.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.fill_rect(ctx.bounds(), self.0);
    }
}

fn layer(offset_y: f32, blur: f32, inset: bool) -> ThemeShadowLayer {
    ThemeShadowLayer {
        offset_x: 0.0,
        offset_y,
        blur,
        spread: 0.0,
        color: Color::BLACK.with_alpha(0.2),
        inset,
    }
}

#[test]
fn a_surface_shadow_replaces_its_elevation_and_draws_inset_layers_over_its_fill() {
    let theme = DefaultTheme::default();
    let output = render(
        Surface::field(SizedBox::new().width(80.0).height(40.0))
            .theme(theme)
            .radius(8.0)
            .elevation(crate::SurfaceElevation::Large)
            .shadow(|_| ThemeShadow::double(layer(4.0, 8.0, false), layer(2.0, 4.0, true))),
    );
    let painted = painted(&output);
    let outer = position(
        &painted,
        |painted| matches!(painted, Painted::Shadow(shadow) if shadow.placement == ShadowPlacement::Behind),
    );
    let fill = position(&painted, |painted| matches!(painted, Painted::Fill(_)));
    let inset = position(
        &painted,
        |painted| matches!(painted, Painted::Shadow(shadow) if shadow.placement == ShadowPlacement::Inside),
    );
    assert!(outer < fill && fill < inset, "{painted:?}");
    let shadows = shadows(&output);
    assert_eq!(shadows.len(), 2, "the large elevation's shadow is replaced");
    assert_eq!(shadows[0].blur, 8.0);
    assert_eq!(shadows[1].offset_y, 2.0);
}

#[test]
fn a_surface_draws_inset_shadows_inside_its_border() {
    let theme = DefaultTheme::default();
    let output = render(
        Surface::field(SizedBox::new().width(80.0).height(40.0))
            .name("Field")
            .theme(theme)
            .radius(8.0)
            .shadow(|_| ThemeShadow::single(layer(1.0, 0.0, true))),
    );
    let face = output
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some("Field"))
        .expect("the surface is in the semantics tree")
        .bounds;
    let mut inset = None;
    output.frame.scene.visit_commands(&mut |command| {
        if let SceneCommand::FillRoundedRect {
            rect,
            radii,
            shadow: Some(shadow),
            ..
        } = command
            && shadow.placement == ShadowPlacement::Inside
        {
            inset = Some((*rect, radii[0]));
        }
    });
    let (inset, inset_radius) = inset.expect("the surface draws its inset shadow");
    // The border lies wholly inside the surface's edge.
    let border = theme.metrics.border_width.max(1.0);
    assert_eq!(inset, face.inflate(-border, -border));
    assert_eq!(inset_radius, 8.0 - border);
}

#[test]
fn surfaces_glow_only_where_the_theme_glows() {
    let glowing = |theme: DefaultTheme| {
        shadows(&render(
            Surface::panel(SizedBox::new().width(80.0).height(40.0))
                .theme(theme)
                .radius(8.0)
                .glow(GlowTone::Accent),
        ))
    };
    let dark = DefaultTheme::dark();
    let halo = dark.glows.accent.first.expect("dark themes glow");
    assert_eq!(
        glowing(dark),
        vec![
            ShadowParams::glow(halo.blur, halo.spread, halo.color)
                .with_placement(ShadowPlacement::Outside)
        ]
    );
    assert!(
        glowing(DefaultTheme::light()).is_empty(),
        "light has no glow"
    );
}

#[test]
fn a_shadow_box_casts_around_its_child_and_insets_over_it() {
    let face = Color::rgba(0.2, 0.4, 0.8, 1.0);
    let output = render(
        ShadowBox::new(Block(face))
            .theme(DefaultTheme::dark())
            .radius(6.0)
            .shadow(|_| ThemeShadow::double(layer(4.0, 8.0, false), layer(2.0, 4.0, true)))
            .glow(GlowTone::Secondary),
    );
    let painted = painted(&output);
    let face_at = position(&painted, |painted| *painted == Painted::Fill(face));
    let behind = position(
        &painted,
        |painted| matches!(painted, Painted::Shadow(shadow) if shadow.placement == ShadowPlacement::Behind),
    );
    let glow = position(
        &painted,
        |painted| matches!(painted, Painted::Shadow(shadow) if shadow.placement == ShadowPlacement::Outside),
    );
    let inset = position(
        &painted,
        |painted| matches!(painted, Painted::Shadow(shadow) if shadow.placement == ShadowPlacement::Inside),
    );
    assert!(
        behind < face_at && glow < face_at && face_at < inset,
        "{painted:?}"
    );
    let secondary = DefaultTheme::dark().glows.secondary.first.unwrap();
    assert!(shadows(&output).iter().any(|shadow| {
        shadow.placement == ShadowPlacement::Outside && shadow.color == secondary.color
    }));
}

#[test]
fn only_enabled_primary_buttons_glow() {
    let glows = |button: Button| {
        shadows(&render(button))
            .iter()
            .any(|shadow| shadow.placement == ShadowPlacement::Outside)
    };
    let dark = DefaultTheme::dark();
    assert!(glows(Button::primary("Run").theme(dark)));
    assert!(!glows(Button::primary("Run").theme(DefaultTheme::light())));
    assert!(!glows(Button::primary("Run").theme(dark).glow(false)));
    assert!(!glows(Button::primary("Run").theme(dark).enabled(false)));
    assert!(!glows(Button::new("Cancel").theme(dark)));
}

#[test]
fn a_busy_spinner_glows_around_its_indicator() {
    let output = render(
        Spinner::new("Loading")
            .size(20.0)
            .theme(DefaultTheme::dark()),
    );
    let glow = shadows(&output)
        .into_iter()
        .find(|shadow| shadow.placement == ShadowPlacement::Outside)
        .expect("the spinner glows");
    assert_eq!(glow.offset_x, 0.0);
    let mut rounded = Vec::new();
    output.frame.scene.visit_commands(&mut |command| {
        if let SceneCommand::FillRoundedRect {
            rect,
            radii,
            shadow: Some(_),
            ..
        } = command
        {
            rounded.push((*rect, *radii));
        }
    });
    assert!(
        rounded
            .iter()
            .any(|(rect, radii)| rect.width() == 20.0 && radii == &[10.0; 4]),
        "the glow is round around the indicator: {rounded:?}"
    );
    assert!(
        shadows(&render(
            Spinner::new("Loading").theme(DefaultTheme::light())
        ))
        .is_empty()
    );
}
