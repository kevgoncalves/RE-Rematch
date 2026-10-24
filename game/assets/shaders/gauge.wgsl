// Slanted gauge (stamina, extra effort). Sizes are Rematch's HUD values at 1080p
// (see notes/hud.md); the shader is ours.
#import bevy_ui::ui_vertex_output::UiVertexOutput

// x: fill 0..1, z: exhaust marker position 0..1 (negative hides it).
@group(1) @binding(0) var<uniform> gauge: vec4<f32>;
// Gauge colour at its left and right ends.
@group(1) @binding(1) var<uniform> color_left: vec4<f32>;
@group(1) @binding(2) var<uniform> color_right: vec4<f32>;

const HEIGHT_1080: f32 = 20.0;
const SHEAR: f32 = 0.46630766;            // tan(25 degrees): the sides lean like "/"
const INSET: vec2<f32> = vec2(3.3, 2.0);  // gauge inside its dark background
const BACKGROUND: vec4<f32> = vec4(0.028, 0.032, 0.036, 0.8); // #2F3235
const MARKER: vec4<f32> = vec4(1.0, 1.0, 1.0, 0.75);

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let px = in.size.y / HEIGHT_1080;
    let p = in.uv * in.size;
    // Distance along the slant from the left edge, and the slanted width.
    let x = p.x - (in.size.y - p.y) * SHEAR;
    let width = in.size.x - in.size.y * SHEAR;
    let inside_bg = clamp(min(x, width - x) + 0.5, 0.0, 1.0);

    let inset = INSET * px;
    let gx = x - inset.x;
    let gw = width - 2.0 * inset.x;
    let inside_gauge = clamp(min(min(gx, gw - gx), min(p.y - inset.y, in.size.y - inset.y - p.y)) + 0.5, 0.0, 1.0);
    let t = gx / gw;

    let filled = clamp((gauge.x * gw - gx) + 0.5, 0.0, 1.0);
    var color = mix(BACKGROUND, mix(color_left, color_right, clamp(t, 0.0, 1.0)), inside_gauge * filled);

    // Exhaust threshold: where stamina must climb back to.
    if gauge.z >= 0.0 {
        let marker = clamp(1.5 * px - abs(gx - gauge.z * gw) + 0.5, 0.0, 1.0);
        color = mix(color, MARKER, marker * MARKER.a);
    }
    return vec4(color.rgb, color.a * inside_bg);
}
