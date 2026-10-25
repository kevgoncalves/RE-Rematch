// Goal net: a diamond mesh of thin strings over a faint tinted fill, box-filtered so it fades to
// a soft haze in the distance instead of shimmering. UVs are in metres along the panel.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}

struct Net {
    // x: cell size (m), y: string width (m), z: string opacity, w: fill opacity
    cell: vec4<f32>,
}

@group(2) @binding(100) var<uniform> net: Net;

// Fraction of the footprint [x - w/2, x + w/2] on strings `duty` wide centred on the integers.
fn strings(x: f32, w: f32, duty: f32) -> f32 {
    let fw = max(w, 1e-4);
    let a = x - 0.5 * fw + 0.5 * duty;
    let b = x + 0.5 * fw + 0.5 * duty;
    let fa = floor(a) * duty + min(fract(a), duty);
    let fb = floor(b) * duty + min(fract(b), duty);
    return clamp((fb - fa) / fw, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // Diamond cells: the strings run at 45 degrees across the panel.
    let uv = vec2(in.uv.x + in.uv.y, in.uv.x - in.uv.y) * 0.70710678 / net.cell.x;
    let w = fwidth(uv) * 1.3;
    let duty = net.cell.y / net.cell.x;
    let a = strings(uv.x, w.x, duty);
    let b = strings(uv.y, w.y, duty);
    let cover = 1.0 - (1.0 - a) * (1.0 - b);
    let alpha = net.cell.w + (net.cell.z - net.cell.w) * cover;
    // The knots and strings are paler than the tint of the fill.
    let colour = mix(pbr_input.material.base_color.rgb, vec3(0.95), cover * 0.6);
    pbr_input.material.base_color = vec4(colour, alpha * pbr_input.material.base_color.a);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
