//! The local player: movement, stamina, extra effort, ball control, kicks and the camera.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::ball::Ball;
use crate::input::{Action, Actions, Axes, InputBlocked};
use crate::tuning::*;

#[derive(Component)]
pub struct Player {
    pub vel: Vec3,
    pub facing: f32, // yaw, radians
    pub stamina: f32,
    pub exhausted: bool,
    pub regen_wait: f32,
    pub sprinting: bool,
    pub extra_effort: f32, // gauge 0..1
    pub extra_active: bool,
    pub has_ball: bool,
    pub kick: Option<KickState>,
    pub locked: bool,
    pub dash: Option<DashState>,
    pub dash_cooldown: f32,
    pub second_dash_window: f32,
}

#[derive(Clone, Copy)]
pub struct DashState {
    pub t: f32,
    pub second: bool,
    pub dir: Vec3,
}

/// A kick from button press to ball contact. The build-up runs while the button is held.
#[derive(Clone, Copy)]
pub struct KickState {
    pub kind: Kick,
    pub elapsed: f32, // since the press
    pub held: f32,    // how long the button stayed down
    pub released: bool,
    /// Shot aim (yaw, pitch in degrees), locked at the press so the mouse can curve the ball.
    pub aim: Vec2,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            vel: Vec3::ZERO,
            facing: 0.0,
            stamina: MAX_STAMINA,
            exhausted: false,
            regen_wait: 0.0,
            sprinting: false,
            extra_effort: MAX_EXTRA_EFFORT,
            extra_active: false,
            has_ball: false,
            kick: None,
            locked: false,
            dash: None,
            dash_cooldown: 0.0,
            second_dash_window: 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kick {
    Shot,
    Pass,
    ShortPass,
    Lob,
    ShortLob,
    PushBall,
}

impl Kick {
    /// The held action for kicks you charge; instant kicks have none.
    fn charge_action(self) -> Option<Action> {
        match self {
            Kick::Shot => Some(Action::Shoot),
            Kick::Pass => Some(Action::Pass),
            Kick::Lob => Some(Action::Lob),
            _ => None,
        }
    }

    fn buildup(self) -> f32 {
        match self {
            Kick::Shot => SHOT_BUILDUP,
            Kick::Pass => PASS_BUILDUP,
            Kick::Lob => LOB_BUILDUP,
            Kick::ShortPass | Kick::ShortLob => SHORT_WINDUP,
            Kick::PushBall => PUSH_BALL_WINDUP,
        }
    }
}

impl Player {
    /// Charging a shot: the mouse curves the ball instead of turning the camera.
    pub fn charging_shot(&self) -> bool {
        self.kick.is_some_and(|k| k.kind == Kick::Shot && !k.released)
    }
}

/// Camera orbit angles, in degrees. Pitch + is up.
#[derive(Component, Default)]
pub struct CameraRig {
    pub yaw: f32,
    pub pitch: f32,
}

fn forward(yaw_rad: f32) -> Vec3 {
    Vec3::new(-yaw_rad.sin(), 0.0, -yaw_rad.cos())
}

fn camera_relative(movement: Vec2, cam_yaw_deg: f32) -> Vec3 {
    let yaw = cam_yaw_deg.to_radians();
    forward(yaw) * movement.y + forward(yaw - std::f32::consts::FRAC_PI_2) * movement.x
}

pub fn grab_cursor(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    blocked: Res<InputBlocked>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    } else if mouse.just_pressed(MouseButton::Left) && !blocked.0 {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

pub fn camera_look(
    mut rig: Single<&mut CameraRig>,
    axes: Res<Axes>,
    actions: Res<Actions>,
    player: Single<(&Transform, &Player)>,
    ball: Single<&Transform, With<Ball>>,
) {
    let (ptf, p) = *player;
    if actions.just_pressed(Action::LookAtBall) {
        let to_ball = ball.translation - ptf.translation;
        rig.yaw = (-to_ball.x).atan2(-to_ball.z).to_degrees();
    }
    let look = if p.charging_shot() { axes.look_pad } else { axes.look_pad + axes.look_mouse };
    rig.yaw -= look.x;
    rig.pitch = (rig.pitch + look.y).clamp(-PITCH_DOWN_LIMIT, PITCH_UP_LIMIT);
}

pub fn player_move(
    mut player: Single<(&mut Transform, &mut Player)>,
    rig: Single<&CameraRig>,
    actions: Res<Actions>,
    axes: Res<Axes>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let (tf, p) = &mut *player;
    let input = axes.movement;
    let wish = camera_relative(input, rig.yaw);

    p.locked = actions.pressed(Action::Lock);

    // Stamina (mechanics.md §1). Lock move and dashes can't sprint.
    let want_sprint = actions.pressed(Action::Sprint) && input != Vec2::ZERO && !p.locked && p.dash.is_none();
    if want_sprint && !p.sprinting && !p.exhausted {
        p.stamina -= SPRINT_START_COST;
    }
    p.sprinting = want_sprint;
    if p.sprinting {
        p.stamina -= if p.has_ball { BALL_SPRINT_DRAIN } else { SPRINT_DRAIN } * dt;
        p.regen_wait = REGEN_DELAY;
    }
    if p.stamina <= 0.0 && !p.exhausted {
        p.exhausted = true;
        p.regen_wait = EXHAUSTED_REGEN_DELAY;
    }
    p.stamina = p.stamina.max(0.0);
    p.regen_wait -= dt;
    if !p.sprinting && p.regen_wait <= 0.0 {
        let rate = if p.exhausted { EXHAUSTED_REGEN } else { STAMINA_REGEN };
        p.stamina = (p.stamina + rate * dt).min(MAX_STAMINA);
    }
    if p.exhausted && p.stamina >= EXHAUSTED_EXIT {
        p.exhausted = false;
    }

    // Extra effort: its own gauge, instant acceleration (mechanics.md §10)
    if actions.just_pressed(Action::ExtraEffort) && p.extra_effort > 0.05 {
        p.extra_active = true;
    }
    if p.extra_active {
        p.extra_effort -= EXTRA_EFFORT_DRAIN * dt;
        if !actions.pressed(Action::ExtraEffort) || p.extra_effort <= 0.0 {
            p.extra_active = false;
        }
    } else {
        p.extra_effort = (p.extra_effort + EXTRA_EFFORT_REGEN * dt).min(MAX_EXTRA_EFFORT);
    }
    p.extra_effort = p.extra_effort.max(0.0);

    // Speed target and turning (mechanics.md §2)
    let (mut top, mut accel, turn) = match (p.sprinting, p.has_ball) {
        (true, false) => (if p.exhausted { EXHAUSTED_SPRINT } else { SPRINT_SPEED }, SPRINT_ACCEL, TURN_RATE_SPRINT),
        (true, true) => (if p.exhausted { EXHAUSTED_BALL_SPRINT } else { BALL_SPRINT_SPEED }, BALL_SPRINT_ACCEL, TURN_RATE_BALL_SPRINT),
        (false, false) => (RUN_SPEED, RUN_ACCEL, TURN_RATE_RUN),
        (false, true) => (BALL_RUN_SPEED, BALL_RUN_ACCEL, TURN_RATE_BALL_RUN),
    };
    let stamina_ratio = curve(&SPEED_BY_STAMINA, p.stamina);
    top *= if p.has_ball { stamina_ratio.min(1.0) } else { stamina_ratio };
    let turn = if p.extra_active {
        top = EXTRA_EFFORT_SPEED;
        accel = EXTRA_EFFORT_ACCEL;
        TURN_RATE_EXTRA_EFFORT
    } else {
        turn
    };
    if p.kick.is_some() {
        top *= WINDUP_SPEED_SCALE;
    }

    // Dash (mechanics.md §8): from lock move; a second dash right after is weaker, then a cooldown.
    p.dash_cooldown -= dt;
    p.second_dash_window -= dt;
    if actions.just_pressed(Action::Dash) && p.dash.is_none() {
        let second = p.second_dash_window > 0.0;
        if second || p.dash_cooldown <= 0.0 {
            let dir = if wish != Vec3::ZERO { wish.normalize() } else { -forward(p.facing) };
            p.dash = Some(DashState { t: 0.0, second, dir });
            p.second_dash_window = 0.0;
            p.stamina = (p.stamina - DASH_COST).max(0.0);
            p.regen_wait = REGEN_DELAY;
        }
    }

    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    if let Some(mut d) = p.dash {
        d.t += dt;
        let speeds: &[(f32, f32)] = if d.second { &SECOND_DASH_SPEED } else { &DASH_SPEED };
        p.vel = d.dir * curve(speeds, d.t / DASH_DURATION);
        p.dash = Some(d);
        if d.t >= DASH_DURATION {
            if !d.second {
                p.second_dash_window = SECOND_DASH_WINDOW;
            }
            p.dash_cooldown = DASH_COOLDOWN;
            p.dash = None;
        }
    } else if p.locked {
        // Lock move (mechanics.md §2): face the camera, strafe, slower going backwards.
        let diff = (rig.yaw.to_radians() - p.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let max_turn = LOCK_TURN_RATE.to_radians() * dt;
        p.facing += diff.clamp(-max_turn, max_turn);
        let (top, accel, by_angle): (f32, f32, &[(f32, f32)]) =
            if p.has_ball { (LOCK_BALL_SPEED, LOCK_BALL_ACCEL, &LOCK_BALL_SPEED_BY_ANGLE) } else { (LOCK_SPEED, LOCK_ACCEL, &LOCK_SPEED_BY_ANGLE) };
        let target = if wish == Vec3::ZERO {
            Vec3::ZERO
        } else {
            let angle = forward(p.facing).angle_between(wish).to_degrees();
            wish.normalize() * top * input.length() * curve(by_angle, angle)
        };
        p.vel = p.vel.move_towards(target, if target == Vec3::ZERO { STOP_DECEL } else { accel } * dt);
    } else if wish != Vec3::ZERO {
        let target = (-wish.x).atan2(-wish.z);
        let diff = (target - p.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        // Standing still you turn freely; moving, the turn rate caps how fast you change direction.
        let max_turn = if speed < 0.5 { f32::INFINITY } else { turn.to_radians() * dt };
        p.facing += diff.clamp(-max_turn, max_turn);
        let new_speed = (speed + accel * input.length() * dt).min(top * input.length());
        p.vel = forward(p.facing) * new_speed.max(speed.min(top) - STOP_DECEL * dt);
    } else {
        p.vel = p.vel.normalize_or_zero() * (speed - STOP_DECEL * dt).max(0.0);
    }

    tf.translation += p.vel * dt;
    tf.translation.x = tf.translation.x.clamp(-HALF_WIDTH + 0.4, HALF_WIDTH - 0.4);
    tf.translation.z = tf.translation.z.clamp(-HALF_LENGTH + 0.4, HALF_LENGTH - 0.4);
    tf.rotation = Quat::from_rotation_y(p.facing);
}

pub fn ball_control(mut player: Single<(&Transform, &mut Player), Without<Ball>>, mut ball: Single<(&mut Transform, &mut Ball)>) {
    let (ptf, p) = &mut *player;
    let (btf, b) = &mut *ball;
    if p.has_ball {
        // ponytail: ball glued in front of the feet; real dribble touches come later
        let target = ptf.translation + forward(p.facing) * DRIBBLE_OFFSET;
        btf.translation = Vec3::new(target.x, BALL_RADIUS, target.z);
        b.vel = p.vel;
        return;
    }
    let flat = Vec2::new(btf.translation.x - ptf.translation.x, btf.translation.z - ptf.translation.z);
    let moving = p.vel.length() > MOVING_THRESHOLD;
    let radius = if moving { CONTROL_RADIUS_MOVING } else { CONTROL_RADIUS_STANDING } + BALL_RADIUS;
    if b.no_control <= 0.0 && flat.length() < radius && btf.translation.y < CONTROL_MAX_HEIGHT {
        p.has_ball = true;
        b.held = true;
        b.spin = Vec3::ZERO;
    }
}

pub fn kicking(
    mut player: Single<&mut Player>,
    mut ball: Single<&mut Ball>,
    rig: Single<&CameraRig>,
    actions: Res<Actions>,
    mut axes: ResMut<Axes>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let p = &mut **player;

    // A pass press while charging a shot is a fake shot (Rematch binds both to the same button).
    if p.charging_shot() && actions.just_pressed(Action::FakeShot) {
        p.kick = None;
        p.stamina = (p.stamina - FAKE_SHOT_COST).max(0.0);
        p.regen_wait = REGEN_DELAY;
        return;
    }

    // Lock move can't shoot, pass or push the ball, and nor can a dash (frame-data.md).
    if p.has_ball && p.kick.is_none() && !p.locked && p.dash.is_none() {
        let charged = [(Action::Shoot, Kick::Shot), (Action::Pass, Kick::Pass), (Action::Lob, Kick::Lob)];
        let instant = [(Action::LowPowerPass, Kick::ShortPass), (Action::LowPowerLob, Kick::ShortLob), (Action::PushBall, Kick::PushBall)];
        let pressed = instant.into_iter().chain(charged).find(|(a, _)| actions.just_pressed(*a));
        if let Some((_, kind)) = pressed {
            axes.reset_curve();
            let released = kind.charge_action().is_none();
            p.kick = Some(KickState { kind, elapsed: 0.0, held: 0.0, released, aim: Vec2::new(rig.yaw, rig.pitch) });
        }
    }

    let Some(k) = &mut p.kick else { return };
    k.elapsed += dt;
    if !k.released {
        if k.kind.charge_action().is_some_and(|a| actions.pressed(a)) {
            k.held = k.elapsed.min(MAX_HOLD);
        } else {
            k.released = true;
        }
    }
    if k.kind == Kick::Shot {
        // The gamepad's right stick keeps aiming a charging shot; the mouse curves it instead.
        k.aim.x -= axes.look_pad.x;
        k.aim.y = (k.aim.y + axes.look_pad.y).clamp(-PITCH_DOWN_LIMIT, PITCH_UP_LIMIT);
    }
    // The build-up plays while you hold: the ball leaves on release, never before the build-up ends.
    if !k.released || k.elapsed < k.kind.buildup() {
        return;
    }
    let k = p.kick.take().expect("checked above");
    if !p.has_ball {
        return;
    }

    let t = k.held.max(k.kind.buildup()).min(MAX_HOLD); // force uses at least the build-up time
    let ratio = t / MAX_HOLD;
    let side = axes.curve.x;
    let (speed, elevation, friction, no_control) = match k.kind {
        Kick::Shot => (curve(&SHOT_FORCE, t), curve(&ELEVATION_BY_PITCH, k.aim.y), SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::Pass => (curve(&PASS_FORCE, t), PASS_ELEVATION, PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortPass => (SHORT_PASS_SPEED, PASS_ELEVATION, SHORT_PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::Lob => (curve(&LOB_FORCE, t), LOB_ELEVATION, SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortLob => (SHORT_LOB_SPEED, LOB_ELEVATION, SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::PushBall if p.extra_active => (PUSH_BALL_SPEED, 0.0, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL_EXTRA_EFFORT),
        Kick::PushBall => (PUSH_BALL_SPEED, 0.0, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
    };
    let yaw = match k.kind {
        Kick::PushBall => p.facing, // goes where you're running
        Kick::Shot => (k.aim.x - side * AIM_NUDGE).to_radians(), // locked at the press
        _ => rig.yaw.to_radians(),
    };
    let flat = forward(yaw);
    let dir = (flat * elevation.to_radians().cos() + Vec3::Y * elevation.to_radians().sin()).normalize();
    let right = flat.cross(Vec3::Y);

    p.has_ball = false;
    ball.held = false;
    ball.vel = dir * speed;
    ball.spin = if k.kind == Kick::Shot {
        // Spin around -Y curves the ball to the right of travel; `right` spin lifts it.
        let top = curve(&TOP_SPIN, axes.curve.y) * curve(&TOP_SPIN_BY_CHARGE, ratio) + SHOT_BASE_SPIN;
        Vec3::NEG_Y * curve(&SIDE_SPIN, side) * curve(&SIDE_SPIN_BY_CHARGE, ratio) + right * top
    } else {
        Vec3::ZERO // ponytail: curve only on shots for now
    };
    ball.friction = friction;
    ball.no_control = no_control;
    if k.kind == Kick::PushBall {
        p.stamina = (p.stamina - PUSH_BALL_COST).max(0.0);
        p.regen_wait = PUSH_BALL_REGEN_DELAY;
    }
}

pub fn camera_follow(
    player: Single<(&Transform, &Player), Without<Camera3d>>,
    rig: Single<&CameraRig>,
    mut camera: Single<(&mut Transform, &mut Projection), With<Camera3d>>,
    time: Res<Time>,
) {
    let (ptf, p) = *player;
    let rot = Quat::from_euler(EulerRot::YXZ, rig.yaw.to_radians(), rig.pitch.to_radians(), 0.0);
    let pivot = ptf.translation + Vec3::Y * CAMERA_HEIGHT;
    let (ctf, proj) = &mut *camera;
    ctf.translation = pivot + rot * Vec3::new(CAMERA_RIGHT, 0.0, CAMERA_DISTANCE);
    ctf.rotation = rot;
    if let Projection::Perspective(persp) = &mut **proj {
        let target = if p.sprinting || p.extra_active { SPRINT_FOV } else { FOV }.to_radians();
        persp.fov += (target - persp.fov) * (time.delta_secs() * 5.0).min(1.0);
    }
}
