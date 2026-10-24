//! Aim aids, as in Rematch:
//! - a semi-transparent reticle that turns opaque from the bottom up as the kick charges,
//!   with a dot pushed toward the curve you're putting on a shot;
//! - a line on the ground in the pass direction, blue going green as the pass gets stronger;
//! - a charge bar for passes, marked at the build-up end and the 0.6 s sweet spot.

use bevy::prelude::*;

use crate::input::{Axes, Bindings};
use crate::player::{CameraRig, Kick, Player, forward, pass_yaw};
use crate::tuning::*;

const RING: f32 = 44.0; // px
const BORDER: f32 = 3.0;
const DOT: f32 = 8.0;
const BAR_W: f32 = 120.0;
const BAR_H: f32 = 6.0;
const FAINT: Color = Color::srgba(1.0, 1.0, 1.0, 0.3);
const SOLID: Color = Color::srgba(1.0, 1.0, 1.0, 0.95);
const CHARGING: Color = Color::srgb(1.0, 0.6, 0.2);
const SWEET: Color = Color::srgb(0.3, 1.0, 0.45);
const LINE_WEAK: Color = Color::srgb(0.2, 0.5, 1.0);
const LINE_STRONG: Color = Color::srgb(0.2, 1.0, 0.35);

#[derive(Component)]
pub struct CurveDot;
/// Clips the solid ring to the charged fraction, growing from the bottom.
#[derive(Component)]
pub struct RingFill;
#[derive(Component)]
pub struct ChargeBar;
#[derive(Component)]
pub struct ChargeFill;

fn centered(size: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        width: Val::Px(size),
        height: Val::Px(size),
        margin: UiRect { left: Val::Px(-size / 2.0), top: Val::Px(-size / 2.0), ..default() },
        ..default()
    }
}

fn ring(color: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Px(RING),
            height: Val::Px(RING),
            border: UiRect::all(Val::Px(BORDER)),
            ..default()
        },
        BorderColor(color),
        BorderRadius::MAX,
    )
}

pub fn spawn_reticle(mut commands: Commands) {
    commands.spawn(centered(RING)).with_children(|root| {
        root.spawn(ring(FAINT));
        root.spawn((
            RingFill,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .with_child(ring(SOLID));
        root.spawn((
            CurveDot,
            Node { position_type: PositionType::Absolute, width: Val::Px(DOT), height: Val::Px(DOT), ..default() },
            BackgroundColor(Color::WHITE),
            BorderRadius::MAX,
            Visibility::Hidden,
        ));
    });

    commands
        .spawn((
            ChargeBar,
            Node {
                width: Val::Px(BAR_W),
                height: Val::Px(BAR_H),
                margin: UiRect { left: Val::Px(-BAR_W / 2.0), top: Val::Px(RING), ..default() },
                ..centered(0.0)
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            bar.spawn((ChargeFill, Node { height: Val::Percent(100.0), width: Val::Percent(0.0), ..default() }, BackgroundColor(CHARGING)));
            for seconds in [PASS_BUILDUP, PASS_SWEET_SPOT] {
                // Ticks at the build-up end and the sweet spot.
                bar.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(seconds / MAX_HOLD * 100.0),
                        width: Val::Px(2.0),
                        height: Val::Px(BAR_H + 6.0),
                        top: Val::Px(-3.0),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                ));
            }
        });
}

/// Charge as a 0..1 share of the kick's own force range, for the reticle and the line colour.
fn power(kind: Kick, held: f32) -> f32 {
    let (points, min, max): (&[(f32, f32)], f32, f32) = match kind {
        Kick::Shot => (&SHOT_FORCE, SHOT_FORCE[0].1, SHOT_FORCE[2].1),
        Kick::Pass => (&PASS_FORCE, PASS_FORCE[0].1, PASS_FORCE[2].1), // peak is the 0.6 s sweet spot
        Kick::Lob => (&LOB_FORCE, LOB_FORCE[0].1, LOB_FORCE[2].1),
        _ => return 0.0,
    };
    ((curve(points, held) - min) / (max - min)).clamp(0.0, 1.0)
}

pub fn update_reticle(
    player: Single<&Player>,
    axes: Res<Axes>,
    mut fill: Single<&mut Node, (With<RingFill>, Without<CurveDot>, Without<ChargeFill>)>,
    mut dot: Single<(&mut Node, &mut Visibility), (With<CurveDot>, Without<RingFill>, Without<ChargeFill>)>,
    mut bar: Single<&mut Visibility, (With<ChargeBar>, Without<CurveDot>)>,
    mut bar_fill: Single<(&mut Node, &mut BackgroundColor), (With<ChargeFill>, Without<CurveDot>, Without<RingFill>)>,
) {
    let charging = player.kick.filter(|k| !k.released);
    let shot = charging.is_some_and(|k| k.kind == Kick::Shot);

    fill.height = Val::Percent(charging.map_or(0.0, |k| power(k.kind, k.held)) * 100.0);

    // The dot sits inside the ring, pushed toward the curve: right/left for side spin, up/down for dip/lift.
    let (dot_node, dot_vis) = &mut *dot;
    **dot_vis = if shot { Visibility::Inherited } else { Visibility::Hidden };
    let r = RING / 2.0 - BORDER - DOT / 2.0 - 1.0;
    let offset = axes.curve.clamp_length_max(1.0) * r;
    dot_node.left = Val::Px(RING / 2.0 - DOT / 2.0 + offset.x);
    dot_node.top = Val::Px(RING / 2.0 - DOT / 2.0 - offset.y);

    let pass = charging.filter(|k| k.kind == Kick::Pass);
    **bar = if pass.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if let Some(k) = pass {
        let (node, color) = &mut *bar_fill;
        node.width = Val::Percent((k.held / MAX_HOLD).min(1.0) * 100.0);
        color.0 = if (k.held - PASS_SWEET_SPOT).abs() < 0.05 { SWEET } else { CHARGING };
    }
}

/// Ground line in the pass direction while a pass or lob charges.
pub fn pass_line(mut gizmos: Gizmos, player: Single<(&Transform, &Player)>, rig: Single<&CameraRig>, axes: Res<Axes>, bindings: Res<Bindings>) {
    let (tf, p) = *player;
    let Some(k) = p.kick.filter(|k| k.kind.is_pass() && !k.released) else { return };
    let t = power(k.kind, k.held);
    let dir = forward(pass_yaw(bindings.pass_aim, &axes, &rig));
    let start = Vec3::new(tf.translation.x, 0.03, tf.translation.z) + dir * 0.6;
    let color = LINE_WEAK.mix(&LINE_STRONG, t);
    gizmos.line(start, start + dir * (4.0 + 6.0 * t), color);
}
