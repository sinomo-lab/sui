pub use sui_animation::*;

use sui_core::{InvalidationKind, InvalidationRequest, InvalidationTarget};
use sui_runtime::EventCtx;

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationBindingInvalidation {
    pub binding: AnimationBinding,
    pub kind: InvalidationKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineTick {
    pub samples: Vec<SampledAnimationValue>,
    pub invalidations: Vec<AnimationBindingInvalidation>,
    pub should_continue: bool,
}

impl TimelineTick {
    pub fn request_current_widget_invalidations(&self, ctx: &mut EventCtx) {
        for invalidation in &self.invalidations {
            request_invalidation_kind(ctx, invalidation.kind);
        }
        if self.should_continue {
            ctx.request_animation_frame();
        }
    }
}

pub trait TimelineBindingSink {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelinePlayer {
    timeline: Timeline,
    playback: PlaybackState,
}

impl TimelinePlayer {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            playback: PlaybackState::default(),
        }
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    pub fn playback(&self) -> PlaybackState {
        self.playback
    }

    pub fn playback_mut(&mut self) -> &mut PlaybackState {
        &mut self.playback
    }

    pub fn play(&mut self) {
        self.playback.play();
    }

    pub fn pause(&mut self) {
        self.playback.pause();
    }

    pub fn stop(&mut self) {
        self.playback.stop();
    }

    pub fn seek(&mut self, time: f64) {
        self.playback.seek(time, self.timeline.duration);
    }

    pub fn sample(&self) -> Vec<SampledAnimationValue> {
        self.timeline.sample(self.playback.playhead)
    }

    pub fn tick<S>(&mut self, delta_seconds: f64, sink: &mut S) -> TimelineTick
    where
        S: TimelineBindingSink,
    {
        self.playback.tick(delta_seconds, self.timeline.duration);
        let samples = self.timeline.sample(self.playback.playhead);
        let mut invalidations = Vec::new();

        for sample in &samples {
            if sink.apply_animation_value(&sample.binding, sample.value) {
                invalidations.push(AnimationBindingInvalidation {
                    binding: sample.binding.clone(),
                    kind: invalidation_for_animation_property(&sample.binding.property),
                });
            }
        }

        TimelineTick {
            samples,
            invalidations,
            should_continue: self.playback.playing,
        }
    }

    pub fn tick_event<S>(
        &mut self,
        delta_seconds: f64,
        sink: &mut S,
        ctx: &mut EventCtx,
    ) -> TimelineTick
    where
        S: TimelineBindingSink,
    {
        let tick = self.tick(delta_seconds, sink);
        tick.request_current_widget_invalidations(ctx);
        tick
    }
}

pub fn invalidation_for_animation_property(property: &AnimationProperty) -> InvalidationKind {
    match property {
        AnimationProperty::LayerOpacity => InvalidationKind::Effect,
        AnimationProperty::LayerTranslation => InvalidationKind::Transform,
        AnimationProperty::Bounds => InvalidationKind::Measure,
        AnimationProperty::FillColor | AnimationProperty::Custom(_) => InvalidationKind::Paint,
    }
}

fn request_invalidation_kind(ctx: &mut EventCtx, kind: InvalidationKind) {
    match kind {
        InvalidationKind::Measure => ctx.request_measure(),
        InvalidationKind::Arrange => ctx.request_arrange(),
        InvalidationKind::Ordering => ctx.request_ordering(),
        InvalidationKind::Transform => ctx.request_transform(),
        InvalidationKind::Clip => ctx.request(InvalidationRequest::new(
            InvalidationTarget::Widget(ctx.widget_id()),
            InvalidationKind::Clip,
        )),
        InvalidationKind::Effect => ctx.request_effect(),
        InvalidationKind::Visibility => ctx.request_visibility(),
        InvalidationKind::Paint => ctx.request_paint(),
        InvalidationKind::HitTest => ctx.request_hit_test(),
        InvalidationKind::Text => ctx.request_text(),
        InvalidationKind::Semantics => ctx.request_semantics(),
        InvalidationKind::Resources => ctx.request_resources(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AnimationBinding, AnimationProperty, AnimationTargetId, AnimationValue, Clip, Easing,
        Keyframe, Timeline, TimelineBindingSink, TimelinePlayer, Track,
        invalidation_for_animation_property,
    };
    use sui_core::{Color, InvalidationKind, Vector};

    #[derive(Default)]
    struct DemoSink {
        opacity: f32,
        translation: Vector,
        fill: Color,
    }

    impl TimelineBindingSink for DemoSink {
        fn apply_animation_value(
            &mut self,
            binding: &AnimationBinding,
            value: AnimationValue,
        ) -> bool {
            match (&binding.property, value) {
                (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                    let changed = (self.opacity - value).abs() > f32::EPSILON;
                    self.opacity = value;
                    changed
                }
                (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                    let changed = self.translation != value;
                    self.translation = value;
                    changed
                }
                (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                    let changed = self.fill != value;
                    self.fill = value;
                    changed
                }
                _ => false,
            }
        }
    }

    fn binding(property: AnimationProperty) -> AnimationBinding {
        AnimationBinding::new(AnimationTargetId::new("preview"), property)
    }

    #[test]
    fn animation_property_maps_to_retained_invalidation_kinds() {
        assert_eq!(
            invalidation_for_animation_property(&AnimationProperty::LayerOpacity),
            InvalidationKind::Effect
        );
        assert_eq!(
            invalidation_for_animation_property(&AnimationProperty::LayerTranslation),
            InvalidationKind::Transform
        );
        assert_eq!(
            invalidation_for_animation_property(&AnimationProperty::FillColor),
            InvalidationKind::Paint
        );
    }

    #[test]
    fn timeline_player_applies_samples_and_reports_invalidations() {
        let timeline = Timeline::new(1.0).with_clip(
            Clip::new("intro", 0.0, 1.0)
                .with_track(
                    Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                        Keyframe::new(0.0, AnimationValue::Scalar(0.0)).with_easing(Easing::Linear),
                        Keyframe::new(1.0, AnimationValue::Scalar(1.0)),
                    ]),
                )
                .with_track(
                    Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                        Keyframe::new(0.0, AnimationValue::Vector(Vector::ZERO))
                            .with_easing(Easing::Linear),
                        Keyframe::new(1.0, AnimationValue::Vector(Vector::new(10.0, 0.0))),
                    ]),
                ),
        );
        let mut player = TimelinePlayer::new(timeline);
        let mut sink = DemoSink::default();
        player.play();

        let tick = player.tick(0.5, &mut sink);

        assert!(tick.should_continue);
        assert_eq!(tick.samples.len(), 2);
        assert_eq!(tick.invalidations.len(), 2);
        assert!((sink.opacity - 0.5).abs() < 1e-6);
        assert_eq!(sink.translation, Vector::new(5.0, 0.0));
    }
}
