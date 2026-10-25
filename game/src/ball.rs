//! Ball flight and the arena it bounces around in. Rematch builds the trajectory from
//! gravity + drag + Magnus + ground friction (mechanics.md §11), each an acceleration from the
//! game's native trajectory modifiers; this does the same.

use bevy::prelude::*;

use crate::tuning::*;

#[derive(Component, Clone)]
pub struct Ball {
    pub vel: Vec3,
    /// Spin in the flight frame (x along the path, y to its right, z up from both): z curves the
    /// ball right, y dips it.
    pub spin: Vec3,
    /// Rolling friction coefficient set by the last touch (shot 0.8, pass 0.96, push ball 2.24, ...).
    pub friction: f32,
    /// Air drag coefficient set by the last touch.
    pub drag: f32,
    /// Seconds spent falling since the last bounce or touch: gravity grows with it.
    pub fall_time: f32,
    /// Carried by a player: physics is off and the player positions it.
    pub held: bool,
    /// Seconds before anyone may control the ball again after a kick.
    pub no_control: f32,
    /// The kick's freeze: the ball waits at the foot this long before it flies.
    pub freeze: f32,
    /// The last step's hit on a wall, post or crossbar: the surface point and its normal.
    pub hit: Option<(Vec3, Vec3)>,
}

impl Default for Ball {
    fn default() -> Self {
        Self { vel: Vec3::ZERO, spin: Vec3::ZERO, friction: SHOT_FRICTION, drag: DRAG_FORCE, fall_time: 0.0, held: false, no_control: 0.0, freeze: 0.0, hit: None }
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
        if ball.freeze > 0.0 {
            ball.freeze -= dt;
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
    let speed = ball.vel.length();
    // The spin's frame follows the path: forward, right (level) and up from both.
    let fwd = ball.vel.normalize_or_zero();
    let right = fwd.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(fwd);
    let dip = if on_ground { fwd } else { up };
    let mut acc = MAGNUS_K * speed * (ball.spin.z * right - ball.spin.y * dip);
    ball.spin *= 1.0 - SPIN_DECAY * dt;
    if on_ground {
        ball.fall_time = 0.0;
        acc -= ball.friction * Vec3::new(ball.vel.x, 0.0, ball.vel.z);
        acc.y = acc.y.max(0.0);
    } else {
        if ball.vel.y <= 0.0 {
            ball.fall_time += dt;
        }
        acc.y -= GRAVITY * (BALL_MASS + FREE_FALL_GROWTH * ball.fall_time);
        acc -= ball.drag * DRAG_K * speed * (1.0 - fwd.y.abs()) * ball.vel;
    }
    ball.vel += acc * dt;
    ball.vel = ball.vel.clamp_length_max(BALL_MAX_SPEED);
    if on_ground && Vec2::new(ball.vel.x, ball.vel.z).length() < ROLL_STOP_SPEED {
        ball.vel = Vec3::ZERO;
    }
    let from = *pos;
    *pos += ball.vel * dt;
    let falling = ball.vel.y < 0.0;
    ball.hit = collide(from, pos, &mut ball.vel).map(|(at, normal)| (at - normal * BALL_RADIUS, normal));
    if falling && ball.vel.y >= 0.0 {
        ball.fall_time = 0.0; // a bounce starts a new fall
    }

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

/// Keeps the ball in the arena; for a wall, post or crossbar it hit (not the ground, the ceiling or
/// the net) returns where its centre touched and the surface's normal. Those are found where the
/// move from `from` first touches them, and the rest of the move goes on bounced, so the bounce
/// doesn't depend on the step's length: the marker predicts with its own steps.
fn collide(from: Vec3, pos: &mut Vec3, vel: &mut Vec3) -> Option<(Vec3, Vec3)> {
    let r = BALL_RADIUS;
    let mut hit = None;
    if pos.y < r {
        pos.y = r;
        bounce(vel, Vec3::Y, GRASS_BOUNCE);
        if vel.y < 0.5 {
            vel.y = 0.0; // settle into rolling instead of micro-bouncing
        }
    }
    if pos.y > CEILING - r {
        pos.y = CEILING - r;
        bounce(vel, Vec3::NEG_Y, CEILING_BOUNCE);
    }
    if pos.x.abs() > HALF_WIDTH - r {
        let normal = Vec3::new(-pos.x.signum(), 0.0, 0.0);
        hit = Some(hit_plane(from, pos, vel, normal, HALF_WIDTH - r, WALL_BOUNCE));
    }

    // Posts and crossbar: segments with a small radius. Their faces stand out in front of the end
    // wall, so they come first: a ball at the post meets it before the wall beside it.
    for end in [-1.0f32, 1.0] {
        let z = end * HALF_LENGTH;
        let bar = (Vec3::new(-GOAL_HALF_WIDTH, GOAL_HEIGHT, z), Vec3::new(GOAL_HALF_WIDTH, GOAL_HEIGHT, z));
        for side in [-1.0f32, 1.0] {
            let post = (Vec3::new(side * GOAL_HALF_WIDTH, 0.0, z), Vec3::new(side * GOAL_HALF_WIDTH, GOAL_HEIGHT, z));
            hit = hit_segment(from, pos, vel, post).or(hit);
        }
        hit = hit_segment(from, pos, vel, bar).or(hit);
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
            // Roof: flat at the side nets' height, then sloping 45 degrees down to the back.
            let depth = pos.z.abs() - HALF_LENGTH;
            if depth < NET_ROOF_FLAT {
                if pos.y > NET_HEIGHT - r {
                    pos.y = NET_HEIGHT - r;
                    bounce(vel, Vec3::NEG_Y, NET_BOUNCE);
                }
            } else {
                let normal = Vec3::new(0.0, -1.0, -pos.z.signum()).normalize();
                let gap = (NET_HEIGHT - (depth - NET_ROOF_FLAT) - pos.y) / std::f32::consts::SQRT_2;
                if gap < r {
                    *pos += normal * (r - gap);
                    bounce(vel, normal, NET_BOUNCE);
                }
            }
        } else {
            let normal = Vec3::new(0.0, 0.0, -pos.z.signum());
            hit = Some(hit_plane(from, pos, vel, normal, HALF_LENGTH - r, END_WALL_BOUNCE));
        }
    }

    hit
}

/// A wall (`normal` pointing in, the centre kept within `limit` of the middle along it): the ball
/// touches where its move crosses the limit, and the rest of the move goes on bounced.
fn hit_plane(from: Vec3, pos: &mut Vec3, vel: &mut Vec3, normal: Vec3, limit: f32, coefs: (f32, f32)) -> (Vec3, Vec3) {
    let (out_from, out_to) = (-from.dot(normal) - limit, -pos.dot(normal) - limit);
    let s = if out_from < 0.0 { (-out_from / (out_to - out_from)).clamp(0.0, 1.0) } else { 0.0 };
    let mut at = from + (*pos - from) * s;
    at -= normal * (at.dot(normal) + limit); // exactly on the limit
    let mut rest = *pos - at;
    bounce(vel, normal, coefs);
    bounce(&mut rest, normal, coefs);
    *pos = at + rest;
    (at, normal)
}

/// A post or the crossbar (a segment of POST_RADIUS): found where the ball's move first touches it
/// (sampled, then halved down to a millimetre), even a move that would pass through it in one step.
fn hit_segment(from: Vec3, pos: &mut Vec3, vel: &mut Vec3, (a, b): (Vec3, Vec3)) -> Option<(Vec3, Vec3)> {
    let min = BALL_RADIUS + POST_RADIUS;
    let ab = b - a;
    let offset = |p: Vec3| p - (a + ab * ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0));
    let touching = |p: Vec3| offset(p).length_squared() < min * min;
    let along = |s: f32| from + (*pos - from) * s;
    // Already touching at the start (resting on it): pushed out where it is.
    let s = if touching(from) {
        if !touching(*pos) {
            return None;
        }
        1.0
    } else {
        const SAMPLES: usize = 8;
        let hit = (1..=SAMPLES).map(|i| i as f32 / SAMPLES as f32).find(|s| touching(along(*s)))?;
        let (mut lo, mut hi) = (hit - 1.0 / SAMPLES as f32, hit);
        while (hi - lo) * (*pos - from).length() > 0.001 {
            let mid = (lo + hi) / 2.0;
            if touching(along(mid)) { hi = mid } else { lo = mid }
        }
        lo
    };
    let p = along(s);
    let normal = offset(p).normalize_or(Vec3::Y);
    let at = p - offset(p) + normal * min;
    let mut rest = *pos - p;
    bounce(vel, normal, (POST_BOUNCE, POST_BOUNCE));
    bounce(&mut rest, normal, (POST_BOUNCE, POST_BOUNCE));
    *pos = at + rest;
    Some((at, normal))
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
    fn a_post_hit_does_not_depend_on_the_step() {
        // A shot at the right post of the -z goal from 12 m out, flat and a little across.
        let shoot = |hz: f32| {
            let mut pos = Vec3::new(GOAL_HALF_WIDTH - 1.0, 1.0, -HALF_LENGTH + 12.0);
            let target = Vec3::new(GOAL_HALF_WIDTH - 0.05, 1.0, -HALF_LENGTH);
            let mut ball = Ball { vel: (target - pos).normalize() * 30.0, drag: 0.0, ..default() };
            for _ in 0..(hz as usize) {
                step(&mut pos, &mut ball, 1.0 / hz);
                if let Some(hit) = ball.hit {
                    return (hit, ball.vel);
                }
            }
            panic!("no post hit at {hz} Hz");
        };
        let ((at, normal), vel) = shoot(240.0);
        for hz in [144.0, 300.0, 1000.0] {
            let ((at2, normal2), vel2) = shoot(hz);
            assert!(at.distance(at2) < 0.01, "{hz} Hz hits at {at2}, 240 Hz at {at}");
            assert!(normal.angle_between(normal2) < 0.01 && vel.angle_between(vel2) < 0.02, "{hz} Hz bounces {vel2}, 240 Hz {vel}");
        }
    }

    #[test]
    fn rolls_speed_over_friction_metres() {
        let start = Vec3::new(0.0, BALL_RADIUS, 0.0);
        let mut pos = start;
        let mut ball = Ball { vel: Vec3::new(0.0, 0.0, PUSH_BALL_SPEED), friction: PUSH_BALL_FRICTION, ..default() };
        run(&mut pos, &mut ball, 5.0);
        let d = pos.distance(start);
        assert!((d - PUSH_BALL_SPEED / PUSH_BALL_FRICTION).abs() < 0.1, "push ball rolled {d} m");
    }

    #[test]
    fn spin_curves_dips_and_brakes() {
        // Flying toward -z (right is +x): side spin curves right, dip spin pulls down.
        let fly = |spin: Vec3| {
            let mut pos = Vec3::new(0.0, 5.0, 0.0);
            let mut ball = Ball { vel: Vec3::new(0.0, 0.0, -30.0), spin, ..default() };
            run(&mut pos, &mut ball, 0.3);
            pos
        };
        let plain = fly(Vec3::ZERO);
        assert!(fly(Vec3::Z * 600.0).x > plain.x + 0.3, "side spin curves right");
        assert!(fly(Vec3::Y * 600.0).y < plain.y - 0.3, "dip spin pulls down");
        assert!(fly(Vec3::Y * -210.0).y > plain.y, "the shot's built-in spin lifts");
        // Rolling, dip spin brakes the ball instead.
        let roll = |spin: Vec3| {
            let mut pos = Vec3::new(0.0, BALL_RADIUS, 0.0);
            let mut ball = Ball { vel: Vec3::new(0.0, 0.0, -10.0), spin, ..default() };
            run(&mut pos, &mut ball, 0.5);
            -pos.z
        };
        assert!(roll(Vec3::Y * 600.0) < roll(Vec3::ZERO) - 0.3);
    }

    #[test]
    fn gravity_grows_while_falling() {
        // From rest, 5 m takes less than it would at a constant 1.625 g.
        let mut pos = Vec3::new(0.0, 5.0, 0.0);
        let mut ball = Ball::default();
        let mut t = 0.0;
        while pos.y > 1.0 {
            step(&mut pos, &mut ball, 1.0 / 240.0);
            t += 1.0 / 240.0;
        }
        let constant = (2.0 * 4.0 / (GRAVITY * BALL_MASS)).sqrt();
        assert!(t < constant - 0.02, "fell 4 m in {t} s, constant gravity takes {constant} s");
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
        let mut pos = Vec3::new(0.0, 1.0, 40.0);
        let mut ball = Ball { vel: Vec3::new(0.0, 0.0, 30.0), ..default() };
        assert_eq!(run(&mut pos, &mut ball, 1.0), Some(1.0));

        let mut pos = Vec3::new(10.0, 1.0, 40.0);
        let mut ball = Ball { vel: Vec3::new(0.0, 0.0, 30.0), ..default() };
        assert_eq!(run(&mut pos, &mut ball, 0.5), None);
        assert!(ball.vel.z < 0.0, "the end wall sends it back");
    }
}
