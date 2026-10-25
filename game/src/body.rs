//! The player's body: our own mannequin at Rematch's sizes (its skeleton and physics shapes; no
//! mesh), posed by our own key poses on Rematch's animation structure (notes/animation.md).
//!
//! A cycle per mode and speed tier plays at a clamped rate with the feet in phase; changing cycle
//! crossfades; the lower body turns toward the velocity; the body leans into turns; kicks play on
//! top. There is no leg IK: the pelvis only drops (smoothly) toward where the lower foot is on the
//! ground.
//!
//! Pose angles are in degrees: x bends forward (for knees and elbows, it is the flexion; for feet,
//! toes down), y turns left, z rolls right (a limb moves toward +x, an upright part tilts right).

use std::f32::consts::{PI, TAU};
use std::sync::LazyLock;

use bevy::prelude::*;

use crate::fx::TrailPoint;
use crate::player::{FeintTurn, Heading, Kick, KickState, Part, Player, charge, contact_time, dribble_contact, roulette_turn, turn_out_angle, yaw_of};
use crate::tuning::*;

// Rematch's player (notes/animation.md §1), metres. The root entity is the capsule's centre.
const ROOT_HEIGHT: f32 = 0.9;
const PELVIS_HEIGHT: f32 = 1.12;
const HIP: Vec3 = Vec3::new(0.096, -0.051, 0.0);
const THIGH: f32 = 0.477;
const SHIN: f32 = 0.48;
/// Where the ankle sits with the legs straight: hips 1.069 m, less the legs.
const ANKLE_HEIGHT: f32 = PELVIS_HEIGHT + HIP.y - THIGH - SHIN;
const SPINE: f32 = 0.13;
const CHEST: f32 = 0.114;
const NECK: f32 = 0.207;
const HEAD: f32 = 0.118;
const SHOULDER: Vec3 = Vec3::new(0.155, 0.154, 0.0);
const UPPER_ARM: f32 = 0.267;
const FOREARM: f32 = 0.259;

// Joints, in pose order.
const PELVIS: usize = 0;
const SPINE_J: usize = 1;
const CHEST_J: usize = 2;
const NECK_J: usize = 3;
const HEAD_J: usize = 4;
const THIGH_L: usize = 5;
const SHIN_L: usize = 6;
const FOOT_L: usize = 7;
const THIGH_R: usize = 8;
const SHIN_R: usize = 9;
const FOOT_R: usize = 10;
const UPPER_L: usize = 11;
const FORE_L: usize = 12;
const HAND_L: usize = 13;
const UPPER_R: usize = 14;
const FORE_R: usize = 15;
const HAND_R: usize = 16;
const JOINTS: usize = 17;
const PAIRS: [(usize, usize); 6] = [(THIGH_L, THIGH_R), (SHIN_L, SHIN_R), (FOOT_L, FOOT_R), (UPPER_L, UPPER_R), (FORE_L, FORE_R), (HAND_L, HAND_R)];

#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Joint(usize);

#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Pose([Vec3; JOINTS]);

impl Pose {
    fn with(mut self, set: &[(usize, [f32; 3])]) -> Pose {
        for &(j, [x, y, z]) in set {
            self.0[j] = Vec3::new(x, y, z);
        }
        self
    }

    /// Adds `delta` joint by joint.
    fn plus(mut self, delta: &Pose) -> Pose {
        for j in 0..JOINTS {
            self.0[j] += delta.0[j];
        }
        self
    }

    /// The same pose on the other side: left and right swapped, turns and rolls reversed.
    fn mirror(self) -> Pose {
        let mut m = self.0.map(|a| Vec3::new(a.x, -a.y, -a.z));
        for (l, r) in PAIRS {
            m.swap(l, r);
        }
        Pose(m)
    }

    fn lerp(self, other: &Pose, t: f32) -> Pose {
        let mut out = self;
        for j in 0..JOINTS {
            out.0[j] = self.0[j].lerp(other.0[j], t);
        }
        out
    }
}

/// A joint's local rotation from its pose angles.
fn joint_rotation(j: usize, a: Vec3) -> Quat {
    let (x, y, z) = (a.x.to_radians(), a.y.to_radians(), a.z.to_radians());
    let upright = j <= HEAD_J;
    let pitch = match j {
        // Upright parts bend forward, knees and feet flex backward, the rest swings forward.
        PELVIS | SPINE_J | CHEST_J | NECK_J | HEAD_J | SHIN_L | SHIN_R | FOOT_L | FOOT_R => -x,
        _ => x,
    };
    Quat::from_rotation_y(y) * Quat::from_rotation_z(if upright { -z } else { z }) * Quat::from_rotation_x(pitch)
}

/// The ankles, from the pelvis joint (in the pelvis's parent frame).
fn ankles(p: &Pose) -> [Vec3; 2] {
    let pelvis = joint_rotation(PELVIS, p.0[PELVIS]);
    [(THIGH_L, SHIN_L, -1.0), (THIGH_R, SHIN_R, 1.0)].map(|(t, s, side)| {
        let thigh = joint_rotation(t, p.0[t]);
        let shin = joint_rotation(s, p.0[s]);
        pelvis * (Vec3::new(side * HIP.x, HIP.y, HIP.z) + thigh * (Vec3::NEG_Y * THIGH + shin * (Vec3::NEG_Y * SHIN)))
    })
}

/// How far the pelvis drops so the lower ankle is at its standing height.
fn ground_drop(p: &Pose) -> f32 {
    let [l, r] = ankles(p);
    (PELVIS_HEIGHT + l.y.min(r.y) - ANKLE_HEIGHT).max(0.0)
}

/// A looping cycle: four key poses, at 0, ¼, ½ and ¾ of the cycle (left foot down at 0, right at
/// ½), the speed it is made for and its length.
#[derive(Clone, Copy)]
struct Clip {
    keys: [Pose; 4],
    speed: f32,
    duration: f32,
    /// Height (m) the body rises between steps: the run's flight.
    flight: f32,
}

impl Clip {
    /// A cycle from its first half (left contact, left mid-stance); the second half mirrors it.
    fn cycle(contact: Pose, stance: Pose, speed: f32, duration: f32, flight: f32) -> Clip {
        Clip { keys: [contact, stance, contact.mirror(), stance.mirror()], speed, duration, flight }
    }

    fn held(pose: Pose) -> Clip {
        Clip { keys: [pose; 4], speed: 0.0, duration: 4.0, flight: 0.0 }
    }

    fn mirror(&self) -> Clip {
        Clip { keys: self.keys.map(Pose::mirror), ..*self }
    }

    /// The pose at `phase` (0..1), smoothly through the keys (Catmull-Rom).
    fn sample(&self, phase: f32) -> Pose {
        let u = phase.rem_euclid(1.0) * 4.0;
        let i = (u.floor() as usize).min(3);
        let t = u - i as f32;
        let k = |n: usize| &self.keys[(i + n) % 4];
        let (p0, p1, p2, p3) = (k(3), k(0), k(1), k(2));
        let mut out = Pose::default();
        for j in 0..JOINTS {
            let (a, b, c, d) = (p0.0[j], p1.0[j], p2.0[j], p3.0[j]);
            out.0[j] = 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t + (3.0 * b - a - 3.0 * c + d) * t * t * t);
        }
        out
    }

    /// The body's rise at `phase`: a bump between each mid-stance and the next contact.
    fn lift(&self, phase: f32) -> f32 {
        self.flight * (TAU * 2.0 * (phase - 0.375)).cos().max(0.0).powi(2)
    }
}

/// A one-shot clip: key poses at points of its progress (0..1), eased between.
struct OneShot(Vec<(f32, Pose)>);

impl OneShot {
    fn sample(&self, u: f32) -> Pose {
        let keys = &self.0;
        let i = keys.iter().rposition(|(at, _)| *at <= u).unwrap_or(0).min(keys.len() - 2);
        let ((a, from), (b, to)) = (keys[i], keys[i + 1]);
        let t = ((u - a) / (b - a)).clamp(0.0, 1.0);
        from.lerp(&to, t * t * (3.0 - 2.0 * t))
    }
}

/// The transitions (notes/animation.md §2): starts, stops and turns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Move {
    Start,
    Stop,
    Turn,
}

#[derive(Clone, Copy, Debug)]
struct Transition {
    kind: Move,
    /// Authored for the left foot (starts, stops) or a right turn; mirrored for the other.
    mirror: bool,
    /// How much of the authored pose: smaller turns and slower stops use less.
    size: f32,
    t: f32,
    u: f32,
    /// The speed a stop started from.
    entry: f32,
}

/// Time a start takes to reach the run, and the shortest stop.
const START_TIME: f32 = 0.35;
/// The fastest any joint may turn, degrees per second (a kick's strike needs about 2000).
const MAX_JOINT_SPEED: f32 = 2400.0;
const STOP_TIME: f32 = 0.45;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cycle {
    Idle,
    Walk,
    Run,
    Sprint,
    Exhausted,
    ExtraEffort,
    LockIdle,
    LockForward,
    LockRight,
    LockLeft,
    LockBack,
}

struct Clips {
    stand: Pose,
    /// Kicks (the right foot): wind-up, contact, follow-through, for a shot, a pass (the inside of
    /// the foot) and a lob (under the ball, leaning back).
    kicks: [[Pose; 3]; 3],
    shot_side: [Pose; 3],
    shot_back: [Pose; 3],
    heel: [Pose; 3],
    /// The rainbow flick: ahead, and across the body to the left (mirrored for the right).
    flick: [Pose; 3],
    flick_side: [Pose; 3],
    /// Volley touches on a ball in the air, the right side's (mirrored for a ball on the left): the
    /// foot, the thigh, the stomach, the chest and the head; and a jumping header.
    touch: [[Pose; 3]; 5],
    jump_head: [Pose; 3],
    /// Landing out of a kick (the right foot), after its follow-through.
    land: Pose,
    start: OneShot,
    stop: OneShot,
    turn: OneShot,
    /// Controlling the ball, added on top of whatever runs (and each touch while you carry it).
    control: Pose,
    /// The moves, each with where its touch sits on its timeline: dribbles (a fast cut, side,
    /// back, a feint) and dashes (front, side, back). Authored going right or with the right foot.
    dribble_forward: (OneShot, f32),
    dribble_side: (OneShot, f32),
    dribble_back: (OneShot, f32),
    feint: (OneShot, f32),
    roulette: (OneShot, f32),
    /// The forward feint out of a fast cut: a quarter turn out and back in a deep crouch.
    turn_out: (OneShot, f32),
    /// Lock move's stance, and with the ball the right sole on it.
    lock: Pose,
    sole: Pose,
    /// Lock move's braking crouch as you turn to go back, added on top.
    brake: Pose,
    dash_front: OneShot,
    dash_side: OneShot,
    dash_back: OneShot,
    cycles: Vec<(Cycle, Clip)>,
}

impl Clips {
    fn get(&self, c: Cycle) -> &Clip {
        &self.cycles.iter().find(|(k, _)| *k == c).expect("every cycle is authored").1
    }
}

/// Our own key poses (notes/animation.md §3).
static CLIPS: LazyLock<Clips> = LazyLock::new(|| {
    let stand = Pose::default().with(&[
        (SPINE_J, [3.0, 0.0, 0.0]),
        (CHEST_J, [2.0, 0.0, 0.0]),
        (NECK_J, [-2.0, 0.0, 0.0]),
        (THIGH_L, [2.0, 0.0, -3.0]),
        (SHIN_L, [5.0, 0.0, 0.0]),
        (FOOT_L, [-3.0, 0.0, 3.0]),
        (THIGH_R, [2.0, 0.0, 3.0]),
        (SHIN_R, [5.0, 0.0, 0.0]),
        (FOOT_R, [-3.0, 0.0, -3.0]),
        (UPPER_L, [5.0, 0.0, -8.0]),
        (FORE_L, [15.0, 0.0, 0.0]),
        (HAND_L, [5.0, 0.0, 0.0]),
        (UPPER_R, [5.0, 0.0, 8.0]),
        (FORE_R, [15.0, 0.0, 0.0]),
        (HAND_R, [5.0, 0.0, 0.0]),
    ]);
    // Feet: toes down = thigh − knee keeps the sole flat; more points the toes down.
    let walk = Clip::cycle(
        stand.with(&[
            (PELVIS, [0.0, -5.0, 0.0]),
            (SPINE_J, [4.0, 0.0, 0.0]),
            (CHEST_J, [2.0, 6.0, 0.0]),
            (THIGH_L, [22.0, 0.0, -2.0]),
            (SHIN_L, [4.0, 0.0, 0.0]),
            (FOOT_L, [6.0, 0.0, 0.0]),
            (THIGH_R, [-14.0, 0.0, 2.0]),
            (SHIN_R, [22.0, 0.0, 0.0]),
            (FOOT_R, [-16.0, 0.0, 0.0]),
            (UPPER_L, [-16.0, 0.0, -8.0]),
            (FORE_L, [15.0, 0.0, 0.0]),
            (UPPER_R, [18.0, 0.0, 8.0]),
            (FORE_R, [25.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 0.0, -2.0]),
            (SPINE_J, [4.0, 0.0, 0.0]),
            (THIGH_L, [2.0, 0.0, -2.0]),
            (SHIN_L, [8.0, 0.0, 0.0]),
            (FOOT_L, [-6.0, 0.0, 0.0]),
            (THIGH_R, [14.0, 0.0, 2.0]),
            (SHIN_R, [45.0, 0.0, 0.0]),
            (FOOT_R, [-26.0, 0.0, 0.0]),
            (UPPER_L, [-2.0, 0.0, -8.0]),
            (UPPER_R, [4.0, 0.0, 8.0]),
            (FORE_R, [20.0, 0.0, 0.0]),
        ]),
        1.6,
        1.1,
        0.0,
    );
    let run_contact = stand.with(&[
        (PELVIS, [0.0, -8.0, 0.0]),
        (SPINE_J, [8.0, 0.0, 0.0]),
        (CHEST_J, [3.0, 12.0, 0.0]),
        (NECK_J, [-6.0, -4.0, 0.0]),
        (THIGH_L, [30.0, 0.0, -2.0]),
        (SHIN_L, [18.0, 0.0, 0.0]),
        (FOOT_L, [4.0, 0.0, 0.0]),
        (THIGH_R, [-24.0, 0.0, 2.0]),
        (SHIN_R, [80.0, 0.0, 0.0]),
        (FOOT_R, [-79.0, 0.0, 0.0]),
        (UPPER_L, [-35.0, 0.0, -10.0]),
        (FORE_L, [75.0, 0.0, 0.0]),
        (UPPER_R, [40.0, 0.0, 10.0]),
        (FORE_R, [95.0, 0.0, 0.0]),
    ]);
    let run_stance = stand.with(&[
        (PELVIS, [0.0, 0.0, -2.0]),
        (SPINE_J, [9.0, 0.0, 0.0]),
        (CHEST_J, [3.0, 0.0, 0.0]),
        (NECK_J, [-7.0, 0.0, 0.0]),
        (THIGH_L, [0.0, 0.0, -2.0]),
        (SHIN_L, [32.0, 0.0, 0.0]),
        (FOOT_L, [-32.0, 0.0, 0.0]),
        (THIGH_R, [25.0, 0.0, 2.0]),
        (SHIN_R, [115.0, 0.0, 0.0]),
        (FOOT_R, [-75.0, 0.0, 0.0]),
        (UPPER_L, [-6.0, 0.0, -10.0]),
        (FORE_L, [80.0, 0.0, 0.0]),
        (UPPER_R, [8.0, 0.0, 10.0]),
        (FORE_R, [90.0, 0.0, 0.0]),
    ]);
    let run = Clip::cycle(run_contact, run_stance, RUN_SPEED, 0.68, 0.0);
    // Starts and turns end on the run's last stretch into a left contact, so the cycle picks up
    // in step; mirrored, into a right one.
    let (into_contact, contact) = (run.sample(0.85), run.sample(0.0));
    // Start, the left foot first: lean and load, push off the right, then into the run.
    let start = OneShot(vec![
        (0.0, stand.with(&[
            (SPINE_J, [12.0, 0.0, 0.0]),
            (CHEST_J, [3.0, 0.0, 0.0]),
            (NECK_J, [-8.0, 0.0, 0.0]),
            (THIGH_L, [6.0, 0.0, -3.0]),
            (SHIN_L, [22.0, 0.0, 0.0]),
            (FOOT_L, [-16.0, 0.0, 3.0]),
            (THIGH_R, [-6.0, 0.0, 3.0]),
            (SHIN_R, [18.0, 0.0, 0.0]),
            (FOOT_R, [-14.0, 0.0, -3.0]),
            (UPPER_L, [-15.0, 0.0, -10.0]),
            (FORE_L, [50.0, 0.0, 0.0]),
            (UPPER_R, [20.0, 0.0, 10.0]),
            (FORE_R, [60.0, 0.0, 0.0]),
        ])),
        (0.45, stand.with(&[
            (SPINE_J, [14.0, 0.0, 0.0]),
            (CHEST_J, [3.0, 8.0, 0.0]),
            (NECK_J, [-10.0, 0.0, 0.0]),
            (THIGH_L, [38.0, 0.0, -2.0]),
            (SHIN_L, [75.0, 0.0, 0.0]),
            (FOOT_L, [-30.0, 0.0, 0.0]),
            (THIGH_R, [-22.0, 0.0, 2.0]),
            (SHIN_R, [20.0, 0.0, 0.0]),
            (FOOT_R, [-17.0, 0.0, 0.0]),
            (UPPER_L, [-35.0, 0.0, -10.0]),
            (FORE_L, [70.0, 0.0, 0.0]),
            (UPPER_R, [40.0, 0.0, 10.0]),
            (FORE_R, [90.0, 0.0, 0.0]),
        ])),
        (0.7, into_contact),
        (1.0, contact),
    ]);
    // Stop on the left foot: plant it ahead and brake, the right comes alongside, then stand.
    let stop = OneShot(vec![
        (0.0, stand.with(&[
            (PELVIS, [0.0, -10.0, 0.0]),
            (SPINE_J, [-2.0, 0.0, 0.0]),
            (CHEST_J, [0.0, 6.0, 0.0]),
            (THIGH_L, [32.0, 0.0, -3.0]),
            (SHIN_L, [12.0, 0.0, 0.0]),
            (FOOT_L, [8.0, 0.0, 3.0]),
            (THIGH_R, [-14.0, 0.0, 3.0]),
            (SHIN_R, [45.0, 0.0, 0.0]),
            (FOOT_R, [-39.0, 0.0, -3.0]),
            (UPPER_L, [25.0, 0.0, -14.0]),
            (FORE_L, [45.0, 0.0, 0.0]),
            (UPPER_R, [30.0, 0.0, 14.0]),
            (FORE_R, [50.0, 0.0, 0.0]),
        ])),
        (0.55, stand.with(&[
            (SPINE_J, [4.0, 0.0, 0.0]),
            (THIGH_L, [14.0, 0.0, -3.0]),
            (SHIN_L, [22.0, 0.0, 0.0]),
            (FOOT_L, [-8.0, 0.0, 3.0]),
            (THIGH_R, [6.0, 0.0, 3.0]),
            (SHIN_R, [30.0, 0.0, 0.0]),
            (FOOT_R, [-24.0, 0.0, -3.0]),
            (UPPER_L, [10.0, 0.0, -10.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [10.0, 0.0, 10.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ])),
        (1.0, stand),
    ]);
    // Turn right: plant the outside (left) foot and lean in through the brake, pivot the hips at
    // its end, then run out. The brake's end sits at 0.45 (see `turn_progress`).
    let turn = OneShot(vec![
        (0.0, stand.with(&[
            (PELVIS, [0.0, -12.0, 6.0]),
            (SPINE_J, [-4.0, 0.0, 6.0]),
            (CHEST_J, [0.0, -10.0, 4.0]),
            (NECK_J, [0.0, -15.0, 0.0]),
            (THIGH_L, [26.0, 0.0, -14.0]),
            (SHIN_L, [22.0, 0.0, 0.0]),
            (FOOT_L, [4.0, 0.0, 14.0]),
            (THIGH_R, [-12.0, 0.0, 6.0]),
            (SHIN_R, [50.0, 0.0, 0.0]),
            (FOOT_R, [-42.0, 0.0, -6.0]),
            (UPPER_L, [35.0, 0.0, -30.0]),
            (FORE_L, [60.0, 0.0, 0.0]),
            (UPPER_R, [-20.0, 0.0, 25.0]),
            (FORE_R, [50.0, 0.0, 0.0]),
        ])),
        (0.45, stand.with(&[
            (PELVIS, [0.0, -40.0, 6.0]),
            (SPINE_J, [6.0, 10.0, 6.0]),
            (CHEST_J, [2.0, 5.0, 4.0]),
            (NECK_J, [-4.0, -10.0, 0.0]),
            (THIGH_L, [4.0, 0.0, -8.0]),
            (SHIN_L, [38.0, 0.0, 0.0]),
            (FOOT_L, [-34.0, 0.0, 8.0]),
            (THIGH_R, [35.0, 0.0, 12.0]),
            (SHIN_R, [80.0, 0.0, 0.0]),
            (FOOT_R, [-30.0, 0.0, -12.0]),
            (UPPER_L, [20.0, 0.0, -35.0]),
            (FORE_L, [70.0, 0.0, 0.0]),
            (UPPER_R, [10.0, 0.0, 20.0]),
            (FORE_R, [70.0, 0.0, 0.0]),
        ])),
        (0.7, into_contact),
        (1.0, contact),
    ]);
    let sprint_contact = stand.with(&[
        (PELVIS, [0.0, -10.0, 0.0]),
        (SPINE_J, [14.0, 0.0, 0.0]),
        (CHEST_J, [4.0, 14.0, 0.0]),
        (NECK_J, [-12.0, -5.0, 0.0]),
        (THIGH_L, [34.0, 0.0, -2.0]),
        (SHIN_L, [20.0, 0.0, 0.0]),
        (FOOT_L, [8.0, 0.0, 0.0]),
        (THIGH_R, [-30.0, 0.0, 2.0]),
        (SHIN_R, [95.0, 0.0, 0.0]),
        (FOOT_R, [-95.0, 0.0, 0.0]),
        (UPPER_L, [-50.0, 0.0, -10.0]),
        (FORE_L, [80.0, 0.0, 0.0]),
        (UPPER_R, [55.0, 0.0, 10.0]),
        (FORE_R, [100.0, 0.0, 0.0]),
    ]);
    let sprint_stance = stand.with(&[
        (PELVIS, [0.0, 0.0, -2.0]),
        (SPINE_J, [15.0, 0.0, 0.0]),
        (CHEST_J, [4.0, 0.0, 0.0]),
        (NECK_J, [-13.0, 0.0, 0.0]),
        (THIGH_L, [-2.0, 0.0, -2.0]),
        (SHIN_L, [36.0, 0.0, 0.0]),
        (FOOT_L, [-38.0, 0.0, 0.0]),
        (THIGH_R, [35.0, 0.0, 2.0]),
        (SHIN_R, [125.0, 0.0, 0.0]),
        (FOOT_R, [-70.0, 0.0, 0.0]),
        (UPPER_L, [-10.0, 0.0, -10.0]),
        (FORE_L, [85.0, 0.0, 0.0]),
        (UPPER_R, [10.0, 0.0, 10.0]),
        (FORE_R, [95.0, 0.0, 0.0]),
    ]);
    let sprint = Clip::cycle(sprint_contact, sprint_stance, SPRINT_SPEED, 0.6, 0.015);
    // Extra effort in Kev's practice recording: the trunk leans 35-45 deg forward, the strides are
    // long and bounding with the back leg pushed out straight, the head stays up and the arms pump
    // hard. The pelvis pitches forward and the thighs are set against it, so the legs keep their
    // swing; the lean is the pelvis, spine and chest together (40 deg).
    let ee_lean = 14.0;
    let ee_contact = stand.with(&[
        (PELVIS, [ee_lean, -12.0, 0.0]),
        (SPINE_J, [20.0, 0.0, 0.0]),
        (CHEST_J, [6.0, 14.0, 0.0]),
        (NECK_J, [-30.0, -5.0, 0.0]),
        (HEAD_J, [-6.0, 0.0, 0.0]),
        (THIGH_L, [42.0 + ee_lean, 0.0, -2.0]),
        (SHIN_L, [24.0, 0.0, 0.0]),
        (FOOT_L, [12.0, 0.0, 0.0]),
        (THIGH_R, [-38.0 + ee_lean, 0.0, 2.0]),
        (SHIN_R, [70.0, 0.0, 0.0]),
        (FOOT_R, [-80.0, 0.0, 0.0]),
        (UPPER_L, [-65.0, 0.0, -12.0]),
        (FORE_L, [85.0, 0.0, 0.0]),
        (UPPER_R, [70.0, 0.0, 12.0]),
        (FORE_R, [100.0, 0.0, 0.0]),
    ]);
    // The drive: the support leg pushed out straight behind, the other knee driven high.
    let ee_drive = stand.with(&[
        (PELVIS, [ee_lean, 0.0, -2.0]),
        (SPINE_J, [20.0, 0.0, 0.0]),
        (CHEST_J, [6.0, 0.0, 0.0]),
        (NECK_J, [-30.0, 0.0, 0.0]),
        (HEAD_J, [-6.0, 0.0, 0.0]),
        (THIGH_L, [-24.0 + ee_lean, 0.0, -2.0]),
        (SHIN_L, [10.0, 0.0, 0.0]),
        (FOOT_L, [-5.0, 0.0, 0.0]),
        (THIGH_R, [58.0 + ee_lean, 0.0, 2.0]),
        (SHIN_R, [110.0, 0.0, 0.0]),
        (FOOT_R, [-40.0, 0.0, 0.0]),
        (UPPER_L, [-15.0, 0.0, -12.0]),
        (FORE_L, [90.0, 0.0, 0.0]),
        (UPPER_R, [20.0, 0.0, 12.0]),
        (FORE_R, [95.0, 0.0, 0.0]),
    ]);
    let extra = Clip::cycle(ee_contact, ee_drive, EXTRA_EFFORT_SPEED, 0.6, 0.05);
    // Tired: the run's shape at a sprint, arms low, head down.
    let tired = Pose::default().with(&[(SPINE_J, [5.0, 0.0, 0.0]), (NECK_J, [8.0, 0.0, 0.0]), (HEAD_J, [6.0, 0.0, 0.0]), (FORE_L, [-30.0, 0.0, 0.0]), (FORE_R, [-30.0, 0.0, 0.0])]);
    let exhausted = Clip::cycle(run_contact.plus(&tired), run_stance.plus(&tired), SPRINT_SPEED * 0.8, 0.66, 0.0);

    // Lock move: a low, wide stance, arms out, head up.
    let stance = stand.plus(&Pose::default().with(&[
        (SPINE_J, [14.0, 0.0, 0.0]),
        (CHEST_J, [4.0, 0.0, 0.0]),
        (NECK_J, [-12.0, 0.0, 0.0]),
        (HEAD_J, [-6.0, 0.0, 0.0]),
        (THIGH_L, [14.0, 0.0, -6.0]),
        (SHIN_L, [28.0, 0.0, 0.0]),
        (FOOT_L, [-14.0, 0.0, 6.0]),
        (THIGH_R, [14.0, 0.0, 6.0]),
        (SHIN_R, [28.0, 0.0, 0.0]),
        (FOOT_R, [-14.0, 0.0, -6.0]),
        (UPPER_L, [20.0, 0.0, -24.0]),
        (FORE_L, [35.0, 0.0, 0.0]),
        (UPPER_R, [20.0, 0.0, 24.0]),
        (FORE_R, [35.0, 0.0, 0.0]),
    ]));
    // Lock move itself (Kev's practice recording): lower and much wider than the moves' stance,
    // the ankles about 0.9 m apart and the knees bent twice as far (standing about 0.25 m lower),
    // the torso a little forward, the arms out about 40° with the elbows bent and the hands at the
    // hips. Every lock pose keeps the planted legs about this long, so the body stays level.
    let lock = stand.with(&[
        (SPINE_J, [18.0, 0.0, 0.0]),
        (CHEST_J, [6.0, 0.0, 0.0]),
        (NECK_J, [-16.0, 0.0, 0.0]),
        (HEAD_J, [-8.0, 0.0, 0.0]),
        (THIGH_L, [32.0, 0.0, -26.0]),
        (SHIN_L, [65.0, 0.0, 0.0]),
        (FOOT_L, [-33.0, 0.0, 26.0]),
        (THIGH_R, [32.0, 0.0, 26.0]),
        (SHIN_R, [65.0, 0.0, 0.0]),
        (FOOT_R, [-33.0, 0.0, -26.0]),
        (UPPER_L, [17.0, 0.0, -40.0]),
        (FORE_L, [60.0, 0.0, 0.0]),
        (UPPER_R, [17.0, 0.0, 40.0]),
        (FORE_R, [60.0, 0.0, 0.0]),
    ]);
    let low = |p: Pose| Pose(std::array::from_fn(|j| if j >= UPPER_L { lock.0[j] } else { p.0[j] }));
    // Forward and back are a little more upright than the stance, on short steps a hip-width apart.
    // Each cycle is two steps, at the recorded rate at full lock speed (back is 0.9 of it).
    let lock_forward = Clip::cycle(
        low(lock.with(&[(THIGH_L, [38.0, 0.0, -12.0]), (SHIN_L, [44.0, 0.0, 0.0]), (FOOT_L, [-4.0, 0.0, 12.0]), (THIGH_R, [-2.0, 0.0, 12.0]), (SHIN_R, [84.0, 0.0, 0.0]), (FOOT_R, [-62.0, 0.0, -12.0])])),
        low(lock.with(&[(THIGH_L, [18.0, 0.0, -12.0]), (SHIN_L, [54.0, 0.0, 0.0]), (FOOT_L, [-36.0, 0.0, 12.0]), (THIGH_R, [36.0, 0.0, 12.0]), (SHIN_R, [104.0, 0.0, 0.0]), (FOOT_R, [-56.0, 0.0, -12.0])])),
        LOCK_SPEED,
        2.0 / LOCK_FORWARD_STEPS,
        0.0,
    );
    let lock_back = Clip::cycle(
        low(lock.with(&[(THIGH_L, [0.0, 0.0, -14.0]), (SHIN_L, [54.0, 0.0, 0.0]), (FOOT_L, [-44.0, 0.0, 14.0]), (THIGH_R, [32.0, 0.0, 14.0]), (SHIN_R, [58.0, 0.0, 0.0]), (FOOT_R, [-26.0, 0.0, -14.0])])),
        low(lock.with(&[(THIGH_L, [20.0, 0.0, -14.0]), (SHIN_L, [50.0, 0.0, 0.0]), (FOOT_L, [-30.0, 0.0, 14.0]), (THIGH_R, [10.0, 0.0, 14.0]), (SHIN_R, [94.0, 0.0, 0.0]), (FOOT_R, [-58.0, 0.0, -14.0])])),
        LOCK_SPEED * LOCK_SPEED_BY_ANGLE[2].1,
        2.0 / LOCK_BACK_STEPS,
        0.0,
    );
    // Strafing right (the game's side cycle runs 44f; Kev found four scissor steps in it too
    // busy): one stride per cycle, a step-together shuffle, 2.4 a second at full speed as in Kev's
    // recording (the feet glide a little, as there). The right foot lifts and reaches out with
    // the weight on the left, lands wide (about 1 m), the left lifts and comes in, and you land again
    // about 0.4 m apart: never feet together, never crossing. The narrower the feet, the deeper the
    // knees, so the hips stay level.
    let reach = lock.with(&[
        (PELVIS, [0.0, 0.0, 4.0]),
        (THIGH_L, [38.0, 0.0, -16.0]),
        (SHIN_L, [77.0, 0.0, 0.0]),
        (FOOT_L, [-39.0, 0.0, 16.0]),
        (THIGH_R, [52.0, 0.0, 34.0]),
        (SHIN_R, [92.0, 0.0, 0.0]),
        (FOOT_R, [-34.0, 0.0, -34.0]),
    ]);
    let wide = lock.with(&[
        (THIGH_L, [30.0, 0.0, -30.0]),
        (SHIN_L, [60.0, 0.0, 0.0]),
        (FOOT_L, [-30.0, 0.0, 30.0]),
        (THIGH_R, [30.0, 0.0, 30.0]),
        (SHIN_R, [60.0, 0.0, 0.0]),
        (FOOT_R, [-30.0, 0.0, -30.0]),
    ]);
    let gather = lock.with(&[
        (PELVIS, [0.0, 0.0, -4.0]),
        (THIGH_L, [52.0, 0.0, -3.0]),
        (SHIN_L, [106.0, 0.0, 0.0]),
        (FOOT_L, [-44.0, 0.0, 3.0]),
        (THIGH_R, [36.0, 0.0, 22.0]),
        (SHIN_R, [71.0, 0.0, 0.0]),
        (FOOT_R, [-35.0, 0.0, -22.0]),
    ]);
    let narrow = lock.with(&[
        (THIGH_L, [40.0, 0.0, -9.0]),
        (SHIN_L, [81.0, 0.0, 0.0]),
        (FOOT_L, [-41.0, 0.0, 9.0]),
        (THIGH_R, [40.0, 0.0, 9.0]),
        (SHIN_R, [81.0, 0.0, 0.0]),
        (FOOT_R, [-41.0, 0.0, -9.0]),
    ]);
    let lock_right = Clip { keys: [reach, wide, gather, narrow], speed: LOCK_SPEED, duration: 1.0 / LOCK_SIDE_SHUFFLES, flight: 0.015 };
    // Turning to go back (Kev's recording): you sink much deeper, the hips swing round to the left
    // and the shoulders lean over the knees, then come up into the backpedal. Added on top.
    let brake = Pose::default().with(&[
        (PELVIS, [0.0, 22.0, 0.0]),
        (SPINE_J, [12.0, -6.0, 0.0]),
        (CHEST_J, [4.0, -8.0, 0.0]),
        (NECK_J, [-10.0, 0.0, 0.0]),
        (THIGH_L, [16.0, 0.0, 0.0]),
        (SHIN_L, [34.0, 0.0, 0.0]),
        (FOOT_L, [-18.0, 0.0, 0.0]),
        (THIGH_R, [16.0, 0.0, 0.0]),
        (SHIN_R, [34.0, 0.0, 0.0]),
        (FOOT_R, [-18.0, 0.0, 0.0]),
        (UPPER_L, [10.0, 0.0, -8.0]),
        (FORE_L, [10.0, 0.0, 0.0]),
        (UPPER_R, [10.0, 0.0, 8.0]),
        (FORE_R, [10.0, 0.0, 0.0]),
    ]);

    // A shot (the right foot): the leg cocks far back with the hips turned away and the left arm
    // thrown out; the hips whip round through the ball with the body over it; the leg swings on up
    // as the plant foot rises onto its toes.
    let shot_kick = [
        stand.with(&[
            (PELVIS, [0.0, -28.0, -6.0]),
            (SPINE_J, [26.0, 0.0, -8.0]),
            (CHEST_J, [6.0, 14.0, -4.0]),
            (NECK_J, [-18.0, 6.0, 0.0]),
            (THIGH_L, [32.0, 0.0, -8.0]),
            (SHIN_L, [62.0, 0.0, 0.0]),
            (FOOT_L, [-30.0, 0.0, 6.0]),
            (THIGH_R, [-38.0, 0.0, 10.0]),
            (SHIN_R, [118.0, 0.0, 0.0]),
            (FOOT_R, [-70.0, 0.0, 0.0]),
            // Both arms out wide at shoulder height while it charges (Kev's practice recording).
            (UPPER_L, [10.0, 0.0, -85.0]),
            (FORE_L, [15.0, 0.0, 0.0]),
            (UPPER_R, [-15.0, 0.0, 85.0]),
            (FORE_R, [15.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 12.0, 0.0]),
            (SPINE_J, [22.0, 0.0, -6.0]),
            (CHEST_J, [4.0, -14.0, 0.0]),
            (NECK_J, [-10.0, 0.0, 0.0]),
            (THIGH_L, [26.0, 0.0, -6.0]),
            (SHIN_L, [50.0, 0.0, 0.0]),
            (FOOT_L, [-26.0, 0.0, 6.0]),
            (THIGH_R, [28.0, 0.0, 2.0]),
            (SHIN_R, [16.0, 0.0, 0.0]),
            (FOOT_R, [55.0, 0.0, 0.0]),
            (UPPER_L, [35.0, 0.0, -80.0]),
            (FORE_L, [20.0, 0.0, 0.0]),
            (UPPER_R, [-45.0, 0.0, 35.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 28.0, 4.0]),
            (SPINE_J, [-12.0, 0.0, -4.0]),
            (CHEST_J, [0.0, -16.0, 0.0]),
            (NECK_J, [-6.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -4.0]),
            (SHIN_L, [30.0, 0.0, 0.0]),
            (FOOT_L, [14.0, 0.0, 4.0]),
            (THIGH_R, [100.0, 0.0, -6.0]),
            (SHIN_R, [45.0, 0.0, 0.0]),
            (FOOT_R, [50.0, 0.0, 0.0]),
            (UPPER_L, [55.0, 0.0, -45.0]),
            (FORE_L, [40.0, 0.0, 0.0]),
            (UPPER_R, [-35.0, 0.0, 30.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
    ];

    // A shot across the body to the left (the right foot; mirrored, it is the left foot to the
    // right, as the game mirrors its side clip): the leg cocks back and out with the hips open to
    // the right, swings across turning the hips through, and follows on high across to the left.
    let shot_side = [
        stand.with(&[
            (PELVIS, [0.0, -25.0, -4.0]),
            (SPINE_J, [10.0, 0.0, -6.0]),
            (CHEST_J, [4.0, 10.0, -4.0]),
            (NECK_J, [-6.0, 10.0, 0.0]),
            (THIGH_L, [18.0, 0.0, -8.0]),
            (SHIN_L, [42.0, 0.0, 0.0]),
            (FOOT_L, [-24.0, 0.0, 8.0]),
            (THIGH_R, [-35.0, -10.0, 18.0]),
            (SHIN_R, [95.0, 0.0, 0.0]),
            (FOOT_R, [-70.0, 0.0, -10.0]),
            (UPPER_L, [40.0, 0.0, -70.0]),
            (FORE_L, [25.0, 0.0, 0.0]),
            (UPPER_R, [-30.0, 0.0, 35.0]),
            (FORE_R, [35.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 20.0, 2.0]),
            (SPINE_J, [14.0, 0.0, -4.0]),
            (CHEST_J, [4.0, 10.0, 0.0]),
            (NECK_J, [-4.0, 8.0, 0.0]),
            (THIGH_L, [20.0, 0.0, -6.0]),
            (SHIN_L, [38.0, 0.0, 0.0]),
            (FOOT_L, [-20.0, 0.0, 6.0]),
            (THIGH_R, [26.0, 25.0, -6.0]),
            (SHIN_R, [18.0, 0.0, 0.0]),
            (FOOT_R, [50.0, 0.0, 10.0]),
            (UPPER_L, [30.0, 0.0, -80.0]),
            (FORE_L, [20.0, 0.0, 0.0]),
            (UPPER_R, [-40.0, 0.0, 40.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 40.0, 4.0]),
            (SPINE_J, [-2.0, 0.0, -2.0]),
            (CHEST_J, [0.0, 12.0, 0.0]),
            (THIGH_L, [4.0, 0.0, -4.0]),
            (SHIN_L, [12.0, 0.0, 0.0]),
            (FOOT_L, [30.0, 0.0, 4.0]),
            (THIGH_R, [70.0, 30.0, -25.0]),
            (SHIN_R, [25.0, 0.0, 0.0]),
            (FOOT_R, [55.0, 0.0, 15.0]),
            (UPPER_L, [50.0, 0.0, -40.0]),
            (FORE_L, [40.0, 0.0, 0.0]),
            (UPPER_R, [-25.0, 0.0, 35.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
    ];
    // A shot back over the left shoulder: the body winds round on the left foot and turns most of
    // the way back through the strike (the game's back clip carries its own turn).
    let shot_back = [
        stand.with(&[
            (PELVIS, [0.0, 35.0, 0.0]),
            (SPINE_J, [10.0, 0.0, -6.0]),
            (CHEST_J, [4.0, 15.0, 0.0]),
            (THIGH_L, [18.0, 0.0, -8.0]),
            (SHIN_L, [44.0, 0.0, 0.0]),
            (FOOT_L, [-26.0, 0.0, 8.0]),
            (THIGH_R, [-30.0, 0.0, 10.0]),
            (SHIN_R, [100.0, 0.0, 0.0]),
            (FOOT_R, [-75.0, 0.0, 0.0]),
            (UPPER_L, [30.0, 0.0, -70.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [-20.0, 0.0, 40.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 100.0, 0.0]),
            (SPINE_J, [14.0, 0.0, -4.0]),
            (CHEST_J, [4.0, 10.0, 0.0]),
            (NECK_J, [-4.0, 20.0, 0.0]),
            (THIGH_L, [18.0, 0.0, -6.0]),
            (SHIN_L, [38.0, 0.0, 0.0]),
            (FOOT_L, [-20.0, 0.0, 6.0]),
            (THIGH_R, [30.0, 10.0, 0.0]),
            (SHIN_R, [16.0, 0.0, 0.0]),
            (FOOT_R, [50.0, 0.0, 0.0]),
            (UPPER_L, [35.0, 0.0, -80.0]),
            (FORE_L, [20.0, 0.0, 0.0]),
            (UPPER_R, [-40.0, 0.0, 40.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 130.0, 4.0]),
            (SPINE_J, [-4.0, 0.0, -2.0]),
            (CHEST_J, [0.0, 10.0, 0.0]),
            (THIGH_L, [4.0, 0.0, -4.0]),
            (SHIN_L, [12.0, 0.0, 0.0]),
            (FOOT_L, [30.0, 0.0, 4.0]),
            (THIGH_R, [75.0, 10.0, -10.0]),
            (SHIN_R, [22.0, 0.0, 0.0]),
            (FOOT_R, [55.0, 0.0, 0.0]),
            (UPPER_L, [50.0, 0.0, -40.0]),
            (FORE_L, [40.0, 0.0, 0.0]),
            (UPPER_R, [-30.0, 0.0, 35.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
    ];
    // A heel flick back (passes and lobs going behind you): the right foot steps over the ball
    // and flicks it back with the heel, the body upright and the arms out.
    let heel = [
        stand.with(&[
            (SPINE_J, [6.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -6.0]),
            (SHIN_L, [30.0, 0.0, 0.0]),
            (FOOT_L, [-18.0, 0.0, 6.0]),
            (THIGH_R, [30.0, 0.0, 4.0]),
            (SHIN_R, [40.0, 0.0, 0.0]),
            (FOOT_R, [-10.0, 0.0, -4.0]),
            (UPPER_L, [10.0, 0.0, -45.0]),
            (UPPER_R, [10.0, 0.0, 45.0]),
        ]),
        stand.with(&[
            (SPINE_J, [12.0, 0.0, 0.0]),
            (THIGH_L, [14.0, 0.0, -6.0]),
            (SHIN_L, [34.0, 0.0, 0.0]),
            (FOOT_L, [-20.0, 0.0, 6.0]),
            (THIGH_R, [-25.0, 0.0, 4.0]),
            (SHIN_R, [45.0, 0.0, 0.0]),
            (FOOT_R, [-30.0, 0.0, -4.0]),
            (UPPER_L, [15.0, 0.0, -55.0]),
            (UPPER_R, [15.0, 0.0, 55.0]),
        ]),
        stand.with(&[
            (SPINE_J, [10.0, 0.0, 0.0]),
            (THIGH_L, [10.0, 0.0, -6.0]),
            (SHIN_L, [26.0, 0.0, 0.0]),
            (FOOT_L, [-16.0, 0.0, 6.0]),
            (THIGH_R, [-32.0, 0.0, 4.0]),
            (SHIN_R, [75.0, 0.0, 0.0]),
            (FOOT_R, [-45.0, 0.0, -4.0]),
            (UPPER_L, [10.0, 0.0, -50.0]),
            (UPPER_R, [10.0, 0.0, 50.0]),
        ]),
    ];

    // Landing out of a kick (Kev's clips): the kicking foot comes down well ahead in a long
    // stride, the other leg trailing heel up, the body still leaning on with the arms out, so the
    // kick carries you forward into your next step.
    let land = stand.with(&[
        (PELVIS, [0.0, 10.0, 0.0]),
        (SPINE_J, [28.0, 0.0, 0.0]),
        (CHEST_J, [4.0, -6.0, 0.0]),
        (NECK_J, [-16.0, 0.0, 0.0]),
        (THIGH_R, [42.0, 0.0, -4.0]),
        (SHIN_R, [28.0, 0.0, 0.0]),
        (FOOT_R, [-14.0, 0.0, 0.0]),
        (THIGH_L, [-34.0, 0.0, 4.0]),
        (SHIN_L, [50.0, 0.0, 0.0]),
        (FOOT_L, [24.0, 0.0, 0.0]),
        (UPPER_L, [20.0, 0.0, -80.0]),
        (FORE_L, [20.0, 0.0, 0.0]),
        (UPPER_R, [-10.0, 0.0, 80.0]),
        (FORE_R, [20.0, 0.0, 0.0]),
    ]);

    // Controlling the ball: lean over it, the right foot reaches out to cushion it.
    let control = Pose::default().with(&[
        (SPINE_J, [6.0, 0.0, 0.0]),
        (NECK_J, [6.0, 0.0, 0.0]),
        (HEAD_J, [6.0, 0.0, 0.0]),
        (THIGH_R, [16.0, 0.0, 6.0]),
        (SHIN_R, [14.0, 0.0, 0.0]),
        (FOOT_R, [-6.0, 0.0, -6.0]),
        (UPPER_L, [0.0, 0.0, -12.0]),
        (UPPER_R, [0.0, 0.0, 10.0]),
    ]);

    // A pass: the leg swings back turned out, the inside of the foot sweeps through the ball with the
    // body over it and both arms wide, and follows on low toward the target.
    let pass_kick = [
        stand.with(&[
            (PELVIS, [0.0, -14.0, 0.0]),
            (SPINE_J, [12.0, 0.0, -4.0]),
            (CHEST_J, [2.0, 10.0, 0.0]),
            (NECK_J, [-8.0, 0.0, 0.0]),
            (THIGH_L, [16.0, 0.0, -6.0]),
            (SHIN_L, [36.0, 0.0, 0.0]),
            (FOOT_L, [-20.0, 0.0, 6.0]),
            (THIGH_R, [-28.0, -40.0, 12.0]),
            (SHIN_R, [60.0, 0.0, 0.0]),
            (FOOT_R, [-40.0, 0.0, -10.0]),
            (UPPER_L, [30.0, 0.0, -60.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [-15.0, 0.0, 45.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 8.0, 0.0]),
            (SPINE_J, [20.0, 0.0, -4.0]),
            (CHEST_J, [2.0, -8.0, 0.0]),
            (NECK_J, [-10.0, 0.0, 0.0]),
            (THIGH_L, [24.0, 0.0, -6.0]),
            (SHIN_L, [46.0, 0.0, 0.0]),
            (FOOT_L, [-22.0, 0.0, 6.0]),
            (THIGH_R, [22.0, -50.0, 4.0]),
            (SHIN_R, [20.0, 0.0, 0.0]),
            (FOOT_R, [-6.0, 0.0, -14.0]),
            (UPPER_L, [30.0, 0.0, -65.0]),
            (FORE_L, [25.0, 0.0, 0.0]),
            (UPPER_R, [-10.0, 0.0, 50.0]),
            (FORE_R, [25.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 16.0, 0.0]),
            (SPINE_J, [14.0, 0.0, -2.0]),
            (CHEST_J, [2.0, -12.0, 0.0]),
            (THIGH_L, [16.0, 0.0, -4.0]),
            (SHIN_L, [34.0, 0.0, 0.0]),
            (FOOT_L, [-6.0, 0.0, 4.0]),
            (THIGH_R, [50.0, -40.0, -4.0]),
            (SHIN_R, [24.0, 0.0, 0.0]),
            (FOOT_R, [10.0, 0.0, -10.0]),
            (UPPER_L, [35.0, 0.0, -55.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [0.0, 0.0, 45.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
    ];
    // A lob: a deep knee-bent backswing, the foot stabs under the ball with the body leaning back
    // and the arms up, and the leg lifts on high.
    let lob_kick = [
        stand.with(&[
            (PELVIS, [0.0, -16.0, -2.0]),
            (SPINE_J, [2.0, 0.0, -8.0]),
            (CHEST_J, [0.0, 12.0, -2.0]),
            (THIGH_L, [18.0, 0.0, -6.0]),
            (SHIN_L, [44.0, 0.0, 0.0]),
            (FOOT_L, [-26.0, 0.0, 6.0]),
            (THIGH_R, [-38.0, 0.0, 6.0]),
            (SHIN_R, [110.0, 0.0, 0.0]),
            (FOOT_R, [-90.0, 0.0, 0.0]),
            (UPPER_L, [15.0, 0.0, -70.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [-30.0, 0.0, 40.0]),
            (FORE_R, [35.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 8.0, 0.0]),
            (SPINE_J, [-12.0, 0.0, -6.0]),
            (CHEST_J, [-4.0, -8.0, 0.0]),
            (NECK_J, [8.0, 0.0, 0.0]),
            (THIGH_L, [22.0, 0.0, -6.0]),
            (SHIN_L, [44.0, 0.0, 0.0]),
            (FOOT_L, [-22.0, 0.0, 6.0]),
            (THIGH_R, [32.0, 0.0, 0.0]),
            (SHIN_R, [26.0, 0.0, 0.0]),
            (FOOT_R, [10.0, 0.0, 0.0]),
            (UPPER_L, [20.0, 0.0, -75.0]),
            (FORE_L, [25.0, 0.0, 0.0]),
            (UPPER_R, [-10.0, 0.0, 60.0]),
            (FORE_R, [25.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 14.0, 0.0]),
            (SPINE_J, [-18.0, 0.0, -2.0]),
            (CHEST_J, [-4.0, -10.0, 0.0]),
            (NECK_J, [10.0, 0.0, 0.0]),
            (THIGH_L, [10.0, 0.0, -4.0]),
            (SHIN_L, [24.0, 0.0, 0.0]),
            (FOOT_L, [10.0, 0.0, 4.0]),
            (THIGH_R, [45.0, 0.0, -4.0]),
            (SHIN_R, [20.0, 0.0, 0.0]),
            (FOOT_R, [30.0, 0.0, 0.0]),
            (UPPER_L, [25.0, 0.0, -65.0]),
            (FORE_L, [35.0, 0.0, 0.0]),
            (UPPER_R, [0.0, 0.0, 60.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
    ];
    // The rainbow flick ahead (our own keys, from the description in volleys-and-flicks.md): the
    // left foot plants beside the ball with the right toe dropped behind it; the toe slides under
    // the ball and snaps up with the body upright and the arms out; the knee drives on up high and
    // the body leans back, the head following the ball up and over.
    let flick = [
        stand.with(&[
            (PELVIS, [0.0, -6.0, 0.0]),
            (SPINE_J, [14.0, 0.0, -2.0]),
            (CHEST_J, [2.0, 4.0, 0.0]),
            (NECK_J, [-10.0, 0.0, 0.0]),
            (THIGH_L, [22.0, 0.0, -6.0]),
            (SHIN_L, [44.0, 0.0, 0.0]),
            (FOOT_L, [-22.0, 0.0, 6.0]),
            (THIGH_R, [-12.0, 0.0, 6.0]),
            (SHIN_R, [62.0, 0.0, 0.0]),
            (FOOT_R, [-40.0, 0.0, 0.0]),
            (UPPER_L, [10.0, 0.0, -40.0]),
            (FORE_L, [25.0, 0.0, 0.0]),
            (UPPER_R, [0.0, 0.0, 40.0]),
            (FORE_R, [25.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 4.0, 0.0]),
            (SPINE_J, [2.0, 0.0, 0.0]),
            (CHEST_J, [-2.0, 0.0, 0.0]),
            (NECK_J, [4.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -6.0]),
            (SHIN_L, [28.0, 0.0, 0.0]),
            (FOOT_L, [-6.0, 0.0, 6.0]),
            (THIGH_R, [38.0, 0.0, 2.0]),
            (SHIN_R, [22.0, 0.0, 0.0]),
            (FOOT_R, [-12.0, 0.0, 0.0]),
            (UPPER_L, [25.0, 0.0, -55.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [15.0, 0.0, 55.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 8.0, 0.0]),
            (SPINE_J, [-10.0, 0.0, 0.0]),
            (CHEST_J, [-4.0, 0.0, 0.0]),
            (NECK_J, [-12.0, 0.0, 0.0]),
            (THIGH_L, [4.0, 0.0, -4.0]),
            (SHIN_L, [12.0, 0.0, 0.0]),
            (FOOT_L, [20.0, 0.0, 4.0]),
            (THIGH_R, [88.0, 0.0, 0.0]),
            (SHIN_R, [70.0, 0.0, 0.0]),
            (FOOT_R, [5.0, 0.0, 0.0]),
            (UPPER_L, [40.0, 0.0, -65.0]),
            (FORE_L, [35.0, 0.0, 0.0]),
            (UPPER_R, [30.0, 0.0, 65.0]),
            (FORE_R, [35.0, 0.0, 0.0]),
        ]),
    ];
    // The flick across the body to the left (the right foot; mirrored for the right): the leg reaches
    // out to the right with the toe under the ball, scoops it across and up with the inside of the
    // foot, and follows on high across, the hips turning through.
    let flick_side = [
        stand.with(&[
            (PELVIS, [0.0, -18.0, 0.0]),
            (SPINE_J, [12.0, 0.0, -4.0]),
            (CHEST_J, [2.0, 8.0, 0.0]),
            (NECK_J, [-8.0, 6.0, 0.0]),
            (THIGH_L, [20.0, 0.0, -8.0]),
            (SHIN_L, [42.0, 0.0, 0.0]),
            (FOOT_L, [-22.0, 0.0, 8.0]),
            (THIGH_R, [-8.0, -25.0, 22.0]),
            (SHIN_R, [48.0, 0.0, 0.0]),
            (FOOT_R, [-30.0, 0.0, -12.0]),
            (UPPER_L, [20.0, 0.0, -55.0]),
            (FORE_L, [25.0, 0.0, 0.0]),
            (UPPER_R, [-10.0, 0.0, 50.0]),
            (FORE_R, [25.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 14.0, 0.0]),
            (SPINE_J, [4.0, 0.0, 6.0]),
            (CHEST_J, [0.0, -6.0, 0.0]),
            (NECK_J, [2.0, 0.0, 0.0]),
            (THIGH_L, [14.0, 0.0, -6.0]),
            (SHIN_L, [30.0, 0.0, 0.0]),
            (FOOT_L, [-10.0, 0.0, 6.0]),
            (THIGH_R, [34.0, 30.0, -8.0]),
            (SHIN_R, [28.0, 0.0, 0.0]),
            (FOOT_R, [-8.0, 0.0, 14.0]),
            (UPPER_L, [30.0, 0.0, -60.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [10.0, 0.0, 60.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (PELVIS, [0.0, 26.0, 0.0]),
            (SPINE_J, [-6.0, 0.0, 8.0]),
            (CHEST_J, [-2.0, -8.0, 0.0]),
            (NECK_J, [-8.0, 10.0, 0.0]),
            (THIGH_L, [4.0, 0.0, -4.0]),
            (SHIN_L, [14.0, 0.0, 0.0]),
            (FOOT_L, [18.0, 0.0, 4.0]),
            (THIGH_R, [70.0, 38.0, -22.0]),
            (SHIN_R, [55.0, 0.0, 0.0]),
            (FOOT_R, [4.0, 0.0, 18.0]),
            (UPPER_L, [40.0, 0.0, -60.0]),
            (FORE_L, [35.0, 0.0, 0.0]),
            (UPPER_R, [25.0, 0.0, 65.0]),
            (FORE_R, [35.0, 0.0, 0.0]),
        ]),
    ];

    // Volley touches (our own keys): the body sets itself under the ball, meets it and lets it go.
    // The foot: the knee lifts and the foot cushions the ball on ahead, the laces flat.
    let touch_foot = [
        stand.with(&[
            (SPINE_J, [10.0, 0.0, 0.0]),
            (NECK_J, [6.0, 0.0, 0.0]),
            (THIGH_L, [14.0, 0.0, -4.0]),
            (SHIN_L, [30.0, 0.0, 0.0]),
            (FOOT_L, [-16.0, 0.0, 4.0]),
            (THIGH_R, [12.0, 0.0, 4.0]),
            (SHIN_R, [52.0, 0.0, 0.0]),
            (FOOT_R, [-30.0, 0.0, 0.0]),
            (UPPER_L, [5.0, 0.0, -30.0]),
            (UPPER_R, [0.0, 0.0, 30.0]),
        ]),
        stand.with(&[
            (SPINE_J, [6.0, 0.0, 0.0]),
            (NECK_J, [8.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -4.0]),
            (SHIN_L, [28.0, 0.0, 0.0]),
            (FOOT_L, [-16.0, 0.0, 4.0]),
            (THIGH_R, [42.0, 0.0, 2.0]),
            (SHIN_R, [30.0, 0.0, 0.0]),
            (FOOT_R, [-2.0, 0.0, 0.0]),
            (UPPER_L, [10.0, 0.0, -35.0]),
            (UPPER_R, [0.0, 0.0, 35.0]),
        ]),
        stand.with(&[
            (SPINE_J, [8.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -4.0]),
            (SHIN_L, [26.0, 0.0, 0.0]),
            (FOOT_L, [-14.0, 0.0, 4.0]),
            (THIGH_R, [30.0, 0.0, 2.0]),
            (SHIN_R, [42.0, 0.0, 0.0]),
            (FOOT_R, [-6.0, 0.0, 0.0]),
            (UPPER_L, [8.0, 0.0, -30.0]),
            (UPPER_R, [0.0, 0.0, 30.0]),
        ]),
    ];
    // The thigh: the knee comes up to hip height, the thigh flat under the ball, eyes on it.
    let touch_thigh = [
        stand.with(&[
            (SPINE_J, [2.0, 0.0, 0.0]),
            (NECK_J, [6.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -4.0]),
            (SHIN_L, [26.0, 0.0, 0.0]),
            (FOOT_L, [-14.0, 0.0, 4.0]),
            (THIGH_R, [30.0, 0.0, 6.0]),
            (SHIN_R, [70.0, 0.0, 0.0]),
            (FOOT_R, [-20.0, 0.0, 0.0]),
            (UPPER_L, [10.0, 0.0, -45.0]),
            (UPPER_R, [10.0, 0.0, 45.0]),
        ]),
        stand.with(&[
            (SPINE_J, [-4.0, 0.0, 0.0]),
            (NECK_J, [12.0, 0.0, 0.0]),
            (THIGH_L, [8.0, 0.0, -4.0]),
            (SHIN_L, [20.0, 0.0, 0.0]),
            (FOOT_L, [-6.0, 0.0, 4.0]),
            (THIGH_R, [75.0, 0.0, 4.0]),
            (SHIN_R, [85.0, 0.0, 0.0]),
            (FOOT_R, [10.0, 0.0, 0.0]),
            (UPPER_L, [15.0, 0.0, -55.0]),
            (UPPER_R, [15.0, 0.0, 55.0]),
        ]),
        stand.with(&[
            (SPINE_J, [2.0, 0.0, 0.0]),
            (NECK_J, [8.0, 0.0, 0.0]),
            (THIGH_L, [10.0, 0.0, -4.0]),
            (SHIN_L, [22.0, 0.0, 0.0]),
            (FOOT_L, [-10.0, 0.0, 4.0]),
            (THIGH_R, [55.0, 0.0, 4.0]),
            (SHIN_R, [70.0, 0.0, 0.0]),
            (FOOT_R, [5.0, 0.0, 0.0]),
            (UPPER_L, [12.0, 0.0, -50.0]),
            (UPPER_R, [12.0, 0.0, 50.0]),
        ]),
    ];
    // The stomach: the arms swing back and the hips push through, the back arched over the ball.
    let touch_belly = [
        stand.with(&[
            (SPINE_J, [12.0, 0.0, 0.0]),
            (NECK_J, [4.0, 0.0, 0.0]),
            (THIGH_L, [16.0, 0.0, -6.0]),
            (SHIN_L, [32.0, 0.0, 0.0]),
            (FOOT_L, [-16.0, 0.0, 6.0]),
            (THIGH_R, [16.0, 0.0, 6.0]),
            (SHIN_R, [32.0, 0.0, 0.0]),
            (FOOT_R, [-16.0, 0.0, -6.0]),
            (UPPER_L, [20.0, 0.0, -30.0]),
            (UPPER_R, [20.0, 0.0, 30.0]),
        ]),
        stand.with(&[
            (SPINE_J, [-14.0, 0.0, 0.0]),
            (CHEST_J, [-6.0, 0.0, 0.0]),
            (NECK_J, [18.0, 0.0, 0.0]),
            (THIGH_L, [-6.0, 0.0, -6.0]),
            (SHIN_L, [22.0, 0.0, 0.0]),
            (FOOT_L, [-28.0, 0.0, 6.0]),
            (THIGH_R, [-6.0, 0.0, 6.0]),
            (SHIN_R, [22.0, 0.0, 0.0]),
            (FOOT_R, [-28.0, 0.0, -6.0]),
            (UPPER_L, [-30.0, 0.0, -30.0]),
            (UPPER_R, [-30.0, 0.0, 30.0]),
        ]),
        stand.with(&[
            (SPINE_J, [-4.0, 0.0, 0.0]),
            (NECK_J, [8.0, 0.0, 0.0]),
            (THIGH_L, [6.0, 0.0, -6.0]),
            (SHIN_L, [16.0, 0.0, 0.0]),
            (FOOT_L, [-10.0, 0.0, 6.0]),
            (THIGH_R, [6.0, 0.0, 6.0]),
            (SHIN_R, [16.0, 0.0, 0.0]),
            (FOOT_R, [-10.0, 0.0, -6.0]),
            (UPPER_L, [-10.0, 0.0, -35.0]),
            (UPPER_R, [-10.0, 0.0, 35.0]),
        ]),
    ];
    // The chest: knees bent and arms wide, the body leans back and the chest takes the ball, the
    // chin down watching it.
    let touch_chest = [
        stand.with(&[
            (SPINE_J, [4.0, 0.0, 0.0]),
            (NECK_J, [-6.0, 0.0, 0.0]),
            (THIGH_L, [24.0, 0.0, -6.0]),
            (SHIN_L, [44.0, 0.0, 0.0]),
            (FOOT_L, [-20.0, 0.0, 6.0]),
            (THIGH_R, [24.0, 0.0, 6.0]),
            (SHIN_R, [44.0, 0.0, 0.0]),
            (FOOT_R, [-20.0, 0.0, -6.0]),
            (UPPER_L, [10.0, 0.0, -60.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [10.0, 0.0, 60.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (SPINE_J, [-18.0, 0.0, 0.0]),
            (CHEST_J, [-10.0, 0.0, 0.0]),
            (NECK_J, [16.0, 0.0, 0.0]),
            (THIGH_L, [20.0, 0.0, -6.0]),
            (SHIN_L, [36.0, 0.0, 0.0]),
            (FOOT_L, [-16.0, 0.0, 6.0]),
            (THIGH_R, [20.0, 0.0, 6.0]),
            (SHIN_R, [36.0, 0.0, 0.0]),
            (FOOT_R, [-16.0, 0.0, -6.0]),
            (UPPER_L, [-10.0, 0.0, -70.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [-10.0, 0.0, 70.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (SPINE_J, [-6.0, 0.0, 0.0]),
            (CHEST_J, [-2.0, 0.0, 0.0]),
            (NECK_J, [6.0, 0.0, 0.0]),
            (THIGH_L, [12.0, 0.0, -6.0]),
            (SHIN_L, [24.0, 0.0, 0.0]),
            (FOOT_L, [-12.0, 0.0, 6.0]),
            (THIGH_R, [12.0, 0.0, 6.0]),
            (SHIN_R, [24.0, 0.0, 0.0]),
            (FOOT_R, [-12.0, 0.0, -6.0]),
            (UPPER_L, [5.0, 0.0, -55.0]),
            (UPPER_R, [5.0, 0.0, 55.0]),
        ]),
    ];
    // The head: knees bent and the head back, then the neck nods through the ball as the legs
    // straighten onto the toes, the arms swinging down.
    let touch_head = [
        stand.with(&[
            (SPINE_J, [-10.0, 0.0, 0.0]),
            (NECK_J, [-20.0, 0.0, 0.0]),
            (THIGH_L, [22.0, 0.0, -6.0]),
            (SHIN_L, [40.0, 0.0, 0.0]),
            (FOOT_L, [-18.0, 0.0, 6.0]),
            (THIGH_R, [22.0, 0.0, 6.0]),
            (SHIN_R, [40.0, 0.0, 0.0]),
            (FOOT_R, [-18.0, 0.0, -6.0]),
            (UPPER_L, [30.0, 0.0, -50.0]),
            (FORE_L, [40.0, 0.0, 0.0]),
            (UPPER_R, [30.0, 0.0, 50.0]),
            (FORE_R, [40.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (SPINE_J, [10.0, 0.0, 0.0]),
            (NECK_J, [16.0, 0.0, 0.0]),
            (THIGH_L, [6.0, 0.0, -6.0]),
            (SHIN_L, [10.0, 0.0, 0.0]),
            (FOOT_L, [6.0, 0.0, 6.0]),
            (THIGH_R, [6.0, 0.0, 6.0]),
            (SHIN_R, [10.0, 0.0, 0.0]),
            (FOOT_R, [6.0, 0.0, -6.0]),
            (UPPER_L, [-10.0, 0.0, -45.0]),
            (FORE_L, [25.0, 0.0, 0.0]),
            (UPPER_R, [-10.0, 0.0, 45.0]),
            (FORE_R, [25.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (SPINE_J, [16.0, 0.0, 0.0]),
            (NECK_J, [10.0, 0.0, 0.0]),
            (THIGH_L, [10.0, 0.0, -6.0]),
            (SHIN_L, [18.0, 0.0, 0.0]),
            (FOOT_L, [-6.0, 0.0, 6.0]),
            (THIGH_R, [10.0, 0.0, 6.0]),
            (SHIN_R, [18.0, 0.0, 0.0]),
            (FOOT_R, [-6.0, 0.0, -6.0]),
            (UPPER_L, [0.0, 0.0, -40.0]),
            (UPPER_R, [0.0, 0.0, 40.0]),
        ]),
    ];
    // A jumping header: a deep crouch with the arms back to take off, then in the air the body
    // arches and snaps the head through the ball, the legs trailing and the arms up and out; it
    // lands on bent knees.
    let jump_head = [
        stand.with(&[
            (SPINE_J, [24.0, 0.0, 0.0]),
            (NECK_J, [-14.0, 0.0, 0.0]),
            (THIGH_L, [45.0, 0.0, -6.0]),
            (SHIN_L, [85.0, 0.0, 0.0]),
            (FOOT_L, [-35.0, 0.0, 6.0]),
            (THIGH_R, [45.0, 0.0, 6.0]),
            (SHIN_R, [85.0, 0.0, 0.0]),
            (FOOT_R, [-35.0, 0.0, -6.0]),
            (UPPER_L, [-40.0, 0.0, -20.0]),
            (UPPER_R, [-40.0, 0.0, 20.0]),
        ]),
        stand.with(&[
            (SPINE_J, [8.0, 0.0, 0.0]),
            (CHEST_J, [4.0, 0.0, 0.0]),
            (NECK_J, [18.0, 0.0, 0.0]),
            (THIGH_L, [-10.0, 0.0, -6.0]),
            (SHIN_L, [60.0, 0.0, 0.0]),
            (FOOT_L, [20.0, 0.0, 6.0]),
            (THIGH_R, [10.0, 0.0, 6.0]),
            (SHIN_R, [70.0, 0.0, 0.0]),
            (FOOT_R, [20.0, 0.0, -6.0]),
            (UPPER_L, [60.0, 0.0, -60.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [60.0, 0.0, 60.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ]),
        stand.with(&[
            (SPINE_J, [16.0, 0.0, 0.0]),
            (NECK_J, [4.0, 0.0, 0.0]),
            (THIGH_L, [30.0, 0.0, -6.0]),
            (SHIN_L, [55.0, 0.0, 0.0]),
            (FOOT_L, [-25.0, 0.0, 6.0]),
            (THIGH_R, [30.0, 0.0, 6.0]),
            (SHIN_R, [55.0, 0.0, 0.0]),
            (FOOT_R, [-25.0, 0.0, -6.0]),
            (UPPER_L, [20.0, 0.0, -50.0]),
            (UPPER_R, [20.0, 0.0, 50.0]),
        ]),
    ];
    let touch = [touch_foot, touch_thigh, touch_belly, touch_chest, touch_head];

    // Dribbles, timed to their touch. A fast cut drops into a deep crouch on the left foot, drives
    // the ball on with the right at 17f low and leaning in, then runs out of it.
    let dribble_forward = (
        OneShot(vec![
            (0.0, stand.with(&[
                (PELVIS, [0.0, -14.0, 0.0]),
                (SPINE_J, [30.0, 0.0, 0.0]),
                (CHEST_J, [6.0, 10.0, 0.0]),
                (NECK_J, [-20.0, 0.0, 0.0]),
                (THIGH_L, [40.0, 0.0, -10.0]),
                (SHIN_L, [80.0, 0.0, 0.0]),
                (FOOT_L, [-40.0, 0.0, 10.0]),
                (THIGH_R, [-20.0, 0.0, 8.0]),
                (SHIN_R, [70.0, 0.0, 0.0]),
                (FOOT_R, [-50.0, 0.0, -8.0]),
                (UPPER_L, [-40.0, 0.0, -30.0]),
                (FORE_L, [70.0, 0.0, 0.0]),
                (UPPER_R, [45.0, 0.0, 25.0]),
                (FORE_R, [70.0, 0.0, 0.0]),
            ])),
            (0.38, stand.with(&[
                (PELVIS, [0.0, 10.0, 0.0]),
                (SPINE_J, [26.0, 0.0, 0.0]),
                (CHEST_J, [6.0, -12.0, 0.0]),
                (NECK_J, [-18.0, 0.0, 0.0]),
                (THIGH_L, [-10.0, 0.0, -8.0]),
                (SHIN_L, [50.0, 0.0, 0.0]),
                (FOOT_L, [10.0, 0.0, 8.0]),
                (THIGH_R, [50.0, 0.0, 12.0]),
                (SHIN_R, [40.0, 0.0, 0.0]),
                (FOOT_R, [30.0, 0.0, -12.0]),
                (UPPER_L, [50.0, 0.0, -30.0]),
                (FORE_L, [80.0, 0.0, 0.0]),
                (UPPER_R, [-45.0, 0.0, 25.0]),
                (FORE_R, [80.0, 0.0, 0.0]),
            ])),
            (0.7, run_contact),
            (1.0, run_stance),
        ]),
        0.38,
    );
    // A side dribble (to the right): load low on the left foot, the right leg sweeps out wide and
    // takes the ball across at 8f as the body leans into it, landing wide and gathering.
    let side_load = stance.with(&[
        (PELVIS, [0.0, 0.0, -6.0]),
        (SPINE_J, [18.0, 0.0, -10.0]),
        (CHEST_J, [4.0, 0.0, -4.0]),
        (NECK_J, [-12.0, 0.0, 8.0]),
        (THIGH_L, [22.0, 0.0, -10.0]),
        (SHIN_L, [60.0, 0.0, 0.0]),
        (FOOT_L, [-38.0, 0.0, 10.0]),
        (THIGH_R, [16.0, 0.0, 22.0]),
        (SHIN_R, [40.0, 0.0, 0.0]),
        (FOOT_R, [-24.0, 0.0, -22.0]),
        (UPPER_L, [10.0, 0.0, -50.0]),
        (FORE_L, [40.0, 0.0, 0.0]),
        (UPPER_R, [10.0, 0.0, 55.0]),
        (FORE_R, [40.0, 0.0, 0.0]),
    ]);
    let dribble_side = (
        OneShot(vec![
            (0.0, side_load),
            (0.14, stance.with(&[
                (PELVIS, [0.0, 0.0, 10.0]),
                (SPINE_J, [16.0, 0.0, 14.0]),
                (CHEST_J, [4.0, 0.0, 4.0]),
                (NECK_J, [-12.0, 0.0, -14.0]),
                (THIGH_L, [8.0, 0.0, -4.0]),
                (SHIN_L, [40.0, 0.0, 0.0]),
                (FOOT_L, [10.0, 0.0, 4.0]),
                (THIGH_R, [14.0, 0.0, 40.0]),
                (SHIN_R, [24.0, 0.0, 0.0]),
                (FOOT_R, [-6.0, 0.0, -40.0]),
                (UPPER_L, [20.0, 0.0, -60.0]),
                (FORE_L, [40.0, 0.0, 0.0]),
                (UPPER_R, [0.0, 0.0, 65.0]),
                (FORE_R, [30.0, 0.0, 0.0]),
            ])),
            (0.5, stance.with(&[
                (PELVIS, [0.0, 0.0, 6.0]),
                (SPINE_J, [16.0, 0.0, 6.0]),
                (THIGH_L, [18.0, 0.0, 14.0]),
                (SHIN_L, [55.0, 0.0, 0.0]),
                (FOOT_L, [-30.0, 0.0, -14.0]),
                (THIGH_R, [18.0, 0.0, 20.0]),
                (SHIN_R, [50.0, 0.0, 0.0]),
                (FOOT_R, [-32.0, 0.0, -20.0]),
                (UPPER_L, [20.0, 0.0, -45.0]),
                (UPPER_R, [15.0, 0.0, 45.0]),
            ])),
            (1.0, stance),
        ]),
        0.14,
    );
    // A drag back: the right sole goes up onto the ball at once, the body leaning back, and rolls
    // it back under you, arms wide.
    let dribble_back = (
        OneShot(vec![
            (0.0, stance.with(&[
                (SPINE_J, [4.0, 0.0, 0.0]),
                (NECK_J, [-4.0, 0.0, 0.0]),
                (THIGH_L, [10.0, 0.0, -8.0]),
                (SHIN_L, [40.0, 0.0, 0.0]),
                (FOOT_L, [-30.0, 0.0, 8.0]),
                (THIGH_R, [55.0, 0.0, 6.0]),
                (SHIN_R, [60.0, 0.0, 0.0]),
                (FOOT_R, [-10.0, 0.0, -6.0]),
                (UPPER_L, [30.0, 0.0, -50.0]),
                (FORE_L, [40.0, 0.0, 0.0]),
                (UPPER_R, [20.0, 0.0, 50.0]),
                (FORE_R, [40.0, 0.0, 0.0]),
            ])),
            (0.35, stance.with(&[
                (SPINE_J, [-6.0, 0.0, 0.0]),
                (THIGH_L, [-6.0, 0.0, -8.0]),
                (SHIN_L, [30.0, 0.0, 0.0]),
                (FOOT_L, [-24.0, 0.0, 8.0]),
                (THIGH_R, [5.0, 0.0, 6.0]),
                (SHIN_R, [70.0, 0.0, 0.0]),
                (FOOT_R, [-40.0, 0.0, -6.0]),
                (UPPER_L, [35.0, 0.0, -55.0]),
                (FORE_L, [40.0, 0.0, 0.0]),
                (UPPER_R, [30.0, 0.0, 55.0]),
                (FORE_R, [40.0, 0.0, 0.0]),
            ])),
            (1.0, stance),
        ]),
        0.0,
    );
    // A feint (to the right): the weight drops onto the left, the right leg circles over the ball at
    // its touch, then the body pushes off to the right.
    let feint = (
        OneShot(vec![
            (0.0, stance.with(&[
                (PELVIS, [0.0, 8.0, -10.0]),
                (SPINE_J, [16.0, 0.0, -14.0]),
                (CHEST_J, [4.0, 8.0, -4.0]),
                (NECK_J, [-12.0, 0.0, 12.0]),
                (THIGH_L, [18.0, 0.0, -24.0]),
                (SHIN_L, [55.0, 0.0, 0.0]),
                (FOOT_L, [-36.0, 0.0, 24.0]),
                (THIGH_R, [12.0, 0.0, 4.0]),
                (SHIN_R, [45.0, 0.0, 0.0]),
                (FOOT_R, [-10.0, 0.0, -4.0]),
                (UPPER_L, [10.0, 0.0, -65.0]),
                (FORE_L, [40.0, 0.0, 0.0]),
                (UPPER_R, [25.0, 0.0, 40.0]),
                (FORE_R, [40.0, 0.0, 0.0]),
            ])),
            (0.22, stance.with(&[
                (PELVIS, [0.0, 14.0, -8.0]),
                (SPINE_J, [16.0, 0.0, -12.0]),
                (CHEST_J, [4.0, 10.0, -4.0]),
                (NECK_J, [-12.0, 0.0, 10.0]),
                (THIGH_L, [16.0, 0.0, -20.0]),
                (SHIN_L, [55.0, 0.0, 0.0]),
                (FOOT_L, [-36.0, 0.0, 20.0]),
                (THIGH_R, [50.0, -30.0, -18.0]),
                (SHIN_R, [80.0, 0.0, 0.0]),
                (FOOT_R, [-30.0, 0.0, 18.0]),
                (UPPER_L, [10.0, 0.0, -70.0]),
                (FORE_L, [40.0, 0.0, 0.0]),
                (UPPER_R, [30.0, 0.0, 35.0]),
                (FORE_R, [40.0, 0.0, 0.0]),
            ])),
            (0.55, side_load),
            (1.0, stance),
        ]),
        0.22,
    );
    // The roulette (the forward feint out of a straight or side dribble; Kev's practice recording):
    // low and compact, the knees bent and the torso fairly upright, the arms out with the elbows
    // bent. The right sole stops the ball; you turn a full turn to the left over it on short pivots,
    // facing back with the legs and arms wide and the left foot on the ball halfway, and the right
    // sole has it again as you come round. The turn itself is added on top (animate_body, from
    // roulette_turn); the keys sit at its times (the touch at 10f, the hand-back at 56f).
    let sole_on = lock.with(&[
        (SPINE_J, [14.0, 0.0, 0.0]),
        (THIGH_L, [36.0, 0.0, -16.0]),
        (SHIN_L, [72.0, 0.0, 0.0]),
        (FOOT_L, [-36.0, 0.0, 16.0]),
        (THIGH_R, [58.0, 0.0, 10.0]),
        (SHIN_R, [96.0, 0.0, 0.0]),
        (FOOT_R, [-38.0, 0.0, -10.0]),
        (UPPER_L, [15.0, 0.0, -50.0]),
        (FORE_L, [70.0, 0.0, 0.0]),
        (UPPER_R, [15.0, 0.0, 50.0]),
        (FORE_R, [70.0, 0.0, 0.0]),
    ]);
    let pivot = |lift_left: bool| {
        let (up, down) = ([(48.0, 0.0, -8.0), (96.0, 0.0, 0.0), (-48.0, 0.0, 8.0)], [(34.0, 0.0, 12.0), (68.0, 0.0, 0.0), (-34.0, 0.0, -12.0)]);
        let p = lock.with(&[
            (SPINE_J, [12.0, 0.0, 0.0]),
            (THIGH_L, [up[0].0, 0.0, up[0].2]),
            (SHIN_L, [up[1].0, 0.0, 0.0]),
            (FOOT_L, [up[2].0, 0.0, up[2].2]),
            (THIGH_R, [down[0].0, 0.0, down[0].2]),
            (SHIN_R, [down[1].0, 0.0, 0.0]),
            (FOOT_R, [down[2].0, 0.0, down[2].2]),
            (UPPER_L, [15.0, 0.0, -55.0]),
            (FORE_L, [75.0, 0.0, 0.0]),
            (UPPER_R, [15.0, 0.0, 55.0]),
            (FORE_R, [75.0, 0.0, 0.0]),
        ]);
        if lift_left { p } else { p.mirror() }
    };
    let facing_back = lock.with(&[
        (SPINE_J, [14.0, 0.0, 0.0]),
        (THIGH_L, [44.0, 0.0, -18.0]),
        (SHIN_L, [86.0, 0.0, 0.0]),
        (FOOT_L, [-42.0, 0.0, 18.0]),
        (THIGH_R, [32.0, 0.0, 22.0]),
        (SHIN_R, [64.0, 0.0, 0.0]),
        (FOOT_R, [-32.0, 0.0, -22.0]),
        (UPPER_L, [10.0, 0.0, -60.0]),
        (FORE_L, [60.0, 0.0, 0.0]),
        (UPPER_R, [10.0, 0.0, 60.0]),
        (FORE_R, [60.0, 0.0, 0.0]),
    ]);
    let roulette = (OneShot(vec![(0.0, lock), (0.15, sole_on), (0.3, pivot(true)), (0.48, facing_back), (0.66, pivot(false)), (0.85, sole_on), (1.0, lock)]), 0.15);
    // Out of a fast cut, the forward feint turns you out the cut's way (authored to the right) into
    // a very deep, almost sitting crouch with the hands by the knees, the ball rolling across under
    // you; you come up facing on with the right sole on it and go on. The turn is added on top.
    let sit = lock.with(&[
        (SPINE_J, [32.0, 0.0, 0.0]),
        (CHEST_J, [6.0, 0.0, 0.0]),
        (NECK_J, [-26.0, 0.0, 0.0]),
        (THIGH_L, [72.0, 0.0, -16.0]),
        (SHIN_L, [112.0, 0.0, 0.0]),
        (FOOT_L, [-40.0, 0.0, 16.0]),
        (THIGH_R, [68.0, 0.0, 18.0]),
        (SHIN_R, [108.0, 0.0, 0.0]),
        (FOOT_R, [-40.0, 0.0, -18.0]),
        (UPPER_L, [45.0, 0.0, -18.0]),
        (FORE_L, [55.0, 0.0, 0.0]),
        (UPPER_R, [45.0, 0.0, 18.0]),
        (FORE_R, [55.0, 0.0, 0.0]),
    ]);
    let go_on = low(lock.with(&[(THIGH_L, [34.0, 0.0, -10.0]), (SHIN_L, [50.0, 0.0, 0.0]), (FOOT_L, [-16.0, 0.0, 10.0]), (THIGH_R, [6.0, 0.0, 10.0]), (SHIN_R, [70.0, 0.0, 0.0]), (FOOT_R, [-64.0, 0.0, -10.0])]));
    let turn_out = (OneShot(vec![(0.0, lock), (0.1, sit), (0.18, sit), (0.3, sole_on), (0.55, go_on), (1.0, lock)]), 0.15);
    // Lock move with the ball: the weight on a bent left leg planted wide, the right knee up and the
    // right sole flat on top of the ball, out by the right hip (STANCE_BALL), the arms out wide.
    let sole = lock.with(&[
        (PELVIS, [0.0, 0.0, -4.0]),
        (SPINE_J, [18.0, 0.0, -4.0]),
        (THIGH_L, [36.0, 0.0, -22.0]),
        (SHIN_L, [72.0, 0.0, 0.0]),
        (FOOT_L, [-36.0, 0.0, 22.0]),
        (THIGH_R, [62.0, 0.0, 17.0]),
        (SHIN_R, [102.0, 0.0, 0.0]),
        (FOOT_R, [-40.0, 0.0, -17.0]),
        (UPPER_L, [15.0, 0.0, -45.0]),
        (UPPER_R, [15.0, 0.0, 45.0]),
    ]);
    // Dashes, timed on their clips (front and back 50f, side 41f; the second dash 60/42/43f). In
    // Kev's practice recording a dash gathers in the stance for about 0.1 s, bursts out from
    // +100-160 ms, is at its widest at +180-330 ms, and is back in a wide, low stance by +400-500
    // ms: the keys follow DASH_SPEED. The side dash is authored to the right.
    // Where every dash settles: the lock stance, wider and lower, arms out.
    let settle = stance.with(&[
        (SPINE_J, [16.0, 0.0, 0.0]),
        (THIGH_L, [22.0, 0.0, -18.0]),
        (SHIN_L, [46.0, 0.0, 0.0]),
        (FOOT_L, [-24.0, 0.0, 18.0]),
        (THIGH_R, [22.0, 0.0, 18.0]),
        (SHIN_R, [46.0, 0.0, 0.0]),
        (FOOT_R, [-24.0, 0.0, -18.0]),
        (UPPER_L, [20.0, 0.0, -34.0]),
        (FORE_L, [35.0, 0.0, 0.0]),
        (UPPER_R, [20.0, 0.0, 34.0]),
        (FORE_R, [35.0, 0.0, 0.0]),
    ]);
    let front_gather = stance.with(&[
        (SPINE_J, [22.0, 0.0, 0.0]),
        (NECK_J, [-14.0, 0.0, 0.0]),
        (THIGH_L, [26.0, 0.0, -6.0]),
        (SHIN_L, [60.0, 0.0, 0.0]),
        (FOOT_L, [-34.0, 0.0, 6.0]),
        (THIGH_R, [20.0, 0.0, 6.0]),
        (SHIN_R, [55.0, 0.0, 0.0]),
        (FOOT_R, [-35.0, 0.0, -6.0]),
        (UPPER_L, [-25.0, 0.0, -20.0]),
        (FORE_L, [60.0, 0.0, 0.0]),
        (UPPER_R, [20.0, 0.0, 20.0]),
        (FORE_R, [60.0, 0.0, 0.0]),
    ]);
    let dash_front = OneShot(vec![
        (0.0, front_gather),
        (0.12, front_gather),
        // The burst: low and driving, the left knee up, the right leg straight out behind.
        (0.26, stand.with(&[
            (PELVIS, [0.0, -10.0, 0.0]),
            (SPINE_J, [38.0, 0.0, 0.0]),
            (CHEST_J, [6.0, 10.0, 0.0]),
            (NECK_J, [-28.0, 0.0, 0.0]),
            (THIGH_L, [75.0, 0.0, -4.0]),
            (SHIN_L, [100.0, 0.0, 0.0]),
            (FOOT_L, [-25.0, 0.0, 4.0]),
            (THIGH_R, [-35.0, 0.0, 4.0]),
            (SHIN_R, [10.0, 0.0, 0.0]),
            (FOOT_R, [45.0, 0.0, -4.0]),
            (UPPER_L, [-60.0, 0.0, -10.0]),
            (FORE_L, [90.0, 0.0, 0.0]),
            (UPPER_R, [70.0, 0.0, 10.0]),
            (FORE_R, [90.0, 0.0, 0.0]),
        ])),
        // The long lunge: the left foot lands well out in front, the right trails far back.
        (0.42, stand.with(&[
            (PELVIS, [0.0, -6.0, 0.0]),
            (SPINE_J, [30.0, 0.0, 0.0]),
            (CHEST_J, [4.0, 6.0, 0.0]),
            (NECK_J, [-22.0, 0.0, 0.0]),
            (THIGH_L, [45.0, 0.0, -4.0]),
            (SHIN_L, [30.0, 0.0, 0.0]),
            (FOOT_L, [-15.0, 0.0, 4.0]),
            (THIGH_R, [-40.0, 0.0, 4.0]),
            (SHIN_R, [60.0, 0.0, 0.0]),
            (FOOT_R, [25.0, 0.0, -4.0]),
            (UPPER_L, [45.0, 0.0, -15.0]),
            (FORE_L, [80.0, 0.0, 0.0]),
            (UPPER_R, [-40.0, 0.0, 15.0]),
            (FORE_R, [80.0, 0.0, 0.0]),
        ])),
        // Gathering back into a low stance as it slows.
        (0.62, stance.with(&[
            (SPINE_J, [20.0, 0.0, 0.0]),
            (THIGH_L, [28.0, 0.0, -6.0]),
            (SHIN_L, [55.0, 0.0, 0.0]),
            (FOOT_L, [-27.0, 0.0, 6.0]),
            (THIGH_R, [6.0, 0.0, 6.0]),
            (SHIN_R, [50.0, 0.0, 0.0]),
            (FOOT_R, [-40.0, 0.0, -6.0]),
        ])),
        (0.8, settle),
        (1.0, settle),
    ]);
    // The side dash in the recording is a low, wide side lunge: the right foot steps far out with
    // the knee deeply bent, the left leg pushes off and stays straight out to the other side, the
    // hips drop, the torso stays upright and the arms open wide.
    let side_gather = stance.with(&[
        (SPINE_J, [16.0, 0.0, 0.0]),
        (THIGH_L, [24.0, 0.0, -10.0]),
        (SHIN_L, [56.0, 0.0, 0.0]),
        (FOOT_L, [-32.0, 0.0, 10.0]),
        (THIGH_R, [18.0, 0.0, 10.0]),
        (SHIN_R, [44.0, 0.0, 0.0]),
        (FOOT_R, [-26.0, 0.0, -10.0]),
    ]);
    // The lead thigh is swung out to the side (yawed) so its knee bends over the foot.
    let side_lunge = stance.with(&[
        (SPINE_J, [8.0, 0.0, 8.0]),
        (CHEST_J, [2.0, 0.0, 2.0]),
        (NECK_J, [-8.0, 0.0, -10.0]),
        (THIGH_L, [2.0, 0.0, -36.0]),
        (SHIN_L, [3.0, 0.0, 0.0]),
        (FOOT_L, [-1.0, 0.0, 36.0]),
        (THIGH_R, [55.0, -90.0, 0.0]),
        (SHIN_R, [60.0, 0.0, 0.0]),
        (FOOT_R, [-5.0, 70.0, 0.0]),
        (UPPER_L, [10.0, 0.0, -70.0]),
        (FORE_L, [20.0, 0.0, 0.0]),
        (UPPER_R, [10.0, 0.0, 62.0]),
        (FORE_R, [25.0, 0.0, 0.0]),
    ]);
    let dash_side = OneShot(vec![
        (0.0, side_gather),
        (0.14, side_gather),
        // The push-off: the left leg drives, the right steps out.
        (0.21, stance.with(&[
            (PELVIS, [0.0, 0.0, 4.0]),
            (SPINE_J, [10.0, 0.0, 6.0]),
            (THIGH_L, [8.0, 0.0, -24.0]),
            (SHIN_L, [14.0, 0.0, 0.0]),
            (FOOT_L, [14.0, 0.0, 24.0]),
            (THIGH_R, [40.0, -60.0, 0.0]),
            (SHIN_R, [55.0, 0.0, 0.0]),
            (FOOT_R, [-10.0, 50.0, 0.0]),
            (UPPER_L, [10.0, 0.0, -55.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [10.0, 0.0, 50.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ])),
        (0.3, side_lunge),
        (0.46, side_lunge.with(&[(THIGH_L, [2.0, 0.0, -34.0]), (SHIN_L, [4.0, 0.0, 0.0]), (FOOT_L, [-2.0, 0.0, 34.0]), (THIGH_R, [50.0, -90.0, 0.0]), (SHIN_R, [52.0, 0.0, 0.0])])),
        // The left leg comes in under you.
        (0.62, stance.with(&[
            (SPINE_J, [14.0, 0.0, 4.0]),
            (THIGH_L, [18.0, 0.0, -20.0]),
            (SHIN_L, [42.0, 0.0, 0.0]),
            (FOOT_L, [-24.0, 0.0, 20.0]),
            (THIGH_R, [30.0, -40.0, 0.0]),
            (SHIN_R, [48.0, 0.0, 0.0]),
            (FOOT_R, [-20.0, 30.0, 0.0]),
            (UPPER_L, [15.0, 0.0, -45.0]),
            (FORE_L, [30.0, 0.0, 0.0]),
            (UPPER_R, [15.0, 0.0, 45.0]),
            (FORE_R, [30.0, 0.0, 0.0]),
        ])),
        (0.75, settle),
        (1.0, settle),
    ]);
    let back_gather = stance.with(&[
        (SPINE_J, [20.0, 0.0, 0.0]),
        (THIGH_L, [26.0, 0.0, -8.0]),
        (SHIN_L, [60.0, 0.0, 0.0]),
        (FOOT_L, [-34.0, 0.0, 8.0]),
        (THIGH_R, [26.0, 0.0, 8.0]),
        (SHIN_R, [60.0, 0.0, 0.0]),
        (FOOT_R, [-34.0, 0.0, -8.0]),
    ]);
    let dash_back = OneShot(vec![
        (0.0, back_gather),
        (0.12, back_gather),
        (0.27, stance.with(&[
            (SPINE_J, [22.0, 0.0, 0.0]),
            (NECK_J, [-14.0, 0.0, 0.0]),
            (THIGH_L, [-14.0, 0.0, -8.0]),
            (SHIN_L, [30.0, 0.0, 0.0]),
            (FOOT_L, [30.0, 0.0, 8.0]),
            (THIGH_R, [30.0, 0.0, 8.0]),
            (SHIN_R, [70.0, 0.0, 0.0]),
            (FOOT_R, [-30.0, 0.0, -8.0]),
            (UPPER_L, [45.0, 0.0, -30.0]),
            (FORE_L, [50.0, 0.0, 0.0]),
            (UPPER_R, [45.0, 0.0, 30.0]),
            (FORE_R, [50.0, 0.0, 0.0]),
        ])),
        (0.46, stance.with(&[
            (THIGH_L, [20.0, 0.0, -8.0]),
            (SHIN_L, [55.0, 0.0, 0.0]),
            (FOOT_L, [-35.0, 0.0, 8.0]),
            (THIGH_R, [-10.0, 0.0, 8.0]),
            (SHIN_R, [40.0, 0.0, 0.0]),
            (FOOT_R, [-20.0, 0.0, -8.0]),
        ])),
        (0.66, settle),
        (1.0, settle),
    ]);
    Clips {
        stand,
        kicks: [shot_kick, pass_kick, lob_kick],
        shot_side,
        shot_back,
        heel,
        flick,
        flick_side,
        touch,
        jump_head,
        land,
        start,
        stop,
        turn,
        control,
        dribble_forward,
        dribble_side,
        dribble_back,
        feint,
        roulette,
        turn_out,
        lock,
        sole,
        brake,
        dash_front,
        dash_side,
        dash_back,
        cycles: vec![
            (Cycle::Idle, Clip::held(stand)),
            (Cycle::Walk, walk),
            (Cycle::Run, run),
            (Cycle::Sprint, sprint),
            (Cycle::Exhausted, exhausted),
            (Cycle::ExtraEffort, extra),
            (Cycle::LockIdle, Clip::held(lock)),
            (Cycle::LockForward, lock_forward),
            (Cycle::LockRight, lock_right),
            (Cycle::LockLeft, lock_right.mirror()),
            (Cycle::LockBack, lock_back),
        ],
    }
});

/// The cycle for the mode and speed (notes/animation.md §2). Lock move picks a cardinal by the
/// movement's angle to the facing (0 ahead, +90 right), in ±45° sectors, keeping the current one
/// up to ±60°.
fn pick_cycle(p: &Player, speed: f32, angle: f32, current: Cycle) -> Cycle {
    if p.locked {
        if speed < 0.3 {
            return Cycle::LockIdle;
        }
        let off = |centre: f32| ((angle - centre + 540.0).rem_euclid(360.0) - 180.0).abs();
        let cardinals = [(Cycle::LockForward, 0.0), (Cycle::LockRight, 90.0), (Cycle::LockBack, 180.0), (Cycle::LockLeft, -90.0)];
        if cardinals.iter().any(|&(c, centre)| c == current && off(centre) <= 60.0) {
            return current;
        }
        return cardinals.iter().find(|(_, centre)| off(*centre) <= 45.0).map_or(Cycle::LockForward, |(c, _)| *c);
    }
    match () {
        _ if speed < 0.3 => Cycle::Idle,
        _ if p.extra_active => Cycle::ExtraEffort,
        _ if p.sprint_running && p.exhausted => Cycle::Exhausted,
        _ if p.sprint_running => Cycle::Sprint,
        _ if speed < 2.7 => Cycle::Walk,
        _ => Cycle::Run,
    }
}

/// The cycle's play rate (notes/animation.md §2): the speed against its own, clamped to 0.7–1.5,
/// faster into sharp turns.
fn play_rate(speed: f32, clip: &Clip, corner: f32) -> f32 {
    if clip.speed <= 0.0 { 1.0 } else { (speed / clip.speed).clamp(0.7, 1.5) * corner }
}


/// A kick's follow-through after its contact, to where the run picks up again (a power shot's
/// in Kev's practice recording; lighter kicks go through the same in less time).
fn follow_time(k: &KickState) -> f32 {
    match k.kind {
        // A jumping volley follows through until it lands.
        _ if k.volley.is_some_and(|v| v.jump > 0.0) => VOLLEY_AIR.1 + 0.15,
        Kick::VolleyPushBall => 0.25,
        Kick::Shot => 0.5,
        Kick::PushBall | Kick::DribblePushBall => 0.2,
        // The knee stays up a moment after the scoop.
        Kick::LobbedPushBall | Kick::DribbleLobbedPushBall => 0.3,
        // Passes and lobs are back in their run 0.15-0.28 s after the touch.
        _ => 0.25,
    }
}

/// How big a kick's swing is, against a full shot.
fn kick_size(kind: Kick) -> f32 {
    match kind {
        Kick::Shot => 1.0,
        Kick::Pass | Kick::Lob => 1.0,
        Kick::ShortPass | Kick::ShortLob => 0.75,
        Kick::PushBall | Kick::DribblePushBall => 0.4,
        Kick::LobbedPushBall | Kick::DribbleLobbedPushBall => 0.9,
        Kick::VolleyPushBall => 0.85,
    }
}

/// A volley's keys by the body part it meets the ball with, for the right side.
fn volley_keys(clips: &Clips, kind: Kick, part: Part) -> [Pose; 3] {
    let _ = kind;
    match part {
        Part::Foot => clips.touch[0],
        Part::Thigh => clips.touch[1],
        Part::Pelvis => clips.touch[2],
        Part::Chest => clips.touch[3],
        Part::Head => clips.touch[4],
        Part::JumpChest | Part::JumpHead => clips.jump_head,
    }
}

#[derive(Default)]
pub struct Animator {
    phase: f32,
    cycle: Option<Cycle>,
    /// The cycle being faded out, and the fade's progress and length.
    from: Option<Cycle>,
    fade: f32,
    fade_time: f32,
    mode: (bool, bool),
    breath: f32,
    /// Smoothed sideways turn of the travel direction, and last frame's direction.
    lateral: f32,
    heading: Vec3,
    kick: Option<KickState>,
    follow: Option<(f32, KickState)>,
    action: f32,
    /// The kick pose last shown, kept to fade out from once the kick is over.
    kick_last: Option<(Pose, KickState, f32)>,
    /// The kick's sector against the body (degrees, left +) and its charge (0..1): they pick its
    /// clip, its turn and the size of its backswing.
    kick_turn: f32,
    kick_power: f32,
    trans: Option<Transition>,
    turn_u: f32,
    /// The pose a transition fades from, and the fade (1 → 0) and its length.
    snap: Pose,
    snap_w: f32,
    snap_time: f32,
    /// Last frame's pose before the layers on top, and its speed.
    last: Pose,
    prev_speed: f32,
    /// The pelvis's drop toward the ground, smoothed so the body doesn't bob step by step.
    drop: f32,
    /// The ball control's weight, and the lean, both eased.
    control: f32,
    roll: f32,
    /// The move on top (a dribble or a dash): its pose, weight, side and when it began.
    move_pose: Pose,
    /// The roulette's spin (degrees), and the sole-on-the-ball weight.
    spin: f32,
    sole: f32,
    /// Lock move's braking crouch, eased (0..1).
    brake: f32,
    move_w: f32,
    move_mirror: bool,
    move_t: f32,
    /// The pose shown last frame (for MAX_JOINT_SPEED).
    shown: Pose,
    shown_ready: bool,
    /// A chained move crossfades from the last one's pose.
    move_prev: Pose,
    move_fade: f32,
}

impl Animator {
    /// Starts a transition, fading from the current pose: 0.1 s from one of the same kind, 0.2 s
    /// otherwise.
    fn begin(&mut self, kind: Move, mirror: bool, size: f32, entry: f32) {
        let blend = if self.trans.is_some_and(|t| t.kind == kind) { 0.1 } else { 0.2 };
        self.fade_from_last(blend);
        self.trans = Some(Transition { kind, mirror, size, t: 0.0, u: 0.0, entry });
    }

    /// Cuts the transition short, fading back over `time`.
    fn interrupt(&mut self, time: f32) {
        if self.trans.take().is_some() {
            self.fade_from_last(time);
        }
    }

    fn fade_from_last(&mut self, time: f32) {
        self.snap = self.last;
        self.snap_w = 1.0;
        self.snap_time = time;
    }
}

/// A turn's progress mapped onto its pose's timeline, where the brake ends at 0.45.
fn turn_progress(u: f32, brake: f32) -> f32 {
    if u < brake { 0.45 * u / brake.max(1e-3) } else { 0.45 + 0.55 * (u - brake) / (1.0 - brake).max(1e-3) }
}

pub fn spawn_body(commands: &mut Commands, player: Entity, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let mut flat = |c: Color| materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.9, ..default() });
    // A blue kit (Kev): royal blue shirt, navy shorts, white socks.
    let shirt = flat(Color::srgb(0.10, 0.30, 0.85));
    let shorts = flat(Color::srgb(0.05, 0.10, 0.32));
    let skin = flat(Color::srgb(0.80, 0.62, 0.5));
    let socks = flat(Color::srgb(0.92, 0.92, 0.9));
    let boots = flat(Color::srgb(0.06, 0.06, 0.07));
    commands.insert_resource(crate::fx::LegMaterials([socks.clone(), boots.clone()]));
    // A capsule of radius r along a bone of length len, hanging down from its joint.
    let mut limb = |r: f32, len: f32| (Mesh3d(meshes.add(Capsule3d::new(r, (len - r).max(0.01)))), Transform::from_xyz(0.0, -len / 2.0, 0.0));
    // Sturdy limbs (Kev wants a more robust player than the bare physics radii).
    let thigh = limb(0.094, THIGH);
    let shin = limb(0.068, SHIN);
    let upper = limb(0.054, UPPER_ARM);
    let fore = limb(0.046, FOREARM);
    // The trunk is one smooth shape, wider than deep, from the hips to the base of the neck; the
    // shorts sit over the pelvis and the shoulders are rounded off.
    let mut trunk = |r: f32, len: f32, at: Vec3, scale: Vec3| (Mesh3d(meshes.add(Capsule3d::new(r, len))), Transform::from_translation(at).with_scale(scale));
    let pelvis_mesh = trunk(0.118, 0.08, Vec3::new(0.0, -0.03, 0.0), Vec3::new(1.3, 1.0, 0.84));
    let chest_mesh = trunk(0.132, 0.2, Vec3::new(0.0, -0.02, -0.005), Vec3::new(1.36, 1.0, 0.76));
    let shoulder_mesh = Mesh3d(meshes.add(Sphere::new(0.07)));
    let neck_mesh = Mesh3d(meshes.add(Capsule3d::new(0.065, 0.05)));
    let head_mesh = Mesh3d(meshes.add(Sphere::new(0.108)));
    let hand = Mesh3d(meshes.add(Capsule3d::new(0.04, 0.08)));
    let foot = Mesh3d(meshes.add(Cuboid::new(0.11, 0.09, 0.28)));
    let joint = |j: usize, at: Vec3| (Joint(j), Transform::from_translation(at), Visibility::default());

    commands.entity(player).with_children(|root| {
        root.spawn(joint(PELVIS, Vec3::Y * (PELVIS_HEIGHT - ROOT_HEIGHT))).with_children(|pelvis| {
            pelvis.spawn((pelvis_mesh.0, MeshMaterial3d(shorts.clone()), pelvis_mesh.1));
            for (t, s, f, side) in [(THIGH_L, SHIN_L, FOOT_L, -1.0f32), (THIGH_R, SHIN_R, FOOT_R, 1.0)] {
                pelvis.spawn(joint(t, Vec3::new(side * HIP.x, HIP.y, HIP.z))).with_children(|hip| {
                    hip.spawn((thigh.0.clone(), MeshMaterial3d(shorts.clone()), thigh.1));
                    hip.spawn(joint(s, Vec3::NEG_Y * THIGH)).with_children(|knee| {
                        knee.spawn((shin.0.clone(), MeshMaterial3d(socks.clone()), shin.1));
                        knee.spawn(joint(f, Vec3::NEG_Y * SHIN)).with_children(|ankle| {
                            ankle.spawn((foot.clone(), MeshMaterial3d(boots.clone()), Transform::from_xyz(0.0, -ANKLE_HEIGHT + 0.0425, -0.07), TrailPoint(side)));
                        });
                    });
                });
            }
            pelvis.spawn(joint(SPINE_J, Vec3::Y * SPINE)).with_children(|spine| {
                spine.spawn(joint(CHEST_J, Vec3::Y * CHEST)).with_children(|chest| {
                    chest.spawn((chest_mesh.0, MeshMaterial3d(shirt.clone()), chest_mesh.1));
                    for side in [-1.0f32, 1.0] {
                        chest.spawn((shoulder_mesh.clone(), MeshMaterial3d(shirt.clone()), Transform::from_xyz(side * 0.15, 0.15, 0.0)));
                    }
                    chest.spawn(joint(NECK_J, Vec3::Y * NECK)).with_children(|neck| {
                        neck.spawn((neck_mesh, MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 0.03, 0.0)));
                        neck.spawn(joint(HEAD_J, Vec3::Y * HEAD)).with_children(|head| {
                            head.spawn((head_mesh, MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 0.095, -0.01).with_scale(Vec3::new(0.9, 1.05, 1.0))));
                        });
                    });
                    for (u, f, h, side) in [(UPPER_L, FORE_L, HAND_L, -1.0f32), (UPPER_R, FORE_R, HAND_R, 1.0)] {
                        chest.spawn(joint(u, Vec3::new(side * SHOULDER.x, SHOULDER.y, SHOULDER.z))).with_children(|shoulder| {
                            shoulder.spawn((upper.0.clone(), MeshMaterial3d(shirt.clone()), upper.1));
                            shoulder.spawn(joint(f, Vec3::NEG_Y * UPPER_ARM)).with_children(|elbow| {
                                elbow.spawn((fore.0.clone(), MeshMaterial3d(skin.clone()), fore.1));
                                elbow.spawn(joint(h, Vec3::NEG_Y * FOREARM)).with_children(|wrist| {
                                    wrist.spawn((hand.clone(), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, -0.07, 0.0)));
                                });
                            });
                        });
                    }
                });
            });
        });
    });
}

/// Poses the body from the player's state.
pub fn animate_body(time: Res<Time>, player: Single<(&Player, &Transform)>, mut joints: Query<(&Joint, &mut Transform), Without<Player>>, mut anim: Local<Animator>) {
    let dt = time.delta_secs();
    let (p, root) = *player;
    // The kick's freeze holds the pose too.
    if dt <= 0.0 || p.freeze > 0.0 {
        return;
    }
    let clips = &*CLIPS;
    let anim: &mut Animator = &mut anim;
    let local = root.rotation.inverse() * p.vel;
    let speed = Vec2::new(local.x, local.z).length();
    let angle = if speed > 0.1 { local.x.atan2(-local.z).to_degrees() } else { 0.0 };

    // Cornering: the smoothed per-frame turn of the travel direction (the game's relative lateral
    // acceleration), positive turning right; it speeds the cycle up and leans the body.
    let heading = Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero();
    let turn = if speed > 0.5 && anim.heading != Vec3::ZERO { heading.dot(anim.heading.cross(Vec3::Y)) } else { 0.0 };
    anim.heading = heading;
    anim.lateral += (turn - anim.lateral) * (7.0 * dt).min(1.0);

    // The cycle; a change fades the old one out, faster when the mode changed.
    let next = pick_cycle(p, speed, angle, anim.cycle.unwrap_or(Cycle::Idle));
    let mode = (p.locked, p.has_ball);
    if anim.cycle != Some(next) {
        anim.from = anim.cycle;
        anim.fade = 0.0;
        anim.fade_time = if mode != anim.mode { 0.1 } else { 0.25 };
        anim.cycle = Some(next);
    }
    anim.mode = mode;
    anim.fade = (anim.fade + dt / anim.fade_time.max(1e-3)).min(1.0);
    let clip = clips.get(next);
    let boost_max = if p.has_ball { 1.0 } else if p.sprint_running { 1.05 } else { 1.15 };
    let rate = play_rate(speed, clip, 1.0 + (boost_max - 1.0) * (anim.lateral.abs() / 0.06).min(1.0));
    if clip.speed > 0.0 {
        anim.phase = (anim.phase + dt * rate / clip.duration).rem_euclid(1.0);
    }

    // Transitions (notes/animation.md §2): turns follow the movement's own turn; starts leave a
    // standstill; stops brake with no input. Lock move has none yet, and any action (a kick, a
    // ball control) cuts the running one at once, as in the game.
    let input = p.last_input != Vec3::ZERO;
    if p.locked || p.kick.is_some() || p.receiving.is_some() {
        anim.interrupt(0.1);
    } else if let Some(tr) = p.turn {
        let (u, brake, right, big) = tr.progress();
        if anim.trans.is_none_or(|t| t.kind != Move::Turn) || u + 0.05 < anim.turn_u {
            anim.begin(Move::Turn, !right, if big { 1.0 } else { 0.6 }, 0.0);
        }
        anim.turn_u = u;
        if let Some(t) = anim.trans.as_mut() {
            t.u = turn_progress(u, brake);
        }
    } else {
        anim.turn_u = 0.0;
        match anim.trans.map(|t| t.kind) {
            // A turn that ends early, a start let go, a stop pushed out of: back to the cycle.
            Some(Move::Turn) => {
                if anim.trans.is_some_and(|t| t.u < 0.95) {
                    anim.interrupt(0.1);
                } else {
                    anim.trans = None;
                }
            }
            Some(Move::Start) if !input => anim.interrupt(0.2),
            Some(Move::Stop) if input => anim.interrupt(0.2),
            _ => {}
        }
        if anim.trans.is_none() && input && anim.prev_speed < 0.3 && speed >= 0.3 {
            anim.begin(Move::Start, angle > 0.0, if p.sprinting { 1.0 } else { 0.8 }, 0.0);
        } else if anim.trans.is_none() && !input && speed > 1.5 && speed <= anim.prev_speed {
            // Plant the foot that lands next.
            let next_right = (0.25..0.75).contains(&anim.phase);
            anim.begin(Move::Stop, next_right, (speed / RUN_SPEED).clamp(0.4, 1.0), speed);
        }
    }
    if let Some(t) = anim.trans.as_mut() {
        t.t += dt;
        match t.kind {
            Move::Start => t.u = (t.t / START_TIME).min(1.0),
            Move::Stop => t.u = (t.t / STOP_TIME).max(1.0 - speed / t.entry.max(0.1)).min(1.0),
            Move::Turn => {}
        }
        // Starts and turns run the cycle in step under their last stretch.
        if t.kind != Move::Stop {
            anim.phase = (if t.mirror { 0.5 } else { 0.0 } + (t.u - 1.0) * 0.5).rem_euclid(1.0);
        }
        if t.kind != Move::Turn && t.u >= 1.0 {
            anim.trans = None;
        }
    }

    let phase = anim.phase;
    let mut pose = clip.sample(phase);
    let mut lift = clip.lift(phase);
    if let Some(from) = anim.from.filter(|_| anim.fade < 1.0) {
        let w = anim.fade * anim.fade * (3.0 - 2.0 * anim.fade);
        let old = clips.get(from);
        pose = old.sample(phase).lerp(&pose, w);
        lift = old.lift(phase) * (1.0 - w) + lift * w;
    }

    // What the rate clamp leaves, the steps absorb (the game's stride warping), toward the stance
    // you stand in (lock move's wide one, so short lock steps don't bring the feet together).
    if clip.speed > 0.0 {
        let stride = (speed / clip.speed / rate).clamp(0.4, 1.3);
        let side = matches!(next, Cycle::LockRight | Cycle::LockLeft);
        let rest_pose = if matches!(next, Cycle::LockForward | Cycle::LockRight | Cycle::LockLeft | Cycle::LockBack) { &clips.lock } else { &clips.stand };
        for j in [THIGH_L, THIGH_R, UPPER_L, UPPER_R] {
            let rest = rest_pose.0[j];
            pose.0[j].x = rest.x + (pose.0[j].x - rest.x) * stride;
            if side && j <= THIGH_R {
                pose.0[j].z = rest.z + (pose.0[j].z - rest.z) * stride;
            }
        }
    }

    // Lock move: asking to go back, you brake in a deep crouch until you're backing away.
    let wants_back = (root.rotation.inverse() * p.last_input).z > 0.5;
    let braking = p.locked && wants_back && local.z < LOCK_BRAKE_UNTIL * LOCK_SPEED;
    anim.brake = if braking { (anim.brake + dt / LOCK_BRAKE_IN).min(1.0) } else { (anim.brake - dt / LOCK_BRAKE_OUT).max(0.0) };
    if anim.brake > 0.0 {
        let w = anim.brake * anim.brake * (3.0 - 2.0 * anim.brake);
        pose = pose.plus(&Pose(clips.brake.0.map(|a| a * w)));
    }

    if let Some(t) = anim.trans {
        let shot = match t.kind {
            Move::Start => &clips.start,
            Move::Stop => &clips.stop,
            Move::Turn => &clips.turn,
        };
        let mut target = shot.sample(t.u);
        if t.mirror {
            target = target.mirror();
        }
        // Starts and turns hand back to the cycle over their last 30%.
        let w = if t.kind == Move::Stop { 1.0 } else { ((1.0 - t.u) / 0.3).clamp(0.0, 1.0) };
        pose = pose.lerp(&target, w * w * (3.0 - 2.0 * w) * t.size);
    }
    if anim.snap_w > 0.0 {
        let w = anim.snap_w;
        pose = pose.lerp(&anim.snap, w * w * (3.0 - 2.0 * w));
        anim.snap_w = (w - dt / anim.snap_time.max(1e-3)).max(0.0);
    }
    anim.last = pose;
    anim.prev_speed = speed;

    // Breathing, mostly at rest.
    anim.breath = (anim.breath + dt / 3.5).rem_euclid(1.0);
    let breath = (TAU * anim.breath).sin() * if speed < 0.3 { 1.5 } else { 0.5 };
    pose.0[CHEST_J].x += breath;
    pose.0[UPPER_L].z -= breath * 0.5;
    pose.0[UPPER_R].z += breath * 0.5;

    // Free move has one forward cycle: the lower body turns toward the velocity and the upper body
    // keeps facing (the game's orientation warping).
    if !p.locked && speed > 0.5 {
        let in_turn = if anim.trans.is_some_and(|t| t.kind == Move::Turn) { 0.5 } else { 1.0 };
        let warp = -angle.clamp(-60.0, 60.0) * (speed / 2.0).min(1.0) * in_turn;
        pose.0[PELVIS].y += warp;
        pose.0[SPINE_J].y -= warp * 0.6;
        pose.0[CHEST_J].y -= warp * 0.4;
    }

    // Lean into turns, by speed, and forward as stamina drops.
    let lean_max = if p.sprint_running { 0.06 } else if p.has_ball { 0.1 } else { 0.075 };
    let lean = (anim.lateral / lean_max).clamp(-1.0, 1.0) * (speed / SPRINT_SPEED).min(1.0) * 14.0;
    anim.roll += (lean - anim.roll) * (8.0 * dt).min(1.0);
    let roll = anim.roll;
    pose.0[PELVIS].z += roll * 0.4;
    pose.0[SPINE_J].z += roll * 0.3;
    pose.0[CHEST_J].z += roll * 0.3;
    pose.0[SPINE_J].x += (1.0 - p.stamina / MAX_STAMINA).max(0.0) * 6.0;

    // Lock move with the ball: the right sole goes up onto the ball and rolls it the way you move,
    // the left foot shuffling underneath (the game's own lock-move ball cycles; players confirm the
    // foot goes on the ball).
    let on_ball = p.locked && p.has_ball && p.dribble.is_none() && p.dash.is_none() && p.kick.is_none() && p.receiving.is_none();
    anim.sole += (if on_ball { 1.0 } else { 0.0 } - anim.sole).clamp(-dt / 0.15, dt / 0.15);
    if anim.sole > 0.0 {
        let (ahead, side) = (-local.z / LOCK_BALL_SPEED, local.x / LOCK_BALL_SPEED);
        let (s, c) = (anim.phase * TAU).sin_cos();
        let mut sole = clips.sole;
        // The sole rolls the ball on and draws it back each step; sideways it sweeps across.
        sole.0[THIGH_R] += Vec3::new(10.0 * ahead * s, 0.0, 8.0 * side * s);
        sole.0[FOOT_R] += Vec3::new(-8.0 * ahead * s, 0.0, -8.0 * side * s);
        // The left foot hops under you in time.
        let hop = c.abs() * (ahead.abs() + side.abs()).min(1.0);
        sole.0[THIGH_L] += Vec3::new(8.0 * ahead * c + 6.0 * hop, 0.0, -6.0 * side * c);
        sole.0[SHIN_L] += Vec3::new(18.0 * hop, 0.0, 0.0);
        sole.0[FOOT_L] += Vec3::new(-10.0 * hop, 0.0, 0.0);
        let w = anim.sole * anim.sole * (3.0 - 2.0 * anim.sole);
        pose = pose.lerp(&sole, w);
    }

    // Kicks on top: wind up while charging, swing to the contact frame, follow through.
    match (anim.kick, p.kick) {
        (Some(was), None) if p.fake.is_none() => anim.follow = Some((0.0, was)),
        (_, Some(_)) => anim.follow = None,
        _ => {}
    }
    anim.kick = p.kick;
    // The follow-through holds while the kick still slides you on (its drag), then fades; running
    // on out of it, it fades quicker into the run.
    let run_out = speed > 1.5 && !p.kick_sliding();
    if let Some((t, k)) = anim.follow.as_mut() {
        *t += dt;
        if *t > follow_time(k) && !p.kick_sliding() {
            anim.follow = None;
        }
    }
    // The kick's clip by its sector (the game has one per sector, the right side its left one
    // mirrored onto the other foot): shots forward, across the body or turning back; passes and
    // lobs open toward their target, a heel flick going back. With it, how much of the sector's
    // turn the body adds on top (the side and back clips carry their own).
    let keys = |k: &KickState, turn: f32| -> ([Pose; 3], f32) {
        // A volley's by the part it meets the ball with, the left side's mirrored.
        if let Some(v) = k.volley {
            let set = volley_keys(clips, k.kind, v.part);
            return (if v.left { set.map(|p| p.mirror()) } else { set }, 0.0);
        }
        let kind = k.kind;
        let side = |set: &[Pose; 3]| if turn < 0.0 { set.map(|p| p.mirror()) } else { *set };
        match kind {
            Kick::Shot if turn.abs() > 120.0 => (side(&clips.shot_back), 0.0),
            Kick::Shot if turn.abs() > 30.0 => (side(&clips.shot_side), 0.3),
            Kick::Shot => (clips.kicks[0], 1.0),
            _ if turn.abs() > 135.0 => (clips.heel, 0.0),
            _ if kind.is_flick() && turn.abs() > 45.0 => (side(&clips.flick_side), 0.3),
            _ if kind.is_flick() => (clips.flick, 1.0),

            Kick::Pass | Kick::ShortPass => (clips.kicks[1], 1.0),
            Kick::Lob | Kick::ShortLob => (clips.kicks[2], 1.0),
            _ => (clips.kicks[0], 1.0),
        }
    };
    // Push balls are a touch on top of the run (below), not a kick pose.
    let push = |kind: Kick| matches!(kind, Kick::PushBall | Kick::DribblePushBall);
    // The kick's sector: a shot's aim at the press, a pass or lob's stick (its aim without one),
    // against the body. Each of the game's clips carries its own turn that way.
    let wrap = |a: f32| (a + PI).rem_euclid(TAU) - PI;
    if let Some(k) = p.kick {
        // A flick with no stick goes the way you face.
        let aim = if k.kind != Kick::Shot && k.steer != Vec3::ZERO {
            yaw_of(k.steer)
        } else if k.kind.is_flick() {
            p.facing
        } else {
            k.aim.x.to_radians()
        };
        anim.kick_turn = wrap(aim - p.facing).to_degrees().clamp(-150.0, 150.0);
        anim.kick_power = charge(k.kind, k.held);
    }
    // The leg cocks back within WINDUP_IN and holds there (a long charge holds it until the
    // release); the strike from the top of the backswing to the ball takes the last 4 frames
    // before contact (Kev's Rematch clips: a power shot's leg reaches the ball in a frame or two of
    // 30 fps video). The body turns toward the sector a third of the way by the strike, most of it
    // by the hit.
    const STRIKE: f32 = 4.0 / 60.0;
    const WINDUP_IN: f32 = 0.12;
    let kick_pose = if let Some(k) = p.kick.filter(|k| !push(k.kind)) {
        let ([windup, hit, _], _) = keys(&k, anim.kick_turn);
        let first = contact_time(&k);
        let up = |t: f32, end: f32| {
            let u = (t / end.max(1.0 / 60.0)).min(1.0);
            1.0 - (1.0 - u) * (1.0 - u)
        };
        // Passes and lobs don't cock the leg early or hold it while they charge: you run on until
        // the standing foot plants PASS_PLANT before the strike, then the leg swings back and
        // through (Kev's practice recording).
        // A volley sets itself VOLLEY_SET before its touch (a jump crouches as it takes off), the
        // run cycle playing as you get there until then.
        const PASS_PLANT: f32 = 0.06;
        let late = k.kind.is_pass() || k.kind.is_flick() || k.volley.is_some();
        let plant = match k.volley {
            Some(v) if v.jump > 0.0 => VOLLEY_AIR.0,
            Some(_) => VOLLEY_SET,
            None => PASS_PLANT,
        };
        let release = (k.held - k.pre).max(0.0);
        let strike_at = (first - STRIKE).max(if k.released { release } else { f32::INFINITY });
        if late && k.elapsed < strike_at - plant {
            None
        } else {
        let (key, turn) = if !k.released && !late {
            let w = up(k.elapsed, (first - STRIKE).min(WINDUP_IN));
            (clips.stand.lerp(&windup, w), 0.3 * w)
        } else {
            if k.elapsed < strike_at {
                let w = if late { up(k.elapsed - (strike_at - plant), plant) } else { up(k.elapsed, strike_at.min(WINDUP_IN)) };
                (clips.stand.lerp(&windup, w), 0.3 * w)
            } else {
                let s = ((k.elapsed - strike_at) / (first.max(release) - strike_at).max(4.0 / 60.0)).clamp(0.0, 1.0);
                let s = s * s * (3.0 - 2.0 * s);
                (windup.lerp(&hit, s), 0.3 + 0.4 * s)
            }
        };
        Some((key, k, turn))
        }
    } else {
        // After the touch, as in Kev's practice recording of power shots on the run: the leg drives
        // up high by a fifth of the follow-through (0.1 s) with the body leaning back, stays up to
        // two fifths, comes down into a long lunging stride by seven tenths, and the run picks up
        // from it at the end (held while the kick slides you).
        anim.follow.filter(|(_, k)| !push(k.kind)).map(|(t, k)| {
            let kind = k.kind;
            let ([_, hit, through], _) = keys(&k, anim.kick_turn);
            let u = t / follow_time(&k);
            let a = (u / 0.2).min(1.0);
            let a = 1.0 - (1.0 - a) * (1.0 - a);
            let b = ((u - 0.4) / 0.3).clamp(0.0, 1.0);
            let b = b * b * (3.0 - 2.0 * b);
            // A shot across or back to the right is the left foot's: it lands on the left. A pass
            // or lob lands in a plain stride, not the shot's lunge.
            let left_foot = kind == Kick::Shot && anim.kick_turn < -30.0;
            let land = if left_foot { clips.land.mirror() } else { clips.land };
            let land = if kind == Kick::Shot { land } else { clips.stand.lerp(&land, 0.4) };
            (hit.lerp(&through, a).lerp(&land, b), k, 0.7 + 0.15 * a)
        })
    };
    // A push ball: the right foot reaches through the ball by its touch (8f) and back, the legs
    // running on underneath.
    let push_touch = match (p.kick.filter(|k| push(k.kind)), anim.follow.filter(|(_, k)| push(k.kind))) {
        (Some(k), _) => (k.elapsed / contact_time(&k).max(1.0 / 60.0)).min(1.0),
        (_, Some((t, k))) => 1.0 - t / follow_time(&k),
        _ => 0.0,
    };
    if push_touch > 0.0 {
        let w = push_touch * push_touch * (3.0 - 2.0 * push_touch) * 1.2;
        pose = pose.plus(&Pose(clips.control.0.map(|a| a * w)));
    }
    // A fake shot: the kicking leg swings low over the ball, then you drop into a deep, wide crouch
    // over it with the arms out low, rising as you can act again (Kev's practice recording).
    if let Some((t, lock)) = p.fake {
        let swing = clips.stand.with(&[
            (SPINE_J, [22.0, 0.0, -4.0]),
            (THIGH_L, [28.0, 0.0, -8.0]),
            (SHIN_L, [55.0, 0.0, 0.0]),
            (FOOT_L, [-28.0, 0.0, 6.0]),
            (THIGH_R, [25.0, -20.0, 10.0]),
            (SHIN_R, [45.0, 0.0, 0.0]),
            (FOOT_R, [-10.0, 0.0, 0.0]),
            (UPPER_L, [10.0, 0.0, -80.0]),
            (UPPER_R, [-10.0, 0.0, 80.0]),
        ]);
        let crouch = clips.stand.with(&[
            (SPINE_J, [28.0, 0.0, 0.0]),
            (NECK_J, [-16.0, 0.0, 0.0]),
            (THIGH_L, [50.0, 0.0, -14.0]),
            (SHIN_L, [95.0, 0.0, 0.0]),
            (FOOT_L, [-45.0, 0.0, 14.0]),
            (THIGH_R, [50.0, 0.0, 14.0]),
            (SHIN_R, [95.0, 0.0, 0.0]),
            (FOOT_R, [-45.0, 0.0, -14.0]),
            (UPPER_L, [20.0, 0.0, -55.0]),
            (FORE_L, [20.0, 0.0, 0.0]),
            (UPPER_R, [20.0, 0.0, 55.0]),
            (FORE_R, [20.0, 0.0, 0.0]),
        ]);
        let into = ((t - FAKE_SHOT_SWING) / 0.12).clamp(0.0, 1.0);
        let key = swing.lerp(&crouch, into * into * (3.0 - 2.0 * into));
        let w = (t / 0.05).min(1.0) * ((lock - t) / 0.12).clamp(0.0, 1.0);
        pose = pose.lerp(&key, w * w * (3.0 - 2.0 * w));
    }

    // In over 0.08 s, out over 0.25 s (0.15 s running on) from the last pose shown, so the kick
    // never snaps off.
    let target = if kick_pose.is_some() { 1.0 } else { 0.0 };
    anim.action += (target - anim.action).clamp(-dt / if run_out { 0.15 } else { 0.25 }, dt / 0.08);
    if kick_pose.is_some() {
        anim.kick_last = kick_pose;
    } else if anim.action <= 0.0 {
        anim.kick_last = None;
    }
    if let Some((key, k, turn)) = anim.kick_last {
        // Power shows in the swing's size: three quarters for a tap, all of it fully charged.
        let size = kick_size(k.kind) * (0.75 + 0.25 * anim.kick_power);
        let w = anim.action * anim.action * (3.0 - 2.0 * anim.action);
        pose = pose.lerp(&clips.stand.lerp(&key, size), w);
        pose.0[PELVIS].y += anim.kick_turn * turn * keys(&k, anim.kick_turn).1 * w;
    }
    // A jumping volley leaves the ground around its touch: up and down on a parabola over its
    // flight (VOLLEY_AIR), as high as it needs.
    let air = match (p.kick, anim.follow) {
        (Some(k), _) => k.volley.map(|v| (k.elapsed - v.t, v.jump)),
        (None, Some((t, k))) => k.volley.map(|v| (t, v.jump)),
        _ => None,
    };
    if let Some((tau, jump)) = air.filter(|(_, jump)| *jump > 0.0) {
        let (before, after) = VOLLEY_AIR;
        let u = (tau + before) / (before + after);
        if (0.0..=1.0).contains(&u) {
            lift += jump * 4.0 * u * (1.0 - u);
        }
    }

    // Dribbles and dashes on top (the game's move montages): in over 0.05 s (dashes 0.1 s), out over
    // BLEND_OUT from when you get the stick back (the montage's blend-out, not its end), faster
    // under a kick. A chained move crossfades from the last one over 0.1 s (the feint's blend-in).
    // A dribble's touch lands on its contact frame and its keys finish by the hand-back; the side is
    // picked as the move starts.
    let local_dir = |d: Vec3| root.rotation.inverse() * d;
    let mut moving = None;
    let mut started = false;
    if let Some(d) = p.dribble.filter(|d| d.moving()) {
        started = d.t < anim.move_t || anim.move_w <= 0.0;
        if started {
            // The roulette always turns to the left; the turn-out goes the cut's way.
            anim.move_mirror = match d.turn {
                FeintTurn::Roulette => false,
                FeintTurn::TurnOut(side) => side < 0.0,
                FeintTurn::None => local_dir(d.dir).x < 0.0,
            };
        }
        anim.move_t = d.t;
        let (shot, contact_at) = match (d.feint, d.heading, d.turn) {
            (_, _, FeintTurn::Roulette) => &clips.roulette,
            (_, _, FeintTurn::TurnOut(_)) => &clips.turn_out,
            (true, ..) => &clips.feint,
            (_, Heading::Forward, _) => &clips.dribble_forward,
            (_, Heading::Side, _) => &clips.dribble_side,
            (_, Heading::Back, _) => &clips.dribble_back,
        };
        let contact = dribble_contact(&d);
        let u = if d.t < contact {
            contact_at * d.t / contact
        } else {
            contact_at + (1.0 - contact_at) * ((d.t - contact) / (d.free() - contact).max(0.05)).min(1.0)
        };
        moving = Some((shot.sample(u), 0.05));
        // The roulette's full turn to the left (Kev: 360 like in the recording), or the turn-out's
        // quarter turn out the cut's way and back (left is +).
        match d.turn {
            FeintTurn::Roulette => anim.spin = roulette_turn(d.t).rem_euclid(360.0),
            FeintTurn::TurnOut(side) => anim.spin = -side * turn_out_angle(d.t),
            FeintTurn::None => {}
        }
    } else if let Some(d) = p.dash {
        started = d.t < anim.move_t || anim.move_w <= 0.0;
        if started {
            anim.move_mirror = local_dir(d.dir).x < 0.0;
        }
        anim.move_t = d.t;
        let shot = match d.heading {
            Heading::Forward => &clips.dash_front,
            Heading::Side => &clips.dash_side,
            Heading::Back => &clips.dash_back,
        };
        let clip = if d.second { [60.0, 42.0, 43.0] } else { [50.0, 41.0, 50.0] }[d.heading as usize] / 60.0;
        moving = Some((shot.sample((d.t / clip).min(1.0)), 0.1));
    } else if let (Some(t), Some(_)) = (p.receiving, p.receive_way) {
        // The ippy: as you swing round, the foot behind reaches back onto the ball and drags it
        // round with the sole (the drag back's touch), over the control's first 0.35 s.
        started = t < anim.move_t || anim.move_w <= 0.0;
        if started {
            anim.move_mirror = false;
        }
        anim.move_t = t;
        moving = Some((clips.dribble_back.0.sample((t / 0.35).min(1.0)), 0.05));
    } else if p.ee_cut > 0.0 && p.extra_active {
        // An extra-effort cut: the side dash's plant, push-off and landing, the way you cut.
        let t = EE_CUT_TIME - p.ee_cut;
        started = t < anim.move_t || anim.move_w <= 0.0;
        if started {
            anim.move_mirror = !p.ee_cut_right;
        }
        anim.move_t = t;
        moving = Some((clips.dash_side.sample(0.17 + 0.33 * t / EE_CUT_TIME), 0.04));
    } else {
        anim.move_t = f32::INFINITY;
    }
    if let Some((target, blend_in)) = moving {
        if started {
            anim.move_prev = anim.move_pose;
            anim.move_fade = if anim.move_w > 0.0 { 0.0 } else { 1.0 };
        }
        let target = if anim.move_mirror { target.mirror() } else { target };
        anim.move_fade = (anim.move_fade + dt / 0.1).min(1.0);
        let f = anim.move_fade;
        anim.move_pose = anim.move_prev.lerp(&target, f * f * (3.0 - 2.0 * f));
        anim.move_w = (anim.move_w + dt / blend_in).min(1.0);
    } else {
        let out = if p.kick.is_some() { 0.1 } else { BLEND_OUT };
        anim.move_w = (anim.move_w - dt / out).max(0.0);
    }
    if anim.move_w > 0.0 {
        let w = anim.move_w;
        pose = pose.lerp(&anim.move_pose, w * w * (3.0 - 2.0 * w));
    }

    // Running with the ball, the right foot meets it at each touch (the game's touch layer).
    if p.has_ball
        && p.dribble.is_none()
        && p.kick.is_none()
        && let Some(c) = p.carry
    {
        let now = time.elapsed_secs();
        let w = (1.0 - (c.next - now) / 0.15).max(1.0 - (now - c.start) / 0.2).clamp(0.0, 1.0);
        let w = w * w * (3.0 - 2.0 * w) * 0.7;
        pose = pose.plus(&Pose(clips.control.0.map(|a| a * w)));
    }

    // Ball control: in over 0.1 s, back out over 0.25 s, on top of the cycle so the legs keep
    // running.
    let receiving = p.receiving.is_some();
    anim.control += (if receiving { 1.0 } else { 0.0 } - anim.control).clamp(-dt / 0.25, dt / 0.1);
    if anim.control > 0.0 {
        let w = anim.control * anim.control * (3.0 - 2.0 * anim.control);
        pose = pose.plus(&Pose(clips.control.0.map(|a| a * w)));
    }

    // The roulette's and turn-out's turn is on the whole body, added after the joint speed limit (a
    // full turn ends where it began); cut short, it turns back the short way over 0.1 s.
    if !p.dribble.is_some_and(|d| d.moving() && d.turn != FeintTurn::None) {
        anim.spin = (anim.spin + 180.0).rem_euclid(360.0) - 180.0;
        anim.spin -= anim.spin * (dt / 0.1).min(1.0);
    }

    // No joint turns faster than MAX_JOINT_SPEED: a layer starting or ending mid-pose can't snap a
    // limb in one frame (the start of a run flicked the right forearm 70 deg), while every authored
    // move (a kick's strike, a dash's burst) stays under it.
    if anim.shown_ready {
        let max = MAX_JOINT_SPEED * dt;
        for j in 0..JOINTS {
            let d = (pose.0[j] - anim.shown.0[j]).clamp(Vec3::splat(-max), Vec3::splat(max));
            pose.0[j] = anim.shown.0[j] + d;
        }
    }
    anim.shown = pose;
    anim.shown_ready = true;
    pose.0[PELVIS].y += anim.spin;

    // The feet stay on the ground: the pelvis follows the pose's drop closely (a slow follow left
    // deep moves like the fast cut floating, then sinking).
    anim.drop += (ground_drop(&pose) - anim.drop) * (30.0 * dt).min(1.0);
    let drop = anim.drop;
    for (j, mut tf) in &mut joints {
        tf.rotation = joint_rotation(j.0, pose.0[j.0]);
        if j.0 == PELVIS {
            tf.translation.y = PELVIS_HEIGHT - ROOT_HEIGHT - drop + lift;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirroring_twice_gives_the_same_pose() {
        let p = CLIPS.get(Cycle::Run).sample(0.3);
        assert_eq!(p.mirror().mirror(), p);
    }

    #[test]
    fn standing_body_has_rematch_sizes() {
        let stand = CLIPS.stand;
        let drop = ground_drop(&stand);
        assert!(drop < 0.01, "standing drops {drop}");
        let [l, r] = ankles(&stand);
        assert!((PELVIS_HEIGHT - drop + l.y.min(r.y) - 0.112).abs() < 0.01);
        // Head joint at 1.69 m, shoulders at 1.52 m.
        assert!((PELVIS_HEIGHT + SPINE + CHEST + NECK + HEAD - 1.689).abs() < 0.01);
        assert!((PELVIS_HEIGHT + SPINE + CHEST + SHOULDER.y - 1.518).abs() < 0.01);
    }

    #[test]
    fn run_feet_alternate_and_stay_on_the_ground() {
        let run = CLIPS.get(Cycle::Run);
        for (phase, front) in [(0.0, 0), (0.5, 1)] {
            let pose = run.sample(phase);
            let feet = ankles(&pose);
            assert!(feet[front].z < feet[1 - front].z - 0.3, "phase {phase}: {feet:?}");
            // At the contact the front foot is the one on the ground.
            let lowest = PELVIS_HEIGHT - ground_drop(&pose) + feet[front].y;
            assert!((lowest - ANKLE_HEIGHT).abs() < 0.02, "phase {phase}: ankle at {lowest}");
        }
        // Mid-stance bends the knee: the body is lower than standing.
        assert!(ground_drop(&run.sample(0.25)) > 0.04);
    }

    #[test]
    fn starts_and_turns_end_on_the_run_cycle() {
        let run = CLIPS.get(Cycle::Run);
        assert_eq!(CLIPS.start.sample(1.0), run.sample(0.0));
        assert_eq!(CLIPS.turn.sample(1.0), run.sample(0.0));
        assert_eq!(CLIPS.turn.sample(1.0).mirror(), run.sample(0.5));
        assert_eq!(CLIPS.stop.sample(1.0), CLIPS.stand);
        // The brake ends where the pose has its pivot, whatever its share of the turn.
        assert!((turn_progress(0.3, 0.3) - 0.45).abs() < 1e-6);
        assert!((turn_progress(1.0, 0.6) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn play_rate_is_clamped() {
        let run = CLIPS.get(Cycle::Run);
        assert_eq!(play_rate(1.0, run, 1.0), 0.7);
        assert_eq!(play_rate(20.0, run, 1.0), 1.5);
        assert!((play_rate(RUN_SPEED, run, 1.15) - 1.15).abs() < 1e-6);
    }

    #[test]
    fn lock_move_steps_at_the_recorded_rhythm() {
        // Cycles a second at full lock speed: a shuffle each, or two steps each going ahead or back.
        let per_second = |c: Cycle, speed: f32| {
            let clip = CLIPS.get(c);
            play_rate(speed, clip, 1.0) / clip.duration
        };
        assert!((per_second(Cycle::LockRight, LOCK_SPEED) - LOCK_SIDE_SHUFFLES).abs() < 0.01);
        assert!((per_second(Cycle::LockLeft, LOCK_SPEED) - LOCK_SIDE_SHUFFLES).abs() < 0.01);
        assert!((2.0 * per_second(Cycle::LockForward, LOCK_SPEED) - LOCK_FORWARD_STEPS).abs() < 0.01);
        assert!((2.0 * per_second(Cycle::LockBack, LOCK_SPEED * LOCK_SPEED_BY_ANGLE[2].1) - LOCK_BACK_STEPS).abs() < 0.01);
    }

    #[test]
    fn lock_move_picks_cardinals_with_some_give() {
        let p = Player { locked: true, ..default() };
        assert_eq!(pick_cycle(&p, 3.0, 50.0, Cycle::LockForward), Cycle::LockForward);
        assert_eq!(pick_cycle(&p, 3.0, 70.0, Cycle::LockForward), Cycle::LockRight);
        assert_eq!(pick_cycle(&p, 3.0, -100.0, Cycle::LockIdle), Cycle::LockLeft);
        assert_eq!(pick_cycle(&p, 3.0, 170.0, Cycle::LockIdle), Cycle::LockBack);
    }
}
