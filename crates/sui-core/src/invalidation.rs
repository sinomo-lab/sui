use crate::{Rect, SurfaceId, WidgetId, WindowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationKind {
    Measure,
    Arrange,
    Transform,
    Clip,
    Effect,
    Visibility,
    Paint,
    HitTest,
    Text,
    Semantics,
    Resources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationTarget {
    Window(WindowId),
    Widget(WidgetId),
    Surface(SurfaceId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirtyRegion {
    pub area: Rect,
    pub kind: InvalidationKind,
}

impl DirtyRegion {
    pub const fn new(area: Rect, kind: InvalidationKind) -> Self {
        Self { area, kind }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvalidationRequest {
    pub target: InvalidationTarget,
    pub kind: InvalidationKind,
    pub region: Option<Rect>,
}

impl InvalidationRequest {
    pub const fn new(target: InvalidationTarget, kind: InvalidationKind) -> Self {
        Self {
            target,
            kind,
            region: None,
        }
    }

    pub const fn with_region(self, region: Rect) -> Self {
        Self {
            region: Some(region),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DirtyRegion, InvalidationKind, InvalidationRequest, InvalidationTarget};
    use crate::{Rect, WidgetId};

    #[test]
    fn invalidation_request_can_capture_target_and_region() {
        let region = Rect::new(5.0, 10.0, 20.0, 25.0);
        let request = InvalidationRequest::new(
            InvalidationTarget::Widget(WidgetId::new(7)),
            InvalidationKind::Paint,
        )
        .with_region(region);

        assert_eq!(request.region, Some(region));
        assert_eq!(request.kind, InvalidationKind::Paint);

        let dirty = DirtyRegion::new(region, InvalidationKind::Paint);
        assert_eq!(dirty.area, region);
    }
}
