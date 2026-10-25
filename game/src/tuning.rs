//! Gameplay values from Rematch's data files (see ../notes/*.md for each source).
//! Units: metres, seconds, m/s. Values marked TUNE are not in the files and were picked by feel.

/// Linear interpolation through (x, y) points, clamped outside the range (Rematch's curves are linear).
pub fn curve(points: &[(f32, f32)], x: f32) -> f32 {
    let (first, last) = (points[0], points[points.len() - 1]);
    if x <= first.0 {
        return first.1;
    }
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    last.1
}

// Arena: the 94 x 64 pitch, the largest of the three, used here for 5v5 (arena.md).
// Bevy axes: x = width, y = up, z = goal-to-goal.
pub const HALF_WIDTH: f32 = 32.0;
pub const HALF_LENGTH: f32 = 47.0;
pub const CEILING: f32 = 26.75;
/// Goalkeeper area painted in front of each goal: width x depth into the pitch.
pub const KEEPER_AREA: (f32, f32) = (40.6, 16.7);
pub const GOAL_HALF_WIDTH: f32 = 4.1;
pub const GOAL_HEIGHT: f32 = 2.77;
pub const GOAL_DEPTH: f32 = 3.78;
/// The net's roof (from the goal's collision panels): flat at 2.8 m up to this depth, then
/// sloping 45 degrees down towards the back net.
pub const NET_ROOF_FLAT: f32 = 2.2;
pub const NET_HEIGHT: f32 = 2.8;
pub const GOAL_SCORE_DEPTH: f32 = 0.4; // ball centre this far past the line scores (unconfirmed)
pub const POST_RADIUS: f32 = 0.08;

// Ball (mechanics.md §11)
pub const BALL_RADIUS: f32 = 0.11;
pub const BALL_MAX_SPEED: f32 = 70.0;
// Flight (the ball's native trajectory modifiers). Each one gives an acceleration directly (no
// division by mass) and applies in flight, rolling or both.
/// Gravity, in flight: the world's g times (the ball's Mass attribute + 0.5 x seconds spent
/// falling since the last bounce), so the longer it falls the faster it comes down.
pub const GRAVITY: f32 = 9.8;
pub const BALL_MASS: f32 = 1.625;
pub const FREE_FALL_GROWTH: f32 = 0.5;
/// Air drag, in flight: a = coefficient x DRAG_K x speed², less the more vertical the flight
/// (x (1 - |vertical share of the velocity|)). DRAG_K is half the air density times the ball's
/// cross-section; the coefficient is the ball's DragForce attribute.
pub const DRAG_K: f32 = 0.022808;
pub const DRAG_FORCE: f32 = 0.45;
/// Magnus, flying and rolling: a = MAGNUS_K x speed x spin. Spin is kept in the flight frame
/// (it turns with the ball's path): z curves the ball right, y dips it (negative lifts). Rolling,
/// the dip part pushes along the path instead (a dipping ball brakes, a lifting one runs on).
pub const MAGNUS_K: f32 = 7.986e-4;
/// Spin fades by this share per second (flying and rolling alike).
pub const SPIN_DECAY: f32 = 1.0;
/// Rolling: a = -friction x velocity. The coefficient is the ball's Friction attribute times the
/// last touch's multiplier and weight, so a roll goes speed / friction metres.
pub const BASE_FRICTION: f32 = 0.8;
pub const ROLL_STOP_SPEED: f32 = 0.05; // TUNE: the decay never quite reaches zero
pub const GRASS_BOUNCE: (f32, f32) = (0.475, 0.95); // (perpendicular, parallel)
/// The 5v5 cage: glass side walls, "repulsive" glass end walls, "absorbent" glass ceiling.
pub const WALL_BOUNCE: (f32, f32) = (0.675, 0.675);
pub const END_WALL_BOUNCE: (f32, f32) = (0.425, 0.625);
pub const CEILING_BOUNCE: (f32, f32) = (0.3, 0.6);
pub const NET_BOUNCE: (f32, f32) = (0.08, 0.15);
pub const POST_BOUNCE: f32 = 0.875;

// Movement (mechanics.md §2). Top speeds are the character's locomotion attributes
// (DefaultCharacterLocomotionAttributeTable; the speed tables read them, not their fallback
// floats). Accelerations come from the time to reach full speed.
pub const RUN_SPEED: f32 = 5.0;
pub const SPRINT_SPEED: f32 = 8.0;
pub const BALL_RUN_SPEED: f32 = 4.5;
pub const BALL_SPRINT_SPEED: f32 = 6.5;
pub const RUN_ACCEL: f32 = 5.0 / 0.22;
pub const SPRINT_ACCEL: f32 = SPRINT_SPEED / 0.6;
pub const BALL_RUN_ACCEL: f32 = 4.5 / 0.6;
pub const BALL_SPRINT_ACCEL: f32 = 6.5 / 0.8;
pub const STOP_DECEL: f32 = 5.0 / 0.38;
/// Above your top speed (out of extra effort, a dash or a sprint) you slow to it at this rate
/// instead of dropping to it at once: an extra-effort tap is a little burst (Kev: the pros' mini
/// dash).
pub const OVERSPEED_DECEL: f32 = 15.0;
/// Each extra-effort touch on the ball keeps your own magnet off this long, so letting go of extra
/// effort doesn't hand you the ball at once (the game's no-control for the kicker is 0.5 s; Kev
/// wants the magnet back a little sooner).
pub const EE_TOUCH_NO_CONTROL: f32 = 0.3;
/// Letting go of extra effort within its start phase (the game's first 0.6 s, its burst) keeps your
/// magnet off this long too, which the pros use to run over a push ball and take it further on.
/// Kev: only then; let go later and the magnet is back at once, touch delays and all. TUNE.
pub const EE_END_NO_CONTROL: f32 = 0.2;
pub const EE_START_PHASE: f32 = 0.6;
/// After such a tap its burst carries you on: for this long (s) your speed above your top fades at
/// only this rate (m/s^2) instead of OVERSPEED_DECEL, and a shot taken faster than you can run with
/// the ball keeps all of that speed (Kev: the EE drag shot of earlier Rematch, which everyone
/// liked). TUNE.
pub const EE_TAP_CARRY: f32 = 0.6;
pub const EE_TAP_DECEL: f32 = 5.0;
/// Time to stop from full speed by state (the stop transitions): run, with ball, sprint, sprint with ball.
pub const STOP_TIME: [f32; 4] = [0.38, 0.7, 0.68, 1.0];

// Turn transitions (turns.md): a sharp change of the movement input within 50 ms brakes you along
// your old velocity down to a pivot speed, then re-accelerates you to full top speed along a
// direction that keeps turning toward the input. You never brake to a stop.
/// One phase: duration (s) and its curve over normalised time (brake 1 -> 0, re-acceleration 0 -> 1).
pub type TurnPhase = (f32, &'static [(f32, f32)]);
pub const LIN_DOWN: &[(f32, f32)] = &[(0.0, 1.0), (1.0, 0.0)];
pub const LIN_UP: &[(f32, f32)] = &[(0.0, 0.0), (1.0, 1.0)];
const V2_TURN90_STOP: &[(f32, f32)] = &[(0.0, 1.0), (0.5, 0.25), (1.0, 0.0)];
const V2_TURN90_START: &[(f32, f32)] = &[(0.0, 0.0), (0.6, 0.6), (1.0, 1.0)];
const V3_TURN90_STOP: &[(f32, f32)] = &[(0.0, 1.0), (0.2, 0.7), (1.0, 0.0)];
const V3_TURN90_START: &[(f32, f32)] = &[(0.0, 0.0), (0.8, 0.8), (1.0, 1.0)];
const V3_TURN180_STOP: &[(f32, f32)] = &[(0.0, 1.0), (0.55, 0.65), (1.0, 0.0)];

/// (brake, re-acceleration, pivot as a share of top speed) by sprinting, ball, and the band
/// (`big` = the 180 band; otherwise the 90-degree data).
pub fn turn_data(sprint: bool, ball: bool, big: bool) -> (TurnPhase, TurnPhase, f32) {
    match (sprint, ball, big) {
        (false, false, false) => ((0.14, V2_TURN90_STOP), (0.13, V2_TURN90_START), 0.85),
        (false, false, true) => ((0.3, LIN_DOWN), (0.3, LIN_UP), 0.6),
        (true, false, false) => ((0.165, V3_TURN90_STOP), (0.35, V3_TURN90_START), 0.7),
        (true, false, true) => ((0.4, V3_TURN180_STOP), (0.35, LIN_UP), 0.5),
        (false, true, false) => ((0.25, LIN_DOWN), (0.1, LIN_UP), 0.55),
        (false, true, true) => ((0.35, LIN_DOWN), (0.32, LIN_UP), 0.55),
        (true, true, false) => ((0.32, LIN_DOWN), (0.25, LIN_UP), 0.7),
        (true, true, true) => ((0.5, LIN_DOWN), (0.3, LIN_UP), 0.45),
    }
}

/// The band for an angle between two movement inputs (degrees): None to just steer, Some(false)
/// for the 90-degree data, Some(true) for the 180 band. Without the ball: over 70 deg (walking:
/// only over 120); with it: over 60.
pub fn turn_band(angle: f32, ball: bool, walking: bool) -> Option<bool> {
    let middle = if ball { 60.0 } else if walking { 120.0 } else { 70.0 };
    if angle > 120.0 {
        Some(true)
    } else if angle > middle {
        Some(false)
    } else {
        None
    }
}

/// How the turn's direction follows the input: rotation rate away from and back toward the old
/// direction (deg/s), and the share of the brake and of the re-acceleration in which it may turn.
pub fn turn_steer(ball: bool, big: bool) -> (f32, f32, f32, f32) {
    match (ball, big) {
        (false, false) => (270.0, 90.0, 0.8, 1.0),
        (false, true) => (180.0, 180.0, 0.5, 1.0),
        (true, false) => (270.0, 90.0, 0.6, 0.8),
        (true, true) => (180.0, 180.0, 0.4, 0.6),
    }
}

/// A new turn may cut into a running turn's re-acceleration after this share of it (never into
/// the brake).
pub fn turn_cut_in(big: bool, sprint: bool) -> f32 {
    match (big, sprint) {
        (false, _) => 0.8,
        (true, false) => 0.6,
        (true, true) => 0.4,
    }
}
/// Inputs count as "recent" for this long after they stop being current.
pub const TURN_INPUT_HISTORY: f32 = 0.05;
/// Entering a turn slower than top speed skips at most this share of the brake.
pub const TURN_CATCH_UP_MAX: f32 = 0.2;
/// The body turns to the turn's direction over at most this long.
pub const TURN_FACE_TIME: f32 = 0.2;
/// A stick under this counts as walking (the walk/run threshold isn't read yet: TUNE).
pub const WALK_INPUT: f32 = 0.5;
pub const TURN_RATE_RUN: f32 = 400.0; // degrees per second
pub const TURN_RATE_BALL_RUN: f32 = 180.0; // with-ball speed table's max angular speed (pi rad/s)
pub const TURN_RATE_SPRINT: f32 = 180.0;
pub const TURN_RATE_BALL_SPRINT: f32 = 90.0;
/// Past 90 deg behind your heading, the body turns this much faster (deg/s).
pub const BODY_CATCH_UP: f32 = 60.0;
pub const LOCK_SPEED: f32 = 5.25;
pub const LOCK_BALL_SPEED: f32 = 4.0;
pub const LOCK_ACCEL: f32 = LOCK_SPEED / 0.2; // full speed in 0.2 s
pub const LOCK_BALL_ACCEL: f32 = LOCK_BALL_SPEED / 0.2;
/// Turning to face the camera in lock move. The speed table says 400 deg/s for outfielders (nothing
/// with the ball), but players find the turn to the front much quicker (TUNE).
pub const LOCK_TURN_RATE: f32 = 400.0; // the lock-move speed table's rotation rate
/// Stance (lock move with the ball) lasts at least this long from the press (GA_DribbleStance).
pub const STANCE_MIN_TIME: f32 = 0.15;
/// Angle between facing and movement (degrees) to speed multiplier.
pub const LOCK_SPEED_BY_ANGLE: [(f32, f32); 3] = [(0.0, 1.0), (90.0, 1.0), (180.0, 0.9)];
pub const LOCK_BALL_SPEED_BY_ANGLE: [(f32, f32); 4] = [(0.0, 1.0), (90.0, 0.95), (135.0, 0.88), (180.0, 0.88)];
/// Lock move's footwork at full speed, from Kev's practice recording (V1 360-375 s): side shuffles
/// (one step out and one in), and steps going forward and back, per second.
pub const LOCK_SIDE_SHUFFLES: f32 = 2.4;
pub const LOCK_FORWARD_STEPS: f32 = 4.4;
pub const LOCK_BACK_STEPS: f32 = 6.0;
/// Asking to go back in lock move, you drop into a short braking crouch (Kev's recording: about
/// 0.3 s): in over this long, and out over this long once you back away at this share of lock speed.
pub const LOCK_BRAKE_IN: f32 = 0.08;
pub const LOCK_BRAKE_OUT: f32 = 0.2;
pub const LOCK_BRAKE_UNTIL: f32 = 0.6;
// Dash (mechanics.md §8): speed (m/s) over the seconds since the press. The game's curves are in
// seconds: their keys fall on the dash's events (blend-in 6f, after-dash 14f, second dash free 42f).
/// When you're free (the dash animation's blend-out), by sector: forward, side, back. Kev's
/// practice recording has you back in the lock stance 0.40-0.50 s after a side dash's press.
pub const DASH_FREE: [f32; 3] = [32.0 / 60.0, 27.0 / 60.0, 32.0 / 60.0];
pub const SECOND_DASH_FREE: [f32; 3] = [42.0 / 60.0, 27.0 / 60.0, 29.0 / 60.0];
/// Keeping the stick within 15 deg of the dash's way leaves it early from these frames (after the
/// burst, which now comes later).
pub const DASH_EXIT_FROM: f32 = 20.0 / 60.0;
pub const SECOND_DASH_EXIT_FROM: f32 = 25.2 / 60.0;
pub const DASH_EXIT_ANGLE: f32 = 15.0;
/// In Kev's practice recording the dash doesn't burst at the press: you gather for about 0.1 s, the
/// lunge shows from +100-160 ms, is widest at +180-330 ms, and covers about 2.3 m by the time you
/// are back in the stance. Both dashes run that shape, the second a little weaker.
pub const DASH_SPEED: [(f32, f32); 6] = [(0.0, 2.5), (0.1, 3.0), (0.18, 7.5), (0.3, 7.5), (0.45, 3.0), (1.0, 3.0)];
pub const SECOND_DASH_SPEED: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 4.0), (0.2, 7.0), (0.3, 7.0), (0.5, 2.0), (1.0, 2.0)];
pub const DASH_COOLDOWN: f32 = 0.8;
pub const DASH_COST: f32 = 0.75;
/// The second dash can cut into the first from here, while the first one's cooldown runs.
pub const SECOND_DASH_FROM: f32 = 14.4 / 60.0;

// Dribble: the dash you do with the ball, from lock move (notes/dribble.md). The second one in a
// chain is the feint. Speeds (m/s) run over DRIBBLE_SPEED_TIME, then hold until the move ends.
pub const DRIBBLE_SPEED_TIME: f32 = 45.0 / 60.0;
/// From here the feint, dribble push balls, passes and shots are allowed.
pub const DRIBBLE_WINDOW_OPENS: f32 = 12.0 / 60.0;
/// Kev wants dribble 1 chains back (as before the patch that widened the feint window): the feint
/// window closes this long before the dribble's end, and a press after it is a new dribble 1.
pub const FEINT_WINDOW_CUT: f32 = 0.2;
/// With lock move let go during a dribble 1, the next press is a new dribble 1 from this far into
/// it (Kev: 0.2 s was too quick back to back, 0.55 s too slow, 0.35 s still a little quick). TUNE.
pub const DRIBBLE_REPEAT: f32 = 0.45;
/// With lock move held, a press this long before a dribble 1's end is a new dribble 1 (between the
/// feint window's close and this, a press does nothing; Kev: chaining was a little quick). TUNE.
pub const DRIBBLE_CHAIN_LATE: f32 = 0.1;
pub const DRIBBLE_FORWARD: [(f32, f32); 4] = [(0.0, 4.0), (0.2, 6.8), (0.5, 4.0), (1.0, 4.0)];
pub const DRIBBLE_SIDE: [(f32, f32); 5] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.35, 3.4), (1.0, 3.4)];
pub const DRIBBLE_BACK: [(f32, f32); 5] = [(0.0, 3.0), (0.1, 5.8), (0.2, 5.8), (0.45, 3.0), (1.0, 3.0)];
pub const FEINT_FORWARD: [(f32, f32); 5] = [(0.0, 4.0), (0.1, 6.0), (0.3, 6.0), (0.6, 4.0), (1.0, 4.0)];
pub const FEINT_SIDE: [(f32, f32); 5] = [(0.0, 4.0), (0.2, 6.8), (0.3, 6.8), (0.5, 3.0), (1.0, 3.4)];
pub const FEINT_BACK: [(f32, f32); 5] = [(0.0, 3.0), (0.1, 5.8), (0.2, 5.8), (0.5, 2.5), (1.0, 3.0)];
/// The forward feint out of a straight or side dribble is the roulette (Kev's practice recording,
/// V1 634.07, 635.50, 640.47 s): the sole stops the ball, the turn to the left starts at 0.23 s,
/// faces back at 0.45 s and holds there to 0.52 s, and is round again at 0.83 s (seconds from the
/// press; 270 degrees comes at about 0.64 s). You go on only a little meanwhile, then pick up.
pub const ROULETTE_TURN: (f32, f32, f32, f32) = (0.23, 0.45, 0.52, 0.83);
pub const ROULETTE_SPEED: [(f32, f32); 4] = [(0.0, 2.5), (0.3, 1.0), (0.9, 1.0), (1.0, 2.0)];
/// Out of a fast cut the forward feint turns you out the cut's way (degrees beyond your facing)
/// into a deep crouch and back, facing on again by the second figure (s; V1 177.43, V2 200.10 s).
/// You nearly stop in the crouch, then go on.
pub const TURN_OUT: (f32, f32) = (75.0, 0.27);
pub const TURN_OUT_SPEED: [(f32, f32); 4] = [(0.0, 3.0), (0.15, 1.0), (0.4, 3.5), (1.0, 4.0)];
/// How fast the roulette and turn-out draw the ball to where they hold it (s).
pub const FEINT_BALL_EASE: f32 = 0.05;
pub const EXHAUSTED_DRIBBLE_FORWARD: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.35, 4.0), (0.8, 2.5), (1.0, 2.5)];
pub const EXHAUSTED_DRIBBLE_SIDE: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.35, 3.5), (0.8, 1.5), (1.0, 1.5)];
pub const EXHAUSTED_DRIBBLE_BACK: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.25, 3.5), (0.4, 2.5), (1.0, 2.5)];
/// How long each move lasts (its animation), as (forward, side, back).
pub const DRIBBLE_LENGTH: [f32; 3] = [54.0 / 60.0, 50.0 / 60.0, 64.0 / 60.0];
pub const FEINT_LENGTH: [f32; 3] = [65.0 / 60.0, 50.0 / 60.0, 53.0 / 60.0];
pub const EXHAUSTED_DRIBBLE_LENGTH: [f32; 3] = [65.0 / 60.0, 63.0 / 60.0, 67.0 / 60.0];
/// You get the stick back when a dribble's animation starts to blend out, 15 frames before its
/// end; its permissions (feint, dribble push ball, no new dribble) last to the end
/// (responsiveness.md §2). Feints to the side: 35f right, 41f left (averaged here).
pub const DRIBBLE_FREE: [f32; 3] = [39.0 / 60.0, 35.0 / 60.0, 49.0 / 60.0];
pub const FEINT_FREE: [f32; 3] = [56.0 / 60.0, 38.0 / 60.0, 38.0 / 60.0];
pub const EXHAUSTED_DRIBBLE_FREE: [f32; 3] = [50.0 / 60.0, 48.0 / 60.0, 52.0 / 60.0];
/// Animations blend out over 15 frames: push ball chases hand back then at the latest.
pub const BLEND_OUT: f32 = 15.0 / 60.0;
/// A move's direction needs this much stick tilt; less is no direction (forward).
pub const MOVE_DIRECTION_DEAD_ZONE: f32 = 0.3;
/// Push balls ignore stick tilts under this.
pub const PUSH_BALL_DEAD_ZONE: f32 = 0.1;
/// For 0.1 s after a push ball or pass starts, a longer chord can still take over.
pub const CHORD_WINDOW: f32 = 0.1;
pub const DRIBBLE_COST: f32 = 0.5;
pub const DRIBBLE_REGEN_DELAY: f32 = 0.9;
/// Dribbles and dashes turn the body to their way linearly over these (BP_RedirectCardinalsToForward
/// 12f; the dash's redirection 9f).
pub const DRIBBLE_TURN_TIME: f32 = 12.0 / 60.0;
pub const DASH_TURN_TIME: f32 = 9.0 / 60.0;
pub const EXHAUSTED_SPRINT: f32 = 6.0;
pub const EXHAUSTED_BALL_SPRINT: f32 = 5.0;

// Stamina (mechanics.md §1)
pub const MAX_STAMINA: f32 = 10.0;
pub const SPRINT_START_COST: f32 = 0.4;
pub const SPRINT_DRAIN: f32 = 0.75; // per second (unconfirmed unit)
pub const BALL_SPRINT_DRAIN: f32 = 1.125;
pub const STAMINA_REGEN: f32 = 4.0;
pub const REGEN_DELAY: f32 = 0.4;
pub const EXHAUSTED_REGEN: f32 = 2.0;
pub const EXHAUSTED_REGEN_DELAY: f32 = 2.0;
pub const EXHAUSTED_EXIT: f32 = 3.0;
pub const SPEED_BY_STAMINA: [(f32, f32); 4] = [(0.0, 0.8), (5.9, 0.95), (6.0, 1.0), (8.0, 1.05)];

// Shooting and passing (mechanics.md §4). The force curves run on the charge, 0..1: the time held
// since the press against the kick's build-up less a frame, so the gauge is full on the frame
// before the contact: ~0.52 s for shots, ~0.38 s for passes and lobs. The kick goes off then if
// you're still holding; a release fires it too, but never before the contact frame.
pub const SHOT_FORCE: [(f32, f32); 3] = [(0.0, 24.0), (0.3, 24.0), (1.0, 38.0)];
/// Pass peaks at 60% charge (~0.14 s held); holding longer drops it to 25.6 m/s before it climbs back to 30.65.
pub const PASS_FORCE: [(f32, f32); 5] = [(0.0, 20.0), (0.3, 20.0), (0.6, 32.0), (0.601, 25.6), (1.0, 30.65)];
pub const LOB_FORCE: [(f32, f32); 3] = [(0.0, 16.0), (0.3, 16.0), (1.0, 27.0)];
/// The force curves' last key; spin scales with the raw seconds held over this.
pub const MAX_HOLD: f32 = 1.0;
pub const SHOT_BUILDUP: f32 = 32.0 / 60.0;
pub const PASS_BUILDUP: f32 = 24.0 / 60.0;
pub const LOB_BUILDUP: f32 = 24.0 / 60.0;
/// A tapped kick leaves this much before its animation's contact frame: the game takes its
/// network margins (0.12 + 0.03 s) off the build-up (mechanics.md §4), and Kev finds taps slower
/// than the game's without them. A tapped shot leaves at 11f.
pub const TAP_NETWORK_MARGIN: f32 = 0.15;
/// A pass or lob's contact frame: the game's is its 24f build-up, but Kev finds tapped passes and
/// lobs slower off the foot than in the game, so they strike as soon as a low-power shot (20f, so a
/// tap leaves at 11f). Their force still counts the full build-up. TUNE.
pub const PASS_CONTACT: f32 = 20.0 / 60.0;
/// A shot, pass or lob draws the ball to the kicking foot by its contact (m ahead, m to that
/// foot's side), no faster than KICK_BALL_PULL against you, so the foot meets it (the carry left it
/// up to 0.8 m ahead and the leg kicked air).
pub const KICK_BALL_SPOT: (f32, f32) = (0.4, 0.1);
pub const KICK_BALL_PULL: f32 = 6.0;
/// A kick released late (charged) leaves once its strike has had this long, so the ball doesn't
/// fly before the leg swings through (Kev's practice recording: a charged shot's contact flash
/// shows within a frame of 30 fps video of the release).
pub const KICK_STRIKE_MIN: f32 = 2.0 / 60.0;
pub const WINDUP_SPEED_SCALE: f32 = 0.5; // TUNE: how much you slow down winding up a pass or lob
/// A shot keeps your run-up: through the build-up you slide on in the same direction at a share
/// of your speed at the press that shrinks to this (HighPowerBeforeShootScaleSpeedData: 0.75, 0.6
/// sprinting at 4 m/s or more, 0.5 out of a dribble; Kev wants the body to slide on, not brake). TUNE.
pub const SHOT_SLIDE: f32 = 0.95;
pub const SHOT_SLIDE_SPRINT: f32 = 0.9;
pub const SHOT_SLIDE_FROM_DRIBBLE: f32 = 0.9;
/// Passes and lobs keep all of your speed at the press, in that direction, through the build-up
/// and after the touch (BeforeTapScaleSpeedData, TapAfterShootSpeedData): walking drags you a
/// little, running more, out of a dribble as running (Kev); standing still (under this) none.
pub const PASS_DRAG_MIN_SPEED: f32 = 0.3;
/// The after-touch drag runs to the pass animation's move interrupt (frames 18 to 63).
pub const PASS_AFTER_TOUCH: f32 = 45.0 / 60.0;
/// With the stick left alone, after the touch you slide on at that speed until the shot's move
/// ends: can act moving at 45f, so 25f after a low-power shot's contact (Kev wants a power shot to
/// slide as long).
pub const SHOT_AFTER_TOUCH: f32 = 25.0 / 60.0;
/// Extra shot speed from your own speed (ShootForceByCharaSpeed, m/s), scaled by how strong the
/// shot is: none up to 27.85 m/s, all of it from 38 m/s (ModifierRatioFromShootForce).
pub const SHOT_FORCE_BY_RUN_SPEED: [(f32, f32); 3] = [(0.0, 0.0), (6.0, 1.25), (9.5, 4.0)];
pub const SHOT_RUN_BONUS_BY_FORCE: [(f32, f32); 3] = [(0.0, 0.0), (0.275, 0.0), (1.0, 1.0)];
/// A shot slower than this uses the low-power kick, struck at 20 frames instead of 32
/// (GA_Shoot shot types, ShootDataTable): quick shots go off sooner.
pub const SHOT_HIGH_POWER_SPEED: f32 = 29.41;
pub const SHOT_LOW_POWER_CONTACT: f32 = 20.0 / 60.0;
/// Camera pitch (degrees, + is up) to launch elevation (degrees).
pub const ELEVATION_BY_PITCH: [(f32, f32); 6] =
    [(-24.9, 2.93), (-14.6, 2.93), (-13.2, 3.99), (-7.6, 9.29), (0.7, 17.42), (4.0, 20.46)];
/// Pulling the curve input down (dip) launches the shot higher, up to this many degrees at full input.
pub const SHOT_DIP_PITCH_CORRECTION: f32 = 4.0;
/// The shot flies from the ball toward where the crosshair meets the arena walls (traced 150 m
/// from the camera, through the floor, which the trace ignores).
pub const SHOT_TRACE_RANGE: f32 = 150.0;
/// Passes leave flat; held past 60% lift (degrees). The game keys this and the lob angle by seconds
/// held up to 1 s; with the kick going off as the gauge fills, they run on the charge instead, so a
/// full lob is the flat one (players: lobs were far too strong held at 42 deg).
pub const PASS_LIFT_BY_HOLD: [(f32, f32); 4] = [(0.0, 0.0), (0.6, 0.0), (0.601, 3.9), (1.0, 6.0)];
/// The side curve input turns the shot this many degrees the other way, so the ball bends back
/// toward the crosshair (Curve_TrajectoryYawByXAxis, applied against the input).
pub const AIM_NUDGE: f32 = 3.0;
pub const SIDE_SPIN: [(f32, f32); 8] = [(-1.0, -825.0), (-0.9, -650.0), (-0.5, -350.0), (-0.1, 0.0), (0.1, 0.0), (0.5, 350.0), (0.9, 650.0), (1.0, 825.0)];
/// Spin scaling by charge (the game's external curves, which override the editor ones).
pub const SIDE_SPIN_BY_CHARGE: [(f32, f32); 2] = [(0.0, 1.02), (1.0, 0.8)];
// Keyboard and mouse: the shot direction locks when you start charging, and mouse movement
// while charging feeds a Magnus accumulator instead of the camera (BP_MK_InputRotationForceComputation).
pub const MAGNUS_ACCUMULATOR_SENSITIVITY: f32 = 0.2;
pub const MOUSE_AXIS_SCALE: f32 = 0.07; // Unreal's default MouseX/MouseY axis scale (unconfirmed for Rematch)
pub const PASS_FRICTION: f32 = BASE_FRICTION * 1.2; // players' passes (analog power data)
/// Built-in spin per kick (the flight frame's y; + dips): passes lift a little, lobs dip.
/// ponytail: the analog pass also switches to drag 0.75 x 1.25 after its first bounce; skipped.
pub const PASS_FORCED_SPIN: f32 = -200.0;
pub const LOB_FORCED_SPIN: f32 = 300.0;
pub const SHORT_LOB_FORCED_SPIN: f32 = 420.0;
// Pass aiming (Tap_OrientationDataTable_*, BP_*_Tap_FilteredInputDirectionComputation)
pub const PASS_STICK_DEAD_ZONE: f32 = 0.3;
pub const PASS_MOUSE_DEAD_ZONE: f32 = 0.4;
pub const PASS_FORWARD_SNAP: f32 = 6.0; // degrees
pub const SHOT_FRICTION: f32 = BASE_FRICTION;
/// Stick/mouse forward-back to top/back spin; pulling back is much stronger (confirmed in-game).
pub const TOP_SPIN: [(f32, f32); 8] = [(-1.0, 780.0), (-0.9, 660.0), (-0.5, 330.0), (-0.1, 0.0), (0.2, 0.0), (0.5, -40.0), (0.9, -80.0), (1.0, -120.0)];
pub const TOP_SPIN_BY_CHARGE: [(f32, f32); 2] = [(0.0, 0.95), (1.0, 0.85)];
/// Every shot's built-in spin: a small lift (the native code subtracts the forced axis).
pub const SHOT_BASE_SPIN: f32 = -210.0;
pub const FAKE_SHOT_COST: f32 = 1.5;
/// A pass press fakes the shot only this early in its charge; later it goes on (Kev's practice
/// recording: 0.17-0.36 s in always faked, 0.50-0.57 s never).
pub const FAKE_SHOT_WINDOW: f32 = 0.45;
/// The fake: the leg swings low over the ball for this long, then you crouch over it, and can act
/// again after (standing, moving, tired standing, tired moving: the game's frame data).
pub const FAKE_SHOT_SWING: f32 = 0.1;
pub const FAKE_SHOT_LOCK: [f32; 4] = [0.33, 0.57, 0.67, 0.83];
/// Through the fake you shuffle at this share of your top speed. TUNE.
pub const FAKE_SHOT_SPEED: f32 = 0.35;

// Fixed-force kicks (StaticShootForceComputation in each ability's data), m/s
pub const SHORT_WINDUP: f32 = 16.0 / 60.0;
pub const SHORT_PASS_SPEED: f32 = 24.25;
pub const SHORT_PASS_FRICTION: f32 = BASE_FRICTION * 1.5 * 1.675; // multiplier x the override's weight
/// Lob launch angle by charge (overrides the aim): fuller is flatter, 44 deg to 30.26 at full.
pub const LOB_ANGLE_BY_HOLD: [(f32, f32); 2] = [(0.3, 44.0), (1.0, 30.26)];
pub const SHORT_LOB_ANGLE: f32 = 52.0;
pub const SHORT_LOB_SPEED: f32 = 13.2;
pub const PUSH_BALL_SPEED: f32 = 11.5;
pub const PUSH_BALL_FRICTION: f32 = BASE_FRICTION * 2.8; // about 5.1 m of roll at 11.5 m/s
pub const PUSH_BALL_WINDUP: f32 = 8.0 / 60.0;
/// After any kick you can't control your own ball for 0.5 s (GE_LostBall). Push balls and
/// extra-effort taps also mark the ball: 0.5 s for you, 0.3 s for others (BP_NoControl_PushBall,
/// BP_NoControl_ExtraEffort).
pub const PUSH_BALL_NO_CONTROL: f32 = 0.5;
pub const PUSH_BALL_COST: f32 = 0.375;
pub const PUSH_BALL_REGEN_DELAY: f32 = 1.0;
/// Sprinting switches the push ball's force up (BP_FastTap_ForceFromSpeed bytecode), but in Kev's
/// practice recording a sprinted push straight on doesn't shoot away and die: the ball leads you by
/// about 0.4 m at +0.27 s, 1.7-1.8 m at +0.53-0.67 s, still about 1.5 m at +1.0 s, and you take it
/// back at about +1.33 s. A little more speed than a normal push and the ground's own friction match
/// that, chasing at your sprint.
pub const PUSH_BALL_SPRINT_FACTOR: f32 = 1.05;
pub const SPRINT_PUSH_BALL_FRICTION: f32 = BASE_FRICTION;
/// Ball speed by how far the push turns you: up to 60 deg, to 95 deg, beyond. A push into a turn
/// in the recording stays within about 0.8 m of you and is yours again by about +0.8 s: chased at
/// your run or your sprint, these keep it there (at a sprint it leads by 0.4 m at most).
pub const PUSH_BALL_SIDE_FACTOR: f32 = 0.72;
pub const PUSH_BALL_BACK_FACTOR: f32 = 0.64;
pub const EXHAUSTED_PUSH_BALL_SPEED: f32 = 11.25;
pub const EXHAUSTED_PUSH_BALL_LIFT: f32 = 13.0; // degrees
/// After the touch you chase the ball on a scripted speed (m/s over the rest of the move),
/// as (forward, side, back); a turn of more than 30 deg waits until the move ends.
pub const PUSH_BALL_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 5.0), (0.5, 5.5), (1.0, 5.5)], &[(0.0, 3.5), (0.15, 4.5), (0.7, 5.0), (1.0, 5.0)], &[(0.0, 2.5), (0.222, 2.5), (1.0, 5.5)]];
pub const SPRINT_PUSH_BALL_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 7.0), (1.0, 7.0)], &[(0.0, 3.5), (0.15, 6.5), (0.7, 7.8), (1.0, 7.8)], &[(0.0, 3.5), (0.302, 3.5), (1.0, 7.5)]];
pub const EXHAUSTED_PUSH_BALL_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 5.5), (1.0, 5.5)], &[(0.0, 2.8), (0.15, 3.8), (0.7, 5.0), (1.0, 5.0)], &[(0.0, 2.5), (0.222, 2.5), (1.0, 5.5)]];
pub const PUSH_BALL_LENGTH: [f32; 3] = [55.0 / 60.0, 55.0 / 60.0, 53.0 / 60.0];
/// You're free to act again this long after the press.
pub const PUSH_BALL_FREE: f32 = 23.0 / 60.0;
pub const RUN_TURN_LIMIT: f32 = 30.0; // degrees
/// Kev's softer hand-back (out of dribbles, passes, lobs, shots) and the ippy's turn: a pivot where
/// your heading and body swing round together at this rate (deg/s) for up to this long (s), your
/// speed dipping to this share while more than 90 deg off. TUNE.
pub const PIVOT_RATE: f32 = 900.0;
pub const PIVOT_TIME: f32 = 0.35;
pub const PIVOT_SPEED: f32 = 0.6;
/// Kev: a push ball just after a dribble ends still counts as the dribble's (no direction
/// falloff), for this long (s).
pub const DRIBBLE_PUSH_GRACE: f32 = 0.3;
// Push ball during a dribble: no direction falloff; x1.35 when sprinting.
pub const DRIBBLE_PUSH_BALL_SPEED: f32 = 11.0;
pub const DRIBBLE_PUSH_BALL_SPRINT_FACTOR: f32 = 1.35;
pub const DRIBBLE_PUSH_BALL_COST: f32 = 0.6;
/// You chase at a constant speed: normal, sprinting, exhausted.
pub const DRIBBLE_PUSH_BALL_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 6.0), (1.0, 6.0)], &[(0.0, 7.8), (1.0, 7.8)], &[(0.0, 5.0), (1.0, 5.0)]];
pub const DRIBBLE_PUSH_BALL_LENGTH: [f32; 3] = [52.0 / 60.0, 47.0 / 60.0, 74.0 / 60.0];

// Extra effort (mechanics.md §10)
pub const MAX_EXTRA_EFFORT: f32 = 1.0;
/// Kev's practice recording (the gauge on the HUD, nine uses): held, the gauge drains 0.45-0.48 a
/// second after its start cost, so a full gauge runs dry about 1.55 s in, Shift still held or not.
pub const EXTRA_EFFORT_DRAIN: f32 = 0.48;
pub const EXTRA_EFFORT_REGEN: f32 = 0.0625;
/// With stamina full, the gauge refills 3x faster (ExtraEffortRegenMultiplier 3 at threshold 1.0; players confirm).
pub const EXTRA_EFFORT_REGEN_FULL_STAMINA: f32 = 0.1875;
pub const EXTRA_EFFORT_SPEED: f32 = 9.5;
pub const EXTRA_EFFORT_ACCEL: f32 = 100.0;
pub const TURN_RATE_EXTRA_EFFORT: f32 = 150.0;
/// A reversal in extra effort (past this many degrees) is a cut (Kev: sharp, with a drastic
/// change-of-direction move, only for a full turn back): the body swings to the new way at
/// EE_CUT_TURN deg/s and a plant-and-push pose plays over EE_CUT_TIME.
pub const EE_CUT_ANGLE: f32 = 150.0;
/// In extra effort the body follows your way this fast (deg/s), so the torso never trails the legs.
pub const EE_BODY_TURN: f32 = 720.0;
pub const EE_CUT_TURN: f32 = 1080.0;
pub const EE_CUT_TIME: f32 = 0.25;
/// Extra effort can't control the ball: starting it taps the ball on, and running into the
/// ball taps it on again. Both use the extra effort friction override.
pub const EXTRA_EFFORT_TAP_SPEED: f32 = 13.0;
pub const EXTRA_EFFORT_KNOCK_SPEED: f32 = 12.0;
/// Reaching a ball that runs the way you push, you ride on it this long before it is tapped on; a
/// turn of the stick past EFFORT_CHOP_ANGLE in that time chops it the new way (players confirm a
/// grace on the ball; 0.2 s is the sprint-tap hitbox's first parameter, its meaning unread).
pub const EFFORT_GRACE: f32 = 0.2;
pub const EFFORT_CHOP_ANGLE: f32 = 20.0;
pub const EXTRA_EFFORT_FRICTION: f32 = BASE_FRICTION * 1.5;
/// At extra effort speed you catch the ball again about half a second after each tap, so a
/// full gauge (one ~1.5 s hold) gives three cuts, as players report.
/// Starting it takes a quarter of the gauge at once (the ability's cost effect): in Kev's practice
/// recording the gauge drops from full to about 0.75 on the first frame after the second Shift
/// press, every time.
pub const EXTRA_EFFORT_START_COST: f32 = 0.25;
/// Once started, extra effort runs at least this long (s) even if Shift is let go at once: a 0.1 s
/// tap in the recording keeps draining the gauge for about 0.2 s and costs about 0.3 in all.
pub const EE_MIN_ACTIVE: f32 = 0.2;
/// The auto tap (starting extra effort with the ball) is weaker in the middle of a big turn:
/// x0.8 in a 135 deg turn, x0.5 in a 180 deg one; x0.9 for a ball 20 cm or more off the ground.
pub const EXTRA_EFFORT_TAP_IN_135_TURN: f32 = 0.8;
pub const EXTRA_EFFORT_TAP_IN_180_TURN: f32 = 0.5;
pub const EXTRA_EFFORT_TAP_HIGH_BALL: (f32, f32) = (0.2, 0.9);
/// Ball centre distance at which the body touches it (capsule 0.35 m + ball).
pub const EXTRA_EFFORT_KNOCK_REACH: f32 = 0.35 + BALL_RADIUS + 0.05;
/// In extra effort a ball less than this far ahead of you along your way is being overtaken: the
/// body knocks it on even through your own push ball's no-control (m). TUNE.
pub const EE_OVERTAKE_AHEAD: f32 = 0.15;

// Ball control (hitboxes.md)
// Ball control zones (the character's control hitboxes; m, centred at pitch level, `forward` ahead
// of you): standing up to 2.5 m/s, moving above it. The ball's own hit sphere is 0.12 m.
pub const CONTROL_SWITCH_SPEED: f32 = 2.5;
pub const BALL_HIT_RADIUS: f32 = 0.12;
/// (forward offset, radius, half height) of the capsules; the standing ground zone is a sphere.
/// Kev wants a bigger magnet than the game's (0.50 / 0.83 standing, 0.90 moving, 0.60 on the
/// ground): almost on the ball, it must be taken.
pub const CONTROL_STANDING: (f32, f32, f32) = (0.10, 0.65, 1.85);
pub const CONTROL_STANDING_GROUND: (f32, f32) = (0.10, 0.95);
pub const CONTROL_MOVING: (f32, f32, f32) = (0.30, 1.0, 1.84);
/// Running you can also take a ball rolling away from you, once it is about under you: a ground
/// zone with no direction check (the game's sits 0.4 m behind you, r 0.6; Kev wants it closer in).
pub const CONTROL_MOVING_GROUND: (f32, f32, f32) = (-0.10, 0.60, 1.80);
/// Lock move without the ball catches from any direction. The game's radius is 0.675; Kev wants
/// the stance to reach a little further than any zone without it.
pub const CONTROL_LOCK_MOVE: (f32, f32, f32) = (0.10, 1.1, 1.85);
/// The normal zones refuse a ball heading away from you: this close to "away" (degrees). A ball
/// slower than CONTROL_SLOW_BALL is taken whichever way it rolls (Kev: on top of a slow ball it
/// must be yours; the game only lets a ball at rest through).
pub const CONTROL_AWAY_ANGLE: f32 = 75.0;
pub const CONTROL_SLOW_BALL: f32 = 1.5;
/// Your own push ball isn't controlled by itself for this long while you leave the stick alone
/// (Kev: a push ball with no input shouldn't end in a control).
pub const OWN_PUSH_HANDS_OFF: f32 = 1.5;
/// A dash pressed almost on top of a loose ball (this close, m) is a dribble at once (Kev: from
/// further off it is a dash, and the ball is controlled the normal way).
pub const DASH_DRIBBLE_REACH: f32 = 1.2;
/// The ball becomes yours 12 frames into the control (it settles at your feet), and you can act
/// from the control's action frame, by the height you take it at: feet and thighs 14f, stomach
/// 16f, chest 20f, head 22f.
pub const CONTROL_SNAP: f32 = 12.0 / 60.0;
pub const CONTROL_ACT_BY_HEIGHT: [(f32, f32); 4] = [(0.6, 14.0 / 60.0), (1.1, 16.0 / 60.0), (1.5, 20.0 / 60.0), (f32::INFINITY, 22.0 / 60.0)];
/// Kev's ippy (as before Season 1, which he says was 100% consistent, sprint or not): after your
/// own push ball, with the stick turned back past IPPY_ANGLE, as soon as you may take the ball
/// again (within IPPY_WINDOW of the touch), if it is within IPPY_REACH you pivot round over it and
/// take it, the foot pulling it round in front of you as you swing (CONTROL_TURN_SPOT out).
pub const IPPY_ANGLE: f32 = 120.0;
pub const IPPY_WINDOW: f32 = 1.0;
pub const IPPY_REACH: f32 = 2.0;
/// Once the ippy's turn has started the ball is still taken this much further off.
pub const IPPY_REACH_TURNING: f32 = 0.5;
/// The ippy's turn starts this long before you may take the ball again, so you are already
/// swinging round as you take it (Kev: the turn came late).
pub const IPPY_TURN_LEAD: f32 = 0.15;
pub const CONTROL_TURN_SPOT: f32 = 0.45;
/// Touches come up to 20% more often as your way turns, from 20 to 180 deg/s.
pub const TOUCH_TURN_RATES: (f32, f32) = (20.0, 180.0);
pub const CONTROL_MAX_HEIGHT: f32 = 2.0;
/// Where a control settles the ball in front of you (Kev's practice recording: 0.3-0.5 m). TUNE.
pub const DRIBBLE_OFFSET: f32 = 0.4;
/// Lock move with the ball: where the ball sits under the right sole (m ahead, m right), how far
/// it rolls ahead the way you move (seconds of your velocity), and how fast it eases there (the
/// stance's snap lands at 17f, so most of the way in about 0.3 s). Kev's practice recording has it
/// about 0.3 m right of the body and only a little ahead, under the sole of the lock stance.
pub const STANCE_BALL: (f32, f32) = (0.17, 0.30);
pub const STANCE_BALL_LEAD: f32 = 0.03;
pub const STANCE_BALL_TIME: f32 = 0.1;
/// Where a push ball's touch draws the ball to, ahead of you the way you push (m). TUNE.
pub const PUSH_TOUCH_REACH: f32 = 0.4;
/// While an extra-effort chop is possible, the ridden ball sits this far ahead of you (m), under
/// your feet. TUNE.
pub const EFFORT_RIDE_AHEAD: f32 = 0.12;
/// Running with the ball, it travels from touch to touch (TouchBallComponent and the run-with-ball
/// cycles' touch data): each touch is aimed this far ahead of you, every few steps.
pub const TOUCH_AHEAD_RUN: f32 = 0.20;
pub const TOUCH_AHEAD_SPRINT: f32 = 0.25;
/// Seconds between touches at the cycles' own speeds: a touch every 3 steps of the 0.6 s run cycle,
/// every 2 of the 0.567 s sprint cycle.
pub const TOUCH_EVERY_RUN: f32 = 3.0 * 0.6 / 2.0;
pub const TOUCH_EVERY_SPRINT: f32 = 2.0 * 0.567 / 2.0;
/// The ball leaves the foot at this many times the average speed to the next touch, slowing to
/// meet it.
pub const TOUCH_LAUNCH: f32 = 1.6;
/// How hard the next touch's spot is pulled to the newest prediction: 15 early in the touch, 5 from
/// halfway.
pub const TOUCH_SPRING: [(f32, f32); 2] = [(0.35, 15.0), (0.5, 5.0)];
/// How far to the side the ball may be, by how far ahead it is (m).
pub const TOUCH_SIDE_LIMIT: [(f32, f32); 3] = [(0.0, 0.4), (0.2, 0.6), (0.8, 2.4)];
/// A control blends the ball to the foot (sine-out) unless it is this close.
pub const CONTROL_SNAP_NEAR: f32 = 0.1;
/// A control brings the ball to your foot no faster than this against you (m/s; the ippy's pull
/// round a little faster), so a ball taken from the edge of the magnet glides in instead of
/// jumping (Kev: the ball moved too fast all of a sudden).
pub const CONTROL_PULL_SPEED: (f32, f32) = (5.0, 9.0);
/// In the ippy the ball keeps its own momentum and eases into the pull over this long (s), instead
/// of taking your velocity at once (Kev: it looked like a teleport).
pub const IPPY_BALL_BLEND: f32 = 0.1;
/// The ball you carry never moves faster than this against you (m/s), kicks aside: a sudden
/// change of way, the stance's sole or a touch glides it there instead of jumping it.
pub const CARRY_MAX_REL_SPEED: f32 = 6.0;
pub const NO_RECONTROL_TIME: f32 = 0.5; // GE_LostBall: no control of your own kick for 0.5 s

// The rainbow flick: the lobbed push ball (notes/volleys-and-flicks.md §4), GA_LobbedPushBall's data.
/// 11.5 m/s at 65 deg above the ground, scooped up off your foot.
pub const FLICK_SPEED: f32 = 11.5;
pub const FLICK_ANGLE: f32 = 65.0;
/// The touch comes 22f after the press flicking ahead (within 45 deg of your facing), 16f to the
/// sides or back (DT_LobbedPushBall).
pub const FLICK_WINDUP_AHEAD: f32 = 22.0 / 60.0;
pub const FLICK_WINDUP_TURNED: f32 = 16.0 / 60.0;
/// The flick's way against your facing at the touch, as a dot product: ahead from 0.9 (about 26 deg),
/// behind under -0.25 (about 104 deg). Each way's (speed factor, degrees added to the angle): ahead
/// x0.982 +3.5, sideways x0.9 +10, behind x0.975 +3.5 (BP_LobbedPushBall_ForceFromDirection and
/// _AngleFromDirection).
pub const FLICK_AHEAD_DOT: f32 = 0.9;
pub const FLICK_BEHIND_DOT: f32 = -0.25;
pub const FLICK_BY_WAY: [(f32, f32); 3] = [(0.982, 3.5), (0.9, 10.0), (0.975, 3.5)];
/// Sprinting at the press: x1.2 and 12.5 deg flatter, so it goes further (BP_LobbedPushBall_ForceFromSpeed).
pub const FLICK_SPRINT: (f32, f32) = (1.2, -12.5);
/// Out of stamina: faster and flatter, (m/s, deg) (BP_ExhaustedLobbedPushBallAbilityData).
pub const EXHAUSTED_FLICK: (f32, f32) = (13.0, 58.0);
/// A huge forced dip (the game's Magnus -2500, our spin.y +2500): it goes up steeply and hooks down
/// about 5 m on, just over a defender's head. Ground friction and air drag are the defaults.
pub const FLICK_SPIN: f32 = 2500.0;
pub const FLICK_FRICTION: f32 = BASE_FRICTION;
/// Nobody can control it for 0.5 s (BP_NoControl_LobbedPushBall).
pub const FLICK_NO_CONTROL: f32 = 0.5;
pub const FLICK_COST: f32 = 0.75;
/// Moving through the wind-up (LobbedPushBallBuildUp*Data, m/s): flicking ahead you run onto the
/// ball at 6 m/s if moving (over 1 m/s), 4 standing, 3.5 out of stamina, the flick's way.
pub const FLICK_WINDUP_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 6.0), (1.0, 6.0)], &[(0.0, 4.0), (1.0, 4.0)], &[(0.0, 3.5), (1.0, 3.5)]];
/// Flicking to the side or back you stop to do it: your speed x1 at 10% of the wind-up, nothing by
/// 80%; from under 2.5 m/s a 5 m/s step that dies the same way.
pub const FLICK_TURNED_BRAKE: [(f32, f32); 4] = [(0.0, 1.0), (0.1, 1.0), (0.8, 0.0), (1.0, 0.0)];
pub const FLICK_TURNED_STEP: [(f32, f32); 4] = [(0.0, 5.0), (0.1, 5.0), (0.8, 0.0), (1.0, 0.0)];
pub const FLICK_STEP_BELOW: f32 = 2.5;
/// After the touch (LobbedPushBallDash*Data): ahead you run on at your speed for 20f; sideways or
/// back you come to a stop by a quarter (back: 30%) of 36f. The body turns along the ball's way over
/// the 13f after the touch (the redirect window), and the stick takes you back from 35f after the
/// press (the push-ball layer).
pub const FLICK_AFTER_AHEAD: (f32, &[(f32, f32)]) = (20.0 / 60.0, &[(0.0, 1.0), (1.0, 1.0)]);
pub const FLICK_AFTER_SIDE: (f32, &[(f32, f32)]) = (36.0 / 60.0, &[(0.0, 1.0), (0.25, 0.0), (1.0, 0.0)]);
pub const FLICK_AFTER_BACK: (f32, &[(f32, f32)]) = (36.0 / 60.0, &[(0.0, 1.0), (0.3, 0.0), (1.0, 0.0)]);
pub const FLICK_REDIRECT: f32 = 13.0 / 60.0;
pub const FLICK_FREE: f32 = 35.0 / 60.0;
// The flick out of a dribble (§5, GA_DribbleLobbedPushBall): from the dribble's 12th frame to its end.
/// 12.5 m/s at 65 deg every way (no way modifiers); sprinting x1.1 and 5 deg flatter; out of stamina
/// 12.75 m/s at 62 deg (dribble.md).
pub const DRIBBLE_FLICK_SPEED: f32 = 12.5;
pub const DRIBBLE_FLICK_SPRINT: (f32, f32) = (1.1, -5.0);
pub const EXHAUSTED_DRIBBLE_FLICK: (f32, f32) = (12.75, 62.0);
/// The touch 16f after the press, every way (DT_LobbedPushBall's dribble rows).
pub const DRIBBLE_FLICK_WINDUP: f32 = 16.0 / 60.0;
/// You keep the dribble's pace through the wind-up, the flick's way (BP_DribbleLobbedPushBallBuildUpData):
/// 5 -> 6.5 m/s by 15% moving (over 1 m/s), 5 -> 6 by 10% standing, 3.5 -> 4.5 out of stamina; after
/// the touch 20f at your speed (DribbleLobbedPushBallDashData).
pub const DRIBBLE_FLICK_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 5.0), (0.15, 6.5), (1.0, 6.5)], &[(0.0, 5.0), (0.1, 6.0), (1.0, 6.0)], &[(0.0, 3.5), (0.1, 4.5), (1.0, 4.5)]];
pub const DRIBBLE_FLICK_AFTER: (f32, &[(f32, f32)]) = (20.0 / 60.0, &[(0.0, 1.0), (1.0, 1.0)]);

// Volleys (§0): first-time kicks on a loose ball you don't own. Pressed while the volley diamond
// shows (fx::volley_target), the kick meets the ball where that query found it, when the ball gets
// there (0.33-0.6 s on): you run there meanwhile, and no control takes the ball first.
/// Above this the meeting point is an air candidate: the push volleys become their jumping forms.
pub const VOLLEY_GROUND_MAX: f32 = 2.1;
/// At the touch the ball must still be this close to where it was to be met (m), or the volley
/// misses (something else touched it). TUNE.
pub const VOLLEY_REACH: f32 = 1.0;
/// The approach runs at whatever speed gets you there in time, at most the query's limit (9.2 m/s,
/// 11 in extra effort).
pub const VOLLEY_APPROACH_SPEED: (f32, f32) = (9.2, 11.0);
/// Where the ball meets you at the touch, by body part (foot, thigh, stomach, chest, head, jumping
/// chest, jumping head): (m ahead of your root, m out to the ball's side). TUNE.
pub const VOLLEY_CONTACT: [(f32, f32); 7] = [(0.45, 0.15), (0.35, 0.12), (0.3, 0.0), (0.28, 0.0), (0.22, 0.0), (0.3, 0.0), (0.25, 0.0)];
/// The body sets itself for the touch this long before it (the run cycle plays until then). TUNE.
pub const VOLLEY_SET: f32 = 0.15;
/// Jumping touches leave the ground from 16f before the touch to 22f after (the clips' fly windows
/// around their touch), high enough for the chest (1.35 m) or the head (1.75 m) to meet the ball:
/// 0.15 to 0.8 m. TUNE.
pub const VOLLEY_AIR: (f32, f32) = (16.0 / 60.0, 22.0 / 60.0);
pub const VOLLEY_JUMP_REACH: (f32, f32) = (1.35, 1.75);
pub const VOLLEY_JUMP_LIMITS: (f32, f32) = (0.15, 0.8);

// The volley push ball (§3, GA_VolleyPushBall).
/// The body part by the ball's height (m): foot under 0.3, thigh under 0.7, stomach under 1.2, chest
/// under 1.65, head up to 2.1 (BP_VolleyPushBallMontagePicker).
pub const VOLLEY_PUSH_HEIGHTS: [f32; 4] = [0.30, 0.70, 1.20, 1.65];
/// Flat at 11.25 m/s, weaker off a higher ball (1 -> 0.8 by 3 m, read at the height less 0.15 m), x1.4
/// sprinting at the touch, x0.9 pushed 45 deg or more off your facing; and never more than 30 deg off
/// your facing (BP_VolleyPushBallAbilityData and its BP_VolleyFastTap_* modifiers).
pub const VOLLEY_PUSH_SPEED: f32 = 11.25;
pub const VOLLEY_PUSH_BY_HEIGHT: [(f32, f32); 2] = [(0.0, 1.0), (3.0, 0.8)];
pub const VOLLEY_PUSH_SPRINT: f32 = 1.4;
pub const VOLLEY_PUSH_TURNED: (f32, f32) = (45.0, 0.9);
pub const VOLLEY_PUSH_CLAMP: f32 = 30.0;
/// It rolls a little short (friction x1.6) and dips (the game's Magnus -1000, our spin.y +1000).
pub const VOLLEY_PUSH_FRICTION: f32 = BASE_FRICTION * 1.6;
pub const VOLLEY_PUSH_SPIN: f32 = 1000.0;
/// Above 2.1 m, the jumping head push: 9 m/s at 40 deg, x0.7 to the sides (45-135 deg off your
/// facing), x0.65 back, default friction and no forced spin (BP_VolleyAerialPushBallAbilityData).
pub const AERIAL_PUSH: (f32, f32) = (9.0, 40.0);
/// The jumping push takes the ball with the chest up to here, with the head higher (m). TUNE: the
/// data has both clips but its picker wasn't read.
pub const VOLLEY_AERIAL_CHEST_MAX: f32 = 2.4;
pub const AERIAL_BY_WAY: [f32; 3] = [1.0, 0.7, 0.65];
/// After the touch you run after it at 5.5 m/s (8.5 sprinting, 5 tired: the redirect and dash
/// data); the stick takes you back from 12f after the touch (only sprint and extra effort are
/// allowed then), the run ends at 26f (the push-ball layer).
pub const VOLLEY_PUSH_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 5.5), (1.0, 5.5)], &[(0.0, 8.5), (1.0, 8.5)], &[(0.0, 5.0), (1.0, 5.0)]];
pub const VOLLEY_PUSH_FREE: f32 = 12.0 / 60.0;
pub const VOLLEY_PUSH_AFTER: f32 = 26.0 / 60.0;

// The volley lobbed push ball (§6, GA_VolleyLobbedPushBall): the flick of a loose ball.
/// The body part by the ball's height (m): foot under 0.4, thigh under 0.8, stomach under 1.2, chest
/// under 1.6, head up to 2.1 (BP_VolleyLobbedPushBallMontagePicker); above it the jumping flick.
pub const VOLLEY_FLICK_HEIGHTS: [f32; 4] = [0.40, 0.80, 1.20, 1.60];
/// 13.5 m/s at 75 deg, x r and r x -10 deg added, where r falls with the height of the touch (the
/// ball's lowest point, m): 1 on the ground, 0.7 at 1.79 m, 0.5 at 3 m. So off the ground it goes up
/// at 65 deg (BP_VolleyLobbedPushBallAbilityData, _ForceFromHeight).
pub const VOLLEY_FLICK: (f32, f32) = (13.5, 75.0);
pub const VOLLEY_FLICK_BY_HEIGHT: [(f32, f32); 3] = [(0.0, 1.0), (1.791, 0.7), (3.0, 0.5)];
pub const VOLLEY_FLICK_HEIGHT_ANGLE: f32 = -10.0;
/// Sprinting at the press: x1.1 and 10 deg flatter (_ForceFromSpeed). Out of stamina: 14.5 m/s at
/// 68 deg, the height modifier only.
pub const VOLLEY_FLICK_SPRINT: (f32, f32) = (1.1, -10.0);
pub const EXHAUSTED_VOLLEY_FLICK: (f32, f32) = (14.5, 68.0);
/// Above 2.1 m the jumping flick: 10 m/s at 65 deg, x0.7 sideways, x0.65 back (AERIAL_BY_WAY), no
/// forced spin (BP_VolleyAerialLobbedPushBallAbilityData).
pub const AERIAL_FLICK: (f32, f32) = (10.0, 65.0);
/// After the touch you turn after the ball (the redirect): 3 m/s, rising to 5.25 by half of it
/// (5 to 7.5 coming in faster than 1 m/s); the push-ball layer opens 22f after the touch.
pub const VOLLEY_FLICK_RUN: [&[(f32, f32)]; 2] = [&[(0.0, 3.0), (0.1, 3.0), (0.5, 5.25), (1.0, 5.25)], &[(0.0, 5.0), (0.1, 5.0), (0.5, 7.5), (1.0, 7.5)]];
pub const VOLLEY_FLICK_AFTER: f32 = 22.0 / 60.0;

// The volley pass (§1, GA_VolleyTap).
/// Volleys charge by seconds held, full at 0.25 s (their force curves' last key; read as seconds,
/// TUNE if Kev finds it too quick). Still held at the touch, the touch reads it then.
pub const VOLLEY_CHARGE_FULL: f32 = 0.25;
/// The body part by the ball's height (m): foot under 0.2, thigh under 0.7, stomach under 1.2, chest
/// under 1.7, head under 2.3, then the jumping header (BP_VolleyTapBuildupMontagePicker).
pub const VOLLEY_PASS_HEIGHTS: [f32; 5] = [0.20, 0.70, 1.20, 1.70, 2.30];
/// Speed by seconds held: the pass's curve squeezed into 0.25 s (BP_VolleyTapAbilityData_AnalogPower).
pub const VOLLEY_PASS_FORCE: [(f32, f32); 5] = [(0.0, 20.0), (0.087, 20.0), (0.1725, 32.0), (0.1726, 25.6), (0.25, 30.65)];
/// x by the ball's height (m): charged past 69% (a lifted pass) gently, otherwise down to 0.8 by
/// 0.7 m; a ball above 1 m also x (1 to 35%, 0.89 at 69%, 1 full) of the charge
/// (BP_VolleyTap_ForceDependingBallHeightAndTime).
pub const VOLLEY_PASS_FLAT_BY_HEIGHT: [(f32, f32); 6] = [(0.11, 1.0), (0.115, 1.05), (0.18, 1.0), (0.20, 0.96), (0.70, 0.80), (3.2, 0.72)];
pub const VOLLEY_PASS_LIFTED_BY_HEIGHT: [(f32, f32); 3] = [(0.15, 1.0), (1.0, 0.93), (3.2, 0.9)];
pub const VOLLEY_PASS_HIGH_BALL: [(f32, f32); 4] = [(0.0, 1.0), (0.348, 1.0), (0.69, 0.89), (1.0, 1.0)];
pub const VOLLEY_PASS_LIFTED_FROM: f32 = 0.69;
/// Degrees up by the charge (lifted past 69%), and down by the ball's height (m) so a high ball
/// isn't skied (Curve_VolleyTapPitchModifierDependingTime, BP_VolleyTap_AngleDependingBallHeight).
pub const VOLLEY_PASS_LIFT: [(f32, f32); 4] = [(0.0, 0.0), (0.69, 0.0), (0.691, 3.9), (1.0, 6.0)];
pub const VOLLEY_PASS_HEIGHT_PITCH: [(f32, f32); 3] = [(0.0, 0.0), (0.15, 0.0), (2.9, -8.0)];
/// The touch freezes 3 frames (ShootDataTable's VolleyTap); you can act again this long after it,
/// by body part (DT_VolleyTap): foot 10f, thigh 9, stomach 13, chest 15, head 16, jumping chest 26,
/// jumping head 31.
pub const VOLLEY_PASS_ACT: [f32; 7] = [10.0 / 60.0, 9.0 / 60.0, 13.0 / 60.0, 15.0 / 60.0, 16.0 / 60.0, 26.0 / 60.0, 31.0 / 60.0];
/// Through that recovery you slow from your approach to this share of it. TUNE.
pub const VOLLEY_RECOVERY: [(f32, f32); 2] = [(0.0, 1.0), (1.0, 0.3)];

// Freeplay ball call
pub const FEED_DISTANCE: f32 = 15.0; // TUNE
pub const FEED_SPEED: f32 = 16.0; // TUNE

// Camera (camera.md). Each state's geometry is in camera.rs.
/// Default's FOV: horizontal at 16:9 (the FOV option shifts every state from it).
pub const FOV: f32 = 80.0;
/// Pitch limits: 22 deg down, 15 deg up.
pub const CAMERA_PITCH_DOWN: f32 = 22.0;
pub const CAMERA_PITCH_UP: f32 = 15.0;
/// Mouse: degrees of yaw / pitch per look unit = scale x the state's turn speed x pi/180.
pub const CAMERA_MOUSE_YAW: f32 = 0.35;
pub const CAMERA_MOUSE_PITCH: f32 = 0.55;
/// Gamepad boost: held past 0.9 it ramps up over 0.2 s, and back down over 0.1 s.
pub const CAMERA_BOOST_THRESHOLD: f32 = 0.9;
pub const CAMERA_BOOST_UP: f32 = 0.2;
pub const CAMERA_BOOST_DOWN: f32 = 0.1;
/// The movement's sideways offset freezes when you stop, then eases back after 0.2 s at 0.5 /s.
pub const CAMERA_SIDE_IDLE: f32 = 0.2;
pub const CAMERA_SIDE_RETURN: f32 = 0.5;
/// Without the camera lock, a gamepad kick only lets the camera turn this far from the press:
/// (yaw, pitch) in degrees. The files have no such clamp (Kev says there is one); this is what the
/// shot camera's 120/60 deg/s allow over a full shot's 31-frame charge (camera-lock.md).
pub const CAMERA_KICK_LIMIT: (f32, f32) = (62.0, 31.0);
/// The crosshair (the camera's sight point) sits at this share of the screen's height.
pub const CROSSHAIR_SCREEN_Y: f32 = 0.38;
/// The FOV option's range (players report a 90 deg maximum; the minimum is a guess: TUNE).
pub const FOV_OPTION_RANGE: (f32, f32) = (70.0, 90.0);
/// The "Magnus sensitivity" option multiplies the mouse curve accumulator (default 1; the
/// range isn't in the game files read so far: TUNE).
pub const MAGNUS_SENSITIVITY_RANGE: (f32, f32) = (0.1, 2.0);
/// Gamepad: the curve stick ignores small tilts (InputShootRotationForceComputation's dead zone).
pub const CURVE_STICK_DEAD_ZONE: f32 = 0.2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shot_force_matches_game_curve() {
        assert_eq!(curve(&SHOT_FORCE, 0.0), 24.0);
        assert_eq!(curve(&SHOT_FORCE, 0.3), 24.0);
        assert!((curve(&SHOT_FORCE, 0.65) - 31.0).abs() < 1e-4);
        assert_eq!(curve(&SHOT_FORCE, 1.0), 38.0);
        assert_eq!(curve(&SHOT_FORCE, 2.0), 38.0);
    }

    #[test]
    fn pass_has_a_sweet_spot() {
        assert_eq!(curve(&PASS_FORCE, 0.6), 32.0);
        assert!(curve(&PASS_FORCE, 0.7) < 32.0, "charging past 60% makes the pass weaker");
    }

    #[test]
    fn full_power_comes_early() {
        use crate::player::{Kick, charge};
        // The gauge fills on the frame before the contact: shots at ~0.52 s held, passes and lobs
        // at ~0.38 s; the pass peaks at ~0.23 s.
        assert!(charge(Kick::Shot, 0.5) < 1.0 && charge(Kick::Shot, 31.0 / 60.0) >= 1.0 - 1e-3);
        assert!(charge(Kick::Pass, 0.37) > 0.96 && charge(Kick::Lob, 23.0 / 60.0) == 1.0);
        assert!((curve(&PASS_FORCE, charge(Kick::Pass, 0.23)) - 32.0).abs() < 0.6);
        // A tap stays on the flat start of the curves.
        assert_eq!(curve(&SHOT_FORCE, charge(Kick::Shot, 0.1)), 24.0);
    }

    #[test]
    fn stamina_speed_bonus() {
        assert_eq!(curve(&SPEED_BY_STAMINA, 10.0), 1.05);
        assert_eq!(curve(&SPEED_BY_STAMINA, 0.0), 0.8);
    }
}
