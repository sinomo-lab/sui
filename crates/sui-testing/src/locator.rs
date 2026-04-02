use std::{cell::RefCell, rc::Rc};

use sui_core::{
    Error, Event, ImeEvent, KeyState, KeyboardEvent, Point, PointerButton, PointerButtons,
    PointerEvent, PointerEventKind, Result, SemanticsNode, WindowId,
};

use crate::{
    diagnostics::format_failure,
    expect::Expectation,
    harness::Harness,
    selector::Selector,
};

#[derive(Clone)]
pub struct Locator {
    harness: Rc<RefCell<Harness>>,
    window_id: WindowId,
    selector: Selector,
}

impl Locator {
    pub(crate) fn new(
        harness: Rc<RefCell<Harness>>,
        window_id: WindowId,
        selector: Selector,
    ) -> Self {
        Self {
            harness,
            window_id,
            selector,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.selector = self.selector.with_name(name);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.selector = self.selector.with_description(description);
        self
    }

    pub fn expect(&self) -> Expectation {
        Expectation::new(self.clone())
    }

    pub fn count(&self) -> Result<usize> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        Ok(self.resolve_all(&harness)?.len())
    }

    pub fn hover(&self) -> Result<()> {
        let point = self.action_point("hover")?;
        self.dispatch_event(Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)))
    }

    pub fn click(&self) -> Result<()> {
        let point = self.action_point("click")?;
        self.dispatch_event(Event::Pointer(PointerEvent::new(PointerEventKind::Move, point)))?;

        let mut down = PointerEvent::new(PointerEventKind::Down, point);
        down.button = Some(PointerButton::Primary);
        down.buttons = PointerButtons::new(1);
        self.dispatch_event(Event::Pointer(down))?;

        let mut up = PointerEvent::new(PointerEventKind::Up, point);
        up.button = Some(PointerButton::Primary);
        self.dispatch_event(Event::Pointer(up))
    }

    pub fn focus(&self) -> Result<()> {
        if self.is_focused()? {
            return Ok(());
        }

        self.click()?;

        let mut harness = self.harness.borrow_mut();
        let timeout = harness.default_timeout();
        harness
            .run_until(timeout, |harness| {
                Ok(self.resolve_unique(harness).ok().filter(|node| node.state.focused))
            })
            .map(|_| ())
            .map_err(|_| self.failure("focus", "locator did not become focused"))
    }

    pub fn press(&self, key: impl Into<String>) -> Result<()> {
        self.focus()?;
        let key = key.into();
        self.dispatch_event(Event::Keyboard(KeyboardEvent::new(key.clone(), KeyState::Pressed)))?;
        self.dispatch_event(Event::Keyboard(KeyboardEvent::new(key, KeyState::Released)))
    }

    pub fn fill(&self, text: impl Into<String>) -> Result<()> {
        self.focus()?;
        let text = text.into();
        self.dispatch_event(Event::Ime(ImeEvent::CompositionStart))?;
        self.dispatch_event(Event::Ime(ImeEvent::CompositionUpdate { text: text.clone() }))?;
        self.dispatch_event(Event::Ime(ImeEvent::CompositionCommit { text }))?;
        self.dispatch_event(Event::Ime(ImeEvent::CompositionEnd))
    }

    pub fn dispatch_event(&self, event: Event) -> Result<()> {
        self.harness.borrow_mut().dispatch_event(self.window_id, event)
    }

    pub(crate) fn harness(&self) -> &Rc<RefCell<Harness>> {
        &self.harness
    }

    pub(crate) fn selector(&self) -> &Selector {
        &self.selector
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) fn default_timeout(&self) -> f64 {
        self.harness.borrow().default_timeout()
    }

    pub(crate) fn resolve_all(&self, harness: &Harness) -> Result<Vec<SemanticsNode>> {
        let snapshot = harness.snapshot(self.window_id)?;
        Ok(snapshot
            .accessibility
            .nodes
            .iter()
            .filter(|node| self.selector.matches(&snapshot.accessibility, node))
            .cloned()
            .collect())
    }

    pub(crate) fn resolve_unique(&self, harness: &Harness) -> Result<SemanticsNode> {
        let nodes = self.resolve_all(harness)?;
        match nodes.as_slice() {
            [node] => Ok(node.clone()),
            [] => Err(Error::new("locator did not match any nodes")),
            _ => Err(Error::new(format!(
                "locator matched {} nodes instead of exactly one",
                nodes.len()
            ))),
        }
    }

    fn action_point(&self, action: &str) -> Result<Point> {
        let mut harness = self.harness.borrow_mut();
        let timeout = harness.default_timeout();
        harness
            .run_until(timeout, |harness| {
                let Ok(node) = self.resolve_unique(harness) else {
                    return Ok(None);
                };
                if !self.selector.is_visible(&node) || node.state.disabled {
                    return Ok(None);
                }

                Ok(Some(center(node.bounds)))
            })
            .map_err(|_| self.failure(action, "locator never became uniquely actionable"))
    }

    fn is_focused(&self) -> Result<bool> {
        let mut harness = self.harness.borrow_mut();
        harness.run_until_idle()?;
        Ok(self
            .resolve_unique(&harness)
            .is_ok_and(|node| node.state.focused))
    }

    fn failure(&self, action: &str, detail: &str) -> Error {
        let harness = self.harness.borrow();
        let snapshot = harness
            .snapshot(self.window_id)
            .unwrap_or_else(|_| harness.fallback_snapshot(self.window_id));
        Error::new(format_failure(action, &self.selector, &snapshot, detail))
    }
}

fn center(bounds: sui_core::Rect) -> Point {
    Point::new(
        bounds.x() + (bounds.width() / 2.0),
        bounds.y() + (bounds.height() / 2.0),
    )
}
