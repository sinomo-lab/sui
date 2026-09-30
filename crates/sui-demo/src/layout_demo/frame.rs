//! The frame the examples are shown in, and the size readouts drawn over
//! them.

use std::rc::Rc;

use sui::{
    EventPhase, PointerButton, PointerEventKind, Rect, SemanticsNode, SemanticsRole,
    SemanticsValue, Signal, TextStyle, WidgetPodMutVisitor, WidgetPodVisitor, paint_text_line,
    prelude::*,
};

use crate::app::{DemoTextRole, DevThemeReader, demo_text_style};

pub(super) const MIN_FRAME_WIDTH: f32 = 280.0;
pub(super) const MAX_FRAME_WIDTH: f32 = 1280.0;
/// The strip right of a frame that drags its width.
pub(super) const HANDLE_WIDTH: f32 = 16.0;
pub(crate) const EXAMPLE_FRAME_NAME: &str = "Example frame";

/// The width asked for with the slider, presets or a frame's handle, and
/// the width the frames have room to show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FrameWidth {
    pub(super) requested: f32,
    pub(super) shown: f32,
}

impl FrameWidth {
    pub(super) fn new(requested: f32) -> Self {
        Self {
            requested,
            shown: requested,
        }
    }

    /// "768 px", or "1180 px, 940 px fit" when the page is narrower.
    pub(super) fn readout(self) -> String {
        if self.shown + 0.5 < self.requested {
            format!(
                "{} px, {} px fit",
                self.requested.round(),
                self.shown.round()
            )
        } else {
            format!("{} px", self.requested.round())
        }
    }
}

/// Shows `child` at the chosen width, as far as the page has room, with a
/// handle on its right edge that drags the width.
pub(super) struct DeviceFrame {
    theme_reader: DevThemeReader,
    width: Signal<FrameWidth>,
    drag: Option<HandleDrag>,
    handle_hovered: bool,
    child: SingleChild,
}

#[derive(Debug, Clone, Copy)]
struct HandleDrag {
    pointer_id: u64,
    start_x: f32,
    start_width: f32,
}

impl DeviceFrame {
    pub(super) fn new<W>(
        theme_reader: &DevThemeReader,
        width: &Signal<FrameWidth>,
        child: W,
    ) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            width: width.clone(),
            drag: None,
            handle_hovered: false,
            child: SingleChild::new(child),
        }
    }

    /// Where the child shows within the frame's `bounds`, which a parent
    /// may have stretched past the width the frame asked for.
    fn content_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x(),
            bounds.y(),
            self.child.child().measured_size().width,
            bounds.height(),
        )
    }

    fn handle_rect(&self, bounds: Rect) -> Rect {
        let content = self.content_rect(bounds);
        Rect::new(content.max_x(), content.y(), HANDLE_WIDTH, content.height())
    }

    fn set_handle_hovered(&mut self, ctx: &mut EventCtx, hovered: bool) {
        if self.handle_hovered != hovered {
            self.handle_hovered = hovered;
            ctx.request_paint();
        }
    }
}

impl Widget for DeviceFrame {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let Event::Pointer(pointer) = event else {
            return;
        };
        if ctx.phase() == EventPhase::Capture {
            return;
        }
        let on_handle = self.handle_rect(ctx.bounds()).contains(pointer.position);
        match pointer.kind {
            PointerEventKind::Down
                if pointer.button == Some(PointerButton::Primary) && on_handle =>
            {
                self.drag = Some(HandleDrag {
                    pointer_id: pointer.pointer_id,
                    start_x: pointer.position.x,
                    start_width: self.content_rect(ctx.bounds()).width(),
                });
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.set_handled();
            }
            PointerEventKind::Move => {
                if let Some(drag) = self
                    .drag
                    .filter(|drag| drag.pointer_id == pointer.pointer_id)
                {
                    let width = (drag.start_width + pointer.position.x - drag.start_x)
                        .clamp(MIN_FRAME_WIDTH, MAX_FRAME_WIDTH)
                        .round();
                    self.width.update(|frame| frame.requested = width);
                    ctx.set_handled();
                } else {
                    self.set_handle_hovered(ctx, on_handle);
                }
            }
            PointerEventKind::Up | PointerEventKind::Cancel
                if self
                    .drag
                    .is_some_and(|drag| drag.pointer_id == pointer.pointer_id) =>
            {
                self.drag = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                self.set_handle_hovered(ctx, on_handle);
                ctx.request_paint();
                ctx.set_handled();
            }
            PointerEventKind::Leave if self.drag.is_none() => {
                self.set_handle_hovered(ctx, false);
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let requested = ctx.observe(&self.width).requested;
        let room = (constraints.max.width - HANDLE_WIDTH).max(0.0);
        let width = requested.min(room);
        let child = self.child.measure(
            ctx,
            Constraints::new(
                Size::new(width, 0.0),
                Size::new(width, constraints.max.height),
            ),
        );
        constraints.clamp(Size::new(width + HANDLE_WIDTH, child.height))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let width = self.child.child().measured_size().width;
        self.child.arrange(
            ctx,
            Rect::new(bounds.x(), bounds.y(), width, bounds.height()),
        );
        self.width.update(|frame| frame.shown = width.round());
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let bounds = ctx.bounds();
        self.child.paint(ctx);
        ctx.stroke(
            Path::rounded_rect(self.content_rect(bounds).inflate(0.5, 0.5), 10.0),
            theme.palette.border,
            StrokeStyle::new(1.0),
        );
        let handle = self.handle_rect(bounds);
        let grip = Rect::new(
            handle.x() + (HANDLE_WIDTH - 4.0) * 0.5,
            handle.y() + (handle.height() - 40.0).max(0.0) * 0.5,
            4.0,
            handle.height().min(40.0),
        );
        let active = self.drag.is_some() || self.handle_hovered;
        ctx.fill(
            Path::rounded_rect(grip, 2.0),
            if active {
                theme.palette.accent
            } else {
                theme.palette.border
            },
        );
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let content = self.content_rect(ctx.bounds());
        let mut node =
            SemanticsNode::new(ctx.widget_id(), SemanticsRole::GenericContainer, content);
        node.name = Some(EXAMPLE_FRAME_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "{} px wide",
            content.width().round()
        )));
        ctx.push(node);
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

/// When a [`SizeTag`] shows its size.
#[derive(Clone)]
pub(super) enum ShowSize {
    Always,
    When(Signal<bool>),
}

/// Labels `child` with its width, as "Sidebar · 212 px", in a tag over its
/// top-left corner.
pub(super) struct SizeTag {
    theme_reader: DevThemeReader,
    label: String,
    show: ShowSize,
    child: SingleChild,
}

impl SizeTag {
    pub(super) fn new<W>(
        theme_reader: &DevThemeReader,
        label: impl Into<String>,
        show: ShowSize,
        child: W,
    ) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            theme_reader: Rc::clone(theme_reader),
            label: label.into(),
            show,
            child: SingleChild::new(child),
        }
    }

    fn text(&self, width: f32) -> String {
        if self.label.is_empty() {
            format!("{} px", width.round())
        } else {
            format!("{} · {} px", self.label, width.round())
        }
    }
}

impl Widget for SizeTag {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        self.child.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.child.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.child.paint(ctx);
        let show = match &self.show {
            ShowSize::Always => true,
            ShowSize::When(show) => ctx.observe(show),
        };
        if show {
            let bounds = ctx.bounds();
            paint_tag(ctx, &self.theme_reader, bounds, &self.text(bounds.width()));
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = (!self.label.is_empty()).then(|| self.label.clone());
        node.value = Some(SemanticsValue::Text(self.text(ctx.bounds().width())));
        ctx.push(node);
        self.child.semantics(ctx);
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        self.child.visit_children(visitor);
    }

    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        self.child.visit_children_mut(visitor);
    }
}

fn tag_style(theme: DefaultTheme) -> TextStyle {
    demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text)
}

/// A small tag reading `text` in the top-left corner of `bounds`.
pub(super) fn paint_tag(
    ctx: &mut PaintCtx,
    theme_reader: &DevThemeReader,
    bounds: Rect,
    text: &str,
) {
    let theme = theme_reader();
    let style = tag_style(theme);
    let text_width = ctx.measure_text(text, style.clone()).map_or(
        text.chars().count() as f32 * style.font_size * 0.55,
        |measured| measured.width,
    );
    let tag = Rect::new(
        bounds.x() + 6.0,
        bounds.y() + 6.0,
        (text_width + 12.0).min((bounds.width() - 12.0).max(0.0)),
        style.line_height + 4.0,
    );
    if tag.width() <= 12.0 || tag.max_y() > bounds.max_y() {
        return;
    }
    ctx.fill(
        Path::rounded_rect(tag, 5.0),
        theme.palette.surface_raised.with_alpha(0.92),
    );
    ctx.push_clip_rect(tag);
    paint_text_line(
        ctx,
        Rect::new(
            tag.x() + 6.0,
            tag.y() + 2.0,
            tag.width() - 12.0,
            style.line_height,
        ),
        text,
        &style,
        TextAlign::Start,
    );
    ctx.pop_clip();
}

/// Reports the width it is offered to `on_width` whenever it changes, for
/// readouts that depend on how wide an example is.
pub(super) struct WidthProbe {
    on_width: Box<dyn FnMut(f32)>,
    last: Option<f32>,
    child: SingleChild,
}

impl WidthProbe {
    pub(super) fn new<W, F>(on_width: F, child: W) -> Self
    where
        W: Widget + 'static,
        F: FnMut(f32) + 'static,
    {
        Self {
            on_width: Box::new(on_width),
            last: None,
            child: SingleChild::new(child),
        }
    }
}

impl Widget for WidthProbe {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let width = constraints.max.width;
        if width.is_finite() && self.last != Some(width) {
            self.last = Some(width);
            (self.on_width)(width);
        }
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

/// Marks the widths the responsive examples change at, under a width slider
/// of the same width.
pub(super) struct BreakpointTicks {
    theme_reader: DevThemeReader,
    width: Signal<FrameWidth>,
    marks: &'static [(f32, &'static str)],
}

impl BreakpointTicks {
    pub(super) fn new(
        theme_reader: &DevThemeReader,
        width: &Signal<FrameWidth>,
        marks: &'static [(f32, &'static str)],
    ) -> Self {
        Self {
            theme_reader: Rc::clone(theme_reader),
            width: width.clone(),
            marks,
        }
    }
}

impl Widget for BreakpointTicks {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let theme = (self.theme_reader)();
        let width = if constraints.max.width.is_finite() {
            constraints.max.width
        } else {
            constraints.min.width
        };
        constraints.clamp(Size::new(width, 8.0 + tag_style(theme).line_height))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme_reader)();
        let requested = ctx.observe(&self.width).requested;
        let bounds = ctx.bounds();
        // The slider's track, which its padding insets from its edges.
        let padding = theme.metrics.slider_padding;
        let track_x = bounds.x() + padding.left;
        let track_width = (bounds.width() - padding.left - padding.right).max(0.0);
        let mut style = tag_style(theme);
        for &(width, _) in self.marks {
            let fraction = (width - MIN_FRAME_WIDTH) / (MAX_FRAME_WIDTH - MIN_FRAME_WIDTH);
            let x = track_x + track_width * fraction.clamp(0.0, 1.0);
            let color = if requested >= width {
                theme.palette.accent
            } else {
                theme.palette.text_muted
            };
            ctx.fill(
                Path::rounded_rect(Rect::new(x - 0.75, bounds.y(), 1.5, 6.0), 0.75),
                color,
            );
            style.color = color;
            paint_text_line(
                ctx,
                Rect::new(x - 30.0, bounds.y() + 7.0, 60.0, style.line_height),
                &format!("{width}"),
                &style,
                TextAlign::Center,
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some("Breakpoints".to_string());
        node.value = Some(SemanticsValue::Text(
            self.marks
                .iter()
                .map(|(width, what)| format!("{width} px: {what}"))
                .collect::<Vec<_>>()
                .join("; "),
        ));
        ctx.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readout_says_when_the_page_is_too_narrow() {
        assert_eq!(FrameWidth::new(768.0).readout(), "768 px");
        let squeezed = FrameWidth {
            requested: 1180.0,
            shown: 940.0,
        };
        assert_eq!(squeezed.readout(), "1180 px, 940 px fit");
    }
}
