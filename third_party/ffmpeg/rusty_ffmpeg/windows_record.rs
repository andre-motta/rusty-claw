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

/// Which finished Windows build cargo links, given `find(h26x, av1)`, the
/// build windows/build.sh made with those switches if there is one: the one
/// for the switches asked for (`h26x`, and AV1 unless `av1_off`); without
/// `require`, when AV1 is asked for and no build has it, the one without
/// AV1 next (cargo then warns that it decodes no AV1). When there is none,
/// `Err` holds the cargo settings that would select a build that is there
/// with other switches, if there is one, nearest first.
pub fn windows_pick<T>(
    h26x: bool,
    av1_off: bool,
    require: bool,
    mut find: impl FnMut(bool, bool) -> Option<T>,
) -> Result<T, Option<String>> {
    let av1 = !av1_off;
    // Without `require`, a build without AV1 is taken when AV1 is asked for.
    let takes_no_av1 = av1 && !require;
    if let Some(found) = find(h26x, av1) {
        return Ok(found);
    }
    // (Not a let chain: build.rs is edition 2021, the test edition 2024.)
    let stand_in = if takes_no_av1 {
        find(h26x, false)
    } else {
        None
    };
    if let Some(found) = stand_in {
        return Ok(found);
    }
    for (other_h26x, other_av1) in [(h26x, !av1), (!h26x, av1), (!h26x, !av1)] {
        if (other_h26x, other_av1) == (h26x, false) && takes_no_av1 {
            continue;
        }
        if find(other_h26x, other_av1).is_none() {
            continue;
        }
        let mut settings = Vec::new();
        if other_h26x != h26x {
            let on = if other_h26x { "on" } else { "off" };
            settings.push(format!("RUSTY_CLAW_FFMPEG_H26X={on}"));
        }
        // A build without AV1 needs no switch where one stands in anyway.
        if other_av1 != av1 && (other_av1 || !takes_no_av1) {
            let value = if other_av1 { "auto" } else { "off" };
            settings.push(format!("RUSTY_CLAW_FFMPEG_AV1={value}"));
        }
        return Err(Some(settings.join(" and ")));
    }
    Err(None)
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

// Runs with the test that includes this file as a module
// (engine/movie/tests/windows_record.rs); build.rs never builds it.
#[cfg(test)]
mod pick_tests {
    use super::windows_pick;

    /// windows_pick over the builds `there`, as (h26x, av1) switches.
    fn pick(
        there: &[(bool, bool)],
        h26x: bool,
        av1_off: bool,
        require: bool,
    ) -> Result<(bool, bool), Option<String>> {
        windows_pick(h26x, av1_off, require, |h, a| {
            there.contains(&(h, a)).then_some((h, a))
        })
    }

    #[test]
    fn the_build_for_the_switches_comes_first() {
        let all = [(false, true), (false, false), (true, true), (true, false)];
        for h26x in [false, true] {
            for av1_off in [false, true] {
                for require in [false, true] {
                    assert_eq!(pick(&all, h26x, av1_off, require), Ok((h26x, !av1_off)));
                }
            }
        }
    }

    #[test]
    fn without_require_a_build_without_av1_stands_in() {
        assert_eq!(
            pick(&[(false, false)], false, false, false),
            Ok((false, false))
        );
        assert_eq!(
            pick(&[(true, false)], true, false, false),
            Ok((true, false))
        );
        // Under require only the exact build counts, and the setting that
        // selects the one there is named.
        assert_eq!(
            pick(&[(false, false)], false, false, true),
            Err(Some("RUSTY_CLAW_FFMPEG_AV1=off".to_string()))
        );
    }

    #[test]
    fn a_build_with_other_switches_is_named() {
        // Asked without AV1, a build with AV1 is there.
        assert_eq!(
            pick(&[(false, true)], false, true, false),
            Err(Some("RUSTY_CLAW_FFMPEG_AV1=auto".to_string()))
        );
        // Asked for H.264, only builds without it are there: the switch alone
        // selects one, as a build without AV1 stands in without require.
        assert_eq!(
            pick(&[(false, false)], true, false, false),
            Err(Some("RUSTY_CLAW_FFMPEG_H26X=off".to_string()))
        );
        assert_eq!(
            pick(&[(false, false)], true, false, true),
            Err(Some(
                "RUSTY_CLAW_FFMPEG_H26X=off and RUSTY_CLAW_FFMPEG_AV1=off".to_string()
            ))
        );
        assert_eq!(
            pick(&[(true, true)], false, true, false),
            Err(Some(
                "RUSTY_CLAW_FFMPEG_H26X=on and RUSTY_CLAW_FFMPEG_AV1=auto".to_string()
            ))
        );
        assert_eq!(pick(&[], false, false, false), Err(None));
    }
}
