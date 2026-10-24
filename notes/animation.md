# Character body and animation

How Rematch builds and animates its player, and how the prototype rebuilds it with our own body and our own poses. Read from the player skeleton, its physics asset and the anim blueprints' structure; **no animation keyframes were read or used**. Status: **approved; coded up to the transitions** (steps 1–4 of §3: body, locomotion, free-move starts, stops and turns; simple kick and ball-control poses). Lock-move transitions, the actions and the layers beyond the lean are next.

## 1. The body
The prototype body is our own mannequin: plain capsules and boxes, flat colours, no textures. Its sizes come from the game's skeleton (bone positions in the reference pose) and its physics asset (one collision shape per bone, which follows the limbs' thickness). The mesh itself is not used.

### Proportions
Heights are above the ground; the character stands in the game's 1.8 m capsule, feet at its bottom.

| | Rematch | Prototype now |
|---|---|---|
| Total height (mesh bounds) | **1.89 m** | ~1.75 m |
| Pelvis | **1.12 m** | 0.90 m |
| Hip joints | 1.07 m, ±0.096 m apart | 0.88 m, ±0.12 m |
| Thigh / shin | **0.477 / 0.480 m** | 0.43 / 0.40 m |
| Ankle above ground | **0.117 m** | 0.065 m |
| Foot (ankle to ball of the foot) | 0.185 m; the foot is ~0.30 m long, ~0.11 m tall | box 0.28 m |
| Spine joints | 1.25 m, 1.36 m (top of the spine) | |
| Neck / head joint | 1.57 m / 1.69 m (the head reaches 1.89 m) | |
| Shoulder joints | 1.52 m, **±0.155 m** apart | ±0.31 m |
| Upper arm / forearm | **0.267 / 0.259 m**; the hand ~0.20 m | 0.32 / 0.30 m |

### Thickness (physics shapes: radius, and cylinder length between the caps)
| Bone | Shape |
|---|---|
| Pelvis | capsule r 0.108, 0.138 |
| Spine (low, mid, top) | r 0.112, 0.118, 0.112 |
| Chest / shoulders (extra) | r 0.062, 0.138 |
| Collar bones | r 0.07 |
| Neck | r 0.05 |
| Head | r 0.07 (the collision is smaller than the visible head) |
| Thigh | **r 0.10**, 0.245 |
| Shin | **r 0.072**, 0.348 |
| Upper arm | **r 0.048**, 0.185 |
| Forearm | **r 0.042**, 0.175 |
| Hand | box 0.205 × 0.105 × 0.055 |
| Foot | box ~0.30 long, 0.11 tall, ~0.075 wide |

So Rematch's players are tall and long-legged (the pelvis is 59% of the height), with narrow shoulders and slim arms. That is why the prototype's body looked small and stocky.

## 2. How Rematch animates
**Native code decides, and the anim blueprint is a thin data-driven graph.** Each part below is read from structure, names and timings only.

### Modes and cycles
- **A mode picks the set of cycles**, first match wins:
  - volley padding;
  - lock move (with the ball, goalkeeper variants);
  - free move with the ball;
  - celebration;
  - free move.
- **Each set is indexed by speed tier:** V0 idle, V1 walk, V2 run, V3 sprint.
  - Extra effort is a V4 cycle inside the V3 slot.
  - Out of stamina, V3 switches to an exhausted cycle.
- **Free move has a single forward cycle per tier.** The direction comes from turning the body to the movement, plus two corrections:
  - **orientation warping** turns the lower body toward the velocity when it differs from the facing;
  - **stride warping** lengthens or shortens the steps to match the speed.
- **Lock move picks one of four cardinal cycles** (front, right, left, back) in ±45° sectors, with a little overlap so it doesn't flicker at the edges. Left is the mirrored right.
- **Play rate** = clamp(speed ÷ the cycle's own speed, **0.7, 1.5**).
  - Turning sharply speeds it up to ×1.15 without the ball, ×1.05 sprinting, ×1.0 with the ball.
  - "Sharply" is the per-frame turn of the velocity, smoothed.
- **Changing cycle** (tier, cardinal, mode) blends the new one over the old in **0.25 s**, or 0.1 s when the mode changed.
- **The feet stay in phase:** cycles and actions share one step sync group with left/right foot markers. A new cycle, or the run after an action, starts on the right foot.

### Transitions
- **Four kinds:** starts, stops, turns and speed changes (run ↔ sprint). Each is a short clip per tier and direction, authored for one foot and mirrored for the other.
- **The clip doesn't play freely.** Its time follows the code-driven movement: the brake and re-acceleration in turns.md.
  - Its root is rotated so the clip's direction matches the real motion.
  - It is mirrored by the current foot phase.
- **Crossfades:**
  - 0.2 s into a transition;
  - 0.1 s to another of the same kind (a ring of four slots lets a turn blend into a turn);
  - 0.2 s to another kind;
  - 0.3 s back to the cycle, from 0.3 s before the clip's end;
  - 0.1 or 0.2 s when interrupted.
- **Any action cancels a running transition at once.**

### Actions (kicks, dribbles, dashes, controls…)
- **One full-body slot.** The action blends in over **0.05–0.15 s** by move (dribble and tap 0.05, dash and high-power shot 0.1, push ball 0.05–0.15) and out over **~0.25 s**.
- **Foot sync:** 50 of the 56 action montages carry the left/right foot markers, so the run picks up on the right foot after them.
- **Timing comes from notifies:**
  - the availability windows (frame-data.md, responsiveness.md);
  - freeze frames;
  - the shoot notify (the ball leaves);
  - the ball snap;
  - the dribble end.
- **Reaching the ball:**
  - motion warping turns or stretches the action toward a warp target;
  - redirection turns dribble and dash clips toward the input (dashes over 0.15 s);
  - a snap system moves the ball to the foot at the touch.

### Running with the ball
- **Touches are layered over the run cycle.** Per tier there is a touch blend space per foot, by where the ball is (ahead, left, right).
- **The ball springs or snaps to the foot at the touch frame:** instantly within 10 cm, with a spring factor from 15 to 5.
- **Touches come faster on sharp turns:** ×1.0 to ×1.2 as the velocity turns from 20 to 180°/s.

### Additive layers
- **Lean:**
  - roll from the smoothed sideways acceleration (full at 0.06 sprinting, 0.1 with the ball, 0.075 otherwise) × the speed;
  - pitch forward as stamina drops.
  - Applied by a control rig on the spine.
- **Breathing** at weight 1 / 1 / 0.8 / 0.6 by tier, faster with low stamina; an exhausted additive when out of stamina.
- **Lock move "mood"** (a relaxed stance by side and speed).
- **Shoulder contact (hindering):** upper body only, or the arm on the contact side.
- **Look-at:** head, neck and eyes turn toward a target from a 10-entry priority list.
  - Targets: the aim when shooting, the ball-call target, the ball, the nearest opponent when hindered.
  - Each entry has yaw limits of 130–180°, pitch limits of 20–30°, delays, random glances and cooldowns, and a spring.
  - Upper-body weights: spine 0.3 / 0.6 / 1.0, neck and head 1.

### Turn in place (lock move only)
The body holds its yaw while the capsule turns. Past **54°** (goalkeeper 18°), a turn clip plays and unwinds the offset; the offset is capped at 144° (goalkeeper 48°).

### IK: where it is and isn't
- **There is no foot IK in locomotion.** The engine has Sloclap foot-IK classes (planting, hips drop, prediction), but no animation blueprint in the game uses them.
- **Inside the run cycle the IK curves are forced to 0.** The feet come from the cycles, adjusted by stride and orientation warping.
- **IK is used per action**, when an action's own curves turn it on (foot, hand, head, spine), and for the goalkeeper's hands on the ball.

(Correction: an earlier reply said Rematch plants the feet with foot IK. The classes exist; nothing uses them.)

## 3. The prototype rebuild
Same structure and timings, our own poses. In this order, each part reviewed in play before the next.

1. **The body** (§1): capsules per bone at Rematch's sizes, the pelvis at 1.12 m.
2. **Clips from our own key poses.**
   - A clip is a few hand-made key poses (joint rotations), interpolated, with left/right foot markers.
   - Clips can be mirrored, so we author one side.
   - Cycles get four keys: left contact, passing, right contact, passing.
3. **Locomotion:**
   - mode and tier choice as above (V0–V3, V4, exhausted);
   - one forward cycle per tier in free move, with a lower-body yaw toward the velocity (our orientation warping);
   - four cardinals in lock move;
   - play rate clamped 0.7–1.5 with the cornering boost;
   - cycle changes blended over 0.25 s and kept in phase by the foot markers.
4. **Transitions** on our existing turn, start and stop code: the clip's time follows the move's progress, mirrored by foot phase, with the crossfade times above.
5. **Actions** as clips timed to the frame data we already use (contact frames, freeze, interrupts), with the blend in/out times above.
   - The foot meets the ball at the contact frame: the action's own IK, as in the game, only during the action.
6. **Layers:** lean, then look-at, breathing and the lock-move mood.

No IK on the legs while walking or running, unlike the version that was just tried.

## Sources
- **Data:**
  - PlayerBase_Skeleton (reference pose, foot markers L/R);
  - SK_PlayerBase_Physics (body shapes);
  - SK_PlayerBase (bounds only);
  - PlayerBase_Skeleton_AnimBP (state machines, transitions, node inventory, functions; the export stops early, so node wiring, slot names and per-node assets are missing);
  - PostProcess_AnimBP;
  - the locomotion datasources (FreeMove, FreeMoveHasBall, LockMove, LockMoveBall, LockMoveGoalKeeper, Padding, Land, FallLoop);
  - DA_DefaultTurnInPlace and the turn-in-place settings;
  - DA_DefaultAnimLookAtStack;
  - the FullBody, AestheticFullBody, UpperBody and LeftArm blend profiles;
  - the ability montages' headers (blend times, sync groups, notify names).
- **Native class properties:** SCAnimInstance, SCTransitionAnimInstance, RuntimeAnimInstance, SCFootIkAnimInstance and its services, TouchBallComponent, the motion-warping and snap classes.

## Open questions
1. The node wiring and the per-node assets and slot names (the anim blueprint export stops early).
2. The native defaults: transition blend settings per type, stride-warp strength, the touch-ball play rate.
3. Which look-at target types 0, 1, 8 and 9 are.
