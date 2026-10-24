# Ball flight (spec)

How the ball moves once it is loose: the forces in flight and on the ground, the spin, and what each touch changes. Interpreted from the game's native trajectory code and its data. The rules are in our words; the sources are at the end.

**Status:** interpreted, waiting for Kev's review. A prototype implementation exists on branch `game/ball-flight` (not merged).

## Units
- The game works in centimetres and cm/s. These notes give metres and m/s; the source values in cm are noted where they differ.
- Angles are degrees unless noted.
- "Spin" is the game's own unit (the curve values such as 825 or 210). Only its product with the constants below matters.

## How a flight is computed
- **Precomputed path.** The ball's path is computed ahead as a predicted trajectory and replayed. It is rebuilt from each touch and each bounce, called a **segment**.
- **One step:**
  1. Each active modifier gives an **acceleration**.
  2. The accelerations are summed, each times its **weight**.
  3. Position moves by v·dt + ½·a·dt², and velocity by a·dt.
  4. Each modifier can then correct the result. For example, friction cannot push the ball backwards; when a correction changes the position, the acceleration is recomputed to match.
- **Nothing is divided by the mass.** The ball's Mass attribute only appears as a factor in gravity.
- **When a modifier is active:**
  - **State mask:** the modifier's "trajectory step" field is a bit mask of where it acts: in flight (bit 0), rolling (bit 1), or both. A "3" in the files means both. It is not a step count.
  - **Bounce range:** a range of bounce numbers within the current flight, e.g. "from the first bounce on" or "up to the first bounce".
  - **Until hit:** stops after that many hits.
  - **Duration:** active only for that many seconds of the flight.
  - **Weight:** a global weight that multiplies the acceleration, optionally times a curve on the speed at the first bounce.
- **Segment timers:** each segment keeps a timer of time spent rising and a timer of time spent falling or flat. Gravity reads the falling one.

## Default stack
The ball's physics component carries these four, all with weight 1:

| Modifier | Acts | Extra |
|---|---|---|
| Gravity | in flight | |
| Air drag | in flight | |
| Magnus | in flight and rolling | stops after 16 hits |
| Friction | rolling | |

## Gravity
- **Acceleration down:** g × (Mass + 0.5 × t_fall).
  - g is the world gravity. It is assumed to be the engine default, **9.8 m/s²** (980 cm/s²); the map's own setting was not read.
  - Mass is the ball attribute, **1.625**.
  - 0.5 is the modifier's free-fall multiplier.
  - t_fall is the seconds spent falling (or flat) in the current segment.
- **So:** **15.9 m/s²** while the ball rises, growing by 4.9 m/s² for each second it falls. A lob comes down harder than it went up.
- **Optional caps, both off on the ball:**
  - a maximum on the 0.5 × t_fall term (−1 = off);
  - a periodic extra force.
- **Above 1000 m/s** there is no gravity (the critical speed threshold, 100000 cm/s). This never happens in play.

## Air drag
- **Acceleration against the velocity:** Cd × K × speed² × (1 − |v̂·up|).
  - K = **0.022808** per metre (2.2808·10⁻⁴ in cm units). It matches half the air density times the ball's cross-section.
  - The last factor weakens drag the more vertical the flight: none straight up or down, full when level.
  - Cd is the touch's drag coefficient, **0.45** by default (the ball's DragForce attribute).
- **How Cd is chosen** (each drag modifier, in order):
  - If the modifier does **not** override, Cd = its own default value.
  - If it overrides, Cd = its override constant, replaced by the gameplay attribute when it reads one (from the ball, or from the kicker).
  - The ball's default drag modifier overrides and reads the ball's DragForce, so its 2.54 is never used.
- **Example:** a 30 m/s shot slows to about 24 m/s over 20 m.

## Magnus and spin
- **Spin is a vector in the ball's flight frame.** The frame follows the velocity: x along the path, y to its right (level), z up from both. The spin turns with the path.
- **Acceleration in flight:** M × speed × (spin.z × right − spin.y × up).
  - M = **0.0007986** (the same in m or cm).
  - **spin.z > 0 curves right; spin.y > 0 dips; spin.y < 0 lifts.**
- **Rolling:** the same force, turned 90° about the level axis across the path.
  - The dip/lift part then pushes along the path: dip spin brakes a rolling ball, lift spin pushes it on.
  - The side part keeps curving the ball.
- **Coefficient:** M is the modifier's default force, 1.0, times the constant above. Kicks don't change it.
- **Units of spin:** none to convert. The kick's spin values (the input curves, e.g. ±825, already charge-scaled) are stored unchanged in the shot (`ShootParams.m_vRotationVelocity`). The flight copies them as its starting spin, and the formula above uses them as they are. Checked end to end in shot-aim.md "Units and timing".
- **Spin fades:** each step spin loses rate × dt, with rate = A ÷ B from the flying (or rolling) pair. Both pairs default to (1, 1), so spin fades at **1 per second**, flying or rolling.
- **Forced spin.** A kick's Magnus modifier can add a fixed spin once, when the flight starts (after an optional delay, and only on the right bounce number).
  - It **subtracts** the file's Y value from spin.y and adds its X value to spin.z.
  - So a file value Y = +210 is a **lift**, and Y = −300 is a **dip**.
- **Bounces:** each surface keeps a share of the spin (its Magnus bounciness): 1.0 on grass, glass, posts and nets; 0.7 on the special glasses.
- **Example:** at 30 m/s, spin 660 gives about 16 m/s² sideways, which bends a shot about 3 m by 20 m.

## Rolling friction
- **Acceleration against the velocity:** μ × v.
  - Friction is proportional to speed, so the ball slows exponentially and rolls **speed ÷ μ metres** in all.
  - μ = the ball's Friction attribute (**0.8**) × the touch's multiplier × the override's weight.
  - The surface's physical material can change μ (the combine rule was not read; grass is assumed neutral).
- **Stop:** a stop speed lives in the global ball physics settings (config, not read). The prototype stops below 0.05 m/s.

## Per-touch stacks
A kick's data replaces modifiers by class for its whole flight (or for a bounce range). Rolling distance = speed ÷ μ.

| Touch | Friction μ | Drag Cd | Forced spin (spin.y) |
|---|---|---|---|
| Shot | 0.8 | 0.45 | −210 (lift) |
| Players' pass (analog power) | 0.8 × 1.2 = **0.96** | 0.45; **0.75 × 1.25 = 0.94 from the first bounce** | −200 (lift), up to the first bounce |
| Low-power pass | 0.8 × 1.5 × 1.675 = **2.01** | 0.45 | none |
| Bots' pass | 0.8 × 0.3 = 0.24 | 0.45 | none |
| Lob (analog power) | 0.8 | 0.45 | **+300 (dip)** |
| Low-power lob | 0.8 | 0.45 | **+420 (dip)** |
| Lob (classic data) | 0.8 | 0.45 | +500 (dip) |
| Push balls (fast tap) | 0.8 × 2.8 = **2.24** | 0.45 | none |
| Lobbed push ball | 0.8 | 0.45 | **+2500 (dip)** |
| Volley push ball | 0.8 × 1.6 = 1.28 | 0.45 | +1000 (dip) |
| Extra effort taps | 0.8 × 1.5 = **1.2** | 0.45 | none |
| Goalkeeper push ball | 0.8 × 6 = 4.8 | 0.45 | none |
| Goalkeeper lobbed push ball | 0.8 | 0.45 | +2200 (dip) |
| Goalkeeper low-power pass from the hand | 0.8 × 1.5 | 1.4 | none |

Examples:
- push ball at 11.5 m/s: 5.1 m;
- 25 m/s pass: 26 m;
- low-power pass at 24.25 m/s: 12 m;
- 30 m/s rolling shot: 37 m.

## Ball attributes (DefaultBallAttributeTable)
| Attribute | Value |
|---|---|
| Friction | 0.8 |
| DragForce | 0.45 |
| Mass | 1.625 |
| Max speed | 70 m/s (7000 cm/s) |
| Perpendicular / parallel / Magnus bounciness | 1.0 (scaled by the surface) |
| Radius | 11.07 cm (hit sphere 12 cm) |

## Bounces
- The surface values (perpendicular and parallel bounciness) are in mechanics.md §11 and arena.md.
- **Roll or bounce:** the curve ToRollMaxAngleBySpeedCurve gives the largest landing angle at which the ball starts rolling instead of bouncing:
  - 90° at rest;
  - 0.5° from 40 m/s.

  The native code that uses it was not found yet.

## Open questions
1. World gravity: assumed 980 cm/s². The 5v5 map's world settings were not read.
2. The ball's stop speed in the global physics settings (config).
3. The meaning of the Magnus "until hit 16". Assumed: the default Magnus stops after 16 hits.
4. Where the roll-or-bounce angle is checked, and how the surface combines with friction.

## Sources
- **Data:**
  - BP_Ball (the physics component's stack and weights);
  - DefaultBallAttributeTable;
  - BP_*TrajectoryModifier* (each touch's values);
  - the kick ability data assets (which touch uses which override, with bounce ranges and weights);
  - BallInteractionWithPhysicalMaterial;
  - ToRollMaxAngleBySpeedCurve.
- **Native code (Rematch build of 2026-10-24):**

| Function | What it is |
|---|---|
| `FUN_145364c70` | sums the modifiers' accelerations, applies weights and corrections |
| `FUN_145372ea0` | builds the trajectory and keeps the rising and falling timers |
| `FUN_14536a450` / `FUN_145369b30` | gravity |
| `FUN_14536a150` | air drag |
| `FUN_1453698a0` | override or attribute coefficient |
| `FUN_14536a6c0` | Magnus: forced spin, fade, force |
| `FUN_145379140` | spin to force, with the rolling rotation |
| `FUN_14536a320` | friction |
| `FUN_1452f5370`, `FUN_1452f5220`, `FUN_1452f50d0`, `FUN_1452f51b0` | the modifiers' constructors (default values) |

- **Constants read from the exe:**
  - the drag factor 2.2808·10⁻⁴;
  - the Magnus factor 7.986·10⁻⁴;
  - the 90° rolling rotation.
