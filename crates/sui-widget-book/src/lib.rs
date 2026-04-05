#![forbid(unsafe_code)]

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{
    AccessibilitySnapshot, DirtyRegion, FocusState, FrameSchedule, InvalidationKind, Rect,
    SceneStatisticsDetailMode, SemanticsNode, SemanticsRole, SemanticsValue,
    TextStyle, TimerToken, Vector, WakeEvent, WidgetGraphSnapshot, WidgetId,
    WidgetNodeSnapshot, WidgetPodMutVisitor, WidgetPodVisitor, WindowEvent, WindowId,
    WindowPerformanceSummary, set_window_scene_statistics_detail_mode,
    window_performance_summary, window_scene_statistics_detail_mode,
};
use sui_debug::{SceneDebugSummary, WindowDebugSnapshot, window_snapshot_view};

pub const WINDOW_TITLE: &str = "SUI Widget Book";
pub const WINDOW_DESCRIPTION: &str =
    "Development gallery for common built-in widgets in sui-widgets";
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

#[derive(Debug, Clone, Default)]
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

struct WidgetBookRoot {
    gallery: SingleChild,
    performance_overlay: SingleChild,
    performance_display: Rc<RefCell<LivePerformanceDisplay>>,
    performance_bootstrap_timer: Option<TimerToken>,
    performance_idle_timer: Option<TimerToken>,
    skip_next_performance_probe: bool,
    last_overlay_frame_index: Option<u64>,
    last_overlay_frame_time: Option<f64>,
}

impl WidgetBookRoot {
    const IDLE_REFRESH_INTERVAL: f64 = 0.25;
    const IDLE_THRESHOLD: f64 = 0.5;

    const OVERLAY_MARGIN: Insets = Insets {
        left: 0.0,
        top: 18.0,
        right: 18.0,
        bottom: 0.0,
    };

    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        let performance_display = Rc::new(RefCell::new(LivePerformanceDisplay::default()));
        Self {
            gallery: SingleChild::new(build_widget_book(state)),
            performance_overlay: SingleChild::new(LivePerformancePanel::with_display(
                Rc::clone(&performance_display),
            )),
            performance_display,
            performance_bootstrap_timer: None,
            performance_idle_timer: None,
            skip_next_performance_probe: false,
            last_overlay_frame_index: None,
            last_overlay_frame_time: None,
        }
    }

    fn set_performance_display(
        &mut self,
        snapshot: Option<WindowPerformanceSummary>,
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

    fn request_performance_refresh(ctx: &mut EventCtx) {
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct LivePerformanceDisplay {
    snapshot: Option<WindowPerformanceSummary>,
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

pub fn build_widget_book_application(state: Rc<RefCell<WidgetBookState>>) -> Application {
    let mut application = Application::new();
    application
        .register_image(
            WIDGET_BOOK_IMAGE_HANDLE,
            RegisteredImage::from_rgba8(72, 72, widget_book_demo_image_pixels())
                .expect("widget-book demo image is valid RGBA data"),
        )
        .expect("widget-book demo image handle should register exactly once");

    application.window(
        WindowBuilder::new()
            .title(WINDOW_TITLE)
            .root(WidgetBookRoot::new(state)),
    )
}

pub fn run_desktop_widget_book() -> Result<()> {
    build_widget_book_application(default_widget_book_state()).run()
}

impl Widget for WidgetBookRoot {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.performance_bootstrap_timer.is_none()
            && window_performance_summary(ctx.window_id()).is_none()
        {
            self.performance_bootstrap_timer = Some(ctx.schedule_timer_after(0.1));
        }

        if self.performance_idle_timer.is_none()
            && window_performance_summary(ctx.window_id()).is_some()
        {
            self.performance_idle_timer =
                Some(ctx.schedule_timer_after(Self::IDLE_REFRESH_INTERVAL));
        }

        if matches!(event, Event::Window(WindowEvent::RedrawRequested))
            && window_performance_summary(ctx.window_id()).is_some()
            && self.performance_bootstrap_timer.is_none()
        {
            if self.skip_next_performance_probe {
                self.skip_next_performance_probe = false;
            } else {
                self.performance_bootstrap_timer = Some(ctx.schedule_timer_after(0.0));
            }
        }

        if let Event::Wake(WakeEvent::Timer { token, .. }) = event {
            if Some(*token) == self.performance_idle_timer {
                self.performance_idle_timer =
                    Some(ctx.schedule_timer_after(Self::IDLE_REFRESH_INTERVAL));

                if let Some(last_frame_time) = self.last_overlay_frame_time {
                    if ctx.current_time() - last_frame_time >= Self::IDLE_THRESHOLD {
                        let snapshot = self.performance_display.borrow().snapshot;
                        if self.set_performance_display(snapshot, true) {
                            self.skip_next_performance_probe = true;
                            Self::request_performance_refresh(ctx);
                        }
                    }
                }
            }

            if Some(*token) == self.performance_bootstrap_timer {
                self.performance_bootstrap_timer = None;

                if let Some(summary) = window_performance_summary(ctx.window_id()) {
                    if self.last_overlay_frame_index == Some(summary.frame_index) {
                        return;
                    }

                    self.last_overlay_frame_index = Some(summary.frame_index);
                    self.last_overlay_frame_time = Some(ctx.current_time());
                    self.set_performance_display(Some(summary), false);
                    self.skip_next_performance_probe = true;
                    Self::request_performance_refresh(ctx);
                } else {
                    self.performance_bootstrap_timer = Some(ctx.schedule_timer_after(0.1));
                }
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let viewport = constraints.clamp(Size::new(1280.0, 720.0));
        self.gallery.measure(ctx, Constraints::tight(viewport));
        self.performance_overlay
            .measure(ctx, Constraints::new(Size::ZERO, viewport));
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.gallery
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
        self.gallery.paint(ctx);
        self.performance_overlay.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut root = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Window, ctx.bounds());
        root.name = Some(WINDOW_TITLE.to_string());
        root.description = Some(WINDOW_DESCRIPTION.to_string());
        ctx.push(root);
        self.gallery.semantics(ctx);
        self.performance_overlay.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.gallery.visit_children(visitor);
        self.performance_overlay.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.gallery.visit_children_mut(visitor);
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
                                            .with_child(Spinner::new("History replay").label("Replaying history cache")),
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
                    .with_child(Spinner::new(SPINNER_NAME).label("Uploading preview tiles")),
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
                "The new sui-debug crate composes reusable diagnostics chrome with SUI-specific views over focus, semantics, widget graph, and scene summaries.",
                SizedBox::new()
                    .height(980.0)
                    .with_child(window_snapshot_view(widget_book_debug_snapshot(&snapshot))),
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
                                SizedBox::new().width(320.0).height(266.0).with_child(
                                    ColorPicker::from_color(COLOR_PICKER_NAME, Color::rgba(0.15, 0.62, 0.48, 0.92)),
                                ),
                            )
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

fn widget_book_debug_snapshot(state: &WidgetBookState) -> WindowDebugSnapshot {
    let window_id = WindowId::new(17);

    let window_widget = WidgetId::new(1000);
    let scroll_widget = WidgetId::new(1001);
    let controls_widget = WidgetId::new(1002);
    let input_widget = WidgetId::new(1003);
    let button_widget = WidgetId::new(1004);
    let slider_widget = WidgetId::new(1005);
    let tabs_widget = WidgetId::new(1006);

    let focused_widget = if state.button_presses >= state.icon_button_presses {
        button_widget
    } else {
        slider_widget
    };

    let mut window_node = SemanticsNode::new(
        window_widget,
        SemanticsRole::Window,
        Rect::new(0.0, 0.0, 1280.0, 720.0),
    );
    window_node.name = Some(WINDOW_TITLE.to_string());
    window_node.description = Some(WINDOW_DESCRIPTION.to_string());

    let mut scroll_node = SemanticsNode::new(
        scroll_widget,
        SemanticsRole::ScrollView,
        Rect::new(0.0, 0.0, 1280.0, 720.0),
    );
    scroll_node.parent = Some(window_widget);
    scroll_node.name = Some("Widget book gallery".to_string());

    let mut controls_node = SemanticsNode::new(
        controls_widget,
        SemanticsRole::GenericContainer,
        Rect::new(24.0, 104.0, 680.0, 220.0),
    );
    controls_node.parent = Some(scroll_widget);
    controls_node.name = Some("Common controls panel".to_string());

    let mut input_node = SemanticsNode::new(
        input_widget,
        SemanticsRole::TextInput,
        Rect::new(48.0, 176.0, 320.0, 42.0),
    );
    input_node.parent = Some(controls_widget);
    input_node.name = Some(NAME_INPUT_LABEL.to_string());
    input_node.value = Some(SemanticsValue::Text(state.name.clone()));

    let mut button_node = SemanticsNode::new(
        button_widget,
        SemanticsRole::Button,
        Rect::new(48.0, 238.0, 180.0, 40.0),
    );
    button_node.parent = Some(controls_widget);
    button_node.name = Some(PRIMARY_BUTTON_LABEL.to_string());
    button_node.description = Some(format!("pressed {} times", state.button_presses));
    button_node.state.focused = focused_widget == button_widget;

    let mut slider_node = SemanticsNode::new(
        slider_widget,
        SemanticsRole::Slider,
        Rect::new(48.0, 346.0, 320.0, 38.0),
    );
    slider_node.parent = Some(scroll_widget);
    slider_node.name = Some(SLIDER_NAME.to_string());
    slider_node.value = Some(SemanticsValue::Range {
        value: state.slider_value,
        min: 0.0,
        max: 100.0,
    });
    slider_node.state.focused = focused_widget == slider_widget;

    let mut tabs_node = SemanticsNode::new(
        tabs_widget,
        SemanticsRole::Tabs,
        Rect::new(48.0, 428.0, 420.0, 172.0),
    );
    tabs_node.parent = Some(scroll_widget);
    tabs_node.name = Some(TABS_NAME.to_string());
    tabs_node.value = Some(SemanticsValue::Text(state.tabs_choice.clone()));

    let accessibility = AccessibilitySnapshot::new(
        window_id,
        vec![
            window_node,
            scroll_node,
            controls_node,
            input_node,
            button_node,
            slider_node,
            tabs_node,
        ],
    );

    let widget_graph = WidgetGraphSnapshot {
        root: window_widget,
        nodes: vec![
            WidgetNodeSnapshot {
                id: window_widget,
                parent: None,
                children: vec![scroll_widget],
                measured_size: Size::new(1280.0, 720.0),
                bounds: Rect::new(0.0, 0.0, 1280.0, 720.0),
                accepts_focus: false,
                focused: false,
            },
            WidgetNodeSnapshot {
                id: scroll_widget,
                parent: Some(window_widget),
                children: vec![controls_widget, slider_widget, tabs_widget],
                measured_size: Size::new(1280.0, 720.0),
                bounds: Rect::new(0.0, 0.0, 1280.0, 720.0),
                accepts_focus: false,
                focused: false,
            },
            WidgetNodeSnapshot {
                id: controls_widget,
                parent: Some(scroll_widget),
                children: vec![input_widget, button_widget],
                measured_size: Size::new(680.0, 220.0),
                bounds: Rect::new(24.0, 104.0, 680.0, 220.0),
                accepts_focus: false,
                focused: false,
            },
            WidgetNodeSnapshot {
                id: input_widget,
                parent: Some(controls_widget),
                children: Vec::new(),
                measured_size: Size::new(320.0, 42.0),
                bounds: Rect::new(48.0, 176.0, 320.0, 42.0),
                accepts_focus: true,
                focused: false,
            },
            WidgetNodeSnapshot {
                id: button_widget,
                parent: Some(controls_widget),
                children: Vec::new(),
                measured_size: Size::new(180.0, 40.0),
                bounds: Rect::new(48.0, 238.0, 180.0, 40.0),
                accepts_focus: true,
                focused: focused_widget == button_widget,
            },
            WidgetNodeSnapshot {
                id: slider_widget,
                parent: Some(scroll_widget),
                children: Vec::new(),
                measured_size: Size::new(320.0, 38.0),
                bounds: Rect::new(48.0, 346.0, 320.0, 38.0),
                accepts_focus: true,
                focused: focused_widget == slider_widget,
            },
            WidgetNodeSnapshot {
                id: tabs_widget,
                parent: Some(scroll_widget),
                children: Vec::new(),
                measured_size: Size::new(420.0, 172.0),
                bounds: Rect::new(48.0, 428.0, 420.0, 172.0),
                accepts_focus: true,
                focused: false,
            },
        ],
    };

    let mut dirty_regions = vec![DirtyRegion::new(
        Rect::new(44.0, 164.0, 336.0, 132.0),
        InvalidationKind::Paint,
    )];
    if state.switch_on {
        dirty_regions.push(DirtyRegion::new(
            Rect::new(44.0, 334.0, 340.0, 58.0),
            InvalidationKind::Semantics,
        ));
    }
    if !state.notes.trim().is_empty() {
        dirty_regions.push(DirtyRegion::new(
            Rect::new(40.0, 616.0, 620.0, 88.0),
            InvalidationKind::Text,
        ));
    }

    let scene_summary = SceneDebugSummary {
        viewport: Size::new(1280.0, 720.0),
        dirty_region_count: dirty_regions.len(),
        dirty_regions,
        command_count: 28 + state.name.len() + state.button_presses + state.icon_button_presses,
        command_breakdown: vec![
            ("Clear".to_string(), 1),
            ("FillRect".to_string(), 8),
            ("DrawText".to_string(), 11 + state.name.len()),
            ("PushClip".to_string(), 2),
            ("PopClip".to_string(), 2),
            ("DrawImage".to_string(), 1),
            (
                "Label".to_string(),
                4 + state.button_presses + state.icon_button_presses,
            ),
        ],
        detail_collected: true,
    };

    WindowDebugSnapshot::new(
        WINDOW_TITLE,
        window_id,
        FocusState {
            focused_widget: Some(focused_widget),
            window_focused: true,
        },
        accessibility,
        widget_graph,
    )
    .with_schedule(FrameSchedule {
        measure: false,
        arrange: false,
        paint: true,
        semantics: state.switch_on,
        hit_test: false,
        text: !state.notes.trim().is_empty(),
        resources: state.dialog_apply_count > 0,
    })
    .with_scene_summary(scene_summary)
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
}

impl WidgetBookSummary {
    fn new(state: Rc<RefCell<WidgetBookState>>) -> Self {
        Self { state }
    }
}

struct LivePerformancePanel {
    display: Rc<RefCell<LivePerformanceDisplay>>,
    lines: Vec<LivePerformanceLine>,
    snapshot: Option<WindowPerformanceSummary>,
    idle: bool,
    detail_mode: SceneStatisticsDetailMode,
    toggle_pressed: bool,
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
            lines: Vec::new(),
            snapshot: None,
            idle: false,
            detail_mode: SceneStatisticsDetailMode::Lightweight,
            toggle_pressed: false,
        }
    }

    fn set_snapshot(&mut self, next_snapshot: Option<WindowPerformanceSummary>) -> bool {
        if self.snapshot == next_snapshot {
            return false;
        }

        self.snapshot = next_snapshot;
        true
    }

    fn set_detail_mode(&mut self, next_detail_mode: SceneStatisticsDetailMode) -> bool {
        if self.detail_mode == next_detail_mode {
            return false;
        }

        self.detail_mode = next_detail_mode;
        true
    }

    fn set_idle(&mut self, next_idle: bool) -> bool {
        if self.idle == next_idle {
            return false;
        }

        self.idle = next_idle;
        true
    }

    fn refresh(&mut self, window_id: WindowId) -> bool {
        let display = *self.display.borrow();
        let mut changed = self.set_snapshot(display.snapshot);
        changed |= self.set_idle(display.idle);
        changed |= self.set_detail_mode(window_scene_statistics_detail_mode(window_id));
        changed
    }

    fn toggle_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.max_x() - Self::PADDING_X - Self::TOGGLE_WIDTH,
            bounds.y() + Self::PADDING_Y - 1.0,
            Self::TOGGLE_WIDTH,
            Self::TOGGLE_HEIGHT,
        )
    }

    fn toggle_label(&self) -> &'static str {
        if self.detail_mode.is_detailed() {
            "detail on"
        } else {
            "detail off"
        }
    }

    fn rebuild_lines(&mut self, _ctx: &MeasureCtx, width: f32) -> f32 {
        let _text_width = (width - Self::PADDING_X * 2.0).max(1.0);
        let mut y = Self::PADDING_Y;
        let mut lines = Vec::new();

        for spec in self.content_specs() {
            let line_height = spec.style.line_height.max(spec.style.font_size);
            lines.push(LivePerformanceLine {
                y,
                text: spec.text,
                style: spec.style,
            });
            y += line_height + spec.spacing_after;
        }

        self.lines = lines;
        y + Self::PADDING_Y
    }

    fn content_specs(&self) -> Vec<LivePerformanceLineSpec> {
        match &self.snapshot {
            Some(snapshot) => {
                let slowest_phase = snapshot.slowest_phase;
                let slowest_label = slowest_phase
                    .map(|sample| sample.phase.label())
                    .unwrap_or("idle");
                let slowest_duration = slowest_phase
                    .map(|sample| format_duration_ms(sample.duration_ms))
                    .unwrap_or_else(|| "0.0 ms".to_string());

                vec![
                    LivePerformanceLineSpec::title("live performance".to_string()),
                    LivePerformanceLineSpec::headline(format!(
                        "{}  |  {}",
                        if self.idle {
                            "0 fps".to_string()
                        } else {
                            format_fps(snapshot.total_time_ms)
                        },
                        if self.idle {
                            "idle".to_string()
                        } else {
                            format_duration_ms(snapshot.total_time_ms)
                        },
                    )),
                    LivePerformanceLineSpec::metric(if self.idle {
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
                    LivePerformanceLineSpec::metric(format!(
                        "scene {} dirty  |  {} cmds  |  {:.0}% dirty",
                        snapshot.dirty_region_count,
                        snapshot.command_count,
                        snapshot.dirty_coverage,
                    )),
                    LivePerformanceLineSpec::metric(format!(
                        "text cache rt {}  |  rr {}  |  glyph {}",
                        snapshot.text_caches.runtime_layout.entries,
                        snapshot.text_caches.renderer_layout.entries,
                        snapshot.text_caches.renderer_glyph.entries,
                    )),
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
        if ctx.phase() != sui::EventPhase::Capture {
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
                    let next_mode = if self.detail_mode.is_detailed() {
                        SceneStatisticsDetailMode::Lightweight
                    } else {
                        SceneStatisticsDetailMode::Detailed
                    };
                    set_window_scene_statistics_detail_mode(ctx.window_id(), next_mode);
                    self.detail_mode = next_mode;
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else if was_pressed {
                    ctx.request_paint();
                }
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

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.refresh(ctx.window_id());
        let width = if constraints.max.width.is_finite() {
            constraints.max.width.min(Self::WIDTH)
        } else {
            Self::WIDTH
        };
        let height = self.rebuild_lines(ctx, width);
        constraints.clamp(Size::new(width, height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
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
        let toggle_fill = if self.detail_mode.is_detailed() {
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
        let toggle_stroke = if self.detail_mode.is_detailed() {
            Color::rgba(0.05, 0.28, 0.44, 1.0)
        } else {
            Color::rgba(0.76, 0.82, 0.88, 1.0)
        };
        ctx.fill(toggle_shape.clone(), toggle_fill);
        ctx.stroke(toggle_shape, toggle_stroke, StrokeStyle::new(1.0));
        ctx.draw_text(
            toggle_bounds,
            self.toggle_label().to_string(),
            text_style(
                if self.detail_mode.is_detailed() {
                    Color::rgba(0.98, 0.995, 1.0, 1.0)
                } else {
                    Color::rgba(0.33, 0.40, 0.48, 1.0)
                },
                10.0,
                14.0,
            ),
        );

        ctx.push_clip_rect(ctx.bounds());
        for line in &self.lines {
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
        node.value = Some(SemanticsValue::Text(self.toggle_label().to_string()));
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
    use std::{cell::RefCell, fs, path::Path, path::PathBuf, rc::Rc};

    use super::{
        BREADCRUMB_NAME, COLOR_PICKER_NAME, COLOR_SWATCH_NAME, CONTEXT_MENU_NAME, DEMO_IMAGE_LABEL,
        DARK_PREVIEW_ACTION_LABEL, DIALOG_TITLE, DIALOG_TRIGGER_LABEL, GALLERY_SCROLL_NAME,
        ICON_BUTTON_LABEL, ICON_LABEL, LIST_VIEW_NAME, LivePerformanceDisplay,
        LivePerformancePanel, MENU_NAME, NAME_INPUT_LABEL, NUMBER_INPUT_NAME, POPOVER_NAME, POPOVER_TRIGGER_LABEL,
        PRIMARY_BUTTON_LABEL, PROGRESS_NAME, RADIO_BUTTON_LABEL, RADIO_GROUP_NAME,
        SELECT_NAME, SLIDER_NAME, SPINNER_NAME, SPLIT_VIEW_NAME, SUBSCRIBE_LABEL,
        SUMMARY_NAME, SWITCH_LABEL, TAB_BAR_NAME, TAB_BAR_OPTIONS, TAB_PANEL_OPTIONS,
        TABLE_NAME, TABS_NAME, TEXT_AREA_LABEL, THEME_PREVIEW_NAME,
        THEME_PREVIEW_TOGGLE_LABEL, TOOLBAR_SEPARATOR_NAME, TOOLTIP_TEXT,
        TOOLTIP_TRIGGER_LABEL, TREE_VIEW_NAME,
        WidgetBookState, build_widget_book_application, default_widget_book_state,
    };
    use sui::{
        Application, Error, Event, FramePhase, FramePhaseSample, Point, PointerButton,
        PointerButtons, PointerEvent, PointerEventKind, Rect,
        RendererSubmissionDiagnostics, Result, SemanticsRole, SemanticsValue,
        TextCacheDiagnostics, Vector, Widget, WidgetPod,
        WidgetPodVisitor, WindowBuilder, WindowId, WindowPerformanceSummary,
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
        TabBar,
        Tabs,
        Menu,
        ContextMenuOpen,
        TooltipVisible,
        PopoverOpen,
        Dialog,
        ProgressBar,
        Spinner,
        ScrollViewScrolled,
        Summary,
        ListView,
        TreeView,
        Table,
        SplitView,
        Breadcrumb,
        ColorSwatch,
        ColorPicker,
        ThemePreview,
        ImageWidget,
    }

    impl StoryCase {
        const ALL: [Self; 39] = [
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
            Self::TabBar,
            Self::Tabs,
            Self::Menu,
            Self::ContextMenuOpen,
            Self::TooltipVisible,
            Self::PopoverOpen,
            Self::Dialog,
            Self::ProgressBar,
            Self::Spinner,
            Self::ScrollViewScrolled,
            Self::Summary,
            Self::ListView,
            Self::TreeView,
            Self::Table,
            Self::SplitView,
            Self::Breadcrumb,
            Self::ColorSwatch,
            Self::ColorPicker,
            Self::ThemePreview,
            Self::ImageWidget,
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
                Self::TabBar => "tab-bar",
                Self::Tabs => "tabs",
                Self::Menu => "menu",
                Self::ContextMenuOpen => "context-menu-open",
                Self::TooltipVisible => "tooltip-visible",
                Self::PopoverOpen => "popover-open",
                Self::Dialog => "dialog",
                Self::ProgressBar => "progress-bar",
                Self::Spinner => "spinner",
                Self::ScrollViewScrolled => "scroll-view-scrolled",
                Self::Summary => "summary",
                Self::ListView => "list-view",
                Self::TreeView => "tree-view",
                Self::Table => "table",
                Self::SplitView => "split-view",
                Self::Breadcrumb => "breadcrumb",
                Self::ColorSwatch => "color-swatch",
                Self::ColorPicker => "color-picker",
                Self::ThemePreview => "theme-preview",
                Self::ImageWidget => "image-widget",
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
                Self::TabBar => "Standalone tab bar crop for editor-style navigation.",
                Self::Tabs => "Tabs crop showing selected panel content.",
                Self::Menu => "Command menu crop for overflow and app menus.",
                Self::ContextMenuOpen => "Open context menu crop anchored to a layer tile.",
                Self::TooltipVisible => "Tooltip crop while the trigger is hovered.",
                Self::PopoverOpen => "Open popover crop for inline inspector content.",
                Self::Dialog => "Dialog crop for confirmations and settings.",
                Self::ProgressBar => "Progress bar crop for long-running tasks.",
                Self::Spinner => "Busy indicator crop for indeterminate work.",
                Self::ScrollViewScrolled => {
                    "Outer widget-book scroll view after paging down through the gallery."
                }
                Self::Summary => "Composed summary panel showing derived state.",
                Self::ListView => "List view crop for asset browser and inspector collections.",
                Self::TreeView => "Tree view crop for layers, files, and scene hierarchies.",
                Self::Table => "Table crop for structured tool data and data-grid layouts.",
                Self::SplitView => "Split view crop with the resizable divider in an editor shell.",
                Self::Breadcrumb => "Breadcrumb crop for path and project navigation surfaces.",
                Self::ColorSwatch => {
                    "Color swatch crop for palette chips and compact property rows."
                }
                Self::ColorPicker => {
                    "Color picker crop for interactive color adjustment workflows."
                }
                Self::ThemePreview => {
                    "Theme preview panel with the light and dark comparison cards visible."
                }
                Self::ImageWidget => {
                    "Image widget crop for previews, thumbnails, and asset panels."
                }
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
                | Self::TabBar
                | Self::Tabs
                | Self::Menu
                | Self::ContextMenuOpen
                | Self::TooltipVisible
                | Self::PopoverOpen
                | Self::Dialog
                | Self::ProgressBar
                | Self::Spinner
                | Self::ScrollViewScrolled
                | Self::ListView
                | Self::TreeView
                | Self::Table
                | Self::SplitView
                | Self::Breadcrumb
                | Self::ColorSwatch
                | Self::ColorPicker
                | Self::ThemePreview
                | Self::ImageWidget => default_widget_book_state(),
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
            if !matches!(
                self,
                Self::Overview | Self::OverviewConfigured | Self::ScrollViewScrolled
            ) {
                scroll_to_story_target(window, self, 12)?;
            }

            match self {
                Self::Button
                | Self::Checkbox
                | Self::CheckboxUnchecked
                | Self::FilledInput
                | Self::Icon
                | Self::IconButton
                | Self::Separator
                | Self::Switch
                | Self::RadioButton
                | Self::RadioGroup
                | Self::Slider
                | Self::NumberInput
                | Self::TabBar
                | Self::Tabs
                | Self::Menu
                | Self::ProgressBar
                | Self::Spinner
                | Self::Summary
                | Self::ListView
                | Self::TreeView
                | Self::Table
                | Self::SplitView
                | Self::Breadcrumb
                | Self::ColorSwatch
                | Self::ColorPicker
                | Self::ThemePreview
                | Self::ImageWidget => Ok(()),
                Self::ButtonHover => {
                    self.target(window).hover()
                }
                Self::ButtonPressed => {
                    press_target(window, SemanticsRole::Button, PRIMARY_BUTTON_LABEL)
                }
                Self::EmptyInputFocused => {
                    self.target(window).focus()
                }
                Self::SelectExpanded => {
                    if matches!(self, Self::SelectExpanded) {
                        self.target(window).click()?;
                    }
                    Ok(())
                }
                Self::ContextMenuOpen
                | Self::TooltipVisible
                | Self::PopoverOpen
                | Self::Dialog => {
                    match self {
                        Self::ContextMenuOpen => secondary_click_target(
                            window,
                            SemanticsRole::ContextMenu,
                            CONTEXT_MENU_NAME,
                        ),
                        Self::TooltipVisible => window
                            .get_by_role(SemanticsRole::Button)
                            .with_name(TOOLTIP_TRIGGER_LABEL)
                            .hover(),
                        Self::PopoverOpen => self.target(window).click(),
                        Self::Dialog => window
                            .get_by_role(SemanticsRole::Button)
                            .with_name(DIALOG_TRIGGER_LABEL)
                            .click(),
                        _ => Ok(()),
                    }
                }
                Self::TextArea => Ok(()),
                Self::ScrollViewScrolled => scroll_gallery(window, 1),
                Self::Overview
                | Self::OverviewConfigured
                    => Ok(()),
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
                Self::Separator => window
                    .get_by_role(SemanticsRole::Separator)
                    .with_name(TOOLBAR_SEPARATOR_NAME),
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
                Self::TabBar => window
                    .get_by_role(SemanticsRole::TabBar)
                    .with_name(TAB_BAR_NAME),
                Self::Tabs => window.get_by_role(SemanticsRole::Tabs).with_name(TABS_NAME),
                Self::Menu => window.get_by_role(SemanticsRole::Menu).with_name(MENU_NAME),
                Self::ContextMenuOpen => window
                    .get_by_role(SemanticsRole::ContextMenu)
                    .with_name(CONTEXT_MENU_NAME),
                Self::TooltipVisible => window
                    .get_by_role(SemanticsRole::Tooltip)
                    .with_name(TOOLTIP_TEXT),
                Self::PopoverOpen => window
                    .get_by_role(SemanticsRole::Popover)
                    .with_name(POPOVER_NAME),
                Self::Dialog => window
                    .get_by_role(SemanticsRole::Dialog)
                    .with_name(DIALOG_TITLE),
                Self::ProgressBar => window
                    .get_by_role(SemanticsRole::ProgressBar)
                    .with_name(PROGRESS_NAME),
                Self::Spinner => window
                    .get_by_role(SemanticsRole::BusyIndicator)
                    .with_name(SPINNER_NAME),
                Self::ScrollViewScrolled => window
                    .get_by_role(SemanticsRole::ScrollView)
                    .with_name(GALLERY_SCROLL_NAME),
                Self::Summary => window
                    .get_by_role(SemanticsRole::GenericContainer)
                    .with_name(SUMMARY_NAME),
                Self::ListView => window
                    .get_by_role(SemanticsRole::List)
                    .with_name(LIST_VIEW_NAME),
                Self::TreeView => window
                    .get_by_role(SemanticsRole::Tree)
                    .with_name(TREE_VIEW_NAME),
                Self::Table => window
                    .get_by_role(SemanticsRole::Table)
                    .with_name(TABLE_NAME),
                Self::SplitView => window
                    .get_by_role(SemanticsRole::Splitter)
                    .with_name(SPLIT_VIEW_NAME),
                Self::Breadcrumb => window
                    .get_by_role(SemanticsRole::Breadcrumb)
                    .with_name(BREADCRUMB_NAME),
                Self::ColorSwatch => window
                    .get_by_role(SemanticsRole::ColorSwatch)
                    .with_name(COLOR_SWATCH_NAME),
                Self::ColorPicker => window
                    .get_by_role(SemanticsRole::ColorPicker)
                    .with_name(COLOR_PICKER_NAME),
                Self::ThemePreview => window
                    .get_by_role(SemanticsRole::GenericContainer)
                    .with_name(THEME_PREVIEW_NAME),
                Self::ImageWidget => window
                    .get_by_role(SemanticsRole::Image)
                    .with_name(DEMO_IMAGE_LABEL),
            }
        }

        fn story_node(self) -> Option<(SemanticsRole, Option<&'static str>)> {
            match self {
                Self::Button | Self::ButtonHover | Self::ButtonPressed => {
                    Some((SemanticsRole::Button, Some(PRIMARY_BUTTON_LABEL)))
                }
                Self::Checkbox | Self::CheckboxUnchecked => {
                    Some((SemanticsRole::CheckBox, Some(SUBSCRIBE_LABEL)))
                }
                Self::FilledInput | Self::EmptyInputFocused => {
                    Some((SemanticsRole::TextInput, Some(NAME_INPUT_LABEL)))
                }
                Self::Icon => Some((SemanticsRole::Image, Some(ICON_LABEL))),
                Self::IconButton => Some((SemanticsRole::Button, Some(ICON_BUTTON_LABEL))),
                Self::Separator => {
                    Some((SemanticsRole::Separator, Some(TOOLBAR_SEPARATOR_NAME)))
                }
                Self::Switch => Some((SemanticsRole::Switch, Some(SWITCH_LABEL))),
                Self::RadioButton => {
                    Some((SemanticsRole::RadioButton, Some(RADIO_BUTTON_LABEL)))
                }
                Self::RadioGroup => Some((SemanticsRole::RadioGroup, Some(RADIO_GROUP_NAME))),
                Self::Slider => Some((SemanticsRole::Slider, Some(SLIDER_NAME))),
                Self::NumberInput => Some((SemanticsRole::SpinBox, Some(NUMBER_INPUT_NAME))),
                Self::TextArea => Some((SemanticsRole::TextInput, Some(TEXT_AREA_LABEL))),
                Self::SelectExpanded => Some((SemanticsRole::ComboBox, Some(SELECT_NAME))),
                Self::TabBar => Some((SemanticsRole::TabBar, Some(TAB_BAR_NAME))),
                Self::Tabs => Some((SemanticsRole::Tabs, Some(TABS_NAME))),
                Self::Menu => Some((SemanticsRole::Menu, Some(MENU_NAME))),
                Self::ContextMenuOpen => {
                    Some((SemanticsRole::ContextMenu, Some(CONTEXT_MENU_NAME)))
                }
                Self::TooltipVisible => Some((SemanticsRole::Button, Some(TOOLTIP_TRIGGER_LABEL))),
                Self::PopoverOpen => Some((SemanticsRole::Button, Some(POPOVER_TRIGGER_LABEL))),
                Self::Dialog => Some((SemanticsRole::Button, Some(DIALOG_TRIGGER_LABEL))),
                Self::ProgressBar => Some((SemanticsRole::ProgressBar, Some(PROGRESS_NAME))),
                Self::Spinner => Some((SemanticsRole::BusyIndicator, Some(SPINNER_NAME))),
                Self::Summary => Some((SemanticsRole::GenericContainer, Some(SUMMARY_NAME))),
                Self::ListView => Some((SemanticsRole::List, Some(LIST_VIEW_NAME))),
                Self::TreeView => Some((SemanticsRole::Tree, Some(TREE_VIEW_NAME))),
                Self::Table => Some((SemanticsRole::Table, Some(TABLE_NAME))),
                Self::SplitView => Some((SemanticsRole::Splitter, Some(SPLIT_VIEW_NAME))),
                Self::Breadcrumb => Some((SemanticsRole::Breadcrumb, Some(BREADCRUMB_NAME))),
                Self::ColorSwatch => Some((SemanticsRole::ColorSwatch, Some(COLOR_SWATCH_NAME))),
                Self::ColorPicker => Some((SemanticsRole::ColorPicker, Some(COLOR_PICKER_NAME))),
                Self::ThemePreview => {
                    Some((SemanticsRole::GenericContainer, Some(THEME_PREVIEW_NAME)))
                }
                Self::ImageWidget => Some((SemanticsRole::Image, Some(DEMO_IMAGE_LABEL))),
                _ => None,
            }
        }
    }

    #[test]
    fn widget_book_theme_preview_toggle_hides_dark_card() -> Result<()> {
        let app = TestApp::from_runtime(build_widget_book_application(default_widget_book_state()).build()?)?;
        let window = app.main_window()?;

        scroll_to_story_target(&window, StoryCase::ThemePreview, 2)?;

        let snapshot = window.snapshot()?;
        assert!(snapshot.accessibility.nodes.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(DARK_PREVIEW_ACTION_LABEL)
        }));

        window
            .get_by_role(SemanticsRole::Switch)
            .with_name(THEME_PREVIEW_TOGGLE_LABEL)
            .click()?;

        let snapshot = window.snapshot()?;
        assert!(!snapshot.accessibility.nodes.iter().any(|node| {
            node.role == SemanticsRole::Button
                && node.name.as_deref() == Some(DARK_PREVIEW_ACTION_LABEL)
        }));

        Ok(())
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

        scroll_to_story_target(&window, StoryCase::Summary, 12)?;
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
    fn widget_book_gallery_wheel_scroll_updates_screenshot_and_reveals_lower_story() -> Result<()> {
        let app = TestApp::from_runtime(build_widget_book_application(default_widget_book_state()).build()?)?;
        let window = app.main_window()?;
        let gallery = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);

        let before = gallery.capture_screenshot()?;
        let before_snapshot = window.snapshot()?;
        let before_button = before_snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(PRIMARY_BUTTON_LABEL)
            })
            .expect("primary button present before wheel scroll")
            .bounds;

        gallery.scroll_pixels(Vector::new(0.0, -360.0))?;

        let after = gallery.capture_screenshot()?;
        let after_snapshot = window.snapshot()?;
        let after_button = after_snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::Button
                    && node.name.as_deref() == Some(PRIMARY_BUTTON_LABEL)
            })
            .expect("primary button present after wheel scroll")
            .bounds;

        assert_ne!(before, after);
        assert!(after_button.y() < before_button.y());

        Ok(())
    }

    // #[test]
    // fn widget_book_generates_visual_artifacts() -> Result<()> {
    //     let artifact_root = artifact_root();
    //     reset_dir(&artifact_root)?;

    //     for story in StoryCase::ALL {
    //         let story_dir = artifact_root.join(story.id());
    //         create_dir(&story_dir)?;

    //         let app = story.build_app()?;
    //         let window = app.main_window()?;
    //         story.prepare(&window)?;
    //         let artifacts = window.capture_artifacts()?;
    //         artifacts.write_to_dir(&story_dir)?;
    //         rename_window_artifacts(&story_dir)?;

    //         let locator = story.target(&window);
    //         let screenshot = locator.capture_screenshot().map_err(|error| {
    //             Error::new(format!(
    //                 "widget book story {} failed to capture screenshot: {}",
    //                 story.id(),
    //                 error
    //             ))
    //         })?;
    //         screenshot.write_png(story_dir.join("screenshot.png"))?;
    //         write_text(story_dir.join("story.txt"), story.description())?;
    //     }

    //     for story in StoryCase::ALL {
    //         assert!(
    //             artifact_root
    //                 .join(story.id())
    //                 .join("screenshot.png")
    //                 .exists()
    //         );
    //     }

    //     Ok(())
    // }

    #[test]
    fn widget_book_configured_story_exposes_expected_semantics() -> Result<()> {
        let app = StoryCase::Summary.build_app()?;
        let window = app.main_window()?;
        let top_snapshot = window.snapshot()?;

        let input = top_snapshot
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

        scroll_to_story_target(&window, StoryCase::Slider, 12)?;
        let controls_snapshot = window.snapshot()?;

        let slider = controls_snapshot
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

        scroll_to_story_target(&window, StoryCase::Summary, 12)?;
        let summary_snapshot = window.snapshot()?;

        let summary = summary_snapshot
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
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("tab bar: Export"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("tabs: History"))
        );
        assert!(
            summary
                .description
                .as_deref()
                .is_some_and(|description| description.contains("dialog apply: 2"))
        );

        let number = controls_snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::SpinBox
                    && node.name.as_deref() == Some(NUMBER_INPUT_NAME)
            })
            .expect("number input semantics node present");
        assert_eq!(number.value, Some(SemanticsValue::Number(24.0)));

        let select = controls_snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| {
                node.role == SemanticsRole::ComboBox && node.name.as_deref() == Some(SELECT_NAME)
            })
            .expect("select semantics node present");
        assert_eq!(
            select.value,
            Some(SemanticsValue::Text("Multiply".to_string()))
        );

        Ok(())
    }

    #[test]
    fn live_performance_panel_replaces_snapshot_without_creating_children() {
        let mut panel = LivePerformancePanel::new();

        assert!(panel.set_snapshot(Some(sample_window_performance_snapshot())));
        assert!(!panel.set_snapshot(panel.snapshot.clone()));
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

        let mut panel = LivePerformancePanel::new();
        let mut visitor = CountingVisitor { count: 0 };
        Widget::visit_children(&panel, &mut visitor);
        assert_eq!(visitor.count, 0);

        assert!(panel.set_snapshot(Some(sample_window_performance_snapshot())));

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
            snapshot: Some(sample_window_performance_snapshot()),
            idle: true,
        }));
        let mut panel = LivePerformancePanel::with_display(display);

        assert!(panel.refresh(WindowId::new(11)));

        let lines = panel.content_specs();
        assert_eq!(lines[1].text, "0 fps  |  idle");
        assert!(lines[2].text.contains("last active"));
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

    fn configured_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
        Rc::new(RefCell::new(WidgetBookState {
            name: "Grace Hopper".to_string(),
            subscribed: false,
            theme_preview_comparison: true,
            button_presses: 1,
            icon_button_presses: 2,
            switch_on: false,
            standalone_radio_selected: true,
            radio_choice: "High".to_string(),
            slider_value: 35.0,
            number_value: 24.0,
            notes: "Line 1\nLine 2".to_string(),
            mode: "Multiply".to_string(),
            tab_bar_choice: "Export".to_string(),
            tabs_choice: "History".to_string(),
            last_menu_action: "Delete layer".to_string(),
            last_context_action: "Duplicate".to_string(),
            dialog_apply_count: 2,
        }))
    }

    fn sample_window_performance_snapshot() -> WindowPerformanceSummary {
        WindowPerformanceSummary {
            window_id: WindowId::new(11),
            frame_index: 7,
            total_time_ms: 1.5,
            slowest_phase: Some(FramePhaseSample::new(FramePhase::Renderer, 1.5)),
            renderer_submission: RendererSubmissionDiagnostics::new(
                2,
                6,
                2048,
                3,
                18,
                15,
                3,
                6,
                65536,
                420,
                160,
            ),
            text_caches: TextCacheDiagnostics::default(),
            dirty_region_count: 0,
            dirty_coverage: 0.0,
            command_count: 0,
        }
    }

    fn blank_widget_book_state() -> Rc<RefCell<WidgetBookState>> {
        Rc::new(RefCell::new(WidgetBookState {
            name: String::new(),
            subscribed: false,
            theme_preview_comparison: true,
            button_presses: 0,
            icon_button_presses: 0,
            switch_on: false,
            standalone_radio_selected: false,
            radio_choice: "Balanced".to_string(),
            slider_value: 50.0,
            number_value: 8.0,
            notes: String::new(),
            mode: String::new(),
            tab_bar_choice: TAB_BAR_OPTIONS[0].to_string(),
            tabs_choice: TAB_PANEL_OPTIONS[0].to_string(),
            last_menu_action: String::new(),
            last_context_action: String::new(),
            dialog_apply_count: 0,
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

    fn secondary_click_target(window: &TestWindow, role: SemanticsRole, name: &str) -> Result<()> {
        let locator = window.get_by_role(role.clone()).with_name(name);
        let point = node_center(window, role, name)?;

        locator.dispatch_event(Event::Pointer(PointerEvent::new(
            PointerEventKind::Move,
            point,
        )))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.button = Some(PointerButton::Secondary);
        down.buttons = PointerButtons::new(2);
        locator.dispatch_event(Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, point);
        up.button = Some(PointerButton::Secondary);
        locator.dispatch_event(Event::Pointer(up))
    }

    fn scroll_gallery(window: &TestWindow, pages: usize) -> Result<()> {
        let locator = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);
        for _ in 0..pages {
            locator.scroll_pixels(Vector::new(0.0, -360.0))?;
        }
        Ok(())
    }

    fn scroll_to_story_target(
        window: &TestWindow,
        story: StoryCase,
        max_pages: usize,
    ) -> Result<()> {
        let Some((role, name)) = story.story_node() else {
            return Ok(());
        };

        if story_node_is_visible(window, role.clone(), name)? {
            return Ok(());
        }

        let locator = window
            .get_by_role(SemanticsRole::ScrollView)
            .with_name(GALLERY_SCROLL_NAME);
        for _ in 0..max_pages {
            locator.scroll_pixels(Vector::new(0.0, -360.0))?;
            if story_node_is_visible(window, role.clone(), name)? {
                return Ok(());
            }
        }

        Err(Error::new(format!(
            "failed to scroll story target {:?} {:?} into view",
            role, name
        )))
    }

    fn story_node_is_visible(
        window: &TestWindow,
        role: SemanticsRole,
        name: Option<&str>,
    ) -> Result<bool> {
        let snapshot = window.snapshot()?;
        let viewport = snapshot
            .accessibility
            .nodes
            .iter()
            .find(|node| node.role == SemanticsRole::Window)
            .map(|node| node.bounds)
            .unwrap_or(Rect::ZERO);
        Ok(snapshot.accessibility.nodes.iter().any(|node| {
            node.role == role
                && node.name.as_deref() == name
                && node.bounds.intersection(viewport).is_some()
        }))
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
