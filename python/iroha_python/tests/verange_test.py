from __future__ import annotations

import base64
import hashlib
import json
from array import array

import pytest

from iroha_python import (
    buildRangeCommitment,
    buildVeRangeDevProofFixture,
    build_range_commitment,
    build_verange_dev_proof_fixture,
    build_verange_proof_envelope,
    decode_privacy_proof_envelope,
    verifyVeRangeProofLocally,
    verify_verange_proof_locally,
)
from iroha_python.verange import build_privacy_proof_envelope


def _payload() -> bytes:
    return b"transfer:alice@wonderland:bob@wonderland:42"


def _base_envelope() -> dict[str, object]:
    payload_digest = hashlib.sha256(_payload()).digest()
    return {
        "commitments": [bytes([0x44]) * 32, bytes([0x45]) * 32],
        "bitLength": 64,
        "commitmentScheme": "pedersen-v1",
        "domainSeparator": "boi:amount-range:v1",
        "payloadDigest": payload_digest,
        "vkHash": bytes([0x55]) * 32,
        "proofBytes": b"prepared-verange-proof",
    }


def test_verange_builders_normalize_commitments_and_dev_fixture() -> None:
    payload = _payload()
    payload_digest = hashlib.sha256(payload).digest()
    commitment_a = bytes([0x44]) * 32
    commitment_b = bytes([0x45]) * 32
    vk_hash = bytes([0x55]) * 32

    descriptor = build_range_commitment(
        {
            "commitment": commitment_a,
            "bitLength": 64,
            "aggregationCount": 2,
            "commitmentScheme": "pedersen-v1",
            "domainSeparator": "boi:amount-range:v1",
            "payload": payload,
        }
    )

    assert descriptor == {
        "version": 1,
        "commitment": commitment_a,
        "bit_length": 64,
        "aggregation_count": 2,
        "commitment_scheme": "pedersen-v1",
        "domain_separator": "boi:amount-range:v1",
        "payload_digest": payload_digest,
    }

    encoded = build_verange_proof_envelope(
        {
            "commitments": [commitment_a, commitment_b],
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
            "domainSeparator": "boi:amount-range:v1",
            "payloadDigest": payload_digest,
            "vkHash": vk_hash,
            "proofBytes": b"prepared-verange-proof",
            "aux": b"prepared externally",
            "maxProofBytes": 64,
            "maxPublicInputBytes": 512,
        }
    )
    decoded = decode_privacy_proof_envelope(encoded)

    assert decoded["backend"] == "Stark"
    assert decoded["circuit_id"] == (
        "stark/fri/sha256-goldilocks:verange_transparent_range_v1"
    )
    assert decoded["vk_hash"] == vk_hash
    assert decoded["proof_bytes"] == b"prepared-verange-proof"
    assert decoded["aux"] == b"prepared externally"
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    assert public_inputs == {
        "aggregation_count": 2,
        "commitments": [commitment_a.hex(), commitment_b.hex()],
        "domain_separator": "boi:amount-range:v1",
        "payload_digest": payload_digest.hex(),
        "range_parameters": {
            "bit_length": 64,
            "commitment_scheme": "pedersen-v1",
        },
        "version": 1,
    }

    fixture = build_verange_dev_proof_fixture(
        {
            "commitments": [commitment_a, commitment_b],
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
            "domainSeparator": "boi:amount-range:v1",
            "payload": payload,
            "vkHash": vk_hash,
        }
    )

    assert fixture["kind"] == "verange-dev-fixture-v1"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert isinstance(fixture["proof_bytes"], bytes)

    verified = verify_verange_proof_locally(
        {
            "envelope": fixture["envelope"],
            "payload": payload,
            "commitments": [commitment_a, commitment_b],
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
            "domainSeparator": "boi:amount-range:v1",
        }
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["kind"] == "verange-dev-fixture-v1"
    assert verified["public_input_bytes"] == len(fixture["public_input_bytes"])
    assert verified["proof_bytes"] == len(fixture["proof_bytes"])
    assert verified["public_inputs"] == fixture["public_inputs"]


def test_privacy_proof_envelope_preserves_pending_production_backend_tags() -> None:
    cases = [
        ("halo2-ipa-orchard", "Halo2IpaOrchard"),
        ("halo2/ipa/orchard", "Halo2IpaOrchard"),
        ("orchard", "Halo2IpaOrchard"),
        ("zcash-orchard", "Halo2IpaOrchard"),
        ("groth16-bls12-377", "Groth16Bls12377"),
        ("groth16/bls12-377", "Groth16Bls12377"),
        ("bls12-377", "Groth16Bls12377"),
        ("decaf377", "Groth16Bls12377"),
        ("masp", "Groth16Bls12377"),
        ("penumbra-masp", "Groth16Bls12377"),
        ("halo2/ipa/penumbra", "Groth16Bls12377"),
        ("halo2/ipa/masp", "Groth16Bls12377"),
        ("fcmp-plus-plus-curve-tree", "FcmpPlusPlusCurveTree"),
        ("fcmp++", "FcmpPlusPlusCurveTree"),
        ("monero-fcmp++", "FcmpPlusPlusCurveTree"),
        ("halo2/ipa/monero", "FcmpPlusPlusCurveTree"),
        ("halo2/ipa/curve-tree", "FcmpPlusPlusCurveTree"),
        ("lattice-pcs-sis", "LatticePcsSis"),
        ("jindo-lattice-pcs-zk", "LatticePcsSis"),
        ("jindo-lattice-pcs-zk-v0", "LatticePcsSis"),
        ("miden-stark", "MidenStark"),
        ("stark/fri/miden", "MidenStark"),
        ("aztec-plonkish-private-kernel", "AztecPlonkishPrivateKernel"),
        ("aztec/private-kernel", "AztecPlonkishPrivateKernel"),
        ("pq-masp-stark-fri", "PqMaspStarkFri"),
        ("stark/fri/pq-masp-stark-fri", "PqMaspStarkFri"),
        ("post-quantum-masp", "PqMaspStarkFri"),
        ("anonymous-pgc", "AnonymousPgc"),
        ("anonymous-pgc-k-out-of-n", "AnonymousPgc"),
        ("anonymous-pgc-k-out-of-n-v1", "AnonymousPgc"),
        ("verange", "VeRange"),
        ("verange-transparent-range", "VeRange"),
        ("verange-transparent-range-v1", "VeRange"),
        ("zkat", "ZkAt"),
        ("zkAt policy-private authenticator", "ZkAt"),
        ("zkat-policy-private-auth-v1", "ZkAt"),
        ("recursive-anonymous-admission", "RecursiveAnonymousAdmission"),
        ("recursive-anonymous-admission-v0", "RecursiveAnonymousAdmission"),
        ("zk-ams-recursive-admission-v0", "RecursiveAnonymousAdmission"),
        ("vega-existing-credential-zk", "VegaExistingCredentialZk"),
        ("vega-existing-credential-zk-v0", "VegaExistingCredentialZk"),
        ("silent-threshold-anoncred", "SilentThresholdAnoncred"),
        ("silent-threshold-anoncred-v0", "SilentThresholdAnoncred"),
        ("threshold-anonymous-credentials", "SilentThresholdAnoncred"),
        ("zk-x509", "ZkX509"),
        ("zkvm-x509-identity", "ZkX509"),
        ("zk-x509-onchain-identity-v0", "ZkX509"),
        ("sis-with-hints", "SisWithHints"),
        ("sis-hints-anoncred-pq-v0", "SisWithHints"),
        ("lattice-anonymous-credentials", "SisWithHints"),
    ]

    for backend, expected in cases:
        encoded = build_privacy_proof_envelope(
            {
                "backend": backend,
                "circuitId": f"{backend}:pending-production-shape-v0",
                "vkHash": bytes([0x66] * 32),
                "publicInputs": b"\x01",
                "proofBytes": b"\x02",
                "maxProofBytes": 16,
                "maxPublicInputBytes": 16,
            }
        )

        assert decode_privacy_proof_envelope(encoded)["backend"] == expected


def test_privacy_proof_envelope_rejects_adversarial_backend_alias_splices() -> None:
    base = {
        "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
        "vkHash": bytes([0x55] * 32),
        "publicInputs": b"\x01",
        "proofBytes": b"\x02",
        "maxProofBytes": 16,
        "maxPublicInputBytes": 16,
    }
    for backend in [
        "mock/dev",
        " unsupported",
        "unsupported ",
        " miden-stark",
        "miden-stark ",
        " stark/fri/sha256-goldilocks",
        "stark/fri/sha256-goldilocks ",
        "halo2/ipa/orchard/dev-fixture",
        "stark/fri/miden/claimed-production",
        "anonymous-pgc-k-out-of-n-v1-production",
        "sis-hints-anoncred-pq-v0-devfixture",
        "groth16/bls12-377/../../prod",
        "post-quantum-masp/audit-claimed",
        "halo2\uFF0Fipa",
        "halo2/\u200Bipa",
        "h\u0430lo2/ipa",
        "stark\uFF0Ffri/sha256-goldilocks",
        "stark/fri/\u200Bsha256-goldilocks",
        "st\u0430rk/fri/sha256-goldilocks",
    ]:
        with pytest.raises(ValueError, match="unsupported backend tag"):
            build_privacy_proof_envelope({**base, "backend": backend})


def test_privacy_proof_envelope_rejects_unclean_circuit_ids() -> None:
    base = {
        "backend": "stark/fri/sha256-goldilocks",
        "vkHash": bytes([0x55] * 32),
        "publicInputs": b"\x01",
        "proofBytes": b"\x02",
        "maxProofBytes": 16,
        "maxPublicInputBytes": 16,
    }
    for circuit_id in (" shape", "shape ", "\tshape", "shape\n"):
        with pytest.raises(ValueError, match="privacyProofEnvelope.circuitId"):
            build_privacy_proof_envelope({**base, "circuitId": circuit_id})


@pytest.mark.parametrize("backend", [None, "missing"])
def test_privacy_proof_envelope_requires_explicit_backend(
    backend: object,
) -> None:
    payload = {
        "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
        "vkHash": bytes([0x55] * 32),
        "publicInputs": b"\x01",
        "proofBytes": b"\x02",
        "maxProofBytes": 16,
        "maxPublicInputBytes": 16,
    }
    if backend != "missing":
        payload["backend"] = backend

    with pytest.raises((TypeError, ValueError), match="privacyProofEnvelope.backendTag"):
        build_privacy_proof_envelope(payload)


def test_privacy_proof_envelope_decodes_clean_base64_byte_strings() -> None:
    vk_hash = bytes([0x55] * 32)
    encoded = build_privacy_proof_envelope(
        {
            "backend": "stark/fri/sha256-goldilocks",
            "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            "vkHash": vk_hash.hex(),
            "publicInputs": "0102",
            "proofBytes": base64.b64encode(b"stark-proof").decode("ascii"),
            "aux": base64.b64encode(b"{}").decode("ascii"),
            "max_proof_bytes": 64,
            "max_public_input_bytes": 16,
        }
    )
    decoded = decode_privacy_proof_envelope(encoded)

    assert decoded["vk_hash"] == vk_hash
    assert decoded["public_inputs"] == base64.b64decode("0102", validate=True)
    assert decoded["proof_bytes"] == b"stark-proof"
    assert decoded["aux"] == b"{}"

    encoded_base64_hash = build_privacy_proof_envelope(
        {
            "backend": "stark/fri/sha256-goldilocks",
            "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            "vkHash": base64.b64encode(vk_hash).decode("ascii"),
            "publicInputs": b"\x01",
            "proofBytes": b"\x02",
            "maxProofBytes": 16,
            "maxPublicInputBytes": 16,
        }
    )
    assert decode_privacy_proof_envelope(encoded_base64_hash)["vk_hash"] == vk_hash


def test_privacy_proof_envelope_accepts_explicit_numeric_byte_arrays() -> None:
    encoded = build_privacy_proof_envelope(
        {
            "backend": "stark/fri/sha256-goldilocks",
            "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            "vkHash": [0x55] * 32,
            "publicInputs": [0x01, 0x02],
            "proofBytes": [0x03, 0x04],
            "aux": [0x7B, 0x7D],
            "maxProofBytes": "16",
            "maxPublicInputBytes": "16",
        }
    )
    decoded = decode_privacy_proof_envelope(encoded)

    assert decoded["vk_hash"] == bytes([0x55] * 32)
    assert decoded["public_inputs"] == b"\x01\x02"
    assert decoded["proof_bytes"] == b"\x03\x04"
    assert decoded["aux"] == b"{}"


def test_privacy_proof_envelope_accepts_unsigned_byte_memoryviews() -> None:
    encoded = build_privacy_proof_envelope(
        {
            "backend": "stark/fri/sha256-goldilocks",
            "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            "vkHash": memoryview(bytes([0x55] * 32)),
            "publicInputs": memoryview(b"\x01\x02"),
            "proofBytes": memoryview(bytearray([0x03, 0x04])),
            "aux": memoryview(b"{}"),
            "maxProofBytes": 16,
            "maxPublicInputBytes": 16,
        }
    )
    decoded = decode_privacy_proof_envelope(encoded)

    assert decoded["vk_hash"] == bytes([0x55] * 32)
    assert decoded["public_inputs"] == b"\x01\x02"
    assert decoded["proof_bytes"] == b"\x03\x04"
    assert decoded["aux"] == b"{}"


def test_privacy_proof_envelope_rejects_non_plain_mappings() -> None:
    class EnvelopeDict(dict):
        pass

    payload = EnvelopeDict(
        {
            "backend": "stark/fri/sha256-goldilocks",
            "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            "vkHash": bytes([0x55] * 32),
            "publicInputs": b"\x01",
            "proofBytes": b"\x02",
            "maxProofBytes": 16,
            "maxPublicInputBytes": 16,
        }
    )

    with pytest.raises(TypeError, match="privacyProofEnvelope"):
        build_privacy_proof_envelope(payload)


def test_privacy_proof_envelope_rejects_non_string_keys() -> None:
    class AliasKey:
        def __init__(self, text: str) -> None:
            self.text = text

        def __str__(self) -> str:
            return self.text

    payload = {
        "backend": "stark/fri/sha256-goldilocks",
        "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
        "vkHash": bytes([0x55] * 32),
        "publicInputs": b"\x01",
        "proofBytes": b"\x02",
        "maxProofBytes": 16,
        "maxPublicInputBytes": 16,
        AliasKey("backend"): "mock/dev",
    }

    with pytest.raises(TypeError, match="privacyProofEnvelope"):
        build_privacy_proof_envelope(payload)


@pytest.mark.parametrize(
    "patch",
    [
        {"publicInputs": "proof"},
        {"proofBytes": "proof"},
        {"aux": "proof"},
        {"publicInputs": " AQI="},
        {"publicInputs": "AQ I="},
        {"proofBytes": "AQI= "},
        {"aux": "e30=\n"},
        {"vkHash": "!" * 32},
        {"vkHash": [True] * 32},
        {"vkHash": ["1"] * 32},
        {"vkHash": {**{index: True for index in range(32)}, "length": 32}},
        {"vkHash": " " + (bytes([0x55] * 32).hex())},
        {"vkHash": (bytes([0x55] * 32).hex()) + " "},
        {"vkHash": " " + base64.b64encode(bytes([0x55] * 32)).decode("ascii")},
        {"vkHash": base64.b64encode(bytes([0x55] * 32)).decode("ascii") + "\n"},
        {"publicInputs": [True]},
        {"publicInputs": ["1"]},
        {"publicInputs": {0: True, "length": 1}},
        {"proofBytes": [False, True]},
        {"proofBytes": [None]},
        {"proofBytes": {0: False, 1: True, "length": 2}},
        {"aux": [True]},
        {"aux": ["1"]},
        {"aux": {0: True, "length": 1}},
        {"publicInputs": memoryview(array("H", [256]))},
        {"proofBytes": memoryview(array("f", [1.5]))},
        {"aux": memoryview(array("H", [0]))},
        {"proofBytes": memoryview(array("b", [-1]))},
        {"vkHash": memoryview(array("H", [0x5555] * 16))},
        {"maxProofBytes": None},
        {"maxPublicInputBytes": None},
        {"maxProofBytes": "016"},
        {"maxProofBytes": " 16"},
        {"maxProofBytes": "16 "},
        {"maxProofBytes": "16\n"},
        {"maxPublicInputBytes": "016"},
        {"maxPublicInputBytes": " 16"},
        {"maxPublicInputBytes": "16 "},
        {"maxPublicInputBytes": "16\n"},
        {"maxProofBytes": 16, "max_proof_bytes": 16},
        {"maxProofBytes": 16, "max_proof_bytes": 1},
        {"maxPublicInputBytes": 16, "max_public_input_bytes": 16},
        {"maxPublicInputBytes": 16, "max_public_input_bytes": 1},
        {
            "vkHash": (
                "hash:"
                + bytes([0x55] * 32).hex().upper()
                + "#2B05"
            )
        },
    ],
)
def test_privacy_proof_envelope_rejects_text_and_unclean_byte_strings(
    patch: dict[str, object],
) -> None:
    base = {
        "backend": "stark/fri/sha256-goldilocks",
        "circuitId": "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
        "vkHash": bytes([0x55] * 32),
        "publicInputs": b"\x01",
        "proofBytes": b"\x02",
        "maxProofBytes": 16,
        "maxPublicInputBytes": 16,
    }
    with pytest.raises((TypeError, ValueError), match="privacyProofEnvelope"):
        build_privacy_proof_envelope({**base, **patch})


def test_verange_package_root_exports_catalog_entrypoint_aliases() -> None:
    payload = _payload()
    fixture = buildVeRangeDevProofFixture(
        {
            "commitments": [bytes([0x44]) * 32],
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
            "payload": payload,
            "vkHash": bytes([0x55]) * 32,
        }
    )
    commitment = buildRangeCommitment(
        {
            "commitment": bytes([0x44]) * 32,
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
            "payload": payload,
        }
    )
    verified = verifyVeRangeProofLocally(
        {
            "envelope": fixture["envelope"],
            "payload": payload,
            "commitment": commitment["commitment"],
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
        }
    )

    assert verified["ok"] is True
    assert verified["production"] is False


@pytest.mark.parametrize(
    "patch",
    [
        {"commitment": bytes(32)},
        {"bitLength": 0},
        {"bitLength": 257},
        {"aggregationCount": 0},
        {"commitmentScheme": "sha256-dev"},
        {"commitment": bytes([0x44]) * 32, "valueCommitment": bytes([0x45]) * 32},
        {"payload": _payload(), "payloadDigest": bytes([0xEE]) * 32},
        {"payloadDigest": None, "payload": None},
        {"maxPayloadBytes": None, "payload": _payload(), "payloadDigest": None},
    ],
)
def test_verange_commitment_builder_rejects_malformed_inputs(
    patch: dict[str, object],
) -> None:
    base = {
        "commitment": bytes([0x44]) * 32,
        "bitLength": 64,
        "commitmentScheme": "pedersen-v1",
        "domainSeparator": "boi:amount-range:v1",
        "payloadDigest": hashlib.sha256(_payload()).digest(),
    }
    base.update(patch)

    with pytest.raises((TypeError, ValueError), match="rangeCommitment"):
        build_range_commitment(base)


@pytest.mark.parametrize(
    "patch",
    [
        {"commitments": []},
        {"commitments": [bytes([0x44]) * 32, bytes([0x44]) * 32]},
        {"aggregationCount": 1},
        {
            "commitments": [
                bytes([0x44]) * 32,
                {
                    "commitment": bytes([0x45]) * 32,
                    "bitLength": 128,
                    "commitmentScheme": "pedersen-v1",
                    "domainSeparator": "boi:amount-range:v1",
                    "payloadDigest": hashlib.sha256(_payload()).digest(),
                },
            ],
        },
        {
            "commitments": [
                bytes([0x44]) * 32,
                {
                    "commitment": bytes([0x45]) * 32,
                    "bitLength": 64,
                    "commitmentScheme": "pedersen-v1",
                    "domainSeparator": "boi:amount-range:v1",
                    "payloadDigest": bytes([0x66]) * 32,
                },
            ],
        },
        {"backend": "groth16"},
        {"circuitId": "other_range_v1"},
        {"vkHash": bytes(32)},
        {"maxPayloadBytes": None},
        {"proofBytes": b""},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"maxProofBytes": 4},
        {"commitment": bytes([0x44]) * 32},
    ],
)
def test_verange_proof_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope = _base_envelope()
    envelope.update(patch)

    with pytest.raises((TypeError, ValueError), match="veRangeProofEnvelope|privacyProofEnvelope"):
        build_verange_proof_envelope(envelope)


def test_verange_local_verifier_rejects_tampered_dev_fixtures() -> None:
    payload = _payload()
    commitment_a = bytes([0x44]) * 32
    commitment_b = bytes([0x45]) * 32
    vk_hash = bytes([0x55]) * 32
    fixture = build_verange_dev_proof_fixture(
        {
            "commitments": [commitment_a, commitment_b],
            "bitLength": 64,
            "commitmentScheme": "pedersen-v1",
            "domainSeparator": "boi:amount-range:v1",
            "payload": payload,
            "vkHash": vk_hash,
        }
    )
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(
        {
            "version": 1,
            "commitments": [commitment_a.hex(), commitment_b.hex()],
            "range_parameters": {
                "bit_length": 64,
                "commitment_scheme": "pedersen-v1",
            },
            "aggregation_count": 2,
            "domain_separator": "boi:amount-range:v1",
            "payload_digest": hashlib.sha256(payload).hexdigest(),
        },
    ).encode("utf-8")
    duplicate_commitment_inputs = json.dumps(
        {
            "aggregation_count": 2,
            "commitments": [commitment_a.hex(), commitment_a.hex()],
            "domain_separator": "boi:amount-range:v1",
            "payload_digest": hashlib.sha256(payload).hexdigest(),
            "range_parameters": {
                "bit_length": 64,
                "commitment_scheme": "pedersen-v1",
            },
            "version": 1,
        },
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return build_privacy_proof_envelope(
            {
                "backend": patch.get("backend", "stark/fri/sha256-goldilocks"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", vk_hash),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            }
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary"), "payload": payload},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof)), "payload": payload},
        {"envelope": fixture["envelope"], "payload": b"substituted-payload"},
        {
            "envelope": fixture["envelope"],
            "payload": payload,
            "commitments": [commitment_b, commitment_a],
            "bitLength": 64,
        },
        {"envelope": rebuild(backend="groth16"), "payload": payload},
        {"envelope": rebuild(publicInputs=noncanonical_inputs), "payload": payload},
        {
            "envelope": rebuild(publicInputs=duplicate_commitment_inputs),
            "payload": payload,
        },
    ]
    for case in cases:
        with pytest.raises((TypeError, ValueError), match="veRangeProofLocalVerification"):
            verify_verange_proof_locally(case)
