"""Exact forwarding and refusal only; Rust owns signature/finality/proof verification."""
import json
from types import SimpleNamespace
import pytest
import iroha_python.crypto as crypto


def inputs():
    return dict(pool_rows=b"pool-proof", balance_rows=b"balance-proof", checkpoint_hex="01", proof_frames_hex=["02", "03"],
        network_id_hex="01" * 32, chain_discriminant=369, pool_id_hex="04" * 32,
        asset_definition_id="asset", payer_account_id="payer", expected_profile_digest_hex="05" * 32, expected_schema="06" * 32)


def test_state_row_wrapper_forwards_original_proof_and_every_independent_pin(monkeypatch):
    arguments = inputs()
    seen = []
    expected = dict(height=3, block_hash_hex="06" * 32, core_hash_hex="07" * 32,
                    context_id_hex="08" * 32, anchor_epoch=1)
    def native(*values):
        seen.append(values)
        return json.dumps(expected)
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_state_rows_v1=native))
    assert crypto.verify_ironwood_state_rows_v1(**arguments) == expected
    assert seen == [tuple(arguments.values())]


@pytest.mark.parametrize("field,value", [("pool_rows", b""), ("balance_rows", bytearray(b"x")),
                                        ("chain_discriminant", True), ("chain_discriminant", 65536)])
def test_state_row_wrapper_rejects_invalid_python_inputs_before_native(monkeypatch, field, value):
    def unexpected(*args): pytest.fail("invalid Python input reached native")
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_state_rows_v1=unexpected))
    with pytest.raises((TypeError, ValueError)):
        crypto.verify_ironwood_state_rows_v1(**{**inputs(), field: value})


def test_state_row_wrapper_preserves_native_refusal_and_requires_object(monkeypatch):
    refusal = ValueError("inert native proof authentication refusal")
    def native(*args): raise refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_state_rows_v1=native))
    with pytest.raises(ValueError) as error:
        crypto.verify_ironwood_state_rows_v1(**inputs())
    assert error.value is refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_state_rows_v1=lambda *args: "[]"))
    with pytest.raises(RuntimeError, match="invalid result"):
        crypto.verify_ironwood_state_rows_v1(**inputs())


def effect_inputs():
    return {**inputs(), "payment_proof_frames_hex": ["03", "04", "05"],
            "signed_transaction_versioned": b"signed-original", "expected_amount_decimal": "17"}


def test_payment_effect_wrapper_forwards_original_rows_history_transaction_and_pins(monkeypatch):
    values = effect_inputs()
    seen = []
    def native(*args):
        seen.append(args)
        return '{"height":5,"payer_debit_decimal":"17","reserve_credit_decimal":"17"}'
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_payment_effect_from_rows_v1=native))
    assert crypto.verify_ironwood_payment_effect_from_rows_v1(**values)["height"] == 5
    assert seen == [tuple(values[name] for name in (
        "pool_rows", "balance_rows", "checkpoint_hex", "proof_frames_hex", "payment_proof_frames_hex",
        "signed_transaction_versioned", "network_id_hex", "chain_discriminant", "pool_id_hex",
        "asset_definition_id", "payer_account_id", "expected_profile_digest_hex", "expected_schema", "expected_amount_decimal"))]


@pytest.mark.parametrize("field,value", [("pool_rows", b""), ("balance_rows", bytearray(b"x")),
    ("signed_transaction_versioned", b""), ("signed_transaction_versioned", "wire"),
    ("chain_discriminant", True), ("chain_discriminant", -1)])
def test_payment_effect_python_bounds_refuse_before_native(monkeypatch, field, value):
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(
        verify_ironwood_payment_effect_from_rows_v1=lambda *a: pytest.fail("invalid input reached native")))
    with pytest.raises((TypeError, ValueError)):
        crypto.verify_ironwood_payment_effect_from_rows_v1(**{**effect_inputs(), field: value})


def test_payment_effect_preserves_native_refusal_and_requires_current_extension(monkeypatch):
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace())
    with pytest.raises(RuntimeError, match="rebuild"):
        crypto.verify_ironwood_payment_effect_from_rows_v1(**effect_inputs())
    refusal = ValueError("inert committed payment refusal")
    def native(*args): raise refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_payment_effect_from_rows_v1=native))
    with pytest.raises(ValueError) as error:
        crypto.verify_ironwood_payment_effect_from_rows_v1(**effect_inputs())
    assert error.value is refusal
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_ironwood_payment_effect_from_rows_v1=lambda *a: "[]"))
    with pytest.raises(RuntimeError, match="invalid result"):
        crypto.verify_ironwood_payment_effect_from_rows_v1(**effect_inputs())




class LivePage:
    def verify_rows(self, first, second, chain, pool, asset, payer, profile, schema):
        assert profile == "05" * 32 and schema == "06" * 32
        assert (first, second) in [(b"first", b"second"), (b"new-pool", b"new-balances")]
        return '{"height":9}'


def test_client_fetches_both_batches_at_one_height_and_keeps_both_originals(monkeypatch):
    from iroha_python.client import ToriiClient
    monkeypatch.setattr(crypto, "_ironwood_finality_page", lambda *args: LivePage())

    pool_keys = [{"table": "world.privacy_commitments", "key_hex": "01"}]
    balance_keys = [{"table": "world.assets", "key_hex": "02"}]
    calls = []
    auth = object()
    class Reader:
        def get_state_rows_proof(self, height, rows, *, canonical_auth):
            calls.append((height, rows, canonical_auth))
            return b"first" if rows == pool_keys else b"second"

    monkeypatch.setattr(crypto, "_ironwood_pool_selectors", lambda pool: pool_keys)
    def derive(frame, **pins):
        assert frame == b"first"
        assert pins["expected_schema"] == "06" * 32
        return balance_keys, "verified-read-plan"
    monkeypatch.setattr(crypto, "_ironwood_balance_selectors", derive)
    pins = {k:v for k,v in inputs().items() if k not in {"pool_rows", "balance_rows"}}
    result = ToriiClient.get_ironwood_pool_context(Reader(), 9, canonical_auth=auth, **pins)
    assert (result.pool_rows, result.balance_rows, result.verified) == (b"first", b"second", {"height": 9})
    assert result.original_responses == (b"first", b"second")
    assert calls == [(9, pool_keys, auth), (9, balance_keys, auth)]


def test_client_refuses_before_dependent_query_when_first_proof_fails(monkeypatch):
    from iroha_python.client import ToriiClient
    monkeypatch.setattr(crypto, "_ironwood_finality_page", lambda *args: LivePage())

    calls = []
    class Reader:
        def get_state_rows_proof(self, height, rows, *, canonical_auth):
            calls.append(height)
            return b"uncertified"
    monkeypatch.setattr(crypto, "_ironwood_pool_selectors", lambda pool: [])
    def reject(frame, **pins):
        raise ValueError("wrong certified root")
    monkeypatch.setattr(crypto, "_ironwood_balance_selectors", reject)
    pins = {k:v for k,v in inputs().items() if k not in {"pool_rows", "balance_rows"}}
    with pytest.raises(ValueError, match="wrong certified root"):
        ToriiClient.get_ironwood_pool_context(Reader(), 9, canonical_auth=object(), **pins)
    assert calls == [9]


def test_generic_fetch_sends_exact_selectors_to_one_signed_path():
    from iroha_python.client import ToriiClient
    from requests import Response
    import io
    seen = []
    auth = object()
    wire = b"generic-row-proof"
    class Reader:
        _encode_json_body = staticmethod(ToriiClient._encode_json_body)
        def _account_request(self, method, path, **kwargs):
            seen.append((method, path, kwargs))
            response = Response()
            response.status_code = 200
            response.headers["Content-Type"] = "application/x-norito"
            response.headers["Content-Length"] = str(len(wire))
            response.raw = io.BytesIO(wire)
            return response
    rows = [{"table": "world.assets", "key_hex": "01"}]
    assert ToriiClient.get_state_rows_proof(Reader(), 19, rows, canonical_auth=auth) == wire
    [(method, path, request)] = seen
    assert (method, path, request["canonical_auth"]) == ("POST", "/v1/ledger/state-proof/19", auth)
    assert json.loads(request["data"]) == {"rows": rows}
    assert request["headers"]["Accept"] == "application/x-norito"


@pytest.mark.parametrize("size,accepted", [(32 * 1024 * 1024, True), (32 * 1024 * 1024 + 1, False)])
def test_generic_fetch_uses_portable_frame_ceiling_and_closes_oversize(size, accepted):
    from iroha_python.client import ToriiClient
    from requests import Response
    import io

    response = Response()
    response.status_code = 200
    response.headers["Content-Type"] = "application/x-norito"
    response.headers["Content-Length"] = str(size)
    response.raw = io.BytesIO(bytes(size if accepted else 0))
    class Reader:
        _encode_json_body = staticmethod(ToriiClient._encode_json_body)
        def _account_request(self, *args, **kwargs):
            return response

    def fetch():
        return ToriiClient.get_state_rows_proof(Reader(), 2, [{"table":"world.assets","key_hex":"01"}], canonical_auth=object())
    if accepted:
        assert len(fetch()) == size
    else:
        with pytest.raises(ValueError, match="byte limit"):
            fetch()
        assert response.raw.closed


def test_generic_fetch_closes_denied_response_without_reading_its_body():
    from iroha_python.client import ToriiClient
    from iroha_torii_client.errors import ToriiForbiddenError
    from requests import Response
    import io
    response = Response()
    response.status_code = 403
    class Body(io.BytesIO):
        def read(self, *args, **kwargs):
            pytest.fail("a denied response body was read")
    response.raw = Body(b"private ledger information")
    class Reader:
        _encode_json_body = staticmethod(ToriiClient._encode_json_body)
        def _account_request(self, *args, **kwargs):
            return response
    with pytest.raises(ToriiForbiddenError) as refused:
        ToriiClient.get_state_rows_proof(Reader(), 2, [{"table":"world.assets","key_hex":"01"}], canonical_auth=object())
    assert "403" in str(refused.value)
    assert response.raw.closed


@pytest.mark.parametrize("moved", [False, True])
def test_previous_context_uses_one_request_or_only_missing_keys(monkeypatch, moved):
    from iroha_python.client import IronwoodPoolContext, ToriiClient
    monkeypatch.setattr(crypto, "_ironwood_finality_page", lambda *args: LivePage())

    known = [{"table": "world.assets", "key_hex": "01"}]
    missing = [{"table": "world.privacy_roots", "key_hex": "02"}]
    calls = []
    class Plan:
        def request(self, network, chain, pool, asset, payer, schema):
            assert (network, chain, pool, asset, payer, schema) == (
                "01" * 32, 369, "04" * 32, "asset", "payer", "06" * 32,
            )
            return json.dumps({"rows": known})
    plan = Plan()
    previous = IronwoodPoolContext(b"old-pool", b"old-balance", {"height": 6}, (b"old",), plan)
    auth = object()
    class Reader:
        def get_state_rows_proof(self, height, rows, *, canonical_auth):
            calls.append((height, rows, canonical_auth))
            return b"bulk" if rows == known else b"supplement"
    def normalize(old_plan, bulk, supplement, **pins):
        assert old_plan is plan and bulk == b"bulk"
        if moved and supplement is None:
            return "next-plan", missing, b"new-pool", None
        assert supplement == (b"supplement" if moved else None)
        return "next-plan", [], b"new-pool", b"new-balances"
    monkeypatch.setattr(crypto, "_ironwood_warm_rows", normalize)
    pins = {k:v for k,v in inputs().items() if k not in {"pool_rows", "balance_rows"}}
    result = ToriiClient.get_ironwood_pool_context(Reader(), 9, canonical_auth=auth, previous=previous, **pins)
    assert result.original_responses == ((b"bulk", b"supplement") if moved else (b"bulk",))
    assert calls == [(9, known, auth)] + ([(9, missing, auth)] if moved else [])


def test_schema_rejects_unbound_bytes_and_plans_have_no_public_constructor():
    with pytest.raises(ValueError):
        crypto.IronwoodStateSchemaV1(b"arbitrary", "01" * 32)
    with pytest.raises(TypeError):
        crypto._crypto.IronwoodReadPlan()


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


def test_generic_verifier_forwards_exact_pins_and_preserves_empty_vs_absence(monkeypatch):
    rows = [{"table": "world.assets", "key_hex": "01"}, {"table": "world.assets", "key_hex": "02"}]
    schema = object()
    def verify(proof, selected, actual_schema, checkpoint, frames, network, height):
        assert (proof, json.loads(selected), actual_schema, checkpoint, frames, network, height) == (
            b"frame", rows, schema, "ab", ["ef"], "07" * 32, 17,
        )
        return [b"", None]
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_state_rows_v1=verify))
    assert crypto.verify_state_rows_v1(b"frame", rows, schema=schema, checkpoint_hex="ab",
        proof_frames_hex=["ef"], network_id_hex="07" * 32, height=17) == [b"", None]


@pytest.mark.parametrize("height,rows", [(True, []), (2, []), (2, [{"table":"x"}]),
    (2, [{"table":"x", "key_hex":"02", "extra":False}]), (2,[{"table":"x", "key_hex":"0"*8193}]), (2, [{"table":"x","key_hex":"AB"}]),
    (2,[{"table":"x","key_hex":"01"}, {"table":"x","key_hex":"00"}])])
def test_generic_verifier_bounds_selectors_before_native(height, rows, monkeypatch):
    monkeypatch.setattr(crypto, "_crypto", SimpleNamespace(verify_state_rows_v1=lambda *args: pytest.fail("unexpected native call")))
    with pytest.raises(ValueError):
        crypto.verify_state_rows_v1(b"frame", rows, schema=object(), checkpoint_hex="ab",
            proof_frames_hex=["ef"], network_id_hex="07"*32, height=height)
