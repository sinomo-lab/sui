//! Layout helpers and text utilities shared by the demo surfaces: the
//! widget book, the Themes page, benchmarks, and validation views.

#![forbid(unsafe_code)]

use std::rc::Rc;

use sui::prelude::*;
use sui::{
    Rect, SemanticsNode, SemanticsRole, TextStyle, ThemeTextToken, WidgetPodMutVisitor,
    WidgetPodVisitor, paint_text_line,
};

use crate::app::{
    DemoTextRole, DevThemeReader, demo_mono_text_style_when, demo_text_style_when, dev_theme_color,
};

pub(crate) fn default_theme_reader() -> DevThemeReader {
    let theme = DefaultTheme::default();
    Rc::new(move || theme)
}

pub(crate) fn theme_mono_text_style(
    theme: DefaultTheme,
    token: ThemeTextToken,
    color: Color,
) -> TextStyle {
    TextStyle {
        font_size: token.size,
        line_height: token.line_height,
        ..theme.mono_text_style(color)
    }
}

pub(crate) fn paint_table_cell(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: &str,
    style: &TextStyle,
    align: TextAlign,
) {
    if rect.is_empty() {
        return;
    }

    ctx.push_clip_rect(rect);
    paint_text_line(
        ctx,
        Rect::new(
            rect.x() + 8.0,
            rect.y(),
            (rect.width() - 16.0).max(0.0),
            rect.height(),
        ),
        text,
        style,
        align,
    );
    ctx.pop_clip();
}

pub(crate) const GALLERY_TEXT_MAX_WIDTH: f32 = 980.0;
pub(crate) const GALLERY_CONTENT_MAX_WIDTH: f32 = 1180.0;

#[derive(Debug, Clone, Copy)]
pub(crate) enum DemoTextColor {
    Text,
    Muted,
}

pub(crate) fn demo_label(
    theme_reader: &DevThemeReader,
    text: impl Into<String>,
    role: DemoTextRole,
    color: DemoTextColor,
) -> Label {
    Label::new(text).text_style_when(demo_text_style_when(theme_reader, role, move |theme| {
        match color {
            DemoTextColor::Text => theme.palette.text,
            DemoTextColor::Muted => theme.palette.text_muted,
        }
    }))
}

pub(crate) fn demo_mono_label<F>(
    theme_reader: &DevThemeReader,
    text: impl Into<String>,
    role: DemoTextRole,
    color: F,
) -> Label
where
    F: Fn(DefaultTheme) -> Color + 'static,
{
    Label::new(text).text_style_when(demo_mono_text_style_when(theme_reader, role, color))
}

pub(crate) struct MinimumWidth {
    min_width: f32,
    child: SingleChild,
}

impl MinimumWidth {
    pub(crate) fn new<W>(min_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            min_width: min_width.max(0.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for MinimumWidth {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = constraints.max.width.max(self.min_width);
        let child_constraints = Constraints::new(
            Size::new(
                constraints.min.width.max(self.min_width).min(max_width),
                constraints.min.height,
            ),
            Size::new(max_width, constraints.max.height),
        );
        let child_size = self.child.measure(ctx, child_constraints);
        Size::new(child_size.width.max(self.min_width), child_size.height)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
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

pub(crate) struct MaximumWidth {
    max_width: f32,
    child: SingleChild,
}

impl MaximumWidth {
    pub(crate) fn new<W>(max_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            max_width: max_width.max(1.0),
            child: SingleChild::new(child),
        }
    }
}

impl Widget for MaximumWidth {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let max_width = if constraints.max.width.is_finite() {
            constraints.max.width.min(self.max_width)
        } else {
            self.max_width
        };
        let child_constraints = Constraints::new(
            Size::new(constraints.min.width.min(max_width), constraints.min.height),
            Size::new(max_width, constraints.max.height),
        );
        let child_size = self.child.measure(ctx, child_constraints);

        Size::new(
            child_size
                .width
                .min(max_width)
                .max(constraints.min.width.min(max_width)),
            child_size
                .height
                .clamp(constraints.min.height, constraints.max.height),
        )
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(
            ctx,
            Rect::from_origin_size(bounds.origin, self.child.child().measured_size()),
        );
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

pub(crate) struct VerticalScrollPane {
    spacing: f32,
    content: SingleChild,
    scroll_bar: SingleChild,
}

impl VerticalScrollPane {
    pub(crate) fn new<W, S>(content: W, scroll_bar: S) -> Self
    where
        W: Widget + 'static,
        S: Widget + 'static,
    {
        Self {
            spacing: 0.0,
            content: SingleChild::new(content),
            scroll_bar: SingleChild::new(scroll_bar),
        }
    }
}

impl Widget for VerticalScrollPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let scroll_bar_size = self.scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
        );
        let content_constraints = Constraints::new(
            Size::new(
                (constraints.min.width - scroll_bar_size.width - self.spacing).max(0.0),
                constraints.min.height,
            ),
            Size::new(
                (constraints.max.width - scroll_bar_size.width - self.spacing).max(0.0),
                constraints.max.height,
            ),
        );
        let content_size = self.content.measure(ctx, content_constraints);

        constraints.clamp(Size::new(
            content_size.width + scroll_bar_size.width + self.spacing,
            content_size.height.max(scroll_bar_size.height),
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let scroll_bar_size = self.scroll_bar.child().measured_size();
        let content_width = (bounds.width() - scroll_bar_size.width - self.spacing).max(0.0);
        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), content_width, bounds.height()),
        );
        self.scroll_bar.arrange(
            ctx,
            Rect::new(
                bounds.max_x() - scroll_bar_size.width,
                bounds.y(),
                scroll_bar_size.width,
                bounds.height(),
            ),
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        self.scroll_bar.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        self.scroll_bar.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.scroll_bar.visit_children_mut(visitor);
    }
}

pub(crate) struct TwoAxisScrollPane {
    spacing: f32,
    state: ScrollState,
    show_vertical_scroll_bar: bool,
    show_horizontal_scroll_bar: bool,
    content: SingleChild,
    vertical_scroll_bar: SingleChild,
    horizontal_scroll_bar: SingleChild,
}

impl TwoAxisScrollPane {
    pub(crate) fn new<W, V, H>(
        state: ScrollState,
        content: W,
        vertical_scroll_bar: V,
        horizontal_scroll_bar: H,
    ) -> Self
    where
        W: Widget + 'static,
        V: Widget + 'static,
        H: Widget + 'static,
    {
        Self {
            spacing: 0.0,
            state,
            show_vertical_scroll_bar: true,
            show_horizontal_scroll_bar: true,
            content: SingleChild::new(content),
            vertical_scroll_bar: SingleChild::new(vertical_scroll_bar),
            horizontal_scroll_bar: SingleChild::new(horizontal_scroll_bar),
        }
    }

    pub(crate) fn viewport_size(&self, bounds: Size) -> Size {
        let vertical_size = self.vertical_scroll_bar.child().measured_size();
        let horizontal_size = self.horizontal_scroll_bar.child().measured_size();
        let vertical_extent = if self.show_vertical_scroll_bar {
            vertical_size.width + self.spacing
        } else {
            0.0
        };
        let horizontal_extent = if self.show_horizontal_scroll_bar {
            horizontal_size.height + self.spacing
        } else {
            0.0
        };
        Size::new(
            (bounds.width - vertical_extent).max(0.0),
            (bounds.height - horizontal_extent).max(0.0),
        )
    }

    pub(crate) fn content_constraints(
        constraints: Constraints,
        vertical_size: Size,
        horizontal_size: Size,
        show_vertical_scroll_bar: bool,
        show_horizontal_scroll_bar: bool,
        spacing: f32,
    ) -> Constraints {
        let vertical_extent = if show_vertical_scroll_bar {
            vertical_size.width + spacing
        } else {
            0.0
        };
        let horizontal_extent = if show_horizontal_scroll_bar {
            horizontal_size.height + spacing
        } else {
            0.0
        };
        Constraints::new(
            Size::new(
                (constraints.min.width - vertical_extent).max(0.0),
                (constraints.min.height - horizontal_extent).max(0.0),
            ),
            Size::new(
                (constraints.max.width - vertical_extent).max(0.0),
                (constraints.max.height - horizontal_extent).max(0.0),
            ),
        )
    }

    pub(crate) fn scroll_bar_visibility(&self) -> (bool, bool) {
        let viewport = self.state.viewport_size();
        let content = self.state.content_size();
        (
            content.width > viewport.width + 0.001,
            content.height > viewport.height + 0.001,
        )
    }
}

impl Widget for TwoAxisScrollPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let vertical_size = self.vertical_scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(f32::INFINITY, constraints.max.height)),
        );
        let horizontal_size = self.horizontal_scroll_bar.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(constraints.max.width, f32::INFINITY)),
        );
        let mut show_vertical = false;
        let mut show_horizontal = false;
        let mut content_size = self.content.measure(
            ctx,
            Self::content_constraints(
                constraints,
                vertical_size,
                horizontal_size,
                show_vertical,
                show_horizontal,
                self.spacing,
            ),
        );
        for _ in 0..3 {
            let (next_horizontal, next_vertical) = self.scroll_bar_visibility();
            if next_vertical == show_vertical && next_horizontal == show_horizontal {
                break;
            }
            show_vertical = next_vertical;
            show_horizontal = next_horizontal;
            content_size = self.content.measure(
                ctx,
                Self::content_constraints(
                    constraints,
                    vertical_size,
                    horizontal_size,
                    show_vertical,
                    show_horizontal,
                    self.spacing,
                ),
            );
        }

        self.show_vertical_scroll_bar = show_vertical;
        self.show_horizontal_scroll_bar = show_horizontal;
        let vertical_extent = if show_vertical {
            vertical_size.width + self.spacing
        } else {
            0.0
        };
        let horizontal_extent = if show_horizontal {
            horizontal_size.height + self.spacing
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            content_size.width + vertical_extent,
            content_size.height + horizontal_extent,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let viewport = self.viewport_size(bounds.size);
        self.content.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), viewport.width, viewport.height),
        );
        self.vertical_scroll_bar.arrange(
            ctx,
            if self.show_vertical_scroll_bar {
                Rect::new(
                    bounds.x() + viewport.width + self.spacing,
                    bounds.y(),
                    self.vertical_scroll_bar.child().measured_size().width,
                    viewport.height,
                )
            } else {
                Rect::new(bounds.max_x(), bounds.y(), 0.0, 0.0)
            },
        );
        self.horizontal_scroll_bar.arrange(
            ctx,
            if self.show_horizontal_scroll_bar {
                Rect::new(
                    bounds.x(),
                    bounds.y() + viewport.height + self.spacing,
                    viewport.width,
                    self.horizontal_scroll_bar.child().measured_size().height,
                )
            } else {
                Rect::new(bounds.x(), bounds.max_y(), 0.0, 0.0)
            },
        );
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
        if self.show_vertical_scroll_bar {
            self.vertical_scroll_bar.paint(ctx);
        }
        if self.show_horizontal_scroll_bar {
            self.horizontal_scroll_bar.paint(ctx);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.content.semantics(ctx);
        if self.show_vertical_scroll_bar {
            self.vertical_scroll_bar.semantics(ctx);
        }
        if self.show_horizontal_scroll_bar {
            self.horizontal_scroll_bar.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
        self.vertical_scroll_bar.visit_children(visitor);
        self.horizontal_scroll_bar.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
        self.vertical_scroll_bar.visit_children_mut(visitor);
        self.horizontal_scroll_bar.visit_children_mut(visitor);
    }
}

pub(crate) fn panel<W>(title: &str, subtitle: &str, body: W) -> impl Widget
where
    W: Widget + 'static,
{
    panel_with_theme(default_theme_reader(), title, subtitle, body)
}

pub(crate) fn panel_with_theme<W>(
    theme_reader: DevThemeReader,
    title: &str,
    subtitle: &str,
    body: W,
) -> impl Widget
where
    W: Widget + 'static,
{
    CenteredContentWidth::new(
        GALLERY_CONTENT_MAX_WIDTH,
        Background::new(
            theme_reader().palette.surface,
            Padding::all(
                18.0,
                Stack::vertical()
                    .gap(10.0)
                    .alignment(Alignment::Stretch)
                    .with_child(MaximumWidth::new(
                        GALLERY_TEXT_MAX_WIDTH,
                        demo_label(
                            &theme_reader,
                            title,
                            DemoTextRole::SectionTitle,
                            DemoTextColor::Text,
                        ),
                    ))
                    .with_child(MaximumWidth::new(
                        GALLERY_TEXT_MAX_WIDTH,
                        demo_label(
                            &theme_reader,
                            subtitle,
                            DemoTextRole::Supporting,
                            DemoTextColor::Muted,
                        ),
                    ))
                    .with_child(body),
            ),
        )
        .brush_when(dev_theme_color(&theme_reader, |theme| {
            theme.palette.surface
        })),
    )
}

pub(crate) struct CenteredContentWidth {
    max_width: f32,
    content_width: f32,
    child: SingleChild,
}

impl CenteredContentWidth {
    pub(crate) fn new<W>(max_width: f32, child: W) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            max_width: max_width.max(1.0),
            content_width: 0.0,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for CenteredContentWidth {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let outer_width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            self.max_width.max(constraints.min.width)
        };
        self.content_width = outer_width.min(self.max_width).max(0.0);
        let child_size = self.child.measure(
            ctx,
            Constraints::new(
                Size::new(self.content_width, 0.0),
                Size::new(self.content_width, constraints.max.height),
            ),
        );
        constraints.clamp(Size::new(outer_width, child_size.height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let measured = self.child.child().measured_size();
        let content_width = self.content_width.min(bounds.width());
        self.child.arrange(
            ctx,
            Rect::new(
                bounds.x() + ((bounds.width() - content_width) * 0.5).max(0.0),
                bounds.y(),
                content_width,
                measured.height.min(bounds.height()),
            ),
        );
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

pub(crate) struct NamedSection {
    name: String,
    content: SingleChild,
}

impl NamedSection {
    pub(crate) fn new(name: impl Into<String>, content: impl Widget + 'static) -> Self {
        Self {
            name: name.into(),
            content: SingleChild::new(content),
        }
    }
}

impl Widget for NamedSection {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.content.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.content.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.content.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        ctx.push(node);
        self.content.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.content.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.content.visit_children_mut(visitor);
    }
}
