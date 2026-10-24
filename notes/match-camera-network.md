# Rematch match rules, camera and network

## Match rules
| Mode | Match length | Other |
|---|---|---|
| **5v5** | **6:00** | 3 s countdown; players can join an ongoing match until 1:30 is left; AFK players get kicked |
| 1v1 | 3:00 | |
| Golden Striker | 4:00 | |
| Fireball | 5:00 or **first to 15** | no backfill if the goal difference is above 8 |

### When the clock hits zero ("clutch time", unconfirmed)
The game state has three end-of-match timeouts:

| State | Timeout | Likely meaning |
|---|---|---|
| Last action | **60 s** | play continues while the ball is live |
| Last shot | **10 s** | |
| Waiting for possession | **7 s** | |

### Other timings
- A shot taken in the **last 1 s** of the countdown is tracked specially ("late shot").
- Delay after a goal before the celebration: 1 s. A match-part transition lasts at most 3 s.

## Camera
The game ships two sets of camera settings: "default" and "Market". "Market" is probably the alternative camera option in the settings (unconfirmed).

### Default set
| Camera | FOV | Turn speed (horizontal / vertical) |
|---|---|---|
| Normal play | **80°** | |
| Sprinting without the ball | **85°** | |
| Aiming | 80° | **160°/s / 80°/s**, +40/+20 boost |
| Shooting (charging) | 80° | 120°/s / 60°/s (slower) |
| Releasing a shot or pass | | **0°/s** (locked) |
| Lock move | 78° | |
| Goalkeeper lock move | 75° | |
| Stunned | 83° | |
| Wall run | 115° | |

- **Position:** 5 m behind the player, 0.6 m to the right, framed 2 m above.
- **Pitch limits:** 22° up, 15° down. Since shot height comes from camera pitch, these limits cap how high and low you can aim.
- **Options:** the field of view can be changed in the settings, **up to 90°** (players confirm; the minimum and how it shifts the per-state FOVs are unread). "Magnus sensitivity" scales the mouse curve (default 1).

### "Market" set
- FOV 70–75°.
- **Turns faster:** 300–360°/s horizontal.
- Tighter pitch limits: 10° up, 13° down.
- Sits closer: 4.25–4.5 m behind.

## Network: who wins a simultaneous touch
When several players' interactions with the ball conflict over the network, the server resolves them in this fixed order (first wins):

1. **Goalkeeper**
2. **Sliding tackle**
3. Standing tackle
4. Outfield dive
5. Winning the ball while sprinting
6. Ball control
7. Lock move (inner zone)
8. Lock move
9. Deflection without control

Practical consequences:
- A sliding tackle beats a reception that lands at the same moment.
- The goalkeeper beats everyone.

Combine this with the 0.04 s contest window in [mechanics.md](mechanics.md#12-contested-balls).

### What the files do not show
- The real netcode is in compiled C++: prediction, rollback, and how lag is compensated.
- A `BP_InputDelay_PlayerController` exists but is empty in the shipped build.
- A forced server net-delay setting exists and is **disabled** (-1).
