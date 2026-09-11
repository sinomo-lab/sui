use crate::DefaultTheme;
use crate::HdrThemeMode;
use crate::MotionScalar;
use crate::ResolvedEffectStyle;
use crate::ResolvedHdrStyle;
use crate::WidgetColorRole;
use crate::WidgetEffectRole;
use crate::WidgetLuminanceRole;
use crate::WidgetMaterialRole;
use crate::composites::forms::{
    set_focus_animation_target, set_hover_animation_target, set_press_animation_target,
};
use crate::composites::indicators::{
    draw_control_frame, draw_focus_ring_frame, draw_popover_arrival_overlay, inset_rect,
    measure_text, mix_color, rounded_rect_path, text_token_style, tooltip_tail,
};
use crate::composites::toolbars::{
    MenuItem, context_menu_item_semantics_node, menu_item_semantics_node, menu_row_height,
    menu_submenu_indicator_width, themed_menu_height_for_rows, virtual_menu_item_path_id,
};
use crate::controls::cap_resolved_hdr_style;
use crate::overlay::OverlayAlignment;
use crate::overlay::OverlayPlacement;
use crate::overlay::OverlayPlacementRequest;
use crate::overlay::OverlaySide;
use crate::overlay::place_overlay;
use crate::paint_theme_shadow;
use crate::resolve_widget_hdr_style;
use crate::text_align::paint_aligned_text;
use std::cell::RefCell;
use std::rc::Rc;
use sui_core::Color;
use sui_core::Event;
use sui_core::InvalidationKind;
use sui_core::InvalidationRequest;
use sui_core::InvalidationTarget;
use sui_core::KeyState;
use sui_core::Point;
use sui_core::PointerButton;
use sui_core::PointerEventKind;
use sui_core::Rect;
use sui_core::SemanticsAction;
use sui_core::SemanticsNode;
use sui_core::SemanticsPopupKind;
use sui_core::SemanticsRole;
use sui_core::SemanticsState;
use sui_core::SemanticsValue;
use sui_core::Size;
use sui_core::TimerToken;
use sui_core::Vector;
use sui_core::WakeEvent;
use sui_core::WidgetId;
use sui_layout::Constraints;
use sui_runtime::ArrangeCtx;
use sui_runtime::Command;
use sui_runtime::EventCtx;
use sui_runtime::EventPhase;
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
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::LayerCompositionMode;
use sui_scene::LayerProperties;
use sui_text::TextMeasurement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPlacement {
    Above,
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipAlignment {
    Start,
    Center,
    End,
}

pub struct Menu {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) items: Vec<MenuItem>,
    pub(super) highlighted: Option<usize>,
    pub(super) highlight_visual: Option<usize>,
    pub(super) pressed: Option<usize>,
    pub(super) press_visual: Option<usize>,
    pub(super) highlight_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) measured_width: f32,
    pub(super) focus_on_pointer_down: bool,
    pub(super) on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
    pub(super) on_activate_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, MenuItem)>>,
}

impl Menu {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            items: Vec::new(),
            highlighted: None,
            highlight_visual: None,
            pressed: None,
            press_visual: None,
            highlight_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            measured_width: 220.0,
            focus_on_pointer_down: true,
            on_activate: None,
            on_activate_with_ctx: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.items.extend(items);
        self
    }

    pub fn highlighted(mut self, index: usize) -> Self {
        self.highlighted = Some(index);
        self.highlight_visual = Some(index);
        self.highlight_animation = AnimatedScalar::new(1.0);
        self
    }

    pub fn on_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, MenuItem) + 'static,
    {
        self.on_activate = Some(Box::new(on_activate));
        self
    }

    pub fn on_activate_with_ctx<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, MenuItem) + 'static,
    {
        self.on_activate_with_ctx = Some(Box::new(on_activate));
        self
    }

    pub fn focus_on_pointer_down(mut self, focus_on_pointer_down: bool) -> Self {
        self.focus_on_pointer_down = focus_on_pointer_down;
        self
    }

    pub(super) fn row_height(&self) -> f32 {
        let theme = self.resolved_theme();
        menu_row_height(&theme)
    }

    pub(super) fn activate(&mut self, ctx: &mut EventCtx, index: usize) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if !item.enabled {
            return;
        }
        match (&mut self.on_activate, &mut self.on_activate_with_ctx) {
            (Some(on_activate), _) => on_activate(index, item),
            (None, Some(on_activate)) => on_activate(ctx, index, item),
            (None, None) => {}
        }
    }

    pub(super) fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let theme = self.resolved_theme();
        let padding = theme.metrics.menu_padding;
        let x = bounds.x() + padding.left;
        let y = bounds.y() + padding.top + (index as f32 * self.row_height());
        Some(Rect::new(
            x,
            y,
            (bounds.width() - padding.left - padding.right).max(0.0),
            self.row_height(),
        ))
    }

    pub(super) fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.items.iter().enumerate().find_map(|(index, _)| {
            self.item_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    pub(super) fn move_highlight(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.items.is_empty() {
            return;
        }

        let len = self.items.len() as isize;
        let start = self.highlighted.unwrap_or(0) as isize;
        let mut index = (start + delta).clamp(0, len - 1);
        while !self.items[index as usize].enabled {
            let next = (index + delta).clamp(0, len - 1);
            if next == index {
                break;
            }
            index = next;
        }
        self.set_highlighted(Some(index as usize), ctx);
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn set_highlighted(&mut self, highlighted: Option<usize>, ctx: &mut EventCtx) {
        if self.highlighted == highlighted {
            return;
        }
        let theme = self.resolved_theme();
        self.highlighted = highlighted;
        if let Some(index) = highlighted {
            self.highlight_visual = Some(index);
            self.highlight_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.highlight_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.highlight_animation, 0.0, &theme, ctx) {
            self.highlight_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn set_pressed(&mut self, pressed: Option<usize>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(index) = pressed {
            self.press_visual = Some(index);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn highlight_amount_for(&self, index: usize) -> f32 {
        if self.highlight_visual == Some(index) {
            self.highlight_animation.value
        } else {
            0.0
        }
    }

    pub(super) fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual == Some(index) {
            self.press_animation.value
        } else {
            0.0
        }
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
        let highlight_animating = self.highlight_animation.advance(time);
        if !highlight_animating
            && self.highlighted.is_none()
            && self.highlight_animation.value <= AnimatedScalar::EPSILON
        {
            self.highlight_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        highlight_animating | press_animating | self.focus_animation.advance(time)
    }
}

impl Widget for Menu {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_highlighted(self.item_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                self.set_highlighted(highlighted, ctx);
                self.set_pressed(
                    highlighted
                        .filter(|index| self.items.get(*index).is_some_and(|item| item.enabled)),
                    ctx,
                );
                if self.focus_on_pointer_down {
                    ctx.request_focus();
                }
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(highlighted)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(ctx, index);
                }
                self.set_highlighted(highlighted, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowDown" => self.move_highlight(1, ctx),
                    "ArrowUp" => self.move_highlight(-1, ctx),
                    "Home" => {
                        self.set_highlighted(self.items.iter().position(|item| item.enabled), ctx);
                    }
                    "End" => {
                        self.set_highlighted(self.items.iter().rposition(|item| item.enabled), ctx);
                    }
                    "Enter" | " " => {
                        if let Some(index) = self.highlighted {
                            self.activate(ctx, index);
                        }
                    }
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                if self.advance_animations(*time) {
                    ctx.request_animation_frame();
                }
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let label_style = theme.body_text_style();
        let shortcut_style = theme.placeholder_text_style();
        let mut width: f32 = 0.0;
        for item in &self.items {
            let label = measure_text(ctx, item.label(), &label_style).width;
            let shortcut = item
                .shortcut
                .as_ref()
                .map(|text| measure_text(ctx, text, &shortcut_style).width)
                .unwrap_or(0.0);
            width = width.max(
                label
                    + shortcut
                    + theme.metrics.menu_item_padding.left
                    + theme.metrics.menu_item_padding.right
                    + theme.metrics.menu_shortcut_width,
            );
        }
        self.measured_width = width.max(220.0);
        let height = themed_menu_height_for_rows(&theme, self.row_height(), self.items.len());
        constraints.clamp(Size::new(
            self.measured_width,
            height.max(themed_menu_height_for_rows(&theme, self.row_height(), 1)),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let item_padding = metrics.menu_item_padding;

        // Cast an elevation shadow behind the raised menu surface before any
        // fill so the soft drop shadow is not clipped by the frame.
        let surface_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(
            ctx,
            ctx.bounds(),
            [surface_radius; 4],
            &theme.shadows.box_shadow.lg,
        );

        draw_control_frame(
            ctx,
            ctx.bounds(),
            surface_radius,
            metrics,
            palette.surface_raised,
            palette.border,
            (self.focus_animation.value > AnimatedScalar::EPSILON).then_some(
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
            ),
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };

            if item.separator_before {
                let line = Rect::new(
                    row.x(),
                    row.y() - (metrics.menu_padding.top * 0.5),
                    row.width(),
                    1.0,
                );
                ctx.fill(rounded_rect_path(line, 0.5), palette.border);
            }

            let highlighted = self.highlighted == Some(index);
            let highlight_amount = self.highlight_amount_for(index);
            let press_amount = self.press_amount_for(index);
            let label_style = theme.text_style(item.text_color(&theme));
            let label_slot = Rect::new(
                row.x() + item_padding.left,
                row.y(),
                (row.width()
                    - item_padding.left
                    - item_padding.right
                    - item
                        .shortcut
                        .as_ref()
                        .map(|_| metrics.menu_shortcut_width)
                        .unwrap_or(0.0))
                .max(0.0),
                row.height(),
            );
            if highlighted || highlight_amount > 0.0 || press_amount > 0.0 {
                let highlight_background =
                    mix_color(palette.control, palette.selection, highlight_amount);
                let background = if press_amount > 0.0 {
                    mix_color(
                        highlight_background,
                        palette.control_active,
                        interaction.pressed_blend * press_amount,
                    )
                } else {
                    highlight_background
                };
                ctx.fill(
                    rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                    background,
                );
            }

            ctx.push_clip_rect(label_slot);
            paint_aligned_text(
                ctx,
                label_slot,
                &item.label,
                &label_style,
                label_style.line_height,
                0.0,
            );
            ctx.pop_clip();

            if let Some(shortcut) = &item.shortcut {
                let shortcut_style = theme.placeholder_text_style();
                let shortcut_slot = Rect::new(
                    row.max_x() - item_padding.right - metrics.menu_shortcut_width,
                    row.y(),
                    metrics.menu_shortcut_width,
                    row.height(),
                );
                ctx.push_clip_rect(shortcut_slot);
                paint_aligned_text(
                    ctx,
                    shortcut_slot,
                    shortcut,
                    &shortcut_style,
                    shortcut_style.line_height,
                    1.0,
                );
                ctx.pop_clip();
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let state = SemanticsState {
            focused: ctx.is_focused(),
            ..SemanticsState::default()
        };
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Menu, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state = state;
        node.value = self
            .highlighted
            .and_then(|index| self.items.get(index))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::SetValue,
            SemanticsAction::Activate,
        ];
        ctx.push(node);
        for (index, item) in self.items.iter().enumerate() {
            let Some(row) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            ctx.push(menu_item_semantics_node(
                ctx.widget_id(),
                index,
                item,
                row,
                self.highlighted == Some(index),
            ));
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub(super) type AnimatedScalar = MotionScalar;

pub(super) fn request_child_invalidation(
    ctx: &mut EventCtx,
    widget_id: WidgetId,
    kind: InvalidationKind,
) {
    ctx.request(InvalidationRequest::new(
        InvalidationTarget::Widget(widget_id),
        kind,
    ));
}

pub(super) fn tooltip_fallback_measurement(theme: &DefaultTheme) -> TextMeasurement {
    TextMeasurement {
        width: 120.0,
        height: theme.typography.body_line_height,
        bounds: Rect::new(0.0, 0.0, 120.0, theme.typography.body_line_height),
        ascent: theme.typography.body_font_size,
        descent: 0.0,
        cap_height: Some(theme.typography.body_font_size),
    }
}

pub(super) fn tooltip_bubble_rect(
    trigger_bounds: Rect,
    measurement: Option<TextMeasurement>,
    theme: &DefaultTheme,
    placement: TooltipPlacement,
    alignment: TooltipAlignment,
    viewport: Rect,
) -> (Rect, TooltipPlacement) {
    let measurement = measurement.unwrap_or_else(|| tooltip_fallback_measurement(theme));
    let padding = theme.metrics.tooltip_padding;
    let width =
        (measurement.width + padding.left + padding.right).max(theme.metrics.tooltip_min_width);
    let height =
        measurement.height.max(theme.typography.body_line_height) + padding.top + padding.bottom;
    let side = match placement {
        TooltipPlacement::Above => OverlaySide::Top,
        TooltipPlacement::Below => OverlaySide::Bottom,
    };
    let alignment = match alignment {
        TooltipAlignment::Start => OverlayAlignment::Start,
        TooltipAlignment::Center => OverlayAlignment::Center,
        TooltipAlignment::End => OverlayAlignment::End,
    };
    let result = place_overlay(
        &OverlayPlacementRequest::new(
            trigger_bounds,
            Size::new(width, height),
            viewport,
            OverlayPlacement::new(side, alignment),
        )
        .gap(theme.metrics.tooltip_gap)
        .margin(theme.metrics.tooltip_gap.max(4.0)),
    );
    let resolved = if result.placement.side == OverlaySide::Top {
        TooltipPlacement::Above
    } else {
        TooltipPlacement::Below
    };
    (result.bounds, resolved)
}

#[derive(Debug, Clone)]
pub(super) struct TooltipPresentationState {
    pub(super) theme: DefaultTheme,
    pub(super) text: String,
    pub(super) placement: TooltipPlacement,
    pub(super) resolved_placement: TooltipPlacement,
    pub(super) alignment: TooltipAlignment,
    pub(super) measurement: Option<TextMeasurement>,
    pub(super) hovered: bool,
    pub(super) trigger_bounds: Rect,
    pub(super) bubble_bounds: Rect,
    pub(super) reveal: AnimatedScalar,
}

impl TooltipPresentationState {
    pub(super) fn new(text: String) -> Self {
        Self {
            theme: DefaultTheme::default(),
            text,
            placement: TooltipPlacement::Above,
            resolved_placement: TooltipPlacement::Above,
            alignment: TooltipAlignment::Center,
            measurement: None,
            hovered: false,
            trigger_bounds: Rect::ZERO,
            bubble_bounds: Rect::ZERO,
            reveal: AnimatedScalar::new(0.0),
        }
    }

    pub(super) fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    pub(super) fn layer_properties(&self) -> LayerProperties {
        let direction = match self.resolved_placement {
            TooltipPlacement::Above => -1.0,
            TooltipPlacement::Below => 1.0,
        };
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(
                0.0,
                self.theme.metrics.tooltip_reveal_offset * (1.0 - self.reveal.value) * direction,
            ),
        }
    }
}

pub(super) struct TooltipOverlay {
    pub(super) state: Rc<RefCell<TooltipPresentationState>>,
}

impl TooltipOverlay {
    pub(super) fn new(state: Rc<RefCell<TooltipPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for TooltipOverlay {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if !state.is_presented() {
            return Size::ZERO;
        }
        state.bubble_bounds.size
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, bounds: Rect) {
        self.state.borrow_mut().bubble_bounds = bounds;
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let bubble = ctx.bounds();
        let metrics = state.theme.metrics;
        // Soft elevation behind the tooltip bubble, drawn before the fill.
        paint_theme_shadow(
            ctx,
            bubble,
            [metrics.corner_radius; 4],
            &state.theme.shadows.box_shadow.sm,
        );
        draw_control_frame(
            ctx,
            bubble,
            metrics.corner_radius,
            metrics,
            state.theme.surfaces.tooltip,
            state.theme.surfaces.tooltip_border,
            None,
        );
        let tail = tooltip_tail(state.trigger_bounds, bubble, state.resolved_placement);
        ctx.fill(tail, state.theme.surfaces.tooltip);
        let text_style = text_token_style(
            &state.theme,
            state.theme.text.sm,
            state.theme.surfaces.tooltip_text,
        );
        let text_slot = inset_rect(bubble, metrics.tooltip_padding);
        ctx.push_clip_rect(text_slot);
        paint_aligned_text(
            ctx,
            text_slot,
            &state.text,
            &text_style,
            text_style.line_height,
            0.0,
        );
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        let presented = self.state.borrow().is_presented();
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if presented {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.state
            .borrow()
            .is_presented()
            .then_some(StackSurfaceOptions {
                transient: true,
                hit_test: false,
                ..StackSurfaceOptions::default()
            })
    }
}

pub struct Tooltip {
    pub(super) child: SingleChild,
    pub(super) overlay: SingleChild,
    pub(super) state: Rc<RefCell<TooltipPresentationState>>,
}

impl Tooltip {
    pub fn new<W>(text: impl Into<String>, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let state = Rc::new(RefCell::new(TooltipPresentationState::new(text.into())));
        Self {
            child: SingleChild::new(child),
            overlay: SingleChild::new(TooltipOverlay::new(Rc::clone(&state))),
            state,
        }
    }

    pub fn theme(self, theme: DefaultTheme) -> Self {
        self.state.borrow_mut().theme = theme;
        self
    }

    pub fn placement(self, placement: TooltipPlacement) -> Self {
        self.state.borrow_mut().placement = placement;
        self
    }

    pub fn alignment(self, alignment: TooltipAlignment) -> Self {
        self.state.borrow_mut().alignment = alignment;
        self
    }

    pub(super) fn set_hovered(&mut self, ctx: &mut EventCtx, hovered: bool) {
        let overlay_id = self.overlay.child().id();
        let mut state = self.state.borrow_mut();
        if state.hovered == hovered {
            return;
        }
        let was_presented = state.is_presented();
        let motion = state.theme.motion;
        state.hovered = hovered;
        let should_animate = state.reveal.set_target(
            hovered as u8 as f32,
            ctx.current_time(),
            motion.entrance_duration(),
            motion.entrance_easing(),
        );
        let is_presented = state.is_presented();
        drop(state);

        if was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, overlay_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_semantics();
    }
}

impl Widget for Tooltip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx, ctx.bounds().contains(pointer.position));
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx, ctx.bounds().contains(pointer.position));
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(ctx, ctx.bounds().contains(pointer.position));
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let overlay_id = self.overlay.child().id();
                let mut state = self.state.borrow_mut();
                let was_presented = state.is_presented();
                let previous = state.reveal.value;
                let animating = state.reveal.advance(*time);
                let changed = state.reveal.changed_since(previous);
                let is_presented = state.is_presented();
                drop(state);

                if changed {
                    request_child_invalidation(ctx, overlay_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, overlay_id, InvalidationKind::Effect);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, overlay_id, InvalidationKind::Visibility);
                }
                if animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let mut state = self.state.borrow_mut();
        let text_style = text_token_style(
            &state.theme,
            state.theme.text.sm,
            state.theme.surfaces.tooltip_text,
        );
        state.measurement = Some(measure_text(ctx, &state.text, &text_style));
        drop(state);
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_bounds =
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size());
        self.child.arrange(ctx, trigger_bounds);

        let mut state = self.state.borrow_mut();
        state.trigger_bounds = trigger_bounds;
        let viewport = Rect::from_origin_size(Point::ZERO, ctx.dpi().viewport);
        let (bubble_bounds, resolved_placement) = tooltip_bubble_rect(
            trigger_bounds,
            state.measurement,
            &state.theme,
            state.placement,
            state.alignment,
            viewport,
        );
        state.bubble_bounds = bubble_bounds;
        state.resolved_placement = resolved_placement;
        let overlay_bounds = if state.is_presented() {
            state.bubble_bounds
        } else {
            Rect::from_origin_size(trigger_bounds.origin, Size::ZERO)
        };
        drop(state);
        self.overlay.arrange(ctx, overlay_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
        self.overlay.paint(ctx);
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        self.state.borrow().is_presented().then_some(
            OverlayOptions::new(OverlayKind::Tooltip)
                .dismiss(OverlayDismissPolicy::NONE)
                .focus(OverlayFocusBehavior::NONE),
        )
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
        let state = self.state.borrow();
        if state.hovered {
            let mut node =
                SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tooltip, state.bubble_bounds);
            node.name = Some(state.text.clone());
            ctx.push(node);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
        if self.state.borrow().is_presented() {
            self.overlay.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
        if self.state.borrow().is_presented() {
            self.overlay.visit_children_mut(visitor);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PopoverVisuals {
    pub(super) background: Color,
    pub(super) border: Color,
    pub(super) focus_ring: Option<Color>,
    pub(super) surface_style: Option<ResolvedHdrStyle>,
    pub(super) arrival_effect: Option<ResolvedEffectStyle>,
}

#[derive(Debug, Clone)]
pub(super) struct PopoverSurfaceState {
    pub(super) theme: DefaultTheme,
    pub(super) frame_rect: Rect,
    pub(super) arrival_active: bool,
    pub(super) reveal: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
}

impl PopoverSurfaceState {
    pub(super) fn new() -> Self {
        Self {
            theme: DefaultTheme::default(),
            frame_rect: Rect::ZERO,
            arrival_active: false,
            reveal: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
        }
    }

    pub(super) fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    pub(super) fn arrival_duration(&self) -> f64 {
        (0.18 / self.theme.hdr.effects.pulse.speed.max(0.25) as f64).clamp(0.10, 0.28)
    }

    pub(super) fn layer_properties(&self) -> LayerProperties {
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(
                0.0,
                -self.theme.metrics.popover_reveal_offset * (1.0 - self.reveal.value),
            ),
        }
    }

    pub(super) fn resolved_visuals(&self) -> PopoverVisuals {
        let palette = self.theme.palette;

        if !self.is_presented() || matches!(self.theme.hdr.mode, HdrThemeMode::Disabled) {
            return PopoverVisuals {
                background: palette.surface_raised,
                border: palette.border,
                focus_ring: Some(palette.focus_ring),
                surface_style: None,
                arrival_effect: None,
            };
        }

        let surface_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::SurfaceElevated,
            WidgetLuminanceRole::Standard,
            WidgetMaterialRole::Raised,
            self.arrival_active.then_some(WidgetEffectRole::Pulse),
        ));
        let border_style = cap_resolved_hdr_style(resolve_widget_hdr_style(
            &self.theme.hdr,
            WidgetColorRole::SurfaceOutline,
            WidgetLuminanceRole::Standard,
            WidgetMaterialRole::Flat,
            None,
        ));

        PopoverVisuals {
            background: surface_style.color,
            border: border_style.color,
            focus_ring: Some(border_style.color.with_alpha(palette.focus_ring.alpha)),
            surface_style: Some(surface_style),
            arrival_effect: surface_style.effect,
        }
    }
}

pub(super) struct PopoverSurface {
    pub(super) content: SingleChild,
    pub(super) state: Rc<RefCell<PopoverSurfaceState>>,
}

impl PopoverSurface {
    pub(super) fn new<C>(state: Rc<RefCell<PopoverSurfaceState>>, content: C) -> Self
    where
        C: Widget + 'static,
    {
        Self {
            content: SingleChild::new(content),
            state,
        }
    }
}

impl Widget for PopoverSurface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if !state.is_presented() {
            return Size::ZERO;
        }
        let padding = state.theme.metrics.popover_padding;
        drop(state);

        let content_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                if constraints.max.width.is_finite() {
                    (constraints.max.width - padding.left - padding.right).max(0.0)
                } else {
                    f32::INFINITY
                },
                if constraints.max.height.is_finite() {
                    (constraints.max.height - padding.top - padding.bottom).max(0.0)
                } else {
                    f32::INFINITY
                },
            ),
        );
        let content_size = self.content.measure(ctx, content_constraints);
        Size::new(
            content_size.width + padding.left + padding.right,
            content_size.height + padding.top + padding.bottom,
        )
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let state = self.state.borrow();
        if !state.is_presented() {
            drop(state);
            self.content
                .arrange(ctx, Rect::from_origin_size(bounds.origin, Size::ZERO));
            return;
        }
        let padding = state.theme.metrics.popover_padding;
        drop(state);
        let content_size = self.content.child().measured_size();
        self.content.arrange(
            ctx,
            Rect::new(
                bounds.x() + padding.left,
                bounds.y() + padding.top,
                content_size.width,
                content_size.height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let rect = ctx.bounds();
        let metrics = state.theme.metrics;
        let visuals = state.resolved_visuals();
        // Elevation shadow behind the popover surface, drawn before the fill.
        let surface_radius = metrics.corner_radius + 2.0;
        paint_theme_shadow(
            ctx,
            rect,
            [surface_radius; 4],
            &state.theme.shadows.box_shadow.md,
        );
        draw_control_frame(
            ctx,
            rect,
            surface_radius,
            metrics,
            visuals.background,
            visuals.border,
            None,
        );
        if let Some(arrival_effect) = visuals.arrival_effect {
            draw_popover_arrival_overlay(
                ctx,
                rect,
                metrics,
                visuals.background,
                visuals.border,
                arrival_effect,
            );
        }
        drop(state);
        self.content.paint(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        let presented = self.state.borrow().is_presented();
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if presented {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.state
            .borrow()
            .is_presented()
            .then_some(StackSurfaceOptions {
                transient: true,
                ..StackSurfaceOptions::default()
            })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}

pub(super) struct PopoverFocusSurface {
    pub(super) state: Rc<RefCell<PopoverSurfaceState>>,
}

impl PopoverFocusSurface {
    pub(super) fn new(state: Rc<RefCell<PopoverSurfaceState>>) -> Self {
        Self { state }
    }
}

impl Widget for PopoverFocusSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.frame_rect.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() || !state.focus_animation.is_presented() {
            return;
        }

        let Some(focus_ring) = state.resolved_visuals().focus_ring else {
            return;
        };
        let progress = state.focus_animation.value;
        if progress <= AnimatedScalar::EPSILON {
            return;
        }

        let metrics = state.theme.metrics;
        draw_focus_ring_frame(
            ctx,
            ctx.bounds(),
            metrics.corner_radius + 2.0,
            metrics,
            focus_ring.with_alpha(focus_ring.alpha * progress),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        let state = self.state.borrow();
        (state.is_presented() && state.focus_animation.is_presented()).then_some(
            StackSurfaceOptions {
                transient: true,
                hit_test: false,
                ..StackSurfaceOptions::default()
            },
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PopoverAlignment {
    #[default]
    Start,
    End,
}

pub struct Popover {
    pub(super) name: String,
    pub(super) trigger: SingleChild,
    pub(super) surface: SingleChild,
    pub(super) focus_surface: SingleChild,
    pub(super) open: bool,
    pub(super) open_reader: Option<Box<dyn Fn() -> bool>>,
    pub(super) on_open_change: Option<Box<dyn FnMut(bool)>>,
    pub(super) alignment: PopoverAlignment,
    pub(super) gap: f32,
    pub(super) arrival_timer: Option<TimerToken>,
    pub(super) state: Rc<RefCell<PopoverSurfaceState>>,
}

impl Popover {
    pub fn new<T, C>(name: impl Into<String>, trigger: T, content: C) -> Self
    where
        T: Widget + 'static,
        C: Widget + 'static,
    {
        let state = Rc::new(RefCell::new(PopoverSurfaceState::new()));
        Self {
            name: name.into(),
            trigger: SingleChild::new(trigger),
            surface: SingleChild::new(PopoverSurface::new(Rc::clone(&state), content)),
            focus_surface: SingleChild::new(PopoverFocusSurface::new(Rc::clone(&state))),
            open: false,
            open_reader: None,
            on_open_change: None,
            alignment: PopoverAlignment::Start,
            gap: DefaultTheme::default().metrics.popover_gap,
            arrival_timer: None,
            state,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.gap = theme.metrics.popover_gap;
        self.state.borrow_mut().theme = theme;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self.open_reader = None;
        {
            let mut state = self.state.borrow_mut();
            state.reveal = AnimatedScalar::new(if open { 1.0 } else { 0.0 });
        }
        self
    }

    pub fn open_when<F>(mut self, open: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.open_reader = Some(Box::new(open));
        self
    }

    pub fn on_open_change<F>(mut self, on_open_change: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_open_change = Some(Box::new(on_open_change));
        self
    }

    /// Aligns a narrower trigger and surface within the popover's measured
    /// width. End alignment keeps title-bar and trailing toolbar triggers
    /// anchored while a wider surface opens beneath them.
    pub fn alignment(mut self, alignment: PopoverAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub(super) fn sync_external_open(&mut self) {
        let Some(open) = self.open_reader.as_ref().map(|open| open()) else {
            return;
        };
        if self.open == open {
            return;
        }
        self.open = open;
        let mut state = self.state.borrow_mut();
        state.reveal = AnimatedScalar::new(if open { 1.0 } else { 0.0 });
        state.arrival_active = false;
    }

    pub(super) fn start_arrival(&mut self, ctx: &mut EventCtx) {
        if let Some(token) = self.arrival_timer.take() {
            ctx.cancel_timer(token);
        }

        let mut state = self.state.borrow_mut();
        state.arrival_active = !matches!(state.theme.hdr.mode, HdrThemeMode::Disabled)
            && state.theme.hdr.effects.pulse.intensity > 0.0;
        if state.arrival_active {
            self.arrival_timer = Some(ctx.schedule_timer_after(state.arrival_duration()));
        }
    }

    pub(super) fn stop_arrival(&mut self, ctx: &mut EventCtx) {
        self.state.borrow_mut().arrival_active = false;
        if let Some(token) = self.arrival_timer.take() {
            ctx.cancel_timer(token);
        }
    }

    pub(super) fn trigger_rect(&self) -> Rect {
        self.trigger.child().bounds()
    }

    pub(super) fn content_rect(&self) -> Rect {
        self.state.borrow().frame_rect
    }

    pub(super) fn is_inside_open_regions(&self, position: Point) -> bool {
        self.trigger_rect().contains(position)
            || (self.open && self.content_rect().contains(position))
    }

    pub(super) fn set_open(&mut self, ctx: &mut EventCtx, open: bool) {
        if self.open == open {
            return;
        }

        if open {
            self.start_arrival(ctx);
        } else {
            self.stop_arrival(ctx);
        }

        self.open = open;
        if let Some(on_open_change) = &mut self.on_open_change {
            on_open_change(open);
        }
        let surface_id = self.surface.child().id();
        let focus_surface_id = self.focus_surface.child().id();
        let mut state = self.state.borrow_mut();
        let was_presented = state.is_presented();
        let motion = state.theme.motion;
        let should_animate = state.reveal.set_target(
            open as u8 as f32,
            ctx.current_time(),
            motion.entrance_duration(),
            motion.entrance_easing(),
        );
        let is_presented = state.is_presented();
        drop(state);

        if open || was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
            request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_semantics();
    }
}

impl Widget for Popover {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(OVERLAY_DISMISS_REQUEST).is_some() && self.open {
            self.set_open(ctx, false);
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.trigger_rect().contains(pointer.position) =>
            {
                let next = !self.open;
                self.set_open(ctx, next);
                ctx.request_focus();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open
                    && !self.is_inside_open_regions(pointer.position) =>
            {
                self.set_open(ctx, false);
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && key.key == "Escape"
                    && self.open =>
            {
                self.set_open(ctx, false);
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.set_open(ctx, !self.open);
                ctx.set_handled();
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                let open = match semantics.action {
                    sui_core::SemanticsActionRequest::Expand => Some(true),
                    sui_core::SemanticsActionRequest::Collapse => Some(false),
                    _ => None,
                };
                if let Some(open) = open {
                    self.set_open(ctx, open);
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let surface_id = self.surface.child().id();
                let focus_surface_id = self.focus_surface.child().id();
                let mut state = self.state.borrow_mut();
                let was_presented = state.is_presented();
                let was_focus_presented = state.focus_animation.is_presented();
                let previous_reveal = state.reveal.value;
                let previous_focus = state.focus_animation.value;
                let reveal_animating = state.reveal.advance(*time);
                let focus_animating = state.focus_animation.advance(*time);
                let reveal_changed = state.reveal.changed_since(previous_reveal);
                let focus_changed = state.focus_animation.changed_since(previous_focus);
                let is_presented = state.is_presented();
                let is_focus_presented = state.focus_animation.is_presented();
                drop(state);

                if reveal_changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Effect);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Effect);
                }
                if focus_changed {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if was_focus_presented != is_focus_presented {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if reveal_animating || focus_animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::Timer { token, .. }) if self.arrival_timer == Some(*token) => {
                self.arrival_timer = None;
                let surface_id = self.surface.child().id();
                let mut state = self.state.borrow_mut();
                if state.arrival_active {
                    state.arrival_active = false;
                    drop(state);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
                } else {
                    drop(state);
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.sync_external_open();
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());
        // A popover's trigger belongs to its parent's layout, but its surface belongs to the
        // window overlay stack. In particular, toolbar and title-bar slots are commonly tight to
        // the trigger; reusing those constraints for the surface collapses a wide panel into that
        // narrow slot. Measure the overlay against the viewport instead and keep it out of the
        // parent's reported size.
        let viewport = ctx.dpi().viewport;
        let viewport_margin = self.gap.max(4.0);
        let surface_max = Size::new(
            if viewport.width > 0.0 {
                (viewport.width - viewport_margin * 2.0).max(0.0)
            } else {
                f32::INFINITY
            },
            if viewport.height > 0.0 {
                (viewport.height - viewport_margin * 2.0).max(0.0)
            } else {
                f32::INFINITY
            },
        );
        let surface_size = self
            .surface
            .measure(ctx, Constraints::new(Size::ZERO, surface_max));
        let presented = self.state.borrow().is_presented();
        let focus_size = if presented { surface_size } else { Size::ZERO };
        self.focus_surface
            .measure(ctx, Constraints::tight(focus_size));
        constraints.clamp(trigger_size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let trigger_size = self.trigger.child().measured_size();
        let aligned_x = |width: f32| match self.alignment {
            PopoverAlignment::Start => bounds.x(),
            PopoverAlignment::End => bounds.max_x() - width,
        };
        let trigger_bounds = Rect::new(
            aligned_x(trigger_size.width),
            bounds.y(),
            trigger_size.width,
            trigger_size.height,
        );
        self.trigger.arrange(ctx, trigger_bounds);

        let presented = self.state.borrow().is_presented();
        let surface_bounds = if presented {
            let surface_size = self.surface.child().measured_size();
            let viewport = Rect::from_origin_size(Point::ZERO, ctx.dpi().viewport);
            let margin = self.gap.max(4.0);
            let width = surface_size.width.max(trigger_size.width);
            let alignment = match self.alignment {
                PopoverAlignment::Start => OverlayAlignment::Start,
                PopoverAlignment::End => OverlayAlignment::End,
            };
            place_overlay(
                &OverlayPlacementRequest::new(
                    trigger_bounds,
                    Size::new(width, surface_size.height),
                    viewport,
                    OverlayPlacement::new(OverlaySide::Bottom, alignment),
                )
                .fallbacks([OverlayPlacement::new(OverlaySide::Top, alignment)])
                .gap(self.gap)
                .margin(margin),
            )
            .bounds
        } else {
            Rect::from_origin_size(trigger_bounds.origin, Size::ZERO)
        };
        self.state.borrow_mut().frame_rect = surface_bounds;
        self.surface.arrange(ctx, surface_bounds);
        self.focus_surface.arrange(ctx, surface_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.state.borrow().is_presented() {
            self.surface.paint(ctx);
            self.focus_surface.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Popover, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(self.open);
        node.popup = Some(SemanticsPopupKind::Dialog);
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
        ];
        ctx.push(node);
        self.trigger.semantics(ctx);
        if self.open {
            self.surface.semantics(ctx);
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (self.open || self.state.borrow().is_presented()).then_some(
            OverlayOptions::new(OverlayKind::Popover)
                .dismiss(if self.open {
                    OverlayDismissPolicy::TRANSIENT
                } else {
                    OverlayDismissPolicy::NONE
                })
                .focus(OverlayFocusBehavior::NONE),
        )
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let focus_surface_id = self.focus_surface.child().id();
        let mut state = self.state.borrow_mut();
        let was_focus_presented = state.focus_animation.is_presented();
        let theme = state.theme;
        set_focus_animation_target(
            &mut state.focus_animation,
            focused as u8 as f32,
            &theme,
            ctx,
        );
        let is_focus_presented = state.focus_animation.is_presented();
        drop(state);

        if was_focus_presented != is_focus_presented {
            request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
        }
        request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
        if !focused && self.open {
            self.set_open(ctx, false);
        }
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.open || self.state.borrow().is_presented() {
            self.surface.visit_children(visitor);
            self.focus_surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.open || self.state.borrow().is_presented() {
            self.surface.visit_children_mut(visitor);
            self.focus_surface.visit_children_mut(visitor);
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ContextMenuPanel {
    pub(super) prefix: Vec<usize>,
    pub(super) items: Vec<MenuItem>,
    pub(super) frame_rect: Rect,
    pub(super) opens_left: bool,
}

impl ContextMenuPanel {
    pub(super) fn item_rect(
        &self,
        theme: &DefaultTheme,
        row_height: f32,
        index: usize,
    ) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let padding = theme.metrics.menu_padding;
        Some(Rect::new(
            self.frame_rect.x() + padding.left,
            self.frame_rect.y() + padding.top + (index as f32 * row_height),
            (self.frame_rect.width() - padding.left - padding.right).max(0.0),
            row_height,
        ))
    }
}

#[derive(Debug, Clone)]
pub(super) struct ContextMenuPresentationState {
    pub(super) theme: DefaultTheme,
    pub(super) panels: Vec<ContextMenuPanel>,
    pub(super) highlighted: Option<Vec<usize>>,
    pub(super) highlight_visual: Option<Vec<usize>>,
    pub(super) pressed: Option<Vec<usize>>,
    pub(super) press_visual: Option<Vec<usize>>,
    pub(super) surface_rect: Rect,
    pub(super) row_height: f32,
    pub(super) reveal: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) highlight_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
}

impl ContextMenuPresentationState {
    pub(super) fn new() -> Self {
        let theme = DefaultTheme::default();
        Self {
            theme,
            panels: Vec::new(),
            highlighted: None,
            highlight_visual: None,
            pressed: None,
            press_visual: None,
            surface_rect: Rect::ZERO,
            row_height: menu_row_height(&theme),
            reveal: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            highlight_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
        }
    }

    pub(super) fn is_presented(&self) -> bool {
        self.reveal.is_presented()
    }

    pub(super) fn item_rect(&self, panel: usize, index: usize) -> Option<Rect> {
        self.panels
            .get(panel)?
            .item_rect(&self.theme, self.row_height, index)
    }

    pub(super) fn layer_properties(&self) -> LayerProperties {
        LayerProperties {
            opacity: self.reveal.value,
            translation: Vector::new(
                0.0,
                -self.theme.metrics.popover_reveal_offset * (1.0 - self.reveal.value),
            ),
        }
    }

    pub(super) fn highlight_amount_for(&self, path: &[usize]) -> f32 {
        if self.highlight_visual.as_deref() == Some(path) {
            self.highlight_animation.value
        } else {
            0.0
        }
    }

    pub(super) fn press_amount_for(&self, path: &[usize]) -> f32 {
        if self.press_visual.as_deref() == Some(path) {
            self.press_animation.value
        } else {
            0.0
        }
    }
}

pub(super) struct ContextMenuSurface {
    pub(super) state: Rc<RefCell<ContextMenuPresentationState>>,
}

impl ContextMenuSurface {
    pub(super) fn new(state: Rc<RefCell<ContextMenuPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for ContextMenuSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.surface_rect.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() {
            return;
        }

        let theme = state.theme;
        let metrics = theme.metrics;
        let palette = theme.palette;
        let interaction = theme.interaction;
        let item_padding = metrics.menu_item_padding;
        let surface_radius = metrics.corner_radius + 2.0;
        let submenu_width = menu_submenu_indicator_width(&theme);

        for (panel_index, panel) in state.panels.iter().enumerate() {
            let menu = panel.frame_rect;
            paint_theme_shadow(ctx, menu, [surface_radius; 4], &theme.shadows.box_shadow.lg);
            draw_control_frame(
                ctx,
                menu,
                surface_radius,
                metrics,
                palette.surface_raised,
                palette.border,
                None,
            );

            for (index, item) in panel.items.iter().enumerate() {
                let Some(row) = state.item_rect(panel_index, index) else {
                    continue;
                };
                let mut path = panel.prefix.clone();
                path.push(index);

                if item.separator_before {
                    let line = Rect::new(
                        row.x(),
                        row.y() - (metrics.menu_padding.top * 0.5),
                        row.width(),
                        1.0,
                    );
                    ctx.fill(rounded_rect_path(line, 0.5), palette.border);
                }

                let highlighted = state.highlighted.as_deref() == Some(path.as_slice());
                let highlight_amount = state.highlight_amount_for(&path);
                let press_amount = state.press_amount_for(&path);
                let label_style = theme.text_style(item.text_color(&theme));
                let shortcut_width = item
                    .shortcut
                    .as_ref()
                    .map(|_| metrics.menu_shortcut_width)
                    .unwrap_or(0.0);
                let indicator_width = if item.has_submenu() {
                    submenu_width
                } else {
                    0.0
                };
                let label_slot = Rect::new(
                    row.x() + item_padding.left,
                    row.y(),
                    (row.width()
                        - item_padding.left
                        - item_padding.right
                        - shortcut_width
                        - indicator_width)
                        .max(0.0),
                    row.height(),
                );
                if highlighted || highlight_amount > 0.0 || press_amount > 0.0 {
                    let highlight_background =
                        mix_color(palette.control, palette.selection, highlight_amount);
                    let background = if press_amount > 0.0 {
                        mix_color(
                            highlight_background,
                            palette.control_active,
                            interaction.pressed_blend * press_amount,
                        )
                    } else {
                        highlight_background
                    };
                    ctx.fill(
                        rounded_rect_path(row.inflate(-2.0, -2.0), metrics.corner_radius - 2.0),
                        background,
                    );
                }

                ctx.push_clip_rect(label_slot);
                paint_aligned_text(
                    ctx,
                    label_slot,
                    &item.label,
                    &label_style,
                    label_style.line_height,
                    0.0,
                );
                ctx.pop_clip();

                if let Some(shortcut) = &item.shortcut {
                    let shortcut_style = theme.placeholder_text_style();
                    let shortcut_slot = Rect::new(
                        row.max_x()
                            - item_padding.right
                            - indicator_width
                            - metrics.menu_shortcut_width,
                        row.y(),
                        metrics.menu_shortcut_width,
                        row.height(),
                    );
                    ctx.push_clip_rect(shortcut_slot);
                    paint_aligned_text(
                        ctx,
                        shortcut_slot,
                        shortcut,
                        &shortcut_style,
                        shortcut_style.line_height,
                        1.0,
                    );
                    ctx.pop_clip();
                }

                if item.has_submenu() {
                    let indicator_style = theme.text_style(item.text_color(&theme));
                    let indicator_slot = Rect::new(
                        row.max_x() - item_padding.right - indicator_width,
                        row.y(),
                        indicator_width,
                        row.height(),
                    );
                    ctx.push_clip_rect(indicator_slot);
                    paint_aligned_text(
                        ctx,
                        indicator_slot,
                        "\u{203a}",
                        &indicator_style,
                        indicator_style.line_height,
                        1.0,
                    );
                    ctx.pop_clip();
                }
            }
        }
    }

    fn layer_options(&self) -> LayerOptions {
        let presented = self.state.borrow().is_presented();
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if presented {
                LayerCompositionMode::Overlay
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.state
            .borrow()
            .is_presented()
            .then_some(StackSurfaceOptions {
                transient: true,
                ..StackSurfaceOptions::default()
            })
    }
}

pub(super) struct ContextMenuFocusSurface {
    pub(super) state: Rc<RefCell<ContextMenuPresentationState>>,
}

impl ContextMenuFocusSurface {
    pub(super) fn new(state: Rc<RefCell<ContextMenuPresentationState>>) -> Self {
        Self { state }
    }
}

impl Widget for ContextMenuFocusSurface {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _constraints: Constraints) -> Size {
        let state = self.state.borrow();
        if state.is_presented() {
            state.surface_rect.size
        } else {
            Size::ZERO
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let state = self.state.borrow();
        if !state.is_presented() || !state.focus_animation.is_presented() {
            return;
        }

        let progress = state.focus_animation.value;
        if progress <= AnimatedScalar::EPSILON {
            return;
        }

        let metrics = state.theme.metrics;
        let palette = state.theme.palette;
        for panel in &state.panels {
            draw_focus_ring_frame(
                ctx,
                panel.frame_rect,
                metrics.corner_radius + 2.0,
                metrics,
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * progress),
            );
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        self.state.borrow().layer_properties()
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        let state = self.state.borrow();
        (state.is_presented() && state.focus_animation.is_presented()).then_some(
            StackSurfaceOptions {
                transient: true,
                hit_test: false,
                ..StackSurfaceOptions::default()
            },
        )
    }
}

pub struct ContextMenu {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) trigger: SingleChild,
    pub(super) items: Vec<MenuItem>,
    pub(super) items_provider: Option<Box<dyn Fn() -> Vec<MenuItem>>>,
    pub(super) open: bool,
    pub(super) open_path: Vec<usize>,
    pub(super) highlighted: Option<Vec<usize>>,
    pub(super) highlight_visual: Option<Vec<usize>>,
    pub(super) pressed: Option<Vec<usize>>,
    pub(super) press_visual: Option<Vec<usize>>,
    pub(super) highlight_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) panels: Vec<ContextMenuPanel>,
    pub(super) surface: SingleChild,
    pub(super) focus_surface: SingleChild,
    pub(super) surface_state: Rc<RefCell<ContextMenuPresentationState>>,
    pub(super) activation_button: PointerButton,
    pub(super) primary_trigger_press: Option<u64>,
    pub(super) anchor_to_pointer: Option<bool>,
    pub(super) open_position: Option<Point>,
    pub(super) on_activate: Option<Box<dyn FnMut(usize, MenuItem)>>,
    pub(super) on_activate_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, usize, MenuItem)>>,
    pub(super) on_activate_path: Option<Box<dyn FnMut(Vec<usize>, MenuItem)>>,
    pub(super) on_activate_path_with_ctx:
        Option<Box<dyn FnMut(&mut EventCtx, Vec<usize>, MenuItem)>>,
}

impl ContextMenu {
    pub fn new<W>(name: impl Into<String>, trigger: W) -> Self
    where
        W: Widget + 'static,
    {
        let surface_state = Rc::new(RefCell::new(ContextMenuPresentationState::new()));
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            trigger: SingleChild::new(trigger),
            items: Vec::new(),
            items_provider: None,
            open: false,
            open_path: Vec::new(),
            highlighted: None,
            highlight_visual: None,
            pressed: None,
            press_visual: None,
            highlight_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            panels: Vec::new(),
            surface: SingleChild::new(ContextMenuSurface::new(Rc::clone(&surface_state))),
            focus_surface: SingleChild::new(ContextMenuFocusSurface::new(Rc::clone(
                &surface_state,
            ))),
            surface_state,
            activation_button: PointerButton::Secondary,
            primary_trigger_press: None,
            anchor_to_pointer: None,
            open_position: None,
            on_activate: None,
            on_activate_with_ctx: None,
            on_activate_path: None,
            on_activate_path_with_ctx: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = MenuItem>,
    {
        self.items.extend(items);
        self
    }

    /// Rebuild the item list every time the menu opens, so per-item enabled
    /// state can reflect current application state (selection, clipboard, …).
    pub fn items_when<F>(mut self, provider: F) -> Self
    where
        F: Fn() -> Vec<MenuItem> + 'static,
    {
        self.items_provider = Some(Box::new(provider));
        self
    }

    /// Widget id of the wrapped trigger. Menu activations can route commands
    /// back to it via `EventCtx::post_event` — for example the standard text
    /// editing commands (`TextCommand`) understood by the text widgets.
    pub fn trigger_id(&self) -> WidgetId {
        self.trigger.child().id()
    }

    /// Handle leaf activation.
    ///
    /// Flat menus receive the activated item index as before. A nested leaf
    /// receives the index of its root submenu owner; use
    /// [`Self::on_activate_path`] when the complete path is significant.
    pub fn on_activate<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(usize, MenuItem) + 'static,
    {
        self.on_activate = Some(Box::new(on_activate));
        self
    }

    /// Event-context variant of [`Self::on_activate`].
    pub fn on_activate_with_ctx<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(&mut EventCtx, usize, MenuItem) + 'static,
    {
        self.on_activate_with_ctx = Some(Box::new(on_activate));
        self
    }

    /// Handle leaf activation with the complete index path from the root item
    /// to the activated nested item.
    pub fn on_activate_path<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(Vec<usize>, MenuItem) + 'static,
    {
        self.on_activate_path = Some(Box::new(on_activate));
        self
    }

    /// Handle leaf activation with event context and its complete nested index
    /// path.
    pub fn on_activate_path_with_ctx<F>(mut self, on_activate: F) -> Self
    where
        F: FnMut(&mut EventCtx, Vec<usize>, MenuItem) + 'static,
    {
        self.on_activate_path_with_ctx = Some(Box::new(on_activate));
        self
    }

    /// Set the pointer button that opens the menu.
    ///
    /// Primary activation owns the trigger click during capture, so an
    /// interactive trigger such as [`crate::Button`] does not consume or also
    /// execute the click. Secondary activation remains in target/bubble order
    /// so context-menu triggers can update their targeted row before opening.
    pub fn activation_button(mut self, activation_button: PointerButton) -> Self {
        self.activation_button = activation_button;
        self
    }

    /// Whether the menu opens at the press position instead of dropping below
    /// the trigger. Defaults by activation button: right-click menus anchor to
    /// the pointer (standard context-menu behavior, and the only sensible
    /// placement for large triggers), other buttons keep the dropdown layout.
    pub fn anchor_to_pointer(mut self, anchor_to_pointer: bool) -> Self {
        self.anchor_to_pointer = Some(anchor_to_pointer);
        self
    }

    pub(super) fn anchors_to_pointer(&self) -> bool {
        self.anchor_to_pointer
            .unwrap_or(self.activation_button == PointerButton::Secondary)
    }

    pub(super) fn row_height(&self) -> f32 {
        menu_row_height(&self.resolved_theme())
    }

    pub(super) fn measured_menu_width_for_items(
        &self,
        ctx: &mut MeasureCtx,
        items: &[MenuItem],
    ) -> f32 {
        let theme = self.resolved_theme();
        let label_style = theme.body_text_style();
        let shortcut_style = theme.placeholder_text_style();
        let mut width: f32 = 220.0;
        for item in items {
            let label = measure_text(ctx, item.label(), &label_style).width;
            let shortcut = item
                .shortcut
                .as_ref()
                .map(|text| measure_text(ctx, text, &shortcut_style).width)
                .unwrap_or(0.0);
            width = width.max(
                label
                    + shortcut
                    + theme.metrics.menu_item_padding.left
                    + theme.metrics.menu_item_padding.right
                    + theme.metrics.menu_shortcut_width
                    + if item.has_submenu() {
                        menu_submenu_indicator_width(&theme)
                    } else {
                        0.0
                    },
            );
        }
        width
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn trigger_rect(&self) -> Rect {
        self.trigger.child().bounds()
    }

    pub(super) fn items_at_prefix(&self, prefix: &[usize]) -> Option<&[MenuItem]> {
        let mut items = self.items.as_slice();
        for index in prefix {
            items = items.get(*index)?.submenu_items();
        }
        Some(items)
    }

    pub(super) fn item_at_path(&self, path: &[usize]) -> Option<&MenuItem> {
        let (&index, prefix) = path.split_last()?;
        self.items_at_prefix(prefix)?.get(index)
    }

    pub(super) fn item_rect(&self, bounds: Rect, path: &[usize]) -> Option<Rect> {
        if !self.open {
            return None;
        }
        let (&index, prefix) = path.split_last()?;
        let panel = self.panels.get(prefix.len())?;
        if panel.prefix != prefix {
            return None;
        }
        let theme = self.resolved_theme();
        panel
            .item_rect(&theme, self.row_height(), index)
            .map(|rect| rect.translate(bounds.origin.to_vector()))
    }

    pub(super) fn item_at(&self, bounds: Rect, position: Point) -> Option<Vec<usize>> {
        self.panels.iter().rev().find_map(|panel| {
            panel.items.iter().enumerate().find_map(|(index, _)| {
                let mut path = panel.prefix.clone();
                path.push(index);
                self.item_rect(bounds, &path)
                    .filter(|rect| rect.contains(position))
                    .map(|_| path)
            })
        })
    }

    pub(super) fn surface_rect(&self) -> Rect {
        self.panels
            .iter()
            .map(|panel| panel.frame_rect)
            .reduce(Rect::union)
            .unwrap_or(Rect::ZERO)
    }

    pub(super) fn sync_surface_state(&self, bounds: Rect) {
        let theme = self.resolved_theme();
        let translation = bounds.origin.to_vector();
        let mut state = self.surface_state.borrow_mut();
        state.theme = theme;
        state.panels = self
            .panels
            .iter()
            .cloned()
            .map(|mut panel| {
                panel.frame_rect = panel.frame_rect.translate(translation);
                panel
            })
            .collect();
        state.highlighted = self.highlighted.clone();
        state.highlight_visual = self.highlight_visual.clone();
        state.pressed = self.pressed.clone();
        state.press_visual = self.press_visual.clone();
        state.highlight_animation = self.highlight_animation;
        state.press_animation = self.press_animation;
        state.surface_rect = self.surface_rect().translate(translation);
        state.row_height = self.row_height();
    }

    pub(super) fn refresh_surface_interaction_state(&self, ctx: &mut EventCtx) {
        let surface_id = self.surface.child().id();
        let mut state = self.surface_state.borrow_mut();
        let changed = state.highlighted != self.highlighted
            || state.highlight_visual != self.highlight_visual
            || state.pressed != self.pressed
            || state.press_visual != self.press_visual
            || state.highlight_animation != self.highlight_animation
            || state.press_animation != self.press_animation;
        state.highlighted = self.highlighted.clone();
        state.highlight_visual = self.highlight_visual.clone();
        state.pressed = self.pressed.clone();
        state.press_visual = self.press_visual.clone();
        state.highlight_animation = self.highlight_animation;
        state.press_animation = self.press_animation;
        let presented = state.is_presented();
        drop(state);

        if changed && presented {
            request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
        }
    }

    pub(super) fn set_highlighted(&mut self, highlighted: Option<Vec<usize>>, ctx: &mut EventCtx) {
        if self.highlighted == highlighted {
            return;
        }
        let theme = self.resolved_theme();
        self.highlighted = highlighted.clone();
        if let Some(path) = highlighted {
            self.highlight_visual = Some(path);
            self.highlight_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.highlight_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.highlight_animation, 0.0, &theme, ctx) {
            self.highlight_visual = None;
        }
        self.refresh_surface_interaction_state(ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn set_pressed(&mut self, pressed: Option<Vec<usize>>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed.clone();
        if let Some(path) = pressed {
            self.press_visual = Some(path);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        self.refresh_surface_interaction_state(ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn advance_row_animations(&mut self, time: f64) -> bool {
        let highlight_animating = self.highlight_animation.advance(time);
        if !highlight_animating
            && self.highlighted.is_none()
            && self.highlight_animation.value <= AnimatedScalar::EPSILON
        {
            self.highlight_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        highlight_animating | press_animating
    }

    pub(super) fn set_open_path(&mut self, ctx: &mut EventCtx, open_path: Vec<usize>) {
        if self.open_path == open_path {
            return;
        }
        self.open_path = open_path;
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn update_pointer_highlight(
        &mut self,
        ctx: &mut EventCtx,
        highlighted: Option<Vec<usize>>,
    ) {
        if let Some(path) = highlighted {
            let opens_submenu = self
                .item_at_path(&path)
                .is_some_and(|item| item.enabled && item.has_submenu());
            let open_path = if opens_submenu {
                path.clone()
            } else {
                path[..path.len().saturating_sub(1)].to_vec()
            };
            self.set_open_path(ctx, open_path);
            self.set_highlighted(Some(path), ctx);
        } else {
            self.set_highlighted(None, ctx);
        }
    }

    pub(super) fn open_highlighted_submenu(&mut self, ctx: &mut EventCtx) -> bool {
        let Some(path) = self.highlighted.clone() else {
            return false;
        };
        let first_enabled = self.item_at_path(&path).and_then(|item| {
            item.enabled
                .then(|| item.submenu_items().iter().position(|child| child.enabled))
                .flatten()
        });
        if self
            .item_at_path(&path)
            .is_none_or(|item| !item.enabled || !item.has_submenu())
        {
            return false;
        }
        self.set_open_path(ctx, path.clone());
        if let Some(index) = first_enabled {
            let mut child = path;
            child.push(index);
            self.set_highlighted(Some(child), ctx);
        }
        true
    }

    pub(super) fn close_current_submenu(&mut self, ctx: &mut EventCtx) -> bool {
        let Some(path) = self.highlighted.clone() else {
            return false;
        };
        if path.len() > 1 {
            let owner = path[..path.len() - 1].to_vec();
            let parent_open_path = owner[..owner.len().saturating_sub(1)].to_vec();
            self.set_open_path(ctx, parent_open_path);
            self.set_highlighted(Some(owner), ctx);
            true
        } else if self.open_path == path {
            self.set_open_path(ctx, Vec::new());
            true
        } else {
            false
        }
    }

    pub(super) fn move_highlight(&mut self, delta: isize, ctx: &mut EventCtx) {
        let prefix = self
            .highlighted
            .as_deref()
            .map(|path| &path[..path.len().saturating_sub(1)])
            .unwrap_or(&[])
            .to_vec();
        let Some(items) = self.items_at_prefix(&prefix) else {
            return;
        };
        if items.is_empty() {
            return;
        }
        let current = self
            .highlighted
            .as_deref()
            .and_then(|path| path.last().copied());
        let len = items.len() as isize;
        let mut index = current.map_or(if delta > 0 { -1 } else { len }, |index| index as isize);
        loop {
            let next = (index + delta).clamp(0, len - 1);
            if next == index {
                return;
            }
            index = next;
            if items[index as usize].enabled {
                break;
            }
        }
        let mut path = prefix.clone();
        path.push(index as usize);
        self.set_open_path(ctx, prefix);
        self.set_highlighted(Some(path), ctx);
    }

    pub(super) fn move_highlight_to_edge(&mut self, first: bool, ctx: &mut EventCtx) {
        let prefix = self
            .highlighted
            .as_deref()
            .map(|path| &path[..path.len().saturating_sub(1)])
            .unwrap_or(&[])
            .to_vec();
        let Some(items) = self.items_at_prefix(&prefix) else {
            return;
        };
        let index = if first {
            items.iter().position(|item| item.enabled)
        } else {
            items.iter().rposition(|item| item.enabled)
        };
        if let Some(index) = index {
            let mut path = prefix.clone();
            path.push(index);
            self.set_open_path(ctx, prefix);
            self.set_highlighted(Some(path), ctx);
        }
    }

    pub(super) fn visible_path_for_semantics_id(
        &self,
        root: WidgetId,
        target: WidgetId,
    ) -> Option<Vec<usize>> {
        self.panels.iter().find_map(|panel| {
            panel.items.iter().enumerate().find_map(|(index, _)| {
                let mut path = panel.prefix.clone();
                path.push(index);
                (virtual_menu_item_path_id(root, &path) == target).then_some(path)
            })
        })
    }

    pub(super) fn set_open(&mut self, ctx: &mut EventCtx, open: bool) {
        if self.open == open {
            return;
        }

        if open && let Some(provider) = &self.items_provider {
            self.items = provider();
        }
        if !open {
            self.open_position = None;
        }

        self.open = open;
        self.open_path.clear();
        self.highlighted = if open {
            self.items
                .iter()
                .position(|item| item.enabled)
                .map(|index| vec![index])
        } else {
            None
        };
        self.highlight_visual = self.highlighted.clone();
        self.highlight_animation = AnimatedScalar::new(self.highlighted.is_some() as u8 as f32);
        self.pressed = None;
        self.press_visual = None;
        self.press_animation = AnimatedScalar::new(0.0);
        self.panels.clear();

        let surface_id = self.surface.child().id();
        let focus_surface_id = self.focus_surface.child().id();
        let theme = self.resolved_theme();
        let mut state = self.surface_state.borrow_mut();
        state.theme = theme;
        state.panels.clear();
        state.highlighted = self.highlighted.clone();
        state.highlight_visual = self.highlight_visual.clone();
        state.pressed = self.pressed.clone();
        state.press_visual = self.press_visual.clone();
        state.highlight_animation = self.highlight_animation;
        state.press_animation = self.press_animation;
        let was_presented = state.is_presented();
        let should_animate = if open {
            let motion = theme.motion;
            state.reveal.set_target(
                1.0,
                ctx.current_time(),
                motion.entrance_duration(),
                motion.entrance_easing(),
            )
        } else {
            state.reveal = AnimatedScalar::new(0.0);
            false
        };
        let is_presented = state.is_presented();
        drop(state);

        if open || was_presented != is_presented {
            ctx.request_measure();
            request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
            request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
        }
        if should_animate {
            ctx.request_animation_frame();
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn activate_path(&mut self, ctx: &mut EventCtx, path: Vec<usize>) {
        let Some(item) = self.item_at_path(&path).cloned() else {
            return;
        };
        if !item.enabled || item.has_submenu() {
            return;
        }
        let Some(root_index) = path.first().copied() else {
            return;
        };
        if let Some(on_activate) = &mut self.on_activate {
            on_activate(root_index, item.clone());
        }
        if let Some(on_activate) = &mut self.on_activate_with_ctx {
            on_activate(ctx, root_index, item.clone());
        }
        if let Some(on_activate) = &mut self.on_activate_path {
            on_activate(path.clone(), item.clone());
        }
        if let Some(on_activate) = &mut self.on_activate_path_with_ctx {
            on_activate(ctx, path, item);
        }
    }
}

impl Widget for ContextMenu {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(OVERLAY_DISMISS_REQUEST).is_some() && self.open {
            self.set_open(ctx, false);
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move && self.open => {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                self.update_pointer_highlight(ctx, highlighted);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(self.activation_button)
                    && if self.activation_button == PointerButton::Primary {
                        ctx.phase() == EventPhase::Capture
                    } else {
                        ctx.phase() != EventPhase::Capture
                    }
                    && self.trigger_rect().contains(pointer.position) =>
            {
                // Context-click targets see the press first so they can update selection.
                // Primary dropdowns own the trigger press during capture but defer opening
                // until release. Registering a transient overlay during the opening press can
                // otherwise make the overlay host classify that same press as an outside click.
                if self.activation_button == PointerButton::Primary {
                    if self.open {
                        self.set_open(ctx, false);
                    } else {
                        self.primary_trigger_press = Some(pointer.pointer_id);
                        ctx.request_pointer_capture(pointer.pointer_id);
                    }
                } else {
                    let open = !self.open;
                    self.open_position = (open && self.anchors_to_pointer()).then(|| {
                        let origin = ctx.bounds().origin;
                        Point::new(pointer.position.x - origin.x, pointer.position.y - origin.y)
                    });
                    self.set_open(ctx, open);
                    if open {
                        ctx.request_focus();
                    }
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.primary_trigger_press == Some(pointer.pointer_id) =>
            {
                self.primary_trigger_press = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                if self.trigger_rect().contains(pointer.position) {
                    self.open_position = None;
                    self.set_open(ctx, true);
                    ctx.request_focus();
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open =>
            {
                if let Some(path) = self.item_at(ctx.bounds(), pointer.position) {
                    self.update_pointer_highlight(ctx, Some(path.clone()));
                    self.set_pressed(
                        self.item_at_path(&path)
                            .filter(|item| item.enabled)
                            .map(|_| path),
                        ctx,
                    );
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                } else if !self.trigger_rect().contains(pointer.position) {
                    self.set_open(ctx, false);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.open =>
            {
                let highlighted = self.item_at(ctx.bounds(), pointer.position);
                if let Some(path) = self
                    .pressed
                    .clone()
                    .zip(highlighted)
                    .filter(|(left, right)| left == right)
                    .map(|(path, _)| path)
                {
                    if self.item_at_path(&path).is_some_and(MenuItem::has_submenu) {
                        self.set_open_path(ctx, path);
                    } else {
                        self.activate_path(ctx, path);
                        self.set_open(ctx, false);
                    }
                }
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.primary_trigger_press == Some(pointer.pointer_id) {
                    self.primary_trigger_press = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if self.activation_button == PointerButton::Primary
                    && ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && !self.open
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.open_position = None;
                self.set_open(ctx, true);
                ctx.request_focus();
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if ctx.is_focused() && key.state == KeyState::Pressed && self.open =>
            {
                match key.key.as_str() {
                    "ArrowDown" => self.move_highlight(1, ctx),
                    "ArrowUp" => self.move_highlight(-1, ctx),
                    "Home" => self.move_highlight_to_edge(true, ctx),
                    "End" => self.move_highlight_to_edge(false, ctx),
                    "ArrowRight" => {
                        self.open_highlighted_submenu(ctx);
                    }
                    "ArrowLeft" => {
                        self.close_current_submenu(ctx);
                    }
                    "Enter" | " " => {
                        if !self.open_highlighted_submenu(ctx)
                            && let Some(path) = self.highlighted.clone()
                        {
                            self.activate_path(ctx, path);
                            self.set_open(ctx, false);
                        }
                    }
                    "Escape" => {
                        self.set_open(ctx, false);
                    }
                    _ => return,
                }
                self.refresh_surface_interaction_state(ctx);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Semantics(semantics) if self.open && semantics.target != ctx.widget_id() => {
                let Some(path) =
                    self.visible_path_for_semantics_id(ctx.widget_id(), semantics.target)
                else {
                    return;
                };
                let has_submenu = self
                    .item_at_path(&path)
                    .is_some_and(|item| item.enabled && item.has_submenu());
                match semantics.action {
                    sui_core::SemanticsActionRequest::Activate
                    | sui_core::SemanticsActionRequest::Expand
                        if has_submenu =>
                    {
                        self.set_highlighted(Some(path), ctx);
                        self.open_highlighted_submenu(ctx);
                    }
                    sui_core::SemanticsActionRequest::Collapse if has_submenu => {
                        self.set_open_path(ctx, path[..path.len().saturating_sub(1)].to_vec());
                        self.set_highlighted(Some(path), ctx);
                    }
                    sui_core::SemanticsActionRequest::Activate => {
                        self.activate_path(ctx, path);
                        self.set_open(ctx, false);
                    }
                    sui_core::SemanticsActionRequest::Focus => {
                        self.set_highlighted(Some(path), ctx);
                        ctx.request_focus();
                    }
                    _ => return,
                }
                ctx.set_handled();
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                let open = match semantics.action {
                    sui_core::SemanticsActionRequest::Activate => Some(!self.open),
                    sui_core::SemanticsActionRequest::Expand => Some(true),
                    sui_core::SemanticsActionRequest::Collapse => Some(false),
                    _ => None,
                };
                if let Some(open) = open {
                    self.open_position = None;
                    self.set_open(ctx, open);
                    if open {
                        ctx.request_focus();
                    }
                    ctx.set_handled();
                }
            }
            Event::Wake(WakeEvent::AnimationFrame { time, .. }) => {
                let surface_id = self.surface.child().id();
                let focus_surface_id = self.focus_surface.child().id();
                let mut state = self.surface_state.borrow_mut();
                let was_presented = state.is_presented();
                let was_focus_presented = state.focus_animation.is_presented();
                let previous = state.reveal.value;
                let previous_focus = state.focus_animation.value;
                let reveal_animating = state.reveal.advance(*time);
                let focus_animating = state.focus_animation.advance(*time);
                let reveal_changed = state.reveal.changed_since(previous);
                let focus_changed = state.focus_animation.changed_since(previous_focus);
                let is_presented = state.is_presented();
                let is_focus_presented = state.focus_animation.is_presented();
                drop(state);

                let previous_highlight = self.highlight_animation.value;
                let previous_press = self.press_animation.value;
                let row_animating = self.advance_row_animations(*time);
                let row_changed = self.highlight_animation.changed_since(previous_highlight)
                    || self.press_animation.changed_since(previous_press);
                if row_changed {
                    self.refresh_surface_interaction_state(ctx);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Paint);
                }

                if reveal_changed {
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Effect);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Transform);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Effect);
                }
                if focus_changed {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
                }
                if was_presented != is_presented {
                    ctx.request_measure();
                    request_child_invalidation(ctx, surface_id, InvalidationKind::Visibility);
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if was_focus_presented != is_focus_presented {
                    request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
                }
                if reveal_animating || row_animating || focus_animating {
                    ctx.request_animation_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let trigger_size = self.trigger.measure(ctx, constraints.loosen());
        if self.open {
            let theme = self.resolved_theme();
            let pointer_anchored = self.open_position.is_some();
            self.panels.clear();
            let mut prefix = Vec::new();
            while let Some(items) = self.items_at_prefix(&prefix).map(<[MenuItem]>::to_vec) {
                let mut width = self.measured_menu_width_for_items(ctx, &items);
                if prefix.is_empty() && !pointer_anchored {
                    width = width.max(trigger_size.width);
                }
                let height = themed_menu_height_for_rows(&theme, self.row_height(), items.len());
                self.panels.push(ContextMenuPanel {
                    prefix: prefix.clone(),
                    items: items.clone(),
                    frame_rect: Rect::from_origin_size(Point::ZERO, Size::new(width, height)),
                    opens_left: false,
                });
                if prefix.len() >= self.open_path.len() {
                    break;
                }
                let next = self.open_path[prefix.len()];
                if items
                    .get(next)
                    .is_none_or(|item| !item.enabled || !item.has_submenu())
                {
                    break;
                }
                prefix.push(next);
            }

            let estimated_width = self
                .panels
                .iter()
                .map(|panel| panel.frame_rect.width())
                .sum::<f32>();
            let estimated_height = self
                .panels
                .iter()
                .map(|panel| panel.frame_rect.height())
                .sum::<f32>();
            let estimated_size = Size::new(estimated_width, estimated_height);
            {
                let mut state = self.surface_state.borrow_mut();
                state.theme = theme;
                state.panels = self.panels.clone();
                state.highlighted = self.highlighted.clone();
                state.highlight_visual = self.highlight_visual.clone();
                state.pressed = self.pressed.clone();
                state.press_visual = self.press_visual.clone();
                state.highlight_animation = self.highlight_animation;
                state.press_animation = self.press_animation;
                state.surface_rect = Rect::from_origin_size(Point::ZERO, estimated_size);
                state.row_height = self.row_height();
            }
            self.surface
                .measure(ctx, Constraints::tight(estimated_size));
            self.focus_surface
                .measure(ctx, Constraints::tight(estimated_size));
        } else {
            self.panels.clear();
        }
        constraints.clamp(trigger_size)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.trigger.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.trigger.child().measured_size()),
        );
        if self.open && !self.panels.is_empty() {
            let theme = self.resolved_theme();
            let anchor = self.open_position.map_or_else(
                || self.trigger.child().bounds(),
                |position| {
                    Rect::from_origin_size(
                        Point::new(bounds.x() + position.x, bounds.y() + position.y),
                        Size::ZERO,
                    )
                },
            );
            let viewport = Rect::from_origin_size(Point::ZERO, ctx.dpi().viewport);
            let result = place_overlay(
                &OverlayPlacementRequest::new(
                    anchor,
                    self.panels[0].frame_rect.size,
                    viewport,
                    OverlayPlacement::BOTTOM_START,
                )
                .fallbacks([
                    OverlayPlacement::TOP_START,
                    OverlayPlacement::RIGHT_START,
                    OverlayPlacement::LEFT_START,
                ])
                .gap(if self.open_position.is_some() {
                    0.0
                } else {
                    theme.metrics.popover_gap
                })
                .margin(theme.metrics.popover_gap.max(4.0)),
            );
            self.panels[0].frame_rect = result
                .bounds
                .translate(Vector::new(-bounds.x(), -bounds.y()));

            for depth in 1..self.panels.len() {
                let owner_path = self.panels[depth].prefix.clone();
                let Some(anchor) = self.item_rect(bounds, &owner_path) else {
                    continue;
                };
                let margin = theme.metrics.popover_gap.max(4.0);
                let remaining_cascade_width = self.panels[depth..]
                    .iter()
                    .map(|panel| panel.frame_rect.width())
                    .sum::<f32>();
                let room_left = (anchor.x() - viewport.x() - margin).max(0.0);
                let room_right = (viewport.max_x() - anchor.max_x() - margin).max(0.0);
                let prefer_left = self.panels[depth - 1].opens_left
                    || (room_right < remaining_cascade_width
                        && room_left >= remaining_cascade_width);
                let (placement, fallbacks) = if prefer_left {
                    (
                        OverlayPlacement::LEFT_START,
                        [
                            OverlayPlacement::RIGHT_START,
                            OverlayPlacement::LEFT_END,
                            OverlayPlacement::RIGHT_END,
                        ],
                    )
                } else {
                    (
                        OverlayPlacement::RIGHT_START,
                        [
                            OverlayPlacement::LEFT_START,
                            OverlayPlacement::RIGHT_END,
                            OverlayPlacement::LEFT_END,
                        ],
                    )
                };
                let result = place_overlay(
                    &OverlayPlacementRequest::new(
                        anchor,
                        self.panels[depth].frame_rect.size,
                        viewport,
                        placement,
                    )
                    .fallbacks(fallbacks)
                    .gap(0.0)
                    .margin(margin),
                );
                self.panels[depth].opens_left = result.placement.side == OverlaySide::Left;
                self.panels[depth].frame_rect = result
                    .bounds
                    .translate(Vector::new(-bounds.x(), -bounds.y()));
            }
        }
        self.sync_surface_state(bounds);
        let state = self.surface_state.borrow();
        let surface_bounds = if state.is_presented() {
            state.surface_rect
        } else {
            Rect::from_origin_size(bounds.origin, Size::ZERO)
        };
        drop(state);
        self.surface.arrange(ctx, surface_bounds);
        self.focus_surface.arrange(ctx, surface_bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.trigger.paint(ctx);
        if self.surface_state.borrow().is_presented() {
            self.surface.paint(ctx);
            self.focus_surface.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::ContextMenu, ctx.bounds());
        node.name = Some(self.name.clone());
        node.state.focused = ctx.is_focused();
        node.state.expanded = Some(self.open);
        node.popup = Some(SemanticsPopupKind::Menu);
        node.value = self
            .highlighted
            .as_deref()
            .and_then(|path| self.item_at_path(path))
            .map(|item| SemanticsValue::Text(item.label.clone()));
        node.actions = vec![
            SemanticsAction::Focus,
            SemanticsAction::Expand,
            SemanticsAction::Collapse,
            SemanticsAction::Activate,
        ];
        ctx.push(node);
        if self.open {
            for panel in &self.panels {
                let parent = if panel.prefix.is_empty() {
                    ctx.widget_id()
                } else {
                    virtual_menu_item_path_id(ctx.widget_id(), &panel.prefix)
                };
                for (index, item) in panel.items.iter().enumerate() {
                    let mut path = panel.prefix.clone();
                    path.push(index);
                    let Some(row) = self.item_rect(ctx.bounds(), &path) else {
                        continue;
                    };
                    ctx.push(context_menu_item_semantics_node(
                        ctx.widget_id(),
                        parent,
                        &path,
                        item,
                        row,
                        self.highlighted.as_deref() == Some(path.as_slice()),
                        self.open_path.starts_with(&path),
                    ));
                }
            }
        }
        self.trigger.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (self.open || self.surface_state.borrow().is_presented()).then_some(
            OverlayOptions::new(OverlayKind::Menu)
                .dismiss(if self.open {
                    OverlayDismissPolicy::TRANSIENT
                } else {
                    OverlayDismissPolicy::NONE
                })
                .focus(OverlayFocusBehavior::NONE),
        )
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        if !focused && self.open {
            self.set_open(ctx, false);
        }
        let focus_surface_id = self.focus_surface.child().id();
        {
            let mut state = self.surface_state.borrow_mut();
            let was_focus_presented = state.focus_animation.is_presented();
            let theme = state.theme;
            set_focus_animation_target(
                &mut state.focus_animation,
                focused as u8 as f32,
                &theme,
                ctx,
            );
            if was_focus_presented != state.focus_animation.is_presented() {
                request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Visibility);
            }
        }
        request_child_invalidation(ctx, focus_surface_id, InvalidationKind::Paint);
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.trigger.visit_children(visitor);
        if self.surface_state.borrow().is_presented() {
            self.surface.visit_children(visitor);
            self.focus_surface.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.trigger.visit_children_mut(visitor);
        if self.surface_state.borrow().is_presented() {
            self.surface.visit_children_mut(visitor);
            self.focus_surface.visit_children_mut(visitor);
        }
    }
}
