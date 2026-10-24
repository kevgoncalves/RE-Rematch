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

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use ball::{Ball, GoalScored};
use input::{Actions, Axes, Bindings, InputBlocked};
use player::{CameraRig, Kick, Player};
use tuning::*;

#[derive(Resource, Default)]
struct Score([u32; 2]);

#[derive(Component)]
struct Hud;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: Some(Window { title: "RE Rematch (prototype)".into(), ..default() }), ..default() })
                // The Vulkan loader logs stale overlay layers some launchers leave registered; harmless noise.
                .set(bevy::log::LogPlugin { filter: "wgpu=error,naga=warn,wgpu_hal::vulkan::instance=off".into(), ..default() }),
        )
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
                menu::toggle_menu,
                input::read_input,
                // Capture before clicks, so the click that opens a capture isn't bound.
                (menu::capture_binding, menu::menu_clicks, menu::drag_sliders, menu::refresh_labels).chain(),
                player::grab_cursor,
                camera::camera_look,
                player::player_move,
                player::kicking,
                player::ball_call,
                player::ball_control,
                ball::ball_physics,
                (fx::apply_cheats, fx::kick_marker, fx::dribble_fx, fx::action_fx, fx::fade_glows, fx::ball_lines).chain(),
                (fx::trails, fx::ball_shadow, fx::volley_diamond),
                on_goal,
                body::animate_body,
                camera::camera_follow,
                update_hud,
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
/// `RE_MENU=1` opens the F1 menu first.
fn auto_screenshot(
    mut commands: Commands,
    time: Res<Time>,
    mut taken: Local<bool>,
    mut exit: EventWriter<AppExit>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    player: Single<&Transform, (With<Player>, Without<Camera3d>)>,
    mut menu: ResMut<menu::Menu>,
    mut menu_root: Single<&mut Node, With<menu::MenuRoot>>,
) {
    let Ok(path) = std::env::var("RE_SCREENSHOT") else { return };
    let at = std::env::var("RE_SHOT_AT").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    if std::env::var("RE_MENU").is_ok() && !*taken && time.elapsed_secs() > 1.0 {
        menu.open = true;
        menu_root.display = Display::Flex;
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
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *taken = true;
    }
    if time.elapsed_secs() > at + 1.0 {
        exit.write(AppExit::Success);
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let player = commands.spawn((Player::default(), Transform::from_xyz(0.0, 0.9, 6.0), Visibility::default())).id();
    body::spawn_body(&mut commands, player, &mut meshes, &mut materials);
    commands.spawn((
        Ball::default(),
        Mesh3d(meshes.add(Sphere::new(BALL_RADIUS))),
        MeshMaterial3d(materials.add(Color::srgb(0.95, 0.95, 0.9))),
        Transform::from_xyz(0.0, BALL_RADIUS, 0.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection { fov: FOV.to_radians(), ..default() }),
        CameraRig::default(),
        camera::CameraState::default(),
        Transform::default(),
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: 18.0, ..default() },
        Node { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(10.0), ..default() },
    ));
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

fn update_hud(
    mut hud: Single<&mut Text, With<Hud>>,
    player: Single<&Player>,
    ball: Single<&Ball>,
    score: Res<Score>,
    axes: Res<Axes>,
) {
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
        _ => "-".into(),
    };
    hud.0 = format!(
        "Score  {} - {}\nStamina {:4.1}/10{}   Extra effort {:3.0}%{}\nSpeed {:4.1} m/s{}{}\nCharge {}\nBall {:4.1} m/s\n\nF1 Controls",
        score.0[0],
        score.0[1],
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
        ball.vel.length(),
    );
}
