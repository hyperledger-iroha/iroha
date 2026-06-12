from __future__ import annotations

import json

import pytest

from iroha_python import (
    AccountAddress,
    buildZkX509IdentityCommitments,
    buildZkX509IdentityDevProofFixture,
    buildZkX509IdentityEnvelope,
    buildZkX509IdentityProofV0,
    build_zk_x509_identity_commitments,
    build_zk_x509_identity_dev_proof_fixture,
    build_zk_x509_identity_envelope,
    build_zk_x509_identity_proof_v0,
    decode_privacy_proof_envelope,
    verifyZkX509IdentityProofV0,
    verifyZkX509IdentityProofLocally,
    verify_zk_x509_identity_proof_v0,
    verify_zk_x509_identity_proof_locally,
)
from iroha_python.verange import build_privacy_proof_envelope


def _account_id(byte: int = 0x11) -> str:
    return AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([byte] * 32),
    ).to_i105(0x02F1)


def _ca_root(root: str = "boi-root-ca") -> dict[str, object]:
    return {
        "root": root,
        "version": 1,
        "not_before": "2026-01-01T00:00:00Z",
    }


def _certificate_policy(policy: str = "institutional-wallet") -> dict[str, object]:
    return {"eku": ["clientAuth"], "policy": policy}


def _revocation(epoch: int = 7) -> dict[str, object]:
    return {"epoch": epoch, "root": "crlite-root-7"}


def _subject(cn: str = "Bank A") -> dict[str, object]:
    return {"cn": cn, "lei": "5493001KJTIIGC8Y1R12"}


def _base() -> dict[str, object]:
    return {
        "caRootJson": _ca_root(),
        "certificatePolicyJson": _certificate_policy(),
        "revocationJson": _revocation(),
        "subjectJson": _subject(),
        "accountId": _account_id(),
        "domainSeparator": "boi:zk-x509:pilot:v0",
    }


def test_zk_x509_builders_normalize_commitments_and_envelopes() -> None:
    base = _base()
    commitments = build_zk_x509_identity_commitments(base)

    assert commitments["version"] == 1
    assert len(commitments["ca_root_commitment"]) == 32
    assert len(commitments["address_binding"]) == 32
    assert commitments["domain_separator"] == "boi:zk-x509:pilot:v0"
    assert commitments["commitment_kinds"]["ca_root_commitment"] == (
        "dev-sha256-ca-root-digest"
    )
    assert commitments["commitment_kinds"]["address_binding"] == (
        "dev-sha256-account-binding"
    )

    prepared = build_zk_x509_identity_envelope(
        {
            **base,
            "vkHash": bytes([0x99]) * 32,
            "proofBytes": b"prepared-zk-x509-proof",
        }
    )
    decoded_prepared = decode_privacy_proof_envelope(prepared)
    assert decoded_prepared["backend"] == "Stark"
    assert decoded_prepared["circuit_id"] == (
        "stark/fri/sha256-goldilocks:zk_x509_onchain_identity_v0"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["ca_root_commitment"] == commitments[
        "ca_root_commitment"
    ].hex()
    assert prepared_inputs["certificate_policy_hash"] == commitments[
        "certificate_policy_hash"
    ].hex()
    assert prepared_inputs["revocation_root"] == commitments["revocation_root"].hex()
    assert prepared_inputs["subject_commitment"] == commitments[
        "subject_commitment"
    ].hex()
    assert prepared_inputs["address_binding"] == commitments["address_binding"].hex()

    fixture = build_zk_x509_identity_dev_proof_fixture(
        {**base, "vkHash": bytes([0x99]) * 32}
    )
    assert fixture["kind"] == "zk-x509-dev-fixture-v0"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    verified = verify_zk_x509_identity_proof_locally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["address_binding"] == commitments["address_binding"].hex()
    assert verified["public_inputs"] == fixture["public_inputs"]

    production_proof = build_zk_x509_identity_proof_v0(
        {
            **base,
            "vkHash": bytes([0x99]) * 32,
            "proofBytes": b"production-zk-x509-identity-proof",
        }
    )
    production_verified = verify_zk_x509_identity_proof_v0(
        {"envelope": production_proof, **base}
    )
    assert production_verified["ok"] is True
    assert production_verified["production"] is True
    assert production_verified["kind"] == "zk-x509-onchain-identity-v0"
    assert production_verified["backend"] == "Stark"
    assert production_verified["address_binding"] == commitments["address_binding"].hex()
    assert production_verified["public_inputs"] == fixture["public_inputs"]


def test_zk_x509_package_root_exports_catalog_entrypoint_aliases() -> None:
    base = _base()
    commitments = buildZkX509IdentityCommitments(base)
    prepared = buildZkX509IdentityEnvelope(
        {
            **base,
            "vkHash": bytes([0x99]) * 32,
            "proofBytes": b"prepared-zk-x509-proof",
        }
    )
    assert decode_privacy_proof_envelope(prepared)["proof_bytes"] == (
        b"prepared-zk-x509-proof"
    )

    fixture = buildZkX509IdentityDevProofFixture(
        {**base, "vkHash": bytes([0x99]) * 32}
    )
    verified = verifyZkX509IdentityProofLocally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["address_binding"] == commitments["address_binding"].hex()

    production_proof = buildZkX509IdentityProofV0(
        {
            **base,
            "vkHash": bytes([0x99]) * 32,
            "proofBytes": b"production-zk-x509-identity-proof",
        }
    )
    production_verified = verifyZkX509IdentityProofV0(
        {"envelope": production_proof, **base}
    )
    assert production_verified["ok"] is True
    assert production_verified["production"] is True
    assert production_verified["address_binding"] == commitments["address_binding"].hex()


def test_zk_x509_public_helpers_reject_non_plain_mapping_inputs() -> None:
    class ZkX509Dict(dict):
        pass

    base = _base()
    proof_options = {
        **base,
        "vkHash": bytes([0x99]) * 32,
        "proofBytes": b"production-zk-x509-identity-proof",
    }

    for helper in (build_zk_x509_identity_commitments, buildZkX509IdentityCommitments):
        with pytest.raises(TypeError, match="zkX509IdentityCommitments"):
            helper(ZkX509Dict(base))

    for helper in (build_zk_x509_identity_envelope, buildZkX509IdentityEnvelope):
        with pytest.raises(TypeError, match="zkX509IdentityEnvelope"):
            helper(ZkX509Dict(proof_options))

    for helper in (build_zk_x509_identity_proof_v0, buildZkX509IdentityProofV0):
        with pytest.raises(TypeError, match="zkX509IdentityProofV0"):
            helper(ZkX509Dict(proof_options))

    for helper in (
        build_zk_x509_identity_dev_proof_fixture,
        buildZkX509IdentityDevProofFixture,
    ):
        with pytest.raises(TypeError, match="zkX509IdentityDevProofFixture"):
            helper(ZkX509Dict({**base, "vkHash": bytes([0x99]) * 32}))

    production_proof = build_zk_x509_identity_proof_v0(proof_options)
    raw_verified = verify_zk_x509_identity_proof_v0(production_proof)
    assert raw_verified["ok"] is True
    verify_options = {"envelope": production_proof, **base}
    for helper in (verify_zk_x509_identity_proof_v0, verifyZkX509IdentityProofV0):
        with pytest.raises(TypeError, match="zkX509IdentityProofV0"):
            helper(ZkX509Dict(verify_options))

    fixture = build_zk_x509_identity_dev_proof_fixture(
        {**base, "vkHash": bytes([0x99]) * 32}
    )
    local_verified = verify_zk_x509_identity_proof_locally(fixture["envelope"])
    assert local_verified["ok"] is True
    local_options = {
        **verify_options,
        "envelope": fixture["envelope"],
    }
    for helper in (verify_zk_x509_identity_proof_locally, verifyZkX509IdentityProofLocally):
        with pytest.raises(TypeError, match="zkX509IdentityLocalVerification"):
            helper(ZkX509Dict(local_options))


def test_zk_x509_wallet_address_alias_derives_address_binding() -> None:
    wallet_base = {
        "caRootJson": _ca_root(),
        "certificatePolicyJson": _certificate_policy(),
        "revocationJson": _revocation(),
        "subjectJson": _subject(),
        "walletAddress": "wallet-address-alias",
        "domainSeparator": "boi:zk-x509:pilot:v0",
    }
    commitments = build_zk_x509_identity_commitments(wallet_base)
    fixture = build_zk_x509_identity_dev_proof_fixture(
        {**wallet_base, "vkHash": bytes([0x99]) * 32}
    )
    verified = verify_zk_x509_identity_proof_locally(
        {"envelope": fixture["envelope"], **wallet_base}
    )

    assert commitments["commitment_kinds"]["address_binding"] == (
        "dev-sha256-account-binding"
    )
    assert verified["address_binding"] == commitments["address_binding"].hex()


@pytest.mark.parametrize(
    "input_value",
    [
        {**_base(), "caRootCommitment": bytes([0xEE]) * 32},
        {**_base(), "certificatePolicyHash": bytes([0xEE]) * 32},
        {**_base(), "revocationRoot": bytes([0xEE]) * 32},
        {**_base(), "subjectCommitment": bytes([0xEE]) * 32},
        {**_base(), "addressBinding": bytes([0xEE]) * 32},
        {
            "certificatePolicyJson": _certificate_policy(),
            "revocationJson": _revocation(),
            "subjectJson": _subject(),
            "accountId": _account_id(),
            "domainSeparator": "boi:zk-x509:pilot:v0",
        },
        {
            "caRootJson": _ca_root(),
            "revocationJson": _revocation(),
            "subjectJson": _subject(),
            "accountId": _account_id(),
            "domainSeparator": "boi:zk-x509:pilot:v0",
        },
        {
            "caRootJson": _ca_root(),
            "certificatePolicyJson": _certificate_policy(),
            "subjectJson": _subject(),
            "accountId": _account_id(),
            "domainSeparator": "boi:zk-x509:pilot:v0",
        },
        {
            "caRootJson": _ca_root(),
            "certificatePolicyJson": _certificate_policy(),
            "revocationJson": _revocation(),
            "accountId": _account_id(),
            "domainSeparator": "boi:zk-x509:pilot:v0",
        },
        {
            "caRootJson": _ca_root(),
            "certificatePolicyJson": _certificate_policy(),
            "revocationJson": _revocation(),
            "subjectJson": _subject(),
            "domainSeparator": "boi:zk-x509:pilot:v0",
        },
        {**_base(), "accountId": "not-an-account-id"},
        {**_base(), "walletAddress": "wallet-address-alias"},
        {**_base(), "walletAddress": " ", "accountId": None},
        {**_base(), "domainSeparator": " "},
        {**_base(), "caRootCommitment": bytes(32)},
        {**_base(), "addressBinding": bytes(32)},
        {**_base(), "version": 2},
        {**_base(), "domain_separator": "boi:zk-x509:pilot:v0"},
        {
            "caRootBytes": b"root-material",
            "certificatePolicyJson": _certificate_policy(),
            "revocationJson": _revocation(),
            "subjectJson": _subject(),
            "accountId": _account_id(),
            "domainSeparator": "boi:zk-x509:pilot:v0",
            "maxCaRootBytes": 4,
        },
        {**_base(), "__proto__": {"polluted": True}},
    ],
)
def test_zk_x509_commitments_reject_malformed_inputs(
    input_value: dict[str, object],
) -> None:
    with pytest.raises((TypeError, ValueError), match="zkX509IdentityCommitments"):
        build_zk_x509_identity_commitments(input_value)


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
        {"domain_separator": "boi:zk-x509:pilot:v0"},
    ],
)
def test_zk_x509_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope_input = {
        **_base(),
        "vkHash": bytes([0x99]) * 32,
        "proofBytes": b"prepared-zk-x509-proof",
    }
    envelope_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="zkX509IdentityEnvelope|privacyProofEnvelope",
    ):
        build_zk_x509_identity_envelope(envelope_input)


def test_zk_x509_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0x99]) * 32}
    fixture = build_zk_x509_identity_dev_proof_fixture(fixture_input)
    decoded = decode_privacy_proof_envelope(fixture["envelope"])
    public_inputs = json.loads(decoded["public_inputs"].decode("utf-8"))
    tampered_proof = bytearray(decoded["proof_bytes"])
    tampered_proof[-1] ^= 0xFF
    noncanonical_inputs = json.dumps(public_inputs, indent=2).encode("utf-8")
    zero_ca_root_inputs = json.dumps(
        {**public_inputs, "ca_root_commitment": bytes(32).hex()},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    alias_collision_inputs = json.dumps(
        {**public_inputs, "caRootCommitment": public_inputs["ca_root_commitment"]},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return build_privacy_proof_envelope(
            {
                "backend": patch.get("backend", "stark/fri/sha256-goldilocks"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0x99]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            }
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary")},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof))},
        {"envelope": fixture["envelope"], "caRootJson": _ca_root(root="other-root")},
        {
            "envelope": fixture["envelope"],
            "certificatePolicyJson": _certificate_policy(policy="server-wallet"),
        },
        {"envelope": fixture["envelope"], "revocationJson": _revocation(epoch=8)},
        {"envelope": fixture["envelope"], "subjectJson": _subject(cn="Bank B")},
        {"envelope": fixture["envelope"], "accountId": _account_id(0x12)},
        {"envelope": fixture["envelope"], "walletAddress": "wallet-address-alias"},
        {"envelope": fixture["envelope"], "domainSeparator": "boi:zk-x509:other:v0"},
        {"envelope": rebuild(backend="groth16")},
        {"envelope": rebuild(circuitId="stark/fri/sha256-goldilocks:wrong")},
        {"envelope": rebuild(vkHash=bytes([0x9A]) * 32)},
        {"envelope": rebuild(publicInputs=noncanonical_inputs)},
        {"envelope": rebuild(publicInputs=zero_ca_root_inputs)},
        {"envelope": rebuild(publicInputs=alias_collision_inputs)},
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="zkX509IdentityLocalVerification|privacyProofEnvelope",
        ):
            verify_zk_x509_identity_proof_locally(case)


def test_zk_x509_production_builder_and_verifier_reject_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0x99]) * 32}
    fixture = build_zk_x509_identity_dev_proof_fixture(fixture_input)

    with pytest.raises(ValueError, match="dev fixture"):
        build_zk_x509_identity_proof_v0(
            {
                **_base(),
                "vkHash": bytes([0x99]) * 32,
                "proofBytes": fixture["proof_bytes"],
            }
        )

    with pytest.raises(ValueError, match="dev fixture"):
        verify_zk_x509_identity_proof_v0({"envelope": fixture["envelope"], **_base()})
