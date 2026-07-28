use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, PointerEventKind, SemanticsNode,
    SemanticsRole, SemanticsValue, Vector, paint_single_line_aligned_text,
};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::{LayerCompositionMode, LayerProperties};

use crate::app::{
    DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style, demo_text_style_when,
    dev_theme_color,
};

pub(crate) const ANIMATION_DEMO_TAB_LABEL: &str = "Animation";
pub(crate) const ANIMATION_DEMO_SCROLL_NAME: &str = "Animation demo scroll";
pub(crate) const ANIMATION_DEMO_NAME: &str = "Animation system examples";
pub(crate) const ANIMATION_TIMELINE_PREVIEW_NAME: &str = "Timeline playback example";
pub(crate) const ANIMATION_RETAINED_LAYER_NAME: &str = "Retained layer animation example";
pub(crate) const ANIMATION_PAINT_INVALIDATION_NAME: &str = "Paint invalidation animation example";
pub(crate) const ANIMATION_EDITOR_SURFACE_NAME: &str = "Animation document editor example";
pub(crate) const ANIMATION_DEMO_TEXT_INPUT_LABEL: &str = "Animation search";
pub(crate) const ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL: &str = "Animation timing";
pub(crate) const ANIMATION_DEMO_TOOLTIP_TEXT: &str =
    "Tooltip entry motion uses retained translation and opacity";
pub(crate) const ANIMATION_DEMO_POPOVER_NAME: &str = "Animation inspector";
pub(crate) const ANIMATION_DEMO_POPOVER_TRIGGER_LABEL: &str = "Open animation inspector";
pub(crate) const ANIMATION_PLAY_BUTTON_LABEL: &str = "Play";
pub(crate) const ANIMATION_PAUSE_BUTTON_LABEL: &str = "Pause";
pub(crate) const ANIMATION_RESTART_BUTTON_LABEL: &str = "Restart";
pub(crate) const ANIMATION_RETARGET_BUTTON_LABEL: &str = "Retarget primitives";
pub(crate) const ANIMATION_PLAYHEAD_NAME: &str = "Timeline playhead";
pub(crate) const ANIMATION_MOTION_SCALE_NAME: &str = "Demo motion scale";
pub(crate) const ANIMATION_EASING_NAME: &str = "Primitive easing";
pub(crate) const ANIMATION_EDITOR_EASING_NAME: &str = "Selected keyframe easing";

const TIMELINE_TARGET: &str = "animation-demo-timeline";
const TIMELINE_RADIUS_PATH: &str = "paint.radius";
const PRIMITIVE_DURATION: f64 = 0.9;
const EASING_OPTIONS: [&str; 5] = [
    "Linear",
    "Ease in",
    "Ease out",
    "Ease in-out",
    "Cubic bezier",
];
const PLAYBACK_RATE_OPTIONS: [&str; 3] = ["0.5×", "1×", "2×"];
const LOOP_MODE_OPTIONS: [&str; 2] = ["Once", "Repeat"];
const SNAP_OPTIONS: [&str; 2] = ["Free", "24 fps"];

pub(crate) fn build_animation_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let state = AnimationDemoState::new();
    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(
            18.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Stack::vertical()
                        .spacing(6.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new(ANIMATION_DEMO_NAME)
                                .style_when(demo_text_style_when(
                                    &theme_reader,
                                    DemoTextRole::PageTitle,
                                    |theme| {
                                    theme.palette.text
                                    },
                                )),
                        )
                        .with_child(
                            Label::new(
                                "Compare animation primitives, drive a shared timeline, inspect invalidation paths, and exercise theme motion in real widgets.",
                            )
                            .style_when(demo_text_style_when(
                                &theme_reader,
                                DemoTextRole::Supporting,
                                |theme| theme.palette.text_muted,
                            )),
                        ),
                )
                .with_child(section(
                    "Transport",
                    "One transport drives every custom example. Retarget primitives during playback to verify continuity.",
                    build_transport_controls(
                        state.clone(),
                        Rc::clone(&theme_reader),
                    ),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Motion primitives",
                    "Synchronized easing lanes compare Transition, AnimatedValue, MotionScalar, SpringF32, Pulse, and Blink.",
                    SizedBox::new()
                        .height(292.0)
                        .with_child(TimelinePlaybackExample::new(
                            state.clone(),
                            Rc::clone(&theme_reader),
                        )),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Theme motion in built-in widgets",
                    "The controls below use the live ThemeMotion duration ladder. Set motion scale to 0× for an instant, demo-local motion policy.",
                    theme_motion_gallery(
                        state.clone(),
                        Rc::clone(&theme_reader),
                    ),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Runtime invalidation",
                    "One TimelinePlayer maps opacity, translation, color, custom paint, and bounds tracks to the narrowest runtime invalidation.",
                    Flex::horizontal()
                        .gap(12.0)
                        .wrap(FlexWrap::Wrap)
                        .align_items(Alignment::Stretch)
                        .with_item(
                            SizedBox::new()
                                .height(156.0)
                                .with_child(RetainedLayerAnimationExample::new(
                                    state.clone(),
                                    Rc::clone(&theme_reader),
                                )),
                            FlexItem::new().basis_fraction(0.5).min_width(300.0),
                        )
                        .with_item(
                            SizedBox::new()
                                .height(156.0)
                                .with_child(PaintInvalidationAnimationExample::new(
                                    state.clone(),
                                    Rc::clone(&theme_reader),
                                )),
                            FlexItem::new().basis_fraction(0.5).min_width(300.0),
                        ),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Timeline studio",
                    "Playback, selection, snapping, easing edits, add/remove, and undo/redo all operate on AnimationEditorState.",
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(build_editor_controls(
                            state.clone(),
                            Rc::clone(&theme_reader),
                        ))
                        .with_child(
                            SizedBox::new()
                                .height(376.0)
                                .with_child(AnimationDocumentEditorExample::new(
                                    state,
                                    Rc::clone(&theme_reader),
                                )),
                        ),
                    Rc::clone(&theme_reader),
                )),
        ))
        .name(ANIMATION_DEMO_SCROLL_NAME),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn section<W>(title: &str, description: &str, body: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(Label::new(title).style_when(demo_text_style_when(
            &theme_reader,
            DemoTextRole::SectionTitle,
            |theme| theme.palette.text,
        )))
        .with_child(Label::new(description).style_when(demo_text_style_when(
            &theme_reader,
            DemoTextRole::Supporting,
            |theme| theme.palette.text_muted,
        )))
        .with_child(body)
        .with_child(
            Separator::horizontal()
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .inset(0.0),
        )
}

fn build_transport_controls(
    state: AnimationDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let restart_state = state.clone();
    let play_state = state.clone();
    let pause_state = state.clone();
    let step_state = state.clone();
    let retarget_state = state.clone();
    let playhead_reader = state.clone();
    let playhead_change = state.clone();
    let rate_reader = state.clone();
    let rate_change = state.clone();
    let loop_reader = state.clone();
    let loop_change = state.clone();
    let scale_reader = state.clone();
    let scale_change = state.clone();
    let easing_reader = state.clone();
    let easing_change = state.clone();
    let status_state = state;

    Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Flex::horizontal()
                .gap(8.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Center)
                .with_item(
                    Button::new(ANIMATION_RESTART_BUTTON_LABEL)
                        .icon(IconGlyph::Restore)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .on_press_with_ctx(move |ctx| {
                            restart_state.restart();
                            request_animation_demo_refresh(ctx, true);
                        }),
                    FlexItem::fixed(124.0),
                )
                .with_item(
                    Button::new(ANIMATION_PLAY_BUTTON_LABEL)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .on_press_with_ctx(move |ctx| {
                            play_state.play();
                            request_animation_demo_refresh(ctx, true);
                        }),
                    FlexItem::fixed(84.0),
                )
                .with_item(
                    Button::new(ANIMATION_PAUSE_BUTTON_LABEL)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .on_press_with_ctx(move |ctx| {
                            pause_state.pause();
                            request_animation_demo_refresh(ctx, false);
                        }),
                    FlexItem::fixed(84.0),
                )
                .with_item(
                    Button::new("Step 1/60 s")
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .on_press_with_ctx(move |ctx| {
                            step_state.step(1.0 / 60.0);
                            request_animation_demo_refresh(ctx, false);
                        }),
                    FlexItem::fixed(112.0),
                )
                .with_item(
                    Button::new(ANIMATION_RETARGET_BUTTON_LABEL)
                        .theme_when(clone_dev_theme_reader(&theme_reader))
                        .on_press_with_ctx(move |ctx| {
                            retarget_state.retarget();
                            request_animation_demo_refresh(ctx, true);
                        }),
                    FlexItem::fixed(154.0),
                ),
        )
        .with_child(
            Flex::horizontal()
                .gap(12.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Start)
                .with_item(
                    PropertyRow::new(
                        "Playhead",
                        Slider::new(ANIMATION_PLAYHEAD_NAME)
                            .range(0.0, 1.8)
                            .step(1.0 / 120.0)
                            .value_when(move || playhead_reader.playhead())
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .on_change_with_ctx(move |ctx, value| {
                                playhead_change.seek(value);
                                request_animation_demo_refresh(ctx, false);
                            }),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .stacked(),
                    FlexItem::new().basis_fraction(0.36).min_width(280.0),
                )
                .with_item(
                    PropertyRow::new(
                        "Speed",
                        Select::new("Playback speed")
                            .options(PLAYBACK_RATE_OPTIONS)
                            .selected_when(move || Some(rate_reader.playback_rate_index()))
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .on_change_with_ctx(move |ctx, index, _| {
                                rate_change.set_playback_rate(index);
                                request_animation_demo_refresh(ctx, true);
                            }),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .stacked(),
                    FlexItem::new().basis_fraction(0.16).min_width(132.0),
                )
                .with_item(
                    PropertyRow::new(
                        "Loop",
                        Select::new("Timeline loop mode")
                            .options(LOOP_MODE_OPTIONS)
                            .selected_when(move || Some(loop_reader.loop_mode_index()))
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .on_change_with_ctx(move |ctx, index, _| {
                                loop_change.set_loop_mode(index);
                                request_animation_demo_refresh(ctx, false);
                            }),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .stacked(),
                    FlexItem::new().basis_fraction(0.16).min_width(132.0),
                )
                .with_item(
                    PropertyRow::new(
                        "Motion scale",
                        Slider::new(ANIMATION_MOTION_SCALE_NAME)
                            .range(0.0, 2.0)
                            .step(0.1)
                            .value_when(move || scale_reader.motion_scale())
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .on_change_with_ctx(move |ctx, value| {
                                scale_change.set_motion_scale(value as f32);
                                request_animation_demo_refresh(ctx, true);
                            }),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .stacked(),
                    FlexItem::new().basis_fraction(0.2).min_width(180.0),
                ),
        )
        .with_child(
            Flex::horizontal()
                .gap(12.0)
                .wrap(FlexWrap::Wrap)
                .align_items(Alignment::Center)
                .with_item(
                    PropertyRow::new(
                        "Primitive easing",
                        Select::new(ANIMATION_EASING_NAME)
                            .options(EASING_OPTIONS)
                            .selected_when(move || Some(easing_reader.selected_easing_index()))
                            .theme_when(clone_dev_theme_reader(&theme_reader))
                            .on_change_with_ctx(move |ctx, index, _| {
                                easing_change.set_easing(index);
                                request_animation_demo_refresh(ctx, true);
                            }),
                    )
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .stacked(),
                    FlexItem::fixed(230.0),
                )
                .with_item(
                    Label::dynamic("Playing · 0.00 / 1.80 s", move || {
                        status_state.transport_summary()
                    })
                    .style_when(demo_text_style_when(
                        &theme_reader,
                        DemoTextRole::Metadata,
                        |theme| theme.palette.text_muted,
                    )),
                    FlexItem::new().basis_fraction(1.0).min_width(260.0),
                ),
        )
}

fn theme_motion_gallery(state: AnimationDemoState, theme_reader: DevThemeReader) -> impl Widget {
    let scaled_theme = scaled_motion_theme_reader(state.clone(), Rc::clone(&theme_reader));
    let overlay_key_state = state.clone();
    let overlay_build_state = state.clone();
    let overlay_theme_reader = Rc::clone(&theme_reader);
    let overlays = RebuildOnChange::new(
        move || overlay_key_state.motion_scale() as f32,
        move |_| {
            let scaled_theme = scale_theme_motion(
                overlay_theme_reader(),
                overlay_build_state.motion_scale() as f32,
            );
            WidgetPod::new(
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        Tooltip::new(
                            ANIMATION_DEMO_TOOLTIP_TEXT,
                            Button::new(ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL).theme(scaled_theme),
                        )
                        .theme(scaled_theme),
                    )
                    .with_child(
                        Popover::new(
                            ANIMATION_DEMO_POPOVER_NAME,
                            Button::new(ANIMATION_DEMO_POPOVER_TRIGGER_LABEL).theme(scaled_theme),
                            Stack::vertical()
                                .spacing(6.0)
                                .with_child(Label::new("TimelinePlayer samples five tracks."))
                                .with_child(Label::new(
                                    "Transform, effect, paint, and measure stay distinct.",
                                )),
                        )
                        .theme(scaled_theme),
                    ),
            )
        },
    );
    let token_state = state;
    Flex::horizontal()
        .gap(12.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start)
        .with_item(
            Surface::panel(
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Start)
                    .with_child(
                        Label::new("Fast and normal").style_when(demo_text_style_when(
                            &theme_reader,
                            DemoTextRole::CardTitle,
                            |theme| theme.palette.text,
                        )),
                    )
                    .with_child(
                        Button::new("Hover and press")
                            .theme_when(clone_dev_theme_reader(&scaled_theme)),
                    )
                    .with_child(
                        Switch::new("Toggle motion")
                            .on(true)
                            .theme_when(clone_dev_theme_reader(&scaled_theme)),
                    )
                    .with_child(
                        SizedBox::new().width(280.0).with_child(
                            TextInput::new(ANIMATION_DEMO_TEXT_INPUT_LABEL)
                                .value("Focus shows the 140 ms token")
                                .theme_when(clone_dev_theme_reader(&scaled_theme)),
                        ),
                    ),
            )
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .padding(Insets::all(14.0)),
            FlexItem::new().basis_fraction(0.48).min_width(310.0),
        )
        .with_item(
            Surface::panel(
                Stack::vertical()
                    .spacing(10.0)
                    .alignment(Alignment::Start)
                    .with_child(Label::new("Entrances and overlays").style_when(
                        demo_text_style_when(&theme_reader, DemoTextRole::CardTitle, |theme| {
                            theme.palette.text
                        }),
                    ))
                    .with_child(overlays)
                    .with_child(
                        Label::dynamic("70 / 140 / 220 / 340 ms", move || {
                            token_state.motion_token_summary()
                        })
                        .style_when(demo_text_style_when(
                            &theme_reader,
                            DemoTextRole::Metadata,
                            |theme| theme.palette.text_muted,
                        )),
                    ),
            )
            .theme_when(clone_dev_theme_reader(&theme_reader))
            .padding(Insets::all(14.0)),
            FlexItem::new().basis_fraction(0.52).min_width(330.0),
        )
}

fn build_editor_controls(state: AnimationDemoState, theme_reader: DevThemeReader) -> impl Widget {
    let add_state = state.clone();
    let remove_state = state.clone();
    let undo_enabled = state.clone();
    let undo_state = state.clone();
    let redo_enabled = state.clone();
    let redo_state = state.clone();
    let easing_reader = state.clone();
    let easing_change = state.clone();
    let snap_reader = state.clone();
    let snap_change = state.clone();
    let summary_state = state;

    Flex::horizontal()
        .gap(8.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Center)
        .with_item(
            Button::new("Add keyframe")
                .icon(IconGlyph::Add)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_press_with_ctx(move |ctx| {
                    add_state.add_keyframe();
                    request_animation_demo_refresh(ctx, false);
                }),
            FlexItem::fixed(132.0),
        )
        .with_item(
            Button::new("Remove keyframe")
                .icon(IconGlyph::Trash)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_press_with_ctx(move |ctx| {
                    remove_state.remove_selected_keyframe();
                    request_animation_demo_refresh(ctx, false);
                }),
            FlexItem::fixed(152.0),
        )
        .with_item(
            IconButton::new(IconGlyph::Undo, "Undo animation edit")
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .enabled_when(move || undo_enabled.can_undo_editor())
                .on_press_with_ctx(move |ctx| {
                    undo_state.undo_editor();
                    request_animation_demo_refresh(ctx, false);
                }),
            FlexItem::fixed(36.0),
        )
        .with_item(
            IconButton::new(IconGlyph::Redo, "Redo animation edit")
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .enabled_when(move || redo_enabled.can_redo_editor())
                .on_press_with_ctx(move |ctx| {
                    redo_state.redo_editor();
                    request_animation_demo_refresh(ctx, false);
                }),
            FlexItem::fixed(36.0),
        )
        .with_item(
            Select::new(ANIMATION_EDITOR_EASING_NAME)
                .options(EASING_OPTIONS)
                .selected_when(move || Some(easing_reader.editor_easing_index()))
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_change_with_ctx(move |ctx, index, _| {
                    easing_change.set_editor_easing(index);
                    request_animation_demo_refresh(ctx, false);
                }),
            FlexItem::fixed(170.0),
        )
        .with_item(
            Select::new("Timeline snapping")
                .options(SNAP_OPTIONS)
                .selected_when(move || Some(snap_reader.snap_index()))
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_change_with_ctx(move |ctx, index, _| {
                    snap_change.set_snap(index);
                    request_animation_demo_refresh(ctx, false);
                }),
            FlexItem::fixed(128.0),
        )
        .with_item(
            Label::dynamic("5 tracks · 1 selected", move || {
                let editor = summary_state.editor_snapshot();
                let tracks = editor
                    .document
                    .timeline
                    .clips
                    .first()
                    .map(|clip| clip.tracks.len())
                    .unwrap_or_default();
                format!(
                    "{tracks} tracks · {} selected · undo {} / redo {}",
                    editor.selection.keyframes.len(),
                    editor.undo_len(),
                    editor.redo_len()
                )
            })
            .style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::Metadata,
                |theme| theme.palette.text_muted,
            )),
            FlexItem::new().basis_fraction(1.0).min_width(220.0),
        )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TimelineExamplePresentation {
    opacity: f32,
    translation: Vector,
    fill: Color,
    radius: f32,
    bounds: Rect,
}

impl Default for TimelineExamplePresentation {
    fn default() -> Self {
        Self {
            opacity: 0.42,
            translation: Vector::new(-30.0, 0.0),
            fill: Color::rgba(0.20, 0.45, 0.95, 1.0),
            radius: 14.0,
            bounds: Rect::new(0.0, 0.0, 64.0, 34.0),
        }
    }
}

impl TimelineBindingSink for TimelineExamplePresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != TIMELINE_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.0, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
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
            (AnimationProperty::Bounds, AnimationValue::Rect(value)) => {
                let changed = self.bounds != value;
                self.bounds = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == TIMELINE_RADIUS_PATH =>
            {
                let value = value.max(4.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            _ => false,
        }
    }
}

#[derive(Clone)]
struct AnimationDemoState {
    inner: Rc<RefCell<AnimationDemoStateInner>>,
}

struct AnimationDemoStateInner {
    player: TimelinePlayer,
    presentation: TimelineExamplePresentation,
    clock: f64,
    motion_scale: f32,
    selected_easing: usize,
    primitive_target: f32,
    primitive_transition: Transition<f32>,
    animated_value: AnimatedValue<f32>,
    motion_scalar: MotionScalar,
    spring: SpringF32,
    invalidation_counts: [u64; 4],
    editor: AnimationEditorState,
}

#[derive(Debug, Clone, Copy)]
struct AnimationDemoSnapshot {
    presentation: TimelineExamplePresentation,
    playback: PlaybackState,
    clock: f64,
    motion_scale: f32,
    primitive_start: f32,
    primitive_target: f32,
    transition_value: f32,
    transition_progress: f32,
    animated_value: f32,
    motion_scalar: f32,
    spring_value: f32,
    spring_velocity: f32,
    invalidation_counts: [u64; 4],
}

impl AnimationDemoState {
    fn new() -> Self {
        let timeline = timeline_playback_example_timeline();
        let mut player = TimelinePlayer::new(timeline.clone());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        player.play();

        let mut presentation = TimelineExamplePresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }

        let easing = easing_for_index(3);
        let mut animated_value = AnimatedValue::new(0.0_f32)
            .with_duration(PRIMITIVE_DURATION as f32)
            .with_easing(easing);
        animated_value.set_target(1.0);
        let mut motion_scalar = MotionScalar::new(0.0);
        motion_scalar.set_target(1.0, 0.0, PRIMITIVE_DURATION, easing);

        let mut editor =
            AnimationEditorState::new(AnimationDocument::new("Motion Lab timeline", timeline));
        editor.playback = player.playback();
        editor.apply_command(AnimationEditorCommand::SelectKeyframe(KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 1,
        }));

        Self {
            inner: Rc::new(RefCell::new(AnimationDemoStateInner {
                player,
                presentation,
                clock: 0.0,
                motion_scale: 1.0,
                selected_easing: 3,
                primitive_target: 1.0,
                primitive_transition: Transition::new(0.0, 1.0, 0.0, PRIMITIVE_DURATION, easing),
                animated_value,
                motion_scalar,
                spring: SpringF32::new(0.0).with_config(150.0, 18.0),
                invalidation_counts: [0; 4],
                editor,
            })),
        }
    }

    fn snapshot(&self) -> AnimationDemoSnapshot {
        let inner = self.inner.borrow();
        AnimationDemoSnapshot {
            presentation: inner.presentation,
            playback: inner.player.playback(),
            clock: inner.clock,
            motion_scale: inner.motion_scale,
            primitive_start: inner.primitive_transition.start,
            primitive_target: inner.primitive_target,
            transition_value: inner.primitive_transition.sample(inner.clock),
            transition_progress: inner.primitive_transition.progress(inner.clock),
            animated_value: inner.animated_value.value(),
            motion_scalar: inner.motion_scalar.value,
            spring_value: inner.spring.value,
            spring_velocity: inner.spring.velocity,
            invalidation_counts: inner.invalidation_counts,
        }
    }

    fn advance(&self, delta: f64) -> bool {
        let mut inner = self.inner.borrow_mut();
        if !inner.player.playback().playing {
            return false;
        }

        let scaled_delta = delta.max(0.0) * inner.player.playback().playback_rate.abs();
        inner.clock += scaled_delta;
        let clock = inner.clock;
        inner.animated_value.tick(scaled_delta as f32);
        inner.motion_scalar.advance(clock);
        let spring_target = inner.primitive_target;
        let mut spring_delta = scaled_delta.min(0.25);
        while spring_delta > f64::EPSILON {
            let step = spring_delta.min(1.0 / 120.0);
            inner.spring.step(spring_target, step);
            spring_delta -= step;
        }

        let mut presentation = inner.presentation;
        let (invalidations, should_continue) = {
            let tick = inner.player.tick(delta, &mut presentation);
            (
                tick.invalidations
                    .iter()
                    .map(|invalidation| invalidation.kind)
                    .collect::<Vec<_>>(),
                tick.should_continue,
            )
        };
        inner.presentation = presentation;
        let playback = inner.player.playback();
        inner.editor.playback = playback;
        for invalidation in invalidations {
            if let Some(index) = invalidation_count_index(invalidation) {
                inner.invalidation_counts[index] =
                    inner.invalidation_counts[index].saturating_add(1);
            }
        }
        should_continue
    }

    fn play(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.player.playback().playhead >= inner.player.timeline().duration
            && inner.player.playback().loop_mode == LoopMode::Once
        {
            inner.player.seek(0.0);
            inner
                .editor
                .apply_command(AnimationEditorCommand::SetPlayhead(0.0));
            resample_demo_timeline(&mut inner);
        }
        inner.player.play();
        inner.editor.playback = inner.player.playback();
    }

    fn pause(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.player.pause();
        inner.editor.playback = inner.player.playback();
    }

    fn restart(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.player.stop();
        inner.player.play();
        inner.clock = 0.0;
        inner.invalidation_counts = [0; 4];
        reset_primitive_motion(&mut inner, 1.0);
        inner
            .editor
            .apply_command(AnimationEditorCommand::SetPlayhead(0.0));
        inner.editor.playback = inner.player.playback();
        resample_demo_timeline(&mut inner);
    }

    fn step(&self, delta: f64) {
        let was_playing = self.snapshot().playback.playing;
        {
            let mut inner = self.inner.borrow_mut();
            inner.player.play();
        }
        self.advance(delta);
        if !was_playing {
            self.pause();
        }
    }

    fn seek(&self, time: f64) {
        let mut inner = self.inner.borrow_mut();
        inner.player.seek(time);
        inner.clock = time.max(0.0);
        inner
            .editor
            .apply_command(AnimationEditorCommand::SetPlayhead(time));
        inner.editor.playback = inner.player.playback();
        resample_demo_timeline(&mut inner);
    }

    fn retarget(&self) {
        let mut inner = self.inner.borrow_mut();
        let target = if inner.primitive_target >= 0.5 {
            0.0
        } else {
            1.0
        };
        reset_primitive_motion(&mut inner, target);
    }

    fn set_easing(&self, index: usize) {
        let mut inner = self.inner.borrow_mut();
        inner.selected_easing = index.min(EASING_OPTIONS.len() - 1);
        let target = if inner.primitive_target >= 0.5 {
            0.0
        } else {
            1.0
        };
        reset_primitive_motion(&mut inner, target);
    }

    fn selected_easing_index(&self) -> usize {
        self.inner.borrow().selected_easing
    }

    fn set_motion_scale(&self, scale: f32) {
        let mut inner = self.inner.borrow_mut();
        inner.motion_scale = scale.clamp(0.0, 2.0);
        let target = inner.primitive_target;
        reset_primitive_motion(&mut inner, target);
    }

    fn motion_scale(&self) -> f64 {
        f64::from(self.inner.borrow().motion_scale)
    }

    fn set_playback_rate(&self, index: usize) {
        let rate = match index {
            0 => 0.5,
            2 => 2.0,
            _ => 1.0,
        };
        let mut inner = self.inner.borrow_mut();
        inner.player.playback_mut().playback_rate = rate;
        inner.editor.playback.playback_rate = rate;
    }

    fn playback_rate_index(&self) -> usize {
        match self.inner.borrow().player.playback().playback_rate {
            rate if rate < 0.75 => 0,
            rate if rate > 1.5 => 2,
            _ => 1,
        }
    }

    fn set_loop_mode(&self, index: usize) {
        let loop_mode = if index == 0 {
            LoopMode::Once
        } else {
            LoopMode::Repeat
        };
        let mut inner = self.inner.borrow_mut();
        inner.player.playback_mut().loop_mode = loop_mode;
        inner.editor.playback.loop_mode = loop_mode;
    }

    fn loop_mode_index(&self) -> usize {
        usize::from(self.inner.borrow().player.playback().loop_mode == LoopMode::Repeat)
    }

    fn playhead(&self) -> f64 {
        self.inner.borrow().player.playback().playhead
    }

    fn transport_summary(&self) -> String {
        let snapshot = self.snapshot();
        let state = if snapshot.playback.playing {
            "Playing"
        } else {
            "Paused"
        };
        format!(
            "{state} · {:.2} / 1.80 s · {:.1}× · {}",
            snapshot.playback.playhead,
            snapshot.playback.playback_rate,
            if snapshot.playback.loop_mode == LoopMode::Repeat {
                "repeat"
            } else {
                "once"
            }
        )
    }

    fn motion_token_summary(&self) -> String {
        let scale = self.inner.borrow().motion_scale;
        format!(
            "{:.0} / {:.0} / {:.0} / {:.0} ms at {scale:.1}×",
            70.0 * scale,
            140.0 * scale,
            220.0 * scale,
            340.0 * scale
        )
    }

    fn editor_snapshot(&self) -> AnimationEditorState {
        self.inner.borrow().editor.clone()
    }

    fn select_keyframe(&self, selection: KeyframeSelection) {
        self.inner
            .borrow_mut()
            .editor
            .apply_command(AnimationEditorCommand::SelectKeyframe(selection));
    }

    fn editor_easing_index(&self) -> usize {
        let inner = self.inner.borrow();
        inner
            .editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe(&inner.editor, *selection))
            .map(|keyframe| easing_index(keyframe.easing))
            .unwrap_or(0)
    }

    fn set_editor_easing(&self, index: usize) {
        let mut inner = self.inner.borrow_mut();
        let Some(selection) = inner.editor.selection.keyframes.last().copied() else {
            return;
        };
        if inner
            .editor
            .apply_command(AnimationEditorCommand::UpdateKeyframeEasing {
                selection,
                easing: easing_for_index(index),
            })
        {
            sync_player_from_editor(&mut inner);
        }
    }

    fn add_keyframe(&self) {
        let mut inner = self.inner.borrow_mut();
        let track_index = inner.editor.selection.track_index.unwrap_or_default().min(
            inner
                .editor
                .document
                .timeline
                .clips
                .first()
                .map(|clip| clip.tracks.len().saturating_sub(1))
                .unwrap_or_default(),
        );
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
            return;
        };
        let easing = easing_for_index(inner.selected_easing);
        if inner
            .editor
            .apply_command(AnimationEditorCommand::AddKeyframe {
                clip_index: 0,
                track_index,
                keyframe: Keyframe::new(playhead, value).with_easing(easing),
            })
        {
            sync_player_from_editor(&mut inner);
        }
    }

    fn remove_selected_keyframe(&self) {
        let mut inner = self.inner.borrow_mut();
        let Some(selection) = inner.editor.selection.keyframes.last().copied() else {
            return;
        };
        if inner
            .editor
            .apply_command(AnimationEditorCommand::RemoveKeyframe(selection))
        {
            sync_player_from_editor(&mut inner);
        }
    }

    fn undo_editor(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.editor.undo() {
            sync_player_from_editor(&mut inner);
        }
    }

    fn redo_editor(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.editor.redo() {
            sync_player_from_editor(&mut inner);
        }
    }

    fn can_undo_editor(&self) -> bool {
        self.inner.borrow().editor.can_undo()
    }

    fn can_redo_editor(&self) -> bool {
        self.inner.borrow().editor.can_redo()
    }

    fn set_snap(&self, index: usize) {
        let snap = if index == 0 {
            TimelineSnap::disabled()
        } else {
            TimelineSnap::new(1.0 / 24.0)
        };
        self.inner
            .borrow_mut()
            .editor
            .apply_command(AnimationEditorCommand::SetSnapping(snap));
    }

    fn snap_index(&self) -> usize {
        usize::from(self.inner.borrow().editor.snap.enabled)
    }
}

fn easing_for_index(index: usize) -> Easing {
    match index {
        0 => Easing::Linear,
        1 => Easing::EaseIn,
        2 => Easing::EaseOut,
        4 => Easing::CubicBezier {
            x1: 0.2,
            y1: 0.0,
            x2: 0.0,
            y2: 1.0,
        },
        _ => Easing::EaseInOut,
    }
}

fn easing_index(easing: Easing) -> usize {
    match easing {
        Easing::Linear => 0,
        Easing::EaseIn => 1,
        Easing::EaseOut => 2,
        Easing::EaseInOut => 3,
        Easing::CubicBezier { .. } => 4,
    }
}

fn reset_primitive_motion(inner: &mut AnimationDemoStateInner, target: f32) {
    let easing = easing_for_index(inner.selected_easing);
    let duration = PRIMITIVE_DURATION * f64::from(inner.motion_scale);
    let transition_current = inner.primitive_transition.sample(inner.clock);
    inner.primitive_target = target;
    inner.primitive_transition =
        Transition::new(transition_current, target, inner.clock, duration, easing);
    inner.animated_value.set_duration(duration as f32);
    inner.animated_value.set_easing(easing);
    inner.animated_value.set_target(target);
    inner
        .motion_scalar
        .set_target(target, inner.clock, duration, easing);
    if duration <= f64::EPSILON {
        inner.animated_value.jump_to(target);
        inner.motion_scalar = MotionScalar::new(target);
        inner.spring.value = target;
        inner.spring.velocity = 0.0;
    }
}

fn resample_demo_timeline(inner: &mut AnimationDemoStateInner) {
    let samples = inner.player.sample_reusing_scratch().to_vec();
    for sample in samples {
        inner
            .presentation
            .apply_animation_value(&sample.binding, sample.value);
    }
}

fn sync_player_from_editor(inner: &mut AnimationDemoStateInner) {
    let playback = inner.player.playback();
    inner
        .player
        .set_timeline(inner.editor.document.timeline.clone());
    *inner.player.playback_mut() = playback;
    inner
        .player
        .seek(playback.playhead.min(inner.player.timeline().duration));
    inner.editor.playback = inner.player.playback();
    resample_demo_timeline(inner);
}

fn invalidation_count_index(kind: InvalidationKind) -> Option<usize> {
    match kind {
        InvalidationKind::Transform => Some(0),
        InvalidationKind::Effect => Some(1),
        InvalidationKind::Paint => Some(2),
        InvalidationKind::Measure => Some(3),
        _ => None,
    }
}

fn scaled_motion_theme_reader(
    state: AnimationDemoState,
    theme_reader: DevThemeReader,
) -> DevThemeReader {
    Rc::new(move || scale_theme_motion(theme_reader(), state.motion_scale() as f32))
}

fn scale_theme_motion(mut theme: DefaultTheme, scale: f32) -> DefaultTheme {
    theme.motion.duration_fast *= scale;
    theme.motion.duration_normal *= scale;
    theme.motion.duration_slow *= scale;
    theme.motion.duration_slower *= scale;
    theme
}

fn request_animation_demo_refresh(ctx: &mut EventCtx, animate: bool) {
    for kind in [
        InvalidationKind::Paint,
        InvalidationKind::HitTest,
        InvalidationKind::Semantics,
    ] {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            kind,
        ));
    }
    if animate {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            InvalidationKind::Measure,
        ));
        ctx.request_animation_frame();
    }
}

struct TimelinePlaybackExample {
    state: AnimationDemoState,
    theme_reader: DevThemeReader,
}

impl TimelinePlaybackExample {
    fn new(state: AnimationDemoState, theme_reader: DevThemeReader) -> Self {
        Self {
            state,
            theme_reader,
        }
    }
}

impl Widget for TimelinePlaybackExample {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.state.retarget();
                request_animation_demo_refresh(ctx, true);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let should_continue = self.state.advance(*delta);
                request_animation_demo_refresh(ctx, false);
                if should_continue {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.state.snapshot().playback.playing {
            ctx.request_animation_frame();
        }
        constraints.clamp(Size::new(680.0, 292.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = (self.theme_reader)();
        let snapshot = self.state.snapshot();
        paint_demo_surface(
            ctx,
            bounds,
            theme.palette.surface_raised,
            theme.palette.border,
        );
        draw_demo_label(
            ctx,
            theme,
            Rect::new(bounds.x() + 18.0, bounds.y() + 14.0, 260.0, 22.0),
            "Synchronized easing lanes",
            DemoTextRole::CardTitle,
            theme.palette.text,
        );

        let easing_area = Rect::new(
            bounds.x() + 18.0,
            bounds.y() + 44.0,
            (bounds.width() * 0.49 - 28.0).max(220.0),
            bounds.height() - 62.0,
        );
        let primitive_area = Rect::new(
            bounds.x() + bounds.width() * 0.51,
            bounds.y() + 14.0,
            bounds.max_x() - (bounds.x() + bounds.width() * 0.51) - 18.0,
            bounds.height() - 28.0,
        );

        for (index, label) in EASING_OPTIONS.iter().enumerate() {
            let easing = easing_for_index(index);
            let y = easing_area.y() + index as f32 * 42.0;
            draw_demo_label(
                ctx,
                theme,
                Rect::new(easing_area.x(), y, 86.0, 18.0),
                *label,
                DemoTextRole::Metadata,
                if index == self.state.selected_easing_index() {
                    theme.palette.accent
                } else {
                    theme.palette.text_muted
                },
            );
            let rail = Rect::new(
                easing_area.x() + 92.0,
                y + 7.0,
                easing_area.width() - 100.0,
                4.0,
            );
            ctx.fill(
                Path::rounded_rect(rail, 2.0),
                theme.palette.border.with_alpha(0.72),
            );
            let eased = easing.sample(snapshot.transition_progress);
            let value =
                f32::interpolate(snapshot.primitive_start, snapshot.primitive_target, eased);
            let x = rail.x() + rail.width() * value.clamp(0.0, 1.0);
            ctx.fill(
                Path::circle(Point::new(x, rail.y() + 2.0), 6.0),
                if index == self.state.selected_easing_index() {
                    theme.palette.accent
                } else {
                    theme.palette.text
                },
            );
        }

        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                primitive_area.x(),
                primitive_area.y(),
                primitive_area.width(),
                22.0,
            ),
            "Primitive values",
            DemoTextRole::CardTitle,
            theme.palette.text,
        );
        let pulse = Pulse::new(
            (1.2 * f64::from(snapshot.motion_scale)).max(f64::EPSILON),
            0.15,
            1.0,
        );
        let blink = Blink::new((0.8 * f64::from(snapshot.motion_scale)).max(f64::EPSILON));
        let rows = [
            ("Transition<T>", snapshot.transition_value),
            ("AnimatedValue<T>", snapshot.animated_value),
            ("MotionScalar", snapshot.motion_scalar),
            ("SpringF32", snapshot.spring_value),
            (
                "Pulse",
                if snapshot.motion_scale <= f32::EPSILON {
                    1.0
                } else {
                    pulse.sample(snapshot.clock)
                },
            ),
            (
                "Blink",
                if snapshot.motion_scale <= f32::EPSILON || blink.is_on(snapshot.clock) {
                    1.0
                } else {
                    0.12
                },
            ),
        ];
        for (index, (label, value)) in rows.into_iter().enumerate() {
            paint_value_bar(
                ctx,
                theme,
                Rect::new(
                    primitive_area.x(),
                    primitive_area.y() + 34.0 + index as f32 * 35.0,
                    primitive_area.width(),
                    26.0,
                ),
                label,
                value,
            );
        }
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                primitive_area.x(),
                primitive_area.max_y() - 20.0,
                primitive_area.width(),
                18.0,
            ),
            format!(
                "target {:.0} · spring velocity {:+.2}",
                snapshot.primitive_target, snapshot.spring_velocity
            ),
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let snapshot = self.state.snapshot();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_TIMELINE_PREVIEW_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "transition {:.2}, animated value {:.2}, motion scalar {:.2}, spring {:.2}, target {:.0}",
            snapshot.transition_value,
            snapshot.animated_value,
            snapshot.motion_scalar,
            snapshot.spring_value,
            snapshot.primitive_target,
        )));
        ctx.push(node);
    }
}

struct RetainedLayerAnimationExample {
    state: AnimationDemoState,
    theme_reader: DevThemeReader,
}

impl RetainedLayerAnimationExample {
    fn new(state: AnimationDemoState, theme_reader: DevThemeReader) -> Self {
        Self {
            state,
            theme_reader,
        }
    }
}

impl Widget for RetainedLayerAnimationExample {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(300.0, 156.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = (self.theme_reader)();
        let snapshot = self.state.snapshot();
        paint_demo_surface(
            ctx,
            bounds,
            theme.palette.surface_raised,
            theme.palette.border,
        );
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 14.0,
                bounds.y() + 12.0,
                bounds.width() - 28.0,
                20.0,
            ),
            "Retained transform/effect",
            DemoTextRole::CardTitle,
            theme.palette.text,
        );

        let rail = Rect::new(
            bounds.x() + 30.0,
            bounds.y() + bounds.height() * 0.58 - 3.0,
            bounds.width() - 60.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            theme.palette.border.with_alpha(0.64),
        );

        let marker = Rect::new(
            bounds.x() + bounds.width() * 0.5 - 34.0,
            bounds.y() + 58.0,
            68.0,
            34.0,
        );
        ctx.fill(
            Path::rounded_rect(marker, 7.0),
            theme.palette.accent.with_alpha(0.88),
        );
        ctx.stroke_rect(marker, theme.palette.border_focus, StrokeStyle::new(1.0));
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 14.0,
                bounds.max_y() - 44.0,
                bounds.width() - 28.0,
                18.0,
            ),
            format!(
                "Transform {:>4} requests · x {:+.1}",
                snapshot.invalidation_counts[0], snapshot.presentation.translation.x
            ),
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 14.0,
                bounds.max_y() - 24.0,
                bounds.width() - 28.0,
                18.0,
            ),
            format!(
                "Effect {:>7} requests · opacity {:.2}",
                snapshot.invalidation_counts[1], snapshot.presentation.opacity
            ),
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        let presentation = self.state.snapshot().presentation;
        LayerProperties::default()
            .with_opacity(presentation.opacity)
            .with_translation(presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        let snapshot = self.state.snapshot();
        node.name = Some(ANIMATION_RETAINED_LAYER_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "transform requests {}, effect requests {}, opacity {:.2}, x {:.1}",
            snapshot.invalidation_counts[0],
            snapshot.invalidation_counts[1],
            snapshot.presentation.opacity,
            snapshot.presentation.translation.x
        )));
        ctx.push(node);
    }
}

struct PaintInvalidationAnimationExample {
    state: AnimationDemoState,
    theme_reader: DevThemeReader,
}

impl PaintInvalidationAnimationExample {
    fn new(state: AnimationDemoState, theme_reader: DevThemeReader) -> Self {
        Self {
            state,
            theme_reader,
        }
    }
}

impl Widget for PaintInvalidationAnimationExample {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(300.0, 156.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = (self.theme_reader)();
        let snapshot = self.state.snapshot();
        paint_demo_surface(
            ctx,
            bounds,
            theme.palette.surface_raised,
            theme.palette.border,
        );
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 14.0,
                bounds.y() + 12.0,
                bounds.width() - 28.0,
                20.0,
            ),
            "Paint/measure invalidation",
            DemoTextRole::CardTitle,
            theme.palette.text,
        );

        let lanes = 7;
        let animated_bounds = Rect::new(
            bounds.x() + bounds.width() * 0.5 - snapshot.presentation.bounds.width() * 0.5,
            bounds.y() + 69.0 - snapshot.presentation.bounds.height() * 0.5,
            snapshot.presentation.bounds.width(),
            snapshot.presentation.bounds.height(),
        );
        ctx.stroke_rect(
            animated_bounds,
            theme.palette.warning.with_alpha(0.72),
            StrokeStyle::new(1.0),
        );
        for lane in 0..lanes {
            let t = lane as f32 / (lanes - 1) as f32;
            let x = bounds.x() + 36.0 + t * (bounds.width() - 72.0);
            let y = bounds.y() + 72.0 + (t - 0.5).sin() * 5.0;
            let radius = (snapshot.presentation.radius * (0.58 + 0.42 * t)).max(4.0);
            ctx.fill(
                Path::circle(Point::new(x, y), radius),
                snapshot
                    .presentation
                    .fill
                    .with_alpha(snapshot.presentation.opacity * (0.56 + 0.40 * t)),
            );
        }
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 14.0,
                bounds.max_y() - 44.0,
                bounds.width() - 28.0,
                18.0,
            ),
            format!(
                "Paint {:>8} requests · radius {:.1}",
                snapshot.invalidation_counts[2], snapshot.presentation.radius
            ),
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                bounds.x() + 14.0,
                bounds.max_y() - 24.0,
                bounds.width() - 28.0,
                18.0,
            ),
            format!(
                "Measure {:>6} requests · {:.0} × {:.0}",
                snapshot.invalidation_counts[3],
                snapshot.presentation.bounds.width(),
                snapshot.presentation.bounds.height()
            ),
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        let snapshot = self.state.snapshot();
        node.name = Some(ANIMATION_PAINT_INVALIDATION_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "paint requests {}, measure requests {}, radius {:.1}, width {:.1}",
            snapshot.invalidation_counts[2],
            snapshot.invalidation_counts[3],
            snapshot.presentation.radius,
            snapshot.presentation.bounds.width()
        )));
        ctx.push(node);
    }
}

struct AnimationDocumentEditorExample {
    state: AnimationDemoState,
    theme_reader: DevThemeReader,
}

impl AnimationDocumentEditorExample {
    fn new(state: AnimationDemoState, theme_reader: DevThemeReader) -> Self {
        Self {
            state,
            theme_reader,
        }
    }
}

impl Widget for AnimationDocumentEditorExample {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Pointer(pointer) = event else {
            return;
        };
        if pointer.kind != PointerEventKind::Down || !ctx.bounds().contains(pointer.position) {
            return;
        }
        let editor = self.state.editor_snapshot();
        let (timeline, _, _, _) = animation_editor_layout(ctx.bounds());
        if let Some((selection, _, _)) = animation_editor_keyframes(&editor, timeline)
            .into_iter()
            .find(|(_, _, hit)| hit.inflate(4.0, 4.0).contains(pointer.position))
        {
            self.state.select_keyframe(selection);
            request_animation_demo_refresh(ctx, false);
            ctx.set_handled();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(680.0, 376.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        let theme = (self.theme_reader)();
        let editor = self.state.editor_snapshot();
        paint_demo_surface(
            ctx,
            bounds,
            theme.palette.surface_raised,
            theme.palette.border,
        );

        draw_demo_label(
            ctx,
            theme,
            Rect::new(bounds.x() + 16.0, bounds.y() + 14.0, 230.0, 22.0),
            "AnimationDocument",
            DemoTextRole::CardTitle,
            theme.palette.text,
        );
        draw_demo_label(
            ctx,
            theme,
            Rect::new(bounds.max_x() - 176.0, bounds.y() + 14.0, 150.0, 22.0),
            format!("playhead {:.2}s", editor.playback.playhead),
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );

        let (timeline, preview, inspector, curve) = animation_editor_layout(bounds);

        self.paint_tracks(ctx, timeline, &editor, theme);
        self.paint_preview(ctx, preview, theme);
        self.paint_inspector(ctx, inspector, &editor, theme);
        self.paint_curve(ctx, curve, &editor, theme);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let editor = self.state.editor_snapshot();
        let track_count = editor
            .document
            .timeline
            .clips
            .first()
            .map(|clip| clip.tracks.len())
            .unwrap_or_default();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_EDITOR_SURFACE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "tracks {track_count}, selected keyframes {}, playhead {:.2}",
            editor.selection.keyframes.len(),
            editor.playback.playhead
        )));
        ctx.push(node);
    }
}

impl AnimationDocumentEditorExample {
    fn paint_tracks(
        &self,
        ctx: &mut PaintCtx,
        rect: Rect,
        editor: &AnimationEditorState,
        theme: DefaultTheme,
    ) {
        ctx.fill(Path::rounded_rect(rect, 6.0), theme.palette.field);
        draw_demo_label(
            ctx,
            theme,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, 160.0, 18.0),
            "Clip tracks",
            DemoTextRole::Metadata,
            theme.palette.text,
        );

        let Some(clip) = editor.document.timeline.clips.first() else {
            return;
        };
        let lane_area = Rect::new(
            rect.x() + 14.0,
            rect.y() + 34.0,
            rect.width() - 28.0,
            rect.height() - 48.0,
        );
        let time_area = Rect::new(
            lane_area.x() + 154.0,
            lane_area.y(),
            (lane_area.width() - 162.0).max(1.0),
            lane_area.height(),
        );
        let lane_height = 28.0;
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let y = lane_area.y() + track_index as f32 * (lane_height + 8.0);
            let lane = Rect::new(lane_area.x(), y, lane_area.width(), lane_height);
            ctx.fill(Path::rounded_rect(lane, 4.0), theme.palette.surface);
            draw_demo_label(
                ctx,
                theme,
                Rect::new(lane.x() + 8.0, lane.y() + 5.0, 138.0, 16.0),
                track.binding.property.path(),
                DemoTextRole::Metadata,
                theme.palette.text_muted,
            );
        }

        let playhead_x = time_area.x()
            + time_area.width()
                * (editor.playback.playhead / clip.duration.max(f64::EPSILON)).clamp(0.0, 1.0)
                    as f32;
        let mut playhead_path = Path::builder();
        playhead_path.move_to(Point::new(playhead_x, time_area.y()));
        playhead_path.line_to(Point::new(playhead_x, time_area.max_y()));
        ctx.stroke(
            playhead_path.build(),
            theme.palette.accent.with_alpha(0.72),
            StrokeStyle::new(1.0),
        );

        for (selection, keyframe, hit) in animation_editor_keyframes(editor, rect) {
            let selected = editor.selection.keyframes.contains(&selection);
            let center = Point::new(hit.x() + hit.width() * 0.5, hit.y() + hit.height() * 0.5);
            ctx.fill(
                Path::circle(center, if selected { 6.5 } else { 4.8 }),
                if selected {
                    theme.palette.warning
                } else {
                    theme.palette.accent
                },
            );
            if selected {
                draw_demo_label(
                    ctx,
                    theme,
                    Rect::new(center.x + 9.0, center.y - 9.0, 70.0, 18.0),
                    format!("{:.1}s", keyframe.time),
                    DemoTextRole::Metadata,
                    theme.palette.warning,
                );
            }
        }
    }

    fn paint_preview(&self, ctx: &mut PaintCtx, rect: Rect, theme: DefaultTheme) {
        ctx.fill(Path::rounded_rect(rect, 6.0), theme.palette.field);
        draw_demo_label(
            ctx,
            theme,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, rect.width() - 20.0, 18.0),
            "Linked preview",
            DemoTextRole::Metadata,
            theme.palette.text,
        );
        let presentation = self.state.snapshot().presentation;
        let rail = Rect::new(
            rect.x() + 22.0,
            rect.max_y() - 24.0,
            rect.width() - 44.0,
            4.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 2.0),
            theme.palette.border.with_alpha(0.72),
        );
        let center = Point::new(
            rect.x() + rect.width() * 0.5 + presentation.translation.x,
            rail.y() + 2.0,
        );
        ctx.fill(
            Path::circle(center, presentation.radius.min(18.0)),
            presentation.fill.with_alpha(presentation.opacity),
        );
    }

    fn paint_inspector(
        &self,
        ctx: &mut PaintCtx,
        rect: Rect,
        editor: &AnimationEditorState,
        theme: DefaultTheme,
    ) {
        ctx.fill(Path::rounded_rect(rect, 6.0), theme.palette.field);
        draw_demo_label(
            ctx,
            theme,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, rect.width() - 20.0, 18.0),
            "Selected keyframe",
            DemoTextRole::Metadata,
            theme.palette.text,
        );
        let detail = editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe_detail(editor, *selection))
            .unwrap_or_else(|| "No keyframe selected".to_string());
        draw_demo_label(
            ctx,
            theme,
            Rect::new(
                rect.x() + 10.0,
                rect.y() + 32.0,
                rect.width() - 20.0,
                rect.height() - 42.0,
            ),
            detail,
            DemoTextRole::Metadata,
            theme.palette.text_muted,
        );
    }

    fn paint_curve(
        &self,
        ctx: &mut PaintCtx,
        rect: Rect,
        editor: &AnimationEditorState,
        theme: DefaultTheme,
    ) {
        ctx.fill(Path::rounded_rect(rect, 6.0), theme.palette.field);
        draw_demo_label(
            ctx,
            theme,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, rect.width() - 20.0, 18.0),
            "Easing curve",
            DemoTextRole::Metadata,
            theme.palette.text,
        );

        let easing = editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe(editor, *selection))
            .map(|keyframe| keyframe.easing)
            .unwrap_or(Easing::Linear);
        let graph = Rect::new(
            rect.x() + 14.0,
            rect.y() + 34.0,
            rect.width() - 28.0,
            rect.height() - 48.0,
        );
        ctx.stroke_rect(graph, theme.palette.border, StrokeStyle::new(1.0));
        let mut path = Path::builder();
        for step in 0..=24 {
            let t = step as f32 / 24.0;
            let point = Point::new(
                graph.x() + graph.width() * t,
                graph.max_y() - graph.height() * easing.sample(t),
            );
            if step == 0 {
                path.move_to(point);
            } else {
                path.line_to(point);
            }
        }
        ctx.stroke(path.build(), theme.palette.warning, StrokeStyle::new(2.0));
    }
}

fn animation_editor_layout(bounds: Rect) -> (Rect, Rect, Rect, Rect) {
    let timeline = Rect::new(
        bounds.x() + 16.0,
        bounds.y() + 46.0,
        bounds.width() * 0.62,
        bounds.height() - 66.0,
    );
    let side_x = timeline.max_x() + 12.0;
    let side_width = bounds.max_x() - side_x - 16.0;
    let preview = Rect::new(side_x, timeline.y(), side_width, 88.0);
    let inspector = Rect::new(side_x, preview.max_y() + 10.0, side_width, 82.0);
    let curve = Rect::new(
        side_x,
        inspector.max_y() + 10.0,
        side_width,
        bounds.max_y() - inspector.max_y() - 26.0,
    );
    (timeline, preview, inspector, curve)
}

fn animation_editor_keyframes(
    editor: &AnimationEditorState,
    bounds: Rect,
) -> Vec<(KeyframeSelection, Keyframe<AnimationValue>, Rect)> {
    let Some(clip) = editor.document.timeline.clips.first() else {
        return Vec::new();
    };
    let lane_area = Rect::new(
        bounds.x() + 14.0,
        bounds.y() + 34.0,
        bounds.width() - 28.0,
        bounds.height() - 48.0,
    );
    let time_area = Rect::new(
        lane_area.x() + 154.0,
        lane_area.y(),
        (lane_area.width() - 162.0).max(1.0),
        lane_area.height(),
    );
    let lane_height = 28.0;
    let mut hits = Vec::new();
    for (track_index, track) in clip.tracks.iter().enumerate() {
        let y = lane_area.y() + track_index as f32 * (lane_height + 8.0);
        for (keyframe_index, keyframe) in track.keyframes.iter().enumerate() {
            let x = time_area.x()
                + time_area.width()
                    * (keyframe.time / clip.duration.max(f64::EPSILON)).clamp(0.0, 1.0) as f32;
            hits.push((
                KeyframeSelection {
                    clip_index: 0,
                    track_index,
                    keyframe_index,
                },
                *keyframe,
                Rect::new(x - 7.0, y + (lane_height * 0.5) - 7.0, 14.0, 14.0),
            ));
        }
    }
    hits
}

fn selected_keyframe(
    editor: &AnimationEditorState,
    selection: KeyframeSelection,
) -> Option<Keyframe<AnimationValue>> {
    editor
        .document
        .timeline
        .clips
        .get(selection.clip_index)
        .and_then(|clip| clip.tracks.get(selection.track_index))
        .and_then(|track| track.keyframes.get(selection.keyframe_index))
        .copied()
}

fn selected_keyframe_detail(
    editor: &AnimationEditorState,
    selection: KeyframeSelection,
) -> Option<String> {
    let clip = editor.document.timeline.clips.get(selection.clip_index)?;
    let track = clip.tracks.get(selection.track_index)?;
    let keyframe = track.keyframes.get(selection.keyframe_index)?;
    Some(format!(
        "{} · {:.2}s · {:?}",
        track.binding.property.path(),
        keyframe.time,
        keyframe.easing
    ))
}

fn timeline_playback_example_timeline() -> Timeline {
    let target = AnimationTargetId::new(TIMELINE_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.8).with_clip(
        Clip::new("timeline-playback", 0.0, 1.8)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.42)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Scalar(0.42)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-30.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Vector(Vector::new(30.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Vector(Vector::new(-30.0, 0.0))),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.9,
                        AnimationValue::Color(Color::rgba(0.10, 0.76, 0.52, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.8,
                        AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(TIMELINE_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(14.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Scalar(25.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Scalar(14.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Bounds)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Rect(Rect::new(0.0, 0.0, 64.0, 34.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Rect(Rect::new(0.0, 0.0, 96.0, 48.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Rect(Rect::new(0.0, 0.0, 64.0, 34.0))),
                ]),
            ),
    )
}

fn paint_demo_surface(ctx: &mut PaintCtx, bounds: Rect, fill: Color, border: Color) {
    ctx.fill(Path::rounded_rect(bounds, 8.0), fill);
    ctx.stroke(
        Path::rounded_rect(bounds, 8.0),
        border,
        StrokeStyle::new(1.0),
    );
}

fn paint_value_bar(ctx: &mut PaintCtx, theme: DefaultTheme, rect: Rect, label: &str, value: f32) {
    draw_demo_label(
        ctx,
        theme,
        Rect::new(rect.x(), rect.y() + 4.0, 116.0, 18.0),
        label,
        DemoTextRole::Metadata,
        theme.palette.text_muted,
    );
    let rail = Rect::new(
        rect.x() + 120.0,
        rect.y() + 9.0,
        (rect.width() - 164.0).max(44.0),
        6.0,
    );
    ctx.fill(
        Path::rounded_rect(rail, 3.0),
        theme.palette.border.with_alpha(0.68),
    );
    ctx.fill(
        Path::rounded_rect(
            Rect::new(
                rail.x(),
                rail.y(),
                rail.width() * value.clamp(0.0, 1.0),
                rail.height(),
            ),
            3.0,
        ),
        theme.palette.accent,
    );
    draw_demo_label(
        ctx,
        theme,
        Rect::new(rail.max_x() + 8.0, rect.y() + 4.0, 42.0, 18.0),
        format!("{value:.2}"),
        DemoTextRole::Metadata,
        theme.palette.text,
    );
}

fn draw_demo_label(
    ctx: &mut PaintCtx,
    theme: DefaultTheme,
    rect: Rect,
    text: impl Into<String>,
    role: DemoTextRole,
    color: Color,
) {
    let style = demo_text_style(theme, role, color);
    let text = text.into();
    paint_single_line_aligned_text(ctx, rect, &text, &style, style.line_height, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_transport_pause_step_seek_and_restart_stay_synchronized() {
        let state = AnimationDemoState::new();
        state.pause();
        let paused = state.snapshot();
        assert!(!paused.playback.playing);

        state.step(0.1);
        let stepped = state.snapshot();
        assert!(!stepped.playback.playing);
        assert!(stepped.playback.playhead > paused.playback.playhead);
        assert_eq!(
            state.editor_snapshot().playback.playhead,
            stepped.playback.playhead
        );

        state.seek(1.25);
        assert!((state.snapshot().playback.playhead - 1.25).abs() < 1e-6);
        state.restart();
        let restarted = state.snapshot();
        assert!(restarted.playback.playing);
        assert!(restarted.playback.playhead.abs() < 1e-6);
        assert_eq!(restarted.invalidation_counts, [0; 4]);
    }

    #[test]
    fn zero_motion_scale_settles_all_retargetable_primitives() {
        let state = AnimationDemoState::new();
        state.pause();
        state.set_motion_scale(0.0);
        state.retarget();
        let settled_low = state.snapshot();
        assert_eq!(settled_low.primitive_target, 0.0);
        assert!(settled_low.transition_value.abs() < 1e-6);
        assert!(settled_low.animated_value.abs() < 1e-6);
        assert!(settled_low.motion_scalar.abs() < 1e-6);
        assert!(settled_low.spring_value.abs() < 1e-6);

        state.retarget();
        let settled_high = state.snapshot();
        assert_eq!(settled_high.primitive_target, 1.0);
        assert!((settled_high.transition_value - 1.0).abs() < 1e-6);
        assert!((settled_high.animated_value - 1.0).abs() < 1e-6);
        assert!((settled_high.motion_scalar - 1.0).abs() < 1e-6);
        assert!((settled_high.spring_value - 1.0).abs() < 1e-6);
    }

    #[test]
    fn shared_timeline_reports_transform_effect_paint_and_measure_invalidations() {
        let state = AnimationDemoState::new();
        assert!(state.advance(0.1));
        let counts = state.snapshot().invalidation_counts;
        assert!(
            counts.into_iter().all(|count| count > 0),
            "expected every invalidation path to advance, got {counts:?}"
        );
    }

    #[test]
    fn editor_easing_and_keyframe_changes_support_undo_redo() {
        let state = AnimationDemoState::new();
        assert_eq!(state.editor_easing_index(), 3);

        state.set_editor_easing(2);
        assert_eq!(state.editor_easing_index(), 2);
        assert!(state.can_undo_editor());
        state.undo_editor();
        assert_eq!(state.editor_easing_index(), 3);
        assert!(state.can_redo_editor());
        state.redo_editor();
        assert_eq!(state.editor_easing_index(), 2);

        state.seek(0.45);
        let before = state.editor_snapshot().document.timeline.clips[0].tracks[0]
            .keyframes
            .len();
        state.add_keyframe();
        let after = state.editor_snapshot().document.timeline.clips[0].tracks[0]
            .keyframes
            .len();
        assert_eq!(after, before + 1);
        state.undo_editor();
        assert_eq!(
            state.editor_snapshot().document.timeline.clips[0].tracks[0]
                .keyframes
                .len(),
            before
        );
    }
}
