//! The 5v5 arena: pitch, markings, the cage and the goals. Sizes come from the game's levels
//! (arena.md); the look is our own and only meant to read like a Rematch stadium.

use std::f32::consts::FRAC_PI_2;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::tuning::*;

const LINE_WIDTH: f32 = 0.12;
const LINE_Y: f32 = 0.004;
// Visual only (TUNE): the physics walls go up to the ceiling.
const CENTRE_CIRCLE: f32 = 9.15;
const STRIPES: usize = 14;
const BOARD_HEIGHT: f32 = 1.0;
const GLASS_HEIGHT: f32 = 7.0;
const FRAME_SPACING: f32 = 6.0;
const NET_CELL: f32 = 0.12;

pub fn spawn_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (w, l) = (HALF_WIDTH * 2.0, HALF_LENGTH * 2.0);
    let flat = |m: Color| StandardMaterial { base_color: m, perceptual_roughness: 0.95, ..default() };

    // Turf in mown bands across the pitch.
    let turf = [materials.add(flat(Color::srgb(0.20, 0.50, 0.22))), materials.add(flat(Color::srgb(0.17, 0.44, 0.19)))];
    let band = l / STRIPES as f32;
    let band_mesh = meshes.add(Plane3d::default().mesh().size(w, band));
    for i in 0..STRIPES {
        let z = -HALF_LENGTH + band * (i as f32 + 0.5);
        commands.spawn((Mesh3d(band_mesh.clone()), MeshMaterial3d(turf[i % 2].clone()), Transform::from_xyz(0.0, 0.0, z)));
    }

    // Turf inside the goals and a concourse around the cage.
    for end in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(GOAL_HALF_WIDTH * 2.0, GOAL_DEPTH))),
            MeshMaterial3d(turf[1].clone()),
            Transform::from_xyz(0.0, 0.0, end * (HALF_LENGTH + GOAL_DEPTH / 2.0)),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(w + 60.0, l + 60.0))),
        MeshMaterial3d(materials.add(flat(Color::srgb(0.24, 0.25, 0.28)))),
        Transform::from_xyz(0.0, -0.02, 0.0),
    ));

    // Markings: halfway line, centre circle and spot, goal lines and the goalkeeper areas.
    let paint = materials.add(StandardMaterial { base_color: Color::srgb(0.92, 0.95, 0.92), unlit: true, ..default() });
    let mut line = |commands: &mut Commands, size: Vec2, at: Vec2| {
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(size.x, size.y))),
            MeshMaterial3d(paint.clone()),
            Transform::from_xyz(at.x, LINE_Y, at.y),
        ));
    };
    line(&mut commands, Vec2::new(w, LINE_WIDTH), Vec2::ZERO);
    let (area_w, area_d) = KEEPER_AREA;
    for end in [-1.0f32, 1.0] {
        let goal_line = end * (HALF_LENGTH - LINE_WIDTH / 2.0);
        line(&mut commands, Vec2::new(GOAL_HALF_WIDTH * 2.0, LINE_WIDTH), Vec2::new(0.0, goal_line));
        line(&mut commands, Vec2::new(area_w, LINE_WIDTH), Vec2::new(0.0, end * (HALF_LENGTH - area_d)));
        for side in [-1.0f32, 1.0] {
            line(&mut commands, Vec2::new(LINE_WIDTH, area_d), Vec2::new(side * area_w / 2.0, end * (HALF_LENGTH - area_d / 2.0)));
        }
    }
    let flat_on_floor = Transform::from_xyz(0.0, LINE_Y, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2));
    let ring = Annulus::new(CENTRE_CIRCLE - LINE_WIDTH / 2.0, CENTRE_CIRCLE + LINE_WIDTH / 2.0);
    commands.spawn((Mesh3d(meshes.add(ring.mesh().resolution(160))), MeshMaterial3d(paint.clone()), flat_on_floor));
    commands.spawn((Mesh3d(meshes.add(Circle::new(0.2))), MeshMaterial3d(paint.clone()), flat_on_floor));

    // The cage: dark boards at the bottom, glass above, a frame post every few metres.
    let board = materials.add(flat(Color::srgb(0.10, 0.11, 0.14)));
    let trim = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.9, 1.0), emissive: LinearRgba::rgb(0.3, 1.6, 2.0), ..default() });
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.7, 0.85, 1.0, 0.08),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        ..default()
    });
    let frame = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.58, 0.62), metallic: 0.8, ..default() });
    // A wall segment from `a` to `b` along the floor, between heights `y0` and `y1`.
    let mut wall = |commands: &mut Commands, a: Vec2, b: Vec2, y0: f32, y1: f32, material: &Handle<StandardMaterial>, thick: f32| {
        let mid = (a + b) / 2.0;
        let len = a.distance(b);
        let yaw = (b - a).y.atan2((b - a).x);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(len, y1 - y0, thick))),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(mid.x, (y0 + y1) / 2.0, mid.y).with_rotation(Quat::from_rotation_y(-yaw)),
        ));
    };
    let (hw, hl) = (HALF_WIDTH, HALF_LENGTH);
    for s in [-1.0f32, 1.0] {
        // Side walls run the full length; the end walls leave the goal mouth open below the crossbar.
        let side = [(Vec2::new(s * hw, -hl), Vec2::new(s * hw, hl))];
        let end_beside_goal = [(Vec2::new(-hw, s * hl), Vec2::new(-GOAL_HALF_WIDTH, s * hl)), (Vec2::new(GOAL_HALF_WIDTH, s * hl), Vec2::new(hw, s * hl))];
        for (a, b) in side.into_iter().chain(end_beside_goal) {
            wall(&mut commands, a, b, 0.0, BOARD_HEIGHT, &board, 0.2);
            wall(&mut commands, a, b, BOARD_HEIGHT, BOARD_HEIGHT + 0.06, &trim, 0.22);
            wall(&mut commands, a, b, BOARD_HEIGHT + 0.06, GLASS_HEIGHT, &glass, 0.05);
            wall(&mut commands, a, b, GLASS_HEIGHT, GLASS_HEIGHT + 0.12, &frame, 0.12);
        }
        let above_goal = (Vec2::new(-GOAL_HALF_WIDTH, s * hl), Vec2::new(GOAL_HALF_WIDTH, s * hl));
        wall(&mut commands, above_goal.0, above_goal.1, GOAL_HEIGHT, GLASS_HEIGHT, &glass, 0.05);
        wall(&mut commands, above_goal.0, above_goal.1, GLASS_HEIGHT, GLASS_HEIGHT + 0.12, &frame, 0.12);
    }
    let post_mesh = meshes.add(Cuboid::new(0.12, GLASS_HEIGHT, 0.12));
    let frame_post = |commands: &mut Commands, at: Vec2| {
        commands.spawn((Mesh3d(post_mesh.clone()), MeshMaterial3d(frame.clone()), Transform::from_xyz(at.x, GLASS_HEIGHT / 2.0, at.y)));
    };
    for s in [-1.0f32, 1.0] {
        let along = (l / FRAME_SPACING).round() as i32;
        for i in 0..=along {
            frame_post(&mut commands, Vec2::new(s * hw, -hl + l * i as f32 / along as f32));
        }
        let across = (w / FRAME_SPACING).round() as i32;
        for i in 1..across {
            let x = -hw + w * i as f32 / across as f32;
            if x.abs() > GOAL_HALF_WIDTH + 0.5 {
                frame_post(&mut commands, Vec2::new(x, s * hl));
            }
        }
    }
    // Goals: white frame, a net shaped like the goal's collision panels (flat roof, then a
    // 45 degree slope down to the back net).
    let white = materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.95, 0.95), metallic: 0.3, perceptual_roughness: 0.4, ..default() });
    let net = materials.add(StandardMaterial {
        base_color_texture: Some(net_texture(&mut images)),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        unlit: true,
        ..default()
    });
    let support = materials.add(flat(Color::srgb(0.25, 0.26, 0.3)));
    let back_top = NET_HEIGHT - (GOAL_DEPTH - NET_ROOF_FLAT);
    for end in [-1.0f32, 1.0] {
        let z = end * hl;
        // Goal-local point: x across, d metres behind the line, y up.
        let at = |x: f32, d: f32, y: f32| Vec3::new(x, y, z + end * d);
        let gx = GOAL_HALF_WIDTH;
        for side in [-1.0f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(POST_RADIUS, GOAL_HEIGHT))),
                MeshMaterial3d(white.clone()),
                Transform::from_translation(at(side * gx, 0.0, GOAL_HEIGHT / 2.0)),
            ));
            let x = side * gx;
            commands.spawn((Mesh3d(meshes.add(panel([at(x, 0.0, 0.0), at(x, 0.0, NET_HEIGHT), at(x, NET_ROOF_FLAT, NET_HEIGHT), at(x, NET_ROOF_FLAT, 0.0)]))), MeshMaterial3d(net.clone())));
            commands.spawn((
                Mesh3d(meshes.add(panel([at(x, NET_ROOF_FLAT, 0.0), at(x, NET_ROOF_FLAT, NET_HEIGHT), at(x, GOAL_DEPTH, back_top), at(x, GOAL_DEPTH, 0.0)]))),
                MeshMaterial3d(net.clone()),
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(POST_RADIUS, gx * 2.0 + POST_RADIUS * 2.0))),
            MeshMaterial3d(white.clone()),
            Transform::from_translation(at(0.0, 0.0, GOAL_HEIGHT)).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        ));
        commands.spawn((Mesh3d(meshes.add(panel([at(-gx, 0.0, NET_HEIGHT), at(gx, 0.0, NET_HEIGHT), at(gx, NET_ROOF_FLAT, NET_HEIGHT), at(-gx, NET_ROOF_FLAT, NET_HEIGHT)]))), MeshMaterial3d(net.clone())));
        commands.spawn((
            Mesh3d(meshes.add(panel([at(-gx, NET_ROOF_FLAT, NET_HEIGHT), at(gx, NET_ROOF_FLAT, NET_HEIGHT), at(gx, GOAL_DEPTH, back_top), at(-gx, GOAL_DEPTH, back_top)]))),
            MeshMaterial3d(net.clone()),
        ));
        commands.spawn((Mesh3d(meshes.add(panel([at(-gx, GOAL_DEPTH, 0.0), at(gx, GOAL_DEPTH, 0.0), at(gx, GOAL_DEPTH, back_top), at(-gx, GOAL_DEPTH, back_top)]))), MeshMaterial3d(net.clone())));
        // Support bars along the net's bottom edges.
        let bar = |commands: &mut Commands, meshes: &mut Assets<Mesh>, a: Vec3, b: Vec3| {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.05, a.distance(b)))),
                MeshMaterial3d(support.clone()),
                Transform::from_translation((a + b) / 2.0).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize())),
            ));
        };
        bar(&mut commands, &mut meshes, at(-gx, GOAL_DEPTH, 0.05), at(gx, GOAL_DEPTH, 0.05));
        for side in [-1.0f32, 1.0] {
            bar(&mut commands, &mut meshes, at(side * gx, 0.0, 0.05), at(side * gx, GOAL_DEPTH, 0.05));
        }
    }

    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(15.0, 40.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight { brightness: 500.0, ..default() });
}

/// A flat two-sided quad through four corners in order, textured in metres (one net cell = NET_CELL).
fn panel(corners: [Vec3; 4]) -> Mesh {
    let [a, b, _, d] = corners;
    let u_axis = (b - a).normalize();
    let normal = u_axis.cross(d - a).normalize();
    let v_axis = normal.cross(u_axis);
    let uv = corners.map(|p| [(p - a).dot(u_axis) / NET_CELL, (p - a).dot(v_axis) / NET_CELL]);
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners.map(|p| p.to_array()).to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![normal.to_array(); 4])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv.to_vec())
        .with_inserted_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]))
}

/// One net cell: thin white strings on two edges, transparent inside; tiles across the panels.
/// Mipmapped by averaging, so the net fades to a light haze in the distance instead of shimmering.
fn net_texture(images: &mut Assets<Image>) -> Handle<Image> {
    const SIZE: usize = 16;
    let mut level: Vec<[f32; 4]> = (0..SIZE * SIZE)
        .map(|i| if i % SIZE < 2 || i / SIZE < 2 { [0.92, 0.93, 0.94, 0.9] } else { [0.0; 4] })
        .collect();
    let mut data = Vec::new();
    let (mut size, mut mips) = (SIZE, 0);
    loop {
        // Premultiplied average keeps the strings' colour as they thin out.
        data.extend(level.iter().flat_map(|c| {
            let a = c[3].max(1e-4);
            [c[0] / a, c[1] / a, c[2] / a, c[3]].map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8)
        }));
        mips += 1;
        if size == 1 {
            break;
        }
        let half = size / 2;
        level = (0..half * half)
            .map(|i| {
                let (x, y) = (i % half * 2, i / half * 2);
                let px = [level[y * size + x], level[y * size + x + 1], level[(y + 1) * size + x], level[(y + 1) * size + x + 1]];
                std::array::from_fn(|c| px.iter().map(|p| if c < 3 { p[c] * p[3] } else { p[3] }).sum::<f32>() / 4.0)
            })
            .collect();
        size = half;
    }
    let mut image = Image::new(
        Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data[..SIZE * SIZE * 4].to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = mips;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    images.add(image)
}
