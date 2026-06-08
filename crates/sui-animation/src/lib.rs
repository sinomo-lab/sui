#![forbid(unsafe_code)]

use std::fmt;

use sui_core::{Color, ColorSpace, Point, Rect, Size, Transform, Vector};

pub const ANIMATION_DOCUMENT_VERSION: u32 = 1;

pub trait Interpolate: Sized {
    fn interpolate(from: Self, to: Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        from + ((to - from) * t.clamp(0.0, 1.0))
    }
}

impl Interpolate for Point {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Point::new(
            f32::interpolate(from.x, to.x, t),
            f32::interpolate(from.y, to.y, t),
        )
    }
}

impl Interpolate for Vector {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Vector::new(
            f32::interpolate(from.x, to.x, t),
            f32::interpolate(from.y, to.y, t),
        )
    }
}

impl Interpolate for Size {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Size::new(
            f32::interpolate(from.width, to.width, t),
            f32::interpolate(from.height, to.height, t),
        )
    }
}

impl Interpolate for Rect {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        Rect::from_origin_size(
            Point::interpolate(from.origin, to.origin, t),
            Size::interpolate(from.size, to.size, t),
        )
    }
}

impl Interpolate for Transform {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Transform::new(
            f32::interpolate(from.xx, to.xx, t),
            f32::interpolate(from.yx, to.yx, t),
            f32::interpolate(from.xy, to.xy, t),
            f32::interpolate(from.yy, to.yy, t),
            f32::interpolate(from.dx, to.dx, t),
            f32::interpolate(from.dy, to.dy, t),
        )
    }
}

impl Interpolate for Color {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Color::new(
            from.space,
            f32::interpolate(from.red, to.red, t),
            f32::interpolate(from.green, to.green, t),
            f32::interpolate(from.blue, to.blue, t),
            f32::interpolate(from.alpha, to.alpha, t),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
}

impl Easing {
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - ((-2.0 * t + 2.0).powi(2) * 0.5)
                }
            }
            Self::CubicBezier { x1, y1, x2, y2 } => sample_cubic_bezier(x1, y1, x2, y2, t),
        }
    }
}

impl Default for Easing {
    fn default() -> Self {
        Self::Linear
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition<T> {
    pub start: T,
    pub end: T,
    pub start_time: f64,
    pub duration: f64,
    pub easing: Easing,
}

impl<T> Transition<T>
where
    T: Copy + Interpolate,
{
    pub fn new(start: T, end: T, start_time: f64, duration: f64, easing: Easing) -> Self {
        Self {
            start,
            end,
            start_time,
            duration: duration.max(0.0),
            easing,
        }
    }

    pub fn progress(&self, time: f64) -> f32 {
        if self.duration <= f64::EPSILON {
            return 1.0;
        }
        ((time - self.start_time) / self.duration).clamp(0.0, 1.0) as f32
    }

    pub fn sample(&self, time: f64) -> T {
        T::interpolate(
            self.start,
            self.end,
            self.easing.sample(self.progress(time)),
        )
    }

    pub fn is_complete(&self, time: f64) -> bool {
        self.progress(time) >= 1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringF32 {
    pub value: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringF32 {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            stiffness: 180.0,
            damping: 24.0,
        }
    }

    pub fn with_config(mut self, stiffness: f32, damping: f32) -> Self {
        self.stiffness = stiffness.max(0.0);
        self.damping = damping.max(0.0);
        self
    }

    pub fn step(&mut self, target: f32, delta: f64) -> f32 {
        let dt = delta.max(0.0) as f32;
        if dt <= f32::EPSILON {
            return self.value;
        }

        let displacement = target - self.value;
        let acceleration = (displacement * self.stiffness) - (self.velocity * self.damping);
        self.velocity += acceleration * dt;
        self.value += self.velocity * dt;
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blink {
    pub period: f64,
    pub duty_cycle: f32,
    pub phase: f64,
}

impl Blink {
    pub fn new(period: f64) -> Self {
        Self {
            period: period.max(f64::EPSILON),
            duty_cycle: 0.5,
            phase: 0.0,
        }
    }

    pub fn with_duty_cycle(mut self, duty_cycle: f32) -> Self {
        self.duty_cycle = duty_cycle.clamp(0.0, 1.0);
        self
    }

    pub fn with_phase(mut self, phase: f64) -> Self {
        self.phase = phase;
        self
    }

    pub fn is_on(&self, time: f64) -> bool {
        let cycle = ((time + self.phase).rem_euclid(self.period)) / self.period;
        cycle < self.duty_cycle as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pulse {
    pub period: f64,
    pub min: f32,
    pub max: f32,
    pub phase: f64,
    pub easing: Easing,
}

impl Pulse {
    pub fn new(period: f64, min: f32, max: f32) -> Self {
        Self {
            period: period.max(f64::EPSILON),
            min,
            max,
            phase: 0.0,
            easing: Easing::EaseInOut,
        }
    }

    pub fn with_phase(mut self, phase: f64) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn sample(&self, time: f64) -> f32 {
        let cycle = ((time + self.phase).rem_euclid(self.period)) / self.period;
        let triangle = if cycle <= 0.5 {
            (cycle * 2.0) as f32
        } else {
            ((1.0 - cycle) * 2.0) as f32
        };
        f32::interpolate(self.min, self.max, self.easing.sample(triangle))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimatedValue<T> {
    start: T,
    target: T,
    current: T,
    elapsed: f32,
    duration: f32,
    easing: Easing,
    animating: bool,
}

impl<T> AnimatedValue<T>
where
    T: Interpolate + Copy,
{
    pub fn new(initial: T) -> Self {
        Self {
            start: initial,
            target: initial,
            current: initial,
            elapsed: 0.0,
            duration: 0.2,
            easing: Easing::EaseInOut,
            animating: false,
        }
    }

    pub fn with_duration(mut self, seconds: f32) -> Self {
        self.duration = seconds.max(0.0);
        self
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn set_duration(&mut self, seconds: f32) {
        self.duration = seconds.max(0.0);
    }

    pub fn set_easing(&mut self, easing: Easing) {
        self.easing = easing;
    }

    pub fn set_target(&mut self, target: T) {
        self.target = target;
        if self.duration <= f32::EPSILON {
            self.start = target;
            self.current = target;
            self.elapsed = 0.0;
            self.animating = false;
            return;
        }
        self.start = self.current;
        self.elapsed = 0.0;
        self.animating = true;
    }

    pub fn jump_to(&mut self, value: T) {
        self.start = value;
        self.target = value;
        self.current = value;
        self.elapsed = 0.0;
        self.animating = false;
    }

    pub fn tick(&mut self, delta_seconds: f32) -> bool {
        if !self.animating {
            return false;
        }
        self.elapsed += delta_seconds.max(0.0);
        let progress = if self.duration <= f32::EPSILON {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        };
        let eased = self.easing.sample(progress);
        self.current = T::interpolate(self.start, self.target, eased);
        if progress >= 1.0 {
            self.current = self.target;
            self.animating = false;
            return false;
        }
        true
    }

    pub fn value(&self) -> T {
        self.current
    }

    pub fn target(&self) -> T {
        self.target
    }

    pub fn is_animating(&self) -> bool {
        self.animating
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationTargetId(String);

impl AnimationTargetId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AnimationTargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for AnimationTargetId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AnimationTargetId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationPropertyPath(String);

impl AnimationPropertyPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AnimationPropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for AnimationPropertyPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for AnimationPropertyPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnimationProperty {
    LayerOpacity,
    LayerTranslation,
    FillColor,
    Bounds,
    Custom(AnimationPropertyPath),
}

impl AnimationProperty {
    pub fn path(&self) -> &str {
        match self {
            Self::LayerOpacity => "layer.opacity",
            Self::LayerTranslation => "layer.translation",
            Self::FillColor => "fill.color",
            Self::Bounds => "bounds",
            Self::Custom(path) => path.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationBinding {
    pub target: AnimationTargetId,
    pub property: AnimationProperty,
}

impl AnimationBinding {
    pub fn new(target: impl Into<AnimationTargetId>, property: AnimationProperty) -> Self {
        Self {
            target: target.into(),
            property,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationValueKind {
    Scalar,
    Point,
    Vector,
    Size,
    Rect,
    Color,
    Transform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationValue {
    Scalar(f32),
    Point(Point),
    Vector(Vector),
    Size(Size),
    Rect(Rect),
    Color(Color),
    Transform(Transform),
}

impl AnimationValue {
    pub fn kind(self) -> AnimationValueKind {
        match self {
            Self::Scalar(_) => AnimationValueKind::Scalar,
            Self::Point(_) => AnimationValueKind::Point,
            Self::Vector(_) => AnimationValueKind::Vector,
            Self::Size(_) => AnimationValueKind::Size,
            Self::Rect(_) => AnimationValueKind::Rect,
            Self::Color(_) => AnimationValueKind::Color,
            Self::Transform(_) => AnimationValueKind::Transform,
        }
    }

    pub fn as_scalar(self) -> Option<f32> {
        match self {
            Self::Scalar(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_vector(self) -> Option<Vector> {
        match self {
            Self::Vector(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_color(self) -> Option<Color> {
        match self {
            Self::Color(value) => Some(value),
            _ => None,
        }
    }
}

impl Interpolate for AnimationValue {
    fn interpolate(from: Self, to: Self, t: f32) -> Self {
        match (from, to) {
            (Self::Scalar(from), Self::Scalar(to)) => Self::Scalar(f32::interpolate(from, to, t)),
            (Self::Point(from), Self::Point(to)) => Self::Point(Point::interpolate(from, to, t)),
            (Self::Vector(from), Self::Vector(to)) => {
                Self::Vector(Vector::interpolate(from, to, t))
            }
            (Self::Size(from), Self::Size(to)) => Self::Size(Size::interpolate(from, to, t)),
            (Self::Rect(from), Self::Rect(to)) => Self::Rect(Rect::interpolate(from, to, t)),
            (Self::Color(from), Self::Color(to)) => Self::Color(Color::interpolate(from, to, t)),
            (Self::Transform(from), Self::Transform(to)) => {
                Self::Transform(Transform::interpolate(from, to, t))
            }
            (from, to) => {
                if t >= 1.0 {
                    to
                } else {
                    from
                }
            }
        }
    }
}

impl From<f32> for AnimationValue {
    fn from(value: f32) -> Self {
        Self::Scalar(value)
    }
}

impl From<Point> for AnimationValue {
    fn from(value: Point) -> Self {
        Self::Point(value)
    }
}

impl From<Vector> for AnimationValue {
    fn from(value: Vector) -> Self {
        Self::Vector(value)
    }
}

impl From<Size> for AnimationValue {
    fn from(value: Size) -> Self {
        Self::Size(value)
    }
}

impl From<Rect> for AnimationValue {
    fn from(value: Rect) -> Self {
        Self::Rect(value)
    }
}

impl From<Color> for AnimationValue {
    fn from(value: Color) -> Self {
        Self::Color(value)
    }
}

impl From<Transform> for AnimationValue {
    fn from(value: Transform) -> Self {
        Self::Transform(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe<T> {
    pub time: f64,
    pub value: T,
    pub easing: Easing,
}

impl<T> Keyframe<T> {
    pub fn new(time: f64, value: T) -> Self {
        Self {
            time: time.max(0.0),
            value,
            easing: Easing::Linear,
        }
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track<T = AnimationValue> {
    pub binding: AnimationBinding,
    pub keyframes: Vec<Keyframe<T>>,
    pub enabled: bool,
}

impl<T> Track<T> {
    pub fn new(binding: AnimationBinding) -> Self {
        Self {
            binding,
            keyframes: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_keyframes(mut self, keyframes: impl IntoIterator<Item = Keyframe<T>>) -> Self {
        self.keyframes.extend(keyframes);
        self
    }

    pub fn push_keyframe(&mut self, keyframe: Keyframe<T>) {
        self.keyframes.push(keyframe);
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl<T> Track<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Option<T> {
        if !self.enabled || self.keyframes.is_empty() {
            return None;
        }

        let mut first = &self.keyframes[0];
        let mut last = &self.keyframes[0];
        let mut previous = None;
        let mut next = None;

        for keyframe in &self.keyframes {
            if keyframe.time < first.time {
                first = keyframe;
            }
            if keyframe.time > last.time {
                last = keyframe;
            }
            if keyframe.time <= time
                && previous
                    .map(|candidate: &Keyframe<T>| keyframe.time >= candidate.time)
                    .unwrap_or(true)
            {
                previous = Some(keyframe);
            }
            if keyframe.time >= time
                && next
                    .map(|candidate: &Keyframe<T>| keyframe.time <= candidate.time)
                    .unwrap_or(true)
            {
                next = Some(keyframe);
            }
        }

        let Some(previous) = previous else {
            return Some(first.value);
        };
        let Some(next) = next else {
            return Some(last.value);
        };
        if (next.time - previous.time).abs() <= f64::EPSILON {
            return Some(next.value);
        }

        let progress = ((time - previous.time) / (next.time - previous.time)).clamp(0.0, 1.0);
        Some(T::interpolate(
            previous.value,
            next.value,
            previous.easing.sample(progress as f32),
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clip<T = AnimationValue> {
    pub id: String,
    pub start_time: f64,
    pub duration: f64,
    pub tracks: Vec<Track<T>>,
    pub enabled: bool,
}

impl<T> Clip<T> {
    pub fn new(id: impl Into<String>, start_time: f64, duration: f64) -> Self {
        Self {
            id: id.into(),
            start_time: start_time.max(0.0),
            duration: duration.max(0.0),
            tracks: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_track(mut self, track: Track<T>) -> Self {
        self.tracks.push(track);
        self
    }

    pub fn push_track(&mut self, track: Track<T>) {
        self.tracks.push(track);
    }

    pub fn end_time(&self) -> f64 {
        self.start_time + self.duration
    }

    pub fn contains_time(&self, time: f64) -> bool {
        self.enabled && time >= self.start_time && time <= self.end_time()
    }
}

impl<T> Clip<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Vec<SampledAnimationValue<T>> {
        if !self.contains_time(time) {
            return Vec::new();
        }

        let local_time = time - self.start_time;
        self.tracks
            .iter()
            .filter_map(|track| {
                track.sample(local_time).map(|value| SampledAnimationValue {
                    clip_id: self.id.clone(),
                    binding: track.binding.clone(),
                    time,
                    value,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timeline<T = AnimationValue> {
    pub duration: f64,
    pub clips: Vec<Clip<T>>,
}

impl<T> Timeline<T> {
    pub fn new(duration: f64) -> Self {
        Self {
            duration: duration.max(0.0),
            clips: Vec::new(),
        }
    }

    pub fn with_clip(mut self, clip: Clip<T>) -> Self {
        self.clips.push(clip);
        self
    }

    pub fn push_clip(&mut self, clip: Clip<T>) {
        self.clips.push(clip);
    }
}

impl<T> Default for Timeline<T> {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl<T> Timeline<T>
where
    T: Copy + Interpolate,
{
    pub fn sample(&self, time: f64) -> Vec<SampledAnimationValue<T>> {
        let clamped_time = time.clamp(0.0, self.duration.max(0.0));
        self.clips
            .iter()
            .flat_map(|clip| clip.sample(clamped_time))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampledAnimationValue<T = AnimationValue> {
    pub clip_id: String,
    pub binding: AnimationBinding,
    pub time: f64,
    pub value: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Once,
    Repeat,
}

impl Default for LoopMode {
    fn default() -> Self {
        Self::Once
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackState {
    pub playhead: f64,
    pub playback_rate: f64,
    pub playing: bool,
    pub loop_mode: LoopMode,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playhead: 0.0,
            playback_rate: 1.0,
            playing: false,
            loop_mode: LoopMode::Once,
        }
    }
}

impl PlaybackState {
    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.playhead = 0.0;
    }

    pub fn seek(&mut self, time: f64, duration: f64) {
        self.playhead = time.clamp(0.0, duration.max(0.0));
    }

    pub fn tick(&mut self, delta_seconds: f64, duration: f64) -> bool {
        if !self.playing {
            return false;
        }

        let previous_time = self.playhead;
        let duration = duration.max(0.0);
        if duration <= f64::EPSILON {
            self.playhead = 0.0;
            self.playing = false;
            return previous_time != self.playhead;
        }

        self.playhead += delta_seconds.max(0.0) * self.playback_rate;
        if self.playhead > duration {
            match self.loop_mode {
                LoopMode::Once => {
                    self.playhead = duration;
                    self.playing = false;
                }
                LoopMode::Repeat => {
                    self.playhead = self.playhead.rem_euclid(duration);
                }
            }
        } else if self.playhead < 0.0 {
            match self.loop_mode {
                LoopMode::Once => {
                    self.playhead = 0.0;
                    self.playing = false;
                }
                LoopMode::Repeat => {
                    self.playhead = self.playhead.rem_euclid(duration);
                }
            }
        }

        previous_time != self.playhead
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationDocument {
    pub version: u32,
    pub name: String,
    pub timeline: Timeline,
}

impl AnimationDocument {
    pub fn new(name: impl Into<String>, timeline: Timeline) -> Self {
        Self {
            version: ANIMATION_DOCUMENT_VERSION,
            name: name.into(),
            timeline,
        }
    }

    pub fn to_document_format(&self) -> String {
        let mut output = String::new();
        output.push_str("sui-animation-document\t");
        output.push_str(&self.version.to_string());
        output.push('\n');
        output.push_str("name\t");
        output.push_str(&escape_document_field(&self.name));
        output.push('\n');
        output.push_str("duration\t");
        output.push_str(&format_f64(self.timeline.duration));
        output.push('\n');

        for clip in &self.timeline.clips {
            output.push_str("clip\t");
            output.push_str(&escape_document_field(&clip.id));
            output.push('\t');
            output.push_str(&format_f64(clip.start_time));
            output.push('\t');
            output.push_str(&format_f64(clip.duration));
            output.push('\t');
            output.push_str(format_bool(clip.enabled));
            output.push('\n');

            for track in &clip.tracks {
                output.push_str("track\t");
                output.push_str(&escape_document_field(track.binding.target.as_str()));
                output.push('\t');
                output.push_str(&escape_document_field(track.binding.property.path()));
                output.push('\t');
                output.push_str(format_bool(track.enabled));
                output.push('\n');

                for keyframe in &track.keyframes {
                    output.push_str("key\t");
                    output.push_str(&format_f64(keyframe.time));
                    output.push('\t');
                    output.push_str(&format_easing(keyframe.easing));
                    output.push('\t');
                    output.push_str(&format_animation_value(keyframe.value));
                    output.push('\n');
                }

                output.push_str("endtrack\n");
            }

            output.push_str("endclip\n");
        }

        output
    }

    pub fn from_document_format(input: &str) -> Result<Self, AnimationDocumentFormatError> {
        AnimationDocumentFormatParser::new(input).parse()
    }
}

impl Default for AnimationDocument {
    fn default() -> Self {
        Self::new("Untitled animation", Timeline::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationDocumentFormatError {
    pub line: Option<usize>,
    pub message: String,
}

impl AnimationDocumentFormatError {
    fn new(line: Option<usize>, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for AnimationDocumentFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(
                f,
                "animation document format error on line {line}: {}",
                self.message
            ),
            None => write!(f, "animation document format error: {}", self.message),
        }
    }
}

impl std::error::Error for AnimationDocumentFormatError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineSnap {
    pub enabled: bool,
    pub interval: f64,
}

impl TimelineSnap {
    pub fn new(interval: f64) -> Self {
        Self {
            enabled: true,
            interval: interval.max(f64::EPSILON),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            interval: 1.0 / 60.0,
        }
    }

    pub fn snap_time(self, time: f64) -> f64 {
        if !self.enabled {
            return time.max(0.0);
        }
        ((time.max(0.0) / self.interval).round() * self.interval).max(0.0)
    }
}

impl Default for TimelineSnap {
    fn default() -> Self {
        Self::new(1.0 / 24.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyframeSelection {
    pub clip_index: usize,
    pub track_index: usize,
    pub keyframe_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnimationSelection {
    pub clip_index: Option<usize>,
    pub track_index: Option<usize>,
    pub keyframes: Vec<KeyframeSelection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnimationEditorCommand {
    SetPlayhead(f64),
    SetZoom(f32),
    SetScroll(f32),
    SetSnapping(TimelineSnap),
    ClearSelection,
    SelectClip(usize),
    SelectTrack {
        clip_index: usize,
        track_index: usize,
    },
    SelectKeyframe(KeyframeSelection),
    AddKeyframe {
        clip_index: usize,
        track_index: usize,
        keyframe: Keyframe<AnimationValue>,
    },
    UpdateKeyframeEasing {
        selection: KeyframeSelection,
        easing: Easing,
    },
    RemoveKeyframe(KeyframeSelection),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEditorState {
    pub document: AnimationDocument,
    pub playback: PlaybackState,
    pub selection: AnimationSelection,
    pub zoom: f32,
    pub scroll: f32,
    pub snap: TimelineSnap,
    undo_stack: Vec<AnimationDocument>,
    redo_stack: Vec<AnimationDocument>,
}

impl AnimationEditorState {
    pub fn new(document: AnimationDocument) -> Self {
        Self {
            document,
            playback: PlaybackState::default(),
            selection: AnimationSelection::default(),
            zoom: 1.0,
            scroll: 0.0,
            snap: TimelineSnap::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn apply_command(&mut self, command: AnimationEditorCommand) -> bool {
        match command {
            AnimationEditorCommand::SetPlayhead(time) => {
                self.playback.seek(time, self.document.timeline.duration);
                true
            }
            AnimationEditorCommand::SetZoom(zoom) => {
                self.zoom = zoom.max(0.05);
                true
            }
            AnimationEditorCommand::SetScroll(scroll) => {
                self.scroll = scroll.max(0.0);
                true
            }
            AnimationEditorCommand::SetSnapping(snap) => {
                self.snap = snap;
                true
            }
            AnimationEditorCommand::ClearSelection => {
                self.selection = AnimationSelection::default();
                true
            }
            AnimationEditorCommand::SelectClip(clip_index) => {
                self.selection.clip_index = Some(clip_index);
                self.selection.track_index = None;
                self.selection.keyframes.clear();
                true
            }
            AnimationEditorCommand::SelectTrack {
                clip_index,
                track_index,
            } => {
                self.selection.clip_index = Some(clip_index);
                self.selection.track_index = Some(track_index);
                self.selection.keyframes.clear();
                true
            }
            AnimationEditorCommand::SelectKeyframe(selection) => {
                self.selection.clip_index = Some(selection.clip_index);
                self.selection.track_index = Some(selection.track_index);
                if !self.selection.keyframes.contains(&selection) {
                    self.selection.keyframes.push(selection);
                }
                true
            }
            AnimationEditorCommand::AddKeyframe {
                clip_index,
                track_index,
                mut keyframe,
            } => {
                let Some(track) = self
                    .document
                    .timeline
                    .clips
                    .get(clip_index)
                    .and_then(|clip| clip.tracks.get(track_index))
                else {
                    return false;
                };
                let mut updated_track = track.clone();
                keyframe.time = self.snap.snap_time(keyframe.time);
                updated_track.push_keyframe(keyframe);

                self.push_undo_snapshot();
                self.document.timeline.clips[clip_index].tracks[track_index] = updated_track;
                self.redo_stack.clear();
                true
            }
            AnimationEditorCommand::UpdateKeyframeEasing { selection, easing } => {
                let Some(track) = self
                    .document
                    .timeline
                    .clips
                    .get(selection.clip_index)
                    .and_then(|clip| clip.tracks.get(selection.track_index))
                else {
                    return false;
                };
                let Some(keyframe) = track.keyframes.get(selection.keyframe_index) else {
                    return false;
                };

                let mut updated_track = track.clone();
                let mut updated_keyframe = *keyframe;
                updated_keyframe.easing = easing;
                updated_track.keyframes[selection.keyframe_index] = updated_keyframe;

                self.push_undo_snapshot();
                self.document.timeline.clips[selection.clip_index].tracks[selection.track_index] =
                    updated_track;
                self.redo_stack.clear();
                true
            }
            AnimationEditorCommand::RemoveKeyframe(selection) => {
                let Some(track) = self
                    .document
                    .timeline
                    .clips
                    .get(selection.clip_index)
                    .and_then(|clip| clip.tracks.get(selection.track_index))
                else {
                    return false;
                };
                if selection.keyframe_index >= track.keyframes.len() {
                    return false;
                }

                let mut updated_track = track.clone();
                updated_track.keyframes.remove(selection.keyframe_index);

                self.push_undo_snapshot();
                self.document.timeline.clips[selection.clip_index].tracks[selection.track_index] =
                    updated_track;
                self.selection
                    .keyframes
                    .retain(|selected| *selected != selection);
                self.redo_stack.clear();
                true
            }
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.document.clone());
        self.document = previous;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.document.clone());
        self.document = next;
        true
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(self.document.clone());
    }
}

impl Default for AnimationEditorState {
    fn default() -> Self {
        Self::new(AnimationDocument::default())
    }
}

struct AnimationDocumentFormatParser<'a> {
    input: &'a str,
}

impl<'a> AnimationDocumentFormatParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input }
    }

    fn parse(self) -> Result<AnimationDocument, AnimationDocumentFormatError> {
        let mut version = None;
        let mut name = None;
        let mut duration = None;
        let mut clips = Vec::new();
        let mut current_clip: Option<Clip> = None;
        let mut current_track: Option<Track> = None;

        for (line_index, raw_line) in self.input.lines().enumerate() {
            let line_no = line_index + 1;
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }

            let fields = line.split('\t').collect::<Vec<_>>();
            match fields.first().copied().unwrap_or_default() {
                "sui-animation-document" => {
                    expect_field_count(line_no, &fields, 2)?;
                    if version.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "duplicate document header",
                        ));
                    }
                    let parsed_version = parse_u32_field(line_no, fields[1], "version")?;
                    if parsed_version != ANIMATION_DOCUMENT_VERSION {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            format!(
                                "unsupported document version {parsed_version}; expected {ANIMATION_DOCUMENT_VERSION}"
                            ),
                        ));
                    }
                    version = Some(parsed_version);
                }
                "name" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 2)?;
                    ensure_no_open_track_or_clip(line_no, &current_track, &current_clip)?;
                    name = Some(unescape_document_field(fields[1], line_no)?);
                }
                "duration" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 2)?;
                    ensure_no_open_track_or_clip(line_no, &current_track, &current_clip)?;
                    duration = Some(parse_f64_field(line_no, fields[1], "duration")?);
                }
                "clip" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 5)?;
                    if current_clip.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "nested clips are not allowed",
                        ));
                    }
                    let id = unescape_document_field(fields[1], line_no)?;
                    let start_time = parse_f64_field(line_no, fields[2], "clip start time")?;
                    let clip_duration = parse_f64_field(line_no, fields[3], "clip duration")?;
                    let enabled = parse_bool_field(line_no, fields[4], "clip enabled")?;
                    let mut clip = Clip::new(id, start_time, clip_duration);
                    clip.enabled = enabled;
                    current_clip = Some(clip);
                }
                "track" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 4)?;
                    if current_clip.is_none() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "track must appear inside a clip",
                        ));
                    }
                    if current_track.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "nested tracks are not allowed",
                        ));
                    }
                    let target =
                        AnimationTargetId::new(unescape_document_field(fields[1], line_no)?);
                    let property_path = unescape_document_field(fields[2], line_no)?;
                    let enabled = parse_bool_field(line_no, fields[3], "track enabled")?;
                    let mut track = Track::new(AnimationBinding::new(
                        target,
                        animation_property_from_path(property_path),
                    ));
                    track.enabled = enabled;
                    current_track = Some(track);
                }
                "key" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 4)?;
                    let Some(track) = &mut current_track else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "keyframe must appear inside a track",
                        ));
                    };
                    let time = parse_f64_field(line_no, fields[1], "keyframe time")?;
                    let easing = parse_easing(line_no, fields[2])?;
                    let value = parse_animation_value(line_no, fields[3])?;
                    track.push_keyframe(Keyframe::new(time, value).with_easing(easing));
                }
                "endtrack" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 1)?;
                    let Some(track) = current_track.take() else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endtrack without an open track",
                        ));
                    };
                    let Some(clip) = &mut current_clip else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endtrack without an open clip",
                        ));
                    };
                    clip.push_track(track);
                }
                "endclip" => {
                    ensure_document_header(line_no, version)?;
                    expect_field_count(line_no, &fields, 1)?;
                    if current_track.is_some() {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endclip reached before endtrack",
                        ));
                    }
                    let Some(clip) = current_clip.take() else {
                        return Err(AnimationDocumentFormatError::new(
                            Some(line_no),
                            "endclip without an open clip",
                        ));
                    };
                    clips.push(clip);
                }
                other => {
                    return Err(AnimationDocumentFormatError::new(
                        Some(line_no),
                        format!("unknown directive {other:?}"),
                    ));
                }
            }
        }

        if version.is_none() {
            return Err(AnimationDocumentFormatError::new(
                None,
                "missing document header",
            ));
        }
        if current_track.is_some() {
            return Err(AnimationDocumentFormatError::new(None, "unclosed track"));
        }
        if current_clip.is_some() {
            return Err(AnimationDocumentFormatError::new(None, "unclosed clip"));
        }

        Ok(AnimationDocument {
            version: ANIMATION_DOCUMENT_VERSION,
            name: name
                .ok_or_else(|| AnimationDocumentFormatError::new(None, "missing document name"))?,
            timeline: Timeline {
                duration: duration.ok_or_else(|| {
                    AnimationDocumentFormatError::new(None, "missing timeline duration")
                })?,
                clips,
            },
        })
    }
}

fn format_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn format_f64(value: f64) -> String {
    value.to_string()
}

fn format_f32(value: f32) -> String {
    value.to_string()
}

fn escape_document_field(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn unescape_document_field(
    value: &str,
    line_no: usize,
) -> Result<String, AnimationDocumentFormatError> {
    let mut unescaped = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            unescaped.push(ch);
            continue;
        }

        match chars.next() {
            Some('\\') => unescaped.push('\\'),
            Some('n') => unescaped.push('\n'),
            Some('r') => unescaped.push('\r'),
            Some('t') => unescaped.push('\t'),
            Some(other) => {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    format!("unsupported escape sequence \\{other}"),
                ));
            }
            None => {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    "unterminated escape sequence",
                ));
            }
        }
    }
    Ok(unescaped)
}

fn expect_field_count(
    line_no: usize,
    fields: &[&str],
    expected: usize,
) -> Result<(), AnimationDocumentFormatError> {
    if fields.len() == expected {
        return Ok(());
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        format!("expected {expected} fields, found {}", fields.len()),
    ))
}

fn ensure_document_header(
    line_no: usize,
    version: Option<u32>,
) -> Result<(), AnimationDocumentFormatError> {
    if version.is_some() {
        return Ok(());
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        "document header must be the first directive",
    ))
}

fn ensure_no_open_track_or_clip(
    line_no: usize,
    track: &Option<Track>,
    clip: &Option<Clip>,
) -> Result<(), AnimationDocumentFormatError> {
    if track.is_none() && clip.is_none() {
        return Ok(());
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        "document metadata must appear before clips",
    ))
}

fn parse_u32_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<u32, AnimationDocumentFormatError> {
    value.parse::<u32>().map_err(|_| {
        AnimationDocumentFormatError::new(Some(line_no), format!("invalid {label}: {value:?}"))
    })
}

fn parse_f64_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<f64, AnimationDocumentFormatError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        AnimationDocumentFormatError::new(Some(line_no), format!("invalid {label}: {value:?}"))
    })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("{label} must be finite"),
        ))
    }
}

fn parse_f32_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<f32, AnimationDocumentFormatError> {
    let parsed = value.parse::<f32>().map_err(|_| {
        AnimationDocumentFormatError::new(Some(line_no), format!("invalid {label}: {value:?}"))
    })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("{label} must be finite"),
        ))
    }
}

fn parse_bool_field(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<bool, AnimationDocumentFormatError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("{label} must be true or false"),
        )),
    }
}

fn format_easing(easing: Easing) -> String {
    match easing {
        Easing::Linear => "linear".to_string(),
        Easing::EaseIn => "ease-in".to_string(),
        Easing::EaseOut => "ease-out".to_string(),
        Easing::EaseInOut => "ease-in-out".to_string(),
        Easing::CubicBezier { x1, y1, x2, y2 } => format!(
            "cubic-bezier:{},{},{},{}",
            format_f32(x1),
            format_f32(y1),
            format_f32(x2),
            format_f32(y2)
        ),
    }
}

fn parse_easing(line_no: usize, value: &str) -> Result<Easing, AnimationDocumentFormatError> {
    match value {
        "linear" => Ok(Easing::Linear),
        "ease-in" => Ok(Easing::EaseIn),
        "ease-out" => Ok(Easing::EaseOut),
        "ease-in-out" => Ok(Easing::EaseInOut),
        _ => {
            let Some(params) = value.strip_prefix("cubic-bezier:") else {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    format!("unknown easing {value:?}"),
                ));
            };
            let values = parse_f32_list(line_no, params, "cubic-bezier")?;
            if values.len() != 4 {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    "cubic-bezier easing requires four values",
                ));
            }
            Ok(Easing::CubicBezier {
                x1: values[0],
                y1: values[1],
                x2: values[2],
                y2: values[3],
            })
        }
    }
}

fn format_animation_value(value: AnimationValue) -> String {
    match value {
        AnimationValue::Scalar(value) => format!("scalar:{}", format_f32(value)),
        AnimationValue::Point(value) => {
            format!("point:{},{}", format_f32(value.x), format_f32(value.y))
        }
        AnimationValue::Vector(value) => {
            format!("vector:{},{}", format_f32(value.x), format_f32(value.y))
        }
        AnimationValue::Size(value) => {
            format!(
                "size:{},{}",
                format_f32(value.width),
                format_f32(value.height)
            )
        }
        AnimationValue::Rect(value) => {
            format!(
                "rect:{},{},{},{}",
                format_f32(value.x()),
                format_f32(value.y()),
                format_f32(value.width()),
                format_f32(value.height())
            )
        }
        AnimationValue::Color(value) => {
            format!(
                "color:{},{},{},{},{}",
                format_color_space(value.space),
                format_f32(value.red),
                format_f32(value.green),
                format_f32(value.blue),
                format_f32(value.alpha)
            )
        }
        AnimationValue::Transform(value) => {
            format!(
                "transform:{},{},{},{},{},{}",
                format_f32(value.xx),
                format_f32(value.yx),
                format_f32(value.xy),
                format_f32(value.yy),
                format_f32(value.dx),
                format_f32(value.dy)
            )
        }
    }
}

fn parse_animation_value(
    line_no: usize,
    value: &str,
) -> Result<AnimationValue, AnimationDocumentFormatError> {
    let Some((kind, payload)) = value.split_once(':') else {
        return Err(AnimationDocumentFormatError::new(
            Some(line_no),
            "animation value must include a kind prefix",
        ));
    };

    match kind {
        "scalar" => Ok(AnimationValue::Scalar(parse_f32_field(
            line_no,
            payload,
            "scalar value",
        )?)),
        "point" => {
            let values = parse_fixed_f32_list(line_no, payload, "point", 2)?;
            Ok(AnimationValue::Point(Point::new(values[0], values[1])))
        }
        "vector" => {
            let values = parse_fixed_f32_list(line_no, payload, "vector", 2)?;
            Ok(AnimationValue::Vector(Vector::new(values[0], values[1])))
        }
        "size" => {
            let values = parse_fixed_f32_list(line_no, payload, "size", 2)?;
            Ok(AnimationValue::Size(Size::new(values[0], values[1])))
        }
        "rect" => {
            let values = parse_fixed_f32_list(line_no, payload, "rect", 4)?;
            Ok(AnimationValue::Rect(Rect::new(
                values[0], values[1], values[2], values[3],
            )))
        }
        "color" => {
            let parts = payload.split(',').collect::<Vec<_>>();
            if parts.len() != 5 {
                return Err(AnimationDocumentFormatError::new(
                    Some(line_no),
                    "color value requires color space plus four channels",
                ));
            }
            Ok(AnimationValue::Color(Color::new(
                parse_color_space(line_no, parts[0])?,
                parse_f32_field(line_no, parts[1], "red channel")?,
                parse_f32_field(line_no, parts[2], "green channel")?,
                parse_f32_field(line_no, parts[3], "blue channel")?,
                parse_f32_field(line_no, parts[4], "alpha channel")?,
            )))
        }
        "transform" => {
            let values = parse_fixed_f32_list(line_no, payload, "transform", 6)?;
            Ok(AnimationValue::Transform(Transform::new(
                values[0], values[1], values[2], values[3], values[4], values[5],
            )))
        }
        _ => Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("unknown animation value kind {kind:?}"),
        )),
    }
}

fn parse_f32_list(
    line_no: usize,
    value: &str,
    label: &str,
) -> Result<Vec<f32>, AnimationDocumentFormatError> {
    value
        .split(',')
        .map(|field| parse_f32_field(line_no, field, label))
        .collect()
}

fn parse_fixed_f32_list(
    line_no: usize,
    value: &str,
    label: &str,
    expected: usize,
) -> Result<Vec<f32>, AnimationDocumentFormatError> {
    let values = parse_f32_list(line_no, value, label)?;
    if values.len() == expected {
        return Ok(values);
    }

    Err(AnimationDocumentFormatError::new(
        Some(line_no),
        format!("{label} value requires {expected} numbers"),
    ))
}

fn format_color_space(color_space: ColorSpace) -> &'static str {
    match color_space {
        ColorSpace::Srgb => "srgb",
        ColorSpace::LinearSrgb => "linear-srgb",
        ColorSpace::DisplayP3 => "display-p3",
        ColorSpace::LinearDisplayP3 => "linear-display-p3",
    }
}

fn parse_color_space(
    line_no: usize,
    value: &str,
) -> Result<ColorSpace, AnimationDocumentFormatError> {
    match value {
        "srgb" => Ok(ColorSpace::Srgb),
        "linear-srgb" => Ok(ColorSpace::LinearSrgb),
        "display-p3" => Ok(ColorSpace::DisplayP3),
        "linear-display-p3" => Ok(ColorSpace::LinearDisplayP3),
        _ => Err(AnimationDocumentFormatError::new(
            Some(line_no),
            format!("unknown color space {value:?}"),
        )),
    }
}

fn animation_property_from_path(path: String) -> AnimationProperty {
    match path.as_str() {
        "layer.opacity" => AnimationProperty::LayerOpacity,
        "layer.translation" => AnimationProperty::LayerTranslation,
        "fill.color" => AnimationProperty::FillColor,
        "bounds" => AnimationProperty::Bounds,
        _ => AnimationProperty::Custom(AnimationPropertyPath::new(path)),
    }
}

fn sample_cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    let sample_curve = |a: f32, b: f32, c: f32, u: f32| {
        let inv = 1.0 - u;
        (3.0 * inv * inv * u * a) + (3.0 * inv * u * u * b) + (u * u * u * c)
    };

    let mut low = 0.0;
    let mut high = 1.0;
    let mut u = t;
    for _ in 0..10 {
        u = (low + high) * 0.5;
        let x = sample_curve(x1, x2, 1.0, u);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            low = u;
        } else {
            high = u;
        }
    }

    sample_curve(y1, y2, 1.0, u)
}

#[cfg(test)]
mod tests {
    use super::{
        AnimatedValue, AnimationBinding, AnimationDocument, AnimationEditorCommand,
        AnimationEditorState, AnimationProperty, AnimationPropertyPath, AnimationTargetId,
        AnimationValue, Blink, Clip, Easing, Interpolate, Keyframe, KeyframeSelection, LoopMode,
        PlaybackState, Pulse, SpringF32, Timeline, Track, Transition,
    };
    use sui_core::{Color, ColorSpace, Rect, Transform, Vector};

    fn opacity_binding() -> AnimationBinding {
        AnimationBinding::new(
            AnimationTargetId::new("hero-card"),
            AnimationProperty::LayerOpacity,
        )
    }

    #[test]
    fn interpolate_supports_common_sui_values() {
        assert!((f32::interpolate(2.0, 6.0, 0.25) - 3.0).abs() < 1e-6);
        assert_eq!(
            Vector::interpolate(Vector::new(0.0, 4.0), Vector::new(8.0, 12.0), 0.5),
            Vector::new(4.0, 8.0)
        );
        assert_eq!(
            Rect::interpolate(
                Rect::new(0.0, 2.0, 10.0, 20.0),
                Rect::new(10.0, 12.0, 30.0, 40.0),
                0.5
            ),
            Rect::new(5.0, 7.0, 20.0, 30.0)
        );
        let interpolated = Color::interpolate(
            Color::rgba(0.2, 0.4, 0.6, 1.0),
            Color::rgba(0.6, 0.8, 1.0, 0.0),
            0.5,
        );
        let expected = Color::rgba(0.4, 0.6, 0.8, 0.5);
        assert_eq!(interpolated.space, expected.space);
        assert!((interpolated.red - expected.red).abs() < 1e-6);
        assert!((interpolated.green - expected.green).abs() < 1e-6);
        assert!((interpolated.blue - expected.blue).abs() < 1e-6);
        assert!((interpolated.alpha - expected.alpha).abs() < 1e-6);
    }

    #[test]
    fn transition_samples_ease_in_out_curve() {
        let transition = Transition::new(0.0_f32, 1.0, 10.0, 2.0, Easing::EaseInOut);

        assert_eq!(transition.sample(10.0), 0.0);
        assert!(transition.sample(11.0) > 0.45 && transition.sample(11.0) < 0.55);
        assert_eq!(transition.sample(12.0), 1.0);
        assert!(transition.is_complete(12.0));
    }

    #[test]
    fn cubic_bezier_easing_has_stable_endpoints() {
        let easing = Easing::CubicBezier {
            x1: 0.4,
            y1: 0.0,
            x2: 0.2,
            y2: 1.0,
        };

        assert_eq!(easing.sample(0.0), 0.0);
        assert_eq!(easing.sample(1.0), 1.0);
        let midpoint = easing.sample(0.5);
        assert!(midpoint > 0.0 && midpoint < 1.0);
    }

    #[test]
    fn track_samples_keyframes_without_requiring_sorted_input() {
        let track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(1.0, 1.0_f32),
            Keyframe::new(0.0, 0.0_f32).with_easing(Easing::Linear),
        ]);

        assert_eq!(track.sample(-1.0), Some(0.0));
        assert_eq!(track.sample(0.5), Some(0.5));
        assert_eq!(track.sample(2.0), Some(1.0));
    }

    #[test]
    fn timeline_samples_clip_tracks_in_global_time() {
        let opacity_track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(0.0, AnimationValue::Scalar(0.25)),
            Keyframe::new(2.0, AnimationValue::Scalar(1.0)),
        ]);
        let timeline =
            Timeline::new(4.0).with_clip(Clip::new("intro", 1.0, 2.0).with_track(opacity_track));

        assert!(timeline.sample(0.5).is_empty());
        let samples = timeline.sample(2.0);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].clip_id, "intro");
        let opacity = samples[0]
            .value
            .as_scalar()
            .expect("opacity sample should be scalar");
        assert!((opacity - 0.625).abs() < 1e-6);
    }

    #[test]
    fn playback_state_advances_and_repeats() {
        let mut playback = PlaybackState {
            loop_mode: LoopMode::Repeat,
            ..PlaybackState::default()
        };
        playback.play();

        assert!(playback.tick(1.25, 1.0));
        assert!((playback.playhead - 0.25).abs() < 1e-6);
        assert!(playback.playing);
    }

    #[test]
    fn blink_is_deterministic_for_same_time_inputs() {
        let blink = Blink::new(0.8).with_duty_cycle(0.25).with_phase(0.1);

        assert_eq!(blink.is_on(1.25), blink.is_on(1.25));
        assert!(blink.is_on(0.0));
        assert!(!blink.is_on(0.35));
    }

    #[test]
    fn pulse_samples_repeatable_range() {
        let pulse = Pulse::new(1.0, 0.2, 1.0);
        let a = pulse.sample(0.25);
        let b = pulse.sample(0.25);
        assert_eq!(a, b);
        assert!(a >= 0.2 && a <= 1.0);
    }

    #[test]
    fn spring_helpers_converge_toward_target_values() {
        let mut spring = SpringF32::new(0.0).with_config(140.0, 22.0);
        let mut value = 0.0;
        for _ in 0..120 {
            value = spring.step(1.0, 1.0 / 120.0);
        }

        assert!(value > 0.95);
        assert!((value - 1.0).abs() < 0.05);
    }

    #[test]
    fn animated_value_reaches_target_and_reports_completion() {
        let mut animated = AnimatedValue::new(0.0_f32)
            .with_duration(0.2)
            .with_easing(Easing::Linear);
        animated.set_target(1.0);
        assert!(animated.is_animating());

        assert!(animated.tick(0.1));
        assert!((animated.value() - 0.5).abs() < 1e-4);

        assert!(!animated.tick(0.1));
        assert_eq!(animated.value(), 1.0);
        assert!(!animated.is_animating());
        assert!(!animated.tick(0.1));
    }

    #[test]
    fn animated_value_with_zero_duration_snaps_immediately() {
        let mut animated = AnimatedValue::new(2.0_f32).with_duration(0.0);
        animated.set_target(9.0);

        assert!(!animated.is_animating());
        assert_eq!(animated.value(), 9.0);
        assert!(!animated.tick(1.0));
    }

    #[test]
    fn editor_state_adds_keyframes_and_undoes_document_changes() {
        let track = Track::new(opacity_binding());
        let document = AnimationDocument::new(
            "Editor test",
            Timeline::new(2.0).with_clip(Clip::new("intro", 0.0, 2.0).with_track(track)),
        );
        let mut editor = AnimationEditorState::new(document);

        assert!(editor.apply_command(AnimationEditorCommand::AddKeyframe {
            clip_index: 0,
            track_index: 0,
            keyframe: Keyframe::new(0.51, AnimationValue::Scalar(0.8)),
        }));
        assert_eq!(editor.undo_len(), 1);
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes[0].time,
            0.5
        );

        assert!(editor.undo());
        assert!(
            editor.document.timeline.clips[0].tracks[0]
                .keyframes
                .is_empty()
        );
        assert!(editor.redo());
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes.len(),
            1
        );
    }

    #[test]
    fn editor_selection_tracks_keyframes_without_mutating_document() {
        let mut editor = AnimationEditorState::default();
        let selection = KeyframeSelection {
            clip_index: 2,
            track_index: 1,
            keyframe_index: 3,
        };

        assert!(editor.apply_command(AnimationEditorCommand::SelectKeyframe(selection)));
        assert_eq!(editor.selection.keyframes, vec![selection]);
        assert_eq!(editor.undo_len(), 0);
    }

    #[test]
    fn editor_updates_keyframe_easing_with_undo() {
        let track = Track::new(opacity_binding()).with_keyframes([
            Keyframe::new(0.0, AnimationValue::Scalar(0.0)),
            Keyframe::new(1.0, AnimationValue::Scalar(1.0)),
        ]);
        let document = AnimationDocument::new(
            "Easing test",
            Timeline::new(1.0).with_clip(Clip::new("intro", 0.0, 1.0).with_track(track)),
        );
        let mut editor = AnimationEditorState::new(document);
        let selection = KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 0,
        };

        assert!(
            editor.apply_command(AnimationEditorCommand::UpdateKeyframeEasing {
                selection,
                easing: Easing::EaseInOut,
            })
        );
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes[0].easing,
            Easing::EaseInOut
        );
        assert!(editor.undo());
        assert_eq!(
            editor.document.timeline.clips[0].tracks[0].keyframes[0].easing,
            Easing::Linear
        );
    }

    #[test]
    fn animation_document_format_round_trips_timeline_data() {
        let target = AnimationTargetId::new("preview card\tmain");
        let binding = |property| AnimationBinding::new(target.clone(), property);
        let mut fill_track = Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
            Keyframe::new(
                0.0,
                AnimationValue::Color(Color::new(ColorSpace::DisplayP3, 0.1, 0.2, 0.3, 0.4)),
            )
            .with_easing(Easing::CubicBezier {
                x1: 0.4,
                y1: 0.0,
                x2: 0.2,
                y2: 1.0,
            }),
            Keyframe::new(
                1.0,
                AnimationValue::Color(Color::new(ColorSpace::LinearDisplayP3, 0.7, 0.6, 0.5, 1.0)),
            ),
        ]);
        fill_track.enabled = false;
        let mut clip = Clip::new("intro\nclip", 0.25, 2.0)
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-12.0, 4.5)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.0, AnimationValue::Vector(Vector::new(22.0, -8.0))),
                ]),
            )
            .with_track(fill_track)
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new("paint.radius"),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(6.0)).with_easing(Easing::EaseOut),
                    Keyframe::new(1.0, AnimationValue::Scalar(18.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new("local.transform"),
                )))
                .with_keyframes([Keyframe::new(
                    0.5,
                    AnimationValue::Transform(Transform::translation(8.0, 9.0)),
                )]),
            );
        clip.enabled = true;
        let document = AnimationDocument::new(
            "Demo\tAnimation\nDocument",
            Timeline::new(3.5).with_clip(clip),
        );

        let serialized = document.to_document_format();
        assert!(serialized.starts_with("sui-animation-document\t1\n"));
        assert!(serialized.contains("paint.radius"));

        let parsed = AnimationDocument::from_document_format(&serialized)
            .expect("serialized document should parse");
        assert_eq!(parsed, document);
        assert_eq!(parsed.to_document_format(), serialized);
    }

    #[test]
    fn animation_document_format_rejects_unsupported_versions() {
        let err = AnimationDocument::from_document_format(
            "sui-animation-document\t99\nname\tBad\nduration\t1\n",
        )
        .expect_err("unsupported version should fail");

        assert_eq!(err.line, Some(1));
        assert!(err.message.contains("unsupported document version"));
    }

    #[test]
    fn animation_document_format_rejects_unclosed_tracks() {
        let err = AnimationDocument::from_document_format(
            "sui-animation-document\t1\nname\tBad\nduration\t1\nclip\tintro\t0\t1\ttrue\ntrack\ttarget\tlayer.opacity\ttrue\n",
        )
        .expect_err("unclosed track should fail");

        assert!(err.message.contains("unclosed track"));
    }
}
