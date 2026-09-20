//! Conservative solid backdrops for perceptual text coverage. This is paint
//! metadata, not a framebuffer readback or a second scene renderer.
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};
use sui_core::{Color, Rect, Transform};
use sui_scene::{Brush, SceneCommand};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TextBackground {
    clear: Option<Color>,
    regions: Arc<Vec<(Rect, Option<Color>)>>,
}

fn contains(outer: Rect, inner: Rect) -> bool {
    outer.x() <= inner.x()
        && outer.y() <= inner.y()
        && outer.max_x() >= inner.max_x()
        && outer.max_y() >= inner.max_y()
}

impl TextBackground {
    pub(crate) fn color_under(&self, bounds: Rect) -> Option<Color> {
        for (rect, color) in self.regions.iter().rev() {
            if rect.intersection(bounds).is_some() {
                return contains(*rect, bounds).then_some(*color).flatten();
            }
        }
        self.clear
    }

    pub(crate) fn fingerprint(&self, hasher: &mut impl Hasher) {
        let mut color = |c: Option<Color>| {
            c.is_some().hash(hasher);
            if let Some(c) = c {
                let c = c.to_linear_srgb();
                for v in [c.red, c.green, c.blue, c.alpha] {
                    v.to_bits().hash(hasher);
                }
            }
        };
        color(self.clear);
        for (_, c) in self.regions.iter() {
            color(*c);
        }
        for (r, _) in self.regions.iter() {
            for v in [r.x(), r.y(), r.width(), r.height()] {
                v.to_bits().hash(hasher);
            }
        }
    }

    pub(crate) fn transform(&mut self, transform: Transform) {
        if transform.xy.abs() > f32::EPSILON || transform.yx.abs() > f32::EPSILON {
            // A rotated bounding box does not establish an opaque rectangle.
            self.regions = Arc::default();
            self.clear = None;
        } else {
            for (rect, _) in Arc::make_mut(&mut self.regions) {
                *rect = transform.transform_rect_bbox(*rect);
            }
        }
    }

    fn record(&mut self, rect: Rect, color: Option<Color>) {
        if rect.is_empty() {
            return;
        }
        let color = color.and_then(|fg| {
            if fg.alpha >= 1.0 {
                return Some(fg);
            }
            let bg = self.color_under(rect)?.to_linear_srgb();
            let fg = fg.to_linear_srgb();
            let a = fg.alpha.clamp(0.0, 1.0);
            Some(Color::linear_rgba(
                fg.red * a + bg.red * (1.0 - a),
                fg.green * a + bg.green * (1.0 - a),
                fg.blue * a + bg.blue * (1.0 - a),
                1.0,
            ))
        });
        let regions = Arc::make_mut(&mut self.regions);
        regions.retain(|(previous, _)| !contains(rect, *previous));
        // Bound snapshot storage and lookup cost even in large documents.
        if regions.len() == 64 {
            regions.remove(0);
            self.clear = None;
        }
        regions.push((rect, color));
    }

    pub(crate) fn observe(
        &mut self,
        command: &SceneCommand,
        transform: Transform,
        clip: Option<Rect>,
        rectangular_clip: bool,
    ) {
        let solid = |brush: &Brush| match brush {
            Brush::Solid(c) => Some(*c),
            _ => None,
        };
        let (rect, color, inset) = match command {
            SceneCommand::Clear(color) => {
                if let Some(clip) = clip {
                    self.record(clip, rectangular_clip.then_some(*color));
                    return;
                }
                self.clear = (color.alpha >= 1.0).then_some(*color);
                self.regions = Arc::default();
                return;
            }
            SceneCommand::FillRect { rect, brush } => (*rect, solid(brush), 0.0),
            SceneCommand::FillRoundedRect {
                rect,
                radii,
                brush,
                border,
                ..
            } => {
                let inset = radii
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max)
                    .max(border.map_or(0.0, |b| b.width));
                (*rect, solid(brush), inset)
            }
            SceneCommand::DrawImage { rect, .. } | SceneCommand::DrawShaderRect { rect, .. } => {
                (*rect, None, 0.0)
            }
            SceneCommand::DrawImageQuad { points, .. } => {
                let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
                let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
                let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
                let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                (
                    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y),
                    None,
                    0.0,
                )
            }
            SceneCommand::FillPath { path, .. } => (path.bounds(), None, 0.0),
            _ => return,
        };
        if color.is_some_and(|color| color.alpha <= 0.0) {
            return;
        }
        let clipped = |r: Rect| {
            let r = transform.transform_rect_bbox(r);
            clip.map_or(Some(r), |clip| r.intersection(clip))
        };
        let Some(bounds) = clipped(rect) else {
            return;
        };
        let axis_aligned = rectangular_clip
            && transform.xy.abs() <= f32::EPSILON
            && transform.yx.abs() <= f32::EPSILON;
        self.record(
            bounds,
            if inset == 0.0 && axis_aligned {
                color
            } else {
                None
            },
        );
        if inset > 0.0 && axis_aligned && rect.width() > inset * 2.0 && rect.height() > inset * 2.0
        {
            if let Some(interior) = clipped(rect.inflate(-inset, -inset)) {
                self.record(interior, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_backdrops_do_not_leak_through_unknown_or_clipped_paint() {
        let mut background = TextBackground::default();
        background.observe(
            &SceneCommand::Clear(Color::WHITE),
            Transform::IDENTITY,
            None,
            true,
        );
        let tile = Rect::new(10.0, 10.0, 40.0, 40.0);
        background.observe(
            &SceneCommand::FillRect {
                rect: tile,
                brush: Color::BLACK.into(),
            },
            Transform::IDENTITY,
            Some(Rect::new(10.0, 10.0, 20.0, 40.0)),
            true,
        );
        assert_eq!(
            background.color_under(Rect::new(12.0, 12.0, 4.0, 4.0)),
            Some(Color::BLACK)
        );
        assert_eq!(
            background.color_under(Rect::new(32.0, 12.0, 4.0, 4.0)),
            Some(Color::WHITE)
        );
        assert_eq!(background.color_under(tile), None);
        background.observe(
            &SceneCommand::FillRect {
                rect: tile,
                brush: Color::BLACK.into(),
            },
            Transform::IDENTITY,
            Some(tile),
            false,
        );
        assert_eq!(
            background.color_under(Rect::new(12.0, 12.0, 4.0, 4.0)),
            None
        );
        assert_eq!(
            background.color_under(Rect::new(60.0, 12.0, 4.0, 4.0)),
            Some(Color::WHITE)
        );
    }
}
