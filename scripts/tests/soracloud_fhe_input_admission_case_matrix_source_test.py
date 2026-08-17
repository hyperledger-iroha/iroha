#!/usr/bin/env python3
"""Protect Soracloud's name-preserving FHE input-admission rejection matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "crates/iroha_core/src/smartcontracts/isi/soracloud.rs"
MAX_SOURCE_LINES = 40_506

REGION_START = """    #[derive(Clone, Copy)]
    enum FheInputAdmissionPayloadShape"""
REGION_END = """    #[cfg(feature = "zk-stark")]
    #[test]
    fn mutate_soracloud_state_rejects_registered_binding_only_fhe_input_admission_proof"""
REGION_HASH = "79725ce363652dc2891ae82f88572123b98101014acb1a4465d5a5389b5dd32a"

TEST_CASES = {
    "mutate_soracloud_state_rejects_fhe_input_admission_proof_without_registered_verifier": (
        ("test",),
        ("MissingVerifier",),
    ),
    "mutate_soracloud_state_rejects_oversized_fhe_input_admission_envelope": (
        ("test",),
        ("OversizedEnvelope",),
    ),
    "mutate_soracloud_state_rejects_registered_fhe_input_admission_wrong_circuit": (
        ('cfg(feature = "zk-stark")', "test"),
        ("RegisteredVerifierWrongCircuit",),
    ),
    "mutate_soracloud_state_rejects_registered_fhe_input_admission_wrong_version": (
        ('cfg(feature = "zk-stark")', "test"),
        ("RegisteredVerifierWrongVersion",),
    ),
    "mutate_soracloud_state_rejects_restored_fhe_input_verifier_metadata_drift": (
        ('cfg(feature = "zk-stark")', "test"),
        ("RestoredVerifierWrongCurve", "RestoredVerifierWrongLength"),
    ),
}

CASE_CONTRACTS = {
    "MissingVerifier": (
        'state_key: "/state/private/input-1"',
        'payload_seed: b"seed-proof-missing-vk"',
        "payload_shape: FheInputAdmissionPayloadShape::Canonical",
        'b"gov-fhe-input-proof"',
        'expectation: "unregistered proof verifier must reject FHE input admission"',
        "error_category: FheInputAdmissionErrorCategory::InvariantViolation",
        'error_fragment: "FHE input admission verifying key not found"',
        'storage_message: "failed admission must not persist FHE input state"',
    ),
    "OversizedEnvelope": (
        'state_key: "/state/private/input-oversized"',
        'payload_seed: b"seed-proof-oversized-envelope"',
        "payload_shape: FheInputAdmissionPayloadShape::Oversized",
        'b"gov-fhe-input-proof-oversized-envelope"',
        'expectation: "oversized FHE input envelopes must fail before verifier lookup"',
        "error_category: FheInputAdmissionErrorCategory::InvalidParameter",
        'error_fragment: "slot count"',
        'storage_message: "oversized FHE input admission must not persist state"',
    ),
    "RegisteredVerifierWrongCircuit": (
        'state_key: "/state/private/input-wrong-circuit"',
        'payload_seed: b"seed-proof-wrong-circuit"',
        'b"gov-fhe-input-proof-wrong-circuit"',
        'expectation: "wrong input-admission circuit must fail closed"',
        'error_fragment: "canonical v1 circuit"',
        'storage_message: "wrong-circuit admission must not persist FHE input state"',
    ),
    "RegisteredVerifierWrongVersion": (
        'state_key: "/state/private/input-wrong-version"',
        'payload_seed: b"seed-proof-wrong-version"',
        'b"gov-fhe-input-proof-wrong-version"',
        'expectation: "wrong input-admission verifier version must fail closed"',
        'error_fragment: "canonical v1 circuit version"',
        'storage_message: "wrong-version admission must not persist FHE input state"',
    ),
    "RestoredVerifierWrongCurve": (
        'state_key: "/state/private/input-wrong-field"',
        'payload_seed: b"seed-proof-wrong-field"',
        "governance_seed: FheInputAdmissionGovernanceSeed::StateKey",
        'error_fragment: "goldilocks STARK field"',
        'storage_message: "metadata-drifted verifier must not persist FHE input state"',
    ),
    "RestoredVerifierWrongLength": (
        'state_key: "/state/private/input-wrong-vk-len"',
        'payload_seed: b"seed-proof-wrong-vk-len"',
        "governance_seed: FheInputAdmissionGovernanceSeed::StateKey",
        'error_fragment: "vk_len mismatch"',
        'storage_message: "metadata-drifted verifier must not persist FHE input state"',
    ),
}

VERIFIER_MUTATION_TOKENS = (
    '.circuit_id = "soracloud_fhe_input_admission_shadow_v1".to_string()',
    "u32::from(SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1) + 1",
    '"test setup must drift the registered verifier record version"',
    '.curve = "bn254".to_string()',
    ".vk_len = u32::try_from(verifier_key.bytes.len())",
    ".saturating_add(1)",
)

RUNNER_TOKENS = (
    "for &case in cases",
    "configure_fhe_input_admission_rejection_verifier",
    "deploy_fhe_job_test_service",
    "sample_fhe_input_admission_proof",
    "sample_fhe_input_admission_binding_air_rejection_proof",
    "isi::MutateSoracloudState",
    "fhe_input_admission_proof: Some(admission_proof.clone())",
    "Some(Hash::new(&payload))",
    "Some(admission_proof)",
    ".expect_err(spec.expectation)",
    "assert_invalid_parameter_contains(err, spec.error_fragment)",
    "assert_invariant_contains(err, spec.error_fragment)",
    "soracloud_service_state_entries",
    ".is_none()",
    "spec.storage_message",
)


class GuardError(AssertionError):
    """Raised when the protected Soracloud case matrix changes."""


def _normalized_hash(source: str) -> str:
    return hashlib.sha256(re.sub(r"\s+", "", source).encode()).hexdigest()


def _region(source: str) -> str:
    if source.count(REGION_START) != 1 or source.count(REGION_END) != 1:
        raise GuardError("case-matrix region markers must occur exactly once")
    start = source.index(REGION_START)
    end = source.index(REGION_END, start)
    return source[start:end]


def _invocation(source: str, test_name: str) -> tuple[tuple[str, ...], tuple[str, ...]]:
    pattern = re.compile(
        rf"fhe_input_admission_rejection_test!\s*\{{\s*"
        rf"(?P<attrs>(?:#\[[^\]\n]+\]\s*)+)"
        rf"fn\s+{re.escape(test_name)}\s*=>\s*\[(?P<cases>.*?)\]\s*\}}",
        re.DOTALL,
    )
    match = pattern.search(source)
    if match is None:
        raise GuardError(f"{test_name}: missing name-preserving macro invocation")
    attributes = tuple(re.findall(r"#\[([^\]\n]+)\]", match.group("attrs")))
    cases = tuple(
        re.findall(r"FheInputAdmissionRejectionCase::([A-Za-z0-9_]+)", match.group("cases"))
    )
    return attributes, cases


def _case_arm(region: str, case: str) -> str:
    marker = f"Self::{case} => FheInputAdmissionRejectionSpec {{"
    if region.count(marker) != 1:
        raise GuardError(f"{case}: expected one typed specification arm")
    start = region.index(marker)
    end_marker = "\n                },"
    end = region.find(end_marker, start)
    if end < 0:
        raise GuardError(f"{case}: unterminated specification arm")
    return region[start : end + len(end_marker)]


def validate_source(source: str) -> None:
    if len(source.splitlines()) > MAX_SOURCE_LINES:
        raise GuardError("soracloud.rs exceeded the frozen source budget")
    region = _region(source)
    for test_name, (expected_attributes, expected_cases) in TEST_CASES.items():
        occurrences = len(re.findall(rf"\b{re.escape(test_name)}\b", source))
        if occurrences != 1:
            raise GuardError(f"{test_name}: expected one source occurrence, found {occurrences}")
        attributes, cases = _invocation(source, test_name)
        if attributes != expected_attributes:
            raise GuardError(
                f"{test_name}: attributes {attributes} != {expected_attributes}"
            )
        if cases != expected_cases:
            raise GuardError(f"{test_name}: cases {cases} != {expected_cases}")
    for case, tokens in CASE_CONTRACTS.items():
        arm = _case_arm(region, case)
        for token in tokens:
            if token not in arm:
                raise GuardError(f"{case}: missing semantic token {token!r}")
    for token in VERIFIER_MUTATION_TOKENS + RUNNER_TOKENS:
        if token not in region:
            raise GuardError(f"case matrix missing semantic token {token!r}")
    observed_hash = _normalized_hash(region)
    if observed_hash != REGION_HASH:
        raise GuardError(f"case-matrix semantic hash changed: {observed_hash}")


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


class SoracloudFheInputAdmissionCaseMatrixSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def test_current_source_preserves_case_matrix(self) -> None:
        validate_source(self.source)

    def test_name_mutation_is_rejected(self) -> None:
        name = next(iter(TEST_CASES))
        mutated = _replace_once(self.source, name, f"{name}_mutated")
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_ordered_attribute_mutation_is_rejected(self) -> None:
        name = "mutate_soracloud_state_rejects_registered_fhe_input_admission_wrong_circuit"
        old = f'#[cfg(feature = "zk-stark")]\n        #[test]\n        fn {name}'
        mutated = _replace_once(self.source, old, old.replace("#[test]", "#[ignore]"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_case_wiring_mutation_is_rejected(self) -> None:
        name = next(iter(TEST_CASES))
        old = f"fn {name} => [\n            FheInputAdmissionRejectionCase::MissingVerifier"
        mutated = _replace_once(self.source, old, old.replace("MissingVerifier", "OversizedEnvelope"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_error_payload_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            'error_fragment: "canonical v1 circuit version"',
            'error_fragment: "canonical v2 circuit version"',
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_adversarial_verifier_mutation_is_rejected(self) -> None:
        old = '.expect("registered verifier")\n                    .curve = "bn254"'
        mutated = _replace_once(self.source, old, old.replace("bn254", "bls12-381"))
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_payload_seed_mutation_is_rejected(self) -> None:
        mutated = _replace_once(
            self.source,
            'payload_seed: b"seed-proof-wrong-vk-len"',
            'payload_seed: b"seed-proof-wrong-vk-len-mutated"',
        )
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_storage_atomicity_mutation_is_rejected(self) -> None:
        old = ".is_none(),\n                \"{}\",\n                spec.storage_message"
        mutated = _replace_once(self.source, old, old.replace(".is_none()", ".is_some()"))
        with self.assertRaises(GuardError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
