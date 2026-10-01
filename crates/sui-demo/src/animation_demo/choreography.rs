//! Choreography: content that comes and goes. A keyed list animates its
//! items in, out, and into new places; a stagger starts a group one after
//! another; a presence shows and hides a panel; tabs and notifications
//! animate as they close and arrive.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use sui::prelude::*;
use sui::{
    AnimateCtx, BrowserTabBar, NotificationCenter, NotificationHost, PointerEventKind,
    SemanticRegion, SemanticsNode, SemanticsRole, SemanticsValue, TransientNotification,
};

use super::widget_motion::card;
use super::{fill_width, millis, refresh_page};
use crate::app::{DevThemeReader, clone_dev_theme_reader};

pub(crate) const KEYED_LIST_NAME: &str = "Animated task list";
pub(crate) const ADD_TASK_LABEL: &str = "Add";
pub(crate) const ADD_THREE_LABEL: &str = "Add three";
pub(crate) const REMOVE_TASK_LABEL: &str = "Remove";
pub(crate) const SHUFFLE_LABEL: &str = "Shuffle";
pub(crate) const STAGGER_NAME: &str = "Stagger bars";
const STAGGER_ORIGIN_NAME: &str = "Stagger from";
pub(crate) const DETAILS_TOGGLE_LABEL: &str = "Show details";
pub(crate) const NOTIFY_LABEL: &str = "Show a notification";
const TABS_NAME: &str = "Closable tabs";
const REOPEN_LABEL: &str = "Reopen tabs";

const TITLES: [&str; 8] = [
    "Draft the brief",
    "Review the layout",
    "Pick a palette",
    "Write release notes",
    "Record a walkthrough",
    "Tidy the backlog",
    "Book the review",
    "Ship it",
];
const TABS: [&str; 4] = ["Overview", "Timeline", "Assets", "Notes"];
const BARS: usize = 12;
const STAGGER_ORIGINS: [(StaggerOrigin, &str); 3] = [
    (StaggerOrigin::First, "First"),
    (StaggerOrigin::Center, "Center"),
    (StaggerOrigin::Last, "Last"),
];

pub(super) fn gallery(theme_reader: DevThemeReader) -> impl Widget {
    let tile = FlexItem::new()
        .basis_gap_aware_fraction(1.0 / 2.0)
        .min_width(320.0);
    Flex::horizontal()
        .gap(14.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Stretch)
        .with_item(task_list_card(&theme_reader), tile)
        .with_item(stagger_card(&theme_reader), tile)
        .with_item(presence_card(&theme_reader), tile)
        .with_item(tabs_and_notifications_card(&theme_reader), tile)
}

/// A task in the keyed list.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Task {
    pub(super) id: u32,
    pub(super) title: String,
}

/// The keyed list's items, with the edits its buttons make.
#[derive(Clone)]
pub(super) struct TaskList {
    pub(super) items: Signal<Vec<Task>>,
    next_id: Rc<Cell<u32>>,
    shuffles: Rc<Cell<u32>>,
}

impl TaskList {
    pub(super) fn new() -> Self {
        let list = Self {
            items: Signal::named("Choreography tasks", Vec::new()),
            next_id: Rc::new(Cell::new(0)),
            shuffles: Rc::new(Cell::new(0)),
        };
        list.reset();
        list
    }

    fn task(&self) -> Task {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        Task {
            id,
            title: TITLES[id as usize % TITLES.len()].to_string(),
        }
    }

    /// Add `count` new tasks at the top.
    pub(super) fn add(&self, count: usize) {
        let new = (0..count).map(|_| self.task()).collect::<Vec<_>>();
        self.items.update(|items| {
            items.splice(0..0, new);
        });
    }

    pub(super) fn remove(&self, id: u32) {
        self.items
            .update(|items| items.retain(|task| task.id != id));
    }

    /// Remove the second task, or the only one.
    pub(super) fn remove_one(&self) {
        self.items.update(|items| {
            if !items.is_empty() {
                items.remove(1.min(items.len() - 1));
            }
        });
    }

    /// Reorder the tasks: alternately reverse them and move the first last.
    pub(super) fn shuffle(&self) {
        let shuffles = self.shuffles.get();
        self.shuffles.set(shuffles + 1);
        self.items.update(|items| {
            if shuffles.is_multiple_of(2) {
                items.reverse();
            } else if !items.is_empty() {
                items.rotate_left(1);
            }
        });
    }

    pub(super) fn reset(&self) {
        let tasks = (0..4).map(|_| self.task()).collect();
        self.items.set(tasks);
    }
}

fn task_list_card(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let theme = || clone_dev_theme_reader(theme_reader);
    let list = TaskList::new();
    let row_list = list.clone();
    let row_theme = Rc::clone(theme_reader);
    let tasks = KeyedStack::vertical(
        list.items.clone(),
        |task: &Task| task.id,
        move |id, task| task_row(*id, task, row_list.clone(), Rc::clone(&row_theme)),
    )
    .gap(6.0);
    let (add, add_three, remove, shuffle) = (list.clone(), list.clone(), list.clone(), list);

    card(
        theme_reader,
        "Keyed list",
        |theme| {
            format!(
                "Items enter in {} one after another, leave in {} while the gap closes, and glide to new places in {}",
                millis(theme.motion.entrance_duration()),
                millis(theme.motion.exit_spec().duration()),
                millis(theme.motion.layout_spec().duration())
            )
        },
        Stack::vertical()
            .gap(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Flex::horizontal()
                    .gap(6.0)
                    .wrap(FlexWrap::Wrap)
                    .with_child(button(ADD_TASK_LABEL, theme(), move || add.add(1)))
                    .with_child(button(ADD_THREE_LABEL, theme(), move || add_three.add(3)))
                    .with_child(button(REMOVE_TASK_LABEL, theme(), move || {
                        remove.remove_one()
                    }))
                    .with_child(button(SHUFFLE_LABEL, theme(), move || shuffle.shuffle())),
            )
            .with_child(
                SizedBox::new()
                    .width(300.0)
                    .child(SemanticRegion::new(KEYED_LIST_NAME, tasks)),
            ),
    )
}

fn button<F>(label: &str, theme: impl Fn() -> DefaultTheme + 'static, press: F) -> Button
where
    F: Fn() + 'static,
{
    Button::new(label)
        .theme_when(theme)
        .on_press_with_ctx(move |ctx| {
            press();
            refresh_page(ctx);
        })
}

fn task_row(
    id: u32,
    task: Signal<Task>,
    list: TaskList,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let title = task.get().title;
    Surface::panel(
        Flex::horizontal()
            .gap(8.0)
            .align_items(Alignment::Center)
            .with_item(
                Label::new(title).text_when(move || task.get().title),
                FlexItem::fill(),
            )
            .with_child(
                IconButton::new(IconGlyph::Close, "Remove task")
                    .theme_when(clone_dev_theme_reader(&theme_reader))
                    .on_press_with_ctx(move |ctx| {
                        list.remove(id);
                        refresh_page(ctx);
                    }),
            ),
    )
    .theme_when(clone_dev_theme_reader(&theme_reader))
    .padding(Insets {
        left: 12.0,
        top: 4.0,
        right: 4.0,
        bottom: 4.0,
    })
}

fn stagger_card(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let origin = Rc::new(Cell::new(StaggerOrigin::First));
    let replays = Signal::named("Stagger replays", 0_u32);
    let (chosen, set_origin, replay) = (Rc::clone(&origin), Rc::clone(&origin), replays.clone());
    card(
        theme_reader,
        "Stagger",
        |theme| {
            let stagger = theme.motion.stagger();
            format!(
                "{} apart, starting within {}; click the bars to replay",
                millis(stagger.interval),
                millis(stagger.max_delay.unwrap_or(0.0))
            )
        },
        Stack::vertical()
            .gap(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                SegmentedControl::new(STAGGER_ORIGIN_NAME)
                    .segments(STAGGER_ORIGINS.map(|(_, label)| label))
                    .selected_when(move || {
                        STAGGER_ORIGINS
                            .iter()
                            .position(|(origin, _)| *origin == chosen.get())
                    })
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .on_change_with_ctx(move |ctx, index, _| {
                        set_origin.set(STAGGER_ORIGINS[index].0);
                        replay.update(|count| *count += 1);
                        refresh_page(ctx);
                    }),
            )
            .with_child(SizedBox::new().width(300.0).child(StaggerBars::new(
                origin,
                replays,
                Rc::clone(theme_reader),
            ))),
    )
}

/// A row of bars that rise one after another from the chosen origin.
pub(super) struct StaggerBars {
    rise: Vec<Motion<f32>>,
    fade: Vec<Motion<f32>>,
    origin: Rc<Cell<StaggerOrigin>>,
    replays: Signal<u32>,
    seen: u32,
    theme_reader: DevThemeReader,
}

impl StaggerBars {
    pub(super) fn new(
        origin: Rc<Cell<StaggerOrigin>>,
        replays: Signal<u32>,
        theme_reader: DevThemeReader,
    ) -> Self {
        let seen = replays.get();
        Self {
            rise: vec![Motion::new(1.0).movement(); BARS],
            fade: vec![Motion::new(1.0); BARS],
            origin,
            replays,
            seen,
            theme_reader,
        }
    }

    fn replay(&mut self, ctx: &mut impl AnimateCtx) {
        let motion = (self.theme_reader)().motion;
        let stagger = motion.stagger().from(self.origin.get());
        let rise = AnimationSpec::spring(SpringSpec::SNAPPY);
        for (index, (bar, fade)) in self.rise.iter_mut().zip(&mut self.fade).enumerate() {
            let delay = stagger.delay(index, BARS);
            bar.jump_to(0.0);
            fade.jump_to(0.0);
            ctx.animate_after(bar, 1.0, delay, rise);
            ctx.animate_after(fade, 1.0, delay, motion.entrance_spec());
        }
    }
}

impl Widget for StaggerBars {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Down
        {
            self.replay(ctx);
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let replays = ctx.observe(&self.replays);
        if replays != self.seen {
            self.seen = replays;
            self.replay(ctx);
        }
        fill_width(constraints, 88.0)
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        let gap = 6.0;
        let width = ((bounds.width() - gap * (BARS - 1) as f32) / BARS as f32).max(2.0);
        for (index, (rise, fade)) in self.rise.iter().zip(&self.fade).enumerate() {
            let height = bounds.height() * (0.35 + 0.65 * index_wave(index)) * rise.get(ctx);
            let x = bounds.x() + index as f32 * (width + gap);
            let bar = Rect::new(x, bounds.max_y() - height, width, height.max(0.0));
            let alpha = fade.get(ctx).clamp(0.0, 1.0);
            ctx.fill(
                Path::rounded_rect(bar, (width * 0.5).min(6.0)),
                theme
                    .palette
                    .accent
                    .with_alpha(theme.palette.accent.alpha * alpha),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Button, ctx.bounds());
        node.name = Some(STAGGER_NAME.to_string());
        node.value = STAGGER_ORIGINS
            .iter()
            .find(|(origin, _)| *origin == self.origin.get())
            .map(|(_, label)| SemanticsValue::Text(format!("from {}", label.to_lowercase())));
        ctx.push(node);
    }
}

/// A gentle wave of bar heights, so the cascade reads as a shape.
fn index_wave(index: usize) -> f32 {
    let t = index as f32 / (BARS - 1) as f32;
    0.5 + 0.5 * (t * std::f32::consts::PI * 2.0).sin() * 0.8
}

fn presence_card(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let shown = Signal::named("Details shown", false);
    let toggle = shown.clone();
    let text = |text: &str| Label::new(text);
    card(
        theme_reader,
        "Presence",
        |theme| {
            format!(
                "Fades and grows in {}, out in {}; the space it takes follows",
                millis(theme.motion.entrance_duration()),
                millis(theme.motion.exit_spec().duration())
            )
        },
        Stack::vertical()
            .gap(8.0)
            .alignment(Alignment::Start)
            .with_child(
                Switch::new(DETAILS_TOGGLE_LABEL)
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .on_change(move |on| {
                        toggle.set(on);
                    }),
            )
            .with_child(
                Presence::new(
                    Surface::panel(
                        Stack::vertical()
                            .gap(4.0)
                            .alignment(Alignment::Start)
                            .with_child(text("Details stay retained while hidden."))
                            .with_child(text("Leaving content ignores clicks and focus."))
                            .with_child(text("Reduced motion fades without moving.")),
                    )
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .padding(Insets::all(12.0)),
                )
                .shown_from(shown)
                .collapse(Axis::Vertical),
            )
            .with_child(text("Content below moves with it.")),
    )
}

fn tabs_and_notifications_card(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    let theme = || clone_dev_theme_reader(theme_reader);
    let tabs = Rc::new(RefCell::new(TABS.map(String::from).to_vec()));
    let (read, close, reopen) = (Rc::clone(&tabs), Rc::clone(&tabs), tabs);
    let center = NotificationCenter::new();
    let notify = center.clone();
    let count = Rc::new(Cell::new(0_u32));

    card(
        theme_reader,
        "Tabs and notifications",
        |theme| {
            format!(
                "Closed tabs collapse and notifications leave in {}",
                millis(theme.motion.exit_spec().duration())
            )
        },
        Stack::vertical()
            .gap(10.0)
            .alignment(Alignment::Stretch)
            .with_child(
                SizedBox::new().width(460.0).child(
                    BrowserTabBar::new(TABS_NAME)
                        .tabs_when(move || read.borrow().clone())
                        .theme_when(theme())
                        .on_close(move |index, _| {
                            close.borrow_mut().remove(index);
                        }),
                ),
            )
            .with_child(
                Flex::horizontal()
                    .gap(6.0)
                    .with_child(
                        Button::new(REOPEN_LABEL)
                            .theme_when(theme())
                            .on_press_with_ctx(move |ctx| {
                                *reopen.borrow_mut() = TABS.map(String::from).to_vec();
                                refresh_page(ctx);
                            }),
                    )
                    .with_child(Button::new(NOTIFY_LABEL).theme_when(theme()).on_press(
                        move || {
                            let n = count.get() + 1;
                            count.set(n);
                            notify.push(
                                TransientNotification::new(
                                    format!("Notification {n}"),
                                    "Leaves after three seconds",
                                )
                                .duration(3.0),
                            );
                        },
                    )),
            )
            .with_child(
                SizedBox::new()
                    .height(176.0)
                    .child(NotificationHost::new(center).width(280.0)),
            ),
    )
}
