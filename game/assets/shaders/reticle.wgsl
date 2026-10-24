// Aim reticle, drawn as signed distances so it stays sharp at any size.
// Shape values are the defaults of Rematch's crosshair material (see notes/hud.md); the shader is ours.
#import bevy_ui::ui_vertex_output::UiVertexOutput

// x: arc opening (0.225 = two short arcs, 1 = full circle), y: arc opacity,
// z: shot force 0..1 (fills the ring from the bottom), w: force fill opacity.
@group(1) @binding(0) var<uniform> shape: vec4<f32>;
// xy: curve (Magnus) input, -1..1, y up; z: Magnus point opacity.
@group(1) @binding(1) var<uniform> magnus: vec4<f32>;

const SCALE: f32 = 2.0;          // the node spans -SCALE..SCALE
const RADIUS: f32 = 0.6;         // arc size 1.0 x size multiplier 0.6
const THICKNESS: f32 = 0.03;     // half width of the line
const SHADOW: f32 = 0.045;       // half width of the dark outline
const SHADOW_BLUR_PX: f32 = 1.5;
const SHADOW_OPACITY: f32 = 0.5;
const DOT: f32 = 0.1;            // Magnus point radius
const DOT_CUT: f32 = 0.104;      // the point cuts a notch this big out of the ring
const DOT_CUT_SMOOTH: f32 = 0.06;
const DOT_REACH: f32 = -0.075;   // at full curve the point sits this far inside the ring
const HALF_PI: f32 = 1.5707964;

fn coverage(d: f32, unit_per_px: f32) -> f32 {
    return clamp(0.5 - d / unit_per_px, 0.0, 1.0);
}

// Smooth "a minus b" for signed distances.
fn smooth_subtract(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (a + b) / k, 0.0, 1.0);
    return mix(a, -b, h) + k * h * (1.0 - h);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let unit_per_px = SCALE * 2.0 / in.size.x;
    var q = (in.uv * 2.0 - 1.0) * SCALE;
    q.y = -q.y;

    // Two arcs centred on the horizontal axis; `opening` is how far they reach toward the
    // vertical (0 = nothing, 1 = they meet into a circle).
    let from_horizontal = atan2(abs(q.y), abs(q.x)) / HALF_PI;
    let ring = abs(length(q) - RADIUS);
    let arc_end = (from_horizontal - shape.x) * HALF_PI * RADIUS;
    var line = max(ring - THICKNESS, arc_end);
    var outline = max(ring - SHADOW, arc_end);

    // Magnus point, and the notch it cuts out of the ring once it shows.
    let dot_pos = clamp(length(magnus.xy), 0.0, 1.0) * normalize(magnus.xy + vec2(0.0, 1e-6)) * (RADIUS + DOT_REACH);
    let to_dot = length(q - dot_pos);
    let cut = to_dot - (DOT_CUT - (1.0 - magnus.z) * 0.5);
    line = smooth_subtract(line, cut, DOT_CUT_SMOOTH);
    outline = smooth_subtract(outline, cut, DOT_CUT_SMOOTH);
    let dot = to_dot - DOT;

    // The force fill is solid from the bottom of the ring up to the charge.
    let height = (q.y + RADIUS) / (2.0 * RADIUS);
    let filled = select(0.0, 1.0, height < shape.z);
    let line_opacity = mix(shape.y, 1.0, filled * shape.w);

    let white = max(coverage(line, unit_per_px) * line_opacity, coverage(dot, unit_per_px) * magnus.z);
    let blur = SHADOW_BLUR_PX * unit_per_px;
    // Each shape's outline fades with the shape, so a hidden Magnus point leaves no shadow.
    let line_dark = (1.0 - smoothstep(-blur, blur, outline)) * line_opacity;
    let dot_dark = (1.0 - smoothstep(-blur, blur, dot - (SHADOW - THICKNESS))) * magnus.z;
    let dark = SHADOW_OPACITY * max(line_dark, dot_dark);
    let alpha = white + dark * (1.0 - white);
    return vec4(vec3(white / max(alpha, 1e-4)), alpha);
}
