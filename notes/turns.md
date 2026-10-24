# Turn transitions (spec)

How the game handles a sharp change of direction while you run: when a turn starts, what happens to your speed, direction and body during it, when another input can cut in, and how it ends. Speeds, start and stop times are in [mechanics.md](mechanics.md) §2.

**Status:** reviewed by Kev and coded in the prototype (2026-10-24); the start transitions and the 180 turn's fixed side are not modelled. Interpreted from the native code and the locomotion data. Answers a pro player's report that alternating W and D quickly makes the prototype almost stop, which the game doesn't do.

## In short
- A turn is a short **root-motion move** with two phases in a row: a **brake** along your old velocity, then a **re-acceleration** along the new direction.
- **You never brake to a stop.** The brake only takes you down to a **pivot speed** (85% of top speed for a run turn of about 90°, 60% for a 180°). Then you re-accelerate to **100% of top speed**. The "85% / 60%" numbers are the bottom of the dip, not the exit speed.
- A turn starts when your **movement input** changes by a big angle **within 50 ms (3 frames)**. The game compares inputs with inputs, not the input with your velocity. A slow stick sweep never starts one: you just steer.
- During the brake **nothing can cut in**. Late in the re-acceleration a new turn can.
- In the meantime the exit direction follows the live input at a capped rotation rate.

## 1. When a turn starts
Each frame, while you're on the ground and no stronger root motion is playing (dribbles, dashes, tackles), the transition ability:
1. Builds your **current movement**:
   - direction = the movement input (the input acceleration), not the velocity;
   - speed state = from the input amount × the top speed (V0 idle, V1 walk, V2 run, V3 sprint).
   - A keyboard key is always full input.
2. Compares it with each **recent movement** in a short history, newest first, and keeps the first transition that is valid. The history:
   - holds each distinct input for **0.05 s (3 frames)** after it stops being current (`m_fMovementLifeSpan`);
   - is cut down to the latest entry while any transition plays;
   - drops idle entries while you're running or sprinting.
3. While a turn is playing, the "old" direction is the **turn's current direction** (see §2), not the old input.

A **turn** is a transition where both movements are walk or faster and the angle between the two directions falls in a turn band. The band comes from the active transition table: FreeMoveTransitionDB without the ball, FreemoveBallTransitionDB with it.

**Without the ball:**

| Angle between the two inputs | Band | Run and sprint (V2, V3) | Walk (V1) |
|---|---|---|---|
| under 60° | none | steer | steer |
| 60–70° | 90° | not allowed: steer | steer |
| **70–120°** | **135°** | **turn** (uses the "90°" data) | steer |
| **120–180°** | **180°** | **turn** | turn |

**With the ball:**

| Angle | Band | Run (V2) and walk (V1) | Sprint (V3) |
|---|---|---|---|
| under 30° | none | steer | steer |
| 30–60° | 45° | not allowed: steer | not allowed: steer |
| **60–120°** | **90°** | **turn** | **turn** |
| **120–180°** | **180°** | **turn** | **turn** |

The bands are inclusive, and the first allowed band in order wins (so exactly 60° counts as "under 60°"). When the main band isn't allowed, the code also tries a fallback set of bands. In free move the fallback never gives anything new.

What this means:
- **No minimum speed, no cooldown, no hold time.** The turn starts on the frame the input changes. Your real speed can be low: see "entering slower" in §2.
- **Keyboard and stick run the same code.**
  - Keys jump 90° in one frame, so W→D always turns.
  - W→W+D (45°) never turns, with or without the ball.
  - A stick only turns if you swing it more than 70° (60° with the ball) within 3 frames. A slower sweep is plain steering at the state's turn rate.
- **Other turn conditions.**
  - The angle sets the turn's data, not the input angle itself.
  - A turn also needs an animation for it in the table. The ones the masks allow all have one.
  - With the ball, the 45° band has an animation but is masked out.

## 2. Speed and direction during the turn
The turn data (SCCodeDrivenTurnTransitionDataAsset) is chosen by your speed state **when the turn starts**:
- `m_DeccelData` gives the brake curve and its duration for the band;
- `m_AccelData` gives the re-acceleration curve and its duration;
- `m_fTargetSpeedMultiplier[band]` gives the pivot.

The two phases run **one after the other**. The move lasts brake + re-acceleration.

**Speed:**
- **Brake:** speed = pivot + (entry speed − pivot) × brake curve. The brake curve goes from 1 to 0, so you slide from your entry speed down to the pivot.
- **Re-acceleration:** speed = pivot + (top − pivot) × acceleration curve. The acceleration curve goes from 0 to 1, so you go from the pivot back to full top speed.
- **Pivot** = `m_fTargetSpeedMultiplier` × **top speed**.
  - Top speed is the turn's out speed state's top speed, read again every frame (stamina effects included).
  - It stays that state's top speed even if you let go of the input.
- The entry speed and the old direction are your **actual velocity** at the start.

**Entering slower than top speed** ("catch-up"), when your speed is below the top speed of the state you turn from:
- The game treats you as if you were at top speed, and starts the brake partway through.
- How far: the point where the brake curve gives your speed. If you're already at or below the pivot, it uses all of the brake.
- That skip is capped at **20% of the brake** (`m_fCatchUpMaxRatioPerType`: start 0.7, stop 0.5, turn 0.2).
- Example: a run turn entered at 3 m/s starts about 20% into the brake, at roughly 4.8 m/s, above your real speed.

**Direction:**
- **During the brake the velocity stays on the old direction** (your velocity at the start). This part of the prototype is right.
- The turn also keeps a **current direction**. It starts as **the input at the moment the turn starts**, and the re-acceleration runs along it.
- Every frame the current direction **turns toward your live input**, at a capped rate and only inside the allowed windows below.
- "Away" means further from your old direction. "Back" means toward it.

| Table, band | Rotation rate | Brake window | Re-acceleration window |
|---|---|---|---|
| No ball, 45° / 90° | 270°/s away, **90°/s back** | first 80% | 100% |
| No ball, 135° / 180° | 180°/s both ways | first 50% | 100% |
| With ball, 90° | 270°/s away, 90°/s back | first 60% | first 80% |
| With ball, 135° / 180° | 180°/s both ways | first 40% | first 60% |

Changing the current direction during the brake doesn't move you (you still slide on the old line). It changes where the re-acceleration goes.

## 3. Cutting in
A new input can start a new transition over a running turn only like this ("phase ratio" = time into the current phase / phase length):

| Running turn is in its… | New transition allowed? |
|---|---|
| **Brake** | **Never** (turns, stops, anything) |
| Re-acceleration, 0° band | at once |
| Re-acceleration, 45° / 90° band | after **80%** of the re-acceleration |
| Re-acceleration, 135° / 180° band | after **60%** (**40%** when sprinting) |

A new start is never allowed during a turn.

For other running transitions:
- a **start** can be cut after 30% of it;
- a **stop** can always be turned out of, and restarted after 30%;
- a speed change (run↔sprint) can almost always be cut.

When a new transition is allowed it **replaces** the running one, starting from your current velocity. The new turn's angle is measured from the old turn's current direction.

**Going back to the old direction mid-brake** (W→D→W quickly):
- No new turn starts. You keep sliding along W down to the pivot.
- The current direction swings back toward W at the "back" rate, only within the brake window, then for the whole re-acceleration.
- So you come out between D and W, closer to W the earlier you pressed it back. Then the normal steering (or a new turn, if allowed by then) does the rest.

**Releasing the input mid-turn:**
- The turn carries on: you still re-accelerate to the top speed, and the current direction stops turning.
- A stop can only start once the turn allows a cut-in.

**Cancelled at once** when the ball is gained or lost, lock move starts, or extra effort starts (GA_Transition's cancel tags).

## 4. How it ends
- The turn ends when the brake plus the re-acceleration (minus any catch-up skip) has played.
- The root motion ends with "keep the last velocity":
  - you leave at **full top speed along the current direction**;
  - normal free movement takes over from there (steering at the state's turn rate, normal acceleration).
- There is no blend.

## 5. Facing
- The body turns from your yaw when the turn starts to the yaw of the **current direction**.
- This takes **min(0.2 s, brake length − skip)**, eased by an S-shaped curve (`m_TurnRotationAlphaRemapping`, flat at both ends).
- After that it tracks the current direction for the rest of the turn.
- So you **face the new way while still sliding the old way**, and the body has finished turning when the re-acceleration starts.
- For 180° turns the turning side (left or right) is fixed when the turn starts, even if the shortest way flips later.

## 6. Values by state
Frames at 60 fps. The "pivot" is the lowest speed in the turn. Top speeds are from mechanics.md §2.

| State (data) | Band | Brake | Re-acceleration | Pivot |
|---|---|---|---|---|
| Run, 5 m/s (V2TurnDataAsset) | 135° (inputs 70–120° apart) | 0.14 s (8.4 f) | 0.13 s (7.8 f) | 85% = **4.25 m/s** |
| | 180° | 0.30 s (18 f) | 0.30 s (18 f) | 60% = 3.0 m/s |
| Sprint, 8 m/s (V3TurnDataAsset) | 135° | 0.165 s (9.9 f) | 0.35 s (21 f) | 70% = 5.6 m/s |
| | 180° | 0.40 s (24 f) | 0.35 s (21 f) | 50% = 4.0 m/s |
| Run with ball, 4.5 m/s (V2BallTurnDataAsset) | 90° (60–120°) | 0.25 s (15 f) | 0.10 s (6 f) | 55% = 2.48 m/s |
| | 180° | 0.35 s (21 f) | 0.32 s (19.2 f) | 55% = 2.48 m/s |
| Sprint with ball, 6.5 m/s (V3BallTurnDataAsset) | 90° | 0.32 s (19.2 f) | 0.25 s (15 f) | 70% = 4.55 m/s |
| | 180° | 0.50 s (30 f) | 0.30 s (18 f) | 45% = 2.93 m/s |
| Sprint after a tackle or an ability (V3RecoveryTurnDataAsset) | 135° | 0.17 s | 0.30 s | 80% = 6.4 m/s |
| | 180° | 0.25 s | 0.30 s | 60% = 4.8 m/s |

- Walking uses the run data (V2 assets) with the walk's top speed.
- The curves:
  - run brake: eases out (half its drop happens in the first third);
  - run re-acceleration: S-shaped;
  - with-ball curves: linear.

## Worked example: W, D, W, D while running
Running north at 5 m/s, no ball. You press D, then W again 0.1 s later, and so on:

| Time | What happens |
|---|---|
| 0 | D: inputs 90° apart, so a 135° band turn starts. You brake along north from 5 to 4.25 m/s over 8 frames, about 0.6 m of slide. The body turns to face east. |
| 0.10 s | W: can't cut in (brake). The brake is 71% done, past its 50% window, so the exit direction stays east. |
| 0.14 s | Re-acceleration along east, turning toward north at 180°/s, 4.25 → 5 m/s. |
| 0.20 s | D again: the exit direction is about 11° off east, so it just turns back toward east. |
| 0.27 s | The turn ends at 5 m/s, heading roughly east. |

The speed never drops below **4.25 m/s**. Extra presses during a brake don't start new turns, they only bend the exit. The zigzag costs a little ground per switch, never your momentum.

## Prototype differences
The prototype is `game/src/player.rs` (player_move, `Turn`) and `game/src/tuning.rs` (`turn_data`, `turn_bucket`).

| | Prototype | Game | Effect in play |
|---|---|---|---|
| Speed in the brake | entry → **0** | entry → **pivot** (85% run, 70% sprint, 55% with ball, 45–60% for 180°) | **W/D alternation nearly stops you** in the prototype. In the game you stay at ≥ 4.25 m/s. |
| Speed in the re-acceleration | 0 → top × multiplier | pivot → **100%** top (read every frame) | The prototype leaves every turn 15–45% slow and needs extra time to recover |
| What the multiplier means | exit speed | the bottom of the dip | Same data, wrong role |
| Trigger | angle between **velocity** and input; any frame with speed > 1 m/s | angle between the **input now and the input up to 50 ms ago** (or the running turn's direction); walk or faster by input | The prototype turns on slow stick sweeps and re-triggers off leftover velocity angles |
| Bands | <30 none, 30–60 → 45, 60–120 → 90, ≥120 → 180 | no ball: <70 none, 70–120 → "90° data", ≥120 → 180; ball: <60 none, 60–120 → 90, ≥120 → 180; walk without ball: 180 only | With the ball the prototype runs 45° turns the game never does (W→W+D dips the speed) |
| New input in the brake | replaces the target direction, instantly, until the brake ends | the target direction turns toward it at 270/90°/s or 180°/s, only in the first 80% / 50% of the brake (ball: 60% / 40%) | The prototype allows instant cuts back. The game bends the exit gradually. |
| New input in the re-acceleration | ignored (direction fixed) | keeps turning toward the input at the same rates (ball: first 80% / 60% only) | The prototype's exit is stiff |
| Cutting in | nothing until the turn ends, then a new turn at once | never in the brake. In the re-acceleration after 80% (90° band) or 60% (135/180°, 40% sprinting). Measured from the turn's direction. | Similar gating, but the game can chain earlier and from the right reference |
| Entering slower than top speed | brakes from the real speed | starts up to 20% into the brake as if at top speed | Turning out of a start is snappier in the game |
| Facing | 720°/s toward the target from the start | S-curve over min(0.2 s, brake) to the current direction, then tracks it | Close enough. The game finishes the body turn exactly at the pivot. |
| End | hands back at top × multiplier | hands back at top speed along the current direction | |
| Cancel | other movement clears it | also on ball gained or lost, lock move, extra effort | |

**To match the game:**
1. Store a pivot = multiplier × top. Brake: `from × (pivot + (entry − pivot) × stop(t))`. Re-accelerate: `cur × (pivot + (top − pivot) × start(t))`, with top read every frame.
2. Keep a `cur` direction: set it to the input when the turn starts. Each frame, inside the windows above, turn it toward the input by rate × dt. Use the "away" rate when the input is on the far side from `from` and the "back" rate otherwise.
3. Trigger on input changes:
   - keep the last 50 ms of distinct input directions;
   - test the angle from each (newest first) to the current input;
   - while a turn plays, test the angle between `cur` and the input instead, and only once the cut-in ratio is reached.
4. Use the band tables above, chosen by ball or no ball, and take the data by the speed state at the start.
5. Catch-up: if the entry speed is below top, set entry = top and start the brake at min(the point where the brake curve gives your real speed, 0.2) of its length.
6. Facing: yaw from the yaw at the start to `cur`, eased, over min(0.2 s, the brake left), then follow `cur`.

## Open questions
1. **The live input the turn reads.**
   - Each frame the turn reads its target direction from a per-move input store on the character (synced for prediction).
   - Whether that input is raw or filtered (dead zone, smoothing) decides how fast a stick must swing to start a turn.
2. **The catch-up speed jump.** Entering a turn slower than top speed jumps you up to near the pivot (e.g. 3 → ~4.8 m/s on a run turn), which is worth checking in the game. It rests on the reference speed being the state's top speed.
3. **The second input source.** The movement description can take an alternative input vector at full strength when a flag on the movement component is set; when that happens is unknown.
4. **The angle for left or right.** The minimum angle that sets the left or right side of a 180° turn was not read.
5. **Speed states from the stick.** The input thresholds between walk, run and sprint (SpeedDescriptionDB) decide which data a half-pushed stick gets; not covered here.

## Sources
**Native code** (decompiled locally; several of these are SCCore functions at 0x144A00000–0x144B50000, outside the dumped game ranges, read from the Ghidra project):
- Detection:
  - `AbilityTask_ComputeTransition__vf91` (1448f1e30): the per-frame test of the history against the current movement.
  - FUN_1448f7620: the history, with its 0.05 s life and the trims.
  - FUN_1448f5840 and FUN_1448ddff0: one entry; the first valid candidate wins.
  - FUN_144b3d040 and FUN_144b3d660: the movement description, from the input acceleration and its amount.
- Candidates:
  - FUN_144b3dd80, FUN_144b3f3e0 and FUN_144b3a700: main bands then fallback; the running turn's direction is the "in" direction.
  - FUN_144a463d0 and FUN_144a665f0: the angle to a band, first allowed range in order.
  - FUN_144a4b110: validity (allowed types, `m_uiAllowedTurns` per speed state, an animation is needed).
- Cutting in: FUN_1448e4f10 and FUN_144b41f20 (per-phase ratio and the brake flag, then the table's BPE_IsTransitionAllowed).
- Settings:
  - FUN_144a57ee0 and FUN_144a52e90: the turn asset by speed state, brake and re-acceleration by band, the multiplier.
  - FUN_1448e37f0: entry speed and direction from the velocity, the yaw, the catch-up.
  - FUN_144a01ab0: inverse curve lookup.
- The move:
  - `AbilityTask_ApplyRootMotionTransition_Base__vf101` (1448ef9b0): creates the root motion source, with its duration, rotation rates and windows. The current direction starts at the input for turns and starts.
  - `AbilityTask_ApplyRootMotionTransition__vf104` (1448ed390): target direction and top speed every frame.
- RootMotionSource_Transition (vtable 1476aa988):
  - FUN_1448ec5f0 (PrepareRootMotion: speed formula, direction by phase, rotation toward the input, facing);
  - FUN_1448e4e90 (in brake);
  - FUN_1448e2db0 (transition type).
- SCTransitionDB constructor FUN_14486f550: catch-up 0.7 / 0.5 / 0.2, max rotation time 0.2 s, default bands 22.5° apart.

**Data:**
- FreeMoveTransitionDB: defaults, plus its Blueprint functions BPE_IsTransitionAllowed, BPF_GetVelocityAngularSpeed and BPF_GetModifyTransitionAllowedIntervals.
- FreemoveBallTransitionDB (child of FreeMoveTransitionDB), FreeMoveAfterTackleTransitionDB and FreeMoveRecoveryTransitionDB.
- V2, V3, V2Ball, V3Ball and V3Recovery TurnDataAsset, with their Turn…Stop and Turn…Start assets and curves.
- GA_Transition: Transition Settings and CancelAnyTransitionOnTagsChanged.
