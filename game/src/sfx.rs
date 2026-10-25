//! Sound effects. Every sound is synthesised here at startup from noise, filters and decaying
//! sines: no audio files. Which sound plays when follows the game's trigger rules (notes/sfx.md);
//! the sounds themselves are ours.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, Source, Volume};
use bevy::prelude::*;

use crate::ball::{self, Ball, GoalScored};
use crate::player::{self, Kick, KickState, Player};
use crate::tuning::*;

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Synth>()
            .init_resource::<SfxSettings>()
            .add_systems(Startup, build_bank)
            .add_systems(Update, (follow_volumes, listen.after(ball::ball_physics), reap));
    }
}

/// The Audio tab's global and sound-effect volumes (0-100, saved with the bindings) drive these.
fn follow_volumes(bindings: Res<crate::input::Bindings>, mut settings: ResMut<SfxSettings>) {
    let volume = bindings.global_volume / 100.0 * bindings.sfx_volume / 100.0;
    if settings.volume != volume {
        settings.volume = volume;
        settings.on = volume > 0.0;
    }
}

/// On/off and the master volume (0..1), for the settings menu to drive.
#[derive(Resource)]
pub struct SfxSettings {
    pub on: bool,
    pub volume: f32,
}

impl Default for SfxSettings {
    fn default() -> Self {
        Self { on: true, volume: 0.8 }
    }
}

// ---- What plays when ----

/// A shot's power tier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Power {
    Low,
    Mid,
    High,
}

/// A shot's spin, as Kev groups it: none or side spin, topspin (it dips), downspin (it floats).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Spin {
    Flat,
    Top,
    Down,
}

/// Where the ball meets you on a control.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Height {
    Foot,
    Knee,
    Chest,
}

/// Footstep pace: the game's Walk, Run and Rush.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Pace {
    Walk,
    Run,
    Rush,
}

/// How hard the ball lands.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hit {
    Light,
    Medium,
    Strong,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sound {
    Shot(Power, Spin),
    Pass { max: bool },
    Lob { max: bool },
    PushBall,
    /// A touch running with the ball.
    Touch,
    FakeShot,
    Control(Height),
    Dribble { tired: bool },
    Dash,
    EffortOn,
    EffortOff,
    Step(Pace),
    Bounce(Hit),
    Wall,
    Post,
    Net,
    Goal,
    /// The crowd's gasp at a shot off the frame.
    Ooh,
}

impl Sound {
    fn all() -> Vec<Sound> {
        let mut all = Vec::new();
        for power in [Power::Low, Power::Mid, Power::High] {
            all.extend([Spin::Flat, Spin::Top, Spin::Down].map(|spin| Sound::Shot(power, spin)));
        }
        all.extend([Sound::Pass { max: false }, Sound::Pass { max: true }, Sound::Lob { max: false }, Sound::Lob { max: true }]);
        all.extend([Sound::PushBall, Sound::Touch, Sound::FakeShot]);
        all.extend([Height::Foot, Height::Knee, Height::Chest].map(Sound::Control));
        all.extend([Sound::Dribble { tired: false }, Sound::Dribble { tired: true }, Sound::Dash, Sound::EffortOn, Sound::EffortOff]);
        all.extend([Pace::Walk, Pace::Run, Pace::Rush].map(Sound::Step));
        all.extend([Hit::Light, Hit::Medium, Hit::Strong].map(Sound::Bounce));
        all.extend([Sound::Wall, Sound::Post, Sound::Net, Sound::Goal, Sound::Ooh]);
        all
    }

    /// How loud it plays at full strength (every sound is rendered to the same peak).
    fn level(self) -> f32 {
        match self {
            Sound::Shot(Power::Low, _) => 0.55,
            Sound::Shot(Power::Mid, _) => 0.75,
            Sound::Shot(Power::High, _) => 0.95,
            Sound::Pass { max } | Sound::Lob { max } => if max { 0.75 } else { 0.5 },
            Sound::PushBall => 0.4,
            Sound::Touch => 0.18,
            Sound::FakeShot => 0.3,
            Sound::Control(_) => 0.45,
            Sound::Dribble { .. } => 0.35,
            Sound::Dash => 0.4,
            Sound::EffortOn => 0.4,
            Sound::EffortOff => 0.25,
            Sound::Step(_) => 0.15,
            Sound::Bounce(Hit::Light) => 0.25,
            Sound::Bounce(Hit::Medium) => 0.4,
            Sound::Bounce(Hit::Strong) => 0.55,
            Sound::Wall => 0.55,
            Sound::Post => 0.7,
            Sound::Net => 0.45,
            Sound::Goal => 0.7,
            Sound::Ooh => 0.5,
        }
    }

    /// Repeated sounds play a little higher or lower each time, so they don't sound identical.
    fn varies(self) -> bool {
        matches!(self, Sound::Step(_) | Sound::Touch | Sound::Bounce(_) | Sound::PushBall | Sound::Control(_))
    }
}

/// The power shot's line, as the impact visuals draw it (fx.rs).
const POWER_SHOT_SPEED: f32 = 34.82;
/// Less spin than this, on top of the shot's built-in lift, is no spin.
const SPIN_MIN: f32 = 40.0;
/// Falling slower than this onto the grass makes no sound.
const BOUNCE_MIN: f32 = 1.0;
/// A surface that just sounded stays quiet this long, so a ball wedged in a corner doesn't buzz.
const REST: f32 = 0.08;
/// The crowd gasps at most this often (the game's shocked reaction cooldown).
const OOH_COOLDOWN: f32 = 5.0;
/// Steps per second by your speed (m/s).
const STRIDE_RATE: [(f32, f32); 5] = [(1.0, 1.6), (2.5, 2.0), (5.0, 2.7), (8.0, 3.3), (9.5, 3.6)];
const STEP_MIN_SPEED: f32 = 0.5;

/// Shot tiers, as the impact visuals and the kick animation split them: low power under
/// 29.41 m/s, mid under 34.82, the power shot above.
pub fn shot_power(speed: f32) -> Power {
    if speed >= POWER_SHOT_SPEED {
        Power::High
    } else if speed >= SHOT_HIGH_POWER_SPEED {
        Power::Mid
    } else {
        Power::Low
    }
}

/// A shot's spin class from its flight-frame spin (y dips, z curves). The lift every shot carries
/// isn't a choice, so it doesn't count; then the game's 45-degree split: mostly side spin goes with
/// none, mostly dip is topspin, mostly lift is downspin.
pub fn shot_spin(spin: Vec3) -> Spin {
    let (dip, side) = (spin.y - SHOT_BASE_SPIN, spin.z);
    if dip.hypot(side) < SPIN_MIN || side.abs() > dip.abs() {
        Spin::Flat
    } else if dip > 0.0 {
        Spin::Top
    } else {
        Spin::Down
    }
}

/// A kick's sound and strength, from its kind, the seconds its button was held, and the ball's speed
/// and spin as it leaves. Passes and lobs sound different only at a full gauge.
pub fn kick_sound(kind: Kick, held: f32, speed: f32, spin: Vec3) -> (Sound, f32) {
    let max = player::charge(kind, held) >= 1.0;
    match kind {
        Kick::Shot => (Sound::Shot(shot_power(speed), shot_spin(spin)), 0.85 + 0.15 * ((speed - SHOT_FORCE[0].1) / 14.0).clamp(0.0, 1.0)),
        Kick::Pass => (Sound::Pass { max }, 1.0),
        Kick::Lob => (Sound::Lob { max }, 1.0),
        Kick::ShortPass => (Sound::Pass { max: false }, 0.75),
        Kick::ShortLob => (Sound::Lob { max: false }, 0.75),
        Kick::PushBall | Kick::DribblePushBall => (Sound::PushBall, 1.0),
        // The flick is a soft scoop up: the lob's sound, never its full one.
        Kick::LobbedPushBall => (Sound::Lob { max: false }, 0.8),
    }
}

/// The game's control heights (the ball's centre): the foot under 0.9 m, the knee under 1.4 m, the
/// chest above.
pub fn control_height(y: f32) -> Height {
    if y < 0.9 {
        Height::Foot
    } else if y < 1.4 {
        Height::Knee
    } else {
        Height::Chest
    }
}

pub fn pace(speed: f32, sprinting: bool) -> Pace {
    if speed < 3.0 {
        Pace::Walk
    } else if sprinting || speed > 6.5 {
        Pace::Rush
    } else {
        Pace::Run
    }
}

/// By how fast the ball was falling.
pub fn bounce_hit(fall: f32) -> Hit {
    if fall >= 9.0 {
        Hit::Strong
    } else if fall >= 4.0 {
        Hit::Medium
    } else {
        Hit::Light
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Surface {
    Wall,
    Post,
}

/// What the ball's move this frame touched besides the grass: a post or the crossbar if it passed
/// within reach of one, else a wall or the ceiling if it was beside one.
fn surface(from: Vec3, to: Vec3) -> Option<Surface> {
    let frame = (0..=8).map(|i| goal_frame_gap(from.lerp(to, i as f32 / 8.0))).fold(f32::INFINITY, f32::min);
    let wall = |p: Vec3| (p.x.abs() - HALF_WIDTH).max(p.z.abs() - HALF_LENGTH).max(p.y - CEILING) + BALL_RADIUS;
    if frame < BALL_RADIUS + POST_RADIUS + 0.15 {
        Some(Surface::Post)
    } else if wall(from).max(wall(to)) > -0.6 {
        Some(Surface::Wall)
    } else {
        None
    }
}

/// How far `p` is from the nearest post's or crossbar's axis.
fn goal_frame_gap(p: Vec3) -> f32 {
    let gap = |a: Vec3, b: Vec3| {
        let ab = b - a;
        p.distance(a + ab * ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0))
    };
    let up = Vec3::Y * GOAL_HEIGHT;
    [-HALF_LENGTH, HALF_LENGTH]
        .into_iter()
        .flat_map(|z| {
            let (left, right) = (Vec3::new(-GOAL_HALF_WIDTH, 0.0, z), Vec3::new(GOAL_HALF_WIDTH, 0.0, z));
            [gap(left, left + up), gap(right, right + up), gap(left + up, right + up)]
        })
        .fold(f32::INFINITY, f32::min)
}

/// A landing on the grass, and how fast it fell: falling faster than BOUNCE_MIN near the ground last
/// frame, and not now.
fn landing(from: Vec3, to: Vec3, was: Vec3, now: Vec3) -> Option<f32> {
    (was.y < -BOUNCE_MIN && now.y > -0.2 && from.y.min(to.y) < BALL_RADIUS + 0.5).then_some(-was.y)
}

/// What the last frame looked like, to tell what just happened.
#[derive(Default)]
struct Seen {
    ready: bool,
    kick: Option<KickState>,
    held: bool,
    pos: Vec3,
    vel: Vec3,
    receiving: bool,
    has_ball: bool,
    /// The ball's height and speed as the current control began.
    control: Option<(f32, f32)>,
    dribble: Option<f32>,
    dash: Option<(f32, bool)>,
    effort: bool,
    carry: Option<(f32, f32)>,
    /// The stride clock: a step at each whole one.
    stride: f32,
    /// Seconds since the grass, a wall, the goal frame and an extra-effort touch last sounded, and
    /// since the crowd last gasped.
    rest: [f32; 4],
    ooh: f32,
    played: u32,
}

/// Watches the player and the ball for what just happened and plays its sound.
fn listen(
    mut commands: Commands,
    time: Res<Time>,
    bank: Res<Bank>,
    settings: Res<SfxSettings>,
    player: Single<(&Player, &Transform)>,
    ball: Single<(&Ball, &Transform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut goals: EventReader<GoalScored>,
    mut seen: Local<Seen>,
) {
    let dt = time.delta_secs();
    let ((p, ptf), (b, btf)) = (*player, *ball);
    let pos = btf.translation;
    // Ball sounds fade with distance from the camera: half as loud 30 m away.
    let far = 1.0 / (1.0 + camera.translation().distance(pos) / 30.0);
    let scored = goals.read().count() > 0;
    let mut cues = Vec::new();

    if seen.ready {
        // Our goal resets the ball before it reaches the net, so the net sounds with the goal.
        if scored {
            cues.extend([(Sound::Net, 0.8), (Sound::Goal, 1.0)]);
        }

        // A kick: the ball leaves your foot. A shot dropped while charging, the ball still yours,
        // is a fake shot.
        if let Some(k) = seen.kick.filter(|_| p.kick.is_none()) {
            if seen.held && !b.held {
                cues.push(kick_sound(k.kind, k.held, b.vel.length(), b.spin));
            } else if b.held && k.kind == Kick::Shot && !k.released {
                cues.push((Sound::FakeShot, 1.0));
            }
        }

        // A control sounds as the ball becomes yours (the game's ownership change), by the ball's
        // height and speed as it began: our control glides the ball down to your foot.
        if p.receiving.is_some() && !seen.receiving {
            seen.control = Some((pos.y, b.vel.length()));
        }
        if p.has_ball && !seen.has_ball {
            let (y, speed) = seen.control.unwrap_or((pos.y, b.vel.length()));
            cues.push((Sound::Control(control_height(y)), 0.35 + 0.65 * (speed / 25.0).min(1.0)));
        }

        // Moves: a dribble (not the one a dash turns into on the ball), a dash, extra effort.
        if let Some(d) = p.dribble
            && seen.dribble.is_none_or(|t| d.t < t)
            && seen.dash.is_none()
        {
            cues.push((Sound::Dribble { tired: d.exhausted }, 1.0));
        }
        if let Some(d) = p.dash
            && seen.dash.is_none_or(|(t, second)| d.t < t || d.second != second)
        {
            cues.push((Sound::Dash, 1.0));
        }
        if p.extra_active != seen.effort {
            cues.push((if p.extra_active { Sound::EffortOn } else { Sound::EffortOff }, 1.0));
        }

        // Running with the ball: each touch, when the carry's next touch came due.
        if let (Some(c), Some((start, next))) = (p.carry, seen.carry)
            && c.start > start
            && next <= c.start
        {
            cues.push((Sound::Touch, 0.6 + 0.4 * (p.vel.length() / SPRINT_SPEED).min(1.0)));
        }

        // Footsteps on a stride clock that runs faster the faster you go.
        let speed = p.vel.with_y(0.0).length();
        if speed > STEP_MIN_SPEED {
            seen.stride += curve(&STRIDE_RATE, speed) * dt;
            if seen.stride >= 1.0 {
                seen.stride -= 1.0;
                cues.push((Sound::Step(pace(speed, p.sprint_running)), (speed / SPRINT_SPEED).clamp(0.4, 1.0)));
            }
        } else {
            seen.stride = 0.6;
        }

        // The loose ball's hits: its velocity jumps. Not while anyone moves it, nor across a reset.
        let free = !b.held && !seen.held && b.freeze <= 0.0 && p.receiving.is_none() && !seen.receiving && !scored;
        let jumped = pos.distance(seen.pos) > seen.vel.length().max(b.vel.length()) * dt + 1.0;
        if free && !jumped {
            let mut change = b.vel - seen.vel;
            if let Some(fall) = landing(seen.pos, pos, seen.vel, b.vel) {
                change.y = 0.0;
                if seen.rest[0] > REST {
                    seen.rest[0] = 0.0;
                    cues.push((Sound::Bounce(bounce_hit(fall)), far * (0.25 + 0.75 * (fall - BOUNCE_MIN) / 12.0).min(1.0)));
                }
            }
            // More than gravity, drag, spin and friction can do in a frame.
            if change.length() > 1.0 + 45.0 * dt {
                let into = (-seen.vel.dot(change.normalize())).max(0.0);
                let near_you = pos.with_y(0.0).distance(ptf.translation.with_y(0.0)) < 2.0;
                match surface(seen.pos, pos) {
                    Some(Surface::Wall) if into > 1.2 && seen.rest[1] > REST => {
                        seen.rest[1] = 0.0;
                        cues.push((Sound::Wall, far * (0.2 + 0.8 * into / 25.0).min(1.0)));
                    }
                    Some(Surface::Post) if into > 1.0 && seen.rest[2] > REST => {
                        seen.rest[2] = 0.0;
                        cues.push((Sound::Post, far * (0.35 + 0.65 * into / 25.0).min(1.0)));
                        if into > 8.0 && seen.ooh > OOH_COOLDOWN {
                            seen.ooh = 0.0;
                            cues.push((Sound::Ooh, 1.0));
                        }
                    }
                    // Extra effort knocking the ball on.
                    None if p.extra_active && near_you && seen.rest[3] > 0.25 => {
                        seen.rest[3] = 0.0;
                        cues.push((Sound::PushBall, 0.6));
                    }
                    _ => {}
                }
            }
        }
    } else {
        seen.ooh = f32::INFINITY;
    }

    seen.ready = true;
    seen.kick = p.kick;
    seen.held = b.held;
    seen.pos = pos;
    seen.vel = b.vel;
    seen.receiving = p.receiving.is_some();
    seen.has_ball = p.has_ball;
    if p.receiving.is_none() {
        seen.control = None;
    }
    seen.dribble = p.dribble.map(|d| d.t);
    seen.dash = p.dash.map(|d| (d.t, d.second));
    seen.effort = p.extra_active;
    seen.carry = p.carry.map(|c| (c.start, c.next));
    for rest in &mut seen.rest {
        *rest += dt;
    }
    seen.ooh += dt;

    if bank.mute || !settings.on {
        return;
    }
    for (sound, gain) in cues {
        let Some(handle) = bank.sounds.get(&sound) else { continue };
        seen.played = seen.played.wrapping_add(1);
        let random = ((seen.played as f32 * 12.9898).sin() * 43758.545).fract().abs();
        let pitch = if sound.varies() { 0.97 + 0.06 * random } else { 1.0 };
        let volume = Volume::Linear(sound.level() * gain * settings.volume);
        commands.spawn((AudioPlayer(handle.clone()), PlaybackSettings::DESPAWN.with_volume(volume).with_speed(pitch), Voice(0)));
    }
}

/// Every sound, rendered once at startup; and whether to play them at all (not in a headless test
/// run, nor without an audio output).
#[derive(Resource)]
struct Bank {
    sounds: HashMap<Sound, Handle<Synth>>,
    mute: bool,
}

fn build_bank(mut commands: Commands, mut synths: ResMut<Assets<Synth>>) {
    let sounds = Sound::all().into_iter().map(|s| (s, synths.add(Synth(render(s).into())))).collect();
    commands.insert_resource(Bank { sounds, mute: std::env::var_os("RE_SCREENSHOT").is_some() });
}

/// A sound being played, and the frames since it was asked for.
#[derive(Component)]
struct Voice(u32);

/// With no audio output a sound never starts, and Bevy would keep it forever: drop it, and play no
/// more.
fn reap(mut commands: Commands, mut voices: Query<(Entity, &mut Voice, Has<AudioSink>)>, mut bank: ResMut<Bank>) {
    for (entity, mut voice, started) in &mut voices {
        voice.0 += 1;
        if !started && voice.0 > 5 {
            commands.entity(entity).despawn();
            bank.mute = true;
        }
    }
}

// ---- Playback ----

const RATE: u32 = 44_100;

/// A synthesised sound: mono samples at RATE.
#[derive(Asset, TypePath)]
pub struct Synth(Arc<[f32]>);

/// A Synth playing: its samples one by one.
pub struct SynthPlayback {
    samples: Arc<[f32]>,
    at: usize,
}

impl Iterator for SynthPlayback {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.samples.get(self.at).copied();
        self.at += 1;
        sample
    }
}

impl Source for SynthPlayback {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.at))
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.samples.len() as f32 / RATE as f32))
    }
}

impl Decodable for Synth {
    type DecoderItem = f32;
    type Decoder = SynthPlayback;

    fn decoder(&self) -> SynthPlayback {
        SynthPlayback { samples: self.0.clone(), at: 0 }
    }
}

// ---- Synthesis ----

/// White noise, the same every run.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// A state-variable filter (Andrew Simper's): low-, band- and high-pass of one sample.
#[derive(Default)]
struct Filter {
    s1: f32,
    s2: f32,
}

impl Filter {
    fn step(&mut self, x: f32, cutoff: f32, q: f32) -> [f32; 3] {
        let g = (PI * (cutoff / RATE as f32).min(0.45)).tan();
        let k = 1.0 / q;
        let band = (self.s1 + g * (x - self.s2)) / (1.0 + g * (g + k));
        let low = self.s2 + g * band;
        self.s1 = 2.0 * band - self.s1;
        self.s2 = 2.0 * low - self.s2;
        [low, k * band, x - k * band - low]
    }
}

/// A filter's output: low-, band- or high-pass, or a steeper band (two in a row) for air and
/// voices, which would otherwise hiss.
#[derive(Clone, Copy)]
enum Pass {
    Low,
    Band,
    High,
    Band4,
}

fn smooth(u: f32) -> f32 {
    u * u * (3.0 - 2.0 * u)
}

/// An attack, then an exponential decay.
fn env(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack { t / attack } else { (-(t - attack) / decay).exp() }
}

/// Up to its peak at `peak` (a share of `len`) and back to nothing at `len`, smoothly.
fn swell(t: f32, len: f32, peak: f32) -> f32 {
    let x = (t / len).clamp(0.0, 1.0);
    smooth(if x < peak { x / peak } else { (1.0 - x) / (1.0 - peak) })
}

/// From `from` to `to` over `secs`, exponentially, then held.
fn glide(from: f32, to: f32, secs: f32) -> impl Fn(f32) -> f32 {
    move |t| from * (to / from).powf((t / secs).min(1.0))
}

/// A sine at `pitch(t)` Hz with `octave` of its octave, shaped by `shape(t)`.
fn tone(pitch: impl Fn(f32) -> f32, octave: f32, shape: impl Fn(f32) -> f32) -> impl FnMut(f32) -> f32 {
    let mut phase = 0.0f32;
    move |t| {
        phase = (phase + TAU * pitch(t) / RATE as f32) % TAU;
        (phase.sin() + octave * (2.0 * phase).sin()) * shape(t)
    }
}

/// Noise through a `pass` filter at `cutoff(t)` Hz, shaped by `shape(t)`.
fn noise(seed: u32, pass: Pass, cutoff: impl Fn(f32) -> f32, q: f32, shape: impl Fn(f32) -> f32) -> impl FnMut(f32) -> f32 {
    let (mut noise, mut first, mut second) = (Noise(seed), Filter::default(), Filter::default());
    move |t| {
        let hz = cutoff(t);
        let [low, band, high] = first.step(noise.next(), hz, q);
        let out = match pass {
            Pass::Low => low,
            Pass::Band => band,
            Pass::High => high,
            Pass::Band4 => second.step(band, hz, q)[1],
        };
        out * shape(t)
    }
}

/// A shoe's scuff on the grass.
fn scuff(seed: u32, hz: f32, decay: f32) -> impl FnMut(f32) -> f32 {
    noise(seed, Pass::Band, move |_| hz, 1.4, move |t| env(t, 0.001, decay))
}

/// Decaying sines (Hz, level, decay in s): struck metal, or a wall panel.
fn ring(partials: &'static [(f32, f32, f32)]) -> impl FnMut(f32) -> f32 {
    move |t| partials.iter().map(|&(hz, level, decay)| level * (std::f64::consts::TAU * hz as f64 * t as f64).sin() as f32 * env(t, 0.0005, decay)).sum()
}

/// A foot or the ground meeting the ball: a thump gliding from `from` to `to` Hz and decaying over
/// `decay` s, the leather's slap (`slap` loud, `crack` 0..1 bright), and its hollow pock (level, Hz).
#[derive(Clone, Copy)]
struct Strike {
    from: f32,
    to: f32,
    decay: f32,
    slap: f32,
    crack: f32,
    pock: (f32, f32),
}

/// A whoosh's sweep (Hz from and to), its sharpness, and a flutter (Hz, depth) for a spinning ball.
#[derive(Clone, Copy)]
struct Air {
    from: f32,
    to: f32,
    q: f32,
    flutter: (f32, f32),
}

const PLAIN_AIR: Air = Air { from: 1300.0, to: 800.0, q: 1.2, flutter: (0.0, 0.0) };

/// The flying ball's whoosh by its spin: flat for none, a steep falling flutter for the dip, a
/// slow rising one for the float.
fn spin_air(spin: Spin) -> Air {
    match spin {
        Spin::Flat => PLAIN_AIR,
        Spin::Top => Air { from: 2800.0, to: 350.0, q: 2.2, flutter: (30.0, 0.6) },
        Spin::Down => Air { from: 450.0, to: 2600.0, q: 1.8, flutter: (12.0, 0.35) },
    }
}

/// A sound being mixed, layer by layer.
struct Track(Vec<f32>);

impl Track {
    fn new(secs: f32) -> Self {
        Self(vec![0.0; (secs * RATE as f32) as usize])
    }

    /// Mixes in `layer(t)` from `at` seconds on, at `gain`.
    fn add(&mut self, at: f32, gain: f32, mut layer: impl FnMut(f32) -> f32) -> &mut Self {
        let start = (at * RATE as f32) as usize;
        for (i, sample) in self.0.iter_mut().enumerate().skip(start) {
            *sample += gain * layer((i - start) as f32 / RATE as f32);
        }
        self
    }

    fn strike(&mut self, at: f32, gain: f32, s: Strike, seed: u32) -> &mut Self {
        let slap = glide(1500.0 + 4000.0 * s.crack, 1200.0 + 2000.0 * s.crack, 0.02);
        self.add(at, gain, tone(glide(s.from, s.to, 0.06), 0.0, move |t| env(t, 0.002, s.decay)))
            .add(at, gain * s.slap, noise(seed, Pass::High, slap, 0.7, move |t| env(t, 0.0003, 0.006 + 0.012 * s.crack)))
            .add(at, gain * s.pock.0, noise(seed + 1, Pass::Band, glide(s.pock.1, s.pock.1 * 0.65, 0.04), 2.5, |t| env(t, 0.001, 0.03)))
    }

    /// Air rushing past for `len` s, peaking at `gain` (a strike peaks near 1).
    fn whoosh(&mut self, at: f32, len: f32, gain: f32, air: Air, seed: u32) -> &mut Self {
        let (hz, depth) = air.flutter;
        let mut layer = noise(seed, Pass::Band4, glide(air.from, air.to, len), air.q, move |t| swell(t, len, 0.25) * (1.0 - depth * (0.5 + 0.5 * (TAU * hz * t).sin())));
        let samples: Vec<f32> = (0..(len * RATE as f32) as usize).map(|i| layer(i as f32 / RATE as f32)).collect();
        let peak = samples.iter().fold(1e-6f32, |m, s| m.max(s.abs()));
        let mut i = 0;
        self.add(at, gain / peak, move |_| {
            i += 1;
            samples.get(i - 1).copied().unwrap_or(0.0)
        })
    }

    /// Peaks at 0.95, with a 5 ms fade at the end so it can't click.
    fn done(mut self) -> Vec<f32> {
        let peak = self.0.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let fade = (0.005 * RATE as f32) as usize;
        let len = self.0.len();
        for (i, sample) in self.0.iter_mut().enumerate() {
            *sample *= 0.95 / peak.max(1e-6) * ((len - i) as f32 / fade as f32).min(1.0);
        }
        self.0
    }
}

/// A crowd's swell: up in 0.4 s, held to 1.3 s, then dying away by 3.6 s.
fn cheer(t: f32) -> f32 {
    smooth((t / 0.4).min(1.0)) * if t < 1.3 { 1.0 } else { (-(t - 1.3) / 0.8).exp() } * (1.0 - smooth(((t - 3.3) / 0.3).clamp(0.0, 1.0)))
}

/// A crowd's "ooh": its vowel rises, then sinks.
fn ooh_vowel(t: f32) -> f32 {
    if t < 0.4 { glide(380.0, 560.0, 0.4)(t) } else { glide(560.0, 430.0, 1.1)(t - 0.4) }
}

fn ooh_shape(t: f32) -> f32 {
    smooth((t / 0.25).min(1.0)) * if t < 0.35 { 1.0 } else { (-(t - 0.35) / 0.45).exp() }
}

fn render(sound: Sound) -> Vec<f32> {
    let strike = |from, to, decay, slap, crack, pock| Strike { from, to, decay, slap, crack, pock };
    let mut t;
    match sound {
        // A shot: the strike by power (the power shot's deeper, with a boom and a crack), then the
        // ball's whoosh by spin, longer and louder the harder it's hit.
        Sound::Shot(power, spin) => {
            let (s, len, air) = match power {
                Power::Low => (strike(160.0, 100.0, 0.045, 0.5, 0.25, (0.5, 900.0)), 0.3, 0.3),
                Power::Mid => (strike(140.0, 75.0, 0.065, 0.7, 0.55, (0.55, 850.0)), 0.45, 0.5),
                Power::High => (strike(120.0, 48.0, 0.1, 0.9, 0.95, (0.5, 800.0)), 0.65, 0.7),
            };
            t = Track::new(len + 0.1);
            t.strike(0.0, 1.0, s, 11).whoosh(0.04, len, air, spin_air(spin), 13);
            if power == Power::High {
                t.add(0.0, 0.6, tone(glide(70.0, 40.0, 0.1), 0.0, |t| env(t, 0.003, 0.16)));
            }
        }
        // Passes: a round "tock"; at full power a crisp, harder strike and a short whoosh.
        Sound::Pass { max: false } => {
            t = Track::new(0.25);
            t.strike(0.0, 1.0, strike(180.0, 115.0, 0.04, 0.35, 0.2, (0.6, 800.0)), 21);
        }
        Sound::Pass { max: true } => {
            t = Track::new(0.4);
            t.strike(0.0, 1.0, strike(165.0, 90.0, 0.055, 0.9, 0.9, (0.5, 850.0)), 23).whoosh(0.03, 0.28, 0.35, PLAIN_AIR, 25);
        }
        // Lobs: a hollow "thock" scooped under the ball and rising air; at full power crisper and
        // with more air.
        Sound::Lob { max: false } => {
            t = Track::new(0.45);
            t.strike(0.0, 1.0, strike(210.0, 150.0, 0.035, 0.25, 0.1, (0.9, 520.0)), 31).whoosh(0.03, 0.35, 0.25, Air { from: 600.0, to: 1300.0, q: 1.0, flutter: (0.0, 0.0) }, 33);
        }
        Sound::Lob { max: true } => {
            t = Track::new(0.6);
            t.strike(0.0, 1.0, strike(190.0, 110.0, 0.05, 0.85, 0.8, (0.8, 560.0)), 35).whoosh(0.03, 0.5, 0.45, Air { from: 500.0, to: 2200.0, q: 1.2, flutter: (0.0, 0.0) }, 37);
        }
        // A toe poke with a brush of grass.
        Sound::PushBall => {
            t = Track::new(0.2);
            t.strike(0.0, 1.0, strike(240.0, 170.0, 0.025, 0.3, 0.3, (0.4, 1000.0)), 41)
                .add(0.0, 0.3, noise(43, Pass::Low, glide(1500.0, 700.0, 0.06), 0.7, |t| env(t, 0.002, 0.05)));
        }
        Sound::Touch => {
            t = Track::new(0.12);
            t.strike(0.0, 1.0, strike(270.0, 210.0, 0.018, 0.12, 0.05, (0.3, 1100.0)), 45);
        }
        // The leg swinging through, with no ball.
        Sound::FakeShot => {
            t = Track::new(0.3);
            t.whoosh(0.0, 0.22, 1.0, Air { from: 500.0, to: 1400.0, q: 0.9, flutter: (0.0, 0.0) }, 47).add(0.04, 0.4, scuff(48, 3000.0, 0.04));
        }
        // Controls: the foot muffles the ball, the knee knocks it, the chest thumps under cloth.
        Sound::Control(Height::Foot) => {
            t = Track::new(0.25);
            t.add(0.0, 1.0, tone(glide(125.0, 80.0, 0.05), 0.0, |t| env(t, 0.002, 0.05)))
                .add(0.0, 0.6, noise(51, Pass::Low, glide(1100.0, 350.0, 0.08), 0.7, |t| env(t, 0.001, 0.05)));
        }
        Sound::Control(Height::Knee) => {
            t = Track::new(0.2);
            t.add(0.0, 1.0, tone(glide(220.0, 160.0, 0.04), 0.2, |t| env(t, 0.001, 0.04)))
                .add(0.0, 0.5, noise(53, Pass::Band, |_| 1000.0, 1.5, |t| env(t, 0.0005, 0.025)));
        }
        Sound::Control(Height::Chest) => {
            t = Track::new(0.3);
            t.add(0.0, 1.0, tone(glide(100.0, 62.0, 0.06), 0.0, |t| env(t, 0.003, 0.09)))
                .add(0.0, 0.5, noise(55, Pass::Band, glide(2400.0, 1600.0, 0.08), 0.7, |t| env(t, 0.002, 0.06)));
        }
        // A dribble: the body's swish and a shoe's scuff; tired, duller and slower.
        Sound::Dribble { tired: false } => {
            t = Track::new(0.25);
            t.whoosh(0.0, 0.2, 0.8, Air { from: 600.0, to: 1600.0, q: 1.0, flutter: (0.0, 0.0) }, 61).add(0.05, 0.6, scuff(62, 3500.0, 0.035));
        }
        Sound::Dribble { tired: true } => {
            t = Track::new(0.33);
            t.whoosh(0.0, 0.28, 0.8, Air { from: 400.0, to: 1000.0, q: 0.9, flutter: (0.0, 0.0) }, 63).add(0.08, 0.4, scuff(64, 2200.0, 0.05));
        }
        // A dash: a bigger swish, two scuffs and the body's push.
        Sound::Dash => {
            t = Track::new(0.4);
            t.whoosh(0.0, 0.32, 1.0, Air { from: 350.0, to: 1300.0, q: 0.9, flutter: (0.0, 0.0) }, 65)
                .add(0.0, 0.7, scuff(66, 3200.0, 0.04))
                .add(0.1, 0.6, scuff(67, 2900.0, 0.04))
                .add(0.0, 0.4, tone(glide(95.0, 70.0, 0.05), 0.0, |t| env(t, 0.003, 0.06)));
        }
        // Extra effort: a power-up rising on a breath; off, a short fall.
        Sound::EffortOn => {
            t = Track::new(0.6);
            t.add(0.0, 1.0, tone(glide(170.0, 540.0, 0.45), 0.3, |t| swell(t, 0.6, 0.65)))
                .add(0.0, 0.5, noise(71, Pass::Band, glide(900.0, 3200.0, 0.5), 1.2, |t| swell(t, 0.6, 0.6)));
        }
        Sound::EffortOff => {
            t = Track::new(0.3);
            t.add(0.0, 1.0, tone(glide(420.0, 150.0, 0.25), 0.2, |t| env(t, 0.005, 0.1)))
                .add(0.0, 0.3, noise(73, Pass::Band, glide(2000.0, 600.0, 0.25), 1.0, |t| env(t, 0.005, 0.08)));
        }
        // Grass steps: softer and darker walking, brighter with a crunch at a run and a sprint.
        Sound::Step(pace) => {
            let (cutoff, decay, thump, crunch) = match pace {
                Pace::Walk => (700.0, 0.03, 0.5, 0.0),
                Pace::Run => (1100.0, 0.04, 0.6, 0.25),
                Pace::Rush => (1500.0, 0.045, 0.7, 0.45),
            };
            t = Track::new(0.15);
            t.add(0.0, 1.0, noise(81, Pass::Low, glide(cutoff, cutoff * 0.6, 0.05), 0.7, move |t| env(t, 0.0015, decay)))
                .add(0.0, thump, tone(glide(90.0, 60.0, 0.04), 0.0, |t| env(t, 0.002, 0.025)))
                .add(0.0, crunch, noise(83, Pass::High, |_| 3500.0, 0.7, |t| env(t, 0.0005, 0.015)));
        }
        // The ball on the grass: a thump and a soft slap, deeper and harder as it falls faster.
        Sound::Bounce(hit) => {
            let (s, boom) = match hit {
                Hit::Light => (strike(170.0, 125.0, 0.03, 0.2, 0.1, (0.4, 900.0)), 0.0),
                Hit::Medium => (strike(150.0, 95.0, 0.05, 0.4, 0.3, (0.4, 800.0)), 0.0),
                Hit::Strong => (strike(130.0, 65.0, 0.08, 0.6, 0.6, (0.35, 700.0)), 0.5),
            };
            t = Track::new(0.3);
            t.strike(0.0, 1.0, s, 91)
                .add(0.0, 0.5, noise(93, Pass::Low, |_| 1600.0, 0.7, |t| env(t, 0.0008, 0.025)))
                .add(0.0, boom, tone(glide(70.0, 45.0, 0.08), 0.0, |t| env(t, 0.002, 0.12)));
        }
        // A wall panel knocked: the ball's thump and the panel's short boxy ring.
        Sound::Wall => {
            t = Track::new(0.45);
            t.strike(0.0, 1.0, strike(150.0, 85.0, 0.06, 0.4, 0.3, (0.5, 1200.0)), 101)
                .add(0.0, 0.8, ring(&[(235.0, 0.5, 0.16), (415.0, 0.35, 0.11), (730.0, 0.2, 0.07), (1210.0, 0.1, 0.04)]));
        }
        // The post: a click and a long metallic ring (a tube's inharmonic partials, one pair
        // beating).
        Sound::Post => {
            t = Track::new(1.5);
            t.add(0.0, 0.6, noise(111, Pass::High, |_| 5000.0, 0.7, |t| env(t, 0.0002, 0.004)))
                .add(0.0, 0.4, tone(glide(200.0, 150.0, 0.02), 0.0, |t| env(t, 0.0005, 0.02)))
                .add(0.0, 1.0, ring(&[(640.0, 1.0, 0.9), (655.0, 0.5, 0.75), (1766.0, 0.55, 0.5), (3456.0, 0.32, 0.28), (5715.0, 0.18, 0.16)]));
        }
        // The net: a soft rustling swish, the ball's dull thud in it.
        Sound::Net => {
            t = Track::new(0.5);
            let rustle = |t: f32| env(t, 0.015, 0.16) * (0.7 + 0.3 * (TAU * 37.0 * t).sin() * (TAU * 23.0 * t).sin());
            t.add(0.0, 1.0, noise(121, Pass::Band4, glide(3200.0, 1700.0, 0.35), 0.8, rustle))
                .add(0.0, 0.25, tone(glide(95.0, 70.0, 0.05), 0.0, |t| env(t, 0.003, 0.05)));
        }
        // A goal: a crowd's swell, many voices in a few vowel bands, each wobbling, over a roar.
        Sound::Goal => {
            t = Track::new(3.6);
            for (i, (hz, q, level)) in [(480.0, 3.0, 1.0), (950.0, 2.5, 0.8), (1650.0, 2.0, 0.55), (2700.0, 1.5, 0.3)].into_iter().enumerate() {
                let i = i as f32;
                let drift = move |t: f32| hz * (1.0 + 0.05 * (TAU * 0.7 * t + i).sin());
                let wobble = move |t: f32| cheer(t) * (0.75 + 0.25 * (TAU * (1.3 + 0.7 * i) * t + i).sin() * (TAU * (2.9 + 1.1 * i) * t).sin());
                t.add(0.0, level, noise(131 + i as u32, Pass::Band4, drift, q, wobble));
            }
            t.add(0.0, 0.4, noise(139, Pass::Low, |_| 350.0, 0.7, cheer));
        }
        // The crowd's gasp: an "ooh" rising and sinking.
        Sound::Ooh => {
            t = Track::new(1.5);
            t.add(0.0, 1.0, noise(141, Pass::Band4, ooh_vowel, 4.0, ooh_shape))
                .add(0.0, 0.5, noise(143, Pass::Band4, |t| ooh_vowel(t) * 2.3, 3.0, ooh_shape))
                .add(0.0, 0.3, noise(145, Pass::Low, |_| 300.0, 0.7, ooh_shape));
        }
    }
    t.done()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_pick_their_sound_by_power_and_spin() {
        assert_eq!(shot_power(SHOT_FORCE[0].1), Power::Low);
        assert_eq!(shot_power(SHOT_HIGH_POWER_SPEED), Power::Mid);
        assert_eq!(shot_power(SHOT_FORCE[2].1), Power::High);
        // Through the shot's own spin curves at full charge, from the curve input (side, up).
        let spin = |side: f32, up: f32| {
            Vec3::new(0.0, curve(&TOP_SPIN, up) * curve(&TOP_SPIN_BY_CHARGE, 1.0) + SHOT_BASE_SPIN, curve(&SIDE_SPIN, side) * curve(&SIDE_SPIN_BY_CHARGE, 1.0))
        };
        assert_eq!(shot_spin(spin(0.0, 0.0)), Spin::Flat, "the built-in lift alone is no spin");
        assert_eq!(shot_spin(spin(0.0, -1.0)), Spin::Top, "pulled down, it dips");
        assert_eq!(shot_spin(spin(0.0, 1.0)), Spin::Down, "pushed up, it floats");
        assert_eq!(shot_spin(spin(1.0, 0.0)), Spin::Flat, "side spin goes with none");
        assert_eq!(shot_spin(spin(0.5, -1.0)), Spin::Top, "more dip than curl");
        assert_eq!(shot_spin(spin(1.0, -0.5)), Spin::Flat, "more curl than dip");
        assert_eq!(kick_sound(Kick::Shot, MAX_HOLD, 38.0, spin(0.0, -1.0)).0, Sound::Shot(Power::High, Spin::Top));
        assert_eq!(kick_sound(Kick::Shot, 0.1, 24.0, spin(0.0, 1.0)).0, Sound::Shot(Power::Low, Spin::Down));
    }

    #[test]
    fn passes_and_lobs_change_only_at_a_full_gauge() {
        for (kind, max, other) in [(Kick::Pass, Sound::Pass { max: true }, Sound::Pass { max: false }), (Kick::Lob, Sound::Lob { max: true }, Sound::Lob { max: false })] {
            assert_eq!(kick_sound(kind, kind.buildup(), 30.0, Vec3::ZERO).0, max);
            assert_eq!(kick_sound(kind, kind.buildup() * 0.8, 25.0, Vec3::ZERO).0, other);
        }
        assert_eq!(kick_sound(Kick::ShortPass, 0.0, SHORT_PASS_SPEED, Vec3::ZERO).0, Sound::Pass { max: false });
        assert_eq!(kick_sound(Kick::DribblePushBall, 0.0, DRIBBLE_PUSH_BALL_SPEED, Vec3::ZERO).0, Sound::PushBall);
    }

    #[test]
    fn hits_by_place_and_strength() {
        let post = Vec3::new(GOAL_HALF_WIDTH - 0.15, 1.0, HALF_LENGTH - 0.15);
        assert_eq!(surface(post - Vec3::Z * 0.4, post), Some(Surface::Post));
        let wall = Vec3::new(HALF_WIDTH - BALL_RADIUS, 1.0, 10.0);
        assert_eq!(surface(wall - Vec3::X * 0.3, wall), Some(Surface::Wall));
        assert_eq!(surface(Vec3::Y, Vec3::new(0.0, 1.0, 0.5)), None);
        assert_eq!(landing(Vec3::Y * 0.3, Vec3::Y * 0.2, Vec3::Y * -6.0, Vec3::Y * 2.8), Some(6.0));
        assert_eq!(landing(Vec3::Y * 3.0, Vec3::Y * 2.9, Vec3::Y * -6.0, Vec3::Y * 2.8), None, "the crossbar's top isn't the grass");
        assert_eq!((bounce_hit(2.0), bounce_hit(6.0), bounce_hit(12.0)), (Hit::Light, Hit::Medium, Hit::Strong));
        assert_eq!((control_height(BALL_RADIUS), control_height(1.0), control_height(1.6)), (Height::Foot, Height::Knee, Height::Chest));
        assert_eq!((pace(2.0, false), pace(5.0, false), pace(7.5, true)), (Pace::Walk, Pace::Run, Pace::Rush));
    }

    #[test]
    fn every_sound_renders_short_and_clean() {
        for sound in Sound::all() {
            let samples = render(sound);
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(!samples.is_empty() && samples.len() < 4 * RATE as usize, "{sound:?} is {} samples", samples.len());
            assert!(samples.iter().all(|s| s.is_finite()) && peak > 0.5 && peak <= 0.951, "{sound:?} peaks at {peak}");
        }
    }

    /// `RE_SFX_DUMP=<dir> cargo test previews` writes every sound there as a WAV file to listen to.
    #[test]
    fn previews() {
        let Some(dir) = std::env::var_os("RE_SFX_DUMP") else { return };
        let dir = std::path::Path::new(&dir);
        std::fs::create_dir_all(dir).unwrap();
        for sound in Sound::all() {
            let name = format!("{sound:?}").to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join("_");
            std::fs::write(dir.join(format!("{name}.wav")), wav(&render(sound))).unwrap();
        }
    }

    /// 16-bit mono PCM.
    fn wav(samples: &[f32]) -> Vec<u8> {
        let data = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        out
    }
}
