// Offset of the retained fragment being drawn: `xy` in normalized device
// coordinates, `zw` in framebuffer pixels. A layer that only moved is drawn from its existing vertex
// buffers with a new offset instead of being recomposed and uploaded.
struct FragmentTranslation {
    offset: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> fragment_translation: FragmentTranslation;

fn translate_fragment(position: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(position.xy + fragment_translation.offset.xy * position.w, position.zw);
}

// Rectangular clips travel with each vertex or instance as framebuffer-pixel
// edges `(min_x, min_y, max_x, max_y)`, snapped like scissor rectangles, so
// draws with different clips can share a draw call. They move with the
// fragment, like its geometry.
fn translate_clip(clip: vec4<f32>) -> vec4<f32> {
    return clip + fragment_translation.offset.zwzw;
}

// Whether a pixel center lies in `clip`: the same pixels a scissor keeps.
fn clip_contains(position: vec4<f32>, clip: vec4<f32>) -> bool {
    return all(position.xy >= clip.xy) && all(position.xy < clip.zw);
}
