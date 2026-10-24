//! Rebindable actions. Defaults are Rematch's own (QWERTY keyboard and gamepad presets,
//! read from Data/Inputs/Mappings in the game files). Bindings are chords: every input held.

use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::tuning::{CURVE_STICK_DEAD_ZONE, FOV, MAGNUS_ACCUMULATOR_SENSITIVITY, MOUSE_AXIS_SCALE};

const SAVE_PATH: &str = "keybinds.ron";
const DOUBLE_TAP_WINDOW: f32 = 0.15; // extra effort: release, then press again within 0.15 s (QuickPressAndHold)

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum Input {
    Key(KeyCode),
    Mouse(MouseButton),
    Pad(GamepadButton),
    /// Whatever the Modifier action is bound to, so "Modifier + Pass" follows a rebind of the modifier.
    Modifier,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum Action {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    Sprint,
    ExtraEffort,
    Shoot,
    FakeShot,
    Pass,
    LowPowerPass,
    Lob,
    LowPowerLob,
    PushBall,
    LobbedPushBall,
    LookAtBall,
    StandingTackle,
    SlidingTackle,
    Lock,
    Dash,
    Dive,
    Ping,
    Modifier,
    BallCall,
}

impl Action {
    pub const ALL: [Action; 23] = [
        Action::Modifier,
        Action::BallCall,
        Action::MoveForward,
        Action::MoveBack,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Sprint,
        Action::ExtraEffort,
        Action::Shoot,
        Action::FakeShot,
        Action::Pass,
        Action::LowPowerPass,
        Action::Lob,
        Action::LowPowerLob,
        Action::PushBall,
        Action::LobbedPushBall,
        Action::LookAtBall,
        Action::StandingTackle,
        Action::SlidingTackle,
        Action::Lock,
        Action::Dash,
        Action::Dive,
        Action::Ping,
    ];

    /// Bound and saved already, but the gameplay behind it isn't in the prototype yet.
    pub fn implemented(self) -> bool {
        !matches!(self, Action::StandingTackle | Action::SlidingTackle | Action::Dive | Action::Ping | Action::LobbedPushBall)
    }

    /// Only fires when tapped twice quickly and held (Rematch's QuickPressAndHold).
    fn double_tap(self) -> bool {
        self == Action::ExtraEffort
    }

    /// Tap moves count on the press, or on the release if they weren't allowed at the press
    /// (responsiveness.md §1). Other kicks count while held.
    fn tap(self) -> bool {
        matches!(self, Action::PushBall | Action::Dash)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Keyboard,
    Gamepad,
}

#[derive(Resource, Serialize, Deserialize, Clone, PartialEq)]
pub struct Bindings {
    pub keyboard: BTreeMap<Action, Vec<Input>>,
    pub gamepad: BTreeMap<Action, Vec<Input>>,
    #[serde(default)]
    pub pass_aim: PassAim,
    /// Rematch's "Magnus sensitivity" option: scales how much mouse movement curves a shot.
    #[serde(default = "one")]
    pub magnus_sensitivity: f32,
    /// Rematch's field of view option (degrees); the per-state FOVs shift with it.
    #[serde(default = "default_fov")]
    pub fov: f32,
}

fn one() -> f32 {
    1.0
}

fn default_fov() -> f32 {
    FOV
}

/// How keyboard-and-mouse players aim passes and lobs (Rematch's ETapAimingOptions).
#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Debug)]
pub enum PassAim {
    /// The pass goes the way your WASD keys point, relative to the camera.
    #[default]
    DirectionalAiming,
    /// Dragging the mouse while charging steers a virtual stick; the camera stays put.
    JoystickEmulation,
    /// The pass goes where the crosshair aims.
    CrosshairOnly,
}

impl PassAim {
    pub fn next(self) -> Self {
        match self {
            PassAim::DirectionalAiming => PassAim::JoystickEmulation,
            PassAim::JoystickEmulation => PassAim::CrosshairOnly,
            PassAim::CrosshairOnly => PassAim::DirectionalAiming,
        }
    }
}

impl Default for Bindings {
    fn default() -> Self {
        use Action::*;
        use GamepadButton as P;
        use Input::{Key as K, Mouse as M, Pad};
        let keyboard = [
            (MoveForward, vec![K(KeyCode::KeyW)]),
            (MoveBack, vec![K(KeyCode::KeyS)]),
            (MoveLeft, vec![K(KeyCode::KeyA)]),
            (MoveRight, vec![K(KeyCode::KeyD)]),
            (Sprint, vec![K(KeyCode::ShiftLeft)]),
            (ExtraEffort, vec![K(KeyCode::ShiftLeft)]),
            (Shoot, vec![M(MouseButton::Left)]),
            (FakeShot, vec![M(MouseButton::Right)]),
            (Pass, vec![M(MouseButton::Right)]),
            (LowPowerPass, vec![M(MouseButton::Right), K(KeyCode::Space)]),
            (Lob, vec![K(KeyCode::KeyF)]),
            (LowPowerLob, vec![K(KeyCode::KeyX)]),
            (PushBall, vec![K(KeyCode::Space)]),
            (LobbedPushBall, vec![K(KeyCode::KeyC)]),
            (LookAtBall, vec![K(KeyCode::KeyR)]),
            (StandingTackle, vec![K(KeyCode::KeyE)]),
            (SlidingTackle, vec![K(KeyCode::ShiftLeft), K(KeyCode::KeyE)]),
            (Lock, vec![K(KeyCode::ControlLeft)]),
            (Dash, vec![K(KeyCode::ControlLeft), K(KeyCode::KeyC)]),
            (Dive, vec![K(KeyCode::KeyQ)]),
            (Ping, vec![M(MouseButton::Middle)]),
            (BallCall, vec![M(MouseButton::Middle)]), // freeplay "trigger cannon ball"
        ];
        // Movement and camera stay on the sticks; the rest follows Rematch's gamepad preset.
        let gamepad = [
            (Sprint, vec![Pad(P::LeftTrigger)]),
            (ExtraEffort, vec![Pad(P::LeftTrigger)]),
            (Shoot, vec![Pad(P::RightTrigger2)]),
            (FakeShot, vec![Pad(P::East)]),
            (Pass, vec![Pad(P::West)]),
            (LowPowerPass, vec![Pad(P::West), Pad(P::South)]),
            (Lob, vec![Pad(P::RightTrigger), Pad(P::West)]),
            (LowPowerLob, vec![Pad(P::West), Pad(P::South), Pad(P::RightTrigger)]),
            (PushBall, vec![Pad(P::South)]),
            (LobbedPushBall, vec![Pad(P::RightTrigger), Pad(P::South)]),
            (StandingTackle, vec![Pad(P::East)]),
            (SlidingTackle, vec![Pad(P::LeftTrigger), Pad(P::East)]),
            (Lock, vec![Pad(P::LeftTrigger2)]),
            (Dash, vec![Pad(P::LeftTrigger2), Pad(P::South)]),
            (Dive, vec![Pad(P::North)]),
            (Ping, vec![Pad(P::RightThumb)]),
            (BallCall, vec![Pad(P::RightThumb)]),
        ];
        Self {
            keyboard: keyboard.into_iter().collect(),
            gamepad: gamepad.into_iter().collect(),
            pass_aim: PassAim::default(),
            magnus_sensitivity: one(),
            fov: default_fov(),
        }
    }
}

impl Bindings {
    /// Rematch's "custom modifier" keyboard preset: side mouse buttons, with C held as the modifier
    /// (Modifier + Pass = lob). Gamepad stays on the default preset.
    pub fn rematch_modifier_preset() -> Self {
        use Action::*;
        use Input::{Key as K, Modifier as Mod, Mouse as M};
        let (thumb, thumb2) = (M(MouseButton::Back), M(MouseButton::Forward));
        let keyboard = [
            (MoveForward, vec![K(KeyCode::KeyW)]),
            (MoveBack, vec![K(KeyCode::KeyS)]),
            (MoveLeft, vec![K(KeyCode::KeyA)]),
            (MoveRight, vec![K(KeyCode::KeyD)]),
            (Sprint, vec![K(KeyCode::ShiftLeft)]),
            (ExtraEffort, vec![K(KeyCode::ShiftLeft)]),
            (Shoot, vec![M(MouseButton::Left)]),
            (FakeShot, vec![M(MouseButton::Right)]),
            (Pass, vec![M(MouseButton::Right)]),
            (LowPowerPass, vec![M(MouseButton::Right), thumb]),
            (Lob, vec![Mod, M(MouseButton::Right)]),
            (LowPowerLob, vec![M(MouseButton::Right), thumb, Mod]),
            (PushBall, vec![thumb]),
            (LobbedPushBall, vec![Mod, thumb]),
            (LookAtBall, vec![K(KeyCode::KeyF)]),
            // Rematch puts both tackles on the same button and picks one from context.
            (StandingTackle, vec![thumb2]),
            (SlidingTackle, vec![thumb2]),
            (Lock, vec![K(KeyCode::ControlLeft)]),
            (Dash, vec![K(KeyCode::ControlLeft), thumb]),
            (Dive, vec![K(KeyCode::Space)]),
            (Ping, vec![M(MouseButton::Middle)]),
            (Modifier, vec![K(KeyCode::KeyC)]),
            (BallCall, vec![M(MouseButton::Middle)]),
        ];
        Self { keyboard: keyboard.into_iter().collect(), ..Self::default() }
    }

    pub fn load() -> Self {
        std::fs::read_to_string(SAVE_PATH).ok().and_then(|s| ron::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(s) = ron::ser::to_string_pretty(self, default()) {
            let _ = std::fs::write(SAVE_PATH, s);
        }
    }

    pub fn get_mut(&mut self, device: Device) -> &mut BTreeMap<Action, Vec<Input>> {
        match device {
            Device::Keyboard => &mut self.keyboard,
            Device::Gamepad => &mut self.gamepad,
        }
    }
}

/// This frame's action state, after chord resolution.
#[derive(Resource, Default)]
pub struct Actions {
    now: HashSet<Action>,
    prev: HashSet<Action>,
    /// A shorter chord loses to a longer one that contains it (RMB vs RMB + Space).
    raw_prev: HashSet<Action>,
    released_at: HashMap<Action, f32>,
    doubled: HashSet<Action>,
    /// Actions with an input of their chord pressed this frame. Only these count as just pressed,
    /// so letting go of a longer chord (LT + A) doesn't fire the shorter one it hid (A).
    fresh: HashSet<Action>,
    /// Presses not used yet (see `pending`).
    pending: HashSet<Action>,
}

impl Actions {
    pub fn pressed(&self, a: Action) -> bool {
        self.now.contains(&a)
    }
    pub fn just_pressed(&self, a: Action) -> bool {
        self.now.contains(&a) && !self.prev.contains(&a) && self.fresh.contains(&a)
    }
    /// Held, even while a longer chord containing it wins: lock move (Ctrl) stays on during the
    /// dash chord (Ctrl + C), which is how a dash or dribble from lock move is pressed.
    pub fn held(&self, a: Action) -> bool {
        self.raw_prev.contains(&a) // already this frame's chords once read_input has run
    }
    /// A press not used yet: tap moves until the frame they're released, other kicks while held.
    /// A move that isn't allowed at the press fires as soon as it is, within that time.
    pub fn pending(&self, a: Action) -> bool {
        self.pending.contains(&a)
    }
    /// Uses up a pending press.
    pub fn consume(&mut self, a: Action) {
        self.pending.remove(&a);
    }
    fn update_pending(&mut self) {
        let fresh: Vec<Action> = Action::ALL.into_iter().filter(|a| self.just_pressed(*a)).collect();
        self.pending.extend(fresh);
        let (now, prev) = (&self.now, &self.prev);
        self.pending.retain(|a| now.contains(a) || (a.tap() && prev.contains(a)));
    }

    /// For tests: start a frame with exactly these actions held.
    #[cfg(test)]
    pub fn hold(&mut self, held: &[Action]) {
        self.prev = std::mem::replace(&mut self.now, held.iter().copied().collect());
        self.raw_prev = self.now.clone();
        self.fresh = self.now.difference(&self.prev).copied().collect();
        self.update_pending();
    }
}

/// Analog inputs for this frame.
#[derive(Resource, Default)]
pub struct Axes {
    pub movement: Vec2,
    /// Camera change in degrees: x = yaw (right +), y = pitch (up +). Mouse and sticks are kept
    /// apart because the mouse drives the curve instead of the camera while a shot charges.
    pub look_mouse: Vec2,
    pub look_pad: Vec2,
    /// Curve input for shots, -1..1 on each axis (mouse accumulator or left stick).
    pub curve: Vec2,
    curve_mouse: Vec2,
}

impl Axes {
    pub fn reset_curve(&mut self) {
        self.curve_mouse = Vec2::ZERO;
    }
}

/// Set while the controls menu is open or a binding is being captured: gameplay ignores input.
#[derive(Resource, Default)]
pub struct InputBlocked(pub bool);

/// Pressed this very frame (`modifier`: the modifier's own key was).
fn pressed_now(input: &Input, modifier: bool, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, pads: &Query<&Gamepad>) -> bool {
    match input {
        Input::Key(k) => keys.just_pressed(*k),
        Input::Mouse(m) => mouse.just_pressed(*m),
        Input::Pad(b) => pads.iter().any(|g| g.just_pressed(*b)),
        Input::Modifier => modifier,
    }
}

fn held(input: &Input, modifier: bool, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, pads: &Query<&Gamepad>) -> bool {
    match input {
        Input::Key(k) => keys.pressed(*k),
        Input::Mouse(m) => mouse.pressed(*m),
        Input::Pad(b) => pads.iter().any(|g| g.pressed(*b)),
        Input::Modifier => modifier,
    }
}

pub fn read_input(
    bindings: Res<Bindings>,
    blocked: Res<InputBlocked>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time>,
    mut actions: ResMut<Actions>,
    mut axes: ResMut<Axes>,
) {
    let t = time.elapsed_secs();
    let modifier = [&bindings.keyboard, &bindings.gamepad].iter().any(|chords| {
        chords.get(&Action::Modifier).is_some_and(|c| !c.is_empty() && c.iter().all(|i| held(i, false, &keys, &mouse, &pads)))
    });
    let modifier_pressed = [&bindings.keyboard, &bindings.gamepad]
        .iter()
        .any(|chords| chords.get(&Action::Modifier).is_some_and(|c| c.iter().any(|i| pressed_now(i, false, &keys, &mouse, &pads))));
    let mut raw: Vec<(Action, &Vec<Input>)> = Vec::new();
    let mut fresh = HashSet::new();
    if !blocked.0 {
        for chords in [&bindings.keyboard, &bindings.gamepad] {
            for (action, chord) in chords {
                if !chord.is_empty() && chord.iter().all(|i| held(i, modifier, &keys, &mouse, &pads)) {
                    raw.push((*action, chord));
                    if chord.iter().any(|i| pressed_now(i, modifier_pressed, &keys, &mouse, &pads)) {
                        fresh.insert(*action);
                    }
                }
            }
        }
    }
    actions.fresh = fresh;
    let mut now: HashSet<Action> = raw
        .iter()
        .filter(|(_, chord)| !raw.iter().any(|(_, other)| other.len() > chord.len() && chord.iter().all(|i| other.contains(i))))
        .map(|(a, _)| *a)
        .collect();

    // Double-tap-and-hold actions.
    let raw_now: HashSet<Action> = raw.iter().map(|(a, _)| *a).collect();
    for a in Action::ALL.into_iter().filter(|a| a.double_tap()) {
        let started = raw_now.contains(&a) && !actions.raw_prev.contains(&a);
        if started && actions.released_at.get(&a).is_some_and(|r| t - r < DOUBLE_TAP_WINDOW) {
            actions.doubled.insert(a);
        }
        if !raw_now.contains(&a) {
            if actions.raw_prev.contains(&a) {
                actions.released_at.insert(a, t);
            }
            actions.doubled.remove(&a);
        }
        if !actions.doubled.contains(&a) {
            now.remove(&a);
        }
    }
    actions.raw_prev = raw_now;
    actions.prev = std::mem::replace(&mut actions.now, now);
    actions.update_pending();

    // Analog: keyboard directions plus the left stick; mouse plus the right stick for the camera.
    let key_axis = |pos: Action, neg: Action| actions.pressed(pos) as i32 as f32 - actions.pressed(neg) as i32 as f32;
    let mut movement = Vec2::new(key_axis(Action::MoveRight, Action::MoveLeft), key_axis(Action::MoveForward, Action::MoveBack));
    let (mut look_pad, mut look_mouse, mut stick_curve) = (Vec2::ZERO, Vec2::ZERO, Vec2::ZERO);
    if !blocked.0 {
        for pad in &pads {
            movement += pad.left_stick();
            if pad.left_stick().length() > CURVE_STICK_DEAD_ZONE {
                stick_curve += pad.left_stick();
            }
            look_pad += pad.right_stick();
        }
        if window.cursor_options.grab_mode != bevy::window::CursorGrabMode::None {
            let delta = Vec2::new(motion.delta.x, -motion.delta.y);
            look_mouse = delta;
            // The accumulator is held inside a circle: full curve and full dip can't both max out.
            axes.curve_mouse += delta * MOUSE_AXIS_SCALE * MAGNUS_ACCUMULATOR_SENSITIVITY * bindings.magnus_sensitivity;
            axes.curve_mouse = axes.curve_mouse.clamp_length_max(1.0);
        }
    }
    // For screenshots: RE_RUN=1 holds forward; RE_RUN=w1,d0.5,.1 holds each key (w a s d, or . for
    // none) for so many seconds in turn, then the last one.
    if let Ok(script) = std::env::var("RE_RUN") {
        movement = scripted_movement(&script, t);
    }
    axes.movement = movement.clamp_length_max(1.0);
    axes.look_mouse = look_mouse;
    axes.look_pad = look_pad;
    axes.curve = (axes.curve_mouse + stick_curve).clamp_length_max(1.0);
}

fn scripted_movement(script: &str, t: f32) -> Vec2 {
    let mut end = 0.0;
    let mut dir = Vec2::Y;
    for step in script.split(',').filter(|s| s.len() > 1) {
        let (key, secs) = step.split_at(1);
        dir = match key {
            "w" => Vec2::Y,
            "s" => Vec2::NEG_Y,
            "a" => Vec2::NEG_X,
            "d" => Vec2::X,
            _ => Vec2::ZERO,
        };
        end += secs.parse::<f32>().unwrap_or(0.0);
        if t < end {
            break;
        }
    }
    dir
}

pub fn input_name(input: &Input) -> String {
    match input {
        Input::Key(k) => {
            let s = format!("{k:?}");
            match s.as_str() {
                "ShiftLeft" => "LShift".into(),
                "ShiftRight" => "RShift".into(),
                "ControlLeft" => "LCtrl".into(),
                "ControlRight" => "RCtrl".into(),
                "AltLeft" => "LAlt".into(),
                _ => s.trim_start_matches("Key").trim_start_matches("Digit").to_string(),
            }
        }
        Input::Mouse(m) => match m {
            MouseButton::Left => "LMB".into(),
            MouseButton::Right => "RMB".into(),
            MouseButton::Middle => "MMB".into(),
            MouseButton::Back => "Mouse4".into(),
            MouseButton::Forward => "Mouse5".into(),
            MouseButton::Other(n) => format!("Mouse{n}"),
        },
        Input::Pad(b) => match b {
            GamepadButton::South => "A".into(),
            GamepadButton::East => "B".into(),
            GamepadButton::West => "X".into(),
            GamepadButton::North => "Y".into(),
            GamepadButton::LeftTrigger => "LB".into(),
            GamepadButton::RightTrigger => "RB".into(),
            GamepadButton::LeftTrigger2 => "LT".into(),
            GamepadButton::RightTrigger2 => "RT".into(),
            GamepadButton::LeftThumb => "L3".into(),
            GamepadButton::RightThumb => "R3".into(),
            other => format!("{other:?}"),
        },
        Input::Modifier => "Modifier".into(),
    }
}

pub fn chord_name(chord: Option<&Vec<Input>>) -> String {
    match chord {
        Some(c) if !c.is_empty() => c.iter().map(input_name).collect::<Vec<_>>().join(" + "),
        _ => "-".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_survive_save_format() {
        let b = Bindings::default();
        let text = ron::ser::to_string_pretty(&b, default()).unwrap();
        assert!(ron::from_str::<Bindings>(&text).unwrap() == b);
    }
}
