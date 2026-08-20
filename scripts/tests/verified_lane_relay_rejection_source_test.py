#!/usr/bin/env python3
"""Protect the typed verified-lane-relay rejection test matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/smartcontracts/isi/mod.rs"

ORIGINAL_SOURCE_LINES = 3_576
ORIGINAL_GOVERNED_LINES = 1_538
MINIMUM_RUST_LINE_SAVING = 750
MAX_SOURCE_LINES = ORIGINAL_SOURCE_LINES - MINIMUM_RUST_LINE_SAVING
MAX_REGION_LINES = ORIGINAL_GOVERNED_LINES - MINIMUM_RUST_LINE_SAVING

REGION_START = "    #[derive(Clone, Copy)]\n    enum LaneRelayRejectionCase"
REGION_END = "    #[test]\n    async fn nft"
REGION_SHA256 = "96e7ae783dba8fdfb4bddcc9db70eab636f6f8aa93f1bd4e53153a5758c418c5"
POSITIVE_START = (
    "    #[test]\n"
    "    async fn register_verified_lane_relay_instruction_box_is_registered"
)
POSITIVE_END = (
    "    #[test]\n"
    "    async fn default_executor_rejects_invalid_instruction_placeholders"
)
POSITIVE_SHA256 = "26a280831e3456fe060b1e9fb9662bdb3af38b15dd9aee2ab67611da9813f749"

EXPECTED_CASES = (
    ("register_verified_lane_relay_rejects_unknown_lane_id", "UnknownLaneId"),
    (
        "register_verified_lane_relay_rejects_stale_geometry_lane_id",
        "StaleGeometryLaneId",
    ),
    (
        "register_verified_lane_relay_rejects_lane_dataspace_mismatch",
        "LaneDataspaceMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_unknown_dataspace_id",
        "UnknownDataspaceId",
    ),
    (
        "register_verified_lane_relay_rejects_empty_proof_payload",
        "EmptyProofPayload",
    ),
    (
        "register_verified_lane_relay_rejects_malformed_proof_envelope",
        "MalformedProofEnvelope",
    ),
    (
        "register_verified_lane_relay_rejects_proof_manifest_root_mismatch",
        "ProofManifestRootMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_proof_dataspace_mismatch",
        "ProofDataspaceMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_stale_fastpq_height",
        "StaleFastpqHeight",
    ),
    (
        "register_verified_lane_relay_rejects_zero_like_fastpq_digest",
        "ZeroLikeFastpqDigest",
    ),
    (
        "register_verified_lane_relay_rejects_envelope_block_height_mismatch",
        "EnvelopeBlockHeightMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_settlement_lane_mismatch",
        "SettlementLaneMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_settlement_dataspace_mismatch",
        "SettlementDataspaceMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_settlement_hash_mismatch",
        "SettlementHashMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_settlement_totals_mismatch",
        "SettlementTotalsMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_mismatched_fastpq_digest",
        "MismatchedFastpqDigest",
    ),
    (
        "register_verified_lane_relay_rejects_mismatched_claim_digest",
        "MismatchedClaimDigest",
    ),
    (
        "register_verified_lane_relay_rejects_future_fastpq_height",
        "FutureFastpqHeight",
    ),
    (
        "register_verified_lane_relay_rejects_missing_manifest_root",
        "MissingManifestRoot",
    ),
    (
        "register_verified_lane_relay_rejects_zero_manifest_root",
        "ZeroManifestRoot",
    ),
    (
        "register_verified_lane_relay_rejects_expired_proof_blob",
        "ExpiredProofBlob",
    ),
    (
        "register_verified_lane_relay_rejects_missing_fastpq_binding",
        "MissingFastpqBinding",
    ),
    (
        "register_verified_lane_relay_rejects_source_dsid_mismatch",
        "SourceDsidMismatch",
    ),
    (
        "register_verified_lane_relay_rejects_wrong_effect_type",
        "WrongEffectType",
    ),
    (
        "register_verified_lane_relay_rejects_business_effect_smuggled_in_lane_proof",
        "BusinessEffectSmuggledInLaneProof",
    ),
    (
        "register_verified_lane_relay_rejects_unanchored_business_effect_proof",
        "UnanchoredBusinessEffectProof",
    ),
    (
        "register_verified_lane_relay_rejects_effect_proof_before_proof_verification",
        "EffectProofBeforeProofVerification",
    ),
    (
        "register_verified_lane_relay_rejects_missing_final_qc_before_state_write",
        "MissingFinalQcBeforeStateWrite",
    ),
    (
        "register_verified_lane_relay_rejects_malformed_existing_state",
        "MalformedExistingState",
    ),
    (
        "register_verified_lane_relay_rejects_conflicting_existing_state",
        "ConflictingExistingState",
    ),
)

EXPECTED_ERRORS = {
    "UnknownLaneId": ("unknown lane id must be rejected", "unknown lane id 4"),
    "StaleGeometryLaneId": (
        "stale derived geometry must not register verified relay state",
        "unknown lane id 4",
    ),
    "LaneDataspaceMismatch": (
        "lane dataspace mismatch must be rejected",
        "belongs to dataspace",
    ),
    "UnknownDataspaceId": (
        "unknown dataspace id must be rejected",
        "unknown dataspace id 10",
    ),
    "EmptyProofPayload": (
        "empty proof payload must be rejected",
        "proof payload is empty",
    ),
    "MalformedProofEnvelope": (
        "malformed proof envelope must be rejected",
        "proof envelope decode failed",
    ),
    "ProofManifestRootMismatch": (
        "proof manifest root mismatch must be rejected",
        "does not match the declared manifest_root",
    ),
    "ProofDataspaceMismatch": (
        "proof dataspace mismatch must be rejected",
        "does not match the declared manifest_root",
    ),
    "StaleFastpqHeight": (
        "stale proof material height must be rejected",
        "FASTPQ binding failed verification",
    ),
    "ZeroLikeFastpqDigest": (
        "zero-like FastPQ digest must be rejected",
        "FASTPQ binding failed verification",
    ),
    "EnvelopeBlockHeightMismatch": (
        "envelope block height mismatch must be rejected",
        "lane relay envelope failed verification",
        "block height",
    ),
    "SettlementLaneMismatch": (
        "settlement lane mismatch must be rejected",
        "lane relay envelope failed verification",
        "settlement",
    ),
    "SettlementDataspaceMismatch": (
        "settlement dataspace mismatch must be rejected",
        "lane relay envelope failed verification",
        "settlement",
    ),
    "SettlementHashMismatch": (
        "settlement hash mismatch must be rejected",
        "lane relay envelope failed verification",
        "settlement",
    ),
    "SettlementTotalsMismatch": (
        "settlement totals mismatch must be rejected",
        "lane relay envelope failed verification",
        "settlement",
    ),
    "MismatchedFastpqDigest": (
        "mismatched proof digest must be rejected",
        "proof digest does not match proof_blob payload",
    ),
    "MismatchedClaimDigest": (
        "mismatched claim digest must be rejected",
        "claim_digest mismatch",
    ),
    "FutureFastpqHeight": (
        "future proof height must be rejected",
        "proof metadata height is in the future",
    ),
    "MissingManifestRoot": (
        "missing manifest root must be rejected",
        "missing manifest_root",
    ),
    "ZeroManifestRoot": (
        "zero manifest root must be rejected",
        "manifest_root cannot be zeroed",
    ),
    "ExpiredProofBlob": ("expired proof must be rejected", "proof expired"),
    "MissingFastpqBinding": (
        "missing fastpq binding must be rejected",
        "missing fastpq_binding",
    ),
    "SourceDsidMismatch": (
        "source dataspace mismatch must be rejected",
        "source_dsid mismatch",
    ),
    "WrongEffectType": (
        "wrong effect type must be rejected",
        "effect must be lane_relay_block",
    ),
    "BusinessEffectSmuggledInLaneProof": (
        "lane proof must not smuggle a business-effect binding",
        "lane relay block proof must not carry a business-effect binding",
    ),
    "UnanchoredBusinessEffectProof": (
        "unanchored business-effect proof must be rejected",
        "business-effect promotion is disabled",
        "finalized, QC-anchored settlement ledger entry",
    ),
    "EffectProofBeforeProofVerification": (
        "disabled effect proof must be rejected before proof verification",
        "business-effect promotion is disabled",
        "finalized, QC-anchored settlement ledger entry",
    ),
    "MissingFinalQcBeforeStateWrite": (
        "a structurally valid proof without a final QC must not write relay state",
        "lane relay finality authentication failed",
        "QC missing",
    ),
    "MalformedExistingState": (
        "malformed existing state must be rejected",
        "stored",
    ),
    "ConflictingExistingState": (
        "conflicting existing state must be rejected",
        "conflicting verified lane relay",
    ),
}

EXPECTED_PROOF_SEEDS = {
    "UnknownLaneId": "register-lane-relay-unknown-lane",
    "StaleGeometryLaneId": "register-lane-relay-stale-geometry-lane",
    "LaneDataspaceMismatch": "register-lane-relay-lane-dsid-mismatch",
    "UnknownDataspaceId": "register-lane-relay-unknown-dsid",
    "ProofManifestRootMismatch": "register-lane-relay-proof-manifest-mismatch",
    "ProofDataspaceMismatch": "register-lane-relay-proof-dsid-mismatch",
    "StaleFastpqHeight": "register-lane-relay-stale-fastpq-height",
    "ZeroLikeFastpqDigest": "register-lane-relay-zero-like-fastpq-digest",
    "EnvelopeBlockHeightMismatch": "register-lane-relay-block-height-mismatch",
    "SettlementLaneMismatch": "register-lane-relay-settlement-lane-mismatch",
    "SettlementDataspaceMismatch": "register-lane-relay-settlement-dsid-mismatch",
    "SettlementHashMismatch": "register-lane-relay-settlement-hash-mismatch",
    "SettlementTotalsMismatch": "register-lane-relay-settlement-totals-mismatch",
    "MismatchedFastpqDigest": "register-lane-relay-digest-mismatch",
    "MismatchedClaimDigest": "register-lane-relay-claim-mismatch",
    "FutureFastpqHeight": "register-lane-relay-future-height",
    "MissingManifestRoot": "register-lane-relay-missing-manifest",
    "ZeroManifestRoot": "register-lane-relay-zero-manifest",
    "ExpiredProofBlob": "register-lane-relay-expired-proof",
    "MissingFastpqBinding": "register-lane-relay-missing-binding",
    "SourceDsidMismatch": "register-lane-relay-source-dsid-mismatch",
    "WrongEffectType": "register-lane-relay-wrong-effect-type",
    "BusinessEffectSmuggledInLaneProof": (
        "register-lane-relay-smuggled-business-effect"
    ),
    "UnanchoredBusinessEffectProof": "register-lane-relay-effect-primary",
    "MissingFinalQcBeforeStateWrite": "register-lane-relay-missing-final-qc",
    "MalformedExistingState": "register-lane-relay-malformed-existing",
    "ConflictingExistingState": "register-lane-relay-conflicting-existing",
}

REQUIRED_RUNNER_TOKENS = (
    "test must seed derived geometry for the removed lane",
    "test must keep the stale lane out of the authoritative catalog",
    "payload: vec![0xFF, 0x00, 0xFE]",
    "proof_envelope.manifest_root = [0x43; 32]",
    "proof_envelope.dsid = DataSpaceId::new(11)",
    '.claim_digest = "ee".repeat(32)',
    "proof_envelope.fastpq_binding = None",
    ".source_dsid = dsid.as_u64() + 1",
    '.verified_effect_type = "nexus_fee_budget".to_owned()',
    "destination_domain: Some(\"hbl.sbp\".to_owned())",
    "source_asset_definition_id: Some(\"aed#cbuae\".to_owned())",
    "destination_asset_definition_id: Some(\"pkr#sbp\".to_owned())",
    "source_amount_i64: Some(10)",
    "destination_amount_i64: Some(760)",
    "proof_digest: iroha_crypto::Hash::prehashed([",
    "envelope.block_height = envelope.block_height.saturating_add(1)",
    "envelope.settlement_commitment.lane_id = LaneId::new(4)",
    "envelope.settlement_commitment.dataspace_id = DataSpaceId::new(11)",
    'iroha_crypto::Hash::new(b"register-lane-relay-bad-settlement-hash")',
    "source_id: [0xA5; 32]",
    "timestamp_ms: 1_700_000_001_000",
    "existing.fastpq_statement_digest[0] ^= 0xFF",
    "unexpected rejection before the business-effect promotion guard: {err:?}",
    "rejected smuggled business effect must not persist relay state",
    "rejected business-effect proof must not persist relay state",
    "missing finality must leave the canonical relay key absent",
)

FORBIDDEN_ESCAPE_HATCHES = (
    "$body",
    "$setup",
    "$mutation",
    "$callback",
    ":expr",
    ":tt",
    "FnMut",
    "dyn Fn",
    "Box<dyn",
)


class GuardError(AssertionError):
    """Raised when the verified-lane-relay matrix contract changes."""


def _normalize(source: str) -> bytes:
    return re.sub(r"\s+", "", source).encode()


def _slice_between(source: str, start_marker: str, end_marker: str) -> str:
    if source.count(start_marker) != 1 or source.count(end_marker) != 1:
        raise GuardError("protected region markers must occur exactly once")
    start = source.index(start_marker)
    return source[start : source.index(end_marker, start)]


def _mask_rust(source: str) -> str:
    """Blank Rust comments and literals while preserving delimiters/newlines."""
    masked = list(source)
    index = 0
    state = "code"
    block_depth = 0
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        if state == "code":
            if source.startswith("//", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                state = "line_comment"
            elif source.startswith("/*", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                block_depth = 1
                state = "block_comment"
            elif char == '"':
                masked[index] = " "
                index += 1
                state = "string"
            elif char == "'":
                lifetime = (
                    index + 1 < len(source)
                    and (source[index + 1].isalpha() or source[index + 1] == "_")
                    and not (index + 2 < len(source) and source[index + 2] == "'")
                )
                if lifetime:
                    index += 1
                else:
                    masked[index] = " "
                    index += 1
                    state = "character"
            elif char == "r":
                raw_match = re.match(r'r(#+)?"', source[index:])
                if raw_match:
                    opener = raw_match.group(0)
                    raw_hashes = len(raw_match.group(1) or "")
                    for offset in range(index, index + len(opener)):
                        masked[offset] = " "
                    index += len(opener)
                    state = "raw_string"
                else:
                    index += 1
            else:
                index += 1
        elif state == "line_comment":
            if char == "\n":
                state = "code"
            else:
                masked[index] = " "
            index += 1
        elif state == "block_comment":
            if source.startswith("/*", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                block_depth += 1
            elif source.startswith("*/", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                block_depth -= 1
                if block_depth == 0:
                    state = "code"
            else:
                if char != "\n":
                    masked[index] = " "
                index += 1
        elif state in {"string", "character"}:
            if char == "\\":
                masked[index] = " "
                if index + 1 < len(source):
                    masked[index + 1] = " "
                index += 2
            elif (state == "string" and char == '"') or (
                state == "character" and char == "'"
            ):
                masked[index] = " "
                index += 1
                state = "code"
            else:
                if char != "\n":
                    masked[index] = " "
                index += 1
        else:
            terminator = '"' + "#" * raw_hashes
            if source.startswith(terminator, index):
                for offset in range(index, index + len(terminator)):
                    masked[offset] = " "
                index += len(terminator)
                state = "code"
            else:
                if char != "\n":
                    masked[index] = " "
                index += 1
    if state not in {"code", "line_comment"}:
        raise GuardError(f"unterminated Rust lexical state: {state}")
    return "".join(masked)


def _matching_brace(source: str, marker: str) -> str:
    start = source.index(marker)
    opening = source.index("{", start)
    masked = _mask_rust(source)
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    raise GuardError(f"unclosed Rust item after {marker!r}")


def _validate_delimiters(source: str) -> None:
    masked = _mask_rust(source)
    closing = {"}": "{", "]": "[", ")": "("}
    stack: list[tuple[str, int]] = []
    for index, char in enumerate(masked):
        if char in "{[(":
            stack.append((char, index))
        elif char in closing:
            if not stack or stack[-1][0] != closing[char]:
                raise GuardError(f"mismatched Rust delimiter {char!r} at byte {index}")
            stack.pop()
    if stack:
        char, index = stack[-1]
        raise GuardError(f"unclosed Rust delimiter {char!r} at byte {index}")


def _assert_in_order(source: str, tokens: tuple[str, ...]) -> None:
    offset = 0
    for token in tokens:
        found = source.find(token, offset)
        if found < 0:
            raise GuardError(f"ordered safety token missing: {token!r}")
        offset = found + len(token)


def validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("isi/mod.rs no longer saves at least 750 Rust lines")
    region = _slice_between(source, REGION_START, REGION_END)
    if len(region.splitlines()) > MAX_REGION_LINES:
        raise GuardError("verified-lane-relay matrix exceeded its 788-line ceiling")
    _validate_delimiters(region)
    digest = hashlib.sha256(_normalize(region)).hexdigest()
    if digest != REGION_SHA256:
        raise GuardError(f"verified-lane-relay region hash changed: {digest}")

    positive = _slice_between(source, POSITIVE_START, POSITIVE_END)
    positive_digest = hashlib.sha256(_normalize(positive)).hexdigest()
    if positive_digest != POSITIVE_SHA256:
        raise GuardError("instruction-registration positive test changed")

    enum_item = _matching_brace(region, "enum LaneRelayRejectionCase")
    variants = tuple(re.findall(r"^\s{8}([A-Z][A-Za-z0-9]+),$", enum_item, re.MULTILINE))
    expected_variants = tuple(variant for _, variant in EXPECTED_CASES)
    if variants != expected_variants:
        raise GuardError("typed rejection variants changed or were reordered")

    invocation = region[region.index("register_verified_lane_relay_rejection_tests! {") :]
    case_pattern = re.compile(
        r"(?P<attrs>(?:\s*#\[[^\]\n]+\]\s*)+)"
        r"(?P<name>register_verified_lane_relay_rejects_[a-z0-9_]+)\s*"
        r"=>\s*(?P<variant>[A-Z][A-Za-z0-9]+);"
    )
    case_matches = tuple(case_pattern.finditer(invocation))
    observed_cases = tuple(
        (match.group("name"), match.group("variant")) for match in case_matches
    )
    if observed_cases != EXPECTED_CASES:
        raise GuardError("test names, case mapping, or declaration order changed")
    if invocation.count("register_verified_lane_relay_rejects_") != len(EXPECTED_CASES):
        raise GuardError("name-emitter inventory is not one-to-one")
    for match in case_matches:
        attrs = tuple(re.findall(r"#\[([^\]\n]+)\]", match.group("attrs")))
        if attrs != ("test",):
            raise GuardError(f"{match.group('name')}: ordered attributes changed")

    expectation_item = _matching_brace(region, "fn expectation")
    for variant, expected_literals in EXPECTED_ERRORS.items():
        arm_match = re.search(
            rf"Case::{re.escape(variant)}\s*=>\s*(?:\{{\s*)?\((.*?)\)",
            expectation_item,
            re.DOTALL,
        )
        if arm_match is None:
            raise GuardError(f"missing error expectation for {variant}")
        literals = tuple(re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', arm_match.group(1)))
        if literals != expected_literals:
            raise GuardError(f"{variant}: error context/fragments changed or reordered")
    compact_expectation = re.sub(r"\s+", "", expectation_item)
    kind_contract = (
        "Case::ConflictingExistingState=>InvariantViolation,"
        "_=>InvalidParameter,"
    )
    if kind_contract not in compact_expectation:
        raise GuardError("InvalidParameter/InvariantViolation case partition changed")

    proof_seed_item = _matching_brace(region, "fn proof_seed")
    for variant, seed in EXPECTED_PROOF_SEEDS.items():
        pattern = rf"Case::{re.escape(variant)}\s*=>\s*(?:\{{\s*)?b\"{re.escape(seed)}\""
        if re.search(pattern, proof_seed_item) is None:
            raise GuardError(f"{variant}: proof seed changed")
    observed_seeds = tuple(re.findall(r'b"([^"]+)"', proof_seed_item))
    if observed_seeds != tuple(EXPECTED_PROOF_SEEDS.values()):
        raise GuardError("proof seeds changed, duplicated, or were reordered")

    macro_item = _matching_brace(region, "macro_rules! register_verified_lane_relay_rejection_tests")
    for token in FORBIDDEN_ESCAPE_HATCHES:
        if token in macro_item or token in _matching_brace(region, "fn run_lane_relay_rejection_case"):
            raise GuardError(f"opaque callback/custom escape hatch present: {token!r}")
    if tuple(re.findall(r"\$([a-z_]+):", macro_item)) != ("attr", "name", "case"):
        raise GuardError("name emitter accepts fields beyond attr/name/case")

    runner = _matching_brace(region, "fn run_lane_relay_rejection_case")
    for token in REQUIRED_RUNNER_TOKENS:
        if token not in runner:
            raise GuardError(f"runner safety literal missing: {token!r}")
    _assert_in_order(
        runner,
        (
            "Case::EffectProofBeforeProofVerification => Some(ProofBlob {",
            "payload: Vec::new()",
            "expiry_slot: None",
            "Case::EffectProofBeforeProofVerification => Some(ProofBlob {",
            "payload: vec![0xFF]",
            "expiry_slot: Some(state_transaction.block_height().saturating_sub(1))",
            "let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay",
            ".execute(&ALICE_ID, &mut state_transaction)",
        ),
    )
    _assert_in_order(
        runner,
        (
            "Case::ConflictingExistingState => {",
            "existing.fastpq_statement_digest[0] ^= 0xFF",
            "smart_contract_state.insert(",
            "let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay",
            ".execute(&ALICE_ID, &mut state_transaction)",
        ),
    )
    _assert_in_order(
        runner,
        (
            "Case::MissingFinalQcBeforeStateWrite => Some(relay_state_key_for_test(&envelope))",
            "let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay",
            ".execute(&ALICE_ID, &mut state_transaction)",
            "(Case::MissingFinalQcBeforeStateWrite, Some(relay_state_key))",
            "missing finality must leave the canonical relay key absent",
        ),
    )


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


class VerifiedLaneRelayRejectionSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def test_current_source_preserves_typed_rejection_matrix(self) -> None:
        validate_source(self.source)

    def test_test_name_mutation_is_rejected(self) -> None:
        name = "register_verified_lane_relay_rejects_missing_final_qc_before_state_write"
        mutated = _replace_once(self.source, name, f"{name}_mutated")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_ordered_attribute_mutation_is_rejected(self) -> None:
        name = "register_verified_lane_relay_rejects_malformed_existing_state"
        old = f"#[test]\n        {name}"
        mutated = _replace_once(self.source, old, old.replace("#[test]", "#[ignore]"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_case_order_mutation_is_rejected(self) -> None:
        first = (
            "register_verified_lane_relay_rejects_unknown_lane_id => UnknownLaneId;"
        )
        second = (
            "register_verified_lane_relay_rejects_stale_geometry_lane_id "
            "=> StaleGeometryLaneId;"
        )
        mutated = _replace_once(self.source, first, "CASE_ORDER_SENTINEL")
        mutated = _replace_once(mutated, second, first)
        mutated = _replace_once(mutated, "CASE_ORDER_SENTINEL", second)
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_fastpq_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            'b"register-lane-relay-claim-mismatch"',
            'b"register-lane-relay-weakened-claim"',
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_error_fragment_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            '"lane relay finality authentication failed"',
            '"finality check failed"',
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_effect_before_proof_order_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "payload: vec![0xFF],",
            "payload: Vec::new(),",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_state_absence_assertion_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "missing finality must leave the canonical relay key absent",
            "weakened state assertion",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_macro_escape_hatch_mutation_is_rejected(self) -> None:
        old = "$name:ident => $case:ident;"
        mutated = _replace_once(self.source, old, "$name:ident => $case:ident, $body:expr;")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_positive_registration_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "RegisterVerifiedLaneRelay must be wired into INSTRUCTION_HANDLERS",
            "weakened instruction registration assertion",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_delimiter_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            "existing.fastpq_statement_digest[0] ^= 0xFF;",
            "existing.fastpq_statement_digest[0] ^= 0xFF; }",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_line_ceiling_mutation_is_rejected(self) -> None:
        padding = "\n" * (MAX_REGION_LINES + 1)
        mutated = _replace_once(self.source, REGION_START, REGION_START + padding)
        with self.assertRaises(GuardError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
