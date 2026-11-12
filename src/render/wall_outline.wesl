import bevy_pbr::render::{
    forward_io::{Vertex, decompress_vertex},
    mesh_functions::get_world_from_local,
    mesh_view_bindings::view,
};

@vertex
fn vertex(input: Vertex) -> @builtin(position) vec4<f32> {
    let v = decompress_vertex(input, input.instance_index);
    var world = get_world_from_local(input.instance_index) * vec4(v.position, 1.0);

    // UV.x is a signed half-width for uprights, zero for the flat contours.
    if abs(v.uv.x) > 0.0001 {
        let toward_camera = view.world_position.xy - world.xy;
        var facing = vec2<f32>(1.0, 0.0);
        if dot(toward_camera, toward_camera) > 0.000001 {
            facing = normalize(toward_camera);
        }
        let across = vec2(-facing.y, facing.x);
        // Same tiny separation as the contours (0.1 / PX_PER_UNIT).
        // This prevents coplanar fighting when looking along a wall face;
        // ordinary depth testing still hides the far corners behind walls.
        world = vec4(world.xy + across * v.uv.x + facing * 0.003125, world.zw);
    }
    return view.clip_from_world * world;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
