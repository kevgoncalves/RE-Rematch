# Rematch bots: how the AI decides

Bots (backfill, practice, tutorials) run on Unreal **State Trees**: a tree of states, each with entry conditions and tasks.
- Positioning comes from **EQS queries**: the bot scores candidate points around itself and picks the best.
- Some exact delay values are stored in a binary format that isn't readable here. They are marked "(delay)" below.

## Top-level decision
```
Goalkeeper?                    -> Goalkeeper tree
Outfield
  Opponents have the ball      -> Defense tree
  Team has possession
    Ball has an owner
      I have the ball          -> Attack-with-ball tree
      A teammate has it        -> "Attack without ball" positioning
    Ball is loose
      I'm the closest          -> go get the ball (sprint to ball)
      otherwise                -> leave it to the teammate
```

## Attacking with the ball
1. **Move**, with a short (delay) right after receiving the ball.
2. **Wait to act**, then pick one of:
   - **Pass**: the pass leads the receiver by 2 m (short) up to 15 m (long).
   - **Shoot**: a "decide to shoot" check, then "shoot to goal".
   - **Dribble**.

**Shot targeting:** the bot scores 20 points on a 20 m arc (70° wide) facing the opponent's goal, and shoots at the best one.

**Carrying the ball:** the next spot is chosen on a 2.5 m arc, 20° wide, ahead of the bot.

## Attacking without the ball (getting open)
The bot looks for space using these candidates and scores:

| Candidates | Score rule |
|---|---|
| Points on a ~10.5 m ring and a 20 m ring around it | prefers spots **at least 8 m** from one reference |
| A 2.5 m grid | prefers spots **17–30 m** from another reference |

The references are probably the ball carrier and the goal (unconfirmed).

## Defending
| Situation | Behaviour |
|---|---|
| Ball owned by an opponent, "rush" mode | **sprint** at the ball carrier and go for a **sliding tackle**, then wait (delay) |
| Ball owned by an opponent, "simple defense" | take a spot **5 m from the carrier**, directly in their path; move to the ball, **tackle**, then a 70-unit end delay (unconfirmed unit) |
| Ball loose | go to the ball |
| Otherwise | base defensive position |

## Goalkeeper
| Situation | Behaviour |
|---|---|
| Ball at feet or in hand | short delay, then pass/kick at **60% power** |
| Waiting for a shot | stand on a **1.2 m arc** (90° wide, 10 points) in front of the goal, between ball and goal; the position updates every **0.5 s** |
| Opponent close with the ball | rush the carrier and tackle |
| Shot coming | dive |
| Nothing happening (relax) | a spot **2.5–8 m** from the goal; updates every **3 s** |

## Utility scoring
Bots also use a small utility system: "has ball" scores 0→1 and "has no ball" scores 1→0. It mainly decides whether shooting at the goal is worth it.

## Workshops
The practice modes (pass interception, protect the ball, tap workshop) reuse the same pieces with their own small trees.
