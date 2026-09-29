
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
fn fs_shade(in: VsOut) -> vec4<f32> {
    let half = max(in.p0.xy, vec2<f32>(0.0));
    let feather = in.p0.w;
    let radii = clamp(in.radii, vec4<f32>(0.0), vec4<f32>(min(half.x, half.y)));
    let p = in.local; let d = sd_round_box(p, half, radii);
    let aa = max(feather, fwidth(d));
    let fill_cov = clamp(0.5 - d / max(aa, 1e-4), 0.0, 1.0);
    let a = in.axis.xy; let b = in.axis.zw;
    let ab = b - a; let denom = max(dot(ab, ab), 1e-6);
    let t = dot(in.local - a, ab) / denom;
    // p0.z = 0: the axis maps the two stops directly. Otherwise the quad is one band
    // of a multi-stop gradient along the full axis (see `gradient_band_code`): keep
    // fragments in the band's half-open range, then ramp between its offsets.
    let band = u32(round(in.p0.z));
    var u = clamp(t, 0.0, 1.0);
    if band != 0u {
        let code = band - 1u;
        let low = f32((code >> 11u) & 0x7FFu) / 2047.0;
        let high = f32(code & 0x7FFu) / 2047.0;
        let open_start = (code & 0x800000u) != 0u;
        let open_end = (code & 0x400000u) != 0u;
        if (!open_start && t < low) || (!open_end && t >= high) { discard; }
        u = clamp((t - low) / max(high - low, 1e-6), 0.0, 1.0);
    }
    let col = mix(in.stop0, in.stop1, u);
    return vec4<f32>(col.rgb, col.a * fill_cov);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return fit_straight(fs_shade(in));
}
