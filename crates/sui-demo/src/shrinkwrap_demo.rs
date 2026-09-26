use std::rc::Rc;

use sui::{
    SemanticsNode, SemanticsRole, WidgetPod, WidgetPodMutVisitor, WidgetPodVisitor, prelude::*,
};
use sui_text::{PersistentTextLayout, TextDocument, TextLayoutRequest};

use crate::app::{
    DemoTextRole, DevThemeReader, clone_dev_theme_reader, demo_text_style, demo_text_style_when,
};

pub(crate) const SHRINKWRAP_TAB_LABEL: &str = "Shrinkwrap";
const CHAT_NAME: &str = "Animated shrinkwrap conversation";
const WIDTH_NAME: &str = "Conversation width";
const MIN_WIDTH: f32 = 200.0;
const MAX_WIDTH: f32 = 450.0;
const PERIOD_SECONDS: f64 = 12.0;
const FRAME_INSET: f32 = 12.0;
const BUBBLE_PADDING: f32 = 12.0;
const BUBBLE_VERTICAL_PADDING: f32 = 8.0;
const GAP: f32 = 8.0;

// Original sample copy. Mixed scripts, combining marks, emoji, and a long
// unbroken token deliberately exercise different wrapping/shaping paths.
const MESSAGES: &[(&str, bool)] = &[
    ("The little window is moving again.", false),
    (
        "Watch the messages settle into a different shape as it gets narrower.",
        true,
    ),
    ("Short reply!", false),
    ("每一行都在重新排列。こんにちは！", true),
    (
        "A cafe\u{301}, a small idea, and a very long conversation ☕",
        false,
    ),
    ("مرحبا بالعالم — hello, world!", true),
    ("Try this: supercalifragilisticexpialidocious", false),
    (
        "Pause anywhere, or move the slider yourself. Nothing disappears when the words wrap.",
        true,
    ),
];

#[derive(Clone, PartialEq)]
struct Motion {
    phase: f64,
    width: f32,
    playing: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            phase: 0.0,
            width: (MIN_WIDTH + MAX_WIDTH) * 0.5,
            playing: true,
        }
    }
}

impl Motion {
    fn advance(&mut self, delta: f64) {
        if !self.playing || !delta.is_finite() {
            return;
        }
        self.phase = (self.phase + delta.max(0.0) * std::f64::consts::TAU / PERIOD_SECONDS)
            .rem_euclid(std::f64::consts::TAU);
        self.width = MIN_WIDTH + (MAX_WIDTH - MIN_WIDTH) * (0.5 + 0.5 * self.phase.sin()) as f32;
    }

    fn seek(&mut self, width: f32) {
        self.width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        self.phase =
            (2.0 * f64::from((self.width - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH)) - 1.0).asin();
        self.playing = false;
    }
}

pub(crate) fn build_shrinkwrap_demo_with_theme(theme: DevThemeReader) -> impl Widget {
    build_demo(Signal::named("Shrinkwrap motion", Motion::default()), theme)
}

fn build_demo(state: Signal<Motion>, theme: DevThemeReader) -> impl Widget {
    let play = state.clone();
    let pause = state.clone();
    let reset = state.clone();
    let slider_read = state.clone();
    let slider_write = state.clone();
    let readout = state.select_named("Shrinkwrap width readout", |state| {
        format!("{:.0} px", state.width)
    });
    Background::new(
        theme().palette.surface,
        ScrollView::vertical(Padding::all(
            24.0,
            Stack::vertical()
                .spacing(16.0)
                .alignment(Alignment::Stretch)
                .with_child(
                    Label::new("Shrinkwrap conversation").style_when(demo_text_style_when(
                        &theme,
                        DemoTextRole::PageTitle,
                        |theme| theme.palette.text,
                    )),
                )
                .with_child(
                    Label::new(
                        "A changing container. Real text reflow. Bubbles that fit their words.",
                    )
                    .style_when(demo_text_style_when(
                        &theme,
                        DemoTextRole::Supporting,
                        |theme| theme.palette.text_muted,
                    )),
                )
                .with_child(
                    Flex::horizontal()
                        .gap(8.0)
                        .wrap(FlexWrap::Wrap)
                        .with_item(
                            Button::new("Play")
                                .theme_when(clone_dev_theme_reader(&theme))
                                .on_press(move || {
                                    play.update(|state| state.playing = true);
                                }),
                            FlexItem::fixed(80.0),
                        )
                        .with_item(
                            Button::new("Pause")
                                .theme_when(clone_dev_theme_reader(&theme))
                                .on_press(move || {
                                    pause.update(|state| state.playing = false);
                                }),
                            FlexItem::fixed(80.0),
                        )
                        .with_item(
                            Button::new("Reset")
                                .theme_when(clone_dev_theme_reader(&theme))
                                .on_press(move || {
                                    reset.set(Motion::default());
                                }),
                            FlexItem::fixed(80.0),
                        ),
                )
                .with_child(
                    Flex::horizontal()
                        .gap(12.0)
                        .wrap(FlexWrap::Wrap)
                        .align_items(Alignment::Center)
                        .with_item(
                            Slider::new(WIDTH_NAME)
                                .range(MIN_WIDTH as f64, MAX_WIDTH as f64)
                                .step(1.0)
                                .theme_when(clone_dev_theme_reader(&theme))
                                .value_when(move || slider_read.get().width as f64)
                                .on_change(move |width| {
                                    slider_write.update(|state| state.seek(width as f32));
                                }),
                            FlexItem::new().basis_fraction(0.8).min_width(120.0),
                        )
                        .with_item(
                            Label::new("")
                                .text_from(readout)
                                .style_when(demo_text_style_when(
                                    &theme,
                                    DemoTextRole::Metadata,
                                    |theme| theme.palette.text_muted,
                                )),
                            FlexItem::fixed(64.0),
                        ),
                )
                .with_child(Chat::new(state, Rc::clone(&theme)))
                .with_child(
                    Label::new(
                        "Drag the width slider to pause. Play resumes from the current width.",
                    )
                    .style_when(demo_text_style_when(
                        &theme,
                        DemoTextRole::Supporting,
                        |theme| theme.palette.text_muted,
                    )),
                ),
        ))
        .name("Shrinkwrap demo scroll"),
    )
    .brush_when(move || theme().palette.surface)
}

struct Chat {
    theme: DevThemeReader,
    state: Signal<Motion>,
    bubbles: Vec<WidgetPod>,
    sizes: Vec<Size>,
    phone_size: Size,
}

impl Chat {
    fn new(state: Signal<Motion>, theme: DevThemeReader) -> Self {
        Self {
            state,
            bubbles: MESSAGES
                .iter()
                .map(|(text, sent)| {
                    WidgetPod::new(Bubble {
                        text,
                        sent: *sent,
                        theme: Rc::clone(&theme),
                        layout: None,
                    })
                })
                .collect(),
            theme,
            sizes: Vec::new(),
            phone_size: Size::ZERO,
        }
    }

    fn header_height(&self) -> f32 {
        (self.theme)().text.xs.line_height + FRAME_INSET * 2.0
    }

    fn phone_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + (bounds.width() - self.phone_size.width).max(0.0) * 0.5,
            bounds.y(),
            self.phone_size.width,
            self.phone_size.height,
        )
    }
}

impl Widget for Chat {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Wake(WakeEvent::AnimationFrame { delta, .. }) = event {
            if self.state.get().playing {
                self.state.update(|state| state.advance(*delta));
                ctx.request_animation_frame();
            }
            ctx.set_handled();
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let state = ctx.observe(&self.state);
        if state.playing {
            ctx.request_animation_frame();
        }
        let width = state.width.min(constraints.max.width).max(0.0);
        let chat_width = (width - FRAME_INSET * 2.0).max(0.0);
        let bubble_width = chat_width * 0.8;
        self.sizes = self
            .bubbles
            .iter_mut()
            .map(|bubble| {
                bubble.measure(
                    ctx,
                    Constraints::new(Size::ZERO, Size::new(bubble_width, f32::INFINITY)),
                )
            })
            .collect();
        let height = self.header_height()
            + FRAME_INSET
            + self.sizes.iter().map(|size| size.height).sum::<f32>()
            + GAP * self.sizes.len().saturating_sub(1) as f32;
        self.phone_size = Size::new(width, height);
        constraints.clamp(Size::new(
            if constraints.max.width.is_finite() {
                constraints.max.width
            } else {
                width
            },
            height,
        ))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        let phone = self.phone_bounds(bounds);
        let mut y = phone.y() + self.header_height();
        for ((bubble, size), (_, sent)) in self.bubbles.iter_mut().zip(&self.sizes).zip(MESSAGES) {
            let x = if *sent {
                phone.max_x() - FRAME_INSET - size.width
            } else {
                phone.x() + FRAME_INSET
            };
            bubble.arrange(ctx, Rect::new(x, y, size.width, size.height));
            y += size.height + GAP;
        }
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let phone = self.phone_bounds(ctx.bounds());
        let theme = (self.theme)();
        ctx.fill_rrect(phone, [theme.radius._3xl; 4], theme.surfaces.window_subtle);
        ctx.draw_text(
            Rect::new(
                phone.x() + FRAME_INSET,
                phone.y() + FRAME_INSET,
                (phone.width() - FRAME_INSET * 2.0).max(0.0),
                theme.text.xs.line_height,
            ),
            "Today · 09:41",
            demo_text_style(theme, DemoTextRole::Metadata, theme.palette.text_muted),
        );
        ctx.push_clip_rect(phone);
        for bubble in &self.bubbles {
            bubble.paint(ctx);
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            self.phone_bounds(ctx.bounds()),
        );
        node.name = Some(CHAT_NAME.into());
        ctx.push(node);
        for bubble in &self.bubbles {
            bubble.semantics(ctx);
        }
    }

    fn visit_children(&self, visitor: &mut dyn WidgetPodVisitor) {
        for bubble in &self.bubbles {
            visitor.visit(bubble);
        }
    }
    fn visit_children_mut(&mut self, visitor: &mut dyn WidgetPodMutVisitor) {
        for bubble in &mut self.bubbles {
            visitor.visit(bubble);
        }
    }
}

/// With uniform line height, matching size-only height preserves the line count.
/// Probe integer logical widths, then create only the final persistent layout.
fn shrink_width(
    max_width: f32,
    mut measure: impl FnMut(f32) -> sui::Result<Size>,
) -> sui::Result<(f32, Size)> {
    let cap = max_width.floor().max(1.0);
    let baseline = measure(cap)?;
    let mut low = 1;
    let mut high = cap as u32;
    while low < high {
        let middle = low + (high - low) / 2;
        if measure(middle as f32)?.height <= baseline.height + 0.001 {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let width = low as f32;
    Ok((width, measure(width)?))
}

struct Bubble {
    text: &'static str,
    sent: bool,
    theme: DevThemeReader,
    layout: Option<PersistentTextLayout>,
}

impl Widget for Bubble {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let cap = (constraints.max.width - 2.0 * BUBBLE_PADDING).max(1.0);
        let theme = (self.theme)();
        let color = if self.sent {
            theme.palette.accent_text
        } else {
            theme.palette.text
        };
        let style = demo_text_style(theme, DemoTextRole::Body, color);
        let line_height = style.line_height;
        let document = TextDocument::from_plain_text(self.text, style);
        let request =
            |width| TextLayoutRequest::new(document.clone()).with_box_size(Size::new(width, 1.0));
        let (width, size) = shrink_width(cap, |width| {
            ctx.layout().measure_document_size(request(width))
        })
        .unwrap_or((cap, Size::new(cap, line_height)));
        self.layout = ctx
            .layout()
            .layout_document_persistent(
                self.layout.as_ref().map(|layout| layout.handle()),
                TextLayoutRequest::new(document)
                    .with_box_size(Size::new(width, size.height.max(1.0))),
            )
            .ok();
        constraints.clamp(Size::new(
            width + 2.0 * BUBBLE_PADDING,
            size.height + 2.0 * BUBBLE_VERTICAL_PADDING,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let theme = (self.theme)();
        let round = theme.radius._2xl;
        let tail = theme.radius.sm;
        let radii = if self.sent {
            [round, round, tail, round]
        } else {
            [round, round, round, tail]
        };
        let color = if self.sent {
            theme.palette.accent
        } else {
            theme.palette.surface_raised
        };
        ctx.fill_rrect(ctx.bounds(), radii, color);
        if let Some(layout) = &self.layout {
            ctx.push_clip_rect(ctx.bounds());
            ctx.draw_persistent_text_layout(
                Point::new(
                    ctx.bounds().x() + BUBBLE_PADDING,
                    ctx.bounds().y() + BUBBLE_VERTICAL_PADDING,
                ),
                layout,
            );
            ctx.pop_clip();
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.text.into());
        ctx.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};
    use sui::{Application, SemanticsActionRequest, WgpuRenderer, WindowBuilder, WindowEvent};
    use sui_text::{FontRegistry, TextSystem};

    fn runtime(state: Signal<Motion>) -> sui::Result<(sui::Runtime, sui::WindowId)> {
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Shrinkwrap benchmark")
                    .root(build_demo(state, crate::app::default_dev_theme_reader())),
            )
            .build()?;
        let window = runtime.window_ids()[0];
        runtime.handle_event(
            window,
            Event::Window(WindowEvent::Resized(Size::new(960.0, 1100.0))),
        )?;
        Ok((runtime, window))
    }

    #[test]
    fn shrinkwrap_restyles_bubbles_when_theme_changes_while_paused() -> sui::Result<()> {
        use std::cell::RefCell;
        let theme = Rc::new(RefCell::new(DefaultTheme::default()));
        let reader = Rc::clone(&theme);
        let mut runtime = Application::new()
            .window(WindowBuilder::new().root(build_demo(
                Signal::new(Motion {
                    playing: false,
                    ..Motion::default()
                }),
                Rc::new(move || *reader.borrow()),
            )))
            .build()?;
        let window = runtime.window_ids()[0];
        let mut custom = DefaultTheme::dark();
        custom.text.base.size = 19.0;
        custom.text.base.line_height = 30.0;
        custom.palette.accent = custom.colors.success;
        custom.palette.accent_text = custom.colors.success_content;
        for next in [DefaultTheme::default(), DefaultTheme::dark(), custom] {
            *theme.borrow_mut() = next;
            runtime.handle_event(
                window,
                Event::Window(WindowEvent::Resized(Size::new(960.0, 1100.0))),
            )?;
            let output = runtime.render(window)?;
            let mut styles = Vec::new();
            let mut fills = Vec::new();
            output
                .frame
                .scene
                .visit_commands(&mut |command| match command {
                    sui::SceneCommand::DrawShapedText(run) => {
                        if let Some(layout) = run.resolve(&output.frame.text_layout_registry) {
                            styles.push((layout.text().to_string(), layout.style().clone()));
                        }
                    }
                    sui::SceneCommand::FillRoundedRect {
                        brush: sui::Brush::Solid(color),
                        ..
                    } => fills.push(*color),
                    _ => {}
                });
            for color in [
                next.surfaces.window_subtle,
                next.palette.accent,
                next.palette.surface_raised,
            ] {
                assert!(
                    fills.contains(&color),
                    "missing themed conversation fill {color:?}"
                );
            }
            for &(text, sent) in MESSAGES {
                let (_, style) = styles
                    .iter()
                    .find(|(value, _)| value == text)
                    .unwrap_or_else(|| panic!("missing bubble {text:?}"));
                assert_eq!(
                    style.color,
                    if sent {
                        next.palette.accent_text
                    } else {
                        next.palette.text
                    }
                );
                assert_eq!(style.font_size, next.text.base.size);
                assert_eq!(style.line_height, next.text.base.line_height);
            }
        }
        Ok(())
    }

    #[test]
    fn shrinkwrap_keeps_line_counts_and_finds_the_minimum_integer_width() -> sui::Result<()> {
        let system = TextSystem::new();
        let fonts = FontRegistry::new();
        for &(text, _) in MESSAGES {
            let document = TextDocument::from_plain_text(
                text,
                TextStyle {
                    font_size: 15.0,
                    line_height: 22.0,
                    ..TextStyle::default()
                },
            );
            for cap in [100.0, 137.0, 211.0, 316.0, 420.0] {
                let request = |width| {
                    TextLayoutRequest::new(document.clone()).with_box_size(Size::new(width, 1.0))
                };
                let (width, _) = shrink_width(cap, |width| {
                    system.measure_document_size(request(width), &fonts)
                })?;
                let baseline = system.layout_document(request(cap), &fonts)?;
                let wrapped = system.layout_document(request(width), &fonts)?;
                assert_eq!(
                    baseline.lines().len(),
                    wrapped.lines().len(),
                    "{text} at {cap}"
                );
                assert!(width <= cap);
                // RTL logical advances can include trailing whitespace beyond
                // the wrap box. Check visible ink against the padded bubble.
                let ink = wrapped.measurement().bounds;
                assert!(
                    ink.x() >= -BUBBLE_PADDING && ink.max_x() <= width + BUBBLE_PADDING,
                    "visible text overflow at {width}: {text}; ink={ink:?}"
                );
                if width > 1.0 {
                    let narrower = system.layout_document(request(width - 1.0), &fonts)?;
                    assert!(
                        narrower.lines().len() > wrapped.lines().len(),
                        "not minimal: {text} at {width}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn shrinkwrap_motion_is_continuous_and_manual_seek_resumes_without_jumping() {
        let mut motion = Motion::default();
        let initial = motion.width;
        for _ in 0..720 {
            motion.advance(1.0 / 60.0);
        }
        assert!((motion.width - initial).abs() < 0.001);
        motion.seek(274.0);
        motion.advance(0.5);
        assert_eq!(motion.width, 274.0);
        motion.playing = true;
        motion.advance(1.0 / 60.0);
        assert!((motion.width - 274.0).abs() < 2.0);
    }

    #[test]
    fn shrinkwrap_runtime_reflows_and_playback_controls_drive_real_frames() -> sui::Result<()> {
        let state = Signal::new(Motion::default());
        let (mut runtime, window) = runtime(state.clone())?;
        let initial = runtime.render(window)?;
        let chat_width = |output: &sui::RenderOutput| {
            output
                .semantics
                .iter()
                .find(|node| node.name.as_deref() == Some(CHAT_NAME))
                .unwrap()
                .bounds
                .width()
        };
        let before = chat_width(&initial);
        runtime.tick(0.1);
        for (id, event) in runtime.drain_ready_events() {
            runtime.handle_event(id, event)?;
        }
        runtime.render(window)?;
        runtime.tick(0.2);
        for (id, event) in runtime.drain_ready_events() {
            runtime.handle_event(id, event)?;
        }
        let animated = runtime.render(window)?;
        assert!(chat_width(&animated) > before);
        let pause = animated
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Pause"))
            .unwrap()
            .id;
        runtime.handle_semantics_action(window, pause, SemanticsActionRequest::Activate)?;
        assert!(!state.get().playing);
        runtime.render(window)?;
        let paused_width = state.get().width;
        // The activated button may finish its own press/focus animation.
        for tick in 3..=24 {
            runtime.tick(tick as f64 * 0.1);
            for (id, event) in runtime.drain_ready_events() {
                runtime.handle_event(id, event)?;
            }
            runtime.render(window)?;
            assert_eq!(state.get().width, paused_width);
        }
        assert!(runtime.next_wakeup_time(window)?.is_none());
        for width in [MIN_WIDTH, MAX_WIDTH] {
            state.update(|state| state.seek(width));
            let output = runtime.render(window)?;
            assert!((chat_width(&output) - width).abs() < 0.01);
            let phone = output
                .semantics
                .iter()
                .find(|node| node.name.as_deref() == Some(CHAT_NAME))
                .unwrap()
                .bounds;
            for &(message, _) in MESSAGES {
                let bubble = output
                    .semantics
                    .iter()
                    .find(|node| node.name.as_deref() == Some(message))
                    .unwrap()
                    .bounds;
                assert!(bubble.x() >= phone.x() && bubble.max_x() <= phone.max_x() + 0.01);
                assert!(bubble.max_y() <= phone.max_y() + 0.01);
            }
        }
        let output = runtime.render(window)?;
        let play = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Play"))
            .unwrap()
            .id;
        runtime.handle_semantics_action(window, play, SemanticsActionRequest::Activate)?;
        runtime.render(window)?;
        assert!(state.get().playing);
        assert!(runtime.next_wakeup_time(window)?.is_some());
        let output = runtime.render(window)?;
        let slider = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some(WIDTH_NAME))
            .unwrap()
            .id;
        runtime.handle_semantics_action(
            window,
            slider,
            SemanticsActionRequest::SetValue(sui::SemanticsValue::Number(274.0)),
        )?;
        assert!(!state.get().playing);
        assert_eq!(state.get().width, 274.0);
        let output = runtime.render(window)?;
        let reset = output
            .semantics
            .iter()
            .find(|node| node.name.as_deref() == Some("Reset"))
            .unwrap()
            .id;
        runtime.handle_semantics_action(window, reset, SemanticsActionRequest::Activate)?;
        assert_eq!(state.get().width, Motion::default().width);
        assert!(state.get().playing);
        assert_eq!(
            crate::app::dev_demo_label_for_slug("shrinkwrap"),
            Some(SHRINKWRAP_TAB_LABEL)
        );
        Ok(())
    }

    #[test]
    fn shrinkwrap_gallery_route_mounts_one_conversation() -> sui::Result<()> {
        let mut runtime = crate::app::build_dev_application_with_initial_demo_and_render_options(
            Some(SHRINKWRAP_TAB_LABEL),
            sui::WindowRenderOptions::new(true, 1.0),
        )
        .build()?;
        let output = runtime.render(runtime.window_ids()[0])?;
        assert_eq!(
            output
                .semantics
                .iter()
                .filter(|node| node.name.as_deref() == Some(CHAT_NAME))
                .count(),
            1
        );
        for &(text, _) in MESSAGES {
            assert!(
                output
                    .semantics
                    .iter()
                    .any(|node| node.name.as_deref() == Some(text))
            );
        }
        Ok(())
    }

    #[test]
    #[ignore = "paced layout/text/animation/renderer benchmark; run serially"]
    fn shrinkwrap_frame_profile() -> sui::Result<()> {
        const FRAMES: usize = 720;
        const WARMUP: usize = 30;
        let state = Signal::new(Motion {
            playing: false,
            ..Motion::default()
        });
        let (mut runtime, window) = runtime(state.clone())?;
        sui::set_window_scene_statistics_detail_mode(
            window,
            sui::SceneStatisticsDetailMode::Detailed,
        );
        let mut renderer = WgpuRenderer::new();
        renderer.render(&runtime.render(window)?.frame)?;
        println!(
            "SHRINKWRAP frames={FRAMES} viewport=960x1100 messages={} width=200..450 period_s={PERIOD_SECONDS} adapter={:?} text_timing={} validation={:?} debug={:?}",
            MESSAGES.len(),
            renderer.adapter_info(),
            std::env::var_os("SUI_PROFILE_TEXT_TIMINGS").is_some(),
            std::env::var_os("WGPU_VALIDATION"),
            std::env::var_os("WGPU_DEBUG")
        );
        let mut time = 0.0;
        let mut trace = String::from(
            "animated,frame,width,chat_height,runtime_ms,renderer_ms,layout_ms,paint_ms,text_us,text_size_requests,text_layout_misses,packet_builds,upload_bytes\n",
        );
        for animated in [false, true] {
            state.set(Motion {
                playing: animated,
                ..Motion::default()
            });
            runtime.render(window)?;
            let mut totals = BTreeMap::<String, f64>::new();
            let mut samples = Vec::new();
            let mut cache_before = runtime.text_preparation_cache_snapshot();
            let mut min_width = f32::INFINITY;
            let mut max_width = 0.0_f32;
            for index in 0..FRAMES + WARMUP {
                std::thread::sleep(Duration::from_millis(17));
                time += 1.0 / 60.0;
                runtime.tick(time);
                let start = Instant::now();
                runtime.handle_event(window, Event::Window(WindowEvent::RedrawRequested))?;
                for (id, event) in runtime.drain_ready_events() {
                    runtime.handle_event(id, event)?;
                }
                let output = runtime.render(window)?;
                let runtime_ms = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                renderer.render(&output.frame)?;
                let renderer_ms = start.elapsed().as_secs_f64() * 1000.0;
                if index < WARMUP {
                    cache_before = runtime.text_preparation_cache_snapshot();
                    continue;
                }
                min_width = min_width.min(state.get().width);
                max_width = max_width.max(state.get().width);
                samples.push(runtime_ms + renderer_ms);
                for (key, value) in [
                    ("runtime_ms", runtime_ms),
                    ("renderer_ms", renderer_ms),
                    (
                        "animation_wakes",
                        output.diagnostics.animation_frame_wake_count as f64,
                    ),
                    (
                        "text_requests",
                        output.diagnostics.runtime_text_timing.request_count as f64,
                    ),
                    (
                        "text_size_requests",
                        output
                            .diagnostics
                            .runtime_text_timing
                            .size_only_request_count as f64,
                    ),
                    (
                        "text_us",
                        output.diagnostics.runtime_text_timing.total_time_us as f64,
                    ),
                    (
                        "text_size_us",
                        output.diagnostics.runtime_text_timing.size_only_time_us as f64,
                    ),
                    (
                        "text_layout_misses",
                        output.diagnostics.runtime_text_timing.cache_miss_count as f64,
                    ),
                ] {
                    *totals.entry(key.into()).or_default() += value;
                }
                for phase in &output.diagnostics.phase_timings {
                    *totals
                        .entry(format!("{}_ms", phase.phase.label()))
                        .or_default() += phase.duration_ms;
                }
                let stats = renderer.last_frame_stats(window).unwrap();
                let phase_ms = |phase| {
                    output
                        .diagnostics
                        .phase_timings
                        .iter()
                        .filter(|sample| sample.phase == phase)
                        .map(|sample| sample.duration_ms)
                        .sum::<f64>()
                };
                let height = output
                    .semantics
                    .iter()
                    .find(|node| node.name.as_deref() == Some(CHAT_NAME))
                    .map_or(0.0, |node| node.bounds.height());
                trace.push_str(&format!("{animated},{},{:.4},{height:.3},{runtime_ms:.6},{renderer_ms:.6},{:.6},{:.6},{},{},{},{},{}\n",
                    index - WARMUP, state.get().width,
                    phase_ms(sui::FramePhase::MeasureArrange), phase_ms(sui::FramePhase::Paint),
                    output.diagnostics.runtime_text_timing.total_time_us,
                    output.diagnostics.runtime_text_timing.size_only_request_count,
                    output.diagnostics.runtime_text_timing.cache_miss_count,
                    stats.retained_packet_build_count, stats.uploaded_vertex_bytes));
                for (key, value) in [
                    ("draws", stats.draw_count as u64),
                    ("packet_builds", stats.retained_packet_build_count as u64),
                    ("prepared_hits", stats.prepared_fragment_cache_hits as u64),
                    ("snapshot_replayed", stats.snapshot_commands_replayed as u64),
                    ("submit_us", stats.queue_submit_time_us),
                    ("upload_bytes", stats.uploaded_vertex_bytes),
                ] {
                    *totals.entry(key.into()).or_default() += value as f64;
                }
            }
            let cache_after = runtime.text_preparation_cache_snapshot();
            samples.sort_by(f64::total_cmp);
            println!(
                "SHRINKWRAP animated={animated} p50_ms={:.3} p95_ms={:.3} max_ms={:.3} width_min={min_width:.1} width_max={max_width:.1} paragraph_misses={} glyph_metric_misses={}",
                samples[FRAMES / 2],
                samples[FRAMES * 95 / 100],
                samples[FRAMES - 1],
                cache_after.paragraphs.misses - cache_before.paragraphs.misses,
                cache_after.glyphs.misses - cache_before.glyphs.misses
            );
            assert_eq!(
                totals["animation_wakes"],
                if animated { FRAMES as f64 } else { 0.0 }
            );
            if animated {
                assert!(min_width < MIN_WIDTH + 0.1 && max_width > MAX_WIDTH - 0.1);
                assert!(totals.get("Measure and arrange_ms").copied().unwrap_or(0.0) > 0.0);
            }
            for (key, value) in totals {
                println!("  {key}={:.3}", value / FRAMES as f64);
            }
        }
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shrinkwrap-demo");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("profile.csv"), trace).unwrap();
        println!("trace={}", directory.join("profile.csv").display());
        Ok(())
    }

    #[test]
    #[ignore = "writes deterministic narrow, middle, and wide demo screenshots"]
    fn shrinkwrap_visual_capture() -> sui::Result<()> {
        let state = Signal::new(Motion {
            playing: false,
            ..Motion::default()
        });
        let (mut runtime, window) = runtime(state.clone())?;
        let mut renderer = WgpuRenderer::new();
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/shrinkwrap-demo");
        std::fs::create_dir_all(&directory).unwrap();
        for width in [MIN_WIDTH, 325.0, MAX_WIDTH] {
            state.update(|state| state.seek(width));
            renderer.render(&runtime.render(window)?.frame)?;
            let image = renderer.capture_rgba(window)?;
            let path = directory.join(format!("width-{width:.0}.png"));
            let mut encoder = png::Encoder::new(
                std::io::BufWriter::new(std::fs::File::create(&path).unwrap()),
                image.width(),
                image.height(),
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(image.pixels())
                .unwrap();
            println!("{}", path.display());
        }
        Ok(())
    }
}
