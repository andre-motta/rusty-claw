//! Menu tree files: synthetic trees, malformed files, fuzzing, and every
//! menu file of your own install when `CLAW_INSTALL` is set.

use std::path::PathBuf;

use claw_assets::ifc::{self, IfcNode, MAX_DEPTH, MAX_NODES, MAX_TEXT, TYPE_IFC};
use claw_assets::{DirNode, Install};
use proptest::prelude::*;

fn node(command: u32, normal: &str, children: Vec<IfcNode>) -> IfcNode {
    let mut n = IfcNode {
        label: format!("entry {command}"),
        command,
        children,
        ..IfcNode::default()
    };
    n.slots[0] = normal.to_string();
    n.slots[3] = format!("{normal}_hi");
    n
}

fn menu() -> IfcNode {
    node(
        1000,
        "001_main_root",
        vec![
            node(
                1001,
                "001_main_play",
                vec![
                    node(2001, "001_play_a", vec![]),
                    node(2002, "002_play_b", vec![]),
                ],
            ),
            node(
                1002,
                "002_main_quit",
                vec![node(3001, "001_quit_yes", vec![])],
            ),
        ],
    )
}

/// The bytes of a node by hand, to pin the layout.
fn text(out: &mut Vec<u8>, s: &[u8]) {
    out.push(s.len() as u8);
    out.extend_from_slice(s);
}

#[test]
fn reads_the_layout_byte_by_byte() {
    let mut b = Vec::new();
    b.extend_from_slice(&7u32.to_le_bytes());
    text(&mut b, b"Root");
    text(&mut b, b"001_main_x");
    for _ in 0..2 {
        text(&mut b, b"");
    }
    text(&mut b, b"002_main_x");
    for _ in 0..4 {
        text(&mut b, b"");
    }
    b.extend_from_slice(&500u32.to_le_bytes());
    b.extend_from_slice(&1i16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    text(&mut b, b"Child");
    for _ in 0..8 {
        text(&mut b, b"");
    }
    b.extend_from_slice(&501u32.to_le_bytes());
    b.extend_from_slice(&(-3i16).to_le_bytes());
    let root = ifc::parse(&b).unwrap();
    assert_eq!(root.tag, 7);
    assert_eq!(root.label, "Root");
    assert_eq!(root.slots[0], "001_main_x");
    assert_eq!(root.slots[3], "002_main_x");
    assert_eq!(root.command, 500);
    assert_eq!(root.children.len(), 1);
    let child = &root.children[0];
    assert_eq!((child.label.as_str(), child.command), ("Child", 501));
    assert!(child.children.is_empty(), "a negative count means none");
    assert!(child.slots.iter().all(String::is_empty));
}

#[test]
fn synthetic_trees_round_trip() {
    let m = menu();
    let bytes = ifc::build(&m);
    let back = ifc::parse(&bytes).unwrap();
    assert_eq!(back, m);
    assert_eq!(back.walk().len(), 6);
    assert_eq!(back.find(3001).unwrap().slots[0], "001_quit_yes");
    assert!(back.find(9999).is_none());
}

#[test]
fn non_zero_child_markers_are_refused() {
    let bytes = ifc::build(&menu());
    // The first child marker follows the root's fields.
    let mut root_only = menu();
    root_only.children.clear();
    let at = ifc::build(&root_only).len();
    for marker in [1u32, 0x8000_0000, u32::MAX] {
        let mut bad = bytes.clone();
        bad[at..at + 4].copy_from_slice(&marker.to_le_bytes());
        let e = ifc::parse(&bad).unwrap_err().to_string();
        assert!(e.contains("child marker"), "{e}");
    }
}

#[test]
fn truncated_and_padded_files_are_refused() {
    let bytes = ifc::build(&menu());
    for cut in 0..bytes.len() {
        assert!(ifc::parse(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    let mut longer = bytes.clone();
    longer.push(0);
    let e = ifc::parse(&longer).unwrap_err().to_string();
    assert!(e.contains("after the tree"), "{e}");
}

#[test]
fn two_byte_text_and_oversized_text_are_refused() {
    let mut b = 0u32.to_le_bytes().to_vec();
    b.extend_from_slice(&[0xFF, 0xFE, 0xFF, 2, b'h', 0, b'i', 0]);
    assert!(ifc::parse(&b).is_err());
    let mut n = node(1, "x", vec![]);
    n.label = "y".repeat(MAX_TEXT);
    assert!(ifc::parse(&ifc::build(&n)).is_ok());
    n.label.push('y');
    let e = ifc::parse(&ifc::build(&n)).unwrap_err().to_string();
    assert!(e.contains("at most"), "{e}");
    // A four-byte length beyond the file is refused before allocating.
    let mut b = 0u32.to_le_bytes().to_vec();
    b.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
    assert!(ifc::parse(&b).is_err());
}

#[test]
fn depth_and_node_count_are_bounded() {
    let mut deep = node(1, "x", vec![]);
    for i in 0..MAX_DEPTH - 1 {
        deep = node(i as u32 + 2, "x", vec![deep]);
    }
    assert!(ifc::parse(&ifc::build(&deep)).is_ok());
    let deeper = node(99, "x", vec![deep]);
    let e = ifc::parse(&ifc::build(&deeper)).unwrap_err().to_string();
    assert!(e.contains("nested deeper"), "{e}");

    let wide = |n: usize| node(0, "x", (0..n).map(|i| node(i as u32, "", vec![])).collect());
    // A root plus its children.
    assert!(ifc::parse(&ifc::build(&wide(MAX_NODES - 1))).is_ok());
    let e = ifc::parse(&ifc::build(&{
        let mut w = wide(MAX_NODES - 1);
        w.children[0].children.push(node(1, "", vec![]));
        w
    }))
    .unwrap_err()
    .to_string();
    assert!(e.contains("more than"), "{e}");
}

#[test]
fn a_child_count_larger_than_the_file_is_refused_at_once() {
    let mut b = ifc::build(&node(1, "x", vec![]));
    let n = b.len();
    b[n - 2..].copy_from_slice(&i16::MAX.to_le_bytes());
    let e = ifc::parse(&b).unwrap_err().to_string();
    assert!(e.contains("cannot fit"), "{e}");
}

fn arb_node() -> impl Strategy<Value = IfcNode> {
    let leaf = (
        any::<u32>(),
        "[ -~]{0,12}",
        proptest::collection::vec("[0-9a-z_]{0,20}", 8),
        any::<u32>(),
    )
        .prop_map(|(tag, label, slots, command)| {
            let mut n = IfcNode {
                tag,
                label,
                command,
                ..IfcNode::default()
            };
            for (d, s) in n.slots.iter_mut().zip(slots) {
                *d = s;
            }
            n
        });
    leaf.prop_recursive(4, 40, 6, |inner| {
        (inner.clone(), proptest::collection::vec(inner, 0..6)).prop_map(|(mut n, children)| {
            n.children = children;
            n
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = ifc::parse(&data);
    }

    #[test]
    fn arbitrary_trees_round_trip(tree in arb_node()) {
        prop_assert_eq!(ifc::parse(&ifc::build(&tree)).unwrap(), tree);
    }

    #[test]
    fn corrupted_trees_never_panic(
        tree in arb_node(),
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..16),
    ) {
        let mut data = ifc::build(&tree);
        let len = data.len();
        for (pos, value) in edits {
            data[pos % len] = value;
        }
        let _ = ifc::parse(&data);
    }
}

fn ifc_files(dir: &DirNode, out: &mut Vec<claw_assets::FileId>) {
    out.extend(dir.files_of_type(TYPE_IFC));
    for sub in dir.dirs() {
        ifc_files(sub, out);
    }
}

#[test]
fn every_install_menu_file_parses() {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let mut ids = Vec::new();
    ifc_files(store.root(), &mut ids);
    assert!(ids.len() >= 3, "only {} menu files found", ids.len());
    for id in &ids {
        let path = store.file(*id).path().to_string();
        let data = store.read(*id).unwrap();
        let tree = ifc::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(ifc::build(&tree), data, "{path} round trip");
        for n in tree.walk() {
            assert!(!n.slots[0].is_empty() && !n.slots[3].is_empty(), "{path}");
        }
    }
    eprintln!("{} menu files parse", ids.len());
}
