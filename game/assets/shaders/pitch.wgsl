// The pitch: stylised turf mown in rings round the centre spot, hand-painted colour noise, a
// slight sheen, and the painted markings drawn as distance fields (crisp at any distance, no
// z-fighting). Everything is in world metres; the sizes come from arena.rs.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}

// x, y: half width and half length; z: line half width; w: mowing ring width
@group(2) @binding(100) var<uniform> pitch_size: vec4<f32>;
// x, y: keeper area half width and depth; z: centre circle radius; w: centre spot radius
@group(2) @binding(101) var<uniform> pitch_keeper: vec4<f32>;
// x, y: goal area half width and depth; z: penalty spot distance; w: penalty arc radius
@group(2) @binding(102) var<uniform> pitch_boxes: vec4<f32>;
// Turf albedo of the light and dark stripes, and the paint's (linear).
@group(2) @binding(103) var<uniform> turf_light: vec4<f32>;
@group(2) @binding(104) var<uniform> turf_dark: vec4<f32>;
@group(2) @binding(105) var<uniform> paint_colour: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2(123.34, 456.21));
    let r = q + dot(q, q + 45.32);
    return fract(r.x * r.y);
}

// Smooth value noise in [0, 1].
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2(1.0, 0.0));
    let c = hash(i + vec2(0.0, 1.0));
    let d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    return noise(p) * 0.55 + noise(p * 2.07 + 17.3) * 0.3 + noise(p * 4.13 + 41.9) * 0.15;
}

// How much of a pixel's footprint [x - w/2, x + w/2] falls on bands of width `duty` (in periods)
// centred on the integers: an exact box filter, so thin bands fade to their average instead of
// shimmering.
fn bands(x: f32, w: f32, duty: f32) -> f32 {
    let fw = max(w, 1e-4);
    let a = x - 0.5 * fw + 0.5 * duty;
    let b = x + 0.5 * fw + 0.5 * duty;
    let fa = floor(a) * duty + min(fract(a), duty);
    let fb = floor(b) * duty + min(fract(b), duty);
    return clamp((fb - fa) / fw, 0.0, 1.0);
}

// Coverage of a painted line of half width `hw`, `d` metres from its centre (d >= 0), for a pixel
// `fw` metres across.
fn line(d: f32, hw: f32, fw: f32) -> f32 {
    let w = max(fw, 1e-4);
    return clamp((min(d + 0.5 * w, hw) - max(d - 0.5 * w, -hw)) / w, 0.0, 1.0);
}

// Distance to the outline of a box centred at `c` with half extents `e`.
fn box_outline(p: vec2<f32>, c: vec2<f32>, e: vec2<f32>) -> f32 {
    let q = abs(p - c) - e;
    return abs(length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.world_position.xz;
    let hw = pitch_size.z;

    // Pixel footprint in metres, and its screen derivatives (taken here, in uniform control flow).
    let dx = dpdx(p);
    let dy = dpdy(p);
    let foot = max(length(dx), length(dy));

    // Mowing: concentric rings round the centre spot, each `w` wide, with hand-drawn wobbly edges.
    let r = length(p);
    let w = pitch_size.w;
    let wobble = (fbm(p * 0.45) - 0.5) * 0.3 + (noise(p * 2.3) - 0.5) * 0.08;
    let t = r / w + wobble;
    let tw = (abs(dot(p, dx)) + abs(dot(p, dy))) / max(r, 1e-3) / w;
    let stripe = bands(t, tw, 0.5);

    var grass = mix(turf_dark.rgb, turf_light.rgb, stripe);
    // Hand-painted variation: broad patches drifting in hue, brush streaks along the rings, and
    // a fine grain that fades out before it can shimmer.
    let blotch = fbm(p * 0.045 + 3.1);
    grass *= mix(vec3(0.93, 0.97, 1.04), vec3(1.06, 1.03, 0.9), blotch);
    grass *= 0.94 + 0.12 * fbm(p * 0.22);
    let streak = noise(vec2(r * 2.0, (p.x - p.y) * 0.15)) - 0.5;
    let grain = noise(p * 9.0) - 0.5;
    let near = 1.0 - smoothstep(0.03, 0.25, foot);
    grass *= 1.0 + streak * 0.10 * (1.0 - smoothstep(0.2, 1.0, foot)) + grain * 0.10 * near;

    // Markings.
    let ax = abs(p.x);
    let az = abs(p.y);
    let fw = foot * 1.2;
    var d = abs(p.y); // halfway line
    d = min(d, abs(min(pitch_size.x - ax, pitch_size.y - az) - hw)); // touch and goal lines
    d = min(d, abs(r - pitch_keeper.z)); // centre circle
    let end = select(-1.0, 1.0, p.y > 0.0);
    let goal_line = end * pitch_size.y;
    d = min(d, box_outline(p, vec2(0.0, goal_line), pitch_keeper.xy)); // keeper area
    d = min(d, box_outline(p, vec2(0.0, goal_line), pitch_boxes.xy)); // goal area
    // The penalty arc: the part of the circle round the spot outside the keeper area.
    let spot = vec2(0.0, end * (pitch_size.y - pitch_boxes.z));
    let arc = abs(length(p - spot) - pitch_boxes.w);
    let outside = az < pitch_size.y - pitch_keeper.y - hw * 0.5;
    d = min(d, select(1e3, arc, outside));
    var paint = line(d, hw, fw);
    // Spots: centre and penalty.
    let spots = min(r, length(p - spot));
    paint = max(paint, line(spots, pitch_keeper.w, fw));

    // Paint sits on the grass, so a little of the turf shows through it.
    let painted = paint_colour.rgb * (0.92 + 0.08 * fbm(p * 1.7));
    let albedo = mix(grass, painted, paint);
    pbr_input.material.base_color = vec4(albedo, 1.0);
    // Sheen: the light stripes catch a little more light at grazing angles.
    pbr_input.material.perceptual_roughness = mix(0.82, 0.7, stripe) - paint * 0.1;
    pbr_input.material.reflectance = vec3(mix(0.22, 0.32, stripe));

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
