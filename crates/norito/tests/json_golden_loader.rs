//! File-based golden tests for canonical JSON writing.
//!
//! Loads pairs of `*.in.json`/`*.out.json` from `tests/data/json_golden/` and
//! asserts that the native writer canonicalizes the input into
//! the expected output. Also verifies that parsing the output round-trips to an
//! equivalent `Value`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use norito::json;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("json_golden")
}

#[test]
fn json_writer_corpus_goldens() {
    let dir = golden_dir();
    let entries = fs::read_dir(&dir).expect("read golden dir");
    let mut cases = Vec::new();
    for e in entries {
        let p = e.unwrap().path();
        if let Some(name) = p.file_name().and_then(|s| s.to_str())
            && name.ends_with(".in.json")
        {
            let base = name.trim_end_matches(".in.json");
            cases.push(base.to_string());
        }
    }
    assert!(!cases.is_empty(), "no golden cases found in {dir:?}");
    for base in cases {
        let pin = dir.join(format!("{base}.in.json"));
        let pout = dir.join(format!("{base}.out.json"));
        let sin = fs::read_to_string(&pin).expect("read in");
        let sout = fs::read_to_string(&pout).expect("read out");
        let v = json::parse_value(&sin).expect("parse in");
        let can = json::to_string(&v).expect("stringify");
        assert_eq!(can, sout.trim(), "canonical mismatch for base={base}");
        // Round-trip: parse expected and compare Values for semantic equivalence
        let v2 = json::parse_value(&sout).expect("parse out");
        assert_eq!(v, v2, "roundtrip mismatch for base={base}");
    }
}
