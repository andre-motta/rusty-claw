// The record windows/build.sh writes for a finished build, worked out from
// the scripts themselves. build.rs includes this file (`include!`), and
// engine/movie/tests/windows_record.rs checks it against the output of
// `sh third_party/ffmpeg/windows/build.sh --stamp`, so the two cannot drift
// apart unnoticed. Outer comments only: an included file takes no inner
// attributes.

/// The record windows/build.sh writes (`--stamp`), without its last line
/// break: the pins of build.sh (`pins`), the configure line turned for MSVC
/// as windows/build.sh (`windows`) turns it, and its WINDOWS_FLAGS; with
/// dav1d unless `av1` is false (RUSTY_CLAW_FFMPEG_AV1=off), and with the
/// H.264 and HEVC decoders when `h26x` (RUSTY_CLAW_FFMPEG_H26X=on).
pub fn windows_record(pins: &str, windows: &str, h26x: bool, av1: bool) -> Option<String> {
    let value = |name: &str| shell_value(pins, name);
    let mut unix = value("CONFIGURE_FLAGS")?;
    if av1 {
        unix = format!("{unix} {}", value("AV1_FLAGS")?);
    }
    if h26x {
        unix = format!("{unix} {}", value("H26X_FLAGS")?);
    }
    let mut flags: Vec<&str> = Vec::new();
    for flag in unix.split_whitespace() {
        match flag {
            "--enable-pthreads" => flags.push("--enable-w32threads"),
            "--enable-pic" => {}
            other => flags.push(other),
        }
    }
    let windows_flags = shell_value(windows, "WINDOWS_FLAGS")?;
    let dav1d = if av1 {
        format!(
            "dav1d {} sha256 {}",
            value("DAV1D_VERSION")?,
            value("DAV1D_SHA256")?
        )
    } else {
        "dav1d none".to_string()
    };
    Some(format!(
        "FFmpeg {}\nsource {}\nsha256 {}\nnasm {} sha256 {}\n{dav1d}\n\
         licence LGPL-2.1-or-later\nconfigure {} {windows_flags}",
        value("FFMPEG_VERSION")?,
        value("FFMPEG_URL")?,
        value("FFMPEG_SHA256")?,
        value("NASM_VERSION")?,
        value("NASM_SHA256")?,
        flags.join(" "),
    ))
}

/// The value of the shell assignment `NAME=...` at the start of a line of
/// `text`, as sh reads it: a double-quoted value may span lines joined by
/// a backslash at their end, and `$NAME` of an earlier assignment is
/// replaced by its value. `None` for anything else (single quotes, `${`,
/// command substitution), which the pins never use.
pub fn shell_value(text: &str, name: &str) -> Option<String> {
    let start = text
        .lines()
        .position(|l| l.starts_with(&format!("{name}=")))?;
    let first = &text.lines().nth(start)?[name.len() + 1..];
    let raw = if let Some(rest) = first.strip_prefix('"') {
        let mut joined = String::new();
        let mut line = rest;
        let mut next = start + 1;
        loop {
            if let Some(end) = line.find('"') {
                joined.push_str(&line[..end]);
                break;
            }
            joined.push_str(line.strip_suffix('\\')?);
            line = text.lines().nth(next)?;
            next += 1;
        }
        joined
    } else {
        first.trim_end().to_string()
    };
    if raw.contains(['\'', '`', '{', '(']) {
        return None;
    }
    let mut out = String::new();
    let mut rest = raw.as_str();
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let len = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(tail.len());
        let var = &tail[..len];
        out.push_str(&shell_value(
            &text[..text.find(&format!("\n{name}="))?],
            var,
        )?);
        rest = &tail[len..];
    }
    out.push_str(rest);
    Some(out)
}
