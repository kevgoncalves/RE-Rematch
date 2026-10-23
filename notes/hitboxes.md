# Rematch hitboxes and reach

How shapes are given:
- **Capsule:** radius and total height (2 × half-height).
- **Box:** full width × depth × height (2 × extent).

All in metres, from the game files. These are **sizes only**. Most of these shapes are attached to a bone or offset from the player, and many scale up and down during the animation, so the exact position is not captured here.

## Always-on zones around the player
| Zone | Shape | Size |
|---|---|---|
| **Ball control, standing** | capsule | r **0.50**, h 3.7 |
| **Ball control, moving** | capsule | r **0.90**, h 3.68 |
| Ball control on the ground, standing | sphere | r **0.83** |
| Ball control on the ground, moving | capsule | r 0.60, h 3.6 |
| "No control" zone (unconfirmed: touches that only deflect) | capsule | r 0.40, h 2.0 |
| Lock move (outfield) | capsule | r **0.99**, h 4.2 |
| Lock move, inner | capsule | r 0.675, h 3.7 |
| **Lock move (goalkeeper)** | capsule | r **1.10**, h 4.6 |
| Goalkeeper lock-move deflect | box | 0.9 × 2.0 × 1.9 |
| **Win the ball while sprinting** | box | 1.1 × 1.5 × 1.0 |
| Sprint tap | box | 0.5 × 1.3 × 1.6 |

Moving, your reception zone is almost twice as wide as standing still (0.9 m against 0.5 m radius).

## Action hitboxes
| Action | Shape | Size |
|---|---|---|
| Dash: hitting the ball | capsule | r 0.8–0.9, h 4.5–4.7 |
| Standing tackle | 2 boxes on the tackling leg/body | 0.6 × 0.6 × 0.8 and 0.6 × 0.8 × 1.4 |
| Standing tackle | capsules | r 0.25–0.5, h 0.9–2.1 |
| Sliding tackle | capsules along the body | r 0.32–0.40, h 1.0–1.2 |
| **Goalkeeper sliding tackle** | box | **1.4–2.2** × 1.24 × 2.3 |
| Outfield dive, wind-up | capsule | r **1.2**, h 4.4 |
| Outfield dive, in flight | capsule | r 0.6, h 1.9 |
| Outfield dive, recovery | capsule | r 0.5, h 1.7 |
| Goalkeeper dive, wind-up | capsule | r **1.35**, h 4.6 |
| Goalkeeper dive, in flight | boxes on the hands/body | 1.2 × 0.6 × 0.4, 0.9 × 0.8 × 1.2, 1.2 × 0.8 × 1.14 |
| Goalkeeper dive, recovery | capsule | r 0.6, h 2.8 |
| Goalkeeper parry recovery | capsule | r 0.5, h 2.8 |

## Distances
| | Distance |
|---|---|
| Dash travel | 2.25 m |
| Knock-back from a standing tackle | 2 m |
| Knock-back from a sliding tackle | 3.5 m |
| Knock-back from a protect | 1.7 m |
| Ball radius | ~0.11–0.12 m |

## Field sizes
The map files name three pitch sizes, in metres:
- **73 × 49**
- **84 × 57**
- **94 × 64**

Every stadium (Coliseum, Geothermic, Hydraulic, Wind, SuperGoal) exists in all three. Presumably each size matches a team size (unconfirmed).

Walls, ceiling, goal and goalkeeper-area measurements are in [arena.md](arena.md).
