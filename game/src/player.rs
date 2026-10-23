//! The local player: movement, stamina, ball control, kicking and the third-person camera.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::ball::Ball;
use crate::tuning::*;

#[derive(Component)]
pub struct Player {
    pub vel: Vec3,
    pub facing: f32, // yaw, radians
    pub stamina: f32,
    pub exhausted: bool,
    pub regen_wait: f32,
    pub sprinting: bool,
    pub has_ball: bool,
    pub charge: Option<(Kick, f32)>, // held time
    pub windup: Option<(Kick, f32, f32)>, // charge, seconds left
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
            has_ball: false,
            charge: None,
            windup: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kick {
    Shot,
    Pass,
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

pub fn grab_cursor(mut window: Single<&mut Window, With<PrimaryWindow>>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>) {
    if keys.just_pressed(KeyCode::Escape) {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    } else if mouse.just_pressed(MouseButton::Left) {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

pub fn camera_look(mut rig: Single<&mut CameraRig>, motion: Res<AccumulatedMouseMotion>, window: Single<&Window, With<PrimaryWindow>>) {
    if window.cursor_options.grab_mode == CursorGrabMode::None {
        return;
    }
    rig.yaw -= motion.delta.x * MOUSE_SENSITIVITY;
    rig.pitch = (rig.pitch - motion.delta.y * MOUSE_SENSITIVITY).clamp(-PITCH_DOWN_LIMIT, PITCH_UP_LIMIT);
}

pub fn player_move(mut player: Single<(&mut Transform, &mut Player)>, rig: Single<&CameraRig>, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>) {
    let dt = time.delta_secs().min(0.05);
    let (tf, p) = &mut *player;

    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let input = Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyW, KeyCode::KeyS)).clamp_length_max(1.0);
    let cam_yaw = rig.yaw.to_radians();
    let wish = forward(cam_yaw) * input.y + forward(cam_yaw - std::f32::consts::FRAC_PI_2) * input.x;

    // Stamina (mechanics.md §1)
    let want_sprint = keys.pressed(KeyCode::ShiftLeft) && input != Vec2::ZERO;
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

    // Speed target and turning (mechanics.md §2)
    let (mut top, accel, turn) = match (p.sprinting, p.has_ball) {
        (true, false) => (if p.exhausted { EXHAUSTED_SPRINT } else { SPRINT_SPEED }, SPRINT_ACCEL, TURN_RATE_SPRINT),
        (true, true) => (if p.exhausted { EXHAUSTED_BALL_SPRINT } else { BALL_SPRINT_SPEED }, BALL_SPRINT_ACCEL, TURN_RATE_BALL_SPRINT),
        (false, false) => (RUN_SPEED, RUN_ACCEL, TURN_RATE_RUN),
        (false, true) => (BALL_RUN_SPEED, BALL_RUN_ACCEL, TURN_RATE_BALL_RUN),
    };
    let stamina_ratio = curve(&SPEED_BY_STAMINA, p.stamina);
    top *= if p.has_ball { stamina_ratio.min(1.0) } else { stamina_ratio };
    if p.windup.is_some() {
        top *= WINDUP_SPEED_SCALE;
    }

    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    if wish != Vec3::ZERO {
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

pub fn ball_control(
    mut player: Single<(&Transform, &mut Player), Without<Ball>>,
    mut ball: Single<(&mut Transform, &mut Ball)>,
) {
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
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let p = &mut **player;

    if p.has_ball && p.windup.is_none() {
        for (button, kick) in [(MouseButton::Left, Kick::Shot), (MouseButton::Right, Kick::Pass)] {
            if mouse.just_pressed(button) && p.charge.is_none() {
                p.charge = Some((kick, 0.0));
            }
            if let Some((k, held)) = p.charge
                && k == kick
            {
                if mouse.pressed(button) {
                    p.charge = Some((k, held + dt));
                } else {
                    let charge = (held / FULL_CHARGE_TIME).min(1.0);
                    let windup = match kick {
                        Kick::Pass => PASS_WINDUP,
                        Kick::Shot if curve(&SHOT_FORCE, charge) > POWER_SHOT_SPEED => POWER_SHOT_WINDUP,
                        Kick::Shot => LOW_SHOT_WINDUP,
                    };
                    p.windup = Some((kick, charge, windup));
                    p.charge = None;
                }
            }
        }
    }

    let Some((kick, charge, left)) = p.windup else { return };
    if left > dt {
        p.windup = Some((kick, charge, left - dt));
        return;
    }
    p.windup = None;
    if !p.has_ball {
        return;
    }

    let stick_x = keys.pressed(KeyCode::KeyD) as i32 as f32 - keys.pressed(KeyCode::KeyA) as i32 as f32;
    let yaw = (rig.yaw - stick_x * AIM_NUDGE).to_radians();
    let (speed, elevation, friction) = match kick {
        Kick::Shot => (curve(&SHOT_FORCE, charge), curve(&ELEVATION_BY_PITCH, rig.pitch), SHOT_FRICTION),
        Kick::Pass => (curve(&PASS_FORCE, charge), PASS_ELEVATION, PASS_FRICTION),
    };
    let dir = (forward(yaw) * elevation.to_radians().cos() + Vec3::Y * elevation.to_radians().sin()).normalize();
    let side_spin = curve(&SIDE_SPIN, stick_x) * curve(&SIDE_SPIN_BY_CHARGE, charge);

    p.has_ball = false;
    ball.held = false;
    ball.vel = dir * speed;
    ball.spin = Vec3::Y * side_spin;
    ball.friction = friction;
    ball.no_control = NO_RECONTROL_TIME;
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
        let target = if p.sprinting { SPRINT_FOV } else { FOV }.to_radians();
        persp.fov += (target - persp.fov) * (time.delta_secs() * 5.0).min(1.0);
    }
}
