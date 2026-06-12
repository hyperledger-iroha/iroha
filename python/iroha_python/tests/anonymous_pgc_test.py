from __future__ import annotations

import json

import pytest

from iroha_python import (
    buildAnonymousPgcAccountCommitmentInstruction,
    buildAnonymousPgcDevProofFixture,
    buildAnonymousPgcKOutOfNProofV1,
    buildAnonymousPgcReceiverSet,
    buildAnonymousPgcTransferInstruction,
    build_anonymous_pgc_dev_proof_fixture,
    build_anonymous_pgc_account_commitment_instruction,
    build_anonymous_pgc_k_out_of_n_proof_v1,
    build_anonymous_pgc_receiver_set,
    build_anonymous_pgc_transfer_instruction,
    decode_privacy_proof_envelope,
    verifyAnonymousPgcKOutOfNProofV1,
    verifyAnonymousPgcDevProofLocally,
    verify_anonymous_pgc_dev_proof_locally,
    verify_anonymous_pgc_k_out_of_n_proof_v1,
)
from iroha_python.verange import build_privacy_proof_envelope


def _payload() -> bytes:
    return b"anonymous-pgc:alice:bob:42"


def _receiver_a() -> dict[str, object]:
    return {
        "accountCommitment": bytes([0x21]) * 32,
        "ciphertextCommitment": bytes([0x31]) * 32,
        "ciphertext": b"ciphertext-for-bob",
    }


def _receiver_b() -> dict[str, object]:
    return {
        "accountCommitment": bytes([0x22]) * 32,
        "ciphertextCommitment": bytes([0x32]) * 32,
        "ciphertext": b"ciphertext-for-carol",
    }


def _base_receiver_set() -> dict[str, object]:
    return {
        "threshold": 1,
        "receivers": [_receiver_a(), _receiver_b()],
    }


def _base_fixture() -> dict[str, object]:
    return {
        "receiverSet": build_anonymous_pgc_receiver_set(_base_receiver_set()),
        "anonymitySetRoot": bytes([0x41]) * 32,
        "payload": _payload(),
        "balanceCommitments": [bytes([0x51]) * 32, bytes([0x52]) * 32],
        "linkTag": bytes([0x61]) * 32,
        "rangeCommitments": [bytes([0x71]) * 32],
        "chainId": "boi-localnet",
        "domainSeparator": "boi:anonymous-pgc:v1",
        "vkHash": bytes([0x55]) * 32,
    }


def test_anonymous_pgc_builders_normalize_receiver_sets_and_dev_fixture() -> None:
    receiver_set = build_anonymous_pgc_receiver_set(_base_receiver_set())

    assert receiver_set["version"] == 1
    assert receiver_set["threshold"] == 1
    assert receiver_set["receiver_count"] == 2
    assert isinstance(receiver_set["receiver_set_commitment"], bytes)
    assert len(receiver_set["receiver_set_commitment"]) == 32
    assert len(receiver_set["receivers"][0]["ciphertext_digest"]) == 32
    assert receiver_set == build_anonymous_pgc_receiver_set(_base_receiver_set())

    fixture = build_anonymous_pgc_dev_proof_fixture(_base_fixture())

    assert fixture["kind"] == "anonymous-pgc-dev-fixture-v1"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert isinstance(fixture["proof_bytes"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    assert decoded["backend"] == "Stark"
    assert decoded["circuit_id"] == (
        "stark/fri/sha256-goldilocks:anonymous_pgc_k_out_of_n_v1"
    )
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    assert public_inputs["receiver_threshold"] == 1
    assert public_inputs["receiver_count"] == 2
    assert public_inputs["receiver_ciphertext_commitments"] == [
        bytes([0x31] * 32).hex(),
        bytes([0x32] * 32).hex(),
    ]
    assert public_inputs["receiver_set_commitment"] == receiver_set[
        "receiver_set_commitment"
    ].hex()

    verified = verify_anonymous_pgc_dev_proof_locally(
        {
            "envelope": fixture["envelope"],
            "receiverSet": fixture["receiver_set"],
            "payload": _payload(),
            "anonymitySetRoot": bytes([0x41]) * 32,
            "balanceCommitments": [bytes([0x51]) * 32, bytes([0x52]) * 32],
            "linkTag": bytes([0x61]) * 32,
            "rangeCommitments": [bytes([0x71]) * 32],
            "chainId": "boi-localnet",
            "domainSeparator": "boi:anonymous-pgc:v1",
        }
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["receiver_count"] == 2
    assert verified["receiver_threshold"] == 1
    assert verified["public_inputs"] == fixture["public_inputs"]
    assert verified["public_input_bytes"] == len(fixture["public_input_bytes"])
    assert verified["proof_bytes"] == len(fixture["proof_bytes"])


def _production_transfer_input(envelope: bytes) -> dict[str, object]:
    fixture = _base_fixture()
    return {
        "proofEnvelope": envelope,
        "receiverSet": fixture["receiverSet"],
        "payload": fixture["payload"],
        "anonymitySetRoot": fixture["anonymitySetRoot"],
        "balanceCommitments": fixture["balanceCommitments"],
        "linkTag": fixture["linkTag"],
        "rangeCommitments": fixture["rangeCommitments"],
        "chainId": fixture["chainId"],
        "domainSeparator": fixture["domainSeparator"],
    }


def test_anonymous_pgc_production_proof_and_instruction_builders_roundtrip() -> None:
    proof_bytes = b"external-anonymous-pgc-proof-v1"
    envelope = build_anonymous_pgc_k_out_of_n_proof_v1(
        {**_base_fixture(), "proofBytes": proof_bytes}
    )

    decoded = decode_privacy_proof_envelope(envelope)
    assert decoded["backend"] == "Stark"
    assert decoded["proof_bytes"] == proof_bytes

    verified = verify_anonymous_pgc_k_out_of_n_proof_v1(
        _production_transfer_input(envelope)
    )
    assert verified["ok"] is True
    assert verified["production"] is True
    assert verified["kind"] == "anonymous-pgc-k-out-of-n-v1"
    assert verified["receiver_count"] == 2
    assert verified["receiver_threshold"] == 1

    account_instruction = build_anonymous_pgc_account_commitment_instruction(
        {
            "accountCommitment": bytes([0x21]) * 32,
            "anonymitySetRoot": bytes([0x41]) * 32,
            "chainId": "boi-localnet",
            "domainSeparator": "boi:anonymous-pgc:v1",
        }
    )
    assert account_instruction["kind"] == "zk::RegisterAnonymousPgcAccountCommitment"
    assert len(account_instruction["instruction_digest"]) == 64

    transfer_instruction = build_anonymous_pgc_transfer_instruction(
        _production_transfer_input(envelope)
    )
    assert transfer_instruction["kind"] == "zk::SubmitAnonymousPgcTransfer"
    assert transfer_instruction["proof_envelope"] == envelope
    assert transfer_instruction["receiver_count"] == 2
    assert len(transfer_instruction["instruction_digest"]) == 64


def test_anonymous_pgc_package_root_exports_production_entrypoint_aliases() -> None:
    envelope = buildAnonymousPgcKOutOfNProofV1(
        {**_base_fixture(), "proofBytes": b"external-anonymous-pgc-proof-v1"}
    )
    verified = verifyAnonymousPgcKOutOfNProofV1(_production_transfer_input(envelope))
    account_instruction = buildAnonymousPgcAccountCommitmentInstruction(
        {
            "accountCommitment": bytes([0x21]) * 32,
            "anonymitySetRoot": bytes([0x41]) * 32,
            "chainId": "boi-localnet",
            "domainSeparator": "boi:anonymous-pgc:v1",
        }
    )
    transfer_instruction = buildAnonymousPgcTransferInstruction(
        _production_transfer_input(envelope)
    )

    assert verified["production"] is True
    assert account_instruction["kind"] == "zk::RegisterAnonymousPgcAccountCommitment"
    assert transfer_instruction["kind"] == "zk::SubmitAnonymousPgcTransfer"


def test_anonymous_pgc_production_helpers_reject_dev_fixture_bytes() -> None:
    fixture = build_anonymous_pgc_dev_proof_fixture(_base_fixture())

    with pytest.raises(ValueError, match="dev fixture"):
        build_anonymous_pgc_k_out_of_n_proof_v1(
            {**_base_fixture(), "proofBytes": fixture["proof_bytes"]}
        )

    with pytest.raises(ValueError, match="dev fixture"):
        verify_anonymous_pgc_k_out_of_n_proof_v1(
            _production_transfer_input(fixture["envelope"])
        )

    with pytest.raises(ValueError, match="dev fixture"):
        build_anonymous_pgc_transfer_instruction(
            _production_transfer_input(fixture["envelope"])
        )


def test_anonymous_pgc_public_helpers_reject_non_plain_mapping_inputs() -> None:
    class AnonymousPgcDict(dict):
        pass

    account_options: dict[str, object] = {
        "accountCommitment": bytes([0x21]) * 32,
        "anonymitySetRoot": bytes([0x41]) * 32,
        "chainId": "boi-localnet",
        "domainSeparator": "boi:anonymous-pgc:v1",
    }
    proof_options = {**_base_fixture(), "proofBytes": b"external-anonymous-pgc-proof-v1"}

    with pytest.raises(TypeError, match="anonymousPgcReceiverSet"):
        build_anonymous_pgc_receiver_set(AnonymousPgcDict(_base_receiver_set()))
    with pytest.raises(TypeError, match=r"anonymousPgcReceiverSet\.receivers\[1\]"):
        build_anonymous_pgc_receiver_set(
            {
                "threshold": 1,
                "receivers": [_receiver_a(), AnonymousPgcDict(_receiver_b())],
            }
        )
    with pytest.raises(TypeError, match="anonymousPgcDevProofFixture"):
        build_anonymous_pgc_dev_proof_fixture(AnonymousPgcDict(_base_fixture()))
    with pytest.raises(TypeError, match=r"anonymousPgcDevProofFixture\.receiverSet"):
        build_anonymous_pgc_dev_proof_fixture(
            {
                **_base_fixture(),
                "receiverSet": AnonymousPgcDict(
                    build_anonymous_pgc_receiver_set(_base_receiver_set())
                ),
            }
        )
    with pytest.raises(TypeError, match=r"anonymousPgcKOutOfNProofV1\.balanceCommitments\[0\]"):
        build_anonymous_pgc_k_out_of_n_proof_v1(
            {
                **proof_options,
                "balanceCommitments": [
                    AnonymousPgcDict({"commitment": bytes([0x51]) * 32}),
                ],
            }
        )

    for helper in (
        build_anonymous_pgc_k_out_of_n_proof_v1,
        buildAnonymousPgcKOutOfNProofV1,
    ):
        with pytest.raises(TypeError, match="anonymousPgcKOutOfNProofV1"):
            helper(AnonymousPgcDict(proof_options))

    for helper in (
        build_anonymous_pgc_account_commitment_instruction,
        buildAnonymousPgcAccountCommitmentInstruction,
    ):
        with pytest.raises(TypeError, match="anonymousPgcAccountCommitmentInstruction"):
            helper(AnonymousPgcDict(account_options))

    envelope = build_anonymous_pgc_k_out_of_n_proof_v1(proof_options)
    raw_verified = verify_anonymous_pgc_k_out_of_n_proof_v1(envelope)
    assert raw_verified["ok"] is True
    transfer_options = _production_transfer_input(envelope)
    for helper in (
        verify_anonymous_pgc_k_out_of_n_proof_v1,
        verifyAnonymousPgcKOutOfNProofV1,
    ):
        with pytest.raises(TypeError, match="anonymousPgcKOutOfNProofV1Verification"):
            helper(AnonymousPgcDict(transfer_options))

    for helper in (
        build_anonymous_pgc_transfer_instruction,
        buildAnonymousPgcTransferInstruction,
    ):
        with pytest.raises(TypeError, match="anonymousPgcTransferInstruction"):
            helper(AnonymousPgcDict(transfer_options))

    fixture = build_anonymous_pgc_dev_proof_fixture(_base_fixture())
    local_verified = verify_anonymous_pgc_dev_proof_locally(fixture["envelope"])
    assert local_verified["ok"] is True
    local_options = {
        **transfer_options,
        "envelope": fixture["envelope"],
    }
    for helper in (
        verify_anonymous_pgc_dev_proof_locally,
        verifyAnonymousPgcDevProofLocally,
    ):
        with pytest.raises(TypeError, match="anonymousPgcDevProofLocalVerification"):
            helper(AnonymousPgcDict(local_options))


def test_anonymous_pgc_package_root_exports_catalog_entrypoint_aliases() -> None:
    receiver_set = buildAnonymousPgcReceiverSet(_base_receiver_set())
    fixture = buildAnonymousPgcDevProofFixture({**_base_fixture(), "receiverSet": receiver_set})
    verified = verifyAnonymousPgcDevProofLocally(
        {
            "envelope": fixture["envelope"],
            "receiverSet": receiver_set,
            "payload": _payload(),
            "anonymitySetRoot": bytes([0x41]) * 32,
            "balanceCommitments": [bytes([0x51]) * 32, bytes([0x52]) * 32],
            "linkTag": bytes([0x61]) * 32,
            "rangeCommitments": [bytes([0x71]) * 32],
            "chainId": "boi-localnet",
            "domainSeparator": "boi:anonymous-pgc:v1",
        }
    )

    assert verified["ok"] is True
    assert verified["kind"] == "anonymous-pgc-dev-fixture-v1"


@pytest.mark.parametrize(
    "patch",
    [
        {"threshold": 0},
        {"threshold": 3},
        {"threshold": 1, "k": 1},
        {"receivers": []},
        {"receivers": [{**_receiver_a(), "accountCommitment": bytes(32)}, _receiver_b()]},
        {
            "receivers": [
                _receiver_a(),
                {**_receiver_b(), "accountCommitment": bytes([0x21]) * 32},
            ],
        },
        {
            "receivers": [
                _receiver_a(),
                {**_receiver_b(), "ciphertextCommitment": bytes([0x31]) * 32},
            ],
        },
        {"receivers": [{"accountCommitment": bytes([0x23]) * 32}, _receiver_b()]},
        {
            "receivers": [
                {
                    **_receiver_a(),
                    "ciphertext": b"ciphertext",
                    "ciphertextDigest": bytes([0xEE]) * 32,
                },
                _receiver_b(),
            ],
        },
        {"receivers": [{**_receiver_a(), "unexpected": "field"}, _receiver_b()]},
    ],
)
def test_anonymous_pgc_receiver_set_rejects_malformed_inputs(
    patch: dict[str, object],
) -> None:
    receiver_set = _base_receiver_set()
    receiver_set.update(patch)

    with pytest.raises((TypeError, ValueError), match="anonymousPgcReceiverSet"):
        build_anonymous_pgc_receiver_set(receiver_set)


@pytest.mark.parametrize(
    "patch",
    [
        {
            "receiverSet": {
                **build_anonymous_pgc_receiver_set(_base_receiver_set()),
                "receiver_set_commitment": bytes([0xAA]) * 32,
            }
        },
        {"anonymitySetRoot": bytes(32)},
        {"payload": b"payload", "txDigest": bytes([0xEE]) * 32},
        {"balanceCommitments": [bytes([0x51]) * 32, bytes([0x51]) * 32]},
        {"rangeCommitments": []},
        {"linkTag": bytes(32)},
        {"chainId": " "},
        {"chain_id": "boi-localnet"},
        {"vkHash": bytes(32)},
        {"backend": "groth16"},
        {"circuitId": "other_anonymous_pgc_v1"},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"maxProofBytes": 4},
    ],
)
def test_anonymous_pgc_dev_fixture_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    fixture_input = _base_fixture()
    fixture_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="anonymousPgcDevProofFixture|privacyProofEnvelope",
    ):
        build_anonymous_pgc_dev_proof_fixture(fixture_input)


def test_anonymous_pgc_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = _base_fixture()
    fixture = build_anonymous_pgc_dev_proof_fixture(fixture_input)
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(public_inputs, indent=2).encode("utf-8")
    duplicate_receiver_inputs = json.dumps(
        {
            **public_inputs,
            "receiver_ciphertext_commitments": [
                public_inputs["receiver_ciphertext_commitments"][0],
                public_inputs["receiver_ciphertext_commitments"][0],
            ],
        },
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return build_privacy_proof_envelope(
            {
                "backend": patch.get("backend", "stark/fri/sha256-goldilocks"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0x55]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            }
        )

    swapped_receiver_set = build_anonymous_pgc_receiver_set(
        {"threshold": 1, "receivers": [_receiver_b(), _receiver_a()]}
    )
    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary"), "payload": _payload()},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof)), "payload": _payload()},
        {"envelope": fixture["envelope"], "payload": b"substituted-payload"},
        {"envelope": fixture["envelope"], "receiverSet": swapped_receiver_set},
        {"envelope": fixture["envelope"], "chainId": "wrong-chain"},
        {"envelope": rebuild(backend="groth16"), "payload": _payload()},
        {"envelope": rebuild(circuitId="other_anonymous_pgc_v1"), "payload": _payload()},
        {"envelope": rebuild(vkHash=bytes([0x56]) * 32), "payload": _payload()},
        {"envelope": rebuild(publicInputs=noncanonical_inputs), "payload": _payload()},
        {
            "envelope": rebuild(publicInputs=duplicate_receiver_inputs),
            "payload": _payload(),
        },
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="anonymousPgcDevProofLocalVerification|privacyProofEnvelope",
        ):
            verify_anonymous_pgc_dev_proof_locally(case)
