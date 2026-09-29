pub use sui_animation::*;

use sui_core::{InvalidationKind, InvalidationRequest, InvalidationTarget};
use sui_runtime::{AnimateCtx, EventCtx, FrameClock, Motion, motion_policy};

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationBindingInvalidation {
    pub binding: AnimationBinding,
    pub kind: InvalidationKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineTick<'a> {
    pub samples: &'a [SampledAnimationValue],
    pub invalidations: &'a [AnimationBindingInvalidation],
    pub should_continue: bool,
}

impl TimelineTick<'_> {
    pub fn request_current_widget_invalidations(&self, ctx: &mut EventCtx) {
        for invalidation in self.invalidations {
            request_invalidation_kind(ctx, invalidation.kind);
        }
        if self.should_continue {
            ctx.request_animation_frame();
        }
    }
}

/// A widget state value that a transition can be started on from an event:
/// runtime-driven [`Progress`] or frame-driven [`MotionScalar`].
pub(crate) trait StateMotion {
    fn start(&mut self, target: f32, spec: AnimationSpec, ctx: &mut EventCtx) -> bool;
}

impl StateMotion for Progress {
    fn start(&mut self, target: f32, spec: AnimationSpec, ctx: &mut EventCtx) -> bool {
        self.animate(target, spec, ctx)
    }
}

impl StateMotion for MotionScalar {
    fn start(&mut self, target: f32, spec: AnimationSpec, ctx: &mut EventCtx) -> bool {
        self.set_target_event_with(target, spec, ctx)
    }
}

/// The scale popovers, menus, and select lists grow from as they appear.
pub(crate) const ENTRANCE_SCALE: f32 = 0.96;

/// An appearing surface's scale at `progress` (0 hidden, 1 shown), anchored
/// at the edge nearest its trigger. Reduced motion shows it at full size.
pub(crate) fn entrance_scale(progress: f32) -> f32 {
    1.0 - (1.0 - ENTRANCE_SCALE) * motion_policy().entrance_offset(progress)
}

pub trait TimelineBindingSink {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool;
}

/// A 0-to-1 progress value for widget state (hover, press, focus, toggle),
/// animated by the runtime.
///
/// Start a transition with [`Progress::animate`] and read the value for the
/// frame being painted with [`Progress::get`]. The runtime repaints the widget
/// until the transition ends, so the widget needs no animation-frame
/// handling. Transitions follow the app's [`MotionPolicy`] and keep momentum
/// when retargeted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    motion: Motion<f32>,
}

impl Progress {
    pub const EPSILON: f32 = 1e-4;

    pub const fn new(value: f32) -> Self {
        Self {
            motion: Motion::new(value),
        }
    }

    /// Mark this as movement (a sliding indicator, for example): reduced
    /// motion finishes it immediately.
    pub const fn movement(mut self) -> Self {
        self.motion = self.motion.movement();
        self
    }

    /// Invalidate `kind` instead of paint while animating, such as
    /// `Transform` or `Effect` for progress read in layer properties.
    pub const fn invalidating(mut self, kind: InvalidationKind) -> Self {
        self.motion = self.motion.invalidating(kind);
        self
    }

    /// The value at the time of the frame `clock` is producing.
    pub fn get(&self, clock: &impl FrameClock) -> f32 {
        self.motion.get(clock)
    }

    /// The value at `time`.
    pub fn at(&self, time: f64) -> f32 {
        self.motion.at(time)
    }

    pub fn target(&self) -> f32 {
        self.motion.target()
    }

    pub fn is_animating(&self, clock: &impl FrameClock) -> bool {
        self.motion.is_animating(clock)
    }

    /// Whether anything shows at `clock`'s frame: the value is above zero,
    /// heading above zero, or still animating.
    pub fn is_presented(&self, clock: &impl FrameClock) -> bool {
        self.get(clock) > Self::EPSILON || self.target() > Self::EPSILON || self.is_animating(clock)
    }

    /// Animate toward `target` (clamped to 0..=1) with `spec`. Returns
    /// whether a transition started.
    pub fn animate(&mut self, target: f32, spec: AnimationSpec, ctx: &mut impl AnimateCtx) -> bool {
        ctx.animate(&mut self.motion, target.clamp(0.0, 1.0), spec)
    }

    /// Rest at `value` without animating.
    pub fn jump_to(&mut self, value: f32) {
        self.motion.jump_to(value.clamp(0.0, 1.0));
    }
}

/// A 0-to-1 progress value for widget state transitions (hover, press,
/// focus, reveal) that the widget advances itself from animation frames.
/// Prefer [`Progress`], which the runtime advances.
///
/// Every transition follows the app's [`MotionPolicy`]: the time scale
/// stretches it, and with motion off it finishes immediately. Transitions
/// that move content can use the `set_movement_*` methods, which also finish
/// immediately under reduced motion. Retargeting mid-flight keeps momentum
/// (see [`MotionValue`]). Springs may briefly overshoot the 0-to-1 range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionScalar {
    pub value: f32,
    pub target: f32,
    motion: MotionValue<f32>,
}

impl MotionScalar {
    pub const EPSILON: f32 = 1e-4;

    pub const fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            motion: MotionValue::new(value),
        }
    }

    pub fn current(&self, time: f64) -> f32 {
        if self.motion.is_animating() {
            self.motion.value(time)
        } else {
            self.value
        }
    }

    pub fn is_animating(&self) -> bool {
        self.motion.is_animating()
    }

    pub fn set_target(&mut self, target: f32, time: f64, duration: f64, easing: Easing) -> bool {
        self.set_target_with(target, time, AnimationSpec::tween(duration, easing))
    }

    /// Start moving toward `target` at `time` with `spec`, as adjusted by the
    /// motion policy. Returns whether a transition is running.
    pub fn set_target_with(&mut self, target: f32, time: f64, spec: AnimationSpec) -> bool {
        self.retarget(target, time, spec.with_policy(motion_policy()))
    }

    /// Like [`MotionScalar::set_target`], for a transition that moves content
    /// (a sliding indicator, a reordering row): reduced motion jumps instead.
    pub fn set_movement_target(
        &mut self,
        target: f32,
        time: f64,
        duration: f64,
        easing: Easing,
    ) -> bool {
        let spec = AnimationSpec::tween(duration, easing).with_movement_policy(motion_policy());
        self.retarget(target, time, spec)
    }

    fn retarget(&mut self, target: f32, time: f64, spec: AnimationSpec) -> bool {
        let target = target.clamp(0.0, 1.0);
        let current = self.current(time);
        if (current - target).abs() < Self::EPSILON {
            self.value = target;
            self.target = target;
            self.motion.jump_to(target);
            return false;
        }

        self.target = target;
        let animating = self.motion.animate_to(target, time, spec);
        self.value = if animating { current } else { target };
        animating
    }

    pub fn set_target_event(
        &mut self,
        target: f32,
        duration: f64,
        easing: Easing,
        ctx: &mut EventCtx,
    ) -> bool {
        self.set_target_event_with(target, AnimationSpec::tween(duration, easing), ctx)
    }

    /// Like [`MotionScalar::set_target_with`] at the event's time, requesting
    /// an animation frame when a transition starts.
    pub fn set_target_event_with(
        &mut self,
        target: f32,
        spec: AnimationSpec,
        ctx: &mut EventCtx,
    ) -> bool {
        let should_animate = self.set_target_with(target, ctx.current_time(), spec);
        if should_animate {
            ctx.request_animation_frame();
        }
        should_animate
    }

    /// Like [`MotionScalar::set_movement_target`] at the event's time,
    /// requesting an animation frame when a transition starts.
    pub fn set_movement_target_event(
        &mut self,
        target: f32,
        duration: f64,
        easing: Easing,
        ctx: &mut EventCtx,
    ) -> bool {
        let should_animate = self.set_movement_target(target, ctx.current_time(), duration, easing);
        if should_animate {
            ctx.request_animation_frame();
        }
        should_animate
    }

    pub fn advance(&mut self, time: f64) -> bool {
        if !self.motion.is_animating() {
            return false;
        }

        let animating = self.motion.advance(time);
        self.value = if animating {
            self.motion.value(time)
        } else {
            self.target
        };
        animating
    }

    pub fn changed_since(&self, previous: f32) -> bool {
        (self.value - previous).abs() > Self::EPSILON
    }

    pub fn is_presented(&self) -> bool {
        self.value > Self::EPSILON || self.target > Self::EPSILON || self.motion.is_animating()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelinePlayer {
    timeline: Timeline,
    compiled: CompiledTimeline,
    compiled_dirty: bool,
    playback: PlaybackState,
    samples: SampleBuffer,
    invalidations: Vec<AnimationBindingInvalidation>,
}

impl TimelinePlayer {
    pub fn new(timeline: Timeline) -> Self {
        let compiled = timeline.compile();
        let sample_capacity = compiled.sample_capacity();
        Self {
            timeline,
            compiled,
            compiled_dirty: false,
            playback: PlaybackState::default(),
            samples: SampleBuffer::with_capacity(sample_capacity),
            invalidations: Vec::with_capacity(sample_capacity),
        }
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        self.compiled_dirty = true;
        &mut self.timeline
    }

    pub fn set_timeline(&mut self, timeline: Timeline) {
        self.timeline = timeline;
        self.recompile_timeline();
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

    pub fn sample_reusing_scratch(&mut self) -> &[SampledAnimationValue] {
        self.ensure_compiled_timeline();
        self.compiled
            .sample_into(self.playback.playhead, &mut self.samples);
        self.samples.samples()
    }

    pub fn tick<S>(&mut self, delta_seconds: f64, sink: &mut S) -> TimelineTick<'_>
    where
        S: TimelineBindingSink,
    {
        self.ensure_compiled_timeline();
        self.playback.tick(delta_seconds, self.compiled.duration());
        self.compiled
            .sample_into(self.playback.playhead, &mut self.samples);
        self.invalidations.clear();
        self.invalidations
            .reserve(self.samples.len().saturating_sub(self.invalidations.len()));

        for sample in self.samples.samples() {
            if sink.apply_animation_value(&sample.binding, sample.value) {
                self.invalidations.push(AnimationBindingInvalidation {
                    binding: sample.binding.clone(),
                    kind: invalidation_for_animation_property(&sample.binding.property),
                });
            }
        }

        TimelineTick {
            samples: self.samples.samples(),
            invalidations: &self.invalidations,
            should_continue: self.playback.playing,
        }
    }

    pub fn tick_event<S>(
        &mut self,
        delta_seconds: f64,
        sink: &mut S,
        ctx: &mut EventCtx,
    ) -> TimelineTick<'_>
    where
        S: TimelineBindingSink,
    {
        let tick = self.tick(delta_seconds, sink);
        tick.request_current_widget_invalidations(ctx);
        tick
    }

    fn ensure_compiled_timeline(&mut self) {
        if self.compiled_dirty {
            self.recompile_timeline();
        }
    }

    fn recompile_timeline(&mut self) {
        self.compiled = self.timeline.compile();
        self.compiled_dirty = false;
        let sample_capacity = self.compiled.sample_capacity();
        self.samples.reserve_capacity(sample_capacity);
        self.invalidations
            .reserve(sample_capacity.saturating_sub(self.invalidations.len()));
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
        AnimationBinding, AnimationProperty, AnimationSpec, AnimationTargetId, AnimationValue,
        Clip, Easing, Keyframe, MotionScalar, Timeline, TimelineBindingSink, TimelinePlayer, Track,
        invalidation_for_animation_property,
    };
    use sui_core::{Color, InvalidationKind, MotionPreference, Vector};

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

    /// Restores the thread's default motion settings when dropped.
    struct ResetMotion;

    impl Drop for ResetMotion {
        fn drop(&mut self) {
            sui_runtime::reset_motion_settings();
        }
    }

    #[test]
    fn motion_scalar_follows_the_time_scale_and_motion_off() {
        let _reset = ResetMotion;
        sui_runtime::set_motion_time_scale(0.5);
        let mut slow = MotionScalar::new(0.0);
        assert!(slow.set_target(1.0, 0.0, 0.2, Easing::Linear));
        assert!(slow.advance(0.2));
        assert!((slow.value - 0.5).abs() < 1e-5);
        assert!(!slow.advance(0.4));
        assert_eq!(slow.value, 1.0);

        sui_runtime::set_app_motion_preference(Some(MotionPreference::Off));
        let mut instant = MotionScalar::new(0.0);
        assert!(!instant.set_target(1.0, 0.0, 0.2, Easing::Linear));
        assert_eq!(instant.value, 1.0);
        assert!(!instant.is_animating());
    }

    #[test]
    fn movement_jumps_but_fades_still_play_under_reduced_motion() {
        let _reset = ResetMotion;
        sui_runtime::set_app_motion_preference(Some(MotionPreference::Reduced));

        let mut indicator = MotionScalar::new(0.0);
        assert!(!indicator.set_movement_target(1.0, 0.0, 0.2, Easing::Linear));
        assert_eq!(indicator.value, 1.0);

        let mut fade = MotionScalar::new(0.0);
        assert!(fade.set_target(1.0, 0.0, 0.2, Easing::Linear));
        assert!(fade.advance(0.1));
        assert!((fade.value - 0.5).abs() < 1e-5);
    }

    #[test]
    fn motion_scalar_turns_around_without_stopping() {
        let _reset = ResetMotion;
        let mut hover = MotionScalar::new(0.0);
        hover.set_target(1.0, 0.0, 0.2, Easing::EaseInOut);
        hover.advance(0.1);
        let rising = hover.value;

        // The pointer leaves halfway: the value keeps rising briefly before
        // falling back, instead of freezing and restarting from rest.
        assert!(hover.set_target(0.0, 0.1, 0.2, Easing::EaseInOut));
        hover.advance(0.12);
        assert!(hover.value > rising);
        assert!(!hover.advance(0.3));
        assert_eq!(hover.value, 0.0);
    }

    #[test]
    fn theme_motion_specs_match_their_tokens() {
        let motion = crate::theme::ThemeMotion::standard();

        assert_eq!(
            motion.hover_spec(),
            AnimationSpec::tween(motion.hover_duration(), motion.hover_easing())
        );
        assert_eq!(
            motion.entrance_spec().duration(),
            motion.entrance_duration()
        );
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
