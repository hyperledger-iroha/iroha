#!/usr/bin/env python3
"""Seal the callback-free Soracloud FHE job fixture consolidation."""

from __future__ import annotations

import hashlib
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_core/src/smartcontracts/isi/soracloud.rs"
INITIAL_FIXTURE_SOURCE = SOURCE.with_name("soracloud_initial_fixture_tests.rs")
INITIAL_FIXTURE_INCLUDE = '    include!("soracloud_initial_fixture_tests.rs");'
PREIMAGE_BLOB = "3592edfb43e7a452e159367335335b193e3c7e04"
PREIMAGE_SHA256 = "a425fa60502c060eb90a6f1732a7ec9065e063b04ac3ef3f0435216e42bc8e33"
PREIMAGE_LINES = 42_294
MINIMUM_RUST_LINE_REDUCTION = 500
MAXIMUM_SOURCE_LINES = PREIMAGE_LINES - MINIMUM_RUST_LINE_REDUCTION

HELPER_START = "    fn deploy_fhe_job_test_service("
HELPER_END = "    const SORACLOUD_BFV_OPERATION_VECTOR_SET"
HELPER_SHA256 = "906a3cc8ffca6eb145ab140fd694746eabbf8789edb9bbe0fe8af3d1bf407f32"

PROTECTED_FUNCTION_SHA256 = {
    "checkpoint_soracloud_training_job_updates_authoritative_state":
        "c21e68f804ff321a66e0f7c2dca7ad2a7c5f7e1226578a1f7b03f485f0458058",
    "deploy_uploaded_model_service":
        "71dae94d8432ccc7139b7420adb4152da8d427792443a36e39a6340ea2d58430",
    "load_soracloud_fhe_inputs_rejects_bounded_noise_public_key_digest_mismatch":
        "22647f5a5513cdee62fea16c131d73d3292c8d8af0d416fa8869f25d6727c9a5",
    "model_weight_lifecycle_updates_authoritative_registry_state":
        "2eda1344ae4f23d7089f97eb9a10bc7ea0d42ba865bbdbef41303ebde2e92c1e",
    "mutate_soracloud_state_rejects_registered_bounded_noise_binding_only_fhe_input_admission_proof":
        "6c29dacac1dd354a8e113a7c02d8bee1fb5d9bbfccc1078a0ffcf9f56a06c598",
    "mutate_soracloud_state_rejects_registered_binding_only_fhe_input_admission_proof":
        "88c5a81d893273babd9c11dda2d4583b1867cb0857ebc181509f4c14d5b6eacc",
    "mutate_soracloud_state_rejects_malformed_fhe_payload_without_optional_proof":
        "e2ef269a57deb3ef12e96261fea2303e3c809ab3aefe573a3d88149333ba7d00",
    "register_soracloud_model_artifact_records_authoritative_state":
        "fbff67f8c870bee71e1d3788f126e5390bae70f94b0ab28fe97dae439e16605e",
    "retry_soracloud_training_job_records_retry_pending_state":
        "7ad5e6128a2c157752fad2eafc383ccc594647b81b7572e0cf0d2afc2b79c9dc",
    "run_fhe_input_admission_rejection_cases":
        "726620ea2007d19390bc47837c4abb34b8d5687d349197686930880f6c59e2fb",
    "run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_bounded_add_output":
        "36abc51e069f77f1449ad364c0b31769094a1781eb949180b4a38f59ca8db086",
    "run_soracloud_fhe_job_rejects_binding_only_key_proofs_without_bounded_non_add_outputs":
        "539d42d5f94a8972b4236592f114cab239a644993ad933df35b86d7faac888e0",
    "run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_exact_output":
        "c605b7efa5e0384978c9b57df2cb244f92cf47ac7060436bce140eed15f9feb1",
    "run_soracloud_fhe_job_rejects_all_zero_persisted_fhe_input":
        "2379db41aa8f74b2edbe22afe1b650afdd196293dd81b1bb3dc217c2d5e393ce",
    "run_soracloud_fhe_job_rejects_bounded_noise_persisted_fhe_input":
        "5faf459919c195fc4ff076f03a7a9ef4dfe8cdbfa3993f50bcb8a2da9a20d811",
    "run_soracloud_fhe_job_rejects_client_mutated_fhe_input_without_residual_metadata":
        "ce2dda6d0bc30d1d9f1b84e048f7027206e8bd47777d38a33f3bcd9bd5032b2f",
    "run_soracloud_fhe_job_rejects_input_public_key_digest_mismatch":
        "440bc7af46cb40f062c54425f46daacec800e22592a1ee789e54014c1173c82d",
    "run_soracloud_fhe_job_rejects_missing_policy_bound_public_key_proof":
        "172e86780c4192c8abfa6eebd0b4770c0ba3684ad55599c80d4413dd09c21beb",
    "run_soracloud_fhe_job_rejects_oversized_persisted_fhe_input_envelope":
        "23b714e5a97e097a4a055f99d03a674a8c7caedcd87abc805e65c0d3088916d6",
    "run_soracloud_fhe_job_rejects_persisted_fhe_input_without_bound_mode":
        "9bd7419a97a6fa8fbfcdda8b1fc0c83edf7fdaa2e73b6f2d16c68c22db3ce0ea",
    "start_soracloud_training_job_records_authoritative_job_state":
        "0aae121c5e82149eb0140654d23035e7e8571e18e091d718d0c922b7909a5ed2",
}

TEST_NAME_REPLACEMENTS = (
    (
        "soracloud_fhe_public_key_proof_accepts_verified_active_record",
        "soracloud_fhe_public_key_proof_rejects_registered_binding_only_proof",
    ),
    (
        "soracloud_fhe_bootstrap_key_proof_accepts_verified_active_verifier",
        "soracloud_fhe_bootstrap_key_proof_rejects_registered_binding_only_proof",
    ),
    (
        "run_soracloud_fhe_job_records_ciphertext_output_state",
        "run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_exact_output",
    ),
    (
        "run_soracloud_fhe_job_records_bounded_noise_add_output_state",
        "run_soracloud_fhe_job_rejects_binding_only_public_key_proof_without_bounded_add_output",
    ),
    (
        "run_soracloud_fhe_job_records_bounded_noise_non_add_output_state",
        "run_soracloud_fhe_job_rejects_binding_only_key_proofs_without_bounded_non_add_outputs",
    ),
    (
        "mutate_soracloud_state_accepts_registered_fhe_input_admission_proof",
        "mutate_soracloud_state_rejects_registered_binding_only_fhe_input_admission_proof",
    ),
    (
        "mutate_soracloud_state_accepts_registered_bounded_noise_fhe_input_admission_proof",
        "mutate_soracloud_state_rejects_registered_bounded_noise_binding_only_fhe_input_admission_proof",
    ),
)
ADDED_FAIL_CLOSED_TEST_AFTER = "registered_bfv_key_switch_decomposition_chain_uses_target_limb_prefix"
ADDED_FAIL_CLOSED_TEST = (
    ("#[test]",),
    "soracloud_bounded_noise_add_matches_plaintext_slots_and_output_bound",
)

REQUIRED_HELPER_TOKENS = (
    "isi::DeploySoracloudService",
    "record_service_state_entry(",
    "SORA_SERVICE_STATE_ENTRY_VERSION_V1",
    "SoraStateEncryptionV1::FheCiphertext",
    "fhe_public_key_digest: public_key_digest",
    "fhe_residual_multiple_bound: residual_bound",
    "fhe_bound_mode: bound_mode",
    "last_update_sequence,",
    "Hash::new(governance_tag)",
    "sample_governed_fhe_material(",
    "install_governed_fhe_material(",
    "sample_fhe_param_set()",
    "fhe_job_provenance(",
    "full_bootstrap_execution_proofs.to_vec()",
)

FORBIDDEN_HELPER_TOKENS = (
    "Box<dyn Fn",
    "dyn Fn",
    "impl Fn",
    "FnMut",
    "FnOnce",
    "macro_rules!",
    "$body",
    "$setup",
    "enum Step",
    "enum Scenario",
    "run_case(",
)


class GuardError(AssertionError):
    """Raised when the protected source contract drifts."""


def _sha256(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode()
    return hashlib.sha256(data).hexdigest()


def _normalized_hash(source: str) -> str:
    return _sha256(re.sub(r"\s+", " ", source).strip())


def _preimage() -> str:
    result = subprocess.run(
        ["git", "cat-file", "blob", PREIMAGE_BLOB],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    if _sha256(result.stdout) != PREIMAGE_SHA256:
        raise GuardError("Soracloud donor blob digest changed")
    if len(result.stdout.splitlines()) != PREIMAGE_LINES:
        raise GuardError("Soracloud donor blob line count changed")
    return result.stdout.decode()


def _skip_rust_non_code(source: str, index: int) -> int | None:
    if source.startswith("//", index):
        end = source.find("\n", index)
        return len(source) if end < 0 else end
    if source.startswith("/*", index):
        depth = 1
        cursor = index + 2
        while cursor < len(source):
            if source.startswith("/*", cursor):
                depth += 1
                cursor += 2
            elif source.startswith("*/", cursor):
                depth -= 1
                cursor += 2
                if depth == 0:
                    return cursor
            else:
                cursor += 1
        return len(source)
    for prefix in ("br", "r"):
        if source.startswith(prefix, index):
            cursor = index + len(prefix)
            while cursor < len(source) and source[cursor] == "#":
                cursor += 1
            if cursor < len(source) and source[cursor] == '"':
                hashes = cursor - index - len(prefix)
                terminator = '"' + "#" * hashes
                end = source.find(terminator, cursor + 1)
                return len(source) if end < 0 else end + len(terminator)
    if source[index : index + 1] not in {'"', "'"}:
        return None
    quote = source[index]
    cursor = index + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == quote:
            return cursor + 1
        cursor += 1
    return len(source)


def _matching_brace(source: str, opening: int) -> int:
    depth = 1
    cursor = opening + 1
    while cursor < len(source):
        skipped = _skip_rust_non_code(source, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        if source[cursor] == "{":
            depth += 1
        elif source[cursor] == "}":
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise GuardError("unterminated protected Rust function")


def _function(source: str, name: str) -> str:
    matches = list(re.finditer(rf"(?m)^\s*fn\s+{re.escape(name)}\b", source))
    if len(matches) != 1:
        raise GuardError(f"{name}: expected exactly one function")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise GuardError(f"{name}: missing function body")
    return source[matches[0].start() : _matching_brace(source, opening) + 1]


def _test_inventory(source: str) -> tuple[tuple[tuple[str, ...], str], ...]:
    inventory: list[tuple[tuple[str, ...], str]] = []
    attributes: list[str] = []
    for line in source.splitlines():
        stripped = line.strip()
        if stripped.startswith("#["):
            attributes.append(stripped)
            continue
        match = re.match(r"(?:async\s+)?fn\s+([A-Za-z0-9_]+)\b", stripped)
        if match and any(
            attribute == "#[test]" or attribute.startswith("#[tokio::test")
            for attribute in attributes
        ):
            inventory.append((tuple(attributes), match.group(1)))
        if stripped:
            attributes = []
    return tuple(inventory)


def _expanded_test_source(source: str, initial_fixture: str) -> str:
    if source.count(INITIAL_FIXTURE_INCLUDE) != 1:
        raise GuardError("Soracloud initial fixture include changed")
    return source.replace(INITIAL_FIXTURE_INCLUDE, initial_fixture, 1)


def _expected_test_inventory(
    donor: str,
) -> tuple[tuple[tuple[str, ...], str], ...]:
    replacements = dict(TEST_NAME_REPLACEMENTS)
    expected: list[tuple[tuple[str, ...], str]] = []
    for attributes, name in _test_inventory(donor):
        expected.append((attributes, replacements.get(name, name)))
        if name == ADDED_FAIL_CLOSED_TEST_AFTER:
            expected.append(ADDED_FAIL_CLOSED_TEST)
    return tuple(expected)


def validate_source(source: str, donor: str, initial_fixture: str) -> None:
    expanded_source = _expanded_test_source(source, initial_fixture)
    if len(expanded_source.splitlines()) > MAXIMUM_SOURCE_LINES:
        raise GuardError("Soracloud fixture consolidation lost its 500-line ratchet")
    if _test_inventory(expanded_source) != _expected_test_inventory(donor):
        raise GuardError(
            "Soracloud ordered test inventory changed outside the exact "
            "seven retired to eight fail-closed replacements"
        )
    if source.count(HELPER_START) != 1 or source.count(HELPER_END) != 1:
        raise GuardError("Soracloud typed helper corridor markers changed")
    start = source.index(HELPER_START)
    end = source.index(HELPER_END, start)
    helper = source[start:end]
    if _normalized_hash(helper) != HELPER_SHA256:
        raise GuardError("Soracloud typed FHE helper corridor changed")
    for token in REQUIRED_HELPER_TOKENS:
        if token not in helper:
            raise GuardError(f"Soracloud helper lost {token!r}")
    for token in FORBIDDEN_HELPER_TOKENS:
        if token in helper:
            raise GuardError(f"Soracloud helper gained escape hatch {token!r}")
    for name, expected in PROTECTED_FUNCTION_SHA256.items():
        if _normalized_hash(_function(source, name)) != expected:
            raise GuardError(f"{name}: protected operation/assertion sequence changed")


class SoracloudFheJobFixtureSourceTest(unittest.TestCase):
    def test_source_contract(self) -> None:
        validate_source(
            SOURCE.read_text(),
            _preimage(),
            INITIAL_FIXTURE_SOURCE.read_text(),
        )

    def test_mutations_fail_closed(self) -> None:
        source = SOURCE.read_text()
        donor = _preimage()
        initial_fixture = INITIAL_FIXTURE_SOURCE.read_text()
        mutations = (
            source.replace(
                'b"gov-fhe-missing-bound"', 'b"gov-fhe-mutated-bound"', 1
            ),
            source.replace(
                "fhe_bound_mode: bound_mode", "fhe_bound_mode: None", 1
            ),
            source.replace(
                "last_update_sequence,", "last_update_sequence: 0,", 1
            ),
            source.replace(
                "    fn record_fhe_job_test_input(",
                "    fn record_fhe_job_test_input_removed(",
                1,
            ),
            source.replace(
                "run_soracloud_fhe_job_rejects_all_zero_persisted_fhe_input",
                "run_soracloud_fhe_job_accepts_all_zero_persisted_fhe_input",
                1,
            ),
            source.replace(
                "soracloud_fhe_public_key_proof_rejects_registered_binding_only_proof",
                "soracloud_fhe_public_key_proof_accepts_registered_binding_only_proof",
                1,
            ),
            source.replace("#[test]", "#[ignore]", 1),
            source.replace(
                HELPER_END, "    fn callback_escape(_f: impl Fn()) {}\n" + HELPER_END, 1
            ),
            source.replace(
                "bounded-noise FHE inputs must fail closed for the exact evaluator",
                "bounded-noise FHE inputs may pass",
                1,
            ),
            source + "\n" * (MAXIMUM_SOURCE_LINES - len(source.splitlines()) + 1),
        )
        for mutation in mutations:
            with self.subTest(digest=_sha256(mutation)[:12]):
                with self.assertRaises(GuardError):
                    validate_source(mutation, donor, initial_fixture)

        mutated_initial_fixture = initial_fixture.replace(
            "soracloud_provenance_signature_admission_rejects_malformed_ed25519_signature_r",
            "soracloud_provenance_signature_admission_accepts_malformed_ed25519_signature_r",
            1,
        )
        with self.subTest(digest=_sha256(mutated_initial_fixture)[:12]):
            with self.assertRaises(GuardError):
                validate_source(source, donor, mutated_initial_fixture)


if __name__ == "__main__":
    unittest.main()
