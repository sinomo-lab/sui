use crate::application::BindingRenderOptions;
use crate::diagnostics::{
    BindingInspectorSnapshot, BindingRenderSnapshot, BindingSemanticNode, binding_semantics_busy,
    binding_semantics_checked, binding_semantics_descriptions, binding_semantics_disabled,
    binding_semantics_editable_multiline, binding_semantics_expanded, binding_semantics_focused,
    binding_semantics_hidden, binding_semantics_hovered, binding_semantics_names,
    binding_semantics_nodes, binding_semantics_roles, binding_semantics_selected,
    binding_semantics_values,
};
use crate::events::{
    BindingEvent, BindingImeEvent, BindingKeyState, BindingKeyboardEvent, BindingPointerButton,
    BindingPointerEvent, BindingPointerEventKind,
};
use crate::handles::BindingWindowId;
use crate::messages::BindingMessageBus;
use crate::tasks::{BindingUiHandle, UiTaskQueue};
use std::fmt;
use sui::Event;
use sui::Runtime;
use sui::SceneCommand;
use sui::WindowEvent;

pub struct BindingRuntime {
    pub(crate) runtime: Runtime,
    pub(crate) window_ids: Vec<BindingWindowId>,
    pub(crate) ui_tasks: UiTaskQueue,
    pub(crate) messages: BindingMessageBus,
    /// Frame time in seconds, advanced by [`Self::tick`] and
    /// [`Self::advance_time`].
    pub(crate) frame_time: f64,
    /// Offscreen renderer for screenshots, created on first use.
    #[cfg(feature = "desktop")]
    pub(crate) screenshot_renderer: Option<sui::WgpuRenderer>,
}

/// Simulated frame length for [`BindingRuntime::settle_animations`].
const SETTLE_STEP: f64 = 1.0 / 60.0;

impl BindingRuntime {
    pub fn ui_handle(&self) -> BindingUiHandle {
        self.ui_tasks
            .handle()
            .with_message_bus(self.messages.clone())
    }

    pub fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.ui_tasks.set_waker(wake);
    }

    pub fn clear_waker(&self) {
        self.ui_tasks.clear_waker();
    }

    pub fn pending_ui_task_count(&self) -> usize {
        self.ui_tasks.pending_count()
    }

    pub fn drain_ui_tasks(&mut self) -> Result<usize, String> {
        let pending = self.ui_tasks.pending_count();
        if pending > 0 {
            for window_id in self.window_ids.clone() {
                self.runtime
                    .wake_root(window_id.into_sui())
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(pending.saturating_sub(self.ui_tasks.pending_count()))
    }

    pub fn window_count(&self) -> usize {
        self.window_ids.len()
    }

    pub fn window_ids(&self) -> Vec<BindingWindowId> {
        self.window_ids.clone()
    }

    pub fn window_id_at(&self, index: usize) -> Result<BindingWindowId, String> {
        self.window_ids
            .get(index)
            .copied()
            .ok_or_else(|| format!("window index {index} is out of range"))
    }

    pub fn tick(&mut self, frame_time: f64) {
        self.frame_time = frame_time;
        self.runtime.tick(frame_time);
    }

    /// The frame time in seconds.
    pub fn frame_time(&self) -> f64 {
        self.frame_time
    }

    /// Move the frame clock forward by `delta` seconds and dispatch the timer
    /// and animation events that became due, as a platform host would.
    pub fn advance_time(&mut self, delta: f64) -> Result<(), String> {
        if !(delta.is_finite() && delta >= 0.0) {
            return Err(format!(
                "time delta must be a finite number of seconds >= 0, not {delta}"
            ));
        }
        self.tick(self.frame_time + delta);
        self.drain_ui_tasks()?;
        for window_id in self.window_ids.clone() {
            self.runtime
                .render(window_id.into_sui())
                .map_err(|error| error.to_string())?;
        }
        self.drain_ready_event_count()?;
        Ok(())
    }

    /// Whether any window still has an animation in progress.
    pub fn has_running_animations(&self) -> Result<bool, String> {
        for window_id in &self.window_ids {
            if self
                .runtime
                .has_pending_animation_frames(window_id.into_sui())
                .map_err(|error| error.to_string())?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Advance time frame by frame until every animation has finished, and
    /// return how many seconds that took. Fails after `limit` seconds, which
    /// usually means an animation repeats forever.
    pub fn settle_animations(&mut self, limit: f64) -> Result<f64, String> {
        let mut elapsed = 0.0;
        // Lay out first so animations started by recent changes are seen.
        self.advance_time(0.0)?;
        while self.has_running_animations()? {
            if elapsed >= limit {
                return Err(format!("animations were still running after {limit} s"));
            }
            self.advance_time(SETTLE_STEP)?;
            elapsed += SETTLE_STEP;
        }
        Ok(elapsed)
    }

    /// Render a window offscreen and return its pixels as PNG data, at the
    /// window's initial size or, without one, its content's size.
    #[cfg(feature = "desktop")]
    pub fn capture_png(&mut self, window_id: BindingWindowId) -> Result<Vec<u8>, String> {
        self.drain_ui_tasks()?;
        let output = self
            .runtime
            .render(window_id.into_sui())
            .map_err(|error| error.to_string())?;
        if output.frame.surface_size.width < 1.0 || output.frame.surface_size.height < 1.0 {
            return Err(
                "the window has no size to capture; give it one, such as with an initial size"
                    .to_owned(),
            );
        }
        let renderer = self
            .screenshot_renderer
            .get_or_insert_with(sui::WgpuRenderer::new);
        renderer
            .render(&output.frame)
            .map_err(|error| error.to_string())?;
        let image = renderer
            .capture_rgba(window_id.into_sui())
            .map_err(|error| error.to_string())?;
        encode_png(image.width(), image.height(), image.pixels())
    }

    #[cfg(not(feature = "desktop"))]
    pub fn capture_png(&mut self, _window_id: BindingWindowId) -> Result<Vec<u8>, String> {
        Err("screenshots require the `desktop` feature".to_owned())
    }

    /// Dispatch due timer, animation-frame, and wake events, as a platform
    /// host does each frame, and return how many were dispatched.
    pub fn drain_ready_event_count(&mut self) -> Result<usize, String> {
        let ready = self.runtime.drain_ready_events();
        let count = ready.len();
        for (window_id, event) in ready {
            self.runtime
                .handle_event(window_id, event)
                .map_err(|error| error.to_string())?;
        }
        Ok(count)
    }

    pub fn request_redraw_all(&mut self) -> Result<(), String> {
        for window_id in self.window_ids.clone() {
            self.request_redraw(window_id)?;
        }
        Ok(())
    }

    pub fn request_redraw(&mut self, window_id: BindingWindowId) -> Result<(), String> {
        self.runtime
            .handle_event(
                window_id.into_sui(),
                Event::Window(WindowEvent::RedrawRequested),
            )
            .map_err(|error| error.to_string())
    }

    pub fn set_render_options(
        &mut self,
        window_id: BindingWindowId,
        options: BindingRenderOptions,
    ) -> Result<(), String> {
        if !self.window_ids.contains(&window_id) {
            return Err(format!(
                "window {} does not belong to this application",
                window_id.get()
            ));
        }
        sui::set_window_render_options(window_id.into_sui(), options.into_sui());
        self.request_redraw(window_id)
    }

    pub fn set_inspector_tracing(
        &mut self,
        window_id: BindingWindowId,
        enabled: bool,
    ) -> Result<(), String> {
        self.runtime
            .set_inspector_tracing(window_id.into_sui(), enabled)
            .map_err(|error| error.to_string())
    }

    pub fn inspector_snapshot(
        &self,
        window_id: BindingWindowId,
    ) -> Result<BindingInspectorSnapshot, String> {
        self.runtime
            .inspector_snapshot(window_id.into_sui())
            .map(BindingInspectorSnapshot::from)
            .map_err(|error| error.to_string())
    }

    pub fn handle_event_at(&mut self, index: usize, event: BindingEvent) -> Result<(), String> {
        let window_id = self.window_id_at(index)?;
        self.handle_event(window_id, event)
    }

    pub fn hover_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
    ) -> Result<(), String> {
        if !node.visible() || node.disabled {
            return Err("semantic node is not actionable".to_owned());
        }
        self.handle_event_at(
            index,
            BindingEvent::Pointer(BindingPointerEvent::new(
                BindingPointerEventKind::Move,
                node.center(),
            )),
        )
    }

    pub fn click_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
    ) -> Result<(), String> {
        self.hover_node_at(index, node)?;
        let mut down = BindingPointerEvent::new(BindingPointerEventKind::Down, node.center());
        down.button = Some(BindingPointerButton::Primary);
        down.buttons = 1;
        self.handle_event_at(index, BindingEvent::Pointer(down))?;
        let mut up = BindingPointerEvent::new(BindingPointerEventKind::Up, node.center());
        up.button = Some(BindingPointerButton::Primary);
        self.handle_event_at(index, BindingEvent::Pointer(up))
    }

    pub fn press_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
        key: impl Into<String>,
    ) -> Result<(), String> {
        self.click_node_at(index, node)?;
        let key = key.into();
        self.handle_event_at(
            index,
            BindingEvent::Keyboard(BindingKeyboardEvent::new(
                key.clone(),
                BindingKeyState::Pressed,
            )),
        )?;
        self.handle_event_at(
            index,
            BindingEvent::Keyboard(BindingKeyboardEvent::new(key, BindingKeyState::Released)),
        )
    }

    pub fn fill_node_at(
        &mut self,
        index: usize,
        node: &BindingSemanticNode,
        text: impl Into<String>,
    ) -> Result<(), String> {
        self.click_node_at(index, node)?;
        let text = text.into();
        self.handle_event_at(index, BindingEvent::Ime(BindingImeEvent::CompositionStart))?;
        self.handle_event_at(
            index,
            BindingEvent::Ime(BindingImeEvent::CompositionUpdate {
                text: text.clone(),
                cursor_start: None,
                cursor_end: None,
            }),
        )?;
        self.handle_event_at(
            index,
            BindingEvent::Ime(BindingImeEvent::CompositionCommit { text }),
        )?;
        self.handle_event_at(index, BindingEvent::Ime(BindingImeEvent::CompositionEnd))
    }

    pub fn handle_event(
        &mut self,
        window_id: BindingWindowId,
        event: BindingEvent,
    ) -> Result<(), String> {
        let event = event.into_sui_event()?;
        self.runtime
            .handle_event(window_id.into_sui(), event)
            .map_err(|error| error.to_string())?;
        self.drain_ui_tasks()?;
        Ok(())
    }

    pub fn wake_window(&mut self, window_id: BindingWindowId) -> Result<(), String> {
        self.runtime
            .wake_root(window_id.into_sui())
            .map_err(|error| error.to_string())
    }

    pub fn needs_render(&self, window_id: BindingWindowId) -> Result<bool, String> {
        self.runtime
            .needs_render(window_id.into_sui())
            .map_err(|error| error.to_string())
    }

    pub fn render_window_at(&mut self, index: usize) -> Result<BindingRenderSnapshot, String> {
        let window_id = self.window_id_at(index)?;
        self.render_window(window_id)
    }

    pub fn render_window(
        &mut self,
        window_id: BindingWindowId,
    ) -> Result<BindingRenderSnapshot, String> {
        self.drain_ui_tasks()?;
        let output = self
            .runtime
            .render(window_id.into_sui())
            .map_err(|error| error.to_string())?;
        let mut command_count = 0;
        let mut fill_rect_count = 0;
        let mut draw_image_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            command_count += 1;
            match command {
                SceneCommand::FillRect { .. } => fill_rect_count += 1,
                SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                    draw_image_count += 1;
                }
                _ => {}
            }
        });
        Ok(BindingRenderSnapshot {
            command_count,
            semantics_count: output.semantics.len(),
            semantics_nodes: binding_semantics_nodes(&output.semantics),
            semantics_roles: binding_semantics_roles(&output.semantics),
            semantics_names: binding_semantics_names(&output.semantics),
            semantics_values: binding_semantics_values(&output.semantics),
            semantics_descriptions: binding_semantics_descriptions(&output.semantics),
            semantics_checked: binding_semantics_checked(&output.semantics),
            semantics_busy: binding_semantics_busy(&output.semantics),
            semantics_editable_multiline: binding_semantics_editable_multiline(&output.semantics),
            semantics_disabled: binding_semantics_disabled(&output.semantics),
            semantics_focused: binding_semantics_focused(&output.semantics),
            semantics_hidden: binding_semantics_hidden(&output.semantics),
            semantics_hovered: binding_semantics_hovered(&output.semantics),
            semantics_selected: binding_semantics_selected(&output.semantics),
            semantics_expanded: binding_semantics_expanded(&output.semantics),
            fill_rect_count,
            draw_image_count,
            registered_font_count: output.frame.font_registry.len(),
            registered_image_count: output.frame.image_registry.len(),
        })
    }
}

impl fmt::Debug for BindingRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingRuntime")
            .field("window_ids", &self.window_ids)
            .field("pending_ui_task_count", &self.pending_ui_task_count())
            .finish()
    }
}

#[cfg(feature = "desktop")]
fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(pixels)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(data)
}
