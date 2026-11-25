import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
};

// The standard unlit sprite, except that the alpha it adds with is
// squared (see `render/glow.rs`).
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var out: FragmentOutput;
    var color = pbr_input.material.base_color;
    color.a = color.a * color.a;
    out.color = main_pass_post_lighting_processing(pbr_input, color);
    return out;
}
