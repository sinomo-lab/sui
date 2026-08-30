use sui_core::{Point, Rect, Size, Transform, Vector};
use sui_widgets::CanvasViewport;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitViewOptions {
    /// Padding in logical screen pixels around the fitted graph.
    pub padding: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
}

impl Default for FitViewOptions {
    fn default() -> Self {
        Self {
            padding: 32.0,
            min_zoom: 0.1,
            max_zoom: 4.0,
        }
    }
}

impl FitViewOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    pub fn zoom_range(mut self, min_zoom: f32, max_zoom: f32) -> Self {
        self.min_zoom = min_zoom.max(0.001);
        self.max_zoom = max_zoom.max(self.min_zoom);
        self
    }

    pub(crate) fn normalized(self) -> Self {
        let min_zoom = self.min_zoom.max(0.001);
        Self {
            padding: self.padding.max(0.0),
            min_zoom,
            max_zoom: self.max_zoom.max(min_zoom),
        }
    }
}

/// A React Flow/xyflow-style viewport transform.
///
/// `x` and `y` are logical-pixel translations from the widget's top-left,
/// while `zoom` scales flow coordinates into screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    pub const fn new(x: f32, y: f32, zoom: f32) -> Self {
        Self { x, y, zoom }
    }

    pub fn normalized(self, min_zoom: f32, max_zoom: f32) -> Self {
        let min_zoom = min_zoom.max(0.001);
        let max_zoom = max_zoom.max(min_zoom);
        Self {
            x: finite_or(self.x, 0.0),
            y: finite_or(self.y, 0.0),
            zoom: finite_or(self.zoom, 1.0).clamp(min_zoom, max_zoom),
        }
    }

    pub fn transform(self, bounds: Rect) -> Transform {
        Transform::scale(self.zoom, self.zoom).then(Transform::translation(
            bounds.x() + self.x,
            bounds.y() + self.y,
        ))
    }

    pub fn flow_to_screen(self, bounds: Rect, point: Point) -> Point {
        Point::new(
            bounds.x() + self.x + (point.x * self.zoom),
            bounds.y() + self.y + (point.y * self.zoom),
        )
    }

    pub fn screen_to_flow(self, bounds: Rect, point: Point) -> Point {
        let zoom = self.zoom.max(0.001);
        Point::new(
            (point.x - bounds.x() - self.x) / zoom,
            (point.y - bounds.y() - self.y) / zoom,
        )
    }

    pub fn flow_rect_to_screen(self, bounds: Rect, rect: Rect) -> Rect {
        Rect::new(
            bounds.x() + self.x + (rect.x() * self.zoom),
            bounds.y() + self.y + (rect.y() * self.zoom),
            rect.width() * self.zoom,
            rect.height() * self.zoom,
        )
    }

    pub fn screen_rect_to_flow(self, bounds: Rect, rect: Rect) -> Rect {
        let origin = self.screen_to_flow(bounds, rect.origin);
        Rect::new(
            origin.x,
            origin.y,
            rect.width() / self.zoom.max(0.001),
            rect.height() / self.zoom.max(0.001),
        )
    }

    pub fn visible_flow_rect(self, bounds: Rect) -> Rect {
        self.screen_rect_to_flow(bounds, bounds)
    }

    pub fn pan_by(&mut self, delta: Vector) {
        self.x += delta.x;
        self.y += delta.y;
    }

    /// Convert the xyflow-style transform into SUI Canvas coordinates for a
    /// viewport of `viewport_size`.
    pub fn to_canvas(self, viewport_size: Size) -> CanvasViewport {
        CanvasViewport {
            pan: Vector::new(
                self.x - (viewport_size.width * 0.5),
                self.y - (viewport_size.height * 0.5),
            ),
            zoom: self.zoom,
            rotation: 0.0,
        }
    }

    /// Convert an unrotated SUI Canvas viewport into graph coordinates.
    /// Rotated canvases return `None` because retained node widgets remain
    /// axis-aligned.
    pub fn from_canvas(viewport: CanvasViewport, viewport_size: Size) -> Option<Self> {
        if !viewport.pan.x.is_finite()
            || !viewport.pan.y.is_finite()
            || !viewport.zoom.is_finite()
            || viewport.zoom <= 0.0
            || !viewport.rotation.is_finite()
            || viewport.rotation.abs() > 0.0001
        {
            return None;
        }
        Some(Self {
            x: viewport.pan.x + (viewport_size.width * 0.5),
            y: viewport.pan.y + (viewport_size.height * 0.5),
            zoom: viewport.zoom,
        })
    }

    pub fn zoom_at(
        &mut self,
        bounds: Rect,
        anchor: Point,
        factor: f32,
        min_zoom: f32,
        max_zoom: f32,
    ) {
        let flow_anchor = self.screen_to_flow(bounds, anchor);
        let min_zoom = min_zoom.max(0.001);
        let max_zoom = max_zoom.max(min_zoom);
        self.zoom = (self.zoom * factor.max(0.001)).clamp(min_zoom, max_zoom);
        self.x = anchor.x - bounds.x() - (flow_anchor.x * self.zoom);
        self.y = anchor.y - bounds.y() - (flow_anchor.y * self.zoom);
    }

    pub fn centered_on(
        point: Point,
        viewport_size: Size,
        zoom: f32,
        min_zoom: f32,
        max_zoom: f32,
    ) -> Self {
        let min_zoom = min_zoom.max(0.001);
        let zoom = zoom.clamp(min_zoom, max_zoom.max(min_zoom));
        Self {
            x: (viewport_size.width * 0.5) - (point.x * zoom),
            y: (viewport_size.height * 0.5) - (point.y * zoom),
            zoom,
        }
    }

    pub fn fit(graph_bounds: Rect, viewport_size: Size, options: FitViewOptions) -> Option<Self> {
        if graph_bounds.is_empty() || viewport_size.is_empty() {
            return None;
        }
        let options = options.normalized();
        let available_width = (viewport_size.width - (options.padding * 2.0)).max(1.0);
        let available_height = (viewport_size.height - (options.padding * 2.0)).max(1.0);
        let zoom = (available_width / graph_bounds.width())
            .min(available_height / graph_bounds.height())
            .clamp(options.min_zoom, options.max_zoom);
        let center = Point::new(
            graph_bounds.x() + (graph_bounds.width() * 0.5),
            graph_bounds.y() + (graph_bounds.height() * 0.5),
        );
        Some(Self::centered_on(
            center,
            viewport_size,
            zoom,
            options.min_zoom,
            options.max_zoom,
        ))
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    #[test]
    fn screen_and_flow_coordinates_round_trip() {
        let viewport = Viewport::new(84.0, -32.0, 1.75);
        let bounds = Rect::new(20.0, 40.0, 800.0, 600.0);
        let flow = Point::new(123.0, -48.0);

        let round_trip = viewport.screen_to_flow(bounds, viewport.flow_to_screen(bounds, flow));

        assert_close(round_trip.x, flow.x);
        assert_close(round_trip.y, flow.y);
    }

    #[test]
    fn anchored_zoom_keeps_the_flow_point_under_the_pointer() {
        let bounds = Rect::new(10.0, 20.0, 640.0, 480.0);
        let anchor = Point::new(240.0, 180.0);
        let mut viewport = Viewport::new(30.0, 40.0, 1.0);
        let before = viewport.screen_to_flow(bounds, anchor);

        viewport.zoom_at(bounds, anchor, 2.0, 0.1, 4.0);

        let after = viewport.screen_to_flow(bounds, anchor);
        assert_close(after.x, before.x);
        assert_close(after.y, before.y);
        assert_close(viewport.zoom, 2.0);
    }

    #[test]
    fn canvas_viewport_conversion_preserves_world_to_screen_mapping() {
        let viewport = Viewport::new(84.0, -32.0, 1.75);
        let bounds = Rect::new(20.0, 40.0, 800.0, 600.0);
        let flow = Point::new(123.0, -48.0);
        let canvas = viewport.to_canvas(bounds.size);

        assert_eq!(
            viewport.flow_to_screen(bounds, flow),
            canvas.world_to_screen(bounds, flow, Point::ZERO)
        );
        assert_eq!(Viewport::from_canvas(canvas, bounds.size), Some(viewport));
        assert!(Viewport::from_canvas(CanvasViewport::new().rotation(0.25), bounds.size).is_none());
    }

    #[test]
    fn fit_centers_the_graph_with_padding() {
        let fitted = Viewport::fit(
            Rect::new(100.0, 50.0, 400.0, 200.0),
            Size::new(1000.0, 600.0),
            FitViewOptions::default().padding(100.0),
        )
        .unwrap();

        assert_close(fitted.zoom, 2.0);
        assert_close(fitted.x, -100.0);
        assert_close(fitted.y, 0.0);
    }
}
