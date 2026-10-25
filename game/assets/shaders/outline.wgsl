// The animated character's outline (character.rs): its skinned meshes again, pushed out along their
// normals (further when the camera is further, so the line keeps its width on screen) and drawn
// from the back faces only, in one flat dark colour.

#import bevy_pbr::{
    mesh_functions,
    skinning,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

@group(2) @binding(0) var<uniform> colour: vec4<f32>;
// x: the width per metre from the camera, y and z: the distances it's clamped to.
@group(2) @binding(1) var<uniform> width: vec4<f32>;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
#ifdef SKINNED
    let world_from_local = skinning::skin_model(vertex.joint_indices, vertex.joint_weights, vertex.instance_index);
    let normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
    let at = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0)).xyz;
    let distance = clamp(length(view.world_position - at), width.y, width.z);
    let pushed = at + normalize(normal) * width.x * distance;
    out.world_position = vec4(pushed, 1.0);
    out.world_normal = normal;
    out.position = position_world_to_clip(pushed);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return colour;
}
