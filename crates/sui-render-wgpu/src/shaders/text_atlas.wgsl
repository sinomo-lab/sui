
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) glyph_coords: vec2<f32>,
    @location(2) @interpolate(flat) metadata: vec4<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) uv_min: vec2<f32>,
    @location(5) @interpolate(flat) uv_max: vec2<f32>,
    @location(6) @interpolate(flat) atlas_kind: u32,
};

@group(0) @binding(0)
var text_atlas_sampler: sampler;

// Single-channel coverage for grayscale glyphs.
@group(0) @binding(1)
var text_atlas_mask: texture_2d_array<f32>;

// RGBA for color glyphs and LCD subpixel masks.
@group(0) @binding(2)
var text_atlas_color: texture_2d_array<f32>;

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
    out.glyph_coords = local_pos;
    out.metadata = vec4<f32>(
        f32(coverage_flags.x),
        f32(coverage_flags.y),
        f32(coverage_flags.z),
        coverage_parameter,
    );
    out.layer = layer;
    out.uv_min = uv_min;
    out.uv_max = uv_max;
    out.atlas_kind = coverage_flags.w;
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
    if policy < 4.5 {
        // Skia-style perceptual gamma/contrast, expressed as coverage for a
        // linear-light blend target. Two UNORM12 luminances share one exact
        // f32 integer; flat interpolation preserves the packed value.
        if c <= 0.0 || c >= 1.0 { return c; }
        let foreground = floor(parameter / 4096.0) / 4095.0;
        let background = (parameter % 4096.0) / 4095.0;
        let gamma = TEXT_PERCEPTUAL_GAMMA;
        let a = c + c * (1.0 - c) * TEXT_PERCEPTUAL_CONTRAST * pow(background, gamma);
        let endpoints = srgb_to_linear(vec3<f32>(foreground, background, 0.0));
        if abs(endpoints.x - endpoints.y) < 1e-4 { return a; }
        let value = pow(pow(foreground, gamma) * a + pow(background, gamma) * (1.0 - a), 1.0 / gamma);
        let linear = srgb_to_linear(vec3<f32>(value)).x;
        return clamp((linear - endpoints.y) / (endpoints.x - endpoints.y), 0.0, 1.0);
    }
    return c;
}

// Atlas bounds are integer texels. Recover them from packed UNORM16 values
// before interpolation, so packing error cannot blur pixel-aligned text.
// Keep bilinear filtering for transforms, but exclude neighbouring glyphs.
fn glyph_atlas_uv(in: VsOut, atlas_size: vec2<f32>) -> vec2<f32> {
    let texel_min = round(in.uv_min * atlas_size);
    let texel_max = round(in.uv_max * atlas_size);
    let texel = clamp(mix(texel_min, texel_max, in.glyph_coords),
        texel_min + vec2<f32>(0.5), texel_max - vec2<f32>(0.5));
    return texel / atlas_size;
}

// Mask glyphs return their coverage in every channel; color and LCD glyphs
// are RGBA. An explicit level (the atlases have no mips) keeps sampling legal
// inside this non-uniform branch.
fn sample_glyph(in: VsOut) -> vec4<f32> {
    if in.atlas_kind == 1u {
        let size = vec2<f32>(textureDimensions(text_atlas_color));
        return textureSampleLevel(text_atlas_color, text_atlas_sampler,
            glyph_atlas_uv(in, size), i32(in.layer), 0.0);
    }
    let size = vec2<f32>(textureDimensions(text_atlas_mask));
    let coverage = textureSampleLevel(text_atlas_mask, text_atlas_sampler,
        glyph_atlas_uv(in, size), i32(in.layer), 0.0).r;
    return vec4<f32>(coverage);
}

fn fs_shade(in: VsOut) -> vec4<f32> {
    let sampled = sample_glyph(in);
    if in.color.a < 0.0 {
        let opacity = -in.color.a;
        let alpha = sampled.a * opacity;
        // Color/bitmap emoji glyphs carry their own RGB. Linearize the stored sRGB and premultiply.
        return vec4<f32>(srgb_to_linear(sampled.rgb) * alpha, alpha);
    }

    // LCD is disabled before atlas preparation without dual-source blending.
    // Fail closed to grayscale if a subpixel atlas instance still arrives;
    // one alpha cannot attenuate destination RGB independently.
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return fit_premultiplied(fs_shade(in));
}
