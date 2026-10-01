//! Timeline studio: an editor for an `AnimationDocument`. Keyframes drag
//! along their tracks, the ruler scrubs the playhead, and the selected
//! keyframe's easing is a curve with draggable handles. Every edit goes
//! through `AnimationEditorState`, so it undoes, and the document copies out
//! as text.

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{
    AnimationBinding, AnimationDocument, AnimationEditorCommand, AnimationEditorState,
    AnimationProperty, AnimationPropertyPath, AnimationTargetId, AnimationValue, Clip, KeyState,
    Keyframe, KeyframeSelection, LoopMode, PointerEventKind, SemanticsNode, SemanticsRole,
    SemanticsValue, Timeline, TimelineBindingSink, TimelinePlayer, TimelineSnap, Track, Vector,
    motion_policy,
};

use super::curves::easing_label;
use super::{draw_text, hairline, paint_card, refresh_page, stroke_polyline};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style_when};

pub(crate) const STUDIO_EDITOR_NAME: &str = "Timeline editor";
pub(crate) const STUDIO_PLAY_LABEL: &str = "Play";
pub(crate) const STUDIO_PAUSE_LABEL: &str = "Pause";
pub(crate) const STUDIO_RESTART_LABEL: &str = "Restart";
pub(crate) const STUDIO_COPY_LABEL: &str = "Copy document";
pub(crate) const STUDIO_ADD_LABEL: &str = "Add keyframe";
pub(crate) const STUDIO_REMOVE_LABEL: &str = "Remove keyframe";
pub(crate) const STUDIO_LOOP_NAME: &str = "Playback loop";
pub(crate) const STUDIO_MARKER_LABEL: &str = "Add marker";
const STUDIO_HOLD_NAME: &str = "Hold at ends";
const STUDIO_EASING_NAME: &str = "Keyframe easing";
const STUDIO_SNAP_NAME: &str = "Timeline snapping";

const TARGET: &str = "studio-preview";
const RADIUS_PATH: &str = "paint.radius";
const DURATION: f64 = 2.0;
/// How far the preview travels either side of the middle.
const TRAVEL: f32 = 90.0;
const EDITOR_HEIGHT: f32 = 372.0;
const LABEL_COLUMN: f32 = 132.0;
const LANE_HEIGHT: f32 = 34.0;
const LANE_GAP: f32 = 6.0;
const RULER_HEIGHT: f32 = 28.0;
/// Pointer travel before a press on a keyframe becomes a drag.
const DRAG_SLOP: f32 = 3.0;
/// Snapping interval when snapping is on.
const SNAP_INTERVAL: f64 = 1.0 / 24.0;
/// Nudge step for arrow keys when snapping is off.
const FREE_NUDGE: f64 = 0.05;
/// Vertical range of the curve editor, room for handles past the ends.
const CURVE_MIN: f32 = -0.35;
const CURVE_MAX: f32 = 1.35;

/// Loop modes in the loop control, in segment order.
const LOOP_CHOICES: [(LoopMode, &str); 3] = [
    (LoopMode::Once, "Once"),
    (LoopMode::Repeat, "Repeat"),
    (LoopMode::PingPong, "Ping-pong"),
];

/// How long playback holds at each end before looping, in menu order.
const HOLD_CHOICES: [(f64, &str); 3] =
    [(0.0, "No hold"), (0.25, "Hold 0.25 s"), (0.5, "Hold 0.5 s")];

/// Easing choices in the easing menu, followed by "Custom" for any other
/// curve.
const EASING_PRESETS: [(&str, Easing); 6] = [
    ("Linear", Easing::Linear),
    ("Ease in", Easing::EaseIn),
    ("Ease out", Easing::EaseOut),
    ("Ease in-out", Easing::EaseInOut),
    (
        "Standard",
        Easing::CubicBezier {
            x1: 0.2,
            y1: 0.0,
            x2: 0.0,
            y2: 1.0,
        },
    ),
    (
        "Emphasized",
        Easing::CubicBezier {
            x1: 0.45,
            y1: 0.0,
            x2: 0.15,
            y2: 1.0,
        },
    ),
];
const CUSTOM_EASING: &str = "Custom";

/// What the timeline animates: a card sliding, fading, recoloring,
/// rounding, and growing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Preview {
    pub(super) opacity: f32,
    pub(super) offset: f32,
    pub(super) fill: Color,
    pub(super) radius: f32,
    pub(super) size: Size,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            offset: 0.0,
            fill: Color::rgba(0.20, 0.45, 0.95, 1.0),
            radius: 8.0,
            size: Size::new(64.0, 40.0),
        }
    }
}

impl TimelineBindingSink for Preview {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != TARGET {
            return false;
        }
        let before = *self;
        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(opacity)) => {
                self.opacity = opacity.clamp(0.0, 1.0);
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(offset)) => {
                self.offset = offset.x;
            }
            (AnimationProperty::FillColor, AnimationValue::Color(fill)) => self.fill = fill,
            (AnimationProperty::Bounds, AnimationValue::Rect(bounds)) => self.size = bounds.size,
            (AnimationProperty::Custom(path), AnimationValue::Scalar(radius))
                if path.as_str() == RADIUS_PATH =>
            {
                self.radius = radius.max(0.0);
            }
            _ => {}
        }
        *self != before
    }
}

fn studio_timeline() -> Timeline {
    let target = AnimationTargetId::new(TARGET);
    let track = |property| Track::new(AnimationBinding::new(target.clone(), property));
    let blue = AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0));
    let green = AnimationValue::Color(Color::rgba(0.10, 0.72, 0.50, 1.0));
    let standard = EASING_PRESETS[4].1;
    let emphasized = EASING_PRESETS[5].1;
    Timeline::new(DURATION)
        .with_marker("Arrive", 1.0)
        .with_marker("Home", DURATION)
        .with_clip(
            Clip::new("studio", 0.0, DURATION)
                .with_track(
                    track(AnimationProperty::LayerTranslation).with_keyframes([
                        Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-TRAVEL, 0.0)))
                            .with_easing(emphasized),
                        Keyframe::new(1.0, AnimationValue::Vector(Vector::new(TRAVEL, 0.0)))
                            .with_easing(emphasized),
                        Keyframe::new(2.0, AnimationValue::Vector(Vector::new(-TRAVEL, 0.0))),
                    ]),
                )
                .with_track(track(AnimationProperty::LayerOpacity).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.45)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.0, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(2.0, AnimationValue::Scalar(0.45)),
                ]))
                .with_track(track(AnimationProperty::FillColor).with_keyframes([
                    Keyframe::new(0.0, blue).with_easing(standard),
                    Keyframe::new(1.0, green).with_easing(standard),
                    Keyframe::new(2.0, blue),
                ]))
                .with_track(
                    track(AnimationProperty::Custom(AnimationPropertyPath::new(
                        RADIUS_PATH,
                    )))
                    .with_keyframes([
                        Keyframe::new(0.0, AnimationValue::Scalar(8.0)).with_easing(standard),
                        Keyframe::new(1.0, AnimationValue::Scalar(24.0)).with_easing(standard),
                        Keyframe::new(2.0, AnimationValue::Scalar(8.0)),
                    ]),
                )
                .with_track(
                    track(AnimationProperty::Bounds).with_keyframes([
                        Keyframe::new(0.0, AnimationValue::Rect(Rect::new(0.0, 0.0, 64.0, 40.0)))
                            .with_easing(Easing::EaseInOut),
                        Keyframe::new(1.0, AnimationValue::Rect(Rect::new(0.0, 0.0, 104.0, 56.0)))
                            .with_easing(Easing::EaseInOut),
                        Keyframe::new(2.0, AnimationValue::Rect(Rect::new(0.0, 0.0, 64.0, 40.0))),
                    ]),
                ),
        )
}

/// The control points of `easing` as a cubic bezier, so any curve can be
/// edited with handles. Quadratic ease in and out convert exactly; ease
/// in-out is approximated.
pub(super) fn bezier_points(easing: Easing) -> [f32; 4] {
    match easing {
        Easing::Linear => [1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0],
        Easing::EaseIn => [1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0 / 3.0],
        Easing::EaseOut => [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 1.0],
        Easing::EaseInOut => [0.45, 0.0, 0.55, 1.0],
        Easing::CubicBezier { x1, y1, x2, y2 } => [x1, y1, x2, y2],
    }
}

fn bezier_easing([x1, y1, x2, y2]: [f32; 4]) -> Easing {
    Easing::CubicBezier { x1, y1, x2, y2 }
}

#[derive(Clone)]
pub(super) struct StudioState {
    inner: Rc<RefCell<StudioInner>>,
}

struct StudioInner {
    editor: AnimationEditorState,
    player: TimelinePlayer,
    preview: Preview,
    status: String,
    /// The marker the playhead passed last.
    last_marker: Option<String>,
}

impl StudioInner {
    /// Rebuild the player from the edited document, keeping playback.
    fn sync_player(&mut self) {
        let playback = self.player.playback();
        self.player
            .set_timeline(self.editor.document.timeline.clone());
        *self.player.playback_mut() = playback;
        self.resample();
    }

    fn resample(&mut self) {
        let samples = self.player.sample_reusing_scratch().to_vec();
        for sample in samples {
            self.preview
                .apply_animation_value(&sample.binding, sample.value);
        }
        self.editor.playback = self.player.playback();
    }

    fn apply(&mut self, command: AnimationEditorCommand) -> bool {
        let changed = self.editor.apply_command(command);
        if changed {
            self.sync_player();
        }
        changed
    }

    fn keyframe(&self, selection: KeyframeSelection) -> Option<(String, Keyframe<AnimationValue>)> {
        let track = self
            .editor
            .document
            .timeline
            .clips
            .get(selection.clip_index)?
            .tracks
            .get(selection.track_index)?;
        let keyframe = track.keyframes.get(selection.keyframe_index)?;
        Some((track.binding.property.path().to_string(), *keyframe))
    }
}

impl StudioState {
    pub(super) fn new() -> Self {
        let timeline = studio_timeline();
        let mut player = TimelinePlayer::new(timeline.clone());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        player.play();
        let mut editor =
            AnimationEditorState::new(AnimationDocument::new("Studio preview", timeline));
        editor.apply_command(AnimationEditorCommand::SelectKeyframe(KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 1,
        }));
        let mut inner = StudioInner {
            editor,
            player,
            preview: Preview::default(),
            status: String::new(),
            last_marker: None,
        };
        inner.resample();
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        self.inner.borrow().player.playback().playing
    }

    pub(super) fn playhead(&self) -> f64 {
        self.inner.borrow().player.playback().playhead
    }

    pub(super) fn preview(&self) -> Preview {
        self.inner.borrow().preview
    }

    pub(super) fn editor(&self) -> AnimationEditorState {
        self.inner.borrow().editor.clone()
    }

    pub(super) fn play(&self) {
        let mut inner = self.inner.borrow_mut();
        let playback = inner.player.playback();
        if playback.loop_mode == LoopMode::Once && playback.playhead >= DURATION {
            inner.player.seek(0.0);
        }
        inner.player.play();
        inner.resample();
    }

    pub(super) fn pause(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.player.pause();
        inner.resample();
    }

    pub(super) fn toggle_playing(&self) {
        if self.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    pub(super) fn restart(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.player.seek(0.0);
        inner.player.play();
        inner.resample();
    }

    pub(super) fn loop_mode(&self) -> LoopMode {
        self.inner.borrow().player.playback().loop_mode
    }

    pub(super) fn set_loop_mode(&self, loop_mode: LoopMode) {
        let mut inner = self.inner.borrow_mut();
        inner.player.playback_mut().loop_mode = loop_mode;
        inner.resample();
    }

    pub(super) fn loop_delay(&self) -> f64 {
        self.inner.borrow().player.playback().loop_delay
    }

    pub(super) fn set_loop_delay(&self, delay: f64) {
        let mut inner = self.inner.borrow_mut();
        inner.player.playback_mut().loop_delay = delay;
        inner.resample();
    }

    /// The marker the playhead passed last.
    #[cfg(test)]
    pub(super) fn last_marker(&self) -> Option<String> {
        self.inner.borrow().last_marker.clone()
    }

    /// Add a marker at the playhead.
    pub(super) fn add_marker(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        let time = inner.player.playback().playhead;
        let name = format!(
            "Marker {}",
            inner.editor.document.timeline.markers.len() + 1
        );
        inner.apply(AnimationEditorCommand::AddMarker { name, time })
    }

    /// The timeline's markers, as edited.
    pub(super) fn markers(&self) -> Vec<(String, f64)> {
        self.inner
            .borrow()
            .editor
            .document
            .timeline
            .markers
            .iter()
            .map(|marker| (marker.name.clone(), marker.time))
            .collect()
    }

    pub(super) fn seek(&self, time: f64) {
        let mut inner = self.inner.borrow_mut();
        inner.player.seek(time);
        inner.resample();
    }

    /// Advance playback by `delta` seconds. Returns whether it is still
    /// playing.
    pub(super) fn advance(&self, delta: f64) -> bool {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        if !inner.player.playback().playing {
            return false;
        }
        let tick = inner.player.tick(delta, &mut inner.preview);
        let playing = tick.should_continue;
        if let Some(marker) = tick.passed_markers.last() {
            inner.last_marker = Some(marker.name.clone());
        }
        inner.editor.playback = inner.player.playback();
        playing
    }

    pub(super) fn selected(&self) -> Option<KeyframeSelection> {
        self.inner
            .borrow()
            .editor
            .selection
            .keyframes
            .last()
            .copied()
    }

    pub(super) fn select(&self, selection: KeyframeSelection) {
        let mut inner = self.inner.borrow_mut();
        inner
            .editor
            .apply_command(AnimationEditorCommand::ClearSelection);
        inner
            .editor
            .apply_command(AnimationEditorCommand::SelectKeyframe(selection));
    }

    pub(super) fn select_track(&self, track_index: usize) {
        self.inner
            .borrow_mut()
            .editor
            .apply_command(AnimationEditorCommand::SelectTrack {
                clip_index: 0,
                track_index,
            });
    }

    pub(super) fn move_keyframe(&self, selection: KeyframeSelection, time: f64) -> bool {
        self.inner
            .borrow_mut()
            .apply(AnimationEditorCommand::MoveKeyframe { selection, time })
    }

    /// Move the selected keyframe one step earlier or later.
    pub(super) fn nudge_selected(&self, direction: f64) -> bool {
        let Some(selection) = self.selected() else {
            return false;
        };
        let step = if self.snapping() {
            SNAP_INTERVAL
        } else {
            FREE_NUDGE
        };
        let Some(time) = self.selected_keyframe().map(|(_, keyframe)| keyframe.time) else {
            return false;
        };
        self.move_keyframe(selection, time + step * direction)
    }

    /// The selected keyframe's track path and keyframe.
    pub(super) fn selected_keyframe(&self) -> Option<(String, Keyframe<AnimationValue>)> {
        let inner = self.inner.borrow();
        inner
            .editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| inner.keyframe(*selection))
    }

    pub(super) fn set_selected_easing(&self, easing: Easing) -> bool {
        let Some(selection) = self.selected() else {
            return false;
        };
        self.inner
            .borrow_mut()
            .apply(AnimationEditorCommand::UpdateKeyframeEasing { selection, easing })
    }

    /// The easing menu entry for the selected keyframe; the entry after the
    /// presets is "Custom".
    pub(super) fn easing_choice(&self) -> Option<usize> {
        let (_, keyframe) = self.selected_keyframe()?;
        Some(
            EASING_PRESETS
                .iter()
                .position(|(_, easing)| *easing == keyframe.easing)
                .unwrap_or(EASING_PRESETS.len()),
        )
    }

    /// Add a keyframe at the playhead on the selected track, holding the
    /// track's current value.
    pub(super) fn add_keyframe(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        let track_index = inner.editor.selection.track_index.unwrap_or(0);
        let playhead = inner.player.playback().playhead;
        let Some(value) = inner
            .editor
            .document
            .timeline
            .clips
            .first()
            .and_then(|clip| clip.tracks.get(track_index))
            .and_then(|track| track.sample(playhead))
        else {
            return false;
        };
        let added = inner.apply(AnimationEditorCommand::AddKeyframe {
            clip_index: 0,
            track_index,
            keyframe: Keyframe::new(playhead, value).with_easing(Easing::EaseInOut),
        });
        if added {
            let keyframe_index = inner.editor.document.timeline.clips[0].tracks[track_index]
                .keyframes
                .len()
                - 1;
            inner
                .editor
                .apply_command(AnimationEditorCommand::ClearSelection);
            inner
                .editor
                .apply_command(AnimationEditorCommand::SelectKeyframe(KeyframeSelection {
                    clip_index: 0,
                    track_index,
                    keyframe_index,
                }));
        }
        added
    }

    pub(super) fn remove_selected(&self) -> bool {
        let Some(selection) = self.selected() else {
            return false;
        };
        self.inner
            .borrow_mut()
            .apply(AnimationEditorCommand::RemoveKeyframe(selection))
    }

    pub(super) fn undo(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        let undone = inner.editor.undo();
        if undone {
            inner
                .editor
                .apply_command(AnimationEditorCommand::ClearSelection);
            inner.sync_player();
        }
        undone
    }

    pub(super) fn redo(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        let redone = inner.editor.redo();
        if redone {
            inner
                .editor
                .apply_command(AnimationEditorCommand::ClearSelection);
            inner.sync_player();
        }
        redone
    }

    pub(super) fn can_undo(&self) -> bool {
        self.inner.borrow().editor.can_undo()
    }

    pub(super) fn can_redo(&self) -> bool {
        self.inner.borrow().editor.can_redo()
    }

    pub(super) fn snapping(&self) -> bool {
        self.inner.borrow().editor.snap.enabled
    }

    pub(super) fn set_snapping(&self, snapping: bool) {
        let snap = if snapping {
            TimelineSnap::new(SNAP_INTERVAL)
        } else {
            TimelineSnap::disabled()
        };
        self.inner
            .borrow_mut()
            .editor
            .apply_command(AnimationEditorCommand::SetSnapping(snap));
    }

    pub(super) fn snap_time(&self, time: f64) -> f64 {
        self.inner
            .borrow()
            .editor
            .snap
            .snap_time(time)
            .clamp(0.0, DURATION)
    }

    pub(super) fn document_text(&self) -> String {
        self.inner.borrow().editor.document.to_document_format()
    }

    pub(super) fn set_status(&self, status: impl Into<String>) {
        self.inner.borrow_mut().status = status.into();
    }

    pub(super) fn summary(&self) -> String {
        let inner = self.inner.borrow();
        let keyframes: usize = inner
            .editor
            .document
            .timeline
            .clips
            .iter()
            .flat_map(|clip| clip.tracks.iter())
            .map(|track| track.keyframes.len())
            .sum();
        let mut summary = format!(
            "{keyframes} keyframes · undo {} · redo {}",
            inner.editor.undo_len(),
            inner.editor.redo_len()
        );
        let markers = inner.editor.document.timeline.markers.len();
        summary.push_str(&format!(" · {markers} markers"));
        if let Some(marker) = &inner.last_marker {
            summary.push_str(&format!(" · passed {marker}"));
        }
        if !inner.status.is_empty() {
            summary.push_str(" · ");
            summary.push_str(&inner.status);
        }
        summary
    }
}

pub(super) fn section(theme_reader: DevThemeReader) -> impl Widget {
    let state = StudioState::new();
    Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch)
        .with_child(toolbar(state.clone(), Rc::clone(&theme_reader)))
        .with_child(StudioEditor::new(state, theme_reader))
}

fn toolbar(state: StudioState, theme_reader: DevThemeReader) -> impl Widget {
    let theme = || clone_dev_theme_reader(&theme_reader);
    let play = state.clone();
    let play_enabled = state.clone();
    let pause = state.clone();
    let pause_enabled = state.clone();
    let restart = state.clone();
    let loop_choice = state.clone();
    let loop_change = state.clone();
    let hold_choice = state.clone();
    let hold_change = state.clone();
    let marker = state.clone();
    let add = state.clone();
    let remove = state.clone();
    let remove_enabled = state.clone();
    let undo = state.clone();
    let undo_enabled = state.clone();
    let redo = state.clone();
    let redo_enabled = state.clone();
    let easing_choice = state.clone();
    let easing_change = state.clone();
    let snap_choice = state.clone();
    let snap_change = state.clone();
    let copy = state.clone();
    let summary = state;
    let easing_options = EASING_PRESETS
        .iter()
        .map(|(label, _)| *label)
        .chain([CUSTOM_EASING]);

    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Flex::horizontal()
                .gap(8.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Center)
                .with_child(
                    Button::new(STUDIO_PLAY_LABEL)
                        .theme_when(theme())
                        .enabled_when(move || !play_enabled.is_playing())
                        .on_press_with_ctx(move |ctx| {
                            play.play();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    Button::new(STUDIO_PAUSE_LABEL)
                        .theme_when(theme())
                        .enabled_when(move || pause_enabled.is_playing())
                        .on_press_with_ctx(move |ctx| {
                            pause.pause();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    Button::new(STUDIO_RESTART_LABEL)
                        .theme_when(theme())
                        .on_press_with_ctx(move |ctx| {
                            restart.restart();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    SegmentedControl::new(STUDIO_LOOP_NAME)
                        .segments(LOOP_CHOICES.map(|(_, label)| label))
                        .selected_when(move || {
                            LOOP_CHOICES
                                .iter()
                                .position(|(mode, _)| *mode == loop_choice.loop_mode())
                        })
                        .theme_when(theme())
                        .on_change_with_ctx(move |ctx, index, _| {
                            loop_change.set_loop_mode(LOOP_CHOICES[index].0);
                            refresh_page(ctx);
                        }),
                )
                .with_item(
                    Select::new(STUDIO_HOLD_NAME)
                        .options(HOLD_CHOICES.map(|(_, label)| label))
                        .selected_when(move || {
                            let delay = hold_choice.loop_delay();
                            HOLD_CHOICES
                                .iter()
                                .position(|(hold, _)| (hold - delay).abs() < 1e-9)
                        })
                        .theme_when(theme())
                        .on_change_with_ctx(move |ctx, index, _| {
                            hold_change.set_loop_delay(HOLD_CHOICES[index].0);
                            refresh_page(ctx);
                        }),
                    FlexItem::fixed(140.0),
                )
                .with_child(SizedBox::new().width(12.0))
                .with_child(
                    Button::new(STUDIO_ADD_LABEL)
                        .icon(IconGlyph::Add)
                        .theme_when(theme())
                        .on_press_with_ctx(move |ctx| {
                            add.add_keyframe();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    Button::new(STUDIO_MARKER_LABEL)
                        .theme_when(theme())
                        .on_press_with_ctx(move |ctx| {
                            marker.add_marker();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    Button::new(STUDIO_REMOVE_LABEL)
                        .icon(IconGlyph::Trash)
                        .theme_when(theme())
                        .enabled_when(move || remove_enabled.selected().is_some())
                        .on_press_with_ctx(move |ctx| {
                            remove.remove_selected();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::Undo, "Undo")
                        .theme_when(theme())
                        .enabled_when(move || undo_enabled.can_undo())
                        .on_press_with_ctx(move |ctx| {
                            undo.undo();
                            refresh_page(ctx);
                        }),
                )
                .with_child(
                    IconButton::new(IconGlyph::Redo, "Redo")
                        .theme_when(theme())
                        .enabled_when(move || redo_enabled.can_redo())
                        .on_press_with_ctx(move |ctx| {
                            redo.redo();
                            refresh_page(ctx);
                        }),
                )
                .with_item(
                    Select::new(STUDIO_EASING_NAME)
                        .options(easing_options)
                        .selected_when(move || easing_choice.easing_choice())
                        .theme_when(theme())
                        .on_change_with_ctx(move |ctx, index, _| {
                            if let Some((_, easing)) = EASING_PRESETS.get(index) {
                                easing_change.set_selected_easing(*easing);
                            }
                            refresh_page(ctx);
                        }),
                    FlexItem::fixed(156.0),
                )
                .with_item(
                    Select::new(STUDIO_SNAP_NAME)
                        .options(["Free", "Snap to 1/24 s"])
                        .selected_when(move || Some(usize::from(snap_choice.snapping())))
                        .theme_when(theme())
                        .on_change_with_ctx(move |ctx, index, _| {
                            snap_change.set_snapping(index == 1);
                            refresh_page(ctx);
                        }),
                    FlexItem::fixed(156.0),
                )
                .with_child(
                    Button::new(STUDIO_COPY_LABEL)
                        .icon(IconGlyph::FileText)
                        .theme_when(theme())
                        .on_press_with_ctx(move |ctx| {
                            let text = copy.document_text();
                            let lines = text.lines().count();
                            ctx.set_clipboard_text(text);
                            copy.set_status(format!("copied {lines} lines"));
                            refresh_page(ctx);
                        }),
                ),
        )
        .with_child(
            Label::new(summary.summary())
                .text_when(move || summary.summary())
                .text_style_when(demo_text_style_when(
                    &theme_reader,
                    DemoTextRole::Metadata,
                    |theme| theme.palette.text_muted,
                )),
        )
}

/// Where the editor's parts sit.
#[derive(Debug, Clone, Copy)]
struct StudioLayout {
    inner: Rect,
    ruler: Rect,
    lanes_top: f32,
    stage: Rect,
    curve: Rect,
}

impl StudioLayout {
    fn new(bounds: Rect) -> Self {
        let inner = bounds.inflate(-16.0, -16.0);
        let left_width = (inner.width() * 0.6).max(360.0).min(inner.width() - 220.0);
        let ruler = Rect::new(
            inner.x() + LABEL_COLUMN,
            inner.y(),
            (left_width - LABEL_COLUMN).max(40.0),
            RULER_HEIGHT,
        );
        let right_x = inner.x() + left_width + 20.0;
        let right_width = (inner.max_x() - right_x).max(0.0);
        let stage = Rect::new(right_x, inner.y(), right_width, 172.0);
        let curve = Rect::new(
            right_x,
            stage.max_y() + 12.0,
            right_width,
            (inner.max_y() - stage.max_y() - 12.0).max(0.0),
        );
        Self {
            inner,
            ruler,
            lanes_top: ruler.max_y() + 8.0,
            stage,
            curve,
        }
    }

    fn time_x(&self, time: f64) -> f32 {
        self.ruler.x() + self.ruler.width() * (time / DURATION).clamp(0.0, 1.0) as f32
    }

    fn time_at(&self, x: f32) -> f64 {
        f64::from(((x - self.ruler.x()) / self.ruler.width().max(1.0)).clamp(0.0, 1.0)) * DURATION
    }

    fn lane(&self, track_index: usize) -> Rect {
        Rect::new(
            self.inner.x(),
            self.lanes_top + track_index as f32 * (LANE_HEIGHT + LANE_GAP),
            self.ruler.max_x() - self.inner.x(),
            LANE_HEIGHT,
        )
    }

    fn keyframe_center(&self, track_index: usize, time: f64) -> Point {
        let lane = self.lane(track_index);
        Point::new(self.time_x(time), lane.y() + lane.height() * 0.5)
    }

    /// The curve editor's plot, mapping the unit square with room for
    /// handles above and below.
    fn curve_plot(&self) -> Rect {
        Rect::new(
            self.curve.x() + 16.0,
            self.curve.y() + 30.0,
            (self.curve.width() - 32.0).max(1.0),
            (self.curve.height() - 42.0).max(1.0),
        )
    }

    fn curve_point(&self, x: f32, y: f32) -> Point {
        let plot = self.curve_plot();
        let y = (y - CURVE_MIN) / (CURVE_MAX - CURVE_MIN);
        Point::new(
            plot.x() + plot.width() * x,
            plot.max_y() - plot.height() * y,
        )
    }

    fn curve_value(&self, point: Point) -> (f32, f32) {
        let plot = self.curve_plot();
        let x = ((point.x - plot.x()) / plot.width()).clamp(0.0, 1.0);
        let y = (plot.max_y() - point.y) / plot.height() * (CURVE_MAX - CURVE_MIN) + CURVE_MIN;
        (x, y.clamp(CURVE_MIN, CURVE_MAX))
    }
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    Scrub {
        pointer: u64,
    },
    Keyframe {
        pointer: u64,
        selection: KeyframeSelection,
        press_x: f32,
        time: f64,
        moved: bool,
    },
    Handle {
        pointer: u64,
        handle: usize,
        points: [f32; 4],
        moved: bool,
    },
}

struct StudioEditor {
    state: StudioState,
    theme_reader: DevThemeReader,
    drag: Option<Drag>,
}

impl StudioEditor {
    fn new(state: StudioState, theme_reader: DevThemeReader) -> Self {
        Self {
            state,
            theme_reader,
            drag: None,
        }
    }

    /// Keyframes with their selection keys and on-screen centers.
    fn keyframe_hits(&self, layout: &StudioLayout) -> Vec<(KeyframeSelection, Point)> {
        let editor = self.state.editor();
        let Some(clip) = editor.document.timeline.clips.first() else {
            return Vec::new();
        };
        clip.tracks
            .iter()
            .enumerate()
            .flat_map(|(track_index, track)| {
                track
                    .keyframes
                    .iter()
                    .enumerate()
                    .map(move |(keyframe_index, keyframe)| {
                        (
                            KeyframeSelection {
                                clip_index: 0,
                                track_index,
                                keyframe_index,
                            },
                            layout.keyframe_center(track_index, keyframe.time),
                        )
                    })
            })
            .collect()
    }

    /// The handles of the selected keyframe's curve, while dragging or not.
    fn handle_points(&self) -> Option<[f32; 4]> {
        if let Some(Drag::Handle { points, .. }) = self.drag {
            return Some(points);
        }
        self.state
            .selected_keyframe()
            .map(|(_, keyframe)| bezier_points(keyframe.easing))
    }

    fn pointer_down(&mut self, ctx: &mut EventCtx, pointer: &PointerEvent) {
        let layout = StudioLayout::new(ctx.bounds());
        let position = pointer.position;
        ctx.request_focus();

        if let Some(points) = self.handle_points() {
            let handles = [
                layout.curve_point(points[0], points[1]),
                layout.curve_point(points[2], points[3]),
            ];
            if let Some(handle) = handles
                .iter()
                .position(|handle| (handle.x - position.x).hypot(handle.y - position.y) <= 10.0)
            {
                self.drag = Some(Drag::Handle {
                    pointer: pointer.pointer_id,
                    handle,
                    points,
                    moved: false,
                });
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
                return;
            }
        }

        if let Some((selection, _)) = self.keyframe_hits(&layout).into_iter().find(|(_, center)| {
            (center.x - position.x).abs() <= 8.0 && (center.y - position.y).abs() <= 10.0
        }) {
            self.state.select(selection);
            let time = self
                .state
                .selected_keyframe()
                .map_or(0.0, |(_, keyframe)| keyframe.time);
            self.drag = Some(Drag::Keyframe {
                pointer: pointer.pointer_id,
                selection,
                press_x: position.x,
                time,
                moved: false,
            });
            ctx.request_pointer_capture(pointer.pointer_id);
            refresh_page(ctx);
            ctx.set_handled();
            return;
        }

        if layout.ruler.inflate(0.0, 4.0).contains(position) {
            self.state.seek(layout.time_at(position.x));
            self.drag = Some(Drag::Scrub {
                pointer: pointer.pointer_id,
            });
            ctx.request_pointer_capture(pointer.pointer_id);
            ctx.request_paint();
            ctx.request_semantics();
            ctx.set_handled();
            return;
        }

        let track = (0..5).find(|track| layout.lane(*track).contains(position));
        if let Some(track) = track {
            self.state.select_track(track);
            refresh_page(ctx);
            ctx.set_handled();
        }
    }

    fn pointer_move(&mut self, ctx: &mut EventCtx, pointer: &PointerEvent) {
        let layout = StudioLayout::new(ctx.bounds());
        let position = pointer.position;
        match self.drag {
            Some(Drag::Scrub { pointer: id }) if id == pointer.pointer_id => {
                self.state.seek(layout.time_at(position.x));
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Some(Drag::Keyframe {
                pointer: id,
                selection,
                press_x,
                moved,
                ..
            }) if id == pointer.pointer_id => {
                let moved = moved || (position.x - press_x).abs() >= DRAG_SLOP;
                self.drag = Some(Drag::Keyframe {
                    pointer: id,
                    selection,
                    press_x,
                    time: self.state.snap_time(layout.time_at(position.x)),
                    moved,
                });
                ctx.request_paint();
                ctx.set_handled();
            }
            Some(Drag::Handle {
                pointer: id,
                handle,
                mut points,
                ..
            }) if id == pointer.pointer_id => {
                let (x, y) = layout.curve_value(position);
                points[handle * 2] = x;
                points[handle * 2 + 1] = y;
                self.drag = Some(Drag::Handle {
                    pointer: id,
                    handle,
                    points,
                    moved: true,
                });
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn pointer_up(&mut self, ctx: &mut EventCtx, pointer: &PointerEvent) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        match drag {
            Drag::Keyframe {
                selection,
                time,
                moved: true,
                ..
            } => {
                self.state.move_keyframe(selection, time);
            }
            Drag::Handle {
                points,
                moved: true,
                ..
            } => {
                self.state.set_selected_easing(bezier_easing(points));
            }
            _ => {}
        }
        ctx.release_pointer_capture(pointer.pointer_id);
        refresh_page(ctx);
        ctx.set_handled();
    }

    fn key_down(&mut self, ctx: &mut EventCtx, key: &str) {
        let handled = match key {
            " " => {
                self.state.toggle_playing();
                true
            }
            "Delete" | "Backspace" => self.state.remove_selected(),
            "ArrowLeft" => self.state.nudge_selected(-1.0),
            "ArrowRight" => self.state.nudge_selected(1.0),
            _ => false,
        };
        if handled {
            refresh_page(ctx);
            ctx.set_handled();
        }
    }

    fn paint_timeline(&self, ctx: &mut PaintCtx, layout: &StudioLayout, theme: DefaultTheme) {
        let palette = theme.palette;
        let editor = self.state.editor();
        let Some(clip) = editor.document.timeline.clips.first() else {
            return;
        };

        // Ruler: a tick every tenth of a second, labels every half second.
        let ruler = layout.ruler;
        ctx.fill(Path::rounded_rect(ruler, 6.0), palette.field);
        for tenth in 0..=(DURATION * 10.0).round() as usize {
            let time = tenth as f64 / 10.0;
            let x = layout.time_x(time);
            let major = tenth % 5 == 0;
            hairline(
                ctx,
                Point::new(x, ruler.max_y() - if major { 10.0 } else { 5.0 }),
                Point::new(x, ruler.max_y()),
                palette.text_muted.with_alpha(if major { 0.8 } else { 0.4 }),
            );
            if major && tenth > 0 && time < DURATION {
                draw_text(
                    ctx,
                    theme,
                    Rect::new(x - 18.0, ruler.y() + 2.0, 36.0, 16.0),
                    &format!("{time:.1} s"),
                    DemoTextRole::Metadata,
                    palette.text_muted,
                );
            }
        }

        let selected = self.state.selected();
        let track_selected = editor.selection.track_index;
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let lane = layout.lane(track_index);
            let lane_fill = if track_selected == Some(track_index) {
                palette.accent.with_alpha(0.08)
            } else {
                palette.field
            };
            ctx.fill(Path::rounded_rect(lane, 6.0), lane_fill);
            draw_text(
                ctx,
                theme,
                Rect::new(lane.x() + 10.0, lane.y() + 8.0, LABEL_COLUMN - 16.0, 18.0),
                track.binding.property.path(),
                DemoTextRole::Metadata,
                palette.text,
            );

            // Keyframes, in time order, joined by the spans they animate.
            let mut keyframes: Vec<(usize, f64)> = track
                .keyframes
                .iter()
                .enumerate()
                .map(|(index, keyframe)| {
                    (index, self.keyframe_time(track_index, index, keyframe.time))
                })
                .collect();
            keyframes.sort_by(|a, b| a.1.total_cmp(&b.1));
            stroke_polyline(
                ctx,
                keyframes
                    .iter()
                    .map(|(_, time)| layout.keyframe_center(track_index, *time)),
                palette.accent.with_alpha(0.35),
                2.0,
            );
            for (keyframe_index, time) in keyframes {
                let center = layout.keyframe_center(track_index, time);
                let is_selected = selected
                    == Some(KeyframeSelection {
                        clip_index: 0,
                        track_index,
                        keyframe_index,
                    });
                let size = if is_selected { 8.0 } else { 6.0 };
                let mut diamond = Path::builder();
                diamond
                    .move_to(Point::new(center.x, center.y - size))
                    .line_to(Point::new(center.x + size, center.y))
                    .line_to(Point::new(center.x, center.y + size))
                    .line_to(Point::new(center.x - size, center.y))
                    .close();
                ctx.fill(
                    diamond.build(),
                    if is_selected {
                        palette.warning
                    } else {
                        palette.accent
                    },
                );
            }
        }

        // Markers: a flag on the ruler, named, with a faint line down the
        // lanes.
        for (name, time) in self.state.markers() {
            let x = layout.time_x(time);
            let bottom = layout.lane(clip.tracks.len().saturating_sub(1)).max_y();
            hairline(
                ctx,
                Point::new(x, ruler.max_y()),
                Point::new(x, bottom),
                palette.warning.with_alpha(0.45),
            );
            let mut flag = Path::builder();
            flag.move_to(Point::new(x, ruler.y() + 12.0))
                .line_to(Point::new(x + 7.0, ruler.y() + 16.0))
                .line_to(Point::new(x, ruler.y() + 20.0))
                .close();
            ctx.fill(flag.build(), palette.warning);
            draw_text(
                ctx,
                theme,
                Rect::new(x - 72.0, ruler.y() + 10.0, 68.0, 16.0),
                &name,
                DemoTextRole::Metadata,
                palette.text_muted,
            );
        }

        // Playhead across the ruler and lanes.
        let x = layout.time_x(self.state.playhead());
        let bottom = layout.lane(clip.tracks.len().saturating_sub(1)).max_y();
        hairline(
            ctx,
            Point::new(x, ruler.y()),
            Point::new(x, bottom),
            palette.accent,
        );
        ctx.fill(
            Path::rounded_rect(Rect::new(x - 5.0, ruler.y(), 10.0, 10.0), 3.0),
            palette.accent,
        );

        draw_text(
            ctx,
            theme,
            Rect::new(
                layout.inner.x(),
                layout.inner.max_y() - 18.0,
                layout.ruler.max_x() - layout.inner.x(),
                18.0,
            ),
            "Space plays or pauses · Delete removes · ←/→ nudge the selected keyframe",
            DemoTextRole::Metadata,
            palette.text_muted,
        );
    }

    /// A keyframe's time as drawn: the drag position while it is dragged.
    fn keyframe_time(&self, track_index: usize, keyframe_index: usize, time: f64) -> f64 {
        match self.drag {
            Some(Drag::Keyframe {
                selection,
                time: dragged,
                moved: true,
                ..
            }) if selection.track_index == track_index
                && selection.keyframe_index == keyframe_index =>
            {
                dragged
            }
            _ => time,
        }
    }

    fn paint_stage(&self, ctx: &mut PaintCtx, layout: &StudioLayout, theme: DefaultTheme) {
        let palette = theme.palette;
        let stage = layout.stage;
        ctx.fill(Path::rounded_rect(stage, 8.0), palette.field);
        hairline(
            ctx,
            Point::new(stage.x() + stage.width() * 0.5, stage.y() + 28.0),
            Point::new(stage.x() + stage.width() * 0.5, stage.max_y() - 10.0),
            palette.border.with_alpha(0.8),
        );
        draw_text(
            ctx,
            theme,
            Rect::new(
                stage.x() + 12.0,
                stage.y() + 8.0,
                stage.width() - 24.0,
                18.0,
            ),
            &format!("{:.2} s of {DURATION:.2} s", self.state.playhead()),
            DemoTextRole::Metadata,
            palette.text_muted,
        );

        let preview = self.state.preview();
        let center = Point::new(
            stage.x() + stage.width() * 0.5 + preview.offset * (stage.width() / 300.0).min(1.0),
            stage.y() + 28.0 + (stage.height() - 28.0) * 0.5,
        );
        let card = Rect::new(
            center.x - preview.size.width * 0.5,
            center.y - preview.size.height * 0.5,
            preview.size.width,
            preview.size.height,
        );
        ctx.fill(
            Path::rounded_rect(card, preview.radius.min(card.height() * 0.5)),
            preview
                .fill
                .with_alpha(preview.fill.alpha * preview.opacity),
        );
    }

    fn paint_curve(&self, ctx: &mut PaintCtx, layout: &StudioLayout, theme: DefaultTheme) {
        let palette = theme.palette;
        let area = layout.curve;
        if area.height() < 60.0 {
            return;
        }
        ctx.fill(Path::rounded_rect(area, 8.0), palette.field);
        let Some((path, keyframe)) = self.state.selected_keyframe() else {
            draw_text(
                ctx,
                theme,
                Rect::new(area.x() + 12.0, area.y() + 8.0, area.width() - 24.0, 18.0),
                "Select a keyframe to shape its curve",
                DemoTextRole::Metadata,
                palette.text_muted,
            );
            return;
        };
        let points = self.handle_points().unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let easing = if matches!(self.drag, Some(Drag::Handle { moved: true, .. })) {
            bezier_easing(points)
        } else {
            keyframe.easing
        };
        draw_text(
            ctx,
            theme,
            Rect::new(area.x() + 12.0, area.y() + 8.0, area.width() - 24.0, 18.0),
            &format!(
                "{path} at {:.2} s · {}",
                keyframe.time,
                easing_label(easing)
            ),
            DemoTextRole::Metadata,
            palette.text_muted,
        );

        let guide = palette.border.with_alpha(0.8);
        let corner = |x, y| layout.curve_point(x, y);
        hairline(ctx, corner(0.0, 0.0), corner(1.0, 0.0), guide);
        hairline(ctx, corner(0.0, 1.0), corner(1.0, 1.0), guide);
        let steps = 48;
        stroke_polyline(
            ctx,
            (0..=steps).map(|step| {
                let t = step as f32 / steps as f32;
                corner(t, easing.sample(t))
            }),
            palette.warning,
            2.0,
        );

        // Handles: dragging one turns the curve into a custom cubic bezier.
        let handles = [
            (corner(0.0, 0.0), corner(points[0], points[1])),
            (corner(1.0, 1.0), corner(points[2], points[3])),
        ];
        for (anchor, handle) in handles {
            stroke_polyline(
                ctx,
                [anchor, handle],
                palette.text_muted.with_alpha(0.7),
                1.0,
            );
            ctx.fill(Path::circle(handle, 5.5), palette.surface_raised);
            ctx.stroke(
                Path::circle(handle, 5.5),
                palette.warning,
                StrokeStyle::new(2.0),
            );
        }
    }
}

impl Widget for StudioEditor {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) => match pointer.kind {
                PointerEventKind::Down if ctx.bounds().contains(pointer.position) => {
                    self.pointer_down(ctx, pointer);
                }
                PointerEventKind::Move => self.pointer_move(ctx, pointer),
                PointerEventKind::Up => self.pointer_up(ctx, pointer),
                _ => {}
            },
            Event::Keyboard(key) if key.state == KeyState::Pressed => {
                self.key_down(ctx, &key.key);
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                if self.state.advance(motion_policy().scale_delta(*delta)) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
                ctx.request_semantics();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.state.is_playing() {
            ctx.request_animation_frame();
        }
        super::fill_width(constraints, EDITOR_HEIGHT)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        paint_card(ctx, bounds, theme);
        if ctx.focused_widget_id() == Some(ctx.widget_id()) {
            ctx.stroke(
                Path::rounded_rect(bounds.inflate(-1.0, -1.0), 9.0),
                theme.palette.border_focus,
                StrokeStyle::new(2.0),
            );
        }
        let layout = StudioLayout::new(bounds);
        self.paint_timeline(ctx, &layout, theme);
        self.paint_stage(ctx, &layout, theme);
        self.paint_curve(ctx, &layout, theme);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(STUDIO_EDITOR_NAME.to_string());
        let selected = self.state.selected_keyframe().map_or_else(
            || "no keyframe selected".to_string(),
            |(path, keyframe)| format!("selected {path} at {:.2} s", keyframe.time),
        );
        node.value = Some(SemanticsValue::Text(format!(
            "playhead {:.2} s of {DURATION:.2} s, {}, {selected}",
            self.state.playhead(),
            if self.state.is_playing() {
                "playing"
            } else {
                "paused"
            },
        )));
        node.state.focused = ctx.focused_widget_id() == Some(ctx.widget_id());
        ctx.push(node);
    }
}
