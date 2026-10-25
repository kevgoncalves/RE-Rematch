//! The aim line for passes and lobs, low-power ones included, as in Rematch: a strip on the ground
//! from the ball in the pass direction, following the pass aiming mode. It comes in thin and pale
//! blue for 0.1 s, then turns green, brighter as the pass gets stronger (Kev's recordings turn it
//! green as fast for a tap as for a long hold, notes/vfx.md); a few angular shards float over it
//! near the ball; lobs bend it into an arc. As the ball leaves it turns cyan, thins and goes in
//! 0.1 s.

use std::f32::consts::PI;

use bevy::asset::RenderAssetUsages;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

use crate::ball::Ball;
use crate::input::{Axes, Bindings};
use crate::player::{CameraRig, Kick, Player, crosshair_yaw, forward, pass_yaw};
use crate::reticle::power;

/// Rematch's line mesh: 28.6 cm wide, 5.1 m long.
const WIDTH: f32 = 0.286;
const LENGTH: f32 = 5.1;
/// The line's first look (pale blue), its green (light lime, brighter with charge) and its colour as
/// the ball leaves (the recordings' #89C4D2, #ADED9E and a cyan).
const INTRO: Color = Color::srgb(0.54, 0.77, 0.82);
const WEAK: Color = Color::srgb(0.62, 0.93, 0.55);
const STRONG: Color = Color::srgb(0.45, 1.0, 0.5);
const RELEASE: Color = Color::srgb(0.4, 0.85, 1.0);
/// Seconds: the thin blue intro, the turn to green after it, and the fade once the ball leaves.
const INTRO_TIME: f32 = 0.1;
const TURN_TIME: f32 = 0.1;
const RELEASE_TIME: f32 = 0.1;
/// TUNE: how high the lob arc rises, and how fast each line fades along its length.
const LOB_ARC: f32 = 0.6;
const FADE: f32 = 1.5;
const SHORT_FADE: f32 = 4.0;
/// The shards over the line: how many, and the stretch from the ball they drift along (m).
const SHARDS: usize = 4;
const SHARD_STRETCH: f32 = 0.9;

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct PassLineMaterial {
    #[uniform(0)]
    color: LinearRgba,
    /// Arc height, fade power.
    #[uniform(1)]
    shape: Vec4,
}

impl Material for PassLineMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/pass_line.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/pass_line.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Component)]
pub struct PassLine;

/// One of the shards floating over the line near the ball.
#[derive(Component)]
pub struct PassShard(usize);

/// A flat strip one unit wide and long, running from the origin along -Z (forward), split along
/// its length so lobs can bend it. UV: x across, y along.
fn strip(segments: u32) -> Mesh {
    let (mut positions, mut uvs) = (Vec::new(), Vec::new());
    for i in 0..=segments {
        let v = i as f32 / segments as f32;
        for u in [0.0, 1.0] {
            positions.push([u - 0.5, 0.0, -v]);
            uvs.push([u, v]);
        }
    }
    let indices = (0..segments).flat_map(|i| [2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 1, 2 * i + 3, 2 * i + 2]).collect();
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

pub fn spawn_pass_line(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PassLineMaterial>>, mut lights: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        PassLine,
        Mesh3d(meshes.add(strip(32))),
        MeshMaterial3d(materials.add(PassLineMaterial::default())),
        Transform::from_scale(Vec3::new(WIDTH, 1.0, LENGTH)),
        Visibility::Hidden,
        NotShadowCaster,
    ));
    let shard = meshes.add(Tetrahedron::default());
    let light = lights.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, ..default() });
    for i in 0..SHARDS {
        commands.spawn((PassShard(i), Mesh3d(shard.clone()), MeshMaterial3d(light.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster));
    }
}

/// The line's own timing: how long it has shown, and how far into its fade once the ball left.
#[derive(Default)]
pub struct LineState {
    shown: f32,
    release: Option<f32>,
}

pub fn update_pass_line(
    time: Res<Time>,
    player: Single<&Player>,
    ball: Single<&Transform, (With<Ball>, Without<PassLine>)>,
    rig: Single<&CameraRig>,
    camera: Single<&Transform, (With<Camera3d>, Without<Ball>, Without<PassLine>)>,
    axes: Res<Axes>,
    bindings: Res<Bindings>,
    mut line: Single<(&mut Transform, &mut Visibility, &MeshMaterial3d<PassLineMaterial>), (With<PassLine>, Without<PassShard>)>,
    mut shards: Query<(&PassShard, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>), (Without<PassLine>, Without<Ball>, Without<Camera3d>)>,
    mut materials: ResMut<Assets<PassLineMaterial>>,
    mut lights: ResMut<Assets<StandardMaterial>>,
    mut state: Local<LineState>,
) {
    let dt = time.delta_secs();
    let (tf, vis, handle) = &mut *line;
    // Shown from the press until the ball leaves, for every pass and lob, then a short fade.
    let Some(k) = player.kick.filter(|k| k.kind.is_pass() && player.has_ball) else {
        if state.shown > 0.0 {
            state.release = Some(0.0);
        }
        state.shown = 0.0;
        for (_, _, mut v, _) in &mut shards {
            v.set_if_neq(Visibility::Hidden);
        }
        let Some(r) = state.release.as_mut() else {
            vis.set_if_neq(Visibility::Hidden);
            return;
        };
        *r += dt;
        let u = *r / RELEASE_TIME;
        if u >= 1.0 {
            state.release = None;
            vis.set_if_neq(Visibility::Hidden);
            return;
        }
        tf.scale.x = WIDTH * (1.0 - 0.7 * u);
        if let Some(m) = materials.get_mut(&handle.0) {
            m.color = RELEASE.with_alpha(1.0 - u).to_linear();
        }
        return;
    };
    state.shown += dt;
    state.release = None;
    vis.set_if_neq(Visibility::Inherited);
    let yaw = pass_yaw(bindings.pass_aim, &axes, &rig, crosshair_yaw(camera.translation, rig.yaw, ball.translation));
    tf.translation = Vec3::new(ball.translation.x, 0.02, ball.translation.z) + forward(yaw) * 0.1;
    tf.rotation = Quat::from_rotation_y(yaw);
    // Thin at first, then the full width.
    tf.scale.x = WIDTH * (0.25 + 0.75 * (state.shown / INTRO_TIME).min(1.0));
    let charge = power(k.kind, k.held);
    let turn = ((state.shown - INTRO_TIME) / TURN_TIME).clamp(0.0, 1.0);
    let colour = INTRO.mix(&WEAK.mix(&STRONG, charge), turn);
    let arc = if matches!(k.kind, Kick::Lob | Kick::ShortLob) { LOB_ARC } else { 0.0 };
    if let Some(m) = materials.get_mut(&handle.0) {
        m.color = colour.with_alpha(0.8 + 0.2 * charge).to_linear();
        let fade = if matches!(k.kind, Kick::ShortPass | Kick::ShortLob) { SHORT_FADE } else { FADE };
        m.shape = Vec4::new(arc, fade, 0.0, 0.0);
    }

    // The shards drift out along the line near the ball, bobbing and turning, smallest at the ends
    // of their stretch; they take the line's colour.
    let t = time.elapsed_secs();
    let (ahead, side) = (tf.rotation * Vec3::NEG_Z, tf.rotation * Vec3::X);
    let grow = (state.shown / (INTRO_TIME + TURN_TIME)).min(1.0);
    for (shard, mut stf, mut v, mat) in &mut shards {
        let i = shard.0 as f32;
        let along = (i / SHARDS as f32 + t * 0.35).fract();
        let d = 0.2 + along * SHARD_STRETCH;
        let lift = 0.07 + 0.04 * (t * 5.0 + i * 1.7).sin() + arc * (PI * d / LENGTH).sin();
        let sway = if shard.0 % 2 == 0 { 0.05 } else { -0.05 };
        stf.translation = tf.translation + ahead * d + side * sway + Vec3::Y * lift;
        stf.rotation = Quat::from_rotation_y(t * 2.0 + i) * Quat::from_rotation_x(0.4 + i);
        stf.scale = Vec3::new(0.08, 0.12, 0.06) * grow * (0.3 + 0.7 * (along * PI).sin());
        v.set_if_neq(Visibility::Inherited);
        if shard.0 == 0
            && let Some(m) = lights.get_mut(&mat.0)
        {
            let c = colour.to_linear();
            m.base_color = Color::linear_rgb(0.9 * c.red, 0.9 * c.green, 0.9 * c.blue);
        }
    }
}
