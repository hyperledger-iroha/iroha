from __future__ import annotations

import json

import pytest

from iroha_python import (
    buildZkAmsAdmissionBatch,
    buildZkAmsAdmissionBatchProofV0,
    buildZkAmsAdmissionDevProofFixture,
    buildZkAmsAdmissionProofEnvelope,
    build_zk_ams_admission_batch,
    build_zk_ams_admission_batch_proof_v0,
    build_zk_ams_admission_dev_proof_fixture,
    build_zk_ams_admission_proof_envelope,
    decode_privacy_proof_envelope,
    verifyZkAmsAdmissionBatchProofV0,
    verifyZkAmsAdmissionProofLocally,
    verify_zk_ams_admission_batch_proof_v0,
    verify_zk_ams_admission_proof_locally,
)
from iroha_python.verange import build_privacy_proof_envelope


def _base() -> dict[str, object]:
    return {
        "issuerRoot": bytes([0x91]) * 32,
        "admissionNullifiers": [bytes([0xA1]) * 32, bytes([0xA2]) * 32],
        "anonymousAccountCommitments": [
            bytes([0xB1]) * 32,
            bytes([0xB2]) * 32,
        ],
        "recursiveProof": b"zk-ams:recursive-proof:batch-7",
        "domainSeparator": "boi:zk-ams:pilot:v0",
    }


def test_zk_ams_builders_normalize_batches_and_proof_envelopes() -> None:
    base = _base()
    batch = build_zk_ams_admission_batch(base)

    assert batch["version"] == 1
    assert batch["batch_size"] == 2
    assert batch["root_kind"] == "dev-sha256-admission-batch-root"
    assert batch["issuer_root"] == base["issuerRoot"]
    assert len(batch["admission_batch_root"]) == 32

    prepared = build_zk_ams_admission_proof_envelope(
        {
            **base,
            "vkHash": bytes([0x66]) * 32,
            "proofBytes": b"prepared-zk-ams-proof",
        }
    )
    decoded_prepared = decode_privacy_proof_envelope(prepared)
    assert decoded_prepared["backend"] == "Stark"
    assert decoded_prepared["circuit_id"] == (
        "stark/fri/sha256-goldilocks:zk_ams_recursive_admission_v0"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["admission_batch_root"] == batch[
        "admission_batch_root"
    ].hex()
    assert prepared_inputs["domain_separator"] == "boi:zk-ams:pilot:v0"

    production_envelope = build_zk_ams_admission_batch_proof_v0(
        {
            **base,
            "vkHash": bytes([0x66]) * 32,
            "proofBytes": b"production-zk-ams-admission-proof",
        }
    )
    production_verified = verify_zk_ams_admission_batch_proof_v0(
        {"envelope": production_envelope, **base}
    )
    assert production_verified["ok"] is True
    assert production_verified["production"] is True
    assert production_verified["kind"] == "zk-ams-recursive-admission-v0"
    assert production_verified["batch_size"] == 2
    assert production_verified["admission_batch_root"] == batch[
        "admission_batch_root"
    ].hex()

    fixture = build_zk_ams_admission_dev_proof_fixture(
        {**base, "vkHash": bytes([0x66]) * 32}
    )
    assert fixture["kind"] == "zk-ams-dev-fixture-v0"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["batch"]["batch_size"] == 2

    verified = verify_zk_ams_admission_proof_locally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["batch_size"] == 2
    assert verified["admission_batch_root"] == batch["admission_batch_root"].hex()
    assert verified["public_inputs"] == fixture["public_inputs"]


def test_zk_ams_package_root_exports_catalog_entrypoint_aliases() -> None:
    base = _base()
    batch = buildZkAmsAdmissionBatch(base)
    prepared = buildZkAmsAdmissionProofEnvelope(
        {
            **base,
            "vkHash": bytes([0x66]) * 32,
            "proofBytes": b"prepared-zk-ams-proof",
        }
    )
    assert decode_privacy_proof_envelope(prepared)["proof_bytes"] == b"prepared-zk-ams-proof"

    production_envelope = buildZkAmsAdmissionBatchProofV0(
        {
            **base,
            "vkHash": bytes([0x66]) * 32,
            "proofBytes": b"production-zk-ams-admission-proof",
        }
    )
    production_verified = verifyZkAmsAdmissionBatchProofV0(
        {"envelope": production_envelope, **base}
    )
    assert production_verified["production"] is True
    assert production_verified["admission_batch_root"] == batch[
        "admission_batch_root"
    ].hex()

    fixture = buildZkAmsAdmissionDevProofFixture({**base, "vkHash": bytes([0x66]) * 32})
    verified = verifyZkAmsAdmissionProofLocally({"envelope": fixture["envelope"], **base})
    assert verified["ok"] is True
    assert verified["admission_batch_root"] == batch["admission_batch_root"].hex()


@pytest.mark.parametrize(
    "patch",
    [
        {"issuerRoot": bytes(32)},
        {"admissionNullifiers": []},
        {"admissionNullifiers": [bytes([0xA1]) * 32, bytes([0xA1]) * 32]},
        {
            "anonymousAccountCommitments": [
                bytes([0xB1]) * 32,
                bytes([0xB1]) * 32,
            ]
        },
        {
            "anonymousAccountCommitments": [
                bytes([0xA1]) * 32,
                bytes([0xB2]) * 32,
            ]
        },
        {"anonymousAccountCommitments": [bytes([0xB1]) * 32]},
        {"recursiveProofDigest": bytes([0xEE]) * 32},
        {"admissionBatchRoot": bytes([0xDD]) * 32},
        {"maxBatchSize": 1},
        {"maxBatchSize": 4097},
        {"domainSeparator": " "},
        {"version": 2},
        {"issuer_root": bytes([0x91]) * 32},
    ],
)
def test_zk_ams_admission_batch_rejects_malformed_inputs(
    patch: dict[str, object],
) -> None:
    batch_input = _base()
    batch_input.update(patch)

    with pytest.raises((TypeError, ValueError), match="zkAmsAdmissionBatch"):
        build_zk_ams_admission_batch(batch_input)


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
    ],
)
def test_zk_ams_admission_proof_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope_input = {
        **_base(),
        "vkHash": bytes([0x66]) * 32,
        "proofBytes": b"prepared-zk-ams-proof",
    }
    envelope_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="zkAmsAdmissionProofEnvelope|privacyProofEnvelope",
    ):
        build_zk_ams_admission_proof_envelope(envelope_input)


def test_zk_ams_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0x66]) * 32}
    fixture = build_zk_ams_admission_dev_proof_fixture(fixture_input)
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(public_inputs, indent=2).encode("utf-8")
    duplicate_nullifier_inputs = json.dumps(
        {
            **public_inputs,
            "admission_nullifiers": [
                public_inputs["admission_nullifiers"][0],
                public_inputs["admission_nullifiers"][0],
            ],
        },
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    zero_issuer_inputs = json.dumps(
        {**public_inputs, "issuer_root": bytes(32).hex()},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    alias_collision_inputs = json.dumps(
        {**public_inputs, "issuerRoot": public_inputs["issuer_root"]},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return build_privacy_proof_envelope(
            {
                "backend": patch.get("backend", "stark/fri/sha256-goldilocks"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0x66]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            }
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary")},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof))},
        {"envelope": fixture["envelope"], "issuerRoot": bytes([0x92]) * 32},
        {
            "envelope": fixture["envelope"],
            "admissionNullifiers": [bytes([0xA1]) * 32, bytes([0xA3]) * 32],
        },
        {
            "envelope": fixture["envelope"],
            "anonymousAccountCommitments": [
                bytes([0xB1]) * 32,
                bytes([0xB3]) * 32,
            ],
        },
        {"envelope": fixture["envelope"], "recursiveProof": b"substituted-proof"},
        {"envelope": fixture["envelope"], "domainSeparator": "boi:zk-ams:other:v0"},
        {"envelope": rebuild(backend="groth16")},
        {"envelope": rebuild(circuitId="stark/fri/sha256-goldilocks:wrong")},
        {"envelope": rebuild(vkHash=bytes([0x67]) * 32)},
        {"envelope": rebuild(publicInputs=noncanonical_inputs)},
        {"envelope": rebuild(publicInputs=duplicate_nullifier_inputs)},
        {"envelope": rebuild(publicInputs=zero_issuer_inputs)},
        {"envelope": rebuild(publicInputs=alias_collision_inputs)},
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="zkAmsAdmissionLocalVerification|privacyProofEnvelope",
        ):
            verify_zk_ams_admission_proof_locally(case)


def test_zk_ams_production_builder_and_verifier_reject_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0x66]) * 32}
    fixture = build_zk_ams_admission_dev_proof_fixture(fixture_input)

    with pytest.raises(ValueError, match="dev fixture"):
        build_zk_ams_admission_batch_proof_v0(
            {
                **fixture_input,
                "proofBytes": fixture["proof_bytes"],
            }
        )

    with pytest.raises(ValueError, match="dev fixture"):
        verify_zk_ams_admission_batch_proof_v0(
            {"envelope": fixture["envelope"], **_base()}
        )
