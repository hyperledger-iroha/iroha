#!/usr/bin/env python3
"""Protect the typed lane-block test helpers and their safety inventory."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/lane_consensus.rs"
MAX_SOURCE_LINES = 9_497
MAX_REGION_LINES = 2_413
REGION_START = "    fn lane_block_validator_fixture"
REGION_END = '    include!("lane_consensus/session_capacity_tests.rs");'
REGION_HASH = "a8aa8300c249fc7725710720fbfcae6ff2d40f749ea398d84c77dc3e3cc7a28b"

EXPECTED_TESTS = (
    "lane_block_proposal_ingress_accepts_canonical_artifact",
    "lane_block_proposal_ingress_accepts_coordinate_boundaries",
    "lane_block_proposal_ingress_rejects_adversarial_coordinates",
    "lane_block_proposal_ingress_rejects_shape_and_committee_drift",
    "lane_block_consensus_rejects_work_above_global_merge_capacity",
    "lane_block_proposal_ingress_rejects_hash_drift",
    "lane_block_vote_ingress_accepts_matching_signed_bls_vote",
    "lane_block_vote_explicit_none_roundtrips_and_omission_fails_closed",
    "lane_block_vote_and_qc_ingress_require_nonzero_proposal_height",
    "lane_block_vote_ingress_rejects_phase_algorithm_and_signature_drift",
    "lane_block_qc_preserves_sparse_high_index_signer_order",
    "lane_block_qc_ingress_accepts_aggregate_shape",
    "lane_block_qc_aggregate_verifier_requires_valid_pops_and_signature",
    "lane_block_qc_ingress_rejects_adversarial_shapes",
    "lane_block_session_cache_accepts_out_of_order_artifacts",
    "lane_block_session_cache_seals_qc_when_vote_quorum_arrives",
    "lane_block_session_cache_drains_committed_session_once_from_sealed_qcs",
    "lane_block_session_cache_rejects_conflicting_commit_vote_after_view_change",
    "lane_block_session_cache_rejects_conflicting_commit_qc_with_overlapping_signer",
    "lane_block_session_cache_drains_committed_session_from_inbound_qcs",
    "lane_block_session_cache_drains_commit_vote_request_once_after_prepare_qc",
    "lane_block_session_cache_lists_prepare_vote_opportunities_until_vote_or_qc_arrives",
    "lane_block_session_cache_lists_commit_vote_opportunities_without_draining",
    "lane_block_session_cache_lists_proposals_without_commit_qc_for_rebroadcast",
    "lane_block_session_cache_lists_local_vote_rebroadcast_artifacts",
    "lane_block_session_cache_lists_qcs_for_incomplete_session_rebroadcast",
    "lane_block_session_cache_skips_commit_vote_request_for_nonmember_or_existing_vote",
    "lane_block_session_cache_does_not_drain_until_proposal_and_both_qcs",
    "lane_block_session_cache_treats_same_body_alternate_quorum_qc_as_duplicate",
    "lane_block_session_cache_reconciles_orphan_qc_drift_before_commit_drain",
    "lane_block_session_cache_seals_reconciled_orphan_vote_quorum",
    "lane_block_session_cache_preflight_rejects_conflicting_proposal_without_mutation",
    "lane_block_session_cache_preflight_rejects_conflicting_vote_without_mutation",
    "lane_block_session_cache_tracks_exact_duplicate_artifacts",
    "lane_block_session_cache_merges_payload_hint_for_duplicate_proposal",
    "lane_block_session_cache_refreshes_commit_drain_after_payload_hint_merge",
    "lane_block_session_cache_does_not_drain_inbound_qc",
    "lane_block_session_cache_rejects_conflicts_and_duplicate_replays",
    "lane_block_session_cache_rejects_cross_session_entrypoint_replays",
    "lane_block_session_cache_recovered_proposal_replaces_uncertified_conflicting_slot",
    "lane_block_session_cache_single_orphan_vote_cannot_displace_slot_proposal",
    "lane_block_session_cache_recovered_proposal_replaces_prepare_voted_conflicting_slot",
    "lane_block_session_cache_recovered_proposal_preserves_prepared_conflicting_slot",
    "lane_block_session_cache_recovered_proposal_preserves_commit_voted_conflicting_slot",
    "lane_block_session_cache_recovered_proposal_preserves_committed_conflicting_slot",
    "lane_block_session_cache_rejects_forged_aggregate_qc",
    "lane_block_session_cache_reconciles_orphan_vote_drift_on_proposal",
    "lane_block_session_cache_enforces_capacity",
    "lane_block_rollover_preserves_partial_votes_prepare_qc_and_commit_lock",
    "lane_block_rollover_prunes_unanchored_finalized_and_inactive_evidence",
    "lane_block_rollover_fails_atomically_on_certified_canonical_conflict",
    "lane_block_rollover_fails_on_pruned_certified_commit_locks",
    "lane_block_session_cache_prunes_inadmissible_lane_sessions_and_slot_claims",
    "lane_block_session_cache_prunes_noncanonical_prepared_siblings_but_preserves_commit_evidence",
    "lane_block_session_cache_bounds_speculative_siblings_by_historical_context",
)

HELPER_OCCURRENCES = {
    "lane_block_validator_fixture": 52,
    "assert_proposal_insert": 44,
    "assert_vote_insert": 33,
    "assert_qc_insert": 7,
    "assert_qc_insert_with_pops": 26,
}
REQUIRED_TOKENS = (
    "(1..=count).map(checked_bls_keypair)",
    "validator_set.sort();",
    "cache.insert_vote(vote.clone(), Some(&vote.signer))",
    "cache.insert_qc_with_pops(qc, pops)",
    "LaneBlockSessionError::ConflictingVote",
    "LaneBlockSessionError::ConflictingProposal",
    "LaneBlockSessionError::EntrypointAlreadyClaimed",
    "duplicate commit votes must not mutate replay state or signer locks",
    "rejected commit QCs must not mutate replay state or signer locks",
    "surviving quorum commit locks must make conflict preflight mutation-free",
)
FORBIDDEN_TOKENS = ("$body", "$setup", "Custom(", "FnMut", "dyn Fn", "Box<dyn")


class GuardError(AssertionError):
    """Raised when the protected lane-block test contract changes."""


def _region(source: str) -> str:
    if source.count(REGION_START) != 1 or source.count(REGION_END) != 1:
        raise GuardError("lane-block helper region markers must occur exactly once")
    start = source.index(REGION_START)
    return source[start : source.index(REGION_END, start)]


def _validate_delimiters(source: str) -> None:
    pairs = {"{": "}", "[": "]", "(": ")"}
    stack: list[str] = []
    state = "code"
    block_depth = 0
    index = 0
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
                lifetime = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", source[index:])
                if lifetime is None or source[index + len(lifetime.group()) :].startswith("'"):
                    state = "char"
            elif char in pairs:
                stack.append(pairs[char])
            elif char in "}])":
                if not stack or stack.pop() != char:
                    raise GuardError(f"mismatched Rust delimiter at byte {index}")
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
    if stack or state not in {"code", "line_comment"}:
        raise GuardError("unterminated Rust delimiter or literal")


def validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("lane_consensus.rs exceeded its compacted source ceiling")
    region = _region(source)
    _validate_delimiters(region)
    if len(region.splitlines()) > MAX_REGION_LINES:
        raise GuardError("lane-block helper region exceeded its compacted source ceiling")
    digest = hashlib.sha256(re.sub(r"\s+", "", region).encode()).hexdigest()
    if digest != REGION_HASH:
        raise GuardError(f"lane-block helper region hash changed: {digest}")

    matches = tuple(
        re.finditer(
            r"(?P<attrs>(?:    #\[[^\]\n]+\]\n)+)"
            r"    fn (?P<name>lane_block[a-z0-9_]+)\s*\(",
            region,
        )
    )
    names = tuple(match.group("name") for match in matches)
    if names != EXPECTED_TESTS:
        raise GuardError("lane-block test names or declaration order changed")
    for match in matches:
        attrs = tuple(re.findall(r"#\[([^\]\n]+)\]", match.group("attrs")))
        if attrs != ("test",):
            raise GuardError(f"{match.group('name')}: ordered attributes changed")

    for helper, expected in HELPER_OCCURRENCES.items():
        observed = source.count(f"{helper}(")
        if observed != expected:
            raise GuardError(f"{helper}: expected {expected} occurrences, found {observed}")
    for token in REQUIRED_TOKENS:
        if token not in region:
            raise GuardError(f"lane-block safety token missing: {token!r}")
    for token in FORBIDDEN_TOKENS:
        if token in region:
            raise GuardError(f"lane-block helper escape hatch present: {token!r}")


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


class LaneBlockTestHelperSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def test_current_source_preserves_typed_helpers_and_inventory(self) -> None:
        validate_source(self.source)

    def test_name_mutation_is_rejected(self) -> None:
        name = "lane_block_session_cache_rejects_forged_aggregate_qc"
        mutated = _replace_once(self.source, name, f"{name}_mutated")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_attribute_mutation_is_rejected(self) -> None:
        name = "lane_block_rollover_fails_atomically_on_certified_canonical_conflict"
        old = f"#[test]\n    fn {name}"
        mutated = _replace_once(self.source, old, old.replace("#[test]", "#[ignore]"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_validator_order_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "(1..=count).map(checked_bls_keypair)",
            "(0..count).map(checked_bls_keypair)",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_typed_result_assertion_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "assert_eq!(cache.insert_proposal(proposal), Ok(expected));",
            "let _ = cache.insert_proposal(proposal);",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_atomicity_assertion_mutation_is_rejected(self) -> None:
        token = "surviving quorum commit locks must make conflict preflight mutation-free"
        mutated = _replace_once(self.source, token, "weakened atomicity assertion")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_delimiter_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "assert_eq!(cache.insert_qc(qc), Ok(expected));",
            "assert_eq!(cache.insert_qc(qc), Ok(expected)); }",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
