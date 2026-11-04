"""Exact forwarding and refusal only; Rust owns signature/finality/proof verification."""
import json
from types import SimpleNamespace
import pytest
import iroha_python.crypto as crypto


def inputs():
    return dict(proof=b"opaque-cut", checkpoint_hex="01", proof_frames_hex=["02", "03"],
        network_id_hex="01" * 32, chain_discriminant=369, pool_id_hex="04" * 32,
        asset_definition_id="asset", payer_account_id="payer", expected_profile_digest_hex="05" * 32)


def test_pool_cut_wrapper_forwards_original_proof_and_every_independent_pin(monkeypatch):
    arguments = inputs()
    seen = []
    expected = dict(height=3, block_hash_hex="06" * 32, core_hash_hex="07" * 32,
                    context_id_hex="08" * 32, anchor_epoch=1)
    def native(*values):
        seen.append(values)
        return json.dumps(expected)
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_pool_cut_v1=native))
    assert crypto.verify_ironwood_pool_cut_v1(**arguments) == expected
    assert seen == [tuple(arguments.values())]


@pytest.mark.parametrize("field,value", [("proof", b""), ("proof", bytearray(b"x")),
                                        ("chain_discriminant", True), ("chain_discriminant", 65536)])
def test_pool_cut_wrapper_rejects_invalid_python_inputs_before_native(monkeypatch, field, value):
    def unexpected(*args): pytest.fail("invalid Python input reached native")
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_pool_cut_v1=unexpected))
    with pytest.raises((TypeError, ValueError)):
        crypto.verify_ironwood_pool_cut_v1(**{**inputs(), field: value})


def test_pool_cut_wrapper_preserves_native_refusal_and_requires_object(monkeypatch):
    refusal = ValueError("inert native proof authentication refusal")
    def native(*args): raise refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_pool_cut_v1=native))
    with pytest.raises(ValueError) as error:
        crypto.verify_ironwood_pool_cut_v1(**inputs())
    assert error.value is refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_pool_cut_v1=lambda *args: "[]"))
    with pytest.raises(RuntimeError, match="invalid result"):
        crypto.verify_ironwood_pool_cut_v1(**inputs())


def test_original_inspection_forwards_canonical_bytes_and_network_prefix(monkeypatch):
    network = crypto.NetworkId.from_bytes(bytes([1]) * 32)
    seen = []
    def native(wire, network_id, *, chain_discriminant):
        seen.append((wire, network_id, chain_discriminant))
        return '{"unsigned_payload_json":"original"}'
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(inspect_signed_privacy_ironwood_note_action_v1=native))
    assert crypto.inspect_signed_privacy_ironwood_note_action_v1(memoryview(b"signed"), network,
        chain_discriminant=369) == {"unsigned_payload_json": "original"}
    assert seen == [(b"signed", network, 369)]
    with pytest.raises(TypeError):
        crypto.inspect_signed_privacy_ironwood_note_action_v1("not bytes", network, chain_discriminant=369)


def test_original_inspection_missing_extension_or_native_refusal_never_accepts(monkeypatch):
    network = crypto.NetworkId.from_bytes(bytes([1]) * 32)
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace())
    with pytest.raises(RuntimeError, match="rebuild"):
        crypto.inspect_signed_privacy_ironwood_note_action_v1(b"wire", network, chain_discriminant=369)
    def native(*args, **kwargs): raise ValueError("inert signature refusal")
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(inspect_signed_privacy_ironwood_note_action_v1=native))
    with pytest.raises(ValueError, match="invalid canonical"):
        crypto.inspect_signed_privacy_ironwood_note_action_v1(b"wire", network, chain_discriminant=369)


def effect_inputs():
    return {**inputs(), "payment_proof_frames_hex": ["03", "04", "05"],
            "signed_transaction_versioned": b"signed-original", "expected_amount_decimal": "17"}


def test_payment_effect_wrapper_forwards_original_cut_history_transaction_and_pins(monkeypatch):
    values = effect_inputs()
    seen = []
    def native(*args):
        seen.append(args)
        return '{"height":5,"payer_debit_decimal":"17","reserve_credit_decimal":"17"}'
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_payment_effect_v1=native))
    assert crypto.verify_ironwood_payment_effect_v1(**values)["height"] == 5
    assert seen == [tuple(values[name] for name in (
        "proof", "checkpoint_hex", "proof_frames_hex", "payment_proof_frames_hex",
        "signed_transaction_versioned", "network_id_hex", "chain_discriminant", "pool_id_hex",
        "asset_definition_id", "payer_account_id", "expected_profile_digest_hex", "expected_amount_decimal"))]


@pytest.mark.parametrize("field,value", [("proof", b""), ("proof", bytearray(b"x")),
    ("signed_transaction_versioned", b""), ("signed_transaction_versioned", "wire"),
    ("chain_discriminant", True), ("chain_discriminant", -1)])
def test_payment_effect_python_bounds_refuse_before_native(monkeypatch, field, value):
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(
        verify_ironwood_payment_effect_v1=lambda *a: pytest.fail("invalid input reached native")))
    with pytest.raises((TypeError, ValueError)):
        crypto.verify_ironwood_payment_effect_v1(**{**effect_inputs(), field: value})


def test_payment_effect_preserves_native_refusal_and_requires_current_extension(monkeypatch):
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace())
    with pytest.raises(RuntimeError, match="rebuild"):
        crypto.verify_ironwood_payment_effect_v1(**effect_inputs())
    refusal = ValueError("inert committed payment refusal")
    def native(*args): raise refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_payment_effect_v1=native))
    with pytest.raises(ValueError) as error:
        crypto.verify_ironwood_payment_effect_v1(**effect_inputs())
    assert error.value is refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_payment_effect_v1=lambda *a: "[]"))
    with pytest.raises(RuntimeError, match="invalid result"):
        crypto.verify_ironwood_payment_effect_v1(**effect_inputs())


def test_bootstrap_export_preserves_native_instruction_json_and_every_pin(monkeypatch):
    seen = []
    raw = '[ "bmF0aXZlLWluc3RydWN0aW9uLWZyYW1l" ]'
    def native(*args):
        seen.append(args)
        return raw
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(ironwood_pool_bootstrap_instruction_json_v1=native))
    assert crypto.ironwood_pool_bootstrap_instruction_json_v1("03" * 32, "asset", "reserve",
        chain_discriminant=369) == raw
    assert seen == [("03" * 32, "asset", "reserve", 369)]
    with pytest.raises((TypeError, ValueError)):
        crypto.ironwood_pool_bootstrap_instruction_json_v1("03" * 32, "asset", "reserve",
                                                         chain_discriminant=True)
    assert len(seen) == 1


@pytest.mark.parametrize("result", ["{}", "[]", "[{},{}]", "[{}]", '[""]', "[7]", b"[{}]"])
def test_bootstrap_export_refuses_invalid_native_shape(monkeypatch, result):
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(
        ironwood_pool_bootstrap_instruction_json_v1=lambda *args: result))
    with pytest.raises(RuntimeError, match="invalid"):
        crypto.ironwood_pool_bootstrap_instruction_json_v1("03" * 32, "asset", "reserve", chain_discriminant=369)
