# Measurements from Kev's Rematch clips

Kev recorded these clips of his own play. They stay in his Downloads folder and are never
committed. The measurements below are timings and body shapes seen on screen, written in our own
words. They are used to tune our hand-made poses and movement. No animation data was taken from
the game.

## Clips
| Clip | Length, fps | What is in it |
|---|---|---|
| `1023.mp4` | 25.8 s, 120 fps | Aerial play, a flick up and volley (3.8–4.6 s), a volley control into a dribble (9.7–10.4 s), a lob off the sole (10.74 s), a first-time pass (14.6 s) |
| `Rematch 2026.10.18 - 07.05.42.35.DVR_trimmed.mp4` | 8.6 s, 30 fps | Dribbles past a sliding tackle, then a charged power shot from a near standstill (contact 4.62 s) |
| `MedalTVRematch20261007214753215.mp4` | 30.3 s, 60 fps | A long run with the ball, a shot (~13 s), and the goal replay from a far camera (24–29 s) |
| `MedalTVRematch20261014233928967-trim-…mp4` | 7.9 s, 60 fps | A control, running with the ball, and a charged shot on the run (contact ~4.77 s). The keyboard overlay in the corner does not show presses |
| `xd.mp4`, `xdd.mp4` | 14.9 s / 18.1 s, 60 fps | Extra effort with the ball, dribbles, receiving passes and high balls, goals |

## Kicks
- **Backswing:**
  - The leg cocks back about 0.1 s after the wind-up starts and holds there while the shot charges (0.3–0.4 s in both power shots).
  - The body crouches low and leans well forward over a deeply bent standing knee.
  - The kicking heel is up behind at about hip height.
  - The arm on the standing-leg side reaches out forward, and the other swings back.
  - The ball glows blue while a shot charges.
- **Strike:** from the top of the backswing to the ball takes one or two frames of 30 fps video (≤ 0.067 s). The power shot's contact shows a gold flash.
- **Follow-through:**
  - The kicking knee drives up high in front about 0.1 s after contact.
  - Then the kicking foot lands well ahead in a long stride, with the other leg trailing.
  - On the run, the body keeps going forward through it; this is the forward carry Kev calls the drag.
  - From a near standstill the player hops on the standing foot instead.
- **Wind-up to contact:** a power shot from a standstill took 0.47 s from the first visible wind-up (the data says 0.53 s).
- **Back to running:** after a slow kick (the lob off the sole) the run picks up about 0.2 s after contact.

## Where these went
- `body.rs`:
  - the strike takes 4 frames (it took 8);
  - the backswing comes in within 0.12 s and holds;
  - the follow-through reaches its top in a quarter of the follow time;
  - the kick lands in the `land` stride;
  - the shot's backswing and follow-through poses are deeper;
  - `MAX_JOINT_SPEED` is 2400 deg/s.

## Practice recordings with an input overlay (2026-10-25)
`Videos/2026-10-25 02-33-58.mkv` (17.8 min) and `02-55-47.mkv` (7 min), both at 30 fps. A key
overlay lights each key yellow while it is held. It sits at (0, 861) in the first recording and at
(1361, 856) in the second, and `keys.py` in the scratchpad reads it into press intervals.
- **Bindings seen:** LMB is shoot, RMB is pass, Shift is sprint (double-tapped for extra effort),
  Ctrl is lock, and Tab spawns the cannon ball.
- **Coverage:** most of both recordings is cannon practice, which means volleys and headers on balls
  fired at the player.
- **Ground shots on the run** are at 184–292 s in the first recording, mostly charged for 0.5–0.8 s.
  The second recording has fake shots at 251–296 s, where pass is pressed about 0.3 s into a
  charge.
- **A charged shot pressed during a control** (213.43 s):
  - the wind-up starts as the control ends, about 0.23 s after the press;
  - while it charges, the body leans forward with the heel up behind and both arms out wide at
    shoulder height;
  - the contact flash shows within a frame of the release (0.73 s).
- **Where these went:** the shot's backswing holds the arms out wide, and `KICK_STRIKE_MIN` drops from
  4 to 2 frames.

## Still wanted
These clips are matches, so the moments of the presses are unknown and the camera moves. Short
practice-mode clips would let us measure the slide distance and the exact timings:
- straight sprint, then a tapped pass;
- the same with a tapped shot;
- the same with a tapped lob;
- a charged shot;
- a pass while turning the stick 90°;
- a standing tapped pass.

Each clip should be filmed from the default camera, at 60 fps or more, and say which button was
pressed and when.
