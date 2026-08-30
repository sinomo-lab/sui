use sui_core::{
    Color, Event, InvalidationKind, KeyState, Path, Point, PointerButton, PointerEventKind, Rect,
    SemanticsAction, SemanticsNode, SemanticsRole, SemanticsValue, Size,
};
use sui_layout::Constraints;
use sui_runtime::{EventCtx, MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_scene::StrokeStyle;
use sui_text::TextStyle;
use sui_widgets::{DefaultTheme, paint_single_line_aligned_text};

use crate::{FitViewOptions, NodeGraphState, Viewport};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NodeControlsAppearance {
    pub background: Option<Color>,
    pub border: Option<Color>,
    pub button: Option<Color>,
    pub button_hovered: Option<Color>,
    pub button_pressed: Option<Color>,
    pub text: Option<Color>,
    pub active: Option<Color>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedAppearance {
    background: Color,
    border: Color,
    button: Color,
    button_hovered: Color,
    button_pressed: Color,
    text: Color,
    active: Color,
}

impl NodeControlsAppearance {
    fn resolve(self, theme: &DefaultTheme) -> ResolvedAppearance {
        ResolvedAppearance {
            background: self.background.unwrap_or(theme.palette.surface_raised),
            border: self.border.unwrap_or(theme.palette.border),
            button: self.button.unwrap_or(theme.palette.control),
            button_hovered: self.button_hovered.unwrap_or(theme.palette.control_hover),
            button_pressed: self.button_pressed.unwrap_or(theme.palette.control_active),
            text: self.text.unwrap_or(theme.palette.text),
            active: self.active.unwrap_or(theme.palette.accent),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlAction {
    ZoomIn,
    ZoomOut,
    FitView,
    ToggleInteractive,
}

impl ControlAction {
    const fn label(self, interactive: bool) -> &'static str {
        match self {
            Self::ZoomIn => "+",
            Self::ZoomOut => "−",
            Self::FitView => "Fit",
            Self::ToggleInteractive if interactive => "Lock",
            Self::ToggleInteractive => "Edit",
        }
    }

    const fn accessible_name(self, interactive: bool) -> &'static str {
        match self {
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::FitView => "Fit view",
            Self::ToggleInteractive if interactive => "Lock graph",
            Self::ToggleInteractive => "Unlock graph",
        }
    }
}

pub struct NodeControls<N = (), E = ()> {
    name: String,
    state: NodeGraphState<N, E>,
    theme: DefaultTheme,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    appearance: NodeControlsAppearance,
    show_zoom: bool,
    show_fit_view: bool,
    show_interactive: bool,
    min_zoom: f32,
    max_zoom: f32,
    zoom_factor: f32,
    transition_duration: f64,
    fit_view: FitViewOptions,
    button_extent: f32,
    hovered: Option<ControlAction>,
    pressed: Option<ControlAction>,
    focused_index: usize,
}

impl<N, E> NodeControls<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    pub fn new(name: impl Into<String>, state: NodeGraphState<N, E>) -> Self {
        Self {
            name: name.into(),
            state,
            theme: DefaultTheme::default(),
            theme_reader: None,
            appearance: NodeControlsAppearance::default(),
            show_zoom: true,
            show_fit_view: true,
            show_interactive: true,
            min_zoom: 0.1,
            max_zoom: 4.0,
            zoom_factor: 1.2,
            transition_duration: 0.0,
            fit_view: FitViewOptions::default(),
            button_extent: 34.0,
            hovered: None,
            pressed: None,
            focused_index: 0,
        }
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = theme;
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn appearance(mut self, appearance: NodeControlsAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn show_zoom(mut self, show: bool) -> Self {
        self.show_zoom = show;
        self
    }

    pub fn show_fit_view(mut self, show: bool) -> Self {
        self.show_fit_view = show;
        self
    }

    pub fn show_interactive(mut self, show: bool) -> Self {
        self.show_interactive = show;
        self
    }

    pub fn zoom_range(mut self, min_zoom: f32, max_zoom: f32) -> Self {
        self.min_zoom = min_zoom.max(0.001);
        self.max_zoom = max_zoom.max(self.min_zoom);
        self
    }

    pub fn zoom_factor(mut self, factor: f32) -> Self {
        self.zoom_factor = factor.max(1.01);
        self
    }

    pub fn transition_duration(mut self, duration: f64) -> Self {
        self.transition_duration = duration.max(0.0);
        self
    }

    pub fn fit_view_options(mut self, options: FitViewOptions) -> Self {
        self.fit_view = options;
        self
    }

    pub fn button_extent(mut self, extent: f32) -> Self {
        self.button_extent = extent.max(24.0);
        self
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or(self.theme)
    }

    fn actions(&self) -> Vec<ControlAction> {
        let mut actions = Vec::with_capacity(4);
        if self.show_zoom {
            actions.extend([ControlAction::ZoomIn, ControlAction::ZoomOut]);
        }
        if self.show_fit_view {
            actions.push(ControlAction::FitView);
        }
        if self.show_interactive {
            actions.push(ControlAction::ToggleInteractive);
        }
        actions
    }

    fn action_at(&self, bounds: Rect, position: Point) -> Option<ControlAction> {
        if !bounds.contains(position) {
            return None;
        }
        let index = ((position.y - bounds.y()) / self.button_extent).floor() as usize;
        self.actions().get(index).copied()
    }

    fn action_rect(&self, bounds: Rect, index: usize) -> Rect {
        let y = bounds.y() + (index as f32 * self.button_extent);
        Rect::new(
            bounds.x(),
            y,
            bounds.width(),
            (bounds.max_y() - y).clamp(0.0, self.button_extent),
        )
    }

    fn activate(&self, action: ControlAction) {
        match action {
            ControlAction::ZoomIn => {
                self.zoom_by(self.zoom_factor);
            }
            ControlAction::ZoomOut => {
                self.zoom_by(1.0 / self.zoom_factor);
            }
            ControlAction::FitView => {
                let snapshot = self.state.snapshot();
                if let Some(viewport) = snapshot
                    .graph
                    .bounds()
                    .and_then(|bounds| Viewport::fit(bounds, snapshot.viewport_size, self.fit_view))
                {
                    if self.transition_duration > 0.0 {
                        self.state
                            .animate_viewport(viewport, self.transition_duration);
                    } else {
                        self.state.set_viewport(viewport);
                    }
                }
            }
            ControlAction::ToggleInteractive => {
                self.state.toggle_interactive();
            }
        }
    }

    fn zoom_by(&self, factor: f32) {
        if self.transition_duration <= 0.0 {
            self.state.zoom_by(factor, self.min_zoom, self.max_zoom);
            return;
        }
        let snapshot = self.state.snapshot();
        let bounds = Rect::from_origin_size(Point::ZERO, snapshot.viewport_size);
        let mut viewport = snapshot.viewport;
        viewport.zoom_at(
            bounds,
            Point::new(
                snapshot.viewport_size.width * 0.5,
                snapshot.viewport_size.height * 0.5,
            ),
            factor,
            self.min_zoom,
            self.max_zoom,
        );
        self.state
            .animate_viewport(viewport, self.transition_duration);
    }
}

impl<N, E> Widget for NodeControls<N, E>
where
    N: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let snapshot = ctx.observe(&self.state.signal, InvalidationKind::Paint);
        let actions = self.actions();
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                let hovered = self.action_at(ctx.bounds(), pointer.position);
                if self.hovered != hovered {
                    self.hovered = hovered;
                    ctx.request_paint();
                    ctx.request_semantics();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                if let Some(action) = self.action_at(ctx.bounds(), pointer.position) {
                    self.pressed = Some(action);
                    self.focused_index =
                        actions.iter().position(|item| *item == action).unwrap_or(0);
                    ctx.request_focus();
                    ctx.request_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = self.action_at(ctx.bounds(), pointer.position);
                if let Some(pressed) = self.pressed
                    && Some(pressed) == hovered
                {
                    self.activate(pressed);
                }
                self.pressed = None;
                self.hovered = hovered;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed.take().is_some() {
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.set_handled();
                }
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Leave => {
                if self.pressed.is_none() && self.hovered.take().is_some() {
                    ctx.request_paint();
                }
            }
            Event::Keyboard(key) if ctx.is_focused() && key.state == KeyState::Pressed => {
                if actions.is_empty() {
                    return;
                }
                match key.key.as_str() {
                    "ArrowUp" => self.focused_index = self.focused_index.saturating_sub(1),
                    "ArrowDown" => {
                        self.focused_index = (self.focused_index + 1).min(actions.len() - 1);
                    }
                    "Home" => self.focused_index = 0,
                    "End" => self.focused_index = actions.len() - 1,
                    "Enter" | " " => self.activate(actions[self.focused_index]),
                    _ => return,
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
        let _ = snapshot;
    }

    fn measure(&mut self, _ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.button_extent,
            self.button_extent * self.actions().len() as f32,
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let snapshot = ctx.observe(&self.state.signal);
        let theme = self.resolved_theme();
        let appearance = self.appearance.resolve(&theme);
        let bounds = ctx.bounds();
        let actions = self.actions();
        ctx.fill(
            Path::rounded_rect(bounds, theme.metrics.corner_radius),
            appearance.background,
        );
        ctx.stroke(
            Path::rounded_rect(bounds, theme.metrics.corner_radius),
            appearance.border,
            StrokeStyle::new(theme.metrics.border_width),
        );
        ctx.push_clip(Path::rounded_rect(bounds, theme.metrics.corner_radius));
        for (index, action) in actions.iter().copied().enumerate() {
            let rect = self.action_rect(bounds, index);
            let fill = if self.pressed == Some(action) {
                appearance.button_pressed
            } else if self.hovered == Some(action) {
                appearance.button_hovered
            } else {
                appearance.button
            };
            ctx.fill_rect(rect, fill);
            if index > 0 {
                let mut divider = Path::builder();
                divider
                    .move_to(Point::new(rect.x(), rect.y()))
                    .line_to(Point::new(rect.max_x(), rect.y()));
                ctx.stroke(divider.build(), appearance.border, StrokeStyle::new(1.0));
            }
            if ctx.is_focused() && index == self.focused_index {
                ctx.stroke_rect(
                    rect.inflate(-2.0, -2.0),
                    appearance.active,
                    StrokeStyle::new(1.5),
                );
            }
            let text_style = TextStyle {
                font_size: if matches!(action, ControlAction::ZoomIn | ControlAction::ZoomOut) {
                    theme.text.lg.size
                } else {
                    theme.text.xs.size
                },
                color: if action == ControlAction::ToggleInteractive && !snapshot.interactive {
                    appearance.active
                } else {
                    appearance.text
                },
                ..theme.body_text_style()
            };
            paint_single_line_aligned_text(
                ctx,
                rect.inflate(-4.0, -2.0),
                action.label(snapshot.interactive),
                &text_style,
                text_style.line_height,
                0.5,
            );
        }
        ctx.pop_clip();
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let snapshot = ctx.observe(&self.state.signal);
        let actions = self.actions();
        let focused_action = actions.get(self.focused_index).copied();
        let mut node = SemanticsNode::new(
            ctx.widget_id(),
            SemanticsRole::GenericContainer,
            ctx.bounds(),
        );
        node.name = Some(self.name.clone());
        node.description = Some("Zoom, fit-view, and graph interaction controls".to_string());
        node.value = focused_action.map(|action| {
            SemanticsValue::Text(action.accessible_name(snapshot.interactive).to_string())
        });
        node.state.focused = ctx.is_focused();
        node.actions = vec![SemanticsAction::Focus, SemanticsAction::Activate];
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

#[cfg(test)]
mod tests {
    use sui_core::Point;

    use super::*;
    use crate::Node;

    #[test]
    fn controls_toggle_shared_interactivity_and_zoom() {
        let state =
            NodeGraphState::<(), ()>::new(vec![Node::new("node", Point::ZERO, ())], Vec::new())
                .unwrap();
        state.set_viewport_size(Size::new(640.0, 480.0));
        let controls = NodeControls::new("Controls", state.clone());

        controls.activate(ControlAction::ZoomIn);
        controls.activate(ControlAction::ToggleInteractive);

        assert!(state.viewport().zoom > 1.0);
        assert!(!state.is_interactive());
    }
}
