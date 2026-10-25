//! The animated character: Mixamo's "Shannon" and Kev's Mixamo clips (made into
//! assets/characters/shannon.glb by tools/mixamo_to_glb.py), behind the "Animated character
//! (Mixamo)" cheat. It stands in for the procedural body, which keeps running hidden underneath
//! (the effects read its feet).
//!
//! One animation graph holds every clip under its root blend node. Each frame the mixer sets every
//! clip's weight and time from the player's state, so the crossfades, the foot-synced cycles and
//! the kicks timed to the game's contact are all decided here, as data the tests can read:
//!
//! - locomotion: idle, walk, run and sprint by speed, all on one phase so the feet stay in step,
//!   advancing at the speed over the blended stride so they don't slide; lock move (and any move
//!   that isn't ahead, like a side dribble or a dash back) blends the eight jogs by the movement's
//!   angle to your facing;
//! - kicks: each clip's contact frame lands on the game's contact. It winds up at its own speed and
//!   holds at the top of the backswing while you charge, then strikes so the foot meets the ball as
//!   the game's ball leaves, sped up only when the game's contact comes sooner;
//! - moves: the roulette's clip follows the game's own turn (which turns the body), the turn and
//!   stop clips the game's turn and braking, the chest control is an upper-body layer over the
//!   running legs.
//!
//! The gameplay timings are the source of truth: nothing here feeds back into the game. On top, as
//! in the procedural body: the lean into turns, the lower body turning toward the velocity, the
//! kick turning toward its target, the trunk leaning on as stamina drops.
//!
//! The look is Rematch's (Kev): the glb's flat colours per region, cel shaded in soft bands with a
//! rim light (assets/shaders/toon.wgsl) and a thin outline (an inverted hull,
//! assets/shaders/outline.wgsl); the shirt, shorts and socks take the team's `Kit`. A realistic glb
//! (`--style real`) keeps its textures and Bevy's own shading.

use bevy::animation::AnimationTarget;
use bevy::app::Animation;
use bevy::asset::io::file::FileAssetReader;
use bevy::gltf::{Gltf, GltfMaterialName};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPipeline, MaterialPipelineKey, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::mesh::skinning::SkinnedMesh;
use bevy::render::render_resource::{AsBindGroup, Face, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError};
use bevy::render::view::NoFrustumCulling;
use serde::Deserialize;

use crate::body::Joint;
use crate::fx::{Cheat, Cheats};
use crate::player::{FeintTurn, Heading, Kick, KickState, Player, contact_time, dribble_contact, roulette_turn, turn_out_angle, yaw_of};
use crate::tuning::*;

const GLB: &str = "characters/shannon.glb";
const SIDECAR: &str = "characters/shannon.json";

/// The player's root is the capsule's centre, this far above the feet.
const FEET: f32 = -0.9;

/// Locomotion weights ease toward their targets with this time constant: every change of cycle,
/// speed tier or direction crossfades.
const LOCO_FADE: f32 = 0.12;
/// Free move turns to the directional jogs past this angle between the movement and the facing,
/// and back under the second.
const SIDEWAYS: (f32, f32) = (60.0, 40.0);
/// A late release still gets this long a strike: the foot meets the ball up to 4 frames after it
/// left (as the procedural body).
const STRIKE: f32 = 4.0 / 60.0;
/// The chest control's share of the upper body at most.
const CONTROL_SHARE: f32 = 0.85;
/// Where the control's clip has the ball (bent over it, arms out), against the game's snap.
const CONTROL_CATCH: f32 = 0.55;
/// A stop plays the stop clip up to where it has braked this share of its travel.
const STOP_SHARE: f32 = 0.55;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clip {
    Idle,
    IdleBall,
    Walk,
    WalkBack,
    Run,
    RunBack,
    Sprint,
    Jog,
    JogBack,
    StrafeLeft,
    StrafeRight,
    DiagLeft,
    DiagRight,
    BackDiagLeft,
    BackDiagRight,
    Pass,
    Chip,
    Strike,
    StrikeLeft,
    PenaltyKick,
    Dribble,
    ReceiveChest,
    Receive,
    Header,
    Spin,
    Turn180,
    Turn180Left,
    RunStop,
}

pub const CLIPS: usize = 28;
/// Each clip has two nodes, so a one-shot can crossfade into itself (a pass after a pass).
const NODES: usize = 2 * CLIPS;

impl Clip {
    pub const ALL: [Clip; CLIPS] = [
        Clip::Idle,
        Clip::IdleBall,
        Clip::Walk,
        Clip::WalkBack,
        Clip::Run,
        Clip::RunBack,
        Clip::Sprint,
        Clip::Jog,
        Clip::JogBack,
        Clip::StrafeLeft,
        Clip::StrafeRight,
        Clip::DiagLeft,
        Clip::DiagRight,
        Clip::BackDiagLeft,
        Clip::BackDiagRight,
        Clip::Pass,
        Clip::Chip,
        Clip::Strike,
        Clip::StrikeLeft,
        Clip::PenaltyKick,
        Clip::Dribble,
        Clip::ReceiveChest,
        Clip::Receive,
        Clip::Header,
        Clip::Spin,
        Clip::Turn180,
        Clip::Turn180Left,
        Clip::RunStop,
    ];

    /// The animation's name in the glb (tools/mixamo_to_glb.py).
    pub fn name(self) -> &'static str {
        match self {
            Clip::Idle => "idle",
            Clip::IdleBall => "idle_ball",
            Clip::Walk => "walk",
            Clip::WalkBack => "walk_back",
            Clip::Run => "run",
            Clip::RunBack => "run_back",
            Clip::Sprint => "sprint",
            Clip::Jog => "jog",
            Clip::JogBack => "jog_back",
            Clip::StrafeLeft => "jog_strafe_left",
            Clip::StrafeRight => "jog_strafe_right",
            Clip::DiagLeft => "jog_diag_left",
            Clip::DiagRight => "jog_diag_right",
            Clip::BackDiagLeft => "jog_back_diag_left",
            Clip::BackDiagRight => "jog_back_diag_right",
            Clip::Pass => "pass",
            Clip::Chip => "chip",
            Clip::Strike => "strike_jog",
            Clip::StrikeLeft => "strike_jog_left",
            Clip::PenaltyKick => "penalty_kick",
            Clip::Dribble => "dribble",
            Clip::ReceiveChest => "receive_chest",
            Clip::Receive => "receive",
            Clip::Header => "header",
            Clip::Spin => "spin",
            Clip::Turn180 => "turn_180",
            Clip::Turn180Left => "turn_180_left",
            Clip::RunStop => "run_stop",
        }
    }

    /// Plays on the upper body only, over the legs' locomotion.
    fn upper(self) -> bool {
        self == Clip::ReceiveChest
    }

    /// Where the clip starts when it plays as a one-shot (skipping a standing lead-in).
    fn start(self) -> f32 {
        match self {
            Clip::Pass => 0.15,
            Clip::Chip => 0.1,
            Clip::Dribble => 0.2,
            _ => 0.0,
        }
    }
}

/// A clip's numbers from the sidecar (shannon.json).
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ClipMeta {
    pub duration: f32,
    /// A cycle's speed (m/s) and the phase where its right foot is down and the left highest.
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub sync: f32,
    /// A kick's top of the backswing and contact (s).
    pub windup: Option<f32>,
    pub contact: Option<f32>,
    /// A one-shot's travel and turn taken out, per frame: right, ahead (m), turn (deg, left +).
    #[serde(default)]
    pub root: Vec<[f32; 3]>,
}

#[derive(Deserialize)]
struct Sidecar {
    fps: f32,
    clips: std::collections::HashMap<String, ClipMeta>,
}

#[derive(Clone, Debug)]
pub struct Metas {
    clips: Vec<ClipMeta>,
    fps: f32,
}

impl Metas {
    fn from_sidecar(s: Sidecar) -> Result<Self, String> {
        let clips = Clip::ALL.iter().map(|c| s.clips.get(c.name()).cloned().ok_or(format!("no clip {}", c.name()))).collect::<Result<_, _>>()?;
        Ok(Self { clips, fps: s.fps })
    }

    pub fn get(&self, c: Clip) -> &ClipMeta {
        &self.clips[c as usize]
    }

    /// How far a cycle carries you per loop.
    fn stride(&self, c: Clip) -> f32 {
        let m = self.get(c);
        m.speed * m.duration
    }

    /// When the clip's taken-out turn first reaches `deg` (its sign ignored); the clip's end if it
    /// never does.
    fn turn_at(&self, c: Clip, deg: f32) -> f32 {
        self.root_at(c, |r| r[2].abs(), deg)
    }

    /// When the clip's taken-out travel ahead reaches `share` of all of it.
    fn travel_at(&self, c: Clip, share: f32) -> f32 {
        let total = self.get(c).root.last().map_or(0.0, |r| r[1]);
        self.root_at(c, |r| r[1], share * total)
    }

    fn root_at(&self, c: Clip, value: impl Fn(&[f32; 3]) -> f32, at: f32) -> f32 {
        let m = self.get(c);
        for (i, w) in m.root.windows(2).enumerate() {
            let (a, b) = (value(&w[0]), value(&w[1]));
            if b >= at && a < at {
                return (i as f32 + (at - a) / (b - a)) / self.fps;
            }
        }
        if m.root.first().is_some_and(|r| value(r) >= at) { 0.0 } else { m.duration }
    }
}

/// What the mixer sets this frame: each node's weight and time, and the touches on top.
#[derive(Clone, Debug)]
pub struct Mix {
    pub weights: [f32; NODES],
    pub times: [f32; NODES],
    /// The whole body's turn (deg, left +): the roulette's and turn-out's spin, the kick's turn.
    pub yaw: f32,
    /// The lean into the turn (deg, right +), from the feet.
    pub lean: f32,
    /// The lower body's turn toward the velocity (deg, left +); the trunk turns back.
    pub warp: f32,
    /// The trunk's lean forward (deg).
    pub pitch: f32,
}

impl Default for Mix {
    fn default() -> Self {
        let mut weights = [0.0; NODES];
        weights[Clip::Idle as usize] = 1.0;
        Self { weights, times: [0.0; NODES], yaw: 0.0, lean: 0.0, warp: 0.0, pitch: 0.0 }
    }
}

#[cfg(test)]
impl Mix {
    /// A clip's weight over both its nodes.
    pub fn weight(&self, c: Clip) -> f32 {
        self.weights[c as usize] + self.weights[c as usize + CLIPS]
    }

    /// The clip that weighs most.
    pub fn top(&self) -> Clip {
        *Clip::ALL.iter().max_by(|a, b| self.weight(**a).total_cmp(&self.weight(**b))).unwrap()
    }
}

/// A one-shot playing or fading out.
#[derive(Clone, Debug)]
struct Shot {
    clip: Clip,
    slot: usize,
    key: u32,
    t: f32,
    w: f32,
    target: f32,
    live: bool,
    fade_in: f32,
    fade_out: f32,
}

/// The one-shot the player's state asks for this frame. A new key starts it afresh.
#[derive(Clone, Copy, Debug)]
struct Want {
    clip: Clip,
    key: u32,
    t: f32,
    weight: f32,
    fade_in: f32,
    fade_out: f32,
}

#[derive(Clone, Copy, Debug)]
struct KickTrack {
    key: u32,
    kind: Kick,
    clip: Clip,
    t: f32,
    elapsed: f32,
}

#[derive(Clone, Copy, Debug)]
struct Follow {
    key: u32,
    kind: Kick,
    clip: Clip,
    t: f32,
    since: f32,
}

/// The kinds of one-shot that aren't kicks, to tell when a new one starts.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Move {
    Fake,
    Spin,
    Touch,
    Control,
    Turn,
    Stop,
}

#[derive(Default)]
pub struct Mixer {
    phase: f32,
    idle: f32,
    loco: [f32; CLIPS],
    sideways: bool,
    shots: Vec<Shot>,
    keys: u32,
    kick: Option<KickTrack>,
    follow: Option<Follow>,
    /// The move playing: its kind, key, clip time and last game time.
    current: Option<(Move, u32, f32, f32)>,
    stop_entry: f32,
    prev_speed: f32,
    heading: Vec3,
    lateral: f32,
    roll: f32,
    pitch: f32,
    spin: f32,
    kick_turn: f32,
    kick_key: u32,
    ready: bool,
    last: Mix,
}

/// A kick's follow-through after its contact, to where the run picks up again (body.rs).
fn follow_time(kind: Kick) -> f32 {
    match kind {
        Kick::Shot => 0.5,
        Kick::PushBall | Kick::DribblePushBall => 0.2,
        _ => 0.25,
    }
}

fn smooth(w: f32) -> f32 {
    let w = w.clamp(0.0, 1.0);
    w * w * (3.0 - 2.0 * w)
}

fn approach(value: f32, target: f32, up: f32, down: f32, dt: f32) -> f32 {
    if target > value { (value + dt / up.max(1e-3)).min(target) } else { (value - dt / down.max(1e-3)).max(target) }
}

/// The locomotion cycles' target weights for a speed and the movement's angle to your facing (deg,
/// 0 ahead, +90 right): by speed among idle, walk, run and sprint going ahead; between the two
/// nearest of the eight jogs (and idle, slow) when directional.
pub fn locomotion(speed: f32, angle: f32, directional: bool, ball: bool, m: &Metas) -> [f32; CLIPS] {
    let mut w = [0.0; CLIPS];
    let idle = if ball { Clip::IdleBall } else { Clip::Idle };
    if directional {
        const RING: [Clip; 8] = [Clip::Jog, Clip::DiagRight, Clip::StrafeRight, Clip::BackDiagRight, Clip::JogBack, Clip::BackDiagLeft, Clip::StrafeLeft, Clip::DiagLeft];
        let moving = (speed / (0.5 * m.get(Clip::Jog).speed.max(0.1))).clamp(0.0, 1.0);
        let sector = angle.rem_euclid(360.0) / 45.0;
        let (i, f) = (sector.floor() as usize % 8, sector.fract());
        w[RING[i] as usize] += moving * (1.0 - f);
        w[RING[(i + 1) % 8] as usize] += moving * f;
        w[idle as usize] += 1.0 - moving;
        return w;
    }
    let ladder = [(0.0, idle), (m.get(Clip::Walk).speed, Clip::Walk), (m.get(Clip::Run).speed, Clip::Run), (m.get(Clip::Sprint).speed, Clip::Sprint)];
    match ladder.windows(2).find(|p| speed < p[1].0) {
        Some(p) => {
            let f = ((speed - p[0].0) / (p[1].0 - p[0].0)).clamp(0.0, 1.0);
            w[p[0].1 as usize] += 1.0 - f;
            w[p[1].1 as usize] += f;
        }
        None => w[Clip::Sprint as usize] = 1.0,
    }
    w
}

/// When the game's foot meets the ball, from the press, once it's known: a charged kick's at its
/// release (no sooner than its contact frame, and with a strike's time after the release), the
/// others' at their contact frame.
pub fn kick_contact(k: &KickState) -> Option<f32> {
    let first = contact_time(k);
    if !matches!(k.kind, Kick::Shot | Kick::Pass | Kick::Lob) {
        return Some(first);
    }
    if !k.released {
        return None;
    }
    let release = (k.held - k.pre).max(0.0);
    Some(if release <= first - STRIKE { first } else { release + STRIKE })
}

/// The kick clip's time this frame from last frame's `t`: it winds up at its own speed and holds at
/// the top of the backswing until the contact is known, then strikes so its contact frame lands on
/// the game's contact, at its own speed when there's time and faster when there isn't.
pub fn kick_time(t: f32, k: &KickState, dt: f32, m: &ClipMeta) -> f32 {
    let windup = m.windup.unwrap_or(0.0);
    let contact = m.contact.unwrap_or(0.4 * m.duration);
    let Some(at) = kick_contact(k) else { return (t + dt).min(windup.max(t)) };
    // Game seconds left to the contact after this frame.
    let left = at - k.elapsed;
    if left <= 0.0 {
        return contact.max(t);
    }
    let to_go = contact - t;
    if to_go > left + dt {
        t + to_go * dt / (left + dt)
    } else {
        (t + dt).min(windup.max(contact - left)).max(t)
    }
}

/// The kick's clip: shots with the right foot, or the left one across to the right (as the
/// procedural body); passes, lobs and push balls their own.
fn kick_clip(kind: Kick, turn: f32) -> Clip {
    match kind {
        Kick::Shot if turn < -30.0 => Clip::StrikeLeft,
        Kick::Shot => Clip::Strike,
        Kick::Pass | Kick::ShortPass => Clip::Pass,
        Kick::Lob | Kick::ShortLob => Clip::Chip,
        Kick::PushBall | Kick::DribblePushBall => Clip::Dribble,
    }
}

/// The roulette clip's time for the game's `t` into it: the clip's own turn (taken out) is matched
/// to the game's, which turns the body.
pub fn spin_time(t: f32, m: &Metas) -> f32 {
    let (start, _, _, end) = ROULETTE_TURN;
    let turn = roulette_turn(t);
    let (first, last) = (m.turn_at(Clip::Spin, 1.0), m.turn_at(Clip::Spin, 359.0));
    if turn <= 0.0 {
        first * (t / start).min(1.0)
    } else if turn >= 360.0 {
        last + (t - end)
    } else {
        m.turn_at(Clip::Spin, turn.clamp(1.0, 359.0))
    }
}

/// The turn clip's time at the game turn's progress `u`, its brake ending at `brake`: the clip's
/// pivot (its quarter turn) lands where the game's brake ends.
pub fn turn_time(u: f32, brake: f32, m: &Metas) -> f32 {
    let (pivot, end) = (m.turn_at(Clip::Turn180, 90.0), m.turn_at(Clip::Turn180, 175.0));
    if u < brake { pivot * u / brake.max(1e-3) } else { pivot + (end - pivot) * (u - brake) / (1.0 - brake).max(1e-3) }
}

impl Mixer {
    fn key(&mut self) -> u32 {
        self.keys += 1;
        self.keys
    }

    /// The move's key, new when the kind changes, its time runs back or it wasn't playing.
    fn instance(&mut self, kind: Move, game_t: f32) -> u32 {
        match self.current {
            Some((k, key, _, last)) if k == kind && game_t >= last => key,
            _ => {
                let key = self.key();
                self.current = Some((kind, key, 0.0, game_t));
                key
            }
        }
    }

    fn set_current(&mut self, t: f32, game_t: f32) {
        if let Some(c) = self.current.as_mut() {
            c.2 = t;
            c.3 = game_t;
        }
    }

    pub fn update(&mut self, p: &Player, rotation: Quat, dt: f32, m: &Metas) -> Mix {
        if dt <= 0.0 && self.ready {
            return self.last.clone();
        }
        let local = rotation.inverse() * p.vel;
        let speed = Vec2::new(local.x, local.z).length();
        let angle = if speed > 0.1 { local.x.atan2(-local.z).to_degrees() } else { 0.0 };

        // Cornering: the smoothed turn of the travel direction, + right (body.rs).
        let heading = Vec3::new(p.vel.x, 0.0, p.vel.z).normalize_or_zero();
        let turn = if speed > 0.5 && self.heading != Vec3::ZERO { heading.dot(self.heading.cross(Vec3::Y)) } else { 0.0 };
        self.heading = heading;
        self.lateral += (turn - self.lateral) * (7.0 * dt).min(1.0);

        // Locomotion: the targets eased in; one phase for every cycle.
        let limits = if self.sideways { SIDEWAYS.1 } else { SIDEWAYS.0 };
        self.sideways = !p.locked && speed > 0.5 && angle.abs() > limits;
        let directional = p.locked || self.sideways;
        let target = locomotion(speed, angle, directional, p.has_ball, m);
        let ease = if self.ready { (dt / LOCO_FADE).min(1.0) } else { 1.0 };
        for (w, t) in self.loco.iter_mut().zip(target) {
            *w += (t - *w) * ease;
        }
        let (mut stride, mut moving) = (0.0, 0.0);
        for c in Clip::ALL.iter().filter(|c| m.get(**c).speed > 0.0) {
            stride += self.loco[*c as usize] * m.stride(*c);
            moving += self.loco[*c as usize];
        }
        if moving > 1e-3 && stride > 1e-3 {
            self.phase = (self.phase + speed * dt / (stride / moving)).rem_euclid(1.0);
        }
        self.idle += dt;

        // The one-shot on top.
        let want = self.want(p, speed, dt, m);
        self.blend_shots(want, dt);

        // Into the nodes: the full-body one-shots take their share from locomotion; an upper-body
        // one takes its share of the upper body only (its legs are masked out).
        let mut mix = Mix { weights: [0.0; NODES], times: [0.0; NODES], ..default() };
        let full: f32 = self.shots.iter().filter(|s| !s.clip.upper()).map(|s| smooth(s.w)).sum();
        let scale = if full > 1.0 { 1.0 / full } else { 1.0 };
        let rest = 1.0 - full.min(1.0);
        for c in Clip::ALL {
            let i = c as usize;
            let meta = m.get(c);
            if meta.speed > 0.0 || c == Clip::Idle || c == Clip::IdleBall {
                mix.weights[i] = self.loco[i] * rest;
                mix.times[i] = if meta.speed > 0.0 { (self.phase + meta.sync).rem_euclid(1.0) * meta.duration } else { self.idle.rem_euclid(meta.duration.max(1e-3)) };
            }
        }
        for s in &self.shots {
            let i = s.clip as usize + s.slot * CLIPS;
            mix.weights[i] = if s.clip.upper() {
                let share = smooth(s.w) * CONTROL_SHARE;
                share / (1.0 - share)
            } else {
                smooth(s.w) * scale
            };
            mix.times[i] = s.t.clamp(0.0, m.get(s.clip).duration);
        }

        // The touches on top. The spin of a roulette or turn-out cut short goes back the short way.
        let spinning = p.dribble.is_some_and(|d| d.moving() && d.turn != FeintTurn::None);
        if !spinning {
            self.spin = (self.spin + 180.0).rem_euclid(360.0) - 180.0;
            self.spin -= self.spin * (dt / 0.1).min(1.0);
        }
        // The kick's turn fades out with its clip.
        let kick_w = self.shots.iter().filter(|s| s.key == self.kick_key).map(|s| smooth(s.w)).sum::<f32>();
        mix.yaw = self.spin + self.kick_turn * self.kick_share(m) * kick_w;
        let lean_max = if p.sprint_running { 0.06 } else if p.has_ball { 0.1 } else { 0.075 };
        let lean = (self.lateral / lean_max).clamp(-1.0, 1.0) * (speed / SPRINT_SPEED).min(1.0) * 14.0;
        self.roll += (lean - self.roll) * (8.0 * dt).min(1.0);
        mix.lean = self.roll;
        if !directional && speed > 0.5 {
            let in_turn = if p.turn.is_some() { 0.5 } else { 1.0 };
            mix.warp = -angle.clamp(-60.0, 60.0) * (speed / 2.0).min(1.0) * in_turn * rest;
        }
        let pitch = (1.0 - p.stamina / MAX_STAMINA).max(0.0) * 6.0 + if p.extra_active { 12.0 } else { 0.0 };
        self.pitch += (pitch - self.pitch) * (6.0 * dt).min(1.0);
        mix.pitch = self.pitch;

        self.prev_speed = speed;
        self.ready = true;
        self.last = mix.clone();
        mix
    }

    /// How much of the kick's turn toward its target the body has made: a third by the top of the
    /// backswing, most of it by the touch (body.rs).
    fn kick_share(&self, m: &Metas) -> f32 {
        let at = |clip: Clip, t: f32| {
            let meta = m.get(clip);
            let (windup, contact) = (meta.windup.unwrap_or(0.0), meta.contact.unwrap_or(0.4 * meta.duration));
            if t < windup { 0.3 * t / windup.max(1e-3) } else { 0.3 + 0.4 * ((t - windup) / (contact - windup).max(1e-3)).min(1.0) }
        };
        if let Some(k) = self.kick {
            at(k.clip, k.t)
        } else if let Some(f) = self.follow {
            0.7 + 0.15 * (f.since / 0.1).min(1.0)
        } else {
            0.85
        }
    }

    /// The one-shot for the player's state: a kick (and its follow-through), a fake shot, a
    /// dribble's move, a control, a turn or a stop, in that order.
    fn want(&mut self, p: &Player, speed: f32, dt: f32, m: &Metas) -> Option<Want> {
        if let Some(k) = p.kick.filter(|_| p.fake.is_none()) {
            self.follow = None;
            self.current = None;
            let aim = if k.kind != Kick::Shot && k.steer != Vec3::ZERO { yaw_of(k.steer) } else { k.aim.x.to_radians() };
            self.kick_turn = wrap(aim - p.facing).to_degrees().clamp(-150.0, 150.0);
            if self.kick.is_none_or(|t| k.elapsed < t.elapsed || t.kind != k.kind) {
                let clip = kick_clip(k.kind, self.kick_turn);
                let key = self.key();
                self.kick_key = key;
                self.kick = Some(KickTrack { key, kind: k.kind, clip, t: clip.start(), elapsed: 0.0 });
            }
            let track = self.kick.as_mut().unwrap();
            track.t = kick_time(track.t, &k, dt, m.get(track.clip));
            track.elapsed = k.elapsed;
            // Passes and lobs don't wind up while they charge (body.rs): they come in once let go.
            let weight = if k.kind.is_pass() && kick_contact(&k).is_none() { 0.0 } else { 1.0 };
            return Some(Want { clip: track.clip, key: track.key, t: track.t, weight, fade_in: 0.08, fade_out: 0.1 });
        }
        // The kick's state ends at its contact (unless faked): the follow-through plays on.
        if let Some(track) = self.kick.take()
            && p.fake.is_none()
        {
            let contact = m.get(track.clip).contact.unwrap_or(track.t);
            self.follow = Some(Follow { key: track.key, kind: track.kind, clip: track.clip, t: track.t.max(contact), since: 0.0 });
        }
        if let Some(f) = self.follow.as_mut() {
            f.t += dt;
            f.since += dt;
            // It holds while the kick slides you on, then fades, quicker running on out of it.
            if f.since <= follow_time(f.kind) || p.kick_sliding() {
                let out = if speed > 1.5 && !p.kick_sliding() { 0.15 } else { 0.25 };
                // A pass or lob is back in its stride soon after the touch (body.rs): only a shot
                // swings all the way through.
                let weight = if f.kind == Kick::Shot { 1.0 } else { 1.0 - 0.6 * (f.since / follow_time(f.kind)).min(1.0) };
                return Some(Want { clip: f.clip, key: f.key, t: f.t, weight, fade_in: 0.08, fade_out: out });
            }
            self.follow = None;
        }

        // A fake shot: the swing over the ball, held as you rise to act again.
        if let Some((t, lock)) = p.fake {
            let key = self.instance(Move::Fake, t);
            let meta = m.get(Clip::Strike);
            let (windup, contact) = (meta.windup.unwrap_or(0.0), meta.contact.unwrap_or(0.0));
            let clip_t = if t < FAKE_SHOT_SWING { windup * t / FAKE_SHOT_SWING } else { windup + (contact - windup) * 0.5 * ((t - FAKE_SHOT_SWING) / 0.1).min(1.0) };
            self.set_current(clip_t, t);
            let weight = ((lock - t) / 0.12).clamp(0.0, 1.0);
            return Some(Want { clip: Clip::Strike, key, t: clip_t, weight, fade_in: 0.05, fade_out: 0.12 });
        }

        // Dribbles: the roulette's spin and the forward touches; side ones and drag backs are the
        // directional jogs' (you keep facing forward).
        if let Some(d) = p.dribble.filter(|d| d.moving()) {
            match d.turn {
                FeintTurn::Roulette => {
                    let key = self.instance(Move::Spin, d.t);
                    self.spin = roulette_turn(d.t).rem_euclid(360.0);
                    let t = spin_time(d.t, m);
                    self.set_current(t, d.t);
                    return Some(Want { clip: Clip::Spin, key, t, weight: 1.0, fade_in: 0.05, fade_out: BLEND_OUT });
                }
                turn if d.heading == Heading::Forward => {
                    if let FeintTurn::TurnOut(side) = turn {
                        self.spin = -side * turn_out_angle(d.t);
                    }
                    let key = self.instance(Move::Touch, d.t);
                    let meta = m.get(Clip::Dribble);
                    let (start, at) = (Clip::Dribble.start(), meta.contact.unwrap_or(0.45));
                    let contact = dribble_contact(&d).max(1.0 / 60.0);
                    let t = if d.t < contact { start + (at - start) * d.t / contact } else { at + (d.t - contact) };
                    self.set_current(t, d.t);
                    return Some(Want { clip: Clip::Dribble, key, t, weight: 1.0, fade_in: 0.05, fade_out: BLEND_OUT });
                }
                _ => {}
            }
        }

        // A control: the chest cushion on the upper body, on the ball as it becomes yours.
        if let Some(t) = p.receiving {
            let key = self.instance(Move::Control, t);
            let clip_t = if t < CONTROL_SNAP { CONTROL_CATCH * t / CONTROL_SNAP } else { CONTROL_CATCH + (t - CONTROL_SNAP) };
            self.set_current(clip_t, t);
            return Some(Want { clip: Clip::ReceiveChest, key, t: clip_t, weight: 1.0, fade_in: 0.1, fade_out: 0.25 });
        }

        // A turn: the clip's plant and push-off on the game's brake and re-acceleration (the game
        // turns your facing; a smaller turn shows less of it).
        if let Some(tr) = p.turn.filter(|_| !p.locked && p.dash.is_none()) {
            let (u, brake, right, big) = tr.progress();
            let key = self.instance(Move::Turn, u);
            let t = turn_time(u, brake, m);
            self.set_current(t, u);
            let clip = if right { Clip::Turn180 } else { Clip::Turn180Left };
            return Some(Want { clip, key, t, weight: if big { 1.0 } else { 0.6 }, fade_in: 0.1, fade_out: 0.15 });
        }

        // A stop: letting go running, the clip's braking steps as you brake (its first half: the
        // rest is a skid into a deep crouch); pushing on out of it ends it, and so does standing.
        // Faster stops show more of it.
        let input = p.last_input != Vec3::ZERO;
        let stopping = matches!(self.current, Some((Move::Stop, ..)));
        let starts = !stopping && !input && !p.locked && p.dash.is_none() && p.dribble.is_none() && speed > 1.5 && speed <= self.prev_speed;
        if ((stopping && !input) || starts) && speed > 0.1 {
            if starts {
                self.stop_entry = speed;
            }
            let (key, last_t) = match self.current {
                Some((Move::Stop, key, t, _)) => (key, t),
                _ => (self.instance(Move::Stop, 0.0), 0.0),
            };
            let end = m.travel_at(Clip::RunStop, STOP_SHARE);
            let t = (end * (1.0 - speed / self.stop_entry.max(0.1)).clamp(0.0, 1.0)).max(last_t);
            self.set_current(t, 0.0);
            let weight = ((self.stop_entry - 1.5) / (SPRINT_SPEED - 1.5)).clamp(0.3, 0.8);
            return Some(Want { clip: Clip::RunStop, key, t, weight, fade_in: 0.15, fade_out: 0.3 });
        }
        self.current = None;
        None
    }

    /// The one-shots' weights: the wanted one in, the others out, each at its own rate.
    fn blend_shots(&mut self, want: Option<Want>, dt: f32) {
        match want {
            Some(w) if self.shots.iter().any(|s| s.live && s.key == w.key) => {
                let s = self.shots.iter_mut().find(|s| s.live && s.key == w.key).unwrap();
                s.t = w.t;
                s.target = w.weight;
                s.fade_out = w.fade_out;
            }
            Some(w) => {
                for s in &mut self.shots {
                    s.live = false;
                }
                // A clip already fading out keeps its node: this one takes the other.
                let slot = if self.shots.iter().any(|s| s.clip == w.clip && s.slot == 0) { 1 } else { 0 };
                self.shots.retain(|s| !(s.clip == w.clip && s.slot == slot));
                self.shots.push(Shot { clip: w.clip, slot, key: w.key, t: w.t, w: 0.0, target: w.weight, live: true, fade_in: w.fade_in, fade_out: w.fade_out });
            }
            None => {
                for s in &mut self.shots {
                    s.live = false;
                }
            }
        }
        for s in &mut self.shots {
            let target = if s.live { s.target } else { 0.0 };
            s.w = approach(s.w, target, s.fade_in, s.fade_out, dt);
            if !s.live {
                s.t += dt;
            }
        }
        self.shots.retain(|s| s.live || s.w > 0.0);
    }
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

// ---- Bevy side ----

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<ToonMaterial>::default(),
            MaterialPlugin::<OutlineMaterial> { prepass_enabled: false, shadows_enabled: false, ..default() },
        ))
        .init_resource::<MixState>()
            .add_systems(Startup, cheats_from_env)
            .add_systems(Update, (load, bind, dress, paint, show).chain())
            .add_systems(PostUpdate, (drive.before(Animation), adjust.after(Animation).before(TransformSystem::TransformPropagate)));
    }
}

/// `RE_CHEATS=mixamo,debug,...` turns cheats on at the start (for scripted runs).
fn cheats_from_env(mut cheats: ResMut<Cheats>) {
    let Ok(list) = std::env::var("RE_CHEATS") else { return };
    for name in list.split(',').map(str::trim) {
        let cheat = match name {
            "mixamo" => Cheat::Mixamo,
            "stamina" => Cheat::InfiniteStamina,
            "effort" => Cheat::InfiniteEffort,
            "trail" => Cheat::BallTrail,
            "path" => Cheat::BallPath,
            "debug" => Cheat::DebugInfo,
            "slow" => Cheat::SlowMotion,
            "zones" => Cheat::ShowZones,
            _ => continue,
        };
        if !cheats.on(cheat) {
            cheats.toggle(cheat);
        }
    }
}

/// The character, once the cheat first turns on.
#[derive(Resource)]
struct Character {
    gltf: Handle<Gltf>,
    metas: Metas,
    root: Entity,
}

#[derive(Component)]
struct CharacterRoot;

/// A team's kit: the character's shirt, shorts and socks take these colours. Set another on the
/// character's root for the other team (`RE_KIT=red` starts in the red one).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Kit {
    pub shirt: Color,
    pub shorts: Color,
    pub socks: Color,
}

impl Kit {
    /// The prototype's kit (Kev): royal blue shirt, navy shorts, white socks.
    pub const BLUE: Kit = Kit { shirt: Color::srgb(0.10, 0.30, 0.85), shorts: Color::srgb(0.05, 0.10, 0.32), socks: Color::srgb(0.92, 0.92, 0.90) };
    /// The scoreboard's other side.
    pub const RED: Kit = Kit { shirt: Color::srgb(0.85, 0.15, 0.15), shorts: Color::srgb(0.95, 0.95, 0.95), socks: Color::srgb(0.85, 0.15, 0.15) };
}

/// The toon character's regions, by their material's name in the glb.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum Part {
    Skin,
    Hair,
    Shirt,
    Shorts,
    Socks,
    Boots,
    Other,
}

impl Part {
    fn of(material: &str) -> Part {
        match material {
            "skin" => Part::Skin,
            "hair" => Part::Hair,
            "shirt" => Part::Shirt,
            "shorts" => Part::Shorts,
            "socks" => Part::Socks,
            "boots" => Part::Boots,
            _ => Part::Other,
        }
    }

    /// The kit's colour for the part; the others keep the glb's.
    fn colour(self, kit: &Kit) -> Option<Color> {
        match self {
            Part::Shirt => Some(kit.shirt),
            Part::Shorts => Some(kit.shorts),
            Part::Socks => Some(kit.socks),
            _ => None,
        }
    }
}

/// The cel shading on top of the glb's flat colours (assets/shaders/toon.wgsl).
pub type ToonMaterial = ExtendedMaterial<StandardMaterial, Toon>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Toon {
    /// The shade band's share of the sun, the middle band's, the N.L where the lit band starts,
    /// and how soft the steps are.
    #[uniform(100)]
    bands: Vec4,
    /// The rim's colour (times the sun) and how far in from the silhouette it reaches.
    #[uniform(101)]
    rim: Vec4,
}

impl Default for Toon {
    fn default() -> Self {
        Self { bands: Vec4::new(0.3, 0.68, 0.4, 0.05), rim: Vec4::new(0.35, 0.35, 0.38, 0.32) }
    }
}

impl MaterialExtension for Toon {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wgsl".into()
    }
}

/// The character's outline: an inverted hull (assets/shaders/outline.wgsl).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct OutlineMaterial {
    #[uniform(0)]
    colour: LinearRgba,
    /// Metres out per metre from the camera, and the distances that's clamped to.
    #[uniform(1)]
    width: Vec4,
}

impl Default for OutlineMaterial {
    fn default() -> Self {
        Self { colour: Color::srgb(0.04, 0.04, 0.06).into(), width: Vec4::new(0.0024, 1.5, 12.0, 0.0) }
    }
}

impl Material for OutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/outline.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/outline.wgsl".into()
    }

    fn specialize(_: &MaterialPipeline<Self>, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        // The back faces only: the hull shows just past the silhouette.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// On the character's animation player: its nodes and the bones the touches turn.
#[derive(Component)]
struct Rig {
    nodes: [Option<AnimationNodeIndex>; NODES],
    hips: Entity,
    spine: Entity,
    chest: Entity,
}

#[derive(Resource, Default)]
struct MixState {
    mixer: Mixer,
    mix: Mix,
    tried: bool,
}

fn load(
    mut commands: Commands,
    cheats: Res<Cheats>,
    mut state: ResMut<MixState>,
    character: Option<Res<Character>>,
    assets: Res<AssetServer>,
    player: Single<Entity, With<Player>>,
) {
    if character.is_some() || state.tried || !cheats.on(Cheat::Mixamo) {
        return;
    }
    state.tried = true;
    let base = FileAssetReader::get_base_path().join("assets");
    let metas = std::fs::read_to_string(base.join(SIDECAR))
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str::<Sidecar>(&s).map_err(|e| e.to_string()))
        .and_then(Metas::from_sidecar);
    let metas = match metas {
        Ok(m) if base.join(GLB).exists() => m,
        Ok(_) => return warn!("No {GLB}: run tools/mixamo_to_glb.py in Blender to make it"),
        Err(e) => return warn!("No character: {SIDECAR}: {e} (run tools/mixamo_to_glb.py in Blender)"),
    };
    let kit = if std::env::var("RE_KIT").is_ok_and(|k| k == "red") { Kit::RED } else { Kit::BLUE };
    let root = commands
        .spawn((CharacterRoot, kit, SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(GLB))), Transform::from_xyz(0.0, FEET, 0.0), Visibility::Hidden))
        .id();
    commands.entity(*player).add_child(root);
    commands.insert_resource(Character { gltf: assets.load(GLB), metas, root });
}

/// Once the scene is in: the graph (one node per clip and slot under the root, the legs a mask
/// group the upper-body clips leave out), every node playing at weight 0 with its time set by
/// hand.
fn bind(
    mut commands: Commands,
    character: Option<Res<Character>>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Without<Rig>>,
    children: Query<&Children>,
    names: Query<(&Name, Option<&AnimationTarget>)>,
) {
    let Some(character) = character else { return };
    let Some(gltf) = gltfs.get(&character.gltf) else { return };
    for (entity, mut player) in &mut players {
        if !children.iter_descendants(character.root).any(|e| e == entity) {
            continue;
        }
        let bone = |name: &str| children.iter_descendants(entity).find(|e| names.get(*e).is_ok_and(|(n, _)| n.as_str().rsplit(':').next() == Some(name)));
        let (Some(hips), Some(spine), Some(chest)) = (bone("Hips"), bone("Spine"), bone("Spine1")) else {
            warn!("The character has no Hips/Spine/Spine1 bones");
            continue;
        };
        let mut graph = AnimationGraph::new();
        const LEGS: [&str; 11] = ["Hips", "LeftUpLeg", "LeftLeg", "LeftFoot", "LeftToeBase", "LeftToe_End", "RightUpLeg", "RightLeg", "RightFoot", "RightToeBase", "RightToe_End"];
        for e in children.iter_descendants(entity) {
            if let Ok((name, Some(target))) = names.get(e)
                && LEGS.contains(&name.as_str().rsplit(':').next().unwrap_or(""))
            {
                graph.add_target_to_mask_group(target.id, 0);
            }
        }
        let mut nodes = [None; NODES];
        for (i, c) in Clip::ALL.iter().enumerate() {
            let Some(clip) = gltf.named_animations.get(c.name()) else {
                warn!("The character has no clip {}", c.name());
                continue;
            };
            for slot in 0..2 {
                let mask = if c.upper() { 1 } else { 0 };
                nodes[i + slot * CLIPS] = Some(graph.add_clip_with_mask(clip.clone(), mask, 1.0, graph.root));
            }
        }
        for node in nodes.iter().flatten() {
            player.play(*node).set_weight(0.0).pause();
        }
        commands.entity(entity).insert((AnimationGraphHandle(graphs.add(graph)), Rig { nodes, hips, spine, chest }));
    }
}

/// A mesh as the glb's scene spawns it.
type GltfMesh = (&'static Mesh3d, &'static MeshMaterial3d<StandardMaterial>, Option<&'static GltfMaterialName>, Option<&'static SkinnedMesh>);

/// Once the scene is in, its meshes: the flat-coloured toon character (tools/mixamo_to_glb.py's
/// default) gets the cel shading, its kit's colours and an outline; the realistic one keeps its
/// textures, with the hair cut out rather than blended. The skinned meshes are never culled (their
/// bounds are the bind pose's).
#[allow(clippy::too_many_arguments)]
fn dress(
    mut commands: Commands,
    character: Option<Res<Character>>,
    kits: Query<&Kit>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut toons: ResMut<Assets<ToonMaterial>>,
    mut outlines: ResMut<Assets<OutlineMaterial>>,
    children: Query<&Children>,
    meshes: Query<GltfMesh, Without<Part>>,
) {
    let Some(character) = character else { return };
    let kit = kits.get(character.root).copied().unwrap_or(Kit::BLUE);
    let mut parts: Vec<(Part, Handle<ToonMaterial>)> = vec![];
    let mut outline = None;
    for e in children.iter_descendants(character.root) {
        let Ok((mesh, material, name, skinned)) = meshes.get(e) else { continue };
        let Some(base) = standard.get_mut(&material.0) else { continue };
        if skinned.is_some() {
            commands.entity(e).insert(NoFrustumCulling);
        }
        let part = Part::of(name.map_or("", |n| n.0.as_str()));
        if base.base_color_texture.is_some() || part == Part::Other {
            if matches!(base.alpha_mode, AlphaMode::Blend) {
                base.alpha_mode = AlphaMode::Mask(0.4);
                base.double_sided = true;
                base.cull_mode = None;
            }
            commands.entity(e).insert(Part::Other);
            continue;
        }
        let toon = match parts.iter().find(|(p, _)| *p == part) {
            Some((_, h)) => h.clone(),
            None => {
                let mut base = base.clone();
                base.base_color = part.colour(&kit).unwrap_or(base.base_color);
                let h = toons.add(ToonMaterial { base, extension: Toon::default() });
                parts.push((part, h.clone()));
                h
            }
        };
        commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert((part, MeshMaterial3d(toon)));
        if let Some(skinned) = skinned {
            let outline = outline.get_or_insert_with(|| outlines.add(OutlineMaterial::default())).clone();
            commands.entity(e).with_child((mesh.clone(), skinned.clone(), MeshMaterial3d(outline), NotShadowCaster, NoFrustumCulling));
        }
    }
}

/// A kit changed (another team's): its parts take the new colours.
fn paint(kits: Query<(Entity, &Kit), Changed<Kit>>, children: Query<&Children>, parts: Query<(&Part, &MeshMaterial3d<ToonMaterial>)>, mut toons: ResMut<Assets<ToonMaterial>>) {
    for (root, kit) in &kits {
        for e in children.iter_descendants(root) {
            if let Ok((part, material)) = parts.get(e)
                && let Some(colour) = part.colour(kit)
                && let Some(m) = toons.get_mut(&material.0)
            {
                m.base.base_color = colour;
            }
        }
    }
}

/// The cheat shows the character and hides the procedural body (which keeps animating).
fn show(cheats: Res<Cheats>, character: Option<Res<Character>>, mut roots: Query<&mut Visibility, With<CharacterRoot>>, mut joints: Query<&mut Visibility, (With<Joint>, Without<CharacterRoot>)>, rigs: Query<(), With<Rig>>) {
    let on = cheats.on(Cheat::Mixamo) && character.is_some() && !rigs.is_empty();
    for mut v in &mut roots {
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
    for mut v in &mut joints {
        v.set_if_neq(if on { Visibility::Hidden } else { Visibility::Inherited });
    }
}

/// Each frame: the mixer's weights and times into the animation player, its turn and lean onto the
/// character's root.
fn drive(
    time: Res<Time>,
    cheats: Res<Cheats>,
    character: Option<Res<Character>>,
    mut state: ResMut<MixState>,
    player: Single<(&Player, &Transform)>,
    mut rigs: Query<(&Rig, &mut AnimationPlayer)>,
    mut roots: Query<&mut Transform, (With<CharacterRoot>, Without<Player>)>,
) {
    let Some(character) = character else { return };
    if !cheats.on(Cheat::Mixamo) {
        // Hidden: nothing to evaluate.
        if cheats.is_changed() {
            for (_, mut anim) in &mut rigs {
                for (_, active) in anim.playing_animations_mut() {
                    active.set_weight(0.0);
                }
            }
        }
        return;
    }
    let (p, tf) = *player;
    // The kick's freeze holds the pose too.
    let dt = if p.freeze > 0.0 { 0.0 } else { time.delta_secs() };
    let state = &mut *state;
    state.mix = state.mixer.update(p, tf.rotation, dt, &character.metas);
    // For looking at a clip: `RE_CLIP=name` plays it alone, over and over from the start.
    if let Ok(name) = std::env::var("RE_CLIP")
        && let Some(c) = Clip::ALL.into_iter().find(|c| c.name() == name)
    {
        state.mix = Mix { weights: [0.0; NODES], times: [0.0; NODES], ..default() };
        state.mix.weights[c as usize] = 1.0;
        state.mix.times[c as usize] = time.elapsed_secs().rem_euclid(character.metas.get(c).duration.max(1e-3));
    }
    let mix = &state.mix;
    for (rig, mut anim) in &mut rigs {
        for (i, node) in rig.nodes.iter().enumerate() {
            if let Some(node) = node
                && let Some(active) = anim.animation_mut(*node)
            {
                active.set_weight(mix.weights[i]).set_seek_time(mix.times[i]);
            }
        }
    }
    for mut root in &mut roots {
        root.rotation = Quat::from_rotation_y(mix.yaw.to_radians()) * Quat::from_rotation_z(-mix.lean.to_radians());
    }
}

/// After the clips are applied: the lower body turned toward the velocity and the trunk back
/// toward the facing (the procedural body's orientation warping), the trunk's lean forward.
fn adjust(cheats: Res<Cheats>, state: Res<MixState>, rigs: Query<&Rig>, mut bones: Query<&mut Transform, Without<CharacterRoot>>) {
    if !cheats.on(Cheat::Mixamo) {
        return;
    }
    let mix = &state.mix;
    for rig in &rigs {
        let Ok([mut hips, mut spine, mut chest]) = bones.get_many_mut([rig.hips, rig.spine, rig.chest]) else { continue };
        let warp = mix.warp.to_radians();
        // The hips' parent (the armature) is the character's root: its frame is the character's.
        hips.rotation = Quat::from_rotation_y(warp) * hips.rotation;
        let above = hips.rotation;
        spine.rotation = above.inverse() * Quat::from_rotation_y(-0.6 * warp) * above * spine.rotation;
        let above = above * spine.rotation;
        chest.rotation = above.inverse() * Quat::from_rotation_y(-0.4 * warp) * Quat::from_rotation_x(-mix.pitch.to_radians()) * above * chest.rotation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::DribbleState;

    /// The sidecar's numbers as tools/mixamo_to_glb.py made them (durations, speeds, marks), with
    /// straight-line turns and travel for the one-shots.
    fn metas() -> Metas {
        let fps = 30.0;
        let clip = |duration: f32, speed: f32, sync: f32| ClipMeta { duration, speed, sync, ..default() };
        let kick = |duration: f32, windup: f32, contact: f32| ClipMeta { duration, windup: Some(windup), contact: Some(contact), ..default() };
        let turning = |duration: f32, turn: f32, ahead: f32| {
            let n = (duration * fps).round() as usize + 1;
            let root = (0..n).map(|k| k as f32 / (n - 1) as f32).map(|u| [0.0, ahead * u, turn * u]).collect();
            ClipMeta { duration, root, ..default() }
        };
        let clips = Clip::ALL
            .iter()
            .map(|c| match c {
                Clip::Idle => clip(10.57, 0.0, 0.73),
                Clip::IdleBall => clip(6.2, 0.0, 0.56),
                Clip::Walk => clip(1.13, 1.49, 0.21),
                Clip::WalkBack => clip(1.2, 1.01, 0.86),
                Clip::Run => clip(0.8, 2.45, 0.83),
                Clip::RunBack => clip(0.77, 1.9, 0.17),
                Clip::Sprint => clip(0.53, 5.07, 0.88),
                Clip::Jog => clip(0.83, 2.32, 0.84),
                Clip::JogBack => clip(0.8, 2.13, 0.12),
                Clip::StrafeLeft => clip(0.7, 2.7, 0.19),
                Clip::StrafeRight => clip(0.73, 2.15, 0.64),
                Clip::DiagLeft => clip(0.83, 2.44, 0.36),
                Clip::DiagRight => clip(0.8, 2.59, 0.38),
                Clip::BackDiagLeft => clip(0.83, 2.29, 0.84),
                Clip::BackDiagRight => clip(0.73, 2.06, 0.64),
                Clip::Pass => kick(1.6, 0.33, 0.46),
                Clip::Chip => kick(1.67, 0.23, 0.47),
                Clip::Strike | Clip::StrikeLeft => kick(1.3, 0.27, 0.41),
                Clip::PenaltyKick => kick(1.5, 0.6, 0.74),
                Clip::Dribble => kick(1.73, 0.3, 0.45),
                Clip::Spin => turning(1.27, 360.0, 3.38),
                Clip::Turn180 | Clip::Turn180Left => turning(0.67, 180.0, 0.0),
                Clip::RunStop => turning(1.67, 0.0, 3.06),
                _ => clip(1.4, 0.0, 0.0),
            })
            .collect();
        Metas { clips, fps }
    }

    fn sum(w: &[f32; CLIPS]) -> f32 {
        w.iter().sum()
    }

    fn kick(kind: Kick) -> KickState {
        KickState { kind, elapsed: 0.0, held: 0.0, pre: 0.0, released: false, aim: Vec2::ZERO, eye: Vec3::ZERO, entry: Vec3::ZERO, from_dribble: false, sprint: false, steer: Vec3::ZERO }
    }

    #[test]
    fn going_ahead_the_cycles_follow_the_speed() {
        let m = metas();
        let at = |speed: f32| locomotion(speed, 0.0, false, false, &m);
        assert_eq!(at(0.0)[Clip::Idle as usize], 1.0);
        // The game's run (5 m/s) is Mixamo's fast run; the sprint is too, faster.
        assert!(at(RUN_SPEED)[Clip::Sprint as usize] > 0.95);
        assert_eq!(at(SPRINT_SPEED)[Clip::Sprint as usize], 1.0);
        // A walk-speed input walks; between run and walk both share.
        assert!(at(1.49)[Clip::Walk as usize] > 0.99);
        let mid = at(2.0);
        assert!(mid[Clip::Walk as usize] > 0.0 && mid[Clip::Run as usize] > 0.0);
        for speed in [0.0, 0.7, 2.0, 3.3, 6.0, 9.5] {
            assert!((sum(&at(speed)) - 1.0).abs() < 1e-5);
        }
        // With the ball, standing is the ball's idle.
        assert_eq!(locomotion(0.0, 0.0, false, true, &m)[Clip::IdleBall as usize], 1.0);
    }

    #[test]
    fn lock_move_blends_the_two_nearest_jogs() {
        let m = metas();
        let at = |angle: f32| locomotion(LOCK_SPEED, angle, true, false, &m);
        assert_eq!(at(0.0)[Clip::Jog as usize], 1.0);
        assert_eq!(at(90.0)[Clip::StrafeRight as usize], 1.0);
        assert_eq!(at(-90.0)[Clip::StrafeLeft as usize], 1.0);
        assert_eq!(at(-45.0)[Clip::DiagLeft as usize], 1.0);
        assert!((at(180.0)[Clip::JogBack as usize] - 1.0).abs() < 1e-5);
        let w = at(160.0);
        assert!(w[Clip::JogBack as usize] > w[Clip::BackDiagRight as usize] && w[Clip::BackDiagRight as usize] > 0.0);
        assert!((sum(&w) - 1.0).abs() < 1e-5);
        // Barely moving, mostly idle.
        assert!(locomotion(0.2, 90.0, true, false, &m)[Clip::Idle as usize] > 0.8);
    }

    #[test]
    fn the_feet_keep_pace_with_the_ground() {
        // Running steadily, a loop of the blended cycles covers its stride.
        let m = metas();
        let mut mixer = Mixer::default();
        let p = Player { vel: Vec3::NEG_Z * RUN_SPEED, last_input: Vec3::NEG_Z, ..default() };
        for _ in 0..120 {
            mixer.update(&p, Quat::IDENTITY, 1.0 / 60.0, &m);
        }
        let before = mixer.phase;
        mixer.update(&p, Quat::IDENTITY, 1.0 / 60.0, &m);
        let advanced = (mixer.phase - before).rem_euclid(1.0);
        let w = locomotion(RUN_SPEED, 0.0, false, false, &m);
        let stride: f32 = Clip::ALL.iter().map(|c| w[*c as usize] * m.stride(*c)).sum();
        assert!((advanced * stride - RUN_SPEED / 60.0).abs() < 1e-3, "{advanced} of {stride} m");
    }

    #[test]
    fn a_tapped_pass_meets_the_ball_on_the_game_s_contact() {
        let m = metas();
        let meta = m.get(Clip::Pass);
        let mut k = kick(Kick::Pass);
        let dt = 1.0 / 60.0;
        let mut t = Clip::Pass.start();
        let mut last = t;
        // Tapped: let go after 3 frames.
        while k.elapsed + dt < contact_time(&k) + 1e-4 {
            k.elapsed += dt;
            if k.elapsed < 0.05 {
                k.held = k.elapsed;
            } else {
                k.released = true;
            }
            t = kick_time(t, &k, dt, meta);
            assert!(t >= last, "the clip never runs back");
            last = t;
        }
        assert!((t - meta.contact.unwrap()).abs() < 1e-3, "at the contact the clip is at {t}");
    }

    #[test]
    fn a_charged_shot_holds_its_backswing_then_strikes_at_the_release() {
        let m = metas();
        let meta = m.get(Clip::Strike);
        let (windup, contact) = (meta.windup.unwrap(), meta.contact.unwrap());
        let mut k = kick(Kick::Shot);
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        // Held for a second: the leg waits at the top of the backswing.
        for _ in 0..60 {
            k.elapsed += dt;
            k.held = k.elapsed;
            t = kick_time(t, &k, dt, meta);
        }
        assert!((t - windup).abs() < 1e-5);
        // Let go: the strike plays out to the contact by the release plus a strike's time.
        k.released = true;
        let at = kick_contact(&k).unwrap();
        assert!((at - (k.held + STRIKE)).abs() < 1e-5);
        while k.elapsed + dt <= at + 1e-4 {
            k.elapsed += dt;
            t = kick_time(t, &k, dt, meta);
        }
        assert!((t - contact).abs() < 1e-3, "{t}");
    }

    #[test]
    fn a_shot_with_time_to_spare_plays_at_its_own_speed() {
        // A shot let go at once at full power: its contact (32 frames) leaves time for the clip's
        // own wind-up and strike, so it plays at its natural speed, holding at the backswing.
        let m = metas();
        let meta = m.get(Clip::Strike);
        let mut k = kick(Kick::Shot);
        k.held = SHOT_BUILDUP;
        k.released = true;
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        let mut steps = vec![];
        while k.elapsed + dt <= kick_contact(&k).unwrap() + 1e-4 {
            k.elapsed += dt;
            let next = kick_time(t, &k, dt, meta);
            steps.push(next - t);
            t = next;
        }
        assert!(steps.iter().all(|s| *s <= dt + 1e-5), "never faster than the clip's own speed");
        assert!((t - meta.contact.unwrap()).abs() < 1e-3);
    }

    #[test]
    fn the_kit_colours_the_shirt_shorts_and_socks_only() {
        assert_eq!(Part::of("shirt").colour(&Kit::RED), Some(Kit::RED.shirt));
        assert_eq!(Part::of("shorts").colour(&Kit::BLUE), Some(Kit::BLUE.shorts));
        assert_eq!(Part::of("socks").colour(&Kit::RED), Some(Kit::RED.socks));
        for part in ["skin", "hair", "boots", "Ch38_body"] {
            assert_eq!(Part::of(part).colour(&Kit::RED), None, "{part}");
        }
    }

    #[test]
    fn kicks_pick_their_clips() {
        assert_eq!(kick_clip(Kick::Shot, 0.0), Clip::Strike);
        assert_eq!(kick_clip(Kick::Shot, -60.0), Clip::StrikeLeft);
        assert_eq!(kick_clip(Kick::Pass, 0.0), Clip::Pass);
        assert_eq!(kick_clip(Kick::ShortLob, 0.0), Clip::Chip);
        assert_eq!(kick_clip(Kick::PushBall, 0.0), Clip::Dribble);
    }

    #[test]
    fn a_pass_plays_through_its_follow_through_and_hands_back_to_the_run() {
        let m = metas();
        let mut mixer = Mixer::default();
        let dt = 1.0 / 60.0;
        let mut p = Player { vel: Vec3::NEG_Z * RUN_SPEED, last_input: Vec3::NEG_Z, ..default() };
        mixer.update(&p, Quat::IDENTITY, dt, &m);
        let mut k = kick(Kick::ShortPass);
        p.kick = Some(k);
        for _ in 0..(contact_time(&k) * 60.0) as usize {
            k.elapsed += dt;
            p.kick = Some(k);
            mixer.update(&p, Quat::IDENTITY, dt, &m);
        }
        assert_eq!(mixer.update(&p, Quat::IDENTITY, dt, &m).top(), Clip::Pass);
        // The touch: the follow-through holds, then the run comes back.
        p.kick = None;
        for _ in 0..10 {
            assert_eq!(mixer.update(&p, Quat::IDENTITY, dt, &m).top(), Clip::Pass);
        }
        for _ in 0..40 {
            mixer.update(&p, Quat::IDENTITY, dt, &m);
        }
        let mix = mixer.update(&p, Quat::IDENTITY, dt, &m);
        assert_eq!(mix.top(), Clip::Sprint);
        assert_eq!(mix.weight(Clip::Pass), 0.0);
    }

    #[test]
    fn the_roulette_spins_with_the_game_s_turn() {
        let m = metas();
        let mut mixer = Mixer::default();
        let dt = 1.0 / 60.0;
        let mut d = DribbleState { t: 0.0, feint: true, exhausted: false, heading: Heading::Forward, dir: Vec3::NEG_Z, facing: 0.0, cut: 0.0, turn: FeintTurn::Roulette };
        let mut p = Player { has_ball: true, ..default() };
        let mut clip_t = 0.0;
        while d.t + dt < d.free() {
            d.t += dt;
            p.dribble = Some(d);
            let mix = mixer.update(&p, Quat::IDENTITY, dt, &m);
            assert!((mix.yaw - roulette_turn(d.t).rem_euclid(360.0)).abs() < 1e-3);
            let t = mix.times[Clip::Spin as usize] + mix.times[Clip::Spin as usize + CLIPS];
            assert!(t + 1e-5 >= clip_t, "the spin never runs back");
            clip_t = t;
        }
        // Halfway round in the game is halfway round in the clip.
        let half = ROULETTE_TURN.1;
        assert!((spin_time(half, &m) - m.turn_at(Clip::Spin, 180.0)).abs() < 1e-3);
    }

    #[test]
    fn a_turn_pivots_where_the_brake_ends() {
        let m = metas();
        assert!((turn_time(0.4, 0.4, &m) - m.turn_at(Clip::Turn180, 90.0)).abs() < 1e-5);
        assert_eq!(turn_time(0.0, 0.4, &m), 0.0);
        assert!((turn_time(1.0, 0.4, &m) - m.turn_at(Clip::Turn180, 175.0)).abs() < 1e-5);
    }

    #[test]
    fn controls_lock_move_and_stops_pick_their_clips() {
        let m = metas();
        let dt = 1.0 / 60.0;
        let run = |p: &Player, frames: usize| {
            let mut mixer = Mixer::default();
            let mut mix = Mix::default();
            for _ in 0..frames {
                mix = mixer.update(p, Quat::IDENTITY, dt, &m);
            }
            mix
        };
        // A control: the chest's on top, the legs keep running.
        let p = Player { vel: Vec3::NEG_Z * BALL_RUN_SPEED, receiving: Some(0.1), last_input: Vec3::NEG_Z, ..default() };
        let mix = run(&p, 30);
        assert!(mix.weight(Clip::ReceiveChest) > 1.0);
        assert!(mix.weight(Clip::Sprint) > 0.5);
        // Lock move to the right: the right strafe.
        let p = Player { vel: Vec3::X * LOCK_SPEED, locked: true, last_input: Vec3::X, ..default() };
        assert_eq!(run(&p, 30).top(), Clip::StrafeRight);
        // Free move but sideways (a side dribble): the strafe too.
        let p = Player { vel: Vec3::NEG_X * 4.0, last_input: Vec3::NEG_X, ..default() };
        assert_eq!(run(&p, 30).top(), Clip::StrafeLeft);
        // Letting go at a run, slowing: the stop.
        let mut mixer = Mixer::default();
        let mut p = Player { vel: Vec3::NEG_Z * RUN_SPEED, last_input: Vec3::NEG_Z, ..default() };
        mixer.update(&p, Quat::IDENTITY, dt, &m);
        p.last_input = Vec3::ZERO;
        let mut mix = Mix::default();
        for i in 0..20 {
            p.vel = Vec3::NEG_Z * (RUN_SPEED - i as f32 * 0.2);
            mix = mixer.update(&p, Quat::IDENTITY, dt, &m);
        }
        assert_eq!(mix.top(), Clip::RunStop);
    }
}
