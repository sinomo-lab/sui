
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) metadata: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) params: vec4<f32>,
};

const KIND_COLOR_WHEEL: u32 = 0u;
const KIND_HUE_BAR: u32 = 1u;
const KIND_SATURATION_VALUE_PLANE: u32 = 2u;
const KIND_SATURATION_BAR: u32 = 3u;
const KIND_VALUE_BAR: u32 = 4u;
const KIND_ALPHA_BAR: u32 = 5u;
const KIND_RGB_CHANNEL_BAR: u32 = 6u;
const TAU: f32 = 6.283185307179586;
const COLOR_SPACE_LINEAR_SRGB: u32 = 1u;
const COLOR_SPACE_DISPLAY_P3: u32 = 2u;
const COLOR_SPACE_LINEAR_DISPLAY_P3: u32 = 3u;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) metadata: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) params: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.metadata = metadata;
    out.uv = uv;
    out.params = params;
    return out;
}

fn srgb_transfer_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        return channel / 12.92;
    }
    return pow((channel + 0.055) / 1.055, 2.4);
}

fn color_space_index(space: f32) -> u32 {
    return u32(space + 0.5);
}

fn to_linear_srgb(rgb: vec3<f32>, space: f32) -> vec3<f32> {
    let index = color_space_index(space);
    var linear = rgb;
    if index != COLOR_SPACE_LINEAR_SRGB && index != COLOR_SPACE_LINEAR_DISPLAY_P3 {
        linear = vec3<f32>(
            srgb_transfer_to_linear(rgb.r),
            srgb_transfer_to_linear(rgb.g),
            srgb_transfer_to_linear(rgb.b),
        );
    }
    if index == COLOR_SPACE_DISPLAY_P3 || index == COLOR_SPACE_LINEAR_DISPLAY_P3 {
        return vec3<f32>(
            1.2249402 * linear.r - 0.22494018 * linear.g,
            -0.042056955 * linear.r + 1.042057 * linear.g,
            -0.019637555 * linear.r - 0.07863604 * linear.g + 1.0982736 * linear.b,
        );
    }
    return linear;
}

fn hsv_to_rgb(hue_value: f32, saturation: f32, value: f32) -> vec3<f32> {
    let hue = fract(hue_value) * 6.0;
    let sector = floor(hue);
    let fraction = hue - sector;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - fraction * saturation);
    let t = value * (1.0 - (1.0 - fraction) * saturation);
    if sector < 1.0 {
        return vec3<f32>(value, t, p);
    }
    if sector < 2.0 {
        return vec3<f32>(q, value, p);
    }
    if sector < 3.0 {
        return vec3<f32>(p, value, t);
    }
    if sector < 4.0 {
        return vec3<f32>(p, q, value);
    }
    if sector < 5.0 {
        return vec3<f32>(t, p, value);
    }
    return vec3<f32>(value, p, q);
}

fn hsv_to_linear_color(space: f32, hue: f32, saturation: f32, value: f32, alpha: f32) -> vec4<f32> {
    return vec4<f32>(to_linear_srgb(hsv_to_rgb(hue, saturation, value), space), clamp(alpha, 0.0, 1.0));
}

fn hdr_slider_to_value(t: f32, max_value: f32) -> f32 {
    if max_value <= 1.0001 {
        return t;
    }
    if t <= 0.5 {
        return t * 2.0;
    }
    return pow(max(max_value, 1.0001), (t - 0.5) / 0.5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let kind = u32(in.metadata.x + 0.5);
    let space = in.metadata.y;
    let u = clamp(in.uv.x, 0.0, 1.0);
    let v = clamp(in.uv.y, 0.0, 1.0);

    if kind == KIND_COLOR_WHEEL {
        let centered = vec2<f32>(u * 2.0 - 1.0, v * 2.0 - 1.0);
        let distance = length(centered);
        let edge = 0.01;
        let alpha = smoothstep(0.55, 0.55 + edge, distance) * (1.0 - smoothstep(1.0 - edge, 1.0, distance));
        let hue = fract((atan2(centered.y, centered.x) / TAU) + 1.0);
        let color = hsv_to_linear_color(0.0, hue, 1.0, 1.0, alpha);
        return color;
    }

    if kind == KIND_HUE_BAR {
        return hsv_to_linear_color(0.0, u, 1.0, 1.0, 1.0);
    }

    if kind == KIND_SATURATION_VALUE_PLANE {
        let hue = in.metadata.z;
        let max_value = in.metadata.w;
        let value = hdr_slider_to_value(1.0 - v, max_value);
        return hsv_to_linear_color(space, hue, u, value, 1.0);
    }

    if kind == KIND_SATURATION_BAR {
        let hue = in.metadata.z;
        let value = in.metadata.w;
        return hsv_to_linear_color(space, hue, u, value, 1.0);
    }

    if kind == KIND_VALUE_BAR {
        let hue = in.metadata.z;
        let saturation = in.metadata.w;
        let value = hdr_slider_to_value(u, in.params.x);
        return hsv_to_linear_color(space, hue, saturation, value, 1.0);
    }

    if kind == KIND_ALPHA_BAR {
        let alpha = u;
        return vec4<f32>(to_linear_srgb(in.params.rgb, space), alpha);
    }

    if kind == KIND_RGB_CHANNEL_BAR {
        let channel = u32(in.metadata.z + 0.5);
        let max_value = in.metadata.w;
        var rgb = in.params.rgb;
        if channel == 0u {
            rgb.r = max_value * u;
        } else if channel == 1u {
            rgb.g = max_value * u;
        } else {
            rgb.b = max_value * u;
        }
        return vec4<f32>(to_linear_srgb(rgb, space), 1.0);
    }

    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
