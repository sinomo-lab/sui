use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use sui_core::{
    DragDropScope, DragEvent, DragEventKind, DragPayload, DragPreview, DragSessionId, DropEffect,
    DropEffects, Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point,
    PointerButton, PointerEventKind, Rect, Size, Vector, WindowEvent,
};
use sui_layout::Constraints;
use sui_runtime::{
    ArrangeCtx, EventCtx, LayerOptions, MeasureCtx, OverlayDismissPolicy, OverlayFocusBehavior,
    OverlayKind, OverlayOptions, PaintBoundaryMode, PaintCtx, SemanticsCtx, SingleChild,
    StackSurfaceOptions, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{Border, LayerCompositionMode};

use crate::DefaultTheme;

const DEFAULT_DRAG_THRESHOLD: f32 = 4.0;

type PayloadFactory = Box<dyn FnMut() -> DragPayload>;
type DragStartCallback = Box<dyn FnMut(&mut EventCtx, &DragPreview)>;
type DragEndCallback = Box<dyn FnMut(&mut EventCtx, &DragEvent)>;
type DropAcceptCallback = Box<dyn FnMut(&DragEvent) -> DropEffect>;
type DropCallback = Box<dyn FnMut(&mut EventCtx, &DragEvent)>;
type HoverCallback = Box<dyn FnMut(bool)>;
type HoverCallbackWithCtx = Box<dyn FnMut(&mut EventCtx, bool)>;
type HoverStateCallback = Box<dyn FnMut(DropHover)>;
type HoverStateCallbackWithCtx = Box<dyn FnMut(&mut EventCtx, DropHover)>;
type ThemeReader = Rc<dyn Fn() -> DefaultTheme>;
type PreviewBuilder = Box<dyn FnMut(&DragPreview) -> Option<Box<dyn Widget>>>;

/// How far below and right of the pointer drag previews are drawn.
const DRAG_PREVIEW_OFFSET: Vector = Vector::new(12.0, 12.0);

#[derive(Default)]
struct DragPreviewThemeSource {
    theme: DefaultTheme,
    reader: Option<ThemeReader>,
}

impl DragPreviewThemeSource {
    fn resolve(&self) -> DefaultTheme {
        self.reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.theme)
    }
}

pub struct DragDropHost {
    scope: DragDropScope,
    child: SingleChild,
    overlay: SingleChild,
    preview_theme: Rc<RefCell<DragPreviewThemeSource>>,
    preview_builder: Option<PreviewBuilder>,
    /// The drag the preview was last built for, and the widget built for it.
    preview_session: Option<DragSessionId>,
    preview: Option<SingleChild>,
    /// The drag whose preview is a built widget, so the label stays hidden.
    widget_preview_session: Rc<Cell<Option<DragSessionId>>>,
    external_hovered: Vec<PathBuf>,
    on_external_hover: Option<Box<dyn FnMut(&mut EventCtx, &[PathBuf])>>,
    on_external_drop: Option<Box<dyn FnMut(&mut EventCtx, PathBuf)>>,
    on_external_cancel: Option<Box<dyn FnMut(&mut EventCtx)>>,
}

impl DragDropHost {
    pub fn new<W>(scope: DragDropScope, child: W) -> Self
    where
        W: Widget + 'static,
    {
        let preview_theme = Rc::new(RefCell::new(DragPreviewThemeSource::default()));
        let widget_preview_session = Rc::new(Cell::new(None));
        Self {
            overlay: SingleChild::new(DragPreviewOverlay::new(
                scope.clone(),
                Rc::clone(&preview_theme),
                Rc::clone(&widget_preview_session),
            )),
            scope,
            child: SingleChild::new(child),
            preview_theme,
            preview_builder: None,
            preview_session: None,
            preview: None,
            widget_preview_session,
            external_hovered: Vec::new(),
            on_external_hover: None,
            on_external_drop: None,
            on_external_cancel: None,
        }
    }

    pub fn scope(&self) -> &DragDropScope {
        &self.scope
    }

    pub fn child_pod(&self) -> &WidgetPod {
        self.child.child()
    }

    #[deprecated(note = "use `child_pod`")]
    pub fn child(&self) -> &WidgetPod {
        self.child_pod()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    pub fn external_hovered_files(&self) -> &[PathBuf] {
        &self.external_hovered
    }

    /// Sets the theme used to paint the drag-preview overlay.
    pub fn theme(self, theme: DefaultTheme) -> Self {
        {
            let mut source = self.preview_theme.borrow_mut();
            source.theme = theme;
            source.reader = None;
        }
        self
    }

    /// Resolves the drag-preview overlay theme whenever it is painted.
    pub fn theme_when<F>(self, reader: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.preview_theme.borrow_mut().reader = Some(Rc::new(reader));
        self
    }

    /// Builds the widget drawn under the pointer while something in this
    /// host's scope is dragged, in place of the label. `builder` runs once
    /// per drag, when it first moves over the host; returning `None` keeps
    /// the label for that drag. The widget is drawn at its natural size and
    /// does not take pointer input.
    pub fn preview<F>(mut self, builder: F) -> Self
    where
        F: FnMut(&DragPreview) -> Option<Box<dyn Widget>> + 'static,
    {
        self.preview_builder = Some(Box::new(builder));
        self
    }

    /// Whether the widget preview was built for the drag in progress.
    fn preview_is_current(&self) -> bool {
        self.preview.is_some()
            && self
                .scope
                .active_drag()
                .is_some_and(|active| Some(active.session_id) == self.preview_session)
    }

    fn follow_drag(&mut self, ctx: &mut EventCtx, drag: &DragEvent) {
        if drag.scope_id != self.scope.id()
            || !matches!(drag.kind, DragEventKind::Enter | DragEventKind::Over)
        {
            return;
        }
        // Drag events reach the host in its own coordinates, where the
        // preview is drawn, even when the source sits in scrolled content.
        self.scope
            .update_drag_position(drag.session_id, drag.position);
        ctx.request_paint();
        if self.preview_session == Some(drag.session_id) {
            // Only the preview moves; lay out just it at the new position.
            if let Some(preview) = &self.preview {
                ctx.request(InvalidationRequest::new(
                    InvalidationTarget::Widget(preview.child().id()),
                    InvalidationKind::Arrange,
                ));
            }
            return;
        }
        self.preview_session = Some(drag.session_id);
        let widget = match (&mut self.preview_builder, self.scope.active_drag()) {
            (Some(builder), Some(active)) if active.session_id == drag.session_id => {
                builder(&active)
            }
            _ => None,
        };
        self.widget_preview_session
            .set(widget.as_ref().map(|_| drag.session_id));
        self.preview = widget.map(|widget| {
            SingleChild::new(DragPreviewSurface {
                scope: self.scope.clone(),
                session_id: drag.session_id,
                child: SingleChild::from_pod(WidgetPod::new_boxed(widget)),
            })
        });
        ctx.request_measure();
        ctx.request_arrange();
    }

    pub fn on_external_file_hover<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &[PathBuf]) + 'static,
    {
        self.on_external_hover = Some(Box::new(callback));
        self
    }

    pub fn on_external_file_drop<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, PathBuf) + 'static,
    {
        self.on_external_drop = Some(Box::new(callback));
        self
    }

    pub fn on_external_file_hover_cancelled<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx) + 'static,
    {
        self.on_external_cancel = Some(Box::new(callback));
        self
    }
}

impl Widget for DragDropHost {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Drag(drag) if ctx.phase() != sui_runtime::EventPhase::Capture => {
                self.follow_drag(ctx, drag);
            }
            Event::Window(WindowEvent::ExternalFileHovered(path)) => {
                if !self.external_hovered.contains(path) {
                    self.external_hovered.push(path.clone());
                }
                if let Some(callback) = &mut self.on_external_hover {
                    callback(ctx, &self.external_hovered);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Window(WindowEvent::ExternalFileHoverCancelled) => {
                self.external_hovered.clear();
                if let Some(callback) = &mut self.on_external_cancel {
                    callback(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Window(WindowEvent::ExternalFileDropped(path)) => {
                self.external_hovered.retain(|hovered| hovered != path);
                if let Some(callback) = &mut self.on_external_drop {
                    callback(ctx, path.clone());
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if !self.preview_is_current() {
            self.preview = None;
        }
        let size = self.child.measure(ctx, constraints);
        self.overlay.measure(ctx, Constraints::tight(size));
        if let Some(preview) = &mut self.preview {
            preview.measure(ctx, Constraints::tight(size));
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
        self.overlay.arrange(ctx, bounds);
        if let Some(preview) = &mut self.preview {
            preview.arrange(ctx, bounds);
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
        self.overlay.paint(ctx);
        if self.preview_is_current()
            && let Some(preview) = &self.preview
        {
            preview.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        self.scope.active_drag().is_some().then_some(
            OverlayOptions::new(OverlayKind::DragPreview)
                .dismiss(OverlayDismissPolicy::NONE)
                .focus(OverlayFocusBehavior::NONE),
        )
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
        self.overlay.visit_children(visitor);
        if self.preview_is_current()
            && let Some(preview) = &self.preview
        {
            preview.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
        self.overlay.visit_children_mut(visitor);
        if self.preview_is_current()
            && let Some(preview) = &mut self.preview
        {
            preview.visit_children_mut(visitor);
        }
    }
}

/// Top-left corner of a `size` preview for a pointer at `pointer`, kept
/// inside `bounds` where it fits.
fn drag_preview_origin(pointer: Point, size: Size, bounds: Rect) -> Point {
    let x = (pointer.x + DRAG_PREVIEW_OFFSET.x).min((bounds.max_x() - size.width).max(bounds.x()));
    let y = (pointer.y + DRAG_PREVIEW_OFFSET.y).min((bounds.max_y() - size.height).max(bounds.y()));
    Point::new(x.max(bounds.x()), y.max(bounds.y()))
}

/// Draws a [`DragDropHost::preview`] widget under the pointer, above the
/// host's content. It covers the host and lays the widget out where the
/// pointer is, so the widget keeps its own layers as it moves.
struct DragPreviewSurface {
    scope: DragDropScope,
    session_id: DragSessionId,
    child: SingleChild,
}

impl Widget for DragPreviewSurface {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints.loosen());
        constraints.clamp(Size::new(
            constraints.max.width.max(0.0),
            constraints.max.height.max(0.0),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let size = self.child.child().measured_size();
        let pointer = self
            .scope
            .active_drag()
            .filter(|active| active.session_id == self.session_id)
            .map_or(bounds.origin, |active| active.position);
        self.child.arrange(
            ctx,
            Rect::from_origin_size(drag_preview_origin(pointer, size, bounds), size),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if self
            .scope
            .active_drag()
            .is_some_and(|active| active.session_id == self.session_id)
        {
            self.child.paint(ctx);
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        Some(StackSurfaceOptions {
            transient: true,
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct DragPreviewOverlay {
    scope: DragDropScope,
    theme: Rc<RefCell<DragPreviewThemeSource>>,
    widget_preview_session: Rc<Cell<Option<DragSessionId>>>,
}

impl DragPreviewOverlay {
    fn new(
        scope: DragDropScope,
        theme: Rc<RefCell<DragPreviewThemeSource>>,
        widget_preview_session: Rc<Cell<Option<DragSessionId>>>,
    ) -> Self {
        Self {
            scope,
            theme,
            widget_preview_session,
        }
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme.borrow().resolve()
    }
}

impl Widget for DragPreviewOverlay {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            constraints.max.width.max(0.0),
            constraints.max.height.max(0.0),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(active) = self.scope.active_drag() else {
            return;
        };
        if self.widget_preview_session.get() == Some(active.session_id) {
            return;
        }

        let label = drag_preview_label(&active);
        let theme = self.resolved_theme();
        let text_style = drag_preview_text_style(theme);
        let estimated_character_width = text_style.font_size * 0.54;
        let width =
            ((label.chars().count() as f32 * estimated_character_width) + 24.0).clamp(48.0, 260.0);
        let height = text_style.line_height + 12.0;
        let rect = Rect::from_origin_size(
            drag_preview_origin(active.position, Size::new(width, height), ctx.bounds()),
            Size::new(width, height),
        );

        ctx.fill_rrect_bordered(
            rect,
            [7.0; 4],
            theme.palette.surface_raised.with_alpha(0.96),
            Border {
                width: 1.0,
                color: theme.palette.border.with_alpha(0.72),
            },
        );

        let text_rect = Rect::new(
            rect.x() + 12.0,
            rect.y() + 6.0,
            rect.width() - 24.0,
            text_style.line_height,
        );
        ctx.push_clip_rect(text_rect);
        ctx.draw_text(text_rect, label, text_style);
        ctx.pop_clip();
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Overlay,
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        Some(StackSurfaceOptions {
            transient: true,
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }
}

fn drag_preview_text_style(theme: DefaultTheme) -> sui_text::TextStyle {
    let mut style = theme.text_style(theme.palette.text);
    style.font_size = theme.text.sm.size.max(1.0);
    style.line_height = theme.text.sm.line_height.max(1.0);
    style
}

pub struct Draggable {
    scope: DragDropScope,
    child: SingleChild,
    payload: PayloadFactory,
    allowed_effects: DropEffects,
    preview_label: Option<String>,
    threshold: f32,
    press: Option<DragPress>,
    active_session: Option<DragSessionId>,
    on_drag_start: Option<DragStartCallback>,
    on_drag_end: Option<DragEndCallback>,
}

#[derive(Debug, Clone, Copy)]
struct DragPress {
    pointer_id: u64,
    start_position: Point,
}

impl Draggable {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            scope: DragDropScope::new(),
            child: SingleChild::new(child),
            payload: Box::new(|| DragPayload::text("")),
            allowed_effects: DropEffects::MOVE,
            preview_label: None,
            threshold: DEFAULT_DRAG_THRESHOLD,
            press: None,
            active_session: None,
            on_drag_start: None,
            on_drag_end: None,
        }
    }

    pub fn scope(mut self, scope: DragDropScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn payload<F>(mut self, payload: F) -> Self
    where
        F: FnMut() -> DragPayload + 'static,
    {
        self.payload = Box::new(payload);
        self
    }

    /// Allow only `effect`.
    pub fn effect(mut self, effect: DropEffect) -> Self {
        self.allowed_effects = effect.into();
        self
    }

    /// Allow each effect in `effects`. A drop takes their
    /// [`DropEffects::default_effect`] unless modifier keys ask for another
    /// allowed one; see [`DragEvent::preferred_effect`].
    pub fn effects(mut self, effects: DropEffects) -> Self {
        self.allowed_effects = effects;
        self
    }

    pub fn preview_label(mut self, label: impl Into<String>) -> Self {
        self.preview_label = Some(label.into());
        self
    }

    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.max(0.0);
        self
    }

    pub fn on_drag_start<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &DragPreview) + 'static,
    {
        self.on_drag_start = Some(Box::new(callback));
        self
    }

    pub fn on_drag_end<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &DragEvent) + 'static,
    {
        self.on_drag_end = Some(Box::new(callback));
        self
    }

    pub fn scope_ref(&self) -> &DragDropScope {
        &self.scope
    }

    pub fn child_pod(&self) -> &WidgetPod {
        self.child.child()
    }

    #[deprecated(note = "use `child_pod`")]
    pub fn child(&self) -> &WidgetPod {
        self.child_pod()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    fn start_drag(&mut self, ctx: &mut EventCtx, press: DragPress, position: Point) {
        let payload = (self.payload)();
        let preview_label = self.preview_label.clone();
        let session_id = ctx.begin_drag(
            self.scope.id(),
            press.pointer_id,
            press.start_position,
            payload.clone(),
            self.allowed_effects,
            preview_label.clone(),
        );
        let preview = DragPreview {
            session_id,
            scope_id: self.scope.id(),
            pointer_id: press.pointer_id,
            source: ctx.widget_id(),
            position,
            start_position: press.start_position,
            payload,
            allowed_effect: self.allowed_effects.default_effect(),
            allowed_effects: self.allowed_effects,
            preview_label: preview_label.map(Arc::from),
        };
        self.scope.set_active_drag(preview.clone());
        self.active_session = Some(session_id);
        if let Some(callback) = &mut self.on_drag_start {
            callback(ctx, &preview);
        }
        ctx.request_paint();
    }
}

impl Widget for Draggable {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.bounds().contains(pointer.position)
                    && ctx.phase() != sui_runtime::EventPhase::Capture =>
            {
                self.press = Some(DragPress {
                    pointer_id: pointer.pointer_id,
                    start_position: pointer.position,
                });
                self.active_session = None;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self
                        .press
                        .is_some_and(|press| press.pointer_id == pointer.pointer_id) =>
            {
                if let Some(session_id) = self.active_session {
                    self.scope
                        .update_drag_position(session_id, pointer.position);
                    ctx.request_paint();
                    ctx.set_handled();
                    return;
                }

                let press = self.press.unwrap();
                let delta = pointer.position - press.start_position;
                let distance_sq = (delta.x * delta.x) + (delta.y * delta.y);
                if distance_sq >= self.threshold * self.threshold {
                    self.start_drag(ctx, press, pointer.position);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Up | PointerEventKind::Cancel
                ) && self
                    .press
                    .is_some_and(|press| press.pointer_id == pointer.pointer_id) =>
            {
                if self.active_session.is_none() {
                    self.press = None;
                }
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Drag(drag)
                if drag.kind == DragEventKind::End
                    && self.active_session == Some(drag.session_id) =>
            {
                self.scope.finish_drag(drag.session_id);
                self.press = None;
                self.active_session = None;
                if let Some(callback) = &mut self.on_drag_end {
                    callback(ctx, drag);
                }
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

/// Whether a drag is over a [`DropTarget`], and what the target made of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropHover {
    #[default]
    Idle,
    /// A drag the target accepts is over it, with the effect it accepts.
    Accepting(DropEffect),
    /// A drag in the target's scope that it does not accept is over it.
    Refusing,
}

impl DropHover {
    pub const fn is_accepting(self) -> bool {
        matches!(self, Self::Accepting(_))
    }
}

pub struct DropTarget {
    scope: DragDropScope,
    child: SingleChild,
    accept: DropAcceptCallback,
    hover: DropHover,
    on_drop: Option<DropCallback>,
    on_hover_change: Option<HoverCallback>,
    on_hover_change_with_ctx: Option<HoverCallbackWithCtx>,
    on_hover_state: Option<HoverStateCallback>,
    on_hover_state_with_ctx: Option<HoverStateCallbackWithCtx>,
}

impl DropTarget {
    /// Registers `child` as a drop target without imposing any hover visuals.
    ///
    /// Use [`Self::on_hover_change`] to let the target's own widget state decide
    /// whether and how to respond to an accepted drag.
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            scope: DragDropScope::new(),
            child: SingleChild::new(child),
            accept: Box::new(|_| DropEffect::Copy),
            hover: DropHover::Idle,
            on_drop: None,
            on_hover_change: None,
            on_hover_change_with_ctx: None,
            on_hover_state: None,
            on_hover_state_with_ctx: None,
        }
    }

    pub fn scope(mut self, scope: DragDropScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn accept<F>(mut self, accept: F) -> Self
    where
        F: FnMut(&DragEvent) -> DropEffect + 'static,
    {
        self.accept = Box::new(accept);
        self
    }

    pub fn on_drop<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, &DragEvent) + 'static,
    {
        self.on_drop = Some(Box::new(callback));
        self
    }

    pub fn on_hover_change<F>(mut self, callback: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_hover_change = Some(Box::new(callback));
        self
    }

    /// [`Self::on_hover_change`], with the event context first.
    pub fn on_hover_change_with_ctx<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, bool) + 'static,
    {
        self.on_hover_change_with_ctx = Some(Box::new(callback));
        self
    }

    /// Follows [`DropHover`]: whether a drag is over the target, the
    /// effect it accepts, or that it refuses the drag. Refusing lets a
    /// target show that a drag cannot land there.
    pub fn on_hover_state<F>(mut self, callback: F) -> Self
    where
        F: FnMut(DropHover) + 'static,
    {
        self.on_hover_state = Some(Box::new(callback));
        self
    }

    /// [`Self::on_hover_state`], with the event context first.
    pub fn on_hover_state_with_ctx<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&mut EventCtx, DropHover) + 'static,
    {
        self.on_hover_state_with_ctx = Some(Box::new(callback));
        self
    }

    pub fn is_hovered(&self) -> bool {
        self.hover.is_accepting()
    }

    pub fn hover_state(&self) -> DropHover {
        self.hover
    }

    pub fn scope_ref(&self) -> &DragDropScope {
        &self.scope
    }

    pub fn child_pod(&self) -> &WidgetPod {
        self.child.child()
    }

    #[deprecated(note = "use `child_pod`")]
    pub fn child(&self) -> &WidgetPod {
        self.child_pod()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }

    fn set_hover(&mut self, ctx: &mut EventCtx, hover: DropHover) {
        if self.hover == hover {
            return;
        }
        let was_hovered = self.hover.is_accepting();
        self.hover = hover;
        if let Some(callback) = &mut self.on_hover_state {
            callback(hover);
        }
        if let Some(callback) = &mut self.on_hover_state_with_ctx {
            callback(ctx, hover);
        }
        let accepting_changed = was_hovered != hover.is_accepting();
        if accepting_changed && let Some(callback) = &mut self.on_hover_change {
            callback(hover.is_accepting());
        }
        if accepting_changed && let Some(callback) = &mut self.on_hover_change_with_ctx {
            callback(ctx, hover.is_accepting());
        }
        ctx.request_paint();
        ctx.request_semantics();
    }
}

impl Widget for DropTarget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Drag(drag) = event else {
            return;
        };
        if drag.scope_id != self.scope.id() {
            return;
        }

        match drag.kind {
            DragEventKind::Enter | DragEventKind::Over => {
                let effect = (self.accept)(drag);
                if effect.is_some() {
                    ctx.accept_drop(effect);
                    self.set_hover(ctx, DropHover::Accepting(effect));
                } else {
                    self.set_hover(ctx, DropHover::Refusing);
                }
            }
            DragEventKind::Leave => {
                self.set_hover(ctx, DropHover::Idle);
            }
            DragEventKind::Drop if drag.target == Some(ctx.widget_id()) => {
                if let Some(callback) = &mut self.on_drop {
                    callback(ctx, drag);
                }
                self.set_hover(ctx, DropHover::Idle);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

fn drag_preview_label(active: &DragPreview) -> String {
    if let Some(label) = &active.preview_label {
        return label.to_string();
    }

    match &active.payload {
        DragPayload::Text(text) if !text.is_empty() => text.clone(),
        DragPayload::Image { .. } => "Image".to_string(),
        DragPayload::Custom { kind, .. } => kind.to_string(),
        DragPayload::Text(_) => "Dragging".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{Label, ScrollState, ScrollView, SizedBox, Stack, Surface, ThemeTextToken};
    use sui_core::{
        Color, DragOutcome, KeyState, KeyboardEvent, Modifiers, Path, PointerButtons, PointerEvent,
        PointerKind, Result, WindowId,
    };
    use sui_runtime::{Application, FramePacing, Runtime, WindowBuilder};
    use sui_scene::{Brush, SceneCommand};

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
            modifiers: Modifiers::NONE,
            pointer_kind: PointerKind::Mouse,
            is_primary: true,
        })
    }

    fn build_runtime<W>(root: W) -> (Runtime, WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Drag Drop").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn has_fill_color(output: &sui_runtime::RenderOutput, color: sui_core::Color) -> bool {
        let mut found = false;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::FillRect {
                    brush: Brush::Solid(fill),
                    ..
                }
                | SceneCommand::FillPath {
                    brush: Brush::Solid(fill),
                    ..
                }
                | SceneCommand::FillRoundedRect {
                    brush: Brush::Solid(fill),
                    ..
                } if *fill == color => found = true,
                _ => {}
            });
        found
    }

    fn has_stroke_color(output: &sui_runtime::RenderOutput, color: sui_core::Color) -> bool {
        let mut found = false;
        output
            .frame
            .scene
            .visit_commands(&mut |command| match command {
                SceneCommand::StrokeRect {
                    brush: Brush::Solid(stroke),
                    ..
                }
                | SceneCommand::StrokePath {
                    brush: Brush::Solid(stroke),
                    ..
                } if *stroke == color => found = true,
                _ => {}
            });
        found
    }

    fn feedback_fill_precedes_text(
        output: &sui_runtime::RenderOutput,
        color: sui_core::Color,
    ) -> bool {
        let mut command_index = 0;
        let mut fill_index = None;
        let mut text_index = None;
        output.frame.scene.visit_commands(&mut |command| {
            match command {
                SceneCommand::FillRect {
                    brush: Brush::Solid(fill),
                    ..
                }
                | SceneCommand::FillPath {
                    brush: Brush::Solid(fill),
                    ..
                }
                | SceneCommand::FillRoundedRect {
                    brush: Brush::Solid(fill),
                    ..
                } if *fill == color => {
                    fill_index.get_or_insert(command_index);
                }
                SceneCommand::DrawText(_)
                | SceneCommand::DrawShapedText(_)
                | SceneCommand::DrawShapedTextWindow(_) => {
                    text_index.get_or_insert(command_index);
                }
                _ => {}
            };
            command_index += 1;
        });
        matches!((fill_index, text_index), (Some(fill), Some(text)) if fill < text)
    }

    #[test]
    fn drag_preview_theme_tracks_static_and_dynamic_small_tokens() {
        let mut static_theme = DefaultTheme::default();
        static_theme.text.sm = ThemeTextToken {
            size: 17.0,
            line_height: 24.0,
        };
        let host = DragDropHost::new(
            DragDropScope::new(),
            SizedBox::new().width(80.0).height(40.0),
        )
        .theme(static_theme);
        let resolved = host.preview_theme.borrow().resolve();
        let style = drag_preview_text_style(resolved);
        assert_eq!(style.font_size, static_theme.text.sm.size);
        assert_eq!(style.line_height, static_theme.text.sm.line_height);
        assert_eq!(style.color, static_theme.palette.text);

        let current = Rc::new(RefCell::new(DefaultTheme::default()));
        let dynamic_host = DragDropHost::new(
            DragDropScope::new(),
            SizedBox::new().width(80.0).height(40.0),
        )
        .theme_when({
            let current = Rc::clone(&current);
            move || *current.borrow()
        });
        current.borrow_mut().text.sm = ThemeTextToken {
            size: 19.0,
            line_height: 27.0,
        };

        let resolved = dynamic_host.preview_theme.borrow().resolve();
        let style = drag_preview_text_style(resolved);
        assert_eq!(style.font_size, 19.0);
        assert_eq!(style.line_height, 27.0);
    }

    #[test]
    fn draggable_starts_after_threshold_and_drops_typed_payload() -> Result<()> {
        let scope = DragDropScope::new();
        let starts = Rc::new(RefCell::new(0));
        let ends = Rc::new(RefCell::new(Vec::new()));
        let drops = Rc::new(RefCell::new(Vec::new()));
        let hover_changes = Rc::new(RefCell::new(Vec::new()));

        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::custom("asset", 42_u32))
            .effect(DropEffect::Move)
            .preview_label("Asset")
            .on_drag_start({
                let starts = Rc::clone(&starts);
                move |_, _| *starts.borrow_mut() += 1
            })
            .on_drag_end({
                let ends = Rc::clone(&ends);
                move |_, drag| ends.borrow_mut().push(drag.outcome)
            });

        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept(|drag| {
                if drag.payload.custom_data::<u32>() == Some(&42) {
                    DropEffect::Move
                } else {
                    DropEffect::None
                }
            })
            .on_drop({
                let drops = Rc::clone(&drops);
                move |_, drag| {
                    drops
                        .borrow_mut()
                        .push(*drag.payload.custom_data::<u32>().unwrap());
                }
            })
            .on_hover_change({
                let hover_changes = Rc::clone(&hover_changes);
                move |hovered| hover_changes.borrow_mut().push(hovered)
            });

        let root = DragDropHost::new(
            scope.clone(),
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(12.0, 20.0), true),
        )?;

        assert_eq!(*starts.borrow(), 0);
        assert!(scope.active_drag().is_none());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        assert_eq!(*starts.borrow(), 1);
        assert!(scope.active_drag().is_some());

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;

        assert_eq!(&*drops.borrow(), &[42]);
        assert!(matches!(
            ends.borrow().as_slice(),
            [Some(DragOutcome::Dropped {
                effect: DropEffect::Move,
                ..
            })]
        ));
        assert_eq!(&*hover_changes.borrow(), &[true, false]);
        assert!(scope.active_drag().is_none());
        Ok(())
    }

    #[test]
    fn accepting_drop_target_has_no_default_hover_feedback() -> Result<()> {
        let scope = DragDropScope::new();
        let theme = DefaultTheme::default();
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("item"))
            .effect(DropEffect::Copy)
            .preview_label("Item");
        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept(|_| DropEffect::Copy);
        let root = DragDropHost::new(
            scope,
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);

        let idle = runtime.render(window_id)?;
        assert!(!has_fill_color(&idle, theme.palette.accent_soft));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;

        let hovering = runtime.render(window_id)?;
        assert!(!has_fill_color(&hovering, theme.palette.accent_soft));
        Ok(())
    }

    #[test]
    fn drop_target_hover_callback_can_drive_child_feedback_behind_content() -> Result<()> {
        let scope = DragDropScope::new();
        let theme = DefaultTheme::default();
        let hovered = Rc::new(RefCell::new(false));
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("item"))
            .effect(DropEffect::Copy)
            .preview_label("Item");
        let surface_hovered = Rc::clone(&hovered);
        let target = DropTarget::new(
            Surface::panel(
                SizedBox::new()
                    .width(80.0)
                    .height(40.0)
                    .child(Label::new("Drop here")),
            )
            .theme_when(move || {
                let mut resolved = theme;
                if *surface_hovered.borrow() {
                    resolved.surfaces.panel = resolved.palette.accent_soft;
                    resolved.surfaces.border = resolved.palette.accent_border_focus;
                }
                resolved
            }),
        )
        .scope(scope.clone())
        .accept(|_| DropEffect::Copy)
        .on_hover_change(move |value| *hovered.borrow_mut() = value);
        let root = DragDropHost::new(
            scope,
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let idle = runtime.render(window_id)?;
        assert!(!has_fill_color(&idle, theme.palette.accent_soft));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;

        let hovering = runtime.render(window_id)?;
        assert!(has_fill_color(&hovering, theme.palette.accent_soft));
        assert!(has_stroke_color(
            &hovering,
            theme.palette.accent_border_focus
        ));
        assert!(feedback_fill_precedes_text(
            &hovering,
            theme.palette.accent_soft
        ));
        Ok(())
    }

    #[test]
    fn nearest_accepting_drop_target_receives_drop() -> Result<()> {
        let scope = DragDropScope::new();
        let inner_drops = Rc::new(RefCell::new(0));
        let outer_drops = Rc::new(RefCell::new(0));

        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("item"));

        let inner = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept(|_| DropEffect::Copy)
            .on_drop({
                let inner_drops = Rc::clone(&inner_drops);
                move |_, _| *inner_drops.borrow_mut() += 1
            });
        let outer = DropTarget::new(inner)
            .scope(scope.clone())
            .accept(|_| DropEffect::Copy)
            .on_drop({
                let outer_drops = Rc::clone(&outer_drops);
                move |_, _| *outer_drops.borrow_mut() += 1
            });

        let root = DragDropHost::new(
            scope,
            Stack::horizontal().with_child(source).with_child(outer),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;

        assert_eq!(*inner_drops.borrow(), 1);
        assert_eq!(*outer_drops.borrow(), 0);
        Ok(())
    }

    #[test]
    fn cross_scope_target_ignores_drag_session() -> Result<()> {
        let source_scope = DragDropScope::new();
        let target_scope = DragDropScope::new();
        let ends = Rc::new(RefCell::new(Vec::new()));
        let drops = Rc::new(RefCell::new(0));

        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(source_scope.clone())
            .payload(|| DragPayload::text("item"))
            .on_drag_end({
                let ends = Rc::clone(&ends);
                move |_, drag| ends.borrow_mut().push(drag.outcome)
            });
        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(target_scope)
            .accept(|_| DropEffect::Copy)
            .on_drop({
                let drops = Rc::clone(&drops);
                move |_, _| *drops.borrow_mut() += 1
            });

        let root = DragDropHost::new(
            source_scope,
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;

        assert_eq!(*drops.borrow(), 0);
        assert_eq!(&*ends.borrow(), &[Some(DragOutcome::Cancelled)]);
        Ok(())
    }

    #[test]
    fn drag_drop_host_preview_layer_is_not_hit_tested() -> Result<()> {
        let scope = DragDropScope::new();
        let starts = Rc::new(RefCell::new(0));
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("item"))
            .on_drag_start({
                let starts = Rc::clone(&starts);
                move |_, _| *starts.borrow_mut() += 1
            });
        let root = DragDropHost::new(scope, source);
        let (mut runtime, window_id) = build_runtime(root);

        let output = runtime.render(window_id)?;
        let mut preview_layer = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.descriptor.composition_mode == LayerCompositionMode::Overlay {
                preview_layer = Some((layer.widget_id(), layer.descriptor.hit_test));
            }
        });

        assert!(matches!(preview_layer, Some((_, false))));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(24.0, 20.0), true),
        )?;

        assert_eq!(*starts.borrow(), 1);
        Ok(())
    }

    #[test]
    fn drag_drop_host_routes_platform_file_hover_drop_and_cancel() -> Result<()> {
        let hovered = Rc::new(RefCell::new(Vec::<Vec<PathBuf>>::new()));
        let dropped = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
        let cancelled = Rc::new(RefCell::new(0));
        let root = DragDropHost::new(
            DragDropScope::new(),
            SizedBox::new().width(160.0).height(80.0),
        )
        .on_external_file_hover({
            let hovered = Rc::clone(&hovered);
            move |_, paths| hovered.borrow_mut().push(paths.to_vec())
        })
        .on_external_file_drop({
            let dropped = Rc::clone(&dropped);
            move |_, path| dropped.borrow_mut().push(path)
        })
        .on_external_file_hover_cancelled({
            let cancelled = Rc::clone(&cancelled);
            move |_| *cancelled.borrow_mut() += 1
        });
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;
        let first = PathBuf::from("/tmp/first.txt");
        let second = PathBuf::from("/tmp/second.png");

        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ExternalFileHovered(first.clone())),
        )?;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ExternalFileHovered(second.clone())),
        )?;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ExternalFileDropped(first.clone())),
        )?;
        runtime.handle_event(
            window_id,
            Event::Window(WindowEvent::ExternalFileHoverCancelled),
        )?;

        assert_eq!(
            hovered.borrow().as_slice(),
            &[vec![first.clone()], vec![first.clone(), second]]
        );
        assert_eq!(dropped.borrow().as_slice(), &[first]);
        assert_eq!(*cancelled.borrow(), 1);
        Ok(())
    }

    fn with_modifiers(event: Event, modifiers: Modifiers) -> Event {
        match event {
            Event::Pointer(mut pointer) => {
                pointer.modifiers = modifiers;
                Event::Pointer(pointer)
            }
            other => other,
        }
    }

    /// The key that asks for a copy on this platform, and the modifiers
    /// held while it is down.
    fn copy_key() -> (&'static str, Modifiers) {
        let mut modifiers = Modifiers::NONE;
        if cfg!(target_os = "macos") {
            modifiers.alt = true;
            ("Alt", modifiers)
        } else {
            modifiers.control = true;
            ("Control", modifiers)
        }
    }

    fn key(name: &str, state: KeyState, modifiers: Modifiers) -> Event {
        let mut event = KeyboardEvent::new(name, state);
        event.modifiers = modifiers;
        Event::Keyboard(event)
    }

    /// A source at x 0–80 and a target at 80–160, both 40 tall, sharing a
    /// host. The target records the effect it accepts each time it is asked.
    struct SourceAndTarget {
        runtime: Runtime,
        window_id: WindowId,
        scope: DragDropScope,
        accepted: Rc<RefCell<Vec<DropEffect>>>,
        hover_changes: Rc<RefCell<Vec<bool>>>,
        drops: Rc<RefCell<Vec<DropEffect>>>,
        ends: Rc<RefCell<Vec<Option<DragOutcome>>>>,
    }

    fn source_and_target(effects: DropEffects) -> Result<SourceAndTarget> {
        let scope = DragDropScope::new();
        let accepted = Rc::new(RefCell::new(Vec::new()));
        let hover_changes = Rc::new(RefCell::new(Vec::new()));
        let drops = Rc::new(RefCell::new(Vec::new()));
        let ends = Rc::new(RefCell::new(Vec::new()));
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("card"))
            .effects(effects)
            .on_drag_end({
                let ends = Rc::clone(&ends);
                move |_, drag| ends.borrow_mut().push(drag.outcome)
            });
        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept({
                let accepted = Rc::clone(&accepted);
                move |drag| {
                    let effect = drag.preferred_effect();
                    accepted.borrow_mut().push(effect);
                    effect
                }
            })
            .on_hover_change({
                let hover_changes = Rc::clone(&hover_changes);
                move |hovered| hover_changes.borrow_mut().push(hovered)
            })
            .on_drop({
                let drops = Rc::clone(&drops);
                move |_, drag| drops.borrow_mut().push(drag.accepted_effect)
            });
        let root = DragDropHost::new(
            scope.clone(),
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;
        Ok(SourceAndTarget {
            runtime,
            window_id,
            scope,
            accepted,
            hover_changes,
            drops,
            ends,
        })
    }

    /// Press on the source and drag onto the target.
    fn drag_onto_target(harness: &mut SourceAndTarget) -> Result<()> {
        let window_id = harness.window_id;
        harness.runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        harness.runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(100.0, 20.0), true),
        )?;
        Ok(())
    }

    #[test]
    fn held_modifiers_choose_among_the_allowed_effects() -> Result<()> {
        let mut harness = source_and_target(DropEffects::MOVE | DropEffects::COPY)?;
        let window_id = harness.window_id;
        let (copy_key, copy_modifiers) = copy_key();
        drag_onto_target(&mut harness)?;
        assert_eq!(harness.accepted.borrow().last(), Some(&DropEffect::Move));

        // Holding the copy key sends the target a fresh Over without the
        // pointer moving; letting go turns it back into a move.
        harness
            .runtime
            .handle_event(window_id, key(copy_key, KeyState::Pressed, copy_modifiers))?;
        assert_eq!(harness.accepted.borrow().last(), Some(&DropEffect::Copy));
        harness.runtime.handle_event(
            window_id,
            key(copy_key, KeyState::Released, Modifiers::NONE),
        )?;
        assert_eq!(harness.accepted.borrow().last(), Some(&DropEffect::Move));

        harness.runtime.handle_event(
            window_id,
            with_modifiers(
                primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
                copy_modifiers,
            ),
        )?;
        assert_eq!(&*harness.drops.borrow(), &[DropEffect::Copy]);
        assert!(matches!(
            harness.ends.borrow().as_slice(),
            [Some(DragOutcome::Dropped {
                effect: DropEffect::Copy,
                ..
            })]
        ));
        Ok(())
    }

    #[test]
    fn modifiers_cannot_ask_for_an_effect_the_source_does_not_allow() -> Result<()> {
        let mut harness = source_and_target(DropEffect::Move.into())?;
        let (copy_key, copy_modifiers) = copy_key();
        drag_onto_target(&mut harness)?;
        harness.runtime.handle_event(
            harness.window_id,
            key(copy_key, KeyState::Pressed, copy_modifiers),
        )?;
        assert!(
            harness
                .accepted
                .borrow()
                .iter()
                .all(|effect| *effect == DropEffect::Move)
        );
        Ok(())
    }

    #[test]
    fn escape_cancels_the_drag() -> Result<()> {
        let mut harness = source_and_target(DropEffects::MOVE)?;
        let window_id = harness.window_id;
        drag_onto_target(&mut harness)?;
        assert_eq!(&*harness.hover_changes.borrow(), &[true]);

        harness
            .runtime
            .handle_event(window_id, key("Escape", KeyState::Pressed, Modifiers::NONE))?;
        assert_eq!(&*harness.ends.borrow(), &[Some(DragOutcome::Cancelled)]);
        assert_eq!(&*harness.hover_changes.borrow(), &[true, false]);
        assert!(harness.scope.active_drag().is_none());

        // Letting go afterwards drops nothing.
        harness.runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(100.0, 20.0), false),
        )?;
        assert!(harness.drops.borrow().is_empty());
        assert_eq!(harness.ends.borrow().len(), 1);
        Ok(())
    }

    #[test]
    fn losing_window_focus_cancels_the_drag() -> Result<()> {
        let mut harness = source_and_target(DropEffects::MOVE)?;
        drag_onto_target(&mut harness)?;
        harness.runtime.handle_event(
            harness.window_id,
            Event::Window(WindowEvent::Focused(false)),
        )?;
        assert_eq!(&*harness.ends.borrow(), &[Some(DragOutcome::Cancelled)]);
        assert_eq!(&*harness.hover_changes.borrow(), &[true, false]);
        assert!(harness.scope.active_drag().is_none());
        Ok(())
    }

    #[test]
    fn drop_targets_report_drags_they_refuse() -> Result<()> {
        let scope = DragDropScope::new();
        let states = Rc::new(RefCell::new(Vec::new()));
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::text("card"));
        let target = DropTarget::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .accept(|drag| {
                if drag.payload.as_text().is_some() {
                    DropEffect::None
                } else {
                    DropEffect::Copy
                }
            })
            .on_hover_state({
                let states = Rc::clone(&states);
                move |state| states.borrow_mut().push(state)
            });
        let root = DragDropHost::new(
            scope,
            Stack::horizontal().with_child(source).with_child(target),
        );
        let (mut runtime, window_id) = build_runtime(root);
        let _ = runtime.render(window_id)?;

        for (kind, x, pressed) in [
            (PointerEventKind::Down, 10.0, true),
            (PointerEventKind::Move, 100.0, true),
            (PointerEventKind::Move, 20.0, true),
            (PointerEventKind::Move, 110.0, true),
            (PointerEventKind::Up, 110.0, false),
        ] {
            runtime.handle_event(
                window_id,
                primary_pointer(kind, Point::new(x, 20.0), pressed),
            )?;
        }
        assert_eq!(
            &*states.borrow(),
            &[
                DropHover::Refusing,
                DropHover::Idle,
                DropHover::Refusing,
                DropHover::Idle,
            ]
        );
        Ok(())
    }

    #[test]
    fn modifier_conventions_follow_the_platform() {
        let mut copy = Modifiers::NONE;
        let mut moving = Modifiers::NONE;
        if cfg!(target_os = "macos") {
            copy.alt = true;
            moving.meta = true;
        } else {
            copy.control = true;
            moving.shift = true;
        }
        let mut both = copy;
        both.alt |= moving.alt;
        both.meta |= moving.meta;
        both.control |= moving.control;
        both.shift |= moving.shift;
        assert_eq!(DropEffect::for_modifiers(Modifiers::NONE), None);
        assert_eq!(DropEffect::for_modifiers(copy), Some(DropEffect::Copy));
        assert_eq!(DropEffect::for_modifiers(moving), Some(DropEffect::Move));
        assert_eq!(DropEffect::for_modifiers(both), Some(DropEffect::Link));

        let effects = DropEffects::COPY | DropEffects::LINK;
        assert!(effects.contains(DropEffect::Link));
        assert!(!effects.contains(DropEffect::Move));
        assert!(!effects.contains(DropEffect::None));
        assert_eq!(effects.default_effect(), DropEffect::Copy);
        assert_eq!(DropEffects::ALL.default_effect(), DropEffect::Move);
        assert_eq!(DropEffects::NONE.default_effect(), DropEffect::None);
    }

    /// A fixed-size block of one color.
    struct Swatch {
        color: Color,
        size: Size,
    }

    impl Widget for Swatch {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(self.size)
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill(Path::rounded_rect(ctx.bounds(), 0.0), self.color);
        }
    }

    #[test]
    fn host_draws_a_built_preview_widget_instead_of_the_label() -> Result<()> {
        let scope = DragDropScope::new();
        let swatch = Color::srgba(0.05, 0.78, 0.35, 1.0);
        let built = Rc::new(RefCell::new(Vec::new()));
        let theme = DefaultTheme::default();
        let label_fill = theme.palette.surface_raised.with_alpha(0.96);
        let source = Draggable::new(SizedBox::new().width(80.0).height(40.0))
            .scope(scope.clone())
            .payload(|| DragPayload::custom("asset", 7_u32))
            .preview_label("Asset");
        let root = DragDropHost::new(scope.clone(), Stack::horizontal().with_child(source))
            .theme(theme)
            .preview({
                let built = Rc::clone(&built);
                move |preview| {
                    let asset = *preview.payload.custom_data::<u32>()?;
                    built.borrow_mut().push(asset);
                    Some(Box::new(Swatch {
                        color: swatch,
                        size: Size::new(30.0, 20.0),
                    }) as Box<dyn Widget>)
                }
            });
        let (mut runtime, window_id) = build_runtime(root);
        let idle = runtime.render(window_id)?;
        assert!(!has_fill_color(&idle, swatch));

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        for x in [40.0, 60.0, 90.0] {
            runtime.handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, Point::new(x, 20.0), true),
            )?;
        }
        let dragging = runtime.render(window_id)?;
        assert_eq!(&*built.borrow(), &[7], "built once for the drag");
        assert!(has_fill_color(&dragging, swatch));
        assert!(!has_fill_color(&dragging, label_fill), "no label box");

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Up, Point::new(90.0, 20.0), false),
        )?;
        let after = runtime.render(window_id)?;
        assert!(!has_fill_color(&after, swatch));
        Ok(())
    }

    #[test]
    fn previews_stay_under_the_pointer_inside_the_host() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
        let size = Size::new(60.0, 30.0);
        assert_eq!(
            drag_preview_origin(Point::new(20.0, 10.0), size, bounds),
            Point::new(32.0, 22.0)
        );
        // Near the far corner the preview moves back inside.
        assert_eq!(
            drag_preview_origin(Point::new(190.0, 95.0), size, bounds),
            Point::new(140.0, 70.0)
        );
    }

    #[test]
    fn scroll_view_scrolls_while_a_drag_holds_near_its_edge() -> Result<()> {
        let scope = DragDropScope::new();
        let state = ScrollState::new();
        let hover_changes = Rc::new(RefCell::new(Vec::new()));
        let content = Stack::vertical()
            .with_child(
                Draggable::new(SizedBox::new().width(200.0).height(40.0))
                    .scope(scope.clone())
                    .payload(|| DragPayload::text("card")),
            )
            .with_child(SizedBox::new().width(200.0).height(400.0))
            .with_child(
                DropTarget::new(SizedBox::new().width(200.0).height(40.0))
                    .scope(scope.clone())
                    .on_hover_change({
                        let hover_changes = Rc::clone(&hover_changes);
                        move |hovered| hover_changes.borrow_mut().push(hovered)
                    }),
            );
        let scroll = SizedBox::new()
            .width(220.0)
            .height(200.0)
            .child(ScrollView::new(content).state(state.clone()));
        let root = DragDropHost::new(scope, Stack::vertical().with_child(scroll));
        let (mut runtime, window_id) = build_runtime(root);
        runtime.set_frame_pacing(window_id, FramePacing::Display)?;
        let _ = runtime.render(window_id)?;

        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Down, Point::new(10.0, 20.0), true),
        )?;
        // Held 10 px from the bottom edge.
        runtime.handle_event(
            window_id,
            primary_pointer(PointerEventKind::Move, Point::new(10.0, 190.0), true),
        )?;
        let mut time = 0.0;
        for _ in 0..120 {
            time += 1.0 / 60.0;
            let events = runtime.begin_animation_frame(window_id, time)?;
            if events.is_empty() {
                break;
            }
            for event in events {
                runtime.handle_event(window_id, event)?;
            }
            let _ = runtime.render(window_id)?;
        }

        // 480 px of content in a 200 px view scrolls to its end, where the
        // target comes under the pointer that never moved.
        assert_eq!(state.current_offset().y, 280.0);
        assert_eq!(&*hover_changes.borrow(), &[true]);
        assert!(
            !runtime.has_pending_animation_frames(window_id)?,
            "scrolling stops at the end"
        );
        Ok(())
    }

    #[test]
    fn drag_payload_custom_equality_uses_stable_kind() {
        let left = DragPayload::custom("asset", 1_u32);
        let right = DragPayload::custom("asset", 2_u32);
        let other = DragPayload::custom("other", 1_u32);

        assert_eq!(left, right);
        assert_ne!(left, other);
        assert_eq!(left.custom_data::<u32>(), Some(&1));
        assert_eq!(right.custom_data::<u32>(), Some(&2));
    }
}
