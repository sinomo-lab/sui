#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    Command, CommandDelivery, CommandKey, CommandTarget,
    command::{QueuedCommand, queued_command},
    diagnostics::{WidgetTimingPhase, record_widget_timing},
};

use sui_core::{
    AsyncWakeToken, Clipboard, Color, DpiInfo, DragPayload, DragScopeId, DragSessionId, DropEffect,
    Event, ImageHandle, InvalidationKind, InvalidationRequest, InvalidationTarget, Path, Point,
    Rect, SemanticsNode, Size, TimerToken, Transform, Vector, WidgetId, WindowId,
};
use sui_layout::{Constraints, LayoutContext};
use sui_reactive::{Observable, Signal};
use sui_scene::{
    Border, Brush, ImageRegistry, ImageSource, LayerCompositionMode, LayerProperties,
    RegisteredExternalImage, RegisteredImage, Scene, SceneCommand, SceneLayer,
    SceneLayerDescriptor, SceneLayerId, ShadowParams, StrokeStyle, TextRenderPolicy, WidgetShader,
};
use sui_text::{
    FontRegistry, PersistentTextLayout, ShapedText, ShapedTextWindow, TextLayout, TextLayoutHandle,
    TextLayoutRequest, TextMeasurement, TextRun, TextStyle, TextSystem,
};
use web_time::Instant;

static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_TIMER_TOKEN: AtomicU64 = AtomicU64::new(1);
static NEXT_ASYNC_WAKE_TOKEN: AtomicU64 = AtomicU64::new(1);
static NEXT_DRAG_SESSION_ID: AtomicU64 = AtomicU64::new(1);
const WIDGET_IMAGE_HANDLE_NAMESPACE: u64 = 1 << 63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

pub trait WidgetPodVisitor {
    fn visit(&mut self, child: &WidgetPod);
}

pub trait WidgetPodMutVisitor {
    fn visit(&mut self, child: &mut WidgetPod);
}

pub trait Widget {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    /// Receive a typed command addressed directly to this widget. Commands are
    /// target-only and do not capture or bubble through the widget tree.
    fn command(&mut self, _ctx: &mut EventCtx, _command: &Command<'_>) {}

    fn debug_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.max
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _bounds: Rect) {}

    fn paint(&self, _ctx: &mut PaintCtx) {}

    fn layer_options(&self) -> LayerOptions {
        LayerOptions::default()
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
    }

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        None
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        None
    }

    fn semantics(&self, _ctx: &mut SemanticsCtx) {}

    fn accepts_focus(&self) -> bool {
        false
    }

    fn focus_changed(&mut self, _ctx: &mut EventCtx, _focused: bool) {}

    /// Visit this widget's logical children.
    ///
    /// The children do not need to correspond to every piece of state or every
    /// visual element a widget owns internally. A widget may expose retained
    /// local children, generated/virtual children, remote children represented
    /// by local pods, or only the subset that should cooperate with the SUI
    /// runtime for this pass.
    fn visit_children(&self, _visitor: &mut dyn WidgetPodVisitor) {}

    /// Mutably visit this widget's logical children.
    ///
    /// This is the mutable counterpart to [`Widget::visit_children`]; SUI uses
    /// it as a cooperation point for the retained runtime, not as ownership of
    /// a widget's complete internal model.
    fn visit_children_mut(&mut self, _visitor: &mut dyn WidgetPodMutVisitor) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintBoundaryMode {
    Flat,
    Explicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerOptions {
    pub paint_boundary: PaintBoundaryMode,
    pub composition_mode: LayerCompositionMode,
}

impl LayerOptions {
    pub const fn emits_layer(self) -> bool {
        matches!(self.paint_boundary, PaintBoundaryMode::Explicit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackOrderPolicy {
    Stable,
    FocusFronted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackHostOptions {
    pub order_policy: StackOrderPolicy,
}

impl Default for StackHostOptions {
    fn default() -> Self {
        Self {
            order_policy: StackOrderPolicy::Stable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSurfaceOptions {
    pub dynamic_ordering: bool,
    pub transient: bool,
    pub hit_test: bool,
}

impl Default for StackSurfaceOptions {
    fn default() -> Self {
        Self {
            dynamic_ordering: true,
            transient: false,
            hit_test: true,
        }
    }
}

impl Default for LayerOptions {
    fn default() -> Self {
        Self {
            paint_boundary: PaintBoundaryMode::Flat,
            composition_mode: LayerCompositionMode::Normal,
        }
    }
}

pub struct SingleChild {
    child: WidgetPod,
}

impl SingleChild {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self::from_pod(WidgetPod::new(child))
    }

    /// Creates a child whose scene is retained behind an explicit paint boundary.
    ///
    /// This is useful for containers that move their complete child subtree as a
    /// unit. The child's own composition mode and layer properties are preserved;
    /// only a flat paint boundary is promoted to an explicit one.
    pub fn new_with_paint_boundary<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        let mut child = WidgetPod::new(child);
        child.force_paint_boundary = true;
        Self::from_pod(child)
    }

    /// Returns this child wrapper with an explicit paint boundary forced on.
    /// The consuming form keeps layer-topology changes in builder flows.
    pub fn with_paint_boundary(mut self) -> Self {
        self.child.force_paint_boundary = true;
        self
    }

    pub fn from_pod(child: WidgetPod) -> Self {
        Self { child }
    }

    pub fn child(&self) -> &WidgetPod {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        &mut self.child
    }

    pub fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    pub fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        self.child.set_bounds(bounds);
    }

    pub fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
    }

    pub fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        visitor.visit(&self.child);
    }

    pub fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        visitor.visit(&mut self.child);
    }
}

#[derive(Default)]
pub struct WidgetChildren {
    children: Vec<WidgetPod>,
}

impl WidgetChildren {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            children: Vec::with_capacity(capacity),
        }
    }

    pub fn push<W>(&mut self, child: W)
    where
        W: Widget + 'static,
    {
        self.push_pod(WidgetPod::new(child));
    }

    pub fn push_pod(&mut self, child: WidgetPod) {
        self.children.push(child);
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn as_slice(&self) -> &[WidgetPod] {
        &self.children
    }

    pub fn as_mut_slice(&mut self) -> &mut [WidgetPod] {
        &mut self.children
    }

    pub fn measure_child(
        &mut self,
        index: usize,
        ctx: &mut MeasureCtx,
        constraints: Constraints,
    ) -> Size {
        self.children[index].measure(ctx, constraints)
    }

    pub fn arrange_child(&mut self, index: usize, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.children[index].arrange(ctx, bounds);
    }

    pub fn paint(&self, ctx: &mut PaintCtx) {
        for child in &self.children {
            child.paint(ctx);
        }
    }

    pub fn semantics(&self, ctx: &mut SemanticsCtx) {
        for child in &self.children {
            child.semantics(ctx);
        }
    }

    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for child in &self.children {
            visitor.visit(child);
        }
    }

    pub fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for child in &mut self.children {
            visitor.visit(child);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyedReconcile {
    pub inserted: usize,
    pub removed: usize,
    pub moved: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub duplicate_keys: usize,
}

struct KeyedChild<K, T> {
    key: K,
    value: Signal<T>,
    child: WidgetPod,
}

/// Layout-neutral retained child collection reconciled by stable application
/// keys.
///
/// Existing [`WidgetPod`] instances are moved rather than recreated. Each
/// child also receives a per-item [`Signal`] so changing item data can update
/// the retained widget without replacing it.
pub struct KeyedChildren<K, T> {
    entries: Vec<KeyedChild<K, T>>,
}

impl<K, T> KeyedChildren<K, T> {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &Signal<T>, &WidgetPod)> {
        self.entries
            .iter()
            .map(|entry| (&entry.key, &entry.value, &entry.child))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &Signal<T>, &mut WidgetPod)> {
        self.entries
            .iter_mut()
            .map(|entry| (&entry.key, &entry.value, &mut entry.child))
    }

    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for entry in &self.entries {
            visitor.visit(&entry.child);
        }
    }

    pub fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for entry in &mut self.entries {
            visitor.visit(&mut entry.child);
        }
    }
}

impl<K, T> KeyedChildren<K, T>
where
    K: Clone + Eq + Hash,
    T: Clone + PartialEq + 'static,
{
    pub fn reconcile<I, KF, B, W>(
        &mut self,
        items: I,
        mut key_for: KF,
        mut build: B,
    ) -> KeyedReconcile
    where
        I: IntoIterator<Item = T>,
        KF: FnMut(&T) -> K,
        B: FnMut(&K, Signal<T>) -> W,
        W: Widget + 'static,
    {
        let previous_len = self.entries.len();
        let mut previous = std::mem::take(&mut self.entries)
            .into_iter()
            .enumerate()
            .map(|(index, entry)| (entry.key.clone(), (index, entry)))
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::new();
        let mut next = Vec::new();
        let mut report = KeyedReconcile::default();

        for item in items {
            let key = key_for(&item);
            if !seen.insert(key.clone()) {
                report.duplicate_keys += 1;
                continue;
            }

            let next_index = next.len();
            if let Some((previous_index, entry)) = previous.remove(&key) {
                if previous_index != next_index {
                    report.moved += 1;
                }
                if entry.value.set(item) {
                    report.updated += 1;
                } else {
                    report.unchanged += 1;
                }
                next.push(entry);
            } else {
                let value = Signal::named("Keyed item", item);
                let child = WidgetPod::new(build(&key, value.clone()));
                next.push(KeyedChild { key, value, child });
                report.inserted += 1;
            }
        }

        report.removed = previous.len();
        debug_assert_eq!(previous_len + report.inserted, next.len() + report.removed);
        self.entries = next;
        report
    }
}

impl<K, T> Default for KeyedChildren<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard retained-widget adapter used by the SUI runtime.
///
/// `WidgetPod` gives a `Widget` stable identity, cached layout state, event
/// routing participation, and scene/semantics cooperation. It is the default
/// local retained model, but a widget's own internals may still use custom
/// state, virtualization, worker threads, or remote systems before rejoining
/// SUI through widget contexts and scene output.
pub struct WidgetPod {
    id: WidgetId,
    layout_state: LayoutState,
    force_paint_boundary: bool,
    widget: Box<dyn Widget>,
}

impl Drop for WidgetPod {
    fn drop(&mut self) {
        crate::reactive::clear_widget(self.id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LayoutState {
    measured_size: Size,
    arranged_bounds: Rect,
    last_constraints: Constraints,
    measure_valid: bool,
    arrange_valid: bool,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            measured_size: Size::ZERO,
            arranged_bounds: Rect::ZERO,
            last_constraints: Constraints::default(),
            measure_valid: false,
            arrange_valid: false,
        }
    }
}

impl WidgetPod {
    pub fn new<W>(widget: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            id: WidgetId::new(NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed)),
            layout_state: LayoutState::default(),
            force_paint_boundary: false,
            widget: Box::new(widget),
        }
    }

    pub const fn id(&self) -> WidgetId {
        self.id
    }

    pub const fn bounds(&self) -> Rect {
        self.layout_state.arranged_bounds
    }

    pub const fn measured_size(&self) -> Size {
        self.layout_state.measured_size
    }

    pub fn set_bounds(&mut self, bounds: Rect) {
        let delta = bounds.origin - self.layout_state.arranged_bounds.origin;
        self.layout_state.arranged_bounds = bounds;
        self.layout_state.measured_size = bounds.size;
        self.layout_state.arrange_valid = true;
        self.translate_descendants(delta);
    }

    pub fn measure(&mut self, parent_ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        // Incremental-layout fast path. Cache invalidation is driven entirely by
        // the measure recursion (never a separate tree walk), so it cannot diverge
        // from how parents actually reach their children:
        //
        // - `force` is true when this pod sits inside a subtree rooted at a widget
        //   that changed this frame; such a pod always re-measures (which also
        //   preserves any side effects of measuring more than once per pass).
        // - otherwise a pod whose own id is listed dirty re-measures, while a clean
        //   pod measured before under identical constraints returns its cached size
        //   without recursing.
        let force = parent_ctx.child_force();
        let must_remeasure = force || parent_ctx.scope().must_remeasure(self.id);
        if !must_remeasure
            && self.layout_state.measure_valid
            && self.layout_state.last_constraints == constraints
        {
            return self.layout_state.measured_size;
        }

        let origin = self.layout_state.arranged_bounds.origin;
        let mut child_ctx = parent_ctx.child(self.id, self.layout_state.arranged_bounds, force);
        let started = Instant::now();
        let size = self.widget.measure(&mut child_ctx, constraints);
        record_widget_timing(
            self.id,
            self.widget.debug_name(),
            WidgetTimingPhase::Measure,
            started.elapsed(),
        );
        self.layout_state.measured_size = size;
        self.layout_state.last_constraints = constraints;
        self.layout_state.measure_valid = true;
        self.layout_state.arranged_bounds = Rect::from_origin_size(origin, size);
        parent_ctx.extend_invalidations(child_ctx.take_invalidations());
        parent_ctx.extend_wake_requests(child_ctx.take_wake_requests());
        size
    }

    pub fn arrange(&mut self, parent_ctx: &mut ArrangeCtx, bounds: Rect) {
        let delta = bounds.origin - self.layout_state.arranged_bounds.origin;
        self.layout_state.arranged_bounds = bounds;
        self.layout_state.arrange_valid = true;
        self.translate_descendants(delta);

        let mut child_ctx = ArrangeCtx::new(parent_ctx.window_id(), self.id, parent_ctx.dpi());
        let started = Instant::now();
        self.widget.arrange(&mut child_ctx, bounds);
        record_widget_timing(
            self.id,
            self.widget.debug_name(),
            WidgetTimingPhase::Arrange,
            started.elapsed(),
        );
        parent_ctx.extend_invalidations(child_ctx.take_invalidations());
    }

    pub fn paint(&self, parent_ctx: &mut PaintCtx) {
        let mut child_ctx = PaintCtx::new(
            parent_ctx.window_id(),
            self.id,
            self.layout_state.arranged_bounds,
            parent_ctx.focused_widget_id(),
            parent_ctx.dpi(),
            Arc::clone(&parent_ctx.text_system),
            Arc::clone(&parent_ctx.font_registry),
            Arc::clone(&parent_ctx.image_registry),
        );
        let started = Instant::now();
        self.widget.paint(&mut child_ctx);
        record_widget_timing(
            self.id,
            self.widget.debug_name(),
            WidgetTimingPhase::Paint,
            started.elapsed(),
        );

        let (scene, images, widget_paint_bounds, invalidations, ime_composition_rect) =
            child_ctx.into_parts();
        let paint_bounds = scene
            .paint_bounds()
            .unwrap_or(self.layout_state.arranged_bounds);
        parent_ctx.record_widget_paint_bounds(self.id, paint_bounds);
        if self.current_layer_options().emits_layer() {
            parent_ctx.push_layer(self.build_layer_descriptor(&scene), scene);
        } else {
            parent_ctx.append_scene(scene);
        }
        parent_ctx.extend_widget_paint_bounds(widget_paint_bounds);
        parent_ctx.extend_images(images);
        parent_ctx.extend_invalidations(invalidations);
        parent_ctx.extend_ime_composition_rect(ime_composition_rect);
    }

    pub(crate) fn paint_layer_contents_for(
        &mut self,
        target: WidgetId,
        parent_ctx: &mut PaintCtx,
    ) -> bool {
        self.find_mut(target, &mut |pod| {
            let started = Instant::now();
            pod.widget.paint(parent_ctx);
            record_widget_timing(
                pod.id,
                pod.widget.debug_name(),
                WidgetTimingPhase::Paint,
                started.elapsed(),
            );
            let paint_bounds = parent_ctx
                .scene()
                .paint_bounds()
                .unwrap_or(pod.layout_state.arranged_bounds);
            parent_ctx.record_widget_paint_bounds(pod.id, paint_bounds);
        })
        .is_some()
    }

    pub(crate) fn layer_descriptor_for(
        &mut self,
        target: WidgetId,
        scene: &Scene,
    ) -> Option<SceneLayerDescriptor> {
        self.find_mut(target, &mut |pod| pod.build_layer_descriptor(scene))
    }

    pub fn semantics(&self, parent_ctx: &mut SemanticsCtx) {
        let mut child_ctx = SemanticsCtx::new(
            parent_ctx.window_id(),
            self.id,
            parent_ctx.root_widget_id(),
            self.layout_state.arranged_bounds,
            parent_ctx.focused_widget_id(),
        );
        let started = Instant::now();
        self.widget.semantics(&mut child_ctx);
        record_widget_timing(
            self.id,
            self.widget.debug_name(),
            WidgetTimingPhase::Semantics,
            started.elapsed(),
        );
        parent_ctx.extend_nodes(child_ctx.into_nodes());
    }

    pub(crate) fn accepts_focus(&self) -> bool {
        self.widget.accepts_focus()
    }

    /// Visit this pod's direct children. Public so container widgets can walk
    /// their own subtree (e.g. to derive a focus-within visual by comparing
    /// descendant ids against the focused widget id — sui has no focus-within
    /// event, and any focus change repaints every window, so polling at paint
    /// time stays correct).
    pub fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.widget.visit_children(visitor);
    }

    pub(crate) fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.widget.visit_children_mut(visitor);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_event_for_path(
        &mut self,
        path: &[WidgetId],
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        phase: EventPhase,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        event: &Event,
    ) -> Option<EventDispatch> {
        self.find_mut_path(path, &mut |pod| {
            pod.dispatch_event(
                window_id,
                dpi_info,
                current_time,
                phase,
                focused_widget,
                clipboard,
                event,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_event_for(
        &mut self,
        target: WidgetId,
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        phase: EventPhase,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        event: &Event,
    ) -> Option<EventDispatch> {
        self.find_mut(target, &mut |pod| {
            pod.dispatch_event(
                window_id,
                dpi_info,
                current_time,
                phase,
                focused_widget,
                clipboard,
                event,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_command_for_path(
        &mut self,
        path: &[WidgetId],
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        command: &Command<'_>,
    ) -> Option<EventDispatch> {
        self.find_mut_path(path, &mut |pod| {
            pod.dispatch_command(
                window_id,
                dpi_info,
                current_time,
                focused_widget,
                clipboard,
                command,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_command_for(
        &mut self,
        target: WidgetId,
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        command: &Command<'_>,
    ) -> Option<EventDispatch> {
        self.find_mut(target, &mut |pod| {
            pod.dispatch_command(
                window_id,
                dpi_info,
                current_time,
                focused_widget,
                clipboard,
                command,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn notify_focus_change_for(
        &mut self,
        target: WidgetId,
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        focused: bool,
    ) -> Option<EventDispatch> {
        self.find_mut(target, &mut |pod| {
            pod.focus_changed(
                window_id,
                dpi_info,
                current_time,
                focused_widget,
                clipboard,
                focused,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn notify_focus_change_for_path(
        &mut self,
        path: &[WidgetId],
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        focused: bool,
    ) -> Option<EventDispatch> {
        self.find_mut_path(path, &mut |pod| {
            pod.focus_changed(
                window_id,
                dpi_info,
                current_time,
                focused_widget,
                clipboard,
                focused,
            )
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_event(
        &mut self,
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        phase: EventPhase,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        event: &Event,
    ) -> EventDispatch {
        let mut ctx = EventCtx::new(
            window_id,
            self.id,
            self.layout_state.arranged_bounds,
            dpi_info,
            current_time,
            phase,
            focused_widget,
            clipboard.clone(),
        );
        self.widget.event(&mut ctx, event);
        EventDispatch {
            handled: ctx.is_handled(),
            invalidations: ctx.take_invalidations(),
            focus_request: ctx.take_focus_request(),
            wake_requests: ctx.take_wake_requests(),
            pointer_capture_requests: ctx.take_pointer_capture_requests(),
            drag_requests: ctx.take_drag_requests(),
            drop_acceptances: ctx.take_drop_acceptances(),
            posted_events: ctx.take_posted_events(),
            posted_commands: ctx.take_posted_commands(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_command(
        &mut self,
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        command: &Command<'_>,
    ) -> EventDispatch {
        let mut ctx = EventCtx::new(
            window_id,
            self.id,
            self.layout_state.arranged_bounds,
            dpi_info,
            current_time,
            EventPhase::Target,
            focused_widget,
            clipboard.clone(),
        );
        self.widget.command(&mut ctx, command);
        ctx.into_dispatch()
    }

    fn focus_changed(
        &mut self,
        window_id: WindowId,
        dpi_info: DpiInfo,
        current_time: f64,
        focused_widget: Option<WidgetId>,
        clipboard: &Clipboard,
        focused: bool,
    ) -> EventDispatch {
        let mut ctx = EventCtx::new(
            window_id,
            self.id,
            self.layout_state.arranged_bounds,
            dpi_info,
            current_time,
            EventPhase::Target,
            focused_widget,
            clipboard.clone(),
        );
        self.widget.focus_changed(&mut ctx, focused);
        EventDispatch {
            handled: ctx.is_handled(),
            invalidations: ctx.take_invalidations(),
            focus_request: ctx.take_focus_request(),
            wake_requests: ctx.take_wake_requests(),
            pointer_capture_requests: ctx.take_pointer_capture_requests(),
            drag_requests: ctx.take_drag_requests(),
            drop_acceptances: ctx.take_drop_acceptances(),
            posted_events: ctx.take_posted_events(),
            posted_commands: ctx.take_posted_commands(),
        }
    }

    fn find_mut<R, F>(&mut self, target: WidgetId, f: &mut F) -> Option<R>
    where
        F: FnMut(&mut WidgetPod) -> R,
    {
        if self.id == target {
            return Some(f(self));
        }

        let mut result = None;
        let mut visitor = FindMutVisitor {
            target,
            callback: f,
            result: &mut result,
        };
        self.visit_children_mut(&mut visitor);
        result
    }

    fn find_mut_path<R, F>(&mut self, path: &[WidgetId], f: &mut F) -> Option<R>
    where
        F: FnMut(&mut WidgetPod) -> R,
    {
        let (&current, rest) = path.split_first()?;
        if current != self.id {
            return None;
        }

        if rest.is_empty() {
            return Some(f(self));
        }

        let mut result = None;
        let mut visitor = FindPathMutVisitor {
            path: rest,
            callback: f,
            result: &mut result,
        };
        self.visit_children_mut(&mut visitor);
        result
    }

    fn translate_descendants(&mut self, delta: Vector) {
        if delta == Vector::ZERO {
            return;
        }

        let mut visitor = TranslateVisitor { delta };
        self.visit_children_mut(&mut visitor);
    }

    fn translate_subtree(&mut self, delta: Vector) {
        if delta == Vector::ZERO {
            return;
        }

        self.layout_state.arranged_bounds = self.layout_state.arranged_bounds.translate(delta);
        self.translate_descendants(delta);
    }

    pub(crate) fn build_layer_descriptor(&self, scene: &Scene) -> SceneLayerDescriptor {
        let options = self.current_layer_options();
        SceneLayerDescriptor::new(
            SceneLayerId::from_widget(self.id),
            self.id,
            self.layout_state.arranged_bounds,
        )
        .with_content_bounds(
            scene
                .content_bounds()
                .unwrap_or(self.layout_state.arranged_bounds),
        )
        .with_paint_bounds(
            scene
                .paint_bounds()
                .unwrap_or(self.layout_state.arranged_bounds),
        )
        .with_properties(self.current_layer_properties())
        .with_composition_mode(options.composition_mode)
    }

    pub(crate) fn current_layer_options(&self) -> LayerOptions {
        let mut options = self.widget.layer_options();
        if self.force_paint_boundary {
            options.paint_boundary = PaintBoundaryMode::Explicit;
        }
        options
    }

    pub(crate) fn current_layer_properties(&self) -> LayerProperties {
        self.widget.layer_properties()
    }

    pub(crate) fn current_paint_boundary_mode(&self) -> PaintBoundaryMode {
        self.current_layer_options().paint_boundary
    }

    pub(crate) fn current_stack_host_options(&self) -> Option<StackHostOptions> {
        self.widget.stack_host_options()
    }

    pub(crate) fn current_stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        self.widget.stack_surface_options()
    }

    pub(crate) fn layer_composition_mode_for(
        &mut self,
        target: WidgetId,
    ) -> Option<LayerCompositionMode> {
        self.find_mut(target, &mut |pod| {
            pod.current_layer_options().composition_mode
        })
    }
}

struct TranslateVisitor {
    delta: Vector,
}

impl WidgetPodMutVisitor for TranslateVisitor {
    fn visit(&mut self, child: &mut WidgetPod) {
        child.translate_subtree(self.delta);
    }
}

struct FindMutVisitor<'a, F, R>
where
    F: FnMut(&mut WidgetPod) -> R,
{
    target: WidgetId,
    callback: &'a mut F,
    result: &'a mut Option<R>,
}

impl<F, R> WidgetPodMutVisitor for FindMutVisitor<'_, F, R>
where
    F: FnMut(&mut WidgetPod) -> R,
{
    fn visit(&mut self, child: &mut WidgetPod) {
        if self.result.is_none() {
            *self.result = child.find_mut(self.target, self.callback);
        }
    }
}

struct FindPathMutVisitor<'a, F, R>
where
    F: FnMut(&mut WidgetPod) -> R,
{
    path: &'a [WidgetId],
    callback: &'a mut F,
    result: &'a mut Option<R>,
}

impl<F, R> WidgetPodMutVisitor for FindPathMutVisitor<'_, F, R>
where
    F: FnMut(&mut WidgetPod) -> R,
{
    fn visit(&mut self, child: &mut WidgetPod) {
        if self.result.is_none() && self.path.first().copied() == Some(child.id) {
            *self.result = child.find_mut_path(self.path, self.callback);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusRequest {
    Focus(WidgetId),
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WakeRequest {
    ScheduleTimer {
        token: TimerToken,
        deadline: f64,
        target: WidgetId,
    },
    CancelTimer {
        token: TimerToken,
    },
    RegisterAsync {
        token: AsyncWakeToken,
        target: WidgetId,
    },
    UnregisterAsync {
        token: AsyncWakeToken,
    },
    RequestAnimationFrame {
        target: WidgetId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerCaptureRequest {
    Capture { pointer_id: u64, target: WidgetId },
    Release { pointer_id: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct BeginDragRequest {
    pub session_id: DragSessionId,
    pub scope_id: DragScopeId,
    pub pointer_id: u64,
    pub source: WidgetId,
    pub position: Point,
    pub payload: DragPayload,
    pub allowed_effect: DropEffect,
    pub preview_label: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum DragRequest {
    Begin(BeginDragRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DropAcceptanceRequest {
    pub target: WidgetId,
    pub effect: DropEffect,
}

/// An event queued by a widget for delivery to another widget once the
/// current dispatch finishes.
#[derive(Debug, Clone)]
pub(crate) struct PostedEventRequest {
    pub target: WidgetId,
    pub event: Event,
}

/// A typed, target-only command posted during widget dispatch.
#[derive(Debug, Clone)]
pub(crate) struct PostedCommandRequest {
    pub command: QueuedCommand,
}

#[derive(Debug, Clone)]
pub(crate) struct EventDispatch {
    pub handled: bool,
    pub invalidations: Vec<InvalidationRequest>,
    pub focus_request: Option<FocusRequest>,
    pub wake_requests: Vec<WakeRequest>,
    pub pointer_capture_requests: Vec<PointerCaptureRequest>,
    pub drag_requests: Vec<DragRequest>,
    pub drop_acceptances: Vec<DropAcceptanceRequest>,
    pub posted_events: Vec<PostedEventRequest>,
    pub posted_commands: Vec<PostedCommandRequest>,
}

#[derive(Debug, Clone)]
pub struct EventCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    bounds: Rect,
    dpi_info: DpiInfo,
    current_time: f64,
    phase: EventPhase,
    focused_widget_id: Option<WidgetId>,
    clipboard: Clipboard,
    handled: bool,
    invalidations: Vec<InvalidationRequest>,
    focus_request: Option<FocusRequest>,
    wake_requests: Vec<WakeRequest>,
    pointer_capture_requests: Vec<PointerCaptureRequest>,
    drag_requests: Vec<DragRequest>,
    drop_acceptances: Vec<DropAcceptanceRequest>,
    posted_events: Vec<PostedEventRequest>,
    posted_commands: Vec<PostedCommandRequest>,
}

impl EventCtx {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        dpi_info: DpiInfo,
        current_time: f64,
        phase: EventPhase,
        focused_widget_id: Option<WidgetId>,
        clipboard: Clipboard,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            bounds,
            dpi_info,
            current_time,
            phase,
            focused_widget_id,
            clipboard,
            handled: false,
            invalidations: Vec::new(),
            focus_request: None,
            wake_requests: Vec::new(),
            pointer_capture_requests: Vec::new(),
            drag_requests: Vec::new(),
            drop_acceptances: Vec::new(),
            posted_events: Vec::new(),
            posted_commands: Vec::new(),
        }
    }

    fn into_dispatch(mut self) -> EventDispatch {
        EventDispatch {
            handled: self.is_handled(),
            invalidations: self.take_invalidations(),
            focus_request: self.take_focus_request(),
            wake_requests: self.take_wake_requests(),
            pointer_capture_requests: self.take_pointer_capture_requests(),
            drag_requests: self.take_drag_requests(),
            drop_acceptances: self.take_drop_acceptances(),
            posted_events: self.take_posted_events(),
            posted_commands: self.take_posted_commands(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    /// Read an observable and subscribe this widget to the requested
    /// invalidation kind.
    pub fn observe<T, O>(&self, observable: &O, kind: InvalidationKind) -> T
    where
        O: Observable<T> + ?Sized,
    {
        crate::reactive::observe(self.window_id, self.widget_id, observable, kind)
    }

    /// Record a development diagnostic explaining a structural widget rebuild.
    pub fn record_rebuild(&self, widget_name: &'static str, reason: impl Into<String>) {
        crate::diagnostics::record_widget_rebuild(
            self.window_id,
            self.widget_id,
            widget_name,
            reason,
        );
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub const fn current_time(&self) -> f64 {
        self.current_time
    }

    pub const fn phase(&self) -> EventPhase {
        self.phase
    }

    pub const fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget_id
    }

    pub fn is_focused(&self) -> bool {
        self.focused_widget_id == Some(self.widget_id)
    }

    pub const fn is_handled(&self) -> bool {
        self.handled
    }

    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    /// Shared clipboard service for this window. Backed by the OS clipboard
    /// when the platform installed a native backend, otherwise by in-process
    /// storage shared across the runtime.
    pub fn clipboard(&self) -> &Clipboard {
        &self.clipboard
    }

    pub fn clipboard_text(&self) -> Option<String> {
        self.clipboard.text()
    }

    pub fn set_clipboard_text(&mut self, text: impl AsRef<str>) {
        self.clipboard.set_text(text);
    }

    /// Queue an event for delivery to `target` after the current dispatch
    /// completes. Used to route commands (for example clipboard actions from
    /// a context menu) to a specific widget outside the normal hit-test or
    /// focus routing.
    pub fn post_event(&mut self, target: WidgetId, event: Event) {
        self.posted_events
            .push(PostedEventRequest { target, event });
    }

    /// Queue a typed command for target-only delivery after the current widget
    /// dispatch completes.
    pub fn post_command<T>(&mut self, target: WidgetId, key: CommandKey<T>, payload: T)
    where
        T: Send + Sync + 'static,
    {
        self.posted_commands.push(PostedCommandRequest {
            command: queued_command(
                CommandTarget::Widget {
                    window_id: self.window_id,
                    widget_id: target,
                },
                CommandDelivery::Directed,
                key,
                payload,
            ),
        });
    }

    pub fn request_focus(&mut self) {
        self.focus_request = Some(FocusRequest::Focus(self.widget_id));
    }

    pub fn clear_focus(&mut self) {
        self.focus_request = Some(FocusRequest::Clear);
    }

    pub fn schedule_timer_at(&mut self, deadline: f64) -> TimerToken {
        let token = TimerToken::new(NEXT_TIMER_TOKEN.fetch_add(1, Ordering::Relaxed));
        self.wake_requests.push(WakeRequest::ScheduleTimer {
            token,
            deadline,
            target: self.widget_id,
        });
        token
    }

    pub fn schedule_timer_after(&mut self, delay: f64) -> TimerToken {
        self.schedule_timer_at(self.current_time + delay)
    }

    pub fn cancel_timer(&mut self, token: TimerToken) {
        self.wake_requests.push(WakeRequest::CancelTimer { token });
    }

    pub fn register_async_wakeup(&mut self) -> AsyncWakeToken {
        let token = AsyncWakeToken::new(NEXT_ASYNC_WAKE_TOKEN.fetch_add(1, Ordering::Relaxed));
        self.wake_requests.push(WakeRequest::RegisterAsync {
            token,
            target: self.widget_id,
        });
        token
    }

    pub fn unregister_async_wakeup(&mut self, token: AsyncWakeToken) {
        self.wake_requests
            .push(WakeRequest::UnregisterAsync { token });
    }

    pub fn request_animation_frame(&mut self) {
        self.wake_requests.push(WakeRequest::RequestAnimationFrame {
            target: self.widget_id,
        });
    }

    pub fn request_pointer_capture(&mut self, pointer_id: u64) {
        self.pointer_capture_requests
            .push(PointerCaptureRequest::Capture {
                pointer_id,
                target: self.widget_id,
            });
    }

    pub fn release_pointer_capture(&mut self, pointer_id: u64) {
        self.pointer_capture_requests
            .push(PointerCaptureRequest::Release { pointer_id });
    }

    pub fn begin_drag(
        &mut self,
        scope_id: DragScopeId,
        pointer_id: u64,
        position: Point,
        payload: DragPayload,
        allowed_effect: DropEffect,
        preview_label: Option<String>,
    ) -> DragSessionId {
        let session_id = DragSessionId::new(NEXT_DRAG_SESSION_ID.fetch_add(1, Ordering::Relaxed));
        self.drag_requests
            .push(DragRequest::Begin(BeginDragRequest {
                session_id,
                scope_id,
                pointer_id,
                source: self.widget_id,
                position,
                payload,
                allowed_effect,
                preview_label,
            }));
        session_id
    }

    pub fn accept_drop(&mut self, effect: DropEffect) {
        if effect.is_none() {
            return;
        }
        self.drop_acceptances.push(DropAcceptanceRequest {
            target: self.widget_id,
            effect,
        });
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_measure(&mut self) {
        self.request_widget(InvalidationKind::Measure);
    }

    pub fn request_arrange(&mut self) {
        self.request_widget(InvalidationKind::Arrange);
    }

    pub fn request_ordering(&mut self) {
        self.request_widget(InvalidationKind::Ordering);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request(
            InvalidationRequest::new(
                InvalidationTarget::Widget(self.widget_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn request_transform(&mut self) {
        self.request_widget(InvalidationKind::Transform);
    }

    pub fn request_effect(&mut self) {
        self.request_widget(InvalidationKind::Effect);
    }

    pub fn request_visibility(&mut self) {
        self.request_widget(InvalidationKind::Visibility);
    }

    pub fn request_hit_test(&mut self) {
        self.request_widget(InvalidationKind::HitTest);
    }

    pub fn request_text(&mut self) {
        self.request_widget(InvalidationKind::Text);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn request_resources(&mut self) {
        self.request_widget(InvalidationKind::Resources);
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn take_focus_request(&mut self) -> Option<FocusRequest> {
        self.focus_request.take()
    }

    pub(crate) fn take_wake_requests(&mut self) -> Vec<WakeRequest> {
        std::mem::take(&mut self.wake_requests)
    }

    pub(crate) fn take_pointer_capture_requests(&mut self) -> Vec<PointerCaptureRequest> {
        std::mem::take(&mut self.pointer_capture_requests)
    }

    pub(crate) fn take_drag_requests(&mut self) -> Vec<DragRequest> {
        std::mem::take(&mut self.drag_requests)
    }

    pub(crate) fn take_drop_acceptances(&mut self) -> Vec<DropAcceptanceRequest> {
        std::mem::take(&mut self.drop_acceptances)
    }

    pub(crate) fn take_posted_events(&mut self) -> Vec<PostedEventRequest> {
        std::mem::take(&mut self.posted_events)
    }

    pub(crate) fn take_posted_commands(&mut self) -> Vec<PostedCommandRequest> {
        std::mem::take(&mut self.posted_commands)
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

/// Per-measure-pass invalidation scope, shared (via `Rc`) by every `MeasureCtx`
/// in a single pass. Drives which pods may reuse their cached measure.
///
/// - `force_all`: ignore all caches and re-measure everything (bootstrap, resize,
///   or any frame whose dirty set could not be resolved to concrete widgets).
/// - `dirty`: pods that must recompute their own measure — the changed widgets
///   plus every ancestor on the path from the root (an ancestor's layout can
///   depend on a descendant's size).
/// - `subtree_roots`: the changed widgets themselves; entering their children
///   forces the whole subtree to re-measure, because the change is inside them.
#[derive(Debug, Default)]
pub(crate) struct MeasureScope {
    force_all: bool,
    dirty: HashSet<WidgetId>,
    subtree_roots: HashSet<WidgetId>,
}

impl MeasureScope {
    pub(crate) fn force_all() -> Self {
        Self {
            force_all: true,
            dirty: HashSet::new(),
            subtree_roots: HashSet::new(),
        }
    }

    pub(crate) fn scoped(dirty: HashSet<WidgetId>, subtree_roots: HashSet<WidgetId>) -> Self {
        Self {
            force_all: false,
            dirty,
            subtree_roots,
        }
    }

    fn must_remeasure(&self, widget_id: WidgetId) -> bool {
        self.force_all || self.dirty.contains(&widget_id)
    }

    fn forces_subtree(&self, widget_id: WidgetId) -> bool {
        self.subtree_roots.contains(&widget_id)
    }
}

#[derive(Debug, Clone)]
pub struct MeasureCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    bounds: Rect,
    layout: LayoutContext,
    current_time: f64,
    invalidations: Vec<InvalidationRequest>,
    wake_requests: Vec<WakeRequest>,
    scope: Rc<MeasureScope>,
    /// Whether the pod this ctx belongs to is inside a forced subtree.
    force: bool,
}

impl MeasureCtx {
    pub fn with_layout(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        layout: LayoutContext,
    ) -> Self {
        // External / test callers get full-measure semantics by default.
        Self::with_layout_scoped(
            window_id,
            widget_id,
            bounds,
            layout,
            0.0,
            Rc::new(MeasureScope::force_all()),
            true,
        )
    }

    pub(crate) fn with_layout_scoped(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        layout: LayoutContext,
        current_time: f64,
        scope: Rc<MeasureScope>,
        force: bool,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            bounds,
            layout,
            current_time,
            invalidations: Vec::new(),
            wake_requests: Vec::new(),
            scope,
            force,
        }
    }

    #[cfg(test)]
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        dpi_info: DpiInfo,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Self {
        Self::with_layout(
            window_id,
            widget_id,
            bounds,
            LayoutContext::new(dpi_info, text_system, font_registry, image_registry),
        )
    }

    pub(crate) fn new_scoped_at(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        dpi_info: DpiInfo,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
        scope: Rc<MeasureScope>,
        current_time: f64,
    ) -> Self {
        Self::with_layout_scoped(
            window_id,
            widget_id,
            bounds,
            LayoutContext::new(dpi_info, text_system, font_registry, image_registry),
            current_time,
            scope,
            false,
        )
    }

    pub(crate) fn scope(&self) -> &MeasureScope {
        &self.scope
    }

    /// Whether the *children* of this pod must be force-remeasured: either this
    /// pod is already inside a forced subtree, or this pod is itself a changed
    /// widget whose subtree must be rebuilt.
    pub(crate) fn child_force(&self) -> bool {
        self.force || self.scope.forces_subtree(self.widget_id)
    }

    /// Build the measure ctx for a child pod, carrying the shared scope and the
    /// child's resolved force flag.
    pub(crate) fn child(&self, widget_id: WidgetId, bounds: Rect, force: bool) -> Self {
        Self::with_layout_scoped(
            self.window_id,
            widget_id,
            bounds,
            self.layout.clone(),
            self.current_time,
            Rc::clone(&self.scope),
            force,
        )
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    /// Read an observable whose changes affect measurement.
    pub fn observe<T, O>(&self, observable: &O) -> T
    where
        O: Observable<T> + ?Sized,
    {
        self.observe_with(observable, InvalidationKind::Measure)
    }

    /// Read an observable and explicitly declare the resulting invalidation.
    pub fn observe_with<T, O>(&self, observable: &O, kind: InvalidationKind) -> T
    where
        O: Observable<T> + ?Sized,
    {
        crate::reactive::observe(self.window_id, self.widget_id, observable, kind)
    }

    /// Record a development diagnostic explaining a structural widget rebuild.
    pub fn record_rebuild(&self, widget_name: &'static str, reason: impl Into<String>) {
        crate::diagnostics::record_widget_rebuild(
            self.window_id,
            self.widget_id,
            widget_name,
            reason,
        );
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub const fn layout(&self) -> &LayoutContext {
        &self.layout
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.layout.dpi()
    }

    pub const fn current_time(&self) -> f64 {
        self.current_time
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_measure(&mut self) {
        self.request_widget(InvalidationKind::Measure);
    }

    pub fn request_arrange(&mut self) {
        self.request_widget(InvalidationKind::Arrange);
    }

    pub fn request_ordering(&mut self) {
        self.request_widget(InvalidationKind::Ordering);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn request_animation_frame(&mut self) {
        self.wake_requests.push(WakeRequest::RequestAnimationFrame {
            target: self.widget_id,
        });
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn take_wake_requests(&mut self) -> Vec<WakeRequest> {
        std::mem::take(&mut self.wake_requests)
    }

    pub(crate) fn extend_invalidations(&mut self, invalidations: Vec<InvalidationRequest>) {
        self.invalidations.extend(invalidations);
    }

    pub(crate) fn extend_wake_requests(&mut self, wake_requests: Vec<WakeRequest>) {
        self.wake_requests.extend(wake_requests);
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct ArrangeCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    dpi_info: DpiInfo,
    invalidations: Vec<InvalidationRequest>,
}

impl ArrangeCtx {
    pub(crate) fn new(window_id: WindowId, widget_id: WidgetId, dpi_info: DpiInfo) -> Self {
        Self {
            window_id,
            widget_id,
            dpi_info,
            invalidations: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    /// Read an observable whose changes affect arrangement.
    pub fn observe<T, O>(&self, observable: &O) -> T
    where
        O: Observable<T> + ?Sized,
    {
        self.observe_with(observable, InvalidationKind::Arrange)
    }

    /// Read an observable and explicitly declare the resulting invalidation.
    pub fn observe_with<T, O>(&self, observable: &O, kind: InvalidationKind) -> T
    where
        O: Observable<T> + ?Sized,
    {
        crate::reactive::observe(self.window_id, self.widget_id, observable, kind)
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_arrange(&mut self) {
        self.request_widget(InvalidationKind::Arrange);
    }

    pub fn request_ordering(&mut self) {
        self.request_widget(InvalidationKind::Ordering);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_semantics(&mut self) {
        self.request_widget(InvalidationKind::Semantics);
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub(crate) fn take_invalidations(&mut self) -> Vec<InvalidationRequest> {
        std::mem::take(&mut self.invalidations)
    }

    pub(crate) fn extend_invalidations(&mut self, invalidations: Vec<InvalidationRequest>) {
        self.invalidations.extend(invalidations);
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

#[derive(Debug, Clone)]
pub struct PaintCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    focused_widget_id: Option<WidgetId>,
    bounds: Rect,
    dpi_info: DpiInfo,
    text_system: Arc<TextSystem>,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
    scene: Scene,
    images: Vec<(ImageHandle, PaintImageResource)>,
    widget_paint_bounds: HashMap<WidgetId, Rect>,
    invalidations: Vec<InvalidationRequest>,
    ime_composition_rect: Option<Rect>,
}

#[derive(Debug, Clone)]
pub(crate) enum PaintImageResource {
    Cpu(RegisteredImage),
    External(RegisteredExternalImage),
}

impl PaintCtx {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        bounds: Rect,
        focused_widget_id: Option<WidgetId>,
        dpi_info: DpiInfo,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            focused_widget_id,
            bounds,
            dpi_info,
            text_system,
            font_registry,
            image_registry,
            scene: Scene::new(),
            images: Vec::new(),
            widget_paint_bounds: HashMap::new(),
            invalidations: Vec::new(),
            ime_composition_rect: None,
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    /// Read an observable whose changes affect painting.
    pub fn observe<T, O>(&self, observable: &O) -> T
    where
        O: Observable<T> + ?Sized,
    {
        self.observe_with(observable, InvalidationKind::Paint)
    }

    /// Read an observable and explicitly declare the resulting invalidation.
    pub fn observe_with<T, O>(&self, observable: &O, kind: InvalidationKind) -> T
    where
        O: Observable<T> + ?Sized,
    {
        crate::reactive::observe(self.window_id, self.widget_id, observable, kind)
    }

    pub const fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget_id
    }

    pub fn is_focused(&self) -> bool {
        self.focused_widget_id == Some(self.widget_id)
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn measure_text(
        &self,
        text: impl Into<String>,
        style: TextStyle,
    ) -> sui_core::Result<TextMeasurement> {
        self.text_system
            .measure_text(text, style, self.font_registry.as_ref())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
    ) -> sui_core::Result<TextLayout> {
        self.text_system
            .shape_text(text, box_size, style, self.font_registry.as_ref())
    }

    pub fn layout_text_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout> {
        self.text_system
            .layout_document(request, self.font_registry.as_ref())
    }

    pub fn clear(&mut self, color: Color) {
        self.scene.push(SceneCommand::Clear(color));
    }

    pub fn fill(&mut self, path: impl Into<Path>, brush: impl Into<Brush>) {
        self.scene.push(SceneCommand::FillPath {
            path: path.into(),
            brush: brush.into(),
        });
    }

    pub fn fill_rect(&mut self, rect: Rect, brush: impl Into<Brush>) {
        self.scene.push(SceneCommand::FillRect {
            rect,
            brush: brush.into(),
        });
    }

    pub fn fill_bounds(&mut self, brush: impl Into<Brush>) {
        self.fill_rect(self.bounds, brush);
    }

    pub fn stroke(&mut self, path: impl Into<Path>, brush: impl Into<Brush>, stroke: StrokeStyle) {
        self.scene.push(SceneCommand::StrokePath {
            path: path.into(),
            brush: brush.into(),
            stroke,
        });
    }

    pub fn stroke_rect(&mut self, rect: Rect, brush: impl Into<Brush>, stroke: StrokeStyle) {
        self.scene.push(SceneCommand::StrokeRect {
            rect,
            brush: brush.into(),
            stroke,
        });
    }

    pub fn stroke_bounds(&mut self, brush: impl Into<Brush>, stroke: StrokeStyle) {
        self.stroke_rect(self.bounds, brush, stroke);
    }

    pub fn draw_text(&mut self, rect: Rect, text: impl Into<String>, style: TextStyle) {
        let text = text.into();
        let paint_color = style.color;
        let mut layout_style = style.clone();
        layout_style.color = Color::WHITE;
        let layout_run = TextRun {
            rect,
            text: text.clone(),
            style: layout_style,
        };
        let fallback_run = TextRun { rect, text, style };

        match self.text_system.shape_text_run_persistent(
            Some(stable_text_run_handle(&layout_run)),
            &layout_run,
            self.font_registry.as_ref(),
        ) {
            Ok(layout) => self.scene.push(SceneCommand::DrawShapedText(
                ShapedText::new(rect.origin, &layout).with_color(paint_color),
            )),
            Err(_) => self.scene.push(SceneCommand::DrawText(fallback_run)),
        }
    }

    pub fn draw_text_layout(&mut self, origin: Point, layout: &TextLayout) {
        let persistent = self.text_system.adopt_layout(layout.clone());
        self.draw_persistent_text_layout(origin, &persistent);
    }

    pub fn draw_text_layout_with_color(
        &mut self,
        origin: Point,
        layout: &TextLayout,
        color: Color,
    ) {
        let persistent = self.text_system.adopt_layout(layout.clone());
        self.draw_persistent_text_layout_with_color(origin, &persistent, color);
    }

    pub fn draw_persistent_text_layout(&mut self, origin: Point, layout: &PersistentTextLayout) {
        self.text_system.touch_persistent_layout(layout);
        self.scene
            .push(SceneCommand::DrawShapedText(ShapedText::new(
                origin, layout,
            )));
    }

    pub fn draw_persistent_text_layout_with_color(
        &mut self,
        origin: Point,
        layout: &PersistentTextLayout,
        color: Color,
    ) {
        self.text_system.touch_persistent_layout(layout);
        self.scene.push(SceneCommand::DrawShapedText(
            ShapedText::new(origin, layout).with_color(color),
        ));
    }

    pub fn draw_persistent_text_layout_window(
        &mut self,
        origin: Point,
        layout: &PersistentTextLayout,
        line_range: std::ops::Range<usize>,
    ) {
        self.text_system.touch_persistent_layout(layout);
        self.scene
            .push(SceneCommand::DrawShapedTextWindow(ShapedTextWindow::new(
                origin, layout, line_range,
            )));
    }

    pub fn draw_persistent_text_layout_window_with_color(
        &mut self,
        origin: Point,
        layout: &PersistentTextLayout,
        line_range: std::ops::Range<usize>,
        color: Color,
    ) {
        self.text_system.touch_persistent_layout(layout);
        self.scene.push(SceneCommand::DrawShapedTextWindow(
            ShapedTextWindow::new(origin, layout, line_range).with_color(color),
        ));
    }

    pub fn label(&mut self, rect: Rect, text: impl Into<String>, color: Color) {
        self.draw_text(rect, text, TextStyle::new(color));
    }

    pub fn push_text_render_policy(&mut self, policy: TextRenderPolicy) {
        self.scene.push(SceneCommand::PushTextRenderPolicy {
            policy: policy.normalized(),
        });
    }

    pub fn pop_text_render_policy(&mut self) {
        self.scene.push(SceneCommand::PopTextRenderPolicy);
    }

    pub fn draw_image(&mut self, rect: Rect, image: sui_core::ImageHandle) {
        self.scene.push(SceneCommand::DrawImage {
            rect,
            source: ImageSource::new(image),
        });
    }

    pub fn image_registered(&self, image: ImageHandle) -> bool {
        self.image_registry.contains(image)
            || self.images.iter().any(|(handle, _)| *handle == image)
    }

    pub fn draw_image_source(&mut self, rect: Rect, source: ImageSource) {
        self.scene.push(SceneCommand::DrawImage { rect, source });
    }

    pub fn draw_image_quad(&mut self, points: [Point; 4], image: ImageHandle) {
        self.draw_image_quad_source(points, ImageSource::new(image));
    }

    pub fn draw_image_quad_source(&mut self, points: [Point; 4], source: ImageSource) {
        self.scene
            .push(SceneCommand::DrawImageQuad { points, source });
    }

    pub fn widget_image_handle(&self, slot: u64) -> ImageHandle {
        let mut hasher = DefaultHasher::new();
        self.widget_id.hash(&mut hasher);
        slot.hash(&mut hasher);
        ImageHandle::new(WIDGET_IMAGE_HANDLE_NAMESPACE | (hasher.finish() >> 1))
    }

    pub fn register_image(&mut self, handle: ImageHandle, image: RegisteredImage) {
        self.images.push((handle, PaintImageResource::Cpu(image)));
    }

    /// Register renderer-neutral metadata for an externally owned image.
    ///
    /// The active renderer must have a matching backend resource registered
    /// for `handle`. The external resource remains app-owned; this only makes
    /// its dimensions available to layout and scene construction for the
    /// current frame.
    pub fn register_external_image(&mut self, handle: ImageHandle, image: RegisteredExternalImage) {
        self.images
            .push((handle, PaintImageResource::External(image)));
    }

    pub fn draw_shader_rect(&mut self, rect: Rect, shader: WidgetShader) {
        self.scene
            .push(SceneCommand::DrawShaderRect { rect, shader });
    }

    /// Fill a rounded rectangle (per-corner radii `[tl, tr, br, bl]`) with a brush.
    pub fn fill_rrect(&mut self, rect: Rect, radii: [f32; 4], brush: impl Into<Brush>) {
        self.scene.push(SceneCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: None,
            shadow: None,
        });
    }

    /// Fill a rounded rectangle with a brush and stroke an inset border.
    pub fn fill_rrect_bordered(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        brush: impl Into<Brush>,
        border: Border,
    ) {
        self.scene.push(SceneCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: Some(border),
            shadow: None,
        });
    }

    /// Paint just a soft drop shadow for a rounded rectangle (no fill).
    pub fn draw_shadow(&mut self, rect: Rect, radii: [f32; 4], shadow: ShadowParams) {
        self.scene.push(SceneCommand::FillRoundedRect {
            rect,
            radii,
            brush: Brush::Solid(Color::TRANSPARENT),
            border: None,
            shadow: Some(shadow),
        });
    }

    /// Fill a rounded rectangle with a brush behind a soft drop shadow.
    pub fn fill_rrect_with_shadow(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        brush: impl Into<Brush>,
        shadow: ShadowParams,
    ) {
        self.scene.push(SceneCommand::FillRoundedRect {
            rect,
            radii,
            brush: brush.into(),
            border: None,
            shadow: Some(shadow),
        });
    }

    pub fn push_clip(&mut self, path: impl Into<Path>) {
        self.scene
            .push(SceneCommand::PushClipPath { path: path.into() });
    }

    pub fn push_clip_rect(&mut self, rect: Rect) {
        self.scene.push(SceneCommand::PushClip { rect });
    }

    pub fn pop_clip(&mut self) {
        self.scene.push(SceneCommand::PopClip);
    }

    pub fn push_transform(&mut self, transform: Transform) {
        self.scene.push(SceneCommand::PushTransform { transform });
    }

    pub fn translate(&mut self, delta: Vector) {
        self.push_transform(Transform::translation_vector(delta));
    }

    pub fn pop_transform(&mut self) {
        self.scene.push(SceneCommand::PopTransform);
    }

    pub fn push(&mut self, command: SceneCommand) {
        self.scene.push(command);
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn request(&mut self, request: InvalidationRequest) {
        self.invalidations.push(request);
    }

    pub fn request_paint(&mut self) {
        self.request_widget(InvalidationKind::Paint);
    }

    pub fn request_paint_rect(&mut self, rect: Rect) {
        self.request(
            InvalidationRequest::new(
                InvalidationTarget::Widget(self.widget_id),
                InvalidationKind::Paint,
            )
            .with_region(rect),
        );
    }

    pub fn invalidations(&self) -> &[InvalidationRequest] {
        &self.invalidations
    }

    pub fn set_ime_composition_rect(&mut self, rect: Rect) {
        self.ime_composition_rect = Some(rect);
    }

    pub fn clear_ime_composition_rect(&mut self) {
        self.ime_composition_rect = None;
    }

    pub const fn ime_composition_rect(&self) -> Option<Rect> {
        self.ime_composition_rect
    }

    pub(crate) fn push_layer(&mut self, descriptor: SceneLayerDescriptor, scene: Scene) {
        self.scene
            .push(SceneCommand::Layer(SceneLayer::from_descriptor(
                descriptor, scene,
            )));
    }

    pub(crate) fn append_scene(&mut self, scene: Scene) {
        self.scene.append(scene);
    }

    pub(crate) fn record_widget_paint_bounds(&mut self, widget_id: WidgetId, paint_bounds: Rect) {
        self.widget_paint_bounds.insert(widget_id, paint_bounds);
    }

    pub(crate) fn extend_widget_paint_bounds(
        &mut self,
        widget_paint_bounds: HashMap<WidgetId, Rect>,
    ) {
        self.widget_paint_bounds.extend(widget_paint_bounds);
    }

    pub(crate) fn extend_images(&mut self, images: Vec<(ImageHandle, PaintImageResource)>) {
        self.images.extend(images);
    }

    pub(crate) fn extend_invalidations(&mut self, invalidations: Vec<InvalidationRequest>) {
        self.invalidations.extend(invalidations);
    }

    pub(crate) fn extend_ime_composition_rect(&mut self, ime_composition_rect: Option<Rect>) {
        if ime_composition_rect.is_some() {
            self.ime_composition_rect = ime_composition_rect;
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Scene,
        Vec<(ImageHandle, PaintImageResource)>,
        HashMap<WidgetId, Rect>,
        Vec<InvalidationRequest>,
        Option<Rect>,
    ) {
        (
            self.scene,
            self.images,
            self.widget_paint_bounds,
            self.invalidations,
            self.ime_composition_rect,
        )
    }

    fn request_widget(&mut self, kind: InvalidationKind) {
        self.request(InvalidationRequest::new(
            InvalidationTarget::Widget(self.widget_id),
            kind,
        ));
    }
}

fn stable_text_run_handle(run: &TextRun) -> TextLayoutHandle {
    let mut hasher = DefaultHasher::new();
    run.text.hash(&mut hasher);
    run.rect.size.width.to_bits().hash(&mut hasher);
    run.rect.size.height.to_bits().hash(&mut hasher);
    run.style.font.map(|font| font.get()).hash(&mut hasher);
    run.style.font_size.to_bits().hash(&mut hasher);
    run.style.line_height.to_bits().hash(&mut hasher);
    run.style.weight.value().hash(&mut hasher);
    run.style.style.hash(&mut hasher);
    run.style.stretch.hash(&mut hasher);
    run.style.features.hash(&mut hasher);
    TextLayoutHandle::new(hasher.finish().max(1))
}

#[derive(Debug, Clone)]
pub struct SemanticsCtx {
    window_id: WindowId,
    widget_id: WidgetId,
    root_widget_id: WidgetId,
    focused_widget_id: Option<WidgetId>,
    bounds: Rect,
    nodes: Vec<SemanticsNode>,
}

impl SemanticsCtx {
    pub(crate) fn new(
        window_id: WindowId,
        widget_id: WidgetId,
        root_widget_id: WidgetId,
        bounds: Rect,
        focused_widget_id: Option<WidgetId>,
    ) -> Self {
        Self {
            window_id,
            widget_id,
            root_widget_id,
            focused_widget_id,
            bounds,
            nodes: Vec::new(),
        }
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn widget_id(&self) -> WidgetId {
        self.widget_id
    }

    /// Read an observable whose changes affect semantics.
    pub fn observe<T, O>(&self, observable: &O) -> T
    where
        O: Observable<T> + ?Sized,
    {
        self.observe_with(observable, InvalidationKind::Semantics)
    }

    /// Read an observable and explicitly declare the resulting invalidation.
    pub fn observe_with<T, O>(&self, observable: &O, kind: InvalidationKind) -> T
    where
        O: Observable<T> + ?Sized,
    {
        crate::reactive::observe(self.window_id, self.widget_id, observable, kind)
    }

    pub const fn root_widget_id(&self) -> WidgetId {
        self.root_widget_id
    }

    pub const fn focused_widget_id(&self) -> Option<WidgetId> {
        self.focused_widget_id
    }

    pub fn is_focused(&self) -> bool {
        self.focused_widget_id == Some(self.widget_id)
    }

    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn push(&mut self, node: SemanticsNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[SemanticsNode] {
        &self.nodes
    }

    pub(crate) fn extend_nodes(&mut self, nodes: Vec<SemanticsNode>) {
        self.nodes.extend(nodes);
    }

    pub(crate) fn into_nodes(self) -> Vec<SemanticsNode> {
        self.nodes
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        ArrangeCtx, EventCtx, EventPhase, KeyedChildren, MeasureCtx, MeasureScope, PaintCtx,
        SemanticsCtx, SingleChild, WakeRequest, Widget, WidgetChildren, WidgetPod,
        WidgetPodMutVisitor, WidgetPodVisitor,
    };
    use std::cell::Cell;
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;
    use sui_core::{
        Clipboard, Color, DpiInfo, ImageHandle, InvalidationKind, Point, Rect, SemanticsNode,
        SemanticsRole, Size, Vector, WidgetId, WindowId,
    };
    use sui_layout::{Constraints, LayoutContext};
    use sui_scene::{ImageRegistry, RegisteredImage, SceneCommand, StrokeStyle};
    use sui_text::{FontRegistry, FontWeight, TextRun, TextStyle, TextSystem};

    fn measure_ctx(window_id: WindowId, widget_id: WidgetId) -> MeasureCtx {
        MeasureCtx::new(
            window_id,
            widget_id,
            Rect::ZERO,
            DpiInfo::default(),
            std::sync::Arc::new(TextSystem::new()),
            std::sync::Arc::new(FontRegistry::new()),
            std::sync::Arc::new(ImageRegistry::new()),
        )
    }

    #[test]
    fn measure_ctx_wraps_reusable_layout_context() {
        let mut images = ImageRegistry::new();
        images.insert(
            ImageHandle::new(9),
            RegisteredImage::from_rgba8(3, 2, vec![255; 3 * 2 * 4]).unwrap(),
        );
        let layout = LayoutContext::new(
            DpiInfo::new(
                1.5,
                Some(144.0),
                Size::new(200.0, 100.0),
                Size::new(300.0, 150.0),
            ),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(images),
        );

        let measure = MeasureCtx::with_layout(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::new(4.0, 6.0, 32.0, 18.0),
            layout.clone(),
        );

        assert_eq!(measure.dpi(), layout.dpi());
        assert_eq!(measure.bounds(), Rect::new(4.0, 6.0, 32.0, 18.0));
        assert_eq!(
            measure.layout().image_size(ImageHandle::new(9)),
            Some(Size::new(3.0, 2.0))
        );
    }

    #[test]
    fn measure_and_paint_ctx_expose_dpi_info() {
        let dpi = DpiInfo::new(
            2.0,
            Some(192.0),
            sui_core::Size::new(320.0, 180.0),
            sui_core::Size::new(640.0, 360.0),
        );

        let measure = MeasureCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::ZERO,
            dpi,
            std::sync::Arc::new(TextSystem::new()),
            std::sync::Arc::new(FontRegistry::new()),
            std::sync::Arc::new(ImageRegistry::new()),
        );
        let paint = PaintCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            dpi,
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        let event = EventCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            dpi,
            0.0,
            EventPhase::Target,
            None,
            Clipboard::new(),
        );

        assert_eq!(measure.dpi(), dpi);
        assert_eq!(paint.dpi(), dpi);
        assert_eq!(event.dpi(), dpi);
    }

    struct LabelWidget;

    impl Widget for LabelWidget {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> sui_core::Size {
            constraints.clamp(sui_core::Size::new(48.0, 20.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::rgba(0.2, 0.3, 0.4, 1.0));
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            ctx.push(SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::Text,
                ctx.bounds(),
            ));
        }
    }

    struct BoundsLeaf;

    impl Widget for BoundsLeaf {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.max
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_rect(
                Rect::new(10.0, 10.0, 80.0, 50.0),
                Color::rgba(0.2, 0.3, 0.4, 1.0),
            );
        }
    }

    struct BoundsWrapper {
        child: WidgetPod,
        clip: Option<Rect>,
        translation: Vector,
    }

    impl BoundsWrapper {
        fn new(child: impl Widget + 'static, clip: Option<Rect>, translation: Vector) -> Self {
            Self {
                child: WidgetPod::new(child),
                clip,
                translation,
            }
        }
    }

    impl Widget for BoundsWrapper {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            self.child.measure(ctx, constraints)
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
            self.child.arrange(ctx, bounds);
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            if let Some(clip) = self.clip {
                ctx.push_clip_rect(clip);
            }
            if self.translation != Vector::ZERO {
                ctx.translate(self.translation);
            }

            self.child.paint(ctx);

            if self.translation != Vector::ZERO {
                ctx.pop_transform();
            }
            if self.clip.is_some() {
                ctx.pop_clip();
            }
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            visitor.visit(&self.child);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
            visitor.visit(&mut self.child);
        }
    }

    #[test]
    fn event_ctx_tracks_widget_scoped_invalidations_and_focus() {
        let mut ctx = EventCtx::new(
            WindowId::new(1),
            WidgetId::new(2),
            Rect::new(8.0, 12.0, 24.0, 36.0),
            DpiInfo::default(),
            0.0,
            EventPhase::Target,
            None,
            Clipboard::new(),
        );

        ctx.request_measure();
        ctx.request_paint_rect(Rect::new(8.0, 12.0, 24.0, 36.0));
        ctx.request_transform();
        ctx.request_effect();
        ctx.request_visibility();
        ctx.request_animation_frame();
        ctx.request_focus();
        ctx.set_handled();

        assert!(ctx.is_handled());
        assert_eq!(ctx.bounds(), Rect::new(8.0, 12.0, 24.0, 36.0));
        assert_eq!(ctx.invalidations().len(), 5);
        assert_eq!(ctx.invalidations()[0].kind, InvalidationKind::Measure);
        assert_eq!(
            ctx.invalidations()[1].region,
            Some(Rect::new(8.0, 12.0, 24.0, 36.0))
        );
        assert_eq!(ctx.invalidations()[2].kind, InvalidationKind::Transform);
        assert_eq!(ctx.invalidations()[3].kind, InvalidationKind::Effect);
        assert_eq!(ctx.invalidations()[4].kind, InvalidationKind::Visibility);
        assert_eq!(
            ctx.take_wake_requests(),
            vec![WakeRequest::RequestAnimationFrame {
                target: WidgetId::new(2)
            }]
        );
    }

    #[test]
    fn request_animation_frame_enqueues_widget_for_next_frame() {
        let mut ctx = EventCtx::new(
            WindowId::new(4),
            WidgetId::new(9),
            Rect::new(0.0, 0.0, 20.0, 10.0),
            DpiInfo::default(),
            16.0,
            EventPhase::Target,
            None,
            Clipboard::new(),
        );

        ctx.request_animation_frame();

        assert_eq!(
            ctx.take_wake_requests(),
            vec![WakeRequest::RequestAnimationFrame {
                target: WidgetId::new(9)
            }]
        );
    }

    struct CountingChild {
        measures: Rc<Cell<u32>>,
    }

    impl Widget for CountingChild {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> sui_core::Size {
            self.measures.set(self.measures.get() + 1);
            constraints.clamp(sui_core::Size::new(10.0, 10.0))
        }
    }

    struct PassthroughParent {
        child: WidgetPod,
    }

    impl Widget for PassthroughParent {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> sui_core::Size {
            self.child.measure(ctx, constraints)
        }

        fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
            visitor.visit(&self.child);
        }

        fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
            visitor.visit(&mut self.child);
        }
    }

    fn scoped_measure_ctx(root_id: WidgetId, scope: MeasureScope) -> MeasureCtx {
        MeasureCtx::new_scoped_at(
            WindowId::new(1),
            root_id,
            Rect::ZERO,
            DpiInfo::default(),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
            Rc::new(scope),
            0.0,
        )
    }

    #[test]
    fn measure_cache_skips_clean_subtrees_and_forces_changed_ones() {
        let measures = Rc::new(Cell::new(0u32));
        let child = WidgetPod::new(CountingChild {
            measures: Rc::clone(&measures),
        });
        let mut parent = WidgetPod::new(PassthroughParent { child });
        let parent_id = parent.id();
        let outside = WidgetId::new(u64::MAX);
        let constraints = Constraints::tight(sui_core::Size::new(10.0, 10.0));

        // 1. First pass (force_all) measures everything, populating caches.
        let mut ctx = scoped_measure_ctx(outside, MeasureScope::force_all());
        parent.measure(&mut ctx, constraints);
        assert_eq!(measures.get(), 1, "initial pass must measure the child");

        // 2. A pass where nothing is dirty short-circuits the clean parent before
        //    it ever recurses, so the child is not re-measured.
        let mut ctx = scoped_measure_ctx(
            outside,
            MeasureScope::scoped(HashSet::new(), HashSet::new()),
        );
        parent.measure(&mut ctx, constraints);
        assert_eq!(measures.get(), 1, "clean subtree must be skipped");

        // 3. Re-measuring only the parent (ancestor) without marking it a subtree
        //    root recomputes the parent but lets the clean child short-circuit.
        let mut dirty = HashSet::new();
        dirty.insert(parent_id);
        let mut ctx = scoped_measure_ctx(outside, MeasureScope::scoped(dirty, HashSet::new()));
        parent.measure(&mut ctx, constraints);
        assert_eq!(
            measures.get(),
            1,
            "ancestor-only re-measure must not force the subtree"
        );

        // 4. Marking the parent a subtree root forces its whole subtree to
        //    re-measure even though the child was never individually invalidated.
        //    This is the case that the popover/stack-surface regression needed.
        let mut dirty = HashSet::new();
        dirty.insert(parent_id);
        let mut roots = HashSet::new();
        roots.insert(parent_id);
        let mut ctx = scoped_measure_ctx(outside, MeasureScope::scoped(dirty, roots));
        parent.measure(&mut ctx, constraints);
        assert_eq!(
            measures.get(),
            2,
            "a changed widget must force its descendants to re-measure"
        );
    }

    #[test]
    fn widget_pod_merges_child_measure_arrange_paint_and_semantics() {
        let mut pod = WidgetPod::new(LabelWidget);
        pod.set_bounds(Rect::new(4.0, 6.0, 0.0, 0.0));

        let mut measure = measure_ctx(WindowId::new(3), WidgetId::new(4));
        let size = pod.measure(
            &mut measure,
            Constraints::tight(sui_core::Size::new(64.0, 32.0)),
        );
        let mut arrange = ArrangeCtx::new(WindowId::new(3), WidgetId::new(4), DpiInfo::default());
        pod.arrange(&mut arrange, Rect::new(4.0, 6.0, 64.0, 32.0));

        let mut paint = PaintCtx::new(
            WindowId::new(3),
            WidgetId::new(4),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        pod.paint(&mut paint);

        let mut semantics = SemanticsCtx::new(
            WindowId::new(3),
            WidgetId::new(4),
            WidgetId::new(4),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
        );
        pod.semantics(&mut semantics);

        assert_eq!(size, sui_core::Size::new(64.0, 32.0));
        assert_eq!(pod.bounds(), Rect::new(4.0, 6.0, 64.0, 32.0));
        assert_eq!(paint.scene().commands().len(), 1);
        assert!(matches!(
            paint.scene().commands()[0],
            SceneCommand::FillRect { .. }
        ));
        assert_eq!(semantics.nodes().len(), 1);
    }

    #[test]
    fn single_child_wraps_measure_arrange_and_visitation() {
        struct CaptureVisitor {
            ids: Vec<WidgetId>,
        }

        impl WidgetPodVisitor for CaptureVisitor {
            fn visit(&mut self, child: &WidgetPod) {
                self.ids.push(child.id());
            }
        }

        impl WidgetPodMutVisitor for CaptureVisitor {
            fn visit(&mut self, child: &mut WidgetPod) {
                self.ids.push(child.id());
            }
        }

        let mut child = SingleChild::new(LabelWidget);
        let mut measure = measure_ctx(WindowId::new(7), WidgetId::new(8));
        let size = child.measure(
            &mut measure,
            Constraints::tight(sui_core::Size::new(80.0, 24.0)),
        );
        let mut arrange = ArrangeCtx::new(WindowId::new(7), WidgetId::new(8), DpiInfo::default());
        child.arrange(&mut arrange, Rect::new(12.0, 18.0, 80.0, 24.0));

        let mut visitor = CaptureVisitor { ids: Vec::new() };
        child.visit_children(&mut visitor);
        child.visit_children_mut(&mut visitor);

        assert_eq!(size, sui_core::Size::new(80.0, 24.0));
        assert_eq!(child.child().bounds(), Rect::new(12.0, 18.0, 80.0, 24.0));
        assert_eq!(visitor.ids, vec![child.child().id(), child.child().id()]);
    }

    #[test]
    fn widget_children_bulk_paint_and_semantics_delegate_to_all_children() {
        let mut children = WidgetChildren::with_capacity(2);
        children.push(LabelWidget);
        children.push(LabelWidget);

        let mut measure = measure_ctx(WindowId::new(9), WidgetId::new(10));
        children.measure_child(
            0,
            &mut measure,
            Constraints::tight(sui_core::Size::new(40.0, 18.0)),
        );
        children.measure_child(
            1,
            &mut measure,
            Constraints::tight(sui_core::Size::new(60.0, 18.0)),
        );
        let mut arrange = ArrangeCtx::new(WindowId::new(9), WidgetId::new(10), DpiInfo::default());
        children.arrange_child(0, &mut arrange, Rect::new(0.0, 0.0, 40.0, 18.0));
        children.arrange_child(1, &mut arrange, Rect::new(44.0, 0.0, 60.0, 18.0));

        let mut paint = PaintCtx::new(
            WindowId::new(9),
            WidgetId::new(10),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            DpiInfo::default(),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        children.paint(&mut paint);

        let mut semantics = SemanticsCtx::new(
            WindowId::new(9),
            WidgetId::new(10),
            WidgetId::new(10),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
        );
        children.semantics(&mut semantics);

        assert_eq!(children.len(), 2);
        assert_eq!(paint.scene().commands().len(), 2);
        assert_eq!(semantics.nodes().len(), 2);
    }

    #[test]
    fn keyed_children_preserve_pods_and_update_item_signals() {
        let mut children = KeyedChildren::<u64, (u64, String)>::new();
        let first = children.reconcile(
            [(1, "one".to_string()), (2, "two".to_string())],
            |item| item.0,
            |_key, _value| LabelWidget,
        );
        assert_eq!(first.inserted, 2);
        let first_ids = children
            .iter()
            .map(|(key, _, child)| (*key, child.id()))
            .collect::<HashMap<_, _>>();

        let second = children.reconcile(
            [
                (2, "two updated".to_string()),
                (1, "one".to_string()),
                (3, "three".to_string()),
            ],
            |item| item.0,
            |_key, _value| LabelWidget,
        );
        assert_eq!(second.inserted, 1);
        assert_eq!(second.removed, 0);
        assert_eq!(second.moved, 2);
        assert_eq!(second.updated, 1);
        assert_eq!(second.unchanged, 1);

        let entries = children
            .iter()
            .map(|(key, value, child)| (*key, value.get(), child.id()))
            .collect::<Vec<_>>();
        assert_eq!(
            entries.iter().map(|(key, _, _)| *key).collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
        assert_eq!(entries[0].1.1, "two updated");
        assert_eq!(entries[0].2, first_ids[&2]);
        assert_eq!(entries[1].2, first_ids[&1]);
    }

    #[test]
    fn nested_paint_wrappers_preserve_content_and_clipped_paint_bounds() {
        let inner = BoundsWrapper::new(
            BoundsLeaf,
            Some(Rect::new(0.0, 0.0, 60.0, 60.0)),
            Vector::new(0.0, 7.0),
        );
        let outer = BoundsWrapper::new(
            inner,
            Some(Rect::new(0.0, 0.0, 100.0, 100.0)),
            Vector::new(5.0, 0.0),
        );
        // This pass-through level exercises the balanced Scene::append summary path
        // after the clipped/transformed child stream has returned to identity state.
        let mut root = WidgetPod::new(BoundsWrapper::new(outer, None, Vector::ZERO));
        let window_id = WindowId::new(17);
        let root_id = WidgetId::new(18);
        let bounds = Rect::new(0.0, 0.0, 120.0, 100.0);
        let mut measure = measure_ctx(window_id, root_id);
        root.measure(&mut measure, Constraints::tight(bounds.size));
        let mut arrange = ArrangeCtx::new(window_id, root_id, DpiInfo::default());
        root.arrange(&mut arrange, bounds);

        let mut paint = PaintCtx::new(
            window_id,
            root_id,
            bounds,
            None,
            DpiInfo::default(),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        root.paint(&mut paint);

        assert_eq!(
            paint.scene().content_bounds(),
            Some(Rect::new(15.0, 17.0, 80.0, 50.0))
        );
        assert_eq!(
            paint.scene().paint_bounds(),
            Some(Rect::new(15.0, 17.0, 50.0, 43.0))
        );
        assert_eq!(
            paint.scene().paint_bounds(),
            paint.scene().clone().paint_bounds()
        );
    }

    #[test]
    fn paint_ctx_emits_extended_scene_commands() {
        let mut paint = PaintCtx::new(
            WindowId::new(11),
            WidgetId::new(12),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );

        let mut path = sui_core::Path::builder();
        path.move_to(Point::new(4.0, 5.0))
            .line_to(Point::new(24.0, 5.0))
            .line_to(Point::new(14.0, 15.0))
            .close();
        paint.stroke(path.build(), Color::WHITE, StrokeStyle::new(2.0));
        paint.draw_text(
            Rect::new(8.0, 10.0, 80.0, 20.0),
            "hello",
            TextStyle::new(Color::BLACK),
        );
        paint.draw_image(
            Rect::new(0.0, 0.0, 16.0, 16.0),
            sui_core::ImageHandle::new(3),
        );
        paint.push_clip(sui_core::Path::circle(Point::new(12.0, 12.0), 8.0));
        paint.push_clip_rect(Rect::new(0.0, 0.0, 50.0, 50.0));
        paint.translate(Vector::new(3.0, 4.0));
        paint.pop_transform();
        paint.pop_clip();
        paint.pop_clip();

        assert!(matches!(
            paint.scene().commands()[0],
            SceneCommand::StrokePath { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[1],
            SceneCommand::DrawShapedText(_)
        ));
        assert!(matches!(
            paint.scene().commands()[2],
            SceneCommand::DrawImage { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[3],
            SceneCommand::PushClipPath { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[4],
            SceneCommand::PushClip { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[5],
            SceneCommand::PushTransform { .. }
        ));
        assert!(matches!(
            paint.scene().commands()[6],
            SceneCommand::PopTransform
        ));
        assert!(matches!(paint.scene().commands()[7], SceneCommand::PopClip));
        assert!(matches!(paint.scene().commands()[8], SceneCommand::PopClip));
    }

    #[test]
    fn stable_text_run_handle_ignores_color_and_includes_layout_style() {
        let run = TextRun {
            rect: Rect::new(8.0, 10.0, 80.0, 20.0),
            text: "hello".to_string(),
            style: TextStyle::new(Color::BLACK),
        };
        let mut recolored = run.clone();
        recolored.style.color = Color::WHITE;
        let mut bold = run.clone();
        bold.style.weight = FontWeight::BOLD;

        assert_eq!(
            super::stable_text_run_handle(&run),
            super::stable_text_run_handle(&recolored)
        );
        assert_ne!(
            super::stable_text_run_handle(&run),
            super::stable_text_run_handle(&bold)
        );
    }

    #[test]
    fn paint_ctx_draw_text_uses_color_override_for_paint_color() {
        let text_system = Arc::new(TextSystem::new());
        let mut paint = PaintCtx::new(
            WindowId::new(12),
            WidgetId::new(13),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
            Arc::clone(&text_system),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        let paint_color = Color::BLACK;
        paint.draw_text(
            Rect::new(8.0, 10.0, 80.0, 20.0),
            "hello",
            TextStyle::new(paint_color),
        );

        let SceneCommand::DrawShapedText(text) = &paint.scene().commands()[0] else {
            panic!("draw_text should emit shaped text when layout succeeds");
        };
        assert_eq!(text.color_override, Some(paint_color));

        let registry = text_system.text_layout_registry();
        let layout = text
            .resolve(registry.as_ref())
            .expect("drawn shaped text should resolve from the registry");
        assert_eq!(layout.style().color, Color::WHITE);
    }

    #[test]
    fn paint_ctx_draw_persistent_text_layout_reinserts_pruned_layout() {
        let text_system = Arc::new(TextSystem::new());
        let layout = text_system
            .shape_text_persistent(
                None,
                "held",
                Size::new(80.0, 20.0),
                TextStyle::new(Color::WHITE),
                &FontRegistry::new(),
            )
            .unwrap();
        text_system.retain_persistent_layouts(&std::collections::HashSet::new());
        assert!(!text_system.text_layout_registry().contains(layout.handle()));

        let mut paint = PaintCtx::new(
            WindowId::new(12),
            WidgetId::new(13),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
            Arc::clone(&text_system),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        paint.draw_persistent_text_layout(Point::ZERO, &layout);

        assert!(text_system.text_layout_registry().contains(layout.handle()));
        assert!(matches!(
            paint.scene().commands()[0],
            SceneCommand::DrawShapedText(_)
        ));
    }

    #[test]
    fn text_layout_shapes_in_measure_and_paints_as_shaped_scene_output() {
        let layout = measure_ctx(WindowId::new(13), WidgetId::new(14))
            .layout()
            .shape_text(
                "hello",
                sui_core::Size::new(80.0, 20.0),
                TextStyle::new(Color::BLACK),
            )
            .unwrap();

        let mut paint = PaintCtx::new(
            WindowId::new(13),
            WidgetId::new(14),
            Rect::new(0.0, 0.0, 120.0, 60.0),
            None,
            DpiInfo::default(),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(ImageRegistry::new()),
        );
        let origin = Point::new(8.0, 10.0);
        paint.draw_text_layout(origin, &layout);
        paint.set_ime_composition_rect(layout.caret_rect(3).translate(origin.to_vector()));

        assert!(matches!(
            paint.scene().commands()[0],
            SceneCommand::DrawShapedText(_)
        ));
        assert!(paint.ime_composition_rect().is_some());
        assert!(!layout.selection_rects(1..4).is_empty());
    }
}
