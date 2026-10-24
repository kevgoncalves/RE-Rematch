# Rematch arena geometry

From each pitch size's shared gameplay level (`00_StadiumCommon_LA_<size>`), which every stadium skin (Coliseum, Geothermic, Hydraulic, Wind, SuperGoal) loads.

## How to read this
- Pitch centre is (0, 0) and the floor is at height 0.
- The pitch length runs along the goal-to-goal axis.
- The walls are plain collision boxes (5–10 m thick), so the numbers below are exact inner faces.
- All in metres.

## The three pitch sizes
| | 73 × 49 | 84 × 57 | 94 × 64 |
|---|---|---|---|
| Length, end wall to end wall | **73** | **84** | **94** |
| Width, side wall to side wall | **49** | **57** | **64** |
| **Ceiling** (invisible, ball can't go higher) | **20.5** | **24.0** | **26.75** |
| Side walls height | to the ceiling | to the ceiling | to the ceiling |
| Goalkeeper area (in the pitch), width × depth | **33.2 × 15.7** | **36.6 × 16.2** | **40.6 × 16.7** |
| Sweeper area | own half (36 m) | own half (41.5 m) | own half (46.5 m) |

- **The pitch is fully enclosed:**
  - the side walls run the whole length;
  - the end walls cover everything except the goal mouth;
  - a ceiling closes the top.

  There is no "out", only rebounds. Bounce values per surface are in [mechanics.md](mechanics.md#11-the-ball).

### Which size is 5v5
Every size's level has kick-off spots for 1 to 5 players per team, so the files don't say which size each mode uses (matchmaking picks the level). The prototype uses 94 × 64 for 5v5: it is the largest, and the field blueprint's default markings are the 94 × 64 ones.

### Surfaces (94 × 64)
Each collision box carries its own physical material, so the ball bounces differently off each one:

| Surface | Material | Kept head-on / glancing |
|---|---|---|
| Floor | grass | 0.475 / 0.95 |
| Side walls | glass | 0.675 / 0.675 |
| **End walls** (beside and above the goal) | **repulsive glass** | **0.425 / 0.625** |
| **Ceiling** (26.75 m) | **absorbent glass** | **0.3 / 0.6** |

A few extra boxes behind the end walls and outside the side walls only block the camera.

## Corners
On the 94 × 64 level the walls meet at a plain 90° corner: the collision has no corner pieces. Players confirm the low corner pieces belong to the 1v1 pitch. (An earlier reading listed 0.92 m pieces, 5.83 m along each wall, on every size; the 73 × 49 and 84 × 57 levels haven't been rechecked.)

The floor trims along the bottom of every wall are visual only (13 cm).

## The goal
The same goal is used on every pitch size.

| | Value |
|---|---|
| **Mouth width** | **8.2 m** (wall opening from −4.1 to +4.1 m) |
| **Mouth height** | **2.77 m** (the end wall starts at 2.77 m above the goal) |
| Scoring volume | 8.16 wide × 4.68 deep × 2.8 tall, behind the goal line |
| Scoring volume starts | ~0.4 m behind the goal line (unconfirmed exact offset) |
| Net depth | ~3.8 m (the back net sits 3.78 m behind the line) |
| Score per goal | 1 |

For scale, a real football goal is 7.32 × 2.44 m. Rematch's goal is **12% wider and 14% taller**.

The net is made of thin collision panels: back, two sides, and a roof.
- The side nets are 2.8 m tall.
- The roof is flat at 2.8 m for about the first 2.2 m behind the line, then slopes about 45° down to the back net, which is roughly 1.2–1.6 m tall.
- Support bars (12.5 cm thick) run along the bottom and top edges of the net.

So a ball can hit the inside of the net and drop.

## Goalkeeper area
- **Shape:** a box centred on the goal line, so half of it (the numbers above) is inside the pitch. It is centred on the goal and goes from the floor to 30 m up.
- **Hysteresis:** you **enter** the area at the inner box and **leave** it only when you cross a box **0.5 m larger** on every side. Hovering on the line won't flicker you in and out of goalkeeper mode.
- **It grows with the pitch:** 33.2 → 36.6 → 40.6 m wide.
- **The painted box:** unconfirmed whether this matches the box drawn on the pitch.

## Sweeper area
- **Shape:** a second, much bigger volume, also centred on each goal line. It is 80 m wide (wider than any pitch) and reaches **0.5 m short of the halfway line**: effectively your **own half**.
- **Hysteresis:** the same 0.5 m on enter/exit.
- **Role:** this is the zone for the Sweeper role and its abilities (Sweeper extra effort runs at 11.5 m/s, see [mechanics.md](mechanics.md#2-movement-speeds)).

## Other zones on the pitch
- **Tactical zones**, probably used for pings/callouts (unconfirmed):
  - two wing lanes, ~12 m wide, along each side;
  - a centre strip in front of each goal, ~10 m wide;
  - two half-spaces per end;
  - a 35 × 21 m zone in front of each goal.
- **Inside each goal:** an "ability blocker" area that blocks some abilities (unconfirmed which).

## Markings
The lines are one texture per pitch size laid over the turf (512 × 512 for the whole pitch). The reader can't decode these textures yet, so the line layout isn't measured. The prototype draws:
- the halfway line;
- a centre circle (radius guessed at 9.15 m) and spot;
- the goal lines;
- the goalkeeper areas, at the gameplay area's size.
