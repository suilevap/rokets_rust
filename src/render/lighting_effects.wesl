import bevy_pbr::render::{forward_io::{Vertex, decompress_vertex}, mesh_view_bindings::view};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var light_map: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var light_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var dirty_glass: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var dirt_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var dark_corners: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var corners_sampler: sampler;

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(input: Vertex) -> Output {
    let v = decompress_vertex(input, input.instance_index);
    var out: Output;
    out.position = vec4(v.position.xy, 0.5, 1.0);
    out.uv = v.uv;
    return out;
}

@fragment
fn fragment(in: Output) -> @location(0) vec4<f32> {
    // Unproject the centre ray through this pixel onto the XY ground plane.
    // The current view handles speed zoom, camera tilt, resize and portrait.
    let near_h = view.world_from_clip * vec4(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, 1.0, 1.0);
    let ray = near_h.xyz / near_h.w - view.world_position;
    let denominator = select(1.0, ray.z, abs(ray.z) > 0.000001);
    let t = -view.world_position.z / denominator;
    let ground = view.world_position + ray * t;
    let light_uv = vec2(ground.x, -ground.y) * params.xy;
    let inside = all(light_uv >= vec2(0.0)) && all(light_uv <= vec2(1.0)) && t > 0.0;
    let light = textureSampleLevel(light_map, light_sampler, clamp(light_uv, vec2(0.0), vec2(1.0)), 0.0).rgb
        * select(0.0, 1.0, inside);

    // Cover the viewport without stretching the glass's circular marks.
    let size = vec2<f32>(textureDimensions(dirty_glass));
    let aspect = view.viewport.z / view.viewport.w;
    let glass_aspect = size.x / size.y;
    let crop = vec2(min(1.0, aspect / glass_aspect), min(1.0, glass_aspect / aspect));
    let dirt = textureSample(dirty_glass, dirt_sampler, vec2(0.5) + (in.uv - vec2(0.5)) * crop).rgb;
    // Original DualTexture Modulate2X, then additive blending with Alpha=.6:
    // the amount occurs in both the premultiplied colour and source alpha.
    let lens = light * dirt * (2.0 * params.z * params.z);
    let border = vec2(1.0) / vec2<f32>(textureDimensions(dark_corners));
    let corners = textureSample(dark_corners, corners_sampler, border + in.uv * (vec2(1.0) - 2.0 * border)).a;
    let vignette = 1.0 - corners * params.w;
    return vec4(lens * vignette, vignette);
}
