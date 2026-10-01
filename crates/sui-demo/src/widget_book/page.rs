//! The scrolling page: an intro, then one header per category followed by
//! that category's story blocks. Items keep fixed positions in the scroll
//! view so navigation can jump by index; filtering collapses items in place.

use std::{cell::RefCell, rc::Rc};

use sui::prelude::*;
use sui::{Rect, WidgetPodMutVisitor, WidgetPodVisitor};

use super::registry::{Category, Story, StoryCtx, stories, stories_in};
use super::specimen::{ColorDot, Section, Stage};
use super::{BookState, GALLERY_SCROLL_NAME};
use crate::app::{DemoTextRole, DevThemeReader, demo_mono_text_style, demo_text_style};
use crate::demo_support::{
    CenteredContentWidth, DemoTextColor, MaximumWidth, NamedSection, demo_label,
};

/// Widest the page content grows before centering in wide windows.
const PAGE_MAX_WIDTH: f32 = 1120.0;
const TEXT_MAX_WIDTH: f32 = 720.0;
const PAGE_PADDING: Insets = Insets {
    left: 32.0,
    top: 28.0,
    right: 32.0,
    bottom: 48.0,
};

/// One entry in the page's scroll view.
#[derive(Clone, Copy)]
pub(crate) enum PageItem {
    Intro,
    Category(Category),
    Story(&'static Story),
    NoResults,
}

/// Every page item in scroll order. Indices into this list are the scroll
/// view's item indices.
pub(crate) fn page_items() -> Vec<PageItem> {
    let mut items = vec![PageItem::Intro];
    for category in Category::ALL {
        items.push(PageItem::Category(category));
        items.extend(stories_in(category).map(PageItem::Story));
    }
    items.push(PageItem::NoResults);
    items
}

pub(crate) fn category_item_index(category: Category) -> usize {
    page_items()
        .iter()
        .position(|item| matches!(item, PageItem::Category(candidate) if *candidate == category))
        .expect("every category has a page item")
}

pub(crate) fn story_item_index(id: &str) -> Option<usize> {
    page_items()
        .iter()
        .position(|item| matches!(item, PageItem::Story(story) if story.id == id))
}

pub(crate) fn category_has_matches(category: Category, query: &str) -> bool {
    stories_in(category).any(|story| story.matches(query))
}

pub(crate) fn matching_story_count(query: &str) -> usize {
    stories()
        .iter()
        .filter(|story| story.matches(query))
        .count()
}

pub(crate) fn build_page(
    state: Rc<RefCell<BookState>>,
    theme_reader: DevThemeReader,
    scroll_state: ScrollState,
) -> VirtualScrollView {
    let mut page = VirtualScrollView::new()
        .name(GALLERY_SCROLL_NAME)
        .state(scroll_state)
        .padding(PAGE_PADDING)
        // Items own their trailing gaps so filtered items collapse completely.
        .spacing(0.0)
        .theme_when({
            let theme_reader = Rc::clone(&theme_reader);
            move || theme_reader()
        });
    for item in page_items() {
        page = match item {
            PageItem::Intro => page.with_child(Filtered::new(
                || true,
                32.0,
                centered(intro(Rc::clone(&theme_reader))),
            )),
            PageItem::Category(category) => {
                let visible_state = Rc::clone(&state);
                page.with_child(Filtered::new(
                    move || category_has_matches(category, &visible_state.borrow().query),
                    20.0,
                    themed(&theme_reader, move |theme| {
                        centered(category_header(category, theme))
                    }),
                ))
            }
            PageItem::Story(story) => {
                let visible_state = Rc::clone(&state);
                page.with_child(Filtered::new(
                    move || story.matches(&visible_state.borrow().query),
                    44.0,
                    themed(&theme_reader, move |theme| {
                        centered(story_block(story, theme))
                    }),
                ))
            }
            PageItem::NoResults => {
                let visible_state = Rc::clone(&state);
                page.with_child(Filtered::new(
                    move || matching_story_count(&visible_state.borrow().query) == 0,
                    0.0,
                    centered(no_results(Rc::clone(&theme_reader))),
                ))
            }
        };
    }
    page
}

fn centered<W>(child: W) -> CenteredContentWidth
where
    W: Widget + 'static,
{
    CenteredContentWidth::new(PAGE_MAX_WIDTH, child)
}

/// Rebuilds `build` whenever the resolved theme changes, so specimens can
/// take a concrete theme instead of threading readers through every widget.
fn themed<W, F>(theme_reader: &DevThemeReader, build: F) -> RebuildOnChange<DefaultTheme>
where
    W: Widget + 'static,
    F: Fn(DefaultTheme) -> W + 'static,
{
    let theme_reader = Rc::clone(theme_reader);
    RebuildOnChange::key_when(
        move || theme_reader(),
        move |theme| WidgetPod::new(build(*theme)),
    )
}

fn intro(theme_reader: DevThemeReader) -> impl Widget {
    let components = stories().len();
    let categories = Category::ALL.len();
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Start)
        .with_child(demo_label(
            &theme_reader,
            "Widget Book",
            DemoTextRole::PageTitle,
            DemoTextColor::Text,
        ))
        .with_child(MaximumWidth::new(
            TEXT_MAX_WIDTH,
            demo_label(
                &theme_reader,
                "Every SUI component and its variations on one page. Jump from the sidebar, filter by name, and switch themes to compare.",
                DemoTextRole::Body,
                DemoTextColor::Muted,
            ),
        ))
        .with_child(demo_label(
            &theme_reader,
            format!("{components} components in {categories} categories"),
            DemoTextRole::Metadata,
            DemoTextColor::Muted,
        ))
}

fn no_results(theme_reader: DevThemeReader) -> impl Widget {
    Stack::vertical()
        .spacing(6.0)
        .alignment(Alignment::Start)
        .with_child(demo_label(
            &theme_reader,
            "No components match your filter",
            DemoTextRole::Emphasis,
            DemoTextColor::Text,
        ))
        .with_child(demo_label(
            &theme_reader,
            "Try a component name like Button, or a concept like progress or color.",
            DemoTextRole::Supporting,
            DemoTextColor::Muted,
        ))
}

pub(crate) fn category_region_name(category: Category) -> String {
    format!("{} category", category.title())
}

fn category_header(category: Category, theme: DefaultTheme) -> impl Widget {
    let hue = theme.decorative.get(category.hue()).solid;
    let count = stories_in(category).count();
    let count_label = if count == 1 {
        "1 component".to_string()
    } else {
        format!("{count} components")
    };
    NamedSection::new(
        category_region_name(category),
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Stack::horizontal()
                    .spacing(10.0)
                    .alignment(Alignment::Center)
                    .with_child(ColorDot::new(10.0, move || hue))
                    .with_child(Label::new(category.title()).text_style(demo_text_style(
                        theme,
                        DemoTextRole::SectionTitle,
                        theme.palette.text,
                    )))
                    .with_child(Label::new(count_label).text_style(demo_text_style(
                        theme,
                        DemoTextRole::Metadata,
                        theme.palette.text_muted,
                    ))),
            )
            .with_child(Label::new(category.summary()).text_style(demo_text_style(
                theme,
                DemoTextRole::Supporting,
                theme.palette.text_muted,
            )))
            .with_child(Separator::horizontal().theme(theme)),
    )
}

pub(crate) fn story_block(story: &'static Story, theme: DefaultTheme) -> impl Widget {
    let sections = (story.build)(&StoryCtx::new(theme));
    NamedSection::new(
        story.region_name(),
        Stack::vertical()
            .spacing(8.0)
            .alignment(Alignment::Stretch)
            .with_child(
                Flex::horizontal()
                    .gap(12.0)
                    .wrap(FlexWrap::Wrap)
                    .align_items(Alignment::Center)
                    .with_child(Label::new(story.title).text_style(demo_text_style(
                        theme,
                        DemoTextRole::Emphasis,
                        theme.palette.text,
                    )))
                    .with_child(Label::new(story.api).text_style(demo_mono_text_style(
                        theme,
                        DemoTextRole::Metadata,
                        theme.palette.text_muted,
                    ))),
            )
            .with_child(MaximumWidth::new(
                TEXT_MAX_WIDTH,
                Label::new(story.summary).text_style(demo_text_style(
                    theme,
                    DemoTextRole::Supporting,
                    theme.palette.text_muted,
                )),
            ))
            .with_child(SizedBox::new().height(4.0))
            .with_child(Stage::new(theme, section_stack(sections, theme))),
    )
}

fn section_stack(sections: Vec<Section>, theme: DefaultTheme) -> Stack {
    sections.into_iter().fold(
        Stack::vertical().spacing(24.0).alignment(Alignment::Start),
        |stack, section| {
            let mut column = Stack::vertical().spacing(12.0).alignment(Alignment::Start);
            if let Some(caption) = section.caption {
                column = column.with_child(Label::new(caption).text_style(demo_text_style(
                    theme,
                    DemoTextRole::CardTitle,
                    theme.palette.text,
                )));
            }
            stack.with_child(column.with_child(BoxedChild::new(section.content)))
        },
    )
}

/// Hosts a type-erased specimen without adding layout behavior.
struct BoxedChild {
    child: SingleChild,
}

impl BoxedChild {
    fn new(child: Box<dyn Widget>) -> Self {
        Self {
            child: SingleChild::from_pod(WidgetPod::new_boxed(child)),
        }
    }
}

impl Widget for BoxedChild {
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

/// Collapses to zero height when its predicate fails and otherwise adds a
/// trailing gap, so hidden page items leave no space behind.
pub(crate) struct Filtered {
    visible: Box<dyn Fn() -> bool>,
    trailing_gap: f32,
    shown: bool,
    child: SingleChild,
}

impl Filtered {
    pub(crate) fn new<F, W>(visible: F, trailing_gap: f32, child: W) -> Self
    where
        F: Fn() -> bool + 'static,
        W: Widget + 'static,
    {
        Self {
            visible: Box::new(visible),
            trailing_gap,
            shown: true,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for Filtered {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.shown = (self.visible)();
        if !self.shown {
            let width = if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                constraints.min.width
            };
            return constraints.clamp(Size::new(width, 0.0));
        }
        let child = self.child.measure(ctx, constraints);
        constraints.clamp(Size::new(child.width, child.height + self.trailing_gap))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        if self.shown {
            let height = self
                .child
                .child()
                .measured_size()
                .height
                .min(bounds.height());
            self.child.arrange(
                ctx,
                Rect::new(bounds.x(), bounds.y(), bounds.width(), height),
            );
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        if self.shown {
            self.child.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        if self.shown {
            self.child.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        if self.shown {
            self.child.visit_children(visitor);
        }
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        if self.shown {
            self.child.visit_children_mut(visitor);
        }
    }
}
