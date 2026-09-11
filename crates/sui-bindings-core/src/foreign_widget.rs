use crate::errors::{
    ForeignCallbackError, ForeignCallbackPhase, ForeignCallbackResult, ForeignErrorSink,
    ForeignWidgetId,
};
use crate::paint::{
    PaintCommand, PaintStackState, PaintValidationResult, validate_paint_command,
    validate_paint_command_with_stack,
};
use crate::support::{panic_message, run_foreign_callback};
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;
use sui::ArrangeCtx;
use sui::Constraints;
use sui::DpiInfo;
use sui::Event;
use sui::EventCtx;
use sui::EventPhase;
use sui::ImageHandle;
use sui::InvalidationRequest;
use sui::MeasureCtx;
use sui::PaintCtx;
use sui::Rect;
use sui::RegisteredImage;
use sui::SemanticsCtx;
use sui::SemanticsNode;
use sui::Size;
use sui::TimerToken;
use sui::Widget;
use sui::WidgetId;
use sui::WidgetPod;
use sui::WidgetPodMutVisitor;
use sui::WidgetPodVisitor;
use sui::WindowId;

pub trait ForeignWidgetCallbacks: Send + Sync + 'static {
    fn debug_name(&self, _id: ForeignWidgetId) -> &'static str {
        "sui_bindings_core::ForeignWidget"
    }

    fn event(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignEventCtx<'_>,
        _event: &Event,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }

    fn measure(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        Ok(constraints.max)
    }

    fn arrange(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignArrangeCtx<'_>,
        _bounds: Rect,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }

    fn semantics(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignSemanticsCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        Ok(())
    }
}

pub struct ForeignWidget {
    pub(crate) id: ForeignWidgetId,
    pub(crate) callbacks: Arc<dyn ForeignWidgetCallbacks>,
    pub(crate) children: Vec<WidgetPod>,
    pub(crate) errors: ForeignErrorSink,
}

impl ForeignWidget {
    pub fn new(callbacks: impl ForeignWidgetCallbacks) -> Self {
        Self::from_arc(Arc::new(callbacks))
    }

    pub fn from_arc(callbacks: Arc<dyn ForeignWidgetCallbacks>) -> Self {
        Self {
            id: ForeignWidgetId::default(),
            callbacks,
            children: Vec::new(),
            errors: ForeignErrorSink::new(),
        }
    }

    pub fn with_id(mut self, id: ForeignWidgetId) -> Self {
        self.id = id;
        self
    }

    pub fn with_error_sink(mut self, errors: ForeignErrorSink) -> Self {
        self.errors = errors;
        self
    }

    pub fn with_child(mut self, child: impl Widget + 'static) -> Self {
        self.push_child(child);
        self
    }

    pub fn push_child(&mut self, child: impl Widget + 'static) {
        self.children.push(WidgetPod::new(child));
    }

    pub fn push_child_pod(&mut self, child: WidgetPod) {
        self.children.push(child);
    }

    pub const fn foreign_id(&self) -> ForeignWidgetId {
        self.id
    }

    pub fn error_sink(&self) -> ForeignErrorSink {
        self.errors.clone()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub(crate) fn record_panic(
        &self,
        phase: ForeignCallbackPhase,
        payload: Box<dyn std::any::Any + Send>,
    ) {
        self.errors.push(ForeignCallbackError::new(
            self.id,
            phase,
            panic_message(payload),
        ));
    }

    pub(crate) fn run_callback<T>(
        &self,
        phase: ForeignCallbackPhase,
        fallback: T,
        callback: impl FnOnce() -> ForeignCallbackResult<T>,
    ) -> T {
        run_foreign_callback(self.id, &self.errors, phase, fallback, callback)
    }
}

impl Widget for ForeignWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let callbacks = Arc::clone(&self.callbacks);
        let mut foreign_ctx = ForeignEventCtx { inner: ctx };
        self.run_callback(ForeignCallbackPhase::Event, (), || {
            callbacks.event(self.id, &mut foreign_ctx, event)
        });
    }

    fn debug_name(&self) -> &'static str {
        let callbacks = Arc::clone(&self.callbacks);
        match catch_unwind(AssertUnwindSafe(|| callbacks.debug_name(self.id))) {
            Ok(name) => name,
            Err(payload) => {
                self.record_panic(ForeignCallbackPhase::DebugName, payload);
                "sui_bindings_core::ForeignWidget"
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let callbacks = Arc::clone(&self.callbacks);
        let id = self.id;
        let errors = self.errors.clone();
        let mut foreign_ctx = ForeignMeasureCtx {
            inner: ctx,
            children: &mut self.children,
        };
        let fallback = constraints.clamp(Size::ZERO);
        run_foreign_callback(id, &errors, ForeignCallbackPhase::Measure, fallback, || {
            callbacks.measure(id, &mut foreign_ctx, constraints)
        })
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let callbacks = Arc::clone(&self.callbacks);
        let id = self.id;
        let errors = self.errors.clone();
        let mut foreign_ctx = ForeignArrangeCtx {
            inner: ctx,
            children: &mut self.children,
        };
        run_foreign_callback(id, &errors, ForeignCallbackPhase::Arrange, (), || {
            callbacks.arrange(id, &mut foreign_ctx, bounds)
        });
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let callbacks = Arc::clone(&self.callbacks);
        let mut foreign_ctx = ForeignPaintCtx {
            inner: ctx,
            children: &self.children,
        };
        self.run_callback(ForeignCallbackPhase::Paint, (), || {
            callbacks.paint(self.id, &mut foreign_ctx)
        });
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let callbacks = Arc::clone(&self.callbacks);
        let mut foreign_ctx = ForeignSemanticsCtx {
            inner: ctx,
            children: &self.children,
        };
        self.run_callback(ForeignCallbackPhase::Semantics, (), || {
            callbacks.semantics(self.id, &mut foreign_ctx)
        });
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for child in &self.children {
            visitor.visit(child);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for child in &mut self.children {
            visitor.visit(child);
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingEventContext {
    pub window_id: u64,
    pub widget_id: u64,
    pub bounds: Rect,
    pub current_time: f64,
    pub phase: &'static str,
    pub focused: bool,
    pub clipboard_text: Option<String>,
    pub(crate) handled: bool,
    pub(crate) focus_request: BindingFocusRequest,
    pub(crate) request_measure: bool,
    pub(crate) request_arrange: bool,
    pub(crate) request_paint: bool,
    pub(crate) request_paint_rect: Option<Rect>,
    pub(crate) request_semantics: bool,
    pub(crate) request_animation_frame: bool,
    pub(crate) capture_pointers: Vec<u64>,
    pub(crate) release_pointers: Vec<u64>,
    pub(crate) next_clipboard_text: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum BindingFocusRequest {
    #[default]
    None,
    Focus,
    Clear,
}

impl BindingEventContext {
    pub fn from_foreign(ctx: &ForeignEventCtx<'_>) -> Self {
        Self {
            window_id: ctx.window_id().get(),
            widget_id: ctx.widget_id().get(),
            bounds: ctx.bounds(),
            current_time: ctx.current_time(),
            phase: match ctx.phase() {
                EventPhase::Capture => "capture",
                EventPhase::Target => "target",
                EventPhase::Bubble => "bubble",
            },
            focused: ctx.is_focused(),
            clipboard_text: ctx.clipboard_text(),
            handled: false,
            focus_request: BindingFocusRequest::None,
            request_measure: false,
            request_arrange: false,
            request_paint: false,
            request_paint_rect: None,
            request_semantics: false,
            request_animation_frame: false,
            capture_pointers: Vec::new(),
            release_pointers: Vec::new(),
            next_clipboard_text: None,
        }
    }

    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    pub fn request_focus(&mut self) {
        self.focus_request = BindingFocusRequest::Focus;
    }

    pub fn clear_focus(&mut self) {
        self.focus_request = BindingFocusRequest::Clear;
    }

    pub fn request_measure(&mut self) {
        self.request_measure = true;
    }

    pub fn request_arrange(&mut self) {
        self.request_arrange = true;
    }

    pub fn request_paint(&mut self) {
        self.request_paint = true;
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request_paint_rect = Some(rect);
    }

    pub fn request_semantics(&mut self) {
        self.request_semantics = true;
    }

    pub fn request_animation_frame(&mut self) {
        self.request_animation_frame = true;
    }

    pub fn capture_pointer(&mut self, pointer_id: u64) {
        self.capture_pointers.push(pointer_id);
    }

    pub fn release_pointer(&mut self, pointer_id: u64) {
        self.release_pointers.push(pointer_id);
    }

    pub fn set_clipboard_text(&mut self, text: impl Into<String>) {
        self.next_clipboard_text = Some(text.into());
    }

    pub fn apply(&self, ctx: &mut ForeignEventCtx<'_>) {
        if self.handled {
            ctx.set_handled();
        }
        match self.focus_request {
            BindingFocusRequest::None => {}
            BindingFocusRequest::Focus => ctx.request_focus(),
            BindingFocusRequest::Clear => ctx.clear_focus(),
        }
        if self.request_measure {
            ctx.request_measure();
        }
        if self.request_arrange {
            ctx.request_arrange();
        }
        if self.request_paint {
            ctx.request_paint();
        }
        if let Some(rect) = self.request_paint_rect {
            ctx.request_paint_rect(rect);
        }
        if self.request_semantics {
            ctx.request_semantics();
        }
        if self.request_animation_frame {
            ctx.request_animation_frame();
        }
        for pointer_id in &self.capture_pointers {
            ctx.request_pointer_capture(*pointer_id);
        }
        for pointer_id in &self.release_pointers {
            ctx.release_pointer_capture(*pointer_id);
        }
        if let Some(text) = &self.next_clipboard_text {
            ctx.set_clipboard_text(text);
        }
    }
}

pub struct ForeignEventCtx<'a> {
    pub(crate) inner: &'a mut EventCtx,
}

impl ForeignEventCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn current_time(&self) -> f64 {
        self.inner.current_time()
    }

    pub fn phase(&self) -> EventPhase {
        self.inner.phase()
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn clipboard_text(&self) -> Option<String> {
        self.inner.clipboard_text()
    }

    pub fn set_clipboard_text(&mut self, text: impl AsRef<str>) {
        self.inner.set_clipboard_text(text);
    }

    pub fn set_handled(&mut self) {
        self.inner.set_handled();
    }

    pub fn request_focus(&mut self) {
        self.inner.request_focus();
    }

    pub fn request_focus_for(&mut self, widget_id: WidgetId) {
        self.inner.request_focus_for(widget_id);
    }

    pub fn clear_focus(&mut self) {
        self.inner.clear_focus();
    }

    pub fn request_measure(&mut self) {
        self.inner.request_measure();
    }

    pub fn request_arrange(&mut self) {
        self.inner.request_arrange();
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.inner.request_paint_rect(rect);
    }

    pub fn request_semantics(&mut self) {
        self.inner.request_semantics();
    }

    pub fn request_animation_frame(&mut self) {
        self.inner.request_animation_frame();
    }

    pub fn request_pointer_capture(&mut self, pointer_id: u64) {
        self.inner.request_pointer_capture(pointer_id);
    }

    pub fn release_pointer_capture(&mut self, pointer_id: u64) {
        self.inner.release_pointer_capture(pointer_id);
    }

    pub fn schedule_timer_after(&mut self, delay: f64) -> TimerToken {
        self.inner.schedule_timer_after(delay)
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.inner.request(request);
    }
}

pub struct ForeignMeasureCtx<'a> {
    pub(crate) inner: &'a mut MeasureCtx,
    pub(crate) children: &'a mut [WidgetPod],
}

impl ForeignMeasureCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn current_time(&self) -> f64 {
        self.inner.current_time()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn measure_child(&mut self, index: usize, constraints: Constraints) -> Option<Size> {
        self.children
            .get_mut(index)
            .map(|child| child.measure(self.inner, constraints))
    }

    pub fn request_measure(&mut self) {
        self.inner.request_measure();
    }

    pub fn request_arrange(&mut self) {
        self.inner.request_arrange();
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_semantics(&mut self) {
        self.inner.request_semantics();
    }
}

pub struct ForeignArrangeCtx<'a> {
    pub(crate) inner: &'a mut ArrangeCtx,
    pub(crate) children: &'a mut [WidgetPod],
}

impl ForeignArrangeCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn arrange_child(&mut self, index: usize, bounds: Rect) -> bool {
        let Some(child) = self.children.get_mut(index) else {
            return false;
        };
        child.arrange(self.inner, bounds);
        true
    }

    pub fn set_child_bounds(&mut self, index: usize, bounds: Rect) -> bool {
        let Some(child) = self.children.get_mut(index) else {
            return false;
        };
        child.set_bounds(bounds);
        true
    }

    pub fn request_arrange(&mut self) {
        self.inner.request_arrange();
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_semantics(&mut self) {
        self.inner.request_semantics();
    }
}

pub struct ForeignPaintCtx<'a> {
    pub(crate) inner: &'a mut PaintCtx,
    pub(crate) children: &'a [WidgetPod],
}

impl ForeignPaintCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn dpi(&self) -> DpiInfo {
        self.inner.dpi()
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn paint_child(&mut self, index: usize) -> bool {
        let Some(child) = self.children.get(index) else {
            return false;
        };
        child.paint(self.inner);
        true
    }

    pub fn apply(&mut self, command: PaintCommand) -> PaintValidationResult<()> {
        validate_paint_command(&command)?;
        command.apply(self.inner);
        Ok(())
    }

    pub fn apply_all(
        &mut self,
        commands: impl IntoIterator<Item = PaintCommand>,
    ) -> PaintValidationResult<()> {
        let mut stack = PaintStackState::default();
        let commands = commands.into_iter().collect::<Vec<_>>();
        for command in &commands {
            validate_paint_command_with_stack(command, &mut stack)?;
        }
        stack.finish()?;
        for command in commands {
            command.apply(self.inner);
        }
        Ok(())
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) {
        self.inner.register_image(handle, image);
    }

    pub fn widget_image_handle(&self, slot: u64) -> ImageHandle {
        self.inner.widget_image_handle(slot)
    }

    pub fn request_paint(&mut self) {
        self.inner.request_paint();
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.inner.request_paint_rect(rect);
    }
}

pub struct ForeignSemanticsCtx<'a> {
    pub(crate) inner: &'a mut SemanticsCtx,
    pub(crate) children: &'a [WidgetPod],
}

impl ForeignSemanticsCtx<'_> {
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    pub fn widget_id(&self) -> WidgetId {
        self.inner.widget_id()
    }

    pub fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    pub fn is_focused(&self) -> bool {
        self.inner.is_focused()
    }

    pub fn push(&mut self, node: SemanticsNode) {
        self.inner.push(node);
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn semantics_child(&mut self, index: usize) -> bool {
        let Some(child) = self.children.get(index) else {
            return false;
        };
        child.semantics(self.inner);
        true
    }
}
