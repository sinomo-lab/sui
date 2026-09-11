use crate::ControlMetrics;
use crate::DefaultTheme;
use crate::IconGlyph;
use crate::SemanticTone;
use crate::composites::forms::{
    set_focus_animation_target, set_hover_animation_target, set_press_animation_target,
};
use crate::composites::indicators::{
    draw_control_shape, inset_rect, measure_text, mix_color, numeric_text_style_if_numeric,
    physical_pixels, rounded_rect_path, semibold_control_text_style, text_token_style,
};
use crate::composites::painting::{EmptyStatePaint, paint_empty_state};
use crate::composites::popups::AnimatedScalar;
use crate::controls::draw_icon_glyph;
use crate::text_align::paint_aligned_text;
use crate::text_align::paint_aligned_text_contained;
use sui_core::Color;
use sui_core::Event;
use sui_core::KeyState;
use sui_core::Point;
use sui_core::PointerButton;
use sui_core::PointerEventKind;
use sui_core::Rect;
use sui_core::SemanticsAction;
use sui_core::SemanticsNode;
use sui_core::SemanticsRole;
use sui_core::SemanticsValue;
use sui_core::Size;
use sui_core::Vector;
use sui_core::WakeEvent;
use sui_core::WidgetId;
use sui_layout::Constraints;
use sui_runtime::ArrangeCtx;
use sui_runtime::EventCtx;
use sui_runtime::MeasureCtx;
use sui_runtime::PaintCtx;
use sui_runtime::SemanticsCtx;
use sui_runtime::SingleChild;
use sui_runtime::Widget;
use sui_runtime::WidgetPod;
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::StrokeStyle;
use sui_text::TextMeasurement;
use sui_text::TextStyle;

pub struct EmptyState {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: Option<String>,
    pub(super) icon: Option<IconGlyph>,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) detail: Option<String>,
    pub(super) action: Option<SingleChild>,
    pub(super) action_height: f32,
    pub(super) action_max_width: f32,
    pub(super) background: Option<Color>,
}

impl EmptyState {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            icon: None,
            title: title.into(),
            description: description.into(),
            detail: None,
            action: None,
            action_height: 32.0,
            action_max_width: 360.0,
            background: None,
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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn action<W>(mut self, action: W) -> Self
    where
        W: Widget + 'static,
    {
        self.action = Some(SingleChild::new(action));
        self
    }

    pub fn action_height(mut self, height: f32) -> Self {
        self.action_height = height.max(32.0);
        self
    }

    pub fn action_max_width(mut self, width: f32) -> Self {
        self.action_max_width = width.max(0.0);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn transparent(mut self) -> Self {
        self.background = Some(Color::TRANSPARENT);
        self
    }

    pub fn action_child(&self) -> Option<&WidgetPod> {
        self.action.as_ref().map(SingleChild::child)
    }

    pub fn action_child_mut(&mut self) -> Option<&mut WidgetPod> {
        self.action.as_mut().map(SingleChild::child_mut)
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(*self.theme)
    }

    pub(super) fn action_rect(
        bounds: Rect,
        action_size: Size,
        action_max_width: f32,
        action_height: f32,
    ) -> Rect {
        let max_width = action_max_width.min((bounds.width() - 32.0).max(0.0));
        let width = action_size.width.min(max_width).max(0.0);
        let height = action_size.height.max(action_height);
        let cx = bounds.x() + bounds.width() * 0.5;
        let cy = bounds.y() + bounds.height() * 0.5 - 18.0;
        Rect::new(cx - width * 0.5, cy + 54.0, width, height)
    }
}

impl Widget for EmptyState {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(self.action_max_width + 32.0)
        };
        let action_height = if let Some(action) = &mut self.action {
            let action_width = self.action_max_width.min((width - 32.0).max(0.0));
            let action_size = action.measure(
                ctx,
                Constraints::new(
                    Size::new(0.0, self.action_height),
                    Size::new(action_width, f32::INFINITY),
                ),
            );
            action_size.height.max(self.action_height)
        } else {
            0.0
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            constraints
                .min
                .height
                .max(142.0 + action_height + if action_height > 0.0 { 12.0 } else { 0.0 })
        };
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let action_max_width = self.action_max_width;
        let action_height = self.action_height;
        if let Some(action) = &mut self.action {
            let action_rect = Self::action_rect(
                bounds,
                action.child().measured_size(),
                action_max_width,
                action_height,
            );
            action.arrange(ctx, action_rect);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let bounds = ctx.bounds();
        let mut paint = EmptyStatePaint::new(&self.title, &self.description)
            .background(self.background.unwrap_or(theme.surfaces.window))
            .reserve_action_space(self.action.is_some());
        if let Some(icon) = self.icon {
            paint = paint.icon(icon);
        }
        if let Some(detail) = self.detail.as_deref() {
            paint = paint.detail(detail);
        }
        ctx.push_clip_rect(bounds);
        paint_empty_state(ctx, &theme, bounds, paint);

        if let Some(action) = &self.action {
            action.paint(ctx);
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone().unwrap_or_else(|| self.title.clone()));
        node.description = Some(match &self.detail {
            Some(detail)
                if matches!(
                    self.description.chars().last(),
                    Some('.') | Some('!') | Some('?')
                ) =>
            {
                format!("{} {}", self.description, detail)
            }
            Some(detail) => format!("{}. {}", self.description, detail),
            None => self.description.clone(),
        });
        ctx.push(node);
        if let Some(action) = &self.action {
            action.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(action) = &self.action {
            action.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(action) = &mut self.action {
            action.visit_children_mut(visitor);
        }
    }
}

pub struct PresetStrip {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) presets: Vec<String>,
    pub(super) selected: Option<usize>,
    pub(super) selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    pub(super) hovered: Option<usize>,
    pub(super) hover_visual: Option<usize>,
    pub(super) pressed: Option<usize>,
    pub(super) press_visual: Option<usize>,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) item_width: Option<f32>,
    pub(super) item_height: Option<f32>,
    pub(super) gap: Option<f32>,
    pub(super) label_measurements: Vec<TextMeasurement>,
    pub(super) item_widths: Vec<f32>,
    pub(super) on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl PresetStrip {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            presets: Vec::new(),
            selected: None,
            selected_reader: None,
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            item_width: None,
            item_height: None,
            gap: None,
            label_measurements: Vec::new(),
            item_widths: Vec::new(),
            on_change: None,
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

    pub fn preset(mut self, preset: impl Into<String>) -> Self {
        self.presets.push(preset.into());
        self
    }

    pub fn presets<I, S>(mut self, presets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.presets.extend(presets.into_iter().map(Into::into));
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_reader = None;
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn item_width(mut self, width: f32) -> Self {
        self.item_width = Some(width.max(0.0));
        self
    }

    pub fn item_height(mut self, height: f32) -> Self {
        self.item_height = Some(height.max(20.0));
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    pub(super) fn current_selected(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|selected| selected())
            .unwrap_or(self.selected)
            .filter(|index| *index < self.presets.len())
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_item_height(&self, metrics: ControlMetrics) -> f32 {
        self.item_height.unwrap_or(metrics.preset_strip_item_height)
    }

    pub(super) fn resolved_gap(&self, metrics: ControlMetrics) -> f32 {
        self.gap.unwrap_or(metrics.preset_strip_gap)
    }

    pub(super) fn item_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.presets.len() || self.item_widths.len() != self.presets.len() {
            return None;
        }

        let metrics = self.resolved_theme().metrics;
        let item_height = self.resolved_item_height(metrics);
        let gap = self.resolved_gap(metrics);
        let mut x = bounds.x();
        for (current, width) in self.item_widths.iter().enumerate() {
            let available = (bounds.max_x() - x).max(0.0);
            let rect = Rect::new(x, bounds.y(), width.min(available), item_height);
            if current == index {
                return (!rect.is_empty()).then_some(rect);
            }
            x += *width + gap;
        }

        None
    }

    pub(super) fn item_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.presets.iter().enumerate().find_map(|(index, _)| {
            self.item_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    pub(super) fn activate(&mut self, index: usize) {
        if self.presets.is_empty() {
            return;
        }

        let index = index.min(self.presets.len() - 1);
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, self.presets[index].clone());
        }
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        if self.presets.is_empty() {
            return;
        }

        let current = self.current_selected().unwrap_or(0) as isize;
        let last = self.presets.len() as isize - 1;
        let next = (current + delta).clamp(0, last) as usize;
        self.hovered = Some(next);
        self.activate(next);
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        self.current_selected()
            .and_then(|index| self.presets.get(index).cloned())
    }

    pub(super) fn set_hovered(&mut self, hovered: Option<usize>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(index) = hovered {
            self.hover_visual = Some(index);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
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

    pub(super) fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual == Some(index) {
            self.hover_animation.value
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
        let hover_animating = self.hover_animation.advance(time);
        if !hover_animating
            && self.hovered.is_none()
            && self.hover_animation.value <= AnimatedScalar::EPSILON
        {
            self.hover_visual = None;
        }

        let press_animating = self.press_animation.advance(time);
        if !press_animating
            && self.pressed.is_none()
            && self.press_animation.value <= AnimatedScalar::EPSILON
        {
            self.press_visual = None;
        }

        hover_animating | press_animating | self.focus_animation.advance(time)
    }
}

impl Widget for PresetStrip {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.item_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.item_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                if self.hovered.is_some() {
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.item_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index);
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.is_some() {
                    self.set_pressed(None, ctx);
                    self.set_hovered(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1),
                    "Home" => self.activate(0),
                    "End" if !self.presets.is_empty() => self.activate(self.presets.len() - 1),
                    "Enter" | " " => {
                        if let Some(selected) = self.current_selected().or(Some(0)) {
                            self.activate(selected);
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
        let metrics = theme.metrics;
        let item_height = self.resolved_item_height(metrics);
        let gap = self.resolved_gap(metrics);
        let style = theme.text_style(theme.palette.text);
        self.label_measurements = self
            .presets
            .iter()
            .map(|preset| measure_text(ctx, preset, &style))
            .collect();
        self.item_widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                self.item_width.unwrap_or(
                    (measurement.width
                        + metrics.preset_strip_item_padding.left
                        + metrics.preset_strip_item_padding.right)
                        .max(metrics.preset_strip_item_min_width),
                )
            })
            .collect();

        let width = self.item_widths.iter().sum::<f32>()
            + (gap * self.presets.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width, item_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let selected = self.current_selected();
        let style = theme.text_style(palette.text);

        if self.focus_animation.value > AnimatedScalar::EPSILON {
            ctx.stroke(
                rounded_rect_path(ctx.bounds().inflate(2.0, 2.0), metrics.corner_radius + 2.0),
                palette
                    .focus_ring
                    .with_alpha(palette.focus_ring.alpha * self.focus_animation.value),
                StrokeStyle::new(physical_pixels(ctx, metrics.focus_ring_width)),
            );
        }

        for (index, preset) in self.presets.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let is_selected = selected == Some(index);
            let is_hovered = self.hovered == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);
            let base_background = if is_selected {
                palette.selection
            } else {
                palette.surface
            };
            let hover_background = if hover_amount > 0.0 {
                mix_color(
                    base_background,
                    palette.control_hover,
                    interaction.hover_blend * if is_selected { 0.35 } else { 1.0 } * hover_amount,
                )
            } else {
                base_background
            };
            let background = if press_amount > 0.0 {
                mix_color(
                    hover_background,
                    palette.control_active,
                    interaction.pressed_blend * if is_selected { 0.45 } else { 1.0 } * press_amount,
                )
            } else {
                hover_background
            };
            let border = if is_selected {
                palette.selection_border
            } else if is_hovered || hover_amount > 0.0 || press_amount > 0.0 {
                palette.border_hover
            } else {
                palette.border
            };
            let text_color = palette.text;

            draw_control_shape(
                ctx,
                rect,
                metrics.corner_radius,
                physical_pixels(ctx, metrics.border_width),
                background,
                border,
            );

            let text_slot = inset_rect(rect, metrics.preset_strip_label_padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            let text_style = TextStyle {
                color: text_color,
                ..style.clone()
            };
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                preset,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.value = self.selected_text().map(SemanticsValue::Text);
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        let selected = self.current_selected();
        for (index, preset) in self.presets.iter().enumerate() {
            let Some(rect) = self.item_rect(ctx.bounds(), index) else {
                continue;
            };
            let mut item = SemanticsNode::new(
                preset_strip_item_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                rect,
            );
            item.parent = Some(ctx.widget_id());
            item.name = Some(preset.clone());
            item.value = Some(SemanticsValue::Text(preset.clone()));
            item.state.hovered = self.hovered == Some(index);
            item.state.selected = selected == Some(index);
            item.actions = vec![SemanticsAction::Activate];
            ctx.push(item);
        }
    }

    fn accepts_focus(&self) -> bool {
        !self.presets.is_empty()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        let theme = self.resolved_theme();
        set_focus_animation_target(&mut self.focus_animation, focused as u8 as f32, &theme, ctx);
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub(super) fn preset_strip_item_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 6_u64 << 50;
    const LOW_MASK: u64 = (1_u64 << 50) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(487)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

pub struct StatusBarSegment {
    pub(super) text: String,
    pub(super) reader: Option<Box<dyn Fn() -> String>>,
    pub(super) min_width: Option<f32>,
    pub(super) tone: SemanticTone,
    pub(super) expand: bool,
}

impl StatusBarSegment {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reader: None,
            min_width: None,
            tone: SemanticTone::Neutral,
            expand: false,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self {
            text: fallback.into(),
            reader: Some(Box::new(reader)),
            min_width: None,
            tone: SemanticTone::Neutral,
            expand: false,
        }
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }

    pub(super) fn text(&self) -> String {
        self.reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.text.clone())
    }
}

pub struct StatusBar {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) description_reader: Option<Box<dyn Fn() -> String>>,
    pub(super) height: Option<f32>,
    pub(super) segments: Vec<StatusBarSegment>,
    pub(super) measured_widths: Vec<f32>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: None,
            description: None,
            description_reader: None,
            height: None,
            segments: Vec::new(),
            measured_widths: Vec::new(),
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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self.description_reader = None;
        self
    }

    pub fn description_when<F>(mut self, description: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.description_reader = Some(Box::new(description));
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(18.0));
        self
    }

    pub fn segment(mut self, segment: StatusBarSegment) -> Self {
        self.segments.push(segment);
        self
    }

    pub fn text_segment(self, text: impl Into<String>) -> Self {
        self.segment(StatusBarSegment::new(text))
    }

    pub fn dynamic_segment<F>(self, fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.segment(StatusBarSegment::dynamic(fallback, reader))
    }

    pub(super) fn text_style(&self) -> TextStyle {
        let theme = self.resolved_theme();
        text_token_style(&theme, theme.text.xs, theme.palette.placeholder)
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn resolved_height(&self, metrics: ControlMetrics) -> f32 {
        self.height.unwrap_or(metrics.status_bar_height)
    }

    pub(super) fn description_text(&self) -> Option<String> {
        self.description_reader
            .as_ref()
            .map(|reader| reader())
            .or_else(|| self.description.clone())
    }

    pub(super) fn resolved_segment_min_width(
        segment: &StatusBarSegment,
        metrics: ControlMetrics,
    ) -> f32 {
        segment
            .min_width
            .unwrap_or(metrics.status_bar_segment_min_width)
    }

    pub(super) fn segment_widths(&self, metrics: ControlMetrics) -> Vec<f32> {
        if self.measured_widths.len() == self.segments.len() {
            self.measured_widths.clone()
        } else {
            self.segments
                .iter()
                .map(|segment| Self::resolved_segment_min_width(segment, metrics))
                .collect()
        }
    }

    pub(super) fn segment_rects(&self, bounds: Rect, metrics: ControlMetrics) -> Vec<Rect> {
        let mut widths = self.segment_widths(metrics);
        let expandable = self
            .segments
            .iter()
            .filter(|segment| segment.expand)
            .count();
        if expandable > 0 {
            let fixed: f32 = widths.iter().sum();
            let extra = (bounds.width() - fixed).max(0.0) / expandable as f32;
            for (index, segment) in self.segments.iter().enumerate() {
                if segment.expand {
                    widths[index] += extra;
                }
            }
        }

        let mut x = bounds.x();
        widths
            .into_iter()
            .map(|width| {
                let available = (bounds.max_x() - x).max(0.0);
                let rect = Rect::new(x, bounds.y(), width.min(available), bounds.height());
                x = rect.max_x();
                rect
            })
            .collect()
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StatusBarHost {
    pub(super) content: SingleChild,
    pub(super) status_bar: SingleChild,
}

impl StatusBarHost {
    pub fn new<C, S>(content: C, status_bar: S) -> Self
    where
        C: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            content: SingleChild::new(content),
            status_bar: SingleChild::new(status_bar),
        }
    }

    pub fn content(&self) -> &sui_runtime::WidgetPod {
        self.content.child()
    }

    pub fn content_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.content.child_mut()
    }

    pub fn status_bar(&self) -> &sui_runtime::WidgetPod {
        self.status_bar.child()
    }

    pub fn status_bar_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.status_bar.child_mut()
    }
}

impl Widget for StatusBarHost {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max = constraints.max;
        let status_size = self.status_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(max.width, max.height)),
        );
        let content_max_height = if max.height.is_finite() {
            (max.height - status_size.height).max(0.0)
        } else {
            f32::INFINITY
        };
        let content_size = self.content.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(max.width, content_max_height)),
        );

        constraints.clamp(Size::new(
            content_size.width.max(status_size.width),
            content_size.height + status_size.height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let status_height = self
            .status_bar
            .child()
            .measured_size()
            .height
            .min(bounds.height())
            .max(0.0);
        let content_height = (bounds.height() - status_height).max(0.0);

        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), bounds.width(), content_height),
        );
        self.status_bar.arrange(
            ctx,
            Rect::new(
                bounds.x(),
                bounds.y() + content_height,
                bounds.width(),
                status_height,
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        self.status_bar.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        self.status_bar.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.status_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.status_bar.visit_children_mut(visitor);
    }
}

impl Widget for StatusBar {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let text_style = self.text_style();
        self.measured_widths = self
            .segments
            .iter()
            .map(|segment| {
                let text = segment.text();
                let segment_style = numeric_text_style_if_numeric(&text, text_style.clone());
                let measured = measure_text(ctx, &text, &segment_style).width
                    + metrics.status_bar_segment_padding * 2.0;
                Self::resolved_segment_min_width(segment, metrics).max(measured.ceil())
            })
            .collect();
        let natural_width: f32 = self.measured_widths.iter().sum();
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                natural_width
            },
            self.resolved_height(metrics),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let bounds = ctx.bounds();
        ctx.fill_bounds(palette.surface);
        ctx.stroke_rect(
            Rect::new(bounds.x(), bounds.y(), bounds.width(), 1.0),
            palette.border,
            StrokeStyle::new(theme.metrics.border_width.max(1.0)),
        );

        let text_style = self.text_style();
        for (index, (segment, rect)) in self
            .segments
            .iter()
            .zip(self.segment_rects(bounds, metrics))
            .enumerate()
        {
            if rect.is_empty() {
                continue;
            }
            if index > 0 {
                let inset = metrics.status_bar_separator_inset.min(rect.height() * 0.5);
                ctx.stroke_rect(
                    Rect::new(
                        rect.x(),
                        rect.y() + inset,
                        1.0,
                        (rect.height() - inset * 2.0).max(0.0),
                    ),
                    palette.border.with_alpha(0.7),
                    StrokeStyle::new(1.0),
                );
            }
            let segment_text = segment.text();
            let segment_style = if segment.tone == SemanticTone::Neutral {
                text_style.clone()
            } else {
                let tone = theme.semantic_tone_color(segment.tone);
                let pill = Rect::new(
                    rect.x() + metrics.status_bar_separator_inset.min(rect.width() * 0.5),
                    rect.y() + metrics.status_bar_separator_inset.min(rect.height() * 0.5),
                    (rect.width() - metrics.status_bar_separator_inset * 2.0).max(0.0),
                    (rect.height() - metrics.status_bar_separator_inset * 2.0).max(0.0),
                );
                if !pill.is_empty() {
                    ctx.fill(
                        rounded_rect_path(pill, metrics.indicator_corner_radius),
                        tone.with_alpha(0.12),
                    );
                }
                TextStyle {
                    color: tone,
                    ..text_style.clone()
                }
            };
            let segment_style = numeric_text_style_if_numeric(&segment_text, segment_style);
            let content_rect = Rect::new(
                rect.x() + metrics.status_bar_segment_padding,
                rect.y(),
                (rect.width() - metrics.status_bar_segment_padding * 2.0).max(0.0),
                rect.height(),
            );
            ctx.push_clip_rect(content_rect);
            paint_aligned_text(
                ctx,
                content_rect,
                &segment_text,
                &segment_style,
                segment_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let theme = self.resolved_theme();
        let metrics = theme.metrics;
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = self.name.clone();
        if let Some(description) = self.description_text() {
            node.value = Some(SemanticsValue::Text(description.clone()));
            node.description = Some(description);
        }
        ctx.push(node);

        for (index, (segment, rect)) in self
            .segments
            .iter()
            .zip(self.segment_rects(ctx.bounds(), metrics))
            .enumerate()
        {
            let text = segment.text();
            let mut child = SemanticsNode::new(
                status_bar_segment_id(ctx.widget_id(), index),
                SemanticsRole::Text,
                rect,
            );
            child.parent = Some(ctx.widget_id());
            child.name = Some(text.clone());
            child.value = Some(SemanticsValue::Text(text));
            ctx.push(child);
        }
    }
}

pub(super) fn status_bar_segment_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 2_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(263)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

pub(super) type SegmentedControlChange = Box<dyn FnMut(usize, String)>;
pub(super) type SegmentedControlContextChange = Box<dyn FnMut(usize, String, &mut EventCtx)>;

pub(super) fn segmented_control_item_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;

    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(269)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

pub struct StatusBadge {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) label: String,
    pub(super) label_reader: Option<Box<dyn Fn() -> String>>,
    pub(super) icon: Option<IconGlyph>,
    pub(super) tone: SemanticTone,
    pub(super) tone_reader: Option<Box<dyn Fn() -> SemanticTone>>,
    pub(super) min_width: Option<f32>,
}

impl StatusBadge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            icon: None,
            tone: SemanticTone::Neutral,
            tone_reader: None,
            min_width: None,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self {
            label: fallback.into(),
            label_reader: Some(Box::new(reader)),
            ..Self::new("")
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

    pub fn icon(mut self, icon: IconGlyph) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn tone(mut self, tone: SemanticTone) -> Self {
        self.tone = tone;
        self.tone_reader = None;
        self
    }

    pub fn tone_when<F>(mut self, tone: F) -> Self
    where
        F: Fn() -> SemanticTone + 'static,
    {
        self.tone_reader = Some(Box::new(tone));
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width.max(0.0));
        self
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn label(&self) -> String {
        self.label_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.label.clone())
    }

    pub(super) fn resolved_tone(&self) -> SemanticTone {
        self.tone_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.tone)
    }

    pub(super) fn text_style(
        &self,
        theme: &DefaultTheme,
        label: &str,
        tone: SemanticTone,
    ) -> TextStyle {
        // Mesh badge label: contextual control text at 600 in the status ink
        // that reads on the soft wash.
        let (_, tone_ink) = theme.semantic_tone_soft_colors(tone);
        let style = semibold_control_text_style(theme, tone_ink);
        numeric_text_style_if_numeric(label, style)
    }

    pub(super) fn metrics(&self, theme: &DefaultTheme) -> (f32, f32, f32, f32) {
        let height = (theme.metrics.min_height - 2.0).max(22.0);
        let icon_size = (height - 13.0).clamp(11.0, 15.0);
        let gap = theme.metrics.icon_label_gap.max(4.0);
        let padding = theme.metrics.button_padding.left.max(8.0);
        (height, icon_size, gap, padding)
    }
}

pub fn paint_status_badge(
    ctx: &mut PaintCtx,
    rect: Rect,
    theme: &DefaultTheme,
    label: &str,
    icon: Option<IconGlyph>,
    tone: SemanticTone,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    // Mesh badge: soft status wash, no border, status-hued ink (`--sm-*-soft`
    // fill + `--sm-*-text` content) at the contextual control/600 label size,
    // r-1 corners.
    let (tone_soft, tone_ink) = theme.semantic_tone_soft_colors(tone);
    let icon_size = (rect.height() - 13.0).clamp(11.0, 15.0);
    let gap = theme.metrics.icon_label_gap.max(4.0);
    let padding = theme.metrics.button_padding.left.max(6.0);
    let radius = theme.radius.sm.min(rect.height() * 0.5);

    ctx.fill(rounded_rect_path(rect, radius), tone_soft);

    let mut x = rect.x() + padding.min(rect.width() * 0.5);
    if let Some(icon) = icon {
        let icon_rect = Rect::new(
            x,
            rect.y() + (rect.height() - icon_size) * 0.5,
            icon_size,
            icon_size,
        );
        draw_icon_glyph(ctx, icon, icon_rect, tone_ink);
        x = icon_rect.max_x() + gap;
    }
    let content_rect = Rect::new(
        x,
        rect.y(),
        (rect.max_x() - x - padding * 0.5).max(0.0),
        rect.height(),
    );
    if content_rect.width() <= 0.0 {
        return;
    }

    let style = semibold_control_text_style(theme, tone_ink);
    let style = numeric_text_style_if_numeric(label, style);
    ctx.push_clip_rect(content_rect);
    paint_aligned_text_contained(ctx, content_rect, label, &style, style.line_height, 0.0);
    ctx.pop_clip();
}
