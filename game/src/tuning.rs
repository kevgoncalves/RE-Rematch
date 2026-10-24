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
/// Time to stop from full speed by state (the stop transitions): run, with ball, sprint, sprint with ball.
pub const STOP_TIME: [f32; 4] = [0.38, 0.7, 0.68, 1.0];

// Turn transitions (code-driven transition data, Data/Locomotion/TransitionDB): a big change of
// direction brakes along the old direction, then re-accelerates along the new one up to a share of
// top speed. Angles under 30 deg just steer; 30-60 counts as 45, 60-120 as 90, from 120 as 180.
/// One phase: duration (s) and speed curve over its normalised time (share of the entry or target speed).
pub type TurnPhase = (f32, &'static [(f32, f32)]);
const LIN_DOWN: &[(f32, f32)] = &[(0.0, 1.0), (1.0, 0.0)];
const LIN_UP: &[(f32, f32)] = &[(0.0, 0.0), (1.0, 1.0)];
const V2_TURN90_STOP: &[(f32, f32)] = &[(0.0, 1.0), (0.5, 0.25), (1.0, 0.0)];
const V2_TURN90_START: &[(f32, f32)] = &[(0.0, 0.0), (0.6, 0.6), (1.0, 1.0)];
const V3_TURN90_STOP: &[(f32, f32)] = &[(0.0, 1.0), (0.2, 0.7), (1.0, 0.0)];
const V3_TURN90_START: &[(f32, f32)] = &[(0.0, 0.0), (0.8, 0.8), (1.0, 1.0)];
const V3_TURN180_STOP: &[(f32, f32)] = &[(0.0, 1.0), (0.55, 0.65), (1.0, 0.0)];

/// (brake, re-accelerate, exit speed share) for a turn, by sprinting, ball and angle bucket
/// (1 = 45, 2 = 90, 3 = 135, 4 = 180); None means no transition, just steer.
pub fn turn_data(sprint: bool, ball: bool, bucket: usize) -> Option<(TurnPhase, TurnPhase, f32)> {
    match (sprint, ball, bucket) {
        (false, false, 2 | 3) => Some(((0.14, V2_TURN90_STOP), (0.13, V2_TURN90_START), 0.85)),
        (false, false, 4) => Some(((0.3, LIN_DOWN), (0.3, LIN_UP), 0.6)),
        (false, true, 1) => Some(((0.2, LIN_DOWN), (0.12, LIN_UP), 0.55)),
        (false, true, 2) => Some(((0.25, LIN_DOWN), (0.1, LIN_UP), 0.55)),
        (false, true, 3 | 4) => Some(((0.35, LIN_DOWN), (0.32, LIN_UP), 0.55)),
        (true, false, 2 | 3) => Some(((0.165, V3_TURN90_STOP), (0.35, V3_TURN90_START), 0.7)),
        (true, false, 4) => Some(((0.4, V3_TURN180_STOP), (0.35, LIN_UP), 0.5)),
        (true, true, 2) => Some(((0.32, LIN_DOWN), (0.25, LIN_UP), 0.7)),
        (true, true, 3) => Some(((0.3, LIN_DOWN), (0.2, LIN_UP), 0.6)),
        (true, true, 4) => Some(((0.5, LIN_DOWN), (0.3, LIN_UP), 0.45)),
        _ => None,
    }
}

/// Angle bucket for a change of direction (degrees): 0 steer, 1 = 45, 2 = 90, 4 = 180. The 135
/// range (112.5-157.5) is not allowed in the free-move tables and falls back to 180.
pub fn turn_bucket(angle: f32) -> usize {
    match angle {
        a if a < 30.0 => 0,
        a if a < 60.0 => 1,
        a if a < 120.0 => 2,
        _ => 4,
    }
}
pub const TURN_RATE_RUN: f32 = 400.0; // degrees per second
pub const TURN_RATE_BALL_RUN: f32 = 180.0; // with-ball speed table's max angular speed (pi rad/s)
pub const TURN_RATE_SPRINT: f32 = 180.0;
pub const TURN_RATE_BALL_SPRINT: f32 = 90.0;
/// Out of lock move the body turns to face where you're going this fast, without changing your heading (TUNE).
pub const FACE_HEADING_TURN_RATE: f32 = 720.0;
pub const LOCK_SPEED: f32 = 5.25;
pub const LOCK_BALL_SPEED: f32 = 4.0;
pub const LOCK_ACCEL: f32 = LOCK_SPEED / 0.2; // full speed in 0.2 s
pub const LOCK_BALL_ACCEL: f32 = LOCK_BALL_SPEED / 0.2;
/// Turning to face the camera in lock move. The speed table says 400 deg/s for outfielders (nothing
/// with the ball), but players find the turn to the front much quicker (TUNE).
pub const LOCK_TURN_RATE: f32 = 720.0;
/// Stance (lock move with the ball) lasts at least this long from the press (GA_DribbleStance).
pub const STANCE_MIN_TIME: f32 = 0.15;
/// Angle between facing and movement (degrees) to speed multiplier.
pub const LOCK_SPEED_BY_ANGLE: [(f32, f32); 3] = [(0.0, 1.0), (90.0, 1.0), (180.0, 0.9)];
pub const LOCK_BALL_SPEED_BY_ANGLE: [(f32, f32); 4] = [(0.0, 1.0), (90.0, 0.95), (135.0, 0.88), (180.0, 0.88)];
// Dash (mechanics.md §8): speed (m/s) over the dash's normalised time
pub const DASH_DURATION: f32 = 50.0 / 60.0;
pub const DASH_SPEED: [(f32, f32); 6] = [(0.0, 6.0), (0.1, 8.5), (0.233, 8.5), (0.331, 6.5), (0.5, 4.5), (1.0, 4.5)];
pub const SECOND_DASH_SPEED: [(f32, f32); 5] = [(0.0, 6.0), (0.1, 7.2), (0.3, 7.2), (0.7, 2.0), (1.0, 2.0)];
pub const DASH_COOLDOWN: f32 = 0.8;
pub const DASH_COST: f32 = 0.75;
/// The second dash can cut into the first from here, while the first one's cooldown runs.
pub const SECOND_DASH_FROM: f32 = 14.4 / 60.0;

// Dribble: the dash you do with the ball, from lock move (notes/dribble.md). The second one in a
// chain is the feint. Speeds (m/s) run over DRIBBLE_SPEED_TIME, then hold until the move ends.
pub const DRIBBLE_SPEED_TIME: f32 = 45.0 / 60.0;
/// From here the feint, dribble push balls, passes and shots are allowed.
pub const DRIBBLE_WINDOW_OPENS: f32 = 12.0 / 60.0;
pub const DRIBBLE_FORWARD: [(f32, f32); 4] = [(0.0, 4.0), (0.2, 6.8), (0.5, 4.0), (1.0, 4.0)];
pub const DRIBBLE_SIDE: [(f32, f32); 5] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.35, 3.4), (1.0, 3.4)];
pub const DRIBBLE_BACK: [(f32, f32); 5] = [(0.0, 3.0), (0.1, 5.8), (0.2, 5.8), (0.45, 3.0), (1.0, 3.0)];
pub const FEINT_FORWARD: [(f32, f32); 5] = [(0.0, 4.0), (0.1, 6.0), (0.3, 6.0), (0.6, 4.0), (1.0, 4.0)];
pub const FEINT_SIDE: [(f32, f32); 5] = [(0.0, 4.0), (0.2, 6.8), (0.3, 6.8), (0.5, 3.0), (1.0, 3.4)];
pub const FEINT_BACK: [(f32, f32); 5] = [(0.0, 3.0), (0.1, 5.8), (0.2, 5.8), (0.5, 2.5), (1.0, 3.0)];
pub const EXHAUSTED_DRIBBLE_FORWARD: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.35, 4.0), (0.8, 2.5), (1.0, 2.5)];
pub const EXHAUSTED_DRIBBLE_SIDE: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.35, 3.5), (0.8, 1.5), (1.0, 1.5)];
pub const EXHAUSTED_DRIBBLE_BACK: [(f32, f32); 6] = [(0.0, 4.0), (0.1, 6.5), (0.15, 6.5), (0.25, 3.5), (0.4, 2.5), (1.0, 2.5)];
/// How long each move lasts (its animation), as (forward, side, back).
pub const DRIBBLE_LENGTH: [f32; 3] = [54.0 / 60.0, 50.0 / 60.0, 64.0 / 60.0];
pub const FEINT_LENGTH: [f32; 3] = [65.0 / 60.0, 50.0 / 60.0, 53.0 / 60.0];
pub const EXHAUSTED_DRIBBLE_LENGTH: [f32; 3] = [65.0 / 60.0, 63.0 / 60.0, 67.0 / 60.0];
pub const DRIBBLE_COST: f32 = 0.5;
pub const DRIBBLE_REGEN_DELAY: f32 = 0.9;
/// No new dribble until this long after a feint starts (our reading of the game's 1 s tag time).
pub const FEINT_LOCKOUT: f32 = 12.0 / 60.0 + 1.0;
/// How fast a fast cut turns you (and the ball) to its direction, degrees per second.
pub const DRIBBLE_TURN_RATE: f32 = 1440.0;
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
// since the press against the kick's build-up (less a frame and the network margins), so full
// power comes early: ~0.37 s for shots, ~0.23 s for passes and lobs. The ball leaves on release,
// but never before the kick's contact frame; held to the end of the build-up it goes off by itself.
pub const SHOT_FORCE: [(f32, f32); 3] = [(0.0, 24.0), (0.3, 24.0), (1.0, 38.0)];
/// Pass peaks at 60% charge (~0.14 s held); holding longer drops it to 25.6 m/s before it climbs back to 30.65.
pub const PASS_FORCE: [(f32, f32); 5] = [(0.0, 20.0), (0.3, 20.0), (0.6, 32.0), (0.601, 25.6), (1.0, 30.65)];
pub const LOB_FORCE: [(f32, f32); 3] = [(0.0, 16.0), (0.3, 16.0), (1.0, 27.0)];
/// The force curves' last key; spin scales with the raw seconds held over this.
pub const MAX_HOLD: f32 = 1.0;
/// Taken off the build-up for the charge: the stadium's network margins (0.12 + 0.03 s, the world
/// settings' defaults), plus one frame.
pub const CHARGE_NETWORK_MARGIN: f32 = 0.15;
pub const SHOT_BUILDUP: f32 = 32.0 / 60.0;
pub const PASS_BUILDUP: f32 = 24.0 / 60.0;
pub const LOB_BUILDUP: f32 = 24.0 / 60.0;
pub const WINDUP_SPEED_SCALE: f32 = 0.5; // TUNE: how much you slow down winding up a pass or lob
/// A shot keeps your run-up: through the build-up you slide on in the same direction at a share
/// of your speed at the press that shrinks to this (HighPowerBeforeShootScaleSpeedData)...
pub const SHOT_SLIDE: f32 = 0.75;
/// ...sprinting at 4 m/s or more...
pub const SHOT_SLIDE_SPRINT: f32 = 0.6;
/// ...or shooting out of a dribble.
pub const SHOT_SLIDE_FROM_DRIBBLE: f32 = 0.5;
/// After the touch you keep that speed until the move ends (can act moving at 0.75 s, standing 0.67 s).
pub const SHOT_FOLLOW_THROUGH: f32 = 13.0 / 60.0;
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
/// Passes leave flat; charging past the 60% sweet spot lifts them (degrees by charge).
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

// Fixed-force kicks (StaticShootForceComputation in each ability's data), m/s
pub const SHORT_WINDUP: f32 = 16.0 / 60.0;
pub const SHORT_PASS_SPEED: f32 = 24.25;
pub const SHORT_PASS_FRICTION: f32 = BASE_FRICTION * 1.5 * 1.675; // multiplier x the override's weight
/// Lob launch angle by charge (overrides the aim): longer lobs are flatter and faster.
pub const LOB_ANGLE_BY_HOLD: [(f32, f32); 2] = [(0.3, 44.0), (1.0, 30.26)];
pub const SHORT_LOB_ANGLE: f32 = 52.0;
pub const SHORT_LOB_SPEED: f32 = 13.2;
pub const PUSH_BALL_SPEED: f32 = 11.5;
pub const PUSH_BALL_FRICTION: f32 = BASE_FRICTION * 2.8; // about 5.1 m of roll at 11.5 m/s
pub const PUSH_BALL_WINDUP: f32 = 8.0 / 60.0;
/// After any kick you can't control your own ball for 0.5 s (GE_LostBall).
pub const PUSH_BALL_NO_CONTROL: f32 = 0.5;
pub const PUSH_BALL_NO_CONTROL_EXTRA_EFFORT: f32 = 12.0 / 60.0;
pub const PUSH_BALL_COST: f32 = 0.375;
pub const PUSH_BALL_REGEN_DELAY: f32 = 1.0;
/// Sprinting switches the push ball's force up (BP_FastTap_ForceFromSpeed bytecode).
pub const PUSH_BALL_SPRINT_FACTOR: f32 = 1.4;
/// Ball speed by push direction against your facing: up to 60 deg, to 95 deg, beyond.
pub const PUSH_BALL_SIDE_FACTOR: f32 = 0.89;
pub const PUSH_BALL_BACK_FACTOR: f32 = 0.79;
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
// Push ball during a dribble: no direction falloff; x1.35 when sprinting.
pub const DRIBBLE_PUSH_BALL_SPEED: f32 = 11.0;
pub const DRIBBLE_PUSH_BALL_SPRINT_FACTOR: f32 = 1.35;
pub const DRIBBLE_PUSH_BALL_COST: f32 = 0.6;
/// You chase at a constant speed: normal, sprinting, exhausted.
pub const DRIBBLE_PUSH_BALL_RUN: [&[(f32, f32)]; 3] = [&[(0.0, 6.0), (1.0, 6.0)], &[(0.0, 7.8), (1.0, 7.8)], &[(0.0, 5.0), (1.0, 5.0)]];
pub const DRIBBLE_PUSH_BALL_LENGTH: [f32; 3] = [52.0 / 60.0, 47.0 / 60.0, 74.0 / 60.0];

// Extra effort (mechanics.md §10)
pub const MAX_EXTRA_EFFORT: f32 = 1.0;
pub const EXTRA_EFFORT_DRAIN: f32 = 0.5;
pub const EXTRA_EFFORT_REGEN: f32 = 0.0625;
/// With stamina full, the gauge refills 3x faster (ExtraEffortRegenMultiplier 3 at threshold 1.0; players confirm).
pub const EXTRA_EFFORT_REGEN_FULL_STAMINA: f32 = 0.1875;
pub const EXTRA_EFFORT_SPEED: f32 = 9.5;
pub const EXTRA_EFFORT_ACCEL: f32 = 100.0;
pub const TURN_RATE_EXTRA_EFFORT: f32 = 150.0;
/// Extra effort can't control the ball: starting it taps the ball on, and running into the
/// ball taps it on again. Both use the extra effort friction override.
pub const EXTRA_EFFORT_TAP_SPEED: f32 = 13.0;
pub const EXTRA_EFFORT_KNOCK_SPEED: f32 = 12.0;
pub const EXTRA_EFFORT_FRICTION: f32 = BASE_FRICTION * 1.5;
/// At extra effort speed you catch the ball again about half a second after each tap, so a
/// full gauge (one ~1.5 s hold) gives three cuts, as players report.
/// The ability's cost effect takes a quarter of the gauge at activation, but players see no
/// instant drop in the game, so the native code must apply it differently (open question).
/// Until that is read, activation is free and only the 0.5/s hold drain applies (~2 s full).
pub const EXTRA_EFFORT_START_COST: f32 = 0.0;
/// The auto tap (starting extra effort with the ball) is weaker in the middle of a big turn:
/// x0.8 in a 135 deg turn, x0.5 in a 180 deg one; x0.9 for a ball 20 cm or more off the ground.
pub const EXTRA_EFFORT_TAP_IN_135_TURN: f32 = 0.8;
pub const EXTRA_EFFORT_TAP_IN_180_TURN: f32 = 0.5;
pub const EXTRA_EFFORT_TAP_HIGH_BALL: (f32, f32) = (0.2, 0.9);
/// Ball centre distance at which the body touches it (capsule 0.35 m + ball).
pub const EXTRA_EFFORT_KNOCK_REACH: f32 = 0.35 + BALL_RADIUS + 0.05;

// Ball control (hitboxes.md)
// Ball control zones (the character's control hitboxes; m, centred at pitch level, `forward` ahead
// of you): standing up to 2.5 m/s, moving above it. The ball's own hit sphere is 0.12 m.
pub const CONTROL_SWITCH_SPEED: f32 = 2.5;
pub const BALL_HIT_RADIUS: f32 = 0.12;
/// (forward offset, radius, half height) of the capsules; the standing ground zone is a sphere.
pub const CONTROL_STANDING: (f32, f32, f32) = (0.10, 0.50, 1.85);
pub const CONTROL_STANDING_GROUND: (f32, f32) = (0.10, 0.83);
pub const CONTROL_MOVING: (f32, f32, f32) = (0.30, 0.90, 1.84);
/// Running forward you can also take a ball rolling away from you.
pub const CONTROL_MOVING_GROUND: (f32, f32, f32) = (-0.10, 0.60, 1.80);
/// Lock move without the ball catches from any direction.
pub const CONTROL_LOCK_MOVE: (f32, f32, f32) = (0.10, 0.675, 1.85);
/// The normal zones refuse a ball heading away from you: this close to "away" (degrees).
pub const CONTROL_AWAY_ANGLE: f32 = 75.0;
/// The ball becomes yours 12 frames into the control (it settles at your feet), and you can act
/// from the control's action frame (14 to 22 by animation; TUNE within that).
pub const CONTROL_SNAP: f32 = 12.0 / 60.0;
pub const CONTROL_ACT: f32 = 16.0 / 60.0;
pub const CONTROL_MAX_HEIGHT: f32 = 2.0;
pub const DRIBBLE_OFFSET: f32 = 0.7; // TUNE: ball distance in front while carrying
pub const NO_RECONTROL_TIME: f32 = 0.5; // GE_LostBall: no control of your own kick for 0.5 s

// Freeplay ball call
pub const FEED_DISTANCE: f32 = 15.0; // TUNE
pub const FEED_SPEED: f32 = 16.0; // TUNE

// Camera (match-camera-network.md)
pub const CAMERA_DISTANCE: f32 = 5.0;
pub const CAMERA_RIGHT: f32 = 0.6;
pub const CAMERA_HEIGHT: f32 = 2.0;
pub const FOV: f32 = 80.0;
/// The FOV option's range (players report a 90 deg maximum; the minimum is a guess: TUNE).
pub const FOV_OPTION_RANGE: (f32, f32) = (70.0, 90.0);
/// The "Magnus sensitivity" option multiplies the mouse curve accumulator (default 1; the
/// range isn't in the game files read so far: TUNE).
pub const MAGNUS_SENSITIVITY_RANGE: (f32, f32) = (0.1, 2.0);
/// Gamepad: the curve stick ignores small tilts (InputShootRotationForceComputation's dead zone).
pub const CURVE_STICK_DEAD_ZONE: f32 = 0.2;
pub const SPRINT_FOV: f32 = 85.0;
pub const PITCH_UP_LIMIT: f32 = 22.0;
pub const PITCH_DOWN_LIMIT: f32 = 15.0;
pub const MOUSE_SENSITIVITY: f32 = 0.12; // degrees per pixel: TUNE

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
        // Shots reach full power at ~0.37 s held, passes and lobs at ~0.23 s; the pass peaks at ~0.14 s.
        assert!(charge(Kick::Shot, 0.36) < 1.0 && charge(Kick::Shot, 0.37) >= 1.0 - 1e-3);
        assert!(charge(Kick::Pass, 0.23) > 0.98 && charge(Kick::Lob, 0.24) == 1.0);
        assert!((curve(&PASS_FORCE, charge(Kick::Pass, 0.14)) - 32.0).abs() < 0.6);
        // A tap stays on the flat start of the curves.
        assert_eq!(curve(&SHOT_FORCE, charge(Kick::Shot, 0.1)), 24.0);
    }

    #[test]
    fn stamina_speed_bonus() {
        assert_eq!(curve(&SPEED_BY_STAMINA, 10.0), 1.05);
        assert_eq!(curve(&SPEED_BY_STAMINA, 0.0), 0.8);
    }
}
