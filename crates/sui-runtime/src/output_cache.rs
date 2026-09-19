//! Window-owned, byte-charged reuse of provisional paint and semantics output.
//! Unaccounted scene payloads and callbacks with side effects use the full path.
use lru::LruCache;
use std::{cell::RefCell, collections::HashMap, mem::size_of, rc::Rc, sync::Arc};
use sui_core::{
    DpiInfo, Rect, SemanticsAction, SemanticsNode, SemanticsValue, Transform, WidgetId,
};
use sui_scene::{Brush, ImageRegistry, Scene, SceneCommand};
use sui_text::{FontRegistry, TextLayoutRegistry};

const MAX_FRAGMENT_BYTES: usize = 32 * 1024;

pub(crate) type SharedOutputCache = Rc<RefCell<OutputCache>>;

/// A cloned drawing context cannot read or repopulate a later frame's cache.
#[derive(Clone, Debug)]
pub(crate) struct OutputScope {
    cache: SharedOutputCache,
    generation: u64,
}

impl OutputScope {
    fn current<T>(&self, f: impl FnOnce(&mut OutputCache) -> T) -> Option<T> {
        let mut cache = self.cache.borrow_mut();
        (cache.active && cache.generation == self.generation).then(|| f(&mut cache))
    }
    pub fn paint(
        &self,
        id: WidgetId,
        key: GeometryKey,
        registry: &TextLayoutRegistry,
    ) -> Option<Rc<PaintFragment>> {
        self.current(|cache| cache.paint(id, key, registry))
            .flatten()
    }
    pub fn put_paint(
        &self,
        id: WidgetId,
        key: GeometryKey,
        scene: &Scene,
        bounds: &HashMap<WidgetId, Rect>,
        ime: Option<Rect>,
    ) {
        self.current(|cache| cache.put_paint(id, key, scene, bounds, ime));
    }
    pub fn semantics(&self, id: WidgetId, key: GeometryKey) -> Option<Rc<[SemanticsNode]>> {
        self.current(|cache| cache.semantics(id, key)).flatten()
    }
    pub fn put_semantics(&self, id: WidgetId, key: GeometryKey, nodes: &[SemanticsNode]) {
        self.current(|cache| cache.put_semantics(id, key, nodes));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GeometryKey {
    pub bounds: Rect,
    pub transform: Transform,
}

#[derive(Clone, Debug)]
pub(crate) struct PaintFragment {
    pub scene: Scene,
    pub bounds: HashMap<WidgetId, Rect>,
    pub ime: Option<Rect>,
}
impl PaintFragment {
    pub fn charge(scene: &Scene, bounds: &HashMap<WidgetId, Rect>) -> Option<usize> {
        let mut bytes = 512 + 2 * std::mem::size_of_val(scene.commands()) + bounds.capacity() * 64;
        for command in scene.commands() {
            let brush = match command {
                SceneCommand::FillRect { brush, .. }
                | SceneCommand::StrokeRect { brush, .. }
                | SceneCommand::FillRoundedRect { brush, .. } => Some(brush),
                SceneCommand::FillPath { path, brush }
                | SceneCommand::StrokePath { path, brush, .. } => {
                    bytes += 2 * std::mem::size_of_val(path.elements());
                    Some(brush)
                }
                SceneCommand::PushClipPath { path } => {
                    bytes += 2 * std::mem::size_of_val(path.elements());
                    None
                }
                SceneCommand::Clear(_)
                | SceneCommand::DrawShapedText(_)
                | SceneCommand::DrawShapedTextWindow(_)
                | SceneCommand::DrawImage { .. }
                | SceneCommand::DrawImageQuad { .. }
                | SceneCommand::PushClip { .. }
                | SceneCommand::PopClip
                | SceneCommand::PushTransform { .. }
                | SceneCommand::PopTransform
                | SceneCommand::PushTextRenderPolicy { .. }
                | SceneCommand::PopTextRenderPolicy => None,
                // Unknown heap payloads, nested scene layers and raw text retain
                // the normal callback path. Check before cloning any payload.
                _ => return None,
            };
            if let Some(Brush::LinearGradient { stops, .. }) = brush {
                bytes += 2 * std::mem::size_of_val(stops.as_slice());
            }
        }
        Some(bytes)
    }
    pub fn text_is_current(&self, registry: &TextLayoutRegistry) -> bool {
        self.scene.commands().iter().all(|c| match c {
            SceneCommand::DrawShapedText(t) => t.resolve(registry).is_some(),
            SceneCommand::DrawShapedTextWindow(t) => t.resolve(registry).is_some(),
            _ => true,
        })
    }
}

pub(crate) fn semantics_charge(nodes: &[SemanticsNode]) -> usize {
    let mut bytes = 512 + std::mem::size_of_val(nodes);
    for n in nodes {
        bytes += n.name.as_ref().map_or(0, String::capacity)
            + n.description.as_ref().map_or(0, String::capacity);
        if let Some(SemanticsValue::Text(s)) = &n.value {
            bytes += s.capacity();
        }
        bytes += n.actions.capacity() * size_of::<SemanticsAction>();
        for a in &n.actions {
            if let SemanticsAction::Custom(s) = a {
                bytes += s.capacity();
            }
        }
        bytes += (n.relations.controls.capacity()
            + n.relations.labelled_by.capacity()
            + n.relations.described_by.capacity()
            + n.relations.owns.capacity())
            * size_of::<WidgetId>();
    }
    // Conservative allocation/bucket overhead and Vec growth allowance.
    bytes.saturating_mul(2)
}

#[derive(Debug)]
struct Entries<T> {
    lru: LruCache<WidgetId, (GeometryKey, T, usize)>,
    bytes: usize,
    budget: usize,
}
impl<T: Clone> Entries<T> {
    fn new(budget: usize) -> Self {
        Self {
            lru: LruCache::unbounded(),
            bytes: 0,
            budget,
        }
    }
    fn remove(&mut self, id: WidgetId) {
        if let Some((_, _, bytes)) = self.lru.pop(&id) {
            self.bytes -= bytes;
        }
    }
    fn get(&mut self, id: WidgetId, key: GeometryKey) -> Option<T> {
        if self.lru.peek(&id).is_some_and(|(old, _, _)| *old != key) {
            self.remove(id);
        }
        self.lru.get(&id).map(|(_, value, _)| value.clone())
    }
    fn put(&mut self, id: WidgetId, key: GeometryKey, value: T, bytes: usize) {
        self.remove(id);
        if bytes > self.budget.min(MAX_FRAGMENT_BYTES) {
            return;
        }
        while self.bytes + bytes > self.budget {
            let Some((_, (_, _, old))) = self.lru.pop_lru() else {
                break;
            };
            self.bytes -= old;
        }
        self.bytes += bytes;
        self.lru.put(id, (key, value, bytes));
    }
    fn retain(&mut self, keep: &impl Fn(WidgetId) -> bool) {
        let removed: Vec<_> = self
            .lru
            .iter()
            .filter_map(|(id, _)| (!keep(*id)).then_some(*id))
            .collect();
        for id in removed {
            self.remove(id);
        }
    }
    fn clear(&mut self) {
        self.lru.clear();
        self.bytes = 0;
    }
}

#[derive(Debug)]
struct Context {
    dpi: DpiInfo,
    options: Option<crate::WindowRenderOptions>,
    focused: Option<WidgetId>,
    fonts: Arc<FontRegistry>,
    images: Arc<ImageRegistry>,
}
#[derive(Debug)]
pub(crate) struct OutputCache {
    pub active: bool,
    paint_active: bool,
    semantics_active: bool,
    generation: u64,
    paints: Entries<Rc<PaintFragment>>,
    semantics: Entries<Rc<[SemanticsNode]>>,
    context: Option<Context>,
}
impl Default for OutputCache {
    fn default() -> Self {
        Self {
            active: true,
            paint_active: true,
            semantics_active: true,
            generation: 0,
            paints: Entries::new(8 * 1024 * 1024),
            semantics: Entries::new(4 * 1024 * 1024),
            context: None,
        }
    }
}
impl OutputCache {
    pub fn next_frame(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active = true;
        self.paint_active = true;
        self.semantics_active = true;
    }
    pub fn scope(cache: &SharedOutputCache) -> Option<OutputScope> {
        let current = cache.borrow();
        current.active.then(|| OutputScope {
            cache: cache.clone(),
            generation: current.generation,
        })
    }
    pub fn close(&mut self) {
        self.next_frame();
        self.active = false;
        self.clear();
        self.context = None;
    }

    pub fn shared() -> SharedOutputCache {
        Rc::new(RefCell::new(Self::default()))
    }
    pub fn context(
        &mut self,
        dpi: DpiInfo,
        focused: Option<WidgetId>,
        fonts: Arc<FontRegistry>,
        images: Arc<ImageRegistry>,
        options: Option<crate::WindowRenderOptions>,
    ) {
        if self.context.as_ref().is_none_or(|old| {
            old.options != options
                || old.dpi != dpi
                || old.focused != focused
                || !Arc::ptr_eq(&old.fonts, &fonts)
                || !Arc::ptr_eq(&old.images, &images)
        }) {
            self.clear();
            self.context = Some(Context {
                dpi,
                options,
                focused,
                fonts,
                images,
            });
        }
    }
    pub fn clear(&mut self) {
        self.paints.clear();
        self.semantics.clear();
    }
    pub fn disable_paint(&mut self) {
        self.paint_active = false;
        self.active = self.semantics_active;
        self.paints.clear();
    }
    pub fn disable_semantics(&mut self) {
        self.semantics_active = false;
        self.active = self.paint_active;
        self.semantics.clear();
    }
    pub fn retain(
        &mut self,
        paint: impl Fn(WidgetId) -> bool,
        semantics: impl Fn(WidgetId) -> bool,
    ) {
        self.paints.retain(&paint);
        self.semantics.retain(&semantics);
    }
    pub fn paint(
        &mut self,
        id: WidgetId,
        key: GeometryKey,
        registry: &TextLayoutRegistry,
    ) -> Option<Rc<PaintFragment>> {
        if !self.paint_active {
            return None;
        }
        let fragment = self.paints.get(id, key)?;
        if fragment.text_is_current(registry) {
            Some(fragment)
        } else {
            self.paints.remove(id);
            None
        }
    }
    pub fn put_paint(
        &mut self,
        id: WidgetId,
        key: GeometryKey,
        scene: &Scene,
        bounds: &HashMap<WidgetId, Rect>,
        ime: Option<Rect>,
    ) {
        if self.paint_active
            && let Some(bytes) = PaintFragment::charge(scene, bounds)
            && bytes <= self.paints.budget.min(MAX_FRAGMENT_BYTES)
        {
            let fragment = PaintFragment {
                scene: scene.clone(),
                bounds: bounds.clone(),
                ime,
            };
            self.paints.put(id, key, Rc::new(fragment), bytes);
        }
    }
    pub fn semantics(&mut self, id: WidgetId, key: GeometryKey) -> Option<Rc<[SemanticsNode]>> {
        self.semantics_active
            .then(|| self.semantics.get(id, key))
            .flatten()
    }
    pub fn put_semantics(&mut self, id: WidgetId, key: GeometryKey, nodes: &[SemanticsNode]) {
        if !self.semantics_active {
            return;
        }
        let bytes = semantics_charge(nodes);
        if bytes <= self.semantics.budget.min(MAX_FRAGMENT_BYTES) {
            self.semantics.put(id, key, Rc::from(nodes), bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_contexts_cannot_repopulate_and_close_releases_retained_data() {
        let shared = OutputCache::shared();
        let old = OutputCache::scope(&shared).unwrap();
        let id = WidgetId::new(41);
        let key = GeometryKey {
            bounds: Rect::ZERO,
            transform: Transform::IDENTITY,
        };
        old.put_semantics(id, key, &[]);
        shared.borrow_mut().next_frame();
        assert!(old.semantics(id, key).is_none());
        shared.borrow_mut().clear();
        old.put_semantics(id, key, &[]);
        assert_eq!(shared.borrow().semantics.bytes, 0);
        let current = OutputCache::scope(&shared).unwrap();
        current.put_semantics(id, key, &[]);
        assert!(shared.borrow().semantics.bytes > 0);
        shared.borrow_mut().close();
        current.put_semantics(id, key, &[]);
        assert_eq!(shared.borrow().semantics.bytes, 0);
    }

    #[test]
    fn byte_budget_evicts_and_geometry_changes_discard_old_results() {
        let mut cache = Entries::new(100);
        let key = GeometryKey {
            bounds: Rect::ZERO,
            transform: Transform::IDENTITY,
        };
        cache.put(WidgetId::new(1), key, 1, 40);
        cache.put(WidgetId::new(2), key, 2, 40);
        cache.get(WidgetId::new(1), key);
        cache.put(WidgetId::new(3), key, 3, 40);
        assert_eq!(cache.get(WidgetId::new(2), key), None);
        assert_eq!(cache.bytes, 80);
        let changed = GeometryKey {
            bounds: Rect::new(0.0, 0.0, 5.0, 5.0),
            ..key
        };
        assert_eq!(cache.get(WidgetId::new(1), changed), None);
        assert_eq!(cache.get(WidgetId::new(1), key), None);
        cache.put(WidgetId::new(4), key, 4, 101);
        assert_eq!(cache.bytes, 40);
        cache.retain(&|_| false);
        assert_eq!(cache.bytes, 0);
    }
    #[test]
    fn incremental_output_preserves_dependencies_and_opaque_subtrees() {
        use crate::{
            Application, ArrangeCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WidgetPod,
            WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
        };
        use std::cell::Cell;
        use sui_core::{Color, Event, SemanticsRole, Size, WindowEvent};
        use sui_layout::Constraints;
        use sui_reactive::Signal;
        struct Leaf {
            value: Signal<usize>,
            paints: Rc<Cell<usize>>,
            semantics: Rc<Cell<usize>>,
            reusable: bool,
        }
        impl Widget for Leaf {
            fn supports_output_reuse(&self) -> bool {
                self.reusable
            }
            fn measure(&mut self, _: &mut MeasureCtx, c: Constraints) -> Size {
                c.clamp(Size::new(50.0, 20.0))
            }
            fn paint(&self, c: &mut PaintCtx) {
                self.paints.set(self.paints.get() + 1);
                let v = c.observe(&self.value);
                c.fill_bounds(if v.is_multiple_of(2) {
                    Color::WHITE
                } else {
                    Color::BLACK
                });
            }
            fn semantics(&self, c: &mut SemanticsCtx) {
                self.semantics.set(self.semantics.get() + 1);
                let v = c.observe(&self.value);
                let mut n = SemanticsNode::new(c.widget_id(), SemanticsRole::Text, c.bounds());
                n.name = Some(v.to_string());
                c.push(n);
            }
        }
        struct Group(Vec<WidgetPod>);
        impl Widget for Group {
            fn supports_output_reuse(&self) -> bool {
                true
            }
            fn measure(&mut self, c: &mut MeasureCtx, k: Constraints) -> Size {
                for p in &mut self.0 {
                    p.measure(c, k);
                }
                k.clamp(Size::new(200.0, 20.0))
            }
            fn arrange(&mut self, c: &mut ArrangeCtx, r: Rect) {
                for (i, p) in self.0.iter_mut().enumerate() {
                    p.arrange(c, Rect::new(r.x() + i as f32 * 50.0, r.y(), 50.0, 20.0));
                }
            }
            fn paint(&self, c: &mut PaintCtx) {
                for p in &self.0 {
                    p.paint(c);
                }
            }
            fn semantics(&self, c: &mut SemanticsCtx) {
                for p in &self.0 {
                    p.semantics(c);
                }
            }
            fn visit_children(&self, v: &mut dyn WidgetPodVisitor) {
                for p in &self.0 {
                    v.visit(p);
                }
            }
            fn visit_children_mut(&mut self, v: &mut dyn WidgetPodMutVisitor) {
                for p in &mut self.0 {
                    v.visit(p);
                }
            }
        }
        let values: Vec<_> = (0..3).map(|_| Signal::new(0usize)).collect();
        let paints: Vec<_> = (0..3).map(|_| Rc::new(Cell::new(0))).collect();
        let semantics: Vec<_> = (0..3).map(|_| Rc::new(Cell::new(0))).collect();
        let children = values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                WidgetPod::new(Group(vec![WidgetPod::new(Leaf {
                    value: value.clone(),
                    paints: paints[i].clone(),
                    semantics: semantics[i].clone(),
                    reusable: i < 2,
                })]))
            })
            .collect();
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(Group(children)))
            .build()
            .unwrap();
        let window = runtime.window_ids()[0];
        runtime.render(window).unwrap();
        values[0].set(1);
        let out = runtime.render(window).unwrap();
        assert!(out.semantics.iter().any(|n| n.name.as_deref() == Some("1")));
        assert_eq!(
            paints.iter().map(|c| c.get()).collect::<Vec<_>>(),
            [2, 1, 2]
        );
        assert_eq!(
            semantics.iter().map(|c| c.get()).collect::<Vec<_>>(),
            [2, 1, 2]
        );
        // A cached sibling's subscriptions must still trigger its next update.
        values[1].set(2);
        let out = runtime.render(window).unwrap();
        assert!(out.semantics.iter().any(|n| n.name.as_deref() == Some("2")));
        assert_eq!(
            paints.iter().map(|c| c.get()).collect::<Vec<_>>(),
            [2, 2, 3]
        );
        // Explicit viewport invalidation forces all callbacks, including cache hits.
        runtime
            .handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(250.0, 40.0))),
            )
            .unwrap();
        runtime.render(window).unwrap();
        assert_eq!(
            paints.iter().map(|c| c.get()).collect::<Vec<_>>(),
            [3, 3, 4]
        );
        assert_eq!(
            semantics.iter().map(|c| c.get()).collect::<Vec<_>>(),
            [3, 3, 4]
        );
    }

    #[test]
    fn context_changes_and_missing_text_handles_cannot_reuse_output() {
        use sui_core::{Color, Point};
        use sui_text::{
            ShapedText, TextDocument, TextLayoutHandle, TextLayoutRequest, TextStyle, TextSystem,
        };
        let mut cache = OutputCache::default();
        let fonts = Arc::new(FontRegistry::new());
        let images = Arc::new(ImageRegistry::new());
        let key = GeometryKey {
            bounds: Rect::ZERO,
            transform: Transform::IDENTITY,
        };
        let id = WidgetId::new(99);
        cache.context(
            DpiInfo::default(),
            None,
            fonts.clone(),
            images.clone(),
            Some(crate::WindowRenderOptions::new(false, 1.0)),
        );
        let system = TextSystem::new();
        let layout = system
            .layout_document(
                TextLayoutRequest::new(TextDocument::from_plain_text(
                    "Cache",
                    TextStyle::new(Color::WHITE),
                )),
                &fonts,
            )
            .unwrap();
        let mut scene = Scene::new();
        scene.push(SceneCommand::DrawShapedText(ShapedText::from_layout(
            Point::ZERO,
            TextLayoutHandle::new(99),
            &layout,
        )));
        cache.put_paint(id, key, &scene, &HashMap::new(), None);
        assert!(
            cache
                .paint(id, key, &TextLayoutRegistry::default())
                .is_none()
        );
        cache.put_semantics(
            id,
            key,
            &[SemanticsNode::new(
                id,
                sui_core::SemanticsRole::Text,
                Rect::ZERO,
            )],
        );
        assert!(cache.semantics(id, key).is_some());
        cache.context(
            DpiInfo::default(),
            Some(id),
            fonts.clone(),
            images.clone(),
            Some(crate::WindowRenderOptions::new(false, 1.0)),
        );
        assert!(cache.semantics(id, key).is_none());
        cache.put_semantics(id, key, &[]);
        cache.context(
            DpiInfo::default(),
            Some(id),
            Arc::new(FontRegistry::new()),
            images,
            Some(crate::WindowRenderOptions::new(false, 1.0)),
        );
        assert_eq!(cache.semantics.bytes, 0);
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use crate::{
        Application, ArrangeCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WidgetPod,
        WidgetPodMutVisitor, WidgetPodVisitor, WindowBuilder,
    };
    use std::cell::Cell;
    use sui_core::{
        Color, InvalidationKind, InvalidationRequest, InvalidationTarget, SemanticsRole, Size,
    };
    use sui_layout::Constraints;
    use sui_reactive::Signal;

    #[derive(Default)]
    struct Calls {
        paint: Cell<usize>,
        semantics: Cell<usize>,
    }
    struct Leaf {
        value: Signal<usize>,
        calls: Rc<Calls>,
    }
    impl Widget for Leaf {
        fn supports_output_reuse(&self) -> bool {
            true
        }
        fn measure(&mut self, _: &mut MeasureCtx, c: Constraints) -> Size {
            c.clamp(Size::new(30.0, 20.0))
        }
        fn paint(&self, c: &mut PaintCtx) {
            self.calls.paint.set(self.calls.paint.get() + 1);
            let value = c.observe(&self.value);
            c.fill_bounds(if value == 0 {
                Color::BLACK
            } else {
                Color::WHITE
            });
        }
        fn semantics(&self, c: &mut SemanticsCtx) {
            self.calls.semantics.set(self.calls.semantics.get() + 1);
            let mut node = SemanticsNode::new(c.widget_id(), SemanticsRole::Text, c.bounds());
            node.name = Some(c.observe(&self.value).to_string());
            c.push(node);
        }
    }
    struct Parent {
        color: Signal<Color>,
        caption: Signal<usize>,
        calls: Rc<Calls>,
        children: Vec<WidgetPod>,
        observe_in_measure: bool,
    }
    impl Widget for Parent {
        fn supports_output_reuse(&self) -> bool {
            true
        }
        fn measure(&mut self, c: &mut MeasureCtx, constraints: Constraints) -> Size {
            if self.observe_in_measure {
                c.observe_with(&self.color, InvalidationKind::Paint);
            }
            for child in &mut self.children {
                child.measure(c, Constraints::tight(Size::new(30.0, 20.0)));
            }
            constraints.clamp(Size::new(200.0, 40.0))
        }
        fn arrange(&mut self, c: &mut ArrangeCtx, bounds: Rect) {
            for (i, child) in self.children.iter_mut().enumerate() {
                child.arrange(
                    c,
                    Rect::new(bounds.x() + i as f32 * 35.0, bounds.y(), 30.0, 20.0),
                );
            }
        }
        fn paint(&self, c: &mut PaintCtx) {
            self.calls.paint.set(self.calls.paint.get() + 1);
            let color = c.observe(&self.color);
            c.fill_bounds(color);
            for child in &self.children {
                child.paint(c);
            }
        }
        fn semantics(&self, c: &mut SemanticsCtx) {
            self.calls.semantics.set(self.calls.semantics.get() + 1);
            let mut node =
                SemanticsNode::new(c.widget_id(), SemanticsRole::GenericContainer, c.bounds());
            node.name = Some(format!("parent {}", c.observe(&self.caption)));
            c.push(node);
            for child in &self.children {
                child.semantics(c);
            }
        }
        fn visit_children(&self, v: &mut dyn WidgetPodVisitor) {
            for child in &self.children {
                v.visit(child);
            }
        }
        fn visit_children_mut(&mut self, v: &mut dyn WidgetPodMutVisitor) {
            for child in &mut self.children {
                v.visit(child);
            }
        }
    }

    #[test]
    fn parent_output_changes_preserve_children_and_independent_phases() {
        let color = Signal::new(Color::BLACK);
        let caption = Signal::new(0usize);
        let value = Signal::new(0usize);
        let calls = Rc::new(Calls::default());
        let child_calls: Vec<_> = (0..5).map(|_| Rc::new(Calls::default())).collect();
        let children = child_calls
            .iter()
            .map(|calls| {
                WidgetPod::new(Leaf {
                    value: value.clone(),
                    calls: calls.clone(),
                })
            })
            .collect();
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(Parent {
                color: color.clone(),
                caption: caption.clone(),
                calls: calls.clone(),
                children,
                observe_in_measure: false,
            }))
            .build()
            .unwrap();
        let window = runtime.window_ids()[0];
        runtime.render(window).unwrap();
        color.set(Color::WHITE);
        runtime.render(window).unwrap();
        assert_eq!((calls.paint.get(), calls.semantics.get()), (2, 1));
        assert!(
            child_calls
                .iter()
                .all(|calls| calls.paint.get() == 1 && calls.semantics.get() == 1)
        );
        caption.set(1);
        let partial = runtime.render(window).unwrap();
        assert!(
            partial
                .semantics
                .iter()
                .any(|n| n.name.as_deref() == Some("parent 1"))
        );
        assert_eq!((calls.paint.get(), calls.semantics.get()), (2, 2));
        assert!(
            child_calls
                .iter()
                .all(|calls| calls.paint.get() == 1 && calls.semantics.get() == 1)
        );
        // Retained descendants keep their own subscriptions alive.
        value.set(1);
        let updated = runtime.render(window).unwrap();
        assert!(
            child_calls
                .iter()
                .all(|calls| calls.paint.get() == 2 && calls.semantics.get() == 2)
        );
        let state = runtime.window_mut(window).unwrap();
        for kind in [InvalidationKind::Paint, InvalidationKind::Semantics] {
            state.pending_invalidations.push(InvalidationRequest::new(
                InvalidationTarget::Widget(state.root.id()),
                kind,
            ));
            state.schedule.mark(kind);
        }
        let full = runtime.render(window).unwrap();
        assert_eq!(updated.frame.scene, full.frame.scene);
        assert_eq!(updated.semantics, full.semantics);
        assert!(
            child_calls
                .iter()
                .all(|calls| calls.paint.get() == 3 && calls.semantics.get() == 3)
        );
        // Older returned frames remain immutable after replacement/reuse.
        assert!(
            partial
                .semantics
                .iter()
                .any(|n| n.name.as_deref() == Some("0"))
        );
    }

    #[test]
    fn explicit_requests_and_cross_phase_observers_keep_subtree_behavior() {
        for observe_in_measure in [false, true] {
            let color = Signal::new(Color::BLACK);
            let child_calls = Rc::new(Calls::default());
            let children = (0..5)
                .map(|_| {
                    WidgetPod::new(Leaf {
                        value: Signal::new(0),
                        calls: child_calls.clone(),
                    })
                })
                .collect();
            let mut runtime = Application::new()
                .window(WindowBuilder::new().root(Parent {
                    color: color.clone(),
                    caption: Signal::new(0),
                    calls: Rc::new(Calls::default()),
                    children,
                    observe_in_measure,
                }))
                .build()
                .unwrap();
            let window = runtime.window_ids()[0];
            runtime.render(window).unwrap();
            color.set(Color::WHITE);
            if !observe_in_measure {
                let state = runtime.window_mut(window).unwrap();
                state.pending_invalidations.push(InvalidationRequest::new(
                    InvalidationTarget::Widget(state.root.id()),
                    InvalidationKind::Paint,
                ));
                state.schedule.mark(InvalidationKind::Paint);
            }
            runtime.render(window).unwrap();
            assert_eq!(child_calls.paint.get(), 10);
            assert_eq!(child_calls.semantics.get(), 5);
        }
    }
}
