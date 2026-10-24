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

/// Where a kick's marker goes and how long the ball takes to get there (BP_BallVFXComponent): a
/// lob or a shot marks its first landing; a pass the point it reaches 2.5 s on (a short one 0.9
/// s), or sooner where it first hits a wall or stops; a push ball has none.
fn marker_for(kind: Kick, pos: Vec3, ball: &Ball) -> Option<(Vec3, f32)> {
    let ahead = match kind {
        Kick::Pass => 2.5,
        Kick::ShortPass => 0.9,
        Kick::Shot | Kick::Lob | Kick::ShortLob => 6.0,
        Kick::PushBall | Kick::DribblePushBall => return None,
    };
    let landing = matches!(kind, Kick::Shot | Kick::Lob | Kick::ShortLob);
    let (mut pos, mut ball) = (pos, ball.clone());
    let mut airborne = false;
    let dt = 1.0 / 240.0;
    for i in 1..=(ahead / dt) as usize {
        let before = Vec2::new(ball.vel.x, ball.vel.z);
        if step(&mut pos, &mut ball, dt).is_some() {
            return None;
        }
        let t = i as f32 * dt;
        let on_ground = pos.y <= BALL_RADIUS + 0.01;
        airborne |= !on_ground;
        let now = Vec2::new(ball.vel.x, ball.vel.z);
        let off_wall = before.length() > 1.0 && now.length() > 0.1 && before.normalize().dot(now.normalize()) < 0.5;
        if landing && airborne && on_ground {
            return Some((Vec3::new(pos.x, 0.0, pos.z), t));
        }
        if !landing && (off_wall || ball.vel == Vec3::ZERO) {
            return Some((Vec3::new(pos.x, 0.0, pos.z), t));
        }
    }
    // A shot kept low marks like a pass; a pass marks where it is at 2.5 s.
    if landing { marker_for(Kick::Pass, pos, &ball).map(|(p, t)| (p, t + ahead)).filter(|(_, t)| *t < ahead + 2.5) } else { Some((Vec3::new(pos.x, 0.0, pos.z), ahead)) }
}

#[derive(Component)]
pub struct Marker {
    mesh: Handle<Mesh>,
    total: f32,
    left: f32,
}

#[derive(Resource)]
pub struct FxAssets {
    sphere: Handle<Mesh>,
}

const MARKER_RADII: (f32, f32) = (0.31, 0.375);

/// A ring on the ground, filled to `fill` of the way round.
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

pub fn spawn_fx(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mesh = meshes.add(ring(1.0));
    let white = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 1.0, 1.0, 0.85), unlit: true, alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None, ..default() });
    commands.spawn((Marker { mesh: mesh.clone(), total: 1.0, left: 0.0 }, Mesh3d(mesh), MeshMaterial3d(white), Transform::default(), Visibility::Hidden));
    commands.insert_resource(FxAssets { sphere: meshes.add(Sphere::new(BALL_RADIUS)) });
    // The trails: vertex-coloured ribbons added onto the scene.
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, ..default() });
    for (source, effect) in [(Source::Ball, Effect::Dribble), (Source::Foot(1.0), Effect::Dribble), (Source::Foot(-1.0), Effect::ExtraEffort), (Source::Foot(1.0), Effect::ExtraEffort)] {
        let mesh = meshes.add(ribbon(&[], Vec3::ZERO, 0.0, 1.0, Vec3::ZERO));
        commands.spawn((Trail { source, effect, points: Vec::new(), mesh: mesh.clone() }, Mesh3d(mesh), MeshMaterial3d(glow.clone()), Transform::default(), Visibility::default()));
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

/// The kick marker: placed as a kick sends the ball, its ring emptying as the ball gets there; gone
/// when it arrives, when someone gains the ball, or at the next kick.
pub fn kick_marker(
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<(&Transform, &Ball)>,
    mut marker: Single<(&mut Marker, &mut Transform, &mut Visibility), Without<Ball>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut last_kick: Local<Option<Kick>>,
) {
    let (btf, b) = *ball;
    let (m, tf, vis) = &mut *marker;
    let kicked = last_kick.filter(|_| player.kick.is_none() && !b.held);
    *last_kick = player.kick.map(|k| k.kind);
    if let Some(kind) = kicked {
        m.left = 0.0;
        if let Some((at, t)) = marker_for(kind, btf.translation, b) {
            tf.translation = at + Vec3::Y * 0.02;
            (m.total, m.left) = (t, t);
        }
    }
    if b.held || player.kick.is_some() {
        m.left = 0.0;
    }
    m.left -= time.delta_secs();
    **vis = if m.left > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
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
        let (land, t) = marker_for(Kick::Lob, at, &lob).expect("a lob lands");
        assert!(land.z < -10.0 && t > 1.0, "{land} at {t}");
        // A slow pass stops before 2.5 s: the marker is where it stops.
        let pass = Ball { vel: Vec3::new(0.0, 0.0, -3.0), friction: PASS_FRICTION, ..default() };
        let (stop, t) = marker_for(Kick::Pass, at, &pass).expect("a pass is marked");
        assert!(t <= 2.5 && stop.z < -1.0, "{stop} at {t}");
        assert!(marker_for(Kick::PushBall, at, &pass).is_none());
    }
}
