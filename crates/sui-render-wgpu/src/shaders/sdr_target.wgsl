// How this pipeline fits colors to its target, as in `fit_to_sdr`: 0 for
// targets that keep extended range, 1 or 2 for SDR targets drawn directly.
override SDR_FIT: u32 = 0u;

fn fit_straight(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(fit_to_sdr(color.rgb, SDR_FIT), color.a);
}

fn fit_premultiplied(color: vec4<f32>) -> vec4<f32> {
    if color.a <= 0.0 {
        return color;
    }
    return vec4<f32>(fit_to_sdr(color.rgb / color.a, SDR_FIT) * color.a, color.a);
}
