use std::{cell::Cell, rc::Rc};

use sui_core::{
    Event, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons, PointerEvent,
    PointerEventKind, PointerKind, Rect, SemanticsRole, Size, WindowEvent, WindowId,
};
use sui_layout::Constraints;
use sui_runtime::{
    ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_widgets::{Button, ContextMenu, MenuItem, Tooltip};

/// A toolbar may probe a clean menu at a new width while retaining its final
/// bounds. Keep that popup at the arranged trigger, including its hit targets.
struct ProbeHost {
    child: WidgetPod,
    measure_context: Option<MeasureCtx>,
    bounds: Rect,
}

impl Widget for ProbeHost {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Keyboard(_)) {
            if let Event::Keyboard(key) = event
                && key.key == "F8"
            {
                self.bounds.size.height *= 0.5;
                ctx.request_measure();
                ctx.request_arrange();
                return;
            }
            let constraints = Constraints::new(Size::ZERO, Size::new(120.0, 60.0));
            if let Event::Keyboard(key) = event {
                if key.key == "F6" {
                    self.child
                        .probe_measure(self.measure_context.as_mut().unwrap(), constraints);
                } else {
                    // Default wrapper probes call the child's ordinary measure.
                    self.child
                        .measure(self.measure_context.as_mut().unwrap(), constraints);
                }
            }
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.measure_context = Some(ctx.clone());
        self.child
            .measure(ctx, Constraints::tight(self.bounds.size));
        constraints.max
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, _bounds: Rect) {
        self.child.arrange(ctx, self.bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }
    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }
    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        visitor.visit(&self.child);
    }
    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        visitor.visit(&mut self.child);
    }
}

fn touch(runtime: &mut sui_runtime::Runtime, id: WindowId, point: Point) {
    for kind in [PointerEventKind::Down, PointerEventKind::Up] {
        let mut pointer = PointerEvent::new(kind, point);
        pointer.pointer_kind = PointerKind::Touch;
        pointer.pointer_id = 1;
        pointer.button = Some(PointerButton::Primary);
        if kind == PointerEventKind::Down {
            pointer.buttons = PointerButtons::new(1);
        }
        runtime.handle_event(id, Event::Pointer(pointer)).unwrap();
        runtime.render(id).unwrap();
    }
}

#[test]
fn context_menu_keeps_placement_and_touch_activation_after_size_probes() {
    let activated = Rc::new(Cell::new(false));
    let action = activated.clone();
    let menu = ContextMenu::new("new-menu", Tooltip::new("Create", Button::new("New")))
        .activation_button(PointerButton::Primary)
        .anchor_to_pointer(false)
        .items([MenuItem::new("New file"), MenuItem::new("New folder")])
        .on_activate(move |index, _| action.set(index == 1));
    let mut runtime = sui_runtime::Application::new()
        .window(sui_runtime::WindowBuilder::new().root(ProbeHost {
            child: WidgetPod::new(menu),
            measure_context: None,
            bounds: Rect::new(32.0, 200.0, 48.0, 40.0),
        }))
        .build()
        .unwrap();
    let id = runtime.window_ids()[0];
    runtime
        .handle_event(
            id,
            Event::Window(WindowEvent::Resized(Size::new(411.0, 800.0))),
        )
        .unwrap();
    runtime.render(id).unwrap();
    touch(&mut runtime, id, Point::new(56.0, 220.0));
    let before = runtime.render(id).unwrap();
    let before_bounds = before
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New folder")
        })
        .expect("initial folder item")
        .bounds;
    assert!(
        before_bounds.y() >= 240.0,
        "initial popup must be below its trigger"
    );
    runtime
        .handle_event(
            id,
            Event::Keyboard(KeyboardEvent::new("F6", KeyState::Pressed)),
        )
        .unwrap();
    runtime
        .handle_event(
            id,
            Event::Keyboard(KeyboardEvent::new("F7", KeyState::Pressed)),
        )
        .unwrap();
    let output = runtime.render(id).unwrap();
    let folder = output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::MenuItem && node.name.as_deref() == Some("New folder")
        })
        .expect("open folder item");
    assert_eq!(
        folder.bounds, before_bounds,
        "size probe must preserve the arranged popup"
    );
    touch(
        &mut runtime,
        id,
        Point::new(folder.bounds.x() + 24.0, folder.bounds.y() + 14.0),
    );
    assert!(
        activated.get(),
        "touch must activate the displayed menu item"
    );
}

#[test]
fn touch_does_not_leave_a_hover_tooltip_over_the_activated_control() {
    let activated = Rc::new(Cell::new(false));
    let action = activated.clone();
    let mut runtime = sui_runtime::Application::new()
        .window(sui_runtime::WindowBuilder::new().root(ProbeHost {
            child: WidgetPod::new(Tooltip::new(
                "Hover hint",
                Button::new("New").on_press(move || action.set(true)),
            )),
            measure_context: None,
            bounds: Rect::new(32.0, 200.0, 48.0, 40.0),
        }))
        .build()
        .unwrap();
    let id = runtime.window_ids()[0];
    runtime
        .handle_event(
            id,
            Event::Window(WindowEvent::Resized(Size::new(411.0, 800.0))),
        )
        .unwrap();
    runtime.render(id).unwrap();
    let point = Point::new(56.0, 220.0);
    for kind in [PointerEventKind::Enter, PointerEventKind::Move] {
        let mut pointer = PointerEvent::new(kind, point);
        pointer.pointer_kind = PointerKind::Touch;
        pointer.pointer_id = 1;
        runtime.handle_event(id, Event::Pointer(pointer)).unwrap();
    }
    touch(&mut runtime, id, point);
    let output = runtime.render(id).unwrap();
    assert!(activated.get());
    assert!(
        !output
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Tooltip)
    );

    runtime
        .handle_event(
            id,
            Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)),
        )
        .unwrap();
    let output = runtime.render(id).unwrap();
    assert!(
        output
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Tooltip),
        "mouse hover must retain its hint"
    );
}

#[test]
fn table_measurement_probes_preserve_the_committed_viewport() {
    let state = sui_widgets::VirtualTableState::new();
    let mut runtime = sui_runtime::Application::new()
        .window(
            sui_runtime::WindowBuilder::new().root(ProbeHost {
                child: WidgetPod::new(
                    sui_widgets::VirtualTable::new("Files")
                        .state(state.clone())
                        .columns([sui_widgets::VirtualTableColumn::new("Name")])
                        .row_count(100),
                ),
                measure_context: None,
                bounds: Rect::new(32.0, 200.0, 300.0, 400.0),
            }),
        )
        .build()
        .unwrap();
    let id = runtime.window_ids()[0];
    runtime
        .handle_event(
            id,
            Event::Window(WindowEvent::Resized(Size::new(411.0, 800.0))),
        )
        .unwrap();
    runtime.render(id).unwrap();
    let committed = state.viewport();
    assert!(
        !committed.visible_keys.is_empty(),
        "arrangement must publish the visible rows"
    );
    for key in ["F6", "F7"] {
        runtime
            .handle_event(
                id,
                Event::Keyboard(KeyboardEvent::new(key, KeyState::Pressed)),
            )
            .unwrap();
        assert_eq!(
            state.viewport(),
            committed,
            "{key} must not publish a provisional viewport"
        );
    }
    runtime
        .handle_event(
            id,
            Event::Keyboard(KeyboardEvent::new("F8", KeyState::Pressed)),
        )
        .unwrap();
    runtime.render(id).unwrap();
    assert!(
        state.viewport().visible_range.end < committed.visible_range.end,
        "real arrangement must publish the smaller visible range"
    );
}

#[test]
fn a_table_without_retained_state_reclamps_scrolling_after_resize() {
    let mut runtime = sui_runtime::Application::new()
        .window(
            sui_runtime::WindowBuilder::new().root(
                sui_widgets::VirtualTable::new("Files")
                    .columns([sui_widgets::VirtualTableColumn::new("Name")])
                    .row_count(100)
                    .row_name(|index| format!("Row {index}")),
            ),
        )
        .build()
        .unwrap();
    let id = runtime.window_ids()[0];
    runtime
        .handle_event(
            id,
            Event::Window(WindowEvent::Resized(Size::new(411.0, 400.0))),
        )
        .unwrap();
    runtime.render(id).unwrap();
    touch(&mut runtime, id, Point::new(100.0, 100.0));
    runtime
        .handle_event(
            id,
            Event::Keyboard(KeyboardEvent::new("End", KeyState::Pressed)),
        )
        .unwrap();
    let scrolled = runtime.render(id).unwrap();
    assert!(
        !scrolled
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::ListItem
                && node.name.as_deref() == Some("Row 0")),
        "positive control must reach the end of the table"
    );
    runtime
        .handle_event(
            id,
            Event::Window(WindowEvent::Resized(Size::new(411.0, 4000.0))),
        )
        .unwrap();
    let enlarged = runtime.render(id).unwrap();
    assert!(
        enlarged
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::ListItem
                && node.name.as_deref() == Some("Row 0")),
        "final arrangement must clamp the old offset when all rows fit"
    );
}
