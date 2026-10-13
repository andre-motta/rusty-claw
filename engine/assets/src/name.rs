//! Naming rules: logical paths, case folding, type codes, frame numbers and
//! registry keys.
//!
//! Folder and file lookups are case-insensitive with ASCII-only folding, as in
//! the C locale. A file is identified by its folder, its name without
//! extension and a type code built from its extension, so `MAIN.PAL` and
//! `MAIN.XMI` in one folder are two distinct entries.

use crate::error::{AssetError, Result};

/// Longest name segment accepted in a logical path or an archive.
pub const MAX_NAME_LEN: usize = 0x3FF;

/// Separator between the parts of a registry key.
pub const REGISTRY_SEPARATOR: &str = "_";

/// Packs up to four extension bytes into a type code, first byte in the most
/// significant used byte, so `PAL` packs to `0x50_41_4C`.
pub const fn pack_type(ext: &[u8]) -> u32 {
    let mut code = 0u32;
    let mut i = 0;
    while i < ext.len() && i < 4 {
        code = (code << 8) | ext[i] as u32;
        i += 1;
    }
    code
}

/// Type code of PID images.
pub const TYPE_PID: u32 = pack_type(b"PID");
/// Type code of PCX images.
pub const TYPE_PCX: u32 = pack_type(b"PCX");
/// Type code of BMP images.
pub const TYPE_BMP: u32 = pack_type(b"BMP");
/// Type code of palettes.
pub const TYPE_PAL: u32 = pack_type(b"PAL");
/// Type code of colour lookup tables.
pub const TYPE_CLT: u32 = pack_type(b"CLT");
/// Type code of level files.
pub const TYPE_WWD: u32 = pack_type(b"WWD");
/// Type code of music files.
pub const TYPE_XMI: u32 = pack_type(b"XMI");
/// Type code of sound files.
pub const TYPE_WAV: u32 = pack_type(b"WAV");
/// Type code of animation files.
pub const TYPE_ANI: u32 = pack_type(b"ANI");

/// Upper-cases ASCII letters only.
pub fn upper(name: &str) -> String {
    name.to_ascii_uppercase()
}

/// Folds ASCII letters to lower case for case-insensitive comparison.
pub fn fold(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// True when every character fits in one byte (Latin-1), the only names the
/// asset tree can hold.
pub fn is_single_byte(name: &str) -> bool {
    name.chars().all(|c| u32::from(c) <= 0xFF)
}

/// Type code of an extension (with or without its leading dot).
///
/// Returns `None` for an extension longer than four characters or one that
/// is not single-byte text.
pub fn type_code(ext: &str) -> Option<u32> {
    let ext = ext.strip_prefix('.').unwrap_or(ext);
    if !is_single_byte(ext) || ext.chars().count() > 4 {
        return None;
    }
    let bytes: Vec<u8> = ext.chars().map(|c| u32::from(c) as u8).collect();
    Some(pack_type(&bytes))
}

/// Extension text of a type code; 0 gives the empty string.
pub fn type_ext(code: u32) -> String {
    code.to_be_bytes()
        .iter()
        .skip_while(|&&b| b == 0)
        .map(|&b| char::from(b))
        .collect()
}

/// Splits a file name found in a folder into its upper-cased name and type.
///
/// The extension is everything after the last dot. An extension of up to
/// four characters becomes the type code; a longer one stays part of the
/// name with type 0, as does a name without extension.
pub fn split_filename(file_name: &str) -> (String, u32) {
    match file_name.rfind('.') {
        None => (upper(file_name), 0),
        Some(dot) => {
            let (base, ext) = (&file_name[..dot], &file_name[dot + 1..]);
            if ext.chars().count() > 4 {
                return (upper(file_name), 0);
            }
            if ext.is_empty() {
                return (upper(base), 0);
            }
            match type_code(&upper(ext)) {
                Some(code) => (upper(base), code),
                None => (upper(file_name), 0),
            }
        }
    }
}

/// C `atol` over a run of ASCII digits, wrapping at 32 bits.
fn atol_digits(digits: &[u8]) -> i32 {
    let mut value: u32 = 0;
    for &d in digits {
        value = value.wrapping_mul(10).wrapping_add(u32::from(d - b'0'));
    }
    value as i32
}

/// Frame number of an image file: skip every non-digit, then read the digit
/// run that follows. `FRAME001` and `001` are frame 1, `A1B2` is frame 1 and
/// a name without digits is frame 0.
pub fn frame_index(name: &str) -> i32 {
    let bytes = name.as_bytes();
    let start = bytes
        .iter()
        .position(u8::is_ascii_digit)
        .unwrap_or(bytes.len());
    let len = bytes[start..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    atol_digits(&bytes[start..start + len])
}

/// First number in a text, if it has one.
pub fn first_number(text: &str) -> Option<i32> {
    let bytes = text.as_bytes();
    let start = bytes.iter().position(u8::is_ascii_digit)?;
    let len = bytes[start..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    Some(atol_digits(&bytes[start..start + len]))
}

/// Registry key of a child folder or file: the prefix and the name joined by
/// `_`, or the name alone when the prefix is empty.
///
/// So `IMAGES/RAT` mapped with prefix `LEVEL` is `LEVEL_RAT`, and
/// `TILES/ACTION` mapped with the empty prefix is `ACTION`.
pub fn registry_key(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}{REGISTRY_SEPARATOR}{name}")
    }
}

fn path_error(path: &str, reason: &'static str) -> AssetError {
    AssetError::Path {
        path: path.to_string(),
        reason,
    }
}

/// Splits a logical path into its segments.
///
/// Either `/` or `\` separates segments; empty and `.` segments are dropped.
/// Absolute paths, drive letters, NUL and `..` segments are rejected, so a
/// logical path can never leave the root it is resolved against.
pub fn split_logical(path: &str) -> Result<Vec<&str>> {
    if path.contains('\0') {
        return Err(path_error(path, "contains NUL"));
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(path_error(path, "drive paths are not allowed"));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(path_error(path, "absolute paths are not allowed"));
    }
    let mut out = Vec::new();
    for seg in path.split(['/', '\\']) {
        match seg {
            "" | "." => continue,
            ".." => return Err(path_error(path, "'..' is not allowed")),
            _ => {}
        }
        if seg.len() > MAX_NAME_LEN {
            return Err(path_error(path, "segment too long"));
        }
        out.push(seg);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_codes_pack_big_endian() {
        assert_eq!(
            TYPE_PAL,
            (u32::from(b'P') << 16) | (u32::from(b'A') << 8) | u32::from(b'L')
        );
        assert_eq!(type_code("pal"), Some(pack_type(b"pal")));
        assert_eq!(type_code(".PID"), Some(TYPE_PID));
        assert_eq!(type_code(""), Some(0));
        assert_eq!(type_code("ABCDE"), None);
        assert_eq!(type_ext(TYPE_WWD), "WWD");
        assert_eq!(type_ext(0), "");
        assert_eq!(type_ext(pack_type(b"ABCD")), "ABCD");
    }

    #[test]
    fn filenames_split_like_the_folder_scan() {
        assert_eq!(split_filename("main.pal"), ("MAIN".into(), TYPE_PAL));
        assert_eq!(split_filename("001.PID"), ("001".into(), TYPE_PID));
        assert_eq!(split_filename("README"), ("README".into(), 0));
        assert_eq!(
            split_filename("script.lua"),
            ("SCRIPT".into(), pack_type(b"LUA"))
        );
        assert_eq!(split_filename("notes.backup"), ("NOTES.BACKUP".into(), 0));
        assert_eq!(split_filename("a.b.wav"), ("A.B".into(), TYPE_WAV));
        assert_eq!(split_filename("trail."), ("TRAIL".into(), 0));
    }

    #[test]
    fn frame_numbers() {
        assert_eq!(frame_index("FRAME001"), 1);
        assert_eq!(frame_index("001"), 1);
        assert_eq!(frame_index("064"), 64);
        assert_eq!(frame_index("A1B2"), 1);
        assert_eq!(frame_index("NOTE"), 0);
        assert_eq!(frame_index("4294967297"), 1);
        assert_eq!(first_number("Level 12 remix"), Some(12));
        assert_eq!(first_number("none"), None);
    }

    #[test]
    fn registry_keys() {
        assert_eq!(registry_key("", "ACTION"), "ACTION");
        assert_eq!(registry_key("LEVEL", "RAT"), "LEVEL_RAT");
        assert_eq!(
            registry_key(&registry_key("GAME", "TREASURE"), "COINS"),
            "GAME_TREASURE_COINS"
        );
    }

    #[test]
    fn logical_paths() {
        assert_eq!(
            split_logical("LEVEL1\\palettes/MAIN.PAL").unwrap(),
            vec!["LEVEL1", "palettes", "MAIN.PAL"]
        );
        assert_eq!(split_logical("a//./b").unwrap(), vec!["a", "b"]);
        assert!(split_logical("").unwrap().is_empty());
        for bad in [
            "../x",
            "a/../b",
            "a\\..\\b",
            "/etc/passwd",
            "\\\\server\\share",
            "C:\\x",
            "c:x",
            "a\0b",
            "..",
        ] {
            assert!(split_logical(bad).is_err(), "{bad:?} accepted");
        }
        let long = "x".repeat(MAX_NAME_LEN + 1);
        assert!(split_logical(&long).is_err());
    }
}
