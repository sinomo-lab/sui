// Fit a linear color into the SDR range on its way to an SDR target.
//
// Mode 1 clips highlights while keeping their hue: the brightest channel lands
// at 1.0 and the others keep their ratio to it, so a bright orange stays
// orange instead of turning yellow. Mode 2 does the same, then turns the
// energy above SDR white toward white along a Reinhard curve, so brighter
// highlights still read brighter. Colors within 0..1 pass through unchanged
// in both; mode 0 leaves every color as it is.
fn fit_to_sdr(color: vec3<f32>, mode: u32) -> vec3<f32> {
    let peak = max(max(color.r, color.g), color.b);
    if mode == 0u || peak <= 1.0 {
        return color;
    }
    let hue = color / peak;
    if mode == 2u {
        return mix(hue, vec3<f32>(1.0), 1.0 - 1.0 / peak);
    }
    return hue;
}
