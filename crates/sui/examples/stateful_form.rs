use std::{cell::RefCell, rc::Rc};

use sui::{InvalidationKind, InvalidationRequest, InvalidationTarget, prelude::*};

#[derive(Debug)]
struct FormState {
    name: String,
    password: String,
    scheduled_for: String,
    submissions: u32,
}

impl Default for FormState {
    fn default() -> Self {
        Self {
            name: String::new(),
            password: String::new(),
            scheduled_for: "2026-07-18 10:00".to_string(),
            submissions: 0,
        }
    }
}

impl FormState {
    fn can_submit(&self) -> bool {
        !self.name.trim().is_empty()
            && self.password.chars().count() >= 8
            && !self.scheduled_for.trim().is_empty()
    }

    fn status(&self) -> String {
        if self.submissions == 0 {
            format!(
                "Draft for {} · {} password characters · scheduled {}",
                display_name(&self.name),
                self.password.chars().count(),
                self.scheduled_for
            )
        } else {
            format!(
                "Saved {} time(s) for {} at {}",
                self.submissions,
                display_name(&self.name),
                self.scheduled_for
            )
        }
    }
}

fn display_name(name: &str) -> &str {
    if name.trim().is_empty() {
        "an unnamed user"
    } else {
        name.trim()
    }
}

fn refresh_window(ctx: &mut EventCtx) {
    let target = InvalidationTarget::Window(ctx.window_id());
    for kind in [
        InvalidationKind::Measure,
        InvalidationKind::Paint,
        InvalidationKind::Semantics,
    ] {
        ctx.request(InvalidationRequest::new(target, kind));
    }
}

fn main() -> Result<()> {
    let theme = DefaultTheme::dark();
    let state = Rc::new(RefCell::new(FormState::default()));

    let name_state = Rc::clone(&state);
    let name = TextInput::new("Display name")
        .placeholder("Ada Lovelace")
        .min_width(360.0)
        .theme(theme)
        .on_change_with_ctx(move |ctx, value| {
            name_state.borrow_mut().name = value;
            refresh_window(ctx);
        });

    let password_state = Rc::clone(&state);
    let password = PasswordInput::new("Password")
        .placeholder("At least eight characters")
        .min_width(360.0)
        .theme(theme)
        .on_change_with_ctx(move |ctx, value| {
            password_state.borrow_mut().password = value;
            refresh_window(ctx);
        });

    let initial_schedule = state.borrow().scheduled_for.clone();
    let schedule_state = Rc::clone(&state);
    let scheduled_for = DateTimeInput::new("Scheduled for")
        .value(initial_schedule)
        .min_width(360.0)
        .theme(theme)
        .on_change_with_ctx(move |ctx, value| {
            schedule_state.borrow_mut().scheduled_for = value;
            refresh_window(ctx);
        });

    let status_state = Rc::clone(&state);
    let status = Label::dynamic("Draft", move || status_state.borrow().status())
        .theme(theme)
        .color(theme.palette.text_muted);

    let enabled_state = Rc::clone(&state);
    let submit_state = Rc::clone(&state);
    let submit = Button::new("Save profile")
        .theme(theme)
        .enabled_when(move || enabled_state.borrow().can_submit())
        .on_press_with_ctx(move |ctx| {
            submit_state.borrow_mut().submissions += 1;
            refresh_window(ctx);
        });

    let form = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Start)
        .with_child(
            Label::new("Create a profile")
                .theme(theme)
                .font_size(26.0)
                .line_height(32.0)
                .color(theme.palette.text),
        )
        .with_child(
            Label::new("Enter a name, password, and local appointment time.")
                .theme(theme)
                .color(theme.palette.text_muted),
        )
        .with_child(name)
        .with_child(password)
        .with_child(scheduled_for)
        .with_child(submit)
        .with_child(status);

    let card = Surface::panel(form).theme(theme).padding(Insets::all(20.0));
    let root = Surface::window(card)
        .theme(theme)
        .padding(Insets::all(24.0))
        .fill();

    App::new()
        .window(Window::new("Stateful SUI Form").root(root))
        .run()
}
