
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
};

@group(1) @binding(0)
var image_sampler: sampler;

@group(1) @binding(1)
var image_texture: texture_2d<f32>;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(4) clip: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = translate_fragment(vec4<f32>(position, 0.0, 1.0));
    out.clip = translate_clip(clip);
    out.color = color;
    out.tex_coords = tex_coords;
    return out;
}

fn fs_shade(in: VsOut) -> vec4<f32> {
    return textureSample(image_texture, image_sampler, in.tex_coords) * in.color;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if !clip_contains(in.position, in.clip) {
        discard;
    }
    return fit_straight(fs_shade(in));
}
