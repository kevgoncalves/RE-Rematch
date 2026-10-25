# Volleys and flicks: six moves (spec, now being built)

Kev asked for six moves. This note maps how Rematch does each one and proposes how to add them to the
prototype. Kev approved the spec ("use the data's values where I didn't answer"); the moves are being
built on `game/volleys` in the order of §10. What was decided while building is in §11.

| # | Kev's name | Game ability |
|---|---|---|
| 1 | volley pass | `GA_VolleyTap` |
| 2 | volley shot | `GA_VolleyShoot` |
| 3 | volley push ball | `GA_VolleyPushBall` (ball up to 2.1 m), `GA_VolleyAerialPushBall` (higher) |
| 4 | lobbed push ball, the rainbow flick | `GA_LobbedPushBall` |
| 5 | rainbow flick out of a dribble | `GA_DribbleLobbedPushBall` |
| 6 | volley lobbed push ball | `GA_VolleyLobbedPushBall` (up to 2.1 m), `GA_VolleyAerialLobbedPushBall` (higher) |

Each push-ball move also has an exhausted twin (`GA_Exhausted…`) that replaces it at 0 stamina.

**How to read this**
- Frames are at 60 fps (6f = 0.1 s). Heights are of the ball's centre above the ground, in cm where
  the game uses cm.
- Every claim carries its source:
  - **[data]** a table, curve or default read from the extracted game files;
  - **[code]** logic read from the Blueprint dumps (`tools/rmx/out_bp`);
  - **[clip]** Kev's practice recordings of 2026-10-25;
  - **[inferred]** our reading of the above;
  - **[guess]** a placeholder to tune.
- "Prototype" means the Bevy code in `game/src` on `game/prototype`. Line numbers are approximate.

---

## 0. What all volleys share

### 0.1 What a volley is
A volley is a first touch on a loose ball that you don't own. It uses the same button as the ground
move: Shoot, Pass, Push ball or Lobbed push ball [data]. The game picks the volley version when both of
these hold:
- you don't have the ball;
- the ability finds a **volley target**, a point on the ball's path you can meet.

Each volley ability has `m_bRequireTarget = true` on its input [data].

### 0.2 The volley target (shared with the diamond)
The volley abilities and the diamond use the same query: `BP_VolleyTargetDatasource` over
`BallInterceptionPointTargetDatasource`. The full rules are in the diamond pass, summarised here
[data]:

| Rule | Value |
|---|---|
| Ball | nobody owns it; no `NoVolley` or `InsideGoal` status |
| You | the `CanVolley` layer is on (free move, dribble stance; **not** lock move, not after a dash); not `HasBall`, `HasLostBall` (0.5 s after your own kick), `RefillStamina`, tackling, stunned |
| When | the ball reaches the point **0.33–0.6 s** from now |
| Where | ≤ 6 m away; 1.2 m behind to 6 m ahead of your facing; side margin rising from 0 (1.2 m behind) to 5 m (3 m ahead) |
| Height | < **2.8 m** above your feet |
| Reach | straight distance ÷ time ≤ **9.2 m/s** (11 m/s in extra effort) |
| Line of sight | a ray to the point must be clear |
| Cache | the found point is kept 0.12 s, and dropped when it is under 0.3 s away |

**Ground or air:** a point up to **2.1 m** high is a ground candidate, and anything higher is an air
candidate [data].
- The volley shot and volley pass take both.
- The volley push ball and volley lobbed push ball take ground candidates only. Their aerial twins
  take the air ones (`BP_VolleyGroundedTargetDataSource` / `BP_VolleyAerialTargetDataSource`).

**Picking one point:**
- each point scores speed difference ×1 + a delay weight ×4 that favours 0.4–0.6 s [data];
- the volley shot and volley pass add +1000 for a contested ball [data];
- the volley shot uses a delay weight instead when the ball is a "BladeShot" (extra effort) [data];
- the prototype's `volley_target` takes the earliest valid point, which is simpler and close enough.

### 0.3 The shared timeline [data + inferred]
1. **Press.**
   - The ability locks the target point P and the time t until the ball gets there (0.33–0.6 s).
   - The blue diamond goes, and a green "confirm" copy follows the ball until contact.
2. **Padding (approach).**
   - Until the volley animation starts, you run toward a "snap" spot: the place your body must be for
     the chosen body part to meet the ball.
   - It is a locomotion mode of its own (`States.Padding`, the `Loco_Padding` cycles), driven by
     `MoveToSnapLocation` with a 0.4 s limit [code].
3. **Volley animation.**
   - It is timed so that its contact frame (the `BPShoot` notify) lands when the ball reaches P.
   - From its first frame until 1f before contact, a snap window (`BP_BallControlSnapAnimNotify`)
     warps you onto the ball [data].
   - The same span is the **volley cancel window**: a refusal can only cancel the volley before contact.
4. **Contact.**
   - The ball is launched, with a hitstop on shots and passes but none on push balls.
   - The ball gets its statuses.
5. **Recovery** until the move's availability layer starts.

**Contact frame = end of the cancel window.** Our `frame-data.csv` puts the volleys'
`ball_contact_frame` at 0–4. That is the start of the snap notify, not the strike:
- every volley's anim metadata ends the cancel window at the `BPShoot` notify [data];
- the volley tables (`DT_VolleyTap`, `DT_VolleyPushBalll`, `DT_VolleyLobbedPushBall`) give the same
  frames as the CSV's cancel-window ends.

**Press to contact in the recordings [clip]:**
- **Jumping volley shot** at 121.07 s in `02-55-47.mkv`:
  - LMB pressed at 121.067 s;
  - the camera swung behind at once, and the diamond turned green;
  - the contact flash came at **121.433 s, 0.37 s after the press**.

  This fits "contact = arrival at P, 0.33–0.6 s".
- **Push ball on a rolling ball** at 141.5 s in `02-33-58.mkv`:
  - the diamond showed from 141.47 s;
  - the overlay read M4 at 141.43 s and again at 141.53 s;
  - yellow snap rings appeared at about 141.60 s;
  - the touch came at about 141.73–141.77 s.

  That is 0.2–0.33 s depending on which press counted, so this case is not conclusive.

**Refusals** come from the server only. A refused volley plays a cancel animation and a red flash on
the ball, then the ability ends after 0.8 s [data].
- The `VolleyThresholdDataTable` heights choose a ground or an aerial cancel animation for this:
  - volley shot 60 cm standing / 120 cm moving;
  - volley pass and volley push ball 70 cm;
  - goalkeeper 75 cm;
  - volley lobbed push ball 80 cm.
- mechanics.md §5 reads them as body-part thresholds, which they are not [code]. The prototype is
  offline and needs no refusals.

**Ball control during a volley:** once a volley has started, it owns the ball's arrival. The magnet
control must not grab the ball first [inferred]. The prototype's `volley_target` already refuses while
receiving, so a press during a control stays the queued "control, then kick".

### 0.4 Which body part, and jumping or standing
Every volley picks the animation from the height of P above the ground. Each cell gives the band in cm
[data]:

| Body part | Volley pass (`BP_VolleyTapBuildupMontagePicker`) | Volley shot, standing < 3 m/s | Volley shot, moving ≥ 3 m/s | Volley push ball | Volley lobbed push ball |
|---|---|---|---|---|---|
| Foot | < 20 | < 30 | < 40 | < 30 | < 40 |
| Thigh | 20–70 | 30–60 (forward and back); the sides use pelvis | 40–120 | 30–70 | 40–80 |
| Pelvis (stomach) | 70–120 | 60–130 (and 30–60 to the sides) | none | 70–120 | 80–120 |
| Chest | 120–170 | 130–160 | 120–180 | 120–165 | 120–160 |
| Head | 170–230 | 160–210 | none | ≥ 165 (to 210) | ≥ 160 (to 210) |
| Jumping chest | none | 210–250 | 180–230 | aerial push ball above 210 | aerial lobbed push ball above 210 |
| Jumping head | ≥ 230 | none (above 250 falls back to the moving foot volley, a data quirk) | ≥ 230 | | |

- **Standing or moving** only matters for the volley shot: below 3 m/s (300 cm/s) it uses the V0
  set [data].
- **Jumping** is just the top band; there is no separate jump rule.
- **Left or right:** the side of your body the ball arrives on picks the mirrored animation
  (`TargetSideAnimSequencePicker`). The kick direction variant then keeps the same mirroring [data].

### 0.5 Direction variants
| Volley | Measured as | Sectors |
|---|---|---|
| Volley pass | pass direction against your facing [data] | ahead ±15°; NE/NW 15–90°; SE/SW 90–150°; behind 150–180° |
| Volley shot | camera aim against your facing [data/inferred reference] | ahead ±30° (standing foot) or ±40°; sides 40–125°; behind 125–180° (turning volley) |
| Volley push ball and volley lobbed push ball | push direction against your facing [data] | ahead ±45°; sides 45–135°; behind 135–180° |

### 0.6 The UI
**Diamond:** already in the prototype (`fx.rs`, `volley_diamond`). It shows when a volley is
available, fades in over 0.23 s and hides at once [data].

**Missing in the prototype:**
- **Green confirm** [data]:
  - starts at the press;
  - the same diamond in green (linear 0.0875, 1.75, 0.226), local player only;
  - follows the ball until contact, then freezes there;
  - alpha 1 → 0 over **0.35 s**.
- **Volley pass aim marker** (`NS_Tap_VolleyMarker`): the pass line, shown while the volley pass is
  being prepared [data].
- **Volley shot camera:** the tag `Camera.VolleyShoot` puts the camera in its "Shoot" state before
  impact (camera.md in the main notes) [data].
  - In the clip it swung behind the player at the press [clip].
- **Ball trail:** on for every volley. The one exception is a volley push ball kicked below 75 cm
  [data].

---

## 1. Volley pass (`GA_VolleyTap`)

### Trigger
- **Input:** Pass. Right mouse on keyboard, X on gamepad (`TapInput`, a hold) [data].
  - The low-power volley pass (right mouse + Space, X + A) and the volley lob (F, RB + X) are
    separate abilities, out of scope here.
  - In its 0.1 s buffer the volley pass adds `CanLobbedTap` and `CanShortTap`, so those chords can
    still take over (`ALBuffer_VolleyTap`) [data].
- **Needs:** `CanTap` + `CanVolley`, and a target from the full datasource (ground or air) [data].
- **Blocked by:** HasLostBall, RefillStamina, TakeBall, HasBall, tackling, Sweeper [data].
- **Buffer:** 0.1 s [data].
- **Body part:** by height (§0.4).
- **Direction variant:** by pass direction (§0.5).
- **No-look tag:** a pass ≥ 60° off your facing gets it. It only skips the head turn [code].
- **Moving or standing:** one set of animations for every speed [data].

### Ball
**Speed by seconds held** (`BP_VolleyTapAbilityData_AnalogPower`, the time axis is in seconds; see the
open questions) [data]. It is the normal pass's curve squeezed into 0.25 s:

| Held | Speed |
|---|---|
| 0–0.087 s | 20 m/s |
| 0.1725 s | 32 m/s |
| just after 0.1725 s | 25.6 m/s |
| 0.25 s (full) | 30.65 m/s |

Charge ratio = held ÷ 0.25 s. It drives three modifiers [data + code]:
- **Force by height** (`BP_VolleyTap_ForceDependingBallHeightAndTime`). The ball height is its centre,
  in cm.
  - Ratio above 0.69 (a lifted pass): 1.0 at 15 cm, 0.93 at 100 cm, 0.90 at 320 cm.
  - Otherwise: 1.0 at 11 cm, 1.05 at 11.5 cm, 1.0 at 18 cm, 0.96 at 20 cm, **0.80 at 70 cm**, 0.72 at
    320 cm.
  - Ball above 100 cm: also × (1.0 up to ratio 0.348, 0.89 at 0.69, 1.0 at 1.0).
- **Lift by charge** (`BP_VolleyTapAnalogPower_PitchDependingForce` with
  `Curve_VolleyTapPitchModifierDependingTime`): +0° up to 0.69, **+3.9°** just past it, +6° at 1.0.
- **Pitch by height** (`BP_VolleyTap_AngleDependingBallHeight`): 0° up to 15 cm, falling linearly to
  **−8° at 290 cm**.

**Aim:** the normal pass's (`BP_Gamepad_Tap_FilteredInputDirectionComputation`; the same keyboard
options) [data]. The prototype's `pass_yaw` applies as is.

**Flight** (the normal analog pass's modifiers) [data]:
- friction ×1.2;
- drag 0.75 with weight 1.25 from the first bounce;
- forced Magnus +200 (lift) until the first bounce, which is spin.y −200 in the prototype
  (`PASS_FORCED_SPIN`).

**Hitstop:** 3f (`ShootDataTable` row VolleyTap) [data].

**Ball statuses:** the Tap set, with NoVolley 0.5 s [data]. The prototype uses `NO_RECONTROL_TIME` 0.5 s.

### Player timeline
Frames counted from the volley animation's start (`DT_VolleyTap`) [data]:

| Body part | Contact | Can act (everything) | After contact | In the air |
|---|---|---|---|---|
| Foot | 15f | 25f | +10f | |
| Thigh | 17f | 26f | +9f | |
| Pelvis | 18f | 31f | +13f | |
| Chest | 18f | 33f | +15f | |
| Head | 18f | 34f | +16f | |
| Jumping header | 24f | 55f | +31f | 8–42f |

- `DT_VolleyTap` also has JumpChest (24f / 50f) and a long-range jumping header (26f / 70f), but the
  outfield picker uses neither [data].
- **From the press:**
  - padding until the animation starts, then the contact at arrival (0.33–0.6 s, 20–36f);
  - so a foot volley pass lets you act **30–46f** after the press [inferred].
- **Movement:**
  - the approach averages at most 9.2 m/s [data];
  - `BP_VolleyTapSnap` places the snap spot keeping your current facing (`m_bKeepActorForwardToPlaceSnapLocation`)
    [data];
  - after contact, the animation's own root motion moves you; no numbers in the data.
- **Stamina:** none [data].

### Prototype
- **New variant `Kick::VolleyPass`:**
  - `charge_action` → Pass;
  - `is_pass` true, so `pass_line.rs` shows its aim line;
  - `freeze_frames` 3.
- **Contact** = the target's t, read at the press (§7).
- **Launch arm:**
  - speed = `VOLLEY_PASS_FORCE`(held s) × the height factor;
  - elevation = pass lift + lift by charge + pitch by height;
  - yaw = `pass_yaw`;
  - friction `PASS_FRICTION`, spin `PASS_FORCED_SPIN`.
- **Body:** six touch poses (§7.4).

---

## 2. Volley shot (`GA_VolleyShoot`)

### Trigger
- **Input:** Shoot. Left mouse on keyboard, RT on gamepad (`ShootInput`) [data].
- **Needs:** `CanShoot` + `CanVolley`, and a target (ground or air) [data].
- **Blocked by:** HasLostBall, RefillStamina, TakeBall, HasBall, tackling, Sweeper, GoalKeeper [data].
- **Body part:** by height and by your speed (standing below 3 m/s); see §0.4.
- **Direction variant:** by camera aim (§0.5).
- **Turning toward the shot:** `BP_VolleyShootSnap` places the snap spot along the shot direction
  (`TargetData.Direction`), so you turn toward it [data].
- **Aim lock:** the game option `LockShootOnController` and the keyboard override work as for the
  normal shot [code].

### Ball
**Speed by seconds held**, with no build-up (`Default__BP_ShootForceFromInputCurve`) [data]:

| Held | Speed |
|---|---|
| up to 0.05 s | 24 m/s |
| 0.25 s (full) | 38 m/s |

- **Analog trigger:** it fills at 25% speed up to 47.5% press, then rises to 100% at a full press
  (`Curve_VolleyShootFillFactorByAnalogPressValue`) [data].
- **The normal shot's modifier** (`BP_Shoot_BallCharaOrientationModifier`) applies on top [code + data]:
  - **Run-up bonus.** Already in the prototype as `SHOT_FORCE_BY_RUN_SPEED` × `SHOT_RUN_BONUS_BY_FORCE`.
    New for volleys: it is **scaled by the contact height**, 1.0 up to 120 cm, 0.9 at 150, 0.75 at
    180, 0.4 at 220 cm (`ShootForceModifierClampByBallHeight`).
  - **Incoming-ball bonus.** Not in the prototype yet, and it matters for volleys.
    `ShootForceByBallSpeed` gives 0 at 2 m/s up to +3.25 m/s at 30 m/s.
    It is multiplied by `ShootOrientationRatioBallSpeed(dot(ball direction, shot direction))`: 1.0 at
    ≤ −0.5 (you send it roughly back where it came from), 0 at ≥ 0.05.
    It is then multiplied by the same charge factor: 0 below 27.5% of the force range, 1 at full.
- **Elevation:**
  - the camera pitch as for the normal shot (`ELEVATION_BY_PITCH`);
  - plus the shared pitch by force;
  - plus pitch by height (`BP_VolleyShoot_PitchDependingBallHeigh`): +0.5° at 50 cm, −3.25° at
    154 cm, −3.5° at 154.1 cm, −6° at 210 cm, −8° at 300 cm.

    High volleys are pushed down [data].
- **Spin:** as the normal shot (stick or mouse curve scaled by charge, forced +210) [data].
- **Shot type** (VFX and camera shake only): Low below 29.41 m/s, Medium below 34.82 m/s, High above
  [code + data].
- **Hitstop:** **5f** at every power (`ShootDataTable` row VolleyShoot) [data].
- **Ball statuses:** NoVolley 0.75 s, plus the Shoot NoControl, NoDeflect and GKDeflect statuses [data].

### Player timeline
Frames from the volley animation's start. "Can act" is everything (GenericAll), from `frame-data.csv`
[data]. Contact is the cancel-window end [data + inferred].

| Set | Body part | Contact | Can act | After contact | In the air |
|---|---|---|---|---|---|
| Standing | Foot | 18f | 48f | +30f | |
| Standing | Thigh / pelvis | 18f (pelvis to the sides 22f) | 48f | +26–30f | |
| Standing | Chest | 19–22f | 52–57f | +30–35f | |
| Standing | Head | 22f | 52–53f | +30f | |
| Standing | Jumping chest | 30f | 60f | +30f | 12–53f |
| Moving | Foot | 22f | 52–53f | +30f | |
| Moving | Thigh | 22–23f | 52–53f | +30f | |
| Moving | Chest | 22f | 59–62f | +37–40f | 5–39f (a hop) |
| Moving | Jumping chest | 16f | 52–53f | +36f | 2–46f |
| Moving | Jumping head | 22f | 65f | +43f | 6–48f |

- **From the press:** contact at arrival (0.33–0.6 s), then you can act about 0.5–0.7 s later
  [inferred].
- **Jumping volley in the clip** [clip]:
  - the knee came up about 0.1 s before contact;
  - the kicking foot struck at head height with the body almost flat, a scissor kick;
  - the player then fell onto their back and side and lay there for about 0.5 s.
- **Movement:** the approach averages at most 9.2 m/s; the redirection takes 0.032 s
  (`BP_VolleyShootConfiguration`) [data]. After contact, root motion only [unknown].
- **Stamina:** none [data].

### Prototype
- **New variant `Kick::VolleyShot`:**
  - `charge_action` → Shoot, and full charge at 0.25 s held;
  - contact = the target's t;
  - `freeze_frames` 5.
- **Launch arm:** the `Kick::Shot` arm, plus the height pitch, the contact-height scaling of the run
  bonus, and the incoming-ball bonus.
- **Aim:** shot aim and the camera lock (`aim`, `eye`) as now.
- **UI elsewhere:**
  - `camera.rs` treats it like Shot, so the camera goes to Shoot until contact;
  - `reticle.rs` `power()` fills for it.
- **Body:** volley strikes by part (§7.4). Jumps need the root to leave the ground, which `body.rs`
  can't do yet.

---

## 3. Volley push ball (`GA_VolleyPushBall`)

### Trigger
- **Input:** Push ball. Space on keyboard, A on gamepad, Back in the thumb preset (`PushballInput`)
  [data].
  - In the recordings, Kev's push ball looks like M4 [clip/inferred].
  - In its 0.1 s buffer it adds `CanLobbedPushBall` and `CanShortTap`. RB within 0.1 s therefore turns
    it into the volley lobbed push ball, and X into a low-power volley pass (`ALBuffer_VolleyPushBall`)
    [data].
- **Needs:** `CanVolley` + `CanPushBall`, and a **ground** target (P ≤ 2.1 m) [data].
- **Above 2.1 m:** the aerial push ball takes over (§3.4).
- **Blocked by:** HasLostBall, RefillStamina, TakeBall, HasBall, tackling, OutOfStam [data].
  - At 0 stamina, `GA_ExhaustedVolleyPushBall` fires instead.
  - Activation ignores the stamina check, so a low bar still allows it.
- **Body part:** foot < 30, thigh 30–70, pelvis 70–120, chest 120–165, head ≥ 165 cm [data].
- **Direction:**
  - the stick, camera-relative; a neutral stick pushes along your facing (`TapOrientationDataTable`);
  - **clamped to 30° of your facing** at contact [data];
  - the animation sector is picked by the push direction (§0.5).

### Ball
- **Speed:** **11.25 m/s, flat**. The pitch is capped at 0°, so the ball leaves from the contact height
  [data].
- **Speed multipliers** [data + code]:
  - height (`VolleyPushBall_HeightForceRatio`, read at the contact height − 15 cm): 1.0, falling to
    0.8 at 315 cm;
  - **×1.4 when sprinting** (`States.Sprinting` at contact);
  - ×0.9 if the push is 45° or more off your facing.
- **Flight:**
  - friction ×1.6 (1.28);
  - forced dip spin, Magnus −1000 in the game, which is **spin.y +1000** in the prototype;
  - default drag [data].
- **Statuses:**
  - NoControl 0.5 s, for the kicker and for everyone;
  - NoVolley 0.425 s;
  - kicker: standing-tackle cooldown 0.8 s [data].
- **Hitstop:** none (`FreezeFrame_VolleyPushBall` = 0) [data].
- **Flight in our model** [inferred], with `ball.rs`:
  - from 15 cm it rolls about 5.4 m;
  - from 50 cm it first lands at about 1.9 m and rolls about 8.6 m;
  - from 1 m it first lands at about 2.8 m and rolls about 11 m.

### Player timeline
From the volley animation's start (`DT_VolleyPushBalll` and `BP_VolleyPushBallAnimMetadata`) [data]:
- **Contact:** 14f (foot, thigh) or 18f (pelvis, chest, head).
- **Contact + 12f (26f or 30f):** only sprint and extra effort (`AL_OnlyExtraEffort`). This matches
  frame-data.md's "your next action has to be a sprint or extra effort".
- **Push-ball layer** (everything a push ball allows), by direction:

  | | Foot | Thigh | Pelvis | Chest | Head |
  |---|---|---|---|---|---|
  | Ahead | 40f | 42f | 38f | 34f | 34f |
  | Behind | 40f | 42f | 38f | 41f | 41f |
  | Sides | 40f | 42f | 38f | 41f | 42f |

- **Running after the ball** ("redirection") [data]:
  - it runs from 0–12f after contact to 6–27f after contact, by row;
  - speed ahead 5.5 m/s (5 m/s in a conditional row), behind 1.5 → 5.5 m/s, sides 2.5 → 5 m/s.
  - The dash phase scales your entry speed 100% → 50% by 20% of it, with an exit of 8 m/s. Coming in
    at ≥ 6.5 m/s you hold 8.5 m/s instead.
- **Stamina:** 0.375, and regen pauses 1.0 s (`GE_Cost_PushBall`) [data].

### 3.4 The aerial push ball, above 2.1 m (brief)
- **Ball:** 9 m/s at 40°, ×0.7 to the sides, ×0.65 back, plus a height-based force modifier [data].
- **Animation:** a jumping head or chest touch.
  - Contact at 18–21f, in the air 6–39f.
  - Sprint and extra effort only from landing, everything at 50–55f (`DT_VolleyAerialPushBall`) [data].
- Build it after the ground one.

### Prototype
- **New variant `Kick::VolleyPushBall`:**
  - no charge;
  - contact = the target's t;
  - launch: 11.25 m/s × factors, elevation 0;
  - from the contact height;
  - friction `BASE_FRICTION` × 1.6, spin.y +1000;
  - no_control 0.5.
- **After contact:** reuse the push ball's chase `Run`, with a new speed table: the redirection numbers
  above, `free_at` = contact + 12f for sprint only.
- **Stamina** as the push ball.
- **Body:** the touch poses with a short forward push (§7.4).

---

## 4. Lobbed push ball: the rainbow flick (`GA_LobbedPushBall`)

### Trigger
- **Input** (`LobbedPushBall`, a hold combination, priority 2 over the push ball's 0) [data]:
  - **C** on keyboard;
  - **RB + A** on gamepad, RB first;
  - modifier + Back in the thumb preset.
  - The prototype already binds all three (`input.rs`), but `implemented()` still excludes the action.
- **Chord rescue:** a push ball started up to 0.1 s before RB still becomes the lobbed push ball
  (the push ball's buffer layer adds `CanLobbedPushBall`) [data, earlier defence draft].
- **Needs:** `HasBall` + `CanLobbedPushBall`. That means free move with the ball, or the dribble stance
  [data].
  - Not in lock move, after a dash, or during a dribble; a dribble uses §5.
- **Blocked by:** Stunned, HasLostBall, HasBallInHand, OutOfStam [data]. At 0 stamina,
  `GA_ExhaustedLobbedPushBall` takes over.
- **Direction** (`PushBallOrientationDataTable`) [data]:
  - the stick, camera-relative (snap tolerance 6° straight ahead, 22.5° otherwise);
  - a **neutral stick flicks along your facing**;
  - dead zone 0.1.
  - The prototype's `steer` handles the push ball the same way.
- **Animation:** ahead ±45° (North), sides 45–135° (East, mirrored for West), behind (South). Ahead and
  behind also pick the foot by the run cycle's phase (`FootSyncMarker`) [data].
- **Moving or standing:** one set of animations. Speed changes the movement curves and, when
  sprinting, the ball [data].

### Ball
**Base** (`BP_LobbedPushBallAbilityData`) [data]:
- **11.5 m/s at 65°** above the horizontal;
- the default flight model: friction 0.8, drag as default;
- a huge forced dip spin, Magnus −2500 in the game = **spin.y +2500** in the prototype
  (`BP_LobPushBall_MagnusTrajectoryModifier`).

**Direction modifiers.** The flick direction is compared with your facing as a 2D dot product, with
thresholds 0.9 and −0.25 [code]. The degrees are added to the 65° [inferred: apply mode 0 = add].

| Flick | Force | Angle |
|---|---|---|
| ahead (within ~26°) | ×0.982 | +3.5° |
| sideways (26–104°) | ×0.9 | +10° |
| behind (beyond 104°) | ×0.975 | +3.5° |

**Sprinting** (`TargetData.IsSprinting` at the press): ×1.2 and **−12.5°** [code].

**Exhausted:** 13 m/s at 58°, with the same modifiers [data].

**Flight in our `ball.rs` model** [inferred]:

| Case | Launch | Apex | First bounce | Rolls to |
|---|---|---|---|---|
| ahead | 11.3 m/s at 68.5° | **2.2 m** at 0.38 s | **5.4 m** at 0.79 s | ~12 m |
| ahead, sprinting | 13.6 m/s at 56° | 2.0 m | 7.0 m at 0.70 s | ~17 m |
| sideways | 10.4 m/s at 75° | 2.2 m | 4.5 m at 0.83 s | ~10 m |
| behind | 11.2 m/s at 68.5° | 2.1 m | 5.3 m | ~12 m |
| ahead, no spin (comparison) | | 3.5 m | 5.3 m at 1.29 s | |

- The dip spin is what makes it a rainbow: steep up, then hooked forward and down over a defender's
  head.
- About 23 m/s² of Magnus at launch. Check it against the game before tuning.
- The base friction 0.8 lets it roll a long way after landing, unlike the push ball's ×2.8. That is
  what the data says, but worth a check.

**Ball statuses**, for everyone [data]:
- NoControl **0.5 s**;
- NoVolley **0.55 s**;
- **NoTackles.Sliding 0.35 s**: a sliding tackle only deflects the ball while it is up.

**Kicker:**
- standing-tackle cooldown **1.1 s** (`GE_Cooldown_StandingTackle_AfterLobPushBall`) [data];
- HasLostBall 0.5 s [data, earlier defence draft].

**Hitstop:** none [data].

### Player timeline
From the press (`DT_LobbedPushBall`, `BP_LobbedPushBallAnimMetadata`, frame-data.csv) [data]:

| | Ahead | Sideways or behind |
|---|---|---|
| Wind-up to contact | **22f (0.37 s)** | **16f (0.27 s)** |
| Body turned along the ball's path (redirect notify) | 22–35f | 16–28f / 16–33f |
| Push-ball layer (shoot, control) | **35f (0.58 s)** | 35f |
| Everything | ~40f | ~40f |
| Animation length | 66f | 70f |

Before 35f only sprint is allowed [data, earlier defence draft]. Ball control has to wait for
NoControl, so the earliest is 52f ahead or 46f otherwise [inferred].

**Movement.** Times are fractions of each phase (`LobbedPushBall*BuildUpData`, `*DashData`) [data]:

| Phase | Ahead | Sideways or behind |
|---|---|---|
| Wind-up | moving ≥ 1 m/s: **6 m/s** flat; standing: 4 m/s; exhausted 3.5 m/s; hands back 3.5 m/s | your entry speed ×1 at 10% → **0 at 80%**, so you stop to flick it; below 2.5 m/s a 5 → 0 m/s step; sideways hands back 3 m/s |
| After contact | 20f at ×1 of your speed, then 5 m/s | 36f, ×1 → 0 by 25–30%, then 5 m/s |

**Stamina:** **0.75**, and regen pauses 1.0 s [data].

### Prototype
- **`input.rs`:** drop `LobbedPushBall` from the `implemented()` exclusion.
- **`player.rs`:**
  - **New variant `Kick::LobbedPushBall`.**
  - **Start:** add `(Action::LobbedPushBall, …)` to the instant list in `kicking`. It starts only
    outside a dribble and not in lock move.
  - **Chord takeover:** add `PushBall → LobbedPushBall` within `CHORD_WINDOW`.
  - **Wind-up:** `contact_time` = 22f ahead or 16f otherwise, from `steer` against your facing at the
    press.
  - **Ball at the foot:** the push ball's pull in `carry_ball`.
  - **Launch arm:**
    - speed `LOB_PUSH_BALL_SPEED` × the direction factor × sprint 1.2;
    - elevation 65° + the offset − 12.5° when sprinting;
    - friction `BASE_FRICTION`, spin.y +2500, no_control 0.5, freeze 0;
    - the exhausted arm uses 13 m/s at 58°.
  - **Stamina:** 0.75 and `PUSH_BALL_REGEN_DELAY`.
  - **Movement:** a wind-up speed table (the prototype's `WINDUP_SPEED_SCALE` doesn't fit) and an
    after-touch `Run` from the table above.
  - **Every exhaustive match:** `buildup`, `freeze_frames` and the launch match.
- **`fx.rs`:** the ball trail goes on. Whether the landing marker shows is an open question.
- **`body.rs`:** the flick poses (§7.4); `follow_time` and `kick_size` arms.

---

## 5. Rainbow flick out of a dribble (`GA_DribbleLobbedPushBall`)

### Trigger
- **Input:** the same as §4 [data].
- **Needs:** `HasBall` + `CanDribbleLobbedPushBall`, which only the dribble layers grant [data]:
  - the dribble, the feinted dribble and the exhausted dribble;
  - from **12f** into the dribble until it ends at 45f.
- **Inherits** everything else from §4: blocks, statuses, chord buffer and tackle cooldown [data].
- **Which one fires:** in free move both abilities are allowed on the same input and priority, and
  which wins wasn't found. Spec rule: dribble active from 12f → this one, else §4 [inferred].
- **Animation:** ahead, sideways or behind by the flick direction, as §4 [data].
  - These six clips are the only ability montages outside the run cycle's foot-sync group [data].

### Ball
- **12.5 m/s at 65°**, the same in every direction (no direction modifiers) [data].
- **Sprinting:** ×1.1 and −5° (`BP_DribbleLobbedPushBall_SprintForceModifier`) [data].
- **Exhausted:** 12.75 m/s at 62° (dribble.md in the main notes) [data].
- **Spin, friction and statuses:** as §4 [data].
- **Flight in our model** [inferred]:
  - apex 2.3 m, first bounce 6.4 m at 0.79 s;
  - sprinting: first bounce 7.4 m.

### Player timeline
[data]:
- **Wind-up:** **16f**. The montage marker sits at 15f.
- **Redirect:** 9–62f ahead, 15–68f sideways, 15–48f behind.
- **Push-ball layer:** 35f. Everything at ~50f (~43f behind).
- **Animation length:** 86f, or 77f behind.
- **Wind-up speed** (`BP_DribbleLobbedPushBallBuildUpData`):
  - moving: **5 → 6.5 m/s** by 15%;
  - standing: 5 → 6 m/s;
  - exhausted: 3.5 → 4.5 m/s;
  - it hands back 3.5 m/s.

  You keep the dribble's pace whatever the direction.
- **After contact:** 20f at ×1 of your speed, then 5 m/s (4.5 m/s exhausted).
- **Stamina:** **0.75** (`Cost_DribbleLobbedPushBall`), regen pause 1.0 s [data].

### Prototype
- **New variant `Kick::DribbleLobbedPushBall`**, chosen when `p.dribble` has run 12f or more.
- The dribble ends as it does for `DribblePushBall`.
- **Launch arm:** 12.5 m/s × sprint 1.1 at 65° − 5° when sprinting.
- **Movement:** its own wind-up and after-touch tables.
- **Facing:** turn to the flick over the wind-up, like `DribblePushBall`.

---

## 6. Volley lobbed push ball (`GA_VolleyLobbedPushBall`)

### Trigger
- **Input:** Lobbed push ball, as §4 [data].
- **Needs:** `CanVolley` + `CanLobbedPushBall`, and a **ground** target (P ≤ 2.1 m) [data].
- **Above 2.1 m:** the aerial lobbed push ball takes over. It is a jumping header flick at 10 m/s and
  65°, ×0.7 to the sides and ×0.65 back [data].
- **Blocked by** (inherited from the volley push ball): HasLostBall, RefillStamina, TakeBall, HasBall,
  tackling, OutOfStam [data]. At 0 stamina, `GA_ExhaustedVolleyLobbedPushBall` (14.5 m/s at 68°)
  fires instead.
- **Target pick:** speed difference + 4 × delay, with no contest bonus
  (`BP_VolleyLobPushBallTargetPicker`) [data].
- **Buffer layer:** adds `CanVolley` and `CanShortLob` [data].
- **Body part:** foot < 40, thigh 40–80, pelvis 80–120, chest 120–160, head ≥ 160 cm [data].
  - In the 120–160 band the approach uses the thigh clip and the touch uses the chest clip.
- **Direction:** the stick, camera-relative, neutral = your facing (`PushBallOrientationDataTable`)
  [data]. The sector is picked as in §0.5.

### Ball
**Base:** **13.5 m/s at 75°** (`BP_VolleyLobbedPushBallAbilityData`) [data].

**Height modifier** (`BP_VolleyLobbedPushBall_ForceFromHeight`) [code + data]:
- h = contact height − ball radius, in cm;
- r = `VolleyLobbedPushBall_HeightForceRatio`(h): 1.0 at 0, 0.7 at 179 cm, 0.5 at 300 cm (cubic);
- force × r, angle **+ r × (−10°)**;
- so a ground-level flick goes at 65°, and a 1.8 m one at 68° and 70% force.
- The blueprint also holds `ForceMultiplier = 0.85`, but its logic doesn't use it.

**Sprinting:** ×1.1 and −10° (`BP_VolleyLobbedPushBall_ForceFromSpeed`) [code].

**Spin:** the same +2500 dip as §4 [data].

**Statuses** [data]:
- NoControl 0.5 s, for the kicker and for everyone;
- NoVolley 0.5 s;
- NoTackles.Sliding 0.35 s;
- **LowPriority.VolleyLobbedPushBall 0.8 s, for the kicker only**: your own flick is a low-priority
  volley target while you chase it.

**Kicker:** standing-tackle cooldown 1.1 s [data].

**Hitstop:** none [data].

**Flight in our model** [inferred]:

| Contact height | Launch | Apex | First bounce |
|---|---|---|---|
| 0.2 m | 13.3 m/s at 65° | 2.6 m | 7.2 m at 0.83 s |
| 0.2 m, sprinting | 14.6 m/s at 55° | 2.3 m | 7.9 m |
| 1.0 m | 11.5 m/s at 66.5° | 3.0 m | 6.0 m |
| 2.0 m | 9.2 m/s at 68° | 3.5 m | 4.4 m |

### Player timeline
From the volley animation's start (`DT_VolleyLobbedPushBall`, `BP_VolleyLobbedPushBallAnimMetadata`)
[data]:
- **Contact:** **18f** for every body part.
- **Redirect:** from contact + 2f to contact + 14–28f, by row. Speed 3 → 5.25 m/s ahead; 5 → 7.5 m/s
  if you came in faster than 1 m/s.
- **Dash phase:** your entry speed falls 100% → 0 by 20% of it, so you stop to flick it, then exits at
  6 m/s.
- **Push-ball layer** (the first thing you can do), by direction:

  | | Foot | Thigh | Pelvis | Chest | Head |
  |---|---|---|---|---|---|
  | Ahead | 40f | 40f | 40f | 45f | 35f |
  | Behind | 55f | 50f | 50f | 45f | 45f |
  | Sides | 40f | 40f | 45f | 43f | 45f |

- **Stamina:** **0.75** (`GE_Cost_LobbedPushBall`), regen pause 1.0 s [data].

### Prototype
- **New variant `Kick::VolleyLobbedPushBall`**, through the volley start (§7).
- **Launch** as above; push direction as §4.
- **Movement:** redirect table and dash table.
- **Body:** a flick from the touch pose of each body part (§7.4).

---

## 7. Fitting it into the prototype

### 7.1 Shape: Kick variants, not a new state
`KickState` already holds the press-to-contact flow:
- charge (`held`, `released`, `pre`);
- aim (`aim`, `eye`) and `steer`;
- `contact_time`;
- the launch match and freeze;
- the body's kick layer (`keys(kind, turn)`).

Six new `Kick` variants plus two fields cover everything:

```text
Kick::VolleyPass, VolleyShot, VolleyPushBall, VolleyLobbedPushBall, LobbedPushBall, DribbleLobbedPushBall
KickState.volley: Option<VolleySpot>   // P, t, height, side, body part, direction sector
```

Every `match` on `Kick` is exhaustive:
- player.rs: `buildup`, `freeze_frames`, the launch match;
- body.rs: `follow_time`, `kick_size`, `keys`.

So the compiler lists every place to fill.

### 7.2 Volley plumbing, written once for all four volleys
1. **One target per frame.**
   - Run `volley_target` once, before `kicking`, and store it on the player or as a resource. Today
     `volley_diamond` runs it after physics.
   - The diamond and the kicks then read the same spot, so the diamond never lies.
   - Add to `VolleySpot`: height above the feet, ground or air (≤ 2.1 m), the ball's side (left or
     right), and the body-part band.
2. **Starting a volley in `kicking`.** Add a second start branch:
   - when: `!has_ball && kick.is_none() && receiving.is_none()` and a spot exists;
   - which: Shoot → VolleyShot, Pass → VolleyPass, PushBall → VolleyPushBall (ground spot only),
     LobbedPushBall → VolleyLobbedPushBall (ground spot only);
   - buffer 0.1 s, chord takeovers as in §3;
   - `contact_time` for volleys = the spot's t at the press.
3. **At the strike,** the `if !p.has_ball { return; }` guard (around line 1401) must let volleys
   through. Launch from where the ball is at that frame.
4. **Approach ("padding").**
   - While a volley kick runs, steer the player toward the snap spot: P minus the body part's offset,
     turned by your facing (the shot: by the shot direction);
   - speed = remaining distance ÷ remaining time, capped at 9.2 m/s (11 in extra effort);
   - pass it to the move layer as a `Run` or a target velocity.
   - The offsets are a [guess]: foot 0.35 m ahead and 0.2 m to the side, thigh 0.3 m ahead, chest and
     head 0.25 m ahead.
5. **Ball control.** In `ball_control`, skip the magnet while a volley kick is pending, so the volley
   gets the ball [inferred].
6. **No-control.** `volley_target` already refuses a ball with `no_control > 0`. That stands in for
   NoVolley; it is short by 0.05 s after a lobbed push ball (0.55 s) and longer by 0.075 s after a
   volley push ball (0.425 s). Close enough.
7. **UI.**
   - `fx.rs`: a green confirm that follows the ball and freezes at contact (0.35 s fade).
   - `pass_line.rs`: the line for the volley pass (via `is_pass`).
   - `reticle.rs`: the charge fill for the volley shot.
   - `camera.rs`: the Shoot state for the volley shot.

### 7.3 New tuning constants (`tuning.rs`)

| Constant | Value | Source |
|---|---|---|
| `VOLLEY_GROUND_MAX` | 2.1 m | [data] |
| `VOLLEY_SHOT_FORCE` | [(0, 24), (0.05, 24), (0.25, 38)] by seconds held | [data] |
| `VOLLEY_PASS_FORCE` | [(0, 20), (0.087, 20), (0.1725, 32), (0.1726, 25.6), (0.25, 30.65)] | [data] |
| `VOLLEY_PASS_HEIGHT_LOW` | the "otherwise" curve in §1 | [data] |
| `VOLLEY_PASS_HEIGHT_LIFTED` | the "above 0.69" curve in §1 | [data] |
| `VOLLEY_PASS_HIGH_BALL` | the extra curve for balls above 1 m | [data] |
| `VOLLEY_PASS_LIFT` | [(0, 0), (0.69, 0), (0.691, 3.9), (1, 6)] by charge ratio | [data] |
| `VOLLEY_PASS_HEIGHT_PITCH` | [(0, 0), (0.15, 0), (2.9, −8)] | [data] |
| `VOLLEY_SHOT_HEIGHT_PITCH` | [(0.5, 0.5), (1.54, −3.25), (1.541, −3.5), (2.1, −6), (3.0, −8)] | [data] |
| `SHOT_BONUS_BY_IMPACT_HEIGHT` | [(0, 1), (1.2, 1), (1.5, 0.9), (1.8, 0.75), (2.2, 0.4)] | [data] |
| `SHOT_FORCE_BY_BALL_SPEED` | [(2, 0), (30, 3.25)] | [data] |
| `SHOT_BALL_DIR_RATIO` | [(−1, 1), (−0.5, 1), (0.05, 0), (1, 0)] | [data] |
| `VOLLEY_PUSH_BALL_SPEED` | 11.25 | [data] |
| `VOLLEY_PUSH_BALL_SPRINT` | 1.4 | [data] |
| `VOLLEY_PUSH_BALL_TURNED` | 0.9 | [data] |
| `VOLLEY_PUSH_BALL_HEIGHT` | [(0, 1), (3.0, 0.8)] at height − 0.15 m | [data] |
| `VOLLEY_PUSH_BALL_FRICTION` | `BASE_FRICTION` × 1.6 | [data] |
| `VOLLEY_PUSH_BALL_SPIN` | +1000 | [data] |
| `LOB_PUSH_BALL_SPEED` / `_ANGLE` | 11.5 / 65 | [data] |
| `LOB_PUSH_BALL_SPIN` | +2500 | [data] |
| `LOB_PUSH_BALL_COST` | 0.75 | [data] |
| `LOB_PUSH_BALL_NO_CONTROL` | 0.5 | [data] |
| `LOB_PUSH_BALL_WINDUP` | 22f ahead, 16f otherwise | [data] |
| `DRIBBLE_LOB_PUSH_BALL_SPEED` / `_WINDUP` | 12.5 / 16f | [data] |
| `VOLLEY_LOB_PUSH_BALL_SPEED` / `_ANGLE` | 13.5 / 75 | [data] |
| `VOLLEY_LOB_PUSH_BALL_HEIGHT` | [(0, 1), (1.79, 0.7), (3.0, 0.5)] | [data] |
| `VOLLEY_CONTACT_LEAD` | per body part, the "Contact" columns above | [data] |
| `VOLLEY_ACT` | per body part, the "Can act" columns above | [data] |

### 7.4 What the body needs (our own poses, described in words)
`body.rs` has no header, chest, thigh, jump, volley or flick poses. It also can't raise the root: the
root stays at 0.9 m.

**New clips, as `[windup, hit, through]` triples for the kick layer:**
- **Foot volley (pass and push ball):**
  - the standing leg planted beside the ball's line;
  - the kicking knee raised so the foot meets a bouncing ball at shin height;
  - the inside of the foot for the pass, the laces pushing forward for the push ball;
  - the hips opened toward the target for the side variants.
- **Thigh:** the knee lifted to hip height, the thigh angled toward the target, arms out for balance.
- **Pelvis (stomach):** the hips pushed forward, back arched, arms swept back.
- **Chest:**
  - knees bent, chest pushed out and up, shoulders back, arms wide;
  - for the push ball, the chest nudges forward.
- **Head:** knees bend then straighten, the neck nods through the ball, arms out.
- **Jumping header:**
  - crouch, then take off on both feet, body arched back;
  - snap forward at contact, then land on both feet.
  - It needs a root height offset over the airborne frames.
- **Volley shot, foot or thigh:**
  - the normal shot strike with the kicking knee over the ball and the body leaning back a little;
  - side volley: the trunk tilts away from the ball, the kicking leg sweeps across at hip height;
  - turning volley (behind): a pivot on the standing foot.
- **Jumping volley shot (scissor), from the clip [clip]:**
  - the knee comes up about 0.1 s before contact;
  - the body lies back almost flat and the kicking foot strikes at head height;
  - then the player falls onto their back and side and gets up.
  - It needs a root height curve and a fall pose.
- **Rainbow flick [guess, Kev to confirm the look]:**
  - the standing foot planted just ahead of and beside the ball;
  - the kicking foot's toes slide under the ball, then the ankle snaps up and the knee drives high, so
    the ball pops steeply up and forward;
  - the body stays upright with the arms out;
  - sideways: the same scoop across the body;
  - behind: the existing `heel` flick pose fits.

**Routing:**
- volleys pick the clip by the spot's body part and by the turn sector, through `keys(kind, turn)`;
- `follow_time` and `kick_size` need arms for each new variant;
- jumping volleys add a root height curve over the airborne frames, which needs a small change in the
  `animate_body` root handling.

### 7.5 The smallest set of changes, per step
1. **Rainbow flick (§4):**
   - `input.rs`: one line;
   - `player.rs`: the variant, start, chord, launch arm, cost and movement tables;
   - `tuning.rs`: the constants;
   - `body.rs`: one flick clip set.

   No volley code.
2. **Dribble flick (§5):** a second variant on the same code, plus the dribble gate.
3. **Volley plumbing (§7.2)** with the volley push ball (§3): flat ball, no charge, the touch poses.
4. **Volley lobbed push ball (§6):** the plumbing plus §4's launch.
5. **Volley pass (§1):** charge, pass aim, pass line, six touch poses including the jumping header,
   which needs the root height.
6. **Volley shot (§2):**
   - charge, camera, reticle, the standing and moving tables;
   - the incoming-ball and height bonuses;
   - the jumping volleys and their fall.

---

## 8. Corrections to our notes (found in this pass)
1. **mechanics.md §5, "Ball height thresholds for volleys":**
   - 60/120, 70, 75 and 80 cm only choose the **refusal** animation (ground or aerial) [code];
   - the body parts come from the height bands in §0.4 here.
2. **frame-data.md, "Volleys":**
   - "Ball contact: start" is the snap notify's start; the contact is the cancel window's end (§0.3);
   - the volley push ball's 26–30f is the "sprint/extra effort only" start; everything opens at
     34–42f (§3).
3. **mechanics.md §4:** volleys charge by **seconds held**: full at 0.25 s for both the volley shot
   and the volley pass. They have no build-up.
4. **mechanics.md §2:** the lobbed push ball wind-up is 6 m/s when moving and 4 m/s standing, not a
   flat 4 m/s.

## 9. Open questions
**For Kev:**
1. **The rainbow flick.**
   - Does it go about 2 m high and land about 5 m ahead after about 0.8 s, with the steep dip our model
     gives?
   - Is it a forward scoop (our pose guess) rather than a heel-over-head trick?
   - Does a sprint flick land further (about 7 m)?
2. **Your lobbed push ball key.** The default is C, or modifier + Back in the thumb preset. The
   overlay suggests M4 is your push ball.
3. **Volley timing.**
   - Does a volley strike when the ball arrives (we measured 0.37 s from the press on a jumping volley)?
   - Or can a push ball on a rolling ball be quicker (the 141.5 s clip looked 0.2–0.33 s)?
4. **Volley charge.** Do volley shots and passes reach full power after about 0.25 s of hold?
5. **Press while the ball is still coming.** Should the volley always beat the automatic control, as
   we assume?
6. **Jumping volleys.** How high do you leave the ground? Is the fall on the back after the scissor
   kick always there?
7. **Landing marker.** Should the flick show it (the prototype's `kick_marker` skips push balls)?

**In the data or the code:**
1. **The volley pass curve's time axis.** It reads as seconds (full at 0.25 s), but it is flagged
   `UseBuildUpDurationFromGlobalDataTable`. If the axis is normalised, full would be 0.1 s.
2. **When the volley animation starts.** Is it held back so its contact frame meets the ball, after a
   padding run, as we read `MoveToSnapLocation` (0.4 s) and the adaptive-snap configs? Or does it
   start at the press?
3. **The volley shot's contact frames** are inferred from the cancel windows (18–30f). No volley
   animation was extracted. Extracting one `Player_VolleyShoot_*` montage would settle it.
4. **Root motion and jump heights** of the volley animations aren't in the data we read.
5. **Free move with the ball:** both the normal and the dribble lobbed push ball are allowed. Which
   fires?
6. **The volley shot reference angle.** It is measured against camera forward. Is the other side your
   facing, or the direction to the ball?
7. **Standing volley shot above 2.5 m** falls back to the moving foot volley. Is that a data quirk, or
   is it unreachable under the 2.8 m cap?

## 10. Proposed build order
1. **Rainbow flick** (§4): on the ground, owns the ball, reuses the push ball path. It tests the 65° dip
   flight, which Kev can check first.
2. **Dribble flick** (§5): the same code, plus the dribble gate.
3. **Volley push ball** (§3): builds the shared volley plumbing (§7.2) with the simplest ball (flat,
   no charge).
4. **Volley lobbed push ball** (§6): the plumbing from step 3 plus the flick launch from step 1.
5. **Volley pass** (§1): adds charge, aim and the jumping header.
6. **Volley shot** (§2): the most parts. Charge, camera, standing and moving sets, scissor jumps and
   the incoming-ball bonus.

The aerial push balls (§3.4, §6) come after the ground ones.

## 11. What was built, and the decisions taken
Kev approved the spec without answering the §9 questions, so the data's values stand wherever there
is one; anything else is a guess marked TUNE in `tuning.rs`.

### 11.1 Rainbow flick (`Kick::LobbedPushBall`)
- **Input:** `LobbedPushBall` is live (C, RB + A, modifier + Back) and counts on the press or the
  release, like the push ball. A push ball started up to 0.1 s before RB becomes the flick.
- **When:** with the ball, outside a dribble; from free move or the stance (its layers allow both).
- **Ball:** §4's launch exactly (11.5 m/s at 65°, the way and sprint modifiers, exhausted 13 m/s at
  58°), spin.y +2500, base friction, no-control 0.5 s, cost 0.75, regen pause 1.0 s. In our model it
  peaks at about 2.2 m and first lands about 5.4 m on at 0.8 s.
- **Timing:** the touch 22f after the press ahead (within 45° of your facing), 16f sideways or back.
- **Movement:** §4's wind-up and after-touch rows as `Run`s; the stick takes you back from 35f after
  the press; the body turns along the ball's way over the 13f after the touch.
- **Guesses (TUNE):**
  - no kick marker, as for the push-ball family (the data doesn't say);
  - the sound is the lob's, not at full;
  - the poses (plant, scoop, knee drive) are ours, from the description in §7.4; back flicks use
    the heel flick.

### 11.2 Rainbow flick out of a dribble (`Kick::DribbleLobbedPushBall`)
- **When:** inside a dribble (any dribble, feint or tired one), from its 12th frame. Pressed earlier,
  it waits for the window, as the dribble push ball does. A dribble push ball started up to 0.1 s
  before RB becomes it.
- **Ball:** 12.5 m/s at 65° every way; sprinting x1.1 and 5° flatter; tired 12.75 m/s at 62° with no
  sprint boost. The rest as §11.1.
- **Timing and movement:** the touch 16f after the press; you keep the dribble's pace through it
  (5 → 6.5 m/s moving, 5 → 6 standing, 3.5 → 4.5 tired) and turn to the flick by the touch, then run on
  at your speed for 20f.
- **Which one in free move:** the spec rule stands (inside a dribble, this one; else §11.1).

### 11.3 The volley base, and the volley push ball (`Kick::VolleyPushBall`)
- **One target for all:** `fx::volley_target` (the diamond's query) now returns the best point, not
  the first: the one whose needed speed is closest to your own pace, preferring 0.4-0.6 s. That stands
  in for the picker's speed-difference and delay weights, whose exact scoring wasn't read
  [inferred]. The diamond only asks whether there is one, so it is unchanged.
- **Start:** with no ball, no kick, no control and no fake shot, a pending press (push ball) with a
  target starts the volley. The kick carries a `Volley`: the point, its time, the body part, the
  ball's side, the way to face and the jump.
- **Approach:** until the touch you run to where the body part meets the ball (`Volley::root`) at
  whatever speed gets you there (at most 9.2 m/s, 11 in extra effort), turning to the volley's way by
  the touch. No control takes the ball meanwhile.
- **Touch:** at the target's time, if the ball is still within 1 m of the point (else it misses).
  The green confirm follows the ball to the touch and fades there over 0.35 s.
- **Volley push ball:** §3 exactly: 11.25 m/s flat, x(1 → 0.8 by 3 m), x1.4 sprinting at the touch,
  x0.9 pushed 45° or more off your facing, clamped to 30° of it; friction x1.6, spin.y +1000,
  no-control 0.5 s, cost 0.375. You then run after it at 5.5 m/s (8.5 sprinting), the stick back
  from 12f after the touch.
- **Above 2.1 m:** the jumping push, 9 m/s at 40° (x0.7 sideways, x0.65 back). Chest up to 2.4 m,
  head higher: a guess, as the data's picker for it wasn't read (TUNE).
- **Guesses (TUNE):** where each body part meets the ball (`VOLLEY_CONTACT`); the body sets itself
  0.15 s before the touch; a jump leaves the ground 16f before to 22f after the touch, as high as the
  chest (1.35 m) or head (1.75 m) needs, 0.15-0.8 m; the touch poses are ours.

### 11.4 Volley lobbed push ball (`Kick::VolleyLobbedPushBall`)
- **Start:** the flick button with a volley target; or a volley push that RB joins within 0.1 s
  (its buffer layer), keeping the same meeting with the body part picked again.
- **Body part:** foot under 0.4 m, thigh under 0.8, stomach under 1.2, chest under 1.6, head to 2.1;
  above, the jumping flick (chest to 2.4 m, head higher: the same guess as §11.3). Off the foot it
  is the rainbow flick's scoop; higher, the touch poses lift it.
- **Ball:** §6 exactly: 13.5 m/s x r at 75° + r x -10°, r from the touch's height (1 on the ground,
  0.7 at 1.79 m, 0.5 at 3 m), sprinting at the press x1.1 and 10° flatter, tired 14.5 m/s at 68°;
  spin.y +2500, base friction, no-control 0.5 s, cost 0.75. Jumping (above 2.1 m): 10 m/s at 65°,
  x0.7 sideways, x0.65 back, no forced spin.
- **After:** you turn after the ball at 3 → 5.25 m/s (5 → 7.5 coming in faster than 1 m/s) for 22f.
- **Not built:** the kicker's 0.8 s low priority on his own flick (one player, no contest).

### 11.5 Volley pass (`Kick::VolleyPass`)
- **Start:** pass pressed or still held while the volley target exists (so a press shortly before
  the target appears also counts, as the pass report saw in Kev's recordings). The facing is kept
  (the game's volley pass snap keeps the actor's forward).
- **Charge:** by seconds held, full at 0.25 s (the open question on the time axis is answered with the
  data read as seconds; TUNE). The touch can't wait for a release: still held, it takes the charge
  so far.
- **Body part:** foot under 0.2 m, thigh under 0.7, stomach under 1.2, chest under 1.7, head under
  2.3, then the jumping header. Off the foot it is the pass's inside-of-the-foot pose.
- **Ball:** §1 exactly: the squeezed pass curve (20 → 32 → 25.6 → 30.65 m/s), the height factors
  (lifted past 69% or flat), lift +3.9 → 6° past 69%, pitch down to -8° by 2.9 m; the pass's aim,
  friction x1.2 and forced lift; hitstop 3 frames; no stamina.
- **After:** you slow from your approach to 30% of it until you can act (foot 10f, thigh 9, stomach
  13, chest 15, head 16, jumping header 31 after the touch). The 30% is a guess (TUNE).
- **Headless check hook:** `RE_BALL=t:x,y,z:vx,vy,vz` throws the ball at `t` s in a scripted run, to
  set up high balls for headers and volleys.

### 11.6 Volley shot (`Kick::VolleyShot`)
- **Start:** shoot pressed or held with a volley target. You turn to the shot (the camera's aim at the
  press) by the touch, as the game's snap places you along the shot's way. The shot camera, the aim
  lock, the mouse curve and the reticle work as for a shot.
- **Charge:** by seconds held, 24 m/s to 0.05 s, 38 m/s at 0.25 s; still held at the touch, it takes
  the charge so far.
- **Body part:** §2's standing and moving tables (the stomach for a thigh-high ball shot 40-125° off
  your facing, standing). Above the top band, the jumping head: the data's fallback (the moving foot
  volley) can't reach a ball that high, so that one is changed on purpose.
- **Poses:** the half-volley is the shot's strike; thigh, stomach and chest a volley swept across the
  body (the side shot's keys, mirrored for a ball on the left); the head a header; both jumping ones
  the scissor kick from Kev's recording (knee up, body flat, foot at head height, the fall).
- **Ball:** the force, plus the shot's run-up bonus (kept down off a high touch: x1 to 1.2 m, 0.4 at
  2.2 m) and the incoming ball's pace sent back (up to +3.25 m/s at 30 m/s, when the shot goes back
  roughly the way the ball came), both scaled by the charge as the shot's run-up bonus is; the
  camera's elevation plus §2's height pitch (+0.5° at 0.5 m to -8° at 3 m); the shot's spin by
  seconds held against 0.25 s; hitstop 5 frames at every power; no stamina.
- **After:** you slow out of the approach until you can act (30f after the touch, chest 35, jumping
  chest 33, jumping head 43). The fall after the scissor kick gets up sooner than the 0.5 s Kev's
  recording shows (TUNE).
- **Not built:** the shot's pitch-by-force curve and its against-the-run penalty (the prototype's
  normal shot has neither); the analog trigger's slower fill (our input is digital).

### 11.7 All the guesses, and what is left
**Guesses (each marked TUNE in `tuning.rs` or here):**
- the flicks show no kick marker, like the push-ball family;
- the flick sounds like a lob, not at full;
- every pose of the six moves is ours (plant and scoop, the touches, the jumping header, the scissor);
- where each body part meets the ball (`VOLLEY_CONTACT`), the 0.15 s set before a touch, the jump's
  window (16f before to 22f after) and height (to reach 1.35 m chest or 1.75 m head, 0.15-0.8 m);
- the jumping push and flick take the ball with the chest up to 2.4 m, the head higher;
- the volley target's choice (closest to your pace, preferring 0.4-0.6 s);
- the volleys' charge time axis read as seconds (full at 0.25 s);
- the 30% you slow to through a volley pass's or shot's recovery, and the scissor's quicker get-up;
- a volley misses if the ball is more than 1 m from where it was to be met;
- standing, a volley shot above 2.5 m is the jumping head (the data's fallback is the moving foot
  volley).

**What is left:**
- the exhausted volley push ball's own data (it uses the normal one's);
- the jumping push and flick's height force modifier (BP_FarVolleyControl, not read);
- the kicker's low priority on his own volley flick, refusals and their red flash, the off-screen HUD
  diamond;
- the low-power volley pass and the volley lob (other abilities, not asked for);
- the analog trigger's fill for volley shots;
- a tune pass with Kev against his recordings: flick height and distance, volley timing, jump heights.

**Strips (scratchpad, headless):** `vf_flick`, `vf_flick_side`, `vf_dribble_flick`, `vf_volley_push`,
`vf_volley_flick`, `vf_volley_pass`, `vf_volley_pass_high`, `vf_volley_header`, `vf_volley_shot`,
`vf_volley_scissor`.

## Sources
**Extracted game files** (`RE Rematch (main)/tools/rmx/out` and `out_bp`):
- **Abilities:** `GA_VolleyBaseShoot`, `GA_VolleyShoot`, `GA_VolleyTap` (+ `_LowPower`),
  `GA_VolleyLobTap`, `GA_VolleyPushBall`, `GA_VolleyAerialPushBall`, `GA_VolleyLobbedPushBall`,
  `GA_VolleyAerialLobbedPushBall`, `GA_PushBall`, `GA_LobbedPushBall`, `GA_DribbleLobbedPushBall`,
  the `GA_Exhausted…` set.
- **Ability data:** `BP_VolleyShootData`, `BP_VolleyTapAbilityData_AnalogPower`,
  `BP_VolleyPushBallAbilityData`, `BP_VolleyLobbedPushBallAbilityData`,
  `BP_VolleyAerial(Lobbed)PushBallAbilityData`, `BP_LobbedPushBallAbilityData`,
  `BP_DribbleLobbedPushBallAbilityData`, `BP_Exhausted(Volley)LobbedPushBallAbilityData`,
  `BP_PushBallAbilityData`.
- **Modifiers:**
  - `BP_VolleyShoot_PitchDependingBallHeigh`;
  - `BP_VolleyTap_*`, `BP_VolleyTapAnalogPower_PitchDependingForce`;
  - `BP_VolleyFastTap_*`;
  - `BP_VolleyLobbedPushBall_ForceFromHeight` / `_ForceFromSpeed`;
  - `BP_LobbedPushBall_ForceFromDirection` / `_AngleFromDirection` / `_ForceFromSpeed`;
  - `BP_DribbleLobbedPushBall_SprintForceModifier`;
  - `BP_Shoot_BallCharaOrientationModifier`;
  - the Magnus and friction trajectory modifiers.
- **Curves:** `Default__BP_ShootForceFromInputCurve`, `Curve_VolleyShootFillFactorByAnalogPressValue`,
  `Curve_VolleyTapPitchModifierDependingTime`, `VolleyPushBall_HeightForceRatio`,
  `VolleyLobbedPushBall_HeightForceRatio`, `ShootForceModifierClampByBallHeight`,
  `ShootForceByBallSpeed`, `ShootOrientationRatioBallSpeed`.
- **Pickers:** `BP_VolleyShootMontagePicker`, `BP_VolleyTap(Buildup)MontagePicker`,
  `BP_VolleyPushBall(Buildup)MontagePicker`, `BP_VolleyLobbedPushBall(Buildup)MontagePicker`,
  `BP_LobbedPushballMontagePicker`, `BP_DribbleLobbedPushballMontagePicker`.
- **Targeting:** `BP_VolleyTargetDatasource`, `BP_VolleyGroundedTargetDataSource`,
  `BP_VolleyAerialTargetDataSource`, `BP_VolleyTargetPicker`, `BP_VolleyShootTargetPicker`,
  `BP_VolleyLobPushBallTargetPicker`.
- **Tables:** `VolleyThresholdDataTable`, `ShootDataTable`, `DT_VolleyTap`, `DT_VolleyPushBalll`,
  `DT_VolleyLobbedPushBall`, `DT_VolleyAerialPushBall`, `DT_LobbedPushBall`,
  `PushBallOrientationDataTable`, `CT_AbilitiesCost_Stamina`.
- **Metadata and configs:** `BP_*VolleyShootAnimMetadata`, `BP_Ground/Aerial_VolleyTapAnimMetadata`,
  `BP_VolleyPushBallAnimMetadata`, `BP_VolleyLobbedPushBallAnimMetadata`,
  `BP_LobbedPushBallAnimMetadata`, the `BP_(BuildUp_)Volley*Configuration` and `*Snap` assets.
- **Movement:** `LobbedPushBall*BuildUp/DashData`, `BP_DribbleLobbedPushBallBuildUpData`,
  `DribbleLobbedPushBallDashData`, `Volley(Lobbed)PushBallDashData`,
  `BP_RedirectVolley*_ScaleSpeedData`.
- **Statuses and effects:** `BP_NoControl_*`, `BP_NoVolley_*`, `NoVolley_Shoot_BallStatus`,
  `BP_BallStatus_NoSlidingTackle_LobPushBall`, `BP_LowPriority_VolleyLobbedPushBall`, `GE_Cost_*`,
  `GE_Cooldown_StandingTackle_After*`, `ALBuffer_*`, `AL_OnlyExtraEffort`.
- **Bindings:** the default gamepad mapping, plus controls.md in the main notes for the keyboard.

**Earlier passes reused (session scratch, not committed):**
- the volley diamond pass (target rules, diamond and confirm looks);
- the defence and rainbow-flick draft (lobbed push ball windows, movement rows, HasLostBall);
- the gameplay FX pass (trail and aim-marker rules).

**Notes:**
- here: mechanics.md, frame-data.md (+ csv), hitboxes.md, clip-measurements.md;
- the main worktree: controls.md, dribble.md, ball-flight.md, camera.md, responsiveness.md.

**Recordings:** `Videos/2026-10-25 02-33-58.mkv` (141.4–142.0 s) and `02-55-47.mkv` (120.8–122.0 s),
frames read with ffmpeg. Only timings and body shapes were noted.

**Prototype read:** `player.rs`, `input.rs`, `fx.rs`, `ball.rs`, `tuning.rs`, `body.rs`, `camera.rs`,
`reticle.rs`, `pass_line.rs`. Flights were simulated with a port of `ball::step`.
