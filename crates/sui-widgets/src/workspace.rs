use sui_core::{
    Event, KeyState, Point, PointerButton, PointerEventKind, Rect, SemanticsAction,
    SemanticsActionRequest, SemanticsNode, SemanticsRole, Size,
};
use sui_layout::Constraints;
use sui_reactive::Signal;
use sui_runtime::{
    ArrangeCtx, Command, EventCtx, FocusScope, FocusScopeState, LayerOptions, MeasureCtx,
    OVERLAY_DISMISS_REQUEST, OverlayDismissPolicy, OverlayFocusBehavior, OverlayKind,
    OverlayOptions, PaintBoundaryMode, PaintCtx, SemanticsCtx, SingleChild, StackSurfaceOptions,
    Widget, WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::LayerCompositionMode;

use crate::{DefaultTheme, SplitExtent, SplitState, SplitStateSnapshot, ThemeBreakpoints};

/// Constraint-derived presentation class for adaptive workspace layouts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AdaptiveClass {
    #[default]
    Compact,
    Medium,
    Expanded,
}

/// Width thresholds used by adaptive widgets.
///
/// Classification uses the widget's own incoming constraints, not a global
/// window width, so adaptive views also work correctly inside split panes and
/// embedded surfaces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveBreakpoints {
    pub medium: f32,
    pub expanded: f32,
}

impl AdaptiveBreakpoints {
    pub fn new(medium: f32, expanded: f32) -> Self {
        let medium = medium.max(0.0);
        Self {
            medium,
            expanded: expanded.max(medium),
        }
    }

    pub fn classify(self, width: f32) -> AdaptiveClass {
        if width < self.medium {
            AdaptiveClass::Compact
        } else if width < self.expanded {
            AdaptiveClass::Medium
        } else {
            AdaptiveClass::Expanded
        }
    }
}

impl Default for AdaptiveBreakpoints {
    fn default() -> Self {
        Self::new(640.0, 1024.0)
    }
}

impl From<ThemeBreakpoints> for AdaptiveBreakpoints {
    fn from(value: ThemeBreakpoints) -> Self {
        Self::new(value.sm, value.lg)
    }
}

fn constraint_width(constraints: Constraints) -> f32 {
    if constraints.max.width.is_finite() {
        constraints.max.width
    } else {
        constraints.min.width
    }
}

fn constraint_height(constraints: Constraints) -> f32 {
    if constraints.max.height.is_finite() {
        constraints.max.height
    } else {
        constraints.min.height
    }
}

fn local_rect(width: f32, height: f32) -> Rect {
    Rect::from_origin_size(Point::ZERO, Size::new(width.max(0.0), height.max(0.0)))
}

/// Retains one subtree per presentation class and switches between them based
/// on the space allocated by the parent.
///
/// Each subtree keeps its widget identity while inactive. Focus history is
/// scoped per presentation and restored when that presentation becomes active
/// again.
pub struct AdaptiveView {
    breakpoints: AdaptiveBreakpoints,
    active: AdaptiveClass,
    compact_scope: FocusScopeState,
    medium_scope: FocusScopeState,
    expanded_scope: FocusScopeState,
    compact: SingleChild,
    medium: SingleChild,
    expanded: SingleChild,
    on_class_change: Option<Box<dyn FnMut(AdaptiveClass)>>,
}

impl AdaptiveView {
    pub fn new<C, M, E>(compact: C, medium: M, expanded: E) -> Self
    where
        C: Widget + 'static,
        M: Widget + 'static,
        E: Widget + 'static,
    {
        let compact_scope = FocusScopeState::new();
        let medium_scope = FocusScopeState::new();
        let expanded_scope = FocusScopeState::new();
        Self {
            breakpoints: AdaptiveBreakpoints::default(),
            active: AdaptiveClass::Compact,
            compact: SingleChild::new(FocusScope::new(compact).state(compact_scope.clone())),
            medium: SingleChild::new(FocusScope::new(medium).state(medium_scope.clone())),
            expanded: SingleChild::new(FocusScope::new(expanded).state(expanded_scope.clone())),
            compact_scope,
            medium_scope,
            expanded_scope,
            on_class_change: None,
        }
    }

    pub fn breakpoints(mut self, breakpoints: AdaptiveBreakpoints) -> Self {
        self.breakpoints = AdaptiveBreakpoints::new(breakpoints.medium, breakpoints.expanded);
        self
    }

    pub fn on_class_change<F>(mut self, on_class_change: F) -> Self
    where
        F: FnMut(AdaptiveClass) + 'static,
    {
        self.on_class_change = Some(Box::new(on_class_change));
        self
    }

    pub fn active_class(&self) -> AdaptiveClass {
        self.active
    }

    fn active(&self) -> &SingleChild {
        match self.active {
            AdaptiveClass::Compact => &self.compact,
            AdaptiveClass::Medium => &self.medium,
            AdaptiveClass::Expanded => &self.expanded,
        }
    }

    fn active_mut(&mut self) -> &mut SingleChild {
        match self.active {
            AdaptiveClass::Compact => &mut self.compact,
            AdaptiveClass::Medium => &mut self.medium,
            AdaptiveClass::Expanded => &mut self.expanded,
        }
    }

    fn activate(&mut self, class: AdaptiveClass) {
        if self.active == class {
            return;
        }
        self.active = class;
        match class {
            AdaptiveClass::Compact => self.compact_scope.request_restore(),
            AdaptiveClass::Medium => self.medium_scope.request_restore(),
            AdaptiveClass::Expanded => self.expanded_scope.request_restore(),
        };
        if let Some(on_class_change) = &mut self.on_class_change {
            on_class_change(class);
        }
    }
}

impl Widget for AdaptiveView {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.activate(self.breakpoints.classify(constraint_width(constraints)));
        self.active_mut().measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.active_mut().arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.active().paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.active().semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.active().visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.active_mut().visit_children_mut(visitor);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConstraintOrientation {
    #[default]
    Any,
    Portrait,
    Landscape,
}

/// Serializable, locally evaluated container query.
///
/// Queries use the widget's incoming constraints and never consult global
/// window size, so the same view works inside split panes and embedded hosts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstraintQuery {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub min_aspect_ratio: Option<f32>,
    pub max_aspect_ratio: Option<f32>,
    pub orientation: ConstraintOrientation,
}

impl ConstraintQuery {
    pub const fn new() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            min_aspect_ratio: None,
            max_aspect_ratio: None,
            orientation: ConstraintOrientation::Any,
        }
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width.max(0.0));
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width.max(0.0));
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height.max(0.0));
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height.max(0.0));
        self
    }

    pub fn min_aspect_ratio(mut self, ratio: f32) -> Self {
        self.min_aspect_ratio = Some(normalize_query_ratio(ratio));
        self
    }

    pub fn max_aspect_ratio(mut self, ratio: f32) -> Self {
        self.max_aspect_ratio = Some(normalize_query_ratio(ratio));
        self
    }

    pub const fn orientation(mut self, orientation: ConstraintOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn matches(self, constraints: Constraints) -> bool {
        let width = constraint_width(constraints);
        let height = constraint_height(constraints);
        let aspect_ratio = if height > 0.0 { width / height } else { width };
        self.min_width.is_none_or(|minimum| width >= minimum)
            && self.max_width.is_none_or(|maximum| width <= maximum)
            && self.min_height.is_none_or(|minimum| height >= minimum)
            && self.max_height.is_none_or(|maximum| height <= maximum)
            && self
                .min_aspect_ratio
                .is_none_or(|minimum| aspect_ratio >= minimum)
            && self
                .max_aspect_ratio
                .is_none_or(|maximum| aspect_ratio <= maximum)
            && match self.orientation {
                ConstraintOrientation::Any => true,
                ConstraintOrientation::Portrait => height >= width,
                ConstraintOrientation::Landscape => width > height,
            }
    }
}

impl Default for ConstraintQuery {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_query_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.max(0.0)
    } else {
        0.0
    }
}

struct ConstraintBranch {
    query: ConstraintQuery,
    focus: FocusScopeState,
    child: SingleChild,
}

impl ConstraintBranch {
    fn new(query: ConstraintQuery, child: impl Widget + 'static) -> Self {
        let focus = FocusScopeState::new();
        Self {
            query,
            child: SingleChild::new(FocusScope::new(child).state(focus.clone())),
            focus,
        }
    }
}

/// Retains each declarative query branch and visits only the first matching one.
pub struct ConstraintView {
    branches: Vec<ConstraintBranch>,
    fallback_focus: FocusScopeState,
    fallback: SingleChild,
    active: Option<usize>,
}

impl ConstraintView {
    pub fn new<W>(fallback: W) -> Self
    where
        W: Widget + 'static,
    {
        let fallback_focus = FocusScopeState::new();
        Self {
            branches: Vec::new(),
            fallback: SingleChild::new(FocusScope::new(fallback).state(fallback_focus.clone())),
            fallback_focus,
            active: None,
        }
    }

    pub fn when<W>(mut self, query: ConstraintQuery, child: W) -> Self
    where
        W: Widget + 'static,
    {
        self.branches.push(ConstraintBranch::new(query, child));
        self
    }

    pub const fn active_branch(&self) -> Option<usize> {
        self.active
    }

    fn active(&self) -> &SingleChild {
        match self.active {
            Some(index) => &self.branches[index].child,
            None => &self.fallback,
        }
    }

    fn active_mut(&mut self) -> &mut SingleChild {
        match self.active {
            Some(index) => &mut self.branches[index].child,
            None => &mut self.fallback,
        }
    }

    fn select(&mut self, constraints: Constraints) {
        let active = self
            .branches
            .iter()
            .position(|branch| branch.query.matches(constraints));
        if self.active == active {
            return;
        }
        self.active = active;
        if let Some(index) = active {
            self.branches[index].focus.request_restore();
        } else {
            self.fallback_focus.request_restore();
        }
    }
}

impl Widget for ConstraintView {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.select(constraints);
        self.active_mut().measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.active_mut().arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.active().paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.active().semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.active().visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.active_mut().visit_children_mut(visitor);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponsiveSidebarSnapshot {
    pub expanded: bool,
    pub overlay_open: bool,
}

impl Default for ResponsiveSidebarSnapshot {
    fn default() -> Self {
        Self {
            expanded: true,
            overlay_open: false,
        }
    }
}

/// Cloneable application-owned state for a responsive sidebar.
#[derive(Clone, Debug)]
pub struct ResponsiveSidebarState {
    snapshot: Signal<ResponsiveSidebarSnapshot>,
}

impl Default for ResponsiveSidebarState {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponsiveSidebarState {
    pub fn new() -> Self {
        Self {
            snapshot: Signal::named(
                "ResponsiveSidebarState",
                ResponsiveSidebarSnapshot::default(),
            ),
        }
    }

    pub fn snapshot(&self) -> ResponsiveSidebarSnapshot {
        self.snapshot.get()
    }

    pub fn apply_snapshot(&self, snapshot: ResponsiveSidebarSnapshot) -> bool {
        self.snapshot.set(snapshot)
    }

    pub fn set_expanded(&self, expanded: bool) -> bool {
        self.snapshot
            .update(|snapshot| snapshot.expanded = expanded)
    }

    pub fn toggle_expanded(&self) -> bool {
        self.snapshot
            .update(|snapshot| snapshot.expanded = !snapshot.expanded)
    }

    pub fn open_overlay(&self) -> bool {
        self.snapshot
            .update(|snapshot| snapshot.overlay_open = true)
    }

    pub fn close_overlay(&self) -> bool {
        self.snapshot
            .update(|snapshot| snapshot.overlay_open = false)
    }

    pub fn toggle_overlay(&self) -> bool {
        self.snapshot
            .update(|snapshot| snapshot.overlay_open = !snapshot.overlay_open)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsiveSidebarMode {
    OverlayClosed,
    OverlayOpen,
    Rail,
    Inline,
}

/// A stable sidebar/content layout that becomes an overlay drawer in compact
/// constraints and a collapsible rail or inline pane in wider constraints.
pub struct ResponsiveSidebar {
    theme: DefaultTheme,
    name: Option<String>,
    breakpoints: AdaptiveBreakpoints,
    state: ResponsiveSidebarState,
    snapshot: ResponsiveSidebarSnapshot,
    split_state: SplitState,
    split_snapshot: SplitStateSnapshot,
    rail_width: f32,
    overlay_width: f32,
    mode: ResponsiveSidebarMode,
    sidebar_frame: Rect,
    content_frame: Rect,
    sidebar_scope: FocusScopeState,
    content_scope: FocusScopeState,
    sidebar: SingleChild,
    content: SingleChild,
    dismiss_on_scrim: bool,
    on_mode_change: Option<Box<dyn FnMut(ResponsiveSidebarMode)>>,
}

impl ResponsiveSidebar {
    pub fn new<S, C>(sidebar: S, content: C) -> Self
    where
        S: Widget + 'static,
        C: Widget + 'static,
    {
        let sidebar_scope = FocusScopeState::new();
        let content_scope = FocusScopeState::new();
        let split_state = SplitState::pixels(280.0);
        Self {
            theme: DefaultTheme::default(),
            name: None,
            breakpoints: AdaptiveBreakpoints::default(),
            state: ResponsiveSidebarState::new(),
            snapshot: ResponsiveSidebarSnapshot::default(),
            split_snapshot: split_state.snapshot(),
            split_state,
            rail_width: 56.0,
            overlay_width: 320.0,
            mode: ResponsiveSidebarMode::OverlayClosed,
            sidebar_frame: Rect::ZERO,
            content_frame: Rect::ZERO,
            sidebar: SingleChild::new(FocusScope::new(sidebar).state(sidebar_scope.clone())),
            content: SingleChild::new(FocusScope::new(content).state(content_scope.clone())),
            sidebar_scope,
            content_scope,
            dismiss_on_scrim: true,
            on_mode_change: None,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn breakpoints(mut self, breakpoints: AdaptiveBreakpoints) -> Self {
        self.breakpoints = AdaptiveBreakpoints::new(breakpoints.medium, breakpoints.expanded);
        self
    }

    pub fn state(mut self, state: ResponsiveSidebarState) -> Self {
        self.snapshot = state.snapshot();
        self.state = state;
        self
    }

    pub fn split_state(mut self, state: SplitState) -> Self {
        self.split_snapshot = state.snapshot();
        self.split_state = state;
        self
    }

    pub fn rail_width(mut self, width: f32) -> Self {
        self.rail_width = width.max(0.0);
        self
    }

    pub fn overlay_width(mut self, width: f32) -> Self {
        self.overlay_width = width.max(0.0);
        self
    }

    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    pub fn on_mode_change<F>(mut self, on_mode_change: F) -> Self
    where
        F: FnMut(ResponsiveSidebarMode) + 'static,
    {
        self.on_mode_change = Some(Box::new(on_mode_change));
        self
    }

    pub fn current_mode(&self) -> ResponsiveSidebarMode {
        self.mode
    }

    pub fn sidebar_state(&self) -> ResponsiveSidebarState {
        self.state.clone()
    }

    fn mode_for(&self, class: AdaptiveClass) -> ResponsiveSidebarMode {
        match class {
            AdaptiveClass::Compact if self.snapshot.overlay_open => {
                ResponsiveSidebarMode::OverlayOpen
            }
            AdaptiveClass::Compact => ResponsiveSidebarMode::OverlayClosed,
            AdaptiveClass::Medium | AdaptiveClass::Expanded if self.snapshot.expanded => {
                ResponsiveSidebarMode::Inline
            }
            AdaptiveClass::Medium | AdaptiveClass::Expanded => ResponsiveSidebarMode::Rail,
        }
    }

    fn activate(&mut self, mode: ResponsiveSidebarMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        match mode {
            ResponsiveSidebarMode::OverlayOpen
            | ResponsiveSidebarMode::Rail
            | ResponsiveSidebarMode::Inline => self.sidebar_scope.request_restore(),
            ResponsiveSidebarMode::OverlayClosed => self.content_scope.request_restore(),
        };
        if let Some(on_mode_change) = &mut self.on_mode_change {
            on_mode_change(mode);
        }
    }

    fn resolved_inline_width(&self, available: f32) -> f32 {
        match self.split_snapshot.extent {
            SplitExtent::Fraction(fraction) => available * fraction,
            SplitExtent::Pixels(pixels) => pixels,
        }
        .clamp(self.rail_width, available)
    }

    fn sidebar_visible(&self) -> bool {
        self.mode != ResponsiveSidebarMode::OverlayClosed
    }

    fn dismiss_overlay(&mut self, ctx: &mut EventCtx) {
        self.state.close_overlay();
        self.content_scope.request_restore();
        ctx.request_measure();
        ctx.request_paint();
        ctx.request_semantics();
    }
}

impl Widget for ResponsiveSidebar {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        if command.get(OVERLAY_DISMISS_REQUEST).is_some()
            && self.mode == ResponsiveSidebarMode::OverlayOpen
        {
            self.dismiss_overlay(ctx);
            ctx.set_handled();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.snapshot = self.state.snapshot();
        if self.mode == ResponsiveSidebarMode::OverlayOpen {
            match event {
                Event::Semantics(semantics)
                    if semantics.target == ctx.widget_id()
                        && matches!(semantics.action, SemanticsActionRequest::Collapse) =>
                {
                    self.dismiss_overlay(ctx);
                    ctx.set_handled();
                }
                Event::Keyboard(key) if key.state == KeyState::Pressed && key.key == "Escape" => {
                    self.dismiss_overlay(ctx);
                    ctx.set_handled();
                }
                Event::Pointer(pointer)
                    if pointer.kind == PointerEventKind::Down
                        && pointer.button == Some(PointerButton::Primary)
                        && !self
                            .sidebar_frame
                            .translate(ctx.bounds().origin.to_vector())
                            .contains(pointer.position) =>
                {
                    if self.dismiss_on_scrim {
                        self.dismiss_overlay(ctx);
                    }
                    ctx.set_handled();
                }
                _ => {}
            }
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.snapshot = ctx.observe(&self.state.snapshot);
        self.split_snapshot = ctx.observe(self.split_state.observable());
        let width = constraint_width(constraints);
        self.activate(self.mode_for(self.breakpoints.classify(width)));

        let size = constraints.clamp(Size::new(width, constraint_height(constraints)));
        let bounds = local_rect(size.width, size.height);
        match self.mode {
            ResponsiveSidebarMode::OverlayClosed => {
                self.sidebar_frame = Rect::ZERO;
                self.content_frame = bounds;
                self.content.measure(ctx, Constraints::tight(size));
            }
            ResponsiveSidebarMode::OverlayOpen => {
                self.content_frame = bounds;
                self.sidebar_frame = local_rect(self.overlay_width.min(size.width), size.height);
                self.content.measure(ctx, Constraints::tight(size));
                self.sidebar
                    .measure(ctx, Constraints::tight(self.sidebar_frame.size));
            }
            ResponsiveSidebarMode::Rail | ResponsiveSidebarMode::Inline => {
                let sidebar_width = if self.mode == ResponsiveSidebarMode::Rail {
                    self.rail_width.min(size.width)
                } else {
                    self.resolved_inline_width(size.width)
                };
                self.sidebar_frame = local_rect(sidebar_width, size.height);
                self.content_frame = Rect::new(
                    sidebar_width,
                    0.0,
                    (size.width - sidebar_width).max(0.0),
                    size.height,
                );
                self.sidebar
                    .measure(ctx, Constraints::tight(self.sidebar_frame.size));
                self.content
                    .measure(ctx, Constraints::tight(self.content_frame.size));
            }
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content
            .arrange(ctx, self.content_frame.translate(bounds.origin.to_vector()));
        if self.sidebar_visible() {
            self.sidebar
                .arrange(ctx, self.sidebar_frame.translate(bounds.origin.to_vector()));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        if self.mode == ResponsiveSidebarMode::OverlayOpen {
            ctx.fill_bounds(self.theme.surfaces.overlay_scrim);
        }
        if self.sidebar_visible() {
            self.sidebar.paint(ctx);
        }
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: if self.mode == ResponsiveSidebarMode::OverlayOpen {
                PaintBoundaryMode::Explicit
            } else {
                PaintBoundaryMode::Flat
            },
            composition_mode: if self.mode == ResponsiveSidebarMode::OverlayOpen {
                LayerCompositionMode::Effect
            } else {
                LayerCompositionMode::Normal
            },
        }
    }

    fn stack_surface_options(&self) -> Option<StackSurfaceOptions> {
        (self.mode == ResponsiveSidebarMode::OverlayOpen).then_some(StackSurfaceOptions {
            transient: true,
            ..StackSurfaceOptions::default()
        })
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        (self.mode == ResponsiveSidebarMode::OverlayOpen).then_some(
            OverlayOptions::new(OverlayKind::Sheet)
                .modal(true)
                .dismiss(OverlayDismissPolicy {
                    escape: true,
                    outside_pointer: self.dismiss_on_scrim,
                })
                .focus(OverlayFocusBehavior::CONTAINED),
        )
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if self.mode == ResponsiveSidebarMode::OverlayOpen {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::Dialog,
                self.sidebar_frame
                    .translate(ctx.bounds().origin.to_vector()),
            );
            node.name = self.name.clone().or_else(|| Some("Navigation".to_string()));
            node.state.expanded = Some(true);
            node.state.modal = true;
            node.actions = vec![SemanticsAction::Collapse];
            ctx.push(node);
            self.sidebar.semantics(ctx);
        } else {
            self.content.semantics(ctx);
            if self.sidebar_visible() {
                self.sidebar.semantics(ctx);
            }
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        if self.sidebar_visible() {
            self.sidebar.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        if self.sidebar_visible() {
            self.sidebar.visit_children_mut(visitor);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MasterDetailRoute {
    #[default]
    Master,
    Detail,
}

/// Cloneable navigation state shared by a master-detail layout and the app.
#[derive(Clone, Debug)]
pub struct MasterDetailState {
    route: Signal<MasterDetailRoute>,
}

impl Default for MasterDetailState {
    fn default() -> Self {
        Self::new(MasterDetailRoute::default())
    }
}

impl MasterDetailState {
    pub fn new(route: MasterDetailRoute) -> Self {
        Self {
            route: Signal::named("MasterDetailState", route),
        }
    }

    pub fn route(&self) -> MasterDetailRoute {
        self.route.get()
    }

    pub fn show_master(&self) -> bool {
        self.route.set(MasterDetailRoute::Master)
    }

    pub fn show_detail(&self) -> bool {
        self.route.set(MasterDetailRoute::Detail)
    }

    pub fn set_route(&self, route: MasterDetailRoute) -> bool {
        self.route.set(route)
    }
}

/// Shows master and detail side-by-side when space permits, and converts to a
/// focus-restoring navigation stack under compact constraints.
pub struct MasterDetail {
    breakpoints: AdaptiveBreakpoints,
    state: MasterDetailState,
    route: MasterDetailRoute,
    split_state: SplitState,
    split_snapshot: SplitStateSnapshot,
    compact: bool,
    master_frame: Rect,
    detail_frame: Rect,
    master_scope: FocusScopeState,
    detail_scope: FocusScopeState,
    master: SingleChild,
    detail: SingleChild,
}

impl MasterDetail {
    pub fn new<M, D>(master: M, detail: D) -> Self
    where
        M: Widget + 'static,
        D: Widget + 'static,
    {
        let master_scope = FocusScopeState::new();
        let detail_scope = FocusScopeState::new();
        let split_state = SplitState::pixels(320.0);
        Self {
            breakpoints: AdaptiveBreakpoints::default(),
            state: MasterDetailState::default(),
            route: MasterDetailRoute::Master,
            split_snapshot: split_state.snapshot(),
            split_state,
            compact: true,
            master_frame: Rect::ZERO,
            detail_frame: Rect::ZERO,
            master: SingleChild::new(FocusScope::new(master).state(master_scope.clone())),
            detail: SingleChild::new(FocusScope::new(detail).state(detail_scope.clone())),
            master_scope,
            detail_scope,
        }
    }

    pub fn breakpoints(mut self, breakpoints: AdaptiveBreakpoints) -> Self {
        self.breakpoints = AdaptiveBreakpoints::new(breakpoints.medium, breakpoints.expanded);
        self
    }

    pub fn state(mut self, state: MasterDetailState) -> Self {
        self.route = state.route();
        self.state = state;
        self
    }

    pub fn split_state(mut self, state: SplitState) -> Self {
        self.split_snapshot = state.snapshot();
        self.split_state = state;
        self
    }

    pub fn navigation_state(&self) -> MasterDetailState {
        self.state.clone()
    }

    fn master_width(&self, available: f32) -> f32 {
        match self.split_snapshot.extent {
            SplitExtent::Fraction(fraction) => available * fraction,
            SplitExtent::Pixels(pixels) => pixels,
        }
        .clamp(0.0, available)
    }

    fn route_changed(&mut self, route: MasterDetailRoute) {
        if self.route == route {
            return;
        }
        self.route = route;
        if self.compact {
            match route {
                MasterDetailRoute::Master => self.master_scope.request_restore(),
                MasterDetailRoute::Detail => self.detail_scope.request_restore(),
            };
        }
    }
}

impl Widget for MasterDetail {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        self.route_changed(self.state.route());
        if self.compact
            && self.route == MasterDetailRoute::Detail
            && matches!(event, Event::Keyboard(key) if key.state == KeyState::Pressed && key.key == "Escape")
        {
            self.state.show_master();
            self.route_changed(MasterDetailRoute::Master);
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let route = ctx.observe(&self.state.route);
        self.route_changed(route);
        self.split_snapshot = ctx.observe(self.split_state.observable());
        let was_compact = self.compact;
        self.compact =
            self.breakpoints.classify(constraint_width(constraints)) == AdaptiveClass::Compact;
        if self.compact != was_compact {
            if self.compact {
                match self.route {
                    MasterDetailRoute::Master => self.master_scope.request_restore(),
                    MasterDetailRoute::Detail => self.detail_scope.request_restore(),
                };
            } else {
                self.detail_scope.request_restore();
            }
        }

        let size = constraints.clamp(Size::new(
            constraint_width(constraints),
            constraint_height(constraints),
        ));
        if self.compact {
            self.master_frame = local_rect(size.width, size.height);
            self.detail_frame = self.master_frame;
            match self.route {
                MasterDetailRoute::Master => {
                    self.master.measure(ctx, Constraints::tight(size));
                }
                MasterDetailRoute::Detail => {
                    self.detail.measure(ctx, Constraints::tight(size));
                }
            }
        } else {
            let master_width = self.master_width(size.width);
            self.master_frame = local_rect(master_width, size.height);
            self.detail_frame = Rect::new(
                master_width,
                0.0,
                (size.width - master_width).max(0.0),
                size.height,
            );
            self.master
                .measure(ctx, Constraints::tight(self.master_frame.size));
            self.detail
                .measure(ctx, Constraints::tight(self.detail_frame.size));
        }
        size
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if !self.compact || self.route == MasterDetailRoute::Master {
            self.master
                .arrange(ctx, self.master_frame.translate(bounds.origin.to_vector()));
        }
        if !self.compact || self.route == MasterDetailRoute::Detail {
            self.detail
                .arrange(ctx, self.detail_frame.translate(bounds.origin.to_vector()));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if !self.compact || self.route == MasterDetailRoute::Master {
            self.master.paint(ctx);
        }
        if !self.compact || self.route == MasterDetailRoute::Detail {
            self.detail.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if !self.compact || self.route == MasterDetailRoute::Master {
            self.master.semantics(ctx);
        }
        if !self.compact || self.route == MasterDetailRoute::Detail {
            self.detail.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if !self.compact || self.route == MasterDetailRoute::Master {
            self.master.visit_children(visitor);
        }
        if !self.compact || self.route == MasterDetailRoute::Detail {
            self.detail.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if !self.compact || self.route == MasterDetailRoute::Master {
            self.master.visit_children_mut(visitor);
        }
        if !self.compact || self.route == MasterDetailRoute::Detail {
            self.detail.visit_children_mut(visitor);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use sui_core::{Event, SemanticsNode, SemanticsRole, WindowEvent};
    use sui_runtime::{Application, Runtime, WindowBuilder};

    use super::*;

    struct NamedPane(&'static str);

    impl Widget for NamedPane {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(320.0, 180.0))
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let mut node = SemanticsNode::new(
                ctx.widget_id(),
                SemanticsRole::GenericContainer,
                ctx.bounds(),
            );
            node.name = Some(self.0.to_string());
            ctx.push(node);
        }
    }

    fn resize(runtime: &mut Runtime, width: f32) {
        let window_id = runtime.window_ids()[0];
        runtime
            .handle_event(
                window_id,
                Event::Window(WindowEvent::Resized(Size::new(width, 480.0))),
            )
            .unwrap();
    }

    fn render_named(runtime: &mut Runtime) -> Vec<(String, sui_core::WidgetId)> {
        let window_id = runtime.window_ids()[0];
        runtime
            .render(window_id)
            .unwrap()
            .semantics
            .into_iter()
            .filter_map(|node| node.name.map(|name| (name, node.id)))
            .collect()
    }

    #[test]
    fn adaptive_view_uses_local_width_and_retains_variant_identity() {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Adaptive")
                    .root(AdaptiveView::new(
                        NamedPane("compact"),
                        NamedPane("medium"),
                        NamedPane("expanded"),
                    )),
            )
            .build()
            .unwrap();

        resize(&mut runtime, 500.0);
        let compact = render_named(&mut runtime);
        assert_eq!(compact.len(), 1);
        assert_eq!(compact[0].0, "compact");
        let compact_id = compact[0].1;

        resize(&mut runtime, 800.0);
        assert_eq!(render_named(&mut runtime)[0].0, "medium");
        resize(&mut runtime, 1200.0);
        assert_eq!(render_named(&mut runtime)[0].0, "expanded");
        resize(&mut runtime, 500.0);
        assert_eq!(
            render_named(&mut runtime)[0],
            ("compact".to_string(), compact_id)
        );
    }

    #[test]
    fn constraint_view_matches_first_local_query_and_retains_identity() {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new().title("Constraint view").root(
                    ConstraintView::new(NamedPane("fallback"))
                        .when(
                            ConstraintQuery::new()
                                .min_width(700.0)
                                .orientation(ConstraintOrientation::Landscape),
                            NamedPane("wide"),
                        )
                        .when(
                            ConstraintQuery::new().max_width(699.0),
                            NamedPane("compact"),
                        ),
                ),
            )
            .build()
            .unwrap();

        resize(&mut runtime, 500.0);
        let compact = render_named(&mut runtime);
        assert_eq!(compact[0].0, "compact");
        let compact_id = compact[0].1;

        resize(&mut runtime, 900.0);
        assert_eq!(render_named(&mut runtime)[0].0, "wide");
        resize(&mut runtime, 500.0);
        assert_eq!(render_named(&mut runtime)[0].1, compact_id);
    }

    #[test]
    fn master_detail_changes_policy_without_recreating_panes() {
        let state = MasterDetailState::default();
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Master Detail").root(
                MasterDetail::new(NamedPane("master"), NamedPane("detail")).state(state.clone()),
            ))
            .build()
            .unwrap();

        resize(&mut runtime, 900.0);
        let expanded = render_named(&mut runtime);
        let detail_id = expanded
            .iter()
            .find(|(name, _)| name == "detail")
            .unwrap()
            .1;
        assert_eq!(expanded.len(), 2);

        resize(&mut runtime, 500.0);
        assert_eq!(render_named(&mut runtime)[0].0, "master");
        state.show_detail();
        let compact_detail = render_named(&mut runtime);
        assert_eq!(compact_detail, vec![("detail".to_string(), detail_id)]);
    }

    #[test]
    fn responsive_sidebar_tracks_overlay_inline_and_rail_modes() {
        let state = ResponsiveSidebarState::new();
        let modes = Rc::new(RefCell::new(Vec::new()));
        let recorded_modes = Rc::clone(&modes);
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new().title("Sidebar").root(
                    ResponsiveSidebar::new(NamedPane("sidebar"), NamedPane("content"))
                        .state(state.clone())
                        .on_mode_change(move |mode| recorded_modes.borrow_mut().push(mode)),
                ),
            )
            .build()
            .unwrap();

        resize(&mut runtime, 500.0);
        let names = render_named(&mut runtime);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].0, "content");

        state.open_overlay();
        assert_eq!(render_named(&mut runtime).len(), 2);
        assert_eq!(
            modes.borrow().last(),
            Some(&ResponsiveSidebarMode::OverlayOpen)
        );

        resize(&mut runtime, 800.0);
        let _ = render_named(&mut runtime);
        assert_eq!(modes.borrow().last(), Some(&ResponsiveSidebarMode::Inline));
        state.set_expanded(false);
        let _ = render_named(&mut runtime);
        assert_eq!(modes.borrow().last(), Some(&ResponsiveSidebarMode::Rail));
    }
}
