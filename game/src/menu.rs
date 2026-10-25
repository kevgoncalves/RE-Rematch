//! The Esc menu, laid out like Rematch's own (from Kev's recording of its menus): a pause page
//! (Options, Leave) and the settings in Rematch's tabs, Gameplay, Audio and Controls, plus our
//! Cheats. Each tab is a scrolling list of options in sections, with the option under the mouse
//! described on the right. Controls also has the custom mapping (click a binding, press the new
//! keys, release; Esc cancels) and, on the right, a map of the keyboard or gamepad with the actions
//! of each part of the game. Rematch's options the prototype doesn't have yet are listed too,
//! dimmed and marked "(soon)", so the pages match. Its Video tab is left out for now.
//!
//! Keys: Esc steps back, F1 goes straight to the settings, Q and E change tab, A and D change the
//! part of the game the Controls map shows, L resets the tab to its defaults (after asking).
//!
//! The look (bars, slanted tabs, row highlight, keycaps, mouse and gamepad glyphs) is drawn in
//! code with Bevy UI and its default font: none of the game's textures, icons or fonts.

use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::ScheduleSystem;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::LineHeight;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::fx::{Cheat, Cheats};
use crate::input::{Action, Actions, Bindings, Device, Input, InputBlocked, MOUSE_SENSITIVITY_RANGE, chord_name, input_name};
use crate::tuning::{CAMERA_KICK_LIMIT, FOV_OPTION_RANGE, MAGNUS_SENSITIVITY_RANGE};

// ------------------------------------------------------------------------------------------------
// Pages, tabs and the menu's state

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Pause,
    Settings,
}

/// Rematch's settings tabs (but Video, for later) and our Cheats.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    #[default]
    Gameplay,
    Audio,
    Controls,
    Cheats,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Gameplay, Tab::Audio, Tab::Controls, Tab::Cheats];

    fn label(self) -> &'static str {
        match self {
            Tab::Gameplay => "GAMEPLAY",
            Tab::Audio => "AUDIO",
            Tab::Controls => "CONTROLS",
            Tab::Cheats => "CHEATS",
        }
    }

    fn step(self, by: i32) -> Self {
        step_in(&Self::ALL, self, by)
    }
}

/// The parts of the game the Controls map shows (Rematch's tabs over it, changed with A and D).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Part {
    #[default]
    Basics,
    WithBall,
    WithoutBall,
    Goalkeeper,
    Spectator,
}

impl Part {
    const ALL: [Part; 5] = [Part::Basics, Part::WithBall, Part::WithoutBall, Part::Goalkeeper, Part::Spectator];

    fn label(self) -> &'static str {
        match self {
            Part::Basics => "BASICS",
            Part::WithBall => "WITH BALL",
            Part::WithoutBall => "WITHOUT BALL",
            Part::Goalkeeper => "GOALKEEPER",
            Part::Spectator => "SPECTATOR",
        }
    }

    fn step(self, by: i32) -> Self {
        step_in(&Self::ALL, self, by)
    }
}

/// The next tab along, stopping at the ends as Rematch's Q and E do.
fn step_in<T: Copy + PartialEq>(all: &[T], at: T, by: i32) -> T {
    let i = all.iter().position(|t| *t == at).unwrap_or(0) as i32;
    all[(i + by).clamp(0, all.len() as i32 - 1) as usize]
}

/// The next option of a choice, going round.
fn wrap(at: usize, by: i32, count: usize) -> usize {
    (at as i32 + by).rem_euclid(count.max(1) as i32) as usize
}

#[derive(Resource, Default)]
pub struct Menu {
    /// The page shown; none while playing.
    page: Option<Page>,
    /// The settings tab, kept while the menu is closed.
    tab: Tab,
    /// The part of the game the Controls map shows.
    part: Part,
    /// Whose bindings the custom mapping shows: the device used last when the menu opened.
    device: Device,
    /// Binding being captured, and the inputs pressed so far (in press order).
    capturing: Option<(Action, Device, Vec<Input>)>,
    /// The row the capture started from, lit while it lasts.
    capture_row: Option<Entity>,
    /// A question waiting for Confirm or Cancel.
    confirm: Option<Confirm>,
    /// The row whose description shows: the last one under the mouse.
    described: Option<Entity>,
    /// For screenshots: scroll the open list this far, and light this row as if hovered.
    scroll_to: Option<f32>,
    lit: Option<usize>,
}

impl Menu {
    pub fn open(&self) -> bool {
        self.page.is_some()
    }

    /// Esc answers an open question with Cancel; otherwise it opens the pause page and closes it
    /// again, and from the settings it steps back to it.
    fn escape(&mut self) {
        if self.confirm.take().is_some() {
            return;
        }
        self.page = match self.page {
            None | Some(Page::Settings) => Some(Page::Pause),
            Some(Page::Pause) => None,
        };
    }

    /// F1 goes straight to the settings and back out.
    fn f1(&mut self) {
        self.page = if self.page == Some(Page::Settings) { None } else { Some(Page::Settings) };
    }

    /// L asks before resetting the tab (Audio has nothing to reset yet).
    fn ask_reset(&mut self) {
        if self.tab != Tab::Audio {
            self.confirm = Some(Confirm::Reset(self.tab));
        }
    }

    fn start_capture(&mut self, action: Action, row: Entity) {
        self.capturing = Some((action, self.device, Vec::new()));
        self.capture_row = Some(row);
    }

    /// For screenshots (RE_MENU): a tab's name opens that tab ("gameplay", "audio", "controls",
    /// "cheats"), anything else the pause page. Options follow after colons: `scroll=<px>` scrolls
    /// the list, `part=<1-5>` picks the Controls map's part, `pad` shows the gamepad mapping,
    /// `lit=<n>` lights the n-th row (from 0) as if hovered, `capture=<Action>` shows that binding
    /// being captured, `ask` opens the reset question. For example `controls:scroll=500:lit=14`.
    pub fn open_scripted(&mut self, what: &str) {
        let mut options = what.split(':');
        let name = options.next().unwrap_or_default();
        self.page = Some(Page::Pause);
        if let Some(tab) = Tab::ALL.into_iter().find(|t| name.eq_ignore_ascii_case(t.label())) {
            self.page = Some(Page::Settings);
            self.tab = tab;
        }
        for option in options {
            let (key, value) = option.split_once('=').unwrap_or((option, ""));
            match key {
                "scroll" => self.scroll_to = value.parse().ok(),
                "part" => {
                    if let Ok(n) = value.parse::<usize>() {
                        self.part = Part::ALL[n.clamp(1, Part::ALL.len()) - 1];
                    }
                }
                "pad" => self.device = Device::Gamepad,
                "lit" => self.lit = value.parse().ok(),
                "ask" => self.ask_reset(),
                "capture" => {
                    if let Some(action) = Action::ALL.into_iter().find(|a| format!("{a:?}") == value) {
                        self.capturing = Some((action, self.device, Vec::new()));
                    }
                }
                _ => {}
            }
        }
    }
}

/// A question the menu asks before it changes something you can't get back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Confirm {
    /// Switching a custom mapping to a preset.
    Preset(Device, Preset),
    /// "Reset to default" on a tab.
    Reset(Tab),
}

impl Confirm {
    fn text(self) -> String {
        match self {
            Confirm::Preset(..) => "Changing this option will reset the current mapping.".into(),
            Confirm::Reset(tab) => format!("Reset the {} settings to their default values?", tab.label()),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Options: sliders, choices and presets

/// Rematch's mapping presets that the prototype has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Preset {
    Default,
    /// Rematch's "Double thumb buttons preset": the ball moves on the mouse's side buttons, with
    /// C held as the dive and lob modifier.
    DoubleThumb,
}

impl Preset {
    fn of(device: Device) -> &'static [Preset] {
        match device {
            Device::Keyboard => &[Preset::Default, Preset::DoubleThumb],
            Device::Gamepad => &[Preset::Default],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Preset::Default => "Default preset",
            Preset::DoubleThumb => "Double thumb buttons preset",
        }
    }

    fn bindings(self) -> Bindings {
        match self {
            Preset::Default => Bindings::default(),
            Preset::DoubleThumb => Bindings::rematch_modifier_preset(),
        }
    }
}

fn chords(b: &Bindings, device: Device) -> &BTreeMap<Action, Vec<Input>> {
    match device {
        Device::Keyboard => &b.keyboard,
        Device::Gamepad => &b.gamepad,
    }
}

fn chord_of(b: &Bindings, device: Device, action: Action) -> Vec<Input> {
    chords(b, device).get(&action).cloned().unwrap_or_default()
}

/// The preset a device's bindings are, or none for a custom mapping.
fn current_preset(b: &Bindings, device: Device) -> Option<Preset> {
    Preset::of(device).iter().copied().find(|p| chords(&p.bindings(), device) == chords(b, device))
}

/// A preset's buttons for one device, everything else as you had it.
fn apply_preset(b: &Bindings, device: Device, preset: Preset) -> Bindings {
    let mut out = b.clone();
    *out.get_mut(device) = chords(&preset.bindings(), device).clone();
    out
}

/// "Reset to default" on a tab: that tab's options back to their defaults, the rest kept.
fn reset_tab(b: &Bindings, tab: Tab, cheats: &mut Cheats) -> Bindings {
    let d = Bindings::default();
    match tab {
        Tab::Gameplay => Bindings {
            mouse_sensitivity: d.mouse_sensitivity,
            mouse_vertical_scale: d.mouse_vertical_scale,
            magnus_sensitivity: d.magnus_sensitivity,
            fov: d.fov,
            invert_y: d.invert_y,
            invert_x: d.invert_x,
            camera_lock: d.camera_lock,
            ..b.clone()
        },
        Tab::Controls => Bindings { keyboard: d.keyboard, gamepad: d.gamepad, pass_aim: d.pass_aim, ..b.clone() },
        Tab::Cheats => {
            for cheat in Cheat::ALL {
                if cheats.on(cheat) {
                    cheats.toggle(cheat);
                }
            }
            b.clone()
        }
        Tab::Audio => b.clone(),
    }
}

/// A slider option, saved with the bindings.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Setting {
    MouseSensitivity,
    MouseVerticalScale,
    MagnusSensitivity,
    Fov,
}

impl Setting {
    fn range(self) -> (f32, f32) {
        match self {
            Setting::MouseSensitivity | Setting::MouseVerticalScale => MOUSE_SENSITIVITY_RANGE,
            Setting::MagnusSensitivity => MAGNUS_SENSITIVITY_RANGE,
            Setting::Fov => FOV_OPTION_RANGE,
        }
    }

    fn step(self) -> f32 {
        match self {
            Setting::Fov => 1.0,
            _ => 0.1,
        }
    }

    fn value(self, b: &Bindings) -> f32 {
        match self {
            Setting::MouseSensitivity => b.mouse_sensitivity,
            Setting::MouseVerticalScale => b.mouse_vertical_scale,
            Setting::MagnusSensitivity => b.magnus_sensitivity,
            Setting::Fov => b.fov,
        }
    }

    fn set(self, b: &mut Bindings, v: f32) {
        match self {
            Setting::MouseSensitivity => b.mouse_sensitivity = v,
            Setting::MouseVerticalScale => b.mouse_vertical_scale = v,
            Setting::MagnusSensitivity => b.magnus_sensitivity = v,
            Setting::Fov => b.fov = v,
        }
    }

    /// Slider position 0..1 to a value on the option's steps.
    fn at_ratio(self, ratio: f32) -> f32 {
        let (lo, hi) = self.range();
        let v = lo + ratio.clamp(0.0, 1.0) * (hi - lo);
        ((v / self.step()).round() * self.step()).clamp(lo, hi)
    }
}

fn ratio(v: f32, (lo, hi): (f32, f32)) -> f32 {
    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// A number as Rematch's sliders show it: "1", "0.2", "90".
fn number(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

const ON_OFF: [&str; 2] = ["Disabled", "Enabled"];

/// An option with a few named values, cycled with the arrows either side of it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Choice {
    /// Rematch's "Tap aiming mode": how keyboard-and-mouse passes and lobs aim.
    PassAim,
    /// Rematch's "Lock shoot on controller".
    CameraLock,
    InvertY,
    InvertX,
    /// Rematch's mapping presets for a device ("Custom" once you change a binding).
    Preset(Device),
    /// Which device's bindings the custom mapping shows (ours: Rematch shows the one in use).
    Mapping,
    /// A practice cheat.
    Cheat(Cheat),
}

impl Choice {
    /// The option's values as shown, and which one is on.
    fn state(self, b: &Bindings, cheats: &Cheats, menu: &Menu) -> (Vec<&'static str>, usize) {
        match self {
            Choice::PassAim => (vec!["Directional aiming", "Joystick emulation", "Crosshair only"], b.pass_aim as usize),
            Choice::CameraLock => (ON_OFF.to_vec(), b.camera_lock as usize),
            Choice::InvertY => (ON_OFF.to_vec(), b.invert_y as usize),
            Choice::InvertX => (ON_OFF.to_vec(), b.invert_x as usize),
            Choice::Preset(device) => {
                let presets = Preset::of(device);
                let mut names: Vec<_> = presets.iter().map(|p| p.label()).collect();
                names.push("Custom");
                let at = current_preset(b, device).and_then(|p| presets.iter().position(|q| *q == p)).unwrap_or(presets.len());
                (names, at)
            }
            Choice::Mapping => (vec!["Keyboard and mouse", "Controller"], (menu.device == Device::Gamepad) as usize),
            Choice::Cheat(cheat) => (ON_OFF.to_vec(), cheats.on(cheat) as usize),
        }
    }
}

/// Steps a choice; true if the bindings changed (and need saving). A preset over a custom
/// mapping asks first.
fn step_choice(choice: Choice, by: i32, b: &mut Bindings, cheats: &mut Cheats, menu: &mut Menu) -> bool {
    match choice {
        // Three modes: two steps forward is one back.
        Choice::PassAim => b.pass_aim = if by > 0 { b.pass_aim.next() } else { b.pass_aim.next().next() },
        Choice::CameraLock => b.camera_lock ^= true,
        Choice::InvertY => b.invert_y ^= true,
        Choice::InvertX => b.invert_x ^= true,
        Choice::Preset(device) => {
            let presets = Preset::of(device);
            match current_preset(b, device) {
                Some(p) => {
                    let next = presets[wrap(presets.iter().position(|q| *q == p).unwrap_or(0), by, presets.len())];
                    if next == p {
                        return false;
                    }
                    *b = apply_preset(b, device, next);
                }
                None => {
                    let next = if by > 0 { presets[0] } else { presets[presets.len() - 1] };
                    menu.confirm = Some(Confirm::Preset(device, next));
                    return false;
                }
            }
        }
        Choice::Mapping => {
            menu.device = if menu.device == Device::Keyboard { Device::Gamepad } else { Device::Keyboard };
            return false;
        }
        Choice::Cheat(cheat) => {
            cheats.toggle(cheat);
            return false;
        }
    }
    true
}

// ------------------------------------------------------------------------------------------------
// The rows of each tab

/// An option Rematch has and the prototype doesn't yet: shown at Rematch's default, dimmed.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Soon {
    Slider { value: f32, lo: f32, hi: f32 },
    Choice { value: &'static str, at: usize, of: usize },
    /// The key Rematch binds it to ("" for none).
    Key(&'static str),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Control {
    Slider(Setting),
    Choice(Choice),
    /// Bindings, one slot per action (Move has four).
    Bind(&'static [Action]),
    Soon(Soon),
}

#[derive(Clone, Copy, Debug)]
struct RowDef {
    label: &'static str,
    desc: &'static str,
    control: Control,
}

impl RowDef {
    /// Not in the prototype: a Rematch option we don't have, or actions bound but not played yet.
    fn soon(&self) -> bool {
        match self.control {
            Control::Soon(_) => true,
            Control::Bind(actions) => actions.iter().all(|a| !a.implemented()),
            _ => false,
        }
    }

    fn label(&self) -> String {
        if self.soon() { format!("{} (soon)", self.label) } else { self.label.to_string() }
    }

    fn desc(&self) -> String {
        self.desc.replace("{kick}", &format!("{:.0}", CAMERA_KICK_LIMIT.0))
    }

    /// The description's "Default value".
    fn default_value(&self, device: Device) -> String {
        let d = Bindings::default();
        match self.control {
            Control::Slider(s) => number(s.value(&d)),
            Control::Choice(Choice::Mapping) => "the device you used last".into(),
            Control::Choice(c) => {
                let (names, at) = c.state(&d, &Cheats::default(), &Menu::default());
                names[at].into()
            }
            Control::Bind(actions) => {
                let names: Vec<_> = actions.iter().filter_map(|a| chords(&d, device).get(a)).map(|c| chord_name(Some(c))).collect();
                match (names.is_empty(), actions == MOVE) {
                    (false, _) => names.join(", "),
                    (true, true) => "LEFT STICK".into(),
                    (true, false) => "None".into(),
                }
            }
            Control::Soon(Soon::Slider { value, .. }) => number(value),
            Control::Soon(Soon::Choice { value, .. }) => value.into(),
            Control::Soon(Soon::Key(k)) => if k.is_empty() { "None".into() } else { k.into() },
        }
    }
}

struct Section {
    title: &'static str,
    /// Rematch's green heading over the custom mapping.
    green: bool,
    rows: Vec<RowDef>,
}

fn row(label: &'static str, desc: &'static str, control: Control) -> RowDef {
    RowDef { label, desc, control }
}

fn soon_slider(value: f32, lo: f32, hi: f32) -> Control {
    Control::Soon(Soon::Slider { value, lo, hi })
}

fn soon_choice(value: &'static str, at: usize, of: usize) -> Control {
    Control::Soon(Soon::Choice { value, at, of })
}

fn soon_key(key: &'static str) -> Control {
    Control::Soon(Soon::Key(key))
}

fn bind(actions: &'static [Action]) -> Control {
    Control::Bind(actions)
}

const MOVE: &[Action] = &[Action::MoveForward, Action::MoveLeft, Action::MoveBack, Action::MoveRight];
const NOT_YET: &str = "Not in the prototype yet.";

/// Rematch's options per tab, in its order. Labels are Rematch's; the descriptions are ours.
fn sections(tab: Tab) -> Vec<Section> {
    use Action::*;
    let s = |title, rows| Section { title, green: false, rows };
    match tab {
        Tab::Gameplay => vec![
            s("ONLINE", vec![
                row("Crossplay", "Play squads, matches and custom matches with players on other platforms. Without it, finding a match can take longer.", soon_choice("Enabled", 1, 2)),
                row("Enable network next", "Sends the game's traffic over Network Next for a steadier connection.", soon_choice("Enabled", 1, 2)),
            ]),
            s("CAMERA", vec![
                row("Mouse sensitivity", "How far the camera turns for a move of the mouse.", Control::Slider(Setting::MouseSensitivity)),
                row("Mouse sensitivity : vertical scale", "The mouse's up and down, on top of its sensitivity.", Control::Slider(Setting::MouseVerticalScale)),
                row("Camera speed for controller", "How fast the right stick turns the camera.", soon_slider(1.0, 0.2, 2.0)),
                row("Camera speed boost for controller", "The extra turn speed with the right stick held all the way.", soon_slider(1.0, 0.0, 2.0)),
                row("Camera aim speed for controller", "How fast the right stick turns the camera while you aim.", soon_slider(1.0, 0.2, 2.0)),
                row("Camera aim speed boost for controller", "The extra turn speed while aiming, with the right stick held all the way.", soon_slider(1.0, 0.0, 2.0)),
                row("Magnus sensitivity", "How much moving the mouse while you charge curves the shot. Lower is finer. Keyboard and mouse only.", Control::Slider(Setting::MagnusSensitivity)),
                row("Camera FOV", "The camera's field of view: every camera state widens or narrows with it.", Control::Slider(Setting::Fov)),
                row("Camera shake", "How much the camera shakes on impacts.", soon_slider(1.0, 0.0, 1.0)),
                row("Inverted look Y axis", "Turns the camera's up and down around, for the mouse and the right stick.", Control::Choice(Choice::InvertY)),
                row("Inverted look X axis", "Turns the camera's left and right around, for the mouse and the right stick.", Control::Choice(Choice::InvertX)),
                row(
                    "Lock shoot on controller",
                    "On a gamepad, a shot locks the camera and its aim where it pointed at the press, as keyboard and mouse shots always do. Disabled: the right stick can still turn the camera while you kick, up to {kick} deg from the press.",
                    Control::Choice(Choice::CameraLock),
                ),
            ]),
            s("HUD", vec![
                row("Crosshair size", "The size of the crosshair.", soon_slider(0.2, 0.0, 1.0)),
                row("Crosshair color", "The colour of the crosshair.", soon_choice("White", 0, 7)),
                row("Minimap mode", "Hidden, fixed (your goal at the bottom) or rotating with you.", soon_choice("Rotating", 2, 3)),
                row("Minimap size", "The size of the minimap.", soon_slider(60.0, 0.0, 100.0)),
                row("Show network performance information", "Shows ping, download and upload speed, and packet loss.", soon_choice("Disabled", 0, 2)),
            ]),
        ],
        Tab::Audio => vec![
            s("VOLUMES", vec![
                row("Global volume", "The volume of the music and every sound.", soon_slider(70.0, 0.0, 100.0)),
                row("Gameplay music volume", "The music during matches.", soon_slider(70.0, 0.0, 100.0)),
                row("Menu music volume", "The music in the menus.", soon_slider(70.0, 0.0, 100.0)),
                row("Sound effects", "Kicks, the ball, footsteps and the rest of the game's sounds.", soon_slider(70.0, 0.0, 100.0)),
                row("Crowd volume", "The crowd in the stands.", soon_slider(70.0, 0.0, 100.0)),
                row("Voice chat volume", "Your teammates' voices.", soon_slider(70.0, 0.0, 100.0)),
                row("Communications volume", "The quick communications players send.", soon_slider(70.0, 0.0, 100.0)),
                row("Ball call volumes", "Players calling for the ball.", soon_slider(70.0, 0.0, 100.0)),
                row("Audio mixing", "Mixes the sound for your speakers or headphones.", soon_choice("Headphones", 0, 5)),
            ]),
            s("COMMUNICATION", vec![
                row("Voice chat", "Talk with your team.", soon_choice("Enabled", 1, 2)),
                row("In game voice chat activation mode", "Push to talk, or an open microphone.", soon_choice("Push to talk", 0, 2)),
            ]),
        ],
        Tab::Controls => vec![
            s("INPUT", vec![
                row("Scoreboard control", "Hold the key to see the scoreboard, or press it to show and hide it.", soon_choice("Hold", 0, 2)),
                row("Controller vibration intensity", "How strongly the controller vibrates; 0 turns it off.", soon_slider(1.0, 0.0, 1.0)),
                row(
                    "Tap aiming mode",
                    "How passes and lobs aim on keyboard and mouse. Directional aiming: the way your movement keys point. Joystick emulation: drag the mouse while you charge, like a stick. Crosshair only: where the crosshair aims.",
                    Control::Choice(Choice::PassAim),
                ),
                row("Smooth correction speed", "How fast joystick emulation's aim settles.", soon_slider(5.0, 1.0, 10.0)),
                row("Smooth correction range", "How far joystick emulation's aim can be corrected.", soon_slider(70.0, 0.0, 180.0)),
                row(
                    "Keyboard mapping presets",
                    "Rematch's keyboard and mouse layouts. Double thumb buttons puts the ball moves on the mouse's two side buttons, with C held as the dive and lob modifier. From a custom mapping it asks first.",
                    Control::Choice(Choice::Preset(Device::Keyboard)),
                ),
                row("Controller mapping presets", "Rematch's controller layout. From a custom mapping it asks first.", Control::Choice(Choice::Preset(Device::Gamepad))),
            ]),
            Section {
                title: "CUSTOM MAPPING",
                green: true,
                rows: vec![row(
                    "Mapping shown",
                    "Whose bindings the list shows and changes. It follows the device you used last when the menu opens. Click a binding, press the new keys and let go; Esc cancels.",
                    Control::Choice(Choice::Mapping),
                )],
            },
            s("BASICS", vec![
                row("Move", "Move forward, left, back and right. Click a key to change it; on a controller it's the left stick.", bind(MOVE)),
                row("Sprint", "Hold to sprint.", bind(&[Sprint])),
                row("Extra effort (double tap)", "Tap twice and hold for a burst of speed. Rematch puts it on the sprint key.", bind(&[ExtraEffort])),
                row("Look at ball", "Turns the camera to the ball.", bind(&[LookAtBall])),
            ]),
            s("WITH BALL", vec![
                row("Use dive/lob modifier", "With it on, a Modifier binding takes the place of Lob and Lob pushball: hold it with Tap or Push ball to lob.", soon_choice("Disabled", 0, 2)),
                row("Shoot", "Hold to charge a shot, let go to strike.", bind(&[Shoot])),
                row("[While shooting] Fake shoot", "While you charge a shot, cancels it with a fake.", bind(&[FakeShot])),
                row("Tap", "Hold to charge a pass, let go to play it.", bind(&[Pass])),
                row("Low power tap", "A soft pass.", bind(&[LowPowerPass])),
                row("Push ball", "Knocks the ball ahead of you.", bind(&[PushBall])),
                row("Lob", "Hold to charge a lob, let go to play it.", bind(&[Lob])),
                row("Low power lob", "A soft lob.", bind(&[LowPowerLob])),
                row("Lob pushball", "Pops the ball up ahead of you.", bind(&[LobbedPushBall])),
                row("Modifier", "Held with another binding, makes it another action (Double thumb buttons: Modifier + Tap is a lob).", bind(&[Modifier])),
                row("[In dribble stance] Dribble", "In dribble stance, a quick dribble to the side. Shares its binding with [In defensive stance] Dash.", bind(&[Dash])),
            ]),
            s("WITHOUT BALL", vec![
                row("Dribble stance / Defensive stance", "Hold for the stance: with the ball you dribble, without it you defend.", bind(&[Lock])),
                row("Tackle", "A standing tackle.", bind(&[StandingTackle])),
                row("[While sprinting] Sliding tackle", "A sliding tackle.", bind(&[SlidingTackle])),
                row("[In defensive stance] Dash", "In defensive stance, a quick step to the side. Shares its binding with [In dribble stance] Dribble.", bind(&[Dash])),
                row("Dive", "Dives for the ball.", bind(&[Dive])),
            ]),
            s("GOALKEEPER", vec![
                row("Goalkeeper dive", "The goalkeeper's dive.", soon_key("")),
                row("Goalkeeper long range dive", "The goalkeeper's long dive.", soon_key("")),
            ]),
            s("COMMUNICATION", vec![
                row("Ball call", "Calls for the ball. In freeplay it fires the ball cannon.", bind(&[BallCall])),
                row("Ping", "Marks a spot for your team.", bind(&[Ping])),
                row("Open/Close microphone", "Turns your microphone on and off.", soon_key("4")),
                row("Use 4 communications set", "Sends a communication with a single key.", soon_choice("Disabled", 0, 2)),
                row("Tactical communications", "Opens the tactical communications.", soon_key("1")),
                row("Compliment communications", "Opens the compliments.", soon_key("2")),
                row("Apologies communications", "Opens the apologies.", soon_key("3")),
                row("Send communication 1", "Sends the first communication of the open set.", soon_key("1")),
                row("Send communication 2", "Sends the second communication of the open set.", soon_key("2")),
                row("Send communication 3", "Sends the third communication of the open set.", soon_key("3")),
            ]),
        ],
        // Our own tab: generated from the cheats there are.
        Tab::Cheats => vec![s(
            "PRACTICE",
            Cheat::ALL.into_iter().map(|c| row(c.label(), "A practice aid of ours. Cheats are off again when the game restarts.", Control::Choice(Choice::Cheat(c)))).collect(),
        )],
    }
}

// ------------------------------------------------------------------------------------------------
// What the Controls map lists for each part of the game

struct Group {
    title: &'static str,
    items: &'static [Item],
}

enum Item {
    /// Actions and their bindings.
    Bind(&'static str, &'static [Action]),
    /// Something fixed: its name, keyboard key and gamepad button ("MOUSE" draws the mouse).
    Fixed(&'static str, &'static str, &'static str),
    /// One of Rematch's actions the prototype doesn't have yet, dimmed: name and Rematch's key.
    Later(&'static str, &'static str),
}

/// The map's three columns of groups for a part, as Rematch lays out its Basics.
fn legend(part: Part) -> [&'static [Group]; 3] {
    use Action::*;
    match part {
        Part::Basics => [
            &[
                Group { title: "Match infos", items: &[Item::Later("Show scoreboard", "TAB")] },
                Group { title: "Movement", items: &[Item::Bind("Move", MOVE), Item::Bind("Sprint", &[Sprint]), Item::Bind("Extra effort", &[ExtraEffort])] },
            ],
            &[Group { title: "Camera", items: &[Item::Fixed("Move camera", "MOUSE", "RIGHT STICK"), Item::Bind("Look at ball", &[LookAtBall])] }],
            &[Group {
                title: "Communication",
                items: &[
                    Item::Bind("Ball call", &[BallCall]),
                    Item::Bind("Ping", &[Ping]),
                    Item::Later("Tactical communications", "1"),
                    Item::Later("Compliment communications", "2"),
                    Item::Later("Apologies communications", "3"),
                    Item::Later("Send communication 1", "1"),
                    Item::Later("Send communication 2", "2"),
                    Item::Later("Send communication 3", "3"),
                    Item::Later("Open/Close microphone", "4"),
                ],
            }],
        ],
        Part::WithBall => [
            &[Group { title: "Shooting", items: &[Item::Bind("Shoot", &[Shoot]), Item::Bind("Fake shoot", &[FakeShot])] }],
            &[Group {
                title: "Passing",
                items: &[Item::Bind("Tap", &[Pass]), Item::Bind("Low power tap", &[LowPowerPass]), Item::Bind("Lob", &[Lob]), Item::Bind("Low power lob", &[LowPowerLob])],
            }],
            &[Group {
                title: "Ball control",
                items: &[Item::Bind("Push ball", &[PushBall]), Item::Bind("Lob pushball", &[LobbedPushBall]), Item::Bind("Dribble", &[Dash]), Item::Bind("Modifier", &[Modifier])],
            }],
        ],
        Part::WithoutBall => [
            &[Group { title: "Defending", items: &[Item::Bind("Defensive stance", &[Lock]), Item::Bind("Tackle", &[StandingTackle]), Item::Bind("Sliding tackle", &[SlidingTackle])] }],
            &[Group { title: "Movement", items: &[Item::Bind("Dash", &[Dash])] }],
            &[Group { title: "Saves", items: &[Item::Bind("Dive", &[Dive])] }],
        ],
        Part::Goalkeeper => [
            &[Group { title: "Goalkeeper", items: &[Item::Later("Goalkeeper dive", ""), Item::Later("Goalkeeper long range dive", "")] }],
            &[],
            &[],
        ],
        Part::Spectator => [&[Group { title: "Spectator", items: &[Item::Later("Spectator controls (soon)", "")] }], &[], &[]],
    }
}

// ------------------------------------------------------------------------------------------------
// Look: colours and sizes (sampled from the recording). Sizes are for a 720-pixel-high window
// (Rematch's 1080p layout scaled by 2/3) and scale with the window's height, as Rematch's do.

const ACCENT: Color = Color::srgb(0.443, 1.0, 0.627);
const ACCENT_DIM: Color = Color::srgb(0.318, 0.859, 0.486);
const ACCENT_HOVER: Color = Color::srgba(0.443, 1.0, 0.627, 0.3);
const BAR: Color = Color::srgb(0.153, 0.184, 0.196);
// UI blends in linear colour, so these need more alpha than they look to darken the game as much.
const LEFT_PANEL: Color = Color::srgba(0.04, 0.09, 0.16, 0.985);
const RIGHT_PANEL: Color = Color::srgba(0.06, 0.13, 0.24, 0.93);
const PAUSE_SHADE: Color = Color::srgba(0.03, 0.08, 0.15, 0.88);
const DIALOG: Color = Color::srgba(0.153, 0.184, 0.196, 0.97);
const TEXT: Color = Color::srgb(0.93, 0.95, 0.96);
const TEXT_DIM: Color = Color::srgb(0.47, 0.52, 0.58);
/// Text on a lit row.
const INK: Color = Color::srgb(0.03, 0.14, 0.11);
const KEYCAP: Color = Color::srgb(0.88, 0.925, 0.95);
const KEYCAP_DIM: Color = Color::srgba(0.88, 0.925, 0.95, 0.4);
const KEYCAP_TEXT: Color = Color::srgb(0.06, 0.11, 0.15);
const TRACK: Color = Color::srgb(0.39, 0.43, 0.46);
const WARN: Color = Color::srgb(0.94, 0.24, 0.43);
const MAP_LINE: Color = Color::srgba(0.85, 0.9, 0.95, 0.7);
const SOON_LIT: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);

const BAR_H: f32 = 48.0;
const FOOT_H: f32 = 47.0;
const LIST_TOP: f32 = 75.0;
const LIST_BOTTOM: f32 = 50.0;
const LIST_LEFT: f32 = 33.0;
const LIST_W: f32 = 561.0;
const ROW_H: f32 = 33.0;
const ROW_GAP: f32 = 7.0;
const SECTION_GAP: f32 = 20.0;
const LABEL_W: f32 = 254.0;
const VALUE_W: f32 = 52.0;
const SLIDER_W: f32 = 234.0;
const F_TAB: f32 = 14.0;
const F_SECTION: f32 = 13.0;
const F_ROW: f32 = 14.0;
const F_CAP: f32 = 12.0;

/// The menu's text size at a 720-pixel-high window (`scale_fonts` follows the window).
#[derive(Component, Clone, Copy)]
struct MenuFont(f32);

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: size, ..default() }, TextColor(color), MenuFont(size))
}

/// Text with Rematch's roomy line spacing, for descriptions.
fn prose(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: size, line_height: LineHeight::RelativeToFont(1.45), ..default() }, TextColor(color), MenuFont(size))
}

/// A length at a 720-pixel-high window, as a share of the window's height.
fn px(v: f32) -> Val {
    Val::Vh(v / 7.2)
}

/// A node filling its parent, behind the parent's other children.
fn fill() -> Node {
    Node { position_type: PositionType::Absolute, left: px(0.0), right: px(0.0), top: px(0.0), bottom: px(0.0), ..default() }
}

fn sliced(image: Handle<Image>, left: f32, right: f32) -> ImageNode {
    ImageNode {
        image,
        color: Color::NONE,
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect { left, right, top: 0.0, bottom: 0.0 },
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        }),
        ..default()
    }
}

// ------------------------------------------------------------------------------------------------
// Shapes, drawn in code

/// The menu's shapes: white, tinted where they're used.
#[derive(Resource, Clone)]
struct MenuArt {
    /// A tab's background: a parallelogram, both ends leaning right.
    slant: Handle<Image>,
    /// The pause page's highlight bar, with a small triangle at its end.
    bar: Handle<Image>,
    diamond: Handle<Image>,
    dot: Handle<Image>,
    arrow_left: Handle<Image>,
    arrow_right: Handle<Image>,
    warn: Handle<Image>,
    clear: Handle<Image>,
    pencil: Handle<Image>,
    corner: Handle<Image>,
    /// Mouse glyphs for keycaps: none lit, then each of MOUSE_BUTTONS lit.
    mouse: [Handle<Image>; 6],
    /// The map's mouse, and its buttons to light.
    mouse_map: Handle<Image>,
    mouse_fill: [Handle<Image>; 5],
}

const MOUSE_BUTTONS: [MouseButton; 5] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle, MouseButton::Back, MouseButton::Forward];

fn mouse_index(button: MouseButton) -> Option<usize> {
    MOUSE_BUTTONS.iter().position(|b| *b == button)
}

/// An image of a shape, antialiased: white where `inside` holds.
fn paint(images: &mut Assets<Image>, w: u32, h: u32, inside: impl Fn(Vec2) -> bool) -> Handle<Image> {
    const SS: u32 = 4;
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let hits = (0..SS * SS)
                .filter(|i| inside(Vec2::new(x as f32 + ((i % SS) as f32 + 0.5) / SS as f32, y as f32 + ((i / SS) as f32 + 0.5) / SS as f32)))
                .count() as u32;
            data.extend_from_slice(&[255, 255, 255, (hits * 255 / (SS * SS)) as u8]);
        }
    }
    let size = Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    images.add(Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()))
}

/// Signed distance to a rounded rectangle.
fn rounded(p: Vec2, min: Vec2, max: Vec2, r: f32) -> f32 {
    let q = (p - (min + max) / 2.0).abs() - ((max - min) / 2.0 - Vec2::splat(r));
    q.max(Vec2::ZERO).length() + q.max_element().min(0.0) - r
}

/// Distance to a segment, and how far along it the closest point is (0..1).
fn segment(p: Vec2, a: Vec2, b: Vec2) -> (f32, f32) {
    let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
    (p.distance(a + (b - a) * t), t)
}

/// A mouse in a 28 x 40 box: its outline (`button` none), or one of its buttons filled.
fn mouse_shape(p: Vec2, button: Option<MouseButton>, stroke: f32) -> bool {
    let body = rounded(p, Vec2::new(5.0, 1.5), Vec2::new(25.0, 38.5), 10.0);
    let wheel = rounded(p, Vec2::new(12.5, 6.0), Vec2::new(17.5, 13.0), 2.5);
    let side = |top: f32| rounded(p, Vec2::new(1.0, top), Vec2::new(5.5, top + 5.0), 1.2);
    let (split_x, split_y) = (15.0, 17.0);
    let inside = body <= 0.0;
    match button {
        None => {
            (inside && body > -stroke)
                || (inside && p.y < split_y && (p.x - split_x).abs() < stroke / 2.0 && wheel > 0.0)
                || (inside && (p.y - split_y).abs() < stroke / 2.0)
                || (wheel <= 0.0 && wheel > -stroke)
                || [19.0, 26.0].into_iter().any(|t| side(t) <= 0.0 && side(t) > -stroke && p.x < 5.5)
        }
        Some(MouseButton::Left) => inside && p.x < split_x && p.y < split_y && wheel > 0.0,
        Some(MouseButton::Right) => inside && p.x > split_x && p.y < split_y && wheel > 0.0,
        Some(MouseButton::Middle) => wheel <= 0.0,
        Some(MouseButton::Forward) => side(19.0) <= 0.0,
        Some(MouseButton::Back) => side(26.0) <= 0.0,
        Some(MouseButton::Other(_)) => false,
    }
}

impl MenuArt {
    fn new(images: &mut Assets<Image>) -> Self {
        let glyph = |images: &mut Assets<Image>, lit: Option<MouseButton>| {
            paint(images, 28, 40, move |p| mouse_shape(p, None, 2.6) || lit.is_some_and(|b| mouse_shape(p, Some(b), 0.0)))
        };
        let map_scale = 28.0 / 60.0;
        Self {
            slant: paint(images, 48, 48, |p| p.x >= 14.0 * (1.0 - p.y / 48.0) && p.x <= 48.0 - 14.0 * (p.y / 48.0)),
            bar: paint(images, 48, 32, |p| p.x <= 36.0 || (p.x >= 38.0 && p.y >= 22.0 && p.x - 38.0 >= 32.0 - p.y)),
            diamond: paint(images, 16, 16, |p| (p - Vec2::splat(8.0)).abs().element_sum() <= 7.5),
            dot: paint(images, 8, 8, |p| p.distance(Vec2::splat(4.0)) <= 2.2),
            arrow_left: paint(images, 10, 14, |p| p.x >= 1.5 && p.x <= 8.5 && (p.y - 7.0).abs() <= (p.x - 1.5) * 0.85),
            arrow_right: paint(images, 10, 14, |p| p.x >= 1.5 && p.x <= 8.5 && (p.y - 7.0).abs() <= (8.5 - p.x) * 0.85),
            warn: paint(images, 18, 16, |p| {
                let tri = p.y >= 1.0 && p.y <= 15.0 && (p.x - 9.0).abs() <= (p.y - 1.0) * 8.0 / 14.0;
                let mark = ((p.x - 9.0).abs() < 1.1 && p.y > 5.5 && p.y < 10.5) || p.distance(Vec2::new(9.0, 12.6)) < 1.2;
                tri && !mark
            }),
            clear: paint(images, 18, 18, |p| {
                let c = Vec2::splat(9.0);
                (p.distance(c) - 7.0).abs() < 1.2 || (segment(p, Vec2::splat(4.0), Vec2::splat(14.0)).0 < 1.2)
            }),
            pencil: paint(images, 18, 18, |p| {
                let (d, t) = segment(p, Vec2::new(2.5, 15.5), Vec2::new(15.0, 3.0));
                d < 2.2 * (t / 0.2).min(1.0) && (t - 0.8).abs() > 0.04
            }),
            corner: paint(images, 24, 24, |p| p.x + p.y <= 24.0),
            mouse: [
                glyph(images, None),
                glyph(images, Some(MouseButton::Left)),
                glyph(images, Some(MouseButton::Right)),
                glyph(images, Some(MouseButton::Middle)),
                glyph(images, Some(MouseButton::Back)),
                glyph(images, Some(MouseButton::Forward)),
            ],
            mouse_map: paint(images, 60, 84, move |p| mouse_shape(p * map_scale, None, 0.8)),
            mouse_fill: MOUSE_BUTTONS.map(|b| paint(images, 60, 84, move |p| mouse_shape(p * map_scale, Some(b), 0.0))),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Components

#[derive(Component)]
pub struct MenuRoot;

/// The page's root node.
#[derive(Component)]
pub struct PageNode(Page);

/// The contents of a settings tab.
#[derive(Component)]
pub struct TabBody(Tab);

#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum MenuButton {
    Options,
    /// There's no main menu yet, so this quits.
    Leave,
    Resume,
    /// From the settings back to the pause page.
    Back,
    Reset,
    Tab(Tab),
    Part(Part),
    Confirm,
    Cancel,
}

/// A part of a button whose colour follows it (its background or its label).
#[derive(Component, Clone, Copy)]
struct Skin(Entity);

/// An option's row: the whole row lights under the mouse and describes the option.
#[derive(Component, Clone, Copy)]
struct Row {
    def: RowDef,
    tab: Tab,
    index: usize,
}

/// A part of a row whose colour follows the row's state (lit, dimmed).
#[derive(Component, Clone, Copy)]
struct Tint {
    row: Entity,
    ink: Ink,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ink {
    LabelPart,
    ControlPart,
    Label,
    /// Values, slider lines and knobs, choice dots: green, dark on a lit row.
    Value,
    Glyph,
    Plus,
    /// Only shown on a lit row (the choice arrows, the binding icons).
    OnHover,
}

fn ink_color(ink: Ink, lit: bool, soon: bool) -> Color {
    match ink {
        Ink::LabelPart if lit => if soon { SOON_LIT } else { ACCENT_DIM },
        Ink::ControlPart if lit => if soon { SOON_LIT } else { ACCENT },
        Ink::LabelPart | Ink::ControlPart => Color::NONE,
        Ink::Label => if soon { TEXT_DIM } else if lit { INK } else { TEXT },
        Ink::Value => if soon { TEXT_DIM } else if lit { INK } else { ACCENT },
        Ink::Glyph => if lit && !soon { INK } else { TEXT },
        Ink::Plus => if lit && !soon { INK } else { TEXT_DIM },
        Ink::OnHover => INK,
    }
}

#[derive(Component, Clone, Copy)]
struct SliderTrack(Setting);

#[derive(Component, Clone, Copy)]
struct SliderKnob(Setting);

#[derive(Component, Clone, Copy)]
struct SliderValue(Setting);

#[derive(Component, Clone, Copy)]
struct ChoiceControl(Choice);

#[derive(Component, Clone, Copy)]
struct ChoiceText(Choice);

/// A choice's dots, one per value, a diamond on the one that's on.
#[derive(Component)]
struct ChoiceDots {
    choice: Choice,
    row: Entity,
    shown: Option<(usize, usize)>,
}

/// One action's binding in a row: keycaps, rebuilt when the binding changes.
#[derive(Component)]
struct BindSlot {
    action: Action,
    row: Entity,
    shown: Option<String>,
}

/// The row's "clear binding" icon.
#[derive(Component, Clone, Copy)]
struct ClearBind(&'static [Action]);

/// Shown only while the mapping is for this device.
#[derive(Component, Clone, Copy)]
struct DeviceOnly(Device);

#[derive(Component, Clone, Copy)]
struct ListView(Tab);

#[derive(Component, Clone, Copy)]
struct ScrollThumb(Tab);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Desc {
    Title,
    Body,
    Default,
    Warn,
    WarnIcon,
}

#[derive(Component, Clone, Copy)]
struct DescPart(Tab, Desc);

#[derive(Component, Clone, Copy)]
struct KbKey(KeyCode);

#[derive(Component, Clone, Copy)]
struct KbLabel(KeyCode);

#[derive(Component, Clone, Copy)]
struct PadKey(GamepadButton);

#[derive(Component, Clone, Copy)]
struct PadLabel(GamepadButton);

#[derive(Component, Clone, Copy)]
struct MouseFill(MouseButton);

/// The Controls map's list of actions, rebuilt when the part, device or bindings change.
#[derive(Component, Default)]
struct Legend {
    shown: Option<String>,
}

#[derive(Component)]
struct ConfirmBox;

#[derive(Component)]
struct ConfirmText;

// ------------------------------------------------------------------------------------------------
// Building the pages

fn keycap(p: &mut ChildSpawnerCommands, label: &str, bg: Color) {
    p.spawn((
        Node {
            min_width: px(18.0),
            height: px(18.0),
            padding: UiRect::horizontal(px(4.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(bg),
        BorderRadius::all(px(3.0)),
    ))
    .with_child(text(label, F_CAP, KEYCAP_TEXT));
}

/// One input as Rematch shows it: a keycap, a mouse with the button lit, a gamepad button.
fn spawn_input(p: &mut ChildSpawnerCommands, input: &Input, art: &MenuArt, row: Option<Entity>) {
    match input {
        Input::Mouse(m) if mouse_index(*m).is_some() => {
            let image = art.mouse[mouse_index(*m).unwrap_or(0) + 1].clone();
            let mut glyph = p.spawn((ImageNode::new(image).with_color(TEXT), Node { width: px(13.0), height: px(19.0), flex_shrink: 0.0, ..default() }));
            if let Some(row) = row {
                glyph.insert(Tint { row, ink: Ink::Glyph });
            }
        }
        Input::Pad(b @ (GamepadButton::South | GamepadButton::East | GamepadButton::West | GamepadButton::North)) => {
            p.spawn((
                Node { width: px(17.0), height: px(17.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_shrink: 0.0, ..default() },
                BackgroundColor(KEYCAP),
                BorderRadius::MAX,
            ))
            .with_child(text(input_name(&Input::Pad(*b)), F_CAP, KEYCAP_TEXT));
        }
        Input::Modifier => keycap(p, "MODIFIER", ACCENT),
        other => keycap(p, &input_name(other), KEYCAP),
    }
}

fn spawn_chord(p: &mut ChildSpawnerCommands, chord: &[Input], art: &MenuArt, row: Option<Entity>) {
    for (i, input) in chord.iter().enumerate() {
        if i > 0 {
            let mut plus = p.spawn(text("+", F_CAP, TEXT_DIM));
            if let Some(row) = row {
                plus.insert(Tint { row, ink: Ink::Plus });
            }
        }
        spawn_input(p, input, art, row);
    }
}

fn bar(p: &mut ChildSpawnerCommands, height: f32, contents: impl FnOnce(&mut ChildSpawnerCommands)) {
    p.spawn((Node { width: Val::Percent(100.0), height: px(height), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }, BackgroundColor(BAR)))
        .with_children(contents);
}

/// A tab: its label on a slanted green background when it's open.
fn tab_button(p: &mut ChildSpawnerCommands, kind: MenuButton, label: &str, art: &MenuArt) {
    let mut b = p.spawn((Button, kind, Node { height: Val::Percent(100.0), padding: UiRect::horizontal(px(16.0)), align_items: AlignItems::Center, ..default() }));
    let id = b.id();
    b.with_children(|b| {
        b.spawn((Skin(id), sliced(art.slant.clone(), 14.0, 14.0), fill()));
        b.spawn((Skin(id), text(label, F_TAB, TEXT)));
    });
}

/// A keycap that goes with a key the menu answers to.
fn key_hint(p: &mut ChildSpawnerCommands, key: &str, margin: UiRect) {
    p.spawn(Node { margin, ..default() }).with_children(|k| keycap(k, key, KEYCAP));
}

/// The bottom bar's links: a keycap and a label.
fn footer(p: &mut ChildSpawnerCommands, links: &[(MenuButton, &str, &str)]) {
    bar(p, FOOT_H, |f| {
        f.spawn(Node { flex_grow: 1.0, ..default() });
        for &(kind, key, label) in links {
            let mut b = f.spawn((Button, kind, Node { column_gap: px(5.0), align_items: AlignItems::Center, margin: UiRect::right(px(16.0)), ..default() }));
            let id = b.id();
            b.with_children(|b| {
                keycap(b, key, KEYCAP);
                b.spawn((Skin(id), text(label, 15.0, TEXT)));
            });
        }
        f.spawn(Node { width: px(25.0), ..default() });
    });
}

/// An item of a vertical menu (pause page, dialog): the green bar under the mouse.
fn menu_item(p: &mut ChildSpawnerCommands, kind: MenuButton, label: &str, width: Val, art: &MenuArt) {
    let mut b = p.spawn((Button, kind, Node { width, height: px(33.0), padding: UiRect::left(px(10.0)), align_items: AlignItems::Center, ..default() }));
    let id = b.id();
    b.with_children(|b| {
        b.spawn((Skin(id), sliced(art.bar.clone(), 0.0, 12.0), fill()));
        b.spawn((Skin(id), text(label, 16.0, TEXT)));
    });
}

pub fn spawn_menu(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let art = MenuArt::new(&mut images);
    let page = |p: Page| {
        (PageNode(p), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, display: Display::None, ..default() })
    };
    commands
        .spawn((
            MenuRoot,
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() },
            BackgroundColor(Color::NONE),
            GlobalZIndex(100),
        ))
        .with_children(|root| {
            // The pause page, like Rematch's system menu; the game runs on behind it.
            root.spawn(page(Page::Pause)).with_children(|p| {
                bar(p, BAR_H, |_| {});
                p.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, padding: UiRect::new(px(67.0), px(0.0), px(138.0), px(0.0)), ..default() })
                    .with_children(|list| {
                        menu_item(list, MenuButton::Options, "OPTIONS", px(231.0), &art);
                        menu_item(list, MenuButton::Leave, "LEAVE", px(231.0), &art);
                    });
                footer(p, &[(MenuButton::Resume, "ESC", "Resume")]);
            });
            root.spawn(page(Page::Settings)).with_children(|p| spawn_settings(p, &art));
            spawn_confirm(root, &art);
        });
    commands.insert_resource(art);
}

fn spawn_settings(p: &mut ChildSpawnerCommands, art: &MenuArt) {
    bar(p, BAR_H, |b| {
        b.spawn(Node { width: px(66.0), ..default() });
        key_hint(b, "Q", UiRect::right(px(4.0)));
        for tab in Tab::ALL {
            tab_button(b, MenuButton::Tab(tab), tab.label(), art);
        }
        key_hint(b, "E", UiRect::left(px(4.0)));
    });
    p.spawn(Node { flex_grow: 1.0, width: Val::Percent(100.0), ..default() }).with_children(|body| {
        for tab in Tab::ALL {
            body.spawn((TabBody(tab), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() })).with_children(|t| {
                t.spawn((Node { width: Val::Percent(50.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(LEFT_PANEL)))
                    .with_children(|left| spawn_list(left, tab, art));
                // A long description is cut at the bottom, as in Rematch.
                t.spawn((Node { width: Val::Percent(50.0), height: Val::Percent(100.0), overflow: Overflow::clip(), ..default() }, BackgroundColor(RIGHT_PANEL))).with_children(|right| {
                    if tab == Tab::Controls {
                        spawn_map(right, art);
                        right
                            .spawn(Node { position_type: PositionType::Absolute, left: px(43.0), right: px(30.0), top: px(528.0), flex_direction: FlexDirection::Column, ..default() })
                            .with_children(|d| spawn_desc(d, tab, 12.0, art));
                    } else {
                        right
                            .spawn(Node { position_type: PositionType::Absolute, left: px(43.0), width: px(420.0), top: px(52.0), flex_direction: FlexDirection::Column, ..default() })
                            .with_children(|d| spawn_desc(d, tab, 14.0, art));
                    }
                });
            });
        }
    });
    footer(p, &[(MenuButton::Back, "ESC", "Back"), (MenuButton::Reset, "L", "Reset to default")]);
}

/// The description of the option under the mouse: name, what it does, default, and a warning.
fn spawn_desc(d: &mut ChildSpawnerCommands, tab: Tab, size: f32, art: &MenuArt) {
    d.spawn((DescPart(tab, Desc::Title), text("", size + 1.0, TEXT)));
    d.spawn((DescPart(tab, Desc::Body), prose("", size, TEXT), Node { margin: UiRect::top(px(3.0)), ..default() }));
    d.spawn((DescPart(tab, Desc::Default), text("", size, TEXT), Node { margin: UiRect::top(px(6.0)), ..default() }));
    d.spawn(Node { margin: UiRect::new(px(10.0), px(0.0), px(6.0), px(0.0)), column_gap: px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|w| {
        w.spawn((DescPart(tab, Desc::WarnIcon), ImageNode::new(art.warn.clone()).with_color(WARN), Node { width: px(15.0), height: px(13.0), ..default() }));
        w.spawn((DescPart(tab, Desc::Warn), text("", size, WARN)));
    });
}

/// The tab's options: a scrolling list in sections, with its scrollbar.
fn spawn_list(left: &mut ChildSpawnerCommands, tab: Tab, art: &MenuArt) {
    left.spawn(Node {
        position_type: PositionType::Absolute,
        top: px(LIST_TOP),
        bottom: px(LIST_BOTTOM),
        left: px(LIST_LEFT),
        column_gap: px(7.0),
        ..default()
    })
    .with_children(|area| {
        area.spawn((
            ListView(tab),
            Node {
                width: px(LIST_W),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(ROW_GAP),
                overflow: Overflow::scroll_y(),
                padding: UiRect::bottom(px(12.0)),
                ..default()
            },
        ))
        .with_children(|list| {
            let mut index = 0;
            for (i, section) in sections(tab).into_iter().enumerate() {
                list.spawn((
                    text(section.title, F_SECTION, if section.green { ACCENT } else { TEXT }),
                    Node { margin: UiRect::top(px(if i == 0 { 0.0 } else { SECTION_GAP })), padding: UiRect::bottom(px(2.0)), flex_shrink: 0.0, ..default() },
                ));
                for def in section.rows {
                    spawn_row(list, tab, index, def, art);
                    index += 1;
                }
            }
        });
        area.spawn((Node { width: px(4.0), height: Val::Percent(100.0), flex_shrink: 0.0, ..default() }, BackgroundColor(TRACK)))
            .with_child((ScrollThumb(tab), Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), ..default() }, BackgroundColor(ACCENT)));
    });
}

fn spawn_row(list: &mut ChildSpawnerCommands, tab: Tab, index: usize, def: RowDef, art: &MenuArt) {
    let soon = def.soon();
    let mut entity = list.spawn((Row { def, tab, index }, Interaction::default(), Node { width: Val::Percent(100.0), min_height: px(ROW_H), flex_shrink: 0.0, ..default() }));
    let row = entity.id();
    entity.with_children(|r| {
        r.spawn((
            Tint { row, ink: Ink::LabelPart },
            Node { width: px(LABEL_W), flex_shrink: 0.0, padding: UiRect::new(px(12.0), px(6.0), px(4.0), px(4.0)), align_items: AlignItems::Center, ..default() },
        ))
        .with_child((Tint { row, ink: Ink::Label }, text(def.label(), F_ROW, if soon { TEXT_DIM } else { TEXT })));
        r.spawn((Tint { row, ink: Ink::ControlPart }, Node { flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }))
            .with_children(|c| match def.control {
                Control::Slider(setting) => {
                    c.spawn((Tint { row, ink: Ink::Value }, SliderValue(setting), text("", F_ROW, ACCENT), TextLayout::new_with_justify(JustifyText::Center), Node { width: px(VALUE_W), ..default() }));
                    spawn_track(c, row, Some(setting), 0.0, art);
                }
                Control::Soon(Soon::Slider { value, lo, hi }) => {
                    c.spawn((Tint { row, ink: Ink::Value }, text(number(value), F_ROW, TEXT_DIM), TextLayout::new_with_justify(JustifyText::Center), Node { width: px(VALUE_W), ..default() }));
                    spawn_track(c, row, None, ratio(value, (lo, hi)), art);
                }
                Control::Choice(choice) => spawn_choice(c, row, Some(choice), "", (0, 0), art),
                Control::Soon(Soon::Choice { value, at, of }) => spawn_choice(c, row, None, value, (of, at), art),
                Control::Soon(Soon::Key(key)) => {
                    c.spawn((DeviceOnly(Device::Keyboard), Node::default())).with_children(|k| {
                        if !key.is_empty() {
                            keycap(k, key, KEYCAP_DIM);
                        }
                    });
                }
                Control::Bind(actions) => {
                    c.spawn(Node { column_gap: px(5.0), align_items: AlignItems::Center, ..default() }).with_children(|b| {
                        if actions == MOVE {
                            b.spawn((DeviceOnly(Device::Gamepad), Node::default())).with_children(|k| keycap(k, "LEFT STICK", KEYCAP));
                        }
                        for &action in actions {
                            b.spawn((
                                BindSlot { action, row, shown: None },
                                Interaction::default(),
                                Node { min_width: px(17.0), min_height: px(21.0), column_gap: px(3.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                            ));
                        }
                    });
                    // Under the mouse: clear the binding, or edit it (a click anywhere on the row).
                    c.spawn(Node { position_type: PositionType::Absolute, right: px(10.0), column_gap: px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|i| {
                        let icon = Node { width: px(15.0), height: px(15.0), ..default() };
                        i.spawn((ClearBind(actions), Interaction::default(), Tint { row, ink: Ink::OnHover }, ImageNode::new(art.clear.clone()).with_color(INK), icon.clone(), Visibility::Hidden));
                        i.spawn((Tint { row, ink: Ink::OnHover }, ImageNode::new(art.pencil.clone()).with_color(INK), icon, Visibility::Hidden));
                    });
                }
            });
    });
}

/// A slider: a thin line with a diamond on the value; `setting` none for a "(soon)" one.
fn spawn_track(c: &mut ChildSpawnerCommands, row: Entity, setting: Option<Setting>, at: f32, art: &MenuArt) {
    let mut track = c.spawn(Node { width: px(SLIDER_W), height: px(ROW_H), margin: UiRect::new(px(8.0), px(13.0), px(0.0), px(0.0)), flex_shrink: 0.0, ..default() });
    if let Some(setting) = setting {
        track.insert((SliderTrack(setting), Interaction::default(), RelativeCursorPosition::default()));
    }
    track.with_children(|t| {
        t.spawn((
            Tint { row, ink: Ink::Value },
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: px(2.0), top: px(ROW_H / 2.0 - 1.0), ..default() },
            BackgroundColor(ACCENT),
        ));
        let mut knob = t.spawn((
            Tint { row, ink: Ink::Value },
            ImageNode::new(art.diamond.clone()).with_color(ACCENT),
            Node {
                position_type: PositionType::Absolute,
                width: px(12.0),
                height: px(12.0),
                top: px(ROW_H / 2.0 - 6.0),
                left: Val::Percent(at * 100.0),
                margin: UiRect::left(px(-6.0)),
                ..default()
            },
        ));
        if let Some(setting) = setting {
            knob.insert(SliderKnob(setting));
        }
    });
}

/// A choice: its value over a row of dots, arrows either side under the mouse. `choice` none for
/// a "(soon)" one, shown at `value` with `dots` = (count, which).
fn spawn_choice(c: &mut ChildSpawnerCommands, row: Entity, choice: Option<Choice>, value: &str, dots: (usize, usize), art: &MenuArt) {
    let mut control = c.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(px(8.0)),
        ..default()
    });
    if let Some(choice) = choice {
        control.insert((ChoiceControl(choice), Interaction::default(), RelativeCursorPosition::default()));
    }
    control.with_children(|cc| {
        let arrow = |cc: &mut ChildSpawnerCommands, image: &Handle<Image>| {
            cc.spawn((Tint { row, ink: Ink::OnHover }, ImageNode::new(image.clone()).with_color(INK), Node { width: px(8.0), height: px(11.0), ..default() }, Visibility::Hidden));
        };
        arrow(cc, &art.arrow_left);
        cc.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(3.0), ..default() }).with_children(|v| {
            let mut label = v.spawn((Tint { row, ink: Ink::Value }, text(value, F_ROW, if choice.is_some() { ACCENT } else { TEXT_DIM })));
            if let Some(choice) = choice {
                label.insert(ChoiceText(choice));
            }
            let mut dot_row = v.spawn(Node { column_gap: px(5.0), height: px(7.0), align_items: AlignItems::Center, ..default() });
            match choice {
                Some(choice) => {
                    dot_row.insert(ChoiceDots { choice, row, shown: None });
                }
                None => {
                    dot_row.with_children(|d| spawn_dots(d, dots.0, dots.1, row, art));
                }
            }
        });
        arrow(cc, &art.arrow_right);
    });
}

fn spawn_dots(d: &mut ChildSpawnerCommands, count: usize, at: usize, row: Entity, art: &MenuArt) {
    for i in 0..count {
        let (image, size) = if i == at { (art.diamond.clone(), 7.0) } else { (art.dot.clone(), 5.0) };
        d.spawn((Tint { row, ink: Ink::Value }, ImageNode::new(image).with_color(ACCENT), Node { width: px(size), height: px(size), ..default() }));
    }
}

/// Rematch's keyboard for the map, in key widths: keys light up when the shown part uses them.
const KEYBOARD: [&[(KeyCode, f32)]; 5] = {
    use KeyCode::*;
    [
        &[
            (Backquote, 1.0),
            (Digit1, 1.0),
            (Digit2, 1.0),
            (Digit3, 1.0),
            (Digit4, 1.0),
            (Digit5, 1.0),
            (Digit6, 1.0),
            (Digit7, 1.0),
            (Digit8, 1.0),
            (Digit9, 1.0),
            (Digit0, 1.0),
            (Minus, 1.0),
            (Equal, 1.0),
            (Backspace, 2.0),
        ],
        &[
            (Tab, 1.5),
            (KeyQ, 1.0),
            (KeyW, 1.0),
            (KeyE, 1.0),
            (KeyR, 1.0),
            (KeyT, 1.0),
            (KeyY, 1.0),
            (KeyU, 1.0),
            (KeyI, 1.0),
            (KeyO, 1.0),
            (KeyP, 1.0),
            (BracketLeft, 1.0),
            (BracketRight, 1.0),
            (Backslash, 1.5),
        ],
        &[
            (CapsLock, 1.75),
            (KeyA, 1.0),
            (KeyS, 1.0),
            (KeyD, 1.0),
            (KeyF, 1.0),
            (KeyG, 1.0),
            (KeyH, 1.0),
            (KeyJ, 1.0),
            (KeyK, 1.0),
            (KeyL, 1.0),
            (Semicolon, 1.0),
            (Quote, 1.0),
            (Enter, 2.25),
        ],
        &[
            (ShiftLeft, 2.25),
            (KeyZ, 1.0),
            (KeyX, 1.0),
            (KeyC, 1.0),
            (KeyV, 1.0),
            (KeyB, 1.0),
            (KeyN, 1.0),
            (KeyM, 1.0),
            (Comma, 1.0),
            (Period, 1.0),
            (Slash, 1.0),
            (ShiftRight, 2.75),
        ],
        &[(ControlLeft, 1.25), (SuperLeft, 1.25), (AltLeft, 1.25), (Space, 6.25), (AltRight, 1.25), (SuperRight, 1.25), (ContextMenu, 1.25), (ControlRight, 1.25)],
    ]
};
const KEY_UNIT: f32 = 22.6;
const KEY_H: f32 = 20.0;
const KEY_GAP: f32 = 3.0;

/// The gamepad for the map: button, left, top, width, height, label.
const PAD: [(GamepadButton, f32, f32, f32, f32, &str); 16] = {
    use GamepadButton::*;
    [
        (LeftTrigger2, 70.0, 0.0, 44.0, 14.0, "LT"),
        (LeftTrigger, 70.0, 17.0, 54.0, 14.0, "LB"),
        (RightTrigger2, 246.0, 0.0, 44.0, 14.0, "RT"),
        (RightTrigger, 236.0, 17.0, 54.0, 14.0, "RB"),
        (LeftThumb, 78.0, 46.0, 30.0, 30.0, "LS"),
        (DPadUp, 126.0, 74.0, 13.0, 13.0, ""),
        (DPadLeft, 113.0, 87.0, 13.0, 13.0, ""),
        (DPadRight, 139.0, 87.0, 13.0, 13.0, ""),
        (DPadDown, 126.0, 100.0, 13.0, 13.0, ""),
        (Select, 152.0, 50.0, 18.0, 11.0, ""),
        (Start, 192.0, 50.0, 18.0, 11.0, ""),
        (RightThumb, 206.0, 80.0, 30.0, 30.0, "RS"),
        (North, 258.0, 40.0, 17.0, 17.0, "Y"),
        (West, 240.0, 57.0, 17.0, 17.0, "X"),
        (East, 276.0, 57.0, 17.0, 17.0, "B"),
        (South, 258.0, 74.0, 17.0, 17.0, "A"),
    ]
};

/// The Controls tab's right side: the parts of the game, a keyboard and mouse or gamepad map,
/// and the list of their actions.
fn spawn_map(right: &mut ChildSpawnerCommands, art: &MenuArt) {
    right
        .spawn(Node { position_type: PositionType::Absolute, top: px(5.0), left: px(33.0), height: px(BAR_H), align_items: AlignItems::Center, ..default() })
        .with_children(|b| {
            key_hint(b, "A", UiRect::right(px(2.0)));
            for part in Part::ALL {
                tab_button(b, MenuButton::Part(part), part.label(), art);
            }
            key_hint(b, "D", UiRect::left(px(2.0)));
        });
    let at = |top: f32, left: f32, width: f32, height: f32| Node {
        position_type: PositionType::Absolute,
        top: px(top),
        left: px(left),
        width: px(width),
        height: px(height),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    right.spawn((DeviceOnly(Device::Keyboard), at(74.0, 93.0, 430.0, 120.0))).with_children(|kb| {
        for (r, keys) in KEYBOARD.iter().enumerate() {
            let mut x = 0.0;
            for &(key, w) in keys.iter() {
                kb.spawn((
                    KbKey(key),
                    Node { border: UiRect::all(px(1.0)), ..at(r as f32 * (KEY_H + KEY_GAP), x * KEY_UNIT, w * KEY_UNIT - KEY_GAP, KEY_H) },
                    BorderColor(MAP_LINE),
                    BorderRadius::all(px(2.0)),
                ))
                .with_child((KbLabel(key), text(input_name(&Input::Key(key)), 7.0, INK), Visibility::Hidden));
                x += w;
            }
        }
        for (key, top, left) in [(KeyCode::ArrowUp, 72.0, 374.0), (KeyCode::ArrowLeft, 95.0, 351.0), (KeyCode::ArrowDown, 95.0, 374.0), (KeyCode::ArrowRight, 95.0, 397.0)] {
            kb.spawn((KbKey(key), Node { border: UiRect::all(px(1.0)), ..at(top, left, KEY_H, KEY_H) }, BorderColor(MAP_LINE), BorderRadius::all(px(2.0))))
                .with_child((KbLabel(key), text(input_name(&Input::Key(key)), 6.0, INK), Visibility::Hidden));
        }
        kb.spawn((ImageNode::new(art.mouse_map.clone()).with_color(MAP_LINE), at(-10.0, 359.0, 50.0, 70.0)));
        for (i, button) in MOUSE_BUTTONS.into_iter().enumerate() {
            kb.spawn((MouseFill(button), ImageNode::new(art.mouse_fill[i].clone()).with_color(ACCENT), at(-10.0, 359.0, 50.0, 70.0), Visibility::Hidden));
        }
    });
    right.spawn((DeviceOnly(Device::Gamepad), at(74.0, 93.0, 360.0, 120.0))).with_children(|pad| {
        pad.spawn((Node { border: UiRect::all(px(2.0)), ..at(34.0, 50.0, 260.0, 84.0) }, BorderColor(MAP_LINE), BorderRadius::all(px(38.0))));
        for (button, left, top, w, h, label) in PAD {
            let round = matches!(button, GamepadButton::LeftThumb | GamepadButton::RightThumb | GamepadButton::North | GamepadButton::South | GamepadButton::East | GamepadButton::West);
            pad.spawn((
                PadKey(button),
                Node { border: UiRect::all(px(1.0)), ..at(top, left, w, h) },
                BorderColor(MAP_LINE),
                BackgroundColor(Color::NONE),
                if round { BorderRadius::MAX } else { BorderRadius::all(px(3.0)) },
            ))
            .with_child((PadLabel(button), text(label, 8.0, TEXT)));
        }
    });
    right.spawn((
        Legend::default(),
        Node { position_type: PositionType::Absolute, top: px(205.0), left: px(37.0), right: px(20.0), column_gap: px(14.0), ..default() },
    ));
}

/// The dialog that asks before a reset or a preset over a custom mapping.
fn spawn_confirm(root: &mut ChildSpawnerCommands, art: &MenuArt) {
    root.spawn((
        ConfirmBox,
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, display: Display::None, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        // Nothing under the dialog takes the mouse while it's open.
        FocusPolicy::Block,
        ZIndex(10),
    ))
    .with_children(|shade| {
        shade
            .spawn((Node { width: px(290.0), flex_direction: FlexDirection::Column, row_gap: px(6.0), padding: UiRect::all(px(22.0)), ..default() }, BackgroundColor(DIALOG)))
            .with_children(|d| {
                d.spawn((ImageNode::new(art.corner.clone()).with_color(ACCENT), Node { position_type: PositionType::Absolute, left: px(-10.0), top: px(-10.0), width: px(26.0), height: px(26.0), ..default() }));
                d.spawn((
                    ImageNode { flip_x: true, flip_y: true, ..ImageNode::new(art.corner.clone()).with_color(ACCENT) },
                    Node { position_type: PositionType::Absolute, right: px(-6.0), bottom: px(-6.0), width: px(12.0), height: px(12.0), ..default() },
                ));
                d.spawn(text("WARNING", 14.0, TEXT));
                d.spawn((ConfirmText, prose("", 13.0, TEXT), Node { margin: UiRect::bottom(px(12.0)), ..default() }));
                menu_item(d, MenuButton::Confirm, "CONFIRM", Val::Percent(100.0), art);
                menu_item(d, MenuButton::Cancel, "CANCEL", Val::Percent(100.0), art);
            });
    });
}

// ------------------------------------------------------------------------------------------------
// Systems

/// The menu's systems after `menu_keys`, in order: input first, then what's drawn.
pub fn update_systems() -> ScheduleConfigs<ScheduleSystem> {
    (
        // Capture before clicks, so the click that opens a capture isn't bound.
        capture_binding,
        menu_clicks,
        drag_sliders,
        scroll_lists,
        refresh_binds,
        refresh_values,
        light_rows,
        describe,
        refresh_chrome,
        refresh_map,
        scale_fonts,
    )
        .chain()
}

/// Esc, F1 and the tab keys. Runs first, so gameplay ignores input from the frame the menu opens.
pub fn menu_keys(keys: Res<ButtonInput<KeyCode>>, mut menu: ResMut<Menu>, mut blocked: ResMut<InputBlocked>, actions: Res<Actions>) {
    // While a binding is being captured, Esc cancels the capture (capture_binding) and the other
    // keys are for binding.
    if menu.capturing.is_none() {
        let was_open = menu.open();
        if keys.just_pressed(KeyCode::Escape) {
            menu.escape();
        } else if keys.just_pressed(KeyCode::F1) {
            menu.f1();
        } else if menu.page == Some(Page::Settings) && menu.confirm.is_none() {
            let step = |back, on| keys.just_pressed(on) as i32 - keys.just_pressed(back) as i32;
            let tab = step(KeyCode::KeyQ, KeyCode::KeyE);
            if tab != 0 {
                menu.tab = menu.tab.step(tab);
            }
            let part = step(KeyCode::KeyA, KeyCode::KeyD);
            if part != 0 && menu.tab == Tab::Controls {
                menu.part = menu.part.step(part);
            }
            if keys.just_pressed(KeyCode::KeyL) {
                menu.ask_reset();
            }
        }
        // Rematch shows the mapping of the device in use.
        if !was_open && menu.open() {
            menu.device = actions.device;
        }
    }
    blocked.0 = menu.open();
}

/// Shows the open page and tab, and frees the cursor while the menu is open. It runs after
/// `grab_cursor` (which frees the cursor on Esc), so closing the menu takes the cursor back: you're
/// straight back in the game, without a click that would also shoot. Like Rematch's, the menu
/// hides the HUD (the other UI) while it's open.
pub fn show_menu(
    menu: Res<Menu>,
    mut was_open: Local<bool>,
    mut root: Single<(&mut Node, &mut BackgroundColor), With<MenuRoot>>,
    mut pages: Query<(&PageNode, &mut Node), Without<MenuRoot>>,
    mut tabs: Query<(&TabBody, &mut Node), (Without<MenuRoot>, Without<PageNode>)>,
    mut hud: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>, Without<MenuRoot>)>,
    mut hidden: Local<HashMap<Entity, Visibility>>,
    window: Option<Single<&mut Window, With<PrimaryWindow>>>,
) {
    if !menu.is_changed() {
        return;
    }
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    root.0.display = display(menu.open());
    // The pause page dims the game; the settings' panels cover it themselves.
    root.1.0 = if menu.page == Some(Page::Pause) { PAUSE_SHADE } else { Color::NONE };
    for (PageNode(page), mut node) in &mut pages {
        node.display = display(menu.page == Some(*page));
    }
    for (TabBody(tab), mut node) in &mut tabs {
        node.display = display(*tab == menu.tab);
    }
    if menu.open() != *was_open {
        *was_open = menu.open();
        for (entity, mut visibility) in &mut hud {
            if menu.open() {
                hidden.insert(entity, *visibility);
                *visibility = Visibility::Hidden;
            } else if let Some(was) = hidden.remove(&entity) {
                *visibility = was;
            }
        }
        if let Some(mut window) = window {
            window.cursor_options.grab_mode = if menu.open() { CursorGrabMode::None } else { CursorGrabMode::Locked };
            window.cursor_options.visible = menu.open();
        }
    }
}

fn menu_clicks(
    mut menu: ResMut<Menu>,
    mut bindings: ResMut<Bindings>,
    mut cheats: ResMut<Cheats>,
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    choices: Query<(&Interaction, &ChoiceControl, &RelativeCursorPosition), Changed<Interaction>>,
    clears: Query<(&Interaction, &ClearBind, &Tint), Changed<Interaction>>,
    slots: Query<(&Interaction, &BindSlot), Changed<Interaction>>,
    rows: Query<(Entity, &Interaction, &Row), Changed<Interaction>>,
    mut exit: EventWriter<AppExit>,
) {
    // While capturing, clicks are inputs to bind, not menu clicks.
    if !menu.open() || menu.capturing.is_some() {
        return;
    }
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    let mut changed = false;
    for (_, button) in buttons.iter().filter(|(i, _)| pressed(i)) {
        match *button {
            MenuButton::Confirm => {
                if let Some(question) = menu.confirm.take() {
                    *bindings = match question {
                        Confirm::Preset(device, preset) => apply_preset(&bindings, device, preset),
                        Confirm::Reset(tab) => reset_tab(&bindings, tab, &mut cheats),
                    };
                    changed = true;
                }
            }
            MenuButton::Cancel => menu.confirm = None,
            _ if menu.confirm.is_some() => {}
            MenuButton::Options => menu.page = Some(Page::Settings),
            MenuButton::Leave => {
                exit.write(AppExit::Success);
            }
            MenuButton::Resume => menu.page = None,
            MenuButton::Back => menu.page = Some(Page::Pause),
            MenuButton::Reset => menu.ask_reset(),
            MenuButton::Tab(tab) => menu.tab = tab,
            MenuButton::Part(part) => menu.part = part,
        }
    }
    if menu.confirm.is_some() {
        return;
    }
    // The left end of a choice steps back, the rest forward.
    for (_, ChoiceControl(choice), cursor) in choices.iter().filter(|(i, ..)| pressed(i)) {
        let by = if cursor.normalized.is_some_and(|p| p.x < 0.25) { -1 } else { 1 };
        changed |= step_choice(*choice, by, &mut bindings, &mut cheats, &mut menu);
    }
    // A binding row: the clear icon clears it, a key starts capturing that key's action, anywhere
    // else on the row its first action.
    let mut done: HashSet<Entity> = HashSet::new();
    for (_, ClearBind(actions), tint) in clears.iter().filter(|(i, ..)| pressed(i)) {
        let device = menu.device;
        for action in *actions {
            bindings.get_mut(device).remove(action);
        }
        changed = true;
        done.insert(tint.row);
    }
    for (_, slot) in slots.iter().filter(|(i, _)| pressed(i)) {
        if done.insert(slot.row) {
            menu.start_capture(slot.action, slot.row);
        }
    }
    for (entity, _, row) in rows.iter().filter(|(_, i, _)| pressed(i)) {
        if let Control::Bind(actions) = row.def.control
            && done.insert(entity)
        {
            menu.start_capture(actions[0], entity);
        }
    }
    if changed {
        bindings.save();
    }
}

/// Dragging on a slider sets its option; saved on release.
fn drag_sliders(menu: Res<Menu>, mouse: Res<ButtonInput<MouseButton>>, mut bindings: ResMut<Bindings>, sliders: Query<(&Interaction, &RelativeCursorPosition, &SliderTrack)>) {
    if !menu.open() || menu.capturing.is_some() || menu.confirm.is_some() {
        return;
    }
    for (interaction, cursor, SliderTrack(setting)) in &sliders {
        if *interaction == Interaction::Pressed
            && let Some(pos) = cursor.normalized
        {
            let v = setting.at_ratio(pos.x);
            if v != setting.value(&bindings) {
                setting.set(&mut bindings, v);
            }
        }
    }
    if mouse.just_released(MouseButton::Left) && bindings.is_changed() {
        bindings.save();
    }
}

/// Collects the new chord: inputs pressed after the click, committed once all are released.
fn capture_binding(mut menu: ResMut<Menu>, mut bindings: ResMut<Bindings>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, pads: Query<&Gamepad>) {
    let Some((action, device, chord)) = &mut menu.capturing else { return };
    if keys.just_pressed(KeyCode::Escape) {
        menu.capturing = None;
        menu.capture_row = None;
        return;
    }
    let mut fresh: Vec<Input> = Vec::new();
    match device {
        Device::Keyboard => {
            fresh.extend(keys.get_just_pressed().filter(|k| **k != KeyCode::F1).map(|k| Input::Key(*k)));
            // The click that opened the capture is still held; only count new presses.
            fresh.extend(mouse.get_just_pressed().map(|m| Input::Mouse(*m)));
        }
        Device::Gamepad => {
            for pad in &pads {
                fresh.extend(pad.get_just_pressed().map(|b| Input::Pad(*b)));
            }
        }
    }
    for input in fresh {
        if !chord.contains(&input) {
            chord.push(input);
        }
    }
    let still_held = chord.iter().any(|i| match i {
        Input::Key(k) => keys.pressed(*k),
        Input::Mouse(m) => mouse.pressed(*m),
        Input::Pad(b) => pads.iter().any(|g| g.pressed(*b)),
        Input::Modifier => false,
    });
    if !chord.is_empty() && !still_held {
        let (action, device, mut chord) = (*action, *device, chord.clone());
        // Pressing the modifier's key inside another binding stores "Modifier", not the raw key.
        let modifier_key = match bindings.get_mut(device).get(&Action::Modifier).map(Vec::as_slice) {
            Some([single]) => Some(*single),
            _ => None,
        };
        if action != Action::Modifier
            && let Some(m) = modifier_key
        {
            chord.iter_mut().filter(|i| **i == m).for_each(|i| *i = Input::Modifier);
        }
        bindings.get_mut(device).insert(action, chord);
        bindings.save();
        menu.capturing = None;
        menu.capture_row = None;
    }
}

/// The mouse wheel scrolls the open list; the scrollbar follows.
fn scroll_lists(
    mut menu: ResMut<Menu>,
    wheel: Res<AccumulatedMouseScroll>,
    mut lists: Query<(&ListView, &mut ScrollPosition, &ComputedNode)>,
    mut thumbs: Query<(&ScrollThumb, &mut Node)>,
) {
    if menu.page != Some(Page::Settings) {
        return;
    }
    let free = menu.confirm.is_none() && menu.capturing.is_none();
    let wheel_px = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y * (ROW_H + ROW_GAP) * 1.5,
        MouseScrollUnit::Pixel => wheel.delta.y,
    };
    for (ListView(tab), mut pos, node) in &mut lists {
        if *tab != menu.tab {
            continue;
        }
        let (view, content) = (node.size.y * node.inverse_scale_factor, node.content_size.y * node.inverse_scale_factor);
        let mut y = pos.offset_y;
        if free {
            y -= wheel_px;
        }
        if content > 0.0
            && let Some(to) = menu.bypass_change_detection().scroll_to.take()
        {
            y = to;
        }
        let y = y.clamp(0.0, (content - view).max(0.0));
        if y != pos.offset_y {
            pos.offset_y = y;
        }
        let (top, height) = if content > view { (y / content, view / content) } else { (0.0, 1.0) };
        for (ScrollThumb(t), mut thumb) in &mut thumbs {
            if t == tab {
                thumb.top = Val::Percent(top * 100.0);
                thumb.height = Val::Percent(height * 100.0);
            }
        }
    }
}

/// Rebuilds the keycaps of bindings that changed, and shows what belongs to the device shown.
fn refresh_binds(
    menu: Res<Menu>,
    bindings: Res<Bindings>,
    art: Res<MenuArt>,
    mut commands: Commands,
    mut slots: Query<(Entity, &mut BindSlot, &mut Node)>,
    mut only: Query<(&DeviceOnly, &mut Node), Without<BindSlot>>,
) {
    if !menu.open() {
        return;
    }
    let device = menu.device;
    for (DeviceOnly(d), mut node) in &mut only {
        let display = if *d == device { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    for (entity, mut slot, mut node) in &mut slots {
        let chord = chord_of(&bindings, device, slot.action);
        let capture = match &menu.capturing {
            Some((a, d, so_far)) if *a == slot.action && *d == device && menu.capture_row.is_none_or(|r| r == slot.row) => Some(so_far.clone()),
            _ => None,
        };
        // A gamepad moves on the left stick: its four move slots only show once bound.
        let hidden = device == Device::Gamepad && MOVE.contains(&slot.action) && chord.is_empty() && capture.is_none();
        let display = if hidden { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
        let key = format!("{device:?} {chord:?} {capture:?}");
        if slot.shown.as_ref() == Some(&key) {
            continue;
        }
        slot.shown = Some(key);
        let row = slot.row;
        commands.entity(entity).despawn_related::<Children>().with_children(|p| match capture {
            Some(so_far) if so_far.is_empty() => {
                p.spawn((ImageNode::new(art.warn.clone()).with_color(WARN), Node { width: px(15.0), height: px(13.0), ..default() }));
                p.spawn(text("Set binding", F_ROW, WARN));
            }
            Some(so_far) => {
                spawn_chord(p, &so_far, &art, Some(row));
                p.spawn(text("...", F_ROW, WARN));
            }
            None if chord.is_empty() => {
                p.spawn((Tint { row, ink: Ink::Plus }, text("-", F_ROW, TEXT_DIM)));
            }
            None => spawn_chord(p, &chord, &art, Some(row)),
        });
    }
}

/// Slider knobs and values, choice values and dots.
fn refresh_values(
    menu: Res<Menu>,
    bindings: Res<Bindings>,
    cheats: Res<Cheats>,
    art: Res<MenuArt>,
    mut commands: Commands,
    mut knobs: Query<(&SliderKnob, &mut Node)>,
    mut values: Query<(&mut Text, Option<&SliderValue>, Option<&ChoiceText>), Or<(With<SliderValue>, With<ChoiceText>)>>,
    mut dots: Query<(Entity, &mut ChoiceDots)>,
) {
    if !menu.open() {
        return;
    }
    for (SliderKnob(setting), mut node) in &mut knobs {
        let left = Val::Percent(ratio(setting.value(&bindings), setting.range()) * 100.0);
        if node.left != left {
            node.left = left;
        }
    }
    for (mut text, slider, choice) in &mut values {
        let label = match (slider, choice) {
            (Some(SliderValue(setting)), _) => number(setting.value(&bindings)),
            (_, Some(ChoiceText(choice))) => {
                let (names, at) = choice.state(&bindings, &cheats, &menu);
                names[at].to_string()
            }
            _ => continue,
        };
        if text.0 != label {
            text.0 = label;
        }
    }
    for (entity, mut d) in &mut dots {
        let (names, at) = d.choice.state(&bindings, &cheats, &menu);
        if d.shown != Some((names.len(), at)) {
            d.shown = Some((names.len(), at));
            let row = d.row;
            commands.entity(entity).despawn_related::<Children>().with_children(|p| spawn_dots(p, names.len(), at, row, &art));
        }
    }
}

/// Lights the row under the mouse (or being captured) in Rematch's green, and dims "(soon)" rows.
fn light_rows(
    mut menu: ResMut<Menu>,
    rows: Query<(Entity, &Row, &Interaction)>,
    slots: Query<&BindSlot>,
    mut tints: Query<(&Tint, Option<&mut BackgroundColor>, Option<&mut TextColor>, Option<&mut ImageNode>, Option<&mut Visibility>, Has<ClearBind>)>,
) {
    if !menu.open() {
        return;
    }
    let capturing = menu.capturing.is_some();
    let mut lit: HashSet<Entity> = HashSet::new();
    if let Some((action, ..)) = &menu.capturing {
        match menu.capture_row {
            Some(row) => {
                lit.insert(row);
            }
            None => lit.extend(slots.iter().filter(|s| s.action == *action).map(|s| s.row)),
        }
    } else if menu.confirm.is_none() {
        for (entity, row, interaction) in &rows {
            if row.tab == menu.tab && (*interaction != Interaction::None || menu.lit == Some(row.index)) {
                lit.insert(entity);
            }
        }
    }
    if let Some(&row) = lit.iter().next()
        && menu.described != Some(row)
    {
        menu.bypass_change_detection().described = Some(row);
    }
    let soon: HashSet<Entity> = rows.iter().filter(|(_, r, _)| r.def.soon()).map(|(e, ..)| e).collect();
    for (tint, bg, color, image, visibility, clear) in &mut tints {
        let (on, dim) = (lit.contains(&tint.row), soon.contains(&tint.row));
        if tint.ink == Ink::OnHover
            && let Some(mut visibility) = visibility
        {
            // While capturing, Rematch keeps only the edit icon.
            let shown = on && !dim && !(clear && capturing);
            visibility.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
            continue;
        }
        let c = ink_color(tint.ink, on, dim);
        if let Some(mut image) = image {
            if image.color != c {
                image.color = c;
            }
        } else if let Some(mut color) = color {
            color.set_if_neq(TextColor(c));
        } else if let Some(mut bg) = bg {
            bg.set_if_neq(BackgroundColor(c));
        }
    }
}

/// The right side's description of the last row under the mouse (or the tab's first row).
fn describe(menu: Res<Menu>, rows: Query<(Entity, &Row)>, mut parts: Query<(&DescPart, Option<&mut Text>, Option<&mut Visibility>)>, mut shown: Local<Option<(Tab, Entity, Device)>>) {
    if !menu.open() {
        return;
    }
    let Some((entity, row)) = menu
        .described
        .and_then(|e| rows.get(e).ok())
        .filter(|(_, r)| r.tab == menu.tab)
        .or_else(|| rows.iter().find(|(_, r)| r.tab == menu.tab && r.index == 0))
    else {
        return;
    };
    if *shown == Some((menu.tab, entity, menu.device)) {
        return;
    }
    *shown = Some((menu.tab, entity, menu.device));
    let def = row.def;
    let warn = if def.soon() { NOT_YET } else { "" };
    for (DescPart(tab, part), text, visibility) in &mut parts {
        if *tab != menu.tab {
            continue;
        }
        let s = match part {
            Desc::Title => def.label.to_string(),
            Desc::Body => def.desc(),
            Desc::Default => format!("Default value: {}", def.default_value(menu.device)),
            Desc::Warn => warn.to_string(),
            Desc::WarnIcon => {
                if let Some(mut v) = visibility {
                    v.set_if_neq(if warn.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
                }
                continue;
            }
        };
        if let Some(mut text) = text {
            text.0 = s;
        }
    }
}

/// Tabs, footer links and menu items under the mouse or open; the confirm dialog.
fn refresh_chrome(
    menu: Res<Menu>,
    buttons: Query<(Entity, &MenuButton, &Interaction)>,
    mut skins: Query<(&Skin, Option<&mut ImageNode>, Option<&mut TextColor>)>,
    mut dialog: Single<&mut Node, With<ConfirmBox>>,
    mut dialog_text: Single<&mut Text, With<ConfirmText>>,
) {
    if !menu.open() {
        return;
    }
    let state: HashMap<Entity, (MenuButton, bool)> = buttons.iter().map(|(e, b, i)| (e, (*b, *i != Interaction::None))).collect();
    for (Skin(button), image, color) in &mut skins {
        let Some(&(kind, hover)) = state.get(button) else { continue };
        let open = match kind {
            MenuButton::Tab(tab) => menu.tab == tab,
            MenuButton::Part(part) => menu.part == part,
            _ => false,
        };
        let (bg, fg) = match kind {
            MenuButton::Tab(_) | MenuButton::Part(_) => (
                if open {
                    ACCENT
                } else if hover {
                    ACCENT_HOVER
                } else {
                    Color::NONE
                },
                if open { INK } else { TEXT },
            ),
            MenuButton::Back | MenuButton::Reset | MenuButton::Resume => (Color::NONE, if hover { ACCENT } else { TEXT }),
            _ => (if hover { ACCENT } else { Color::NONE }, if hover { INK } else { TEXT }),
        };
        if let Some(mut image) = image {
            if image.color != bg {
                image.color = bg;
            }
        }
        if let Some(mut color) = color {
            color.set_if_neq(TextColor(fg));
        }
    }
    let display = if menu.confirm.is_some() { Display::Flex } else { Display::None };
    if dialog.display != display {
        dialog.display = display;
    }
    if let Some(question) = menu.confirm {
        let s = question.text();
        if dialog_text.0 != s {
            dialog_text.0 = s;
        }
    }
}

/// The Controls map: the keys, mouse buttons or gamepad buttons the shown part uses, and the list
/// of its actions.
fn refresh_map(
    menu: Res<Menu>,
    bindings: Res<Bindings>,
    art: Res<MenuArt>,
    mut commands: Commands,
    mut legend_root: Single<(Entity, &mut Legend)>,
    mut keys: Query<(&KbKey, &mut BackgroundColor)>,
    mut key_labels: Query<(&KbLabel, &mut Visibility)>,
    mut pad_keys: Query<(&PadKey, &mut BackgroundColor), Without<KbKey>>,
    mut pad_labels: Query<(&PadLabel, &mut TextColor)>,
    mut fills: Query<(&MouseFill, &mut Visibility), Without<KbLabel>>,
) {
    if !menu.open() || menu.tab != Tab::Controls {
        return;
    }
    let (device, part) = (menu.device, menu.part);
    let key = format!("{part:?} {device:?} {:?}", chords(&bindings, device));
    let (entity, shown) = &mut *legend_root;
    if shown.shown.as_ref() == Some(&key) {
        return;
    }
    shown.shown = Some(key);
    let columns = legend(part);
    // Every input the part's actions use, the modifier's own key for "Modifier".
    let mut used: HashSet<Input> = HashSet::new();
    for item in columns.iter().flat_map(|c| c.iter()).flat_map(|g| g.items.iter()) {
        if let Item::Bind(_, actions) = item {
            for input in actions.iter().flat_map(|a| chord_of(&bindings, device, *a)) {
                match input {
                    Input::Modifier => used.extend(chord_of(&bindings, device, Action::Modifier)),
                    other => {
                        used.insert(other);
                    }
                }
            }
        }
    }
    for (KbKey(k), mut bg) in &mut keys {
        bg.set_if_neq(BackgroundColor(if used.contains(&Input::Key(*k)) { ACCENT } else { Color::NONE }));
    }
    for (KbLabel(k), mut visibility) in &mut key_labels {
        visibility.set_if_neq(if used.contains(&Input::Key(*k)) { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (MouseFill(b), mut visibility) in &mut fills {
        visibility.set_if_neq(if used.contains(&Input::Mouse(*b)) { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (PadKey(b), mut bg) in &mut pad_keys {
        bg.set_if_neq(BackgroundColor(if used.contains(&Input::Pad(*b)) { ACCENT } else { Color::NONE }));
    }
    for (PadLabel(b), mut color) in &mut pad_labels {
        color.set_if_neq(TextColor(if used.contains(&Input::Pad(*b)) { INK } else { TEXT }));
    }
    commands.entity(*entity).despawn_related::<Children>().with_children(|cols| {
        for column in columns {
            cols.spawn(Node { width: Val::Percent(33.3), flex_direction: FlexDirection::Column, row_gap: px(8.0), ..default() }).with_children(|c| {
                for (i, group) in column.iter().enumerate() {
                    c.spawn((text(group.title, 13.0, TEXT), Node { margin: UiRect::top(px(if i == 0 { 0.0 } else { 22.0 })), ..default() }));
                    for item in group.items {
                        c.spawn(Node { column_gap: px(5.0), align_items: AlignItems::Center, ..default() })
                            .with_children(|r| spawn_legend_item(r, item, &bindings, device, &art));
                    }
                }
            });
        }
    });
}

/// Text sizes follow the window's height, like the rest of the menu.
fn scale_fonts(root: Single<&ComputedNode, With<MenuRoot>>, mut texts: Query<(&MenuFont, &mut TextFont)>, added: Query<(), Added<MenuFont>>, mut last: Local<f32>) {
    let height = root.size.y * root.inverse_scale_factor;
    if height <= 0.0 || (height == *last && added.is_empty()) {
        return;
    }
    *last = height;
    for (MenuFont(size), mut font) in &mut texts {
        let scaled = size * height / 720.0;
        if font.font_size != scaled {
            font.font_size = scaled;
        }
    }
}

fn spawn_legend_item(r: &mut ChildSpawnerCommands, item: &Item, bindings: &Bindings, device: Device, art: &MenuArt) {
    match item {
        Item::Bind(name, actions) => {
            let soon = actions.iter().all(|a| !a.implemented());
            let bound: Vec<Vec<Input>> = actions.iter().map(|a| chord_of(bindings, device, *a)).filter(|c| !c.is_empty()).collect();
            if bound.is_empty() && *actions == MOVE && device == Device::Gamepad {
                keycap(r, "LEFT STICK", KEYCAP);
            }
            for chord in &bound {
                spawn_chord(r, chord, art, None);
                // Extra effort is a double tap: Rematch shows the key twice.
                if *actions == [Action::ExtraEffort] {
                    spawn_chord(r, chord, art, None);
                }
            }
            let stick = *actions == MOVE && device == Device::Gamepad;
            r.spawn(text(*name, 12.0, if soon || (bound.is_empty() && !stick) { TEXT_DIM } else { TEXT }));
        }
        Item::Fixed(name, kb, pad) => {
            match if device == Device::Keyboard { *kb } else { *pad } {
                "" => {}
                "MOUSE" => {
                    r.spawn((ImageNode::new(art.mouse[0].clone()).with_color(TEXT), Node { width: px(13.0), height: px(19.0), ..default() }));
                }
                cap => keycap(r, cap, KEYCAP),
            }
            r.spawn(text(*name, 12.0, TEXT));
        }
        Item::Later(name, kb) => {
            if device == Device::Keyboard && !kb.is_empty() {
                keycap(r, kb, KEYCAP_DIM);
            }
            r.spawn(text(*name, 12.0, TEXT_DIM));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PassAim;

    #[test]
    fn esc_and_f1() {
        let mut menu = Menu::default();
        // Esc opens the pause page and closes it.
        menu.escape();
        assert_eq!(menu.page, Some(Page::Pause));
        menu.escape();
        assert_eq!(menu.page, None);
        // F1 goes straight to the settings; Esc steps back to the pause page from there.
        menu.f1();
        assert_eq!(menu.page, Some(Page::Settings));
        menu.escape();
        assert_eq!(menu.page, Some(Page::Pause));
        menu.f1();
        menu.f1();
        assert_eq!(menu.page, None);
        // With a question open, Esc only cancels it.
        menu.f1();
        menu.ask_reset();
        menu.escape();
        assert_eq!((menu.page, menu.confirm), (Some(Page::Settings), None));
    }

    #[test]
    fn presets_keep_the_options() {
        let mine = Bindings { camera_lock: true, fov: 88.0, pass_aim: PassAim::CrosshairOnly, ..Bindings::default() };
        let b = apply_preset(&mine, Device::Keyboard, Preset::DoubleThumb);
        assert!(b.camera_lock && b.fov == 88.0 && b.pass_aim == PassAim::CrosshairOnly);
        assert!(b.keyboard == Bindings::rematch_modifier_preset().keyboard && b.gamepad == mine.gamepad);
        assert_eq!(current_preset(&b, Device::Keyboard), Some(Preset::DoubleThumb));
        // A changed binding is a custom mapping, and a preset over it asks first.
        let mut custom = b.clone();
        custom.keyboard.insert(Action::Shoot, vec![Input::Key(KeyCode::KeyK)]);
        assert_eq!(current_preset(&custom, Device::Keyboard), None);
        let (mut menu, mut cheats) = (Menu::default(), Cheats::default());
        assert!(!step_choice(Choice::Preset(Device::Keyboard), 1, &mut custom, &mut cheats, &mut menu));
        assert_eq!(menu.confirm, Some(Confirm::Preset(Device::Keyboard, Preset::Default)));
        // Without a custom mapping it just switches.
        let mut b = Bindings::default();
        assert!(step_choice(Choice::Preset(Device::Keyboard), 1, &mut b, &mut cheats, &mut Menu::default()));
        assert_eq!(current_preset(&b, Device::Keyboard), Some(Preset::DoubleThumb));
    }

    #[test]
    fn reset_is_per_tab() {
        let mut cheats = Cheats::default();
        let mut mine = Bindings::rematch_modifier_preset();
        mine.fov = 88.0;
        mine.invert_y = true;
        let gameplay = reset_tab(&mine, Tab::Gameplay, &mut cheats);
        assert!(gameplay.fov == Bindings::default().fov && !gameplay.invert_y && gameplay.keyboard == mine.keyboard);
        let controls = reset_tab(&mine, Tab::Controls, &mut cheats);
        assert!(controls.keyboard == Bindings::default().keyboard && controls.fov == 88.0);
        cheats.toggle(Cheat::ALL[0]);
        reset_tab(&mine, Tab::Cheats, &mut cheats);
        assert!(Cheat::ALL.into_iter().all(|c| !cheats.on(c)));
    }

    #[test]
    fn every_action_has_a_row() {
        let bound: Vec<Action> = sections(Tab::Controls)
            .iter()
            .flat_map(|s| s.rows.iter())
            .filter_map(|r| if let Control::Bind(a) = r.control { Some(a) } else { None })
            .flatten()
            .copied()
            .collect();
        assert!(Action::ALL.iter().all(|a| bound.contains(a)));
        // The cheats tab lists every cheat there is.
        assert_eq!(sections(Tab::Cheats)[0].rows.len(), Cheat::ALL.len());
    }

    #[test]
    fn values_as_rematch_writes_them() {
        assert_eq!((number(1.0), number(0.2), number(90.0), number(0.30000001)), ("1".into(), "0.2".into(), "90".into(), "0.3".into()));
        assert_eq!(Setting::MouseSensitivity.at_ratio(1.0), MOUSE_SENSITIVITY_RANGE.1);
        let soon = row("x", "", soon_choice("Hold", 0, 2));
        assert!(soon.soon() && soon.label() == "x (soon)" && soon.default_value(Device::Keyboard) == "Hold");
        assert_eq!(row("Move", "", bind(MOVE)).default_value(Device::Gamepad), "LEFT STICK");
    }

    /// The menu with its systems, on the Controls tab; the tests press its nodes by hand (there's
    /// no window to hover).
    fn menu_app() -> App {
        let mut app = App::new();
        app.insert_resource(Assets::<Image>::default())
            .init_resource::<Menu>()
            .init_resource::<Bindings>()
            .init_resource::<Cheats>()
            .init_resource::<InputBlocked>()
            .init_resource::<Actions>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseScroll>()
            .add_event::<AppExit>()
            .add_systems(Startup, spawn_menu)
            .add_systems(Update, ((menu_keys, update_systems()).chain(), show_menu));
        app.world_mut().resource_mut::<Menu>().open_scripted("controls");
        app.update();
        app
    }

    fn press<C: Component>(app: &mut App, pick: impl Fn(&C) -> bool) -> Entity {
        let world = app.world_mut();
        let entity = world.query::<(Entity, &C)>().iter(world).find(|(_, c)| pick(c)).map(|(e, _)| e).expect("node to press");
        world.entity_mut(entity).insert(Interaction::Pressed);
        entity
    }

    fn release_all(app: &mut App) {
        let world = app.world_mut();
        for mut i in world.query::<&mut Interaction>().iter_mut(world) {
            *i = Interaction::None;
        }
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
    }

    #[test]
    fn rebinding_with_a_click_and_keys() {
        let mut app = menu_app();
        // Clicking Shoot's keycap captures the next chord; it's bound once every key is let go.
        let slot = press::<BindSlot>(&mut app, |s| s.action == Action::Shoot);
        app.update();
        let row = app.world().get::<BindSlot>(slot).unwrap().row;
        assert_eq!(app.world().resource::<Menu>().capture_row, Some(row));
        release_all(&mut app);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyK);
        app.update();
        release_all(&mut app);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release_all();
        app.update();
        let chord = vec![Input::Key(KeyCode::ControlLeft), Input::Key(KeyCode::KeyK)];
        assert_eq!(app.world().resource::<Bindings>().keyboard.get(&Action::Shoot), Some(&chord));
        assert!(app.world().resource::<Menu>().capturing.is_none());
        // Esc cancels a capture and leaves the binding and the page as they were.
        press::<Row>(&mut app, |r| r.def.label == "Tap");
        app.update();
        release_all(&mut app);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
        app.update();
        let menu = app.world().resource::<Menu>();
        assert!(menu.capturing.is_none() && menu.page == Some(Page::Settings));
        assert_eq!(app.world().resource::<Bindings>().keyboard.get(&Action::Pass), Bindings::default().keyboard.get(&Action::Pass));
    }

    #[test]
    fn clearing_a_binding_does_not_start_a_capture() {
        let mut app = menu_app();
        // The icon sits on the row, so a click presses both.
        let clear = press::<ClearBind>(&mut app, |c| c.0 == [Action::Lob]);
        let row = app.world().get::<Tint>(clear).unwrap().row;
        app.world_mut().entity_mut(row).insert(Interaction::Pressed);
        app.update();
        assert!(app.world().resource::<Bindings>().keyboard.get(&Action::Lob).is_none());
        assert!(app.world().resource::<Menu>().capturing.is_none());
        assert_eq!(current_preset(app.world().resource::<Bindings>(), Device::Keyboard), None);
    }

    #[test]
    fn choices_tabs_and_the_preset_question() {
        let mut app = menu_app();
        // The right of a choice steps forward, its left end back.
        let aim = press::<ChoiceControl>(&mut app, |c| c.0 == Choice::PassAim);
        app.world_mut().entity_mut(aim).insert(RelativeCursorPosition { normalized: Some(Vec2::new(0.8, 0.5)), ..default() });
        app.update();
        assert_eq!(app.world().resource::<Bindings>().pass_aim, PassAim::JoystickEmulation);
        release_all(&mut app);
        app.update();
        app.world_mut().entity_mut(aim).insert((Interaction::Pressed, RelativeCursorPosition { normalized: Some(Vec2::new(0.1, 0.5)), ..default() }));
        app.update();
        assert_eq!(app.world().resource::<Bindings>().pass_aim, PassAim::DirectionalAiming);
        // A preset over a custom mapping asks, and Confirm applies it.
        release_all(&mut app);
        app.world_mut().resource_mut::<Bindings>().keyboard.insert(Action::Shoot, vec![Input::Key(KeyCode::KeyK)]);
        app.update();
        press::<ChoiceControl>(&mut app, |c| c.0 == Choice::Preset(Device::Keyboard));
        app.update();
        assert!(app.world().resource::<Menu>().confirm.is_some());
        release_all(&mut app);
        press::<MenuButton>(&mut app, |b| *b == MenuButton::Confirm);
        app.update();
        assert_eq!(current_preset(app.world().resource::<Bindings>(), Device::Keyboard), Some(Preset::Default));
        assert!(app.world().resource::<Menu>().confirm.is_none());
        // Tabs: a click, or Q and E.
        release_all(&mut app);
        press::<MenuButton>(&mut app, |b| *b == MenuButton::Tab(Tab::Cheats));
        app.update();
        assert_eq!(app.world().resource::<Menu>().tab, Tab::Cheats);
        release_all(&mut app);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyQ);
        app.update();
        assert_eq!(app.world().resource::<Menu>().tab, Tab::Controls);
        // The cheats' toggles flip the cheats.
        release_all(&mut app);
        app.world_mut().resource_mut::<Menu>().tab = Tab::Cheats;
        press::<ChoiceControl>(&mut app, |c| c.0 == Choice::Cheat(Cheat::ALL[0]));
        app.update();
        assert!(app.world().resource::<Cheats>().on(Cheat::ALL[0]));
    }

    #[test]
    fn scripted_pages() {
        let mut menu = Menu::default();
        menu.open_scripted("controls:scroll=500:part=2:pad:lit=3");
        assert_eq!((menu.page, menu.tab, menu.part, menu.device), (Some(Page::Settings), Tab::Controls, Part::WithBall, Device::Gamepad));
        assert_eq!((menu.scroll_to, menu.lit), (Some(500.0), Some(3)));
        let mut menu = Menu::default();
        menu.open_scripted("1");
        assert_eq!(menu.page, Some(Page::Pause));
        // Q and E stop at the ends.
        assert_eq!(Tab::Gameplay.step(-1), Tab::Gameplay);
        assert_eq!(Tab::Cheats.step(1), Tab::Cheats);
    }
}
