//! Movies like the upscaled copies players make: clips are made here, in a
//! temporary folder, with the `ffmpeg` command-line tool of the system, and
//! played through the player.
//!
//! - AV1 8-bit in WebM and AV1 10-bit in MKV, with Opus sound (libsvtav1,
//!   else libaom-av1, else av1_nvenc; libopus, else FFmpeg's own opus
//!   encoder), tagged with left colour siting; the upscaled movies' format;
//! - in a build with the H.264 and HEVC decoders (RUSTY_CLAW_FFMPEG_H26X=on)
//!   only: H.264 in MP4 and in MKV with AAC sound (the libopenh264 encoder),
//!   and HEVC Main (8-bit) and Main10 (10-bit) in MKV with AAC sound
//!   (hevc_nvenc, else hevc_vaapi);
//!
//! each 2560x1920 at 29.97 pictures a second, tagged BT.601, limited range.
//! Encoders that work on the GPU (av1_nvenc, hevc_nvenc, hevc_vaapi) are
//! used only with `CLAW_CODECS_BENCH=1` and `CLAW_GPU_TESTS=1` both set,
//! and then under the bench lock; otherwise the tests leave the GPU alone.
//! The first pictures are a moving test pattern under fresh grain in every
//! picture, so the encoder spends its whole bit rate (16 Mb/s, as a
//! detailed upscale would), and the last ones a flat colour. Each test
//! checks the picture count, the size, the I420 layout and the flat colour
//! after conversion, one picture at a time as they come. With
//! `CLAW_CODECS_BENCH=1` it also measures how fast the player decodes and
//! converts the clip with every core and with four (through `taskset -c
//! 0-3`, as a child process) and asks for at least 1.5 times real time with
//! every core; run that in an optimised build (`--release`) on an otherwise
//! idle machine. Without it, as in `make check` and `make test-install`,
//! the tests check the pictures only. A clip whose encoder the system lacks
//! is skipped with the reason printed, and so is an AV1 clip in a build
//! without AV1, except with RUSTY_CLAW_FFMPEG=require (and
//! RUSTY_CLAW_FFMPEG_AV1 not off), where that fails. Run with `--nocapture`
//! to see the table.
#![cfg(claw_ffmpeg)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use claw_movie::{ChromaSiting, FileKind, FrameLayout, Matrix, Player, Source, VideoFrame};

const W: u32 = 2560;
const H: u32 = 1920;
/// 29.97 pictures a second.
const RATE: (u32, u32) = (30000, 1001);
const FRAMES: usize = 90;
/// Pictures from this one on are the flat colour.
const FLAT_FROM: usize = 60;
const FLAT: [u8; 3] = [48, 128, 192];
const OUT_RATE: u32 = 48000;
/// The speed every clip decodes at with every core, over real time.
const MIN_SPEED: f64 = 1.5;
/// Set in the child that measures a clip on four cores.
const MEASURE_ENV: &str = "CLAW_CODECS_MEASURE";
/// Set to 1 to measure speeds.
const BENCH_ENV: &str = "CLAW_CODECS_BENCH";
/// Set to 1, with [`BENCH_ENV`], to encode clips on the GPU too.
const GPU_ENV: &str = "CLAW_GPU_TESTS";

fn set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| v == "1")
}

/// Whether this run measures speeds: when [`BENCH_ENV`] is 1.
fn measuring() -> bool {
    set(BENCH_ENV)
}

/// Whether this run may encode on the GPU: when [`BENCH_ENV`] and
/// [`GPU_ENV`] are both 1.
fn gpu_allowed() -> bool {
    measuring() && set(GPU_ENV)
}

/// One test decodes at a time, so the speeds are measured on an idle
/// machine.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn fps() -> f64 {
    f64::from(RATE.0) / f64::from(RATE.1)
}

/// Why the clips cannot be made, if they cannot.
fn ffmpeg_missing() -> Option<String> {
    match Command::new("ffmpeg").arg("-version").output() {
        Ok(o) if o.status.success() => None,
        Ok(_) => Some("the ffmpeg tool does not run".into()),
        Err(e) => Some(format!("no ffmpeg tool on PATH ({e})")),
    }
}

fn has_encoder(name: &str) -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.split_whitespace().nth(1) == Some(name))
        })
        .unwrap_or(false)
}

/// The video format of a clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Codec {
    /// AV1 with Opus sound, which every build with dav1d plays.
    Av1,
    /// H.264 or HEVC with AAC sound, which only a build made with
    /// RUSTY_CLAW_FFMPEG_H26X=on plays.
    H26x,
}

impl Codec {
    /// Why this build cannot play the codec, if it cannot.
    fn missing(self) -> Option<&'static str> {
        match self {
            Codec::Av1 if !cfg!(claw_ffmpeg_av1) => {
                // A build that must have AV1 fails here instead of skipping.
                let var = |name| std::env::var(name).unwrap_or_default();
                assert!(
                    var("RUSTY_CLAW_FFMPEG").trim() != "require"
                        || var("RUSTY_CLAW_FFMPEG_AV1").trim() == "off",
                    "RUSTY_CLAW_FFMPEG=require, but this FFmpeg was built without AV1 (no dav1d)"
                );
                Some("this FFmpeg was built without AV1 (no dav1d)")
            }
            Codec::H26x if !cfg!(claw_ffmpeg_h26x) => Some(
                "this FFmpeg has no H.264 or HEVC decoder (they come with RUSTY_CLAW_FFMPEG_H26X=on)",
            ),
            _ => None,
        }
    }
}

/// How a clip is made.
#[derive(Debug, Clone, Copy)]
struct Clip {
    name: &'static str,
    codec: Codec,
    /// The file name, whose extension picks the container.
    file: &'static str,
    /// Encoders to try in turn.
    encoders: &'static [&'static str],
    /// The pixel format handed to the encoder.
    pix_fmt: &'static str,
    /// Extra encoder options.
    options: &'static [&'static str],
    size: (u32, u32),
    frames: usize,
    flat_from: usize,
    bt709: bool,
    full_range: bool,
}

const AV1_WEBM: Clip = Clip {
    name: "AV1 8-bit WebM + Opus",
    codec: Codec::Av1,
    file: "av1.webm",
    encoders: &["libsvtav1", "libaom-av1", "av1_nvenc"],
    pix_fmt: "yuv420p",
    options: &["-b:v", "16M"],
    size: (W, H),
    frames: FRAMES,
    flat_from: FLAT_FROM,
    bt709: false,
    full_range: false,
};

const AV1_10_MKV: Clip = Clip {
    name: "AV1 10-bit MKV + Opus",
    file: "av1-10.mkv",
    pix_fmt: "yuv420p10le",
    ..AV1_WEBM
};

const H264_MP4: Clip = Clip {
    name: "H.264 MP4 + AAC",
    codec: Codec::H26x,
    file: "h264.mp4",
    encoders: &["libopenh264"],
    pix_fmt: "yuv420p",
    options: &["-b:v", "16M", "-movflags", "+write_colr"],
    size: (W, H),
    frames: FRAMES,
    flat_from: FLAT_FROM,
    bt709: false,
    full_range: false,
};

const H264_MKV: Clip = Clip {
    name: "H.264 MKV + AAC",
    file: "h264.mkv",
    ..H264_MP4
};

const HEVC_MAIN: Clip = Clip {
    name: "H.265 Main MKV + AAC",
    file: "hevc.mkv",
    encoders: &["hevc_nvenc", "hevc_vaapi"],
    pix_fmt: "yuv420p",
    options: &["-b:v", "16M", "-profile:v", "main"],
    ..H264_MP4
};

const HEVC_MAIN10: Clip = Clip {
    name: "H.265 Main10 MKV + AAC",
    file: "hevc10.mkv",
    pix_fmt: "p010le",
    options: &["-b:v", "16M", "-profile:v", "main10"],
    ..HEVC_MAIN
};

/// Makes `clip` in `dir`; `Err` names why it could not be made.
fn make(clip: &Clip, dir: &Path) -> Result<(PathBuf, &'static str), String> {
    if let Some(why) = ffmpeg_missing() {
        return Err(why);
    }
    let path = dir.join(clip.file);
    let (w, h) = clip.size;
    let (matrix, tag) = if clip.bt709 {
        ("bt709", ["bt709", "bt709", "bt709"])
    } else {
        ("bt601", ["bt470bg", "bt470bg", "smpte170m"])
    };
    let range = if clip.full_range { "pc" } else { "tv" };
    let [r, g, b] = FLAT;
    let video = format!(
        "testsrc2=s={w}x{h}:r={}/{},format=yuv444p,noise=alls=12:allf=t,format=rgb24,\
         drawbox=enable='gte(n\\,{})':x=0:y=0:w=iw:h=ih:color=0x{r:02X}{g:02X}{b:02X}@1:t=fill,\
         scale=out_color_matrix={matrix}:out_range={range}",
        RATE.0, RATE.1, clip.flat_from
    );
    let mut tried = Vec::new();
    let audio: &[&str] = match clip.codec {
        Codec::Av1 if has_encoder("libopus") => &["-c:a", "libopus", "-b:a", "128k"],
        // FFmpeg's own Opus encoder is marked experimental.
        Codec::Av1 if has_encoder("opus") => &["-c:a", "opus", "-strict", "-2", "-b:a", "128k"],
        Codec::Av1 => return Err("no Opus encoder (libopus or opus) in this ffmpeg".into()),
        Codec::H26x => &["-c:a", "aac"],
    };
    for &encoder in clip.encoders {
        let gpu = encoder.ends_with("_nvenc") || encoder.ends_with("_vaapi");
        if gpu && !gpu_allowed() {
            tried.push(format!(
                "{encoder}: encodes on the GPU; set {BENCH_ENV}=1 and {GPU_ENV}=1 and run under the bench lock"
            ));
            continue;
        }
        if !has_encoder(encoder) {
            tried.push(format!("{encoder}: not in this ffmpeg"));
            continue;
        }
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
        let mut filter = video.clone();
        if encoder.ends_with("_vaapi") {
            cmd.args(["-vaapi_device", "/dev/dri/renderD128"]);
            let upload = if clip.pix_fmt == "p010le" {
                "p010"
            } else {
                "nv12"
            };
            filter.push_str(&format!(",format={upload},hwupload"));
        } else if encoder == "av1_nvenc" && clip.pix_fmt == "yuv420p10le" {
            filter.push_str(",format=p010le");
        } else {
            filter.push_str(&format!(",format={}", clip.pix_fmt));
        }
        cmd.args(["-f", "lavfi", "-i", &filter]);
        cmd.args(["-f", "lavfi", "-i", "sine=f=440:r=48000"]);
        cmd.args(["-frames:v", &clip.frames.to_string(), "-shortest"]);
        cmd.args(["-c:v", encoder]);
        if encoder == "hevc_vaapi" {
            // The options are nvenc's; vaapi takes the profile only.
            cmd.args([
                "-profile:v",
                if clip.pix_fmt == "p010le" {
                    "main10"
                } else {
                    "main"
                },
            ]);
        } else {
            cmd.args(clip.options);
        }
        match encoder {
            // Fast presets: the clips test decoding, not the encoder.
            "libsvtav1" => cmd.args(["-preset", "10"]),
            "libaom-av1" => cmd.args(["-cpu-used", "8", "-row-mt", "1", "-usage", "realtime"]),
            _ => &mut cmd,
        };
        if clip.codec == Codec::Av1 {
            // AV1's sequence header carries the colour siting; left (as
            // MPEG) is what upscaled copies made from the originals have.
            cmd.args(["-chroma_sample_location", "left"]);
        }
        if encoder == "libopenh264" {
            // The colour description inside the H.264 stream too.
            let (prim, matrix) = if clip.bt709 { (1, 1) } else { (6, 6) };
            let bsf = format!(
                "h264_metadata=colour_primaries={prim}:transfer_characteristics={prim}:\
                 matrix_coefficients={matrix}:video_full_range_flag={}",
                u8::from(clip.full_range)
            );
            cmd.args(["-bsf:v", &bsf]);
        }
        cmd.args([
            "-colorspace",
            tag[0],
            "-color_primaries",
            tag[1],
            "-color_trc",
            tag[2],
            "-color_range",
            range,
        ]);
        cmd.args(audio);
        cmd.args(["-ac", "2"]);
        cmd.arg(&path);
        match cmd.output() {
            Ok(o) if o.status.success() && path.is_file() => return Ok((path, encoder)),
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stderr);
                let last = text.lines().last().unwrap_or("failed").trim().to_string();
                tried.push(format!("{encoder}: {last}"));
            }
            Err(e) => tried.push(format!("{encoder}: {e}")),
        }
    }
    Err(format!("no encoder could make it ({})", tried.join("; ")))
}

/// How decoding a clip went.
#[derive(Debug)]
struct Decoded {
    /// The pictures that came.
    count: usize,
    /// From opening the file to the last picture.
    took: Duration,
    error: Option<String>,
}

/// Hands every picture of `path` to `each`, in order and as fast as the
/// player decodes them; each is dropped after it, so only the queue's few
/// pictures are held at once.
fn decode_each(path: &Path, mut each: impl FnMut(usize, VideoFrame)) -> Decoded {
    let start = Instant::now();
    let source = Source::File {
        path: path.to_path_buf(),
        kind: FileKind::Upscaled,
    };
    let mut player = Player::open(&source, OUT_RATE).expect("the clip opens");
    let mut count = 0;
    while let Some(f) = player.decoded_frame(Duration::from_secs(10)) {
        each(count, f);
        count += 1;
    }
    Decoded {
        count,
        took: start.elapsed(),
        error: player.error(),
    }
}

/// What [`decode_and_check`] kept of a clip's pictures.
struct Checked {
    decoded: Decoded,
    /// The picture asked for, if it came.
    kept: Option<VideoFrame>,
    /// When the last picture shows.
    last_pts: Option<Duration>,
}

/// Decodes `path`, checking each picture's layout and that the times go
/// forward, and keeps only picture `keep`.
fn decode_and_check(clip: &Clip, path: &Path, keep: usize) -> Checked {
    let mut kept = None;
    let mut last_pts: Option<Duration> = None;
    let decoded = decode_each(path, |i, f| {
        check_layout(clip, &f);
        if let Some(last) = last_pts {
            assert!(f.pts > last, "{}: times go forward", clip.name);
        }
        last_pts = Some(f.pts);
        if i == keep {
            kept = Some(f);
        }
    });
    Checked {
        decoded,
        kept,
        last_pts,
    }
}

/// The colour of a YUV sample, as the renderer would show it.
fn rgb_of(yuv: [u8; 3], matrix: Matrix, full_range: bool) -> [f64; 3] {
    let (kr, kb) = match matrix {
        Matrix::Bt601 => (0.299, 0.114),
        Matrix::Bt709 => (0.2126, 0.0722),
    };
    let kg = 1.0 - kr - kb;
    let [y, u, v] = yuv.map(f64::from);
    let (y, cb, cr) = if full_range {
        (y / 255.0, (u - 128.0) / 255.0, (v - 128.0) / 255.0)
    } else {
        ((y - 16.0) / 219.0, (u - 128.0) / 224.0, (v - 128.0) / 224.0)
    };
    let r = y + 2.0 * (1.0 - kr) * cr;
    let b = y + 2.0 * (1.0 - kb) * cb;
    let g = (y - kr * r - kb * b) / kg;
    [r, g, b].map(|c| (c * 255.0).clamp(0.0, 255.0))
}

/// Checks one picture's layout and plane sizes.
fn check_layout(clip: &Clip, f: &VideoFrame) {
    let (w, h) = clip.size;
    assert_eq!((f.width, f.height), (w, h));
    let matrix = if clip.bt709 {
        Matrix::Bt709
    } else {
        Matrix::Bt601
    };
    assert_eq!(
        f.layout,
        FrameLayout::I420 {
            matrix,
            full_range: clip.full_range
        },
        "{}",
        clip.name
    );
    if clip.codec == Codec::Av1 {
        assert_eq!(f.chroma, ChromaSiting::Left, "{}", clip.name);
    }
    let (cw, ch) = (w.div_ceil(2) as usize, h.div_ceil(2) as usize);
    assert!(f.strides[0] >= w as usize && f.strides[1] >= cw && f.strides[2] >= cw);
    assert!(f.planes[0].len() >= f.strides[0] * (h as usize - 1) + w as usize);
    for i in 1..3 {
        assert!(f.planes[i].len() >= f.strides[i] * (ch - 1) + cw);
    }
}

/// The flat picture's colour, from the middle half of the picture, which
/// must be even.
fn flat_colour(clip: &Clip, f: &VideoFrame) -> [f64; 3] {
    let FrameLayout::I420 { matrix, full_range } = f.layout else {
        panic!("not I420");
    };
    let (w, h) = (f.width as usize, f.height as usize);
    let mut sum = [0.0; 3];
    let mut n = 0.0;
    let (mut lo, mut hi) = ([255u8; 3], [0u8; 3]);
    for y in (h / 4..3 * h / 4).step_by(7) {
        for x in (w / 4..3 * w / 4).step_by(7) {
            let yuv = [
                f.planes[0][y * f.strides[0] + x],
                f.planes[1][(y / 2) * f.strides[1] + x / 2],
                f.planes[2][(y / 2) * f.strides[2] + x / 2],
            ];
            for i in 0..3 {
                lo[i] = lo[i].min(yuv[i]);
                hi[i] = hi[i].max(yuv[i]);
            }
            let rgb = rgb_of(yuv, matrix, full_range);
            for i in 0..3 {
                sum[i] += rgb[i];
            }
            n += 1.0;
        }
    }
    for i in 0..3 {
        assert!(
            hi[i] - lo[i] <= 4,
            "{}: the flat picture varies in plane {i} from {} to {}",
            clip.name,
            lo[i],
            hi[i]
        );
    }
    sum.map(|s| s / n)
}

/// The speed of decoding `path` on cores 0-3, measured by a child process
/// of this test binary under `taskset`; `Err` says why it was not.
fn speed_on_four_cores(path: &Path) -> Result<f64, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = Command::new("taskset")
        .args(["-c", "0-3"])
        .arg(exe)
        .args(["--ignored", "--exact", "measure_in_child", "--nocapture"])
        .env(MEASURE_ENV, path)
        .output()
        .map_err(|e| format!("taskset does not run ({e})"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("codecs-fps "))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .ok_or_else(|| {
            format!(
                "the child failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )
        })
}

/// Makes, plays and measures `clip`; prints one table row.
fn check(clip: Clip) {
    if let Some(why) = clip.codec.missing() {
        println!("skipped {}: {why}", clip.name);
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let (path, encoder) = match make(&clip, dir.path()) {
        Ok(made) => made,
        Err(why) => {
            println!("skipped {}: {why}", clip.name);
            return;
        }
    };
    let checked = decode_and_check(&clip, &path, (clip.flat_from + clip.frames) / 2);
    let Decoded { count, took, error } = checked.decoded;
    assert_eq!(error, None, "{}", clip.name);
    assert_eq!(count, clip.frames, "{}: pictures", clip.name);
    let last = checked.last_pts.unwrap();
    let want = Duration::from_secs_f64((clip.frames - 1) as f64 / fps());
    assert!(
        last.abs_diff(want) < Duration::from_millis(5),
        "{}: the last picture shows at {last:?}",
        clip.name,
    );
    let got = flat_colour(&clip, &checked.kept.unwrap());
    for (g, w) in got.iter().zip(FLAT) {
        assert!(
            (g - f64::from(w)).abs() <= 4.0,
            "{}: the flat colour shows as {got:?}, not {FLAT:?}",
            clip.name
        );
    }
    let all = clip.frames as f64 / took.as_secs_f64();
    if !measuring() {
        println!(
            "| {:<24} | {:<11} | {} | speeds measured with {BENCH_ENV}=1 |",
            clip.name, encoder, count
        );
        return;
    }
    let four = speed_on_four_cores(&path);
    let four_text = match &four {
        Ok(v) => format!("{v:7.1} fps ({:4.2}x)", v / fps()),
        Err(why) => format!("not measured: {why}"),
    };
    println!(
        "| {:<24} | {:<11} | {} | {:7.1} fps ({:4.2}x) | {} |",
        clip.name,
        encoder,
        count,
        all,
        all / fps(),
        four_text
    );
    assert!(
        all >= MIN_SPEED * fps(),
        "{}: decodes at {all:.1} pictures a second, under {MIN_SPEED} times real time",
        clip.name
    );
}

#[test]
fn av1_8_bit_in_webm_with_opus_plays_as_i420() {
    check(AV1_WEBM);
}

#[test]
fn av1_10_bit_in_mkv_with_opus_plays_as_8_bit_i420() {
    check(AV1_10_MKV);
}

#[test]
fn h264_in_mp4_plays_as_i420() {
    check(H264_MP4);
}

#[test]
fn h264_in_mkv_plays_as_i420() {
    check(H264_MKV);
}

#[test]
fn hevc_main_in_mkv_plays_as_i420() {
    check(HEVC_MAIN);
}

#[test]
fn hevc_main10_in_mkv_plays_as_8_bit_i420() {
    check(HEVC_MAIN10);
}

/// BT.709 full-range clips keep their matrix and range through the 8-bit
/// copy (AV1 8-bit, H.264) and the 10-bit conversion (AV1 10-bit, HEVC
/// Main10).
#[test]
fn bt709_full_range_tags_pass_through() {
    let small = |clip: Clip| Clip {
        size: (320, 240),
        frames: 12,
        flat_from: 6,
        bt709: true,
        full_range: true,
        ..clip
    };
    for clip in [
        small(AV1_WEBM),
        small(AV1_10_MKV),
        small(H264_MKV),
        small(HEVC_MAIN10),
    ] {
        if let Some(why) = clip.codec.missing() {
            println!("skipped {} at BT.709 full range: {why}", clip.name);
            continue;
        }
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = match make(&clip, dir.path()) {
            Ok((path, _)) => path,
            Err(why) => {
                println!("skipped {} at BT.709 full range: {why}", clip.name);
                continue;
            }
        };
        let checked = decode_and_check(&clip, &path, clip.frames - 1);
        assert_eq!(checked.decoded.error, None);
        assert_eq!(checked.decoded.count, clip.frames, "{}", clip.name);
        let got = flat_colour(&clip, &checked.kept.unwrap());
        for (g, w) in got.iter().zip(FLAT) {
            assert!(
                (g - f64::from(w)).abs() <= 4.0,
                "{}: the flat colour shows as {got:?}, not {FLAT:?}",
                clip.name
            );
        }
    }
}

/// Run by [`speed_on_four_cores`] in a child process: decodes the clip
/// named by the environment and prints its speed.
#[test]
#[ignore = "run by the other tests in a child process"]
fn measure_in_child() {
    let Some(path) = std::env::var_os(MEASURE_ENV) else {
        return;
    };
    let path = PathBuf::from(path);
    let decoded = decode_each(&path, |_, _| {});
    assert_eq!(decoded.error, None);
    println!(
        "codecs-fps {}",
        decoded.count as f64 / decoded.took.as_secs_f64()
    );
}
