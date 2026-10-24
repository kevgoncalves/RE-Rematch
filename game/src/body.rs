//! A jointed body for the player, posed every frame from the gameplay state so you can read what
//! the character is doing. The poses are our own procedural ones, nothing from the game.
//!
//! The player entity sits at hip height and faces -Z; each joint swings its limb, which hangs
//! down from it. A positive pitch swings a limb forward.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::player::{Kick, Player};
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

/// Running state of the animation: the stride's phase and the kick follow-through.
#[derive(Default)]
pub struct Gait {
    phase: f32,
    /// Seconds since the ball left on a kick, and that kick's kind.
    follow: Option<(f32, Kick)>,
    last_kick: Option<Kick>,
    receive: f32,
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
/// sideways), lock move's crouch, dribbles, dashes, extra effort, receiving and kicks.
pub fn animate_body(
    time: Res<Time>,
    player: Single<(&Player, &Transform)>,
    mut parts: Query<(&Part, &mut Transform), Without<Player>>,
    mut gait: Local<Gait>,
) {
    let dt = time.delta_secs();
    let (p, root) = *player;
    let local = root.rotation.inverse() * p.vel;
    let (fwd, side, speed) = (-local.z, local.x, local.length());

    // Stride: one full cycle covers more ground the faster you go.
    gait.phase = (gait.phase + speed * dt / (1.2 + 0.45 * speed) * TAU) % TAU;
    let amp = (speed / 6.0).min(1.3);
    let (fwd_share, side_share) = if speed > 0.1 { (fwd / speed, side / speed) } else { (1.0, 0.0) };

    // Kick follow-through starts the frame the ball leaves.
    let kick = p.kick.map(|k| k.kind);
    if let (Some(last), None) = (gait.last_kick, kick) {
        gait.follow = Some((0.0, last));
    }
    gait.last_kick = kick;
    if let Some((t, k)) = &mut gait.follow {
        *t += dt;
        if *t > follow_time(*k) {
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
                q.pitch = -0.05 * amp * fwd_share;
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
    let mut poses: Vec<(Part, Pose)> = Vec::new();
    for part in all_parts() {
        poses.push((part, pose(part)));
    }
    let set = |poses: &mut Vec<(Part, Pose)>, part: Part, f: &dyn Fn(&mut Pose)| {
        if let Some((_, q)) = poses.iter_mut().find(|(p, _)| *p == part) {
            f(q);
        }
    };

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
    // Extra effort: pumping arms and a deeper lean.
    if p.extra_active {
        set(&mut poses, Part::Spine, &|q| q.pitch -= 0.15);
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::UpperArm(s), &|q| q.pitch *= 1.5);
            set(&mut poses, Part::Forearm(s), &|q| q.pitch = 1.4);
        }
    }
    // Dribble: lean into the move, the right foot carrying the ball.
    if let Some(d) = p.dribble {
        let dir = root.rotation.inverse() * d.dir;
        set(&mut poses, Part::Body, &|q| {
            q.pitch -= 0.2 * -dir.z;
            q.roll = -0.25 * dir.x;
        });
        set(&mut poses, Part::Thigh(1.0), &|q| q.roll -= 0.25 * dir.x);
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
    // Kicks: the right leg draws back while you charge, then swings through.
    if let Some(k) = p.kick {
        let wind = (k.elapsed / k.kind.buildup()).min(1.0);
        let (back, knee, open) = kick_shape(k.kind);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = -back * wind);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -knee * wind);
        set(&mut poses, Part::Thigh(-1.0), &|q| q.pitch = 0.15 * wind);
        set(&mut poses, Part::Shin(-1.0), &|q| q.pitch = -0.35 * wind);
        set(&mut poses, Part::Spine, &|q| {
            q.pitch = 0.12 * wind;
            q.yaw = -0.3 * wind * open;
        });
        set(&mut poses, Part::UpperArm(-1.0), &|q| {
            q.pitch = 0.4 * wind;
            q.roll = 0.12 + 0.8 * wind * open;
        });
        set(&mut poses, Part::UpperArm(1.0), &|q| {
            q.pitch = -0.5 * wind;
            q.roll = 0.12 + 0.4 * wind;
        });
    } else if let Some((t, kind)) = gait.follow {
        let (back, _, open) = kick_shape(kind);
        let u = t / follow_time(kind);
        // Through the ball in the first third, then settle.
        let through = if u < 0.33 { -back + (back + 1.3 * back.min(1.0)) * (u / 0.33) } else { 1.3 * back.min(1.0) * (1.0 - (u - 0.33) / 0.67) };
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = through);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -0.15);
        set(&mut poses, Part::Foot(1.0), &|q| q.pitch = -0.3);
        set(&mut poses, Part::Spine, &|q| {
            q.pitch = -0.15 * (1.0 - u);
            q.yaw = 0.25 * open * (1.0 - u);
        });
        set(&mut poses, Part::UpperArm(-1.0), &|q| q.roll = 0.12 + 0.7 * open * (1.0 - u));
    }

    let blend = 1.0 - (-25.0 * dt).exp();
    for (part, mut tf) in &mut parts {
        let Some((_, q)) = poses.iter().find(|(p, _)| p == part) else { continue };
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

/// How far the kicking leg draws back, how much the knee bends, and how much the body opens up.
fn kick_shape(kind: Kick) -> (f32, f32, f32) {
    match kind {
        Kick::Shot => (1.0, 1.5, 1.0),
        Kick::Pass | Kick::ShortPass => (0.6, 0.9, 0.5),
        Kick::Lob | Kick::ShortLob => (0.75, 1.2, 0.6),
        _ => (0.35, 0.6, 0.2),
    }
}

fn follow_time(kind: Kick) -> f32 {
    match kind {
        Kick::Shot => SHOT_FOLLOW_THROUGH + 0.2,
        _ => 0.3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose_after(player: Player, seconds: f32) -> Vec<(Part, Quat, f32)> {
        let mut app = App::new();
        app.init_resource::<Time>().add_systems(Update, animate_body);
        let world = app.world_mut();
        let root = world.spawn((player, Transform::default())).id();
        spawn_body(&mut world.commands(), root, &mut Assets::default(), &mut Assets::default());
        world.flush();
        for _ in 0..(seconds * 60.0) as usize {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.update();
        }
        app.world_mut().query::<(&Part, &Transform)>().iter(app.world()).map(|(p, t)| (*p, t.rotation, t.translation.y)).collect()
    }

    fn pitch(poses: &[(Part, Quat, f32)], part: Part) -> f32 {
        let (_, q, _) = poses.iter().find(|(p, ..)| *p == part).unwrap();
        q.to_euler(EulerRot::YZX).2
    }

    #[test]
    fn running_swings_the_legs_and_lock_move_crouches() {
        // Running forward, at some point within a stride the legs are clearly apart.
        let run = Player { vel: Vec3::new(0.0, 0.0, -5.0), ..default() };
        let mut apart: f32 = 0.0;
        for t in [0.3, 0.4, 0.5, 0.6] {
            let poses = pose_after(Player { ..run }, t);
            apart = apart.max((pitch(&poses, Part::Thigh(1.0)) - pitch(&poses, Part::Thigh(-1.0))).abs());
        }
        assert!(apart > 0.6, "legs apart by {apart} rad");
        // Lock move, standing: hips lower and both knees bent.
        let poses = pose_after(Player { locked: true, ..default() }, 0.5);
        let body = poses.iter().find(|(p, ..)| *p == Part::Body).unwrap().2;
        assert!(body < -0.1, "body at {body}");
        assert!(pitch(&poses, Part::Shin(1.0)) < -0.7 && pitch(&poses, Part::Shin(-1.0)) < -0.7);
    }
}
