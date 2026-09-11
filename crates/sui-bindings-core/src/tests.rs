use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use super::*;
use sui::{RuntimeApplication, SceneCommand, SemanticsRole, WindowBuilder};

#[test]
fn binding_menu_item_preserves_recursive_submenus() {
    let item = BindingMenuItem::new(
        "Move to",
        None,
        false,
        false,
        false,
        vec![BindingMenuItem::new(
            "Shared",
            None,
            false,
            false,
            false,
            vec![BindingMenuItem::new(
                "Team workspace",
                None,
                false,
                false,
                false,
                Vec::new(),
            )],
        )],
    )
    .into_sui();

    assert_eq!(item.submenu_items()[0].label(), "Shared");
    assert_eq!(
        item.submenu_items()[0].submenu_items()[0].label(),
        "Team workspace"
    );
}

#[derive(Default)]
struct MockCallbacks {
    events: AtomicUsize,
    measures: AtomicUsize,
    paints: AtomicUsize,
}

impl ForeignWidgetCallbacks for MockCallbacks {
    fn debug_name(&self, _id: ForeignWidgetId) -> &'static str {
        "MockForeignWidget"
    }

    fn event(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignEventCtx<'_>,
        _event: &Event,
    ) -> ForeignCallbackResult<()> {
        self.events.fetch_add(1, Ordering::Relaxed);
        ctx.request_paint();
        ctx.set_handled();
        Ok(())
    }

    fn measure(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        self.measures.fetch_add(1, Ordering::Relaxed);
        Ok(constraints.clamp(Size::new(80.0, 24.0)))
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        self.paints.fetch_add(1, Ordering::Relaxed);
        let mut builder = PaintCommandBuilder::new();
        builder
            .fill_rect(ctx.bounds(), Color::rgba(0.2, 0.3, 0.4, 1.0))
            .unwrap();
        ctx.apply_all(builder.finish().unwrap())?;
        Ok(())
    }

    fn semantics(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignSemanticsCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Canvas, ctx.bounds());
        node.name = Some("Foreign canvas".to_string());
        node.state.disabled = true;
        node.state.hidden = true;
        node.state.hovered = true;
        node.state.selected = true;
        node.state.expanded = Some(true);
        ctx.push(node);
        Ok(())
    }
}

fn test_png_rgba(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
    }
    encoded
}

struct AppImageCallbacks {
    image: BindingImageHandle,
}

impl ForeignWidgetCallbacks for AppImageCallbacks {
    fn measure(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        Ok(constraints.clamp(Size::new(32.0, 16.0)))
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        let mut builder = PaintCommandBuilder::new();
        builder.draw_binding_image(ctx.bounds(), self.image)?;
        ctx.apply_all(builder.finish()?)?;
        Ok(())
    }
}

struct ChildCallbacks;

impl ForeignWidgetCallbacks for ChildCallbacks {
    fn measure(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        Ok(constraints.clamp(Size::new(40.0, 12.0)))
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        ctx.apply(PaintCommand::FillRect {
            rect: ctx.bounds(),
            brush: Brush::Solid(Color::WHITE),
        })?;
        Ok(())
    }
}

struct ContainerCallbacks;

impl ForeignWidgetCallbacks for ContainerCallbacks {
    fn measure(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignMeasureCtx<'_>,
        constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        let child = ctx
            .measure_child(0, constraints.loosen())
            .expect("child should be present");
        Ok(constraints.clamp(Size::new(child.width + 4.0, child.height + 4.0)))
    }

    fn arrange(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignArrangeCtx<'_>,
        bounds: Rect,
    ) -> ForeignCallbackResult<()> {
        assert!(ctx.arrange_child(0, Rect::new(bounds.x() + 2.0, bounds.y() + 2.0, 40.0, 12.0)));
        Ok(())
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        assert!(ctx.paint_child(0));
        Ok(())
    }
}

struct FailingCallbacks;

impl ForeignWidgetCallbacks for FailingCallbacks {
    fn measure(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignMeasureCtx<'_>,
        _constraints: Constraints,
    ) -> ForeignCallbackResult<Size> {
        Err(ForeignCallbackFailure::new("measure failed"))
    }

    fn paint(
        &self,
        _id: ForeignWidgetId,
        _ctx: &mut ForeignPaintCtx<'_>,
    ) -> ForeignCallbackResult<()> {
        panic!("paint failed")
    }
}

#[test]
fn ui_task_queue_posts_wakes_and_drains_tasks() {
    let woke = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicBool::new(false));
    let queue = UiTaskQueue::with_waker({
        let woke = Arc::clone(&woke);
        move || {
            woke.store(true, Ordering::Relaxed);
        }
    });
    let handle = queue.handle();

    handle.post({
        let completed = Arc::clone(&completed);
        move || {
            completed.store(true, Ordering::Relaxed);
        }
    });

    assert!(woke.load(Ordering::Relaxed));
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(queue.drain(), 1);
    assert!(completed.load(Ordering::Relaxed));
    assert!(queue.is_empty());
}

#[test]
fn paint_command_builder_validates_stack_balance_and_geometry() {
    let mut builder = PaintCommandBuilder::new();
    builder
        .push_clip_rect(Rect::new(0.0, 0.0, 10.0, 10.0))
        .unwrap()
        .fill_rect(Rect::new(1.0, 1.0, 8.0, 8.0), Color::WHITE)
        .unwrap();

    let error = builder.finish().unwrap_err();
    assert_eq!(error.kind, PaintValidationErrorKind::InvalidStackOperation);

    let mut invalid = PaintCommandBuilder::new();
    let error = invalid
        .fill_rect(Rect::new(0.0, 0.0, f32::NAN, 1.0), Color::WHITE)
        .unwrap_err();
    assert_eq!(error.kind, PaintValidationErrorKind::NonFiniteGeometry);
}

#[test]
fn paint_command_builder_validates_shader_commands() {
    let shader = BindingShader::saturation_value_plane(ColorSpace::Srgb, 0.25, 1.0).unwrap();
    let mut builder = PaintCommandBuilder::new();
    builder
        .draw_binding_shader_rect(Rect::new(0.0, 0.0, 20.0, 10.0), shader)
        .unwrap();

    assert!(matches!(
        builder.finish().unwrap().as_slice(),
        [PaintCommand::DrawShaderRect { .. }]
    ));

    let invalid_max =
        BindingShader::saturation_value_plane(ColorSpace::Srgb, 0.25, 0.0).unwrap_err();
    assert_eq!(invalid_max.kind, PaintValidationErrorKind::InvalidShader);

    let invalid_channel = BindingShader::rgb_channel_bar(Color::WHITE, 4, 1.0).unwrap_err();
    assert_eq!(
        invalid_channel.kind,
        PaintValidationErrorKind::InvalidShader
    );

    let mut invalid_builder = PaintCommandBuilder::new();
    let invalid_hue = invalid_builder
        .draw_shader_rect(
            Rect::new(0.0, 0.0, 20.0, 10.0),
            WidgetShader::ColorPickerSaturationBar {
                color_space: ColorSpace::Srgb,
                hue: f32::NAN,
                value: 1.0,
            },
        )
        .unwrap_err();
    assert_eq!(invalid_hue.kind, PaintValidationErrorKind::InvalidShader);
}

#[test]
fn paint_command_builder_validates_and_resolves_image_commands() {
    let local = BindingImageHandle::local(7);
    assert_eq!(local.local_slot(), Some(7));

    let mut builder = PaintCommandBuilder::new();
    builder
        .draw_binding_image(Rect::new(0.0, 0.0, 20.0, 10.0), local)
        .unwrap();
    let mut commands = builder.finish().unwrap();

    resolve_binding_image_slots(&mut commands, |slot| {
        assert_eq!(slot, 7);
        ImageHandle::new(99)
    });

    assert!(matches!(
        commands.as_slice(),
        [PaintCommand::DrawImage { source, .. }] if source.image == ImageHandle::new(99)
    ));

    let mut invalid_builder = PaintCommandBuilder::new();
    let error = invalid_builder
        .draw_image(Rect::new(0.0, 0.0, 20.0, 10.0), ImageHandle::new(0))
        .unwrap_err();
    assert_eq!(error.kind, PaintValidationErrorKind::InvalidImage);
}

#[test]
fn paint_command_builder_records_rich_low_level_commands() {
    let path = Path::circle(Point::new(8.0, 8.0), 4.0);
    let local = BindingImageHandle::local(3);
    let shadow = ShadowParams {
        offset_x: 1.0,
        offset_y: 2.0,
        blur: 3.0,
        spread: 0.5,
        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
    };

    let mut builder = PaintCommandBuilder::new();
    builder
        .push_clip_path(path.clone())
        .unwrap()
        .push_transform(Transform::translation(2.0, 3.0))
        .unwrap()
        .fill_path(path.clone(), Color::WHITE)
        .unwrap()
        .stroke_path(path, Color::BLACK, StrokeStyle::new(1.5))
        .unwrap()
        .draw_shadow(Rect::new(0.0, 0.0, 20.0, 12.0), [4.0; 4], shadow)
        .unwrap()
        .fill_rrect_with_shadow(
            Rect::new(2.0, 2.0, 16.0, 8.0),
            [3.0; 4],
            Color::rgba(0.2, 0.4, 0.8, 1.0),
            shadow,
        )
        .unwrap()
        .draw_binding_image_quad(
            [
                Point::new(0.0, 0.0),
                Point::new(16.0, 0.0),
                Point::new(16.0, 16.0),
                Point::new(0.0, 16.0),
            ],
            local,
        )
        .unwrap()
        .pop_transform()
        .unwrap()
        .pop_clip()
        .unwrap();
    let mut commands = builder.finish().unwrap();

    resolve_binding_image_slots(&mut commands, |slot| {
        assert_eq!(slot, 3);
        ImageHandle::new(42)
    });

    assert!(matches!(
        commands.as_slice(),
        [
            PaintCommand::PushClipPath(_),
            PaintCommand::PushTransform(_),
            PaintCommand::FillPath { .. },
            PaintCommand::StrokePath { .. },
            PaintCommand::FillRoundedRect { shadow: Some(_), .. },
            PaintCommand::FillRoundedRect { shadow: Some(_), .. },
            PaintCommand::DrawImageQuad { source, .. },
            PaintCommand::PopTransform,
            PaintCommand::PopClip,
        ] if source.image == ImageHandle::new(42)
    ));
}

#[test]
fn paint_command_builder_validates_text_style() {
    let mut style = TextStyle::new(Color::WHITE);
    style.font_size = f32::NAN;
    let mut builder = PaintCommandBuilder::new();
    let error = builder
        .draw_text(Rect::new(0.0, 0.0, 100.0, 20.0), "Bad text", style)
        .unwrap_err();
    assert_eq!(error.kind, PaintValidationErrorKind::InvalidTextStyle);

    let mut style = TextStyle::new(Color::WHITE);
    style.font = Some(FontHandle::new(0));
    let error = PaintCommandBuilder::new()
        .draw_text(Rect::new(0.0, 0.0, 100.0, 20.0), "Bad font", style)
        .unwrap_err();
    assert_eq!(error.kind, PaintValidationErrorKind::InvalidTextStyle);
}

#[test]
fn foreign_widget_adapter_renders_and_records_semantics() {
    let callbacks = Arc::new(MockCallbacks::default());
    let widget = ForeignWidget::from_arc(callbacks.clone());
    let mut runtime = RuntimeApplication::new()
        .window(WindowBuilder::new().title("Foreign").root(widget))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let output = runtime.render(window_id).unwrap();

    assert_eq!(callbacks.measures.load(Ordering::Relaxed), 1);
    assert_eq!(callbacks.paints.load(Ordering::Relaxed), 1);
    assert!(
        output
            .frame
            .scene
            .commands()
            .iter()
            .any(|command| matches!(command, SceneCommand::FillRect { .. }))
    );
    assert!(
        output
            .semantics
            .iter()
            .any(|node| node.role == SemanticsRole::Canvas
                && node.name.as_deref() == Some("Foreign canvas"))
    );

    let mut pointer = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(8.0, 8.0));
    pointer.button = Some(BindingPointerButton::Primary);
    pointer.buttons = 1;
    runtime
        .handle_event(
            window_id,
            BindingEvent::Pointer(pointer).into_sui_event().unwrap(),
        )
        .unwrap();
    assert_eq!(callbacks.events.load(Ordering::Relaxed), 1);
}

#[test]
fn foreign_widget_can_measure_arrange_and_paint_retained_children() {
    let widget =
        ForeignWidget::new(ContainerCallbacks).with_child(ForeignWidget::new(ChildCallbacks));
    let mut runtime = RuntimeApplication::new()
        .window(WindowBuilder::new().title("Children").root(widget))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let output = runtime.render(window_id).unwrap();

    assert!(
        output
            .frame
            .scene
            .commands()
            .iter()
            .any(|command| matches!(command, SceneCommand::FillRect { .. }))
    );
    let graph = runtime.widget_graph(window_id).unwrap();
    assert_eq!(graph.nodes.len(), 2);
}

#[test]
fn foreign_widget_callback_failures_are_captured() {
    let sink = ForeignErrorSink::new();
    let widget = ForeignWidget::new(FailingCallbacks).with_error_sink(sink.clone());
    let mut runtime = RuntimeApplication::new()
        .window(WindowBuilder::new().title("Errors").root(widget))
        .build()
        .unwrap();
    let window_id = runtime.window_ids()[0];

    let _ = runtime.render(window_id).unwrap();
    let errors = sink.snapshot();

    assert!(
        errors
            .iter()
            .any(|error| error.phase == ForeignCallbackPhase::Measure
                && error.message.contains("measure failed"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.phase == ForeignCallbackPhase::Paint
                && error.message.contains("paint failed"))
    );
}

#[test]
fn external_cpu_texture_descriptor_validates_pixel_length() {
    let texture = ExternalTextureDescriptor::cpu_rgba8(
        Size::new(2.0, 2.0),
        Arc::<[u8]>::from(vec![0; 16]),
        7,
    );

    assert_eq!(texture.tier(), RendererInteropTier::CpuUpload);
    assert!(texture.validate().is_ok());

    let invalid = ExternalTextureDescriptor::cpu_rgba8(
        Size::new(2.0, 2.0),
        Arc::<[u8]>::from(vec![0; 15]),
        8,
    );

    assert_eq!(
        invalid.validate().unwrap_err(),
        ExternalTextureValidationError::InvalidPixelLength {
            expected: 16,
            actual: 15,
        }
    );
}

#[test]
fn renderer_interop_capabilities_report_supported_tiers() {
    let cpu = RendererInteropCapabilities::cpu_only(NativeGraphicsBackend::Cpu);
    assert!(cpu.supports(RendererInteropTier::CpuUpload));
    assert!(!cpu.supports(RendererInteropTier::SharedTexture));
    assert!(!cpu.supports(RendererInteropTier::SharedRenderTarget));

    let gpu = RendererInteropCapabilities {
        backend: NativeGraphicsBackend::Wgpu,
        cpu_upload: true,
        shared_texture: true,
        shared_render_target: false,
    };
    assert!(gpu.supports(RendererInteropTier::CpuUpload));
    assert!(gpu.supports(RendererInteropTier::SharedTexture));
    assert!(!gpu.supports(RendererInteropTier::SharedRenderTarget));
}

#[test]
fn binding_app_renders_basic_widget_tree() {
    let state = BindingState::new("Ready");
    let pressed = Arc::new(AtomicBool::new(false));
    let button_action = BindingAction::new({
        let pressed = Arc::clone(&pressed);
        move || {
            pressed.store(true, Ordering::Relaxed);
            Ok(())
        }
    });
    let root = BindingWidget::column(
        [
            BindingWidget::label_state(state.clone()),
            BindingWidget::button("Apply", Some(button_action)),
        ],
        8.0,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Bindings", root));

    let snapshot = app.render_window(0).unwrap();

    assert!(snapshot.command_count > 0);
    assert!(snapshot.semantics_count >= 2);
    state.set("Updated");
    assert_eq!(state.label_text(), "Updated");
    assert!(!pressed.load(Ordering::Relaxed));
}

fn assert_cross_language_snapshot_signature(snapshot: &BindingRenderSnapshot) {
    assert!(snapshot.command_count > 0);
    assert!(snapshot.semantics_count >= 30);

    for role in [
        "generic_container",
        "text",
        "button",
        "link",
        "checkbox",
        "switch",
        "radio_button",
        "radio_group",
        "breadcrumb",
        "list",
        "list_item",
        "table",
        "slider",
        "spin_box",
        "combo_box",
        "progress_bar",
        "busy_indicator",
        "text_input",
        "image",
        "scroll_view",
        "color_swatch",
        "separator",
    ] {
        assert!(
            snapshot.semantics_roles.iter().any(|value| value == role),
            "missing semantics role {role:?} in {:?}",
            snapshot.semantics_roles
        );
    }

    for name in [
        "Ready",
        "Apply",
        "Search icon",
        "Download",
        "Main surface",
        "Surface content",
        "Main toolbar",
        "Toolbar action",
        "Toolbar search",
        "Documentation",
        "Enabled",
        "Airplane mode",
        "Manual",
        "Priority",
        "View mode",
        "Show list view",
        "Gallery",
        "Show map view",
        "Workspace path",
        "Assets",
        "Brush",
        "Canvas",
        "Export",
        "Build table",
        "Input signal",
        "Online",
        "Editor status",
        "Ln 12",
        "Writable",
        "UTF-8",
        "Build",
        "Opacity",
        "Count",
        "Mode",
        "Load progress",
        "Background work",
        "Name",
        "Password",
        "Scheduled for",
        "Notes",
        "Scrollable content",
        "Rich summary",
        "Accent",
        "Section divider",
        "Projects empty",
        "New project",
    ] {
        assert!(
            snapshot.semantics_names.iter().any(|value| value == name),
            "missing semantics name {name:?} in {:?}",
            snapshot.semantics_names
        );
    }

    for value in [
        "https://example.invalid/docs",
        "0.5:0:1",
        "3:0:10",
        "Medium",
        "Gallery",
        "List",
        "Map",
        "sui",
        "Canvas",
        "Bindings",
        "active",
        "Online",
        "All systems nominal",
        "Ln 12",
        "Writable",
        "UTF-8",
        "Debug profile with local bindings",
        "Final",
        "0.25:0:1",
        "Ada",
        "••••••",
        "2026-07-15 09:30",
        "Line one\nLine two",
        "Warm cool",
        "#4080BFFF",
    ] {
        assert!(
            snapshot.semantics_values.iter().any(|found| found == value),
            "missing semantics value {value:?} in {:?}",
            snapshot.semantics_values
        );
    }

    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|value| value == "Loading assets"),
        "missing busy indicator description in {:?}",
        snapshot.semantics_descriptions
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|value| value == "Download file"),
        "missing icon button description in {:?}",
        snapshot.semantics_descriptions
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|value| value == "Live audio input"),
        "missing signal meter description in {:?}",
        snapshot.semantics_descriptions
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|value| value == "Compact rows"),
        "missing segmented control description in {:?}",
        snapshot.semantics_descriptions
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|value| value == "All systems nominal"),
        "missing status bar description in {:?}",
        snapshot.semantics_descriptions
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|value| value == "Create a project to get started. Templates are available"),
        "missing empty state description in {:?}",
        snapshot.semantics_descriptions
    );
    for checked in ["checked", "unchecked"] {
        assert!(
            snapshot
                .semantics_checked
                .iter()
                .any(|value| value == checked),
            "missing checked state {checked:?} in {:?}",
            snapshot.semantics_checked
        );
    }
    assert!(
        snapshot.semantics_busy.iter().any(|value| *value),
        "missing busy semantics state in {:?}",
        snapshot.semantics_busy
    );
    assert!(
        snapshot
            .semantics_editable_multiline
            .iter()
            .any(|value| *value),
        "missing multiline editable semantics in {:?}",
        snapshot.semantics_editable_multiline
    );
    assert!(
        snapshot.semantics_selected.iter().any(|value| *value),
        "missing selected semantics state in {:?}",
        snapshot.semantics_selected
    );
}

#[test]
fn binding_app_renders_cross_language_compatibility_signature() {
    let opacity = BindingState::new(0.5);
    let count = BindingState::new(3.0);
    let progress = BindingState::new(0.25);
    let text = BindingState::new("Ada");
    let password = BindingState::new("sëcret");
    let scheduled_for = BindingState::new("2026-07-15 09:30");
    let notes = BindingState::new("Line one\nLine two");
    let root = BindingWidget::column(
        [
            BindingWidget::label("Ready"),
            BindingWidget::button("Apply", None),
            BindingWidget::icon(
                IconGlyph::Search,
                Some("Search icon".to_owned()),
                None,
                None,
            ),
            BindingWidget::icon_button(
                IconGlyph::Download,
                "Download",
                true,
                true,
                Some(28.0),
                Some(16.0),
                Some("Download file".to_owned()),
                None,
            ),
            BindingWidget::surface(
                BindingWidget::label("Surface content"),
                SurfaceRole::Panel,
                Some("Main surface".to_owned()),
                None,
                Some(SurfaceElevation::Small),
                None,
                Some(6.0),
                false,
                false,
            ),
            BindingWidget::toolbar(
                [
                    BindingWidget::button("Toolbar action", None),
                    BindingWidget::icon(
                        IconGlyph::Search,
                        Some("Toolbar search".to_owned()),
                        None,
                        None,
                    ),
                ],
                Axis::Horizontal,
                Some("Main toolbar".to_owned()),
                Some(32.0),
                Some(4.0),
                Some(4.0),
                None,
                true,
            ),
            BindingWidget::link(
                "Documentation",
                "https://example.invalid/docs",
                None,
                true,
                None,
            ),
            BindingWidget::checkbox("Enabled", true, None),
            BindingWidget::switch("Airplane mode", false, None),
            BindingWidget::radio_button("Manual", true, None),
            BindingWidget::radio_group(
                "Priority",
                ["Low", "Medium", "High"],
                Some(BindingNumber::Static(1.0)),
                None,
            ),
            BindingWidget::segmented_control(
                "View mode",
                [
                    BindingSegmentedControlItem::new(
                        "List",
                        Some("Show list view".to_string()),
                        Some("Compact rows".to_string()),
                        false,
                    ),
                    BindingSegmentedControlItem::new("Gallery", None, None, false),
                    BindingSegmentedControlItem::new(
                        "Map",
                        Some("Show map view".to_string()),
                        None,
                        true,
                    ),
                ],
                Some(BindingNumber::Static(1.0)),
                None,
            ),
            BindingWidget::breadcrumb(
                "Workspace path",
                ["D:", "Workspace", "sui"],
                Some(BindingNumber::Static(2.0)),
                None,
            ),
            BindingWidget::list_view(
                "Assets",
                ["Brush", "Canvas", "Export"],
                Some(BindingNumber::Static(1.0)),
                None,
            ),
            BindingWidget::table(
                "Build table",
                [
                    BindingTableColumn::new(
                        "Task",
                        Some(160.0),
                        None,
                        TableColumnAlignment::Start,
                        false,
                    ),
                    BindingTableColumn::new(
                        "Owner",
                        Some(96.0),
                        None,
                        TableColumnAlignment::Center,
                        false,
                    ),
                ],
                [
                    BindingTableRow::new(["Bindings", "IX"]),
                    BindingTableRow::new(["Renderer", "Core"]),
                ],
                Some(BindingNumber::Static(0.0)),
                None,
            ),
            BindingWidget::signal_meter(
                "Input signal",
                true,
                Some("Live audio input".to_string()),
                8,
                Some(Size::new(76.0, 16.0)),
            ),
            BindingWidget::status_badge(
                "Online",
                SemanticTone::Success,
                Some(IconGlyph::Check),
                Some(72.0),
            ),
            BindingWidget::status_bar(
                [
                    BindingStatusBarSegment::new("Ln 12", SemanticTone::Neutral, None, false),
                    BindingStatusBarSegment::new(
                        "Writable",
                        SemanticTone::Success,
                        Some(84.0),
                        false,
                    ),
                    BindingStatusBarSegment::new("UTF-8", SemanticTone::Info, None, true),
                ],
                Some("Editor status".to_string()),
                Some("All systems nominal".into()),
                Some(24.0),
            ),
            BindingWidget::detail_row("Build", "Debug profile with local bindings", Some(2)),
            BindingWidget::slider("Opacity", opacity, 0.0, 1.0, 0.25, None),
            BindingWidget::number_input("Count", count, 0.0, 10.0, 1.0, 0, None),
            BindingWidget::select(
                "Mode",
                ["Draft", "Final", "Review"],
                Some(BindingNumber::Static(1.0)),
                Some("Choose mode".to_string()),
                None,
            ),
            BindingWidget::progress_bar("Load progress", progress, 0.0, 1.0, true),
            BindingWidget::busy_indicator("Background work", Some("Loading assets".into()), 20.0),
            BindingWidget::text_input("Name", text, Some("Type a name".to_string()), None),
            BindingWidget::password_input(
                "Password",
                password,
                Some("Enter a password".to_string()),
                None,
            ),
            BindingWidget::datetime_input("Scheduled for", scheduled_for, None, None),
            BindingWidget::text_area("Notes", notes, Some("Type notes".to_string()), None),
            BindingWidget::scroll_view(
                BindingWidget::rich_text(
                    [
                        BindingTextSpan::new(
                            "Warm",
                            TextStyle::new(Color::rgba(0.9, 0.35, 0.2, 1.0)),
                        ),
                        BindingTextSpan::new(
                            " cool",
                            TextStyle::new(Color::rgba(0.25, 0.55, 0.9, 1.0)),
                        ),
                    ],
                    Some("Rich summary".to_string()),
                    0.0,
                    0.0,
                ),
                BindingScrollAxes::Vertical,
                Some("Scrollable content".to_string()),
            ),
            BindingWidget::color_swatch(
                "Accent",
                Color::rgba(0.25, 0.5, 0.75, 1.0),
                Some(Size::new(24.0, 24.0)),
                false,
                None,
            ),
            BindingWidget::separator(
                Axis::Horizontal,
                Some("Section divider".to_string()),
                0.0,
                None,
                Some(24.0),
            ),
            BindingWidget::action_card(
                "Create document",
                "Start from a blank canvas",
                Some(IconGlyph::Add),
                SemanticTone::Accent,
                true,
                None,
            ),
            BindingWidget::brush_preview(
                "Brush preview",
                "Ink brush",
                BindingBrushPreviewSpec::new(
                    Color::rgba(0.2, 0.45, 0.8, 1.0),
                    18.0,
                    0.75,
                    BrushPreviewShape::Round,
                ),
                Some(Size::new(48.0, 48.0)),
            ),
            BindingWidget::command_group(
                "Editing commands",
                [BindingWidget::button("Duplicate", None)],
                Axis::Horizontal,
                None,
                Some(4.0),
                None,
                None,
                None,
            ),
            BindingWidget::coverage_dots(
                "Replica coverage",
                2,
                3,
                SemanticTone::Success,
                4,
                true,
                Some(84.0),
            ),
            BindingWidget::dock(
                BindingWidget::label("Dock body"),
                Some((24.0, BindingWidget::label("Dock top"))),
                Some((24.0, BindingWidget::label("Dock bottom"))),
                240.0,
                120.0,
            ),
            BindingWidget::fixed_pane_split(
                Axis::Horizontal,
                BindingWidget::label("Fixed pane"),
                BindingWidget::separator(Axis::Vertical, None, 0.0, None, None),
                BindingWidget::label("Flexible pane"),
                false,
                96.0,
                1.0,
                160.0,
            ),
            BindingWidget::framed_field(
                BindingWidget::label("Framed value"),
                Some("Framed field".to_string()),
                Some("Reusable editor frame".to_string()),
                Some(Insets::all(6.0)),
                Some(32.0),
                true,
                false,
                false,
            ),
            BindingWidget::measured_bottom_dock(
                BindingWidget::label("Measured dock body"),
                BindingWidget::label("Measured dock footer"),
                Size::new(240.0, 120.0),
            ),
            BindingWidget::placement_badge(
                "Primary replica",
                Some(IconGlyph::Check),
                SemanticTone::Success,
                Some(2),
                Some(3),
                Some(120.0),
            ),
            BindingWidget::property_row(
                "Property",
                BindingWidget::label("Property value"),
                false,
                Some(100.0),
                Some(160.0),
                Some(8.0),
            ),
            BindingWidget::section_label("Advanced", Some("Advanced section".to_string()), None),
            BindingWidget::side_sheet(
                "Inspector",
                BindingWidget::label("Inspector body"),
                Some("Document settings".to_string()),
                true,
                false,
                true,
                SideSheetPlacement::Right,
                Some(280.0),
                Some(BindingWidget::button("Close inspector", None)),
                [BindingWidget::button("Save inspector", None)],
                None,
            ),
            BindingWidget::split_view(
                Some("Document split".to_string()),
                Axis::Horizontal,
                BindingWidget::label("Split first"),
                BindingWidget::label("Split second"),
                0.4,
                40.0,
                40.0,
                Some(2.0),
                None,
            ),
            BindingWidget::switch_view(
                [
                    BindingWidget::label("Switch first"),
                    BindingWidget::label("Switch second"),
                ],
                0.0,
            ),
            BindingWidget::trailing_slot_row(
                BindingWidget::label("Trailing row body"),
                BindingWidget::button("Trailing action", None),
                96.0,
                28.0,
                6.0,
            ),
            BindingWidget::virtual_scroll_view(
                [
                    BindingWidget::label("Virtual row one"),
                    BindingWidget::label("Virtual row two"),
                ],
                Some("Virtual rows".to_string()),
                Some(Insets::all(4.0)),
                Some(2.0),
            ),
            BindingWidget::floating_stack(
                [
                    BindingFloatingStackWindow::new(
                        Rect::new(0.0, 0.0, 160.0, 48.0),
                        BindingWidget::label("Floating first"),
                    ),
                    BindingFloatingStackWindow::new(
                        Rect::new(24.0, 16.0, 160.0, 48.0),
                        BindingWidget::label("Floating second"),
                    ),
                ],
                Some("Floating windows".to_string()),
            ),
            BindingWidget::reorderable_list(
                "Reorderable tasks",
                [
                    BindingWidget::label("First task"),
                    BindingWidget::label("Second task"),
                ],
                4.0,
                4.0,
                Some("Reordering task".to_string()),
                None,
            ),
            BindingWidget::empty_state(
                "No projects",
                "Create a project to get started.",
                Some("Projects empty".to_string()),
                Some("Templates are available".to_string()),
                Some(IconGlyph::Folder),
                Some(BindingWidget::button("New project", None)),
                None,
                true,
            ),
        ],
        6.0,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Compatibility", root));

    let snapshot = app.render_window(0).unwrap();

    assert_cross_language_snapshot_signature(&snapshot);
}

#[test]
fn binding_side_sheet_reads_bound_shown_state() {
    let shown = BindingState::new(false);
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Side sheet state",
        BindingWidget::side_sheet(
            "Bound inspector",
            BindingWidget::label("Inspector content"),
            Some("State-driven drawer".to_string()),
            shown.clone(),
            true,
            true,
            SideSheetPlacement::Right,
            Some(280.0),
            None,
            std::iter::empty(),
            None,
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let hidden = runtime.render_window(window_id).unwrap();
    assert!(shown.is_ui_bound());
    assert!(!hidden.semantics_roles.iter().any(|role| role == "dialog"));
    assert!(
        !hidden
            .semantics_names
            .iter()
            .any(|name| name == "Bound inspector")
    );

    shown.set(true);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    let visible = runtime.render_window(window_id).unwrap();
    assert!(visible.semantics_roles.iter().any(|role| role == "dialog"));
    assert!(
        visible
            .semantics_names
            .iter()
            .any(|name| name == "Bound inspector")
    );
    let content_id = runtime
        .runtime
        .semantics(window_id.into_sui())
        .unwrap()
        .iter()
        .find(|node| node.name.as_deref() == Some("Inspector content"))
        .unwrap()
        .id;

    shown.set(false);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    let hidden_again = runtime.render_window(window_id).unwrap();
    assert!(
        !hidden_again
            .semantics_names
            .iter()
            .any(|name| name == "Bound inspector")
    );

    shown.set(true);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    let _ = runtime.render_window(window_id).unwrap();
    assert_eq!(
        runtime
            .runtime
            .semantics(window_id.into_sui())
            .unwrap()
            .iter()
            .find(|node| node.name.as_deref() == Some("Inspector content"))
            .unwrap()
            .id,
        content_id,
        "bound visibility updates must retain sheet content identity"
    );
}

#[test]
fn binding_split_view_reads_bound_ratio_state() {
    let ratio = BindingState::new(0.25);
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Split view state",
        BindingWidget::split_view(
            Some("Bound split".to_string()),
            Axis::Horizontal,
            BindingWidget::label("First pane"),
            BindingWidget::label("Second pane"),
            ratio.clone(),
            20.0,
            20.0,
            Some(2.0),
            None,
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let initial = runtime.render_window(window_id).unwrap();
    assert!(ratio.is_ui_bound());
    assert!(
        initial
            .semantics_roles
            .iter()
            .any(|role| role == "splitter")
    );
    assert!(
        initial
            .semantics_names
            .iter()
            .any(|name| name == "Bound split")
    );
    assert!(initial.semantics_values.iter().any(|value| value == "0.25"));
    let first_pane_id = runtime
        .runtime
        .semantics(window_id.into_sui())
        .unwrap()
        .iter()
        .find(|node| node.name.as_deref() == Some("First pane"))
        .unwrap()
        .id;

    ratio.set(0.75);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    let updated = runtime.render_window(window_id).unwrap();
    assert!(updated.semantics_values.iter().any(|value| value == "0.75"));
    assert!(!updated.semantics_values.iter().any(|value| value == "0.25"));
    assert_eq!(
        runtime
            .runtime
            .semantics(window_id.into_sui())
            .unwrap()
            .iter()
            .find(|node| node.name.as_deref() == Some("First pane"))
            .unwrap()
            .id,
        first_pane_id,
        "bound ratio updates must retain pane widget identity"
    );
}

#[test]
fn binding_virtual_scroll_floating_stack_and_reorderable_list_render_children() {
    let root = BindingWidget::column(
        [
            BindingWidget::sized_box(
                Some(BindingWidget::virtual_scroll_view(
                    [
                        BindingWidget::label("Virtual child one"),
                        BindingWidget::label("Virtual child two"),
                    ],
                    Some("Virtual collection".to_string()),
                    Some(Insets::all(4.0)),
                    Some(2.0),
                )),
                Some(240.0),
                Some(80.0),
            ),
            BindingWidget::sized_box(
                Some(BindingWidget::floating_stack(
                    [
                        BindingFloatingStackWindow::new(
                            Rect::new(0.0, 0.0, 160.0, 36.0),
                            BindingWidget::label("Floating child one"),
                        ),
                        BindingFloatingStackWindow::new(
                            Rect::new(20.0, 28.0, 160.0, 36.0),
                            BindingWidget::label("Floating child two"),
                        ),
                    ],
                    Some("Floating workspace".to_string()),
                )),
                Some(240.0),
                Some(80.0),
            ),
            BindingWidget::reorderable_list(
                "Queued work",
                [
                    BindingWidget::sized_box(
                        Some(BindingWidget::label("Queued first")),
                        Some(200.0),
                        Some(28.0),
                    ),
                    BindingWidget::sized_box(
                        Some(BindingWidget::label("Queued second")),
                        Some(200.0),
                        Some(28.0),
                    ),
                ],
                2.0,
                4.0,
                Some("Moving queued work".to_string()),
                None,
            ),
        ],
        6.0,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Portable containers", root));

    let snapshot = app.render_window(0).unwrap();

    assert!(snapshot.command_count > 0);
    for name in [
        "Virtual collection",
        "Virtual child one",
        "Floating workspace",
        "Floating child one",
        "Floating child two",
        "Queued work",
        "Queued first",
        "Queued second",
    ] {
        assert!(
            snapshot.semantics_names.iter().any(|found| found == name),
            "missing semantics name {name:?} in {:?}",
            snapshot.semantics_names
        );
    }
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "scroll_view")
    );
    assert!(snapshot.semantics_roles.iter().any(|role| role == "list"));
}

#[test]
fn binding_reorderable_list_reports_reorder_and_captures_callback_error() {
    let changes = Arc::new(Mutex::new(Vec::new()));
    let action = BindingReorderAction::new({
        let changes = Arc::clone(&changes);
        move |item, from, to| {
            recover_lock(&changes).push((item, from, to));
            Err(ForeignCallbackFailure::new("reorder callback failed"))
        }
    });
    let root = BindingWidget::reorderable_list(
        "Tasks",
        [
            BindingWidget::sized_box(None, Some(120.0), Some(30.0)),
            BindingWidget::sized_box(None, Some(120.0), Some(30.0)),
            BindingWidget::sized_box(None, Some(120.0), Some(30.0)),
        ],
        0.0,
        4.0,
        Some("Moving task".to_string()),
        Some(action),
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Reorder", root));
    let errors = app.error_sink();
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();
    let _ = runtime.render_window(window_id).unwrap();

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(10.0, 15.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();

    for position in [Point::new(10.0, 48.0), Point::new(10.0, 78.0)] {
        let mut moved = BindingPointerEvent::new(BindingPointerEventKind::Move, position);
        moved.button = Some(BindingPointerButton::Primary);
        moved.buttons = 1;
        runtime
            .handle_event(window_id, BindingEvent::Pointer(moved))
            .unwrap();
    }

    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(10.0, 78.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert_eq!(&*recover_lock(&changes), &[(0, 0, 2)]);
    let errors = errors.snapshot();
    assert!(errors.iter().any(|error| {
        error.phase == ForeignCallbackPhase::Event
            && error.message.contains("reorder callback failed")
    }));
}

#[test]
fn binding_app_renders_form_controls_and_updates_bound_checkbox() {
    let checked = BindingState::new(false);
    let slider_value = BindingState::new(0.25);
    let toggled = Arc::new(AtomicBool::new(false));
    let toggle_action = BindingBoolAction::new({
        let toggled = Arc::clone(&toggled);
        move |value| {
            toggled.store(value, Ordering::Relaxed);
            Ok(())
        }
    });
    let root = BindingWidget::column(
        [
            BindingWidget::checkbox("Enabled", checked.clone(), Some(toggle_action)),
            BindingWidget::switch("Airplane mode", false, None),
            BindingWidget::slider("Opacity", slider_value.clone(), 0.0, 1.0, 0.05, None),
        ],
        8.0,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Controls", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.command_count > 0);
    assert!(snapshot.semantics_count >= 3);
    assert_eq!(checked.get(), BindingValue::Bool(false));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(32.0, 18.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert_eq!(checked.get(), BindingValue::Bool(true));
    assert!(toggled.load(Ordering::Relaxed));
}

#[test]
fn binding_radio_button_updates_bound_state_from_pointer() {
    let selected = BindingState::new(false);
    let selected_action = Arc::new(AtomicBool::new(false));
    let action = BindingAction::new({
        let selected_action = Arc::clone(&selected_action);
        move || {
            selected_action.store(true, Ordering::Relaxed);
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Radio",
        BindingWidget::radio_button("Manual", selected.clone(), Some(action)),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "radio_button")
    );
    assert_eq!(selected.get(), BindingValue::Bool(false));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(32.0, 18.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert_eq!(selected.get(), BindingValue::Bool(true));
    assert!(selected_action.load(Ordering::Relaxed));
}

#[test]
fn binding_link_invokes_open_callback_from_pointer() {
    let opened = Arc::new(Mutex::new(None::<String>));
    let action = BindingStringAction::new({
        let opened = Arc::clone(&opened);
        move |url| {
            *opened.lock().unwrap() = Some(url);
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Link",
        BindingWidget::link(
            "Documentation",
            "https://example.invalid/docs",
            None,
            true,
            Some(action),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.semantics_roles.iter().any(|role| role == "link"));
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "https://example.invalid/docs")
    );

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(4.0, 4.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(4.0, 4.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert_eq!(
        opened.lock().unwrap().as_deref(),
        Some("https://example.invalid/docs")
    );
}

#[test]
fn binding_color_swatch_invokes_press_callback_from_pointer() {
    let pressed = Arc::new(AtomicBool::new(false));
    let action = BindingAction::new({
        let pressed = Arc::clone(&pressed);
        move || {
            pressed.store(true, Ordering::Relaxed);
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Swatch",
        BindingWidget::color_swatch(
            "Accent",
            Color::rgba(0.25, 0.5, 0.75, 1.0),
            Some(Size::new(24.0, 24.0)),
            false,
            Some(action),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "color_swatch")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "#4080BFFF")
    );

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(12.0, 12.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(12.0, 12.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert!(pressed.load(Ordering::Relaxed));
}

#[test]
fn binding_rich_text_exposes_plain_text_semantics() {
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Rich text",
        BindingWidget::rich_text(
            [
                BindingTextSpan::new("Warm", TextStyle::new(Color::rgba(0.9, 0.35, 0.2, 1.0))),
                BindingTextSpan::new(" cool", TextStyle::new(Color::rgba(0.25, 0.55, 0.9, 1.0))),
            ],
            Some("Rich summary".to_string()),
            80.0,
            0.0,
        ),
    ));

    let snapshot = app.render_window(0).unwrap();

    assert!(snapshot.command_count > 0);
    assert!(snapshot.semantics_roles.iter().any(|role| role == "text"));
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Rich summary")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Warm cool")
    );
}

#[test]
fn binding_scroll_view_exposes_container_and_child_semantics() {
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Scroll",
        BindingWidget::scroll_view(
            BindingWidget::label("Inside"),
            BindingScrollAxes::Vertical,
            Some("Scrollable content".to_string()),
        ),
    ));

    let snapshot = app.render_window(0).unwrap();

    assert!(snapshot.command_count > 0);
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "scroll_view")
    );
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Scrollable content")
    );
    assert!(snapshot.semantics_names.iter().any(|name| name == "Inside"));
}

#[test]
fn binding_breadcrumb_reads_bound_state() {
    let name = BindingState::new("Workspace path");
    let current = BindingState::new(0.0);
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Breadcrumb",
        BindingWidget::breadcrumb(
            BindingText::State(name.clone()),
            ["D:", "Workspace", "sui"],
            Some(BindingNumber::State(current.clone())),
            None,
        ),
    ));

    let snapshot = app.render_window(0).unwrap();
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "breadcrumb")
    );
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|found| found == "Workspace path")
    );
    assert!(snapshot.semantics_values.iter().any(|value| value == "D:"));

    name.set("Project path");
    current.set(2.0);
    let snapshot = app.render_window(0).unwrap();
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|found| found == "Project path")
    );
    assert!(snapshot.semantics_values.iter().any(|value| value == "sui"));
}

#[test]
fn binding_table_reads_bound_state() {
    let name = BindingState::new("Build table");
    let selected = BindingState::new(1.0);
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Table",
        BindingWidget::table(
            BindingText::State(name.clone()),
            [
                BindingTableColumn::new("Task", None, None, TableColumnAlignment::Start, false),
                BindingTableColumn::new("Owner", None, None, TableColumnAlignment::Center, false),
            ],
            [
                BindingTableRow::new(["Bindings", "IX"]),
                BindingTableRow::new(["Renderer", "Core"]),
            ],
            Some(BindingNumber::State(selected.clone())),
            None,
        ),
    ));

    let snapshot = app.render_window(0).unwrap();
    assert!(snapshot.semantics_roles.iter().any(|role| role == "table"));
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|found| found == "Build table")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Renderer")
    );

    name.set("Task table");
    selected.set(0.0);
    let snapshot = app.render_window(0).unwrap();
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|found| found == "Task table")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Bindings")
    );
}

#[test]
fn binding_select_updates_bound_state_from_keyboard() {
    let selected = BindingState::new(0.0);
    let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
    let action = BindingSelectAction::new({
        let changes = Arc::clone(&changes);
        move |index, value| {
            changes.lock().unwrap().push((index, value));
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Select",
        BindingWidget::select(
            "Mode",
            ["Draft", "Final", "Review"],
            Some(BindingNumber::State(selected.clone())),
            Some("Choose mode".to_string()),
            Some(action),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "combo_box")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Draft")
    );
    assert_eq!(selected.get(), BindingValue::Number(0.0));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(20.0, 20.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(20.0, 20.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();
    runtime
        .handle_event(
            window_id,
            BindingEvent::Keyboard(BindingKeyboardEvent::new(
                "ArrowDown",
                BindingKeyState::Pressed,
            )),
        )
        .unwrap();
    runtime
        .handle_event(
            window_id,
            BindingEvent::Keyboard(BindingKeyboardEvent::new("Enter", BindingKeyState::Pressed)),
        )
        .unwrap();

    assert_eq!(selected.get(), BindingValue::Number(1.0));
    assert_eq!(
        changes.lock().unwrap().as_slice(),
        &[(1, "Final".to_string())]
    );
    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Final")
    );
}

#[test]
fn binding_radio_group_updates_bound_state_from_pointer() {
    let selected = BindingState::new(0.0);
    let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
    let action = BindingSelectAction::new({
        let changes = Arc::clone(&changes);
        move |index, value| {
            changes.lock().unwrap().push((index, value));
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Radio group",
        BindingWidget::radio_group(
            "Priority",
            ["Low", "Medium", "High"],
            Some(BindingNumber::State(selected.clone())),
            Some(action),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "radio_group")
    );
    assert!(snapshot.semantics_values.iter().any(|value| value == "Low"));
    assert_eq!(selected.get(), BindingValue::Number(0.0));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(20.0, 52.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(20.0, 52.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert_eq!(selected.get(), BindingValue::Number(1.0));
    assert_eq!(
        changes.lock().unwrap().as_slice(),
        &[(1, "Medium".to_string())]
    );
    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Medium")
    );
}

#[test]
fn binding_segmented_control_updates_bound_state_from_pointer() {
    let selected = BindingState::new(0.0);
    let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
    let action = BindingSelectAction::new({
        let changes = Arc::clone(&changes);
        move |index, value| {
            changes.lock().unwrap().push((index, value));
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Segmented control",
        BindingWidget::segmented_control(
            "View mode",
            [
                BindingSegmentedControlItem::new(
                    "List",
                    Some("Show list view".to_string()),
                    Some("Compact rows".to_string()),
                    false,
                ),
                BindingSegmentedControlItem::new("Gallery", None, None, false),
                BindingSegmentedControlItem::new(
                    "Map",
                    Some("Show map view".to_string()),
                    None,
                    true,
                ),
            ],
            Some(BindingNumber::State(selected.clone())),
            Some(action),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "radio_group")
    );
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Show list view")
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|description| description == "Compact rows")
    );
    assert_eq!(selected.get(), BindingValue::Number(0.0));

    'scan: for y in [4.0, 12.0, 20.0, 32.0, 48.0, 64.0] {
        for x in (0..=2000).step_by(24) {
            let point = Point::new(x as f32, y);
            let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, point);
            down.button = Some(BindingPointerButton::Primary);
            down.buttons = 1;
            runtime
                .handle_event(window_id, BindingEvent::Pointer(down))
                .unwrap();
            let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, point);
            up.button = Some(BindingPointerButton::Primary);
            runtime
                .handle_event(window_id, BindingEvent::Pointer(up))
                .unwrap();
            if selected.get() == BindingValue::Number(1.0) {
                break 'scan;
            }
        }
    }

    assert_eq!(selected.get(), BindingValue::Number(1.0));
    assert_eq!(
        changes.lock().unwrap().as_slice(),
        &[(1, "Gallery".to_string())]
    );
    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Gallery")
    );
    assert!(
        snapshot.semantics_disabled.iter().any(|disabled| *disabled),
        "missing disabled segmented-control item in {:?}",
        snapshot.semantics_disabled
    );
}

#[test]
fn binding_list_view_updates_bound_state_from_pointer() {
    let selected = BindingState::new(0.0);
    let changes = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
    let action = BindingSelectAction::new({
        let changes = Arc::clone(&changes);
        move |index, value| {
            changes.lock().unwrap().push((index, value));
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "List view",
        BindingWidget::list_view(
            "Assets",
            ["Brush", "Canvas", "Export"],
            Some(BindingNumber::State(selected.clone())),
            Some(action),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.semantics_roles.iter().any(|role| role == "list"));
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "list_item")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Brush")
    );
    assert_eq!(selected.get(), BindingValue::Number(0.0));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(44.0, 44.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, Point::new(44.0, 44.0));
    up.button = Some(BindingPointerButton::Primary);
    runtime
        .handle_event(window_id, BindingEvent::Pointer(up))
        .unwrap();

    assert_eq!(selected.get(), BindingValue::Number(1.0));
    assert_eq!(
        changes.lock().unwrap().as_slice(),
        &[(1, "Canvas".to_string())]
    );
    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "Canvas")
    );
    assert!(
        snapshot.semantics_selected.iter().any(|selected| *selected),
        "missing selected list item state in {:?}",
        snapshot.semantics_selected
    );
}

#[test]
fn binding_signal_meter_reads_bound_active_state() {
    let active = BindingState::new(true);
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Signal meter",
        BindingWidget::signal_meter(
            "Input signal",
            active.clone(),
            Some("Live audio input".to_string()),
            8,
            Some(Size::new(76.0, 16.0)),
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.command_count > 0);
    assert!(
        snapshot
            .semantics_roles
            .iter()
            .any(|role| role == "generic_container")
    );
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Input signal")
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|description| description == "Live audio input")
    );
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "active")
    );

    active.set(false);
    assert_eq!(runtime.pending_ui_task_count(), 1);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_values
            .iter()
            .any(|value| value == "idle")
    );
}

#[test]
fn binding_icon_button_reads_bound_state() {
    let selected = BindingState::new(false);
    let enabled = BindingState::new(true);
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Icon button",
        BindingWidget::icon_button(
            IconGlyph::Download,
            "Download",
            selected.clone(),
            enabled.clone(),
            Some(28.0),
            Some(16.0),
            Some("Download file".to_string()),
            None,
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.command_count > 0);
    assert!(snapshot.semantics_roles.iter().any(|role| role == "button"));
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Download")
    );
    assert!(
        snapshot
            .semantics_descriptions
            .iter()
            .any(|description| description == "Download file")
    );
    assert!(!snapshot.semantics_selected.iter().any(|value| *value));
    assert!(!snapshot.semantics_disabled.iter().any(|value| *value));

    selected.set(true);
    enabled.set(false);
    assert_eq!(runtime.pending_ui_task_count(), 2);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 2);
    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot.semantics_selected.iter().any(|value| *value),
        "missing selected icon button state in {:?}",
        snapshot.semantics_selected
    );
    assert!(
        snapshot.semantics_disabled.iter().any(|value| *value),
        "missing disabled icon button state in {:?}",
        snapshot.semantics_disabled
    );
}

#[test]
fn binding_text_input_updates_bound_state_from_keyboard() {
    let text = BindingState::new("");
    let root = BindingWidget::text_input("Name", text.clone(), Some("Type here".to_string()), None);
    let app = BindingApp::new().with_window(BindingWindow::new("Text input", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.command_count > 0);
    assert_eq!(text.get(), BindingValue::String(String::new()));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    runtime
        .handle_event(
            window_id,
            BindingEvent::Keyboard(BindingKeyboardEvent::new("a", BindingKeyState::Pressed)),
        )
        .unwrap();

    assert_eq!(text.get(), BindingValue::String("a".to_string()));
}

#[test]
fn binding_password_input_masks_semantics_and_updates_state_and_action() {
    let text = BindingState::new("");
    let changed = Arc::new(Mutex::new(None::<String>));
    let action = BindingStringAction::new({
        let changed = Arc::clone(&changed);
        move |value| {
            *changed.lock().unwrap() = Some(value);
            Ok(())
        }
    });
    let root = BindingWidget::password_input(
        "Password",
        text.clone(),
        Some("Enter a password".to_string()),
        Some(action),
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Password input", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    let password_index = snapshot
        .semantics_names
        .iter()
        .position(|name| name == "Password")
        .expect("password input semantics");
    assert_eq!(snapshot.semantics_values[password_index], "");

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    runtime
        .handle_event(
            window_id,
            BindingEvent::Keyboard(BindingKeyboardEvent::new("s", BindingKeyState::Pressed)),
        )
        .unwrap();

    assert_eq!(text.get(), BindingValue::String("s".to_string()));
    assert_eq!(changed.lock().unwrap().as_deref(), Some("s"));
    let snapshot = runtime.render_window(window_id).unwrap();
    let password_index = snapshot
        .semantics_names
        .iter()
        .position(|name| name == "Password")
        .expect("password input semantics");
    assert_eq!(snapshot.semantics_values[password_index], "•");
    assert!(!snapshot.semantics_values.iter().any(|value| value == "s"));

    text.set("sëcret");
    let snapshot = runtime.render_window(window_id).unwrap();
    let password_index = snapshot
        .semantics_names
        .iter()
        .position(|name| name == "Password")
        .expect("password input semantics");
    assert_eq!(snapshot.semantics_values[password_index], "••••••");
    assert!(
        !snapshot
            .semantics_values
            .iter()
            .any(|value| value == "sëcret")
    );
}

#[test]
fn binding_datetime_input_preserves_local_string_and_updates_action() {
    let text = BindingState::new("");
    let changed = Arc::new(Mutex::new(None::<String>));
    let action = BindingStringAction::new({
        let changed = Arc::clone(&changed);
        move |value| {
            *changed.lock().unwrap() = Some(value);
            Ok(())
        }
    });
    let root = BindingWidget::datetime_input("Scheduled for", text.clone(), None, Some(action));
    let app = BindingApp::new().with_window(BindingWindow::new("Date/time input", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Scheduled for")
    );

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    runtime
        .handle_event(
            window_id,
            BindingEvent::Keyboard(BindingKeyboardEvent::new("2", BindingKeyState::Pressed)),
        )
        .unwrap();

    assert_eq!(text.get(), BindingValue::String("2".to_string()));
    assert_eq!(changed.lock().unwrap().as_deref(), Some("2"));

    text.set("2026-07-15 09:30");
    let snapshot = runtime.render_window(window_id).unwrap();
    let datetime_index = snapshot
        .semantics_names
        .iter()
        .position(|name| name == "Scheduled for")
        .expect("date/time input semantics");
    assert_eq!(
        snapshot.semantics_values[datetime_index],
        "2026-07-15 09:30"
    );
}

#[test]
fn binding_text_area_updates_bound_state_from_keyboard() {
    let text = BindingState::new("");
    let root =
        BindingWidget::text_area("Notes", text.clone(), Some("Type notes".to_string()), None);
    let app = BindingApp::new().with_window(BindingWindow::new("Text area", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.command_count > 0);
    assert!(
        snapshot
            .semantics_editable_multiline
            .iter()
            .any(|value| *value)
    );
    assert_eq!(text.get(), BindingValue::String(String::new()));

    let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(32.0, 18.0));
    down.button = Some(BindingPointerButton::Primary);
    down.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(down))
        .unwrap();
    runtime
        .handle_event(
            window_id,
            BindingEvent::Keyboard(BindingKeyboardEvent::new("a", BindingKeyState::Pressed)),
        )
        .unwrap();

    assert_eq!(text.get(), BindingValue::String("a".to_string()));
}

#[test]
fn binding_app_renders_foreign_widget_tree_and_dispatches_events() {
    let callbacks = Arc::new(MockCallbacks::default());
    let root = BindingWidget::column(
        [
            BindingWidget::foreign_arc(callbacks.clone()),
            BindingWidget::label("Tail"),
        ],
        4.0,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Foreign binding", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();

    assert!(snapshot.command_count > 0);
    assert!(snapshot.semantics_count >= 2);
    assert!(snapshot.semantics_disabled.iter().any(|value| *value));
    assert!(snapshot.semantics_hidden.iter().any(|value| *value));
    assert!(snapshot.semantics_hovered.iter().any(|value| *value));
    assert!(snapshot.semantics_selected.iter().any(|value| *value));
    assert!(
        snapshot
            .semantics_expanded
            .iter()
            .any(|value| value == "expanded")
    );
    assert!(callbacks.measures.load(Ordering::Relaxed) >= 1);
    assert_eq!(callbacks.paints.load(Ordering::Relaxed), 1);

    let mut pointer = BindingPointerEvent::new(BindingPointerEventKind::Down, Point::new(8.0, 8.0));
    pointer.button = Some(BindingPointerButton::Primary);
    pointer.buttons = 1;
    runtime
        .handle_event(window_id, BindingEvent::Pointer(pointer))
        .unwrap();

    assert_eq!(callbacks.events.load(Ordering::Relaxed), 1);
}

#[test]
fn binding_app_registers_app_level_image_resources() {
    let mut app = BindingApp::new();
    let image = app
        .register_rgba_image(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap();
    let root = BindingWidget::foreign(AppImageCallbacks { image });
    app.push_window(BindingWindow::new("Image resource", root));

    assert_eq!(app.image_resource_count(), 1);

    let snapshot = app.render_window(0).unwrap();

    assert_eq!(snapshot.draw_image_count, 1);
    assert!(snapshot.registered_image_count >= 1);
}

#[test]
fn binding_app_renders_high_level_image_widget() {
    let mut app = BindingApp::new();
    let image = app
        .register_rgba_image(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap();
    let root = BindingWidget::image(
        image,
        Some("Preview".to_string()),
        BindingImageFit::Contain,
        Some(Size::new(32.0, 16.0)),
    );
    app.push_window(BindingWindow::new("Image widget", root));

    let snapshot = app.render_window(0).unwrap();

    assert_eq!(snapshot.draw_image_count, 1);
    assert!(snapshot.registered_image_count >= 1);
    assert!(snapshot.semantics_roles.iter().any(|role| role == "image"));
    assert!(
        snapshot
            .semantics_names
            .iter()
            .any(|name| name == "Preview")
    );
}

#[test]
fn binding_app_registers_app_level_png_resources() {
    let mut app = BindingApp::new();
    let png = test_png_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]);
    let image = app.register_png_image(png).unwrap();
    let root = BindingWidget::foreign(AppImageCallbacks { image });
    app.push_window(BindingWindow::new("PNG resource", root));

    assert_eq!(app.image_resource_count(), 1);

    let snapshot = app.render_window(0).unwrap();

    assert_eq!(snapshot.draw_image_count, 1);
    assert!(snapshot.registered_image_count >= 1);
}

#[test]
fn binding_app_registers_app_level_font_resources() {
    let mut app = BindingApp::new();
    let font = app.register_font_bytes(vec![0, 1, 2, 3]).unwrap();
    assert!(font.get() > 0);
    app.push_window(BindingWindow::new("Fonts", BindingWidget::label("Text")));

    assert_eq!(app.font_resource_count(), 1);

    let snapshot = app.render_window(0).unwrap();

    assert_eq!(snapshot.registered_font_count, 1);
}

#[test]
fn binding_external_surface_draws_cpu_fallback() {
    let texture = ExternalTextureDescriptor::cpu_rgba8(
        Size::new(2.0, 1.0),
        vec![255, 0, 0, 255, 0, 0, 255, 255],
        3,
    );
    let root = BindingWidget::external_surface(
        texture,
        Some(Size::new(64.0, 32.0)),
        Some("External preview".to_string()),
    )
    .unwrap();
    let app = BindingApp::new().with_window(BindingWindow::new("External", root));

    let snapshot = app.render_window(0).unwrap();

    assert_eq!(snapshot.draw_image_count, 1);
    assert!(snapshot.registered_image_count >= 1);
    assert_eq!(snapshot.semantics_count, 1);
}

#[test]
fn binding_runtime_queues_bound_state_updates_and_marks_redraw() {
    let state = BindingState::new("Ready");
    let root = BindingWidget::column(
        [
            BindingWidget::label_state(state.clone()),
            BindingWidget::button("Apply", None),
        ],
        8.0,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Bindings", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    assert!(state.is_ui_bound());
    assert_eq!(runtime.window_count(), 1);
    assert_eq!(runtime.window_ids(), vec![window_id]);
    assert!(runtime.needs_render(window_id).unwrap());

    let initial = runtime.render_window(window_id).unwrap();
    assert!(initial.command_count > 0);
    runtime
        .handle_event(
            window_id,
            BindingEvent::Custom(BindingCustomEvent {
                kind: "binding-smoke".to_string(),
                payload: Some("ok".to_string()),
            }),
        )
        .unwrap();

    let woke = Arc::new(AtomicBool::new(false));
    runtime.set_waker({
        let woke = Arc::clone(&woke);
        move || woke.store(true, Ordering::Relaxed)
    });
    state.set("Updated");

    assert!(woke.load(Ordering::Relaxed));
    assert_eq!(state.label_text(), "Ready");
    assert_eq!(runtime.pending_ui_task_count(), 1);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    assert_eq!(state.label_text(), "Updated");
    assert!(runtime.needs_render(window_id).unwrap());

    let updated = runtime.render_window_at(0).unwrap();
    assert!(updated.command_count > 0);
}

#[test]
fn binding_runtime_external_wake_drains_bound_state_updates() {
    let state = BindingState::new("Idle");
    let root = BindingWidget::label_state(state.clone());
    let app = BindingApp::new().with_window(BindingWindow::new("Wake", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    state.set("Awake");
    assert_eq!(runtime.pending_ui_task_count(), 1);

    runtime.wake_window(window_id).unwrap();

    assert_eq!(runtime.pending_ui_task_count(), 0);
    assert_eq!(state.label_text(), "Awake");
    assert!(runtime.needs_render(window_id).unwrap());
}

#[test]
fn binding_theme_updates_are_live_and_use_the_ui_queue() {
    let theme = BindingTheme::preset("dark").unwrap();
    let mut app = BindingApp::new().with_window(BindingWindow::new(
        "Themed",
        BindingWidget::button("Save", None),
    ));
    app.set_theme(theme.clone());
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    runtime.render_window(window_id).unwrap();
    theme.set_preset("light").unwrap();
    assert_eq!(runtime.pending_ui_task_count(), 1);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    assert!(runtime.needs_render(window_id).unwrap());

    let (interaction_tokens, selection_border) = {
        let snapshot = theme.snapshot();
        (
            (
                snapshot.palette.selection,
                snapshot.palette.surface_focus,
                snapshot.palette.border_focus,
                snapshot.palette.focus_ring,
                snapshot.palette.caret,
            ),
            snapshot.palette.selection_border,
        )
    };
    theme.set_accent(Color::rgba(0.2, 0.5, 0.9, 1.0));
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    assert!(theme.accent().blue > theme.accent().red);
    let updated = theme.snapshot();
    assert_ne!(updated.palette.selection_border, selection_border);
    assert!(updated.palette.selection_border.blue > updated.palette.selection_border.red);
    assert_eq!(
        interaction_tokens,
        (
            updated.palette.selection,
            updated.palette.surface_focus,
            updated.palette.border_focus,
            updated.palette.focus_ring,
            updated.palette.caret,
        )
    );
    theme
        .set_color("success", Color::rgba(0.1, 0.8, 0.3, 1.0))
        .unwrap();
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    assert!(theme.color("success").unwrap().green > 0.7);
    theme.set_number("radius-md", 9.0).unwrap();
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    assert_eq!(theme.number("radius-md").unwrap(), 9.0);
}

#[test]
fn binding_state_observers_are_distinct_and_unsubscribable() {
    let state = BindingState::new(1.0);
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut subscription = state.observe({
        let observed = Arc::clone(&observed);
        move |value| recover_lock(&observed).push(value)
    });

    state.set(1.0);
    assert!(recover_lock(&observed).is_empty());
    state.set(2.0);
    assert_eq!(
        recover_lock(&observed).as_slice(),
        &[BindingValue::Number(2.0)]
    );
    assert!(subscription.unsubscribe());
    assert!(!subscription.unsubscribe());
    state.set(3.0);
    assert_eq!(recover_lock(&observed).len(), 1);
}

#[test]
fn binding_message_bus_posts_named_payloads_to_the_ui_queue() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let mut app = BindingApp::new().with_window(BindingWindow::new(
        "Messages",
        BindingWidget::label("Ready"),
    ));
    app.on_message(
        "background.complete",
        BindingMessageAction::new({
            let received = Arc::clone(&received);
            move |payload| {
                recover_lock(&received).push(payload);
                Ok(())
            }
        }),
    );
    let mut runtime = app.start().unwrap();
    let handle = runtime.ui_handle();

    assert!(handle.emit("background.complete", "Loaded".into()));
    assert!(!handle.emit("unknown", true.into()));
    assert_eq!(runtime.pending_ui_task_count(), 1);
    assert_eq!(runtime.drain_ui_tasks().unwrap(), 1);
    assert_eq!(
        recover_lock(&received).as_slice(),
        &[BindingValue::String("Loaded".to_owned())]
    );
}

#[test]
fn binding_runtime_exposes_renderer_neutral_inspector_counts() {
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Inspector",
        BindingWidget::column(
            [
                BindingWidget::label("Ready"),
                BindingWidget::button("Run", None),
            ],
            4.0,
        ),
    ));
    let mut runtime = app.start().unwrap();
    let window = runtime.window_id_at(0).unwrap();
    runtime.set_inspector_tracing(window, true).unwrap();
    runtime.render_window(window).unwrap();
    runtime
        .handle_event(
            window,
            BindingEvent::Custom(BindingCustomEvent {
                kind: "inspect".to_owned(),
                payload: None,
            }),
        )
        .unwrap();

    let snapshot = runtime.inspector_snapshot(window).unwrap();
    assert!(snapshot.tracing_enabled);
    assert_eq!(snapshot.title, "Inspector");
    assert!(snapshot.widget_count >= 3);
    assert!(snapshot.semantics_count >= 2);
    assert!(snapshot.event_route_count >= 1);
    assert_eq!(snapshot.semantics_nodes.len(), snapshot.semantics_count);
    assert_eq!(snapshot.event_routes.len(), snapshot.event_route_count);
    assert!(
        snapshot
            .event_routes
            .iter()
            .any(|event| event.event_kind == "custom")
    );
}

#[test]
fn adaptive_workspace_bindings_retain_state_and_switch_local_presentations() {
    let classes = Arc::new(Mutex::new(Vec::new()));
    let adaptive = BindingWidget::adaptive_view(
        BindingWidget::label("Compact branch"),
        BindingWidget::label("Medium branch"),
        BindingWidget::label("Expanded branch"),
        300.0,
        600.0,
        Some(BindingStringAction::new({
            let classes = Arc::clone(&classes);
            move |value| {
                recover_lock(&classes).push(value);
                Ok(())
            }
        })),
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Adaptive", adaptive));
    let mut runtime = app.start().unwrap();
    let window = runtime.window_id_at(0).unwrap();

    runtime
        .handle_event(
            window,
            BindingEvent::Window(BindingWindowEvent::Resized(Size::new(200.0, 300.0))),
        )
        .unwrap();
    let compact = runtime.render_window(window).unwrap();
    assert!(
        compact
            .semantics_names
            .iter()
            .any(|name| name == "Compact branch")
    );

    runtime
        .handle_event(
            window,
            BindingEvent::Window(BindingWindowEvent::Resized(Size::new(800.0, 300.0))),
        )
        .unwrap();
    let expanded = runtime.render_window(window).unwrap();
    assert!(
        expanded
            .semantics_names
            .iter()
            .any(|name| name == "Expanded branch")
    );
    assert!(
        recover_lock(&classes)
            .iter()
            .any(|class| class == "expanded")
    );

    let sidebar = BindingResponsiveSidebarState::new(true, false);
    assert!(sidebar.set_expanded(false));
    assert!(!sidebar.expanded());
    assert!(sidebar.open_overlay());
    assert!(sidebar.overlay_open());

    let master_detail = BindingMasterDetailState::new("master").unwrap();
    assert!(master_detail.show_detail());
    assert_eq!(master_detail.route(), "detail");
}

#[test]
fn binding_virtual_list_model_updates_keyed_rows_without_realizing_the_dataset() {
    let items = (1..=500)
        .map(|key| BindingVirtualListItem::new(key, format!("Row {key}")).unwrap())
        .collect::<Vec<_>>();
    let model = BindingVirtualListModel::new("Rows", items).unwrap();
    let root = BindingWidget::virtual_list(
        "Rows",
        model.clone(),
        28.0,
        0.0,
        None,
        None,
        1.0,
        64,
        true,
        false,
        false,
        true,
        None,
        None,
        None,
    );
    let app = BindingApp::new().with_window(BindingWindow::new("Virtual", root));
    let mut runtime = app.start().unwrap();
    let window = runtime.window_id_at(0).unwrap();
    let initial = runtime.render_window(window).unwrap();

    assert_eq!(model.len(), 500);
    assert!(initial.semantics_count < 100);
    assert!(
        model
            .update(BindingVirtualListItem::new(1, "Updated row").unwrap())
            .unwrap()
    );
    let updated = runtime.render_window(window).unwrap();
    assert!(
        updated
            .semantics_names
            .iter()
            .any(|name| name == "Updated row")
    );
}

#[test]
fn binding_pixel_canvas_state_is_thread_safe_and_exports_rgba_bytes() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BindingPixelCanvasState>();

    let state = BindingPixelCanvasState::new();
    state.set_tool("fill").unwrap();
    state.set_brush_color(Color::rgba(0.2, 0.5, 0.9, 1.0));
    state.set_brush_size(2.0);
    state.request_export();
    let root = BindingWidget::pixel_canvas(
        state.clone(),
        "Pixel editor",
        4,
        4,
        None,
        Size::new(240.0, 180.0),
        BindingCanvasViewport::new(Vector::ZERO, 14.0, 0.0),
        true,
        Vec::new(),
    )
    .unwrap();
    let app = BindingApp::new().with_window(BindingWindow::new("Pixels", root));
    let snapshot = app.render_window(0).unwrap();

    assert!(snapshot.command_count > 0);
    let export = state.latest_export().expect("pixel export");
    assert_eq!((export.width, export.height), (4, 4));
    assert_eq!(export.rgba8.len(), 4 * 4 * 4);
}

#[test]
fn binding_render_options_validate_host_facing_names() {
    let options = BindingRenderOptions::new(
        true,
        -2.0,
        true,
        "display-p3",
        "hdr",
        "reinhard",
        "prefer-hdr",
        203.0,
        true,
    )
    .unwrap();
    assert_eq!(options.feather_width(), 0.0);
    assert!(
        BindingRenderOptions::new(
            true, 1.0, true, "rec2020", "auto", "auto", "auto", 203.0, true,
        )
        .is_err()
    );
}

#[test]
fn binding_window_retains_geometry_and_icon_configuration() {
    let window = BindingWindow::new("Configured", BindingWidget::label("Ready"))
        .with_initial_size(Size::new(800.0, 600.0))
        .with_initial_position(Point::new(40.0, 60.0))
        .without_icon();
    assert_eq!(window.initial_size(), Some(Size::new(800.0, 600.0)));
    assert_eq!(window.initial_position(), Some(Point::new(40.0, 60.0)));
}

#[test]
fn raw_mouse_motion_and_window_movement_round_trip() {
    let raw = BindingEvent::RawMouseMotion(BindingRawMouseMotionEvent {
        delta: Vector::new(3.0, -2.0),
        modifiers: BindingModifiers {
            shift: true,
            ..BindingModifiers::default()
        },
    });
    let Event::RawMouseMotion(raw) = raw.into_sui_event().unwrap() else {
        panic!("raw mouse motion event expected");
    };
    assert_eq!(raw.delta, Vector::new(3.0, -2.0));
    assert!(raw.modifiers.shift);

    let moved = BindingEvent::from(&Event::Window(WindowEvent::Moved(Point::new(9.0, 12.0))));
    assert!(matches!(
        moved,
        BindingEvent::Window(BindingWindowEvent::Moved(Point { x: 9.0, y: 12.0 }))
    ));
}

#[test]
fn binding_dock_workspace_preserves_portable_state_and_panels() {
    let layout = BindingDockLayout::new(BindingDockNode::tabs([1, 2], Some(1)).unwrap(), [], []);
    let state = BindingDockState::new(layout).unwrap();
    let workspace = BindingWidget::dock_workspace(
        state.clone(),
        [
            BindingDockPanel::new(1, "Files", BindingWidget::label("Files panel")).unwrap(),
            BindingDockPanel::new(2, "Search", BindingWidget::label("Search panel")).unwrap(),
        ],
        "Editor workspace",
    )
    .unwrap();
    let app = BindingApp::new().with_window(BindingWindow::new("Docking", workspace));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let snapshot = runtime.render_window(window_id).unwrap();
    assert!(snapshot.semantics_names.iter().any(|name| name == "Files"));
    assert!(state.activate(2).unwrap());
    assert!(state.hide(1).unwrap());
    assert!(state.snapshot().hidden.contains(&1));
    assert!(state.show(1).unwrap());
    assert!(!state.snapshot().hidden.contains(&1));
}

#[test]
fn binding_rich_document_streams_across_threads_and_renders_structured_blocks() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BindingRichDocument>();

    let document = BindingRichDocument::new("# Report\n\nWaiting");
    let root = BindingWidget::rich_document(document.clone(), None, None, None);
    let app = BindingApp::new().with_window(BindingWindow::new("Document", root));
    let mut runtime = app.start().unwrap();
    let window_id = runtime.window_id_at(0).unwrap();

    let initial = runtime.render_window(window_id).unwrap();
    assert!(initial.semantics_names.iter().any(|name| name == "Report"));

    let producer = document.clone();
    std::thread::spawn(move || {
        producer.append_markdown("\n\n```text\nready\n```");
    })
    .join()
    .unwrap();
    let update = document.last_update();
    assert!(update.append_only);
    assert!(update.reparsed_end > update.reparsed_start);

    let attachment = document.append_attachment(
        "trace.json",
        Some("application/json".to_owned()),
        Some("artifact:trace".to_owned()),
        Some(128),
        Some("Execution trace".to_owned()),
    );
    assert!(attachment > 0);
    let extension = document
        .append_extension(
            "tool-call",
            "Build",
            Some("Completed".to_owned()),
            "cargo test",
            "success",
            false,
            vec![("exit_code".to_owned(), "0".to_owned())],
        )
        .unwrap();
    assert!(extension > attachment);

    let updated = runtime.render_window(window_id).unwrap();
    assert!(updated.semantics_names.iter().any(|name| name == "Build"));
}

#[test]
fn portable_animation_values_timelines_documents_and_editor_round_trip() {
    let zero = BindingAnimationValue::scalar(0.0);
    let ten = BindingAnimationValue::scalar(10.0);
    let transition = BindingTransition::new(zero, ten, 0.0, 1.0, Easing::Linear);
    assert_eq!(transition.sample(0.5).as_scalar(), Some(5.0));

    let mut track = BindingAnimationTrack::new("card", "layer.opacity");
    track.add_keyframe(BindingAnimationKeyframe::new(0.0, zero, Easing::Linear));
    track.add_keyframe(BindingAnimationKeyframe::new(1.0, ten, Easing::Linear));
    let mut clip = BindingAnimationClip::new("fade", 0.0, 1.0);
    clip.add_track(track);
    let mut timeline = BindingAnimationTimeline::new(1.0);
    timeline.add_clip(clip);
    let samples = timeline.sample(0.5);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].target, "card");
    assert_eq!(samples[0].value.as_scalar(), Some(5.0));

    let document = BindingAnimationDocument::new("Card motion", timeline.clone());
    let encoded = document.to_document_format();
    let decoded = BindingAnimationDocument::parse(&encoded).unwrap();
    assert_eq!(decoded.name(), "Card motion");
    assert_eq!(
        decoded.timeline().sample(0.5)[0].value.as_scalar(),
        Some(5.0)
    );

    let mut player = BindingAnimationPlayer::new(&timeline);
    player.play();
    assert_eq!(player.tick(0.25)[0].value.as_scalar(), Some(2.5));

    let mut editor = BindingAnimationEditor::new(decoded);
    assert!(editor.add_keyframe(
        0,
        0,
        BindingAnimationKeyframe::new(0.75, ten, Easing::EaseOut),
    ));
    assert!(editor.can_undo());
    assert!(editor.undo());
    assert!(editor.can_redo());
}

#[test]
fn semantic_queries_and_locator_style_actions_drive_the_runtime() {
    let pressed = Arc::new(AtomicBool::new(false));
    let action = BindingAction::new({
        let pressed = Arc::clone(&pressed);
        move || {
            pressed.store(true, Ordering::Relaxed);
            Ok(())
        }
    });
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Testing",
        BindingWidget::button("Save", Some(action)),
    ));
    let mut runtime = app.start().unwrap();
    let snapshot = runtime.render_window_at(0).unwrap();
    let node = snapshot
        .get_one(Some("button"), Some("Save"), None)
        .unwrap();
    assert!(node.visible());
    assert_eq!(
        snapshot
            .find_nodes(Some("button"), None, None, None, None, Some(true))
            .len(),
        1
    );

    runtime.click_node_at(0, &node).unwrap();
    assert!(pressed.load(Ordering::Relaxed));
}

#[cfg(not(feature = "desktop"))]
#[test]
fn binding_app_run_reports_missing_desktop_feature() {
    let app = BindingApp::new().with_window(BindingWindow::new(
        "Headless",
        BindingWidget::label("No desktop"),
    ));

    assert!(app.run().unwrap_err().contains("desktop"));
    assert!(app.run_with_handle(|_| {}).unwrap_err().contains("desktop"));
}
