use sui::prelude::*;
use sui::{
    AnimationDocument, AnimationSpec, LoopMode, MotionPreference, Point, SpringSpec, Vector,
    reset_motion_settings, set_app_motion_preference, set_motion_time_scale,
};

use super::choreography::TaskList;
use super::curves::{CURVES, Curve, Shuttle};
use super::interruption::{FlingPuck, RetargetPair};
use super::policy_summary;
use super::studio::{StudioState, bezier_points};
use super::under_the_hood::FrameIntervals;

/// Restores the thread's default motion settings when dropped.
struct ResetMotion;

impl Drop for ResetMotion {
    fn drop(&mut self) {
        reset_motion_settings();
    }
}

#[test]
fn shuttle_sets_off_at_once_then_rests_between_legs() {
    let _reset = ResetMotion;
    let spec = AnimationSpec::tween(0.2, Easing::Linear);
    let mut shuttle = Shuttle::new();

    shuttle.advance(10.0, spec, 0.5);
    assert!(shuttle.heading_up());
    shuttle.advance(10.1, spec, 0.5);
    assert!((shuttle.value() - 0.5).abs() < 1e-5);

    shuttle.advance(10.2, spec, 0.5);
    assert_eq!(shuttle.value(), 1.0);
    shuttle.advance(10.5, spec, 0.5);
    assert!(shuttle.heading_up(), "still resting at the top");
    shuttle.advance(10.71, spec, 0.5);
    assert!(!shuttle.heading_up(), "heads back after resting");
}

#[test]
fn shuttle_turns_mid_flight_without_jumping() {
    let spec = AnimationSpec::tween(0.4, Easing::EaseInOut);
    let mut shuttle = Shuttle::new();
    shuttle.advance(0.0, spec, 0.5);
    shuttle.advance(0.2, spec, 0.5);
    let before = shuttle.value();

    shuttle.turn(0.2, spec);

    assert_eq!(shuttle.value(), before);
    assert!(!shuttle.heading_up());
    assert_eq!(shuttle.leg_elapsed(), 0.0);
}

#[test]
fn token_curves_follow_the_theme_motion_tokens() {
    let mut motion = DefaultTheme::default().motion;
    motion.duration_slow = 0.5;
    let standard = CURVES
        .iter()
        .copied()
        .find(|curve| curve.name() == "Standard")
        .expect("the standard curve");

    assert_eq!(
        standard.spec(&motion),
        AnimationSpec::tween(0.5, motion.easing_standard)
    );
    assert!(CURVES.iter().any(|curve| matches!(
        curve,
        Curve::Spring { spec, .. } if *spec == SpringSpec::BOUNCY
    )));
}

fn slope(pair: &RetargetPair, lane: fn((f32, f32)) -> f32, from: f64, to: f64) -> f32 {
    (lane(pair.values_at(to)) - lane(pair.values_at(from))) / (to - from) as f32
}

#[test]
fn momentum_lane_keeps_speed_where_the_restart_lane_stops() {
    let _reset = ResetMotion;
    let mut pair = RetargetPair::new();
    pair.advance(0.0);
    // Halfway through the first leg, both lanes rise at the same speed.
    pair.advance(0.2);
    let momentum_before = slope(&pair, |values| values.1, 0.2, 0.201);
    let restart_before = slope(&pair, |values| values.0, 0.2, 0.201);
    assert!((momentum_before - restart_before).abs() < 0.05);

    pair.retarget(0.2);

    let momentum_after = slope(&pair, |values| values.1, 0.2, 0.201);
    let restart_after = slope(&pair, |values| values.0, 0.2, 0.201);
    assert!(
        (momentum_after - momentum_before).abs() < 0.1 * momentum_before,
        "momentum keeps its speed: {momentum_before} then {momentum_after}"
    );
    assert!(
        restart_after.abs() < 0.1 * restart_before,
        "restarting stops dead: {restart_before} then {restart_after}"
    );
}

#[test]
fn a_flung_puck_travels_then_settles_home() {
    let _reset = ResetMotion;
    let mut puck = FlingPuck::new();
    puck.grab(0.0, Point::new(0.0, 0.0));
    puck.drag(0.02, Point::new(10.0, 0.0), Vector::new(10.0, 0.0));
    puck.release(SpringSpec::SNAPPY);
    assert!(puck.is_springing());

    puck.step(0.02);
    assert!(puck.offset().x > 10.0, "the fling carries it further first");
    let mut elapsed = 0.0;
    while puck.step(1.0 / 60.0) {
        elapsed += 1.0 / 60.0;
        assert!(elapsed < 5.0, "the puck settles");
    }
    assert_eq!(puck.offset(), Vector::ZERO);
}

#[test]
fn a_released_puck_jumps_home_with_reduced_motion() {
    let _reset = ResetMotion;
    set_app_motion_preference(Some(MotionPreference::Reduced));
    let mut puck = FlingPuck::new();
    puck.grab(0.0, Point::new(0.0, 0.0));
    puck.drag(0.02, Point::new(40.0, 0.0), Vector::new(40.0, 0.0));

    puck.release(SpringSpec::BOUNCY);

    assert!(!puck.is_springing());
    assert_eq!(puck.offset(), Vector::ZERO);
}

#[test]
fn policy_summary_describes_source_effect_and_speed() {
    let _reset = ResetMotion;
    assert_eq!(
        policy_summary(),
        "Following the system setting (full): every transition plays."
    );

    set_app_motion_preference(Some(MotionPreference::Reduced));
    set_motion_time_scale(0.25);
    assert_eq!(
        policy_summary(),
        "Overriding the system setting (full): fades play, movement jumps, at 0.25× speed."
    );
}

#[test]
fn quadratic_easings_convert_to_exact_bezier_handles() {
    for easing in [Easing::Linear, Easing::EaseIn, Easing::EaseOut] {
        let [x1, y1, x2, y2] = bezier_points(easing);
        let bezier = Easing::CubicBezier { x1, y1, x2, y2 };
        for step in 0..=10 {
            let t = step as f32 / 10.0;
            assert!(
                (bezier.sample(t) - easing.sample(t)).abs() < 1e-3,
                "{easing:?} at {t}"
            );
        }
    }
}

#[test]
fn studio_moves_keyframes_with_snapping_and_undo() {
    let studio = StudioState::new();
    let selection = studio.selected().expect("a keyframe starts selected");

    assert!(studio.move_keyframe(selection, 0.74));
    let moved = studio.selected_keyframe().expect("still selected").1.time;
    assert!(
        (moved - 18.0 / 24.0).abs() < 1e-9,
        "snapped to 1/24 s: {moved}"
    );

    assert!(studio.nudge_selected(1.0));
    let nudged = studio.selected_keyframe().expect("still selected").1.time;
    assert!((nudged - 19.0 / 24.0).abs() < 1e-9);

    assert!(studio.undo());
    assert!(studio.can_redo());
    assert!(studio.redo());
}

#[test]
fn studio_easing_menu_tracks_the_selected_keyframe() {
    let studio = StudioState::new();
    assert_eq!(studio.easing_choice(), Some(5), "starts on Emphasized");

    assert!(studio.set_selected_easing(Easing::EaseOut));
    assert_eq!(studio.easing_choice(), Some(2));

    assert!(studio.set_selected_easing(Easing::CubicBezier {
        x1: 0.1,
        y1: 0.9,
        x2: 0.2,
        y2: 1.0,
    }));
    assert_eq!(studio.easing_choice(), Some(6), "anything else is Custom");
}

#[test]
fn studio_adds_keyframes_at_the_playhead_and_selects_them() {
    let studio = StudioState::new();
    studio.pause();
    studio.seek(0.5);

    assert!(studio.add_keyframe());

    let (path, keyframe) = studio.selected_keyframe().expect("the new keyframe");
    assert_eq!(path, "layer.translation");
    assert!((keyframe.time - 0.5).abs() < 1e-9);
    assert!(studio.remove_selected());
    assert!(studio.selected().is_none());
}

#[test]
fn studio_pause_and_loop_control_playback() {
    let studio = StudioState::new();
    assert!(studio.is_playing());
    assert_eq!(studio.loop_mode(), LoopMode::Repeat);

    studio.set_loop_mode(LoopMode::Once);
    studio.seek(1.9);
    assert!(!studio.advance(0.5), "a single play stops at the end");
    assert!((studio.playhead() - 2.0).abs() < 1e-9);

    studio.play();
    assert!(studio.playhead() < 1e-9, "playing again starts over");
    studio.pause();
    assert!(!studio.advance(0.5));
}

#[test]
fn studio_document_round_trips_through_its_text_format() {
    let studio = StudioState::new();
    let text = studio.document_text();

    let parsed = AnimationDocument::from_document_format(&text).expect("the export parses");

    assert_eq!(parsed.to_document_format(), text);
    assert_eq!(parsed.timeline.clips[0].tracks.len(), 5);
}

#[test]
fn frame_intervals_ignore_the_first_frame_and_report_extremes() {
    let mut intervals = FrameIntervals::default();
    assert_eq!(intervals.summary(), "waiting for frames");

    intervals.record(0.0);
    intervals.record(0.008);
    intervals.record(0.012);

    assert!((intervals.average().expect("an average") - 10.0).abs() < 1e-9);
    assert!((intervals.worst().expect("a worst") - 12.0).abs() < 1e-9);
    assert!((intervals.median().expect("a median") - 12.0).abs() < 1e-9);
    assert!(intervals.summary().starts_with("typically 12.0 ms (83 Hz)"));
}

#[test]
fn studio_ping_pong_turns_back_at_the_end() {
    let studio = StudioState::new();
    studio.set_loop_mode(LoopMode::PingPong);
    studio.seek(1.9);
    studio.play();
    assert!(studio.advance(0.3), "ping-pong keeps playing");
    assert!(
        (studio.playhead() - 1.8).abs() < 1e-9,
        "{}",
        studio.playhead()
    );

    studio.set_loop_delay(0.5);
    assert_eq!(studio.loop_delay(), 0.5);
}

#[test]
fn studio_reports_the_markers_it_passes() {
    let studio = StudioState::new();
    assert_eq!(studio.markers().len(), 2);
    studio.seek(0.9);
    studio.advance(0.2);
    assert_eq!(studio.last_marker().as_deref(), Some("Arrive"));

    studio.seek(0.5);
    assert!(studio.add_marker());
    assert_eq!(studio.markers().len(), 3);
    assert!(studio.summary().contains("3 markers"));
}

#[test]
fn task_list_edits_keep_keys_unique() {
    let list = TaskList::new();
    let ids = |list: &TaskList| {
        list.items
            .get()
            .iter()
            .map(|task| task.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&list), vec![0, 1, 2, 3]);

    list.add(3);
    assert_eq!(ids(&list), vec![4, 5, 6, 0, 1, 2, 3]);
    list.remove_one();
    assert_eq!(ids(&list), vec![4, 6, 0, 1, 2, 3]);
    list.shuffle();
    assert_eq!(ids(&list), vec![3, 2, 1, 0, 6, 4]);
    list.shuffle();
    assert_eq!(ids(&list), vec![2, 1, 0, 6, 4, 3]);
    list.remove(0);
    assert_eq!(ids(&list), vec![2, 1, 6, 4, 3]);
}
