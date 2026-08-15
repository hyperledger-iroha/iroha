#!/usr/bin/env python3
"""Freeze the shared SoraCloud FHE open-verify admission contract."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_data_model/src/soracloud/fhe.rs"

REVIEWED_BLOCK_DIGESTS = {
    "SoracloudFheOpenVerifyEnvelopeContract": (
        "struct",
        "ad446782240ed6de9cd57167bdb228252a113c00fb782ffbb9234ad6dc1a0786",
    ),
    "validate_soracloud_fhe_open_verify_envelope": (
        "fn",
        "3306b9ebf1f82ba96a89ae5434198d269c3fd57cea6722b0a895a292bfa177b6",
    ),
    "SORACLOUD_FHE_INPUT_ADMISSION_OPEN_VERIFY_CONTRACT": (
        "const",
        "af02ebd7eb35722ac892a4c2e16a5afa1e1ae727e803aa3dbb74f94c1277150c",
    ),
    "SORACLOUD_FHE_PUBLIC_KEY_PROOF_OPEN_VERIFY_CONTRACT": (
        "const",
        "e1ab0f5ce928c00015f89d87834fc499ebf1a5e356b205d94477259757c0a648",
    ),
    "SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_OPEN_VERIFY_CONTRACT": (
        "const",
        "8452f89ae15c8d7bc8500257c55efc0a19f0310e6a29118c429721d5aa291101",
    ),
    "SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_OPEN_VERIFY_CONTRACT": (
        "const",
        "9f1cb9a0b154d2f52598408973a172d142f0c5d3d60013cc1697b86c37b9edd4",
    ),
}

CONTRACT_BINDINGS = {
    "input_admission": (
        'manifest: "soracloud fhe input admission proof"',
        "max_open_verify_bytes: SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES",
        "bounds: soracloud_fhe_input_admission_open_verify_bounds",
        "circuit_id: SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1",
        "public_inputs: SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1",
        "max_native_envelope_bytes: SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES",
    ),
    "public_key_proof": (
        'manifest: "soracloud fhe public-key proof"',
        "max_open_verify_bytes: SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES",
        "bounds: soracloud_fhe_public_key_proof_open_verify_bounds",
        "circuit_id: SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1",
        "public_inputs: SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1",
        "max_native_envelope_bytes: SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES",
    ),
    "bootstrap_key_proof": (
        'manifest: "soracloud fhe bootstrap key proof"',
        "max_open_verify_bytes: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES",
        "bounds: soracloud_fhe_bootstrap_key_proof_open_verify_bounds",
        "circuit_id: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1",
        "public_inputs: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1",
        "max_native_envelope_bytes: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES",
    ),
    "full_bootstrap_execution_proof": (
        'manifest: "soracloud fhe full-bootstrap execution proof"',
        "max_open_verify_bytes: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES",
        "bounds: soracloud_fhe_full_bootstrap_execution_proof_open_verify_bounds",
        "circuit_id: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1",
        "public_inputs: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1",
        "max_native_envelope_bytes: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES",
    ),
}

VALIDATION_SEQUENCE = (
    "proof_bytes.len() > contract.max_open_verify_bytes",
    "norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes)",
    ".validate_with_bounds((contract.bounds)())",
    "envelope.backend != BackendTag::Stark",
    "envelope.circuit_id != contract.circuit_id",
    "envelope.public_inputs != contract.public_inputs",
    "vk_commitment != envelope.vk_hash",
    "norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)",
    "open_proof.version != 1",
    "open_proof.public_inputs != expected_public_inputs",
    "validate_soracloud_fhe_stark_native_envelope_bytes(",
)


def _block(source: str, kind: str, name: str) -> str:
    terminator = r"^\};\n" if kind == "const" else r"^\}\n"
    matches = list(
        re.finditer(
            rf"^{kind} {re.escape(name)}\b.*?{terminator}",
            source,
            re.MULTILINE | re.DOTALL,
        )
    )
    if len(matches) != 1:
        raise AssertionError(f"expected one {kind} {name}, found {len(matches)}")
    return matches[0].group(0)


def _digest(block: str) -> str:
    return hashlib.sha256(re.sub(r"\s+", "", block).encode()).hexdigest()


def validate_source(source: str) -> None:
    blocks = {}
    for name, (kind, expected_digest) in REVIEWED_BLOCK_DIGESTS.items():
        block = _block(source, kind, name)
        blocks[name] = block
        if _digest(block) != expected_digest:
            raise AssertionError(f"reviewed block drifted: {name}")

    shared = blocks["validate_soracloud_fhe_open_verify_envelope"]
    positions = [shared.find(token) for token in VALIDATION_SEQUENCE]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        raise AssertionError("open-verify validation order drifted")

    if source.count("validate_soracloud_fhe_open_verify_envelope(") != 5:
        raise AssertionError("shared validator must have one definition and four calls")

    for suffix, expected_bindings in CONTRACT_BINDINGS.items():
        name = f"SORACLOUD_FHE_{suffix.upper()}_OPEN_VERIFY_CONTRACT"
        block = blocks[name]
        compact_block = re.sub(r"\s+", "", block)
        if source.count(name) != 2:
            raise AssertionError(f"{name} must be declared and consumed exactly once")
        for binding in expected_bindings:
            compact_binding = re.sub(r"\s+", "", binding)
            if compact_block.count(compact_binding) != 1:
                raise AssertionError(f"{name} binding drifted: {binding}")


class SoracloudFheOpenVerifyContractSourceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")

    def test_reviewed_contract_is_exact(self) -> None:
        validate_source(self.source)

    def test_rejects_contract_constant_substitution(self) -> None:
        mutated = self.source.replace(
            "SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES",
            "SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES",
            1,
        )
        with self.assertRaisesRegex(AssertionError, "reviewed block drifted"):
            validate_source(mutated)

    def test_rejects_validation_order_change(self) -> None:
        first, second = VALIDATION_SEQUENCE[:2]
        mutated = self.source.replace(first, "TEMP_VALIDATION_STEP", 1)
        mutated = mutated.replace(second, first, 1).replace("TEMP_VALIDATION_STEP", second, 1)
        with self.assertRaisesRegex(AssertionError, "reviewed block drifted"):
            validate_source(mutated)

    def test_rejects_wrapper_bypass(self) -> None:
        mutated = self.source.replace(
            "validate_soracloud_fhe_open_verify_envelope(",
            "validate_soracloud_fhe_stark_native_envelope_bytes(",
            2,
        )
        with self.assertRaises(AssertionError):
            validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
