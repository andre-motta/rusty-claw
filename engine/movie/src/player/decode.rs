//! The demuxer and decoders, and the decoding thread that feeds the
//! player's picture queue and the soundtrack's ring.

use std::any::Any;
use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use rtrb::Producer;
use rusty_ffmpeg::ffi;

use super::audio::Resampler;
use super::convert::Converter;
use super::io::CustomIo;
use crate::audio::AudioLink;
use crate::{MovieError, ReadSeek, VideoFrame};

/// Pictures decoded ahead of the one showing.
pub(super) const QUEUE_FRAMES: usize = 4;
/// While an audio engine plays the soundtrack and its ring holds less than
/// this, the decoder does not wait for room in a full picture queue: in a
/// file whose pictures come well before their sound (as in many VOB files),
/// the sound it needs lies further on, behind more pictures. It is below
/// what a well-interleaved file keeps in the ring alongside a full queue of
/// [`QUEUE_FRAMES`] pictures, so such files keep that queue.
const SOUND_LOW_WATER: Duration = Duration::from_millis(60);
/// How far the picture queue may then reach, from its oldest picture to
/// its newest.
const QUEUE_MAX_SPAN: Duration = Duration::from_secs(2);
/// How many bytes of pictures the queue may then hold (about 25 pictures of
/// 2560x1920).
const QUEUE_MAX_BYTES: usize = 192 << 20;
/// How long a waiting decoder sleeps before it looks at the stop flag
/// again.
const POLL: Duration = Duration::from_millis(5);
/// How far into a movie a time stamp may reach. A stamp beyond it (or one
/// that is not a number) is damaged and counts as missing, so every time
/// the player keeps stays far from the limits of [`Duration`].
const MAX_TIME: Duration = Duration::from_secs(24 * 60 * 60);
/// How many errors in a row a decoder may give for the frames it holds
/// before the rest of them is given up. Each damaged frame gives one, and
/// the frames behind it still come.
const MAX_RECEIVE_ERRORS: u32 = 64;

/// What the decoding thread and the player share.
#[derive(Debug, Default)]
pub(super) struct Shared {
    pub stop: AtomicBool,
    pub queue: Mutex<Queue>,
    pub cond: Condvar,
}

impl Shared {
    pub(super) fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }
}

/// The pictures decoded and not yet taken, and how decoding ended.
#[derive(Debug, Default)]
pub(super) struct Queue {
    pub frames: VecDeque<VideoFrame>,
    /// Decoding is over: every picture is in the queue.
    pub done: bool,
    /// When the last picture stops showing.
    pub end: Duration,
    /// Why decoding stopped early.
    pub error: Option<String>,
}

/// The player asked the thread to stop.
struct Stop;

pub(super) fn err(code: c_int) -> String {
    ffi::av_err2str(code)
}

/// A demuxer reading through a [`CustomIo`].
pub(super) struct Input {
    pub fmt: *mut ffi::AVFormatContext,
    _io: CustomIo,
}

impl Input {
    pub(super) fn open(
        reader: Box<dyn ReadSeek + Send>,
        name_hint: &str,
    ) -> Result<Input, MovieError> {
        let io = CustomIo::new(reader)?;
        // SAFETY: a fresh context, freed by avformat_open_input on failure
        // or by Drop below.
        let mut fmt = unsafe { ffi::avformat_alloc_context() };
        if fmt.is_null() {
            return Err(MovieError::Open("out of memory".into()));
        }
        let name: String = name_hint.chars().filter(|c| *c != '\0').collect();
        let name = CString::new(name).unwrap_or_default();
        // SAFETY: the context reads through our I/O context, which outlives
        // it (it is a field of the Input, freed after the context).
        let ret = unsafe {
            (*fmt).pb = io.ctx;
            (*fmt).flags |= ffi::AVFMT_FLAG_CUSTOM_IO as c_int;
            ffi::avformat_open_input(&mut fmt, name.as_ptr(), ptr::null(), ptr::null_mut())
        };
        if ret < 0 {
            // avformat_open_input freed the context; the I/O context is
            // freed with `io`.
            return Err(MovieError::Open(err(ret)));
        }
        let input = Input { fmt, _io: io };
        // SAFETY: an opened context.
        let ret = unsafe { ffi::avformat_find_stream_info(input.fmt, ptr::null_mut()) };
        if ret < 0 {
            return Err(MovieError::Open(err(ret)));
        }
        Ok(input)
    }

    /// The best stream of `kind`, related to `related` when given.
    pub(super) fn best(&self, kind: ffi::AVMediaType, related: c_int) -> Option<c_int> {
        // SAFETY: an opened context.
        let i =
            unsafe { ffi::av_find_best_stream(self.fmt, kind, -1, related, ptr::null_mut(), 0) };
        (i >= 0).then_some(i)
    }

    /// Stream `index`.
    pub(super) fn stream(&self, index: c_int) -> *mut ffi::AVStream {
        // SAFETY: `index` came from av_find_best_stream, so it is in range.
        unsafe { *(*self.fmt).streams.add(index as usize) }
    }

    /// The container's start time in seconds (0 when it names none).
    pub(super) fn start_seconds(&self) -> f64 {
        // SAFETY: an opened context.
        let start = unsafe { (*self.fmt).start_time };
        if start == ffi::AV_NOPTS_VALUE {
            0.0
        } else {
            start as f64 / f64::from(ffi::AV_TIME_BASE)
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        // SAFETY: the context is ours; with custom I/O it leaves the I/O
        // context alone, which the `_io` field frees next.
        unsafe { ffi::avformat_close_input(&mut self.fmt) };
    }
}

/// A decoder for one stream.
pub(super) struct Decoder {
    ctx: *mut ffi::AVCodecContext,
    pub stream: c_int,
    /// The stream's time base in seconds per tick.
    pub tick: f64,
}

impl Decoder {
    pub(super) fn open(input: &Input, stream: c_int) -> Result<Decoder, MovieError> {
        let st = input.stream(stream);
        // SAFETY: the stream and its parameters belong to the open input.
        unsafe {
            let par = (*st).codecpar;
            let codec = ffi::avcodec_find_decoder((*par).codec_id);
            if codec.is_null() {
                let name = CStr::from_ptr(ffi::avcodec_get_name((*par).codec_id));
                return Err(MovieError::Unsupported(format!(
                    "no decoder for {} in this build",
                    name.to_string_lossy()
                )));
            }
            let mut ctx = ffi::avcodec_alloc_context3(codec);
            if ctx.is_null() {
                return Err(MovieError::Open("out of memory".into()));
            }
            let ret = ffi::avcodec_parameters_to_context(ctx, par);
            if ret < 0 {
                ffi::avcodec_free_context(&mut ctx);
                return Err(MovieError::Open(err(ret)));
            }
            (*ctx).pkt_timebase = (*st).time_base;
            // Let FFmpeg pick the number of decoding threads.
            (*ctx).thread_count = 0;
            let ret = ffi::avcodec_open2(ctx, codec, ptr::null_mut());
            if ret < 0 {
                ffi::avcodec_free_context(&mut ctx);
                return Err(MovieError::Open(err(ret)));
            }
            let tb = (*st).time_base;
            let tick = if tb.den != 0 {
                f64::from(tb.num) / f64::from(tb.den)
            } else {
                0.0
            };
            Ok(Decoder { ctx, stream, tick })
        }
    }

    /// Sends `pkt` (NULL to drain) and hands every picture or sound frame
    /// that comes out to `each`. Damaged packets are skipped.
    ///
    /// # Safety
    /// `pkt` is a valid packet of this decoder's stream, or NULL; `frame`
    /// is an allocated frame.
    unsafe fn feed(
        &mut self,
        pkt: *const ffi::AVPacket,
        frame: *mut ffi::AVFrame,
        mut each: impl FnMut(*mut ffi::AVFrame) -> Result<(), Stop>,
    ) -> Result<(), Stop> {
        let again = ffi::AVERROR(ffi::EAGAIN);
        let ctx = self.ctx;
        for _ in 0..2 {
            // SAFETY: as documented above.
            let sent = unsafe { ffi::avcodec_send_packet(ctx, pkt) };
            receive_all(
                // SAFETY: as documented above.
                || unsafe { ffi::avcodec_receive_frame(ctx, frame) },
                || {
                    let r = each(frame);
                    // SAFETY: the frame was filled by the decoder.
                    unsafe { ffi::av_frame_unref(frame) };
                    r
                },
            )?;
            if sent != again {
                break;
            }
        }
        Ok(())
    }
}

/// Hands every frame a decoder has ready to `each`: `receive` asks the
/// decoder for the next one and returns what `avcodec_receive_frame` does.
/// A damaged frame's error is skipped, so the frames behind it still come
/// (a decoder working on several frames at once reports one bad frame and
/// then gives the others, also while it is drained at the end), up to
/// [`MAX_RECEIVE_ERRORS`] errors in a row. Only "send more input" (EAGAIN)
/// and "nothing more" (EOF, once drained) end it otherwise.
fn receive_all(
    mut receive: impl FnMut() -> c_int,
    mut each: impl FnMut() -> Result<(), Stop>,
) -> Result<(), Stop> {
    let again = ffi::AVERROR(ffi::EAGAIN);
    let mut errors = 0;
    loop {
        let got = receive();
        if got >= 0 {
            errors = 0;
            each()?;
        } else if got == again || got == ffi::AVERROR_EOF {
            return Ok(());
        } else {
            errors += 1;
            if errors >= MAX_RECEIVE_ERRORS {
                return Ok(());
            }
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the context is ours.
        unsafe { ffi::avcodec_free_context(&mut self.ctx) };
    }
}

/// An owned packet.
struct Packet(*mut ffi::AVPacket);

impl Drop for Packet {
    fn drop(&mut self) {
        // SAFETY: allocated with av_packet_alloc.
        unsafe { ffi::av_packet_free(&mut self.0) };
    }
}

/// An owned frame.
struct Frame(*mut ffi::AVFrame);

impl Drop for Frame {
    fn drop(&mut self) {
        // SAFETY: allocated with av_frame_alloc.
        unsafe { ffi::av_frame_free(&mut self.0) };
    }
}

/// The soundtrack's side of the decoding thread.
pub(super) struct Sound {
    pub decoder: Decoder,
    pub resampler: Resampler,
    pub ring: Producer<f32>,
    pub link: Arc<AudioLink>,
    pub started: bool,
}

impl Sound {
    /// An audio engine plays the soundtrack on its clock and the ring holds
    /// less than [`SOUND_LOW_WATER`] of it.
    fn starving(&self) -> bool {
        if !self.link.worth_waiting() {
            return false;
        }
        let held = self.ring.buffer().capacity() - self.ring.slots();
        let low = SOUND_LOW_WATER.as_secs_f64() * f64::from(self.link.rate) * 2.0;
        (held as f64) < low
    }
}

/// Everything the decoding thread owns.
pub(super) struct Job {
    pub input: Input,
    pub video: Decoder,
    pub sound: Option<Sound>,
    pub converter: Converter,
    /// The container's start, in seconds.
    pub start: f64,
    /// One picture's duration.
    pub frame_time: Duration,
    pub last_pts: Option<Duration>,
}

// SAFETY: the FFmpeg objects are created on the opening thread and then
// used by the decoding thread alone; none of them is shared.
unsafe impl Send for Job {}

impl Job {
    /// Decodes until the end or a stop, then marks the queue done. It does
    /// so on every path: a panic while decoding ends the movie with an
    /// error rather than leaving it on its last picture.
    pub(super) fn run(mut self, shared: &Shared) {
        let link = self.sound.as_ref().map(|s| Arc::clone(&s.link));
        let error = settle(|| self.decode(shared));
        let end = self
            .last_pts
            .map_or(Duration::ZERO, |p| p.saturating_add(self.frame_time));
        finish(shared, link.as_deref(), end, error);
    }

    fn decode(&mut self, shared: &Shared) -> Result<Result<(), String>, Stop> {
        // SAFETY: plain allocations, freed by the wrappers.
        let (pkt, frame) =
            unsafe { (Packet(ffi::av_packet_alloc()), Frame(ffi::av_frame_alloc())) };
        if pkt.0.is_null() || frame.0.is_null() {
            return Ok(Err("out of memory".into()));
        }
        let mut result = Ok(());
        loop {
            if shared.stopping() {
                return Err(Stop);
            }
            // SAFETY: an opened input and an allocated packet.
            let ret = unsafe { ffi::av_read_frame(self.input.fmt, pkt.0) };
            if ret < 0 {
                if ret != ffi::AVERROR_EOF {
                    result = Err(err(ret));
                }
                break;
            }
            // SAFETY: av_read_frame filled the packet.
            let index = unsafe { (*pkt.0).stream_index };
            let r = self.packet(shared, index, pkt.0, frame.0);
            // SAFETY: the packet was filled by av_read_frame.
            unsafe { ffi::av_packet_unref(pkt.0) };
            r?;
        }
        // Drain the decoders and the resampler.
        self.packet(shared, self.video.stream, ptr::null(), frame.0)?;
        if let Some(s) = &self.sound {
            let stream = s.decoder.stream;
            self.packet(shared, stream, ptr::null(), frame.0)?;
        }
        if let Some(s) = self.sound.as_mut() {
            match s.resampler.flush() {
                Ok(samples) => push_sound(shared, s, &samples)?,
                Err(e) => result = result.and(Err(e.to_string())),
            }
        }
        Ok(result)
    }

    /// Decodes one packet of stream `index` (NULL drains that stream).
    fn packet(
        &mut self,
        shared: &Shared,
        index: c_int,
        pkt: *const ffi::AVPacket,
        frame: *mut ffi::AVFrame,
    ) -> Result<(), Stop> {
        if index == self.video.stream {
            let Job {
                video,
                sound,
                converter,
                start,
                frame_time,
                last_pts,
                ..
            } = self;
            let tick = video.tick;
            // SAFETY: a packet of the video stream, or NULL.
            unsafe {
                video.feed(pkt, frame, |f| {
                    // SAFETY: a decoded picture.
                    let ts = frame_stamp(f);
                    let pts = picture_time(ts, tick, *start, *frame_time, *last_pts);
                    // SAFETY: a decoded picture.
                    match converter.convert(f, pts) {
                        Ok(picture) => {
                            *last_pts = Some(pts);
                            push_picture(shared, picture, sound.as_ref())
                        }
                        // A picture that cannot be converted is skipped.
                        Err(_) => Ok(()),
                    }
                })
            }
        } else if let Some(s) = self.sound.as_mut()
            && index == s.decoder.stream
        {
            let start = self.start;
            let Sound {
                decoder,
                resampler,
                ring,
                link,
                started,
            } = s;
            let tick = decoder.tick;
            // SAFETY: a packet of the sound stream, or NULL.
            unsafe {
                decoder.feed(pkt, frame, |f| {
                    if !*started {
                        *started = true;
                        if let Some(t) = stamp_time((*f).best_effort_timestamp, tick, start) {
                            link.set_start(t);
                        }
                    }
                    match resampler.convert(f) {
                        Ok(samples) => push_samples(shared, ring, link, &samples),
                        // Sound that cannot be converted is skipped.
                        Err(_) => Ok(()),
                    }
                })
            }
        } else {
            Ok(())
        }
    }
}

/// Turns what decoding gave into how it ended: `None` when it reached the
/// end or was stopped, else why it stopped early, a panic included.
fn settle(decode: impl FnOnce() -> Result<Result<(), String>, Stop>) -> Option<String> {
    match catch_unwind(AssertUnwindSafe(decode)) {
        Ok(Ok(Ok(()))) | Ok(Err(Stop)) => None,
        Ok(Ok(Err(e))) => Some(e),
        Err(panic) => Some(format!("the decoder failed: {}", panic_text(&*panic))),
    }
}

/// The message a panic carried.
fn panic_text(panic: &(dyn Any + Send)) -> &str {
    if let Some(s) = panic.downcast_ref::<&str>() {
        s
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s
    } else {
        "an unexpected error"
    }
}

/// Marks the queue done: decoding is over, the last picture stops showing
/// at `end`, and `error` says why it stopped early. The soundtrack, if any,
/// hears that no more samples come.
fn finish(shared: &Shared, link: Option<&AudioLink>, end: Duration, error: Option<String>) {
    if let Some(link) = link {
        link.set_ended();
    }
    let mut q = shared.lock();
    q.done = true;
    q.end = end;
    q.error = error;
    shared.cond.notify_all();
}

/// The time stamp of frame `f`: its best effort one, else its own.
///
/// # Safety
/// `f` is a valid frame.
unsafe fn frame_stamp(f: *const ffi::AVFrame) -> i64 {
    // SAFETY: a valid frame.
    let (best, pts) = unsafe { ((*f).best_effort_timestamp, (*f).pts) };
    if best != ffi::AV_NOPTS_VALUE {
        best
    } else {
        pts
    }
}

/// Time stamp `ts`, in ticks of `tick` seconds, as a time from the
/// container's `start` (in seconds); a time before the start is the start.
/// `None` when there is no stamp, or it is not a number or lies beyond
/// [`MAX_TIME`] (a damaged one).
fn stamp_time(ts: i64, tick: f64, start: f64) -> Option<Duration> {
    if ts == ffi::AV_NOPTS_VALUE {
        return None;
    }
    let secs = ts as f64 * tick - start;
    if secs.is_nan() {
        return None;
    }
    Duration::try_from_secs_f64(secs.max(0.0))
        .ok()
        .filter(|t| *t <= MAX_TIME)
}

/// When a picture stamped `ts` (see [`stamp_time`]) shows: at its stamp,
/// or one picture after the `last` one when it has none, a damaged one, or
/// one that does not move forward.
fn picture_time(
    ts: i64,
    tick: f64,
    start: f64,
    frame_time: Duration,
    last: Option<Duration>,
) -> Duration {
    match (stamp_time(ts, tick, start), last) {
        (Some(t), Some(l)) if t <= l => l.saturating_add(frame_time),
        (Some(t), _) => t,
        (None, Some(l)) => l.saturating_add(frame_time),
        (None, None) => Duration::ZERO,
    }
}

/// Queues `picture`, waiting while the queue is full. A full queue takes
/// more pictures, up to [`QUEUE_MAX_SPAN`] and [`QUEUE_MAX_BYTES`], while
/// the soundtrack is starving (see [`SOUND_LOW_WATER`]): waiting then would
/// hold back the sound the movie's clock runs on, and the clock would fall
/// to the wall clock for the rest of the movie.
fn push_picture(shared: &Shared, picture: VideoFrame, sound: Option<&Sound>) -> Result<(), Stop> {
    let mut q = shared.lock();
    loop {
        if shared.stopping() {
            return Err(Stop);
        }
        if q.frames.len() < QUEUE_FRAMES
            || (sound.is_some_and(Sound::starving) && room_beyond_full(&q.frames, &picture))
        {
            break;
        }
        q = shared
            .cond
            .wait_timeout(q, POLL)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
    q.frames.push_back(picture);
    shared.cond.notify_all();
    Ok(())
}

/// Whether `next` fits in a queue past its usual length: the queue then
/// spans at most [`QUEUE_MAX_SPAN`] and holds at most [`QUEUE_MAX_BYTES`].
fn room_beyond_full(frames: &VecDeque<VideoFrame>, next: &VideoFrame) -> bool {
    let span = frames
        .front()
        .map_or(Duration::ZERO, |f| next.pts.saturating_sub(f.pts));
    let bytes: usize = frames
        .iter()
        .chain([next])
        .flat_map(|f| f.planes.iter().map(Vec::len))
        .sum();
    span <= QUEUE_MAX_SPAN && bytes <= QUEUE_MAX_BYTES
}

fn push_sound(shared: &Shared, s: &mut Sound, samples: &[f32]) -> Result<(), Stop> {
    push_samples(shared, &mut s.ring, &s.link, samples)
}

/// Writes `samples` to the ring, waiting for room while an audio engine
/// plays the soundtrack on its clock; otherwise what does not fit is
/// dropped, so pictures never wait for sound nobody plays.
fn push_samples(
    shared: &Shared,
    ring: &mut Producer<f32>,
    link: &AudioLink,
    samples: &[f32],
) -> Result<(), Stop> {
    let mut left = samples;
    while !left.is_empty() {
        if shared.stopping() {
            return Err(Stop);
        }
        // Whole stereo frames only.
        let room = ring.slots() & !1;
        if room == 0 {
            if !link.worth_waiting() {
                return Ok(());
            }
            std::thread::sleep(POLL);
            continue;
        }
        let n = room.min(left.len());
        if let Ok(chunk) = ring.write_chunk_uninit(n) {
            chunk.fill_from_iter(left[..n].iter().copied());
        }
        left = &left[n..];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn time_stamps_become_times_from_the_start() {
        assert_eq!(
            stamp_time(90, 1.0 / 30.0, 0.0),
            Some(Duration::from_secs(3))
        );
        assert_eq!(stamp_time(5, 1.0, 2.0), Some(Duration::from_secs(3)));
        // Before the start is the start.
        assert_eq!(stamp_time(1, 1.0, 2.0), Some(Duration::ZERO));
        assert_eq!(stamp_time(i64::MIN + 1, 1.0, 0.0), Some(Duration::ZERO));
        assert_eq!(stamp_time(ffi::AV_NOPTS_VALUE, 1.0, 0.0), None);
    }

    #[test]
    fn time_stamps_out_of_range_count_as_missing() {
        // Far beyond what a Duration holds: 4 s ticks, 5e18 of them.
        assert_eq!(stamp_time(5_000_000_000_000_000_000, 4.0, 0.0), None);
        assert_eq!(stamp_time(i64::MAX, 1000.0, 0.0), None);
        // Representable, but beyond any movie.
        let day = MAX_TIME.as_secs() as i64;
        assert_eq!(stamp_time(day, 1.0, 0.0), Some(MAX_TIME));
        assert_eq!(stamp_time(day + 1, 1.0, 0.0), None);
        // Not a number.
        assert_eq!(stamp_time(1, f64::NAN, 0.0), None);
        assert_eq!(stamp_time(1, 1.0, f64::NAN), None);
        assert_eq!(stamp_time(i64::MAX, f64::INFINITY, 0.0), None);
    }

    #[test]
    fn a_picture_with_a_damaged_stamp_shows_one_picture_after_the_last() {
        let ft = Duration::from_millis(40);
        let huge = 5_000_000_000_000_000_000;
        assert_eq!(picture_time(huge, 4.0, 0.0, ft, None), Duration::ZERO);
        assert_eq!(picture_time(huge, 4.0, 0.0, ft, Some(SECOND)), SECOND + ft);
        // Stamps that do not move forward, and missing ones.
        assert_eq!(picture_time(1, 1.0, 0.0, ft, Some(SECOND)), SECOND + ft);
        assert_eq!(
            picture_time(ffi::AV_NOPTS_VALUE, 1.0, 0.0, ft, Some(SECOND)),
            SECOND + ft
        );
        assert_eq!(picture_time(2, 1.0, 0.0, ft, Some(SECOND)), 2 * SECOND);
        // No overflow, whatever the last time and the picture's length.
        assert_eq!(
            picture_time(huge, 4.0, 0.0, Duration::MAX, Some(SECOND)),
            Duration::MAX
        );
    }

    /// Runs [`receive_all`] over the return codes `codes` (frames for
    /// codes of 0 or more, then EAGAIN once they run out) and says how many
    /// frames came and how many codes were read.
    fn receive(codes: &[c_int]) -> (usize, usize) {
        let next = Cell::new(0);
        let mut frames = 0;
        let r = receive_all(
            || {
                let i = next.get();
                next.set(i + 1);
                codes.get(i).copied().unwrap_or(ffi::AVERROR(ffi::EAGAIN))
            },
            || {
                frames += 1;
                Ok(())
            },
        );
        assert!(r.is_ok());
        (frames, next.get())
    }

    #[test]
    fn a_damaged_frame_does_not_lose_the_frames_behind_it() {
        let bad = ffi::AVERROR_INVALIDDATA;
        let eof = ffi::AVERROR_EOF;
        let again = ffi::AVERROR(ffi::EAGAIN);
        // While draining: everything up to the end comes.
        assert_eq!(receive(&[0, bad, 0, 0, bad, bad, 0, eof, 0]), (4, 8));
        // While fed: up to the call for more input.
        assert_eq!(receive(&[0, bad, 0, again, 0]), (2, 4));
        assert_eq!(receive(&[again, 0]), (0, 1));
        assert_eq!(receive(&[eof, 0]), (0, 1));
    }

    #[test]
    fn a_decoder_that_only_fails_is_given_up() {
        let mut codes = vec![0];
        codes.extend([ffi::AVERROR_INVALIDDATA; 200]);
        codes.push(0);
        let (frames, read) = receive(&codes);
        assert_eq!((frames, read), (1, 1 + MAX_RECEIVE_ERRORS as usize));
        // Errors with frames between them are not in a row.
        let mut codes = Vec::new();
        for _ in 0..100 {
            codes.extend([ffi::AVERROR_INVALIDDATA, 0]);
        }
        codes.push(ffi::AVERROR_EOF);
        assert_eq!(receive(&codes).0, 100);
    }

    #[test]
    fn a_stop_ends_receiving_at_once() {
        let mut calls = 0;
        let r = receive_all(
            || {
                calls += 1;
                0
            },
            || Err(Stop),
        );
        assert!(r.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_panic_while_decoding_still_ends_the_movie_with_an_error() {
        assert_eq!(settle(|| Ok(Ok(()))), None);
        assert_eq!(settle(|| Err(Stop)), None);
        assert_eq!(settle(|| Ok(Err("bad".into()))), Some("bad".into()));
        let error = settle(|| panic!("the stamp {} is too large", 7)).unwrap();
        assert!(error.contains("the stamp 7 is too large"), "{error}");
        assert!(error.starts_with("the decoder failed"), "{error}");

        // The decoding thread's ending, as `Job::run` does it after a panic.
        let shared = Shared::default();
        let link = AudioLink::new(48000);
        finish(&shared, Some(&link), SECOND, settle(|| panic!("boom")));
        let q = shared.lock();
        assert!(q.done);
        assert_eq!(q.end, SECOND);
        assert!(q.error.as_deref().is_some_and(|e| e.contains("boom")));
    }

    #[test]
    fn a_panicking_decoding_thread_marks_the_queue_done() {
        let shared = Arc::new(Shared::default());
        let thread_shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            // The conversion the decoder made before: it panics on this
            // stamp.
            let error = settle(|| {
                let secs = 5_000_000_000_000_000_000_i64 as f64 * 4.0;
                let _ = Duration::from_secs_f64(secs);
                Ok(Ok(()))
            });
            finish(&thread_shared, None, Duration::ZERO, error);
        })
        .join()
        .expect("the thread ends normally");
        let q = shared.lock();
        assert!(q.done && q.error.is_some());
    }
}
