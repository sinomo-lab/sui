use sui_core::{
    Color, Event, KeyState, Point, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsNode, SemanticsRole, SemanticsValue, Size,
};
use sui_layout::{Axis, Constraints};
use sui_runtime::{
    ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, SingleChild, StackHostOptions,
    StackOrderPolicy, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::StrokeStyle;

use crate::DefaultTheme;

pub type ResizablePane = SplitView;

pub struct SplitView {
    theme: Box<DefaultTheme>,
    name: Option<String>,
    axis: Axis,
    ratio: f32,
    min_first: f32,
    min_second: f32,
    divider_thickness: f32,
    first: SingleChild,
    second: SingleChild,
    hovered: bool,
    drag_pointer: Option<u64>,
    divider_bounds: Rect,
    on_change: Option<Box<dyn FnMut(f32)>>,
}

impl SplitView {
    pub fn new<W1, W2>(axis: Axis, first: W1, second: W2) -> Self
    where
        W1: Widget + 'static,
        W2: Widget + 'static,
    {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: None,
            axis,
            ratio: 0.5,
            min_first: 120.0,
            min_second: 120.0,
            divider_thickness: 10.0,
            first: SingleChild::new(first),
            second: SingleChild::new(second),
            hovered: false,
            drag_pointer: None,
            divider_bounds: Rect::ZERO,
            on_change: None,
        }
    }

    pub fn horizontal<W1, W2>(first: W1, second: W2) -> Self
    where
        W1: Widget + 'static,
        W2: Widget + 'static,
    {
        Self::new(Axis::Horizontal, first, second)
    }

    pub fn vertical<W1, W2>(first: W1, second: W2) -> Self
    where
        W1: Widget + 'static,
        W2: Widget + 'static,
    {
        Self::new(Axis::Vertical, first, second)
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio.clamp(0.0, 1.0);
        self
    }

    pub fn min_first(mut self, min_first: f32) -> Self {
        self.min_first = min_first.max(0.0);
        self
    }

    pub fn min_second(mut self, min_second: f32) -> Self {
        self.min_second = min_second.max(0.0);
        self
    }

    pub fn divider_thickness(mut self, divider_thickness: f32) -> Self {
        self.divider_thickness = divider_thickness.max(4.0);
        self
    }

    pub fn on_change<F>(mut self, on_change: F) -> Self
    where
        F: FnMut(f32) + 'static,
    {
        self.on_change = Some(Box::new(on_change));
        self
    }

    pub fn first(&self) -> &sui_runtime::WidgetPod {
        self.first.child()
    }

    pub fn first_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.first.child_mut()
    }

    pub fn second(&self) -> &sui_runtime::WidgetPod {
        self.second.child()
    }

    pub fn second_mut(&mut self) -> &mut sui_runtime::WidgetPod {
        self.second.child_mut()
    }

    pub fn current_ratio(&self) -> f32 {
        self.ratio
    }

    fn resolved_divider_thickness(&self) -> f32 {
        self.divider_thickness
            .max(self.theme.metrics.border_width * 6.0)
    }

    fn divider_rect(&self, bounds: Rect) -> Rect {
        self.divider_bounds.translate(bounds.origin.to_vector())
    }

    fn divider_main_offset(&self, bounds: Rect) -> f32 {
        let divider = self.resolved_divider_thickness();
        let available = (axis_main(self.axis, bounds.size) - divider).max(0.0);
        let first = (available * self.ratio).clamp(
            self.min_first.min(available),
            (available - self.min_second).max(0.0),
        );
        first
    }

    fn update_hover(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn set_ratio_from_position(&mut self, bounds: Rect, position: Point) {
        let divider = self.resolved_divider_thickness();
        let total = (axis_main(self.axis, bounds.size) - divider).max(0.0);
        if total <= 0.0 {
            return;
        }

        let pointer_main = axis_position(self.axis, position) - axis_origin(self.axis, bounds);
        let clamped = pointer_main.clamp(
            self.min_first.min(total),
            (total - self.min_second).max(0.0),
        );
        let ratio = (clamped / total).clamp(0.0, 1.0);
        if (ratio - self.ratio).abs() > f32::EPSILON {
            self.ratio = ratio;
            if let Some(on_change) = &mut self.on_change {
                on_change(self.ratio);
            }
        }
    }

    fn nudge_ratio(&mut self, delta: f32) {
        let next = (self.ratio + delta).clamp(0.0, 1.0);
        if (next - self.ratio).abs() > f32::EPSILON {
            self.ratio = next;
            if let Some(on_change) = &mut self.on_change {
                on_change(self.ratio);
            }
        }
    }
}

impl Widget for SplitView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let divider = self.divider_rect(ctx.bounds());
                if self.drag_pointer == Some(pointer.pointer_id) {
                    self.set_ratio_from_position(ctx.bounds(), pointer.position);
                    ctx.request_arrange();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                } else {
                    self.update_hover(divider.contains(pointer.position), ctx);
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.divider_rect(ctx.bounds()).contains(pointer.position) =>
            {
                self.drag_pointer = Some(pointer.pointer_id);
                self.hovered = true;
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                self.set_ratio_from_position(ctx.bounds(), pointer.position);
                ctx.request_arrange();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.drag_pointer == Some(pointer.pointer_id) =>
            {
                self.drag_pointer = None;
                self.hovered = self.divider_rect(ctx.bounds()).contains(pointer.position);
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.drag_pointer == Some(pointer.pointer_id) =>
            {
                self.drag_pointer = None;
                self.hovered = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.drag_pointer.is_none() {
                    self.update_hover(false, ctx);
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let step = 0.05;
                let delta = match (self.axis, key.key.as_str()) {
                    (Axis::Horizontal, "ArrowLeft") | (Axis::Vertical, "ArrowUp") => -step,
                    (Axis::Horizontal, "ArrowRight") | (Axis::Vertical, "ArrowDown") => step,
                    (Axis::Horizontal, "Home") | (Axis::Vertical, "Home") => {
                        self.ratio = 0.0;
                        ctx.request_arrange();
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                        return;
                    }
                    (Axis::Horizontal, "End") | (Axis::Vertical, "End") => {
                        self.ratio = 1.0;
                        ctx.request_arrange();
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                        return;
                    }
                    _ => return,
                };
                self.nudge_ratio(delta);
                ctx.request_arrange();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let divider = self.resolved_divider_thickness();

        let probe_constraints = constraints.loosen();
        let first_probe = self.first.measure(ctx, probe_constraints);
        let second_probe = self.second.measure(ctx, probe_constraints);
        let natural = axis_size(
            self.axis,
            axis_main(self.axis, first_probe) + divider + axis_main(self.axis, second_probe),
            axis_cross(self.axis, first_probe).max(axis_cross(self.axis, second_probe)),
        );

        let size = constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                natural.width
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                natural.height
            },
        ));

        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let divider = self.resolved_divider_thickness();
        let size = bounds.size;

        let total_main = axis_main(self.axis, size);
        let cross = axis_cross(self.axis, size);
        let divider_offset = self.divider_main_offset(Rect::from_origin_size(Point::ZERO, size));
        let first_main = divider_offset.max(0.0);
        let second_main = (total_main - divider - first_main).max(0.0);
        let first_constraints = split_child_constraints(self.axis, first_main, cross);
        let second_constraints = split_child_constraints(self.axis, second_main, cross);
        let first_size = first_constraints.clamp(self.first.child().measured_size());
        let second_size = second_constraints.clamp(self.second.child().measured_size());

        self.first.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, first_size),
        );
        let second_origin = bounds.origin + axis_point(self.axis, first_main + divider, 0.0).to_vector();
        self.second.arrange(
            ctx,
            Rect::from_origin_size(second_origin, second_size),
        );

        self.divider_bounds = Rect::from_origin_size(
            axis_point(self.axis, first_size_main(self.axis, first_size), 0.0),
            axis_size(self.axis, divider, cross),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.first.paint(ctx);
        self.second.paint(ctx);

        let palette = self.theme.palette;
        let metrics = self.theme.metrics;
        let divider_bounds = self.divider_rect(ctx.bounds());
        let divider_color = if self.drag_pointer.is_some() {
            palette.accent.with_alpha(0.16)
        } else if self.hovered {
            palette.surface_hover
        } else {
            Color::rgba(0.94, 0.955, 0.975, 1.0)
        };
        let border_color = if self.drag_pointer.is_some() || self.hovered || ctx.is_focused() {
            palette.border_focus
        } else {
            palette.border
        };

        ctx.fill_rect(divider_bounds, divider_color);
        ctx.stroke_rect(
            divider_bounds,
            border_color,
            StrokeStyle::new(metrics.border_width.max(1.0)),
        );

        let handle = if self.axis == Axis::Horizontal {
            Rect::new(
                divider_bounds.x() + ((divider_bounds.width() - 4.0) * 0.5),
                divider_bounds.y() + ((divider_bounds.height() - 28.0) * 0.5),
                4.0,
                28.0,
            )
        } else {
            Rect::new(
                divider_bounds.x() + ((divider_bounds.width() - 28.0) * 0.5),
                divider_bounds.y() + ((divider_bounds.height() - 4.0) * 0.5),
                28.0,
                4.0,
            )
        };
        ctx.fill_rect(handle, border_color.with_alpha(0.9));
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Splitter, ctx.bounds());
        node.name = self.name.clone();
        node.state.focused = ctx.is_focused();
        node.value = Some(SemanticsValue::Number(self.ratio as f64));
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetValue];
        ctx.push(node);
        self.first.semantics(ctx);
        self.second.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.first.visit_children(visitor);
        self.second.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.first.visit_children_mut(visitor);
        self.second.visit_children_mut(visitor);
    }
}

struct FloatingWindowEntry {
    bounds: Rect,
    child: WidgetPod,
}

pub struct FloatingStack {
    theme: Box<DefaultTheme>,
    name: Option<String>,
    windows: Vec<FloatingWindowEntry>,
}

impl FloatingStack {
    pub fn new() -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            name: None,
            windows: Vec::new(),
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_window<W>(mut self, bounds: Rect, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.push_window(bounds, child);
        self
    }

    pub fn push_window<W>(&mut self, bounds: Rect, child: W)
    where
        W: Widget + 'static,
    {
        self.windows.push(FloatingWindowEntry {
            bounds,
            child: WidgetPod::new(child),
        });
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    fn frontmost_window_at(&self, host_bounds: Rect, position: Point) -> Option<usize> {
        self.windows.iter().enumerate().rev().find_map(|(index, entry)| {
            entry
                .bounds
                .translate(host_bounds.origin.to_vector())
                .contains(position)
                .then_some(index)
        })
    }

    fn bring_to_front(&mut self, index: usize) -> bool {
        if index >= self.windows.len() || index + 1 == self.windows.len() {
            return false;
        }

        let entry = self.windows.remove(index);
        self.windows.push(entry);
        true
    }

    fn content_rect(&self) -> Rect {
        self.windows
            .iter()
            .fold(Rect::ZERO, |current, entry| current.union(entry.bounds))
    }
}

impl Default for FloatingStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for FloatingStack {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event {
            if pointer.kind == PointerEventKind::Down
                && pointer.button == Some(PointerButton::Primary)
                && let Some(index) = self.frontmost_window_at(ctx.bounds(), pointer.position)
                && self.bring_to_front(index)
            {
                ctx.request_ordering();
                ctx.request_hit_test();
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        for entry in self.windows.iter_mut() {
            entry
                .child
                .measure(ctx, Constraints::tight(entry.bounds.size));
        }

        let content = self.content_rect();
        constraints.clamp(Size::new(content.max_x().max(0.0), content.max_y().max(0.0)))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        for entry in self.windows.iter_mut() {
            entry
                .child
                .arrange(ctx, entry.bounds.translate(bounds.origin.to_vector()));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for entry in &self.windows {
            entry.child.paint(ctx);
        }
    }

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        Some(StackHostOptions {
            order_policy: StackOrderPolicy::FocusFronted,
        })
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            self.content_rect().translate(ctx.bounds().origin.to_vector()),
        );
        node.name = self.name.clone();
        node.state.focused = ctx.is_focused();
        ctx.push(node);

        for entry in &self.windows {
            entry.child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for entry in &self.windows {
            visitor.visit(&entry.child);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for entry in &mut self.windows {
            visitor.visit(&mut entry.child);
        }
    }
}

fn split_child_constraints(axis: Axis, main: f32, cross: f32) -> Constraints {
    match axis {
        Axis::Horizontal => Constraints::tight(Size::new(main.max(0.0), cross.max(0.0))),
        Axis::Vertical => Constraints::tight(Size::new(cross.max(0.0), main.max(0.0))),
    }
}

fn axis_main(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn axis_cross(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn axis_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

fn axis_point(axis: Axis, main: f32, cross: f32) -> Point {
    match axis {
        Axis::Horizontal => Point::new(main, cross),
        Axis::Vertical => Point::new(cross, main),
    }
}

fn axis_position(axis: Axis, point: Point) -> f32 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn axis_origin(axis: Axis, rect: Rect) -> f32 {
    match axis {
        Axis::Horizontal => rect.x(),
        Axis::Vertical => rect.y(),
    }
}

fn first_size_main(axis: Axis, size: Size) -> f32 {
    axis_main(axis, size)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{FloatingStack, SplitView};
    use crate::containers::SizedBox;
    use sui_core::{
        Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind, Rect,
        Result, SemanticsRole, SemanticsValue, Vector,
    };
    use sui_layout::Axis;
    use sui_runtime::{Application, Runtime, StackOrderPolicy, Widget, WindowBuilder};
    use sui_scene::SceneLayerUpdateKind;

    fn build_runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("SplitView").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut buttons = PointerButtons::NONE;
        if pressed {
            buttons.insert(PointerButton::Primary);
        }

        Event::Pointer(PointerEvent {
            pointer_id: 1,
            kind,
            position,
            delta: Vector::ZERO,
            scroll_delta: None,
            button: Some(PointerButton::Primary),
            buttons,
            modifiers: sui_core::Modifiers::NONE,
            pointer_kind: sui_core::PointerKind::Mouse,
            is_primary: true,
        })
    }

    #[test]
    fn split_view_drag_updates_ratio_and_semantics() -> Result<()> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let on_change = Rc::clone(&changes);
        let (mut runtime, window_id) = build_runtime(
            SplitView::new(
                Axis::Horizontal,
                SizedBox::new().width(100.0).height(40.0),
                SizedBox::new().width(100.0).height(40.0),
            )
            .name("Editor split")
            .min_first(40.0)
            .min_second(40.0)
            .on_change(move |ratio| on_change.borrow_mut().push(ratio)),
        );

        let _ = runtime.render(window_id)?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(105.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(145.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(145.0, 20.0), false),
        )?;

        assert!(changes.borrow().last().is_some_and(|ratio| *ratio > 0.65));

        let output = runtime.render(window_id)?;
        let splitter = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Splitter)
            .expect("splitter semantics present");
        assert_eq!(splitter.name.as_deref(), Some("Editor split"));
        assert!(matches!(
            splitter.value,
            Some(SemanticsValue::Number(value)) if value > 0.65
        ));
        Ok(())
    }

    #[test]
    fn floating_stack_reorders_host_surfaces_on_pointer_focus() -> Result<()> {
        let (mut runtime, window_id) = build_runtime(
            FloatingStack::new()
                .name("floating workspace")
                .with_window(
                    Rect::new(0.0, 0.0, 120.0, 80.0),
                    SizedBox::new().width(120.0).height(80.0),
                )
                .with_window(
                    Rect::new(48.0, 0.0, 120.0, 80.0),
                    SizedBox::new().width(120.0).height(80.0),
                ),
        );

        let _ = runtime.render(window_id)?;
        let before = runtime.widget_graph(window_id)?;
        let host = before
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should be present");
        assert_eq!(host.surfaces.len(), 2);
        let first_surface = host.surfaces[0];

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(12.0, 12.0), true),
        )?;
        let reordered = runtime.render(window_id)?;

        let after = runtime.widget_graph(window_id)?;
        let host = after
            .stack_hosts
            .iter()
            .find(|host| host.order_policy == StackOrderPolicy::FocusFronted)
            .expect("focus-fronted host should still be present");
        assert_eq!(host.surfaces.len(), 2);
        assert_eq!(host.surfaces[1], first_surface);

        assert!(reordered
            .frame
            .layer_updates
            .iter()
            .any(|update| update.kind == SceneLayerUpdateKind::Ordering));
        assert!(reordered
            .frame
            .layer_updates
            .iter()
            .all(|update| update.kind != SceneLayerUpdateKind::Content));
        Ok(())
    }
}
