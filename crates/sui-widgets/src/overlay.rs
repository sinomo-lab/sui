use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use sui_core::{
    Event, Rect, SemanticsLiveRegion, SemanticsNode, SemanticsRole, Size, TimerToken, WakeEvent,
    WidgetId,
};
use sui_layout::Constraints;
use sui_reactive::Signal;
use sui_runtime::{
    ArrangeCtx, Command, EventCtx, LayerOptions, MeasureCtx, OverlayDismissPolicy,
    OverlayFocusBehavior, OverlayKind, OverlayOptions, PaintBoundaryMode, PaintCtx,
    REACTIVE_CHANGED, SemanticsCtx, SingleChild, StackHostOptions, StackSurfaceOptions, Widget,
    WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::{Border, LayerCompositionMode};

use crate::DefaultTheme;

use sui_core::Point;

/// Defines an independent overlay stacking root.
///
/// Windows are stacking hosts automatically. Use this widget for embedded
/// viewports or independently ordered workspace regions; focus and dismissal
/// remain coordinated by the containing window's overlay manager.
pub struct OverlayHost {
    child: SingleChild,
}

impl OverlayHost {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            child: SingleChild::new(child),
        }
    }

    pub fn child(&self) -> &WidgetPod {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut WidgetPod {
        self.child.child_mut()
    }
}

impl Widget for OverlayHost {
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

    fn stack_host_options(&self) -> Option<StackHostOptions> {
        Some(StackHostOptions::default())
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlaySide {
    Top,
    Bottom,
    Left,
    Right,
}

impl OverlaySide {
    pub const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayPlacement {
    pub side: OverlaySide,
    pub alignment: OverlayAlignment,
}

impl OverlayPlacement {
    pub const TOP_START: Self = Self::new(OverlaySide::Top, OverlayAlignment::Start);
    pub const TOP_CENTER: Self = Self::new(OverlaySide::Top, OverlayAlignment::Center);
    pub const TOP_END: Self = Self::new(OverlaySide::Top, OverlayAlignment::End);
    pub const BOTTOM_START: Self = Self::new(OverlaySide::Bottom, OverlayAlignment::Start);
    pub const BOTTOM_CENTER: Self = Self::new(OverlaySide::Bottom, OverlayAlignment::Center);
    pub const BOTTOM_END: Self = Self::new(OverlaySide::Bottom, OverlayAlignment::End);
    pub const LEFT_START: Self = Self::new(OverlaySide::Left, OverlayAlignment::Start);
    pub const LEFT_CENTER: Self = Self::new(OverlaySide::Left, OverlayAlignment::Center);
    pub const LEFT_END: Self = Self::new(OverlaySide::Left, OverlayAlignment::End);
    pub const RIGHT_START: Self = Self::new(OverlaySide::Right, OverlayAlignment::Start);
    pub const RIGHT_CENTER: Self = Self::new(OverlaySide::Right, OverlayAlignment::Center);
    pub const RIGHT_END: Self = Self::new(OverlaySide::Right, OverlayAlignment::End);

    pub const fn new(side: OverlaySide, alignment: OverlayAlignment) -> Self {
        Self { side, alignment }
    }

    pub const fn flipped(self) -> Self {
        Self::new(self.side.opposite(), self.alignment)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayCollisionPolicy {
    pub flip: bool,
    pub shift: bool,
    pub resize: bool,
}

impl OverlayCollisionPolicy {
    pub const NONE: Self = Self {
        flip: false,
        shift: false,
        resize: false,
    };

    pub const ADAPTIVE: Self = Self {
        flip: true,
        shift: true,
        resize: true,
    };
}

impl Default for OverlayCollisionPolicy {
    fn default() -> Self {
        Self::ADAPTIVE
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayPlacementRequest {
    pub anchor: Rect,
    pub overlay_size: Size,
    pub viewport: Rect,
    pub preferred: OverlayPlacement,
    pub fallbacks: Vec<OverlayPlacement>,
    pub gap: f32,
    pub margin: f32,
    pub collision: OverlayCollisionPolicy,
}

impl OverlayPlacementRequest {
    pub fn new(
        anchor: Rect,
        overlay_size: Size,
        viewport: Rect,
        preferred: OverlayPlacement,
    ) -> Self {
        Self {
            anchor,
            overlay_size,
            viewport,
            preferred,
            fallbacks: Vec::new(),
            gap: 0.0,
            margin: 4.0,
            collision: OverlayCollisionPolicy::ADAPTIVE,
        }
    }

    pub fn fallbacks(mut self, fallbacks: impl IntoIterator<Item = OverlayPlacement>) -> Self {
        self.fallbacks = fallbacks.into_iter().collect();
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin.max(0.0);
        self
    }

    pub fn collision(mut self, collision: OverlayCollisionPolicy) -> Self {
        self.collision = collision;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayPlacementResult {
    pub bounds: Rect,
    pub placement: OverlayPlacement,
    pub shifted: bool,
    pub resized: bool,
    pub overflow: f32,
}

/// Place an overlay relative to an anchor while keeping it inside a viewport.
///
/// Candidates are evaluated before shifting so an actually fitting fallback
/// wins over a preferred side that would need a large corrective movement.
pub fn place_overlay(request: &OverlayPlacementRequest) -> OverlayPlacementResult {
    if !request.viewport.width().is_finite()
        || !request.viewport.height().is_finite()
        || request.viewport.width() <= 0.0
        || request.viewport.height() <= 0.0
    {
        return OverlayPlacementResult {
            bounds: placement_rect(
                request.anchor,
                request.overlay_size,
                request.preferred,
                request.gap,
            ),
            placement: request.preferred,
            shifted: false,
            resized: false,
            overflow: 0.0,
        };
    }
    let safe_viewport = inset_rect(request.viewport, request.margin);
    let mut size = request.overlay_size;
    let mut resized = false;
    if request.collision.resize {
        let next = Size::new(
            size.width.min(safe_viewport.width()).max(0.0),
            size.height.min(safe_viewport.height()).max(0.0),
        );
        resized = next != size;
        size = next;
    }

    let mut candidates = vec![request.preferred];
    candidates.extend(request.fallbacks.iter().copied());
    if request.collision.flip {
        let flipped = request.preferred.flipped();
        if !candidates.contains(&flipped) {
            candidates.push(flipped);
        }
    }

    let mut best = None::<(usize, f32, Rect, OverlayPlacement)>;
    for (index, placement) in candidates.into_iter().enumerate() {
        let raw = placement_rect(request.anchor, size, placement, request.gap);
        let overflow = overflow_amount(raw, safe_viewport);
        let score = overflow + index as f32 * 0.001;
        if best
            .as_ref()
            .is_none_or(|(_, best_score, _, _)| score < *best_score)
        {
            best = Some((index, score, raw, placement));
        }
    }

    let (_, _, raw, placement) = best.unwrap_or((0, 0.0, Rect::ZERO, request.preferred));
    let overflow = overflow_amount(raw, safe_viewport);
    let bounds = if request.collision.shift {
        shift_inside(raw, safe_viewport)
    } else {
        raw
    };
    OverlayPlacementResult {
        bounds,
        placement,
        shifted: bounds.origin != raw.origin,
        resized,
        overflow,
    }
}

fn placement_rect(anchor: Rect, size: Size, placement: OverlayPlacement, gap: f32) -> Rect {
    let (x, y) = match placement.side {
        OverlaySide::Top | OverlaySide::Bottom => {
            let x = match placement.alignment {
                OverlayAlignment::Start => anchor.x(),
                OverlayAlignment::Center => anchor.x() + (anchor.width() - size.width) * 0.5,
                OverlayAlignment::End => anchor.max_x() - size.width,
            };
            let y = if placement.side == OverlaySide::Top {
                anchor.y() - gap - size.height
            } else {
                anchor.max_y() + gap
            };
            (x, y)
        }
        OverlaySide::Left | OverlaySide::Right => {
            let y = match placement.alignment {
                OverlayAlignment::Start => anchor.y(),
                OverlayAlignment::Center => anchor.y() + (anchor.height() - size.height) * 0.5,
                OverlayAlignment::End => anchor.max_y() - size.height,
            };
            let x = if placement.side == OverlaySide::Left {
                anchor.x() - gap - size.width
            } else {
                anchor.max_x() + gap
            };
            (x, y)
        }
    };
    Rect::new(x, y, size.width, size.height)
}

fn inset_rect(rect: Rect, margin: f32) -> Rect {
    let horizontal = margin.min(rect.width() * 0.5);
    let vertical = margin.min(rect.height() * 0.5);
    Rect::new(
        rect.x() + horizontal,
        rect.y() + vertical,
        (rect.width() - horizontal * 2.0).max(0.0),
        (rect.height() - vertical * 2.0).max(0.0),
    )
}

fn overflow_amount(rect: Rect, viewport: Rect) -> f32 {
    (viewport.x() - rect.x()).max(0.0)
        + (rect.max_x() - viewport.max_x()).max(0.0)
        + (viewport.y() - rect.y()).max(0.0)
        + (rect.max_y() - viewport.max_y()).max(0.0)
}

fn shift_inside(rect: Rect, viewport: Rect) -> Rect {
    let max_x = (viewport.max_x() - rect.width()).max(viewport.x());
    let max_y = (viewport.max_y() - rect.height()).max(viewport.y());
    Rect::from_origin_size(
        Point::new(
            rect.x().clamp(viewport.x(), max_x),
            rect.y().clamp(viewport.y(), max_y),
        ),
        rect.size,
    )
}

static NEXT_NOTIFICATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationId(u64);

impl NotificationId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Polite,
    Assertive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransientNotification {
    pub id: NotificationId,
    pub title: String,
    pub message: String,
    pub duration: Option<f64>,
    pub urgency: NotificationUrgency,
}

impl TransientNotification {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: NotificationId(NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::Relaxed)),
            title: title.into(),
            message: message.into(),
            duration: Some(5.0),
            urgency: NotificationUrgency::Polite,
        }
    }

    pub fn duration(mut self, seconds: f64) -> Self {
        self.duration = Some(seconds.max(0.0));
        self
    }

    pub fn persistent(mut self) -> Self {
        self.duration = None;
        self
    }

    pub fn urgency(mut self, urgency: NotificationUrgency) -> Self {
        self.urgency = urgency;
        self
    }
}

#[derive(Default)]
struct NotificationCenterInner {
    revision: u64,
    notifications: Vec<TransientNotification>,
}

/// Thread-safe producer for transient, window-hosted notifications.
#[derive(Clone)]
pub struct NotificationCenter {
    inner: Arc<Mutex<NotificationCenterInner>>,
    revision: Signal<u64>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(NotificationCenterInner::default())),
            revision: Signal::named("NotificationCenter", 0),
        }
    }

    pub fn push(&self, notification: TransientNotification) -> NotificationId {
        let id = notification.id;
        let revision = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner.notifications.push(notification);
            inner.revision = inner.revision.wrapping_add(1);
            inner.revision
        };
        self.revision.set(revision);
        id
    }

    pub fn notify(&self, title: impl Into<String>, message: impl Into<String>) -> NotificationId {
        self.push(TransientNotification::new(title, message))
    }

    pub fn dismiss(&self, id: NotificationId) -> bool {
        let (changed, revision) = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let before = inner.notifications.len();
            inner
                .notifications
                .retain(|notification| notification.id != id);
            let changed = before != inner.notifications.len();
            if changed {
                inner.revision = inner.revision.wrapping_add(1);
            }
            (changed, inner.revision)
        };
        if changed {
            self.revision.set(revision);
        }
        changed
    }

    pub fn clear(&self) -> bool {
        let (changed, revision) = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let changed = !inner.notifications.is_empty();
            inner.notifications.clear();
            if changed {
                inner.revision = inner.revision.wrapping_add(1);
            }
            (changed, inner.revision)
        };
        if changed {
            self.revision.set(revision);
        }
        changed
    }

    pub fn snapshot(&self) -> Vec<TransientNotification> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .notifications
            .clone()
    }
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

/// Window-level visual and semantic host for a [`NotificationCenter`].
pub struct NotificationHost {
    center: NotificationCenter,
    notifications: Vec<TransientNotification>,
    timers: HashMap<TimerToken, NotificationId>,
    scheduled: HashMap<NotificationId, TimerToken>,
    frames: Vec<(NotificationId, Rect)>,
    theme: DefaultTheme,
    width: f32,
    margin: f32,
    gap: f32,
}

impl NotificationHost {
    pub fn new(center: NotificationCenter) -> Self {
        Self {
            center,
            notifications: Vec::new(),
            timers: HashMap::new(),
            scheduled: HashMap::new(),
            frames: Vec::new(),
            theme: DefaultTheme::default(),
            width: 340.0,
            margin: 16.0,
            gap: 8.0,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(120.0);
        self
    }

    fn sync(&mut self, ctx: &mut EventCtx) {
        self.notifications = self.center.snapshot();
        let active = self
            .notifications
            .iter()
            .map(|notification| notification.id)
            .collect::<Vec<_>>();
        let obsolete = self
            .scheduled
            .keys()
            .copied()
            .filter(|id| !active.contains(id))
            .collect::<Vec<_>>();
        for id in obsolete {
            if let Some(token) = self.scheduled.remove(&id) {
                self.timers.remove(&token);
                ctx.cancel_timer(token);
            }
        }
        for notification in &self.notifications {
            let Some(duration) = notification.duration else {
                continue;
            };
            if self.scheduled.contains_key(&notification.id) {
                continue;
            }
            let token = ctx.schedule_timer_after(duration);
            self.scheduled.insert(notification.id, token);
            self.timers.insert(token, notification.id);
        }
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn notification_height(&self, notification: &TransientNotification) -> f32 {
        if notification.message.is_empty() {
            48.0
        } else {
            68.0
        }
    }
}

impl Widget for NotificationHost {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.is(REACTIVE_CHANGED) {
            self.sync(ctx);
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Wake(WakeEvent::AnimationFrame { .. }) => {
                self.sync(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::Timer { token, .. }) => {
                if let Some(id) = self.timers.remove(token) {
                    self.scheduled.remove(&id);
                    self.center.dismiss(id);
                    self.sync(ctx);
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let _ = ctx.observe(&self.center.revision);
        self.notifications = self.center.snapshot();
        if self.notifications.iter().any(|notification| {
            notification.duration.is_some() && !self.scheduled.contains_key(&notification.id)
        }) {
            ctx.request_animation_frame();
        }
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                self.width + self.margin * 2.0
            },
            if constraints.max.height.is_finite() {
                constraints.max.height
            } else {
                480.0
            },
        ))
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, bounds: Rect) {
        self.frames.clear();
        let width = self
            .width
            .min((bounds.width() - self.margin * 2.0).max(0.0));
        let mut y = bounds.y() + self.margin;
        for notification in &self.notifications {
            let height = self.notification_height(notification);
            self.frames.push((
                notification.id,
                Rect::new(bounds.max_x() - self.margin - width, y, width, height),
            ));
            y += height + self.gap;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let metrics = self.theme.metrics;
        let title_style = self.theme.body_text_style();
        let message_style = self.theme.placeholder_text_style();
        for notification in &self.notifications {
            let Some((_, frame)) = self.frames.iter().find(|(id, _)| *id == notification.id) else {
                continue;
            };
            ctx.fill_rrect_bordered(
                *frame,
                [metrics.corner_radius; 4],
                self.theme.palette.surface_raised,
                Border {
                    width: metrics.border_width,
                    color: self.theme.palette.border,
                },
            );
            let padding = 12.0;
            let title_rect = Rect::new(
                frame.x() + padding,
                frame.y() + 8.0,
                (frame.width() - padding * 2.0).max(0.0),
                title_style.line_height,
            );
            ctx.draw_text(title_rect, notification.title.clone(), title_style.clone());
            if !notification.message.is_empty() {
                let message_rect = Rect::new(
                    title_rect.x(),
                    title_rect.max_y() + 4.0,
                    title_rect.width(),
                    message_style.line_height,
                );
                ctx.draw_text(
                    message_rect,
                    notification.message.clone(),
                    message_style.clone(),
                );
            }
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: if self.notifications.is_empty() {
                LayerCompositionMode::Normal
            } else {
                LayerCompositionMode::Overlay
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        (!self.notifications.is_empty()).then_some(StackSurfaceOptions {
            transient: true,
            hit_test: false,
            ..StackSurfaceOptions::default()
        })
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (!self.notifications.is_empty()).then_some(
            OverlayOptions::new(OverlayKind::Notification)
                .dismiss(OverlayDismissPolicy::NONE)
                .focus(OverlayFocusBehavior::NONE),
        )
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        for notification in &self.notifications {
            let Some((_, frame)) = self.frames.iter().find(|(id, _)| *id == notification.id) else {
                continue;
            };
            let mut node = SemanticsNode::new(
                WidgetId::new((1_u64 << 62) | notification.id.get()),
                SemanticsRole::Status,
                *frame,
            );
            node.parent = Some(ctx.widget_id());
            node.name = Some(notification.title.clone());
            node.description =
                (!notification.message.is_empty()).then(|| notification.message.clone());
            node.live_region = Some(match notification.urgency {
                NotificationUrgency::Polite => SemanticsLiveRegion::Polite,
                NotificationUrgency::Assertive => SemanticsLiveRegion::Assertive,
            });
            ctx.push(node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_runtime::{Application, WindowBuilder};

    #[test]
    fn placement_flips_before_shifting_when_the_other_side_fits() {
        let result = place_overlay(
            &OverlayPlacementRequest::new(
                Rect::new(80.0, 170.0, 40.0, 20.0),
                Size::new(100.0, 70.0),
                Rect::new(0.0, 0.0, 240.0, 200.0),
                OverlayPlacement::BOTTOM_START,
            )
            .gap(4.0),
        );
        assert_eq!(result.placement, OverlayPlacement::TOP_START);
        assert_eq!(result.bounds, Rect::new(80.0, 96.0, 100.0, 70.0));
        assert!(!result.shifted);
    }

    #[test]
    fn placement_shifts_and_resizes_inside_safe_viewport() {
        let result = place_overlay(
            &OverlayPlacementRequest::new(
                Rect::new(190.0, 90.0, 10.0, 10.0),
                Size::new(260.0, 140.0),
                Rect::new(0.0, 0.0, 200.0, 120.0),
                OverlayPlacement::RIGHT_START,
            )
            .margin(8.0),
        );
        assert_eq!(result.bounds, Rect::new(8.0, 8.0, 184.0, 104.0));
        assert!(result.resized);
        assert!(result.shifted);
    }

    #[test]
    fn notification_host_exposes_live_status_and_expires_it() {
        let center = NotificationCenter::new();
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Notifications")
                    .root(NotificationHost::new(center.clone())),
            )
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        let initial = runtime.render(window_id).unwrap();
        assert!(
            initial
                .semantics
                .iter()
                .all(|node| node.role != SemanticsRole::Status)
        );

        center.push(
            TransientNotification::new("Upload complete", "artifact.zip is ready")
                .duration(0.05)
                .urgency(NotificationUrgency::Assertive),
        );
        let _ = runtime.render(window_id).unwrap();
        for (ready_window, event) in runtime.drain_ready_events() {
            runtime.handle_event(ready_window, event).unwrap();
        }
        let shown = runtime.render(window_id).unwrap();
        let status = shown
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Status)
            .expect("notification status semantics");
        assert_eq!(status.name.as_deref(), Some("Upload complete"));
        assert_eq!(status.live_region, Some(SemanticsLiveRegion::Assertive));
        assert_eq!(
            runtime.overlay_snapshot(window_id).unwrap().overlays.len(),
            1
        );

        runtime.tick(0.06);
        for (ready_window, event) in runtime.drain_ready_events() {
            runtime.handle_event(ready_window, event).unwrap();
        }
        let expired = runtime.render(window_id).unwrap();
        assert!(
            expired
                .semantics
                .iter()
                .all(|node| node.role != SemanticsRole::Status)
        );
        assert!(
            runtime
                .overlay_snapshot(window_id)
                .unwrap()
                .overlays
                .is_empty()
        );
    }

    #[test]
    fn notification_center_accepts_threaded_producers() {
        let center = NotificationCenter::new();
        let producer = center.clone();
        std::thread::spawn(move || {
            producer.notify("Background task", "Indexed 42 files");
        })
        .join()
        .unwrap();

        let notifications = center.snapshot();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].title, "Background task");
    }
}
