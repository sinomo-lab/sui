
const RR_MODE_FILL: f32 = 0.0;
// Shadows of the box: beneath and around it, around it only, or within it.
const RR_MODE_SHADOW_BEHIND: f32 = 1.0;
const RR_MODE_SHADOW_OUTSIDE: f32 = 2.0;
const RR_MODE_SHADOW_INSIDE: f32 = 3.0;
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>, @location(1) local: vec2<f32>,
    @location(2) p0: vec4<f32>, @location(3) radii: vec4<f32>,
    @location(4) p2: vec4<f32>, @location(5) border_color: vec4<f32>,
};
@vertex
fn vs_main(@location(0) corner: vec2<f32>, @location(1) ndc_min: vec2<f32>,
    @location(2) ndc_max: vec2<f32>, @location(3) local_min: vec2<f32>,
    @location(4) local_max: vec2<f32>, @location(5) color: vec4<f32>,
    @location(6) p0: vec4<f32>, @location(7) radii: vec4<f32>,
    @location(8) p2: vec4<f32>, @location(9) border_color: vec4<f32>) -> VsOut {
    var out: VsOut; out.position = vec4<f32>(mix(ndc_min, ndc_max, corner), 0.0, 1.0);
    out.color = color; out.local = mix(local_min, local_max, corner); out.p0 = p0;
    out.radii = radii; out.p2 = p2; out.border_color = border_color;
    return out;
}
fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    let rt = select(r.x, r.y, p.x > 0.0);
    let rb = select(r.w, r.z, p.x > 0.0);
    let rr = select(rt, rb, p.y > 0.0);
    let q = abs(p) - b + vec2<f32>(rr, rr);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}
// The error function, to within 5e-4 (Abramowitz and Stegun 7.1.27).
fn erf2(x: vec2<f32>) -> vec2<f32> {
    let a = abs(x);
    let t = 1.0 + a * (0.278393 + a * (0.230389 + a * (0.000972 + a * 0.078108)));
    let t2 = t * t;
    return sign(x) * (1.0 - 1.0 / (t2 * t2));
}
// Coverage near x of the horizontal slice through a rounded box at height y,
// blurred along x. The slice runs between the box's left and right edges,
// which the corners of the half it is in pull inward. inv_width is
// 1 / (sqrt(2) sigma). Corner radii are ordered top-left, top-right,
// bottom-right, bottom-left, with y growing downward.
fn blurred_slice(x: f32, y: f32, half: vec2<f32>, r: vec4<f32>, inv_width: f32) -> f32 {
    let corners = select(r.xy, r.wz, y > 0.0);
    let into_corner = min(half.y - corners - abs(y), vec2<f32>(0.0));
    let reach = half.x - corners
        + sqrt(max(corners * corners - into_corner * into_corner, vec2<f32>(0.0)));
    let edges = erf2(vec2<f32>(x + reach.x, x - reach.y) * inv_width);
    return 0.5 * (edges.x - edges.y);
}
// How much of a rounded box, blurred by a Gaussian of deviation sigma, covers
// p. Across the box the blur is integrated exactly. Along it, the box is cut
// into four bands within three deviations of p, the outer two running on to
// the box's ends, and each band's slice at its middle is weighted by the
// Gaussian's exact mass over the band. Where the box is uniform the result is
// exact, so a large box is fully covered inside.
fn blurred_box(p: vec2<f32>, half: vec2<f32>, r: vec4<f32>, sigma: f32) -> f32 {
    let low = p.y - half.y;
    let high = p.y + half.y;
    let start = clamp(-3.0 * sigma, low, high);
    let end = clamp(3.0 * sigma, low, high);
    let step = (end - start) * 0.25;
    let inv_width = 0.70710678 / sigma;
    // The Gaussian's cumulative mass at the bands' edges, doubled.
    let lower = erf2(vec2<f32>(low, start + step) * inv_width);
    let upper = erf2(vec2<f32>(start + 2.0 * step, start + 3.0 * step) * inv_width);
    let last = erf2(vec2<f32>(high, 0.0) * inv_width).x;
    let mass = 0.5 * vec4<f32>(lower.y - lower.x, upper.x - lower.y, upper.y - upper.x,
        last - upper.y);
    let x = p.x;
    return blurred_slice(x, p.y - (start + 0.5 * step), half, r, inv_width) * mass.x
        + blurred_slice(x, p.y - (start + 1.5 * step), half, r, inv_width) * mass.y
        + blurred_slice(x, p.y - (start + 2.5 * step), half, r, inv_width) * mass.z
        + blurred_slice(x, p.y - (start + 3.5 * step), half, r, inv_width) * mass.w;
}
fn fs_shade(in: VsOut) -> vec4<f32> {
    let half = max(in.p0.xy, vec2<f32>(0.0));
    let mode = in.p0.z; let feather = in.p0.w;
    let radii = clamp(in.radii, vec4<f32>(0.0), vec4<f32>(min(half.x, half.y)));
    let p = in.local;
    // Derivatives must execute in uniform control flow on WebGPU. Compute the
    // fill edge before selecting the shadow path; the shadow branch simply
    // leaves the derivative result unused.
    let d = sd_round_box(p, half, radii);
    let aa = max(feather, fwidth(d));
    let fill_cov = clamp(0.5 - d / max(aa, 1e-4), 0.0, 1.0);
    if (mode >= RR_MODE_SHADOW_BEHIND) {
        // p2 holds the spread, the deviation, and the offset; the border
        // color slot holds the shadow box's corner radii.
        let spread = in.p2.x;
        let sigma = max(in.p2.y, 1e-3);
        let shadow_p = p - in.p2.zw;
        if (mode == RR_MODE_SHADOW_INSIDE) {
            // The shadow fills the box except a blurred hole inset from it.
            let hole = blurred_box(shadow_p, max(half - vec2<f32>(spread), vec2<f32>(0.0)),
                in.border_color, sigma);
            return vec4<f32>(in.color.rgb, in.color.a * fill_cov * (1.0 - hole));
        }
        var cov = blurred_box(shadow_p, max(half + vec2<f32>(spread), vec2<f32>(0.0)),
            in.border_color, sigma);
        if (mode == RR_MODE_SHADOW_OUTSIDE) {
            cov = cov * (1.0 - fill_cov);
        }
        return vec4<f32>(in.color.rgb, in.color.a * cov);
    }
    let bw = in.p2.x;
    if (bw > 0.0) {
        let inner_cov = clamp(0.5 - (d + bw) / max(aa, 1e-4), 0.0, 1.0);
        let ring = clamp(fill_cov - inner_cov, 0.0, 1.0);
        let interior = inner_cov;
        let a = in.border_color.a * ring + in.color.a * interior;
        if (a <= 0.0) { return vec4<f32>(0.0); }
        let rgb = in.border_color.rgb * (in.border_color.a * ring) + in.color.rgb * (in.color.a * interior);
        return vec4<f32>(rgb / a, a);
    }
    return vec4<f32>(in.color.rgb, in.color.a * fill_cov);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return fit_straight(fs_shade(in));
}
