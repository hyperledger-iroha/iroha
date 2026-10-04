//! Build script of the oracle crate.
//!
//! 1. Declares the `iroha_plonk_oracle` cfg for `check-cfg`. Oracle mode of
//!    `iroha_plonk` (`create_proof_oracle`, `verify_full_oracle`, fixed prover
//!    seeds) exists only when `--cfg iroha_plonk_oracle` is passed through
//!    `RUSTFLAGS` into a separate target directory, a manual run today (see
//!    `README.md`; TODO: an oracle CI job); it is never a Cargo feature (spec
//!    section 6.4). The tests of this crate that need it are compiled only
//!    under that cfg.
//! 2. Copies `vendor/halo2-axiom/tests/golden_proof_bytes.rs` into `OUT_DIR`
//!    with its inner doc comments (`//!`) turned into plain comments, which is
//!    the only change. `include!` cannot expand a file that starts with inner
//!    doc comments, and the `vendored_goldens` test target includes the
//!    vendored goldens with `include!` so that its native parity modules can
//!    use the vendored circuits, seeds and digests directly. Nothing under
//!    `vendor/` is edited.
//!
//! The script reads only the vendored file and `OUT_DIR`, which Cargo sets for
//! every build script; it changes no runtime behaviour.

use std::{env, fs, path::PathBuf};

/// The vendored golden file, relative to this crate's manifest directory.
const VENDORED_GOLDENS: &str = "../../vendor/halo2-axiom/tests/golden_proof_bytes.rs";
/// The file written into `OUT_DIR`.
const GENERATED_GOLDENS: &str = "golden_proof_bytes.rs";

/// The vendored text with every inner line doc comment turned into a plain
/// comment, so that `include!` accepts it. Every other byte is unchanged.
fn includable(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let indent = line.len() - line.trim_start().len();
        let (head, rest) = line.split_at(indent);
        match rest.strip_prefix("//!") {
            Some(comment) => {
                out.push_str(head);
                out.push_str("//");
                out.push_str(comment);
            }
            None => out.push_str(line),
        }
    }
    out
}

fn main() {
    println!("cargo::rustc-check-cfg=cfg(iroha_plonk_oracle)");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={VENDORED_GOLDENS}");
    let source = fs::read_to_string(VENDORED_GOLDENS)
        .unwrap_or_else(|error| panic!("read {VENDORED_GOLDENS}: {error}"));
    let out_dir = env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR for build scripts");
    let target = PathBuf::from(out_dir).join(GENERATED_GOLDENS);
    let text = includable(&source);
    // Rewrite only on change so dependent targets are not rebuilt needlessly.
    if fs::read_to_string(&target).ok().as_deref() != Some(text.as_str()) {
        fs::write(&target, text)
            .unwrap_or_else(|error| panic!("write {}: {error}", target.display()));
    }
}
