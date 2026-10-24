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

// Arena: 73 x 49 pitch (arena.md). Bevy axes: x = width, y = up, z = goal-to-goal.
pub const HALF_WIDTH: f32 = 24.5;
pub const HALF_LENGTH: f32 = 36.5;
pub const CEILING: f32 = 20.5;
pub const GOAL_HALF_WIDTH: f32 = 4.1;
pub const GOAL_HEIGHT: f32 = 2.77;
pub const GOAL_DEPTH: f32 = 3.8;
pub const GOAL_SCORE_DEPTH: f32 = 0.4; // ball centre this far past the line scores (unconfirmed)
pub const POST_RADIUS: f32 = 0.08;

// Ball (mechanics.md §11)
pub const BALL_RADIUS: f32 = 0.11;
pub const BALL_MAX_SPEED: f32 = 70.0;
pub const GRAVITY: f32 = 9.81; // game: FreeFallMultiplier 0.5 on an unknown base force: TUNE
pub const DRAG: f32 = 0.004; // quadratic air drag: TUNE (game: DragForce 0.45, modifier 2.54)
pub const ROLL_DECEL: f32 = 0.8 * 2.5; // base friction 0.8 x TUNE scale, m/s², times the touch multiplier
pub const MAGNUS_SIDE: f32 = 0.0006; // side-spin to curve strength: TUNE
pub const MAGNUS_VERTICAL: f32 = 0.0012; // top/back-spin to dip/lift strength: TUNE
pub const SPIN_DECAY: f32 = 0.6; // per second: TUNE
pub const GRASS_BOUNCE: (f32, f32) = (0.475, 0.95); // (perpendicular, parallel)
pub const WALL_BOUNCE: (f32, f32) = (0.675, 0.675);
pub const NET_BOUNCE: (f32, f32) = (0.08, 0.15);
pub const POST_BOUNCE: f32 = 0.875;

// Movement (mechanics.md §2). Accelerations come from the time to reach full speed.
pub const RUN_SPEED: f32 = 5.0;
pub const SPRINT_SPEED: f32 = 8.75;
pub const BALL_RUN_SPEED: f32 = 4.5;
pub const BALL_SPRINT_SPEED: f32 = 6.5;
pub const RUN_ACCEL: f32 = 5.0 / 0.22;
pub const SPRINT_ACCEL: f32 = 8.75 / 0.6;
pub const BALL_RUN_ACCEL: f32 = 4.5 / 0.6;
pub const BALL_SPRINT_ACCEL: f32 = 6.5 / 0.8;
pub const STOP_DECEL: f32 = 5.0 / 0.38;
pub const TURN_RATE_RUN: f32 = 400.0; // degrees per second
pub const TURN_RATE_BALL_RUN: f32 = 360.0;
pub const TURN_RATE_SPRINT: f32 = 180.0;
pub const TURN_RATE_BALL_SPRINT: f32 = 90.0;
pub const LOCK_SPEED: f32 = 4.0;
pub const LOCK_BALL_SPEED: f32 = 3.5;
pub const LOCK_ACCEL: f32 = 16.0;
pub const LOCK_BALL_ACCEL: f32 = 13.0;
pub const LOCK_TURN_RATE: f32 = 400.0;
/// Angle between facing and movement (degrees) to speed multiplier.
pub const LOCK_SPEED_BY_ANGLE: [(f32, f32); 3] = [(0.0, 1.0), (90.0, 1.0), (180.0, 0.9)];
pub const LOCK_BALL_SPEED_BY_ANGLE: [(f32, f32); 4] = [(0.0, 1.0), (90.0, 0.95), (135.0, 0.88), (180.0, 0.88)];
// Dash (mechanics.md §8): speed (m/s) over the dash's normalised time
pub const DASH_DURATION: f32 = 50.0 / 60.0;
pub const DASH_SPEED: [(f32, f32); 6] = [(0.0, 6.0), (0.1, 8.5), (0.233, 8.5), (0.331, 6.5), (0.5, 4.5), (1.0, 4.5)];
pub const SECOND_DASH_SPEED: [(f32, f32); 5] = [(0.0, 6.0), (0.1, 7.2), (0.3, 7.2), (0.7, 2.0), (1.0, 2.0)];
pub const DASH_COOLDOWN: f32 = 0.8;
pub const DASH_COST: f32 = 0.75;
pub const SECOND_DASH_WINDOW: f32 = 0.3; // TUNE: how long after a dash the weaker second dash is offered
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

// Shooting and passing (mechanics.md §4). Force curves are by seconds held since the press
// ("ShootForceByTimeHeld"). The build-up runs while you hold: the ball leaves on release, but
// never before the build-up ends, and the force uses at least the build-up time.
pub const SHOT_FORCE: [(f32, f32); 3] = [(0.0, 24.0), (0.3, 24.0), (1.0, 38.0)];
/// Pass peaks at 0.6 s; holding longer drops it to 25.6 m/s before it climbs back to 30.65.
pub const PASS_FORCE: [(f32, f32); 5] = [(0.0, 20.0), (0.3, 20.0), (0.6, 32.0), (0.601, 25.6), (1.0, 30.65)];
pub const LOB_FORCE: [(f32, f32); 3] = [(0.0, 16.0), (0.3, 16.0), (1.0, 27.0)];
pub const MAX_HOLD: f32 = 1.0; // last key of the force curves
pub const SHOT_BUILDUP: f32 = 32.0 / 60.0;
pub const PASS_BUILDUP: f32 = 24.0 / 60.0;
pub const LOB_BUILDUP: f32 = 24.0 / 60.0;
pub const WINDUP_SPEED_SCALE: f32 = 0.5; // TUNE: how much you slow down while winding up
/// Camera pitch (degrees, + is up) to launch elevation (degrees).
pub const ELEVATION_BY_PITCH: [(f32, f32); 6] =
    [(-24.9, 2.93), (-14.6, 2.93), (-13.2, 3.99), (-7.6, 9.29), (0.7, 17.42), (4.0, 20.46)];
pub const PASS_ELEVATION: f32 = 2.0; // TUNE: passes stay low
pub const AIM_NUDGE: f32 = 3.0; // degrees of side nudge from the curve input
pub const SIDE_SPIN: [(f32, f32); 5] = [(-1.0, -825.0), (-0.5, -350.0), (0.0, 0.0), (0.5, 350.0), (1.0, 825.0)];
pub const SIDE_SPIN_BY_CHARGE: [(f32, f32); 2] = [(0.1, 0.5), (1.0, 0.9)];
// Keyboard and mouse: the shot direction locks when you start charging, and mouse movement
// while charging feeds a Magnus accumulator instead of the camera (BP_MK_InputRotationForceComputation).
pub const MAGNUS_ACCUMULATOR_SENSITIVITY: f32 = 0.2;
pub const MOUSE_AXIS_SCALE: f32 = 0.07; // Unreal's default MouseX/MouseY axis scale (unconfirmed for Rematch)
pub const PASS_FRICTION: f32 = 0.3;
// Pass aiming (Tap_OrientationDataTable_*, BP_*_Tap_FilteredInputDirectionComputation)
pub const PASS_STICK_DEAD_ZONE: f32 = 0.3;
pub const PASS_MOUSE_DEAD_ZONE: f32 = 0.4;
pub const PASS_FORWARD_SNAP: f32 = 6.0; // degrees
pub const SHOT_FRICTION: f32 = 1.0;
/// Stick/mouse forward-back to top/back spin; one direction is much stronger (which one is unconfirmed).
pub const TOP_SPIN: [(f32, f32); 6] = [(-1.0, 780.0), (-0.5, 330.0), (-0.1, 0.0), (0.2, 0.0), (0.5, -40.0), (1.0, -120.0)];
pub const TOP_SPIN_BY_CHARGE: [(f32, f32); 2] = [(0.1, 0.35), (1.0, 0.5)];
pub const SHOT_BASE_SPIN: f32 = -210.0; // "forced Magnus" on every shot; sign (dip vs rise) unconfirmed
pub const FAKE_SHOT_COST: f32 = 1.5;

// Fixed-force kicks (StaticShootForceComputation in each ability's data), m/s
pub const SHORT_WINDUP: f32 = 16.0 / 60.0;
pub const SHORT_PASS_SPEED: f32 = 24.25;
pub const SHORT_PASS_FRICTION: f32 = 1.5;
pub const LOB_ELEVATION: f32 = 35.0; // TUNE
pub const SHORT_LOB_SPEED: f32 = 13.2;
pub const PUSH_BALL_SPEED: f32 = 11.5;
pub const PUSH_BALL_FRICTION: f32 = 2.8;
pub const PUSH_BALL_WINDUP: f32 = 8.0 / 60.0;
pub const PUSH_BALL_NO_CONTROL: f32 = 23.0 / 60.0;
pub const PUSH_BALL_NO_CONTROL_EXTRA_EFFORT: f32 = 12.0 / 60.0;
pub const PUSH_BALL_COST: f32 = 0.375;
pub const PUSH_BALL_REGEN_DELAY: f32 = 1.0;

// Extra effort (mechanics.md §10)
pub const MAX_EXTRA_EFFORT: f32 = 1.0;
pub const EXTRA_EFFORT_DRAIN: f32 = 0.5;
pub const EXTRA_EFFORT_REGEN: f32 = 0.0625;
pub const EXTRA_EFFORT_SPEED: f32 = 8.75;
pub const EXTRA_EFFORT_ACCEL: f32 = 100.0;
pub const TURN_RATE_EXTRA_EFFORT: f32 = 150.0;

// Ball control (hitboxes.md)
pub const CONTROL_RADIUS_STANDING: f32 = 0.5;
pub const CONTROL_RADIUS_MOVING: f32 = 0.9;
pub const CONTROL_MAX_HEIGHT: f32 = 2.0;
pub const MOVING_THRESHOLD: f32 = 3.0; // "standing" is below 3 m/s
pub const DRIBBLE_OFFSET: f32 = 0.7; // TUNE: ball distance in front while carrying
pub const NO_RECONTROL_TIME: f32 = 0.3; // TUNE: after kicking, you can't instantly re-control

// Freeplay ball call
pub const FEED_DISTANCE: f32 = 15.0; // TUNE
pub const FEED_SPEED: f32 = 16.0; // TUNE

// Camera (match-camera-network.md)
pub const CAMERA_DISTANCE: f32 = 5.0;
pub const CAMERA_RIGHT: f32 = 0.6;
pub const CAMERA_HEIGHT: f32 = 2.0;
pub const FOV: f32 = 80.0;
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
        assert!(curve(&PASS_FORCE, 0.7) < 32.0, "holding past 0.6 s makes the pass weaker");
    }

    #[test]
    fn stamina_speed_bonus() {
        assert_eq!(curve(&SPEED_BY_STAMINA, 10.0), 1.05);
        assert_eq!(curve(&SPEED_BY_STAMINA, 0.0), 0.8);
    }
}
