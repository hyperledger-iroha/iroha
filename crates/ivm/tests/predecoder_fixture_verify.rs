//! Verify predecoder golden fixtures when present on disk.
//!
//! These tests are optional: if the fixture directory does not exist
//! (not generated yet), they log a message and return early without failing.

use std::{fs, path::PathBuf};

use ivm::ivm_cache::IvmCache;
use norito::json::Value;
use sha2::{Digest, Sha256};

fn fixtures_root() -> PathBuf {
    ivm::predecoder_fixtures::default_predecoder_mixed_root()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[test]
fn fixture_decoded_json_matches_decoder() {
    let root = fixtures_root();
    if !root.exists() {
        // Generate on demand with a coarse lock to avoid parallel races.
        let parent = root.parent().expect("has parent dir");
        fs::create_dir_all(parent).expect("create parent dirs");
        let lock_path = parent.join(".predecoder_mixed.lock");
        let lock = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path);
        match lock {
            Ok(_) => {
                ivm::predecoder_fixtures::generate_predecoder_mixed_fixtures(&root)
                    .expect("generate fixtures");
                let _ = std::fs::remove_file(&lock_path);
            }
            Err(_) => {
                // Another thread/process is generating. Wait briefly until ready.
                for _ in 0..100 {
                    if root.exists() && root.join("index.json").exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
    let code = fs::read(root.join("code.bin")).expect("read code.bin");
    let decoded = IvmCache::decode_stream(&code).expect("decode ok");

    let decoded_json = fs::read(root.join("decoded.json")).expect("read decoded.json");
    let v: Value = norito::json::from_slice(&decoded_json).expect("parse json");

    // format string should match
    let fmt = match &v {
        Value::Object(m) => m.get("format").and_then(|v| v.as_str()).unwrap_or(""),
        _ => "",
    };
    assert_eq!(fmt, "ivm.predecoder.v1");

    // code sha256 should match
    let expected_sha = match &v {
        Value::Object(m) => m
            .get("code_sha256")
            .and_then(|v| v.as_str())
            .expect("code_sha256 present"),
        _ => panic!("decoded.json is not an object"),
    };
    assert_eq!(expected_sha, sha256_hex(&code));

    // Compare decoded ops
    let arr = match &v {
        Value::Object(m) => m
            .get("decoded")
            .and_then(|v| v.as_array())
            .expect("decoded array present"),
        _ => panic!("decoded.json is not an object"),
    };
    assert_eq!(arr.len(), decoded.len(), "decoded length must match");
    for (i, op) in decoded.iter().enumerate() {
        let item = &arr[i];
        let (pc, len, inst) = match item {
            Value::Object(m) => (
                m.get("pc").and_then(|v| v.as_u64()).expect("pc"),
                m.get("len").and_then(|v| v.as_u64()).expect("len"),
                m.get("inst").and_then(|v| v.as_u64()).expect("inst"),
            ),
            _ => panic!("decoded item is not object"),
        };
        assert_eq!(pc, op.pc, "pc mismatch at {i}");
        assert_eq!(len as u32, op.len, "len mismatch at {i}");
        assert_eq!(inst as u32, op.inst, "inst mismatch at {i}");
    }
}

#[test]
fn fixture_artifacts_conform_to_index() {
    let root = fixtures_root();
    if !root.exists() {
        let parent = root.parent().expect("has parent dir");
        fs::create_dir_all(parent).expect("create parent dirs");
        let lock_path = parent.join(".predecoder_mixed.lock");
        let lock = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path);
        match lock {
            Ok(_) => {
                ivm::predecoder_fixtures::generate_predecoder_mixed_fixtures(&root)
                    .expect("generate fixtures");
                let _ = std::fs::remove_file(&lock_path);
            }
            Err(_) => {
                for _ in 0..100 {
                    if root.exists() && root.join("index.json").exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
    // Load code and decoded reference
    let code = fs::read(root.join("code.bin")).expect("read code.bin");
    let decoded_ref = IvmCache::decode_stream(&code).expect("decode ok");

    // Load index.json
    let index_bytes = fs::read(root.join("index.json")).expect("read index.json");
    let idx: Value = norito::json::from_slice(&index_bytes).expect("parse json");
    let fmt = match &idx {
        Value::Object(m) => m.get("format").and_then(|v| v.as_str()).unwrap_or(""),
        _ => "",
    };
    assert_eq!(fmt, "ivm.predecoder.v1/index");

    let artifacts_dir = match &idx {
        Value::Object(m) => {
            let rel = m
                .get("artifacts_dir")
                .and_then(|v| v.as_str())
                .expect("artifacts_dir present");
            root.join(rel)
        }
        _ => panic!("index.json is not object"),
    };

    let artifacts = match &idx {
        Value::Object(m) => m
            .get("artifacts")
            .and_then(|v| v.as_array())
            .expect("artifacts array present"),
        _ => panic!("index.json is not object"),
    };

    // For each artifact, verify sha256 and that decoding matches the reference.
    for entry in artifacts {
        let (fname, expected_sha) = match entry {
            Value::Object(m) => (
                m.get("file").and_then(|v| v.as_str()).expect("file name"),
                m.get("artifact_sha256")
                    .and_then(|v| v.as_str())
                    .expect("artifact sha"),
            ),
            _ => panic!("artifact entry is not object"),
        };
        let bytes = fs::read(artifacts_dir.join(fname)).expect("read artifact");
        assert_eq!(sha256_hex(&bytes), expected_sha, "sha mismatch for {fname}");
        let (_meta, decoded) = IvmCache::decode_artifact(&bytes).expect("artifact decode");
        assert_eq!(&*decoded, &*decoded_ref, "decoded mismatch for {fname}");
    }
}
