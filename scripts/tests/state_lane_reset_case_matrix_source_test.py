#!/usr/bin/env python3
"""Protect lane-reset regressions across the state test include closure."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path

from state_source_bundle import read_rust_source_bundle


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/state/tests.rs"
AUTOSCALE_GUARD_PATH = REPO_ROOT / "scripts/tests/state_autoscale_case_matrix_source_test.py"
AUTOSCALE_GUARD_SHA256 = "7e04f0cd09ddd357b9de5bd8a30fcba47cca8ada13ff84b54fd3975ea8b0ea67"
MAX_SOURCE_LINES = 36_806

REGIONS = {
    "da_recreated_replay": (
        """state_test! { sync apply_lane_lifecycle_recreated_lane_hides_previous_da_indexes_after_kura_replay
""",
        """state_test! { sync durable_lane_diagnostics_reconstruct_after_kura_restart
""",
        "73e05d3874b75b19e82275dc558509e50ebd7b9b7801ceadc88b4552e240b816",
    ),
    "relay_contract": (
        """state_test! { sync apply_lane_lifecycle_recreated_lane_prunes_verified_relay_contract_state
""",
        """state_test! { sync apply_lane_lifecycle_recreated_lane_persists_da_cursor_reset
""",
        "caba17462b111defc48431025f6ee794f63f8c21067ad32102b33c6ff7af82c5",
    ),
    "cursor_journal": (
        """state_test! { sync apply_lane_lifecycle_recreated_lane_persists_da_cursor_reset
""",
        """#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SameLaneDaResetCase""",
        "f832f2550b43e3c8f7bb725106ef809a92473c9a0e0525464784d43b02207ac4",
    ),
    "da_policy_replay": (
        """#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SameLaneDaResetCase""",
        "fn install_lane_manifest_registry(",
        "76570a7df1314385e70b57d768ec9ddb871caa3d392ef6a80e5fce1150a72ab7",
    ),
}

EXPECTED_TESTS = {
    "apply_lane_lifecycle_recreated_lane_hides_previous_da_indexes_after_kura_replay": (
        "da_recreated_replay",
        "state_test",
    ),
    "apply_lane_lifecycle_recreated_lane_prunes_verified_relay_contract_state": (
        "relay_contract",
        "state_test",
    ),
    "set_nexus_recreated_lane_prunes_verified_relay_contract_state": (
        "relay_contract",
        "state_test",
    ),
    "apply_lane_lifecycle_recreated_lane_persists_da_cursor_reset": (
        "cursor_journal",
        "state_test",
    ),
    "apply_lane_lifecycle_same_plan_recreated_lane_resets_da_cursors": (
        "cursor_journal",
        "state_test",
    ),
    "set_nexus_same_shard_dataspace_rebind_persists_da_cursor_reset": (
        "cursor_journal",
        "state_test",
    ),
    "lane_lifecycle_same_shard_dataspace_rebind_hides_previous_da_indexes_after_kura_replay": (
        "da_policy_replay",
        "test",
    ),
    "lane_lifecycle_same_lane_manifest_policy_change_hides_previous_da_indexes_after_kura_replay": (
        "da_policy_replay",
        "test",
    ),
    "lane_lifecycle_same_lane_shard_mapping_change_hides_previous_da_indexes_after_kura_replay": (
        "da_policy_replay",
        "test",
    ),
    "lane_lifecycle_same_lane_storage_profile_change_hides_previous_da_indexes_after_kura_replay": (
        "da_policy_replay",
        "test",
    ),
    "lane_lifecycle_same_lane_visibility_change_hides_previous_da_indexes_after_kura_replay": (
        "da_policy_replay",
        "test",
    ),
    "lane_lifecycle_same_lane_confidential_policy_change_hides_previous_da_indexes_after_kura_replay": (
        "da_policy_replay",
        "test",
    ),
}

REQUIRED_TOKENS = {
    "da_recreated_replay": (
        "sample_da_commitment_record(recreated_lane_id, 2, 1, 0xC8)",
        "StorageTicketId::new([0xCB; 32])",
        "ManifestDigest::new([0xCC; 32])",
        '"old-incarnation-pin"',
        ".with_da_commitments(Some(",
        ".with_da_pin_intents(Some(",
        '"test setup should hydrate old-incarnation DA commitment before reset"',
        '"test setup should hydrate old-incarnation pin before reset"',
        "rewind_da_indexes_to_height(old_block.header().height().get())",
        "get_committed_by_key(&DaCommitmentKey::from_record(&stale_record))",
        "bundle_at(old_block.header().height().get())",
        '"old-incarnation commitment must not reserve fresh-lane identities after replay"',
        '"committed block bundle remains available as historical proof material"',
        '"old-incarnation pin intent must not rehydrate into the fresh lane"',
        '"restart should hydrate with lane reset watermark"',
        '"persisted reset watermark must suppress old-incarnation commitment after restart"',
        '"persisted reset watermark must suppress old-incarnation pin after restart"',
    ),
    "relay_contract": (
        "NexusFeeSettlementMode::LaneRelayBurn",
        "[0x44; 32]",
        "[0x84; 32]",
        "_spoofed_cleanup_sibling",
        "_set_nexus_spoofed_cleanup",
        '"aa".repeat(32)',
        '"bb".repeat(32)',
        "verified_lane_relay_state_key",
        "verified_lane_relay_contract_map_state_key",
        "encode_verified_lane_relay_record_contract_map_state_for_test",
        "encode_verified_lane_relay_record(&old_record)",
        '"retiring the lane must prune its verified relay contract-state record"',
        '"retiring the lane must prune its verified relay contract-map record"',
        '"retiring the lane must not prune noncanonical prefixed siblings"',
        '"retiring the lane must not prune spoofed contract-map siblings"',
        '"set_nexus lane reset must prune the verified relay contract-state record"',
        '"set_nexus lane reset must prune the verified relay contract-map record"',
        '"set_nexus lane reset must not prune noncanonical prefixed siblings"',
        '"set_nexus lane reset must not prune spoofed contract-map siblings"',
        "state.lane_relay_snapshot().is_empty()",
        "seed_effect_authenticated_relay_for_merge_test",
        "snapshot.lane_incarnation == recreated_lane_incarnation",
        'b"recreated-lane-relay-parent"',
        "ensure_merge_carrier_parent_for_test(&state)",
        '"retired verified relay contract state must not suppress fresh recreated-lane relay"',
        '"retired verified relay contract state must not suppress fresh set_nexus recreated-lane relay"',
    ),
    "cursor_journal": (
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
        "reset_journal.cursor_for_lane(recreated_lane_id).is_none()",
        "reset_journal.cursor_for_lane(rebound_lane_id).is_none()",
        ".get(reset_shard_id, rebound_lane_id)",
        ".get(reset_config.shard_id(recreated_lane_id), recreated_lane_id)",
        ".highest(LaneEpoch::new(recreated_lane_id, 2))",
        ".highest(LaneEpoch::new(rebound_lane_id, 2))",
        "assert_recreated_lane_da_cursors_accept_fresh_sequence",
        "assert_public_lane_staking_status_absent",
        "assert_public_lane_staking_status_bonded",
        '"fresh recreated-lane cursor persisted"',
        "seed_committed_height_for_state_test(&restarted, 11)",
        '"restart should hydrate fresh recreated-lane cursor from journal"',
        '"restarted state should restore fresh recreated-lane cursor"',
    ),
    "da_policy_replay": (
        "enum SameLaneDaResetCase",
        "Self::DataspaceRebind => 0xCE",
        "Self::ManifestPolicy => 0xD0",
        "Self::ShardMapping => 0xD1",
        "Self::StorageProfile => 0xD2",
        "Self::ConfidentialPolicy => 0xD4",
        "Self::Visibility => 0xD6",
        "manifest_policy: DaManifestPolicy::Audit",
        "shard_id: Some(ShardId::new(shard_id))",
        "(mapped(5), mapped(7))",
        "LaneStorageProfile::FullReplica",
        "LaneStorageProfile::SplitReplica",
        "LaneVisibility::Public",
        "LaneVisibility::Restricted",
        "confidential_compute: Some(ConfidentialComputePolicy::new(",
        "ConfidentialComputeMechanism::Encryption",
        "NonZeroU32::new(key_version)",
        'confidential(8, "auditor,operator")',
        "StorageTicketId::new([0xD1; 32])",
        "ManifestDigest::new([0xD2; 32])",
        '"same-shard-old-dataspace-pin"',
        ".with_da_commitments(Some(DaCommitmentBundle::new",
        ".with_da_pin_intents(Some(DaPinIntentBundle::new",
        "reset_journal.canonical_reset_height_for_lane(lane_id)",
        "reset_journal.cursor_for_lane(lane_id).is_none()",
        "rewind_da_indexes_to_height(old_block.header().height().get())",
        "get_committed_by_key(&DaCommitmentKey::from_record(&stale_record))",
        "bundle_at(old_block.header().height().get())",
        "cursors.get(5, lane_id).is_none()",
        "cursors.get(7, lane_id).is_none()",
        '"the persisted reset watermark must hide pre-reset commitments after restart"',
        "assert_same_lane_da_reset_hides_previous_indexes(SameLaneDaResetCase::DataspaceRebind);",
        "assert_same_lane_da_reset_hides_previous_indexes(SameLaneDaResetCase::ManifestPolicy);",
        "assert_same_lane_da_reset_hides_previous_indexes(SameLaneDaResetCase::ShardMapping);",
        "assert_same_lane_da_reset_hides_previous_indexes(SameLaneDaResetCase::StorageProfile);",
        "assert_same_lane_da_reset_hides_previous_indexes(SameLaneDaResetCase::Visibility);",
        "assert_same_lane_da_reset_hides_previous_indexes(SameLaneDaResetCase::ConfidentialPolicy);",
    ),
}

FORBIDDEN_PROTECTED_TOKENS = (
    "macro_rules! lane_reset_case_tests",
    "enum LaneLifecycleDaReplayResetCase",
    "enum LaneRelayContractResetPath",
    "enum LaneCursorResetPath",
    "run_lane_lifecycle_da_replay_reset_case",
    "run_lane_relay_contract_reset_case",
    "run_lane_cursor_reset_case",
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


def _test_declaration(test_name: str, declaration_kind: str) -> str:
    if declaration_kind == "state_test":
        return f"state_test! {{ sync {test_name}\n"
    if declaration_kind == "test":
        return f"#[test]\nfn {test_name}"
    raise AssertionError(f"unknown declaration kind: {declaration_kind}")


def validate_source(source: str, *, parent_source: str | None = None) -> None:
    if parent_source is None:
        parent_source = SOURCE_PATH.read_text(encoding="utf-8")
    if len(parent_source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("state/tests.rs exceeded the frozen lane-reset source budget")
    regions = {label: _region(source, label) for label in REGIONS}
    for test_name, (label, declaration_kind) in EXPECTED_TESTS.items():
        occurrences = len(re.findall(rf"\b{re.escape(test_name)}\b", source))
        if occurrences != 1:
            raise GuardError(f"{test_name}: expected one source occurrence, found {occurrences}")
        declaration = _test_declaration(test_name, declaration_kind)
        if regions[label].count(declaration) != 1:
            raise GuardError(f"{test_name}: expected one {declaration_kind} declaration")
    for label, tokens in REQUIRED_TOKENS.items():
        for token in tokens:
            if token not in regions[label]:
                raise GuardError(f"{label}: missing semantic token {token!r}")
    protected = "".join(regions.values())
    for token in FORBIDDEN_PROTECTED_TOKENS:
        if token in protected:
            raise GuardError(f"lane-reset tests contain forbidden indirection {token!r}")
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
        cls.parent_source = SOURCE_PATH.read_text(encoding="utf-8")
        cls.source = read_rust_source_bundle(SOURCE_PATH, root=REPO_ROOT)

    def assert_rejected(self, mutated: str) -> None:
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_current_source_preserves_explicit_lane_reset_regressions(self) -> None:
        validate_source(self.source)

    def test_frozen_autoscale_guard_is_byte_exact(self) -> None:
        observed = hashlib.sha256(AUTOSCALE_GUARD_PATH.read_bytes()).hexdigest()
        self.assertEqual(observed, AUTOSCALE_GUARD_SHA256)

    def test_name_mutation_is_rejected(self) -> None:
        name = next(iter(EXPECTED_TESTS))
        self.assert_rejected(_replace_once(self.source, name, f"{name}_mutated"))

    def test_explicit_case_wiring_mutation_is_rejected(self) -> None:
        old = (
            "assert_same_lane_da_reset_hides_previous_indexes("
            "SameLaneDaResetCase::DataspaceRebind);"
        )
        mutated = _replace_in_region(
            self.source,
            "da_policy_replay",
            old,
            old.replace("DataspaceRebind", "Visibility"),
        )
        self.assert_rejected(mutated)

    def test_test_attribute_mutation_is_rejected(self) -> None:
        old = (
            "#[test]\n"
            "fn lane_lifecycle_same_shard_dataspace_rebind_hides_previous_da_indexes_after_kura_replay"
        )
        mutated = _replace_in_region(
            self.source,
            "da_policy_replay",
            old,
            old.replace("#[test]", "#[ignore]"),
        )
        self.assert_rejected(mutated)

    def test_policy_mutation_is_rejected(self) -> None:
        old = "manifest_policy: DaManifestPolicy::Audit"
        mutated = _replace_in_region(
            self.source,
            "da_policy_replay",
            old,
            old.replace("DaManifestPolicy::Audit", "DaManifestPolicy::Strict"),
        )
        self.assert_rejected(mutated)

    def test_confidential_mechanism_mutation_is_rejected(self) -> None:
        old = "ConfidentialComputeMechanism::Encryption"
        mutated = _replace_in_region(
            self.source,
            "da_policy_replay",
            old,
            old.replace("Encryption", "SecretSharing"),
        )
        self.assert_rejected(mutated)

    def test_setup_subject_mutation_is_rejected(self) -> None:
        old = 'Self::ConfidentialPolicy => "confidential-policy change",'
        mutated = _replace_in_region(
            self.source,
            "da_policy_replay",
            old,
            old.replace("confidential-policy change", "policy change"),
        )
        self.assert_rejected(mutated)

    def test_pin_adversary_mutation_is_rejected(self) -> None:
        mutated = _replace_in_region(
            self.source,
            "da_recreated_replay",
            "StorageTicketId::new([0xCB; 32])",
            "StorageTicketId::new([0xCA; 32])",
        )
        self.assert_rejected(mutated)

    def test_replay_identity_assertion_mutation_is_rejected(self) -> None:
        old = "get_committed_by_key(&DaCommitmentKey::from_record(&stale_record))"
        mutated = _replace_in_region(
            self.source,
            "da_recreated_replay",
            old,
            "get_by_manifest(&stale_record.manifest_hash)",
        )
        self.assert_rejected(mutated)

    def test_relay_root_mutation_is_rejected(self) -> None:
        mutated = _replace_in_region(
            self.source,
            "relay_contract",
            "[0x84; 32]",
            "[0x85; 32]",
        )
        self.assert_rejected(mutated)

    def test_relay_storage_polarity_mutation_is_rejected(self) -> None:
        old = """.get(&spoofed_map_key)
            .is_some(),
        "set_nexus lane reset must not prune spoofed contract-map siblings"""
        mutated = _replace_in_region(
            self.source,
            "relay_contract",
            old,
            old.replace("is_some", "is_none"),
        )
        self.assert_rejected(mutated)

    def test_cursor_quantity_mutation_is_rejected(self) -> None:
        mutated = _replace_in_region(
            self.source,
            "cursor_journal",
            "919_u32",
            "920_u32",
        )
        self.assert_rejected(mutated)

    def test_cursor_pair_key_postcondition_mutation_is_rejected(self) -> None:
        old = ".get(reset_shard_id, rebound_lane_id)"
        mutated = _replace_in_region(
            self.source,
            "cursor_journal",
            old,
            ".get(reset_shard_id, LaneId::SINGLE)",
        )
        self.assert_rejected(mutated)

    def test_cursor_journal_postcondition_mutation_is_rejected(self) -> None:
        old = "reset_journal.cursor_for_lane(rebound_lane_id).is_none()"
        mutated = _replace_in_region(
            self.source,
            "cursor_journal",
            old,
            old.replace("is_none", "is_some"),
        )
        self.assert_rejected(mutated)

    def test_restart_pair_key_postcondition_mutation_is_rejected(self) -> None:
        old = ".get(reset_config.shard_id(recreated_lane_id), recreated_lane_id)"
        mutated = _replace_in_region(
            self.source,
            "cursor_journal",
            old,
            ".get(reset_config.shard_id(recreated_lane_id), LaneId::SINGLE)",
        )
        self.assert_rejected(mutated)

    def test_callback_escape_hatch_is_rejected(self) -> None:
        old = "fn assert_same_lane_da_reset_hides_previous_indexes(case: SameLaneDaResetCase)"
        mutated = _replace_in_region(
            self.source,
            "da_policy_replay",
            old,
            old.replace(
                ")",
                ", callback: Box<dyn Fn()>)",
            ),
        )
        self.assert_rejected(mutated)

    def test_source_budget_growth_is_rejected(self) -> None:
        growth = MAX_SOURCE_LINES - len(self.parent_source.splitlines()) + 1
        with self.assertRaises(GuardError):
            validate_source(
                self.source,
                parent_source=self.parent_source + "// synthetic growth\n" * growth,
            )


if __name__ == "__main__":
    unittest.main()
