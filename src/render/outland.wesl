// The outland's colours times the light layer where the line of sight
// crosses the ground (see outland.rs).

import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::view};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> area: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var light_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var light_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = in.color.rgb;
    let p = in.world_position.xyz;
    let eye = view.world_position;
    // Above the ground the layer is behind the outland: unlit.
    if (p.z < 0.0 && eye.z > 0.0) {
        let ground = eye.xy + (p.xy - eye.xy) * (eye.z / (eye.z - p.z));
        let uv = vec2<f32>((ground.x - area.x) / area.z, (area.y - ground.y) / area.w);
        if (all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0))) {
            color *= textureSampleLevel(light_texture, light_sampler, uv, 0.0).rgb;
        }
    }
    return vec4<f32>(color, 1.0);
}
