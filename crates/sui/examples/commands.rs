use sui::prelude::*;

static WORK_FINISHED: CommandKey<String> = CommandKey::new("example.work-finished");
static ANNOUNCEMENT: CommandKey<String> = CommandKey::new("example.announcement");

fn main() -> Result<()> {
    let worker_status = Signal::named(
        "worker_status",
        "Waiting for the background worker…".to_string(),
    );
    let application_status = Signal::named(
        "application_status",
        "No application multicast received yet.".to_string(),
    );
    let window_status = Signal::named(
        "window_status",
        "No window multicast received yet.".to_string(),
    );

    let worker_handler = worker_status.clone();
    let application_handler = application_status.clone();
    let window_handler = window_status.clone();
    let root = Padding::all(
        24.0,
        Stack::vertical()
            .spacing(12.0)
            .alignment(Alignment::Stretch)
            .with_child(Label::new("Typed commands"))
            .with_child(Label::new("").text_from(worker_status))
            .with_child(Label::new("").text_from(application_status))
            .with_child(Label::new("").text_from(window_status))
            .with_child(
                Button::new("Broadcast announcement").on_press_with_ctx(|ctx| {
                    ctx.command_sender().broadcast_application(
                        ANNOUNCEMENT,
                        "Hello from a widget callback".to_string(),
                    );
                }),
            ),
    );

    App::new()
        .on_command(WORK_FINISHED, move |ctx, message| {
            worker_handler.set(format!("Worker result: {message}"));
            ctx.set_handled();
        })
        .on_command(ANNOUNCEMENT, move |_, message| {
            application_handler.set(format!("Application received: {message}"));
        })
        .window(
            Window::new("Commands")
                .on_command(ANNOUNCEMENT, move |_, message| {
                    window_handler.set(format!("Window received: {message}"));
                })
                .root(root),
        )
        .run_with_handle(|ui| {
            std::thread::spawn(move || {
                // Replace this with blocking I/O or CPU work.
                ui.send_application(WORK_FINISHED, "loaded".to_string());
            });
        })
}
