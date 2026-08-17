#!/usr/bin/env python3
"""Protect the typed lane-reset cleanup matrices in state tests."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/state/tests.rs"
AUTOSCALE_GUARD_PATH = REPO_ROOT / "scripts/tests/state_autoscale_case_matrix_source_test.py"
AUTOSCALE_GUARD_SHA256 = "d59bcdd807640f5520dc881d48e5bfd3a4a4b6b7d977c038de6934173b533639"
MAX_SOURCE_LINES = 53_996

REGIONS = {
    "da_replay": (
        """#[derive(Clone, Copy, PartialEq, Eq)]
enum LaneLifecycleDaReplayResetCase""",
        """#[test]
fn non_nexus_lane_artifact_snapshots_track_only_implicit_default_route""",
        "0f9b5ccff8a3ea4fa0c4acc8011ca90e8ab0443058806e05e476cb55e7c8aab2",
    ),
    "relay_contract": (
        """#[derive(Clone, Copy, PartialEq, Eq)]
enum LaneRelayContractResetPath""",
        """#[derive(Clone, Copy, PartialEq, Eq)]
enum LaneCursorResetPath""",
        "47dad9fca3e459232be0bf1f672ad3f3c8b5b111b69e35aa1981dbd561d6701d",
    ),
    "cursor_journal": (
        """#[derive(Clone, Copy, PartialEq, Eq)]
enum LaneCursorResetPath""",
        """#[test]
fn apply_lane_lifecycle_rejects_when_nexus_disabled""",
        "9fc397b951e49e11dbb26fd8e058efddc51e78573fbe3e5ba6692a809a7d1b58",
    ),
}

TEST_CASES = {
    "apply_lane_lifecycle_recreated_lane_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "RecreatedLane",
    ),
    "lane_lifecycle_same_shard_dataspace_rebind_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "DataspaceRebind",
    ),
    "lane_lifecycle_same_lane_manifest_policy_change_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "ManifestPolicy",
    ),
    "lane_lifecycle_same_lane_shard_mapping_change_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "ShardMapping",
    ),
    "lane_lifecycle_same_lane_storage_profile_change_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "StorageProfile",
    ),
    "lane_lifecycle_same_lane_visibility_change_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "Visibility",
    ),
    "lane_lifecycle_same_lane_confidential_policy_change_hides_previous_da_indexes_after_kura_replay": (
        "da_replay",
        "ConfidentialPolicy",
    ),
    "apply_lane_lifecycle_recreated_lane_prunes_verified_relay_contract_state": (
        "relay_contract",
        "Lifecycle",
    ),
    "set_nexus_recreated_lane_prunes_verified_relay_contract_state": (
        "relay_contract",
        "SetNexus",
    ),
    "apply_lane_lifecycle_recreated_lane_persists_da_cursor_reset": (
        "cursor_journal",
        "RetireThenRecreate",
    ),
    "apply_lane_lifecycle_same_plan_recreated_lane_resets_da_cursors": (
        "cursor_journal",
        "SamePlanRecreate",
    ),
    "set_nexus_same_shard_dataspace_rebind_persists_da_cursor_reset": (
        "cursor_journal",
        "DataspaceRebind",
    ),
}

REQUIRED_TOKENS = {
    "da_replay": (
        "macro_rules! lane_reset_case_tests",
        "#[test]\n            fn $name()",
        "$runner($case_type::$case);",
        "run_lane_lifecycle_da_replay_reset_case, LaneLifecycleDaReplayResetCase;",
        '"da_manifest_policy".to_string()',
        'if replacement { "audit" } else { "strict" }',
        '"da_shard_id".to_string()',
        "if replacement { 7 } else { 5 }",
        "LaneStorageProfile::FullReplica",
        "LaneStorageProfile::SplitReplica",
        "LaneVisibility::Public",
        "LaneVisibility::Restricted",
        '"confidential_key_version".to_string()',
        "if replacement { 8 } else { 7 }",
        '"auditor,operator"',
        '"same-shard-rebind-da-kura"',
        '"same-lane-manifest-policy-reset-da-kura"',
        '"same-lane-shard-reset-da-kura"',
        '"same-lane-storage-profile-reset-da-kura"',
        '"same-lane-visibility-reset-da-kura"',
        '"same-lane-confidential-policy-reset-da-kura"',
        '| LaneLifecycleDaReplayResetCase::ConfidentialPolicy => "policy",',
        "0xC8",
        "0xCE",
        "0xD0",
        "0xD1",
        "0xD2",
        "0xD6",
        "0xD4",
        "StorageTicketId::new([0xCB; 32])",
        "ManifestDigest::new([0xCC; 32])",
        "StorageTicketId::new([0xD1; 32])",
        "ManifestDigest::new([0xD2; 32])",
        '"old-incarnation-pin"',
        '"same-shard-old-dataspace-pin"',
        ".with_da_commitments(Some(DaCommitmentBundle::new",
        ".with_da_pin_intents(pin_bundle)",
        "canonical_reset_height_for_lane(lane_id)",
        "rewind_da_indexes_to_height",
        "get_committed_by_key(&DaCommitmentKey::from_record(&stale_record))",
        "bundle_at(old_block.header().height().get())",
        "assert_lane_lifecycle_da_replay_confidential_receipt_absent",
        '"old shard cursor must not survive rewind after remap"',
        '"restart must not replay old-shard records into the new shard cursor"',
        '"persisted reset watermark must suppress old-{subject} commitment after restart"',
    ),
    "relay_contract": (
        "run_lane_relay_contract_reset_case, LaneRelayContractResetPath;",
        "NexusFeeSettlementMode::LaneRelayBurn",
        "[0x44; 32]",
        "[0x84; 32]",
        '"_spoofed_cleanup_sibling"',
        '"_set_nexus_spoofed_cleanup"',
        '"aa"',
        '"bb"',
        "verified_lane_relay_state_key",
        "verified_lane_relay_contract_map_state_key",
        "encode_verified_lane_relay_record_contract_map_state_for_test",
        "encode_verified_lane_relay_record(&old_record)",
        '"noncanonical prefixed siblings"',
        '"spoofed contract-map siblings"',
        "state.lane_relay_snapshot().is_empty()",
        'b"recreated-lane-relay-parent"',
        "ensure_merge_carrier_parent_for_test(&state)",
        "seed_effect_authenticated_relay_for_merge_test",
        "snapshot.lane_incarnation == recreated_incarnation",
        '"retired verified relay contract state must not suppress fresh {fresh_prefix}recreated-lane relay"',
    ),
    "cursor_journal": (
        "run_lane_cursor_reset_case, LaneCursorResetPath;",
        '"same-plan-kura"',
        '"same-shard-rebind-kura"',
        "912_u32",
        "902_u32",
        "913_u32",
        "903_u32",
        "919_u32",
        "901_u32",
        "seed_stale_da_cursors_for_lane_recreation",
        "persist_da_shard_cursor_journal",
        '"stale cursor persisted before lane recreation"',
        '"test setup should persist stale cursor before same-plan recreation"',
        '"test setup should persist stale cursor before dataspace rebind"',
        '"retire lane before recreation"',
        '"same-plan lane recreation should be accepted"',
        '"same-shard lane dataspace rebind should apply"',
        "state.da_shard_cursor_index().get(reset_shard_id).is_none()",
        "state.da_receipt_cursors().highest(LaneEpoch::new(lane_id, 2))",
        "assert_recreated_lane_da_cursors_accept_fresh_sequence",
        "assert_public_lane_staking_status_absent",
        "assert_public_lane_staking_status_bonded",
        '"fresh recreated-lane cursor persisted"',
        "seed_committed_height_for_state_test(&restarted, 11)",
        '"restart should hydrate fresh recreated-lane cursor from journal"',
    ),
}

FORBIDDEN_RUNNER_TOKENS = (
    "Box<dyn Fn",
    "Box<dyn FnMut",
    "impl Fn",
    "callback:",
    "custom_case:",
    "escape_hatch",
)


class GuardError(AssertionError):
    """Raised when the protected lane-reset source contract changes."""


def _normalized_hash(source: str) -> str:
    return hashlib.sha256(re.sub(r"\s+", "", source).encode()).hexdigest()


def _region(source: str, label: str) -> str:
    start_marker, end_marker, _expected_hash = REGIONS[label]
    if source.count(start_marker) != 1 or source.count(end_marker) != 1:
        raise GuardError(f"{label}: region markers must occur exactly once")
    start = source.index(start_marker)
    end = source.index(end_marker, start)
    return source[start:end]


def _case_row(region: str, test_name: str) -> str:
    pattern = re.compile(
        rf"\b{re.escape(test_name)}\s*=>\s*(?P<case>[A-Za-z0-9_]+);"
    )
    matches = list(pattern.finditer(region))
    if len(matches) != 1:
        raise GuardError(f"{test_name}: expected one name-preserving typed row")
    return matches[0].group("case")


def validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("state/tests.rs exceeded the frozen lane-reset source budget")
    regions = {label: _region(source, label) for label in REGIONS}
    for test_name, (label, expected_case) in TEST_CASES.items():
        occurrences = len(re.findall(rf"\b{re.escape(test_name)}\b", source))
        if occurrences != 1:
            raise GuardError(f"{test_name}: expected one source occurrence, found {occurrences}")
        observed_case = _case_row(regions[label], test_name)
        if observed_case != expected_case:
            raise GuardError(f"{test_name}: case {observed_case} != {expected_case}")
    for label, tokens in REQUIRED_TOKENS.items():
        for token in tokens:
            if token not in regions[label]:
                raise GuardError(f"{label}: missing semantic token {token!r}")
    protected = "".join(regions.values())
    for token in FORBIDDEN_RUNNER_TOKENS:
        if token in protected:
            raise GuardError(f"lane-reset runners contain forbidden escape hatch {token!r}")
    for label, region in regions.items():
        expected_hash = REGIONS[label][2]
        observed_hash = _normalized_hash(region)
        if observed_hash != expected_hash:
            raise GuardError(f"{label}: semantic hash changed: {observed_hash}")


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


def _replace_in_region(source: str, label: str, old: str, new: str) -> str:
    region = _region(source, label)
    if region.count(old) != 1:
        raise AssertionError(f"{label}: mutation preimage must occur once: {old!r}")
    return source.replace(region, region.replace(old, new, 1), 1)


class StateLaneResetCaseMatrixSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def assert_rejected(self, mutated: str) -> None:
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_current_source_preserves_lane_reset_matrices(self) -> None:
        validate_source(self.source)

    def test_frozen_autoscale_guard_is_byte_exact(self) -> None:
        observed = hashlib.sha256(AUTOSCALE_GUARD_PATH.read_bytes()).hexdigest()
        self.assertEqual(observed, AUTOSCALE_GUARD_SHA256)

    def test_name_mutation_is_rejected(self) -> None:
        name = next(iter(TEST_CASES))
        self.assert_rejected(_replace_once(self.source, name, f"{name}_mutated"))

    def test_case_wiring_mutation_is_rejected(self) -> None:
        name = next(iter(TEST_CASES))
        old = f"{name} => RecreatedLane;"
        self.assert_rejected(_replace_once(self.source, old, old.replace("RecreatedLane", "Visibility")))

    def test_test_attribute_mutation_is_rejected(self) -> None:
        old = "#[test]\n            fn $name()"
        self.assert_rejected(_replace_once(self.source, old, old.replace("#[test]", "#[ignore]")))

    def test_policy_mutation_is_rejected(self) -> None:
        old = 'if replacement { "audit" } else { "strict" }'
        self.assert_rejected(_replace_once(self.source, old, old.replace("audit", "permissive")))

    def test_setup_message_subject_mutation_is_rejected(self) -> None:
        old = '| LaneLifecycleDaReplayResetCase::ConfidentialPolicy => "policy",'
        mutated = _replace_in_region(
            self.source,
            "da_replay",
            old,
            old.replace('"policy"', '"confidential-policy"'),
        )
        self.assert_rejected(mutated)

    def test_pin_adversary_mutation_is_rejected(self) -> None:
        old = "StorageTicketId::new([0xCB; 32])"
        self.assert_rejected(_replace_once(self.source, old, "StorageTicketId::new([0xCA; 32])"))

    def test_replay_identity_assertion_mutation_is_rejected(self) -> None:
        old = "get_committed_by_key(&DaCommitmentKey::from_record(&stale_record))"
        self.assert_rejected(_replace_once(self.source, old, "get_by_manifest(&stale_record.manifest_hash)"))

    def test_relay_root_mutation_is_rejected(self) -> None:
        old = "[0x84; 32]"
        mutated = _replace_in_region(
            self.source,
            "relay_contract",
            old,
            "[0x85; 32]",
        )
        self.assert_rejected(mutated)

    def test_relay_storage_polarity_mutation_is_rejected(self) -> None:
        old = '(&spoofed_map_key, true, false, "spoofed contract-map siblings")'
        mutated = _replace_in_region(
            self.source,
            "relay_contract",
            old,
            old.replace("true", "false"),
        )
        self.assert_rejected(mutated)

    def test_cursor_quantity_mutation_is_rejected(self) -> None:
        self.assert_rejected(_replace_once(self.source, "919_u32", "920_u32"))

    def test_cursor_postcondition_mutation_is_rejected(self) -> None:
        old = "state.da_shard_cursor_index().get(reset_shard_id).is_none()"
        self.assert_rejected(_replace_once(self.source, old, old.replace("is_none", "is_some")))

    def test_callback_escape_hatch_is_rejected(self) -> None:
        old = "fn run_lane_cursor_reset_case(path: LaneCursorResetPath)"
        mutated = _replace_once(
            self.source,
            old,
            "fn run_lane_cursor_reset_case(path: LaneCursorResetPath, callback: Box<dyn Fn()>)",
        )
        self.assert_rejected(mutated)

    def test_source_budget_growth_is_rejected(self) -> None:
        self.assert_rejected(self.source + "// synthetic growth\n" * 10)


if __name__ == "__main__":
    unittest.main()
