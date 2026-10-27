//! The movie player: FFmpeg demuxes and decodes on a thread of its own,
//! pictures wait in a short queue until the movie's clock reaches them, and
//! the soundtrack goes to the audio engine through a lock-free ring.

mod audio;
mod convert;
mod decode;
mod io;

use std::ffi::{CStr, c_int};
use std::fs::File;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use claw_audio::MusicSource;
use rusty_ffmpeg::ffi;

use self::decode::{Decoder, Input, Job, Shared, Sound};
use crate::audio::{AudioLink, MovieAudio, ring};
use crate::clock::Clock;
use crate::{MovieError, ReadSeek, Source, VideoFrame, fec};

/// Seconds of sound the ring holds.
const RING_SECONDS: u32 = 2;

pub(crate) fn ffmpeg_version() -> Option<String> {
    // SAFETY: av_version_info returns a pointer to a static, NUL-terminated
    // string that lives as long as the program.
    let info = unsafe { CStr::from_ptr(ffi::av_version_info()) };
    Some(info.to_string_lossy().into_owned())
}

/// FFmpeg says nothing on the terminal unless `CLAW_MOVIE_LOG` is set: the
/// player reports what matters in one line of its own.
fn quiet_ffmpeg() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let level = if std::env::var_os("CLAW_MOVIE_LOG").is_some() {
            ffi::AV_LOG_WARNING as c_int
        } else {
            ffi::AV_LOG_QUIET
        };
        // SAFETY: sets a global level; no other state is touched.
        unsafe { ffi::av_log_set_level(level) };
    });
}

/// A playing movie (see the module documentation).
///
/// Every FFmpeg object is freed on every path, and dropping the player
/// stops and joins its decoding thread within a few milliseconds.
pub struct Player {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    size: (u32, u32),
    aspect: f32,
    audio: Option<MovieAudio>,
    link: Option<Arc<AudioLink>>,
    clock: Clock,
    /// The next picture, taken from the queue but not due yet.
    pending: Option<VideoFrame>,
    /// The picture last returned by [`Player::next_frame`].
    current: Option<VideoFrame>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("size", &self.size)
            .field("aspect", &self.aspect)
            .field("sound", &self.link.is_some())
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}

impl Player {
    /// Opens `source`, with its soundtrack at `audio_rate` Hz.
    pub fn open(source: &Source, audio_rate: u32) -> Result<Player, MovieError> {
        match source {
            Source::File { path, .. } => {
                let file = File::open(path).map_err(|e| MovieError::Open(e.to_string()))?;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Player::open_reader(Box::new(file), &name, audio_rate)
            }
            Source::Archive { path, entry } => {
                let reader = fec::open_entry(path, *entry).map_err(|e| match e.kind() {
                    std::io::ErrorKind::Unsupported => MovieError::Unsupported(e.to_string()),
                    _ => MovieError::Open(e.to_string()),
                })?;
                Player::open_reader(reader, &format!("CLAW{entry}.smk"), audio_rate)
            }
        }
    }

    /// Opens a movie read from `reader`; `name_hint` (a file name) helps
    /// FFmpeg recognise the format.
    pub fn open_reader(
        reader: Box<dyn ReadSeek + Send>,
        name_hint: &str,
        audio_rate: u32,
    ) -> Result<Player, MovieError> {
        quiet_ffmpeg();
        let input = Input::open(reader, name_hint)?;
        let Some(video_index) = input.best(ffi::AVMEDIA_TYPE_VIDEO, -1) else {
            return Err(MovieError::Unsupported("no picture stream".into()));
        };
        let st = input.stream(video_index);
        // SAFETY: the stream belongs to the open input.
        let (size, aspect, frame_time) = unsafe {
            let par = (*st).codecpar;
            let (w, h) = ((*par).width, (*par).height);
            if w <= 0 || h <= 0 {
                return Err(MovieError::Unsupported("a picture without a size".into()));
            }
            // Before any decoder or picture is made for it.
            convert::check_size(w, h)?;
            let sar = ffi::av_guess_sample_aspect_ratio(input.fmt, st, std::ptr::null_mut());
            let sar = if sar.num > 0 && sar.den > 0 {
                f64::from(sar.num) / f64::from(sar.den)
            } else {
                1.0
            };
            let aspect = (f64::from(w) * sar / f64::from(h)) as f32;
            let rate = [(*st).avg_frame_rate, (*st).r_frame_rate]
                .into_iter()
                .find(|r| r.num > 0 && r.den > 0)
                .map_or(30.0, |r| f64::from(r.num) / f64::from(r.den));
            (
                (w as u32, h as u32),
                aspect,
                // A rate outside 1 to 1000 pictures a second is not a movie's:
                // its picture time would be the step for missing stamps.
                if (1.0..=1000.0).contains(&rate) {
                    Duration::from_secs_f64(1.0 / rate)
                } else {
                    Duration::from_nanos(1_000_000_000 / 30)
                },
            )
        };
        let video = Decoder::open(&input, video_index)?;
        let mut sound = None;
        let mut audio = None;
        let mut link = None;
        if let Some(i) = input.best(ffi::AVMEDIA_TYPE_AUDIO, video_index) {
            match Decoder::open(&input, i) {
                Ok(decoder) => {
                    let l = Arc::new(AudioLink::new(audio_rate));
                    let (tx, rx) = ring(&l, RING_SECONDS);
                    sound = Some(Sound {
                        decoder,
                        resampler: audio::Resampler::new(audio_rate),
                        ring: tx,
                        link: Arc::clone(&l),
                        started: false,
                    });
                    audio = Some(rx);
                    link = Some(l);
                }
                Err(e) => eprintln!("claw-movie: {name_hint}: the movie plays without sound: {e}"),
            }
        }
        let job = Job {
            start: input.start_seconds(),
            input,
            video,
            sound,
            converter: convert::Converter::new(),
            frame_time,
            last_pts: None,
        };
        let shared = Arc::new(Shared::default());
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("claw-movie".into())
            .spawn(move || job.run(&thread_shared))
            .map_err(|e| MovieError::Open(format!("cannot start the decoding thread: {e}")))?;
        Ok(Player {
            shared,
            thread: Some(thread),
            size,
            aspect,
            audio,
            link,
            clock: Clock::wall(),
            pending: None,
            current: None,
        })
    }

    /// The picture size in pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The shape the picture shows at, width over height, from its size
    /// and its pixels' shape.
    pub fn display_aspect(&self) -> f32 {
        self.aspect
    }

    /// The soundtrack, once, for an audio engine to play (for example
    /// through `AudioEngine::set_movie`). The movie's clock follows it from
    /// then on; without it, the movie runs on the wall clock. `None` when
    /// the movie has no sound or it was taken already.
    pub fn take_audio(&mut self) -> Option<Box<dyn MusicSource>> {
        let audio = self.audio.take()?;
        audio.mark_live();
        if let Some(link) = &self.link {
            self.clock.follow(Arc::clone(link));
        }
        Some(Box::new(audio))
    }

    /// The movie's time (see [`Clock`]).
    pub fn clock(&self) -> Duration {
        self.clock.now()
    }

    /// The newest picture that is due and was not returned yet; pictures
    /// that fell behind the clock are dropped. `None` when no new picture
    /// is due; the last one keeps showing.
    pub fn next_frame(&mut self) -> Option<&VideoFrame> {
        let now = self.clock.now();
        let mut newest = None;
        loop {
            if self.pending.is_none() {
                let mut q = self.shared.lock();
                self.pending = q.frames.pop_front();
                if self.pending.is_some() {
                    self.shared.cond.notify_all();
                }
            }
            match &self.pending {
                Some(f) if f.pts <= now => newest = self.pending.take(),
                _ => break,
            }
        }
        let frame = newest?;
        self.current = Some(frame);
        self.current.as_ref()
    }

    /// The next decoded picture whatever the clock, waiting up to `wait`
    /// for it; `None` at the end of the movie or when none came in time.
    /// For tools and tests that read every picture.
    pub fn decoded_frame(&mut self, wait: Duration) -> Option<VideoFrame> {
        if let Some(f) = self.pending.take() {
            return Some(f);
        }
        let deadline = Instant::now() + wait;
        let mut q = self.shared.lock();
        loop {
            if let Some(f) = q.frames.pop_front() {
                self.shared.cond.notify_all();
                return Some(f);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if q.done || left.is_zero() {
                return None;
            }
            q = self
                .shared
                .cond
                .wait_timeout(q, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// How many pictures are decoded and not shown yet: those in the queue
    /// and the next one, taken but not due. For tests of the queue's
    /// length.
    #[doc(hidden)]
    pub fn decoded_ahead(&self) -> usize {
        self.shared.lock().frames.len() + usize::from(self.pending.is_some())
    }

    /// Where the soundtrack is, from the movie's start: the sound frames
    /// played so far. `None` for a movie without sound.
    pub fn audio_position(&self) -> Option<Duration> {
        self.link.as_ref().map(|l| l.position())
    }

    /// Whether the movie is over: every picture was shown for its time and
    /// the soundtrack, while an engine plays it, has played out.
    pub fn finished(&self) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let (done, end) = {
            let q = self.shared.lock();
            (q.done && q.frames.is_empty(), q.end)
        };
        if !done || self.clock.now() < end {
            return false;
        }
        self.link
            .as_ref()
            .is_none_or(|l| !l.live() || l.drained() || self.clock.on_wall())
    }

    /// Why decoding stopped before the end, if it did.
    pub fn error(&self) -> Option<String> {
        self.shared.lock().error.clone()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.cond.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
