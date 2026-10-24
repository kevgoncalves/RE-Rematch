//! Effects and practice aids: the dribble's blue glow on the ball, the marker where a kick lands,
//! and the cheats from the F1 menu (infinite stamina and extra effort, the ball's trail and its
//! predicted path). The look is our own; when and where they show is the game's (notes/hud.md).

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::view::NoFrustumCulling;

use crate::ball::{Ball, step};
use crate::player::{Kick, Player};
use crate::tuning::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cheat {
    InfiniteStamina,
    InfiniteEffort,
    BallTrail,
    BallPath,
}

impl Cheat {
    pub const ALL: [Cheat; 4] = [Cheat::InfiniteStamina, Cheat::InfiniteEffort, Cheat::BallTrail, Cheat::BallPath];

    pub fn label(self) -> &'static str {
        match self {
            Cheat::InfiniteStamina => "Infinite stamina",
            Cheat::InfiniteEffort => "Infinite extra effort",
            Cheat::BallTrail => "Ball trail",
            Cheat::BallPath => "Ball path prediction",
        }
    }
}

#[derive(Resource, Default)]
pub struct Cheats([bool; 4]);

impl Cheats {
    pub fn on(&self, c: Cheat) -> bool {
        self.0[c as usize]
    }

    pub fn toggle(&mut self, c: Cheat) {
        self.0[c as usize] ^= true;
    }
}

pub fn apply_cheats(cheats: Res<Cheats>, mut player: Single<&mut Player>) {
    if cheats.on(Cheat::InfiniteStamina) {
        player.stamina = MAX_STAMINA;
        player.exhausted = false;
    }
    if cheats.on(Cheat::InfiniteEffort) {
        player.extra_effort = MAX_EXTRA_EFFORT;
    }
}

/// The ball's path from here, sampled at 60 Hz on its own 240 Hz physics, until it stops, goes in,
/// or `seconds` run out.
fn predict_path(pos: Vec3, ball: &Ball, seconds: f32) -> Vec<Vec3> {
    let (mut pos, mut ball) = (pos, ball.clone());
    let mut path = vec![pos];
    for i in 1..=(seconds * 240.0) as usize {
        let scored = step(&mut pos, &mut ball, 1.0 / 240.0).is_some();
        if i % 4 == 0 || scored || ball.vel == Vec3::ZERO {
            path.push(pos);
        }
        if scored || ball.vel == Vec3::ZERO {
            break;
        }
    }
    path
}

/// The cheats' lines: the loose ball's trail, its oldest points fading over 3 s, and its predicted
/// path, which fades over 1 s once the ball is controlled or stops.
pub fn ball_lines(
    time: Res<Time>,
    cheats: Res<Cheats>,
    ball: Single<(&Transform, &Ball)>,
    mut trail: Local<Vec<(Vec3, f32)>>,
    mut path: Local<(Vec<Vec3>, f32)>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs();
    let (tf, b) = *ball;
    for point in trail.iter_mut() {
        point.1 += dt;
    }
    trail.retain(|(_, age)| *age < 3.0);
    if !cheats.on(Cheat::BallTrail) {
        trail.clear();
    } else if !b.held && trail.last().is_none_or(|(p, _)| p.distance(tf.translation) > 0.05) {
        trail.push((tf.translation, 0.0));
    }
    let yellow = Color::srgb(1.0, 0.85, 0.2);
    gizmos.linestrip_gradient(trail.iter().map(|(p, age)| (*p, yellow.with_alpha(1.0 - age / 3.0))));
    if !cheats.on(Cheat::BallPath) {
        path.0.clear();
        return;
    }
    if !b.held && b.vel != Vec3::ZERO {
        *path = (predict_path(tf.translation, b, 6.0), 0.0);
    } else {
        path.1 += dt;
    }
    let fade = (1.0 - path.1).clamp(0.0, 1.0);
    let n = path.0.len().max(1) as f32;
    let cyan = Color::srgb(0.3, 0.9, 1.0);
    gizmos.linestrip_gradient(path.0.iter().enumerate().map(|(i, p)| (*p, cyan.with_alpha(fade * (1.0 - 0.6 * i as f32 / n)))));
}

/// Kicks this fast or faster get a marker (players confirm: a full pass or a shot shows one, a push
/// ball doesn't).
const MARKER_MIN_SPEED: f32 = 15.0;

/// Where a marker goes: the surface point, its normal, when the ball gets there and the way it is
/// going then.
struct MarkerSpot {
    at: Vec3,
    normal: Vec3,
    t: f32,
    way: Vec3,
}

/// Where a kicked ball first hits a wall, post or crossbar (the game marks every such bounce), or
/// else comes to rest: its own physics run on until it rolls slower than 0.5 m/s on the ground
/// (the game's prediction stop speed), up to 10 s. None if it is slower than `min_speed` (a marker
/// is for fast kicks), or it goes in.
fn marker_for(pos: Vec3, ball: &Ball, min_speed: f32) -> Option<MarkerSpot> {
    if ball.vel.length() < min_speed {
        return None;
    }
    let (mut pos, mut ball) = (pos, ball.clone());
    let dt = 1.0 / 240.0;
    for i in 1..=(10.0 / dt) as usize {
        let way = ball.vel.normalize_or_zero();
        if step(&mut pos, &mut ball, dt).is_some() {
            return None;
        }
        let t = i as f32 * dt;
        if let Some((at, normal)) = ball.hit {
            return Some(MarkerSpot { at, normal, t, way });
        }
        if pos.y <= BALL_RADIUS + 0.01 && Vec2::new(ball.vel.x, ball.vel.z).length() < 0.5 && ball.vel.y.abs() < 0.5 {
            return Some(MarkerSpot { at: Vec3::new(pos.x, 0.0, pos.z), normal: Vec3::Y, t, way });
        }
    }
    None
}

#[derive(Component)]
pub struct Marker {
    /// Seconds until the ball gets there (the marker shows no countdown: Rematch's doesn't).
    left: f32,
    /// On a wall or post: when the ball gets there the next marker follows from the bounce.
    chain: bool,
}

/// The marker's arrow: the way the ball comes into it, on the ground only.
#[derive(Component)]
pub struct MarkerArrow;

/// The volley diamond on the ball (GC_VolleyPredict).
#[derive(Component)]
pub struct VolleyDiamond;

/// A point where a volley could meet the ball, and when the ball gets there.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // the diamond only asks whether there is one; the volleys will aim at it
pub struct VolleySpot {
    pub at: Vec3,
    pub t: f32,
}

/// Whether you could volley the ball, and where (the volley abilities' own target query, which the
/// diamond shows). Your state must allow volleys: no lock move or dash, not your ball, not just
/// after your own kick. Then the ball must reach a point 0.33-0.6 s from now that is within 6 m,
/// 1.2 m behind to 6 m ahead of where you face, inside a side margin that widens to 5 m by 3 m
/// ahead, under 2.8 m high, and close enough to reach at 9.2 m/s (11 in extra effort).
pub fn volley_target(p: &Player, root: &Transform, pos: Vec3, ball: &Ball) -> Option<VolleySpot> {
    // ponytail: the ball's own no-control stands in for the game's HasLostBall and NoVolley timers.
    if p.locked || p.dash.is_some() || p.has_ball || p.kick.is_some() || p.receiving.is_some() || ball.held || ball.no_control > 0.0 {
        return None;
    }
    let forward = root.rotation * Vec3::NEG_Z;
    let side = root.rotation * Vec3::X;
    let feet = root.translation.y - 0.9;
    let reach = if p.extra_active { 11.0 } else { 9.2 };
    // The side margin: a smooth rise from 0 at 1.2 m behind to 5 m at 3 m ahead.
    let margin = |f: f32| {
        let u = ((f + 1.2) / 4.2).clamp(0.0, 1.0);
        let (h00, h10, h01) = (2.0 * u.powi(3) - 3.0 * u * u + 1.0, u.powi(3) - 2.0 * u * u + u, -2.0 * u.powi(3) + 3.0 * u * u);
        let _ = h00;
        h10 * 4.2 * 2.512 + h01 * 5.0
    };
    let (mut at, mut ball) = (pos, ball.clone());
    let dt = 1.0 / 240.0;
    for i in 1..=(0.6 / dt) as usize {
        if ball.vel != Vec3::ZERO && step(&mut at, &mut ball, dt).is_some() {
            return None;
        }
        let t = i as f32 * dt;
        if t < 0.33 || i % 4 != 0 {
            continue;
        }
        let rel = at - root.translation;
        let (f, s) = (rel.dot(forward), rel.dot(side));
        let fits = Vec2::new(f, s).length() <= 6.0
            && (-1.2..=6.0).contains(&f)
            && s.abs() < margin(f)
            && at.y - feet < 2.8
            && rel.length() / t <= reach;
        if fits {
            return Some(VolleySpot { at, t });
        }
    }
    None
}

/// The diamond: a square plate stood on a point with a round hole, 0.55 m tip to tip around a
/// 0.32 m hole, in the plane facing +Z.
fn diamond() -> Mesh {
    let n = 64;
    let (tip, hole) = (0.547 / 2.0, 0.315 / 2.0);
    let mut positions = Vec::new();
    for i in 0..=n {
        let a = TAU * i as f32 / n as f32;
        let (s, c) = a.sin_cos();
        let outer = tip / (c.abs() + s.abs());
        positions.push([c * hole, s * hole, 0.0]);
        positions.push([c * outer, s * outer, 0.0]);
    }
    let indices = (0..n as u32).flat_map(|i| [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 1, 2 * i + 3, 2 * i + 2]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// Shows the diamond on the ball while a volley is on (players' "lock" diamond): it fades in over
/// 0.23 s with a flash at 0.06 s, faces the camera, and goes at once when the volley is off.
pub fn volley_diamond(
    time: Res<Time>,
    player: Single<(&Player, &Transform), Without<VolleyDiamond>>,
    ball: Single<(&Transform, &Ball), (Without<Player>, Without<VolleyDiamond>)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut diamond: Single<(&mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>), With<VolleyDiamond>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shown: Local<f32>,
) {
    let ((p, root), (btf, b)) = (*player, *ball);
    let (tf, vis, mat) = &mut *diamond;
    if volley_target(p, root, btf.translation, b).is_none() {
        *shown = 0.0;
        **vis = Visibility::Hidden;
        return;
    }
    *shown += time.delta_secs();
    **vis = Visibility::Inherited;
    **tf = Transform::from_translation(btf.translation).looking_at(camera.translation(), Vec3::Y);
    let fade = (*shown / 0.23).min(1.0);
    let flash = 1.0 + 3.0 * (-((*shown - 0.058) / 0.03).powi(2)).exp();
    if let Some(m) = materials.get_mut(&mat.0) {
        let c = Vec3::new(0.0, 0.385, 1.65) * fade * flash * 0.6;
        m.base_color = Color::linear_rgb(c.x, c.y, c.z);
    }
}

#[derive(Resource)]
pub struct FxAssets {
    sphere: Handle<Mesh>,
    /// A thin ring of radius 1, flat in its XY plane.
    ring: Handle<Mesh>,
}

/// Whether the kicked ball draws its trail: from a kick until anyone gains the ball.
#[derive(Resource, Default)]
pub struct FxState {
    kick_trail: bool,
}

/// The marker's ring, about 80 cm across (SM_Bounce_Circle, 75 x 86 cm).
const MARKER_RADII: (f32, f32) = (0.36, 0.43);

/// A flat ring (in the local XZ plane), filled to `fill` of the way round.
/// The marker's colours: the game's effect hue row starts blue and slides to cyan (its sheen).
const MARKER_BLUE: Vec3 = Vec3::new(0.0, 0.25, 0.84);
const MARKER_CYAN: Vec3 = Vec3::new(0.0, 0.68, 0.74);

fn ring() -> Mesh {
    let n = 48;
    let (inner, outer) = MARKER_RADII;
    let mut positions = Vec::new();
    let mut colours = Vec::new();
    for i in 0..=n {
        let a = FRAC_PI_2 + TAU * i as f32 / n as f32;
        let (s, c) = a.sin_cos();
        positions.push([c * inner, 0.0, -s * inner]);
        positions.push([c * outer, 0.0, -s * outer]);
        // Cyan on the inner edge, blue on the outer, the sheen sliding round the ring.
        let sheen = 0.5 + 0.5 * (a * 2.0).sin();
        colours.push((MARKER_CYAN.lerp(MARKER_BLUE, 0.3 * sheen) * 1.6).extend(1.0).to_array());
        colours.push((MARKER_BLUE * 1.4).extend(1.0).to_array());
    }
    let indices = (0..n as u32).flat_map(|i| [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 1, 2 * i + 3, 2 * i + 2]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// The soft glow inside the marker: blue at the centre fading out at the ring.
fn glow_disc() -> Mesh {
    let n = 32;
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut colours = vec![(MARKER_BLUE * 0.55).extend(1.0).to_array()];
    for i in 0..=n {
        let a = TAU * i as f32 / n as f32;
        positions.push([a.cos() * MARKER_RADII.1, 0.0, a.sin() * MARKER_RADII.1]);
        colours.push([0.0, 0.0, 0.0, 1.0]);
    }
    let indices = (1..=n as u32).flat_map(|i| [0, i + 1, i]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// The marker's arrow: a thin streak from the ring out along local -Z, fading toward its tip, with
/// a chevron head.
fn marker_arrow() -> Mesh {
    let (start, length, half) = (MARKER_RADII.1 + 0.08, 1.1, 0.035);
    let bright = (MARKER_CYAN * 1.3).extend(1.0).to_array();
    let dim = (MARKER_BLUE * 0.25).extend(1.0).to_array();
    let positions = vec![
        [-half, 0.0, -start],
        [half, 0.0, -start],
        [-half * 0.4, 0.0, -start - length],
        [half * 0.4, 0.0, -start - length],
        // The head, at the near end, pointing on.
        [-0.12, 0.0, -start - 0.02],
        [0.12, 0.0, -start - 0.02],
        [0.0, 0.0, -start - 0.2],
    ];
    let colours = vec![bright, bright, dim, dim, bright, bright, bright];
    let indices = vec![0, 2, 1, 1, 2, 3, 4, 6, 5];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

pub fn spawn_fx(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // The marker (NS_Bounce): a glowing blue ring, a soft glow inside it and an arrow the way the
    // ball comes in, all added light drawn over the ground.
    let flat = |c: Color, mode: AlphaMode| StandardMaterial { base_color: c, unlit: true, alpha_mode: mode, double_sided: true, cull_mode: None, depth_bias: 50.0, ..default() };
    let mesh = meshes.add(ring());
    let light = materials.add(flat(Color::WHITE, AlphaMode::Add));
    commands
        .spawn((Marker { left: 0.0, chain: false }, Transform::default(), Visibility::Hidden))
        .with_children(|m| {
            m.spawn((NotShadowCaster, Mesh3d(mesh), MeshMaterial3d(light.clone()), Transform::from_xyz(0.0, 0.02, 0.0)));
            m.spawn((NotShadowCaster, Mesh3d(meshes.add(glow_disc())), MeshMaterial3d(light.clone()), Transform::from_xyz(0.0, 0.015, 0.0)));
            m.spawn((MarkerArrow, NotShadowCaster, Mesh3d(meshes.add(marker_arrow())), MeshMaterial3d(light), Transform::from_xyz(0.0, 0.02, 0.0)));
        });
    commands.spawn((
        VolleyDiamond,
        NotShadowCaster,
        Mesh3d(meshes.add(diamond())),
        MeshMaterial3d(materials.add(flat(Color::BLACK, AlphaMode::Add))),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.insert_resource(FxAssets { sphere: meshes.add(Sphere::new(BALL_RADIUS)), ring: meshes.add(Annulus::new(0.88, 1.0)) });
    // The trails: vertex-coloured ribbons added onto the scene.
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, ..default() });
    for (source, effect) in [(Source::Ball, Effect::Dribble), (Source::Foot(-1.0), Effect::ExtraEffort), (Source::Foot(1.0), Effect::ExtraEffort), (Source::Ball, Effect::Kick)] {
        let mesh = meshes.add(ribbon(&[], Vec3::ZERO, 0.0, 1.0, Vec3::ZERO));
        // Rebuilt every frame wherever the ball goes, so its first bounds would cull it later.
        commands.spawn((NotShadowCaster, NoFrustumCulling, Trail { source, effect, points: Vec::new(), mesh: mesh.clone() }, Mesh3d(mesh), MeshMaterial3d(glow.clone()), Transform::default(), Visibility::default()));
    }
}

/// What a trail follows: the ball, or a foot (-1 left, +1 right).
#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    Ball,
    Foot(f32),
}

/// A foot's trail point, on the boot (set by the body).
#[derive(Component)]
pub struct TrailPoint(pub f32);

/// Which move a trail belongs to.
#[derive(Clone, Copy, PartialEq)]
pub enum Effect {
    Dribble,
    ExtraEffort,
    /// The kicked ball's trail.
    Kick,
}

impl Effect {
    /// Colour (linear), how long a point lives and the trail's width.
    fn look(self) -> (Vec3, f32, f32) {
        match self {
            // The dribble's blue (NS_Dribble's colour, 0.3 / 1.05 / 5.0, scaled down to display).
            Effect::Dribble => (Vec3::new(0.06, 0.21, 1.0), 0.25, 0.16),
            // Extra effort's streaks on the legs (its rays: 00FFCB, 56FFFD, 4BB5FF).
            Effect::ExtraEffort => (Vec3::new(0.1, 1.0, 0.85), 0.2, 0.1),
            // The kicked ball's trail: additive at 0.55 (NS_Ball_Trail; its colour isn't in the data).
            Effect::Kick => (Vec3::new(0.5, 0.55, 0.6), 0.45, 0.2),
        }
    }
}

#[derive(Component)]
pub struct Trail {
    source: Source,
    effect: Effect,
    /// Positions, newest last, with their age.
    points: Vec<(Vec3, f32)>,
    mesh: Handle<Mesh>,
}

/// A ribbon through `points` (newest last), facing `eye`, tapering and fading with age.
fn ribbon(points: &[(Vec3, f32)], eye: Vec3, width: f32, life: f32, colour: Vec3) -> Mesh {
    let mut positions = Vec::new();
    let mut colours = Vec::new();
    for (i, &(p, age)) in points.iter().enumerate() {
        let prev = points[i.saturating_sub(1)].0;
        let next = points[(i + 1).min(points.len() - 1)].0;
        let fresh = (1.0 - age / life).clamp(0.0, 1.0);
        let side = (next - prev).cross(eye - p).normalize_or_zero() * width * 0.5 * fresh;
        positions.push((p + side).to_array());
        positions.push((p - side).to_array());
        let c = (colour * fresh * fresh).extend(1.0).to_array();
        colours.push(c);
        colours.push(c);
    }
    let n = points.len().saturating_sub(1) as u32;
    let indices = (0..n).flat_map(|i| [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 1, 2 * i + 3, 2 * i + 2]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// The moves' trails: a dribble streaks blue behind the ball (Kev wants none on the foot), extra
/// effort behind both feet. Points age out after the effect's life.
pub fn trails(
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<&GlobalTransform, With<Ball>>,
    feet: Query<(&TrailPoint, &GlobalTransform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    state: Res<FxState>,
    mut trails: Query<&mut Trail>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    let eye = camera.translation();
    for mut trail in &mut trails {
        let (colour, life, width) = trail.effect.look();
        let on = match trail.effect {
            Effect::Dribble => player.dribble.is_some_and(|d| d.t < 0.45),
            Effect::ExtraEffort => player.extra_active,
            Effect::Kick => state.kick_trail,
        };
        for point in &mut trail.points {
            point.1 += dt;
        }
        trail.points.retain(|(_, age)| *age < life);
        if on {
            let at = match trail.source {
                Source::Ball => Some(ball.translation()),
                Source::Foot(side) => feet.iter().find(|(f, _)| f.0 == side).map(|(_, g)| g.translation()),
            };
            if let Some(at) = at {
                trail.points.push((at, 0.0));
            }
        }
        if let Some(mesh) = meshes.get_mut(&trail.mesh) {
            *mesh = ribbon(&trail.points, eye, width, life, colour);
        }
    }
}

/// The kick marker: placed where a fast kick will first hit a wall or post (lying on it), or else
/// where it will stop, until the ball gets there. At a wall or post the next
/// one follows from the bounce; it is gone when the ball has nearly stopped on it, when anyone
/// gains the ball, or at the next kick.
pub fn kick_marker(
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<(&Transform, &Ball)>,
    mut marker: Single<(&mut Marker, &mut Transform, &mut Visibility), Without<Ball>>,
    mut arrow: Single<(&mut Transform, &mut Visibility), (With<MarkerArrow>, Without<Marker>, Without<Ball>)>,
    mut last_kick: Local<Option<Kick>>,
) {
    let (btf, b) = *ball;
    let (m, tf, vis) = &mut *marker;
    let (atf, avis) = &mut *arrow;
    let kicked = last_kick.is_some() && player.kick.is_none() && !b.held;
    *last_kick = player.kick.map(|k| k.kind);
    let arrived = m.chain && m.left > 0.0 && m.left <= time.delta_secs();
    let spot = if kicked {
        m.left = 0.0;
        marker_for(btf.translation, b, MARKER_MIN_SPEED)
    } else if arrived {
        m.left = 0.0;
        marker_for(btf.translation, b, 0.0)
    } else {
        None
    };
    if let Some(s) = spot {
        tf.translation = s.at + s.normal * 0.01;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, s.normal);
        (m.left, m.chain) = (s.t, s.normal != Vec3::Y);
        let flat = Vec3::new(s.way.x, 0.0, s.way.z);
        **avis = if m.chain || flat.length() < 0.1 { Visibility::Hidden } else { Visibility::Inherited };
        atf.rotation = Quat::from_rotation_y((-flat.x).atan2(-flat.z));
    }
    if b.held || player.kick.is_some() {
        m.left = 0.0;
    }
    m.left -= time.delta_secs();
    **vis = if m.left > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
}

/// The legs' materials (shins and boots), which glow in extra effort.
#[derive(Resource)]
pub struct LegMaterials(pub [Handle<StandardMaterial>; 2]);

/// Extra effort lights the legs cyan for as long as it lasts (its leg rays and body overlay), with a
/// slow pulse; it eases in and out over 0.15 s.
pub fn effort_legs(time: Res<Time>, player: Single<&Player>, legs: Res<LegMaterials>, mut materials: ResMut<Assets<StandardMaterial>>, mut glow: Local<f32>) {
    let target = if player.extra_active { 1.0 } else { 0.0 };
    *glow += (target - *glow).clamp(-time.delta_secs() / 0.15, time.delta_secs() / 0.15);
    let pulse = 0.8 + 0.2 * (time.elapsed_secs() * 12.0).sin();
    let c = Vec3::new(0.0, 1.0, 0.8) * *glow * pulse * 1.5;
    for handle in &legs.0 {
        if let Some(m) = materials.get_mut(handle) {
            m.emissive = LinearRgba::rgb(c.x, c.y, c.z);
        }
    }
}

/// A fading glow: the dribble's shell on the ball.
#[derive(Component)]
pub struct Glow {
    age: f32,
    life: f32,
    alpha: f32,
    /// Scale and colour (linear) from and to, over its life.
    scale: (f32, f32),
    colour: (Vec3, Vec3),
    follow: bool,
}

impl Glow {
    fn new(life: f32, alpha: f32, scale: (f32, f32), colour: (Vec3, Vec3), follow: bool) -> Glow {
        Glow { age: 0.0, life, alpha, scale, colour, follow }
    }
}

const BLUE: Vec3 = Vec3::new(0.06, 0.21, 1.0);
const ORANGE: Vec3 = Vec3::new(1.0, 0.42, 0.08);
const WHITE: Vec3 = Vec3::ONE;

fn spawn_glow(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, mesh: &Handle<Mesh>, at: Vec3, rotation: Quat, glow: Glow) {
    let material = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, ..default() });
    commands.spawn((NotShadowCaster, Mesh3d(mesh.clone()), MeshMaterial3d(material), Transform { translation: at, rotation, scale: Vec3::splat(glow.scale.0) }, glow));
}

/// The dribble's glow on the ball (GC_Dribble, NS_Dribble): a blue shell riding the ball through
/// the move; the trails do the rest.
pub fn dribble_fx(
    mut commands: Commands,
    player: Single<&Player>,
    ball: Single<&Transform, With<Ball>>,
    fx: Res<FxAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<Option<f32>>,
) {
    let started = player.dribble.is_some_and(|d| last.is_none_or(|t| d.t < t));
    *last = player.dribble.map(|d| d.t);
    if started {
        spawn_glow(&mut commands, &mut materials, &fx.sphere, ball.translation, Quat::IDENTITY, Glow::new(0.5, 0.9, (1.25, 1.25), (BLUE, BLUE), true));
    }
}

/// The moves' bursts (gameplay_fx data): a kick's impact where the ball was, sized by the kick
/// (shots by power: low under 29.4 m/s, mid under 34.8, high above, with more rings each step); the
/// kicked ball's trail until anyone gains it (not for push balls on the ground); a control's two
/// rings closing on the ball, orange to blue, as it pulses.
pub fn action_fx(
    mut commands: Commands,
    player: Single<&Player>,
    ball: Single<(&Transform, &Ball)>,
    fx: Res<FxAssets>,
    mut state: ResMut<FxState>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_kick: Local<Option<Kick>>,
    mut was_receiving: Local<bool>,
) {
    let (p, (btf, b)) = (*player, *ball);
    let at = btf.translation;
    let kicked = last_kick.filter(|_| p.kick.is_none() && !b.held);
    *last_kick = p.kick.map(|k| k.kind);
    let flat = |v: Vec3| Quat::from_rotation_arc(Vec3::Z, v.normalize_or(Vec3::NEG_Z));
    if let Some(kind) = kicked {
        let speed = b.vel.length();
        let facing = flat(b.vel);
        let (rings, size) = match kind {
            Kick::Shot if speed >= 34.82 => (2, 1.6),
            Kick::Shot if speed >= SHOT_HIGH_POWER_SPEED => (1, 1.2),
            Kick::Shot => (0, 1.0),
            Kick::PushBall | Kick::DribblePushBall => (1, 0.5),
            _ => (1, 0.8),
        };
        let glow = |c: &mut Commands, m: &mut Assets<StandardMaterial>, mesh: &Handle<Mesh>, rot: Quat, g: Glow| spawn_glow(c, m, mesh, at, rot, g);
        glow(&mut commands, &mut materials, &fx.sphere, Quat::IDENTITY, Glow::new(0.05, 0.6, (1.5 * size, 2.8 * size), (WHITE, WHITE), false));
        glow(&mut commands, &mut materials, &fx.sphere, Quat::IDENTITY, Glow::new(0.22, 0.15, (1.3 * size, 3.2 * size), (Vec3::new(0.2, 0.9, 1.0), BLUE), false));
        for r in 0..rings {
            let reach = size * (0.35 + 0.2 * r as f32);
            glow(&mut commands, &mut materials, &fx.ring, facing, Glow::new(0.22 + 0.05 * r as f32, 0.45, (0.1, reach), (Vec3::new(1.0, 0.95, 0.3), Vec3::new(0.9, 0.2, 0.8)), false));
        }
        state.kick_trail = !matches!(kind, Kick::PushBall | Kick::DribblePushBall) || at.y > 0.75;
    }
    if b.held {
        state.kick_trail = false;
    }
    // A control shows on the ball only (players confirm): two rings closing on it, facing you, as
    // the ball pulses.
    if p.receiving.is_some() && !*was_receiving {
        let facing = Transform::from_translation(at).looking_at(camera.translation(), Vec3::Y).rotation;
        for life in [0.25, 0.35] {
            spawn_glow(&mut commands, &mut materials, &fx.ring, at, facing, Glow::new(life, 0.5, (0.45, 0.14), (ORANGE, BLUE), true));
        }
        spawn_glow(&mut commands, &mut materials, &fx.sphere, at, Quat::IDENTITY, Glow::new(0.45, 0.5, (1.5, 1.0), (WHITE, BLUE), true));
    }
    *was_receiving = p.receiving.is_some();
}

pub fn fade_glows(
    mut commands: Commands,
    time: Res<Time>,
    ball: Single<&Transform, (With<Ball>, Without<Glow>)>,
    mut glows: Query<(Entity, &mut Glow, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, mut g, mut tf, mat) in &mut glows {
        g.age += time.delta_secs();
        let u = g.age / g.life;
        if u >= 1.0 {
            materials.remove(&mat.0);
            commands.entity(e).despawn();
            continue;
        }
        if g.follow {
            tf.translation = ball.translation;
        }
        // Eased out: fast at first, settling.
        let e = 1.0 - (1.0 - u) * (1.0 - u);
        tf.scale = Vec3::splat(g.scale.0 + (g.scale.1 - g.scale.0) * e);
        if let Some(m) = materials.get_mut(&mat.0) {
            // Additive: fading the colour fades the glow.
            let c = g.colour.0.lerp(g.colour.1, u) * g.alpha * (1.0 - u);
            m.base_color = Color::linear_rgb(c.x, c.y, c.z);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volley_target_needs_a_reachable_ball_in_front() {
        let root = Transform::from_xyz(0.0, 0.9, 0.0);
        let p = Player::default();
        // A ball rolling at you from 5 m ahead comes within reach.
        let rolling = Ball { vel: Vec3::new(0.0, 0.0, 8.0), friction: PASS_FRICTION, ..default() };
        assert!(volley_target(&p, &root, Vec3::new(0.0, BALL_RADIUS, -5.0), &rolling).is_some());
        // Too far, too high, behind you, or in lock move: no volley.
        let still = Ball::default();
        assert!(volley_target(&p, &root, Vec3::new(0.0, BALL_RADIUS, -9.0), &still).is_none());
        assert!(volley_target(&p, &root, Vec3::new(0.0, 6.0, -2.0), &Ball { vel: Vec3::Y * 3.0, ..default() }).is_none());
        assert!(volley_target(&p, &root, Vec3::new(0.0, BALL_RADIUS, 3.0), &still).is_none());
        let locked = Player { locked: true, ..default() };
        assert!(volley_target(&locked, &root, Vec3::new(0.0, BALL_RADIUS, -2.0), &still).is_none());
        // A still ball close in front shows it too.
        assert!(volley_target(&p, &root, Vec3::new(0.3, BALL_RADIUS, -2.0), &still).is_some());
    }

    #[test]
    fn markers_mark_where_a_fast_kick_stops_or_first_hits_a_wall() {
        let at = Vec3::new(0.0, BALL_RADIUS, 0.0);
        // A full pass: marked where it stops, a while later.
        let pass = Ball { vel: Vec3::new(0.0, 0.0, -25.0), friction: PASS_FRICTION, ..default() };
        let s = marker_for(at, &pass, MARKER_MIN_SPEED).expect("a full pass is marked");
        let (mut pos, mut rolled) = (at, pass.clone());
        for _ in 0..(s.t * 240.0) as usize {
            step(&mut pos, &mut rolled, 1.0 / 240.0);
        }
        assert!(Vec2::new(pos.x - s.at.x, pos.z - s.at.z).length() < 0.05 && s.t > 1.0 && s.normal == Vec3::Y, "{} at {}", s.at, s.t);
        // A push ball is too slow to get one, and a ball going in gets none.
        assert!(marker_for(at, &Ball { vel: Vec3::new(0.0, 0.0, -PUSH_BALL_SPEED), friction: PUSH_BALL_FRICTION, ..default() }, MARKER_MIN_SPEED).is_none());
        let shot = Ball { vel: Vec3::new(0.0, 1.0, -35.0), ..default() };
        assert!(marker_for(Vec3::new(0.0, BALL_RADIUS, -30.0), &shot, MARKER_MIN_SPEED).is_none());
        // A shot wide of the goal is marked on the end wall, lying on it.
        let wide = Ball { vel: Vec3::new(0.0, 1.0, -35.0), ..default() };
        let s = marker_for(Vec3::new(GOAL_HALF_WIDTH + 3.0, BALL_RADIUS, -20.0), &wide, MARKER_MIN_SPEED).expect("marked on the wall");
        assert!(s.normal == Vec3::Z && (s.at.z + HALF_LENGTH).abs() < 0.01, "{} {}", s.at, s.normal);
    }
}
