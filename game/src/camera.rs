//! The match camera (camera.md): a pivot that trails the player, an arm that turns with the pitch
//! and changes length with it, per-state camera data blended by transitions, a sideways offset
//! from movement, and Rematch's horizontal FOV. Only the player turns it.

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::ball::Ball;
use crate::input::{Action, Actions, Axes, Bindings};
use crate::player::{CameraRig, Kick, Player, crosshair_point, yaw_of};
use crate::tuning::*;

/// Which camera data is active (the camera behaviour tree's choice).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum CamState {
    #[default]
    Default,
    WithBall,
    SprintNoBall,
    SprintWithBall,
    ExtraEffort,
    LockMove,
    /// Lock move with the ball.
    Aim,
    /// Charging a shot.
    Shoot,
    /// From a shot's release to the strike: the camera is locked.
    LockShot,
}

/// The blended part of a camera data. Offsets are in the camera's frame as the game writes them:
/// x forward (negative = behind), y right, z up; metres.
#[derive(Clone, Copy)]
struct CamData {
    fov: f32,
    ideal: Vec3,
    /// At the down limit (looking down) and at the up limit (looking up).
    above: Vec3,
    below: Vec3,
    fulcrum: f32,
    down: f32,
    up: f32,
    /// Pivot smoothing, per second.
    k: f32,
}

impl CamData {
    fn lerp(self, o: CamData, r: f32) -> CamData {
        let f = |a: f32, b: f32| a + (b - a) * r;
        CamData {
            fov: f(self.fov, o.fov),
            ideal: self.ideal.lerp(o.ideal, r),
            above: self.above.lerp(o.above, r),
            below: self.below.lerp(o.below, r),
            fulcrum: f(self.fulcrum, o.fulcrum),
            down: f(self.down, o.down),
            up: f(self.up, o.up),
            k: f(self.k, o.k),
        }
    }

    /// The arm offset at this pitch: a cubic Hermite spline through below -> ideal -> above, each
    /// side on its own 0..1 range (camera.md §3).
    fn arm(&self, pitch: f32) -> Vec3 {
        let h = |s: f32, p0: Vec3, p1: Vec3, m0: Vec3, m1: Vec3| {
            let (s2, s3) = (s * s, s * s * s);
            p0 * (2.0 * s3 - 3.0 * s2 + 1.0) + m0 * (s3 - 2.0 * s2 + s) + p1 * (-2.0 * s3 + 3.0 * s2) + m1 * (s3 - s2)
        };
        let (i, a, b) = (self.ideal, self.above, self.below);
        let t = (a - b) / 2.0;
        if pitch < 0.0 {
            h((-pitch / self.down).clamp(0.0, 1.0), i, a, t, a - i)
        } else {
            h((1.0 - pitch / self.up).clamp(0.0, 1.0), b, i, i - b, t)
        }
    }
}

const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn data(s: CamState) -> CamData {
    use CamState::*;
    let d = |fov, ideal: Vec3, above: Option<Vec3>, below: Vec3, fulcrum, k| CamData {
        fov,
        ideal,
        above: above.unwrap_or(ideal),
        below,
        fulcrum,
        down: CAMERA_PITCH_DOWN,
        up: CAMERA_PITCH_UP,
        k,
    };
    match s {
        Default => d(80.0, v(-5.0, 0.6, 0.1), Some(v(-5.5, 0.6, 0.15)), v(-4.0, 0.6, 0.0), 0.45, 15.0),
        WithBall => d(80.0, v(-4.75, 0.6, 0.1), Some(v(-5.25, 0.6, 0.15)), v(-3.75, 0.6, 0.0), 0.45, 15.0),
        SprintNoBall => d(85.0, v(-4.5, 0.6, 0.1), None, v(-3.5, 0.6, 0.0), 0.3, 100.0),
        SprintWithBall => d(80.0, v(-4.5, 0.6, 0.1), None, v(-3.5, 0.6, 0.0), 0.3, 100.0),
        ExtraEffort => d(90.0, v(-3.5, 0.6, 0.1), None, v(-3.25, 0.6, 0.0), 0.35, 100.0),
        LockMove => d(78.0, v(-5.0, 0.3, 0.1), Some(v(-6.0, 0.3, 0.1)), v(-4.5, 0.3, 0.1), 0.55, 50.0),
        Aim => d(80.0, v(-4.25, 0.6, 0.0), None, v(-3.75, 0.6, 0.05), 0.58, 20.0),
        // ponytail: the shot cameras' slower vertical pivot (k 3) isn't separate here.
        Shoot | LockShot => d(80.0, v(-3.6, 0.6, 0.1), None, v(-3.5, 0.6, 0.0), 0.58, 10.0),
    }
}

/// Turn speeds, switched at once on a state change: (yaw, pitch, yaw boost, pitch boost) in deg/s.
fn turn(s: CamState) -> (f32, f32, f32, f32) {
    use CamState::*;
    match s {
        LockMove | Aim => (160.0, 80.0, 40.0, 20.0),
        Shoot => (120.0, 60.0, 0.0, 0.0),
        LockShot => (0.0, 0.0, 0.0, 0.0),
        _ => (240.0, 120.0, 240.0, 120.0),
    }
}

/// The sideways offset from movement (camera.md §6): how far right (max) or left (min) moving
/// across the camera pushes it, its smoothing, and whether it follows your facing instead of your
/// velocity, or keeps the side you had when the state started.
struct SideOffset {
    max: f32,
    min: f32,
    rate: f32,
    facing: bool,
    keep_entry: bool,
}

fn side_offset(s: CamState) -> Option<SideOffset> {
    use CamState::*;
    let o = |max, min, rate, facing, keep_entry| Some(SideOffset { max, min, rate, facing, keep_entry });
    match s {
        Default | WithBall => o(0.45, -0.2, 3.0, false, false),
        SprintWithBall => o(0.2, -0.2, 0.25, true, false),
        // The game swaps to the left shoulder here (min -1.6); Kev doesn't want shoulder swaps.
        ExtraEffort => o(0.4, -0.2, 2.0, true, false),
        Shoot => o(1.0, 0.0, 2.0, false, true),
        LockShot => o(1.0, 0.0, 0.1, false, true),
        SprintNoBall | LockMove | Aim => None,
    }
}

/// The transition into `to` from `from` (camera.md §8): the old state's "out" first, then the new
/// state's "in", then the default 0.65 s.
fn transition(from: CamState, to: CamState) -> f32 {
    use CamState::*;
    match (from, to) {
        (SprintNoBall, SprintWithBall) => 2.5,
        (SprintNoBall, Default | WithBall) => 3.0,
        // Every transition eases in and out, at least 0.4 s, with no overshoot: the game's
        // steep-start curves and its in-out-in keys into sprint and extra effort made the camera
        // jolt and wobble (Kev).
        (SprintWithBall, Default | WithBall | SprintNoBall | ExtraEffort) => 2.0,
        (ExtraEffort, _) => 2.0,
        (LockShot, _) | (Shoot, Default | WithBall) => 0.9,
        (LockMove, Aim) => 0.5,
        (LockMove, _) => 0.8,
        (_, SprintNoBall) => 0.8,
        (_, ExtraEffort) => 0.8,
        (_, SprintWithBall) => 0.5,
        (_, LockMove) => 0.6,
        (Aim, Shoot) => 0.4,
        (_, Shoot) => 0.5,
        (_, LockShot) => 0.4,
        _ => 0.65,
    }
}

fn ratio(t: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return 1.0;
    }
    let u = (t / duration).min(1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The camera's running state.
#[derive(Component, Default)]
pub struct CameraState {
    state: CamState,
    prev: CamState,
    /// Blending from a snapshot of the previous camera to the current state, at ratio `r`.
    from: Option<CamData>,
    t: f32,
    r: f32,
    pivot: Option<Vec3>,
    /// The movement's sideways offset (added to the arm's y), the side ratio kept at a state's
    /// start, how long you've stood still, and whether you've moved since.
    side: f32,
    entry_side: f32,
    /// How much of the sideways offset the current state takes, eased on its own.
    side_w: f32,
    still: f32,
    moved: bool,
    /// Gamepad turn boost per axis, -1..1.
    boost: Vec2,
}

impl CameraState {
    fn blended(&self) -> CamData {
        let to = data(self.state);
        self.from.map_or(to, |from| from.lerp(to, self.r))
    }
}

/// The state the camera's behaviour tree would pick.
fn pick_state(p: &Player) -> CamState {
    use CamState::*;
    match p.kick {
        Some(k) if k.kind == Kick::Shot && !k.released => return Shoot,
        Some(k) if k.kind == Kick::Shot => return LockShot,
        _ => {}
    }
    match (p.locked, p.has_ball) {
        (true, false) => return LockMove,
        (true, true) => return Aim,
        _ => {}
    }
    match (p.extra_active, p.sprint_running, p.has_ball) {
        (true, ..) => ExtraEffort,
        (_, true, true) => SprintWithBall,
        (_, true, false) => SprintNoBall,
        (.., true) => WithBall,
        _ => Default,
    }
}

/// Look input to yaw and pitch: mouse per frame, gamepad per second with a boost when the stick
/// is held near full (camera.md §2). The state's turn speeds act on both.
pub fn camera_look(
    mut rig: Single<&mut CameraRig>,
    mut cam: Single<&mut CameraState>,
    camera: Single<&Transform, With<Camera3d>>,
    axes: Res<Axes>,
    actions: Res<Actions>,
    player: Single<(&Transform, &Player)>,
    ball: Single<&Transform, With<Ball>>,
    bindings: Res<Bindings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ptf, p) = *player;
    if actions.just_pressed(Action::LookAtBall) {
        rig.yaw = yaw_of(ball.translation - ptf.translation).to_degrees();
    }
    let (h, vert, hb, vb) = turn(cam.state);
    let stick = axes.look_pad.clamp(Vec2::NEG_ONE, Vec2::ONE);
    let boost = |b: f32, s: f32| {
        if s.abs() >= CAMERA_BOOST_THRESHOLD {
            (b + s.signum() * dt / CAMERA_BOOST_UP).clamp(-1.0, 1.0)
        } else {
            b - b.signum() * (dt / CAMERA_BOOST_DOWN).min(b.abs())
        }
    };
    cam.boost = Vec2::new(boost(cam.boost.x, stick.x), boost(cam.boost.y, stick.y));
    let mut d = Vec2::new(stick.x * h + cam.boost.x * hb, stick.y * vert + cam.boost.y * vb) * dt;
    if !p.mouse_steers_kick(bindings.pass_aim) {
        let m = axes.look_mouse * MOUSE_AXIS_SCALE * PI / 180.0;
        d += Vec2::new(m.x * CAMERA_MOUSE_YAW * h, m.y * CAMERA_MOUSE_PITCH * vert);
    }
    let lim = cam.blended();
    rig.yaw -= d.x;
    rig.pitch = (rig.pitch + d.y).clamp(-lim.down, lim.up);
    // A shot locks its aim at the press. The camera keeps following you as you slide on and turns
    // to keep the crosshair on that point (the lock shot; players confirm the aim locks as a shot
    // starts, and passes and lobs don't lock).
    if let Some(k) = p.kick.filter(|k| k.kind == Kick::Shot) {
        rig.yaw = yaw_of(crosshair_point(k.eye, k.aim.x.to_radians()) - camera.translation).to_degrees();
        rig.pitch = k.aim.y;
    }
}

pub fn camera_follow(
    player: Single<(&Transform, &Player), Without<Camera3d>>,
    rig: Single<&CameraRig>,
    bindings: Res<Bindings>,
    mut camera: Single<(&mut Transform, &mut Projection, &mut CameraState), With<Camera3d>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ptf, p) = *player;
    let (ctf, proj, cam) = &mut *camera;

    // State changes start a transition from wherever the camera is now.
    let want = pick_state(p);
    if want != cam.state {
        let now = cam.blended();
        cam.from = Some(now);
        cam.t = 0.0;
        cam.r = 0.0;
        cam.prev = cam.state;
        cam.state = want;
        cam.entry_side = side_ratio(p, rig.yaw, side_offset(want).is_some_and(|o| o.facing));
    }
    cam.t += dt;
    if cam.from.is_some() {
        cam.r = ratio(cam.t, transition(cam.prev, cam.state));
        if cam.r >= 0.9999 {
            cam.from = None;
        }
    }
    let d = cam.blended();

    // Pivot: the capsule centre, trailing by the state's smoothing (frame-rate dependent, as in
    // the game).
    let target = ptf.translation;
    let pivot = cam.pivot.unwrap_or(target);
    let pivot = pivot + (target - pivot) * (d.k * dt).min(1.0);
    cam.pivot = Some(pivot);

    // Sideways offset from movement.
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    if let Some(o) = side_offset(cam.state) {
        if speed > 1.0 {
            cam.moved = true;
            cam.still = 0.0;
            let side = if o.keep_entry { cam.entry_side } else { side_ratio(p, rig.yaw, o.facing) };
            let target = if side > 0.0 { side * o.max } else { side.abs() * o.min };
            cam.side += (target - cam.side) * (o.rate * dt).min(1.0);
        } else if cam.moved {
            // Freezes when you stop, then eases back after 0.2 s.
            cam.still += dt;
            if cam.still > CAMERA_SIDE_IDLE {
                cam.side += (0.0 - cam.side) * (CAMERA_SIDE_RETURN * dt).min(1.0);
            }
        }
    }
    // A state without a sideways offset (lock move, sprinting without the ball, aiming) takes it
    // away over the camera's transition, not at once.
    // Eased on its own over 0.4 s, so a transition started mid-transition doesn't jump it.
    let has_side = side_offset(cam.state).is_some() as u8 as f32;
    cam.side_w += (has_side - cam.side_w).clamp(-dt / 0.4, dt / 0.4);
    let side = cam.side * cam.side_w * cam.side_w * (3.0 - 2.0 * cam.side_w);
    // The offset can grow on its own shoulder, never shrink below the base or jump to the other
    // shoulder (Kev: the swaps were jarring).
    let shoulder = |base: f32| {
        let sum = base + side;
        if sum.abs() > base.abs() && sum.signum() == base.signum() { sum } else { base }
    };
    let mut d = d;
    for off in [&mut d.ideal, &mut d.above, &mut d.below] {
        off.y = shoulder(off.y);
    }

    // Position: fulcrum + the arm in the camera's yaw and pitch frame; the camera looks along its
    // own direction (no look-at).
    let rot = Quat::from_euler(EulerRot::YXZ, rig.yaw.to_radians(), rig.pitch.to_radians(), 0.0);
    let arm = d.arm(rig.pitch);
    ctf.translation = pivot + Vec3::Y * d.fulcrum + rot * Vec3::new(arm.y, arm.z, -arm.x);
    ctf.rotation = rot;
    if let Projection::Perspective(persp) = &mut **proj {
        // The game's FOV is horizontal at 16:9; Bevy's is vertical. The option shifts every state.
        let horizontal = (d.fov + bindings.fov - FOV).clamp(1.0, 179.0).to_radians();
        persp.fov = 2.0 * ((horizontal / 2.0).tan() * 9.0 / 16.0).atan();
    }
}

/// Sine of the angle between your movement (or facing) and the camera's yaw: + to its right.
fn side_ratio(p: &Player, yaw_deg: f32, facing: bool) -> f32 {
    let dir = if facing {
        crate::player::forward(p.facing)
    } else {
        Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero()
    };
    let right = Quat::from_rotation_y(yaw_deg.to_radians()) * Vec3::X;
    right.dot(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_follows_the_pitch() {
        let d = data(CamState::Default);
        // Level: 5 m behind; full down: 5.5 m; full up: 4 m.
        assert!((d.arm(0.0) - v(-5.0, 0.6, 0.1)).length() < 1e-4);
        assert!((d.arm(-22.0) - v(-5.5, 0.6, 0.15)).length() < 1e-4);
        assert!((d.arm(15.0) - v(-4.0, 0.6, 0.0)).length() < 1e-4);
        // Camera height above the feet at level pitch: capsule centre 0.9 + fulcrum 0.45 + 0.1.
        assert!((0.9 + d.fulcrum + d.arm(0.0).z - 1.45).abs() < 1e-4);
    }

    #[test]
    fn fov_80_horizontal_is_50_vertical() {
        let vertical = 2.0 * ((80f32.to_radians() / 2.0).tan() * 9.0 / 16.0).atan();
        assert!((vertical.to_degrees() - 50.5).abs() < 0.1);
    }
}
