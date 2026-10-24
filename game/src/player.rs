//! The local player: movement, stamina, extra effort, ball control, kicks and the camera.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::ball::Ball;
use crate::input::{Action, Actions, Axes, Bindings, InputBlocked, PassAim};
use crate::tuning::*;

#[derive(Component)]
pub struct Player {
    pub vel: Vec3,
    pub facing: f32, // yaw, radians
    pub stamina: f32,
    pub exhausted: bool,
    pub regen_wait: f32,
    /// Holding sprint (outside lock move and dashes), moving or not: push balls read this.
    pub sprinting: bool,
    /// Sprinting while moving: only this costs stamina.
    pub sprint_running: bool,
    pub extra_effort: f32, // gauge 0..1
    pub extra_active: bool,
    pub has_ball: bool,
    pub kick: Option<KickState>,
    pub locked: bool,
    /// How long lock move has been on, whether you had the ball last frame, and whether losing it
    /// ended lock move (which then needs a fresh press).
    pub lock_time: f32,
    pub had_ball: bool,
    pub relock_needed: bool,
    pub dash: Option<DashState>,
    pub dash_cooldown: f32,
    pub second_dash_cooldown: f32,
    pub dribble: Option<DribbleState>,
    pub dribble_lockout: f32,
    /// A push ball pressed before the dribble's window opened, waiting for it.
    pub push_buffered: bool,
    /// Scripted chase after a push ball.
    pub run: Option<Run>,
    /// Seconds into receiving the ball (the ball becomes yours at CONTROL_SNAP; you act from CONTROL_ACT).
    pub receiving: Option<f32>,
    /// A turn transition in progress.
    pub turn: Option<Turn>,
}

#[derive(Clone, Copy)]
pub struct DashState {
    pub t: f32,
    pub second: bool,
    pub dir: Vec3,
    /// The yaw you turn to, as in a dribble: the dash's own direction going forward, the camera's
    /// for side steps and dashes back.
    pub facing: f32,
}

/// Which way a dribble or push ball goes, against the camera (dribbles) or your facing (push balls).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Heading {
    Forward,
    Side,
    Back,
}

impl Heading {
    /// Unsigned angle in degrees; forward up to `forward`, back beyond `back`.
    fn of(angle: f32, forward: f32, back: f32) -> Self {
        match angle.abs() {
            a if a <= forward => Heading::Forward,
            a if a <= back => Heading::Side,
            _ => Heading::Back,
        }
    }
}

/// Dribble sectors from the input (x right, y forward): forward within 45 deg, back beyond 135.
/// Half a degree of slack keeps the exact keyboard diagonals on the forward and side sectors.
fn dribble_heading(input: Vec2) -> Heading {
    Heading::of(input.x.atan2(input.y).to_degrees(), 45.5, 135.5)
}

/// A dribble in progress. `feint` is the second one of a chain.
#[derive(Clone, Copy)]
pub struct DribbleState {
    pub t: f32,
    pub feint: bool,
    pub exhausted: bool,
    pub heading: Heading,
    pub dir: Vec3,
    /// The yaw you turn to: the cut's direction going forward, the camera's otherwise.
    pub facing: f32,
}

impl DribbleState {
    fn speeds(&self) -> &'static [(f32, f32)] {
        use Heading::*;
        match (self.exhausted, self.feint, self.heading) {
            (true, _, Forward) => &EXHAUSTED_DRIBBLE_FORWARD,
            (true, _, Side) => &EXHAUSTED_DRIBBLE_SIDE,
            (true, _, Back) => &EXHAUSTED_DRIBBLE_BACK,
            (_, true, Forward) => &FEINT_FORWARD,
            (_, true, Side) => &FEINT_SIDE,
            (_, true, Back) => &FEINT_BACK,
            (_, _, Forward) => &DRIBBLE_FORWARD,
            (_, _, Side) => &DRIBBLE_SIDE,
            (_, _, Back) => &DRIBBLE_BACK,
        }
    }

    fn length(&self) -> f32 {
        let lengths = if self.exhausted { EXHAUSTED_DRIBBLE_LENGTH } else if self.feint { FEINT_LENGTH } else { DRIBBLE_LENGTH };
        lengths[self.heading as usize]
    }
}

/// A big change of direction: brake along `from`, then re-accelerate along `to`.
#[derive(Clone, Copy)]
pub struct Turn {
    t: f32,
    from: Vec3,
    to: Vec3,
    entry: f32,
    target: f32,
    brake: TurnPhase,
    accel: TurnPhase,
}

/// Moving on a scripted speed curve (m/s over the run's normalised time).
#[derive(Clone, Copy)]
pub struct Run {
    t: f32,
    length: f32,
    /// From here, steering within RUN_TURN_LIMIT hands control back.
    free_at: f32,
    dir: Vec3,
    speeds: &'static [(f32, f32)],
    /// The curve's values are multiplied by this (1 for absolute speeds).
    scale: f32,
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
    /// Your velocity at the press: a shot slides on from it.
    pub entry: Vec3,
    pub from_dribble: bool,
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
            sprint_running: false,
            extra_effort: MAX_EXTRA_EFFORT,
            extra_active: false,
            has_ball: false,
            kick: None,
            locked: false,
            lock_time: 0.0,
            had_ball: false,
            relock_needed: false,
            dash: None,
            dash_cooldown: 0.0,
            second_dash_cooldown: 0.0,
            dribble: None,
            dribble_lockout: 0.0,
            push_buffered: false,
            run: None,
            receiving: None,
            turn: None,
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
    /// A push ball from inside a dribble.
    DribblePushBall,
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

    /// The force uses at least this much hold time.
    pub fn buildup(self) -> f32 {
        match self {
            Kick::Shot => SHOT_BUILDUP,
            Kick::Pass => PASS_BUILDUP,
            Kick::Lob => LOB_BUILDUP,
            Kick::ShortPass | Kick::ShortLob => SHORT_WINDUP,
            Kick::PushBall | Kick::DribblePushBall => PUSH_BALL_WINDUP,
        }
    }
}

impl Kick {
    /// Passes and lobs follow the pass-aim option; shots and push balls don't.
    pub fn is_pass(self) -> bool {
        matches!(self, Kick::Pass | Kick::ShortPass | Kick::Lob | Kick::ShortLob)
    }
}

impl Player {
    /// Charging a shot: the mouse curves the ball instead of turning the camera.
    pub fn charging_shot(&self) -> bool {
        self.kick.is_some_and(|k| k.kind == Kick::Shot && !k.released)
    }

    /// While charging, the mouse feeds the curve or a virtual stick instead of the camera.
    pub fn mouse_steers_kick(&self, aim: PassAim) -> bool {
        self.charging_shot() || (aim == PassAim::JoystickEmulation && self.kick.is_some_and(|k| k.kind.is_pass() && !k.released))
    }
}

/// Where the crosshair meets the arena walls: the game traces up to 150 m from the camera along
/// its view, ignoring the floor, and kicks fly from the ball toward that point.
fn crosshair_point(camera: Vec3, yaw_rad: f32) -> Vec3 {
    let dir = forward(yaw_rad);
    let to_wall = |from: f32, d: f32, half: f32| if d.abs() < 1e-6 { f32::INFINITY } else { (half * d.signum() - from) / d };
    let t = to_wall(camera.x, dir.x, HALF_WIDTH).min(to_wall(camera.z, dir.z, HALF_LENGTH)).min(SHOT_TRACE_RANGE);
    camera + dir * t
}

/// The yaw (radians) from the ball to the crosshair's point, for a camera looking along `yaw_deg`.
pub fn crosshair_yaw(camera: Vec3, yaw_deg: f32, ball: Vec3) -> f32 {
    yaw_of(crosshair_point(camera, yaw_deg.to_radians()) - ball)
}

/// Where a pass goes right now, for the chosen aim mode (yaw in radians). Without a stick
/// direction it goes toward the crosshair (`crosshair`, from `crosshair_yaw`).
pub fn pass_yaw(aim: PassAim, axes: &Axes, rig: &CameraRig, crosshair: f32) -> f32 {
    let (stick, dead_zone) = match aim {
        PassAim::CrosshairOnly => (Vec2::ZERO, 1.0),
        PassAim::DirectionalAiming => (axes.movement, PASS_STICK_DEAD_ZONE),
        PassAim::JoystickEmulation => (axes.curve, PASS_MOUSE_DEAD_ZONE),
    };
    if stick.length() < dead_zone {
        return crosshair;
    }
    let dir = camera_relative(stick.normalize(), rig.yaw);
    let yaw = (-dir.x).atan2(-dir.z);
    // Straight ahead snaps to the crosshair within a few degrees (the "Top" tolerance).
    let off = (yaw - rig.yaw.to_radians() + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    if off.abs() < PASS_FORWARD_SNAP.to_radians() { crosshair } else { yaw }
}

/// Camera orbit angles, in degrees. Pitch + is up.
#[derive(Component, Default)]
pub struct CameraRig {
    pub yaw: f32,
    pub pitch: f32,
}

/// Yaw (radians) a horizontal direction points at; the inverse of `forward`.
fn yaw_of(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

/// Shortest signed angle from `from` to `to`, radians.
fn angle_diff(to: f32, from: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn forward(yaw_rad: f32) -> Vec3 {
    Vec3::new(-yaw_rad.sin(), 0.0, -yaw_rad.cos())
}

/// Where an extra effort touch sends the ball: the movement input relative to the camera, or
/// the way your body faces with no input (the fallback isn't read from the game).
fn control_tap_dir(movement: Vec2, cam_yaw_deg: f32, facing: f32) -> Vec3 {
    if movement == Vec2::ZERO { forward(facing) } else { camera_relative(movement.normalize(), cam_yaw_deg).normalize() }
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
    bindings: Res<Bindings>,
) {
    let (ptf, p) = *player;
    if actions.just_pressed(Action::LookAtBall) {
        let to_ball = ball.translation - ptf.translation;
        rig.yaw = (-to_ball.x).atan2(-to_ball.z).to_degrees();
    }
    let look = if p.mouse_steers_kick(bindings.pass_aim) { axes.look_pad } else { axes.look_pad + axes.look_mouse };
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

    // Lock move follows the button, with two rules from the stance ability: with the ball it lasts
    // at least 0.15 s (the release is only checked then), and losing the ball ends it until you
    // press again.
    let held = actions.held(Action::Lock);
    if !held {
        p.relock_needed = false;
    }
    if p.had_ball && !p.has_ball && p.locked {
        p.relock_needed = true;
    }
    let min_stance = p.locked && p.has_ball && p.lock_time < STANCE_MIN_TIME;
    p.locked = (held && !p.relock_needed) || min_stance;
    p.lock_time = if p.locked { p.lock_time + dt } else { 0.0 };
    p.had_ball = p.has_ball;
    if p.locked || p.extra_active || p.dribble.is_some() || p.dash.is_some() {
        p.turn = None; // other movement takes over from a turn transition
    }

    // Dribble (notes/dribble.md): the dash you do with the ball, from lock move, on the push ball
    // or dash button. From 0.2 s into it the same press chains the feint; nothing chains after a
    // feint, and a tired dribble can't chain at all.
    p.dribble_lockout -= dt;
    let dribble_press = actions.just_pressed(Action::PushBall) || actions.just_pressed(Action::Dash);
    if dribble_press && p.has_ball && p.locked && p.kick.is_none() {
        let feint = !p.exhausted && p.dribble.is_some_and(|d| !d.feint && !d.exhausted && d.t >= DRIBBLE_WINDOW_OPENS);
        if feint || (p.dribble.is_none() && p.dribble_lockout <= 0.0) {
            // No direction is a forward dribble. Keyboard diagonals sit on the 45/135 deg sector edges:
            // W+A and W+D are fast cuts, S+A and S+D side dribbles going back (players confirm).
            let dir = if input == Vec2::ZERO { Vec2::Y } else { input.normalize() };
            let heading = dribble_heading(dir);
            // A fast cut goes where the camera looks (flick, then dribble) and turns you that way;
            // side dribbles and drag backs keep you facing forward.
            let dir = camera_relative(dir, rig.yaw);
            let facing = if heading == Heading::Forward { (-dir.x).atan2(-dir.z) } else { rig.yaw.to_radians() };
            p.dribble = Some(DribbleState { t: 0.0, feint, exhausted: p.exhausted, heading, dir, facing });
            if feint {
                p.dribble_lockout = FEINT_LOCKOUT;
            }
            p.stamina = (p.stamina - DRIBBLE_COST).max(0.0);
            p.regen_wait = DRIBBLE_REGEN_DELAY;
        }
    }

    // Stamina (mechanics.md §1). Lock move and dashes can't sprint. Holding sprint standing still
    // still counts as sprinting (a push ball from it is a sprint push ball), but stamina only
    // drains while you move (players confirm).
    p.sprinting = actions.pressed(Action::Sprint) && !p.locked && p.dash.is_none();
    let running = p.sprinting && input != Vec2::ZERO;
    if running && !p.sprint_running && !p.exhausted {
        p.stamina -= SPRINT_START_COST;
    }
    p.sprint_running = running;
    if running {
        p.stamina -= if p.has_ball { BALL_SPRINT_DRAIN } else { SPRINT_DRAIN } * dt;
        p.regen_wait = REGEN_DELAY;
    }
    if p.stamina <= 0.0 && !p.exhausted {
        p.exhausted = true;
        p.regen_wait = EXHAUSTED_REGEN_DELAY;
    }
    p.stamina = p.stamina.max(0.0);
    p.regen_wait -= dt;
    if !running && p.regen_wait <= 0.0 {
        let rate = if p.exhausted { EXHAUSTED_REGEN } else { STAMINA_REGEN };
        p.stamina = (p.stamina + rate * dt).min(MAX_STAMINA);
    }
    if p.exhausted && p.stamina >= EXHAUSTED_EXIT {
        p.exhausted = false;
    }

    // Extra effort: its own gauge, instant acceleration (mechanics.md §10)
    // No extra effort (or sprint) in lock move: its layers remove both.
    if p.locked {
        p.extra_active = false;
    }
    if actions.just_pressed(Action::ExtraEffort) && p.extra_effort > 0.05 && !p.locked {
        p.extra_effort -= EXTRA_EFFORT_START_COST;
        p.extra_active = true;
    }
    if p.extra_active {
        p.extra_effort -= EXTRA_EFFORT_DRAIN * dt;
        if !actions.pressed(Action::ExtraEffort) || p.extra_effort <= 0.0 {
            p.extra_active = false;
        }
    } else {
        let regen = if p.stamina >= MAX_STAMINA { EXTRA_EFFORT_REGEN_FULL_STAMINA } else { EXTRA_EFFORT_REGEN };
        p.extra_effort = (p.extra_effort + regen * dt).min(MAX_EXTRA_EFFORT);
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
    if p.kick.is_some_and(|k| !matches!(k.kind, Kick::PushBall | Kick::DribblePushBall)) {
        top *= WINDUP_SPEED_SCALE;
    }

    // Dash (mechanics.md §8): only without the ball, from lock move. While the first dash's cooldown
    // runs, a second (weaker) dash can cut into it from 0.24 s; then both wait for its own cooldown.
    p.dash_cooldown -= dt;
    p.second_dash_cooldown -= dt;
    if actions.just_pressed(Action::Dash) && !p.has_ball && p.second_dash_cooldown <= 0.0 {
        let first = p.dash.is_none() && p.locked && p.dash_cooldown <= 0.0;
        let second = p.dash_cooldown > 0.0 && p.dash.is_none_or(|d| !d.second && d.t >= SECOND_DASH_FROM);
        if first || second {
            // Same sectors as dribbles (players confirm): forward and the W+A / W+D diagonals turn
            // you that way, side steps and dashes back keep you facing forward. No direction
            // dashes back.
            let (dir, heading) = if input == Vec2::ZERO {
                (-forward(p.facing), Heading::Back)
            } else {
                (camera_relative(input.normalize(), rig.yaw).normalize(), dribble_heading(input.normalize()))
            };
            let facing = if heading == Heading::Forward { yaw_of(dir) } else { rig.yaw.to_radians() };
            p.dash = Some(DashState { t: 0.0, second: !first, dir, facing });
            if first {
                p.dash_cooldown = DASH_COOLDOWN;
            } else {
                p.second_dash_cooldown = DASH_COOLDOWN;
            }
            p.stamina = (p.stamina - DASH_COST).max(0.0);
            p.regen_wait = REGEN_DELAY;
        }
    }

    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    if let Some(mut r) = p.run {
        r.t += dt;
        p.vel = r.dir * curve(r.speeds, (r.t / r.length).min(1.0)) * r.scale;
        // Once free, steering within the turn limit hands back to normal running; sharper turns wait.
        let sharp = wish != Vec3::ZERO && r.dir.angle_between(wish).to_degrees() > RUN_TURN_LIMIT;
        p.run = (r.t < r.length && (r.t < r.free_at || sharp)).then_some(r);
    } else if let Some(mut d) = p.dribble {
        d.t += dt;
        let diff = (d.facing - p.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let max_turn = DRIBBLE_TURN_RATE.to_radians() * dt;
        p.facing += diff.clamp(-max_turn, max_turn);
        p.vel = d.dir * curve(d.speeds(), (d.t / DRIBBLE_SPEED_TIME).min(1.0));
        p.dribble = (d.t < d.length()).then_some(d);
    } else if let Some(k) = p.kick.filter(|k| k.kind == Kick::Shot) {
        // A shot keeps your run-up: no steering, sliding on at a shrinking share of your speed.
        let end = if k.from_dribble {
            SHOT_SLIDE_FROM_DRIBBLE
        } else if p.sprinting && k.entry.length() >= 4.0 {
            SHOT_SLIDE_SPRINT
        } else {
            SHOT_SLIDE
        };
        p.vel = k.entry * (1.0 + (end - 1.0) * (k.elapsed / SHOT_BUILDUP).min(1.0));
    } else if let Some(mut d) = p.dash {
        d.t += dt;
        // Turn as a dribble would. Dashing onto the ball controls it and carries it on in front,
        // the way you dash, like a fast cut (players confirm): then the body faces the dash.
        let target = if p.has_ball { yaw_of(d.dir) } else { d.facing };
        let max_turn = DRIBBLE_TURN_RATE.to_radians() * dt;
        p.facing += angle_diff(target, p.facing).clamp(-max_turn, max_turn);
        let speeds: &[(f32, f32)] = if d.second { &SECOND_DASH_SPEED } else { &DASH_SPEED };
        p.vel = d.dir * curve(speeds, d.t / DASH_DURATION);
        p.dash = (d.t < DASH_DURATION).then_some(d);
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
    } else if p.extra_active && wish != Vec3::ZERO {
        // Extra effort accelerates almost instantly, so it changes direction by acceleration rather
        // than by turning: much snappier than a sprint (players confirm). The body follows.
        let target = wish.normalize() * top * input.length();
        p.vel = p.vel.move_towards(target, accel * dt);
        let yaw = (-p.vel.x).atan2(-p.vel.z);
        let diff = (yaw - p.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let max_turn = turn.to_radians() * dt;
        p.facing += diff.clamp(-max_turn, max_turn);
    } else if let Some(mut tr) = p.turn.filter(|_| !p.locked) {
        // Turn transition: brake along the old direction, then re-accelerate along the new one
        // (the direction can still change until the re-acceleration starts).
        tr.t += dt;
        let (brake, accel_phase) = (tr.brake, tr.accel);
        if tr.t < brake.0 {
            p.vel = tr.from * tr.entry * curve(brake.1, tr.t / brake.0);
            if wish != Vec3::ZERO {
                tr.to = wish.normalize();
            }
        } else {
            p.vel = tr.to * tr.target * curve(accel_phase.1, ((tr.t - brake.0) / accel_phase.0).min(1.0));
        }
        let body_turn = FACE_HEADING_TURN_RATE.to_radians() * dt;
        p.facing += angle_diff(yaw_of(tr.to), p.facing).clamp(-body_turn, body_turn);
        p.turn = (tr.t < brake.0 + accel_phase.0).then_some(tr);
    } else if wish != Vec3::ZERO && speed > 1.0 && {
        // A big change of direction starts a turn transition (the game's code-driven turns).
        let bucket = turn_bucket(p.vel.angle_between(wish).to_degrees());
        let sprinting = p.sprint_running && speed > RUN_SPEED;
        if let Some((brake, accel_phase, exit)) = (bucket > 0).then(|| turn_data(sprinting, p.has_ball, bucket)).flatten() {
            p.turn = Some(Turn { t: 0.0, from: p.vel.normalize(), to: wish.normalize(), entry: speed, target: top * exit, brake, accel: accel_phase });
        }
        p.turn.is_some()
    } {
        // Started a turn this frame: the next frames run it.
    } else if wish != Vec3::ZERO {
        // Your heading turns toward the input at the turn rate (standing still: at once). The body
        // then turns to face your heading on its own, without dragging it: leaving lock move while
        // backing off turns you round while you keep moving back (ToFreemove transitions keep the
        // velocity), instead of arcing sideways.
        let heading_now = if speed < 0.5 { yaw_of(wish) } else { yaw_of(p.vel) };
        let max_turn = if speed < 0.5 { f32::INFINITY } else { turn.to_radians() * dt };
        let heading = heading_now + angle_diff(yaw_of(wish), heading_now).clamp(-max_turn, max_turn);
        let new_speed = (speed + accel * input.length() * dt).min(top * input.length());
        p.vel = forward(heading) * new_speed.max(speed.min(top) - STOP_DECEL * dt);
        let body_turn = FACE_HEADING_TURN_RATE.to_radians() * dt;
        p.facing += angle_diff(heading, p.facing).clamp(-body_turn, body_turn);
    } else {
        // Stopping takes the state's stop time from its top speed (stop transitions).
        let (state, full) = match (speed > RUN_SPEED + 0.25, p.has_ball) {
            (false, false) => (0, RUN_SPEED),
            (false, true) => (1, BALL_RUN_SPEED),
            (true, false) => (2, SPRINT_SPEED),
            (true, true) => (3, BALL_SPRINT_SPEED),
        };
        p.vel = p.vel.normalize_or_zero() * (speed - full / STOP_TIME[state] * dt).max(0.0);
    }

    tf.translation += p.vel * dt;
    tf.translation.x = tf.translation.x.clamp(-HALF_WIDTH + 0.4, HALF_WIDTH - 0.4);
    tf.translation.z = tf.translation.z.clamp(-HALF_LENGTH + 0.4, HALF_LENGTH - 0.4);
    tf.rotation = Quat::from_rotation_y(p.facing);
}

/// Whether a ball at `rel` (relative to you: x forward, y up, z right) moving at `vel` (same frame)
/// is inside one of your control zones (BC: the character's control hitboxes).
fn in_control_zone(rel: Vec3, vel: Vec3, speed: f32, locked: bool) -> bool {
    let capsule = |(fwd, radius, half): (f32, f32, f32)| {
        let p = rel - Vec3::X * fwd;
        let axis_y = p.y.clamp(-(half - radius), half - radius);
        (p - Vec3::Y * axis_y).length() < radius + BALL_HIT_RADIUS
    };
    let sphere = |(fwd, radius): (f32, f32)| (rel - Vec3::X * fwd).length() < radius + BALL_HIT_RADIUS;
    if locked {
        return capsule(CONTROL_LOCK_MOVE);
    }
    // A ball heading away from you is refused, unless you're running forward onto it.
    let coming = vel.length() < 0.5 || rel.angle_between(vel).to_degrees() > CONTROL_AWAY_ANGLE;
    if speed <= CONTROL_SWITCH_SPEED {
        coming && (capsule(CONTROL_STANDING) || sphere(CONTROL_STANDING_GROUND))
    } else {
        (coming && capsule(CONTROL_MOVING)) || capsule(CONTROL_MOVING_GROUND)
    }
}

pub fn ball_control(
    time: Res<Time>,
    axes: Res<Axes>,
    rig: Single<&CameraRig>,
    mut player: Single<(&Transform, &mut Player), Without<Ball>>,
    mut ball: Single<(&mut Transform, &mut Ball)>,
) {
    let (ptf, p) = &mut *player;
    let (btf, b) = &mut *ball;
    // Receiving: the ball settles at your feet and becomes yours 12 frames in; kicks wait for the
    // control's action frame.
    if let Some(t) = p.receiving {
        let t = t + time.delta_secs();
        p.receiving = (t < CONTROL_ACT).then_some(t);
        if !p.has_ball {
            let target = ptf.translation + forward(p.facing) * DRIBBLE_OFFSET;
            let target = Vec3::new(target.x, BALL_RADIUS, target.z);
            btf.translation = btf.translation.lerp(target, (t / CONTROL_SNAP).min(1.0));
            b.vel = p.vel;
            if t >= CONTROL_SNAP {
                p.has_ball = true;
                b.held = true;
            }
            return;
        }
    }
    if p.has_ball {
        // ponytail: ball glued in front of the feet; real dribble touches come later
        let target = ptf.translation + forward(p.facing) * DRIBBLE_OFFSET;
        btf.translation = Vec3::new(target.x, BALL_RADIUS, target.z);
        b.vel = p.vel;
        return;
    }
    let flat = Vec2::new(btf.translation.x - ptf.translation.x, btf.translation.z - ptf.translation.z);
    // Into your own frame: x forward, y up (from the pitch), z right.
    let fwd = forward(p.facing);
    let right = fwd.cross(Vec3::Y);
    let to_ball = Vec3::new(flat.x, 0.0, flat.y);
    let rel = Vec3::new(to_ball.dot(fwd), btf.translation.y, to_ball.dot(right));
    let vel = Vec3::new(b.vel.dot(fwd), b.vel.y, b.vel.dot(right));
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    let in_reach = b.no_control <= 0.0 && in_control_zone(rel, vel, speed, p.locked);
    if p.extra_active {
        // Extra effort can't control the ball: touching it taps it on at 12 m/s, flat, wherever
        // your movement input points at that moment (relative to the camera), whatever way your
        // body is still turning. Changing direction on the ball is just touching it with a new
        // input (extra-effort.md, the control tap).
        let dir = control_tap_dir(axes.movement, rig.yaw, p.facing);
        let touching = btf.translation.y < CONTROL_MAX_HEIGHT && b.no_control <= 0.0 && flat.length() < EXTRA_EFFORT_KNOCK_REACH;
        if touching && b.vel.dot(dir) < EXTRA_EFFORT_KNOCK_SPEED * 0.9 {
            b.vel = dir * EXTRA_EFFORT_KNOCK_SPEED;
            b.friction = EXTRA_EFFORT_FRICTION;
            b.spin = Vec3::ZERO;
        }
        return;
    }
    if in_reach {
        p.receiving = Some(0.0);
        b.spin = Vec3::ZERO;
    }
}

pub fn kicking(
    mut player: Single<&mut Player>,
    mut ball: Single<&mut Ball>,
    ball_tf: Single<&Transform, With<Ball>>,
    camera: Single<&Transform, (With<Camera3d>, Without<Ball>)>,
    rig: Single<&CameraRig>,
    actions: Res<Actions>,
    mut axes: ResMut<Axes>,
    bindings: Res<Bindings>,
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

    // Kicks wait for the first 0.2 s of a dribble, except shots, passes and lobs. Lock move (defence
    // stance) can shoot: the game's lock-move layer forbids it in the files, but players confirm
    // you can shoot from stance.
    // Shots, passes and lobs are open at any moment of a dribble (players confirm); the dribble
    // push ball waits for 0.2 s.
    let dribble_open = p.dribble.is_none_or(|d| d.t >= DRIBBLE_WINDOW_OPENS);
    // A push ball pressed early in a dribble waits for the window instead of being lost (the game
    // buffers 0.1 s; players find it more responsive than that).
    if p.dribble.is_none() {
        p.push_buffered = false;
    } else if !dribble_open && !p.locked && actions.just_pressed(Action::PushBall) {
        p.push_buffered = true;
    }
    let buffered_push = p.push_buffered && dribble_open;
    // A dash that picked up the ball doesn't block kicks (players confirm). A control does until
    // its action frame.
    if p.has_ball && p.kick.is_none() && p.receiving.is_none() {
        let charged = [(Action::Shoot, Kick::Shot), (Action::Pass, Kick::Pass), (Action::Lob, Kick::Lob)];
        // In lock move the push ball button dribbles instead (player_move); inside a dribble,
        // released from lock move, it's the dribble push ball.
        let push = if p.dribble.is_some() { Kick::DribblePushBall } else { Kick::PushBall };
        let instant = [(Action::LowPowerPass, Kick::ShortPass), (Action::LowPowerLob, Kick::ShortLob), (Action::PushBall, push)];
        let pressed = instant
            .into_iter()
            .chain(charged)
            .filter(|(a, _)| *a != Action::PushBall || !p.locked)
            .filter(|(a, _)| dribble_open || *a != Action::PushBall)
            .find(|(a, _)| actions.just_pressed(*a) || (*a == Action::PushBall && buffered_push));
        // Extra effort can't keep the ball: starting it touches the ball on ahead.
        let pressed = pressed.or((p.extra_active && dribble_open).then_some((Action::ExtraEffort, Kick::PushBall)));
        if let Some((_, kind)) = pressed {
            axes.reset_curve();
            let from_dribble = p.dribble.is_some();
            // Shots and push balls take over from the dribble; a pass or lob keeps its momentum
            // (players confirm), so the dribble runs on under it.
            if !kind.is_pass() {
                p.dribble = None;
                p.dash = None;
            }
            p.push_buffered = false;
            let released = kind.charge_action().is_none();
            p.kick = Some(KickState { kind, elapsed: 0.0, held: 0.0, released, aim: Vec2::new(rig.yaw, rig.pitch), entry: p.vel, from_dribble });
        }
    }

    let Some(k) = &mut p.kick else { return };
    k.elapsed += dt;
    if !k.released {
        if k.kind.charge_action().is_some_and(|a| actions.pressed(a)) {
            k.held = k.elapsed.min(MAX_HOLD);
        }
        // Release, or full power: a fully charged kick goes off by itself.
        k.released = k.held >= MAX_HOLD || !k.kind.charge_action().is_some_and(|a| actions.pressed(a));
    }
    if k.kind == Kick::Shot {
        // The gamepad's right stick keeps aiming a charging shot; the mouse curves it instead.
        k.aim.x -= axes.look_pad.x;
        k.aim.y = (k.aim.y + axes.look_pad.y).clamp(-PITCH_DOWN_LIMIT, PITCH_UP_LIMIT);
    }
    // The ball leaves on release, but never before the kick's contact frame (ShootDataTable):
    // a shot under 29.41 m/s uses the quicker low-power kick (20 frames, not 32).
    let contact = if k.kind == Kick::Shot && shot_speed(&k) < SHOT_HIGH_POWER_SPEED { SHOT_LOW_POWER_CONTACT } else { k.kind.buildup() };
    if !k.released || k.elapsed < contact {
        return;
    }
    let k = p.kick.take().expect("checked above");
    if !p.has_ball {
        return;
    }

    let t = k.held.max(k.kind.buildup()).min(MAX_HOLD); // force uses at least the build-up time
    let ratio = t / MAX_HOLD;
    let side = axes.curve.x;
    // Push balls go the way you steer, or straight on with no direction. The normal one loses
    // speed going sideways or back and gains it when sprinting; the dribble one only when sprinting.
    let push_dir = if axes.movement == Vec2::ZERO { forward(p.facing) } else { camera_relative(axes.movement.normalize(), rig.yaw).normalize() };
    let push_angle = forward(p.facing).angle_between(push_dir).to_degrees();
    let push_factor = match push_angle {
        a if a <= 60.0 => 1.0,
        a if a <= 95.0 => PUSH_BALL_SIDE_FACTOR,
        _ => PUSH_BALL_BACK_FACTOR,
    };
    let sprint = |factor: f32| if p.sprinting { factor } else { 1.0 };
    let (speed, elevation, friction, no_control) = match k.kind {
        // Height from the camera's pitch; a dip launches a little higher to make up for it.
        Kick::Shot => (
            shot_speed(&k),
            curve(&ELEVATION_BY_PITCH, k.aim.y) + SHOT_DIP_PITCH_CORRECTION * (-axes.curve.y).max(0.0),
            SHOT_FRICTION,
            NO_RECONTROL_TIME,
        ),
        Kick::Pass => (curve(&PASS_FORCE, t), curve(&PASS_LIFT_BY_HOLD, t), PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortPass => (SHORT_PASS_SPEED, 0.0, SHORT_PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::Lob => (curve(&LOB_FORCE, t), curve(&LOB_ANGLE_BY_HOLD, t), SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortLob => (SHORT_LOB_SPEED, SHORT_LOB_ANGLE, SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::PushBall if p.extra_active => (
            EXTRA_EFFORT_TAP_SPEED * auto_tap_factor(k.entry, push_dir, ball_tf.translation.y),
            0.0,
            EXTRA_EFFORT_FRICTION,
            PUSH_BALL_NO_CONTROL_EXTRA_EFFORT,
        ),
        // Tired push balls ignore sprinting and pop up a little.
        Kick::PushBall if p.exhausted => (EXHAUSTED_PUSH_BALL_SPEED * push_factor, EXHAUSTED_PUSH_BALL_LIFT, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
        Kick::DribblePushBall if p.exhausted => (EXHAUSTED_PUSH_BALL_SPEED, EXHAUSTED_PUSH_BALL_LIFT, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
        Kick::PushBall => (PUSH_BALL_SPEED * push_factor * sprint(PUSH_BALL_SPRINT_FACTOR), 0.0, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
        Kick::DribblePushBall => (DRIBBLE_PUSH_BALL_SPEED * sprint(DRIBBLE_PUSH_BALL_SPRINT_FACTOR), 0.0, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
    };
    let yaw = match k.kind {
        // The extra effort auto tap goes the way your body faces, not your input.
        Kick::PushBall if p.extra_active => p.facing,
        Kick::PushBall | Kick::DribblePushBall => (-push_dir.x).atan2(-push_dir.z),
        // Toward the crosshair's point on the walls (aim locked at the press), turned a little
        // against the curve so the ball bends back toward it.
        Kick::Shot => crosshair_yaw(camera.translation, k.aim.x, ball_tf.translation) + (side * AIM_NUDGE).to_radians(),
        _ => pass_yaw(bindings.pass_aim, &axes, &rig, crosshair_yaw(camera.translation, rig.yaw, ball_tf.translation)),
    };
    let flat = forward(yaw);
    let dir = (flat * elevation.to_radians().cos() + Vec3::Y * elevation.to_radians().sin()).normalize();

    p.has_ball = false;
    ball.held = false;
    ball.vel = dir * speed;
    ball.spin = if k.kind == Kick::Shot {
        // Flight-frame spin: z curves right, y dips. Pulling the curve input down dips the ball,
        // pushing it up lifts it (confirmed in-game); every shot also carries a small lift.
        let dip = curve(&TOP_SPIN, axes.curve.y) * curve(&TOP_SPIN_BY_CHARGE, ratio) + SHOT_BASE_SPIN;
        Vec3::new(0.0, dip, curve(&SIDE_SPIN, side) * curve(&SIDE_SPIN_BY_CHARGE, ratio))
    } else {
        // Passes and lobs carry the game's forced spin; low-power passes and push balls none.
        Vec3::Y
            * match k.kind {
                Kick::Pass => PASS_FORCED_SPIN,
                Kick::Lob => LOB_FORCED_SPIN,
                Kick::ShortLob => SHORT_LOB_FORCED_SPIN,
                _ => 0.0,
            }
    };
    ball.fall_time = 0.0;
    ball.drag = DRAG_FORCE;
    ball.friction = friction;
    ball.no_control = no_control;
    if k.kind == Kick::Shot {
        let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        p.run = Some(Run { t: 0.0, length: SHOT_FOLLOW_THROUGH, free_at: SHOT_FOLLOW_THROUGH, dir: k.entry.normalize_or_zero(), speeds: &[(0.0, 1.0), (1.0, 1.0)], scale: speed });
    }
    // After a push ball you chase it on the move's own speed (extra effort keeps its running).
    if matches!(k.kind, Kick::PushBall | Kick::DribblePushBall) && !p.extra_active {
        let cost = if k.kind == Kick::PushBall { PUSH_BALL_COST } else { DRIBBLE_PUSH_BALL_COST };
        p.stamina = (p.stamina - cost).max(0.0);
        p.regen_wait = PUSH_BALL_REGEN_DELAY;
        p.facing = yaw;
        let state = if p.exhausted { 2 } else if p.sprinting { 1 } else { 0 };
        let (speeds, length) = if k.kind == Kick::DribblePushBall {
            (DRIBBLE_PUSH_BALL_RUN[state], DRIBBLE_PUSH_BALL_LENGTH[Heading::of(push_angle, 60.0, 135.0) as usize])
        } else {
            let i = Heading::of(push_angle, 60.0, 150.0) as usize;
            ([PUSH_BALL_RUN, SPRINT_PUSH_BALL_RUN, EXHAUSTED_PUSH_BALL_RUN][state][i], PUSH_BALL_LENGTH[i])
        };
        p.run = Some(Run { t: 0.0, length: length - PUSH_BALL_WINDUP, free_at: PUSH_BALL_FREE - PUSH_BALL_WINDUP, dir: flat, speeds, scale: 1.0 });
    }
}

/// The extra effort auto tap's force multiplier: weaker when starting it turns you hard (from
/// your run at the press to where you steer), and for a ball off the ground.
fn auto_tap_factor(entry: Vec3, steer: Vec3, ball_height: f32) -> f32 {
    let run = Vec3::new(entry.x, 0.0, entry.z);
    let turn = if run.length() > 1.0 { run.angle_between(steer).to_degrees() } else { 0.0 };
    let by_turn = match turn {
        a if a >= 157.5 => EXTRA_EFFORT_TAP_IN_180_TURN,
        a if a >= 112.5 => EXTRA_EFFORT_TAP_IN_135_TURN,
        _ => 1.0,
    };
    let (high, factor) = EXTRA_EFFORT_TAP_HIGH_BALL;
    by_turn * if ball_height - BALL_RADIUS >= high { factor } else { 1.0 }
}

/// A shot's ball speed: the force curve by seconds held (at least the build-up), plus your run-up
/// speed's bonus, which only counts for stronger shots.
fn shot_speed(k: &KickState) -> f32 {
    let force = curve(&SHOT_FORCE, k.held.max(SHOT_BUILDUP).min(MAX_HOLD));
    let strength = (force - SHOT_FORCE[0].1) / (SHOT_FORCE[2].1 - SHOT_FORCE[0].1);
    force + curve(&SHOT_FORCE_BY_RUN_SPEED, k.entry.length()) * curve(&SHOT_RUN_BONUS_BY_FORCE, strength)
}

/// Freeplay ball call: the ball is passed to you from in front of the camera.
pub fn ball_call(
    actions: Res<Actions>,
    rig: Single<&CameraRig>,
    mut player: Single<(&Transform, &mut Player), Without<Ball>>,
    mut ball: Single<(&mut Transform, &mut Ball)>,
) {
    if !actions.just_pressed(Action::BallCall) {
        return;
    }
    let (ptf, p) = &mut *player;
    let (btf, b) = &mut *ball;
    p.has_ball = false;
    p.kick = None;
    p.receiving = None;
    let from = ptf.translation + forward(rig.yaw.to_radians()) * FEED_DISTANCE;
    let from = Vec3::new(
        from.x.clamp(-HALF_WIDTH + 1.0, HALF_WIDTH - 1.0),
        BALL_RADIUS,
        from.z.clamp(-HALF_LENGTH + 1.0, HALF_LENGTH - 1.0),
    );
    let to = Vec3::new(ptf.translation.x, BALL_RADIUS, ptf.translation.z);
    let flight = from.distance(to) / FEED_SPEED;
    **b = Ball { friction: PASS_FRICTION, ..default() };
    btf.translation = from;
    // A low arc that lands at your feet: enough vertical speed to come back down after `flight`.
    b.vel = (to - from).normalize_or_zero() * FEED_SPEED + Vec3::Y * GRAVITY * BALL_MASS * flight / 2.0;
}

pub fn camera_follow(
    player: Single<(&Transform, &Player), Without<Camera3d>>,
    rig: Single<&CameraRig>,
    bindings: Res<Bindings>,
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
        // The FOV option moves every state's FOV by the same amount (assumed).
        let state = if p.sprint_running || p.extra_active { SPRINT_FOV } else { FOV };
        let target = (state + bindings.fov - FOV).to_radians();
        persp.fov += (target - persp.fov) * (time.delta_secs() * 5.0).min(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dribble_sectors() {
        let d = Vec2::new(1.0, 1.0).normalize();
        assert_eq!(dribble_heading(Vec2::Y), Heading::Forward);
        assert_eq!(dribble_heading(d), Heading::Forward); // W+D: fast cut
        assert_eq!(dribble_heading(Vec2::new(-d.x, d.y)), Heading::Forward); // W+A
        assert_eq!(dribble_heading(Vec2::X), Heading::Side);
        assert_eq!(dribble_heading(-d), Heading::Side); // S+A: side, going back
        assert_eq!(dribble_heading(Vec2::NEG_Y), Heading::Back); // drag back
    }

    /// Runs player_move for `frames` frames of `dt` with `held` held (pressed on the first one).
    fn frames(app: &mut App, held: &[Action], frames: u32, dt: f32) {
        for _ in 0..frames {
            app.world_mut().resource_mut::<Actions>().hold(held);
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(dt));
            app.update();
        }
    }

    fn dribble(app: &mut App) -> Option<DribbleState> {
        app.world_mut().query::<&Player>().single(app.world()).unwrap().dribble
    }

    #[test]
    fn dribble_chain() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().spawn((Transform::default(), Player { has_ball: true, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        let (lock, press) = ([Action::Lock].as_slice(), [Action::Lock, Action::PushBall].as_slice());

        frames(&mut app, lock, 1, 0.016);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| !d.feint), "lock move + push ball dribbles");
        // Too early for the feint, then in its window.
        frames(&mut app, lock, 1, 0.016);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| !d.feint));
        frames(&mut app, lock, 5, 0.04);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| d.feint && d.t < 0.05), "the feint chains from 0.2 s");
        // Nothing chains after the feint.
        frames(&mut app, lock, 6, 0.04);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| d.feint && d.t > 0.2));
    }

    #[test]
    #[ignore = "activation cost on hold until the native cost code is read"]
    fn full_extra_effort_gives_three_cuts() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().spawn((Transform::default(), Player::default()));
        app.world_mut().spawn(CameraRig::default());
        let gauge = |app: &mut App| app.world_mut().query::<&Player>().single(app.world()).unwrap().extra_effort;
        let mut cuts = 0;
        for _ in 0..4 {
            let before = gauge(&mut app);
            frames(&mut app, &[Action::ExtraEffort], 1, 0.016);
            if gauge(&mut app) < before - 0.2 {
                cuts += 1;
            }
            frames(&mut app, &[Action::ExtraEffort], 10, 0.016); // a short cut, ~0.17 s held
            frames(&mut app, &[], 1, 0.016);
        }
        assert_eq!(cuts, 3, "a full gauge gives three cuts, gauge left {}", gauge(&mut app));
    }

    #[test]
    fn stance_backwards_dribbles_keep_going_back() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::NEG_Y; // holding S
        app.world_mut().spawn((Transform::default(), Player { has_ball: true, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        let (lock, press) = ([Action::Lock].as_slice(), [Action::Lock, Action::PushBall].as_slice());
        let mut log = Vec::new();
        for _ in 0..8 {
            frames(&mut app, press, 1, 0.016);
            frames(&mut app, lock, 15, 0.016);
            let (tf, p) = app.world_mut().query::<(&Transform, &Player)>().single(app.world()).unwrap();
            log.push((tf.translation, p.vel, p.dribble.map(|d| (d.heading, d.feint))));
        }
        // The camera looks down -Z, so going back is +Z; there should be no sideways or forward drift.
        let (end, ..) = log.last().unwrap();
        assert!(end.z > 2.0 && end.x.abs() < 0.2, "ended at {end:?}");
    }

    #[test]
    fn spamming_stance_while_backing_off_keeps_going_back() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::NEG_Y; // holding S
        app.world_mut().spawn((Transform::default(), Player { has_ball: true, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        frames(&mut app, &[Action::Lock], 30, 0.016);
        for _ in 0..10 {
            frames(&mut app, &[], 8, 0.016);
            frames(&mut app, &[Action::Lock], 8, 0.016);
        }
        let tf = app.world_mut().query::<&Transform>().iter(app.world()).next().copied().unwrap();
        // Backing off is +Z; there should be no sideways drift.
        assert!(tf.translation.z > 3.0 && tf.translation.x.abs() < 0.3, "ended at {:?}", tf.translation);
    }

    #[test]
    fn extra_effort_touch_follows_the_input_not_the_body() {
        // Running forward (-Z) with the camera behind, then steering right: the touch sends the
        // ball right at once, even though the body is still moving forward.
        let dir = control_tap_dir(Vec2::X, 0.0, 0.0);
        assert!(dir.x > 0.99, "right is +X: {dir:?}");
        // With no input it goes the way the body faces.
        assert!(control_tap_dir(Vec2::ZERO, 90.0, 0.0).z < -0.99);
        // Starting extra effort with the ball: full force straight on, x0.5 when it turns you round.
        let run = Vec3::new(0.0, 0.0, -6.0);
        assert_eq!(auto_tap_factor(run, Vec3::NEG_Z, BALL_RADIUS), 1.0);
        assert_eq!(auto_tap_factor(run, Vec3::Z, BALL_RADIUS), EXTRA_EFFORT_TAP_IN_180_TURN);
        assert_eq!(auto_tap_factor(run, Vec3::new(0.7, 0.0, 0.7).normalize(), BALL_RADIUS), EXTRA_EFFORT_TAP_IN_135_TURN);
    }

    #[test]
    fn control_zones() {
        let toward = Vec3::new(-5.0, 0.0, 0.0); // rolling at you
        let away = Vec3::new(5.0, 0.0, 0.0);
        let ground = |x: f32| Vec3::new(x, BALL_RADIUS, 0.0);
        // Standing: a ball rolling at you is taken within ~0.95 m of a point 0.1 m ahead.
        assert!(in_control_zone(ground(0.9), toward, 0.0, false));
        assert!(!in_control_zone(ground(1.2), toward, 0.0, false));
        // A ball rolling away is refused standing, but taken running forward onto it.
        assert!(!in_control_zone(ground(0.4), away, 0.0, false));
        assert!(in_control_zone(ground(0.4), away, 6.0, false));
        // Moving, the zone reaches further ahead; lock move catches from any side.
        assert!(in_control_zone(ground(1.25), toward, 4.0, false));
        assert!(in_control_zone(Vec3::new(0.0, BALL_RADIUS, 0.7), away, 0.0, true));
    }

    #[test]
    fn running_reversal_brakes_then_reaccelerates() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::Y; // run forward (-Z)
        app.world_mut().spawn((Transform::default(), Player::default()));
        app.world_mut().spawn(CameraRig::default());
        frames(&mut app, &[], 60, 1.0 / 60.0);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::NEG_Y; // reverse
        let mut max_side: f32 = 0.0;
        for _ in 0..30 {
            frames(&mut app, &[], 1, 1.0 / 60.0);
            let v = app.world_mut().query::<&Player>().single(app.world()).unwrap().vel;
            max_side = max_side.max(v.x.abs());
        }
        let v = app.world_mut().query::<&Player>().single(app.world()).unwrap().vel;
        // No sideways arc; after the 0.3 s brake and 0.3 s re-acceleration you go back at 60% of 5 m/s.
        assert!(max_side < 0.01, "sideways {max_side}");
        frames(&mut app, &[], 10, 1.0 / 60.0);
        let v2 = app.world_mut().query::<&Player>().single(app.world()).unwrap().vel;
        assert!(v.z >= 0.0 && v2.z > 2.5, "speed back {v2:?}");
    }

    #[test]
    fn crosshair_point_is_on_the_walls() {
        // From the centre looking at the -z goal, and from the right side angled across.
        assert!(crosshair_point(Vec3::new(0.0, 3.0, 0.0), 0.0).distance(Vec3::new(0.0, 3.0, -HALF_LENGTH)) < 1e-3);
        let p = crosshair_point(Vec3::new(10.0, 3.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!((p.x + HALF_WIDTH).abs() < 1e-3 && p.z.abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn pass_aim_modes() {
        let rig = CameraRig::default();
        let camera = rig.yaw.to_radians();
        let mut axes = Axes::default();
        axes.movement = Vec2::X;
        // Crosshair ignores the keys; directional sends the pass to the right.
        assert_eq!(pass_yaw(PassAim::CrosshairOnly, &axes, &rig, camera), camera);
        assert!(forward(pass_yaw(PassAim::DirectionalAiming, &axes, &rig, camera)).dot(forward(camera - std::f32::consts::FRAC_PI_2)) > 0.99);
        // Inside the dead zone, or a few degrees off straight ahead, it goes where the camera looks.
        axes.movement = Vec2::new(0.05, 0.2);
        assert_eq!(pass_yaw(PassAim::DirectionalAiming, &axes, &rig, camera), camera);
        axes.movement = Vec2::new(0.05, 1.0);
        assert_eq!(pass_yaw(PassAim::DirectionalAiming, &axes, &rig, camera), camera);
    }
}
