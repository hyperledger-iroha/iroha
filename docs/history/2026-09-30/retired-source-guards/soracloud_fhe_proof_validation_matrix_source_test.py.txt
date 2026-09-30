#!/usr/bin/env python3
"""Seal the callback-free Soracloud FHE proof-validation test matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_data_model/src/soracloud/tests/proof_validation.rs"
ORIGINAL_LINE_COUNT = 1_804
MINIMUM_RUST_LINE_REDUCTION = 750
MAXIMUM_LINE_COUNT = ORIGINAL_LINE_COUNT - MINIMUM_RUST_LINE_REDUCTION
PROVENANCE_SUFFIX_LINES = 137
PROVENANCE_SUFFIX_SHA256 = (
    "9cf253efe5ab26f15331eff89cfff59ad2fb8e3d4d2a5ea4b89fca5b83c5fc28"
)
FHE_PREFIX_NORMALIZED_SHA256 = (
    "cf5fafc76195a6ee628fae10f64b73cb33205c016848ce3239cbdbbd47961464"
)
SUFFIX_MARKER = b"#[test]\nfn rollout_provenance_payload_encodes_canonical_tuple() {\n"

EXPECTED_CASE_IDS = (
    "fhe_input_admission_proof_validate_rejects_public_input_shape_replay",
    "fhe_input_admission_open_verify_bounds_match_published_caps",
    "fhe_input_admission_proof_validate_rejects_oversized_proof_payloads",
    "fhe_input_admission_proof_validate_requires_canonical_vk_ref_name",
    "fhe_input_admission_proof_validate_rejects_backend_mismatch",
    "fhe_public_key_proof_validate_accepts_canonical_envelope",
    "fhe_public_key_proof_validate_requires_vk_commitment_and_matching_envelope_hash",
    "fhe_public_key_proof_validate_rejects_open_verify_envelope_drift",
    "fhe_public_key_proof_validate_rejects_public_input_shape_replay",
    "fhe_public_key_proof_open_verify_bounds_match_published_caps",
    "fhe_public_key_proof_validate_rejects_oversized_proof_payloads",
    "fhe_public_key_proof_validate_rejects_attachment_metadata_drift",
    "fhe_bootstrap_key_proof_validate_accepts_canonical_envelope",
    "fhe_bootstrap_key_proof_validate_requires_vk_commitment_and_matching_envelope_hash",
    "fhe_bootstrap_key_proof_validate_rejects_open_verify_envelope_drift",
    "fhe_bootstrap_key_proof_validate_rejects_public_input_shape_replay",
    "fhe_bootstrap_key_proof_open_verify_bounds_match_published_caps",
    "fhe_bootstrap_key_proof_validate_rejects_oversized_proof_payloads",
    "fhe_bootstrap_key_proof_validate_requires_canonical_vk_ref_name",
    "fhe_bootstrap_key_proof_validate_rejects_attachment_metadata_drift",
    "fhe_full_bootstrap_execution_proof_validate_accepts_canonical_envelope",
    "fhe_full_bootstrap_execution_proof_validate_requires_vk_commitment_and_matching_envelope_hash",
    "fhe_full_bootstrap_execution_proof_validate_rejects_open_verify_envelope_drift",
    "fhe_full_bootstrap_execution_proof_validate_rejects_public_input_shape_replay",
    "fhe_full_bootstrap_execution_proof_open_verify_bounds_match_published_caps",
    "fhe_full_bootstrap_execution_proof_validate_rejects_oversized_proof_payloads",
    "fhe_full_bootstrap_execution_proof_validate_rejects_attachment_metadata_drift",
)

EXPECTED_ROUTES = (
    ("InputAdmission", "PublicInputShapeReplay"),
    ("InputAdmission", "PublishedBounds"),
    ("InputAdmission", "OversizedPayloads"),
    ("InputAdmission", "CanonicalVerifierName"),
    ("InputAdmission", "InputAdmissionBackendMismatch"),
    ("PublicKey", "CanonicalEnvelope"),
    ("PublicKey", "CommitmentAndEnvelopeHash"),
    ("PublicKey", "OpenVerifyEnvelopeDrift"),
    ("PublicKey", "PublicInputShapeReplay"),
    ("PublicKey", "PublishedBounds"),
    ("PublicKey", "OversizedPayloads"),
    ("PublicKey", "AttachmentMetadataDrift"),
    ("BootstrapKey", "CanonicalEnvelope"),
    ("BootstrapKey", "CommitmentAndEnvelopeHash"),
    ("BootstrapKey", "OpenVerifyEnvelopeDrift"),
    ("BootstrapKey", "PublicInputShapeReplay"),
    ("BootstrapKey", "PublishedBounds"),
    ("BootstrapKey", "OversizedPayloads"),
    ("BootstrapKey", "CanonicalVerifierName"),
    ("BootstrapKey", "AttachmentMetadataDrift"),
    ("FullBootstrapExecution", "CanonicalEnvelope"),
    ("FullBootstrapExecution", "CommitmentAndEnvelopeHash"),
    ("FullBootstrapExecution", "OpenVerifyEnvelopeDrift"),
    ("FullBootstrapExecution", "PublicInputShapeReplay"),
    ("FullBootstrapExecution", "PublishedBounds"),
    ("FullBootstrapExecution", "OversizedPayloads"),
    ("FullBootstrapExecution", "AttachmentMetadataDrift"),
)

SEALED_ATOMS = (
    b"fill_byte: 0xA5",
    b"fill_byte: 0xAA",
    b"fill_byte: 0xB5",
    b"fill_byte: 0xD5",
    b"other_statement_seed: 15",
    b"canonical_commitment: 0x42",
    b"canonical_commitment: 0x4A",
    b"canonical_commitment: 0x52",
    b"canonical_commitment: 0x63",
    b"forged_commitment: 0xA4",
    b"forged_commitment: 0x25",
    b"forged_commitment: 0x27",
    b'wrong_circuit_id: "soracloud_fhe_public_key_v2"',
    b'wrong_circuit_id: "soracloud_fhe_bootstrap_key_proof_v2"',
    b'wrong_circuit_id: "iroha_bfv_full_bootstrap_v2"',
    b'b"soracloud:fhe-public-key:public-inputs:v2"',
    b'b"soracloud:fhe-bootstrap-key:public-inputs:v2"',
    b'b"soracloud:fhe-full-bootstrap-execution:public-inputs:v2"',
    b"wrong_vk_hash_envelope.vk_hash = [0xA4; 32]",
    b"version_drift.version = 2",
    b"sample_hash(99)",
    b"all_zero_native_open.envelope_bytes = vec![0; 32]",
    b'empty_backend.attachment_mut().backend = " \\t ".into()',
    b'FheErrorExpectation::EmptyField("proof.backend")',
    b"#[test]\n#[allow(clippy::too_many_lines)]\nfn fhe_proof_validation_matrix()",
)

EXPECTED_PROFILE_ATOMS = {
    "InputAdmission": (
        b"fill_byte: 0xA5",
        b"other_statement_seed: 21",
        b"canonical_commitment: 0x42",
        b"forged_commitment: 0xA4",
        b"circuit_id: SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1",
        b'wrong_circuit_id: "soracloud_fhe_input_admission_v2"',
        b"public_inputs_schema: SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1",
        b'b"soracloud:fhe-input-admission:public-inputs:v2"',
        b'verifier_alias: "soracloud_fhe_input_admission_alias_v1"',
        b"SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES",
        b"SORACLOUD_FHE_INPUT_ADMISSION_MAX_STARK_WRAPPER_BYTES",
        b"SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES",
    ),
    "PublicKey": (
        b"fill_byte: 0xAA",
        b"other_statement_seed: 15",
        b"canonical_commitment: 0x4A",
        b"forged_commitment: 0xA4",
        b"circuit_id: SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1",
        b'wrong_circuit_id: "soracloud_fhe_public_key_v2"',
        b"public_inputs_schema: SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1",
        b'b"soracloud:fhe-public-key:public-inputs:v2"',
        b'verifier_alias: "soracloud_fhe_public_key_alias_v1"',
        b"SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES",
        b"SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_STARK_WRAPPER_BYTES",
        b"SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES",
    ),
    "BootstrapKey": (
        b"fill_byte: 0xB5",
        b"other_statement_seed: 21",
        b"canonical_commitment: 0x52",
        b"forged_commitment: 0x25",
        b"circuit_id: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1",
        b'wrong_circuit_id: "soracloud_fhe_bootstrap_key_proof_v2"',
        b"public_inputs_schema: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1",
        b'b"soracloud:fhe-bootstrap-key:public-inputs:v2"',
        b'verifier_alias: "soracloud_fhe_bootstrap_key_alias_v1"',
        b"SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES",
        b"SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_STARK_WRAPPER_BYTES",
        b"SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES",
    ),
    "FullBootstrapExecution": (
        b"fill_byte: 0xD5",
        b"other_statement_seed: 21",
        b"canonical_commitment: 0x63",
        b"forged_commitment: 0x27",
        b"circuit_id: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1",
        b'wrong_circuit_id: "iroha_bfv_full_bootstrap_v2"',
        b"SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1",
        b'b"soracloud:fhe-full-bootstrap-execution:public-inputs:v2"',
        b'verifier_alias: "soracloud_fhe_full_bootstrap_execution_alias_v1"',
        b"SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES",
        b"SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_STARK_WRAPPER_BYTES",
        b"SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES",
    ),
}

EXPECTED_ERROR_TOKEN_COUNTS = {
    b'FheErrorExpectation::InvalidField("proof.proof.bytes")': 14,
    b'FheErrorExpectation::InvalidField("proof.proof.backend")': 2,
    b'FheErrorExpectation::InvalidField("proof.vk_ref.backend")': 1,
    b'FheErrorExpectation::InvalidField("proof.vk_ref.name")': 2,
    b'FheErrorExpectation::InvalidField("proof.backend")': 4,
    b'FheErrorExpectation::InvalidField("proof.vk_commitment")': 3,
    b'FheErrorExpectation::InvalidField("proof.envelope_hash")': 1,
    b'FheErrorExpectation::InvalidField("proof")': 1,
    b'FheErrorExpectation::EmptyField("proof.backend")': 1,
}

ORDERED_REGIONS = (
    (
        b"fn run_public_input_shape_replay(",
        b"fn run_published_bounds(",
        (
            b"PublicInputShapeMutation::ExtraRow",
            b"PublicInputShapeMutation::ExtraColumn",
            b"PublicInputShapeMutation::DuplicateStatement",
            b"replay_open.public_inputs = public_inputs",
            b"proof.replace_envelope(&replay_envelope)",
        ),
    ),
    (
        b"fn run_oversized_payloads(",
        b"fn run_canonical_verifier_name(",
        (
            b"let mut oversized_outer",
            b"let mut oversized_circuit",
            b"let mut oversized_schema",
            b"let mut oversized_wrapper",
            b"let mut oversized_native",
        ),
    ),
    (
        b"fn run_commitment_and_envelope_hash(",
        b"fn run_open_verify_envelope_drift(",
        (
            b"vk_commitment = None",
            b"vk_commitment = Some([profile.canonical_commitment; 32])",
            b"envelope_hash = None",
            b"matching envelope hash must be accepted",
            b"vk_commitment = Some([profile.forged_commitment; 32])",
            b"forged_hash[0] ^= 0x01",
        ),
    ),
    (
        b"fn run_open_verify_envelope_drift(",
        b"fn reject_wrong_open_verify_schema(",
        (
            b"let mut malformed",
            b"let mut wrong_backend",
            b"let mut wrong_circuit",
            b"if family == FullBootstrapExecution",
            b"let mut wrong_vk_hash",
            b"let mut wrong_wrapper_version",
            b"let mut wrong_statement",
            b"if family != FullBootstrapExecution",
            b"let mut empty_native",
            b"let mut all_zero_native",
        ),
    ),
    (
        b"fn run_attachment_metadata_drift(",
        b"fn run_fhe_proof_validation_case(",
        (
            b"let mut proof_backend_mismatch",
            b"let mut vk_backend_mismatch",
            b"if family != BootstrapKey",
            b"let mut wrong_vk_ref",
            b"let mut wrong_stark_profile",
            b"let mut unsupported",
            b"if family == BootstrapKey",
            b"let mut empty_backend",
        ),
    ),
)


class GuardFailure(RuntimeError):
    """Raised when the governed source contract drifts."""


def _region(data: bytes, start: bytes, end: bytes) -> bytes:
    start_index = data.find(start)
    end_index = data.find(end, start_index + len(start))
    if start_index < 0 or end_index < 0:
        raise GuardFailure(f"missing governed region: {start!r} .. {end!r}")
    return data[start_index:end_index]


def _assert_ordered(region: bytes, atoms: tuple[bytes, ...]) -> None:
    cursor = 0
    for atom in atoms:
        position = region.find(atom, cursor)
        if position < 0:
            raise GuardFailure(f"missing or reordered governed mutation: {atom!r}")
        cursor = position + len(atom)


def validate_source(data: bytes) -> None:
    lines = data.splitlines(keepends=True)
    if len(lines) > MAXIMUM_LINE_COUNT:
        raise GuardFailure(
            f"Rust line floor regressed: {len(lines)} > {MAXIMUM_LINE_COUNT}"
        )
    if data.count(SUFFIX_MARKER) != 1:
        raise GuardFailure("provenance suffix marker must occur exactly once")
    suffix_index = data.index(SUFFIX_MARKER)
    prefix, suffix = data[:suffix_index], data[suffix_index:]
    if len(suffix.splitlines()) != PROVENANCE_SUFFIX_LINES:
        raise GuardFailure("provenance suffix line count drifted")
    if hashlib.sha256(suffix).hexdigest() != PROVENANCE_SUFFIX_SHA256:
        raise GuardFailure("provenance suffix bytes drifted")

    if prefix.count(b"#[test]\n") != 1:
        raise GuardFailure("FHE matrix must remain one compiled test unit")
    if b"#[rustfmt::skip]" in prefix or b"include!(" in prefix:
        raise GuardFailure("format skipping and body relocation are forbidden")
    if re.search(rb"\b(?:Fn|FnMut|FnOnce)\b|=\s*(?:move\s*)?\|", prefix):
        raise GuardFailure("callbacks and closure bodies are forbidden")
    if re.search(rb"\b(?:Action|Step)\b", prefix):
        raise GuardFailure("generic action/step DSL vocabulary is forbidden")

    ids_region = _region(
        prefix,
        b"const FHE_PROOF_VALIDATION_CASE_IDS:",
        b"const FHE_PROOF_VALIDATION_ROUTES:",
    )
    case_ids = tuple(
        item.decode("ascii")
        for item in re.findall(rb'^\s*"([^"]+)",$', ids_region, re.MULTILINE)
    )
    if case_ids != EXPECTED_CASE_IDS or len(set(case_ids)) != 27:
        raise GuardFailure("historical FHE case IDs drifted, reordered, or duplicated")
    for case_id in EXPECTED_CASE_IDS:
        if f"fn {case_id}(".encode() in prefix:
            raise GuardFailure(f"historical body was not consolidated: {case_id}")

    routes_region = _region(
        prefix,
        b"const FHE_PROOF_VALIDATION_ROUTES:",
        b"struct FheProofProfile",
    )
    routes = tuple(
        (family.decode("ascii"), scenario.decode("ascii"))
        for family, scenario in re.findall(
            rb"^\s*\(([A-Za-z]+), ([A-Za-z]+)\),$", routes_region, re.MULTILINE
        )
    )
    if routes != EXPECTED_ROUTES:
        raise GuardFailure("historical FHE family/scenario routes drifted or reordered")

    profile_matches = re.findall(
        rb"Self::(InputAdmission|PublicKey|BootstrapKey|FullBootstrapExecution) "
        rb"=> FheProofProfile \{(.*?)\n            \},",
        prefix,
        re.DOTALL,
    )
    profiles = {name.decode("ascii"): body for name, body in profile_matches}
    if set(profiles) != set(EXPECTED_PROFILE_ATOMS):
        raise GuardFailure("FHE family profile inventory drifted")
    for family, atoms in EXPECTED_PROFILE_ATOMS.items():
        for atom in atoms:
            if atom not in profiles[family]:
                raise GuardFailure(f"{family} profile lost semantic atom: {atom!r}")

    for atom in SEALED_ATOMS:
        if atom not in prefix:
            raise GuardFailure(f"missing governed semantic atom: {atom!r}")
    for token, count in EXPECTED_ERROR_TOKEN_COUNTS.items():
        if prefix.count(token) != count:
            raise GuardFailure(
                f"error assertion axis drifted for {token!r}: "
                f"expected {count}, found {prefix.count(token)}"
            )
    for start, end, atoms in ORDERED_REGIONS:
        _assert_ordered(_region(prefix, start, end), atoms)
    normalized_prefix = b" ".join(prefix.split())
    if hashlib.sha256(normalized_prefix).hexdigest() != FHE_PREFIX_NORMALIZED_SHA256:
        raise GuardFailure("normalized FHE matrix source seal changed")


def _replace_once(data: bytes, old: bytes, new: bytes) -> bytes:
    if data.count(old) != 1:
        raise GuardFailure(f"mutation target is not unique: {old!r}")
    return data.replace(old, new, 1)


def exercise_mutation_guard(data: bytes) -> int:
    mutations = (
        _replace_once(data, b"fill_byte: 0xA5", b"fill_byte: 0xA6"),
        _replace_once(
            data,
            b"fill_byte: 0xA5,\n                other_statement_seed: 21",
            b"fill_byte: 0xA5,\n                other_statement_seed: 22",
        ),
        _replace_once(data, b"other_statement_seed: 15", b"other_statement_seed: 16"),
        _replace_once(data, b"forged_commitment: 0x27", b"forged_commitment: 0x28"),
        _replace_once(data, EXPECTED_CASE_IDS[0].encode(), b"missing_historical_id"),
        _replace_once(
            data,
            b"(InputAdmission, PublicInputShapeReplay)",
            b"(InputAdmission, PublishedBounds)",
        ),
        _replace_once(data, b"version_drift.version = 2", b"version_drift.version = 3"),
        _replace_once(data, b"sample_hash(99)", b"sample_hash(98)"),
        _replace_once(
            data,
            b"#[test]\n#[allow(clippy::too_many_lines)]\nfn fhe_proof_validation_matrix()",
            b"#[allow(clippy::too_many_lines)]\n#[test]\nfn fhe_proof_validation_matrix()",
        ),
        data[: data.index(SUFFIX_MARKER)]
        + b"let callback = |value| value;\n"
        + data[data.index(SUFFIX_MARKER) :],
        data[: data.index(SUFFIX_MARKER)]
        + (b"\n" * (MAXIMUM_LINE_COUNT - len(data.splitlines()) + 1))
        + data[data.index(SUFFIX_MARKER) :],
        data[:-2] + b" \n",
    )
    for number, mutation in enumerate(mutations, start=1):
        try:
            validate_source(mutation)
        except GuardFailure:
            continue
        raise GuardFailure(f"mutation {number} escaped the source guard")
    return len(mutations)


class SoracloudFheProofValidationMatrixSourceTest(unittest.TestCase):
    """Exercise the source seal and its fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.data = SOURCE.read_bytes()

    def test_source_contract(self) -> None:
        validate_source(self.data)

    def test_mutations_fail_closed(self) -> None:
        self.assertEqual(exercise_mutation_guard(self.data), 12)


if __name__ == "__main__":
    unittest.main()
