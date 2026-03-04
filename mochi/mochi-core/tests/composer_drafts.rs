//! Fixture-driven tests for Mochi composer draft JSON payloads.

use mochi_core::{drafts_from_json_str, drafts_to_pretty_json};

const IMPLICIT_RECEIVE_FIXTURE: &str =
    include_str!("fixtures/composer_drafts/implicit_receive_transfer.json");
const ADMISSION_POLICY_FIXTURE: &str =
    include_str!("fixtures/composer_drafts/account_admission_policy.json");
const MULTISIG_PROPOSE_FIXTURE: &str =
    include_str!("fixtures/composer_drafts/multisig_propose.json");
const SPACE_DIRECTORY_FIXTURE: &str =
    include_str!("fixtures/composer_drafts/space_directory_manifest.json");
const PIN_MANIFEST_FIXTURE: &str = include_str!("fixtures/composer_drafts/pin_manifest.json");

#[test]
fn composer_draft_fixtures_roundtrip() {
    let fixtures = [
        ("implicit_receive", IMPLICIT_RECEIVE_FIXTURE),
        ("account_admission_policy", ADMISSION_POLICY_FIXTURE),
        ("multisig_propose", MULTISIG_PROPOSE_FIXTURE),
        ("space_directory_manifest", SPACE_DIRECTORY_FIXTURE),
        ("pin_manifest", PIN_MANIFEST_FIXTURE),
    ];

    for (name, fixture) in fixtures {
        let drafts =
            drafts_from_json_str(fixture).unwrap_or_else(|err| panic!("{name} parse: {err}"));
        assert!(
            !drafts.is_empty(),
            "{name} fixture should yield at least one draft"
        );

        let json =
            drafts_to_pretty_json(&drafts).unwrap_or_else(|err| panic!("{name} serialize: {err}"));
        let reparsed =
            drafts_from_json_str(&json).unwrap_or_else(|err| panic!("{name} reparse: {err}"));
        assert_eq!(
            drafts.len(),
            reparsed.len(),
            "{name} should preserve draft count on roundtrip"
        );
        for (expected, actual) in drafts.iter().zip(reparsed.iter()) {
            assert_eq!(
                expected.summary(),
                actual.summary(),
                "{name} summary roundtrip mismatch"
            );
        }
    }
}
