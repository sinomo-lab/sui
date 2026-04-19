use std::{cell::RefCell, rc::Rc};

use sui_core::{Result, SemanticsRole, WindowId};
use sui_render_wgpu::{DebugCaptureArtifact, DebugCaptureRequest};
use sui_runtime::WindowPerformanceSnapshot;

use crate::{
    harness::Harness,
    locator::Locator,
    screenshot::{ArtifactBundle, Screenshot},
    selector::Selector,
    snapshot::WindowSnapshot,
};

#[derive(Clone)]
pub struct TestWindow {
    harness: Rc<RefCell<Harness>>,
    window_id: WindowId,
}

impl TestWindow {
    pub(crate) fn new(harness: Rc<RefCell<Harness>>, window_id: WindowId) -> Self {
        Self { harness, window_id }
    }

    pub fn id(&self) -> WindowId {
        self.window_id
    }

    pub fn snapshot(&self) -> Result<WindowSnapshot> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        harness.snapshot(self.window_id)
    }

    pub fn run_until_idle(&self) -> Result<()> {
        self.harness.borrow_mut().run_until_idle()
    }

    pub fn advance_time(&self, delta: f64) -> Result<()> {
        self.harness.borrow_mut().advance_time(delta)
    }

    pub fn capture_screenshot(&self) -> Result<Screenshot> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        harness.capture_screenshot(self.window_id)
    }

    pub fn capture_artifacts(&self) -> Result<ArtifactBundle> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        harness.capture_artifacts(self.window_id)
    }

    pub fn capture_debug_frame(
        &self,
        request: DebugCaptureRequest,
    ) -> Result<DebugCaptureArtifact> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        harness.capture_debug_frame(self.window_id, request)
    }

    pub fn performance_snapshot(&self) -> Result<WindowPerformanceSnapshot> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        harness.performance_snapshot(self.window_id)
    }

    pub fn locator(&self, selector: Selector) -> Locator {
        Locator::new(Rc::clone(&self.harness), self.window_id, selector)
    }

    pub fn root(&self) -> Locator {
        self.locator(Selector::root())
    }

    pub fn focused(&self) -> Locator {
        self.locator(Selector::focused())
    }

    pub fn get_by_role(&self, role: SemanticsRole) -> Locator {
        self.locator(Selector::by_role(role))
    }

    pub fn get_by_text(&self, text: impl Into<String>) -> Locator {
        self.locator(Selector::by_text(text))
    }

    pub fn get_by_description(&self, text: impl Into<String>) -> Locator {
        self.locator(Selector::by_description(text))
    }
}
