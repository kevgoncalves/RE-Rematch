# Code map: where each mechanic lives in the game

The remake is rebuilt one part at a time, in three steps per part:

1. **Map:** list the native classes, Blueprints and data assets behind the part.
2. **Interpret:** read the code and the data, and write the rules in our own words in that part's notes (formulas, constants, units, order of operations, and where each one comes from).
3. **Code:** implement the part in the prototype from those notes only, in its own module, with tests that check the rules.

No part gets prototype code before its notes are written and reviewed.

## Sources
- **Native code:** the game's own modules are `Runtime` (Rematch), `SCCore` (Sloclap's shared base, also used by Sifu) and `SCCombatSystem`. They are 1,466 of the exe's 5,140 reflected classes.
  - `tools/natives.py` names 26,689 native entry points from the reflection tables.
  - `tools/classmap.py` lists the game's classes with their properties and functions (local output: `tools/rmx/out_bp/_game_classes.md`).
  - `tools/ghidra/NameAndDump.java` applies the names in Ghidra and decompiles the game modules. The output stays local, outside the repo.
- **Blueprints:** abilities and gameplay Blueprints, extracted with their bytecode (`tools/rmx --script`, summarised by `tools/kismet.py`).
- **Data:** data tables, curve tables, data assets and levels (`tools/rmx`).

Class and property names below are the game's identifiers. Everything else is our description.

## 1. Ball flight and bounces
The ball's flight is a stack of **trajectory modifiers**, each adding one force or rule. Every kick type brings its own stack.

| Class | Role | Parameters |
|---|---|---|
| `BallPhysicComponent` | runs the simulation, bounces, ball rotation, predictions | angular speed decrease over time, critical speed threshold, speed compensation |
| `BallAttributeSet` | the ball's live attributes | parallel / perpendicular / Magnus bounciness, friction, drag force, mass, max speed |
| `BallPhysicSettings` | global settings | speed at which it stops, prediction duration and rate, aim point steps |
| `GravityTrajectoryModifier` | gravity | mass override, free-fall multiplier, max free-fall acceleration, an optional periodic extra force |
| `DragTrajectoryModifier` | air drag | default drag force (or the ball attribute) |
| `MagnusTrajectoryModifier` | spin curving the ball | separate flying and rolling Magnus, default force, forced Magnus axis, delay before Magnus applies, rebounds before forced Magnus |
| `FrictionTrajectoryModifier` | ground friction | default friction and a multiplier |
| `HeightFollowerTrajectoryModifier`, `LocationFollowerTrajectoryModifier` | scripted paths (height or location curves) for special touches | |
| `SpeedKeeperTrajectoryModifier`, `ForceNullifierTrajectoryModifier`, `CurrentForceBonusTrajectoryModifier` | keep speed, cancel forces, bonus from the current force | |
| `TrajectoryStatus` + activate / deactivate conditions | temporary states on the flight | on bounce, distance, duration, inside the goal, quick bounce, volley |

**Known:** bounce values per surface, friction per touch, the ball's mass, max speed and gravity multiplier (mechanics.md §11, arena.md).
**Missing:** the formulas inside each modifier, the units of spin, and the values each kick's stack uses.

## 2. Kicks: direction, force and spin
Each kick ability (`RuntimeShootGameplayAbility`, the GA_Shoot family) carries a `ShootAbilityData`, which picks four strategies and a list of modifiers:

| Piece | Classes | What it decides |
|---|---|---|
| Direction | `ControllerShootDirectionComputation`, `CameraShootDirectionComputation`, `InputForwardShootDirectionComputation`, `FilteredForwardInputShootDirectionComputation`, `LockedDir…`, `LocalDir…`, `MergeWithBallVelocity…`, `ContestMergeWithBallVelocity…`, `Hit…`, `HitBounce…`, `ImpactPoint…`, `TargetActor…` | where the ball goes. The controller version maps camera pitch to trajectory pitch and stick X to yaw, corrects pitch by stick Y, has a pitch range, a direction-lock option and a vertical Magnus multiplier |
| Force | `InputShootForceComputation`, `StaticShootForceComputation`, `ModifierShootForceComputation`, `HitBounceShootForceComputation` | how fast. The input version can count time spent on the input stack |
| Spin | `InputShootRotationForceComputation` | curve from the stick or mouse: X and Y spin curves, tolerance angle, dead zone, the mouse accumulator and its sensitivity option, axis inversion |
| Modifiers | `AngleShootModifier`, `BasicShootModifier`, `JoystickShootModifier`, `SpeedStateShootModifier`, `VerticalRotationInputShootModifier`, with curve / range / simple computation data | multipliers on force, angle or spin, e.g. by speed state or stick axis |
| Flight | `m_TrajectoryModifiers` | the ball-flight stack from part 1 |

Other pieces: `AbilityTask_ApplyShoot`, `AbilityTask_ApplyVolleyShoot`, `AbilityTask_NetworkSyncShootData`, `AnimNotify_Shoot`, `ShootForceAnimationPicker`, `ShootHelpers`, `ShootComputationHelper`.
**Known:** force curves, build-up and contact frames, launch angles, spin curves by charge (mechanics.md §4).
**Missing:** how the direction is computed from the camera, how the pieces combine, and the values per kick type.

## 3. Touching and receiving the ball
| Class | Role |
|---|---|
| `BallPlayersInteractionComponent` | who touches the ball, volley requests and priority, kick-off |
| `TouchBallComponent` | touch handling on the character; snap and push phases come from animation notifies |
| `AbilityTask_BallCapture`, `AbilityTask_PredictBallCapture`, `BallCaptureCondition` | receiving (control) |
| `ContestData`, `BallContestPhase…` | two players on the ball at once |
| `AbilityTask_HandleBallStealTracking`, `BallStealBinder` | steals |
| `BallInterceptionPoint…` | where a player can reach the ball in time |

**Known:** control hitboxes, snap and action frames, contest windows (mechanics.md §5, §12).

## 4. Movement
| Class | Role |
|---|---|
| `RuntimeMovementComponent` → `SCTransitionMovementComponent` → `SCDynamicSpeedMovementComponent` | the character's movement, speeds and transitions |
| `SpeedDescriptionDB`, `RuntimeMovementAttributeSet` | speeds per state from attributes |
| `SCTransitionDB`, `SCCodeDrivenTransitionDataAsset`, `SCCodeDrivenTurnTransitionDataAsset` | start, stop and turn transitions |
| `AbilityTask_HandleCharacterLocomotion`, `AbilityTask_HandleSteeringLocomotion` | locomotion inside abilities |
| `RuntimeSprintGameplayAbility` | sprint |
| `…RootMotionModifier…` | root-motion moves: dribbles, dashes, dives, redirects to the ball |

**Known:** speeds, transition durations and curves (mechanics.md §2–3).

## 5. Stamina and extra effort
`RuntimeAttributeSet`, `StaminaLossCalculation`, `StaminaRegenCalculation`, `OutOfStaminaCalculation`, `ExtraEffortRegenCalculation`, `AbilityTask_ApplyAbilityCostOverTime`.
**Known:** costs and regen curves from the curve tables.
**Missing:** the regen calculations' code (e.g. why the extra effort activation cost doesn't show in-game).

## 6. Abilities and input
The GA_* Blueprints (availability layers, buffers, windows), the input conditions (`…InputConditionData`, `InputConditionMappingDB`, `InputMappingPresetDB`), `InputModifierModeGameOption`, `InputDelayComponent`.
**Known:** most ability rules (dribble.md, controls.md).

## 7. Defence and contact
`SCCombatSystem`: hitboxes and hurtboxes (`HitboxComponent`, `HurtboxSolverComponent`, `BallHurtboxSolver`), attacks (`AbilityTask_HandleAttack`, `AttackComponent`), hit reactions, network hit validation.
**Not started.**

## 8. Goalkeeper
`RuntimeDiveGameplayAbility`, `RootMotionModifier_Dive`, `RootMotionModifier_DiveAssist`, `DiveAssistTimeStretchComputation`, `CameraServiceGoalKeeper`.
**Not started.**

## 9. Camera
`SCCameraComponent` with services (following, lock on the ball, lock move framing, pitch resting, collision, FOV), `RuntimeCameraData`, `SCCameraDB`.
**Partly known** (match-camera-network.md).

## Order
Parts 1 and 2 decide how the ball behaves and are what feels most wrong today, so they come first. Then 3 and 4, then 5 and 6, then 7 to 9.
