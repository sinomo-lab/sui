//! The Commands page: a map of the listeners each route delivers a command
//! to, a toolbar that sends edit commands to the editor you were using, and
//! an export that reports back from a worker thread. A strip under the
//! header lists what the window's runtime delivered, as it delivers it.

mod editing;
mod export;
mod routes;
#[cfg(test)]
mod tests;

use sui::{
    CommandDelivery, CommandDispatchSample, CommandTarget, Signal, WidgetPodMutVisitor,
    WidgetPodVisitor, WindowId, prelude::*, window_command_dispatches_signal,
};

use self::export::ExportState;
use self::routes::RouteState;
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, dev_theme_color};
#[cfg(test)]
use crate::demo_support::default_theme_reader;
use crate::demo_support::{DemoTextColor, NamedSection, demo_label, demo_mono_label};

#[cfg(test)]
pub(crate) use self::routes::SEND_LABEL;

pub(crate) const COMMAND_DEMO_TAB_LABEL: &str = "Commands";
pub(crate) const COMMAND_DEMO_SCROLL_NAME: &str = "Command routing demo scroll";
const SUMMARY: &str = "Send a command down each route and see which listeners it reaches, act on the editor you were using from a toolbar that leaves focus alone, and follow an export as it reports back from a worker.";

pub(crate) const TRACE_NAME: &str = "Recent dispatches";
const TRACE_LINES: usize = 6;
/// The name the runtime records a scheduler-only wake under.
const WAKE_NAME: &str = "scheduler wake";

/// What the page and its command listeners share. The listeners are
/// installed with the window, before the page is first built.
#[derive(Clone)]
pub(crate) struct CommandDemoState {
    routes: RouteState,
    export: ExportState,
}

impl CommandDemoState {
    pub(crate) fn new() -> Self {
        Self {
            routes: RouteState::new(),
            export: ExportState::new(),
        }
    }
}

/// Add the page's application listeners to `app`, and `window` with its
/// window listeners.
pub(crate) fn install(app: App, window: Window, state: &CommandDemoState) -> App {
    let window = routes::window_listeners(window, state);
    routes::application_listeners(app, state).window(window)
}

pub(crate) fn build_command_demo_with_theme(
    state: CommandDemoState,
    theme_reader: DevThemeReader,
) -> impl Widget {
    InWindow::new(move |window_id| page(&state, &theme_reader, window_id))
}

/// The page on its own, with its listeners, for tests.
#[cfg(test)]
pub(crate) fn build_command_application() -> Application {
    let state = CommandDemoState::new();
    let page = build_command_demo_with_theme(state.clone(), default_theme_reader());
    install(
        App::new(),
        Window::new(COMMAND_DEMO_TAB_LABEL)
            .initial_size(Size::new(1100.0, 900.0))
            .root(page),
        &state,
    )
    .into_application()
}

fn page(
    state: &CommandDemoState,
    theme_reader: &DevThemeReader,
    window_id: WindowId,
) -> impl Widget + use<> {
    let history = window_command_dispatches_signal(window_id);
    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(32.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Stack::vertical()
                        .spacing(16.0)
                        .alignment(Alignment::Stretch)
                        .with_child(header(theme_reader))
                        .with_child(trace(theme_reader, &history)),
                )
                .with_child(routes::section(theme_reader, state, &history))
                .with_child(editing::section(theme_reader))
                .with_child(export::section(theme_reader, state)),
        ))
        .name(COMMAND_DEMO_SCROLL_NAME)
        .theme_when(clone_dev_theme_reader(theme_reader)),
    )
    .brush_when(dev_theme_color(theme_reader, |theme| theme.palette.surface))
}

fn header(theme_reader: &DevThemeReader) -> impl Widget + use<> {
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(demo_label(
            theme_reader,
            COMMAND_DEMO_TAB_LABEL,
            DemoTextRole::PageTitle,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            theme_reader,
            SUMMARY,
            DemoTextRole::Supporting,
            DemoTextColor::Muted,
        ))
}

/// The window's latest dispatches, straight from the runtime's record.
fn trace(
    theme_reader: &DevThemeReader,
    history: &Signal<Vec<CommandDispatchSample>>,
) -> impl Widget + use<> {
    Surface::field(
        Stack::vertical()
            .spacing(6.0)
            .alignment(Alignment::Stretch)
            .with_child(demo_label(
                theme_reader,
                "Dispatch trace",
                DemoTextRole::CardTitle,
                DemoTextColor::Text,
            ))
            .with_child(demo_label(
                theme_reader,
                "What this window delivered, newest last, from window_command_dispatches_signal. Repeats are folded into one line.",
                DemoTextRole::Metadata,
                DemoTextColor::Muted,
            ))
            .with_child(
                demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
                    theme.palette.text
                })
                .semantic_name(TRACE_NAME)
                .text_from(history.select(|samples| trace_text(samples))),
            ),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(12.0))
    .fill_width()
}

/// The last lines of the trace. A run of dispatches that read the same,
/// such as an export's progress, folds into one line with its count.
fn trace_text(samples: &[CommandDispatchSample]) -> String {
    let mut lines: Vec<(String, u64, u64, usize)> = Vec::new();
    for sample in samples {
        let text = describe(sample);
        match lines.last_mut() {
            Some((last, _, end, count)) if *last == text => {
                *end = sample.sequence;
                *count += 1;
            }
            _ => lines.push((text, sample.sequence, sample.sequence, 1)),
        }
    }
    if lines.is_empty() {
        return blank_lines("Nothing delivered yet. Send a ping below and it shows up here.");
    }
    let skip = lines.len().saturating_sub(TRACE_LINES);
    let numbered = lines
        .into_iter()
        .skip(skip)
        .map(|(text, start, end, count)| {
            let number = if start == 0 {
                "wake".to_string()
            } else if count > 1 && start != end {
                format!("#{start}–{end} ×{count}")
            } else if count > 1 {
                format!("#{start} ×{count}")
            } else {
                format!("#{start}")
            };
            (number, text)
        })
        .collect::<Vec<_>>();
    let width = numbered
        .iter()
        .map(|(number, _)| number.chars().count())
        .max()
        .unwrap_or(0);
    let text = numbered
        .iter()
        .map(|(number, text)| format!("{number:<width$}  {text}"))
        .collect::<Vec<_>>()
        .join("\n");
    blank_lines(&text)
}

/// `text` filled out with empty lines, so the strip keeps its height.
fn blank_lines(text: &str) -> String {
    let lines = text.lines().count();
    format!("{text}{}", "\n".repeat(TRACE_LINES.saturating_sub(lines)))
}

/// A dispatch as one line: the command, where it went, who ran, and what
/// came of it.
fn describe(sample: &CommandDispatchSample) -> String {
    let target = match sample.target {
        CommandTarget::Widget { widget_id, .. } => format!("widget {}", widget_id.get()),
        CommandTarget::FocusedWidget(_) => "focused widget".to_string(),
        CommandTarget::Window(_) => "window".to_string(),
        CommandTarget::Application => "application".to_string(),
    };
    let delivery = match sample.delivery {
        CommandDelivery::Directed => "",
        CommandDelivery::Broadcast => ", broadcast",
    };
    let handlers = if sample.handlers.is_empty() {
        "no listeners".to_string()
    } else {
        sample.handlers.join(", ")
    };
    let outcome = if sample.name == WAKE_NAME {
        "woke"
    } else if !sample.delivered {
        "not delivered"
    } else if sample.handled {
        "handled"
    } else {
        "not handled"
    };
    format!(
        "{} → {target}{delivery} · {handlers} · {outcome}",
        sample.name
    )
}

/// A titled part of the page.
fn titled_section<W>(
    theme_reader: &DevThemeReader,
    title: &str,
    summary: &str,
    body: W,
) -> impl Widget + use<W>
where
    W: Widget + 'static,
{
    NamedSection::new(
        title,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(demo_label(
                theme_reader,
                title,
                DemoTextRole::SectionTitle,
                DemoTextColor::Text,
            ))
            .with_child(demo_label(
                theme_reader,
                summary,
                DemoTextRole::Supporting,
                DemoTextColor::Muted,
            ))
            .with_child(body),
    )
}

/// Code in a field, named `name` and following `code`.
fn code_panel<O>(theme_reader: &DevThemeReader, name: &str, code: O) -> impl Widget + use<O>
where
    O: sui::Observable<String> + 'static,
{
    Surface::field(
        demo_mono_label(theme_reader, "", DemoTextRole::Metadata, |theme| {
            theme.palette.text
        })
        .semantic_name(name)
        .text_from(code),
    )
    .theme_when(clone_dev_theme_reader(theme_reader))
    .padding(Insets::all(12.0))
    .fill_width()
}

type BuildForWindow = Box<dyn FnOnce(WindowId) -> Box<dyn Widget>>;

/// Builds its content once it knows the window it is in: the page reads
/// that window's dispatch history.
struct InWindow {
    build: Option<BuildForWindow>,
    child: SingleChild,
}

impl InWindow {
    fn new<F, W>(build: F) -> Self
    where
        F: FnOnce(WindowId) -> W + 'static,
        W: Widget + 'static,
    {
        Self {
            build: Some(Box::new(move |window_id| Box::new(build(window_id)))),
            child: SingleChild::new(SizedBox::new()),
        }
    }
}

impl Widget for InWindow {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(build) = self.build.take() {
            self.child = SingleChild::from_pod(WidgetPod::new_boxed(build(ctx.window_id())));
            ctx.record_rebuild("InWindow", "built for its window");
        }
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: sui::Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}
