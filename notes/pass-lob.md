# Passes and lobs (spec)

What passes and lobs share with shots, and what differs. Force curves, launch angles, build-up and contact frames are in mechanics.md §4. The aim modes are in controls.md ("Pass aiming"). Ball flight is in [ball-flight.md](ball-flight.md).

**Status:** interpreted, waiting for Kev's review. A prototype implementation exists on branch `game/ball-flight` (not merged).

## Direction
- **The same crosshair base as shots.** The pass direction classes (FilteredForwardInput → InputForward) derive from the controller direction class.
  - Without a stick or drag direction, they call the same direction code as shots: the 150 m crosshair trace, then the level direction from the ball to the traced point (shot-aim.md, steps 1–2).
  - So a pass aimed "straight ahead" goes to the crosshair's point, not along the camera.
- **With a direction input**, the aim mode decides (controls.md):
  - directional aiming (keys or stick, camera-relative, dead zone 0.4);
  - joystick emulation (mouse drag);
  - crosshair only.
  - Within **6°** of straight ahead the pass snaps to straight ahead; the other directions use a **22.5°** tolerance (Tap_OrientationDataTable_*).
- **Filtering:** the filtered computation can smooth the direction (a lerp system, off in the keyboard data) and has an acceleration range and multiplier (70 and 10 in the Blueprints). How these act was not traced.
- **Height:** the pass direction Blueprints carry no camera-pitch curve, so passes and lobs take their launch angle from the kick data, not the camera:
  - passes leave flat, lifting 3.9–6° past the 0.6 s sweet spot;
  - lobs leave at 44° → 30.26° by charge;
  - low-power lobs at 52°.

  Whether the controller's pitch step is skipped for passes (by the mode byte) was not confirmed.

## Charge
- **Same clock as shots** (mechanics.md §4): the time held from the press, including time spent in the input buffer, read against the build-up.
  - The build-up is 24 frames from the global shot table, less the 0.15 s network margins.
  - So the curves run on **charge = time held ÷ ~0.23 s**, capped at 1.
- **Full charge after ~0.23 s;** the ball still leaves at the 0.4 s contact frame.
- **The pass sweet spot** (32 m/s, 60% of the charge) is at **~0.14 s**. Holding longer drops it to 25.6 m/s, then it climbs to 30.65 m/s at full charge.
- **A tap** gives 20 m/s (pass) and 16 m/s (lob). Full lob: 27 m/s.
- **No analog fill curve:** a half-pressed trigger charges at full speed.

## Moving while you pass or lob
- **You keep all of your speed at the press,** in that direction and without steering, through the build-up and after the touch (the pass and lob animations' speed tracks: 100% of the entry velocity, kept to the animation's end).
- So a pass or lob out of a dribble **drags you on the dribble's way**, like a shot (players confirm). A shot instead shrinks to 75% (50% out of a dribble).
- Under 2 m/s the animation's own motion plays instead.
- After the touch the drag lasts to the animation's move interrupt (frames 18 to 63 of its track); steering the same way leaves it earlier.

## Flight
| Kick | Friction μ | Drag Cd | Forced spin (spin.y) | Rolls (at speed v) |
|---|---|---|---|---|
| Pass (players, analog power) | 0.96 | 0.45, then 0.94 from the first bounce | −200 (**lift**), up to the first bounce | v ÷ 0.96 m: 26 m at 25 m/s |
| Low-power pass | 2.01 | 0.45 | none | 12 m at 24.25 m/s |
| Lob (analog power) | 0.8 | 0.45 | **+300 (dip)** | |
| Low-power lob | 0.8 | 0.45 | **+420 (dip)** | |

- A pass floats very slightly. A lob dips, so it drops more steeply than plain gravity would make it.
- **Examples:**
  - full lob (27 m/s at 30.26°): lands about 28 m out, peaking at 4.3 m;
  - weakest lob (16 m/s at 44°): lands about 14 m out;
  - low-power lob (13.2 m/s at 52°): lands about 10 m out.

## Spin input
- Passes and lobs take no curve input; their spin is only the forced spin above.
- The pass ability has a Magnus input action; its use for passes was not traced.

## Sources
- **Native:**
  - `FUN_14531c7f0` (the filtered pass direction);
  - `FUN_14531da00` (the input-forward base, which calls the shared shot direction `FUN_145315720` → `FUN_145314f50`).
- **Data:**
  - BP_TapAbilityData_AnalogPower, BP_TapAbilityData_LowPower, BP_LobAbilityData_AnalogPower and BP_LobAbilityData_LowPower (overrides, weights, bounce ranges);
  - BP_*_Tap_FilteredInputDirectionComputation*;
  - Tap_OrientationDataTable_*;
  - BP_Tap_ and BP_Lob*_MagnusTrajectoryModifier(s).
