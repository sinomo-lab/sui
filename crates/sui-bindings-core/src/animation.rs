use crate::values::{binding_animation_property_from_path, normalize_binding_name};
use sui::AnimatedValue;
use sui::AnimationBinding;
use sui::AnimationDocument;
use sui::AnimationEditorCommand;
use sui::AnimationEditorState;
use sui::AnimationPlayer;
use sui::AnimationTargetId;
use sui::AnimationValue;
use sui::Clip;
use sui::Color;
use sui::Easing;
use sui::Keyframe;
use sui::LoopMode;
use sui::Point;
use sui::Rect;
use sui::SampleBuffer;
use sui::Size;
use sui::SpringF32;
use sui::Stagger;
use sui::StaggerOrigin;
use sui::TableColumnAlignment;
use sui::Timeline;
use sui::TimelineMarker;
use sui::Track;
use sui::Transform;
use sui::Transition;
use sui::Vector;

/// A language-neutral animation value. Host bindings expose named constructors instead of the
/// Rust enum so Python and JavaScript callers can work with ordinary geometry and color objects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingAnimationValue {
    Scalar(f32),
    Point(Point),
    Vector(Vector),
    Size(Size),
    Rect(Rect),
    Color(Color),
    Transform(Transform),
}

impl BindingAnimationValue {
    pub const fn scalar(value: f32) -> Self {
        Self::Scalar(value)
    }

    pub const fn point(value: Point) -> Self {
        Self::Point(value)
    }

    pub const fn vector(value: Vector) -> Self {
        Self::Vector(value)
    }

    pub const fn size(value: Size) -> Self {
        Self::Size(value)
    }

    pub const fn rect(value: Rect) -> Self {
        Self::Rect(value)
    }

    pub const fn color(value: Color) -> Self {
        Self::Color(value)
    }

    pub const fn transform(value: Transform) -> Self {
        Self::Transform(value)
    }

    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "scalar",
            Self::Point(_) => "point",
            Self::Vector(_) => "vector",
            Self::Size(_) => "size",
            Self::Rect(_) => "rect",
            Self::Color(_) => "color",
            Self::Transform(_) => "transform",
        }
    }

    pub const fn as_scalar(&self) -> Option<f32> {
        match self {
            Self::Scalar(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_point(&self) -> Option<Point> {
        match self {
            Self::Point(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_vector(&self) -> Option<Vector> {
        match self {
            Self::Vector(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_size(&self) -> Option<Size> {
        match self {
            Self::Size(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_rect(&self) -> Option<Rect> {
        match self {
            Self::Rect(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(value) => Some(*value),
            _ => None,
        }
    }

    pub const fn as_transform(&self) -> Option<Transform> {
        match self {
            Self::Transform(value) => Some(*value),
            _ => None,
        }
    }
}

impl From<BindingAnimationValue> for AnimationValue {
    fn from(value: BindingAnimationValue) -> Self {
        match value {
            BindingAnimationValue::Scalar(value) => Self::Scalar(value),
            BindingAnimationValue::Point(value) => Self::Point(value),
            BindingAnimationValue::Vector(value) => Self::Vector(value),
            BindingAnimationValue::Size(value) => Self::Size(value),
            BindingAnimationValue::Rect(value) => Self::Rect(value),
            BindingAnimationValue::Color(value) => Self::Color(value),
            BindingAnimationValue::Transform(value) => Self::Transform(value),
        }
    }
}

impl From<AnimationValue> for BindingAnimationValue {
    fn from(value: AnimationValue) -> Self {
        match value {
            AnimationValue::Scalar(value) => Self::Scalar(value),
            AnimationValue::Point(value) => Self::Point(value),
            AnimationValue::Vector(value) => Self::Vector(value),
            AnimationValue::Size(value) => Self::Size(value),
            AnimationValue::Rect(value) => Self::Rect(value),
            AnimationValue::Color(value) => Self::Color(value),
            AnimationValue::Transform(value) => Self::Transform(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingTransition {
    pub(crate) inner: Transition<AnimationValue>,
}

impl BindingTransition {
    pub fn new(
        start: BindingAnimationValue,
        end: BindingAnimationValue,
        start_time: f64,
        duration: f64,
        easing: Easing,
    ) -> Self {
        Self {
            inner: Transition::new(start.into(), end.into(), start_time, duration, easing),
        }
    }

    pub fn progress(&self, time: f64) -> f32 {
        self.inner.progress(time)
    }

    pub fn sample(&self, time: f64) -> BindingAnimationValue {
        self.inner.sample(time).into()
    }

    pub fn is_complete(&self, time: f64) -> bool {
        self.inner.is_complete(time)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingSpring {
    pub(crate) inner: SpringF32,
}

impl BindingSpring {
    pub fn new(value: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            inner: SpringF32::new(value).with_config(stiffness, damping),
        }
    }

    pub fn step(&mut self, target: f32, delta_seconds: f64) -> f32 {
        self.inner.step(target, delta_seconds)
    }

    pub const fn value(&self) -> f32 {
        self.inner.value
    }

    pub const fn velocity(&self) -> f32 {
        self.inner.velocity
    }

    pub const fn stiffness(&self) -> f32 {
        self.inner.stiffness
    }

    pub const fn damping(&self) -> f32 {
        self.inner.damping
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingAnimatedValue {
    pub(crate) inner: AnimatedValue<AnimationValue>,
}

impl BindingAnimatedValue {
    pub fn new(initial: BindingAnimationValue, duration: f32, easing: Easing) -> Self {
        Self {
            inner: AnimatedValue::new(initial.into())
                .with_duration(duration)
                .with_easing(easing),
        }
    }

    pub fn set_duration(&mut self, seconds: f32) {
        self.inner.set_duration(seconds);
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.inner.set_easing(easing);
    }

    pub fn set_target(&mut self, target: BindingAnimationValue) {
        self.inner.set_target(target.into());
    }

    /// Start toward `target` after `delay` seconds of ticks.
    pub fn set_target_after(&mut self, target: BindingAnimationValue, delay: f64) {
        self.inner.set_target_after(target.into(), delay);
    }

    pub fn jump_to(&mut self, value: BindingAnimationValue) {
        self.inner.jump_to(value.into());
    }

    pub fn tick(&mut self, delta_seconds: f32) -> bool {
        self.inner.tick(delta_seconds)
    }

    pub fn value(&self) -> BindingAnimationValue {
        self.inner.value().into()
    }

    pub fn target(&self) -> BindingAnimationValue {
        self.inner.target().into()
    }

    pub fn is_animating(&self) -> bool {
        self.inner.is_animating()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingAnimationKeyframe {
    pub(crate) inner: Keyframe<AnimationValue>,
}

impl BindingAnimationKeyframe {
    pub fn new(time: f64, value: BindingAnimationValue, easing: Easing) -> Self {
        Self {
            inner: Keyframe::new(time, value.into()).with_easing(easing),
        }
    }

    pub const fn time(&self) -> f64 {
        self.inner.time
    }

    pub fn value(&self) -> BindingAnimationValue {
        self.inner.value.into()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationTrack {
    pub(crate) inner: Track<AnimationValue>,
}

impl BindingAnimationTrack {
    pub fn new(target: impl Into<String>, property: impl Into<String>) -> Self {
        let target = target.into();
        let property = property.into();
        Self {
            inner: Track::new(AnimationBinding::new(
                AnimationTargetId::new(target),
                binding_animation_property_from_path(&property),
            )),
        }
    }

    pub fn add_keyframe(&mut self, keyframe: BindingAnimationKeyframe) {
        self.inner.push_keyframe(keyframe.inner);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.enabled = enabled;
    }

    pub fn sample(&self, time: f64) -> Option<BindingAnimationValue> {
        self.inner.sample(time).map(Into::into)
    }

    pub fn target(&self) -> &str {
        self.inner.binding.target.as_str()
    }

    pub fn property(&self) -> &str {
        self.inner.binding.property.path()
    }

    pub fn keyframe_count(&self) -> usize {
        self.inner.keyframes.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationClip {
    pub(crate) inner: Clip<AnimationValue>,
}

impl BindingAnimationClip {
    pub fn new(id: impl Into<String>, start_time: f64, duration: f64) -> Self {
        Self {
            inner: Clip::new(id, start_time, duration),
        }
    }

    pub fn add_track(&mut self, track: BindingAnimationTrack) {
        self.inner.push_track(track.inner);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.enabled = enabled;
    }

    pub fn id(&self) -> &str {
        &self.inner.id
    }

    pub fn start_time(&self) -> f64 {
        self.inner.start_time
    }

    pub fn duration(&self) -> f64 {
        self.inner.duration
    }

    pub fn track_count(&self) -> usize {
        self.inner.tracks.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationSample {
    pub clip_id: String,
    pub target: String,
    pub property: String,
    pub time: f64,
    pub value: BindingAnimationValue,
}

pub(crate) fn binding_animation_samples(
    samples: Vec<sui::SampledAnimationValue>,
) -> Vec<BindingAnimationSample> {
    samples
        .into_iter()
        .map(|sample| BindingAnimationSample {
            clip_id: sample.clip_id,
            target: sample.binding.target.as_str().to_owned(),
            property: sample.binding.property.path().to_owned(),
            time: sample.time,
            value: sample.value.into(),
        })
        .collect()
}

/// A named point in time on a timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationMarker {
    pub name: String,
    pub time: f64,
}

impl From<TimelineMarker> for BindingAnimationMarker {
    fn from(marker: TimelineMarker) -> Self {
        Self {
            name: marker.name,
            time: marker.time,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationTimeline {
    pub(crate) inner: Timeline<AnimationValue>,
}

impl BindingAnimationTimeline {
    pub fn new(duration: f64) -> Self {
        Self {
            inner: Timeline::new(duration),
        }
    }

    pub fn add_clip(&mut self, clip: BindingAnimationClip) {
        self.inner.push_clip(clip.inner);
    }

    pub fn add_marker(&mut self, name: impl Into<String>, time: f64) {
        self.inner.push_marker(TimelineMarker::new(name, time));
    }

    pub fn markers(&self) -> Vec<BindingAnimationMarker> {
        self.inner.markers.iter().cloned().map(Into::into).collect()
    }

    pub fn duration(&self) -> f64 {
        self.inner.duration
    }

    pub fn clip_count(&self) -> usize {
        self.inner.clips.len()
    }

    pub fn sample(&self, time: f64) -> Vec<BindingAnimationSample> {
        binding_animation_samples(self.inner.sample(time))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationPlayer {
    pub(crate) inner: AnimationPlayer<AnimationValue>,
}

impl BindingAnimationPlayer {
    pub fn new(timeline: &BindingAnimationTimeline) -> Self {
        Self {
            inner: AnimationPlayer::from_compiled(timeline.inner.compile()),
        }
    }

    pub fn play(&mut self) {
        self.inner.play();
    }

    pub fn pause(&mut self) {
        self.inner.pause();
    }

    pub fn stop(&mut self) {
        self.inner.stop();
    }

    pub fn seek(&mut self, time: f64) {
        self.inner.seek(time);
    }

    pub fn set_repeat(&mut self, repeat: bool) {
        self.set_loop_mode(if repeat {
            LoopMode::Repeat
        } else {
            LoopMode::Once
        });
    }

    pub fn set_loop_mode(&mut self, loop_mode: LoopMode) {
        self.inner.playback_mut().loop_mode = loop_mode;
    }

    pub fn loop_mode(&self) -> LoopMode {
        self.inner.playback().loop_mode
    }

    /// Wait `seconds` before playing from the start.
    pub fn set_start_delay(&mut self, seconds: f64) {
        self.inner.playback_mut().start_delay = seconds;
    }

    /// Hold `seconds` at each end before looping.
    pub fn set_loop_delay(&mut self, seconds: f64) {
        self.inner.playback_mut().loop_delay = seconds;
    }

    /// The markers the playhead passed in the last tick, in order.
    pub fn passed_markers(&self) -> Vec<BindingAnimationMarker> {
        self.inner
            .passed_markers()
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }

    pub fn set_playback_rate(&mut self, rate: f64) {
        self.inner.playback_mut().playback_rate = rate;
    }

    pub fn playhead(&self) -> f64 {
        self.inner.playback().playhead
    }

    pub fn is_playing(&self) -> bool {
        self.inner.playback().playing
    }

    pub fn sample(&self) -> Vec<BindingAnimationSample> {
        binding_animation_samples(self.inner.sample())
    }

    pub fn tick(&mut self, delta_seconds: f64) -> Vec<BindingAnimationSample> {
        let mut samples = SampleBuffer::new();
        self.inner.tick_into(delta_seconds, &mut samples);
        binding_animation_samples(samples.into_samples())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationDocument {
    pub(crate) inner: AnimationDocument,
}

impl BindingAnimationDocument {
    pub fn new(name: impl Into<String>, timeline: BindingAnimationTimeline) -> Self {
        Self {
            inner: AnimationDocument::new(name, timeline.inner),
        }
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        AnimationDocument::from_document_format(input)
            .map(|inner| Self { inner })
            .map_err(|error| error.to_string())
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn timeline(&self) -> BindingAnimationTimeline {
        BindingAnimationTimeline {
            inner: self.inner.timeline.clone(),
        }
    }

    pub fn to_document_format(&self) -> String {
        self.inner.to_document_format()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingAnimationEditor {
    pub(crate) inner: AnimationEditorState,
}

impl BindingAnimationEditor {
    pub fn new(document: BindingAnimationDocument) -> Self {
        Self {
            inner: AnimationEditorState::new(document.inner),
        }
    }

    pub fn document(&self) -> BindingAnimationDocument {
        BindingAnimationDocument {
            inner: self.inner.document.clone(),
        }
    }

    pub fn set_playhead(&mut self, time: f64) {
        self.inner
            .apply_command(AnimationEditorCommand::SetPlayhead(time));
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.inner
            .apply_command(AnimationEditorCommand::SetZoom(zoom));
    }

    pub fn set_scroll(&mut self, scroll: f32) {
        self.inner
            .apply_command(AnimationEditorCommand::SetScroll(scroll));
    }

    pub fn set_snapping(&mut self, enabled: bool, interval: f64) {
        self.inner
            .apply_command(AnimationEditorCommand::SetSnapping(if enabled {
                sui::TimelineSnap::new(interval)
            } else {
                sui::TimelineSnap::disabled()
            }));
    }

    pub fn add_keyframe(
        &mut self,
        clip_index: usize,
        track_index: usize,
        keyframe: BindingAnimationKeyframe,
    ) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::AddKeyframe {
                clip_index,
                track_index,
                keyframe: keyframe.inner,
            })
    }

    pub fn update_keyframe_easing(
        &mut self,
        clip_index: usize,
        track_index: usize,
        keyframe_index: usize,
        easing: Easing,
    ) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::UpdateKeyframeEasing {
                selection: sui::KeyframeSelection {
                    clip_index,
                    track_index,
                    keyframe_index,
                },
                easing,
            })
    }

    pub fn remove_keyframe(
        &mut self,
        clip_index: usize,
        track_index: usize,
        keyframe_index: usize,
    ) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::RemoveKeyframe(
                sui::KeyframeSelection {
                    clip_index,
                    track_index,
                    keyframe_index,
                },
            ))
    }

    pub fn add_marker(&mut self, name: impl Into<String>, time: f64) -> bool {
        self.inner.apply_command(AnimationEditorCommand::AddMarker {
            name: name.into(),
            time,
        })
    }

    pub fn move_marker(&mut self, index: usize, time: f64) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::MoveMarker { index, time })
    }

    pub fn rename_marker(&mut self, index: usize, name: impl Into<String>) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::RenameMarker {
                index,
                name: name.into(),
            })
    }

    pub fn remove_marker(&mut self, index: usize) -> bool {
        self.inner
            .apply_command(AnimationEditorCommand::RemoveMarker(index))
    }

    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.inner.redo()
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub fn playhead(&self) -> f64 {
        self.inner.playback.playhead
    }

    pub fn zoom(&self) -> f32 {
        self.inner.zoom
    }

    pub fn scroll(&self) -> f32 {
        self.inner.scroll
    }
}

/// Delays that start a group of animations one after another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BindingStagger {
    pub(crate) inner: Stagger,
}

impl BindingStagger {
    pub fn new(interval: f64) -> Self {
        Self {
            inner: Stagger::new(interval),
        }
    }

    pub fn set_origin(&mut self, origin: StaggerOrigin) {
        self.inner = self.inner.from(origin);
    }

    pub fn set_max_delay(&mut self, max_delay: Option<f64>) {
        self.inner.max_delay = max_delay;
    }

    pub fn delay(&self, index: usize, count: usize) -> f64 {
        self.inner.delay(index, count)
    }

    pub fn interval(&self) -> f64 {
        self.inner.interval
    }
}

pub fn binding_loop_mode_from_name(value: &str) -> Option<LoopMode> {
    match normalize_binding_name(value).as_str() {
        "once" => Some(LoopMode::Once),
        "repeat" | "loop" => Some(LoopMode::Repeat),
        "pingpong" | "alternate" => Some(LoopMode::PingPong),
        _ => None,
    }
}

pub fn binding_loop_mode_name(mode: LoopMode) -> &'static str {
    match mode {
        LoopMode::Once => "once",
        LoopMode::Repeat => "repeat",
        LoopMode::PingPong => "ping-pong",
    }
}

/// A stagger origin from its name: `"first"`, `"last"`, `"center"`, or an
/// item index.
pub fn binding_stagger_origin_from_name(value: &str) -> Option<StaggerOrigin> {
    match normalize_binding_name(value).as_str() {
        "first" | "start" => Some(StaggerOrigin::First),
        "last" | "end" => Some(StaggerOrigin::Last),
        "center" | "centre" | "middle" => Some(StaggerOrigin::Center),
        other => other.parse().ok().map(StaggerOrigin::Index),
    }
}

pub fn binding_table_column_alignment_from_name(value: &str) -> Option<TableColumnAlignment> {
    match normalize_binding_name(value).as_str() {
        "start" | "left" => Some(TableColumnAlignment::Start),
        "center" | "centre" | "middle" => Some(TableColumnAlignment::Center),
        "end" | "right" => Some(TableColumnAlignment::End),
        _ => None,
    }
}
