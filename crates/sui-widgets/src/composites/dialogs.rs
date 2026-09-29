use crate::Button;
use crate::ButtonAppearance;
use crate::DefaultTheme;
use crate::Progress;
use crate::SemanticTone;
use crate::animation::AnimationSpec;
use crate::animation::Reveal;
use crate::composites::forms::set_focus_animation_target;
use crate::composites::indicators::{
    draw_control_frame, draw_focus_ring_frame, measure_text, physical_pixels, rounded_rect_path,
    text_token_style,
};
use crate::composites::popups::request_child_invalidation;
use crate::paint_theme_shadow;
use crate::text_align::paint_text;
use std::cell::RefCell;
use std::rc::Rc;
use sui_core::Event;
use sui_core::InvalidationKind;
use sui_core::KeyState;
use sui_core::Point;
use sui_core::PointerButton;
use sui_core::PointerEventKind;
use sui_core::Rect;
use sui_core::SemanticsAction;
use sui_core::SemanticsNode;
use sui_core::SemanticsRole;
use sui_core::Size;
use sui_core::Transform;
use sui_core::Vector;
use sui_core::WakeEvent;
use sui_core::WidgetId;
use sui_layout::Constraints;
use sui_reactive::Signal;
use sui_runtime::ArrangeCtx;
use sui_runtime::Command;
use sui_runtime::EventCtx;
use sui_runtime::FrameClock;
use sui_runtime::LayerOptions;
use sui_runtime::MeasureCtx;
use sui_runtime::OVERLAY_DISMISS_REQUEST;
use sui_runtime::OverlayDismissPolicy;
use sui_runtime::OverlayFocusBehavior;
use sui_runtime::OverlayKind;
use sui_runtime::OverlayOptions;
use sui_runtime::PaintBoundaryMode;
use sui_runtime::PaintCtx;
use sui_runtime::SemanticsCtx;
use sui_runtime::SingleChild;
use sui_runtime::StackSurfaceOptions;
use sui_runtime::Widget;
use sui_runtime::WidgetChildren;
use sui_runtime::WidgetPod;
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::LayerCompositionMode;
use sui_scene::LayerProperties;
use sui_scene::StrokeStyle;
use sui_text::TextStyle;
use sui_text::{TextAlign, TextMeasurement};

/// A theme source shared between a surface and the action buttons it builds.
/// Actions read the surface's current theme through it, so they follow
/// `theme(...)` and `theme_when(...)` regardless of builder call order and
/// keep following live theme switches.
#[derive(Clone)]
pub(super) struct SharedTheme(Rc<RefCell<SharedThemeSource>>);

enum SharedThemeSource {
    Fixed(Box<DefaultTheme>),
    Live(Box<dyn Fn() -> DefaultTheme>),
}

impl SharedTheme {
    pub(super) fn new() -> Self {
        Self(Rc::new(RefCell::new(SharedThemeSource::Fixed(
            Box::default(),
        ))))
    }

    pub(super) fn set(&self, theme: DefaultTheme) {
        *self.0.borrow_mut() = SharedThemeSource::Fixed(Box::new(theme));
    }

    pub(super) fn set_reader(&self, reader: impl Fn() -> DefaultTheme + 'static) {
        *self.0.borrow_mut() = SharedThemeSource::Live(Box::new(reader));
    }

    pub(super) fn resolve(&self) -> DefaultTheme {
        match &*self.0.borrow() {
            SharedThemeSource::Fixed(theme) => **theme,
            SharedThemeSource::Live(reader) => reader(),
        }
    }

    /// A reader for child widgets that always resolves the current source.
    pub(super) fn reader(&self) -> impl Fn() -> DefaultTheme + 'static {
        let shared = self.clone();
        move || shared.resolve()
    }
}

#[derive(Debug, Clone)]
pub(super) struct DialogFocusState {
    pub(super) theme: DefaultTheme,
    pub(super) frame: Rect,
    pub(super) shown: bool,
    pub(super) animation: Progress,
}

impl DialogFocusState {
    pub(super) fn new() -> Self {
        Self {
            theme: DefaultTheme::default(),
            frame: Rect::ZERO,
            shown: false,
            animation: Progress::new(0.0),
        }
    }
}

pub(super) struct DialogFocusSurface {
    pub(super) state: Rc<RefCell<DialogFocusState>>,
}

impl DialogFocusSurface {
    pub(super) fn new(state: Rc<RefCell<DialogFocusState>>) -> Self {
        Self { state }
    }
}

impl Widget for DialogFocusSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.shown {
            state.frame.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.shown {
            return;
        }
        let progress = state.animation.get(ctx);
        if progress <= Progress::EPSILON {
            return;
        }
        let metrics = state.theme.metrics;
        draw_focus_ring_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius + 3.0,
            metrics,
            state
                .theme
                .palette
                .focus_ring
                .with_alpha(state.theme.palette.focus_ring.alpha * progress),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        let state = self.state.borrow();
        state.shown.then_some(StackSurfaceOptions {
            transient: true,
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }
}

pub struct Dialog {
    pub(super) theme: SharedTheme,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) shown: bool,
    pub(super) modal: bool,
    pub(super) dismiss_on_scrim: bool,
    pub(super) max_width: Option<f32>,
    pub(super) header_action: Option<SingleChild>,
    pub(super) body: SingleChild,
    pub(super) actions: WidgetChildren,
    pub(super) body_frame: Rect,
    pub(super) dialog_frame: Rect,
    pub(super) title_measurement: Option<TextMeasurement>,
    pub(super) description_measurement: Option<TextMeasurement>,
    pub(super) reveal: Reveal,
    pub(super) focus_animation: Progress,
    pub(super) focus_state: Rc<RefCell<DialogFocusState>>,
    pub(super) focus_surface: SingleChild,
    pub(super) entrance_started: bool,
    pub(super) on_dismiss: Option<Box<dyn FnMut()>>,
    pub(super) overlay_kind: OverlayKind,
    /// Laid out in flow at its content size; see [`Dialog::show_inline`].
    pub(super) inline: bool,
}

impl Dialog {
    pub fn new<W>(title: impl Into<String>, body: W) -> Self
    where
        W: Widget + 'static,
    {
        let focus_state = Rc::new(RefCell::new(DialogFocusState::new()));
        Self {
            theme: SharedTheme::new(),
            title: title.into(),
            description: None,
            shown: true,
            modal: true,
            dismiss_on_scrim: false,
            max_width: None,
            header_action: None,
            body: SingleChild::new(body),
            actions: WidgetChildren::new(),
            body_frame: Rect::ZERO,
            dialog_frame: Rect::ZERO,
            title_measurement: None,
            description_measurement: None,
            reveal: Reveal::new(0.0),
            focus_animation: Progress::new(0.0),
            focus_surface: SingleChild::new(DialogFocusSurface::new(Rc::clone(&focus_state))),
            focus_state,
            entrance_started: false,
            on_dismiss: None,
            overlay_kind: OverlayKind::Dialog,
            inline: false,
        }
    }

    /// Lays the dialog surface out in flow at its content size, without a
    /// scrim, entrance animation, or dismissal, instead of centering it in the
    /// window overlay stack. Widget galleries and documentation use this to
    /// show a dialog beside other content.
    pub fn show_inline(mut self) -> Self {
        self.inline = true;
        self.modal = false;
        self.shown = true;
        self.reveal = Reveal::new(1.0);
        self.entrance_started = true;
        self
    }

    pub fn theme(self, theme: DefaultTheme) -> Self {
        self.theme.set(theme);
        self
    }

    /// Resolve the theme on every layout and paint, including for the
    /// dialog's built-in action buttons.
    pub fn theme_when<F>(self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme.set_reader(theme);
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme.resolve()
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn shown(mut self, shown: bool) -> Self {
        self.set_shown(shown);
        self
    }

    pub fn set_shown(&mut self, shown: bool) -> bool {
        if self.shown == shown {
            return false;
        }
        self.shown = shown;
        if !shown {
            self.reveal = Reveal::new(0.0);
            self.focus_animation = Progress::new(0.0);
            self.entrance_started = false;
            let mut focus = self.focus_state.borrow_mut();
            focus.shown = false;
            focus.animation = Progress::new(0.0);
        }
        true
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn dismiss_on_scrim(mut self, dismiss_on_scrim: bool) -> Self {
        self.dismiss_on_scrim = dismiss_on_scrim;
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width.max(self.resolved_theme().metrics.dialog_min_width));
        self
    }

    pub fn on_dismiss<F>(mut self, on_dismiss: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_dismiss = Some(Box::new(on_dismiss));
        self
    }

    /// Place a compact control beside the title, such as a close button.
    pub fn header_action<W: Widget + 'static>(mut self, action: W) -> Self {
        self.header_action = Some(SingleChild::new(action));
        self
    }

    /// Add a footer control or a composed action row outside the scrolling body.
    pub fn action<W: Widget + 'static>(mut self, action: W) -> Self {
        self.actions.push(action);
        self
    }

    pub fn primary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.actions.push(
            Button::primary(label.into())
                .theme_when(self.theme.reader())
                .min_width(self.resolved_theme().metrics.dialog_action_min_width)
                .on_press(on_press),
        );
        self
    }

    pub fn secondary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.actions.push(
            Button::new(label.into())
                .theme_when(self.theme.reader())
                .min_width(self.resolved_theme().metrics.dialog_action_min_width)
                .on_press(on_press),
        );
        self
    }

    pub(super) fn resolved_max_width(&self) -> f32 {
        self.max_width
            .unwrap_or(self.resolved_theme().metrics.dialog_max_width)
    }

    pub(super) fn title_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        text_token_style(&theme, theme.text.lg, theme.palette.text)
    }

    pub(super) fn dismiss(&mut self) {
        if self.inline {
            return;
        }
        if let Some(on_dismiss) = &mut self.on_dismiss {
            on_dismiss();
        }
    }

    pub(super) fn ensure_entrance_started(&mut self, ctx: &mut MeasureCtx) {
        if self.entrance_started {
            return;
        }
        self.entrance_started = true;
        let motion = self.resolved_theme().motion;
        if let Some(until) = self
            .reveal
            .start(1.0, ctx.frame_time(), motion.entrance_spec())
        {
            ctx.track_motion(until, InvalidationKind::Effect);
            if !self.modal {
                ctx.track_motion(until, InvalidationKind::Transform);
            }
        }
    }
}

impl Widget for Dialog {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(OVERLAY_DISMISS_REQUEST).is_some() && self.shown {
            self.dismiss();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.shown {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self
                        .dialog_frame
                        .translate(ctx.bounds().origin.to_vector())
                        .contains(pointer.position) =>
            {
                ctx.request_focus();
                ctx.request_semantics();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && !self
                        .dialog_frame
                        .translate(ctx.bounds().origin.to_vector())
                        .contains(pointer.position) =>
            {
                if self.dismiss_on_scrim {
                    self.dismiss();
                }
                if self.modal || self.dismiss_on_scrim {
                    ctx.set_handled();
                }
                ctx.request_paint();
                ctx.request_semantics();
            }
            Event::Keyboard(key)
                if ctx.is_focused() && key.state == KeyState::Pressed && key.key == "Escape" =>
            {
                self.dismiss();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if !self.shown {
            self.dialog_frame = Rect::ZERO;
            self.body_frame = Rect::ZERO;
            self.reveal = Reveal::new(0.0);
            self.focus_animation = Progress::new(0.0);
            self.entrance_started = false;
            let mut focus = self.focus_state.borrow_mut();
            focus.shown = false;
            focus.frame = Rect::ZERO;
            focus.animation = Progress::new(0.0);
            return Size::ZERO;
        }
        self.ensure_entrance_started(ctx);

        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                640.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else if self.inline {
                f32::MAX / 4.0
            } else {
                420.0
            },
        ));
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let outer_margin = if self.inline {
            0.0
        } else {
            metrics.dialog_outer_margin
        };
        let padding = metrics.dialog_padding;
        let title_style = self.title_style();
        let description_style = theme.placeholder_text_style();
        self.title_measurement = Some(measure_text(ctx, &self.title, &title_style));
        self.description_measurement = self
            .description
            .as_ref()
            .map(|text| measure_text(ctx, text, &description_style));

        let available_width = (viewport.width - outer_margin * 2.0).max(0.0);
        let dialog_width = available_width
            .min(self.resolved_max_width())
            .max(metrics.dialog_min_width.min(available_width));
        let content_width = (dialog_width - padding.left - padding.right).max(0.0);
        let header_action_size = self.header_action.as_mut().map_or(Size::ZERO, |action| {
            action.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(content_width, metrics.touch_target_size),
                ),
            )
        });
        let mut footer_height: f32 = 0.0;
        for button in self.actions.as_mut_slice().iter_mut() {
            let button_size = button.measure(
                ctx,
                Constraints::new(Size::ZERO, Size::new(content_width, viewport.height)),
            );
            footer_height = footer_height.max(button_size.height);
        }

        let title_height = self
            .title_measurement
            .map(|measurement| measurement.height.max(title_style.line_height))
            .unwrap_or(title_style.line_height)
            .max(header_action_size.height);
        let description_height = self
            .description_measurement
            .map(|measurement| measurement.height.max(description_style.line_height))
            .unwrap_or(0.0);
        let header_gap = if self.description.is_some() {
            metrics.dialog_description_gap
        } else {
            0.0
        };
        let body_top =
            padding.top + title_height + header_gap + description_height + metrics.dialog_body_gap;
        let footer_gap = if self.actions.is_empty() {
            0.0
        } else {
            metrics.dialog_footer_gap
        };
        let body_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                (dialog_width - padding.left - padding.right).max(0.0),
                (viewport.height
                    - outer_margin * 2.0
                    - body_top
                    - footer_gap
                    - footer_height
                    - padding.bottom)
                    .max(0.0),
            ),
        );
        let body_size = self.body.measure(ctx, body_constraints);

        let dialog_height =
            body_top + body_size.height + footer_gap + footer_height + padding.bottom;
        let (dialog_x, dialog_y) = if self.inline {
            (0.0, 0.0)
        } else {
            (
                ((viewport.width - dialog_width) * 0.5).max(outer_margin),
                ((viewport.height - dialog_height) * 0.5).max(outer_margin),
            )
        };
        self.dialog_frame = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        self.body_frame = Rect::new(padding.left, body_top, body_size.width, body_size.height);
        {
            let mut focus = self.focus_state.borrow_mut();
            focus.theme = theme;
            focus.shown = true;
            focus.frame = self.dialog_frame;
            focus.animation = self.focus_animation;
        }
        self.focus_surface
            .measure(ctx, Constraints::tight(self.dialog_frame.size));

        if self.inline {
            constraints.clamp(self.dialog_frame.size)
        } else {
            viewport
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(bounds.origin.to_vector());
        self.focus_state.borrow_mut().frame = dialog;
        self.focus_surface.arrange(ctx, dialog);
        let title_line_height = self.title_style().line_height;
        let theme = self.resolved_theme();
        if let Some(action) = &mut self.header_action {
            let size = action.child().measured_size();
            let padding = theme.metrics.dialog_padding;
            let title_height = self
                .title_measurement
                .map(|measurement| measurement.height.max(title_line_height))
                .unwrap_or(title_line_height)
                .max(size.height);
            action.arrange(
                ctx,
                Rect::new(
                    dialog.max_x() - padding.right - size.width,
                    dialog.y() + padding.top + (title_height - size.height) * 0.5,
                    size.width,
                    size.height,
                ),
            );
        }
        self.body.arrange(
            ctx,
            Rect::new(
                dialog.x() + self.body_frame.x(),
                dialog.y() + self.body_frame.y(),
                self.body_frame.width(),
                self.body_frame.height(),
            ),
        );

        if !self.actions.is_empty() {
            let metrics = theme.metrics;
            let padding = metrics.dialog_padding;
            let action_gap = metrics.dialog_action_gap;
            let footer_width = self
                .actions
                .as_slice()
                .iter()
                .map(|button| button.measured_size().width)
                .sum::<f32>()
                + (action_gap * self.actions.len().saturating_sub(1) as f32);
            let footer_height = self
                .actions
                .as_slice()
                .iter()
                .map(|button| button.measured_size().height)
                .fold(0.0, f32::max);
            let mut x = dialog.x() + dialog.width() - padding.right - footer_width;
            let y = dialog.y() + dialog.height() - padding.bottom - footer_height;
            for button in self.actions.as_mut_slice().iter_mut() {
                let size = button.measured_size();
                button.arrange(ctx, Rect::new(x, y, size.width, size.height));
                x += size.width + action_gap;
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(ctx.bounds().origin.to_vector());
        let theme = self.resolved_theme();

        if self.modal {
            ctx.fill_bounds(theme.surfaces.overlay_scrim);
        }

        let metrics = theme.metrics;
        let palette = theme.palette;
        // Prominent elevation shadow behind the dialog surface, drawn over the
        // (optional) modal backdrop and before the surface fill.
        let surface_radius = metrics.corner_radius + 3.0;
        paint_theme_shadow(
            ctx,
            dialog,
            [surface_radius; 4],
            &theme.shadows.box_shadow.xl,
        );
        draw_control_frame(
            ctx,
            dialog,
            surface_radius,
            metrics,
            palette.surface_raised,
            palette.border,
            None,
        );

        let title_style = self.title_style();
        let description_style = theme.placeholder_text_style();
        let padding = metrics.dialog_padding;
        let text_x = dialog.x() + padding.left;
        let text_y = dialog.y() + padding.top;
        let header_action_size = self
            .header_action
            .as_ref()
            .map(|action| action.child().measured_size())
            .unwrap_or(Size::ZERO);
        let action_gap = if self.header_action.is_some() {
            header_action_size.width + metrics.dialog_action_gap
        } else {
            0.0
        };
        let text_width = (dialog.width() - padding.left - padding.right - action_gap).max(0.0);
        let title_height = self
            .title_measurement
            .map(|measurement| measurement.height.max(title_style.line_height))
            .unwrap_or(title_style.line_height)
            .max(header_action_size.height);
        let title_slot = Rect::new(text_x, text_y, text_width, title_height);
        ctx.push_clip_rect(title_slot);
        paint_text(ctx, title_slot, &self.title, &title_style, TextAlign::Start);
        ctx.pop_clip();
        if let Some(description) = &self.description {
            let description_height = self
                .description_measurement
                .map(|measurement| measurement.height.max(description_style.line_height))
                .unwrap_or(description_style.line_height);
            let description_slot = Rect::new(
                text_x,
                title_slot.max_y() + metrics.dialog_description_gap,
                (dialog.width() - padding.left - padding.right).max(0.0),
                description_height,
            );
            ctx.push_clip_rect(description_slot);
            paint_text(
                ctx,
                description_slot,
                description,
                &description_style,
                TextAlign::Start,
            );
            ctx.pop_clip();
        }

        if let Some(action) = &self.header_action {
            action.paint(ctx);
        }
        self.body.paint(ctx);
        for button in self.actions.as_slice() {
            button.paint(ctx);
        }
        self.focus_surface.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if self.shown && !self.inline {
                if self.modal {
                    LayerCompositionMode::Effect
                } else {
                    LayerCompositionMode::Overlay
                }
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties_at(&self, frame_time: f64) -> LayerProperties {
        let reveal = self.reveal.at(frame_time);
        let translation = if self.modal {
            Vector::ZERO
        } else {
            Vector::new(
                0.0,
                self.resolved_theme().metrics.popover_reveal_offset
                    * sui_runtime::motion_policy().entrance_offset(reveal),
            )
        };
        LayerProperties::new(reveal, translation)
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        (self.shown && !self.inline).then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (self.shown && !self.inline).then_some(
            OverlayOptions::new(self.overlay_kind)
                .modal(self.modal)
                .dismiss(OverlayDismissPolicy {
                    escape: true,
                    outside_pointer: self.dismiss_on_scrim,
                })
                .focus(OverlayFocusBehavior::CONTAINED),
        )
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if !self.shown {
            return;
        }

        let dialog = self.dialog_frame.translate(ctx.bounds().origin.to_vector());
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Dialog, dialog);
        node.name = Some(self.title.clone());
        node.description = self.description.clone();
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(self.shown);
        node.state.modal = self.modal;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Collapse];
        ctx.push(node);
        if let Some(action) = &self.header_action {
            action.semantics(ctx);
        }
        self.body.semantics(ctx);
        for button in self.actions.as_slice() {
            button.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let spec = self.resolved_theme().motion.focus_spec();
        let focus_surface_id = self.focus_surface.child().id();
        self.focus_animation
            .animate_for(focused as u8 as f32, spec, ctx, focus_surface_id);
        self.focus_state.borrow_mut().animation = self.focus_animation;
        request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.shown {
            if let Some(action) = &self.header_action {
                action.visit_children(visitor);
            }
            self.body.visit_children(visitor);
            self.actions.visit_children(visitor);
            self.focus_surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.shown {
            if let Some(action) = &mut self.header_action {
                action.visit_children_mut(visitor);
            }
            self.body.visit_children_mut(visitor);
            self.actions.visit_children_mut(visitor);
            self.focus_surface.visit_children_mut(visitor);
        }
    }
}

/// Modal presentation shell for application command search and execution.
///
/// Query state, ranking, and command execution remain application policies;
/// this shell supplies the shared overlay lifecycle and desktop interaction
/// behavior.
pub struct CommandPalette {
    pub(super) inner: Dialog,
}

impl CommandPalette {
    pub fn new<W>(name: impl Into<String>, content: W) -> Self
    where
        W: Widget + 'static,
    {
        let mut inner = Dialog::new(name, content).dismiss_on_scrim(true);
        inner.overlay_kind = OverlayKind::CommandPalette;
        Self { inner }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.inner = self.inner.theme(theme);
        self
    }

    pub fn shown(mut self, shown: bool) -> Self {
        self.inner = self.inner.shown(shown);
        self
    }

    pub fn set_shown(&mut self, shown: bool) -> bool {
        self.inner.set_shown(shown)
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.inner = self.inner.description(description);
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.inner = self.inner.max_width(max_width);
        self
    }

    pub fn on_dismiss<F>(mut self, on_dismiss: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.inner = self.inner.on_dismiss(on_dismiss);
        self
    }
}

impl Widget for CommandPalette {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        self.inner.command(ctx, command);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        self.inner.layer_options()
    }

    fn layer_properties(&self) -> LayerProperties {
        self.inner.layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.inner.stack_surface_options()
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        self.inner.overlay_options()
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.inner.visit_children_mut(visitor);
    }
}

pub type Modal = Dialog;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SideSheetPlacement {
    Left,
    #[default]
    Right,
    Bottom,
}

/// Cloneable presentation state shared by a sheet and application controls.
#[derive(Clone, Debug)]
pub struct SheetState {
    pub(super) shown: Signal<bool>,
}

impl SheetState {
    pub fn new(shown: bool) -> Self {
        Self {
            shown: Signal::named("SheetState", shown),
        }
    }

    pub fn is_shown(&self) -> bool {
        self.shown.get()
    }

    pub fn show(&self) -> bool {
        self.shown.set(true)
    }

    pub fn hide(&self) -> bool {
        self.shown.set(false)
    }

    pub fn toggle(&self) -> bool {
        self.shown.update(|shown| *shown = !*shown)
    }
}

impl Default for SheetState {
    fn default() -> Self {
        Self::new(false)
    }
}

/// An overlay panel anchored to a viewport edge.
///
/// `SideSheet` is suitable for responsive inspectors, conversation drawers,
/// and focused configuration flows. It shares SUI's dialog surface, spacing,
/// elevation, motion, focus, and semantic contracts while using a horizontal
/// reveal appropriate to a drawer.
pub struct SideSheet {
    pub(super) theme: SharedTheme,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) shown: bool,
    pub(super) state: Option<SheetState>,
    pub(super) modal: bool,
    pub(super) dismiss_on_scrim: bool,
    pub(super) placement: SideSheetPlacement,
    pub(super) width: Option<f32>,
    pub(super) height: Option<f32>,
    pub(super) body: SingleChild,
    pub(super) header_action: Option<SingleChild>,
    pub(super) actions: WidgetChildren,
    pub(super) sheet_frame: Rect,
    pub(super) body_frame: Rect,
    pub(super) header_action_frame: Rect,
    pub(super) title_measurement: Option<TextMeasurement>,
    pub(super) description_measurement: Option<TextMeasurement>,
    pub(super) reveal: Progress,
    pub(super) focus_animation: Progress,
    pub(super) entrance_started: bool,
    pub(super) focus_requested: bool,
    pub(super) previous_focus: Option<WidgetId>,
    pub(super) on_dismiss: Option<Box<dyn FnMut()>>,
}

impl SideSheet {
    pub fn new<W>(title: impl Into<String>, body: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme: SharedTheme::new(),
            title: title.into(),
            description: None,
            shown: true,
            state: None,
            modal: true,
            dismiss_on_scrim: true,
            placement: SideSheetPlacement::Right,
            width: None,
            height: None,
            body: SingleChild::new(body),
            header_action: None,
            actions: WidgetChildren::new(),
            sheet_frame: Rect::ZERO,
            body_frame: Rect::ZERO,
            header_action_frame: Rect::ZERO,
            title_measurement: None,
            description_measurement: None,
            reveal: Progress::new(0.0),
            focus_animation: Progress::new(0.0),
            entrance_started: false,
            focus_requested: false,
            previous_focus: None,
            on_dismiss: None,
        }
    }

    pub fn theme(self, theme: DefaultTheme) -> Self {
        self.theme.set(theme);
        self
    }

    pub fn theme_when<F>(self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme.set_reader(theme);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn shown(mut self, shown: bool) -> Self {
        self.set_shown(shown);
        self
    }

    pub fn is_shown(&self) -> bool {
        self.shown
    }

    pub fn set_shown(&mut self, shown: bool) -> bool {
        self.state = None;
        if self.shown == shown {
            return false;
        }
        self.shown = shown;
        if !self.shown {
            self.reveal = Progress::new(0.0);
            self.focus_animation = Progress::new(0.0);
            self.entrance_started = false;
            self.focus_requested = false;
            self.previous_focus = None;
        }
        true
    }

    pub fn state(mut self, state: SheetState) -> Self {
        self.shown = state.is_shown();
        self.state = Some(state);
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn dismiss_on_scrim(mut self, dismiss_on_scrim: bool) -> Self {
        self.dismiss_on_scrim = dismiss_on_scrim;
        self
    }

    pub fn placement(mut self, placement: SideSheetPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(0.0));
        self
    }

    /// Set the panel height when placed at [`SideSheetPlacement::Bottom`].
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(0.0));
        self
    }

    pub fn header_action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.header_action = Some(SingleChild::new(action));
        self
    }

    pub fn action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.actions.push(action);
        self
    }

    pub fn primary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        let theme = self.resolved_theme();
        self.actions.push(
            Button::primary(label)
                .theme_when(self.theme.reader())
                .min_width(theme.metrics.dialog_action_min_width)
                .on_press(on_press),
        );
        self
    }

    pub fn secondary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        let theme = self.resolved_theme();
        self.actions.push(
            Button::new(label)
                .theme_when(self.theme.reader())
                .appearance(ButtonAppearance::Outline)
                .tone(SemanticTone::Neutral)
                .min_width(theme.metrics.dialog_action_min_width)
                .on_press(on_press),
        );
        self
    }

    pub fn on_dismiss<F>(mut self, on_dismiss: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_dismiss = Some(Box::new(on_dismiss));
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme.resolve()
    }

    pub(super) fn resolved_width(&self, viewport_width: f32, theme: &DefaultTheme) -> f32 {
        self.width
            .unwrap_or(theme.metrics.dialog_max_width.min(420.0))
            .max(theme.metrics.dialog_min_width.min(viewport_width))
            .min(viewport_width)
            .max(0.0)
    }

    pub(super) fn resolved_height(&self, viewport_height: f32, theme: &DefaultTheme) -> f32 {
        self.height
            .unwrap_or((viewport_height * 0.62).min(560.0))
            .max(theme.metrics.touch_target_size * 3.0)
            .min(viewport_height)
            .max(0.0)
    }

    pub(super) fn title_style(theme: &DefaultTheme) -> TextStyle {
        text_token_style(theme, theme.text.lg, theme.palette.text)
    }

    pub(super) fn dismiss(&mut self, ctx: &mut EventCtx) {
        if let Some(state) = &self.state {
            state.hide();
            self.shown = false;
            ctx.request_measure();
            ctx.request_paint();
        }
        if let Some(on_dismiss) = &mut self.on_dismiss {
            on_dismiss();
        }
        if let Some(previous_focus) = self.previous_focus.take() {
            ctx.request_focus_for(previous_focus);
        }
    }

    pub(super) fn contains_widget(&self, target: WidgetId) -> bool {
        struct Finder {
            target: WidgetId,
            found: bool,
        }

        impl WidgetPodVisitor for Finder {
            fn visit(&mut self, child: &WidgetPod) {
                if self.found {
                    return;
                }
                if child.id() == self.target {
                    self.found = true;
                } else {
                    child.visit_children(self);
                }
            }
        }

        fn inspect(pod: &WidgetPod, finder: &mut Finder) {
            if finder.found {
                return;
            }
            if pod.id() == finder.target {
                finder.found = true;
            } else {
                pod.visit_children(finder);
            }
        }

        let mut finder = Finder {
            target,
            found: false,
        };
        inspect(self.body.child(), &mut finder);
        if let Some(action) = &self.header_action {
            inspect(action.child(), &mut finder);
        }
        for action in self.actions.as_slice() {
            inspect(action, &mut finder);
        }
        finder.found
    }

    pub(super) fn ensure_entrance_started(&mut self, ctx: &mut MeasureCtx) {
        if self.entrance_started {
            return;
        }
        self.entrance_started = true;
        let motion = self.resolved_theme().motion;
        self.reveal.animate(
            1.0,
            AnimationSpec::tween(f64::from(motion.duration_slower), motion.easing_decelerate),
            ctx,
        );
        if !self.focus_requested {
            // Focus moves into the sheet on the next frame, once it is laid
            // out.
            ctx.request_animation_frame();
        }
    }

    pub(super) fn reveal_offset(&self, clock: &impl FrameClock) -> Vector {
        // Reduced motion shows the sheet in place instead of sliding it in.
        let hidden = sui_runtime::motion_policy().entrance_offset(self.reveal.get(clock));
        let horizontal_distance = self.sheet_frame.width() * hidden;
        let vertical_distance = self.sheet_frame.height() * hidden;
        match self.placement {
            SideSheetPlacement::Left => Vector::new(-horizontal_distance, 0.0),
            SideSheetPlacement::Right => Vector::new(horizontal_distance, 0.0),
            SideSheetPlacement::Bottom => Vector::new(0.0, vertical_distance),
        }
    }

    pub(super) fn presented_sheet(&self, origin: Point, clock: &impl FrameClock) -> Rect {
        self.sheet_frame
            .translate(origin.to_vector() + self.reveal_offset(clock))
    }
}

impl Widget for SideSheet {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(OVERLAY_DISMISS_REQUEST).is_some() && self.is_shown() {
            self.dismiss(ctx);
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Some(state) = &self.state {
            self.shown = state.is_shown();
        }
        if !self.shown {
            return;
        }
        match event {
            Event::Wake(WakeEvent::AnimationFrame { .. }) if !self.focus_requested => {
                self.focus_requested = true;
                let current_focus = ctx.focused_widget_id();
                let focus_is_inside = current_focus.is_some_and(|focused| {
                    focused == ctx.widget_id() || self.contains_widget(focused)
                });
                if !focus_is_inside {
                    self.previous_focus = current_focus;
                    ctx.request_focus();
                }
                ctx.set_handled();
            }
            Event::Semantics(semantics)
                if semantics.target == ctx.widget_id()
                    && matches!(semantics.action, sui_core::SemanticsActionRequest::Collapse) =>
            {
                self.dismiss(ctx);
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if self
                    .presented_sheet(ctx.bounds().origin, ctx)
                    .contains(pointer.position)
                {
                    ctx.request_focus();
                    ctx.request_semantics();
                } else {
                    if self.dismiss_on_scrim {
                        self.dismiss(ctx);
                    }
                    if self.modal || self.dismiss_on_scrim {
                        ctx.set_handled();
                    }
                    ctx.request_semantics();
                }
            }
            Event::Keyboard(key) if key.state == KeyState::Pressed && key.key == "Escape" => {
                self.dismiss(ctx);
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(state) = &self.state {
            self.shown = ctx.observe(&state.shown);
        }
        if !self.shown {
            self.sheet_frame = Rect::ZERO;
            self.body_frame = Rect::ZERO;
            self.header_action_frame = Rect::ZERO;
            self.reveal = Progress::new(0.0);
            self.focus_animation = Progress::new(0.0);
            self.entrance_started = false;
            self.focus_requested = false;
            self.previous_focus = None;
            return Size::ZERO;
        }
        self.ensure_entrance_started(ctx);

        let viewport = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                960.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                640.0
            },
        ));
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let padding = metrics.dialog_padding;
        let (sheet_x, sheet_y, sheet_width, sheet_height) = match self.placement {
            SideSheetPlacement::Left => (
                0.0,
                0.0,
                self.resolved_width(viewport.width, &theme),
                viewport.height,
            ),
            SideSheetPlacement::Right => {
                let width = self.resolved_width(viewport.width, &theme);
                (viewport.width - width, 0.0, width, viewport.height)
            }
            SideSheetPlacement::Bottom => {
                let height = self.resolved_height(viewport.height, &theme);
                (0.0, viewport.height - height, viewport.width, height)
            }
        };
        self.sheet_frame = Rect::new(sheet_x, sheet_y, sheet_width, sheet_height);

        let title_style = Self::title_style(&theme);
        let description_style = theme.placeholder_text_style();
        self.title_measurement = Some(measure_text(ctx, &self.title, &title_style));
        self.description_measurement = self
            .description
            .as_ref()
            .map(|description| measure_text(ctx, description, &description_style));

        let header_action_size = if let Some(action) = &mut self.header_action {
            action.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(
                        (sheet_width - padding.left - padding.right).max(0.0),
                        metrics.touch_target_size,
                    ),
                ),
            )
        } else {
            Size::ZERO
        };
        let title_height = self
            .title_measurement
            .map(|measurement| measurement.height.max(title_style.line_height))
            .unwrap_or(title_style.line_height)
            .max(header_action_size.height);
        let description_height = self
            .description_measurement
            .map(|measurement| measurement.height.max(description_style.line_height))
            .unwrap_or(0.0);
        let description_gap = if self.description.is_some() {
            metrics.dialog_description_gap
        } else {
            0.0
        };
        let body_top = padding.top
            + title_height
            + description_gap
            + description_height
            + metrics.dialog_body_gap;

        let mut footer_height: f32 = 0.0;
        for action in self.actions.as_mut_slice() {
            footer_height = footer_height.max(
                action
                    .measure(
                        ctx,
                        Constraints::new(
                            Size::ZERO,
                            Size::new(sheet_width, metrics.touch_target_size),
                        ),
                    )
                    .height,
            );
        }
        let footer_gap = if self.actions.is_empty() {
            0.0
        } else {
            metrics.dialog_footer_gap
        };
        let body_height =
            (sheet_height - body_top - footer_gap - footer_height - padding.bottom).max(0.0);
        let body_width = (sheet_width - padding.left - padding.right).max(0.0);
        let _ = self.body.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(body_width, body_height)),
        );
        self.body_frame = Rect::new(padding.left, body_top, body_width, body_height);
        self.header_action_frame = Rect::new(
            sheet_width - padding.right - header_action_size.width,
            padding.top,
            header_action_size.width,
            header_action_size.height,
        );
        viewport
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if !self.shown {
            return;
        }
        let sheet = self.sheet_frame.translate(bounds.origin.to_vector());
        self.body
            .arrange(ctx, self.body_frame.translate(sheet.origin.to_vector()));
        if let Some(action) = &mut self.header_action {
            action.arrange(
                ctx,
                self.header_action_frame.translate(sheet.origin.to_vector()),
            );
        }
        if !self.actions.is_empty() {
            let theme = self.resolved_theme();
            let metrics = theme.metrics;
            let padding = metrics.dialog_padding;
            let gap = metrics.dialog_action_gap;
            let total_width = self
                .actions
                .as_slice()
                .iter()
                .map(|action| action.measured_size().width)
                .sum::<f32>()
                + gap * self.actions.len().saturating_sub(1) as f32;
            let footer_height = self
                .actions
                .as_slice()
                .iter()
                .map(|action| action.measured_size().height)
                .fold(0.0, f32::max);
            let mut x = sheet.max_x() - padding.right - total_width;
            let y = sheet.max_y() - padding.bottom - footer_height;
            for action in self.actions.as_mut_slice() {
                let size = action.measured_size();
                action.arrange(ctx, Rect::new(x, y, size.width, size.height));
                x += size.width + gap;
            }
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.shown {
            return;
        }
        let theme = self.resolved_theme();
        if self.modal {
            let scrim = theme
                .surfaces
                .overlay_scrim
                .with_alpha(theme.surfaces.overlay_scrim.alpha * self.reveal.get(ctx));
            ctx.fill_bounds(scrim);
        }

        let reveal_offset = self.reveal_offset(ctx);
        ctx.push_transform(Transform::translation(reveal_offset.x, reveal_offset.y));
        let sheet = self.sheet_frame.translate(ctx.bounds().origin.to_vector());
        let metrics = theme.metrics;
        paint_theme_shadow(ctx, sheet, [0.0; 4], &theme.shadows.box_shadow.xl);
        ctx.fill_rect(sheet, theme.palette.surface_raised);
        let border_width = physical_pixels(ctx, metrics.border_width.max(1.0));
        let border = match self.placement {
            SideSheetPlacement::Left => Rect::new(
                sheet.max_x() - border_width,
                sheet.y(),
                border_width,
                sheet.height(),
            ),
            SideSheetPlacement::Right => {
                Rect::new(sheet.x(), sheet.y(), border_width, sheet.height())
            }
            SideSheetPlacement::Bottom => {
                Rect::new(sheet.x(), sheet.y(), sheet.width(), border_width)
            }
        };
        ctx.fill_rect(border, theme.palette.border);
        let focus = self.focus_animation.get(ctx);
        if focus > Progress::EPSILON {
            let inset = physical_pixels(ctx, theme.metrics.focus_ring_width) * 0.5;
            ctx.stroke(
                rounded_rect_path(sheet.inflate(-inset, -inset), 0.0),
                theme
                    .palette
                    .focus_ring
                    .with_alpha(theme.palette.focus_ring.alpha * focus),
                StrokeStyle::new(physical_pixels(ctx, theme.metrics.focus_ring_width)),
            );
        }

        let padding = metrics.dialog_padding;
        let title_style = Self::title_style(&theme);
        let action_width = if self.header_action.is_some() {
            self.header_action_frame.width() + metrics.dialog_action_gap
        } else {
            0.0
        };
        let title_height = self
            .title_measurement
            .map(|measurement| measurement.height.max(title_style.line_height))
            .unwrap_or(title_style.line_height)
            .max(self.header_action_frame.height());
        let title_slot = Rect::new(
            sheet.x() + padding.left,
            sheet.y() + padding.top,
            (sheet.width() - padding.left - padding.right - action_width).max(0.0),
            title_height,
        );
        paint_text(ctx, title_slot, &self.title, &title_style, TextAlign::Start);
        if let Some(description) = &self.description {
            let style = theme.placeholder_text_style();
            let height = self
                .description_measurement
                .map(|measurement| measurement.height.max(style.line_height))
                .unwrap_or(style.line_height);
            let slot = Rect::new(
                sheet.x() + padding.left,
                title_slot.max_y() + metrics.dialog_description_gap,
                (sheet.width() - padding.left - padding.right).max(0.0),
                height,
            );
            paint_text(ctx, slot, description, &style, TextAlign::Start);
        }
        self.body.paint(ctx);
        if let Some(action) = &self.header_action {
            action.paint(ctx);
        }
        self.actions.paint(ctx);
        ctx.pop_transform();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if self.shown {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.shown.then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        self.is_shown().then_some(
            OverlayOptions::new(OverlayKind::Sheet)
                .modal(self.modal)
                .dismiss(OverlayDismissPolicy {
                    escape: true,
                    outside_pointer: self.dismiss_on_scrim,
                })
                .focus(OverlayFocusBehavior::CONTAINED),
        )
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if !self.shown {
            return;
        }
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Dialog,
            self.sheet_frame.translate(ctx.bounds().origin.to_vector()),
        );
        node.name = Some(self.title.clone());
        node.description = self.description.clone();
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(true);
        node.state.modal = self.modal;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Collapse];
        ctx.push(node);
        self.body.semantics(ctx);
        if let Some(action) = &self.header_action {
            action.semantics(ctx);
        }
        self.actions.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.shown
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.shown {
            self.body.visit_children(visitor);
            if let Some(action) = &self.header_action {
                action.visit_children(visitor);
            }
            self.actions.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.shown {
            self.body.visit_children_mut(visitor);
            if let Some(action) = &mut self.header_action {
                action.visit_children_mut(visitor);
            }
            self.actions.visit_children_mut(visitor);
        }
    }
}

/// A familiar alias for navigation-oriented side sheets.
pub type Drawer = SideSheet;

/// A modal surface anchored to the bottom edge of its allocated viewport.
///
/// `BottomSheet` follows the same focus, dismissal, semantics, and action
/// contracts as [`SideSheet`], while exposing height-oriented configuration.
pub struct BottomSheet {
    pub(super) inner: SideSheet,
}

impl BottomSheet {
    pub fn new<W>(title: impl Into<String>, body: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            inner: SideSheet::new(title, body).placement(SideSheetPlacement::Bottom),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.inner = self.inner.theme(theme);
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.inner = self.inner.theme_when(theme);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.inner = self.inner.description(description);
        self
    }

    pub fn shown(mut self, shown: bool) -> Self {
        self.inner = self.inner.shown(shown);
        self
    }

    pub fn state(mut self, state: SheetState) -> Self {
        self.inner = self.inner.state(state);
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.inner = self.inner.modal(modal);
        self
    }

    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.inner = self.inner.dismiss_on_scrim(dismiss);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn header_action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.inner = self.inner.header_action(action);
        self
    }

    pub fn action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.inner = self.inner.action(action);
        self
    }

    pub fn primary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.inner = self.inner.primary_action(label, on_press);
        self
    }

    pub fn secondary_action<F>(mut self, label: impl Into<String>, on_press: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.inner = self.inner.secondary_action(label, on_press);
        self
    }

    pub fn on_dismiss<F>(mut self, on_dismiss: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.inner = self.inner.on_dismiss(on_dismiss);
        self
    }
}

impl Widget for BottomSheet {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        self.inner.command(ctx, command);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.inner.event(ctx, event);
    }

    fn debug_name(&self) -> &'static str {
        "sui_widgets::BottomSheet"
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.inner.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.inner.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.inner.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        self.inner.layer_options()
    }

    fn layer_properties(&self) -> LayerProperties {
        self.inner.layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.inner.stack_surface_options()
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        self.inner.overlay_options()
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.inner.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.inner.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.inner.focus_changed(ctx, focused);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.inner.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.inner.visit_children_mut(visitor);
    }
}
