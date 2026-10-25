//! Controls menu (F1): every action with its keyboard and gamepad binding, click to rebind, and
//! Rematch's camera and curve options as sliders.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::fx::{Cheat, Cheats};
use crate::input::{Action, Bindings, Device, Input, InputBlocked, chord_name};
use crate::tuning::{FOV_OPTION_RANGE, MAGNUS_SENSITIVITY_RANGE};

#[derive(Resource, Default)]
pub struct Menu {
    pub open: bool,
    /// Binding being captured, and the inputs pressed so far (in press order).
    capturing: Option<(Action, Device, Vec<Input>)>,
}

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component, Clone, Copy)]
pub struct BindButton(Action, Device);

#[derive(Component, Clone, Copy)]
pub enum MenuButton {
    /// Rematch's default layout.
    Defaults,
    /// Rematch's "modifier" layout for mice with thumb buttons (modifier + pass = lob).
    ModifierPreset,
    /// Cycles the keyboard-and-mouse pass aiming mode.
    PassAim,
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
    const ALL: [Setting; 2] = [Setting::MagnusSensitivity, Setting::Fov];

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

    fn label(self, v: f32) -> String {
        match self {
            Setting::MagnusSensitivity => format!("Magnus sensitivity  {v:.2}"),
            Setting::Fov => format!("Field of view  {v:.0} deg"),
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
pub struct SliderLabel(Setting);

const BG: Color = Color::srgba(0.02, 0.03, 0.05, 0.92);
const BUTTON: Color = Color::srgb(0.16, 0.18, 0.24);
const BUTTON_ACTIVE: Color = Color::srgb(0.85, 0.45, 0.15);

pub fn spawn_menu(mut commands: Commands) {
    let text = |s: String, size: f32| (Text::new(s), TextFont { font_size: size, ..default() });
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
        ))
        .with_children(|root| {
            root.spawn((
                Node { flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(12.0)), row_gap: Val::Px(1.0), ..default() },
                BackgroundColor(BG),
            ))
            .with_children(|panel| {
                panel.spawn(text("Controls  (F1 to close) - defaults copied from Rematch".into(), 20.0));
                panel.spawn(text(
                    "Click a binding, press the new key or combination, release to confirm. Esc cancels.".into(),
                    14.0,
                ));
                for action in Action::ALL {
                    panel
                        .spawn(Node { column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() })
                        .with_children(|row| {
                            let label = if action.implemented() { format!("{action:?}") } else { format!("{action:?} (soon)") };
                            row.spawn((text(label, 13.0), Node { width: Val::Px(190.0), ..default() }));
                            for device in [Device::Keyboard, Device::Gamepad] {
                                row.spawn((
                                    Button,
                                    BindButton(action, device),
                                    Node { width: Val::Px(200.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(1.0)), ..default() },
                                    BackgroundColor(BUTTON),
                                ))
                                .with_child(text(String::new(), 13.0));
                            }
                        });
                }
                panel.spawn(Node { margin: UiRect::top(Val::Px(8.0)), column_gap: Val::Px(8.0), ..default() }).with_children(|row| {
                    for (button, label) in [
                        (MenuButton::Defaults, "Rematch defaults"),
                        (MenuButton::ModifierPreset, "Rematch modifier (thumb buttons)"),
                        (MenuButton::PassAim, ""),
                    ] {
                        row.spawn((Button, button, Node { padding: UiRect::all(Val::Px(6.0)), ..default() }, BackgroundColor(BUTTON)))
                            .with_child(text(label.into(), 15.0));
                    }
                });
                panel.spawn(Node { margin: UiRect::top(Val::Px(8.0)), column_gap: Val::Px(8.0), ..default() }).with_children(|row| {
                    for cheat in Cheat::ALL {
                        row.spawn((Button, MenuButton::Cheat(cheat), Node { padding: UiRect::all(Val::Px(6.0)), ..default() }, BackgroundColor(BUTTON)))
                            .with_child(text(String::new(), 15.0));
                    }
                });
                // Options: click or drag on a bar.
                for setting in Setting::ALL {
                    panel
                        .spawn(Node { margin: UiRect::top(Val::Px(6.0)), column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() })
                        .with_children(|row| {
                            row.spawn((SliderLabel(setting), text(String::new(), 15.0), Node { width: Val::Px(230.0), ..default() }));
                            row.spawn((
                                Button,
                                setting,
                                RelativeCursorPosition::default(),
                                Node { width: Val::Px(300.0), height: Val::Px(14.0), ..default() },
                                BackgroundColor(BUTTON),
                            ))
                            .with_child((SliderFill(setting), Node { height: Val::Percent(100.0), ..default() }, BackgroundColor(BUTTON_ACTIVE)));
                        });
                }
            });
        });
}

pub fn toggle_menu(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<Menu>,
    mut blocked: ResMut<InputBlocked>,
    mut root: Single<&mut Node, With<MenuRoot>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::F1) {
        menu.open = !menu.open;
        menu.capturing = None;
        root.display = if menu.open { Display::Flex } else { Display::None };
        if menu.open {
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
        }
    }
    blocked.0 = menu.open;
}

pub fn menu_clicks(
    mut menu: ResMut<Menu>,
    mut bindings: ResMut<Bindings>,
    mut cheats: ResMut<Cheats>,
    binds: Query<(&Interaction, &BindButton), Changed<Interaction>>,
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
) {
    // While capturing, clicks are inputs to bind, not menu clicks.
    if !menu.open || menu.capturing.is_some() {
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
        // Presets swap the buttons but keep the pass aiming choice.
        let aim = bindings.pass_aim;
        match button {
            MenuButton::Defaults => *bindings = Bindings { pass_aim: aim, ..Bindings::default() },
            MenuButton::ModifierPreset => *bindings = Bindings { pass_aim: aim, ..Bindings::rematch_modifier_preset() },
            MenuButton::PassAim => bindings.pass_aim = aim.next(),
            MenuButton::Cheat(cheat) => {
                cheats.toggle(*cheat);
                continue;
            }
        }
        bindings.save();
    }
}

/// Dragging on a slider's bar sets its option; saved on release.
pub fn drag_sliders(
    menu: Res<Menu>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut bindings: ResMut<Bindings>,
    sliders: Query<(&Interaction, &RelativeCursorPosition, &Setting)>,
) {
    if !menu.open || menu.capturing.is_some() {
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

pub fn refresh_labels(
    menu: Res<Menu>,
    bindings: Res<Bindings>,
    cheats: Res<Cheats>,
    mut buttons: Query<(&BindButton, &Children, &mut BackgroundColor)>,
    mut menu_buttons: Query<(&MenuButton, &Children, &mut BackgroundColor), Without<BindButton>>,
    mut texts: Query<&mut Text, Without<SliderLabel>>,
    mut fills: Query<(&SliderFill, &mut Node)>,
    mut slider_labels: Query<(&SliderLabel, &mut Text)>,
) {
    if !menu.open {
        return;
    }
    for (SliderFill(setting), mut node) in &mut fills {
        let (lo, hi) = setting.range();
        node.width = Val::Percent((setting.value(&bindings) - lo) / (hi - lo) * 100.0);
    }
    for (SliderLabel(setting), mut text) in &mut slider_labels {
        let label = setting.label(setting.value(&bindings));
        if text.0 != label {
            text.0 = label;
        }
    }
    for (BindButton(action, device), children, mut bg) in &mut buttons {
        let capturing = matches!(&menu.capturing, Some((a, d, _)) if a == action && d == device);
        let label = match (&menu.capturing, capturing) {
            (Some((_, _, chord)), true) if !chord.is_empty() => format!("{} ...", chord_name(Some(chord))),
            (_, true) => "press keys...".into(),
            _ => chord_name(match device {
                Device::Keyboard => bindings.keyboard.get(action),
                Device::Gamepad => bindings.gamepad.get(action),
            }),
        };
        bg.0 = if capturing { BUTTON_ACTIVE } else { BUTTON };
        if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && text.0 != label
        {
            text.0 = label;
        }
    }
    for (button, children, mut bg) in &mut menu_buttons {
        let label = match button {
            MenuButton::PassAim => format!("Pass aim: {:?}", bindings.pass_aim),
            MenuButton::Cheat(cheat) => {
                bg.0 = if cheats.on(*cheat) { BUTTON_ACTIVE } else { BUTTON };
                format!("{}: {}", cheat.label(), if cheats.on(*cheat) { "on" } else { "off" })
            }
            _ => continue,
        };
        if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok())
            && text.0 != label
        {
            text.0 = label;
        }
    }
}
