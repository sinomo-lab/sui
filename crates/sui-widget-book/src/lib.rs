#![forbid(unsafe_code)]

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, Rect,
    SceneStatisticsDetailMode, SemanticsNode, SemanticsRole, SemanticsValue,
    TextStyle, TimerToken, Vector, WidgetPodMutVisitor, WidgetPodVisitor, WindowEvent, WindowId,
    WindowPerformanceSnapshot, set_window_scene_statistics_detail_mode,
    window_performance_snapshot, window_scene_statistics_detail_mode,
};

mod visual_artifacts;

pub use visual_artifacts::write_visual_artifacts;

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";
pub const BUTTON_GRID_BENCHMARK_TITLE: &str = "SUI 64 Button Grid Benchmark";
pub const BUTTON_GRID_ROWS: usize = 8;
pub const BUTTON_GRID_COLUMNS: usize = 8;
pub const NAME_INPUT_LABEL: &str = "Name";
pub const TEXT_AREA_LABEL: &str = "Notes";
pub const SUBSCRIBE_LABEL: &str = "Subscribe to product updates";
pub const PRIMARY_BUTTON_LABEL: &str = "Trigger action";
pub const TOOLBAR_SEPARATOR_NAME: &str = "Toolbar divider";
pub const ICON_LABEL: &str = "Search icon";
pub const ICON_BUTTON_LABEL: &str = "More actions";
pub const SWITCH_LABEL: &str = "Enable snapping";
pub const RADIO_BUTTON_LABEL: &str = "Standalone radio sample";
pub const RADIO_GROUP_NAME: &str = "Render quality";
pub const SLIDER_NAME: &str = "Opacity";
pub const NUMBER_INPUT_NAME: &str = "Brush size";
pub const SELECT_NAME: &str = "Blend mode";
pub const TAB_BAR_NAME: &str = "Workspace tabs";
pub const TABS_NAME: &str = "Inspector tabs";
pub const MENU_NAME: &str = "Command menu";
pub const CONTEXT_MENU_NAME: &str = "Layer context menu";
pub const TOOLTIP_TRIGGER_LABEL: &str = "Hover for shortcuts";
pub const TOOLTIP_TEXT: &str = "Quick access to common commands";
pub const POPOVER_NAME: &str = "Inline inspector";
pub const POPOVER_TRIGGER_LABEL: &str = "Open inspector";
pub const DIALOG_TITLE: &str = "Project settings";
pub const DIALOG_TRIGGER_LABEL: &str = "Toggle project settings";
pub const PROGRESS_NAME: &str = "Export progress";
pub const SPINNER_NAME: &str = "Background work";
pub const SUMMARY_NAME: &str = "Widget book summary";
pub const GALLERY_SCROLL_NAME: &str = "Widget book gallery";
pub const THEME_PREVIEW_NAME: &str = "Theme preview showcase";
pub const THEME_PREVIEW_TOGGLE_LABEL: &str = "Compare light and dark themes";
pub const LIGHT_PREVIEW_ACTION_LABEL: &str = "Light preview action";
pub const DARK_PREVIEW_ACTION_LABEL: &str = "Dark preview action";
pub const LIGHT_PREVIEW_INPUT_LABEL: &str = "Light preview query";
pub const DARK_PREVIEW_INPUT_LABEL: &str = "Dark preview query";
pub const LIST_VIEW_NAME: &str = "Assets list";
pub const TREE_VIEW_NAME: &str = "Scene tree";
pub const TABLE_NAME: &str = "Material table";
pub const SPLIT_VIEW_NAME: &str = "Editor split";
pub const BREADCRUMB_NAME: &str = "Project path";
pub const COLOR_SWATCH_NAME: &str = "Primary swatch";
pub const COLOR_PICKER_NAME: &str = "Accent picker";
pub const DEMO_IMAGE_LABEL: &str = "Preview image";

const WIDGET_BOOK_IMAGE_HANDLE: ImageHandle = ImageHandle::new(1);

const RADIO_OPTIONS: [&str; 3] = ["Balanced", "High", "Fast"];
const BLEND_MODE_OPTIONS: [&str; 4] = ["Normal", "Multiply", "Screen", "Overlay"];
const TAB_BAR_OPTIONS: [&str; 3] = ["Canvas", "Inspector", "Export"];
const TAB_PANEL_OPTIONS: [&str; 3] = ["Layout", "Data", "History"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WidgetBookState {
    pub name: String,
    pub subscribed: bool,
    pub theme_preview_comparison: bool,
    pub button_presses: usize,
    pub icon_button_presses: usize,
    pub switch_on: bool,
    pub standalone_radio_selected: bool,
    pub radio_choice: String,
    pub slider_value: f64,
    pub number_value: f64,
    pub notes: String,
    pub mode: String,
    pub tab_bar_choice: String,
    pub tabs_choice: String,
    pub last_menu_action: String,
    pub last_context_action: String,
    pub dialog_apply_count: usize,
}

pub struct LivePerformanceRoot {
    content: SingleChild,
    performance_overlay: SingleChild,
    performance_display: Rc<RefCell<LivePerformanceDisplay>>,
    watched_state: Option<Rc<RefCell<WidgetBookState>>>,
    last_seen_state: Option<WidgetBookState>,
    window_title: String,
    window_description: String,
}

impl LivePerformanceRoot {
    const OVERLAY_MARGIN: Insets = Insets {
        left: 0.0,
        top: 18.0,
        right: 18.0,
        bottom: 0.0,
    };

    pub fn new<Content>(
        window_title: impl Into<String>,
        window_description: impl Into<String>,
        content: Content,
    ) -> Self
    where
        Content: Widget + 'static,
    {
        let performance_display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        Self {
            content: SingleChild::new(content),
            performance_overlay: SingleChild::new(LivePerformancePanel::with_display(
                Rc::clone(&performance_display),
            )),
            performance_display,
            watched_state: None,
            last_seen_state: None,
            window_title: window_title.into(),
            window_description: window_description.into(),
        }
    }

    pub fn watch_widget_book_state(mut self, state: Rc<RefCell<WidgetBookState>>) -> Self {
        self.last_seen_state = Some(state.borrow().clone());
        self.watched_state = Some(state);
        self
    }

    fn set_performance_display(
        &mut self,
        snapshot: Option<WindowPerformanceSnapshot>,
        idle: bool,
    ) -> bool {
        let next = LivePerformanceDisplay { snapshot, idle };
        let mut display = self.performance_display.borrow_mut();
        if *display == next {
            return false;
        }

        *display = next;
        true
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct LivePerformanceDisplay {
    snapshot: Option<WindowPerformanceSnapshot>,
    idle: bool,
}

pub fn default_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
    Rc::new(RefCell::new(WidgetBookState {
        name: "Ada".to_string(),
        subscribed: true,
        theme_preview_comparison: true,
        button_presses: 0,
        icon_button_presses: 0,
        switch_on: true,
        standalone_radio_selected: false,
        radio_choice: RADIO_OPTIONS[0].to_string(),
        slider_value: 72.0,
        number_value: 12.0,
        notes: "Pinned notes for inspector workflows.\nSupports multiline editing.".to_string(),
        mode: BLEND_MODE_OPTIONS[0].to_string(),
        tab_bar_choice: TAB_BAR_OPTIONS[0].to_string(),
        tabs_choice: TAB_PANEL_OPTIONS[0].to_string(),
        last_menu_action: "New tab".to_string(),
        last_context_action: "Rename".to_string(),
        dialog_apply_count: 0,
    }))
}

/// Register the images used by [`build_widget_book_gallery`] onto the given
/// application.  Call this before adding a window that contains the gallery
/// when you are assembling the application yourself (rather than using
/// [`build_widget_book_application`]).
pub fn register_widget_book_images(application: &mut Application) {
    application
        .register_image(
            WIDGET_BOOK_IMAGE_HANDLE,
            RegisteredImage::from_rgba8(72, 72, widget_book_demo_image_pixels())
                .expect("widget-book demo image is valid RGBA data"),
        )
        .expect("widget-book demo image handle should register exactly once");
}

pub fn build_widget_book_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    let mut application = Application::new();
    register_widget_book_images(&mut application);

    application.window(
        WindowBuilder::new()
            .title(WINDOW_TITLE)
            .root(LivePerformanceRoot::new(
                WINDOW_TITLE,
                WINDOW_DESCRIPTION,
                build_widget_book_gallery(Rc::clone(&state)),
            )
            .watch_widget_book_state(state)),
    )
}

pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application(default_widget_book_state()).run()
}

impl Widget for LivePerformanceRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            if let Some(state) = &self.watched_state {
                let next_state = state.borrow().clone();
                if self.last_seen_state.as_ref() != Some(&next_state) {
                    self.last_seen_state = Some(next_state);
                    let content_id = self.content.child().id();
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(content_id),
                            InvalidationKind::Paint,
                        )
                        .with_region(self.content.child().bounds()),
                    );
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(content_id),
                            InvalidationKind::Semantics,
                        )
                        .with_region(self.content.child().bounds()),
                    );
                }
            }

            if let Some(snapshot) = window_performance_snapshot(ctx.window_id()) {
                if self.set_performance_display(Some(snapshot), false) {
                    let overlay_id = self.performance_overlay.child().id();
                    ctx.request(
                        InvalidationRequest::new(
                            InvalidationTarget::Widget(overlay_id),
                            InvalidationKind::Paint,
                        )
                        .with_region(self.performance_overlay.child().bounds()),
                    );
                }
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(1280.0, 720.0));
        self.content.measure(ctx, Constraints::tight(viewport));
        self.performance_overlay
            .measure(ctx, Constraints::new(Size::ZERO, viewport));
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content
            .arrange(ctx, Rect::from_origin_size(bounds.origin, bounds.size));

        let overlay_size = self.performance_overlay.child().measured_size();
        let overlay_x = (bounds.max_x() - overlay_size.width - Self::OVERLAY_MARGIN.right)
            .max(bounds.x() + Self::OVERLAY_MARGIN.left);
        let overlay_y = bounds.y() + Self::OVERLAY_MARGIN.top;
        self.performance_overlay.arrange(
            ctx,
            Rect::from_origin_size(Point::new(overlay_x, overlay_y), overlay_size),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        ctx.clear(Color::rgba(0.95, 0.968, 0.985, 1.0));
        self.content.paint(ctx);
        self.performance_overlay.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(self.window_title.clone());
        root.description = Some(self.window_description.clone());
        ctx.push(root);
        self.content.semantics(ctx);
        self.performance_overlay.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.performance_overlay.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.performance_overlay.visit_children_mut(visitor);
    }
}

struct ProjectSettingsPreview {
    trigger: SingleChild,
    dialog: SingleChild,
    dialog_open: bool,
    trigger_pressed: bool,
}

impl ProjectSettingsPreview {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        Self {
            trigger: SingleChild::new(Button::new(DIALOG_TRIGGER_LABEL).min_width(220.0)),
            dialog: SingleChild::new(
                Dialog::new(
                    DIALOG_TITLE,
                    Stack::vertical()
                        .spacing(10.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new("Autosave every 90 seconds")
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.18, 0.22, 0.30, 1.0)),
                        )
                        .with_child(
                            Label::new("Export color profile: Display P3")
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.18, 0.22, 0.30, 1.0)),
                        )
                        .with_child(
                            Label::new("Scratch disk: fast-local-ssd")
                                .font_size(14.0)
                                .line_height(18.0)
                                .color(Color::rgba(0.18, 0.22, 0.30, 1.0)),
                        ),
                )
                .description(
                    "Compact dialog framing for confirmations, settings, and import/export flows.",
                )
                .modal(false)
                .secondary_action("Cancel", || {})
                .primary_action("Apply", move || {
                    state.borrow_mut().dialog_apply_count += 1;
                }),
            ),
            dialog_open: false,
            trigger_pressed: false,
        }
    }

    fn trigger_bounds(&self) -> Rect {
        self.trigger.child().bounds()
    }
}

impl Widget for ProjectSettingsPreview {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Capture {
            return;
        }

        match event {
            Event::Window(WindowEvent::RedrawRequested) => {

            }
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Down
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                self.trigger_pressed = self.trigger_bounds().contains(pointer.position);
            }
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Up
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                let activate =
                    self.trigger_pressed && self.trigger_bounds().contains(pointer.position);
                self.trigger_pressed = false;
                if activate {
                    self.dialog_open = !self.dialog_open;
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer) if pointer.kind == sui::PointerEventKind::Cancel => {
                self.trigger_pressed = false;
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self
            .trigger
            .measure(ctx, constraints.loosen());

        if !self.dialog_open {
            return constraints.clamp(trigger_size);
        }

        let dialog_size = self
            .dialog
            .measure(ctx, Constraints::tight(Size::new(560.0, 320.0)));

        constraints.clamp(Size::new(
            trigger_size.width.max(dialog_size.width),
            trigger_size.height + 12.0 + dialog_size.height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_size = self.trigger.child().measured_size();
        self.trigger
            .arrange(ctx, Rect::from_origin_size(bounds.origin, trigger_size));
        if self.dialog_open {
            let dialog_size = self.dialog.child().measured_size();
            self.dialog.arrange(
                ctx,
                Rect::new(
                    bounds.x(),
                    bounds.y() + trigger_size.height + 12.0,
                    dialog_size.width,
                    dialog_size.height,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.dialog_open {
            self.dialog.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.trigger.semantics(ctx);
        if self.dialog_open {
            self.dialog.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.dialog_open {
            self.dialog.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.dialog_open {
            self.dialog.visit_children_mut(visitor);
        }
    }
}

struct ThemePreviewShowcase {
    state: Rc<RefCell<WidgetBookState>>,
    toggle: SingleChild,
    light_card: SingleChild,
    dark_card: SingleChild,
}

impl ThemePreviewShowcase {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        let comparison_enabled = state.borrow().theme_preview_comparison;
        let toggle_state = Rc::clone(&state);

        Self {
            state,
            toggle: SingleChild::new(
                Switch::new(THEME_PREVIEW_TOGGLE_LABEL)
                    .on(comparison_enabled)
                    .on_toggle(move |checked| {
                        toggle_state.borrow_mut().theme_preview_comparison = checked;
                    }),
            ),
            light_card: SingleChild::new(theme_preview_card(
                DefaultTheme::light(),
                "Light",
                LIGHT_PREVIEW_ACTION_LABEL,
                LIGHT_PREVIEW_INPUT_LABEL,
            )),
            dark_card: SingleChild::new(theme_preview_card(
                DefaultTheme::dark(),
                "Dark",
                DARK_PREVIEW_ACTION_LABEL,
                DARK_PREVIEW_INPUT_LABEL,
            )),
        }
    }

    fn card_height() -> f32 {
        272.0
    }

    fn comparison_enabled(&self) -> bool {
        self.state.borrow().theme_preview_comparison
    }
}

impl Widget for ThemePreviewShowcase {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() != sui::EventPhase::Capture {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    sui::PointerEventKind::Down | sui::PointerEventKind::Up
                ) && self.toggle.child().bounds().contains(pointer.position) =>
            {
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let comparison_enabled = self.comparison_enabled();
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(320.0)
        } else {
            920.0
        };
        let toggle_size = self.toggle.measure(ctx, constraints.loosen());
        let top = toggle_size.height + 16.0;
        let gap = 16.0;
        let card_height = Self::card_height();

        if comparison_enabled {
            let stacked = max_width < 760.0;
            if stacked {
                let light_size = self
                    .light_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));
                let dark_size = self
                    .dark_card
                    .measure(ctx, Constraints::tight(Size::new(max_width, card_height)));

                return constraints.clamp(Size::new(
                    max_width,
                    top + light_size.height + gap + dark_size.height,
                ));
            }

            let card_width = ((max_width - gap) / 2.0).max(280.0);
            let light_size = self
                .light_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));
            let dark_size = self
                .dark_card
                .measure(ctx, Constraints::tight(Size::new(card_width, card_height)));

            return constraints.clamp(Size::new(
                light_size.width + gap + dark_size.width,
                top + light_size.height.max(dark_size.height),
            ));
        }

        let light_width = max_width.min(420.0);
        let light_size = self
            .light_card
            .measure(ctx, Constraints::tight(Size::new(light_width, card_height)));

        constraints.clamp(Size::new(light_size.width, top + light_size.height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let comparison_enabled = self.comparison_enabled();
        let toggle_size = self.toggle.child().measured_size();
        self.toggle
            .arrange(ctx, Rect::from_origin_size(bounds.origin, toggle_size));

        let top = bounds.y() + toggle_size.height + 16.0;
        let gap = 16.0;
        if comparison_enabled {
            if bounds.width() < 760.0 {
                let light_size = self.light_card.child().measured_size();
                let dark_size = self.dark_card.child().measured_size();
                self.light_card.arrange(
                    ctx,
                    Rect::new(bounds.x(), top, light_size.width, light_size.height),
                );
                self.dark_card.arrange(
                    ctx,
                    Rect::new(
                        bounds.x(),
                        top + light_size.height + gap,
                        dark_size.width,
                        dark_size.height,
                    ),
                );
            } else {
                let light_size = self.light_card.child().measured_size();
                let dark_size = self.dark_card.child().measured_size();
                self.light_card.arrange(
                    ctx,
                    Rect::new(bounds.x(), top, light_size.width, light_size.height),
                );
                self.dark_card.arrange(
                    ctx,
                    Rect::new(
                        bounds.x() + light_size.width + gap,
                        top,
                        dark_size.width,
                        dark_size.height,
                    ),
                );
            }
        } else {
            let light_size = self.light_card.child().measured_size();
            self.light_card.arrange(
                ctx,
                Rect::new(bounds.x(), top, light_size.width, light_size.height),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.paint(ctx);
        self.light_card.paint(ctx);
        if comparison_enabled {
            self.dark_card.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let comparison_enabled = self.comparison_enabled();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(THEME_PREVIEW_NAME.to_string());
        node.description = Some(if comparison_enabled {
            "Light and dark preview cards are visible side by side.".to_string()
        } else {
            "Only the light preview card is visible.".to_string()
        });
        ctx.push(node);
        self.toggle.semantics(ctx);
        self.light_card.semantics(ctx);
        if comparison_enabled {
            self.dark_card.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.visit_children(visitor);
        self.light_card.visit_children(visitor);
        if comparison_enabled {
            self.dark_card.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        let comparison_enabled = self.comparison_enabled();
        self.toggle.visit_children_mut(visitor);
        self.light_card.visit_children_mut(visitor);
        if comparison_enabled {
            self.dark_card.visit_children_mut(visitor);
        }
    }
}

pub fn build_widget_book_gallery(state: Rc<RefCell<WidgetBookState>>) -> impl Widget {
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
    let initial_tab_bar_choice = snapshot.tab_bar_choice.clone();
    let initial_tabs_choice = snapshot.tabs_choice.clone();

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
    let tab_bar_state = Rc::clone(&state);
    let tabs_state = Rc::clone(&state);
    let menu_state = Rc::clone(&state);
    let context_menu_state = Rc::clone(&state);
    let dialog_state = Rc::clone(&state);

    VirtualScrollView::new()
        .name(GALLERY_SCROLL_NAME)
        .padding(Insets::all(24.0))
        .spacing(18.0)
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
                "Theme preview",
                "Flip the compare toggle to inspect the simplified light and dark daisy-style themes with the same control composition.",
                ThemePreviewShowcase::new(Rc::clone(&state)),
            ))
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
                        Separator::horizontal()
                            .name(TOOLBAR_SEPARATOR_NAME)
                            .inset(12.0),
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
                "Navigation surfaces",
                "Tab bars and tab containers should work for editor chrome and docked inspectors without waiting for a custom application shell.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(520.0).with_child(
                            TabBar::new(TAB_BAR_NAME)
                                .tabs(TAB_BAR_OPTIONS)
                                .selected(option_index(&TAB_BAR_OPTIONS, &initial_tab_bar_choice).unwrap_or(0))
                                .on_change(move |_, value| {
                                    tab_bar_state.borrow_mut().tab_bar_choice = value;
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(540.0).height(220.0).with_child(
                            Tabs::new(TABS_NAME)
                                .selected(option_index(&TAB_PANEL_OPTIONS, &initial_tabs_choice).unwrap_or(0))
                                .tab(
                                    TAB_PANEL_OPTIONS[0],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Alignment, spacing, and surface geometry controls belong in a compact inspector tab.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.36, 0.44, 0.54, 1.0)),
                                            )
                                            .with_child(
                                                ProgressBar::new("Layout completion")
                                                    .range(0.0, 100.0)
                                                    .value(initial_slider_value)
                                                    .show_value(true),
                                            ),
                                    ),
                                )
                                .tab(
                                    TAB_PANEL_OPTIONS[1],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Inline data summaries and editable metadata fit naturally in a reusable tabs widget.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.36, 0.44, 0.54, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Selection: 4 layers, 2 masks, 1 smart object")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color(Color::rgba(0.46, 0.54, 0.63, 1.0)),
                                            ),
                                    ),
                                )
                                .tab(
                                    TAB_PANEL_OPTIONS[2],
                                    Padding::all(
                                        4.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Undo groups, import checkpoints, and review markers are another common fit for tabbed panels.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.36, 0.44, 0.54, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Replaying history cache")
                                                    .font_size(13.0)
                                                    .line_height(18.0)
                                                    .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
                                            ),
                                    ),
                                )
                                .on_change(move |_, value| {
                                    tabs_state.borrow_mut().tabs_choice = value;
                                }),
                        ),
                    ),
            ))
            .with_child(panel(
                "Menus and overlays",
                "App menus, context menus, popovers, tooltips, and dialogs are the small but high-value surfaces that make desktop workflows feel complete.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(300.0).with_child(
                            Menu::new(MENU_NAME)
                                .item(MenuItem::new("New tab").shortcut("Ctrl+T"))
                                .item(MenuItem::new("Duplicate panel").shortcut("Ctrl+D"))
                                .item(
                                    MenuItem::new("Delete layer")
                                        .shortcut("Del")
                                        .separator_before()
                                        .destructive(),
                                )
                                .on_activate(move |_, item| {
                                    menu_state.borrow_mut().last_menu_action = item.label().to_string();
                                }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            ContextMenu::new(
                                CONTEXT_MENU_NAME,
                                Background::new(
                                    Color::rgba(0.96, 0.975, 0.995, 1.0),
                                    Padding::all(
                                        14.0,
                                        Label::new("Right-click this layer tile")
                                            .font_size(14.0)
                                            .line_height(18.0)
                                            .color(Color::rgba(0.16, 0.21, 0.29, 1.0)),
                                    ),
                                ),
                            )
                            .item(MenuItem::new("Rename"))
                            .item(MenuItem::new("Duplicate"))
                            .item(MenuItem::new("Delete").separator_before().destructive())
                            .on_activate(move |_, item| {
                                context_menu_state.borrow_mut().last_context_action = item.label().to_string();
                            }),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(220.0).with_child(
                            Tooltip::new(
                                TOOLTIP_TEXT,
                                Button::new(TOOLTIP_TRIGGER_LABEL).min_width(180.0),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(360.0).with_child(
                            Popover::new(
                                POPOVER_NAME,
                                Button::new(POPOVER_TRIGGER_LABEL).min_width(190.0),
                                Stack::vertical()
                                    .spacing(8.0)
                                    .alignment(Alignment::Stretch)
                                    .with_child(
                                        Label::new("Inline inspector content can stay lightweight instead of forcing a full modal.")
                                            .font_size(14.0)
                                            .line_height(19.0)
                                            .color(Color::rgba(0.34, 0.42, 0.52, 1.0)),
                                    )
                                    .with_child(
                                        Label::new("Blend preview: Screen @ 72%")
                                            .font_size(13.0)
                                            .line_height(18.0)
                                            .color(Color::rgba(0.46, 0.54, 0.63, 1.0)),
                                    ),
                            ),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(560.0).with_child(
                            ProjectSettingsPreview::new(dialog_state),
                        ),
                    ),
            ))
            .with_child(panel(
                "Progress and busy",
                "Progress bars and busy indicators are simple, but they anchor long-running exports, caching, and background processing workflows.",
                Stack::vertical()
                    .spacing(14.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(320.0).with_child(
                            ProgressBar::new(PROGRESS_NAME)
                                .range(0.0, 100.0)
                                .value(initial_slider_value)
                                .show_value(true),
                        ),
                    )
                    .with_child(
                        Label::new(SPINNER_NAME)
                            .font_size(13.0)
                            .line_height(18.0)
                            .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
                    ),
            ))
            .with_child(panel(
                "Live state",
                "This summary reads state produced by reusable controls so screenshot stories can cover both isolated widgets and composed UI.",
                WidgetBookSummary::new(state),
            ))
            .with_child(panel(
                "Live performance overlay",
                "The stats card now floats over the gallery so frame timing stays visible while you inspect any part of the widget book.",
                Label::new(
                    "Use the compact panel pinned in the top-right corner while you scroll and interact with the rest of the gallery.",
                )
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
            ))
            .with_child(panel(
                "Debugging and inspection",
                "The sui-debug crate composes reusable diagnostics chrome with SUI-specific views over focus, semantics, widget graph, and scene summaries.",
                Label::new(
                    "Debug inspector available via sui-debug crate. Open the standalone debug view for full semantics, widget graph, and scene inspection."
                )
                .font_size(13.0)
                .line_height(18.0)
                .color(Color::rgba(0.45, 0.53, 0.62, 1.0)),
            ))
            .with_child(panel(
                "Collections and hierarchy",
                "Foundational editor widgets need to cover lists, trees, and structured tables without requiring app-specific shells first.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(360.0).height(220.0).with_child(
                            ListView::new(LIST_VIEW_NAME)
                                .items([
                                    ListItem::new("Hero texture").detail("2048 x 2048 RGBA").accent(Color::rgba(0.16, 0.54, 0.88, 1.0)),
                                    ListItem::new("Normals atlas").detail("Streaming mip chain"),
                                    ListItem::new("Glass material").detail("Referenced in 3 prefabs"),
                                    ListItem::new("UI icon sheet").detail("Tagged for export").accent(Color::rgba(0.78, 0.50, 0.17, 1.0)),
                                    ListItem::new("Archive cache").detail("Read only").disabled(),
                                ])
                                .selected(1),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(420.0).height(240.0).with_child(
                            TreeView::new(TREE_VIEW_NAME)
                                .items([
                                    TreeItem::new("Scene")
                                        .expanded(true)
                                        .with_child(
                                            TreeItem::new("Environment")
                                                .expanded(true)
                                                .with_child(TreeItem::new("Sky dome").detail("Visible"))
                                                .with_child(TreeItem::new("Fog volume").detail("Animated")),
                                        )
                                        .with_child(
                                            TreeItem::new("Characters")
                                                .expanded(true)
                                                .with_child(TreeItem::new("Pilot").detail("Selected"))
                                                .with_child(TreeItem::new("Companion drone")),
                                        )
                                        .with_child(TreeItem::new("FX").detail("Collapsed group")),
                                ]),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(250.0).with_child(
                            Table::new(TABLE_NAME)
                                .columns([
                                    TableColumn::new("Material"),
                                    TableColumn::new("Domain").width(120.0),
                                    TableColumn::new("Shader").width(180.0),
                                    TableColumn::new("Passes").width(90.0).alignment(TableColumnAlignment::End),
                                    TableColumn::new("Last edit").width(130.0),
                                ])
                                .rows([
                                    TableRow::new(["ClearCoat_Glass", "Surface", "pbr.clearcoat", "3", "2 min ago"]),
                                    TableRow::new(["Terrain_Master", "Surface", "terrain.layered", "5", "11 min ago"]),
                                    TableRow::new(["UI_Highlight", "Overlay", "ui.gradient", "1", "24 min ago"]),
                                    TableRow::new(["CloudShadow", "Decal", "fx.projected", "2", "1 hour ago"]),
                                    TableRow::new(["Water_Foam", "Surface", "water.foam", "4", "yesterday"]),
                                ])
                                .selected(2),
                        ),
                    ),
            ))
            .with_child(panel(
                "Layout and pathing",
                "Editor shells need split panes and breadcrumb-style navigation before the rest of the UI can settle into place.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        SizedBox::new().width(620.0).with_child(
                            Breadcrumb::new(BREADCRUMB_NAME)
                                .items([
                                    BreadcrumbItem::new("Workspace"),
                                    BreadcrumbItem::new("Projects"),
                                    BreadcrumbItem::new("Starfall"),
                                    BreadcrumbItem::new("Materials"),
                                    BreadcrumbItem::new("Glass"),
                                ])
                                .current(4),
                        ),
                    )
                    .with_child(
                        SizedBox::new().width(720.0).height(240.0).with_child(
                            SplitView::horizontal(
                                Background::new(
                                    Color::rgba(0.97, 0.981, 0.992, 1.0),
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Viewport")
                                                    .font_size(18.0)
                                                    .line_height(22.0)
                                                    .color(Color::rgba(0.12, 0.16, 0.22, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Resizable panes let editor shells settle into familiar two-up and inspector layouts.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                                            ),
                                    ),
                                ),
                                Background::new(
                                    Color::rgba(0.985, 0.99, 1.0, 1.0),
                                    Padding::all(
                                        16.0,
                                        Stack::vertical()
                                            .spacing(8.0)
                                            .alignment(Alignment::Stretch)
                                            .with_child(
                                                Label::new("Inspector")
                                                    .font_size(18.0)
                                                    .line_height(22.0)
                                                    .color(Color::rgba(0.12, 0.16, 0.22, 1.0)),
                                            )
                                            .with_child(
                                                Label::new("Drag the divider to rebalance the viewport and detail pane without custom shell code.")
                                                    .font_size(14.0)
                                                    .line_height(19.0)
                                                    .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                                            ),
                                    ),
                                ),
                            )
                            .name(SPLIT_VIEW_NAME)
                            .ratio(0.62),
                        ),
                    ),
            ))
            .with_child(panel(
                "Color and imagery",
                "SUI targets visual tooling, so swatches, a usable picker, and image previews need to exist as first-class widgets.",
                Stack::vertical()
                    .spacing(16.0)
                    .alignment(Alignment::Stretch)
                    .with_child(
                        Stack::horizontal()
                            .spacing(12.0)
                            .alignment(Alignment::Center)
                            .with_child(ColorSwatch::new(COLOR_SWATCH_NAME, Color::rgba(0.12, 0.55, 0.88, 1.0)).size(Size::new(64.0, 36.0)))
                            .with_child(ColorSwatch::new("Shadow swatch", Color::rgba(0.08, 0.10, 0.14, 0.84)).size(Size::new(64.0, 36.0)))
                            .with_child(
                                Label::new("Use swatches for palettes, material chips, and compact property rows.")
                                    .font_size(14.0)
                                    .line_height(18.0)
                                    .color(Color::rgba(0.42, 0.49, 0.58, 1.0)),
                            ),
                    )
                    .with_child(
                        Stack::horizontal()
                            .spacing(16.0)
                            .alignment(Alignment::Start)
                            .with_child(
                                SizedBox::new().width(220.0).height(220.0).with_child(
                                    Image::new(WIDGET_BOOK_IMAGE_HANDLE)
                                        .label(DEMO_IMAGE_LABEL)
                                        .fit(ImageFit::Contain)
                                        .background(Color::rgba(0.965, 0.975, 0.99, 1.0))
                                        .corner_radius(12.0),
                                ),
                            ),
                    ),
            ))
}

pub fn build_button_grid_benchmark() -> impl Widget {
    let mut grid = Stack::vertical().spacing(12.0).alignment(Alignment::Stretch);

    for row in 0..BUTTON_GRID_ROWS {
        let mut line = Stack::horizontal().spacing(12.0).alignment(Alignment::Start);
        for column in 0..BUTTON_GRID_COLUMNS {
            line = line.with_child(
                Button::new(format!("Button {row}:{column}"))
                    .min_width(112.0)
                    .min_height(24.0),
            );
        }
        grid = grid.with_child(line);
    }

    grid
}

pub fn build_button_grid_benchmark_application() -> Application {
    Application::new().window(
        WindowBuilder::new()
            .title(BUTTON_GRID_BENCHMARK_TITLE)
            .root(LivePerformanceRoot::new(
                BUTTON_GRID_BENCHMARK_TITLE,
                "Focused benchmark surface for measuring the initial frame cost of a 64-button grid.",
                build_button_grid_benchmark(),
            )),
    )
}

fn widget_book_demo_image_pixels() -> Vec<u8> {
    let width = 72usize;
    let height = 72usize;
    let mut pixels = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let mut red = if checker { 228 } else { 208 };
            let mut green = if checker { 236 } else { 216 };
            let mut blue = if checker { 248 } else { 228 };
            let alpha = 255u8;

            if x > 10 && x < 62 && y > 10 && y < 62 {
                red = 38 + ((x as f32 / width as f32) * 50.0) as u8;
                green = 108 + ((y as f32 / height as f32) * 60.0) as u8;
                blue = 190;
            }

            if (x > 18 && x < 54) && (y > 18 && y < 54) {
                red = 245;
                green = 248;
                blue = 252;
            }

            if (x > 28 && x < 44) && (y > 24 && y < 48) {
                red = 255;
                green = 168;
                blue = 60;
            }

            pixels[index] = red;
            pixels[index + 1] = green;
            pixels[index + 2] = blue;
            pixels[index + 3] = alpha;
        }
    }

    pixels
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

fn theme_preview_card(
    theme: DefaultTheme,
    title: &'static str,
    action_label: &'static str,
    input_label: &'static str,
) -> impl Widget {
    let body = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new(format!("{title} theme"))
                .font_size(18.0)
                .line_height(22.0)
                .color(theme.palette.text),
        )
        .with_child(
            Label::new(format!(
                "{} base surface with {} accent for primary actions.",
                theme.colors.name, theme.colors.name
            ))
            .font_size(13.0)
            .line_height(18.0)
            .color(theme.palette.placeholder),
        )
        .with_child(
            SizedBox::new().width(220.0).with_child(
                TextInput::new(input_label)
                    .placeholder("Find layer, panel, or asset")
                    .theme(theme),
            ),
        )
        .with_child(
            Stack::horizontal()
                .spacing(12.0)
                .alignment(Alignment::Center)
                .with_child(Button::new(action_label).theme(theme))
                .with_child(
                    Label::new("Reusable controls should stay coherent across both theme variants.")
                        .font_size(13.0)
                        .line_height(18.0)
                        .color(theme.palette.placeholder),
                ),
        )
        .with_child(
            Checkbox::new(format!("{title} preview snap to grid"))
                .checked(true)
                .theme(theme),
        )
        .with_child(
            Switch::new(format!("{title} preview live updates"))
                .on(true)
                .theme(theme),
        )
        .with_child(
            Stack::horizontal()
                .spacing(10.0)
                .alignment(Alignment::Center)
                .with_child(
                    ColorSwatch::new(
                        format!("{title} base swatch"),
                        theme.colors.base_200,
                    )
                    .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(
                        format!("{title} primary swatch"),
                        theme.colors.primary,
                    )
                    .size(Size::new(58.0, 28.0)),
                )
                .with_child(
                    ColorSwatch::new(
                        format!("{title} secondary swatch"),
                        theme.colors.secondary,
                    )
                    .size(Size::new(58.0, 28.0)),
                ),
        );

    Background::new(
        theme.palette.border,
        Padding::all(
            1.0,
            Background::new(
                theme.palette.surface,
                Padding::all(18.0, body),
            ),
        ),
    )
}

struct WidgetBookSummary {
    state: Rc<RefCell<WidgetBookState>>,
    last_seen_state: WidgetBookState,
}

impl WidgetBookSummary {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        let last_seen_state = state.borrow().clone();
        Self {
            state,
            last_seen_state,
        }
    }
}

struct LivePerformancePanel {
    display: Rc<RefCell<LivePerformanceDisplay>>,
    toggle_pressed: bool,
    detail_refresh_timer: Option<TimerToken>,
}

#[derive(Debug, Clone)]
struct LivePerformanceLine {
    y: f32,
    text: String,
    style: TextStyle,
}

#[derive(Debug, Clone)]
struct LivePerformanceLineSpec {
    text: String,
    style: TextStyle,
    spacing_after: f32,
}

impl LivePerformancePanel {
    const WIDTH: f32 = 252.0;
    const HEIGHT: f32 = 204.0;
    const PADDING_X: f32 = 12.0;
    const PADDING_Y: f32 = 10.0;
    const CORNER_RADIUS: f32 = 12.0;
    const TOGGLE_WIDTH: f32 = 76.0;
    const TOGGLE_HEIGHT: f32 = 18.0;

    #[cfg(test)]
    fn new() -> Self {
        Self::with_display(Rc::new(RefCell::new(LivePerformanceDisplay::default())))
    }

    fn with_display(display: Rc<RefCell<LivePerformanceDisplay>>) -> Self {
        Self {
            display,
            toggle_pressed: false,
            detail_refresh_timer: None,
        }
    }

    fn toggle_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.max_x() - Self::PADDING_X - Self::TOGGLE_WIDTH,
            bounds.y() + Self::PADDING_Y - 1.0,
            Self::TOGGLE_WIDTH,
            Self::TOGGLE_HEIGHT,
        )
    }

    fn toggle_label(detail_mode: SceneStatisticsDetailMode) -> &'static str {
        if detail_mode.is_detailed() {
            "detail on"
        } else {
            "detail off"
        }
    }

    fn rebuild_lines(&self, width: f32, specs: &[LivePerformanceLineSpec]) -> Vec<LivePerformanceLine> {
        let _text_width = (width - Self::PADDING_X * 2.0).max(1.0);
        let mut y = Self::PADDING_Y;
        let mut lines = Vec::new();

        for spec in specs {
            let line_height = spec.style.line_height.max(spec.style.font_size);
            lines.push(LivePerformanceLine {
                y,
                text: spec.text.clone(),
                style: spec.style.clone(),
            });
            y += line_height + spec.spacing_after;
        }

        lines
    }

    fn content_specs(&self, _window_id: WindowId) -> Vec<LivePerformanceLineSpec> {
        let display = self.display.borrow().clone();

        match display.snapshot {
            Some(snapshot) => {
                let headline = format!(
                    "{}  |  {}",
                    if display.idle {
                        "0 fps".to_string()
                    } else {
                        format_fps(snapshot.total_time_ms)
                    },
                    if display.idle {
                        "idle".to_string()
                    } else {
                        format_duration_ms(snapshot.total_time_ms)
                    },
                );
                if !snapshot.scene.detail_mode.is_detailed() {
                    return vec![
                        LivePerformanceLineSpec::title("live performance".to_string()),
                        LivePerformanceLineSpec::headline(headline),
                    ];
                }

                let slowest_phase = snapshot.slowest_phase();
                let slowest_label = slowest_phase
                    .map(|sample| sample.phase.label())
                    .unwrap_or("idle");
                let slowest_duration = slowest_phase
                    .map(|sample| format_duration_ms(sample.duration_ms))
                    .unwrap_or_else(|| "0.0 ms".to_string());
                let scene_metric = format!(
                    "scene {} dirty  |  {} layers  |  {} updates",
                    snapshot.scene.dirty_region_count,
                    snapshot.scene.layer_count,
                    snapshot.scene.layer_update_count,
                );
                let trailing_metric = format!(
                    "cmds txt {}  |  img {}  |  clip {}",
                    snapshot.scene.text_command_count,
                    snapshot.scene.image_command_count,
                    snapshot.scene.clip_command_count,
                );

                vec![
                    LivePerformanceLineSpec::title("live performance".to_string()),
                    LivePerformanceLineSpec::headline(headline),
                    LivePerformanceLineSpec::metric(if display.idle {
                        format!(
                            "frame {}  |  last active {} {}",
                            snapshot.frame_index, slowest_label, slowest_duration,
                        )
                    } else {
                        format!(
                            "frame {}  |  slowest {} {}",
                            snapshot.frame_index, slowest_label, slowest_duration,
                        )
                    }),
                    LivePerformanceLineSpec::metric(format!(
                        "gpu {} passes  |  {} draws  |  {}",
                        snapshot.renderer_submission.pass_count,
                        snapshot.renderer_submission.draw_count,
                        format_byte_size(snapshot.renderer_submission.uploaded_vertex_bytes),
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "tiles {} vis  |  {} reused  |  {} regen",
                        snapshot.renderer_submission.visible_tile_count,
                        snapshot.renderer_submission.reused_tile_count,
                        snapshot.renderer_submission.regenerated_tile_count,
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "tile mem {}  |  packets {}  |  layers {}",
                        format_byte_size(snapshot.renderer_submission.tile_memory_bytes),
                        snapshot.renderer_submission.direct_packet_count,
                        snapshot.renderer_submission.visible_layer_count,
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "tile gen {}  |  compose {}",
                        format_duration_ms(snapshot.renderer_submission.tile_generation_time_us as f64 / 1000.0),
                        format_duration_ms(snapshot.renderer_submission.composition_time_us as f64 / 1000.0),
                    )),
                    LivePerformanceLineSpec::metric(scene_metric),
                    LivePerformanceLineSpec::metric(trailing_metric),
                ]
            }
            None => vec![
                LivePerformanceLineSpec::title("live performance".to_string()),
                LivePerformanceLineSpec::body("waiting for the first completed frame".to_string()),
                LivePerformanceLineSpec::body("from the desktop loop".to_string()),
            ],
        }
    }
}

impl LivePerformanceLineSpec {
    fn title(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.14, 0.20, 0.29, 1.0), 11.0, 14.0),
            spacing_after: 6.0,
        }
    }

    fn headline(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.07, 0.34, 0.52, 1.0), 14.0, 18.0),
            spacing_after: 6.0,
        }
    }

    fn body(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.42, 0.49, 0.58, 1.0), 12.0, 16.0),
            spacing_after: 0.0,
        }
    }

    fn metric(text: String) -> Self {
        Self {
            text,
            style: text_style(Color::rgba(0.18, 0.24, 0.32, 1.0), 12.0, 16.0),
            spacing_after: 4.0,
        }
    }
}

impl Widget for LivePerformancePanel {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !matches!(ctx.phase(), sui::EventPhase::Capture | sui::EventPhase::Target) {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Down
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                self.toggle_pressed = self.toggle_bounds(ctx.bounds()).contains(pointer.position);
                if self.toggle_pressed {
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == sui::PointerEventKind::Up
                    && pointer.button == Some(sui::PointerButton::Primary) =>
            {
                let was_pressed = self.toggle_pressed;
                let activate =
                    was_pressed && self.toggle_bounds(ctx.bounds()).contains(pointer.position);
                self.toggle_pressed = false;

                if activate {
                    let current_mode = window_scene_statistics_detail_mode(ctx.window_id());
                    let next_mode = if current_mode.is_detailed() {
                        SceneStatisticsDetailMode::Lightweight
                    } else {
                        SceneStatisticsDetailMode::Detailed
                    };
                    set_window_scene_statistics_detail_mode(ctx.window_id(), next_mode);
                    if let Some(token) = self.detail_refresh_timer.take() {
                        ctx.cancel_timer(token);
                    }
                    self.detail_refresh_timer = Some(ctx.schedule_timer_after(0.0));
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if was_pressed {
                    ctx.request_paint();
                }
            }
            Event::Wake(WakeEvent::Timer { token, .. })
                if self.detail_refresh_timer == Some(*token) =>
            {
                self.detail_refresh_timer = None;
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == sui::PointerEventKind::Cancel => {
                if self.toggle_pressed {
                    self.toggle_pressed = false;
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.min(Self::WIDTH)
        } else {
            Self::WIDTH
        };
        constraints.clamp(Size::new(width, Self::HEIGHT))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let detail_mode = window_scene_statistics_detail_mode(ctx.window_id());
        let lines = self.rebuild_lines(ctx.bounds().width(), &self.content_specs(ctx.window_id()));
        let shadow = ctx.bounds().translate(Vector::new(0.0, 4.0));
        ctx.fill(
            rounded_rect_path(shadow, Self::CORNER_RADIUS),
            Color::rgba(0.10, 0.16, 0.22, 0.12),
        );

        let frame = rounded_rect_path(ctx.bounds(), Self::CORNER_RADIUS);
        ctx.fill(frame.clone(), Color::rgba(0.985, 0.993, 1.0, 0.96));
        ctx.stroke(
            frame,
            Color::rgba(0.78, 0.84, 0.90, 1.0),
            StrokeStyle::new(1.0),
        );

        let toggle_bounds = self.toggle_bounds(ctx.bounds());
        let toggle_shape = rounded_rect_path(toggle_bounds, toggle_bounds.height() * 0.5);
        let toggle_fill = if detail_mode.is_detailed() {
            if self.toggle_pressed {
                Color::rgba(0.07, 0.34, 0.52, 0.92)
            } else {
                Color::rgba(0.09, 0.40, 0.60, 0.88)
            }
        } else if self.toggle_pressed {
            Color::rgba(0.88, 0.92, 0.96, 1.0)
        } else {
            Color::rgba(0.94, 0.96, 0.98, 1.0)
        };
        let toggle_stroke = if detail_mode.is_detailed() {
            Color::rgba(0.05, 0.28, 0.44, 1.0)
        } else {
            Color::rgba(0.76, 0.82, 0.88, 1.0)
        };
        ctx.fill(toggle_shape.clone(), toggle_fill);
        ctx.stroke(toggle_shape, toggle_stroke, StrokeStyle::new(1.0));
        ctx.draw_text(
            toggle_bounds,
            Self::toggle_label(detail_mode).to_string(),
            text_style(
                if detail_mode.is_detailed() {
                    Color::rgba(0.98, 0.995, 1.0, 1.0)
                } else {
                    Color::rgba(0.33, 0.40, 0.48, 1.0)
                },
                10.0,
                14.0,
            ),
        );

        ctx.push_clip_rect(ctx.bounds());
        for line in &lines {
            let line_rect = Rect::new(
                ctx.bounds().x() + Self::PADDING_X,
                ctx.bounds().y() + line.y,
                (ctx.bounds().width() - Self::PADDING_X * 2.0).max(1.0),
                line.style.line_height.max(line.style.font_size),
            );
            ctx.draw_text(line_rect, line.text.clone(), line.style.clone());
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::GenericContainer, ctx.bounds());
        node.name = Some("Live performance overlay".to_string());
        node.description =
            Some("Compact floating renderer and scene performance statistics with a scene detail toggle.".to_string());
        node.value = Some(SemanticsValue::Text(
            Self::toggle_label(window_scene_statistics_detail_mode(ctx.window_id())).to_string(),
        ));
        ctx.push(node);
    }
}

fn text_style(color: Color, size: f32, line_height: f32) -> TextStyle {
    let mut style = TextStyle::new(color);
    style.font_size = size;
    style.line_height = line_height;
    style
}

fn rounded_rect_path(rect: Rect, radius: f32) -> Path {
    Path::rounded_rect(rect, radius.min(rect.width().min(rect.height()) * 0.5))
}

fn format_fps(total_time_ms: f64) -> String {
    if total_time_ms <= 0.0 {
        "idle".to_string()
    } else {
        format!("{:.0} fps", 1000.0 / total_time_ms)
    }
}

fn format_duration_ms(duration_ms: f64) -> String {
    format!("{duration_ms:.1} ms")
}

fn format_byte_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;

    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

impl Widget for WidgetBookSummary {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !matches!(event, Event::Window(WindowEvent::RedrawRequested)) {
            return;
        }

        let current_state = self.state.borrow().clone();
        if current_state != self.last_seen_state {
            self.last_seen_state = current_state;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            320.0
        };
        constraints.clamp(Size::new(width, 270.0))
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
            format!(
                "tabs: bar={} panel={}",
                if state.tab_bar_choice.is_empty() {
                    "unset"
                } else {
                    state.tab_bar_choice.as_str()
                },
                if state.tabs_choice.is_empty() {
                    "unset"
                } else {
                    state.tabs_choice.as_str()
                }
            ),
            format!(
                "menu: {} | context: {} | dialog apply: {}",
                if state.last_menu_action.is_empty() {
                    "idle"
                } else {
                    state.last_menu_action.as_str()
                },
                if state.last_context_action.is_empty() {
                    "idle"
                } else {
                    state.last_context_action.as_str()
                },
                state.dialog_apply_count,
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
            "name: {}; subscription: {}; button presses: {}; icon actions: {}; switch: {}; standalone radio: {}; radio choice: {}; slider: {:.0}; brush size: {:.0}; mode: {}; tab bar: {}; tabs: {}; menu: {}; context menu: {}; dialog apply: {}; notes lines: {}",
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
            if state.tab_bar_choice.is_empty() {
                "unset"
            } else {
                state.tab_bar_choice.as_str()
            },
            if state.tabs_choice.is_empty() {
                "unset"
            } else {
                state.tabs_choice.as_str()
            },
            if state.last_menu_action.is_empty() {
                "idle"
            } else {
                state.last_menu_action.as_str()
            },
            if state.last_context_action.is_empty() {
                "idle"
            } else {
                state.last_context_action.as_str()
            },
            state.dialog_apply_count,
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
    use std::{cell::RefCell, rc::Rc};

    use super::{
        GALLERY_SCROLL_NAME, LivePerformanceDisplay, LivePerformancePanel, NAME_INPUT_LABEL,
        NUMBER_INPUT_NAME, SELECT_NAME, SLIDER_NAME, SUMMARY_NAME,
        THEME_PREVIEW_TOGGLE_LABEL,
        build_widget_book_application, default_widget_book_state,
    };
    use super::visual_artifacts::{
        StoryCase, configured_widget_book_state, scroll_to_story_target,
    };
    use sui::{
        Application, Event, FramePhase, FramePhaseSample, Point, PointerButton,
        PointerButtons, PointerEvent, PointerEventKind, RendererSubmissionDiagnostics, Result,
        SceneStatistics, SceneStatisticsDetailMode, SemanticsRole, SemanticsValue, Size,
        TextCacheDiagnostics, TextCacheDeltaDiagnostics, Vector, Widget, WidgetPod,
        WidgetPodVisitor, WindowBuilder, WindowEvent, WindowId, WindowPerformanceSnapshot,
        window_scene_statistics_detail_mode,
    };
    use sui_runtime::publish_window_performance_snapshot;
    use sui_testing::prelude::*;

    fn build_default_widget_book_app() -> Result<TestApp> {
        TestApp::new(|| build_widget_book_application(default_widget_book_state()).build())
    }

    fn build_configured_widget_book_app() -> Result<TestApp> {
        TestApp::new(|| build_widget_book_application(configured_widget_book_state()).build())
    }

    fn build_overlay_placeholder_app() -> Result<TestApp> {
        TestApp::new(|| {
            Application::new()
                .window(WindowBuilder::new().title("Overlay").root(LivePerformancePanel::new()))
                .build()
        })
    }

    #[test]
    fn widget_book_theme_preview_toggle_hides_dark_card() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::ThemePreview, 2)?;
        let before = window.capture_screenshot()?;

        window
            .get_by_role(SemanticsRole::Switch)
            .with_name(THEME_PREVIEW_TOGGLE_LABEL)
            .click()?;

        let after = window.capture_screenshot()?;
        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_text_input_accepts_plain_keyboard_typing() -> Result<()> {
        let baseline_summary = {
            let baseline_app = build_default_widget_book_app()?;
            let baseline_window = baseline_app.main_window()?;
            scroll_to_story_target(&baseline_window, StoryCase::Summary, 12)?;
            baseline_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?
        };

        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;

        let input = window
            .get_by_role(SemanticsRole::TextInput)
            .with_name(NAME_INPUT_LABEL);
        input.focus()?;
        input.press("Z")?;
        let input_value = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::TextInput
                    && node.name.as_deref() == Some(NAME_INPUT_LABEL)
            })
            .and_then(|node| match node.value {
                Some(SemanticsValue::Text(value)) => Some(value),
                _ => None,
            })
            .expect("text input semantics value present after typing");
        assert_eq!(input_value, "AdaZ");

        scroll_to_story_target(&window, StoryCase::Summary, 12)?;
        let summary_description = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some(SUMMARY_NAME)
            })
            .and_then(|node| node.description)
            .expect("summary semantics description present after typing");
        assert!(
            summary_description.contains("AdaZ"),
            "summary semantics did not reflect the typed name: {summary_description}"
        );
        let edited_summary = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name(SUMMARY_NAME)
            .capture_screenshot()?;

        assert!(
            edited_summary != baseline_summary,
            "summary screenshot did not change after typing"
        );

        Ok(())
    }

    #[test]
    fn widget_book_gallery_wheel_scroll_updates_screenshot_and_reveals_lower_story() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;

        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;

        let after = gallery.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_gallery_scroll_redraws_when_split_view_is_visible() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        scroll_to_story_target(&window, StoryCase::SplitView, 12)?;

        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;

        gallery.scroll_pixels(Vector::new(0.0, -48.0))?;

        let after = gallery.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_gallery_small_wheel_scroll_updates_screenshot() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;

        gallery.scroll_pixels(Vector::new(0.0, -12.0))?;

        let after = gallery.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    #[ignore = "slow; run `cargo run -p sui-widget-book` to generate artifacts"]
    fn widget_book_generates_visual_artifacts() -> Result<()> {
        let artifact_root = super::write_visual_artifacts()?;

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
    fn widget_book_configured_story_renders_expected_visual_state() -> Result<()> {
        let (
            _default_slider,
            default_number_value,
            default_select,
            default_summary,
            default_slider_value,
        ) = {
            let default_app = build_default_widget_book_app()?;
            let default_window = default_app.main_window()?;
            scroll_to_story_target(&default_window, StoryCase::Slider, 12)?;
            let default_slider = default_window
                .get_by_role(SemanticsRole::Slider)
                .with_name(SLIDER_NAME)
                .capture_screenshot()?;
            let default_slider_value = default_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::Slider
                        && node.name.as_deref() == Some(SLIDER_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Range { value, .. }) => Some(value),
                    _ => None,
                })
                .expect("default slider semantics value present");
            scroll_to_story_target(&default_window, StoryCase::NumberInput, 12)?;
            let default_number_value = default_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::SpinBox
                        && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Number(value)) => Some(value),
                    _ => None,
                })
                .expect("default number input semantics value present");
            scroll_to_story_target(&default_window, StoryCase::SelectExpanded, 12)?;
            let default_select = default_window
                .get_by_role(SemanticsRole::ComboBox)
                .with_name(SELECT_NAME)
                .capture_screenshot()?;
            scroll_to_story_target(&default_window, StoryCase::Summary, 12)?;
            let default_summary = default_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?;
            (
                default_slider,
                default_number_value,
                default_select,
                default_summary,
                default_slider_value,
            )
        };

        let (
            _configured_slider,
            configured_number_value,
            configured_select,
            configured_summary,
            configured_slider_value,
        ) = {
            let configured_app = build_configured_widget_book_app()?;
            let configured_window = configured_app.main_window()?;
            scroll_to_story_target(&configured_window, StoryCase::Slider, 12)?;
            let configured_slider = configured_window
                .get_by_role(SemanticsRole::Slider)
                .with_name(SLIDER_NAME)
                .capture_screenshot()?;
            let configured_slider_value = configured_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::Slider
                        && node.name.as_deref() == Some(SLIDER_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Range { value, .. }) => Some(value),
                    _ => None,
                })
                .expect("configured slider semantics value present");
            scroll_to_story_target(&configured_window, StoryCase::NumberInput, 12)?;
            let configured_number_value = configured_window
                .snapshot()?
                .accessibility
                .nodes
                .into_iter()
                .find(|node| {
                    node.role == SemanticsRole::SpinBox
                        && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
                })
                .and_then(|node| match node.value {
                    Some(SemanticsValue::Number(value)) => Some(value),
                    _ => None,
                })
                .expect("configured number input semantics value present");
            scroll_to_story_target(&configured_window, StoryCase::SelectExpanded, 12)?;
            let configured_select = configured_window
                .get_by_role(SemanticsRole::ComboBox)
                .with_name(SELECT_NAME)
                .capture_screenshot()?;
            scroll_to_story_target(&configured_window, StoryCase::Summary, 12)?;
            let configured_summary = configured_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name(SUMMARY_NAME)
                .capture_screenshot()?;
            (
                configured_slider,
                configured_number_value,
                configured_select,
                configured_summary,
                configured_slider_value,
            )
        };

        assert_eq!(default_slider_value, 72.0);
        assert_eq!(configured_slider_value, 35.0);
        assert_eq!(default_number_value, 12.0);
        assert_eq!(configured_number_value, 24.0);

        assert!(
            configured_select != default_select,
            "configured select screenshot matched default state"
        );
        assert!(
            configured_summary != default_summary,
            "configured summary screenshot matched default state"
        );

        Ok(())
    }

    #[test]
    fn live_performance_panel_replaces_snapshot_without_creating_children() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        let panel = LivePerformancePanel::with_display(Rc::clone(&display));

        assert_eq!(panel.content_specs(WindowId::new(11)).len(), 3);

        display.borrow_mut().snapshot = Some(sample_window_performance_snapshot_record(WindowId::new(11)));

        assert_eq!(panel.content_specs(WindowId::new(11)).len(), 2);
    }

    #[test]
    fn live_performance_panel_does_not_create_child_widgets_when_snapshot_updates() {
        struct CountingVisitor {
            count: usize,
        }

        impl WidgetPodVisitor for CountingVisitor {
            fn visit(&mut self, _child: &WidgetPod) {
                self.count += 1;
            }
        }

        let display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        let panel = LivePerformancePanel::with_display(Rc::clone(&display));
        let mut visitor = CountingVisitor { count: 0 };
        Widget::visit_children(&panel, &mut visitor);
        assert_eq!(visitor.count, 0);

        display.borrow_mut().snapshot = Some(sample_window_performance_snapshot_record(WindowId::new(11)));

        let mut visitor = CountingVisitor { count: 0 };
        Widget::visit_children(&panel, &mut visitor);
        assert_eq!(visitor.count, 0);
    }

    #[test]
    fn live_performance_panel_measures_to_compact_width() {
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Overlay").root(LivePerformancePanel::new()))
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id).expect("panel should render");
        let graph = runtime.widget_graph(window_id).expect("widget graph should exist");
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .expect("panel root node present");

        assert!(root.bounds.width() <= 252.0);
        assert!(root.bounds.height() > 0.0);
    }

    #[test]
    fn live_performance_panel_reports_zero_fps_when_idle() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay {
            snapshot: Some(sample_window_performance_snapshot_record(WindowId::new(11))),
            idle: true,
        }));
        let panel = LivePerformancePanel::with_display(display);

        let lines = panel.content_specs(WindowId::new(11));
        assert_eq!(lines[1].text, "0 fps  |  idle");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn live_performance_panel_renders_detailed_scene_metrics() {
        let display = Rc::new(RefCell::new(LivePerformanceDisplay {
            snapshot: Some(sample_detailed_window_performance_snapshot_record(WindowId::new(11))),
            idle: false,
        }));
        let panel = LivePerformancePanel::with_display(display);

        let lines = panel.content_specs(WindowId::new(11));
        assert!(lines.iter().any(|line| line.text.contains("layers")));
        assert!(lines.iter().any(|line| line.text.contains("cmds txt")));
    }

    #[test]
    fn widget_book_root_requests_paint_when_a_published_snapshot_arrives() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];

        runtime.render(window_id).expect("initial render should succeed");
        assert!(
            !runtime.needs_render(window_id).expect("window should be idle after initial render")
        );

        publish_window_performance_snapshot(sample_window_performance_snapshot_record(window_id));
        runtime
            .handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))
            .expect("redraw event should be handled");

        assert!(runtime.needs_render(window_id).expect(
            "widget-book root should request a paint when the published performance snapshot changes"
        ));
    }

    #[test]
    fn widget_book_startup_bootstraps_live_performance_overlay() -> Result<()> {
        let placeholder_image = {
            let placeholder = build_overlay_placeholder_app()?;
            let placeholder_window = placeholder.main_window()?;
            placeholder_window
                .get_by_role(SemanticsRole::GenericContainer)
                .with_name("Live performance overlay")
                .capture_screenshot()?
        };

        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");

        let live_image = overlay.capture_screenshot()?;
        let performance = window.performance_snapshot()?;

        assert_ne!(live_image, placeholder_image);
        assert!(performance.frame_index >= 2);

        Ok(())
    }

    #[test]
    fn widget_book_overlay_toggle_enables_detail_mode() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");
        let before = overlay.capture_screenshot()?;

        let overlay_node = window
            .snapshot()?
            .accessibility
            .nodes
            .into_iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Live performance overlay")
            })
            .expect("overlay semantics node present");
        let toggle_point = Point::new(
            overlay_node.bounds.max_x()
                - LivePerformancePanel::PADDING_X
                - LivePerformancePanel::TOGGLE_WIDTH * 0.5,
            overlay_node.bounds.y()
                + LivePerformancePanel::PADDING_Y
                - 1.0
                + LivePerformancePanel::TOGGLE_HEIGHT * 0.5,
        );

        overlay.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            toggle_point,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, toggle_point);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        overlay.dispatch_event(Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, toggle_point);
        up.button = Some(PointerButton::Primary);
        overlay.dispatch_event(Event::Pointer(up))?;

        let after = overlay.capture_screenshot()?;
        assert_eq!(
            window_scene_statistics_detail_mode(window.id()),
            SceneStatisticsDetailMode::Detailed,
            "overlay toggle did not enable detailed scene statistics mode"
        );
        assert!(
            before != after,
            "overlay screenshot did not change after toggling detail mode"
        );

        Ok(())
    }

    #[test]
    fn widget_book_scroll_updates_performance_overlay_without_extra_frame() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");
        let before = overlay.capture_screenshot()?;
        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
        let after = overlay.capture_screenshot()?;
        assert_ne!(after, before);

        Ok(())
    }

    #[test]
    fn widget_book_scroll_updates_performance_overlay_visuals() -> Result<()> {
        let app = build_default_widget_book_app()?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);
        let overlay = window
            .get_by_role(SemanticsRole::GenericContainer)
            .with_name("Live performance overlay");

        let before = overlay.capture_screenshot()?;
        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
        let after = overlay.capture_screenshot()?;

        assert_ne!(before, after);

        Ok(())
    }

    #[test]
    fn widget_book_exposes_compact_performance_overlay_semantics() {
        let mut runtime = build_widget_book_application(default_widget_book_state())
            .build()
            .expect("runtime should build");
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id).expect("widget book should render");
        let semantics = runtime
            .semantics(window_id)
            .expect("semantics snapshot should exist");

        let overlay = semantics
            .iter()
            .find(|node| {
                node.role == SemanticsRole::GenericContainer
                    && node.name.as_deref() == Some("Live performance overlay")
            })
            .expect("overlay semantics node present");

        assert!(overlay.bounds.width() <= 252.0);
        assert!(overlay.bounds.x() >= 1000.0);
        assert!(overlay.bounds.y() <= 24.0);
    }

    fn sample_window_performance_snapshot_record(window_id: WindowId) -> WindowPerformanceSnapshot {
        WindowPerformanceSnapshot::new(
            window_id,
            7,
            vec![FramePhaseSample::new(FramePhase::Renderer, 1.5)],
            RendererSubmissionDiagnostics::new(
                2,
                6,
                2048,
                24,
                1536,
                3,
                18,
                15,
                3,
                6,
                65536,
                420,
                160,
                210,
                120,
                3,
                4,
                90,
                1,
                440,
                210,
                130,
                15,
                95,
                4,
                32768,
                115,
                85,
                22,
                16384,
                920,
                640,
                180,
                70,
                560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: Default::default(),
                viewport: Size::new(1280.0, 720.0),
                dirty_region_count: 0,
                dirty_regions: Vec::new(),
                dirty_area: 0.0,
                dirty_coverage: 0.0,
                command_count: 0,
                command_breakdown: Vec::new(),
                layer_count: 0,
                layer_update_count: 0,
                layer_update_breakdown: Vec::new(),
                text_command_count: 0,
                image_command_count: 0,
                clip_command_count: 0,
                transform_command_count: 0,
            },
        )
    }

    fn sample_detailed_window_performance_snapshot_record(
        window_id: WindowId,
    ) -> WindowPerformanceSnapshot {
        WindowPerformanceSnapshot::new(
            window_id,
            8,
            vec![
                FramePhaseSample::new(FramePhase::Paint, 0.8),
                FramePhaseSample::new(FramePhase::Renderer, 1.9),
            ],
            RendererSubmissionDiagnostics::new(
                2,
                6,
                2048,
                24,
                1536,
                3,
                18,
                15,
                3,
                6,
                65536,
                420,
                160,
                210,
                120,
                3,
                4,
                90,
                1,
                440,
                210,
                130,
                15,
                95,
                4,
                32768,
                115,
                85,
                22,
                16384,
                920,
                640,
                180,
                70,
                560,
            ),
            TextCacheDiagnostics::default(),
            TextCacheDeltaDiagnostics::default(),
            SceneStatistics {
                detail_mode: SceneStatisticsDetailMode::Detailed,
                viewport: Size::new(1280.0, 720.0),
                dirty_region_count: 2,
                dirty_regions: Vec::new(),
                dirty_area: 128.0,
                dirty_coverage: 3.0,
                command_count: 14,
                command_breakdown: vec![("FillRect".to_string(), 8), ("Layer".to_string(), 6)],
                layer_count: 6,
                layer_update_count: 4,
                layer_update_breakdown: vec![("Repaint".to_string(), 4)],
                text_command_count: 3,
                image_command_count: 1,
                clip_command_count: 2,
                transform_command_count: 1,
            },
        )
    }
}
