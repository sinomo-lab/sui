//! Background work: an export that runs off the UI thread and reports back
//! with commands. Each thumbnail goes to the progress bar with
//! `send_widget`; the end goes to the window's listeners with
//! `send_window`. Cancel sets a flag the worker checks between thumbnails.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use sui::{
    SemanticTone, SemanticsNode, SemanticsRole, SemanticsValue, Signal, WidgetId, WindowId,
    paint_progress_bar, prelude::*,
};

use super::{CommandDemoState, code_panel, titled_section};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader};
use crate::demo_support::{DemoTextColor, demo_label};

pub(super) const EXPORT_LABEL: &str = "Export 24 thumbnails";
pub(super) const CANCEL_LABEL: &str = "Cancel";
pub(super) const EXPORT_STATUS_NAME: &str = "Export status";
pub(super) const EXPORT_PROGRESS_NAME: &str = "Export progress";
pub(super) const THUMBNAILS: u32 = 24;
/// How long each thumbnail takes.
const STEP_SECONDS: f64 = 0.06;

#[cfg(not(target_arch = "wasm32"))]
const EXPORT_CODE: &str = "let sender = ctx.command_sender().clone();\nstd::thread::spawn(move || {\n    for done in 1..=24 {\n        if cancel.load(Ordering::Relaxed) { /* send Cancelled */ }\n        sender.send_widget(window_id, progress_id, EXPORT_PROGRESS, Progress { done });\n    }\n    sender.send_window(window_id, EXPORT_FINISHED, Finished::Done);\n});";
#[cfg(target_arch = "wasm32")]
const EXPORT_CODE: &str = "// The web build has no threads: a timer on the progress bar does one\n// thumbnail per tick, and reports through the same cloned sender.\nsender.send_widget(window_id, progress_id, EXPORT_PROGRESS, Progress { done });\nsender.send_window(window_id, EXPORT_FINISHED, Finished::Done);";

pub(super) static EXPORT_PROGRESS: CommandKey<Progress> = CommandKey::new("demo.export.progress");
pub(super) static EXPORT_FINISHED: CommandKey<Finished> = CommandKey::new("demo.export.finished");
/// Starts the web build's stand-in for the worker.
#[cfg(target_arch = "wasm32")]
static EXPORT_START: CommandKey<ExportJob> = CommandKey::new("demo.export.start");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Progress {
    done: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Finished {
    Done,
    Cancelled { done: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Running { done: u32 },
    Done,
    Cancelled { done: u32 },
}

impl Status {
    fn text(self) -> String {
        match self {
            Self::Idle => "Not started.".to_string(),
            Self::Running { done } => format!("Exporting: {done} of {THUMBNAILS} done."),
            Self::Done => format!("Exported {THUMBNAILS} thumbnails."),
            Self::Cancelled { done } => {
                format!("Cancelled after {done} of {THUMBNAILS}.")
            }
        }
    }
}

/// The export's state, shared by its controls, its progress bar, and the
/// status handler that hears it finish.
#[derive(Clone)]
pub(crate) struct ExportState {
    status: Signal<Status>,
    /// The running export's cancel flag.
    cancel: Rc<RefCell<Arc<AtomicBool>>>,
    /// The progress bar, which the worker reports to by id.
    meter: Rc<Cell<Option<WidgetId>>>,
}

impl ExportState {
    pub(super) fn new() -> Self {
        Self {
            status: Signal::named(EXPORT_STATUS_NAME, Status::Idle),
            cancel: Rc::new(RefCell::new(Arc::new(AtomicBool::new(false)))),
            meter: Rc::new(Cell::new(None)),
        }
    }

    fn running(&self) -> bool {
        matches!(self.status.get(), Status::Running { .. })
    }

    /// The status handler heard the export finish.
    pub(super) fn finish(&self, finished: Finished) {
        self.status.set(match finished {
            Finished::Done => Status::Done,
            Finished::Cancelled { done } => Status::Cancelled { done },
        });
    }

    fn start(&self, ctx: &mut EventCtx) {
        let Some(meter) = self.meter.get() else {
            return;
        };
        if self.running() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *self.cancel.borrow_mut() = Arc::clone(&cancel);
        self.status.set(Status::Running { done: 0 });
        let job = ExportJob {
            sender: ctx.command_sender().clone(),
            window_id: ctx.window_id(),
            meter,
            cancel,
            done: 0,
        };
        run(job, ctx);
    }

    fn cancel(&self) {
        self.cancel.borrow().store(true, Ordering::Relaxed);
    }
}

/// One export, as the worker sees it: where to report, and how far it is.
#[derive(Clone)]
struct ExportJob {
    sender: CommandSender,
    window_id: WindowId,
    meter: WidgetId,
    cancel: Arc<AtomicBool>,
    done: u32,
}

impl ExportJob {
    /// Report that nothing is done yet, which empties the progress bar.
    fn begin(&self) {
        self.sender.send_widget(
            self.window_id,
            self.meter,
            EXPORT_PROGRESS,
            Progress { done: 0 },
        );
    }

    /// Export the next thumbnail, or finish. Returns whether there is more
    /// to do.
    fn step(&mut self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            self.sender.send_window(
                self.window_id,
                EXPORT_FINISHED,
                Finished::Cancelled { done: self.done },
            );
            return false;
        }
        self.done += 1;
        self.sender.send_widget(
            self.window_id,
            self.meter,
            EXPORT_PROGRESS,
            Progress { done: self.done },
        );
        if self.done < THUMBNAILS {
            return true;
        }
        self.sender
            .send_window(self.window_id, EXPORT_FINISHED, Finished::Done);
        false
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(mut job: ExportJob, _ctx: &mut EventCtx) {
    std::thread::spawn(move || {
        job.begin();
        loop {
            std::thread::sleep(std::time::Duration::from_secs_f64(STEP_SECONDS));
            if !job.step() {
                break;
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn run(job: ExportJob, ctx: &mut EventCtx) {
    let meter = job.meter;
    ctx.post_command(meter, EXPORT_START, job);
}

pub(super) fn section(
    theme_reader: &DevThemeReader,
    state: &CommandDemoState,
) -> impl Widget + use<> {
    let export = &state.export;
    let start = {
        let export = export.clone();
        Button::primary(EXPORT_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .on_press_with_ctx(move |ctx| export.start(ctx))
    };
    let cancel = {
        let export = export.clone();
        Button::new(CANCEL_LABEL)
            .theme_when(clone_dev_theme_reader(theme_reader))
            .on_press(move || export.cancel())
    };
    titled_section(
        theme_reader,
        "Background work",
        "The export runs on a worker thread with a clone of the command sender. It reports each thumbnail to the progress bar's command() with send_widget, and tells the window's listeners it finished with send_window: the status handler takes it from there.",
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Flex::horizontal()
                    .gap(12.0)
                    .wrap(FlexWrap::Wrap)
                    .align_items(Alignment::Center)
                    .with_child(start)
                    .with_child(cancel)
                    .with_item(
                        ExportMeter::new(theme_reader, export),
                        FlexItem::new().grow(1.0).basis(240.0).min_width(160.0),
                    )
                    .with_child(
                        demo_label(theme_reader, "", DemoTextRole::Body, DemoTextColor::Text)
                            .semantic_name(EXPORT_STATUS_NAME)
                            .text_from(export.status.select(|status| status.text())),
                    ),
            )
            .with_child(code_panel(
                theme_reader,
                "Export code",
                Signal::new(EXPORT_CODE.to_string()),
            )),
    )
}

/// The export's progress bar. The worker reports each thumbnail to its
/// `command()` by id, and it draws what the command carries.
struct ExportMeter {
    theme_reader: DevThemeReader,
    export: ExportState,
    done: u32,
    /// The web build's stand-in for the worker, while it runs.
    #[cfg(target_arch = "wasm32")]
    job: Option<ExportJob>,
}

impl ExportMeter {
    fn new(theme_reader: &DevThemeReader, export: &ExportState) -> Self {
        Self {
            theme_reader: Rc::clone(theme_reader),
            export: export.clone(),
            done: 0,
            #[cfg(target_arch = "wasm32")]
            job: None,
        }
    }
}

impl Widget for ExportMeter {
    #[cfg(target_arch = "wasm32")]
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Wake(sui::WakeEvent::Timer { .. }) = event
            && let Some(job) = &mut self.job
        {
            if job.step() {
                ctx.schedule_timer_after(STEP_SECONDS);
            } else {
                self.job = None;
            }
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        #[cfg(target_arch = "wasm32")]
        if let Some(job) = command.get(EXPORT_START) {
            job.begin();
            self.job = Some(job.clone());
            ctx.schedule_timer_after(STEP_SECONDS);
            ctx.set_handled();
            return;
        }
        if let Some(progress) = command.get(EXPORT_PROGRESS) {
            self.done = progress.done;
            self.export.status.update(|status| {
                if let Status::Running { done } = status {
                    *done = progress.done;
                }
            });
            ctx.request_paint();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.export.meter.set(Some(ctx.widget_id()));
        constraints.clamp(Size::new(240.0, 8.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        paint_progress_bar(
            ctx,
            bounds,
            &theme,
            self.done as f32 / THUMBNAILS as f32,
            SemanticTone::Accent,
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ProgressBar, ctx.bounds());
        node.name = Some(EXPORT_PROGRESS_NAME.to_string());
        node.value = Some(SemanticsValue::Range {
            value: f64::from(self.done),
            min: 0.0,
            max: f64::from(THUMBNAILS),
        });
        ctx.push(node);
    }
}
