#![forbid(unsafe_code)]

use sui_core::{Event, Result, WindowId};
use sui_render_wgpu::WgpuRenderer;
use sui_runtime::Runtime;

#[derive(Debug, Clone)]
pub struct PlatformWindow {
    pub id: WindowId,
    pub title: String,
}

#[derive(Debug, Default)]
pub struct DesktopPlatform {
    renderer: WgpuRenderer,
}

impl DesktopPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(&mut self, runtime: &mut Runtime) -> Result<Vec<PlatformWindow>> {
        let mut windows = Vec::new();

        for window_id in runtime.window_ids() {
            let output = runtime.render(window_id)?;
            self.renderer.render(&output.frame);
            windows.push(PlatformWindow {
                id: window_id,
                title: output.title,
            });
        }

        Ok(windows)
    }

    pub fn dispatch_event(
        &mut self,
        runtime: &mut Runtime,
        window_id: WindowId,
        event: Event,
    ) -> Result<()> {
        let _ = &mut self.renderer;
        runtime.handle_event(window_id, event)
    }

    pub fn renderer(&self) -> &WgpuRenderer {
        &self.renderer
    }
}