# Dribbles and push balls

How the dash with the ball (the "dribble"), its second move and the push balls work, from the game's ability data, montages and tables. 60 fps: 1 frame = 1/60 s. Speeds are in m/s. Values marked *unconfirmed* are our reading where the files are silent.

## The dribble

### What triggers it
- **You need the ball and lock move (defence stance).**
  - Keyboard: hold **Left Ctrl**, press **Space** (the push ball key) with a direction.
  - Gamepad: hold **LT**, press **A** with the stick.
- Without the ball the same gamepad press is the normal dash. **The dash itself can't be used with the ball.**
- No direction counts as forward.

### Dribble 1: which move you get
The direction is your input against the camera (not against the way the player faces). Players confirm that flicking the camera and then pressing W + dribble fast-cuts the way you flicked, and the player and ball turn that way.

| Input | Move | Lasts | Ball touched |
|---|---|---|---|
| **Forward, within 45°** (W, W+A, W+D) | **fast cut** | 54f (0.9 s) | 17f |
| **Sideways** (A, D, and S+A / S+D) | side dribble: you keep facing forward and slide that way | 50f (0.83 s) | 8f |
| **Back** (S) | **drag back** | 64f (1.07 s) | 0f |

- Keyboard diagonals sit exactly on the sector edges. Players confirm that W+A / W+D give the fast cut and S+A / S+D the side dribble going back.
- There are only forward, side and back animations; diagonals reuse them, and the move follows your actual input direction.

### Dribble 2 (the feint)
- **From 12f (0.2 s) into dribble 1 until dribble 1 ends**, pressing again chains dribble 2. It replaces dribble 1.
- **It is picked from the new direction, the same three sectors**, not from what dribble 1 was. So forward after a fast cut gives the forward feint (the **roulette**), and back after a drag back gives another drag back. A different direction gives that direction's feint.
- **At most two in a row.** Dribble 2 can't chain into anything, and a fresh dribble 1 has to wait until **1.2 s** after dribble 2 started (*unconfirmed*: our reading of a 1 s tag time that starts at 12f).
- Lengths: forward 65f (1.08 s), side 50f, back 53f.

### Speed
Each move follows its own speed curve over its first **45f (0.75 s)**, then holds the last value until it ends.

| Move | Speed over the first 0.75 s (share of it → m/s) |
|---|---|
| Fast cut | 0 → 4.0, 20% → **6.8**, 50% → 4.0 |
| Side | 0 → 4.0, 10–15% → **6.5**, 35% → 3.4 |
| Drag back | 0 → 3.0, 10–20% → **5.8**, 45% → 3.0 |
| Feint forward (roulette) | 0 → 4.0, 10–30% → **6.0**, 60% → 4.0 |
| Feint side | 0 → 4.0, 20–30% → **6.8**, 50% → 3.0, end 3.4 |
| Feint back | 0 → 3.0, 10–20% → **5.8**, 50% → 2.5, end 3.0 |

The roulette covers about the same ground as the fast cut (average 4.8 against 4.7 m/s) but takes longer.

### What you can do during a dribble
- **First 12f (0.2 s): nothing.**
- **From 12f to the end:**
  - allowed: the feint, passes, lobs, short lobs, a special shot ("shoot after dribble"), sprint, extra effort, lock move, and the **dribble push balls**;
  - players say shots, passes and lobs after a dribble are almost instant and work at any moment of it, so they may be buffered from the start (unconfirmed in the data);
  - not allowed: a new dribble 1, the dash, tackles, dives, the normal push ball and the normal shot.

### Cost and exhaustion
- **Cost:** 0.5 stamina per dribble, so a full chain costs 1.0 (*unconfirmed* amount: the cost table isn't exported). Stamina regen then waits 0.9 s. Goalkeepers and Sweepers don't pay.
- **No ability cooldown** on dribble 1 in the data.
- **Out of stamina:** you get a tired dribble instead. It **can't chain a feint**. It lasts 65f forward, 63f side and 67f back, with the same 6.5 m/s peak, but it ends slow: 2.5 m/s forward and back, **1.5 m/s** sideways.

## Push balls

### Normal push ball
- **Ball:** 11.5 m/s along the ground, no charge. It goes the way you steer, or straight on with no direction.
- **Direction against your facing changes the speed:**

  | Direction | Ball speed |
  |---|---|
  | Forward, up to 60° | ×1.0 (11.5 m/s) |
  | Sideways, to 95° | ×0.89 (10.2 m/s) |
  | Back | ×0.79 (9.1 m/s) |

- **Sprinting** switches in a stronger force. The factor isn't in the exported files. Walking and jogging with the ball give **the same** push ball; only sprinting changes it.
- **Sprinting means holding the sprint button**, even standing still: players confirm that a push ball with sprint held is a sprint push ball, and that standing still with sprint held costs no stamina.
- **Tired:** 11.25 m/s, pops up 13°, sprint ignored.
- **Timing:** ball touched at **8f**; free to act at **23f**. After that you can only steer out of the move within 30° of it until it ends (about 55f).
- **You chase the ball** on a set speed after the touch:

  | Direction | Normal | Sprinting | Tired |
  |---|---|---|---|
  | Forward | 5.0 → 5.5 | **7.0** | 5.5 |
  | Side | 3.5 → 4.5 → 5.0 | 3.5 → 6.5 → **7.8** | 2.8 → 3.8 → 5.0 |
  | Back | 2.5, then → 5.5 | 3.5, then → **7.5** | |

  So a sprint push ball keeps you close to the ball; otherwise you trail it.
- A forward push ball at **6 m/s or more** (in practice, sprinting) uses a different animation.
- **You can't push the ball from lock move.** There the same button dribbles.

### Push ball during a dribble
- **When:** from 12f (0.2 s) into any dribble (dribble 1, the feint or the tired dribble) until it ends.
- **How:** the push ball button while **not** in lock move. In lock move the same button is the feint, which needs lock move, while the push ball is blocked by it (*unconfirmed* that this is how players switch between them).
- **Animations by direction:** five animations over six sectors: forward (±20°), the two forward diagonals (to ±60°), the sides (to ±135°) and back.
- **Ball:** **11 m/s**, **×1.35 when sprinting = 14.85 m/s**. The direction doesn't change the speed. Tired: 11.25 m/s at 13°.
- **Timing:** touched at 8f, free at 23f, like the normal one. It lasts 47–74f depending on direction.
- **You chase at a constant 6 m/s** (sprinting 7.8, tired 5).
- **Cost:** 0.6 stamina (*unconfirmed*).

### Lobbed push balls
| Variant | Speed | Angle |
|---|---|---|
| Normal | 11.5 | 65° |
| During a dribble | 12.5 | 65°; sprinting ×1.1 and −5° |
| Tired | 13.0 | 58° |
| Tired, during a dribble | 12.75 | 62° |

- The normal one loses force going sideways (×0.9, +10° higher) and back (×0.975, +3.5°).
- Ball touched at 22f going forward, 16f sideways or back; free to shoot or control at 35f.

### Rough distances
The push ball's ground friction isn't exported, so these are estimates from our model (friction 2.24):

| Push | Ball speed | Distance |
|---|---|---|
| Push ball forward | 11.5 | ~3–5 m |
| Push ball back | 9.1 | ~2–4 m |
| Dribble push ball | 11 | ~3–5 m |
| Dribble push ball, sprinting | 14.85 | ~5–6.6 m |

## Extra effort and the ball
Players confirm that **extra effort can't control the ball**:
- Starting it with the ball touches the ball on ahead (13 m/s, at 8f).
- Running into the ball taps it on again (12 m/s). Against your 8.75 m/s that keeps the ball within about a metre.
- **Cuts:** you hold extra effort and change direction with the ball at your feet; each change taps the ball the new way. Touches on a sharp turn are weaker: ×0.8 at 135°, ×0.5 at 180°.
- **A full gauge gives three cuts** in one hold: activating costs a quarter of the gauge and holding drains 0.5/s, about 1.5 s in all.
- **Changing direction in extra effort is snappy**: its near-instant acceleration (100 m/s²) turns your run; the 150°/s turn rate seems to apply to the body only (our reading of the players' description).

## The dash (without the ball)
- Only without the ball, from lock move.
- **A second dash can cut into the first** from **0.24 s** into it, while the first dash's cooldown (0.8 s) is running. It is weaker: 6 → 7.2 → 2 m/s against 6 → 8.5 → 4.5.
- After the second dash, both wait for its own cooldown. So at most two in a row.
