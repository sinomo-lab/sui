use crate::{
    config::Config,
    fixtures::{Fixture, MARKER},
    results::{Sample, Trial, rss_kib, work_json},
};
use serde_json::json;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};
use sui_core::{SemanticsRole, SemanticsValue, Vector};
use sui_platform::{
    DesktopAutomationAction, DesktopAutomationConfig, DesktopExtension, DesktopExtensionContext,
    DesktopFramePresented, DesktopPlatform,
};
use sui_runtime::{
    Runtime, SceneStatisticsDetailMode, set_window_scene_statistics_detail_mode,
    take_layout_work_collection,
};

#[derive(Debug)]
struct Observation {
    first: Option<(f64, f64)>,
    settled: Option<f64>,
    frames: usize,
    mounted: usize,
    dpr: Option<f64>,
    work: serde_json::Value,
}

#[derive(Debug)]
struct Observer {
    started: Instant,
    data: Rc<RefCell<Observation>>,
}
impl DesktopExtension for Observer {
    fn frame_presented(
        &mut self,
        context: DesktopExtensionContext<'_>,
        frame: DesktopFramePresented,
    ) -> sui_core::Result<()> {
        let semantics = context.runtime().semantics(frame.window_id)?;
        if !semantics.iter().any(|n| {
            n.name.as_deref() == Some(MARKER) && n.value == Some(SemanticsValue::Text("0".into()))
        }) {
            return Ok(());
        }
        let mut data = self.data.borrow_mut();
        if data.settled.is_some() {
            return Ok(());
        }
        let elapsed = frame
            .presented_at
            .duration_since(self.started)
            .as_secs_f64()
            * 1e6;
        data.first
            .get_or_insert((elapsed, frame.runtime_duration.as_secs_f64() * 1e6));
        data.frames += 1;
        data.mounted = context.runtime().widget_graph(frame.window_id)?.nodes.len();
        data.dpr = context
            .window(frame.window_id)
            .map(|w| w.host_window().scale_factor());
        if !context.runtime().needs_render(frame.window_id)? {
            data.settled.get_or_insert(elapsed);
            data.work = work_json(take_layout_work_collection());
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    c: &Config,
    trial: usize,
    runtime: Runtime,
    _fixture: Fixture,
    started: Instant,
    input_us: f64,
    construct_us: f64,
    attach_us: f64,
) -> Result<Trial, String> {
    if cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
        && std::env::var_os("WAYLAND_SOCKET").is_none()
    {
        return Err("unsupported: desktop mode requires a real display server".into());
    }
    let window = runtime.window_ids()[0];
    set_window_scene_statistics_detail_mode(
        window,
        if c.diagnostics {
            SceneStatisticsDetailMode::Detailed
        } else {
            SceneStatisticsDetailMode::Lightweight
        },
    );
    let data = Rc::new(RefCell::new(Observation {
        first: None,
        settled: None,
        frames: 0,
        mounted: 0,
        dpr: None,
        work: serde_json::Value::Null,
    }));
    let registry = sui_render_wgpu::WgpuExternalTextureRegistry::default();
    let platform = DesktopPlatform::new()
        .with_vsync_enabled(c.vsync)
        .with_external_texture_registry(registry.clone())
        .with_extension(Observer {
            started,
            data: data.clone(),
        })
        .with_automation(DesktopAutomationConfig {
            label: "widget-bench-startup".into(),
            target_role: SemanticsRole::GenericContainer,
            target_name: MARKER.into(),
            action: DesktopAutomationAction::ScrollPixels {
                delta: Vector::ZERO,
            },
            step_interval: Duration::from_millis(10),
            duration: Duration::from_millis(30),
            report_interval: Duration::from_secs(10),
            startup_timeout: Duration::from_secs(c.timeout_secs),
        });
    platform.run(runtime).map_err(|e| e.to_string())?;
    let data = data.borrow();
    let (first, runtime_us) = data
        .first
        .ok_or("native runner never presented the expected content")?;
    let settled = data
        .settled
        .ok_or("native initial content did not settle")?;
    if data.dpr.is_none_or(|actual| (actual - c.dpr).abs() > 0.01) {
        return Err(format!(
            "unsupported: requested DPR {}, actual {:?}",
            c.dpr, data.dpr
        ));
    }
    let context = registry
        .context()
        .ok_or("native renderer has no adapter information")?;
    let info = context.adapter_info();
    if info.device_type == wgpu::DeviceType::Cpu && !c.allow_software {
        return Err("unsupported: native runner selected a software adapter".into());
    }
    let sample = Sample {
        fixture: c.fixture.clone(),
        phase: "startup".into(),
        trial,
        elapsed_us: first,
        frames: data.frames,
        mounted_widgets: data.mounted,
        timings: BTreeMap::from([
            ("input_us".into(), input_us),
            ("widget_construct_us".into(), construct_us),
            ("runtime_attach_us".into(), attach_us),
            ("first_runtime_frame_us".into(), runtime_us),
            ("first_content_present_us".into(), first),
            ("first_settled_content_us".into(), settled),
        ]),
        work: data.work.clone(),
        ..Default::default()
    };
    Ok(Trial {
        status: "ok".into(),
        error: None,
        backend: json!({"mode":"desktop","name":info.name,"backend":format!("{:?}",info.backend),"driver":info.driver,"device_type":format!("{:?}",info.device_type),"actual_dpr":data.dpr,"vsync":c.vsync,"scanout_time":null}),
        samples: vec![sample],
        teardown_us: None,
        final_rss_kib: rss_kib(),
        teardown_work: serde_json::Value::Null,
        warmup_work: serde_json::Value::Null,
    })
}
