//! RE Rematch prototype: one player, the ball and a 73 x 49 arena, tuned with Rematch's own values.

mod ball;
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
        .insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.09)))
        .init_resource::<Score>()
        .insert_resource(Bindings::load())
        .init_resource::<Actions>()
        .init_resource::<Axes>()
        .init_resource::<InputBlocked>()
        .init_resource::<menu::Menu>()
        .add_event::<GoalScored>()
        .add_systems(Startup, (setup, menu::spawn_menu, reticle::spawn_reticle, gauges::spawn_gauges, pass_line::spawn_pass_line))
        .add_systems(
            Update,
            (
                menu::toggle_menu,
                input::read_input,
                // Capture before clicks, so the click that opens a capture isn't bound.
                menu::capture_binding,
                menu::menu_clicks,
                menu::refresh_labels,
                player::grab_cursor,
                player::camera_look,
                player::player_move,
                player::kicking,
                player::ball_call,
                player::ball_control,
                ball::ball_physics,
                on_goal,
                player::camera_follow,
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

/// For checking visuals: `RE_SCREENSHOT=out.png` saves a frame after 2 s, then quits.
fn auto_screenshot(mut commands: Commands, time: Res<Time>, mut taken: Local<bool>, mut exit: EventWriter<AppExit>) {
    let Ok(path) = std::env::var("RE_SCREENSHOT") else { return };
    if !*taken && time.elapsed_secs() > 2.0 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *taken = true;
    }
    if time.elapsed_secs() > 3.0 {
        exit.write(AppExit::Success);
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (w, l) = (HALF_WIDTH * 2.0, HALF_LENGTH * 2.0);

    // Pitch, with a halfway line and the goalkeeper areas painted on (arena.md).
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(w, l))),
        MeshMaterial3d(materials.add(Color::srgb(0.16, 0.42, 0.2))),
    ));
    let line = materials.add(Color::srgba(1.0, 1.0, 1.0, 0.7));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(w, 0.01, 0.12))), MeshMaterial3d(line.clone()), Transform::from_xyz(0.0, 0.005, 0.0)));
    for end in [-1.0f32, 1.0] {
        let (gk_w, gk_d) = (33.2, 15.7);
        let z = end * (HALF_LENGTH - gk_d);
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(gk_w, 0.01, 0.12))), MeshMaterial3d(line.clone()), Transform::from_xyz(0.0, 0.005, z)));
        for side in [-1.0f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.01, gk_d))),
                MeshMaterial3d(line.clone()),
                Transform::from_xyz(side * gk_w / 2.0, 0.005, end * (HALF_LENGTH - gk_d / 2.0)),
            ));
        }
    }

    // Glass walls: sides full length, ends with the goal mouth cut out.
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.6, 0.8, 1.0, 0.12),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let wall_h = 6.0; // visual only: physics walls go to the 20.5 m ceiling
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.1, wall_h, l))),
            MeshMaterial3d(glass.clone()),
            Transform::from_xyz(side * HALF_WIDTH, wall_h / 2.0, 0.0),
        ));
    }
    let post = materials.add(Color::srgb(0.95, 0.95, 0.95));
    for end in [-1.0f32, 1.0] {
        let z = end * HALF_LENGTH;
        let side_w = HALF_WIDTH - GOAL_HALF_WIDTH;
        for side in [-1.0f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(side_w, wall_h, 0.1))),
                MeshMaterial3d(glass.clone()),
                Transform::from_xyz(side * (GOAL_HALF_WIDTH + side_w / 2.0), wall_h / 2.0, z),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(POST_RADIUS, GOAL_HEIGHT))),
                MeshMaterial3d(post.clone()),
                Transform::from_xyz(side * GOAL_HALF_WIDTH, GOAL_HEIGHT / 2.0, z),
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(GOAL_HALF_WIDTH * 2.0, wall_h - GOAL_HEIGHT, 0.1))),
            MeshMaterial3d(glass.clone()),
            Transform::from_xyz(0.0, GOAL_HEIGHT + (wall_h - GOAL_HEIGHT) / 2.0, z),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(GOAL_HALF_WIDTH * 2.0, POST_RADIUS * 2.0, POST_RADIUS * 2.0))),
            MeshMaterial3d(post.clone()),
            Transform::from_xyz(0.0, GOAL_HEIGHT, z),
        ));
        // Net: back panel behind the line.
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(GOAL_HALF_WIDTH * 2.0, GOAL_HEIGHT, 0.05))),
            MeshMaterial3d(materials.add(Color::srgba(1.0, 1.0, 1.0, 0.25))),
            Transform::from_xyz(0.0, GOAL_HEIGHT / 2.0, end * (HALF_LENGTH + GOAL_DEPTH)),
        ));
    }

    commands.spawn((
        DirectionalLight { illuminance: 8000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(10.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight { brightness: 400.0, ..default() });

    commands.spawn((
        Player::default(),
        Mesh3d(meshes.add(Capsule3d::new(0.35, 1.1))),
        MeshMaterial3d(materials.add(Color::srgb(0.9, 0.35, 0.2))),
        Transform::from_xyz(0.0, 0.9, 6.0),
    ));
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
                Kick::Shot => curve(&SHOT_FORCE, k.held.max(SHOT_BUILDUP)),
                Kick::Pass => curve(&PASS_FORCE, k.held.max(PASS_BUILDUP)),
                Kick::Lob => curve(&LOB_FORCE, k.held.max(LOB_BUILDUP)),
                _ => 0.0,
            };
            let curve_info = if k.kind == Kick::Shot { format!("  curve ({:+.2}, {:+.2})", axes.curve.x, axes.curve.y) } else { String::new() };
            format!("{:?} {:.2}s -> {:.1} m/s{}", k.kind, k.held, force, curve_info)
        }
        _ => "-".into(),
    };
    hud.0 = format!(
        "Score  {} - {}\nStamina {:4.1}/10{}   Extra effort {:3.0}%{}\nSpeed {:4.1} m/s{}\nCharge {}\nBall {:4.1} m/s\n\nF1 Controls",
        score.0[0],
        score.0[1],
        p.stamina,
        if p.exhausted { " EXHAUSTED" } else { "" },
        p.extra_effort * 100.0,
        if p.extra_active { " ACTIVE" } else { "" },
        p.vel.length(),
        match (p.dribble, p.dash.is_some(), p.locked, p.sprinting) {
            (Some(d), ..) if d.feint => "  FEINT",
            (Some(_), ..) => "  DRIBBLE",
            (_, true, ..) => "  DASH",
            (_, _, true, _) => "  lock move",
            (.., true) => "  sprint",
            _ => "",
        },
        charge,
        ball.vel.length(),
    );
}
