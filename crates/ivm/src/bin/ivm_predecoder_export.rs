//! Export predecoder golden vectors (JSON/bin) for cross-implementation reuse.
//!
//! Writes a mixed 16/32-bit instruction stream, its decoded op list, and a
//! small set of header-variant artifacts under:
//!   `crates/ivm/tests/fixtures/predecoder/mixed/`
//!
//! Files produced:
//! - `code.bin`                 — raw instruction bytes (no header)
//! - `decoded.json`             — decoded ops: [{ pc, len, inst, inst_hex }]
//! - `index.json`               — list of artifact files and their metadata
//! - `artifacts/*.to`           — header + code artifacts for selected variants
//!
//! Usage:
//!   cargo run -p ivm --bin ivm_predecoder_export
//!
//! Notes:
//! - The decoded op list is invariant across the header variants emitted here.
//! - JSON is produced via `norito::json` as per repo policy.

use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = ivm::predecoder_fixtures::default_predecoder_mixed_root();
    fs::create_dir_all(root.parent().unwrap())?;
    ivm::predecoder_fixtures::generate_predecoder_mixed_fixtures(&root)?;
    eprintln!("wrote fixtures to {}", root.display());
    Ok(())
}
