use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use sui::prelude::*;

use crate::app::{DevThemeReader, clone_dev_theme_reader, dev_text_style, dev_theme_color};

pub(crate) const COMMAND_DEMO_TAB_LABEL: &str = "Commands";
pub(crate) const COMMAND_DEMO_SCROLL_NAME: &str = "Command routing demo scroll";
pub(crate) const WINDOW_COMMAND_BUTTON: &str = "Send to this window";
pub(crate) const APPLICATION_COMMAND_BUTTON: &str = "Send to application";
pub(crate) const APPLICATION_BROADCAST_BUTTON: &str = "Broadcast to application";
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const BACKGROUND_COMMAND_BUTTON: &str = "Send from worker thread";
#[cfg(target_arch = "wasm32")]
pub(crate) const BACKGROUND_COMMAND_BUTTON: &str = "Send through cloned command handle";
pub(crate) const WAKE_CONTROLLERS_BUTTON: &str = "Wake controllers only";

pub(crate) static DEMO_WINDOW_COMMAND: CommandKey<CommandDemoMessage> =
    CommandKey::new("sui.demo.commands.window");
pub(crate) static DEMO_APPLICATION_COMMAND: CommandKey<CommandDemoMessage> =
    CommandKey::new("sui.demo.commands.application");
pub(crate) static DEMO_APPLICATION_BROADCAST: CommandKey<CommandDemoMessage> =
    CommandKey::new("sui.demo.commands.application-broadcast");

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandDemoMessage {
    sequence: u64,
    source: &'static str,
}

#[derive(Clone)]
pub(crate) struct CommandDemoState {
    next_sequence: Arc<AtomicU64>,
    wake_count: Arc<AtomicU64>,
    window_status: Signal<String>,
    application_status: Signal<String>,
    application_broadcast_status: Signal<String>,
    window_broadcast_status: Signal<String>,
    wake_status: Signal<String>,
}

impl CommandDemoState {
    pub(crate) fn new() -> Self {
        Self {
            next_sequence: Arc::new(AtomicU64::new(1)),
            wake_count: Arc::new(AtomicU64::new(0)),
            window_status: Signal::named(
                "command_demo_window_status",
                "No directed window command received yet.".to_string(),
            ),
            application_status: Signal::named(
                "command_demo_application_status",
                "No directed application command received yet.".to_string(),
            ),
            application_broadcast_status: Signal::named(
                "command_demo_application_broadcast_status",
                "Application multicast subscriber is waiting.".to_string(),
            ),
            window_broadcast_status: Signal::named(
                "command_demo_window_broadcast_status",
                "Window multicast subscriber is waiting.".to_string(),
            ),
            wake_status: Signal::named(
                "command_demo_wake_status",
                "No scheduler-only wake received yet.".to_string(),
            ),
        }
    }

    fn message(&self, source: &'static str) -> CommandDemoMessage {
        CommandDemoMessage {
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            source,
        }
    }

    pub(crate) fn record_window_command(&self, message: &CommandDemoMessage) {
        self.window_status.set(format!(
            "Window subscriber received #{} from {}.",
            message.sequence, message.source
        ));
    }

    pub(crate) fn record_application_command(&self, message: &CommandDemoMessage) {
        self.application_status.set(format!(
            "Application subscriber received #{} from {}.",
            message.sequence, message.source
        ));
    }

    pub(crate) fn record_application_broadcast(&self, message: &CommandDemoMessage) {
        self.application_broadcast_status.set(format!(
            "Application multicast subscriber received #{}.",
            message.sequence
        ));
    }

    pub(crate) fn record_window_broadcast(&self, message: &CommandDemoMessage) {
        self.window_broadcast_status.set(format!(
            "Window multicast subscriber received the same #{}.",
            message.sequence
        ));
    }

    fn record_wake(&self) {
        let count = self.wake_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.wake_status.set(format!(
            "Window controller wake hook ran {count} time{}; no custom widget event was sent.",
            if count == 1 { "" } else { "s" }
        ));
    }
}

pub(crate) struct CommandDemoWakeController {
    state: CommandDemoState,
}

impl CommandDemoWakeController {
    pub(crate) fn new(state: CommandDemoState) -> Self {
        Self { state }
    }
}

impl CommandController for CommandDemoWakeController {
    fn wake(&mut self, _ctx: &mut CommandCtx) {
        self.state.record_wake();
    }
}

pub(crate) fn build_command_demo_with_theme(
    state: CommandDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let theme = theme_reader();
    let window_state = state.clone();
    let application_state = state.clone();
    let broadcast_state = state.clone();

    let actions = Flex::horizontal()
        .gap(10.0)
        .wrap(FlexWrap::Wrap)
        .with_item(
            Button::new(WINDOW_COMMAND_BUTTON)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_press_with_ctx(move |ctx| {
                    ctx.command_sender().send_window(
                        ctx.window_id(),
                        DEMO_WINDOW_COMMAND,
                        window_state.message("widget callback"),
                    );
                }),
            FlexItem::new().no_shrink(),
        )
        .with_item(
            Button::new(APPLICATION_COMMAND_BUTTON)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_press_with_ctx(move |ctx| {
                    ctx.command_sender().send_application(
                        DEMO_APPLICATION_COMMAND,
                        application_state.message("widget callback"),
                    );
                }),
            FlexItem::new().no_shrink(),
        )
        .with_item(
            Button::new(APPLICATION_BROADCAST_BUTTON)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_press_with_ctx(move |ctx| {
                    ctx.command_sender().broadcast_application(
                        DEMO_APPLICATION_BROADCAST,
                        broadcast_state.message("widget callback"),
                    );
                }),
            FlexItem::new().no_shrink(),
        )
        .with_item(
            background_command_button(state.clone(), Rc::clone(&theme_reader)),
            FlexItem::new().no_shrink(),
        )
        .with_item(
            Button::new(WAKE_CONTROLLERS_BUTTON)
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .on_press_with_ctx(move |ctx| {
                    ctx.command_sender().wake();
                }),
            FlexItem::new().no_shrink(),
        );

    let statuses = Stack::vertical()
        .spacing(10.0)
        .alignment(Alignment::Stretch)
        .with_child(status_card(
            "Directed window scope",
            state.window_status.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(status_card(
            "Directed application scope",
            state.application_status.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(status_card(
            "Application-wide multicast",
            state.application_broadcast_status.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(status_card(
            "Window multicast subscriber",
            state.window_broadcast_status.clone(),
            Rc::clone(&theme_reader),
        ))
        .with_child(status_card(
            "Scheduler wake hook",
            state.wake_status.clone(),
            Rc::clone(&theme_reader),
        ));

    Background::new(
        theme.palette.surface,
        ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(16.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new("Typed application commands")
                        .style(dev_text_style(
                            theme,
                            theme.text._2xl,
                            theme.palette.text,
                        ))
                        .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
                )
                .with_child(
                    Label::new(
                        "Commands cross widget-tree boundaries without turning the presentation tree into a message bus. Try each route, then open the performance overlay to inspect command and invalidation traces.",
                    )
                    .style(dev_text_style(
                        theme,
                        theme.text.base,
                        theme.palette.text_muted,
                    ))
                    .color_when(dev_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
                )
                .with_child(actions)
                .with_child(statuses)
                .with_child(
                    Label::new(
                        "UiHandle exposes the same thread-safe producer to background work. command_sender().wake() schedules controller hooks only; it never synthesizes a root custom event.",
                    )
                    .style(dev_text_style(
                        theme,
                        theme.text.sm,
                        theme.palette.text_muted,
                    ))
                    .color_when(dev_theme_color(&theme_reader, |theme| {
                        theme.palette.text_muted
                    })),
                ),
        ))
        .name(COMMAND_DEMO_SCROLL_NAME)
        .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn status_card(
    title: &'static str,
    status: Signal<String>,
    theme_reader: DevThemeReader,
) -> impl Widget {
    let theme = theme_reader();
    Background::new(
        theme.surfaces.panel,
        Padding::all(
            14.0,
            Stack::vertical()
                .spacing(4.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(title)
                        .style(dev_text_style(theme, theme.text.sm, theme.palette.text))
                        .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
                )
                .with_child(
                    Label::new("")
                        .text_from(status)
                        .style(dev_text_style(
                            theme,
                            theme.text.sm,
                            theme.palette.text_muted,
                        ))
                        .color_when(dev_theme_color(&theme_reader, |theme| {
                            theme.palette.text_muted
                        })),
                ),
        ),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| theme.surfaces.panel))
}

#[cfg(not(target_arch = "wasm32"))]
fn background_command_button(state: CommandDemoState, theme_reader: DevThemeReader) -> Button {
    Button::new(BACKGROUND_COMMAND_BUTTON)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_press_with_ctx(move |ctx| {
            let sender = ctx.command_sender().clone();
            let message = state.message("worker thread");
            std::thread::spawn(move || {
                sender.send_application(DEMO_APPLICATION_COMMAND, message);
            });
        })
}

#[cfg(target_arch = "wasm32")]
fn background_command_button(state: CommandDemoState, theme_reader: DevThemeReader) -> Button {
    Button::new(BACKGROUND_COMMAND_BUTTON)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_press_with_ctx(move |ctx| {
            ctx.command_sender().clone().send_application(
                DEMO_APPLICATION_COMMAND,
                state.message("cloned web command handle"),
            );
        })
}
