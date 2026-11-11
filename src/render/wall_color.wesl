import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};
import bevy_render::color_operations::srgb_to_linear_rgb;

// The rasterizer has already interpolated XNA's sRGB vertex colours.
// Bevy's render target expects linear values; convert exactly once here.
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = vec4<f32>(srgb_to_linear_rgb(in.color.rgb), in.color.a);
    return out;
}
