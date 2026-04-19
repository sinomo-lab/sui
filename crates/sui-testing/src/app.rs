use std::{cell::RefCell, rc::Rc};

use sui_core::{Error, Result};
use sui_runtime::Runtime;

use crate::{harness::Harness, window::TestWindow};

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
        let harness = Rc::new(RefCell::new(Harness::new_live(move || {
            build().into_test_runtime()
        })?));
        Ok(Self { harness })
    }

    pub fn new_with_vsync<F, A>(build: F, vsync_enabled: bool) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        let harness = Rc::new(RefCell::new(Harness::new_live_with_vsync(move || {
            build().into_test_runtime()
        }, vsync_enabled)?));
        Ok(Self { harness })
    }

    pub fn new_with_options<F, A>(build: F, vsync_enabled: bool, visible: bool) -> Result<Self>
    where
        F: FnOnce() -> A + Send + 'static,
        A: IntoTestRuntime,
    {
        let harness = Rc::new(RefCell::new(Harness::new_live_with_options(move || {
            build().into_test_runtime()
        }, vsync_enabled, visible)?));
        Ok(Self { harness })
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

    pub fn set_default_timeout(&self, timeout: f64) -> Result<()> {
        if timeout.is_sign_negative() {
            return Err(Error::new("default timeout must be >= 0"));
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

    pub fn advance_time(&self, delta: f64) -> Result<()> {
        self.harness.borrow_mut().advance_time(delta)
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
