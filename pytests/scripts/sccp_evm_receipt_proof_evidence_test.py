import contextlib
import io
import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


def load_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_evm_receipt_proof_evidence.py"
    )
    spec = spec_from_file_location("sccp_evm_receipt_proof_evidence", script_path)
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

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


class FakeRawResponse:
    def __init__(self, payload):
        self.payload = payload.encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            return self.payload
        return self.payload[:size]


class SecretErrorBody:
    def read(self, size=-1):
        if size is None or size < 0:
            size = 4097
        return b"secret-token-receipt-rpc-error" * size

    def close(self):
        return None


def rpc_response(result):
    return {"jsonrpc": "2.0", "id": 1, "result": result}


def quantity(value):
    return "0x" + format(value, "x")


def hex_bytes(byte, count):
    return "0x" + f"{byte:02x}" * count


class HostileReceiptString(str):
    """String subclass that receipt RPC parsers must reject before hooks."""

    def __new__(cls, value):
        return str.__new__(cls, value)

    def __eq__(self, _other):
        raise AssertionError("secret-token receipt exact string compared")

    def __iter__(self):
        raise AssertionError("secret-token receipt exact string iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token receipt exact string indexed")

    def strip(self, *args, **kwargs):
        raise AssertionError("secret-token receipt exact string stripped")

    def startswith(self, _prefix):
        raise AssertionError("secret-token receipt exact string startswith ran")

    def lower(self):
        raise AssertionError("secret-token receipt exact string lower ran")

    def isascii(self):
        raise AssertionError("secret-token receipt exact string isascii ran")

    def isdecimal(self):
        raise AssertionError("secret-token receipt exact string isdecimal ran")


class HostileReceiptBytes(bytes):
    """Bytes subclass that receipt proof helpers must reject before hooks."""

    def __new__(cls, value):
        return bytes.__new__(cls, value)

    def __bytes__(self):
        raise AssertionError("secret-token receipt bytes coerced")

    def __repr__(self):
        raise AssertionError("secret-token receipt bytes repr'd")

    def __len__(self):
        raise AssertionError("secret-token receipt bytes length read")

    def __iter__(self):
        raise AssertionError("secret-token receipt bytes iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token receipt bytes indexed")


class HostileReceiptBytearray(bytearray):
    """Bytearray subclass that receipt proof helpers must reject before hooks."""

    def __init__(self, value):
        super().__init__(value)

    def __bytes__(self):
        raise AssertionError("secret-token receipt bytearray coerced")

    def __repr__(self):
        raise AssertionError("secret-token receipt bytearray repr'd")

    def __len__(self):
        raise AssertionError("secret-token receipt bytearray length read")

    def __iter__(self):
        raise AssertionError("secret-token receipt bytearray iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token receipt bytearray indexed")


class HostileReceiptDict(dict):
    """Dict subclass that receipt proof helpers must reject before hooks."""

    def get(self, _key, _default=None):
        raise AssertionError("secret-token receipt dict get ran")

    def __contains__(self, _key):
        raise AssertionError("secret-token receipt dict contains ran")

    def __iter__(self):
        raise AssertionError("secret-token receipt dict iterated")

    def __repr__(self):
        raise AssertionError("secret-token receipt dict repr'd")


class HostileReceiptList(list):
    """List subclass that receipt proof helpers must reject before hooks."""

    def __bool__(self):
        raise AssertionError("secret-token receipt list bool ran")

    def __len__(self):
        raise AssertionError("secret-token receipt list length read")

    def __iter__(self):
        raise AssertionError("secret-token receipt list iterated")

    def __getitem__(self, _key):
        raise AssertionError("secret-token receipt list indexed")

    def __repr__(self):
        raise AssertionError("secret-token receipt list repr'd")


def test_receipt_cli_redacts_top_level_exception_details(monkeypatch, capsys):
    module = load_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        OSError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):

        def fail_collect(*_args, exception_type=exception_type, **_kwargs):
            raise exception_type("secret-token /tmp/operator/private-path")

        with monkeypatch.context() as patch:
            patch.setattr(module, "collect_receipt_proof_evidence", fail_collect)
            try:
                module.main(
                    [
                        "--rpc-url",
                        "https://evm.example.invalid",
                        "--domain",
                        "eth",
                        "--transaction-hash",
                        "0x" + "11" * 32,
                        "--source-bridge-address",
                        "0x" + "33" * 20,
                    ]
                )
            except SystemExit as exc:
                assert exc.code == 2
            else:
                raise AssertionError(
                    "receipt proof CLI accepted top-level collection failure"
                )

            captured = capsys.readouterr()
            assert "SCCP EVM receipt proof evidence collection failed" in captured.err
            assert "secret-token" not in captured.err
            assert "private-path" not in captured.err
            assert exception_type.__name__ not in captured.err


def test_receipt_hex_parser_redacts_parser_causes():
    """Invalid EVM receipt hex inputs must not chain parser payloads."""

    module = load_module()
    payload = "secret-token-evm-receipt-hex"

    try:
        module.parse_hex_bytes(
            "0x" + payload + ("a" * (64 - len(payload))),
            label="transaction hash",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        rendered = str(exc)
        assert rendered == "transaction hash must be hex"
        assert "secret-token" not in rendered
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid EVM receipt transaction hash hex was accepted")


def test_receipt_cli_parsers_reject_non_string_values_without_stringification():
    """Receipt CLI parsers must reject hostile non-string values before coercion."""

    module = load_module()

    class HostileParserValue:
        def __str__(self):
            raise AssertionError("secret-token-receipt-parser-value was stringified")

        def __repr__(self):
            raise AssertionError("secret-token-receipt-parser-value was repr'd")

        def strip(self):
            raise AssertionError("secret-token-receipt-parser-value strip ran")

        def startswith(self, _prefix):
            raise AssertionError("secret-token-receipt-parser-value startswith ran")

        def lower(self):
            raise AssertionError("secret-token-receipt-parser-value lower ran")

        def isascii(self):
            raise AssertionError("secret-token-receipt-parser-value isascii ran")

        def isdecimal(self):
            raise AssertionError("secret-token-receipt-parser-value isdecimal ran")

    cases = (
        (
            lambda value: module.parse_hex_bytes(
                value,
                label="transaction hash",
                byte_length=32,
            ),
            "transaction hash must be canonical lowercase 0x hex",
        ),
        (
            module.parse_domain,
            "domain must be eth or bsc",
        ),
        (
            module.parse_rpc_chain_id,
            "--expected-rpc-chain-id must be a canonical decimal integer",
        ),
    )

    for parser, expected_message in cases:
        try:
            parser(HostileParserValue())
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
            assert "HostileParserValue" not in rendered
        else:
            raise AssertionError("hostile receipt parser value was accepted")


def test_receipt_rpc_scalar_parsers_reject_string_subclasses_without_hooks():
    module = load_module()
    hostile_quantity = HostileReceiptString("0x1")
    hostile_hex = HostileReceiptString("0x11")

    cases = (
        (
            lambda: module._rpc_quantity(hostile_quantity, method="receipt.status"),
            "receipt.status returned non-canonical quantity",
        ),
        (
            lambda: module._rpc_hex_data(hostile_hex, method="eth_getBlockReceipts"),
            "eth_getBlockReceipts returned non-canonical lowercase 0x hex data",
        ),
        (
            lambda: module._rpc_exact_string_literal(
                hostile_quantity,
                "0x1",
                message="receipt.status must be 0x1",
            ),
            "receipt.status must be 0x1",
        ),
    )

    for parser, expected_message in cases:
        try:
            parser()
        except RuntimeError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
        else:
            raise AssertionError("string-subclass receipt RPC scalar was accepted")


def test_receipt_sequence_parsers_reject_string_subclasses_without_iteration():
    module = load_module()
    hostile_sequence = HostileReceiptString("[]")
    valid_receipts = block_receipts(module)
    transaction_receipt = valid_receipts[0]
    proof = module.build_receipt_trie_proof_from_receipts(
        valid_receipts,
        transaction_index=0,
    )
    receipts_root = "0x" + proof["receipts_root"].hex()

    def hostile_block_receipts_opener(request, timeout=15.0):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        params = payload["params"]
        if method == "eth_chainId":
            return FakeResponse(rpc_response(quantity(1)))
        if method == "eth_getTransactionReceipt":
            assert params == ["0x" + "11" * 32]
            return FakeResponse(rpc_response(transaction_receipt))
        if method == "eth_getBlockByHash":
            assert params == ["0x" + "aa" * 32, False]
            return FakeResponse(
                rpc_response(
                    {
                        "hash": "0x" + "aa" * 32,
                        "number": "0x1234",
                        "receiptsRoot": receipts_root,
                    }
                )
            )
        if method == "eth_getBlockReceipts":
            assert params == ["0x1234"]
            return FakeResponse(rpc_response(hostile_sequence))
        raise AssertionError(f"unexpected RPC method {method}")

    sequence_cases = (
        (
            lambda: module.rlp_encode(hostile_sequence),
            TypeError,
            "RLP value must be bytes or a sequence",
        ),
        (
            lambda: module._receipt_logs({"logs": hostile_sequence}),
            RuntimeError,
            "receipt.logs must be a list",
        ),
        (
            lambda: module._receipt_logs(
                {
                    "logs": [
                        {
                            "address": "0x" + "33" * 20,
                            "topics": hostile_sequence,
                            "data": "0x",
                        }
                    ]
                }
            ),
            RuntimeError,
            "receipt.logs[0].topics must be a list",
        ),
        (
            lambda: module._source_event_digest_from_receipt(
                {"logs": hostile_sequence},
                source_bridge_address=bytes.fromhex("33" * 20),
                transaction_hash=bytes.fromhex("11" * 32),
                block_hash=bytes.fromhex("aa" * 32),
                block_number=0x1234,
            ),
            RuntimeError,
            "receipt.logs is required for SCCP source event validation",
        ),
        (
            lambda: module._source_event_digest_from_receipt(
                receipt(
                    module,
                    index=0,
                    tx_byte=0x11,
                    logs=source_log(module, topics=hostile_sequence),
                ),
                source_bridge_address=bytes.fromhex("33" * 20),
                transaction_hash=bytes.fromhex("11" * 32),
                block_hash=bytes.fromhex("aa" * 32),
                block_number=0x1234,
            ),
            RuntimeError,
            "receipt.logs[0].topics must be a list",
        ),
        (
            lambda: module.collect_receipt_proof_evidence(
                "https://rpc.example",
                domain=module.SCCP_DOMAIN_ETH,
                transaction_hash=bytes.fromhex("11" * 32),
                source_bridge_address=bytes.fromhex("33" * 20),
                opener=hostile_block_receipts_opener,
            ),
            RuntimeError,
            "eth_getBlockReceipts returned a non-list response",
        ),
    )

    for parser, exception_type, expected_message in sequence_cases:
        try:
            parser()
        except exception_type as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
        else:
            raise AssertionError("string-subclass receipt sequence was accepted")


def test_receipt_container_and_bytes_boundaries_reject_subclasses_without_hooks(
    monkeypatch,
):
    module = load_module()
    transaction_hash = bytes.fromhex("11" * 32)
    source_bridge_address = bytes.fromhex("33" * 20)
    valid_receipts = block_receipts(module)

    assert module.rlp_encode(bytearray(b"\x01")) == b"\x01"
    assert (
        module._require_direct_bytes_arg(
            bytearray(transaction_hash),
            label="transaction_hash",
            byte_length=32,
        )
        == transaction_hash
    )

    def fixed_opener(_request, timeout=15.0):
        del timeout
        return FakeRawResponse("{}")

    with monkeypatch.context() as patch:
        patch.setattr(
            module.json,
            "loads",
            lambda *_args, **_kwargs: HostileReceiptDict(
                {"jsonrpc": "2.0", "id": 1, "result": "0x1"}
            ),
        )
        try:
            module._json_rpc(
                "https://rpc.example",
                "eth_chainId",
                [],
                opener=fixed_opener,
                timeout=15.0,
            )
        except RuntimeError as exc:
            rendered = str(exc)
            assert rendered == "JSON-RPC eth_chainId returned a non-object response"
            assert "secret-token" not in rendered
        else:
            raise AssertionError("receipt JSON-RPC accepted hostile envelope root")

    cases = (
        (
            lambda: module.rlp_encode(HostileReceiptBytes(b"\x01")),
            TypeError,
            "RLP value must be bytes or a sequence",
        ),
        (
            lambda: module.rlp_encode(HostileReceiptBytearray(b"\x01")),
            TypeError,
            "RLP value must be bytes or a sequence",
        ),
        (
            lambda: module._require_direct_bytes_arg(
                HostileReceiptBytes(transaction_hash),
                label="transaction_hash",
                byte_length=32,
            ),
            ValueError,
            "transaction_hash must be bytes",
        ),
        (
            lambda: module._require_direct_bytes_arg(
                HostileReceiptBytearray(transaction_hash),
                label="transaction_hash",
                byte_length=32,
            ),
            ValueError,
            "transaction_hash must be bytes",
        ),
        (
            lambda: module._receipt_logs(
                {"logs": HostileReceiptList(source_log(module))}
            ),
            RuntimeError,
            "receipt.logs must be a list",
        ),
        (
            lambda: module._receipt_logs({"logs": [HostileReceiptDict({})]}),
            RuntimeError,
            "receipt.logs[0] must be an object",
        ),
        (
            lambda: module._source_event_digest_from_receipt(
                {"logs": HostileReceiptList(source_log(module))},
                source_bridge_address=source_bridge_address,
                transaction_hash=transaction_hash,
                block_hash=bytes.fromhex("aa" * 32),
                block_number=0x1234,
            ),
            RuntimeError,
            "receipt.logs is required for SCCP source event validation",
        ),
        (
            lambda: module._source_event_digest_from_receipt(
                {"logs": [HostileReceiptDict({})]},
                source_bridge_address=source_bridge_address,
                transaction_hash=transaction_hash,
                block_hash=bytes.fromhex("aa" * 32),
                block_number=0x1234,
            ),
            RuntimeError,
            "receipt.logs[0] must be an object",
        ),
        (
            lambda: module.build_receipt_trie_proof_from_receipts(
                HostileReceiptList(valid_receipts),
                transaction_index=0,
            ),
            ValueError,
            "block receipts must contain "
            f"1..{module.EVM_RECEIPT_PROOF_MAX_BLOCK_RECEIPTS} entries",
        ),
        (
            lambda: module.build_receipt_trie_proof_from_receipts(
                [HostileReceiptDict(valid_receipts[0])],
                transaction_index=0,
            ),
            TypeError,
            "block receipts[0] must be an object",
        ),
    )

    for call, exception_type, expected_message in cases:
        try:
            call()
        except exception_type as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError("receipt proof accepted hostile container or bytes")


def test_receipt_domain_parser_rejects_retired_aliases_without_stringifying():
    """Receipt domain parsing must stay exact for first-release source lanes."""

    module = load_module()

    class HostileDomainText(str):
        def __new__(cls):
            return str.__new__(cls, "eth")

        def __str__(self):
            raise AssertionError("secret-token receipt domain was stringified")

        def __repr__(self):
            raise AssertionError("secret-token receipt domain was repr'd")

        def __eq__(self, _other):
            raise AssertionError("secret-token receipt domain was compared")

        def __ne__(self, _other):
            raise AssertionError("secret-token receipt domain was compared")

        def strip(self):
            raise AssertionError("secret-token receipt domain was stripped")

        def lower(self):
            raise AssertionError("secret-token receipt domain was lowered")

        def isascii(self):
            raise AssertionError("secret-token receipt domain was inspected")

        def isdecimal(self):
            raise AssertionError("secret-token receipt domain was inspected")

    assert module.parse_domain("eth") == module.SCCP_DOMAIN_ETH
    assert module.parse_domain("bsc") == module.SCCP_DOMAIN_BSC

    for value in (
        "1",
        "2",
        "ethereum",
        "bnb",
        "ETH",
        "BSC",
        "01",
        "0x1",
        "+1",
        " eth ",
        "١",
        "secret-token-receipt-domain",
        HostileDomainText(),
    ):
        try:
            module.parse_domain(value)
        except module.argparse.ArgumentTypeError as exc:
            rendered = str(exc)
            assert rendered == "domain must be eth or bsc"
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
            assert exc.__suppress_context__ is True
        else:
            raise AssertionError("noncanonical receipt domain was accepted")


def test_receipt_hex_nonzero_controls_reject_non_booleans():
    """Receipt fixed-hex nonzero policy must not accept truthy aliases."""

    module = load_module()
    malformed_values = (1, "true", None)

    for nonzero in malformed_values:
        try:
            module.parse_hex_bytes(
                "0x" + "00" * 32,
                label="transaction hash",
                byte_length=32,
                nonzero=nonzero,
            )
        except ValueError as exc:
            assert str(exc) == "parse_hex_bytes nonzero must be a boolean"
        else:
            raise AssertionError("malformed receipt hex nonzero control was accepted")

        try:
            module._rpc_fixed_hex_data(
                "0x" + "00" * 32,
                method="eth_getProof",
                byte_length=32,
                nonzero=nonzero,
            )
        except ValueError as exc:
            assert str(exc) == "RPC fixed hex nonzero must be a boolean"
        else:
            raise AssertionError(
                "malformed receipt RPC fixed-hex nonzero control was accepted"
            )


def test_receipt_compact_path_leaf_control_rejects_non_booleans():
    """Receipt trie path type selection must not accept truthy aliases."""

    module = load_module()

    for leaf in (1, "true", None):
        try:
            module._encode_compact_path((1, 2, 3), leaf=leaf)
        except ValueError as exc:
            assert str(exc) == "compact trie path leaf must be a boolean"
        else:
            raise AssertionError("malformed receipt trie path leaf control was accepted")


def test_receipt_hex_parser_redacts_helper_exit_parser_causes(monkeypatch):
    """Parser helper exits must collapse to the same fixed public hex category."""

    module = load_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):
        detail = (
            "secret-token EVM receipt hex TypeError detail"
            if exception_type is TypeError
            else f"secret-token EVM receipt hex {exception_type.__name__} detail"
        )

        class SecretBytes:
            @staticmethod
            def fromhex(_text, detail=detail, exception_type=exception_type):
                raise exception_type(detail)

        with monkeypatch.context() as patch:
            patch.setattr(module, "bytes", SecretBytes, raising=False)

            try:
                module.parse_hex_bytes(
                    "0x" + "11" * 32,
                    label="transaction hash",
                    byte_length=32,
                )
            except module.argparse.ArgumentTypeError as exc:
                rendered = str(exc)
                assert rendered == "transaction hash must be hex"
                assert "secret-token" not in rendered
                assert exception_type.__name__ not in rendered
                if exception_type is module.argparse.ArgumentTypeError:
                    assert (
                        "ArgumentTypeError" not in rendered
                    ), "EVM receipt hex ArgumentTypeError detail leaked"
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    f"EVM receipt parser {exception_type.__name__} was accepted"
                )


def test_receipt_rpc_hex_data_redacts_helper_exit_parser_causes(monkeypatch):
    """RPC hex helper exits must collapse to the fixed public RPC hex category."""

    module = load_module()

    for exception_type in (
        module.argparse.ArgumentTypeError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ):

        class SecretBytes:
            @staticmethod
            def fromhex(_text, exception_type=exception_type):
                raise exception_type(
                    "secret-token EVM receipt RPC hex "
                    f"{exception_type.__name__} detail"
                )

        with monkeypatch.context() as patch:
            patch.setattr(module, "bytes", SecretBytes, raising=False)

            try:
                module._rpc_hex_data("0x11", method="eth_getBlockReceipts")
            except RuntimeError as exc:
                rendered = str(exc)
                assert (
                    rendered
                    == "eth_getBlockReceipts returned non-canonical lowercase 0x hex data"
                )
                assert "secret-token" not in rendered
                assert exception_type.__name__ not in rendered
                if exception_type is module.argparse.ArgumentTypeError:
                    assert (
                        "ArgumentTypeError" not in rendered
                    ), "EVM receipt RPC hex ArgumentTypeError detail leaked"
                assert exc.__cause__ is None
                assert exc.__suppress_context__ is True
            else:
                raise AssertionError(
                    f"EVM receipt RPC hex {exception_type.__name__} was accepted"
                )


def source_log(module, *, duplicate=False, **overrides):
    log = {
        "address": "0x" + "33" * 20,
        "topics": [module.EVM_SOURCE_EVENT_TOPIC, "0x" + "55" * 32],
        "data": "0x",
        "transactionHash": "0x" + "11" * 32,
        "blockHash": "0x" + "aa" * 32,
        "blockNumber": "0x1234",
        **overrides,
    }
    return [log, dict(log)] if duplicate else [log]


def receipt(
    module,
    *,
    index=0,
    tx_byte=0x11,
    status="0x1",
    logs=None,
    receipt_type="0x2",
    transaction_index_override=None,
):
    return {
        "type": receipt_type,
        "transactionHash": hex_bytes(tx_byte, 32),
        "transactionIndex": quantity(
            index if transaction_index_override is None else transaction_index_override
        ),
        "blockHash": "0x" + "aa" * 32,
        "blockNumber": "0x1234",
        "status": status,
        "cumulativeGasUsed": quantity(21_000 * (index + 1)),
        "logsBloom": "0x" + "00" * 256,
        "logs": [] if logs is None else logs,
    }


def block_receipts(module, **overrides):
    first_logs = source_log(
        module,
        duplicate=overrides.get("duplicate_source_log", False),
        **overrides.get("source_log_overrides", {}),
    )
    first = receipt(
        module,
        index=0,
        tx_byte=0x11,
        status=overrides.get("status", "0x1"),
        logs=first_logs,
        transaction_index_override=overrides.get("transaction_index_override"),
    )
    second = receipt(module, index=1, tx_byte=0x22)
    return [first, second]


def fake_opener_for(
    module,
    *,
    chain_id=1,
    receipts=None,
    block_receipts_root=None,
    transaction_receipt=None,
):
    receipts = block_receipts(module) if receipts is None else receipts
    transaction_receipt = receipts[0] if transaction_receipt is None else transaction_receipt
    proof = module.build_receipt_trie_proof_from_receipts(
        receipts,
        transaction_index=0,
    )
    receipts_root = (
        "0x" + proof["receipts_root"].hex()
        if block_receipts_root is None
        else block_receipts_root
    )
    calls = []

    def opener(request, timeout=15.0):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        params = payload["params"]
        calls.append((method, params))
        if method == "eth_chainId":
            return FakeResponse(rpc_response(quantity(chain_id)))
        if method == "eth_getTransactionReceipt":
            assert params == ["0x" + "11" * 32]
            return FakeResponse(rpc_response(transaction_receipt))
        if method == "eth_getBlockByHash":
            assert params == ["0x" + "aa" * 32, False]
            return FakeResponse(
                rpc_response(
                    {
                        "hash": "0x" + "aa" * 32,
                        "number": "0x1234",
                        "receiptsRoot": receipts_root,
                    }
                )
            )
        if method == "eth_getBlockReceipts":
            assert params == ["0x1234"]
            return FakeResponse(rpc_response(receipts))
        raise AssertionError(f"unexpected method {method}")

    opener.calls = calls
    return opener


def test_collect_receipt_proof_evidence_builds_verified_source_event_proof():
    module = load_module()
    opener = fake_opener_for(module)

    summary = module.collect_receipt_proof_evidence(
        "https://rpc.example",
        domain=module.SCCP_DOMAIN_ETH,
        transaction_hash=bytes.fromhex("11" * 32),
        source_bridge_address=bytes.fromhex("33" * 20),
        opener=opener,
    )

    assert summary["read_only"] is True
    assert summary["evidence_mode"] == "sccp_source_event"
    assert summary["chain"] == "eth"
    assert summary["rpc_chain_id"] == 1
    assert summary["transaction_hash"] == "0x" + "11" * 32
    assert summary["transaction_index"] == 0
    assert summary["receipt_status"] == "0x1"
    assert summary["receipt_root_verified"] is True
    assert summary["source_event_validated"] is True
    assert summary["execution_receipts_root"] == summary["computed_receipts_root"]
    assert summary["source_event_digest"] == "0x" + "55" * 32
    assert "receipt_only_evidence" not in summary
    assert summary["receipt_rlp"].startswith("0x02")
    assert summary["receipt_trie_key"] == "0x80"
    assert len(summary["receipt_trie_proof_nodes"]) >= 1
    assert [call[0] for call in opener.calls] == [
        "eth_chainId",
        "eth_getTransactionReceipt",
        "eth_getBlockByHash",
        "eth_getBlockReceipts",
    ]


def test_collect_receipt_proof_requires_source_bridge_without_receipt_only_bypass():
    module = load_module()
    opener = fake_opener_for(module)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            opener=opener,
        )
    except ValueError as exc:
        assert str(exc) == "source_bridge_address is required for SCCP source-event evidence"
    else:
        raise AssertionError("receipt evidence was accepted without source bridge validation")
    assert opener.calls == []


def test_collect_receipt_proof_rejects_removed_receipt_only_keyword_before_rpc():
    module = load_module()

    def opener(_request, timeout=15.0):
        del timeout
        raise AssertionError("removed receipt-only option reached RPC")

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            allow_receipt_only_evidence=True,
            opener=opener,
        )
    except TypeError as exc:
        assert "allow_receipt_only_evidence" in str(exc)
        assert "unexpected keyword argument" in str(exc)
    else:
        raise AssertionError("removed receipt-only keyword was accepted")


def test_cli_requires_source_bridge_without_receipt_only_bypass():
    module = load_module()
    stderr = io.StringIO()

    with contextlib.redirect_stderr(stderr):
        try:
            module.main(
                [
                    "--rpc-url",
                    "https://rpc.example",
                    "--domain",
                    "eth",
                    "--expected-rpc-chain-id",
                    "1",
                    "--transaction-hash",
                    "0x" + "11" * 32,
                ]
            )
        except SystemExit as exc:
            assert exc.code == 2
        else:
            raise AssertionError("CLI accepted source-event evidence without a bridge")

    assert "--source-bridge-address" in stderr.getvalue()
    assert "--allow-receipt-only-evidence" not in stderr.getvalue()


def test_cli_rejects_removed_receipt_only_mode_before_collection(monkeypatch, capsys):
    module = load_module()

    def fail_collect(*_args, **_kwargs):
        raise AssertionError("removed receipt-only CLI flag reached collection")

    monkeypatch.setattr(module, "collect_receipt_proof_evidence", fail_collect)
    try:
        module.main(
            [
                "--rpc-url",
                "https://rpc.example",
                "--domain",
                "eth",
                "--transaction-hash",
                "0x" + "11" * 32,
                "--source-bridge-address",
                "0x" + "33" * 20,
                "--allow-receipt-only-evidence",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("removed receipt-only CLI flag was accepted")

    captured = capsys.readouterr()
    assert "unrecognized arguments: --allow-receipt-only-evidence" in captured.err


def test_receipt_rlp_rejects_unknown_typed_receipt_prefix():
    module = load_module()
    unknown_typed_receipt = receipt(
        module,
        receipt_type="0x7f",
        logs=source_log(module),
    )

    try:
        module.canonical_receipt_rlp(unknown_typed_receipt)
    except RuntimeError as exc:
        assert "typed receipt type is not supported" in str(exc)
    else:
        raise AssertionError("unknown EIP-2718 receipt type was accepted")


def test_collect_receipt_proof_accepts_zero_log_topic_in_receipt_rlp():
    module = load_module()
    receipts = block_receipts(module)
    receipts[1] = receipt(
        module,
        index=1,
        tx_byte=0x22,
        logs=[
            {
                "address": "0x" + "12" * 20,
                "topics": ["0x" + "00" * 32],
                "data": "0x",
            }
        ],
    )
    opener = fake_opener_for(module, receipts=receipts)

    summary = module.collect_receipt_proof_evidence(
        "https://rpc.example",
        domain=module.SCCP_DOMAIN_ETH,
        transaction_hash=bytes.fromhex("11" * 32),
        source_bridge_address=bytes.fromhex("33" * 20),
        opener=opener,
    )

    assert summary["receipt_root_verified"] is True
    assert summary["receipt_rlp"].startswith("0x02")


def test_collect_receipt_proof_accepts_zero_log_address_in_receipt_rlp():
    module = load_module()
    receipts = block_receipts(module)
    receipts[1] = receipt(
        module,
        index=1,
        tx_byte=0x22,
        logs=[
            {
                "address": "0x" + "00" * 20,
                "topics": ["0x" + "44" * 32],
                "data": "0x",
            }
        ],
    )
    opener = fake_opener_for(module, receipts=receipts)

    summary = module.collect_receipt_proof_evidence(
        "https://rpc.example",
        domain=module.SCCP_DOMAIN_ETH,
        transaction_hash=bytes.fromhex("11" * 32),
        source_bridge_address=bytes.fromhex("33" * 20),
        opener=opener,
    )

    assert summary["receipt_root_verified"] is True
    assert summary["receipt_rlp"].startswith("0x02")


def test_collect_receipt_proof_rejects_non_mainnet_chain_id():
    module = load_module()
    opener = fake_opener_for(module, chain_id=56)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except ValueError as exc:
        assert "eth_chainId for eth lane" in str(exc)
    else:
        raise AssertionError("non-mainnet ETH chain id was accepted")
    assert [call[0] for call in opener.calls] == ["eth_chainId"]


def test_collect_receipt_proof_rejects_boolean_domain_and_expected_chain_id_before_rpc():
    module = load_module()

    def opener(_request, timeout=15.0):
        del timeout
        raise AssertionError("boolean receipt proof metadata reached RPC")

    for kwargs, expected_message in (
        (
            {
                "domain": True,
                "expected_rpc_chain_id": None,
            },
            "domain must be an EVM-family source lane",
        ),
        (
            {
                "domain": module.SCCP_DOMAIN_ETH,
                "expected_rpc_chain_id": True,
            },
            "expected RPC chain id must be an exact integer",
        ),
    ):
        try:
            module.collect_receipt_proof_evidence(
                "https://rpc.example",
                transaction_hash=bytes.fromhex("11" * 32),
                source_bridge_address=bytes.fromhex("33" * 20),
                opener=opener,
                **kwargs,
            )
        except ValueError as exc:
            assert str(exc) == expected_message
        else:
            raise AssertionError("boolean receipt proof domain metadata was accepted")


def test_collect_receipt_proof_rejects_direct_namespace_args_before_rpc():
    module = load_module()

    class HostileImportedScalar:
        def __str__(self):
            raise AssertionError("secret-token receipt namespace scalar was stringified")

        def __repr__(self):
            raise AssertionError("secret-token receipt namespace scalar was repr'd")

    def opener(_request, timeout=15.0):
        del timeout
        raise AssertionError("receipt proof namespace validation reached RPC")

    cases = (
        (
            {"transaction_hash": HostileImportedScalar()},
            "transaction_hash must be bytes",
        ),
        (
            {"transaction_hash": b"\x11" * 31},
            "transaction_hash must be 32 bytes",
        ),
        (
            {"transaction_hash": b"\x00" * 32},
            "transaction_hash must not be zero",
        ),
        (
            {"source_bridge_address": HostileImportedScalar()},
            "source_bridge_address must be bytes",
        ),
        (
            {"source_bridge_address": b"\x22" * 19},
            "source_bridge_address must be 20 bytes",
        ),
        (
            {"source_bridge_address": b"\x00" * 20},
            "source_bridge_address must not be zero",
        ),
        (
            {"expected_rpc_chain_id": 0},
            "--expected-rpc-chain-id must be a positive u64 integer",
        ),
        (
            {"expected_rpc_chain_id": 2**64},
            "--expected-rpc-chain-id must be a positive u64 integer",
        ),
        (
            {"expected_rpc_chain_id": module.EXPECTED_RPC_CHAIN_IDS[module.SCCP_DOMAIN_BSC]},
            "--expected-rpc-chain-id must match the canonical eth mainnet chain id 1",
        ),
    )
    for overrides, expected_message in cases:
        kwargs = {
            "domain": module.SCCP_DOMAIN_ETH,
            "transaction_hash": bytes.fromhex("11" * 32),
            "source_bridge_address": bytes.fromhex("22" * 20),
            "opener": opener,
        }
        kwargs.update(overrides)
        try:
            module.collect_receipt_proof_evidence(
                "https://rpc.example",
                **kwargs,
            )
        except ValueError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
            assert exc.__cause__ is None
        else:
            raise AssertionError("hostile receipt proof namespace value was accepted")


def test_collect_receipt_proof_rejects_noncanonical_chain_id_quantity():
    module = load_module()

    for chain_id_result in ("0x01", "0X1", " 0x1", "0x1 ", 1):
        calls = []

        def opener(request, timeout=15.0):
            del timeout
            payload = json.loads(request.data.decode("utf-8"))
            calls.append(payload["method"])
            return FakeResponse(rpc_response(chain_id_result))

        try:
            module.collect_receipt_proof_evidence(
                "https://rpc.example",
                domain=module.SCCP_DOMAIN_ETH,
                transaction_hash=bytes.fromhex("11" * 32),
                source_bridge_address=bytes.fromhex("33" * 20),
                opener=opener,
            )
        except RuntimeError as exc:
            assert "eth_chainId returned non-canonical quantity" in str(exc)
        else:
            raise AssertionError(
                f"noncanonical eth_chainId quantity {chain_id_result!r} was accepted"
            )
        assert calls == ["eth_chainId"], chain_id_result


def test_collect_receipt_proof_rejects_duplicate_json_rpc_result_keys():
    module = load_module()
    calls = []

    def opener(request, timeout=15.0):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        calls.append(payload["method"])
        return FakeRawResponse(
            '{"jsonrpc":"2.0","id":1,'
            '"secret-token-result":"0x1","secret-token-result":"0x2",'
            '"result":"0x3"}'
        )

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC eth_chainId returned duplicate JSON keys"
        assert "secret-token" not in message
        assert "duplicate JSON key " not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("duplicate JSON-RPC result keys were accepted")
    assert calls == ["eth_chainId"]


def test_collect_receipt_proof_rejects_duplicate_json_receipt_fields():
    module = load_module()
    calls = []
    transaction_receipt = json.dumps(
        receipt(module, index=0, tx_byte=0x11, logs=source_log(module)),
        separators=(",", ":"),
    )
    duplicated_receipt = (
        transaction_receipt[:-1] + ',"transactionHash":"0x' + "22" * 32 + '"}'
    )

    def opener(request, timeout=15.0):
        del timeout
        payload = json.loads(request.data.decode("utf-8"))
        method = payload["method"]
        calls.append(method)
        if method == "eth_chainId":
            return FakeResponse(rpc_response("0x1"))
        if method == "eth_getTransactionReceipt":
            return FakeRawResponse(f'{{"jsonrpc":"2.0","id":1,"result":{duplicated_receipt}}}')
        raise AssertionError(f"unexpected method {method}")

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC eth_getTransactionReceipt returned duplicate JSON keys"
        assert "transactionHash" not in message
        assert "duplicate JSON key " not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("duplicate JSON receipt fields were accepted")
    assert calls == ["eth_chainId", "eth_getTransactionReceipt"]


def test_receipt_json_object_rejects_key_subclasses_without_hooks():
    module = load_module()

    class HostileJsonKey(str):
        def __new__(cls):
            return str.__new__(cls, "secret-token-result")

        def __hash__(self):
            raise AssertionError("secret-token receipt JSON key was hashed")

        def __eq__(self, _other):
            raise AssertionError("secret-token receipt JSON key was compared")

        def __str__(self):
            raise AssertionError("secret-token receipt JSON key was stringified")

        def __repr__(self):
            raise AssertionError("secret-token receipt JSON key was repr'd")

    try:
        module._json_object_without_duplicate_keys([(HostileJsonKey(), "0x1")])
    except ValueError as exc:
        message = str(exc)
        assert message == "JSON-RPC returned duplicate JSON keys"
        assert "secret-token" not in message
    else:
        raise AssertionError("hostile receipt JSON key subclass was accepted")


def test_receipt_json_rpc_redacts_invalid_json_parser_details():
    module = load_module()

    def invalid_json_opener(_request, timeout=15.0):
        del timeout
        return FakeRawResponse('{"secret-token invalid EVM receipt JSON-RPC payload": ')

    try:
        module._json_rpc(
            "https://rpc.example",
            "eth_chainId",
            [],
            opener=invalid_json_opener,
            timeout=3.0,
        )
    except RuntimeError as exc:
        message = str(exc)
        assert message == "JSON-RPC eth_chainId returned invalid JSON"
        assert "secret-token" not in message
        assert "receipt JSON-RPC payload" not in message
        assert exc.__cause__ is None
        assert exc.__suppress_context__ is True
    else:
        raise AssertionError("invalid receipt JSON-RPC payload was accepted")


def test_receipt_json_rpc_url_rejects_hidden_request_state():
    module = load_module()

    class HostileReceiptRpcUrl(str):
        def __new__(cls):
            return str.__new__(cls, "https://rpc.example")

        def __str__(self):
            raise AssertionError("secret-token receipt RPC URL was stringified")

        def __repr__(self):
            raise AssertionError("secret-token receipt RPC URL was repr'd")

        def __eq__(self, _other):
            raise AssertionError("secret-token receipt RPC URL was compared")

        def __ne__(self, _other):
            raise AssertionError("secret-token receipt RPC URL was compared")

        def __iter__(self):
            raise AssertionError("secret-token receipt RPC URL was iterated")

        def strip(self, *_args):
            raise AssertionError("secret-token receipt RPC URL was stripped")

    class HostileReceiptRpcUrlLabel:
        def __str__(self):
            raise AssertionError("secret-token receipt RPC URL label was stringified")

        def __repr__(self):
            raise AssertionError("secret-token receipt RPC URL label was repr'd")

    class HostileReceiptRpcHost(str):
        def __new__(cls):
            return str.__new__(cls, "localhost")

        def __str__(self):
            raise AssertionError("secret-token receipt RPC host was stringified")

        def __repr__(self):
            raise AssertionError("secret-token receipt RPC host was repr'd")

        def strip(self, *_args):
            raise AssertionError("secret-token receipt RPC host was stripped")

        def lower(self):
            raise AssertionError("secret-token receipt RPC host was lowered")

    assert module._normalize_evm_rpc_url("https://rpc.example") == (
        "https://rpc.example"
    )
    assert module._normalize_evm_rpc_url("https://rpc.example/provider-token") == (
        "https://rpc.example/provider-token"
    )
    assert module._normalize_evm_rpc_url("http://127.0.0.1:8545") == (
        "http://127.0.0.1:8545"
    )
    hostile_host = HostileReceiptRpcHost()
    assert module._evm_rpc_host_is_loopback(hostile_host) is False
    assert module._evm_rpc_host_is_non_public_dns(hostile_host) is True

    def forbidden_opener(_request, timeout=15.0):
        raise AssertionError("malformed receipt RPC URL reached the opener")

    for rpc_url, expected_error in (
        ("https://token@rpc.example", "credentials"),
        ("https://rpc.example/root;param", "params, query, or fragment"),
        ("https://rpc.example?api_key=secret", "params, query, or fragment"),
        ("https://rpc.example#fragment", "params, query, or fragment"),
        ("http://rpc.example", "HTTPS unless it is loopback HTTP"),
        ("https://localhost", "public DNS"),
        ("https://127.0.0.1", "public DNS"),
        ("https://ethereum", "public DNS"),
        ("https://ethereum.local", "public DNS"),
        ("https://bad_host.rpc.example", "public DNS"),
        (" https://rpc.example", "exact http(s) URL"),
        ("https://rpc.example\nsecret", "exact http(s) URL"),
        (HostileReceiptRpcUrl(), "exact http(s) URL"),
        (HostileReceiptRpcUrlLabel(), "exact http(s) URL"),
    ):
        try:
            module._json_rpc(
                rpc_url,
                "eth_chainId",
                [],
                opener=forbidden_opener,
                timeout=3.0,
            )
        except ValueError as exc:
            assert expected_error in str(exc)
        else:
            raise AssertionError(
                f"hidden receipt RPC URL state {rpc_url!r} was accepted"
            )


def test_receipt_json_rpc_redacts_transport_and_error_response_details():
    module = load_module()

    def secret_http_error_opener(request, timeout=15.0):
        del timeout
        raise module.urllib.error.HTTPError(
            request.full_url,
            503,
            "secret-token gateway",
            {},
            SecretErrorBody(),
        )

    def secret_url_error_opener(_request, timeout=15.0):
        del timeout
        raise module.urllib.error.URLError(
            "secret-token provider URL leaked from transport"
        )

    def secret_error_object_opener(_request, timeout=15.0):
        del timeout
        return FakeResponse(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32000,
                    "message": "secret-token receipt proof error object",
                },
            }
        )

    cases = (
        (
            secret_http_error_opener,
            "JSON-RPC eth_chainId failed with HTTP 503",
            "secret-bearing HTTP receipt RPC error was accepted",
        ),
        (
            secret_url_error_opener,
            "JSON-RPC eth_chainId request failed",
            "secret-bearing receipt RPC transport error was accepted",
        ),
        (
            secret_error_object_opener,
            "JSON-RPC eth_chainId returned error response",
            "secret-bearing receipt RPC error object was accepted",
        ),
    )
    for opener, expected_message, failure in cases:
        try:
            module._json_rpc(
                "https://rpc.example",
                "eth_chainId",
                [],
                opener=opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            message = str(exc)
            assert message == expected_message
            assert "secret-token" not in message
            assert "error object" not in message
            assert exc.__cause__ is None
            if expected_message != "JSON-RPC eth_chainId returned error response":
                assert exc.__suppress_context__ is True
        else:
            raise AssertionError(failure)


def test_receipt_json_rpc_rejects_envelope_drift():
    module = load_module()

    cases = (
        (
            {"jsonrpc": "2.0", "id": 2, "result": "0x1"},
            "response id",
            "mismatched receipt JSON-RPC id was accepted",
        ),
        (
            {"jsonrpc": "2.0", "id": "1", "result": "0x1"},
            "response id",
            "string receipt JSON-RPC id was accepted",
        ),
        (
            {"jsonrpc": "2.0", "id": True, "result": "0x1"},
            "response id",
            "boolean receipt JSON-RPC id was accepted",
        ),
        (
            {"id": 1, "result": "0x1"},
            "protocol version",
            "missing receipt JSON-RPC protocol version was accepted",
        ),
        (
            {"jsonrpc": "2.0 ", "id": 1, "result": "0x1"},
            "protocol version",
            "padded receipt JSON-RPC protocol version was accepted",
        ),
    )

    for payload, expected_message, failure in cases:
        def opener(_request, timeout=15.0, payload=payload):
            del timeout
            return FakeResponse(payload)

        try:
            module._json_rpc(
                "https://rpc.example",
                "eth_chainId",
                [],
                opener=opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            assert expected_message in str(exc)
        else:
            raise AssertionError(failure)

    original_json_loads = module.json.loads

    def hostile_json_loads(*args, **kwargs):
        decoded = original_json_loads(*args, **kwargs)
        if type(decoded) is dict and decoded.get("jsonrpc") == "2.0":
            decoded["jsonrpc"] = HostileReceiptString("2.0")
        return decoded

    def opener(_request, timeout=15.0):
        del timeout
        return FakeResponse({"jsonrpc": "2.0", "id": 1, "result": "0x1"})

    module.json.loads = hostile_json_loads
    try:
        try:
            module._json_rpc(
                "https://rpc.example",
                "eth_chainId",
                [],
                opener=opener,
                timeout=3.0,
            )
        except RuntimeError as exc:
            rendered = str(exc)
            assert "protocol version" in rendered
            assert "secret-token" not in rendered
        else:
            raise AssertionError("hostile receipt JSON-RPC protocol version was accepted")
    finally:
        module.json.loads = original_json_loads


def test_collect_receipt_proof_rejects_failed_receipt():
    module = load_module()
    receipts = block_receipts(module, status="0x0")
    opener = fake_opener_for(module, receipts=receipts)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "receipt.status must be 0x1" in str(exc)
    else:
        raise AssertionError("failed receipt was accepted")


def test_collect_receipt_proof_rejects_hostile_receipt_status_without_hooks():
    module = load_module()
    opener = fake_opener_for(module)
    original_json_loads = module.json.loads

    def hostile_json_loads(*args, **kwargs):
        decoded = original_json_loads(*args, **kwargs)
        result = decoded.get("result") if type(decoded) is dict else None
        if type(result) is dict and result.get("status") == "0x1":
            result["status"] = HostileReceiptString("0x1")
        return decoded

    module.json.loads = hostile_json_loads
    try:
        try:
            module.collect_receipt_proof_evidence(
                "https://rpc.example",
                domain=module.SCCP_DOMAIN_ETH,
                transaction_hash=bytes.fromhex("11" * 32),
                source_bridge_address=bytes.fromhex("33" * 20),
                opener=opener,
            )
        except RuntimeError as exc:
            rendered = str(exc)
            assert rendered == "receipt.status must be 0x1"
            assert "secret-token" not in rendered
        else:
            raise AssertionError("hostile receipt status was accepted")
    finally:
        module.json.loads = original_json_loads


def test_collect_receipt_proof_rejects_receipts_root_mismatch():
    module = load_module()
    opener = fake_opener_for(module, block_receipts_root="0x" + "99" * 32)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "computed receipt trie root does not match" in str(exc)
    else:
        raise AssertionError("receipt root mismatch was accepted")


def test_collect_receipt_proof_rejects_duplicate_source_event_logs():
    module = load_module()
    receipts = block_receipts(module, duplicate_source_log=True)
    opener = fake_opener_for(module, receipts=receipts)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "duplicate SCCP source event logs" in str(exc)
    else:
        raise AssertionError("duplicate SCCP source event logs were accepted")


def test_receipt_trie_builder_rejects_non_boolean_removed_log_flags():
    module = load_module()

    for removed, expected_message in (
        (True, "receipt.logs[0] must not be removed"),
        (None, "receipt.logs[0].removed must be a boolean"),
        ("secret-token-removed", "receipt.logs[0].removed must be a boolean"),
        (1, "receipt.logs[0].removed must be a boolean"),
    ):
        receipts = block_receipts(
            module,
            source_log_overrides={"removed": removed},
        )

        try:
            module.build_receipt_trie_proof_from_receipts(
                receipts,
                transaction_index=0,
            )
        except RuntimeError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
        else:
            raise AssertionError(
                f"receipt trie builder accepted removed log flag {removed!r}"
            )


def test_source_event_digest_rejects_non_boolean_removed_log_flags_directly():
    module = load_module()

    for removed, expected_message in (
        (True, "receipt.logs[0] must not be removed"),
        (None, "receipt.logs[0].removed must be a boolean"),
        ("secret-token-removed", "receipt.logs[0].removed must be a boolean"),
        (1, "receipt.logs[0].removed must be a boolean"),
    ):
        try:
            module._source_event_digest_from_receipt(
                receipt(
                    module,
                    index=0,
                    tx_byte=0x11,
                    logs=source_log(module, removed=removed),
                ),
                source_bridge_address=bytes.fromhex("33" * 20),
                transaction_hash=bytes.fromhex("11" * 32),
                block_hash=bytes.fromhex("aa" * 32),
                block_number=0x1234,
            )
        except RuntimeError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
        else:
            raise AssertionError(
                f"source event digest accepted removed log flag {removed!r}"
            )


def test_collect_receipt_proof_rejects_source_event_missing_context_fields():
    module = load_module()

    for field in ("transactionHash", "blockHash", "blockNumber"):
        receipts = block_receipts(module)
        del receipts[0]["logs"][0][field]
        opener = fake_opener_for(module, receipts=receipts)

        try:
            module.collect_receipt_proof_evidence(
                "https://rpc.example",
                domain=module.SCCP_DOMAIN_ETH,
                transaction_hash=bytes.fromhex("11" * 32),
                source_bridge_address=bytes.fromhex("33" * 20),
                opener=opener,
            )
        except RuntimeError as exc:
            assert f"receipt.logs[0].{field}" in str(exc)
        else:
            raise AssertionError(f"source event log without {field} was accepted")


def test_collect_receipt_proof_rejects_direct_receipt_rlp_drift():
    module = load_module()
    opener = fake_opener_for(
        module,
        transaction_receipt=receipt(module, index=0, tx_byte=0x11, logs=[]),
    )

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "target receipt RLP must match" in str(exc)
    else:
        raise AssertionError("direct receipt drift from proven block receipt was accepted")


def test_collect_receipt_proof_rejects_block_receipt_metadata_drift():
    module = load_module()

    for field, value, expected in (
        ("blockHash", "0x" + "ab" * 32, "blockHash does not match"),
        ("blockNumber", "0x1235", "blockNumber does not match"),
    ):
        receipts = block_receipts(module)
        receipts[0][field] = value
        opener = fake_opener_for(
            module,
            receipts=receipts,
            transaction_receipt=receipt(
                module,
                index=0,
                tx_byte=0x11,
                logs=source_log(module),
            ),
        )

        try:
            module.collect_receipt_proof_evidence(
                "https://rpc.example",
                domain=module.SCCP_DOMAIN_ETH,
                transaction_hash=bytes.fromhex("11" * 32),
                source_bridge_address=bytes.fromhex("33" * 20),
                opener=opener,
            )
        except RuntimeError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"block receipt {field} drift from direct receipt was accepted"
            )


def test_collect_receipt_proof_rejects_source_event_extra_topics():
    module = load_module()
    receipts = block_receipts(
        module,
        source_log_overrides={
            "topics": [
                module.EVM_SOURCE_EVENT_TOPIC,
                "0x" + "55" * 32,
                "0x" + "66" * 32,
            ]
        },
    )
    opener = fake_opener_for(module, receipts=receipts)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "must contain exactly 2 topics" in str(exc)
    else:
        raise AssertionError("source event log with extra topics was accepted")


def test_collect_receipt_proof_rejects_source_event_non_empty_data():
    module = load_module()
    receipts = block_receipts(module, source_log_overrides={"data": "0x01"})
    opener = fake_opener_for(module, receipts=receipts)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "source event log data must be 0x" in str(exc)
    else:
        raise AssertionError("source event log with non-empty data was accepted")


def test_source_event_digest_rejects_hostile_log_data_without_hooks():
    module = load_module()
    try:
        module._source_event_digest_from_receipt(
            receipt(
                module,
                index=0,
                tx_byte=0x11,
                logs=source_log(module, data=HostileReceiptString("0x")),
            ),
            source_bridge_address=bytes.fromhex("33" * 20),
            transaction_hash=bytes.fromhex("11" * 32),
            block_hash=bytes.fromhex("aa" * 32),
            block_number=0x1234,
        )
    except RuntimeError as exc:
        rendered = str(exc)
        assert rendered == "SCCP source event log data must be 0x"
        assert "secret-token" not in rendered
    else:
        raise AssertionError("hostile source-event log data was accepted")


def test_collect_receipt_proof_rejects_zero_source_event_digest():
    module = load_module()
    receipts = block_receipts(
        module,
        source_log_overrides={
            "topics": [module.EVM_SOURCE_EVENT_TOPIC, "0x" + "00" * 32],
        },
    )
    opener = fake_opener_for(module, receipts=receipts)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            source_bridge_address=bytes.fromhex("33" * 20),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "receipt.logs[0].topics[1] returned zero data" in str(exc)
    else:
        raise AssertionError("zero source event digest was accepted")


def test_receipt_trie_builder_rejects_receipt_order_drift():
    module = load_module()
    receipts = block_receipts(module, transaction_index_override=2)

    try:
        module.build_receipt_trie_proof_from_receipts(receipts, transaction_index=0)
    except RuntimeError as exc:
        assert "transactionIndex must match receipt order" in str(exc)
    else:
        raise AssertionError("receipt transactionIndex order drift was accepted")


def test_receipt_trie_builder_rejects_duplicate_transaction_hashes():
    module = load_module()
    receipts = block_receipts(module)
    receipts[1]["transactionHash"] = receipts[0]["transactionHash"]

    try:
        module.build_receipt_trie_proof_from_receipts(receipts, transaction_index=0)
    except RuntimeError as exc:
        assert "transactionHash values must be unique" in str(exc)
    else:
        raise AssertionError("duplicate block receipt transaction hashes were accepted")


def test_receipt_trie_key_uses_rlp_transaction_index_not_hashed_key():
    module = load_module()

    assert module._receipt_trie_key(0) == bytes.fromhex("80")
    assert module._receipt_trie_key(1) == bytes.fromhex("01")
    assert module._receipt_trie_key(128) == bytes.fromhex("8180")
