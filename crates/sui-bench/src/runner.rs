use crate::{
    config::Config,
    fixtures,
    results::{Sample, Trial, rss_kib, work_json},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use sui_core::{Event, SemanticsValue, Size, WindowEvent, WindowId};
use sui_runtime::{
    RenderOutput, Runtime, SceneStatisticsDetailMode, begin_layout_work_collection,
    set_window_scene_statistics_detail_mode, take_layout_work_collection,
};

pub fn micros(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e6
}

pub struct Backend {
    pub info: Value,
    stats: Vec<Value>,
    widget_details: Vec<Value>,
    diagnostics: bool,
    redraw: bool,
    #[cfg(feature = "gpu")]
    renderer: Option<sui_render_wgpu::WgpuRenderer>,
    #[cfg(feature = "gpu")]
    registry: sui_render_wgpu::WgpuExternalTextureRegistry,
    allow_software: bool,
}

impl Backend {
    fn new(c: &Config) -> Result<Self, String> {
        if c.mode == "offscreen" && !cfg!(feature = "gpu") {
            return Err("unsupported: rebuild with --features gpu".into());
        }
        #[cfg(feature = "gpu")]
        let registry = sui_render_wgpu::WgpuExternalTextureRegistry::default();
        #[cfg(feature = "gpu")]
        let renderer = if c.mode == "offscreen" {
            let mut renderer = sui_render_wgpu::WgpuRenderer::new();
            renderer.set_external_texture_registry(registry.clone());
            renderer.set_runtime_diagnostics_enabled(c.diagnostics);
            Some(renderer)
        } else {
            None
        };
        Ok(Self {
            info: json!({"mode":c.mode,"adapter":null,"gpu_execution_time":null,"gpu_execution_time_reason":"CPU submission timing only"}),
            stats: Vec::new(),
            widget_details: Vec::new(),
            diagnostics: c.diagnostics,
            redraw: c.redraw == "requested",
            #[cfg(feature = "gpu")]
            renderer,
            #[cfg(feature = "gpu")]
            registry,
            allow_software: c.allow_software,
        })
    }
    fn render(&mut self, output: &RenderOutput) -> Result<(), String> {
        #[cfg(feature = "gpu")]
        if let Some(renderer) = &mut self.renderer {
            renderer.render(&output.frame).map_err(|e| e.to_string())?;
            if self.diagnostics
                && let Some(s) = renderer.last_frame_stats(output.frame.window_id)
            {
                self.stats.push(json!({"pass_count":s.pass_count,"draw_count":s.draw_count,"uploaded_vertex_bytes":s.uploaded_vertex_bytes,
                        "text_glyph_instances":s.text_glyph_instance_count,"atlas_misses":s.text_atlas_miss_count,"atlas_upload_bytes":s.text_atlas_upload_bytes,
                        "retained_packet_builds":s.retained_packet_build_count,"retained_packet_build_us":s.retained_packet_build_time_us,
                        "surface_acquire_us":s.surface_acquire_time_us,"surface_present_us":s.surface_present_time_us,
                        "retained_rebuild_reasons":format!("{:?}",s.retained_packet_rebuilds),
                        "device_prepare_us":s.device_prepare_time_us,"target_prepare_us":s.target_prepare_time_us,
                        "text_engine_init_us":s.text_engine_init_time_us,"pipeline_create_us":s.pipeline_create_time_us,
                        "pipeline_create_count":s.pipeline_create_count,"scene_traversal_us":s.retained_scene_traversal_time_us,
                        "composition_us":s.composition_time_us,"resource_collection_us":s.resource_collection_time_us,
                        "bind_group_prepare_us":s.bind_group_prepare_time_us,"batch_prepare_us":s.batch_prepare_time_us,
                        "gpu_upload_us":s.gpu_upload_time_us,"pass_encode_us":s.pass_encode_time_us,
                        "queue_submit_us":s.queue_submit_time_us,"queue_submit_count":s.queue_submit_count,
                        "atlas_allocate_us":s.text_atlas_allocate_time_us,"atlas_clear_us":s.text_atlas_clear_time_us,
                        "atlas_copy_us":s.text_atlas_copy_time_us,"atlas_bind_group_create_us":s.text_atlas_create_bind_group_time_us}));
            }
            if let Some(context) = self.registry.context() {
                let info = context.adapter_info();
                self.info = json!({"mode":"offscreen","name":info.name,"driver":info.driver,"driver_info":info.driver_info,
                    "device_type":format!("{:?}",info.device_type),"backend":format!("{:?}",info.backend),
                    "gpu_execution_time":null,"gpu_execution_time_reason":"CPU submission timing only"});
                if info.device_type == wgpu::DeviceType::Cpu && !self.allow_software {
                    return Err("unsupported: software GPU adapter selected; use --allow-software explicitly".into());
                }
            } else {
                return Err("GPU render did not expose an initialized adapter".into());
            }
        }
        #[cfg(not(feature = "gpu"))]
        let _ = (output, self.allow_software, self.diagnostics);
        Ok(())
    }
    fn take_stats(&mut self) -> Value {
        if self.diagnostics && self.info["mode"] == "offscreen" {
            json!(std::mem::take(&mut self.stats))
        } else {
            Value::Null
        }
    }
    fn take_widget_details(&mut self) -> Value {
        if self.diagnostics {
            json!(std::mem::take(&mut self.widget_details))
        } else {
            Value::Null
        }
    }
    fn validate_pixels(&self, window: WindowId) -> Result<(), String> {
        #[cfg(feature = "gpu")]
        if let Some(renderer) = &self.renderer {
            let capture = renderer.capture_rgba(window).map_err(|e| e.to_string())?;
            if capture.width() == 0 || capture.height() == 0 {
                return Err("offscreen capture is empty".into());
            }
            let pixels = capture.pixels();
            if !pixels.chunks_exact(4).any(|pixel| pixel != &pixels[..4]) {
                return Err("offscreen capture contains only a flat color".into());
            }
        }
        #[cfg(not(feature = "gpu"))]
        let _ = window;
        Ok(())
    }
}

fn begin(c: &Config) -> Result<(), String> {
    if c.diagnostics && !begin_layout_work_collection() {
        return Err("unsupported: rebuild with --features diagnostics".into());
    }
    Ok(())
}

fn preparation(runtime: &Runtime) -> Value {
    let s = runtime.text_preparation_cache_snapshot();
    json!({"paragraph_entries":s.paragraphs.entries,"paragraph_bytes":s.paragraphs.retained_bytes,
        "paragraph_hits":s.paragraphs.hits,"paragraph_misses":s.paragraphs.misses,"paragraph_evictions":s.paragraphs.evictions,
        "glyph_entries":s.glyphs.entries,"glyph_bytes":s.glyphs.retained_bytes,"glyph_hits":s.glyphs.hits,"glyph_misses":s.glyphs.misses,"glyph_evictions":s.glyphs.evictions})
}

fn validate(
    runtime: &Runtime,
    window: WindowId,
    fixture: &fixtures::Fixture,
    output: &RenderOutput,
    c: &Config,
) -> Result<usize, String> {
    let expected = fixture.revision.get().to_string();
    if !output.semantics.iter().any(|n| {
        n.name.as_deref() == Some(fixtures::MARKER)
            && n.value == Some(SemanticsValue::Text(expected.clone()))
    }) {
        return Err(format!("content revision {expected} was not observed"));
    }
    if output.semantics.len() < 2 || output.frame.scene.commands().is_empty() {
        return Err("fixture produced empty content".into());
    }
    if let Some(expected) = &fixture.expected_label
        && !output
            .semantics
            .iter()
            .any(|node| node.name.as_ref() == Some(expected))
    {
        return Err(format!("updated label missing: {expected}"));
    }
    let graph = runtime.widget_graph(window).map_err(|e| e.to_string())?;
    if graph.nodes.len() < 2
        || graph.nodes.iter().any(|n| {
            !n.bounds.x().is_finite()
                || !n.bounds.y().is_finite()
                || !n.bounds.width().is_finite()
                || !n.bounds.height().is_finite()
        })
    {
        return Err("invalid widget graph geometry".into());
    }
    if c.fixture == "virtual-collection" && c.size >= 1000 && graph.nodes.len() >= c.size / 2 {
        return Err("virtual fixture mounted too much of its logical collection".into());
    }
    Ok(graph.nodes.len())
}

fn settle(
    runtime: &mut Runtime,
    window: WindowId,
    backend: &mut Backend,
) -> Result<(RenderOutput, usize, BTreeMap<String, f64>), String> {
    let mut phases = BTreeMap::new();
    for frame in 1..=16 {
        #[cfg(feature = "diagnostics")]
        if backend.diagnostics {
            sui_runtime::begin_frame_timing_collection();
        }
        if backend.redraw {
            let started = Instant::now();
            runtime
                .handle_event(window, Event::Window(WindowEvent::RedrawRequested))
                .map_err(|error| error.to_string())?;
            *phases.entry("redraw_dispatch_us".into()).or_default() += micros(started);
        }
        let start = Instant::now();
        let output = runtime.render(window).map_err(|e| e.to_string())?;
        let runtime_us = micros(start);
        *phases.entry("runtime_us".into()).or_default() += runtime_us;
        if frame == 1 {
            phases.insert("first_runtime_frame_us".into(), runtime_us);
        }
        if backend.diagnostics {
            let hotspots:Vec<_>=output.diagnostics.widget_timings.iter().take(24).map(|s|json!({"widget_id":s.widget_id.get(),"widget":s.widget_name,"phase":s.phase.label(),"inclusive_us":s.duration_ms*1000.0,"calls":s.calls})).collect();
            let invalidations: Vec<_> = output
                .diagnostics
                .invalidations
                .iter()
                .take(24)
                .map(|s| json!({"kind":format!("{:?}",s.kind),"source":s.source,"reason":s.reason}))
                .collect();
            backend.widget_details.push(json!({"frame":frame,"hotspots":hotspots,"hotspots_total":output.diagnostics.widget_timings.len(),"invalidations":invalidations,"invalidations_total":output.diagnostics.invalidations.len(),"widget_rebuilds":output.diagnostics.widget_rebuilds.len(),"root_commands":output.frame.scene.commands().len(),
                "text_timing":{"requests":output.diagnostics.runtime_text_timing.request_count,"size_only_requests":output.diagnostics.runtime_text_timing.size_only_request_count,"total_us":output.diagnostics.runtime_text_timing.total_time_us,"size_only_us":output.diagnostics.runtime_text_timing.size_only_time_us,"miss_layout_us":output.diagnostics.runtime_text_timing.miss_layout_time_us},
                "full_text_layout_cache":{"entries":output.diagnostics.text_caches.runtime_layout.entries,"hits":output.diagnostics.text_caches.runtime_layout.hits,"misses":output.diagnostics.text_caches.runtime_layout.misses}}));
        }
        for phase in &output.diagnostics.phase_timings {
            *phases
                .entry(format!("runtime_{}", phase.phase.label()))
                .or_default() += phase.duration_ms * 1000.0;
        }
        let start = Instant::now();
        backend.render(&output)?;
        if backend.info["mode"] == "offscreen" {
            *phases.entry("renderer_submit_us".into()).or_default() += micros(start);
        }
        if !runtime.needs_render(window).map_err(|e| e.to_string())?
            && !runtime.has_pending_commands()
        {
            return Ok((output, frame, phases));
        }
        runtime.process_commands();
    }
    Err("layout did not settle within 16 frames".into())
}

#[cfg(test)]
pub fn run(c: &Config, trial: usize) -> Trial {
    run_at(c, trial, Instant::now())
}

pub fn run_at(c: &Config, trial: usize, entered: Instant) -> Trial {
    let mut partial = Vec::new();
    match run_inner(c, trial, &mut partial, entered) {
        Ok(result) => result,
        Err(error) => {
            let mut result = Trial::failure(
                if error.starts_with("unsupported:") {
                    "unsupported"
                } else {
                    "error"
                },
                error,
            );
            result.samples = partial;
            result
        }
    }
}

fn run_inner(
    c: &Config,
    trial: usize,
    samples: &mut Vec<Sample>,
    started: Instant,
) -> Result<Trial, String> {
    c.validate()?;
    begin(c)?;
    let input_started = Instant::now();
    let input = fixtures::input(c);
    let input_us = micros(input_started);
    let construction_started = Instant::now();
    let (app, mut fixture) = fixtures::build(c, input)?;
    let construct_us = micros(construction_started);
    let attach_started = Instant::now();
    let mut runtime = app.build().map_err(|e| e.to_string())?;
    let attach_us = micros(attach_started);
    let window = runtime.window_ids()[0];
    if c.mode == "desktop" {
        #[cfg(feature = "desktop")]
        return crate::desktop::run(
            c,
            trial,
            runtime,
            fixture,
            started,
            input_us,
            construct_us,
            attach_us,
        );
        #[cfg(not(feature = "desktop"))]
        return Err("unsupported: rebuild with --features desktop".into());
    }
    if c.mode == "construct" {
        let elapsed = micros(started);
        let work = work_json(take_layout_work_collection());
        begin(c)?;
        let drop_started = Instant::now();
        drop((runtime, fixture));
        return Ok(Trial {
            status: "ok".into(),
            error: None,
            backend: json!({"mode":"construct"}),
            samples: vec![Sample {
                fixture: c.fixture.clone(),
                phase: "construct".into(),
                trial,
                elapsed_us: elapsed,
                timings: BTreeMap::from([
                    ("input_us".into(), input_us),
                    ("widget_construct_us".into(), construct_us),
                    ("runtime_attach_us".into(), attach_us),
                ]),
                work,
                ..Default::default()
            }],
            teardown_us: Some(micros(drop_started)),
            final_rss_kib: rss_kib(),
            teardown_work: work_json(take_layout_work_collection()),
            warmup_work: Value::Null,
        });
    }
    let event_started = Instant::now();
    runtime
        .handle_event(
            window,
            Event::Window(WindowEvent::ScaleFactorChanged {
                scale_factor: c.dpr,
                raw_dpi: None,
                suggested_size: Some(Size::new(c.width, c.height)),
            }),
        )
        .map_err(|e| e.to_string())?;
    let event_us = micros(event_started);
    set_window_scene_statistics_detail_mode(
        window,
        if c.diagnostics {
            SceneStatisticsDetailMode::Detailed
        } else {
            SceneStatisticsDetailMode::Lightweight
        },
    );
    let backend_started = Instant::now();
    let mut backend = Backend::new(c)?;
    let backend_us = micros(backend_started);
    let first_started = Instant::now();
    let (initial, frames, mut timings) = settle(&mut runtime, window, &mut backend)?;
    let first_us = micros(first_started);
    let elapsed = micros(started);
    let work = work_json(take_layout_work_collection());
    let mounted = validate(&runtime, window, &fixture, &initial, c)?;
    timings.extend([
        ("input_us".into(), input_us),
        ("widget_construct_us".into(), construct_us),
        ("runtime_attach_us".into(), attach_us),
        ("initial_event_us".into(), event_us),
        ("backend_construct_us".into(), backend_us),
        ("first_settled_frame_us".into(), first_us),
    ]);
    samples.push(Sample {
        fixture: c.fixture.clone(),
        phase: "startup".into(),
        trial,
        elapsed_us: elapsed,
        frames,
        mounted_widgets: mounted,
        timings,
        work,
        text_cache: preparation(&runtime),
        renderer: backend.take_stats(),
        widget_details: backend.take_widget_details(),
        ..Default::default()
    });
    drop(initial);
    let mut warmup_work = Value::Null;
    if c.preset != "startup" {
        begin(c)?;
        for step in 0..c.warmup {
            fixture.mutate(c, step, &mut runtime, window)?;
            runtime.tick(step as f64 / 60.0);
            for (target, event) in runtime.drain_ready_events() {
                runtime
                    .handle_event(target, event)
                    .map_err(|e| e.to_string())?;
            }
            let (output, _, _) = settle(&mut runtime, window, &mut backend)?;
            validate(&runtime, window, &fixture, &output, c)?;
        }
        warmup_work = work_json(take_layout_work_collection());
        backend.take_stats();
        backend.take_widget_details();
        let trace_started = Instant::now();
        let mut operation = 0;
        while operation < c.steps {
            let due = if c.rate_hz > 0.0 {
                Duration::from_secs_f64(operation as f64 / c.rate_hz)
            } else {
                trace_started.elapsed()
            };
            if let Some(wait) = due.checked_sub(trace_started.elapsed()) {
                std::thread::sleep(wait);
            }
            let operation_started = Instant::now();
            begin(c)?;
            let mut offered = 1;
            if c.rate_hz > 0.0 {
                let ready = (trace_started.elapsed().as_secs_f64() * c.rate_hz).floor() as usize;
                offered = (ready.saturating_sub(operation) + 1).min(c.steps - operation);
            }
            let mut changed = 0;
            for update in operation..operation + offered {
                changed += fixture.mutate(c, c.warmup + update, &mut runtime, window)?;
            }
            runtime.process_commands();
            runtime.tick((c.warmup + operation) as f64 / 60.0);
            for (target, event) in runtime.drain_ready_events() {
                runtime
                    .handle_event(target, event)
                    .map_err(|e| e.to_string())?;
            }
            let mutation_us = micros(operation_started);
            let (output, frames, timings) = if c.mutation == "idle" {
                if runtime.needs_render(window).map_err(|e| e.to_string())? {
                    return Err("idle fixture unexpectedly scheduled a frame".into());
                }
                (
                    runtime.render(window).map_err(|e| e.to_string())?,
                    0,
                    BTreeMap::new(),
                )
            } else {
                settle(&mut runtime, window, &mut backend)?
            };
            let service_us = micros(operation_started);
            let elapsed_us = if c.rate_hz > 0.0 {
                trace_started.elapsed().saturating_sub(due).as_secs_f64() * 1e6
            } else {
                service_us
            };
            let work = work_json(take_layout_work_collection());
            let mounted = validate(&runtime, window, &fixture, &output, c)?;
            samples.push(Sample {
                fixture: c.fixture.clone(),
                phase: "update".into(),
                trial,
                operation,
                elapsed_us,
                mutation_us,
                service_us,
                queue_age_us: (elapsed_us - service_us).max(0.0),
                changed_items: changed,
                offered_updates: offered,
                coalesced_updates: offered - 1,
                frames,
                mounted_widgets: mounted,
                timings,
                work,
                text_cache: preparation(&runtime),
                renderer: backend.take_stats(),
                widget_details: backend.take_widget_details(),
            });
            operation += offered;
        }
    }
    backend.validate_pixels(window)?;
    let info = backend.info.clone();
    begin(c)?;
    let drop_started = Instant::now();
    drop((runtime, fixture, backend));
    let teardown_us = micros(drop_started);
    Ok(Trial {
        status: "ok".into(),
        error: None,
        backend: info,
        samples: std::mem::take(samples),
        teardown_us: Some(teardown_us),
        final_rss_kib: rss_kib(),
        teardown_work: work_json(take_layout_work_collection()),
        warmup_work,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(feature = "diagnostics")]
    fn local_grid_updates_reuse_clean_paint_and_semantics_in_both_redraw_modes() {
        for redraw in ["natural", "requested"] {
            let config = Config {
                fixture: "controls-grid".into(),
                size: 128,
                steps: 3,
                warmup: 32,
                diagnostics: true,
                redraw: redraw.into(),
                ..Default::default()
            };
            let result = run(&config, 0);
            assert_eq!(result.status, "ok", "{:?}", result.error);
            for sample in result
                .samples
                .iter()
                .filter(|sample| sample.phase == "update")
            {
                assert!(sample.work["paint_cache_hits"].as_u64().unwrap() > 20);
                assert!(sample.work["semantics_cache_hits"].as_u64().unwrap() > 20);
                assert!(sample.work["paint_executions"].as_u64().unwrap() < 20);
            }
        }
    }

    #[test]
    fn every_fixture_reaches_requested_content_with_finite_geometry() {
        for fixture in crate::config::FIXTURES {
            let config = Config {
                fixture: fixture.to_string(),
                size: 16,
                steps: 4,
                warmup: 1,
                ..Default::default()
            };
            let result = run(&config, 0);
            assert_eq!(result.status, "ok", "{fixture}: {:?}", result.error);
            assert_eq!(result.samples.len(), 5);
            assert!(
                result
                    .samples
                    .iter()
                    .all(|sample| sample.elapsed_us.is_finite() && sample.mounted_widgets >= 2)
            );
        }
    }
    #[test]
    fn idle_control_does_not_claim_new_frames() {
        let config = Config {
            fixture: "controls-grid".into(),
            size: 8,
            steps: 4,
            warmup: 0,
            mutation: "idle".into(),
            ..Default::default()
        };
        let result = run(&config, 0);
        assert_eq!(result.status, "ok", "{:?}", result.error);
        assert!(
            result
                .samples
                .iter()
                .skip(1)
                .all(|sample| sample.frames == 0)
        );
    }
    #[test]
    #[cfg(feature = "diagnostics")]
    fn paint_updates_do_not_measure_and_counter_requests_balance() {
        let config = Config {
            fixture: "scene-properties".into(),
            size: 8,
            steps: 4,
            warmup: 0,
            diagnostics: true,
            ..Default::default()
        };
        let result = run(&config, 0);
        assert_eq!(result.status, "ok", "{:?}", result.error);
        assert!(
            result.samples[0].work["measure_executions"]
                .as_u64()
                .unwrap()
                > 0
        );
        for sample in result.samples.iter().skip(1) {
            assert_eq!(sample.work["measure_executions"], 0);
            assert_eq!(sample.work["arrange_executions"], 0);
            assert_eq!(
                sample.work["measure_requests"].as_u64().unwrap(),
                sample.work["measure_executions"].as_u64().unwrap()
                    + sample.work["measure_cache_hits"].as_u64().unwrap()
            );
        }
    }

    #[test]
    #[cfg(feature = "diagnostics")]
    fn full_rebuild_control_constructs_widgets_while_local_updates_retain_them() {
        for mutation in ["local", "rebuild"] {
            let config = Config {
                fixture: "controls-grid".into(),
                mutation: mutation.into(),
                size: 8,
                steps: 3,
                warmup: 0,
                diagnostics: true,
                ..Default::default()
            };
            let result = run(&config, 0);
            assert_eq!(result.status, "ok", "{:?}", result.error);
            for sample in result.samples.iter().skip(1) {
                let constructed = sample.work["constructed"].as_u64().unwrap();
                assert_eq!(constructed > 0, mutation == "rebuild");
            }
            assert!(result.teardown_work["dropped"].as_u64().unwrap() > 0);
        }
    }
}
