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
pub const MAGNUS: f32 = 0.0003; // spin to curve strength: TUNE
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

// Shooting and passing (mechanics.md §4). Charge is 0..1.
pub const SHOT_FORCE: [(f32, f32); 3] = [(0.0, 24.0), (0.3, 24.0), (1.0, 38.0)];
pub const PASS_FORCE: [(f32, f32); 3] = [(0.0, 16.5), (0.3, 16.5), (1.0, 30.4)];
pub const FULL_CHARGE_TIME: f32 = 1.0; // TUNE
pub const POWER_SHOT_SPEED: f32 = 34.82; // above this the power-shot wind-up is used
pub const POWER_SHOT_WINDUP: f32 = 32.0 / 60.0;
pub const LOW_SHOT_WINDUP: f32 = 20.0 / 60.0;
pub const PASS_WINDUP: f32 = 24.0 / 60.0;
pub const WINDUP_SPEED_SCALE: f32 = 0.5; // TUNE: how much you slow down while winding up
/// Camera pitch (degrees, + is up) to launch elevation (degrees).
pub const ELEVATION_BY_PITCH: [(f32, f32); 6] =
    [(-24.9, 2.93), (-14.6, 2.93), (-13.2, 3.99), (-7.6, 9.29), (0.7, 17.42), (4.0, 20.46)];
pub const PASS_ELEVATION: f32 = 2.0; // TUNE: passes stay low
pub const AIM_NUDGE: f32 = 3.0; // degrees of side nudge from the stick
pub const SIDE_SPIN: [(f32, f32); 5] = [(-1.0, -825.0), (-0.5, -350.0), (0.0, 0.0), (0.5, 350.0), (1.0, 825.0)];
pub const SIDE_SPIN_BY_CHARGE: [(f32, f32); 2] = [(0.1, 0.5), (1.0, 0.9)];
pub const PASS_FRICTION: f32 = 0.3;
pub const SHOT_FRICTION: f32 = 1.0;

// Ball control (hitboxes.md)
pub const CONTROL_RADIUS_STANDING: f32 = 0.5;
pub const CONTROL_RADIUS_MOVING: f32 = 0.9;
pub const CONTROL_MAX_HEIGHT: f32 = 2.0;
pub const MOVING_THRESHOLD: f32 = 3.0; // "standing" is below 3 m/s
pub const DRIBBLE_OFFSET: f32 = 0.7; // TUNE: ball distance in front while carrying
pub const NO_RECONTROL_TIME: f32 = 0.3; // TUNE: after kicking, you can't instantly re-control

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
    fn stamina_speed_bonus() {
        assert_eq!(curve(&SPEED_BY_STAMINA, 10.0), 1.05);
        assert_eq!(curve(&SPEED_BY_STAMINA, 0.0), 0.8);
    }
}
