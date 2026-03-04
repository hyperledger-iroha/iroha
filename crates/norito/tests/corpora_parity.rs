//! Parity tests for large corpora (citm_catalog/twitter), if present locally.
//! Looks for JSON and matching .tape files under `crates/norito/tests/corpora/`.
#![cfg(feature = "json")]

use std::{fs, path::Path};

use norito::json::build_struct_index;

#[test]
fn corpora_parity_if_present() {
    let dir = Path::new("crates/norito/tests/corpora");
    if !dir.exists() {
        eprintln!("corpora dir not present; skipping");
        return;
    }
    let mut found_any = false;
    for entry in fs::read_dir(dir).expect("read corpora dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        let tape_path = dir.join(format!("{stem}.tape"));
        if !tape_path.exists() {
            eprintln!("no .tape for {stem} — skipping");
            continue;
        }
        found_any = true;
        let json = fs::read_to_string(&path).expect("read json");
        let got = build_struct_index(&json);
        let expected: Vec<u32> = fs::read_to_string(&tape_path)
            .expect("read tape")
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.parse::<u32>().unwrap())
                }
            })
            .collect();
        assert_eq!(expected, got.offsets, "fixture={stem}");
    }
    if !found_any {
        eprintln!("no corpora .json+.tape pairs found; skipping");
    }
}
