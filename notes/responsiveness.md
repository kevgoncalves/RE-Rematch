# Responsiveness: from the press to the move

When a press counts, when the move starts, when the ball is touched, which windows open, and when you get control back, for dribbles, feints, push balls and dashes. The last section lists where the prototype differs.

60 fps: 1 frame (f) = 1/60 s. Frames count from the press. Speeds are in m/s.

**Status:** reviewed by Kev and coded in the prototype (2026-10-24), except #8 (push ball wind-up on rails) and #9 (blended body turns). Interpreted from the ability Blueprints, the input condition assets, the montages and their metadata, and the native code for the defaults and the move interrupt. Not checked in-game. A few rules lean on engine defaults; they're flagged.

## The rules in short
- **A move starts on the frame of the press.** The ability runs on your own machine at once (no wait for the server), plays its animation that frame, and sets its speed from its curve that frame.
- **Tap moves (dribble, feint, push ball, dash) count on the press, or on the release.** If the move isn't allowed when you press, letting go of the button once it is allowed still fires it. There is no timed input buffer.
- **Hold moves (pass, shot, lob) fire on the first frame they're allowed while the button is down.** Holding pass through a blocked moment gives the pass the moment it opens.
- **After a push ball, pass or tackle starts, a 0.1 s window lets the longer chord take over** (lobbed push ball, low-power pass, short lob, sliding tackle). That window is what the files call the "buffer".
- **Control comes back when the animation starts to blend out**, not when it ends: 15 frames early for most dribbles, 18 for dashes.
- **Some moves can be left early with the stick** (a "move interrupt"): push balls from 23f if you steer within 30° of where you're running, dashes from 14.4f if you keep the dash's direction. Dribbles can't.

## 1. When a press counts

### Input conditions
Each action is a small tree of conditions on the buttons and the stick (the gamepad preset; the keyboard presets use the same conditions).

| Action | Used by | Fires when |
|---|---|---|
| Dribble input (A / Space) | dribble, feint, tired dribble | press with a direction, press alone, **or release** |
| Push ball input (A / Space) | push ball, dribble push ball | same |
| Dash input (LT + A / Ctrl + C) | dash, second dash | lock key held, and A pressed (with a direction), held, or released |
| Lock input (LT / Ctrl) | lock move, stance | held (a toggle option exists) |
| Pass (X / right mouse) | pass | held |
| Shot (RT / left mouse) | shot, shot after a dribble | trigger past 10% |
| Lob, low-power pass, lobbed push ball | | buttons held together |
| Extra effort | | release, then press again within **0.15 s**, and hold |

- **Direction:** the stick (or WASD) counts from **0.3** tilt and is full at **0.9**. Below 0.3 the press has no direction, which gives the forward move.
- The press and the direction are read **on the same frame** and stored with the move.
- The two A-button actions are separate: whether A dribbles or pushes the ball depends only on the move's tags. The dribble and the feint need lock move held; the push balls are blocked by it.

### No timed buffer
- The dribble abilities have **no buffer time** (the native default is 0), and the condition lifespans are 0.
- A tap made too early is dropped, unless you let go of the button after the move becomes allowed: then it fires on the release frame. In practice a normal tap (5–7 frames down) carries a few frames of slack.
- Hold moves don't need a buffer: they fire as soon as they're allowed, as long as you're still holding. This is why passes and shots "out of a dribble" feel instant: held from the start, they come out at 12f.
- **Correction to [frame-data.md](frame-data.md#input-buffer):** its "Input buffer" table is the chord window below, not a buffer for presses made before the move is allowed.

### The chord window
When an ability with a buffer time starts, it adds a few permissions on top of its own for that long.

| Ability starting | For | Still allowed |
|---|---|---|
| Push ball | 0.1 s (6f) | lobbed push ball, low-power pass, short lob |
| Pass | 0.1 s | lob, low-power pass, short lob |
| Lob, lobbed push ball, low-power pass | 0.1 s (inherited) | short lob |
| Standing tackle | 0.1 s | sliding tackle |
| Sliding tackle | 0.1 s | standing tackle |
| Volley push ball / volley pass | 0.1 s | the lobbed and low-power variants |
| Low-power lob | 0 | nothing |

So pressing A a moment before X still gives the low-power pass: the push ball starts, then the chord takes over within 6 frames.

### Prediction and network
- **Predicted:** the abilities keep the engine's local prediction. Only the server may end a dribble early, but the client starts it on the press.
- **Push ball sync:** the kick waits for a network sync of at least 0.05 s, shorter than the 8f (0.13 s) wind-up, so it never delays the touch.
- **Server catch-up:** up to 0.2 s.
- **No input delay:** the input delay component is a test and replay tool, not used in normal play.

### Lock move with the ball (stance)
- **Lock move is on from the press frame.** So lock + dribble pressed together works.
- **The release is only read after 0.15 s**, so a quick tap gives at least 0.15 s of stance.
- **Losing the ball ends it at once.**
- The stance removes sprint and extra effort.
- **Releasing lock during a dribble doesn't stop the dribble.** It only decides what the next A press does:
  - still held (or within the 0.15 s): the feint;
  - released: the dribble push ball.

## 2. From the press to the move
"Free" is when the animation starts to blend out: the ability ends there and normal movement takes over. With the engine's default blend-out of 0.25 s, that is 15 frames before the end.

### Dribbles
| Move | Length | Ball touched | Actions from | Free | Speed at 0f |
|---|---|---|---|---|---|
| Fast cut | 54f | 17f | 12f | **39f (0.65 s)** | 4.0 |
| Side | 50f | 8f | 12f | **35f (0.58 s)** | 4.0 |
| Drag back | 64f | 0f | 12f | **49f (0.82 s)** | 3.0 |
| Feint forward (roulette) | 65f | 10f | 12f | **56f (0.93 s)** | 4.0 |
| Feint right | 50f | 8f | 12f | 35f | 4.0 |
| Feint left | 50f | 8f | 12f | 41f | 4.0 |
| Feint back | 53f | 8f | 12f | 38f | 3.0 |
| Tired, forward / side / back | 65 / 63 / 67f | 8f | 12f | 50 / 48 / 52f | 4.0 |

- **The move starts at full curve speed on 0f.** The speed comes from the curve, not from your run-up: no ramp from your current speed.
- **Pose:** it blends in over 3f (the feint over 6f). This is visual only.
- **The speed curves span 0–45f.** The fast cut hands back at 39f, so its last 6 frames of curve never play.
- **Actions from 12f** are listed in §3. They last to the end of the animation, after you're free.

### Push balls
| Move | Ball touched | Stick exit from | Free at the latest | Turn toward the push |
|---|---|---|---|---|
| Push ball | **8f (0.13 s)** | **23f (0.38 s)**, within 30° | 34–49f by direction | 8–15f (right 11–26f, back 14–30f, back-right 20–32f) |
| Dribble push ball | 8f | 23f, within 30° | 34–59f by direction | **1–9f**, before the touch |

- **Free at the latest**, by animation:
  - push ball: forward 40f, forward diagonals 34f, right 37f, left 49f, back 35f, back-right 42f;
  - dribble push ball: forward 37f, forward-left 40f, forward-right 49f, right 34f, back 59f.
- **Up to the touch you keep your run-up speed (×1.0).** The push ball's chase speed starts at the touch (dribble.md has the speeds).
- **The dribble push ball** surges to ×1.3 of your speed early in the wind-up, then eases off toward the touch, where its own chase speed takes over.
- **No hitstop:** the push ball's freeze at the touch is set to 0 frames.

### Dashes (without the ball)
| Move | Length | Dive or standing tackle from | Everything but kicks, and stick exit, from | Free at the latest | Kicks from |
|---|---|---|---|---|---|
| Dash forward / back | 50f | 10.8f (0.18 s) | **14.4f (0.24 s)** | **32f (0.53 s)** | 50f |
| Dash sideways | 41f | 10.8f | 14.4f | **23f (0.38 s)** | 41f |
| Second dash forward | 60f | 10.8f | 25.2f (0.42 s) | 42f | 60f |
| Second dash sideways | 42f | 10.8f | 25.2f | 24f | 42f |
| Second dash back | 43f | 10.8f | 25.2f | 29f | 43f |

- **The animation blends in over 6f.** The dash's speed curves run in **seconds** from the press, not over the animation (their keys fall on the 6f blend-in, the 14f after-dash and the second dash's 42f free):
  - first dash: 6 m/s, 8.5 from 0.1 to 0.233 s, 6.5 at 0.331 s, 4.5 from 0.5 s;
  - second dash: 6 m/s, 7.2 from 0.1 to 0.3 s, down to 2 at 0.7 s. So at any moment it has covered less ground than a first dash, whichever way it goes (players confirm it is always shorter).
- **Back in free move with the stick against the dash** (lock released), you turn as from a run: brake to the pivot speed, then re-accelerate the other way (players confirm the change of side is smooth).
- **The second dash** cuts into the first from 14.4f, and also works after the first ends while its 0.8 s cooldown runs. It doesn't need lock move.
- **From 14.4f** you can also lock move, sprint, extra effort and dash again. Kicks wait for the end of the animation.
- **So in the game a dash holds you for 0.4–0.5 s at most**, and only 0.24 s if you keep pushing the way you dashed.

## 3. Windows and cancels
The permissions come from layers. The latest layer pushed on a stack replaces the one before it; the chord window adds to it.

| Moment | Layer | You can |
|---|---|---|
| Dribble 0–11f | the ability's own | move the camera, nothing else |
| Dribble 12f to the end of the animation | dribble layer | feint, pass, lob, short lob, shot after a dribble, dribble push ball, dribble lobbed push ball, sprint, extra effort, lock move. No new dribble, no normal push ball or shot, **no stick exit** |
| Feint 12f to the end | feinted dribble layer | the same, minus the feint and the short lob |
| Tired dribble 12f to the end | tired dribble layer | the same, minus the feint |
| Push ball 0–22f | the ability's own | move the camera, sprint, extra effort, hinder |
| Push ball 23f to the end | everything except the dribble-only moves | anything, and the stick exit within 30° |
| Dash 0–10f | the dash's own | lock move, ball control, hinder |
| Dash 10.8f | dive and tackle | dive, standing tackle, lock move, ball control |
| Dash 14.4f to the end (second dash 25.2f) | after-dash | everything except kicks, volleys and push balls, and the stick exit |

### Chains
| From | Into | From frame |
|---|---|---|
| Dribble | feint (lock held, same three directions) | 12f |
| Dribble, feint | dribble push ball (lock released) | 12f |
| Dribble, feint | pass, lob, shot after a dribble | 12f (held presses fire then) |
| Dribble, feint | extra effort (touches the ball on) | 12f |
| Dribble | new dribble | only after the animation ends (54 / 50 / 64f) |
| Feint, tired dribble | another feint | never |
| Push ball | low-power pass, lobbed push ball, short lob | first 0.1 s (the chord window) |
| Push ball | anything | 23f, but the ball is gone, so dribbles need a control first |
| Dash | second dash, dribble, lock move, sprint | 14.4f |
| Dash | dive, standing tackle | 10.8f |
| Any shot, pass or push ball in progress | dribble | blocked until it ends |

## 4. Direction
- **Read once, at the press**, and stored with the move. The stick after the press doesn't steer it.
  - Dribbles and dashes pick their animation from that direction.
  - Push balls also aim the ball with it: the push ball's direction comes from the input stored at the press, not the stick at the touch.
- **Camera-relative.** The direction is measured against the camera's forward, not the body's.
- **Sectors:**
  - dribbles: forward within 45°, sides to 135°, back beyond (dribble.md);
  - dashes: front, sides or back;
  - dribble push balls: six sectors.
- **The move follows your exact input, but the body faces by sector:**
  - forward moves face the input;
  - side moves face 90° off it, so you keep looking forward and slide;
  - back moves face away from it, so you keep looking forward and back off.
- **The turn into that facing is blended, not snapped:**
  - dribbles over **0.2 s (12f)**, linearly (the montage player's default);
  - dashes over **0.15 s (9f)**, fast at first.
- **Push ball aim probably snaps** near the cardinal directions: the push ball's orientation table carries the passes' tolerances, 6° around straight ahead and 22.5° for the others (how it applies them wasn't traced).
  - With no direction the push ball goes along your body.
  - Stick tilts under **0.1** don't count for it.

## 5. Movement: start and hand-back

### Start
- **Dribbles and feints:** speed set from the curve on 0f (4.0 or 3.0 m/s, peaking at 5.8–6.8). Your run-up doesn't matter.
- **Push balls:** your run-up speed, unchanged, until the touch; then the chase.
- **Dashes:** 6 m/s on 0f, 8.5 by 10% of the animation.
- **Steps:** the speed modifiers integrate in 1/60 s steps.

### The stick exit (move interrupt)
Only inside a layer that allows it. Every frame the game checks your stick against the move, and leaves the move into free running when all of these hold:
1. **Moving matches pushing.** If the move is still carrying you (0.5 m/s or more) and you aren't pushing the stick, you stay in the move. Pushing during a part where the move stands still doesn't exit either.
2. **Direction:** your stick is within the angle of the move's current motion:
   - **15°** by default;
   - **30°** on push balls.
3. **Speed:** the speed your stick asks for is no faster than what the move is doing.
4. **Facing:** where you face is within 15° of where you move. This probably keeps side and back moves on until they end (our reading).
5. **Orientation:** the orient-to-movement state hasn't changed.

The dribble's layers switch every check off and don't allow the exit at all.

### Hand-back at the blend-out
- **The ability ends when its animation starts to blend out**, or on a stick exit.
  - The blend-out is 0.25 s for most animations here, and 0.15–0.35 s for some.
  - From then on normal movement drives you, starting from the move's last speed, with the usual acceleration and turn rules (mechanics.md §2).
- **The move's permissions last until the animation's real end.** So after a fast cut you run freely from 39f, but the feint and the dribble push ball stay available until 54f, and a new dribble only comes after that.

## 6. Other things that make it feel immediate
- **No ramp-up:** dribbles and dashes start at speed on the press frame.
- **Ball touches are early:** at 8f for most moves, 0f on the drag back.
- **Held kicks fire on the first allowed frame**, so pressing early is never punished.
- **Releases count for taps**, so mashing the feint a little early still works.
- **Stick exits** cut push balls (23f) and dashes (14.4f) short when you keep going the same way.
- **Hand-back at the blend-out**, a quarter second before the animation ends.

## Prototype differences
Ranked by how much each one hurts responsiveness. "Prototype" is `game/src` on game/prototype as of 2026-10-24.

| # | Prototype | Game | Effect in play |
|---|---|---|---|
| 1 | Dribbles hold you on their speed curve until the animation's end: 54 / 50 / 64f, feints 65 / 50 / 53f, tired 65 / 63 / 67f | Free at the blend-out: 39 / 35 / 49f, feints 56 / 35–41 / 38f, tired 50 / 48 / 52f | **15 frames (0.25 s) of dead stick after every dribble.** The most likely cause of "doesn't respond" |
| 2 | The dash runs its curve for 50f in every direction (both dashes), with no stick exit | Free at 32f forward and back, 23f sideways; second dash 42 / 24 / 29f. Keeping the dash's direction exits at 14.4f (second: 25.2f) | **18–27 frames (0.3–0.45 s) too long on rails**, 35 frames if you hold the direction: "the dash keeps you locked" |
| 3 | The push ball's direction is the stick at the touch (8f after the press) | The stick at the press | Flick-then-release push balls go the wrong way, or straight on. Feels laggy |
| 4 | Taps count only on the press frame; an early press is lost (except the push ball's own early buffer in a dribble) | A tap fires on the press, or on the release if the move became allowed. Hold moves fire as soon as allowed while held | An early feint, a push ball pressed during a control, and a pass held through a control all get eaten |
| 5 | No chord window: the first button commits (A then X gives a push ball) | 0.1 s after a push ball or pass starts, the longer chord takes over | Slightly staggered chords give the wrong kick |
| 6 | After a push ball the chase ends at 23f unless you steer more than 30°, even with no stick. A sharp turn waits to 55f | Stick within 30° exits at 23f. With no stick the chase goes on to the blend-out. A sharp turn frees you at 34–49f | Sharp turns after a push ball wait 6–21 frames too long. Releasing the stick stops the chase too early |
| 7 | Any stick tilt gives a direction | Under 0.3 tilt is no direction (forward) | On a pad, a slight tilt turns a fast cut into a side dribble or drag back |
| 8 | The push ball wind-up (0–8f) is free running, and you can steer | Your run-up speed on rails until the touch | Small. The prototype is looser here |
| 9 | Fast cut body turn at 1440°/s; side and back moves snap to the camera's facing; dashes don't turn the body | Body turn blended over 12f (dribbles) or 9f (dashes). Side moves face 90° off the input | Visual. The prototype turns faster, not slower |
| 10 | Extra effort double tap within 0.3 s | Within 0.15 s of the release | Sprint taps can turn into extra effort by accident |
| 11 | A push ball pressed in a dribble's first 12f waits for 12f | Needs the press or the release at 12f or later | The prototype is more lenient. Keep it, or match the release rule |

## Open questions
1. **Blend-out hand-back.** It relies on the engine's rule that a montage stops driving the character when it starts to blend out. Sloclap's movement layer wasn't traced, so check in-game: does a fast cut give the stick back around 0.65 s?
2. **Stick exit settings.** The 15° default and 0.5 m/s come from the settings class; the game's config file (not in the extracted files) could override them.
3. **Exits on side and back moves.** Does the facing check block the stick exit on side and back dashes and push balls?
4. **Release rule.** Is a release after an accepted press consumed? The condition resets on activation in our reading. And does holding the dash button re-fire the second dash at 14.4f?
5. **The dash's forced exit speed** at the end of its speed track: the data asks for one but doesn't set a value.
6. **Push ball snapping.** How exactly the orientation table's 6° and 22.5° tolerances bend push balls, and what "keep the last direction" does with no stick.

## Sources
**Native code** (Rematch build of 2026-10-24):

| Function | What it is |
|---|---|
| `FUN_1448a4530` | SCGameplayAbility constructor: buffer time 0, trigger "inputs" |
| `SCGameplayAbility__vf114` (`1448b0070`) | ability activation: pushes the ability's layer, then the chord window layer for the buffer time on a timer |
| `SCGameplayAbility__BPF_PopBufferAvailabilityLayer` (`1448ab340`) | removes the chord window layer |
| `FUN_144784ce0` | input sub-condition constructor: lifespan 0 |
| `FUN_144b12da0` | availability layer notify constructor: all stick exit checks on by default |
| `FUN_144812a00` | SCGameplayAbilitySettings constructor: stick exit 15°, 0.5 m/s, 2-frame look-ahead; network catch-up 0.2 s |
| `FUN_1448e1a70` | the stick exit check (moving vs pushing, direction, speed state, facing, orientation, other montages) |
| `FUN_14474be30` | montage player configuration constructor: redirection over 0.2 s, linear |
| `FUN_142f644e0` | engine montage constructor: blend-in and blend-out 0.25 s, auto blend-out on |

Constructors were found from each class's registration (`tools/usmapgen.py` tables). Property offsets come from the reflection tables.

**Data:**
- **Abilities:** GA_Dribble, GA_FeintedDribble, GA_DribbleExhaust, GA_PushBall, GA_DribblePushBall, GA_Tap and the other buffered abilities, GA_DribbleStance, GA_Dash and GA_SecondDash (bytecode and defaults).
- **Input:** DefaultGamepadMapping_Gameplay; the conditions ActionWithDirectionInput, CombinationActionWithDirectionInput, JustPressInput, ReleaseInput, HoldInput, LocoVectorInput, ShootInput and QuickPressAndHold.
- **Layers:** AL_Dribble, AL_FeintedDribble, AL_DribbleExhaust, AL_DribbleStance, GenericNone, GenericNone_ButHinderAndSprint, GenericAll, GenericAll_AfterDash, GenericNone_ButDiveAndTackle, AL_Dash and ALBuffer_*.
- **Animations:** the dribble, feint, tired dribble, push ball, dribble push ball, dash and second dash montages (notifies, blend times); BP_DribbleAnimMetadata, BP_FeintedDribbleAnimMetadata, DT_Dribble, BP_DribblePlayerConfig and BP_RedirectCardinalsToForward; BP_PushBallAnimMetadata, PushBallAvailabilityLayer, FreezeFrame_PushBall and DT_PushBall.
- **Movement and aim:** PushBallBuildUpData, DribblePushBallBuildUpData; BP_PushBallAbilityData and BP_DribblePushBallAbilityData (direction computation), TargetDataManifest_PushBall, PushBallOrientationDataTable.
- **Dash:** BP_DashAnimMetadata (Dash/Animations), BP_SecondDashAnimMetadata, BP_DashMontagePlayerConfiguration, DashAnimPicker, DT_Dash, DashRootModifierData_ScaleSpeed_Base and SecondDashRootModifierData_ScaleSpeed_Base, GE_Cooldown_Dash and GE_Cooldown_SecondDash.

**Prototype (read only):**
- `game/src/player.rs`: the dribble and dash branches of `player_move`, and `kicking`;
- `game/src/input.rs`: `just_pressed`, chord resolution, the double tap;
- `game/src/tuning.rs`: the DRIBBLE_*, FEINT_*, PUSH_BALL_* and DASH_* constants.
