
const RR_MODE_FILL: f32 = 0.0;
const RR_MODE_SHADOW: f32 = 1.0;
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
fn gaussian_box_coverage(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>, sigma: f32) -> f32 {
    let s = max(sigma, 1e-3); let d = sd_round_box(p, b, r); let edge = 1.4142136 * s;
    return 1.0 - smoothstep(-edge, edge, d);
}
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let half = max(in.p0.xy, vec2<f32>(0.0));
    let mode = in.p0.z; let feather = in.p0.w;
    let radii = clamp(in.radii, vec4<f32>(0.0), vec4<f32>(min(half.x, half.y)));
    let p = in.local;
    // Derivatives must execute in uniform control flow on WebGPU. Compute the
    // fill edge before selecting the shadow path; the shadow branch simply
    // leaves the derivative result unused.
    let d = sd_round_box(p, half, radii);
    let aa = max(feather, fwidth(d));
    if (mode == RR_MODE_SHADOW) {
        let sigma = in.p2.y;
        let pp = p - vec2<f32>(in.p2.z, in.p2.w);
        let cov = gaussian_box_coverage(pp, half, radii, sigma);
        return vec4<f32>(in.color.rgb, in.color.a * cov);
    }
    let fill_cov = clamp(0.5 - d / max(aa, 1e-4), 0.0, 1.0);
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
