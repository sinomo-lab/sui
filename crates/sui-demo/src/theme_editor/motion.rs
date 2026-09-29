//! A looping motion sample, so motion speed is visible without clicking: a
//! switch that toggles with the theme's toggle timing and a panel bar that
//! slides with its slow surface timing.

use sui::prelude::*;
use sui::{TimerToken, Transition, WakeEvent, paint_text_line};

use crate::app::{DemoTextRole, demo_text_style};

const PAUSE_SECONDS: f64 = 0.9;
const ROW_HEIGHT: f32 = 28.0;
const TRACK_WIDTH: f32 = 180.0;

pub(super) struct MotionSample {
    theme: DefaultTheme,
    on: bool,
    toggle: f32,
    panel: f32,
    transitions: Option<(Transition<f32>, Transition<f32>)>,
    pause: Option<TimerToken>,
    waiting_for_frame: bool,
}

impl MotionSample {
    pub(super) fn new(theme: DefaultTheme) -> Self {
        Self {
            theme,
            on: false,
            toggle: 0.0,
            panel: 0.0,
            transitions: None,
            pause: None,
            waiting_for_frame: false,
        }
    }

    fn start(&mut self, time: f64) {
        self.on = !self.on;
        let target = if self.on { 1.0 } else { 0.0 };
        let motion = self.theme.motion;
        self.transitions = Some((
            Transition::new(
                self.toggle,
                target,
                time,
                motion.toggle_duration(),
                motion.toggle_easing(),
            ),
            Transition::new(
                self.panel,
                target,
                time,
                f64::from(motion.duration_slow),
                motion.easing_decelerate,
            ),
        ));
    }

    fn milliseconds(seconds: f32) -> String {
        format!("{:.0} ms", seconds * 1000.0)
    }
}

impl Widget for MotionSample {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                self.waiting_for_frame = false;
                if self.transitions.is_none() && self.pause.is_none() {
                    self.start(*time);
                }
                let Some((toggle, panel)) = self.transitions else {
                    return;
                };
                self.toggle = toggle.sample(*time);
                self.panel = panel.sample(*time);
                if toggle.is_complete(*time) && panel.is_complete(*time) {
                    self.transitions = None;
                    self.pause = Some(ctx.schedule_timer_after(PAUSE_SECONDS));
                } else {
                    ctx.request_animation_frame();
                    self.waiting_for_frame = true;
                }
                ctx.request_paint();
            }
            Event::Wake(WakeEvent::Timer { token, .. }) if self.pause == Some(*token) => {
                self.pause = None;
                ctx.request_animation_frame();
                self.waiting_for_frame = true;
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.transitions.is_none() && self.pause.is_none() && !self.waiting_for_frame {
            ctx.request_animation_frame();
            self.waiting_for_frame = true;
        }
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            TRACK_WIDTH + 200.0
        };
        constraints.clamp(Size::new(width, ROW_HEIGHT * 2.0 + 8.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.theme;
        let palette = theme.palette;
        let bounds = ctx.bounds();
        let label_style = demo_text_style(theme, DemoTextRole::Supporting, palette.text_muted);

        let switch_row = Rect::new(bounds.x(), bounds.y(), bounds.width(), ROW_HEIGHT);
        let track = Rect::new(switch_row.x(), switch_row.y() + 4.0, 40.0, 20.0);
        let fill = Color::interpolate(palette.control_active, palette.accent, self.toggle);
        ctx.fill_rrect(track, [10.0; 4], fill);
        let knob_x = track.x() + 10.0 + (track.width() - 20.0) * self.toggle;
        ctx.fill(
            Path::circle(Point::new(knob_x, track.y() + 10.0), 8.0),
            palette.surface_raised,
        );
        paint_text_line(
            ctx,
            Rect::new(
                track.max_x() + 12.0,
                switch_row.y(),
                (switch_row.width() - track.width() - 12.0).max(0.0),
                ROW_HEIGHT,
            ),
            &format!(
                "Toggle, {}",
                Self::milliseconds(theme.motion.duration_normal)
            ),
            &label_style,
            TextAlign::Start,
        );

        let panel_row = Rect::new(
            bounds.x(),
            switch_row.max_y() + 8.0,
            bounds.width(),
            ROW_HEIGHT,
        );
        let bar = Rect::new(panel_row.x(), panel_row.y() + 9.0, TRACK_WIDTH, 10.0);
        ctx.fill_rrect(bar, [5.0; 4], palette.control);
        ctx.fill_rrect(
            Rect::new(
                bar.x(),
                bar.y(),
                (bar.width() * self.panel).max(bar.height()),
                bar.height(),
            ),
            [5.0; 4],
            palette.accent,
        );
        paint_text_line(
            ctx,
            Rect::new(
                bar.max_x() + 12.0,
                panel_row.y(),
                (panel_row.width() - bar.width() - 12.0).max(0.0),
                ROW_HEIGHT,
            ),
            &format!("Panel, {}", Self::milliseconds(theme.motion.duration_slow)),
            &label_style,
            TextAlign::Start,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            sui::SemanticsNode::new(ctx.widget_id(), sui::SemanticsRole::Image, ctx.bounds());
        node.name = Some("Motion sample".to_string());
        node.description = Some(format!(
            "Toggles take {} and panels {}",
            Self::milliseconds(self.theme.motion.duration_normal),
            Self::milliseconds(self.theme.motion.duration_slow)
        ));
        ctx.push(node);
    }
}
