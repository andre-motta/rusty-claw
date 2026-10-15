//! The deterministic software mixer.
//!
//! A fixed pool of voices plays [`Pcm`] samples into interleaved stereo `f32`
//! at the output rate. Each voice carries the game's volume curve, pan and
//! pitch, and is resampled to the output rate by linear interpolation. Playing
//! and rendering never allocate, so the mixer can run inside an audio callback.
//!
//! Time: the mixer keeps its own clock, advanced by the frames it renders. The
//! per-sound replay throttle measures against it.
//!
//! Channels: each channel plays at its configured volume
//! ([`crate::settings::channel_volume`]), and a request's volume is a percent
//! of it. A request with a handle that matches a playing voice of the same
//! sound updates that voice's volume, pan and pitch instead of starting
//! another one.

use std::fmt;
use std::sync::Arc;

use crate::bank::{Pcm, SoundBank, SoundId};
use crate::curve;
use crate::settings::{AudioSettings, Channel, MAX_VOICES, MixSettings, PlayRequest};

/// A music generator mixed under the sound effects at the music volume.
pub trait MusicSource: Send {
    /// Fills `out` (interleaved stereo, already zeroed) with the next
    /// `out.len() / 2` frames at `rate` Hz. Must not allocate or block.
    fn render(&mut self, out: &mut [f32], rate: u32);
}

/// One started voice, valid until it ends or is reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceId {
    slot: u16,
    seq: u64,
}

impl VoiceId {
    /// Pool slot the voice plays in.
    pub fn slot(self) -> u16 {
        self.slot
    }

    /// Start order of the voice across the mixer's life.
    pub fn seq(self) -> u64 {
        self.seq
    }
}

/// The play parameters of a [`PlayRequest`] without its name, as plain data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayParams {
    /// 0..=100 relative to the configured sound volume.
    pub volume: u8,
    /// -100..=100.
    pub pan: i8,
    /// Percent change of the sample rate.
    pub pitch: i16,
    /// Loop until stopped.
    pub looped: bool,
    /// Switch and volume the sound plays under.
    pub channel: Channel,
    /// Caller handle for a later stop.
    pub handle: Option<u32>,
}

impl From<&PlayRequest> for PlayParams {
    fn from(r: &PlayRequest) -> PlayParams {
        PlayParams {
            volume: r.volume,
            pan: r.pan,
            pitch: r.pitch,
            looped: r.looped,
            channel: r.channel,
            handle: r.handle,
        }
    }
}

/// Frames of music rendered per inner step; the scratch buffer holds this many
/// stereo frames.
const SCRATCH_FRAMES: usize = 1024;

/// 32.32 fixed-point one.
const ONE: u64 = 1 << 32;

#[derive(Debug, Clone, Default)]
struct Voice {
    active: bool,
    pcm: Option<Arc<Pcm>>,
    /// Last sound started in this slot, used to prefer reusing its slot.
    sound: Option<SoundId>,
    params: PlayParams,
    seq: u64,
    /// Read position in source frames, 32.32 fixed point.
    pos: u64,
    /// Source frames per output frame, 32.32 fixed point.
    step: u64,
    frequency: u32,
    frames: u64,
    gain_l: f32,
    gain_r: f32,
}

impl Voice {
    fn end(&mut self) {
        self.active = false;
        self.pcm = None;
        self.pos = 0;
    }
}

/// The software mixer. See the module documentation.
pub struct Mixer {
    out_rate: u32,
    voices: Box<[Voice]>,
    settings: MixSettings,
    next_seq: u64,
    clock_base_ms: u64,
    clock_frames: u64,
    game_paused: bool,
    all_paused: bool,
    music: Option<Box<dyn MusicSource>>,
    music_gain: f32,
    scratch: Box<[f32]>,
}

impl fmt::Debug for Mixer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mixer")
            .field("out_rate", &self.out_rate)
            .field("active_voices", &self.active_voices())
            .field("settings", &self.settings)
            .field("game_paused", &self.game_paused)
            .field("all_paused", &self.all_paused)
            .field("music", &self.music.is_some())
            .finish()
    }
}

impl Mixer {
    /// A mixer rendering at `out_rate` Hz with default settings.
    pub fn new(out_rate: u32) -> Mixer {
        let settings = MixSettings::from(&AudioSettings::default());
        Mixer {
            out_rate: out_rate.max(1),
            voices: vec![Voice::default(); usize::from(MAX_VOICES)].into_boxed_slice(),
            settings,
            next_seq: 0,
            clock_base_ms: 0,
            clock_frames: 0,
            game_paused: false,
            all_paused: false,
            music: None,
            music_gain: curve::music_gain(settings.music_volume),
            scratch: vec![0.0; SCRATCH_FRAMES * 2].into_boxed_slice(),
        }
    }

    /// Output rate in Hz.
    pub fn out_rate(&self) -> u32 {
        self.out_rate
    }

    /// Changes the output rate. Playing voices keep their pitch and position,
    /// and the clock keeps counting from its current time.
    pub fn set_out_rate(&mut self, out_rate: u32) {
        let out_rate = out_rate.max(1);
        if out_rate == self.out_rate {
            return;
        }
        self.clock_base_ms = self.now_ms();
        self.clock_frames = 0;
        self.out_rate = out_rate;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.step = step_for(v.frequency, out_rate);
        }
    }

    /// Mixer time in milliseconds: the frames rendered so far.
    pub fn now_ms(&self) -> u64 {
        self.clock_base_ms + self.clock_frames * 1000 / u64::from(self.out_rate)
    }

    /// Applies new settings. Playing voices follow volume changes; voices on a
    /// channel that is now switched off, and voices above a lowered cap, stop.
    pub fn set_settings(&mut self, settings: &AudioSettings) {
        self.apply_settings(MixSettings::from(settings));
    }

    pub(crate) fn apply_settings(&mut self, settings: MixSettings) {
        self.settings = settings;
        self.music_gain = if settings.enabled && settings.music {
            curve::music_gain(settings.music_volume)
        } else {
            0.0
        };
        let cap = usize::from(settings.voice_cap);
        for (slot, v) in self.voices.iter_mut().enumerate() {
            if !v.active {
                continue;
            }
            if slot >= cap || !settings.channel_on(v.params.channel) {
                v.end();
            } else {
                (v.gain_l, v.gain_r) = voice_gains(&settings, &v.params);
            }
        }
    }

    /// Starts the sound named by `request`. Returns `None` when the name is
    /// unknown, the channel is off, the sound is throttled, the sample cannot
    /// play, or every voice is busy with a loop.
    pub fn play(&mut self, request: &PlayRequest, bank: &SoundBank) -> Option<VoiceId> {
        let id = bank.id(&request.key)?;
        self.play_sound(id, PlayParams::from(request), bank)
    }

    /// Starts the sound at `id` in `bank`. Same rules as [`Mixer::play`];
    /// never allocates.
    ///
    /// When `params` carries a handle that a playing voice of the same sound
    /// was started with, that voice is updated instead: it takes the new
    /// volume, pan, pitch and loop flag and keeps its position.
    pub fn play_sound(
        &mut self,
        id: SoundId,
        params: PlayParams,
        bank: &SoundBank,
    ) -> Option<VoiceId> {
        if !self.settings.channel_on(params.channel) {
            return None;
        }
        let pcm = bank.by_id(id)?;
        if !pcm.is_playable() {
            return None;
        }
        if let Some(handle) = params.handle
            && let Some(slot) = self
                .voices
                .iter()
                .position(|v| v.active && v.params.handle == Some(handle) && v.sound == Some(id))
        {
            let frequency = curve::pitch_frequency(pcm.rate, params.pitch);
            let (gain_l, gain_r) = voice_gains(&self.settings, &params);
            let out_rate = self.out_rate;
            let v = &mut self.voices[slot];
            v.params = params;
            v.frequency = frequency;
            v.step = step_for(frequency, out_rate);
            (v.gain_l, v.gain_r) = (gain_l, gain_r);
            return Some(VoiceId {
                slot: slot as u16,
                seq: v.seq,
            });
        }
        // The throttle is taken before a voice is found, so a play that finds
        // no voice still uses up the gap, as in the game.
        if !bank.throttle_pass(id, self.now_ms(), self.settings.min_replay_gap_ms) {
            return None;
        }
        let slot = self.pick_slot(id)?;
        let seq = self.next_seq;
        self.next_seq += 1;
        let frequency = curve::pitch_frequency(pcm.rate, params.pitch);
        let (gain_l, gain_r) = voice_gains(&self.settings, &params);
        let v = &mut self.voices[slot];
        *v = Voice {
            active: true,
            pcm: Some(Arc::clone(pcm)),
            sound: Some(id),
            params,
            seq,
            pos: 0,
            step: step_for(frequency, self.out_rate),
            frequency,
            frames: pcm.frames() as u64,
            gain_l,
            gain_r,
        };
        Some(VoiceId {
            slot: slot as u16,
            seq,
        })
    }

    /// Voice reuse: a free slot that last played the same sound, else any free
    /// slot, else the oldest one-shot voice. Loops are never taken over.
    fn pick_slot(&self, id: SoundId) -> Option<usize> {
        let pool = &self.voices[..usize::from(self.settings.voice_cap)];
        pool.iter()
            .position(|v| !v.active && v.sound == Some(id))
            .or_else(|| pool.iter().position(|v| !v.active))
            .or_else(|| {
                pool.iter()
                    .enumerate()
                    .filter(|(_, v)| !v.params.looped)
                    .min_by_key(|(_, v)| v.seq)
                    .map(|(i, _)| i)
            })
    }

    /// Stops every voice started with `handle`; returns how many stopped.
    pub fn stop(&mut self, handle: u32) -> usize {
        let mut n = 0;
        for v in self.voices.iter_mut() {
            if v.active && v.params.handle == Some(handle) {
                v.end();
                n += 1;
            }
        }
        n
    }

    /// Stops one voice; false when it already ended or its slot was reused.
    pub fn stop_voice(&mut self, id: VoiceId) -> bool {
        match self.voices.get_mut(usize::from(id.slot)) {
            Some(v) if v.active && v.seq == id.seq => {
                v.end();
                true
            }
            _ => false,
        }
    }

    /// Stops every voice. Music is not affected.
    pub fn stop_all(&mut self) {
        for v in self.voices.iter_mut() {
            v.end();
        }
    }

    /// Pauses the game, as the game's own pause does: every game sound
    /// (Effect, Voice and Ambient, looping or not) stops, the music holds
    /// where it is, and interface sounds keep playing. Sounds started while
    /// paused wait for the resume.
    pub fn pause_game(&mut self) {
        self.game_paused = true;
        for v in self.voices.iter_mut() {
            if v.active && v.params.channel.is_game() {
                v.end();
            }
        }
    }

    /// Resumes the game: the music continues where it was. The sounds the
    /// pause stopped stay stopped; the caller starts the ambient loops
    /// again.
    pub fn resume_game(&mut self) {
        self.game_paused = false;
    }

    /// Pauses every voice and the music.
    pub fn pause_all(&mut self) {
        self.all_paused = true;
    }

    /// Resumes what [`Mixer::pause_all`] paused. A separate game pause stays
    /// in force.
    pub fn resume_all(&mut self) {
        self.all_paused = false;
    }

    /// Whether the game channels are paused.
    pub fn game_paused(&self) -> bool {
        self.game_paused
    }

    /// Replaces the music source and returns the previous one, so the caller
    /// decides where it is dropped.
    pub fn set_music(
        &mut self,
        music: Option<Box<dyn MusicSource>>,
    ) -> Option<Box<dyn MusicSource>> {
        std::mem::replace(&mut self.music, music)
    }

    /// Whether a music source is set.
    pub fn has_music(&self) -> bool {
        self.music.is_some()
    }

    /// Number of playing (or paused) voices.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Whether the voice is still playing.
    pub fn is_active(&self, id: VoiceId) -> bool {
        self.voices
            .get(usize::from(id.slot))
            .is_some_and(|v| v.active && v.seq == id.seq)
    }

    /// Start order of the oldest playing voice.
    pub fn oldest_active_seq(&self) -> Option<u64> {
        self.voices.iter().filter(|v| v.active).map(|v| v.seq).min()
    }

    /// Start order the next voice will get.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Renders `out.len() / 2` stereo frames into `out` (interleaved left,
    /// right), replacing its contents, and advances the clock. A trailing odd
    /// sample is zeroed. Deterministic and allocation-free.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        let out = &mut out[..frames * 2];
        if !self.all_paused {
            for v in self.voices.iter_mut() {
                if v.active && !(self.game_paused && v.params.channel.is_game()) {
                    mix_voice(v, out);
                }
            }
            let music = if self.game_paused {
                None
            } else {
                self.music.as_mut()
            };
            if let Some(music) = music {
                let gain = self.music_gain;
                for chunk in out.chunks_mut(SCRATCH_FRAMES * 2) {
                    let scratch = &mut self.scratch[..chunk.len()];
                    scratch.fill(0.0);
                    music.render(scratch, self.out_rate);
                    for (o, m) in chunk.iter_mut().zip(scratch.iter()) {
                        *o += m * gain;
                    }
                }
            }
            for s in out.iter_mut() {
                *s = s.clamp(-1.0, 1.0);
            }
        }
        self.clock_frames += frames as u64;
    }
}

fn step_for(frequency: u32, out_rate: u32) -> u64 {
    (u64::from(frequency) << 32) / u64::from(out_rate)
}

fn voice_gains(settings: &MixSettings, params: &PlayParams) -> (f32, f32) {
    let v = curve::relative_volume(params.volume, settings.channel_volume(params.channel));
    let g = curve::gain(curve::attenuation(v));
    let (l, r) = curve::pan_gains(params.pan);
    (g * l, g * r)
}

#[inline]
fn frame_at(samples: &[i16], channels: usize, i: usize) -> (f32, f32) {
    const SCALE: f32 = 1.0 / 32768.0;
    if channels == 2 {
        (
            f32::from(samples[2 * i]) * SCALE,
            f32::from(samples[2 * i + 1]) * SCALE,
        )
    } else {
        let s = f32::from(samples[i]) * SCALE;
        (s, s)
    }
}

/// Adds one voice into `out` until it ends or `out` is full.
fn mix_voice(v: &mut Voice, out: &mut [f32]) {
    let Some(pcm) = v.pcm.as_ref() else {
        v.end();
        return;
    };
    let samples: &[i16] = &pcm.samples;
    let channels = usize::from(pcm.channels);
    let n = v.frames;
    let span = n << 32;
    let mut ended = false;
    for frame in out.chunks_exact_mut(2) {
        let i = (v.pos >> 32) as usize;
        let frac = ((v.pos & (ONE - 1)) >> 8) as f32 * (1.0 / 16_777_216.0);
        let (l0, r0) = frame_at(samples, channels, i);
        let (l1, r1) = if (i as u64) + 1 < n {
            frame_at(samples, channels, i + 1)
        } else if v.params.looped {
            frame_at(samples, channels, 0)
        } else {
            (0.0, 0.0)
        };
        frame[0] += (l0 + (l1 - l0) * frac) * v.gain_l;
        frame[1] += (r0 + (r1 - r0) * frac) * v.gain_r;
        v.pos += v.step;
        if v.pos >= span {
            if v.params.looped {
                v.pos %= span;
            } else {
                ended = true;
                break;
            }
        }
    }
    if ended {
        v.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 8000;

    fn tone(n: usize, seed: i32) -> Vec<i16> {
        (0..n)
            .map(|i| (((i as i32 * 37 + seed * 101) % 2001) - 1000) as i16 * 8)
            .collect()
    }

    fn bank(sounds: &[(&str, Pcm)]) -> SoundBank {
        let mut b = SoundBank::new();
        for (k, p) in sounds {
            b.insert(*k, Arc::new(p.clone()));
        }
        b
    }

    fn full_volume() -> AudioSettings {
        AudioSettings {
            sound_volume: 100,
            min_replay_gap_ms: 0,
            ..AudioSettings::default()
        }
    }

    fn mixer(settings: &AudioSettings) -> Mixer {
        let mut m = Mixer::new(RATE);
        m.set_settings(settings);
        m
    }

    fn req(key: &str) -> PlayRequest {
        PlayRequest::new(key, Channel::Effect)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() <= 1e-6
    }

    #[test]
    fn render_equals_computed_sum() {
        let a = tone(400, 1);
        let b = tone(300, 2);
        let bk = bank(&[
            ("A", Pcm::mono(RATE, a.clone())),
            ("B", Pcm::mono(RATE, b.clone())),
        ]);
        let mut m = mixer(&full_volume());
        m.play(&req("A"), &bk).unwrap();
        let mut rb = req("B");
        rb.volume = 50;
        rb.pan = -100;
        m.play(&rb, &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 500];
        m.render(&mut out);
        let gb = curve::gain(-1000);
        for i in 0..500 {
            let sa = a.get(i).map_or(0.0, |&s| f32::from(s) / 32768.0);
            let sb = b.get(i).map_or(0.0, |&s| f32::from(s) / 32768.0);
            assert!(close(out[2 * i], sa + sb * gb), "left {i}");
            assert!(close(out[2 * i + 1], sa), "right {i}");
        }
        assert_eq!(m.active_voices(), 0);
    }

    #[test]
    fn stereo_source_and_configured_volume() {
        let s: Vec<i16> = vec![1000, -2000, 3000, -4000];
        let bk = bank(&[(
            "S",
            Pcm {
                rate: RATE,
                channels: 2,
                samples: s.clone().into(),
            },
        )]);
        // Request volume 100 plays at the configured volume of 50.
        let mut m = mixer(&AudioSettings {
            sound_volume: 50,
            ..full_volume()
        });
        m.play(&req("S"), &bk).unwrap();
        let mut out = vec![0.0f32; 4];
        m.render(&mut out);
        let g = curve::gain(-1000);
        for (o, v) in out.iter().zip(&s) {
            assert!(close(*o, f32::from(*v) / 32768.0 * g));
        }
    }

    #[test]
    fn resampling_and_pitch() {
        let s: Vec<i16> = (0..64).map(|i| i * 100).collect();
        let bk = bank(&[
            ("DOUBLE", Pcm::mono(2 * RATE, s.clone())),
            ("HALF", Pcm::mono(RATE / 2, s.clone())),
        ]);
        let mut m = mixer(&full_volume());
        m.play(&req("DOUBLE"), &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 8];
        m.render(&mut out);
        for k in 0..8 {
            assert!(close(out[2 * k], f32::from(s[2 * k]) / 32768.0));
        }

        let mut m = mixer(&full_volume());
        m.play(&req("HALF"), &bk).unwrap();
        m.render(&mut out);
        for k in 0..8 {
            let i = k / 2;
            let e = if k % 2 == 0 {
                f32::from(s[i])
            } else {
                (f32::from(s[i]) + f32::from(s[i + 1])) / 2.0
            };
            assert!(close(out[2 * k], e / 32768.0), "frame {k}");
        }

        // Pitch +100 percent doubles the rate.
        let bk = bank(&[("P", Pcm::mono(RATE, s.clone()))]);
        let mut m = mixer(&full_volume());
        let mut r = req("P");
        r.pitch = 100;
        m.play(&r, &bk).unwrap();
        m.render(&mut out);
        for k in 0..8 {
            assert!(close(out[2 * k], f32::from(s[2 * k]) / 32768.0));
        }
    }

    #[test]
    fn throttle() {
        let bk = bank(&[("A", Pcm::mono(RATE, tone(8, 0)))]);
        let mut m = mixer(&AudioSettings {
            min_replay_gap_ms: 50,
            ..full_volume()
        });
        assert!(m.play(&req("A"), &bk).is_some());
        assert!(m.play(&req("A"), &bk).is_none());
        // 49 ms later: still throttled.
        let mut out = vec![0.0f32; 2 * (RATE as usize * 49 / 1000)];
        m.render(&mut out);
        assert_eq!(m.now_ms(), 49);
        assert!(m.play(&req("A"), &bk).is_none());
        let mut out = vec![0.0f32; 2 * (RATE as usize / 1000)];
        m.render(&mut out);
        assert_eq!(m.now_ms(), 50);
        assert!(m.play(&req("A"), &bk).is_some());
    }

    #[test]
    fn throttle_is_used_up_when_no_voice_is_free() {
        let bk = bank(&[
            ("L", Pcm::mono(RATE, tone(8, 0))),
            ("A", Pcm::mono(RATE, tone(8, 1))),
        ]);
        let mut m = mixer(&AudioSettings {
            voice_cap: 1,
            min_replay_gap_ms: 50,
            ..full_volume()
        });
        let mut l = req("L");
        l.looped = true;
        m.play(&l, &bk).unwrap();
        assert!(m.play(&req("A"), &bk).is_none());
        m.stop_all();
        assert!(m.play(&req("A"), &bk).is_none(), "gap was used up");
    }

    #[test]
    fn voice_reuse() {
        let bk = bank(&[
            ("A", Pcm::mono(RATE, tone(4, 0))),
            ("B", Pcm::mono(RATE, tone(4, 1))),
        ]);
        let mut m = mixer(&full_volume());
        let a = m.play(&req("A"), &bk).unwrap();
        let b = m.play(&req("B"), &bk).unwrap();
        assert_ne!(a.slot(), b.slot());
        let mut out = vec![0.0f32; 2 * 8];
        m.render(&mut out);
        assert_eq!(m.active_voices(), 0);
        assert!(!m.is_active(a));
        // B's free slot is preferred for B, A's for A.
        let b2 = m.play(&req("B"), &bk).unwrap();
        let a2 = m.play(&req("A"), &bk).unwrap();
        assert_eq!(b2.slot(), b.slot());
        assert_eq!(a2.slot(), a.slot());
        // A busy sound gets a second voice.
        let a3 = m.play(&req("A"), &bk).unwrap();
        assert!(a3.slot() != a2.slot() && a3.slot() != b2.slot());
        assert_eq!(m.active_voices(), 3);
    }

    #[test]
    fn cap_steals_oldest_one_shot() {
        let bk = bank(&[
            ("L", Pcm::mono(RATE, tone(100, 0))),
            ("X", Pcm::mono(RATE, tone(100, 1))),
            ("Y", Pcm::mono(RATE, tone(100, 2))),
            ("Z", Pcm::mono(RATE, tone(100, 3))),
        ]);
        let mut m = mixer(&AudioSettings {
            voice_cap: 3,
            ..full_volume()
        });
        let mut l = req("L");
        l.looped = true;
        let lv = m.play(&l, &bk).unwrap();
        let x = m.play(&req("X"), &bk).unwrap();
        let y = m.play(&req("Y"), &bk).unwrap();
        let z = m.play(&req("Z"), &bk).unwrap();
        assert_eq!(z.slot(), x.slot());
        assert!(!m.is_active(x));
        assert!(m.is_active(lv) && m.is_active(y) && m.is_active(z));
        assert_eq!(m.active_voices(), 3);

        // With only loops playing nothing is taken over.
        let mut m = mixer(&AudioSettings {
            voice_cap: 2,
            ..full_volume()
        });
        let mut lx = req("X");
        lx.looped = true;
        m.play(&l, &bk).unwrap();
        m.play(&lx, &bk).unwrap();
        assert!(m.play(&req("Y"), &bk).is_none());

        // Lowering the cap stops voices above it.
        let mut m = mixer(&full_volume());
        for k in ["X", "Y", "Z"] {
            m.play(&req(k), &bk).unwrap();
        }
        m.set_settings(&AudioSettings {
            voice_cap: 1,
            ..full_volume()
        });
        assert_eq!(m.active_voices(), 1);
    }

    #[test]
    fn handles_and_stop() {
        let bk = bank(&[("L", Pcm::mono(RATE, tone(10, 0)))]);
        let mut m = mixer(&full_volume());
        let mut l = req("L");
        l.looped = true;
        l.handle = Some(7);
        let v = m.play(&l, &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 100];
        m.render(&mut out);
        assert!(m.is_active(v), "a loop keeps playing");
        assert!(out.iter().any(|s| *s != 0.0));
        assert_eq!(m.stop(8), 0);
        assert_eq!(m.stop(7), 1);
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(!m.stop_voice(v));
    }

    #[test]
    fn looping_wraps_exactly() {
        let s: Vec<i16> = vec![100, 200, 300];
        let bk = bank(&[("L", Pcm::mono(RATE, s.clone()))]);
        let mut m = mixer(&full_volume());
        let mut l = req("L");
        l.looped = true;
        m.play(&l, &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 10];
        m.render(&mut out);
        for k in 0..10 {
            assert!(close(out[2 * k], f32::from(s[k % 3]) / 32768.0));
        }
    }

    #[test]
    fn channel_mute() {
        let bk = bank(&[
            ("V", Pcm::mono(RATE, tone(1000, 0))),
            ("AMB", Pcm::mono(RATE, tone(1000, 1))),
        ]);
        let mut m = mixer(&AudioSettings {
            voice: false,
            ..full_volume()
        });
        assert!(
            m.play(&PlayRequest::new("V", Channel::Voice), &bk)
                .is_none()
        );
        assert!(
            m.play(&PlayRequest::new("V", Channel::Effect), &bk)
                .is_some()
        );

        let mut m = mixer(&full_volume());
        let mut amb = PlayRequest::new("AMB", Channel::Ambient);
        amb.looped = true;
        let a = m.play(&amb, &bk).unwrap();
        m.set_settings(&AudioSettings {
            ambient: false,
            ..full_volume()
        });
        assert!(!m.is_active(a));
        assert!(m.play(&amb, &bk).is_none());

        // Sound volume 0 is silence.
        let mut m = mixer(&AudioSettings {
            sound_volume: 0,
            ..full_volume()
        });
        m.play(&req("V"), &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 50];
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));

        // Audio off plays nothing.
        let mut m = mixer(&AudioSettings {
            enabled: false,
            ..full_volume()
        });
        assert!(m.play(&req("V"), &bk).is_none());
    }

    #[test]
    fn channels_play_at_their_configured_volumes() {
        let bk = bank(&[("K", Pcm::mono(RATE, vec![16384i16; 8]))]);
        let level = |channel: Channel, sound_volume: u8| {
            let mut m = mixer(&AudioSettings {
                sound_volume,
                ..full_volume()
            });
            m.play(&PlayRequest::new("K", channel), &bk).unwrap();
            let mut out = vec![0.0f32; 2];
            m.render(&mut out);
            out[0]
        };
        let at = |v: u8| 0.5 * curve::gain(curve::attenuation(v));
        // Sound volume 80: effects and interface at 80, speech at 100,
        // ambient loops at 65.
        assert!(close(level(Channel::Effect, 80), at(80)));
        assert!(close(level(Channel::Interface, 80), at(80)));
        assert!(close(level(Channel::Voice, 80), at(100)));
        assert!(close(level(Channel::Ambient, 80), at(65)));
        // A cue at 90 percent of the speech level of sound volume 50 (70).
        let mut m = mixer(&AudioSettings {
            sound_volume: 50,
            ..full_volume()
        });
        let mut r = PlayRequest::new("K", Channel::Voice);
        r.volume = 90;
        m.play(&r, &bk).unwrap();
        let mut out = vec![0.0f32; 2];
        m.render(&mut out);
        assert!(close(out[0], at(63)));
        // Ambient loops are silent from sound volume 6 to 15.
        assert_eq!(level(Channel::Ambient, 10), 0.0);
        assert!(close(level(Channel::Ambient, 5), at(5)));
    }

    #[test]
    fn a_play_for_a_live_handle_updates_it() {
        let s: Vec<i16> = vec![16384; 1000];
        let bk = bank(&[
            ("AMB", Pcm::mono(RATE, s.clone())),
            ("OTHER", Pcm::mono(RATE, s.clone())),
        ]);
        let mut m = mixer(&full_volume());
        let mut r = PlayRequest::new("AMB", Channel::Effect);
        r.looped = true;
        r.handle = Some(4);
        r.volume = 100;
        let v = m.play(&r, &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 10];
        m.render(&mut out);
        r.volume = 50;
        r.pan = 100;
        assert_eq!(m.play(&r, &bk), Some(v), "same voice");
        assert_eq!(m.active_voices(), 1);
        m.render(&mut out);
        assert_eq!(out[0], 0.0);
        assert!(close(out[1], 0.5 * curve::gain(-1000)));
        // The same handle on another sound starts another voice; a stop of
        // the handle ends both.
        let mut o = PlayRequest::new("OTHER", Channel::Effect);
        o.handle = Some(4);
        let w = m.play(&o, &bk).unwrap();
        assert!(m.is_active(v) && m.is_active(w));
        assert_eq!(m.stop(4), 2);
    }

    #[test]
    fn volume_change_follows_playing_voices() {
        let bk = bank(&[("L", Pcm::mono(RATE, vec![16384i16; 4]))]);
        let mut m = mixer(&full_volume());
        let mut l = req("L");
        l.looped = true;
        m.play(&l, &bk).unwrap();
        m.set_settings(&AudioSettings {
            sound_volume: 25,
            ..full_volume()
        });
        let mut out = vec![0.0f32; 2];
        m.render(&mut out);
        assert!(close(out[0], 0.5 * 0.1));
    }

    #[test]
    fn pause_and_resume() {
        let s: Vec<i16> = (0..100).map(|i| i * 10).collect();
        let bk = bank(&[
            ("G", Pcm::mono(RATE, s.clone())),
            ("UI", Pcm::mono(RATE, s.clone())),
            ("SHOT", Pcm::mono(RATE, s.clone())),
            ("AMB", Pcm::mono(RATE, s.clone())),
        ]);
        let mut m = mixer(&full_volume());
        let mut loop_req = req("G");
        loop_req.looped = true;
        let g = m.play(&loop_req, &bk).unwrap();
        let mut amb_req = PlayRequest::new("AMB", Channel::Ambient);
        amb_req.looped = true;
        amb_req.handle = Some(4);
        let amb = m.play(&amb_req, &bk).unwrap();
        let shot = m.play(&req("SHOT"), &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 10];
        m.render(&mut out);
        m.pause_game();
        // Every game sound stops at the pause, loops included, and does not
        // come back.
        assert!(!m.is_active(shot) && !m.is_active(g) && !m.is_active(amb));
        let ui = m
            .play(&PlayRequest::new("UI", Channel::Interface), &bk)
            .unwrap();
        m.render(&mut out);
        // Only the interface voice sounds, from its start.
        for k in 0..10 {
            assert!(close(out[2 * k], f32::from(s[k]) / 32768.0));
        }
        assert!(m.is_active(ui));
        m.stop_voice(ui);
        m.resume_game();
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0), "nothing comes back");
        // The caller starts the ambient loop again, from its start.
        let again = m.play(&amb_req, &bk).unwrap();
        m.render(&mut out);
        assert!(m.is_active(again) && m.active_voices() == 1);
        assert_eq!(out[0], 0.0, "from its first sample");
        assert!(out[2] > 0.0);
        m.stop(4);

        // Pausing everything holds every voice where it is.
        m.play(&loop_req, &bk).unwrap();
        m.render(&mut out);
        m.pause_all();
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        m.resume_all();
        m.render(&mut out);
        assert!(close(out[0], f32::from(s[10]) / 32768.0));
    }

    struct Constant(f32);

    impl MusicSource for Constant {
        fn render(&mut self, out: &mut [f32], _rate: u32) {
            out.fill(self.0);
        }
    }

    #[test]
    fn music_at_music_volume() {
        let bk = SoundBank::new();
        let _ = bk;
        let mut m = mixer(&full_volume());
        assert!(m.set_music(Some(Box::new(Constant(0.5)))).is_none());
        // Longer than the scratch buffer.
        let mut out = vec![0.0f32; 2 * 3000];
        m.render(&mut out);
        let e = 0.5 * (63.0f32 / 127.0).powi(2);
        assert!(out.iter().all(|s| close(*s, e)));
        m.pause_game();
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0), "music holds while paused");
        m.resume_game();
        m.render(&mut out);
        assert!(out.iter().all(|s| close(*s, e)));
        m.set_settings(&AudioSettings {
            music: false,
            ..full_volume()
        });
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0), "music switch off");
        m.set_settings(&AudioSettings {
            music_volume: 0,
            ..full_volume()
        });
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(m.set_music(None).is_some());
    }

    #[test]
    fn output_is_clamped() {
        let bk = bank(&[("A", Pcm::mono(RATE, vec![32767i16; 10]))]);
        let mut m = mixer(&full_volume());
        for _ in 0..3 {
            m.play(&req("A"), &bk).unwrap();
        }
        let mut out = vec![0.0f32; 2 * 4];
        m.render(&mut out);
        assert!(out.iter().all(|s| *s == 1.0));
    }

    #[test]
    fn deterministic() {
        let bk = bank(&[
            ("A", Pcm::mono(22050, tone(5000, 1))),
            ("B", Pcm::mono(11025, tone(3000, 2))),
        ]);
        let run = || {
            let mut m = Mixer::new(48000);
            m.set_settings(&full_volume());
            let mut out = vec![0.0f32; 2 * 4096];
            let mut all = Vec::new();
            for k in 0..8 {
                let mut r = req(if k % 2 == 0 { "A" } else { "B" });
                r.pitch = (k * 13 - 40) as i16;
                r.pan = (k * 25 - 100) as i8;
                r.volume = 30 + k as u8 * 10;
                m.play(&r, &bk);
                m.render(&mut out[..2 * 512]);
                all.extend_from_slice(&out[..2 * 512]);
            }
            all
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn rate_change_keeps_clock_and_pitch() {
        let bk = bank(&[("A", Pcm::mono(RATE, tone(100_000, 0)))]);
        let mut m = mixer(&full_volume());
        let v = m.play(&req("A"), &bk).unwrap();
        let mut out = vec![0.0f32; 2 * 800];
        m.render(&mut out);
        assert_eq!(m.now_ms(), 100);
        m.set_out_rate(16000);
        m.render(&mut out);
        assert_eq!(m.now_ms(), 150);
        assert!(m.is_active(v));
    }

    #[test]
    fn rejects_unplayable_and_unknown() {
        let bk = bank(&[(
            "BAD",
            Pcm {
                rate: RATE,
                channels: 4,
                samples: vec![0i16; 8].into(),
            },
        )]);
        let mut m = mixer(&full_volume());
        assert!(m.play(&req("BAD"), &bk).is_none());
        assert!(m.play(&req("MISSING"), &bk).is_none());
    }
}
