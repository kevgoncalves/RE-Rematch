// The animated character's cel shading (character.rs): the sun's light in three soft bands (the
// sun's shadow is the darkest), the sky's ambient flat on top, and a rim of light around the
// silhouette, brighter on the sun's side. Flat colours come in from the StandardMaterial it extends.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::{view, lights},
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    shadows::fetch_directional_shadow,
}

// x: the shade band's share of the sun, y: the middle band's, z: where the lit band starts (N.L),
// w: how soft the steps are.
@group(2) @binding(100) var<uniform> bands: vec4<f32>;
// rgb: the rim's colour (times the sun), w: how far in from the silhouette it reaches.
@group(2) @binding(101) var<uniform> rim_light: vec4<f32>;

const PI: f32 = 3.141592653589793;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    let base = pbr_input.material.base_color.rgb;
    let n = normalize(pbr_input.N);
    let v = normalize(pbr_input.V);

    var sun = vec3(0.0);
    var band = 1.0;
    var toward_sun = 0.5;
    if lights.n_directional_lights > 0u {
        let light = lights.directional_lights[0];
        let ndl = dot(n, light.direction_to_light);
        var shadow = 1.0;
        if (pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u && (light.flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            let view_z = dot(vec4(view.view_from_world[0].z, view.view_from_world[1].z, view.view_from_world[2].z, view.view_from_world[3].z), in.world_position);
            shadow = fetch_directional_shadow(0u, in.world_position, in.world_normal, view_z);
        }
        // In the sun's shadow it's as if facing away.
        let d = min(ndl, shadow * 2.0 - 1.0);
        let s = bands.w;
        band = bands.x + (bands.y - bands.x) * smoothstep(-s, s, d) + (1.0 - bands.y) * smoothstep(bands.z - s, bands.z + s, d);
        sun = light.color.rgb / PI;
        toward_sun = saturate(ndl * 0.5 + 0.5);
    }
    let rim = smoothstep(1.0 - rim_light.w, 1.0 - rim_light.w * 0.4, 1.0 - saturate(dot(n, v)));
    let colour = base * (sun * band + lights.ambient_color.rgb) + rim_light.rgb * sun * rim * (0.25 + 0.75 * toward_sun);

    var out: FragmentOutput;
    out.color = vec4(colour * view.exposure, pbr_input.material.base_color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
