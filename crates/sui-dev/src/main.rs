use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{Rect, SemanticsNode, SemanticsRole, WidgetPodMutVisitor, WidgetPodVisitor};

fn main() -> Result<()> {
    run_desktop_app()
}

fn run_desktop_app() -> Result<()> {
    build_gallery_application(default_gallery_state()).run()
}

#[derive(Debug, Clone, Default)]
struct GalleryState {
    name: String,
    subscribed: bool,
    button_presses: usize,
}

struct GalleryRoot {
    child: SingleChild,
}

impl GalleryRoot {
    fn new(state: Rc<RefCell<GalleryState>>) -> Self {
        Self {
            child: SingleChild::new(build_gallery(state)),
        }
    }
}

fn default_gallery_state() -> Rc<RefCell<GalleryState>> {
    Rc::new(RefCell::new(GalleryState {
        name: "Ada".to_string(),
        subscribed: true,
        button_presses: 0,
    }))
}

fn build_gallery_application(state: Rc<RefCell<GalleryState>>) -> Application {
    Application::new().window(
        WindowBuilder::new()
            .title("SUI Widget Gallery")
            .root(GalleryRoot::new(state)),
    )
}

impl Widget for GalleryRoot {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(1280.0, 720.0));
        self.child
            .layout_at(ctx, Constraints::tight(viewport), Point::ZERO);
        viewport
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.06, 0.07, 0.09, 1.0));
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some("SUI Widget Gallery".to_string());
        root.description =
            Some("Development gallery for common built-in widgets in sui-widgets".to_string());
        ctx.push(root);
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

fn build_gallery(state: Rc<RefCell<GalleryState>>) -> impl Widget {
    let initial_name = state.borrow().name.clone();
    let initial_subscribed = state.borrow().subscribed;

    let name_state = Rc::clone(&state);
    let subscribed_state = Rc::clone(&state);
    let action_state = Rc::clone(&state);

    Padding::all(
        24.0,
        Stack::vertical()
            .spacing(18.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Stack::vertical()
                    .spacing(6.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new("SUI Widget Gallery")
                            .font_size(30.0)
                            .line_height(34.0)
                            .color(Color::rgba(0.96, 0.97, 0.99, 1.0)),
                    )
                    .with_child(
                        Label::new(
                            "First pass of common built-in controls: text, button, checkbox, and text input. Layout still comes from the existing container primitives.",
                        )
                        .font_size(15.0)
                        .line_height(20.0)
                        .color(Color::rgba(0.72, 0.77, 0.84, 1.0)),
                    ),
            )
            .with_child(panel(
                "Common controls",
                "These are the widgets implemented in sui-widgets first because they fit the current retained runtime, semantics model, and event surface cleanly.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        TextInput::new("Name")
                            .value(initial_name)
                            .placeholder("Type your name")
                            .on_change(move |value| {
                                name_state.borrow_mut().name = value;
                            }),
                    )
                    .with_child(
                        Checkbox::new("Subscribe to product updates")
                            .checked(initial_subscribed)
                            .on_toggle(move |checked| {
                                subscribed_state.borrow_mut().subscribed = checked;
                            }),
                    )
                    .with_child(
                        Stack::horizontal()
                            .spacing(12.0)
                            .alignment(Alignment::Center)
                            .with_child(
                                SizedBox::new().width(180.0).with_child(
                                    Button::new("Trigger action").on_press(move || {
                                        action_state.borrow_mut().button_presses += 1;
                                    }),
                                ),
                            )
                            .with_child(
                                Label::new(
                                    "The button updates shared gallery state. The input and checkbox also push state changes through callbacks.",
                                )
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.66, 0.71, 0.78, 1.0)),
                            ),
                    )
                    .with_child(
                        Label::new(
                            "Text input currently supports focus, direct keyboard typing, IME composition commits, placeholder rendering, and backspace. That is intentionally minimal for this phase.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(Color::rgba(0.59, 0.65, 0.73, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Typography",
                "Static text is now a real widget too, so the dev host no longer needs to hand-paint every heading and caption.",
                Stack::vertical()
                    .spacing(8.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Label::new("Section heading")
                            .font_size(22.0)
                            .line_height(26.0)
                            .color(Color::rgba(0.93, 0.95, 0.99, 1.0)),
                    )
                    .with_child(
                        Label::new("Body copy can use the same widget with different size and color settings.")
                            .font_size(15.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.73, 0.78, 0.85, 1.0)),
                    )
                    .with_child(
                        Label::new("Secondary note")
                            .font_size(13.0)
                            .line_height(18.0)
                            .color(Color::rgba(0.56, 0.62, 0.70, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Live state",
                "This summary is still a small custom widget in sui-dev, but it now reads state produced by reusable controls from sui-widgets.",
                GallerySummary::new(state),
            )),
    )
}

fn panel<W>(title: &str, subtitle: &str, body: W) -> impl Widget
where
    W: Widget + 'static,
{
    Background::new(
        Color::rgba(0.11, 0.13, 0.18, 1.0),
        Padding::all(
            18.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(title)
                        .font_size(20.0)
                        .line_height(24.0)
                        .color(Color::rgba(0.95, 0.97, 0.99, 1.0)),
                )
                .with_child(
                    Label::new(subtitle)
                        .font_size(14.0)
                        .line_height(19.0)
                        .color(Color::rgba(0.69, 0.75, 0.82, 1.0)),
                )
                .with_child(body),
        ),
    )
}

struct GallerySummary {
    state: Rc<RefCell<GalleryState>>,
}

impl GallerySummary {
    fn new(state: Rc<RefCell<GalleryState>>) -> Self {
        Self { state }
    }
}

impl Widget for GallerySummary {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            320.0
        };
        constraints.clamp(Size::new(width, 116.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        let greeting = if state.name.trim().is_empty() {
            "Hello, stranger".to_string()
        } else {
            format!("Hello, {}", state.name)
        };
        let subscription = if state.subscribed {
            "Subscribed"
        } else {
            "Not subscribed"
        };

        ctx.fill_bounds(Color::rgba(0.08, 0.10, 0.14, 1.0));
        ctx.stroke_bounds(Color::rgba(0.22, 0.30, 0.43, 1.0), StrokeStyle::new(1.0));
        ctx.label(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 14.0,
                ctx.bounds().width() - 28.0,
                24.0,
            ),
            greeting,
            Color::rgba(0.95, 0.97, 0.99, 1.0),
        );
        ctx.label(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 48.0,
                ctx.bounds().width() - 28.0,
                20.0,
            ),
            format!("button presses: {}", state.button_presses),
            Color::rgba(0.72, 0.78, 0.85, 1.0),
        );
        ctx.label(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 74.0,
                ctx.bounds().width() - 28.0,
                20.0,
            ),
            format!("subscription: {}", subscription),
            Color::rgba(0.72, 0.78, 0.85, 1.0),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = self.state.borrow();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some("Gallery summary".to_string());
        node.description = Some(format!(
            "name: {}; subscription: {}; button presses: {}",
            if state.name.is_empty() {
                "stranger"
            } else {
                state.name.as_str()
            },
            if state.subscribed { "on" } else { "off" },
            state.button_presses,
        ));
        ctx.push(node);
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};

    use super::{GalleryState, build_gallery_application, default_gallery_state};
    use sui::prelude::Result;
    use sui::{SemanticsRole, SemanticsValue};
    use sui_testing::prelude::TestApp;

    #[test]
    fn gallery_text_input_accepts_plain_keyboard_typing() -> Result<()> {
        let state = Rc::new(RefCell::new(GalleryState::default()));
        let app = TestApp::from_runtime(build_gallery_application(Rc::clone(&state)).build()?)?;
        let window = app.main_window()?;

        let input = window.get_by_role(SemanticsRole::TextInput).with_name("Name");
        input.focus()?;
        input.press("A")?;
        input.press("d")?;
        input.press("a")?;
        input.expect().to_have_value("Ada")?;

        assert_eq!(state.borrow().name, "Ada");

        let snapshot = window.snapshot()?;
        let summary = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Gallery summary")
            })
            .expect("gallery summary semantics node present");
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("Ada"))
        );

        Ok(())
    }

    #[test]
    fn gallery_can_write_visual_artifacts_for_inspection() -> Result<()> {
        let artifact_root = gallery_artifact_root();
        reset_dir(&artifact_root)?;

        let default_app =
            TestApp::from_runtime(build_gallery_application(default_gallery_state()).build()?)?;
        let default_window = default_app.main_window()?;
        let initial_dir = artifact_root.join("initial");
        let initial_artifacts = default_window.capture_artifacts()?;
        initial_artifacts.write_to_dir(&initial_dir)?;

        let configured_state = Rc::new(RefCell::new(GalleryState {
            name: "Grace Hopper".to_string(),
            subscribed: false,
            button_presses: 1,
        }));
        let configured_app =
            TestApp::from_runtime(build_gallery_application(configured_state).build()?)?;
        let configured_window = configured_app.main_window()?;
        let configured_dir = artifact_root.join("configured");
        let configured_artifacts = configured_window.capture_artifacts()?;
        configured_artifacts.write_to_dir(&configured_dir)?;

        let summary = configured_artifacts
            .snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Gallery summary")
            })
            .expect("gallery summary semantics node present");
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("Grace Hopper"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("off"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("1"))
        );

        let input = configured_artifacts
            .snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput && node.name.as_deref() == Some("Name")
            })
            .expect("name input semantics node present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("Grace Hopper".to_string()))
        );

        assert!(initial_dir.join("summary.txt").exists());
        assert!(initial_dir.join("screenshot.png").exists());
        assert!(configured_dir.join("summary.txt").exists());
        assert!(configured_dir.join("screenshot.png").exists());

        Ok(())
    }

    fn gallery_artifact_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("ui-artifacts")
            .join("sui-dev-gallery")
    }

    fn reset_dir(path: &PathBuf) -> Result<()> {
        if path.exists() {
            fs::remove_dir_all(path).map_err(|error| {
                sui::Error::new(format!("failed to clear {}: {error}", path.display()))
            })?;
        }
        fs::create_dir_all(path).map_err(|error| {
            sui::Error::new(format!("failed to create {}: {error}", path.display()))
        })?;
        Ok(())
    }
}
