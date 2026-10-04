//! The deviation registry, linked to spec section 14 (every build).
//!
//! `specs/plonk_ipa_v1.md` section 14 lists every verdict difference from the
//! vendored verifier as a `DEV-xx` row. This module ties each row to the
//! tests that pin it, so a row cannot be added, removed or changed without a
//! failure here:
//!
//! - [`DEVIATIONS`]: the stricter rejections that the oracle's tamper corpora
//!   observe (`verdict_parity`, oracle builds): the vendored verifier
//!   accepts, the native verifier rejects with the registered typed reason.
//!   Each entry's `id` is its spec row.
//! - [`NATIVE_TESTS`]: for every row, a named native test (in `iroha_plonk`
//!   or in this crate) that asserts the row's typed rejection or production
//!   behaviour. The named test must exist, carry `#[test]`, and mention the
//!   row id in its doc comment or body.
//!
//! [`registry_matches_spec_section_14`] checks that the spec rows, the
//! oracle entries and the native tests agree in both directions.

use iroha_plonk::verifier::VerifyError;

/// Which deviation class an input belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Verdicts must be equal.
    Ordinary,
    /// Bytes follow the last proof message.
    TrailingBytes,
    /// An instance column carries extra trailing zero values.
    InstancePadding,
}

/// A registered stricter rejection observed by the tamper corpora.
#[derive(Clone, Copy, Debug)]
pub struct Deviation {
    /// The spec section 14 row (`DEV-xx`).
    pub id: &'static str,
    /// A short label.
    pub label: &'static str,
    /// The inputs it covers.
    pub class: Class,
    /// Why the native verifier is stricter.
    pub reason: &'static str,
    /// The typed native rejection required for it.
    pub native_rejection: fn(&VerifyError) -> bool,
}

/// The stricter rejections the tamper corpora observe: the vendored verifier
/// accepts, the native verifier rejects with the stated typed reason.
pub const DEVIATIONS: &[Deviation] = &[
    Deviation {
        id: "DEV-05",
        label: "trailing bytes",
        class: Class::TrailingBytes,
        reason: "the vendored Blake2bRead stops after the last message it needs and ignores \
                 any trailing input, so a proof with appended bytes still verifies; PIPA-v1 \
                 requires the exact proof length the descriptor implies (spec section 8, \
                 step 2)",
        native_rejection: |error| matches!(error, VerifyError::ProofLength { .. }),
    },
    Deviation {
        id: "DEV-04",
        label: "instance length",
        class: Class::InstancePadding,
        reason: "the vendored verifier only bounds a committed instance column by the usable \
                 rows, and appended zeros leave its commitment unchanged, so the padded \
                 statement verifies; PIPA-v1 fixes the exact length of every instance column \
                 in the descriptor (S4)",
        native_rejection: |error| matches!(error, VerifyError::InstanceLength { .. }),
    },
];

/// A native test that pins a spec section 14 row.
#[derive(Clone, Copy, Debug)]
pub struct NativeTest {
    /// The spec row (`DEV-xx`).
    pub id: &'static str,
    /// The source file, relative to the repository root.
    pub file: &'static str,
    /// The test function.
    pub test: &'static str,
}

/// One named native test per spec section 14 row.
pub const NATIVE_TESTS: &[NativeTest] = &[
    NativeTest {
        id: "DEV-01",
        file: "crates/iroha_plonk/src/keys/tests.rs",
        test: "the_descriptor_binds_transcript_repr_but_not_vk_bytes",
    },
    NativeTest {
        id: "DEV-02",
        file: "crates/iroha_plonk/src/transcript/mod.rs",
        test: "prelude_frames_the_instance_shape",
    },
    NativeTest {
        id: "DEV-03",
        file: "crates/iroha_plonk/src/transcript/kagemusha_poseidon.rs",
        test: "injective_encoding_recovers_the_point",
    },
    NativeTest {
        id: "DEV-04",
        file: "crates/iroha_plonk/src/verifier/tests.rs",
        test: "instance_shapes_are_exact_in_both_modes",
    },
    NativeTest {
        id: "DEV-05",
        file: "crates/iroha_plonk/src/verifier/tests.rs",
        test: "the_proof_length_is_exact",
    },
    NativeTest {
        id: "DEV-06",
        file: "crates/iroha_plonk/src/keys/tests.rs",
        test: "the_reader_rejects_every_malformed_field",
    },
    NativeTest {
        id: "DEV-07",
        file: "crates/iroha_plonk/src/verifier/tests.rs",
        test: "degenerate_challenges_are_typed_rejections",
    },
    NativeTest {
        id: "DEV-08",
        file: "crates/iroha_plonk/src/pcs/ipa/mod.rs",
        test: "pinned_params_accept_only_pinned_bytes",
    },
    NativeTest {
        id: "DEV-09",
        file: "crates/iroha_plonk/src/pcs/ipa/accumulator.rs",
        test: "cancelling_errors_are_rejected_under_derived_weights",
    },
    NativeTest {
        id: "DEV-10",
        file: "crates/iroha_plonk/src/cs/descriptor.rs",
        test: "mutations_fail_the_named_rule",
    },
    NativeTest {
        id: "DEV-11",
        file: "crates/iroha_plonk_oracle/src/export/tests.rs",
        test: "multi_phase_circuits_and_challenges_are_rejected",
    },
];

/// The registry inputs: the spec and every file [`NATIVE_TESTS`] names,
/// included at compile time so an edit to any of them rebuilds this test.
const SOURCES: &[(&str, &str)] = &[
    (
        "specs/plonk_ipa_v1.md",
        include_str!("../../../../specs/plonk_ipa_v1.md"),
    ),
    (
        "crates/iroha_plonk/src/keys/tests.rs",
        include_str!("../../../iroha_plonk/src/keys/tests.rs"),
    ),
    (
        "crates/iroha_plonk/src/transcript/mod.rs",
        include_str!("../../../iroha_plonk/src/transcript/mod.rs"),
    ),
    (
        "crates/iroha_plonk/src/transcript/kagemusha_poseidon.rs",
        include_str!("../../../iroha_plonk/src/transcript/kagemusha_poseidon.rs"),
    ),
    (
        "crates/iroha_plonk/src/verifier/tests.rs",
        include_str!("../../../iroha_plonk/src/verifier/tests.rs"),
    ),
    (
        "crates/iroha_plonk/src/pcs/ipa/mod.rs",
        include_str!("../../../iroha_plonk/src/pcs/ipa/mod.rs"),
    ),
    (
        "crates/iroha_plonk/src/pcs/ipa/accumulator.rs",
        include_str!("../../../iroha_plonk/src/pcs/ipa/accumulator.rs"),
    ),
    (
        "crates/iroha_plonk/src/cs/descriptor.rs",
        include_str!("../../../iroha_plonk/src/cs/descriptor.rs"),
    ),
    (
        "crates/iroha_plonk_oracle/src/export/tests.rs",
        include_str!("../../src/export/tests.rs"),
    ),
];

/// The included text of `file`.
fn source(file: &str) -> &'static str {
    SOURCES.iter().find(|(path, _)| *path == file).map_or_else(
        || panic!("{file} is not included in SOURCES"),
        |(_, text)| *text,
    )
}

/// One row of the spec section 14 table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecRow {
    /// `DEV-xx`.
    pub id: String,
    /// The modes column (`both` or `production`).
    pub modes: String,
}

/// The `| DEV-xx | ... | modes |` rows of spec section 14.
pub fn spec_rows() -> Vec<SpecRow> {
    let spec = source("specs/plonk_ipa_v1.md");
    let section = spec
        .split("\n## ")
        .find(|section| section.starts_with("14. "))
        .expect("spec section 14");
    section
        .lines()
        .filter(|line| line.starts_with("| DEV-"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            SpecRow {
                id: cells.first().copied().unwrap_or_default().to_owned(),
                modes: cells.last().copied().unwrap_or_default().to_owned(),
            }
        })
        .collect()
}

/// The text of test function `name` in `text`: its leading doc comments
/// and attributes, its signature and its body. `None` when absent.
pub fn test_text<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let signature = format!("fn {name}(");
    let position = text.find(&signature)?;
    let line_start = text[..position].rfind('\n').map_or(0, |index| index + 1);
    let indent = &text[line_start..position];
    // Walk back over the item's attributes and doc comments.
    let mut start = line_start;
    while start > 0 {
        let previous_start = text[..start - 1].rfind('\n').map_or(0, |index| index + 1);
        let previous = text[previous_start..start - 1].trim_start();
        if previous.starts_with("///") || previous.starts_with("#[") || previous.starts_with("//") {
            start = previous_start;
        } else {
            break;
        }
    }
    let close = format!("\n{indent}}}\n");
    let end = text[position..]
        .find(&close)
        .map(|offset| position + offset + close.len())?;
    Some(&text[start..end])
}

/// Spec section 14, the oracle deviations and the native tests agree: every
/// row has a named native test that exists, is a `#[test]` and mentions the
/// row; every row whose modes are `both` and that a verdict corpus can
/// observe has its oracle entry; every oracle entry names a `both` row.
#[test]
fn registry_matches_spec_section_14() {
    let rows = spec_rows();
    assert_eq!(rows.len(), 11, "spec section 14 rows: {rows:?}");
    for row in &rows {
        assert!(
            ["both", "production"].contains(&row.modes.as_str()),
            "{}: modes {:?}",
            row.id,
            row.modes
        );
        let tests: Vec<_> = NATIVE_TESTS
            .iter()
            .filter(|test| test.id == row.id)
            .collect();
        assert!(!tests.is_empty(), "{}: no named native test", row.id);
        for test in tests {
            let text = test_text(source(test.file), test.test).unwrap_or_else(|| {
                panic!("{}: {}::{} does not exist", row.id, test.file, test.test)
            });
            assert!(
                text.contains("#[test]"),
                "{}: {} is not a test",
                row.id,
                test.test
            );
            assert!(
                text.contains(row.id.as_str()),
                "{}: {} does not mention the row",
                row.id,
                test.test
            );
        }
    }
    for deviation in DEVIATIONS {
        let row = rows
            .iter()
            .find(|row| row.id == deviation.id)
            .unwrap_or_else(|| panic!("{} is not in spec section 14", deviation.id));
        assert_eq!(row.modes, "both", "{}", deviation.id);
        assert_ne!(deviation.class, Class::Ordinary, "{}", deviation.id);
    }
    for test in NATIVE_TESTS {
        assert!(
            rows.iter().any(|row| row.id == test.id),
            "{} names no spec row",
            test.id
        );
    }
    let mut ids: Vec<_> = DEVIATIONS.iter().map(|d| d.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), DEVIATIONS.len(), "distinct oracle entries");
}

#[test]
fn deviations_are_distinct_and_typed() {
    assert!(DEVIATIONS.iter().all(|d| d.class != Class::Ordinary));
    assert!(
        DEVIATIONS
            .iter()
            .all(|d| !d.label.is_empty() && !d.reason.is_empty())
    );
    assert!((DEVIATIONS[0].native_rejection)(
        &VerifyError::ProofLength {
            expected: 1,
            actual: 2
        }
    ));
    assert!(!(DEVIATIONS[0].native_rejection)(
        &VerifyError::DegenerateChallenge
    ));
    assert!((DEVIATIONS[1].native_rejection)(
        &VerifyError::InstanceLength {
            column: 0,
            expected: 1,
            found: 2
        }
    ));
}

#[test]
fn test_text_finds_items_with_their_docs() {
    let text =
        "use x;\n\n/// DEV-99 doc\n#[test]\nfn sample() {\n    body();\n}\n\nfn other() {}\n";
    let found = test_text(text, "sample").expect("found");
    assert!(found.starts_with("/// DEV-99 doc\n#[test]\nfn sample() {"));
    assert!(found.ends_with("body();\n}\n"));
    let nested = "mod tests {\n    #[test]\n    fn inner() {\n        x();\n    }\n}\n";
    assert_eq!(
        test_text(nested, "inner"),
        Some("    #[test]\n    fn inner() {\n        x();\n    }\n")
    );
    assert_eq!(test_text(text, "missing"), None);
}
