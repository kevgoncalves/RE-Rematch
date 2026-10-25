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
    /// Lock move held all through the current dribble: only then does the next press feint.
    pub dribble_lock: bool,
    /// A push ball pressed before the dribble's window opened, waiting for it.
    pub push_buffered: bool,
    /// Scripted chase after a push ball.
    pub run: Option<Run>,
    /// Seconds into receiving the ball (the ball becomes yours at CONTROL_SNAP; you act from receive_act).
    pub receiving: Option<f32>,
    /// A kick pressed during a control, and how long it has been held: it waits for the control's
    /// action frame instead of being lost.
    pub queued: Option<(Action, f32)>,
    /// A turn transition in progress.
    pub turn: Option<Turn>,
    /// Movement input directions that stopped being current in the last 50 ms (time left), and
    /// the current one: a sharp change against them starts a turn.
    pub recent_input: Vec<(f32, Vec3)>,
    /// A quick pivot's time left (soft_hand_back).
    pub pivot: f32,
    /// Seconds since your last dribble ended.
    pub since_dribble: f32,
    /// Seconds since your own push ball's touch, and the way it went (the ippy).
    pub own_push: f32,
    pub push_way: Vec3,
    /// Whether that push ball was sprinted at the press (no ippy then, as in the game).
    pub push_sprinted: bool,
    /// Your magnet is off this long after an extra-effort touch on the ball or its end.
    pub magnet_off: f32,
    /// A fake shot: how long since it, and how long until you can act again.
    pub fake: Option<(f32, f32)>,
    /// How long extra effort has been on (0 when off), and an extra-effort tap's carry left.
    pub ee_on: f32,
    pub ee_carry: f32,
    /// An extra-effort cut's time left, and whether it cuts right.
    pub ee_cut: f32,
    pub ee_cut_right: bool,
    pub last_input: Vec3,
    /// A kick's freeze at the touch: you hold still this long (hitstop).
    pub freeze: f32,
    /// Seconds riding on the ball in extra effort, before it is tapped on.
    pub effort_ride: f32,
    /// The control's action frame.
    pub receive_act: f32,
    /// A turning control's new way.
    pub receive_way: Option<Vec3>,
    /// Running with the ball: its travel from one touch to the next.
    pub carry: Option<Carry>,
}

/// The ball between two touches while you run with it: it leaves the last touch's spot and slows
/// into the next touch's, a spot sprung toward where your foot will be.
#[derive(Clone, Copy, Debug)]
pub struct Carry {
    from: Vec3,
    /// The last touch and the next one (seconds, the app's clock).
    pub start: f32,
    pub next: f32,
    target: Vec3,
    touches: u32,
    /// When the dribble that owns the touches began.
    move_start: f32,
    /// Your velocity last frame, for how fast your way turns.
    vel: Vec3,
}

/// When a dribble's foot touches the ball, from its start (responsiveness.md): a fast cut at 17f,
/// side 8f, back at once; a feint 10f forward, 8f otherwise; a tired dribble 8f.
pub fn dribble_contact(d: &DribbleState) -> f32 {
    let frames = match (d.exhausted, d.feint, d.heading) {
        (true, ..) => 8.0,
        (_, true, Heading::Forward) => 10.0,
        (_, true, _) => 8.0,
        (_, _, Heading::Forward) => 17.0,
        (_, _, Heading::Side) => 8.0,
        (_, _, Heading::Back) => 0.0,
    };
    frames / 60.0
}

#[derive(Clone, Copy)]
pub struct DashState {
    pub t: f32,
    pub second: bool,
    pub dir: Vec3,
    /// The yaw you turn to, as in a dribble: the dash's own direction going forward, the camera's
    /// for side steps and dashes back.
    pub facing: f32,
    pub heading: Heading,
}

impl DashState {
    /// When you're free (the animation's blend-out).
    pub fn free(&self) -> f32 {
        (if self.second { SECOND_DASH_FREE } else { DASH_FREE })[self.heading as usize]
    }
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

    pub fn free(&self) -> f32 {
        let free = if self.exhausted { EXHAUSTED_DRIBBLE_FREE } else if self.feint { FEINT_FREE } else { DRIBBLE_FREE };
        free[self.heading as usize]
    }

    /// Still carrying you; after this the dribble only keeps its permissions to the animation's end.
    pub fn moving(&self) -> bool {
        self.t < self.free()
    }
}

/// A big change of direction: brake along `from`, then re-accelerate along `to`.
#[derive(Clone, Copy)]
pub struct Turn {
    /// Seconds into the turn (a catch-up starts it partway into the brake).
    t: f32,
    /// Your velocity's direction at the start: the brake slides along it.
    from: Vec3,
    /// The turn's current direction: the re-acceleration runs along it.
    cur: Vec3,
    entry: f32,
    /// The pivot as a share of top speed.
    share: f32,
    brake: TurnPhase,
    accel: TurnPhase,
    big: bool,
    ball: bool,
    sprint: bool,
    face_from: f32,
    face_t: f32,
    face_time: f32,
    /// Which way the body swings (+1 left, -1 right), fixed at the start: a turn to the exact
    /// opposite (a keyboard reversal) is otherwise either way, and flipped from frame to frame.
    side: f32,
}

impl Turn {
    /// For the body's turn pose: how far through the turn (0..1), the brake's share of it,
    /// whether it turns right, and whether it is a big one.
    pub fn progress(&self) -> (f32, f32, bool, bool) {
        let total = self.brake.0 + self.accel.0;
        ((self.t / total).clamp(0.0, 1.0), self.brake.0 / total, self.from.cross(self.cur).y < 0.0, self.big)
    }
}

/// Kev's softer hand-back out of a dribble, a pass or lob's drag or a shot (the game brakes and
/// pivots, which he finds too punishing): a quick pivot, your heading and body swinging round to
/// the stick together, with no brake.
fn soft_hand_back(p: &mut Player, dir_now: Vec3) {
    if dir_now == Vec3::ZERO {
        return;
    }
    p.pivot = PIVOT_TIME;
    p.turn = None;
    p.recent_input.clear();
}

/// Which way the body swings from `facing` to `dir`: the shorter way, or left for an exact reversal.
fn swing_side(facing: f32, dir: Vec3) -> f32 {
    let d = angle_diff(yaw_of(dir), facing);
    if d.abs() > std::f32::consts::PI - 0.05 { 1.0 } else { d.signum() }
}

/// When a dash or a push ball's chase hands control back with the stick against the way you're
/// going, you turn as from a run: brake to the pivot and re-accelerate, instead of arcing round
/// (players confirm a smooth change of way after a dash).
fn hand_back(p: &mut Player, dir_now: Vec3, walking: bool, top: f32) {
    if p.locked || dir_now == Vec3::ZERO {
        return;
    }
    if let Some(big) = turn_band(p.vel.angle_between(dir_now).to_degrees(), p.has_ball, walking) {
        p.turn = Some(start_turn(p, dir_now, big, p.vel.length(), top));
    }
}

/// How fast your body turns to face your heading, deg/s (the speed tables' rotation rates): without
/// the ball 600, 400 at full stick, 360 sprinting; with it 720 barely tilting the stick, else 360,
/// 180 sprinting; in extra effort 600, 450 at full stick.
fn body_turn_rate(p: &Player, stick: f32) -> f32 {
    match (p.extra_active, p.sprinting, p.has_ball) {
        (true, ..) => if stick >= 0.95 { 450.0 } else { 600.0 },
        (_, true, true) => 180.0,
        (_, true, false) => 360.0,
        (_, false, true) => if stick < 0.3 { 720.0 } else { 360.0 },
        (_, false, false) => if stick >= 0.95 { 400.0 } else { 600.0 },
    }
}

/// Starts a turn toward `dir` (turns.md §2). Entering slower than top speed, the game treats you
/// as at top speed and starts the brake where its curve gives your speed, at most 20% in.
fn start_turn(p: &Player, dir: Vec3, big: bool, speed: f32, top: f32) -> Turn {
    let (brake, accel, share) = turn_data(p.sprint_running, p.has_ball, big);
    let pivot = share * top;
    let (entry, skip) = if speed < top {
        let u = (0..=100).map(|i| i as f32 / 100.0).find(|u| pivot + (top - pivot) * curve(brake.1, *u) <= speed).unwrap_or(1.0);
        (top, u.min(TURN_CATCH_UP_MAX) * brake.0)
    } else {
        (speed, 0.0)
    };
    Turn {
        t: skip,
        from: Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or(dir),
        cur: dir,
        entry,
        share,
        brake,
        accel,
        big,
        ball: p.has_ball,
        sprint: p.sprint_running,
        face_from: p.facing,
        face_t: 0.0,
        face_time: TURN_FACE_TIME.min(brake.0 - skip),
        side: swing_side(p.facing, dir),
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
    /// A push ball's chase follows your live sprint: its walk, sprint and tired tables.
    tables: Option<[&'static [(f32, f32)]; 3]>,
    /// The curve's values are multiplied by this (1 for absolute speeds).
    scale: f32,
    /// Steering within this many degrees of the way leaves it, once free.
    exit: f32,
    /// Hand back softly at its end (soft_hand_back).
    soft: bool,
    /// The body's turn over the run: from, by how much (radians), over which stretch (s).
    face: Option<(f32, f32, f32, f32)>,
}

/// A kick from button press to ball contact. The build-up runs while the button is held.
#[derive(Clone, Copy)]
pub struct KickState {
    pub kind: Kick,
    pub elapsed: f32, // since the press
    pub held: f32,    // how long the button stayed down
    /// Held during the control before the kick could start: it counts toward the charge.
    pub pre: f32,
    pub released: bool,
    /// Shot aim (yaw, pitch in degrees), locked at the press so the mouse can curve the ball.
    pub aim: Vec2,
    /// The camera's position at the press. A shot's aim is traced from it, so sliding on through
    /// the kick doesn't move where you aimed (players confirm a shot's aim locks as it starts;
    /// passes and lobs don't lock).
    pub eye: Vec3,
    /// Your velocity at the press: a shot slides on from it.
    pub entry: Vec3,
    pub from_dribble: bool,
    /// Sprinting at the press: a push ball's sprint bonus reads this, not the touch.
    pub sprint: bool,
    /// The push direction at the press (zero with no stick): push balls go where you steered
    /// when you pressed, not at the touch.
    pub steer: Vec3,
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
            dribble_lock: false,
            push_buffered: false,
            run: None,
            receiving: None,
            queued: None,
            turn: None,
            recent_input: Vec::new(),
            pivot: 0.0,
            since_dribble: f32::INFINITY,
            own_push: f32::INFINITY,
            push_way: Vec3::ZERO,
            push_sprinted: false,
            ee_cut: 0.0,
            magnet_off: 0.0,
            fake: None,
            ee_on: 0.0,
            ee_carry: 0.0,
            ee_cut_right: false,
            last_input: Vec3::ZERO,
            freeze: 0.0,
            effort_ride: 0.0,
            receive_act: 16.0 / 60.0,
            receive_way: None,
            carry: None,
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

    /// Frames the kicker and the ball freeze at the touch (ShootDataTable's freeze frames): the
    /// power shot 5, quick shots, passes and lobs 3, low-power ones 2; push balls none.
    fn freeze_frames(self, speed: f32) -> f32 {
        match self {
            Kick::Shot if speed >= SHOT_HIGH_POWER_SPEED => 5.0,
            Kick::Shot | Kick::Pass | Kick::Lob => 3.0,
            Kick::ShortPass | Kick::ShortLob => 2.0,
            Kick::PushBall | Kick::DribblePushBall => 0.0,
        }
    }

    pub fn is_pass(self) -> bool {
        matches!(self, Kick::Pass | Kick::ShortPass | Kick::Lob | Kick::ShortLob)
    }
}

impl Player {
    /// A shot, pass or lob is still sliding you on after its touch: its follow-through holds.
    pub fn kick_sliding(&self) -> bool {
        self.run.is_some_and(|r| r.soft && r.tables.is_none())
    }

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
pub fn crosshair_point(camera: Vec3, yaw_rad: f32) -> Vec3 {
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
    let yaw = yaw_of(camera_relative(stick.normalize(), rig.yaw));
    // Straight ahead snaps to the crosshair within a few degrees (the "Top" tolerance).
    let off = angle_diff(yaw, rig.yaw.to_radians());
    if off.abs() < PASS_FORWARD_SNAP.to_radians() { crosshair } else { yaw }
}

/// Camera orbit angles, in degrees. Pitch + is up.
#[derive(Component, Default)]
pub struct CameraRig {
    pub yaw: f32,
    pub pitch: f32,
}

/// Yaw (radians) a horizontal direction points at; the inverse of `forward`.
pub fn yaw_of(dir: Vec3) -> f32 {
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

pub fn player_move(
    mut player: Single<(&mut Transform, &mut Player)>,
    rig: Single<&CameraRig>,
    mut actions: ResMut<Actions>,
    axes: Res<Axes>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let (tf, p) = &mut *player;
    // A kick's freeze holds your pose and timers for a few frames at the touch (hitstop), but you
    // slide on at your speed (Kev: stopping dead there felt like braking on every kick).
    if p.freeze > 0.0 {
        p.freeze -= dt;
        tf.translation += Vec3::new(p.vel.x, 0.0, p.vel.z) * dt;
        return;
    }
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
    if !p.locked {
        p.dribble_lock = false;
    }
    // Locking takes over from a kick's drag (not from a push ball's chase).
    if p.locked && p.run.is_some_and(|r| r.tables.is_none()) {
        p.run = None;
    }
    p.had_ball = p.has_ball;
    // Other movement takes over from a turn; gaining or losing the ball cancels it too.
    if p.locked || p.extra_active || p.dribble.is_some_and(|d| d.moving()) || p.dash.is_some() || p.turn.is_some_and(|t| t.ball != p.has_ball) {
        p.turn = None;
    }
    // Recent inputs: each one stays for 50 ms after it stops being current.
    let dir_now = wish.normalize_or_zero();
    for (life, _) in &mut p.recent_input {
        *life -= dt;
    }
    p.recent_input.retain(|(life, _)| *life > 0.0);
    if dir_now.angle_between(p.last_input).to_degrees() > 1.0 || (dir_now == Vec3::ZERO) != (p.last_input == Vec3::ZERO) {
        if p.last_input != Vec3::ZERO {
            let last = p.last_input;
            p.recent_input.push((TURN_INPUT_HISTORY, last));
        }
        p.last_input = dir_now;
    }
    let walking = input.length() < WALK_INPUT;
    // A move's direction needs a 0.3 tilt; less is no direction (keys are always full).
    let tap_input = if input.length() < MOVE_DIRECTION_DEAD_ZONE { Vec2::ZERO } else { input };

    // Dribble (notes/dribble.md): the dash you do with the ball, from lock move, on the push ball
    // or dash button. From 0.2 s into it the same press chains the feint; nothing chains after a
    // feint, and a tired dribble can't chain at all.
    // From a control's effect on the ball a dribble goes at once (Kev: no wait for the ball to
    // settle); the ball joins it as it does.
    let dribble_press = actions.pending(Action::PushBall) || actions.pending(Action::Dash);
    if dribble_press && (p.has_ball || p.receiving.is_some()) && p.locked && p.kick.is_none() && p.fake.is_none() {
        // The feint needs lock move held all through dribble 1 (Kev: let go and lock again, and the
        // next press is dribble 1 again, not the feint).
        let feint = !p.exhausted && p.dribble_lock && p.dribble.is_some_and(|d| !d.feint && !d.exhausted && d.t >= DRIBBLE_WINDOW_OPENS && d.t < d.length() - FEINT_WINDOW_CUT);
        // A new dribble 1 from DRIBBLE_CHAIN_LATE before a dribble 1's end, or with lock move let go
        // from DRIBBLE_REPEAT. After a feint, only once it ends (Kev; the game's 1 s lock after it
        // made dribbles late).
        let again = p.dribble.is_none_or(|d| !d.feint && (d.t >= d.length() - DRIBBLE_CHAIN_LATE || (!p.dribble_lock && d.t >= DRIBBLE_REPEAT)));
        if feint || again {
            // No direction is a forward dribble. Keyboard diagonals sit on the 45/135 deg sector edges:
            // W+A and W+D are fast cuts, S+A and S+D side dribbles going back (players confirm).
            let dir = if tap_input == Vec2::ZERO { Vec2::Y } else { tap_input.normalize() };
            let heading = dribble_heading(dir);
            // A fast cut goes where the camera looks (flick, then dribble) and turns you that way;
            // side dribbles and drag backs keep you facing forward.
            let dir = camera_relative(dir, rig.yaw);
            let facing = if heading == Heading::Forward { yaw_of(dir) } else { rig.yaw.to_radians() };
            p.dribble = Some(DribbleState { t: 0.0, feint, exhausted: p.exhausted, heading, dir, facing });
            // A new move takes over from a kick's drag (or any other scripted run).
            p.run = None;
            p.dribble_lock = true;
            actions.consume(Action::PushBall);
            actions.consume(Action::Dash);
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
    // Starting it takes its cost at once; pressing again while it still runs doesn't start it anew.
    if actions.just_pressed(Action::ExtraEffort) && p.extra_effort > 0.05 && !p.locked && !p.extra_active {
        p.extra_effort -= EXTRA_EFFORT_START_COST;
        p.extra_active = true;
    }
    if p.extra_active {
        p.extra_effort -= EXTRA_EFFORT_DRAIN * dt;
        // A tap still runs its first EE_MIN_ACTIVE (p.ee_on counts up to the last frame).
        let held = actions.pressed(Action::ExtraEffort) || p.ee_on + dt < EE_MIN_ACTIVE;
        if !held || p.extra_effort <= 0.0 {
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
    if p.fake.is_some() {
        top *= FAKE_SHOT_SPEED;
    }

    // Dash (mechanics.md §8): only without the ball, from lock move. While the first dash's cooldown
    // runs, a second (weaker) dash can cut into it from 0.24 s; then both wait for its own cooldown.
    p.dash_cooldown -= dt;
    p.second_dash_cooldown -= dt;
    if actions.pending(Action::Dash) && !p.has_ball && p.receiving.is_none() && p.second_dash_cooldown <= 0.0 {
        let first = p.dash.is_none() && p.locked && p.dash_cooldown <= 0.0;
        let second = p.dash_cooldown > 0.0 && p.dash.is_none_or(|d| !d.second && d.t >= SECOND_DASH_FROM);
        if first || second {
            // Same sectors as dribbles (players confirm): forward and the W+A / W+D diagonals turn
            // you that way, side steps and dashes back keep you facing forward. No direction
            // dashes back.
            let (dir, heading) = if tap_input == Vec2::ZERO {
                (-forward(p.facing), Heading::Back)
            } else {
                (camera_relative(tap_input.normalize(), rig.yaw).normalize(), dribble_heading(tap_input.normalize()))
            };
            let facing = if heading == Heading::Forward { yaw_of(dir) } else { rig.yaw.to_radians() };
            p.dash = Some(DashState { t: 0.0, second: !first, dir, facing, heading });
            p.run = None;
            actions.consume(Action::Dash);
            if first {
                p.dash_cooldown = DASH_COOLDOWN;
            } else {
                p.second_dash_cooldown = DASH_COOLDOWN;
            }
            p.stamina = (p.stamina - DASH_COST).max(0.0);
            p.regen_wait = REGEN_DELAY;
        }
    }

    p.since_dribble = if p.dribble.is_some() { 0.0 } else { p.since_dribble + dt };
    p.own_push += dt;
    p.ee_cut -= dt;
    p.magnet_off -= dt;
    p.ee_carry -= dt;
    if p.ee_on > 0.0 && !p.extra_active {
        let tap = p.ee_on < EE_START_PHASE;
        p.magnet_off = if tap { p.magnet_off.max(EE_END_NO_CONTROL) } else { 0.0 };
        if tap {
            p.ee_carry = EE_TAP_CARRY;
        }
    }
    p.ee_on = if p.extra_active { p.ee_on + dt } else { 0.0 };
    // After the hand-back a dribble keeps only its permissions, to the animation's end.
    if let Some(mut d) = p.dribble.filter(|d| !d.moving()) {
        d.t += dt;
        p.dribble = (d.t < d.length()).then_some(d);
    }

    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    if let Some(mut r) = p.run {
        r.t += dt;
        // A push ball's chase reads your sprint live (its ball's speed was set at the press).
        let speeds = r.tables.map_or(r.speeds, |t| t[if p.exhausted { 2 } else if p.sprinting { 1 } else { 0 }]);
        if let Some((from, by, start, end)) = r.face {
            let u = ((r.t - start) / (end - start)).clamp(0.0, 1.0);
            p.facing = from + by * u * u * (3.0 - 2.0 * u);
        }
        p.vel = r.dir * curve(speeds, (r.t / r.length).min(1.0)) * r.scale;
        // Once free, steering within the turn limit leaves the chase at once; with no stick or a
        // sharper turn it runs on to the blend-out (responsiveness.md §5).
        let exit = r.t >= r.free_at && wish != Vec3::ZERO && r.dir.angle_between(wish).to_degrees() <= r.exit;
        p.run = (r.t < r.length && !exit).then_some(r);
        if p.run.is_none() {
            if r.soft {
                soft_hand_back(p, dir_now);
            } else {
                hand_back(p, dir_now, walking, top);
            }
        }
    } else if let Some(drag) = p.kick.and_then(|k| pass_drag(&k)) {
        // A pass or lob carries you on at your speed at the press, that way (players confirm: a pass
        // out of a dribble carries you like a shot). No steering or turning till the touch: Rematch
        // won't, and turning mid-kick faked its angle (Kev).
        p.vel = drag;
    } else if let Some(mut d) = p.dribble.filter(|d| d.moving()) {
        d.t += dt;
        // The body turns to the dribble's way linearly over its redirect time.
        p.facing += angle_diff(d.facing, p.facing) * (dt / (DRIBBLE_TURN_TIME - (d.t - dt)).max(dt)).min(1.0);
        p.vel = d.dir * curve(d.speeds(), (d.t / DRIBBLE_SPEED_TIME).min(1.0));
        p.dribble = Some(d);
        if !d.moving() {
            soft_hand_back(p, dir_now);
        }
    } else if let Some(k) = p.kick.filter(|k| k.kind == Kick::Shot) {
        // A shot keeps your run-up, sliding on that way at a shrinking share of your speed.
        let end = if Vec3::new(k.entry.x, 0.0, k.entry.z).length() > BALL_SPRINT_SPEED {
            // Faster than you can run with the ball (an extra-effort tap's carry): all of it.
            1.0
        } else if k.from_dribble {
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
        p.facing += angle_diff(target, p.facing) * (dt / (DASH_TURN_TIME - (d.t - dt)).max(dt)).min(1.0);
        let speeds: &[(f32, f32)] = if d.second { &SECOND_DASH_SPEED } else { &DASH_SPEED };
        p.vel = d.dir * curve(speeds, d.t);
        // Free at the blend-out; earlier if you keep the stick within 15 deg of the dash's way
        // (a forward dash, facing where it goes).
        let exit_from = if d.second { SECOND_DASH_EXIT_FROM } else { DASH_EXIT_FROM };
        let keeps_way = wish != Vec3::ZERO
            && wish.angle_between(d.dir).to_degrees() <= DASH_EXIT_ANGLE
            && forward(p.facing).angle_between(d.dir).to_degrees() <= DASH_EXIT_ANGLE;
        p.dash = (d.t < d.free() && !(d.t >= exit_from && keeps_way)).then_some(d);
        if p.dash.is_none() {
            hand_back(p, dir_now, walking, top);
        }
    } else if p.locked {
        // Lock move (mechanics.md §2): face the camera, strafe, slower going backwards.
        let diff = angle_diff(rig.yaw.to_radians(), p.facing);
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
        // A sharp change of way is a cut: the body swings round at once with a plant-and-push.
        let flat = Vec3::new(p.vel.x, 0.0, p.vel.z);
        if p.ee_cut <= 0.0 && flat.length() > 3.0 && flat.angle_between(wish).to_degrees() > EE_CUT_ANGLE {
            p.ee_cut = EE_CUT_TIME;
            p.ee_cut_right = flat.cross(wish).y < 0.0;
        }
        p.vel = p.vel.move_towards(target, accel * dt);
        let diff = angle_diff(yaw_of(p.vel), p.facing);
        let max_turn = if p.ee_cut > 0.0 { EE_CUT_TURN } else { EE_BODY_TURN.max(turn) }.to_radians() * dt;
        p.facing += diff.clamp(-max_turn, max_turn);
    } else if p.pivot > 0.0 && wish != Vec3::ZERO {
        // A quick pivot: heading and body swing round together, your speed dipping while far off.
        p.pivot -= dt;
        let heading_now = if speed < 0.5 { yaw_of(wish) } else { yaw_of(p.vel) };
        let off = angle_diff(yaw_of(wish), heading_now);
        let max = PIVOT_RATE.to_radians() * dt;
        let heading = heading_now + off.clamp(-max, max);
        let target = if off.abs() > std::f32::consts::FRAC_PI_2 { top * PIVOT_SPEED } else { top * input.length() };
        let new_speed = speed + (target - speed).clamp(-STOP_DECEL * dt, accel * dt);
        p.vel = forward(heading) * new_speed;
        p.facing = heading;
        if off.abs() < 0.02 {
            p.pivot = 0.0;
        }
    } else if let Some(mut tr) = p.turn {
        // A new turn can cut into the re-acceleration late enough, measured from the turn's own
        // direction; never into the brake.
        let accel_ratio = (tr.t - tr.brake.0) / tr.accel.0;
        if dir_now != Vec3::ZERO
            && accel_ratio >= turn_cut_in(tr.big, tr.sprint)
            && let Some(big) = turn_band(tr.cur.angle_between(dir_now).to_degrees(), p.has_ball, walking)
        {
            tr = start_turn(p, dir_now, big, speed, top);
        }
        tr.t += dt;
        let in_brake = tr.t < tr.brake.0;
        // The turn's direction follows the input at a capped rate, only inside its windows:
        // faster turning away from the old direction than back toward it.
        let (away_rate, back_rate, brake_window, accel_window) = turn_steer(tr.ball, tr.big);
        let window = if in_brake { tr.t / tr.brake.0 < brake_window } else { (tr.t - tr.brake.0) / tr.accel.0 < accel_window };
        if window && dir_now != Vec3::ZERO {
            let away = tr.from.angle_between(dir_now) > tr.from.angle_between(tr.cur);
            let max = if away { away_rate } else { back_rate }.to_radians() * dt;
            tr.cur = forward(yaw_of(tr.cur) + angle_diff(yaw_of(dir_now), yaw_of(tr.cur)).clamp(-max, max));
        }
        // Brake along the old velocity down to the pivot, then back up to full top speed along the
        // turn's direction. The pivot follows the top speed every frame.
        let pivot = tr.share * top;
        p.vel = if in_brake {
            tr.from * (pivot + (tr.entry - pivot) * curve(tr.brake.1, tr.t / tr.brake.0))
        } else {
            tr.cur * (pivot + (top - pivot) * curve(tr.accel.1, ((tr.t - tr.brake.0) / tr.accel.0).min(1.0)))
        };
        // The body eases round to the turn's direction, then follows it.
        tr.face_t += dt;
        let target = yaw_of(tr.cur);
        p.facing = if tr.face_t < tr.face_time {
            let a = (tr.face_t / tr.face_time).clamp(0.0, 1.0);
            let mut d = angle_diff(target, tr.face_from);
            if d.abs() > std::f32::consts::PI - 0.05 {
                d = d.abs() * tr.side;
            }
            tr.face_from + d * a * a * (3.0 - 2.0 * a)
        } else {
            target
        };
        p.turn = (tr.t < tr.brake.0 + tr.accel.0).then_some(tr);
    } else if let Some(big) = (dir_now != Vec3::ZERO && speed > 0.5)
        .then(|| p.recent_input.iter().rev().find_map(|(_, old)| turn_band(old.angle_between(dir_now).to_degrees(), p.has_ball, walking)))
        .flatten()
    {
        // A sharp change of the movement input within 50 ms starts a turn (newest input first).
        // ponytail: no start transitions modelled, so a turn needs some speed (0.5 m/s).
        p.turn = Some(start_turn(p, dir_now, big, speed, top));
        p.recent_input.clear();
    } else if wish != Vec3::ZERO {
        // Your heading turns toward the input at the turn rate (standing still: at once). The body
        // then turns to face your heading on its own, without dragging it: leaving lock move while
        // backing off turns you round while you keep moving back (ToFreemove transitions keep the
        // velocity), instead of arcing sideways.
        let heading_now = if speed < 0.5 { yaw_of(wish) } else { yaw_of(p.vel) };
        let max_turn = if speed < 0.5 { f32::INFINITY } else { turn.to_radians() * dt };
        let heading = heading_now + angle_diff(yaw_of(wish), heading_now).clamp(-max_turn, max_turn);
        let new_speed = (speed + accel * input.length() * dt).min(top * input.length());
        let over_decel = if p.ee_carry > 0.0 { EE_TAP_DECEL } else { OVERSPEED_DECEL };
        p.vel = forward(heading) * new_speed.max(speed.min(top) - STOP_DECEL * dt).max(speed - over_decel * dt);
        let lag = angle_diff(heading, p.facing);
        // Past 90 deg behind your heading the body catches up a little faster.
        let catch_up = if lag.abs() > std::f32::consts::FRAC_PI_2 { BODY_CATCH_UP } else { 0.0 };
        let body_turn = (body_turn_rate(p, input.length()) + catch_up).to_radians() * dt;
        p.facing += lag.clamp(-body_turn, body_turn);
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
fn in_control_zone(rel: Vec3, vel: Vec3, speed: f32, locked: bool, turning: bool) -> bool {
    let capsule = |(fwd, radius, half): (f32, f32, f32)| {
        let p = rel - Vec3::X * fwd;
        let axis_y = p.y.clamp(-(half - radius), half - radius);
        (p - Vec3::Y * axis_y).length() < radius + BALL_HIT_RADIUS
    };
    let sphere = |(fwd, radius): (f32, f32)| (rel - Vec3::X * fwd).length() < radius + BALL_HIT_RADIUS;
    if locked {
        return capsule(CONTROL_LOCK_MOVE);
    }
    // A ball heading away from you is refused; a slow ball always passes (CONTROL_SLOW_BALL).
    let coming = vel.length() < CONTROL_SLOW_BALL || rel.angle_between(vel).to_degrees() > CONTROL_AWAY_ANGLE;
    // Running, the ground zone takes any ball about under you.
    let ground = speed > CONTROL_SWITCH_SPEED && capsule(CONTROL_MOVING_GROUND);
    // Starting or turning uses the moving zones even when slow.
    if speed <= CONTROL_SWITCH_SPEED && !turning {
        coming && (capsule(CONTROL_STANDING) || sphere(CONTROL_STANDING_GROUND))
    } else {
        (coming && capsule(CONTROL_MOVING)) || ground
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
    // Through a kick's freeze the ball waits at the foot as you slide on.
    if b.freeze > 0.0 {
        btf.translation += Vec3::new(p.vel.x, 0.0, p.vel.z) * time.delta_secs();
    }
    // Receiving: the ball settles at your feet and becomes yours 12 frames in; kicks wait for the
    // control's action frame.
    if let Some(t) = p.receiving {
        let t = t + time.delta_secs();
        p.receiving = (t < p.receive_act).then_some(t);
        if !p.has_ball {
            // The ball glides to your foot: it moves on with you and closes on the spot by the
            // control's touch frame, but never faster than CONTROL_PULL_SPEED against you; only
            // within 10 cm does it snap.
            let fwd = forward(p.facing);
            let target = match p.receive_way {
                // The ippy: the foot pulls the ball round in front of you as you swing.
                Some(_) => ptf.translation + fwd * CONTROL_TURN_SPOT,
                None => ptf.translation + fwd * DRIBBLE_OFFSET,
            };
            let target = Vec3::new(target.x, BALL_RADIUS, target.z);
            let dt = time.delta_secs();
            let to = Vec3::new(target.x - btf.translation.x, 0.0, target.z - btf.translation.z);
            let left = (CONTROL_SNAP - t + dt).max(dt);
            let cap = if p.receive_way.is_some() { CONTROL_PULL_SPEED.1 } else { CONTROL_PULL_SPEED.0 } * dt;
            let step = if to.length() < CONTROL_SNAP_NEAR { to } else { (to * (dt / left).min(1.0)).clamp_length_max(cap) };
            let before = btf.translation;
            let mut moved = Vec3::new(p.vel.x, 0.0, p.vel.z) * dt + step;
            // The ippy's ball keeps its momentum and eases into the pull.
            if p.receive_way.is_some() {
                let own = Vec3::new(b.vel.x, 0.0, b.vel.z) * dt;
                moved = own + (moved - own) * (dt / IPPY_BALL_BLEND).min(1.0);
            }
            btf.translation += moved;
            btf.translation.y += (BALL_RADIUS - btf.translation.y) * (dt / left).min(1.0);
            b.vel = (btf.translation - before) / dt.max(1e-4);
            if t >= CONTROL_SNAP {
                p.has_ball = true;
                b.held = true;
                p.carry = None;
            }
            return;
        }
    }
    if p.has_ball {
        let now = time.elapsed_secs();
        let dt = time.delta_secs();
        let before = btf.translation;
        let carried = carry_ball(p, ptf.translation, btf.translation, now, dt);
        // Never faster than CARRY_MAX_REL_SPEED against you, kicks aside.
        let own = Vec3::new(p.vel.x, 0.0, p.vel.z) * dt;
        let rel = carried - before - own;
        let rel = if p.kick.is_some() { rel } else { rel.clamp_length_max(CARRY_MAX_REL_SPEED * dt) };
        btf.translation = before + own + rel;
        b.vel = (btf.translation - before) / dt.max(1e-4);
        return;
    }
    p.carry = None;
    let flat = Vec2::new(btf.translation.x - ptf.translation.x, btf.translation.z - ptf.translation.z);
    // Into your own frame: x forward, y up (from the pitch), z right.
    let fwd = forward(p.facing);
    let right = fwd.cross(Vec3::Y);
    let to_ball = Vec3::new(flat.x, 0.0, flat.y);
    let rel = Vec3::new(to_ball.dot(fwd), btf.translation.y, to_ball.dot(right));
    let vel = Vec3::new(b.vel.dot(fwd), b.vel.y, b.vel.dot(right));
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    let starting = axes.movement.length() > MOVE_DIRECTION_DEAD_ZONE && speed <= CONTROL_SWITCH_SPEED;
    // Kev's ippy: after your own push ball, with the stick turned back, as soon as you may take the
    // ball again, if it is within reach you turn over it and take it with the back foot, out the new
    // way: every time, sprint or not (IPPY_*).
    let wish = (axes.movement.length() > MOVE_DIRECTION_DEAD_ZONE).then(|| camera_relative(axes.movement.normalize(), rig.yaw).normalize());
    if p.own_push < IPPY_WINDOW
        && !p.push_sprinted
        && !p.locked
        && p.dash.is_none()
        && b.no_control <= IPPY_TURN_LEAD
        && btf.translation.y < CONTROL_MAX_HEIGHT
        && flat.length() < IPPY_REACH + if p.pivot > 0.0 { IPPY_REACH_TURNING } else { 0.0 }
        && let Some(w) = wish.filter(|w| p.push_way.angle_between(*w).to_degrees() > IPPY_ANGLE)
    {
        // The swing starts a little before the ball may be taken...
        if p.pivot <= 0.0 {
            p.run = None;
            p.turn = None;
            p.pivot = PIVOT_TIME;
            p.recent_input.clear();
        }
        // ...and it is taken as soon as it may be.
        if b.no_control > 0.0 {
            return;
        }
        p.receiving = Some(0.0);
        p.receive_way = Some(w);
        p.receive_act = CONTROL_ACT_BY_HEIGHT[0].1;
        p.own_push = f32::INFINITY;
        b.spin = Vec3::ZERO;
        return;
    }
    // A dash pressed almost on top of a loose ball is a dribble at once (DASH_DRIBBLE_REACH).
    if let Some(d) = p.dash
        && d.t <= 2.0 / 60.0
        && b.no_control <= 0.0
        && btf.translation.y < CONTROL_MAX_HEIGHT
        && flat.length() < DASH_DRIBBLE_REACH
    {
        p.dribble = Some(DribbleState { t: d.t, feint: false, exhausted: p.exhausted, heading: d.heading, dir: d.dir, facing: d.facing });
        p.dribble_lock = p.locked;
        p.dash = None;
        p.has_ball = true;
        p.carry = None;
        b.held = true;
        b.spin = Vec3::ZERO;
        return;
    }
    if p.extra_active {
        // Extra effort can't control the ball: touching it taps it on at 12 m/s, flat, wherever
        // your movement input points at that moment (relative to the camera), whatever way your
        // body is still turning. Changing direction on the ball is just touching it with a new
        // input (extra-effort.md, the control tap).
        let dir = control_tap_dir(axes.movement, rig.yaw, p.facing);
        // Your own push ball's no-control doesn't stop extra effort running into it: overtaking it,
        // the body knocks it on (Kev: after a dribble push ball it was left behind).
        let way = Vec2::new(p.vel.x, p.vel.z).normalize_or_zero();
        let overtaking = flat.dot(way) < EE_OVERTAKE_AHEAD;
        let touching = btf.translation.y < CONTROL_MAX_HEIGHT && (b.no_control <= 0.0 || overtaking) && flat.length() < EXTRA_EFFORT_KNOCK_REACH;
        match touching.then(|| effort_touch(b.vel, dir, p.effort_ride)) {
            Some(EffortTouch::Ride) => {
                p.magnet_off = EE_TOUCH_NO_CONTROL;
                p.effort_ride += time.delta_secs();
                b.vel = Vec3::new(p.vel.x, b.vel.y, p.vel.z);
                // While you can chop, the ball sits under you, so the window shows.
                let under = ptf.translation + Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero() * EFFORT_RIDE_AHEAD;
                let pull = (time.delta_secs() / 0.08).min(1.0);
                btf.translation.x += (under.x - btf.translation.x) * pull;
                btf.translation.z += (under.z - btf.translation.z) * pull;
            }
            Some(EffortTouch::Tap) => {
                p.magnet_off = EE_TOUCH_NO_CONTROL;
                b.vel = dir * EXTRA_EFFORT_KNOCK_SPEED;
                b.friction = EXTRA_EFFORT_FRICTION;
                b.spin = Vec3::ZERO;
                p.effort_ride = 0.0;
            }
            Some(EffortTouch::Leave) | None => p.effort_ride = 0.0,
        }
        return;
    }
    let hands_off = p.own_push < OWN_PUSH_HANDS_OFF && wish.is_none();
    let in_reach = b.no_control <= 0.0 && p.magnet_off <= 0.0 && !hands_off && in_control_zone(rel, vel, speed, p.locked, starting || p.turn.is_some());
    if in_reach {
        p.receiving = Some(0.0);
        b.spin = Vec3::ZERO;
        p.receive_act = CONTROL_ACT_BY_HEIGHT.iter().find(|(h, _)| btf.translation.y < *h).map_or(22.0 / 60.0, |(_, act)| *act);
        // A control ends a push ball's chase, and a dash almost at once (mechanics.md: 8% in); you
        // move freely as it settles the ball (Kev: no carry, no delay).
        p.run = None;
        p.dash = None;
        p.receive_way = None;
    }
}

/// Where the ball is this frame while you run with it (TouchBallComponent): it leaves each touch's
/// spot fast and slows into the next one's, aimed a little ahead of where you'll be then. That spot
/// is sprung toward the newest prediction, hard just after a touch and softly later, so late stick
/// changes bend the ball's path; and the ball never strays far to your side.
fn carry_ball(p: &mut Player, root: Vec3, ball: Vec3, now: f32, dt: f32) -> Vec3 {
    // A shot, pass or lob draws the ball to the kicking foot by its contact: the right foot, or
    // the left for a shot across to the right (the mirrored clip), even from the stance.
    if let Some(k) = p.kick.filter(|k| !matches!(k.kind, Kick::PushBall | Kick::DribblePushBall)) {
        let fwd = forward(p.facing);
        let left_foot = k.kind == Kick::Shot && angle_diff(k.aim.x.to_radians(), p.facing).to_degrees() < -30.0;
        let side = if left_foot { -KICK_BALL_SPOT.1 } else { KICK_BALL_SPOT.1 };
        let spot = root + fwd * KICK_BALL_SPOT.0 + fwd.cross(Vec3::Y) * side;
        let ball = ball + Vec3::new(p.vel.x, 0.0, p.vel.z) * dt;
        let left = (contact_time(&k) - k.elapsed).max(dt);
        let to = Vec3::new(spot.x - ball.x, 0.0, spot.z - ball.z);
        let ball = ball + (to * (dt / left).min(1.0)).clamp_length_max(KICK_BALL_PULL * dt);
        return Vec3::new(ball.x, BALL_RADIUS, ball.z);
    }
    // Lock move with the ball: the right sole goes onto the ball (players confirm) and rolls it
    // with you, a little ahead the way you move; it eases there as you lock.
    if p.locked && p.dribble.is_none() {
        let fwd = forward(p.facing);
        let spot = root + fwd * STANCE_BALL.0 + fwd.cross(Vec3::Y) * STANCE_BALL.1 + Vec3::new(p.vel.x, 0.0, p.vel.z) * STANCE_BALL_LEAD;
        p.carry = None;
        return ball.lerp(Vec3::new(spot.x, BALL_RADIUS, spot.z), 1.0 - (-dt / STANCE_BALL_TIME).exp());
    }
    // A push ball's touch draws the ball onto the way you push by its contact frame, so it leaves
    // in line with you and you chase it straight, even flicked a new way out of a control. It isn't
    // pulled back: a ball already further out the way you push stays that far.
    if let Some(k) = p.kick.filter(|k| matches!(k.kind, Kick::PushBall | Kick::DribblePushBall)) {
        let way = if k.steer == Vec3::ZERO { forward(p.facing) } else { k.steer };
        // It moves on with you meanwhile, so it doesn't stall in front of you.
        let ball = ball + Vec3::new(p.vel.x, 0.0, p.vel.z) * dt;
        let spot = root + way * (ball - root).dot(way).max(PUSH_TOUCH_REACH);
        let left = (contact_time(&k) - k.elapsed).max(dt);
        return ball.lerp(Vec3::new(spot.x, BALL_RADIUS, spot.z), (dt / left).min(1.0));
    }
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    let (ahead, every, own) = if p.sprinting { (TOUCH_AHEAD_SPRINT, TOUCH_EVERY_SPRINT, BALL_SPRINT_SPEED) } else { (TOUCH_AHEAD_RUN, TOUCH_EVERY_RUN, BALL_RUN_SPEED) };
    // Touches come with the steps: faster when you run faster (as the cycle's play rate), up to
    // 20% more often on sharp turns, each a little off the beat.
    let (last, now_way) = (p.carry.map_or(p.vel, |c| c.vel).with_y(0.0), p.vel.with_y(0.0));
    let rate = if last.length() > 0.1 && now_way.length() > 0.1 { last.angle_between(now_way).to_degrees() / dt.max(1e-4) } else { 0.0 };
    let turning = 0.2 * ((rate - TOUCH_TURN_RATES.0) / (TOUCH_TURN_RATES.1 - TOUCH_TURN_RATES.0)).clamp(0.0, 1.0);
    let gap = |n: u32| {
        let jitter = ((n as f32 * 12.9898).sin() * 43758.545).fract().abs() * 0.06 - 0.01;
        every / (speed / own).clamp(0.7, 1.5) / (1.0 + turning) + jitter
    };
    // Touches go the way you move (a dribble's way while it lasts), a little ahead of you.
    let way = p.dribble.map_or_else(|| Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or(forward(p.facing)), |d| d.dir);
    let predict = |at: f32| {
        // Your pace, at most your run with the ball (a dribble sets its own): faster, the touch
        // would send the ball far ahead and you'd run back onto it.
        let pace = Vec3::new(p.vel.x, 0.0, p.vel.z);
        let pace = if p.dribble.is_some() { pace } else { pace.clamp_length_max(own) };
        let spot = root + pace * (at - now).max(0.0) + way * ahead;
        Vec3::new(spot.x, BALL_RADIUS, spot.z)
    };
    let mut c = p.carry.unwrap_or_else(|| {
        let next = now + gap(0);
        Carry { from: ball, start: now, next, target: predict(next), touches: 0, move_start: -1.0, vel: p.vel }
    });
    // A dribble or feint owns the touches: the ball goes from where it is to your foot by the move's
    // contact frame, then is pushed its way until the move ends.
    let dribble_end = p.dribble.map(|d| now - d.t + DRIBBLE_SPEED_TIME);
    if let Some(d) = p.dribble
        && (now - d.t - c.move_start).abs() > 1e-3
    {
        c.move_start = now - d.t;
        c.from = ball;
        c.start = now;
        c.next = (c.move_start + dribble_contact(&d)).max(now + 1.0 / 60.0);
        c.target = predict(c.next);
    }
    if now >= c.next {
        c.touches += 1;
        c.from = c.target;
        c.start = now;
        c.next = now + dribble_end.map_or_else(|| gap(c.touches), |end| (end - now).max(0.25));
        c.target = predict(c.next);
    }
    let alpha = ((now - c.start) / (c.next - c.start)).clamp(0.0, 1.0);
    c.target += (predict(c.next) - c.target) * (curve(&TOUCH_SPRING, alpha) * dt).min(1.0);
    c.vel = p.vel;
    p.carry = Some(c);
    let eased = 1.0 - (1.0 - alpha).powf(TOUCH_LAUNCH);
    let pos = c.from.lerp(c.target, eased);
    // Keep it within reach to the side of the way you're going.
    let fwd = way;
    let right = fwd.cross(Vec3::Y);
    let rel = pos - root;
    let (front, side) = (rel.dot(fwd), rel.dot(right));
    let limit = curve(&TOUCH_SIDE_LIMIT, front.max(0.0));
    let pos = pos - right * (side - side.clamp(-limit, limit));
    Vec3::new(pos.x, BALL_RADIUS, pos.z)
}

#[derive(Debug, PartialEq)]
enum EffortTouch {
    /// Run on with the ball under you.
    Ride,
    /// Tap it on the way the stick points.
    Tap,
    /// It was just tapped: let it go.
    Leave,
}

/// What an extra-effort touch does to a ball within reach (extra-effort.md, the control tap). A
/// ball you catch running the way you push gives you a grace on it: you ride it until the stick
/// turns away (the chop) or EFFORT_GRACE runs out. Any other ball is tapped on at once.
fn effort_touch(ball_vel: Vec3, dir: Vec3, ridden: f32) -> EffortTouch {
    let flat = Vec3::new(ball_vel.x, 0.0, ball_vel.z);
    if flat.dot(dir) >= EXTRA_EFFORT_KNOCK_SPEED * 0.9 {
        return EffortTouch::Leave;
    }
    // Riding, the ball moves with you, so this compares the stick with your own way.
    let straight = flat.length() > 1.0 && flat.angle_between(dir).to_degrees() < EFFORT_CHOP_ANGLE;
    if straight && ridden < EFFORT_GRACE { EffortTouch::Ride } else { EffortTouch::Tap }
}

pub fn kicking(
    mut player: Single<&mut Player>,
    mut ball: Single<&mut Ball>,
    ball_tf: Single<&Transform, With<Ball>>,
    camera: Single<&Transform, (With<Camera3d>, Without<Ball>)>,
    rig: Single<&CameraRig>,
    mut actions: ResMut<Actions>,
    mut axes: ResMut<Axes>,
    bindings: Res<Bindings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let p = &mut **player;

    // A pass press early in a shot's charge is a fake shot (Rematch binds both to the same button):
    // the leg swings low over the ball and you crouch over it, no kicks until you can act again.
    if let Some((t, lock)) = p.fake {
        p.fake = (t + dt < lock).then_some((t + dt, lock));
        return;
    }
    if p.charging_shot() && p.kick.is_some_and(|k| k.elapsed <= FAKE_SHOT_WINDOW) && actions.just_pressed(Action::FakeShot) {
        let moving = Vec3::new(p.vel.x, 0.0, p.vel.z).length() > 1.0;
        p.fake = Some((0.0, FAKE_SHOT_LOCK[2 * p.exhausted as usize + moving as usize]));
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
    // its action frame, but from its effect on the ball a kick's press counts (players confirm):
    // it waits for the action frame, and a held one charges meanwhile, like a pre-charge.
    if p.receiving.is_some() {
        let kicks = [Action::LowPowerPass, Action::LowPowerLob, Action::PushBall, Action::Shoot, Action::Pass, Action::Lob];
        if let Some(a) = kicks.into_iter().find(|a| actions.just_pressed(*a)) {
            p.queued = Some((a, 0.0));
        } else if p.queued.is_none() {
            p.queued = kicks.into_iter().find(|a| actions.pending(*a)).map(|a| (a, 0.0));
        }
        if let Some((a, held)) = p.queued.as_mut()
            && actions.pressed(*a)
        {
            *held += dt;
        }
    }
    if p.has_ball && p.kick.is_none() && p.receiving.is_none() {
        let charged = [(Action::Shoot, Kick::Shot), (Action::Pass, Kick::Pass), (Action::Lob, Kick::Lob)];
        // In lock move the push ball button dribbles instead (player_move); inside a dribble,
        // released from lock move, it's the dribble push ball.
        let push = if p.dribble.is_some() || p.since_dribble < DRIBBLE_PUSH_GRACE { Kick::DribblePushBall } else { Kick::PushBall };
        let instant = [(Action::LowPowerPass, Kick::ShortPass), (Action::LowPowerLob, Kick::ShortLob), (Action::PushBall, push)];
        let pressed = instant
            .into_iter()
            .chain(charged)
            .filter(|(a, _)| *a != Action::PushBall || !p.locked)
            .filter(|(a, _)| dribble_open || *a != Action::PushBall)
            .find(|(a, _)| actions.pending(*a) || (*a == Action::PushBall && buffered_push) || p.queued.is_some_and(|(q, _)| q == *a));
        // Extra effort can't keep the ball: starting it touches the ball on ahead.
        let pressed = pressed.or((p.extra_active && dribble_open).then_some((Action::ExtraEffort, Kick::PushBall)));
        if let Some((action, kind)) = pressed {
            actions.consume(action);
            let pre = p.queued.filter(|(q, _)| *q == action).map_or(0.0, |(_, held)| held);
            axes.reset_curve();
            let from_dribble = p.dribble.is_some() || p.since_dribble < DRIBBLE_PUSH_GRACE;
            // Shots and push balls take over from the dribble. A pass or lob keeps the dribble's
            // permissions, but its own drag moves you from now on (the dribble stops carrying you).
            if kind.is_pass() {
                if let Some(d) = p.dribble.as_mut() {
                    d.t = d.t.max(d.free());
                }
            } else {
                p.dribble = None;
                p.dash = None;
            }
            p.push_buffered = false;
            let released = kind.charge_action().is_none();
            let steer = if axes.movement.length() < PUSH_BALL_DEAD_ZONE {
                Vec3::ZERO
            } else {
                camera_relative(axes.movement.normalize(), rig.yaw).normalize()
            };
            p.run = None;
            p.kick = Some(KickState { kind, elapsed: 0.0, held: pre, pre, released, aim: Vec2::new(rig.yaw, rig.pitch), eye: camera.translation, entry: p.vel, from_dribble, steer, sprint: p.sprinting });
        }
    }
    if p.receiving.is_none() {
        p.queued = None;
    }
    // For 0.1 s after a push ball or pass starts, the longer chord can still take over, so a
    // slightly staggered chord gives the kick you meant (the abilities' "buffer" layers).
    if let Some(k) = p.kick.as_mut().filter(|k| k.elapsed < CHORD_WINDOW) {
        let takeovers: &[(Action, Kick)] = match k.kind {
            Kick::PushBall => &[(Action::LowPowerLob, Kick::ShortLob), (Action::LowPowerPass, Kick::ShortPass)],
            Kick::Pass => &[(Action::LowPowerLob, Kick::ShortLob), (Action::LowPowerPass, Kick::ShortPass), (Action::Lob, Kick::Lob)],
            Kick::Lob | Kick::ShortPass => &[(Action::LowPowerLob, Kick::ShortLob)],
            _ => &[],
        };
        if let Some(&(action, kind)) = takeovers.iter().find(|(a, _)| actions.pending(*a)) {
            actions.consume(action);
            k.kind = kind;
            k.released = kind.charge_action().is_none();
        }
    }

    let Some(k) = &mut p.kick else { return };
    k.elapsed += dt;
    // Extra effort's auto tap goes where the stick points at its touch, not at the press: you press
    // extra effort and the new way together, often a frame or two apart.
    if k.kind == Kick::PushBall && p.extra_active && axes.movement.length() >= PUSH_BALL_DEAD_ZONE {
        k.steer = camera_relative(axes.movement.normalize(), rig.yaw).normalize();
    }
    // Extra effort's auto tap turns you to where you push by its touch (as the game's 135/180 deg
    // weakening implies), so starting it any way, even out of a dribble, takes the ball with you.
    // A dribble push ball turns you to the push before its touch, over its first 9 frames.
    if (k.kind == Kick::DribblePushBall || (k.kind == Kick::PushBall && p.extra_active)) && k.steer != Vec3::ZERO {
        let left = (contact_time(k) - (k.elapsed - dt)).max(dt);
        p.facing += angle_diff(yaw_of(k.steer), p.facing) * (dt / left).min(1.0);
    }
    if !k.released {
        if k.kind.charge_action().is_some_and(|a| actions.pressed(a)) {
            k.held = (k.pre + k.elapsed).min(MAX_HOLD);
        }
        // Held to a full gauge, the kick goes off by itself (players confirm: the ball leaves as
        // the gauge fills).
        k.released = charge(k.kind, k.held) >= 1.0 || !k.kind.charge_action().is_some_and(|a| actions.pressed(a));
    }
    // The ball leaves on release, but never before the kick's contact frame (ShootDataTable):
    // a shot under 29.41 m/s uses the quicker low-power kick (20 frames, not 32).
    let contact = contact_time(k);
    let release = (k.held - k.pre).max(0.0);
    if !k.released || k.elapsed < contact.max(release + KICK_STRIKE_MIN) {
        return;
    }
    let k = p.kick.take().expect("checked above");
    if !p.has_ball {
        return;
    }

    let t = charge(k.kind, k.held);
    // Spin scales with the raw seconds held, not the charge (shot-aim.md).
    let ratio = (k.held / MAX_HOLD).min(1.0);
    let side = axes.curve.x;
    // Push balls go the way you steer, or straight on with no direction. The normal one loses
    // speed going sideways or back and gains it when sprinting; the dribble one only when sprinting.
    let push_dir = if k.steer == Vec3::ZERO { forward(p.facing) } else { k.steer };
    let push_angle = forward(p.facing).angle_between(push_dir).to_degrees();
    // How far the push turns you: against your facing at the touch, and against your run at the
    // press, so turning toward the push during its wind-up still counts as pushing into a turn.
    let run_way = Vec3::new(k.entry.x, 0.0, k.entry.z);
    let turn_angle = if run_way.length() > 1.0 { push_angle.max(run_way.angle_between(push_dir).to_degrees()) } else { push_angle };
    let push_factor = match turn_angle {
        a if a <= 60.0 => 1.0,
        a if a <= 95.0 => PUSH_BALL_SIDE_FACTOR,
        _ => PUSH_BALL_BACK_FACTOR,
    };
    let sprint = |factor: f32| if k.sprint { factor } else { 1.0 };
    // A locked shot aims with the camera at the press. On a gamepad without the camera lock it aims
    // with the camera at the strike, which can't turn from the release on (camera-lock.md).
    let (eye, aim) = if bindings.shot_camera_lock(actions.device) { (k.eye, k.aim) } else { (camera.translation, Vec2::new(rig.yaw, rig.pitch)) };
    let (speed, elevation, friction, no_control) = match k.kind {
        // Height from the camera's pitch; a dip launches a little higher to make up for it.
        Kick::Shot => (
            shot_speed(&k),
            curve(&ELEVATION_BY_PITCH, aim.y) + SHOT_DIP_PITCH_CORRECTION * (-axes.curve.y).max(0.0),
            SHOT_FRICTION,
            NO_RECONTROL_TIME,
        ),
        Kick::Pass => (curve(&PASS_FORCE, t), curve(&PASS_LIFT_BY_HOLD, t), PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortPass => (SHORT_PASS_SPEED, 0.0, SHORT_PASS_FRICTION, NO_RECONTROL_TIME),
        Kick::Lob => (curve(&LOB_FORCE, t), curve(&LOB_ANGLE_BY_HOLD, t), SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::ShortLob => (SHORT_LOB_SPEED, SHORT_LOB_ANGLE, SHOT_FRICTION, NO_RECONTROL_TIME),
        Kick::PushBall if p.extra_active => (
            // Out of a dribble it isn't weakened by the turn, so the ball goes on ahead of you the new
            // way (Kev).
            EXTRA_EFFORT_TAP_SPEED * if k.from_dribble { 1.0 } else { auto_tap_factor(k.entry, push_dir, ball_tf.translation.y) },
            0.0,
            EXTRA_EFFORT_FRICTION,
            PUSH_BALL_NO_CONTROL,
        ),
        // Tired push balls ignore sprinting and pop up a little.
        Kick::PushBall if p.exhausted => (EXHAUSTED_PUSH_BALL_SPEED * push_factor, EXHAUSTED_PUSH_BALL_LIFT, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
        Kick::DribblePushBall if p.exhausted => (EXHAUSTED_PUSH_BALL_SPEED, EXHAUSTED_PUSH_BALL_LIFT, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
        // Sprinting pushes the ball a little harder and it runs on longer.
        Kick::PushBall => (
            PUSH_BALL_SPEED * push_factor * sprint(PUSH_BALL_SPRINT_FACTOR),
            0.0,
            if k.sprint { SPRINT_PUSH_BALL_FRICTION } else { PUSH_BALL_FRICTION },
            PUSH_BALL_NO_CONTROL,
        ),
        Kick::DribblePushBall => (DRIBBLE_PUSH_BALL_SPEED * sprint(DRIBBLE_PUSH_BALL_SPRINT_FACTOR), 0.0, PUSH_BALL_FRICTION, PUSH_BALL_NO_CONTROL),
    };
    let yaw = match k.kind {
        // The extra effort auto tap goes the way your body faces, not your input.
        Kick::PushBall if p.extra_active => p.facing,
        Kick::PushBall | Kick::DribblePushBall => yaw_of(push_dir),
        // Toward the crosshair's point on the walls (`aim` above), turned a little against the
        // curve so the ball bends back toward it.
        Kick::Shot => crosshair_yaw(eye, aim.x, ball_tf.translation) + (side * AIM_NUDGE).to_radians(),
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
    ball.freeze = k.kind.freeze_frames(speed) / 60.0;
    p.freeze = ball.freeze;
    if k.kind == Kick::Shot {
        let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        p.run = Some(Run { t: 0.0, length: SHOT_AFTER_TOUCH, free_at: 0.0, dir: Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero(), speeds: &[(0.0, 1.0), (1.0, 1.0)], tables: None, scale: speed, exit: 180.0, soft: true, face: None });
    }
    // After the touch a pass or lob keeps dragging you until its move interrupt. Either drag only
    // lasts while you leave the stick alone: any stick hands you back at once (Kev: held on, they
    // felt stuck).
    if let Some(drag) = pass_drag(&k) {
        p.run = Some(Run { t: 0.0, length: PASS_AFTER_TOUCH, free_at: 0.0, dir: Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or(drag.normalize()), speeds: &[(0.0, 1.0), (1.0, 1.0)], tables: None, scale: drag.length(), exit: 180.0, soft: true, face: None });
    }
    if matches!(k.kind, Kick::PushBall | Kick::DribblePushBall) {
        p.own_push = 0.0;
        p.push_way = flat;
        p.push_sprinted = k.sprint;
    }
    // After a push ball you chase it on the move's own speed (extra effort keeps its running).
    if matches!(k.kind, Kick::PushBall | Kick::DribblePushBall) && !p.extra_active {
        let cost = if k.kind == Kick::PushBall { PUSH_BALL_COST } else { DRIBBLE_PUSH_BALL_COST };
        p.stamina = (p.stamina - cost).max(0.0);
        p.regen_wait = PUSH_BALL_REGEN_DELAY;
        // A normal push ball turns you smoothly to the push over the 8 frames after its touch (Kev:
        // the clips' later turns sideways and back looked wrong here).
        let by = angle_diff(yaw, p.facing);
        let by = if by.abs() > std::f32::consts::PI - 0.05 { by.abs() * swing_side(p.facing, forward(yaw)) } else { by };
        let face = (k.kind == Kick::PushBall).then_some((p.facing, by, 0.0, 8.0 / 60.0));
        if k.kind == Kick::DribblePushBall {
            p.facing = yaw;
        }
        let state = if p.exhausted { 2 } else if p.sprinting { 1 } else { 0 };
        let (tables, length) = if k.kind == Kick::DribblePushBall {
            (DRIBBLE_PUSH_BALL_RUN, DRIBBLE_PUSH_BALL_LENGTH[Heading::of(push_angle, 60.0, 135.0) as usize])
        } else {
            let i = Heading::of(push_angle, 60.0, 150.0) as usize;
            ([PUSH_BALL_RUN[i], SPRINT_PUSH_BALL_RUN[i], EXHAUSTED_PUSH_BALL_RUN[i]], PUSH_BALL_LENGTH[i])
        };
        let speeds = tables[state];
        // The chase runs to the animation's blend-out, or ends earlier on a stick exit.
        p.run = Some(Run { t: 0.0, length: length - BLEND_OUT - PUSH_BALL_WINDUP, free_at: PUSH_BALL_FREE - PUSH_BALL_WINDUP, dir: flat, speeds, tables: Some(tables), scale: 1.0, exit: RUN_TURN_LIMIT, soft: false, face });
    }
}

/// A pass or lob's drag: your velocity at the press, or out of a dribble your run speed with the
/// ball that way; none standing still (PASS_DRAG_MIN_SPEED).
fn pass_drag(k: &KickState) -> Option<Vec3> {
    let entry = Vec3::new(k.entry.x, 0.0, k.entry.z);
    (k.kind.is_pass() && entry.length() > PASS_DRAG_MIN_SPEED).then(|| if k.from_dribble { entry.normalize() * BALL_RUN_SPEED } else { entry })
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

/// How far a kick has charged, 0..1: the seconds held against its build-up, less a frame, so the
/// gauge is full just as the foot meets the ball (players confirm; mechanics.md §4 has the network
/// margins the code also takes off, which would fill it 0.15 s earlier).
pub fn charge(kind: Kick, held: f32) -> f32 {
    (held / (kind.buildup() - 1.0 / 60.0)).clamp(0.0, 1.0)
}

/// When the foot meets the ball, from the press (ShootDataTable): a shot under 29.41 m/s is the
/// quicker low-power kick (20 frames, not 32). The ball leaves then or at the release, if later.
pub fn contact_time(k: &KickState) -> f32 {
    match k.kind {
        Kick::Shot if shot_speed(k) < SHOT_HIGH_POWER_SPEED => SHOT_LOW_POWER_CONTACT - TAP_NETWORK_MARGIN,
        Kick::Pass | Kick::Lob => PASS_CONTACT - TAP_NETWORK_MARGIN,
        _ => k.kind.buildup(),
    }
}

/// A shot's ball speed: the force curve by seconds held (at least the build-up), plus your run-up
/// speed's bonus, which only counts for stronger shots.
fn shot_speed(k: &KickState) -> f32 {
    let force = curve(&SHOT_FORCE, charge(Kick::Shot, k.held));
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
    fn kicks_pressed_during_a_control_wait_for_it_and_precharge() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Bindings>().init_resource::<Time>().add_systems(Update, kicking);
        app.world_mut().spawn(Player { has_ball: true, receiving: Some(0.0), ..default() });
        app.world_mut().spawn((Transform::default(), Ball::default()));
        app.world_mut().spawn((Camera3d::default(), Transform::default()));
        app.world_mut().spawn(CameraRig::default());
        fn kick(app: &mut App) -> Option<(Kick, f32)> {
            app.world_mut().query::<&Player>().single(app.world()).unwrap().kick.map(|k| (k.kind, k.held))
        }
        fn controlled(app: &mut App) {
            app.world_mut().query::<&mut Player>().single_mut(app.world_mut()).unwrap().receiving = None;
        }
        // A shot held through the control starts at its action frame, already charging.
        frames(&mut app, &[Action::Shoot], 10, 1.0 / 60.0);
        assert!(kick(&mut app).is_none());
        controlled(&mut app);
        frames(&mut app, &[Action::Shoot], 1, 1.0 / 60.0);
        assert!(kick(&mut app).is_some_and(|(kind, held)| kind == Kick::Shot && held > 0.15), "pre-charged shot");
        // A push ball tapped during a control isn't lost.
        let mut p = app.world_mut().query::<&mut Player>().single_mut(app.world_mut()).unwrap();
        (p.kick, p.receiving) = (None, Some(0.0));
        frames(&mut app, &[], 1, 1.0 / 60.0);
        frames(&mut app, &[Action::PushBall], 2, 1.0 / 60.0);
        frames(&mut app, &[], 5, 1.0 / 60.0);
        controlled(&mut app);
        frames(&mut app, &[], 1, 1.0 / 60.0);
        assert!(kick(&mut app).is_some_and(|(kind, _)| kind == Kick::PushBall), "queued push ball");
    }

    #[test]
    fn a_kicks_freeze_holds_the_pose_not_your_run() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        let id = app.world_mut().spawn((Transform::default(), Player { freeze: 3.0 / 60.0, vel: Vec3::NEG_Z * 6.5, ..default() })).id();
        app.world_mut().spawn(CameraRig::default());
        frames(&mut app, &[], 3, 1.0 / 60.0);
        let z = app.world().get::<Transform>(id).unwrap().translation.z;
        assert!((z + 6.5 * 3.0 / 60.0).abs() < 1e-4, "slid {z}");
    }

    #[test]
    fn a_shot_fakes_only_early_and_the_fake_holds_off_kicks() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Bindings>().init_resource::<Time>().add_systems(Update, kicking);
        app.world_mut().spawn(Player { has_ball: true, ..default() });
        app.world_mut().spawn((Transform::default(), Ball::default()));
        app.world_mut().spawn((Camera3d::default(), Transform::default()));
        app.world_mut().spawn(CameraRig::default());
        fn state(app: &mut App) -> (Option<Kick>, bool) {
            let p = app.world_mut().query::<&Player>().single(app.world()).unwrap();
            (p.kick.map(|k| k.kind), p.fake.is_some())
        }
        // 0.2 s into the charge, pass fakes the shot; no kick starts in the fake.
        frames(&mut app, &[Action::Shoot], 12, 1.0 / 60.0);
        frames(&mut app, &[Action::Shoot, Action::FakeShot], 1, 1.0 / 60.0);
        assert_eq!(state(&mut app), (None, true));
        frames(&mut app, &[], 2, 1.0 / 60.0);
        frames(&mut app, &[Action::Shoot], 2, 1.0 / 60.0);
        assert_eq!(state(&mut app).0, None, "no kick in the fake");
        // Past the window (before the full charge fires it), the shot goes on.
        frames(&mut app, &[], 40, 1.0 / 60.0);
        frames(&mut app, &[Action::Shoot], 28, 1.0 / 60.0);
        frames(&mut app, &[Action::Shoot, Action::FakeShot], 1, 1.0 / 60.0);
        assert_eq!(state(&mut app), (Some(Kick::Shot), false));
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
        // Nothing chains after the feint, even late in it.
        frames(&mut app, lock, 6, 0.04);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| d.feint && d.t > 0.2));
        let length = dribble(&mut app).unwrap().length();
        frames(&mut app, lock, ((length - 0.1 - 0.26) / 0.016) as u32, 0.016);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| d.feint && d.t > length - 0.15), "no dribble 1 out of the feint's end");
    }

    #[test]
    fn letting_go_of_lock_move_makes_the_next_press_dribble_1() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().spawn((Transform::default(), Player { has_ball: true, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        let (lock, press) = ([Action::Lock].as_slice(), [Action::Lock, Action::PushBall].as_slice());
        frames(&mut app, lock, 1, 0.016);
        frames(&mut app, press, 1, 0.016);
        // Let go (past the stance's 0.15 s minimum), lock again, press: too soon for a new dribble,
        // and no feint either.
        frames(&mut app, &[], 12, 0.016);
        frames(&mut app, lock, 2, 0.016);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| !d.feint && d.t > 0.2), "the first dribble goes on");
        // From DRIBBLE_REPEAT, the press is dribble 1 again, not the feint.
        let t = dribble(&mut app).unwrap().t;
        frames(&mut app, lock, ((DRIBBLE_REPEAT + 0.03 - t) / 0.016) as u32, 0.016);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| !d.feint && d.t < 0.05), "dribble 1 again");
    }

    #[test]
    fn late_in_a_dribble_the_same_press_is_a_new_dribble() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().spawn((Transform::default(), Player { has_ball: true, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        let (lock, press) = ([Action::Lock].as_slice(), [Action::Lock, Action::PushBall].as_slice());
        frames(&mut app, lock, 1, 0.016);
        frames(&mut app, press, 1, 0.016);
        let length = dribble(&mut app).unwrap().length();
        // Late in it (DRIBBLE_CHAIN_LATE before the end), a press is dribble 1.
        frames(&mut app, lock, ((length - DRIBBLE_CHAIN_LATE + 0.03) / 0.016) as u32, 0.016);
        frames(&mut app, press, 1, 0.016);
        assert!(dribble(&mut app).is_some_and(|d| !d.feint && d.t < 0.05), "a new dribble 1");
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
            frames(&mut app, &[], 15, 0.016); // let go, and the next double tap
        }
        assert_eq!(cuts, 3, "a full gauge gives three cuts, gauge left {}", gauge(&mut app));
    }

    #[test]
    fn side_dash_gathers_then_lunges_about_2_3_m() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        let dash = DashState { t: 0.0, second: false, dir: Vec3::X, facing: 0.0, heading: Heading::Side };
        app.world_mut().spawn((Transform::default(), Player { dash: Some(dash), locked: true, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        let dt = 1.0 / 60.0;
        let (mut t, mut x, mut fastest) = (0.0, 0.0, (0.0, 0.0));
        let mut gathered = 0.0;
        while app.world_mut().query::<&Player>().single(app.world()).unwrap().dash.is_some() {
            frames(&mut app, &[Action::Lock], 1, dt);
            let v = app.world_mut().query::<&Player>().single(app.world()).unwrap().vel.x;
            t += dt;
            x += v * dt;
            if t <= 0.1 + 1e-4 {
                gathered = x;
            }
            if v > fastest.1 {
                fastest = (t, v);
            }
        }
        // Kev's practice recording: barely moving for the first 0.1 s, fastest at +180-330 ms,
        // about 2.3 m to the side by the time you are back in the stance (0.40-0.50 s).
        assert!(gathered < 0.35, "{gathered} m in the gather");
        assert!((0.17..0.31).contains(&fastest.0), "fastest at {} s", fastest.0);
        assert!((0.4..0.5).contains(&t), "free at {t} s");
        assert!((2.1..2.6).contains(&x), "{x} m");
    }

    #[test]
    fn extra_effort_gauge_runs_as_in_kevs_recording() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().spawn((Transform::default(), Player::default()));
        app.world_mut().spawn(CameraRig::default());
        let state = |app: &mut App| {
            let p = app.world_mut().query::<&Player>().single(app.world()).unwrap();
            (p.extra_effort, p.extra_active)
        };
        let dt = 1.0 / 60.0;
        // Held from full: about a quarter goes at once, then it runs dry about 1.55 s in.
        frames(&mut app, &[Action::ExtraEffort], 1, dt);
        let (first, _) = state(&mut app);
        assert!((first - 0.74).abs() < 0.02, "after the first frame {first}");
        let mut held = dt;
        while state(&mut app).1 && held < 3.0 {
            frames(&mut app, &[Action::ExtraEffort], 1, dt);
            held += dt;
        }
        assert!((1.5..1.62).contains(&held), "a full hold lasts {held} s");
        // Refilled, a 0.1 s tap still runs about 0.2 s and costs about a third of the gauge.
        app.world_mut().query::<&mut Player>().single_mut(app.world_mut()).unwrap().extra_effort = MAX_EXTRA_EFFORT;
        frames(&mut app, &[], 1, dt);
        frames(&mut app, &[Action::ExtraEffort], 6, dt);
        let mut on = 6.0 * dt;
        while state(&mut app).1 && on < 1.0 {
            frames(&mut app, &[], 1, dt);
            on += dt;
        }
        let (left, _) = state(&mut app);
        assert!((0.18..0.24).contains(&on), "a tap runs {on} s");
        assert!((0.62..0.7).contains(&left), "a tap leaves {left}");
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
    fn carried_ball_goes_from_touch_to_touch_ahead_of_you() {
        // Running straight at 4.5 m/s: the ball leads and falls back into each touch, never behind
        // you or off to the side. A dribble then takes the ball its way at its contact frame.
        let mut p = Player { has_ball: true, vel: Vec3::NEG_Z * BALL_RUN_SPEED, ..default() };
        let (mut root, mut ball) = (Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, BALL_RADIUS, -0.7));
        let (mut nearest, mut furthest) = (f32::MAX, 0.0f32);
        for i in 0..240 {
            let dt = 1.0 / 60.0;
            root += p.vel * dt;
            ball = carry_ball(&mut p, root, ball, i as f32 * dt, dt);
            let ahead = root.z - ball.z;
            if i > 60 {
                nearest = nearest.min(ahead);
                furthest = furthest.max(ahead);
            }
            assert!((ball.x - root.x).abs() < 1e-3);
        }
        assert!(nearest > 0.1 && nearest < 0.35, "closest {nearest}");
        assert!(furthest > 0.6 && furthest < 1.3, "furthest {furthest}");
        // A side dribble to the right: the ball comes to the foot by its contact frame, then goes
        // right with you.
        p.vel = Vec3::X * 6.0;
        p.dribble = Some(DribbleState { t: 0.0, feint: false, exhausted: false, heading: Heading::Side, dir: Vec3::X, facing: 0.0 });
        for i in 240..300 {
            let dt = 1.0 / 60.0;
            root += p.vel * dt;
            if let Some(d) = p.dribble.as_mut() {
                d.t += dt;
            }
            ball = carry_ball(&mut p, root, ball, i as f32 * dt, dt);
        }
        assert!(ball.x > root.x + 0.1 && (ball.z - root.z).abs() < 0.5, "ball {ball} root {root}");
    }

    #[test]
    fn extra_effort_overtaking_its_own_push_ball_knocks_it_on() {
        let mut app = App::new();
        app.init_resource::<Axes>().init_resource::<Time>().add_systems(Update, ball_control);
        app.world_mut().spawn(CameraRig::default());
        app.world_mut().spawn((Transform::default(), Player { extra_active: true, vel: Vec3::NEG_Z * EXTRA_EFFORT_SPEED, facing: 0.0, ..default() }));
        // Level with you and still marked by the push: knocked on, not run past.
        let ball = app.world_mut().spawn((Transform::from_xyz(0.0, BALL_RADIUS, -0.05), Ball { vel: Vec3::NEG_Z * 5.0, no_control: 0.3, ..default() })).id();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
        app.update();
        let b = app.world().get::<Ball>(ball).unwrap();
        assert!(b.vel.z < -5.5, "ball {:?}", b.vel);
        // Still well ahead of you, the push keeps its no-control.
        app.world_mut().get_mut::<Transform>(ball).unwrap().translation.z = -0.4;
        app.world_mut().get_mut::<Ball>(ball).unwrap().vel = Vec3::NEG_Z * 5.0;
        app.update();
        assert_eq!(app.world().get::<Ball>(ball).unwrap().vel, Vec3::NEG_Z * 5.0);
    }

    #[test]
    fn effort_chop_has_a_grace_on_the_ball() {
        let ahead = Vec3::NEG_Z;
        // Catching a slowed ball straight on, you ride it; past the grace it is tapped on.
        assert_eq!(effort_touch(ahead * 6.0, ahead, 0.0), EffortTouch::Ride);
        assert_eq!(effort_touch(ahead * 9.5, ahead, 0.1), EffortTouch::Ride);
        assert_eq!(effort_touch(ahead * 9.5, ahead, EFFORT_GRACE), EffortTouch::Tap);
        // Turning the stick while on it chops at once.
        assert_eq!(effort_touch(ahead * 9.5, Vec3::X, 0.05), EffortTouch::Tap);
        // A ball still slow or lying there is tapped on at once; a fresh tap is left alone.
        assert_eq!(effort_touch(Vec3::ZERO, ahead, 0.0), EffortTouch::Tap);
        assert_eq!(effort_touch(Vec3::X * 6.0, ahead, 0.0), EffortTouch::Tap);
        assert_eq!(effort_touch(ahead * 12.0, ahead, 0.0), EffortTouch::Leave);
    }

    #[test]
    fn control_zones() {
        let toward = Vec3::new(-5.0, 0.0, 0.0); // rolling at you
        let away = Vec3::new(5.0, 0.0, 0.0);
        let ground = |x: f32| Vec3::new(x, BALL_RADIUS, 0.0);
        // Standing: a ball rolling at you is taken within ~0.95 m of a point 0.1 m ahead.
        assert!(in_control_zone(ground(1.0), toward, 0.0, false, false));
        assert!(!in_control_zone(ground(1.3), toward, 0.0, false, false));
        // A ball rolling away is refused standing, but taken running forward onto it, once it is
        // about under you.
        assert!(!in_control_zone(ground(0.4), away, 0.0, false, false));
        assert!(in_control_zone(ground(0.4), away, 6.0, false, false));
        assert!(!in_control_zone(ground(0.9), away, 6.0, false, false));
        // Starting or turning, the moving zones are on even when slow.
        assert!(in_control_zone(ground(1.25), toward, 1.0, false, true));
        // Moving, the zone reaches further ahead; lock move catches from any side.
        assert!(in_control_zone(ground(1.25), toward, 4.0, false, false));
        assert!(in_control_zone(Vec3::new(0.0, BALL_RADIUS, 0.7), away, 0.0, true, false));
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
    fn a_dribble_hands_back_softly_against_the_stick() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::NEG_X; // stick left, the dribble went right
        let d = DribbleState { t: DRIBBLE_FREE[1] - 0.01, feint: false, exhausted: false, heading: Heading::Side, dir: Vec3::X, facing: 0.0 };
        app.world_mut().spawn((Transform::default(), Player { has_ball: true, dribble: Some(d), vel: Vec3::X * 3.0, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        // No brake and pivot: a quick pivot swings you round to the stick within 0.3 s, the body
        // with your heading.
        frames(&mut app, &[], 20, 1.0 / 60.0);
        let p = app.world_mut().query::<&Player>().single(app.world()).unwrap();
        assert!(p.turn.is_none() && p.vel.x < -1.0 && angle_diff(yaw_of(p.vel), p.facing).abs() < 0.05, "{:?} {}", p.vel, p.facing);
    }

    #[test]
    fn dash_handed_back_against_the_stick_turns_without_an_arc() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::NEG_X; // stick left all through
        let dash = DashState { t: 0.0, second: false, dir: Vec3::X, facing: 0.0, heading: Heading::Side };
        app.world_mut().spawn((Transform::default(), Player { dash: Some(dash), vel: Vec3::X * 6.0, ..default() }));
        app.world_mut().spawn(CameraRig::default());
        let mut max_side: f32 = 0.0;
        for _ in 0..60 {
            frames(&mut app, &[], 1, 1.0 / 60.0);
            max_side = max_side.max(app.world_mut().query::<&Player>().single(app.world()).unwrap().vel.z.abs());
        }
        let v = app.world_mut().query::<&Player>().single(app.world()).unwrap().vel;
        assert!(max_side < 0.01, "sideways {max_side}");
        assert!(v.x < -1.0, "going left {v:?}");
    }

    #[test]
    fn crosshair_point_is_on_the_walls() {
        // From the centre looking at the -z goal, and from the right side angled across.
        assert!(crosshair_point(Vec3::new(0.0, 3.0, 0.0), 0.0).distance(Vec3::new(0.0, 3.0, -HALF_LENGTH)) < 1e-3);
        let p = crosshair_point(Vec3::new(10.0, 3.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!((p.x + HALF_WIDTH).abs() < 1e-3 && p.z.abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn alternating_w_and_d_keeps_your_speed() {
        let mut app = App::new();
        app.init_resource::<Actions>().init_resource::<Axes>().init_resource::<Time>().add_systems(Update, player_move);
        app.world_mut().resource_mut::<Axes>().movement = Vec2::Y;
        app.world_mut().spawn((Transform::default(), Player::default()));
        app.world_mut().spawn(CameraRig::default());
        frames(&mut app, &[], 60, 1.0 / 60.0);
        // W, D, W, D every 0.1 s: the game never brakes below the pivot (85% of 5 m/s).
        let mut slowest = f32::MAX;
        for i in 0..20 {
            app.world_mut().resource_mut::<Axes>().movement = if i % 2 == 0 { Vec2::X } else { Vec2::Y };
            for _ in 0..6 {
                frames(&mut app, &[], 1, 1.0 / 60.0);
                let v = app.world_mut().query::<&Player>().single(app.world()).unwrap().vel;
                slowest = slowest.min(v.length());
            }
        }
        assert!(slowest > 4.0, "slowest {slowest} m/s");
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
