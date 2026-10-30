//! Partial redraw: which part of a persistent scene target a frame changes.
//!
//! A target that keeps its pixels between frames only needs the region where
//! the frame's fragments differ from the previous frame's to be cleared and
//! drawn again. Anything this module cannot account for redraws everything.

use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

use sui_core::{Rect, Size, Vector};
use sui_scene::ImageRegistry;

use crate::draw::DrawOpArena;
use crate::output::SdrFit;
use crate::retained::RetainedPacketId;

/// Damage above this share of the target redraws everything; clearing and
/// scissoring most of the frame saves nothing.
const FULL_REDRAW_AREA_FRACTION: f64 = 0.5;

/// Pixels added around each fragment's bounds, so antialiased edges and
/// rounding never leave a stale fringe.
const BOUNDS_MARGIN: i64 = 2;

/// A rectangle of target pixels, `min` inclusive and `max` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub(crate) min_x: u32,
    pub(crate) min_y: u32,
    pub(crate) max_x: u32,
    pub(crate) max_y: u32,
}

impl PixelRect {
    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn area(self) -> u64 {
        u64::from(self.max_x - self.min_x) * u64::from(self.max_y - self.min_y)
    }

    pub(crate) fn intersection(self, other: Self) -> Option<Self> {
        let rect = Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        };
        (rect.min_x < rect.max_x && rect.min_y < rect.max_y).then_some(rect)
    }
}

/// What a frame redraws in its persistent scene target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedrawRegion {
    /// Clear the target and draw every fragment.
    Full,
    /// Keep the target, clear this rectangle, and draw only inside it.
    Partial(PixelRect),
    /// The target already shows this frame.
    Nothing,
}

/// One fragment as a frame places it.
#[derive(Debug, Clone)]
pub(crate) struct FragmentPlacement {
    pub(crate) id: RetainedPacketId,
    /// Held, not just compared by address, so a freed arena's address cannot
    /// be reused by different geometry and look unchanged.
    pub(crate) source: Arc<DrawOpArena>,
    pub(crate) translation: Vector,
    pub(crate) clip: Option<Rect>,
    /// Target pixels the fragment can touch; `None` when it touches none.
    pub(crate) bounds: Option<PixelRect>,
}

impl FragmentPlacement {
    fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
            && self.translation == other.translation
            && self.clip == other.clip
    }
}

/// Everything besides its fragments that a frame's pixels depend on.
#[derive(Debug, Clone)]
pub(crate) struct FrameInputs {
    pub(crate) viewport: Size,
    pub(crate) framebuffer_size: (u32, u32),
    pub(crate) sdr_fit: SdrFit,
    /// Held so an updated registry can't reuse a freed one's address.
    pub(crate) image_registry: Arc<ImageRegistry>,
    /// External images can change every frame without the scene changing.
    pub(crate) has_external_images: bool,
}

impl FrameInputs {
    fn same_as(&self, other: &Self) -> bool {
        self.viewport == other.viewport
            && self.framebuffer_size == other.framebuffer_size
            && self.sdr_fit == other.sdr_fit
            && Arc::ptr_eq(&self.image_registry, &other.image_registry)
            && !self.has_external_images
            && !other.has_external_images
    }
}

/// What a persistent target last showed.
#[derive(Debug, Clone)]
pub(crate) struct TargetContents {
    /// The texture drawn into; state from one target never describes
    /// another, such as after a window switches between SDR and HDR output.
    pub(crate) target: Option<wgpu::Texture>,
    pub(crate) inputs: FrameInputs,
    pub(crate) placements: Vec<FragmentPlacement>,
}

/// Whether native windows on this platform redraw only what changed, into a
/// persistent texture copied to the swapchain. Desktop GPUs copy a frame in
/// a fraction of a millisecond; on tile-based mobile GPUs reading the kept
/// frame back and copying it can cost more than redrawing, so they redraw
/// everything until measured on devices.
pub(crate) const PERSISTENT_SWAPCHAIN_REDRAW: bool = cfg!(not(any(
    target_os = "android",
    target_os = "ios",
    target_arch = "wasm32"
)));

/// The region of a target showing `previous` that must be redrawn to show
/// `current` instead.
pub(crate) fn plan_redraw(
    previous: Option<&TargetContents>,
    current: &TargetContents,
) -> RedrawRegion {
    let Some(previous) = previous else {
        return RedrawRegion::Full;
    };
    if previous.target.is_none() || previous.target != current.target {
        return RedrawRegion::Full;
    }
    if !previous.inputs.same_as(&current.inputs) {
        return RedrawRegion::Full;
    }
    // Most frames keep the same packets in the same order.
    if previous.placements.len() == current.placements.len()
        && previous
            .placements
            .iter()
            .zip(&current.placements)
            .all(|(before, after)| before.id == after.id)
    {
        let mut damage = None::<PixelRect>;
        for (before, after) in previous.placements.iter().zip(&current.placements) {
            if !before.same_as(after) {
                for bounds in [before.bounds, after.bounds].into_iter().flatten() {
                    damage = Some(damage.map_or(bounds, |damage| damage.union(bounds)));
                }
            }
        }
        return limit_damage(damage, current.inputs.framebuffer_size);
    }

    let previous_index = previous
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| (placement.id, index))
        .collect::<FxHashMap<_, _>>();
    let mut damage = None::<PixelRect>;
    let mut add = |bounds: Option<PixelRect>| {
        if let Some(bounds) = bounds {
            damage = Some(damage.map_or(bounds, |damage| damage.union(bounds)));
        }
    };
    let mut last_index = None;
    let mut kept = 0;
    for placement in &current.placements {
        match previous_index.get(&placement.id) {
            Some(&index) => {
                // Reordered fragments change which one is drawn on top
                // anywhere they overlap.
                if last_index.is_some_and(|last| index < last) {
                    return RedrawRegion::Full;
                }
                last_index = Some(index);
                kept += 1;
                let before = &previous.placements[index];
                if !before.same_as(placement) {
                    add(before.bounds);
                    add(placement.bounds);
                }
            }
            None => add(placement.bounds),
        }
    }
    if kept < previous.placements.len() {
        let current_ids = current
            .placements
            .iter()
            .map(|placement| placement.id)
            .collect::<FxHashSet<_>>();
        for placement in &previous.placements {
            if !current_ids.contains(&placement.id) {
                add(placement.bounds);
            }
        }
    }

    limit_damage(damage, current.inputs.framebuffer_size)
}

fn limit_damage(damage: Option<PixelRect>, framebuffer_size: (u32, u32)) -> RedrawRegion {
    let Some(damage) = damage else {
        return RedrawRegion::Nothing;
    };
    let total = u64::from(framebuffer_size.0) * u64::from(framebuffer_size.1);
    if damage.area() as f64 > total as f64 * FULL_REDRAW_AREA_FRACTION {
        RedrawRegion::Full
    } else {
        RedrawRegion::Partial(damage)
    }
}

/// The target pixels a fragment with normalized-device `ndc_bounds`, moved
/// by `translation` and clipped to `clip` (both logical), can touch.
pub(crate) fn fragment_pixel_bounds(
    ndc_bounds: [f32; 4],
    translation: Vector,
    clip: Option<Rect>,
    viewport: Size,
    framebuffer_size: (u32, u32),
) -> Option<PixelRect> {
    if !ndc_bounds.iter().all(|value| value.is_finite()) || viewport.is_empty() {
        return None;
    }
    let (width, height) = framebuffer_size;
    let scale_x = f64::from(width) / f64::from(viewport.width);
    let scale_y = f64::from(height) / f64::from(viewport.height);
    let mut min_x = (f64::from(ndc_bounds[0]) + 1.0) * 0.5 * f64::from(width)
        + f64::from(translation.x) * scale_x;
    let mut max_x = (f64::from(ndc_bounds[2]) + 1.0) * 0.5 * f64::from(width)
        + f64::from(translation.x) * scale_x;
    let mut min_y = (1.0 - f64::from(ndc_bounds[3])) * 0.5 * f64::from(height)
        + f64::from(translation.y) * scale_y;
    let mut max_y = (1.0 - f64::from(ndc_bounds[1])) * 0.5 * f64::from(height)
        + f64::from(translation.y) * scale_y;
    if let Some(clip) = clip {
        min_x = min_x.max(f64::from(clip.x()) * scale_x);
        min_y = min_y.max(f64::from(clip.y()) * scale_y);
        max_x = max_x.min(f64::from(clip.max_x()) * scale_x);
        max_y = max_y.min(f64::from(clip.max_y()) * scale_y);
    }
    let clamp = |value: f64, margin: i64, limit: u32| {
        (value as i64 + margin).clamp(0, i64::from(limit)) as u32
    };
    let rect = PixelRect {
        min_x: clamp(min_x.floor(), -BOUNDS_MARGIN, width),
        min_y: clamp(min_y.floor(), -BOUNDS_MARGIN, height),
        max_x: clamp(max_x.ceil(), BOUNDS_MARGIN, width),
        max_y: clamp(max_y.ceil(), BOUNDS_MARGIN, height),
    };
    (rect.min_x < rect.max_x && rect.min_y < rect.max_y).then_some(rect)
}
