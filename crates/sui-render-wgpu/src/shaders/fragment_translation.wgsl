// Offset of the retained fragment being drawn, in normalized device
// coordinates. A layer that only moved is drawn from its existing vertex
// buffers with a new offset instead of being recomposed and uploaded.
struct FragmentTranslation {
    offset: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> fragment_translation: FragmentTranslation;

fn translate_fragment(position: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(position.xy + fragment_translation.offset.xy * position.w, position.zw);
}

