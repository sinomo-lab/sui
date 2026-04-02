#![forbid(unsafe_code)]

use sui_scene::SceneFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCapabilities {
    pub supports_color_management: bool,
    pub supports_offscreen_surfaces: bool,
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self {
            supports_color_management: true,
            supports_offscreen_surfaces: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererInterop {
    pub raw_wgpu_enabled: bool,
}

#[derive(Debug, Default)]
pub struct WgpuRenderer {
    frames_rendered: usize,
    capabilities: RendererCapabilities,
}

impl WgpuRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self, _frame: &SceneFrame) {
        self.frames_rendered += 1;
    }

    pub fn capabilities(&self) -> RendererCapabilities {
        self.capabilities
    }

    pub fn frames_rendered(&self) -> usize {
        self.frames_rendered
    }
}