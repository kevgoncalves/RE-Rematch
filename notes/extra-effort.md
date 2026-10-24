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
   - **13 m/s** along your forward;
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
3. Releases the ball and applies the lost-ball effect (no re-control for 0.5 s).

## Without the ball: the control tap
- While active, a sprint-tap hitbox (BallSteal.Hitbox.Sprint.Tap) looks for the loose ball. Its parameters 0.2, 0.04 and 5 were not interpreted.
- When it touches the ball it kicks it on (the "control tap"):
  - **12 m/s** along your input direction;
  - same friction 1.2;
  - only the zone and sweeper modifiers apply (so, in practice, none).

## How far the ball runs
At μ = 1.2 the ball rolls speed ÷ 1.2:
- 10.8 m from the 13 m/s auto tap;
- 10 m from the 12 m/s control tap.

Running at extra effort speed you catch it again after about half a second, which is what makes repeated cuts possible.

## Open questions
1. Why the 25% activation cost isn't visible in the game.
2. The sprint-tap hitbox's parameters (0.2, 0.04, 5) and its exact shape.

## Sources
**Data:**
- GA_ExtraEffort (bytecode);
- BP_ExtraEffortAutoTapAbilityData and BP_ExtraEffortControlTapAbilityData;
- BP_ExtraEffortAutoTap_* and BP_ExtraEffortControlTap_* modifiers;
- GE_Cost_ExtraEffort, GE_Cost_ExtraEffort_Maintenance, GE_ExtraEffortStart and GE_ExtraEffort_Deplete_Sprint;
- CT_AbilitiesCost_Stamina (rows Cost_ExtraEffortMaintenance 0.5 and Cost_SprintMaintenanceRate 0.05);
- BP_ExtraEffort_FrictionTrajectoryModifier.
