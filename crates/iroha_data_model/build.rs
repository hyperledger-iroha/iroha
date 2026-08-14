//! Build script that copies precomputed constants.
//!
//! Note: tests define their own helper module for parsing and collecting
//! `#[model]` items and do not depend on this build script anymore. This keeps
//! build.rs minimal to avoid long compile times or unexpected coupling.
use std::{env, fs, path::PathBuf};
const BUILD_CONSTS: &str = include_str!("build_consts.rs");
fn main() {
    println!("cargo:rerun-if-changed=build_consts.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("build_consts.rs"), BUILD_CONSTS).expect("failed to write build consts");
}
// Intentionally no helpers here; keep build script minimal.
