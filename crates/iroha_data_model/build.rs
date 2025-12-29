//! Build script that copies precomputed constants.
//!
//! Note: tests define their own helper module for parsing and collecting
//! `#[model]` items and do not depend on this build script anymore. This keeps
//! build.rs minimal to avoid long compile times or unexpected coupling.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build_consts.rs");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::copy(
        PathBuf::from(manifest_dir).join("build_consts.rs"),
        out_dir.join("build_consts.rs"),
    )
    .expect("failed to copy build consts");
}

// Intentionally no helpers here; keep build script minimal.
