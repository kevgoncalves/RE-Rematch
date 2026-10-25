//! The 5v5 arena: pitch, markings, the cage, the goals, the stadium round them and its light.
//! Sizes come from the game's levels (arena.md); the look is our own and only meant to read like
//! a Rematch stadium. Everything is made here: meshes, procedural textures and two shaders
//! (pitch.wgsl, net.wgsl). Visual only: the collisions use the constants in tuning.rs.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, ExtendedMaterial, MaterialExtension, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderRef, TextureDimension, TextureFormat};

use crate::tuning::*;

// Markings (visual only; TUNE where arena.md has no number).
const LINE_WIDTH: f32 = 0.15;
const CENTRE_CIRCLE: f32 = 9.15;
const SPOT_RADIUS: f32 = 0.18;
/// The goal area: 5.5 m past each post and 6 m deep, a real one scaled like Rematch's goal. TUNE
const GOAL_AREA: (f32, f32) = (GOAL_HALF_WIDTH + 5.5, 6.0);
const PENALTY_SPOT: f32 = 11.5;
const PENALTY_ARC: f32 = 9.15;
/// Mowing: concentric rings round the centre spot, each this wide (measured off Rematch frames:
/// a light and a dark ring every metre or so).
/// The mowing period: a light and a dark stripe (Rematch's are about half a metre each).
const STRIPE: f32 = 1.1;
// Turf and paint albedo (sRGB): Rematch's bright, slightly yellow green.
const TURF_LIGHT: Color = Color::srgb(0.42, 0.58, 0.24);
const TURF_DARK: Color = Color::srgb(0.3, 0.48, 0.25);
const PAINT: Color = Color::srgb(0.93, 0.95, 0.93);

// The cage round the pitch.
const KICK_PLATE: f32 = 0.13;
const BOARD_HEIGHT: f32 = 1.0;
const BOARD_PANEL: f32 = 8.0;
const GLASS_HEIGHT: f32 = 7.0;
const FRAME_SPACING: f32 = 6.0;
const NET_CELL: f32 = 0.16;

// The stadium: distances are behind the walls' inner faces, heights from the pitch.
const SEAT: f32 = 0.55;
const AISLE_EVERY: f32 = 12.0;
const AISLE: f32 = 1.3;
/// (start behind the wall, floor height, rows, row depth, row rise)
const LOWER_TIER: (f32, f32, usize, f32, f32) = (4.0, 1.6, 16, 0.85, 0.45);
const UPPER_TIER: (f32, f32, usize, f32, f32) = (19.6, 12.2, 20, 0.8, 0.55);
const ROOF: (f32, f32) = (12.0, 31.0);
const FLOODLIGHT_SPACING: f32 = 9.0;

/// Camera exposure (EV100). The lights are set against it.
pub const EXPOSURE_EV100: f32 = 9.7;
const KEY_LUX: f32 = 5200.0;
const FILL_LUX: f32 = 1200.0;
const BOUNCE_LUX: f32 = 700.0;
/// The stadium's ambient light, as display brightness on a white surface.
const AMBIENT: f32 = 0.2;

pub type PitchMaterial = ExtendedMaterial<StandardMaterial, PitchExt>;
pub type NetMaterial = ExtendedMaterial<StandardMaterial, NetExt>;

/// The pitch shader's inputs (pitch.wgsl).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct PitchExt {
    /// Half width and length, line half width, mowing ring width.
    #[uniform(100)]
    size: Vec4,
    /// Keeper area half width and depth, centre circle and spot radii.
    #[uniform(101)]
    keeper: Vec4,
    /// Goal area half width and depth, penalty spot distance and arc radius.
    #[uniform(102)]
    boxes: Vec4,
    /// Turf albedo of the light and dark stripes, and the paint's (linear).
    #[uniform(103)]
    light: Vec4,
    #[uniform(104)]
    dark: Vec4,
    #[uniform(105)]
    paint: Vec4,
}

impl MaterialExtension for PitchExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/pitch.wgsl".into()
    }
}

/// The net shader's inputs (net.wgsl): x cell size, y string width, z string and w fill opacity.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct NetExt {
    #[uniform(100)]
    cell: Vec4,
}

impl MaterialExtension for NetExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/net.wgsl".into()
    }
}

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MaterialPlugin::<PitchMaterial>::default(), MaterialPlugin::<NetMaterial>::default()))
            .insert_resource(DirectionalLightShadowMap { size: 4096 })
            // A soft violet ambient from the stands and the night sky. (An image-based environment
            // light looked a touch richer, but with it Bevy's async pipeline compilation crashed
            // the AMD Vulkan driver at startup about one run in eight.)
            .insert_resource(AmbientLight { color: Color::srgb(0.78, 0.76, 1.0), brightness: nits(AMBIENT), ..default() })
            .insert_resource(ClearColor(Color::linear_rgb(0.05, 0.05, 0.14)));
    }
}

pub fn spawn_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pitch_materials: ResMut<Assets<PitchMaterial>>,
    mut net_materials: ResMut<Assets<NetMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    spawn_pitch(&mut commands, &mut meshes, &mut pitch_materials);
    spawn_cage(&mut commands, &mut meshes, &mut materials, &mut images);
    spawn_goals(&mut commands, &mut meshes, &mut materials, &mut net_materials);
    spawn_stadium(&mut commands, &mut meshes, &mut materials, &mut images);

    // Key: a high floodlight from one side, with shadows over the whole pitch. Fill: a weaker,
    // cooler light from the other side so the players' shaded side still reads. Bounce: the lit
    // turf throwing a little green back up under chins and arms.
    commands.spawn((
        DirectionalLight { illuminance: KEY_LUX, color: Color::srgb(1.0, 0.96, 0.9), shadows_enabled: true, shadow_depth_bias: 0.03, shadow_normal_bias: 1.4, ..default() },
        CascadeShadowConfigBuilder { num_cascades: 4, minimum_distance: 0.1, first_cascade_far_bound: 9.0, maximum_distance: 90.0, overlap_proportion: 0.25 }.build(),
        Transform::from_xyz(-30.0, 48.0, 22.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight { illuminance: FILL_LUX, color: Color::srgb(0.78, 0.82, 1.0), shadows_enabled: false, ..default() },
        Transform::from_xyz(28.0, 18.0, -30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight { illuminance: BOUNCE_LUX, color: Color::srgb(0.75, 0.95, 0.55), shadows_enabled: false, ..default() },
        Transform::from_xyz(5.0, -10.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// The turf and its markings, one shader over the pitch and the goals' floors.
fn spawn_pitch(commands: &mut Commands, meshes: &mut Assets<Mesh>, pitch_materials: &mut Assets<PitchMaterial>) {
    let lin = |c: Color| Vec4::from_array(c.to_linear().to_f32_array());
    let turf = pitch_materials.add(PitchMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.8, ..default() },
        extension: PitchExt {
            size: Vec4::new(HALF_WIDTH, HALF_LENGTH, LINE_WIDTH / 2.0, STRIPE),
            keeper: Vec4::new(KEEPER_AREA.0 / 2.0, KEEPER_AREA.1, CENTRE_CIRCLE, SPOT_RADIUS),
            boxes: Vec4::new(GOAL_AREA.0, GOAL_AREA.1, PENALTY_SPOT, PENALTY_ARC),
            light: lin(TURF_LIGHT),
            dark: lin(TURF_DARK),
            paint: lin(PAINT),
        },
    });
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(HALF_WIDTH * 2.0, HALF_LENGTH * 2.0))), MeshMaterial3d(turf.clone()), Transform::default()));
    for end in [-1.0f32, 1.0] {
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(GOAL_HALF_WIDTH * 2.0, GOAL_DEPTH))),
            MeshMaterial3d(turf.clone()),
            Transform::from_xyz(0.0, 0.0, end * (HALF_LENGTH + GOAL_DEPTH / 2.0)),
        ));
    }
}

/// The cage: a kick plate and LED boards at the bottom, glass above in a light frame. The inner
/// faces sit exactly on the collision walls.
fn spawn_cage(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>) {
    let (hw, hl) = (HALF_WIDTH, HALF_LENGTH);
    let plate = materials.add(StandardMaterial { base_color: Color::srgb(0.82, 0.84, 0.9), perceptual_roughness: 0.5, ..default() });
    let boards = board_texture(images);
    let board = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.04, 0.06),
        base_color_texture: Some(boards.clone()),
        emissive: glow(Color::WHITE, 0.85),
        emissive_texture: Some(boards),
        perceptual_roughness: 0.35,
        ..default()
    });
    let metal = materials.add(StandardMaterial { base_color: Color::srgb(0.78, 0.8, 0.86), metallic: 0.7, perceptual_roughness: 0.3, ..default() });
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.75, 0.82, 1.0, 0.07),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.04,
        reflectance: 0.6,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    // Each run of wall: from `a` to `b` along the floor, its inner face on that line.
    let ends_beside_goal = |s: f32| [(Vec2::new(-hw, s * hl), Vec2::new(-GOAL_HALF_WIDTH, s * hl)), (Vec2::new(GOAL_HALF_WIDTH, s * hl), Vec2::new(hw, s * hl))];
    let mut runs = Vec::new();
    for s in [-1.0f32, 1.0] {
        runs.push((Vec2::new(s * hw, -hl), Vec2::new(s * hw, hl)));
        runs.extend(ends_beside_goal(s));
    }
    let mut boards = Build::default();
    for &(a, b) in &runs {
        slab(commands, meshes, &plate, (a, b), 0.0, KICK_PLATE, 0.06);
        slab(commands, meshes, &metal, (a, b), BOARD_HEIGHT, BOARD_HEIGHT + 0.07, 0.16);
        slab(commands, meshes, &glass, (a, b), BOARD_HEIGHT + 0.07, GLASS_HEIGHT, 0.04);
        slab(commands, meshes, &metal, (a, b), GLASS_HEIGHT, GLASS_HEIGHT + 0.14, 0.14);
        // The boards' face, textured a panel at a time (four designs in turn).
        let inward = inward_of(a, b);
        let len = a.distance(b);
        let (y0, y1) = (KICK_PLATE, BOARD_HEIGHT);
        let at = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y) + inward * 0.001;
        let u = len / BOARD_PANEL / 4.0;
        boards.quad([at(a, y0), at(b, y0), at(b, y1), at(a, y1)], [Vec2::new(0.0, 1.0), Vec2::new(u, 1.0), Vec2::new(u, 0.0), Vec2::ZERO], Color::WHITE, inward);
        slab(commands, meshes, &plate, (a, b), y0, y1, 0.12);
    }
    commands.spawn((Mesh3d(meshes.add(boards.mesh())), MeshMaterial3d(board), NotShadowCaster));
    // Above the goals: glass from the crossbar up, on a beam.
    for s in [-1.0f32, 1.0] {
        let (a, b) = (Vec2::new(-GOAL_HALF_WIDTH, s * hl), Vec2::new(GOAL_HALF_WIDTH, s * hl));
        slab(commands, meshes, &metal, (a, b), GOAL_HEIGHT, GOAL_HEIGHT + 0.18, 0.2);
        slab(commands, meshes, &glass, (a, b), GOAL_HEIGHT + 0.18, GLASS_HEIGHT, 0.04);
        slab(commands, meshes, &metal, (a, b), GLASS_HEIGHT, GLASS_HEIGHT + 0.14, 0.14);
    }
    // Frame posts every few metres and at the corners.
    let post = meshes.add(Cuboid::new(0.1, GLASS_HEIGHT - BOARD_HEIGHT, 0.1));
    let mut frame_post = |at: Vec2| {
        commands.spawn((Mesh3d(post.clone()), MeshMaterial3d(metal.clone()), Transform::from_xyz(at.x, (GLASS_HEIGHT + BOARD_HEIGHT) / 2.0, at.y), NotShadowCaster));
    };
    let (w, l) = (hw * 2.0, hl * 2.0);
    for s in [-1.0f32, 1.0] {
        let along = (l / FRAME_SPACING).round() as i32;
        for i in 0..=along {
            frame_post(Vec2::new(s * (hw + 0.05), -hl + l * i as f32 / along as f32));
        }
        let across = (w / FRAME_SPACING).round() as i32;
        for i in 1..across {
            let x = -hw + w * i as f32 / across as f32;
            if x.abs() > GOAL_HALF_WIDTH + 0.5 {
                frame_post(Vec2::new(x, s * (hl + 0.05)));
            }
        }
        frame_post(Vec2::new(-GOAL_HALF_WIDTH - 0.05, s * (hl + 0.05)));
        frame_post(Vec2::new(GOAL_HALF_WIDTH + 0.05, s * (hl + 0.05)));
    }
}

/// A slab of wall from `a` to `b` between heights `y0` and `y1`, `thick` deep behind the line.
/// Only the boards shade the pitch: the glass and its frame above cast no shadow, so the turf
/// stays clean along the walls.
fn slab(commands: &mut Commands, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, (a, b): (Vec2, Vec2), y0: f32, y1: f32, thick: f32) {
    let mid = (a + b) / 2.0 - inward_of(a, b).xz() * thick / 2.0;
    let yaw = (b - a).y.atan2((b - a).x);
    let mut slab = commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(a.distance(b), y1 - y0, thick))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(mid.x, (y0 + y1) / 2.0, mid.y).with_rotation(Quat::from_rotation_y(-yaw)),
    ));
    if y0 >= BOARD_HEIGHT {
        slab.insert(NotShadowCaster);
    }
}

/// The horizontal direction from a wall line back towards the centre spot.
fn inward_of(a: Vec2, b: Vec2) -> Vec3 {
    let mid = (a + b) / 2.0;
    let d = (b - a).normalize();
    let n = Vec2::new(-d.y, d.x);
    let n = if n.dot(-mid) < 0.0 { -n } else { n };
    Vec3::new(n.x, 0.0, n.y)
}

/// Goals: white posts and crossbar, a tinted net shaped like the goal's collision panels (flat
/// roof, then a 45 degree slope down to the back net) with a little sag, a frame behind it and a
/// glowing team-coloured surround on the wall.
fn spawn_goals(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, net_materials: &mut Assets<NetMaterial>) {
    let white = materials.add(StandardMaterial { base_color: Color::srgb(0.96, 0.96, 0.97), perceptual_roughness: 0.3, reflectance: 0.6, ..default() });
    let support = materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.77, 0.82), metallic: 0.6, perceptual_roughness: 0.35, ..default() });
    let backdrop = materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.09), perceptual_roughness: 0.6, ..default() });
    let hl = HALF_LENGTH;
    let gx = GOAL_HALF_WIDTH;
    let back_top = NET_HEIGHT - (GOAL_DEPTH - NET_ROOF_FLAT);
    // The blue team defends the +z goal, the red team the -z one (the scoreboard's colours).
    for (end, tint) in [(-1.0f32, Color::srgb(1.0, 0.5, 0.22)), (1.0, Color::srgb(0.3, 0.85, 1.0))] {
        let net = net_materials.add(NetMaterial {
            base: StandardMaterial {
                base_color: tint,
                emissive: glow(tint, 0.3),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.7,
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            extension: NetExt { cell: Vec4::new(NET_CELL, 0.018, 0.95, 0.28) },
        });
        let strip = materials.add(StandardMaterial { base_color: tint, emissive: glow(tint, 3.0), ..default() });
        let z = end * hl;
        // Goal-local point: x across, d metres behind the line, y up.
        let at = |x: f32, d: f32, y: f32| Vec3::new(x, y, z + end * d);
        let back = Vec3::new(0.0, 0.0, end);
        let mut nets = Build::default();
        for side in [-1.0f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(POST_RADIUS, GOAL_HEIGHT))),
                MeshMaterial3d(white.clone()),
                Transform::from_translation(at(side * gx, 0.0, GOAL_HEIGHT / 2.0)),
            ));
            let x = side * gx;
            let out = Vec3::new(side, 0.0, 0.0);
            nets.net([at(x, 0.0, 0.0), at(x, 0.0, NET_HEIGHT), at(x, NET_ROOF_FLAT, NET_HEIGHT), at(x, NET_ROOF_FLAT, 0.0)], out * 0.08);
            nets.net([at(x, NET_ROOF_FLAT, 0.0), at(x, NET_ROOF_FLAT, NET_HEIGHT), at(x, GOAL_DEPTH, back_top), at(x, GOAL_DEPTH, 0.0)], out * 0.08);
            // The frame behind the net: along the roof's sides and down the back corners.
            bar(commands, meshes, &support, at(x, 0.0, NET_HEIGHT), at(x, NET_ROOF_FLAT, NET_HEIGHT), 0.03);
            bar(commands, meshes, &support, at(x, NET_ROOF_FLAT, NET_HEIGHT), at(x, GOAL_DEPTH, back_top), 0.03);
            bar(commands, meshes, &support, at(x, GOAL_DEPTH, back_top), at(x, GOAL_DEPTH, 0.0), 0.03);
            bar(commands, meshes, &support, at(x, 0.0, 0.04), at(x, GOAL_DEPTH, 0.04), 0.04);
            // Glowing strip down the wall beside the post.
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.07, GOAL_HEIGHT + 0.18, 0.03))),
                MeshMaterial3d(strip.clone()),
                Transform::from_translation(at(side * (gx + 0.2), -0.02, (GOAL_HEIGHT + 0.18) / 2.0)),
                NotShadowCaster,
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(POST_RADIUS, gx * 2.0 + POST_RADIUS * 2.0))),
            MeshMaterial3d(white.clone()),
            Transform::from_translation(at(0.0, 0.0, GOAL_HEIGHT)).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(gx * 2.0 + 0.47, 0.07, 0.03))),
            MeshMaterial3d(strip),
            Transform::from_translation(at(0.0, -0.02, GOAL_HEIGHT + 0.22)),
            NotShadowCaster,
        ));
        nets.net([at(-gx, 0.0, NET_HEIGHT), at(gx, 0.0, NET_HEIGHT), at(gx, NET_ROOF_FLAT, NET_HEIGHT), at(-gx, NET_ROOF_FLAT, NET_HEIGHT)], -Vec3::Y * 0.1);
        nets.net([at(-gx, NET_ROOF_FLAT, NET_HEIGHT), at(gx, NET_ROOF_FLAT, NET_HEIGHT), at(gx, GOAL_DEPTH, back_top), at(-gx, GOAL_DEPTH, back_top)], (back - Vec3::Y) * 0.06);
        nets.net([at(-gx, GOAL_DEPTH, 0.0), at(gx, GOAL_DEPTH, 0.0), at(gx, GOAL_DEPTH, back_top), at(-gx, GOAL_DEPTH, back_top)], back * 0.12);
        commands.spawn((Mesh3d(meshes.add(nets.mesh())), MeshMaterial3d(net), NotShadowCaster));
        bar(commands, meshes, &support, at(-gx, GOAL_DEPTH, back_top), at(gx, GOAL_DEPTH, back_top), 0.03);
        bar(commands, meshes, &support, at(-gx, NET_ROOF_FLAT, NET_HEIGHT), at(gx, NET_ROOF_FLAT, NET_HEIGHT), 0.03);
        bar(commands, meshes, &support, at(-gx, GOAL_DEPTH, 0.04), at(gx, GOAL_DEPTH, 0.04), 0.04);
        // A dark alcove behind the net, so the goal reads against the concourse.
        let alcove = |commands: &mut Commands, meshes: &mut Assets<Mesh>, size: Vec3, centre: Vec3| {
            commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(backdrop.clone()), Transform::from_translation(centre), NotShadowCaster));
        };
        let depth = GOAL_DEPTH + 0.5;
        alcove(commands, meshes, Vec3::new(gx * 2.0 + 1.2, 3.3, 0.1), at(0.0, depth, 1.65));
        for side in [-1.0f32, 1.0] {
            alcove(commands, meshes, Vec3::new(0.1, 3.3, depth), at(side * (gx + 0.6), depth / 2.0, 1.65));
        }
        alcove(commands, meshes, Vec3::new(gx * 2.0 + 1.3, 0.1, depth), at(0.0, depth / 2.0, 3.3));
    }
}

/// A round bar from `a` to `b`.
fn bar(commands: &mut Commands, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, a: Vec3, b: Vec3, radius: f32) {
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(radius, a.distance(b)).mesh().resolution(12))),
        MeshMaterial3d(material.clone()),
        Transform::from_translation((a + b) / 2.0).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize())),
    ));
}

/// The stadium round the cage: a concourse, two tiers of seats with LED ribbons, a band of
/// boxes between them, a roof ring hung with floodlights, and the night sky.
fn spawn_stadium(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>) {
    let seat_tex = seat_texture(images);
    let seats = materials.add(StandardMaterial { base_color_texture: Some(seat_tex), perceptual_roughness: 0.75, ..default() });
    let concrete = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.15, 0.24), perceptual_roughness: 0.9, ..default() });
    let ribbon_tex = ribbon_texture(images);
    let ribbon = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: glow(Color::WHITE, 1.3),
        emissive_texture: Some(ribbon_tex),
        ..default()
    });
    let windows = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.1, 0.16),
        emissive: glow(Color::linear_rgb(1.0, 0.78, 0.62), 0.3),
        emissive_texture: Some(window_texture(images)),
        perceptual_roughness: 0.2,
        ..default()
    });
    let flood = materials.add(StandardMaterial { base_color: Color::WHITE, emissive: glow(Color::linear_rgb(1.0, 0.97, 0.92), 14.0), ..default() });
    let roof_under = materials.add(StandardMaterial { base_color: Color::srgb(0.07, 0.07, 0.12), perceptual_roughness: 0.8, ..default() });

    let mut seat_mesh = Build::default();
    let mut solid = Build::default();
    let mut ribbons = Build::default();
    let mut boxes = Build::default();
    let mut roof = Build::default();

    // The concourse floor between the cage and the stands, a little below the turf: the goals' floors
    // reach out over it and would flicker against it at the same height.
    for side in 0..4 {
        let o0 = 0.0;
        let o1 = LOWER_TIER.0;
        let (a0, b0) = ring_edge(side, o0);
        let (a1, b1) = ring_edge(side, o1);
        let y = -0.03;
        solid.quad([on(a0, y), on(b0, y), on(b1, y), on(a1, y)], [Vec2::ZERO; 4], Color::srgb(0.6, 0.6, 0.7), Vec3::Y);
    }
    // Ribbon strips in ribbon_texture: v 0..0.5 the flags, 0.5..1 the sweep.
    for (tier, ribbon_v) in [(LOWER_TIER, 0.5), (UPPER_TIER, 0.0)] {
        let (start, floor, rows, depth, rise) = tier;
        // Front parapet with an LED ribbon along its top (the upper tier's sits on the boxes).
        for side in 0..4 {
            let (a, b) = ring_edge(side, start);
            let inward = ring_inward(side);
            if floor < 5.0 {
                solid.quad([on(a, 0.0), on(b, 0.0), on(b, floor - 0.7), on(a, floor - 0.7)], [Vec2::ZERO; 4], Color::WHITE, inward);
            }
            let u = a.distance(b) / 24.0;
            let lift = inward * 0.01;
            ribbons.quad(
                [on(a, floor - 0.7) + lift, on(b, floor - 0.7) + lift, on(b, floor + 0.05) + lift, on(a, floor + 0.05) + lift],
                [Vec2::new(0.0, ribbon_v + 0.5), Vec2::new(u, ribbon_v + 0.5), Vec2::new(u, ribbon_v), Vec2::new(0.0, ribbon_v)],
                Color::WHITE,
                inward,
            );
        }
        for row in 0..rows {
            let o0 = start + row as f32 * depth;
            let o1 = o0 + depth;
            let y0 = floor + row as f32 * rise;
            let y1 = y0 + rise;
            for side in 0..4 {
                let inward = ring_inward(side);
                for (s0, s1, aisle) in row_pieces(side, o0) {
                    // The pieces are cut square to the side; the end pieces follow the mitred corner.
                    let back = |s: f32| clamp_to_side(side, o1, s, o0);
                    let colour = if aisle {
                        Color::srgb(0.62, 0.62, 0.72)
                    } else if (s0 / AISLE_EVERY).floor() as i32 % 2 == 0 {
                        Color::srgb(0.38, 0.4, 0.8)
                    } else {
                        Color::srgb(0.46, 0.35, 0.78)
                    };
                    let (u0, u1) = (s0 / SEAT, s1 / SEAT);
                    // Tread, then riser up to the next row.
                    seat_mesh.quad(
                        [on(ring_point(side, o0, s0), y0), on(ring_point(side, o0, s1), y0), on(ring_point(side, o1, back(s1)), y0), on(ring_point(side, o1, back(s0)), y0)],
                        [Vec2::new(u0, 1.0), Vec2::new(u1, 1.0), Vec2::new(u1, 0.5), Vec2::new(u0, 0.5)],
                        colour,
                        Vec3::Y,
                    );
                    seat_mesh.quad(
                        [on(ring_point(side, o1, back(s0)), y0), on(ring_point(side, o1, back(s1)), y0), on(ring_point(side, o1, back(s1)), y1), on(ring_point(side, o1, back(s0)), y1)],
                        [Vec2::new(u0, 0.5), Vec2::new(u1, 0.5), Vec2::new(u1, 0.0), Vec2::new(u0, 0.0)],
                        colour,
                        inward,
                    );
                }
            }
        }
        // Behind the top row: a back wall up to the next level.
        let top_o = start + rows as f32 * depth;
        let top_y = floor + rows as f32 * rise;
        for side in 0..4 {
            let (a, b) = ring_edge(side, top_o);
            let inward = ring_inward(side);
            if floor < 5.0 {
                // Under the upper tier: a band of lit boxes.
                let (c, d) = ring_edge(side, UPPER_TIER.0);
                solid.quad([on(a, top_y), on(b, top_y), on(d, top_y), on(c, top_y)], [Vec2::ZERO; 4], Color::WHITE, Vec3::Y);
                let u = c.distance(d) / 6.0;
                boxes.quad([on(c, top_y), on(d, top_y), on(d, UPPER_TIER.1 - 0.7), on(c, UPPER_TIER.1 - 0.7)], [Vec2::new(0.0, 1.0), Vec2::new(u, 1.0), Vec2::new(u, 0.0), Vec2::ZERO], Color::WHITE, inward);
            } else {
                solid.quad([on(a, top_y), on(b, top_y), on(b, top_y + 4.5), on(a, top_y + 4.5)], [Vec2::ZERO; 4], Color::WHITE, inward);
            }
        }
    }
    // The roof ring: from above the top row, sloping up and in to a fascia hung with floodlights.
    let top_o = UPPER_TIER.0 + UPPER_TIER.2 as f32 * UPPER_TIER.3;
    let top_y = UPPER_TIER.1 + UPPER_TIER.2 as f32 * UPPER_TIER.4 + 4.5;
    let (front_o, front_y) = ROOF;
    let flood_mesh = meshes.add(Cuboid::new(1.6, 0.5, 0.9));
    let truss = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.2, 0.28), metallic: 0.5, perceptual_roughness: 0.5, ..default() });
    for side in 0..4 {
        // Trusses under the roof, back from the fascia.
        let h = ring_half_length(side, front_o);
        let mut s = -h + 6.0;
        while s < h - 3.0 {
            let a = on(ring_point(side, front_o, s), front_y - 0.9);
            let b = on(ring_point(side, top_o, s), top_y - 0.4);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.35, 0.9, a.distance(b)))),
                MeshMaterial3d(truss.clone()),
                Transform::from_translation((a + b) / 2.0).looking_to(b - a, Vec3::Y),
                NotShadowCaster,
            ));
            s += 12.0;
        }
        let (a, b) = ring_edge(side, top_o);
        let (c, d) = ring_edge(side, front_o);
        roof.quad([on(a, top_y), on(b, top_y), on(d, front_y), on(c, front_y)], [Vec2::ZERO; 4], Color::WHITE, -Vec3::Y);
        let inward = ring_inward(side);
        roof.quad([on(c, front_y - 1.4), on(d, front_y - 1.4), on(d, front_y), on(c, front_y)], [Vec2::ZERO; 4], Color::WHITE, inward);
        let u = c.distance(d) / 24.0;
        let lift = inward * 0.02;
        ribbons.quad(
            [on(c, front_y - 1.1) + lift, on(d, front_y - 1.1) + lift, on(d, front_y - 0.5) + lift, on(c, front_y - 0.5) + lift],
            [Vec2::new(0.0, 1.0), Vec2::new(u, 1.0), Vec2::new(u, 0.5), Vec2::new(0.0, 0.5)],
            Color::WHITE,
            inward,
        );
        // Floodlights along the fascia, tilted down at the pitch.
        let len = c.distance(d);
        let count = (len / FLOODLIGHT_SPACING) as i32;
        let dir = (d - c).normalize();
        for i in 0..count {
            let s = (i as f32 + 0.5) / count as f32 * len;
            let p = c + dir * s;
            let at = on(p, front_y - 1.9) + inward * 0.6;
            let tilt = Quat::from_rotation_arc(Vec3::Z, inward) * Quat::from_rotation_x(0.6);
            commands.spawn((Mesh3d(flood_mesh.clone()), MeshMaterial3d(flood.clone()), Transform::from_translation(at).with_rotation(tilt), NotShadowCaster, NotShadowReceiver));
        }
    }
    for (build, material) in [(seat_mesh, seats), (solid, concrete), (ribbons, ribbon), (boxes, windows), (roof, roof_under)] {
        commands.spawn((Mesh3d(meshes.add(build.mesh())), MeshMaterial3d(material), NotShadowCaster));
    }

    // The sky: a night gradient on a dome far beyond the roof.
    let mut dome = Sphere::new(900.0).mesh().uv(96, 48);
    let colours: Vec<[f32; 4]> = match dome.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) => p.iter().map(|&v| night_sky(Vec3::from_array(v).normalize()).extend(1.0).to_array()).collect(),
        _ => Vec::new(),
    };
    dome.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    commands.spawn((
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, fog_enabled: false, cull_mode: None, ..default() })),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Display brightness to luminance at the camera's exposure.
fn nits(display: f32) -> f32 {
    display * 1.2 * EXPOSURE_EV100.exp2()
}

/// An emissive colour, `display` bright on screen (emissive ignores the exposure by default).
fn glow(colour: Color, display: f32) -> LinearRgba {
    let (c, n) = (colour.to_linear(), display);
    LinearRgba::rgb(c.red * n, c.green * n, c.blue * n)
}

fn on(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.y)
}

// The stands are a ring of four sides with mitred corners. Side 0 is the -z end, then +x, +z,
// -x; `o` is the distance behind the walls and `s` the distance along the side from its middle.
fn ring_inward(side: usize) -> Vec3 {
    [Vec3::Z, -Vec3::X, -Vec3::Z, Vec3::X][side]
}

fn ring_half_length(side: usize, o: f32) -> f32 {
    if side.is_multiple_of(2) { HALF_WIDTH + o } else { HALF_LENGTH + o }
}

fn ring_point(side: usize, o: f32, s: f32) -> Vec2 {
    let n = ring_inward(side).xz();
    let along = Vec2::new(-n.y, n.x);
    let half_depth = if side.is_multiple_of(2) { HALF_LENGTH } else { HALF_WIDTH };
    -n * (half_depth + o) + along * s
}

fn ring_edge(side: usize, o: f32) -> (Vec2, Vec2) {
    let h = ring_half_length(side, o);
    (ring_point(side, o, -h), ring_point(side, o, h))
}

/// Where a cut at `s` on the line `from` lands on the line `o`: the same `s`, unless it is a
/// corner, which follows the mitre.
fn clamp_to_side(side: usize, o: f32, s: f32, from: f32) -> f32 {
    let h = ring_half_length(side, from);
    if (s.abs() - h).abs() < 1e-3 { s.signum() * ring_half_length(side, o) } else { s }
}

/// One row of one side cut into seat blocks and aisles: (from, to, is an aisle).
fn row_pieces(side: usize, o: f32) -> Vec<(f32, f32, bool)> {
    let h = ring_half_length(side, o);
    let mut cuts = vec![(-h, false)];
    let k = (h / AISLE_EVERY).floor() as i32;
    for i in -k..=k {
        let c = i as f32 * AISLE_EVERY + AISLE_EVERY / 2.0;
        if c - AISLE / 2.0 > -h + 2.0 && c + AISLE / 2.0 < h - 2.0 {
            cuts.push((c - AISLE / 2.0, true));
            cuts.push((c + AISLE / 2.0, false));
        }
    }
    cuts.push((h, false));
    cuts.windows(2).map(|w| (w[0].0, w[1].0, w[0].1)).collect()
}

/// A mesh built from quads.
#[derive(Default)]
struct Build {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Build {
    /// A flat quad through four corners in order, facing `facing` (its winding is fixed to suit).
    fn quad(&mut self, q: [Vec3; 4], uv: [Vec2; 4], colour: Color, facing: Vec3) {
        let n = (q[1] - q[0]).cross(q[3] - q[0]).normalize_or_zero();
        let flip = n.dot(facing) < 0.0;
        let n = if flip { -n } else { n };
        let base = self.pos.len() as u32;
        let c = colour.to_linear().to_f32_array();
        for i in 0..4 {
            self.pos.push(q[i].to_array());
            self.nor.push(n.to_array());
            self.uv.push(uv[i].to_array());
            self.col.push(c);
        }
        if flip {
            self.idx.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            self.idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    /// A net panel through four corners, subdivided and bowed by `sag` in the middle; UVs in metres.
    fn net(&mut self, corners: [Vec3; 4], sag: Vec3) {
        const N: usize = 10;
        let [a, b, c, d] = corners;
        let u_axis = (b - a).normalize();
        let normal = u_axis.cross(d - a).normalize();
        let v_axis = normal.cross(u_axis);
        let base = self.pos.len() as u32;
        for j in 0..=N {
            for i in 0..=N {
                let (u, v) = (i as f32 / N as f32, j as f32 / N as f32);
                let flat = a.lerp(b, u).lerp(d.lerp(c, u), v);
                let p = flat + sag * (PI * u).sin() * (PI * v).sin();
                self.pos.push(p.to_array());
                self.nor.push(normal.to_array());
                self.uv.push([(flat - a).dot(u_axis), (flat - a).dot(v_axis)]);
                self.col.push([1.0; 4]);
            }
        }
        let row = (N + 1) as u32;
        for j in 0..N as u32 {
            for i in 0..N as u32 {
                let k = base + j * row + i;
                self.idx.extend([k, k + 1, k + row + 1, k, k + row + 1, k + row]);
            }
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// A tiling texture from `f(x, y)` (sRGB, 0..1), mipmapped by averaging so distant detail fades
/// to its mean instead of shimmering.
fn texture(images: &mut Assets<Image>, w: usize, h: usize, f: impl Fn(f32, f32) -> [f32; 4]) -> Handle<Image> {
    let mut level: Vec<[f32; 4]> = (0..w * h).map(|i| f((i % w) as f32 + 0.5, (i / w) as f32 + 0.5)).collect();
    let (mut lw, mut lh) = (w, h);
    let mut data = Vec::new();
    let mut mips = 0;
    loop {
        data.extend(level.iter().flat_map(|c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)));
        mips += 1;
        if lw == 1 && lh == 1 {
            break;
        }
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        level = (0..nw * nh)
            .map(|i| {
                let (x, y) = (i % nw, i / nw);
                let mut sum = [0.0; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = level[(y * 2 + dy).min(lh - 1) * lw + (x * 2 + dx).min(lw - 1)];
                    for c in 0..4 {
                        sum[c] += p[c] / 4.0;
                    }
                }
                sum
            })
            .collect();
        (lw, lh) = (nw, nh);
    }
    let mut image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data[..w * h * 4].to_vec(),
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

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The LED boards: four panel designs side by side, each one panel long. Abstract graphics in
/// the arena's palette (no lettering).
fn board_texture(images: &mut Assets<Image>) -> Handle<Image> {
    const W: usize = 256;
    const H: usize = 32;
    texture(images, W * 4, H, |x, y| {
        let design = (x / W as f32) as usize;
        let (u, v) = ((x % W as f32) / W as f32, y / H as f32);
        // Slanted coordinate for italic shapes.
        let sl = u * 8.0 - v * 0.35;
        let c = match design {
            0 => {
                // Night blue with a magenta-to-cyan sweep and chevrons.
                let base = mix3([0.05, 0.06, 0.2], [0.12, 0.08, 0.32], u);
                let sweep = smooth(0.35, 0.0, (v - 0.5 - (u - 0.5) * 0.6).abs()) * 0.8;
                let col = mix3(base, mix3([0.95, 0.25, 0.7], [0.2, 0.85, 1.0], u), sweep);
                let chevron = ((sl * 2.0 + (v - 0.5).abs() * 1.6).fract() - 0.5).abs();
                if chevron < 0.08 && u > 0.6 { [0.95, 0.97, 1.0] } else { col }
            }
            1 => {
                // Pale panel with bold dark slanted blocks.
                let block = (sl * 1.1).fract();
                let inner = v > 0.22 && v < 0.78 && u > 0.12 && u < 0.88;
                if inner && block < 0.55 { [0.14, 0.12, 0.38] } else { mix3([0.88, 0.88, 0.97], [0.75, 0.78, 0.95], v) }
            }
            2 => {
                // Warm gradient with dark streaks.
                let col = mix3([1.0, 0.45, 0.2], [0.9, 0.2, 0.55], u);
                let streak = ((sl * 3.0).fract() < 0.18) as i32 as f32;
                mix3(col, [0.25, 0.08, 0.2], streak * 0.75)
            }
            _ => {
                // Dark with a cyan line and a row of diamonds.
                let col = [0.04, 0.05, 0.14];
                let line = smooth(0.06, 0.0, (v - 0.5).abs());
                let cell = (u * 12.0).fract() - 0.5;
                let diamond = cell.abs() + (v - 0.5).abs() * 0.45 < 0.22;
                let col = mix3(col, [0.25, 0.9, 1.0], line);
                if diamond { [0.95, 0.75, 0.25] } else { col }
            }
        };
        // Dark seams between the panels.
        let seam = if !(0.006..=0.994).contains(&u) { 0.1 } else { 1.0 };
        [c[0] * seam, c[1] * seam, c[2] * seam, 1.0]
    })
}

/// One seat, tiled along the rows: the back on the riser (v 0..0.5), the seat pan on the tread
/// (v 0.5..1). Grey, tinted per block by the vertex colour.
fn seat_texture(images: &mut Assets<Image>) -> Handle<Image> {
    texture(images, 32, 64, |x, y| {
        let (u, v) = (x / 32.0, y / 64.0);
        let concrete = [0.22, 0.22, 0.25];
        let seat = [0.62, 0.62, 0.66];
        let shade = |c: [f32; 3], k: f32| [c[0] * k, c[1] * k, c[2] * k];
        let inside = (0.1..0.9).contains(&u);
        let c = if v < 0.5 {
            // The seat back fills the upper riser; a shadowed gap below it.
            let back = inside && (0.08..0.38).contains(&v);
            if back { shade(seat, 1.0 - (v - 0.08) * 0.6) } else { shade(concrete, 0.7) }
        } else {
            // The pan near the back of the tread, then the walkway.
            let pan = inside && (0.5..0.68).contains(&v);
            if pan { shade(seat, 0.8) } else { concrete }
        };
        [c[0], c[1], c[2], 1.0]
    })
}

/// LED ribbons: two strips (v 0..0.5 and 0.5..1): bands of colour blocks like a ring of flags,
/// and a cyan-magenta sweep. Abstract, our own.
fn ribbon_texture(images: &mut Assets<Image>) -> Handle<Image> {
    const W: usize = 512;
    texture(images, W, 32, |x, y| {
        let (u, v) = (x / W as f32, y / 32.0);
        let c = if v < 0.5 {
            let v = v * 2.0;
            let cell = (u * 24.0) as usize;
            let f = (u * 24.0).fract();
            let palette = [[0.9, 0.2, 0.3], [0.95, 0.95, 1.0], [0.2, 0.5, 1.0], [1.0, 0.75, 0.2], [0.2, 0.8, 0.5], [0.6, 0.3, 0.95]];
            let (a, b) = (palette[cell % 6], palette[(cell * 7 + 3) % 6]);
            let flag = if (cell.is_multiple_of(3) && v < 0.5) || (cell % 3 == 1 && f < 0.5) || (cell % 3 == 2 && (v - 0.5).abs() < 0.17) { a } else { b };
            if !(0.12..=0.88).contains(&f) || !(0.15..=0.85).contains(&v) { [0.03, 0.03, 0.08] } else { flag }
        } else {
            let v = v * 2.0 - 1.0;
            let sweep = mix3([0.95, 0.2, 0.75], [0.15, 0.85, 1.0], (u * TAU * 2.0).sin() * 0.5 + 0.5);
            let core = smooth(0.4, 0.05, (v - 0.5).abs());
            mix3([0.02, 0.02, 0.06], sweep, core)
        };
        [c[0], c[1], c[2], 1.0]
    })
}

/// The boxes between the tiers: lit windows with mullions.
fn window_texture(images: &mut Assets<Image>) -> Handle<Image> {
    texture(images, 96, 32, |x, y| {
        let (u, v) = (x / 96.0, y / 32.0);
        // Three panes a box, lit warm at the top fading down, with a dark sill and mullions.
        let f = (u * 3.0).fract();
        let pane = (0.05..0.95).contains(&f) && (0.2..0.78).contains(&v);
        let k = (0.35 + 0.65 * (1.0 - (v - 0.2) / 0.58)) * if (u * 3.0) as i32 == 1 { 0.75 } else { 1.0 };
        if pane { [k, k * 0.85, k * 0.9, 1.0] } else if v > 0.86 { [0.05, 0.05, 0.08, 1.0] } else { [0.0, 0.0, 0.0, 1.0] }
    })
}

/// The sky seen through the roof's opening: a night gradient, a little lighter at the horizon
/// where the stadium's light hangs in the haze.
fn night_sky(dir: Vec3) -> Vec3 {
    let zenith = Vec3::new(0.01, 0.012, 0.04);
    let horizon = Vec3::new(0.05, 0.04, 0.11);
    zenith.lerp(horizon, smooth(0.8, 0.1, dir.y)) + Vec3::new(0.08, 0.05, 0.12) * smooth(0.45, 0.15, dir.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stand_rows_cover_each_side() {
        for side in 0..4 {
            let pieces = row_pieces(side, 5.0);
            let h = ring_half_length(side, 5.0);
            assert_eq!(pieces.first().unwrap().0, -h);
            assert_eq!(pieces.last().unwrap().1, h);
            assert!(pieces.windows(2).all(|w| w[0].1 == w[1].0));
            assert!(pieces.iter().all(|p| p.1 > p.0));
        }
    }

    #[test]
    fn walls_face_the_pitch() {
        let n = inward_of(Vec2::new(HALF_WIDTH, -HALF_LENGTH), Vec2::new(HALF_WIDTH, HALF_LENGTH));
        assert!(n.x < -0.99);
        let n = inward_of(Vec2::new(-HALF_WIDTH, HALF_LENGTH), Vec2::new(-GOAL_HALF_WIDTH, HALF_LENGTH));
        assert!(n.z < -0.99);
    }
}
