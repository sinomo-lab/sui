use std::{cell::RefCell, rc::Rc};

use sui_core::{Error, MotionPreference, Result};
use sui_render_wgpu::{DisplayCapabilities, WgpuExternalTextureRegistry};
use sui_runtime::{
    Runtime, set_app_motion_preference, set_motion_time_scale, set_window_render_options,
};

/// How far [`TestApp::settle_animations`] advances time per frame.
const SETTLE_STEP: f64 = 1.0 / 60.0;

use crate::{harness::Harness, window::TestWindow};

/// Options for a [`TestApp`], from [`TestApp::builder`].
///
/// Apps run headless unless asked to run live: their windows render
/// offscreen, sharing one GPU device, so tests run in parallel. A live app
/// opens real windows on the platform's event loop, one app at a time; ask
/// for one with [`Self::live`] or [`Self::visible`], or run every test live
/// by setting `SUI_TEST_BACKEND=live`.
pub struct TestAppBuilder<F> {
    build: F,
    vsync_enabled: bool,
    visible: bool,
    live: bool,
    display_capabilities: Option<DisplayCapabilities>,
}

impl<F, A> TestAppBuilder<F>
where
    F: FnOnce() -> A + Send + 'static,
    A: IntoTestRuntime,
{
    /// Wait for vertical sync when the app runs live and its windows are
    /// shown. Hidden windows render offscreen and never present.
    pub fn vsync(mut self, enabled: bool) -> Self {
        self.vsync_enabled = enabled;
        self
    }

    /// Show the app's windows on screen, which runs it live.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Run the app in real windows on the platform's event loop, as frame
    /// pacing and native input tests need, rather than headless. Live apps
    /// run one at a time. Where there is no display, the app runs headless.
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    /// Run the app as if its windows were on a display with `capabilities`,
    /// whatever this machine's screens are: output strategies, diagnostics,
    /// captures, and HDR theme styling follow them. The app runs headless,
    /// so nothing but `capabilities` decides its output.
    pub fn display_capabilities(mut self, capabilities: DisplayCapabilities) -> Self {
        self.display_capabilities = Some(capabilities);
        self
    }

    pub fn launch(self) -> Result<TestApp> {
        let harness = match self.display_capabilities {
            Some(capabilities) => Harness::new_headless_with_display(
                (self.build)().into_test_runtime()?,
                capabilities,
                30.0,
            )?,
            None if (self.live || self.visible || live_by_default()) && display_available() => {
                let build = self.build;
                Harness::new_live_with_options(
                    move || build().into_test_runtime(),
                    self.vsync_enabled,
                    self.visible,
                )?
            }
            None => Harness::new_headless_with_timeout((self.build)().into_test_runtime()?, 30.0)?,
        };
        Ok(TestApp {
            harness: Rc::new(RefCell::new(harness)),
        })
    }
}

pub trait IntoTestRuntime {
    fn into_test_runtime(self) -> Result<Runtime>;
}

impl IntoTestRuntime for Runtime {
    fn into_test_runtime(self) -> Result<Runtime> {
        Ok(self)
    }
}

impl IntoTestRuntime for sui_runtime::Application {
    fn into_test_runtime(self) -> Result<Runtime> {
        self.build()
    }
}

impl IntoTestRuntime for sui::Application {
    fn into_test_runtime(self) -> Result<Runtime> {
        let initial_window_render_options = self.initial_window_render_options();
        let runtime = self.build()?;
        if let Some(options) = initial_window_render_options {
            for window_id in runtime.window_ids() {
                set_window_render_options(window_id, options);
            }
        }
        Ok(runtime)
    }
}

impl IntoTestRuntime for Result<Runtime> {
    fn into_test_runtime(self) -> Result<Runtime> {
        self
    }
}

#[derive(Clone)]
pub struct TestApp {
    pub(crate) harness: Rc<RefCell<Harness>>,
}

impl TestApp {
    pub fn new<F, A>(build: F) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        Self::new_with_options(build, true, false)
    }

    pub fn new_with_vsync<F, A>(build: F, vsync_enabled: bool) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        Self::new_with_options(build, vsync_enabled, false)
    }

    pub fn new_with_options<F, A>(build: F, vsync_enabled: bool, visible: bool) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        Self::builder(build)
            .vsync(vsync_enabled)
            .visible(visible)
            .launch()
    }

    /// Start configuring a test app built by `build`.
    pub fn builder<F, A>(build: F) -> TestAppBuilder<F>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        TestAppBuilder {
            build,
            vsync_enabled: true,
            visible: false,
            live: false,
            display_capabilities: None,
        }
    }

    pub fn new_no_vsync<F, A>(build: F) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        Self::new_with_vsync(build, false)
    }

    pub fn new_visible_no_vsync<F, A>(build: F) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        Self::new_with_options(build, false, true)
    }

    pub fn from_runtime(runtime: Runtime) -> Result<Self> {
        let harness = Rc::new(RefCell::new(Harness::new_headless(runtime)?));
        Ok(Self { harness })
    }

    /// Build a headless test harness that can resolve app-owned WGPU textures.
    pub fn from_runtime_with_external_texture_registry(
        runtime: Runtime,
        registry: WgpuExternalTextureRegistry,
    ) -> Result<Self> {
        let harness = Rc::new(RefCell::new(
            Harness::new_headless_with_external_texture_registry(runtime, registry)?,
        ));
        Ok(Self { harness })
    }

    pub fn from_runtime_with_frame_budget(runtime: Runtime, initial_frames: usize) -> Result<Self> {
        let harness = Rc::new(RefCell::new(Harness::new_headless_with_frame_budget(
            runtime,
            initial_frames,
        )?));
        Ok(Self { harness })
    }

    /// How many seconds waits such as [`Self::run_until_idle`] allow before
    /// failing. Infinity waits without a deadline.
    pub fn set_default_timeout(&self, timeout: f64) -> Result<()> {
        if timeout.is_nan() || timeout < 0.0 {
            return Err(Error::new(format!(
                "default timeout must be a number of seconds >= 0, not {timeout}"
            )));
        }

        self.harness.borrow_mut().set_default_timeout(timeout);
        Ok(())
    }

    pub fn default_timeout(&self) -> f64 {
        self.harness.borrow().default_timeout()
    }

    pub fn run_until_idle(&self) -> Result<()> {
        self.harness.borrow_mut().run_until_idle()
    }

    pub fn pump_frames(&self, frames: usize) -> Result<()> {
        self.harness.borrow_mut().pump_frames(frames)
    }

    pub fn advance_time(&self, delta: f64) -> Result<()> {
        self.harness.borrow_mut().advance_time(delta)
    }

    /// Override the motion preference of the app under test, or pass `None`
    /// to follow the system preference, which tests treat as full motion.
    pub fn set_motion_preference(&self, preference: Option<MotionPreference>) -> Result<()> {
        self.harness
            .borrow_mut()
            .with_runtime(move |_| set_app_motion_preference(preference))
    }

    /// Play the app's transitions at `time_scale` speed.
    pub fn set_motion_time_scale(&self, time_scale: f32) -> Result<()> {
        self.harness
            .borrow_mut()
            .with_runtime(move |_| set_motion_time_scale(time_scale))
    }

    /// Whether any window still has a transition running.
    pub fn has_running_animations(&self) -> Result<bool> {
        self.harness.borrow_mut().with_runtime(|runtime| {
            runtime.window_ids().into_iter().any(|window_id| {
                runtime
                    .has_pending_animation_frames(window_id)
                    .unwrap_or(false)
            })
        })
    }

    /// Advance time frame by frame until every transition has finished, and
    /// return how many seconds that took. Fails after ten seconds, which
    /// usually means an animation repeats forever.
    pub fn settle_animations(&self) -> Result<f64> {
        self.settle_animations_within(10.0)
    }

    /// Like [`TestApp::settle_animations`], failing after `limit` seconds.
    pub fn settle_animations_within(&self, limit: f64) -> Result<f64> {
        let mut elapsed = 0.0;
        while self.has_running_animations()? {
            if elapsed >= limit {
                return Err(Error::new(format!(
                    "animations were still running after {limit} s"
                )));
            }
            self.advance_time(SETTLE_STEP)?;
            elapsed += SETTLE_STEP;
        }
        Ok(elapsed)
    }

    /// Sample `probe` now and then every `step` seconds for `duration`
    /// seconds, returning `(elapsed, value)` pairs, to assert on the shape
    /// of a transition rather than only its end state.
    pub fn record_motion<T>(
        &self,
        duration: f64,
        step: f64,
        mut probe: impl FnMut() -> Result<T>,
    ) -> Result<Vec<(f64, T)>> {
        if step <= 0.0 || !step.is_finite() {
            return Err(Error::new("motion sampling step must be > 0"));
        }
        if !(duration.is_finite() && duration >= 0.0) {
            return Err(Error::new(format!(
                "motion sampling duration must be a finite number of seconds >= 0, not {duration}"
            )));
        }
        let mut samples = vec![(0.0, probe()?)];
        let mut elapsed = 0.0;
        while elapsed + step <= duration + step * 1.0e-6 {
            self.advance_time(step)?;
            elapsed += step;
            samples.push((elapsed, probe()?));
        }
        Ok(samples)
    }

    pub fn windows(&self) -> Result<Vec<TestWindow>> {
        let window_ids = self.harness.borrow().window_ids();
        Ok(window_ids
            .into_iter()
            .map(|window_id| TestWindow::new(Rc::clone(&self.harness), window_id))
            .collect())
    }

    pub fn main_window(&self) -> Result<TestWindow> {
        let window_id = self
            .harness
            .borrow()
            .window_ids()
            .into_iter()
            .next()
            .ok_or_else(|| Error::new("test app did not create any windows"))?;
        Ok(TestWindow::new(Rc::clone(&self.harness), window_id))
    }

    pub fn window_by_title(&self, title: &str) -> Result<TestWindow> {
        let window_id = self
            .harness
            .borrow()
            .window_id_by_title(title)
            .ok_or_else(|| Error::new(format!("no test window found with title \"{title}\"")))?;
        Ok(TestWindow::new(Rc::clone(&self.harness), window_id))
    }
}

/// Whether `SUI_TEST_BACKEND=live` asks for every app to run live.
fn live_by_default() -> bool {
    std::env::var("SUI_TEST_BACKEND").as_deref() == Ok("live")
}

fn display_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var_os("WAYLAND_SOCKET").is_some()
            || std::env::var_os("DISPLAY").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}
