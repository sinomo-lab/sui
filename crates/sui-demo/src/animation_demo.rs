use std::rc::Rc;

use sui::prelude::*;
use sui::{PointerEventKind, SemanticsNode, SemanticsRole, SemanticsValue, Vector};
use sui_runtime::{LayerOptions, PaintBoundaryMode};
use sui_scene::{LayerCompositionMode, LayerProperties};

use crate::app::{DevThemeReader, clone_dev_theme_reader, dev_theme_color};

pub(crate) const ANIMATION_DEMO_TAB_LABEL: &str = "Animation";
pub(crate) const ANIMATION_DEMO_SCROLL_NAME: &str = "Animation demo scroll";
pub(crate) const ANIMATION_DEMO_NAME: &str = "Animation system examples";
pub(crate) const ANIMATION_TIMELINE_PREVIEW_NAME: &str = "Timeline playback example";
pub(crate) const ANIMATION_RETAINED_LAYER_NAME: &str = "Retained layer animation example";
pub(crate) const ANIMATION_PAINT_INVALIDATION_NAME: &str = "Paint invalidation animation example";
pub(crate) const ANIMATION_EDITOR_SURFACE_NAME: &str = "Animation document editor example";
pub(crate) const ANIMATION_DEMO_BUTTON_LABEL: &str = "Preview transition";
pub(crate) const ANIMATION_DEMO_SWITCH_LABEL: &str = "Motion enabled";
pub(crate) const ANIMATION_DEMO_TEXT_INPUT_LABEL: &str = "Animation search";
pub(crate) const ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL: &str = "Animation timing";
pub(crate) const ANIMATION_DEMO_TOOLTIP_TEXT: &str =
    "Tooltip entry motion uses retained translation and opacity";
pub(crate) const ANIMATION_DEMO_POPOVER_NAME: &str = "Animation inspector";
pub(crate) const ANIMATION_DEMO_POPOVER_TRIGGER_LABEL: &str = "Open animation inspector";

const TIMELINE_TARGET: &str = "animation-demo-timeline";
const RETAINED_TARGET: &str = "animation-demo-retained";
const PAINT_TARGET: &str = "animation-demo-paint";
const TIMELINE_RADIUS_PATH: &str = "paint.radius";
const PAINT_RADIUS_PATH: &str = "paint.radius";
const PAINT_ALPHA_PATH: &str = "paint.alpha";

pub(crate) fn build_animation_demo_with_theme(theme_reader: DevThemeReader) -> impl Widget {
    Background::new(
        theme_reader().palette.surface,
        ScrollView::vertical(Padding::all(
            18.0,
            Stack::vertical()
                .spacing(18.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Stack::vertical()
                        .spacing(6.0)
                        .alignment(Alignment::Stretch)
                        .with_child(
                            Label::new(ANIMATION_DEMO_NAME)
                                .font_size(22.0)
                                .line_height(28.0)
                                .color_when(dev_theme_color(&theme_reader, |theme| {
                                    theme.palette.text
                                })),
                        )
                        .with_child(
                            Label::new(
                                "Timeline sampling, retained transforms, repaint invalidation, editor state, and overlay animation in one focused surface.",
                            )
                            .font_size(13.0)
                            .line_height(18.0)
                            .color_when(dev_theme_color(&theme_reader, |theme| {
                                theme.palette.text_muted
                            })),
                        ),
                )
                .with_child(section(
                    "Timeline playback",
                    "A single timeline drives layer opacity, layer translation, fill color, and a custom radius path.",
                    SizedBox::new()
                        .height(142.0)
                        .with_child(TimelinePlaybackExample::new()),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Invalidation paths",
                    "The same timeline player feeds either retained layer properties or paint-bound visual state.",
                    Flex::horizontal()
                        .gap(12.0)
                        .wrap(FlexWrap::Wrap)
                        .align_items(Alignment::Stretch)
                        .with_item(
                            SizedBox::new()
                                .height(132.0)
                                .with_child(RetainedLayerAnimationExample::new()),
                            FlexItem::new().basis_fraction(0.5).min_width(300.0),
                        )
                        .with_item(
                            SizedBox::new()
                                .height(132.0)
                                .with_child(PaintInvalidationAnimationExample::new()),
                            FlexItem::new().basis_fraction(0.5).min_width(300.0),
                        ),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Animation document",
                    "Timeline, clip, track, keyframe selection, and easing data rendered from AnimationEditorState.",
                    SizedBox::new()
                        .height(310.0)
                        .with_child(AnimationDocumentEditorExample::new()),
                    Rc::clone(&theme_reader),
                ))
                .with_child(section(
                    "Controls and overlays",
                    "Built-in widgets exercise hover, focus, tooltip, and popover motion around the custom animation examples.",
                    controls_and_overlays(Rc::clone(&theme_reader)),
                    Rc::clone(&theme_reader),
                )),
        ))
        .name(ANIMATION_DEMO_SCROLL_NAME),
    )
    .brush_when(dev_theme_color(&theme_reader, |theme| {
        theme.palette.surface
    }))
}

fn section<W>(title: &str, description: &str, body: W, theme_reader: DevThemeReader) -> impl Widget
where
    W: Widget + 'static,
{
    Stack::vertical()
        .spacing(8.0)
        .alignment(Alignment::Stretch)
        .with_child(
            Label::new(title)
                .font_size(18.0)
                .line_height(22.0)
                .color_when(dev_theme_color(&theme_reader, |theme| theme.palette.text)),
        )
        .with_child(
            Label::new(description)
                .font_size(13.0)
                .line_height(18.0)
                .color_when(dev_theme_color(&theme_reader, |theme| {
                    theme.palette.text_muted
                })),
        )
        .with_child(body)
        .with_child(
            Separator::horizontal()
                .theme_when(clone_dev_theme_reader(&theme_reader))
                .inset(0.0),
        )
}

fn controls_and_overlays(theme_reader: DevThemeReader) -> impl Widget {
    Flex::horizontal()
        .gap(12.0)
        .wrap(FlexWrap::Wrap)
        .align_items(Alignment::Start)
        .with_item(
            Stack::vertical()
                .spacing(12.0)
                .alignment(Alignment::Start)
                .with_child(
                    Button::new(ANIMATION_DEMO_BUTTON_LABEL)
                        .min_width(220.0)
                        .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    Switch::new(ANIMATION_DEMO_SWITCH_LABEL)
                        .on(true)
                        .theme_when(clone_dev_theme_reader(&theme_reader)),
                )
                .with_child(
                    SizedBox::new().width(320.0).with_child(
                        TextInput::new(ANIMATION_DEMO_TEXT_INPUT_LABEL)
                            .value("Layer opacity")
                            .placeholder("Search animation property")
                            .theme_when(clone_dev_theme_reader(&theme_reader)),
                    ),
                ),
            FlexItem::new().basis_fraction(0.38).min_width(300.0),
        )
        .with_item(
            Stack::vertical()
                .spacing(12.0)
                .alignment(Alignment::Start)
                .with_child(
                    SizedBox::new().width(260.0).with_child(Tooltip::new(
                        ANIMATION_DEMO_TOOLTIP_TEXT,
                        Button::new(ANIMATION_DEMO_TOOLTIP_TRIGGER_LABEL)
                            .min_width(220.0)
                            .theme_when(clone_dev_theme_reader(&theme_reader)),
                    )),
                )
                .with_child(
                    SizedBox::new().width(360.0).with_child(Popover::new(
                        ANIMATION_DEMO_POPOVER_NAME,
                        Button::new(ANIMATION_DEMO_POPOVER_TRIGGER_LABEL)
                            .min_width(230.0)
                            .theme_when(clone_dev_theme_reader(&theme_reader)),
                        Stack::vertical()
                            .spacing(8.0)
                            .alignment(Alignment::Stretch)
                            .with_child(
                                Label::new(
                                    "Tracks: opacity, translation, fill color, custom radius",
                                )
                                .font_size(13.0)
                                .line_height(18.0)
                                .color_when(dev_theme_color(&theme_reader, |theme| {
                                    theme.palette.text
                                })),
                            )
                            .with_child(
                                Label::new("Invalidation: transform, effect, and paint")
                                    .font_size(12.0)
                                    .line_height(17.0)
                                    .color_when(dev_theme_color(&theme_reader, |theme| {
                                        theme.palette.text_muted
                                    })),
                            ),
                    )),
                ),
            FlexItem::new().basis_fraction(0.62).min_width(360.0),
        )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TimelineExamplePresentation {
    opacity: f32,
    translation: Vector,
    fill: Color,
    radius: f32,
}

impl Default for TimelineExamplePresentation {
    fn default() -> Self {
        Self {
            opacity: 0.42,
            translation: Vector::new(-30.0, 0.0),
            fill: Color::rgba(0.20, 0.45, 0.95, 1.0),
            radius: 14.0,
        }
    }
}

impl TimelineBindingSink for TimelineExamplePresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != TIMELINE_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.0, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
                self.opacity = value;
                changed
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                let changed = self.translation != value;
                self.translation = value;
                changed
            }
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = self.fill != value;
                self.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == TIMELINE_RADIUS_PATH =>
            {
                let value = value.max(4.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            _ => false,
        }
    }
}

struct TimelinePlaybackExample {
    player: TimelinePlayer,
    presentation: TimelineExamplePresentation,
}

impl TimelinePlaybackExample {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(timeline_playback_example_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        player.play();
        let mut presentation = TimelineExamplePresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
        }
        ctx.request_animation_frame();
    }
}

impl Widget for TimelinePlaybackExample {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if matches!(
                    pointer.kind,
                    PointerEventKind::Enter | PointerEventKind::Move | PointerEventKind::Down
                ) && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.request_paint();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.player.playback().playing {
            ctx.request_animation_frame();
        }
        constraints.clamp(Size::new(680.0, 142.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        paint_demo_surface(ctx, bounds, Color::rgba(0.065, 0.075, 0.10, 1.0));
        draw_demo_label(
            ctx,
            Rect::new(bounds.x() + 18.0, bounds.y() + 14.0, 260.0, 22.0),
            "Timeline player",
            13.0,
            Color::rgba(0.90, 0.94, 1.0, 1.0),
        );

        let rail = Rect::new(
            bounds.x() + 42.0,
            bounds.y() + bounds.height() * 0.56 - 3.0,
            bounds.width() - 84.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            Color::rgba(0.32, 0.36, 0.43, 0.55),
        );

        let center = Point::new(
            bounds.x() + bounds.width() * 0.5,
            bounds.y() + bounds.height() * 0.56,
        );
        ctx.fill(
            Path::circle(center, self.presentation.radius),
            self.presentation.fill.with_alpha(self.presentation.opacity),
        );
        ctx.stroke(
            Path::circle(center, self.presentation.radius + 5.0),
            Color::rgba(1.0, 1.0, 1.0, 0.35 * self.presentation.opacity),
            StrokeStyle::new(1.5),
        );

        let meter = Rect::new(
            bounds.x() + 42.0,
            bounds.max_y() - 24.0,
            (bounds.width() - 84.0) * self.presentation.opacity,
            5.0,
        );
        ctx.fill(
            Path::rounded_rect(meter, 2.5),
            Color::rgba(0.72, 0.86, 1.0, 0.88),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
            .with_opacity(self.presentation.opacity)
            .with_translation(self.presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_TIMELINE_PREVIEW_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "opacity {:.2}, radius {:.1}, x {:.1}",
            self.presentation.opacity, self.presentation.radius, self.presentation.translation.x
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RetainedLayerPresentation {
    opacity: f32,
    translation: Vector,
}

impl Default for RetainedLayerPresentation {
    fn default() -> Self {
        Self {
            opacity: 0.70,
            translation: Vector::new(-26.0, 0.0),
        }
    }
}

impl TimelineBindingSink for RetainedLayerPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != RETAINED_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::LayerOpacity, AnimationValue::Scalar(value)) => {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.opacity - value).abs() > 0.001;
                self.opacity = value;
                changed
            }
            (AnimationProperty::LayerTranslation, AnimationValue::Vector(value)) => {
                let changed = self.translation != value;
                self.translation = value;
                changed
            }
            _ => false,
        }
    }
}

struct RetainedLayerAnimationExample {
    player: TimelinePlayer,
    presentation: RetainedLayerPresentation,
}

impl RetainedLayerAnimationExample {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(retained_layer_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        player.play();
        let mut presentation = RetainedLayerPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
        }
        ctx.request_animation_frame();
    }
}

impl Widget for RetainedLayerAnimationExample {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.player.playback().playing {
            ctx.request_animation_frame();
        }
        constraints.clamp(Size::new(300.0, 132.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        paint_demo_surface(ctx, bounds, Color::rgba(0.070, 0.088, 0.105, 1.0));
        draw_demo_label(
            ctx,
            Rect::new(
                bounds.x() + 14.0,
                bounds.y() + 12.0,
                bounds.width() - 28.0,
                20.0,
            ),
            "Retained transform/effect",
            12.0,
            Color::rgba(0.90, 0.95, 1.0, 1.0),
        );

        let rail = Rect::new(
            bounds.x() + 30.0,
            bounds.y() + bounds.height() * 0.58 - 3.0,
            bounds.width() - 60.0,
            6.0,
        );
        ctx.fill(
            Path::rounded_rect(rail, 3.0),
            Color::rgba(0.38, 0.48, 0.56, 0.40),
        );

        let marker = Rect::new(
            bounds.x() + bounds.width() * 0.5 - 34.0,
            bounds.y() + 58.0,
            68.0,
            34.0,
        );
        ctx.fill(
            Path::rounded_rect(marker, 7.0),
            Color::rgba(0.34, 0.72, 0.88, 0.88),
        );
        ctx.stroke_rect(
            marker,
            Color::rgba(0.86, 0.96, 1.0, 0.78),
            StrokeStyle::new(1.0),
        );
    }

    fn layer_options(&self) -> LayerOptions {
        LayerOptions {
            paint_boundary: PaintBoundaryMode::Explicit,
            composition_mode: LayerCompositionMode::Normal,
        }
    }

    fn layer_properties(&self) -> LayerProperties {
        LayerProperties::default()
            .with_opacity(self.presentation.opacity)
            .with_translation(self.presentation.translation)
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_RETAINED_LAYER_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "opacity {:.2}, x {:.1}",
            self.presentation.opacity, self.presentation.translation.x
        )));
        ctx.push(node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PaintInvalidationPresentation {
    fill: Color,
    radius: f32,
    alpha: f32,
}

impl Default for PaintInvalidationPresentation {
    fn default() -> Self {
        Self {
            fill: Color::rgba(0.82, 0.33, 0.24, 1.0),
            radius: 18.0,
            alpha: 0.76,
        }
    }
}

impl TimelineBindingSink for PaintInvalidationPresentation {
    fn apply_animation_value(&mut self, binding: &AnimationBinding, value: AnimationValue) -> bool {
        if binding.target.as_str() != PAINT_TARGET {
            return false;
        }

        match (&binding.property, value) {
            (AnimationProperty::FillColor, AnimationValue::Color(value)) => {
                let changed = self.fill != value;
                self.fill = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == PAINT_RADIUS_PATH =>
            {
                let value = value.max(3.0);
                let changed = (self.radius - value).abs() > 0.001;
                self.radius = value;
                changed
            }
            (AnimationProperty::Custom(path), AnimationValue::Scalar(value))
                if path.as_str() == PAINT_ALPHA_PATH =>
            {
                let value = value.clamp(0.25, 1.0);
                let changed = (self.alpha - value).abs() > 0.001;
                self.alpha = value;
                changed
            }
            _ => false,
        }
    }
}

struct PaintInvalidationAnimationExample {
    player: TimelinePlayer,
    presentation: PaintInvalidationPresentation,
}

impl PaintInvalidationAnimationExample {
    fn new() -> Self {
        let mut player = TimelinePlayer::new(paint_invalidation_timeline());
        player.playback_mut().loop_mode = LoopMode::Repeat;
        player.play();
        let mut presentation = PaintInvalidationPresentation::default();
        for sample in player.sample_reusing_scratch() {
            presentation.apply_animation_value(&sample.binding, sample.value);
        }
        Self {
            player,
            presentation,
        }
    }

    fn start(&mut self, ctx: &mut EventCtx) {
        if !self.player.playback().playing {
            self.player.play();
        }
        ctx.request_animation_frame();
    }
}

impl Widget for PaintInvalidationAnimationExample {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && ctx.bounds().contains(pointer.position) =>
            {
                self.start(ctx);
                ctx.set_handled();
            }
            Event::Wake(WakeEvent::AnimationFrame { delta, .. }) => {
                let tick = self.player.tick(*delta, &mut self.presentation);
                tick.request_current_widget_invalidations(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        if self.player.playback().playing {
            ctx.request_animation_frame();
        }
        constraints.clamp(Size::new(300.0, 132.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        paint_demo_surface(ctx, bounds, Color::rgba(0.105, 0.082, 0.072, 1.0));
        draw_demo_label(
            ctx,
            Rect::new(
                bounds.x() + 14.0,
                bounds.y() + 12.0,
                bounds.width() - 28.0,
                20.0,
            ),
            "Paint invalidation",
            12.0,
            Color::rgba(1.0, 0.94, 0.88, 1.0),
        );

        let lanes = 7;
        for lane in 0..lanes {
            let t = lane as f32 / (lanes - 1) as f32;
            let x = bounds.x() + 36.0 + t * (bounds.width() - 72.0);
            let y = bounds.y() + 72.0 + (t - 0.5).sin() * 5.0;
            let radius = (self.presentation.radius * (0.58 + 0.42 * t)).max(4.0);
            ctx.fill(
                Path::circle(Point::new(x, y), radius),
                self.presentation
                    .fill
                    .with_alpha(self.presentation.alpha * (0.56 + 0.40 * t)),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_PAINT_INVALIDATION_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "alpha {:.2}, radius {:.1}",
            self.presentation.alpha, self.presentation.radius
        )));
        ctx.push(node);
    }
}

struct AnimationDocumentEditorExample {
    editor: AnimationEditorState,
}

impl AnimationDocumentEditorExample {
    fn new() -> Self {
        let mut editor = AnimationEditorState::new(AnimationDocument::new(
            "Animation system example",
            timeline_playback_example_timeline(),
        ));
        editor.apply_command(AnimationEditorCommand::SelectKeyframe(KeyframeSelection {
            clip_index: 0,
            track_index: 0,
            keyframe_index: 1,
        }));
        Self { editor }
    }
}

impl Widget for AnimationDocumentEditorExample {
    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(680.0, 310.0))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let bounds = ctx.bounds();
        paint_demo_surface(ctx, bounds, Color::rgba(0.060, 0.070, 0.092, 1.0));

        draw_demo_label(
            ctx,
            Rect::new(bounds.x() + 16.0, bounds.y() + 14.0, 230.0, 22.0),
            "AnimationDocument",
            13.0,
            Color::rgba(0.90, 0.94, 1.0, 1.0),
        );
        draw_demo_label(
            ctx,
            Rect::new(bounds.max_x() - 176.0, bounds.y() + 14.0, 150.0, 22.0),
            "duration 1.80s",
            11.0,
            Color::rgba(0.72, 0.78, 0.88, 1.0),
        );

        let timeline = Rect::new(
            bounds.x() + 16.0,
            bounds.y() + 46.0,
            bounds.width() * 0.62,
            bounds.height() - 66.0,
        );
        let inspector = Rect::new(
            timeline.max_x() + 12.0,
            timeline.y(),
            bounds.max_x() - timeline.max_x() - 28.0,
            96.0,
        );
        let curve = Rect::new(
            inspector.x(),
            inspector.max_y() + 12.0,
            inspector.width(),
            bounds.max_y() - inspector.max_y() - 28.0,
        );

        self.paint_tracks(ctx, timeline);
        self.paint_inspector(ctx, inspector);
        self.paint_curve(ctx, curve);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let track_count = self
            .editor
            .document
            .timeline
            .clips
            .first()
            .map(|clip| clip.tracks.len())
            .unwrap_or_default();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(ANIMATION_EDITOR_SURFACE_NAME.to_string());
        node.value = Some(SemanticsValue::Text(format!(
            "tracks {track_count}, selected keyframes {}",
            self.editor.selection.keyframes.len()
        )));
        ctx.push(node);
    }
}

impl AnimationDocumentEditorExample {
    fn paint_tracks(&self, ctx: &mut PaintCtx, rect: Rect) {
        ctx.fill(
            Path::rounded_rect(rect, 6.0),
            Color::rgba(0.092, 0.105, 0.132, 1.0),
        );
        draw_demo_label(
            ctx,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, 160.0, 18.0),
            "Clip tracks",
            11.0,
            Color::rgba(0.78, 0.84, 0.92, 1.0),
        );

        let Some(clip) = self.editor.document.timeline.clips.first() else {
            return;
        };
        let lane_area = Rect::new(
            rect.x() + 14.0,
            rect.y() + 34.0,
            rect.width() - 28.0,
            rect.height() - 48.0,
        );
        let lane_height = 28.0;
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let y = lane_area.y() + track_index as f32 * (lane_height + 8.0);
            let lane = Rect::new(lane_area.x(), y, lane_area.width(), lane_height);
            ctx.fill(
                Path::rounded_rect(lane, 4.0),
                Color::rgba(0.13, 0.15, 0.19, 1.0),
            );
            draw_demo_label(
                ctx,
                Rect::new(lane.x() + 8.0, lane.y() + 5.0, 138.0, 16.0),
                track.binding.property.path(),
                10.0,
                Color::rgba(0.72, 0.76, 0.84, 1.0),
            );
        }

        for (selection, keyframe, hit) in animation_editor_keyframes(&self.editor, rect) {
            let selected = self.editor.selection.keyframes.contains(&selection);
            let center = Point::new(hit.x() + hit.width() * 0.5, hit.y() + hit.height() * 0.5);
            ctx.fill(
                Path::circle(center, if selected { 6.5 } else { 4.8 }),
                if selected {
                    Color::rgba(0.90, 0.72, 0.28, 1.0)
                } else {
                    Color::rgba(0.46, 0.68, 1.0, 1.0)
                },
            );
            if selected {
                draw_demo_label(
                    ctx,
                    Rect::new(center.x + 9.0, center.y - 9.0, 70.0, 18.0),
                    format!("{:.1}s", keyframe.time),
                    9.0,
                    Color::rgba(0.96, 0.84, 0.42, 1.0),
                );
            }
        }
    }

    fn paint_inspector(&self, ctx: &mut PaintCtx, rect: Rect) {
        ctx.fill(
            Path::rounded_rect(rect, 6.0),
            Color::rgba(0.105, 0.122, 0.155, 1.0),
        );
        draw_demo_label(
            ctx,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, rect.width() - 20.0, 18.0),
            "Selected keyframe",
            11.0,
            Color::rgba(0.84, 0.88, 0.94, 1.0),
        );
        let detail = self
            .editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe_detail(&self.editor, *selection))
            .unwrap_or_else(|| "No keyframe selected".to_string());
        draw_demo_label(
            ctx,
            Rect::new(
                rect.x() + 10.0,
                rect.y() + 32.0,
                rect.width() - 20.0,
                rect.height() - 42.0,
            ),
            detail,
            10.0,
            Color::rgba(0.70, 0.76, 0.86, 1.0),
        );
    }

    fn paint_curve(&self, ctx: &mut PaintCtx, rect: Rect) {
        ctx.fill(
            Path::rounded_rect(rect, 6.0),
            Color::rgba(0.105, 0.122, 0.155, 1.0),
        );
        draw_demo_label(
            ctx,
            Rect::new(rect.x() + 10.0, rect.y() + 8.0, rect.width() - 20.0, 18.0),
            "Easing curve",
            11.0,
            Color::rgba(0.84, 0.88, 0.94, 1.0),
        );

        let easing = self
            .editor
            .selection
            .keyframes
            .last()
            .and_then(|selection| selected_keyframe(&self.editor, *selection))
            .map(|keyframe| keyframe.easing)
            .unwrap_or(Easing::Linear);
        let graph = Rect::new(
            rect.x() + 14.0,
            rect.y() + 34.0,
            rect.width() - 28.0,
            rect.height() - 48.0,
        );
        ctx.stroke_rect(
            graph,
            Color::rgba(0.26, 0.30, 0.38, 1.0),
            StrokeStyle::new(1.0),
        );
        let mut path = Path::builder();
        for step in 0..=24 {
            let t = step as f32 / 24.0;
            let point = Point::new(
                graph.x() + graph.width() * t,
                graph.max_y() - graph.height() * easing.sample(t),
            );
            if step == 0 {
                path.move_to(point);
            } else {
                path.line_to(point);
            }
        }
        ctx.stroke(
            path.build(),
            Color::rgba(0.92, 0.70, 0.24, 1.0),
            StrokeStyle::new(2.0),
        );
    }
}

fn animation_editor_keyframes(
    editor: &AnimationEditorState,
    bounds: Rect,
) -> Vec<(KeyframeSelection, Keyframe<AnimationValue>, Rect)> {
    let Some(clip) = editor.document.timeline.clips.first() else {
        return Vec::new();
    };
    let lane_area = Rect::new(
        bounds.x() + 28.0,
        bounds.y() + 80.0,
        bounds.width() - 56.0,
        bounds.height() - 94.0,
    );
    let lane_height = 28.0;
    let mut hits = Vec::new();
    for (track_index, track) in clip.tracks.iter().enumerate() {
        let y = lane_area.y() + track_index as f32 * (lane_height + 8.0);
        for (keyframe_index, keyframe) in track.keyframes.iter().enumerate() {
            let x = lane_area.x()
                + lane_area.width()
                    * (keyframe.time / clip.duration.max(f64::EPSILON)).clamp(0.0, 1.0) as f32;
            hits.push((
                KeyframeSelection {
                    clip_index: 0,
                    track_index,
                    keyframe_index,
                },
                *keyframe,
                Rect::new(x - 7.0, y + (lane_height * 0.5) - 7.0, 14.0, 14.0),
            ));
        }
    }
    hits
}

fn selected_keyframe(
    editor: &AnimationEditorState,
    selection: KeyframeSelection,
) -> Option<Keyframe<AnimationValue>> {
    editor
        .document
        .timeline
        .clips
        .get(selection.clip_index)
        .and_then(|clip| clip.tracks.get(selection.track_index))
        .and_then(|track| track.keyframes.get(selection.keyframe_index))
        .copied()
}

fn selected_keyframe_detail(
    editor: &AnimationEditorState,
    selection: KeyframeSelection,
) -> Option<String> {
    let clip = editor.document.timeline.clips.get(selection.clip_index)?;
    let track = clip.tracks.get(selection.track_index)?;
    let keyframe = track.keyframes.get(selection.keyframe_index)?;
    Some(format!(
        "{}\ntime {:.2}s, easing {:?}",
        track.binding.property.path(),
        keyframe.time,
        keyframe.easing
    ))
}

fn timeline_playback_example_timeline() -> Timeline {
    let target = AnimationTargetId::new(TIMELINE_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.8).with_clip(
        Clip::new("timeline-playback", 0.0, 1.8)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.42)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Scalar(0.42)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-30.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Vector(Vector::new(30.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Vector(Vector::new(-30.0, 0.0))),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.9,
                        AnimationValue::Color(Color::rgba(0.10, 0.76, 0.52, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.8,
                        AnimationValue::Color(Color::rgba(0.20, 0.45, 0.95, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(TIMELINE_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(14.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.9, AnimationValue::Scalar(25.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.8, AnimationValue::Scalar(14.0)),
                ]),
            ),
    )
}

fn retained_layer_timeline() -> Timeline {
    let target = AnimationTargetId::new(RETAINED_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.4).with_clip(
        Clip::new("retained-layer", 0.0, 1.4)
            .with_track(
                Track::new(binding(AnimationProperty::LayerOpacity)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.70)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Scalar(0.70)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::LayerTranslation)).with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Vector(Vector::new(-26.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(0.7, AnimationValue::Vector(Vector::new(26.0, 0.0)))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.4, AnimationValue::Vector(Vector::new(-26.0, 0.0))),
                ]),
            ),
    )
}

fn paint_invalidation_timeline() -> Timeline {
    let target = AnimationTargetId::new(PAINT_TARGET);
    let binding = |property| AnimationBinding::new(target.clone(), property);

    Timeline::new(1.5).with_clip(
        Clip::new("paint-invalidation", 0.0, 1.5)
            .with_track(
                Track::new(binding(AnimationProperty::FillColor)).with_keyframes([
                    Keyframe::new(
                        0.0,
                        AnimationValue::Color(Color::rgba(0.82, 0.33, 0.24, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        0.75,
                        AnimationValue::Color(Color::rgba(0.92, 0.70, 0.24, 1.0)),
                    )
                    .with_easing(Easing::EaseInOut),
                    Keyframe::new(
                        1.5,
                        AnimationValue::Color(Color::rgba(0.82, 0.33, 0.24, 1.0)),
                    ),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(PAINT_RADIUS_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(18.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.75, AnimationValue::Scalar(28.0))
                        .with_easing(Easing::EaseInOut),
                    Keyframe::new(1.5, AnimationValue::Scalar(18.0)),
                ]),
            )
            .with_track(
                Track::new(binding(AnimationProperty::Custom(
                    AnimationPropertyPath::new(PAINT_ALPHA_PATH),
                )))
                .with_keyframes([
                    Keyframe::new(0.0, AnimationValue::Scalar(0.76)).with_easing(Easing::EaseInOut),
                    Keyframe::new(0.75, AnimationValue::Scalar(1.0)).with_easing(Easing::EaseInOut),
                    Keyframe::new(1.5, AnimationValue::Scalar(0.76)),
                ]),
            ),
    )
}

fn paint_demo_surface(ctx: &mut PaintCtx, bounds: Rect, fill: Color) {
    ctx.fill(Path::rounded_rect(bounds, 8.0), fill);
    ctx.stroke(
        Path::rounded_rect(bounds, 8.0),
        Color::rgba(0.34, 0.39, 0.48, 0.72),
        StrokeStyle::new(1.0),
    );
}

fn draw_demo_label(
    ctx: &mut PaintCtx,
    rect: Rect,
    text: impl Into<String>,
    size: f32,
    color: Color,
) {
    ctx.draw_text(
        rect,
        text.into(),
        TextStyle {
            font_size: size,
            line_height: size + 4.0,
            color,
            ..TextStyle::default()
        },
    );
}
