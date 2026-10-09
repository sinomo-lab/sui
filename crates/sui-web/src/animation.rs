use crate::prelude::*;

pub(crate) fn js_loop_mode(value: &str) -> Result<sui_crate::LoopMode> {
    binding_loop_mode_from_name(value).ok_or_else(|| {
        invalid_arg(format!(
            "unknown loop mode '{value}'; expected once, repeat, or ping-pong"
        ))
    })
}

pub(crate) fn js_stagger_origin(value: &str) -> Result<sui_crate::StaggerOrigin> {
    binding_stagger_origin_from_name(value).ok_or_else(|| {
        invalid_arg(format!(
            "unknown stagger origin '{value}'; expected first, last, center, or an item index"
        ))
    })
}

pub(crate) fn js_easing(value: Option<&str>) -> Result<sui_crate::Easing> {
    let value = value.unwrap_or("ease-in-out");
    binding_easing_from_name(value).ok_or_else(|| {
        invalid_arg(format!(
            "unknown easing '{value}'; expected linear, ease-in, ease-out, or ease-in-out"
        ))
    })
}

#[wasm_bindgen(js_name = AnimationValue)]
#[derive(Debug, Clone, Copy)]
pub struct JsAnimationValue {
    pub(crate) inner: BindingAnimationValue,
}

js_class!(JsAnimationValue => "AnimationValue");

#[wasm_bindgen(js_class = AnimationValue)]
impl JsAnimationValue {
    #[wasm_bindgen]
    pub fn scalar(value: f64) -> Self {
        Self {
            inner: BindingAnimationValue::scalar(value as f32),
        }
    }

    #[wasm_bindgen]
    pub fn point(value: &JsPoint) -> Self {
        Self {
            inner: BindingAnimationValue::point((*value).into()),
        }
    }

    #[wasm_bindgen]
    pub fn vector(value: &JsPoint) -> Self {
        Self {
            inner: BindingAnimationValue::vector((*value).into()),
        }
    }

    #[wasm_bindgen]
    pub fn size(value: &JsSize) -> Self {
        Self {
            inner: BindingAnimationValue::size((*value).into()),
        }
    }

    #[wasm_bindgen]
    pub fn rect(value: &JsRect) -> Self {
        Self {
            inner: BindingAnimationValue::rect((*value).into()),
        }
    }

    #[wasm_bindgen]
    pub fn color(value: &JsColor) -> Self {
        Self {
            inner: BindingAnimationValue::color((*value).into()),
        }
    }

    #[wasm_bindgen]
    pub fn transform(value: &JsTransform) -> Self {
        Self {
            inner: BindingAnimationValue::transform((*value).into()),
        }
    }

    #[wasm_bindgen(getter = kind)]
    pub fn kind(&self) -> String {
        self.inner.kind().to_owned()
    }

    #[wasm_bindgen(getter = scalarValue)]
    pub fn scalar_value(&self) -> Option<f64> {
        self.inner.as_scalar().map(f64::from)
    }

    #[wasm_bindgen(getter = pointValue)]
    pub fn point_value(&self) -> Option<JsPoint> {
        self.inner.as_point().map(Into::into)
    }

    #[wasm_bindgen(getter = vectorValue)]
    pub fn vector_value(&self) -> Option<JsPoint> {
        self.inner
            .as_vector()
            .map(|value| JsPoint::new(f64::from(value.x), f64::from(value.y)))
    }

    #[wasm_bindgen(getter = sizeValue)]
    pub fn size_value(&self) -> Option<JsSize> {
        self.inner.as_size().map(Into::into)
    }

    #[wasm_bindgen(getter = rectValue)]
    pub fn rect_value(&self) -> Option<JsRect> {
        self.inner.as_rect().map(Into::into)
    }

    #[wasm_bindgen(getter = colorValue)]
    pub fn color_value(&self) -> Option<JsColor> {
        self.inner.as_color().map(Into::into)
    }

    #[wasm_bindgen(getter = transformValue)]
    pub fn transform_value(&self) -> Option<JsTransform> {
        self.inner.as_transform().map(Into::into)
    }
}

impl From<BindingAnimationValue> for JsAnimationValue {
    fn from(inner: BindingAnimationValue) -> Self {
        Self { inner }
    }
}

js_object! {
    pub struct JsTransitionOptions {
        start_time: Option<f64> = "startTime",
        easing: Option<String> = "easing",
    }
}

#[wasm_bindgen(js_name = Transition)]
#[derive(Debug, Clone, Copy)]
pub struct JsTransition {
    pub(crate) inner: BindingTransition,
}

js_class!(JsTransition => "Transition");

#[wasm_bindgen(js_class = Transition)]
impl JsTransition {
    #[wasm_bindgen(constructor)]
    pub fn new(
        start: &JsAnimationValue,
        end: &JsAnimationValue,
        duration: f64,
        options: JsValue,
    ) -> Result<Self> {
        let options = <Option<JsTransitionOptions> as FromJs>::from_js(&options)?;
        let start_time = options
            .as_ref()
            .and_then(|options| options.start_time)
            .unwrap_or(0.0);
        let easing = js_easing(
            options
                .as_ref()
                .and_then(|options| options.easing.as_deref()),
        )?;
        Ok(Self {
            inner: BindingTransition::new(start.inner, end.inner, start_time, duration, easing),
        })
    }

    #[wasm_bindgen]
    pub fn progress(&self, time: f64) -> f64 {
        f64::from(self.inner.progress(time))
    }

    #[wasm_bindgen]
    pub fn sample(&self, time: f64) -> JsAnimationValue {
        self.inner.sample(time).into()
    }

    #[wasm_bindgen(js_name = isComplete)]
    pub fn is_complete(&self, time: f64) -> bool {
        self.inner.is_complete(time)
    }
}

js_object! {
    pub struct JsSpringOptions {
        stiffness: Option<f64> = "stiffness",
        damping: Option<f64> = "damping",
    }
}

#[wasm_bindgen(js_name = Spring)]
#[derive(Debug, Clone, Copy)]
pub struct JsSpring {
    pub(crate) inner: BindingSpring,
}

js_class!(JsSpring => "Spring");

#[wasm_bindgen(js_class = Spring)]
impl JsSpring {
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64, options: JsValue) -> Result<Self> {
        let options = <Option<JsSpringOptions> as FromJs>::from_js(&options)?;
        Ok(Self {
            inner: BindingSpring::new(
                value as f32,
                options
                    .as_ref()
                    .and_then(|options| options.stiffness)
                    .unwrap_or(180.0) as f32,
                options
                    .as_ref()
                    .and_then(|options| options.damping)
                    .unwrap_or(24.0) as f32,
            ),
        })
    }

    #[wasm_bindgen]
    pub fn step(&mut self, target: f64, delta_seconds: f64) -> f64 {
        f64::from(self.inner.step(target as f32, delta_seconds))
    }

    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> f64 {
        f64::from(self.inner.value())
    }
    #[wasm_bindgen(getter = velocity)]
    pub fn velocity(&self) -> f64 {
        f64::from(self.inner.velocity())
    }
    #[wasm_bindgen(getter = stiffness)]
    pub fn stiffness(&self) -> f64 {
        f64::from(self.inner.stiffness())
    }
    #[wasm_bindgen(getter = damping)]
    pub fn damping(&self) -> f64 {
        f64::from(self.inner.damping())
    }
}

js_object! {
    pub struct JsAnimatedValueOptions {
        duration: Option<f64> = "duration",
        easing: Option<String> = "easing",
    }
}

#[wasm_bindgen(js_name = AnimatedValue)]
#[derive(Debug, Clone, Copy)]
pub struct JsAnimatedValue {
    pub(crate) inner: BindingAnimatedValue,
}

js_class!(JsAnimatedValue => "AnimatedValue");

#[wasm_bindgen(js_class = AnimatedValue)]
impl JsAnimatedValue {
    #[wasm_bindgen(constructor)]
    pub fn new(initial: &JsAnimationValue, options: JsValue) -> Result<Self> {
        let options = <Option<JsAnimatedValueOptions> as FromJs>::from_js(&options)?;
        Ok(Self {
            inner: BindingAnimatedValue::new(
                initial.inner,
                options
                    .as_ref()
                    .and_then(|options| options.duration)
                    .unwrap_or(0.2) as f32,
                js_easing(
                    options
                        .as_ref()
                        .and_then(|options| options.easing.as_deref()),
                )?,
            ),
        })
    }

    #[wasm_bindgen(js_name = setDuration)]
    pub fn set_duration(&mut self, seconds: f64) {
        self.inner.set_duration(seconds as f32);
    }

    #[wasm_bindgen(js_name = setEasing)]
    pub fn set_easing(&mut self, easing: String) -> Result<()> {
        self.inner.set_easing(js_easing(Some(&easing))?);
        Ok(())
    }

    /// Start toward `target` after `delay` seconds of ticks.
    #[wasm_bindgen(js_name = setTargetAfter)]
    pub fn set_target_after(&mut self, target: &JsAnimationValue, delay: f64) {
        self.inner.set_target_after(target.inner, delay);
    }

    #[wasm_bindgen(js_name = setTarget)]
    pub fn set_target(&mut self, target: &JsAnimationValue) {
        self.inner.set_target(target.inner);
    }

    #[wasm_bindgen(js_name = jumpTo)]
    pub fn jump_to(&mut self, value: &JsAnimationValue) {
        self.inner.jump_to(value.inner);
    }

    #[wasm_bindgen]
    pub fn tick(&mut self, delta_seconds: f64) -> bool {
        self.inner.tick(delta_seconds as f32)
    }

    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> JsAnimationValue {
        self.inner.value().into()
    }
    #[wasm_bindgen(getter = target)]
    pub fn target(&self) -> JsAnimationValue {
        self.inner.target().into()
    }
    #[wasm_bindgen(getter = isAnimating)]
    pub fn is_animating(&self) -> bool {
        self.inner.is_animating()
    }
}

js_object! {
    pub struct JsKeyframeOptions {
        easing: Option<String> = "easing",
    }
}

#[wasm_bindgen(js_name = Keyframe)]
#[derive(Debug, Clone, Copy)]
pub struct JsAnimationKeyframe {
    pub(crate) inner: BindingAnimationKeyframe,
}

js_class!(JsAnimationKeyframe => "Keyframe");

#[wasm_bindgen(js_class = Keyframe)]
impl JsAnimationKeyframe {
    #[wasm_bindgen(constructor)]
    pub fn new(time: f64, value: &JsAnimationValue, options: JsValue) -> Result<Self> {
        let options = <Option<JsKeyframeOptions> as FromJs>::from_js(&options)?;
        Ok(Self {
            inner: BindingAnimationKeyframe::new(
                time,
                value.inner,
                js_easing(
                    options
                        .as_ref()
                        .and_then(|options| options.easing.as_deref())
                        .or(Some("linear")),
                )?,
            ),
        })
    }

    #[wasm_bindgen(getter = time)]
    pub fn time(&self) -> f64 {
        self.inner.time()
    }
    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> JsAnimationValue {
        self.inner.value().into()
    }
}

#[wasm_bindgen(js_name = AnimationTrack)]
#[derive(Debug, Clone)]
pub struct JsAnimationTrack {
    pub(crate) inner: BindingAnimationTrack,
}

js_class!(JsAnimationTrack => "AnimationTrack");

#[wasm_bindgen(js_class = AnimationTrack)]
impl JsAnimationTrack {
    #[wasm_bindgen(constructor)]
    pub fn new(target: String, property: String) -> Self {
        Self {
            inner: BindingAnimationTrack::new(target, property),
        }
    }
    #[wasm_bindgen(js_name = addKeyframe)]
    pub fn add_keyframe(&mut self, keyframe: &JsAnimationKeyframe) {
        self.inner.add_keyframe(keyframe.inner);
    }
    #[wasm_bindgen(js_name = setEnabled)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }
    #[wasm_bindgen]
    pub fn sample(&self, time: f64) -> Option<JsAnimationValue> {
        self.inner.sample(time).map(Into::into)
    }
    #[wasm_bindgen(getter = target)]
    pub fn target(&self) -> String {
        self.inner.target().to_owned()
    }
    #[wasm_bindgen(getter = property)]
    pub fn property(&self) -> String {
        self.inner.property().to_owned()
    }
    #[wasm_bindgen(getter = keyframeCount)]
    pub fn keyframe_count(&self) -> u32 {
        self.inner.keyframe_count() as u32
    }
}

#[wasm_bindgen(js_name = AnimationClip)]
#[derive(Debug, Clone)]
pub struct JsAnimationClip {
    pub(crate) inner: BindingAnimationClip,
}

js_class!(JsAnimationClip => "AnimationClip");

#[wasm_bindgen(js_class = AnimationClip)]
impl JsAnimationClip {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, start_time: f64, duration: f64) -> Self {
        Self {
            inner: BindingAnimationClip::new(id, start_time, duration),
        }
    }
    #[wasm_bindgen(js_name = addTrack)]
    pub fn add_track(&mut self, track: &JsAnimationTrack) {
        self.inner.add_track(track.inner.clone());
    }
    #[wasm_bindgen(js_name = setEnabled)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }
    #[wasm_bindgen(getter = id)]
    pub fn id(&self) -> String {
        self.inner.id().to_owned()
    }
    #[wasm_bindgen(getter = startTime)]
    pub fn start_time(&self) -> f64 {
        self.inner.start_time()
    }
    #[wasm_bindgen(getter = duration)]
    pub fn duration(&self) -> f64 {
        self.inner.duration()
    }
    #[wasm_bindgen(getter = trackCount)]
    pub fn track_count(&self) -> u32 {
        self.inner.track_count() as u32
    }
}

#[wasm_bindgen(js_name = AnimationSample)]
#[derive(Debug, Clone)]
pub struct JsAnimationSample {
    #[wasm_bindgen(readonly, getter_with_clone, js_name = clipId)]
    pub clip_id: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub target: String,
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub property: String,
    #[wasm_bindgen(readonly)]
    pub time: f64,
    pub(crate) value: JsAnimationValue,
}

js_class!(JsAnimationSample => "AnimationSample");

#[wasm_bindgen(js_class = AnimationSample)]
impl JsAnimationSample {
    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> JsAnimationValue {
        self.value
    }
}

impl From<BindingAnimationSample> for JsAnimationSample {
    fn from(value: BindingAnimationSample) -> Self {
        Self {
            clip_id: value.clip_id,
            target: value.target,
            property: value.property,
            time: value.time,
            value: value.value.into(),
        }
    }
}

#[wasm_bindgen(js_name = AnimationMarker)]
#[derive(Debug, Clone)]
pub struct JsAnimationMarker {
    #[wasm_bindgen(readonly, getter_with_clone)]
    pub name: String,
    #[wasm_bindgen(readonly)]
    pub time: f64,
}

js_class!(JsAnimationMarker => "AnimationMarker");

impl From<BindingAnimationMarker> for JsAnimationMarker {
    fn from(value: BindingAnimationMarker) -> Self {
        Self {
            name: value.name,
            time: value.time,
        }
    }
}

js_object! {
    pub struct JsStaggerOptions {
        origin: Option<String> = "origin",
        max_delay: Option<f64> = "maxDelay",
    }
}

#[wasm_bindgen(js_name = Stagger)]
#[derive(Debug, Clone, Copy)]
pub struct JsStagger {
    pub(crate) inner: BindingStagger,
}

js_class!(JsStagger => "Stagger");

#[wasm_bindgen(js_class = Stagger)]
impl JsStagger {
    #[wasm_bindgen(constructor)]
    pub fn new(interval: f64, options: JsValue) -> Result<Self> {
        let options = <Option<JsStaggerOptions> as FromJs>::from_js(&options)?;
        let mut inner = BindingStagger::new(interval);
        if let Some(options) = options {
            if let Some(origin) = options.origin {
                inner.set_origin(js_stagger_origin(&origin)?);
            }
            inner.set_max_delay(options.max_delay);
        }
        Ok(Self { inner })
    }
    /// The delay in seconds for item `index` of `count`.
    #[wasm_bindgen]
    pub fn delay(&self, index: u32, count: u32) -> f64 {
        self.inner.delay(index as usize, count as usize)
    }
    #[wasm_bindgen(getter = interval)]
    pub fn interval(&self) -> f64 {
        self.inner.interval()
    }
}

#[wasm_bindgen(js_name = AnimationTimeline)]
#[derive(Debug, Clone)]
pub struct JsAnimationTimeline {
    pub(crate) inner: BindingAnimationTimeline,
}

js_class!(JsAnimationTimeline => "AnimationTimeline");

#[wasm_bindgen(js_class = AnimationTimeline)]
impl JsAnimationTimeline {
    #[wasm_bindgen(constructor)]
    pub fn new(duration: f64) -> Self {
        Self {
            inner: BindingAnimationTimeline::new(duration),
        }
    }
    #[wasm_bindgen(js_name = addClip)]
    pub fn add_clip(&mut self, clip: &JsAnimationClip) {
        self.inner.add_clip(clip.inner.clone());
    }
    #[wasm_bindgen(js_name = addMarker)]
    pub fn add_marker(&mut self, name: String, time: f64) {
        self.inner.add_marker(name, time);
    }
    #[wasm_bindgen(getter = markers)]
    pub fn markers(&self) -> Vec<JsAnimationMarker> {
        self.inner.markers().into_iter().map(Into::into).collect()
    }
    #[wasm_bindgen]
    pub fn sample(&self, time: f64) -> Vec<JsAnimationSample> {
        self.inner
            .sample(time)
            .into_iter()
            .map(Into::into)
            .collect()
    }
    #[wasm_bindgen(getter = duration)]
    pub fn duration(&self) -> f64 {
        self.inner.duration()
    }
    #[wasm_bindgen(getter = clipCount)]
    pub fn clip_count(&self) -> u32 {
        self.inner.clip_count() as u32
    }
}

#[wasm_bindgen(js_name = AnimationPlayer)]
#[derive(Debug, Clone)]
pub struct JsAnimationPlayer {
    pub(crate) inner: BindingAnimationPlayer,
}

js_class!(JsAnimationPlayer => "AnimationPlayer");

#[wasm_bindgen(js_class = AnimationPlayer)]
impl JsAnimationPlayer {
    #[wasm_bindgen(constructor)]
    pub fn new(timeline: &JsAnimationTimeline) -> Self {
        Self {
            inner: BindingAnimationPlayer::new(&timeline.inner),
        }
    }
    #[wasm_bindgen]
    pub fn play(&mut self) {
        self.inner.play();
    }
    #[wasm_bindgen]
    pub fn pause(&mut self) {
        self.inner.pause();
    }
    #[wasm_bindgen]
    pub fn stop(&mut self) {
        self.inner.stop();
    }
    #[wasm_bindgen]
    pub fn seek(&mut self, time: f64) {
        self.inner.seek(time);
    }
    #[wasm_bindgen(js_name = setRepeat)]
    pub fn set_repeat(&mut self, repeat: bool) {
        self.inner.set_repeat(repeat);
    }
    /// Play `"once"`, `"repeat"` from the start, or `"ping-pong"` back and
    /// forth.
    #[wasm_bindgen(js_name = setLoopMode)]
    pub fn set_loop_mode(&mut self, mode: String) -> Result<()> {
        self.inner.set_loop_mode(js_loop_mode(&mode)?);
        Ok(())
    }
    #[wasm_bindgen(getter = loopMode)]
    pub fn loop_mode(&self) -> String {
        binding_loop_mode_name(self.inner.loop_mode()).to_owned()
    }
    #[wasm_bindgen(js_name = setStartDelay)]
    pub fn set_start_delay(&mut self, seconds: f64) {
        self.inner.set_start_delay(seconds);
    }
    #[wasm_bindgen(js_name = setLoopDelay)]
    pub fn set_loop_delay(&mut self, seconds: f64) {
        self.inner.set_loop_delay(seconds);
    }
    /// The markers the playhead passed in the last tick, in order.
    #[wasm_bindgen(getter = passedMarkers)]
    pub fn passed_markers(&self) -> Vec<JsAnimationMarker> {
        self.inner
            .passed_markers()
            .into_iter()
            .map(Into::into)
            .collect()
    }
    #[wasm_bindgen(js_name = setPlaybackRate)]
    pub fn set_playback_rate(&mut self, rate: f64) {
        self.inner.set_playback_rate(rate);
    }
    #[wasm_bindgen]
    pub fn sample(&self) -> Vec<JsAnimationSample> {
        self.inner.sample().into_iter().map(Into::into).collect()
    }
    #[wasm_bindgen]
    pub fn tick(&mut self, delta_seconds: f64) -> Vec<JsAnimationSample> {
        self.inner
            .tick(delta_seconds)
            .into_iter()
            .map(Into::into)
            .collect()
    }
    #[wasm_bindgen(getter = playhead)]
    pub fn playhead(&self) -> f64 {
        self.inner.playhead()
    }
    #[wasm_bindgen(getter = isPlaying)]
    pub fn is_playing(&self) -> bool {
        self.inner.is_playing()
    }
}

#[wasm_bindgen(js_name = AnimationDocument)]
#[derive(Debug, Clone)]
pub struct JsAnimationDocument {
    pub(crate) inner: BindingAnimationDocument,
}

js_class!(JsAnimationDocument => "AnimationDocument");

#[wasm_bindgen(js_class = AnimationDocument)]
impl JsAnimationDocument {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, timeline: &JsAnimationTimeline) -> Self {
        Self {
            inner: BindingAnimationDocument::new(name, timeline.inner.clone()),
        }
    }
    #[wasm_bindgen]
    pub fn parse(input: String) -> Result<Self> {
        BindingAnimationDocument::parse(&input)
            .map(|inner| Self { inner })
            .map_err(runtime_error)
    }
    #[wasm_bindgen(getter = name)]
    pub fn name(&self) -> String {
        self.inner.name().to_owned()
    }
    #[wasm_bindgen(getter = timeline)]
    pub fn timeline(&self) -> JsAnimationTimeline {
        JsAnimationTimeline {
            inner: self.inner.timeline(),
        }
    }
    #[wasm_bindgen(js_name = toDocumentFormat)]
    pub fn to_document_format(&self) -> String {
        self.inner.to_document_format()
    }
}

#[wasm_bindgen(js_name = AnimationEditor)]
#[derive(Debug, Clone)]
pub struct JsAnimationEditor {
    pub(crate) inner: BindingAnimationEditor,
}

js_class!(JsAnimationEditor => "AnimationEditor");

#[wasm_bindgen(js_class = AnimationEditor)]
impl JsAnimationEditor {
    #[wasm_bindgen(constructor)]
    pub fn new(document: &JsAnimationDocument) -> Self {
        Self {
            inner: BindingAnimationEditor::new(document.inner.clone()),
        }
    }
    #[wasm_bindgen(getter = document)]
    pub fn document(&self) -> JsAnimationDocument {
        JsAnimationDocument {
            inner: self.inner.document(),
        }
    }
    #[wasm_bindgen(js_name = setPlayhead)]
    pub fn set_playhead(&mut self, time: f64) {
        self.inner.set_playhead(time);
    }
    #[wasm_bindgen(js_name = setZoom)]
    pub fn set_zoom(&mut self, zoom: f64) {
        self.inner.set_zoom(zoom as f32);
    }
    #[wasm_bindgen(js_name = setScroll)]
    pub fn set_scroll(&mut self, scroll: f64) {
        self.inner.set_scroll(scroll as f32);
    }
    #[wasm_bindgen(js_name = setSnapping)]
    pub fn set_snapping(&mut self, enabled: Option<bool>, interval: Option<f64>) {
        self.inner
            .set_snapping(enabled.unwrap_or(true), interval.unwrap_or(1.0 / 24.0));
    }
    #[wasm_bindgen(js_name = addKeyframe)]
    pub fn add_keyframe(
        &mut self,
        clip_index: u32,
        track_index: u32,
        keyframe: &JsAnimationKeyframe,
    ) -> bool {
        self.inner
            .add_keyframe(clip_index as usize, track_index as usize, keyframe.inner)
    }
    #[wasm_bindgen(js_name = updateKeyframeEasing)]
    pub fn update_keyframe_easing(
        &mut self,
        clip_index: u32,
        track_index: u32,
        keyframe_index: u32,
        easing: String,
    ) -> Result<bool> {
        Ok(self.inner.update_keyframe_easing(
            clip_index as usize,
            track_index as usize,
            keyframe_index as usize,
            js_easing(Some(&easing))?,
        ))
    }
    #[wasm_bindgen(js_name = removeKeyframe)]
    pub fn remove_keyframe(
        &mut self,
        clip_index: u32,
        track_index: u32,
        keyframe_index: u32,
    ) -> bool {
        self.inner.remove_keyframe(
            clip_index as usize,
            track_index as usize,
            keyframe_index as usize,
        )
    }
    #[wasm_bindgen(js_name = addMarker)]
    pub fn add_marker(&mut self, name: String, time: f64) -> bool {
        self.inner.add_marker(name, time)
    }
    #[wasm_bindgen(js_name = moveMarker)]
    pub fn move_marker(&mut self, index: u32, time: f64) -> bool {
        self.inner.move_marker(index as usize, time)
    }
    #[wasm_bindgen(js_name = renameMarker)]
    pub fn rename_marker(&mut self, index: u32, name: String) -> bool {
        self.inner.rename_marker(index as usize, name)
    }
    #[wasm_bindgen(js_name = removeMarker)]
    pub fn remove_marker(&mut self, index: u32) -> bool {
        self.inner.remove_marker(index as usize)
    }
    #[wasm_bindgen]
    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }
    #[wasm_bindgen]
    pub fn redo(&mut self) -> bool {
        self.inner.redo()
    }
    #[wasm_bindgen(getter = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }
    #[wasm_bindgen(getter = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }
    #[wasm_bindgen(getter = playhead)]
    pub fn playhead(&self) -> f64 {
        self.inner.playhead()
    }
    #[wasm_bindgen(getter = zoom)]
    pub fn zoom(&self) -> f64 {
        f64::from(self.inner.zoom())
    }
    #[wasm_bindgen(getter = scroll)]
    pub fn scroll(&self) -> f64 {
        f64::from(self.inner.scroll())
    }
}
