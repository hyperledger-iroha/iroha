#!/usr/bin/env python3
"""Protect autoscale test matrices across the state test include closure."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path

from state_source_bundle import read_rust_source_bundle


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/state/tests.rs"
MAX_SOURCE_LINES = 54_850

TRIGGER_TESTS = (
    "autoscale_scale_in_triggered_requires_window_and_low_utilization",
    "autoscale_scale_in_triggered_rejects_disabled_missing_or_high_utilization",
    "autoscale_scale_in_triggered_rejects_missing_or_high_latency",
    "autoscale_scale_in_triggered_rejects_zero_window_even_with_ideal_metrics",
    "autoscale_scale_in_triggered_is_inclusive_at_thresholds_only",
    "autoscale_scale_out_triggered_accepts_hot_latency_or_utilization",
    "autoscale_scale_out_triggered_rejects_disabled_short_or_cold_windows",
    "autoscale_scale_in_triggered_rejects_public_window_shortfall_even_with_zero_metrics",
)
COMMIT_TESTS = (
    "autoscale_scale_out_height_mismatch_does_not_publish_storage_or_da",
    "autoscale_catalog_publication_failure_retains_original_geometry_for_explicit_rollback",
    "autoscale_commit_rejects_tampered_pending_transition_metadata_before_storage_publish",
    "autoscale_commit_rejects_tampered_pending_catalog_update_before_storage_publish",
    "autoscale_commit_scale_in_rejects_tampered_pending_transition_metadata_before_storage_publish",
    "autoscale_commit_scale_in_rejects_tampered_pending_catalog_update_before_storage_publish",
    "autoscale_commit_rejects_canonical_autoscale_disable_before_storage_publish",
    "autoscale_commit_rejects_committed_autoscale_setting_drift_before_storage_publish",
    "autoscale_commit_rejects_committed_routing_policy_drift_before_storage_publish",
    "autoscale_commit_rejects_canonical_dataspace_catalog_drift_before_storage_publish",
    "autoscale_commit_rejects_canonical_catalog_drift_before_storage_publish",
    "autoscale_commit_rejects_canonical_lane_config_drift_before_storage_publish",
    "autoscale_catalog_caches_cannot_replace_current_or_undo_runtime_owners",
    "autoscale_commit_failure_does_not_publish_staged_da_indexes",
    "autoscale_commit_kura_preflight_failure_does_not_publish_staged_da_or_tiered_state",
    "autoscale_commit_tiered_preflight_failure_does_not_publish_staged_da_or_kura_state",
    "autoscale_commit_scale_in_kura_preflight_failure_does_not_publish_staged_da_or_tiered_state",
    "autoscale_commit_scale_in_tiered_preflight_failure_does_not_publish_staged_da_or_kura_state",
)
COMMITTEE_TESTS = (
    "autoscale_scale_out_committee_preflight_rejects_three_peers_atomically",
    "autoscale_scale_out_committee_preflight_rejects_duplicate_topology_peers",
    "autoscale_scale_out_committee_preflight_rejects_non_live_consensus_key",
    "autoscale_scale_out_committee_preflight_rejects_insufficient_explicit_manifest",
    "autoscale_scale_out_committee_preflight_rejects_duplicate_manifest_authority",
    "autoscale_scale_out_committee_preflight_rejects_invalid_size_policy",
    "autoscale_scale_out_committee_preflight_rejects_proposal_height_overflow",
)
EXPECTED_TESTS = TRIGGER_TESTS + COMMIT_TESTS + COMMITTEE_TESTS
DIRECT_DECLARATION_TESTS = frozenset(
    (
        "autoscale_scale_in_triggered_rejects_public_window_shortfall_even_with_zero_metrics",
        "autoscale_commit_rejects_canonical_autoscale_disable_before_storage_publish",
        "autoscale_commit_rejects_committed_autoscale_setting_drift_before_storage_publish",
        "autoscale_commit_rejects_committed_routing_policy_drift_before_storage_publish",
        "autoscale_commit_rejects_canonical_dataspace_catalog_drift_before_storage_publish",
        "autoscale_commit_rejects_canonical_catalog_drift_before_storage_publish",
        "autoscale_commit_rejects_canonical_lane_config_drift_before_storage_publish",
        "autoscale_catalog_caches_cannot_replace_current_or_undo_runtime_owners",
    )
)
GENERATED_DECLARATION_TESTS = frozenset(
    (
        "autoscale_commit_rejects_tampered_pending_transition_metadata_before_storage_publish",
        "autoscale_commit_rejects_tampered_pending_catalog_update_before_storage_publish",
        "autoscale_commit_scale_in_rejects_tampered_pending_transition_metadata_before_storage_publish",
        "autoscale_commit_scale_in_rejects_tampered_pending_catalog_update_before_storage_publish",
    )
)

# The hashes retain the original matrices' assertions while tracking authenticated
# storage identities, canonical runtime ownership, and typed local storage refusal.
REGIONS = {
    "state_test_macro": (
        "macro_rules! state_test {",
        '#[path = "da_hydration_test_cases.rs"]',
        "f797fa141a1b987e7160758d273ba5120a36ca2df58dc2110459bdab8d2beb48",
    ),
    "trigger": (
        "state_test! { sync autoscale_scale_in_triggered_requires_window_and_low_utilization\n",
        "state_test! { sync autoscale_ratio_permille_sanitizes_adversarial_values\n",
        "8be2ec725769bd06268dd5e06ae0b10e4156230c05c7d711c76687049b9a74c1",
    ),
    "da_helpers": (
        "state_test! { sync autoscale_scale_out_height_mismatch_does_not_publish_storage_or_da\n",
        "state_test! { sync autoscale_catalog_publication_failure_retains_original_geometry_for_explicit_rollback\n",
        "fc0aeeb83ab52b1425dbfc8d62c9566a991143797753a988e63853f8687eaff9",
    ),
    "pending_tamper": (
        "#[derive(Clone, Copy)]\nenum PendingAutoscaleTamper",
        "#[derive(Clone, Copy, Debug)]\nenum CommittedAutoscaleDrift",
        "59f553ec4dbb638e9a2b2da90b8adc220b4f8572c872da8bb6db1fd0f95e8b86",
    ),
    "committed_drift": (
        "#[derive(Clone, Copy, Debug)]\nenum CommittedAutoscaleDrift",
        "state_test! { sync autoscale_commit_failure_does_not_publish_staged_da_indexes\n",
        "7d5c2e9cf5ed54da41c55865192762862519e139676c5f3e04a358b0f30a6561",
    ),
    "committee": (
        "state_test! { sync autoscale_scale_out_committee_preflight_rejects_three_peers_atomically\n",
        "state_test! { sync autoscale_transition_scale_out_fails_closed_when_id_range_exhausted\n",
        "2ab571b5a7df36e725439b40c6060ac63a69415d77c98c39265b3efcecce030e",
    ),
    "scale_out_preflight": (
        "state_test! { sync autoscale_commit_failure_does_not_publish_staged_da_indexes\n",
        "fn assert_autoscale_scale_in_preflight_failure_is_atomic(",
        "30e3df0ca919a2128a256ec780bd2c50f201b08a467d51a30899e3c9658da38c",
    ),
    "scale_in_preflight": (
        "fn assert_autoscale_scale_in_preflight_failure_is_atomic(",
        "#[derive(Clone, Copy, Debug)]\nenum AutoscaleNoopReason",
        "60e60e86926a6d7b67f6b3255204cf1899016bee0ec6027bfcc8b7f5dff24789",
    ),
}

DIRECT_TEST_HASHES = {
    "autoscale_scale_out_height_mismatch_does_not_publish_storage_or_da":
        "706f25515e56fdfd46abf7df1a6848498cddfad98ab71e082360f87204262de8",
    "autoscale_catalog_publication_failure_retains_original_geometry_for_explicit_rollback":
        "7e58f9202bf789a884e6b87fbe27117cd5e2b93a845b97666acce98a9d5b0416",
    "autoscale_commit_failure_does_not_publish_staged_da_indexes":
        "21ec5582995a345e66d23a870d5660e4fcb04dcb27af6d1db57dfa0d1ea73e91",
}

REGION_TOKENS = {
    "state_test_macro": (
        "(sync $name:ident $($body:tt)*)",
        "(result $name:ident $($body:tt)*)",
        "#[test]",
        "fn $name()",
        "fn $name() -> Result<()>",
    ),
    "trigger": (
        "autoscale_scale_in_triggered(",
        "autoscale_scale_out_triggered(",
        "Some(1_101)",
        "Some(251)",
        "Some(1_199)",
        "Some(599)",
        "assert!(",
    ),
    "da_helpers": (
        "StorageTicketId::new",
        "ManifestDigest::new",
        "pending_autoscale_lifecycle.is_some()",
        "pending_da_commitments.is_some()",
        "pending_da_pin_intents.is_some()",
        "da_commitments().bundle_at(2).is_none()",
        "da_receipt_cursors()",
        "da_pin_intents()",
        'get_by_alias("autoscale-height-mismatch-pin")',
    ),
    "pending_tamper": (
        "saturating_sub(1)",
        "AUTOSCALE_META_CREATED_HEIGHT",
        "previous_catalog.clone()",
        "previous_lane_config.clone()",
        "TransactionsBlockError::AutoscaleLaneLifecycle",
        "assert_lane_ids!",
        "elastic_blocks_dir.exists()",
        "retired_snapshot_dir.exists()",
        "pending_autoscale_tamper_tests!",
    ),
    "committed_drift": (
        "nonzero!(11_u32)",
        '"operator-routing-drift"',
        "DataSpaceId::new(7)",
        "LaneId::new(8)",
        '"drifted-default-lane"',
        "TransactionsBlockError::AutoscaleLaneLifecycle",
        "assert_lane_ids!",
        "assert_autoscale_rejects_canonical_catalog_tamper",
        "state.canonical_runtime.view().get(), &retained",
        "validate_canonical_runtime_projection",
        "block_and_revert(header)",
    ),
    "committee": (
        "LaneLifecycleError::AutoscaleCommitteeUnavailable",
        "LaneLifecycleError::AutoscaleCommitteeSizeInvalid",
        "LaneLifecycleError::AutoscaleProposalHeightOverflow",
        "assert_autoscale_committee_rejection_is_atomic",
        "u32::MAX",
        "u64::MAX",
    ),
    "scale_out_preflight": (
        "0xC0",
        "0xC1",
        "0xC2",
        "0xD0",
        "0xD1",
        "0xD2",
        "0xD4",
        "0xD5",
        "0xD6",
        "assert_autoscale_da_effects_staged",
        "bundle_at(bundle_height).is_none()",
        "untouched_path",
    ),
    "scale_in_preflight": (
        "0xE0",
        "0xE1",
        "0xE2",
        "0xE4",
        "0xE5",
        "0xE6",
        "0xD1",
        "0x94",
        "841",
        "894",
        "assert_lane_scoped_cleanup_fixture_pruned_from_state_block",
        "assert_lane_scoped_cleanup_fixture_present",
        "assert_public_lane_staking_status_bonded",
        "bundle_at(3).is_none()",
    ),
}

CASE_MACROS = ("pending_autoscale_tamper_tests",)
FORBIDDEN_CASE_TOKENS = ("Custom(", "Box::new", "=> |", "move |")


class GuardError(AssertionError):
    """Raised when the protected autoscale matrix contract changes."""


def _normalized_hash(source: str) -> str:
    normalized = re.sub(r"\s+", "", source)
    return hashlib.sha256(normalized.encode()).hexdigest()


def _unique_region(source: str, start: str, end: str, label: str) -> str:
    if source.count(start) != 1 or source.count(end) != 1:
        raise GuardError(f"{label}: region markers must each occur exactly once")
    start_at = source.index(start)
    end_at = source.index(end, start_at)
    return source[start_at:end_at]


def _matching_delimiter(source: str, opening: int) -> int:
    pairs = {"{": "}", "[": "]", "(": ")"}
    stack: list[str] = []
    state = "code"
    block_depth = 0
    index = opening
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and following == "/":
                state = "line_comment"
                index += 2
                continue
            if char == "/" and following == "*":
                state = "block_comment"
                block_depth = 1
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char == "'":
                state = "char"
            elif char in pairs:
                stack.append(pairs[char])
            elif char in "}])":
                if not stack or stack.pop() != char:
                    raise GuardError(f"mismatched delimiter at byte {index}")
                if not stack:
                    return index
        elif state == "line_comment":
            if char == "\n":
                state = "code"
        elif state == "block_comment":
            if char == "/" and following == "*":
                block_depth += 1
                index += 2
                continue
            if char == "*" and following == "/":
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
                continue
        else:
            if char == "\\":
                index += 2
                continue
            if (state == "string" and char == '"') or (state == "char" and char == "'"):
                state = "code"
        index += 1
    raise GuardError("unterminated Rust delimiter")


def _function_source(source: str, name: str) -> str:
    match = re.search(rf"\bfn\s+{re.escape(name)}\s*\([^)]*\)\s*\{{", source)
    if match is None:
        raise GuardError(f"missing direct function {name}")
    opening = match.end() - 1
    return source[match.start() : _matching_delimiter(source, opening) + 1]


def _test_source(source: str, name: str) -> str:
    state_test = re.search(
        rf"\bstate_test!\s*\{{\s*sync\s+{re.escape(name)}\b",
        source,
    )
    if state_test is None:
        return _function_source(source, name)
    opening = source.index("{", state_test.start())
    return source[state_test.start() : _matching_delimiter(source, opening) + 1]


def _macro_invocations(source: str, name: str) -> tuple[str, ...]:
    invocations = []
    for match in re.finditer(rf"\b{re.escape(name)}!\s*[{{([]", source):
        opening = match.end() - 1
        invocations.append(source[match.start() : _matching_delimiter(source, opening) + 1])
    if not invocations:
        raise GuardError(f"missing {name}! invocation")
    return tuple(invocations)


def validate_source(source: str) -> None:
    if len(SOURCE_PATH.read_text(encoding="utf-8").splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("state/tests.rs exceeded the frozen autoscale source budget")
    for name in EXPECTED_TESTS:
        occurrences = len(re.findall(rf"\b{re.escape(name)}\b", source))
        if occurrences != 1:
            raise GuardError(f"{name}: expected one source occurrence, found {occurrences}")
        if name in DIRECT_DECLARATION_TESTS:
            declaration = rf"#\[test\]\s*fn\s+{re.escape(name)}\b"
            if len(re.findall(declaration, source)) != 1:
                raise GuardError(f"{name}: expected one direct #[test] declaration")
        elif name in GENERATED_DECLARATION_TESTS:
            invocations = _macro_invocations(source, "pending_autoscale_tamper_tests")
            if sum(bool(re.search(rf"\b{re.escape(name)}\b", item)) for item in invocations) != 1:
                raise GuardError(f"{name}: expected one typed tamper-matrix declaration")
        else:
            declaration = rf"state_test!\s*\{{\s*sync\s+{re.escape(name)}\b"
            if len(re.findall(declaration, source)) != 1:
                raise GuardError(f"{name}: expected one synchronous state_test declaration")
    long_test_attributes = (
        "#[test]\n#[allow(clippy::too_many_lines)]\n"
        "fn autoscale_repeated_scale_in_retires_highest_safe_managed_lane_one_per_carrier"
    )
    if source.count(long_test_attributes) != 1:
        raise GuardError("the out-of-scope autoscale too_many_lines attribute order changed")
    for label, (start, end, expected_hash) in REGIONS.items():
        region = _unique_region(source, start, end, label)
        for token in REGION_TOKENS[label]:
            if token not in region:
                raise GuardError(f"{label}: missing semantic token {token!r}")
        observed_hash = _normalized_hash(region)
        if observed_hash != expected_hash:
            raise GuardError(
                f"{label}: semantic hash changed: {observed_hash} != {expected_hash}"
            )
    for name, expected_hash in DIRECT_TEST_HASHES.items():
        observed_hash = _normalized_hash(_test_source(source, name))
        if observed_hash != expected_hash:
            raise GuardError(f"{name}: bespoke test body changed")
    for macro_name in CASE_MACROS:
        for invocation in _macro_invocations(source, macro_name):
            for token in FORBIDDEN_CASE_TOKENS:
                if token in invocation:
                    raise GuardError(
                        f"{macro_name}: opaque case escape hatch {token!r} is forbidden"
                    )


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


class AutoscaleCaseMatrixSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = read_rust_source_bundle(SOURCE_PATH, root=REPO_ROOT)

    def test_current_source_preserves_autoscale_contract(self) -> None:
        validate_source(self.source)

    def test_missing_name_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            TRIGGER_TESTS[0],
            f"{TRIGGER_TESTS[0]}_mutated",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_attribute_mutation_is_rejected(self) -> None:
        name = TRIGGER_TESTS[-1]
        old = f"#[test]\nfn {name}"
        mutated = _replace_once(self.source, old, f"#[ignore]\nfn {name}")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_trigger_boundary_mutation_is_rejected(self) -> None:
        old = (
            "assert!(!autoscale_scale_in_triggered(\n"
            "        true,\n"
            "        191,\n"
            "        192,\n"
            "        Some(0),\n"
            "        1_100,\n"
            "        Some(0),\n"
            "        250\n"
            "    ));"
        )
        mutated = _replace_once(self.source, old, old.replace("191", "192", 1))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_adversarial_manifest_byte_mutation_is_rejected(self) -> None:
        old = (
            '0xE6,\n'
            '            "autoscale-scale-in-tiered-preflight-failure-pin"'
        )
        mutated = _replace_once(self.source, old, old.replace("0xE6", "0xE7"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_error_payload_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "LaneLifecycleError::AutoscaleProposalHeightOverflow(lane)",
            "LaneLifecycleError::AutoscaleProposalHeightOverflow(_lane)",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_canonical_disable_case_cannot_use_cache_drift_helper(self) -> None:
        mutated = _replace_once(
            self.source,
            "assert_autoscale_rejects_canonical_catalog_tamper(CommittedAutoscaleDrift::Disabled);",
            "assert_autoscale_catalog_cache_drift_is_not_authority(CommittedAutoscaleDrift::Disabled);",
        )
        with self.assertRaisesRegex(GuardError, "committed_drift: semantic hash changed"):
            validate_source(mutated)

    def test_explicit_rollback_call_mutation_is_rejected(self) -> None:
        name = "autoscale_catalog_publication_failure_retains_original_geometry_for_explicit_rollback"
        original = _test_source(self.source, name)
        mutated_test = _replace_once(
            original,
            "state.rollback_lane_geometry_updates(",
            "state.unchecked_geometry_update(",
        )
        mutated = _replace_once(self.source, original, mutated_test)
        with self.assertRaisesRegex(GuardError, f"{name}: bespoke test body changed"):
            validate_source(mutated)

    def test_storage_preservation_mutation_is_rejected(self) -> None:
        old = '"storage failure must preserve retired-lane operator staking status"'
        mutated = _replace_once(self.source, old, old.replace("preserve", "discard"))
        with self.assertRaises(GuardError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
