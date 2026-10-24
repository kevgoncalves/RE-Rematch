# Rematch frame data: when you can act again

Every action animation carries time markers:
- when the hitbox turns on and off;
- when the ball is touched;
- when an **availability layer** starts, meaning a set of actions you are allowed to do from that moment.

This is the game's cancel system.

60 frames = 1 second. The full per-animation table (766 animations) is in [frame-data.csv](frame-data.csv). This page summarises it.

## The availability layers
"Can act" in the tables below means one of these layers has started.

| Layer | What it allows |
|---|---|
| **Everything** | every action, except dribble push balls and shoot-after-dribble (those only exist inside a dribble) |
| **Everything except ball control** | all actions, but you cannot control the ball yet |
| **Everything except passes** | no pass, short pass, lob, short lob or lobbed push ball |
| **After dash** | everything except shooting, passing, volleys and push balls |
| **Only dive and tackle** | move, lock move, dive, standing tackle, ball control |
| **Only extra effort** | move, sprint, extra effort |
| **Dribble** | pass, lob, short lob, sprint, extra effort, dribble push balls, feint, shoot after dribble, lock move. No new dribble until it ends. |
| **Exhausted dribble** | same as dribble |
| **Feinted dribble** | same as dribble, minus short lob |
| **Lock move** (while strafing) | everything except shooting, passing, push balls, sprinting and volleys |
| **Dribble stance** | everything except sprinting and extra effort |

## Chord window (not an input buffer)
**Correction:** these times are not a buffer for presses made too early. When the ability starts, it keeps a few longer chords allowed for that long, so a slightly staggered chord still gives the kick you meant (e.g. push ball, then the low-power pass within 0.1 s). How early presses really count (taps on the release, held kicks as soon as allowed) is in [responsiveness.md](responsiveness.md#1-when-a-press-counts).

| Action | Buffer |
|---|---|
| Pass (tap) | **0.1 s** (6 frames) |
| Push ball | 0.1 s |
| Volley pass | 0.1 s |
| Volley push ball | 0.1 s |
| Standing tackle | 0.1 s |
| Sliding tackle | 0.1 s |
| GK sliding tackle | 0.1 s |
| Low-power lob (outfield and GK) | **0** (no buffer) |

Other actions use the engine default, which is not stored in these files.

### Other input timings
- **Button combinations:** the second button must come within **0.2 s** of the first.
- **Flick-direction taps:** the stick must pass **90%** deflection to count as a flick.

---

## Frame data by action

### Shots and passes
| Action | Can act, standing | Can act, moving | Layer |
|---|---|---|---|
| Power shot | 40f (0.67 s) | 45f (0.75 s) | Everything |
| Low-power shot | 35f (0.58 s) | 45f (0.75 s) | Everything |
| Pass | 35f (0.58 s) | 39f (0.65 s) | Everything |
| Lob | 35f (0.58 s) | 39f (0.65 s); backwards 42f (0.7 s) | Everything |
| Push ball | 23f (0.38 s) | 23f (0.38 s) | Push-ball layer |
| Lobbed push ball | 35f (0.58 s) | 35f (0.58 s) | Push-ball layer |
| Dribble push ball | 23f (0.38 s) | | Push-ball layer |
| Dribble lobbed push ball | 35f (0.58 s) | | Push-ball layer |

### Fake shot
| | Can act |
|---|---|
| Standing | 20f (0.33 s) |
| Moving | 20–34f (0.33–0.57 s) |
| Exhausted, standing | 40f (0.67 s) |
| Exhausted, moving | 50f (0.83 s) |

### Dribble
| Event | Frame |
|---|---|
| Ball touched | 8f (0.13 s) |
| Dribble layer starts | **12f (0.2 s)** |
| Dribble ends | 45f (0.75 s) |
| Animation length | 50f (exhausted: 63f) |

### Dash
| Event | First dash | Second dash |
|---|---|---|
| **Only dive and tackle** | **11f (0.18 s)** | 11f (0.18 s) |
| **After dash** layer (no shooting/passing) | **14f (0.23 s)** | **25f (0.42 s)** |
| Animation ends (everything back) | 41–50f (0.68–0.83 s) | 42–60f (0.7–1.0 s) |

What this means:
- From 0.18 s into a dash you can cancel into a dive or a standing tackle.
- You cannot shoot or pass until the dash animation finishes.

### Extra effort start
| Event | Frame |
|---|---|
| Ball kicked forward (if you have it) | 8f (0.13 s) |
| Push-ball layer starts | 12f (0.2 s) |

### Ball control
| Kind | Ball controlled | Can act |
|---|---|---|
| "Magnet" (normal) | 12f (0.2 s) | feet/thigh 14f, stomach 16f, chest 20f, head 22f (0.23–0.37 s) |
| "Instant" | 5f (0.083 s) | feet/thigh 7f, stomach 8–9f, chest 13f, head 15f (0.12–0.25 s) |

Goalkeepers use the same numbers.

### Volleys
| Action | Ball contact | Can act | Cancel window | Airborne |
|---|---|---|---|---|
| Volley pass, foot | start | 25f (0.42 s) | 0–15f | |
| Volley pass, thigh | start | 26f (0.43 s) | 0–17f | |
| Volley pass, pelvis | start | 31f (0.52 s) | 0–18f | |
| Volley pass, chest | start | 33f (0.55 s) | 0–18f | |
| Volley pass, head | start | 34f (0.57 s) | 0–18f | |
| Volley pass, jumping header | 8f | 55f (0.92 s) | 0–24f | 8–42f |
| Volley shot, standing | ~2f | 48–57f (0.8–0.95 s) | 0–18/22f | |
| Volley shot, moving | ~2f | 52–62f (0.87–1.03 s) | 0–22f | |
| Volley shot, jumping | ~2–4f | 60–65f (1.0–1.08 s) | 0–30f | ~8–50f |
| Volley push ball | start | 26–30f (0.43–0.5 s) | 0–14/18f | |
| Volley lobbed push ball | start | 35–55f (0.58–0.92 s) | 0–18f | |
| Aerial volley push ball | start | 39f (0.65 s) | | 6–39f |

Notes:
- **Volley push balls only unlock "only extra effort" at that point.** Your next action has to be a sprint or extra effort.
- **Cancel window (unconfirmed):** a section at the start of every volley where it can still be cancelled.

### Tackles
| Action | Hitbox | Recovery hitbox | Can act (no ball control) |
|---|---|---|---|
| Standing tackle, miss | 7–22f | 22–40f | 40f (0.67 s) |
| Standing tackle, **hit** | | | **30f (0.5 s)** |
| Dash deflect (chest/head/pelvis) | 7–22f | 22–40f | 30–40f |
| Sliding tackle | **7–47f** | 47–62f | **62f (1.03 s)** |
| GK sliding tackle, catch | 7–55f | | 70f (1.17 s) |
| GK sliding tackle, miss | 0–55f | | 70f (1.17 s) |

### Getting hit
| Hit by | Can act | Layer |
|---|---|---|
| Standing tackle / block | 73f (1.22 s) | Everything |
| Sliding tackle | 61–90f (1.02–1.5 s) | Everything |

### Dives
| Action | Hitbox in flight | Recovery hitbox | Can act | Layer |
|---|---|---|---|---|
| Outfield dive, free move | 1–43f | 40–80f | 50–80f (0.83–1.33 s) | Everything except passes |
| Outfield dive, from lock move | 1–35f | 22–45f | 30–66f (0.5–1.1 s) | Everything except passes |
| Goalkeeper dives | 0–45f | 31–116f | 42–92f (0.7–1.53 s) | Everything except passes |

- **Dive assist** (unconfirmed: aim assist toward the ball) covers the first 0.4 s (1–25f) of every outfield dive.
- **Apex slow-motion:** most dives slow down at their apex for ~14 frames, from frame 8.

### Goalkeeper with the ball
| Action | Release | Can act |
|---|---|---|
| Power kick/throw | 32f (0.53 s) | 41–50f (0.68–0.83 s) |
| Pass / lob | 24f (0.4 s) | 39f (0.65 s) |
| Push ball | 8f | 23f |
| Lobbed push ball | 16f | 25–35f |

### Other
| Action | Can act |
|---|---|
| Protect (shield) | 35f (0.58 s) |
| Push back | 33f (0.55 s) |
| Kickoff (centre player) | touches the ball at 47f (0.78 s) |
