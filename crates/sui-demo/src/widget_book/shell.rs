//! The widget book frame: a navigation rail that follows the scroll position,
//! a top bar with the filter and theme switch, and the scrolling page.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use sui::prelude::*;
use sui::{
    InvalidationKind, InvalidationRequest, InvalidationTarget, KeyState, PointerButton,
    PointerEventKind, Rect, SemanticsAction, SemanticsActionRequest, SemanticsNode, SemanticsRole,
    Vector, WidgetPodMutVisitor, WidgetPodVisitor, paint_text_line,
};

use super::page::{
    Filtered, build_page, category_has_matches, category_item_index, story_item_index,
};
use super::registry::{Category, Story, stories, stories_in};
use super::{
    BookState, ThemeChoice, WIDGET_BOOK_NAV_NAME, WIDGET_BOOK_SEARCH_NAME, WIDGET_BOOK_SHELL_NAME,
    WIDGET_BOOK_THEME_SWITCH_NAME,
};
use crate::app::{DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style};
use crate::demo_support::{DemoTextColor, demo_label};

const RAIL_WIDTH: f32 = 236.0;
/// Below this width the rail hides and the page takes the full window.
const RAIL_BREAKPOINT: f32 = 900.0;
const TOP_BAR_HEIGHT: f32 = 56.0;
const RAIL_HEADER_HEIGHT: f32 = 76.0;
/// How far below the viewport top an item must reach before the rail marks
/// it current. Small enough that a jumped-to item is immediately current.
const SCROLL_SPY_OFFSET: f32 = 24.0;
/// Semantics name of the rail's scroll view.
pub(crate) const RAIL_SCROLL_NAME: &str = "Widget book navigation scroll";

fn request_book_refresh(ctx: &mut EventCtx) {
    for kind in [
        InvalidationKind::Measure,
        InvalidationKind::Paint,
        InvalidationKind::HitTest,
        InvalidationKind::Semantics,
    ] {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            kind,
        ));
    }
}

/// Repaints the rail and page chrome without relayout, for current-item and
/// hover changes that span several rail entries.
fn request_book_repaint(ctx: &mut EventCtx) {
    for kind in [InvalidationKind::Paint, InvalidationKind::Semantics] {
        ctx.request(InvalidationRequest::new(
            InvalidationTarget::Window(ctx.window_id()),
            kind,
        ));
    }
}

pub(crate) fn build_shell(
    state: Rc<RefCell<BookState>>,
    theme_reader: DevThemeReader,
    theme_choices: &'static [ThemeChoice],
) -> BookShell {
    let scroll_state = ScrollState::new();
    let nav = Rc::new(NavShared {
        state: Rc::clone(&state),
        scroll_state: scroll_state.clone(),
        rail_scroll_state: ScrollState::new(),
        active_item: Cell::new(None),
        jump_origin: Cell::new(None),
        entry_extents: RefCell::new(Vec::new()),
    });
    let page = build_page(
        Rc::clone(&state),
        Rc::clone(&theme_reader),
        scroll_state.clone(),
    )
    .on_offset_change_with_ctx({
        let nav = Rc::clone(&nav);
        move |ctx, _| nav.follow_page(ctx)
    });
    let top_bar = top_bar(
        Rc::clone(&state),
        Rc::clone(&theme_reader),
        scroll_state,
        theme_choices,
    );
    let rail = rail(Rc::clone(&theme_reader), Rc::clone(&nav));
    BookShell::new(
        theme_reader,
        rail,
        Dock::new(page).top(TOP_BAR_HEIGHT, top_bar),
    )
}

fn top_bar(
    state: Rc<RefCell<BookState>>,
    theme_reader: DevThemeReader,
    scroll_state: ScrollState,
    theme_choices: &'static [ThemeChoice],
) -> impl Widget {
    let query_state = Rc::clone(&state);
    let search = TextInput::new(WIDGET_BOOK_SEARCH_NAME)
        .placeholder("Filter components")
        .leading_icon(IconGlyph::Search)
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_change_with_ctx(move |ctx, value| {
            query_state.borrow_mut().query = value;
            let _ = scroll_state.set_offset(Vector::ZERO);
            request_book_refresh(ctx);
        });
    let selected_index = |state: &Rc<RefCell<BookState>>| {
        let state = Rc::clone(state);
        move || {
            let current = state.borrow().theme;
            theme_choices.iter().position(|choice| *choice == current)
        }
    };
    let select_choice = |state: &Rc<RefCell<BookState>>| {
        let state = Rc::clone(state);
        move |ctx: &mut EventCtx, index: usize| {
            if let Some(choice) = theme_choices.get(index) {
                state.borrow_mut().theme = *choice;
                request_book_refresh(ctx);
            }
        }
    };
    let on_segment = select_choice(&state);
    let segmented = SegmentedControl::new(WIDGET_BOOK_THEME_SWITCH_NAME)
        .segments(theme_choices.iter().map(|choice| choice.label()))
        .selected_when(selected_index(&state))
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_change_with_ctx(move |ctx, index, _| on_segment(ctx, index));
    let on_option = select_choice(&state);
    let compact = Select::new(WIDGET_BOOK_THEME_SWITCH_NAME)
        .options(theme_choices.iter().map(|choice| choice.label()))
        .selected_when(selected_index(&state))
        .theme_when(clone_dev_theme_reader(&theme_reader))
        .on_change_with_ctx(move |ctx, index, _| on_option(ctx, index));
    let label = demo_label(
        &theme_reader,
        "Theme",
        DemoTextRole::Supporting,
        DemoTextColor::Muted,
    )
    .single_line(true);
    Padding::new(
        Insets {
            left: 24.0,
            top: 0.0,
            right: 24.0,
            bottom: 1.0,
        },
        Align::new(
            Alignment::Stretch,
            Alignment::Center,
            Flex::horizontal()
                .gap(16.0)
                .align_items(Alignment::Center)
                .with_item(
                    search,
                    FlexItem::new().basis(280.0).min_width(160.0).shrink(1.0),
                )
                .with_item(
                    ThemeSwitch::new(label, segmented, compact),
                    FlexItem::new().grow(1.0).basis(0.0),
                ),
        ),
    )
}

/// Shows the labeled segmented theme switch when it fits and a compact select
/// otherwise, right-aligned in the space it is given.
struct ThemeSwitch {
    label: SingleChild,
    segmented: SingleChild,
    compact: SingleChild,
    wide: bool,
}

impl ThemeSwitch {
    const LABEL_GAP: f32 = 12.0;
    const COMPACT_WIDTH: f32 = 180.0;

    fn new<L, W, C>(label: L, segmented: W, compact: C) -> Self
    where
        L: Widget + 'static,
        W: Widget + 'static,
        C: Widget + 'static,
    {
        Self {
            label: SingleChild::new(label),
            segmented: SingleChild::new(segmented),
            compact: SingleChild::new(compact),
            wide: true,
        }
    }
}

impl Widget for ThemeSwitch {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let loose = Constraints::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));
        let label = self.label.measure(ctx, loose);
        let segmented = self.segmented.measure(ctx, loose);
        let wide_width = label.width + Self::LABEL_GAP + segmented.width;
        let available = constraints.max.width;
        self.wide = !available.is_finite() || wide_width <= available;
        let size = if self.wide {
            Size::new(wide_width, label.height.max(segmented.height))
        } else {
            self.compact.measure(
                ctx,
                Constraints::new(
                    Size::ZERO,
                    Size::new(Self::COMPACT_WIDTH.min(available), f32::INFINITY),
                ),
            )
        };
        let width = if available.is_finite() {
            available
        } else {
            size.width
        };
        constraints.clamp(Size::new(width, size.height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let centered = |size: Size, x: f32| {
            Rect::new(
                x,
                bounds.y() + (bounds.height() - size.height) * 0.5,
                size.width,
                size.height,
            )
        };
        if self.wide {
            let segmented = self.segmented.child().measured_size();
            let label = self.label.child().measured_size();
            let segmented_x = bounds.max_x() - segmented.width;
            self.segmented
                .arrange(ctx, centered(segmented, segmented_x));
            self.label.arrange(
                ctx,
                centered(label, segmented_x - Self::LABEL_GAP - label.width),
            );
        } else {
            let compact = self.compact.child().measured_size();
            self.compact
                .arrange(ctx, centered(compact, bounds.max_x() - compact.width));
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if self.wide {
            self.label.paint(ctx);
            self.segmented.paint(ctx);
        } else {
            self.compact.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if self.wide {
            self.label.semantics(ctx);
            self.segmented.semantics(ctx);
        } else {
            self.compact.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.wide {
            self.label.visit_children(visitor);
            self.segmented.visit_children(visitor);
        } else {
            self.compact.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.wide {
            self.label.visit_children_mut(visitor);
            self.segmented.visit_children_mut(visitor);
        } else {
            self.compact.visit_children_mut(visitor);
        }
    }
}

fn rail(theme_reader: DevThemeReader, nav: Rc<NavShared>) -> impl Widget {
    let header = Padding::new(
        Insets {
            left: 20.0,
            top: 20.0,
            right: 16.0,
            bottom: 12.0,
        },
        Stack::vertical()
            .gap(2.0)
            .alignment(Alignment::Start)
            .with_child(demo_label(
                &theme_reader,
                "SUI Widget Book",
                DemoTextRole::CardTitle,
                DemoTextColor::Text,
            ))
            .with_child(demo_label(
                &theme_reader,
                format!("{} components", stories().len()),
                DemoTextRole::Metadata,
                DemoTextColor::Muted,
            )),
    );
    Dock::new(
        ScrollView::vertical(Padding::new(
            Insets {
                left: 10.0,
                top: 0.0,
                right: 10.0,
                bottom: 20.0,
            },
            BookNav::new(theme_reader.clone(), Rc::clone(&nav)),
        ))
        .name(RAIL_SCROLL_NAME)
        .state(nav.rail_scroll_state.clone())
        .theme_when(clone_dev_theme_reader(&theme_reader)),
    )
    .top(RAIL_HEADER_HEIGHT, header)
}

/// Navigation state shared by the rail entries.
pub(crate) struct NavShared {
    state: Rc<RefCell<BookState>>,
    scroll_state: ScrollState,
    /// The rail's own scroll position, so the current entry stays in view.
    rail_scroll_state: ScrollState,
    /// Page item the rail marks as current.
    active_item: Cell<Option<usize>>,
    /// Scroll offset a requested jump lands on. The rail keeps the jump
    /// target current until the page scrolls away from it, so a target the
    /// page cannot bring to the top, near its end, stays marked.
    jump_origin: Cell<Option<f32>>,
    /// Rail entry extents relative to the rail list's top, from its last
    /// arrange; entries follow page order without the page intro.
    entry_extents: RefCell<Vec<(f32, f32)>>,
}

impl NavShared {
    fn jump(&self, item: usize, ctx: &mut EventCtx) {
        let state = &self.scroll_state;
        let landing = state
            .virtual_item_offset(item)
            .map_or(state.current_offset().y, |top| {
                top.clamp(0.0, state.max_offset().y)
            });
        self.jump_origin.set(Some(landing));
        self.active_item.set(Some(item));
        let _ = self.scroll_state.scroll_to_item_with_ctx(item, ctx);
    }

    /// Scrolls the rail so the current entry is visible. Returns whether the
    /// rail moved and needs a layout pass.
    fn reveal_active_entry(&self) -> bool {
        // Rail entries follow page order without the page intro.
        let Some(entry) = self.active_item.get().and_then(|item| item.checked_sub(1)) else {
            return false;
        };
        let Some((top, bottom)) = self.entry_extents.borrow().get(entry).copied() else {
            return false;
        };
        let rail = &self.rail_scroll_state;
        let offset = rail.current_offset().y;
        let viewport = rail.viewport_size().height;
        if viewport <= 0.0 || (top >= offset && bottom <= offset + viewport) {
            return false;
        }
        // Keep a little context above the entry rather than pinning it to
        // the edge.
        rail.set_offset(Vector::new(0.0, (top - viewport * 0.25).max(0.0)))
    }

    /// Moves the current entry to follow the page's scroll position, and
    /// the rail to keep that entry in view.
    fn follow_page(&self, ctx: &mut EventCtx) {
        if self.sync_scroll_spy() {
            if self.reveal_active_entry() {
                request_book_refresh(ctx);
            } else {
                request_book_repaint(ctx);
            }
        }
    }

    /// Recomputes the current item from the scroll position. Returns whether
    /// it changed.
    fn sync_scroll_spy(&self) -> bool {
        let offset = self.scroll_state.current_offset().y;
        if let Some(origin) = self.jump_origin.get() {
            if (offset - origin).abs() < 0.5 {
                return false;
            }
            self.jump_origin.set(None);
        }
        let next = self
            .scroll_state
            .virtual_item_at(offset + SCROLL_SPY_OFFSET)
            .filter(|item| *item > 0);
        let changed = self.active_item.get() != next;
        self.active_item.set(next);
        changed
    }
}

/// The rail's list of category and component entries.
struct BookNav {
    nav: Rc<NavShared>,
    entries: WidgetChildren,
}

impl BookNav {
    fn new(theme_reader: DevThemeReader, nav: Rc<NavShared>) -> Self {
        let mut entries = WidgetChildren::new();
        for category in Category::ALL {
            let visible_state = Rc::clone(&nav.state);
            entries.push(Filtered::new(
                move || category_has_matches(category, &visible_state.borrow().query),
                0.0,
                NavEntry::category(category, theme_reader.clone(), Rc::clone(&nav)),
            ));
            for story in stories_in(category) {
                let visible_state = Rc::clone(&nav.state);
                entries.push(Filtered::new(
                    move || story.matches(&visible_state.borrow().query),
                    0.0,
                    NavEntry::story(story, theme_reader.clone(), Rc::clone(&nav)),
                ));
            }
        }
        Self { nav, entries }
    }
}

impl Widget for BookNav {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            RAIL_WIDTH
        };
        let entry_constraints =
            Constraints::new(Size::new(width, 0.0), Size::new(width, f32::INFINITY));
        let height = self
            .entries
            .as_mut_slice()
            .iter_mut()
            .map(|entry| entry.measure(ctx, entry_constraints).height)
            .sum();
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let mut extents = self.nav.entry_extents.borrow_mut();
        extents.clear();
        let mut y = bounds.y();
        for entry in self.entries.as_mut_slice() {
            let height = entry.measured_size().height;
            entry.arrange(ctx, Rect::new(bounds.x(), y, bounds.width(), height));
            extents.push((y - bounds.y(), y - bounds.y() + height));
            y += height;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.entries.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::List, ctx.bounds());
        node.name = Some(WIDGET_BOOK_NAV_NAME.to_string());
        ctx.push(node);
        self.entries.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.entries.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.entries.visit_children_mut(visitor);
    }
}

#[derive(Clone, Copy)]
enum NavTarget {
    Category(Category),
    Story(&'static Story),
}

/// One rail entry: a category heading or a component link.
struct NavEntry {
    target: NavTarget,
    item: usize,
    theme_reader: DevThemeReader,
    nav: Rc<NavShared>,
    hovered: bool,
    pressed: bool,
}

impl NavEntry {
    const CATEGORY_HEIGHT: f32 = 34.0;
    const CATEGORY_TOP_GAP: f32 = 10.0;
    const STORY_HEIGHT: f32 = 28.0;
    const STORY_INDENT: f32 = 28.0;

    fn category(category: Category, theme_reader: DevThemeReader, nav: Rc<NavShared>) -> Self {
        Self {
            target: NavTarget::Category(category),
            item: category_item_index(category),
            theme_reader,
            nav,
            hovered: false,
            pressed: false,
        }
    }

    fn story(story: &'static Story, theme_reader: DevThemeReader, nav: Rc<NavShared>) -> Self {
        Self {
            target: NavTarget::Story(story),
            item: story_item_index(story.id).expect("every story has a page item"),
            theme_reader,
            nav,
            hovered: false,
            pressed: false,
        }
    }

    fn label(&self) -> &'static str {
        match self.target {
            NavTarget::Category(category) => category.title(),
            NavTarget::Story(story) => story.title,
        }
    }

    fn is_active(&self) -> bool {
        self.nav.active_item.get() == Some(self.item)
    }

    /// The clickable row, excluding the category gap above it.
    fn row_rect(&self, bounds: Rect) -> Rect {
        match self.target {
            NavTarget::Category(_) => Rect::new(
                bounds.x(),
                bounds.y() + Self::CATEGORY_TOP_GAP,
                bounds.width(),
                bounds.height() - Self::CATEGORY_TOP_GAP,
            ),
            NavTarget::Story(_) => bounds,
        }
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        self.nav.jump(self.item, ctx);
        request_book_repaint(ctx);
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
        }
    }
}

impl Widget for NavEntry {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Move | PointerEventKind::Enter
                ) =>
            {
                let hovered = self.row_rect(ctx.bounds()).contains(pointer.position);
                self.set_hovered(hovered, ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && self.row_rect(ctx.bounds()).contains(pointer.position) =>
            {
                self.pressed = true;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary)
                    && self.pressed =>
            {
                self.pressed = false;
                ctx.release_pointer_capture(pointer.pointer_id);
                if self.row_rect(ctx.bounds()).contains(pointer.position) {
                    self.activate(ctx);
                }
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if ctx.is_focused()
                    && key.state == KeyState::Pressed
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.activate(ctx);
                ctx.set_handled();
            }
            Event::Semantics(request)
                if request.target == ctx.widget_id()
                    && request.action == SemanticsActionRequest::Activate =>
            {
                self.activate(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let height = match self.target {
            NavTarget::Category(_) => Self::CATEGORY_HEIGHT + Self::CATEGORY_TOP_GAP,
            NavTarget::Story(_) => Self::STORY_HEIGHT,
        };
        constraints.clamp(Size::new(constraints.max.width.min(RAIL_WIDTH), height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let palette = theme.palette;
        let row = self.row_rect(ctx.bounds());
        let active = self.is_active();
        if active || self.hovered || ctx.is_focused() {
            let fill = if active {
                palette.selection
            } else {
                palette.control_hover
            };
            ctx.fill(Path::rounded_rect(row, 6.0), fill);
        }
        if active {
            let bar = Rect::new(row.x() + 1.0, row.y() + 6.0, 3.0, row.height() - 12.0);
            ctx.fill(Path::rounded_rect(bar, 1.5), palette.accent);
        }
        match self.target {
            NavTarget::Category(category) => {
                let dot = theme.decorative.get(category.hue()).solid;
                let center = Point::new(row.x() + 16.0, row.y() + row.height() * 0.5);
                ctx.fill(Path::circle(center, 4.0), dot);
                let style = demo_text_style(theme, DemoTextRole::CardTitle, palette.text);
                let label = Rect::new(
                    row.x() + Self::STORY_INDENT,
                    row.y(),
                    (row.width() - Self::STORY_INDENT - 36.0).max(0.0),
                    row.height(),
                );
                paint_text_line(ctx, label, category.title(), &style, TextAlign::Start);
                let query = self.nav.state.borrow().query.clone();
                let count = stories_in(category)
                    .filter(|story| story.matches(&query))
                    .count();
                let count_style =
                    demo_text_style(theme, DemoTextRole::Metadata, palette.text_muted);
                let count_rect = Rect::new(row.max_x() - 40.0, row.y(), 32.0, row.height());
                paint_text_line(
                    ctx,
                    count_rect,
                    &count.to_string(),
                    &count_style,
                    TextAlign::End,
                );
            }
            NavTarget::Story(story) => {
                let color = if active {
                    palette.text
                } else {
                    palette.text_muted
                };
                let style = demo_text_style(theme, DemoTextRole::Supporting, color);
                let label = Rect::new(
                    row.x() + Self::STORY_INDENT,
                    row.y(),
                    (row.width() - Self::STORY_INDENT - 8.0).max(0.0),
                    row.height(),
                );
                paint_text_line(ctx, label, story.title, &style, TextAlign::Start);
            }
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::Link,
            self.row_rect(ctx.bounds()),
        );
        node.name = Some(self.label().to_string());
        node.state.selected = self.is_active();
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered;
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

/// Rail on the left, page on the right; the rail hides in narrow windows.
pub(crate) struct BookShell {
    theme_reader: DevThemeReader,
    rail: SingleChild,
    content: SingleChild,
    rail_visible: bool,
}

impl BookShell {
    fn new<R, C>(theme_reader: DevThemeReader, rail: R, content: C) -> Self
    where
        R: Widget + 'static,
        C: Widget + 'static,
    {
        Self {
            theme_reader,
            rail: SingleChild::new(rail),
            content: SingleChild::new(content),
            rail_visible: true,
        }
    }

    fn rail_extent(&self) -> f32 {
        if self.rail_visible {
            RAIL_WIDTH + 1.0
        } else {
            0.0
        }
    }
}

impl Widget for BookShell {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width.max(1280.0)
        };
        let height = if constraints.max.height.is_finite() {
            constraints.max.height
        } else {
            constraints.min.height.max(760.0)
        };
        self.rail_visible = width >= RAIL_BREAKPOINT;
        if self.rail_visible {
            self.rail
                .measure(ctx, Constraints::tight(Size::new(RAIL_WIDTH, height)));
        }
        self.content.measure(
            ctx,
            Constraints::tight(Size::new((width - self.rail_extent()).max(0.0), height)),
        );
        constraints.clamp(Size::new(width, height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if self.rail_visible {
            self.rail.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), RAIL_WIDTH, bounds.height()),
            );
        }
        let rail = self.rail_extent();
        self.content.arrange(
            ctx,
            Rect::new(
                bounds.x() + rail,
                bounds.y(),
                (bounds.width() - rail).max(0.0),
                bounds.height(),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let neutrals = theme.colors.neutrals;
        let bounds = ctx.bounds();
        ctx.fill_rect(bounds, neutrals.window);
        let content_x = bounds.x() + self.rail_extent();
        ctx.fill_rect(
            Rect::new(
                content_x,
                bounds.y() + TOP_BAR_HEIGHT - 1.0,
                (bounds.max_x() - content_x).max(0.0),
                1.0,
            ),
            neutrals.border_subtle,
        );
        if self.rail_visible {
            ctx.fill_rect(
                Rect::new(bounds.x(), bounds.y(), RAIL_WIDTH, bounds.height()),
                neutrals.subtle,
            );
            ctx.fill_rect(
                Rect::new(bounds.x() + RAIL_WIDTH, bounds.y(), 1.0, bounds.height()),
                neutrals.border_subtle,
            );
            self.rail.paint(ctx);
        }
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(WIDGET_BOOK_SHELL_NAME.to_string());
        ctx.push(node);
        if self.rail_visible {
            self.rail.semantics(ctx);
        }
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.rail_visible {
            self.rail.visit_children(visitor);
        }
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.rail_visible {
            self.rail.visit_children_mut(visitor);
        }
        self.content.visit_children_mut(visitor);
    }
}
