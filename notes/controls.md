# Rematch default controls

The game's own default bindings, from its input presets: the QWERTY keyboard preset and the default gamepad preset. AZERTY and the "thumb button" presets also exist.

"+" means hold both buttons. "Direction" means the action takes your movement direction (WASD or left stick) into account.

| Action | Keyboard and mouse | Gamepad (Xbox names) | How it triggers |
|---|---|---|---|
| Move | W A S D | left stick | |
| Camera | mouse | right stick | |
| Sprint | Left Shift | LB | hold, with a direction |
| **Extra effort** | **Left Shift twice, then hold** | **LB twice, then hold** | quick press and hold |
| Shoot | left mouse | RT | hold to charge |
| Curve (Magnus) | **mouse movement while charging** | left stick | |
| Pass | right mouse | X | hold to charge |
| Low-power pass | right mouse + Space | X + A | hold both |
| Lob | F | RB + X | hold to charge |
| Low-power lob | X | X + A + RB | |
| Fake shot | right mouse, with a direction | B, with a direction | while charging a shot |
| Push ball / dribble | Space, with a direction | A, with a direction | |
| Lobbed push ball | C | RB + A | |
| Standing tackle (block) | E, with a direction | B, with a direction | |
| Sliding tackle | Left Shift + E | LB + B | hold both |
| Lock move | Left Ctrl | LT | hold |
| Dash | Left Ctrl + C, with a direction | LT + A, with a direction | |
| Dive (outfield) | Q, with a direction | Y, with a direction | |
| Goalkeeper dive | Space, with a direction (long range: Q) | Y, with a direction | |
| Goalkeeper tackle | E, with a direction | B, with a direction | |
| Goalkeeper push ball | V, with a direction | A, with a direction | |
| Look at ball | R | - | |
| Ping | middle mouse | R3 | |
| Quick chat 1 / 2 / 3 | 1 / 2 / 3 | D-pad left / up / right | |
| Voice chat (push to talk) | 4 | D-pad down | hold |

## Notes
- **The same key does different things depending on context.** On keyboard, Space is push ball, dribble and goalkeeper dive. Right mouse is a pass, or a fake shot while you are charging a shot.
- **A longer combination wins.** Holding right mouse + Space gives the low-power pass, not a pass plus a push ball.
- **Shot aim on keyboard and mouse** locks when you start charging; the mouse then curves the ball. Details in [mechanics.md](mechanics.md#direction-height-and-curve).
- **Input buffer:** passes, push balls and tackles fire if pressed up to **0.1 s** before they are allowed. Details in [frame-data.md](frame-data.md#input-buffer).
- **Practice mode** reuses some keys: Tab spawns the cannon ball, middle mouse fires it, and 1 / 2 / 3 change its yaw, force and pitch.
