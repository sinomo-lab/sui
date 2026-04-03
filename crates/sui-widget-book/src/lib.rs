#![forbid(unsafe_code)]

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{Rect, SemanticsNode, SemanticsRole, WidgetPodMutVisitor, WidgetPodVisitor};

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";
pub const NAME_INPUT_LABEL: &str = "Name";
pub const SUBSCRIBE_LABEL: &str = "Subscribe to product updates";
pub const PRIMARY_BUTTON_LABEL: &str = "Trigger action";
pub const SUMMARY_NAME: &str = "Widget book summary";

#[derive(Debug, Clone, Default)]
pub struct WidgetBookState {
    pub name: String,
    pub subscribed: bool,
    pub button_presses: usize,
}

struct WidgetBookRoot {
    child: SingleChild,
}

impl WidgetBookRoot {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        Self {
            child: SingleChild::new(build_widget_book(state)),
        }
    }
}

pub fn default_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: "Ada".to_string(),
        subscribed: true,
        button_presses: 0,
    }))
}

pub fn build_widget_book_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    Application::new().window(
        WindowBuilder::new()
            .title(WINDOW_TITLE)
            .root(WidgetBookRoot::new(state)),
    )
}

pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application(default_widget_book_state()).run()
}

impl Widget for WidgetBookRoot {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(1280.0, 720.0));
        self.child
            .layout_at(ctx, Constraints::tight(viewport), Point::ZERO);
        viewport
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.95, 0.968, 0.985, 1.0));
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(WINDOW_TITLE.to_string());
        root.description = Some(WINDOW_DESCRIPTION.to_string());
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

fn build_widget_book(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
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
                        Label::new(WINDOW_TITLE)
                            .font_size(30.0)
                            .line_height(34.0)
                            .color(Color::rgba(0.10, 0.14, 0.20, 1.0)),
                    )
                    .with_child(
                        Label::new(
                            "A dedicated widget book for exercising built-in controls, generating inspection artifacts, and providing stable screenshot stories.",
                        )
                        .font_size(15.0)
                        .line_height(20.0)
                        .color(Color::rgba(0.40, 0.48, 0.58, 1.0)),
                    ),
            )
            .with_child(panel(
                "Common controls",
                "These defaults should feel contemporary and light, while still staying dense enough for inspectors, toolbars, and side panels.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        TextInput::new(NAME_INPUT_LABEL)
                            .value(initial_name)
                            .placeholder("Type your name")
                            .on_change(move |value| {
                                name_state.borrow_mut().name = value;
                            }),
                    )
                    .with_child(
                        Checkbox::new(SUBSCRIBE_LABEL)
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
                                    Button::new(PRIMARY_BUTTON_LABEL).on_press(move || {
                                        action_state.borrow_mut().button_presses += 1;
                                    }),
                                ),
                            )
                            .with_child(
                                Label::new(
                                    "Primary actions, boolean toggles, and text fields should feel related by default instead of looking like separate experiments.",
                                )
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                            ),
                    )
                    .with_child(
                        Label::new(
                            "The widget book tests capture these controls directly so visual regressions can be reviewed manually or compared automatically.",
                        )
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
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
                            .color(Color::rgba(0.13, 0.17, 0.23, 1.0)),
                    )
                    .with_child(
                        Label::new("Body copy can use the same widget with different size and color settings.")
                            .font_size(15.0)
                            .line_height(20.0)
                            .color(Color::rgba(0.38, 0.46, 0.56, 1.0)),
                    )
                    .with_child(
                        Label::new("Secondary note")
                            .font_size(13.0)
                            .line_height(18.0)
                            .color(Color::rgba(0.50, 0.57, 0.66, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Live state",
                "This summary reads state produced by reusable controls so screenshot stories can cover both isolated widgets and composed UI.",
                WidgetBookSummary::new(state),
            )),
    )
}

fn panel<W>(title: &str, subtitle: &str, body: W) -> impl Widget
where
    W: Widget + 'static,
{
    Background::new(
        Color::rgba(0.985, 0.99, 1.0, 1.0),
        Padding::all(
            18.0,
            Stack::vertical()
                .spacing(10.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new(title)
                        .font_size(20.0)
                        .line_height(24.0)
                        .color(Color::rgba(0.11, 0.15, 0.21, 1.0)),
                )
                .with_child(
                    Label::new(subtitle)
                        .font_size(14.0)
                        .line_height(19.0)
                        .color(Color::rgba(0.44, 0.51, 0.60, 1.0)),
                )
                .with_child(body),
        ),
    )
}

struct WidgetBookSummary {
    state: Rc<RefCell<WidgetBookState>>,
}

impl WidgetBookSummary {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        Self { state }
    }
}

impl Widget for WidgetBookSummary {
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

        ctx.fill_bounds(Color::rgba(0.985, 0.99, 1.0, 1.0));
        ctx.stroke_bounds(Color::rgba(0.80, 0.85, 0.91, 1.0), StrokeStyle::new(1.0));
        ctx.label(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 14.0,
                ctx.bounds().width() - 28.0,
                24.0,
            ),
            greeting,
            Color::rgba(0.11, 0.15, 0.21, 1.0),
        );
        ctx.label(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 48.0,
                ctx.bounds().width() - 28.0,
                20.0,
            ),
            format!("button presses: {}", state.button_presses),
            Color::rgba(0.41, 0.49, 0.58, 1.0),
        );
        ctx.label(
            Rect::new(
                ctx.bounds().x() + 14.0,
                ctx.bounds().y() + 74.0,
                ctx.bounds().width() - 28.0,
                20.0,
            ),
            format!("subscription: {}", subscription),
            Color::rgba(0.41, 0.49, 0.58, 1.0),
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = self.state.borrow();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(SUMMARY_NAME.to_string());
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
    use std::{cell::RefCell, env, fs, path::Path, path::PathBuf, rc::Rc};

    use super::{
        NAME_INPUT_LABEL, PRIMARY_BUTTON_LABEL, SUBSCRIBE_LABEL, SUMMARY_NAME, WidgetBookState,
        build_widget_book_application, default_widget_book_state,
    };
    use sui::{
        Error, Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Result,
        SemanticsRole, SemanticsValue,
    };
    use sui_testing::prelude::*;

    #[derive(Clone, Copy)]
    enum StoryCase {
        Overview,
        OverviewConfigured,
        Button,
        ButtonHover,
        ButtonPressed,
        Checkbox,
        CheckboxUnchecked,
        FilledInput,
        EmptyInputFocused,
        Summary,
    }

    impl StoryCase {
        const ALL: [Self; 10] = [
            Self::Overview,
            Self::OverviewConfigured,
            Self::Button,
            Self::ButtonHover,
            Self::ButtonPressed,
            Self::Checkbox,
            Self::CheckboxUnchecked,
            Self::FilledInput,
            Self::EmptyInputFocused,
            Self::Summary,
        ];

        fn id(self) -> &'static str {
            match self {
                Self::Overview => "overview",
                Self::OverviewConfigured => "overview-configured",
                Self::Button => "button",
                Self::ButtonHover => "button-hover",
                Self::ButtonPressed => "button-pressed",
                Self::Checkbox => "checkbox",
                Self::CheckboxUnchecked => "checkbox-unchecked",
                Self::FilledInput => "filled-input",
                Self::EmptyInputFocused => "empty-input-focused",
                Self::Summary => "summary",
            }
        }

        fn description(self) -> &'static str {
            match self {
                Self::Overview => "Whole-window widget book overview screenshot.",
                Self::OverviewConfigured => {
                    "Whole-window widget book overview with configured state changes."
                }
                Self::Button => "Primary button crop for direct visual regression review.",
                Self::ButtonHover => "Primary button crop in the hovered state.",
                Self::ButtonPressed => "Primary button crop while the pointer is held down.",
                Self::Checkbox => "Checkbox crop in the checked default state.",
                Self::CheckboxUnchecked => "Checkbox crop in the unchecked configured state.",
                Self::FilledInput => {
                    "Text input crop with a configured value for text rendering checks."
                }
                Self::EmptyInputFocused => {
                    "Empty text input crop with focus ring and placeholder visible."
                }
                Self::Summary => "Composed summary panel showing derived state.",
            }
        }

        fn build_app(self) -> Result<TestApp> {
            let state = match self {
                Self::Overview
                | Self::Button
                | Self::ButtonHover
                | Self::ButtonPressed
                | Self::Checkbox => default_widget_book_state(),
                Self::OverviewConfigured
                | Self::CheckboxUnchecked
                | Self::FilledInput
                | Self::Summary => configured_widget_book_state(),
                Self::EmptyInputFocused => blank_widget_book_state(),
            };

            TestApp::from_runtime(build_widget_book_application(state).build()?)
        }

        fn prepare(self, window: &TestWindow) -> Result<()> {
            match self {
                Self::ButtonHover => self.target(window).hover(),
                Self::ButtonPressed => {
                    press_target(window, SemanticsRole::Button, PRIMARY_BUTTON_LABEL)
                }
                Self::EmptyInputFocused => self.target(window).focus(),
                Self::Overview
                | Self::OverviewConfigured
                | Self::Button
                | Self::Checkbox
                | Self::CheckboxUnchecked
                | Self::FilledInput
                | Self::Summary => Ok(()),
            }
        }

        fn target(self, window: &TestWindow) -> Locator {
            match self {
                Self::Overview | Self::OverviewConfigured => window.root(),
                Self::Button | Self::ButtonHover | Self::ButtonPressed => window
                    .get_by_role(SemanticsRole::Button)
                    .with_name(PRIMARY_BUTTON_LABEL),
                Self::Checkbox | Self::CheckboxUnchecked => window
                    .get_by_role(SemanticsRole::CheckBox)
                    .with_name(SUBSCRIBE_LABEL),
                Self::FilledInput | Self::EmptyInputFocused => window
                    .get_by_role(SemanticsRole::TextInput)
                    .with_name(NAME_INPUT_LABEL),
                Self::Summary => window
                    .get_by_role(SemanticsRole::GenericContainer)
                    .with_name(SUMMARY_NAME),
            }
        }
    }

    #[test]
    fn widget_book_text_input_accepts_plain_keyboard_typing() -> Result<()> {
        let state = Rc::new(RefCell::new(WidgetBookState::default()));
        let app = TestApp::from_runtime(build_widget_book_application(Rc::clone(&state)).build()?)?;
        let window = app.main_window()?;

        let input = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(NAME_INPUT_LABEL);
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
                    && node.name.as_deref() == Some(SUMMARY_NAME)
            })
            .expect("widget book summary semantics node present");
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("Ada"))
        );

        Ok(())
    }

    #[test]
    fn widget_book_generates_visual_artifacts() -> Result<()> {
        let artifact_root = artifact_root();
        reset_dir(&artifact_root)?;

        for story in StoryCase::ALL {
            let story_dir = artifact_root.join(story.id());
            create_dir(&story_dir)?;

            let app = story.build_app()?;
            let window = app.main_window()?;
            story.prepare(&window)?;
            let artifacts = window.capture_artifacts()?;
            artifacts.write_to_dir(&story_dir)?;
            rename_window_artifacts(&story_dir)?;

            let locator = story.target(&window);
            let screenshot = locator.capture_screenshot()?;
            screenshot.write_png(story_dir.join("screenshot.png"))?;
            write_text(story_dir.join("story.txt"), story.description())?;
        }

        for story in StoryCase::ALL {
            assert!(
                artifact_root
                    .join(story.id())
                    .join("screenshot.png")
                    .exists()
            );
        }

        Ok(())
    }

    #[test]
    fn widget_book_stories_match_baselines_when_present() -> Result<()> {
        let baseline_root = baseline_root();
        let candidate_root = artifact_root().join("baseline-candidates");
        let update_baselines = env_flag("SUI_WIDGET_BOOK_UPDATE_BASELINES");
        let require_baselines = env_flag("SUI_WIDGET_BOOK_REQUIRE_BASELINES");

        create_dir(&baseline_root)?;
        create_dir(&candidate_root)?;

        for story in StoryCase::ALL {
            let app = story.build_app()?;
            let window = app.main_window()?;
            story.prepare(&window)?;
            let locator = story.target(&window);
            let baseline = baseline_root.join(format!("{}.png", story.id()));
            let candidate = candidate_root.join(format!("{}.png", story.id()));

            locator.capture_screenshot()?.write_png(&candidate)?;

            if update_baselines {
                locator.capture_screenshot()?.write_png(&baseline)?;
                continue;
            }

            if baseline.exists() {
                locator.expect().to_match_screenshot(&baseline)?;
                continue;
            }

            if require_baselines {
                return Err(Error::new(format!(
                    "missing widget book baseline {}",
                    baseline.display()
                )));
            }
        }

        Ok(())
    }

    #[test]
    fn widget_book_configured_story_exposes_expected_semantics() -> Result<()> {
        let app = StoryCase::Summary.build_app()?;
        let window = app.main_window()?;
        let snapshot = window.snapshot()?;

        let summary = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(SUMMARY_NAME)
            })
            .expect("widget book summary semantics node present");
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

        let input = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(NAME_INPUT_LABEL)
            })
            .expect("name input semantics node present");
        assert_eq!(
            input.value,
            Some(SemanticsValue::Text("Grace Hopper".to_string()))
        );

        Ok(())
    }

    fn configured_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
        Rc::new(RefCell::new(WidgetBookState {
            name: "Grace Hopper".to_string(),
            subscribed: false,
            button_presses: 1,
        }))
    }

    fn blank_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
        Rc::new(RefCell::new(WidgetBookState {
            name: String::new(),
            subscribed: false,
            button_presses: 0,
        }))
    }

    fn artifact_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("ui-artifacts")
            .join("sui-widget-book")
    }

    fn baseline_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("baselines")
    }

    fn env_flag(name: &str) -> bool {
        env::var(name)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    fn reset_dir(path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_dir_all(path).map_err(|error| {
                Error::new(format!("failed to clear {}: {error}", path.display()))
            })?;
        }
        create_dir(path)
    }

    fn create_dir(path: &Path) -> Result<()> {
        fs::create_dir_all(path)
            .map_err(|error| Error::new(format!("failed to create {}: {error}", path.display())))
    }

    fn write_text(path: PathBuf, contents: &str) -> Result<()> {
        fs::write(&path, contents)
            .map_err(|error| Error::new(format!("failed to write {}: {error}", path.display())))
    }

    fn rename_window_artifacts(dir: &Path) -> Result<()> {
        rename_if_exists(dir, "screenshot.png", "window.png")?;
        rename_if_exists(dir, "semantics-overlay.png", "window-semantics-overlay.png")?;
        rename_if_exists(dir, "widget-overlay.png", "window-widget-overlay.png")
    }

    fn rename_if_exists(dir: &Path, from: &str, to: &str) -> Result<()> {
        let from_path = dir.join(from);
        if !from_path.exists() {
            return Ok(());
        }

        let to_path = dir.join(to);
        if to_path.exists() {
            fs::remove_file(&to_path).map_err(|error| {
                Error::new(format!("failed to remove {}: {error}", to_path.display()))
            })?;
        }

        fs::rename(&from_path, &to_path).map_err(|error| {
            Error::new(format!("failed to rename {}: {error}", from_path.display()))
        })
    }

    fn press_target(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
        let locator = window.get_by_role(role.clone()).with_name(name);
        let point = node_center(window, role, name)?;

        locator.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            point,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        locator.dispatch_event(Event::Pointer(down))
    }

    fn node_center(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<Point> {
        let snapshot = window.snapshot()?;
        let node = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .ok_or_else(|| Error::new(format!("missing story node {:?} {name}", role)))?;

        Ok(Point::new(
            node.bounds.x() + (node.bounds.width() / 2.0),
            node.bounds.y() + (node.bounds.height() / 2.0),
        ))
    }
}
