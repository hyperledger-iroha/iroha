#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Minimal header smoke test (ignored by default)
//!
//! If a `.to` artifact is present at `target/examples/hello.to` (built via the
//! examples or Makefile), this test checks the magic `IVM\0` at the start.
//! It is ignored by default to avoid CI failures when the artifact isn't built.
//!
//! Run manually with:
//!   cargo test -p `integration_tests` --test `ivm_header_smoke` -- --ignored --nocapture

use std::{
    fs::File,
    io::{Read, Result as IoResult},
    path::PathBuf,
};

fn read_prefix(path: &PathBuf, n: usize) -> IoResult<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; n];
    let read = f.read(&mut buf)?;
    buf.truncate(read);
    Ok(buf)
}

#[test]
fn magic_is_ivm() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let path = root.join("target/examples/hello.to");
    if !path.exists() {
        eprintln!(
            "Skipping: {} not found. Build with `make examples-run`.",
            path.display()
        );
        return;
    }
    // Basic magic check
    let prefix = read_prefix(&path, 4).expect("failed to read artifact");
    assert_eq!(&prefix[..], b"IVM\0", "bad magic: {prefix:?}");
    // Require a minimal size to avoid false positives from tiny files
    let meta = std::fs::metadata(&path).expect("stat failed");
    assert!(
        meta.len() >= 64,
        "artifact too small ({} bytes)",
        meta.len()
    );

    // Optional: peek at the next 20 bytes for debugging (no assertions)
    if let Ok(mut f) = File::open(&path) {
        use std::io::Read as _;
        let mut skip = [0u8; 4];
        let _ = f.read(&mut skip);
        let mut next = [0u8; 20];
        if let Ok(n) = f.read(&mut next) {
            // Print a short hex preview only when running with --nocapture
            let mut hex = String::with_capacity(n * 2);
            for b in &next[..n] {
                use core::fmt::Write as _;
                let _ = write!(hex, "{b:02x}");
            }
            eprintln!("IVM header peek (20 bytes): {hex}");
        }
    }
}
