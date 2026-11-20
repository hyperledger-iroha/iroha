"""Shared dense transaction vectors through the existing native typed models."""

from __future__ import annotations

import base64
import importlib.util
import json
from pathlib import Path

import pytest

# Do not substitute a stale artifact or a Python transaction implementation.
# When present, the ordinary package import authenticates the native owner.
if importlib.util.find_spec("iroha_native._crypto") is None:
    pytest.skip("requires the matching iroha_native._crypto extension", allow_module_level=True)

crypto = importlib.import_module("iroha_python.crypto")

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "fixtures" / "norito_rpc"
DENSE = json.loads((FIXTURES / "transaction_fixtures.dense.json").read_text())["fixtures"]
CURRENT = {
    entry["name"]: entry
    for entry in json.loads((FIXTURES / "transaction_fixtures.manifest.json").read_text())["fixtures"]
}
CONTEXT = {"chain_discriminant": 753}


def fixture_bytes(entry: dict, field: str) -> bytes:
    return base64.b64decode(entry[field], validate=True)


@pytest.mark.parametrize("entry", DENSE, ids=lambda entry: entry["name"])
def test_typed_transaction_roundtrip_hash_and_signing(entry: dict) -> None:
    current = CURRENT[entry["name"]]
    payload = fixture_bytes(entry, "payload_base64")
    signed = fixture_bytes(entry, "signed_base64")
    payload_json = crypto.decode_transaction_payload_with_layout(payload, layout="dense", **CONTEXT)
    current_json = crypto.decode_transaction_payload_with_layout(
        fixture_bytes(current, "payload_base64"), layout="current", framed=True, **CONTEXT
    )
    assert json.loads(payload_json) == json.loads(current_json)
    assert crypto.encode_transaction_payload_with_layout(payload_json, layout="dense", **CONTEXT) == payload
    assert crypto.encode_transaction_payload_with_layout(
        payload_json, layout="current", framed=True, **CONTEXT
    ) == fixture_bytes(current, "payload_base64")
    signed_json = crypto.decode_signed_transaction_with_layout(signed, layout="dense", **CONTEXT)
    assert crypto.encode_signed_transaction_with_layout(signed_json, layout="dense", **CONTEXT) == signed
    current_signed_json = crypto.decode_signed_transaction_with_layout(
        fixture_bytes(current, "signed_base64"), layout="current", framed=True, **CONTEXT
    )
    assert crypto.encode_signed_transaction_with_layout(
        current_signed_json, layout="current", framed=True, **CONTEXT
    ) == fixture_bytes(current, "signed_base64")
    assert crypto.transaction_payload_hash_with_layout(payload, layout="dense").hex() == entry["signing_hash"]
    # The manifest's `payload_hash` is the BLAKE2b-32 of the framed payload bytes,
    # not the typed signing hash; the typed current-layout hash is checked against
    # the fixture's own signature instead.
    assert crypto.hash_blake2b_32(fixture_bytes(current, "payload_base64")).hex() == current["payload_hash"]
    current_hash = crypto.transaction_payload_hash_with_layout(
        fixture_bytes(current, "payload_base64"), layout="current", framed=True
    )
    key = crypto.derive_ed25519_keypair_from_seed(b"android-fixture-signing-key-0102")
    (current_signature,) = json.loads(current_signed_json)["signature"]
    assert crypto.verify_ed25519(key.public_key, current_hash, bytes.fromhex(current_signature))
    assert not crypto.verify_ed25519(
        key.public_key,
        crypto.transaction_payload_hash_with_layout(payload, layout="dense"),
        bytes.fromhex(current_signature),
    )
    assert crypto.sign_transaction_payload_with_layout(payload, key.private_key, layout="dense") == signed
    assert crypto.sign_transaction_payload_with_layout(
        fixture_bytes(current, "payload_base64"), key.private_key, layout="current", framed=True
    ) == fixture_bytes(current, "signed_base64")


@pytest.mark.parametrize("layout", ["current", "dense"])
def test_framed_transactions_refuse_wrong_layout_unknown_minor_truncation_and_suffix(layout: str) -> None:
    entry = DENSE[0]
    decoded = crypto.decode_signed_transaction_with_layout(
        fixture_bytes(entry, "signed_base64"), layout="dense", **CONTEXT
    )
    frame = crypto.encode_signed_transaction_with_layout(decoded, layout=layout, framed=True, **CONTEXT)
    assert frame[5] == (1 if layout == "dense" else 0)
    assert crypto.decode_signed_transaction_with_layout(frame, layout=layout, framed=True, **CONTEXT) == decoded
    opposite = "current" if layout == "dense" else "dense"
    with pytest.raises(ValueError):
        crypto.decode_signed_transaction_with_layout(frame, layout=opposite, framed=True, **CONTEXT)
    unknown = bytearray(frame)
    unknown[5] = 2
    for malformed in [bytes(unknown), frame[:-1], frame + b"\x00"]:
        with pytest.raises(ValueError):
            crypto.decode_signed_transaction_with_layout(malformed, layout=layout, framed=True, **CONTEXT)


@pytest.mark.parametrize("field,decoder", [
    ("payload_base64", "decode_transaction_payload_with_layout"),
    ("signed_base64", "decode_signed_transaction_with_layout"),
])
def test_bare_dense_transactions_require_the_explicit_layout_and_exact_consumption(field: str, decoder: str) -> None:
    wire = fixture_bytes(DENSE[0], field)
    decode = getattr(crypto, decoder)
    with pytest.raises(ValueError):
        decode(wire, layout="current", **CONTEXT)
    for malformed in [wire[:-1], wire + b"\x00"]:
        with pytest.raises(ValueError):
            decode(malformed, layout="dense", **CONTEXT)
    for invalid_layout in ["compact", "Dense", " dense", "unknown"]:
        with pytest.raises(ValueError, match="layout"):
            decode(wire, layout=invalid_layout, **CONTEXT)


def test_existing_transaction_builder_explicit_layout_methods() -> None:
    key = crypto.Ed25519KeyPair.from_private_key(bytes([0x51]) * 32)
    authority = key.account_id(discriminant=753)
    builder = crypto.TransactionBuilder(
        crypto.NetworkId.from_bytes(bytes([0xA5]) * 32), authority,
        '{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}}',
        **CONTEXT,
    )
    builder.set_creation_time_ms(42)
    builder.set_ttl_ms(30_000)
    instruction = json.dumps({"name": "SetAssetHoldingLimit", "params": {
        "account_id": authority, "asset_definition_id": "66owaQmAQMuHxPzxUN3bqZ6FJfDa", "holding_limit": "100",
    }})
    builder.add_instruction_json(instruction)
    assert builder.encode_payload_with_layout(layout="current") == builder.encode_payload()
    assert builder.payload_hash_with_layout(layout="current") == builder.payload_hash()
    dense = builder.encode_payload_with_layout(layout="dense")
    assert builder.payload_hash_with_layout(layout="dense") == crypto.transaction_payload_hash_with_layout(dense, layout="dense")
    expected = crypto.sign_transaction_payload_with_layout(dense, key.private_key, layout="dense")
    with pytest.raises(ValueError, match="layout"):
        builder.sign_with_layout(key.private_key, layout="unknown")
    assert builder.encode_payload_with_layout(layout="dense") == dense
    assert builder.sign_with_layout(key.private_key, layout="dense") == expected
    builder.add_instruction_json(instruction)
    current = builder.sign_with_layout(key.private_key, layout="current")
    builder.add_instruction_json(instruction)
    assert builder.sign(key.private_key).signed_transaction == current
