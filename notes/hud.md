# Rematch HUD

What the game's own HUD widgets and materials tell us. Values are the widget and material defaults; anything we had to interpret is marked.

## Aim reticle (crosshair)

The reticle is one 140 × 140 px image (at 1080p) in the middle of the screen. A single material draws all of it from parameters, which the widget animates.

### Shape
| Part | What it is | Default |
|---|---|---|
| **Arcs ("parentheses")** | two arcs, left and right, like `( )` | opening **0.225** at rest |
| Arc size | radius of the arcs | 1.0 in the widget (1.2 in the bare material) |
| Size multiplier | overall scale; the crosshair size option moves it between 0.5 and 1.0 | 0.6 |
| Line thickness | | 0.03 |
| **Dark outline** | soft black shadow around every shape | thickness 0.045, blur 1.5, **50% opacity** |
| **Magnus point** | a dot that follows your curve input | size 0.1 |
| Notch | the point cuts a gap out of the ring where they overlap | 0.104, smoothness 0.06 |
| Point reach | at full curve the point sits slightly inside the ring | −0.075 |
| **Force fill** | fills the ring with the shot's force | never less than **3%** |
| Colour | white by default; the colour option offers white, dark grey, cyan, green, pink and red | |

The opening is how far the arcs reach: at 1.0 they meet into a full circle. How the material turns 0.225 into an arc length is not in the files (we read it as each arc reaching 22.5% of the way from the horizontal to the vertical, unconfirmed).

### How it moves
| When | Over | What changes |
|---|---|---|
| **Start charging a shot** | **0.2 s** | arcs close into a circle (opening 0.225 → 1), the circle fades to **40% opacity**, the Magnus point appears; the force fill appears in 0.083 s |
| **Shot released or cancelled** | **0.3 s** | everything goes back; the force fill fades in 0.2 s |
| **Enter lock move (defence stance)** | ~0.21 s | arcs close into a circle at full opacity, no Magnus point |
| Leave lock move | ~0.21 s | arcs open again |

While a shot charges, the force fill rises from the bottom of the faint circle in solid white, so the ring turns opaque from the bottom up as the shot gets stronger.

When the ball is shot the widget also plays a short wobble (a curve that swings +0.85, −0.375, +0.25 and settles); what it moves is not in the files.

## Stamina and extra effort bars

Both sit side by side at the **bottom centre** of the screen, their bottom edge **65 px** above the screen's bottom (1080p).

| | Stamina | Extra effort |
|---|---|---|
| Size | **280 × 20 px** (gauge 273 × 16 inside a dark frame) | **80 × 20 px** (gauge inset 3.3 × 2 px) |
| Order | left | right, **4 px** after the stamina bar |
| Shape | slanted parallelogram, sides at **25°** | same |
| Segments | none: one continuous bar | |
| Colours | green: `65E474`, `52EDCE`, `49B873` | cyan: `E6F8FF`, `00E7FF`, `6EEDFF` |
| Other states | **danger red** `B51313`; a red glow pulses when stamina runs out (opacity 0.2–0.8) | **grey while recovering**: `63737F`, `90A1AF` |
| Background | dark grey `2F3235` | same |

- **Exhaust threshold:** when you run out of stamina, a thin marker (4 × 26 px, 75% opacity) appears on the bar. We read it as the level stamina has to climb back to before you leave exhaustion (unconfirmed).
- The goalkeeper gets a gold glow on the bar (`CAA92B`, `FFFFFF`).
- The extra effort bar has three small triangles next to it that light up on a "boosted" refill.
