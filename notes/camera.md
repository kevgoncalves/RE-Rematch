# Match camera (spec)

How the in-match player camera is placed, turned and smoothed, from the native camera code, the camera Blueprints and the camera data assets. The rules are in our words; the sources are at the end. How shots use the camera is in [shot-aim.md](shot-aim.md).

**Status:** reviewed by Kev and coded in the prototype (2026-10-24).

This note replaces three lines of the Camera section in [match-camera-network.md](match-camera-network.md):
- "framed 2 m above": the pivot is 1.35 m above the feet (§1). The 2 m value belongs to a framing feature that is off.
- "Pitch limits: 22° up, 15° down": it is the other way round, **22° down and 15° up** (§2).
- "Market is probably the settings option": it is a **cheat-only** preset (§12).

## Units and axes
- The game works in cm and degrees. These notes give metres; the source values are in the tables and in Constants.
- Unreal axes: X forward, Y right, Z up. An offset (x, y, z) in these notes is "x forward (negative = behind), y right, z up".
  - In Bevy (right = +X, up = +Y, forward = −Z): (x, y, z) → (y, z, −x).
- **Pitch is positive when looking up.** The data calls the downward limit "above" (the camera rises above the player) and the upward limit "below".

## How it differs from a simple rig
A rig with "pivot 2 m above the player, camera 5 m behind and 0.6 m right, looking at the pivot" is wrong in five ways:

1. **Pivot height:** the pivot is the capsule centre (0.90 m) plus a 0.45 m fulcrum, so **1.35 m** above the feet. At level pitch the camera is at 1.45 m.
2. **The arm turns with pitch.** The offset is applied in the camera's full yaw + pitch frame, like a boom. Looking down 22° lifts the camera to **3.55 m**; looking up 15° drops it to **0.31 m**.
3. **The arm length changes with pitch:** 5.5 m when looking fully down, 5 m level, 4 m looking fully up, on a smooth spline (§3).
4. **The camera does not look at the player.** It looks along its own yaw and pitch, parallel to the arm. So the player sits left of centre (about 43% across the screen), and the crosshair is **above** the centre, at 38% from the top. Shots aim through that crosshair.
5. **FOV 80 is horizontal at 16:9.** In Bevy terms (vertical FOV) that is **50.5°**. Using 80° as a vertical FOV gives about 112° horizontal, which stretches everything.

Also:
- There is **no auto-follow** behind the running player and **no pitch return**: only the player turns the camera.
- The pivot trails the player slightly (time constant about 67 ms), and is locked to the player while sprinting.
- There is no camera collision.

## Where the shot goes
The shot heads for the point where a 150 m trace hits the arena ([shot-aim.md](shot-aim.md)). This is the ray that trace uses.

### The crosshair on screen
- The reticle widget is placed by its own placement function, from the camera's sight point:
  - anchors (min = max) = **(0.50, 0.38)** of the screen;
  - alignment (0.5, 0.5), so the reticle is centred on that point;
  - position offset 0.
- So the crosshair is centred **38% from the top**, 12% of the screen height above the middle: 130 px above centre at 1080p.
- The sight point is a camera setting (BP_CameraThird). Nothing changes it at run time.

### The ray
- **Direction:** the screen point (0.50 × width, 0.38 × height) **deprojected** through the player's current view. It is not the camera's forward vector.
  - At FOV 80 and 16:9 it points **6.5° above** the camera's forward (7.7° at FOV 90).
  - It is at the screen's middle column, so its **yaw is exactly the camera's yaw**.
- **Origin:** the deprojected point (the camera position), then moved forward along the ray to the point closest to the player's capsule centre, if that point is ahead.
  - At level pitch in Default that is about 5 m ahead of the camera, abeam of the player and 0.6 m to their right.
  - The move doesn't change the line, only where the trace starts. It skips anything between the camera and the player.
- **The view is the rig's rotation.** In normal play nothing turns the view away from the yaw and pitch of §2:
  - no look-at, no framing and no pitch offset are active;
  - the look-at-ball button (§11) turns the real yaw and pitch, so the aim follows it. It can't be used with the ball at your feet, but it can during a volley.
  - Camera shakes are part of the rendered view, so they move the ray too (sprint and extra effort have shakes; amplitude not traced).
- **What decides where the ball goes sideways:**
  - The trace keeps to the vertical plane through the camera position along the camera's yaw. On a vertical wall or goal, the hit's ground position doesn't depend on the ray's tilt.
  - The base direction is from the **ball** to that hit, made level.
  - So the ball converges on a line that runs **0.6 m to the right of the pivot** (more with the velocity offset), not through the player.
  - The angle between the shot and the camera's yaw is atan(gap / distance), with gap the sideways distance from the ball to that line. A 0.6 m gap gives 1.7° at 20 m and 0.9° at 40 m; a 1.6 m gap (Shoot at full side offset) gives 4.6° at 20 m.
- **Pivot lag moves that line.** The line hangs on the smoothed pivot. Running sideways at speed v, the pivot trails by about v × (1/k − dt):
  - Default k = 15 at 60 fps: 0.05 s × v, so **0.35 m at 7 m/s**;
  - Shoot k = 10: 0.083 s × v, so 0.4 m at 5 m/s;
  - sprint k = 100: none.
- **The tilt matters only vertically.** The 6.5° up-tilt changes where the ray hits non-vertical things (the crossbar, the top of the net), or whether it misses everything within 150 m.
  - The launch angle comes from the **camera pitch**, not from this ray (shot-aim.md).

### The camera at the moment of the kick
- **Mouse and keyboard:** the aim locks at the **press** (shot-aim.md). So the ray is the one of the camera you had just before the press: Default, sprint, and so on.
  - The shot camera has barely started its transition at that point.
- **Gamepad:** no lock; the ray is read at the strike, with the **Shoot** camera mostly blended in:
  - 0.5 s ease-out from Default or sprint, 0.3 s from Aim;
  - a full-power shot fires at about 0.53 s.
- **Shoot camera** (charging) compared with Default:
  - **closer and higher:** 3.6 m behind instead of 5, fulcrum 0.58 m instead of 0.45;
  - looking down doesn't pull the arm back (no above offset);
  - same FOV, 80;
  - **side offset up to +1.0 m right** (to 1.6 m), from the direction you were moving when the state started (entry kept), eased in at 2 /s. Moving left or standing gives none;
  - pivot k = 10, with the vertical following at k = 3, so the camera barely follows a jump;
  - gamepad turning capped at **120 / 60 °/s, no boost**; the mouse is at half Default sensitivity if it turns the camera at all (open question 7).
- **Release:** Lock shot keeps the Shoot geometry, with turning **locked at 0 °/s**; it blends in over 0.3 s and out over 0.9 s.
- **Aim state:** 4.25 m behind, fulcrum 0.58 m, no side offset, ball lift on, 160 / 80 °/s.
- **Taps:** Aim tap is 5.5 m behind, FOV 75, fulcrum 0.35 m; Lock tap is the same but locked at 0 °/s.

## Per-frame order
1. The behaviour tree picks the state's camera data; a change starts a transition (§8).
2. **Pivot** (§1).
3. **Node services**, in this order: keep previous offset, lock framing (off), ball offset, velocity offset, smoothed speed (off) (§6).
4. **Camera services**, in this order: sleeping, keep previous data, collision detection, following (off), dead-zone repulsion, lock, lock-algo following, offset (no effect), input-move transform, look at ball, look-at, goalkeeper, repulsion, pitch resting (off), side collision, mirror (off), **manual input**, dynamic fulcrum (off), **collision/arm** (computes the offset from pitch), lock framing container, freeze.
5. **Rotation** from yaw and pitch, then FOV, then **position**.

The arm offset is computed after the input, so the position uses this frame's pitch.

## 1. Pivot
- **Target:** the player's root component location, which is the **capsule centre**.
  - The capsule half-height is 0.90 m (radius 0.35 m), so the target is 0.90 m above the feet.
  - For other players' characters (network proxies) the smoothed mesh position is used instead. Same point, smoothed.
  - Other tracking modes exist (pelvis bone, a camera joint, ability-defined). Only the Market shoot and stun cameras use the pelvis.
- **Smoothing, every frame:** pivot += (target − pivot) × min(1, k × dt).
  - X and Y use m_fRefDampingFactor.
  - Z uses m_fRefVerticalDampingFactor when the data sets "override vertical damping" (shots and dives), otherwise the same k.
  - Default **k = 15 /s**. Sprint, sprint with ball and extra effort use **k = 100**: the factor clamps to 1 above about 0.01 s per frame, so the pivot is glued to the player.
  - This is frame-rate dependent. Reproduce the formula, not a continuous exponential.
  - Teleports (kick-off, respawn) snap it.
- **The pivot's rotation** is the player's facing, tracked the same way. It is only used to rotate the fulcrum offset.
- **Fulcrum:** fulcrum point = pivot + (player rotation applied to m_vFulcrumOffset).
  - Default (0, 0, 0.45), so **1.35 m above the feet**.
  - The offset is vertical, so the player's facing doesn't move it.
- **m_fHeightAboveOwner (2 m) is not the pivot height.** It is a parameter of the lock-framing service, which no shipped camera data enables.

## 2. Rotation
- The camera keeps its own **world yaw and pitch** (roll 0).
- **What turns it:** the player's look input, and look-at-ball (§11) and teleports.
- **Pitch clamp:** pitch stays in [−down limit, +up limit], **[−22°, +15°]** in Default, checked on every pitch change.
  - Some states have other limits; tackles and stun can't look up at all (§9).
- **No auto-follow, no return to rest.** The following and pitch-resting services exist but are disabled in every shipped camera data (§10).
- **On teleport (kick-off):**
  - yaw = the player's facing, pitch = 0;
  - the database's "extra pitch on teleport" (−8°) is computed but not passed on (open question).

### Mouse
- **Per frame, not scaled by dt:**
  - Δyaw = x × 0.35 × S × H × π/180 degrees;
  - Δpitch = y × 0.55 × S × Vs × V × π/180 degrees;
  - S = mouse camera speed option, Vs = vertical scale option;
  - H, V = the state's turn speeds in °/s (Default 240 and 120).
- **Default:** **1.466° of yaw per unit of x**, 1.152° of pitch per unit of y (options at 1).
- **Aim (160/80):** 0.977° and 0.768°. Turn speeds act as mouse sensitivity too.
- x and y are the look action's per-frame value, after the invert options. How many units one mouse count gives was not traced.
- No smoothing, acceleration or lag. The input-lag settings exist but are disabled.

### Gamepad
- Each stick axis is clamped to [−1, 1].
- **Anti-rebound:** when an axis crosses to the opposite side with a deflection under 0.5, that input is dropped for up to 0.06 s. This catches the stick springing back on release.
- **Rate (°/s):** yaw rate = x × H × P + b × H_boost × B; pitch rate = y × V × P + b × V_boost × B.
  - P = gamepad camera speed option, B = gamepad boost option.
  - b = a boost value per axis, from −1 to 1.
- **Boost value:**
  - With |input| ≥ **0.9**: b moves linearly toward ±1 (the stick's side), taking (1 − |b|) × **0.2 s**.
  - Below 0.9: b moves linearly back to 0, taking |b| × **0.1 s**.
- **Default:** 240 °/s at full stick at first, **480 °/s** after 0.2 s held. Pitch 120 → 240 °/s.
- **Aim:** 160 → 200 °/s. **Shot charging:** 120 °/s, no boost.
- The acceleration curve option in the database is off. A non-boost path exists but no state uses it.

### Options
- Invert X/Y flip the input first.
- States with the "Aim" settings tag (shot charging and fake shots) read the aim-specific speed options instead of the normal ones.
- After some events, the stick is ignored until it comes back within 0.3 of centre. The trigger was not traced.

## 3. Arm offset by pitch
Each camera data has three offsets in the camera frame:
- Ideal **I**, used at pitch 0;
- Above **A**, used at the down limit; if "use above" is off, A = I;
- Below **B**, used at the up limit; if "use below" is off, B = I.

The offset is a cubic Hermite spline through B → I → A:
- Tangent at I: **T = (A − B) / 2**.
- H(s; P0, P1, M0, M1) = (2s³ − 3s² + 1)·P0 + (s³ − 2s² + s)·M0 + (−2s³ + 3s²)·P1 + (s³ − s²)·M1.
- **Looking down (pitch < 0):** s = clamp(−pitch / down limit, 0, 1); offset = H(s; I, A, T, A − I).
- **Looking level or up (pitch ≥ 0):** s = clamp(1 − pitch / up limit, 0, 1); offset = H(s; B, I, I − B, T).
- Each segment has its own s, so the slope in degrees changes at pitch 0. That is how the game does it.

**Default** (I = (−5.00, 0.60, 0.10), A = (−5.50, 0.60, 0.15), B = (−4.00, 0.60, 0), limits 22° down / 15° up, fulcrum 0.45 m).
Positions are from the fulcrum point and then from the ground, with the player standing.

| Pitch | Offset (camera frame) | Camera behind / right / above fulcrum | Camera height above feet |
|---|---|---|---|
| −22° (full down) | −5.50, 0.60, 0.15 | 5.04, 0.60, 2.65 | **3.55 m** |
| −15° | −5.36, 0.60, 0.14 | 5.14, 0.60, 1.97 | 2.87 m |
| −11° | −5.28, 0.60, 0.13 | 5.16, 0.60, 1.58 | 2.48 m |
| −5° | −5.15, 0.60, 0.12 | 5.12, 0.60, 1.01 | 1.91 m |
| 0° | −5.00, 0.60, 0.10 | 5.00, 0.60, 0.10 | **1.45 m** |
| +5° | −4.70, 0.60, 0.07 | 4.69, 0.60, −0.44 | 1.01 m |
| +10° | −4.35, 0.60, 0.04 | 4.29, 0.60, −0.82 | 0.63 m |
| +15° (full up) | −4.00, 0.60, 0 | 3.86, 0.60, −1.04 | **0.31 m** |

## 4. Position, view and crosshair
- **Camera position** = fulcrum point + (yaw + pitch rotation applied to the arm offset) + the dynamic fulcrum offset.
  - The dynamic fulcrum offset is 0 in normal play (§10).
- **Camera rotation** = (yaw, pitch, 0). The camera looks along that direction from its position. There is no look-at.
- **Where the player appears:** at level pitch with FOV 80 at 16:9, the fulcrum point is at about 43% across and 52% down the screen. It is left of centre by the 0.6 m side offset.
- **Crosshair:** the reticle widget and the aim ray both use the camera's sight point, **(0.50, 0.38)** of the screen, 12% of the screen height above centre. Details in "Where the shot goes".
  - [hud.md](hud.md) says the reticle is in the middle of the screen. It is anchored at this sight point instead.
- **Collision:** none in normal play. Every camera data inherits "collision off" from the template.
  - The safe offset (−2, 0.2, 0) is only the collision fallback, so it is unused.
  - The camera component has a dither parameter for walls; not traced.

## 5. FOV
- **m_fFov is the horizontal FOV at 16:9.**
  - The game turns it into a vertical FOV: vFOV = 2·atan(tan(FOV/2) × 9/16).
  - It then sets the horizontal FOV for the real window: 2·atan(tan(vFOV/2) × aspect).
  - So the vertical FOV is fixed and wider screens see more ("Hor+").
- **Values:**

| m_fFov | 80 | 85 | 90 | 78 | 75 | 115 |
|---|---|---|---|---|---|---|
| Vertical FOV | **50.5°** | 54.5° | 58.7° | 49.0° | 46.7° | 82.9° |
| Horizontal at 21:9 | 95.5° | 100.5° | 105.4° | 93.5° | 90.4° | 128.2° |

- **The FOV option** shifts every state by the same amount: FOV = state FOV + (option − Default's FOV of 80), clamped to [1°, 179°].
- During a transition the FOV is lerped between the two states by the transition ratio.
- If a node's FOV is 0, 90° is used.

## 6. Dynamic offsets (node services)
Each runs only when its flag is on in the current camera data. They write into the frame's copy of the data, so the arm spline (§3) uses the modified offsets.

### Velocity offset: sideways
- **Direction d:**
  - the player's horizontal velocity, normalised; zero when standing still;
  - or the player's facing, when "use forward over velocity" is on (sprint with ball, extra effort).
- **Ratios:** forward = camera forward · d, side = camera right · d. Side is the sine of the angle between d and the camera's yaw.
- **Target lateral offset**, for each of I, A and B:
  - dynamic = side × Max.y when side > 0, |side| × Min.y when side < 0;
  - sum = base y + dynamic;
  - if |sum| > |base y|, the result is sum; otherwise it is |base y| with the sign of sum.
  - The offset never shrinks; it can only grow or **jump to the other shoulder** at the same size.
- **Only Y is applied.** A forward part is computed from Max.x and Min.x but never written. Z is reset to the base value.
- **Smoothing:** VInterpTo, i.e. current += (target − current) × min(1, speed × dt).
  - speed = the data's interp speed (Default 3 /s).
  - Idle rule: once the player has moved, as soon as speed ≤ 1 m/s the offset freezes. After **0.2 s** at rest it eases back at speed **0.5**.
- **Default:** running to the camera's right moves the camera up to **0.45 m further right** (1.05 m). Running left changes nothing, since 0.6 − 0.2 is still on the right side.
- **Shoulder swaps** happen in states where |Min.y| is larger than the base y:
  - extra effort: facing more than 22° left of the camera's yaw;
  - tackle and block;
  - dives (§9).
- With "keep entry", the ratio taken when the state started is used for the whole state (shots).

### Ball offset: upwards
- If the ball is within **7.5 m** horizontally of the camera: lift = clamp((ball height − 1 m) / (2 m − 1 m), 0, 1) × **0.30 m**. Otherwise 0.
- The lift is smoothed by FInterpTo at 3 /s going up and 2.1 /s going down (× 0.7).
- It is added to the base Z of I, A and B.
- **The velocity offset runs after it and rewrites Z** from the base value. So in states with both on (Default and most states), the ball lift has no effect.
  - It only works where the velocity offset is off: sprint without the ball, aim, lock move.
  - This is how the Blueprints read; open question.

### Keep previous offset
- Lock shot, lock tap and outfield dive keep the previous camera's lateral offset (Y only) instead of their own.
- The velocity offset still runs after it.
  - In lock shot (interp 0.1 /s) the previous value effectively stays.
  - In lock tap (interp 50 /s) its own value wins almost at once.

### Goalkeeper camera
- **When:** only for the player with the goalkeeper state, and only in data with the goalkeeper flag (Default, with ball, taps, lock move, dives).
- **Blend:** a value from 0 to 1, driven by a distance between **45 m and 47.5 m**. It rises at 8 /s and falls at 5 /s.
  - What the distance is measured between was not traced. It is probably the ball and the keeper.
- **Effect at full blend:**
  - FOV → 65° (Default data);
  - I and A move **2 m forward and 0.2 m up**, so the camera comes closer when play is far away.
  - B is unchanged.

## 7. States (behaviour tree)
The camera behaviour tree picks the data; the first match wins:

1. Level-design rail camera (blackboard), not used in matches as far as seen.
2. **Hurt → Stun.**
3. **Tackle → Tackling; block → Standing tackle.**
4. Aerial push-ball volley → Aerial push ball.
5. **Without the ball:** dive + goalkeeper → Dive; lock move + goalkeeper → Lock move goalkeeper; dive → Dive outfield; **lock move → Lock move (no ball)**.
6. **Shots:**
   - a tap phase: Lock tap (with the "tap lock" tag) or Aim tap;
   - the shot release phase: **Lock shot**;
   - "Camera.Shoot" or a volley before impact: **Shoot** (charging);
   - fake shot: Fake shoot, or Fake shoot sprinting while sprinting;
   - "Aiming" state: **Aim**.
   - The tap and release phases are decided by shot-tag checks whose enums are unnamed in the files; read from the data names.
7. Wall run → Wall run.
8. **Wants to sprint:** extra effort → **Extra effort**; with the ball → **Sprint with ball**; otherwise **Sprint no ball**.
9. **With the ball → Default with ball.**
10. **Default.**

## 8. Transitions
- **Which transition**, first found:
  1. the old data's "out" transition;
  2. the new data's entry for the old data's class;
  3. the new data's "in" transition;
  4. the component's default (0.65 s).
- **Ratio:** r = clamp(curve(elapsed / duration), 0, 1).
  - Values under 0.0001 count as 0. Duration 0 means r = 1 at once (a cut).
  - The time is **not** clamped before the curve, so a curve with keys past 1 runs longer than the duration.
  - The curve keeps its last value after its last key.
- **End:** the transition ends, and the old camera is dropped, when r ≥ 0.9999.
  - **Two curves never get there.** Entering sprint (no ball) settles at **0.85**; entering extra effort settles at **0.65**.
  - The camera then stays a blend of the two states until the next change. For example, sprint FOV = 80 + 0.85 × 5 = 84.25°.
- **What blends:**
  - Lerped by r: offsets (I, A, B, safe, fulcrum), pitch limits, FOV, framing numbers. Each side keeps its own pivot smoothing, and the two pivots are lerped.
  - Switched at once to the new state: turn speeds, boost settings, flags, tracking mode.
  - Yaw and pitch are shared, so they stay continuous.
- A new change during a transition stacks a new transition on top of the current blend.
- Curves are Unreal cubic keys with stored tangents (the curve assets are listed in Sources). "Ease-out" below means a steep start and a flat end.

| Transition | Duration | Shape | Used for |
|---|---|---|---|
| Default | 0.65 s | S curve (zero tangents) | anything without a more specific one |
| Template default | 1.0 s | S curve | out of wall run |
| Cut | 0 | instant | prematch, from the template |
| To sprint | 2.0 s | 0.95 at 0.2 s, then **0.85** from 0.3 s on | into Sprint no ball |
| To sprint with ball | 0.25 s | S | into Sprint with ball |
| From sprint to sprint with ball | 2.5 s | S | Sprint no ball → with ball |
| From sprint | 3.0 s | S | Sprint no ball → Default |
| Out sprint with ball | 2.0 s | S | Sprint with ball → Default, Sprint no ball or Extra effort |
| In extra effort | 2.0 s | 0.85 at 0.2 s, 0.55 at 0.4 s, then **0.65** from 0.5 s on | into Extra effort |
| Out extra effort | 4.0 s | ease-out (start slope 3.3) | out of Extra effort |
| In / out lock move | 0.6 / 0.8 s | ease-out | Lock move, no ball and goalkeeper |
| From lock move no ball to with ball | 0.5 s | ease-out | Lock move → Aim |
| In shoot (from Default or sprint) | 0.5 s | ease-out (start slope 2.76) | into Shoot |
| In shoot from aim | 0.3 s | same curve | Aim → Shoot |
| Lock shoot | 0.3 s | same curve | into Lock shot |
| Out shoot | 0.6 s, keys to 1.5 | S over **0.9 s** | out of Lock shot; Shoot → Default; Lock move aim → lock move |
| In / out aim tap | 0.7 / 0.8 s | ease-out / S | taps, aerial push |
| In fake shoot | 1.4 s | ease-out | Shoot → fake shot |
| To / from slide tackle | 0.4 / 1.5 s | ease-out / 0.85 at 70% | tackles, block |
| In / out dive (goalkeeper) | 0.4 / 2.0 s | reaches 1 at 60% | Dive; freezes the old camera |
| In / out dive outfield | 0.6 / 2.0 s | reaches 1 at 60% / S | Dive outfield |
| In wall run | 1.5 s | ease-out | Wall run |

## 9. Per-state values
**Geometry.** Offsets I / A / B in m (behind is negative, right, up). "–" means that offset is off (= I). Limits are down / up.

| State | FOV | I | A | B | Fulcrum | Limits |
|---|---|---|---|---|---|---|
| **Default** | 80 | −5.00, 0.60, 0.10 | −5.50, 0.60, 0.15 | −4.00, 0.60, 0 | 0.45 | 22 / 15 |
| Default with ball | 80 | −4.75, 0.60, 0.10 | −5.25, 0.60, 0.15 | −3.75, 0.60, 0 | 0.45 | 22 / 15 |
| **Sprint no ball** | 85 | −4.50, 0.60, 0.10 | – | −3.50, 0.60, 0 | 0.30 | 22 / 15 |
| **Sprint with ball** | 80 | −4.50, 0.60, 0.10 | – | −3.50, 0.60, 0 | 0.30 | 22 / 15 |
| **Extra effort** | 90 | −3.50, 0.60, 0.10 | – | −3.25, 0.60, 0 | 0.35 | 22 / 15 |
| **Lock move (no ball)** | 78 | −5.00, 0.30, 0.10 | −6.00, 0.30, 0.10 | −4.50, 0.30, 0.10 | 0.55 | 22 / 15 |
| Lock move goalkeeper | 75 | −5.75, 0, 0.10 | −6.75, 0, 0.10 | −5.25, 0, 0.10 | 0.65 | 22 / 15 |
| **Aim** | 80 | −4.25, 0.60, 0 | – | −3.75, 0.60, 0.05 | 0.58 | 22 / 15 |
| **Shoot** (charging) | 80 | −3.60, 0.60, 0.10 | – | −3.50, 0.60, 0 | 0.58 | 22 / 15 |
| **Lock shot** (release) | 80 | same as Shoot | – | same | 0.58 | 22 / 15 |
| Aim tap / lock tap | 75 | −5.50, 0.60, 0.10 | −6.00, 0.60, 0.15 | −4.50, 0.60, 0.20 | 0.35 | 22 / 15 |
| Fake shoot | 80 | −4.75, 0.60, 0.10 | −5.25, 0.60, 0.15 | −3.75, 0.60, 0 | 0.45 | 22 / 15 |
| Fake shoot sprinting | 80 | −4.50, 0.60, 0.10 | – | −3.50, 0.60, 0 | 0.30 | 22 / 15 |
| Aerial push ball | 80 | −5.75, 0.60, 0.10 | −6.25, 0.60, 0.15 | −4.25, 0.60, 0.20 | 0.45 | 22 / 15 |
| Dive (goalkeeper) | 80 | −5.00, 0.05, 0.05 | −6.00, 0.05, 0.05 | −4.00, 0.05, 0.05 | 0.55 | 22 / 15 |
| Dive outfield | 80 | −4.50, 0.45, 0.05 | −5.00, 0.45, 0.05 | −4.00, 0.45, 0.05 | 0.55 | 22 / 15 |
| Tackling | 83 | −4.50, 0.60, 0 | – | – | 0.30 | 10 / **−5** |
| Standing tackle | 80 | −4.50, 0.60, 0 | – | – | 0.40 | 10 / **−5** |
| Stun | 83 | −3.50, 0.40, 0 | – | – | 0.45 | 10 / **−5** |
| Wall run | 115 | −5.00, 1.00, 0 | – | – | 0.50 | 20 / 20 |

An up limit of −5 means the pitch must stay at least 5° down.

**Behaviour.** k = pivot smoothing (vertical in brackets when overridden). Turn speeds in °/s, "+" is the boost. "Side offset" is the velocity offset's effect on the 0.6 m base.

| State | k | Turn H / V | Side offset (right / left) | Ball lift | Entry transition |
|---|---|---|---|---|---|
| **Default** | 15 | 240 / 120, +240 / +120 | +0.45 / none | cancelled | Default 0.65 s (see §8 for from-sprint, from-shoot) |
| Default with ball | 15 | 240 / 120, +240 / +120 | +0.45 / none | cancelled | Default 0.65 s |
| **Sprint no ball** | **100** | 240 / 120, +240 / +120 | off | **on** | To sprint 2.0 s (settles 85%) |
| **Sprint with ball** | **100** | 240 / 120, +240 / +120 | +0.20 / none (facing, 0.25 /s) | cancelled | 0.25 s |
| **Extra effort** | **100** | 240 / 120, +240 / +120 | +0.40 / swap to −0.6…−1.0 (facing, 2 /s) | cancelled | 2.0 s (settles 65%); out 4.0 s |
| **Lock move (no ball)** | 50 | 160 / 80, +40 / +20 | off | **on** | 0.6 s; out 0.8 s |
| Lock move goalkeeper | 10 | 160 / 80, +40 / +20 | off | off | 0.6 s; out 0.8 s |
| **Aim** | 20 | 160 / 80, +40 / +20 | off | **on** | Default 0.65 s |
| **Shoot** | 10 (3) | **120 / 60, no boost** | +1.00 / none (entry kept, 2 /s) | off | 0.5 s (0.3 s from Aim) |
| **Lock shot** | 10 (3) | **0 / 0** (locked) | entry kept, 0.1 /s | off | 0.3 s; out 0.9 s |
| Aim tap | 15 | 240 / 120, +240 / +120 | none | cancelled | 0.7 s; out 0.8 s |
| Lock tap | 15 (2.4) | **0 / 0** | none | cancelled | Default 0.65 s |
| Fake shoot | 15 (2.4) | 240 / 120, +240 / +120 | +0.45 / none (entry kept) | off | 1.4 s from Shoot |
| Dive (goalkeeper) | 8 (0) | 160 / 80, +40 / +20 | ±0.6 around 0.05, follows the side | cancelled | 0.4 s; out 2.0 s |
| Dive outfield | 7.5 (1.5) | 160 / 80, +40 / +20 | +0.25 / swap to −0.45 | cancelled | 0.6 s; out 2.0 s |
| Tackling | 7.5 | 160 / 80, +40 / +20 | +1.5 / swap to −0.6…−0.9 | off | 0.4 s; out 1.5 s |
| Standing tackle | 7.5 | 160 / 80, +40 / +20 | +1.0 / swap to −0.6 | off | 0.4 s; out 1.5 s |
| Stun | 20 | 160 / 80, +40 / +20 | off | off | Default 0.65 s |
| Wall run | 20 | 250 / 140, +240 / +120 | off | off | 1.5 s; out 1.0 s |

- A vertical k of 0 (goalkeeper dive) freezes the pivot's height for the dive.
- "Cancelled" ball lift: on in the data, but the velocity offset rewrites it (§6).
- The "Aim" settings tag (aim speed options) is on Shoot, Fake shoot and Fake shoot sprinting, not on Aim.

## 10. Off in normal play
These exist, with data, but no shipped camera data turns them on. A remake can skip them.

| Feature | Switch | What it would do |
|---|---|---|
| **Auto-follow** (yaw behind the movement) | m_bFollow, off everywhere | turn yaw toward the velocity at a rate from FollowYawDampingCurve (0.15 at 0°, 0.4 at 90°, 0 at 160°) × a velocity curve; that curve is unset, so the rate would be 0 anyway |
| **Pitch at rest** | m_bUsePitchAtRest, off everywhere | spring the pitch toward PitchValuePerSlopeAngle(slope): −10° on flat ground, 6° at −20° slope, −20° at +20°. Default sets the curve but not the switch |
| Collision | m_bIsCollisionEnabled, off in the template | slide the camera from the ideal offset toward the safe offset (−2, 0.2, 0) |
| Lock framing | m_bLockFramingService | keep a target inside screen soft edges (0.35), "height above owner" 2 m, distance range from 5 m |
| Dynamic fulcrum offset | m_bDynamicFulcrumOffsetEnabled | move the fulcrum up to 1.5 m around dead zones |
| Mirror (shoulder side) | m_bCanBeMirrored | lerp Y to −Y |
| Offset service | its gains are all 0 | input-driven offset |
| Smoothed turn speed | EnableSmoothedCameraSpeed | VInterpTo on the turn speeds |
| Input lag | CameraLagStruct, disabled | fade-in/out of look input |

The inactive services use a critically damped spring: v ← (v − ω²·dt·(x − target)) / (1 + ω·dt)², then x ← x + v·dt.

## 11. Look at the ball (button)
A separate ability ("look at ball" input) turns the camera toward the ball. It is not traced in depth; what the data says:
- It can't be used while you have the ball or are taking it. Camera data must allow it, and all do by default.
- **Reach:** the camera reaches the ball's position predicted **0.2 s** ahead, in **0.2 s** with a cubic ease-out. The ball sits 0.13 of the screen above centre.
- **Tap** (released before 0.2 s): the camera stays pointed at the ball.
- **Hold:** it keeps tracking the ball, damped by distance (YawDampingFromDistance: 0 at 0 m up to 3.5 at 9 m). On release it goes back to the orientation it had before.
- Manual input during the lock is allowed; the lock wakes up again 0.5 s after the last input.
- While active, the goalkeeper's dive assist is disabled.

## 12. The "Market" set
- **Cheat only:** the cheat command "ToggleMarketCameraPreset" redirects each camera data to its "_Market" twin. It also sets a "market mode" material flag. It is a marketing/capture camera, not a player setting.
- **Differences from Default:**
  - closer: Default 4.5 m behind, 0.8 m right, fulcrum 0.25 m;
  - FOV 70;
  - faster yaw: 360 °/s (300 in most other states);
  - tighter pitch: 10° down, 13° up;
  - a bigger rear pull when running at the camera: Min.x −2.5, which has no effect (§6);
  - shoot and stun cameras track the pelvis bone.

## Constants
Source values (cm, degrees, per second) from Default_CameraData unless noted.

| Name | Value | Source |
|---|---|---|
| Pivot target | capsule centre, half-height 90 cm, radius 35 | BP_RuntimeTPC CollisionCylinder |
| Fulcrum | (0, 0, 45) | m_vFulcrumOffset |
| Ideal / above / below offsets | (−500, 60, 10) / (−550, 60, 15) / (−400, 60, 0) | m_vIdealOffset, m_vAboveIdealOffset, m_vBelowIdealOffset |
| Pitch limits | 22 down, 15 up | m_fAboveLimitPitch, m_fBelowLimitPitch |
| FOV | 80 (horizontal at 16:9) | m_fFov; the 9/16 factor is a native constant |
| Pivot smoothing | 15 (sprint and extra effort 100) | m_fRefDampingFactor |
| Turn speeds | 240 / 120, boost 240 / 120 | m_fHorizontalSpeed, m_fVerticalSpeed, …Boost (template) |
| Boost threshold, ramp | 0.9; up 0.2 s, down 0.1 s | m_fSpeedBoostInputThreshold; CameraDB m_fAccelDuration, m_fDecelDuration |
| Anti-rebound | below 0.5, for 0.06 s | CameraDB m_fThresholdToSkipAntiRebound, m_fTimeToSkipAntiRebound |
| Mouse scales | yaw 0.35, pitch 0.55 | CameraDB m_fMouseYawScale, m_fMousePitchScale |
| Crosshair | (0.5, 0.38) of the screen | BP_CameraThird m_v2DScreenPercentSight |
| Velocity offset | min (−60, −20, 0), max (0, 45, 0), 3 /s | MinVelocityOffset, MaxVelocityOffset, VelocityOffsetInterpSpeed |
| Velocity idle | 100 cm/s, 0.2 s, then 0.5 /s | BP_VelocityDynamicOffset_CameraNodeService |
| Ball lift | 30 cm between 100 and 200 cm high, within 750 cm; 3 /s, × 0.7 down | template Ball… fields |
| Goalkeeper camera | 4500–4750 cm; in 8, out 5; FOV 65; +(200, 0, 20) | BP_GoalKeeperCameraService, m_fGoalKeeperServiceMaxDistanceFOV, m_fGoalKeeperOffsetFromDistance |
| Default transition | 0.65 s; template 1.0 s | Default_CameraTransition, native default |
| Teleport extra pitch | −8 (not applied, see Open questions) | CameraDB m_fExtraPitchOnTeleport (native default) |

## Open questions
1. **Mouse scale:** how many look-input units one mouse count gives, and the camera speed and vertical scale option ranges. The formulas above take the input as given.
2. **Ball lift cancelled?** In states with both dynamic offsets on, the velocity offset rewrites Z after the ball offset. Is the ball lift ever visible in Default?
3. **Partial transitions:** do sprint (85%) and extra effort (65%) really stay blended? Compare the sprint FOV in-game: 84.25° expected, not 85°.
4. **Goalkeeper camera:** which distance it measures.
5. **Teleport pitch:** the −8° extra pitch is computed but appears unused. Check the pitch after a kick-off in-game.
6. **Shot phases:** which shot-tag enums select Aim tap, Lock tap and Lock shot. The names are unnamed numbers in the files.
7. **Mouse during shots:** with the mouse feeding the curve while charging (shot-aim.md), does the Shoot state still turn the camera?
8. **Camera shakes** (sprint, extra effort, landing) are part of the view and so of the aim ray. Their amplitude wasn't traced.

## Sources
**Native code** (Rematch build of 2026-10-24). Some labels in the decompiled dump are wrong (e.g. the functions labelled "SCCameraManualService__vf89/vf90" belong to the mirror service), so functions are listed by address.

| Function | What it is |
|---|---|
| `0x14496bb50` | camera component tick: order, look input flags, rotation, FOV, position |
| `0x144974bf0`, `0x144965b90` | pivot tracking and its damping factor |
| `0x14495e070` | tracked transform per tracking mode |
| `0x14495b6c0`, `0x144a51a20` | arm offset by pitch (spline), the clamped remap |
| `0x144975730`, `0x14497fe20` | arm with collision ratio, run by the collision service |
| `0x14495b5e0`, `0x14495bea0` | final camera position |
| `0x14495d530`, `0x14495d780` | rotation from yaw, pitch, roll |
| `0x14496ffc0` | pitch clamp |
| `0x144982250`, `0x14497ea70` | manual service: input to yaw and pitch, its run condition (vtable `0x1476f4f38`) |
| `0x14498ff30`, `0x14498f8a0`, `0x1449911c0` | gamepad boost, anti-rebound, input lag |
| `0x144963d20`, `0x144963c40`, `0x144953410` | FOV conversion and the FOV option |
| `0x1449591b0`, `0x145315720` | crosshair ray (deprojection, moved to the player) and the shot ray built from it for the camera shot direction |
| `0x144976e80`, `0x144960e90` | transition update and ratio |
| `0x144971f10`, `0x144974f50`, `0x144963c90` | data blend, pivot blend, FOV blend |
| `0x144967f80`, `0x144966270`, `0x144964fb0` | transition choice |
| `0x144980180`, `0x14497e550`, `0x144988390` | following (off) |
| `0x144982fe0` | pitch resting (off) |
| `0x144980440` | dynamic fulcrum offset (off) |
| `0x14535c4b0` | goalkeeper node service |
| `0x1449695f0` | teleport reset |
| `0x144a528a0`, `0x144a53ec0` | damped spring, angle lerp |
| `0x1447e0e50`, `0x1452b6a50`, `0x1447e0990`, `0x1447e9ac0` | SCCameraData, RuntimeCameraData, SCCameraDB, transition defaults |

Property offsets come from the reflection tables (tools/usmapgen.py).

**Blueprints:** BP_CameraThird (service order, sight point), BP_VelocityDynamicOffset_, BP_BallDynamicOffset_, BP_KeepPreviousOffset_ and BP_SmoothedCameraSpeed_CameraNodeService, BP_GoalKeeperCameraService, BP_CameraServiceManual, CameraBehaviorTree, BTDecorator_HasShootTag, WBP_Crosshair (PlaceCrosshair and its canvas slot), GA_LookAtBall, BP_CameraLibrary, BP_RuntimeCheatManager (ToggleMarketCameraPreset).

**Data:** Template_CameraData and every *_CameraData and *_CameraData_Market asset; CameraDB, CameraLagStruct; the *_CameraTransition assets and their curves (DefaultTransitionCurve, InSprintTransitionCurve, InExtraeffortTransitionCurve, OutExtraeffortTransitionCurve, InShootCameraTransitionCurve, OutShootCameraTransitionCurve, In_/Out_LockmoveTransitionCurve, In_/Out_DiveTransitionCurve, SlideTackleCurve, LinearInAutoOut, WallRunTransitionCurve); FollowYawDampingCurve, PitchValuePerSlopeAngle, YawDampingFromDistance; BP_RuntimeTPC.
