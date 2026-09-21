
enable dual_source_blending;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) glyph_coords: vec2<f32>,
    @location(2) @interpolate(flat) metadata: vec4<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) uv_min: vec2<f32>,
    @location(5) @interpolate(flat) uv_max: vec2<f32>,
};

struct FragmentOutput {
    @location(0) @blend_src(0) foreground: vec4<f32>,
    @location(0) @blend_src(1) alpha: vec4<f32>,
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
        let gamma = 1.8;
        let a = c + c * (1.0 - c) * 0.5 * pow(background, gamma);
        let endpoints = srgb_to_linear(vec3<f32>(foreground, background, 0.0));
        if abs(endpoints.x - endpoints.y) < 1e-4 { return a; }
        let value = pow(pow(foreground, gamma) * a + pow(background, gamma) * (1.0 - a), 1.0 / gamma);
        let linear = srgb_to_linear(vec3<f32>(value)).x;
        return clamp((linear - endpoints.y) / (endpoints.x - endpoints.y), 0.0, 1.0);
    }
    return c;
}

fn dual_source(foreground: vec3<f32>, alpha: vec3<f32>) -> FragmentOutput {
    var out: FragmentOutput;
    let source_alpha = max(max(alpha.r, alpha.g), alpha.b);
    out.foreground = vec4<f32>(foreground, source_alpha);
    out.alpha = vec4<f32>(alpha, 1.0);
    return out;
}

fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let c = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(
        1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055,
        c * 12.92,
        c <= vec3<f32>(0.0031308));
}

fn lcd_perceptual_coverage(
    raw: vec3<f32>,
    foreground: vec3<f32>,
    packed_background: f32
) -> vec3<f32> {
    let c = clamp(raw, vec3<f32>(0.0), vec3<f32>(1.0));
    let text = linear_to_srgb(foreground);
    let packed = u32(packed_background);
    let background = vec3<f32>(
        f32((packed >> 16u) & 255u),
        f32((packed >> 8u) & 255u),
        f32(packed & 255u)) / 255.0;
    let guessed_bg = vec3<f32>(1.0) - text;
    let guessed_linear = srgb_to_linear(guessed_bg);
    let a = c + c * (vec3<f32>(1.0)-c) * guessed_linear;
    let delta = text-guessed_bg;
    let near = abs(delta) < vec3<f32>(1.0/256.0);
    let divisor = select(delta,vec3<f32>(1.0),near);
    let encoded_alpha = clamp(
        select(
            (linear_to_srgb(foreground * a + guessed_linear * (vec3<f32>(1.0) - a)) - guessed_bg) / divisor,
            a, near),
        vec3<f32>(0.0), vec3<f32>(1.0));
    let bg_linear = srgb_to_linear(background);
    let blend_delta = foreground-bg_linear;
    let same = abs(blend_delta) < vec3<f32>(1e-4);
    let desired = srgb_to_linear(background+(text-background)*encoded_alpha);
    var coverage = clamp(
        select(
            (desired - bg_linear) / select(blend_delta, vec3<f32>(1.0), same),
            c, same),
        vec3<f32>(0.0), vec3<f32>(1.0));
    coverage = select(coverage,vec3<f32>(0.0),c <= vec3<f32>(0.0));
    return select(coverage,vec3<f32>(1.0),c >= vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> FragmentOutput {
    // Atlas bounds are integer texels. Recover them from packed UNORM16 values
    // before interpolation, so packing error cannot blur pixel-aligned text.
    let atlas_size = vec2<f32>(textureDimensions(text_atlas_texture));
    let texel_min = round(in.uv_min * atlas_size);
    let texel_max = round(in.uv_max * atlas_size);
    // Keep bilinear filtering for transforms, but exclude neighbouring glyphs.
    let texel = clamp(mix(texel_min, texel_max, in.glyph_coords),
        texel_min + vec2<f32>(0.5), texel_max - vec2<f32>(0.5));
    let sampled = textureSample(text_atlas_texture, text_atlas_sampler, texel / atlas_size, i32(in.layer));
    if in.color.a < 0.0 {
        let opacity = -in.color.a;
        // Color/bitmap emoji glyphs carry their own RGB. Linearize the stored sRGB before
        // premultiplying.
        return dual_source(srgb_to_linear(sampled.rgb), vec3<f32>(sampled.a * opacity));
    }

    if in.metadata.x > 0.5 {
        if in.metadata.z > 4.5 {
            return dual_source(in.color.rgb, lcd_perceptual_coverage(sampled.rgb,in.color.rgb,in.metadata.w)*in.color.a);
        }
        let coverage = vec3<f32>(
            apply_text_coverage(sampled.r, in.metadata.z, in.metadata.w),
            apply_text_coverage(sampled.g, in.metadata.z, in.metadata.w),
            apply_text_coverage(sampled.b, in.metadata.z, in.metadata.w),
        );
        return dual_source(in.color.rgb, coverage * in.color.a);
    }

    if in.metadata.y > 0.5 {
        let coverage = apply_text_coverage(
            (sampled.r + sampled.g + sampled.b) / 3.0,
            in.metadata.z,
            in.metadata.w,
        );
        return dual_source(in.color.rgb, vec3<f32>(coverage * in.color.a));
    }

    let coverage = apply_text_coverage(sampled.a, in.metadata.z, in.metadata.w);
    return dual_source(in.color.rgb, vec3<f32>(coverage * in.color.a));
}
