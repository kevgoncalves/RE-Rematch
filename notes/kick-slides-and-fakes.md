# Kick slides and fake shots: how Rematch moves you

Research for the prototype's kicks and fake shot. Kev says both are wrong; this maps what the game
does. Rules and numbers come from the extracted data, the native code and Kev's two practice
recordings. Everything is written in our own words; no code or animation data was copied.

Tags used below:
- **[data]** read from an asset;
- **[native]** read from the game's compiled code (function names are from our own Ghidra naming);
- **[root]** measured from an animation's root-bone track (decoded locally, only numbers kept);
- **[clip]** seen in Kev's recordings;
- **[inferred]** our reading, not proven.

60 frames = 1 s. Speeds in m/s. "Entry speed" means your flat speed when the move starts.

---

## 0. The one mechanism behind both: root motion with its speed replaced

Every kick and the fake shot play an animation whose root motion moves you. A
`ScaleSpeedRootMotionModifier` on the animation keeps the **direction** of that root motion but
replaces its **speed** [native: `ScaleSpeedRootMotionModifier__vf95`, `__vf101`, `__vf102`,
`FUN_144931e70` builds the speed table, `FUN_1449318a0` evaluates one curve, constructor
`FUN_1447c02a0`].

1. **At the start** it stores your flat speed, **capped at 10 m/s** (`m_fMaxInitialSpeed` = 1000 cm/s
   by default; no kick or fake asset changes it).
2. **The animation is cut into tracks.** Kicks use two: from the start to the touch, and from the
   touch to the move-interrupt point. The fake uses one track for the whole animation.
3. **Each track picks one curve:**
   - The data lists conditional curves, each with a speed test (≥, ≤, <, …) against the stored
     speed, and optionally a gameplay-tag test (for example "is sprinting").
   - The first one that passes wins; otherwise the default curve is used.
4. **The curve is stretched over the track.** The curve's last key lands on the track's end, so a
   curve that ends at 0.4 or 0.8 still spans the whole track.
5. **The speed at each moment is entry speed × curve value** for the usual "keep entry" curves.
   - The first track's entry is your stored speed.
   - **A second track starts from the first track's last speed**, not from your speed again.
   - A curve with no keys and a default below zero means "no override": the animation's own root
     speed is used.
   - A curve not marked as a percentage gives an absolute speed in cm/s.
6. **Direction:**
   - Each frame you move along the animation's own root direction at the table's speed.
   - When the animation's root doesn't move that frame, you keep going the way you were moving
     when it started, turned with your body [native `__vf101`, inferred for the fallback angle].
7. **Sampling:** the table is sampled every 1/60 s for kicks (`m_fStep` 0.0167 in their metadata)
   and every 0.03 s otherwise (the constructor's default).
8. **Holding the speed:** a flag, `m_bConserveLastModifiedSpeedUpToAnimEnd`, keeps the last speed
   to the animation's end.

**What this means in practice:** a kick or a fake carries your momentum. How far you slide is
almost entirely set by **your speed when the move starts**, shaped by a short curve. The animation
only says which way.

---

## 1. Fake shot

### 1.1 Trigger, window, cost

- **Trigger:** the pass button during a shot charge. The shot ability listens for `FakeShootInput`
  as an "input that syncs earlier" with a time of **0.33 s** [data: `GA_Shoot` defaults
  `FakeInputToSyncEarlier`].
  - [clip] Every fake Kev pressed 0.20–0.37 s into the charge worked (27 presses).
  - The prototype's note has 0.50–0.57 s never faking.
  - The exact cut-off is between 0.37 and 0.50 s; the data's 0.33 s is probably it plus a frame
    of lag or network margin [inferred].
- **A fake pressed while the shot is still waiting on a control did not fake** [clip, one case,
  V2 280.93: control ring at the press, then the shot went off].
- **Stamina cost** [data: `CT_ShootAbilitiesCost_Stamina` row `Cost_FakeShoot`; `GA_Shoot`]:
  - The first fake costs **1.5**.
  - Each fake puts a marker on you (`GE_FakeShoot`). It lasts **3 s** and is removed as soon as you
    lose the ball. The effect itself is set to never expire; the 3 s comes from its
    predictable-duration part, which we take to be the one that counts [inferred].
  - A fake while that marker is on costs **4.0**. The cost level is 1 + markers, and the marker
    stacks to 1, so every later fake within 3 s with the same ball costs 4.0.
  - The cost effect also applies the "long" regen pause, **1.36 s** (`GE_Cooldown_Regen_Long`).
- **Tired fake:** if you can't pay that cost, or you are out of stamina (`Effects.OutOfStam`), you
  get the **tired fake** instead [data: `GA_Shoot.GenerateManifest`, `IsExhaustedFake`].

### 1.2 Which fake plays

The fake picker (`BP_FakeShoot_MontagePicker`) works in two layers [data].

**Layer 1: standing or moving, and the shot's direction against your body.** It reuses the shot
build-up picker (`BP_BuildUpShootMontagePicker`):
- **Standing:** at **≤ 1.5 m/s** you get the standing set. A transition-type check also feeds this
  test; its meaning is unconfirmed.
- **Moving:** above that you get the moving set.
- **Shot direction:** the angle between your body and the camera (the shot's aim) picks a family.

| Shot vs body (moving; signed angle, "East" side by the slot names) | Family used by the fake |
|---|---|
| within ±30° | **North** (straight) |
| +30° to +120° ("East") | **West**, mirrored |
| −30° to −120° ("West") | **West** |
| +100° to +150° ("South East") | **West**, mirrored |
| −150° to −85° ("South West") | **West** |
| beyond ±150° | **SouthWest** (mirrored on one side; lock move has its own slot) |

Which side "East" is on screen isn't proven; it doesn't change the result, because mirroring
always turns the side fake to your right (below).

**Layer 2: the movement keys at the fake press.**
- **No direction held:** a "neutral input" condition picks the **stop** fake.
- **A direction held:** an input-angle picker splits the stick into sectors measured from the
  camera's forward, in steps of 45°, 90°, 90°, 90°, with the rest wrapping back to forward
  [native: `InputAngleAnimationPicker__vf97`].
  - Each step's upper edge is included up to 180° and excluded beyond it.
  - With the recordings' direction (W 0°, A 90°, S 180°, D 270°; see 1.6):

| Keys at the fake press | Angle | Moving fake | Its motion |
|---|---|---|---|
| W, W+A, W+D | 0°, 45°, 315° | **Forward** (`BtoN`) | keeps running forward with the ball |
| A, A+S | 90°, 135° | **Turn-back** (`BtoS`) | spins 180° and comes back |
| S | 180° | **Turn-back** (`BtoS`) | same |
| D, D+S | 270°, 225° | **Side** (`BtoE`, or `BtoW` mirrored) | roulette, turns 90° right and runs right |
| none | — | **Stop** (`BtoNeutral`) | a step forward, then stops over the ball |

- **No left roulette while moving.** In every reachable slot, the side sector gives a move to your
  right, after mirroring. Both the left sector and the back sector give the same turn-back.
  - A left-going animation exists (`FakeShoot_V2_North_BtoW`), but the picker never uses it.
  - This contradicts Kev's "to the sides it does a roulette to that side" for the left side. See
    open question 1.
- **Standing fakes ignore the keys.** They depend only on the shot's direction:
  - North: `V0_North_BtoN`;
  - East: `BtoE`;
  - West: `BtoW`;
  - South-east and south-west: `BtoSW` (mirrored for south-east).
- **Tired fakes ignore the keys too:**
  - moving: always a forward fake (`ExhaustedFakeShoot_V2_*_BtoN`);
  - standing: like the normal standing set.

### 1.3 Each variant's motion and timing

[data: the fake montages, decoded locally; root: their root tracks]

- The montages skip the animation's first 8–12 frames (they start 0.13–0.2 s in).
- They blend in over 0.05–0.1 s and blend out over 0.2–0.3 s.
- **The leg passes over the ball about 0.2 s after the press:** the "Ono" voice and the whoosh fire
  0.35 s into each animation.
- "Can act" is the frame where every action is allowed again (`GenericAll`).
  - Some montages label their layer "GenericAll_ButShoot", but all of them point to `GenericAll`.
  - That layer also lets the stick interrupt the move, with a 30° angle override whose exact meaning
    is open.

| Variant (North family) | Montage length | Can act | Root path (own speed) |
|---|---|---|---|
| Forward `BtoN` | 1.83 s | 34f (0.567 s) | straight on, about 4.5 m/s, 8 m over the montage |
| Side `BtoE` | 1.62 s | 34f | 0.4 m forward, then turns **+90°** (right) between 0.18 and 0.28 s, then right at 4.3 m/s (6.5 m) |
| Turn-back `BtoS` | 0.83 s | 34f | 0.6 m forward, turns **180°** via the right (+74° at 0.2 s, 180° at 0.3 s), then back 1.7 m |
| Stop `BtoNeutral` | 0.83 s | **20f** (0.333 s) | 0.75 m forward in 0.2 s, then still |

The West and SouthWest families have the same shapes but shorter:
- Forward: 1.33–1.5 s.
- Side: 0.98–1.05 s, turning 77–90°.
- Turn-back: 0.97–1.45 s. **The unmirrored West and SouthWest turn-backs spin via the left** (−108°
  and −45° at 0.2 s); the mirrored ones spin via the right.
- Stop: 0.72–0.98 s.

Standing and tired:
- **Standing fakes:** no travel.
  - Straight: none.
  - East and West: a 39° twist toward the shot.
  - South-west: a 118° turn with a 0.6 m step.
  - Can act at **20f**.
- **Tired fakes:**
  - moving: forward at 3–5 m/s on their own root motion, can act at **50f**;
  - standing: can act at **40f**.

### 1.4 How far the fake slides: entry speed × curve

[data: `DT_FakeShoot` rows and their speed data; native rules from §0]

| Row | Used by | Speed curve over the whole animation |
|---|---|---|
| `VX_Default` | forward and side fakes | default: **your entry speed, unchanged**. Sprinting at ≥ 2 m/s: rises linearly to **×1.25** by the end. At ≤ 2.25 m/s: a burst to **×2.0** at 18% of the animation, back to ×1.0 at the end |
| `VX_Default_BToS` | turn-back fakes | default: dips to **×0.59** at 31%, back to ×1.0 by 61%. Sprinting at ≥ 2 m/s: dips to ×0.85 at 31%, then climbs to **×2.0** at the end. At ≤ 2.25 m/s: ×0.7 at 11%, **×2.0** at 89%, ×1.0 at the end |
| `VX_Default_BToNeutral` | stop fakes | none: the animation's own short step |
| `V0_North_Default`, `*_Exhausted` | standing and tired fakes | none |

Where you are when you can act again (34f), and at the montage's end, by entry speed. These are
the North family's root paths integrated with the curves [root + data]:

| Entry | Forward fake | Side fake (right) | Turn-back fake | Stop fake |
|---|---|---|---|---|
| 2.0 m/s (slow burst) | 2.1 m ahead; 5.3 m by the end | 1.9 m right, 0.65 m ahead | 0.3 m back | 0.73 m ahead, then stopped |
| 4.5 m/s (running with ball) | 2.5 m ahead; 8.2 m by the end | **2.25 m right**, 0.8 m ahead; 6.9 m right by the end | 0.2 m back; 1.0 m back by the end | same |
| 6.5 m/s (sprinting with ball) | 3.9 m ahead | **3.4 m right**, 1.25 m ahead | 0.6 m back; 2.2 m back by the end | same |

- The West and SouthWest side fakes come out at 1.4–2.1 m to the right at 4.5 m/s.
- Their turn-backs at 0.9 m back (West) or still 0.7 m ahead (SouthWest).

**Answer to Kev's guess:**
- **Momentum: yes, directly.** The fake keeps your speed when it starts, up to 10 m/s, for its
  whole length, along the chosen path.
- **The shot's ball speed: not directly.** No fake curve reads the charge.
  - The link is indirect: the shot's wind-up has already been slowing you (×1 → ×0.7 over the
    wind-up, §2.2).
  - So a fake later in the charge starts slower and slides less. At 0.33 s into a 32-frame power
    wind-up you are at about ×0.9 of your run-up, ×0.72 if sprinting, ×0.58 out of a dribble
    [data: the shot curves, evaluated].

### 1.5 Costs and when you can act, all together

| | Moving, direction held | Moving, no direction | Standing | Tired, moving | Tired, standing |
|---|---|---|---|---|---|
| Can act | 34f (0.567 s) | 20f (0.333 s) | 20f | 50f (0.833 s) | 40f (0.667 s) |
| Stamina | 1.5, or 4.0 within 3 s of a fake with the same ball | same | same | (could not pay) | (could not pay) |

After "can act", any action is allowed and the stick can take over (the move interrupt).

### 1.6 What the recordings show

[clip: `Videos/2026-10-25 02-55-47.mkv` 249–298 s, `02-33-58.mkv` 325–339 s; keys from
`obs1_keysfix.json` and `obs2_keysfix.json`]

- **The presses:**
  - 27 fake presses, all 0.20–0.37 s into a shot charge, nearly always with A, D, A+S or D+S held.
  - There are no clean S-only or no-key fakes in these clips.
- **W fake (V1 328.73):** the orange rings show 0.1 s after the press as the leg passes over the
  ball. Then he simply keeps running forward with the ball. That is the forward fake.
- **D fake (V2 271.50):** the rings show at about +0.2 s. The body spins, and the camera, following
  him, slides right until the touchline that was off-screen on the right crosses toward the middle
  by +0.8 s. **He travelled to the right.**
- **A fake (V2 251.90):** the rings show at about +0.3 s.
  - The body spins round to face the camera by +0.6 s.
  - No sideways travel is visible: the goal and the stands stay put.
  - At +0.87 s Kev presses W+D and he turns away again.
  - **This matches the turn-back, not a left roulette.**
- **A+S and D+S fakes:** similar spins. **Their direction can't be read reliably:**
  - The fake has its own camera transition (`InFakeShoot_CameraTransition`, 1.4 s) and a velocity
    offset (`FakeShoot_CameraData`), so the camera moves on its own.
  - For the same reason, slide distances could not be measured from these clips.
- **Conclusion for the side mapping:**
  - The D fake travelling right fixes the sector direction as W 0°, A 90°, S 180°, D 270°.
  - The dash and dribble pickers seem to say the opposite. Those moves are probably steered toward
    the stick after they are picked, so the side their animation goes doesn't show the convention
    [inferred].

### 1.7 The prototype now

`Player.fake`, the fake-shot block in `kicking`, `top *= FAKE_SHOT_SPEED` in `player_move`, and the
`FAKE_SHOT_*` constants:
- **One fake for every key:**
  - the leg swings, then you crouch;
  - you keep normal steering at **35% of top speed** (`FAKE_SHOT_SPEED`);
  - the game has no free steering until "can act", and keeps **100% of your entry speed** along a
    fixed path.
- **Missing pieces:**
  - no forward, side, turn-back or stop variants;
  - no standing or tired variants with their own motion;
  - no speed curves.
- **Locks:** `FAKE_SHOT_LOCK` matches the game (0.33, 0.57, 0.67, 0.83 s), except that a moving fake
  with no key held can act at 0.33 s, not 0.57.
- **Cost:** a flat 1.5. The 4.0 repeat cost and the regen pause are missing.
- **Window:** `FAKE_SHOT_WINDOW` 0.45 s fits the recordings (the data says 0.33 s).

---

## 2. Kick slides (passes, lobs, shots, low-power kicks)

### 2.1 Passes and lobs

[data: `BP_TapAnimationMetadata`, `BP_LobTapAnimMetadata`, `BeforeTapScaleSpeedData`,
`TapAfterShootSpeedData`, `ShootDataTable`, the tap and lob pickers]

- **Standing below 2 m/s:**
  - The picker plays the standing pass at **≤ 2 m/s** (`ActorVelocityTargetDataCondition` ≤ 200).
  - The wind-up speed is pinned to an absolute 1 cm/s: **no slide**.
  - After the touch, the standing animation's own root motion.
- **Moving above 2 m/s:**
  - **Wind-up**, from the press to the touch: **100% of your entry speed** (a flat curve ×1.0).
  - The touch comes at **24f**; short pass and short lob at 16f.
  - **After the touch**: still **×1.0**, on to the move interrupt. "Can act" is at **39f**; the
    backwards lob at 42f.
  - The pass data then **keeps that speed to the animation's end**, about 1.0–1.07 s after the press
    (the hold-to-end flag), unless the stick or an action interrupts after 39f.
  - The direction is the pass animation's own: forward for `BtoN`, angled for the NE, NW and S
    variants.
- **Freeze at the touch:** 3f (short ones 2f).

### 2.2 Shots

[data: `BP_HighPowerShootMetadata`, `BP_LowPowerShootMetadata`,
`HighPowerBeforeShootScaleSpeedData` with its external curves `V2ToShootSpeedScale`,
`V3ToShootSpeedScale` and `ShootAfterDribbleSpeedScale`, `HighPowerAfterShootScaleSpeedData`,
`ShootDataTable`]

- **Standing below 1.5 m/s:** the standing shot set, with no speed scaling.
- **Wind-up** (from the press to the touch at **32f** for power shots, **20f** for low-power ones;
  the split is at 29.41 m/s of ball speed): **entry × a falling curve** (cubic):

| Case (first match wins) | Halfway through the wind-up | At the touch | Shape |
|---|---|---|---|
| After a dribble (tag `ShootAfterDribble`) | ×0.62 | **×0.5** | drops early |
| Sprinting and ≥ 4 m/s | ×0.73 | **×0.6** | drops early |
| Otherwise | ×0.94 | **×0.7** | holds, then drops late |

  The default's own keys say 0.75, but its external curve, 0.7, overrides them.

- **After the touch, to the move interrupt** (can act moving at **45f**):
  - The speed starts from the touch speed.
  - It eases up to **×2.0 of it**, a smooth S-curve from ×1 to ×2.
  - Then the animation's own root motion takes over (about 4.5–5.9 m/s forward in
    `Shoot_HighPower_V2_BtoN`) until the stick or an action.
- **Freeze at the touch:** 5f (power) or 3f (low power).

Distances from the press, power shot [data + native rules; tables computed in the scratchpad's
`slidecalc.py`]:

| Entry | To the touch (speed there) | Touch → 45f (speed at 45f) |
|---|---|---|
| Running with ball, 4.5 m/s | 2.19 m (3.15 m/s) | +1.00 m (6.3 m/s) |
| Sprinting with ball, 6.5 m/s | 2.63 m (3.9) | +1.23 m (7.8) |
| Out of a dribble, 6.8 m/s | 2.44 m (3.4) | +1.08 m (6.8) |
| Just controlled from a sprint, 8.0 m/s | 3.89 m (5.6) | +1.77 m (11.2) |

So a shot doesn't brake you for good: you dip to 50–70% at the strike, then lunge back up to
100–140% of your run-up by "can act". That is the forward carry Kev calls the drag.

### 2.3 Why Kev's four cases slide more and more

The pass keeps 100% of your speed when its animation starts, so the order is simply **the speed
each case hands to the pass**:

| Case | Speed carried into the pass | Why | Pass slide (to the touch + touch → 39f) |
|---|---|---|---|
| Pass while running | 4.5 (6.5 sprinting) | your with-ball run | 1.8 + 1.1 m (2.6 + 1.6 m) |
| Pass while dribbling | **up to 6.5–6.8** | the dribble's burst speed (mechanics.md §2) is your speed at the press | 2.7 + 1.7 m |
| Pass right as you control | **up to 8** | the control keeps your *no-ball* speed (below) | 3.2 + 2.0 m |
| Push ball → dash to it → pass | **up to 8.5** (10 cap) | the dash's peak speed is what the pass starts with | 3.4 + 2.1 m |

- **The control keeps your speed** [data: `BP_BallControlAnimMetadata` → `BallControlSpeedScaleModifier`]:
  - A moving control runs your entry speed through ×1 → **×0.9** at 23% → ×1.0 from 50%.
  - At ≥ 8 m/s it switches to an absolute **8 → 7.2 → 8 m/s**.
  - You can act at 14f (feet).
  - So if you were sprinting without the ball (8.75 m/s), you are still at about 7.2–8 m/s when a
    buffered pass starts.
  - Running with the ball would have held you to 4.5 or 6.5. **This, not a skipped wind-up, is why
    passing out of a control slides further** [inferred from the data; Kev's buffering hunch is half
    right: the buffer lets the pass start before you slow down].
- **Does the control's own movement carry?** Yes: the control's speed *is* the speed the pass
  inherits.
  - From a lock-move or standing control the curve dips to ×0.4 between 23% and 40%
    (`LockmoveBallControlSpeedScaleModifier`), so you inherit much less.
- **The buffer and pre-charge do not start the pass mid-move.**
  - The pass's wind-up configuration (`BP_BuildUp_Tap_Config`) starts at 0.
  - What carries over is the **charge**: shots, analog passes and lobs count the time the button
    sat on the input stack (`InputShootForceComputation.m_bUseTimeSpentOnInputStack` = true in
    `BP_ShootAbilityData`, `BP_TapAbilityData_AnalogPower` and `BP_LobAbilityData_AnalogPower`).
  - The lob's wind-up configuration (`Bp_TapBuildUp_PlayerConfiguration`) is set to "start from
    context", so a buffered lob *may* start part-way in. See the open questions.
- **Push ball + dash:**
  - The dash peaks at 8.5 m/s (mechanics.md §8; the prototype's `DASH_SPEED` peaks at 7.5).
  - A pass that starts as the dash picks up the ball inherits that speed, capped at 10.
  - A full dash control (`DashBallControlSpeedScaleModifier`), however, blends you down toward the
    animation's own speed. It keeps 50% of your entry at 8% and 30% from 50%; below 5.3 m/s it
    leaves the animation's speed alone.
  - So the pass has to start before that control slows you (open question 6).
- **Lobs and shots follow the same rule.** Lobs use the pass curves. Shots use their own
  (×0.5–0.7 at the touch, then ×2 after it), still scaled by the speed carried in.

### 2.4 The prototype now

- **`pass_drag`:**
  - **Same idea:** it keeps your velocity from the press through the wind-up, and `PASS_SLIDE` /
    `PASS_AFTER_TOUCH` hold it after the touch.
  - Out of a dribble it uses **`BALL_RUN_SPEED` (4.5)** instead of the dribble's real speed (up to
    6.8). This flattens Kev's second case.
  - **Minimum:** `PASS_DRAG_MIN_SPEED` is 0.3 m/s; the game's standing pass covers ≤ 2 m/s.
  - No 10 m/s cap.
- **The shot slide in `player_move`:** entry × a straight line from 1 to **0.95, 0.9 or 0.9**
  (`SHOT_SLIDE*`), always over 32 frames.
  - The game uses 0.7, 0.6 or 0.5 on a curve, over the real touch frame (32f or 20f).
  - After the touch the prototype runs at the touch speed for 25f. The game ramps it ×1 → ×2 up to
    45f, then uses its own root motion.
  - Kev tuned 0.95 to avoid "braking on every kick". The game's after-touch ramp is what removes
    that braking feel, so with the ramp in, the wind-up dip can follow the data.
- **The control:** while `receiving`, once the ball is yours at 12f, `player_move` pulls you down to
  the with-ball top speed at 15 m/s² (`OVERSPEED_DECEL`).
  - A queued kick (`p.queued`) therefore starts slower than in the game, and **Kev's third case is
    missing**.
  - The game holds 90–100% of your speed, capped at 8 m/s, through the control.
- **`KICK_TURN_FREE`** (15f after the touch) is close to the game's "can act" (pass 15f after its
  24f touch; power shot 13f after its 32f touch).

---

## 3. Implementation plan (after the kick-code work merges)

### 3.1 New and changed constants (`tuning.rs`)

```text
KICK_ENTRY_CAP            = 10.0      // every scripted move's entry speed is capped here
PASS_SLIDE_MIN_ENTRY      = 2.0       // replaces PASS_DRAG_MIN_SPEED 0.3 (standing pass at or below)
PASS_KEEP_TO              = 64/60     // a pass keeps its speed to its animation's end unless steered
SHOT_WINDUP               = [(0,1.0),(0.5,0.94),(1,0.7)]   // replaces SHOT_SLIDE 0.95 (TUNE: Kev may keep more)
SHOT_WINDUP_SPRINT        = [(0,1.0),(0.5,0.73),(1,0.6)]   // sprinting and >= 4 m/s; replaces SHOT_SLIDE_SPRINT
SHOT_WINDUP_DRIBBLE       = [(0,1.0),(0.5,0.62),(1,0.5)]   // out of a dribble; replaces SHOT_SLIDE_FROM_DRIBBLE
SHOT_SLIDE_SPRINT_MIN     = 4.0
SHOT_AFTER_TOUCH_GAIN     = 2.0       // touch speed x1 -> x2, smooth, until SHOT_MOVE_INTERRUPT
SHOT_MOVE_INTERRUPT       = 45/60     // from the press
CONTROL_SPEED_KEEP        = [(0,1.0),(0.23,0.9),(0.5,1.0),(1,1.0)]  // share of entry through a moving control
CONTROL_SPEED_CAP         = 8.0       // entries at or above: 8 -> 7.2 -> 8 m/s absolute
FAKE_SHOT_COST            = [1.5, 4.0]  // first fake; any fake within FAKE_REPEAT of the last with the same ball
FAKE_REPEAT               = 3.0
FAKE_STANDING_SPEED       = 1.5       // at or below: standing fakes
FAKE_ACT                  = moving 34/60, no key 20/60, standing 20/60, tired moving 50/60, tired standing 40/60
FAKE_SPRINT_MIN = 2.0, FAKE_SLOW_MAX = 2.25
FAKE_SPEED_SPRINT         = [(0,1.0),(1,1.25)]
FAKE_SPEED_SLOW           = [(0,1.0),(0.18,2.0),(1,1.0)]
FAKE_BACK_SPEED           = [(0,1.0),(0.04,1.0),(0.31,0.59),(0.61,1.0),(1,1.0)]
FAKE_BACK_SPEED_SPRINT    = [(0,1.0),(0.04,1.0),(0.31,0.85),(1,2.0)]
FAKE_BACK_SPEED_SLOW      = [(0,1.0),(0.11,0.7),(0.89,2.0),(1,1.0)]
FAKE_LENGTH  (montage s)  = forward 1.83 / 1.50 / 1.33, side 1.62 / 1.05 / 0.98,
                            back 0.83 / 0.97 / 1.45, stop 0.83 / 0.72 / 0.98   (North / West / SouthWest)
FAKE_ANIM_OFFSET          = 0.15      // the speed curve runs over the whole animation, which starts ~0.13-0.2 s before the montage
FAKE_TURN                 = (0.1, 0.3)  // side and back fakes lead straight on, then turn between these times (montage s)
FAKE_STOP_STEP            = 0.75 m over 0.2 s, then still
```

The curves are piecewise-linear stand-ins for the game's cubic ones, close enough to tune.

### 3.2 Fake shot (`player.rs`, `body.rs`)

1. **State:** `Player.fake` becomes a `FakeState`:
   - `t`, `act`, `len`;
   - `kind`: Forward, Side, Back, Stop, Standing or Tired;
   - `family`: Straight, Side or Back shot;
   - `turn_sign` (+1 right, −1 left) for the spin;
   - `entry` (flat velocity, capped);
   - `dir0`, `facing0`;
   - the curve.
2. **`kicking`, at the fake press:**
   - **Kind:**
     - Standing if your flat speed is ≤ 1.5 m/s.
     - Tired if `stamina` can't pay the cost, or you are exhausted.
     - Otherwise the sector of `axes.movement` against the camera (as in the table in 1.2):
       forward within ±45°, Side for D and D+S, Back for A, A+S and S, Stop with no direction.
       Keep a `FAKE_SIDE_ON_D: bool` switch until Kev's test (open question 1).
   - **Family:** from the angle between `p.facing` and the camera yaw: within ±30° Straight,
     30–150° Side, beyond 150° Back. The family sets the lengths and the turn-back's spin side.
   - **Cost and stamina:** take 1.5, or 4.0 if a fake happened within `FAKE_REPEAT` with this ball.
     Track `since_fake` and reset it when the ball is lost. Set `regen_wait` to the long pause.
   - Drop `p.kick` as now.
3. **`player_move`:** replace `top *= FAKE_SHOT_SPEED` with a scripted branch, next to the
   dribble's:
   - u = (t + `FAKE_ANIM_OFFSET`) / (len + `FAKE_ANIM_OFFSET`).
   - speed = |entry| × curve(u). Which curve:
     - sprinting and ≥ 2 m/s: the sprint curve;
     - ≤ 2.25 m/s: the slow curve;
     - otherwise ×1 for forward and side fakes, the back curve for turn-backs;
     - none for Stop, Standing and Tired, which use their own small root paths.
   - way = `dir0` until `FAKE_TURN.0`, then turning to +90° (Side) or 180° (Back) by `FAKE_TURN.1`.
   - The body yaw follows the same turn.
   - From `act` on, any action or a stick within 30° of the current way hands back
     (`soft_hand_back`) at the current velocity. Otherwise run to `len`.
4. **`kicking`:** the early return during the fake now ends at `act`, not at the lock array.
5. **`body.rs` poses, in words:**
   - **Swing** (all fakes, 0 to about 0.2 s): the shot's backswing pose carries on. The kicking
     leg whips through low but stops short: the foot passes *over* the ball and lands beside it
     (the orange rings fire here, as now). The arms stay out wide.
   - **Forward:** the kicking foot lands ahead of the ball, then a normal with-ball running stride,
     body leaning forward. This is a step-over that never loses pace.
   - **Side (right):**
     - The sole of the kicking foot rolls the ball across to the right.
     - The torso spins clockwise (seen from above) past 90°: shoulders square to the original line
       at about 0.25 s, the head turning first.
     - Then a low, wide stride running right, ball on the outside foot.
   - **Back (turn-back):**
     - The sole drags the ball back under the body.
     - The body turns 180° through the spin side (clockwise for straight shots; for side and back
       shots it depends on the mirror), staying low and wide with the arms out for balance.
     - Then a run back the way you came.
   - **Stop:** after the swing, plant the standing foot, sole on the ball, knees bent, upright
     chest; small arms.
   - **Standing:** the swing, then a 40° twist toward the shot side (straight shot: none; backward
     shot: a 120° pivot with one step).
   - **Tired:** the same as forward, but slower and heavier, with a lower chest and arms hanging.

### 3.3 Kick slides (`player.rs`)

1. **`KickState.entry`:**
   - Cap at `KICK_ENTRY_CAP`.
   - For kicks queued in a control, set it at the action frame (as now).
2. **`pass_drag`:**
   - Use the real flat velocity, also out of a dribble: drop the `BALL_RUN_SPEED` override.
   - None at or below `PASS_SLIDE_MIN_ENTRY`.
   - **After the touch:** hold, with no steering, until "can act" (15f after the touch), then keep
     the speed until `PASS_KEEP_TO`; any stick or action ends it. That is the existing `Run` with
     `free_at` = 15/60 and `length` up to the animation's end.
3. **Shot slide in `player_move`:**
   - v = entry × curve(u), with u = elapsed / touch time (32f, or 20f for low power).
   - **The curves:**
     - out of a dribble (dribble or `since_dribble` grace): `SHOT_WINDUP_DRIBBLE` =
       [(0,1),(0.5,0.62),(1,0.5)];
     - sprinting at ≥ 4 m/s: `SHOT_WINDUP_SPRINT` = [(0,1),(0.5,0.73),(1,0.6)];
     - otherwise: `SHOT_WINDUP` = [(0,1),(0.5,0.94),(1,0.7)].
   - These replace the three `SHOT_SLIDE*` end values.
   - Keep the extra-effort case ("faster than you can run with the ball: all of it") as it is.
4. **After the shot's touch** (the `Run` built in `kicking`): speed = touch speed × (1 +
   smoothstep(t / (`SHOT_MOVE_INTERRUPT` − touch))), up to `SHOT_MOVE_INTERRUPT`. Then hold it until
   the stick, as now.
5. **Control (`player_move`, while `p.receiving`):**
   - Hold the speed you came in with × `CONTROL_SPEED_KEEP`.
   - If you came in at ≥ 8 m/s, use the absolute 8 → 7.2 → 8 instead.
   - Skip `OVERSPEED_DECEL` until the action frame, so the queued kick inherits it.
   - Lock-move controls dip to ×0.4 instead.
6. **Dash:**
   - Check `DASH_SPEED` against the 8.5 m/s peak.
   - A kick that starts while a dash carries the ball keeps the dash's speed.
7. **Tests:**
   - A pass out of a dribble keeps the dribble's speed.
   - A pass queued in a control from a no-ball sprint keeps about 7–8 m/s.
   - A walking pass at 2 m/s doesn't slide.
   - A shot is at 0.7× at the touch and 1.4× of its run-up by 45f.
   - The fake picks forward, side, back and stop by the keys.
   - The fake's cost is 4.0 within 3 s.

---

## 4. Open questions

1. **Side mapping of the moving fake.**
   - The data gives a side roulette on one side only, and the same turn-back for the other side and
     for back.
   - The recordings put the roulette on **D** (it travelled right) and the turn-back on **A**.
   - Kev expects a roulette to each side.
   - Test in practice mode, camera behind, mouse still: sprint straight, charge a shot, and fake
     with only A, only D, only S, and no key. Note where you end up after a second.
2. **The fake window:** the data says 0.33 s; presses at 0.37 s faked; is 0.45 s too generous?
3. **Is the sprint tag still on during a charge?** It switches the fake's ×1.25 / ×2.0 curves and
   the shot's ×0.6.
4. **The shot's after-touch ×2.0.** It gives 11 m/s after a control at 8 m/s. Does the move
   interrupt really come at 45f, or earlier?
5. **A fake pressed during a control's queue didn't fake** (one case). Is the fake ignored while a
   shot waits on the input stack?
6. **Push ball + dash + pass:** does the pass start before the dash control's slow-down (blending
   to 30% of entry), or does the dash pick the ball up without a control?
7. **The 30° move-interrupt override on fakes:** the stick must be within 30° of what to interrupt?
8. **Build-up to kick hand-off:** the final kick starts "from context" at the build-up's position.
   Is the entry speed re-read there (a second decay), or carried?
9. **Standing-set test:** what is "transition type 2" in the build-up picker's ≤ 1.5 m/s
   condition?
10. **Buffered lobs:** does a lob's wind-up skip ahead when buffered (its configuration starts "from
    context")?

---

## Sources

- **Fake shot**
  - Pickers: `BP_FakeShoot_MontagePicker`, `BP_Exhausted_FakeShoot_MontagePicker`,
    `BP_BuildUpShootMontagePicker`.
  - Rows and speed data: `DT_FakeShoot`, `FakeShootRow`, `VXFakeShoot_ScaleSpeedData`,
    `VXFakeShoot_BToS_ScaleSpeedData` and its two external curves.
  - Metadata: `BP_FakeShootMetadata`, `BP_V0_FakeShootMetadata`, `BP_ExhaustedFakeShootMetadata`.
  - Effects and cost: `GE_FakeShoot`, `GE_Cost_FakeShoot`, `CT_ShootAbilitiesCost_Stamina`.
  - Ability (decoded Blueprint): `GA_Shoot` (defaults, `GenerateManifest`).
  - Camera: `FakeShoot_CameraData`, `InFakeShoot_CameraTransition`.
  - The 52 `Player_*FakeShoot_*` montages and animations, extracted locally for their rows,
    lengths, start offsets, notifies and root tracks.
- **Kicks**
  - Metadata: `BP_HighPowerShootMetadata`, `BP_LowPowerShootMetadata`, `BP_TapAnimationMetadata`,
    `BP_LobTapAnimMetadata`.
  - Shot curves: `HighPowerBeforeShootScaleSpeedData`, `HighPowerAfterShootScaleSpeedData`,
    `V2ToShootSpeedScale`, `V3ToShootSpeedScale`, `ShootAfterDribbleSpeedScale`.
  - Pass curves: `BeforeTapScaleSpeedData`, `TapAfterShootSpeedData`.
  - Tables and configurations: `ShootDataTable`, `BP_Final_Shoot_Config`, `BP_BuildUp_Tap_Config`,
    `Bp_TapBuildUp_PlayerConfiguration`.
  - Pickers: the tap and lob pickers (≤ 2 m/s).
- **Controls**
  - Metadata: `BP_BallControlAnimMetadata`, `BP_BallControlV0AnimMetadata`,
    `BP_DashBallControlAnimMetadata`.
  - Speed curves: `BallControlSpeedScaleModifier`, `LockmoveBallControlSpeedScaleModifier`,
    `DashBallControlSpeedScaleModifier`.
- **Native** (Ghidra dump)
  - `ScaleSpeedRootMotionModifier__vf92/95/101/102`, `FUN_1449302b0`, `FUN_144931e70`,
    `FUN_1449318a0`.
  - Constructor `FUN_1447c02a0`: max entry 1000 cm/s, step 0.03 s.
  - `InputAngleAnimationPicker__vf97`.
- **Scratchpad tools** (local only): `rmtool` (root tracks), `rmsum.py`, `slidecalc.py`, `flow.py`
  and the `clip/fk_*.png` strips.
