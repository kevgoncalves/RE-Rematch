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
   - **Gamepad:** the left stick.
     - A tolerance angle of 15° and a dead zone of 0.2 apply (class defaults).
   - **Mouse:** an accumulator.
     - Each update adds input × **0.2** × the "Magnus sensitivity" option.
     - The total is clamped to a **circle of radius 1**, so diagonals don't reach 1 on both axes.
     - It is kept on the shot's own data, so it starts at zero for each shot (assumed).
     - Both input setups also have a "swap axis" flag set. The raw order of the mouse action's vector was not traced, so the net effect is unconfirmed.
2. **Inversion:** the invert X and invert Y options negate x or y.
3. **Spin:**
   - **spin.z = SpinningRotationByXStickAxis(x):** 0 within ±0.1, ±350 at 0.5, ±650 at 0.9, ±825 at 1. Positive curves right.
   - **spin.y = SpinningRotationByYStickAxis(y):** pulling down gives +330 at −0.5, +660 at −0.9, **+780** at −1 (dip). Pushing up gives −40, −80, **−120** at 1 (lift). Dead between −0.1 and 0.2.
4. **Charge scaling:** a ratio of time held ÷ the force curve's last key scales each axis:
   - side (spin.z): **1.02 → 0.8**;
   - vertical (spin.y): **0.95 → 0.85**, from a tap to a full charge.
5. **Built-in spin:** the shot's own Magnus modifier then adds its forced spin once at launch, spin.y −= 210. That is a small **lift** on every shot.

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
1. Is the 2° pitch cap in effect? If raising the camera makes shots fly higher in Rematch, it is not.
2. The mouse input's raw scale and axis order before the accumulator (the custom input mapping wasn't traced). The prototype uses 0.07 per pixel.

## Sources
**Native code** (Rematch build of 2026-10-24):

| Function | What it is |
|---|---|
| `FUN_145314f50` | the shot direction: trace, base direction, spin, pitch, yaw |
| `FUN_145312700` | the 150 m trace |
| `FUN_1453161f0` | launch angle: curve, dip correction, range clamp |
| `FUN_14530ba80` | yaw offset |
| `FUN_14531aa80` | mouse accumulator |
| `FUN_145316870` | input to spin, inversion and charge scaling |
| `FUN_1452de6a0`, `FUN_1452e7370` | constructors (defaults) |

**Data:** GA_Shoot, BP_ShootAbilityData, BP_MK_ and BP_Gamepad_CameraDirectionComputation, BP_MK_ and BP_Gamepad_InputRotationForceComputation, and the curves listed above.
