//! The route map: a ping sent down the route you pick, and the listeners it
//! reaches. Each listener reports what it did with the ping; the runtime's
//! own record of the dispatch says whether it was delivered at all.

use std::{cell::Cell, rc::Rc};

use sui::diagnostics::CommandDispatchSample;
use sui::{
    CommandTarget, Easing, EventPhase, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsValue, Signal, WidgetId, WidgetPod, WidgetPodMutVisitor,
    WidgetPodVisitor, WindowId, prelude::*,
};

use super::export::{EXPORT_FINISHED, ExportState};
use super::{CommandDemoState, WAKE_NAME, code_panel, titled_section};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::{DemoTextColor, demo_label, demo_mono_label};
use crate::settings::controls::labeled_control;

pub(crate) const SEND_LABEL: &str = "Send ping";
pub(super) const ROUTE_NAME: &str = "Route";
pub(super) const SENDER_NAME: &str = "Send from";
pub(super) const DELIVERY_NAME: &str = "Delivery";
pub(super) const ROUTE_CODE_NAME: &str = "Route code";
pub(super) const NOTES_NAME: &str = "Notes";

/// What the route map sends.
pub(super) static PING: CommandKey<Ping> = CommandKey::new("demo.ping");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Ping {
    /// Which press of Send this is, for the listeners to report against.
    serial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    Widget,
    Focused,
    Closed,
    Window,
    WindowBroadcast,
    Application,
    ApplicationBroadcast,
    Wake,
}

pub(super) const ROUTES: [Route; 8] = [
    Route::Widget,
    Route::Focused,
    Route::Closed,
    Route::Window,
    Route::WindowBroadcast,
    Route::Application,
    Route::ApplicationBroadcast,
    Route::Wake,
];

impl Route {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Widget => "Widget",
            Self::Focused => "Focused widget",
            Self::Closed => "Closed widget",
            Self::Window => "Window",
            Self::WindowBroadcast => "Window broadcast",
            Self::Application => "Application",
            Self::ApplicationBroadcast => "Application broadcast",
            Self::Wake => "Wake only",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Widget => {
                "Straight to one widget's command(), by id. Nothing else sees it, and it does not bubble."
            }
            Self::Focused => {
                "To whatever has focus. Click the target card or the notes field first: Send leaves focus where it is. From the keyboard, Send itself has focus, so the ping goes to Send, which ignores it."
            }
            Self::Closed => {
                "To a widget that has since been dropped, as a closed dialog's would be. Nothing receives it, and the runtime records it as not delivered."
            }
            Self::Window => {
                "To the window's listeners in order, until one handles it. Turn a switch off to pass it along."
            }
            Self::WindowBroadcast => "To every window listener, handled or not.",
            Self::Application => {
                "To the application's listeners in order, until one handles it. Windows do not see it."
            }
            Self::ApplicationBroadcast => {
                "To every application listener, then every window's listeners."
            }
            Self::Wake => {
                "Runs every controller's wake hook and delivers no command: for work queued elsewhere."
            }
        }
    }

    fn call(self) -> &'static str {
        match self {
            Self::Widget => "sender.send_widget(window_id, card_id, PING, ping)",
            Self::Focused => "sender.send_focused(window_id, PING, ping)",
            Self::Closed => "sender.send_widget(window_id, closed_id, PING, ping)",
            Self::Window => "sender.send_window(window_id, PING, ping)",
            Self::WindowBroadcast => "sender.broadcast_window(window_id, PING, ping)",
            Self::Application => "sender.send_application(PING, ping)",
            Self::ApplicationBroadcast => "sender.broadcast_application(PING, ping)",
            Self::Wake => "sender.wake()",
        }
    }

    /// Whether delivery stops at the first listener that handles it.
    fn directed(self) -> bool {
        matches!(
            self,
            Self::Widget | Self::Focused | Self::Closed | Self::Window | Self::Application
        )
    }

    fn reaches(self, lane: Lane) -> bool {
        use Lane::*;
        match self {
            Self::Widget => lane == TargetCard,
            Self::Focused => matches!(lane, TargetCard | Notes),
            Self::Closed => false,
            Self::Window | Self::WindowBroadcast => {
                matches!(lane, WindowController | StatusHandler)
            }
            Self::Application => matches!(lane, Settings | Analytics),
            Self::ApplicationBroadcast | Self::Wake => {
                matches!(
                    lane,
                    Settings | Analytics | WindowController | StatusHandler
                )
            }
        }
    }
}

/// The thread a ping is sent from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Ui,
    Worker,
}

impl Origin {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Ui => "UI thread",
            #[cfg(not(target_arch = "wasm32"))]
            Self::Worker => "Worker thread",
            #[cfg(target_arch = "wasm32")]
            Self::Worker => "Cloned handle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Choice {
    route: Route,
    origin: Origin,
}

fn route_code(choice: &Choice) -> String {
    let call = choice.route.call();
    match choice.origin {
        Origin::Ui => format!("let sender = ctx.command_sender();\n{call};"),
        #[cfg(not(target_arch = "wasm32"))]
        Origin::Worker => format!(
            "let sender = ctx.command_sender().clone();\nstd::thread::spawn(move || {call});"
        ),
        #[cfg(target_arch = "wasm32")]
        Origin::Worker => format!(
            "// The web build has no threads; a clone works the same.\nlet sender = ctx.command_sender().clone();\n{call};"
        ),
    }
}

/// A listener on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lane {
    Settings,
    Analytics,
    WindowController,
    StatusHandler,
    TargetCard,
    Notes,
}

const LANES: [Lane; 6] = [
    Lane::Settings,
    Lane::Analytics,
    Lane::WindowController,
    Lane::StatusHandler,
    Lane::TargetCard,
    Lane::Notes,
];

impl Lane {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Settings => "Settings service",
            Self::Analytics => "Analytics",
            Self::WindowController => "Window controller",
            Self::StatusHandler => "Status handler",
            Self::TargetCard => "Target card",
            Self::Notes => "Notes field",
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::Settings => "App controller · handles pings",
            Self::Analytics => "App controller · only observes",
            Self::WindowController => "Window controller · handles pings",
            Self::StatusHandler => "Window controller · handles pings",
            Self::TargetCard => "Widget · command() handles pings",
            Self::Notes => "Text field · ignores pings",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }

    /// Which of the page's switches decides whether this lane handles
    /// pings, if one does.
    fn switch(self) -> Option<usize> {
        match self {
            Self::Settings => Some(0),
            Self::WindowController => Some(1),
            Self::StatusHandler => Some(2),
            _ => None,
        }
    }

    pub(super) fn switch_name(self) -> String {
        format!("{} handles pings", self.name())
    }
}

/// What a listener did with a ping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    Handled,
    PassedOn,
    Woke,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Record {
    serial: u64,
    sequence: Option<u64>,
    outcome: Outcome,
}

/// The latest press of Send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sent {
    serial: u64,
    route: Route,
    /// The command's sequence, when it was sent on the UI thread; a
    /// worker's comes back with the listeners' reports.
    sequence: Option<u64>,
    /// Which lane had focus, for a send to the focused widget.
    focused: Option<Lane>,
}

/// The latest send and what each listener did with the pings it heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct RouteMap {
    sent: Option<Sent>,
    records: [Option<Record>; 6],
}

/// What a lane shows for the latest send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LaneView {
    Idle,
    OffRoute,
    Waiting,
    Ran {
        serial: u64,
        sequence: Option<u64>,
        outcome: Outcome,
    },
    Skipped {
        sequence: Option<u64>,
    },
}

impl RouteMap {
    fn sequence(&self, sent: &Sent) -> Option<u64> {
        sent.sequence.or_else(|| {
            self.records
                .iter()
                .flatten()
                .find(|record| record.serial == sent.serial)
                .and_then(|record| record.sequence)
        })
    }

    pub(super) fn view(&self, lane: Lane) -> LaneView {
        let Some(sent) = self.sent else {
            return LaneView::Idle;
        };
        let sequence = self.sequence(&sent);
        if !sent.route.reaches(lane) {
            return LaneView::OffRoute;
        }
        if sent.route == Route::Focused {
            // Of the two widgets, only the one with focus is on this route.
            if sent.focused != Some(lane) {
                return LaneView::OffRoute;
            }
            if lane == Lane::Notes {
                // A text field cannot report, and does nothing with pings.
                return LaneView::Ran {
                    serial: sent.serial,
                    sequence,
                    outcome: Outcome::Ignored,
                };
            }
        }
        if let Some(record) =
            self.records[lane.index()].filter(|record| record.serial == sent.serial)
        {
            return LaneView::Ran {
                serial: sent.serial,
                sequence,
                outcome: record.outcome,
            };
        }
        let handled_before = LANES.iter().any(|other| {
            sent.route.reaches(*other)
                && self.records[other.index()].is_some_and(|record| {
                    record.serial == sent.serial && record.outcome == Outcome::Handled
                })
        });
        if sent.route.directed() && handled_before {
            LaneView::Skipped { sequence }
        } else {
            LaneView::Waiting
        }
    }
}

impl LaneView {
    pub(super) fn status(self) -> String {
        let number =
            |sequence: Option<u64>| sequence.map_or_else(String::new, |s| format!("#{s} "));
        match self {
            Self::Idle => "Waiting for a ping".to_string(),
            Self::OffRoute => "Not on this route".to_string(),
            Self::Waiting => "Not reached".to_string(),
            Self::Ran {
                sequence, outcome, ..
            } => match outcome {
                Outcome::Handled => format!("{}handled it", number(sequence)),
                Outcome::PassedOn => format!("{}ran, passed it on", number(sequence)),
                Outcome::Woke => "Wake hook ran".to_string(),
                Outcome::Ignored => format!("{}got it, ignored it", number(sequence)),
            },
            Self::Skipped { sequence } => {
                format!("{}skipped, already handled", number(sequence))
            }
        }
    }
}

/// The route map's state, shared by its controls, its listeners, and the
/// target card.
#[derive(Clone)]
pub(super) struct RouteState {
    choice: Signal<Choice>,
    /// Whether the Settings service, Window controller, and Status handler
    /// handle pings.
    handles: Signal<[bool; 3]>,
    pub(super) map: Signal<RouteMap>,
    /// Which of the target card and the notes field has focus.
    focus: Signal<Option<Lane>>,
    next_serial: Rc<Cell<u64>>,
    /// The target card, for sends to it by id.
    card: Rc<Cell<Option<WidgetId>>>,
    /// A widget that was built and dropped, for sends that reach nothing.
    closed: WidgetId,
}

impl RouteState {
    pub(super) fn new() -> Self {
        Self {
            choice: Signal::named(
                ROUTE_NAME,
                Choice {
                    route: Route::Window,
                    origin: Origin::Ui,
                },
            ),
            handles: Signal::named("Listeners handle pings", [true; 3]),
            map: Signal::named("Route map", RouteMap::default()),
            focus: Signal::named("Route map focus", None),
            next_serial: Rc::new(Cell::new(1)),
            card: Rc::new(Cell::new(None)),
            closed: WidgetPod::new(SizedBox::new()).id(),
        }
    }

    fn handles(&self, lane: Lane) -> bool {
        lane.switch()
            .is_some_and(|switch| self.handles.get()[switch])
    }

    fn record(&self, lane: Lane, serial: u64, sequence: Option<u64>, outcome: Outcome) {
        self.map.update(|map| {
            map.records[lane.index()] = Some(Record {
                serial,
                sequence,
                outcome,
            });
        });
    }

    /// A listener heard a command: if it is a ping, handle it or pass it
    /// on, and report which.
    fn hear(&self, lane: Lane, ctx: &mut CommandCtx, command: &Command<'_>) {
        if let Some(ping) = command.get(PING) {
            let handles = self.handles(lane);
            if handles {
                ctx.set_handled();
            }
            self.record(
                lane,
                ping.serial,
                Some(command.sequence()),
                if handles {
                    Outcome::Handled
                } else {
                    Outcome::PassedOn
                },
            );
        }
    }

    /// A controller's wake hook ran. Wakes carry nothing, so this reports
    /// against the latest send, if it was a wake.
    fn woke(&self, lane: Lane) {
        let Some(sent) = self.map.get().sent.filter(|sent| sent.route == Route::Wake) else {
            return;
        };
        self.record(lane, sent.serial, None, Outcome::Woke);
    }

    fn set_focus(&self, lane: Lane, focused: bool) {
        self.focus.update(|focus| {
            if focused {
                *focus = Some(lane);
            } else if *focus == Some(lane) {
                *focus = None;
            }
        });
    }

    /// Send a ping down the chosen route, from the chosen thread.
    fn send(&self, ctx: &mut EventCtx) {
        let Choice { route, origin } = self.choice.get();
        let serial = self.next_serial.get();
        self.next_serial.set(serial + 1);
        let window_id = ctx.window_id();
        let targets = Targets {
            card: self.card.get(),
            closed: self.closed,
        };
        let ping = Ping { serial };
        let sequence = match origin {
            Origin::Ui => deliver(ctx.command_sender(), route, window_id, targets, ping),
            Origin::Worker => {
                let sender = ctx.command_sender().clone();
                on_worker(move || {
                    deliver(&sender, route, window_id, targets, ping);
                });
                None
            }
        };
        let focused = (route == Route::Focused)
            .then(|| self.focus.get())
            .flatten();
        self.map.update(|map| {
            map.sent = Some(Sent {
                serial,
                route,
                sequence,
                focused,
            });
        });
    }
}

/// The widgets pings go to by id.
#[derive(Clone, Copy)]
struct Targets {
    card: Option<WidgetId>,
    closed: WidgetId,
}

/// Send `ping` down `route`, returning its sequence; a wake has none.
fn deliver(
    sender: &CommandSender,
    route: Route,
    window_id: WindowId,
    targets: Targets,
    ping: Ping,
) -> Option<u64> {
    match route {
        Route::Widget => targets
            .card
            .map(|card| sender.send_widget(window_id, card, PING, ping)),
        Route::Closed => Some(sender.send_widget(window_id, targets.closed, PING, ping)),
        Route::Focused => Some(sender.send_focused(window_id, PING, ping)),
        Route::Window => Some(sender.send_window(window_id, PING, ping)),
        Route::WindowBroadcast => Some(sender.broadcast_window(window_id, PING, ping)),
        Route::Application => Some(sender.send_application(PING, ping)),
        Route::ApplicationBroadcast => Some(sender.broadcast_application(PING, ping)),
        Route::Wake => {
            sender.wake();
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn on_worker(work: impl FnOnce() + Send + 'static) {
    std::thread::spawn(work);
}

/// The web build has no threads: the cloned sender is used right away.
#[cfg(target_arch = "wasm32")]
fn on_worker(work: impl FnOnce() + Send + 'static) {
    work();
}

/// One of the page's controllers: it hears every command in its scope and
/// reports what it did with pings. The status handler also finishes
/// exports.
struct Listener {
    lane: Lane,
    routes: RouteState,
    export: ExportState,
}

impl Listener {
    fn new(lane: Lane, state: &CommandDemoState) -> Self {
        Self {
            lane,
            routes: state.routes.clone(),
            export: state.export.clone(),
        }
    }
}

impl CommandController for Listener {
    fn command(&mut self, ctx: &mut CommandCtx, command: &Command<'_>) {
        if self.lane == Lane::StatusHandler
            && let Some(finished) = command.get(EXPORT_FINISHED)
        {
            self.export.finish(*finished);
            ctx.set_handled();
            return;
        }
        self.routes.hear(self.lane, ctx, command);
    }

    fn wake(&mut self, _ctx: &mut CommandCtx) {
        self.routes.woke(self.lane);
    }

    fn debug_name(&self) -> &'static str {
        self.lane.name()
    }
}

pub(super) fn application_listeners(app: App, state: &CommandDemoState) -> App {
    app.controller(Listener::new(Lane::Settings, state))
        .controller(Listener::new(Lane::Analytics, state))
}

pub(super) fn window_listeners(window: Window, state: &CommandDemoState) -> Window {
    window
        .controller(Listener::new(Lane::WindowController, state))
        .controller(Listener::new(Lane::StatusHandler, state))
}

pub(super) fn section(
    theme_reader: &DevThemeReader,
    state: &CommandDemoState,
    history: &Signal<Vec<CommandDispatchSample>>,
) -> impl Widget + use<> {
    let routes = &state.routes;
    let lanes = Flex::horizontal()
        .gap(16.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start)
        .with_item(
            lane_group(
                theme_reader,
                routes,
                "Application listeners",
                [Lane::Settings, Lane::Analytics],
            ),
            FlexItem::new().grow(1.0).basis(300.0).min_width(260.0),
        )
        .with_item(
            lane_group(
                theme_reader,
                routes,
                "Window listeners",
                [Lane::WindowController, Lane::StatusHandler],
            ),
            FlexItem::new().grow(1.0).basis(300.0).min_width(260.0),
        )
        .with_item(
            lane_group(
                theme_reader,
                routes,
                "Widgets",
                [Lane::TargetCard, Lane::Notes],
            ),
            FlexItem::new().grow(1.0).basis(300.0).min_width(260.0),
        );
    titled_section(
        theme_reader,
        "Route map",
        "Pick a route and send a ping. Each listener lights up with what it did, and the line under the map says whether the runtime delivered it at all.",
        Stack::vertical()
            .gap(14.0)
            .alignment(Alignment::Stretch)
            .with_child(controls(theme_reader, routes))
            .with_child(
                demo_label(
                    theme_reader,
                    "",
                    DemoTextRole::Supporting,
                    DemoTextColor::Muted,
                )
                .text_from(
                    routes
                        .choice
                        .select(|choice| choice.route.hint().to_string()),
                ),
            )
            .with_child(lanes)
            .with_child(delivery(theme_reader, history))
            .with_child(code_panel(
                theme_reader,
                ROUTE_CODE_NAME,
                routes.choice.select(route_code),
            )),
    )
}

fn controls(theme_reader: &DevThemeReader, routes: &RouteState) -> impl Widget + use<> {
    let route = {
        let read = routes.choice.clone();
        let write = routes.choice.clone();
        Select::new(ROUTE_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .options(ROUTES.map(Route::label))
            .selected_when(move || {
                let route = read.get().route;
                ROUTES.iter().position(|candidate| *candidate == route)
            })
            .on_change(move |index, _| {
                write.update(|choice| choice.route = ROUTES[index]);
            })
    };
    let origin = {
        let read = routes.choice.clone();
        let write = routes.choice.clone();
        SegmentedControl::new(SENDER_NAME)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .segments([Origin::Ui.label(), Origin::Worker.label()])
            .selected_when(move || Some(usize::from(read.get().origin == Origin::Worker)))
            .on_change(move |index, _| {
                write.update(|choice| {
                    choice.origin = if index == 0 {
                        Origin::Ui
                    } else {
                        Origin::Worker
                    };
                });
            })
    };
    let send = {
        let routes = routes.clone();
        // Clicking Send leaves focus where it is, so a ping to the focused
        // widget reaches the one you clicked before. Tab still reaches it.
        Button::primary(SEND_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .focus_on_press(false)
            .on_press_with_ctx(move |ctx| routes.send(ctx))
    };
    Flex::horizontal()
        .gap(20.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::End)
        .with_child(labeled_control(theme_reader, ROUTE_NAME, 220.0, route))
        .with_child(labeled_control(theme_reader, SENDER_NAME, 240.0, origin))
        .with_child(send)
}

/// A column of listeners that share a scope.
fn lane_group(
    theme_reader: &DevThemeReader,
    routes: &RouteState,
    title: &'static str,
    lanes: [Lane; 2],
) -> impl Widget + use<> {
    let mut group = Stack::vertical()
        .gap(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            title,
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ));
    for lane in lanes {
        group = group.with_child(LaneChip::new(theme_reader, routes, lane));
    }
    group
}

/// Whether the latest ping was delivered, from the window's dispatch
/// history.
fn delivery(
    theme_reader: &DevThemeReader,
    history: &Signal<Vec<CommandDispatchSample>>,
) -> impl Widget + use<> {
    demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
        .semantic_name(DELIVERY_NAME)
        .text_from(history.select(|samples| verdict(samples)))
}

/// What became of the latest ping or wake, as the runtime recorded it.
fn verdict(samples: &[CommandDispatchSample]) -> String {
    let Some(latest) = samples
        .iter()
        .rev()
        .find(|sample| sample.name == PING.name() || sample.name == WAKE_NAME)
    else {
        return "Nothing sent yet.".to_string();
    };
    if latest.name == WAKE_NAME {
        let hooks: usize = samples
            .iter()
            .rev()
            .take_while(|sample| sample.name == WAKE_NAME)
            .map(|sample| sample.handlers.len())
            .sum();
        return format!("Wake: {hooks} controller wake hooks ran. No command was delivered.");
    }
    let sequence = latest.sequence;
    let parts = samples
        .iter()
        .filter(|sample| sample.sequence == sequence)
        .collect::<Vec<_>>();
    if parts.iter().all(|sample| !sample.delivered) {
        let why = match latest.target {
            CommandTarget::FocusedWidget(_) => "nothing had focus",
            CommandTarget::Widget { .. } => "no widget has that id",
            _ => "nothing listens there",
        };
        return format!("#{sequence} was not delivered: {why}.");
    }
    let listeners: usize = parts.iter().map(|sample| sample.handlers.len()).sum();
    let reached = if listeners == 1 {
        "1 listener".to_string()
    } else {
        format!("{listeners} listeners")
    };
    if parts.iter().any(|sample| sample.handled) {
        format!("#{sequence} reached {reached} and was handled.")
    } else {
        format!("#{sequence} reached {reached}, and none handled it.")
    }
}

/// A listener on the map: its name, what it did with the latest ping, and
/// a flash when it runs. The target card is one too: it takes focus, and
/// handles pings in its own `command()`. The notes field sits in one.
struct LaneChip {
    theme_reader: DevThemeReader,
    lane: Lane,
    routes: RouteState,
    view: sui::Selector<Signal<RouteMap>, RouteMap, LaneView>,
    seen: LaneView,
    flash: Motion<f32>,
    child: SingleChild,
}

impl LaneChip {
    fn new(theme_reader: &DevThemeReader, routes: &RouteState, lane: Lane) -> Self {
        let view = routes.map.select(move |map| map.view(lane));
        let mut title = Flex::horizontal()
            .gap(8.0)
            .align_items(Alignment::Center)
            .with_item(
                demo_label(
                    theme_reader,
                    lane.name(),
                    DemoTextRole::CardTitle,
                    DemoTextColor::Text,
                ),
                FlexItem::flex(1.0),
            );
        if let Some(switch) = lane.switch() {
            let read = routes.handles.clone();
            let write = routes.handles.clone();
            title = title.with_child(
                Switch::new("Handles it")
                    .semantic_name(lane.switch_name())
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .checked_when(move || read.get()[switch])
                    .on_change(move |on| {
                        write.update(|handles| handles[switch] = on);
                    }),
            );
        }
        let mut content = Stack::vertical()
            .gap(4.0)
            .alignment(Alignment::Stretch)
            .with_child(title)
            .with_child(demo_label(
                theme_reader,
                lane.kind(),
                DemoTextRole::Metadata,
                DemoTextColor::Muted,
            ));
        if lane == Lane::Notes {
            let focus = routes.clone();
            content = content.with_child(
                TextInput::new(NOTES_NAME)
                    .theme_when(clone_dev_theme_reader(theme_reader))
                    .placeholder("Focus me, then send")
                    .on_focus_change(move |focused| focus.set_focus(Lane::Notes, focused)),
            );
        }
        content = content.with_child(
            demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
                theme.palette.text
            })
            .text_from(routes.map.select(move |map| map.view(lane).status())),
        );
        let seen = view.get();
        Self {
            theme_reader: Rc::clone(theme_reader),
            lane,
            routes: routes.clone(),
            view,
            seen,
            flash: Motion::new(0.0),
            child: SingleChild::new(Padding::all(10.0, content)),
        }
    }

    fn is_target(&self) -> bool {
        self.lane == Lane::TargetCard
    }
}

impl Widget for LaneChip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_target() {
            return;
        }
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Down
            && pointer.button == Some(PointerButton::Primary)
            && ctx.phase() != EventPhase::Capture
        {
            // Handled here, so the page's scroll view does not take focus.
            ctx.request_focus();
            ctx.set_handled();
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if self.is_target()
            && let Some(ping) = command.get(PING)
        {
            self.routes.record(
                Lane::TargetCard,
                ping.serial,
                Some(command.sequence()),
                Outcome::Handled,
            );
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.is_target() {
            self.routes.card.set(Some(ctx.widget_id()));
        }
        let view = ctx.observe(&self.view);
        if view != self.seen {
            let ran_now = matches!(view, LaneView::Ran { serial, .. }
                if !matches!(self.seen, LaneView::Ran { serial: seen, .. } if seen == serial));
            if ran_now {
                self.flash.jump_to(1.0);
                ctx.animate(
                    &mut self.flash,
                    0.0,
                    AnimationSpec::tween(0.9, Easing::EaseOut),
                );
            }
            self.seen = view;
        }
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let palette = theme.palette;
        let bounds = ctx.bounds();
        let (fill, border) = match self.seen {
            LaneView::Ran {
                outcome: Outcome::Handled,
                ..
            } => (palette.success_soft, palette.success_border),
            LaneView::Ran {
                outcome: Outcome::Woke,
                ..
            } => (palette.info_soft, palette.info_border),
            LaneView::Ran { .. } => (palette.surface_raised, palette.border_strong),
            LaneView::Skipped { .. } => (palette.surface_raised, palette.border),
            LaneView::OffRoute => (palette.surface, palette.border.with_alpha(0.5)),
            LaneView::Idle | LaneView::Waiting => (palette.surface_raised, palette.border),
        };
        let radius = 10.0;
        ctx.fill(Path::rounded_rect(bounds, radius), fill);
        let flash = self.flash.get(ctx);
        if flash > 0.0 {
            ctx.fill(
                Path::rounded_rect(bounds, radius),
                palette.accent.with_alpha(0.3 * flash),
            );
        }
        let (border, width) = if self.is_target() && ctx.is_focused() {
            (palette.focus_ring, 2.0)
        } else {
            (border, 1.0)
        };
        ctx.stroke(
            Path::rounded_rect(
                bounds.inflate(-width * 0.5, -width * 0.5),
                radius - width * 0.5,
            ),
            border,
            StrokeStyle::new(width),
        );
        self.child.paint(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.is_target()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if self.is_target() {
            self.routes.set_focus(Lane::TargetCard, focused);
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.lane.name().to_string());
        node.value = Some(SemanticsValue::Text(self.seen.status()));
        if self.is_target() {
            node.description = Some("Focus it, then send to the focused widget".to_string());
            node.actions = vec![SemanticsAction::Focus];
            node.state.focused = ctx.is_focused();
        }
        ctx.push(node);
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}
