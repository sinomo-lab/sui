//! Content that animates in and out.
//!
//! [`Presence`] shows or hides one child. [`KeyedStack`] keeps a row or
//! column of children in step with a list of keyed items: new items animate
//! in, removed ones animate out while the gap closes, and items that change
//! places glide to their new positions.
//!
//! Leaving content stays on screen until its exit finishes, but it is inert
//! from the moment it starts to leave: it takes no pointer input or focus and
//! drops out of the accessibility tree. The runtime drives every transition;
//! enter and exit fades and scales move retained layers without repainting.

use std::{cell::Cell, collections::HashMap, hash::Hash, rc::Rc, sync::Arc};

use sui_animation::{AnimationSpec, Stagger};
use sui_core::{
    Event, InvalidationKind, InvalidationRequest, InvalidationTarget, Point, Rect, Size, Vector,
    WidgetId,
};
use sui_layout::{Alignment, Axis, Constraints};
use sui_reactive::{Observable, Signal};
use sui_runtime::{
    ArrangeCtx, EventCtx, FrameClock, LayerOptions, MeasureCtx, Motion, PaintBoundaryMode,
    PaintCtx, SemanticsCtx, SingleChild, Widget, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor,
    motion_policy,
};
use sui_scene::{LayerCompositionMode, LayerProperties};

use crate::DefaultTheme;
use crate::animation::ENTRANCE_SCALE;

/// Presence below this counts as gone.
const ABSENT: f32 = 1.0e-4;

/// How content looks at the absent end of an enter or exit, and how long each
/// takes.
///
/// Content fades in from `opacity`, grows from `scale`, and slides in from
/// `offset` as it appears, and does the reverse as it leaves. With `collapse`,
/// the space it takes grows and shrinks too, so neighbors move smoothly;
/// otherwise it takes its full space for as long as any of it shows.
///
/// Scaling, sliding, and collapsing are movement: under reduced motion content
/// only fades, and space opens at once on entry and closes after an exit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresenceTransition {
    pub opacity: f32,
    pub scale: f32,
    pub offset: Vector,
    pub collapse: bool,
    pub enter: AnimationSpec,
    pub exit: AnimationSpec,
}

impl PresenceTransition {
    /// Fade and grow from 96% with the theme's entrance and exit motion.
    pub fn new() -> Self {
        let motion = DefaultTheme::default().motion;
        Self {
            opacity: 0.0,
            scale: ENTRANCE_SCALE,
            offset: Vector::ZERO,
            collapse: false,
            enter: motion.entrance_spec(),
            exit: motion.exit_spec(),
        }
    }

    /// Only fade.
    pub fn fade() -> Self {
        Self::new().scale(1.0)
    }

    /// Appear and disappear at once.
    pub fn none() -> Self {
        Self {
            enter: AnimationSpec::INSTANT,
            exit: AnimationSpec::INSTANT,
            ..Self::fade()
        }
    }

    pub const fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub const fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Slide in from, and out to, `offset` from the resting position.
    pub const fn offset(mut self, offset: Vector) -> Self {
        self.offset = offset;
        self
    }

    pub const fn collapse(mut self, collapse: bool) -> Self {
        self.collapse = collapse;
        self
    }

    pub const fn enter(mut self, spec: AnimationSpec) -> Self {
        self.enter = spec;
        self
    }

    pub const fn exit(mut self, spec: AnimationSpec) -> Self {
        self.exit = spec;
        self
    }

    /// The layer presentation at `presence` (0 absent, 1 present). Without
    /// `movement`, only the opacity changes.
    pub fn layer_properties(&self, presence: f32, movement: bool) -> LayerProperties {
        let opacity = lerp(self.opacity, 1.0, presence).clamp(0.0, 1.0);
        let properties = LayerProperties::default().with_opacity(opacity);
        if !movement {
            return properties;
        }
        let absence = 1.0 - presence;
        properties
            .with_translation(Vector::new(
                self.offset.x * absence,
                self.offset.y * absence,
            ))
            .with_scale(lerp(self.scale, 1.0, presence).max(0.0))
    }

    /// How much of its space content takes at `presence`, heading to
    /// `target`: the presence itself while collapsing with movement,
    /// otherwise all of it while any of the content is there.
    fn space(&self, presence: f32, target: f32, movement: bool) -> f32 {
        if self.collapse && movement {
            presence.clamp(0.0, 1.0)
        } else if target > ABSENT || presence > ABSENT {
            1.0
        } else {
            0.0
        }
    }

    fn spec(&self, entering: bool) -> AnimationSpec {
        if entering { self.enter } else { self.exit }
    }

    /// Whether entering (or leaving) takes any time under the motion policy,
    /// so leaving content stays around for it.
    fn animates(&self, entering: bool) -> bool {
        let spec = self.spec(entering);
        !spec.with_policy(motion_policy()).is_instant()
    }
}

impl Default for PresenceTransition {
    fn default() -> Self {
        Self::new()
    }
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn main_extent(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn cross_extent(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn from_main_cross(main: f32, cross: f32, axis: Axis) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

/// Where content is in its enter or exit.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PresenceMotion {
    presence: Motion<f32>,
    /// Whether the running transition may scale, slide, and collapse; fixed
    /// when it starts, from the motion policy.
    movement: bool,
}

impl PresenceMotion {
    fn new(present: bool) -> Self {
        Self {
            presence: Motion::new(if present { 1.0 } else { 0.0 })
                .invalidating(InvalidationKind::Effect),
            movement: true,
        }
    }

    fn target(&self) -> f32 {
        self.presence.target()
    }

    fn is_present(&self) -> bool {
        self.target() > 0.5
    }

    fn space(&self, transition: &PresenceTransition, time: f64) -> f32 {
        transition.space(self.presence.at(time), self.target(), self.movement)
    }

    fn is_gone(&self, time: f64) -> bool {
        !self.is_present() && !self.presence.is_animating(&time) && self.presence.at(time) <= ABSENT
    }

    /// Start toward present or absent, `delay` seconds from `time`. Returns
    /// when the transition ends, if one runs.
    fn start(
        &mut self,
        present: bool,
        time: f64,
        delay: f64,
        transition: &PresenceTransition,
    ) -> Option<f64> {
        self.movement = motion_policy().allows_movement();
        let target = if present { 1.0 } else { 0.0 };
        self.presence
            .start_after(target, time, delay, transition.spec(present))
    }
}

/// Keep a layer showing a presence transition updated until `until`: its
/// opacity always, and its scale and offset when the transition moves.
fn track_presence(ctx: &mut MeasureCtx, layer: WidgetId, until: f64, motion: &PresenceMotion) {
    ctx.track_motion_for(layer, until, InvalidationKind::Effect);
    if motion.movement {
        ctx.track_motion_for(layer, until, InvalidationKind::Transform);
    }
}

/// Keep the widget laying out content in a presence transition measured as
/// the content's space changes: every frame while it collapses, otherwise
/// once when the transition ends.
fn track_space(
    ctx: &mut MeasureCtx,
    until: f64,
    motion: &PresenceMotion,
    transition: &PresenceTransition,
) {
    if transition.collapse && motion.movement {
        ctx.track_motion(until, InvalidationKind::Measure);
    } else {
        ctx.track_motion_end(until, InvalidationKind::Measure);
    }
}

/// Shows or hides one child, animating it in and out.
///
/// A hidden child takes no space and stays retained, keeping its state for
/// when it shows again. Drive visibility with [`Presence::shown_from`]; with
/// [`Presence::appear`] the child also animates in when first laid out.
pub struct Presence {
    child: SingleChild,
    shown_source: Option<Arc<dyn Observable<bool>>>,
    shown_reader: Option<Box<dyn Fn() -> bool>>,
    transition: PresenceTransition,
    collapse_axis: Axis,
    delay: f64,
    appear: bool,
    initialized: bool,
    motion: PresenceMotion,
}

impl Presence {
    pub fn new<W>(child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            child: SingleChild::new(child),
            shown_source: None,
            shown_reader: None,
            transition: PresenceTransition::new(),
            collapse_axis: Axis::Vertical,
            delay: 0.0,
            appear: false,
            initialized: false,
            motion: PresenceMotion::new(true),
        }
    }

    /// Start shown or hidden, without animating.
    pub fn shown(mut self, shown: bool) -> Self {
        self.motion = PresenceMotion::new(shown);
        self.child.child_mut().set_inert(!shown);
        self
    }

    /// Follow `source`: animate in when it becomes true and out when it
    /// becomes false.
    pub fn shown_from<O>(mut self, source: O) -> Self
    where
        O: Observable<bool> + 'static,
    {
        let shown = source.get();
        self.shown_source = Some(Arc::new(source));
        self.shown(shown)
    }

    /// Follow `shown`, read when the window redraws and on layout: animate in
    /// when it returns true and out when it returns false. Prefer
    /// [`Presence::shown_from`], which updates without polling.
    pub fn shown_when<F>(mut self, shown: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        let initial = shown();
        self.shown_reader = Some(Box::new(shown));
        self.shown(initial)
    }

    /// Animate in when first laid out, if shown then.
    pub fn appear(mut self) -> Self {
        self.appear = true;
        self
    }

    pub fn transition(mut self, transition: PresenceTransition) -> Self {
        self.transition = transition;
        self
    }

    /// Grow and shrink the space the child takes along `axis` as it enters
    /// and leaves.
    pub fn collapse(mut self, axis: Axis) -> Self {
        self.transition.collapse = true;
        self.collapse_axis = axis;
        self
    }

    /// Wait `delay` seconds before entering or leaving.
    pub fn delay(mut self, delay: f64) -> Self {
        self.delay = delay.max(0.0);
        self
    }

    pub fn is_shown(&self) -> bool {
        self.motion.is_present()
    }

    fn set_shown(&mut self, shown: bool, ctx: &mut MeasureCtx) {
        if shown == self.motion.is_present() {
            return;
        }
        self.child.child_mut().set_inert(!shown);
        let time = ctx.frame_time();
        let own = ctx.widget_id();
        if let Some(until) = self.motion.start(shown, time, self.delay, &self.transition) {
            track_presence(ctx, own, until, &self.motion);
            track_space(ctx, until, &self.motion, &self.transition);
        }
        // An instant change shows right away.
        for kind in [
            InvalidationKind::Paint,
            InvalidationKind::Effect,
            InvalidationKind::Transform,
        ] {
            ctx.request(InvalidationRequest::new(
                InvalidationTarget::Widget(own),
                kind,
            ));
        }
    }
}

impl Widget for Presence {
    fn event(&mut self, ctx: &mut EventCtx, _event: &Event) {
        if self
            .shown_reader
            .as_ref()
            .is_some_and(|shown| shown() != self.motion.is_present())
        {
            ctx.request_measure();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if let Some(source) = &self.shown_source {
            let shown = ctx.observe(source.as_ref());
            self.set_shown(shown, ctx);
        }
        if let Some(shown) = self.shown_reader.as_ref().map(|shown| shown()) {
            self.set_shown(shown, ctx);
        }
        if !self.initialized {
            self.initialized = true;
            if self.appear && self.motion.is_present() {
                self.motion = PresenceMotion::new(false);
                self.set_shown(true, ctx);
            }
        }

        let time = ctx.frame_time();
        let space = self.motion.space(&self.transition, time);
        if space <= 0.0 {
            return constraints.clamp(Size::ZERO);
        }
        let size = self.child.measure(ctx, constraints);
        if space >= 1.0 {
            return size;
        }
        let collapsed = match self.collapse_axis {
            Axis::Horizontal => Size::new(size.width * space, size.height),
            Axis::Vertical => Size::new(size.width, size.height * space),
        };
        constraints.clamp(collapsed)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        // The child keeps its full size while the space it takes collapses.
        let size = self.child.child().measured_size();
        self.child
            .arrange(ctx, Rect::from_origin_size(bounds.origin, size));
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if self.motion.presence.get(ctx) > ABSENT || self.motion.is_present() {
            self.child.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.child.semantics(ctx);
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties_at(&self, frame_time: f64) -> LayerProperties {
        self.transition
            .layer_properties(self.motion.presence.at(frame_time), self.motion.movement)
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

/// What a [`KeyedStack`] item's layer shows: its presence and how far it is
/// from where it was arranged while it moves.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ItemMotion {
    presence: PresenceMotion,
    offset: Motion<Vector>,
}

/// The layer around each [`KeyedStack`] item. The stack owns the motion; the
/// layer presents it.
struct ItemLayer {
    child: SingleChild,
    motion: Rc<Cell<ItemMotion>>,
    transition: Rc<Cell<PresenceTransition>>,
}

impl Widget for ItemLayer {
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

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties_at(&self, frame_time: f64) -> LayerProperties {
        let motion = self.motion.get();
        let properties = self.transition.get().layer_properties(
            motion.presence.presence.at(frame_time),
            motion.presence.movement,
        );
        properties.with_translation(properties.translation + motion.offset.at(frame_time))
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

struct StackEntry<K, T> {
    key: K,
    value: Signal<T>,
    layer: WidgetPod,
    motion: Rc<Cell<ItemMotion>>,
    /// Where the item was on screen before the latest reorder, until it is
    /// arranged in its new place.
    moved_from: Option<Point>,
}

impl<K, T> StackEntry<K, T> {
    fn motion(&self) -> ItemMotion {
        self.motion.get()
    }

    fn update(&self, change: impl FnOnce(&mut ItemMotion)) {
        let mut motion = self.motion.get();
        change(&mut motion);
        self.motion.set(motion);
    }

    fn is_leaving(&self) -> bool {
        !self.motion().presence.is_present()
    }
}

type BuildItem<K, T> = Box<dyn Fn(&K, Signal<T>) -> Box<dyn Widget>>;

/// A row or column of children kept in step with a list of keyed items.
///
/// Each item's widget is built once and kept while its key stays in the list,
/// updating through the [`Signal`] it was built with. New items animate in,
/// one after another with a [`Stagger`]; removed items animate out while the
/// gap they leave closes; items that change places glide to their new
/// positions. An item added back while it is still leaving turns around and
/// comes back.
pub struct KeyedStack<K, T> {
    axis: Axis,
    spacing: f32,
    alignment: Alignment,
    items: Arc<dyn Observable<Vec<T>>>,
    key_for: Box<dyn Fn(&T) -> K>,
    build: BuildItem<K, T>,
    transition: Rc<Cell<PresenceTransition>>,
    stagger: Stagger,
    move_spec: Option<AnimationSpec>,
    appear: bool,
    entries: Vec<StackEntry<K, T>>,
    last_items: Option<Vec<T>>,
    arranged: bool,
}

impl<K, T> KeyedStack<K, T>
where
    K: Clone + Eq + Hash + 'static,
    T: Clone + PartialEq + 'static,
{
    /// A stack along `axis` of the items in `items`, identified by
    /// `key_for` and built by `build`.
    pub fn new<O, KF, B, W>(axis: Axis, items: O, key_for: KF, build: B) -> Self
    where
        O: Observable<Vec<T>> + 'static,
        KF: Fn(&T) -> K + 'static,
        B: Fn(&K, Signal<T>) -> W + 'static,
        W: Widget + 'static,
    {
        let motion = DefaultTheme::default().motion;
        Self {
            axis,
            spacing: 0.0,
            alignment: Alignment::Stretch,
            items: Arc::new(items),
            key_for: Box::new(key_for),
            build: Box::new(move |key, value| Box::new(build(key, value))),
            transition: Rc::new(Cell::new(PresenceTransition::new().collapse(true))),
            stagger: motion.stagger(),
            move_spec: Some(motion.layout_spec()),
            appear: false,
            entries: Vec::new(),
            last_items: None,
            arranged: false,
        }
    }

    pub fn vertical<O, KF, B, W>(items: O, key_for: KF, build: B) -> Self
    where
        O: Observable<Vec<T>> + 'static,
        KF: Fn(&T) -> K + 'static,
        B: Fn(&K, Signal<T>) -> W + 'static,
        W: Widget + 'static,
    {
        Self::new(Axis::Vertical, items, key_for, build)
    }

    pub fn horizontal<O, KF, B, W>(items: O, key_for: KF, build: B) -> Self
    where
        O: Observable<Vec<T>> + 'static,
        KF: Fn(&T) -> K + 'static,
        B: Fn(&K, Signal<T>) -> W + 'static,
        W: Widget + 'static,
    {
        Self::new(Axis::Horizontal, items, key_for, build)
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    /// How items line up across the stack. `Stretch`, the default, makes
    /// them all as wide (or tall) as the stack.
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// How items enter and leave. The stack collapses the space of leaving
    /// items unless the transition turns `collapse` off.
    pub fn transition(self, transition: PresenceTransition) -> Self {
        self.transition.set(transition);
        self
    }

    /// Delays for items that arrive together; [`Stagger::NONE`] brings them
    /// in at once.
    pub fn stagger(mut self, stagger: Stagger) -> Self {
        self.stagger = stagger;
        self
    }

    /// How items glide to new places, or `None` to move them at once.
    pub fn move_spec(mut self, spec: Option<AnimationSpec>) -> Self {
        self.move_spec = spec;
        self
    }

    /// Animate the first items in too, instead of showing them at once.
    pub fn appear(mut self) -> Self {
        self.appear = true;
        self
    }

    /// The items shown, and those still leaving, in order.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn new_entry(&self, key: K, item: T) -> StackEntry<K, T> {
        let value = Signal::named("Keyed stack item", item);
        let motion = Rc::new(Cell::new(ItemMotion {
            presence: PresenceMotion::new(false),
            offset: Motion::new(Vector::ZERO)
                .invalidating(InvalidationKind::Transform)
                .movement(),
        }));
        let child = WidgetPod::new_boxed((self.build)(&key, value.clone()));
        let layer = WidgetPod::new(ItemLayer {
            child: SingleChild::from_pod(child),
            motion: Rc::clone(&motion),
            transition: Rc::clone(&self.transition),
        });
        StackEntry {
            key,
            value,
            layer,
            motion,
            moved_from: None,
        }
    }

    fn reconcile(&mut self, items: Vec<T>, ctx: &mut MeasureCtx) {
        let time = ctx.frame_time();
        let transition = self.transition.get();
        let first = self.last_items.is_none();

        let previous_keys = self
            .entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<Vec<_>>();
        let mut previous = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| (entry.key.clone(), entry))
            .collect::<HashMap<_, _>>();

        // Listed items, in their new order.
        let mut order = Vec::with_capacity(items.len());
        let mut next = HashMap::with_capacity(items.len());
        let mut arrivals = Vec::new();
        for item in items {
            let key = (self.key_for)(&item);
            if next.contains_key(&key) {
                continue;
            }
            let entry = match previous.remove(&key) {
                Some(mut entry) => {
                    entry.value.set(item);
                    if entry.is_leaving() {
                        // Back while still leaving: turn around.
                        arrivals.push(key.clone());
                    } else if self.arranged && self.move_spec.is_some() {
                        let offset = entry.motion().offset.at(time);
                        entry.moved_from = Some(entry.layer.bounds().origin + offset);
                    }
                    entry
                }
                None => {
                    let entry = self.new_entry(key.clone(), item);
                    if first && !self.appear {
                        entry.update(|motion| motion.presence = PresenceMotion::new(true));
                    } else {
                        arrivals.push(key.clone());
                    }
                    entry
                }
            };
            order.push(key.clone());
            next.insert(key, entry);
        }

        // Unlisted items leave from where they were: after the item that
        // preceded them.
        for (index, key) in previous_keys.iter().enumerate() {
            let Some(mut entry) = previous.remove(key) else {
                continue;
            };
            if !entry.is_leaving() {
                if !transition.animates(false) {
                    continue;
                }
                entry.layer.set_inert(true);
                entry.moved_from = None;
                let layer = entry.layer.id();
                let mut motion = entry.motion();
                if let Some(until) = motion.presence.start(false, time, 0.0, &transition) {
                    track_presence(ctx, layer, until, &motion.presence);
                    track_space(ctx, until, &motion.presence, &transition);
                }
                entry.motion.set(motion);
            }
            let position = previous_keys[..index]
                .iter()
                .rev()
                .find_map(|earlier| order.iter().position(|placed| placed == earlier))
                .map_or(0, |position| position + 1);
            order.insert(position, key.clone());
            next.insert(key.clone(), entry);
        }

        let arriving = arrivals.len();
        for (index, key) in arrivals.iter().enumerate() {
            let Some(entry) = next.get_mut(key) else {
                continue;
            };
            entry.layer.set_inert(false);
            let delay = self.stagger.delay(index, arriving);
            let layer = entry.layer.id();
            let mut motion = entry.motion();
            if let Some(until) = motion.presence.start(true, time, delay, &transition) {
                track_presence(ctx, layer, until, &motion.presence);
                track_space(ctx, until, &motion.presence, &transition);
            }
            entry.motion.set(motion);
        }

        self.entries = order
            .into_iter()
            .filter_map(|key| next.remove(&key))
            .collect();
        ctx.request_paint();
        ctx.request_semantics();
    }

    fn child_constraints(&self, constraints: Constraints) -> Constraints {
        let cross = cross_extent(constraints.max, self.axis);
        let min_cross = if self.alignment == Alignment::Stretch && cross.is_finite() {
            cross
        } else {
            0.0
        };
        Constraints::new(
            from_main_cross(0.0, min_cross, self.axis),
            from_main_cross(f32::INFINITY, cross, self.axis),
        )
    }
}

impl<K, T> Widget for KeyedStack<K, T>
where
    K: Clone + Eq + Hash + 'static,
    T: Clone + PartialEq + 'static,
{
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let items = ctx.observe(self.items.as_ref());
        if self.last_items.as_ref() != Some(&items) {
            self.reconcile(items.clone(), ctx);
            self.last_items = Some(items);
        }

        let time = ctx.frame_time();
        let before = self.entries.len();
        self.entries
            .retain(|entry| !(entry.is_leaving() && entry.motion().presence.is_gone(time)));
        if self.entries.len() != before {
            ctx.request_paint();
        }

        let transition = self.transition.get();
        let child_constraints = self.child_constraints(constraints);
        let mut main = 0.0_f32;
        let mut cross = 0.0_f32;
        let mut widest_space = 0.0_f32;
        for entry in &mut self.entries {
            let size = entry.layer.measure(ctx, child_constraints);
            let space = entry.motion().presence.space(&transition, time);
            main += (main_extent(size, self.axis) + self.spacing) * space;
            cross = cross.max(cross_extent(size, self.axis) * space.min(1.0));
            widest_space = widest_space.max(space);
        }
        main = (main - self.spacing * widest_space).max(0.0);
        constraints.clamp(from_main_cross(main, cross, self.axis))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let time = ctx.frame_time();
        let transition = self.transition.get();
        let available_cross = cross_extent(bounds.size, self.axis);
        let mut position = 0.0_f32;
        for entry in &mut self.entries {
            let size = entry.layer.measured_size();
            let child_cross = cross_extent(size, self.axis);
            let cross_offset = match self.alignment {
                Alignment::Start | Alignment::Stretch => 0.0,
                Alignment::Center => (available_cross - child_cross) * 0.5,
                Alignment::End => available_cross - child_cross,
            };
            let offset = from_main_cross(position, cross_offset, self.axis);
            let origin = bounds.origin + Vector::new(offset.width, offset.height);

            if let Some(from) = entry.moved_from.take()
                && let Some(spec) = self.move_spec
            {
                let delta = from - origin;
                if delta.x.abs() > 0.5 || delta.y.abs() > 0.5 {
                    // Present the item where it was, then glide it home.
                    let layer = entry.layer.id();
                    let mut motion = entry.motion();
                    motion.offset.jump_to(delta);
                    match motion.offset.start(Vector::ZERO, time, spec) {
                        Some(until) => {
                            ctx.track_motion_for(layer, until, InvalidationKind::Transform)
                        }
                        None => motion.offset.jump_to(Vector::ZERO),
                    }
                    entry.motion.set(motion);
                }
            }

            entry
                .layer
                .arrange(ctx, Rect::from_origin_size(origin, size));
            let space = entry.motion().presence.space(&transition, time);
            position += (main_extent(size, self.axis) + self.spacing) * space;
        }
        self.arranged = true;
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for entry in &self.entries {
            entry.layer.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        for entry in &self.entries {
            entry.layer.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for entry in &self.entries {
            visitor.visit(&entry.layer);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for entry in &mut self.entries {
            visitor.visit(&mut entry.layer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_core::{Color, Event, WindowEvent, WindowId};
    use sui_runtime::{
        Application, MotionPreference, RenderOutput, Runtime, WidgetGraphSnapshot, WindowBuilder,
        reset_motion_settings, set_app_motion_preference,
    };

    use crate::Flex;

    /// A block `height` tall that paints white.
    struct Block(f32);

    impl Widget for Block {
        fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(constraints.min.width.max(40.0), self.0))
        }

        fn paint(&self, ctx: &mut PaintCtx) {
            ctx.fill_bounds(Color::WHITE);
        }

        fn accepts_focus(&self) -> bool {
            true
        }
    }

    fn app(root: impl Widget + 'static) -> (Runtime, WindowId) {
        reset_motion_settings();
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(root))
            .build()
            .unwrap();
        let window = runtime.window_ids()[0];
        runtime
            .handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(200.0, 400.0))),
            )
            .unwrap();
        runtime.render(window).unwrap();
        (runtime, window)
    }

    fn frame(runtime: &mut Runtime, window: WindowId, time: f64) -> RenderOutput {
        runtime.tick(time);
        for (ready_window, event) in runtime.drain_ready_events() {
            runtime.handle_event(ready_window, event).unwrap();
        }
        runtime.render(window).unwrap()
    }

    fn graph(runtime: &Runtime, window: WindowId) -> WidgetGraphSnapshot {
        runtime.widget_graph(window).unwrap()
    }

    fn nodes_named<'a>(
        graph: &'a WidgetGraphSnapshot,
        name: &str,
    ) -> Vec<&'a sui_runtime::WidgetNodeSnapshot> {
        graph
            .nodes
            .iter()
            .filter(|node| node.widget_name.ends_with(name))
            .collect()
    }

    fn layer_properties(output: &RenderOutput, widget: WidgetId) -> Option<LayerProperties> {
        let mut found = None;
        output.frame.scene.visit_layers(&mut |layer| {
            if layer.widget_id() == widget {
                found = Some(layer.descriptor.properties);
            }
        });
        found
    }

    fn items_stack(items: &Signal<Vec<u32>>) -> KeyedStack<u32, u32> {
        KeyedStack::vertical(items.clone(), |item| *item, |_, _| Block(20.0))
            .spacing(10.0)
            .stagger(Stagger::NONE)
    }

    fn item_tops(runtime: &Runtime, window: WindowId) -> Vec<f32> {
        nodes_named(&graph(runtime, window), "ItemLayer")
            .iter()
            .map(|node| node.bounds.y())
            .collect()
    }

    #[test]
    fn presence_fades_out_then_takes_no_space() {
        let shown = Signal::new(true);
        let root = Flex::vertical()
            .with_child(Presence::new(Block(30.0)).shown_from(shown.clone()))
            .with_child(Block(20.0));
        let (mut runtime, window) = app(root);
        let exit = DefaultTheme::default().motion.exit_spec().duration();

        shown.set(false);
        let start = frame(&mut runtime, window, 1.0);
        let graph_now = graph(&runtime, window);
        let presence = nodes_named(&graph_now, "Presence")[0];
        let blocks = nodes_named(&graph_now, "Block");
        assert!(blocks[0].inert && !blocks[0].accepts_focus && !blocks[0].hit_test);
        assert!(!blocks[1].inert);
        assert_eq!(
            layer_properties(&start, presence.id).map(|properties| properties.opacity),
            Some(1.0)
        );

        let midway = frame(&mut runtime, window, 1.0 + exit * 0.5);
        let opacity = layer_properties(&midway, presence.id)
            .expect("the presence layer")
            .opacity;
        assert!(opacity > 0.0 && opacity < 1.0, "fading: {opacity}");
        // Without collapse, the child keeps its space until it is gone.
        assert_eq!(
            nodes_named(&graph(&runtime, window), "Block")[1].bounds.y(),
            30.0
        );

        frame(&mut runtime, window, 1.0 + exit + 0.01);
        assert_eq!(
            nodes_named(&graph(&runtime, window), "Block")[1].bounds.y(),
            0.0
        );
        assert!(
            runtime
                .semantics(window)
                .unwrap()
                .iter()
                .all(|node| node.id != blocks[0].id)
        );

        shown.set(true);
        frame(&mut runtime, window, 2.0);
        let graph_now = graph(&runtime, window);
        assert!(!nodes_named(&graph_now, "Block")[0].inert);
        assert_eq!(nodes_named(&graph_now, "Block")[1].bounds.y(), 30.0);
    }

    #[test]
    fn collapsing_presence_closes_its_space_smoothly() {
        let shown = Signal::new(true);
        let root = Flex::vertical()
            .with_child(
                Presence::new(Block(40.0))
                    .shown_from(shown.clone())
                    .collapse(Axis::Vertical),
            )
            .with_child(Block(20.0));
        let (mut runtime, window) = app(root);
        let exit = DefaultTheme::default().motion.exit_spec().duration();

        shown.set(false);
        frame(&mut runtime, window, 1.0);
        frame(&mut runtime, window, 1.0 + exit * 0.5);
        let below = nodes_named(&graph(&runtime, window), "Block")[1].bounds.y();
        assert!(below > 0.0 && below < 40.0, "the gap is closing: {below}");
        frame(&mut runtime, window, 1.0 + exit + 0.01);
        assert_eq!(
            nodes_named(&graph(&runtime, window), "Block")[1].bounds.y(),
            0.0
        );
        assert_eq!(runtime.next_wakeup_time(window).unwrap(), None);
    }

    #[test]
    fn reduced_motion_fades_without_collapsing_or_scaling() {
        let shown = Signal::new(true);
        let root = Flex::vertical()
            .with_child(
                Presence::new(Block(40.0))
                    .shown_from(shown.clone())
                    .collapse(Axis::Vertical),
            )
            .with_child(Block(20.0));
        let (mut runtime, window) = app(root);
        set_app_motion_preference(Some(MotionPreference::Reduced));
        let exit = DefaultTheme::default().motion.exit_spec().duration();

        shown.set(false);
        frame(&mut runtime, window, 1.0);
        let midway = frame(&mut runtime, window, 1.0 + exit * 0.5);
        let presence = nodes_named(&graph(&runtime, window), "Presence")[0].id;
        let properties = layer_properties(&midway, presence).expect("the presence layer");
        assert!(properties.opacity < 1.0);
        assert_eq!(properties.scale, LayerProperties::default().scale);
        assert_eq!(
            nodes_named(&graph(&runtime, window), "Block")[1].bounds.y(),
            40.0
        );
        frame(&mut runtime, window, 1.0 + exit + 0.01);
        assert_eq!(
            nodes_named(&graph(&runtime, window), "Block")[1].bounds.y(),
            0.0
        );
        reset_motion_settings();
    }

    #[test]
    fn presence_can_animate_in_when_it_first_appears() {
        let root = Flex::vertical().with_child(Presence::new(Block(30.0)).appear());
        let (mut runtime, window) = app(root);
        let enter = DefaultTheme::default().motion.entrance_spec().duration();
        let presence = nodes_named(&graph(&runtime, window), "Presence")[0].id;
        let start = frame(&mut runtime, window, 0.0);
        let opacity = layer_properties(&start, presence).expect("layer").opacity;
        assert!(opacity < 0.5, "starts faded out: {opacity}");
        let end = frame(&mut runtime, window, enter + 0.01);
        assert_eq!(
            layer_properties(&end, presence).expect("layer").opacity,
            1.0
        );
    }

    #[test]
    fn keyed_stack_shows_its_first_items_without_animating() {
        let items = Signal::new(vec![1, 2, 3]);
        let (runtime, window) = app(items_stack(&items));
        assert_eq!(item_tops(&runtime, window), vec![0.0, 30.0, 60.0]);
        assert_eq!(runtime.next_wakeup_time(window).unwrap(), None);
    }

    #[test]
    fn removed_items_leave_while_the_gap_closes() {
        let items = Signal::new(vec![1, 2, 3]);
        let (mut runtime, window) = app(items_stack(&items));
        let exit = DefaultTheme::default().motion.exit_spec().duration();
        let blocks_before = nodes_named(&graph(&runtime, window), "Block")
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();

        items.set(vec![1, 3]);
        frame(&mut runtime, window, 1.0);
        let graph_now = graph(&runtime, window);
        let layers = nodes_named(&graph_now, "ItemLayer");
        assert_eq!(layers.len(), 3, "the removed item stays while it leaves");
        assert!(layers[1].inert && !layers[0].inert && !layers[2].inert);
        assert_eq!(item_tops(&runtime, window), vec![0.0, 30.0, 60.0]);

        frame(&mut runtime, window, 1.0 + exit * 0.5);
        let tops = item_tops(&runtime, window);
        assert!(
            tops[2] < 60.0 && tops[2] > 30.0,
            "the next item moves up: {tops:?}"
        );

        frame(&mut runtime, window, 1.0 + exit + 0.01);
        assert_eq!(item_tops(&runtime, window), vec![0.0, 30.0]);
        let remaining = nodes_named(&graph(&runtime, window), "Block")
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec![blocks_before[0], blocks_before[2]]);
    }

    #[test]
    fn inserted_items_grow_in_one_after_another() {
        let items = Signal::new(vec![1]);
        let root = items_stack(&items).stagger(Stagger::new(0.1));
        let (mut runtime, window) = app(root);
        let enter = DefaultTheme::default().motion.entrance_spec().duration();

        items.set(vec![1, 2, 3]);
        frame(&mut runtime, window, 1.0);
        let layers = nodes_named(&graph(&runtime, window), "ItemLayer")
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(item_tops(&runtime, window)[1], 30.0);

        let early = frame(&mut runtime, window, 1.0 + enter * 0.5);
        let second = layer_properties(&early, layers[1]).expect("layer").opacity;
        let third = layer_properties(&early, layers[2]).expect("layer").opacity;
        assert!(second > 0.0, "the first arrival is fading in: {second}");
        assert!(third < second, "the next one follows: {third} < {second}");

        frame(&mut runtime, window, 1.0 + 0.1 + enter + 0.01);
        assert_eq!(item_tops(&runtime, window), vec![0.0, 30.0, 60.0]);
        assert_eq!(runtime.next_wakeup_time(window).unwrap(), None);
    }

    #[test]
    fn reordered_items_glide_to_their_new_places() {
        let items = Signal::new(vec![1, 2, 3]);
        let (mut runtime, window) = app(items_stack(&items));
        let layers = nodes_named(&graph(&runtime, window), "ItemLayer")
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        let glide = DefaultTheme::default().motion.layout_spec().duration();

        items.set(vec![3, 2, 1]);
        let start = frame(&mut runtime, window, 1.0);
        // Layout settles at once; the layers start where the items were.
        let order = nodes_named(&graph(&runtime, window), "ItemLayer")
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(order, vec![layers[2], layers[1], layers[0]]);
        let first = layer_properties(&start, layers[2]).expect("layer");
        assert_eq!(first.translation.y, 60.0);
        assert_eq!(
            layer_properties(&start, layers[1])
                .expect("layer")
                .translation
                .y,
            0.0
        );

        let midway = frame(&mut runtime, window, 1.0 + glide * 0.5);
        let y = layer_properties(&midway, layers[2])
            .expect("layer")
            .translation
            .y;
        assert!(y > 0.0 && y < 60.0, "gliding up: {y}");
        let end = frame(&mut runtime, window, 1.0 + glide + 0.01);
        assert_eq!(
            layer_properties(&end, layers[2])
                .expect("layer")
                .translation
                .y,
            0.0
        );
    }

    #[test]
    fn an_item_added_back_while_leaving_turns_around() {
        let items = Signal::new(vec![1, 2]);
        let (mut runtime, window) = app(items_stack(&items));
        let exit = DefaultTheme::default().motion.exit_spec().duration();
        let enter = DefaultTheme::default().motion.entrance_spec().duration();
        let layer = nodes_named(&graph(&runtime, window), "ItemLayer")[1].id;

        items.set(vec![1]);
        frame(&mut runtime, window, 1.0);
        frame(&mut runtime, window, 1.0 + exit * 0.5);
        items.set(vec![1, 2]);
        frame(&mut runtime, window, 1.0 + exit * 0.5);
        let graph_now = graph(&runtime, window);
        let layers = nodes_named(&graph_now, "ItemLayer");
        assert_eq!(layers[1].id, layer, "the same widget comes back");
        assert!(!layers[1].inert);

        let end = frame(&mut runtime, window, 1.0 + exit + enter + 0.01);
        assert_eq!(layer_properties(&end, layer).expect("layer").opacity, 1.0);
        assert_eq!(item_tops(&runtime, window), vec![0.0, 30.0]);
    }
}
