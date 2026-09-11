
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) scene_position: vec2<f32>,
    @location(2) @interpolate(flat) path_index: u32,
};

struct AnalyticPathMeta {
    contour_start: u32,
    contour_count: u32,
    point_start: u32,
    mode: u32,
    feather_width: f32,
    stroke_width: f32,
    _pad0: vec2<f32>,
};

struct AnalyticContour {
    start: u32,
    len: u32,
    flags: u32,
    _pad0: u32,
};

const ANALYTIC_CONTOUR_FLAG_CLOSED: u32 = 1u;
const ANALYTIC_PATH_MODE_FILL: u32 = 0u;
const ANALYTIC_PATH_MODE_STROKE: u32 = 1u;

struct AnalyticPoint {
    position: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<storage, read> path_metas: array<AnalyticPathMeta>;

@group(0) @binding(1)
var<storage, read> contours: array<AnalyticContour>;

@group(0) @binding(2)
var<storage, read> points: array<AnalyticPoint>;

@vertex
fn vs_main(
    @location(0) corner: vec2<f32>,
    @location(1) ndc_min: vec2<f32>,
    @location(2) ndc_max: vec2<f32>,
    @location(3) scene_min: vec2<f32>,
    @location(4) scene_max: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(6) path_index: u32,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(mix(ndc_min, ndc_max, corner), 0.0, 1.0);
    out.color = color;
    out.scene_position = mix(scene_min, scene_max, corner);
    out.path_index = path_index;
    return out;
}

fn segment_distance(point: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let denom = max(dot(ab, ab), 1e-5);
    let t = clamp(dot(point - a, ab) / denom, 0.0, 1.0);
    return length(point - (a + (ab * t)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Derivatives must be evaluated before path-indexed storage-buffer access introduces
    // potentially non-uniform control flow. The larger scene-space pixel axis gives a stable
    // one-physical-pixel coverage transition without the sqrt(2) over-blur of combining axes.
    let scene_dx = dpdx(in.scene_position);
    let scene_dy = dpdy(in.scene_position);
    let derivative_width = max(max(length(scene_dx), length(scene_dy)), 1e-4);

    let path_meta = path_metas[in.path_index];
    if path_meta.contour_count == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let point = in.scene_position;
    var inside = false;
    var min_distance = 1e9;

    for (var contour_index = 0u; contour_index < path_meta.contour_count; contour_index = contour_index + 1u) {
        let contour = contours[path_meta.contour_start + contour_index];
        if contour.len < 2u {
            continue;
        }

        let closed = (contour.flags & ANALYTIC_CONTOUR_FLAG_CLOSED) != 0u;
        let point_start = path_meta.point_start + contour.start;
        var previous = select(
            points[point_start].position,
            points[point_start + contour.len - 1u].position,
            closed,
        );
        var start_index = select(1u, 0u, closed);
        for (var point_index = start_index; point_index < contour.len; point_index = point_index + 1u) {
            let current = points[point_start + point_index].position;
                let denom = previous.y - current.y;
                let safe_denom = select(
                    denom,
                    select(-1e-5, 1e-5, denom >= 0.0),
                    abs(denom) < 1e-5,
                );
            let intersects = ((current.y > point.y) != (previous.y > point.y))
                && (point.x < (((previous.x - current.x) * (point.y - current.y))
                / safe_denom) + current.x);
            if intersects {
                inside = !inside;
            }

            min_distance = min(min_distance, segment_distance(point, previous, current));
            previous = current;
        }
    }

    let feather = max(path_meta.feather_width, derivative_width);
    var coverage = 0.0;

    if path_meta.mode == ANALYTIC_PATH_MODE_FILL {
        let signed_distance = select(min_distance, -min_distance, inside);
        coverage = clamp(0.5 - (signed_distance / max(feather, 1e-4)), 0.0, 1.0);
    } else {
        let half_width = max(0.0, 0.5 * path_meta.stroke_width);
        let edge_distance = min_distance - half_width;
        coverage = clamp(0.5 - (edge_distance / max(feather, 1e-4)), 0.0, 1.0);
    }

    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
