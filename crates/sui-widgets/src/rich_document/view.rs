use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use sui_core::{
    Event, ImageHandle, KeyState, Point, PointerButton, PointerEventKind, Rect, ScrollDelta,
    SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsRole, SemanticsValue, Size,
    Vector, WidgetId,
};
use sui_layout::Constraints;
use sui_reactive::Signal;
use sui_runtime::{
    ArrangeCtx, EventCtx, EventPhase, MeasureCtx, PaintCtx, SemanticsCtx, Widget, WidgetPod,
    WidgetPodMutVisitor, WidgetPodVisitor,
};
use sui_scene::StrokeStyle;
use sui_text::{
    FontStyle, FontWeight, PersistentTextLayout, TextDocument, TextLayoutRequest, TextParagraph,
    TextSpan, TextStyle,
};

use super::{
    BasicSyntaxHighlighter, RichBlockId, RichDocumentBlock, RichDocumentBlockKind,
    RichDocumentModel, RichDocumentSpan, RichDocumentStatus, RichInlineImage, RichInlineKind,
    RichSyntaxHighlighter, RichSyntaxSpan, RichSyntaxTokenKind,
};
use crate::{DefaultTheme, TextCommand};

const BLOCK_GAP: f32 = 8.0;
const TEXT_INSET: f32 = 2.0;
const QUOTE_INSET: f32 = 14.0;
const CODE_INSET: f32 = 10.0;
const CODE_HEADER_HEIGHT: f32 = 30.0;
const STRUCTURED_INSET: f32 = 10.0;
const SYNTHETIC_RICH_DOCUMENT_TAG: u64 = 0x2d << 46;
static NEXT_SYNTHETIC_ID: AtomicU64 = AtomicU64::new(1);

type Renderer = Rc<dyn Fn(RichDocumentRenderContext) -> WidgetPod>;
type StringAction = Rc<RefCell<Box<dyn FnMut(&str)>>>;
type BlockAction = Rc<RefCell<Box<dyn FnMut(RichBlockId)>>>;
type ImageResolver = Rc<dyn Fn(&RichInlineImage) -> Option<ImageHandle>>;

/// Context passed to an application-defined rich-document block renderer.
///
/// The `block` signal preserves the renderer widget's identity while the
/// structured payload changes. Observe it with the relevant widget context
/// (`ctx.observe` or `ctx.observe_with`) to declare invalidation dependencies.
#[derive(Clone)]
pub struct RichDocumentRenderContext {
    pub block: Signal<RichDocumentBlock>,
    pub state: RichDocumentViewState,
    pub theme: DefaultTheme,
}

/// Registry of block renderers keyed by architecture-neutral names.
///
/// Markdown blocks use built-in renderers. Applications typically register
/// extension keys such as `extension:tool-call`, `extension:diff`, or
/// `extension:operation-log` without coupling SUI to a chat schema.
#[derive(Clone, Default)]
pub struct RichDocumentRendererRegistry {
    renderers: Rc<RefCell<HashMap<String, Renderer>>>,
}

impl RichDocumentRendererRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F, W>(&self, key: impl Into<String>, renderer: F)
    where
        F: Fn(RichDocumentRenderContext) -> W + 'static,
        W: Widget + 'static,
    {
        self.renderers.borrow_mut().insert(
            key.into(),
            Rc::new(move |context| WidgetPod::new(renderer(context))),
        );
    }

    pub fn unregister(&self, key: &str) -> bool {
        self.renderers.borrow_mut().remove(key).is_some()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.renderers.borrow().contains_key(key)
    }

    fn renderer(&self, key: &str) -> Option<Renderer> {
        self.renderers.borrow().get(key).cloned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DocumentPoint {
    block: RichBlockId,
    offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PressedInline {
    Link(String),
    Image(String),
}

#[derive(Clone, Debug, Default)]
struct DocumentSelection {
    anchor: Option<DocumentPoint>,
    focus: Option<DocumentPoint>,
    order: Vec<RichBlockId>,
    selected_text: String,
}

#[derive(Default)]
struct ViewStateInner {
    selection: DocumentSelection,
    expanded: HashMap<RichBlockId, bool>,
}

/// Shareable retained state for a [`RichDocumentView`].
///
/// The state owns document-spanning selection and expansion state separately
/// from Markdown content, so structural streaming updates do not reset either.
#[derive(Clone)]
pub struct RichDocumentViewState {
    inner: Rc<RefCell<ViewStateInner>>,
    revision: Signal<u64>,
}

impl RichDocumentViewState {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ViewStateInner::default())),
            revision: Signal::named("RichDocumentViewState", 0),
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let text = self.inner.borrow().selection.selected_text.clone();
        (!text.is_empty()).then_some(text)
    }

    pub fn clear_selection(&self) -> bool {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            let changed = inner.selection.anchor.is_some()
                || inner.selection.focus.is_some()
                || !inner.selection.selected_text.is_empty();
            inner.selection.anchor = None;
            inner.selection.focus = None;
            inner.selection.selected_text.clear();
            changed
        };
        if changed {
            self.bump_revision();
        }
        changed
    }

    pub fn is_expanded(&self, block: RichBlockId) -> bool {
        self.inner
            .borrow()
            .expanded
            .get(&block)
            .copied()
            .unwrap_or(false)
    }

    pub fn set_expanded(&self, block: RichBlockId, expanded: bool) -> bool {
        let changed = self.inner.borrow_mut().expanded.insert(block, expanded) != Some(expanded);
        if changed {
            self.bump_revision();
        }
        changed
    }

    pub fn toggle_expanded(&self, block: RichBlockId) -> bool {
        let expanded = !self.is_expanded(block);
        self.set_expanded(block, expanded);
        expanded
    }

    fn bump_revision(&self) {
        let next = self.revision.get().wrapping_add(1);
        let _ = self.revision.set(next);
    }

    fn set_order(&self, order: Vec<RichBlockId>) {
        let mut inner = self.inner.borrow_mut();
        inner.selection.order = order;
        let live = inner
            .selection
            .order
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        inner.expanded.retain(|id, _| live.contains(id));
        if inner
            .selection
            .anchor
            .is_some_and(|point| !live.contains(&point.block))
            || inner
                .selection
                .focus
                .is_some_and(|point| !live.contains(&point.block))
        {
            inner.selection.anchor = None;
            inner.selection.focus = None;
            inner.selection.selected_text.clear();
        }
    }

    fn selection(&self) -> DocumentSelection {
        self.inner.borrow().selection.clone()
    }

    fn set_selection(&self, anchor: DocumentPoint, focus: DocumentPoint, text: String) {
        let mut inner = self.inner.borrow_mut();
        if inner.selection.anchor == Some(anchor)
            && inner.selection.focus == Some(focus)
            && inner.selection.selected_text == text
        {
            return;
        }
        inner.selection.anchor = Some(anchor);
        inner.selection.focus = Some(focus);
        inner.selection.selected_text = text;
        drop(inner);
        self.bump_revision();
    }
}

impl Default for RichDocumentViewState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct BlockTextHandle {
    block: RichBlockId,
    bounds: Rect,
    origin: Point,
    layout: Option<PersistentTextLayout>,
    text: String,
}

impl BlockTextHandle {
    fn new(block: RichBlockId) -> Self {
        Self {
            block,
            bounds: Rect::ZERO,
            origin: Point::ZERO,
            layout: None,
            text: String::new(),
        }
    }

    fn point(&self, point: Point, strict: bool) -> Option<DocumentPoint> {
        if strict && !self.bounds.contains(point) {
            return None;
        }
        let layout = self.layout.as_ref()?;
        let cursor =
            layout.hit_test_point(Point::new(point.x - self.origin.x, point.y - self.origin.y));
        Some(DocumentPoint {
            block: self.block,
            offset: cursor.utf8_offset.min(self.text.len()),
        })
    }
}

struct RetainedBlock {
    key: String,
    value: Signal<RichDocumentBlock>,
    pod: WidgetPod,
    text: Rc<RefCell<BlockTextHandle>>,
}

/// Retained, selectable, incrementally updating rich-document widget.
pub struct RichDocumentView {
    model: RichDocumentModel,
    state: RichDocumentViewState,
    registry: RichDocumentRendererRegistry,
    theme: DefaultTheme,
    highlighter: Rc<dyn RichSyntaxHighlighter>,
    blocks: HashMap<RichBlockId, RetainedBlock>,
    order: Vec<RichBlockId>,
    measured_heights: HashMap<RichBlockId, f32>,
    dragging: bool,
    drag_anchor: Option<DocumentPoint>,
    pressed_inline: Option<(RichBlockId, PressedInline)>,
    on_link: Option<StringAction>,
    on_image: Option<StringAction>,
    image_resolver: Option<ImageResolver>,
    on_attachment: Option<BlockAction>,
}

impl RichDocumentView {
    pub fn new(model: RichDocumentModel) -> Self {
        Self {
            model,
            state: RichDocumentViewState::new(),
            registry: RichDocumentRendererRegistry::new(),
            theme: DefaultTheme::default(),
            highlighter: Rc::new(BasicSyntaxHighlighter),
            blocks: HashMap::new(),
            order: Vec::new(),
            measured_heights: HashMap::new(),
            dragging: false,
            drag_anchor: None,
            pressed_inline: None,
            on_link: None,
            on_image: None,
            image_resolver: None,
            on_attachment: None,
        }
    }

    pub fn state(mut self, state: RichDocumentViewState) -> Self {
        self.state = state;
        self
    }

    pub fn renderer_registry(mut self, registry: RichDocumentRendererRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn syntax_highlighter<H>(mut self, highlighter: H) -> Self
    where
        H: RichSyntaxHighlighter + 'static,
    {
        self.highlighter = Rc::new(highlighter);
        self
    }

    pub fn on_link<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_link = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    pub fn on_image<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_image = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    /// Resolve Markdown image sources to images registered with the runtime.
    ///
    /// Until a source resolves, the default renderer keeps the image's alt
    /// text as an inline placeholder with full image semantics.
    pub fn image_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&RichInlineImage) -> Option<ImageHandle> + 'static,
    {
        self.image_resolver = Some(Rc::new(resolver));
        self
    }

    pub fn on_attachment<F>(mut self, callback: F) -> Self
    where
        F: FnMut(RichBlockId) + 'static,
    {
        self.on_attachment = Some(Rc::new(RefCell::new(Box::new(callback))));
        self
    }

    pub fn model(&self) -> &RichDocumentModel {
        &self.model
    }

    pub fn view_state(&self) -> &RichDocumentViewState {
        &self.state
    }

    fn sync_blocks(&mut self) {
        let blocks = self.model.blocks();
        let order = blocks.iter().map(|block| block.id).collect::<Vec<_>>();
        let live = order.iter().copied().collect::<HashSet<_>>();
        self.blocks.retain(|id, _| live.contains(id));
        self.measured_heights.retain(|id, _| live.contains(id));

        for block in blocks {
            let key = block.renderer_key();
            if let Some(retained) = self.blocks.get_mut(&block.id)
                && retained.key == key
            {
                continue;
            }
            let signal = self
                .model
                .block_signal(block.id)
                .unwrap_or_else(|| Signal::named("DetachedRichDocumentBlock", block.clone()));
            let text = Rc::new(RefCell::new(BlockTextHandle::new(block.id)));
            let context = RichDocumentRenderContext {
                block: signal.clone(),
                state: self.state.clone(),
                theme: self.theme,
            };
            let pod = self.registry.renderer(&key).map_or_else(
                || {
                    WidgetPod::new(DefaultBlockView::new(
                        signal.clone(),
                        self.state.clone(),
                        Rc::clone(&text),
                        self.theme,
                        Rc::clone(&self.highlighter),
                        self.on_link.clone(),
                        self.on_image.clone(),
                        self.image_resolver.clone(),
                        self.on_attachment.clone(),
                    ))
                },
                |renderer| renderer(context),
            );
            self.blocks.insert(
                block.id,
                RetainedBlock {
                    key,
                    value: signal,
                    pod,
                    text,
                },
            );
        }
        self.order = order;
        self.state.set_order(self.order.clone());
    }

    fn point_at(&self, point: Point, strict: bool) -> Option<DocumentPoint> {
        if strict {
            return self.order.iter().find_map(|id| {
                self.blocks
                    .get(id)
                    .and_then(|block| block.text.borrow().point(point, true))
            });
        }
        let mut nearest: Option<(f32, Rc<RefCell<BlockTextHandle>>)> = None;
        for id in &self.order {
            let Some(block) = self.blocks.get(id) else {
                continue;
            };
            let handle = block.text.borrow();
            if handle.layout.is_none() {
                continue;
            }
            if handle.bounds.contains(point) {
                return handle.point(point, false);
            }
            let distance = if point.y < handle.bounds.y() {
                handle.bounds.y() - point.y
            } else {
                point.y - handle.bounds.max_y()
            };
            if nearest.as_ref().is_none_or(|(best, _)| distance < *best) {
                nearest = Some((distance, Rc::clone(&block.text)));
            }
        }
        nearest.and_then(|(_, handle)| handle.borrow().point(point, false))
    }

    fn selection_text(&self, anchor: DocumentPoint, focus: DocumentPoint) -> String {
        let Some(anchor_index) = self.order.iter().position(|id| *id == anchor.block) else {
            return String::new();
        };
        let Some(focus_index) = self.order.iter().position(|id| *id == focus.block) else {
            return String::new();
        };
        let (start, end) = if (anchor_index, anchor.offset) <= (focus_index, focus.offset) {
            (anchor, focus)
        } else {
            (focus, anchor)
        };
        let start_index = self
            .order
            .iter()
            .position(|id| *id == start.block)
            .unwrap_or(0);
        let end_index = self
            .order
            .iter()
            .position(|id| *id == end.block)
            .unwrap_or(start_index);
        let mut parts = Vec::new();
        for id in &self.order[start_index..=end_index] {
            let Some(block) = self.blocks.get(id) else {
                continue;
            };
            let handle = block.text.borrow();
            let text = handle.text.as_str();
            let range = if *id == start.block && *id == end.block {
                clamp_boundary(text, start.offset)..clamp_boundary(text, end.offset)
            } else if *id == start.block {
                clamp_boundary(text, start.offset)..text.len()
            } else if *id == end.block {
                0..clamp_boundary(text, end.offset)
            } else {
                0..text.len()
            };
            if let Some(part) = text.get(range) {
                parts.push(part.to_string());
            }
        }
        parts.join("\n\n")
    }

    fn update_selection(&self, anchor: DocumentPoint, focus: DocumentPoint) {
        let text = self.selection_text(anchor, focus);
        self.state.set_selection(anchor, focus, text);
    }

    fn inline_at(&self, point: DocumentPoint) -> Option<PressedInline> {
        let block = self.blocks.get(&point.block)?.value.get();
        inline_targets(&block.kind)
            .into_iter()
            .find_map(|target| match target {
                InlineTarget::Link {
                    range, destination, ..
                } if range.contains(&point.offset) => Some(PressedInline::Link(destination)),
                InlineTarget::Image { range, image } if range.contains(&point.offset) => {
                    Some(PressedInline::Image(image.source))
                }
                _ => None,
            })
    }

    fn copy_selection(&self, ctx: &mut EventCtx) -> bool {
        let Some(text) = self.state.selected_text() else {
            return false;
        };
        ctx.set_clipboard_text(text);
        true
    }

    fn select_all(&self) -> bool {
        let first = self.order.iter().find_map(|id| {
            self.blocks.get(id).and_then(|block| {
                let handle = block.text.borrow();
                (!handle.text.is_empty()).then_some(DocumentPoint {
                    block: *id,
                    offset: 0,
                })
            })
        });
        let last = self.order.iter().rev().find_map(|id| {
            self.blocks.get(id).and_then(|block| {
                let handle = block.text.borrow();
                (!handle.text.is_empty()).then_some(DocumentPoint {
                    block: *id,
                    offset: handle.text.len(),
                })
            })
        });
        if let (Some(first), Some(last)) = (first, last) {
            self.update_selection(first, last);
            true
        } else {
            false
        }
    }
}

impl Widget for RichDocumentView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Pointer(pointer) = event
            && ctx.phase() != EventPhase::Bubble
        {
            match pointer.kind {
                PointerEventKind::Down
                    if pointer.button == Some(PointerButton::Primary)
                        && let Some(point) = self.point_at(pointer.position, true) =>
                {
                    let selection = self.state.selection();
                    let anchor = if pointer.modifiers.shift {
                        selection.anchor.unwrap_or(point)
                    } else {
                        point
                    };
                    self.dragging = true;
                    self.drag_anchor = Some(anchor);
                    self.pressed_inline = self.inline_at(point).map(|action| (point.block, action));
                    self.update_selection(anchor, point);
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                PointerEventKind::Move
                    if self.dragging
                        && pointer.buttons.contains(PointerButton::Primary)
                        && let (Some(anchor), Some(focus)) =
                            (self.drag_anchor, self.point_at(pointer.position, false)) =>
                {
                    if focus != anchor {
                        self.pressed_inline = None;
                    }
                    self.update_selection(anchor, focus);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                PointerEventKind::Up
                    if self.dragging && pointer.button == Some(PointerButton::Primary) =>
                {
                    let focus = self.point_at(pointer.position, false);
                    let activate = self.pressed_inline.take().and_then(|(block, action)| {
                        focus
                            .filter(|point| point.block == block)
                            .filter(|point| self.inline_at(*point).as_ref() == Some(&action))
                            .map(|_| action)
                    });
                    self.dragging = false;
                    self.drag_anchor = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    match activate {
                        Some(PressedInline::Link(destination)) => {
                            if let Some(callback) = &self.on_link {
                                (callback.borrow_mut())(&destination);
                            }
                        }
                        Some(PressedInline::Image(source)) => {
                            if let Some(callback) = &self.on_image {
                                (callback.borrow_mut())(&source);
                            }
                        }
                        None => {}
                    }
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
                PointerEventKind::Cancel if self.dragging => {
                    self.dragging = false;
                    self.drag_anchor = None;
                    self.pressed_inline = None;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.set_handled();
                }
                _ => {}
            }
            return;
        }

        match event {
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                let command = key.modifiers.control || key.modifiers.meta;
                match key.key.as_str() {
                    "a" | "A" if command => {
                        if self.select_all() {
                            ctx.request_paint();
                            ctx.request_semantics();
                            ctx.set_handled();
                        }
                    }
                    "c" | "C" if command => {
                        if self.copy_selection(ctx) {
                            ctx.set_handled();
                        }
                    }
                    "Escape" if self.state.clear_selection() => {
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            Event::Custom(custom) => {
                if let Some(command) = TextCommand::from_custom_event(custom) {
                    match command {
                        TextCommand::Copy => {
                            if self.copy_selection(ctx) {
                                ctx.set_handled();
                            }
                        }
                        TextCommand::SelectAll => {
                            if self.select_all() {
                                ctx.request_paint();
                                ctx.request_semantics();
                                ctx.set_handled();
                            }
                        }
                        TextCommand::Cut | TextCommand::Paste => {}
                    }
                }
            }
            Event::Semantics(semantics)
                if semantics.target == ctx.widget_id()
                    && semantics.action == SemanticsActionRequest::Copy
                    && self.copy_selection(ctx) =>
            {
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let _ = ctx.observe(self.model.structure_observable());
        self.sync_blocks();
        let child_constraints =
            Constraints::new(Size::ZERO, Size::new(constraints.max.width, f32::INFINITY));
        let mut width = 0.0_f32;
        let mut height = 0.0_f32;
        for (index, id) in self.order.iter().enumerate() {
            let Some(block) = self.blocks.get_mut(id) else {
                continue;
            };
            let size = block.pod.measure(ctx, child_constraints);
            width = width.max(size.width);
            height += size.height;
            self.measured_heights.insert(*id, size.height);
            if index + 1 < self.order.len() {
                height += BLOCK_GAP;
            }
        }
        let selection = self.state.selection();
        if let (Some(anchor), Some(focus)) = (selection.anchor, selection.focus) {
            self.update_selection(anchor, focus);
        }
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let mut y = bounds.y();
        for id in &self.order {
            let height = self.measured_heights.get(id).copied().unwrap_or(0.0);
            if let Some(block) = self.blocks.get_mut(id) {
                block
                    .pod
                    .arrange(ctx, Rect::new(bounds.x(), y, bounds.width(), height));
            }
            y += height + BLOCK_GAP;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        for id in &self.order {
            if let Some(block) = self.blocks.get(id) {
                block.pod.paint(ctx);
            }
        }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut document =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::Document, ctx.bounds());
        document.name = Some("Rich document".to_string());
        if self.state.selected_text().is_some() {
            document.actions.push(SemanticsAction::Copy);
        }
        ctx.push(document);
        for id in &self.order {
            if let Some(block) = self.blocks.get(id) {
                block.pod.semantics(ctx);
            }
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for id in &self.order {
            if let Some(block) = self.blocks.get(id) {
                visitor.visit(&block.pod);
            }
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for id in self.order.clone() {
            if let Some(block) = self.blocks.get_mut(&id) {
                visitor.visit(&mut block.pod);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SemanticPart {
    Copy,
    Link(usize),
    Image(usize),
    ListItem(usize),
}

#[derive(Clone)]
enum InlineTarget {
    Link {
        range: Range<usize>,
        destination: String,
        label: String,
    },
    Image {
        range: Range<usize>,
        image: RichInlineImage,
    },
}

struct DefaultBlockView {
    block: Signal<RichDocumentBlock>,
    state: RichDocumentViewState,
    text_handle: Rc<RefCell<BlockTextHandle>>,
    theme: DefaultTheme,
    highlighter: Rc<dyn RichSyntaxHighlighter>,
    layout: Option<PersistentTextLayout>,
    semantic_ids: HashMap<SemanticPart, WidgetId>,
    horizontal_offset: f32,
    max_horizontal_offset: f32,
    on_link: Option<StringAction>,
    on_image: Option<StringAction>,
    image_resolver: Option<ImageResolver>,
    on_attachment: Option<BlockAction>,
}

impl DefaultBlockView {
    #[allow(clippy::too_many_arguments)]
    fn new(
        block: Signal<RichDocumentBlock>,
        state: RichDocumentViewState,
        text_handle: Rc<RefCell<BlockTextHandle>>,
        theme: DefaultTheme,
        highlighter: Rc<dyn RichSyntaxHighlighter>,
        on_link: Option<StringAction>,
        on_image: Option<StringAction>,
        image_resolver: Option<ImageResolver>,
        on_attachment: Option<BlockAction>,
    ) -> Self {
        Self {
            block,
            state,
            text_handle,
            theme,
            highlighter,
            layout: None,
            semantic_ids: HashMap::new(),
            horizontal_offset: 0.0,
            max_horizontal_offset: 0.0,
            on_link,
            on_image,
            image_resolver,
            on_attachment,
        }
    }

    fn semantic_id(&mut self, part: SemanticPart) -> WidgetId {
        *self
            .semantic_ids
            .entry(part)
            .or_insert_with(next_semantic_id)
    }

    fn sync_semantic_ids(&mut self, block: &RichDocumentBlock) {
        let mut live = HashSet::new();
        for (index, target) in inline_targets(&block.kind).into_iter().enumerate() {
            let part = match target {
                InlineTarget::Link { .. } => SemanticPart::Link(index),
                InlineTarget::Image { .. } => SemanticPart::Image(index),
            };
            live.insert(part.clone());
            self.semantic_id(part);
        }
        if let RichDocumentBlockKind::List { items, .. } = &block.kind {
            for index in 0..items.len() {
                let part = SemanticPart::ListItem(index);
                live.insert(part.clone());
                self.semantic_id(part);
            }
        }
        if matches!(block.kind, RichDocumentBlockKind::CodeBlock { .. }) {
            live.insert(SemanticPart::Copy);
            self.semantic_id(SemanticPart::Copy);
        }
        self.semantic_ids.retain(|part, _| live.contains(part));
    }

    fn text_insets(&self, kind: &RichDocumentBlockKind) -> (f32, f32, f32, f32) {
        match kind {
            RichDocumentBlockKind::BlockQuote { .. } => {
                (QUOTE_INSET, TEXT_INSET, TEXT_INSET, TEXT_INSET)
            }
            RichDocumentBlockKind::CodeBlock { .. } => (
                CODE_INSET,
                CODE_HEADER_HEIGHT + CODE_INSET,
                CODE_INSET,
                CODE_INSET,
            ),
            RichDocumentBlockKind::Attachment(_) => (
                STRUCTURED_INSET,
                STRUCTURED_INSET,
                STRUCTURED_INSET,
                STRUCTURED_INSET,
            ),
            RichDocumentBlockKind::Extension(_) => (
                STRUCTURED_INSET + 20.0,
                STRUCTURED_INSET,
                STRUCTURED_INSET,
                STRUCTURED_INSET,
            ),
            RichDocumentBlockKind::ThematicBreak => (0.0, 0.0, 0.0, 0.0),
            _ => (TEXT_INSET, TEXT_INSET, TEXT_INSET, TEXT_INSET),
        }
    }

    fn layout_origin(&self, bounds: Rect, kind: &RichDocumentBlockKind) -> Point {
        let (left, top, _, _) = self.text_insets(kind);
        let layout_x = self
            .layout
            .as_ref()
            .map(|layout| layout.measurement().bounds.x())
            .unwrap_or(0.0);
        Point::new(
            bounds.x() + left - layout_x - self.horizontal_offset,
            bounds.y() + top,
        )
    }

    fn text_bounds(&self, bounds: Rect, kind: &RichDocumentBlockKind) -> Rect {
        let (left, top, right, bottom) = self.text_insets(kind);
        Rect::new(
            bounds.x() + left,
            bounds.y() + top,
            (bounds.width() - left - right).max(0.0),
            (bounds.height() - top - bottom).max(0.0),
        )
    }

    fn selection_range(&self, block: RichBlockId, text_len: usize) -> Range<usize> {
        let selection = self.state.selection();
        selection_range_for(&selection, block, text_len)
    }

    fn status_colors(&self, status: RichDocumentStatus) -> (sui_core::Color, sui_core::Color) {
        match status {
            RichDocumentStatus::Neutral => (self.theme.palette.control, self.theme.palette.text),
            RichDocumentStatus::Pending => (
                self.theme.palette.control_hover,
                self.theme.palette.text_muted,
            ),
            RichDocumentStatus::Running => (
                self.theme.palette.info_soft,
                self.theme.palette.info_soft_text,
            ),
            RichDocumentStatus::Success => (
                self.theme.palette.success_soft,
                self.theme.palette.success_soft_text,
            ),
            RichDocumentStatus::Warning => (
                self.theme.palette.warning_soft,
                self.theme.palette.warning_soft_text,
            ),
            RichDocumentStatus::Error => (
                self.theme.palette.danger_soft,
                self.theme.palette.danger_soft_text,
            ),
        }
    }

    fn invoke_semantic_target(
        &mut self,
        target: WidgetId,
        action: &SemanticsActionRequest,
        ctx: &mut EventCtx,
    ) -> bool {
        let block = self.block.get();
        if self.semantic_ids.get(&SemanticPart::Copy) == Some(&target)
            && *action == SemanticsActionRequest::Copy
            && let RichDocumentBlockKind::CodeBlock { code, .. } = &block.kind
        {
            ctx.set_clipboard_text(code);
            return true;
        }
        for (index, inline) in inline_targets(&block.kind).into_iter().enumerate() {
            let part = match inline {
                InlineTarget::Link { .. } => SemanticPart::Link(index),
                InlineTarget::Image { .. } => SemanticPart::Image(index),
            };
            if self.semantic_ids.get(&part) != Some(&target)
                || *action != SemanticsActionRequest::Activate
            {
                continue;
            }
            match inline {
                InlineTarget::Link { destination, .. } => {
                    if let Some(callback) = &self.on_link {
                        (callback.borrow_mut())(&destination);
                    }
                }
                InlineTarget::Image {
                    image: inline_image,
                    ..
                } => {
                    if let Some(callback) = &self.on_image {
                        (callback.borrow_mut())(&inline_image.source);
                    }
                }
            }
            return true;
        }
        false
    }
}

impl Widget for DefaultBlockView {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if ctx.phase() == EventPhase::Capture {
            return;
        }
        let block = self.block.get();
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Scroll
                    && ctx.bounds().contains(pointer.position)
                    && matches!(block.kind, RichDocumentBlockKind::CodeBlock { .. }) =>
            {
                let delta = pointer
                    .scroll_delta
                    .map(scroll_delta_to_offset)
                    .unwrap_or(pointer.delta);
                let axis = if delta.x.abs() > f32::EPSILON {
                    delta.x
                } else if pointer.modifiers.shift {
                    delta.y
                } else {
                    0.0
                };
                let next = (self.horizontal_offset - axis).clamp(0.0, self.max_horizontal_offset);
                if (next - self.horizontal_offset).abs() > f32::EPSILON {
                    self.horizontal_offset = next;
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && matches!(block.kind, RichDocumentBlockKind::CodeBlock { .. })
                    && pointer.position.y <= ctx.bounds().y() + CODE_HEADER_HEIGHT
                    && pointer.position.x >= ctx.bounds().max_x() - 56.0 =>
            {
                if let RichDocumentBlockKind::CodeBlock { code, .. } = &block.kind {
                    ctx.set_clipboard_text(code);
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && matches!(block.kind, RichDocumentBlockKind::Attachment(_))
                    && ctx.bounds().contains(pointer.position) =>
            {
                if let Some(callback) = &self.on_attachment {
                    (callback.borrow_mut())(block.id);
                }
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && matches!(block.kind, RichDocumentBlockKind::Extension(_))
                    && ctx.bounds().contains(pointer.position) =>
            {
                if let RichDocumentBlockKind::Extension(extension) = &block.kind {
                    let current = self
                        .state
                        .inner
                        .borrow()
                        .expanded
                        .get(&block.id)
                        .copied()
                        .unwrap_or(extension.initially_expanded);
                    self.state.set_expanded(block.id, !current);
                }
                ctx.request_measure();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Semantics(semantics) => {
                let handled = if semantics.target == ctx.widget_id() {
                    match (&block.kind, &semantics.action) {
                        (
                            RichDocumentBlockKind::Attachment(_),
                            SemanticsActionRequest::Activate,
                        ) => {
                            if let Some(callback) = &self.on_attachment {
                                (callback.borrow_mut())(block.id);
                            }
                            true
                        }
                        (RichDocumentBlockKind::Extension(_), SemanticsActionRequest::Expand) => {
                            self.state.set_expanded(block.id, true);
                            true
                        }
                        (RichDocumentBlockKind::Extension(_), SemanticsActionRequest::Collapse) => {
                            self.state.set_expanded(block.id, false);
                            true
                        }
                        _ => false,
                    }
                } else {
                    self.invoke_semantic_target(semantics.target, &semantics.action, ctx)
                };
                if handled {
                    ctx.request_measure();
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let block = ctx.observe(&self.block);
        let _ = ctx.observe(&self.state.revision);
        self.sync_semantic_ids(&block);
        if matches!(block.kind, RichDocumentBlockKind::ThematicBreak) {
            self.layout = None;
            let size = constraints.clamp(Size::new(constraints.max.width, 9.0));
            let mut handle = self.text_handle.borrow_mut();
            handle.text.clear();
            handle.layout = None;
            return size;
        }

        let expanded = match &block.kind {
            RichDocumentBlockKind::Extension(extension) => self
                .state
                .inner
                .borrow()
                .expanded
                .get(&block.id)
                .copied()
                .unwrap_or(extension.initially_expanded),
            _ => false,
        };
        let document = block_document(&block.kind, self.theme, self.highlighter.as_ref(), expanded);
        let text = document.plain_text();
        let (left, top, right, bottom) = self.text_insets(&block.kind);
        let available_width = (constraints.max.width - left - right).max(1.0);
        let request = if matches!(block.kind, RichDocumentBlockKind::CodeBlock { .. }) {
            TextLayoutRequest::new(document)
        } else {
            TextLayoutRequest::new(document).with_box_size(Size::new(available_width, 1.0))
        };
        let handle = self.layout.as_ref().map(PersistentTextLayout::handle);
        self.layout = ctx
            .layout()
            .layout_document_persistent(handle, request)
            .ok();
        let (measured_width, measured_height) = self
            .layout
            .as_ref()
            .map(|layout| {
                let measurement = layout.measurement();
                (measurement.width, measurement.height)
            })
            .unwrap_or((0.0, 0.0));
        self.max_horizontal_offset =
            if matches!(block.kind, RichDocumentBlockKind::CodeBlock { .. }) {
                (measured_width - available_width).max(0.0)
            } else {
                0.0
            };
        self.horizontal_offset = self.horizontal_offset.min(self.max_horizontal_offset);
        let height = measured_height + top + bottom;
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            measured_width + left + right
        };
        let size = constraints.clamp(Size::new(width, height));
        let mut shared = self.text_handle.borrow_mut();
        shared.text = text;
        shared.layout = self.layout.clone();
        size
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, bounds: Rect) {
        let block = self.block.get();
        let mut shared = self.text_handle.borrow_mut();
        shared.bounds = self.text_bounds(bounds, &block.kind);
        shared.origin = self.layout_origin(bounds, &block.kind);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let block = self.block.get();
        let _ = self.state.revision.get();
        let bounds = ctx.bounds();
        match &block.kind {
            RichDocumentBlockKind::BlockQuote { .. } => {
                ctx.fill_rect(
                    Rect::new(bounds.x() + 2.0, bounds.y(), 3.0, bounds.height()),
                    self.theme.palette.accent,
                );
            }
            RichDocumentBlockKind::CodeBlock { language, .. } => {
                ctx.fill_rect(bounds, self.theme.surfaces.window_subtle);
                ctx.stroke_rect(
                    bounds,
                    self.theme.surfaces.border_subtle,
                    StrokeStyle::new(1.0),
                );
                ctx.fill_rect(
                    Rect::new(bounds.x(), bounds.y(), bounds.width(), CODE_HEADER_HEIGHT),
                    self.theme.surfaces.surface_2,
                );
                let mut label = self.theme.text_style(self.theme.palette.text_muted);
                label.font_size = (label.font_size - 2.0).max(10.0);
                ctx.draw_text(
                    Rect::new(
                        bounds.x() + CODE_INSET,
                        bounds.y() + 7.0,
                        (bounds.width() - 80.0).max(1.0),
                        label.line_height,
                    ),
                    language.as_deref().unwrap_or("plain text"),
                    label,
                );
                let mut action = self.theme.text_style(self.theme.palette.accent_soft_text);
                action.font_size = (action.font_size - 2.0).max(10.0);
                ctx.draw_text(
                    Rect::new(
                        bounds.max_x() - 42.0,
                        bounds.y() + 7.0,
                        38.0,
                        action.line_height,
                    ),
                    "Copy",
                    action,
                );
            }
            RichDocumentBlockKind::Attachment(_) => {
                ctx.fill_rect(bounds, self.theme.palette.control);
                ctx.stroke_rect(bounds, self.theme.palette.border, StrokeStyle::new(1.0));
            }
            RichDocumentBlockKind::Extension(extension) => {
                let (background, _) = self.status_colors(extension.status);
                ctx.fill_rect(bounds, background);
                ctx.stroke_rect(bounds, self.theme.palette.border, StrokeStyle::new(1.0));
                let expanded = self
                    .state
                    .inner
                    .borrow()
                    .expanded
                    .get(&block.id)
                    .copied()
                    .unwrap_or(extension.initially_expanded);
                let mut disclosure = self.theme.text_style(self.theme.palette.text_muted);
                disclosure.weight = FontWeight::SEMIBOLD;
                ctx.draw_text(
                    Rect::new(
                        bounds.x() + STRUCTURED_INSET,
                        bounds.y() + STRUCTURED_INSET,
                        16.0,
                        disclosure.line_height,
                    ),
                    if expanded { "▾" } else { "▸" },
                    disclosure,
                );
            }
            RichDocumentBlockKind::ThematicBreak => {
                ctx.fill_rect(
                    Rect::new(bounds.x(), bounds.y() + 4.0, bounds.width(), 1.0),
                    self.theme.palette.border,
                );
            }
            _ => {}
        }

        let Some(layout) = &self.layout else {
            return;
        };
        let text_bounds = self.text_bounds(bounds, &block.kind);
        let origin = self.layout_origin(bounds, &block.kind);
        ctx.push_clip_rect(text_bounds);
        let selection = self.selection_range(block.id, layout.text().len());
        if !selection.is_empty() {
            for rect in layout.selection_rects(selection) {
                ctx.fill_rect(
                    rect.translate(origin.to_vector()),
                    self.theme.palette.selection,
                );
            }
        }
        ctx.draw_persistent_text_layout(origin, layout);
        if let Some(resolver) = &self.image_resolver {
            for target in inline_targets(&block.kind) {
                let InlineTarget::Image { range, image } = target else {
                    continue;
                };
                let Some(handle) = resolver(&image) else {
                    continue;
                };
                for rect in layout.selection_rects(range) {
                    let rect = rect.translate(origin.to_vector());
                    ctx.fill_rect(rect, self.theme.palette.surface);
                    ctx.draw_image(rect, handle);
                }
            }
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let block = self.block.get();
        let _ = self.state.revision.get();
        let role = match block.kind {
            RichDocumentBlockKind::Paragraph { .. } => SemanticsRole::Paragraph,
            RichDocumentBlockKind::Heading { .. } => SemanticsRole::Heading,
            RichDocumentBlockKind::BlockQuote { .. } => SemanticsRole::GenericContainer,
            RichDocumentBlockKind::List { .. } => SemanticsRole::List,
            RichDocumentBlockKind::CodeBlock { .. } => SemanticsRole::Code,
            RichDocumentBlockKind::ThematicBreak => SemanticsRole::Separator,
            RichDocumentBlockKind::Attachment(_) => SemanticsRole::Attachment,
            RichDocumentBlockKind::Extension(_) => SemanticsRole::Status,
        };
        let mut node = SemanticsNode::new(ctx.widget_id(), role, ctx.bounds());
        node.name = Some(block.plain_text());
        match &block.kind {
            RichDocumentBlockKind::Heading { level, .. } => {
                node.description = Some(format!("Heading level {level}"));
            }
            RichDocumentBlockKind::CodeBlock { language, .. } => {
                node.description = Some(format!(
                    "{} code block",
                    language.as_deref().unwrap_or("Plain text")
                ));
            }
            RichDocumentBlockKind::Attachment(attachment) => {
                node.actions.push(SemanticsAction::Activate);
                node.description = attachment.description.clone().or_else(|| {
                    attachment
                        .media_type
                        .as_ref()
                        .map(|media_type| format!("Attachment, {media_type}"))
                });
            }
            RichDocumentBlockKind::Extension(extension) => {
                let expanded = self
                    .state
                    .inner
                    .borrow()
                    .expanded
                    .get(&block.id)
                    .copied()
                    .unwrap_or(extension.initially_expanded);
                node.name = Some(extension.title.clone());
                if expanded && !extension.body.is_empty() {
                    node.value = Some(SemanticsValue::Text(extension.body.clone()));
                }
                node.state.expanded = Some(expanded);
                node.state.busy = matches!(
                    extension.status,
                    RichDocumentStatus::Pending | RichDocumentStatus::Running
                );
                node.actions.push(if expanded {
                    SemanticsAction::Collapse
                } else {
                    SemanticsAction::Expand
                });
                node.description = extension.summary.clone();
            }
            _ => {}
        }
        ctx.push(node);

        if let RichDocumentBlockKind::List { items, .. } = &block.kind {
            for (index, item) in items.iter().enumerate() {
                let Some(id) = self
                    .semantic_ids
                    .get(&SemanticPart::ListItem(index))
                    .copied()
                else {
                    continue;
                };
                let mut child = SemanticsNode::new(id, SemanticsRole::ListItem, ctx.bounds());
                child.parent = Some(ctx.widget_id());
                child.name = Some(item.plain_text());
                child.state.checked = item.checked.map(|checked| {
                    if checked {
                        sui_core::ToggleState::Checked
                    } else {
                        sui_core::ToggleState::Unchecked
                    }
                });
                ctx.push(child);
            }
        }

        for (index, target) in inline_targets(&block.kind).into_iter().enumerate() {
            match target {
                InlineTarget::Link {
                    destination, label, ..
                } => {
                    let Some(id) = self.semantic_ids.get(&SemanticPart::Link(index)).copied()
                    else {
                        continue;
                    };
                    let mut link = SemanticsNode::new(id, SemanticsRole::Link, ctx.bounds());
                    link.parent = Some(ctx.widget_id());
                    link.name = Some(label);
                    link.value = Some(SemanticsValue::Text(destination));
                    link.actions.push(SemanticsAction::Activate);
                    ctx.push(link);
                }
                InlineTarget::Image {
                    image: inline_image,
                    ..
                } => {
                    let Some(id) = self.semantic_ids.get(&SemanticPart::Image(index)).copied()
                    else {
                        continue;
                    };
                    let mut image = SemanticsNode::new(id, SemanticsRole::Image, ctx.bounds());
                    image.parent = Some(ctx.widget_id());
                    image.name = Some(inline_image.alt);
                    image.value = Some(SemanticsValue::Text(inline_image.source));
                    image.actions.push(SemanticsAction::Activate);
                    ctx.push(image);
                }
            }
        }

        if let RichDocumentBlockKind::CodeBlock { .. } = block.kind
            && let Some(id) = self.semantic_ids.get(&SemanticPart::Copy).copied()
        {
            let mut copy = SemanticsNode::new(
                id,
                SemanticsRole::Button,
                Rect::new(
                    ctx.bounds().max_x() - 52.0,
                    ctx.bounds().y(),
                    52.0,
                    CODE_HEADER_HEIGHT,
                ),
            );
            copy.parent = Some(ctx.widget_id());
            copy.name = Some("Copy code".to_string());
            copy.actions.push(SemanticsAction::Copy);
            ctx.push(copy);
        }
    }
}

fn block_document(
    kind: &RichDocumentBlockKind,
    theme: DefaultTheme,
    highlighter: &dyn RichSyntaxHighlighter,
    expanded: bool,
) -> TextDocument {
    match kind {
        RichDocumentBlockKind::Paragraph { spans } => {
            document_from_spans(spans, theme.body_text_style(), theme)
        }
        RichDocumentBlockKind::Heading { level, spans } => {
            let mut style = theme.body_text_style();
            style.font_size *= match level {
                1 => 1.75,
                2 => 1.48,
                3 => 1.28,
                4 => 1.15,
                _ => 1.05,
            };
            style.line_height = style.font_size * 1.3;
            style.weight = FontWeight::BOLD;
            document_from_spans(spans, style, theme)
        }
        RichDocumentBlockKind::BlockQuote { spans, .. } => {
            let mut style = theme.body_text_style();
            style.color = theme.palette.text_muted;
            style.style = FontStyle::Italic;
            document_from_spans(spans, style, theme)
        }
        RichDocumentBlockKind::List { start, items } => {
            let mut spans = Vec::new();
            let mut next = start.unwrap_or(1);
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    spans.push(TextSpan::new("\n", theme.body_text_style()));
                }
                let marker = match item.checked {
                    Some(true) => "☑ ".to_string(),
                    Some(false) => "☐ ".to_string(),
                    None if start.is_some() => {
                        let marker = format!("{next}. ");
                        next = next.saturating_add(1);
                        marker
                    }
                    None => "• ".to_string(),
                };
                let mut marker_style = theme.body_text_style();
                marker_style.color = theme.palette.text_muted;
                spans.push(TextSpan::new(marker, marker_style));
                spans.extend(styled_spans(&item.spans, theme.body_text_style(), theme));
            }
            document_from_text_spans(spans)
        }
        RichDocumentBlockKind::CodeBlock { language, code } => {
            let base = theme.mono_text_style(theme.palette.text);
            let highlights = highlighter.highlight(language.as_deref(), code);
            document_from_text_spans(syntax_spans(code, &highlights, base, theme))
        }
        RichDocumentBlockKind::ThematicBreak => TextDocument::new(),
        RichDocumentBlockKind::Attachment(attachment) => {
            let mut spans = vec![TextSpan::new(format!("📎 {}", attachment.name), {
                let mut style = theme.body_text_style();
                style.weight = FontWeight::SEMIBOLD;
                style
            })];
            if let Some(description) = &attachment.description {
                let mut style = theme.body_text_style();
                style.color = theme.palette.text_muted;
                spans.push(TextSpan::new(format!("\n{description}"), style));
            }
            document_from_text_spans(spans)
        }
        RichDocumentBlockKind::Extension(extension) => {
            let mut title = theme.body_text_style();
            title.weight = FontWeight::SEMIBOLD;
            let mut spans = vec![TextSpan::new(extension.title.clone(), title)];
            if let Some(summary) = &extension.summary {
                let mut style = theme.body_text_style();
                style.color = theme.palette.text_muted;
                spans.push(TextSpan::new(format!("\n{summary}"), style));
            }
            if expanded && !extension.body.is_empty() {
                spans.push(TextSpan::new(
                    format!("\n\n{}", extension.body),
                    theme.mono_text_style(theme.palette.text),
                ));
            }
            document_from_text_spans(spans)
        }
    }
}

fn document_from_spans(
    spans: &[RichDocumentSpan],
    base: TextStyle,
    theme: DefaultTheme,
) -> TextDocument {
    document_from_text_spans(styled_spans(spans, base, theme))
}

fn document_from_text_spans(spans: Vec<TextSpan>) -> TextDocument {
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();
    let mut empty_line_style = spans
        .first()
        .map(|span| span.style.clone())
        .unwrap_or_default();

    for span in spans {
        empty_line_style = span.style.clone();
        let mut remainder = span.text.as_str();
        while let Some(newline) = remainder.find('\n') {
            if newline > 0 {
                current.push(TextSpan::new(&remainder[..newline], span.style.clone()));
            }
            if current.is_empty() {
                current.push(TextSpan::new("", span.style.clone()));
            }
            paragraphs.push(TextParagraph::from_spans(std::mem::take(&mut current)));
            remainder = &remainder[newline + 1..];
        }
        if !remainder.is_empty() {
            current.push(TextSpan::new(remainder, span.style));
        }
    }

    if current.is_empty() {
        current.push(TextSpan::new("", empty_line_style));
    }
    paragraphs.push(TextParagraph::from_spans(current));
    TextDocument { paragraphs }
}

fn styled_spans(spans: &[RichDocumentSpan], base: TextStyle, theme: DefaultTheme) -> Vec<TextSpan> {
    spans
        .iter()
        .map(|span| {
            let mut style = base.clone();
            if span.style.strong {
                style.weight = FontWeight::BOLD;
            }
            if span.style.emphasis {
                style.style = FontStyle::Italic;
            }
            match &span.kind {
                RichInlineKind::Code => {
                    style.font_families = Some(theme.fonts.mono.into());
                    style.color = theme.palette.accent_soft_text;
                }
                RichInlineKind::Link(_) => {
                    style.color = theme.palette.accent_soft_text;
                    style.weight = FontWeight::SEMIBOLD;
                }
                RichInlineKind::Image(_) => {
                    style.color = theme.palette.text_muted;
                    style.style = FontStyle::Italic;
                }
                RichInlineKind::Text => {}
            }
            TextSpan::new(span.text.clone(), style)
        })
        .collect()
}

fn syntax_spans(
    source: &str,
    highlights: &[RichSyntaxSpan],
    base: TextStyle,
    theme: DefaultTheme,
) -> Vec<TextSpan> {
    let mut highlights = highlights
        .iter()
        .filter_map(|span| {
            let start = clamp_boundary(source, span.range.start);
            let end = clamp_boundary(source, span.range.end).max(start);
            (start < end).then_some((start..end, span.kind))
        })
        .collect::<Vec<_>>();
    highlights.sort_by_key(|(range, _)| range.start);
    let mut output = Vec::new();
    let mut cursor = 0;
    for (range, kind) in highlights {
        if range.start < cursor {
            continue;
        }
        if cursor < range.start {
            output.push(TextSpan::new(&source[cursor..range.start], base.clone()));
        }
        let mut style = base.clone();
        style.color = syntax_color(theme, kind);
        output.push(TextSpan::new(&source[range.clone()], style));
        cursor = range.end;
    }
    if cursor < source.len() {
        output.push(TextSpan::new(&source[cursor..], base.clone()));
    }
    if output.is_empty() {
        output.push(TextSpan::new(source, base));
    }
    output
}

fn syntax_color(theme: DefaultTheme, kind: RichSyntaxTokenKind) -> sui_core::Color {
    match kind {
        RichSyntaxTokenKind::Keyword | RichSyntaxTokenKind::Header => {
            theme.palette.accent_soft_text
        }
        RichSyntaxTokenKind::String | RichSyntaxTokenKind::Added => theme.palette.success_text,
        RichSyntaxTokenKind::Number | RichSyntaxTokenKind::Property => theme.palette.info_text,
        RichSyntaxTokenKind::Comment => theme.palette.text_muted,
        RichSyntaxTokenKind::Type | RichSyntaxTokenKind::Function => theme.palette.warning_text,
        RichSyntaxTokenKind::Removed => theme.palette.danger_text,
    }
}

fn inline_targets(kind: &RichDocumentBlockKind) -> Vec<InlineTarget> {
    let spans = match kind {
        RichDocumentBlockKind::Paragraph { spans }
        | RichDocumentBlockKind::Heading { spans, .. }
        | RichDocumentBlockKind::BlockQuote { spans, .. } => {
            spans.iter().map(|span| (span, 0_usize)).collect::<Vec<_>>()
        }
        RichDocumentBlockKind::List { start, items } => {
            let mut offset = 0;
            let mut next = start.unwrap_or(1);
            let mut flattened = Vec::new();
            for (item_index, item) in items.iter().enumerate() {
                if item_index > 0 {
                    offset += 1;
                }
                offset += match item.checked {
                    Some(_) => "☐ ".len(),
                    None if start.is_some() => {
                        let len = format!("{next}. ").len();
                        next = next.saturating_add(1);
                        len
                    }
                    None => "• ".len(),
                };
                for span in &item.spans {
                    flattened.push((span, offset));
                    offset += span.text.len();
                }
            }
            flattened
        }
        _ => Vec::new(),
    };
    let mut offset = 0;
    let mut targets = Vec::new();
    for (span, explicit_offset) in spans {
        if explicit_offset > 0 {
            offset = explicit_offset;
        }
        let range = offset..offset + span.text.len();
        match &span.kind {
            RichInlineKind::Link(link) => targets.push(InlineTarget::Link {
                range,
                destination: link.destination.clone(),
                label: span.text.clone(),
            }),
            RichInlineKind::Image(image) => targets.push(InlineTarget::Image {
                range,
                image: image.clone(),
            }),
            RichInlineKind::Text | RichInlineKind::Code => {}
        }
        offset += span.text.len();
    }
    targets
}

fn selection_range_for(
    selection: &DocumentSelection,
    block: RichBlockId,
    text_len: usize,
) -> Range<usize> {
    let (Some(anchor), Some(focus)) = (selection.anchor, selection.focus) else {
        return 0..0;
    };
    let Some(anchor_index) = selection.order.iter().position(|id| *id == anchor.block) else {
        return 0..0;
    };
    let Some(focus_index) = selection.order.iter().position(|id| *id == focus.block) else {
        return 0..0;
    };
    let Some(block_index) = selection.order.iter().position(|id| *id == block) else {
        return 0..0;
    };
    let ((start_index, start_offset), (end_index, end_offset)) =
        if (anchor_index, anchor.offset) <= (focus_index, focus.offset) {
            ((anchor_index, anchor.offset), (focus_index, focus.offset))
        } else {
            ((focus_index, focus.offset), (anchor_index, anchor.offset))
        };
    if block_index < start_index || block_index > end_index {
        return 0..0;
    }
    let start = if block_index == start_index {
        start_offset.min(text_len)
    } else {
        0
    };
    let end = if block_index == end_index {
        end_offset.min(text_len)
    } else {
        text_len
    };
    start.min(end)..end.max(start)
}

fn clamp_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    offset
}

fn scroll_delta_to_offset(delta: ScrollDelta) -> Vector {
    match delta {
        ScrollDelta::Lines(delta) => Vector::new(delta.x * 40.0, delta.y * 40.0),
        ScrollDelta::Pixels(delta) => delta,
    }
}

fn next_semantic_id() -> WidgetId {
    WidgetId::new(SYNTHETIC_RICH_DOCUMENT_TAG | NEXT_SYNTHETIC_ID.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, cell::RefCell, rc::Rc};

    use sui_core::{
        Event, ImageHandle, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
        ScrollDelta, SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsRole,
        SemanticsValue, Size, Vector, WindowEvent,
    };
    use sui_layout::Constraints;
    use sui_reactive::Signal;
    use sui_runtime::{Application, MeasureCtx, Runtime, SemanticsCtx, Widget, WindowBuilder};
    use sui_scene::{RegisteredImage, SceneCommand};

    use super::{
        RichDocumentBlock, RichDocumentBlockKind, RichDocumentModel, RichDocumentRenderContext,
        RichDocumentRendererRegistry, RichDocumentView, RichDocumentViewState,
    };
    use crate::{RichAttachment, RichExtensionBlock, TextCommand};

    fn runtime<W>(root: W) -> (Runtime, sui_core::WindowId)
    where
        W: Widget + 'static,
    {
        let runtime = Application::new()
            .window(WindowBuilder::new().title("Rich document").root(root))
            .build()
            .expect("rich document runtime should build");
        let window_id = runtime.window_ids()[0];
        (runtime, window_id)
    }

    fn node<'a>(nodes: &'a [SemanticsNode], role: SemanticsRole, name: &str) -> &'a SemanticsNode {
        nodes
            .iter()
            .find(|node| node.role == role && node.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("missing {role:?} semantics named {name:?}"))
    }

    fn primary_pointer(kind: PointerEventKind, position: Point, pressed: bool) -> Event {
        let mut pointer = PointerEvent::new(kind, position);
        pointer.pointer_id = 7;
        pointer.button = Some(PointerButton::Primary);
        pointer.buttons = if pressed {
            PointerButtons::new(1)
        } else {
            PointerButtons::NONE
        };
        Event::Pointer(pointer)
    }

    #[test]
    fn streaming_append_retains_completed_block_widget_identity() {
        let model = RichDocumentModel::from_markdown("# Stable\n\nStreaming");
        let (mut runtime, window_id) = runtime(RichDocumentView::new(model.clone()));
        let first = runtime.render(window_id).unwrap();
        let heading_id = node(&first.semantics, SemanticsRole::Heading, "Stable").id;
        let paragraph_id = node(&first.semantics, SemanticsRole::Paragraph, "Streaming").id;

        model.append_markdown(" text\n\n- next");
        let second = runtime.render(window_id).unwrap();
        assert_eq!(
            node(&second.semantics, SemanticsRole::Heading, "Stable").id,
            heading_id
        );
        assert_eq!(
            node(
                &second.semantics,
                SemanticsRole::Paragraph,
                "Streaming text"
            )
            .id,
            paragraph_id
        );
        assert!(
            second
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::List)
        );
    }

    #[test]
    fn document_semantics_expose_links_images_code_status_and_attachments() {
        let model = RichDocumentModel::from_markdown(
            "# Report\n\nRead [docs](https://example.test) and ![plot](plot.png).\n\n- one\n- [x] two\n\n```rust\nlet n = 42;\n```",
        );
        model.append_attachment(RichAttachment::new("results.csv"));
        let mut extension = RichExtensionBlock::new("tool-call", "Search");
        extension.summary = Some("3 results".to_string());
        extension.body = "structured payload".to_string();
        model.append_extension(extension);

        let opened = Rc::new(RefCell::new(Vec::new()));
        let opened_link = Rc::clone(&opened);
        let images = Rc::new(RefCell::new(Vec::new()));
        let opened_image = Rc::clone(&images);
        let (mut runtime, window_id) = runtime(
            RichDocumentView::new(model)
                .on_link(move |destination| opened_link.borrow_mut().push(destination.to_string()))
                .on_image(move |source| opened_image.borrow_mut().push(source.to_string())),
        );
        let first = runtime.render(window_id).unwrap();
        assert!(
            first
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Document)
        );
        assert!(
            first
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::ListItem && node.state.checked.is_some())
        );
        assert!(
            first
                .semantics
                .iter()
                .any(|node| node.role == SemanticsRole::Attachment)
        );

        let link = node(&first.semantics, SemanticsRole::Link, "docs");
        assert_eq!(
            link.value,
            Some(SemanticsValue::Text("https://example.test".to_string()))
        );
        let link_id = link.id;
        let image_id = node(&first.semantics, SemanticsRole::Image, "plot").id;
        let copy = node(&first.semantics, SemanticsRole::Button, "Copy code");
        assert!(copy.actions.contains(&SemanticsAction::Copy));
        let copy_id = copy.id;
        let status = node(&first.semantics, SemanticsRole::Status, "Search");
        assert_eq!(status.state.expanded, Some(false));
        let status_id = status.id;

        assert!(
            runtime
                .handle_semantics_action(window_id, link_id, SemanticsActionRequest::Activate)
                .unwrap()
        );
        assert!(
            runtime
                .handle_semantics_action(window_id, image_id, SemanticsActionRequest::Activate)
                .unwrap()
        );
        assert_eq!(opened.borrow().as_slice(), &["https://example.test"]);
        assert_eq!(images.borrow().as_slice(), &["plot.png"]);

        assert!(
            runtime
                .handle_semantics_action(window_id, copy_id, SemanticsActionRequest::Copy)
                .unwrap()
        );
        assert_eq!(runtime.clipboard().text().as_deref(), Some("let n = 42;\n"));

        assert!(
            runtime
                .handle_semantics_action(window_id, status_id, SemanticsActionRequest::Expand)
                .unwrap()
        );
        let expanded = runtime.render(window_id).unwrap();
        let status = node(&expanded.semantics, SemanticsRole::Status, "Search");
        assert_eq!(status.id, status_id);
        assert_eq!(status.state.expanded, Some(true));
        assert_eq!(
            status.value,
            Some(SemanticsValue::Text("structured payload".to_string()))
        );
    }

    #[test]
    fn overflowing_code_block_scrolls_horizontally() {
        let model = RichDocumentModel::from_markdown(
            "```rust\nfn answer() -> u32 {\n    let diagnostic_context = \"a deliberately long code line that must overflow a narrow rich document viewport\";\n    42\n}\n```",
        );
        let (mut runtime, window_id) = runtime(RichDocumentView::new(model));
        runtime
            .handle_event(
                window_id,
                Event::Window(WindowEvent::Resized(Size::new(320.0, 240.0))),
            )
            .unwrap();
        let first = runtime.render(window_id).unwrap();
        let code = first
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Code)
            .expect("code semantics");
        assert!(
            code.bounds.height() > 100.0,
            "multiline code should retain every visual line, bounds={:?}",
            code.bounds
        );
        let first_origin = first
            .frame
            .scene
            .commands()
            .iter()
            .filter_map(|command| match command {
                SceneCommand::DrawShapedText(text) => Some(text),
                _ => None,
            })
            .max_by(|left, right| left.bounds.width().total_cmp(&right.bounds.width()))
            .map(|text| text.origin)
            .expect("code text draw");

        let code_center = Point::new(
            code.bounds.x() + code.bounds.width() * 0.5,
            code.bounds.y() + code.bounds.height() * 0.5,
        );
        let mut scroll = PointerEvent::new(PointerEventKind::Scroll, code_center);
        scroll.scroll_delta = Some(ScrollDelta::Pixels(Vector::new(-180.0, 0.0)));
        runtime
            .handle_event(window_id, Event::Pointer(scroll))
            .unwrap();
        let second = runtime.render(window_id).unwrap();
        let second_origin = second
            .frame
            .scene
            .commands()
            .iter()
            .filter_map(|command| match command {
                SceneCommand::DrawShapedText(text) => Some(text),
                _ => None,
            })
            .max_by(|left, right| left.bounds.width().total_cmp(&right.bounds.width()))
            .map(|text| text.origin)
            .expect("scrolled code text draw");

        assert!(
            second_origin.x < first_origin.x,
            "expected code origin to move left, before={first_origin:?}, after={second_origin:?}"
        );
    }

    #[test]
    fn inline_image_resolver_paints_registered_image() {
        let handle = ImageHandle::new(71);
        let model = RichDocumentModel::from_markdown("![plot](asset:plot)");
        let mut application = Application::new();
        application
            .register_image(
                handle,
                RegisteredImage::from_rgba8(8, 8, vec![255; 8 * 8 * 4]).unwrap(),
            )
            .unwrap();
        let mut runtime =
            application
                .window(WindowBuilder::new().title("Inline image").root(
                    RichDocumentView::new(model).image_resolver(move |image| {
                        (image.source == "asset:plot").then_some(handle)
                    }),
                ))
                .build()
                .unwrap();
        let output = runtime.render(runtime.window_ids()[0]).unwrap();
        let mut draws = 0;
        output.frame.scene.visit_commands(&mut |command| {
            if matches!(command, SceneCommand::DrawImage { source, .. } if source.image == handle) {
                draws += 1;
            }
        });
        assert_eq!(draws, 1);
    }

    #[test]
    fn pointer_drag_selects_text_across_multiple_blocks() {
        let model = RichDocumentModel::from_markdown("# First block\n\nSecond block");
        let state = RichDocumentViewState::new();
        let (mut runtime, window_id) = runtime(RichDocumentView::new(model).state(state.clone()));
        let first = runtime.render(window_id).unwrap();
        let heading = node(&first.semantics, SemanticsRole::Heading, "First block");
        let paragraph = node(&first.semantics, SemanticsRole::Paragraph, "Second block");
        let start = Point::new(heading.bounds.x() + 4.0, heading.bounds.y() + 4.0);
        let end = Point::new(
            paragraph.bounds.max_x() - 4.0,
            paragraph.bounds.y() + paragraph.bounds.height() * 0.5,
        );

        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Down, start, true),
            )
            .unwrap();
        runtime
            .handle_event(
                window_id,
                primary_pointer(PointerEventKind::Move, end, true),
            )
            .unwrap();
        runtime
            .handle_event(window_id, primary_pointer(PointerEventKind::Up, end, false))
            .unwrap();

        let selected = state.selected_text().expect("cross-block selection");
        assert!(selected.contains("First block"), "selected {selected:?}");
        assert!(selected.contains("Second block"), "selected {selected:?}");

        runtime
            .handle_event(window_id, TextCommand::Copy.into_event())
            .unwrap();
        assert_eq!(
            runtime.clipboard().text().as_deref(),
            Some(selected.as_str())
        );
    }

    struct ExtensionProbe {
        block: Signal<RichDocumentBlock>,
        measures: Option<Rc<Cell<usize>>>,
    }

    impl Widget for ExtensionProbe {
        fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
            let _ = ctx.observe(&self.block);
            if let Some(measures) = &self.measures {
                measures.set(measures.get() + 1);
            }
            constraints.clamp(Size::new(160.0, 32.0))
        }

        fn semantics(&self, ctx: &mut SemanticsCtx) {
            let block = self.block.get();
            let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Status, ctx.bounds());
            node.name = Some(format!("Custom: {}", block.plain_text()));
            ctx.push(node);
        }
    }

    #[test]
    fn extension_registry_updates_payload_without_replacing_renderer_widget() {
        let registry = RichDocumentRendererRegistry::new();
        registry.register(
            "extension:tool-call",
            |context: RichDocumentRenderContext| ExtensionProbe {
                block: context.block,
                measures: None,
            },
        );
        let model = RichDocumentModel::new();
        let id = model.append_extension(RichExtensionBlock::new("tool-call", "Pending"));
        let (mut runtime, window_id) =
            runtime(RichDocumentView::new(model.clone()).renderer_registry(registry));
        let first = runtime.render(window_id).unwrap();
        let widget_id = node(&first.semantics, SemanticsRole::Status, "Custom: Pending").id;

        let mut completed = RichExtensionBlock::new("tool-call", "Complete");
        completed.body = "42".to_string();
        assert!(model.update_structured(id, RichDocumentBlockKind::Extension(completed)));
        let second = runtime.render(window_id).unwrap();
        assert_eq!(
            node(
                &second.semantics,
                SemanticsRole::Status,
                "Custom: Complete\n42"
            )
            .id,
            widget_id
        );
    }

    #[test]
    fn tail_update_does_not_remeasure_retained_prefix_renderer() {
        let registry = RichDocumentRendererRegistry::new();
        let measures = Rc::new(Cell::new(0));
        let renderer_measures = Rc::clone(&measures);
        registry.register(
            "extension:tool-call",
            move |context: RichDocumentRenderContext| ExtensionProbe {
                block: context.block,
                measures: Some(Rc::clone(&renderer_measures)),
            },
        );
        let model = RichDocumentModel::new();
        model.append_extension(RichExtensionBlock::new("tool-call", "Stable prefix"));
        model.append_markdown("Streaming");
        let (mut runtime, window_id) =
            runtime(RichDocumentView::new(model.clone()).renderer_registry(registry));
        runtime.render(window_id).unwrap();
        let initial_measures = measures.get();

        model.append_markdown(" tail");
        runtime.render(window_id).unwrap();
        assert_eq!(measures.get(), initial_measures);
    }
}
