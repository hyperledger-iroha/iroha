from __future__ import annotations

import json

import pytest

from iroha_python import (
    AccountAddress,
    buildZkAtAuthenticatorEnvelope,
    buildZkAtDevProofFixture,
    buildZkAtPolicyCommitment,
    build_zkat_authenticator_envelope,
    build_zkat_dev_proof_fixture,
    build_zkat_policy_commitment,
    decode_privacy_proof_envelope,
    verifyZkAtAuthenticatorLocally,
    verify_zkat_authenticator_locally,
)
from iroha_python.verange import build_privacy_proof_envelope


def _account_id() -> str:
    return AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x11] * 32),
    ).to_i105(0x02F1)


def _policy() -> dict[str, object]:
    return {
        "threshold": 2,
        "roles": ["ops", "risk", "treasury"],
        "fallback": {"recovery_after_slots": 1440},
    }


def _payload() -> bytes:
    return b"zkat:transparent-transfer:42"


def _base_fixture() -> dict[str, object]:
    return {
        "policyJson": _policy(),
        "policyEpoch": 7,
        "policySchema": "boi-hidden-threshold-v1",
        "payload": _payload(),
        "accountId": _account_id(),
        "actionClass": "transparent_transfer",
        "domainSeparator": "boi:zkat:v1",
        "vkHash": bytes([0x55]) * 32,
    }


def test_zkat_builders_normalize_policy_commitments_and_envelopes() -> None:
    policy_commitment = build_zkat_policy_commitment(
        {
            "policyJson": _policy(),
            "policyEpoch": 7,
            "domainSeparator": "boi:zkat:v1",
            "policySchema": "boi-hidden-threshold-v1",
        }
    )

    assert policy_commitment["version"] == 1
    assert policy_commitment["commitment_kind"] == "dev-sha256-policy-digest"
    assert isinstance(policy_commitment["policy_commitment"], bytes)
    assert len(policy_commitment["policy_commitment"]) == 32
    assert isinstance(policy_commitment["policy_digest"], bytes)
    assert len(policy_commitment["policy_digest"]) == 32

    prepared = build_zkat_authenticator_envelope(
        {
            "policyCommitment": policy_commitment["policy_commitment"],
            "policyEpoch": 7,
            "payload": _payload(),
            "accountId": _account_id(),
            "actionClass": "transparent_transfer",
            "domainSeparator": "boi:zkat:v1",
            "vkHash": bytes([0x55]) * 32,
            "proofBytes": b"prepared-zkat-proof",
        }
    )
    decoded_prepared = decode_privacy_proof_envelope(prepared)
    assert decoded_prepared["backend"] == "Stark"
    assert decoded_prepared["circuit_id"] == (
        "stark/fri/sha256-goldilocks:zkat_policy_private_auth_v1"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["account_id"] == _account_id()
    assert prepared_inputs["action_class"] == "transparent_transfer"
    assert prepared_inputs["policy_epoch"] == 7
    assert prepared_inputs["policy_commitment"] == policy_commitment[
        "policy_commitment"
    ].hex()

    fixture = build_zkat_dev_proof_fixture(_base_fixture())
    assert fixture["kind"] == "zkat-dev-fixture-v1"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    verified = verify_zkat_authenticator_locally(
        {
            "envelope": fixture["envelope"],
            "policyJson": _policy(),
            "policySchema": "boi-hidden-threshold-v1",
            "payload": _payload(),
            "accountId": _account_id(),
            "actionClass": "transparent_transfer",
            "domainSeparator": "boi:zkat:v1",
            "policyEpoch": 7,
        }
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["account_id"] == _account_id()
    assert verified["action_class"] == "transparent_transfer"
    assert verified["policy_epoch"] == 7
    assert verified["public_inputs"] == fixture["public_inputs"]


def test_zkat_package_root_exports_catalog_entrypoint_aliases() -> None:
    policy_commitment = buildZkAtPolicyCommitment(
        {
            "policyJson": _policy(),
            "policyEpoch": 7,
            "domainSeparator": "boi:zkat:v1",
            "policySchema": "boi-hidden-threshold-v1",
        }
    )
    prepared = buildZkAtAuthenticatorEnvelope(
        {
            "policyCommitment": policy_commitment["policy_commitment"],
            "policyEpoch": 7,
            "payload": _payload(),
            "accountId": _account_id(),
            "actionClass": "transparent_transfer",
            "domainSeparator": "boi:zkat:v1",
            "vkHash": bytes([0x55]) * 32,
            "proofBytes": b"prepared-zkat-proof",
        }
    )
    assert decode_privacy_proof_envelope(prepared)["proof_bytes"] == b"prepared-zkat-proof"

    fixture = buildZkAtDevProofFixture(_base_fixture())
    verified = verifyZkAtAuthenticatorLocally(
        {
            "envelope": fixture["envelope"],
            "policyJson": _policy(),
            "policySchema": "boi-hidden-threshold-v1",
            "payload": _payload(),
            "accountId": _account_id(),
            "actionClass": "transparent_transfer",
            "domainSeparator": "boi:zkat:v1",
            "policyEpoch": 7,
        }
    )
    assert verified["ok"] is True


@pytest.mark.parametrize(
    "input_value",
    [
        {
            "policyJson": _policy(),
            "policyEpoch": 0,
            "domainSeparator": "boi:zkat:v1",
        },
        {
            "policyJson": _policy(),
            "policyCommitment": bytes([0xEE]) * 32,
            "policyEpoch": 7,
            "domainSeparator": "boi:zkat:v1",
        },
        {"policyEpoch": 7},
        {
            "policyCommitment": bytes(32),
            "policyEpoch": 7,
            "domainSeparator": "boi:zkat:v1",
        },
        {
            "policyJson": _policy(),
            "policyEpoch": 7,
            "policy_epoch": 7,
            "domainSeparator": "boi:zkat:v1",
        },
        {
            "policyJson": _policy(),
            "policyEpoch": 7,
            "domainSeparator": " ",
        },
        {
            "policyJson": _policy(),
            "policyEpoch": 7,
            "policySchema": " ",
        },
        {
            "policyBytes": b"policy",
            "policyEpoch": 7,
            "maxPolicyBytes": 4,
        },
        {
            "policyJson": _policy(),
            "policyEpoch": 7,
            "version": 2,
        },
    ],
)
def test_zkat_policy_commitment_rejects_malformed_inputs(
    input_value: dict[str, object],
) -> None:
    with pytest.raises((TypeError, ValueError), match="zkAtPolicyCommitment"):
        build_zkat_policy_commitment(input_value)


@pytest.mark.parametrize(
    "patch",
    [
        {"policyEpoch": 0},
        {"payload": b"payload", "txDigest": bytes([0xEE]) * 32},
        {"accountId": "alice@wonderland"},
        {"accountId": "0x" + "11" * 32},
        {"actionClass": " "},
        {"domain_separator": "boi:zkat:v1"},
        {"vkHash": bytes(32)},
        {"proofBytes": b""},
        {"backend": "groth16"},
        {"circuitId": "other_zkat_v1"},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"maxProofBytes": 4},
    ],
)
def test_zkat_authenticator_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope_input = {**_base_fixture(), "proofBytes": b"prepared-zkat-proof"}
    envelope_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="zkAtAuthenticatorEnvelope|privacyProofEnvelope",
    ):
        build_zkat_authenticator_envelope(envelope_input)


def test_zkat_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = _base_fixture()
    fixture = build_zkat_dev_proof_fixture(fixture_input)
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(public_inputs, indent=2).encode("utf-8")
    zero_policy_inputs = json.dumps(
        {
            **public_inputs,
            "policy_commitment": bytes(32).hex(),
        },
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    alias_collision_inputs = json.dumps(
        {
            **public_inputs,
            "accountId": public_inputs["account_id"],
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

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary"), "payload": _payload()},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof)), "payload": _payload()},
        {"envelope": fixture["envelope"], "payload": b"substituted-payload"},
        {
            "envelope": fixture["envelope"],
            "policyJson": {"threshold": 1, "roles": ["ops"]},
            "policyEpoch": 7,
            "policySchema": "boi-hidden-threshold-v1",
        },
        {
            "envelope": fixture["envelope"],
            "accountId": _account_id(),
            "actionClass": "different_action",
        },
        {"envelope": fixture["envelope"], "policyEpoch": 8},
        {"envelope": rebuild(backend="groth16"), "payload": _payload()},
        {"envelope": rebuild(circuitId="other_zkat_v1"), "payload": _payload()},
        {"envelope": rebuild(vkHash=bytes([0x56]) * 32), "payload": _payload()},
        {"envelope": rebuild(publicInputs=noncanonical_inputs), "payload": _payload()},
        {"envelope": rebuild(publicInputs=zero_policy_inputs), "payload": _payload()},
        {
            "envelope": rebuild(publicInputs=alias_collision_inputs),
            "payload": _payload(),
        },
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="zkAtAuthenticatorLocalVerification|privacyProofEnvelope",
        ):
            verify_zkat_authenticator_locally(case)
