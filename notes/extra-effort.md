# Extra effort (spec)

What the extra effort ability does to the gauge and to the ball. Movement speed, acceleration and recharge are in mechanics.md §10. Ball flight is in [ball-flight.md](ball-flight.md).

**Status:** interpreted from the ability Blueprint and its data, waiting for Kev's review. The native hitbox and cost code was not read.

## Gauge
- **Activation cost:** an instant effect takes **25% of the maximum gauge** when the ability commits. The ability also requires that much to start (its cost check is on).
  - Golden Striker players "in the zone" are exempt.
  - In the prototype the activation cost is left out, because Kev sees no instant drop in the game. Why the data's cost doesn't show is an **open question**.
- **Hold drain:** **0.5 per second**, applied in small ticks (every 0.05 s).
- **While active:** it also applies an effect that depletes sprint, and a 0.6 s start phase (tag State.ExtraEffort.Start).
- **Ending:** the ability watches the gauge and ends when the condition is met (gauge empty), or when the input is released.

## With the ball at the start: the auto tap
1. Plays the extra-effort-with-ball animation. During it, everything except dribbles and move interrupts is blocked.
2. Kicks the ball (the "auto tap"):
   - **13 m/s** along the way **your body faces** (a fixed direction in your own frame, not your input);
   - flat;
   - with the extra effort friction override (μ = 0.8 × 1.5 = **1.2**).

   The force is scaled by:

   | Condition | Multiplier |
   |---|---|
   | Your current movement transition is a 135° turn | ×0.8 |
   | Your current movement transition is a 180° turn | ×0.5 |
   | The ball is 20 cm or more off the ground | ×0.9 |
   | Golden Striker "in the zone" | ×1.15 |
   | Sweeper modifier | ×1.15, but its tag check is empty, so it never applies |
3. Releases the ball. You can't control it again for **0.5 s**, other players for **0.3 s** (the ability's ball status, BP_NoControl_ExtraEffort, the same values as a push ball). It also puts the lost-ball effect on you (GE_LostBall: 0.5 s, tag HasLostBall).

## Without the ball: the control tap
- While active, a sprint-tap hitbox (BallSteal.Hitbox.Sprint.Tap) looks for the loose ball. Its parameters 0.2, 0.04 and 5 were not interpreted.
  - The ball control's no-control status doesn't stop it: the hitbox only skips a ball marked no-deflect or under goalkeeper protection (SprintHitbox's condition).
  - Each touch marks the ball: no-deflect 0.35 s and no-volley 0.35 s for everyone, no-control 0.5 s and "no hit, no repeat" 0.5 s for the kicker only (BP_NoSelfInteract_ExtraEffort). So your own next touch waits 0.5 s.
  - **Chops have a grace on the ball** (players confirm). Reaching a ball that runs the way you push, you ride on it for a moment, and turning the stick then sends it the new way. The prototype rides it up to 0.2 s (the hitbox's first parameter, its meaning unread) and chops on a turn past 20°.
- When it touches the ball it kicks it on (the "control tap"):
  - **12 m/s** along your **movement input at the moment of the touch, read relative to the camera** (the direction computation's default space is the camera; it uses the input stored with the touch, with no clamp to your body's facing);
  - flat (vertical angle 0);
  - same friction 1.2;
  - only the zone and sweeper modifiers apply (so, in practice, none). The 135°/180° turn multipliers belong to the auto tap only;
  - the same no-control as the auto tap: 0.5 s for you, 0.3 s for others.
- So there is no separate "cut" rule: every touch sends the ball where you point. Changing direction on the ball is just touching it with a new input, before your body has finished turning.
- With no input at the touch, the computation doesn't keep the last direction. Where it falls back to was not read; the prototype uses your body's facing.

## How far the ball runs
At μ = 1.2 the ball rolls speed ÷ 1.2:
- 10.8 m from the 13 m/s auto tap;
- 10 m from the 12 m/s control tap.

Running at extra effort speed you catch it again after about half a second, just as the 0.5 s no-control ends. That is what makes repeated cuts possible.

## Open questions
1. Why the 25% activation cost isn't visible in the game.
2. The sprint-tap hitbox's parameters (0.2, 0.04, 5) and its exact shape.

## Sources
**Native code:** `InputForwardShootDirectionComputation__vf89` (`FUN_14531da00`, the input direction by space: 0 = the actor's forward, 1 = the camera), its constructor `FUN_1452de730` (defaults: camera space, vertical angle 0, no clamp to the actor's forward, no keeping the last direction).

**Data:**
- GA_ExtraEffort (bytecode);
- BP_ExtraEffortAutoTapAbilityData and BP_ExtraEffortControlTapAbilityData;
- BP_NoControl_ExtraEffort (0.5 s for the kicker only, 0.3 s for everyone) and GE_LostBall (0.5 s);
- BP_ExtraEffortAutoTap_* and BP_ExtraEffortControlTap_* modifiers;
- GE_Cost_ExtraEffort, GE_Cost_ExtraEffort_Maintenance, GE_ExtraEffortStart and GE_ExtraEffort_Deplete_Sprint;
- CT_AbilitiesCost_Stamina (rows Cost_ExtraEffortMaintenance 0.5 and Cost_SprintMaintenanceRate 0.05);
- BP_ExtraEffort_FrictionTrajectoryModifier.
