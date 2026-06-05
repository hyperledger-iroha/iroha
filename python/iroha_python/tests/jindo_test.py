from __future__ import annotations

import json

import pytest

from iroha_python import (
    buildJindoLatticeDevProofFixture,
    buildJindoLatticeProofEnvelope,
    buildJindoLatticePublicInputs,
    build_jindo_lattice_dev_proof_fixture,
    build_jindo_lattice_proof_envelope,
    build_jindo_lattice_public_inputs,
    verifyJindoLatticeProofLocally,
    verify_jindo_lattice_proof_locally,
)
from iroha_python.verange import (
    _build_privacy_proof_envelope_internal,
    _decode_privacy_proof_envelope_internal,
    decode_privacy_proof_envelope,
)


def _polynomial(degree: int = 1024) -> dict[str, object]:
    return {
        "ring": "Rq",
        "degree": degree,
        "coefficients_digest": "poly-digest-1",
    }


def _opening_claim(point: str = "x=42") -> dict[str, object]:
    return {"point": point, "value_digest": "evaluation-digest-1"}


def _query_set(queries: list[int] | None = None) -> dict[str, object]:
    return {"queries": [0, 7, 42] if queries is None else queries, "batch": "opening-batch-1"}


def _parameters(scheme: str = "jindo-pcs-v0") -> dict[str, object]:
    return {
        "scheme": scheme,
        "q_bits": 64,
        "sigma": "research-parameter-set",
    }


def _base() -> dict[str, object]:
    return {
        "polynomialJson": _polynomial(),
        "openingClaimJson": _opening_claim(),
        "querySetJson": _query_set(),
        "parametersJson": _parameters(),
        "domainSeparator": "boi:jindo:pcs:pilot:v0",
    }


def test_jindo_builders_normalize_public_inputs_and_envelopes() -> None:
    base = _base()
    public_inputs = build_jindo_lattice_public_inputs(base)

    assert public_inputs["version"] == 1
    assert len(public_inputs["commitment"]) == 32
    assert len(public_inputs["parameter_hash"]) == 32
    assert public_inputs["domain_separator"] == "boi:jindo:pcs:pilot:v0"
    assert public_inputs["commitment_kinds"]["commitment"] == (
        "dev-sha256-commitment-digest"
    )
    assert public_inputs["commitment_kinds"]["parameter_hash"] == (
        "dev-sha256-parameter-hash"
    )

    prepared = build_jindo_lattice_proof_envelope(
        {
            **base,
            "vkHash": bytes([0xAA]) * 32,
            "proofBytes": b"prepared-jindo-lattice-proof",
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
        "lattice/jindo-pcs-v0:jindo_lattice_pcs_zk_v0"
    )
    prepared_inputs = json.loads(decoded_prepared["public_inputs"].decode("utf-8"))
    assert prepared_inputs["commitment"] == public_inputs["commitment"].hex()
    assert prepared_inputs["opening_claim"] == public_inputs["opening_claim"].hex()
    assert prepared_inputs["query_set"] == public_inputs["query_set"].hex()
    assert prepared_inputs["parameter_hash"] == public_inputs["parameter_hash"].hex()

    fixture = build_jindo_lattice_dev_proof_fixture(
        {**base, "vkHash": bytes([0xAA]) * 32}
    )
    assert fixture["kind"] == "jindo-lattice-dev-fixture-v0"
    assert fixture["production"] is False
    assert isinstance(fixture["envelope"], bytes)
    assert fixture["proofBytes"] == fixture["proof_bytes"]
    assert fixture["publicInputBytes"] == fixture["public_input_bytes"]

    verified = verify_jindo_lattice_proof_locally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["production"] is False
    assert verified["parameter_hash"] == public_inputs["parameter_hash"].hex()
    assert verified["public_inputs"] == fixture["public_inputs"]


def test_jindo_package_root_exports_catalog_entrypoint_aliases() -> None:
    base = _base()
    public_inputs = buildJindoLatticePublicInputs(base)
    prepared = buildJindoLatticeProofEnvelope(
        {
            **base,
            "vkHash": bytes([0xAA]) * 32,
            "proofBytes": b"prepared-jindo-lattice-proof",
        }
    )
    assert _decode_privacy_proof_envelope_internal(
        prepared,
        allow_unsupported_backend=True,
    )["proof_bytes"] == (
        b"prepared-jindo-lattice-proof"
    )

    fixture = buildJindoLatticeDevProofFixture(
        {**base, "vkHash": bytes([0xAA]) * 32}
    )
    verified = verifyJindoLatticeProofLocally(
        {"envelope": fixture["envelope"], **base}
    )
    assert verified["ok"] is True
    assert verified["parameter_hash"] == public_inputs["parameter_hash"].hex()


@pytest.mark.parametrize(
    "input_value",
    [
        {**_base(), "commitment": bytes([0xEE]) * 32},
        {**_base(), "openingClaimHash": bytes([0xEE]) * 32},
        {**_base(), "querySetHash": bytes([0xEE]) * 32},
        {**_base(), "parameterHash": bytes([0xEE]) * 32},
        {
            "openingClaimJson": _opening_claim(),
            "querySetJson": _query_set(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:jindo:pcs:pilot:v0",
        },
        {
            "polynomialJson": _polynomial(),
            "querySetJson": _query_set(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:jindo:pcs:pilot:v0",
        },
        {
            "polynomialJson": _polynomial(),
            "openingClaimJson": _opening_claim(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:jindo:pcs:pilot:v0",
        },
        {
            "polynomialJson": _polynomial(),
            "openingClaimJson": _opening_claim(),
            "querySetJson": _query_set(),
            "domainSeparator": "boi:jindo:pcs:pilot:v0",
        },
        {**_base(), "domainSeparator": " "},
        {**_base(), "commitment": bytes(32)},
        {**_base(), "version": 2},
        {**_base(), "domain_separator": "boi:jindo:pcs:pilot:v0"},
        {
            "polynomialBytes": b"polynomial-material",
            "openingClaimJson": _opening_claim(),
            "querySetJson": _query_set(),
            "parametersJson": _parameters(),
            "domainSeparator": "boi:jindo:pcs:pilot:v0",
            "maxPolynomialBytes": 4,
        },
        {**_base(), "__proto__": {"polluted": True}},
    ],
)
def test_jindo_public_inputs_reject_malformed_inputs(
    input_value: dict[str, object],
) -> None:
    with pytest.raises((TypeError, ValueError), match="jindoLatticePublicInputs"):
        build_jindo_lattice_public_inputs(input_value)


@pytest.mark.parametrize(
    "patch",
    [
        {"proofBytes": b""},
        {"vkHash": bytes(32)},
        {"backend": "stark/fri/sha256-goldilocks"},
        {"circuitId": "lattice/jindo-pcs-v0:wrong"},
        {"production": True},
        {"productionReady": True},
        {"production_ready": True},
        {"productionGate": {"ready": True}},
        {"production_gate": {"ready": True}},
        {"maxProofBytes": 4},
        {"maxPublicInputBytes": 4},
        {"domain_separator": "boi:jindo:pcs:pilot:v0"},
    ],
)
def test_jindo_proof_envelope_rejects_unsafe_shapes(
    patch: dict[str, object],
) -> None:
    envelope_input = {
        **_base(),
        "vkHash": bytes([0xAA]) * 32,
        "proofBytes": b"prepared-jindo-lattice-proof",
    }
    envelope_input.update(patch)

    with pytest.raises(
        (TypeError, ValueError),
        match="jindoLatticeProofEnvelope|privacyProofEnvelope",
    ):
        build_jindo_lattice_proof_envelope(envelope_input)


def test_jindo_local_verifier_rejects_tampered_dev_fixtures() -> None:
    fixture_input = {**_base(), "vkHash": bytes([0xAA]) * 32}
    fixture = build_jindo_lattice_dev_proof_fixture(fixture_input)
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
    zero_commitment_inputs = json.dumps(
        {**public_inputs, "commitment": bytes(32).hex()},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    alias_collision_inputs = json.dumps(
        {**public_inputs, "openingClaim": public_inputs["opening_claim"]},
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")

    def rebuild(**patch: object) -> bytes:
        return _build_privacy_proof_envelope_internal(
            {
                "backend": patch.get("backend", "unsupported"),
                "circuitId": patch.get("circuitId", decoded["circuit_id"]),
                "vkHash": patch.get("vkHash", bytes([0xAA]) * 32),
                "publicInputs": patch.get("publicInputs", decoded["public_inputs"]),
                "proofBytes": patch.get("proofBytes", decoded["proof_bytes"]),
            },
            allow_unsupported_backend=True,
        )

    cases = [
        {"envelope": rebuild(proofBytes=b"arbitrary")},
        {"envelope": rebuild(proofBytes=bytes(tampered_proof))},
        {"envelope": fixture["envelope"], "polynomialJson": _polynomial(degree=2048)},
        {"envelope": fixture["envelope"], "openingClaimJson": _opening_claim(point="x=9")},
        {"envelope": fixture["envelope"], "querySetJson": _query_set(queries=[99])},
        {"envelope": fixture["envelope"], "parametersJson": _parameters(scheme="other")},
        {"envelope": fixture["envelope"], "domainSeparator": "boi:jindo:pcs:other:v0"},
        {"envelope": rebuild(backend="stark/fri/sha256-goldilocks")},
        {"envelope": rebuild(circuitId="lattice/jindo-pcs-v0:wrong")},
        {"envelope": rebuild(vkHash=bytes([0xAB]) * 32)},
        {"envelope": rebuild(publicInputs=noncanonical_inputs)},
        {"envelope": rebuild(publicInputs=zero_commitment_inputs)},
        {"envelope": rebuild(publicInputs=alias_collision_inputs)},
    ]

    for case in cases:
        with pytest.raises(
            (TypeError, ValueError),
            match="jindoLatticeLocalVerification|privacyProofEnvelope",
        ):
            verify_jindo_lattice_proof_locally(case)
