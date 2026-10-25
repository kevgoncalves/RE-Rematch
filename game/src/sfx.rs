//! Sound effects from Kenney's Impact Sounds (CC0, assets/sfx). Which sound plays when follows
//! the game's trigger rules (notes/sfx.md); each sound is one or more samples at a set volume and
//! playback speed, a random take of five each time.

use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::ball::{self, Ball, GoalScored};
use crate::player::{self, Kick, KickState, Player};
use crate::tuning::*;

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SfxSettings>()
            .add_systems(Startup, load_bank)
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

/// How hard the ball hits the grass or a wall.
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
    Control(Height),
    Dribble { tired: bool },
    Dash,
    Step(Pace),
    Bounce(Hit),
    Wall(Hit),
    Post,
    Net,
}

/// A family of samples in assets/sfx: five takes of one Kenney sound.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Sample {
    PunchHeavy,
    PunchMedium,
    SoftHeavy,
    SoftMedium,
    PlateLight,
    PlateMedium,
    PlateHeavy,
    MetalHeavy,
    BellHeavy,
    Grass,
}

impl Sample {
    const ALL: [Sample; 10] = [
        Sample::PunchHeavy,
        Sample::PunchMedium,
        Sample::SoftHeavy,
        Sample::SoftMedium,
        Sample::PlateLight,
        Sample::PlateMedium,
        Sample::PlateHeavy,
        Sample::MetalHeavy,
        Sample::BellHeavy,
        Sample::Grass,
    ];
    const TAKES: usize = 5;

    fn name(self) -> &'static str {
        match self {
            Sample::PunchHeavy => "impactPunch_heavy",
            Sample::PunchMedium => "impactPunch_medium",
            Sample::SoftHeavy => "impactSoft_heavy",
            Sample::SoftMedium => "impactSoft_medium",
            Sample::PlateLight => "impactPlate_light",
            Sample::PlateMedium => "impactPlate_medium",
            Sample::PlateHeavy => "impactPlate_heavy",
            Sample::MetalHeavy => "impactMetal_heavy",
            Sample::BellHeavy => "impactBell_heavy",
            Sample::Grass => "footstep_grass",
        }
    }

    fn path(self, take: usize) -> String {
        format!("sfx/{}_{take:03}.ogg", self.name())
    }
}

impl Sound {
    /// The samples it plays together: (sample, volume, playback speed). Speed is pitch too: a shot's
    /// topspin plays higher, its downspin lower.
    fn layers(self) -> Vec<(Sample, f32, f32)> {
        use Sample::*;
        match self {
            Sound::Shot(power, spin) => {
                let pitch = match spin {
                    Spin::Flat => 1.0,
                    Spin::Top => 1.12,
                    Spin::Down => 0.89,
                };
                match power {
                    Power::Low => vec![(PunchMedium, 0.6, 1.1 * pitch)],
                    Power::Mid => vec![(PunchMedium, 0.85, pitch), (SoftMedium, 0.35, 0.9)],
                    Power::High => vec![(PunchHeavy, 1.0, 0.95 * pitch), (SoftHeavy, 0.6, 0.9)],
                }
            }
            Sound::Pass { max: false } => vec![(SoftMedium, 0.6, 1.05)],
            Sound::Pass { max: true } => vec![(SoftHeavy, 0.75, 1.1), (PunchMedium, 0.4, 1.3)],
            Sound::Lob { max: false } => vec![(SoftMedium, 0.6, 0.85)],
            Sound::Lob { max: true } => vec![(SoftHeavy, 0.75, 0.9), (PunchMedium, 0.4, 1.1)],
            Sound::PushBall => vec![(SoftMedium, 0.5, 1.25)],
            Sound::Touch => vec![(SoftMedium, 0.25, 1.45)],
            Sound::Control(Height::Foot) => vec![(SoftMedium, 0.6, 0.95)],
            Sound::Control(Height::Knee) => vec![(SoftMedium, 0.55, 1.2)],
            Sound::Control(Height::Chest) => vec![(SoftHeavy, 0.6, 0.85)],
            // A dribble's cut and a dash plant a foot hard.
            Sound::Dribble { tired: false } => vec![(Grass, 0.7, 1.1)],
            Sound::Dribble { tired: true } => vec![(Grass, 0.5, 0.9)],
            Sound::Dash => vec![(Grass, 0.8, 0.95)],
            Sound::Step(Pace::Walk) => vec![(Grass, 0.3, 0.95)],
            Sound::Step(Pace::Run) => vec![(Grass, 0.4, 1.0)],
            Sound::Step(Pace::Rush) => vec![(Grass, 0.5, 1.05)],
            Sound::Bounce(Hit::Light) => vec![(SoftMedium, 0.35, 1.15)],
            Sound::Bounce(Hit::Medium) => vec![(SoftMedium, 0.6, 1.0)],
            Sound::Bounce(Hit::Strong) => vec![(SoftHeavy, 0.8, 1.0)],
            // A wall panel and the ball's thud on it.
            Sound::Wall(Hit::Light) => vec![(PlateLight, 0.5, 1.0), (SoftMedium, 0.4, 1.1)],
            Sound::Wall(Hit::Medium) => vec![(PlateMedium, 0.7, 1.0), (SoftMedium, 0.5, 1.0)],
            Sound::Wall(Hit::Strong) => vec![(PlateHeavy, 0.9, 1.0), (SoftHeavy, 0.5, 1.0)],
            Sound::Post => vec![(MetalHeavy, 0.9, 1.0), (BellHeavy, 0.4, 1.0)],
            // The net: a low, soft catch and a rustle (a grass step slowed down).
            Sound::Net => vec![(SoftHeavy, 0.5, 0.75), (Grass, 0.6, 0.7)],
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

/// By how fast the ball went into the wall.
pub fn wall_hit(into: f32) -> Hit {
    if into >= 16.0 {
        Hit::Strong
    } else if into >= 8.0 {
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
    carry: Option<(f32, f32)>,
    /// The stride clock: a step at each whole one.
    stride: f32,
    /// Seconds since the grass, a wall, the goal frame and an extra-effort touch last sounded.
    rest: [f32; 4],
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
            cues.push((Sound::Net, 1.0));
        }

        // A kick: the ball leaves your foot.
        if let Some(k) = seen.kick
            && p.kick.is_none()
            && seen.held
            && !b.held
        {
            cues.push(kick_sound(k.kind, k.held, b.vel.length(), b.spin));
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

        // Moves: a dribble (not the one a dash turns into on the ball), a dash.
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
                        cues.push((Sound::Wall(wall_hit(into)), far * (0.2 + 0.8 * into / 25.0).min(1.0)));
                    }
                    Some(Surface::Post) if into > 1.0 && seen.rest[2] > REST => {
                        seen.rest[2] = 0.0;
                        cues.push((Sound::Post, far * (0.35 + 0.65 * into / 25.0).min(1.0)));
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
    seen.carry = p.carry.map(|c| (c.start, c.next));
    for rest in &mut seen.rest {
        *rest += dt;
    }

    if bank.mute || !settings.on {
        return;
    }
    for (sound, gain) in cues {
        for (sample, volume, speed) in sound.layers() {
            seen.played = seen.played.wrapping_add(1);
            let random = ((seen.played as f32 * 12.9898).sin() * 43758.545).fract().abs();
            let Some(takes) = bank.samples.get(&sample) else { continue };
            let take = takes[(random * takes.len() as f32) as usize % takes.len()].clone();
            let jitter = if sound.varies() { 0.97 + 0.06 * (random * 7.0).fract() } else { 1.0 };
            let volume = Volume::Linear(volume * gain * settings.volume);
            commands.spawn((AudioPlayer(take), PlaybackSettings::DESPAWN.with_volume(volume).with_speed(speed * jitter), Voice(0)));
        }
    }
}

/// Every sample's takes, loaded at startup; and whether to play them at all (not in a headless test
/// run, nor without an audio output).
#[derive(Resource)]
struct Bank {
    samples: HashMap<Sample, Vec<Handle<AudioSource>>>,
    mute: bool,
}

fn load_bank(mut commands: Commands, server: Res<AssetServer>) {
    let samples = Sample::ALL.into_iter().map(|s| (s, (0..Sample::TAKES).map(|take| server.load(s.path(take))).collect())).collect();
    commands.insert_resource(Bank { samples, mute: std::env::var_os("RE_SCREENSHOT").is_some() });
}

/// A sound being played, and the frames it has waited, loaded, to start.
#[derive(Component)]
struct Voice(u32);

/// With no audio output a loaded sound never starts, and Bevy would keep it forever: drop it, and
/// play no more. A sample that failed to load is dropped too.
fn reap(mut commands: Commands, server: Res<AssetServer>, mut voices: Query<(Entity, &mut Voice, &AudioPlayer, Has<AudioSink>)>, mut bank: ResMut<Bank>) {
    for (entity, mut voice, player, started) in &mut voices {
        if started {
            continue;
        }
        let state = server.load_state(player.0.id());
        if state.is_failed() {
            commands.entity(entity).despawn();
        } else if state.is_loaded() {
            voice.0 += 1;
            if voice.0 > 5 {
                commands.entity(entity).despawn();
                bank.mute = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sound and variant.
    fn every_sound() -> Vec<Sound> {
        let mut all = Vec::new();
        for power in [Power::Low, Power::Mid, Power::High] {
            all.extend([Spin::Flat, Spin::Top, Spin::Down].map(|spin| Sound::Shot(power, spin)));
        }
        all.extend([Sound::Pass { max: false }, Sound::Pass { max: true }, Sound::Lob { max: false }, Sound::Lob { max: true }]);
        all.extend([Sound::PushBall, Sound::Touch, Sound::Dribble { tired: false }, Sound::Dribble { tired: true }, Sound::Dash, Sound::Post, Sound::Net]);
        all.extend([Height::Foot, Height::Knee, Height::Chest].map(Sound::Control));
        all.extend([Pace::Walk, Pace::Run, Pace::Rush].map(Sound::Step));
        for hit in [Hit::Light, Hit::Medium, Hit::Strong] {
            all.extend([Sound::Bounce(hit), Sound::Wall(hit)]);
        }
        all
    }

    fn pitch(sound: Sound) -> f32 {
        sound.layers()[0].2
    }

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
        // The power shot is the heavy punch; topspin plays higher and downspin lower at every power.
        assert_eq!(Sound::Shot(Power::High, Spin::Flat).layers()[0].0, Sample::PunchHeavy);
        assert_eq!(Sound::Shot(Power::Mid, Spin::Flat).layers()[0].0, Sample::PunchMedium);
        for power in [Power::Low, Power::Mid, Power::High] {
            let [flat, top, down] = [Spin::Flat, Spin::Top, Spin::Down].map(|s| pitch(Sound::Shot(power, s)));
            assert!(top > flat && flat > down, "{power:?}: top {top}, flat {flat}, down {down}");
        }
    }

    #[test]
    fn passes_and_lobs_change_only_at_a_full_gauge() {
        for (kind, max, other) in [(Kick::Pass, Sound::Pass { max: true }, Sound::Pass { max: false }), (Kick::Lob, Sound::Lob { max: true }, Sound::Lob { max: false })] {
            assert_eq!(kick_sound(kind, kind.buildup(), 30.0, Vec3::ZERO).0, max);
            assert_eq!(kick_sound(kind, kind.buildup() * 0.8, 25.0, Vec3::ZERO).0, other);
            // At full power: louder, with a crisp punch on top.
            assert!(max.layers()[0].1 > other.layers()[0].1 && max.layers().iter().any(|l| l.0 == Sample::PunchMedium));
        }
        assert!(pitch(Sound::Lob { max: false }) < pitch(Sound::Pass { max: false }), "a lob is scooped, lower");
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
        assert_eq!((wall_hit(5.0), wall_hit(10.0), wall_hit(20.0)), (Hit::Light, Hit::Medium, Hit::Strong));
        assert_eq!(Sound::Wall(Hit::Strong).layers()[0].0, Sample::PlateHeavy);
        assert_eq!((control_height(BALL_RADIUS), control_height(1.0), control_height(1.6)), (Height::Foot, Height::Knee, Height::Chest));
        assert_eq!((pace(2.0, false), pace(5.0, false), pace(7.5, true)), (Pace::Walk, Pace::Run, Pace::Rush));
    }

    #[test]
    fn layers_play_at_sane_levels() {
        for sound in every_sound() {
            for (sample, volume, speed) in sound.layers() {
                assert!(volume > 0.0 && volume <= 1.0 && (0.5..=2.0).contains(&speed), "{sound:?}: {sample:?} at {volume}, x{speed}");
            }
        }
    }

    /// assets/sfx holds exactly the takes the sounds use, and Kenney's licence; Bevy's decoder reads
    /// every take.
    #[test]
    fn the_shipped_samples_are_the_used_ones() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let used: Vec<Sample> = Sample::ALL.into_iter().filter(|s| every_sound().iter().any(|sound| sound.layers().iter().any(|l| l.0 == *s))).collect();
        assert_eq!(used, Sample::ALL, "a sample nothing plays");
        let mut wanted: Vec<String> = Sample::ALL.iter().flat_map(|s| (0..Sample::TAKES).map(|take| s.path(take))).collect();
        wanted.push("sfx/License.txt".into());
        wanted.sort();
        let mut shipped: Vec<String> = std::fs::read_dir(dir.join("sfx")).unwrap().map(|e| format!("sfx/{}", e.unwrap().file_name().to_string_lossy())).collect();
        shipped.sort();
        assert_eq!(shipped, wanted);
        for path in wanted.iter().filter(|p| p.ends_with(".ogg")) {
            let source = AudioSource { bytes: std::fs::read(dir.join(path)).unwrap().into() };
            assert!(source.decoder().count() > 1000, "{path} decodes to almost nothing");
        }
    }
}
