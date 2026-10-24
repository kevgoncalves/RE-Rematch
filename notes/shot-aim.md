# Shot aim: direction, height and curve (spec)

How a shot's direction, launch angle and spin are built from the camera and the curve input. Force (speed) curves, build-up and contact frames are in mechanics.md §4. Ball flight after the kick is in [ball-flight.md](ball-flight.md).

**Status:** interpreted, waiting for Kev's review. A prototype implementation exists on branch `game/ball-flight` (not merged).

## Order of operations
The shot's direction computation runs these steps when the ball is struck.

1. **Trace the crosshair.**
   - From the camera along its view, up to **150 m** (15000 cm).
   - The trace ignores players (Pawn) and the static world (WorldStatic), so the floor doesn't stop it. It ends on the arena's walls or the goal, or at 150 m.
   - The game has two versions, "from the player" and "from the predicted player", for when the kicker's position is still being predicted.
2. **Base direction.**
   - The direction from the ball to the traced point, made level (the vertical part is dropped).
   - This is what puts the ball under the crosshair even though the camera sits to one side of the player.
3. **Spin from the input** (below).
4. **Launch angle (pitch)** (below).
5. **Lift the base direction by the pitch**, about the level axis to its right.
6. **Yaw offset** (below), about the vertical axis.
7. **Speed** = the shot's force curve (mechanics.md §4) along that direction.

A mode byte in the shot data picks whether steps 4 and 6 run; outfield shots run both.

## Launch angle
- **Camera pitch:** the angle of the camera's view above the horizon, or the controller's view pitch for a local player.
- **pitch = TrajectoryAngleByCameraPitch(camera pitch)**:

| Camera pitch | −24.9 | −14.6 | −13.2 | −7.6 | 0.7 | 4.0 |
|---|---|---|---|---|---|---|
| Launch angle | 2.93 | 2.93 | 3.99 | 9.29 | 17.42 | 20.46 |

Linear between keys and flat outside them.
- **Dip correction:** when the vertical curve input y is negative (pulled down = dip), add **4° × |y|**.
  - The correction curve is constant over camera pitch: 0 for one sign of the input, 4 for the other.
  - The |y| comes from a multiplier curve (−1→1, 0→0, 1→1).
- **Pitch range clamp:** the result is clamped (as an angle) to the computation's pitch range.
  - The native default is [1°, 45°]. The shot Blueprints set the upper bound to 2°, which would make every shot almost flat.
  - Since the camera curve goes to 20°, that cap is assumed not to be in effect. **Open question.**
- **Examples at 30 m/s:**
  - level camera (17°): the ball passes 20 m at about 3 m high;
  - camera slightly down (9°): it lands around 20 m;
  - full dip at a level camera: about 1.2 m high at 20 m.

## Yaw offset
- **offset = TrajectoryYawByXAxis(−x)**, with x the side curve input (the sign flips when the "invert X" option is on).
- The curve is linear from −3° to +3°.
- So the shot turns up to **3° against the curve**: curving right, it leaves a little to the left and bends back toward the crosshair.
- The offset is measured against a second trace from the ball to the wall ("Trace Ball Wall Impact Point").

## Direction lock
- **Keyboard and mouse:** the aim locks at the start of the ability (the press). After that the mouse feeds the curve instead of the camera.
- **Gamepad:** no lock. The right stick keeps aiming while the left stick sets the curve.

## Spin from the input
The spin computation gives a spin vector in the flight frame (see ball-flight.md): spin.z curves right, spin.y dips.

1. **Input (x, y).**
   - **Gamepad:** the left stick, with a dead zone of 0.2 (class default). The tolerance angle is 0 (set in the Blueprint).
   - **Mouse:** an accumulator.
     - **What it reads:** the "Magnus" input has two axis slots: MagnusHorizontal is MouseX and MagnusVertical is MouseY, both with scale 1.0 (keyboard presets).
       - These are engine mouse axes. Their value is the frame's mouse movement in pixels times the engine's mouse axis sensitivity, 0.07 by default (the game's input config was not read).
       - Mouse up is positive Y, so pulling the mouse down (toward you) gives negative y, which is the dip.
     - **Swap axis:** the "swap axis" flag swaps the vector built from the slots back into (horizontal, vertical). Net effect: **horizontal mouse → curve, vertical mouse → dip/lift**.
     - **Accumulation:** each update adds the input × **0.2** × the "Magnus sensitivity" option. The option's default was not found; assumed 1.0.
       - At 1.0 a full curve takes about **71 pixels** of mouse movement (1 ÷ (0.07 × 0.2)).
     - **Clamp:** the total is clamped to a **circle of radius 1** (as a 2D length, not each axis), so full curve and full dip together give about 0.71 on each axis.
     - **Start:** it is kept on the shot's own data, so it starts at zero for each shot.
2. **Inversion:** the invert X and invert Y options negate x or y.
3. **Spin:**
   - **spin.z = SpinningRotationByXStickAxis(x):** 0 within ±0.1, ±350 at 0.5, ±650 at 0.9, ±825 at 1. Positive curves right.
   - **spin.y = SpinningRotationByYStickAxis(y):** pulling down gives +330 at −0.5, +660 at −0.9, **+780** at −1 (dip). Pushing up gives −40, −80, **−120** at 1 (lift). Dead between −0.1 and 0.2.
4. **Charge scaling:** a ratio scales each axis:
   - side (spin.z): **1.02 → 0.8**;
   - vertical (spin.y): **0.95 → 0.85**, from a tap to a full charge.

   - **The ratio comes from time, not force or speed.** The modifiers' source is "max time from shoot force curve": the shot's input time ÷ the force curve's last key (1.0 s), capped at 1.
   - The input time is the same one the force uses, held at least the build-up (the shot's force source clamps with the build-up duration). So a tap counts as 0.53.
   - The "DependingForce" in the curve names is misleading.
5. **Built-in spin:** the shot's own Magnus modifier then adds its forced spin once at launch, spin.y −= 210. That is a small **lift** on every shot.

## Units and timing
- **No unit conversion anywhere on the way.** The curve values (825, 780, …), scaled by charge, are stored unchanged as the shot's rotation velocity. The flight copies them as its starting spin, and the Magnus force multiplies them by 0.0007986 × speed (ball-flight.md). They are not degrees, radians or rpm; only that product matters.
- **When the spin is read:** the spin is recomputed on every update while the shot charges (the accumulator keeps adding).
  - Once the shot's data is synced (after the release), the computation returns the cached value instead.
  - The ball takes that value when the shot is built at contact.
  - The exact sync moment is the shot sync task's (not traced further).

## Prototype check (game/prototype b74e576)
Everything above matches the prototype except:

| # | Prototype | Game | In play |
|---|---|---|---|
| 1 | The mouse curve is clamped per axis (a square) | Clamped to a circle of radius 1 | **Curve and dip together are too strong in the prototype**: up to 825 side + 780 dip, where the game gives about 505 + 500. Pure curve or pure dip are the same |
| 2 | Stick and mouse are added together; no stick dead zone | One device's computation; stick dead zone 0.2 | Gamepad only: small stick tilts curve a little in the prototype |

Matching:
- the spin curves, the charge ratio (with the build-up floor), the shot's built-in lift, the units;
- the Magnus constant, the fade and the flight frame;
- the dip launch correction and the 3° yaw offset;
- the mouse scale (0.07 × 0.2 per pixel, assuming the option is 1.0 and the engine's default mouse sensitivity).

## Constants
| Name | Value | Source |
|---|---|---|
| Trace length | 150 m | native constant |
| Launch angle by camera pitch | table above | Curve_TrajectoryAngleByCameraPitch |
| Dip correction | 4° × \|y\| for y < 0 | Curve_ShootPitchCorrectionByCameraPitchForVMagnusNegatifOrPositif, VerticalMagnusMultiplier |
| Pitch range | [1°, 45°] native; upper 2° in the Blueprints | ControllerShootDirectionComputation, BP_*_CameraDirectionComputation |
| Yaw offset | ±3° against x | Curve_TrajectoryYawByXAxis |
| Mouse accumulator | 0.2 × option, radius 1 | BP_MK_InputRotationForceComputation |
| Side spin | ±825 | Curve_SpinningRotationByXStickAxis |
| Vertical spin | +780 / −120 | Curve_SpinningRotationByYStickAxis |
| Charge scaling | side 1.02→0.8, vertical 0.95→0.85 | CurveShootModifierComputationData_H/VMagnusDependingForce |
| Built-in spin | −210 on spin.y (lift) | BP_Shot_MagnusDefault |

## Open questions
1. ~~Is the 2° pitch cap in effect?~~ No: Kev confirms raising the camera makes shots fly higher, up to a cap. That cap matches the launch angle curve going flat at 20.46° (camera pitch +4° and above) together with the camera's own pitch limit, so the Blueprints' 2° upper bound isn't what applies.
2. The Magnus sensitivity option's default, and the value Kev plays with: it scales the mouse curve directly (at 2.0 a full curve takes 36 pixels instead of 71).
3. The engine mouse axis sensitivity, assumed to be the default 0.07 per pixel (the game's input config wasn't read).
4. Whether the accumulator is fed more than once per frame (it adds on every call). If two systems ask for the spin each frame, the effective mouse sensitivity would double.
5. The camera's default pitch and range decide every shot's height through the launch angle curve. They belong to the camera notes.

## Sources
**Native code** (Rematch build of 2026-10-24):

| Function | What it is |
|---|---|
| `FUN_145314f50` | the shot direction: trace, base direction, spin, pitch, yaw |
| `FUN_145312700` | the 150 m trace |
| `FUN_1453161f0` | launch angle: curve, dip correction, range clamp |
| `FUN_14530ba80` | yaw offset |
| `FUN_14531aa80` (`InputShootRotationForceComputation` vf89) | mouse accumulator |
| `FUN_145316870` (vf90) | input to spin, inversion, charge scaling, freeze after the sync |
| `FUN_145313fd0` (vf92) | whether the curve input is taken this update |
| `FUN_14531fbc0` (vf91) | spin as a share of the curves' range (for the crosshair) |
| `ShootModifierComputationData` vf89 (`FUN_145321f90`) | the modifier ratio by source: force, time, max time from the force curve, … |
| `FUN_14530eec0` | builds the shot: force × direction, spin into `ShootParams.m_vRotationVelocity` |
| `FUN_145372ea0` | the flight copies that spin as its starting spin |
| `FUN_1452de6a0`, `FUN_1452e7370` | constructors (defaults) |

**Data:** GA_Shoot, BP_ShootAbilityData, BP_MK_ and BP_Gamepad_CameraDirectionComputation, BP_MK_ and BP_Gamepad_InputRotationForceComputation, and the curves listed above.
