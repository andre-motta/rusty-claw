//! Menu tree files: the main menu, the pause menu and the game-over menu.
//!
//! Each menu is a tree of nodes. A node has a label, eight image names
//! (one per look of the entry: normal, pressed, highlighted, disabled and
//! their blinking variants), the command number the game runs when the
//! entry is chosen, and child nodes for its submenu. The file is the tree
//! written depth first, all integers little-endian:
//!
//! ```text
//! file  := node                          (nothing may follow it)
//! node  := u32 tag, str label, str slot[0..8], u32 command,
//!          i16 child_count, child_count x (u32 create, node)
//! str   := u8 n (n < 0xFF)
//!        | 0xFF u16 n (n < 0xFFFE)
//!        | 0xFF 0xFFFF u32 n
//!          then n bytes of single-byte text
//! ```
//!
//! - `tag` is a number stored with the node (0 in the game's own files) and
//!   kept as [`IfcNode::tag`].
//! - A child count of 0 or below means no children.
//! - `create` must be 0 before every child; any other value is refused.
//! - A length prefix of `0xFF 0xFE 0xFF` marks two-byte text, which the
//!   game's menu files never use; it is refused.
//!
//! Like every reader in this crate it treats its input as untrusted: nesting
//! is limited to [`MAX_DEPTH`] levels, a file to [`MAX_NODES`] nodes and
//! each text to [`MAX_TEXT`] bytes, lengths are checked against the bytes
//! left before anything is allocated, and nothing panics. Nothing in these
//! files is ever executed.

use crate::bytes::{i16_at, latin1, slice, u8_at, u16_at, u32_at};
use crate::error::{Result, format_error};
use crate::name::pack_type;

const FORMAT: &str = "menu tree";

/// Type code of menu tree files.
pub const TYPE_IFC: u32 = pack_type(b"IFC");

/// Image names per node.
pub const SLOTS: usize = 8;
/// Deepest nesting accepted, the root counting as level 1.
pub const MAX_DEPTH: usize = 16;
/// Most nodes in one file.
pub const MAX_NODES: usize = 4096;
/// Longest text accepted, in bytes.
pub const MAX_TEXT: usize = 1024;

/// One node of a menu tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IfcNode {
    /// The number stored in front of the node (0 in the game's files).
    pub tag: u32,
    /// The node's label.
    pub label: String,
    /// The eight image names; empty when unset.
    pub slots: [String; SLOTS],
    /// The command the entry runs.
    pub command: u32,
    /// The submenu's entries, in order.
    pub children: Vec<IfcNode>,
}

impl IfcNode {
    /// Every node of the tree, depth first, the root first.
    pub fn walk(&self) -> Vec<&IfcNode> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    /// The first node, depth first, whose command is `command`.
    pub fn find(&self, command: u32) -> Option<&IfcNode> {
        self.walk().into_iter().find(|n| n.command == command)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    nodes: usize,
}

impl Reader<'_> {
    fn fail<T>(&self, what: impl Into<String>) -> Result<T> {
        Err(format_error(
            FORMAT,
            format!("{} at byte {}", what.into(), self.pos),
        ))
    }

    fn u8(&mut self) -> Result<u8> {
        match u8_at(self.data, self.pos) {
            Some(v) => {
                self.pos += 1;
                Ok(v)
            }
            None => self.fail("file ends inside a node"),
        }
    }

    fn u16(&mut self) -> Result<u16> {
        match u16_at(self.data, self.pos) {
            Some(v) => {
                self.pos += 2;
                Ok(v)
            }
            None => self.fail("file ends inside a node"),
        }
    }

    fn i16(&mut self) -> Result<i16> {
        match i16_at(self.data, self.pos) {
            Some(v) => {
                self.pos += 2;
                Ok(v)
            }
            None => self.fail("file ends inside a node"),
        }
    }

    fn u32(&mut self) -> Result<u32> {
        match u32_at(self.data, self.pos) {
            Some(v) => {
                self.pos += 4;
                Ok(v)
            }
            None => self.fail("file ends inside a node"),
        }
    }

    fn text(&mut self) -> Result<String> {
        let n = match self.u8()? {
            0xFF => match self.u16()? {
                0xFFFE => return self.fail("two-byte text is not supported"),
                0xFFFF => self.u32()? as usize,
                n => usize::from(n),
            },
            n => usize::from(n),
        };
        if n > MAX_TEXT {
            return self.fail(format!("text of {n} bytes (at most {MAX_TEXT})"));
        }
        match slice(self.data, self.pos, n) {
            Some(b) => {
                self.pos += n;
                Ok(latin1(b))
            }
            None => self.fail(format!("text of {n} bytes runs past the end")),
        }
    }

    fn node(&mut self, depth: usize) -> Result<IfcNode> {
        if depth > MAX_DEPTH {
            return self.fail(format!("menus nested deeper than {MAX_DEPTH} levels"));
        }
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return self.fail(format!("more than {MAX_NODES} nodes"));
        }
        let tag = self.u32()?;
        let label = self.text()?;
        let mut slots: [String; SLOTS] = Default::default();
        for s in &mut slots {
            *s = self.text()?;
        }
        let command = self.u32()?;
        let count = self.i16()?.max(0) as usize;
        // Every child takes at least 4 + 4 + 9 + 4 + 2 bytes.
        let left = self.data.len().saturating_sub(self.pos);
        if count.saturating_mul(23) > left {
            return self.fail(format!("{count} children cannot fit in {left} bytes"));
        }
        let mut children = Vec::with_capacity(count);
        for _ in 0..count {
            let create = self.u32()?;
            if create != 0 {
                return self.fail(format!("child marker {create:#x} (only 0 is valid)"));
            }
            children.push(self.node(depth + 1)?);
        }
        Ok(IfcNode {
            tag,
            label,
            slots,
            command,
            children,
        })
    }
}

/// Reads a menu tree file. The file must hold exactly one tree.
pub fn parse(data: &[u8]) -> Result<IfcNode> {
    let mut r = Reader {
        data,
        pos: 0,
        nodes: 0,
    };
    let root = r.node(1)?;
    if r.pos != data.len() {
        return r.fail(format!("{} bytes after the tree", data.len() - r.pos));
    }
    Ok(root)
}

fn put_text(out: &mut Vec<u8>, text: &str) {
    let bytes: Vec<u8> = text
        .chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect();
    let n = bytes.len();
    if n < 0xFF {
        out.push(n as u8);
    } else if n < 0xFFFE {
        out.push(0xFF);
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else {
        out.push(0xFF);
        out.extend_from_slice(&0xFFFFu16.to_le_bytes());
        out.extend_from_slice(&(n as u32).to_le_bytes());
    }
    out.extend_from_slice(&bytes);
}

fn put_node(out: &mut Vec<u8>, node: &IfcNode) {
    out.extend_from_slice(&node.tag.to_le_bytes());
    put_text(out, &node.label);
    for s in &node.slots {
        put_text(out, s);
    }
    out.extend_from_slice(&node.command.to_le_bytes());
    let count = i16::try_from(node.children.len()).unwrap_or(i16::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for child in node.children.iter().take(count as usize) {
        out.extend_from_slice(&0u32.to_le_bytes());
        put_node(out, child);
    }
}

/// Writes `root` as a menu tree file, the inverse of [`parse`]. Characters
/// outside single-byte text are written as `?`.
pub fn build(root: &IfcNode) -> Vec<u8> {
    let mut out = Vec::new();
    put_node(&mut out, root);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(command: u32, normal: &str) -> IfcNode {
        let mut n = IfcNode {
            label: format!("item {command}"),
            command,
            ..IfcNode::default()
        };
        n.slots[0] = normal.to_string();
        n.slots[3] = format!("{normal}_lit");
        n
    }

    #[test]
    fn round_trips_a_small_tree() {
        let mut root = leaf(100, "001_main_root");
        let mut sub = leaf(101, "002_main_sub");
        sub.children.push(leaf(201, "003_sub_a"));
        root.children = vec![sub, leaf(102, "004_main_quit")];
        let bytes = build(&root);
        assert_eq!(parse(&bytes).unwrap(), root);
        assert_eq!(
            root.find(201).map(|n| n.slots[0].as_str()),
            Some("003_sub_a")
        );
        assert_eq!(
            root.walk().iter().map(|n| n.command).collect::<Vec<_>>(),
            vec![100, 101, 201, 102]
        );
    }

    #[test]
    fn long_texts_use_the_wide_length_forms() {
        let mut n = leaf(1, "x");
        n.label = "a".repeat(300);
        let bytes = build(&n);
        assert_eq!(&bytes[4..7], &[0xFF, 0x2C, 0x01]);
        assert_eq!(parse(&bytes).unwrap().label.len(), 300);
    }
}
