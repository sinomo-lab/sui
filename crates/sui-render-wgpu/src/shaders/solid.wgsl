
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

fn fs_shade(in: VsOut) -> vec4<f32> {
    return in.color;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return fit_straight(fs_shade(in));
}
