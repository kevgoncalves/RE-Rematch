//! Aim reticle: a ring at the aim point with a dot that shows the curve you're putting on a shot,
//! plus a charge bar marked where the build-up ends (and the pass sweet spot).

use bevy::prelude::*;

use crate::input::Axes;
use crate::player::{Kick, Player};
use crate::tuning::*;

const RING: f32 = 44.0; // px
const DOT: f32 = 8.0;
const BAR_W: f32 = 120.0;
const BAR_H: f32 = 6.0;
const IDLE: Color = Color::srgba(1.0, 1.0, 1.0, 0.55);
const CHARGING: Color = Color::srgb(1.0, 0.6, 0.2);
const SWEET: Color = Color::srgb(0.3, 1.0, 0.45);

#[derive(Component)]
pub struct Ring;
#[derive(Component)]
pub struct CurveDot;
#[derive(Component)]
pub struct ChargeBar;
#[derive(Component)]
pub struct ChargeFill;
/// A tick on the charge bar at this many seconds held.
#[derive(Component)]
pub struct ChargeMark(f32);

fn centered(size: f32, y_offset: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        width: Val::Px(size),
        height: Val::Px(size),
        margin: UiRect { left: Val::Px(-size / 2.0), top: Val::Px(-size / 2.0 + y_offset), ..default() },
        ..default()
    }
}

pub fn spawn_reticle(mut commands: Commands) {
    commands
        .spawn((Ring, Node { border: UiRect::all(Val::Px(2.0)), ..centered(RING, 0.0) }, BorderColor(IDLE), BorderRadius::MAX))
        .with_child((
            CurveDot,
            Node { position_type: PositionType::Absolute, width: Val::Px(DOT), height: Val::Px(DOT), ..default() },
            BackgroundColor(Color::WHITE),
            BorderRadius::MAX,
            Visibility::Hidden,
        ));

    commands
        .spawn((
            ChargeBar,
            Node { height: Val::Px(BAR_H), width: Val::Px(BAR_W), margin: UiRect { left: Val::Px(-BAR_W / 2.0), top: Val::Px(RING), ..default() }, ..centered(0.0, 0.0) },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            bar.spawn((ChargeFill, Node { height: Val::Percent(100.0), width: Val::Percent(0.0), ..default() }, BackgroundColor(CHARGING)));
            for seconds in [SHOT_BUILDUP, PASS_BUILDUP, PASS_SWEET_SPOT] {
                bar.spawn((
                    ChargeMark(seconds),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(seconds / MAX_HOLD * 100.0),
                        width: Val::Px(2.0),
                        height: Val::Px(BAR_H + 6.0),
                        top: Val::Px(-3.0),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                    Visibility::Hidden,
                ));
            }
        });
}

pub fn update_reticle(
    player: Single<&Player>,
    axes: Res<Axes>,
    mut ring: Single<&mut BorderColor, With<Ring>>,
    mut dot: Single<(&mut Node, &mut Visibility), (With<CurveDot>, Without<ChargeFill>, Without<ChargeMark>)>,
    mut bar: Single<&mut Visibility, (With<ChargeBar>, Without<CurveDot>, Without<ChargeMark>)>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), (With<ChargeFill>, Without<CurveDot>, Without<ChargeMark>)>,
    mut marks: Query<(&ChargeMark, &mut Visibility), (Without<CurveDot>, Without<ChargeBar>)>,
) {
    let charging = player.kick.filter(|k| !k.released);
    let shot = charging.is_some_and(|k| k.kind == Kick::Shot);

    ring.0 = if shot { CHARGING } else { IDLE };

    // The dot sits inside the ring, pushed toward the curve: right/left for side spin, up/down for top/back.
    let (dot_node, dot_vis) = &mut *dot;
    **dot_vis = if shot { Visibility::Inherited } else { Visibility::Hidden };
    let r = (RING - DOT) / 2.0 - 2.0;
    let offset = axes.curve.clamp_length_max(1.0) * r;
    dot_node.left = Val::Px(RING / 2.0 - 2.0 - DOT / 2.0 + offset.x);
    dot_node.top = Val::Px(RING / 2.0 - 2.0 - DOT / 2.0 - offset.y);

    **bar = if charging.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    let Some(k) = charging else { return };
    let (fill_node, fill_color) = &mut *fill;
    fill_node.width = Val::Percent((k.held / MAX_HOLD).min(1.0) * 100.0);
    let near_sweet_spot = k.kind == Kick::Pass && (k.held - PASS_SWEET_SPOT).abs() < 0.05;
    fill_color.0 = if near_sweet_spot { SWEET } else { CHARGING };
    let buildup = match k.kind {
        Kick::Shot => SHOT_BUILDUP,
        _ => PASS_BUILDUP,
    };
    for (ChargeMark(at), mut vis) in &mut marks {
        let show = *at == buildup || (k.kind == Kick::Pass && *at == PASS_SWEET_SPOT);
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
}
