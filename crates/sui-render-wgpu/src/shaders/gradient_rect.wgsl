
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) stop0: vec4<f32>, @location(1) local: vec2<f32>,
    @location(2) p0: vec4<f32>, @location(3) radii: vec4<f32>,
    @location(4) axis: vec4<f32>, @location(5) stop1: vec4<f32>,
};
@vertex
fn vs_main(@location(0) corner: vec2<f32>, @location(1) ndc_min: vec2<f32>,
    @location(2) ndc_max: vec2<f32>, @location(3) local_min: vec2<f32>,
    @location(4) local_max: vec2<f32>, @location(5) stop0: vec4<f32>,
    @location(6) p0: vec4<f32>, @location(7) radii: vec4<f32>,
    @location(8) axis: vec4<f32>, @location(9) stop1: vec4<f32>) -> VsOut {
    var out: VsOut; out.position = vec4<f32>(mix(ndc_min, ndc_max, corner), 0.0, 1.0);
    out.stop0 = stop0; out.local = mix(local_min, local_max, corner); out.p0 = p0;
    out.radii = radii; out.axis = axis; out.stop1 = stop1;
    return out;
}
fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    let rt = select(r.x, r.y, p.x > 0.0);
    let rb = select(r.w, r.z, p.x > 0.0);
    let rr = select(rt, rb, p.y > 0.0);
    let q = abs(p) - b + vec2<f32>(rr, rr);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - rr;
}
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let half = max(in.p0.xy, vec2<f32>(0.0));
    let feather = in.p0.w;
    let radii = clamp(in.radii, vec4<f32>(0.0), vec4<f32>(min(half.x, half.y)));
    let p = in.local; let d = sd_round_box(p, half, radii);
    let aa = max(feather, fwidth(d));
    let fill_cov = clamp(0.5 - d / max(aa, 1e-4), 0.0, 1.0);
    let a = in.axis.xy; let b = in.axis.zw;
    let ab = b - a; let denom = max(dot(ab, ab), 1e-6);
    let t = clamp(dot(in.local - a, ab) / denom, 0.0, 1.0);
    let col = mix(in.stop0, in.stop1, t);
    return vec4<f32>(col.rgb, col.a * fill_cov);
}
