
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) metadata: vec4<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) uv_min: vec2<f32>,
    @location(5) uv_max: vec2<f32>,
};

@group(0) @binding(0)
var text_atlas_sampler: sampler;

@group(0) @binding(1)
var text_atlas_texture: texture_2d_array<f32>;

@vertex
fn vs_main(
    @location(0) local_pos: vec2<f32>,
    @location(1) top_left: vec2<f32>,
    @location(2) x_axis: vec2<f32>,
    @location(3) y_axis: vec2<f32>,
    @location(4) uv_min: vec2<f32>,
    @location(5) uv_max: vec2<f32>,
    @location(6) color: vec4<f32>,
    @location(7) coverage_flags: vec4<u32>,
    @location(8) coverage_parameter: f32,
    @location(9) layer: u32,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(top_left + local_pos.x * x_axis + local_pos.y * y_axis, 0.0, 1.0);
    out.color = color;
    out.tex_coords = uv_min + local_pos * (uv_max - uv_min);
    out.metadata = vec4<f32>(
        f32(coverage_flags.x),
        f32(coverage_flags.y),
        f32(coverage_flags.z),
        coverage_parameter,
    );
    out.layer = layer;
    out.uv_min = uv_min;
    out.uv_max = uv_max;
    return out;
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((color + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}

fn apply_text_coverage(coverage: f32, policy: f32, parameter: f32) -> f32 {
    let c = clamp(coverage, 0.0, 1.0);
    if policy < 0.5 {
        return c;
    }
    if policy < 1.5 {
        return pow(c, max(parameter, 1e-4));
    }
    if policy < 2.5 {
        let amount = clamp(parameter, 0.0, 1.0);
        return c + (c * (1.0 - c) * amount);
    }
    if policy < 3.5 {
        return (2.0 * c) - (c * c);
    }
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Clamp the sample point to the glyph's half-texel-inset UV rect so bilinear taps at the quad
    // edges can't reach into neighbouring glyphs (or the padding) at non-integer scales.
    let atlas_half_texel = 0.5 / vec2<f32>(textureDimensions(text_atlas_texture));
    let clamped_uv = clamp(in.tex_coords, in.uv_min + atlas_half_texel, in.uv_max - atlas_half_texel);
    let sampled = textureSample(text_atlas_texture, text_atlas_sampler, clamped_uv, i32(in.layer));
    if in.color.a < 0.0 {
        let opacity = -in.color.a;
        let alpha = sampled.a * opacity;
        // Color/bitmap emoji glyphs carry their own RGB. Linearize the stored sRGB and premultiply.
        return vec4<f32>(srgb_to_linear(sampled.rgb) * alpha, alpha);
    }

    if in.metadata.x > 0.5 {
        let coverage = vec3<f32>(
            apply_text_coverage(sampled.r, in.metadata.z, in.metadata.w),
            apply_text_coverage(sampled.g, in.metadata.z, in.metadata.w),
            apply_text_coverage(sampled.b, in.metadata.z, in.metadata.w),
        );
        let max_coverage = max(max(coverage.r, coverage.g), coverage.b);
        let premul = in.color.rgb * coverage * in.color.a;
        return vec4<f32>(premul, in.color.a * max_coverage);
    }

    if in.metadata.y > 0.5 {
        let coverage = apply_text_coverage(
            (sampled.r + sampled.g + sampled.b) / 3.0,
            in.metadata.z,
            in.metadata.w,
        );
        let alpha = in.color.a * coverage;
        return vec4<f32>(in.color.rgb * alpha, alpha);
    }

    let coverage = apply_text_coverage(sampled.a, in.metadata.z, in.metadata.w);
    let alpha = in.color.a * coverage;
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
