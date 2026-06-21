import hashlib
import json
import urllib.error
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364"
)
TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "9e13e2c5a72e2a60d73c4fd3fb46d819802f27ae52e7efa9375d7979f9d4a086"
)
TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "ed465ff6cd2b229705abf3b3dfd07980d643392f6c2cb1932fcf044aa6f4d81d"
)
TRON_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "a128acf9ca3e42e11ffb777822ae8e1c1d5f4cea514f7bbace652445aaae17f1"
)
TRON_ROUTE_CANARY_EVIDENCE_HASH = "e3" * 32
TRON_TEST_BRIDGE20 = bytes.fromhex("11" * 20)
TRON_TEST_OWNER20 = bytes.fromhex("7e5f4552091a69125d5dfcb7b8c2659029395bdf")
TRON_TRIGGER_SMART_CONTRACT_TYPE_URL = (
    b"type.googleapis.com/protocol.TriggerSmartContract"
)
TRON_SOURCE_EVENT_DIGEST_VECTOR = "34" * 32
TRON_SOURCE_EVENT_CALL_DATA_VECTOR = (
    "06841e30"
    + "00" * 31
    + "05"
    + "00" * 32
    + TRON_SOURCE_EVENT_DIGEST_VECTOR
)
DEFAULT_SOURCE_RUNTIME_BYTECODE = object()
DEFAULT_TRANSACTION_INFO_FIELD = object()


def transaction_info_field(default, override):
    return default if override is DEFAULT_TRANSACTION_INFO_FIELD else override


def protobuf_varint(value):
    out = bytearray()
    while value >= 0x80:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def protobuf_key(field_number, wire_type):
    return protobuf_varint((field_number << 3) | wire_type)


def protobuf_bytes(field_number, value):
    return protobuf_key(field_number, 2) + protobuf_varint(len(value)) + value


def protobuf_u64(field_number, value):
    return protobuf_key(field_number, 0) + protobuf_varint(value)


def protobuf_int_value(value):
    return int(value) if isinstance(value, str) else value


def tron_source_event_raw_data_hex(
    *,
    owner20=TRON_TEST_OWNER20,
    bridge20=TRON_TEST_BRIDGE20,
    call_data=bytes.fromhex(TRON_SOURCE_EVENT_CALL_DATA_VECTOR),
):
    trigger = b"".join(
        [
            protobuf_bytes(1, b"\x41" + owner20),
            protobuf_bytes(2, b"\x41" + bridge20),
            protobuf_bytes(4, call_data),
        ]
    )
    parameter = b"".join(
        [
            protobuf_bytes(1, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL),
            protobuf_bytes(2, trigger),
        ]
    )
    contract = b"".join([protobuf_u64(1, 31), protobuf_bytes(2, parameter)])
    raw_data = b"".join(
        [
            protobuf_bytes(1, b"\x12\x34"),
            protobuf_u64(3, 12_345),
            protobuf_bytes(4, b"\x56" * 8),
            protobuf_u64(8, 123_456_789),
            protobuf_bytes(11, contract),
            protobuf_u64(14, 123_450_000),
            protobuf_u64(18, 50_000_000),
        ]
    )
    return raw_data.hex()


TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR = tron_source_event_raw_data_hex()
TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR = hashlib.sha256(
    bytes.fromhex(TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR)
).hexdigest()
TRON_SOURCE_EVENT_SIGNATURE_VECTOR = (
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    "38508a4cf743e4a97ab3550672d69d980545ff8d776f6e9bade4ff4196f3693b"
    "00"
)


def tron_source_event_transaction_bytes_hex(
    *,
    raw_data_hex=TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR,
    signature_hex=TRON_SOURCE_EVENT_SIGNATURE_VECTOR,
    fee=None,
    ret=None,
):
    result = bytearray()
    if fee is not None:
        result.extend(protobuf_u64(1, fee))
    if ret is not None:
        result.extend(protobuf_u64(2, ret))
    result.extend(protobuf_u64(3, 1))
    transaction = b"".join(
        [
            protobuf_bytes(1, bytes.fromhex(raw_data_hex)),
            protobuf_bytes(2, bytes.fromhex(signature_hex)),
            protobuf_bytes(5, bytes(result)),
        ]
    )
    return transaction.hex()


def tron_dummy_transaction_bytes_hex(raw_data_hex="01"):
    return protobuf_bytes(1, bytes.fromhex(raw_data_hex)).hex()


def protobuf_string(field_number, value):
    return protobuf_bytes(field_number, value.encode("utf-8"))


def tron_market_order_detail_bytes(detail):
    out = bytearray()
    if "makerOrderId" in detail:
        out.extend(protobuf_bytes(1, bytes.fromhex(detail["makerOrderId"])))
    if "takerOrderId" in detail:
        out.extend(protobuf_bytes(2, bytes.fromhex(detail["takerOrderId"])))
    if "fillSellQuantity" in detail:
        out.extend(
            protobuf_u64(3, protobuf_int_value(detail["fillSellQuantity"]))
        )
    if "fillBuyQuantity" in detail:
        out.extend(
            protobuf_u64(4, protobuf_int_value(detail["fillBuyQuantity"]))
        )
    return bytes(out)


def tron_transaction_result_bytes(result):
    out = bytearray()
    if "fee" in result:
        out.extend(protobuf_u64(1, protobuf_int_value(result["fee"])))
    if "ret" in result:
        ret = result["ret"]
        out.extend(
            protobuf_u64(
                2,
                0 if ret in ("0", "SUCESS") else protobuf_int_value(ret),
            )
        )
    if "contractRet" in result:
        contract_ret = result["contractRet"]
        out.extend(protobuf_u64(3, 1 if contract_ret == "SUCCESS" else contract_ret))
    if "assetIssueID" in result:
        out.extend(protobuf_string(14, result["assetIssueID"]))
    for field_number, field_name in (
        (15, "withdraw_amount"),
        (16, "unfreeze_amount"),
        (18, "exchange_received_amount"),
        (19, "exchange_inject_another_amount"),
        (20, "exchange_withdraw_another_amount"),
        (21, "exchange_id"),
        (22, "shielded_transaction_fee"),
    ):
        if field_name in result:
            out.extend(
                protobuf_u64(field_number, protobuf_int_value(result[field_name]))
            )
    if "orderId" in result:
        out.extend(protobuf_bytes(25, bytes.fromhex(result["orderId"])))
    for detail in result.get("orderDetails", []):
        out.extend(protobuf_bytes(26, tron_market_order_detail_bytes(detail)))
    if "withdraw_expire_amount" in result:
        out.extend(
            protobuf_u64(
                27,
                protobuf_int_value(result["withdraw_expire_amount"]),
            )
        )
    cancel_amounts = result.get(
        "cancel_unfreezeV2_amount",
        result.get("cancelUnfreezeV2Amount", {}),
    )
    for key, value in cancel_amounts.items():
        out.extend(
            protobuf_bytes(
                28,
                protobuf_string(1, key) + protobuf_u64(2, protobuf_int_value(value)),
            )
        )
    return bytes(out)


def tron_transaction_bytes_hex(transaction):
    out = bytearray()
    out.extend(protobuf_bytes(1, bytes.fromhex(transaction["raw_data_hex"])))
    for signature in transaction.get("signature", []):
        out.extend(protobuf_bytes(2, bytes.fromhex(signature)))
    for result in transaction.get("ret", []):
        out.extend(protobuf_bytes(5, tron_transaction_result_bytes(result)))
    return bytes(out).hex()


def tron_merkle_root_hex(transaction_bytes_hexes):
    hashes = [hashlib.sha256(bytes.fromhex(value)).digest() for value in transaction_bytes_hexes]
    if not hashes:
        return "00" * 32
    while len(hashes) > 1:
        next_hashes = []
        for index in range(0, len(hashes), 2):
            if index + 1 >= len(hashes):
                next_hashes.append(hashes[index])
            else:
                next_hashes.append(
                    hashlib.sha256(hashes[index] + hashes[index + 1]).digest()
                )
        hashes = next_hashes
    return hashes[0].hex()


def tron_block_header_raw_data_hex(
    *,
    tx_trie_root_hex,
    number=123,
    parent_hash_hex="66" * 32,
    witness_address_hex="41" + "77" * 20,
    version=34,
    timestamp=456000,
    account_state_root_hex=None,
):
    fields = [
        protobuf_u64(1, timestamp),
        protobuf_bytes(2, bytes.fromhex(tx_trie_root_hex)),
        protobuf_bytes(3, bytes.fromhex(parent_hash_hex)),
        protobuf_u64(7, number),
        protobuf_bytes(9, bytes.fromhex(witness_address_hex)),
        protobuf_u64(10, version),
    ]
    if account_state_root_hex is not None:
        fields.append(protobuf_bytes(11, bytes.fromhex(account_state_root_hex)))
    raw_data = b"".join(
        fields
    )
    return raw_data.hex()


def tron_block_id_hex(number, raw_data_hex):
    raw_hash = hashlib.sha256(bytes.fromhex(raw_data_hex)).digest()
    return number.to_bytes(8, "big").hex() + raw_hash[8:].hex()


def tron_signature_hex(module, message_hash, nonce_start=2):
    scalar_order = module.SECP256K1_SCALAR_ORDER
    private_key = 1
    z = int.from_bytes(message_hash, "big")
    for nonce in range(nonce_start, nonce_start + 128):
        point = module._secp256k1_scalar_mul(nonce, module.SECP256K1_GENERATOR)
        assert point is not None
        x, y = point
        r = x % scalar_order
        if r == 0:
            continue
        s = (pow(nonce, -1, scalar_order) * (z + r * private_key)) % scalar_order
        if s == 0:
            continue
        recovery_id = (1 if y & 1 else 0) + (2 if x >= scalar_order else 0)
        if s > module.SECP256K1_SCALAR_HALF_ORDER:
            s = scalar_order - s
            recovery_id ^= 1
        signature = r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([recovery_id])
        if module._tron_recovered_signature_address20(message_hash, signature) == TRON_TEST_OWNER20:
            return signature.hex()
    raise AssertionError("could not build recoverable TRON header signature")


def tron_signature_hex_for_private_key(module, message_hash, private_key, nonce_start=2):
    scalar_order = module.SECP256K1_SCALAR_ORDER
    z = int.from_bytes(message_hash, "big")
    for nonce in range(nonce_start, nonce_start + 128):
        point = module._secp256k1_scalar_mul(nonce, module.SECP256K1_GENERATOR)
        assert point is not None
        x, y = point
        r = x % scalar_order
        if r == 0:
            continue
        s = (pow(nonce, -1, scalar_order) * (z + r * private_key)) % scalar_order
        if s == 0:
            continue
        recovery_id = (1 if y & 1 else 0) + (2 if x >= scalar_order else 0)
        if s > module.SECP256K1_SCALAR_HALF_ORDER:
            s = scalar_order - s
            recovery_id ^= 1
        signature = r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([recovery_id])
        if module._tron_recovered_signature_address20(message_hash, signature) is not None:
            return signature.hex()
    raise AssertionError("could not build recoverable TRON signature")


def tron_high_s_signature_hex(module, signature_hex):
    raw = bytes.fromhex(signature_hex.removeprefix("0x"))
    s = int.from_bytes(raw[32:64], "big")
    high_s = module.SECP256K1_SCALAR_ORDER - s
    assert high_s > module.SECP256K1_SCALAR_HALF_ORDER
    return (raw[:32] + high_s.to_bytes(32, "big") + raw[64:65]).hex()


def tron_header_signature_hex(module, raw_data_hex, nonce_start=2):
    return tron_signature_hex(
        module,
        hashlib.sha256(bytes.fromhex(raw_data_hex)).digest(),
        nonce_start=nonce_start,
    )


def tron_witness_schedule_payload_hex(addresses, weights):
    payload = bytearray([1])
    payload.extend(len(addresses).to_bytes(4, "little"))
    for address, weight in zip(addresses, weights):
        payload.extend(bytes.fromhex(address.removeprefix("0x")))
        payload.extend(int(weight).to_bytes(8, "little"))
    return payload.hex()


def tron_live_witness_seal_material(
    module,
    fake,
    witness_payload_hex,
    *,
    receipt_root_hex="ab" * 32,
    receipt_proof_hash_hex="cd" * 32,
    parent_tx_root_hex="dd" * 32,
    child_account_root_hex="ee" * 32,
    parent_account_root_hex="aa" * 32,
    ancestor_depth=0,
):
    expected_transaction_bytes = tron_source_event_transaction_bytes_hex()
    expected_dummy_bytes = tron_dummy_transaction_bytes_hex()
    tx_trie_root = tron_merkle_root_hex(
        [expected_dummy_bytes, expected_transaction_bytes]
    )
    parent_parent_hash = "55" * 32
    for number in range(122 - ancestor_depth, 122):
        ancestor_raw_data = tron_block_header_raw_data_hex(
            number=number,
            tx_trie_root_hex="cc" * 32,
            parent_hash_hex=parent_parent_hash,
            witness_address_hex="41" + fake.owner20.hex(),
            timestamp=453000 - (122 - number) * 3_000,
            account_state_root_hex="99" * 32,
        )
        parent_parent_hash = tron_block_id_hex(number, ancestor_raw_data)
    parent_header_raw_data = tron_block_header_raw_data_hex(
        number=122,
        tx_trie_root_hex=parent_tx_root_hex,
        parent_hash_hex=parent_parent_hash,
        witness_address_hex="41" + fake.owner20.hex(),
        timestamp=453000,
        account_state_root_hex=parent_account_root_hex,
    )
    parent_block_id = tron_block_id_hex(122, parent_header_raw_data)
    header_raw_data = tron_block_header_raw_data_hex(
        tx_trie_root_hex=tx_trie_root,
        parent_hash_hex=parent_block_id,
        witness_address_hex="41" + fake.owner20.hex(),
        account_state_root_hex=child_account_root_hex,
    )
    block_id = tron_block_id_hex(123, header_raw_data)
    schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(witness_payload_hex)
    )
    solid_block_message_input = {
        "source_domain": module.sccp_client.SCCP_DOMAIN_TRON,
        "solid_block_number": 123,
        "block_hash": "0x" + block_id,
        "witness_schedule_hash": schedule_hash,
        "receipt_root": "0x" + receipt_root_hex,
        "transaction_root": "0x" + tx_trie_root,
        "receipt_proof_hash": "0x" + receipt_proof_hash_hex,
    }
    solid_block_message_hash = module.sccp_client.tron_solid_block_message_hash(
        solid_block_message_input
    )
    signature = tron_signature_hex(
        module,
        bytes.fromhex(solid_block_message_hash.removeprefix("0x")),
        nonce_start=9,
    )
    witness_payload = bytes.fromhex(witness_payload_hex)
    witness_weight = int.from_bytes(witness_payload[26:34], "little")
    seal_input = {
        "version": 1,
        "total_weight": witness_weight,
        "signed_weight": witness_weight,
        "solid_block_message_hash": solid_block_message_hash,
        "witness_addresses": ["0x41" + fake.owner20.hex()],
        "witness_weights": [witness_weight],
        "signers_bitmap": "0x01",
        "signatures": ["0x" + signature],
    }
    return SimpleNamespace(
        block_id=block_id,
        parent_block_id=parent_block_id,
        tx_trie_root=tx_trie_root,
        solid_block_message_input=solid_block_message_input,
        solid_block_message_hash=solid_block_message_hash,
        signature=signature,
        seal_input=seal_input,
        seal_hash=module.sccp_client.tron_witness_seal_hash(seal_input),
        receipt_root=bytes.fromhex(receipt_root_hex),
        receipt_proof_hash=bytes.fromhex(receipt_proof_hash_hex),
        parent_tx_root=parent_tx_root_hex,
        child_account_root=child_account_root_hex,
        parent_account_root=parent_account_root_hex,
    )


def metadata_bytecode_hex(value):
    return value if isinstance(value, str) else value.hex()


def load_live_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_tron_live_evidence.py"
    )
    spec = spec_from_file_location("sccp_tron_live_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


class FakeResponse:
    def __init__(self, payload):
        self.payload = json.dumps(payload).encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        return False

    def close(self):
        return None

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


class RawResponse:
    def __init__(self, payload):
        self.payload = payload

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        return False

    def close(self):
        return None

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


def test_tron_api_response_size_is_bounded():
    module = load_live_module()
    payload = b'{"result":"' + (b"a" * module.TRON_API_MAX_RESPONSE_BYTES) + b'"}'

    def oversized_opener(_request, timeout):
        del timeout
        return RawResponse(payload)

    try:
        module._post_json(
            "https://tron.example",
            "/wallet/getnowblock",
            {},
            opener=oversized_opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        assert "response exceeds" in str(exc)
    else:
        raise AssertionError("oversized TRON API response was accepted")


def test_tron_api_http_error_detail_is_bounded():
    module = load_live_module()
    error_body = b"secret-token-tron-error" * (
        module.TRON_API_MAX_ERROR_BYTES + 128
    )

    def error_opener(_request, timeout):
        del timeout
        raise urllib.error.HTTPError(
            "https://tron.example/wallet/getnowblock",
            500,
            "boom",
            {},
            RawResponse(error_body),
        )

    try:
        module._post_json(
            "https://tron.example",
            "/wallet/getnowblock",
            {},
            opener=error_opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TRON API /wallet/getnowblock failed with HTTP 500"
        assert "secret-token" not in message
        assert len(message) < 100
    else:
        raise AssertionError("oversized TRON API error body was accepted")


def test_tron_api_rejects_duplicate_json_keys():
    module = load_live_module()
    duplicate_payload = (
        b'{"secret-token-result":{"result":true},'
        b'"secret-token-result":{"result":false},"result":{"result":true}}'
    )

    def duplicate_json_opener(_request, timeout):
        del timeout
        return RawResponse(duplicate_payload)

    try:
        module._post_json(
            "https://tron.example",
            "/wallet/triggerconstantcontract",
            {},
            opener=duplicate_json_opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == (
            "TRON API /wallet/triggerconstantcontract returned duplicate JSON keys"
        )
        assert "secret-token" not in message
        assert "duplicate JSON key " not in message
        assert exc.__cause__ is None
    else:
        raise AssertionError("duplicate-key TRON API response was accepted")


def test_tron_api_redacts_transport_and_error_response_details():
    module = load_live_module()

    def secret_url_error_opener(_request, timeout):
        del timeout
        raise urllib.error.URLError(
            "secret-token provider URL leaked from transport"
        )

    def secret_error_object_opener(_request, timeout):
        del timeout
        return FakeResponse(
            {
                "Error": "secret-token TRON API error object",
            }
        )

    try:
        module._post_json(
            "https://tron.example",
            "/wallet/getnowblock",
            {},
            opener=secret_url_error_opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TRON API /wallet/getnowblock request failed"
        assert "secret-token" not in message
    else:
        raise AssertionError("secret-bearing TRON transport error was accepted")

    try:
        module._post_json(
            "https://tron.example",
            "/wallet/getnowblock",
            {},
            opener=secret_error_object_opener,
            timeout=1.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TRON API /wallet/getnowblock returned error response"
        assert "secret-token" not in message
        assert "error object" not in message
    else:
        raise AssertionError("secret-bearing TRON API error response was accepted")


def test_tron_api_redacts_exception_causes():
    module = load_live_module()

    def http_error_opener(_request, timeout):
        del timeout
        raise urllib.error.HTTPError(
            "https://tron.example/wallet/getnowblock?secret-token=http-url",
            502,
            "secret-token HTTP reason",
            {},
            RawResponse(b"secret-token HTTP response body"),
        )

    def transport_error_opener(_request, timeout):
        del timeout
        raise urllib.error.URLError("secret-token provider transport reason")

    def invalid_json_opener(_request, timeout):
        del timeout
        return RawResponse(b'{"result": "secret-token invalid JSON payload"')

    cases = (
        (
            http_error_opener,
            "TRON API /wallet/getnowblock failed with HTTP 502",
        ),
        (
            transport_error_opener,
            "TRON API /wallet/getnowblock request failed",
        ),
        (
            invalid_json_opener,
            "TRON API /wallet/getnowblock returned invalid JSON",
        ),
    )
    for opener, expected_message in cases:
        try:
            module._post_json(
                "https://tron.example",
                "/wallet/getnowblock",
                {},
                opener=opener,
                timeout=1.0,
            )
        except RuntimeError as exc:
            message = str(exc)
            assert message == expected_message
            assert "secret-token" not in message
            assert exc.__cause__ is None
            assert exc.__context__ is not None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("secret-bearing TRON API failure was accepted")


def test_tron_live_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_live_module()

    for exception_type in (OSError, RuntimeError, TypeError, ValueError):

        def fail_collect(_args, exception_type=exception_type):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "collect_live_evidence", fail_collect)
            try:
                module.main(
                    [
                        "--source-bridge-address",
                        module.tron_base58check_from_address20(TRON_TEST_BRIDGE20),
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError("TRON live CLI accepted top-level collection failure")

            captured = capsys.readouterr()
            assert "SCCP TRON live evidence collection failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_tron_api_key_is_runtime_exact_ascii(tmp_path):
    module = load_live_module()
    key_file = tmp_path / "trongrid.key"
    key_file.write_text("runtime-secret-key\r\n", encoding="utf-8")
    assert (
        module._runtime_tron_pro_api_key(
            SimpleNamespace(tron_pro_api_key=None, tron_pro_api_key_file=str(key_file))
        )
        == "runtime-secret-key"
    )
    assert (
        module._runtime_tron_pro_api_key(
            SimpleNamespace(tron_pro_api_key="runtime-secret-key", tron_pro_api_key_file=None)
        )
        == "runtime-secret-key"
    )

    for api_key in (
        "",
        " runtime-secret-key",
        "runtime-secret-key ",
        "runtime-secret-key\n",
        "runtime secret key",
        "runtime\tsecret",
        "runtime-secret-\u2603",
    ):
        try:
            module._runtime_tron_pro_api_key(
                SimpleNamespace(tron_pro_api_key=api_key, tron_pro_api_key_file=None)
            )
        except ValueError as exc:
            assert "TRON-PRO-API-KEY" in str(exc)
            if "\u2603" in api_key:
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
        else:
            raise AssertionError(f"non-exact TRON API key {api_key!r} was accepted")

    padded_key_file = tmp_path / "padded-trongrid.key"
    padded_key_file.write_text(" runtime-secret-key\n", encoding="utf-8")
    try:
        module._runtime_tron_pro_api_key(
            SimpleNamespace(
                tron_pro_api_key=None,
                tron_pro_api_key_file=str(padded_key_file),
            )
        )
    except ValueError as exc:
        assert "TRON-PRO-API-KEY" in str(exc)
    else:
        raise AssertionError("padded file-backed TRON API key was accepted")


def test_tron_runtime_input_parsers_redact_exception_causes(tmp_path):
    module = load_live_module()

    api_key_path = tmp_path / "secret-token-trongrid.key"
    payload_path = tmp_path / "secret-token-witness-schedule.hex"
    transition_path = tmp_path / "secret-token-transition.json"
    cases = (
        (
            lambda: module._runtime_tron_pro_api_key(
                SimpleNamespace(
                    tron_pro_api_key=None,
                    tron_pro_api_key_file=str(api_key_path),
                )
            ),
            "--tron-pro-api-key-file cannot be read",
        ),
        (
            lambda: module._runtime_witness_schedule_payload(
                SimpleNamespace(
                    witness_schedule_payload_hex=None,
                    witness_schedule_payload_file=str(payload_path),
                )
            ),
            "--witness-schedule-payload-file cannot be read",
        ),
        (
            lambda: module._runtime_witness_schedule_transitions(
                SimpleNamespace(
                    witness_schedule_transition_json=[f"@{transition_path}"],
                )
            ),
            "--witness-schedule-transition-json 0 file cannot be read",
        ),
        (
            lambda: module._runtime_witness_schedule_transitions(
                SimpleNamespace(
                    witness_schedule_transition_json=[
                        '{"secret-token invalid transition JSON"'
                    ],
                )
            ),
            "--witness-schedule-transition-json 0 must be JSON",
        ),
        (
            lambda: module._runtime_witness_schedule_transitions(
                SimpleNamespace(
                    witness_schedule_transition_json=[
                        (
                            '{"secret-token-duplicate-transition":"0x11",'
                            '"secret-token-duplicate-transition":"0x22"}'
                        )
                    ],
                )
            ),
            (
                "--witness-schedule-transition-json 0 must not contain "
                "duplicate JSON keys"
            ),
        ),
    )
    for action, expected_message in cases:
        try:
            action()
        except ValueError as exc:
            message = str(exc)
            assert message == expected_message
            assert "secret-token" not in message
            assert "invalid transition JSON" not in message
            assert "duplicate-transition" not in message
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("secret-bearing runtime input was accepted")


def test_witness_schedule_transition_json_rejects_duplicate_keys(tmp_path):
    module = load_live_module()
    duplicate_transition = (
        '{"secret-token-parent-witness-schedule-payload":"0x11",'
        '"secret-token-parent-witness-schedule-payload":"0x22"}'
    )
    for value in (duplicate_transition, None):
        if value is None:
            transition_file = tmp_path / "transition.json"
            transition_file.write_text(duplicate_transition, encoding="utf-8")
            value = f"@{transition_file}"
        try:
            module._runtime_witness_schedule_transitions(
                SimpleNamespace(witness_schedule_transition_json=[value])
            )
        except ValueError as exc:
            rendered = str(exc)
            assert "duplicate JSON keys" in rendered
            assert "duplicate JSON keys:" not in rendered
            assert "secret-token" not in rendered
            assert "parent-witness-schedule" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("duplicate transition JSON key was accepted")


def abi_word_u32(value):
    return value.to_bytes(32, "big")


def abi_word_address(address20):
    return b"\x00" * 12 + address20


def tron_route_canary_submit_call_data(
    *,
    message_id=bytes.fromhex("dd" * 32),
    source_domain=0,
    target_domain=5,
    commitment_root=bytes.fromhex("ee" * 32),
    statement_hash=bytes.fromhex("f1" * 32),
    payload_hash=bytes.fromhex("ab" * 32),
    finality_height=123,
    finality_block_hash=bytes.fromhex("cd" * 32),
):
    proof_words = [
        abi_word_u32(1),
        message_id,
        abi_word_u32(source_domain),
        commitment_root,
        *(bytes([index]) * 32 for index in range(1, 9)),
    ]
    proof_bytes = b"".join(proof_words)
    public_inputs = [
        message_id,
        payload_hash,
        abi_word_u32(target_domain),
        commitment_root,
        abi_word_u32(finality_height),
        finality_block_hash,
    ]
    call_data = bytearray(bytes.fromhex("bd57826c"))
    call_data.extend(abi_word_u32(32 * 8))
    for word in public_inputs:
        call_data.extend(word)
    call_data.extend(statement_hash)
    call_data.extend(abi_word_u32(len(proof_bytes)))
    call_data.extend(proof_bytes)
    return bytes(call_data)


TRON_ROUTE_CANARY_CALL_DATA_VECTOR = tron_route_canary_submit_call_data().hex()


def tron_route_canary_raw_data_hex(
    *,
    owner20=TRON_TEST_OWNER20,
    verifier20=bytes.fromhex("44" * 20),
    call_data=bytes.fromhex(TRON_ROUTE_CANARY_CALL_DATA_VECTOR),
):
    return tron_source_event_raw_data_hex(
        owner20=owner20,
        bridge20=verifier20,
        call_data=call_data,
    )


TRON_ROUTE_CANARY_RAW_DATA_HEX_VECTOR = tron_route_canary_raw_data_hex()
TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR = hashlib.sha256(
    bytes.fromhex(TRON_ROUTE_CANARY_RAW_DATA_HEX_VECTOR)
).hexdigest()


def fake_opener_for(
    module,
    *,
    source_config_override=None,
    source_source_domain=5,
    source_target_domain=0,
    source_runtime_bytecode=DEFAULT_SOURCE_RUNTIME_BYTECODE,
    source_metadata_address_override=None,
    destination_network_id_override=None,
    destination_binding_override=None,
    destination_runtime_bytecode=None,
    destination_metadata_address_override=None,
    destination_code_hash_override=None,
    destination_backend_hash_override=None,
    destination_proof_family_hash_override=None,
    submitted_source_event_digests=(),
    submitted_source_event_word_override=None,
    source_event_transaction_id=None,
    source_event_transaction_digest=None,
    source_event_transaction_status="SUCCESS",
    source_event_transaction_address_override=None,
    source_event_transaction_topic0_override=None,
    source_event_transaction_topics_extra=(),
    source_event_transaction_data="",
    source_event_transaction_duplicate_matching_log=False,
    source_event_transaction_log_index_override=None,
    source_event_transaction_log_index_fields=("logIndex",),
    source_event_transaction_info_id_alias_overrides=None,
    source_event_transaction_block_number=DEFAULT_TRANSACTION_INFO_FIELD,
    source_event_transaction_block_timestamp=DEFAULT_TRANSACTION_INFO_FIELD,
    source_event_transaction_owner_override=None,
    source_event_transaction_contract_override=None,
    source_event_transaction_call_data_override=None,
    source_event_transaction_type_override="TriggerSmartContract",
    source_event_transaction_type_url_override=(
        "type.googleapis.com/protocol.TriggerSmartContract"
    ),
    source_event_transaction_ret=None,
    source_event_transaction_raw_data_hex=TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR,
    source_event_transaction_signatures=None,
    source_event_transaction_include_txid=True,
    source_event_transaction_id_alias_overrides=None,
    source_event_block_number=123,
    source_event_block_timestamp=456000,
    source_event_block_id_override=None,
    source_event_block_tx_trie_root_override=None,
    source_event_block_prefix_transactions=(),
    source_event_block_transactions_override=None,
    source_event_child_parent_hash_override=None,
    source_event_parent_block_id_override=None,
    source_event_parent_block_timestamp=453000,
    source_event_parent_block_tx_trie_root="00" * 32,
    source_event_block_account_state_root=None,
    source_event_parent_block_account_state_root=None,
    source_event_block_transaction_id_alias_overrides=None,
    source_event_block_witness_signature_override=None,
    source_event_parent_block_witness_signature_override=None,
    source_event_ancestor_depth=0,
    source_event_confirmation_depth=0,
    route_canary_transaction_id=None,
    route_canary_transaction_status="SUCCESS",
    route_canary_transaction_address_override=None,
    route_canary_transaction_topic0_override=None,
    route_canary_transaction_message_id=None,
    route_canary_transaction_source_domain=0,
    route_canary_transaction_data_override=None,
    route_canary_transaction_destination_binding_override=None,
    route_canary_transaction_backend_hash_override=None,
    route_canary_transaction_proof_family_hash_override=None,
    route_canary_transaction_network_id_override=None,
    route_canary_transaction_duplicate_matching_log=False,
    route_canary_transaction_log_index_override=None,
    route_canary_transaction_log_index_fields=("logIndex",),
    route_canary_transaction_info_id_alias_overrides=None,
    route_canary_transaction_block_number=DEFAULT_TRANSACTION_INFO_FIELD,
    route_canary_transaction_block_timestamp=DEFAULT_TRANSACTION_INFO_FIELD,
    route_canary_used_message_proof=True,
    route_canary_used_message_proof_word_override=None,
    route_canary_transaction_owner_override=None,
    route_canary_transaction_contract_override=None,
    route_canary_transaction_call_data_override=None,
    route_canary_transaction_ret=None,
    route_canary_transaction_type_override="TriggerSmartContract",
    route_canary_transaction_type_url_override=(
        "type.googleapis.com/protocol.TriggerSmartContract"
    ),
    route_canary_transaction_raw_data_hex=TRON_ROUTE_CANARY_RAW_DATA_HEX_VECTOR,
    route_canary_transaction_signatures=None,
    route_canary_transaction_include_txid=True,
    route_canary_transaction_id_alias_overrides=None,
    route_canary_block_number=234,
    route_canary_block_timestamp=567000,
    expected_api_key=None,
    expected_constant_endpoint="/wallet/triggerconstantcontract",
    expected_transaction_endpoint="/wallet/gettransactioninfobyid",
    expected_transaction_by_id_endpoint="/wallet/gettransactionbyid",
    expected_block_endpoint="/wallet/getblockbynum",
):
    network_id = bytes.fromhex("33" * 32)
    bridge20 = TRON_TEST_BRIDGE20
    owner20 = TRON_TEST_OWNER20
    destination_code_hash = destination_code_hash_override or bytes.fromhex("bb" * 32)
    destination_key_hash = bytes.fromhex("cc" * 32)
    destination_backend_hash = (
        destination_backend_hash_override
        if destination_backend_hash_override is not None
        else module.evidence._keccak_256(
            module.evidence.TRON_GROTH16_BACKEND.encode("utf-8")
        )
    )
    destination_proof_family_hash = (
        destination_proof_family_hash_override
        if destination_proof_family_hash_override is not None
        else module.evidence._keccak_256(
            module.evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
        )
    )
    destination20 = bytes.fromhex("44" * 20)
    bridge = module.tron_base58check_from_address20(bridge20)
    destination = module.tron_base58check_from_address20(destination20)
    source_config = module.evidence.tron_source_bridge_config_hash(
        bridge_address=bridge20,
        network_id=network_id,
        source_domain=5,
        target_domain=0,
        owner_address=owner20,
    )
    if source_config_override is not None:
        source_config = source_config_override
    destination_network_id = (
        destination_network_id_override
        if destination_network_id_override is not None
        else network_id
    )
    destination_binding = module.evidence.tron_destination_binding_hash(
        network_id=destination_network_id,
        source_domain=0,
        target_domain=5,
        verifier_address=destination,
        verifier_code_hash=destination_code_hash,
        verifier_key_hash=destination_key_hash,
    )
    if destination_binding_override is not None:
        destination_binding = destination_binding_override
    runtime_bytecode = (
        bytes.fromhex("6001600055")
        if source_runtime_bytecode is DEFAULT_SOURCE_RUNTIME_BYTECODE
        else source_runtime_bytecode
    )

    constant_words = {
        (bridge, "networkId()"): network_id,
        (bridge, "sourceDomain()"): abi_word_u32(source_source_domain),
        (bridge, "targetDomain()"): abi_word_u32(source_target_domain),
        (bridge, "owner()"): abi_word_address(owner20),
        (bridge, "sourceBridgeConfigHash()"): source_config,
        (destination, "networkId()"): destination_network_id,
        (destination, "expectedSourceDomain()"): abi_word_u32(0),
        (destination, "expectedTargetDomain()"): abi_word_u32(5),
        (destination, "verifierCodeHash()"): destination_code_hash,
        (destination, "verifierKeyHash()"): destination_key_hash,
        (destination, "verifierBackendHash()"): destination_backend_hash,
        (destination, "proofFamilyHash()"): destination_proof_family_hash,
        (destination, "destinationBindingHash()"): destination_binding,
    }
    submitted_source_event_digests = {
        bytes.fromhex(str(digest).removeprefix("0x"))
        for digest in submitted_source_event_digests
    }

    def source_event_transaction_object():
        assert source_event_transaction_id is not None
        owner_address = source_event_transaction_owner_override or (
            "41" + owner20.hex()
        )
        contract_address = source_event_transaction_contract_override or (
            "41" + bridge20.hex()
        )
        call_data = (
            source_event_transaction_call_data_override
            or TRON_SOURCE_EVENT_CALL_DATA_VECTOR
        )
        ret = (
            source_event_transaction_ret
            if source_event_transaction_ret is not None
            else [{"contractRet": "SUCCESS"}]
        )
        signatures = (
            source_event_transaction_signatures
            if source_event_transaction_signatures is not None
            else [TRON_SOURCE_EVENT_SIGNATURE_VECTOR]
        )
        transaction = {
            "ret": ret,
            "raw_data_hex": source_event_transaction_raw_data_hex,
            "signature": signatures,
            "raw_data": {
                "contract": [
                    {
                        "type": source_event_transaction_type_override,
                        "parameter": {
                            "type_url": source_event_transaction_type_url_override,
                            "value": {
                                "owner_address": owner_address,
                                "contract_address": contract_address,
                                "data": call_data,
                            },
                        },
                    }
                ]
            },
        }
        if source_event_transaction_include_txid:
            transaction["txID"] = source_event_transaction_id
        if source_event_transaction_id_alias_overrides is not None:
            transaction.update(source_event_transaction_id_alias_overrides)
        return transaction

    def route_canary_transaction_info():
        assert route_canary_transaction_id is not None
        message_id = route_canary_transaction_message_id or bytes.fromhex("dd" * 32)
        commitment_root = bytes.fromhex("ee" * 32)
        statement_hash = bytes.fromhex("f1" * 32)
        canary_destination_binding = (
            route_canary_transaction_destination_binding_override
            if route_canary_transaction_destination_binding_override is not None
            else destination_binding
        )
        canary_backend_hash = (
            route_canary_transaction_backend_hash_override
            if route_canary_transaction_backend_hash_override is not None
            else destination_backend_hash
        )
        canary_proof_family_hash = (
            route_canary_transaction_proof_family_hash_override
            if route_canary_transaction_proof_family_hash_override is not None
            else destination_proof_family_hash
        )
        canary_network_id = (
            route_canary_transaction_network_id_override
            if route_canary_transaction_network_id_override is not None
            else destination_network_id
        )
        data = route_canary_transaction_data_override or b"".join(
            (
                commitment_root,
                statement_hash,
                canary_destination_binding,
                canary_backend_hash,
                canary_proof_family_hash,
                canary_network_id,
            )
        ).hex()
        event_topic0 = (
            route_canary_transaction_topic0_override
            or module.TRON_MESSAGE_PROOF_ACCEPTED_TOPIC.hex()
        )
        event_address = route_canary_transaction_address_override or destination20.hex()
        event_log = {
            "address": event_address,
            "topics": [
                event_topic0,
                message_id.hex(),
                abi_word_u32(route_canary_transaction_source_domain).hex(),
            ],
            "data": data,
        }
        if route_canary_transaction_log_index_override is not None:
            for field in route_canary_transaction_log_index_fields:
                event_log[field] = route_canary_transaction_log_index_override
        logs = [event_log]
        if route_canary_transaction_duplicate_matching_log:
            logs.append(dict(event_log))
        transaction_info = {
            "id": route_canary_transaction_id,
            "blockNumber": transaction_info_field(
                route_canary_block_number,
                route_canary_transaction_block_number,
            ),
            "blockTimeStamp": transaction_info_field(
                route_canary_block_timestamp,
                route_canary_transaction_block_timestamp,
            ),
            "receipt": {"result": route_canary_transaction_status},
            "log": logs,
        }
        if route_canary_transaction_info_id_alias_overrides is not None:
            transaction_info.update(route_canary_transaction_info_id_alias_overrides)
        return transaction_info

    def route_canary_transaction_object():
        assert route_canary_transaction_id is not None
        signatures = (
            route_canary_transaction_signatures
            if route_canary_transaction_signatures is not None
            else [
                tron_signature_hex(
                    module,
                    hashlib.sha256(
                        bytes.fromhex(route_canary_transaction_raw_data_hex)
                    ).digest(),
                    nonce_start=17,
                )
            ]
        )
        owner_address = route_canary_transaction_owner_override or (
            "41" + owner20.hex()
        )
        contract_address = route_canary_transaction_contract_override or (
            "41" + destination20.hex()
        )
        call_data = (
            route_canary_transaction_call_data_override
            or TRON_ROUTE_CANARY_CALL_DATA_VECTOR
        )
        ret = (
            route_canary_transaction_ret
            if route_canary_transaction_ret is not None
            else [{"contractRet": "SUCCESS"}]
        )
        transaction = {
            "ret": ret,
            "signature": signatures,
            "raw_data_hex": route_canary_transaction_raw_data_hex,
            "raw_data": {
                "contract": [
                    {
                        "type": route_canary_transaction_type_override,
                        "parameter": {
                            "type_url": route_canary_transaction_type_url_override,
                            "value": {
                                "owner_address": owner_address,
                                "contract_address": contract_address,
                                "data": call_data,
                            },
                        },
                    }
                ]
            },
        }
        if route_canary_transaction_include_txid:
            transaction["txID"] = route_canary_transaction_id
        if route_canary_transaction_id_alias_overrides is not None:
            transaction.update(route_canary_transaction_id_alias_overrides)
        return transaction

    def opener(request, timeout):
        del timeout
        headers = {name.lower(): value for name, value in request.header_items()}
        if expected_api_key is None:
            assert "tron-pro-api-key" not in headers
        else:
            assert headers["tron-pro-api-key"] == expected_api_key
        payload = json.loads(request.data.decode("utf-8"))
        if request.full_url.endswith(expected_constant_endpoint):
            key = (payload["contract_address"], payload["function_selector"])
            if key == (bridge, "submittedSourceEvents(bytes32)"):
                parameter = payload["parameter"]
                assert isinstance(parameter, str)
                assert len(parameter) == 64
                if submitted_source_event_word_override is not None:
                    word = submitted_source_event_word_override
                else:
                    word = abi_word_u32(
                        bytes.fromhex(parameter)
                        in submitted_source_event_digests
                    )
                return FakeResponse(
                    {
                        "result": {"result": True},
                        "constant_result": [word.hex()],
                    }
                )
            if key == (destination, "usedMessageProofs(bytes32)"):
                parameter = payload["parameter"]
                assert isinstance(parameter, str)
                assert len(parameter) == 64
                message_id = route_canary_transaction_message_id or bytes.fromhex(
                    "dd" * 32
                )
                assert bytes.fromhex(parameter) == message_id
                if route_canary_used_message_proof_word_override is not None:
                    word = route_canary_used_message_proof_word_override
                else:
                    word = abi_word_u32(1 if route_canary_used_message_proof else 0)
                return FakeResponse(
                    {
                        "result": {"result": True},
                        "constant_result": [word.hex()],
                    }
                )
            assert payload["parameter"] == ""
            return FakeResponse(
                {
                    "result": {"result": True},
                    "constant_result": [constant_words[key].hex()],
                }
            )
        if request.full_url.endswith(expected_transaction_endpoint):
            if (
                route_canary_transaction_id is not None
                and payload["value"] == route_canary_transaction_id
            ):
                return FakeResponse(route_canary_transaction_info())
            assert source_event_transaction_id is not None
            assert payload["value"] == source_event_transaction_id
            event_digest = (
                source_event_transaction_digest or TRON_SOURCE_EVENT_DIGEST_VECTOR
            )
            event_topic0 = (
                source_event_transaction_topic0_override
                or module.TRON_SOURCE_EVENT_TOPIC.hex()
            )
            event_address = source_event_transaction_address_override or bridge20.hex()
            event_log = {
                "address": event_address,
                "topics": [
                    event_topic0,
                    event_digest,
                    *source_event_transaction_topics_extra,
                ],
                "data": source_event_transaction_data,
            }
            if source_event_transaction_log_index_override is not None:
                for field in source_event_transaction_log_index_fields:
                    event_log[field] = source_event_transaction_log_index_override
            logs = [event_log]
            if source_event_transaction_duplicate_matching_log:
                logs.append(dict(event_log))
            transaction_info = {
                "id": source_event_transaction_id,
                "blockNumber": transaction_info_field(
                    source_event_block_number,
                    source_event_transaction_block_number,
                ),
                "blockTimeStamp": transaction_info_field(
                    source_event_block_timestamp,
                    source_event_transaction_block_timestamp,
                ),
                "receipt": {"result": source_event_transaction_status},
                "log": logs,
            }
            if source_event_transaction_info_id_alias_overrides is not None:
                transaction_info.update(source_event_transaction_info_id_alias_overrides)
            return FakeResponse(transaction_info)
        if request.full_url.endswith(expected_transaction_by_id_endpoint):
            if (
                route_canary_transaction_id is not None
                and payload["value"] == route_canary_transaction_id
            ):
                return FakeResponse(route_canary_transaction_object())
            assert source_event_transaction_id is not None
            assert payload["value"] == source_event_transaction_id
            return FakeResponse(source_event_transaction_object())
        if request.full_url.endswith(expected_block_endpoint):
            assert source_event_transaction_id is not None
            target_transaction = source_event_transaction_object()
            if source_event_block_transaction_id_alias_overrides is not None:
                target_transaction = dict(target_transaction)
                target_transaction.update(
                    source_event_block_transaction_id_alias_overrides
                )
            dummy_raw_data_hex = "01"
            dummy_transaction = {
                "txID": hashlib.sha256(bytes.fromhex(dummy_raw_data_hex)).hexdigest(),
                "raw_data_hex": dummy_raw_data_hex,
            }
            if source_event_block_transactions_override is not None:
                transactions = source_event_block_transactions_override
            elif source_event_block_prefix_transactions:
                transactions = [
                    *source_event_block_prefix_transactions,
                    target_transaction,
                ]
            else:
                transactions = [dummy_transaction, target_transaction]
            transaction_bytes_hexes = []
            for transaction in transactions:
                transaction_bytes_hexes.append(tron_transaction_bytes_hex(transaction))
            tx_trie_root = (
                source_event_block_tx_trie_root_override
                or tron_merkle_root_hex(transaction_bytes_hexes)
            )
            ancestor_blocks = {}
            next_parent_hash = "55" * 32
            deepest_ancestor = source_event_block_number - 1 - source_event_ancestor_depth
            for number in range(deepest_ancestor, source_event_block_number - 1):
                timestamp = source_event_parent_block_timestamp - (
                    source_event_block_number - 1 - number
                ) * 3_000
                ancestor_raw_data_hex = tron_block_header_raw_data_hex(
                    number=number,
                    tx_trie_root_hex="cc" * 32,
                    parent_hash_hex=next_parent_hash,
                    witness_address_hex="41" + owner20.hex(),
                    timestamp=timestamp,
                    account_state_root_hex="99" * 32,
                )
                ancestor_block_id = tron_block_id_hex(number, ancestor_raw_data_hex)
                ancestor_blocks[number] = {
                    "blockID": ancestor_block_id,
                    "block_header": {
                        "raw_data": {
                            "number": number,
                            "txTrieRoot": "cc" * 32,
                            "witness_address": "41" + owner20.hex(),
                            "parentHash": next_parent_hash,
                            "version": 34,
                            "timestamp": timestamp,
                            "accountStateRoot": "99" * 32,
                        },
                        "witness_signature": tron_header_signature_hex(
                            module,
                            ancestor_raw_data_hex,
                            6 + number,
                        ),
                    },
                    "transactions": [],
                }
                next_parent_hash = ancestor_block_id
            parent_parent_hash = next_parent_hash
            parent_header_raw_data_hex = tron_block_header_raw_data_hex(
                number=source_event_block_number - 1,
                tx_trie_root_hex=source_event_parent_block_tx_trie_root,
                parent_hash_hex=parent_parent_hash,
                witness_address_hex="41" + owner20.hex(),
                timestamp=source_event_parent_block_timestamp,
                account_state_root_hex=source_event_parent_block_account_state_root,
            )
            parent_witness_signature = (
                source_event_parent_block_witness_signature_override
                or tron_header_signature_hex(module, parent_header_raw_data_hex, 2)
            )
            parent_block_id = (
                source_event_parent_block_id_override
                or tron_block_id_hex(
                    source_event_block_number - 1,
                    parent_header_raw_data_hex,
                )
            )
            header_raw_data_hex = tron_block_header_raw_data_hex(
                number=source_event_block_number,
                tx_trie_root_hex=tx_trie_root,
                parent_hash_hex=(
                    source_event_child_parent_hash_override or parent_block_id
                ),
                witness_address_hex="41" + owner20.hex(),
                timestamp=source_event_block_timestamp,
                account_state_root_hex=source_event_block_account_state_root,
            )
            witness_signature = (
                source_event_block_witness_signature_override
                or tron_header_signature_hex(module, header_raw_data_hex, 4)
            )
            block_id = (
                source_event_block_id_override
                or tron_block_id_hex(source_event_block_number, header_raw_data_hex)
            )
            confirmation_blocks = {}
            confirmation_parent_hash = block_id
            for offset in range(1, source_event_confirmation_depth + 1):
                number = source_event_block_number + offset
                timestamp = source_event_block_timestamp + offset * 3_000
                confirmation_raw_data_hex = tron_block_header_raw_data_hex(
                    number=number,
                    tx_trie_root_hex="de" * 32,
                    parent_hash_hex=confirmation_parent_hash,
                    witness_address_hex="41" + owner20.hex(),
                    timestamp=timestamp,
                    account_state_root_hex="ef" * 32,
                )
                confirmation_block_id = tron_block_id_hex(
                    number,
                    confirmation_raw_data_hex,
                )
                confirmation_blocks[number] = {
                    "blockID": confirmation_block_id,
                    "block_header": {
                        "raw_data": {
                            "number": number,
                            "txTrieRoot": "de" * 32,
                            "witness_address": "41" + owner20.hex(),
                            "parentHash": confirmation_parent_hash,
                            "version": 34,
                            "timestamp": timestamp,
                            "accountStateRoot": "ef" * 32,
                        },
                        "witness_signature": tron_header_signature_hex(
                            module,
                            confirmation_raw_data_hex,
                            70 + offset,
                        ),
                    },
                    "transactions": [],
                }
                confirmation_parent_hash = confirmation_block_id
            allowed_numbers = {
                source_event_block_number,
                source_event_block_number - 1,
                *ancestor_blocks,
                *confirmation_blocks,
            }
            assert payload["num"] in allowed_numbers
            if payload["num"] in ancestor_blocks:
                return FakeResponse(ancestor_blocks[payload["num"]])
            if payload["num"] in confirmation_blocks:
                return FakeResponse(confirmation_blocks[payload["num"]])
            if payload["num"] == source_event_block_number - 1:
                parent_raw_data = {
                    "number": source_event_block_number - 1,
                    "txTrieRoot": source_event_parent_block_tx_trie_root,
                    "witness_address": "41" + owner20.hex(),
                    "parentHash": parent_parent_hash,
                    "version": 34,
                    "timestamp": source_event_parent_block_timestamp,
                }
                if source_event_parent_block_account_state_root is not None:
                    parent_raw_data["accountStateRoot"] = (
                        source_event_parent_block_account_state_root
                    )
                return FakeResponse(
                    {
                        "blockID": parent_block_id,
                        "block_header": {
                            "raw_data": parent_raw_data,
                            "witness_signature": parent_witness_signature,
                        },
                        "transactions": [],
                    }
                )
            child_raw_data = {
                "number": source_event_block_number,
                "txTrieRoot": tx_trie_root,
                "witness_address": "41" + owner20.hex(),
                "parentHash": (
                    source_event_child_parent_hash_override
                    or parent_block_id
                ),
                "version": 34,
                "timestamp": source_event_block_timestamp,
            }
            if source_event_block_account_state_root is not None:
                child_raw_data["accountStateRoot"] = source_event_block_account_state_root
            return FakeResponse(
                {
                    "blockID": block_id,
                    "block_header": {
                        "raw_data": child_raw_data,
                        "witness_signature": witness_signature,
                    },
                    "transactions": transactions,
                }
            )
        if request.full_url.endswith("/wallet/getcontract"):
            if payload["value"] == bridge:
                if runtime_bytecode is None:
                    return FakeResponse(
                        {"contract_address": source_metadata_address_override or bridge}
                    )
                return FakeResponse(
                    {
                        "contract_address": source_metadata_address_override or bridge,
                        "bytecode": metadata_bytecode_hex(runtime_bytecode),
                        "code_hash": "observer-code-hash",
                    }
                )
            if payload["value"] == destination and destination_runtime_bytecode is not None:
                return FakeResponse(
                    {
                        "contract_address": (
                            destination_metadata_address_override or destination
                        ),
                        "bytecode": metadata_bytecode_hex(destination_runtime_bytecode),
                        "code_hash": "destination-observer-code-hash",
                    }
                )
            return FakeResponse({"contract_address": payload["value"]})
        raise AssertionError(f"unexpected URL {request.full_url}")

    return SimpleNamespace(
        opener=opener,
        network_id=network_id,
        bridge=bridge,
        bridge20=bridge20,
        owner20=owner20,
        destination=destination,
        source_config=source_config,
        destination_code_hash=destination_code_hash,
        destination_key_hash=destination_key_hash,
        destination_binding=destination_binding,
        runtime_bytecode=runtime_bytecode,
        route_canary_transaction_id=route_canary_transaction_id,
        route_canary_call_data=bytes.fromhex(TRON_ROUTE_CANARY_CALL_DATA_VECTOR),
    )


def source_record_hashes_for(module, fake, *, source_code_hash):
    args = SimpleNamespace(
        source_domain=5,
        target_domain=0,
        bridge_address=fake.bridge20,
        owner_address=fake.owner20,
        network_id=fake.network_id,
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=source_code_hash,
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=None,
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
    )
    module.evidence.apply_source_adapter_verifier_vk_hash(args)
    material_hash = module.evidence.tron_source_verifier_material_record_hash(
        args,
        fake.source_config,
    )
    deployment_hash = (
        module.evidence.tron_source_adapter_engine_deployment_record_hash(
            args,
            fake.source_config,
        )
    )
    gate_hash = module.evidence.tron_dpos_source_gate_hash(
        args,
        fake.source_config,
    )
    route_hash = module.evidence.tron_route_allowlist_hash(
        source_verifier_material_hash=material_hash,
        source_adapter_engine_deployment_hash=deployment_hash,
        destination_binding_hash=fake.destination_binding,
    )
    return SimpleNamespace(
        args=args,
        material_hash=material_hash,
        deployment_hash=deployment_hash,
        gate_hash=gate_hash,
        route_hash=route_hash,
    )


def live_full_rollout_args(
    fake,
    expected,
    *,
    source_code_hash,
    route_canary_evidence_hash=None,
    route_canary_transaction_id=None,
):
    return SimpleNamespace(
        tron_node_url="https://tron.example",
        source_bridge_address=fake.bridge,
        destination_verifier_address=fake.destination,
        caller_address=None,
        no_getcontract=False,
        timeout=1.0,
        tron_pro_api_key=None,
        tron_pro_api_key_file=None,
        solid=False,
        source_trust_anchor_hash=expected.args.source_trust_anchor_hash,
        consensus_verifier_hash=expected.args.consensus_verifier_hash,
        message_inclusion_verifier_hash=(
            expected.args.message_inclusion_verifier_hash
        ),
        source_bridge_emitter_code_hash=source_code_hash,
        expected_source_bridge_config_hash=fake.source_config,
        finality_policy_hash=expected.args.finality_policy_hash,
        deployment_receipt_hash=expected.args.deployment_receipt_hash,
        adapter_verifier_vk_hash=None,
        expected_source_verifier_material_hash=expected.material_hash,
        expected_source_adapter_engine_deployment_hash=expected.deployment_hash,
        expected_tron_dpos_source_gate_hash=expected.gate_hash,
        expected_destination_binding_hash=fake.destination_binding,
        route_allowlist_hash=expected.route_hash,
        route_canary_evidence_hash=route_canary_evidence_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )


def route_canary_full_rollout_setup(module, **fake_kwargs):
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        **fake_kwargs,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )
    return SimpleNamespace(
        fake=fake,
        expected=expected,
        source_code_hash=source_code_hash,
    )


def test_live_tron_address_payload_parser_rejects_all_zero_base58check():
    module = load_live_module()

    for zero_address in [
        "0x41" + "00" * 20,
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
    ]:
        try:
            module.parse_tron_address_payload(
                zero_address,
                label="destination verifier address",
            )
        except module.evidence.argparse.ArgumentTypeError as exc:
            assert "must not be zero" in str(exc)
        else:
            raise AssertionError("zero TRON address payload was accepted")

    for padded_address in [
        " TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8\n",
    ]:
        try:
            module.parse_tron_address_payload(
                padded_address,
                label="destination verifier address",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "surrounding whitespace" in str(exc)
        else:
            raise AssertionError("padded TRON address payload was accepted")

    try:
        module.tron_base58check_from_payload(b"\x41" + bytes(20))
    except ValueError as exc:
        assert "non-zero" in str(exc)
    else:
        raise AssertionError("zero TRON payload was encoded")


def test_live_evidence_collects_source_destination_and_offline_args():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    source = summary["source_bridge"]
    assert summary["constant_endpoint"] == "wallet/triggerconstantcontract"
    assert summary["transaction_info_endpoint"] == "wallet/gettransactioninfobyid"
    assert summary["transaction_endpoint"] == "wallet/gettransactionbyid"
    assert source["address"] == fake.bridge
    assert source["source_domain"] == 5
    assert source["target_domain"] == 0
    assert source["config_hash_matches"] is True
    assert source["source_bridge_network_id"] == "0x" + "33" * 32
    assert source["source_bridge_owner_address"] == "0x" + TRON_TEST_OWNER20.hex()
    assert source["source_bridge_emitter_code_hash"] == "0x" + module.evidence.runtime_bytecode_hash(
        fake.runtime_bytecode
    ).hex()
    assert source["source_bridge_runtime_bytecode_hex"] == (
        "0x" + fake.runtime_bytecode.hex()
    )

    destination = summary["destination_verifier"]
    assert destination["address"] == fake.destination
    assert destination["destination_source_domain"] == 0
    assert destination["destination_target_domain"] == 5
    assert (
        destination["destination_verifier_code_hash"]
        == "0x" + fake.destination_code_hash.hex()
    )
    assert destination["destination_verifier_key_hash"] == "0x" + "cc" * 32
    assert destination["destination_binding_hash"] == "0x" + fake.destination_binding.hex()
    assert destination["destination_binding_hash_matches"] is True
    assert (
        destination["tron_getcontract_bytecode_hash"]
        == "0x" + fake.destination_code_hash.hex()
    )
    assert destination["destination_verifier_runtime_bytecode_hex"] == (
        "0x" + destination_runtime_bytecode.hex()
    )
    expected_backend_hash = module.evidence._keccak_256(
        module.evidence.TRON_GROTH16_BACKEND.encode("utf-8")
    )
    expected_proof_family_hash = module.evidence._keccak_256(
        module.evidence.SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")
    )
    assert destination["verifier_backend_hash"] == "0x" + expected_backend_hash.hex()
    assert destination["proof_family_hash"] == "0x" + expected_proof_family_hash.hex()
    assert destination["verifier_backend_hash_matches"] is True
    assert destination["proof_family_hash_matches"] is True
    assert destination["bytecode_hash_matches_verifier_code_hash"] is True
    assert destination["source_bridge_network_id_matches"] is True
    assert destination["destination_binding_key"].startswith("tron:0:5:333333")

    offline_args = summary["offline_evidence_args"]
    assert "--bridge-address" in offline_args
    assert fake.bridge in offline_args
    assert "--source-bridge-emitter-code-hash" in offline_args
    assert "--source-bridge-runtime-bytecode-hex" in offline_args
    assert "0x" + fake.runtime_bytecode.hex() in offline_args
    assert "--destination-verifier-address" in offline_args
    assert "--destination-verifier-runtime-bytecode-hex" in offline_args
    assert "0x" + destination_runtime_bytecode.hex() in offline_args
    assert fake.destination in offline_args
    assert "--expected-config-hash" not in offline_args
    assert "--expected-destination-binding-hash" not in offline_args
    assert summary["full_toml_ready"] is False

    assert "torii_destination_query_params" not in summary
    assert "torii_destination_query_proof_bytes_hex_required" not in summary


def test_live_evidence_emits_source_event_call_data_and_replay_args():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    source_event_call = summary["source_event_call"]
    assert source_event_call["source_bridge_address"] == fake.bridge
    assert source_event_call["source_domain"] == 5
    assert source_event_call["target_domain"] == 0
    assert source_event_call["source_event_digest"] == (
        "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR
    )
    assert source_event_call["source_event_call_data"] == (
        "0x" + TRON_SOURCE_EVENT_CALL_DATA_VECTOR
    )
    assert source_event_call["submitted_source_events_checked"] is True
    assert source_event_call["source_event_already_submitted"] is False
    assert source_event_call["trigger_request"] == {
        "endpoint": "wallet/triggersmartcontract",
        "owner_address": source_event_call["source_bridge_owner_base58"],
        "contract_address": fake.bridge,
        "function_selector": "submitSccpSourceEvent(uint32,uint32,bytes32)",
        "parameter": TRON_SOURCE_EVENT_CALL_DATA_VECTOR[8:],
        "visible": True,
        "call_value": 0,
    }
    assert source_event_call["transaction_required"] is True
    offline_source_event_args = summary["offline_source_event_args"]
    assert "--source-event-digest" in offline_source_event_args
    assert "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR in offline_source_event_args
    assert "--full-toml" not in offline_source_event_args


def test_live_evidence_source_event_replay_args_revalidate_call_binding():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_call"]["source_event_call_data"] = "0x" + "99" * 100
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_call"]["source_domain"] = 6
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_call"]["trigger_request"]["parameter"] = "00" * 96
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    tampered["offline_evidence_args"] = ["--bridge-address", "poisoned"]
    replay_args = module._offline_source_event_args(tampered)
    assert replay_args is not None
    assert "poisoned" not in replay_args
    assert fake.bridge in replay_args


def test_live_evidence_rejects_already_submitted_source_event_digest():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "already been submitted" in str(exc)
    else:
        raise AssertionError("already submitted source-event digest emitted calldata")


def test_live_evidence_verifies_source_event_transaction_readback():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    source_event_call = summary["source_event_call"]
    assert source_event_call["source_event_already_submitted"] is True
    assert source_event_call["transaction_required"] is False
    assert "trigger_request" not in source_event_call

    transaction = summary["source_event_transaction"]
    replay_args = module._offline_source_event_args(summary)
    assert replay_args is not None
    assert "--source-event-digest" in replay_args
    assert transaction["transaction_id"] == (
        "0x" + TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
    )
    assert transaction["receipt_status"] == "SUCCESS"
    assert transaction["log_index"] == 0
    assert transaction["event_address"] == "0x" + fake.bridge20.hex()
    assert transaction["event_topic0"] == "0x" + module.TRON_SOURCE_EVENT_TOPIC.hex()
    assert transaction["source_event_digest"] == (
        "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR
    )
    assert transaction["event_data"] == "0x"
    assert transaction["event_matches"] is True
    assert transaction["block_number"] == 123
    assert transaction["block_timestamp"] == 456000
    solid_block = transaction["solid_block"]
    expected_transaction_bytes = tron_source_event_transaction_bytes_hex()
    expected_dummy_bytes = tron_dummy_transaction_bytes_hex()
    expected_tx_trie_root = tron_merkle_root_hex(
        [expected_dummy_bytes, expected_transaction_bytes]
    )
    expected_parent_header_raw_data = tron_block_header_raw_data_hex(
        number=122,
        tx_trie_root_hex="00" * 32,
        parent_hash_hex="55" * 32,
        witness_address_hex="41" + fake.owner20.hex(),
        timestamp=453000,
    )
    expected_parent_block_id = tron_block_id_hex(122, expected_parent_header_raw_data)
    expected_header_raw_data = tron_block_header_raw_data_hex(
        tx_trie_root_hex=expected_tx_trie_root,
        parent_hash_hex=expected_parent_block_id,
        witness_address_hex="41" + fake.owner20.hex(),
    )
    assert solid_block["block_id"] == (
        "0x" + tron_block_id_hex(123, expected_header_raw_data)
    )
    assert solid_block["block_number"] == 123
    assert solid_block["block_timestamp"] == 456000
    assert solid_block["block_parent_hash"] == "0x" + expected_parent_block_id
    assert solid_block["block_tx_trie_root"] == "0x" + expected_tx_trie_root
    assert solid_block["block_account_state_root"] is None
    assert solid_block["block_witness_address"] == "0x41" + fake.owner20.hex()
    assert len(solid_block["block_witness_signature"]) == 132
    assert solid_block["block_witness_signature_recovered_address"] == (
        "0x41" + fake.owner20.hex()
    )
    assert solid_block["block_header_raw_data_bytes"] == (
        "0x" + expected_header_raw_data
    )
    assert solid_block["block_header_raw_data_hash"] == (
        "0x" + hashlib.sha256(bytes.fromhex(expected_header_raw_data)).hexdigest()
    )
    assert solid_block["block_id_matches_header"] is True
    assert solid_block["parent_block_id"] == "0x" + expected_parent_block_id
    assert solid_block["parent_block_number"] == 122
    assert solid_block["parent_block_timestamp"] == 453000
    assert solid_block["parent_block_tx_trie_root"] == "0x" + "00" * 32
    assert solid_block["parent_block_account_state_root"] is None
    assert solid_block["parent_block_witness_address"] == "0x41" + fake.owner20.hex()
    assert len(solid_block["parent_block_witness_signature"]) == 132
    assert solid_block["parent_block_witness_signature_recovered_address"] == (
        "0x41" + fake.owner20.hex()
    )
    assert solid_block["parent_block_header_raw_data_bytes"] == (
        "0x" + expected_parent_header_raw_data
    )
    assert solid_block["parent_block_header_raw_data_hash"] == (
        "0x"
        + hashlib.sha256(bytes.fromhex(expected_parent_header_raw_data)).hexdigest()
    )
    assert solid_block["parent_block_id_matches_header"] is True
    assert solid_block["parent_block_link_checked"] is True
    assert solid_block["parent_timestamp_before_child"] is True
    assert solid_block["transaction_count"] == 2
    assert solid_block["transaction_index"] == 1
    assert solid_block["transaction_merkle_branch"] == [
        "0x" + hashlib.sha256(bytes.fromhex(expected_dummy_bytes)).hexdigest()
    ]
    assert solid_block["transaction_merkle_branch_length"] == 1
    assert solid_block["transaction_source_proof_ready"] is False
    assert solid_block["transaction_source_proof_blocker"] == (
        "receipt root required; source inclusion branch required"
    )
    assert solid_block["source_proof_transaction_hash"] == (
        "0x" + hashlib.sha256(bytes.fromhex(expected_transaction_bytes)).hexdigest()
    )
    assert solid_block["calculated_tx_trie_root"] == "0x" + expected_tx_trie_root
    assert solid_block["tx_trie_root_matches"] is True
    assert solid_block["block_transaction_root_checked"] is True
    assert solid_block["solid_block_header_proof_ready"] is False
    assert solid_block["solid_block_header_proof_blocker"] == (
        "child accountStateRoot missing or zero"
    )
    assert solid_block["witness_schedule_proof_ready"] is False
    assert solid_block["witness_schedule_proof_blocker"] == (
        "active witness schedule payload required"
    )
    assert solid_block["witness_seal_proof_ready"] is False
    assert solid_block["witness_seal_proof_blocker"] == (
        "active witness schedule payload required; receipt root required; "
        "receipt proof hash required; witness seal signers bitmap required; "
        "witness seal signatures required"
    )
    assert solid_block["solid_block_ancestor_headers_ready"] is False
    assert solid_block["solid_block_ancestor_headers_blocker"] == (
        "at least one signed ancestor header required for non-placeholder TRON material"
    )
    assert solid_block["solid_block_ancestor_header_count"] == 0
    assert solid_block["solid_block_confirmation_headers_ready"] is False
    assert solid_block["solid_block_confirmation_headers_blocker"] == (
        "confirmation headers required for non-placeholder TRON material"
    )
    assert solid_block["solid_block_confirmation_header_count"] == 0
    assert solid_block["signed_header_proof_required"] is True
    assert transaction["source_event_transaction_production_ready"] is False
    assert transaction["source_event_transaction_production_blockers"] == [
        "transaction source proof: receipt root required; source inclusion branch required",
        "solid block header proof: child accountStateRoot missing or zero",
        "witness schedule proof: active witness schedule payload required",
        (
            "witness seal proof: active witness schedule payload required; "
            "receipt root required; receipt proof hash required; "
            "witness seal signers bitmap required; witness seal signatures required"
        ),
        (
            "solid block ancestor headers: at least one signed ancestor header "
            "required for non-placeholder TRON material"
        ),
        (
            "solid block confirmation headers: confirmation headers required for "
            "non-placeholder TRON material"
        ),
    ]
    trigger_contract = transaction["trigger_contract"]
    assert trigger_contract["transaction_id"] == (
        "0x" + TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
    )
    assert trigger_contract["raw_data_hex"] == (
        "0x" + TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR
    )
    assert trigger_contract["raw_data_sha256"] == (
        "0x" + TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
    )
    assert trigger_contract["transaction_id_matches_raw_data"] is True
    assert trigger_contract["raw_data_source_call_matches"] is True
    assert trigger_contract["raw_data_ref_block_bytes"] == "0x1234"
    assert trigger_contract["raw_data_ref_block_hash"] == "0x" + "56" * 8
    assert trigger_contract["raw_data_expiration"] == 123_456_789
    assert trigger_contract["raw_data_timestamp"] == 123_450_000
    assert trigger_contract["raw_data_fee_limit"] == 50_000_000
    assert trigger_contract["raw_data_type_url"] == (
        "type.googleapis.com/protocol.TriggerSmartContract"
    )
    assert trigger_contract["raw_data_owner_address"] == "0x41" + fake.owner20.hex()
    assert trigger_contract["raw_data_owner_base58"] == (
        module.tron_base58check_from_payload(b"\x41" + fake.owner20)
    )
    assert trigger_contract["raw_data_contract_address"] == (
        "0x41" + fake.bridge20.hex()
    )
    assert trigger_contract["raw_data_contract_base58"] == fake.bridge
    assert trigger_contract["raw_data_call_data"] == (
        "0x" + TRON_SOURCE_EVENT_CALL_DATA_VECTOR
    )
    assert trigger_contract["signature_count"] == 1
    assert trigger_contract["signature"] == "0x" + TRON_SOURCE_EVENT_SIGNATURE_VECTOR
    assert trigger_contract["signature_sha256"] == (
        "0x"
        + hashlib.sha256(
            bytes.fromhex(TRON_SOURCE_EVENT_SIGNATURE_VECTOR)
        ).hexdigest()
    )
    assert trigger_contract["signature_recovery_id"] == 0
    assert trigger_contract["signature_recovered_address"] == (
        "0x41" + fake.owner20.hex()
    )
    assert trigger_contract["signature_recovered_base58"] == (
        module.tron_base58check_from_payload(b"\x41" + fake.owner20)
    )
    assert trigger_contract["signature_recovers_to_owner"] is True
    assert trigger_contract["source_proof_transaction_bytes"] == (
        "0x" + expected_transaction_bytes
    )
    assert trigger_contract["source_proof_transaction_hash"] == (
        "0x" + hashlib.sha256(bytes.fromhex(expected_transaction_bytes)).hexdigest()
    )
    assert trigger_contract["source_proof_result_bytes"] == (
        "0x" + protobuf_u64(3, 1).hex()
    )
    assert trigger_contract["source_proof_transaction_bytes_checked"] is True
    assert trigger_contract["transaction_merkle_branch_required"] is True
    assert trigger_contract["contract_ret"] == "SUCCESS"
    assert trigger_contract["contract_type"] == "TriggerSmartContract"
    assert trigger_contract["owner_address"] == "0x41" + fake.owner20.hex()
    assert trigger_contract["owner_base58"] == (
        module.tron_base58check_from_payload(b"\x41" + fake.owner20)
    )
    assert trigger_contract["contract_address"] == "0x41" + fake.bridge20.hex()
    assert trigger_contract["contract_base58"] == fake.bridge
    assert trigger_contract["call_data"] == "0x" + TRON_SOURCE_EVENT_CALL_DATA_VECTOR
    assert trigger_contract["call_matches"] is True


def test_live_evidence_source_event_replay_requires_block_metadata_binding():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_transaction"].pop("block_timestamp")
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_transaction"]["block_number"] = 0
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_transaction"]["solid_block"]["block_timestamp"] += 1
    assert module._offline_source_event_args(tampered) is None


def test_live_evidence_rejects_source_event_explicit_log_index_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_log_index_override=1,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "source-event log logIndex does not match log list index: "
            "expected 0, got 1"
        ) in str(exc)
    else:
        raise AssertionError("source event with mismatched explicit logIndex was accepted")


def test_live_evidence_rejects_source_event_snake_log_index_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_log_index_override=1,
        source_event_transaction_log_index_fields=("log_index",),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "source-event log log_index does not match log list index: "
            "expected 0, got 1"
        ) in str(exc)
    else:
        raise AssertionError("source event with mismatched explicit log_index was accepted")


def test_live_evidence_rejects_source_event_duplicate_log_index_aliases():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_log_index_override=0,
        source_event_transaction_log_index_fields=("logIndex", "log_index"),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event log must not include both logIndex and log_index" in str(exc)
    else:
        raise AssertionError("source event with duplicate log-index aliases was accepted")


def test_live_evidence_redacts_source_event_topic_parser_failures(monkeypatch):
    module = load_live_module()
    original_parse_exact_hex32 = module._parse_exact_hex32

    for exception_type in (TypeError, ValueError):

        def fail_source_event_topic(value, *, label, exception_type=exception_type):
            if label == "source-event log topic0":
                raise exception_type(
                    "secret-token source-event log topic0 parser detail"
                )
            return original_parse_exact_hex32(value, label=label)

        with monkeypatch.context() as patch:
            patch.setattr(module, "_parse_exact_hex32", fail_source_event_topic)
            try:
                module._source_event_transaction_summary(
                    {
                        "id": TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
                        "receipt": {"result": "SUCCESS"},
                        "log": [
                            {
                                "address": TRON_TEST_BRIDGE20.hex(),
                                "topics": [
                                    module.TRON_SOURCE_EVENT_TOPIC.hex(),
                                    TRON_SOURCE_EVENT_DIGEST_VECTOR,
                                ],
                                "data": "",
                            }
                        ],
                    },
                    transaction_id=bytes.fromhex(
                        TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                    ),
                    source_bridge_address20=TRON_TEST_BRIDGE20,
                    source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                )
            except RuntimeError as exc:
                message = str(exc)
            else:
                raise AssertionError(
                    "source event topic parser failure was accepted"
                )

        assert (
            "source-event transaction log did not contain the expected "
            "SccpSourceEvent(bytes32) event"
        ) in message
        assert "secret-token" not in message
        assert "parser detail" not in message
        assert exception_type.__name__ not in message


def test_live_evidence_rejects_source_event_info_conflicting_txid_aliases():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_info_id_alias_overrides={"txID": "11" * 32},
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "source-event transaction info returned conflicting transaction id aliases"
            in str(exc)
        )
    else:
        raise AssertionError(
            "source-event transaction info with conflicting txID aliases was accepted"
        )


def test_live_evidence_source_event_replay_args_revalidate_transaction_summary():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_transaction"]["trigger_contract"][
        "signature_recovers_to_owner"
    ] = False
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    tampered["source_event_transaction"]["trigger_contract"][
        "raw_data_call_data"
    ] = "0x" + "11" * 100
    assert module._offline_source_event_args(tampered) is None

    tampered = json.loads(json.dumps(summary))
    trigger_contract = tampered["source_event_transaction"]["trigger_contract"]
    failed_result = protobuf_u64(3, 2)
    failed_source_transaction = b"".join(
        [
            protobuf_bytes(1, bytes.fromhex(trigger_contract["raw_data_hex"][2:])),
            protobuf_bytes(2, bytes.fromhex(trigger_contract["signature"][2:])),
            protobuf_bytes(5, failed_result),
        ]
    )
    trigger_contract["source_proof_result_bytes"] = "0x" + failed_result.hex()
    trigger_contract["source_proof_transaction_bytes"] = (
        "0x" + failed_source_transaction.hex()
    )
    trigger_contract["source_proof_transaction_hash"] = (
        "0x" + hashlib.sha256(failed_source_transaction).hexdigest()
    )
    assert module._offline_source_event_args(tampered) is None


def test_live_evidence_source_event_result_bytes_require_success():
    module = load_live_module()

    assert module._source_event_result_bytes_are_success(protobuf_u64(3, 1)) is True
    assert (
        module._source_event_result_bytes_are_success(
            protobuf_u64(1, 0) + protobuf_u64(2, 0) + protobuf_u64(3, 1)
        )
        is True
    )
    assert module._source_event_result_bytes_are_success(protobuf_u64(3, 2)) is False
    assert (
        module._source_event_result_bytes_are_success(
            protobuf_u64(2, 1) + protobuf_u64(3, 1)
        )
        is False
    )
    assert (
        module._source_event_result_bytes_are_success(
            protobuf_u64(3, 1) + protobuf_u64(1, 0)
        )
        is False
    )


def test_live_evidence_rejects_boolean_source_event_block_number():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_number=True,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "blockNumber must be a positive integer" in str(exc)
    else:
        raise AssertionError("boolean TRON source-event blockNumber was accepted")


def test_live_evidence_rejects_missing_source_event_block_timestamp():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_block_timestamp=None,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "source-event transaction info blockTimeStamp must be a non-negative integer"
            in str(exc)
        )
    else:
        raise AssertionError("missing TRON source-event blockTimeStamp was accepted")


def test_live_evidence_rejects_source_event_transaction_info_timestamp_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_block_timestamp=456001,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "blockTimeStamp does not match block header timestamp" in str(exc)
    else:
        raise AssertionError(
            "source-event transaction-info timestamp drift was accepted"
        )


def test_live_evidence_rejects_zero_block_witness_address_before_header_hashing():
    module = load_live_module()
    raw_data = {
        "timestamp": 123_456,
        "txTrieRoot": "11" * 32,
        "parentHash": "22" * 32,
        "number": 12_345,
        "witness_address": "41" + "00" * 20,
        "version": 31,
        "accountStateRoot": "33" * 32,
    }

    try:
        module._tron_block_header_raw_data_bytes(raw_data)
    except RuntimeError as exc:
        assert "witness_address must be a non-zero TRON address" in str(exc)
    else:
        raise AssertionError("all-zero TRON block witness address was accepted")


def test_live_evidence_rejects_uppercase_tron_log_address_hex():
    module = load_live_module()

    try:
        module._parse_log_address20(("ab" * 20).upper(), label="TRON log address")
    except RuntimeError as exc:
        assert "TRON log address must be canonical lowercase hex" in str(exc)
    else:
        raise AssertionError("uppercase TRON log address was accepted")


def test_live_evidence_rejects_uppercase_generic_tron_hex_blob():
    module = load_live_module()

    try:
        module._parse_hex_blob("0X" + "ab", label="TRON payload")
    except RuntimeError as exc:
        assert "TRON payload must be canonical lowercase hex" in str(exc)
    else:
        raise AssertionError("uppercase generic TRON hex blob was accepted")


def test_live_evidence_reconstructs_block_with_result_extension_transactions():
    module = load_live_module()
    unrelated_raw_data_hex = "02"
    unrelated_transaction = {
        "txID": hashlib.sha256(bytes.fromhex(unrelated_raw_data_hex)).hexdigest(),
        "raw_data_hex": unrelated_raw_data_hex,
        "ret": [
            {
                "fee": "17",
                "ret": "SUCESS",
                "contractRet": "SUCCESS",
                "assetIssueID": "1002000",
                "withdraw_amount": "23",
                "unfreeze_amount": 29,
                "exchange_received_amount": 31,
                "exchange_inject_another_amount": 37,
                "exchange_withdraw_another_amount": 41,
                "exchange_id": 43,
                "shielded_transaction_fee": 47,
                "orderId": "aa" * 32,
                "orderDetails": [
                    {
                        "makerOrderId": "bb" * 32,
                        "takerOrderId": "cc" * 32,
                        "fillSellQuantity": "53",
                        "fillBuyQuantity": 59,
                    }
                ],
                "withdraw_expire_amount": "61",
                "cancel_unfreezeV2_amount": {
                    "BANDWIDTH": "67",
                    "ENERGY": 71,
                },
            }
        ],
    }
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_prefix_transactions=[unrelated_transaction],
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    expected_unrelated_bytes = tron_transaction_bytes_hex(unrelated_transaction)
    expected_transaction_bytes = tron_source_event_transaction_bytes_hex()
    expected_tx_trie_root = tron_merkle_root_hex(
        [expected_unrelated_bytes, expected_transaction_bytes]
    )
    assert solid_block["transaction_count"] == 2
    assert solid_block["transaction_index"] == 1
    assert solid_block["transaction_merkle_branch"] == [
        "0x" + hashlib.sha256(bytes.fromhex(expected_unrelated_bytes)).hexdigest()
    ]
    assert solid_block["block_tx_trie_root"] == "0x" + expected_tx_trie_root
    assert solid_block["calculated_tx_trie_root"] == "0x" + expected_tx_trie_root
    assert solid_block["tx_trie_root_matches"] is True


def test_live_evidence_rejects_result_extension_hex_with_internal_whitespace():
    module = load_live_module()
    unrelated_raw_data_hex = "02"
    unrelated_transaction = {
        "txID": hashlib.sha256(bytes.fromhex(unrelated_raw_data_hex)).hexdigest(),
        "raw_data_hex": unrelated_raw_data_hex,
        "ret": [{"orderId": ("aa" * 15) + "  " + ("aa" * 16)}],
    }
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_prefix_transactions=[unrelated_transaction],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "ret[0] orderId must not contain whitespace" in str(exc)
    else:
        raise AssertionError("internally spaced result extension orderId was accepted")


def test_live_evidence_rejects_uppercase_result_extension_hex():
    module = load_live_module()
    unrelated_raw_data_hex = "02"
    unrelated_transaction = {
        "txID": hashlib.sha256(bytes.fromhex(unrelated_raw_data_hex)).hexdigest(),
        "raw_data_hex": unrelated_raw_data_hex,
        "ret": [{"orderId": ("aa" * 32).upper()}],
    }
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_prefix_transactions=[unrelated_transaction],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "ret[0] orderId must be canonical lowercase hex" in str(exc)
    else:
        raise AssertionError("uppercase result extension orderId was accepted")


def test_live_evidence_accepts_numeric_source_event_success_enums():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_ret=[{"ret": "0", "contractRet": 1}],
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    trigger_contract = summary["source_event_transaction"]["trigger_contract"]
    expected_transaction_bytes = tron_source_event_transaction_bytes_hex(ret=0)
    assert trigger_contract["contract_ret"] == "SUCCESS"
    assert trigger_contract["source_proof_result_bytes"] == (
        "0x" + (protobuf_u64(2, 0) + protobuf_u64(3, 1)).hex()
    )
    assert trigger_contract["source_proof_transaction_bytes"] == (
        "0x" + expected_transaction_bytes
    )
    assert summary["source_event_transaction"]["solid_block"][
        "tx_trie_root_matches"
    ] is True


def test_live_evidence_rejects_non_int64_result_numbers():
    module = load_live_module()
    bad_results = [
        ({"fee": True}, "ret[0] fee must fit non-negative int64"),
        ({"fee": str(2**63)}, "ret[0] fee must fit non-negative int64"),
        ({"fee": "01"}, "ret[0] fee must fit non-negative int64"),
        ({"fee": "\uff11"}, "ret[0] fee must fit non-negative int64"),
        ({"ret": True}, "ret[0] ret enum is unsupported"),
        ({"ret": "SUCCESS"}, "ret[0] ret enum is unsupported"),
        ({"ret": "00"}, "ret[0] ret enum is unsupported"),
        ({"ret": "\uff10"}, "ret[0] ret enum is unsupported"),
        ({"contractRet": "01"}, "ret[0] contractRet enum is unsupported"),
    ]
    for result, expected_error in bad_results:
        try:
            module._tron_transaction_result_bytes(result, label="ret[0]")
        except RuntimeError as exc:
            assert expected_error in str(exc)
        else:
            raise AssertionError(f"{result!r} was accepted")


def test_live_evidence_redacts_unsupported_transaction_result_fields():
    module = load_live_module()
    result_cases = (
        (
            {"secret-token-result-field": 1},
            "ret[0] has unsupported fields: field with sensitive name",
            "secret-token-result-field",
        ),
        (
            {"operator|result": 1},
            "ret[0] has unsupported fields: field with malformed name",
            "operator|result",
        ),
        (
            {7: 1},
            "ret[0] has unsupported fields: non-string field name",
            None,
        ),
        (
            {"operator_override": 1},
            "ret[0] has unsupported fields: operator_override",
            None,
        ),
    )
    for result, expected_error, forbidden in result_cases:
        try:
            module._tron_transaction_result_bytes(result, label="ret[0]")
        except RuntimeError as exc:
            message = str(exc)
            assert expected_error in message
            if forbidden is not None:
                assert forbidden not in message
        else:
            raise AssertionError(f"{result!r} was accepted")

    detail_cases = (
        (
            {"secret-token-detail-field": 1},
            "ret[0] orderDetails[0] has unsupported fields: field with sensitive name",
            "secret-token-detail-field",
        ),
        (
            {"operator|detail": 1},
            "ret[0] orderDetails[0] has unsupported fields: field with malformed name",
            "operator|detail",
        ),
        (
            {7: 1},
            "ret[0] orderDetails[0] has unsupported fields: non-string field name",
            None,
        ),
        (
            {"operator_override": 1},
            "ret[0] orderDetails[0] has unsupported fields: operator_override",
            None,
        ),
    )
    for detail, expected_error, forbidden in detail_cases:
        try:
            module._tron_market_order_detail_bytes(
                detail,
                label="ret[0] orderDetails[0]",
            )
        except RuntimeError as exc:
            message = str(exc)
            assert expected_error in message
            if forbidden is not None:
                assert forbidden not in message
        else:
            raise AssertionError(f"{detail!r} was accepted")


def test_live_evidence_emits_solid_block_header_proof_hash_when_roots_present():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex(), "0x41" + "22" * 20],
        [7, 3],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            witness_schedule_payload_hex=witness_payload,
            witness_schedule_payload_file=None,
            expected_witness_schedule_hash=None,
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    assert solid_block["block_account_state_root"] == "0x" + "ee" * 32
    assert solid_block["parent_block_account_state_root"] == "0x" + "aa" * 32
    assert solid_block["solid_block_header_proof_ready"] is True
    proof_input = solid_block["solid_block_header_proof_input"]
    assert solid_block["solid_block_header_proof_hash"] == (
        module.sccp_client.tron_solid_block_header_proof_hash(proof_input)
    )
    assert solid_block["solid_block_header_proof_bytes"] == (
        "0x"
        + module.sccp_client.canonical_tron_solid_block_header_proof_bytes(
            proof_input
        ).hex()
    )
    assert solid_block["witness_schedule_proof_ready"] is True
    assert solid_block["witness_schedule_payload"] == "0x" + witness_payload
    assert solid_block["witness_schedule_payload_hash"] == (
        module.sccp_client.tron_witness_schedule_payload_hash(bytes.fromhex(witness_payload))
    )
    assert solid_block["witness_schedule_hash"] == (
        module.sccp_client.tron_witness_schedule_hash_from_payload(
            bytes.fromhex(witness_payload)
        )
    )
    assert solid_block["witness_schedule_witness_count"] == 2
    assert solid_block["witness_schedule_total_weight"] == 10
    assert solid_block["block_witness_in_schedule"] is True
    assert solid_block["block_witness_weight"] == 7
    assert solid_block["parent_block_witness_in_schedule"] is True
    assert solid_block["parent_block_witness_weight"] == 7


def test_live_evidence_redacts_solid_block_header_proof_encoder_failures(monkeypatch):
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )

    for exception_type in (TypeError, ValueError, RuntimeError):

        def fail_header_proof(_proof_input, *, exception_type=exception_type):
            raise exception_type("secret-token solid block proof parser detail")

        monkeypatch.setattr(
            module.sccp_client,
            "canonical_tron_solid_block_header_proof_bytes",
            fail_header_proof,
        )

        summary = module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )

        solid_block = summary["source_event_transaction"]["solid_block"]
        assert solid_block["solid_block_header_proof_ready"] is False
        assert solid_block["solid_block_header_proof_blocker"] == (
            "solid block header proof is invalid"
        )
        rendered = json.dumps(solid_block, sort_keys=True)
        assert "secret-token" not in rendered
        assert "parser detail" not in rendered
        assert exception_type.__name__ not in rendered


def test_live_evidence_redacts_witness_schedule_hash_failures(monkeypatch):
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )

    def collect_summary():
        return module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                receipt_root=None,
                receipt_proof_hash=None,
                witness_seal_signers_bitmap_hex=None,
                witness_seal_signature_hex=[],
                expected_witness_seal_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )

    failure_cases = (
        (
            "tron_witness_schedule_payload_hash",
            "secret-token witness schedule payload parser detail",
        ),
        (
            "tron_witness_schedule_hash_from_payload",
            "secret-token witness schedule hash parser detail",
        ),
    )
    for patched_name, secret_detail in failure_cases:
        for exception_type in (TypeError, ValueError, RuntimeError):
            with monkeypatch.context() as patch:

                def fail_hash(
                    *_args,
                    exception_type=exception_type,
                    secret_detail=secret_detail,
                    **_kwargs,
                ):
                    raise exception_type(secret_detail)

                patch.setattr(module.sccp_client, patched_name, fail_hash)
                summary = collect_summary()

            solid_block = summary["source_event_transaction"]["solid_block"]
            assert solid_block["witness_schedule_proof_ready"] is False
            assert solid_block["witness_schedule_proof_blocker"] == (
                "witness schedule payload is invalid"
            )
            assert (
                "witness schedule proof: witness schedule payload is invalid"
                in summary["source_event_transaction"][
                    "source_event_transaction_production_blockers"
                ]
            )
            rendered = json.dumps(summary, sort_keys=True)
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exception_type.__name__ not in rendered


def test_live_evidence_emits_witness_seal_hash_when_certificate_supplied():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            witness_schedule_payload_hex=witness_payload,
            witness_schedule_payload_file=None,
            expected_witness_schedule_hash=bytes.fromhex(
                module.sccp_client.tron_witness_schedule_hash_from_payload(
                    bytes.fromhex(witness_payload)
                ).removeprefix("0x")
            ),
            receipt_root=seal.receipt_root,
            receipt_proof_hash=seal.receipt_proof_hash,
            witness_seal_signers_bitmap_hex="01",
            witness_seal_signature_hex=[seal.signature],
            expected_witness_seal_hash=bytes.fromhex(
                seal.seal_hash.removeprefix("0x")
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    assert solid_block["witness_schedule_proof_ready"] is True
    assert solid_block["witness_schedule_expected_hash_matches"] is True
    assert solid_block["witness_seal_proof_ready"] is True
    assert solid_block["solid_block_message_input"] == seal.solid_block_message_input
    assert solid_block["solid_block_message_hash"] == seal.solid_block_message_hash
    assert solid_block["solid_block_message_bytes"] == (
        "0x"
        + module.sccp_client.canonical_tron_solid_block_message_bytes(
            seal.solid_block_message_input
        ).hex()
    )
    assert solid_block["witness_seal_proof_input"] == seal.seal_input
    assert solid_block["witness_seal_proof_bytes"] == (
        "0x" + module.sccp_client.canonical_tron_witness_seal_bytes(seal.seal_input).hex()
    )
    assert solid_block["witness_seal_hash"] == seal.seal_hash
    assert solid_block["witness_seal_expected_hash_matches"] is True
    assert solid_block["witness_seal_signer_indices"] == [0]
    assert solid_block["witness_seal_signer_addresses"] == [
        "0x41" + TRON_TEST_OWNER20.hex()
    ]
    assert solid_block["witness_seal_recovered_addresses"] == [
        "0x41" + TRON_TEST_OWNER20.hex()
    ]
    assert solid_block["witness_seal_signed_weight"] == 1
    assert solid_block["witness_seal_total_weight"] == 1
    assert solid_block["witness_seal_threshold_checked"] is True


def test_live_evidence_redacts_witness_seal_encoder_failures(monkeypatch):
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)
    expected_witness_schedule_hash = bytes.fromhex(
        module.sccp_client.tron_witness_schedule_hash_from_payload(
            bytes.fromhex(witness_payload)
        ).removeprefix("0x")
    )

    def collect_summary():
        return module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=expected_witness_schedule_hash,
                receipt_root=seal.receipt_root,
                receipt_proof_hash=seal.receipt_proof_hash,
                witness_seal_signers_bitmap_hex="01",
                witness_seal_signature_hex=[seal.signature],
                expected_witness_seal_hash=bytes.fromhex(
                    seal.seal_hash.removeprefix("0x")
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )

    failure_cases = (
        (
            "canonical_tron_solid_block_message_bytes",
            "secret-token witness seal message parser detail",
            "witness seal solid-block message is invalid",
        ),
        (
            "canonical_tron_witness_seal_bytes",
            "secret-token witness seal proof parser detail",
            "witness seal proof is invalid",
        ),
    )
    for patched_name, secret_detail, expected_blocker in failure_cases:
        for exception_type in (TypeError, ValueError, RuntimeError):
            with monkeypatch.context() as patch:

                def fail_encoder(
                    *_args,
                    exception_type=exception_type,
                    secret_detail=secret_detail,
                    **_kwargs,
                ):
                    raise exception_type(secret_detail)

                patch.setattr(module.sccp_client, patched_name, fail_encoder)
                summary = collect_summary()

            solid_block = summary["source_event_transaction"]["solid_block"]
            assert solid_block["witness_seal_proof_ready"] is False
            assert solid_block["witness_seal_proof_blocker"] == expected_blocker
            assert (
                f"witness seal proof: {expected_blocker}"
                in summary["source_event_transaction"][
                    "source_event_transaction_production_blockers"
                ]
            )
            rendered = json.dumps(summary, sort_keys=True)
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exception_type.__name__ not in rendered


def test_live_evidence_emits_transaction_source_proof_hash_when_branch_supplied():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            receipt_root=bytes.fromhex("ab" * 32),
            receipt_proof_hash=None,
            source_inclusion_branch_hex=["aa" * 32],
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    assert solid_block["transaction_source_proof_ready"] is True
    proof_input = solid_block["transaction_source_proof_input"]
    assert proof_input["source_event_digest"] == (
        "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR
    )
    assert proof_input["receipt_root"] == "0x" + "ab" * 32
    assert proof_input["transaction_index"] == 1
    assert proof_input["transaction_count"] == 2
    assert proof_input["transaction_merkle_branch"] == solid_block["transaction_merkle_branch"]
    assert proof_input["inclusion_branch"] == ["0x" + "aa" * 32]
    assert solid_block["transaction_source_proof_bytes"] == (
        "0x"
        + module.sccp_client.canonical_tron_sccp_transaction_source_proof_bytes(
            proof_input
        ).hex()
    )
    assert solid_block["transaction_source_proof_hash"] == (
        module.sccp_client.tron_sccp_transaction_source_proof_hash(proof_input)
    )
    assert (
        solid_block["transaction_source_proof_hash_matches_receipt_proof_hash"]
        is False
    )


def test_live_evidence_redacts_transaction_source_proof_encoder_failures(monkeypatch):
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    for exception_type in (TypeError, ValueError, RuntimeError):

        def fail_source_proof(_proof_input, *, exception_type=exception_type):
            raise exception_type("secret-token transaction source proof parser detail")

        monkeypatch.setattr(
            module.sccp_client,
            "canonical_tron_sccp_transaction_source_proof_bytes",
            fail_source_proof,
        )

        summary = module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                receipt_root=bytes.fromhex("ab" * 32),
                receipt_proof_hash=None,
                source_inclusion_branch_hex=["aa" * 32],
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )

        solid_block = summary["source_event_transaction"]["solid_block"]
        assert solid_block["transaction_source_proof_ready"] is False
        assert solid_block["transaction_source_proof_blocker"] == (
            "transaction source proof is invalid"
        )
        assert (
            "transaction source proof: transaction source proof is invalid"
            in summary["source_event_transaction"][
                "source_event_transaction_production_blockers"
            ]
        )
        rendered = json.dumps(solid_block, sort_keys=True)
        assert "secret-token" not in rendered
        assert "parser detail" not in rendered
        assert exception_type.__name__ not in rendered


def test_live_evidence_accepts_zero_source_inclusion_branch_sibling():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            receipt_root=bytes.fromhex("ab" * 32),
            receipt_proof_hash=None,
            source_inclusion_branch_hex=["00" * 32],
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    assert solid_block["transaction_source_proof_ready"] is True
    assert solid_block["transaction_source_proof_input"]["inclusion_branch"] == [
        "0x" + "00" * 32
    ]


def test_live_evidence_rejects_padded_source_event_operator_proof_inputs():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )
    base = {
        "tron_node_url": "https://tron.example",
        "source_bridge_address": fake.bridge,
        "destination_verifier_address": None,
        "caller_address": None,
        "no_getcontract": True,
        "source_event_digest": bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
        "source_event_transaction_id": bytes.fromhex(
            TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
        ),
        "witness_schedule_payload_hex": None,
        "witness_schedule_payload_file": None,
        "expected_witness_schedule_hash": None,
        "receipt_root": None,
        "receipt_proof_hash": None,
        "witness_seal_signers_bitmap_hex": None,
        "witness_seal_signature_hex": [],
        "expected_witness_seal_hash": None,
        "source_inclusion_branch_hex": [],
        "solid": False,
        "full_toml": False,
        "timeout": 1.0,
    }
    cases = [
        (
            {"source_event_digest": " " + TRON_SOURCE_EVENT_DIGEST_VECTOR},
            "source event digest must not contain surrounding whitespace",
        ),
        (
            {"witness_schedule_payload_hex": " " + witness_payload},
            "witness schedule payload must not contain surrounding whitespace",
        ),
        (
            {"receipt_root": " " + ("ab" * 32)},
            "receipt root must not contain surrounding whitespace",
        ),
        (
            {"witness_seal_signers_bitmap_hex": " 01"},
            "witness seal signers bitmap hex must not contain surrounding whitespace",
        ),
        (
            {"witness_seal_signature_hex": [" " + ("01" * 65)]},
            "witness seal signature 0 must not contain surrounding whitespace",
        ),
        (
            {"source_inclusion_branch_hex": [" " + ("aa" * 32)]},
            "source inclusion branch hex 0 must not contain surrounding whitespace",
        ),
    ]

    for overrides, expected_message in cases:
        values = dict(base)
        values.update(overrides)
        try:
            module.collect_live_evidence(
                SimpleNamespace(**values),
                opener=fake.opener,
            )
        except RuntimeError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(f"padded operator input was accepted: {overrides!r}")


def test_live_evidence_emits_ancestor_and_confirmation_header_proofs():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
        source_event_ancestor_depth=1,
        source_event_confirmation_depth=1,
    )
    seal = tron_live_witness_seal_material(
        module,
        fake,
        witness_payload,
        ancestor_depth=1,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            witness_schedule_payload_hex=witness_payload,
            witness_schedule_payload_file=None,
            expected_witness_schedule_hash=None,
            receipt_root=seal.receipt_root,
            receipt_proof_hash=seal.receipt_proof_hash,
            witness_seal_signers_bitmap_hex="01",
            witness_seal_signature_hex=[seal.signature],
            expected_witness_seal_hash=None,
            solid_block_ancestor_depth=1,
            solid_block_confirmation_depth=1,
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    assert solid_block["solid_block_header_proof_ready"] is True
    assert solid_block["witness_seal_proof_ready"] is True
    assert solid_block["solid_block_ancestor_headers_ready"] is True
    assert solid_block["solid_block_ancestor_header_count"] == 1
    ancestor = solid_block["solid_block_ancestor_header_proofs"][0]
    assert ancestor["version"] == 1
    assert ancestor["witness_address"] == "0x41" + TRON_TEST_OWNER20.hex()
    assert ancestor["timestamp_ms"] == 450000
    assert solid_block["solid_block_confirmation_headers_ready"] is True
    assert solid_block["solid_block_confirmation_header_count"] == 1
    assert solid_block["solid_block_confirmation_unique_witness_count"] == 1
    assert solid_block["solid_block_confirmation_signed_weight"] == 1
    assert solid_block["solid_block_confirmation_total_weight"] == 1
    confirmation = solid_block["solid_block_confirmation_header_proofs"][0]
    assert confirmation["version"] == 1
    assert confirmation["parent_block_id"] == solid_block["block_id"]
    assert confirmation["witness_address"] == "0x41" + TRON_TEST_OWNER20.hex()
    assert confirmation["timestamp_ms"] == 459000


def collect_complete_source_event_transaction_summary(
    module,
    *,
    include_expected_witness_schedule_hash=True,
    include_source_record_trust_anchor=False,
    source_trust_anchor_hash_override=None,
    witness_payload_hex=None,
    transition_parent_payload_hex=None,
):
    witness_payload = witness_payload_hex or tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
        source_event_ancestor_depth=1,
        source_event_confirmation_depth=1,
    )
    expected_transaction_bytes = tron_source_event_transaction_bytes_hex()
    expected_dummy_bytes = tron_dummy_transaction_bytes_hex()
    transaction_root = tron_merkle_root_hex(
        [expected_dummy_bytes, expected_transaction_bytes]
    )
    transaction_source_input = {
        "source_event_digest": "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR,
        "receipt_root": "0x" + "ab" * 32,
        "transaction_root": "0x" + transaction_root,
        "transaction_index": 1,
        "transaction_count": 2,
        "transaction_bytes": "0x" + expected_transaction_bytes,
        "transaction_merkle_branch": [
            "0x" + hashlib.sha256(bytes.fromhex(expected_dummy_bytes)).hexdigest()
        ],
        "inclusion_branch": ["0x" + "aa" * 32],
        "source_bridge_emitter_address": "0x" + fake.bridge20.hex(),
        "source_bridge_owner_address": "0x" + fake.owner20.hex(),
    }
    receipt_proof_hash = module.sccp_client.tron_sccp_transaction_source_proof_hash(
        transaction_source_input
    )
    schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(witness_payload)
    )
    expected_witness_schedule_hash = None
    if include_expected_witness_schedule_hash:
        expected_witness_schedule_hash = bytes.fromhex(schedule_hash.removeprefix("0x"))
    source_trust_anchor_hash = None
    if include_source_record_trust_anchor:
        source_trust_anchor_hash = bytes.fromhex(schedule_hash.removeprefix("0x"))
    if source_trust_anchor_hash_override is not None:
        source_trust_anchor_hash = source_trust_anchor_hash_override
    source_record_requested = source_trust_anchor_hash is not None
    seal = tron_live_witness_seal_material(
        module,
        fake,
        witness_payload,
        receipt_proof_hash_hex=receipt_proof_hash.removeprefix("0x"),
        ancestor_depth=1,
    )
    transition_json = []
    transition_seal_hash = None
    if transition_parent_payload_hex is not None:
        parent_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
            bytes.fromhex(transition_parent_payload_hex)
        )
        expected_witness_schedule_hash = bytes.fromhex(
            parent_schedule_hash.removeprefix("0x")
        )
        transition_message_input = {
            "source_domain": module.sccp_client.SCCP_DOMAIN_TRON,
            "from_witness_schedule_epoch": 0,
            "to_witness_schedule_epoch": 1,
            "transition_block_number": 122,
            "transition_block_hash": "0x" + seal.parent_block_id,
            "parent_witness_schedule_hash": parent_schedule_hash,
            "next_witness_schedule_hash": schedule_hash,
            "next_witness_schedule_payload": "0x" + witness_payload,
        }
        transition_message_hash = (
            module.sccp_client.tron_witness_schedule_transition_message_hash(
                transition_message_input
            )
        )
        transition_signature = tron_signature_hex(
            module,
            bytes.fromhex(transition_message_hash.removeprefix("0x")),
            nonce_start=41,
        )
        transition_seal_input = {
            **transition_message_input,
            "transition_message_hash": transition_message_hash,
            "seal_proof": {
                "version": 1,
                "total_weight": 1,
                "signed_weight": 1,
                "solid_block_message_hash": transition_message_hash,
                "witness_addresses": ["0x41" + fake.owner20.hex()],
                "witness_weights": [1],
                "signers_bitmap": "0x01",
                "signatures": ["0x" + transition_signature],
            },
        }
        transition_seal_hash = (
            module.sccp_client.tron_witness_schedule_transition_seal_hash(
                transition_seal_input
            )
        )
        transition_json = [
            json.dumps(
                {
                    "from_witness_schedule_epoch": 0,
                    "to_witness_schedule_epoch": 1,
                    "transition_block_number": 122,
                    "transition_block_hash": "0x" + seal.parent_block_id,
                    "parent_witness_schedule_payload": (
                        "0x" + transition_parent_payload_hex
                    ),
                    "next_witness_schedule_payload": "0x" + witness_payload,
                    "transition_message_hash": transition_message_hash,
                    "transition_seal_hash": transition_seal_hash,
                    "signers_bitmap": "0x01",
                    "signatures": ["0x" + transition_signature],
                }
            )
        ]

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            witness_schedule_payload_hex=witness_payload,
            witness_schedule_payload_file=None,
            expected_witness_schedule_hash=expected_witness_schedule_hash,
            receipt_root=seal.receipt_root,
            receipt_proof_hash=seal.receipt_proof_hash,
            witness_seal_signers_bitmap_hex="01",
            witness_seal_signature_hex=[seal.signature],
            expected_witness_seal_hash=bytes.fromhex(
                seal.seal_hash.removeprefix("0x")
            ),
            witness_schedule_transition_json=transition_json,
            source_inclusion_branch_hex=["aa" * 32],
            solid_block_ancestor_depth=1,
            solid_block_confirmation_depth=1,
            expected_source_bridge_config_hash=None,
            source_trust_anchor_hash=source_trust_anchor_hash,
            consensus_verifier_hash=(
                bytes.fromhex("55" * 32) if source_record_requested else None
            ),
            message_inclusion_verifier_hash=(
                bytes.fromhex("66" * 32) if source_record_requested else None
            ),
            source_bridge_emitter_code_hash=None,
            finality_policy_hash=(
                bytes.fromhex("88" * 32) if source_record_requested else None
            ),
            adapter_verifier_vk_hash=(
                bytes.fromhex(TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR)
                if source_record_requested
                else None
            ),
            deployment_receipt_hash=(
                bytes.fromhex("aa" * 32) if source_record_requested else None
            ),
            expected_source_verifier_material_hash=None,
            expected_source_adapter_engine_deployment_hash=None,
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )
    return SimpleNamespace(
        summary=summary,
        receipt_proof_hash=receipt_proof_hash,
        schedule_hash=schedule_hash,
        seal=seal,
        transition_seal_hash=transition_seal_hash,
    )


def test_live_evidence_marks_source_event_transaction_production_ready():
    module = load_live_module()
    result = collect_complete_source_event_transaction_summary(module)

    transaction = result.summary["source_event_transaction"]
    solid_block = transaction["solid_block"]
    assert transaction["source_event_transaction_production_ready"] is True
    assert "source_event_transaction_production_blockers" not in transaction
    assert solid_block["transaction_source_proof_ready"] is True
    assert solid_block["transaction_source_proof_hash"] == result.receipt_proof_hash
    assert (
        solid_block["transaction_source_proof_hash_matches_receipt_proof_hash"]
        is True
    )
    assert solid_block["witness_schedule_expected_hash_matches"] is True
    assert solid_block["witness_seal_expected_hash_matches"] is True
    assert solid_block["solid_block_header_proof_ready"] is True
    assert solid_block["solid_block_ancestor_headers_ready"] is True
    assert solid_block["solid_block_confirmation_headers_ready"] is True


def test_live_evidence_requires_expected_witness_schedule_hash_for_production_ready():
    module = load_live_module()
    result = collect_complete_source_event_transaction_summary(
        module,
        include_expected_witness_schedule_hash=False,
    )

    transaction = result.summary["source_event_transaction"]
    solid_block = transaction["solid_block"]
    assert solid_block["transaction_source_proof_ready"] is True
    assert solid_block["solid_block_header_proof_ready"] is True
    assert solid_block["witness_schedule_proof_ready"] is True
    assert solid_block["witness_schedule_expected_hash_matches"] is False
    assert solid_block["witness_seal_proof_ready"] is True
    assert solid_block["solid_block_ancestor_headers_ready"] is True
    assert solid_block["solid_block_confirmation_headers_ready"] is True
    assert transaction["source_event_transaction_production_ready"] is False
    assert transaction["source_event_transaction_production_blockers"] == [
        "witness schedule must match expected source trust-anchor hash or carry a valid transition chain"
    ]


def test_live_evidence_accepts_witness_schedule_transition_chain_for_production_ready():
    module = load_live_module()
    parent_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    active_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [2],
    )
    result = collect_complete_source_event_transaction_summary(
        module,
        witness_payload_hex=active_payload,
        transition_parent_payload_hex=parent_payload,
    )

    transaction = result.summary["source_event_transaction"]
    solid_block = transaction["solid_block"]
    assert transaction["source_event_transaction_production_ready"] is True
    assert solid_block["witness_schedule_expected_hash_matches"] is False
    assert solid_block["witness_schedule_transition_chain_ready"] is True
    assert solid_block["witness_schedule_transition_count"] == 1
    assert (
        solid_block["witness_schedule_transition_final_hash"]
        == result.schedule_hash
    )
    assert (
        solid_block["witness_schedule_transition_proofs"][0][
            "transition_seal_hash"
        ]
        == result.transition_seal_hash
    )


def test_live_evidence_redacts_witness_schedule_transition_encoder_failures(
    monkeypatch,
):
    module = load_live_module()
    parent_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    active_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [2],
    )
    parent_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(parent_payload)
    )
    active_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(active_payload)
    )
    active_payload_hash = module.sccp_client.tron_witness_schedule_payload_hash(
        bytes.fromhex(active_payload)
    )
    transition_block_hash = "0x" + "22" * 32
    transition_message_input = {
        "source_domain": module.sccp_client.SCCP_DOMAIN_TRON,
        "from_witness_schedule_epoch": 0,
        "to_witness_schedule_epoch": 1,
        "transition_block_number": 122,
        "transition_block_hash": transition_block_hash,
        "parent_witness_schedule_hash": parent_schedule_hash,
        "next_witness_schedule_hash": active_schedule_hash,
        "next_witness_schedule_payload": "0x" + active_payload,
        "next_witness_schedule_payload_hash": active_payload_hash,
    }
    transition_message_hash = (
        module.sccp_client.tron_witness_schedule_transition_message_hash(
            transition_message_input
        )
    )
    transition_signature = tron_signature_hex(
        module,
        bytes.fromhex(transition_message_hash.removeprefix("0x")),
        nonce_start=41,
    )
    transition_seal_input = {
        **transition_message_input,
        "transition_message_hash": transition_message_hash,
        "seal_proof": {
            "version": 1,
            "total_weight": 1,
            "signed_weight": 1,
            "solid_block_message_hash": transition_message_hash,
            "witness_addresses": ["0x41" + TRON_TEST_OWNER20.hex()],
            "witness_weights": [1],
            "signers_bitmap": "0x01",
            "signatures": ["0x" + transition_signature],
        },
    }
    transition_seal_hash = (
        module.sccp_client.tron_witness_schedule_transition_seal_hash(
            transition_seal_input
        )
    )
    transition_inputs = [
        {
            "from_witness_schedule_epoch": 0,
            "to_witness_schedule_epoch": 1,
            "transition_block_number": 122,
            "transition_block_hash": transition_block_hash,
            "parent_witness_schedule_payload": "0x" + parent_payload,
            "next_witness_schedule_payload": "0x" + active_payload,
            "transition_message_hash": transition_message_hash,
            "transition_seal_hash": transition_seal_hash,
            "signers_bitmap": "0x01",
            "signatures": ["0x" + transition_signature],
        }
    ]
    child_header = {"number": 123, "block_id": bytes.fromhex("33" * 32)}
    parent_header = {"number": 122, "block_id": bytes.fromhex("22" * 32)}

    failure_cases = (
        (
            "canonical_tron_witness_schedule_transition_message_bytes",
            "secret-token transition message parser detail",
            "witness schedule transition 0 message is invalid",
        ),
        (
            "canonical_tron_witness_schedule_transition_seal_bytes",
            "secret-token transition seal parser detail",
            "witness schedule transition 0 seal is invalid",
        ),
    )
    for patched_name, secret_detail, expected_blocker in failure_cases:
        for exception_type in (TypeError, ValueError, RuntimeError):
            with monkeypatch.context() as patch:

                def fail_encoder(
                    *_args,
                    exception_type=exception_type,
                    secret_detail=secret_detail,
                    **_kwargs,
                ):
                    raise exception_type(secret_detail)

                patch.setattr(module.sccp_client, patched_name, fail_encoder)
                summary = (
                    module._source_event_witness_schedule_transition_chain_summary(
                        transition_inputs,
                        active_witness_schedule_payload=bytes.fromhex(active_payload),
                        expected_schedule_hash=bytes.fromhex(
                            parent_schedule_hash.removeprefix("0x")
                        ),
                        child_header=child_header,
                        parent_header=parent_header,
                        ancestor_headers=[],
                    )
                )

            assert summary["witness_schedule_transition_chain_ready"] is False
            assert summary["witness_schedule_transition_chain_required"] is True
            assert summary["witness_schedule_transition_chain_blocker"] == (
                expected_blocker
            )
            assert summary["witness_schedule_transition_count"] == 1
            rendered = json.dumps(summary, sort_keys=True)
            assert "secret-token" not in rendered
            assert "parser detail" not in rendered
            assert exception_type.__name__ not in rendered


def test_live_evidence_rejects_witness_schedule_transition_signature_for_wrong_witness():
    module = load_live_module()
    parent_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    active_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [2],
    )
    parent_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(parent_payload)
    )
    active_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(active_payload)
    )
    active_payload_hash = module.sccp_client.tron_witness_schedule_payload_hash(
        bytes.fromhex(active_payload)
    )
    transition_block_hash = "0x" + "22" * 32
    transition_message_input = {
        "source_domain": module.sccp_client.SCCP_DOMAIN_TRON,
        "from_witness_schedule_epoch": 0,
        "to_witness_schedule_epoch": 1,
        "transition_block_number": 122,
        "transition_block_hash": transition_block_hash,
        "parent_witness_schedule_hash": parent_schedule_hash,
        "next_witness_schedule_hash": active_schedule_hash,
        "next_witness_schedule_payload": "0x" + active_payload,
        "next_witness_schedule_payload_hash": active_payload_hash,
    }
    transition_message_hash = (
        module.sccp_client.tron_witness_schedule_transition_message_hash(
            transition_message_input
        )
    )
    wrong_signature = tron_signature_hex_for_private_key(
        module,
        bytes.fromhex(transition_message_hash.removeprefix("0x")),
        private_key=2,
        nonce_start=53,
    )

    try:
        module._source_event_witness_schedule_transition_chain_summary(
            [
                {
                    "from_witness_schedule_epoch": 0,
                    "to_witness_schedule_epoch": 1,
                    "transition_block_number": 122,
                    "transition_block_hash": transition_block_hash,
                    "parent_witness_schedule_payload": "0x" + parent_payload,
                    "next_witness_schedule_payload": "0x" + active_payload,
                    "transition_message_hash": transition_message_hash,
                    "signers_bitmap": "0x01",
                    "signatures": ["0x" + wrong_signature],
                }
            ],
            active_witness_schedule_payload=bytes.fromhex(active_payload),
            expected_schedule_hash=bytes.fromhex(parent_schedule_hash.removeprefix("0x")),
            child_header={"number": 123, "block_id": bytes.fromhex("33" * 32)},
            parent_header={"number": 122, "block_id": bytes.fromhex("22" * 32)},
            ancestor_headers=[],
        )
    except RuntimeError as exc:
        assert (
            "witness schedule transition 0 signature 0 does not recover "
            "to selected witness"
        ) in str(exc)
    else:
        raise AssertionError("transition seal signature from a different key was accepted")


def test_live_evidence_rejects_witness_schedule_transition_noncanonical_signature():
    module = load_live_module()
    parent_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    active_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [2],
    )
    parent_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(parent_payload)
    )
    active_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(active_payload)
    )
    active_payload_hash = module.sccp_client.tron_witness_schedule_payload_hash(
        bytes.fromhex(active_payload)
    )
    transition_block_hash = "0x" + "22" * 32
    transition_message_input = {
        "source_domain": module.sccp_client.SCCP_DOMAIN_TRON,
        "from_witness_schedule_epoch": 0,
        "to_witness_schedule_epoch": 1,
        "transition_block_number": 122,
        "transition_block_hash": transition_block_hash,
        "parent_witness_schedule_hash": parent_schedule_hash,
        "next_witness_schedule_hash": active_schedule_hash,
        "next_witness_schedule_payload": "0x" + active_payload,
        "next_witness_schedule_payload_hash": active_payload_hash,
    }
    transition_message_hash = (
        module.sccp_client.tron_witness_schedule_transition_message_hash(
            transition_message_input
        )
    )
    valid_signature = tron_signature_hex(
        module,
        bytes.fromhex(transition_message_hash.removeprefix("0x")),
        nonce_start=41,
    )
    zero_r_signature = bytearray(bytes.fromhex(valid_signature))
    zero_r_signature[:32] = b"\x00" * 32

    for label, signature in (
        ("high-S", tron_high_s_signature_hex(module, valid_signature)),
        ("zero-R", zero_r_signature.hex()),
    ):
        try:
            module._source_event_witness_schedule_transition_chain_summary(
                [
                    {
                        "from_witness_schedule_epoch": 0,
                        "to_witness_schedule_epoch": 1,
                        "transition_block_number": 122,
                        "transition_block_hash": transition_block_hash,
                        "parent_witness_schedule_payload": "0x" + parent_payload,
                        "next_witness_schedule_payload": "0x" + active_payload,
                        "transition_message_hash": transition_message_hash,
                        "signers_bitmap": "0x01",
                        "signatures": ["0x" + signature],
                    }
                ],
                active_witness_schedule_payload=bytes.fromhex(active_payload),
                expected_schedule_hash=bytes.fromhex(
                    parent_schedule_hash.removeprefix("0x")
                ),
                child_header={"number": 123, "block_id": bytes.fromhex("33" * 32)},
                parent_header={"number": 122, "block_id": bytes.fromhex("22" * 32)},
                ancestor_headers=[],
            )
        except RuntimeError as exc:
            assert (
                "witness schedule transition 0 signature 0 must be a canonical 65-byte"
            ) in str(exc)
        else:
            raise AssertionError(
                f"transition seal {label} signature was accepted"
            )


def test_live_evidence_rejects_witness_schedule_transition_below_two_thirds_weight():
    module = load_live_module()
    parent_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex(), "0x41" + "22" * 20],
        [1, 2],
    )
    active_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex(), "0x41" + "22" * 20],
        [2, 2],
    )
    parent_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(parent_payload)
    )
    active_schedule_hash = module.sccp_client.tron_witness_schedule_hash_from_payload(
        bytes.fromhex(active_payload)
    )
    active_payload_hash = module.sccp_client.tron_witness_schedule_payload_hash(
        bytes.fromhex(active_payload)
    )
    transition_block_hash = "0x" + "22" * 32
    transition_message_input = {
        "source_domain": module.sccp_client.SCCP_DOMAIN_TRON,
        "from_witness_schedule_epoch": 0,
        "to_witness_schedule_epoch": 1,
        "transition_block_number": 122,
        "transition_block_hash": transition_block_hash,
        "parent_witness_schedule_hash": parent_schedule_hash,
        "next_witness_schedule_hash": active_schedule_hash,
        "next_witness_schedule_payload": "0x" + active_payload,
        "next_witness_schedule_payload_hash": active_payload_hash,
    }
    transition_message_hash = (
        module.sccp_client.tron_witness_schedule_transition_message_hash(
            transition_message_input
        )
    )
    signature = tron_signature_hex(
        module,
        bytes.fromhex(transition_message_hash.removeprefix("0x")),
        nonce_start=59,
    )

    try:
        module._source_event_witness_schedule_transition_chain_summary(
            [
                {
                    "from_witness_schedule_epoch": 0,
                    "to_witness_schedule_epoch": 1,
                    "transition_block_number": 122,
                    "transition_block_hash": transition_block_hash,
                    "parent_witness_schedule_payload": "0x" + parent_payload,
                    "next_witness_schedule_payload": "0x" + active_payload,
                    "transition_message_hash": transition_message_hash,
                    "signers_bitmap": "0x01",
                    "signatures": ["0x" + signature],
                }
            ],
            active_witness_schedule_payload=bytes.fromhex(active_payload),
            expected_schedule_hash=bytes.fromhex(parent_schedule_hash.removeprefix("0x")),
            child_header={"number": 123, "block_id": bytes.fromhex("33" * 32)},
            parent_header={"number": 122, "block_id": bytes.fromhex("22" * 32)},
            ancestor_headers=[],
        )
    except RuntimeError as exc:
        assert (
            "witness schedule transition 0 signed weight does not exceed two thirds"
        ) in str(exc)
    else:
        raise AssertionError("transition seal below the two-thirds threshold was accepted")


def test_live_evidence_uses_source_trust_anchor_hash_for_production_ready():
    module = load_live_module()
    result = collect_complete_source_event_transaction_summary(
        module,
        include_expected_witness_schedule_hash=False,
        include_source_record_trust_anchor=True,
    )

    transaction = result.summary["source_event_transaction"]
    solid_block = transaction["solid_block"]
    assert transaction["source_event_transaction_production_ready"] is True
    assert solid_block["witness_schedule_expected_hash_matches"] is True
    assert solid_block["expected_witness_schedule_hash"] == result.schedule_hash
    assert result.summary["source_record_inputs"]["source_trust_anchor_hash"] == (
        result.schedule_hash
    )


def test_live_evidence_rejects_witness_schedule_hash_drift_from_source_trust_anchor():
    module = load_live_module()

    try:
        collect_complete_source_event_transaction_summary(
            module,
            include_expected_witness_schedule_hash=True,
            source_trust_anchor_hash_override=bytes.fromhex("99" * 32),
        )
    except ValueError as exc:
        assert "--expected-witness-schedule-hash must match" in str(exc)
        assert "--source-trust-anchor-hash" in str(exc)
    else:
        raise AssertionError("witness schedule hash drift from source trust anchor was accepted")


def test_live_evidence_rejects_witness_seal_hash_mismatch():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                receipt_root=seal.receipt_root,
                receipt_proof_hash=seal.receipt_proof_hash,
                witness_seal_signers_bitmap_hex="01",
                witness_seal_signature_hex=[seal.signature],
                expected_witness_seal_hash=bytes.fromhex("99" * 32),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness seal hash does not match" in str(exc)
    else:
        raise AssertionError("mismatched witness seal hash was accepted")


def test_live_evidence_rejects_witness_seal_signature_for_wrong_witness():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)
    wrong_signature = tron_signature_hex_for_private_key(
        module,
        bytes.fromhex(seal.solid_block_message_hash.removeprefix("0x")),
        private_key=2,
        nonce_start=29,
    )
    assert module._tron_recovered_signature_address20(
        bytes.fromhex(seal.solid_block_message_hash.removeprefix("0x")),
        bytes.fromhex(wrong_signature),
    ) != TRON_TEST_OWNER20

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                receipt_root=seal.receipt_root,
                receipt_proof_hash=seal.receipt_proof_hash,
                witness_seal_signers_bitmap_hex="01",
                witness_seal_signature_hex=[wrong_signature],
                expected_witness_seal_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness seal signature 0 does not recover to selected witness" in str(exc)
    else:
        raise AssertionError("witness seal signature from a different key was accepted")


def test_live_evidence_rejects_witness_seal_below_two_thirds_weight():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex(), "0x41" + "22" * 20],
        [1, 2],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                receipt_root=seal.receipt_root,
                receipt_proof_hash=seal.receipt_proof_hash,
                witness_seal_signers_bitmap_hex="01",
                witness_seal_signature_hex=[seal.signature],
                expected_witness_seal_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness seal signed weight does not exceed two thirds" in str(exc)
    else:
        raise AssertionError("witness seal below the two-thirds threshold was accepted")


def test_live_evidence_rejects_malformed_witness_seal_signature():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                receipt_root=seal.receipt_root,
                receipt_proof_hash=seal.receipt_proof_hash,
                witness_seal_signers_bitmap_hex="01",
                witness_seal_signature_hex=["12" * 65],
                expected_witness_seal_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness seal signature 0 must be a canonical 65-byte" in str(exc)
    else:
        raise AssertionError("malformed witness seal signature was accepted")


def test_live_evidence_rejects_witness_seal_high_s_signature():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
        source_event_parent_block_tx_trie_root="dd" * 32,
    )
    seal = tron_live_witness_seal_material(module, fake, witness_payload)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                receipt_root=seal.receipt_root,
                receipt_proof_hash=seal.receipt_proof_hash,
                witness_seal_signers_bitmap_hex="01",
                witness_seal_signature_hex=[
                    tron_high_s_signature_hex(module, seal.signature)
                ],
                expected_witness_seal_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness seal signature 0 must be a canonical 65-byte" in str(exc)
    else:
        raise AssertionError("witness seal high-S signature was accepted")


def test_live_evidence_marks_header_proof_blocked_when_parent_tx_root_zero():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_account_state_root="ee" * 32,
        source_event_parent_block_account_state_root="aa" * 32,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    solid_block = summary["source_event_transaction"]["solid_block"]
    assert solid_block["solid_block_header_proof_ready"] is False
    assert solid_block["solid_block_header_proof_blocker"] == (
        "parent txTrieRoot missing or zero"
    )


def test_live_evidence_rejects_witness_schedule_hash_mismatch():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex()],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=bytes.fromhex("99" * 32),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness schedule hash does not match" in str(exc)
    else:
        raise AssertionError("mismatched witness schedule hash was accepted")


def test_live_evidence_rejects_witness_schedule_total_weight_overflow():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex(), "0x41" + "22" * 20],
        [(1 << 64) - 1, 1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness schedule payload total weight must fit u64" in str(exc)
    else:
        raise AssertionError("overflowing witness schedule total weight was accepted")


def test_live_evidence_rejects_duplicate_witness_schedule_address():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + TRON_TEST_OWNER20.hex(), "0x41" + TRON_TEST_OWNER20.hex()],
        [1, 1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness schedule payload witness 1 must be unique" in str(exc)
    else:
        raise AssertionError("duplicate witness schedule address was accepted")


def test_live_evidence_rejects_block_witness_not_in_schedule():
    module = load_live_module()
    witness_payload = tron_witness_schedule_payload_hex(
        ["0x41" + "22" * 20],
        [1],
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                witness_schedule_payload_hex=witness_payload,
                witness_schedule_payload_file=None,
                expected_witness_schedule_hash=None,
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "block witness is not in active witness schedule" in str(exc)
    else:
        raise AssertionError("non-member block witness schedule was accepted")


def test_live_evidence_rejects_source_event_block_tx_trie_root_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_tx_trie_root_override="99" * 32,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "txTrieRoot does not match transactions" in str(exc)
    else:
        raise AssertionError("source-event block txTrieRoot mismatch was accepted")


def test_live_evidence_rejects_source_event_block_id_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_id_override="aa" * 32,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "blockID does not match header raw_data hash" in str(exc)
    else:
        raise AssertionError("source-event blockID mismatch was accepted")


def test_live_evidence_rejects_padded_source_event_block_id():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_id_override=" " + ("aa" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event block blockID must not contain surrounding whitespace" in str(exc)
    else:
        raise AssertionError("padded source-event blockID was accepted")


def test_live_evidence_rejects_source_event_parent_block_id_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_parent_block_id_override="ab" * 32,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "parent block blockID does not match header raw_data hash" in str(exc)
    else:
        raise AssertionError("source-event parent blockID mismatch was accepted")


def test_live_evidence_rejects_source_event_parent_link_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_child_parent_hash_override="ac" * 32,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "parentHash does not match parent blockID" in str(exc)
    else:
        raise AssertionError("source-event parent link mismatch was accepted")


def test_live_evidence_rejects_source_event_parent_timestamp_not_before_child():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_parent_block_timestamp=456000,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "parent block timestamp must be before child" in str(exc)
    else:
        raise AssertionError("source-event parent timestamp drift was accepted")


def test_live_evidence_rejects_source_event_block_witness_signature_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_witness_signature_override=("01" * 64) + "00",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness_signature does not recover to witness_address" in str(exc)
    else:
        raise AssertionError("source-event block witness signature mismatch was accepted")


def test_live_evidence_rejects_source_event_block_witness_high_s_signature():
    module = load_live_module()
    tx_trie_root = tron_merkle_root_hex(
        [
            tron_dummy_transaction_bytes_hex(),
            tron_source_event_transaction_bytes_hex(),
        ]
    )
    parent_raw_data_hex = tron_block_header_raw_data_hex(
        number=122,
        tx_trie_root_hex="00" * 32,
        parent_hash_hex="55" * 32,
        witness_address_hex="41" + TRON_TEST_OWNER20.hex(),
        timestamp=453000,
    )
    parent_block_id = tron_block_id_hex(122, parent_raw_data_hex)
    child_raw_data_hex = tron_block_header_raw_data_hex(
        tx_trie_root_hex=tx_trie_root,
        parent_hash_hex=parent_block_id,
        witness_address_hex="41" + TRON_TEST_OWNER20.hex(),
    )
    high_s_signature = tron_high_s_signature_hex(
        module,
        tron_header_signature_hex(module, child_raw_data_hex, 4),
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_witness_signature_override=high_s_signature,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event block witness_signature must be a canonical 65-byte" in str(
            exc
        )
    else:
        raise AssertionError("source-event block witness high-S signature was accepted")


def test_live_evidence_rejects_padded_source_event_block_witness_signature():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_witness_signature_override=" " + (("01" * 64) + "00"),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "block witness_signature must not contain surrounding whitespace" in str(
            exc
        )
    else:
        raise AssertionError("padded source-event block witness signature was accepted")


def test_live_evidence_rejects_source_event_parent_witness_signature_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_parent_block_witness_signature_override=("02" * 64) + "00",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "witness_signature does not recover to witness_address" in str(exc)
    else:
        raise AssertionError("parent block witness signature mismatch was accepted")


def test_live_evidence_rejects_source_event_parent_witness_high_s_signature():
    module = load_live_module()
    parent_raw_data_hex = tron_block_header_raw_data_hex(
        number=122,
        tx_trie_root_hex="00" * 32,
        parent_hash_hex="55" * 32,
        witness_address_hex="41" + TRON_TEST_OWNER20.hex(),
        timestamp=453000,
    )
    high_s_signature = tron_high_s_signature_hex(
        module,
        tron_header_signature_hex(module, parent_raw_data_hex, 2),
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_parent_block_witness_signature_override=high_s_signature,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event parent block witness_signature must be a canonical 65-byte" in str(
            exc
        )
    else:
        raise AssertionError("parent block witness high-S signature was accepted")


def test_live_evidence_rejects_source_event_block_missing_transaction():
    module = load_live_module()
    dummy_raw_data_hex = "02"
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_transactions_override=[
            {
                "txID": hashlib.sha256(bytes.fromhex(dummy_raw_data_hex)).hexdigest(),
                "raw_data_hex": dummy_raw_data_hex,
            }
        ],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "block does not contain transaction id" in str(exc)
    else:
        raise AssertionError("source-event block missing transaction was accepted")


def test_live_evidence_rejects_source_event_block_conflicting_txid_aliases():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_block_transaction_id_alias_overrides={"id": "11" * 32},
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "source-event block transaction[1] returned conflicting "
            "transaction id aliases"
        ) in str(exc)
    else:
        raise AssertionError(
            "source-event block transaction with conflicting id aliases was accepted"
        )


def test_live_evidence_source_event_transaction_readback_uses_solid_endpoint():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        expected_constant_endpoint="/walletsolidity/triggerconstantcontract",
        expected_transaction_endpoint="/walletsolidity/gettransactioninfobyid",
        expected_transaction_by_id_endpoint="/walletsolidity/gettransactionbyid",
        expected_block_endpoint="/walletsolidity/getblockbynum",
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            solid=True,
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    assert summary["constant_endpoint"] == "walletsolidity/triggerconstantcontract"
    assert (
        summary["transaction_info_endpoint"]
        == "walletsolidity/gettransactioninfobyid"
    )
    assert summary["transaction_endpoint"] == "walletsolidity/gettransactionbyid"
    assert summary["block_endpoint"] == "walletsolidity/getblockbynum"
    assert summary["source_event_transaction"]["event_matches"] is True
    assert summary["source_event_transaction"]["trigger_contract"]["call_matches"] is True


def test_live_evidence_source_event_transaction_accepts_legacy_recovery_id():
    module = load_live_module()
    legacy_signature = TRON_SOURCE_EVENT_SIGNATURE_VECTOR[:-2] + "1b"
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=[legacy_signature],
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
            source_event_transaction_id=bytes.fromhex(
                TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
            ),
            full_toml=False,
            timeout=1.0,
        ),
        opener=fake.opener,
    )

    trigger_contract = summary["source_event_transaction"]["trigger_contract"]
    assert trigger_contract["signature_recovery_id"] == 27
    assert trigger_contract["signature_recovers_to_owner"] is True
    assert trigger_contract["signature_recovered_address"] == (
        "0x41" + fake.owner20.hex()
    )


def test_live_evidence_rejects_source_event_transaction_raw_data_hash_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_raw_data_hex="02" * 32,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "raw_data_hex" in str(exc)
    else:
        raise AssertionError("source-event transaction with wrong raw_data_hex was accepted")


def test_live_evidence_rejects_source_event_conflicting_txid_aliases():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_id_alias_overrides={"id": "11" * 32},
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "source-event transaction returned conflicting transaction id aliases"
            in str(exc)
        )
    else:
        raise AssertionError(
            "source-event transaction with conflicting txID aliases was accepted"
        )


def test_live_evidence_rejects_source_event_transaction_without_txid_alias():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_include_txid=False,
        source_event_transaction_id_alias_overrides={
            "id": TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
        },
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event transaction did not return txID" in str(exc)
    else:
        raise AssertionError("source-event transaction without txID was accepted")


def test_live_evidence_rejects_source_event_transaction_raw_data_owner_mismatch():
    module = load_live_module()
    raw_data_hex = tron_source_event_raw_data_hex(owner20=bytes.fromhex("99" * 20))
    transaction_id = hashlib.sha256(bytes.fromhex(raw_data_hex)).hexdigest()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=transaction_id,
        source_event_transaction_raw_data_hex=raw_data_hex,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(transaction_id),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "raw_data_hex owner_address does not match" in str(exc)
    else:
        raise AssertionError("source-event transaction with wrong raw_data owner was accepted")


def test_live_evidence_rejects_source_event_transaction_multiple_signatures():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=[
            TRON_SOURCE_EVENT_SIGNATURE_VECTOR,
            TRON_SOURCE_EVENT_SIGNATURE_VECTOR,
        ],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "exactly one signature" in str(exc)
    else:
        raise AssertionError("source-event transaction with multiple signatures was accepted")


def test_live_evidence_rejects_source_event_transaction_malformed_signature():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=["12" * 64],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "canonical 65-byte" in str(exc)
    else:
        raise AssertionError("source-event transaction with malformed signature was accepted")


def test_live_evidence_rejects_source_event_transaction_high_s_signature():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=[
            tron_high_s_signature_hex(module, TRON_SOURCE_EVENT_SIGNATURE_VECTOR)
        ],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event transaction signature must be a canonical 65-byte" in str(
            exc
        )
    else:
        raise AssertionError("source-event transaction with high-S signature was accepted")


def test_live_evidence_rejects_source_event_signature_with_internal_whitespace():
    module = load_live_module()
    signature = (
        TRON_SOURCE_EVENT_SIGNATURE_VECTOR[:2]
        + "  "
        + TRON_SOURCE_EVENT_SIGNATURE_VECTOR[2:]
    )
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=[signature],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "transaction signature must not contain whitespace" in str(exc)
    else:
        raise AssertionError(
            "source-event transaction signature with whitespace was accepted"
        )


def test_live_evidence_rejects_source_event_transaction_wrong_signature_signer():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=[("12" * 64) + "00"],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "does not recover to source bridge owner" in str(exc)
    else:
        raise AssertionError("source-event transaction with wrong signer was accepted")


def test_live_evidence_source_event_transaction_id_requires_digest():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=None,
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "--source-event-transaction-id requires --source-event-digest" in str(
            exc
        )
    else:
        raise AssertionError("source-event transaction id was accepted without digest")


def test_live_evidence_rejects_source_event_transaction_without_matching_log():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_digest="35" * 32,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "SccpSourceEvent(bytes32)" in str(exc)
    else:
        raise AssertionError("source-event transaction with wrong log was accepted")


def test_live_evidence_rejects_duplicate_source_event_transaction_logs():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_duplicate_matching_log=True,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "exactly one matching SccpSourceEvent" in str(exc)
    else:
        raise AssertionError("source-event transaction with duplicate logs was accepted")


def test_live_evidence_rejects_padded_source_event_log_address():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_address_override=" " + TRON_TEST_BRIDGE20.hex(),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "SccpSourceEvent(bytes32)" in str(exc)
    else:
        raise AssertionError("padded source-event log address was accepted")


def test_live_evidence_rejects_source_event_log_address_with_internal_whitespace():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_address_override=("11" * 9) + "  " + ("11" * 10),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "SccpSourceEvent(bytes32)" in str(exc)
    else:
        raise AssertionError("internally spaced source-event log address was accepted")


def test_live_evidence_rejects_padded_source_event_log_topic():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_topic0_override=" "
        + module.TRON_SOURCE_EVENT_TOPIC.hex(),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "SccpSourceEvent(bytes32)" in str(exc)
    else:
        raise AssertionError("padded source-event log topic was accepted")


def test_live_evidence_rejects_padded_source_event_raw_data_hex():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_raw_data_hex=" " + TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "raw_data_hex must not contain surrounding whitespace" in str(exc)
    else:
        raise AssertionError("padded source-event raw_data_hex was accepted")


def test_live_evidence_rejects_uppercase_source_event_raw_data_hex():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_raw_data_hex=TRON_SOURCE_EVENT_RAW_DATA_HEX_VECTOR.upper(),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "raw_data_hex must be canonical lowercase hex" in str(exc)
    else:
        raise AssertionError("uppercase source-event raw_data_hex was accepted")


def test_live_evidence_rejects_padded_source_event_signature():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_signatures=[" " + TRON_SOURCE_EVENT_SIGNATURE_VECTOR],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "transaction signature must not contain surrounding whitespace" in str(exc)
    else:
        raise AssertionError("padded source-event signature was accepted")


def test_live_evidence_rejects_padded_source_event_trigger_calldata():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_call_data_override=" "
        + TRON_SOURCE_EVENT_CALL_DATA_VECTOR,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "transaction data must not contain surrounding whitespace" in str(exc)
    else:
        raise AssertionError("padded source-event trigger calldata was accepted")


def test_live_evidence_rejects_source_event_trigger_calldata_with_internal_whitespace():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_call_data_override=(
            TRON_SOURCE_EVENT_CALL_DATA_VECTOR[:8]
            + "  "
            + TRON_SOURCE_EVENT_CALL_DATA_VECTOR[8:]
        ),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "transaction data must not contain whitespace" in str(exc)
    else:
        raise AssertionError("internally spaced source-event trigger calldata was accepted")


def test_live_evidence_rejects_uppercase_source_event_trigger_calldata():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_call_data_override=(
            TRON_SOURCE_EVENT_CALL_DATA_VECTOR.upper()
        ),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source-event transaction data must be canonical lowercase hex" in str(
            exc
        )
    else:
        raise AssertionError("uppercase source-event trigger calldata was accepted")


def test_live_evidence_rejects_non_canonical_source_event_log_data_prefix():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_data="0X",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "SccpSourceEvent(bytes32)" in str(exc)
    else:
        raise AssertionError("non-canonical source-event log data was accepted")


def test_live_evidence_rejects_source_event_transaction_wrong_owner():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_owner_override="41" + "99" * 20,
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "owner_address does not match" in str(exc)
    else:
        raise AssertionError("source-event transaction with wrong owner was accepted")


def test_live_evidence_rejects_source_event_transaction_failed_ret():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_ret=[{"contractRet": "REVERT"}],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "contractRet must be SUCCESS" in str(exc)
    else:
        raise AssertionError("source-event transaction with failed ret was accepted")


def test_live_evidence_rejects_source_event_transaction_failed_ret_enum():
    module = load_live_module()
    for ret_value in ("FAILED", "SUCCESS"):
        fake = fake_opener_for(
            module,
            submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
            source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
            source_event_transaction_ret=[{"ret": ret_value, "contractRet": "SUCCESS"}],
        )

        try:
            module.collect_live_evidence(
                SimpleNamespace(
                    tron_node_url="https://tron.example",
                    source_bridge_address=fake.bridge,
                    destination_verifier_address=None,
                    caller_address=None,
                    no_getcontract=False,
                    source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                    source_event_transaction_id=bytes.fromhex(
                        TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                    ),
                    full_toml=False,
                    timeout=1.0,
                ),
                opener=fake.opener,
            )
        except RuntimeError as exc:
            assert "ret enum must be SUCESS" in str(exc)
        else:
            raise AssertionError(
                f"source-event transaction with ret {ret_value!r} was accepted"
            )


def test_live_evidence_rejects_source_event_transaction_multiple_ret_results():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_ret=[
            {"contractRet": "SUCCESS"},
            {"contractRet": "SUCCESS"},
        ],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "one ret result" in str(exc)
    else:
        raise AssertionError("source-event transaction with multiple ret results was accepted")


def test_live_evidence_rejects_source_event_transaction_wrong_calldata():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_call_data_override=(
            "06841e30" + "00" * 31 + "05" + "00" * 32 + "35" * 32
        ),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "calldata does not match" in str(exc)
    else:
        raise AssertionError("source-event transaction with wrong calldata was accepted")


def test_live_evidence_rejects_source_event_transaction_with_extra_topics():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_digests=[TRON_SOURCE_EVENT_DIGEST_VECTOR],
        source_event_transaction_id=TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR,
        source_event_transaction_topics_extra=["00" * 32],
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                source_event_transaction_id=bytes.fromhex(
                    TRON_SOURCE_EVENT_TRANSACTION_ID_VECTOR
                ),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "SccpSourceEvent(bytes32)" in str(exc)
    else:
        raise AssertionError("source-event transaction with extra topics was accepted")


def test_live_evidence_rejects_malformed_source_event_replay_word():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        submitted_source_event_word_override=abi_word_u32(2),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "submittedSourceEvents(bytes32) must be an ABI-encoded bool" in str(
            exc
        )
    else:
        raise AssertionError("malformed source-event replay word was accepted")


def test_live_evidence_source_event_digest_requires_json_source_bridge():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                full_toml=False,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "--source-event-digest requires --source-bridge-address" in str(exc)
    else:
        raise AssertionError("source-event calldata rendered without source bridge")

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
                full_toml=True,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "--source-event-digest is only supported for JSON evidence output" in str(
            exc
        )
    else:
        raise AssertionError("source-event calldata rendered in full TOML mode")


def test_live_evidence_rejects_mismatched_source_config_hash():
    module = load_live_module()
    fake = fake_opener_for(module, source_config_override=bytes.fromhex("99" * 32))

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "sourceBridgeConfigHash" in str(exc)
    else:
        raise AssertionError("mismatched source bridge config hash was accepted")


def test_live_evidence_rejects_constant_result_without_success_flag():
    module = load_live_module()
    fake = fake_opener_for(module)

    def opener(_request, timeout):
        del timeout
        return FakeResponse({"constant_result": ["00" * 32]})

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "TRON constant call networkId() failed" in str(exc)
    else:
        raise AssertionError("constant call without success flag was accepted")


def test_live_evidence_redacts_constant_failure_result_message():
    module = load_live_module()
    fake = fake_opener_for(module)

    def opener(_request, timeout):
        del timeout
        return FakeResponse(
            {
                "result": {
                    "result": False,
                    "message": "secret-token constant failure detail",
                },
                "constant_result": ["00" * 32],
            }
        )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=opener,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TRON constant call networkId() failed"
        assert "secret-token" not in message
        assert "failure detail" not in message
    else:
        raise AssertionError("secret-bearing constant call failure was accepted")


def test_live_evidence_rejects_padded_constant_result_word():
    module = load_live_module()
    fake = fake_opener_for(module)

    def opener(_request, timeout):
        del timeout
        return FakeResponse(
            {
                "result": {"result": True},
                "constant_result": [" " + "00" * 32],
            }
        )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "TRON constant call networkId() returned non-hex data" in str(exc)
    else:
        raise AssertionError("padded constant call word was accepted")


def test_live_evidence_redacts_constant_result_word_parser_cause():
    module = load_live_module()
    fake = fake_opener_for(module)

    def opener(_request, timeout):
        del timeout
        return FakeResponse(
            {
                "result": {"result": True},
                "constant_result": ["0xsecret-token constant parser detail"],
            }
        )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=opener,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "TRON constant call networkId() returned non-hex data"
        assert "secret-token" not in message
        assert "parser detail" not in message
        assert "canonical lowercase hex" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret-bearing constant call word was accepted")


def test_live_evidence_rejects_non_production_source_lane():
    module = load_live_module()
    wrong_source = fake_opener_for(module, source_source_domain=1)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=wrong_source.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=wrong_source.opener,
        )
    except ValueError as exc:
        assert "source_domain must be TRON" in str(exc)
    else:
        raise AssertionError("non-TRON source bridge lane was accepted")

    fake = fake_opener_for(module, source_target_domain=1)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "target_domain must be SORA" in str(exc)
    else:
        raise AssertionError("non-production TRON source lane was accepted")


def test_live_evidence_rejects_mismatched_destination_binding_hash():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_binding_override=bytes.fromhex("aa" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "destinationBindingHash" in str(exc)
    else:
        raise AssertionError("mismatched destination binding hash was accepted")


def test_live_evidence_rejects_expected_destination_binding_hash_mismatch():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                expected_destination_binding_hash=bytes.fromhex("aa" * 32),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-destination-binding-hash" in str(exc)
    else:
        raise AssertionError("mismatched expected destination binding was accepted")


def test_live_evidence_rejects_programmatic_malformed_hex32_bytes():
    module = load_live_module()
    fake = fake_opener_for(module)

    for bad_hash, expected in ((b"", "must be 32 bytes"), (bytes(32), "must not be zero")):
        try:
            module.collect_live_evidence(
                SimpleNamespace(
                    tron_node_url="https://tron.example",
                    source_bridge_address=None,
                    destination_verifier_address=fake.destination,
                    caller_address=None,
                    no_getcontract=True,
                    timeout=1.0,
                    expected_destination_binding_hash=bad_hash,
                ),
                opener=fake.opener,
            )
        except module.evidence.argparse.ArgumentTypeError as exc:
            assert "expected destination binding hash" in str(exc)
            assert expected in str(exc)
        else:
            raise AssertionError("malformed programmatic hex32 bytes were accepted")


def test_live_evidence_expected_destination_binding_requires_destination():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                expected_destination_binding_hash=fake.destination_binding,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-destination-binding-hash" in str(exc)
        assert "destination-verifier-address" in str(exc)
    else:
        raise AssertionError("destination binding pin was accepted without destination")


def test_live_evidence_rejects_source_destination_network_id_drift():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_network_id_override=bytes.fromhex("99" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "destination verifier networkId()" in str(exc)
    else:
        raise AssertionError("source/destination network id drift was accepted")


def test_live_evidence_rejects_destination_bytecode_metadata_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=bytes.fromhex("6002600055"),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "verifierCodeHash()" in str(exc)
    else:
        raise AssertionError("destination bytecode/hash mismatch was accepted")


def test_live_evidence_rejects_destination_backend_hash_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_backend_hash_override=bytes.fromhex("88" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "verifierBackendHash() is not tron-groth16-bn254-v1" in str(exc)
    else:
        raise AssertionError("destination backend hash mismatch was accepted")


def test_live_evidence_rejects_destination_proof_family_hash_mismatch():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_proof_family_hash_override=bytes.fromhex("99" * 32),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "proofFamilyHash() is not stark-fri-v1" in str(exc)
    else:
        raise AssertionError("destination proof-family hash mismatch was accepted")


def test_live_evidence_rejects_missing_destination_bytecode_metadata():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "destination verifier bytecode" in str(exc)
    else:
        raise AssertionError("missing destination verifier bytecode was accepted")


def test_live_evidence_rejects_mismatched_destination_metadata_address():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        destination_metadata_address_override=module.tron_base58check_from_address20(
            bytes.fromhex("99" * 20)
        ),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "destination verifier contract_address" in str(exc)
    else:
        raise AssertionError("mismatched destination metadata address was accepted")


def test_live_evidence_rejects_malformed_destination_metadata_address():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        destination_metadata_address_override="not-a-tron-address",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "malformed destination verifier contract_address" in str(exc)
    else:
        raise AssertionError("malformed destination metadata address was accepted")


def test_live_evidence_rejects_malformed_destination_metadata_bytecode():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode="0xnot-runtime-bytecode",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=None,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "malformed destination verifier bytecode" in str(exc)
    else:
        raise AssertionError("malformed destination metadata bytecode was accepted")


def test_live_evidence_redacts_metadata_parser_exception_causes(monkeypatch):
    module = load_live_module()

    try:
        module._metadata_runtime_bytecode(
            {"bytecode": "0xsecret-token-bytecode"},
            label="destination verifier",
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == (
            "/wallet/getcontract returned malformed destination verifier bytecode"
        )
        assert "secret-token" not in message
        assert "must be hex" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret-bearing metadata bytecode was accepted")

    function_selector = module.evidence.TRON_SOURCE_MESSAGE_CALL_ABI.decode("ascii")
    for exception_type in (TypeError, ValueError):

        def fail_hex_blob(
            _value,
            *,
            label,
            nonzero=True,
            exception_type=exception_type,
        ):
            raise exception_type(f"secret-token {label} bytecode parser detail")

        with monkeypatch.context() as patch:
            patch.setattr(module, "_parse_exact_hex_blob", fail_hex_blob)
            try:
                module._metadata_runtime_bytecode(
                    {"bytecode": "0xignored"},
                    label="destination verifier",
                )
            except RuntimeError as exc:
                message = str(exc)
                assert message == (
                    "/wallet/getcontract returned malformed destination verifier bytecode"
                )
                assert "secret-token" not in message
                assert "parser detail" not in message
                assert exception_type.__name__ not in message
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    "secret-bearing metadata bytecode parser was accepted"
                )

    for exception_type in (TypeError, ValueError):

        def fail_address_parser(_value, *, label, exception_type=exception_type):
            raise exception_type(f"secret-token {label} parser detail")

        with monkeypatch.context() as patch:
            patch.setattr(module, "parse_tron_address_payload", fail_address_parser)
            try:
                module._check_contract_metadata_address(
                    {"contract_address": "TSecretToken"},
                    expected_payload=b"\x41" + (b"\x11" * 20),
                    label="destination verifier",
                )
            except RuntimeError as exc:
                message = str(exc)
                assert message == (
                    "/wallet/getcontract returned malformed destination verifier "
                    "contract_address"
                )
                assert "secret-token" not in message
                assert "parser detail" not in message
                assert exception_type.__name__ not in message
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("secret-bearing metadata address was accepted")

        with monkeypatch.context() as patch:
            patch.setattr(module, "parse_tron_address_payload", fail_address_parser)
            try:
                module._parse_transaction_address_payload(
                    "TSecretToken",
                    label="source-event transaction owner_address",
                )
            except RuntimeError as exc:
                message = str(exc)
                assert message == (
                    "source-event transaction owner_address is not a valid TRON address"
                )
                assert "secret-token" not in message
                assert "parser detail" not in message
                assert exception_type.__name__ not in message
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError("secret-bearing transaction address was accepted")

        with monkeypatch.context() as patch:
            patch.setattr(module, "parse_tron_address_payload", fail_address_parser)
            assert (
                module._source_event_trigger_request_verified(
                    {
                        "endpoint": "wallet/triggersmartcontract",
                        "owner_address": "TSecretToken",
                        "contract_address": "TSecretToken",
                        "function_selector": function_selector,
                        "parameter": "",
                        "visible": True,
                        "call_value": 0,
                    },
                    owner_payload=b"\x11" * 20,
                    source_bridge_payload=b"\x22" * 20,
                    source_event_call_data=b"\x12\x34\x56\x78",
                )
                is False
            )


def test_live_evidence_sends_runtime_trongrid_api_key_file_without_printing_it(tmp_path):
    module = load_live_module()
    api_key_file = tmp_path / "trongrid.key"
    api_key_file.write_text("runtime-secret-key\n", encoding="utf-8")
    fake = fake_opener_for(module, expected_api_key="runtime-secret-key")

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=True,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=str(api_key_file),
        ),
        opener=fake.opener,
    )

    assert summary["source_bridge"]["address"] == fake.bridge
    assert "runtime-secret-key" not in json.dumps(summary)


def test_live_evidence_can_use_solid_constant_endpoint():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        expected_constant_endpoint="/walletsolidity/triggerconstantcontract",
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=True,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=True,
        ),
        opener=fake.opener,
    )

    assert summary["constant_endpoint"] == "walletsolidity/triggerconstantcontract"
    assert summary["source_bridge"]["config_hash_matches"] is True
    assert summary["destination_verifier"]["destination_binding_hash_matches"] is True


def test_live_evidence_redacts_generated_full_toml_parser_exception_cause(
    monkeypatch,
):
    module = load_live_module()

    class SecretFailingParser:
        def parse_args(self, _args):
            raise SystemExit("secret-token generated full TOML parser detail")

    monkeypatch.setattr(
        module,
        "_offline_full_toml_args",
        lambda _summary: ["--secret-token-generated-arg"],
    )
    monkeypatch.setattr(module.evidence, "build_parser", SecretFailingParser)

    try:
        module.render_offline_full_toml({"destination_verifier": {}})
    except RuntimeError as exc:
        message = str(exc)
        assert message == "generated offline full TOML arguments are invalid"
        assert "secret-token" not in message
        assert "parser detail" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("secret-bearing generated full TOML parser was accepted")


def test_live_evidence_preflights_source_records_and_full_rollout_args(monkeypatch):
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=False,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            source_trust_anchor_hash=expected.args.source_trust_anchor_hash,
            consensus_verifier_hash=expected.args.consensus_verifier_hash,
            message_inclusion_verifier_hash=(
                expected.args.message_inclusion_verifier_hash
            ),
            source_bridge_emitter_code_hash=source_code_hash,
            expected_source_bridge_config_hash=fake.source_config,
            finality_policy_hash=expected.args.finality_policy_hash,
            deployment_receipt_hash=expected.args.deployment_receipt_hash,
            adapter_verifier_vk_hash=None,
            expected_source_verifier_material_hash=expected.material_hash,
            expected_source_adapter_engine_deployment_hash=expected.deployment_hash,
            expected_tron_dpos_source_gate_hash=expected.gate_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=expected.route_hash,
            route_canary_evidence_hash=None,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )

    source_records = summary["source_records"]
    assert (
        source_records["adapter_verifier_vk_hash"]
        == "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )
    assert (
        source_records["source_verifier_material_hash"]
        == "0x" + expected.material_hash.hex()
    )
    assert (
        source_records["source_adapter_engine_deployment_hash"]
        == "0x" + expected.deployment_hash.hex()
    )
    assert (
        source_records["tron_dpos_source_gate_hash"]
        == "0x" + expected.gate_hash.hex()
    )
    assert source_records["expected_source_verifier_material_hash_matches"] is True
    assert (
        source_records["expected_source_adapter_engine_deployment_hash_matches"] is True
    )
    assert source_records["expected_tron_dpos_source_gate_hash_matches"] is True
    assert summary["route_allowlist_hash"] == "0x" + expected.route_hash.hex()
    assert (
        summary["expected_route_allowlist_hash"]
        == "0x" + expected.route_hash.hex()
    )
    assert summary["expected_route_allowlist_hash_matches"] is True
    assert (
        summary["source_bridge"]["expected_source_bridge_config_hash_matches"]
        is True
    )
    assert (
        summary["destination_verifier"]["expected_destination_binding_hash_matches"]
        is True
    )

    offline_args = summary["offline_evidence_args"]
    assert "--expected-config-hash" in offline_args
    assert "0x" + fake.source_config.hex() in offline_args
    assert "--expected-source-verifier-material-hash" in offline_args
    assert "0x" + expected.material_hash.hex() in offline_args
    assert "--expected-source-adapter-engine-deployment-hash" in offline_args
    assert "0x" + expected.deployment_hash.hex() in offline_args
    assert "--expected-tron-dpos-source-gate-hash" in offline_args
    assert "0x" + expected.gate_hash.hex() in offline_args
    assert "--expected-destination-binding-hash" in offline_args
    assert "0x" + fake.destination_binding.hex() in offline_args
    assert "--route-allowlist-hash" in offline_args
    assert "0x" + expected.route_hash.hex() in offline_args
    assert "--route-canary-evidence-hash" in offline_args
    assert "--route-canary-transaction-owner-address" in offline_args
    assert "0x41" + fake.owner20.hex() in offline_args
    assert "--route-canary-raw-data-owner-matches-transaction" in offline_args
    assert "--route-canary-signature-sha256" in offline_args
    assert "--route-canary-signature-recovered-address" in offline_args
    assert "--route-canary-signature-recovers-to-owner" in offline_args
    assert summary["route_canary"]["status"] == "passed"
    assert summary["route_canary"]["evidence_source"] == (
        "tron_message_proof_accepted_transaction"
    )
    assert summary["route_canary"]["evidence_hash"] in offline_args
    assert summary["full_toml_ready"] is True
    assert summary["torii_destination_query_params"] == {
        "network_id_hex": "0x" + "33" * 32,
        "tron_verifier_address": fake.destination,
        "verifier_code_hash_hex": "0x" + fake.destination_code_hash.hex(),
        "verifier_key_hash_hex": "0x" + "cc" * 32,
        "expected_destination_binding_hash_hex": "0x" + fake.destination_binding.hex(),
    }
    assert summary["torii_destination_query_proof_bytes_hex_required"] is True
    for tamper in (
        lambda destination: destination.update(
            {"destination_binding_hash": "0x" + "99" * 32}
        ),
        lambda destination: destination.update({"verifier_backend_hash_matches": False}),
        lambda destination: destination.update({"destination_source_domain": 1}),
        lambda destination: destination.update({"destination_source_domain": False}),
        lambda destination: destination.update({"destination_target_domain": "05"}),
        lambda destination: destination.update({"network_id": "0x" + "00" * 32}),
    ):
        tampered_summary = dict(summary)
        tampered_summary["destination_verifier"] = dict(summary["destination_verifier"])
        tamper(tampered_summary["destination_verifier"])
        assert module._torii_destination_query_params(tampered_summary) is None

    full_toml_args = summary["offline_full_toml_args"]
    assert full_toml_args[:-1] == offline_args
    assert full_toml_args[-1] == "--full-toml"
    full_toml = module.render_offline_full_toml(summary)
    assert "# sccp_tron_source_bridge_runtime_code_hash" in full_toml
    assert "# sccp_tron_source_bridge_runtime_bytecode_hex" in full_toml
    assert "# sccp_tron_source_bridge_config_hash" in full_toml
    assert "# sccp_tron_dpos_source_gate_hash" in full_toml
    assert "tron_dpos_source_gate_hash = " in full_toml
    assert "# sccp_tron_destination_verifier_runtime_code_hash" in full_toml
    assert "# sccp_tron_destination_verifier_runtime_bytecode_hex" in full_toml
    assert "# sccp_tron_destination_verifier_key_hash" in full_toml
    assert "# sccp_tron_destination_verifier_backend_hash" in full_toml
    assert "# sccp_tron_destination_proof_family_hash" in full_toml
    assert "# sccp_tron_route_canary_transaction_id" in full_toml
    assert "# sccp_tron_route_canary_transaction_owner_address" in full_toml
    assert '# sccp_tron_route_canary_block_number = "234"' in full_toml
    assert '# sccp_tron_route_canary_block_timestamp = "567000"' in full_toml
    assert "# sccp_tron_route_canary_statement_hash" in full_toml
    assert '# sccp_tron_route_canary_used_message_proof = "true"' in full_toml
    assert (
        '# sccp_tron_route_canary_raw_data_owner_matches_transaction = "true"'
        in full_toml
    )
    assert "tron_route_canary_transaction_id = " in full_toml
    assert "tron_route_canary_transaction_owner_address = " in full_toml
    assert "tron_route_canary_log_index = " in full_toml
    assert "tron_route_canary_message_id = " in full_toml
    assert "tron_route_canary_statement_hash = " in full_toml
    assert "tron_route_canary_commitment_root = " in full_toml
    assert "tron_route_canary_used_message_proof = true" in full_toml
    assert "tron_route_canary_raw_data_owner_matches_transaction = true" in full_toml
    assert (
        '# sccp_route_canary_route_allowlist_hash = "0x'
        + expected.route_hash.hex()
        + '"'
        in full_toml
    )
    assert "# sccp_route_canary_route_allowlist_hash = #" not in full_toml
    assert (
        '# sccp_route_canary_destination_binding_hash = "0x'
        + fake.destination_binding.hex()
        + '"'
        in full_toml
    )
    for key in (
        "# sccp_tron_source_bridge_address = ",
        "# sccp_tron_source_bridge_runtime_code_hash = ",
        "# sccp_tron_source_bridge_runtime_bytecode_hex = ",
        "# sccp_tron_source_bridge_config_hash = ",
        "# sccp_tron_dpos_source_gate_hash = ",
        "# sccp_tron_destination_verifier_address = ",
        "# sccp_tron_destination_verifier_runtime_code_hash = ",
        "# sccp_tron_destination_verifier_runtime_bytecode_hex = ",
        "# sccp_tron_destination_verifier_key_hash = ",
        "# sccp_tron_destination_verifier_backend_hash = ",
        "# sccp_tron_destination_proof_family_hash = ",
        "# sccp_tron_route_canary_transaction_id = ",
        "# sccp_tron_route_canary_block_number = ",
        "# sccp_tron_route_canary_block_timestamp = ",
        "# sccp_tron_route_canary_log_index = ",
        "# sccp_tron_route_canary_message_id = ",
        "# sccp_tron_route_canary_statement_hash = ",
        "# sccp_tron_route_canary_commitment_root = ",
        "# sccp_tron_route_canary_used_message_proof = ",
        "# sccp_tron_route_canary_raw_data_owner_matches_transaction = ",
    ):
        assert full_toml.count(key) == 1
    assert "[[zk.sccp_source_verifier_materials]]" in full_toml
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in full_toml
    assert "[[zk.sccp_destination_rollouts]]" in full_toml
    assert "[[zk.sccp_route_allowlists]]" in full_toml
    assert '# sccp_route_canary_status = "passed"' in full_toml
    assert (
        summary["offline_full_toml_sha256"]
        == hashlib.sha256(full_toml.encode("utf-8")).hexdigest()
    )
    for field in (
        "expected_source_verifier_material_hash_matches",
        "expected_source_adapter_engine_deployment_hash_matches",
        "expected_tron_dpos_source_gate_hash_matches",
    ):
        tampered_summary = dict(summary)
        tampered_summary["source_records"] = dict(summary["source_records"])
        tampered_summary["source_records"].pop(field)
        try:
            module.render_offline_full_toml(tampered_summary)
        except ValueError as exc:
            assert "expected source record hashes" in str(exc)
        else:
            raise AssertionError(
                "TRON full TOML rendered without source record hash match flags"
            )
    for tamper, expected_message in (
        (
            lambda destination: destination.pop(
                "bytecode_hash_matches_verifier_code_hash"
            ),
            "destination /wallet/getcontract bytecode to match verifierCodeHash",
        ),
        (
            lambda destination: destination.update(
                {"tron_getcontract_bytecode_hash": "0x" + "99" * 32}
            ),
            "destination runtime bytecode hash does not match",
        ),
        (
            lambda destination: destination.pop(
                "destination_verifier_runtime_bytecode_hex"
            ),
            "runtime bytecode preimage for the destination verifier",
        ),
        (
            lambda destination: destination.update(
                {"destination_verifier_runtime_bytecode_hex": "0xsecret-token"}
            ),
            "TRON destination verifier runtime bytecode metadata is invalid",
        ),
        (
            lambda destination: destination.update(
                {"destination_verifier_runtime_bytecode_hex": "0x600260ff55"}
            ),
            "destination runtime bytecode hash does not match",
        ),
    ):
        tampered_summary = dict(summary)
        tampered_summary["destination_verifier"] = dict(summary["destination_verifier"])
        tamper(tampered_summary["destination_verifier"])
        assert (
            "offline_full_toml_args" not in tampered_summary
            or module._offline_full_toml_args(tampered_summary) is None
        )
        try:
            module.render_offline_full_toml(tampered_summary)
        except ValueError as exc:
            assert expected_message in str(exc)
            assert "secret-token" not in str(exc)
            assert "must be hex" not in str(exc)
        else:
            raise AssertionError(
                "TRON full TOML rendered without destination bytecode/hash match"
            )

    original_parse_runtime_bytecode_hex = module.evidence.parse_runtime_bytecode_hex
    with monkeypatch.context() as patch:
        def fail_destination_runtime(value, *, label):
            if label == "destination verifier runtime bytecode":
                raise TypeError(f"secret-token {label} helper TypeError detail")
            return original_parse_runtime_bytecode_hex(value, label=label)

        patch.setattr(
            module.evidence,
            "parse_runtime_bytecode_hex",
            fail_destination_runtime,
        )
        tampered_summary = dict(summary)
        tampered_summary["destination_verifier"] = dict(
            summary["destination_verifier"]
        )
        try:
            module.render_offline_full_toml(tampered_summary)
        except ValueError as exc:
            rendered = str(exc)
            assert (
                "TRON destination verifier runtime bytecode metadata is invalid"
                in rendered
            )
            assert "secret-token" not in rendered
            assert "helper TypeError detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("TRON full TOML leaked destination parser TypeError")

        try:
            module._annotate_full_toml_with_live_metadata(full_toml, tampered_summary)
        except ValueError as exc:
            rendered = str(exc)
            assert (
                "TRON destination verifier runtime bytecode metadata is invalid"
                in rendered
            )
            assert "secret-token" not in rendered
            assert "helper TypeError detail" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError(
                "TRON full TOML annotation leaked destination parser TypeError"
            )


def test_live_evidence_derives_route_canary_from_verifier_transaction():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )

    route_canary = summary["route_canary"]
    transaction = summary["route_canary_transaction"]
    assert route_canary["evidence_source"] == (
        "tron_message_proof_accepted_transaction"
    )
    assert route_canary["transaction"] == transaction
    assert route_canary["evidence_hash"] == transaction["route_canary_evidence_hash"]
    assert transaction["transaction_id"] == "0x" + route_canary_transaction_id
    assert transaction["block_number"] == 234
    assert transaction["block_timestamp"] == 567000
    assert transaction["trigger_contract"]["owner_address"] == (
        "0x41" + fake.owner20.hex()
    )
    assert transaction["receipt_status"] == "SUCCESS"
    assert transaction["event_topic0"] == (
        "0x" + module.TRON_MESSAGE_PROOF_ACCEPTED_TOPIC.hex()
    )
    assert transaction["used_message_proofs_checked"] is True
    assert transaction["message_proof_used"] is True
    assert transaction["used_message_proofs_function"] == "usedMessageProofs(bytes32)"
    assert transaction["used_message_proofs_parameter"] == transaction["message_id"]
    assert transaction["source_domain"] == 0
    assert transaction["destination_binding_hash"] == (
        "0x" + fake.destination_binding.hex()
    )
    assert transaction["route_allowlist_hash"] == "0x" + expected.route_hash.hex()
    trigger_contract = transaction["trigger_contract"]
    assert trigger_contract["call_matches"] is True
    assert trigger_contract["raw_data_call_matches"] is True
    assert trigger_contract["raw_data_owner_matches_transaction"] is True
    assert trigger_contract["signature_count"] == 1
    assert trigger_contract["signature_recovers_to_owner"] is True
    assert trigger_contract["signature_recovered_address"] == (
        trigger_contract["owner_address"]
    )
    assert trigger_contract["call_data_matches_event"] is True
    assert trigger_contract["function_selector"] == "0xbd57826c"
    assert trigger_contract["function_signature"] == (
        "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
    )
    assert trigger_contract["public_inputs_message_id"] == transaction["message_id"]
    assert trigger_contract["public_inputs_commitment_root"] == (
        transaction["commitment_root"]
    )
    assert trigger_contract["statement_hash"] == transaction["statement_hash"]
    assert trigger_contract["public_inputs_target_domain"] == 5
    assert trigger_contract["proof_bytes_length"] == 384
    assert trigger_contract["proof_version"] == 1
    assert trigger_contract["proof_source_domain"] == 0
    assert "--route-canary-evidence-hash" in summary["offline_full_toml_args"]
    assert "--route-canary-transaction-owner-address" in summary["offline_full_toml_args"]
    assert trigger_contract["owner_address"] in summary["offline_full_toml_args"]
    assert (
        "--route-canary-raw-data-owner-matches-transaction"
        in summary["offline_full_toml_args"]
    )
    assert route_canary["evidence_hash"] in summary["offline_full_toml_args"]
    assert summary["full_toml_ready"] is True


def test_live_evidence_full_toml_replay_requires_route_canary_block_metadata():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            setup.fake,
            setup.expected,
            source_code_hash=setup.source_code_hash,
            route_canary_transaction_id=bytes.fromhex(
                TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
            ),
        ),
        opener=setup.fake.opener,
    )

    for field, value in (
        ("block_number", 0),
        ("block_timestamp", None),
    ):
        tampered_summary = json.loads(json.dumps(summary))
        tampered_summary["route_canary_transaction"][field] = value
        tampered_summary["route_canary"]["transaction"][field] = value
        assert module._route_canary_transaction_verified(tampered_summary) is False
        assert module._offline_full_toml_args(tampered_summary) is None
        try:
            module.render_offline_full_toml(tampered_summary)
        except ValueError as exc:
            assert "route-canary-transaction-id" in str(exc)
        else:
            raise AssertionError(
                "TRON full TOML rendered without route-canary block metadata"
            )


def test_live_evidence_full_toml_requires_expected_tron_dpos_source_gate_hash():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )
    args = live_full_rollout_args(
        fake,
        expected,
        source_code_hash=source_code_hash,
        route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
    )
    args.expected_tron_dpos_source_gate_hash = None

    summary = module.collect_live_evidence(args, opener=fake.opener)

    assert (
        "expected_tron_dpos_source_gate_hash_matches"
        not in summary["source_records"]
    )
    assert "offline_full_toml_args" not in summary
    assert summary["full_toml_ready"] is False
    try:
        module.render_offline_full_toml(summary)
    except ValueError as exc:
        assert "--expected-tron-dpos-source-gate-hash" in str(exc)
    else:
        raise AssertionError("TRON full TOML rendered without expected source gate")


def test_live_evidence_rejects_expected_tron_dpos_source_gate_hash_mismatch():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
    )
    args = live_full_rollout_args(
        setup.fake,
        setup.expected,
        source_code_hash=setup.source_code_hash,
        route_canary_transaction_id=bytes.fromhex(TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR),
    )
    args.expected_tron_dpos_source_gate_hash = bytes.fromhex("ce" * 32)

    try:
        module.collect_live_evidence(args, opener=setup.fake.opener)
    except ValueError as exc:
        assert "--expected-tron-dpos-source-gate-hash" in str(exc)
    else:
        raise AssertionError("mismatched live TRON DPoS source gate hash was accepted")


def test_live_evidence_full_toml_requires_route_canary_raw_data_owner_binding():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )
    tampered_summary = dict(summary)
    tampered_transaction = dict(summary["route_canary_transaction"])
    tampered_trigger = dict(tampered_transaction["trigger_contract"])
    tampered_trigger["raw_data_owner_matches_transaction"] = False
    tampered_transaction["trigger_contract"] = tampered_trigger
    tampered_summary["route_canary_transaction"] = tampered_transaction

    assert module._offline_full_toml_args(tampered_summary) is None
    try:
        module.render_offline_full_toml(tampered_summary)
    except ValueError as exc:
        assert "--route-canary-transaction-id" in str(exc)
    else:
        raise AssertionError(
            "TRON full TOML rendered without route-canary raw_data owner binding"
        )


def test_live_evidence_full_toml_requires_route_canary_signature_owner_binding():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )
    tampered_summary = dict(summary)
    tampered_transaction = dict(summary["route_canary_transaction"])
    tampered_trigger = dict(tampered_transaction["trigger_contract"])
    tampered_trigger["signature_recovered_address"] = "0x41" + "99" * 20
    tampered_transaction["trigger_contract"] = tampered_trigger
    tampered_summary["route_canary_transaction"] = tampered_transaction

    assert module._route_canary_transaction_verified(tampered_summary) is False
    assert module._offline_full_toml_args(tampered_summary) is None
    try:
        module.render_offline_full_toml(tampered_summary)
    except ValueError as exc:
        assert "--route-canary-transaction-id" in str(exc)
    else:
        raise AssertionError(
            "TRON full TOML rendered without route-canary signature owner binding"
        )


def test_live_evidence_full_toml_revalidates_route_canary_raw_transaction_fields():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )

    for field, value in (
        ("raw_data_hex", "0x" + "01" * 16),
        ("signature", "0x" + "01" * 65),
    ):
        tampered_summary = json.loads(json.dumps(summary))
        tampered_transaction = tampered_summary["route_canary_transaction"]
        tampered_transaction["trigger_contract"][field] = value
        tampered_summary["route_canary"]["transaction"] = tampered_transaction

        assert module._route_canary_transaction_verified(tampered_summary) is False
        assert module._offline_full_toml_args(tampered_summary) is None


def test_live_evidence_full_toml_revalidates_route_canary_proof_header_fields():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )
    for field, value in (
        ("proof_version", 2),
        ("proof_source_domain", 1),
        ("public_inputs_finality_height", "0x" + "03" * 32),
        ("public_inputs_finality_block_hash", "0x" + "04" * 32),
    ):
        tampered_summary = json.loads(json.dumps(summary))
        tampered_transaction = tampered_summary["route_canary_transaction"]
        tampered_trigger = tampered_transaction["trigger_contract"]
        tampered_trigger[field] = value
        tampered_summary["route_canary"]["transaction"] = tampered_transaction

        assert module._route_canary_transaction_verified(tampered_summary) is False
        assert module._offline_full_toml_args(tampered_summary) is None
        try:
            module.render_offline_full_toml(tampered_summary)
        except ValueError as exc:
            assert "--route-canary-transaction-id" in str(exc)
        else:
            raise AssertionError(
                "TRON full TOML rendered with tampered "
                f"route-canary proof header field {field}"
            )


def test_tron_route_canary_evidence_hash_rejects_invalid_transcript_fields():
    module = load_live_module()
    owner = b"\x41" + bytes.fromhex("33" * 20)
    base = {
        "route_allowlist_hash": bytes.fromhex("11" * 32),
        "transaction_id": bytes.fromhex("22" * 32),
        "transaction_owner_address": owner,
        "block_number": 234,
        "block_timestamp": 567000,
        "log_index": 0,
        "verifier_address20": bytes.fromhex("44" * 20),
        "call_data_sha256": bytes.fromhex("55" * 32),
        "message_id": bytes.fromhex("66" * 32),
        "source_domain": module.evidence.SCCP_DOMAIN_SORA,
        "target_domain": module.evidence.SCCP_DOMAIN_TRON,
        "payload_hash": bytes.fromhex("77" * 32),
        "commitment_root": bytes.fromhex("88" * 32),
        "finality_height": bytes.fromhex("99" * 32),
        "finality_block_hash": bytes.fromhex("aa" * 32),
        "statement_hash": bytes.fromhex("bb" * 32),
        "proof_version": module.TRON_GROTH16_PROOF_VERSION,
        "proof_source_domain": module.evidence.SCCP_DOMAIN_SORA,
        "destination_binding_hash": bytes.fromhex("cc" * 32),
        "verifier_backend_hash": bytes.fromhex("dd" * 32),
        "proof_family_hash": bytes.fromhex("ee" * 32),
        "network_id": bytes.fromhex("12" * 32),
        "used_message_proof": True,
        "raw_data_owner_matches_transaction": True,
        "signature_sha256": bytes.fromhex("34" * 32),
        "signature_recovered_address": owner,
        "signature_recovers_to_owner": True,
    }
    assert isinstance(module._tron_route_canary_transaction_evidence_hash(**base), bytes)

    def assert_hash_rejects(field, value, expected):
        candidate = dict(base)
        candidate[field] = value
        try:
            module._tron_route_canary_transaction_evidence_hash(**candidate)
        except RuntimeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(f"{field} was accepted by the route-canary hasher")

    for field, value, expected in (
        ("route_allowlist_hash", bytes(32), "route allowlist hash"),
        ("transaction_id", bytes(32), "transaction id"),
        ("transaction_owner_address", b"\x41" + bytes(20), "transaction owner"),
        ("block_number", True, "block number"),
        ("block_number", 0, "block number"),
        ("block_timestamp", True, "block timestamp"),
        ("block_timestamp", -1, "block timestamp"),
        ("log_index", True, "log index"),
        ("log_index", 0x1_0000_0000, "log index"),
        ("verifier_address20", bytes(20), "verifier address"),
        ("call_data_sha256", bytes(32), "call data SHA-256"),
        ("message_id", bytes(32), "message id"),
        ("source_domain", False, "source domain"),
        ("source_domain", module.evidence.SCCP_DOMAIN_TRON, "source domain"),
        ("target_domain", True, "target domain"),
        ("target_domain", module.evidence.SCCP_DOMAIN_SORA, "target domain"),
        ("payload_hash", bytes(32), "payload hash"),
        ("commitment_root", bytes(32), "commitment root"),
        ("finality_height", bytes(32), "finality height"),
        ("finality_block_hash", bytes(32), "finality block hash"),
        ("statement_hash", bytes(32), "statement hash"),
        ("proof_version", True, "proof version"),
        ("proof_version", module.TRON_GROTH16_PROOF_VERSION + 1, "proof version"),
        ("proof_source_domain", False, "proof source"),
        ("proof_source_domain", module.evidence.SCCP_DOMAIN_TRON, "proof source"),
        ("destination_binding_hash", bytes(32), "destination binding"),
        ("verifier_backend_hash", bytes(32), "verifier backend"),
        ("proof_family_hash", bytes(32), "proof family"),
        ("network_id", bytes(32), "network id"),
        ("used_message_proof", False, "usedMessageProofs"),
        (
            "raw_data_owner_matches_transaction",
            False,
            "raw_data owner",
        ),
        ("signature_sha256", bytes(32), "signature hash"),
        (
            "signature_recovered_address",
            b"\x41" + bytes.fromhex("35" * 20),
            "signature recovered address",
        ),
        ("signature_recovers_to_owner", False, "signature recovery"),
    ):
        assert_hash_rejects(field, value, expected)


def test_live_evidence_full_toml_revalidates_route_canary_destination_fields():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
        ),
        opener=fake.opener,
    )

    def parse_hex(value):
        return bytes.fromhex(value[2:] if value.startswith("0x") else value)

    def rehash(tampered_summary):
        transaction = tampered_summary["route_canary_transaction"]
        destination = tampered_summary["destination_verifier"]
        trigger = transaction["trigger_contract"]
        return (
            "0x"
            + module._tron_route_canary_transaction_evidence_hash(
                route_allowlist_hash=parse_hex(
                    tampered_summary["route_allowlist_hash"]
                ),
                transaction_id=parse_hex(transaction["transaction_id"]),
                transaction_owner_address=parse_hex(trigger["owner_address"]),
                block_number=transaction["block_number"],
                block_timestamp=transaction["block_timestamp"],
                log_index=transaction["log_index"],
                verifier_address20=module.parse_tron_address_payload(
                    destination["address"],
                    label="destination verifier address",
                )[1:],
                call_data_sha256=parse_hex(trigger["call_data_sha256"]),
                message_id=parse_hex(transaction["message_id"]),
                source_domain=transaction["source_domain"],
                target_domain=trigger["public_inputs_target_domain"],
                payload_hash=parse_hex(trigger["public_inputs_payload_hash"]),
                commitment_root=parse_hex(transaction["commitment_root"]),
                finality_height=parse_hex(trigger["public_inputs_finality_height"]),
                finality_block_hash=parse_hex(
                    trigger["public_inputs_finality_block_hash"]
                ),
                statement_hash=parse_hex(transaction["statement_hash"]),
                proof_version=trigger["proof_version"],
                proof_source_domain=trigger["proof_source_domain"],
                destination_binding_hash=parse_hex(
                    transaction["destination_binding_hash"]
                ),
                verifier_backend_hash=parse_hex(transaction["verifier_backend_hash"]),
                proof_family_hash=parse_hex(transaction["proof_family_hash"]),
                network_id=parse_hex(transaction["network_id"]),
                used_message_proof=transaction["message_proof_used"],
                raw_data_owner_matches_transaction=trigger[
                    "raw_data_owner_matches_transaction"
                ],
                signature_sha256=parse_hex(trigger["signature_sha256"]),
                signature_recovered_address=parse_hex(
                    trigger["signature_recovered_address"]
                ),
                signature_recovers_to_owner=trigger["signature_recovers_to_owner"],
            ).hex()
        )

    for field, value, hash_error in (
        ("source_domain", 1, "source domain"),
        ("destination_binding_hash", "0x" + "ab" * 32, None),
        ("verifier_backend_hash", "0x" + "cd" * 32, None),
        ("proof_family_hash", "0x" + "ef" * 32, None),
        ("network_id", "0x" + "12" * 32, None),
    ):
        tampered_summary = json.loads(json.dumps(summary))
        transaction = tampered_summary["route_canary_transaction"]
        transaction[field] = value
        try:
            tampered_hash = rehash(tampered_summary)
        except RuntimeError as exc:
            assert hash_error is not None
            assert hash_error in str(exc)
        else:
            assert hash_error is None
            transaction["route_canary_evidence_hash"] = tampered_hash
            tampered_summary["route_canary"]["evidence_hash"] = tampered_hash
        tampered_summary["route_canary"]["transaction"] = transaction

        assert module._route_canary_transaction_verified(tampered_summary) is False
        assert module._offline_full_toml_args(tampered_summary) is None


def test_live_evidence_rejects_route_canary_hash_mismatch():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                fake,
                expected,
                source_code_hash=source_code_hash,
                route_canary_evidence_hash=bytes.fromhex(
                    TRON_ROUTE_CANARY_EVIDENCE_HASH
                ),
                route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "does not match the MessageProofAccepted transaction evidence hash" in str(
            exc
        )
    else:
        raise AssertionError("mismatched route canary transaction hash was accepted")


def test_live_evidence_redacts_route_canary_topic_parser_failures(monkeypatch):
    module = load_live_module()
    original_parse_exact_hex32 = module._parse_exact_hex32

    for exception_type in (TypeError, ValueError):

        def fail_route_canary_topic(value, *, label, exception_type=exception_type):
            if label == "route-canary log topic0":
                raise exception_type(
                    "secret-token route-canary log topic0 parser detail"
                )
            return original_parse_exact_hex32(value, label=label)

        with monkeypatch.context() as patch:
            patch.setattr(module, "_parse_exact_hex32", fail_route_canary_topic)
            try:
                summary = module._route_canary_message_proof_event_summary(
                    {
                        "address": TRON_TEST_BRIDGE20.hex(),
                        "topics": [
                            module.TRON_MESSAGE_PROOF_ACCEPTED_TOPIC.hex(),
                            "dd" * 32,
                            "00" * 32,
                        ],
                        "data": "",
                    },
                    log_index=0,
                    transaction_id=bytes.fromhex(
                        TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                    ),
                    route_allowlist_hash=bytes.fromhex(
                        TRON_ROUTE_ALLOWLIST_HASH_VECTOR
                    ),
                    verifier_address20=TRON_TEST_BRIDGE20,
                    expected_source_domain=0,
                    expected_destination_binding_hash=bytes.fromhex("11" * 32),
                    expected_verifier_backend_hash=bytes.fromhex("22" * 32),
                    expected_proof_family_hash=bytes.fromhex("33" * 32),
                    expected_network_id=bytes.fromhex("44" * 32),
                )
            except exception_type as exc:
                raise AssertionError(
                    "route canary topic parser failure leaked"
                ) from exc

        assert summary is None


def test_live_evidence_rejects_route_canary_destination_binding_mismatch():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    route_canary_transaction_id = TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
        route_canary_transaction_id=route_canary_transaction_id,
        route_canary_transaction_destination_binding_override=bytes.fromhex(
            "99" * 32
        ),
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                fake,
                expected,
                source_code_hash=source_code_hash,
                route_canary_transaction_id=bytes.fromhex(route_canary_transaction_id),
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "destinationBindingHash does not match live" in str(exc)
    else:
        raise AssertionError("route canary with wrong destination binding was accepted")


def test_live_evidence_rejects_duplicate_route_canary_transaction_logs():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_duplicate_matching_log=True,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "exactly one matching MessageProofAccepted" in str(exc)
    else:
        raise AssertionError("route canary with duplicate logs was accepted")


def test_live_evidence_rejects_route_canary_explicit_log_index_mismatch():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_log_index_override=1,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "route-canary MessageProofAccepted log logIndex does not match "
            "log list index: expected 0, got 1"
        ) in str(exc)
    else:
        raise AssertionError(
            "route canary with mismatched explicit logIndex was accepted"
        )


def test_live_evidence_rejects_route_canary_snake_log_index_mismatch():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_log_index_override=1,
        route_canary_transaction_log_index_fields=("log_index",),
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "route-canary MessageProofAccepted log log_index does not match "
            "log list index: expected 0, got 1"
        ) in str(exc)
    else:
        raise AssertionError(
            "route canary with mismatched explicit log_index was accepted"
        )


def test_live_evidence_rejects_route_canary_duplicate_log_index_aliases():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_log_index_override=0,
        route_canary_transaction_log_index_fields=("logIndex", "log_index"),
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "route-canary MessageProofAccepted log must not include both "
            "logIndex and log_index"
        ) in str(exc)
    else:
        raise AssertionError(
            "route canary with duplicate log-index aliases was accepted"
        )


def test_live_evidence_rejects_route_canary_info_conflicting_txid_aliases():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_info_id_alias_overrides={"txID": "11" * 32},
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "route-canary transaction info returned conflicting transaction id aliases"
            in str(exc)
        )
    else:
        raise AssertionError(
            "route canary transaction info with conflicting id aliases was accepted"
        )


def test_live_evidence_rejects_route_canary_missing_block_number():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_block_number=None,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction info blockNumber must be a positive integer" in str(exc)
    else:
        raise AssertionError("missing TRON route-canary blockNumber was accepted")


def test_live_evidence_rejects_route_canary_missing_block_timestamp():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_block_timestamp=None,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "route-canary transaction info blockTimeStamp must be a non-negative integer"
            in str(exc)
        )
    else:
        raise AssertionError("missing TRON route-canary blockTimeStamp was accepted")


def test_live_evidence_rejects_route_canary_missing_used_message_state():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_used_message_proof=False,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "usedMessageProofs(bytes32) is false" in str(exc)
    else:
        raise AssertionError("route canary without used-message state was accepted")


def test_live_evidence_rejects_route_canary_raw_data_owner_mismatch():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_owner_override="41" + "99" * 20,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "owner_address does not match raw_data_hex owner_address" in str(exc)
    else:
        raise AssertionError("route canary with mismatched raw_data owner was accepted")


def test_live_evidence_rejects_route_canary_conflicting_txid_aliases():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_id_alias_overrides={"id": "11" * 32},
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert (
            "route-canary transaction returned conflicting transaction id aliases"
            in str(exc)
        )
    else:
        raise AssertionError(
            "route canary with conflicting transaction id aliases was accepted"
        )


def test_live_evidence_rejects_route_canary_transaction_without_txid_alias():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_include_txid=False,
        route_canary_transaction_id_alias_overrides={
            "id": TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
        },
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction did not return txID" in str(exc)
    else:
        raise AssertionError("route canary transaction without txID was accepted")


def test_live_evidence_rejects_route_canary_multiple_signatures():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_signatures=[
            TRON_SOURCE_EVENT_SIGNATURE_VECTOR,
            TRON_SOURCE_EVENT_SIGNATURE_VECTOR,
        ],
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction must contain exactly one signature" in str(exc)
    else:
        raise AssertionError("route canary with multiple signatures was accepted")


def test_live_evidence_rejects_route_canary_malformed_signature():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_signatures=["12" * 64],
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction signature must be a canonical 65-byte" in str(
            exc
        )
    else:
        raise AssertionError("route canary with malformed signature was accepted")


def test_live_evidence_rejects_route_canary_high_s_signature():
    module = load_live_module()
    route_canary_signature = tron_signature_hex(
        module,
        hashlib.sha256(bytes.fromhex(TRON_ROUTE_CANARY_RAW_DATA_HEX_VECTOR)).digest(),
        nonce_start=17,
    )
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_signatures=[
            tron_high_s_signature_hex(module, route_canary_signature)
        ],
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction signature must be a canonical 65-byte" in str(
            exc
        )
    else:
        raise AssertionError("route canary with high-S signature was accepted")


def test_live_evidence_rejects_route_canary_wrong_signature_signer():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_signatures=[("12" * 64) + "00"],
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction signature does not recover" in str(exc)
    else:
        raise AssertionError("route canary with wrong signature signer was accepted")


def test_live_evidence_rejects_route_canary_wrong_trigger_contract():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_contract_override="41" + "55" * 20,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "contract_address does not match destination verifier" in str(exc)
    else:
        raise AssertionError("route canary with wrong trigger contract was accepted")


def test_live_evidence_rejects_uppercase_route_canary_trigger_calldata():
    module = load_live_module()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR,
        route_canary_transaction_call_data_override=(
            TRON_ROUTE_CANARY_CALL_DATA_VECTOR.upper()
        ),
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(
                    TRON_ROUTE_CANARY_TRANSACTION_ID_VECTOR
                ),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "route-canary transaction data must be canonical lowercase hex" in str(
            exc
        )
    else:
        raise AssertionError("uppercase route-canary trigger calldata was accepted")


def test_live_evidence_rejects_route_canary_wrong_submit_selector():
    module = load_live_module()
    wrong_call = b"\x00\x00\x00\x00" + bytes.fromhex(
        TRON_ROUTE_CANARY_CALL_DATA_VECTOR
    )[4:]
    raw_data_hex = tron_route_canary_raw_data_hex(call_data=wrong_call)
    transaction_id = hashlib.sha256(bytes.fromhex(raw_data_hex)).hexdigest()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=transaction_id,
        route_canary_transaction_call_data_override=wrong_call.hex(),
        route_canary_transaction_raw_data_hex=raw_data_hex,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(transaction_id),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "submitSccpMessageProof" in str(exc)
    else:
        raise AssertionError("route canary with wrong submit selector was accepted")


def test_live_evidence_rejects_route_canary_public_input_drift():
    module = load_live_module()
    wrong_call = tron_route_canary_submit_call_data(message_id=bytes.fromhex("de" * 32))
    raw_data_hex = tron_route_canary_raw_data_hex(call_data=wrong_call)
    transaction_id = hashlib.sha256(bytes.fromhex(raw_data_hex)).hexdigest()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=transaction_id,
        route_canary_transaction_call_data_override=wrong_call.hex(),
        route_canary_transaction_raw_data_hex=raw_data_hex,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(transaction_id),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "publicInputs[0] must match event messageId" in str(exc)
    else:
        raise AssertionError("route canary with public input drift was accepted")


def test_live_evidence_rejects_route_canary_proof_header_drift():
    module = load_live_module()
    wrong_call = bytearray(bytes.fromhex(TRON_ROUTE_CANARY_CALL_DATA_VECTOR))
    proof_start = 4 + 32 * 8 + 32
    proof_source_domain_offset = proof_start + 32 * 2
    wrong_call[proof_source_domain_offset : proof_source_domain_offset + 32] = (
        abi_word_u32(1)
    )
    raw_data_hex = tron_route_canary_raw_data_hex(call_data=bytes(wrong_call))
    transaction_id = hashlib.sha256(bytes.fromhex(raw_data_hex)).hexdigest()
    setup = route_canary_full_rollout_setup(
        module,
        route_canary_transaction_id=transaction_id,
        route_canary_transaction_call_data_override=wrong_call.hex(),
        route_canary_transaction_raw_data_hex=raw_data_hex,
    )

    try:
        module.collect_live_evidence(
            live_full_rollout_args(
                setup.fake,
                setup.expected,
                source_code_hash=setup.source_code_hash,
                route_canary_transaction_id=bytes.fromhex(transaction_id),
            ),
            opener=setup.fake.opener,
        )
    except RuntimeError as exc:
        assert "proof sourceDomain does not match" in str(exc)
    else:
        raise AssertionError("route canary with proof header drift was accepted")


def test_live_evidence_full_toml_requires_route_canary_evidence():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=False,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            source_trust_anchor_hash=expected.args.source_trust_anchor_hash,
            consensus_verifier_hash=expected.args.consensus_verifier_hash,
            message_inclusion_verifier_hash=(
                expected.args.message_inclusion_verifier_hash
            ),
            source_bridge_emitter_code_hash=source_code_hash,
            expected_source_bridge_config_hash=fake.source_config,
            finality_policy_hash=expected.args.finality_policy_hash,
            deployment_receipt_hash=expected.args.deployment_receipt_hash,
            adapter_verifier_vk_hash=None,
            expected_source_verifier_material_hash=expected.material_hash,
            expected_source_adapter_engine_deployment_hash=expected.deployment_hash,
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=expected.route_hash,
            route_canary_evidence_hash=None,
        ),
        opener=fake.opener,
    )

    assert "route_canary" not in summary
    assert "offline_full_toml_args" not in summary
    try:
        module.render_offline_full_toml(summary)
    except ValueError as exc:
        assert "route-canary-transaction-id" in str(exc)
    else:
        raise AssertionError("TRON full TOML rendered without route canary evidence")


def test_live_evidence_full_toml_requires_verified_route_canary_transaction():
    module = load_live_module()
    destination_runtime_bytecode = bytes.fromhex("6003600055")
    destination_code_hash = module.evidence.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    fake = fake_opener_for(
        module,
        destination_runtime_bytecode=destination_runtime_bytecode,
        destination_code_hash_override=destination_code_hash,
    )
    source_code_hash = module.evidence.runtime_bytecode_hash(fake.runtime_bytecode)
    expected = source_record_hashes_for(
        module,
        fake,
        source_code_hash=source_code_hash,
    )

    summary = module.collect_live_evidence(
        live_full_rollout_args(
            fake,
            expected,
            source_code_hash=source_code_hash,
            route_canary_evidence_hash=bytes.fromhex(
                TRON_ROUTE_CANARY_EVIDENCE_HASH
            ),
        ),
        opener=fake.opener,
    )

    assert summary["route_canary"]["evidence_hash"] == (
        "0x" + TRON_ROUTE_CANARY_EVIDENCE_HASH
    )
    assert "route_canary_transaction" not in summary
    assert "offline_full_toml_args" not in summary
    try:
        module.render_offline_full_toml(summary)
    except ValueError as exc:
        assert "route-canary-transaction-id" in str(exc)
    else:
        raise AssertionError("TRON full TOML rendered with only a manual canary hash")


def test_live_evidence_full_toml_requires_expected_source_config_pin():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=True,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            source_trust_anchor_hash=bytes.fromhex("44" * 32),
            consensus_verifier_hash=bytes.fromhex("55" * 32),
            message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
            source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
            finality_policy_hash=bytes.fromhex("88" * 32),
            deployment_receipt_hash=bytes.fromhex("aa" * 32),
            adapter_verifier_vk_hash=None,
            expected_source_verifier_material_hash=bytes.fromhex(
                TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
            ),
            expected_source_adapter_engine_deployment_hash=bytes.fromhex(
                TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
            ),
            expected_destination_binding_hash=fake.destination_binding,
            route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
        ),
        opener=fake.opener,
    )

    offline_args = summary["offline_evidence_args"]
    assert "--expected-config-hash" not in offline_args
    assert "--expected-destination-binding-hash" in offline_args
    assert "--route-allowlist-hash" in offline_args
    assert "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR in offline_args
    assert "offline_full_toml_args" not in summary
    assert "offline_full_toml_sha256" not in summary
    try:
        module.render_offline_full_toml(summary)
    except ValueError as exc:
        assert "expected-source-bridge-config-hash" in str(exc)
    else:
        raise AssertionError("full TOML rendered without a pinned source config hash")


def test_live_evidence_full_toml_requires_expected_destination_binding_pin():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=bytes.fromhex("44" * 32),
                consensus_verifier_hash=bytes.fromhex("55" * 32),
                message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
                source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
                expected_source_bridge_config_hash=fake.source_config,
                finality_policy_hash=bytes.fromhex("88" * 32),
                deployment_receipt_hash=bytes.fromhex("aa" * 32),
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=bytes.fromhex(
                    TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                ),
                expected_source_adapter_engine_deployment_hash=bytes.fromhex(
                    TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                ),
                expected_destination_binding_hash=None,
                route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "route-allowlist-hash requires --expected-destination-binding-hash" in str(exc)
    else:
        raise AssertionError(
            "route allowlist hash accepted an unpinned destination binding"
        )


def test_live_evidence_rejects_expected_source_record_hash_mismatch():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=bytes.fromhex("44" * 32),
                consensus_verifier_hash=bytes.fromhex("55" * 32),
                message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
                source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
                finality_policy_hash=bytes.fromhex("88" * 32),
                deployment_receipt_hash=bytes.fromhex("aa" * 32),
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=bytes.fromhex("99" * 32),
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-source-verifier-material-hash" in str(exc)
    else:
        raise AssertionError("mismatched live source material hash was accepted")


def test_live_evidence_rejects_expected_source_config_hash_mismatch():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                expected_source_bridge_config_hash=bytes.fromhex("99" * 32),
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "expected-source-bridge-config-hash" in str(exc)
    else:
        raise AssertionError("mismatched live source config hash was accepted")


def test_live_evidence_expected_source_config_pin_does_not_require_source_records():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=None,
            caller_address=None,
            no_getcontract=False,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            expected_source_bridge_config_hash=fake.source_config,
            source_trust_anchor_hash=None,
            consensus_verifier_hash=None,
            message_inclusion_verifier_hash=None,
            source_bridge_emitter_code_hash=None,
            finality_policy_hash=None,
            deployment_receipt_hash=None,
            adapter_verifier_vk_hash=None,
            expected_source_verifier_material_hash=None,
            expected_source_adapter_engine_deployment_hash=None,
            route_allowlist_hash=None,
        ),
        opener=fake.opener,
    )

    assert (
        summary["source_bridge"]["expected_source_bridge_config_hash_matches"]
        is True
    )
    assert "source_records" not in summary


def test_live_evidence_rejects_missing_source_bytecode_metadata():
    module = load_live_module()
    fake = fake_opener_for(module, source_runtime_bytecode=None)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                expected_source_bridge_config_hash=fake.source_config,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source bridge bytecode" in str(exc)
    else:
        raise AssertionError("missing source bridge bytecode was accepted")


def test_live_evidence_rejects_mismatched_source_metadata_address():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        source_metadata_address_override=module.tron_base58check_from_address20(
            bytes.fromhex("99" * 20)
        ),
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                expected_source_bridge_config_hash=fake.source_config,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "source bridge contract_address" in str(exc)
    else:
        raise AssertionError("mismatched source metadata address was accepted")


def test_live_evidence_rejects_malformed_source_metadata_address():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        source_metadata_address_override="not-a-tron-address",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                expected_source_bridge_config_hash=fake.source_config,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "malformed source bridge contract_address" in str(exc)
    else:
        raise AssertionError("malformed source metadata address was accepted")


def test_live_evidence_rejects_malformed_source_metadata_bytecode():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        source_runtime_bytecode="0xnot-runtime-bytecode",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                expected_source_bridge_config_hash=fake.source_config,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "malformed source bridge bytecode" in str(exc)
    else:
        raise AssertionError("malformed source metadata bytecode was accepted")


def test_live_evidence_rejects_padded_source_metadata_bytecode():
    module = load_live_module()
    fake = fake_opener_for(
        module,
        source_runtime_bytecode=" 0x6001600055",
    )

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                expected_source_bridge_config_hash=fake.source_config,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except RuntimeError as exc:
        assert "malformed source bridge bytecode" in str(exc)
    else:
        raise AssertionError("padded source metadata bytecode was accepted")


def test_live_evidence_rejects_source_code_hash_metadata_mismatch():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=bytes.fromhex("44" * 32),
                consensus_verifier_hash=bytes.fromhex("55" * 32),
                message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
                source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
                finality_policy_hash=bytes.fromhex("88" * 32),
                deployment_receipt_hash=bytes.fromhex("aa" * 32),
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "source-bridge-emitter-code-hash" in str(exc)
    else:
        raise AssertionError("source bytecode/hash mismatch was accepted")


def test_live_evidence_rejects_missing_source_bytecode_for_record_preflight():
    module = load_live_module()
    fake = fake_opener_for(module, source_runtime_bytecode=None)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=False,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=bytes.fromhex("44" * 32),
                consensus_verifier_hash=bytes.fromhex("55" * 32),
                message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
                source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
                finality_policy_hash=bytes.fromhex("88" * 32),
                deployment_receipt_hash=bytes.fromhex("aa" * 32),
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=None,
            ),
            opener=fake.opener,
        )
    except (RuntimeError, ValueError) as exc:
        assert "source bridge bytecode" in str(exc)
    else:
        raise AssertionError("missing source bridge bytecode was accepted")


def test_live_evidence_route_allowlist_hash_requires_destination():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=None,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=None,
                consensus_verifier_hash=None,
                message_inclusion_verifier_hash=None,
                source_bridge_emitter_code_hash=None,
                finality_policy_hash=None,
                deployment_receipt_hash=None,
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=None,
                expected_source_adapter_engine_deployment_hash=None,
                route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "route-allowlist-hash requires --destination-verifier-address" in str(exc)
    else:
        raise AssertionError("route allowlist hash accepted missing destination")


def test_live_evidence_route_allowlist_hash_requires_source_records():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "complete source record preflight arguments" in str(exc)
    else:
        raise AssertionError("route allowlist hash accepted missing source records")


def test_live_evidence_torii_destination_params_require_bytecode_metadata():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=None,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=True,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            expected_destination_binding_hash=fake.destination_binding,
        ),
        opener=fake.opener,
    )

    assert (
        summary["destination_verifier"]["expected_destination_binding_hash_matches"]
        is True
    )
    assert "torii_destination_query_params" not in summary
    assert "torii_destination_query_proof_bytes_hex_required" not in summary


def test_live_evidence_route_allowlist_hash_requires_expected_destination_binding_pin():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=bytes.fromhex("44" * 32),
                consensus_verifier_hash=bytes.fromhex("55" * 32),
                message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
                source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
                expected_source_bridge_config_hash=fake.source_config,
                finality_policy_hash=bytes.fromhex("88" * 32),
                deployment_receipt_hash=bytes.fromhex("aa" * 32),
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=bytes.fromhex(
                    TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                ),
                expected_source_adapter_engine_deployment_hash=bytes.fromhex(
                    TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                ),
                expected_destination_binding_hash=None,
                route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "route-allowlist-hash requires --expected-destination-binding-hash" in str(exc)
    else:
        raise AssertionError("route allowlist hash accepted an unpinned destination binding")


def test_live_evidence_rejects_route_allowlist_hash_drift():
    module = load_live_module()
    fake = fake_opener_for(module)

    try:
        module.collect_live_evidence(
            SimpleNamespace(
                tron_node_url="https://tron.example",
                source_bridge_address=fake.bridge,
                destination_verifier_address=fake.destination,
                caller_address=None,
                no_getcontract=True,
                timeout=1.0,
                tron_pro_api_key=None,
                tron_pro_api_key_file=None,
                solid=False,
                source_trust_anchor_hash=bytes.fromhex("44" * 32),
                consensus_verifier_hash=bytes.fromhex("55" * 32),
                message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
                source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
                expected_source_bridge_config_hash=fake.source_config,
                finality_policy_hash=bytes.fromhex("88" * 32),
                deployment_receipt_hash=bytes.fromhex("aa" * 32),
                adapter_verifier_vk_hash=None,
                expected_source_verifier_material_hash=bytes.fromhex(
                    TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                ),
                expected_source_adapter_engine_deployment_hash=bytes.fromhex(
                    TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                ),
                expected_destination_binding_hash=fake.destination_binding,
                route_allowlist_hash=bytes.fromhex("dd" * 32),
            ),
            opener=fake.opener,
        )
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("drifted route allowlist hash was accepted")


def test_live_evidence_full_toml_render_requires_complete_evidence():
    module = load_live_module()
    fake = fake_opener_for(module)

    summary = module.collect_live_evidence(
        SimpleNamespace(
            tron_node_url="https://tron.example",
            source_bridge_address=fake.bridge,
            destination_verifier_address=fake.destination,
            caller_address=None,
            no_getcontract=True,
            timeout=1.0,
            tron_pro_api_key=None,
            tron_pro_api_key_file=None,
            solid=False,
            source_trust_anchor_hash=None,
            consensus_verifier_hash=None,
            message_inclusion_verifier_hash=None,
            source_bridge_emitter_code_hash=None,
            finality_policy_hash=None,
            deployment_receipt_hash=None,
            adapter_verifier_vk_hash=None,
            expected_source_verifier_material_hash=None,
            expected_source_adapter_engine_deployment_hash=None,
            route_allowlist_hash=None,
        ),
        opener=fake.opener,
    )

    try:
        module.render_offline_full_toml(summary)
    except ValueError as exc:
        assert "full TOML output requires --expected-source-bridge-config-hash" in str(
            exc
        )
    else:
        raise AssertionError("full TOML rendered from incomplete live evidence")
