#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Decode IVM header fields from a `.to` artifact (ignored by default).
//!
//! This test uses the current header layout from `crates/ivm/src/metadata.rs`:
//! magic (4) + `version_major` (1) + `version_minor` (1) + mode (1) + `vector_len` (1)
//! + `max_cycles` (u64 LE) + `abi_version` (1) = 17 bytes total.
//!
//! Run manually after building an artifact (e.g., via `make examples-run`):
//!   cargo test -p `integration_tests` --test `ivm_header_decode` -- --ignored --nocapture

use std::{fs::File, io::Read, path::PathBuf};

#[derive(Debug)]
struct Header {
    version_major: u8,
    version_minor: u8,
    mode: u8,
    vector_length: u8,
    max_cycles: u64,
    abi_version: u8,
}

fn read_header(path: &PathBuf) -> std::io::Result<Header> {
    let mut f = File::open(path)?;
    let mut buf = [0u8; 17];
    f.read_exact(&mut buf)?;
    assert_eq!(&buf[0..4], b"IVM\0", "bad magic");
    let version_major = buf[4];
    let version_minor = buf[5];
    let mode = buf[6];
    let vector_length = buf[7];
    let mut mc = [0u8; 8];
    mc.copy_from_slice(&buf[8..16]);
    let max_cycles = u64::from_le_bytes(mc);
    let abi_version = buf[16];
    Ok(Header {
        version_major,
        version_minor,
        mode,
        vector_length,
        max_cycles,
        abi_version,
    })
}

#[test]
fn decode_hello_header() {
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
    let h = read_header(&path).expect("failed to read header");
    // Basic sanity checks
    assert_eq!(h.version_major, 1, "unexpected version: {h:?}");
    // Mode bits are implementation-defined and fit in one byte by design.
    // vector_length 0 means unset; otherwise, any value is accepted here.
    // max_cycles may be 0 for non-ZK; otherwise, enforce a modest upper bound for the test.
    assert!(
        h.max_cycles <= (1u64 << 48),
        "unreasonably large max_cycles: {}",
        h.max_cycles
    );
    eprintln!(
        "IVM header: version {}.{}, mode=0x{:02X}, vl={}, max_cycles={}, abi_version={}",
        h.version_major, h.version_minor, h.mode, h.vector_length, h.max_cycles, h.abi_version
    );
}
