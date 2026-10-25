//! RE Rematch prototype: one player, the ball and the 5v5 arena, tuned with Rematch's own values.

mod arena;
mod ball;
mod body;
mod camera;
mod fx;
mod gauges;
mod input;
mod menu;
mod pass_line;
mod player;
mod reticle;
mod tuning;

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use ball::{Ball, GoalScored};
use input::{Actions, Axes, Bindings, InputBlocked};
use player::{CameraRig, Kick, Player};
use tuning::*;

#[derive(Resource, Default)]
struct Score([u32; 2]);

#[derive(Component)]
struct Hud;

/// The scoreboard's texts: left score, clock, right score.
#[derive(Component)]
enum Board {
    Left,
    Clock,
    Right,
}

/// A scripted test run (RE_SCREENSHOT) is headless: no window, the camera renders into an image
/// that the screenshot is taken from, and the app ticks at 60 Hz on its own.
fn headless() -> bool {
    std::env::var("RE_SCREENSHOT").is_ok()
}

/// The image a headless run renders into.
#[derive(Resource)]
struct HeadlessTarget(Handle<Image>);

fn main() {
    let mut app = App::new();
    // The Vulkan loader logs stale overlay layers some launchers leave registered; harmless noise.
    let log = bevy::log::LogPlugin { filter: "wgpu=error,naga=warn,wgpu_hal::vulkan::instance=off".into(), ..default() };
    if headless() {
        app.add_plugins((
            DefaultPlugins
                .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, close_when_requested: false })
                .set(log)
                .disable::<WinitPlugin>(),
            ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 60.0)),
        ));
        // Every frame advances exactly 1/60 s, so scripted runs repeat frame for frame.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)));
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "RE Rematch (prototype)".into(), ..default() }), ..default() }).set(log));
    }
    app
        .add_plugins((UiMaterialPlugin::<reticle::ReticleMaterial>::default(), UiMaterialPlugin::<gauges::GaugeMaterial>::default(), MaterialPlugin::<pass_line::PassLineMaterial>::default()))
        .insert_resource(ClearColor(Color::srgb(0.56, 0.68, 0.8)))
        .init_resource::<Score>()
        .insert_resource(Bindings::load())
        .init_resource::<Actions>()
        .init_resource::<Axes>()
        .init_resource::<InputBlocked>()
        .init_resource::<menu::Menu>()
        .init_resource::<fx::Cheats>()
        .init_resource::<fx::FxState>()
        .add_event::<GoalScored>()
        .add_systems(Startup, (arena::spawn_arena, setup, menu::spawn_menu, reticle::spawn_reticle, gauges::spawn_gauges, pass_line::spawn_pass_line, fx::spawn_fx))
        .add_systems(
            Update,
            (
                menu::menu_keys,
                input::read_input,
                // Capture before clicks, so the click that opens a capture isn't bound.
                (menu::capture_binding, menu::menu_clicks, menu::drag_sliders, menu::refresh_labels).chain(),
                (player::grab_cursor, menu::show_menu).chain(),
                camera::camera_look,
                player::player_move,
                player::kicking,
                player::ball_call,
                player::ball_control,
                ball::ball_physics,
                (fx::apply_cheats, fx::kick_marker, fx::dribble_fx, fx::action_fx, fx::fade_glows, fx::ball_lines).chain(),
                (fx::trails, fx::volley_diamond, fx::effort_legs),
                on_goal,
                body::animate_body,
                camera::camera_follow,
                (update_hud, update_board, fx::debug_zones),
                reticle::update_reticle,
                pass_line::update_pass_line,
                gauges::update_gauges,
                auto_screenshot,
            )
                .chain(),
        )
        .run();
}

/// For checking visuals: `RE_SCREENSHOT=out.png` saves a frame after 2 s (or `RE_SHOT_AT` s), then quits.
/// `RE_VIEW=x,y,z[,tx,ty,tz]` looks from that point at the centre spot (or the target) instead;
/// `RE_VIEW=@x,y,z` looks at the player from that offset.
/// `RE_MENU=1` opens the Esc menu after 1 s; `RE_MENU=controls` or `RE_MENU=cheats` opens that
/// settings tab.
fn auto_screenshot(
    mut commands: Commands,
    time: Res<Time>,
    mut taken: Local<bool>,
    mut exit: EventWriter<AppExit>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    player: Single<&Transform, (With<Player>, Without<Camera3d>)>,
    mut menu: ResMut<menu::Menu>,
    target: Option<Res<HeadlessTarget>>,
) {
    let Ok(path) = std::env::var("RE_SCREENSHOT") else { return };
    let at = std::env::var("RE_SHOT_AT").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    if let Ok(page) = std::env::var("RE_MENU")
        && !menu.open()
        && !*taken
        && time.elapsed_secs() > 1.0
    {
        menu.open_scripted(&page);
    }
    if let Ok(view) = std::env::var("RE_VIEW") {
        let v: Vec<f32> = view.trim_start_matches('@').split(',').filter_map(|n| n.trim().parse().ok()).collect();
        let target = if v.len() == 6 { Vec3::new(v[3], v[4], v[5]) } else { Vec3::ZERO };
        if view.starts_with('@') && v.len() >= 3 {
            let at = player.translation - Vec3::Y * 0.3;
            **camera = Transform::from_translation(at + Vec3::new(v[0], v[1], v[2])).looking_at(at, Vec3::Y);
        } else if v.len() >= 3 {
            **camera = Transform::from_xyz(v[0], v[1], v[2]).looking_at(target, Vec3::Y);
        }
    }
    if !*taken && time.elapsed_secs() > at {
        let shot = target.map_or_else(Screenshot::primary_window, |t| Screenshot::image(t.0.clone()));
        commands.spawn(shot).observe(save_to_disk(path));
        *taken = true;
    }
    if time.elapsed_secs() > at + 1.0 {
        exit.write(AppExit::Success);
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let player = commands.spawn((Player::default(), Transform::from_xyz(0.0, 0.9, 6.0), Visibility::default())).id();
    body::spawn_body(&mut commands, player, &mut meshes, &mut materials);
    commands.spawn((
        Ball::default(),
        Mesh3d(meshes.add(Sphere::new(BALL_RADIUS))),
        MeshMaterial3d(materials.add(Color::srgb(0.95, 0.95, 0.9))),
        Transform::from_xyz(0.0, BALL_RADIUS, 0.0),
    ));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::from(PerspectiveProjection { fov: FOV.to_radians(), ..default() }),
            CameraRig::default(),
            camera::CameraState::default(),
            Transform::default(),
        ))
        .id();
    if headless() {
        let size = Extent3d { width: 1280, height: 720, depth_or_array_layers: 1 };
        let mut image = Image::new_fill(size, TextureDimension::D2, &[0, 0, 0, 255], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
        let handle = images.add(image);
        // Bevy only draws the UI (HUD, reticle, menu) on a window's camera unless told otherwise.
        commands.entity(camera).insert((Camera { target: RenderTarget::Image(handle.clone().into()), ..default() }, IsDefaultUiCamera));
        commands.insert_resource(HeadlessTarget(handle));
    }
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        Visibility::Hidden,
        Node { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(70.0), ..default() },
    ));
    // The scoreboard, as in Rematch: a dark banner at the top centre with the match clock between
    // the two teams' goals, each beside its colour.
    let text = |board: Board, size: f32, colour: Color| (board, Text::new(""), TextFont { font_size: size, ..default() }, TextColor(colour));
    let chip = |colour: Color| (Node { width: Val::Px(6.0), height: Val::Px(30.0), ..default() }, BackgroundColor(colour));
    commands
        .spawn(Node { position_type: PositionType::Absolute, top: Val::Px(12.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() })
        .with_children(|row| {
            row.spawn((
                Node { align_items: AlignItems::Center, column_gap: Val::Px(14.0), padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)), ..default() },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.85)),
                BorderRadius::all(Val::Px(4.0)),
            ))
            .with_children(|bar| {
                bar.spawn(chip(Color::srgb(0.10, 0.30, 0.85)));
                bar.spawn(text(Board::Left, 26.0, Color::WHITE));
                bar.spawn(text(Board::Clock, 20.0, Color::srgb(0.85, 0.88, 0.95)));
                bar.spawn(text(Board::Right, 26.0, Color::WHITE));
                bar.spawn(chip(Color::srgb(0.85, 0.15, 0.15)));
            });
        });
}

/// The scoreboard: the goals at each end and the time played.
fn update_board(score: Res<Score>, time: Res<Time>, mut texts: Query<(&Board, &mut Text)>) {
    let secs = time.elapsed_secs() as u32;
    for (board, mut text) in &mut texts {
        let s = match board {
            Board::Left => score.0[0].to_string(),
            Board::Clock => format!("{:02}:{:02}", secs / 60, secs % 60),
            Board::Right => score.0[1].to_string(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// After a goal: count it, put the ball back on the centre spot and the player in their half.
fn on_goal(
    mut goals: EventReader<GoalScored>,
    mut score: ResMut<Score>,
    mut ball: Single<(&mut Transform, &mut Ball)>,
    mut player: Single<(&mut Transform, &mut Player), Without<Ball>>,
) {
    for GoalScored(end) in goals.read() {
        score.0[(*end > 0.0) as usize] += 1;
        let (btf, b) = &mut *ball;
        btf.translation = Vec3::new(0.0, BALL_RADIUS, 0.0);
        **b = Ball::default();
        let (ptf, p) = &mut *player;
        ptf.translation = Vec3::new(0.0, 0.9, 6.0);
        p.has_ball = false;
        p.receiving = None;
        p.vel = Vec3::ZERO;
    }
}

/// The "Debug info" cheat's readout: your state, the kick and the ball.
fn update_hud(
    mut hud: Single<(&mut Text, &mut Visibility), With<Hud>>,
    player: Single<&Player>,
    ball: Single<&Ball>,
    axes: Res<Axes>,
    cheats: Res<fx::Cheats>,
) {
    let (hud, visibility) = &mut *hud;
    let shown = cheats.on(fx::Cheat::DebugInfo);
    visibility.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    if !shown {
        return;
    }
    let p = *player;
    let charge = match p.kick {
        Some(k) if !k.released => {
            let force = match k.kind {
                Kick::Shot => curve(&SHOT_FORCE, player::charge(k.kind, k.held)),
                Kick::Pass => curve(&PASS_FORCE, player::charge(k.kind, k.held)),
                Kick::Lob => curve(&LOB_FORCE, player::charge(k.kind, k.held)),
                _ => 0.0,
            };
            let curve_info = if k.kind == Kick::Shot { format!("  curve ({:+.2}, {:+.2})", axes.curve.x, axes.curve.y) } else { String::new() };
            format!("{:?} {:.2}s -> {:.1} m/s{}", k.kind, k.held, force, curve_info)
        }
        Some(k) => format!("{:?} {:.2}s{}", k.kind, k.elapsed, if k.released { " released" } else { "" }),
        _ => "-".into(),
    };
    let dribble = p.dribble.map_or("-".into(), |d| format!("{:?}{} {:.2}s", d.heading, if d.feint { " feint" } else { "" }, d.t));
    let text = format!(
        "Stamina {:4.1}/10{}   Extra effort {:3.0}%{}\nSpeed {:4.1} m/s{}{}\nCharge {}\nDribble {}   drag {}   pivot {:.2}\nControl {}   magnet off {:.2}   ball no-control {:.2}\nBall {:4.1} m/s{}\n\nEsc Menu   F1 Settings",
        p.stamina,
        if p.exhausted { " EXHAUSTED" } else { "" },
        p.extra_effort * 100.0,
        if p.extra_active { " ACTIVE" } else { "" },
        p.vel.length(),
        match (p.dribble, p.dash.is_some(), p.locked, p.sprint_running) {
            (Some(d), ..) if d.feint => "  FEINT",
            (Some(_), ..) => "  DRIBBLE",
            (_, true, ..) => "  DASH",
            (_, _, true, _) => "  lock move",
            (.., true) => "  sprint",
            _ => "",
        },
        if p.turn.is_some() { "  turning" } else { "" },
        charge,
        dribble,
        if p.run.is_some() { "on" } else { "-" },
        p.pivot.max(0.0),
        p.receiving.map_or("-".into(), |t| format!("{t:.2}s")),
        p.magnet_off.max(0.0),
        ball.no_control,
        ball.vel.length(),
        if ball.freeze > 0.0 { "  frozen" } else { "" },
    );
    // Set only when it changes: a changed text is laid out again.
    if hud.0 != text {
        hud.0 = text;
    }
}
