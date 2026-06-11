from __future__ import annotations

import json

import pytest

from iroha_python import (
    AccountAddress,
    buildVegaCredentialDevProofFixture,
    buildVegaCredentialPredicateCommitment,
    buildVegaCredentialPredicateProofV0,
    buildVegaCredentialProofEnvelope,
    build_vega_credential_dev_proof_fixture,
    build_vega_credential_predicate_commitment,
    build_vega_credential_predicate_proof_v0,
    build_vega_credential_proof_envelope,
    decode_privacy_proof_envelope,
    verifyVegaCredentialPredicateProofV0,
    verifyVegaCredentialProofLocally,
    verify_vega_credential_predicate_proof_v0,
    verify_vega_credential_proof_locally,
)
from iroha_python.verange import build_privacy_proof_envelope


def _account_id(byte: int = 0x11) -> str:
    return AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([byte] * 32),
    ).to_i105(0x02F1)


def _issuer() -> dict[str, object]:
    return {"did": "did:example:issuer:boi", "key": "issuer-key-1"}


def _predicate(threshold: int = 18) -> dict[str, object]:
    return {"kind": "age_over", "attribute": "age", "threshold": threshold}


def _base() -> dict[str, object]:
    return {
        "issuerJson": _issuer(),
        "predicateJson": _predicate(),
        "credentialSchema": "boi-age-credential-v1",
        "accountId": _account_id(),
        "expirationEpoch": 42,
        "domainSeparator": "boi:vega:pilot:v0",
    }


def test_vega_builders_normalize_predicates_and_proof_envelopes() -> None:
    predicate_input = {
        "predicateJson": _predicate(),
        "credentialSchema": "boi-age-credential-v1",
        "domainSeparator": "boi:vega:pilot:v0",
    }
    predicate_commitment = build_vega_credential_predicate_commitment(predicate_input)

    assert predicate_commitment["version"] == 1
    assert predicate_commitment["credential_schema"] == "boi-age-credential-v1"
    assert predicate_commitment["commitment_kind"] == "dev-sha256-predicate-digest"
    assert isinstance(predicate_commitment["predicate_commitment"], bytes)
    assert len(predicate_commitment["predicate_commitment"]) == 32
    assert isinstance(predicate_commitment["predicate_digest"], bytes)

    prepared = build_vega_credential_proof_envelope(
        {
            **_base(),
            "vkHash": bytes([0x77]) * 32,
            "proofBytes": b"prepared-vega-proof",
        }
    )
    decoded_prepared = decode_privacy_proof_envelope(prepared)
    assert decoded_prepared["backend"] == "Stark"
    assert decoded_prepared["circuit_id"] == (
        "stark/fri/sha256-goldilocks:vega_existing_credential_zk_v0"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["credential_schema"] == "boi-age-credential-v1"
    assert prepared_inputs["predicate_commitment"] == predicate_commitment[
        "predicate_commitment"
    ].hex()

    fixture = build_vega_credential_dev_proof_fixture(
        {**_base(), "vkHash": bytes([0x77]) * 32}
    )
    assert fixture["kind"] == "vega-dev-fixture-v0"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    verified = verify_vega_credential_proof_locally(
        {"envelope": fixture["envelope"], **_base()}
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["credential_schema"] == "boi-age-credential-v1"
    assert verified["expiration_epoch"] == 42
    assert verified["public_inputs"] == fixture["public_inputs"]

    production_proof = build_vega_credential_predicate_proof_v0(
        {
            **_base(),
            "vkHash": bytes([0x77]) * 32,
            "proofBytes": b"production-vega-predicate-proof",
        }
    )
    production_verified = verify_vega_credential_predicate_proof_v0(
        {"envelope": production_proof, **_base()}
    )
    assert production_verified["ok"] is True
    assert production_verified["production"] is True
    assert production_verified["kind"] == "vega-existing-credential-zk-v0"
    assert production_verified["credential_schema"] == "boi-age-credential-v1"
    assert production_verified["expiration_epoch"] == 42


def test_vega_package_root_exports_catalog_entrypoint_aliases() -> None:
    predicate_commitment = buildVegaCredentialPredicateCommitment(
        {
            "predicateJson": _predicate(),
            "credentialSchema": "boi-age-credential-v1",
            "domainSeparator": "boi:vega:pilot:v0",
        }
    )
    assert predicate_commitment["predicate_commitment"]

    prepared = buildVegaCredentialProofEnvelope(
        {
            **_base(),
            "vkHash": bytes([0x77]) * 32,
            "proofBytes": b"prepared-vega-proof",
        }
    )
    assert decode_privacy_proof_envelope(prepared)["proof_bytes"] == b"prepared-vega-proof"

    fixture = buildVegaCredentialDevProofFixture({**_base(), "vkHash": bytes([0x77]) * 32})
    verified = verifyVegaCredentialProofLocally({"envelope": fixture["envelope"], **_base()})
    assert verified["ok"] is True

    production_proof = buildVegaCredentialPredicateProofV0(
        {
            **_base(),
            "vkHash": bytes([0x77]) * 32,
            "proofBytes": b"production-vega-predicate-proof",
        }
    )
    production_verified = verifyVegaCredentialPredicateProofV0(
        {"envelope": production_proof, **_base()}
    )
    assert production_verified["ok"] is True
    assert production_verified["production"] is True
    assert production_verified["kind"] == "vega-existing-credential-zk-v0"


def test_vega_production_builder_and_verifier_reject_dev_fixtures() -> None:
    proof = build_vega_credential_predicate_proof_v0(
        {
            **_base(),
            "vkHash": bytes([0x77]) * 32,
            "proofBytes": b"production-vega-predicate-proof",
        }
    )
    decoded = decode_privacy_proof_envelope(proof)
    assert decoded["backend"] == "Stark"

    verified = verify_vega_credential_predicate_proof_v0(
        {"envelope": proof, **_base()}
    )
    assert verified["ok"] is True
    assert verified["production"] is True

    fixture = build_vega_credential_dev_proof_fixture(
        {**_base(), "vkHash": bytes([0x77]) * 32}
    )
    with pytest.raises(ValueError, match="dev fixture"):
        verify_vega_credential_predicate_proof_v0(
            {"envelope": fixture["envelope"], **_base()}
        )
    with pytest.raises(ValueError, match="dev fixture"):
        build_vega_credential_predicate_proof_v0(
            {
                **_base(),
                "vkHash": bytes([0x77]) * 32,
                "proofBytes": fixture["proof_bytes"],
            }
        )


@pytest.mark.parametrize(
    "input_value",
    [
        {
            "predicateJson": _predicate(),
            "predicateCommitment": bytes([0xEE]) * 32,
            "credentialSchema": "boi-age-credential-v1",
            "domainSeparator": "boi:vega:pilot:v0",
        },
        {"credentialSchema": "boi-age-credential-v1"},
        {
            "predicateCommitment": bytes(32),
            "credentialSchema": "boi-age-credential-v1",
        },
        {"predicateJson": _predicate(), "credentialSchema": " "},
        {"predicateJson": _predicate(), "credentialSchema": "x" * 257},
        {
            "predicateBytes": b"predicate",
            "credentialSchema": "boi-age-credential-v1",
            "maxPredicateBytes": 4,
        },
        {
            "predicateJson": _predicate(),
            "credentialSchema": "boi-age-credential-v1",
            "domainSeparator": "boi:vega:pilot:v0",
            "domain_separator": "boi:vega:pilot:v0",
        },
    ],
)
def test_vega_predicate_commitment_rejects_malformed_inputs(
    input_value: dict[str, object],
) -> None:
    with pytest.raises((TypeError, ValueError), match="vegaCredentialPredicateCommitment"):
        build_vega_credential_predicate_commitment(input_value)


@pytest.mark.parametrize(
    "patch",
    [
        {"issuerCommitment": bytes(32), "issuerJson": None},
        {"subjectBinding": bytes([0x01]) * 32, "accountId": _account_id()},
        {"accountId": "alice@wonderland"},
        {"accountId": "0x" + "11" * 32},
        {"expirationEpoch": -1},
        {"credential_schema": "boi-age-credential-v1"},
        {"vkHash": bytes(32)},
        {"proofBytes": b""},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"backend": "groth16"},
        {"circuitId": "stark/fri/sha256-goldilocks:wrong"},
        {"maxProofBytes": 4},
    ],
)
def test_vega_credential_proof_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    proof_input = {
        **_base(),
        "vkHash": bytes([0x77]) * 32,
        "proofBytes": b"prepared-vega-proof",
    }
    proof_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="vegaCredentialProofEnvelope|privacyProofEnvelope",
    ):
        build_vega_credential_proof_envelope(proof_input)


def test_vega_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0x77]) * 32}
    fixture = build_vega_credential_dev_proof_fixture(fixture_input)
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
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
        {**public_inputs, "credentialSchema": public_inputs["credential_schema"]},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return build_privacy_proof_envelope(
            {
                "backend": patch.get("backend", "stark/fri/sha256-goldilocks"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0x77]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            }
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary")},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof))},
        {"envelope": fixture["envelope"], "issuerJson": {"did": "did:example:issuer:other"}},
        {
            "envelope": fixture["envelope"],
            "predicateJson": _predicate(threshold=21),
        },
        {"envelope": fixture["envelope"], "accountId": _account_id(0x12)},
        {"envelope": fixture["envelope"], "expirationEpoch": 43},
        {"envelope": fixture["envelope"], "credentialSchema": "boi-other-v1"},
        {"envelope": fixture["envelope"], "domainSeparator": "boi:vega:other:v0"},
        {"envelope": rebuild(backend="groth16")},
        {"envelope": rebuild(circuitId="stark/fri/sha256-goldilocks:wrong")},
        {"envelope": rebuild(vkHash=bytes([0x78]) * 32)},
        {"envelope": rebuild(publicInputs=noncanonical_inputs)},
        {"envelope": rebuild(publicInputs=zero_issuer_inputs)},
        {"envelope": rebuild(publicInputs=alias_collision_inputs)},
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="vegaCredentialLocalVerification|privacyProofEnvelope",
        ):
            verify_vega_credential_proof_locally(case)
