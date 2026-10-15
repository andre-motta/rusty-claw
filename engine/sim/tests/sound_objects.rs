//! Sound objects on synthetic levels: sound triggers, Claw's dialog
//! triggers, ambient loops and placed sounds, and the cues they emit.

mod common;

use claw_sim::claw_frame::{AudioCue, SoundChannel, SoundCue};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::sound::{self, PAN_RANGE_PX};
use claw_sim::logic::sound_object::{
    self, AMBIENT_VOICE_BASE, AmbientManager, SoundData, SoundPart, ambient_volume, fade_volume,
    spot_level_pan, trigger_area,
};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::{InputLatch, InputScript, ObjectData, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const PUDDLE: &str = "LEVEL_TRIGGER_PUDDLE";
const DRIP: &str = "LEVEL_TRIGGER_DRIP";
const LINE: &str = "LEVEL_TRIGGER_LINE";
const LINE2: &str = "LEVEL_TRIGGER_LINE2";
const CAVE: &str = "LEVEL_AMBIENT_CAVE";
const CHAINS: &str = "LEVEL_AMBIENT_CHAINS";
const CRIES: &str = "LEVEL_AMBIENT_CRIES";

/// Claw's feet rest on y = 899 + 60 on the flat level.
const FLOOR_Y: i32 = 899;

/// A sound object record as the level files place them: always active
/// until its logic says otherwise, hidden, its sound named by the
/// animation string.
fn spec(logic: &str, x: i32, y: i32, key: &str) -> ObjectSpec {
    let mut s = ObjectSpec::at(logic, x, y, 9000);
    s.record[0] = x;
    s.record[10] = 2;
    s.record[11] = 1;
    s.image_set = "GAME_SOUNDICON".into();
    s.anim = key.into();
    s
}

fn with(mut s: ObjectSpec, f: impl FnOnce(&mut [i32; 71])) -> ObjectSpec {
    f(&mut s.record);
    s
}

/// The flat level with `objects` and the test sounds registered.
fn level(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut l = flat_level();
    for (key, ms) in [
        (PUDDLE, 500),
        (DRIP, 300),
        (LINE, 1000),
        (LINE2, 800),
        (CAVE, 9000),
        (CHAINS, 4000),
        (CRIES, 5000),
    ] {
        l.sounds.insert(key.to_string(), Some(ms));
    }
    l.objects = objects;
    l
}

fn sim(objects: Vec<ObjectSpec>) -> Sim {
    Sim::new(level(objects), SimConfig::default())
}

fn step(sim: &mut Sim, input: TickInput) -> Vec<AudioCue> {
    sim.step(input).audio
}

fn idle_cues(sim: &mut Sim, n: usize) -> Vec<AudioCue> {
    (0..n)
        .flat_map(|_| step(sim, TickInput::default()))
        .collect()
}

fn effect(key: &str) -> AudioCue {
    AudioCue::Play(SoundCue {
        key: key.into(),
        ..SoundCue::default()
    })
}

fn voice(key: &str) -> AudioCue {
    AudioCue::Play(SoundCue {
        key: key.into(),
        channel: SoundChannel::Voice,
        ..SoundCue::default()
    })
}

fn count(cues: &[AudioCue], want: &AudioCue) -> usize {
    cues.iter().filter(|c| *c == want).count()
}

/// Ambient cues of `key`, as (volume, pan) for a Play and None for a Stop.
fn ambient(cues: &[AudioCue], handle: u32) -> Vec<Option<(u8, i8)>> {
    cues.iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) if s.voice == Some(handle) => {
                assert!(s.looped && s.channel == SoundChannel::Ambient, "{s:?}");
                Some(Some((s.volume, s.pan)))
            }
            AudioCue::Stop { voice } if *voice == handle => Some(None),
            _ => None,
        })
        .collect()
}

fn manager(sim: &Sim) -> &AmbientManager {
    sound_object::manager_of(sim.objects()).expect("an ambient manager")
}

fn handle(sim: &Sim, key: &str) -> u32 {
    manager(sim).voices[key].handle
}

fn manager_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find_map(|(id, o)| {
            matches!(
                o.data,
                ObjectData::Sound(SoundData {
                    part: SoundPart::Manager(_)
                })
            )
            .then_some(id)
        })
        .expect("an ambient manager")
}

/// Claw dies and comes back, as the manager sees it: one call with Claw
/// dying, one after.
fn die_and_return(sim: &mut Sim) {
    let m = manager_id(sim);
    let claw = sim.player().map(|o| o.state).unwrap();
    for s in [state::DEATH, claw] {
        sim.with_world(|w| {
            let id = w.claw_id.unwrap();
            w.objects.get_mut(id).unwrap().state = s;
            sound_object::tick(w, m);
        });
    }
}

fn put_claw(sim: &mut Sim, x: i32) {
    sim.teleport_player(x, FLOOR_Y);
}

fn exists(sim: &Sim, logic: &str) -> bool {
    sim.objects().all().any(|(_, o)| &*o.logic == logic)
}

// ------------------------------------------------------------- triggers

#[test]
fn a_trigger_plays_once_per_life() {
    let mut sim = sim(vec![spec("SoundTrigger", 1300, FLOOR_Y, PUDDLE)]);
    let cues = idle_cues(&mut sim, 3);
    assert_eq!(count(&cues, &effect(PUDDLE)), 0, "Claw is 300 px away");
    // Its square reaches 64 px each way, edges included.
    put_claw(&mut sim, 1300 - 65);
    assert_eq!(count(&idle_cues(&mut sim, 2), &effect(PUDDLE)), 0);
    put_claw(&mut sim, 1300 - 64);
    let cues = step(&mut sim, TickInput::default());
    assert_eq!(cues, vec![effect(PUDDLE)]);
    // Staying, leaving and coming back plays nothing more in this life.
    let mut cues = idle_cues(&mut sim, 60);
    put_claw(&mut sim, 1100);
    cues.extend(idle_cues(&mut sim, 5));
    put_claw(&mut sim, 1300);
    cues.extend(idle_cues(&mut sim, 60));
    assert_eq!(count(&cues, &effect(PUDDLE)), 0);
    // After a death it plays again.
    die_and_return(&mut sim);
    assert_eq!(count(&idle_cues(&mut sim, 2), &effect(PUDDLE)), 1);
    assert!(exists(&sim, "SoundTrigger"));
}

#[test]
fn smarts_counts_the_uses() {
    let once = with(spec("SmallSoundTrigger", 1300, FLOOR_Y, DRIP), |r| {
        r[17] = 1
    });
    let twice = with(spec("BigSoundTrigger", 2300, FLOOR_Y, PUDDLE), |r| {
        r[17] = 2
    });
    let mut sim = sim(vec![once, twice]);
    put_claw(&mut sim, 1300);
    assert_eq!(count(&idle_cues(&mut sim, 3), &effect(DRIP)), 1);
    assert!(!exists(&sim, "SmallSoundTrigger"), "smarts 1 removes it");
    put_claw(&mut sim, 2300);
    let mut plays = count(&idle_cues(&mut sim, 30), &effect(PUDDLE));
    for _ in 0..3 {
        die_and_return(&mut sim);
        plays += count(&idle_cues(&mut sim, 30), &effect(PUDDLE));
    }
    assert_eq!(plays, 2, "smarts 2 plays in two lives");
    assert!(!exists(&sim, "BigSoundTrigger"));
}

#[test]
fn smarts_minus_one_plays_on_every_entry() {
    let s = with(spec("SoundTrigger", 1300, FLOOR_Y, PUDDLE), |r| r[17] = -1);
    let mut sim = sim(vec![s]);
    idle(&mut sim, 2);
    put_claw(&mut sim, 1300);
    assert_eq!(step(&mut sim, TickInput::default()), vec![effect(PUDDLE)]);
    // Staying inside plays nothing more.
    assert_eq!(count(&idle_cues(&mut sim, 60), &effect(PUDDLE)), 0);
    // Out and back in: it plays again.
    put_claw(&mut sim, 1100);
    step(&mut sim, TickInput::default());
    put_claw(&mut sim, 1300);
    assert_eq!(count(&idle_cues(&mut sim, 2), &effect(PUDDLE)), 1);
    // Out and back in before the sound's length plus 100 ms: it waits for
    // that time, then for Claw to leave again.
    put_claw(&mut sim, 1100);
    let mut cues = idle_cues(&mut sim, 5);
    put_claw(&mut sim, 1300);
    cues.extend(idle_cues(&mut sim, 60));
    assert_eq!(count(&cues, &effect(PUDDLE)), 0);
    put_claw(&mut sim, 1100);
    step(&mut sim, TickInput::default());
    put_claw(&mut sim, 1300);
    assert_eq!(count(&idle_cues(&mut sim, 2), &effect(PUDDLE)), 1);
}

#[test]
fn the_wait_after_a_play_is_the_sound_length_plus_100_ms() {
    let s = with(spec("SoundTrigger", 1300, FLOOR_Y, PUDDLE), |r| r[17] = -1);
    let mut sim = sim(vec![s]);
    put_claw(&mut sim, 1300);
    let mut fired = Vec::new();
    for t in 0..200 {
        let cues = step(&mut sim, TickInput::default());
        if cues.contains(&effect(PUDDLE)) {
            fired.push(t);
            // Leave at once; the trigger watches again only after the wait.
            put_claw(&mut sim, 1100);
        } else if t % 2 == 1 {
            put_claw(&mut sim, 1300);
        } else {
            put_claw(&mut sim, 1100);
        }
    }
    assert!(fired.len() >= 3, "{fired:?}");
    for pair in fired.windows(2) {
        let gap_ms = (pair[1] - pair[0]) * 18;
        assert!((600..=650).contains(&gap_ms), "{fired:?}");
    }
}

#[test]
fn an_unknown_sound_plays_nothing_but_counts_the_use() {
    let s = with(
        spec("SoundTrigger", 1300, FLOOR_Y, "LEVEL_NOT_THERE"),
        |r| r[17] = 1,
    );
    let mut sim = sim(vec![s]);
    put_claw(&mut sim, 1300);
    let cues = idle_cues(&mut sim, 3);
    assert!(
        cues.iter()
            .all(|c| !matches!(c, AudioCue::Play(s) if s.key == "LEVEL_NOT_THERE"))
    );
    assert!(!exists(&sim, "SoundTrigger"));
}

#[test]
fn triggers_watch_only_while_awake() {
    // 2000 px away, the trigger is asleep when Claw is teleported there and
    // keeps its state; it fires once it wakes with Claw inside.
    let mut sim = sim(vec![spec("SoundTrigger", 3000, FLOOR_Y, PUDDLE)]);
    idle(&mut sim, 3);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "SoundTrigger")
        .map(|(id, _)| id)
        .unwrap();
    assert!(!sim.objects().get(id).unwrap().is_listed());
    put_claw(&mut sim, 3000);
    assert_eq!(count(&idle_cues(&mut sim, 5), &effect(PUDDLE)), 1);
}

// --------------------------------------------------------------- dialog

fn exclamation(snap: &claw_sim::SimSnapshot) -> Option<(f64, f64)> {
    snap.renderables
        .iter()
        .find(|r| r.image_set_key == sound::EXCLAMATION_IMAGE_SET)
        .map(|r| (r.x, r.y))
}

#[test]
fn a_dialog_trigger_says_its_line_once_on_the_voice_channel() {
    let s = with(
        spec("ClawDialogSmallSoundTrigger", 1300, FLOOR_Y, LINE),
        |r| r[17] = 1,
    );
    let mut sim = sim(vec![s]);
    idle(&mut sim, 2);
    put_claw(&mut sim, 1300);
    let snap = sim.step(TickInput::default());
    assert_eq!(snap.audio, vec![voice(LINE)]);
    assert!(!exists(&sim, "ClawDialogSmallSoundTrigger"), "smarts 1");
    // The exclamation mark stands over Claw for the line's length.
    let claw = sim.player().unwrap().clone();
    let top = claw.move_rect.top;
    let mut shown = 0;
    for _ in 0..80 {
        let snap = sim.step(TickInput::default());
        let claw = sim.player().unwrap();
        match exclamation(&snap) {
            Some(at) => {
                shown += 1;
                assert_eq!(at, (f64::from(claw.x), f64::from(claw.y + top - 30)));
            }
            None => break,
        }
    }
    assert_eq!(shown, 1000 / 18, "shown while the line runs");
    // Dying and coming back does not bring a smarts 1 line back.
    die_and_return(&mut sim);
    assert_eq!(count(&idle_cues(&mut sim, 10), &voice(LINE)), 0);
}

#[test]
fn dialog_lines_do_not_overlap_but_count_as_used() {
    // Claw's first line still runs when he reaches the second trigger, so
    // that line is refused, and the trigger still counts its use.
    let a = spec("ClawDialogSmallSoundTrigger", 1300, FLOOR_Y, LINE);
    let b = with(
        spec("ClawDialogBigSoundTrigger", 1500, FLOOR_Y, LINE2),
        |r| r[17] = 1,
    );
    let mut sim = sim(vec![a, b]);
    idle(&mut sim, 2);
    put_claw(&mut sim, 1300);
    let mut cues = idle_cues(&mut sim, 2);
    put_claw(&mut sim, 1500);
    cues.extend(idle_cues(&mut sim, 120));
    assert_eq!(count(&cues, &voice(LINE)), 1);
    assert_eq!(count(&cues, &voice(LINE2)), 0);
    assert!(!exists(&sim, "ClawDialogBigSoundTrigger"));
    // The smarts 0 line comes back after a death.
    put_claw(&mut sim, 1300);
    die_and_return(&mut sim);
    assert_eq!(count(&idle_cues(&mut sim, 5), &voice(LINE)), 1);
    assert!(
        exists(&sim, "ClawDialogSmallSoundTrigger"),
        "dialog triggers stay"
    );
}

#[test]
fn claw_refuses_lines_while_dying_unless_forced() {
    let mut sim = sim(Vec::new());
    idle(&mut sim, 2);
    let said = sim.with_world(|w| {
        let id = w.claw_id.unwrap();
        w.objects.get_mut(id).unwrap().state = state::DEATH;
        let a = sound::claw_say(w, LINE, false);
        let b = sound::claw_say(w, LINE2, true);
        let c = sound::claw_say(w, "LEVEL_NOT_THERE", true);
        w.objects.get_mut(id).unwrap().state = state::STAND;
        let d = sound::claw_say(w, LINE, false);
        (a, b, c, d, sound::claw_is_speaking(w))
    });
    assert_eq!(said, (false, true, false, false, true));
    let cues = step(&mut sim, TickInput::default());
    assert_eq!(cues, vec![voice(LINE2)]);
}

#[test]
fn enemy_lines_have_their_own_turn_and_end_with_the_speaker() {
    let mut sim = sim(Vec::new());
    idle(&mut sim, 2);
    let enemy = sim.spawn(Object::new(
        0,
        1200,
        FLOOR_Y,
        50,
        "Officer".into(),
        LogicKind::Inert,
    ));
    let r = sim.with_world(|w| {
        let a = sound::claw_say(w, LINE, false);
        let b = sound::enemy_say(w, enemy, LINE2, false);
        let c = sound::enemy_say(w, enemy, LINE, false);
        (a, b, c, sound::enemy_is_speaking(w, enemy))
    });
    assert_eq!(r, (true, true, false, true));
    let cues = step(&mut sim, TickInput::default());
    assert_eq!(
        cues,
        vec![
            voice(LINE),
            AudioCue::Play(SoundCue {
                key: LINE2.into(),
                volume: sound::ENEMY_VOICE_VOLUME,
                channel: SoundChannel::Voice,
                ..SoundCue::default()
            }),
        ]
    );
    sim.with_world(|w| sound::enemy_silence(w, enemy));
    step(&mut sim, TickInput::default());
    assert!(!sim.with_world(|w| sound::enemy_is_speaking(w, enemy)));
    assert!(sim.with_world(|w| sound::claw_is_speaking(w)));
}

// -------------------------------------------------------- ambient loops

#[test]
fn a_global_loop_starts_at_level_load() {
    let g = with(spec("GlobalAmbientSound", 40, 40, CAVE), |r| r[16] = 60);
    let mut sim = sim(vec![g]);
    let cues = step(&mut sim, TickInput::default());
    let h = handle(&sim, CAVE);
    assert_eq!(h, AMBIENT_VOICE_BASE);
    assert_eq!(ambient(&cues, h), vec![Some((60, 0))]);
    assert!(!exists(&sim, "GlobalAmbientSound"), "the loop outlives it");
    // It plays on unchanged wherever Claw goes.
    put_claw(&mut sim, 9000);
    assert!(ambient(&idle_cues(&mut sim, 300), h).is_empty());
}

#[test]
fn an_area_loop_fades_in_and_out_with_the_view() {
    // The area covers x 1500..2500 of the level; the view centre follows
    // Claw.
    let a = with(spec("AmbientSound", 2000, 800, CHAINS), |r| {
        r[51] = 1500;
        r[52] = 0;
        r[53] = 2500;
        r[54] = 1088;
    });
    let mut sim = sim(vec![a]);
    let cues = idle_cues(&mut sim, 30);
    assert!(cues.is_empty(), "outside the area: {cues:?}");
    put_claw(&mut sim, 2000);
    let cues = idle_cues(&mut sim, 70);
    let h = handle(&sim, CHAINS);
    let levels: Vec<(u8, i8)> = ambient(&cues, h).into_iter().map(Option::unwrap).collect();
    assert_eq!(levels.first(), Some(&(1, 0)), "starts quietly");
    assert_eq!(levels.last(), Some(&(100, 0)), "up to full level");
    assert!(
        levels.windows(2).all(|p| p[0].0 < p[1].0),
        "rises: {levels:?}"
    );
    // 1000 ms at 18 ms a tick.
    assert_eq!(levels.len(), 57);
    put_claw(&mut sim, 6000);
    let cues = idle_cues(&mut sim, 70);
    let fade = ambient(&cues, h);
    assert_eq!(fade.last(), Some(&None), "stops at the end of the fade");
    let levels: Vec<u8> = fade.iter().flatten().map(|l| l.0).collect();
    assert!(levels.windows(2).all(|p| p[0] > p[1]), "falls: {levels:?}");
    // From 99 down to 1 over a second, then the stop.
    assert_eq!(levels.len(), 55);
}

#[test]
fn a_spot_loop_follows_distance_and_side_and_stops_when_it_sleeps() {
    let s = with(spec("SpotAmbientSound", 1600, FLOOR_Y, CHAINS), |r| {
        r[16] = 0
    });
    let mut sim = sim(vec![s]);
    let mut stops = 0;
    let mut seen = Vec::new();
    // Walk right past the spot and on until it is out of range.
    for _ in 0..ticks(5000) {
        let cues = step(&mut sim, hold(RIGHT));
        let Some(h) = manager(&sim).voices.get(CHAINS).map(|v| v.handle) else {
            continue;
        };
        let centre = sound::view_centre(sim.view().rect);
        let want = spot_level_pan(1600, FLOOR_Y, centre.0, centre.1);
        for c in ambient(&cues, h) {
            match c {
                Some((v, p)) => {
                    assert_eq!(Some((i32::from(v), i32::from(p))), want);
                    seen.push((v, p));
                }
                None => {
                    assert_eq!(want, None, "stops only out of range");
                    stops += 1;
                }
            }
        }
    }
    assert!(seen.iter().any(|&(_, p)| p > 0), "heard on the right");
    assert!(seen.iter().any(|&(_, p)| p < 0), "then on the left");
    assert!(seen.iter().any(|&(v, _)| v > 90), "loud when close");
    assert_eq!(stops, 1);
    // Farther away the spot goes to sleep and its loop goes with it.
    let h = handle(&sim, CHAINS);
    put_claw(&mut sim, 9000);
    let cues = idle_cues(&mut sim, 5);
    assert!(ambient(&cues, h).is_empty(), "already stopped");
    assert!(
        manager(&sim).entries.is_empty(),
        "unregistered while asleep"
    );
    // Back near it, the spot registers again and the loop restarts.
    put_claw(&mut sim, 1700);
    let cues = idle_cues(&mut sim, 5);
    assert!(matches!(ambient(&cues, h).first(), Some(Some(_))));
    assert_eq!(manager(&sim).entries.len(), 1);
}

#[test]
fn loops_of_one_sound_share_a_voice_at_the_loudest_level() {
    let near = spec("SpotAmbientSound", 1100, FLOOR_Y, CHAINS);
    let far = with(spec("SpotAmbientSound", 1500, FLOOR_Y, CHAINS), |r| {
        r[0] = 7
    });
    let mut sim = sim(vec![near, far]);
    let cues = idle_cues(&mut sim, 3);
    let plays: Vec<&SoundCue> = cues
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s),
            _ => None,
        })
        .collect();
    assert!(!plays.is_empty());
    assert!(plays.iter().all(|s| s.voice == Some(AMBIENT_VOICE_BASE)));
    let centre = sound::view_centre(sim.view().rect);
    let want = spot_level_pan(1100, FLOOR_Y, centre.0, centre.1).unwrap();
    let last = plays.last().unwrap();
    assert_eq!((i32::from(last.volume), i32::from(last.pan)), want);
    assert_eq!(manager(&sim).entries.len(), 2);
}

#[test]
fn a_timed_loop_comes_and_goes() {
    let t = with(spec("GlobalAmbientSound", 40, 40, CRIES), |r| {
        r[16] = 75;
        r[19] = 1000;
        r[20] = 1500;
        r[21] = 2000;
        r[22] = 3000;
    });
    let mut sim = sim(vec![t]);
    let mut cues = Vec::new();
    let mut first = None;
    for t in 0..ticks(20_000) {
        let c = step(&mut sim, TickInput::default());
        if first.is_none() && c.iter().any(|c| matches!(c, AudioCue::Play(_))) {
            first = Some(t + 1);
        }
        cues.extend(c);
    }
    let h = handle(&sim, CRIES);
    let a = ambient(&cues, h);
    let starts = a
        .windows(2)
        .filter(|p| p[0].is_none() && p[1].is_some())
        .count()
        + 1;
    let stops = a.iter().filter(|c| c.is_none()).count();
    assert!(starts >= 3, "{starts} starts");
    assert!(stops >= 2, "{stops} stops");
    let peak = a.iter().flatten().map(|l| l.0).max().unwrap();
    assert_eq!(peak, 75);
    // Two off periods (2 to 3 s each) pass before it is first heard.
    let first_ms = first.unwrap() as u64 * 18;
    assert!((4000..=6100).contains(&first_ms), "{first_ms}");
}

#[test]
fn deaths_are_counted_when_claw_comes_back() {
    let mut sim = sim(vec![spec("SoundTrigger", 3000, FLOOR_Y, PUDDLE)]);
    idle(&mut sim, 2);
    assert_eq!(manager(&sim).attempt, 0);
    die_and_return(&mut sim);
    die_and_return(&mut sim);
    assert_eq!(manager(&sim).attempt, 2);
}

// ------------------------------------------------------ placed sounds

#[test]
fn a_placed_sound_pans_to_its_side_of_the_screen() {
    let mut sim = sim(Vec::new());
    idle(&mut sim, 30);
    let view = sim.view().rect;
    let (cx, _) = sound::view_centre(view);
    let mut pans = Vec::new();
    for x in [view.left, cx - 100, cx, cx + 100, view.right] {
        let id = sim.spawn(Object::new(
            0,
            x,
            FLOOR_Y,
            0,
            "Thing".into(),
            LogicKind::Inert,
        ));
        sim.with_world(|w| sound::play_from(w, id, PUDDLE, SoundChannel::Effect));
        let cues = step(&mut sim, TickInput::default());
        let AudioCue::Play(s) = &cues[0] else {
            panic!("{cues:?}")
        };
        assert_eq!((s.volume, s.pitch, s.looped), (100, 0, false));
        pans.push(s.pan);
    }
    assert_eq!(pans, vec![-100, -46, 0, 46, 100]);
    // Without an object it plays centred.
    sim.with_world(|w| sound::play_from(w, 99_999, PUDDLE, SoundChannel::Effect));
    assert_eq!(step(&mut sim, TickInput::default()), vec![effect(PUDDLE)]);
}

// ------------------------------------------------------------ formulas

#[test]
fn formula_table() {
    // Trigger squares by name.
    for (name, r) in [
        ("SoundTrigger", (-64, -64, 64, 64)),
        ("BigSoundTrigger", (-128, -128, 128, 128)),
        ("SmallSoundTrigger", (-32, -32, 32, 32)),
        ("HugeSoundTrigger", (-256, -256, 256, 256)),
        ("TinySoundTrigger", (-16, -16, 16, 16)),
        ("TallSoundTrigger", (-32, -100, 32, 100)),
        ("WideSoundTrigger", (-100, -32, 100, 32)),
        ("ClawDialogSoundTrigger", (-64, -64, 64, 64)),
        ("ClawDialogBigSoundTrigger", (-128, -128, 128, 128)),
        ("ClawDialogSmallSoundTrigger", (-32, -32, 32, 32)),
        ("ClawDialogTallSoundTrigger", (-32, -100, 32, 100)),
    ] {
        assert_eq!(trigger_area(name), Rect::new(r.0, r.1, r.2, r.3), "{name}");
    }
    // Placed sounds: balance by offset from the view centre.
    for (dx, pan) in [
        (0, 0),
        (2, 0),
        (3, 1),
        (100, 46),
        (-100, -46),
        (PAN_RANGE_PX, 100),
        (-PAN_RANGE_PX - 1, -100),
        (2560, 100),
    ] {
        assert_eq!(sound::positional_pan(5000 + dx, 5000), pan, "{dx}");
    }
    // Ambient level with the record's percent.
    for (level, percent, v) in [
        (100, 0, 100),
        (100, -5, 100),
        (100, 60, 60),
        (100, 75, 75),
        (99, 50, 49),
        (100, 150, 100),
        (0, 60, 0),
        (37, 100, 37),
    ] {
        assert_eq!(ambient_volume(level, percent), v, "{level} {percent}");
    }
    // Spot level and balance from the distance.
    for (sx, sy, cx, cy, want) in [
        (1000, 1000, 1000, 1000, Some((100, 0))),
        (1060, 1000, 1000, 1000, Some((90, 15))),
        (940, 1000, 1000, 1000, Some((90, -15))),
        (1000, 1300, 1000, 1000, Some((50, 0))),
        (1300, 1400, 1000, 1000, Some((17, 75))),
        (1600, 1000, 1000, 1000, Some((0, 100))),
        (1640, 1000, 1000, 1000, Some((0, 100))),
        (1641, 1000, 1000, 1000, None),
        (1000, 359, 1000, 1000, None),
        (600, 600, 1000, 1000, Some((6, -100))),
    ] {
        assert_eq!(spot_level_pan(sx, sy, cx, cy), want, "{sx},{sy}");
    }
    // Fades: linear over 1000 ms.
    for (from, to, t, v) in [
        (1, 100, 0, 1),
        (1, 100, 18, 2),
        (1, 60, 540, 32),
        (1, 100, 1000, 100),
        (80, 0, 250, 60),
        (80, 0, 2000, 0),
    ] {
        assert_eq!(fade_volume(from, to, t), v, "{from}->{to} at {t}");
    }
    // The random timer bounds.
    let mut rng = Rng::new(3);
    for _ in 0..1000 {
        let r = sound_object::rand_range(&mut rng, 30_000, 60_000);
        assert!((30_000..=60_000).contains(&r));
    }
}

// --------------------------------------------------------- display rates

/// A course with every kind of sound object along Claw's way.
fn course() -> SimLevel {
    level(vec![
        with(spec("SoundTrigger", 1300, FLOOR_Y, PUDDLE), |r| r[17] = -1),
        spec("BigSoundTrigger", 1900, FLOOR_Y, DRIP),
        with(spec("SmallSoundTrigger", 2400, FLOOR_Y, DRIP), |r| {
            r[17] = 1
        }),
        spec("ClawDialogSmallSoundTrigger", 1600, FLOOR_Y, LINE),
        with(
            spec("ClawDialogBigSoundTrigger", 2700, FLOOR_Y, LINE2),
            |r| r[17] = 1,
        ),
        with(spec("GlobalAmbientSound", 40, 40, CAVE), |r| r[16] = 60),
        with(spec("GlobalAmbientSound", 40, 80, CRIES), |r| {
            r[16] = 75;
            r[19] = 400;
            r[20] = 900;
            r[21] = 500;
            r[22] = 1500;
        }),
        with(spec("AmbientSound", 2500, 800, CHAINS), |r| {
            r[51] = 2200;
            r[53] = 2900;
            r[54] = 1088;
        }),
        spec("SpotAmbientSound", 3200, FLOOR_Y, CHAINS),
    ])
}

fn walk() -> InputScript {
    script(&[
        (RIGHT, 6000),
        (0, 500),
        (LEFT, 3000),
        (RIGHT | JUMP, 800),
        (RIGHT, 4000),
        (LEFT, 7000),
        (0, 2000),
    ])
}

fn cue_log(sim: &Sim) -> String {
    format!("{}: {:?}\n", sim.tick(), sim.last_audio())
}

fn log_direct(script: &InputScript) -> String {
    let mut sim = Sim::new(course(), SimConfig::default());
    let mut log = String::new();
    for &input in &script.ticks {
        sim.step(input);
        log.push_str(&cue_log(&sim));
    }
    log
}

/// Runs `script` as a display at `hz` would: button changes reach a latch
/// at the first frame at or after their time, and ticks run once their
/// end has passed. With `jitter`, each frame lasts from half a frame to a
/// full one.
fn log_at_rate(script: &InputScript, hz: u64, jitter: bool) -> String {
    let mut sim = Sim::new(course(), SimConfig::default());
    let mut latch = InputLatch::new();
    let events = script.events();
    let mut rng = Rng::new(hz as u32);
    let mut next_event = 0;
    let mut log = String::new();
    let mut tick = 0u64;
    let n = script.ticks.len() as u64;
    let frame_us = 1_000_000 / hz;
    let mut now_us = 0u64;
    while tick < n {
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            log.push_str(&cue_log(&sim));
            tick += 1;
        }
        now_us += if jitter {
            frame_us / 2 + u64::from(rng.below(frame_us as u32 / 2 + 1))
        } else {
            frame_us
        };
    }
    log
}

#[test]
fn cue_logs_are_identical_at_60_144_and_240_hz() {
    let s = walk();
    let direct = log_direct(&s);
    for key in [PUDDLE, DRIP, LINE, CAVE, CHAINS] {
        assert!(direct.contains(key), "the walk hears {key}");
    }
    for hz in [60, 144, 240] {
        assert_eq!(log_at_rate(&s, hz, false), direct, "{hz} Hz");
        assert_eq!(log_at_rate(&s, hz, true), direct, "{hz} Hz with jitter");
    }
}
