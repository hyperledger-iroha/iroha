from __future__ import annotations

import json

import pytest

from iroha_python import (
    buildSisHintsCredentialCommitments,
    buildSisHintsCredentialDevProofFixture,
    buildSisHintsCredentialEnvelope,
    build_sis_hints_credential_commitments,
    build_sis_hints_credential_dev_proof_fixture,
    build_sis_hints_credential_envelope,
    verifySisHintsCredentialProofLocally,
    verify_sis_hints_credential_proof_locally,
)
from iroha_python.verange import (
    _build_privacy_proof_envelope_internal,
    _decode_privacy_proof_envelope_internal,
    decode_privacy_proof_envelope,
)


def _issuer(issuer: str = "boi-issuer-set") -> dict[str, object]:
    return {"issuer": issuer, "commitment_scheme": "sis-hints-v0"}


def _credential(nonce: str = "presentation-1") -> dict[str, object]:
    return {
        "credential_type": "pq-wallet-eligibility",
        "attributes": ["resident", "institution"],
        "nonce": nonce,
    }


def _showing_policy(verifier: str = "boi-wallet-enrollment") -> dict[str, object]:
    return {"verifier": verifier, "accepted_attributes": ["resident"]}


def _parameters(scheme: str = "sis-hints-anoncred-v0") -> dict[str, object]:
    return {"scheme": scheme, "q_bits": 64, "module_rank": 8}


def _base() -> dict[str, object]:
    return {
        "issuerJson": _issuer(),
        "credentialJson": _credential(),
        "showingPolicyJson": _showing_policy(),
        "parametersJson": _parameters(),
        "domainSeparator": "boi:sis-hints:pilot:v0",
    }


def test_sis_hints_builders_normalize_commitments_and_envelopes() -> None:
    base = _base()
    commitments = build_sis_hints_credential_commitments(base)

    assert commitments["version"] == 1
    assert len(commitments["issuer_commitment"]) == 32
    assert len(commitments["parameter_hash"]) == 32
    assert commitments["domain_separator"] == "boi:sis-hints:pilot:v0"
    assert commitments["commitment_kinds"]["credential_commitment"] == (
        "dev-sha256-credential-digest"
    )
    assert commitments["commitment_kinds"]["parameter_hash"] == (
        "dev-sha256-parameter-hash"
    )

    prepared = build_sis_hints_credential_envelope(
        {
            **base,
            "vkHash": bytes([0xBB]) * 32,
            "proofBytes": b"prepared-sis-hints-proof",
        }
    )
    with pytest.raises(ValueError, match="unsupported tag"):
        decode_privacy_proof_envelope(prepared)
    decoded_prepared = _decode_privacy_proof_envelope_internal(
        prepared,
        allow_unsupported_backend=True,
    )
    assert decoded_prepared["backend"] == "Unsupported"
    assert decoded_prepared["circuit_id"] == (
        "lattice/sis-hints-anoncred-v0:sis_hints_anoncred_pq_v0"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["issuer_commitment"] == commitments[
        "issuer_commitment"
    ].hex()
    assert prepared_inputs["credential_commitment"] == commitments[
        "credential_commitment"
    ].hex()
    assert prepared_inputs["showing_policy_hash"] == commitments[
        "showing_policy_hash"
    ].hex()
    assert prepared_inputs["parameter_hash"] == commitments["parameter_hash"].hex()

    fixture = build_sis_hints_credential_dev_proof_fixture(
        {**base, "vkHash": bytes([0xBB]) * 32}
    )
    assert fixture["kind"] == "sis-hints-dev-fixture-v0"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    verified = verify_sis_hints_credential_proof_locally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["parameter_hash"] == commitments["parameter_hash"].hex()
    assert verified["public_inputs"] == fixture["public_inputs"]


def test_sis_hints_package_root_exports_catalog_entrypoint_aliases() -> None:
    base = _base()
    commitments = buildSisHintsCredentialCommitments(base)
    prepared = buildSisHintsCredentialEnvelope(
        {
            **base,
            "vkHash": bytes([0xBB]) * 32,
            "proofBytes": b"prepared-sis-hints-proof",
        }
    )
    assert _decode_privacy_proof_envelope_internal(
        prepared,
        allow_unsupported_backend=True,
    )["proof_bytes"] == (
        b"prepared-sis-hints-proof"
    )

    fixture = buildSisHintsCredentialDevProofFixture(
        {**base, "vkHash": bytes([0xBB]) * 32}
    )
    verified = verifySisHintsCredentialProofLocally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["parameter_hash"] == commitments["parameter_hash"].hex()


@pytest.mark.parametrize(
    "input_value",
    [
        {**_base(), "issuerCommitment": bytes([0xEE]) * 32},
        {**_base(), "credentialCommitment": bytes([0xEE]) * 32},
        {**_base(), "showingPolicyHash": bytes([0xEE]) * 32},
        {**_base(), "parameterHash": bytes([0xEE]) * 32},
        {
            "credentialJson": _credential(),
            "showingPolicyJson": _showing_policy(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:sis-hints:pilot:v0",
        },
        {
            "issuerJson": _issuer(),
            "showingPolicyJson": _showing_policy(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:sis-hints:pilot:v0",
        },
        {
            "issuerJson": _issuer(),
            "credentialJson": _credential(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:sis-hints:pilot:v0",
        },
        {
            "issuerJson": _issuer(),
            "credentialJson": _credential(),
            "showingPolicyJson": _showing_policy(),
            "domainSeparator": "boi:sis-hints:pilot:v0",
        },
        {**_base(), "domainSeparator": " "},
        {**_base(), "issuerCommitment": bytes(32)},
        {**_base(), "version": 2},
        {**_base(), "domain_separator": "boi:sis-hints:pilot:v0"},
        {
            "issuerBytes": b"issuer-material",
            "credentialJson": _credential(),
            "showingPolicyJson": _showing_policy(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:sis-hints:pilot:v0",
            "maxIssuerBytes": 4,
        },
        {**_base(), "__proto__": {"polluted": True}},
    ],
)
def test_sis_hints_commitments_reject_malformed_inputs(
    input_value: dict[str, object],
) -> None:
    with pytest.raises((TypeError, ValueError), match="sisHintsCredentialCommitments"):
        build_sis_hints_credential_commitments(input_value)


@pytest.mark.parametrize(
    "patch",
    [
        {"proofBytes": b""},
        {"vkHash": bytes(32)},
        {"backend": "stark/fri/sha256-goldilocks"},
        {"circuitId": "lattice/sis-hints-anoncred-v0:wrong"},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"maxProofBytes": 4},
        {"maxPublicInputBytes": 4},
        {"domain_separator": "boi:sis-hints:pilot:v0"},
    ],
)
def test_sis_hints_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope_input = {
        **_base(),
        "vkHash": bytes([0xBB]) * 32,
        "proofBytes": b"prepared-sis-hints-proof",
    }
    envelope_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="sisHintsCredentialEnvelope|privacyProofEnvelope",
    ):
        build_sis_hints_credential_envelope(envelope_input)


def test_sis_hints_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0xBB]) * 32}
    fixture = build_sis_hints_credential_dev_proof_fixture(fixture_input)
    with pytest.raises(ValueError, match="unsupported tag"):
        decode_privacy_proof_envelope(fixture["envelope"])
    decoded = _decode_privacy_proof_envelope_internal(
        fixture["envelope"],
        allow_unsupported_backend=True,
    )
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(public_inputs, indent=2).encode("utf-8")
    zero_issuer_inputs = json.dumps(
        {**public_inputs, "issuer_commitment": bytes(32).hex()},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    alias_collision_inputs = json.dumps(
        {**public_inputs, "issuerCommitment": public_inputs["issuer_commitment"]},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return _build_privacy_proof_envelope_internal(
            {
                "backend": patch.get("backend", "unsupported"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0xBB]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            },
            allow_unsupported_backend=True,
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary")},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof))},
        {"envelope": fixture["envelope"], "issuerJson": _issuer(issuer="other")},
        {"envelope": fixture["envelope"], "credentialJson": _credential(nonce="n-2")},
        {"envelope": fixture["envelope"], "showingPolicyJson": _showing_policy(verifier="other")},
        {"envelope": fixture["envelope"], "parametersJson": _parameters(scheme="other")},
        {"envelope": fixture["envelope"], "domainSeparator": "boi:sis-hints:other:v0"},
        {"envelope": rebuild(backend="stark/fri/sha256-goldilocks")},
        {"envelope": rebuild(circuitId="lattice/sis-hints-anoncred-v0:wrong")},
        {"envelope": rebuild(vkHash=bytes([0xBC]) * 32)},
        {"envelope": rebuild(publicInputs=noncanonical_inputs)},
        {"envelope": rebuild(publicInputs=zero_issuer_inputs)},
        {"envelope": rebuild(publicInputs=alias_collision_inputs)},
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="sisHintsCredentialLocalVerification|privacyProofEnvelope",
        ):
            verify_sis_hints_credential_proof_locally(case)
