pub(crate) const SHADER_SOURCE: &str = include_str!("shaders/solid.wgsl");
pub(crate) const TEXTURED_SHADER_SOURCE: &str = include_str!("shaders/textured.wgsl");
pub(crate) const WIDGET_SHADER_SOURCE: &str = include_str!("shaders/widget.wgsl");
pub(crate) const ROUNDED_RECT_SHADER_SOURCE: &str = include_str!("shaders/rounded_rect.wgsl");
// Linear-gradient brush. The gradient is packed entirely into vertex attributes
// (bind-group-free, like the rounded-rect pipeline): two stops carried in `color`
// (stop 0, linear) and `border_color` (stop 1, linear); the gradient axis end-points
// (rect-local) in p2 = [start.x, start.y, end.x, end.y]. Coverage reuses the rounded
// rect SDF so the same pipeline fills both sharp FillRect (radii = 0) and rounded fills.
pub(crate) const GRADIENT_RECT_SHADER_SOURCE: &str = include_str!("shaders/gradient_rect.wgsl");
pub(crate) const TEXT_ATLAS_SHADER_SOURCE: &str = include_str!("shaders/text_atlas.wgsl");
pub(crate) const TEXT_ATLAS_DUAL_SOURCE_SHADER_SOURCE: &str =
    include_str!("shaders/text_atlas_dual_source.wgsl");
pub(crate) const ANALYTIC_PATH_SHADER_SOURCE: &str = include_str!("shaders/analytic_path.wgsl");
pub(crate) const OUTPUT_TRANSFORM_SHADER_SOURCE: &str =
    include_str!("shaders/output_transform.wgsl");
