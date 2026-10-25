# Sound effects: which sound plays when

Rematch's sound runs on Wwise. The game code only *posts events* and sets *switches* (a choice
between variants) and *RTPCs* (live numbers such as the ball's speed). The variant blending itself
lives in the Wwise project, which isn't in our extracted data. So below:
- **found** = read from the game's blueprints or data;
- **inferred** = the most likely reading of what was found;
- **guessed** = our own choice, for our prototype.

None of the game's audio is used. Every sound in `game/src/sfx.rs` is synthesised at startup.

## Sources

All in the main checkout's `tools/rmx/`:
- `out_bp/BP_BallSoundComponent.txt`: the ball's sounds (kicks, bounces, control, roll, goal).
- `out_bp/BP_CharacterSoundComponent.txt`: the player's sounds (light touch, breathing, threat).
- `out/BP_ShockedOnlyReaction.json`, `BP_HomeScoredReaction.json`, `BP_AwayScoredReaction.json` and
  the other `BP_*Reaction.json`: the crowd's reactions and their cooldowns.
- `files.txt`: the asset list. It names every Wwise event, switch and RTPC under `/Game/WwiseAudio/`.

Not extracted, so unknown:
- `DefaultBallShootSFXDataAsset`: which events each kick type posts;
- `DefaultSoundSurfaceDataAsset`: each surface's switch and absorption;
- the locomotion animations' notifies (footsteps);
- the Wwise project itself.

## Units

The game works in centimetres. The ball component divides speeds by 100, so most RTPCs are in m/s.
`RTPC_Shoot_Power` is the exception: the raw launch speed in cm/s.

## Kicks (shots, passes, lobs, push balls)

**Found** (`PlayShootTypeSFX`, called on every shot):
- The kick's gameplay tags pick an entry in the shoot SFX table. The exact tag is tried first, then
  a parent tag. The tags are `ShootAbility.Shoot`, `.Tap` (pass), `.Lob`, `.PushBall`, `.Tap.LowPower`,
  `.Lob.LowPower`, the volley ones, and so on.
- Each entry holds:
  - the events to post on the ball;
  - an optional header event, used when the ball is at least 1.40 m up (0.75 m for a keeper);
  - an optional event on the kicker (local player only, if flagged);
  - flags: check spin, check height, check aim, use sprint, set the ball's speed on the player;
  - an optional fixed "force ratio".
- RTPCs set on every kick:
  - `Shoot_ForceRatio` = the charge, 0..1. An entry can replace it with a fixed value. With "use
    sprint", a sprinting kicker gets 0.8 instead, plus an effort grunt.
  - `Shoot_Power` = the launch speed.
- **Spin, when the entry checks it:**
  - zero spin: the switch `Magnus_Type-NoMagnus`;
  - otherwise the game measures the angle of the spin vector (side part, top/back part) from the
    top/back axis:
    - over 45 degrees, mostly side spin: `FinesseShoot`;
    - 45 degrees or less, mostly top or back spin: `StraightShoot`;
  - the signed angle also goes out as `RTPC_MagnusAngle`, so Wwise can still tell top from back.
- During the kick's freeze (hitstop) the kick sounds are paused, then resumed. A 1 s timer stops any
  that were never resumed.
- A rejected volley plays `Play_SFX_VolleyFailed` and an effort grunt.

**Wwise names that fit:**
- `Play_Ball_Impact_Shoot`, `_Tap` (pass), `_Pushball`, `_Head`, `_Tap_Volley`, `_Head_Volley`;
- `Play_Looping_Shoot_Whoosh_Heavy` and `_Light`: a flying-ball whoosh;
- `Play_SFX_Pushball_Exhausted`, `Play_SFX_FakeShoot` and `_Exhausted`.

**Inferred:**
- Shot power is continuous in the game (two RTPCs), not a few fixed sounds.
- The only hard power lines in the data are the shot tiers that the impact visuals and the kick
  animation use: low power under 29.41 m/s, mid under 34.82 m/s, power shot above. The heavy and
  light whoosh loops probably split at one of them.
- **What counts as max power for a pass or a lob:** a full gauge (force ratio 1). The exact line
  is in the Wwise project. Kev's description (only the max sounds different) fits a full gauge.

**Ours:**

| Kick | Sound | Variants |
|---|---|---|
| Shot | strike + whoosh | 3 powers × 3 spins |
| Pass | strike | max (crisp, louder, short whoosh) or not (round "tock") |
| Lob | strike + rising air | max (crisp, longer air) or not (hollow "thock") |
| Low-power pass / lob | the non-max pass / lob | quieter |
| Push ball, dribble push ball, extra effort's auto tap | toe poke | |
| Fake shot (pass pressed while charging a shot) | leg swish, no ball | |

Shots in detail:
- **Power:**
  - Low (under 29.41 m/s): light thump, short quiet whoosh.
  - Mid (29.41 to 34.82): solid thump.
  - Power (34.82 and up): deep thump + sub boom + a hard crack and a long loud whoosh.
- **Spin:**
  - The shot's built-in lift (210, every shot has it) isn't counted.
  - What's left must be at least 40. If not, the shot is **none**.
  - Then the game's 45-degree split:
    - side spin dominant: grouped with **none** (Kev's "none/sidespin");
    - dip dominant: **topspin**;
    - lift dominant: **downspin** (backspin: the ball floats).
  - The whooshes:
    - none: a flat whoosh, slightly falling;
    - topspin: a steep falling sweep with a fast flutter;
    - downspin: a rising sweep with a slow flutter.
  - *Guessed:* that Kev's "downspin" means the floating backspin shot. If he means the dip, swap
    the two names; the three sounds stay distinct.

## Controls and touches

**Found** (`OnBallOwnershipChanged` → `SetBallControlLocation`):
- When the ball gets a new owner, the switch `Ball_Control_Location` is set from the ball's height:
  - Foot under 0.90 m;
  - Knee under 1.40 m;
  - Chest at 1.40 m and up.

  A Head value exists in Wwise but isn't picked here.
- `RTPC_Ball_Speed` = the ball's speed, then `Play_Ball_Control_Switch` on the new owner.
- **Local player:** also `Play_UI_Ball_Controll`.
- **An opponent:** `Play_SFX_BallControl_Threat`.
- **Light touch:** the character's "ball touched" event plays `Play_Ball_LightTouch` on the ball.

**Wwise names that fit:**
- `Play_Ball_Touch` and `Play_Ball_RollTouch`: running touches;
- `Play_SFX_Dribble_Dash` and `_Exhausted`: Rematch's dribble is the dash button with the ball;
- `Play_DribbleStance_Enter` and `_Leave`.

**Ours:**
- **Control:** foot, knee or chest, by the ball's height when the control started (our control
  glides the ball down to the foot). It plays when the ball becomes yours, as in the game.
  - Louder for a faster ball: full at 25 m/s (guessed).
  - A dash that picks up a loose ball counts too.
- **Running touches with the ball:** a soft tap at each touch (the carry's touch beat), louder when
  you run faster.
- **Dribble and feint:** a body swish and a shoe scuff. A tired dribble is duller and slower.
- **Extra effort taps** on a loose ball: the push ball's toe poke, quieter.

## Ball bounces and hits

**Found** (`OnBounced` → `Bounce SFX`):
- Every bounce reports the speed before the bounce, the hit point and normal, and the physical
  material.
- `RTPC_Ball_Impact` = that speed in m/s.
- `RTPC_Surface_Absorption` comes from the surface table.
- A known surface type sets the `Surface_Type` switch and plays `Play_Ball_Impact_Surface`. A bounce
  with no surface plays `Play_Ball_Impact_Bounce`.
- **Surfaces in Wwise:** Grass, Dirt, Metal, Net, HolographicWall, Plexi, RepulsiveWall.
- The Wwise switch `Bounce_Hit_Strength` has four values: Light, Medium, Strong, PowerShot. Who sets
  it, and at which speeds, isn't in the extracted data.
- There's no minimum speed and no cooldown in the blueprint. Any floor is in Wwise.

**Inferred:**
- Posts and crossbar are the Metal surface.
- Walls are the HolographicWall or Plexi surface.

**Ours:**

| Hit | Sound | When | Full volume at |
|---|---|---|---|
| Ground | thump + slap: light, medium or strong | falling faster than 1 m/s; strong from 9 m/s, medium from 4 | 13 m/s |
| Wall | panel knock | hitting it faster than 1.2 m/s | 25 m/s |
| Post or crossbar | metal ring | hitting it faster than 1 m/s | 25 m/s |

- **All thresholds are guessed.**
- **Ground:** the falling speed sets the strength and the volume.
- **Wall:** side walls, end walls and the ceiling. The speed into the wall sets the volume.
- **Post or crossbar:** told from a wall by the ball's path passing within reach of a post or the bar.
- **Each surface rests 0.08 s** after it sounds, so a ball wedged in a corner doesn't buzz.
- **Distance:** ball hits get quieter with distance from the camera: half volume at 30 m (guessed).
- **How we detect hits:** from the ball's velocity jumping between frames, not from `Ball.hit`.
  `Ball.hit` keeps only the last 240 Hz substep, so at 60 fps it misses three hits in four.

## Goal, net and crowd

**Found:**
- `OnGoalScored` plays `Play_SFX_TeamScoring` when the scoring team is the local player's, else
  `Play_SFX_OpponentScoring`.
- **Crowd reactions** (match effects):
  - a home goal: `Play_Crowd_HomeScore`, cooldown 10 s;
  - an away goal: `Play_Crowd_AwayScore`, cooldown 10 s;
  - a goal-post hit, a keeper's deflecting dive or an "arrest": `Play_Crowd_ShockedReact` (a gasp),
    cooldown 5 s, filtered by the kick type (that filter's list isn't read).
- **Other crowd reactions:**
  - dribbles and long passes: `ShortSuspenseReact`, cooldown 5 s and 10 s;
  - interceptions: `HappyReact`, cooldown 10 s.

**Ours:**
- **A goal:** the net's swish plus a crowd swell (formant-filtered noise with a slow wobble).
  - Our goal resets the ball 0.4 m past the line, before it can reach the net, so the net sound
    plays with the goal.
  - We have no teams, so there is one goal sound.
- **A shot on the post or bar:** also a crowd "ooh", for hits over 8 m/s (guessed), at most once
  every 5 s (the game's cooldown).

## Player movement

**Found in Wwise names only** (the triggers are animation notifies, not extracted):
- **Footsteps:**
  - `Play_Footstep`, with the switch `Footsteps_Speed` = Walk, Run or Rush;
  - `Footsteps_Holowall` = Walk, Run or Rush (steps by the holographic walls);
  - `Play_FS_Landing`;
  - `Play_Fs_Scuff_Start` and `_Stop`;
  - `Play_Slide_Loop`.
- **Extra effort:** `Play_SFX_ExtraEffort` and `Stop_SFX_ExtraEffort`, a loop for while it's on,
  plus `RTPC_IsExtraEffort`.
- **Sprint:** `Play_SFX_Sprint_Start` and `_Stop`.
- **Breathing** (`BP_CharacterSoundComponent`): starts when stamina falls under 70%, stops above it.
  Running out of stamina plays a strong grunt.

**Ours:**
- **Footsteps:** a stride clock on your speed.
  - Steps per second (guessed): 1.6 at walking pace, 2.7 at 5 m/s, 3.3 at 8 m/s, up to 3.6.
  - Walk under 3 m/s, Rush when sprinting, Run otherwise.
  - Soft grass taps, harder and brighter the faster you go.
  - Each step is 3% higher or lower at random, so they don't sound identical.
- **Dash** (without the ball): a body whoosh with two shoe scuffs (guessed from the scuff names).
- **Extra effort:** a rising power-up when it starts, a short falling tone when it ends.

## Not done yet

- **The ball's rolling and flying whoosh loop** (`Play_Ball_RollWhoosh`). In the game it runs while
  the ball moves faster than 0.1 m/s, with the speed and a Doppler shift (relative to the local
  player, every 0.1 s). It needs a live, looping sound we change while it plays.
- **Sprint start and stop**, breathing, grunts, tackles, dives, keeper sounds, headers, volleys,
  the UI's control tick, and crowd beds and reactions other than goals and posts.
- **Surfaces:** one sound per surface family. No dirt, plexi or repulsive-wall variants.
- **Settings:** `SfxSettings` (on/off, master volume) exists in `sfx.rs`, for the settings menu to
  drive. Nothing in the menu sets it yet.
- **Unconfirmed:**
  - the Light/Medium/Strong bounce speeds;
  - every volume curve;
  - the stride rates;
  - which events each kick type posts (the shoot SFX table isn't extracted).
