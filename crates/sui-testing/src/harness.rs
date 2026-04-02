use sui_core::{Error, Event, Result, WindowId};
use sui_platform::{AccessibilitySnapshot, HeadlessPlatform};
use sui_runtime::{FocusState, Runtime, WidgetGraphSnapshot};

use crate::snapshot::WindowSnapshot;

pub(crate) struct Harness {
    runtime: Runtime,
    platform: HeadlessPlatform,
    default_timeout: f64,
}

impl Harness {
    pub(crate) fn new(runtime: Runtime) -> Result<Self> {
        let mut harness = Self {
            runtime,
            platform: HeadlessPlatform::new(),
            default_timeout: 5.0,
        };
        harness.run_until_idle()?;
        Ok(harness)
    }

    pub(crate) fn default_timeout(&self) -> f64 {
        self.default_timeout
    }

    pub(crate) fn set_default_timeout(&mut self, timeout: f64) {
        self.default_timeout = timeout;
    }

    pub(crate) fn window_ids(&self) -> Vec<WindowId> {
        self.runtime.window_ids()
    }

    pub(crate) fn window_id_by_title(&self, title: &str) -> Option<WindowId> {
        self.runtime.window_ids().into_iter().find(|window_id| {
            self.runtime
                .window_title(*window_id)
                .is_ok_and(|window_title| window_title == title)
        })
    }

    pub(crate) fn advance_time(&mut self, delta: f64) -> Result<()> {
        if delta.is_sign_negative() {
            return Err(Error::new("time delta must be >= 0"));
        }

        self.platform.advance_time(delta);
        self.run_until_idle()
    }

    pub(crate) fn run_until_idle(&mut self) -> Result<()> {
        while self.platform.pump(&mut self.runtime)? {}
        Ok(())
    }

    pub(crate) fn run_until<T, F>(&mut self, timeout: f64, mut predicate: F) -> Result<T>
    where
        F: FnMut(&Self) -> Result<Option<T>>,
    {
        let timeout = timeout.max(0.0);
        let deadline = self.platform.current_time() + timeout;

        loop {
            self.run_until_idle()?;
            if let Some(value) = predicate(self)? {
                return Ok(value);
            }

            let now = self.platform.current_time();
            if now >= deadline {
                break;
            }

            let Some(next_wakeup) = self.next_wakeup_time()? else {
                break;
            };

            if next_wakeup > deadline {
                break;
            }

            self.platform.advance_time((next_wakeup - now).max(0.0));
        }

        self.run_until_idle()?;
        predicate(self)?.ok_or_else(|| Error::new("condition not satisfied before timeout"))
    }

    pub(crate) fn dispatch_event(&mut self, window_id: WindowId, event: Event) -> Result<()> {
        self.platform.dispatch_event(&self.runtime, window_id, event)?;
        self.run_until_idle()
    }

    pub(crate) fn snapshot(&self, window_id: WindowId) -> Result<WindowSnapshot> {
        let accessibility = self
            .platform
            .accessibility_snapshot(window_id)
            .cloned()
            .ok_or_else(|| {
                Error::new(format!(
                    "window {} does not have an accessibility snapshot yet",
                    window_id.get()
                ))
            })?;
        let title = self.runtime.window_title(window_id)?.to_string();
        let focus_state = self.runtime.focus_state(window_id)?;
        let widget_graph = self.runtime.widget_graph(window_id)?;

        Ok(WindowSnapshot {
            window_id,
            title,
            accessibility,
            widget_graph,
            focus_state,
        })
    }

    pub(crate) fn fallback_snapshot(&self, window_id: WindowId) -> WindowSnapshot {
        WindowSnapshot {
            window_id,
            title: self
                .runtime
                .window_title(window_id)
                .unwrap_or("<unknown>")
                .to_string(),
            accessibility: AccessibilitySnapshot {
                window_id,
                root: None,
                focused_widget: None,
                nodes: Vec::new(),
            },
            widget_graph: self.runtime.widget_graph(window_id).unwrap_or(WidgetGraphSnapshot {
                root: Default::default(),
                nodes: Vec::new(),
            }),
            focus_state: self.runtime.focus_state(window_id).unwrap_or(FocusState::default()),
        }
    }

    fn next_wakeup_time(&self) -> Result<Option<f64>> {
        let mut next: Option<f64> = None;
        for window_id in self.runtime.window_ids() {
            let candidate = self.runtime.next_wakeup_time(window_id)?;
            next = match (next, candidate) {
                (Some(current), Some(candidate)) => Some(current.min(candidate)),
                (None, Some(candidate)) => Some(candidate),
                (current, None) => current,
            };
        }
        Ok(next)
    }
}
