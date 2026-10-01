//! Build script of `iroha_core`: the mutation-testing switch of spec §13.4.
//!
//! Every Core mutation of §13.4 also requires `cfg(test)` at its call site. This
//! script sets that cfg **only** when the crate feature `mutation-testing` is enabled
//! (`CARGO_FEATURE_MUTATION_TESTING`) and `SUMERAGI_CORE_MUTATION=<ID>` is set; it also checks that
//! the ID has a switch in `src/`, so a typo cannot silently build the unmutated crate.
//!
//! Production builds cannot be mutated: without the feature `SUMERAGI_CORE_MUTATION` is ignored
//! (with a warning), and a `sumeragi_core_mutation` cfg smuggled in through `RUSTFLAGS` stops the
//! build. The library rejects the feature in every non-test build, even without a selected
//! mutation. The mutation gate is `scripts/sumeragi_mutation_gate.py --core`.

use std::{env, fs, path::Path};

const CFG: &str = "sumeragi_core_mutation";
const ENV: &str = "SUMERAGI_CORE_MUTATION";

fn main() {
    println!("cargo:rustc-check-cfg=cfg({CFG}, values(any()))");
    println!("cargo:rerun-if-env-changed={ENV}");
    println!("cargo:rerun-if-changed=build.rs");
    let requested = env::var(ENV).ok().filter(|id| !id.is_empty());
    if env::var_os("CARGO_FEATURE_MUTATION_TESTING").is_none() {
        // A production (or ordinary test) build: never mutated.
        let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
        assert!(
            !rustflags.contains(CFG),
            "`{CFG}` is set through RUSTFLAGS without the `mutation-testing` feature; \
             mutations exist only in mutation-testing builds (spec §13.4)"
        );
        if let Some(id) = requested {
            println!(
                "cargo:warning={ENV}={id} ignored: the `mutation-testing` feature is off, so \
                 this build is not mutated"
            );
        }
        return;
    }
    let Some(id) = requested else {
        return;
    };
    assert!(
        id.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "{ENV}={id:?}: a mutation id has only ASCII letters, digits, '-' and '_'"
    );
    println!("cargo:rerun-if-changed=src");
    let needle = format!("{CFG} = \"{id}\"");
    assert!(
        mentions(Path::new("src"), &needle),
        "{ENV}={id}: no `{needle}` switch in src/ (unknown mutation id)"
    );
    println!("cargo:rustc-cfg={CFG}=\"{id}\"");
    println!("cargo:warning=iroha_core is built with mutation {id} (spec §13.4)");
}

/// Whether any `.rs` file under `dir` contains `needle`.
fn mentions(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            mentions(&path, needle)
        } else {
            path.extension().is_some_and(|ext| ext == "rs")
                && fs::read_to_string(&path).is_ok_and(|text| text.contains(needle))
        }
    })
}
