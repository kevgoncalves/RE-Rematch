//! Effects and practice aids: the dribble's blue glow on the ball, the marker where a kick lands,
//! and the cheats from the F1 menu (infinite stamina and extra effort, the ball's trail and its
//! predicted path). The look is our own; when and where they show is the game's (notes/hud.md).

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::asset::RenderAssetUsages;
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

/// The ball's shadow on the ground.
#[derive(Component)]
pub struct BallShadow;

#[derive(Resource)]
pub struct FxAssets {
    sphere: Handle<Mesh>,
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
            m.spawn((Mesh3d(mesh), MeshMaterial3d(materials.add(flat(Color::srgba(1.0, 1.0, 1.0, 0.95), AlphaMode::Blend))), Transform::from_xyz(0.0, 0.02, 0.0)));
            m.spawn((
                Mesh3d(meshes.add(Circle::new(0.7))),
                MeshMaterial3d(materials.add(flat(Color::srgb(0.25, 0.3, 0.35), AlphaMode::Add))),
                Transform::from_xyz(0.0, 0.015, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
            m.spawn((MarkerArrow, Mesh3d(meshes.add(arrow())), MeshMaterial3d(materials.add(flat(Color::srgba(1.0, 1.0, 1.0, 0.5), AlphaMode::Blend))), Transform::from_xyz(0.0, 0.02, 0.0)));
        });
    commands
        .spawn((MarkerBeacon, Transform::default(), Visibility::Hidden))
        .with_children(|b| {
            b.spawn((Mesh3d(meshes.add(Annulus::new(0.2, 0.28))), MeshMaterial3d(materials.add(flat(Color::srgb(0.85, 0.95, 1.0), AlphaMode::Add))), Transform::default()));
            b.spawn((Mesh3d(meshes.add(Circle::new(0.09))), MeshMaterial3d(materials.add(flat(Color::srgb(0.85, 0.95, 1.0), AlphaMode::Add))), Transform::default()));
        });
    // The ball's shadow, always on: a soft dark disc and a faint ring under it (NS_Shadow_Ball).
    commands
        .spawn((BallShadow, Transform::default(), Visibility::default()))
        .with_children(|s| {
            s.spawn((
                Mesh3d(meshes.add(Circle::new(0.18))),
                MeshMaterial3d(materials.add(flat(Color::srgba(0.0, 0.0, 0.0, 0.5), AlphaMode::Blend))),
                Transform::from_xyz(0.0, 0.012, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
            s.spawn((
                Mesh3d(meshes.add(Annulus::new(0.34, 0.39))),
                MeshMaterial3d(materials.add(flat(Color::srgba(1.0, 1.0, 1.0, 0.15), AlphaMode::Blend))),
                Transform::from_xyz(0.0, 0.013, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
        });
    commands.insert_resource(FxAssets { sphere: meshes.add(Sphere::new(BALL_RADIUS)) });
    // The trails: vertex-coloured ribbons added onto the scene.
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, ..default() });
    for (source, effect) in [(Source::Ball, Effect::Dribble), (Source::Foot(1.0), Effect::Dribble), (Source::Foot(-1.0), Effect::ExtraEffort), (Source::Foot(1.0), Effect::ExtraEffort)] {
        let mesh = meshes.add(ribbon(&[], Vec3::ZERO, 0.0, 1.0, Vec3::ZERO));
        commands.spawn((Trail { source, effect, points: Vec::new(), mesh: mesh.clone() }, Mesh3d(mesh), MeshMaterial3d(glow.clone()), Transform::default(), Visibility::default()));
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
}

impl Effect {
    /// Colour (linear), how long a point lives and the trail's width.
    fn look(self) -> (Vec3, f32, f32) {
        match self {
            // The dribble's blue (NS_Dribble's colour, 0.3 / 1.05 / 5.0, scaled down to display).
            Effect::Dribble => (Vec3::new(0.06, 0.21, 1.0), 0.25, 0.16),
            // ponytail: extra effort's colour and size until its effect data is read.
            Effect::ExtraEffort => (Vec3::new(1.0, 0.55, 0.12), 0.18, 0.1),
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
    /// Grows to this scale over its life.
    grow: f32,
    follow: bool,
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
        let material = materials.add(StandardMaterial { base_color: blue(0.9), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
        commands.spawn((Glow { age: 0.0, life: 0.5, alpha: 0.9, grow: 1.25, follow: true }, Mesh3d(fx.sphere.clone()), MeshMaterial3d(material), Transform::from_translation(ball.translation).with_scale(Vec3::splat(1.25))));
    }
}

fn blue(alpha: f32) -> Color {
    Color::linear_rgba(0.3 / 5.0, 1.05 / 5.0, 1.0, 1.0).with_alpha(alpha)
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
        let start = tf.scale.x.min(g.grow);
        tf.scale = Vec3::splat(start + (g.grow - start) * (time.delta_secs() / (g.life - g.age + time.delta_secs())).min(1.0));
        if let Some(m) = materials.get_mut(&mat.0) {
            // Additive: fading the colour fades the glow.
            m.base_color = Color::linear_rgba(0.3 / 5.0, 1.05 / 5.0, 1.0, 1.0).mix(&Color::BLACK, u).with_alpha(g.alpha * (1.0 - u));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
