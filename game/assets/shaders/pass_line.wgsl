// Pass and lob aim line: a strip on the ground with bright edges and a soft fill that fades out
// along its length; lobs bend it up into an arc. Sizes follow Rematch's tap line (notes/hud.md);
// the shader is ours.
#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_world}
#import bevy_pbr::view_transformations::position_world_to_clip

@group(2) @binding(0) var<uniform> color: vec4<f32>;
// x: arc height (m, 0 for passes), y: fade power along the line (higher = shorter-looking).
@group(2) @binding(1) var<uniform> shape: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

const PI: f32 = 3.14159265;
const FILL: f32 = 0.35;
const EDGE: f32 = 0.95;

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var pos = v.position;
    pos.y += shape.x * sin(PI * v.uv.y);
    let world = mesh_position_local_to_world(get_world_from_local(v.instance_index), vec4(pos, 1.0));
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(world.xyz);
    out.uv = v.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let across = abs(in.uv.x - 0.5) * 2.0; // 0 in the middle, 1 at the edges
    let edge = smoothstep(0.6, 0.8, across) * (1.0 - smoothstep(0.9, 1.0, across));
    let fill = 1.0 - smoothstep(0.8, 1.0, across);
    // Fades in right at the ball and out toward the far end.
    let fade = smoothstep(0.0, 0.04, in.uv.y) * pow(1.0 - in.uv.y, shape.y);
    let alpha = clamp(FILL * fill + EDGE * edge, 0.0, 1.0) * fade * color.a;
    // Edges glow toward white.
    let rgb = mix(color.rgb, vec3(1.0), edge * 0.6);
    return vec4(rgb, alpha);
}
