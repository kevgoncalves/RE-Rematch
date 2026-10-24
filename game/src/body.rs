//! A jointed body for the player, posed every frame from the gameplay state so you can read what
//! the character is doing. The poses are our own procedural ones, nothing from the game.
//!
//! The player entity sits at hip height and faces -Z; each joint swings its limb, which hangs
//! down from it. A positive pitch swings a limb forward.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::player::{CameraRig, Kick, Player};
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

/// A joint's target pose: pitch (forward swing), roll (outwards for limbs, sideways for the
/// spine) and yaw (twist), radians; plus a vertical offset for the body.
#[derive(Clone, Copy, Default)]
struct Pose {
    pitch: f32,
    roll: f32,
    yaw: f32,
    drop: f32,
}

/// Running state of the animation.
#[derive(Default)]
pub struct Gait {
    /// The stride's phase, radians.
    phase: f32,
    /// Seconds since the ball left on a kick, and that kick's kind.
    follow: Option<(f32, Kick)>,
    last_kick: Option<Kick>,
    receive: f32,
    /// Smoothed acceleration in the body's frame and turning speed (rad/s), from last frame's
    /// velocity and facing.
    accel: Vec3,
    yaw_rate: f32,
    prev_vel: Vec3,
    prev_facing: f32,
    clock: f32,
}

pub fn spawn_body(commands: &mut Commands, player: Entity, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let shirt = materials.add(Color::srgb(0.92, 0.38, 0.16));
    let shorts = materials.add(Color::srgb(0.10, 0.12, 0.22));
    let skin = materials.add(Color::srgb(0.85, 0.64, 0.5));
    let socks = materials.add(Color::srgb(0.92, 0.38, 0.16));
    let boots = materials.add(Color::srgb(0.06, 0.06, 0.07));
    let hair = materials.add(Color::srgb(0.12, 0.08, 0.05));
    // A capsule hanging `len` metres below its joint.
    let mut limb = |radius: f32, len: f32| (Mesh3d(meshes.add(Capsule3d::new(radius, len - 2.0 * radius))), Transform::from_xyz(0.0, -len / 2.0, 0.0));
    let thigh = limb(0.075, 0.44);
    let shin = limb(0.058, 0.43);
    let upper_arm = limb(0.052, 0.29);
    let forearm = limb(0.045, 0.27);
    let joint = |part: Part, at: Vec3| (part, Transform::from_translation(at), Visibility::default());

    commands.entity(player).with_children(|root| {
        root.spawn(joint(Part::Body, Vec3::ZERO)).with_children(|body| {
            body.spawn((Mesh3d(meshes.add(Cuboid::new(0.34, 0.16, 0.2))), MeshMaterial3d(shorts.clone()), Transform::from_xyz(0.0, 0.0, 0.0)));
            for side in [-1.0f32, 1.0] {
                body.spawn(joint(Part::Thigh(side), Vec3::new(side * 0.1, -0.02, 0.0))).with_children(|hip| {
                    hip.spawn((thigh.0.clone(), MeshMaterial3d(skin.clone()), thigh.1));
                    hip.spawn((Mesh3d(meshes.add(Cylinder::new(0.085, 0.18))), MeshMaterial3d(shorts.clone()), Transform::from_xyz(0.0, -0.07, 0.0)));
                    hip.spawn(joint(Part::Shin(side), Vec3::new(0.0, -0.44, 0.0))).with_children(|knee| {
                        knee.spawn((shin.0.clone(), MeshMaterial3d(socks.clone()), shin.1));
                        knee.spawn(joint(Part::Foot(side), Vec3::new(0.0, -0.43, 0.0))).with_children(|ankle| {
                            ankle.spawn((Mesh3d(meshes.add(Cuboid::new(0.1, 0.07, 0.26))), MeshMaterial3d(boots.clone()), Transform::from_xyz(0.0, -0.03, -0.06)));
                        });
                    });
                });
            }
            body.spawn(joint(Part::Spine, Vec3::new(0.0, 0.08, 0.0))).with_children(|spine| {
                spine.spawn((Mesh3d(meshes.add(Capsule3d::new(0.17, 0.2))), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, 0.28, 0.0).with_scale(Vec3::new(1.1, 1.0, 0.65))));
                spine.spawn(joint(Part::Head, Vec3::new(0.0, 0.56, 0.0))).with_children(|neck| {
                    neck.spawn((Mesh3d(meshes.add(Cylinder::new(0.05, 0.1))), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 0.04, 0.0)));
                    neck.spawn((Mesh3d(meshes.add(Sphere::new(0.11))), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 0.17, 0.0).with_scale(Vec3::new(0.9, 1.05, 1.0))));
                    neck.spawn((Mesh3d(meshes.add(Sphere::new(0.112))), MeshMaterial3d(hair.clone()), Transform::from_xyz(0.0, 0.2, 0.015).with_scale(Vec3::new(0.92, 0.85, 1.0))));
                    // Nose, so you can tell where the head faces.
                    neck.spawn((Mesh3d(meshes.add(Sphere::new(0.025))), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 0.16, -0.105)));
                });
                for side in [-1.0f32, 1.0] {
                    spine.spawn(joint(Part::UpperArm(side), Vec3::new(side * 0.23, 0.47, 0.0))).with_children(|shoulder| {
                        shoulder.spawn((upper_arm.0.clone(), MeshMaterial3d(skin.clone()), upper_arm.1));
                        shoulder.spawn((Mesh3d(meshes.add(Sphere::new(0.068))), MeshMaterial3d(shirt.clone()), Transform::from_xyz(0.0, -0.04, 0.0)));
                        shoulder.spawn(joint(Part::Forearm(side), Vec3::new(0.0, -0.29, 0.0))).with_children(|elbow| {
                            elbow.spawn((forearm.0.clone(), MeshMaterial3d(skin.clone()), forearm.1));
                        });
                    });
                }
            });
        });
    });
}

/// Poses every joint from the player's state: idle, run and sprint strides (also backwards and
/// sideways), leaning against braking and into curves, turning on the spot, lock move's crouch,
/// dribbles and feints, dashes, extra effort, exhaustion, carrying and receiving the ball, and kicks.
/// The head follows the camera's aim.
pub fn animate_body(
    time: Res<Time>,
    player: Single<(&Player, &Transform)>,
    camera: Option<Single<&CameraRig>>,
    mut parts: Query<(&Part, &mut Transform), Without<Player>>,
    mut gait: Local<Gait>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let (p, root) = *player;
    let to_local = root.rotation.inverse();
    let local = to_local * p.vel;
    let (fwd, side, speed) = (-local.z, local.x, local.length());

    // Acceleration and turning speed, smoothed over a few frames: they drive the leans.
    let smooth = 1.0 - (-10.0 * dt).exp();
    let accel = to_local * (p.vel - gait.prev_vel) / dt;
    gait.accel = gait.accel.lerp(accel, smooth);
    let yaw_rate = wrap(p.facing - gait.prev_facing) / dt;
    gait.yaw_rate += (yaw_rate - gait.yaw_rate) * smooth;
    gait.prev_vel = p.vel;
    gait.prev_facing = p.facing;
    gait.clock += dt;
    let (fwd_accel, side_accel) = (-gait.accel.z, gait.accel.x);

    // Stride: one cycle covers more ground the faster you go; turning on the spot shuffles the feet.
    let shuffle = (gait.yaw_rate.abs() * 0.5).min(3.0);
    let pace = speed.max(shuffle);
    gait.phase = (gait.phase + pace * dt / (1.2 + 0.45 * pace) * TAU) % TAU;
    let amp = (speed / 6.0).max(shuffle * 0.08).min(1.3);
    let (fwd_share, side_share) = if speed > 0.1 { (fwd / speed, side / speed) } else { (1.0, 0.0) };

    // Kick follow-through starts the frame the ball leaves.
    let kick = p.kick.map(|k| k.kind);
    if let (Some(last), None) = (gait.last_kick, kick) {
        gait.follow = Some((0.0, last));
    }
    gait.last_kick = kick;
    if let Some((t, k)) = &mut gait.follow {
        *t += dt;
        if *t > kick_shape(*k).follow {
            gait.follow = None;
        }
    }
    gait.receive = if p.receiving.is_some() { (gait.receive + dt).min(CONTROL_ACT) } else { 0.0 };

    let phase = gait.phase;
    let leg_swing = |s: f32| phase + if s > 0.0 { 0.0 } else { PI };
    let pose = |part: Part| -> Pose {
        let mut q = Pose::default();
        match part {
            Part::Body => {
                q.drop = 0.03 * amp - 0.035 * amp * (2.0 * phase).cos();
                // Lean back against braking and forward into pushing off; bank into curves.
                q.pitch = -0.05 * amp * fwd_share - (0.03 * fwd_accel).clamp(-0.3, 0.35);
                q.roll = -(0.03 * side_accel).clamp(-0.35, 0.35);
            }
            Part::Spine => {
                q.pitch = -(0.04 + 0.035 * speed.min(9.5)) * fwd_share.max(0.0);
                q.yaw = 0.12 * amp * phase.sin();
            }
            Part::Head => q.pitch = (0.04 + 0.035 * speed.min(9.5)) * fwd_share.max(0.0) * 0.7,
            Part::Thigh(s) => {
                let swing = leg_swing(s).sin();
                q.pitch = 0.75 * amp * swing * fwd_share;
                q.roll = 0.35 * amp * swing * side_share * s;
            }
            Part::Shin(s) => q.pitch = -amp * (0.2 + 1.1 * leg_swing(s).cos().max(0.0)),
            Part::Foot(s) => q.pitch = 0.3 * amp * leg_swing(s).cos().min(0.0),
            Part::UpperArm(s) => {
                q.pitch = -0.7 * amp * leg_swing(s).sin() * fwd_share;
                q.roll = 0.12;
            }
            Part::Forearm(_) => q.pitch = 0.35 + 0.7 * amp.min(1.0),
        }
        q
    };
    let mut poses: Vec<(Part, Pose)> = all_parts().map(|part| (part, pose(part))).collect();
    let set = |poses: &mut Vec<(Part, Pose)>, part: Part, f: &dyn Fn(&mut Pose)| {
        if let Some((_, q)) = poses.iter_mut().find(|(p, _)| *p == part) {
            f(q);
        }
    };
    let get = |poses: &Vec<(Part, Pose)>, part: Part| poses.iter().find(|(p, _)| *p == part).map_or(Pose::default(), |(_, q)| *q);

    // Standing still: breathing and a slow weight shift.
    if speed < 0.3 && shuffle < 0.5 && p.kick.is_none() && !p.locked {
        let (breath, shift) = ((gait.clock * 1.7).sin(), (gait.clock * 0.6).sin());
        set(&mut poses, Part::Spine, &|q| q.pitch += 0.025 * breath);
        set(&mut poses, Part::Body, &|q| q.roll += 0.03 * shift);
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::UpperArm(s), &|q| q.roll += 0.03 * breath);
            set(&mut poses, Part::Thigh(s), &|q| q.roll -= 0.03 * shift * s);
        }
    }
    // Hard braking (stops and the brake of a big turn): the leading leg plants, the other bends.
    let brake = ((-fwd_accel - 5.0) / 8.0).clamp(0.0, 1.0) * if speed > 0.5 { 1.0 } else { 0.0 };
    if brake > 0.0 {
        let mix = |q: &mut Pose, target: f32| q.pitch += (target - q.pitch) * brake;
        set(&mut poses, Part::Thigh(-1.0), &|q| mix(q, 0.55));
        set(&mut poses, Part::Shin(-1.0), &|q| mix(q, -0.1));
        set(&mut poses, Part::Foot(-1.0), &|q| mix(q, 0.3));
        set(&mut poses, Part::Thigh(1.0), &|q| mix(q, -0.25));
        set(&mut poses, Part::Shin(1.0), &|q| mix(q, -0.75));
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::UpperArm(s), &|q| q.roll += 0.35 * brake);
        }
    }
    // Lock move: crouched, arms out, short steps.
    if p.locked {
        set(&mut poses, Part::Body, &|q| q.drop += 0.13);
        set(&mut poses, Part::Spine, &|q| q.pitch -= 0.35);
        set(&mut poses, Part::Head, &|q| q.pitch += 0.3);
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::Thigh(s), &|q| {
                q.pitch = q.pitch * 0.6 + 0.55;
                q.roll += 0.1 * s;
            });
            set(&mut poses, Part::Shin(s), &|q| q.pitch = q.pitch * 0.6 - 0.9);
            set(&mut poses, Part::Foot(s), &|q| q.pitch = 0.35);
            set(&mut poses, Part::UpperArm(s), &|q| {
                q.pitch = 0.35;
                q.roll = 0.55;
            });
            set(&mut poses, Part::Forearm(s), &|q| q.pitch = 0.6);
        }
    }
    // Exhausted: hunched, arms hanging.
    if p.exhausted {
        set(&mut poses, Part::Spine, &|q| q.pitch -= 0.22);
        set(&mut poses, Part::Head, &|q| q.pitch += 0.1);
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::UpperArm(s), &|q| q.pitch *= 0.5);
            set(&mut poses, Part::Forearm(s), &|q| q.pitch = 0.25);
        }
    }
    // Extra effort: pumping arms and a deeper lean.
    if p.extra_active {
        set(&mut poses, Part::Spine, &|q| q.pitch -= 0.15);
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::UpperArm(s), &|q| q.pitch *= 1.5);
            set(&mut poses, Part::Forearm(s), &|q| q.pitch = 1.4);
        }
    }
    // Running with the ball: the right foot touches it once a stride.
    if p.has_ball && p.kick.is_none() && p.dribble.is_none() && p.receiving.is_none() && !p.locked && speed > 1.0 {
        let touch = phase.sin().max(0.0).powi(6);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch += 0.2 * touch);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch += (-0.15 - q.pitch) * touch);
        set(&mut poses, Part::Foot(1.0), &|q| q.pitch = -0.25 * touch);
    }
    // Dribble: lean into the move, the right foot carrying the ball. A feint first drops the
    // shoulder the other way.
    if let Some(d) = p.dribble {
        let dir = to_local * d.dir;
        set(&mut poses, Part::Body, &|q| {
            q.pitch = -0.05 - 0.2 * -dir.z;
            q.roll = -0.25 * dir.x;
        });
        set(&mut poses, Part::Thigh(1.0), &|q| q.roll -= 0.25 * dir.x);
        if d.feint {
            let fake = (1.0 - d.t / 0.2).max(0.0);
            let s = if dir.x.abs() > 0.2 { dir.x.signum() } else { 1.0 };
            set(&mut poses, Part::Spine, &|q| {
                q.roll += 0.5 * s * fake;
                q.yaw += 0.4 * s * fake;
            });
        }
    }
    // Dash: a long lunge.
    if p.dash.is_some() {
        set(&mut poses, Part::Spine, &|q| q.pitch -= 0.45);
        set(&mut poses, Part::Body, &|q| q.drop += 0.1);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = 0.9);
        set(&mut poses, Part::Thigh(-1.0), &|q| q.pitch = -0.6);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -0.4);
        set(&mut poses, Part::Shin(-1.0), &|q| q.pitch = -0.9);
    }
    // Receiving: the right foot reaches for the ball and cushions it.
    if p.receiving.is_some() {
        let reach = (gait.receive / CONTROL_SNAP).min(1.0);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = q.pitch * (1.0 - reach) + 0.45 * reach);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = q.pitch * (1.0 - reach) - 0.35 * reach);
        set(&mut poses, Part::Foot(1.0), &|q| q.pitch = 0.4 * reach);
    }
    // Kicks: the right leg draws back while you charge, then swings through. Shots lean over the
    // ball, lobs lean back and scoop higher.
    if let Some(k) = p.kick {
        let wind = (k.elapsed / k.kind.buildup()).min(1.0);
        let shape = kick_shape(k.kind);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = -shape.back * wind);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -shape.knee * wind);
        set(&mut poses, Part::Thigh(-1.0), &|q| q.pitch = 0.15 * wind);
        set(&mut poses, Part::Shin(-1.0), &|q| q.pitch = -0.35 * wind);
        set(&mut poses, Part::Spine, &|q| {
            q.pitch = 0.12 * wind;
            q.yaw = -0.3 * wind * shape.open;
        });
        set(&mut poses, Part::UpperArm(-1.0), &|q| {
            q.pitch = 0.4 * wind;
            q.roll = 0.12 + 0.8 * wind * shape.open;
        });
        set(&mut poses, Part::UpperArm(1.0), &|q| {
            q.pitch = -0.5 * wind;
            q.roll = 0.12 + 0.4 * wind;
        });
    } else if let Some((t, kind)) = gait.follow {
        let shape = kick_shape(kind);
        let u = t / shape.follow;
        // Through the ball in the first third, then settle.
        let through = if u < 0.33 { -shape.back + (shape.back + shape.peak) * (u / 0.33) } else { shape.peak * (1.0 - (u - 0.33) / 0.67) };
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = through);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -0.15);
        set(&mut poses, Part::Foot(1.0), &|q| q.pitch = -0.3);
        set(&mut poses, Part::Spine, &|q| {
            q.pitch = shape.lean * (1.0 - u);
            q.yaw = 0.25 * shape.open * (1.0 - u);
        });
        set(&mut poses, Part::UpperArm(-1.0), &|q| q.roll = 0.12 + 0.7 * shape.open * (1.0 - u));
    }
    // The head turns to where the camera aims; the upper body follows a little.
    if let Some(rig) = camera {
        let look = wrap(rig.yaw.to_radians() - p.facing).clamp(-1.4, 1.4);
        set(&mut poses, Part::Spine, &|q| q.yaw += 0.25 * look);
        let spine_yaw = get(&poses, Part::Spine).yaw;
        set(&mut poses, Part::Head, &|q| {
            q.yaw = (look - spine_yaw).clamp(-1.2, 1.2);
            q.pitch += (rig.pitch.to_radians() * 0.6).clamp(-0.45, 0.35);
        });
    }

    let blend = 1.0 - (-25.0 * dt).exp();
    for (part, mut tf) in &mut parts {
        let q = get(&poses, *part);
        let limb_side = match *part {
            Part::UpperArm(s) | Part::Forearm(s) | Part::Thigh(s) | Part::Shin(s) | Part::Foot(s) => s,
            _ => 1.0,
        };
        // Limbs roll outwards on their own side.
        let target = Quat::from_rotation_y(q.yaw) * Quat::from_rotation_z(q.roll * limb_side) * Quat::from_rotation_x(q.pitch);
        tf.rotation = tf.rotation.slerp(target, blend);
        if *part == Part::Body {
            tf.translation.y += (-q.drop - tf.translation.y) * blend;
        }
    }
}

fn all_parts() -> impl Iterator<Item = Part> {
    [Part::Body, Part::Spine, Part::Head].into_iter().chain([-1.0f32, 1.0].into_iter().flat_map(|s| {
        [Part::UpperArm(s), Part::Forearm(s), Part::Thigh(s), Part::Shin(s), Part::Foot(s)]
    }))
}

/// Angle wrapped to -pi..pi.
fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// How a kick looks: how far the leg draws back, the knee bend, how much the body opens up, the
/// spine's lean through the ball (negative = over it), how high the leg finishes, and how long
/// the follow-through lasts.
struct KickShape {
    back: f32,
    knee: f32,
    open: f32,
    lean: f32,
    peak: f32,
    follow: f32,
}

fn kick_shape(kind: Kick) -> KickShape {
    let (back, knee, open, lean, peak, follow) = match kind {
        Kick::Shot => (1.0, 1.5, 1.0, -0.25, 1.3, SHOT_FOLLOW_THROUGH + 0.2),
        Kick::Pass | Kick::ShortPass => (0.6, 0.9, 0.5, -0.1, 0.9, 0.3),
        Kick::Lob | Kick::ShortLob => (0.75, 1.2, 0.6, 0.25, 1.5, 0.4),
        Kick::PushBall | Kick::DribblePushBall => (0.35, 0.6, 0.2, -0.1, 0.6, 0.2),
    };
    KickShape { back, knee, open, lean, peak, follow }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(player: Player) -> App {
        let mut app = App::new();
        app.init_resource::<Time>().add_systems(Update, animate_body);
        let world = app.world_mut();
        let root = world.spawn((player, Transform::default())).id();
        spawn_body(&mut world.commands(), root, &mut Assets::default(), &mut Assets::default());
        world.flush();
        app
    }

    fn run_for(app: &mut App, seconds: f32) {
        for _ in 0..(seconds * 60.0).round() as usize {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.update();
        }
    }

    /// A joint's pitch and height.
    fn joint(app: &mut App, part: Part) -> (f32, f32) {
        let tf = *app.world_mut().query::<(&Part, &Transform)>().iter(app.world()).find(|(p, _)| **p == part).unwrap().1;
        (tf.rotation.to_euler(EulerRot::YZX).2, tf.translation.y)
    }

    fn set_speed(app: &mut App, z: f32) {
        app.world_mut().query::<&mut Player>().single_mut(app.world_mut()).unwrap().vel.z = z;
    }

    #[test]
    fn running_swings_the_legs_and_lock_move_crouches() {
        // Running forward, at some point within a stride the legs are clearly apart.
        let mut app = app_with(Player { vel: Vec3::new(0.0, 0.0, -5.0), ..default() });
        let mut apart: f32 = 0.0;
        for _ in 0..30 {
            run_for(&mut app, 1.0 / 60.0);
            apart = apart.max((joint(&mut app, Part::Thigh(1.0)).0 - joint(&mut app, Part::Thigh(-1.0)).0).abs());
        }
        assert!(apart > 0.6, "legs apart by {apart} rad");
        // Lock move, standing: hips lower and both knees bent.
        let mut app = app_with(Player { locked: true, ..default() });
        run_for(&mut app, 0.5);
        assert!(joint(&mut app, Part::Body).1 < -0.1);
        assert!(joint(&mut app, Part::Shin(1.0)).0 < -0.7 && joint(&mut app, Part::Shin(-1.0)).0 < -0.7);
    }

    #[test]
    fn braking_leans_back_and_pushing_off_leans_forward() {
        let mut app = app_with(Player { vel: Vec3::new(0.0, 0.0, -5.0), ..default() });
        run_for(&mut app, 1.0);
        // Stop over 0.38 s, as from a run: the body leans back.
        let mut lean: f32 = 0.0;
        for i in 1..=23 {
            set_speed(&mut app, -5.0 * (1.0 - i as f32 / 23.0));
            run_for(&mut app, 1.0 / 60.0);
            lean = lean.max(joint(&mut app, Part::Body).0);
        }
        assert!(lean > 0.15, "leaned back {lean} rad");
        // Pushing off from standing leans forward.
        run_for(&mut app, 1.0);
        let mut lean: f32 = 0.0;
        for i in 1..=12 {
            set_speed(&mut app, -5.0 * i as f32 / 12.0);
            run_for(&mut app, 1.0 / 60.0);
            lean = lean.min(joint(&mut app, Part::Body).0);
        }
        assert!(lean < -0.15, "leaned forward {lean} rad");
    }
}
