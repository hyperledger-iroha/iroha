#!/usr/bin/env python3
"""Protect the name-preserving autoscale test matrices in state/tests.rs."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


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
    "autoscale_catalog_publication_failure_rolls_back_prepared_geometry_in_process",
    "autoscale_commit_rejects_tampered_pending_transition_metadata_before_storage_publish",
    "autoscale_commit_rejects_tampered_pending_catalog_update_before_storage_publish",
    "autoscale_commit_scale_in_rejects_tampered_pending_transition_metadata_before_storage_publish",
    "autoscale_commit_scale_in_rejects_tampered_pending_catalog_update_before_storage_publish",
    "autoscale_commit_revalidates_disabled_autoscale_before_storage_publish",
    "autoscale_commit_rejects_committed_autoscale_setting_drift_before_storage_publish",
    "autoscale_commit_rejects_committed_routing_policy_drift_before_storage_publish",
    "autoscale_commit_rejects_committed_dataspace_catalog_drift_before_storage_publish",
    "autoscale_commit_rejects_committed_catalog_drift_before_storage_publish",
    "autoscale_commit_rejects_committed_lane_config_drift_before_storage_publish",
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

REGIONS = {
    "trigger": (
        "#[derive(Clone, Copy, Debug)]\nenum AutoscaleTriggerDirection",
        "#[test]\nfn autoscale_ratio_permille_sanitizes_adversarial_values",
        "3f4911068a3bfd98c46d7cf70d4e65e28a077dbc3acab499af18c27e96a538ef",
    ),
    "da_helpers": (
        "#[derive(Clone, Copy)]\nstruct AutoscaleDaTestSentinels",
        "#[test]\nfn autoscale_scale_out_height_mismatch_does_not_publish_storage_or_da",
        "528708975f57a31c7d3ef52a79f0b4b934a83a2efa1fa7b983d3a86836d5c155",
    ),
    "pending_tamper": (
        "#[derive(Clone, Copy)]\nenum PendingTransitionTamper",
        "#[derive(Clone, Copy, Debug)]\nenum CommittedAutoscaleDrift",
        "c3bfdee81308f2096dc08a7d87825ffc69bb4065d6ee09a522c554682f9a4d38",
    ),
    "committed_drift": (
        "#[derive(Clone, Copy, Debug)]\nenum CommittedAutoscaleDrift",
        "#[test]\nfn autoscale_commit_failure_does_not_publish_staged_da_indexes",
        "1e9f3e02f414efc2e552fe31ca6e44099e8912e13a82367ce39b0f27d07486d1",
    ),
    "committee": (
        "#[derive(Clone, Copy, Debug)]\nenum AutoscaleCommitteeRejectionCase",
        "#[test]\nfn lane_committee_protocol_limit_accepts_128_and_rejects_larger_sets",
        "cd11a4479184cd31a94e377ad41c894eadd9466bac8ee2f9ad1021372d87b0b6",
    ),
    "scale_out_preflight": (
        "#[derive(Clone, Copy)]\nenum ScaleOutAutoscalePreflightFailure",
        "#[derive(Clone, Copy)]\nenum ScaleInAutoscalePreflightFailure",
        "52a6bd08bba94e0b9dbf71d06e5b7f50cc1c7c985a6c7ce8ad4c1367db25c0b3",
    ),
    "scale_in_preflight": (
        "#[derive(Clone, Copy)]\nenum ScaleInAutoscalePreflightFailure",
        "#[test]\nfn autoscale_transition_noops_when_nexus_disabled_even_if_autoscale_enabled",
        "d662f6e40bd4138b3e93bfe22ad10b554033e1685377247db7d1d833fc80ca1c",
    ),
}

DIRECT_TEST_HASHES = {
    "autoscale_scale_out_height_mismatch_does_not_publish_storage_or_da":
        "5cffaebcf73332972d1eb6bf31082cd48c54938d22948606900a905ea20e9aca",
    "autoscale_catalog_publication_failure_rolls_back_prepared_geometry_in_process":
        "f115d5c089bbc5b3eba545694a6fbc42299a0edd843add73518523efee803ae3",
    "autoscale_commit_failure_does_not_publish_staged_da_indexes":
        "bbdf12586a20d38af6b3f7cacc62654a0e24c70fea3129bcf878bff329e558f5",
}

REGION_TOKENS = {
    "trigger": (
        "AutoscaleTriggerCase::new",
        "autoscale_scale_in_triggered",
        "autoscale_scale_out_triggered",
        "assert!(",
    ),
    "da_helpers": (
        "StorageTicketId::new",
        "ManifestDigest::new",
        "pending_autoscale_lifecycle.is_some()",
        "pending_da_commitments.is_some()",
        "pending_da_pin_intents.is_some()",
        "da_commitments().bundle_at(height).is_none()",
        "da_receipt_cursors()",
        "da_pin_intents().get_by_alias(alias).is_none()",
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
    ),
    "committed_drift": (
        "nonzero!(11_u32)",
        '"operator-routing-drift"',
        "DataSpaceId::new(7)",
        "LaneId::new(8)",
        '"drifted-default-lane"',
        "TransactionsBlockError::AutoscaleLaneLifecycle",
        "assert_lane_ids!",
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
        "0xD0",
        "0xD1",
        "0xD2",
        "0xD4",
        "0xD5",
        "0xD6",
        "autoscale_da_test_bundles",
        "assert_autoscale_da_payload_absent",
        "elastic_blocks_dir.is_file()",
        "elastic_snapshot_dir.is_file()",
    ),
    "scale_in_preflight": (
        "0xE0",
        "0xE1",
        "0xE2",
        "0xE4",
        "0xE5",
        "0xE6",
        "0x81",
        "0x94",
        "841",
        "894",
        "assert_lane_scoped_cleanup_fixture_pruned_from_state_block",
        "assert_lane_scoped_cleanup_fixture_present",
        "assert_public_lane_economic_state_presence",
        "assert_public_lane_staking_status_bonded",
        "assert_autoscale_da_payload_absent",
    ),
}

CASE_MACROS = (
    "autoscale_trigger_tests",
    "autoscale_pending_tamper_test",
    "autoscale_commit_drift_tests",
    "autoscale_committee_rejection_tests",
    "scale_out_autoscale_preflight_failure_tests",
    "scale_in_autoscale_preflight_failure_tests",
)
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


def _macro_invocations(source: str, name: str) -> tuple[str, ...]:
    invocations = []
    for match in re.finditer(rf"\b{re.escape(name)}!\s*\{{", source):
        opening = match.end() - 1
        invocations.append(source[match.start() : _matching_delimiter(source, opening) + 1])
    if not invocations:
        raise GuardError(f"missing {name}! invocation")
    return tuple(invocations)


def validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("state/tests.rs exceeded the frozen autoscale source budget")
    for name in EXPECTED_TESTS:
        occurrences = len(re.findall(rf"\b{re.escape(name)}\b", source))
        if occurrences != 1:
            raise GuardError(f"{name}: expected one source occurrence, found {occurrences}")
        attributed = re.search(
            rf"(?P<attrs>(?:#\[[^\]\n]+\]\s*)+)(?:fn\s+)?{re.escape(name)}\b",
            source,
        )
        if attributed is None:
            raise GuardError(f"{name}: missing ordered attributes")
        attributes = re.findall(r"#\[([^\]\n]+)\]", attributed.group("attrs"))
        if attributes != ["test"]:
            raise GuardError(f"{name}: expected only #[test], found {attributes}")
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
        observed_hash = _normalized_hash(_function_source(source, name))
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
        cls.source = SOURCE_PATH.read_text()

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
        old = f"#[test]\n    {COMMITTEE_TESTS[0]}"
        mutated = _replace_once(self.source, old, f"#[ignore]\n    {COMMITTEE_TESTS[0]}")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_trigger_boundary_mutation_is_rejected(self) -> None:
        old = "AutoscaleTriggerCase::new(true, 191, 192, Some(0), 1_100, Some(0), 250, false)"
        mutated = _replace_once(self.source, old, old.replace("191", "192", 1))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_adversarial_manifest_byte_mutation_is_rejected(self) -> None:
        old = (
            'manifest: 0xE6,\n'
            '                    alias: "autoscale-scale-in-tiered-preflight-failure-pin"'
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

    def test_storage_preservation_mutation_is_rejected(self) -> None:
        old = (
            '"tiered retire preflight failure must abort state commit",\n'
            '            "Kura retirement must not run after tiered preflight failure",\n'
            '            "source tiered snapshot must remain after failed retire preflight",'
        )
        mutated = _replace_once(self.source, old, old.replace("remain", "vanish"))
        with self.assertRaises(GuardError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
