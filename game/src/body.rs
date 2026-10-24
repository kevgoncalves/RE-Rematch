//! A simple, sturdy jointed body for the player, posed every frame from the gameplay state so you
//! can read what the character is doing. The poses are our own procedural ones, nothing from the
//! game.
//!
//! The player entity sits at hip height (the game's capsule is 1.8 m tall) and faces -Z; each
//! joint swings its limb, which hangs down from it. A positive pitch swings a limb forward.

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
/// body) and yaw (twist), radians; plus how far the body drops.
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
    /// Forward lean, smoothed so the body doesn't twitch with every speed change.
    lean: f32,
    /// Seconds since the ball left on a kick, and that kick's kind.
    follow: Option<(f32, Kick)>,
    last_kick: Option<Kick>,
    receive: f32,
}

pub fn spawn_body(commands: &mut Commands, player: Entity, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let shirt = materials.add(Color::srgb(0.92, 0.38, 0.16));
    let shorts = materials.add(Color::srgb(0.10, 0.12, 0.22));
    let skin = materials.add(Color::srgb(0.80, 0.62, 0.5));
    let boots = materials.add(Color::srgb(0.06, 0.06, 0.07));
    // A capsule hanging `len` metres below its joint.
    let mut limb = |radius: f32, len: f32| (Mesh3d(meshes.add(Capsule3d::new(radius, len - 2.0 * radius))), Transform::from_xyz(0.0, -len / 2.0, 0.0));
    let thigh = limb(0.1, 0.44);
    let shin = limb(0.08, 0.43);
    let upper_arm = limb(0.075, 0.32);
    let forearm = limb(0.065, 0.3);
    let joint = |part: Part, at: Vec3| (part, Transform::from_translation(at), Visibility::default());

    commands.entity(player).with_children(|root| {
        root.spawn(joint(Part::Body, Vec3::ZERO)).with_children(|body| {
            body.spawn((Mesh3d(meshes.add(Cuboid::new(0.4, 0.22, 0.26))), MeshMaterial3d(shorts.clone()), Transform::default()));
            for side in [-1.0f32, 1.0] {
                body.spawn(joint(Part::Thigh(side), Vec3::new(side * 0.12, -0.02, 0.0))).with_children(|hip| {
                    hip.spawn((thigh.0.clone(), MeshMaterial3d(shorts.clone()), thigh.1));
                    hip.spawn(joint(Part::Shin(side), Vec3::new(0.0, -0.43, 0.0))).with_children(|knee| {
                        knee.spawn((shin.0.clone(), MeshMaterial3d(skin.clone()), shin.1));
                        knee.spawn(joint(Part::Foot(side), Vec3::new(0.0, -0.4, 0.0))).with_children(|ankle| {
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

/// Poses every joint from the player's state: standing, the run and sprint stride (also backwards
/// and sideways), lock move's crouch, dribbles, dashes, receiving and kicks.
pub fn animate_body(
    time: Res<Time>,
    player: Single<(&Player, &Transform)>,
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
    let (fwd_share, side_share) = if speed > 0.1 { (fwd / speed, side / speed) } else { (1.0, 0.0) };

    // Stride: one cycle covers more ground the faster you go.
    gait.phase = (gait.phase + speed * dt / (1.2 + 0.45 * speed) * TAU) % TAU;
    let amp = (speed / 7.0).min(1.2);
    let lean_target = -(0.04 + 0.03 * speed.min(9.5)) * fwd_share.max(0.0);
    gait.lean += (lean_target - gait.lean) * (1.0 - (-6.0 * dt).exp());

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

    let (phase, lean) = (gait.phase, gait.lean);
    let swing = |s: f32| phase + if s > 0.0 { 0.0 } else { PI };
    let mut poses: Vec<(Part, Pose)> = all_parts()
        .map(|part| {
            let mut q = Pose::default();
            match part {
                Part::Body => q.drop = 0.02 * amp * (1.0 - (2.0 * phase).cos()),
                Part::Spine => q.pitch = lean,
                Part::Head => q.pitch = -0.6 * lean,
                Part::Thigh(s) => {
                    q.pitch = 0.7 * amp * swing(s).sin() * fwd_share;
                    q.roll = 0.3 * amp * swing(s).sin() * side_share * s;
                }
                // The knee bends as the leg swings through.
                Part::Shin(s) => q.pitch = -amp * (0.15 + swing(s).cos().max(0.0)),
                Part::Foot(_) => {}
                Part::UpperArm(s) => {
                    q.pitch = -0.6 * amp * swing(s).sin() * fwd_share;
                    q.roll = 0.1;
                }
                Part::Forearm(_) => q.pitch = 0.4 + 0.6 * amp.min(1.0),
            }
            (part, q)
        })
        .collect();
    let set = |poses: &mut Vec<(Part, Pose)>, part: Part, f: &dyn Fn(&mut Pose)| {
        if let Some((_, q)) = poses.iter_mut().find(|(p, _)| *p == part) {
            f(q);
        }
    };

    // Lock move: crouched, arms out, short steps.
    if p.locked {
        set(&mut poses, Part::Body, &|q| q.drop += 0.12);
        set(&mut poses, Part::Spine, &|q| q.pitch -= 0.3);
        set(&mut poses, Part::Head, &|q| q.pitch += 0.25);
        for s in [-1.0f32, 1.0] {
            set(&mut poses, Part::Thigh(s), &|q| q.pitch = q.pitch * 0.5 + 0.5);
            set(&mut poses, Part::Shin(s), &|q| q.pitch = q.pitch * 0.5 - 0.85);
            set(&mut poses, Part::Foot(s), &|q| q.pitch = 0.35);
            set(&mut poses, Part::UpperArm(s), &|q| {
                q.pitch = 0.3;
                q.roll = 0.5;
            });
            set(&mut poses, Part::Forearm(s), &|q| q.pitch = 0.5);
        }
    }
    // Dribble: lean into the move.
    if let Some(d) = p.dribble.filter(|d| d.moving()) {
        let dir = to_local * d.dir;
        set(&mut poses, Part::Body, &|q| {
            q.pitch = 0.2 * dir.z;
            q.roll = -0.25 * dir.x;
        });
    }
    // Dash: the body keeps facing where it faced and the move is directional, like the game's
    // front, side and back dash animations (DashAnimPicker's sectors).
    if let Some(d) = p.dash {
        set(&mut poses, Part::Body, &|q| q.drop += 0.08);
        match dash_sector(to_local * d.dir) {
            DashSector::Front => {
                set(&mut poses, Part::Spine, &|q| q.pitch = -0.45);
                set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = 0.8);
                set(&mut poses, Part::Thigh(-1.0), &|q| q.pitch = -0.5);
                set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -0.3);
                set(&mut poses, Part::Shin(-1.0), &|q| q.pitch = -0.8);
            }
            DashSector::Side(s) => {
                // Lean into the step, the leading leg reaching out, the other pushing off.
                set(&mut poses, Part::Body, &|q| q.roll = -0.3 * s);
                set(&mut poses, Part::Spine, &|q| q.pitch = -0.15);
                set(&mut poses, Part::Thigh(s), &|q| {
                    q.pitch = 0.0;
                    q.roll = 0.55;
                });
                set(&mut poses, Part::Thigh(-s), &|q| {
                    q.pitch = 0.0;
                    q.roll = -0.15;
                });
                for side in [-1.0f32, 1.0] {
                    set(&mut poses, Part::Shin(side), &|q| q.pitch = -0.4);
                    set(&mut poses, Part::UpperArm(side), &|q| q.roll = 0.4);
                }
            }
            DashSector::Back => {
                set(&mut poses, Part::Body, &|q| q.pitch = 0.25);
                set(&mut poses, Part::Spine, &|q| q.pitch = 0.1);
                set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = -0.5);
                set(&mut poses, Part::Thigh(-1.0), &|q| q.pitch = 0.3);
                for side in [-1.0f32, 1.0] {
                    set(&mut poses, Part::Shin(side), &|q| q.pitch = -0.6);
                    set(&mut poses, Part::UpperArm(side), &|q| q.pitch = 0.4);
                }
            }
        }
    }
    // Receiving: the right foot reaches for the ball.
    if p.receiving.is_some() {
        let reach = (gait.receive / CONTROL_SNAP).min(1.0);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch += (0.45 - q.pitch) * reach);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch += (-0.3 - q.pitch) * reach);
    }
    // Kicks: the right leg draws back while you charge, then swings through.
    if let Some(k) = p.kick {
        let wind = (k.elapsed / k.kind.buildup()).min(1.0);
        let shape = kick_shape(k.kind);
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = -shape.back * wind);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -shape.knee * wind);
        set(&mut poses, Part::Thigh(-1.0), &|q| q.pitch = 0.1 * wind);
        set(&mut poses, Part::Shin(-1.0), &|q| q.pitch = -0.3 * wind);
        set(&mut poses, Part::Spine, &|q| q.yaw = -0.25 * wind * shape.open);
        set(&mut poses, Part::UpperArm(-1.0), &|q| q.roll = 0.1 + 0.7 * wind * shape.open);
    } else if let Some((t, kind)) = gait.follow {
        let shape = kick_shape(kind);
        let u = t / shape.follow;
        // Through the ball in the first third, then settle.
        let through = if u < 0.33 { -shape.back + (shape.back + shape.peak) * (u / 0.33) } else { shape.peak * (1.0 - (u - 0.33) / 0.67) };
        set(&mut poses, Part::Thigh(1.0), &|q| q.pitch = through);
        set(&mut poses, Part::Shin(1.0), &|q| q.pitch = -0.15);
        set(&mut poses, Part::Spine, &|q| q.yaw = 0.2 * shape.open * (1.0 - u));
        set(&mut poses, Part::UpperArm(-1.0), &|q| q.roll = 0.1 + 0.6 * shape.open * (1.0 - u));
    }

    let blend = 1.0 - (-18.0 * dt).exp();
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

fn all_parts() -> impl Iterator<Item = Part> {
    [Part::Body, Part::Spine, Part::Head].into_iter().chain([-1.0f32, 1.0].into_iter().flat_map(|s| {
        [Part::UpperArm(s), Part::Forearm(s), Part::Thigh(s), Part::Shin(s), Part::Foot(s)]
    }))
}

/// How a kick looks: how far the leg draws back, the knee bend, how much the body opens up, how
/// high the leg finishes, and how long the follow-through lasts.
struct KickShape {
    back: f32,
    knee: f32,
    open: f32,
    peak: f32,
    follow: f32,
}

fn kick_shape(kind: Kick) -> KickShape {
    let (back, knee, open, peak, follow) = match kind {
        Kick::Shot => (1.0, 1.4, 1.0, 1.2, SHOT_FOLLOW_THROUGH + 0.2),
        Kick::Pass | Kick::ShortPass => (0.6, 0.9, 0.5, 0.8, 0.3),
        Kick::Lob | Kick::ShortLob => (0.75, 1.2, 0.6, 1.4, 0.4),
        Kick::PushBall | Kick::DribblePushBall => (0.35, 0.6, 0.2, 0.5, 0.2),
    };
    KickShape { back, knee, open, peak, follow }
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

    #[test]
    fn dash_sectors() {
        assert_eq!(dash_sector(Vec3::NEG_Z), DashSector::Front);
        assert_eq!(dash_sector(Vec3::X), DashSector::Side(1.0));
        assert_eq!(dash_sector(Vec3::NEG_X), DashSector::Side(-1.0));
        assert_eq!(dash_sector(Vec3::Z), DashSector::Back);
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
}
