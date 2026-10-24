//! Ball flight and the arena it bounces around in. Rematch builds the trajectory from
//! gravity + drag + Magnus + ground friction (mechanics.md §11); this does the same.

use bevy::prelude::*;

use crate::tuning::*;

#[derive(Component)]
pub struct Ball {
    pub vel: Vec3,
    pub spin: Vec3,
    /// Ground-friction multiplier set by the last touch (pass 0.3, shot 1.0, ...).
    pub friction: f32,
    /// Carried by a player: physics is off and the player positions it.
    pub held: bool,
    /// Seconds before anyone may control the ball again after a kick.
    pub no_control: f32,
}

impl Default for Ball {
    fn default() -> Self {
        Self { vel: Vec3::ZERO, spin: Vec3::ZERO, friction: SHOT_FRICTION, held: false, no_control: 0.0 }
    }
}

/// Which end the ball went in: +1 for the +z goal, -1 for the -z goal.
#[derive(Event)]
pub struct GoalScored(pub f32);

pub fn ball_physics(mut balls: Query<(&mut Transform, &mut Ball)>, time: Res<Time>, mut goals: EventWriter<GoalScored>) {
    let dt = time.delta_secs().min(0.05);
    // ponytail: fixed 240 Hz substeps, swept collision if tunnelling ever shows up at 70 m/s
    let steps = (dt * 240.0).ceil().max(1.0) as usize;
    for (mut tf, mut ball) in &mut balls {
        ball.no_control = (ball.no_control - dt).max(0.0);
        if ball.held {
            continue;
        }
        for _ in 0..steps {
            if let Some(end) = step(&mut tf.translation, &mut ball, dt / steps as f32) {
                goals.write(GoalScored(end));
                break;
            }
        }
        let axis = Vec3::Y.cross(ball.vel).normalize_or_zero();
        tf.rotate(Quat::from_axis_angle(axis, ball.vel.length() / BALL_RADIUS * dt));
    }
}

/// One integration step. Returns the goal end if the ball crossed a goal line.
pub fn step(pos: &mut Vec3, ball: &mut Ball, dt: f32) -> Option<f32> {
    let on_ground = pos.y <= BALL_RADIUS + 0.001 && ball.vel.y.abs() < 0.5;
    // Spin around the vertical axis curves the ball sideways; spin around a horizontal axis lifts or dips it.
    let side_spin = Vec3::Y * ball.spin.y;
    let magnus = MAGNUS_SIDE * side_spin.cross(ball.vel) + MAGNUS_VERTICAL * (ball.spin - side_spin).cross(ball.vel);
    let mut acc = Vec3::NEG_Y * GRAVITY - DRAG * ball.vel.length() * ball.vel + magnus;
    if on_ground {
        acc.y = acc.y.max(0.0);
        let flat = Vec3::new(ball.vel.x, 0.0, ball.vel.z);
        let decel = ROLL_DECEL * ball.friction * dt;
        let speed = flat.length();
        let new_flat = if speed > decel { flat * (1.0 - decel / speed) } else { Vec3::ZERO };
        ball.vel.x = new_flat.x;
        ball.vel.z = new_flat.z;
    }
    ball.vel += acc * dt;
    ball.vel = ball.vel.clamp_length_max(BALL_MAX_SPEED);
    ball.spin *= 1.0 - SPIN_DECAY * dt;
    *pos += ball.vel * dt;
    collide(pos, &mut ball.vel);

    let past_line = pos.z.abs() - HALF_LENGTH;
    (past_line > GOAL_SCORE_DEPTH).then(|| pos.z.signum())
}

fn bounce(vel: &mut Vec3, normal: Vec3, (perp, para): (f32, f32)) {
    let into = vel.dot(normal);
    if into < 0.0 {
        let normal_part = normal * into;
        *vel = (*vel - normal_part) * para - normal_part * perp;
    }
}

fn collide(pos: &mut Vec3, vel: &mut Vec3) {
    let r = BALL_RADIUS;
    if pos.y < r {
        pos.y = r;
        bounce(vel, Vec3::Y, GRASS_BOUNCE);
        if vel.y < 0.5 {
            vel.y = 0.0; // settle into rolling instead of micro-bouncing
        }
    }
    if pos.y > CEILING - r {
        pos.y = CEILING - r;
        bounce(vel, Vec3::NEG_Y, WALL_BOUNCE);
    }
    if pos.x.abs() > HALF_WIDTH - r {
        pos.x = pos.x.signum() * (HALF_WIDTH - r);
        bounce(vel, Vec3::new(-pos.x.signum(), 0.0, 0.0), WALL_BOUNCE);
    }

    let in_mouth = pos.x.abs() < GOAL_HALF_WIDTH - r && pos.y < GOAL_HEIGHT - r;
    if pos.z.abs() > HALF_LENGTH - r {
        if in_mouth || pos.z.abs() > HALF_LENGTH {
            // Inside the goal: the net is a box behind the line.
            let back = HALF_LENGTH + GOAL_DEPTH - r;
            if pos.z.abs() > back {
                pos.z = pos.z.signum() * back;
                bounce(vel, Vec3::new(0.0, 0.0, -pos.z.signum()), NET_BOUNCE);
            }
            if pos.x.abs() > GOAL_HALF_WIDTH - r {
                pos.x = pos.x.signum() * (GOAL_HALF_WIDTH - r);
                bounce(vel, Vec3::new(-pos.x.signum(), 0.0, 0.0), NET_BOUNCE);
            }
            if pos.y > GOAL_HEIGHT - r {
                pos.y = GOAL_HEIGHT - r;
                bounce(vel, Vec3::NEG_Y, NET_BOUNCE);
            }
        } else {
            pos.z = pos.z.signum() * (HALF_LENGTH - r);
            bounce(vel, Vec3::new(0.0, 0.0, -pos.z.signum()), WALL_BOUNCE);
        }
    }

    // Posts and crossbar: segments with a small radius.
    for end in [-1.0f32, 1.0] {
        let z = end * HALF_LENGTH;
        let bar = (Vec3::new(-GOAL_HALF_WIDTH, GOAL_HEIGHT, z), Vec3::new(GOAL_HALF_WIDTH, GOAL_HEIGHT, z));
        for side in [-1.0f32, 1.0] {
            let post = (Vec3::new(side * GOAL_HALF_WIDTH, 0.0, z), Vec3::new(side * GOAL_HALF_WIDTH, GOAL_HEIGHT, z));
            hit_segment(pos, vel, post);
        }
        hit_segment(pos, vel, bar);
    }
}

fn hit_segment(pos: &mut Vec3, vel: &mut Vec3, (a, b): (Vec3, Vec3)) {
    let ab = b - a;
    let t = ((*pos - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    let offset = *pos - (a + ab * t);
    let min = BALL_RADIUS + POST_RADIUS;
    if offset.length_squared() < min * min {
        let normal = offset.normalize_or(Vec3::Y);
        *pos = a + ab * t + normal * min;
        bounce(vel, normal, (POST_BOUNCE, POST_BOUNCE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pos: &mut Vec3, ball: &mut Ball, seconds: f32) -> Option<f32> {
        for _ in 0..(seconds * 240.0) as usize {
            if let Some(end) = step(pos, ball, 1.0 / 240.0) {
                return Some(end);
            }
        }
        None
    }

    #[test]
    fn dropped_ball_loses_height_on_grass() {
        let mut pos = Vec3::new(0.0, 5.0, 0.0);
        let mut ball = Ball::default();
        run(&mut pos, &mut ball, 1.6); // fall, bounce once
        let mut peak = 0.0f32;
        for _ in 0..480 {
            step(&mut pos, &mut ball, 1.0 / 240.0);
            peak = peak.max(pos.y);
        }
        assert!(peak < 2.0, "grass keeps 47.5% of vertical speed, peak was {peak}");
    }

    #[test]
    fn shot_into_mouth_scores_and_wide_shot_rebounds() {
        let mut pos = Vec3::new(0.0, 1.0, 30.0);
        let mut ball = Ball { vel: Vec3::new(0.0, 0.0, 30.0), ..default() };
        assert_eq!(run(&mut pos, &mut ball, 1.0), Some(1.0));

        let mut pos = Vec3::new(10.0, 1.0, 30.0);
        let mut ball = Ball { vel: Vec3::new(0.0, 0.0, 30.0), ..default() };
        assert_eq!(run(&mut pos, &mut ball, 0.5), None);
        assert!(ball.vel.z < 0.0, "the end wall sends it back");
    }
}
