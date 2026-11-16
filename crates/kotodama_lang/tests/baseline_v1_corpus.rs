//! The baseline engine target reproduces the anchor compiler's phase-1 corpus.
//!
//! `crates/ivm/tests/fixtures/baseline_v1` keeps every Kotodama source the anchor `77600a4840`
//! compiled with its own compiler: the artifacts it produced (byte for byte, including the
//! `force_zk` variants) and the diagnostics of the sources it refused. Compiling each source with
//! `CompilerOptions { engine: IvmEngine::Baseline, .. }` must give exactly the same outcome.

use std::path::{Path, PathBuf};

use kotodama_lang::{
    IvmEngine, IvmProfileId,
    compiler::{Compiler, CompilerOptions},
};
use norito::json::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ivm/tests/fixtures/baseline_v1")
}

fn text_field<'a>(entry: &'a Value, name: &str) -> Option<&'a str> {
    entry.get(name).and_then(Value::as_str)
}

fn compile(source: &str, force_zk: bool) -> Result<Vec<u8>, String> {
    Compiler::new_with_options(CompilerOptions {
        engine: IvmEngine::Baseline,
        force_zk,
        ..CompilerOptions::default()
    })
    .compile_source(source)
}

/// First differing byte and the two lengths, for a readable mismatch report.
fn first_difference(expected: &[u8], actual: &[u8]) -> String {
    let at = expected
        .iter()
        .zip(actual)
        .position(|(left, right)| left != right)
        .unwrap_or(expected.len().min(actual.len()));
    format!(
        "first difference at byte {at} (expected {} bytes, got {})",
        expected.len(),
        actual.len()
    )
}

#[test]
fn baseline_target_reproduces_every_anchor_compilation() {
    let root = fixtures();
    let corpus: Value = norito::json::from_str(
        &std::fs::read_to_string(root.join("oracle/corpus.json")).expect("read corpus index"),
    )
    .expect("corpus index JSON");
    let sources = corpus
        .get("kotodama_sources")
        .and_then(Value::as_array)
        .expect("Kotodama source index");
    let (mut artifacts, mut refusals, mut mismatches) = (0usize, 0usize, Vec::new());
    for entry in sources {
        let id = text_field(entry, "id").expect("source id");
        let source =
            std::fs::read_to_string(root.join(text_field(entry, "source").expect("source path")))
                .expect("read corpus source");
        let mut expected = Vec::new();
        if let Some(path) = text_field(entry, "artifact") {
            expected.push((false, Ok(path)));
        }
        if let Some(path) = text_field(entry, "zk_artifact") {
            expected.push((true, Ok(path)));
        }
        if let Some(diagnostic) = text_field(entry, "compile_error") {
            expected.push((false, Err(diagnostic)));
        }
        assert!(!expected.is_empty(), "{id}: no recorded outcome");
        for (force_zk, outcome) in expected {
            let label = if force_zk {
                format!("zk/{id}")
            } else {
                id.to_owned()
            };
            match (outcome, compile(&source, force_zk)) {
                (Ok(path), Ok(actual)) => {
                    artifacts += 1;
                    let anchor = std::fs::read(root.join(path)).expect("read anchor artifact");
                    if anchor != actual {
                        mismatches.push(format!(
                            "{label}: artifact differs, {}",
                            first_difference(&anchor, &actual)
                        ));
                    }
                }
                (Err(anchor), Err(actual)) => {
                    refusals += 1;
                    if anchor != actual {
                        mismatches.push(format!(
                            "{label}: diagnostic differs\n  anchor: {anchor:?}\n  actual: {actual:?}"
                        ));
                    }
                }
                (Ok(_), Err(actual)) => {
                    artifacts += 1;
                    mismatches.push(format!(
                        "{label}: the anchor compiled it, now refused: {actual}"
                    ));
                }
                (Err(anchor), Ok(_)) => {
                    refusals += 1;
                    mismatches.push(format!(
                        "{label}: the anchor refused it ({anchor:?}), now compiled"
                    ));
                }
            }
        }
    }
    assert_eq!(sources.len(), 166, "corpus sources");
    assert_eq!((artifacts, refusals), (125, 45), "corpus outcomes");
    assert!(
        mismatches.is_empty(),
        "{} of 170 outcomes differ from the anchor compiler:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn the_default_target_is_the_baseline_and_c10_is_explicit() {
    // The default target is the engine the interface policy runs: a default build and an explicit
    // baseline build are the same bytes, and the explicit C10 build of the same source differs.
    let source = "seiyaku Target { view fn read() authorize(anyone) -> int { 1 } }";
    let default = Compiler::new_with_options(CompilerOptions::default())
        .compile_source(source)
        .expect("default build");
    let c10 = Compiler::new_with_options(CompilerOptions::c10())
        .compile_source(source)
        .expect("C10 build");
    let baseline = compile(source, false).expect("baseline build");
    assert_eq!(default, baseline);
    assert_ne!(default, c10);
    let profile = |bytes: &[u8], target| {
        kotodama_lang::metadata::ProgramMetadata::parse_header_for_profile(bytes, target)
            .map(|header| header.profile())
            .ok()
    };
    let interface = IvmProfileId::of_policy(kotodama_lang::SyscallPolicy::AbiV1);
    assert_eq!(interface, IvmProfileId::BASELINE_ABI_V1);
    assert_eq!(profile(&default, interface), Some(interface));
    assert_eq!(
        profile(&c10, IvmProfileId::C10_ABI_V1),
        Some(IvmProfileId::C10_ABI_V1)
    );
    assert_eq!(profile(&c10, interface), None, "no layout fallback");
}

#[test]
fn the_baseline_target_refuses_a_profile_it_does_not_implement() {
    let error = Compiler::new_with_options(CompilerOptions {
        engine: IvmEngine::Baseline,
        host_policy: kotodama_lang::SyscallPolicy::AbiV1RuntimeLifecycle,
        ..CompilerOptions::c10()
    })
    .compile_source("seiyaku Target { view fn read() authorize(anyone) -> int { 1 } }")
    .expect_err("the baseline engine has only the ABI V1 profile");
    assert!(error.contains("K4001"), "{error}");
    assert!(error.contains("only the ABI V1 host profile"), "{error}");
}
