//! A simple, sturdy jointed body for the player, posed every frame from the gameplay state. The
//! poses are our own; their timing is the game's (contact frames, freezes, move lengths).
//!
//! Legs are placed by two-bone IK: while running each foot stays planted while it carries you and
//! then steps to the next foothold, so the feet don't slide; on a touch the foot goes to the ball
//! itself at the frame the game's data has the contact.
//!
//! The player entity sits at hip height (the game's capsule is 1.8 m tall) and faces -Z; each
//! joint swings its limb, which hangs down from it. A positive pitch swings a limb forward.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::ball::Ball;
use crate::player::{Heading, Kick, KickState, Player, contact_time};
use crate::tuning::*;

#[derive(Component, Clone, Copy, PartialEq)]
pub enum Part {
    /// Everything above the root: bobbing, crouching and leaning.
    Body,
    Spine,
    Head,
    /// Side: -1 left, +1 right.
    UpperArm(f32),
    Forearm(f32),
    Thigh(f32),
    Shin(f32),
    Foot(f32),
}

// The rig, in metres: hips under the body joint, then thigh and shin; the ankle sits this high
// with the foot flat on the ground.
const HIP_X: f32 = 0.12;
const HIP_Y: f32 = -0.02;
const THIGH: f32 = 0.43;
const SHIN: f32 = 0.40;
const ANKLE_HEIGHT: f32 = 0.065;
const FRAME: f32 = 1.0 / 60.0;

/// An upper-body joint's target pose: pitch (forward swing), roll (outwards for limbs, sideways for
/// the spine) and yaw (twist), radians.
#[derive(Clone, Copy, Default)]
struct Pose {
    pitch: f32,
    roll: f32,
    yaw: f32,
}

#[derive(Clone, Copy, Default)]
struct Foot {
    /// Where the foot is planted (world), and where its step started (from under the hip).
    planted: Vec3,
    from: Vec3,
    stance: bool,
    /// Last frame's target (world).
    last: Vec3,
}

/// Running state of the animation.
#[derive(Default)]
pub struct Gait {
    /// Stride phase, 0..1 per full cycle (two steps).
    phase: f32,
    /// Smoothed forward lean, body drop and side lean.
    lean: f32,
    drop: f32,
    roll: f32,
    /// Left and right foot.
    feet: [Foot; 2],
    started: bool,
    /// The kick in progress, and after the touch its follow-through (seconds since the touch).
    kick: Option<KickState>,
    follow: Option<(f32, Kick)>,
}

pub fn spawn_body(commands: &mut Commands, player: Entity, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let shirt = materials.add(Color::srgb(0.92, 0.38, 0.16));
    let shorts = materials.add(Color::srgb(0.10, 0.12, 0.22));
    let skin = materials.add(Color::srgb(0.80, 0.62, 0.5));
    let boots = materials.add(Color::srgb(0.06, 0.06, 0.07));
    // A capsule hanging `len` metres below its joint.
    let mut limb = |radius: f32, len: f32| (Mesh3d(meshes.add(Capsule3d::new(radius, len - 2.0 * radius))), Transform::from_xyz(0.0, -len / 2.0, 0.0));
    let thigh = limb(0.1, THIGH + 0.01);
    let shin = limb(0.08, SHIN + 0.03);
    let upper_arm = limb(0.075, 0.32);
    let forearm = limb(0.065, 0.3);
    let joint = |part: Part, at: Vec3| (part, Transform::from_translation(at), Visibility::default());

    commands.entity(player).with_children(|root| {
        root.spawn(joint(Part::Body, Vec3::ZERO)).with_children(|body| {
            body.spawn((Mesh3d(meshes.add(Cuboid::new(0.4, 0.22, 0.26))), MeshMaterial3d(shorts.clone()), Transform::default()));
            for side in [-1.0f32, 1.0] {
                body.spawn(joint(Part::Thigh(side), Vec3::new(side * HIP_X, HIP_Y, 0.0))).with_children(|hip| {
                    hip.spawn((thigh.0.clone(), MeshMaterial3d(shorts.clone()), thigh.1));
                    hip.spawn(joint(Part::Shin(side), Vec3::new(0.0, -THIGH, 0.0))).with_children(|knee| {
                        knee.spawn((shin.0.clone(), MeshMaterial3d(skin.clone()), shin.1));
                        knee.spawn(joint(Part::Foot(side), Vec3::new(0.0, -SHIN, 0.0))).with_children(|ankle| {
                            ankle.spawn((Mesh3d(meshes.add(Cuboid::new(0.13, 0.09, 0.28))), MeshMaterial3d(boots.clone()), Transform::from_xyz(0.0, -0.02, -0.06)));
                        });
                    });
                });
            }
            body.spawn(joint(Part::Spine, Vec3::new(0.0, 0.1, 0.0))).with_children(|spine| {
                spine.spawn((Mesh3d(meshes.add(Capsule3d::new(0.2, 0.32))), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, 0.3, 0.0).with_scale(Vec3::new(1.25, 1.0, 0.75))));
                spine.spawn(joint(Part::Head, Vec3::new(0.0, 0.62, 0.0))).with_children(|neck| {
                    neck.spawn((Mesh3d(meshes.add(Sphere::new(0.14))), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 0.17, 0.0)));
                });
                for side in [-1.0f32, 1.0] {
                    spine.spawn(joint(Part::UpperArm(side), Vec3::new(side * 0.31, 0.55, 0.0))).with_children(|shoulder| {
                        shoulder.spawn((upper_arm.0.clone(), MeshMaterial3d(shirt.clone()), upper_arm.1));
                        shoulder.spawn(joint(Part::Forearm(side), Vec3::new(0.0, -0.32, 0.0))).with_children(|elbow| {
                            elbow.spawn((forearm.0.clone(), MeshMaterial3d(skin.clone()), forearm.1));
                        });
                    });
                }
            });
        });
    });
}

/// Two-bone IK for a leg: the thigh and shin rotations (in the body's frame) that put the ankle at
/// `target`, given relative to the hip in the body's frame. The knee points forward.
fn leg_ik(target: Vec3) -> (Quat, Quat) {
    let dist = target.length().clamp(0.05, THIGH + SHIN - 1e-3);
    let dir = target.normalize_or(Vec3::NEG_Y);
    let cos_knee = ((THIGH * THIGH + SHIN * SHIN - dist * dist) / (2.0 * THIGH * SHIN)).clamp(-1.0, 1.0);
    let bend = PI - cos_knee.acos();
    let cos_hip = ((THIGH * THIGH + dist * dist - SHIN * SHIN) / (2.0 * THIGH * dist)).clamp(-1.0, 1.0);
    let aim = Quat::from_rotation_arc(Vec3::NEG_Y, dir);
    (aim * Quat::from_rotation_x(cos_hip.acos()), Quat::from_rotation_x(-bend))
}

/// Share of the stride a foot spends on the ground: walking ~0.6, running ~0.35.
fn duty(speed: f32) -> f32 {
    0.62 - 0.27 * ((speed - 1.5) / 4.5).clamp(0.0, 1.0)
}

/// Ground covered by one full stride (two steps), metres.
fn stride(speed: f32, locked: bool) -> f32 {
    if locked { 0.9 + 0.3 * speed } else { 1.2 + 0.45 * speed }
}

/// 0 -> 1 -> 0 around `at`: rising over `before`, holding for `hold`, falling over `after` (seconds).
fn bell(t: f32, at: f32, before: f32, hold: f32, after: f32) -> f32 {
    let s = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    if t < at { s((t - (at - before)) / before) } else { 1.0 - s((t - at - hold) / after) }
}

/// The frame a dribble's foot touches the ball (responsiveness.md §2).
fn dribble_contact(feint: bool, exhausted: bool, heading: Heading) -> f32 {
    let frames = match (exhausted, feint, heading) {
        (true, ..) => 8.0,
        (_, true, Heading::Forward) => 10.0,
        (_, true, _) => 8.0,
        (_, _, Heading::Forward) => 17.0,
        (_, _, Heading::Side) => 8.0,
        (_, _, Heading::Back) => 0.0,
    };
    frames * FRAME
}

/// A kick's foot path, as offsets from the ground under the right hip in the player's frame
/// (x right, y up, z back): the wind-up and the follow-through's peak.
fn kick_path(kind: Kick) -> (Vec3, Vec3) {
    match kind {
        Kick::Shot => (Vec3::new(0.05, 0.45, 0.45), Vec3::new(0.0, 0.7, -0.75)),
        Kick::Pass => (Vec3::new(0.05, 0.3, 0.3), Vec3::new(-0.05, 0.35, -0.55)),
        Kick::Lob => (Vec3::new(0.05, 0.35, 0.35), Vec3::new(0.0, 0.8, -0.6)),
        Kick::ShortPass | Kick::ShortLob => (Vec3::new(0.03, 0.2, 0.2), Vec3::new(0.0, 0.25, -0.4)),
        Kick::PushBall | Kick::DribblePushBall => (Vec3::new(0.03, 0.15, 0.12), Vec3::new(0.0, 0.15, -0.35)),
    }
}

fn follow_time(kind: Kick) -> f32 {
    match kind {
        Kick::Shot => 0.4,
        Kick::PushBall | Kick::DribblePushBall => 0.2,
        _ => 0.3,
    }
}

/// Poses the body from the player's state.
pub fn animate_body(
    time: Res<Time>,
    player: Single<(&Player, &Transform)>,
    ball: Option<Single<&Transform, (With<Ball>, Without<Player>)>>,
    mut parts: Query<(&Part, &mut Transform), (Without<Player>, Without<Ball>)>,
    mut gait: Local<Gait>,
) {
    let dt = time.delta_secs();
    let (p, root) = *player;
    // The kick's freeze holds the pose too.
    if dt <= 0.0 || p.freeze > 0.0 {
        return;
    }
    let to_local = root.rotation.inverse();
    let local = to_local * p.vel;
    let (fwd, side, speed) = (-local.z, local.x, Vec3::new(local.x, 0.0, local.z).length());
    let (fwd_share, side_share) = if speed > 0.1 { (fwd / speed, side / speed) } else { (1.0, 0.0) };
    let ahead = root.rotation * Vec3::NEG_Z;
    let right = root.rotation * Vec3::X;
    let near_ball = ball
        .map(|b| b.translation)
        .filter(|b| Vec2::new(b.x - root.translation.x, b.z - root.translation.z).length() < 1.5);

    // Kicks: remember the one in progress; when it's gone the ball has left, so follow through.
    match (gait.kick, p.kick) {
        (Some(was), None) => gait.follow = Some((0.0, was.kind)),
        (_, Some(_)) => gait.follow = None,
        _ => {}
    }
    gait.kick = p.kick;
    if let Some((t, kind)) = gait.follow.as_mut() {
        *t += dt;
        if *t > follow_time(*kind) {
            gait.follow = None;
        }
    }

    // Body: crouch in lock move and dashes, bob with the stride, lean forward with speed and into
    // dribbles and dashes.
    let rate = 1.0 - (-12.0 * dt).exp();
    let amp = (speed / 7.0).min(1.2);
    // Running bends the knees more the faster you go, so the planted foot can reach.
    let crouch = if p.locked { 0.12 } else { 0.08 * (speed / 5.0).min(1.2) } + if p.dash.is_some() { 0.08 } else { 0.0 };
    // A foot stays planted only as far as the leg reaches from the hip: that caps its share of
    // the stride.
    let len = stride(speed, p.locked);
    gait.phase = (gait.phase + speed * dt / len).fract();
    let height = root.translation.y + HIP_Y - crouch - ANKLE_HEIGHT;
    let reach = ((0.96 * (THIGH + SHIN)).powi(2) - height * height).max(0.0).sqrt();
    let d = (if p.locked { 0.6 } else { duty(speed) }).min(2.0 * reach / len);
    // The hips are lowest halfway through each stance.
    let bob = 0.02 * amp * (1.0 + (2.0 * TAU * (gait.phase - d / 2.0)).cos());
    gait.drop += (crouch + bob - gait.drop) * rate;
    let lean = -(0.06 + 0.04 * speed.min(9.5)) * fwd_share.max(0.0);
    gait.lean += (lean - gait.lean) * (1.0 - (-6.0 * dt).exp());
    let roll = match (p.dribble.filter(|d| d.moving()), p.dash) {
        (Some(d), _) => -0.25 * (to_local * d.dir).x,
        (_, Some(d)) => -0.3 * (to_local * d.dir).x,
        _ => 0.0,
    };
    gait.roll += (roll - gait.roll) * rate;
    let body_rot = Quat::from_rotation_z(gait.roll);
    let body_pos = root.translation + Vec3::Y * -gait.drop;
    let hip = |s: f32| body_pos + root.rotation * (body_rot * Vec3::new(s * HIP_X, HIP_Y, 0.0));
    let ground = |v: Vec3| Vec3::new(v.x, ANKLE_HEIGHT, v.z);

    // Locomotion: each foot planted for its share of the stride, then stepping to the next
    // foothold, placed so it lands under you halfway through its next stance.
    let lift = if p.locked { 0.06 } else { 0.12 + 0.04 * speed.min(8.0) };
    let width = if p.locked { 0.08 } else { 0.0 };
    let carrying = p.has_ball && p.kick.is_none() && p.dribble.is_none() && p.receiving.is_none();
    let phase = gait.phase;
    let started = gait.started;
    let mut targets = [Vec3::ZERO; 2];
    for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
        let rest = ground(hip(s)) + right * (s * width);
        let foot = &mut gait.feet[i];
        if !started {
            *foot = Foot { planted: rest, from: Vec3::ZERO, stance: true, last: rest };
        }
        targets[i] = if speed < 0.3 {
            // Standing: feet stay put, drifting back under the hips if they've ended up far away.
            if !foot.stance {
                foot.stance = true;
                foot.planted = ground(foot.last);
            }
            let off = rest - foot.planted;
            if off.length() > 0.05 {
                foot.planted += off.clamp_length_max(2.5 * dt);
            }
            foot.planted
        } else {
            let u = (phase + if s > 0.0 { 0.0 } else { 0.5 }).fract();
            if u < d {
                if !foot.stance {
                    foot.stance = true;
                    foot.planted = ground(foot.last);
                }
                foot.planted
            } else {
                if foot.stance {
                    foot.stance = false;
                    foot.from = foot.planted - rest;
                }
                let w = (u - d) / (1.0 - d);
                // The foot lands half a stance ahead of the hip, so it is under you halfway through
                // the stance.
                let mut land = Vec3::new(p.vel.x, 0.0, p.vel.z) * (d * len / speed / 2.0);
                // Running with the ball, the right foot steps up to it: a touch every stride.
                if s > 0.0 && carrying {
                    if let Some(b) = near_ball {
                        land = ground(b - ahead * 0.18) - rest;
                    }
                }
                // The swing moves with the hips: the heel comes up behind first, then the foot
                // swings through to where it lands.
                let h = w * w * (3.0 - 2.0 * w * w);
                rest + foot.from.lerp(land, h) + Vec3::Y * (lift * (PI * w.sqrt()).sin())
            }
        };
    }
    gait.started = true;

    // Touches: the foot goes to the ball at the game's contact frames.
    let to_ball = |b: Vec3, back: f32| Vec3::new(b.x, b.y.max(ANKLE_HEIGHT), b.z) - ahead * back;
    let mut kick_weight = 0.0f32;
    if let Some(k) = p.kick {
        // Wind up while you charge; swing through to the ball from the release to the contact.
        let (back, _) = kick_path(k.kind);
        let windup = ground(hip(1.0)) + root.rotation * back;
        let first_contact = contact_time(&k);
        let contact = if k.released { first_contact.max(k.held) } else { first_contact };
        let foot_at = near_ball.map_or(ground(hip(1.0)) - ahead * 0.35, |b| to_ball(b, 0.18));
        let target = if !k.released {
            let w = (k.elapsed / (first_contact - 6.0 * FRAME).max(FRAME)).min(1.0);
            targets[1].lerp(windup, w)
        } else {
            let swing = ((k.elapsed - k.held) / (contact - k.held).max(4.0 * FRAME)).clamp(0.0, 1.0);
            windup.lerp(foot_at, swing * swing * (3.0 - 2.0 * swing))
        };
        kick_weight = (k.elapsed / (5.0 * FRAME)).min(1.0);
        targets[1] = targets[1].lerp(target, kick_weight);
    } else if let Some((t, kind)) = gait.follow {
        // Through the ball to the follow-through's peak, then back into the stride.
        let (_, through) = kick_path(kind);
        let peak = ground(hip(1.0)) + root.rotation * through;
        let u = t / follow_time(kind);
        let target = if u < 0.4 { gait.feet[1].last.lerp(peak, u / 0.4) } else { peak };
        kick_weight = 1.0 - ((u - 0.4) / 0.6).clamp(0.0, 1.0);
        targets[1] = targets[1].lerp(target, kick_weight);
    }
    if kick_weight > 0.0 {
        // The other foot plants beside the ball.
        let support = near_ball.map_or(ground(hip(-1.0)), |b| ground(b - right * 0.28 - ahead * 0.05));
        targets[0] = targets[0].lerp(support, kick_weight);
    }
    if let (Some(dr), Some(b)) = (p.dribble, near_ball) {
        let at = dribble_contact(dr.feint, dr.exhausted, dr.heading);
        let w = bell(dr.t, at, 6.0 * FRAME, 3.0 * FRAME, 6.0 * FRAME);
        // Fast cuts push it on, side dribbles sweep it across, drag backs pull it back with the sole.
        let touch = match dr.heading {
            Heading::Forward => to_ball(b, 0.18),
            Heading::Side => to_ball(b, 0.0) - dr.dir * 0.15,
            Heading::Back => to_ball(b, 0.0) + Vec3::Y * 0.1,
        };
        targets[1] = targets[1].lerp(touch, w);
    }
    if let (Some(t), Some(b)) = (p.receiving, near_ball) {
        // Receiving: the foot meets the ball and cushions it in.
        let w = (t / CONTROL_SNAP).min(1.0) * (1.0 - 0.5 * ((t - CONTROL_SNAP) / (CONTROL_ACT - CONTROL_SNAP)).clamp(0.0, 1.0));
        targets[1] = targets[1].lerp(to_ball(b, 0.15), w);
    }
    for (i, t) in targets.iter().enumerate() {
        gait.feet[i].last = *t;
    }

    // Upper body: arms swing against the legs, lean with speed; lock move opens the arms, kicks
    // open the body.
    let leg_swing = |s: f32| (TAU * (phase + if s > 0.0 { 0.0 } else { 0.5 })).sin();
    let mut spine = Pose { pitch: gait.lean, ..default() };
    let mut head = Pose { pitch: -0.6 * gait.lean, ..default() };
    let mut arms = [-1.0f32, 1.0].map(|s| {
        (Pose { pitch: 0.6 * amp * leg_swing(-s) * fwd_share, roll: 0.1 + 0.15 * amp * side_share.abs(), yaw: 0.0 }, Pose { pitch: 0.4 + 0.6 * amp.min(1.0), ..default() })
    });
    if p.locked {
        spine.pitch -= 0.3;
        head.pitch += 0.25;
        for (upper, fore) in &mut arms {
            *upper = Pose { pitch: 0.3, roll: 0.5, yaw: 0.0 };
            fore.pitch = 0.5;
        }
    }
    if p.extra_active {
        spine.pitch -= 0.15;
        for (upper, fore) in &mut arms {
            upper.pitch *= 1.5;
            fore.pitch = 1.4;
        }
    }
    if let Some(dash) = p.dash {
        spine.pitch = match dash_sector(to_local * dash.dir) {
            DashSector::Front => -0.45,
            DashSector::Side(_) => -0.15,
            DashSector::Back => 0.15,
        };
    }
    if kick_weight > 0.0 {
        // The left arm out for balance, the right one back.
        spine.yaw = -0.25 * kick_weight;
        arms[0].0.roll = 0.1 + 0.7 * kick_weight;
        arms[0].0.pitch = 0.3 * kick_weight;
        arms[1].0.pitch = -0.4 * kick_weight;
    }

    let blend = 1.0 - (-18.0 * dt).exp();
    let limb = |q: Pose, s: f32| Quat::from_rotation_y(q.yaw) * Quat::from_rotation_z(q.roll * s) * Quat::from_rotation_x(q.pitch);
    for (part, mut tf) in &mut parts {
        match *part {
            Part::Body => {
                tf.translation.y = -gait.drop;
                tf.rotation = body_rot;
            }
            Part::Spine => tf.rotation = tf.rotation.slerp(limb(spine, 1.0), blend),
            Part::Head => tf.rotation = tf.rotation.slerp(limb(head, 1.0), blend),
            Part::UpperArm(s) => tf.rotation = tf.rotation.slerp(limb(arms[(s > 0.0) as usize].0, s), blend),
            Part::Forearm(s) => tf.rotation = tf.rotation.slerp(limb(arms[(s > 0.0) as usize].1, s), blend),
            Part::Thigh(s) | Part::Shin(s) | Part::Foot(s) => {
                // The leg reaches its foot target: world -> the body's frame, relative to the hip.
                let i = (s > 0.0) as usize;
                let rel = body_rot.inverse() * (to_local * (targets[i] - body_pos)) - Vec3::new(s * HIP_X, HIP_Y, 0.0);
                let (thigh, shin) = leg_ik(rel);
                tf.rotation = match *part {
                    Part::Thigh(_) => thigh,
                    Part::Shin(_) => shin,
                    // Keep the foot flat on the ground.
                    _ => (thigh * shin).inverse(),
                };
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum DashSector {
    Front,
    /// +1 right, -1 left.
    Side(f32),
    Back,
}

/// Which dash animation a direction (in the body's frame) plays: front within 45 deg, back beyond
/// 135 deg, a side step in between.
fn dash_sector(dir: Vec3) -> DashSector {
    let angle = dir.x.atan2(-dir.z).to_degrees();
    match angle.abs() {
        a if a <= 45.0 => DashSector::Front,
        a if a < 135.0 => DashSector::Side(angle.signum()),
        _ => DashSector::Back,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leg_ik_reaches_its_target() {
        for target in [Vec3::new(0.0, -0.8, 0.0), Vec3::new(0.0, -0.6, -0.3), Vec3::new(0.1, -0.7, 0.25), Vec3::new(-0.15, -0.5, -0.2)] {
            let (thigh, shin) = leg_ik(target);
            let ankle = thigh * (Vec3::new(0.0, -THIGH, 0.0) + shin * Vec3::new(0.0, -SHIN, 0.0));
            assert!((ankle - target).length() < 0.01, "{target:?} reached {ankle:?}");
            // The knee bends forward, never backward.
            let knee = thigh * Vec3::new(0.0, -THIGH, 0.0);
            assert!(knee.z <= target.z + 0.01, "knee {knee:?} for {target:?}");
        }
    }

    #[test]
    fn dash_sectors() {
        assert_eq!(dash_sector(Vec3::NEG_Z), DashSector::Front);
        assert_eq!(dash_sector(Vec3::X), DashSector::Side(1.0));
        assert_eq!(dash_sector(Vec3::NEG_X), DashSector::Side(-1.0));
        assert_eq!(dash_sector(Vec3::Z), DashSector::Back);
    }

    fn app_with(player: Player) -> App {
        let mut app = App::new();
        app.init_resource::<Time>().add_systems(Update, animate_body);
        let world = app.world_mut();
        let root = world.spawn((player, Transform::from_xyz(0.0, 0.9, 0.0))).id();
        spawn_body(&mut world.commands(), root, &mut Assets::default(), &mut Assets::default());
        world.flush();
        app
    }

    fn part(app: &mut App, part: Part) -> Transform {
        *app.world_mut().query::<(&Part, &Transform)>().iter(app.world()).find(|(p, _)| **p == part).unwrap().1
    }

    /// The left ankle in the world, from the joints (forward kinematics).
    fn left_ankle(app: &mut App) -> Vec3 {
        let root = *app.world_mut().query::<(&Player, &Transform)>().single(app.world()).unwrap().1;
        let (body, thigh, shin, foot) = (part(app, Part::Body), part(app, Part::Thigh(-1.0)), part(app, Part::Shin(-1.0)), part(app, Part::Foot(-1.0)));
        root.transform_point(body.transform_point(thigh.transform_point(shin.transform_point(foot.translation))))
    }

    fn tick(app: &mut App) {
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        app.update();
    }

    #[test]
    fn running_feet_stay_planted_and_above_ground() {
        let vel = Vec3::new(0.0, 0.0, -5.0);
        let mut app = app_with(Player { vel, ..default() });
        let (mut planted_frames, mut longest, mut lowest) = (0, 0, f32::MAX);
        let mut last = Vec3::ZERO;
        for i in 0..120 {
            app.world_mut().query::<(&Player, &mut Transform)>().single_mut(app.world_mut()).unwrap().1.translation += vel / 60.0;
            tick(&mut app);
            let ankle = left_ankle(&mut app);
            lowest = lowest.min(ankle.y);
            if i > 30 {
                planted_frames = if (ankle - last).length() < 0.01 { planted_frames + 1 } else { 0 };
                longest = longest.max(planted_frames);
            }
            last = ankle;
        }
        // At 5 m/s the foot is on the ground for about a third of each 0.7 s stride: it stays put.
        assert!(longest >= 6, "longest planted run {longest} frames");
        assert!(lowest > ANKLE_HEIGHT - 0.02, "ankle went down to {lowest}");
    }

    #[test]
    fn lock_move_crouches_with_bent_knees() {
        let mut app = app_with(Player { locked: true, ..default() });
        for _ in 0..30 {
            tick(&mut app);
        }
        assert!(part(&mut app, Part::Body).translation.y < -0.1);
        let knee = part(&mut app, Part::Shin(1.0)).rotation.to_euler(EulerRot::YZX).2;
        assert!(knee < -0.7, "knee bend {knee}");
    }
}
