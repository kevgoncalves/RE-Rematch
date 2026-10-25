//! Effects and practice aids. The effects follow Kev's recordings (notes/vfx.md): a shot's spikes,
//! flare and shards by power, grass kicked up; the control's closing rings and ghost balls; the
//! dribble's and push ball's glow; the pass release; extra effort's burst, bolts, footprints and
//! leg rim; the fake shot's rings; the volley diamond; the landing marker and the ball's shadow.
//! Also the cheats from the menu's Cheats tab (infinite stamina and extra effort, the ball's trail
//! and its predicted path). The look is our own; when and where they show is the game's
//! (notes/hud.md, notes/vfx.md).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::Face;
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
    DebugInfo,
    SlowMotion,
    ShowZones,
}

impl Cheat {
    pub const ALL: [Cheat; 7] = [
        Cheat::InfiniteStamina,
        Cheat::InfiniteEffort,
        Cheat::BallTrail,
        Cheat::BallPath,
        Cheat::DebugInfo,
        Cheat::SlowMotion,
        Cheat::ShowZones,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Cheat::InfiniteStamina => "Infinite stamina",
            Cheat::InfiniteEffort => "Infinite extra effort",
            Cheat::BallTrail => "Ball trail",
            Cheat::BallPath => "Ball path prediction",
            Cheat::DebugInfo => "Debug info",
            Cheat::SlowMotion => "Slow motion (25%)",
            Cheat::ShowZones => "Show control zones",
        }
    }
}

#[derive(Resource, Default)]
pub struct Cheats([bool; 7]);

impl Cheats {
    pub fn on(&self, c: Cheat) -> bool {
        self.0[c as usize]
    }

    pub fn toggle(&mut self, c: Cheat) {
        self.0[c as usize] ^= true;
    }
}

pub fn apply_cheats(cheats: Res<Cheats>, mut player: Single<&mut Player>, mut time: ResMut<Time<Virtual>>) {
    let speed = if cheats.on(Cheat::SlowMotion) { 0.25 } else { 1.0 };
    if time.relative_speed() != speed {
        time.set_relative_speed(speed);
    }
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

/// A ground marker is this size against a wall's (Kev: the same look, smaller).
const MARKER_GROUND_SCALE: f32 = 0.7;

/// Where a marker goes: the surface point, its normal and when the ball gets there.
struct MarkerSpot {
    at: Vec3,
    normal: Vec3,
    t: f32,
}

/// Where a kicked ball first hits a goal end's wall (the backboard), a post or the crossbar, or
/// else comes to rest: its own physics run on until it rolls slower than 0.5 m/s on the ground
/// (the game's prediction stop speed), up to 10 s. The side walls get no marker (Kev). None if it
/// goes in.
fn marker_for(pos: Vec3, ball: &Ball) -> Option<MarkerSpot> {
    let (mut pos, mut ball) = (pos, ball.clone());
    let dt = 1.0 / 240.0;
    for i in 1..=(10.0 / dt) as usize {
        if step(&mut pos, &mut ball, dt).is_some() {
            return None;
        }
        let t = i as f32 * dt;
        if let Some((at, normal)) = ball.hit
            && (at.x.abs() - HALF_WIDTH).abs() > 0.05
        {
            return Some(MarkerSpot { at, normal, t });
        }
        if pos.y <= BALL_RADIUS + 0.01 && Vec2::new(ball.vel.x, ball.vel.z).length() < 0.5 && ball.vel.y.abs() < 0.5 {
            return Some(MarkerSpot { at: Vec3::new(pos.x, 0.0, pos.z), normal: Vec3::Y, t });
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

/// The arrow over the marker, pointing down at it.
#[derive(Component)]
pub struct MarkerArrow;

/// The volley diamond on the ball (GC_VolleyPredict): its four arrows and the glow behind them.
#[derive(Component, PartialEq)]
pub enum VolleyDiamond {
    Arrows,
    Glow,
}

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
        let (h10, h01) = (u.powi(3) - 2.0 * u * u + u, -2.0 * u.powi(3) + 3.0 * u * u);
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

/// A mesh from flat-shaded, vertex-coloured triangles (the effects are unlit, so normals only
/// need to exist).
fn coloured(positions: Vec<[f32; 3]>, colours: Vec<[f32; 4]>, indices: Vec<u32>) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// White at strength `v`, carried in alpha: additive materials scale their light by it and blended
/// ones their cover, so one mesh serves both.
fn grey(v: f32) -> [f32; 4] {
    [1.0, 1.0, 1.0, v]
}

/// The diamond: four separate triangles around the ball pointing out (up, right, down, left), 0.54 m
/// tip to tip, brightest at their inner edge, in the plane facing +Z.
fn diamond() -> Mesh {
    let (base, tip, half) = (0.16, 0.27, 0.075);
    let (mut positions, mut colours) = (Vec::new(), Vec::new());
    for i in 0..4 {
        let a = FRAC_PI_2 * i as f32;
        let (out, across) = (Vec2::from_angle(a), Vec2::from_angle(a + FRAC_PI_2));
        for (p, light) in [(out * base + across * half, 1.0), (out * base - across * half, 1.0), (out * tip, 0.7)] {
            positions.push([p.x, p.y, 0.0]);
            colours.push([light, light, light, 1.0]);
        }
    }
    coloured(positions, colours, (0..12).collect())
}

/// A disc of light in the XY plane, radius 1: bright in the middle, soft to nothing at the edge.
/// `bands` are (radius, light) from the centre out; the last should be dark.
fn disc(bands: &[(f32, f32)], segments: u32) -> Mesh {
    let (mut positions, mut colours, mut indices) = (vec![[0.0, 0.0, 0.0]], vec![grey(bands[0].1)], Vec::new());
    let ring = segments + 1;
    for (b, &(r, light)) in bands.iter().enumerate().skip(1) {
        for i in 0..=segments {
            let a = TAU * i as f32 / segments as f32;
            positions.push([a.cos() * r, a.sin() * r, 0.0]);
            colours.push(grey(light));
            if i == segments {
                continue;
            }
            let at = 1 + (b as u32 - 1) * ring + i;
            if b == 1 {
                indices.extend([0, at, at + 1]);
            } else {
                indices.extend([at - ring, at, at + 1, at - ring, at + 1, at + 1 - ring]);
            }
        }
    }
    coloured(positions, colours, indices)
}

/// A soft round glow (flares, puffs, shells' halos).
fn soft() -> Mesh {
    disc(&[(0.0, 1.0), (0.4, 0.45), (0.75, 0.12), (1.0, 0.0)], 24)
}

/// Extra effort's ground burst: a bright middle, a dimmer band, a bright rim, and three swirl arms.
fn swirl() -> Mesh {
    let mut mesh = disc(&[(0.0, 0.9), (0.35, 0.45), (0.7, 0.35), (0.85, 0.9), (1.0, 0.0)], 40);
    let (mut positions, mut colours, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    let base = mesh.count_vertices() as u32;
    for arm in 0..3 {
        let a0 = TAU * arm as f32 / 3.0;
        for i in 0..=12 {
            let s = i as f32 / 12.0;
            let (r, a) = (0.2 + 0.7 * s, a0 + 2.4 * s);
            let (dir, w) = (Vec2::from_angle(a), 0.035 * (1.0 - 0.5 * s));
            for side in [-1.0, 1.0] {
                let p = dir * (r + side * w);
                positions.push([p.x, p.y, 0.0]);
                colours.push(grey(0.9 * (1.0 - s)));
            }
            if i < 12 {
                let at = base + (arm * 13 + i) * 2;
                indices.extend([at, at + 1, at + 2, at + 1, at + 3, at + 2]);
            }
        }
    }
    let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) else { unreachable!() };
    p.extend(positions.iter().copied());
    let Some(bevy::render::mesh::VertexAttributeValues::Float32x4(c)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) else { unreachable!() };
    c.extend(colours);
    let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(n)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) else { unreachable!() };
    n.extend(vec![[0.0, 0.0, 1.0]; positions.len()]);
    let Some(Indices::U32(i)) = mesh.indices_mut() else { unreachable!() };
    i.extend(indices);
    mesh
}

/// A broken ring of radius 1 in the XY plane: three arcs with gaps, 12 % thick, fading at their ends.
fn arcs() -> Mesh {
    let (n, gap, inner, steps) = (3, 0.55, 0.88, 14);
    let (mut positions, mut colours, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for arc in 0..n {
        let (start, span) = (TAU * arc as f32 / n as f32, TAU / n as f32 - gap);
        for i in 0..=steps {
            let s = i as f32 / steps as f32;
            let dir = Vec2::from_angle(start + span * s);
            let light = 0.75 + 0.25 * (s * PI).sin();
            for r in [inner, 1.0] {
                positions.push([dir.x * r, dir.y * r, 0.0]);
                colours.push(grey(light));
            }
            if i < steps {
                let at = (arc * (steps + 1) + i) * 2;
                indices.extend([at, at + 1, at + 2, at + 1, at + 3, at + 2]);
            }
        }
    }
    coloured(positions, colours, indices)
}

/// Spikes from a point in the XY plane, up to 1 long: a strike's burst.
fn star() -> Mesh {
    let lengths = [1.0, 0.6, 0.85, 0.5, 0.95, 0.65, 0.8, 0.55, 0.9];
    let (mut positions, mut colours) = (Vec::new(), Vec::new());
    for (i, len) in lengths.iter().enumerate() {
        let a = TAU * i as f32 / lengths.len() as f32 + 0.2 * (i as f32 * 2.3).sin();
        let (out, across) = (Vec2::from_angle(a), Vec2::from_angle(a + FRAC_PI_2));
        for (p, light) in [(out * 0.05 + across * 0.05, 1.0), (out * 0.05 - across * 0.05, 1.0), (out * *len, 0.35)] {
            positions.push([p.x, p.y, 0.0]);
            colours.push(grey(light));
        }
    }
    let n = positions.len() as u32;
    coloured(positions, colours, (0..n).collect())
}

/// One flat shard, about 1 across.
fn shard() -> Mesh {
    coloured(vec![[0.0, 0.55, 0.0], [-0.4, -0.4, 0.0], [0.45, -0.3, 0.0]], vec![grey(1.0), grey(0.7), grey(0.85)], vec![0, 1, 2])
}

/// A lightning bolt 1 long in the XY plane: a zigzag strip, `variant` picks its kinks.
fn bolt(variant: u32) -> Mesh {
    let kinks = 6;
    let points: Vec<Vec2> = (0..=kinks)
        .map(|i| {
            let s = i as f32 / kinks as f32;
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let x = if i == 0 || i == kinks { 0.0 } else { side * (0.08 + 0.07 * ((i as u32 * 7 + variant * 3) % 5) as f32 / 4.0) };
            Vec2::new(x, s - 0.5)
        })
        .collect();
    let (mut positions, mut colours, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for (i, p) in points.iter().enumerate() {
        let prev = points[i.saturating_sub(1)];
        let next = points[(i + 1).min(points.len() - 1)];
        let across = (next - prev).perp().normalize_or_zero() * 0.03;
        let light = if i == 0 || i == kinks as usize { 0.3 } else { 1.0 };
        for q in [*p + across, *p - across] {
            positions.push([q.x, q.y, 0.0]);
            colours.push(grey(light));
        }
        if i < kinks as usize {
            let at = i as u32 * 2;
            indices.extend([at, at + 1, at + 2, at + 1, at + 3, at + 2]);
        }
    }
    coloured(positions, colours, indices)
}

/// A soft streak in the XY plane, 1 by 1, long along Y: dark at its ends and sides (a footprint).
fn streak() -> Mesh {
    let (xs, ys) = ([-0.5, 0.0, 0.5], [-0.5, -0.25, 0.25, 0.5]);
    let (mut positions, mut colours, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for (j, &y) in ys.iter().enumerate() {
        for &x in &xs {
            positions.push([x, y, 0.0]);
            let end = if j == 0 || j == ys.len() - 1 { 0.0 } else { 1.0 };
            colours.push(grey(end * (1.0 - 2.0 * f32::abs(x))));
        }
    }
    for j in 0..ys.len() as u32 - 1 {
        for i in 0..xs.len() as u32 - 1 {
            let at = j * 3 + i;
            indices.extend([at, at + 1, at + 3, at + 1, at + 4, at + 3]);
        }
    }
    coloured(positions, colours, indices)
}

/// The marker's arrow: two crossed upright triangles pointing down at the marker, tip at the origin,
/// 0.3 m tall, bright at the tip.
fn arrow() -> Mesh {
    let (top, half) = (0.3, 0.13);
    let tip = [0.25, 0.95, 0.45, 1.0];
    let edge = [0.08, 0.45, 0.18, 1.0];
    let positions = vec![[0.0, 0.0, 0.0], [-half, top, 0.0], [half, top, 0.0], [0.0, 0.0, 0.0], [0.0, top, -half], [0.0, top, half]];
    coloured(positions, vec![tip, edge, edge, tip, edge, edge], vec![0, 1, 2, 3, 4, 5])
}

/// Effect colours, linear, read off the recordings (notes/vfx.md). Additive: above 1 blooms.
const YELLOW: Vec3 = Vec3::new(1.0, 0.85, 0.05);
/// The control rings' pale end.
const PALE_YELLOW: Vec3 = Vec3::new(1.0, 0.92, 0.55);
const CONTROL_BLUE: Vec3 = Vec3::new(0.3, 0.68, 0.84);
const TEAL: Vec3 = Vec3::new(0.11, 0.42, 0.38);
const DRIBBLE_BLUE: Vec3 = Vec3::new(0.22, 0.56, 0.69);
const PUFF: Vec3 = Vec3::new(0.3, 0.8, 1.0);
const PUSH_BLUE: Vec3 = Vec3::new(0.12, 0.28, 0.85);
const WHITE: Vec3 = Vec3::ONE;
const ICE: Vec3 = Vec3::new(0.6, 0.95, 1.0);
const GOLD: Vec3 = Vec3::new(1.0, 0.72, 0.18);
const GOLD_GLOW: Vec3 = Vec3::new(0.73, 0.6, 0.15);
const PASS_BLUE: Vec3 = Vec3::new(0.15, 0.4, 1.0);
const EE_WHITE: Vec3 = Vec3::new(0.45, 0.78, 0.9);
const BOLT: Vec3 = Vec3::new(0.3, 1.0, 0.85);
const EE_RIM: Vec3 = Vec3::new(0.36, 0.43, 0.71);
const FOOTPRINT: Vec3 = Vec3::new(0.2, 0.75, 1.0);
const FAKE_RED: Vec3 = Vec3::new(0.9, 0.15, 0.06);
const FAKE_PINK: Vec3 = Vec3::new(0.8, 0.3, 0.45);
const PEACH: Vec3 = Vec3::new(0.67, 0.31, 0.23);
const GRASS_BLADE: Vec3 = Vec3::new(0.12, 0.35, 0.06);
const DIAMOND: Vec3 = Vec3::new(0.4, 0.87, 0.92);
const DIAMOND_GLOW: Vec3 = Vec3::new(0.22, 0.53, 0.81);
/// The power shot's launch speed (m/s): the third, biggest impact (gameplay_fx data).
const SHOT_TOP_SPEED: f32 = 34.82;

#[derive(Resource)]
pub struct FxAssets {
    sphere: Handle<Mesh>,
    arcs: Handle<Mesh>,
    soft: Handle<Mesh>,
    star: Handle<Mesh>,
    shard: Handle<Mesh>,
    bolts: [Handle<Mesh>; 4],
    blade: Handle<Mesh>,
    streak: Handle<Mesh>,
    swirl: Handle<Mesh>,
    /// The legs' rim, shared by every shell.
    rim: Handle<StandardMaterial>,
}

/// A run of ghost balls left where the ball passes (the dribble's, control's and push ball's).
#[derive(Clone, Copy)]
struct Ghosts {
    delay: f32,
    left: f32,
    /// One ghost every so far the ball travels.
    spacing: f32,
    colour: Vec3,
    alpha: f32,
    life: f32,
    blend: bool,
    last: Option<Vec3>,
}

/// Effects' own state: the kicked ball's trail, recent ball positions, ghost runs, and so on.
#[derive(Resource)]
pub struct FxState {
    kick_trail: bool,
    /// Where the ball was over the last 0.35 s (time, position), oldest first.
    history: Vec<(f32, Vec3)>,
    ghosts: Vec<Ghosts>,
    /// Seconds since the last control (the legs flash on it).
    control_age: f32,
    fake_shown: bool,
    rng: Rng,
}

impl Default for FxState {
    fn default() -> Self {
        Self { kick_trail: false, history: Vec::new(), ghosts: Vec::new(), control_age: f32::INFINITY, fake_shown: false, rng: Rng(0x9E37_79B9) }
    }
}

/// A small fixed-seed random source, so scripted runs repeat frame for frame.
struct Rng(u32);

impl Rng {
    /// 0..1.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }

    /// A random way along the ground.
    fn flat(&mut self) -> Vec3 {
        let a = self.next() * TAU;
        Vec3::new(a.cos(), 0.0, a.sin())
    }

    /// A random axis.
    fn axis(&mut self) -> Vec3 {
        Vec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)).normalize_or(Vec3::Y)
    }
}

/// The marker's ring, about 80 cm across (SM_Bounce_Circle, 75 x 86 cm).
const MARKER_RADII: (f32, f32) = (0.36, 0.43);

/// The marker's colours: a light green with a brighter inner edge (the recordings, notes/vfx.md).
const MARKER_GREEN: Vec3 = Vec3::new(0.08, 0.5, 0.2);
const MARKER_LIME: Vec3 = Vec3::new(0.25, 0.85, 0.4);

/// The marker's ring, flat in the local XZ plane.
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
        // Lime on the inner edge, green on the outer, a sheen sliding round the ring.
        let sheen = 0.5 + 0.5 * (a * 2.0).sin();
        colours.push((MARKER_LIME.lerp(MARKER_GREEN, 0.3 * sheen) * 1.4).extend(1.0).to_array());
        colours.push((MARKER_GREEN * 1.2).extend(1.0).to_array());
    }
    let indices = (0..n as u32).flat_map(|i| [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 1, 2 * i + 3, 2 * i + 2]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices))
}

/// The soft glow inside the marker: green at the centre fading out at the ring.
fn glow_disc() -> Mesh {
    let n = 32;
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut colours = vec![(MARKER_GREEN * 0.55).extend(1.0).to_array()];
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

/// The dark disc on the ground under the ball (NS_Shadow_Ball).
#[derive(Component)]
pub struct BallShadow;

/// A shell around a leg part that lights its outline (extra effort's rim, the control's flash).
#[derive(Component)]
pub struct Rim;

pub fn spawn_fx(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // The marker (NS_Bounce): a glowing green ring, a soft glow inside it and an arrow over it, added
    // light drawn over the ground or the wall.
    let flat = |c: Color, mode: AlphaMode| StandardMaterial { base_color: c, unlit: true, alpha_mode: mode, double_sided: true, cull_mode: None, depth_bias: 50.0, ..default() };
    let mesh = meshes.add(ring());
    let light = materials.add(flat(Color::WHITE, AlphaMode::Add));
    commands
        .spawn((Marker { left: 0.0, chain: false }, Transform::default(), Visibility::Hidden))
        .with_children(|m| {
            m.spawn((NotShadowCaster, Mesh3d(mesh), MeshMaterial3d(light.clone()), Transform::from_xyz(0.0, 0.02, 0.0)));
            m.spawn((NotShadowCaster, Mesh3d(meshes.add(glow_disc())), MeshMaterial3d(light.clone()), Transform::from_xyz(0.0, 0.015, 0.0)));
            m.spawn((MarkerArrow, NotShadowCaster, Mesh3d(meshes.add(arrow())), MeshMaterial3d(light), Transform::from_xyz(0.0, 0.42, 0.0)));
        });
    let soft_mesh = meshes.add(soft());
    for (part, mesh) in [(VolleyDiamond::Glow, soft_mesh.clone()), (VolleyDiamond::Arrows, meshes.add(diamond()))] {
        commands.spawn((part, NotShadowCaster, Mesh3d(mesh), MeshMaterial3d(materials.add(flat(Color::NONE, AlphaMode::Blend))), Transform::default(), Visibility::Hidden));
    }
    // The ball's shadow: dark in the middle, soft to nothing, lying on the ground.
    commands.spawn((
        BallShadow,
        NotShadowCaster,
        Mesh3d(meshes.add(disc(&[(0.0, 1.0), (0.55, 0.7), (1.0, 0.0)], 24))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::BLACK.with_alpha(0.5), unlit: true, alpha_mode: AlphaMode::Blend, depth_bias: 40.0, ..default() })),
        Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    ));
    let rim = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, cull_mode: Some(Face::Front), ..default() });
    commands.insert_resource(FxAssets {
        sphere: meshes.add(Sphere::new(BALL_RADIUS)),
        arcs: meshes.add(arcs()),
        soft: soft_mesh,
        star: meshes.add(star()),
        shard: meshes.add(shard()),
        bolts: [0, 1, 2, 3].map(|v| meshes.add(bolt(v))),
        blade: meshes.add(coloured(vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.5, 0.5, 0.0], [-0.5, 0.5, 0.0]], vec![grey(1.0); 4], vec![0, 1, 2, 0, 2, 3])),
        streak: meshes.add(streak()),
        swirl: meshes.add(swirl()),
        rim,
    });
    // The trails: vertex-coloured ribbons added onto the scene.
    let glow = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, ..default() });
    for (source, effect) in [(Source::Foot(-1.0), Effect::ExtraEffort), (Source::Foot(1.0), Effect::ExtraEffort), (Source::Ball, Effect::Kick)] {
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
    ExtraEffort,
    /// The kicked ball's trail.
    Kick,
}

impl Effect {
    /// Colour (linear), how long a point lives and the trail's width.
    fn look(self) -> (Vec3, f32, f32) {
        match self {
            // Extra effort's streaks on the legs (its rays: 00FFCB, 56FFFD, 4BB5FF), short now that
            // the rim and bolts carry the look.
            Effect::ExtraEffort => (Vec3::new(0.1, 1.0, 0.85), 0.12, 0.07),
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

/// The trails: extra effort behind both feet, the kicked ball behind it. Points age out after the
/// effect's life.
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
            Effect::ExtraEffort => player.extra_active,
            Effect::Kick => state.kick_trail,
        };
        let was_empty = trail.points.is_empty();
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
        // An idle trail's mesh is already empty: rebuilding it would only re-upload it.
        if was_empty && trail.points.is_empty() {
            continue;
        }
        if let Some(mesh) = meshes.get_mut(&trail.mesh) {
            *mesh = ribbon(&trail.points, eye, width, life, colour);
        }
    }
}

/// The kick marker: every shot, pass and lob gets one, push balls none (the game's rule): placed
/// where the ball will first hit the backboard or the goal frame (lying on it), or else where it
/// will stop, until the ball gets there. At the backboard or the frame the next one follows from
/// the bounce; it is gone when the ball has nearly stopped on it, when anyone gains the ball, or at
/// the next kick. Its arrow bobs over it.
pub fn kick_marker(
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<(&Transform, &Ball)>,
    mut marker: Single<(&mut Marker, &mut Transform, &mut Visibility), (Without<Ball>, Without<MarkerArrow>)>,
    mut arrow: Single<&mut Transform, (With<MarkerArrow>, Without<Ball>, Without<Marker>)>,
    mut last_kick: Local<Option<Kick>>,
) {
    let (btf, b) = *ball;
    let (m, tf, vis) = &mut *marker;
    let kicked = last_kick.is_some_and(|k| !matches!(k, Kick::PushBall | Kick::DribblePushBall)) && player.kick.is_none() && !b.held;
    *last_kick = player.kick.map(|k| k.kind);
    let arrived = m.chain && m.left > 0.0 && m.left <= time.delta_secs();
    let spot = if kicked || arrived {
        m.left = 0.0;
        marker_for(btf.translation, b)
    } else {
        None
    };
    if let Some(s) = spot {
        tf.translation = s.at + s.normal * 0.01;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, s.normal);
        (m.left, m.chain) = (s.t, s.normal != Vec3::Y);
        tf.scale = Vec3::splat(if m.chain { 1.0 } else { MARKER_GROUND_SCALE });
    }
    if b.held || player.kick.is_some() {
        m.left = 0.0;
    }
    m.left -= time.delta_secs();
    **vis = if m.left > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    arrow.translation.y = 0.42 + 0.04 * (time.elapsed_secs() * 4.0).sin();
}

/// The legs' materials (shins and boots), which glow in extra effort.
#[derive(Resource)]
pub struct LegMaterials(pub [Handle<StandardMaterial>; 2]);

/// Extra effort lights the boots cyan for as long as it lasts, with a slow pulse; it eases in and
/// out over 0.15 s. The shins' light is the rim (`rims`).
pub fn effort_legs(time: Res<Time>, player: Single<&Player>, legs: Res<LegMaterials>, mut materials: ResMut<Assets<StandardMaterial>>, mut glow: Local<f32>) {
    let target = if player.extra_active { 1.0 } else { 0.0 };
    let before = *glow;
    *glow += (target - *glow).clamp(-time.delta_secs() / 0.15, time.delta_secs() / 0.15);
    // Unlit, the legs keep their black emissive: writing it again would only re-upload them.
    if before == 0.0 && *glow == 0.0 {
        return;
    }
    let pulse = 0.8 + 0.2 * (time.elapsed_secs() * 12.0).sin();
    let c = Vec3::new(0.0, 1.0, 0.8) * *glow * pulse * 1.5;
    if let Some(m) = materials.get_mut(&legs.0[1]) {
        m.emissive = LinearRgba::rgb(c.x, c.y, c.z);
    }
}

/// How an effect piece turns.
#[derive(Clone, Copy, PartialEq)]
enum Facing {
    /// Keeps the rotation it was spawned with.
    Fixed,
    /// Turns its face (the mesh's XY plane) to the camera, rolling as it spins.
    Camera,
    /// Lies flat on the ground, spinning about the vertical.
    Flat,
    /// Tumbles about this axis.
    Tumble(Vec3),
}

/// One short-lived piece of an effect: it waits out its delay, then eases its scale, slides its
/// colour, holds and fades out over its life, and may ride the ball, fly, fall and spin.
#[derive(Component, Clone, Copy)]
pub struct Bit {
    age: f32,
    delay: f32,
    life: f32,
    colour: (Vec3, Vec3),
    /// Peak brightness (additive) or opacity (blended).
    alpha: f32,
    /// The share of its life it stays at full strength before fading out.
    hold: f32,
    scale: (Vec3, Vec3),
    /// Scale goes there and back (a pulse) instead of easing out to the end.
    pulse: bool,
    vel: Vec3,
    gravity: f32,
    drag: f32,
    roll: f32,
    spin: f32,
    face: Facing,
    /// Rides with the ball.
    follow: bool,
    /// Blended over the scene instead of added to it (for colours light can't add, like blue on
    /// grass).
    blend: bool,
}

impl Bit {
    fn new(life: f32, colour: Vec3) -> Bit {
        Bit {
            age: 0.0,
            delay: 0.0,
            life,
            colour: (colour, colour),
            alpha: 1.0,
            hold: 0.0,
            scale: (Vec3::ONE, Vec3::ONE),
            pulse: false,
            vel: Vec3::ZERO,
            gravity: 0.0,
            drag: 0.0,
            roll: 0.0,
            spin: 0.0,
            face: Facing::Camera,
            follow: false,
            blend: false,
        }
    }

    fn to(mut self, colour: Vec3) -> Bit {
        self.colour.1 = colour;
        self
    }

    fn alpha(mut self, alpha: f32) -> Bit {
        self.alpha = alpha;
        self
    }

    fn hold(mut self, hold: f32) -> Bit {
        self.hold = hold;
        self
    }

    fn size(self, from: f32, to: f32) -> Bit {
        self.sized(Vec3::splat(from), Vec3::splat(to))
    }

    fn sized(mut self, from: Vec3, to: Vec3) -> Bit {
        self.scale = (from, to);
        self
    }

    fn pulse(mut self) -> Bit {
        self.pulse = true;
        self
    }

    fn delay(mut self, delay: f32) -> Bit {
        self.delay = delay;
        self
    }

    fn moving(mut self, vel: Vec3, gravity: f32, drag: f32) -> Bit {
        (self.vel, self.gravity, self.drag) = (vel, gravity, drag);
        self
    }

    fn spin(mut self, roll: f32, spin: f32) -> Bit {
        (self.roll, self.spin) = (roll, spin);
        self
    }

    fn face(mut self, face: Facing) -> Bit {
        self.face = face;
        self
    }

    fn follow(mut self) -> Bit {
        self.follow = true;
        self
    }

    fn blend(mut self) -> Bit {
        self.blend = true;
        self
    }
}

/// Spawns effect pieces, each with its own material to fade.
struct Spawner<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    materials: &'a mut Assets<StandardMaterial>,
}

impl Spawner<'_, '_, '_> {
    fn bit(&mut self, mesh: &Handle<Mesh>, at: Vec3, bit: Bit) {
        self.turned(mesh, at, Quat::IDENTITY, bit);
    }

    fn turned(&mut self, mesh: &Handle<Mesh>, at: Vec3, rotation: Quat, bit: Bit) {
        let (base_color, alpha_mode) = if bit.blend { (Color::NONE, AlphaMode::Blend) } else { (Color::BLACK, AlphaMode::Add) };
        let material = self.materials.add(StandardMaterial { base_color, unlit: true, alpha_mode, double_sided: true, cull_mode: None, ..default() });
        let visibility = if bit.delay > 0.0 { Visibility::Hidden } else { Visibility::Inherited };
        self.commands.spawn((NotShadowCaster, Mesh3d(mesh.clone()), MeshMaterial3d(material), Transform { translation: at, rotation, scale: bit.scale.0 }, visibility, bit));
    }

    /// A ghost of the ball: a copy that fades where it was left.
    fn ghost(&mut self, fx: &FxAssets, at: Vec3, g: &Ghosts, strength: f32) {
        let bit = Bit::new(g.life, g.colour).alpha(g.alpha * strength).hold(0.2).size(1.12, 0.92).face(Facing::Fixed);
        self.bit(&fx.sphere, at, if g.blend { bit.blend() } else { bit });
    }
}

/// The effect pieces: their timing, motion, turn, scale and fade.
pub fn animate_bits(
    mut commands: Commands,
    time: Res<Time>,
    ball: Single<&Transform, (With<Ball>, Without<Bit>)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut bits: Query<(Entity, &mut Bit, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    let eye = camera.translation();
    for (e, mut b, mut tf, mut vis, mat) in &mut bits {
        b.age += dt;
        let t = b.age - b.delay;
        if t < 0.0 {
            continue;
        }
        let u = t / b.life;
        if u >= 1.0 {
            materials.remove(&mat.0);
            commands.entity(e).despawn();
            continue;
        }
        vis.set_if_neq(Visibility::Inherited);
        if b.follow {
            tf.translation = ball.translation;
        } else {
            b.vel = b.vel * (1.0 - b.drag * dt).max(0.0) - Vec3::Y * b.gravity * dt;
            tf.translation += b.vel * dt;
        }
        let roll = b.roll + b.spin * t;
        tf.rotation = match b.face {
            Facing::Fixed => tf.rotation,
            Facing::Camera => Transform::from_translation(tf.translation).looking_at(eye, Vec3::Y).rotation * Quat::from_rotation_z(roll),
            Facing::Flat => Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(roll),
            Facing::Tumble(axis) => Quat::from_axis_angle(axis, roll),
        };
        let shape = if b.pulse { (u * PI).sin() } else { 1.0 - (1.0 - u) * (1.0 - u) };
        tf.scale = b.scale.0.lerp(b.scale.1, shape);
        let strength = if u < b.hold { 1.0 } else { 1.0 - (u - b.hold) / (1.0 - b.hold) };
        let c = b.colour.0.lerp(b.colour.1, u);
        if let Some(m) = materials.get_mut(&mat.0) {
            m.base_color = if b.blend {
                Color::linear_rgba(c.x, c.y, c.z, b.alpha * strength)
            } else {
                let c = c * b.alpha * strength;
                Color::linear_rgb(c.x, c.y, c.z)
            };
        }
    }
}

/// The dribble (GC_Dribble, NS_Dribble), on the ball as the move starts: a soft cyan puff, a
/// light-blue shell for 0.2 s and teal ghost balls left along its path.
pub fn dribble_fx(
    mut commands: Commands,
    player: Single<&Player>,
    ball: Single<&Transform, With<Ball>>,
    fx: Res<FxAssets>,
    mut state: ResMut<FxState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<Option<f32>>,
) {
    let started = player.dribble.is_some_and(|d| last.is_none_or(|t| d.t < t));
    *last = player.dribble.map(|d| d.t);
    if !started {
        return;
    }
    let at = ball.translation;
    let mut s = Spawner { commands: &mut commands, materials: &mut materials };
    s.bit(&fx.soft, at, Bit::new(0.1, PUFF).alpha(1.3).size(0.4, 0.1).delay(0.03).follow());
    s.bit(&fx.sphere, at, Bit::new(0.2, DRIBBLE_BLUE).alpha(1.1).hold(0.5).size(1.25, 1.1).delay(0.06).face(Facing::Fixed).follow());
    state.ghosts.push(Ghosts { delay: 0.08, left: 0.2, spacing: 0.14, colour: TEAL, alpha: 0.75, life: 0.15, blend: false, last: None });
}

/// A shot's strike (NS_Impact_Shoot_Low/Mid/High), where the foot met the ball: spikes and a flare,
/// cyan-white on a weak shot, white and bigger with power; a power shot adds a gold glow and gold
/// shards falling around the foot. Grass blades fly up from the ground under it.
fn shot_strike(s: &mut Spawner, fx: &FxAssets, rng: &mut Rng, at: Vec3, way: Vec3, speed: f32) {
    let power = if speed >= SHOT_TOP_SPEED {
        2
    } else if speed >= SHOT_HIGH_POWER_SPEED {
        1
    } else {
        0
    };
    let (spike_colour, spike, flare) = [(ICE, 0.3, 0.12), (WHITE, 0.45, 0.25), (WHITE, 0.55, 0.35)][power];
    let roll = rng.next() * TAU;
    s.bit(&fx.star, at, Bit::new(0.1, spike_colour).alpha(1.8).hold(0.5).size(spike * 0.3, spike).pulse().spin(roll, 0.0));
    if power > 0 {
        s.bit(&fx.star, at, Bit::new(0.1, spike_colour).alpha(1.5).hold(0.5).size(spike * 0.25, spike * 0.85).pulse().spin(roll + 0.35, 0.0).delay(0.03));
        let glow = if power == 2 { 0.5 } else { 0.35 };
        s.bit(&fx.soft, at, Bit::new(0.16, GOLD_GLOW).alpha(1.4).hold(0.3).size(glow * 0.7, glow));
    }
    s.bit(&fx.soft, at, Bit::new(0.05 + 0.015 * power as f32, spike_colour).alpha(2.0).size(flare * 0.7, flare * 1.5));
    // Shards: thin cyan slivers on a weak shot, gold triangles on stronger ones.
    let (count, colour, size) = [(4, ICE, Vec3::new(0.03, 0.09, 1.0)), (6, GOLD, Vec3::splat(0.1)), (10, GOLD, Vec3::splat(0.13))][power];
    for _ in 0..count {
        let vel = rng.flat() * rng.range(1.5, 3.5) + Vec3::Y * rng.range(0.4, 1.6);
        let spin = rng.range(10.0, 20.0);
        let bit = Bit::new(rng.range(0.2, 0.3), colour).alpha(1.5).hold(0.4).sized(size, size * 0.6).moving(vel, 9.0, 0.0).spin(0.0, spin).face(Facing::Tumble(rng.axis()));
        s.bit(&fx.shard, at, bit);
    }
    // Turf kicked up and on from the ground under the ball (NS_Grass_Shoot).
    let flat = Vec3::new(way.x, 0.0, way.z).normalize_or(Vec3::NEG_Z);
    let ground = Vec3::new(at.x, 0.03, at.z);
    for _ in 0..[6, 10, 14][power] {
        let vel = flat * rng.range(0.8, 3.0) + rng.flat() * 0.7 + Vec3::Y * rng.range(1.5, 3.5);
        let bit = Bit::new(rng.range(0.45, 0.7), GRASS_BLADE).hold(0.75).sized(Vec3::new(0.025, 0.1, 1.0), Vec3::new(0.02, 0.08, 1.0)).moving(vel, 9.0, 1.0).spin(0.0, rng.range(8.0, 16.0));
        s.bit(&fx.blade, ground, bit.face(Facing::Tumble(rng.axis())).blend());
    }
}

/// A push ball's touch (NS_Impact_PushBall, NS_BallLost): white spikes for a frame or two, then a
/// blue smear behind the ball (a blue shell and blue ghosts left close together).
fn push_touch(s: &mut Spawner, fx: &FxAssets, rng: &mut Rng, state_ghosts: &mut Vec<Ghosts>, at: Vec3) {
    s.bit(&fx.star, at, Bit::new(0.06, WHITE).alpha(1.6).size(0.12, 0.3).pulse().spin(rng.next() * TAU, 0.0));
    s.bit(&fx.sphere, at, Bit::new(0.13, PUSH_BLUE).alpha(0.55).hold(0.4).size(1.3, 1.1).delay(0.04).face(Facing::Fixed).follow().blend());
    state_ghosts.push(Ghosts { delay: 0.03, left: 0.15, spacing: 0.05, colour: PUSH_BLUE, alpha: 0.35, life: 0.12, blend: true, last: None });
}

/// A pass or lob leaving the foot (NS_Impact_Tap): a blue flash on the ground under the ball, a few
/// blue sparks and a small cyan burst.
fn pass_release(s: &mut Spawner, fx: &FxAssets, rng: &mut Rng, at: Vec3) {
    let ground = Vec3::new(at.x, 0.02, at.z);
    s.bit(&fx.soft, ground, Bit::new(0.12, PASS_BLUE).alpha(0.8).hold(0.3).size(0.15, 0.3).face(Facing::Flat).blend());
    s.bit(&fx.star, at, Bit::new(0.05, ICE).alpha(1.4).size(0.08, 0.18).pulse().spin(rng.next() * TAU, 0.0));
    for _ in 0..5 {
        let vel = rng.flat() * rng.range(1.5, 2.5) + Vec3::Y * rng.range(0.3, 1.0);
        let bit = Bit::new(0.12, Vec3::new(0.4, 0.7, 1.0)).alpha(1.5).sized(Vec3::new(0.02, 0.06, 1.0), Vec3::new(0.01, 0.04, 1.0)).moving(vel, 4.0, 0.0);
        s.bit(&fx.shard, at, bit.face(Facing::Tumble(rng.axis())).spin(0.0, 15.0));
    }
}

/// Two broken rings closing on the ball, camera-facing and turning against each other, painted over
/// the scene so their colour stays true on the grass.
fn closing_rings(s: &mut Spawner, fx: &FxAssets, rng: &mut Rng, at: Vec3, rings: [(f32, f32); 2], life: f32, from: Vec3, to: Vec3) {
    for (i, (r0, r1)) in rings.into_iter().enumerate() {
        let way = if i == 0 { 1.0 } else { -1.0 };
        s.bit(&fx.arcs, at, Bit::new(life, from).to(to).alpha(1.0).hold(0.6).size(r0, r1).spin(rng.next() * TAU, way * 2.5).follow().blend());
    }
}

/// The moves' bursts at the ball: a kick's strike by kind (shots by power, push balls, passes and
/// lobs), with the kicked ball's trail until anyone gains it (not for push balls on the ground); a
/// control's yellow rings closing on the ball, its shell going yellow to white to blue and ghosts
/// where it came from; a fake shot's orange rings as the leg passes over the ball. Also the ghost
/// runs and the ball's recent path.
pub fn action_fx(
    mut commands: Commands,
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<(&Transform, &Ball)>,
    fx: Res<FxAssets>,
    mut state: ResMut<FxState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_kick: Local<Option<Kick>>,
    mut was_receiving: Local<bool>,
) {
    let (p, (btf, b)) = (*player, *ball);
    let (at, now, dt) = (btf.translation, time.elapsed_secs(), time.delta_secs());
    let state = &mut *state;
    state.history.push((now, at));
    state.history.retain(|(t, _)| now - t < 0.35);
    state.control_age += dt;
    let mut s = Spawner { commands: &mut commands, materials: &mut materials };

    let kicked = last_kick.filter(|_| p.kick.is_none() && !b.held);
    *last_kick = p.kick.map(|k| k.kind);
    if let Some(kind) = kicked {
        match kind {
            Kick::Shot => shot_strike(&mut s, &fx, &mut state.rng, at, b.vel.normalize_or(Vec3::NEG_Z), b.vel.length()),
            Kick::PushBall | Kick::DribblePushBall => push_touch(&mut s, &fx, &mut state.rng, &mut state.ghosts, at),
            _ => pass_release(&mut s, &fx, &mut state.rng, at),
        }
        state.kick_trail = !matches!(kind, Kick::PushBall | Kick::DribblePushBall) || at.y > 0.75;
    }
    if b.held {
        state.kick_trail = false;
    }

    // A control shows on the ball (players confirm), in the ally colour (GC_HasBall).
    if p.receiving.is_some() && !*was_receiving {
        closing_rings(&mut s, &fx, &mut state.rng, at, [(0.6, 0.14), (0.42, 0.13)], 0.2, YELLOW, PALE_YELLOW);
        s.bit(&fx.sphere, at, Bit::new(0.15, YELLOW * 0.8 + Vec3::splat(0.2)).to(WHITE).alpha(0.9).size(1.35, 1.15).face(Facing::Fixed).follow());
        s.bit(&fx.sphere, at, Bit::new(0.14, CONTROL_BLUE).alpha(0.9).size(1.3, 1.1).delay(0.12).face(Facing::Fixed).follow());
        let ghosts = Ghosts { delay: 0.0, left: 0.3, spacing: 0.12, colour: TEAL, alpha: 0.7, life: 0.15, blend: false, last: Some(at) };
        // Where it came from: the ball a moment ago, fainter the older.
        for (i, back) in [0.05, 0.1, 0.15].into_iter().enumerate() {
            if let Some(&(_, then)) = state.history.iter().find(|(t, _)| now - t <= back)
                && then.distance(at) > 0.08
            {
                s.ghost(&fx, then, &ghosts, 1.0 - 0.25 * i as f32);
            }
        }
        state.ghosts.push(ghosts);
        state.control_age = 0.0;
    }
    *was_receiving = p.receiving.is_some();

    // The fake shot's rings, as the leg swings over the ball.
    match p.fake {
        Some((t, _)) if t >= FAKE_SHOT_SWING && !state.fake_shown => {
            state.fake_shown = true;
            closing_rings(&mut s, &fx, &mut state.rng, at, [(0.42, 0.2), (0.3, 0.16)], 0.25, FAKE_RED, FAKE_PINK);
            s.bit(&fx.sphere, at, Bit::new(0.25, PEACH).to(FAKE_PINK).alpha(0.8).hold(0.3).size(1.35, 1.2).face(Facing::Fixed).follow());
        }
        None => state.fake_shown = false,
        _ => {}
    }

    // Ghost runs: a ball copy each time the ball has moved on far enough.
    for g in &mut state.ghosts {
        if g.delay > 0.0 {
            g.delay -= dt;
            continue;
        }
        g.left -= dt;
        if g.last.is_none_or(|l| l.distance(at) >= g.spacing) {
            g.last = Some(at);
            s.ghost(&fx, at, g, 1.0);
        }
    }
    state.ghosts.retain(|g| g.left > 0.0);
}

/// Extra effort (NS_ExtraEffort_Activation, _Loop, _Footstep): as it starts, a white-cyan swirl
/// bursts on the ground around you with lightning flung out; while it lasts, small bolts flicker at
/// your feet and calves and each foot plant leaves a cyan streak on the ground.
pub fn effort_fx(
    mut commands: Commands,
    time: Res<Time>,
    player: Single<(&Player, &Transform)>,
    feet: Query<(&TrailPoint, &GlobalTransform)>,
    fx: Res<FxAssets>,
    mut state: ResMut<FxState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut was_on: Local<bool>,
    mut bolt_wait: Local<f32>,
    mut planted: Local<[bool; 2]>,
) {
    let (p, root) = *player;
    let on = p.extra_active;
    let started = on && !*was_on;
    *was_on = on;
    if !on {
        *planted = [true; 2];
        return;
    }
    let rng = &mut state.rng;
    let mut s = Spawner { commands: &mut commands, materials: &mut materials };
    let ground = Vec3::new(root.translation.x, 0.02, root.translation.z);
    if started {
        s.bit(&fx.swirl, ground, Bit::new(0.18, WHITE).to(EE_WHITE).alpha(1.3).hold(0.25).size(0.5, 1.25).delay(0.05).face(Facing::Flat).spin(rng.next() * TAU, 4.0));
        for i in 0..8 {
            let way = rng.flat();
            let bit = Bit::new(rng.range(0.15, 0.22), BOLT).alpha(1.6).hold(0.5).size(rng.range(0.18, 0.25), 0.15).delay(0.05).moving(way * rng.range(3.5, 5.0), 0.0, 2.0);
            s.bit(&fx.bolts[i % 4], ground + way * 0.2 + Vec3::Y * rng.range(0.05, 0.2), bit.spin(rng.next() * TAU, 0.0));
        }
    }
    // Bolts at the feet and calves, one every 0.07 s.
    *bolt_wait -= time.delta_secs();
    let feet: Vec<(f32, Vec3)> = feet.iter().map(|(f, g)| (f.0, g.translation())).collect();
    if *bolt_wait <= 0.0 && !feet.is_empty() {
        *bolt_wait = 0.07;
        let foot = feet[(rng.next() * feet.len() as f32) as usize % feet.len()].1;
        let at = foot + Vec3::Y * rng.range(0.0, 0.35) + rng.flat() * 0.08;
        s.bit(&fx.bolts[(rng.next() * 4.0) as usize % 4], at, Bit::new(0.09, BOLT).alpha(1.4).size(0.13, 0.11).spin(rng.next() * TAU, 0.0));
    }
    // A footprint as each boot comes down onto the ground.
    let run = Vec3::new(p.vel.x, 0.0, p.vel.z);
    let yaw = if run.length() > 0.5 { f32::atan2(-run.x, -run.z) } else { p.facing };
    for (side, at) in feet {
        let i = (side > 0.0) as usize;
        if !planted[i] && at.y < 0.07 {
            planted[i] = true;
            let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-FRAC_PI_2);
            let bit = Bit::new(0.28, FOOTPRINT).alpha(1.2).hold(0.2).sized(Vec3::new(0.1, 0.5, 1.0), Vec3::new(0.12, 0.55, 1.0)).face(Facing::Fixed);
            s.turned(&fx.streak, Vec3::new(at.x, 0.015, at.z), rotation, bit);
        } else if at.y > 0.1 {
            planted[i] = false;
        }
    }
}

/// The legs' rim: a slightly larger shell behind each shin and boot that only shows round their
/// outline. Lavender in extra effort (its body overlay), easing in and out over 0.15 s with a
/// flicker; a yellow flash for 0.4 s when you gain the ball (the control's player overlay).
pub fn rims(
    mut commands: Commands,
    time: Res<Time>,
    player: Single<&Player>,
    legs: Option<Res<LegMaterials>>,
    parts: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    mut shells: Query<&mut Visibility, With<Rim>>,
    fx: Res<FxAssets>,
    state: Res<FxState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut built: Local<bool>,
    mut glow: Local<f32>,
) {
    if !*built && let Some(legs) = legs {
        for (e, mesh, mat) in &parts {
            if legs.0.contains(&mat.0) {
                commands.entity(e).with_child((Rim, NotShadowCaster, Mesh3d(mesh.0.clone()), MeshMaterial3d(fx.rim.clone()), Transform::from_scale(Vec3::splat(1.2)), Visibility::Hidden));
            }
        }
        *built = true;
        return;
    }
    let dt = time.delta_secs();
    *glow += ((player.extra_active as u8 as f32) - *glow).clamp(-dt / 0.15, dt / 0.15);
    let flicker = 0.85 + 0.15 * (time.elapsed_secs() * 30.0).sin();
    let flash = (1.0 - state.control_age / 0.4).max(0.0).powf(1.5);
    let c = EE_RIM * 1.6 * *glow * flicker + YELLOW * 1.2 * flash;
    let shown = c.max_element() > 0.001;
    for mut vis in &mut shells {
        vis.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
    if shown && let Some(m) = materials.get_mut(&fx.rim) {
        m.base_color = Color::linear_rgb(c.x, c.y, c.z);
    }
}

/// The ball's shadow (NS_Shadow_Ball), straight under it however high it is: a soft dark disc that
/// grows and pales as the ball rises. The ball casts no light shadow of its own.
pub fn ball_shadow(
    mut commands: Commands,
    ball: Single<(Entity, &Transform, Has<NotShadowCaster>), (With<Ball>, Without<BallShadow>)>,
    mut shadow: Single<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<BallShadow>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (e, btf, cast_off) = *ball;
    if !cast_off {
        commands.entity(e).insert(NotShadowCaster);
    }
    let (tf, mat) = &mut *shadow;
    let rise = ((btf.translation.y - BALL_RADIUS) / 8.0).clamp(0.0, 1.0);
    tf.translation = Vec3::new(btf.translation.x, 0.012, btf.translation.z);
    tf.scale = Vec3::splat(0.15 * (1.0 + 1.2 * rise));
    let alpha = 0.5 * (1.0 - 0.55 * rise);
    if let Some(m) = materials.get_mut(&mat.0)
        && (m.base_color.alpha() - alpha).abs() > 0.005
    {
        m.base_color.set_alpha(alpha);
    }
}

/// Shows the diamond on the ball while a volley is on (players' "lock" diamond), facing the camera:
/// four pale cyan arrows in a blue bloom; as it appears the arrows flash white and the bloom swells,
/// settling over 0.17 s; it goes at once when the volley is off. Both are painted over the scene, so
/// the grass doesn't turn them green.
pub fn volley_diamond(
    time: Res<Time>,
    player: Single<(&Player, &Transform), Without<VolleyDiamond>>,
    ball: Single<(&Transform, &Ball), (Without<Player>, Without<VolleyDiamond>)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut parts: Query<(&VolleyDiamond, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shown: Local<f32>,
) {
    let ((p, root), (btf, b)) = (*player, *ball);
    let on = volley_target(p, root, btf.translation, b).is_some();
    *shown = if on { *shown + time.delta_secs() } else { 0.0 };
    let fade_in = (*shown / 0.05).min(1.0);
    let flare = 1.0 - (*shown / 0.17).min(1.0);
    let facing = Transform::from_translation(btf.translation).looking_at(camera.translation(), Vec3::Y);
    for (part, mut tf, mut vis, mat) in &mut parts {
        vis.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        if !on {
            continue;
        }
        let (c, alpha, scale) = match part {
            // Brighter than white can paint, so they stay pale and lit once tone-mapped.
            VolleyDiamond::Arrows => (DIAMOND.lerp(WHITE, 0.7 * flare) * (1.6 + flare), 1.0, 1.0),
            VolleyDiamond::Glow => (DIAMOND_GLOW, 0.35 + 0.55 * flare, 0.33 * (1.0 + 0.4 * flare)),
        };
        *tf = facing.with_scale(Vec3::splat(scale));
        if let Some(m) = materials.get_mut(&mat.0) {
            m.base_color = Color::linear_rgba(c.x, c.y, c.z, alpha * fade_in);
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
        let s = marker_for(at, &pass).expect("a full pass is marked");
        let (mut pos, mut rolled) = (at, pass.clone());
        for _ in 0..(s.t * 240.0) as usize {
            step(&mut pos, &mut rolled, 1.0 / 240.0);
        }
        assert!(Vec2::new(pos.x - s.at.x, pos.z - s.at.z).length() < 0.05 && s.t > 1.0 && s.normal == Vec3::Y, "{} at {}", s.at, s.t);
        // A ball going in gets none.
        let shot = Ball { vel: Vec3::new(0.0, 1.0, -35.0), ..default() };
        assert!(marker_for(Vec3::new(0.0, BALL_RADIUS, -30.0), &shot).is_none());
        // A shot wide of the goal is marked on the end wall, lying on it.
        let wide = Ball { vel: Vec3::new(0.0, 1.0, -35.0), ..default() };
        let s = marker_for(Vec3::new(GOAL_HALF_WIDTH + 3.0, BALL_RADIUS, -20.0), &wide).expect("marked on the wall");
        assert!(s.normal == Vec3::Z && (s.at.z + HALF_LENGTH).abs() < 0.01, "{} {}", s.at, s.normal);
        // A ball across to a side wall is marked past it, not on it.
        let across = Ball { vel: Vec3::new(30.0, 0.0, -2.0), friction: PASS_FRICTION, ..default() };
        let s = marker_for(Vec3::new(0.0, BALL_RADIUS, 0.0), &across).expect("marked");
        assert!((s.at.x.abs() - HALF_WIDTH).abs() > 0.05, "{}", s.at);
    }

    #[test]
    fn effect_meshes_hold_together() {
        // Every index points at a vertex, and the swirl keeps its disc and adds three arms.
        for mesh in [diamond(), soft(), swirl(), arcs(), star(), shard(), bolt(2), streak(), arrow()] {
            let n = mesh.count_vertices() as u32;
            let Some(Indices::U32(indices)) = mesh.indices() else { panic!("u32 indices") };
            assert!(indices.len() % 3 == 0 && indices.iter().all(|i| *i < n), "{n} vertices");
        }
        assert_eq!(swirl().count_vertices(), soft_disc_vertices(5, 40) + 3 * 13 * 2);
        assert_eq!(diamond().count_vertices(), 12);
    }

    /// A `disc` with this many bands (the centre included) and segments.
    fn soft_disc_vertices(bands: usize, segments: usize) -> usize {
        1 + (bands - 1) * (segments + 1)
    }

    #[test]
    fn bits_fade_to_nothing_and_grow_as_asked() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<Mesh>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / 60.0)))
            .add_systems(Update, animate_bits);
        app.world_mut().spawn((Ball::default(), Transform::default()));
        app.world_mut().spawn((Camera3d::default(), GlobalTransform::from_xyz(0.0, 2.0, 5.0)));
        let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        let bit = Bit::new(0.2, WHITE).size(1.0, 2.0).hold(0.7);
        let e = app.world_mut().spawn((bit, Transform::default(), Visibility::Inherited, MeshMaterial3d(material.clone()))).id();
        for _ in 0..8 {
            app.update();
        }
        // Over halfway through its life (the first frame has no time step): bigger, still at full
        // light.
        let tf = *app.world().get::<Transform>(e).expect("alive");
        assert!(tf.scale.x > 1.5 && tf.scale.x < 2.0, "{}", tf.scale);
        let lit = app.world().resource::<Assets<StandardMaterial>>().get(&material).expect("material").base_color.to_linear();
        assert!(lit.red > 0.9, "{lit:?}");
        for _ in 0..12 {
            app.update();
        }
        assert!(app.world().get_entity(e).is_err(), "gone once its life is out");
    }
}

/// The "Show control zones" cheat: on the ground around you, the control zone that applies now
/// (red while your magnet is off or the ball can't be taken), the extra-effort touch reach and the
/// reach of a dash onto the ball, and a line the way you face.
pub fn debug_zones(cheats: Res<Cheats>, player: Single<(&Transform, &Player)>, ball: Single<&Ball>, mut gizmos: Gizmos) {
    if !cheats.on(Cheat::ShowZones) {
        return;
    }
    let (tf, p) = *player;
    let flat = Quat::from_rotation_x(FRAC_PI_2);
    let ground = Vec3::new(tf.translation.x, 0.02, tf.translation.z);
    let fwd = crate::player::forward(p.facing);
    let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    let (ahead, radius, _) = if p.locked {
        CONTROL_LOCK_MOVE
    } else if speed > 0.5 {
        CONTROL_MOVING
    } else {
        CONTROL_STANDING
    };
    let off = p.magnet_off > 0.0 || ball.no_control > 0.0;
    let colour = if off { Color::srgb(1.0, 0.2, 0.2) } else { Color::srgb(0.2, 1.0, 0.4) };
    gizmos.circle(Isometry3d::new(ground + fwd * ahead, flat), radius, colour);
    gizmos.circle(Isometry3d::new(ground, flat), EXTRA_EFFORT_KNOCK_REACH, Color::srgb(0.2, 0.9, 1.0));
    gizmos.circle(Isometry3d::new(ground, flat), DASH_DRIBBLE_REACH, Color::srgba(1.0, 1.0, 1.0, 0.4));
    gizmos.line(ground, ground + fwd * 1.5, Color::srgb(1.0, 0.9, 0.2));
}
