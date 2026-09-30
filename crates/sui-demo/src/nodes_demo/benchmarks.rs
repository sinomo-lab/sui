//! Diagnostics for the node graph editor: profiles of the page itself, and
//! benchmarks of animation, zoom, and retained edges on larger graphs. They
//! are ignored by default; run them serially on an idle machine.

use sui::{WgpuRenderer, prelude::*};
use sui_nodes::{
    BackgroundVariant, Edge, FitViewOptions, Node, NodeGraph, NodeGraphConfig, NodeGraphState,
    Viewport,
};

use super::{NODES_MAIN_GRAPH_NAME, NODES_TAB_LABEL};

/// Node data for the benchmark graphs.
#[derive(Debug, Clone, PartialEq, Default)]
struct DemoNodeData {
    detail: String,
}

impl DemoNodeData {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

/// Edge data for the benchmark graphs.
#[derive(Debug, Clone, PartialEq, Default)]
struct DemoEdgeData {
    note: String,
}

impl DemoEdgeData {
    fn new(note: impl Into<String>) -> Self {
        Self { note: note.into() }
    }
}

#[test]
#[ignore = "diagnostic profiling of the actual small node graph demo"]
fn small_node_demo_paint_profile() -> Result<()> {
    use std::collections::BTreeMap;
    use std::time::Instant;
    const FRAMES: usize = 90;
    const REPAINT: sui::CommandKey<sui::WidgetId> = sui::CommandKey::new("nodes.profile.repaint");
    let document = super::lab::lab_document();
    let mut runtime = crate::app::build_dev_application_with_initial_demo_and_render_options(
        Some(NODES_TAB_LABEL),
        sui::WindowRenderOptions::new(true, 1.0),
    )
    .on_command(REPAINT, |ctx, id| {
        ctx.request(sui::InvalidationRequest::new(
            sui::InvalidationTarget::Widget(*id),
            sui::InvalidationKind::Paint,
        ));
    })
    .build()?;
    let window_id = runtime.window_ids()[0];
    sui::set_window_scene_statistics_detail_mode(
        window_id,
        sui::SceneStatisticsDetailMode::Detailed,
    );
    runtime.handle_event(
        window_id,
        Event::Window(sui::WindowEvent::Resized(Size::new(1440.0, 900.0))),
    )?;
    let initial = runtime.render(window_id)?;
    let mut renderer = WgpuRenderer::new();
    renderer.render(&initial.frame)?;
    println!(
        "SMALL_NODE_DEMO nodes={} edges={} widgets={} size=1440x900",
        document.nodes.len(),
        document.edges.len(),
        initial.diagnostics.widget_count
    );
    println!("SMALL_NODE_ADAPTER {:?}", renderer.adapter_info());
    assert!(
        initial
            .semantics
            .iter()
            .any(|node| node.name.as_deref() == Some(NODES_MAIN_GRAPH_NAME))
    );
    let main_graph = initial
        .semantics
        .iter()
        .find(|node| node.name.as_deref() == Some(NODES_MAIN_GRAPH_NAME))
        .unwrap();
    let graph_id = main_graph.id;
    let graph_corner = Point::new(main_graph.bounds.x() + 16.0, main_graph.bounds.y() + 16.0);
    println!(
        "SMALL_NODE_GRAPH bounds={:?} value={:?}",
        main_graph.bounds, main_graph.value
    );
    let mut frame_time = 0.0;
    for mode in [
        "unchanged",
        "animation",
        "graph-repaint",
        "full-repaint",
        "zoom",
        "full-repaint-no-renderer-diagnostics",
    ] {
        renderer.set_runtime_diagnostics_enabled(mode != "full-repaint-no-renderer-diagnostics");
        let mut totals = BTreeMap::<String, f64>::new();
        let mut widgets = BTreeMap::<String, f64>::new();
        let mut samples = Vec::new();
        for frame in 0..FRAMES + 20 {
            // Use real scheduled wakes so the graph (nested in the demo
            // shell) receives the same target-only events as on desktop.
            // Keep offscreen submissions paced; an unbounded loop measures
            // GPU queue backpressure as well as painting work.
            std::thread::sleep(std::time::Duration::from_millis(17));
            frame_time += 1.0 / 60.0;
            let events = if mode == "unchanged" {
                Vec::new()
            } else {
                runtime.tick(frame_time);
                runtime.drain_ready_events()
            };
            let started = Instant::now();
            for (id, event) in events {
                runtime.handle_event(id, event)?;
            }
            runtime.handle_event(window_id, Event::Window(sui::WindowEvent::RedrawRequested))?;
            if mode == "zoom" {
                let mut wheel = sui::PointerEvent::new(sui::PointerEventKind::Scroll, graph_corner);
                wheel.scroll_delta = Some(sui::ScrollDelta::Pixels(sui::Vector::new(
                    0.0,
                    if frame % 2 == 0 { 3.0 } else { -3.0 },
                )));
                runtime.handle_event(window_id, Event::Pointer(wheel))?;
            } else if mode != "animation" && mode != "unchanged" {
                let target = if mode == "graph-repaint" {
                    graph_id
                } else {
                    runtime.widget_graph(window_id)?.root
                };
                runtime.command_sender().send_application(REPAINT, target);
                runtime.process_commands();
            }
            let output = runtime.render(window_id)?;
            let runtime_ms = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            renderer.render(&output.frame)?;
            let renderer_ms = started.elapsed().as_secs_f64() * 1000.0;
            if frame < 20 {
                continue;
            }
            samples.push(runtime_ms + renderer_ms);
            *totals.entry("runtime_ms".into()).or_default() += runtime_ms;
            *totals.entry("renderer_ms".into()).or_default() += renderer_ms;
            *totals.entry("animation_wakes".into()).or_default() +=
                output.diagnostics.animation_frame_wake_count as f64;
            for phase in &output.diagnostics.phase_timings {
                *totals
                    .entry(format!("runtime_{}_ms", phase.phase.label()))
                    .or_default() += phase.duration_ms;
            }
            for timing in &output.diagnostics.widget_timings {
                if timing.phase.label() == "Paint" {
                    *widgets.entry(timing.widget_name.to_string()).or_default() +=
                        timing.duration_ms;
                }
            }
            let stats = renderer.last_frame_stats(window_id).unwrap();
            for (name, value) in [
                ("traversal_us", stats.retained_scene_traversal_time_us),
                ("packet_build_us", stats.retained_packet_build_time_us),
                ("path_us", stats.retained_packet_path_command_time_us),
                ("text_us", stats.retained_packet_text_command_time_us),
                ("resources_us", stats.resource_collection_time_us),
                ("bind_groups_us", stats.bind_group_prepare_time_us),
                ("batch_us", stats.batch_prepare_time_us),
                ("upload_us", stats.gpu_upload_time_us),
                ("encode_us", stats.pass_encode_time_us),
                ("submit_us", stats.queue_submit_time_us),
                ("command_finish_us", stats.command_finish_time_us),
                ("state_update_us", stats.retained_state_update_time_us),
                ("composition_us", stats.composition_time_us),
                ("draws", stats.draw_count as u64),
                ("direct_packets", stats.direct_packet_count as u64),
                ("prepared_hits", stats.prepared_fragment_cache_hits as u64),
                (
                    "prepared_builds",
                    stats.prepared_fragment_build_count as u64,
                ),
                ("snapshot_replayed", stats.snapshot_commands_replayed as u64),
                ("snapshot_reused", stats.snapshot_commands_reused as u64),
                ("vertex_upload_bytes", stats.uploaded_vertex_bytes),
                (
                    "path_misses",
                    stats.analytic_path_bind_group_miss_count as u64,
                ),
                (
                    "path_upload_bytes",
                    stats.analytic_path_bind_group_upload_bytes,
                ),
                ("packet_builds", stats.retained_packet_build_count as u64),
            ] {
                *totals.entry(name.into()).or_default() += value as f64;
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "SMALL_NODE_DEMO mode={mode} p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            samples[FRAMES / 2],
            samples[FRAMES * 95 / 100],
            samples[FRAMES - 1]
        );
        assert_eq!(
            totals["animation_wakes"],
            if mode == "unchanged" {
                0.0
            } else {
                FRAMES as f64
            }
        );
        if mode == "graph-repaint" && std::env::var_os("SUI_PROFILE_WIDGET_TIMINGS").is_some() {
            assert!(widgets.iter().any(|(name, duration)| {
                name.contains("sui_nodes::widget::NodeGraph<") && *duration > 0.0
            }));
        }
        for (name, value) in totals {
            println!("  {name}={:.3}", value / FRAMES as f64);
        }
        let mut widgets = widgets.into_iter().collect::<Vec<_>>();
        widgets.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (name, value) in widgets.into_iter().take(15) {
            println!("  paint_widget_avg_ms={:.3} {name}", value / FRAMES as f64);
        }
    }
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for node graph animation runtime and GPU work"]
fn node_graph_animation_gpu_benchmark() -> Result<()> {
    const FRAMES: usize = 120;
    for animated_edges in [1, 368] {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for row in 0..16 {
            for column in 0..24 {
                let index = row * 24 + column;
                nodes.push(
                    Node::new(
                        format!("node-{index}"),
                        Point::new(column as f32 * 120.0, row as f32 * 64.0),
                        (),
                    )
                    .label(format!("Node {index}"))
                    .size(Size::new(100.0, 48.0)),
                );
                if column > 0 {
                    let animated = edges.len() < animated_edges;
                    edges.push(
                        Edge::new(
                            format!("edge-{index}"),
                            format!("node-{}", index - 1),
                            format!("node-{index}"),
                            (),
                        )
                        .animated(animated)
                        .animation_speed(0.75),
                    );
                }
            }
        }
        let state = NodeGraphState::new(nodes, edges).unwrap();
        let graph = NodeGraph::new("Animation benchmark", state.clone());
        let mut runtime = Application::new()
            .window(
                WindowBuilder::new()
                    .title("Animation benchmark")
                    .root(graph),
            )
            .build()?;
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id)?;
        state.fit_view(state.viewport_size(), FitViewOptions::default());
        let initial = runtime.render(window_id)?;
        let mut renderer = WgpuRenderer::new();
        renderer.render(&initial.frame)?;
        let mut runtime_ms = 0.0;
        let mut renderer_ms = 0.0;
        let mut packets = 0;
        let mut paths = 0;
        let mut text = 0;
        for frame in 0..FRAMES + 20 {
            let started = std::time::Instant::now();
            runtime.handle_event(
                window_id,
                Event::Wake(sui::WakeEvent::AnimationFrame {
                    time: frame as f64 / 60.0,
                    delta: 1.0 / 60.0,
                    frame_index: frame as u64,
                }),
            )?;
            let output = runtime.render(window_id)?;
            let runtime_elapsed = started.elapsed();
            let started = std::time::Instant::now();
            renderer.render(&output.frame)?;
            let renderer_elapsed = started.elapsed();
            if frame >= 20 {
                runtime_ms += runtime_elapsed.as_secs_f64() * 1000.0;
                renderer_ms += renderer_elapsed.as_secs_f64() * 1000.0;
                let stats = renderer.last_frame_stats(window_id).unwrap();
                packets += stats.retained_packet_build_count;
                paths += stats.retained_packet_path_command_count;
                text += stats.retained_packet_text_command_count;
            }
        }
        println!(
            "NODE_ANIMATION nodes=384 animated_edges={animated_edges} frames={FRAMES} runtime_avg_ms={:.3} renderer_avg_ms={:.3} packet_build_avg={:.1} rebuilt_paths_avg={:.1} rebuilt_text_avg={:.1}",
            runtime_ms / FRAMES as f64,
            renderer_ms / FRAMES as f64,
            packets as f64 / FRAMES as f64,
            paths as f64 / FRAMES as f64,
            text as f64 / FRAMES as f64
        );
        assert_eq!(paths, 0, "particle frames must reuse static curve paths");
        assert_eq!(text, 0, "particle frames must reuse node labels");
    }
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for retained node graph runtime and GPU zoom frames"]
fn retained_node_graph_gpu_zoom_current_status_benchmark() -> Result<()> {
    const COLUMNS: usize = 24;
    const ROWS: usize = 16;
    const FRAMES: usize = 120;
    let mut nodes = Vec::with_capacity(COLUMNS * ROWS);
    let mut edges = Vec::new();
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let index = row * COLUMNS + column;
            nodes.push(
                Node::new(
                    format!("gpu-node-{index}"),
                    Point::new(column as f32 * 120.0, row as f32 * 64.0),
                    DemoNodeData::new(format!("item {index}")),
                )
                .kind("gpu-benchmark")
                .label(format!("Node {index}"))
                .size(Size::new(100.0, 48.0)),
            );
            if column > 0 {
                edges.push(Edge::new(
                    format!("gpu-edge-{index}"),
                    format!("gpu-node-{}", index - 1),
                    format!("gpu-node-{index}"),
                    DemoEdgeData::new("benchmark"),
                ));
            }
        }
    }
    let state = NodeGraphState::new(nodes, edges).expect("valid GPU benchmark graph");
    let graph = NodeGraph::new("GPU node benchmark", state.clone())
        .config(NodeGraphConfig {
            min_zoom: 0.1,
            max_zoom: 2.0,
            background_variant: BackgroundVariant::Dots,
            ..NodeGraphConfig::default()
        })
        .node_type("gpu-benchmark", |_id, node| {
            let title = node.select_named("GPU benchmark node title", |node| node.label.clone());
            Padding::all(6.0, Label::new("").text_from(title)).fill_child()
        });
    let mut runtime = Application::new()
        .window(WindowBuilder::new().title("GPU node benchmark").root(graph))
        .build()?;
    let window_id = runtime.window_ids()[0];
    let detailed_profile = std::env::var_os("SUI_NODE_BENCH_PROFILE").is_some();
    if detailed_profile {
        sui::set_window_scene_statistics_detail_mode(
            window_id,
            sui::SceneStatisticsDetailMode::Detailed,
        );
    }
    let initial = runtime.render(window_id)?;
    let mut renderer = WgpuRenderer::new();
    renderer.render(&initial.frame)?;
    let viewport_size = state.viewport_size();
    let center = Point::new(1_430.0, 510.0);
    let mut runtime_time = std::time::Duration::ZERO;
    let mut renderer_time = std::time::Duration::ZERO;
    let mut draw_count = 0usize;
    let mut path_misses = 0usize;
    let mut path_upload_bytes = 0u64;
    let mut scene_traversal_us = 0u64;
    let mut packet_build_us = 0u64;
    let mut packet_scene_build_us = 0u64;
    let mut packet_text_us = 0u64;
    let mut packet_path_us = 0u64;
    let mut packet_rect_us = 0u64;
    let mut resource_collection_us = 0u64;
    let mut bind_group_us = 0u64;
    let mut batch_prepare_us = 0u64;
    let mut gpu_upload_us = 0u64;
    let mut encode_us = 0u64;
    let mut queue_submit_us = 0u64;
    let mut vertex_upload_bytes = 0u64;
    let mut text_vertex_bytes = 0u64;
    let mut text_glyph_instances = 0usize;
    let mut visible_layers = 0usize;
    let mut packet_builds = 0usize;
    let mut packet_new = 0usize;
    let mut packet_signature = 0usize;
    let mut packet_scene = 0usize;
    let mut packet_state = 0usize;
    let mut measure_arrange_ms = 0.0_f64;
    let mut hit_test_ms = 0.0_f64;
    let mut paint_ms = 0.0_f64;
    let mut semantics_ms = 0.0_f64;
    let mut widget_arrange_ms = 0.0_f64;
    let mut widget_paint_ms = 0.0_f64;
    let mut widget_semantics_ms = 0.0_f64;

    for frame in 0..FRAMES {
        let zoom = 0.32 + ((frame % 17) as f32 * 0.006);
        if detailed_profile {
            runtime.handle_event(window_id, Event::Window(sui::WindowEvent::RedrawRequested))?;
        }
        state.set_viewport(Viewport::centered_on(center, viewport_size, zoom, 0.1, 2.0));
        let runtime_started = std::time::Instant::now();
        let output = runtime.render(window_id)?;
        runtime_time += runtime_started.elapsed();
        for phase in &output.diagnostics.phase_timings {
            match phase.phase {
                sui::FramePhase::MeasureArrange => measure_arrange_ms += phase.duration_ms,
                sui::FramePhase::HitTest => hit_test_ms += phase.duration_ms,
                sui::FramePhase::Paint => paint_ms += phase.duration_ms,
                sui::FramePhase::Semantics => semantics_ms += phase.duration_ms,
                _ => {}
            }
        }
        for timing in &output.diagnostics.widget_timings {
            match timing.phase.label() {
                "Arrange" => widget_arrange_ms += timing.duration_ms,
                "Paint" => widget_paint_ms += timing.duration_ms,
                "Semantics" => {
                    widget_semantics_ms += timing.duration_ms;
                }
                _ => {}
            }
        }
        let renderer_started = std::time::Instant::now();
        renderer.render(&output.frame)?;
        renderer_time += renderer_started.elapsed();
        let stats = renderer
            .last_frame_stats(window_id)
            .expect("renderer frame stats");
        draw_count += stats.draw_count;
        path_misses += stats.analytic_path_bind_group_miss_count;
        path_upload_bytes += stats.analytic_path_bind_group_upload_bytes;
        scene_traversal_us += stats.retained_scene_traversal_time_us;
        packet_build_us += stats.retained_packet_build_time_us;
        packet_scene_build_us += stats.retained_packet_scene_build_time_us;
        packet_text_us += stats.retained_packet_text_command_time_us;
        packet_path_us += stats.retained_packet_path_command_time_us;
        packet_rect_us += stats.retained_packet_rect_command_time_us;
        resource_collection_us += stats.resource_collection_time_us;
        bind_group_us += stats.bind_group_prepare_time_us;
        batch_prepare_us += stats.batch_prepare_time_us;
        gpu_upload_us += stats.gpu_upload_time_us;
        encode_us += stats.pass_encode_time_us;
        queue_submit_us += stats.queue_submit_time_us;
        vertex_upload_bytes += stats.uploaded_vertex_bytes;
        text_vertex_bytes += stats.text_vertex_bytes;
        text_glyph_instances += stats.text_glyph_instance_count;
        visible_layers += stats.visible_layer_count;
        packet_builds += stats.retained_packet_build_count;
        packet_new += stats.retained_packet_rebuilds.new_count;
        packet_signature += stats.retained_packet_rebuilds.signature_count;
        packet_scene += stats.retained_packet_rebuilds.scene_count;
        packet_state += stats.retained_packet_rebuilds.state_count;
    }

    println!(
        "NODE_GRAPH_GPU_ZOOM_BENCHMARK nodes=384 edges=368 frames={FRAMES} detailed={detailed_profile} runtime_avg_ms={:.3} renderer_avg_ms={:.3} total_avg_ms={:.3} draw_avg={:.1} path_miss_avg={:.1} path_upload_avg_bytes={:.1} scene_traversal_avg_us={:.1} packet_build_avg_us={:.1} packet_scene_build_avg_us={:.1} packet_text_avg_us={:.1} packet_path_avg_us={:.1} packet_rect_avg_us={:.1} resource_collection_avg_us={:.1} bind_group_avg_us={:.1} batch_prepare_avg_us={:.1} gpu_upload_avg_us={:.1} encode_avg_us={:.1} queue_submit_avg_us={:.1} vertex_upload_avg_bytes={:.1} text_vertex_avg_bytes={:.1} text_glyph_avg={:.1} visible_layer_avg={:.1} packet_build_avg={:.1} packet_new={packet_new} packet_signature={packet_signature} packet_scene={packet_scene} packet_state={packet_state} measure_arrange_avg_ms={:.3} hit_test_avg_ms={:.3} paint_avg_ms={:.3} semantics_avg_ms={:.3} widget_arrange_avg_ms={:.3} widget_paint_avg_ms={:.3} widget_semantics_avg_ms={:.3}",
        runtime_time.as_secs_f64() * 1000.0 / FRAMES as f64,
        renderer_time.as_secs_f64() * 1000.0 / FRAMES as f64,
        (runtime_time + renderer_time).as_secs_f64() * 1000.0 / FRAMES as f64,
        draw_count as f64 / FRAMES as f64,
        path_misses as f64 / FRAMES as f64,
        path_upload_bytes as f64 / FRAMES as f64,
        scene_traversal_us as f64 / FRAMES as f64,
        packet_build_us as f64 / FRAMES as f64,
        packet_scene_build_us as f64 / FRAMES as f64,
        packet_text_us as f64 / FRAMES as f64,
        packet_path_us as f64 / FRAMES as f64,
        packet_rect_us as f64 / FRAMES as f64,
        resource_collection_us as f64 / FRAMES as f64,
        bind_group_us as f64 / FRAMES as f64,
        batch_prepare_us as f64 / FRAMES as f64,
        gpu_upload_us as f64 / FRAMES as f64,
        encode_us as f64 / FRAMES as f64,
        queue_submit_us as f64 / FRAMES as f64,
        vertex_upload_bytes as f64 / FRAMES as f64,
        text_vertex_bytes as f64 / FRAMES as f64,
        text_glyph_instances as f64 / FRAMES as f64,
        visible_layers as f64 / FRAMES as f64,
        packet_builds as f64 / FRAMES as f64,
        measure_arrange_ms / FRAMES as f64,
        hit_test_ms / FRAMES as f64,
        paint_ms / FRAMES as f64,
        semantics_ms / FRAMES as f64,
        widget_arrange_ms / FRAMES as f64,
        widget_paint_ms / FRAMES as f64,
        widget_semantics_ms / FRAMES as f64,
    );
    Ok(())
}

fn run_edge_world_benchmark(retained: bool) -> (f64, f64, f64, f64) {
    const EDGE_COUNT: usize = 1_024;
    const FRAMES: usize = 60;
    let mut nodes = Vec::with_capacity(EDGE_COUNT + 1);
    let mut edges = Vec::with_capacity(EDGE_COUNT);
    for index in 0..=EDGE_COUNT {
        let mut node = Node::new(
            format!("edge-world-node-{index}"),
            Point::new((index % 64) as f32 * 32.0, (index / 64) as f32 * 32.0),
            DemoNodeData::new("edge world"),
        )
        .size(Size::new(24.0, 20.0));
        node.handles.clear();
        nodes.push(node);
        if index > 0 {
            edges.push(Edge::new(
                format!("edge-world-{index}"),
                format!("edge-world-node-{}", index - 1),
                format!("edge-world-node-{index}"),
                DemoEdgeData::new("retained edge world"),
            ));
        }
    }
    let state = NodeGraphState::new(nodes, edges).expect("valid edge-world benchmark graph");
    let graph = NodeGraph::new("Edge world benchmark", state.clone())
        .config(NodeGraphConfig {
            min_zoom: 0.05,
            max_zoom: 2.0,
            retain_edge_world: retained,
            retained_edge_world_min: 0,
            ..NodeGraphConfig::default()
        })
        .node_painter(|_ctx, _node, _paint| {});
    let mut runtime = Application::new()
        .window(
            WindowBuilder::new()
                .title("Edge world benchmark")
                .root(graph),
        )
        .build()
        .expect("edge-world runtime");
    let window_id = runtime.window_ids()[0];
    let initial = runtime.render(window_id).expect("initial edge-world frame");
    let mut renderer = WgpuRenderer::new();
    renderer
        .render(&initial.frame)
        .expect("initial edge-world GPU frame");
    let viewport_size = state.viewport_size();
    let center = Point::new(1_008.0, 256.0);
    let mut runtime_time = std::time::Duration::ZERO;
    let mut renderer_time = std::time::Duration::ZERO;
    let mut packet_builds = 0usize;
    let mut path_time_us = 0u64;
    for frame in 0..FRAMES {
        let zoom = 0.42 + ((frame % 13) as f32 * 0.008);
        state.set_viewport(Viewport::centered_on(
            center,
            viewport_size,
            zoom,
            0.05,
            2.0,
        ));
        let runtime_started = std::time::Instant::now();
        let output = runtime.render(window_id).expect("edge-world runtime frame");
        runtime_time += runtime_started.elapsed();
        let renderer_started = std::time::Instant::now();
        renderer
            .render(&output.frame)
            .expect("edge-world GPU frame");
        renderer_time += renderer_started.elapsed();
        let stats = renderer
            .last_frame_stats(window_id)
            .expect("edge-world renderer stats");
        packet_builds += stats.retained_packet_build_count;
        path_time_us += stats.retained_packet_path_command_time_us;
    }
    (
        runtime_time.as_secs_f64() * 1000.0 / FRAMES as f64,
        renderer_time.as_secs_f64() * 1000.0 / FRAMES as f64,
        packet_builds as f64 / FRAMES as f64,
        path_time_us as f64 / FRAMES as f64,
    )
}

#[test]
#[ignore = "diagnostic benchmark for retained flow-space edge layers"]
fn retained_edge_world_gpu_zoom_benchmark() {
    let direct = run_edge_world_benchmark(false);
    let retained = run_edge_world_benchmark(true);
    println!(
        "EDGE_WORLD_BENCHMARK edges=1024 frames=60 direct_runtime_ms={:.3} direct_renderer_ms={:.3} direct_packet_build_avg={:.2} direct_path_us={:.1} retained_runtime_ms={:.3} retained_renderer_ms={:.3} retained_packet_build_avg={:.2} retained_path_us={:.1} total_ratio={:.3}",
        direct.0,
        direct.1,
        direct.2,
        direct.3,
        retained.0,
        retained.1,
        retained.2,
        retained.3,
        (retained.0 + retained.1) / (direct.0 + direct.1).max(0.001),
    );
}
