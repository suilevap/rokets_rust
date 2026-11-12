import bevy_pbr::render::{
    forward_io::{Vertex, decompress_vertex},
    mesh_functions::get_world_from_local,
    mesh_view_bindings::view,
};

struct LineMaterial {
    color: vec4<f32>,
    camera_offset: f32,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: LineMaterial;

@vertex
fn vertex(input: Vertex) -> @builtin(position) vec4<f32> {
    let v = decompress_vertex(input, input.instance_index);
    let world_from_local = get_world_from_local(input.instance_index);
    var world = world_from_local * vec4(v.position, 1.0);

    if v.uv.y > 0.5 {
        let axis = normalize((world_from_local * vec4(v.normal, 0.0)).xyz);
        let toward_camera = view.world_position - world.xyz;
        let perpendicular = toward_camera - axis * dot(axis, toward_camera);
        // A camera on the segment's axis has no unique facing direction.
        // Pick a stable perpendicular to avoid NaNs at that view's pole.
        var reference = vec3<f32>(1.0, 0.0, 0.0);
        if abs(axis.x) > 0.9 { reference = vec3<f32>(0.0, 1.0, 0.0); }
        var facing = normalize(reference - axis * dot(axis, reference));
        if dot(perpendicular, perpendicular) > 0.000001 {
            facing = normalize(perpendicular);
        }
        let across = cross(axis, facing);
        world = vec4(world.xyz + across * v.uv.x + facing * material.camera_offset, 1.0);
    }
    return view.clip_from_world * world;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return material.color;
}
