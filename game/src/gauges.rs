//! Stamina and extra effort bars, laid out like Rematch's HUD (notes/hud.md): bottom centre,
//! a slanted stamina bar with the extra effort bar beside it.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

use crate::player::Player;
use crate::tuning::*;

/// Sizes in px at 1080p, scaled with the window height.
const STAMINA_W: f32 = 280.0;
const EXTRA_EFFORT_W: f32 = 80.0;
const GAUGE_H: f32 = 20.0;
const GAP: f32 = 4.0;
const FROM_BOTTOM: f32 = 65.0;

/// (left, right) colours.
const STAMINA: (Color, Color) = (Color::srgb_u8(0x65, 0xE4, 0x74), Color::srgb_u8(0x52, 0xED, 0xCE));
const EXHAUSTED: (Color, Color) = (Color::srgb_u8(0xB5, 0x13, 0x13), Color::srgb_u8(0xB5, 0x13, 0x13));
const EXTRA_EFFORT: (Color, Color) = (Color::srgb_u8(0xE6, 0xF8, 0xFF), Color::srgb_u8(0x6E, 0xED, 0xFF));
/// Extra effort refilling after use (our reading of the game's "recovery" colours).
const EXTRA_EFFORT_REFILL: (Color, Color) = (Color::srgb_u8(0x63, 0x73, 0x7F), Color::srgb_u8(0x90, 0xA1, 0xAF));

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct GaugeMaterial {
    /// Fill, unused, exhaust marker position (negative hides it).
    #[uniform(0)]
    gauge: Vec4,
    #[uniform(1)]
    left: LinearRgba,
    #[uniform(2)]
    right: LinearRgba,
}

impl UiMaterial for GaugeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/gauge.wgsl".into()
    }
}

#[derive(Component)]
pub struct GaugeRow;

#[derive(Component, Clone, Copy)]
pub enum Gauge {
    Stamina,
    ExtraEffort,
}

pub fn spawn_gauges(mut commands: Commands, mut materials: ResMut<Assets<GaugeMaterial>>) {
    commands
        .spawn((
            GaugeRow,
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        ))
        .with_children(|row| {
            for gauge in [Gauge::Stamina, Gauge::ExtraEffort] {
                row.spawn((gauge, Node::default(), MaterialNode(materials.add(GaugeMaterial::default()))));
            }
        });
}

pub fn update_gauges(
    window: Single<&Window>,
    player: Single<&Player>,
    mut row: Single<&mut Node, (With<GaugeRow>, Without<Gauge>)>,
    mut gauges: Query<(&Gauge, &mut Node, &MaterialNode<GaugeMaterial>)>,
    mut materials: ResMut<Assets<GaugeMaterial>>,
) {
    let s = window.height() / 1080.0;
    row.bottom = Val::Px(FROM_BOTTOM * s);
    row.column_gap = Val::Px(GAP * s);
    let p = *player;
    for (gauge, mut node, handle) in &mut gauges {
        let (width, fill, marker, (left, right)) = match gauge {
            Gauge::Stamina => (
                STAMINA_W,
                p.stamina / MAX_STAMINA,
                if p.exhausted { EXHAUSTED_EXIT / MAX_STAMINA } else { -1.0 },
                if p.exhausted { EXHAUSTED } else { STAMINA },
            ),
            Gauge::ExtraEffort => (
                EXTRA_EFFORT_W,
                p.extra_effort / MAX_EXTRA_EFFORT,
                -1.0,
                if p.extra_active || p.extra_effort >= MAX_EXTRA_EFFORT { EXTRA_EFFORT } else { EXTRA_EFFORT_REFILL },
            ),
        };
        node.width = Val::Px(width * s);
        node.height = Val::Px(GAUGE_H * s);
        if let Some(m) = materials.get_mut(&handle.0) {
            m.gauge = Vec4::new(fill, 0.0, marker, 0.0);
            m.left = left.to_linear();
            m.right = right.to_linear();
        }
    }
}
