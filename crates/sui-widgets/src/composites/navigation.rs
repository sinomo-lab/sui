use crate::DefaultTheme;
use crate::IconGlyph;
use crate::composites::forms::{
    set_focus_animation_target, set_hover_animation_target, set_press_animation_target,
};
use crate::composites::indicators::{
    draw_control_frame, draw_control_shape, draw_focus_ring_frame, inset_rect, measure_text,
    physical_pixels, rounded_rect_path, semibold_control_text_style, sliding_inset_rect,
    tab_indicator_rect, tab_panel_transition_translation, tab_state_visuals,
};
use crate::composites::popups::AnimatedScalar;
use crate::composites::status::{
    SegmentedControlChange, SegmentedControlContextChange, segmented_control_item_id,
};
use crate::controls::draw_icon_glyph;
use crate::text_align::paint_aligned_text;
use std::sync::Arc;
use sui_core::Event;
use sui_core::ImageHandle;
use sui_core::InvalidationKind;
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
use sui_layout::Padding as Insets;
use sui_reactive::Observable;
use sui_runtime::ArrangeCtx;
use sui_runtime::Command;
use sui_runtime::EventCtx;
use sui_runtime::MeasureCtx;
use sui_runtime::PaintCtx;
use sui_runtime::REACTIVE_CHANGED;
use sui_runtime::SemanticsCtx;
use sui_runtime::Widget;
use sui_runtime::WidgetChildren;
use sui_runtime::WidgetPodMutVisitor;
use sui_runtime::WidgetPodVisitor;
use sui_scene::ImageSource;
use sui_text::TextMeasurement;
use sui_text::TextStyle;

/// One navigation tab, measured and painted as a single optional-icon + label item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabBarItem {
    pub(super) label: String,
    pub(super) icon: Option<ImageHandle>,
}

impl TabBarItem {
    /// Create a text-only tab item.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    /// Add a leading registered image. The image is tinted like the label, and both share one
    /// centered content box and active indicator.
    pub fn icon(mut self, icon: ImageHandle) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Visible and accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Optional leading glyph.
    pub fn icon_handle(&self) -> Option<ImageHandle> {
        self.icon
    }
}

impl From<String> for TabBarItem {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}

impl From<&str> for TabBarItem {
    fn from(label: &str) -> Self {
        Self::new(label)
    }
}

pub struct TabBar {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) tabs: Vec<TabBarItem>,
    pub(super) selected: usize,
    pub(super) selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    pub(super) selected_source: Option<Arc<dyn Observable<Option<usize>>>>,
    pub(super) selection_from: usize,
    pub(super) selection_animation: AnimatedScalar,
    pub(super) hovered: Option<usize>,
    pub(super) hover_visual: Option<usize>,
    pub(super) pressed: Option<usize>,
    pub(super) press_visual: Option<usize>,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) gap: Option<f32>,
    pub(super) label_measurements: Vec<TextMeasurement>,
    pub(super) content_widths: Vec<f32>,
    pub(super) widths: Vec<f32>,
    pub(super) on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl TabBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            tabs: Vec::new(),
            selected: 0,
            selected_reader: None,
            selected_source: None,
            selection_from: 0,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            gap: None,
            label_measurements: Vec::new(),
            content_widths: Vec::new(),
            widths: Vec::new(),
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

    pub fn tab(mut self, label: impl Into<String>) -> Self {
        self.tabs.push(TabBarItem::new(label));
        self
    }

    pub fn tabs<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tabs.extend(labels.into_iter().map(TabBarItem::new));
        self
    }

    /// Append one icon-capable tab item.
    pub fn item(mut self, item: impl Into<TabBarItem>) -> Self {
        self.tabs.push(item.into());
        self
    }

    /// Append icon-capable tab items.
    pub fn items<I, T>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<TabBarItem>,
    {
        self.tabs.extend(items.into_iter().map(Into::into));
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self.selected_reader = None;
        self.selected_source = None;
        self.selection_from = index;
        self.selection_animation = AnimatedScalar::new(1.0);
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        if let Some(index) = selected() {
            self.selected = index;
            self.selection_from = index;
        }
        self.selected_reader = Some(Box::new(selected));
        self.selected_source = None;
        self
    }

    /// Bind the selected tab to an observable value without rebuilding the
    /// retained tab bar.
    pub fn selected_from<O>(mut self, selected: O) -> Self
    where
        O: Observable<Option<usize>> + 'static,
    {
        if let Some(index) = selected.get() {
            self.selected = index;
            self.selection_from = index;
        }
        self.selected_reader = None;
        self.selected_source = Some(Arc::new(selected));
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

    pub fn selected_index(&self) -> usize {
        self.normalized_selected()
    }

    pub fn current_tab(&self) -> Option<&str> {
        self.tabs
            .get(self.normalized_selected())
            .map(TabBarItem::label)
    }

    pub(super) fn normalized_selected(&self) -> usize {
        let selected = self
            .selected_source
            .as_ref()
            .and_then(|source| source.get())
            .or_else(|| self.selected_reader.as_ref().and_then(|reader| reader()))
            .unwrap_or(self.selected);
        if self.tabs.is_empty() {
            0
        } else {
            selected.min(self.tabs.len() - 1)
        }
    }

    pub(super) fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        if self.tabs.is_empty() {
            return;
        }

        let index = index.min(self.tabs.len() - 1);
        let selected = self.normalized_selected();
        if selected != index {
            self.selected = index;
            if let Some(on_change) = &mut self.on_change {
                on_change(index, self.tabs[index].label.clone());
            }
            let target = self.normalized_selected();
            self.selected = target;
            self.start_selection_animation(selected, target, ctx);
        }
    }

    pub(super) fn start_selection_animation(&mut self, from: usize, to: usize, ctx: &mut EventCtx) {
        if self.tabs.is_empty() || from == to {
            self.selection_from = to;
            self.selection_animation = AnimatedScalar::new(1.0);
            return;
        }

        let theme = self.resolved_theme();
        self.selection_from = from.min(self.tabs.len() - 1);
        self.selection_animation = AnimatedScalar::new(0.0);
        self.selection_animation.set_target_event(
            1.0,
            theme.motion.tab_switch_duration(),
            theme.motion.tab_switch_easing(),
            ctx,
        );
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn sync_external_selected(&mut self, ctx: &mut EventCtx) {
        if (self.selected_reader.is_none() && self.selected_source.is_none())
            || self.tabs.is_empty()
        {
            return;
        }

        if let Some(source) = &self.selected_source {
            let _ = ctx.observe(source.as_ref(), InvalidationKind::Paint);
            let _ = ctx.observe(source.as_ref(), InvalidationKind::Semantics);
        }

        let previous = self.selected.min(self.tabs.len() - 1);
        let selected = self.normalized_selected();
        if previous != selected {
            self.selected = selected;
            self.start_selection_animation(previous, selected, ctx);
        }
    }

    pub(super) fn tab_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    pub(super) fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.tab_gap)
            .max(0.0)
    }

    pub(super) fn measured_widths(&self) -> &[f32] {
        &self.widths
    }

    pub(super) fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() || self.measured_widths().len() != self.tabs.len() {
            return None;
        }

        let gap = self.resolved_gap();
        let base_total =
            self.widths.iter().sum::<f32>() + (gap * self.tabs.len().saturating_sub(1) as f32);
        let extra_per_tab = if bounds.width() > base_total && !self.tabs.is_empty() {
            (bounds.width() - base_total) / self.tabs.len() as f32
        } else {
            0.0
        };

        let tab_height = self.tab_height().min(bounds.height()).max(0.0);
        let tab_y = bounds.y() + ((bounds.height() - tab_height) * 0.5).max(0.0);
        let mut x = bounds.x();
        for (current, width) in self.widths.iter().enumerate() {
            let width = *width + extra_per_tab;
            let rect = Rect::new(x, tab_y, width, tab_height);
            if current == index {
                return Some(rect);
            }
            x += width + gap;
        }

        None
    }

    pub(super) fn tab_content_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        let tab = self.tab_rect(bounds, index)?;
        let width = *self.content_widths.get(index)?;
        let slot = inset_rect(tab, self.resolved_theme().metrics.tab_padding);
        let width = width.min(slot.width()).max(0.0);
        Some(Rect::new(
            slot.x() + ((slot.width() - width) * 0.5).max(0.0),
            slot.y(),
            width,
            slot.height(),
        ))
    }

    pub(super) fn tab_indicator_anchor_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        let tab = self.tab_rect(bounds, index)?;
        if self.tabs.get(index)?.icon.is_some() {
            let content = self.tab_content_rect(bounds, index)?;
            Some(Rect::new(
                content.x(),
                tab.y(),
                content.width(),
                tab.height(),
            ))
        } else {
            let padding = self.resolved_theme().metrics.tab_padding;
            Some(Rect::new(
                tab.x() + padding.left,
                tab.y(),
                (tab.width() - padding.left - padding.right).max(0.0),
                tab.height(),
            ))
        }
    }

    pub(super) fn tab_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.tabs.iter().enumerate().find_map(|(index, _)| {
            self.tab_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    pub(super) fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.tabs.is_empty() {
            return;
        }

        let selected = self.normalized_selected() as isize;
        let last = self.tabs.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next, ctx);
        self.set_hovered(Some(next), ctx);
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
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
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
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
}

impl Widget for TabBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.sync_external_selected(ctx);
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.tab_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index, ctx);
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
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.activate(0, ctx),
                    "End" if !self.tabs.is_empty() => self.activate(self.tabs.len() - 1, ctx),
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

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.is(REACTIVE_CHANGED) && self.selected_source.is_some() {
            self.sync_external_selected(ctx);
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(source) = &self.selected_source {
            let selected = ctx.observe_with(source.as_ref(), InvalidationKind::Paint);
            let _ = ctx.observe_with(source.as_ref(), InvalidationKind::Semantics);
            if let Some(selected) = selected {
                self.selected = selected;
            }
        }
        let theme = self.resolved_theme();
        let style = theme.text_style(theme.palette.text);
        let padding = theme.metrics.tab_padding;
        self.label_measurements = self
            .tabs
            .iter()
            .map(|tab| measure_text(ctx, &tab.label, &style))
            .collect();
        let icon_size = theme
            .metrics
            .icon_size
            .min((self.tab_height() - padding.top - padding.bottom).max(0.0));
        self.content_widths = self
            .tabs
            .iter()
            .zip(self.label_measurements.iter())
            .map(|(tab, measurement)| {
                measurement.width
                    + tab
                        .icon
                        .map(|_| icon_size + theme.metrics.icon_label_gap)
                        .unwrap_or_default()
            })
            .collect();
        self.widths = self
            .content_widths
            .iter()
            .map(|content_width| {
                (content_width + padding.left + padding.right).max(theme.metrics.tab_min_width)
            })
            .collect();

        let gap = self.resolved_gap();
        let width =
            self.widths.iter().sum::<f32>() + (gap * self.tabs.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width.max(160.0), self.tab_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let tab_padding = metrics.tab_padding;
        let label_style = theme.text_style(palette.text_muted);
        let selected_label_style = theme.text_style(palette.text);

        // Navigation tabs share one flat strip. Selection is communicated by the
        // animated underline below rather than by a second, raised tile.
        ctx.fill_rect(ctx.bounds(), palette.control);
        let divider_height = physical_pixels(ctx, metrics.border_width);
        ctx.fill_rect(
            Rect::new(
                ctx.bounds().x(),
                ctx.bounds().max_y() - divider_height,
                ctx.bounds().width(),
                divider_height,
            ),
            palette.border.with_alpha(0.72),
        );

        let focus_progress = self.focus_animation.value;
        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            // Hover and press remain local to each tab, but the steady selected
            // state stays flat so it does not compete with the underline.
            if (hovered
                || pressed
                || hover_amount > AnimatedScalar::EPSILON
                || press_amount > AnimatedScalar::EPSILON)
                && let Some((background, border)) =
                    tab_state_visuals(&theme, false, hovered, pressed, hover_amount, press_amount)
            {
                draw_control_shape(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = if selected {
                selected_label_style.clone()
            } else {
                label_style.clone()
            };
            let text_slot = inset_rect(rect, tab_padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            let Some(content) = self.tab_content_rect(ctx.bounds(), index) else {
                continue;
            };
            let content = content.translate(Vector::new(0.0, pressed_offset));
            ctx.push_clip_rect(text_slot);
            let label_slot = if let Some(icon) = tab.icon {
                let icon_size = metrics.icon_size.min(content.height());
                let icon_rect = Rect::new(
                    content.x(),
                    content.y() + (content.height() - icon_size) * 0.5,
                    icon_size,
                    icon_size,
                );
                ctx.draw_image_source(
                    icon_rect,
                    ImageSource::new(icon).with_tint(text_style.color),
                );
                Rect::new(
                    icon_rect.max_x() + metrics.icon_label_gap,
                    content.y(),
                    (content.max_x() - icon_rect.max_x() - metrics.icon_label_gap).max(0.0),
                    content.height(),
                )
            } else {
                content
            };
            paint_aligned_text(
                ctx,
                label_slot,
                &tab.label,
                &text_style,
                text_style.line_height,
                0.0,
            );
            ctx.pop_clip();
        }

        if let Some(accent) = tab_indicator_rect(
            |index| self.tab_indicator_anchor_rect(ctx.bounds(), index),
            self.selection_from,
            self.normalized_selected(),
            self.selection_animation.value,
            Insets::ZERO,
            interaction.active_indicator_thickness,
        ) {
            ctx.fill(
                rounded_rect_path(accent, accent.height() * 0.5),
                palette.accent,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TabBar, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .current_tab()
            .map(|value| SemanticsValue::Text(value.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BrowserTabHit {
    Tab(usize),
    Close(usize),
}

impl BrowserTabHit {
    pub(super) fn index(self) -> usize {
        match self {
            Self::Tab(index) | Self::Close(index) => index,
        }
    }
}

pub(super) type BrowserTabBarChange = Box<dyn FnMut(usize, String)>;
pub(super) type BrowserTabBarContextChange = Box<dyn FnMut(usize, String, &mut EventCtx)>;

pub struct BrowserTabBar {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) tabs: Vec<String>,
    pub(super) tabs_reader: Option<Box<dyn Fn() -> Vec<String>>>,
    pub(super) selected: Option<usize>,
    pub(super) selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    pub(super) selection_from: Option<usize>,
    pub(super) selection_to: Option<usize>,
    pub(super) selection_animation: AnimatedScalar,
    pub(super) hovered: Option<BrowserTabHit>,
    pub(super) hover_visual: Option<BrowserTabHit>,
    pub(super) pressed: Option<BrowserTabHit>,
    pub(super) press_visual: Option<BrowserTabHit>,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) label_measurements: Vec<TextMeasurement>,
    pub(super) widths: Vec<f32>,
    pub(super) on_change: Option<BrowserTabBarChange>,
    pub(super) on_change_with_ctx: Option<BrowserTabBarContextChange>,
    pub(super) on_close: Option<BrowserTabBarChange>,
    pub(super) on_close_with_ctx: Option<BrowserTabBarContextChange>,
}

impl BrowserTabBar {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            tabs: Vec::new(),
            tabs_reader: None,
            selected: None,
            selected_reader: None,
            selection_from: None,
            selection_to: None,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
            widths: Vec::new(),
            on_change: None,
            on_change_with_ctx: None,
            on_close: None,
            on_close_with_ctx: None,
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

    pub fn tabs<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tabs.extend(labels.into_iter().map(Into::into));
        self
    }

    pub fn tabs_when<F>(mut self, tabs: F) -> Self
    where
        F: Fn() -> Vec<String> + 'static,
    {
        self.tabs_reader = Some(Box::new(tabs));
        self
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self.selection_from = index;
        self.selection_to = index;
        self.selection_animation = AnimatedScalar::new(1.0);
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String, &mut EventCtx) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    pub fn on_close<F>(mut self, on_close: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_close = Some(Box::new(on_close));
        self
    }

    pub fn on_close_with_ctx<F>(mut self, on_close: F) -> Self
    where
        F: FnMut(usize, String, &mut EventCtx) + 'static,
    {
        self.on_close_with_ctx = Some(Box::new(on_close));
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.normalized_selected()
    }

    pub(super) fn refresh_tabs(&mut self) {
        if let Some(reader) = &self.tabs_reader {
            self.tabs = reader();
        }
        self.selected = self.resolved_selected_raw();
        let selected = self.normalized_selected();
        if self.selection_animation.value >= 1.0 - AnimatedScalar::EPSILON
            && self.selection_to != selected
        {
            self.selection_from = selected;
            self.selection_to = selected;
            self.selection_animation = AnimatedScalar::new(1.0);
        }
    }

    pub(super) fn resolved_selected_raw(&self) -> Option<usize> {
        self.selected_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.selected)
    }

    pub(super) fn normalized_selected(&self) -> Option<usize> {
        let selected = self.resolved_selected_raw()?;
        (selected < self.tabs.len()).then_some(selected)
    }

    pub(super) fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        self.refresh_tabs();
        if index >= self.tabs.len() {
            return;
        }
        let from = self.normalized_selected();
        if from == Some(index) {
            return;
        }
        let label = self.tabs[index].clone();
        self.selected = Some(index);
        if let Some(on_change) = &mut self.on_change {
            on_change(index, label.clone());
        }
        if let Some(on_change) = &mut self.on_change_with_ctx {
            on_change(index, label, ctx);
        }
        self.refresh_tabs();
        self.start_selection_animation(from, self.normalized_selected(), ctx);
    }

    pub(super) fn close(&mut self, index: usize, ctx: &mut EventCtx) {
        self.refresh_tabs();
        if index >= self.tabs.len() {
            return;
        }
        let from = self.normalized_selected();
        let label = self.tabs[index].clone();
        if let Some(on_close) = &mut self.on_close {
            on_close(index, label.clone());
        }
        if let Some(on_close) = &mut self.on_close_with_ctx {
            on_close(index, label, ctx);
        }
        self.refresh_tabs();
        self.start_selection_animation(from, self.normalized_selected(), ctx);
    }

    pub(super) fn start_selection_animation(
        &mut self,
        from: Option<usize>,
        to: Option<usize>,
        ctx: &mut EventCtx,
    ) {
        self.selection_from = from.or(to);
        self.selection_to = to;
        if from.zip(to).is_some_and(|(from, to)| from != to) {
            let theme = self.resolved_theme();
            self.selection_animation = AnimatedScalar::new(0.0);
            self.selection_animation.set_target_event(
                1.0,
                theme.motion.tab_switch_duration(),
                theme.motion.tab_switch_easing(),
                ctx,
            );
        } else {
            self.selection_animation = AnimatedScalar::new(1.0);
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn close_size(theme: &DefaultTheme) -> f32 {
        (theme.metrics.tab_height * 0.56).clamp(16.0, 20.0)
    }

    pub(super) fn close_gap(theme: &DefaultTheme) -> f32 {
        theme.metrics.icon_label_gap.max(7.0)
    }

    pub(super) fn tab_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    pub(super) fn measured_widths(&self) -> &[f32] {
        &self.widths
    }

    pub(super) fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() || self.measured_widths().len() != self.tabs.len() {
            return None;
        }

        let theme = self.resolved_theme();
        let gap = theme.metrics.tab_gap;
        let tab_height = self.tab_height().min(bounds.height()).max(0.0);
        let tab_y = bounds.y() + ((bounds.height() - tab_height) * 0.5).max(0.0);
        let mut x = bounds.x();
        for (current, measured_width) in self.widths.iter().enumerate() {
            let visible_width = (*measured_width).min((bounds.max_x() - x).max(0.0));
            let rect = Rect::new(x, tab_y, visible_width, tab_height);
            if current == index {
                return (visible_width > 0.0).then_some(rect);
            }
            x += *measured_width + gap;
            if x >= bounds.max_x() {
                break;
            }
        }

        None
    }

    pub(super) fn close_rect_for(&self, tab_rect: Rect) -> Rect {
        let theme = self.resolved_theme();
        let close = Self::close_size(&theme)
            .min(tab_rect.width())
            .min(tab_rect.height());
        Rect::new(
            tab_rect.max_x() - close - Self::close_gap(&theme),
            tab_rect.y() + ((tab_rect.height() - close) * 0.5),
            close,
            close,
        )
    }

    pub(super) fn label_rect_for(&self, tab_rect: Rect) -> Rect {
        let theme = self.resolved_theme();
        let padding = theme.metrics.tab_padding;
        let close = self.close_rect_for(tab_rect);
        Rect::new(
            tab_rect.x() + padding.left,
            tab_rect.y() + padding.top,
            (close.x() - tab_rect.x() - padding.left - Self::close_gap(&theme)).max(0.0),
            (tab_rect.height() - padding.top - padding.bottom).max(0.0),
        )
    }

    pub(super) fn hit_at(&self, bounds: Rect, position: Point) -> Option<BrowserTabHit> {
        for index in 0..self.tabs.len() {
            let Some(rect) = self.tab_rect(bounds, index) else {
                continue;
            };
            if self.close_rect_for(rect).contains(position) {
                return Some(BrowserTabHit::Close(index));
            }
            if rect.contains(position) {
                return Some(BrowserTabHit::Tab(index));
            }
        }
        None
    }

    pub(super) fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        self.refresh_tabs();
        if self.tabs.is_empty() {
            return;
        }
        let selected = self.normalized_selected().unwrap_or(0) as isize;
        let last = self.tabs.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next, ctx);
        self.set_hovered(Some(BrowserTabHit::Tab(next)), ctx);
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
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
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }

    pub(super) fn set_hovered(&mut self, hovered: Option<BrowserTabHit>, ctx: &mut EventCtx) {
        if self.hovered == hovered {
            return;
        }
        let theme = self.resolved_theme();
        self.hovered = hovered;
        if let Some(hit) = hovered {
            self.hover_visual = Some(hit);
            self.hover_animation = AnimatedScalar::new(0.0);
            set_hover_animation_target(&mut self.hover_animation, 1.0, &theme, ctx);
        } else if !set_hover_animation_target(&mut self.hover_animation, 0.0, &theme, ctx) {
            self.hover_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn set_pressed(&mut self, pressed: Option<BrowserTabHit>, ctx: &mut EventCtx) {
        if self.pressed == pressed {
            return;
        }
        let theme = self.resolved_theme();
        self.pressed = pressed;
        if let Some(hit) = pressed {
            self.press_visual = Some(hit);
            self.press_animation = AnimatedScalar::new(0.0);
            set_press_animation_target(&mut self.press_animation, 1.0, &theme, ctx);
        } else if !set_press_animation_target(&mut self.press_animation, 0.0, &theme, ctx) {
            self.press_visual = None;
        }
        ctx.request_paint();
        ctx.request_semantics();
    }

    pub(super) fn hover_amount_for(&self, index: usize) -> f32 {
        if self.hover_visual.is_some_and(|hit| hit.index() == index) {
            self.hover_animation.value
        } else {
            0.0
        }
    }

    pub(super) fn press_amount_for(&self, index: usize) -> f32 {
        if self.press_visual.is_some_and(|hit| hit.index() == index) {
            self.press_animation.value
        } else {
            0.0
        }
    }
}

impl Widget for BrowserTabBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.refresh_tabs();
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.hit_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.hit_at(ctx.bounds(), pointer.position);
                if let Some(hit) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(hit, _)| hit)
                {
                    match hit {
                        BrowserTabHit::Tab(index) => self.activate(index, ctx),
                        BrowserTabHit::Close(index) => self.close(index, ctx),
                    }
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
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.activate(0, ctx),
                    "End" if !self.tabs.is_empty() => self.activate(self.tabs.len() - 1, ctx),
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
        self.refresh_tabs();
        let theme = self.resolved_theme();
        let style = theme.text_style(theme.palette.text);
        let padding = theme.metrics.tab_padding;
        let close_extent = Self::close_size(&theme) + (Self::close_gap(&theme) * 2.0);
        self.label_measurements = self
            .tabs
            .iter()
            .map(|tab| measure_text(ctx, tab, &style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                (measurement.width + padding.left + padding.right + close_extent)
                    .max(theme.metrics.tab_min_width)
            })
            .collect();

        let gap = theme.metrics.tab_gap;
        let width =
            self.widths.iter().sum::<f32>() + (gap * self.tabs.len().saturating_sub(1) as f32);
        constraints.clamp(Size::new(width, self.tab_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let interaction = theme.interaction;
        let selected_index = self.normalized_selected();
        let focus_progress = self.focus_animation.value;

        let clip_outset = physical_pixels(
            ctx,
            theme.metrics.focus_ring_outset + (theme.metrics.focus_ring_width * 0.5),
        );
        ctx.push_clip_rect(ctx.bounds().inflate(clip_outset, clip_outset));
        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = selected_index == Some(index);
            let hovered = self.hovered.is_some_and(|hit| hit.index() == index);
            let pressed = self.pressed.is_some_and(|hit| hit.index() == index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if let Some((background, border)) = tab_state_visuals(
                &theme,
                selected,
                hovered,
                pressed,
                hover_amount,
                press_amount,
            ) {
                draw_control_shape(
                    ctx,
                    rect,
                    theme.metrics.corner_radius,
                    physical_pixels(ctx, theme.metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect,
                    theme.metrics.corner_radius,
                    theme.metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = theme.text_style(if selected {
                palette.text
            } else {
                palette.text_muted
            });
            let text_slot = self.label_rect_for(rect);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                tab,
                &text_style,
                text_style.line_height,
                0.0,
            );
            ctx.pop_clip();

            let close = self.close_rect_for(rect);
            let close_hovered = self.hovered == Some(BrowserTabHit::Close(index));
            let close_pressed = self.pressed == Some(BrowserTabHit::Close(index));
            if close_hovered || close_pressed {
                ctx.fill(
                    rounded_rect_path(close, theme.metrics.corner_radius.min(5.0)),
                    if close_pressed {
                        palette.control_active
                    } else {
                        palette.control_hover
                    },
                );
            }
            draw_icon_glyph(
                ctx,
                IconGlyph::Close,
                close.inflate(-3.0, -3.0),
                if close_hovered || selected {
                    palette.text
                } else {
                    palette.placeholder
                }
                .with_alpha(if close_pressed { 0.95 } else { 0.78 }),
            );
        }

        if let Some(selected) = selected_index {
            let progress = if self.selection_to == Some(selected) {
                self.selection_animation.value
            } else {
                1.0
            };
            if let Some(accent) = tab_indicator_rect(
                |index| self.tab_rect(ctx.bounds(), index),
                self.selection_from.unwrap_or(selected),
                selected,
                progress,
                theme.metrics.tab_padding,
                interaction.active_indicator_thickness,
            ) {
                ctx.fill(
                    rounded_rect_path(accent, accent.height() * 0.5),
                    palette.accent,
                );
            }
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::TabBar, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .normalized_selected()
            .and_then(|index| self.tabs.get(index))
            .map(|value| SemanticsValue::Text(value.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);

        for (index, tab) in self.tabs.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let tab_id = browser_tab_semantics_id(ctx.widget_id(), index);
            let mut tab_node = SemanticsNode::new(tab_id, SemanticsRole::Button, rect);
            tab_node.parent = Some(ctx.widget_id());
            tab_node.name = Some(tab.clone());
            tab_node.state.selected = self.normalized_selected() == Some(index);
            tab_node.state.hovered = self.hovered.is_some_and(|hit| hit.index() == index);
            tab_node.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
            ctx.push(tab_node);

            let mut close_node = SemanticsNode::new(
                browser_tab_close_semantics_id(ctx.widget_id(), index),
                SemanticsRole::Button,
                self.close_rect_for(rect),
            );
            close_node.parent = Some(tab_id);
            close_node.name = Some(format!("Close {tab} tab"));
            close_node.state.hovered = self.hovered == Some(BrowserTabHit::Close(index));
            close_node.actions = vec![SemanticsAction::Activate, SemanticsAction::Focus];
            ctx.push(close_node);
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

pub(super) fn browser_tab_semantics_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(397)
            .wrapping_add(index as u64 + 1)
            & LOW_MASK),
    )
}

pub(super) fn browser_tab_close_semantics_id(parent: WidgetId, index: usize) -> WidgetId {
    const TAG: u64 = 3_u64 << 51;
    const LOW_MASK: u64 = (1_u64 << 51) - 1;
    WidgetId::new(
        TAG | (parent
            .get()
            .wrapping_mul(397)
            .wrapping_add(10_000 + index as u64)
            & LOW_MASK),
    )
}

pub struct SegmentedControl {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) segments: Vec<SegmentedControlItem>,
    pub(super) selected: usize,
    pub(super) selected_reader: Option<Box<dyn Fn() -> Option<usize>>>,
    pub(super) selection_from: usize,
    pub(super) selection_animation: AnimatedScalar,
    pub(super) hovered: Option<usize>,
    pub(super) hover_visual: Option<usize>,
    pub(super) pressed: Option<usize>,
    pub(super) press_visual: Option<usize>,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) label_measurements: Vec<TextMeasurement>,
    pub(super) on_change: Option<SegmentedControlChange>,
    pub(super) on_change_with_ctx: Option<SegmentedControlContextChange>,
}

pub struct SegmentedControlItem {
    pub(super) label: String,
    pub(super) semantic_name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) disabled: bool,
}

impl SegmentedControlItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            semantic_name: None,
            description: None,
            disabled: false,
        }
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

impl SegmentedControl {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            segments: Vec::new(),
            selected: 0,
            selected_reader: None,
            selection_from: 0,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
            on_change: None,
            on_change_with_ctx: None,
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

    pub fn item(mut self, item: SegmentedControlItem) -> Self {
        self.segments.push(item);
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = SegmentedControlItem>,
    {
        self.segments.extend(items);
        self
    }

    pub fn segments<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.segments
            .extend(labels.into_iter().map(SegmentedControlItem::new));
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self.selected_reader = None;
        self.selection_from = index;
        self.selection_animation = AnimatedScalar::new(1.0);
        self
    }

    pub fn selected_when<F>(mut self, selected: F) -> Self
    where
        F: Fn() -> Option<usize> + 'static,
    {
        self.selected_reader = Some(Box::new(selected));
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn on_change_with_ctx<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String, &mut EventCtx) + 'static,
    {
        self.on_change_with_ctx = Some(Box::new(on_change));
        self
    }

    pub fn selected_index(&self) -> usize {
        self.normalized_selected()
    }

    pub(super) fn normalized_selected(&self) -> usize {
        let selected = self
            .selected_reader
            .as_ref()
            .and_then(|reader| reader())
            .unwrap_or(self.selected);
        if self.segments.is_empty() {
            0
        } else {
            selected.min(self.segments.len() - 1)
        }
    }

    pub(super) fn segment_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    pub(super) fn segment_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.segments.len() {
            return None;
        }
        let count = self.segments.len().max(1);
        let width = bounds.width() / count as f32;
        let x = bounds.x() + width * index as f32;
        let width = if index + 1 == count {
            bounds.max_x() - x
        } else {
            width
        };
        Some(Rect::new(
            x,
            bounds.y(),
            width.max(0.0),
            bounds.height().max(0.0),
        ))
    }

    pub(super) fn segment_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        if !bounds.contains(position) || self.segments.is_empty() {
            return None;
        }
        let slot_width = (bounds.width() / self.segments.len() as f32).max(1.0);
        let index = ((position.x - bounds.x()) / slot_width).floor() as usize;
        Some(index.min(self.segments.len() - 1))
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    pub(super) fn activate(&mut self, index: usize, ctx: &mut EventCtx) {
        let Some(segment) = self.segments.get(index) else {
            return;
        };
        if segment.disabled {
            return;
        }

        let index = index.min(self.segments.len() - 1);
        let selected = self.normalized_selected();
        if selected != index {
            let theme = self.resolved_theme();
            self.selection_from = selected;
            self.selected = index;
            self.selection_animation = AnimatedScalar::new(0.0);
            self.selection_animation.set_target_event(
                1.0,
                theme.motion.tab_switch_duration(),
                theme.motion.tab_switch_easing(),
                ctx,
            );
            let label = self.segments[index].label.clone();
            if let Some(on_change) = &mut self.on_change {
                on_change(index, label.clone());
            }
            if let Some(on_change) = &mut self.on_change_with_ctx {
                on_change(index, label, ctx);
            }
        }
    }

    pub(super) fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.segments.is_empty() {
            return;
        }
        let selected = self.normalized_selected() as isize;
        let last = self.segments.len() as isize - 1;
        let next = (selected + delta).clamp(0, last) as usize;
        self.activate(next, ctx);
        self.set_hovered(Some(next), ctx);
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
        let selection_animating = self.selection_animation.advance(time);
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
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }
}

impl Widget for SegmentedControl {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.segment_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Leave
                    || pointer.kind == PointerEventKind::Cancel =>
            {
                if pointer.kind == PointerEventKind::Cancel && self.pressed.is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
                self.set_pressed(None, ctx);
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.segment_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.segment_at(ctx.bounds(), pointer.position);
                if let Some(index) = self
                    .pressed
                    .zip(hovered)
                    .filter(|(left, right)| left == right)
                    .map(|(index, _)| index)
                {
                    self.activate(index, ctx);
                }
                self.set_hovered(hovered, ctx);
                self.set_pressed(None, ctx);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                match key.key.as_str() {
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.activate(0, ctx),
                    "End" if !self.segments.is_empty() => {
                        self.activate(self.segments.len() - 1, ctx)
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
        let style = semibold_control_text_style(&theme, theme.palette.text);
        let padding = theme.metrics.tab_padding;
        self.label_measurements = self
            .segments
            .iter()
            .map(|segment| measure_text(ctx, &segment.label, &style))
            .collect();
        let widest = self
            .label_measurements
            .iter()
            .map(|measurement| measurement.width + padding.left + padding.right)
            .fold(theme.metrics.tab_min_width, f32::max);
        let width = widest * self.segments.len().max(1) as f32;
        constraints.clamp(Size::new(width.max(160.0), self.segment_height()))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let padding = metrics.tab_padding;
        let label_style = semibold_control_text_style(&theme, palette.text_muted);
        let selected_label_style = TextStyle {
            color: palette.text,
            ..label_style.clone()
        };
        let radius = metrics.corner_radius;

        ctx.fill(rounded_rect_path(ctx.bounds(), radius), palette.control);

        let selected_thumb = if !self.segments.is_empty() {
            let from = self.selection_from.min(self.segments.len() - 1);
            let selected = self.normalized_selected();
            let thumb = sliding_inset_rect(
                |index| self.segment_rect(ctx.bounds(), index),
                from,
                selected,
                self.selection_animation.value,
                Insets::all(2.0),
            );
            if let Some(thumb) = thumb {
                draw_control_shape(
                    ctx,
                    thumb,
                    (thumb.height() * 0.5).min(radius),
                    physical_pixels(ctx, metrics.border_width),
                    palette.selection,
                    palette.selection_border,
                );
            }
            thumb
        } else {
            None
        };

        let focus_progress = self.focus_animation.value;
        for (index, segment) in self.segments.iter().enumerate() {
            let Some(rect) = self.segment_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if !selected
                && let Some((background, border)) =
                    tab_state_visuals(&theme, false, hovered, pressed, hover_amount, press_amount)
            {
                draw_control_shape(
                    ctx,
                    rect.inflate(-1.0, -1.0),
                    radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                let focus_bounds = selected_thumb.unwrap_or_else(|| rect.inflate(-2.0, -2.0));
                draw_focus_ring_frame(
                    ctx,
                    focus_bounds,
                    (focus_bounds.height() * 0.5).min(radius),
                    metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = if selected {
                selected_label_style.clone()
            } else {
                label_style.clone()
            };
            let text_slot = inset_rect(rect, padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                &segment.label,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let selected = self.normalized_selected();
        let value = self
            .segments
            .get(selected)
            .map(|segment| segment.label.clone());
        let mut group =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::RadioGroup, ctx.bounds());
        group.name = Some(self.name.clone());
        group.value = value.map(SemanticsValue::Text);
        group.state.focused = ctx.is_focused();
        group.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(group);

        for (index, segment) in self.segments.iter().enumerate() {
            let Some(bounds) = self.segment_rect(ctx.bounds(), index) else {
                continue;
            };
            let mut node = SemanticsNode::new(
                segmented_control_item_id(ctx.widget_id(), index),
                SemanticsRole::RadioButton,
                bounds,
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some(
                segment
                    .semantic_name
                    .clone()
                    .unwrap_or_else(|| segment.label.clone()),
            );
            node.description = segment.description.clone();
            node.value = Some(SemanticsValue::Text(segment.label.clone()));
            node.actions = if segment.disabled {
                Vec::new()
            } else {
                vec![SemanticsAction::Activate]
            };
            node.state.disabled = segment.disabled;
            node.state.hovered = self.hovered == Some(index);
            node.state.selected = selected == index;
            node.state.checked = Some(if selected == index {
                sui_core::ToggleState::Checked
            } else {
                sui_core::ToggleState::Unchecked
            });
            ctx.push(node);
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

pub struct Tabs {
    pub(super) theme: Box<DefaultTheme>,
    pub(super) theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    pub(super) name: String,
    pub(super) labels: Vec<String>,
    pub(super) panels: WidgetChildren,
    pub(super) selected: usize,
    pub(super) selection_from: usize,
    pub(super) selection_animation: AnimatedScalar,
    pub(super) hovered: Option<usize>,
    pub(super) hover_visual: Option<usize>,
    pub(super) pressed: Option<usize>,
    pub(super) press_visual: Option<usize>,
    pub(super) hover_animation: AnimatedScalar,
    pub(super) press_animation: AnimatedScalar,
    pub(super) focus_animation: AnimatedScalar,
    pub(super) label_measurements: Vec<TextMeasurement>,
    pub(super) widths: Vec<f32>,
    pub(super) gap: Option<f32>,
    pub(super) panel_frame: Rect,
    pub(super) on_change: Option<Box<dyn FnMut(usize, String)>>,
}

impl Tabs {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            name: name.into(),
            labels: Vec::new(),
            panels: WidgetChildren::new(),
            selected: 0,
            selection_from: 0,
            selection_animation: AnimatedScalar::new(1.0),
            hovered: None,
            hover_visual: None,
            pressed: None,
            press_visual: None,
            hover_animation: AnimatedScalar::new(0.0),
            press_animation: AnimatedScalar::new(0.0),
            focus_animation: AnimatedScalar::new(0.0),
            label_measurements: Vec::new(),
            widths: Vec::new(),
            gap: None,
            panel_frame: Rect::ZERO,
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

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self.selection_from = index;
        self.selection_animation = AnimatedScalar::new(1.0);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(usize, String) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn tab<W>(mut self, label: impl Into<String>, panel: W) -> Self
    where
        W: Widget + 'static,
    {
        self.labels.push(label.into());
        self.panels.push(panel);
        self
    }

    pub fn selected_index(&self) -> usize {
        self.normalized_selected()
    }

    pub fn current_tab(&self) -> Option<&str> {
        self.labels
            .get(self.normalized_selected())
            .map(String::as_str)
    }

    pub(super) fn normalized_selected(&self) -> usize {
        if self.labels.is_empty() {
            0
        } else {
            self.selected.min(self.labels.len() - 1)
        }
    }

    pub(super) fn header_height(&self) -> f32 {
        self.resolved_theme().metrics.tab_height
    }

    pub(super) fn resolved_gap(&self) -> f32 {
        self.gap
            .unwrap_or(self.resolved_theme().metrics.tab_gap)
            .max(0.0)
    }

    pub(super) fn header_rect(&self, bounds: Rect) -> Rect {
        Rect::new(bounds.x(), bounds.y(), bounds.width(), self.header_height())
    }

    pub(super) fn tab_rect(&self, bounds: Rect, index: usize) -> Option<Rect> {
        if index >= self.labels.len() || self.widths.len() != self.labels.len() {
            return None;
        }

        let header = self.header_rect(bounds);
        let gap = self.resolved_gap();
        let base_total =
            self.widths.iter().sum::<f32>() + (gap * self.labels.len().saturating_sub(1) as f32);
        let extra_per_tab = if header.width() > base_total && !self.labels.is_empty() {
            (header.width() - base_total) / self.labels.len() as f32
        } else {
            0.0
        };

        let mut x = header.x();
        for (current, width) in self.widths.iter().enumerate() {
            let rect = Rect::new(x, header.y(), *width + extra_per_tab, header.height());
            if current == index {
                return Some(rect);
            }
            x += rect.width() + gap;
        }

        None
    }

    pub(super) fn tab_at(&self, bounds: Rect, position: Point) -> Option<usize> {
        self.labels.iter().enumerate().find_map(|(index, _)| {
            self.tab_rect(bounds, index)
                .filter(|rect| rect.contains(position))
                .map(|_| index)
        })
    }

    pub(super) fn select(&mut self, index: usize, ctx: &mut EventCtx) {
        if self.labels.is_empty() {
            return;
        }

        let index = index.min(self.labels.len() - 1);
        if self.selected != index {
            let theme = self.resolved_theme();
            self.selection_from = self.normalized_selected();
            self.selected = index;
            self.selection_animation = AnimatedScalar::new(0.0);
            self.selection_animation.set_target_event(
                1.0,
                theme.motion.tab_switch_duration(),
                theme.motion.tab_switch_easing(),
                ctx,
            );
            if let Some(on_change) = &mut self.on_change {
                on_change(index, self.labels[index].clone());
            }
        }
    }

    pub(super) fn move_selection(&mut self, delta: isize, ctx: &mut EventCtx) {
        if self.labels.is_empty() {
            return;
        }

        let next = (self.normalized_selected() as isize + delta)
            .clamp(0, self.labels.len() as isize - 1) as usize;
        self.set_hovered(Some(next), ctx);
        self.select(next, ctx);
    }

    pub(super) fn advance_animations(&mut self, time: f64) -> bool {
        let selection_animating = self.selection_animation.advance(time);
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
        let focus_animating = self.focus_animation.advance(time);
        selection_animating | hover_animating | press_animating | focus_animating
    }

    pub(super) fn selected_panel(&self) -> Option<&sui_runtime::WidgetPod> {
        self.panels.as_slice().get(self.normalized_selected())
    }

    pub(super) fn selected_panel_mut(&mut self) -> Option<&mut sui_runtime::WidgetPod> {
        let index = self.normalized_selected();
        self.panels.as_mut_slice().get_mut(index)
    }

    pub(super) fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
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
}

impl Widget for Tabs {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(self.tab_at(ctx.bounds(), pointer.position), ctx);
            }
            Event::Pointer(_pointer) if matches!(_pointer.kind, PointerEventKind::Leave) => {
                self.set_hovered(None, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.header_rect(ctx.bounds()).contains(pointer.position) =>
            {
                let hovered = self.tab_at(ctx.bounds(), pointer.position);
                self.set_hovered(hovered, ctx);
                self.set_pressed(hovered, ctx);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if self.pressed.is_some() {
                    let hovered = self.tab_at(ctx.bounds(), pointer.position);
                    if let Some(index) = self
                        .pressed
                        .zip(hovered)
                        .filter(|(left, right)| left == right)
                        .map(|(index, _)| index)
                    {
                        self.select(index, ctx);
                        ctx.request_measure();
                    }
                    self.set_hovered(hovered, ctx);
                    self.set_pressed(None, ctx);
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
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
                    "ArrowLeft" | "ArrowUp" => self.move_selection(-1, ctx),
                    "ArrowRight" | "ArrowDown" => self.move_selection(1, ctx),
                    "Home" => self.select(0, ctx),
                    "End" if !self.labels.is_empty() => self.select(self.labels.len() - 1, ctx),
                    _ => return,
                }
                ctx.request_measure();
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
        let text_style = theme.text_style(theme.palette.text);
        let tab_padding = theme.metrics.tab_padding;
        self.label_measurements = self
            .labels
            .iter()
            .map(|label| measure_text(ctx, label, &text_style))
            .collect();
        self.widths = self
            .label_measurements
            .iter()
            .map(|measurement| {
                (measurement.width + tab_padding.left + tab_padding.right)
                    .max(theme.metrics.tab_min_width)
            })
            .collect();

        let gap = self.resolved_gap();
        let header_width =
            self.widths.iter().sum::<f32>() + (gap * self.labels.len().saturating_sub(1) as f32);
        let available_width = if constraints.max.width.is_finite() {
            constraints.max.width.max(header_width)
        } else {
            header_width.max(320.0)
        };
        let header_height = self.header_height();
        let padding = theme.metrics.tab_panel_padding;
        let panel_gap = theme.metrics.tab_panel_gap;

        let panel_constraints = Constraints::new(
            Size::ZERO,
            Size::new(
                (available_width - padding.left - padding.right).max(0.0),
                if constraints.max.height.is_finite() {
                    (constraints.max.height
                        - header_height
                        - panel_gap
                        - padding.top
                        - padding.bottom)
                        .max(0.0)
                } else {
                    f32::INFINITY
                },
            ),
        );

        let panel_size = if let Some(panel) = self.selected_panel_mut() {
            panel.measure(ctx, panel_constraints)
        } else {
            Size::new(0.0, theme.metrics.min_height)
        };

        let content_width = (panel_size.width + padding.left + padding.right).max(available_width);
        let content_height = panel_size.height + padding.top + padding.bottom;
        self.panel_frame = Rect::new(
            0.0,
            header_height + panel_gap,
            content_width,
            content_height,
        );

        constraints.clamp(Size::new(
            content_width,
            header_height + panel_gap + content_height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let theme = self.resolved_theme();
        let header_height = self.header_height();
        let padding = theme.metrics.tab_panel_padding;
        let panel_gap = theme.metrics.tab_panel_gap;
        if let Some(panel) = self.selected_panel_mut() {
            let panel_size = panel.measured_size();
            panel.arrange(
                ctx,
                Rect::new(
                    bounds.x() + padding.left,
                    bounds.y() + header_height + panel_gap + padding.top,
                    panel_size.width,
                    panel_size.height,
                ),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = self.resolved_theme();
        let palette = theme.palette;
        let metrics = theme.metrics;
        let interaction = theme.interaction;
        let tab_padding = metrics.tab_padding;
        let header = self.header_rect(ctx.bounds());
        let label_style = theme.text_style(palette.text_muted);
        let selected_label_style = theme.text_style(palette.text);

        ctx.fill(
            rounded_rect_path(header, metrics.corner_radius),
            palette.control,
        );

        let focus_progress = self.focus_animation.value;
        for (index, label) in self.labels.iter().enumerate() {
            let Some(rect) = self.tab_rect(ctx.bounds(), index) else {
                continue;
            };
            let selected = self.normalized_selected() == index;
            let hovered = self.hovered == Some(index);
            let pressed = self.pressed == Some(index);
            let hover_amount = self.hover_amount_for(index);
            let press_amount = self.press_amount_for(index);

            if let Some((background, border)) = tab_state_visuals(
                &theme,
                selected,
                hovered,
                pressed,
                hover_amount,
                press_amount,
            ) {
                draw_control_shape(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    physical_pixels(ctx, metrics.border_width),
                    background,
                    border,
                );
            }

            if selected && focus_progress > AnimatedScalar::EPSILON {
                draw_focus_ring_frame(
                    ctx,
                    rect,
                    metrics.corner_radius,
                    metrics,
                    palette
                        .focus_ring
                        .with_alpha(palette.focus_ring.alpha * focus_progress),
                );
            }

            let text_style = if selected {
                selected_label_style.clone()
            } else {
                label_style.clone()
            };
            let text_slot = inset_rect(rect, tab_padding);
            let pressed_offset = press_amount * interaction.pressed_offset;
            ctx.push_clip_rect(text_slot);
            paint_aligned_text(
                ctx,
                text_slot.translate(Vector::new(0.0, pressed_offset)),
                label,
                &text_style,
                text_style.line_height,
                0.5,
            );
            ctx.pop_clip();
        }

        if let Some(accent) = tab_indicator_rect(
            |index| self.tab_rect(ctx.bounds(), index),
            self.selection_from,
            self.normalized_selected(),
            self.selection_animation.value,
            tab_padding,
            interaction.active_indicator_thickness,
        ) {
            ctx.fill(
                rounded_rect_path(accent, accent.height() * 0.5),
                palette.accent,
            );
        }

        let content = self.panel_frame.translate(ctx.bounds().origin.to_vector());
        draw_control_frame(
            ctx,
            content,
            metrics.corner_radius + 2.0,
            metrics,
            palette.surface_raised,
            palette.border,
            None,
        );
        if let Some(panel) = self.selected_panel() {
            let panel_translation = tab_panel_transition_translation(
                self.selection_from,
                self.normalized_selected(),
                self.selection_animation.value,
                metrics,
            );
            if panel_translation == Vector::ZERO {
                panel.paint(ctx);
            } else {
                ctx.push_clip_rect(content);
                ctx.translate(panel_translation);
                panel.paint(ctx);
                ctx.pop_transform();
                ctx.pop_clip();
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Tabs, ctx.bounds());
        node.name = Some(self.name.clone());
        node.value = self
            .current_tab()
            .map(|value| SemanticsValue::Text(value.to_string()));
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
        if let Some(panel) = self.selected_panel() {
            panel.semantics(ctx);
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

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if let Some(panel) = self.selected_panel() {
            visitor.visit(panel);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if let Some(panel) = self.selected_panel_mut() {
            visitor.visit(panel);
        }
    }
}
