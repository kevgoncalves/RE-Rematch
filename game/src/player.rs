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
    /// A fresh extra effort activation's direction, waiting to cut the ball that way (time left).
    pub extra_cut: Option<(Vec3, f32)>,
    pub has_ball: bool,
    pub kick: Option<KickState>,
    pub locked: bool,
    pub dash: Option<DashState>,
    pub dash_cooldown: f32,
    pub second_dash_cooldown: f32,
    pub dribble: Option<DribbleState>,
    pub dribble_lockout: f32,
    /// A push ball pressed before the dribble's window opened, waiting for it.
    pub push_buffered: bool,
    /// Scripted chase after a push ball.
    pub run: Option<Run>,
}

#[derive(Clone, Copy)]
pub struct DashState {
    pub t: f32,
    pub second: bool,
    pub dir: Vec3,
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
            extra_cut: None,
            has_ball: false,
            kick: None,
            locked: false,
            dash: None,
            dash_cooldown: 0.0,
            second_dash_cooldown: 0.0,
            dribble: None,
            dribble_lockout: 0.0,
            push_buffered: false,
            run: None,
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

    /// Earliest ball contact from the press: a quick tap goes off this soon.
    fn contact(self) -> f32 {
        match self {
            Kick::Shot => SHOT_TAP_CONTACT,
            Kick::Pass => PASS_TAP_CONTACT,
            Kick::Lob => LOB_TAP_CONTACT,
            _ => self.buildup(),
        }
    }

    /// The force uses at least this much hold time.
    fn buildup(self) -> f32 {
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

/// Where a pass goes right now, for the chosen aim mode (yaw in radians).
pub fn pass_yaw(aim: PassAim, axes: &Axes, rig: &CameraRig) -> f32 {
    let (stick, dead_zone) = match aim {
        PassAim::CrosshairOnly => (Vec2::ZERO, 1.0),
        PassAim::DirectionalAiming => (axes.movement, PASS_STICK_DEAD_ZONE),
        PassAim::JoystickEmulation => (axes.curve, PASS_MOUSE_DEAD_ZONE),
    };
    let camera = rig.yaw.to_radians();
    if stick.length() < dead_zone {
        return camera;
    }
    let dir = camera_relative(stick.normalize(), rig.yaw);
    let yaw = (-dir.x).atan2(-dir.z);
    // Straight ahead snaps to the camera within a few degrees (the "Top" tolerance).
    let off = (yaw - camera + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    if off.abs() < PASS_FORWARD_SNAP.to_radians() { camera } else { yaw }
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

    p.locked = actions.held(Action::Lock);

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
    // Each activation costs a quarter of the gauge and needs it: a full gauge gives three cuts.
    if actions.just_pressed(Action::ExtraEffort) && p.extra_effort >= EXTRA_EFFORT_START_COST {
        p.extra_effort -= EXTRA_EFFORT_START_COST;
        p.extra_active = true;
        let dir = if wish != Vec3::ZERO { wish.normalize() } else { forward(p.facing) };
        p.extra_cut = Some((dir, EXTRA_EFFORT_CUT_WINDOW));
    }
    p.extra_cut = p.extra_cut.map(|(d, t)| (d, t - dt)).filter(|(_, t)| *t > 0.0 && p.extra_active);
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
            let dir = if wish != Vec3::ZERO { wish.normalize() } else { -forward(p.facing) };
            p.dash = Some(DashState { t: 0.0, second: !first, dir });
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
        p.vel = p.vel.normalize_or_zero() * (speed - STOP_DECEL * dt).max(0.0);
    }

    tf.translation += p.vel * dt;
    tf.translation.x = tf.translation.x.clamp(-HALF_WIDTH + 0.4, HALF_WIDTH - 0.4);
    tf.translation.z = tf.translation.z.clamp(-HALF_LENGTH + 0.4, HALF_LENGTH - 0.4);
    // ponytail: no animations yet, so a dribble leans the body into its move to make it readable.
    let lean = p.dribble.map_or(Quat::IDENTITY, |d| Quat::from_axis_angle(Vec3::Y.cross(d.dir).normalize(), 0.035 * p.vel.length()));
    tf.rotation = lean * Quat::from_rotation_y(p.facing);
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
    let in_reach = b.no_control <= 0.0 && flat.length() < radius && btf.translation.y < CONTROL_MAX_HEIGHT;
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    // A dash can't keep the ball: running into it takes it along the dash's direction.
    if let Some(d) = p.dash {
        let touching = flat.length() < EXTRA_EFFORT_KNOCK_REACH && btf.translation.y < CONTROL_MAX_HEIGHT;
        if b.no_control <= 0.0 && touching && b.vel.dot(d.dir) < speed {
            b.vel = d.dir * (speed + DASH_TOUCH_LEAD) + Vec3::Y * b.vel.y.max(0.0);
            b.friction = PUSH_BALL_FRICTION;
        }
        return;
    }
    if p.extra_active {
        // A fresh activation near the ball cuts it the new way, weaker on a sharp turn.
        if let Some((dir, _)) = p.extra_cut
            && b.no_control <= 0.0
            && flat.length() < EXTRA_EFFORT_CUT_REACH
            && btf.translation.y < CONTROL_MAX_HEIGHT
        {
            let turn = if b.vel.length() > 0.5 { b.vel.angle_between(dir).to_degrees() } else { 0.0 };
            let force = EXTRA_EFFORT_KNOCK_SPEED * curve(&EXTRA_EFFORT_TURN_FORCE, turn);
            b.vel = dir * force + Vec3::Y * b.vel.y.max(0.0);
            b.friction = EXTRA_EFFORT_FRICTION;
            p.extra_cut = None;
            return;
        }
        // Extra effort can't control the ball. Running into it taps it on the way you're running,
        // and changing direction with it at your feet cuts it the new way, weaker on a sharp turn
        // (players confirm; three cuts per full gauge).
        let moving = Vec3::new(p.vel.x, 0.0, p.vel.z);
        let dir = if moving.length() > 0.5 { moving.normalize() } else { forward(p.facing) };
        let low = btf.translation.y < CONTROL_MAX_HEIGHT && b.no_control <= 0.0;
        let ball_flat = Vec3::new(b.vel.x, 0.0, b.vel.z);
        let turn = if ball_flat.length() > 0.5 { ball_flat.angle_between(dir).to_degrees() } else { 0.0 };
        let cut = low && flat.length() < EXTRA_EFFORT_CUT_REACH && turn > EXTRA_EFFORT_CUT_ANGLE;
        let knock = low && flat.length() < EXTRA_EFFORT_KNOCK_REACH && b.vel.dot(dir) < moving.length();
        if cut || knock {
            b.vel = dir * EXTRA_EFFORT_KNOCK_SPEED * curve(&EXTRA_EFFORT_TURN_FORCE, turn) + Vec3::Y * b.vel.y.max(0.0);
            b.friction = EXTRA_EFFORT_FRICTION;
        }
        return;
    }
    if in_reach {
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

    // No kicks during a dash or the first 0.2 s of a dribble (frame-data.md). Lock move (defence
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
    if p.has_ball && p.kick.is_none() && p.dash.is_none() {
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
    // The ball leaves on release, or soon after the press for a quick tap.
    if !k.released || k.elapsed < k.kind.contact() {
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
        Kick::Shot => (
            curve(&SHOT_FORCE, t) + curve(&SHOT_FORCE_BY_RUN_SPEED, k.entry.length()),
            curve(&ELEVATION_BY_PITCH, k.aim.y),
            SHOT_FRICTION,
            NO_RECONTROL_TIME,
        ),
        Kick::Pass => (curve(&PASS_FORCE, t), PASS_ELEVATION, PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortPass => (SHORT_PASS_SPEED, PASS_ELEVATION, SHORT_PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::Lob => (curve(&LOB_FORCE, t), LOB_ELEVATION, SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortLob => (SHORT_LOB_SPEED, LOB_ELEVATION, SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::PushBall if p.extra_active => (
            EXTRA_EFFORT_TAP_SPEED,
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
        Kick::PushBall | Kick::DribblePushBall => (-push_dir.x).atan2(-push_dir.z),
        Kick::Shot => (k.aim.x - side * AIM_NUDGE).to_radians(), // locked at the press
        _ => pass_yaw(bindings.pass_aim, &axes, &rig),
    };
    let flat = forward(yaw);
    let dir = (flat * elevation.to_radians().cos() + Vec3::Y * elevation.to_radians().sin()).normalize();
    let right = flat.cross(Vec3::Y);

    p.has_ball = false;
    ball.held = false;
    ball.vel = dir * speed;
    ball.spin = if k.kind == Kick::Shot {
        // Spin around -Y curves the ball to the right of travel; `right` spin lifts it.
        // Pulling the curve input down dips the ball, pushing it up lifts it (confirmed in-game).
        let lift = -curve(&TOP_SPIN, axes.curve.y) * curve(&TOP_SPIN_BY_CHARGE, ratio) + SHOT_BASE_SPIN;
        Vec3::NEG_Y * curve(&SIDE_SPIN, side) * curve(&SIDE_SPIN_BY_CHARGE, ratio) + right * lift
    } else {
        Vec3::ZERO // ponytail: curve only on shots for now
    };
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
    b.vel = (to - from).normalize_or_zero() * FEED_SPEED + Vec3::Y * GRAVITY * flight / 2.0;
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
        let target = if p.sprint_running || p.extra_active { SPRINT_FOV } else { FOV }.to_radians();
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
    fn pass_aim_modes() {
        let rig = CameraRig::default();
        let camera = rig.yaw.to_radians();
        let mut axes = Axes::default();
        axes.movement = Vec2::X;
        // Crosshair ignores the keys; directional sends the pass to the right.
        assert_eq!(pass_yaw(PassAim::CrosshairOnly, &axes, &rig), camera);
        assert!(forward(pass_yaw(PassAim::DirectionalAiming, &axes, &rig)).dot(forward(camera - std::f32::consts::FRAC_PI_2)) > 0.99);
        // Inside the dead zone, or a few degrees off straight ahead, it goes where the camera looks.
        axes.movement = Vec2::new(0.05, 0.2);
        assert_eq!(pass_yaw(PassAim::DirectionalAiming, &axes, &rig), camera);
        axes.movement = Vec2::new(0.05, 1.0);
        assert_eq!(pass_yaw(PassAim::DirectionalAiming, &axes, &rig), camera);
    }
}
