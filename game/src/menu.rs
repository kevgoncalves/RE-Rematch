//! The Esc menu: a pause page (Settings, Leave) and the settings in tabs. Controls has every action
//! with its keyboard and gamepad binding (click to rebind) and Rematch's control and camera options;
//! Cheats has the practice aids. F1 goes straight to the settings.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::fx::{Cheat, Cheats};
use crate::input::{Action, Bindings, Device, Input, InputBlocked, PassAim, chord_name};
use crate::tuning::{CAMERA_KICK_LIMIT, FOV_OPTION_RANGE, MAGNUS_SENSITIVITY_RANGE};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Pause,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    #[default]
    Controls,
    Cheats,
}

impl Tab {
    const ALL: [Tab; 2] = [Tab::Controls, Tab::Cheats];

    fn label(self) -> &'static str {
        match self {
            Tab::Controls => "Controls",
            Tab::Cheats => "Cheats",
        }
    }
}

#[derive(Resource, Default)]
pub struct Menu {
    /// The page shown; none while playing.
    page: Option<Page>,
    /// The settings tab, kept while the menu is closed.
    tab: Tab,
    /// Binding being captured, and the inputs pressed so far (in press order).
    capturing: Option<(Action, Device, Vec<Input>)>,
}

impl Menu {
    pub fn open(&self) -> bool {
        self.page.is_some()
    }

    /// Esc opens the pause page and closes it again; from the settings it steps back to it.
    fn escape(&mut self) {
        self.page = match self.page {
            None | Some(Page::Settings) => Some(Page::Pause),
            Some(Page::Pause) => None,
        };
    }

    /// F1 goes straight to the settings and back out.
    fn f1(&mut self) {
        self.page = if self.page == Some(Page::Settings) { None } else { Some(Page::Settings) };
    }

    /// For screenshots (RE_MENU): "controls" or "cheats" opens that settings tab, anything else
    /// the pause page.
    pub fn open_scripted(&mut self, what: &str) {
        self.page = Some(Page::Pause);
        if let Some(tab) = Tab::ALL.into_iter().find(|t| what.eq_ignore_ascii_case(t.label())) {
            self.page = Some(Page::Settings);
            self.tab = tab;
        }
    }
}

#[derive(Component)]
pub struct MenuRoot;

/// The panel of a page.
#[derive(Component)]
pub struct PageNode(Page);

/// The contents of a settings tab.
#[derive(Component)]
pub struct TabBody(Tab);

#[derive(Component, Clone, Copy)]
pub struct BindButton(Action, Device);

#[derive(Component, Clone, Copy)]
pub enum MenuButton {
    Settings,
    /// There's no main menu yet, so this quits.
    Leave,
    /// From the settings back to the pause page.
    Back,
    Tab(Tab),
    /// Rematch's default layout.
    Defaults,
    /// Rematch's "modifier" layout for mice with thumb buttons (modifier + pass = lob).
    ModifierPreset,
    /// Cycles the keyboard-and-mouse pass aiming mode.
    PassAim,
    /// Turns the gamepad's camera lock on or off.
    CameraLock,
    /// Turns a practice cheat on or off.
    Cheat(Cheat),
}

/// A slider option, saved with the bindings.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum Setting {
    MagnusSensitivity,
    Fov,
}

impl Setting {
    fn range(self) -> (f32, f32) {
        match self {
            Setting::MagnusSensitivity => MAGNUS_SENSITIVITY_RANGE,
            Setting::Fov => FOV_OPTION_RANGE,
        }
    }

    fn step(self) -> f32 {
        match self {
            Setting::MagnusSensitivity => 0.05,
            Setting::Fov => 1.0,
        }
    }

    fn value(self, b: &Bindings) -> f32 {
        match self {
            Setting::MagnusSensitivity => b.magnus_sensitivity,
            Setting::Fov => b.fov,
        }
    }

    fn set(self, b: &mut Bindings, v: f32) {
        match self {
            Setting::MagnusSensitivity => b.magnus_sensitivity = v,
            Setting::Fov => b.fov = v,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Setting::MagnusSensitivity => "Magnus sensitivity",
            Setting::Fov => "Field of view",
        }
    }

    fn value_label(self, v: f32) -> String {
        match self {
            Setting::MagnusSensitivity => format!("{v:.2}"),
            Setting::Fov => format!("{v:.0} deg"),
        }
    }

    /// Slider position 0..1 to a value on the option's steps.
    fn at_ratio(self, ratio: f32) -> f32 {
        let (lo, hi) = self.range();
        let v = lo + ratio.clamp(0.0, 1.0) * (hi - lo);
        ((v / self.step()).round() * self.step()).clamp(lo, hi)
    }
}

#[derive(Component)]
pub struct SliderFill(Setting);

#[derive(Component)]
pub struct SliderValue(Setting);

const OVERLAY: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
const PANEL: Color = Color::srgb(0.05, 0.06, 0.09);
const SECTION: Color = Color::srgba(1.0, 1.0, 1.0, 0.04);
const DIVIDER: Color = Color::srgba(1.0, 1.0, 1.0, 0.1);
const BUTTON: Color = Color::srgb(0.16, 0.18, 0.24);
const BUTTON_HOVER: Color = Color::srgb(0.23, 0.26, 0.34);
const BUTTON_ACTIVE: Color = Color::srgb(0.85, 0.45, 0.15);
const BUTTON_DISABLED: Color = Color::srgb(0.1, 0.11, 0.14);
const TEXT: Color = Color::srgb(0.93, 0.94, 0.96);
const TEXT_DIM: Color = Color::srgb(0.58, 0.61, 0.68);

/// Sizes in pixels: every button is BUTTON_H high except the binding rows (ROW_H).
const PAD: f32 = 20.0;
const GAP: f32 = 14.0;
const RADIUS: f32 = 6.0;
const BUTTON_H: f32 = 34.0;
const ROW_H: f32 = 20.0;
const ACTION_W: f32 = 200.0;
const BIND_W: f32 = 170.0;
const TOGGLE_W: f32 = 96.0;
/// The settings panel keeps one size whichever tab is open.
const SETTINGS_SIZE: Vec2 = Vec2::new(980.0, 660.0);

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: size, ..default() }, TextColor(color))
}

/// The menu's button look, its label centred.
fn button(width: Val, height: f32) -> impl Bundle {
    (
        Button,
        Node { width, height: Val::Px(height), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        BackgroundColor(BUTTON),
        BorderRadius::all(Val::Px(RADIUS)),
    )
}

/// A labelled button of the standard height.
fn labelled(parent: &mut ChildSpawnerCommands, width: Val, kind: MenuButton, label: &str) {
    parent.spawn((button(width, BUTTON_H), kind)).with_child(text(label, 15.0, TEXT));
}

/// A boxed group of options under a small heading.
fn section(parent: &mut ChildSpawnerCommands, title: &str, contents: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn((
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::all(Val::Px(12.0)), ..default() },
            BackgroundColor(SECTION),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .with_children(|s| {
            s.spawn(text(title, 12.0, TEXT_DIM));
            contents(s);
        });
}

/// An option: its name on the left, its control on the right.
fn option_row(parent: &mut ChildSpawnerCommands, name: &str, control: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: Val::Px(12.0), ..default() })
        .with_children(|row| {
            row.spawn(text(name, 14.0, TEXT));
            control(row);
        });
}

/// An On/Off button; its label and colour follow the option.
fn toggle(parent: &mut ChildSpawnerCommands, kind: MenuButton) {
    labelled(parent, Val::Px(TOGGLE_W), kind, "");
}

/// A slider: name and value above, a bar to click or drag below.
fn slider(parent: &mut ChildSpawnerCommands, setting: Setting) {
    parent.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }).with_children(|col| {
        col.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
            row.spawn(text(setting.name(), 14.0, TEXT));
            row.spawn((SliderValue(setting), text("", 14.0, TEXT_DIM)));
        });
        col.spawn((
            Button,
            setting,
            RelativeCursorPosition::default(),
            Node { width: Val::Percent(100.0), height: Val::Px(12.0), ..default() },
            BackgroundColor(BUTTON),
            BorderRadius::all(Val::Px(6.0)),
        ))
        .with_child((SliderFill(setting), Node { height: Val::Percent(100.0), ..default() }, BackgroundColor(BUTTON_ACTIVE), BorderRadius::all(Val::Px(6.0))));
    });
}

fn panel(page: Page, width: Val, height: Val) -> impl Bundle {
    (
        PageNode(page),
        Node {
            width,
            height,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(PAD)),
            row_gap: Val::Px(GAP),
            display: Display::None,
            ..default()
        },
        BackgroundColor(PANEL),
        BorderRadius::all(Val::Px(10.0)),
    )
}

pub fn spawn_menu(mut commands: Commands) {
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(OVERLAY),
            GlobalZIndex(100),
        ))
        .with_children(|root| {
            root.spawn(panel(Page::Pause, Val::Px(340.0), Val::Auto)).with_children(|p| {
                // The game runs on behind it, as in Rematch.
                p.spawn((text("Menu", 28.0, TEXT), Node { align_self: AlignSelf::Center, margin: UiRect::bottom(Val::Px(6.0)), ..default() }));
                labelled(p, Val::Percent(100.0), MenuButton::Settings, "Settings");
                labelled(p, Val::Percent(100.0), MenuButton::Leave, "Leave");
                p.spawn((text("Esc to resume", 13.0, TEXT_DIM), Node { align_self: AlignSelf::Center, ..default() }));
            });
            root.spawn(panel(Page::Settings, Val::Px(SETTINGS_SIZE.x), Val::Px(SETTINGS_SIZE.y))).with_children(spawn_settings);
        });
}

fn spawn_settings(p: &mut ChildSpawnerCommands) {
    // Header: back, title and the tabs.
    p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|row| {
        labelled(row, Val::Px(90.0), MenuButton::Back, "Back");
        row.spawn((text("Settings", 22.0, TEXT), Node { margin: UiRect::horizontal(Val::Px(12.0)), ..default() }));
        for tab in Tab::ALL {
            labelled(row, Val::Px(130.0), MenuButton::Tab(tab), tab.label());
        }
        row.spawn(Node { flex_grow: 1.0, ..default() });
        row.spawn(text("Esc: back", 12.0, TEXT_DIM));
    });
    p.spawn((Node { height: Val::Px(1.0), ..default() }, BackgroundColor(DIVIDER)));
    p.spawn((TabBody(Tab::Controls), Node { column_gap: Val::Px(28.0), ..default() })).with_children(spawn_controls);
    p.spawn((TabBody(Tab::Cheats), Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() })).with_children(|body| {
        body.spawn(text("Practice aids. They're off again when the game restarts.", 13.0, TEXT_DIM));
        body.spawn(Node { width: Val::Px(440.0), flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
            section(col, "PRACTICE", |s| {
                for cheat in Cheat::ALL {
                    option_row(s, cheat.label(), |row| toggle(row, MenuButton::Cheat(cheat)));
                }
            });
        });
    });
}

fn spawn_controls(body: &mut ChildSpawnerCommands) {
    // Bindings, one row per action.
    body.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).with_children(|col| {
        col.spawn((text("Click a binding, press the new keys, release. Esc cancels.", 13.0, TEXT_DIM), Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() }));
        col.spawn(Node { column_gap: Val::Px(6.0), ..default() }).with_children(|row| {
            for (label, width) in [("Action", ACTION_W), ("Keyboard and mouse", BIND_W), ("Gamepad", BIND_W)] {
                row.spawn((text(label, 12.0, TEXT_DIM), Node { width: Val::Px(width), ..default() }));
            }
        });
        for action in Action::ALL {
            col.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                let (label, color) = if action.implemented() { (format!("{action:?}"), TEXT) } else { (format!("{action:?} (soon)"), TEXT_DIM) };
                row.spawn((text(label, 13.0, color), Node { width: Val::Px(ACTION_W), ..default() }));
                for device in [Device::Keyboard, Device::Gamepad] {
                    row.spawn((button(Val::Px(BIND_W), ROW_H), BindButton(action, device))).with_child(text("", 13.0, TEXT));
                }
            });
        }
    });
    // Presets and options.
    body.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), flex_grow: 1.0, ..default() }).with_children(|col| {
        section(col, "PRESETS", |s| {
            labelled(s, Val::Percent(100.0), MenuButton::Defaults, "Rematch defaults");
            labelled(s, Val::Percent(100.0), MenuButton::ModifierPreset, "Rematch modifier (thumb buttons)");
        });
        section(col, "KEYBOARD AND MOUSE", |s| {
            option_row(s, "Pass aim", |row| labelled(row, Val::Px(180.0), MenuButton::PassAim, ""));
            slider(s, Setting::MagnusSensitivity);
        });
        section(col, "CAMERA", |s| {
            slider(s, Setting::Fov);
            s.spawn((text("Camera lock while kicking", 14.0, TEXT), Node { margin: UiRect::top(Val::Px(4.0)), ..default() }));
            option_row(s, "Gamepad", |row| toggle(row, MenuButton::CameraLock));
            option_row(s, "Keyboard and mouse", |row| {
                row.spawn((
                    Node { width: Val::Px(TOGGLE_W), height: Val::Px(BUTTON_H), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(BUTTON_DISABLED),
                    BorderRadius::all(Val::Px(RADIUS)),
                ))
                .with_child(text("Always on", 13.0, TEXT_DIM));
            });
            let note = format!(
                "On: shots aim where the camera pointed at the press, and it stays there. Off: the right stick can still turn the camera while you kick, up to {:.0} deg from the press.",
                CAMERA_KICK_LIMIT.0
            );
            s.spawn(text(note, 12.0, TEXT_DIM));
        });
    });
}

/// Esc and F1. Runs first, so gameplay ignores input from the frame the menu opens.
pub fn menu_keys(keys: Res<ButtonInput<KeyCode>>, mut menu: ResMut<Menu>, mut blocked: ResMut<InputBlocked>) {
    // While a binding is being captured, Esc cancels the capture (capture_binding) and F1 is ignored.
    if menu.capturing.is_none() {
        if keys.just_pressed(KeyCode::Escape) {
            menu.escape();
        } else if keys.just_pressed(KeyCode::F1) {
            menu.f1();
        }
    }
    blocked.0 = menu.open();
}

/// Shows the open page and tab, and frees the cursor while the menu is open. It runs after
/// `grab_cursor` (which frees the cursor on Esc), so closing the menu takes the cursor back: you're
/// straight back in the game, without a click that would also shoot.
pub fn show_menu(
    menu: Res<Menu>,
    mut was_open: Local<bool>,
    mut root: Single<&mut Node, With<MenuRoot>>,
    mut pages: Query<(&PageNode, &mut Node), Without<MenuRoot>>,
    mut tabs: Query<(&TabBody, &mut Node), (Without<MenuRoot>, Without<PageNode>)>,
    window: Option<Single<&mut Window, With<PrimaryWindow>>>,
) {
    if !menu.is_changed() {
        return;
    }
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    root.display = display(menu.open());
    for (PageNode(page), mut node) in &mut pages {
        node.display = display(menu.page == Some(*page));
    }
    for (TabBody(tab), mut node) in &mut tabs {
        node.display = display(*tab == menu.tab);
    }
    if menu.open() != *was_open {
        *was_open = menu.open();
        if let Some(mut window) = window {
            window.cursor_options.grab_mode = if menu.open() { CursorGrabMode::None } else { CursorGrabMode::Locked };
            window.cursor_options.visible = menu.open();
        }
    }
}

/// A preset's buttons with the options you already have.
fn keep_options(current: &Bindings, preset: Bindings) -> Bindings {
    Bindings { keyboard: preset.keyboard, gamepad: preset.gamepad, ..current.clone() }
}

pub fn menu_clicks(
    mut menu: ResMut<Menu>,
    mut bindings: ResMut<Bindings>,
    mut cheats: ResMut<Cheats>,
    binds: Query<(&Interaction, &BindButton), Changed<Interaction>>,
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut exit: EventWriter<AppExit>,
) {
    // While capturing, clicks are inputs to bind, not menu clicks.
    if !menu.open() || menu.capturing.is_some() {
        return;
    }
    for (interaction, button) in &binds {
        if *interaction == Interaction::Pressed {
            menu.capturing = Some((button.0, button.1, Vec::new()));
        }
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let changed = match *button {
            MenuButton::Settings => {
                menu.page = Some(Page::Settings);
                None
            }
            MenuButton::Leave => {
                exit.write(AppExit::Success);
                None
            }
            MenuButton::Back => {
                menu.page = Some(Page::Pause);
                None
            }
            MenuButton::Tab(tab) => {
                menu.tab = tab;
                None
            }
            MenuButton::Cheat(cheat) => {
                cheats.toggle(cheat);
                None
            }
            // Presets swap the buttons but keep the options.
            MenuButton::Defaults => Some(keep_options(&bindings, Bindings::default())),
            MenuButton::ModifierPreset => Some(keep_options(&bindings, Bindings::rematch_modifier_preset())),
            MenuButton::PassAim => Some(Bindings { pass_aim: bindings.pass_aim.next(), ..bindings.clone() }),
            MenuButton::CameraLock => Some(Bindings { camera_lock: !bindings.camera_lock, ..bindings.clone() }),
        };
        if let Some(changed) = changed {
            *bindings = changed;
            bindings.save();
        }
    }
}

/// Dragging on a slider's bar sets its option; saved on release.
pub fn drag_sliders(
    menu: Res<Menu>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut bindings: ResMut<Bindings>,
    sliders: Query<(&Interaction, &RelativeCursorPosition, &Setting)>,
) {
    if !menu.open() || menu.capturing.is_some() {
        return;
    }
    for (interaction, cursor, setting) in &sliders {
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
pub fn capture_binding(
    mut menu: ResMut<Menu>,
    mut bindings: ResMut<Bindings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
) {
    let Some((action, device, chord)) = &mut menu.capturing else { return };
    if keys.just_pressed(KeyCode::Escape) {
        menu.capturing = None;
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
    }
}

/// A button's colour: lit when on (the open tab, an option that's on), lighter under the mouse.
fn shade(on: bool, interaction: Interaction) -> BackgroundColor {
    BackgroundColor(match (on, interaction) {
        (true, _) => BUTTON_ACTIVE,
        (false, Interaction::None) => BUTTON,
        (false, _) => BUTTON_HOVER,
    })
}

fn on_off(on: bool) -> String {
    if on { "On" } else { "Off" }.into()
}

/// Rematch's names for the pass aiming modes.
fn pass_aim_name(aim: PassAim) -> &'static str {
    match aim {
        PassAim::DirectionalAiming => "Directional aiming",
        PassAim::JoystickEmulation => "Joystick emulation",
        PassAim::CrosshairOnly => "Crosshair only",
    }
}

fn set_label(texts: &mut Query<&mut Text, Without<SliderValue>>, children: &Children, label: String) {
    if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok())
        && text.0 != label
    {
        text.0 = label;
    }
}

pub fn refresh_labels(
    menu: Res<Menu>,
    bindings: Res<Bindings>,
    cheats: Res<Cheats>,
    mut binds: Query<(&BindButton, &Interaction, &Children, &mut BackgroundColor)>,
    mut buttons: Query<(&MenuButton, &Interaction, &Children, &mut BackgroundColor), Without<BindButton>>,
    mut texts: Query<&mut Text, Without<SliderValue>>,
    mut fills: Query<(&SliderFill, &mut Node)>,
    mut values: Query<(&SliderValue, &mut Text)>,
) {
    if !menu.open() {
        return;
    }
    for (SliderFill(setting), mut node) in &mut fills {
        let (lo, hi) = setting.range();
        node.width = Val::Percent((setting.value(&bindings) - lo) / (hi - lo) * 100.0);
    }
    for (SliderValue(setting), mut text) in &mut values {
        let label = setting.value_label(setting.value(&bindings));
        if text.0 != label {
            text.0 = label;
        }
    }
    for (BindButton(action, device), interaction, children, mut bg) in &mut binds {
        let capturing = matches!(&menu.capturing, Some((a, d, _)) if a == action && d == device);
        let label = match (&menu.capturing, capturing) {
            (Some((_, _, chord)), true) if !chord.is_empty() => format!("{} ...", chord_name(Some(chord))),
            (_, true) => "press keys...".into(),
            _ => chord_name(match device {
                Device::Keyboard => bindings.keyboard.get(action),
                Device::Gamepad => bindings.gamepad.get(action),
            }),
        };
        bg.set_if_neq(shade(capturing, *interaction));
        set_label(&mut texts, children, label);
    }
    for (button, interaction, children, mut bg) in &mut buttons {
        let (on, label) = match *button {
            MenuButton::Tab(tab) => (menu.tab == tab, None),
            MenuButton::PassAim => (false, Some(pass_aim_name(bindings.pass_aim).into())),
            MenuButton::CameraLock => (bindings.camera_lock, Some(on_off(bindings.camera_lock))),
            MenuButton::Cheat(cheat) => (cheats.on(cheat), Some(on_off(cheats.on(cheat)))),
            _ => (false, None),
        };
        bg.set_if_neq(shade(on, *interaction));
        if let Some(label) = label {
            set_label(&mut texts, children, label);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn presets_keep_the_options() {
        let mine = Bindings { camera_lock: true, fov: 88.0, pass_aim: PassAim::CrosshairOnly, ..Bindings::default() };
        let b = keep_options(&mine, Bindings::rematch_modifier_preset());
        assert!(b.camera_lock && b.fov == 88.0 && b.pass_aim == PassAim::CrosshairOnly);
        assert!(b.keyboard == Bindings::rematch_modifier_preset().keyboard);
    }
}
