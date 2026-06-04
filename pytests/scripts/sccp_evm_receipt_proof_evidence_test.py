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


def rpc_response(result):
    return {"jsonrpc": "2.0", "id": 1, "result": result}


def quantity(value):
    return "0x" + format(value, "x")


def hex_bytes(byte, count):
    return "0x" + f"{byte:02x}" * count


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
    assert summary["chain"] == "eth"
    assert summary["rpc_chain_id"] == 1
    assert summary["transaction_hash"] == "0x" + "11" * 32
    assert summary["transaction_index"] == 0
    assert summary["receipt_status"] == "0x1"
    assert summary["receipt_root_verified"] is True
    assert summary["execution_receipts_root"] == summary["computed_receipts_root"]
    assert summary["source_event_digest"] == "0x" + "55" * 32
    assert summary["receipt_rlp"].startswith("0x02")
    assert summary["receipt_trie_key"] == "0x80"
    assert len(summary["receipt_trie_proof_nodes"]) >= 1
    assert [call[0] for call in opener.calls] == [
        "eth_chainId",
        "eth_getTransactionReceipt",
        "eth_getBlockByHash",
        "eth_getBlockReceipts",
    ]


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
            opener=opener,
        )
    except ValueError as exc:
        assert "eth_chainId for eth lane" in str(exc)
    else:
        raise AssertionError("non-mainnet ETH chain id was accepted")
    assert [call[0] for call in opener.calls] == ["eth_chainId"]


def test_collect_receipt_proof_rejects_failed_receipt():
    module = load_module()
    receipts = block_receipts(module, status="0x0")
    opener = fake_opener_for(module, receipts=receipts)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
            opener=opener,
        )
    except RuntimeError as exc:
        assert "receipt.status must be 0x1" in str(exc)
    else:
        raise AssertionError("failed receipt was accepted")


def test_collect_receipt_proof_rejects_receipts_root_mismatch():
    module = load_module()
    opener = fake_opener_for(module, block_receipts_root="0x" + "99" * 32)

    try:
        module.collect_receipt_proof_evidence(
            "https://rpc.example",
            domain=module.SCCP_DOMAIN_ETH,
            transaction_hash=bytes.fromhex("11" * 32),
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


def test_receipt_trie_builder_rejects_receipt_order_drift():
    module = load_module()
    receipts = block_receipts(module, transaction_index_override=2)

    try:
        module.build_receipt_trie_proof_from_receipts(receipts, transaction_index=0)
    except RuntimeError as exc:
        assert "transactionIndex must match receipt order" in str(exc)
    else:
        raise AssertionError("receipt transactionIndex order drift was accepted")


def test_receipt_trie_key_uses_rlp_transaction_index_not_hashed_key():
    module = load_module()

    assert module._receipt_trie_key(0) == bytes.fromhex("80")
    assert module._receipt_trie_key(1) == bytes.fromhex("01")
    assert module._receipt_trie_key(128) == bytes.fromhex("8180")
