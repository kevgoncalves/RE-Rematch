# Rematch gameplay VFX, catalogued from Kev's recordings

Date: 2026-10-25. Written in our own words from frames of Kev's practice recordings. No game assets were copied. `NS_*` names come from the game's file list (names only) and are matched to what is on screen by look and folder. That match is our reading, not proven.

The "Prototype has?" column describes the prototype **before** branch `game/fx-effects`. What that branch changed is at the end.

## Sources and limits

- **Practice recordings used:**
  - V1 = `2026-10-25 02-33-58.mkv`;
  - V2 = `2026-10-25 02-55-47.mkv` (full screen 1080p, the sharpest).
- **Match clips:** the ones that were in `Downloads/` (1023.mp4, xd.mp4, …) were gone by the time of this pass. Only an earlier strip from `xd.mp4` was used.
- **Key semantics seen in the overlay:**
  - MMB in practice is **Ball recall**: the HUD labels it, and the player raises an arm.
  - Space is a **jump** (no effect).
  - There are no M5 (tackle) presses, so tackles are not covered.
  - V1 never shows Ctrl. Its M4 touches still produce the dribble look, so in V1 we read M4 as "a touch on the ball".
- **Timings** are in ms from the key press (±33 ms at 30 fps). Effects with no press (control, diamond) are timed from their first visible frame.
- **Colours** are sampled screen sRGB after the game's tonemapping and bloom. They are "what it looks like", not material values.
- **Sizes** are relative to the player (1.8 m) and the ball (~0.22 m).
- **Frame strips** (session scratchpad, `clip/`) are named under each effect.

## A finding that affects every colour: the team tint

`GC_HasBall` (the cue played on every possession) writes two global material colours:
- `FX | Current Team` = the **ally or enemy outline colour**, chosen by whether the new owner is on the local player's team;
- `FX | Current Team Trail` = **team 1 or team 2's colour**.

This explains what the frames show:
- In practice (always an ally) the control rings are **yellow**, not the orange that the raw curves hold.
- In practice the ball trail is near-white. In a match clip it is **cyan**.

So ring and trail colours should come from an ally/enemy colour pair. Yellow is the local/ally colour seen here; the enemy colour is not observed.

The recorded player also wears **cosmetic shoes** ("Sweeper"). Even at idle they add thin yellow/cyan rims on the trouser edges, tiny yellow and cyan sparkles at the feet, and cyan soles. These are cosmetics, not mechanics.

## Summary table

Priority: P1 = biggest feel or readability win, P3 = polish.

| # | Effect (likely game name) | Trigger and timing | Look | Prototype had? | Priority |
|---|---|---|---|---|---|
| 1 | **Shot impact** (`NS_Impact_Shoot_Low/Mid/High`) | Foot contact: ~430 ms after a tap press, ~600–700 ms after a 0.5 s hold, ~770–830 ms on overhead volleys. Lasts ~0.2–0.25 s | Spikes radiating from the ball. **Weak:** small cyan-white spikes. **Power:** a white-hot flare with long white spikes and a warm gold glow, then **gold triangle shards** falling around the foot | Different: expanding spheres plus yellow→magenta rings | **P1** |
| 2 | **Control** (`NS_BallControlOutline` + player overlay) | The touch that gains the ball (no press). ~0.4 s | Two **broken yellow rings** (#F1DF4B) closing from ~1.2 m onto the ball in ~130 ms. The ball shell goes yellow → white → **light blue**. **Teal ghost copies** of the ball behind it. A yellow rim flashes on the legs | Different: full annuli, orange→blue, no ghosts, no leg rim | **P1** |
| 3 | **Volley diamond** (`NS_VolleyPredict_TargetLock_02`) | While a loose ball is volleyable. Strong bloom for the first ~130 ms, settled by ~170 ms. Gone at the touch | **Four separate outward triangles** (N/E/S/W), pale cyan-white (#AAF0F6), ~0.5 m tip to tip, with a blue bloom (#80C0E8) on entry | Different: one plate with a round hole, saturated blue | **P1** |
| 4 | **Extra effort** (`NS_ExtraEffort_Activation/Loop/Footstep` + body overlay) | Double-tap Shift. HUD bar empties at ~150–200 ms. Burst at ~130–330 ms, visible ~70–100 ms. Then ~1–1.2 s of look | **Ground swirl burst** 1 → 2.5 m, white-cyan, with lightning shards. Then a lavender-cyan **rim on legs and shoes**, cyan shoes, **zigzag bolts** at calves and feet, **cyan footprint streaks**, faint speed lines | Partly: cyan emissive legs and foot ribbons | **P1** |
| 5 | **Push / set-up touch** (`NS_Impact_PushBall` + `NS_BallLost`) | Each push touch. When shooting from a controlled ball, the set-up touch comes ~100–230 ms after LMB (~200 ms after the control rings) | 1–2 frames of **white crystal spikes** (the "white sparkles"), then ~100–130 ms of a **blue glow smear** (#567BAB) and shell, plus teal ghosts | Different: a small sphere burst with one ring | P2 |
| 6 | **Dribble** (`NS_Dribble`) | Lock-move push-ball press: puff at +33–66 ms, shell and ghosts +100–233 ms, gone by ~266 ms | A soft **cyan puff** (~0.5 m), a **light-blue shell** (~1.2× ball, #82C5D8) with a bright rim, and **2–4 discrete teal ghost balls** (#5DAAA3) trailing | Close: blue shell, but 0.5 s long and with a ribbon instead of ghost balls; no puff | P2 |
| 7 | **Pass aim line** (`NS_Tap`, `NS_Tap_Lob`) + **pass release** (`NS_Impact_Tap`) | Pass press: the line appears within ~30 ms. Pale blue for ~100 ms, then green. Turns cyan and goes ~100 ms after the kick | A ground strip ≥5 m long: thin pale blue (#89C4D2), then ~0.25 m light lime (#ADED9E), with **3–5 angular translucent shards** floating near the ball. At release: a **blue ground flash** (~0.5 m) and blue sparks | Mostly: the strip and blue→green by charge; no shards, no release flash | P2 |
| 8 | **Landing marker** (`NS_Bounce`) | An airborne ball's landing spot, from ~0.4 s before landing until it lands | **Translucent green** (#63A372) ground ring ~0.8 m, with an angular **down-pointing arrow** (~0.5 m) above it | Different: blue/cyan ring and glow disc, no arrow | P2 |
| 9 | **Ball trail** (`NS_Ball_Trail`) | From a kick until anyone gains the ball | A soft beam ~1–1.5 ball widths wide, near-white (#D8E2EE) in practice, cyan in a match; the tail fades over ~0.3–0.4 s | Matching (near-white ribbon, 0.2 m, 0.45 s); no team tint | P3 |
| 10 | **Grass on shots** (`NS_Grass_Shoot`) | Shot contact, visible ~0.1–0.4 s after | A handful of small green slivers kicked up and forward from the contact point | Missing | P2 |
| 11 | **Fake shot** (shot cancelled with the pass button; maybe `NS_FakeShoot`) | When the foot passes the ball, ~250 ms after the pass press. Lasts ~250 ms | **Orange-red broken rings** (#D0553F) and a peach glow on the ball, fading to pink | Missing | P3 |
| 12 | **Control flash on the player** (`BP_CharacterOutlineOverlay`) | With #2, ~0.4 s | A bright yellow rim light on the legs and body, stronger than the cosmetic rim | Missing | P3 |
| 13 | **Sprint lines** (`NS_Sprint_Global/Local`) | Sprinting fast; clearly visible only with low stamina or in EE | Very thin, faint white lines ~1 m long streaming past the player | Missing | P3 |
| 14 | **Ball shadow** (`NS_Shadow_Ball`) | Always; matters for high balls | A dark soft round disc **straight under** the ball, visible however high it is | Partly: a real directional shadow, offset by the sun angle | P3 |
| — | Not observed in these clips | — | Wall/glass bounce, goal burst, tackle, dive, volley confirm (green) and refuse (red). Data only | — | — |

## Effects in detail (how to rebuild them in Bevy)

Common recipe: unlit `StandardMaterial` with `AlphaMode::Add`, `NotShadowCaster`, driven by a small "age / life" component. "Camera-facing" means re-orient towards the camera each frame. Since the materials are additive, fade by scaling the colour.

### 1. Shot impact (P1)
Strips: `v2_s225_b` (power), `v1_quick709z` (weak), `v2_shot_13773` and `v2_vol128` (aerial).
- **When:** at the frame the foot meets the ball, not at the button release. It stays where the contact happened.
- **Weak / quick shot:** 5–8 thin cyan-white spikes (long thin triangles, 0.15–0.3 m) for 2–3 frames, and a few cyan slivers flung out for ~0.15 s. No gold.
- **Power shot:**
  - a white core flare (camera-facing soft star, ~0.6 m, one to two frames at full brightness);
  - 6–10 long white spikes (0.3–0.5 m);
  - a warm gold glow (#DECE6C);
  - then 6–12 **flat gold triangles** (10–20 cm) thrown outward and down around the foot, tumbling and fading over ~100–150 ms.
- **Overhead volleys** look the same and are big (the flare reaches ~0.7 m).
- **Variants:** this fits the data's three steps (Low adds one burst and a 1-frame flare; Mid adds a second flare and a ring; High adds three bursts and longer flares). Pick the step from launch speed (29.4 / 34.8 m/s) and slide the colour **cyan → white → gold** with power.
- **Bevy:**
  - spikes = thin triangles pointing out from the ball, scaled 0 → 1 → 0 over 60–100 ms;
  - shards = small triangle meshes with velocity, gravity and spin, life 0.15–0.25 s;
  - the flare = a camera-facing quad with a radial falloff.
  - No expanding spheres: they read as a bubble, not a strike.

### 2. Control (P1)
Strips: `v2_ctl_1173z`, `v2_s225_a`, `v1_s192_ball`.
- **When:** the touch that gains the ball, with no press. It follows the ball.
- **Time line** (t0 = first ring):
  1. **0–130 ms:** two concentric rings around the ball, each made of **2–3 arc segments with gaps** (like "(( ))"), line ~4 cm. The outer one starts ~1.2 m across, the inner ~0.8 m. Both close in onto the ball with an ease-out, and the ball itself glows yellow-white. Colour yellow #F1DF4B (bright #F8E95E). They read as circles around the ball (camera-facing), sometimes slightly squashed.
  2. **~130 ms:** the rings reach ball size and wash to pale white.
  3. **130–230 ms:** a shell ~1.2× the ball turns light blue-lavender, then fades.
  4. **0–300 ms:** 2–4 **teal ghost copies of the ball** (#5DAAA3, ~60 % opacity) along the path the ball came in on. Each fades over ~0.15 s.
  5. **Player:** a yellow rim light flashes on the legs for ~0.4 s (see #12).
- **Bevy:**
  - each ring = an arc mesh (an annulus with 2–3 gaps), camera-facing, scale 1 → 0.15 over 0.13 s;
  - the shell = a ball sphere ×1.2 with a colour sequence (yellow → white → blue);
  - the ghosts = ball-sized spheres spawned every ~0.3 ball widths of travel, each fading where it was spawned;
  - colours from an ally/enemy pair.

### 3. Volley diamond (P1)
Strips: `v2_dia_1173z`, `v1_s192_goal`, `my_ee_688`.
- **When:** as soon as a loose ball is volleyable, also on balls far away. It follows the ball, camera-facing, and the control rings replace it at the touch.
- **Shape:** four **separate filled triangles**, up, down, left and right of the ball, pointing outward. Each is ~0.12 m with a thin white inner edge; tip to tip ~0.5 m (≈2.2 ball widths). **No connecting frame and no hole.**
- **Animation:** for the first ~130 ms a strong blue bloom (#80C0E8) with the ball washed white. By ~170 ms it settles to pale cyan-white triangles (#AAF0F6) with a thin blue glow, until the touch.
- **Bevy:** one mesh of four triangles plus a soft glow quad behind it; glow intensity 3× → 1× over 0.17 s.

### 4. Extra effort (P1)
Strips: `v1_ee604b`, `v1_ee604f`, `v1_ee604`, `v2_ee_11633z`, `my_ee_688`.
- **Activation:**
  - **0–100 ms:** shoes glow cyan;
  - **~150–200 ms:** the HUD bar empties;
  - **130–330 ms after the first tap:** the **ground burst**. A flat swirl disc centred on the player, white-cyan (#AADCEB), brightest in the middle, with concentric swirl lines. It grows from ~1 m to ~2.5 m in ~70 ms and is gone ~100 ms after it appears. With it come 6–10 cyan-green **lightning shards** (zigzag quads, 15–25 cm), flung outward along the ground.
- **While active** (~1–1.2 s in these clips):
  - a lavender-blue rim (#A1B0DB) on trousers and shoes (fresnel, lower body only);
  - small cyan zigzag bolts (10–15 cm) flickering at calves and feet (a new one every ~0.1 s, life ~0.1 s);
  - at each foot plant, a **cyan soft streak** on the ground (~0.5 × 0.1 m, along the run direction) that fades over ~0.2–0.3 s;
  - faint thin white speed lines (#13).
- **End:** the rim fades over ~0.1–0.2 s.
- **Bevy:**
  - burst = a ground disc with swirl arcs, scale 0.4 → 1, colour fading out;
  - bolts = zigzag strip meshes;
  - footprints = ground quads spawned at foot plants;
  - rim = an inflated back-face shell on the legs (additive), or a fresnel term in a leg material.

### 5. Push / set-up touch (P2)
Strips: `v2_s225_a` (100–266 ms), `v2_s269z` (183–283 ms), `v1_s192_ball` (233–366 ms), `v1_push168`.
- **When:** every push touch, and the touch that pushes the ball ahead when you start a shot from a controlled ball (~100–230 ms after the shot press). This is the "blue glow while a shot charges" and the "white sparkles on some touches".
- **Look:**
  - **1–2 frames** of 6–8 white crystal spikes (thin triangles, 0.2–0.5 m) from the ball;
  - then ~100–130 ms of a soft **blue smear** (#567BAB) ~1 m long behind the moving ball, with a blue shell and teal ghosts.
  - During the rest of the run-up the ball only has a faint warm rim.

### 6. Dribble (P2)
Strips: `v2_drib_265z`, `sw`.
- **+33–66 ms:** a soft cyan bloom puff at the ball, ~0.5 m.
- **+100–233 ms:** a light-blue shell ~1.2× the ball (#82C5D8) with a brighter rim, plus **2–4 discrete teal ghost balls** (#5DAAA3) spaced about half a ball apart along the path, each fading in ~0.15 s.
- **Gone by ~266 ms.**

### 7. Pass aim line and release (P2)
Strips: `v1_pline345`, `v1_in345`, `v2_pass_1569`.
- **Line:** starts thin (~6 cm) and pale blue (#89C4D2); after ~100 ms it widens to ~0.25 m and turns light lime (#ADED9E) with a bright core. It went green ~100 ms after appearing both for a 0.13 s tap and a 0.63 s hold, so the blue → green is at least partly a fixed intro, not only charge.
- **Shards:** 3–5 angular translucent shards (each ~ball-sized) float just above the line near the ball, cyan then green, drifting a little along it.
- **At the kick:** the line goes cyan, thins and disappears ~100 ms later. A **blue ground flash** (~0.5 m disc) and a few blue sparks at the foot for ~100 ms, then the ball trail.

### 8. Landing marker (P2)
Strips: `v2_ee_11633g`, `v1_s218_wall`, `v1_s192_goal`.
- **Look:** a translucent **green** ring (#63A372 over grass) ~0.8 m on the ground, with an angular arrow pointing down at its centre (~0.5 m tall, like a funnel with side fins), same green, soft glow.
- **When:** seen for recalled balls dropping in; visible from ~0.4 s before landing, gone when the ball lands.

### 9. Ball trail (P3)
Strips: `v2_s225_b`, `fl_v1a`, `xd_shot_tail`.
- **Look:** a soft additive beam ~1–1.5 ball widths wide, brightest at the ball, the tail fading over ~0.3–0.4 s, slightly noisy along its length.
- **Colour:** near-white (#D8E2EE) in practice, cyan in the match clip (the team tint).

### 10. Grass on shots (P2)
Strips: `v2_s225_b` (800–933 ms), `v1_quick709z` (493–560 ms).
- **Look:** 8–15 tiny green slivers (2–4 cm × 10 cm) thrown up and forward from the contact point, with gravity; gone in ~0.4–0.7 s.

### 11. Fake shot (P3)
Strips: `v2_feint254`, `v2_s269z` (516–650 ms).
- **When:** a shot is charging and the pass button is pressed (a shot cancel). The effect lands when the foot passes the ball, ~250 ms after the pass press.
- **Look:** two broken rings around the ball, like the control rings but **orange-red** (#D0553F), and a peach glow on the ball (#D59785), fading to pink over ~250 ms.
- **Name:** whether this is `NS_FakeShoot` (it exists under `VFX/04_Gameplay/FakeShoot`) or the control effect in its untinted orange is not settled.

### 12. Control flash on the player (P3)
Strips: `v2_s225_a` (−100 ms), `v2_ctl_1173z` (500–600 ms).
- **Look:** a bright yellow rim light on the trousers for ~0.4 s when the ball is gained, clearly brighter than the cosmetic rim.

### 13. Sprint lines (P3)
Strips: `v1_sprint465`, `v1_ee604f`.
- **Look:** very thin, faint white lines ~1 m long beside and behind the running player.
- **When:** seen with a low stamina bar and during EE; hardly visible at full stamina. This fits the data: more lines as stamina drops.

### 14. Ball shadow (P3)
Strips: `v1_dive1035`, `v2_ee_11633`.
- **Look:** a dark soft round disc straight under a high ball, even several metres up. It shows where a lob will come down.

## Ranked: what to add first

1. **Shot impact by power.** Spikes and a flare; gold shards for power shots, cyan spikes for weak ones. It plays on every shot, and power should be readable at a glance.
2. **Control in the ally colour.** Yellow broken rings closing in, then the yellow → white → blue shell, then teal ghost balls. It plays on every reception and is the clearest "I have it" signal.
3. **Volley diamond as four outward triangles**, with the blue bloom on entry.
4. **EE activation.** The ground swirl burst with lightning shards, then footprints and calf bolts, and a rim on the legs instead of a fill.
5. **Push / set-up touch.** White spikes for 1–2 frames, then a blue smear and ghosts.
6. **Dribble polish.** The cyan puff, a ~0.2 s shell, discrete ghost balls.
7. **Pass line polish.** The ~0.1 s blue intro, floating shards near the ball, the blue release flash.
8. **Landing marker.** Green, with the down arrow.
9. **Grass blades on shots** and the **ball's straight-down shadow**.
10. **The team-tint plumbing** (an ally/enemy colour pair feeding the rings and the trail), the control flash on the player, and sprint lines.
11. **When the mechanics exist:** the fake-shot rings, then wall bounce and goal burst (data only, not seen here).

## What the prototype has now (branch `game/fx-effects`)

All in `game/src/fx.rs` and `game/src/pass_line.rs`. One `Bit` piece (delay, life, scale, colour, fade, motion, facing) drives every short effect.

| # | Effect | Now |
|---|---|---|
| 1 | Shot impact | A spike star and flare at the contact point. Weak: cyan-white with cyan slivers. Mid/high (29.4 / 34.8 m/s): white, a second star, a gold glow, 6–10 gold shards falling and tumbling |
| 2 | Control | Two broken yellow rings close on the ball in 0.2 s, turning against each other. The shell goes yellow → white → light blue. Teal ghost balls are left where the ball came from. A yellow rim flashes on the shins and boots for 0.4 s |
| 3 | Volley diamond | Four separate pale-cyan arrows in a soft blue bloom. They flash white as it appears and settle by 0.17 s |
| 4 | Extra effort | A white-cyan swirl disc with three arms bursts on the ground, with lightning flung out. Bolts flicker at the feet and calves every 0.07 s, each foot plant leaves a cyan streak, the shins and boots get a lavender rim, and the boots glow cyan. The foot ribbons are shorter |
| 5 | Push ball | White spikes for a frame or two, then a blue shell and close blue ghosts (a smear) |
| 6 | Dribble | A cyan puff, a 0.2 s light-blue shell, and teal ghost balls spaced along the path (no ribbon) |
| 7 | Pass line | Thin pale blue for 0.1 s, green within 0.1 s more (brighter with charge), four shards drifting over it near the ball, and a 0.1 s cyan fade as the ball leaves. Release: a blue ground flash and blue sparks |
| 8 | Landing marker | A green ring and glow with a bobbing down arrow |
| 9 | Grass, ball shadow | Turf blades fly up and on from shot contacts (more with power). A soft dark disc lies straight under the ball at any height (the ball no longer casts a light shadow) |
| 11 | Fake shot | Orange-red broken rings and a peach shell on the ball as the leg passes over it, fading to pink in 0.25 s |

Not done:
- the team-tint plumbing (the colours are constants: the ally/practice look);
- sprint lines;
- the wall/glass bounce and the goal burst.
