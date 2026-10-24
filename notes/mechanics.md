# Rematch mechanics: the numbers behind the game

Read straight from the game files (Steam build, October 2026): 1119 gameplay assets, plus 766 animations for the frame data.

Related notes:
- [frame-data.md](frame-data.md): cancel windows and when you can act again.
- [hitboxes.md](hitboxes.md): reach and hitbox sizes.
- [match-camera-network.md](match-camera-network.md): rules, camera, network priority.

## How to read this
- **Time:** the game runs gameplay at **60 frames per second**.
  - Verified: the standing tackle hitbox starts on frame 7 in the data table, and its animation fires the hit at 0.1167 s = 7/60.
  - Everything is converted to seconds: 6 frames = 0.1 s, 30 frames = 0.5 s, 60 frames = 1 s.
- **Units:** speeds in m/s and km/h, distances in metres.
- **"Standing" vs "moving":** many actions have two versions. "Standing" means below 3 m/s.
- **(unconfirmed):** the number is exact, but what it means in-game is our interpretation.

---

## 1. Stamina

The bar goes from **0 to 10**.

### Cost per action
| Action | Cost | % of bar |
|---|---|---|
| Sliding tackle | **3.0** | 30% |
| Standing tackle (the file calls it "Block", unconfirmed) | 1.5 | 15% |
| Fake shot | 1.5 | 15% |
| Dive (outfield) | 1.25 | 12.5% |
| Dive (goalkeeper) | **0** | free |
| Dash | 0.75 | 7.5% |
| Lobbed push ball | 0.75 | 7.5% |
| Push ball while dribbling | 0.6 | 6% |
| Dribble | 0.5 | 5% |
| Start sprinting | 0.4 | 4% |
| Push ball | 0.375 | ~4% |
| Keep sprinting | 0.75/s (unconfirmed) | |
| Keep sprinting with the ball | ~1.1/s (unconfirmed) | |

### Regeneration
- Regenerates **4 per second**, so empty to full takes 2.5 s.
- Regen only starts after a pause that depends on your last action:

| Last action | Pause before regen |
|---|---|
| Normal | **0.4 s** |
| "Medium" | 0.6 s |
| Dribble | **0.9 s** |
| Push ball | 1.0 s |
| "Long" | 1.36 s |

### Exhausted (bar at 0)
- **Regen:** starts only after **2 s**, at half speed (2 per second).
- **Leaving the state:** you stay exhausted until the bar is back to **3**.
- **Sprint cap:** **6 m/s without the ball** (instead of 8.75) and **5 m/s with it** (instead of 6.5).
- **Slower recovery:** fake shots and dribbles take longer to recover from (see [frame-data.md](frame-data.md)).

### Stamina changes your speed
This is a hidden detail.

| Stamina | Speed without ball | With ball |
|---|---|---|
| 8 to 10 | **105%** | 100% |
| 6 | 100% | 100% |
| 5.9 | 95% | 95% |
| 0 | **80%** | 80% |

- **Above 8 stamina you are 5% faster.**
- Below 6 you lose 5% straight away, down to -20% with an empty bar.
- With the ball you never go above 100%.
- It appears to apply to sprinting without the ball and to running with it (unconfirmed).

---

## 2. Movement speeds

Top speeds come from the character's locomotion attributes (DefaultCharacterLocomotionAttributeTable). The speed tables read those attributes, not their own fallback numbers, which is where older figures here came from; the out-of-stamina effect overrides the same attributes with the tired speeds.

| Situation | Top speed | km/h | Acceleration |
|---|---|---|---|
| Walk | 2.5 m/s | 9 | |
| Run (stick fully pushed) | **5 m/s** | 18 | 18 m/s² (0 to 5 m/s in ~0.3 s) |
| **Sprint** | **8.0 m/s** | **28.8** | |
| Run with ball | 4.5 m/s | 16 | |
| **Sprint with ball** | **6.5 m/s** | **23.4** | |
| Extra effort | **9.5 m/s** | 34.2 | **100 m/s² (instant)** |
| Sweeper extra effort | **11.5 m/s** | 41 | |

### Turning
| Situation | Max turn rate |
|---|---|
| Run | 400°/s |
| Run with ball | 360°/s |
| Sprint | 180°/s |
| Extra effort | 150°/s |
| **Sprint with ball** | **90°/s** |

Sprinting with the ball turns 4× slower than running with it. To turn sharply, leave the sprint.

### Starting, stopping and turning
Every direction change is a short transition: you **brake**, then you **re-accelerate**, and you leave the turn at a reduced speed.

**From standing to full speed, and from full speed to a stop:**

| State | Reach full speed | Stop |
|---|---|---|
| Run | 0.16–0.25 s | 0.38 s |
| **Sprint** | **0.6 s** | **0.68 s** |
| Run with ball | 0.6 s | 0.7 s |
| **Sprint with ball** | **0.8 s** | **1.0 s** |
| Lock move | 0.2 s | 0.2 s |
| Back to sprint after a tackle | 0.35 s | |

**Turns** (brake + re-accelerate, and the speed you come out at):

| State | 90° turn | 180° turn | Exit speed at 90° / 135° / 180° |
|---|---|---|---|
| Run | 0.14 + 0.13 s | **0.3 + 0.3 s** | 85% / 85% / 60% |
| **Sprint** | 0.17 + 0.35 s | **0.4 + 0.35 s** | 70% / 70% / **50%** |
| Run with ball | 0.25 + 0.1 s | 0.35 + 0.32 s | **55%** for any turn |
| **Sprint with ball** | 0.32 + 0.25 s | **0.5 + 0.3 s** | 70% / 60% / **45%** |
| Sprint after a tackle | 0.17 + 0.3 s | 0.25 + 0.3 s | 80% / 80% / 60% |
| Lock move with ball | 0.18 + 0.12 s | 0.26 + 0.12 s | 47% / 47% / 32% |
| Lock move while sprinting | 0.1 + 0.1 s | 0.1 + 0.1 s | 85% / 60% / 50% |

How a turn plays (the code-driven transition data in Data/Locomotion/TransitionDB): you **brake along your old direction** on the brake curve, then **re-accelerate along the new direction** to the exit share of top speed; the direction can still change while braking. Which turn you get depends on the angle between your run and the stick: **under 30° you just steer** (with the ball at most 180°/s, without it 400°/s), **30–60° is a 45° turn** (with the ball only), **60–120° a 90° turn**, **120° and over a 180° turn** (the 135° band isn't used in free move).

What this means in practice:
- **A 180° turn in a sprint costs ~0.75 s** and you come out at half speed.
- **With the ball it costs 0.8 s** and you come out at 45%.
- Running with the ball, any turn of 45° or more drops you to 55% speed.
- Some transitions can be cut short by a new input: a start or stop after 30% of it, a 180° after 60% (40% in a sprint), a 90° after 80% (unconfirmed meaning).

### Lock move (strafing: facing one way, moving another)
- **Stance** (lock move with the ball) lasts at least **0.15 s** from the press: the release is only checked then.
- **Losing the ball ends lock move at once**; it needs a fresh press to come back.
- **No sprint and no extra effort in lock move** (its layers remove both).
- **Leaving lock move keeps your velocity:** if you were backing off, the body turns round to face the way you move while you keep moving back (the leave-lock-move transitions rotate the body, not the velocity). Spamming stance while backing off never drags you sideways.

| Who | Slow | Full | Turn rate |
|---|---|---|---|
| Outfield | 2 m/s | **5.25 m/s** | 400°/s |
| Outfield with ball | 1.75 m/s | **4 m/s** | 400°/s (inherited) |
| **Goalkeeper** | 2 m/s | **5 m/s** | **900°/s** |

Players find the outfield turn to face the front in lock move much quicker than 400°/s; the table has no value with the ball.

**Direction penalty:**

| Moving | Outfield | With ball | Goalkeeper |
|---|---|---|---|
| Forward | 100% | 100% | 90% |
| Sideways | 100% | 95% | **100%** |
| Backward | 90% | 88% | 90% |

### Speed during actions
Many actions move you on a scripted speed curve instead of your normal movement. In the curves below, time is a fraction of the action's duration.

| Action | Speed profile |
|---|---|
| Dribble (side) | 4 → **6.5 m/s** peak (10–15% in) → 3.4 m/s |
| Dribble forward | 4 → **6.8 m/s** peak (20% in) → 4 m/s |
| Dribble backwards | 3 → 5.8 m/s → 3 m/s |
| Feinted dribble | 4 → 6.8 m/s (held 20–30%) → 3–3.4 m/s |
| Exhausted dribble | same peak (6.5 m/s), but ends at only **1.5–2.5 m/s** |
| Push ball forward | **5.5 m/s** constant (some variants 7–7.8 m/s) |
| Push ball sideways | 3.5 → 5 m/s |
| Push ball backwards | 2.5 → 5.5 m/s |
| **Push ball in extra effort** | 6 → **9.5 m/s** in a third of the move, then holds 9.5 m/s |
| Push ball in Sweeper extra effort | up to **11.5 m/s** |
| Lobbed push ball, wind-up | 4 m/s |
| Standing tackle lunge | **9.85 m/s** burst in the first ~4%, then 4.5–5 m/s |
| Sliding tackle | 3 → **5.3 m/s** at ~9% → 1.7 m/s at half → stops |
| GK sliding tackle | 6.15 → 2 m/s |
| Ball control from lock move | you stop between 23% and 40% of the reception, then resume |
| Ball control during a dash | the dash stops almost immediately (8% in) |

A push ball during extra effort is the fastest way to move with the ball: 9.5 m/s, against 6.5 m/s for a sprint with the ball.

---

## 3. Cooldowns

| Action | Cooldown |
|---|---|
| **Sliding tackle** | **2.15 s** |
| Standing tackle | 1.0 s |
| Dive (outfield) | 1.35 s |
| Dash | 0.8 s |
| Dash right after a tackle | 0.96 s |
| Second dash in a row | 0.8 s |
| Dribble | none found (0.2 s unconfirmed) |
| "Ability recovery" (unconfirmed what this is) | 1.5 s |
| Ping | 1.0 s |

---

## 4. Shooting and passing

### How charging works (native code)
- **The charge clock** is how long the button has been held, counted from the press. A press that waited in the input buffer before the kick started counts too (the shot, pass and lob all count their time on the input stack).
- **Analog trigger:**
  - Shots fill at 50% speed up to a half press and at 100% fully pressed (Curve_ShootFillFactorByAnalogPressValue).
  - Pressing fully snaps the clock up to the real time held (catch-up).
  - Passes and lobs have no fill curve, so they always run at 100%.
- **The force curves are not read in seconds.** Shots, passes and lobs "clamp with the build-up duration", so the curves' 0→1 axis is the share of the build-up you have held:

  **charge = time held ÷ (build-up − 0.15 s), capped at 1.**
  - The build-up is the kick's own:
    - **Shot:** the time left in its build-up animation, 32 frames = **0.53 s** for the power kick. The table row Shoot_HighPower_VX (32) is only a fallback.
    - **Pass and lob:** **24 frames = 0.4 s** from the global shot table (rows Tap_VX and LobTap). Both kicks have "clamp with build-up" and "use the global table" on.

    The time of the frame in which the kick starts is taken off: the world's delta time, 1/60 s at 60 fps.
  - **The 0.15 s** is the stadium's network margins (time margin 0.12 + tolerance 0.03).
    - It is subtracted for everyone: the local player, the server and the other players' copies alike (no check on who is predicting).
    - The 5v5 maps' world settings don't change them (Coliseum and Wind main maps checked), and no code writes them at runtime.
    - Taking them off makes full power arrive before the animation ends.
  - **The clock** is the input's own hold time. It can only start earlier than the kick (a buffered press), never later. A prototype clock that starts at the press and counts while the button is down is the same thing.
- **So full power arrives early:**

  | Kick | Full power after holding | Earliest ball contact |
  |---|---|---|
  | Shot | **~0.37 s** | 0.53 s (power kick); a shot under 29.41 m/s is the low-power kick, struck at **0.33 s** |
  | Pass | **~0.23 s** | 0.4 s |
  | Lob | **~0.23 s** | 0.4 s |
- **The kick waits for your release; it doesn't fire at the end of the build-up.**
  - On press, the ability plays the build-up animation. On release (the sync), it plays the kick animation starting "from context": the position the build-up had reached (TargetData.AnimPos; BP_Final_Tap_Config, BP_Final_Shoot_Config). The ball leaves at that animation's kick notify.
  - The earliest sync is 0.08 s after the press for a pass and 0.2 s for a shot (0.33 s for the fake-shot input).
  - Holding past the build-up keeps you in the wind-up until you let go. Several rules run on the raw time held up to 1 s, which only makes sense if you can hold that long:
    - the pass lift from 0.6 s;
    - the lob flattening from 44° to 30° over 0.3–1 s;
    - the spin's charge scaling.
  - The only cap found is the sync task's timeout (its value wasn't read). That is probably the "fully charged kick goes off by itself" players describe.
- **Release:** the force is the one at your release (the shot's time is overridden at release time).
- **The crosshair's force fill** uses the same clock: fill = (current force − the curve's lowest) ÷ (highest − lowest), shown from 3% to 100%. It is full when the force is.

### Time held to ball speed
| Held | Shot | Pass | Lob |
|---|---|---|---|
| tap (shot 0–0.11 s; pass and lob 0–0.07 s) | 24 m/s | 20 m/s | 16 m/s |
| 0.11 s | 24 m/s | 26.9 m/s | 18.7 m/s |
| 0.14 s | 25.6 m/s | **32 m/s (115 km/h): the pass peak** | 20.7 m/s |
| 0.15 s | 26.2 m/s | 26.1 m/s: drops | 21.4 m/s |
| 0.21 s | **29.4 m/s**: from here on, the power kick | 29.4 m/s | 25.4 m/s |
| 0.23 s | 30.7 m/s | **30.65 m/s (full)** | **27 m/s (full)** |
| 0.3 s | 34.4 m/s | 30.65 m/s | 27 m/s |
| **0.37 s and longer** | **38 m/s (137 km/h, full)** | 30.65 m/s | 27 m/s |

Computed with a build-up of 32 frames for shots and 24 for passes and lobs, one frame off, and the 0.15 s network margins.

What this means in practice:
- **The pass has a sweet spot at ~0.14 s** (60% of its charge). That is the strongest pass in the game. Holding a moment longer drops it to 25.6 m/s, and a full charge only reaches 30.65 m/s.
- **Taps are weak:** the first 30% of each curve is flat. Shot 24 m/s, pass 20, lob 16.
- **Shots released before ~0.21 s are the quick low-power kick** (under 29.41 m/s, contact at 0.33 s). Later releases are power kicks struck at 0.53 s.

### Moving while you shoot
A shot keeps your run-up instead of stopping you:
- **Through the build-up** you keep moving in the direction you were going, at a share of your speed at the press that shrinks over the build-up: to **75%** normally, **60%** when sprinting at 4 m/s or more, **50%** for a shot straight out of a dribble.
- **After the touch** that speed is kept until the move ends.
- Passes keep 100% of your speed after the touch (below 2 m/s they use a fixed speed).
- **Running adds shot speed:** +1.25 m/s at 6 m/s, **+4 m/s at 9.5 m/s**.

### Launch angle, roll and spin by kick
- **Quick shots are a different kick.** A shot whose final speed (run-up bonus included) is under **29.41 m/s** uses the low-power kick, struck at **20 frames** (0.33 s) with a 3-frame freeze; stronger shots use the power kick at 32 frames with a 5-frame freeze. Every quick release is under 29.41 m/s, which is why taps go off before the 0.53 s build-up.
- **The run-up bonus** to shot speed (+1.25 m/s at 6 m/s, +4 m/s at 9.5 m/s) only counts for stronger shots: none up to 27.85 m/s, all of it from 38 m/s.
- **Passes leave flat.** Holding past the 0.6 s sweet spot lifts them by 3.9°, rising to 6° at 1 s. Low-power passes stay flat.
- **Lobs leave at 44°**, flattening to **30.26°** as you hold to 1 s (longer lobs are flatter and faster). The low-power lob leaves at **52°**.
- **Roll:** players' passes use a friction multiplier of **1.2** (0.3 belongs to the bots' pass); low-power passes 1.5× the base, weighted 1.675 (2.01 in all).
- **Forced spin:** shots (210) and passes (200) get a small **lift**; lobs (300) and low-power lobs (420) **dip**. The native code subtracts the forced axis from the spin, so the files' positive 210/200 lift and the lobs' negative values dip.
- **Curve input to spin:** side ±350 at 0.5, ±650 at 0.9, ±825 at full (dead zone 0.1); pulling back +330/+660/+780, pushing up −40/−80/−120. Scaled by charge: side ×1.02 → ×0.8, top ×0.95 → ×0.85 from a tap to full power.

### Fixed-force kicks
These ignore how long you hold:

| Kick | Ball speed |
|---|---|
| Low-power pass | **24.25 m/s** |
| Standing tackle hitting the ball | 19 m/s |
| Tackle hitting the ball | 17 m/s |
| Contested tackle | 15 m/s |
| Low-power lob | 13.2 m/s |
| Extra effort auto tap / control tap | 13 / 12 m/s |
| Push ball, lobbed push ball | **11.5 m/s** |
| Volley push ball | 11.25 m/s |
| Dribble push ball | 11 m/s |
| Goalkeeper push ball | 8.75 m/s |

### Execution timing
| Action | Build-up | Hitstop | Can act (standing) | Can act (moving) |
|---|---|---|---|---|
| Shot | **0.53 s** | 0.083 s | 0.67 s | 0.75 s |
| Pass | 0.4 s | 0.05 s | 0.58 s | 0.65 s |
| Lob | 0.4 s | 0.05 s | 0.58 s | 0.65 s (backwards: 0.7 s) |
| Short pass / short lob | **0.27 s** | 0.03 s | 0.65 s | 0.65 s |
| Push ball | **0.13 s** | | 0.38 s | 0.38 s |
| Push ball during extra effort | 0.13 s | | **0.2 s** | 0.2 s |
| Lobbed push ball (forward) | 0.37 s | | 0.58 s | 0.58 s |
| Lobbed push ball (sides/back) | 0.27 s | | 0.58 s | 0.58 s |

All times are from the press. Charged kicks wait for your release if you hold past the build-up.

### Analog trigger (controller)
| Trigger | Shot | Volley |
|---|---|---|
| up to half | bar fills at 50% speed | fills at 25% |
| fully pressed | 100% | 100% |

### Direction, height and curve
Full spec: [shot-aim.md](shot-aim.md).

- **Direction: the ball goes toward the crosshair's point, not along the camera.** The game traces up to 150 m from the camera through the crosshair (the trace ignores the floor and players, so it stops on the arena walls or the goal) and shoots from the ball toward that point, horizontally. With the camera to one side of the player, this is what makes the ball land under the crosshair.
- **The side curve input turns the shot 3° the other way** (±3° at full input), so a curving ball starts off to one side and bends back toward the crosshair.
- **Height comes from the camera's pitch, not from the trace:**
  - camera pitched well down: ~3° (driven along the floor);
  - level camera: ~17°;
  - camera up: ~20°.

  Pulling the curve input down (dip) adds up to **+4°** to make up for the dip; pushing it up adds nothing. The data also carries a pitch range whose top is 2°, which would flatten every shot; it cannot be in effect given the camera curve, so it is ignored here (open question).
- **At 30 m/s from a level camera** the ball passes 20 m at about 3 m high; with the camera slightly down (9°) it lands at 20 m.
- **Keyboard and mouse:**
  - **The shot direction locks the moment you start charging.**
  - From then on, **moving the mouse curves the ball instead of turning the camera.** Mouse movement feeds a spin accumulator with a sensitivity of 0.2, adjustable in the options ("Magnus sensitivity").
  - There are also options to invert the X and Y curve directions.
- **Controller:** the direction does not lock, so the right stick keeps aiming while the left stick sets the curve.
- **Side spin (stick left/right):**
  - dead zone up to 10%;
  - rises to 825 at full deflection;
  - symmetric.
- **Top/back spin (stick or mouse forward/back):**
  - **pulling down dips the ball**, up to 780;
  - pushing up lifts it, but only up to 120.

  The dip is about 6× stronger than the lift (confirmed in-game).
- **Spin scales with how long you held** (the external curves, which override the editor keys): side ×1.02 for a tap down to ×0.8 at full power, top/back ×0.95 down to ×0.85. A weak shot curves slightly more for its speed, not less.
- **Built-in spin:** every shot carries a small **lift** of 210 on top of what you add.
- **How strong spin is:** see §11. At 30 m/s a full side curve bends the ball about 3 m by 20 m.
- **Trajectory model:** the ball's flight is the sum of four pieces (§11): gravity, air drag, Magnus and ground friction. Each piece has a mask of when it applies (in flight, rolling or both); the "3" in the Magnus files is that mask (both), not a step count. Each kick swaps in its own version of some pieces, sometimes only for a range of bounces.

### Fake shot
- **Cost:** 1.5 stamina.
- **Can act again after:**

| Situation | Time |
|---|---|
| Standing | 0.33 s |
| Moving | 0.57 s |
| Exhausted, standing | 0.67 s |
| Exhausted, moving | 0.83 s |

---

## 5. Receiving and volleys

### Ball control
The game has two kinds of reception. Their timing:

| Body part | "Magnet" (normal): ball controlled | Magnet: can act | "Instant": controlled | Instant: can act |
|---|---|---|---|---|
| Feet / thigh | 0.2 s | 0.23 s | **0.083 s** | 0.12 s |
| Stomach | 0.2 s | 0.27 s | 0.083 s | 0.15 s |
| Chest | 0.2 s | 0.33 s | 0.083 s | 0.22 s |
| Head | 0.2 s | **0.37 s** | 0.083 s | 0.25 s |

"Instant" control is a match option (off in normal play), so the magnet timings are the normal ones. Instant controls also exist mid-dash.

**Control zones** (the character's control hitboxes, centred at pitch level, metres ahead of you; the ball's hit sphere is 0.12 m):

| When | Zone |
|---|---|
| Standing (up to **2.5 m/s**) | capsule r **0.50**, reaching 1.85 m, 0.10 ahead; plus a ground sphere r **0.83**, 0.10 ahead |
| Moving (above 2.5 m/s, or starting/turning) | capsule r **0.90**, reaching ~1.84 m, 0.30 ahead |
| Running forward | a ground capsule r 0.60 that also takes a ball rolling **away** from you |
| Lock move, no ball | capsule r **0.675**, from **any** direction |

- The normal zones **refuse a ball heading away from you**: the angle between you-to-ball and the ball's velocity must be over 75°.
- During a control you can only move, sprint, lock move, use extra effort and turn the camera; the ball becomes yours at **12 frames** and the rest opens at the control's action frame (14–22 frames by animation).
- **You can't control your own kick for 0.5 s** after any kick (the "lost ball" effect), and most kicks stop anyone controlling them for 0.5 s (push balls 0.3 s for others).

### First-time volley pass (volley tap)
| Contact | Ball leaves | Can act |
|---|---|---|
| Foot | 0.25 s | 0.42 s |
| Thigh | 0.28 s | 0.43 s |
| Pelvis | 0.3 s | 0.52 s |
| Chest | 0.3 s | 0.55 s |
| Head | 0.3 s | 0.57 s |
| Jumping chest/head | 0.4 s | 0.83–0.92 s |
| Long-range jumping header | 0.43 s | **1.17 s** |
| Goalkeeper header | 0.3 s | 0.72 s |

### Ball height thresholds for volleys (unconfirmed)
- **Volley shot:** below **60 cm** (standing) or **120 cm** (moving) the game uses the foot animation, and above that thigh/body.
- **Other volleys:**

| Volley | Threshold |
|---|---|
| Volley pass | 70 cm |
| Goalkeeper | 75 cm |
| Lobbed push ball | 80 cm |

---

## 6. Dribbling
Full rules (fast cut, side, drag back, the feint, push balls during a dribble): [dribble.md](dribble.md).

- **Cost:** 0.5 stamina.
- **Cooldown:** none on the ability in the data (the 0.2 s figure is unconfirmed).
- **Stamina regen** waits 0.9 s after a dribble.
- **Timing:**
  - the ball is touched at 0.13 s;
  - you can pass, shoot, sprint or push the ball from **0.2 s**;
  - the dribble ends at 0.75 s.
- **Protection window** (what it protects against is unconfirmed: tackle immunity?):
  - starts 0.2 s in;
  - lasts **0.25 s** on a normal dribble and **1 s** on a feinted dribble.
- **Volley dribble:** lasts ~0.5 s, and its protection lasts 3 s.

---

## 7. Tackles

| | Standing tackle | Sliding tackle | GK sliding tackle |
|---|---|---|---|
| Startup (until the hitbox is live) | **0.117 s** | **0.117 s** | — |
| Hitbox active | 0.25 s | **0.78 s** | 0.92 s |
| Recovery | 0.3 s | 0.25 s | 0.25 s |
| **Total** | **0.67 s** | **1.15 s** | |
| Hitstop on hit | 0.12 s | 0.15 s | 0.12 s |
| Cooldown | 1.0 s | **2.15 s** | |
| Stamina | 1.5 (unconfirmed) | 3.0 | |

A standing tackle that **connects** lets you act at **0.5 s**. One that misses lets you act at 0.67 s, and in both cases ball control comes back later still.

### Getting tackled (stun)
| Hit by | Stunned for | Pushed back |
|---|---|---|
| **Standing tackle** | **1.22 s** | 2 m |
| **Sliding tackle** | **1.5 s** | **3.5 m** |
| Sliding tackle during lock move | 1.02 s | 3.5 m |
| Dribble (unconfirmed: a dribble that bumps an opponent?) | 1.22 s | 3.5 m |
| Protect (ball shielding) | 0.5 s | 1.7 m |
| Interrupt | 0.5 s | |

Separately, losing the ball puts a 0.5 s "lost ball" flag on you (unconfirmed effect).

### Lock move catch hitbox
It stays live for **1.5 s**, the same in the centre and on the sides. Unconfirmed whether this is goalkeeper-only.

---

## 8. Dash
- **Startup:** 0.067 s.
- **Duration:** ~0.8 s.
- **Distance:** **2.25 m**.
- **You can deflect the ball during a dash:** it carries a "deflect" hitbox.
- **Speed:** peaks at 8.5 m/s (goalkeeper 9.5 m/s), then ends at 4.5 m/s.
- **Second dash in a row:** it can cut into the first from 0.24 s while the first one's cooldown runs. It peaks at only 7.2 m/s and ends at 2 m/s.
- **Not with the ball:** with the ball the same press is a dribble ([dribble.md](dribble.md)).
- **Dashing onto a loose ball controls it** (no deflect) and carries it on in front, the way you dash, like a fast cut (players confirm).
- **Cost:** 0.75 stamina. **Cooldown:** 0.8 s.
- What you can do during a dash, and when, is in [frame-data.md](frame-data.md).

---

## 9. Dives

### Outfield
| Type | Wind-up | Airborne | Recovery | Total |
|---|---|---|---|---|
| Ground | 0.1 s | 0.37 s | **0.67 s** | 1.13 s |
| Air | 0.1 s | 0.58 s | 0.57 s | 1.25 s |
| Up | 0.1 s | 0.5 s | 0.47 s | 1.07 s |
| Air, from a run | 0.1 s | 0.72 s | 0.45 s | 1.27 s |

- **Cost:** 1.25 stamina. **Cooldown:** 1.35 s.

### Goalkeeper
- **Dive:** 0.5 to 1 s airborne depending on angle and range. **Free in stamina.**
- **Parrying recovers faster than catching.** Long-range sideways dive, recovery:
  - parry: **0.33 s**;
  - catch: **0.67–0.8 s**.

### Goalkeeper with the ball in hand
| | Time |
|---|---|
| Caught from an opponent: protected | **4.5 s** |
| Received from a teammate (back pass): protected | **2.65 s** |
| Power throw/kick from hand: wind-up | 0.53 s (+0.1 s hitstop) |
| Pass/lob from hand: wind-up | 0.4 s |
| Can act after a power kick | 0.72–0.82 s |
| Can act after a pass | 0.65 s |

---

## 10. Extra effort
Ability spec (cost, auto tap, control tap): [extra-effort.md](extra-effort.md).

- **Own gauge:** separate from stamina, from 0 to 1.
- **Duration:** while active it drains 0.5 per second, so it lasts **~2 s at most**.
- **Start phase:** the first **0.6 s** is a separate phase. If you have the ball, it gets kicked forward at 0.13 s and you can act at 0.2 s.
- **What it gives you:** instant acceleration (100 m/s² instead of 18). Top speed is the same as a sprint (8.75 m/s), you just reach it immediately.
- **During extra effort:**
  - balls you touch roll less (friction 1.5×);
  - push balls let you act much sooner (0.2 s instead of 0.38 s).
- **Recharge:**
  - normal: 0.0625 per second, so **16 s** to fill;
  - **with stamina full: 3× faster, 5.3 s** (the table's multiplier threshold is 1.0; players confirm it applies at full stamina).
- **Each activation costs a quarter of the gauge** and needs that much to start, on top of the 0.5/s hold drain, so a full gauge gives **three direction cuts** (players confirm).
- **Touches on a sharp turn are weaker:** ×0.8 at 135°, ×0.5 at 180°.

---

## 11. The ball

| Property | Value |
|---|---|
| Top speed | **70 m/s (252 km/h)** |
| Radius | ~11–12 cm |
| Mass (gameplay) | 1.625 |
| Base ground friction | 0.8 |
| Air drag coefficient | 0.45 |
| Magnus effect | yes: spin curves the ball in flight and while rolling |

### Forces (native code)
Full spec: [ball-flight.md](ball-flight.md).

Each piece gives an **acceleration** directly; nothing is divided by the mass. In metres:
- **Gravity (in flight):** g × (1.625 + 0.5 × seconds spent falling since the last bounce), with g the world's 9.8 m/s². So a ball rises against **15.9 m/s²** and comes down faster the longer it falls. The 1.625 is the ball's Mass attribute; the 0.5 is the "free fall multiplier".
- **Air drag (in flight):** coefficient × 0.0228 × speed², weaker the more vertical the flight (× (1 − the vertical share of the velocity)). 0.0228 is half the air density times the ball's cross-section. With the default 0.45, a 30 m/s shot slows to about 24 m/s over 20 m.
- **Magnus (flying and rolling):** 0.0007986 × speed × spin.
  - The spin is kept in the ball's flight frame, so it turns with the path: the side part curves the ball, the other part dips or lifts it.
  - Rolling, the dip/lift part pushes along the path instead: a dipping ball brakes, a lifting one runs on. The side part keeps curving a rolling ball.
  - At 30 m/s a spin of 660 (full side curve at full power) gives about 16 m/s² sideways.
- **Spin fades** by 100% per second (exponentially), flying or rolling.
- **Ground friction (rolling):** deceleration = friction × speed, so the ball slows exponentially and rolls **speed ÷ friction** metres. The friction is the base 0.8 × the touch's multiplier × the override's weight. Examples:
  - push ball: 11.5 m/s with 2.24, so about 5.1 m;
  - 25 m/s pass with 0.96: about 26 m;
  - 30 m/s rolling shot with 0.8: about 37 m.

### Bounce by surface
"Perpendicular" is how much speed is kept on a head-on hit; "parallel" is how much is kept on a glancing hit.

| Surface | Perpendicular | Parallel | In practice |
|---|---|---|---|
| Grass | 0.475 | 0.95 | loses half its height, skids well |
| Glass walls | 0.675 | 0.675 | keeps 2/3 |
| Repulsive glass | 0.425 | 0.625 | |
| Absorbent glass | 0.3 | 0.6 | almost dead |
| **Posts (metal)** | **0.875** | 0.875 | the posts are the bounciest surface |
| Net | 0.08 | 0.15 | dead |
| 1v1 net | 0.05 | 0.05 | |

### How far it rolls after a touch (friction multiplier)
The ball rolls speed ÷ (0.8 × multiplier × weight) metres.

| Touch | Multiplier | Effect |
|---|---|---|
| Bots' pass | **0.3×** | rolls very far |
| Players' pass (analog power) | 1.2× | normal |
| Low-power pass | 1.5×, weight 1.675 | stops sooner |
| During extra effort | 1.5× | stops sooner |
| Volley push ball | 1.6× | stops sooner |
| Fast tap (push balls) | 2.8× | dies quickly |
| Goalkeeper push | **6×** | stops almost at once |

### Air drag by touch
| Touch | Air drag coefficient |
|---|---|
| Default (the ball's attribute) | 0.45 |
| Players' pass, from its first bounce | 0.75 × weight 1.25 |
| Goalkeeper low-power pass from the hand | 1.4 |

The 2.54 in the default drag file is only a fallback: that file reads the ball's attribute (0.45) instead.

### Roll or bounce (unconfirmed)
- **Slow balls** start rolling at almost any landing angle.
- **Above 40 m/s** they only roll if they land almost flat; otherwise they bounce.

---

## 12. Contested balls
When two players touch the ball almost at the same time:

| Gap between touches | Result |
|---|---|
| up to 0.04 s | **contest**: up to 3 touches count |
| 0.04–0.15 s | **deflection** |
| over 0.15 s | normal touches, one after another |

Which touch wins over the network is in [match-camera-network.md](match-camera-network.md).

---

## Confirmed by players, and open questions
**Confirmed:**
- 4.5 s / 2.65 s = how long the goalkeeper is protected holding the ball.
- Pulling the curve input down dips the ball, and that is the strong direction.
- You can shoot from lock move (defence stance).
- A fully charged kick goes off by itself.

**Open questions:**
1. With a full stamina bar, do you get ~13 s of sprint without the ball and ~9 s with it?
2. Does a standing tackle cost half the stamina of a sliding tackle?
3. When does an "instant" reception (0.083 s instead of 0.2 s) happen?
4. Is the dribble protection window (0.25 s, or 1 s when feinted) tackle immunity?
5. Does raising the camera make shots fly higher? The shot data carries a 2° cap on the launch angle that would make every shot almost flat; the notes assume it is not in effect.
