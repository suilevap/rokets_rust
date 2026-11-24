import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::view};

struct DirtyLens {
    // The blob's colour times the gain.
    color: vec4<f32>,
    // The glass window: scale in xy, offset in zw.
    glass_window: vec4<f32>,
    // 1 when the view is turned a quarter.
    turned: f32,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> lens: DirtyLens;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var blob: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var blob_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var glass: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var glass_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Premultiplied blending: alpha 0 adds the colour to the picture.
    // The light map's blob (`flash2`): the sprite's alpha, once as its
    // premultiplied colour and once as the additive blend's source alpha.
    let a = textureSample(blob, blob_sampler, in.uv).a;
    // Where on the screen this pixel is, 0..1 from the top left.
    var screen = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    if lens.turned > 0.5 {
        screen = vec2<f32>(screen.y, 1.0 - screen.x);
    }
    let dirt = textureSample(glass, glass_sampler, screen * lens.glass_window.xy + lens.glass_window.zw).rgb;
    return vec4<f32>(lens.color.rgb * dirt * (a * a), 0.0);
}
