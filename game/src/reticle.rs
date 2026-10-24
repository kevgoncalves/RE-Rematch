//! Aim aids, as in Rematch (values from its crosshair widget, notes/hud.md):
//! - a reticle of two short arcs, white with a soft dark outline. Charging a shot closes them
//!   into a faint circle that fills solid from the bottom with the shot's force, and shows a
//!   Magnus point that follows the curve input. Lock move closes them too, at full opacity.
//! Rematch has no charge bar for passes; the pass line (pass_line.rs) shows their strength.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

use crate::input::Axes;
use crate::player::{Kick, Player};
use crate::tuning::*;

/// Reticle size at 1080p; scaled with the window height like the game's UI.
const RETICLE_PX: f32 = 140.0;
const IDLE_OPENING: f32 = 0.225;
const AIM_ARC_OPACITY: f32 = 0.4;
/// The force fill never shows less than this.
const MIN_FORCE_FILL: f32 = 0.03;
/// Transition times, in seconds.
const AIM_IN: f32 = 0.2;
const AIM_OUT: f32 = 0.3;
const FORCE_IN: f32 = 0.083;
const FORCE_OUT: f32 = 0.2;
const DEFENSE: f32 = 0.214;


#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct ReticleMaterial {
    /// Opening, arc opacity, force, force opacity.
    #[uniform(0)]
    shape: Vec4,
    /// Curve x, curve y, Magnus point opacity.
    #[uniform(1)]
    magnus: Vec4,
}

impl UiMaterial for ReticleMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/reticle.wgsl".into()
    }
}

#[derive(Component)]
pub struct Reticle;

/// Where each reticle transition is, 0..1, plus the last force and curve so they fade out in place.
#[derive(Default)]
pub struct ReticleAnim {
    aim: f32,
    defense: f32,
    force_shown: f32,
    force: f32,
    curve: Vec2,
}

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

pub fn spawn_reticle(mut commands: Commands, mut materials: ResMut<Assets<ReticleMaterial>>) {
    commands.spawn((
        Reticle,
        centered(RETICLE_PX),
        MaterialNode(materials.add(ReticleMaterial { shape: Vec4::new(IDLE_OPENING, 1.0, 0.0, 0.0), ..default() })),
    ));

}

/// Charge as a 0..1 share of the kick's own force range, for the reticle and the line colour.
pub fn power(kind: Kick, held: f32) -> f32 {
    let (points, min, max): (&[(f32, f32)], f32, f32) = match kind {
        Kick::Shot => (&SHOT_FORCE, SHOT_FORCE[0].1, SHOT_FORCE[2].1),
        Kick::Pass => (&PASS_FORCE, PASS_FORCE[0].1, PASS_FORCE[2].1), // peak is the 0.6 s sweet spot
        Kick::Lob => (&LOB_FORCE, LOB_FORCE[0].1, LOB_FORCE[2].1),
        _ => return 0.0,
    };
    ((curve(points, held) - min) / (max - min)).clamp(0.0, 1.0)
}

/// Moves `x` toward on/off at a rate that takes `time_in` / `time_out` seconds end to end.
fn step(x: &mut f32, on: bool, time_in: f32, time_out: f32, dt: f32) {
    *x = if on { (*x + dt / time_in).min(1.0) } else { (*x - dt / time_out).max(0.0) };
}

fn ease(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

pub fn update_reticle(
    time: Res<Time>,
    window: Single<&Window>,
    player: Single<&Player>,
    axes: Res<Axes>,
    mut anim: Local<ReticleAnim>,
    mut materials: ResMut<Assets<ReticleMaterial>>,
    mut reticle: Single<(&mut Node, &MaterialNode<ReticleMaterial>), With<Reticle>>,
) {
    let dt = time.delta_secs();
    let charging = player.kick.filter(|k| !k.released);
    let shot = charging.filter(|k| k.kind == Kick::Shot);
    if let Some(k) = shot {
        anim.force = power(k.kind, k.held).max(MIN_FORCE_FILL);
        anim.curve = axes.curve.clamp_length_max(1.0);
    }
    let on = shot.is_some();
    step(&mut anim.aim, on, AIM_IN, AIM_OUT, dt);
    step(&mut anim.force_shown, on, FORCE_IN, FORCE_OUT, dt);
    step(&mut anim.defense, player.locked, DEFENSE, DEFENSE, dt);

    let (node, handle) = &mut *reticle;
    let size = RETICLE_PX * window.height() / 1080.0;
    node.width = Val::Px(size);
    node.height = Val::Px(size);
    node.margin = UiRect { left: Val::Px(-size / 2.0), top: Val::Px(-size / 2.0), ..default() };
    if let Some(m) = materials.get_mut(&handle.0) {
        let (aim, closed) = (ease(anim.aim), ease(anim.aim.max(anim.defense)));
        m.shape = Vec4::new(IDLE_OPENING.lerp(1.0, closed), 1.0.lerp(AIM_ARC_OPACITY, aim), anim.force, ease(anim.force_shown));
        m.magnus = Vec4::new(anim.curve.x, anim.curve.y, aim, 0.0);
    }

}
