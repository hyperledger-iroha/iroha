//! Validate struct index against on-disk golden fixtures (.json + .tape).
#![cfg(feature = "json")]

use std::{fs, path::Path};

use norito::json::build_struct_index;

fn load_tape(path: &Path) -> Vec<u32> {
    let s = fs::read_to_string(path).expect("read tape");
    s.lines()
        .filter_map(|l| {
            let ll = l.trim();
            if ll.is_empty() {
                None
            } else {
                Some(ll.parse::<u32>().expect("parse tape"))
            }
        })
        .collect()
}

#[test]
fn fixtures_parity() {
    // Use crate-relative path so tests work regardless of CWD
    let dir = Path::new("tests/data");
    let entries = fs::read_dir(dir).expect("data dir");
    for ent in entries {
        let ent = ent.unwrap();
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
            let tape_path = dir.join(format!("{stem}.tape"));
            let json = fs::read_to_string(&path).expect("read json");
            let expected = load_tape(&tape_path);
            let got = build_struct_index(&json);
            assert_eq!(expected, got.offsets, "fixture={stem}");
        }
    }
}
