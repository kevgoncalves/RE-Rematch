//! The aim line for passes and lobs, low-power ones included, as in Rematch: a strip on the ground
//! from the ball in the pass direction, following the pass aiming mode. It goes from blue to green
//! as the pass gets stronger; lobs bend it into an arc.

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
const WEAK: Color = Color::srgb(0.35, 0.65, 1.0);
const STRONG: Color = Color::srgb(0.45, 1.0, 0.55);
/// TUNE: how high the lob arc rises, and how fast each line fades along its length.
const LOB_ARC: f32 = 0.6;
const FADE: f32 = 1.5;
const SHORT_FADE: f32 = 4.0;

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

pub fn spawn_pass_line(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PassLineMaterial>>) {
    commands.spawn((
        PassLine,
        Mesh3d(meshes.add(strip(32))),
        MeshMaterial3d(materials.add(PassLineMaterial::default())),
        Transform::from_scale(Vec3::new(WIDTH, 1.0, LENGTH)),
        Visibility::Hidden,
        NotShadowCaster,
    ));
}

pub fn update_pass_line(
    player: Single<&Player>,
    ball: Single<&Transform, (With<Ball>, Without<PassLine>)>,
    rig: Single<&CameraRig>,
    camera: Single<&Transform, (With<Camera3d>, Without<Ball>, Without<PassLine>)>,
    axes: Res<Axes>,
    bindings: Res<Bindings>,
    mut line: Single<(&mut Transform, &mut Visibility, &MeshMaterial3d<PassLineMaterial>), With<PassLine>>,
    mut materials: ResMut<Assets<PassLineMaterial>>,
) {
    let (tf, vis, handle) = &mut *line;
    // Shown from the press until the ball leaves, for every pass and lob.
    let Some(k) = player.kick.filter(|k| k.kind.is_pass() && player.has_ball) else {
        **vis = Visibility::Hidden;
        return;
    };
    **vis = Visibility::Inherited;
    let yaw = pass_yaw(bindings.pass_aim, &axes, &rig, crosshair_yaw(camera.translation, rig.yaw, ball.translation));
    tf.translation = Vec3::new(ball.translation.x, 0.02, ball.translation.z) + forward(yaw) * 0.1;
    tf.rotation = Quat::from_rotation_y(yaw);
    if let Some(m) = materials.get_mut(&handle.0) {
        m.color = WEAK.mix(&STRONG, power(k.kind, k.held)).to_linear();
        let arc = if matches!(k.kind, Kick::Lob | Kick::ShortLob) { LOB_ARC } else { 0.0 };
        let fade = if matches!(k.kind, Kick::ShortPass | Kick::ShortLob) { SHORT_FADE } else { FADE };
        m.shape = Vec4::new(arc, fade, 0.0, 0.0);
    }
}
