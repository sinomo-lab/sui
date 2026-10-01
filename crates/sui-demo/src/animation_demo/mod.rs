//! The animation demo: one scroll that shows how SUI moves things. Curves and
//! springs come first, then interruption, the built-in widgets, content that
//! comes and goes, a timeline editor, and what motion costs the renderer.
//!
//! The motion controls at the top set the app-wide motion policy, so every
//! transition in the app, not just on this page, follows them.

mod choreography;
mod curves;
mod interruption;
mod studio;
#[cfg(test)]
mod tests;
mod under_the_hood;
mod widget_motion;

use std::{cell::Cell, rc::Rc};

use sui::prelude::*;
use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, MotionPreference,
    app_motion_preference, motion_policy, motion_time_scale, paint_text_line,
    set_app_motion_preference, set_motion_time_scale, system_motion_preference,
};

use crate::app::{
    DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style, demo_text_style_when,
    dev_theme_color,
};
use crate::demo_support::NamedSection;

pub(crate) const ANIMATION_DEMO_TAB_LABEL: &str = "Animation";
pub(crate) const ANIMATION_DEMO_SCROLL_NAME: &str = "Animation demo scroll";
pub(crate) const ANIMATION_DEMO_TITLE: &str = "Animation";
pub(crate) const MOTION_PREFERENCE_NAME: &str = "Motion preference";
pub(crate) const MOTION_SPEED_NAME: &str = "Animation speed";
pub(crate) const SHOW_TRACES_LABEL: &str = "Show traces";
pub(crate) const MOTION_STATUS_NAME: &str = "Motion policy status";
pub(crate) const CURVES_SECTION_NAME: &str = "Easing and springs";
pub(crate) const INTERRUPTION_SECTION_NAME: &str = "Interruptible motion";
pub(crate) const WIDGET_MOTION_SECTION_NAME: &str = "Built-in widget motion";
pub(crate) const CHOREOGRAPHY_SECTION_NAME: &str = "Choreography";
pub(crate) const STUDIO_SECTION_NAME: &str = "Timeline studio";
pub(crate) const UNDER_THE_HOOD_SECTION_NAME: &str = "Under the hood";

#[cfg(test)]
pub(crate) use choreography::{
    ADD_THREE_LABEL, DETAILS_TOGGLE_LABEL, KEYED_LIST_NAME, SHUFFLE_LABEL, STAGGER_NAME,
};
#[cfg(test)]
pub(crate) use interruption::{FLING_PAD_NAME, INTERRUPTION_COMPARISON_NAME};
#[cfg(test)]
pub(crate) use studio::{STUDIO_EDITOR_NAME, STUDIO_PAUSE_LABEL, STUDIO_PLAY_LABEL};
#[cfg(test)]
pub(crate) use under_the_hood::{FRAME_PACING_NAME, LAYER_STAGE_NAME, REPAINT_STAGE_NAME};

const PAGE_PADDING: f32 = 28.0;
const SECTION_GAP: f32 = 40.0;

/// Motion preference choices in the header, in segment order. `None` follows
/// the operating system.
const PREFERENCE_CHOICES: [(Option<MotionPreference>, &str); 4] = [
    (None, "System"),
    (Some(MotionPreference::Full), "Full"),
    (Some(MotionPreference::Reduced), "Reduced"),
    (Some(MotionPreference::Off), "Off"),
];

/// Playback speeds in the header, in segment order.
const SPEED_CHOICES: [(f32, &str); 4] =
    [(1.0, "1×"), (0.5, "0.5×"), (0.25, "0.25×"), (0.1, "0.1×")];

/// State shared by the page's custom widgets.
#[derive(Clone)]
pub(super) struct MotionDemoState {
    show_traces: Rc<Cell<bool>>,
}

impl MotionDemoState {
    pub(super) fn new() -> Self {
        Self {
            show_traces: Rc::new(Cell::new(true)),
        }
    }

    /// Whether moving pucks leave a fading trail of where they were.
    pub(super) fn show_traces(&self) -> bool {
        self.show_traces.get()
    }

    fn set_show_traces(&self, show: bool) {
        self.show_traces.set(show);
    }
}

pub(crate) fn build_animation_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    let state = MotionDemoState::new();
    let page = Stack::vertical()
        .spacing(SECTION_GAP)
        .alignment(Alignment::Stretch)
        .with_child(header(state.clone(), Rc::clone(&theme_reader)))
        .with_child(section(
            &theme_reader,
            CURVES_SECTION_NAME,
            "Easing and springs",
            "The theme's curves next to spring presets. Each card loops; click one to send it back mid-flight.",
            curves::gallery(state.clone(), Rc::clone(&theme_reader)),
        ))
        .with_child(section(
            &theme_reader,
            INTERRUPTION_SECTION_NAME,
            "Interruptible motion",
            "Retargeting keeps momentum: the value turns around smoothly instead of stopping dead. Springs carry the speed of a fling.",
            interruption::section(state.clone(), Rc::clone(&theme_reader)),
        ))
        .with_child(section(
            &theme_reader,
            WIDGET_MOTION_SECTION_NAME,
            "Built-in widgets",
            "Real widgets using the theme's motion tokens. Try them at a slower speed or with reduced motion.",
            widget_motion::gallery(Rc::clone(&theme_reader)),
        ))
        .with_child(section(
            &theme_reader,
            CHOREOGRAPHY_SECTION_NAME,
            "Choreography",
            "Content that comes and goes: new items cascade in, removed ones leave while the gap closes, and reordered ones glide. Leaving content stops taking input at once.",
            choreography::gallery(Rc::clone(&theme_reader)),
        ))
        .with_child(section(
            &theme_reader,
            STUDIO_SECTION_NAME,
            "Timeline studio",
            "Drag keyframes, scrub the ruler, and shape the selected keyframe's curve. Edits undo, and the document copies as text.",
            studio::section(Rc::clone(&theme_reader)),
        ))
        .with_child(section(
            &theme_reader,
            UNDER_THE_HOOD_SECTION_NAME,
            "Under the hood",
            "Moving or scaling a retained layer asks only for a transform update, which the renderer applies without repainting; painting at a new offset repaints every frame. On a vsync display, animation frames follow its refresh.",
            under_the_hood::section(Rc::clone(&theme_reader)),
        ));

    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(PAGE_PADDING, page)).name(ANIMATION_DEMO_SCROLL_NAME),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn header(state: MotionDemoState, theme_reader: DevThemeReader) -> impl Widget {
    let traces_state = state;
    let controls = Flex::horizontal()
        .gap(28.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::End)
        .with_child(labelled(
            &theme_reader,
            "Motion, whole app",
            SegmentedControl::new(MOTION_PREFERENCE_NAME)
                .segments(PREFERENCE_CHOICES.map(|(_, label)| label))
                .selected_when(|| {
                    let current = app_motion_preference();
                    PREFERENCE_CHOICES
                        .iter()
                        .position(|(preference, _)| *preference == current)
                })
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_change_with_ctx(|ctx, index, _| {
                    set_app_motion_preference(PREFERENCE_CHOICES[index].0);
                    refresh_page(ctx);
                }),
        ))
        .with_child(labelled(
            &theme_reader,
            "Speed",
            SegmentedControl::new(MOTION_SPEED_NAME)
                .segments(SPEED_CHOICES.map(|(_, label)| label))
                .selected_when(|| Some(speed_index(motion_time_scale())))
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_change_with_ctx(|ctx, index, _| {
                    set_motion_time_scale(SPEED_CHOICES[index].0);
                    refresh_page(ctx);
                }),
        ))
        .with_child(
            Switch::new(SHOW_TRACES_LABEL)
                .checked(true)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_change(move |on| traces_state.set_show_traces(on)),
        );

    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new(ANIMATION_DEMO_TITLE).text_style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::PageTitle,
                |theme| theme.palette.text,
            )),
        )
        .with_child(
            Label::new(
                "Curves, springs, and timelines, and how SUI keeps them smooth, interruptible, and respectful of reduced motion.",
            )
            .text_style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::Supporting,
                |theme| theme.palette.text_muted,
            )),
        )
        .with_child(SizedBox::new().height(8.0))
        .with_child(controls)
        .with_child(NamedSection::new(
            MOTION_STATUS_NAME,
            Label::new(policy_summary()).text_when(policy_summary).text_style_when(demo_text_style_when(
                &theme_reader,
                DemoTextRole::Metadata,
                |theme| theme.palette.text_muted,
            )),
        ))
}

/// A one-line description of the motion policy in effect.
pub(super) fn policy_summary() -> String {
    let policy = motion_policy();
    let source = match app_motion_preference() {
        None => format!(
            "Following the system setting ({})",
            system_motion_preference().label().to_lowercase()
        ),
        Some(_) => format!(
            "Overriding the system setting ({})",
            system_motion_preference().label().to_lowercase()
        ),
    };
    let effect = match policy.preference {
        MotionPreference::Full => "every transition plays",
        MotionPreference::Reduced => "fades play, movement jumps",
        MotionPreference::Off => "transitions finish immediately",
    };
    let speed = if (policy.time_scale() - 1.0).abs() < f32::EPSILON {
        String::new()
    } else {
        format!(", at {}× speed", policy.time_scale())
    };
    format!("{source}: {effect}{speed}.")
}

fn speed_index(time_scale: f32) -> usize {
    SPEED_CHOICES
        .iter()
        .enumerate()
        .min_by(|(_, (a, _)), (_, (b, _))| {
            (a - time_scale).abs().total_cmp(&(b - time_scale).abs())
        })
        .map_or(0, |(index, _)| index)
}

fn labelled<W>(theme_reader: &DevThemeReader, label: &str, control: W) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Start)
        .with_child(Label::new(label).text_style_when(demo_text_style_when(
            theme_reader,
            DemoTextRole::Metadata,
            |theme| theme.palette.text_muted,
        )))
        .with_child(control)
}

fn section<W>(
    theme_reader: &DevThemeReader,
    name: &str,
    title: &str,
    summary: &str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    NamedSection::new(
        name,
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(Label::new(title).text_style_when(demo_text_style_when(
                theme_reader,
                DemoTextRole::SectionTitle,
                |theme| theme.palette.text,
            )))
            .with_child(Label::new(summary).text_style_when(demo_text_style_when(
                theme_reader,
                DemoTextRole::Supporting,
                |theme| theme.palette.text_muted,
            )))
            .with_child(SizedBox::new().height(6.0))
            .with_child(body),
    )
}

/// Repaint and re-measure the window after a page-wide setting changes, so
/// labels that describe it update and paused animations can restart.
pub(super) fn refresh_page(ctx: &mut EventCtx) {
    for kind in [
        InvalidationKind::Measure,
        InvalidationKind::Paint,
        InvalidationKind::Semantics,
    ] {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            kind,
        ));
    }
}

/// A size that fills the available width (or a default width when
/// unbounded) at `height`.
pub(super) fn fill_width(constraints: Constraints, height: f32) -> Size {
    let width = if constraints.max.width.is_finite() {
        constraints.max.width
    } else {
        640.0
    };
    constraints.clamp(Size::new(width, height))
}

/// Seconds as whole milliseconds, for labels.
pub(super) fn millis(seconds: f64) -> String {
    format!("{:.0} ms", seconds * 1000.0)
}

/// Paint a demo card's background: a raised, rounded, bordered surface.
pub(super) fn paint_card(ctx: &mut PaintCtx, bounds: Rect, theme: DefaultTheme) {
    let card = Path::rounded_rect(bounds, 10.0);
    ctx.fill(card.clone(), theme.palette.surface_raised);
    ctx.stroke(card, theme.palette.border, StrokeStyle::new(1.0));
}

/// Paint one line of text in a demo text role.
pub(super) fn draw_text(
    ctx: &mut PaintCtx,
    theme: DefaultTheme,
    rect: Rect,
    text: &str,
    role: DemoTextRole,
    color: Color,
) {
    let style = demo_text_style(theme, role, color);
    paint_text_line(ctx, rect, text, &style, TextAlign::Start);
}

/// Paint `points` as a connected line.
pub(super) fn stroke_polyline(
    ctx: &mut PaintCtx,
    points: impl IntoIterator<Item = Point>,
    color: Color,
    width: f32,
) {
    let mut path = Path::builder();
    let mut started = false;
    for point in points {
        if started {
            path.line_to(point);
        } else {
            path.move_to(point);
            started = true;
        }
    }
    if started {
        ctx.stroke(path.build(), color, StrokeStyle::new(width));
    }
}

/// Paint a horizontal hairline.
pub(super) fn hairline(ctx: &mut PaintCtx, from: Point, to: Point, color: Color) {
    stroke_polyline(ctx, [from, to], color, 1.0);
}
