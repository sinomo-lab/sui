#![forbid(unsafe_code)]

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{Rect, SemanticsNode, SemanticsRole, WidgetPodMutVisitor, WidgetPodVisitor};

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";
pub const NAME_INPUT_LABEL: &str = "Name";
pub const TEXT_AREA_LABEL: &str = "Notes";
pub const SUBSCRIBE_LABEL: &str = "Subscribe to product updates";
pub const PRIMARY_BUTTON_LABEL: &str = "Trigger action";
pub const ICON_LABEL: &str = "Search icon";
pub const ICON_BUTTON_LABEL: &str = "More actions";
pub const SWITCH_LABEL: &str = "Enable snapping";
pub const RADIO_BUTTON_LABEL: &str = "Standalone radio sample";
pub const RADIO_GROUP_NAME: &str = "Render quality";
pub const SLIDER_NAME: &str = "Opacity";
pub const NUMBER_INPUT_NAME: &str = "Brush size";
pub const SELECT_NAME: &str = "Blend mode";
pub const SUMMARY_NAME: &str = "Widget book summary";

const RADIO_OPTIONS: [&str; 3] = ["Balanced", "High", "Fast"];
const BLEND_MODE_OPTIONS: [&str; 4] = ["Normal", "Multiply", "Screen", "Overlay"];

#[derive(Debug, Clone, Default)]
pub struct WidgetBookState {
    pub name: String,
    pub subscribed: bool,
    pub button_presses: usize,
    pub icon_button_presses: usize,
    pub switch_on: bool,
    pub standalone_radio_selected: bool,
    pub radio_choice: String,
    pub slider_value: f64,
    pub number_value: f64,
    pub notes: String,
    pub mode: String,
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
        icon_button_presses: 0,
        switch_on: true,
        standalone_radio_selected: false,
        radio_choice: RADIO_OPTIONS[0].to_string(),
        slider_value: 72.0,
        number_value: 12.0,
        notes: "Pinned notes for inspector workflows.\nSupports multiline editing.".to_string(),
        mode: BLEND_MODE_OPTIONS[0].to_string(),
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
    let snapshot = state.borrow().clone();
    let initial_name = snapshot.name.clone();
    let initial_notes = snapshot.notes.clone();
    let initial_subscribed = snapshot.subscribed;
    let initial_switch_on = snapshot.switch_on;
    let initial_standalone_radio = snapshot.standalone_radio_selected;
    let initial_slider_value = snapshot.slider_value;
    let initial_number_value = snapshot.number_value;
    let initial_radio_choice = snapshot.radio_choice.clone();
    let initial_mode = snapshot.mode.clone();

    let name_state = Rc::clone(&state);
    let subscribed_state = Rc::clone(&state);
    let action_state = Rc::clone(&state);
    let icon_action_state = Rc::clone(&state);
    let switch_state = Rc::clone(&state);
    let radio_button_state = Rc::clone(&state);
    let radio_group_state = Rc::clone(&state);
    let slider_state = Rc::clone(&state);
    let number_state = Rc::clone(&state);
    let notes_state = Rc::clone(&state);
    let select_state = Rc::clone(&state);

    ScrollView::vertical(Padding::all(
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
                        SizedBox::new().width(320.0).with_child(
                            TextInput::new(NAME_INPUT_LABEL)
                                .value(initial_name)
                                .placeholder("Type your name")
                                .on_change(move |value| {
                                    name_state.borrow_mut().name = value;
                                }),
                        ),
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
                "Toolbar pieces",
                "Compact controls, separators, and icons need to feel intentional before any themed application shell exists.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Stack::horizontal()
                            .spacing(14.0)
                            .alignment(Alignment::Center)
                            .with_child(Icon::new(IconGlyph::Search).label(ICON_LABEL).size(24.0))
                            .with_child(
                                IconButton::new(IconGlyph::MoreHorizontal, ICON_BUTTON_LABEL)
                                    .on_press(move || {
                                        icon_action_state.borrow_mut().icon_button_presses += 1;
                                    }),
                            )
                            .with_child(
                                Label::new(
                                    "Icons and icon buttons round out dense toolbar layouts.",
                                )
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                            ),
                    )
                    .with_child(SizedBox::new().width(260.0).with_child(
                        Separator::horizontal().inset(12.0),
                    )),
            ))
            .with_child(panel(
                "Choices and ranges",
                "Desktop-style inspectors rely on switches, radio groups, sliders, numeric inputs, and selects more than oversized form controls.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Switch::new(SWITCH_LABEL)
                            .on(initial_switch_on)
                            .on_toggle(move |checked| {
                                switch_state.borrow_mut().switch_on = checked;
                            }),
                    )
                    .with_child(
                        RadioButton::new(RADIO_BUTTON_LABEL)
                            .selected(initial_standalone_radio)
                            .on_select(move || {
                                radio_button_state.borrow_mut().standalone_radio_selected = true;
                            }),
                    )
                    .with_child(
                        SizedBox::new().width(280.0).with_child(
                            RadioGroup::new(RADIO_GROUP_NAME)
                                .options(RADIO_OPTIONS)
                                .selected(option_index(&RADIO_OPTIONS, &initial_radio_choice).unwrap_or(0))
                                .on_change(move |_, value| {
                                    radio_group_state.borrow_mut().radio_choice = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            Slider::new(SLIDER_NAME)
                                .range(0.0, 100.0)
                                .step(1.0)
                                .value(initial_slider_value)
                                .on_change(move |value| {
                                    slider_state.borrow_mut().slider_value = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            NumberInput::new(NUMBER_INPUT_NAME)
                                .range(1.0, 256.0)
                                .step(1.0)
                                .precision(0)
                                .value(initial_number_value)
                                .on_change(move |value| {
                                    number_state.borrow_mut().number_value = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(260.0).with_child(
                            Select::new(SELECT_NAME)
                                .placeholder("Choose blend mode")
                                .options(BLEND_MODE_OPTIONS)
                                .selected(option_index(&BLEND_MODE_OPTIONS, &initial_mode).unwrap_or(0))
                                .on_change(move |_, value| {
                                    select_state.borrow_mut().mode = value;
                                }),
                        ),
                    ),
            ))
            .with_child(panel(
                "Multiline and scroll",
                "The widget book itself now scrolls, and the multiline editor fills the long-form text entry gap for notes, JSON, and small scripting panes.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(420.0).with_child(
                            TextArea::new(TEXT_AREA_LABEL)
                                .min_height(150.0)
                                .value(initial_notes)
                                .placeholder("Write notes")
                                .on_change(move |value| {
                                    notes_state.borrow_mut().notes = value;
                                }),
                        ),
                    )
                    .with_child(
                        Label::new(
                            "Use PageDown on the outer scroll view story to capture the lower panels and prove the gallery exceeds the viewport.",
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
    ))
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
        constraints.clamp(Size::new(width, 210.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        let lines = [
            if state.name.trim().is_empty() {
                "Hello, stranger".to_string()
            } else {
                format!("Hello, {}", state.name)
            },
            format!(
                "buttons: primary={} icon={}",
                state.button_presses, state.icon_button_presses
            ),
            format!(
                "subscription: {} | snapping: {}",
                if state.subscribed { "on" } else { "off" },
                if state.switch_on { "on" } else { "off" }
            ),
            format!(
                "radio: standalone={} group={}",
                if state.standalone_radio_selected {
                    "selected"
                } else {
                    "idle"
                },
                if state.radio_choice.is_empty() {
                    "unset"
                } else {
                    state.radio_choice.as_str()
                }
            ),
            format!(
                "opacity: {:.0} | brush size: {:.0} | mode: {}",
                state.slider_value,
                state.number_value,
                if state.mode.is_empty() {
                    "unset"
                } else {
                    state.mode.as_str()
                }
            ),
            format!("notes lines: {}", state.notes.lines().count().max(1)),
        ];

        ctx.fill_bounds(Color::rgba(0.985, 0.99, 1.0, 1.0));
        ctx.stroke_bounds(Color::rgba(0.80, 0.85, 0.91, 1.0), StrokeStyle::new(1.0));
        for (index, line) in lines.into_iter().enumerate() {
            ctx.label(
                Rect::new(
                    ctx.bounds().x() + 14.0,
                    ctx.bounds().y() + 14.0 + (index as f32 * 28.0),
                    ctx.bounds().width() - 28.0,
                    22.0,
                ),
                line,
                if index == 0 {
                    Color::rgba(0.11, 0.15, 0.21, 1.0)
                } else {
                    Color::rgba(0.41, 0.49, 0.58, 1.0)
                },
            );
        }
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
            "name: {}; subscription: {}; button presses: {}; icon actions: {}; switch: {}; standalone radio: {}; radio choice: {}; slider: {:.0}; brush size: {:.0}; mode: {}; notes lines: {}",
            if state.name.is_empty() {
                "stranger"
            } else {
                state.name.as_str()
            },
            if state.subscribed { "on" } else { "off" },
            state.button_presses,
            state.icon_button_presses,
            if state.switch_on { "on" } else { "off" },
            if state.standalone_radio_selected {
                "selected"
            } else {
                "off"
            },
            if state.radio_choice.is_empty() {
                "unset"
            } else {
                state.radio_choice.as_str()
            },
            state.slider_value,
            state.number_value,
            if state.mode.is_empty() {
                "unset"
            } else {
                state.mode.as_str()
            },
            state.notes.lines().count().max(1),
        ));
        ctx.push(node);
    }
}

fn option_index(options: &[&str], value: &str) -> Option<usize> {
    options.iter().position(|option| *option == value)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, env, fs, path::Path, path::PathBuf, rc::Rc};

    use super::{
        ICON_BUTTON_LABEL, ICON_LABEL, NAME_INPUT_LABEL, NUMBER_INPUT_NAME, PRIMARY_BUTTON_LABEL,
        RADIO_BUTTON_LABEL, RADIO_GROUP_NAME, SELECT_NAME, SLIDER_NAME, SUBSCRIBE_LABEL,
        SUMMARY_NAME, SWITCH_LABEL, TEXT_AREA_LABEL, WidgetBookState,
        build_widget_book_application, default_widget_book_state,
    };
    use sui::{
        Error, Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
        Result, SemanticsRole, SemanticsValue,
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
        Icon,
        IconButton,
        Separator,
        Switch,
        RadioButton,
        RadioGroup,
        Slider,
        NumberInput,
        TextArea,
        SelectExpanded,
        ScrollViewScrolled,
        Summary,
    }

    impl StoryCase {
        const ALL: [Self; 21] = [
            Self::Overview,
            Self::OverviewConfigured,
            Self::Button,
            Self::ButtonHover,
            Self::ButtonPressed,
            Self::Checkbox,
            Self::CheckboxUnchecked,
            Self::FilledInput,
            Self::EmptyInputFocused,
            Self::Icon,
            Self::IconButton,
            Self::Separator,
            Self::Switch,
            Self::RadioButton,
            Self::RadioGroup,
            Self::Slider,
            Self::NumberInput,
            Self::TextArea,
            Self::SelectExpanded,
            Self::ScrollViewScrolled,
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
                Self::Icon => "icon",
                Self::IconButton => "icon-button",
                Self::Separator => "separator",
                Self::Switch => "switch",
                Self::RadioButton => "radio-button",
                Self::RadioGroup => "radio-group",
                Self::Slider => "slider",
                Self::NumberInput => "number-input",
                Self::TextArea => "text-area",
                Self::SelectExpanded => "select-expanded",
                Self::ScrollViewScrolled => "scroll-view-scrolled",
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
                Self::Icon => "Standalone icon crop for compact toolbar glyph review.",
                Self::IconButton => "Icon button crop for titlebar-style actions.",
                Self::Separator => "Separator crop for toolbar and inspector dividers.",
                Self::Switch => "Switch crop for boolean controls distinct from checkbox rows.",
                Self::RadioButton => "Standalone radio button crop.",
                Self::RadioGroup => "Radio group crop for mutually exclusive choices.",
                Self::Slider => "Slider crop for numeric tuning controls.",
                Self::NumberInput => "Number input crop for spinbox-style editing.",
                Self::TextArea => "Text area crop with multiline content.",
                Self::SelectExpanded => "Expanded select crop showing compact option picking.",
                Self::ScrollViewScrolled => {
                    "Outer widget-book scroll view after paging down through the gallery."
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
                | Self::Checkbox
                | Self::Icon
                | Self::IconButton
                | Self::Separator
                | Self::Switch
                | Self::RadioButton
                | Self::RadioGroup
                | Self::Slider
                | Self::NumberInput
                | Self::SelectExpanded
                | Self::ScrollViewScrolled => default_widget_book_state(),
                Self::OverviewConfigured
                | Self::CheckboxUnchecked
                | Self::FilledInput
                | Self::TextArea
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
                Self::RadioButton
                | Self::RadioGroup
                | Self::Slider
                | Self::NumberInput
                | Self::SelectExpanded => {
                    scroll_gallery(window, 1)?;
                    if matches!(self, Self::SelectExpanded) {
                        self.target(window).click()?;
                    }
                    Ok(())
                }
                Self::TextArea | Self::Summary => scroll_gallery(window, 2),
                Self::ScrollViewScrolled => {
                    scroll_gallery(window, 1)
                }
                Self::Overview
                | Self::OverviewConfigured
                | Self::Button
                | Self::Checkbox
                | Self::CheckboxUnchecked
                | Self::FilledInput
                | Self::Icon
                | Self::IconButton
                | Self::Separator
                | Self::Switch => Ok(()),
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
                Self::Icon => window
                    .get_by_role(SemanticsRole::Image)
                    .with_name(ICON_LABEL),
                Self::IconButton => window
                    .get_by_role(SemanticsRole::Button)
                    .with_name(ICON_BUTTON_LABEL),
                Self::Separator => window.get_by_role(SemanticsRole::Separator),
                Self::Switch => window
                    .get_by_role(SemanticsRole::Switch)
                    .with_name(SWITCH_LABEL),
                Self::RadioButton => window
                    .get_by_role(SemanticsRole::RadioButton)
                    .with_name(RADIO_BUTTON_LABEL),
                Self::RadioGroup => window
                    .get_by_role(SemanticsRole::RadioGroup)
                    .with_name(RADIO_GROUP_NAME),
                Self::Slider => window
                    .get_by_role(SemanticsRole::Slider)
                    .with_name(SLIDER_NAME),
                Self::NumberInput => window
                    .get_by_role(SemanticsRole::SpinBox)
                    .with_name(NUMBER_INPUT_NAME),
                Self::TextArea => window
                    .get_by_role(SemanticsRole::TextInput)
                    .with_name(TEXT_AREA_LABEL),
                Self::SelectExpanded => window
                    .get_by_role(SemanticsRole::ComboBox)
                    .with_name(SELECT_NAME),
                Self::ScrollViewScrolled => window.get_by_role(SemanticsRole::ScrollView),
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
                .is_some_and(|description| description.contains("subscription: off"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("button presses: 1"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("icon actions: 2"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("switch: off"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("radio choice: High"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("mode: Multiply"))
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

        let slider = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Slider && node.name.as_deref() == Some(SLIDER_NAME)
            })
            .expect("slider semantics node present");
        assert_eq!(
            slider.value,
            Some(SemanticsValue::Range {
                value: 35.0,
                min: 0.0,
                max: 100.0,
            })
        );

        let number = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
            })
            .expect("number input semantics node present");
        assert_eq!(number.value, Some(SemanticsValue::Number(24.0)));

        let select = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox
                    && node.name.as_deref() == Some(SELECT_NAME)
            })
            .expect("select semantics node present");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Multiply".to_string()))
        );

        Ok(())
    }

    fn configured_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
        Rc::new(RefCell::new(WidgetBookState {
            name: "Grace Hopper".to_string(),
            subscribed: false,
            button_presses: 1,
            icon_button_presses: 2,
            switch_on: false,
            standalone_radio_selected: true,
            radio_choice: "High".to_string(),
            slider_value: 35.0,
            number_value: 24.0,
            notes: "Line 1\nLine 2".to_string(),
            mode: "Multiply".to_string(),
        }))
    }

    fn blank_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
        Rc::new(RefCell::new(WidgetBookState {
            name: String::new(),
            subscribed: false,
            button_presses: 0,
            icon_button_presses: 0,
            switch_on: false,
            standalone_radio_selected: false,
            radio_choice: "Balanced".to_string(),
            slider_value: 50.0,
            number_value: 8.0,
            notes: String::new(),
            mode: String::new(),
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

    fn scroll_gallery(window: &TestWindow, pages: usize) -> Result<()> {
        let locator = window.get_by_role(SemanticsRole::ScrollView);
        locator.focus()?;
        for _ in 0..pages {
            locator.press("PageDown")?;
        }
        Ok(())
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
