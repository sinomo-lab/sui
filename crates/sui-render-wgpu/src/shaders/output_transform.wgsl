
struct VsOut {
    @builtin(position) position: vec4<f32>,
}

struct OutputUniform {
    tone_mapping_mode: u32,
    encode_srgb: u32,
    output_primaries: u32,
    _padding2: u32,
    sdr_content_scale: f32,
    _padding3: u32,
    _padding4: u32,
    _padding5: u32,
}

@group(0) @binding(0)
var scene_texture: texture_2d<f32>;

@group(0) @binding(1)
var<uniform> output_uniform: OutputUniform;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

fn tone_map(color: vec3<f32>) -> vec3<f32> {
    let scaled = max(color, vec3<f32>(0.0)) * output_uniform.sdr_content_scale;
    switch output_uniform.tone_mapping_mode {
        case 0u: {
            return scaled;
        }
        case 2u: {
            return scaled / (vec3<f32>(1.0) + scaled);
        }
        default: {
            return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0));
        }
    }
}

fn linear_srgb_to_output_primaries(color: vec3<f32>) -> vec3<f32> {
    if output_uniform.output_primaries == 1u {
        return vec3<f32>(
            0.82246196 * color.r + 0.17753802 * color.g,
            0.0331942 * color.r + 0.96680576 * color.g,
            0.01708263 * color.r + 0.07239743 * color.g + 0.91051996 * color.b,
        );
    }
    return color;
}

fn linear_to_srgb_channel(channel: f32) -> f32 {
    let value = max(channel, 0.0);
    if value <= 0.0031308 {
        return value * 12.92;
    }
    return (1.055 * pow(value, 1.0 / 2.4)) - 0.055;
}

fn encode_for_output(color: vec3<f32>) -> vec3<f32> {
    if output_uniform.encode_srgb == 0u {
        return color;
    }
    return vec3<f32>(
        linear_to_srgb_channel(color.r),
        linear_to_srgb_channel(color.g),
        linear_to_srgb_channel(color.b),
    );
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = textureDimensions(scene_texture);
    let max_coord = vec2<i32>(max(vec2<u32>(1u), dims) - vec2<u32>(1u));
    let coords = clamp(vec2<i32>(position.xy), vec2<i32>(0), max_coord);
    let color = textureLoad(scene_texture, coords, 0);
    return vec4<f32>(
        encode_for_output(linear_srgb_to_output_primaries(tone_map(color.rgb))),
        clamp(color.a, 0.0, 1.0),
    );
}
