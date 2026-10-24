//! Effects and practice aids: the dribble's blue glow on the ball, the marker where a kick lands,
//! and the cheats from the F1 menu (infinite stamina and extra effort, the ball's trail and its
//! predicted path). The look is our own; when and where they show is the game's (notes/hud.md).

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

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

/// The cheats' lines: the loose ball's trail and its predicted path.
pub fn ball_lines(cheats: Res<Cheats>, ball: Single<(&Transform, &Ball)>, mut trail: Local<Vec<Vec3>>, mut gizmos: Gizmos) {
    let (tf, b) = *ball;
    if !cheats.on(Cheat::BallTrail) || b.held {
        trail.clear();
    } else if trail.last().is_none_or(|p| p.distance(tf.translation) > 0.05) {
        trail.push(tf.translation);
        if trail.len() > 1200 {
            trail.remove(0);
        }
    }
    gizmos.linestrip(trail.iter().copied(), Color::srgb(1.0, 0.85, 0.2));
    if cheats.on(Cheat::BallPath) && !b.held && b.vel != Vec3::ZERO {
        gizmos.linestrip(predict_path(tf.translation, b, 6.0), Color::srgb(0.3, 0.9, 1.0));
    }
}

/// Where a kick's marker goes (BP_BallVFXComponent): the spot, how long the ball takes to get there,
/// the surface's normal (up on the ground, out of a wall) and the ball's way on from it.
#[derive(Clone, Copy, Debug)]
struct Spot {
    at: Vec3,
    t: f32,
    normal: Vec3,
    way: Vec3,
}

/// A pass marks the point it reaches 2.5 s on (a short one 0.9 s), or sooner where it first hits a
/// wall or stops; a lob or a shot marks its first touch of anything, ground, wall or post; a push
/// ball has none. Only that first marker is shown.
fn marker_for(kind: Kick, pos: Vec3, ball: &Ball) -> Option<Spot> {
    let ahead = match kind {
        Kick::Pass => 2.5,
        Kick::ShortPass => 0.9,
        Kick::Shot | Kick::Lob | Kick::ShortLob => 6.0,
        Kick::PushBall | Kick::DribblePushBall => return None,
    };
    let aerial = matches!(kind, Kick::Shot | Kick::Lob | Kick::ShortLob);
    let (mut pos, mut ball) = (pos, ball.clone());
    let mut airborne = false;
    let dt = 1.0 / 240.0;
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let way = |v: Vec3| flat(v).normalize_or(Vec3::NEG_Z);
    for i in 1..=(ahead / dt) as usize {
        let before = flat(ball.vel);
        if step(&mut pos, &mut ball, dt).is_some() {
            return None;
        }
        let t = i as f32 * dt;
        let on_ground = pos.y <= BALL_RADIUS + 0.01;
        airborne |= !on_ground;
        let after = flat(ball.vel);
        if before.length() > 1.0 && after.length() > 0.1 && before.normalize().dot(after.normalize()) < 0.5 {
            // Off a wall or post: the marker stands on its face.
            let normal = (after - before).normalize_or(Vec3::Y);
            return Some(Spot { at: pos - normal * BALL_RADIUS, t, normal, way: way(after) });
        }
        if (aerial && airborne && on_ground) || (!aerial && ball.vel == Vec3::ZERO) {
            return Some(Spot { at: Vec3::new(pos.x, 0.0, pos.z), t, normal: Vec3::Y, way: way(before) });
        }
    }
    // A shot kept low marks like a pass; a pass marks where it is at 2.5 s.
    if aerial {
        marker_for(Kick::Pass, pos, &ball).map(|s| Spot { t: s.t + ahead, ..s }).filter(|s| s.t < ahead + 2.5)
    } else {
        Some(Spot { at: Vec3::new(pos.x, 0.0, pos.z), t: ahead, normal: Vec3::Y, way: way(ball.vel) })
    }
}

#[derive(Component)]
pub struct Marker {
    mesh: Handle<Mesh>,
    total: f32,
    left: f32,
}

/// The marker's arrow, hidden on walls.
#[derive(Component)]
pub struct MarkerArrow;

/// The marker's glow facing the camera, so it reads from far away (NS_Bounce's camera sprite).
#[derive(Component)]
pub struct MarkerBeacon;

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

/// The ball's shadow on the ground.
#[derive(Component)]
pub struct BallShadow;

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
fn ring(fill: f32) -> Mesh {
    let n = 48;
    let (inner, outer) = MARKER_RADII;
    let mut positions = Vec::new();
    for i in 0..=n {
        let a = FRAC_PI_2 + fill * TAU * i as f32 / n as f32;
        let (s, c) = a.sin_cos();
        positions.push([c * inner, 0.0, -s * inner]);
        positions.push([c * outer, 0.0, -s * outer]);
    }
    let indices = (0..n as u32).flat_map(|i| [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 1, 2 * i + 3, 2 * i + 2]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// A flat arrow along local -Z: a shaft from the ring out, and a head at 2.8 m (SM_Bounce_Arrow).
fn arrow() -> Mesh {
    let (w, head) = (0.05, 0.16);
    let positions = vec![
        [-w, 0.0, -0.55],
        [w, 0.0, -0.55],
        [-w, 0.0, -2.4],
        [w, 0.0, -2.4],
        [-head, 0.0, -2.4],
        [head, 0.0, -2.4],
        [0.0, 0.0, -2.8],
    ];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 1, 3, 2, 4, 5, 6]))
}

pub fn spawn_fx(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // The marker: a ring that runs down as the ball comes, a soft glow under it and an arrow the way
    // the ball goes on, all drawn over the pitch.
    let flat = |c: Color, mode: AlphaMode| StandardMaterial { base_color: c, unlit: true, alpha_mode: mode, double_sided: true, cull_mode: None, depth_bias: 50.0, ..default() };
    let mesh = meshes.add(ring(1.0));
    commands
        .spawn((Marker { mesh: mesh.clone(), total: 1.0, left: 0.0 }, Transform::default(), Visibility::Hidden))
        .with_children(|m| {
            m.spawn((NotShadowCaster, Mesh3d(mesh), MeshMaterial3d(materials.add(flat(Color::srgba(1.0, 1.0, 1.0, 0.95), AlphaMode::Blend))), Transform::from_xyz(0.0, 0.02, 0.0)));
            m.spawn((NotShadowCaster, 
                Mesh3d(meshes.add(Circle::new(0.7))),
                MeshMaterial3d(materials.add(flat(Color::srgb(0.25, 0.3, 0.35), AlphaMode::Add))),
                Transform::from_xyz(0.0, 0.015, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
            m.spawn((NotShadowCaster, MarkerArrow, Mesh3d(meshes.add(arrow())), MeshMaterial3d(materials.add(flat(Color::srgba(1.0, 1.0, 1.0, 0.5), AlphaMode::Blend))), Transform::from_xyz(0.0, 0.02, 0.0)));
        });
    commands
        .spawn((MarkerBeacon, Transform::default(), Visibility::Hidden))
        .with_children(|b| {
            b.spawn((NotShadowCaster, Mesh3d(meshes.add(Annulus::new(0.2, 0.28))), MeshMaterial3d(materials.add(flat(Color::srgb(0.85, 0.95, 1.0), AlphaMode::Add))), Transform::default()));
            b.spawn((NotShadowCaster, Mesh3d(meshes.add(Circle::new(0.09))), MeshMaterial3d(materials.add(flat(Color::srgb(0.85, 0.95, 1.0), AlphaMode::Add))), Transform::default()));
        });
    commands.spawn((
        VolleyDiamond,
        NotShadowCaster,
        Mesh3d(meshes.add(diamond())),
        MeshMaterial3d(materials.add(flat(Color::BLACK, AlphaMode::Add))),
        Transform::default(),
        Visibility::Hidden,
    ));
    // The ball's shadow, always on: a soft dark disc under it (NS_Shadow_Ball; its faint ring is
    // left out, players don't see one).
    commands
        .spawn((BallShadow, Transform::default(), Visibility::default()))
        .with_children(|s| {
            s.spawn((NotShadowCaster, 
                Mesh3d(meshes.add(Circle::new(0.18))),
                MeshMaterial3d(materials.add(flat(Color::srgba(0.0, 0.0, 0.0, 0.5), AlphaMode::Blend))),
                Transform::from_xyz(0.0, 0.012, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
        });
    commands.insert_resource(FxAssets { sphere: meshes.add(Sphere::new(BALL_RADIUS)), ring: meshes.add(Annulus::new(0.88, 1.0)) });
    // The trails: vertex-coloured ribbons added onto the scene.
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, ..default() });
    for (source, effect) in [(Source::Ball, Effect::Dribble), (Source::Foot(1.0), Effect::Dribble), (Source::Foot(-1.0), Effect::ExtraEffort), (Source::Foot(1.0), Effect::ExtraEffort), (Source::Ball, Effect::Kick)] {
        let mesh = meshes.add(ribbon(&[], Vec3::ZERO, 0.0, 1.0, Vec3::ZERO));
        commands.spawn((NotShadowCaster, Trail { source, effect, points: Vec::new(), mesh: mesh.clone() }, Mesh3d(mesh), MeshMaterial3d(glow.clone()), Transform::default(), Visibility::default()));
    }
}

/// The ball's shadow follows it on the ground, growing with its height (x2.5) and fading out
/// between 1.5 and 8 m up.
pub fn ball_shadow(ball: Single<&Transform, With<Ball>>, mut shadow: Single<(&mut Transform, &mut Visibility), (With<BallShadow>, Without<Ball>)>) {
    let height = (ball.translation.y - BALL_RADIUS).max(0.0);
    let (tf, vis) = &mut *shadow;
    tf.translation = Vec3::new(ball.translation.x, 0.0, ball.translation.z);
    tf.scale = Vec3::splat(1.0 + 1.5 * (height / 8.0).min(1.0));
    **vis = if height < 8.0 { Visibility::Inherited } else { Visibility::Hidden };
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

/// The moves' trails: a dribble streaks blue behind the ball and the touching foot (players
/// confirm the look), extra effort behind both feet. Points age out after the effect's life.
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

/// The kick marker: placed as a kick sends the ball, its ring running down as the ball gets there;
/// gone when it arrives, when anyone gains the ball, or at the next kick.
pub fn kick_marker(
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<(&Transform, &Ball)>,
    mut marker: Single<(&mut Marker, &mut Transform, &mut Visibility), (Without<Ball>, Without<MarkerBeacon>)>,
    mut arrow: Single<&mut Visibility, (With<MarkerArrow>, Without<Marker>, Without<MarkerBeacon>)>,
    mut beacon: Single<(&mut Transform, &mut Visibility), (With<MarkerBeacon>, Without<Marker>, Without<Ball>, Without<MarkerArrow>)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut last_kick: Local<Option<Kick>>,
) {
    let (btf, b) = *ball;
    let (m, tf, vis) = &mut *marker;
    let kicked = last_kick.filter(|_| player.kick.is_none() && !b.held);
    *last_kick = player.kick.map(|k| k.kind);
    if let Some(kind) = kicked {
        m.left = 0.0;
        if let Some(spot) = marker_for(kind, btf.translation, b) {
            // On the ground the arrow points the way the ball goes on; on a wall it stands up.
            tf.translation = spot.at;
            tf.rotation = if spot.normal == Vec3::Y { Quat::from_rotation_y((-spot.way.x).atan2(-spot.way.z)) } else { Quat::from_rotation_arc(Vec3::Y, spot.normal) };
            **arrow = if spot.normal == Vec3::Y { Visibility::Inherited } else { Visibility::Hidden };
            (m.total, m.left) = (spot.t, spot.t);
        }
    }
    if b.held || player.kick.is_some() {
        m.left = 0.0;
    }
    m.left -= time.delta_secs();
    **vis = if m.left > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    let (btf2, bvis) = &mut *beacon;
    **bvis = **vis;
    let above = tf.translation + tf.rotation * Vec3::Y * 0.45;
    // It keeps a readable size on screen however far it is.
    let far = (camera.translation().distance(above) / 12.0).max(1.0);
    **btf2 = Transform::from_translation(above).looking_at(camera.translation(), Vec3::Y).with_scale(Vec3::splat(far));
    if m.left > 0.0
        && let Some(mesh) = meshes.get_mut(&m.mesh)
    {
        *mesh = ring((m.left / m.total).clamp(0.0, 1.0));
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
/// rings closing on the ball, orange to blue, as it pulses; and extra effort's burst at your feet.
pub fn action_fx(
    mut commands: Commands,
    player: Single<(&Player, &Transform)>,
    ball: Single<(&Transform, &Ball), Without<Player>>,
    fx: Res<FxAssets>,
    mut state: ResMut<FxState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_kick: Local<Option<Kick>>,
    mut was: Local<(bool, bool)>,
) {
    let ((p, ptf), (btf, b)) = (*player, *ball);
    let at = btf.translation;
    let kicked = last_kick.filter(|_| p.kick.is_none() && !b.held);
    *last_kick = p.kick.map(|k| k.kind);
    let flat = |v: Vec3| Quat::from_rotation_arc(Vec3::Z, v.normalize_or(Vec3::NEG_Z));
    let ground = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
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
        glow(&mut commands, &mut materials, &fx.sphere, Quat::IDENTITY, Glow::new(0.06, 1.0, (2.0 * size, 3.5 * size), (WHITE, WHITE), false));
        glow(&mut commands, &mut materials, &fx.sphere, Quat::IDENTITY, Glow::new(0.25, 0.25, (1.5 * size, 4.0 * size), (Vec3::new(0.2, 0.9, 1.0), BLUE), false));
        for r in 0..rings {
            let reach = size * (0.45 + 0.25 * r as f32);
            glow(&mut commands, &mut materials, &fx.ring, facing, Glow::new(0.25 + 0.05 * r as f32, 0.8, (0.12, reach), (Vec3::new(1.0, 0.95, 0.3), Vec3::new(0.9, 0.2, 0.8)), false));
        }
        state.kick_trail = !matches!(kind, Kick::PushBall | Kick::DribblePushBall) || at.y > 0.75;
    }
    if b.held {
        state.kick_trail = false;
    }
    let (was_receiving, was_effort) = *was;
    if p.receiving.is_some() && !was_receiving {
        for life in [0.25, 0.35] {
            spawn_glow(&mut commands, &mut materials, &fx.ring, Vec3::new(at.x, 0.03, at.z), ground, Glow::new(life, 0.9, (1.08, 0.27), (ORANGE, BLUE), false));
        }
        spawn_glow(&mut commands, &mut materials, &fx.sphere, at, Quat::IDENTITY, Glow::new(0.45, 0.8, (1.5, 1.0), (WHITE, BLUE), true));
    }
    if p.extra_active && !was_effort {
        let feet = Vec3::new(ptf.translation.x, 0.03, ptf.translation.z);
        for (life, reach) in [(0.3, 1.2), (0.4, 1.7)] {
            spawn_glow(&mut commands, &mut materials, &fx.ring, feet, ground, Glow::new(life, 0.9, (0.3, reach), (Vec3::new(0.0, 1.0, 0.8), Vec3::new(0.3, 0.7, 1.0)), false));
        }
    }
    *was = (p.receiving.is_some(), p.extra_active);
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
    fn markers_mark_the_landing_or_where_a_pass_gets_to() {
        let at = Vec3::new(0.0, BALL_RADIUS, 0.0);
        // A lob comes down ahead of you.
        let lob = Ball { vel: Vec3::new(0.0, 12.0, -15.0), ..default() };
        let land = marker_for(Kick::Lob, at, &lob).expect("a lob lands");
        assert!(land.at.z < -10.0 && land.t > 1.0 && land.normal == Vec3::Y, "{land:?}");
        // A slow pass stops before 2.5 s: the marker is where it stops.
        let pass = Ball { vel: Vec3::new(0.0, 0.0, -3.0), friction: PASS_FRICTION, ..default() };
        let stop = marker_for(Kick::Pass, at, &pass).expect("a pass is marked");
        assert!(stop.t <= 2.5 && stop.at.z < -1.0, "{stop:?}");
        assert!(marker_for(Kick::PushBall, at, &pass).is_none());
    }
}
