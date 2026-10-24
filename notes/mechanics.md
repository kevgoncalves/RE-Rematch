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

| Situation | Top speed | km/h | Acceleration |
|---|---|---|---|
| Walk | 2.5 m/s | 9 | |
| Run (stick fully pushed) | **5 m/s** | 18 | 18 m/s² (0 to 5 m/s in ~0.3 s) |
| **Sprint** | **8.75 m/s** | **31.5** | |
| Run with ball | 4.5 m/s | 16 | |
| **Sprint with ball** | **6.5 m/s** | **23.4** | |
| Extra effort | 8.75 m/s | 31.5 | **100 m/s² (instant)** |
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

What this means in practice:
- **A 180° turn in a sprint costs ~0.75 s** and you come out at half speed.
- **With the ball it costs 0.8 s** and you come out at 45%.
- Running with the ball, any turn of 45° or more drops you to 55% speed.
- Some transitions can be cut short by a new input: a start or stop after 30% of it, a 180° after 60% (40% in a sprint), a 90° after 80% (unconfirmed meaning).

### Lock move (strafing: facing one way, moving another)
| Who | Slow | Full | Turn rate |
|---|---|---|---|
| Outfield | 1 m/s | 4 m/s | 400°/s |
| Outfield with ball | 1.75 m/s | 3.5 m/s | |
| **Goalkeeper** | 2 m/s | **5 m/s** | **900°/s** |

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
| Dribble | 0.2 s |
| "Ability recovery" (unconfirmed what this is) | 1.5 s |
| Ping | 1.0 s |

---

## 4. Shooting and passing

### How charging works
The force curves are by **time held**, in seconds since you pressed the button.

The **build-up (wind-up) runs while you hold**, not after you release:
- the ball leaves when you release;
- but never before the build-up has finished;
- the force uses at least the build-up time.

| Kick | Build-up (earliest ball contact) |
|---|---|
| Shot | **0.53 s** |
| Pass | 0.4 s |
| Lob | 0.4 s |

So a full-power shot takes 1 s from press to ball contact, not 1 s plus the wind-up.

### Time held to ball speed
| Held | Shot | Pass | Lob |
|---|---|---|---|
| 0–0.3 s | 24 m/s | 20 m/s | 16 m/s |
| 0.4 s (fastest pass/lob) | 26 m/s | 24 m/s | 17.6 m/s |
| 0.53 s (fastest shot) | **28.7 m/s (103 km/h)** | 29.3 m/s | 19.7 m/s |
| **0.6 s** | 30 m/s | **32 m/s (115 km/h): the pass peak** | 20.7 m/s |
| 0.61 s | 30.2 m/s | **25.7 m/s: drops** | 20.9 m/s |
| 0.8 s | 34 m/s | 28.1 m/s | 23.9 m/s |
| 1.0 s and longer | **38 m/s (137 km/h)** | 30.65 m/s | **27 m/s** |

What this means in practice:
- **The pass has a sweet spot at 0.6 s.** That is the strongest pass in the game. Holding a moment longer drops it to 25.6 m/s, and even a full 1 s hold only reaches 30.65 m/s.
- **The quickest possible shot is already 28.7 m/s.** The flat 0–0.3 s part of the curve is never reached, because the 0.53 s build-up comes first.
- The power-shot animation plays above 34.8 m/s, which is a hold of ~0.84 s (unconfirmed).

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
- **Height comes from the camera:**
  - camera pitched well down: ~3° (driven along the floor);
  - level camera: ~17°;
  - camera up: ~20°.
- **Direction:** the stick only nudges the aim by **±3°**. The camera/crosshair does the rest.
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
- **Spin scales with how long you held**, as a share of the stick's spin:

| Held | Side spin | Top/back spin |
|---|---|---|
| 0.1 s | 50% | 35% |
| 1.0 s | 90% | 50% |

  A weak shot curves much less than a full-power one.
- **Built-in spin:** every shot carries a fixed spin of 210 on one axis on top of what you add (the file calls it "forced Magnus"; unconfirmed whether it is dip or rise).
- **Trajectory model:** the ball's flight is the sum of four pieces, recomputed every 3 physics steps for the Magnus part:
  1. gravity;
  2. air drag;
  3. Magnus (spin);
  4. ground friction.

  Each shot type swaps in its own version of these pieces.

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

When each kind triggers is unconfirmed. Instant controls also exist mid-dash.

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
- **Cost:** 0.5 stamina.
- **Cooldown:** 0.2 s.
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
- **Second dash in a row:** peaks at only 7.2 m/s and ends at 2 m/s.
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
- **Own gauge:** separate from stamina, from 0 to 1.
- **Duration:** while active it drains 0.5 per second, so it lasts **~2 s at most**.
- **Start phase:** the first **0.6 s** is a separate phase. If you have the ball, it gets kicked forward at 0.13 s and you can act at 0.2 s.
- **What it gives you:** instant acceleration (100 m/s² instead of 18). Top speed is the same as a sprint (8.75 m/s), you just reach it immediately.
- **During extra effort:**
  - balls you touch roll less (friction 1.5×);
  - push balls let you act much sooner (0.2 s instead of 0.38 s).
- **Recharge:**
  - normal: 0.0625 per second, so **16 s** to fill;
  - with a 3× bonus: **5.3 s** (unconfirmed when the bonus applies).

---

## 11. The ball

| Property | Value |
|---|---|
| Top speed | **70 m/s (252 km/h)** |
| Radius | ~11–12 cm |
| Mass (gameplay) | 1.625 |
| Base ground friction | 0.8 |
| Magnus effect | yes: spin curves the ball in flight |
| Gravity multiplier | 0.5× on the ball's own gravity value |

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
| Touch | Friction | Effect |
|---|---|---|
| Normal pass | **0.3×** | rolls very far |
| Analog-power pass | 1.2× | normal |
| Low-power pass | 1.5× | stops sooner |
| During extra effort | 1.5× | stops sooner |
| Volley push ball | 1.6× | stops sooner |
| Fast tap | 2.8× | dies quickly |
| Goalkeeper push | **6×** | stops almost at once |

### Air drag by touch
| Touch | Air drag |
|---|---|
| Default | 2.54 |
| Low-power pass | 1.4 |
| Analog-power pass | 0.75 (much less slowdown in the air) |

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
