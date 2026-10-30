
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) clip: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) clip: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = translate_fragment(vec4<f32>(position, 0.0, 1.0));
    out.clip = translate_clip(clip);
    out.color = color;
    return out;
}

fn fs_shade(in: VsOut) -> vec4<f32> {
    return in.color;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if !clip_contains(in.position, in.clip) {
        discard;
    }
    return fit_straight(fs_shade(in));
}
