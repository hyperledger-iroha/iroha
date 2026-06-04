from __future__ import annotations

import json

import pytest

from iroha_python import (
    buildSilentThresholdCredentialCommitments,
    buildSilentThresholdCredentialDevProofFixture,
    buildSilentThresholdCredentialEnvelope,
    build_silent_threshold_credential_commitments,
    build_silent_threshold_credential_dev_proof_fixture,
    build_silent_threshold_credential_envelope,
    decode_privacy_proof_envelope,
    verifySilentThresholdCredentialProofLocally,
    verify_silent_threshold_credential_proof_locally,
)
from iroha_python.verange import build_privacy_proof_envelope


def _issuer_set(threshold: int = 2) -> dict[str, object]:
    return {
        "version": 3,
        "threshold": threshold,
        "issuers": ["boi-supervisor", "bank-a", "bank-b"],
    }


def _threshold_policy(threshold: int = 2) -> dict[str, object]:
    return {
        "threshold": threshold,
        "issuer_set_version": 3,
        "purpose": "retail-wallet-eligibility",
    }


def _credential_showing(nonce: str = "nonce-42") -> dict[str, object]:
    return {
        "credential_type": "retail-wallet-eligibility",
        "attributes": ["resident", "adult"],
        "presentation_nonce": nonce,
    }


def _verifier_policy(verifier: str = "boi-wallet-enrollment") -> dict[str, object]:
    return {
        "verifier": verifier,
        "accepted_purposes": ["retail-wallet-eligibility"],
    }


def _base() -> dict[str, object]:
    return {
        "issuerSetJson": _issuer_set(),
        "thresholdPolicyJson": _threshold_policy(),
        "credentialShowingJson": _credential_showing(),
        "verifierPolicyJson": _verifier_policy(),
        "domainSeparator": "boi:silent-threshold:pilot:v0",
    }


def test_silent_threshold_builders_normalize_commitments_and_envelopes() -> None:
    base = _base()
    commitments = build_silent_threshold_credential_commitments(base)

    assert commitments["version"] == 1
    assert len(commitments["issuer_set_commitment"]) == 32
    assert len(commitments["showing_nullifier"]) == 32
    assert commitments["domain_separator"] == "boi:silent-threshold:pilot:v0"
    assert commitments["commitment_kinds"]["issuer_set_commitment"] == (
        "dev-sha256-issuer-set-digest"
    )
    assert commitments["commitment_kinds"]["credential_showing_commitment"] == (
        "dev-sha256-credential-showing-digest"
    )

    prepared = build_silent_threshold_credential_envelope(
        {
            **base,
            "vkHash": bytes([0x88]) * 32,
            "proofBytes": b"prepared-silent-threshold-proof",
        }
    )
    decoded_prepared = decode_privacy_proof_envelope(prepared)
    assert decoded_prepared["backend"] == "Stark"
    assert decoded_prepared["circuit_id"] == (
        "stark/fri/sha256-goldilocks:silent_threshold_anoncred_v0"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["issuer_set_commitment"] == commitments[
        "issuer_set_commitment"
    ].hex()
    assert prepared_inputs["threshold_policy_hash"] == commitments[
        "threshold_policy_hash"
    ].hex()
    assert prepared_inputs["credential_showing_commitment"] == commitments[
        "credential_showing_commitment"
    ].hex()
    assert prepared_inputs["showing_nullifier"] == commitments["showing_nullifier"].hex()
    assert prepared_inputs["verifier_policy_hash"] == commitments[
        "verifier_policy_hash"
    ].hex()

    fixture = build_silent_threshold_credential_dev_proof_fixture(
        {**base, "vkHash": bytes([0x88]) * 32}
    )
    assert fixture["kind"] == "silent-threshold-dev-fixture-v0"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    verified = verify_silent_threshold_credential_proof_locally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["showing_nullifier"] == commitments["showing_nullifier"].hex()
    assert verified["public_inputs"] == fixture["public_inputs"]


def test_silent_threshold_package_root_exports_catalog_entrypoint_aliases() -> None:
    base = _base()
    commitments = buildSilentThresholdCredentialCommitments(base)
    prepared = buildSilentThresholdCredentialEnvelope(
        {
            **base,
            "vkHash": bytes([0x88]) * 32,
            "proofBytes": b"prepared-silent-threshold-proof",
        }
    )
    assert decode_privacy_proof_envelope(prepared)["proof_bytes"] == (
        b"prepared-silent-threshold-proof"
    )

    fixture = buildSilentThresholdCredentialDevProofFixture(
        {**base, "vkHash": bytes([0x88]) * 32}
    )
    verified = verifySilentThresholdCredentialProofLocally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["showing_nullifier"] == commitments["showing_nullifier"].hex()


@pytest.mark.parametrize(
    "input_value",
    [
        {**_base(), "issuerSetCommitment": bytes([0xEE]) * 32},
        {**_base(), "thresholdPolicyHash": bytes([0xEE]) * 32},
        {**_base(), "credentialShowingCommitment": bytes([0xEE]) * 32},
        {**_base(), "showingNullifier": bytes([0xEE]) * 32},
        {**_base(), "verifierPolicyHash": bytes([0xEE]) * 32},
        {
            "thresholdPolicyJson": _threshold_policy(),
            "credentialShowingJson": _credential_showing(),
            "verifierPolicyJson": _verifier_policy(),
            "domainSeparator": "boi:silent-threshold:pilot:v0",
        },
        {
            "issuerSetJson": _issuer_set(),
            "credentialShowingJson": _credential_showing(),
            "verifierPolicyJson": _verifier_policy(),
            "domainSeparator": "boi:silent-threshold:pilot:v0",
        },
        {
            "issuerSetJson": _issuer_set(),
            "thresholdPolicyJson": _threshold_policy(),
            "verifierPolicyJson": _verifier_policy(),
            "domainSeparator": "boi:silent-threshold:pilot:v0",
        },
        {
            "issuerSetJson": _issuer_set(),
            "thresholdPolicyJson": _threshold_policy(),
            "credentialShowingJson": _credential_showing(),
            "domainSeparator": "boi:silent-threshold:pilot:v0",
        },
        {**_base(), "domainSeparator": " "},
        {**_base(), "issuerSetCommitment": bytes(32)},
        {**_base(), "version": 2},
        {**_base(), "domain_separator": "boi:silent-threshold:pilot:v0"},
        {
            "issuerSetBytes": b"issuer-set",
            "thresholdPolicyJson": _threshold_policy(),
            "credentialShowingJson": _credential_showing(),
            "verifierPolicyJson": _verifier_policy(),
            "domainSeparator": "boi:silent-threshold:pilot:v0",
            "maxIssuerSetBytes": 4,
        },
        {**_base(), "__proto__": {"polluted": True}},
    ],
)
def test_silent_threshold_commitments_reject_malformed_inputs(
    input_value: dict[str, object],
) -> None:
    with pytest.raises(
        (TypeError, ValueError),
        match="silentThresholdCredentialCommitments",
    ):
        build_silent_threshold_credential_commitments(input_value)


@pytest.mark.parametrize(
    "patch",
    [
        {"proofBytes": b""},
        {"vkHash": bytes(32)},
        {"backend": "groth16"},
        {"circuitId": "stark/fri/sha256-goldilocks:wrong"},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"maxProofBytes": 4},
        {"maxPublicInputBytes": 4},
        {"domain_separator": "boi:silent-threshold:pilot:v0"},
    ],
)
def test_silent_threshold_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope_input = {
        **_base(),
        "vkHash": bytes([0x88]) * 32,
        "proofBytes": b"prepared-silent-threshold-proof",
    }
    envelope_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="silentThresholdCredentialEnvelope|privacyProofEnvelope",
    ):
        build_silent_threshold_credential_envelope(envelope_input)


def test_silent_threshold_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0x88]) * 32}
    fixture = build_silent_threshold_credential_dev_proof_fixture(fixture_input)
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(public_inputs, indent=2).encode("utf-8")
    zero_issuer_inputs = json.dumps(
        {**public_inputs, "issuer_set_commitment": bytes(32).hex()},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    alias_collision_inputs = json.dumps(
        {**public_inputs, "issuerSetCommitment": public_inputs["issuer_set_commitment"]},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return build_privacy_proof_envelope(
            {
                "backend": patch.get("backend", "stark/fri/sha256-goldilocks"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0x88]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            }
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary")},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof))},
        {"envelope": fixture["envelope"], "issuerSetJson": _issuer_set(threshold=1)},
        {
            "envelope": fixture["envelope"],
            "thresholdPolicyJson": _threshold_policy(threshold=1),
        },
        {
            "envelope": fixture["envelope"],
            "credentialShowingJson": _credential_showing(nonce="nonce-43"),
        },
        {"envelope": fixture["envelope"], "showingNullifier": bytes([0x44]) * 32},
        {
            "envelope": fixture["envelope"],
            "verifierPolicyJson": _verifier_policy(verifier="other-verifier"),
        },
        {"envelope": fixture["envelope"], "domainSeparator": "boi:silent-threshold:other:v0"},
        {"envelope": rebuild(backend="groth16")},
        {"envelope": rebuild(circuitId="stark/fri/sha256-goldilocks:wrong")},
        {"envelope": rebuild(vkHash=bytes([0x89]) * 32)},
        {"envelope": rebuild(publicInputs=noncanonical_inputs)},
        {"envelope": rebuild(publicInputs=zero_issuer_inputs)},
        {"envelope": rebuild(publicInputs=alias_collision_inputs)},
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="silentThresholdCredentialLocalVerification|privacyProofEnvelope",
        ):
            verify_silent_threshold_credential_proof_locally(case)
