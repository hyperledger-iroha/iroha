#!/usr/bin/env python3
"""Collect EVM-family SCCP receipt-trie proof evidence from JSON-RPC."""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence


REPO_ROOT = Path(__file__).resolve().parents[1]
PYTHON_CLIENT = REPO_ROOT / "python"
if str(PYTHON_CLIENT) not in sys.path:
    sys.path.insert(0, str(PYTHON_CLIENT))

from iroha_torii_client.sccp import _keccak_256  # noqa: E402


Urlopen = Callable[..., Any]

SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
EXPECTED_RPC_CHAIN_IDS = {
    SCCP_DOMAIN_ETH: 1,
    SCCP_DOMAIN_BSC: 56,
}
EVM_RECEIPT_PROOF_JSON_RPC_MAX_RESPONSE_BYTES = 32 * 1024 * 1024
EVM_RECEIPT_PROOF_JSON_RPC_MAX_ERROR_BYTES = 4096
EVM_RECEIPT_PROOF_MAX_BLOCK_RECEIPTS = 4096
EVM_RECEIPT_PROOF_MAX_NODES = 64
EVM_RECEIPT_PROOF_MAX_NODE_BYTES = 16 * 1024
EVM_SOURCE_EVENT_ABI = b"SccpSourceEvent(bytes32)"
EVM_SOURCE_EVENT_TOPIC = "0x" + _keccak_256(EVM_SOURCE_EVENT_ABI).hex()


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _strip_lower_0x_hex(value: str, *, label: str) -> str:
    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    if not value.startswith("0x"):
        raise argparse.ArgumentTypeError(f"{label} must be canonical lowercase 0x hex")
    text = value[2:]
    if text != text.lower():
        raise argparse.ArgumentTypeError(f"{label} must use lowercase hex")
    return text


def parse_hex_bytes(value: str, *, label: str, byte_length: int, nonzero: bool = True) -> bytes:
    """Parse fixed-width canonical hex bytes."""

    text = _strip_lower_0x_hex(value, label=label)
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except (SystemExit, RuntimeError, TypeError, ValueError):
        raise argparse.ArgumentTypeError(f"{label} must be hex") from None
    if nonzero and not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def parse_hex32(value: str, *, label: str) -> bytes:
    """Parse a non-zero bytes32 hex value."""

    return parse_hex_bytes(value, label=label, byte_length=32)


def parse_evm_address(value: str, *, label: str) -> bytes:
    """Parse a non-zero EVM address."""

    return parse_hex_bytes(value, label=label, byte_length=20)


def parse_domain(value: str) -> int:
    """Parse an EVM-family source domain."""

    if value != value.strip():
        raise argparse.ArgumentTypeError("domain must be eth, bsc, 1, or 2")
    text = value.lower()
    if text == "eth":
        return SCCP_DOMAIN_ETH
    if text == "bsc":
        return SCCP_DOMAIN_BSC
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError("domain must be eth, bsc, 1, or 2")
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError("domain must be eth, bsc, 1, or 2")
    parsed = int(text, 10)
    if parsed not in EXPECTED_RPC_CHAIN_IDS:
        raise argparse.ArgumentTypeError("domain must be eth, bsc, 1, or 2")
    return parsed


def parse_rpc_chain_id(value: str) -> int:
    """Parse an expected chain id flag."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    if not value or not value.isascii() or not value.isdecimal():
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    if len(value) > 1 and value.startswith("0"):
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    parsed = int(value, 10)
    if parsed <= 0 or parsed > 0xFFFF_FFFF_FFFF_FFFF:
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a positive u64 integer"
        )
    return parsed


def _json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    decoded: dict[str, Any] = {}
    for key, value in pairs:
        if key in decoded:
            raise ValueError("JSON-RPC returned duplicate JSON keys")
        decoded[key] = value
    return decoded


def _json_rpc(
    rpc_url: str,
    method: str,
    params: list[Any],
    *,
    opener: Urlopen,
    timeout: float,
) -> Any:
    request = urllib.request.Request(
        rpc_url,
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": params},
            separators=(",", ":"),
        ).encode("utf-8"),
        headers={"Accept": "application/json", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with opener(request, timeout=timeout) as response:
            raw = response.read(EVM_RECEIPT_PROOF_JSON_RPC_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f"JSON-RPC {method} failed with HTTP {exc.code}") from None
    except urllib.error.URLError:
        raise RuntimeError(f"JSON-RPC {method} request failed") from None
    if len(raw) > EVM_RECEIPT_PROOF_JSON_RPC_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            f"JSON-RPC {method} response exceeds "
            f"{EVM_RECEIPT_PROOF_JSON_RPC_MAX_RESPONSE_BYTES} bytes"
        )
    try:
        decoded = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_json_object_without_duplicate_keys,
        )
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from None
    except ValueError as exc:
        if str(exc) == "JSON-RPC returned duplicate JSON keys":
            raise RuntimeError(f"JSON-RPC {method} returned duplicate JSON keys") from None
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from None
    if not isinstance(decoded, dict):
        raise RuntimeError(f"JSON-RPC {method} returned a non-object response")
    if decoded.get("jsonrpc") != "2.0":
        raise RuntimeError(f"JSON-RPC {method} returned an invalid protocol version")
    if decoded.get("id") != 1:
        raise RuntimeError(f"JSON-RPC {method} returned a mismatched response id")
    error = decoded.get("error")
    if error is not None:
        raise RuntimeError(f"JSON-RPC {method} returned error response")
    if "result" not in decoded:
        raise RuntimeError(f"JSON-RPC {method} returned no result")
    return decoded["result"]


def _rpc_quantity(result: Any, *, method: str) -> int:
    if not isinstance(result, str) or result != result.strip() or not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-canonical quantity")
    text = result[2:]
    if (
        not text
        or any(symbol not in "0123456789abcdef" for symbol in text)
        or (len(text) > 1 and text.startswith("0"))
    ):
        raise RuntimeError(f"{method} returned non-canonical quantity")
    return int(text, 16)


def _rpc_hex_data(result: Any, *, method: str) -> bytes:
    if not isinstance(result, str) or result != result.strip() or not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    text = result[2:]
    if len(text) % 2 != 0:
        raise RuntimeError(f"{method} returned odd-length hex")
    if any(symbol not in "0123456789abcdef" for symbol in text):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    try:
        return bytes.fromhex(text)
    except (SystemExit, RuntimeError, TypeError, ValueError):
        raise RuntimeError(
            f"{method} returned non-canonical lowercase 0x hex data"
        ) from None


def _rpc_fixed_hex_data(
    result: Any,
    *,
    method: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    raw = _rpc_hex_data(result, method=method)
    if len(raw) != byte_length:
        raise RuntimeError(f"{method} returned {len(raw)} bytes; expected {byte_length}")
    if nonzero and not any(raw):
        raise RuntimeError(f"{method} returned zero data")
    return raw


def _int_to_minimal_be(value: int, *, label: str) -> bytes:
    if value < 0:
        raise RuntimeError(f"{label} must not be negative")
    if value == 0:
        return b""
    return value.to_bytes((value.bit_length() + 7) // 8, "big")


def _rlp_bytes(value: bytes) -> bytes:
    if len(value) == 1 and value[0] < 0x80:
        return value
    if len(value) <= 55:
        return bytes([0x80 + len(value)]) + value
    length = _int_to_minimal_be(len(value), label="RLP byte length")
    return bytes([0xB7 + len(length)]) + length + value


def _rlp_list(items: Sequence[bytes]) -> bytes:
    payload = b"".join(items)
    if len(payload) <= 55:
        return bytes([0xC0 + len(payload)]) + payload
    length = _int_to_minimal_be(len(payload), label="RLP list length")
    return bytes([0xF7 + len(length)]) + length + payload


def rlp_encode(value: Any) -> bytes:
    """Encode bytes or nested byte lists using Ethereum RLP."""

    if isinstance(value, (bytes, bytearray)):
        return _rlp_bytes(bytes(value))
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return _rlp_list([rlp_encode(item) for item in value])
    raise TypeError("RLP value must be bytes or a sequence")


def _receipt_type(receipt: dict[str, Any]) -> int | None:
    raw_type = receipt.get("type")
    if raw_type is None:
        return None
    receipt_type = _rpc_quantity(raw_type, method="receipt.type")
    if receipt_type == 0:
        return None
    if receipt_type > 0x7F:
        raise RuntimeError("typed receipt type must fit one byte below 0x80")
    if receipt_type not in {0x01, 0x02, 0x03, 0x04}:
        raise RuntimeError("typed receipt type is not supported for Ethereum mainnet receipt proofs")
    return receipt_type


def _receipt_status_bytes(receipt: dict[str, Any]) -> bytes:
    if "status" not in receipt:
        raise RuntimeError("receipt.status is required")
    status = _rpc_quantity(receipt.get("status"), method="receipt.status")
    if status not in (0, 1):
        raise RuntimeError("receipt.status must be 0x0 or 0x1")
    return _int_to_minimal_be(status, label="receipt.status")


def _receipt_logs(receipt: dict[str, Any]) -> list[Any]:
    logs = receipt.get("logs")
    if not isinstance(logs, Sequence) or isinstance(logs, (str, bytes, bytearray)):
        raise RuntimeError("receipt.logs must be a list")
    encoded = []
    for log_index, log in enumerate(logs):
        if not isinstance(log, dict):
            raise RuntimeError(f"receipt.logs[{log_index}] must be an object")
        if log.get("removed") is True:
            raise RuntimeError(f"receipt.logs[{log_index}] must not be removed")
        address = _rpc_fixed_hex_data(
            log.get("address"),
            method=f"receipt.logs[{log_index}].address",
            byte_length=20,
            nonzero=False,
        )
        topics = log.get("topics")
        if not isinstance(topics, Sequence) or isinstance(topics, (str, bytes, bytearray)):
            raise RuntimeError(f"receipt.logs[{log_index}].topics must be a list")
        if len(topics) > 4:
            raise RuntimeError(f"receipt.logs[{log_index}].topics must contain at most 4 entries")
        encoded_topics = [
            _rpc_fixed_hex_data(
                topic,
                method=f"receipt.logs[{log_index}].topics[{topic_index}]",
                byte_length=32,
                nonzero=False,
            )
            for topic_index, topic in enumerate(topics)
        ]
        data = _rpc_hex_data(
            log.get("data"),
            method=f"receipt.logs[{log_index}].data",
        )
        encoded.append([address, encoded_topics, data])
    return encoded


def canonical_receipt_rlp(receipt: dict[str, Any]) -> bytes:
    """Return the canonical EIP-2718-aware receipt trie value."""

    payload = rlp_encode(
        [
            _receipt_status_bytes(receipt),
            _int_to_minimal_be(
                _rpc_quantity(
                    receipt.get("cumulativeGasUsed"),
                    method="receipt.cumulativeGasUsed",
                ),
                label="receipt.cumulativeGasUsed",
            ),
            _rpc_fixed_hex_data(
                receipt.get("logsBloom"),
                method="receipt.logsBloom",
                byte_length=256,
                nonzero=False,
            ),
            _receipt_logs(receipt),
        ]
    )
    receipt_type = _receipt_type(receipt)
    return payload if receipt_type is None else bytes([receipt_type]) + payload


def _encode_compact_path(nibbles: Sequence[int], *, leaf: bool) -> bytes:
    for nibble in nibbles:
        if nibble < 0 or nibble > 15:
            raise ValueError("trie path nibble out of range")
    flags = 2 if leaf else 0
    out = bytearray()
    if len(nibbles) % 2:
        out.append(((flags + 1) << 4) | nibbles[0])
        start = 1
    else:
        out.append(flags << 4)
        start = 0
    for index in range(start, len(nibbles), 2):
        out.append((nibbles[index] << 4) | nibbles[index + 1])
    return bytes(out)


def _bytes_to_nibbles(value: bytes) -> tuple[int, ...]:
    nibbles: list[int] = []
    for byte in value:
        nibbles.append(byte >> 4)
        nibbles.append(byte & 0x0F)
    return tuple(nibbles)


class _TrieNode:
    def __init__(
        self,
        kind: str,
        *,
        path: tuple[int, ...] = (),
        value: bytes = b"",
        child: "_TrieNode | None" = None,
        children: tuple["_TrieNode | None", ...] | None = None,
    ) -> None:
        self.kind = kind
        self.path = path
        self.value = value
        self.child = child
        self.children = children
        self.rlp: bytes | None = None


def _longest_common_prefix(paths: Sequence[tuple[int, ...]]) -> tuple[int, ...]:
    if not paths:
        return ()
    prefix = list(paths[0])
    for path in paths[1:]:
        limit = min(len(prefix), len(path))
        index = 0
        while index < limit and prefix[index] == path[index]:
            index += 1
        del prefix[index:]
        if not prefix:
            break
    return tuple(prefix)


def _build_trie_node(items: Sequence[tuple[tuple[int, ...], bytes]]) -> _TrieNode:
    if not items:
        raise ValueError("cannot build an empty trie node")
    if len(items) == 1:
        path, value = items[0]
        return _TrieNode("leaf", path=path, value=value)
    prefix = _longest_common_prefix([path for path, _value in items])
    if prefix:
        child_items = [(path[len(prefix):], value) for path, value in items]
        return _TrieNode("extension", path=prefix, child=_build_trie_node(child_items))
    grouped: list[list[tuple[tuple[int, ...], bytes]]] = [[] for _ in range(16)]
    branch_value = b""
    for path, value in items:
        if not path:
            branch_value = value
        else:
            grouped[path[0]].append((path[1:], value))
    children = tuple(
        None if not child_items else _build_trie_node(child_items)
        for child_items in grouped
    )
    return _TrieNode("branch", value=branch_value, children=children)


def _node_reference(node: _TrieNode) -> bytes:
    rlp = _encode_trie_node(node)
    return rlp if len(rlp) < 32 else _keccak_256(rlp)


def _encode_trie_node(node: _TrieNode) -> bytes:
    if node.rlp is not None:
        return node.rlp
    if node.kind == "leaf":
        node.rlp = rlp_encode([_encode_compact_path(node.path, leaf=True), node.value])
    elif node.kind == "extension":
        if node.child is None:
            raise ValueError("extension node missing child")
        node.rlp = rlp_encode(
            [_encode_compact_path(node.path, leaf=False), _node_reference(node.child)]
        )
    elif node.kind == "branch":
        if node.children is None or len(node.children) != 16:
            raise ValueError("branch node missing children")
        node.rlp = rlp_encode(
            [b"" if child is None else _node_reference(child) for child in node.children]
            + [node.value]
        )
    else:
        raise ValueError(f"unknown trie node kind {node.kind!r}")
    return node.rlp


def _collect_proof_nodes(node: _TrieNode, path: tuple[int, ...]) -> list[bytes]:
    proof = [_encode_trie_node(node)]
    if node.kind == "leaf":
        if node.path != path:
            raise RuntimeError("receipt trie proof path does not end at requested receipt")
        return proof
    if node.kind == "extension":
        if node.child is None or not path[: len(node.path)] == node.path:
            raise RuntimeError("receipt trie proof path does not match extension")
        proof.extend(_collect_proof_nodes(node.child, path[len(node.path):]))
        return proof
    if node.kind == "branch":
        if node.children is None:
            raise RuntimeError("receipt trie branch is malformed")
        if not path:
            if not node.value:
                raise RuntimeError("receipt trie branch has no value for requested receipt")
            return proof
        child = node.children[path[0]]
        if child is None:
            raise RuntimeError("receipt trie proof path is missing child")
        proof.extend(_collect_proof_nodes(child, path[1:]))
        return proof
    raise RuntimeError(f"unknown trie node kind {node.kind!r}")


def _receipt_trie_key(transaction_index: int) -> bytes:
    return rlp_encode(_int_to_minimal_be(transaction_index, label="transactionIndex"))


def build_receipt_trie_proof_from_receipts(
    receipts: Sequence[dict[str, Any]],
    *,
    transaction_index: int,
) -> dict[str, Any]:
    """Build and verify an index-keyed receipt-trie proof for a block receipt list."""

    if not receipts or len(receipts) > EVM_RECEIPT_PROOF_MAX_BLOCK_RECEIPTS:
        raise ValueError(
            "block receipts must contain "
            f"1..{EVM_RECEIPT_PROOF_MAX_BLOCK_RECEIPTS} entries"
        )
    if transaction_index < 0 or transaction_index >= len(receipts):
        raise ValueError("transactionIndex is outside the block receipt list")

    items = []
    seen_transaction_hashes: set[bytes] = set()
    target_receipt_rlp = b""
    for index, receipt in enumerate(receipts):
        if not isinstance(receipt, dict):
            raise TypeError(f"block receipts[{index}] must be an object")
        receipt_index = _rpc_quantity(
            receipt.get("transactionIndex"),
            method=f"block receipts[{index}].transactionIndex",
        )
        if receipt_index != index:
            raise RuntimeError("block receipt transactionIndex must match receipt order")
        transaction_hash = _rpc_fixed_hex_data(
            receipt.get("transactionHash"),
            method=f"block receipts[{index}].transactionHash",
            byte_length=32,
        )
        if transaction_hash in seen_transaction_hashes:
            raise RuntimeError("block receipt transactionHash values must be unique")
        seen_transaction_hashes.add(transaction_hash)
        receipt_rlp = canonical_receipt_rlp(receipt)
        if index == transaction_index:
            target_receipt_rlp = receipt_rlp
        key = _bytes_to_nibbles(_receipt_trie_key(index))
        items.append((key, receipt_rlp))

    root = _build_trie_node(items)
    root_rlp = _encode_trie_node(root)
    receipts_root = _keccak_256(root_rlp)
    proof_nodes = _collect_proof_nodes(
        root,
        _bytes_to_nibbles(_receipt_trie_key(transaction_index)),
    )
    if len(proof_nodes) > EVM_RECEIPT_PROOF_MAX_NODES:
        raise RuntimeError(
            f"receiptTrieProofNodes must contain at most {EVM_RECEIPT_PROOF_MAX_NODES} entries"
        )
    for index, node in enumerate(proof_nodes):
        if not node or len(node) > EVM_RECEIPT_PROOF_MAX_NODE_BYTES:
            raise RuntimeError(
                "receiptTrieProofNodes"
                f"[{index}] must contain 1..{EVM_RECEIPT_PROOF_MAX_NODE_BYTES} bytes"
            )
    return {
        "receipts_root": receipts_root,
        "receipt_rlp": target_receipt_rlp,
        "receipt_trie_key": _receipt_trie_key(transaction_index),
        "receipt_trie_proof_nodes": proof_nodes,
    }


def _require_mainnet_chain(
    rpc_url: str,
    *,
    domain: int,
    expected_rpc_chain_id: int | None,
    opener: Urlopen,
    timeout: float,
) -> int:
    chain_id = _rpc_quantity(
        _json_rpc(rpc_url, "eth_chainId", [], opener=opener, timeout=timeout),
        method="eth_chainId",
    )
    canonical = EXPECTED_RPC_CHAIN_IDS[domain]
    if expected_rpc_chain_id is None:
        expected_rpc_chain_id = canonical
    elif expected_rpc_chain_id != canonical:
        chain = "eth" if domain == SCCP_DOMAIN_ETH else "bsc"
        raise ValueError(
            "--expected-rpc-chain-id must match the canonical "
            f"{chain} mainnet chain id {canonical}"
        )
    if chain_id != expected_rpc_chain_id:
        chain = "eth" if domain == SCCP_DOMAIN_ETH else "bsc"
        raise ValueError(
            f"eth_chainId for {chain} lane must be canonical mainnet chain id "
            f"{expected_rpc_chain_id}, got {chain_id}"
        )
    return chain_id


def _source_event_digest_from_receipt(
    receipt: dict[str, Any],
    *,
    source_bridge_address: bytes,
    transaction_hash: bytes,
    block_hash: bytes,
    block_number: int,
) -> bytes:
    logs = receipt.get("logs")
    if not isinstance(logs, Sequence) or isinstance(logs, (str, bytes, bytearray)):
        raise RuntimeError("receipt.logs is required for SCCP source event validation")
    matched_digest: bytes | None = None
    for index, log in enumerate(logs):
        if not isinstance(log, dict):
            raise RuntimeError(f"receipt.logs[{index}] must be an object")
        if log.get("removed") is True:
            raise RuntimeError(f"receipt.logs[{index}] must not be removed")
        address = _rpc_fixed_hex_data(
            log.get("address"),
            method=f"receipt.logs[{index}].address",
            byte_length=20,
        )
        topics = log.get("topics")
        if not isinstance(topics, Sequence) or isinstance(topics, (str, bytes, bytearray)):
            raise RuntimeError(f"receipt.logs[{index}].topics must be a list")
        if len(topics) == 0 or topics[0] != EVM_SOURCE_EVENT_TOPIC:
            continue
        if address != source_bridge_address:
            continue
        if len(topics) != 2:
            raise RuntimeError("SCCP source event log must contain exactly 2 topics")
        if log.get("data") != "0x":
            raise RuntimeError("SCCP source event log data must be 0x")
        log_transaction_hash = _rpc_fixed_hex_data(
            log.get("transactionHash"),
            method=f"receipt.logs[{index}].transactionHash",
            byte_length=32,
        )
        if log_transaction_hash != transaction_hash:
            raise RuntimeError("source event log transactionHash does not match receipt")
        log_block_hash = _rpc_fixed_hex_data(
            log.get("blockHash"),
            method=f"receipt.logs[{index}].blockHash",
            byte_length=32,
        )
        if log_block_hash != block_hash:
            raise RuntimeError("source event log blockHash does not match receipt")
        log_block_number = _rpc_quantity(
            log.get("blockNumber"),
            method=f"receipt.logs[{index}].blockNumber",
        )
        if log_block_number != block_number:
            raise RuntimeError("source event log blockNumber does not match receipt")
        digest = _rpc_fixed_hex_data(
            topics[1],
            method=f"receipt.logs[{index}].topics[1]",
            byte_length=32,
        )
        if matched_digest is not None:
            raise RuntimeError("receipt contains duplicate SCCP source event logs")
        matched_digest = digest
    if matched_digest is None:
        raise RuntimeError("receipt did not contain the expected SCCP source event log")
    return matched_digest


def collect_receipt_proof_evidence(
    rpc_url: str,
    *,
    domain: int,
    transaction_hash: bytes,
    expected_rpc_chain_id: int | None = None,
    source_bridge_address: bytes | None = None,
    allow_receipt_only_evidence: bool = False,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Collect a receipt trie proof from a mainnet JSON-RPC endpoint."""

    chain_id = _require_mainnet_chain(
        rpc_url,
        domain=domain,
        expected_rpc_chain_id=expected_rpc_chain_id,
        opener=opener,
        timeout=timeout,
    )
    if source_bridge_address is None and not allow_receipt_only_evidence:
        raise ValueError(
            "source_bridge_address is required for SCCP source-event evidence; "
            "set allow_receipt_only_evidence=True only for generic "
            "receipt-trie diagnostics"
        )
    receipt = _json_rpc(
        rpc_url,
        "eth_getTransactionReceipt",
        [_hex(transaction_hash)],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(receipt, dict):
        raise RuntimeError("eth_getTransactionReceipt returned a non-object receipt")
    receipt_tx_hash = _rpc_fixed_hex_data(
        receipt.get("transactionHash"),
        method="receipt.transactionHash",
        byte_length=32,
    )
    if receipt_tx_hash != transaction_hash:
        raise RuntimeError("receipt.transactionHash does not match requested transaction")
    if receipt.get("status") != "0x1":
        raise RuntimeError("receipt.status must be 0x1")
    block_hash = _rpc_fixed_hex_data(
        receipt.get("blockHash"),
        method="receipt.blockHash",
        byte_length=32,
    )
    block_number = _rpc_quantity(receipt.get("blockNumber"), method="receipt.blockNumber")
    if block_number == 0:
        raise RuntimeError("receipt.blockNumber must be positive")
    transaction_index = _rpc_quantity(
        receipt.get("transactionIndex"),
        method="receipt.transactionIndex",
    )
    block = _json_rpc(
        rpc_url,
        "eth_getBlockByHash",
        [_hex(block_hash), False],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(block, dict):
        raise RuntimeError("eth_getBlockByHash returned a non-object block")
    block_number_check = _rpc_quantity(block.get("number"), method="block.number")
    if block_number_check != block_number:
        raise RuntimeError("block.number does not match receipt.blockNumber")
    block_hash_check = _rpc_fixed_hex_data(
        block.get("hash"),
        method="block.hash",
        byte_length=32,
    )
    if block_hash_check != block_hash:
        raise RuntimeError("block.hash does not match receipt.blockHash")
    block_receipts_root = _rpc_fixed_hex_data(
        block.get("receiptsRoot"),
        method="block.receiptsRoot",
        byte_length=32,
    )
    block_receipts = _json_rpc(
        rpc_url,
        "eth_getBlockReceipts",
        [hex(block_number)],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(block_receipts, Sequence) or isinstance(block_receipts, (str, bytes, bytearray)):
        raise RuntimeError("eth_getBlockReceipts returned a non-list response")
    proof = build_receipt_trie_proof_from_receipts(
        block_receipts,
        transaction_index=transaction_index,
    )
    if proof["receipts_root"] != block_receipts_root:
        raise RuntimeError("computed receipt trie root does not match block.receiptsRoot")
    indexed_receipt = block_receipts[transaction_index]
    if not isinstance(indexed_receipt, dict):
        raise RuntimeError("eth_getBlockReceipts target receipt must be an object")
    indexed_tx_hash = _rpc_fixed_hex_data(
        indexed_receipt.get("transactionHash"),
        method="block receipt transactionHash",
        byte_length=32,
    )
    if indexed_tx_hash != transaction_hash:
        raise RuntimeError("eth_getBlockReceipts target receipt does not match transactionHash")
    indexed_block_hash = _rpc_fixed_hex_data(
        indexed_receipt.get("blockHash"),
        method="block receipt blockHash",
        byte_length=32,
    )
    if indexed_block_hash != block_hash:
        raise RuntimeError("eth_getBlockReceipts target receipt blockHash does not match")
    indexed_block_number = _rpc_quantity(
        indexed_receipt.get("blockNumber"),
        method="block receipt blockNumber",
    )
    if indexed_block_number != block_number:
        raise RuntimeError("eth_getBlockReceipts target receipt blockNumber does not match")
    if proof["receipt_rlp"] != canonical_receipt_rlp(receipt):
        raise RuntimeError(
            "eth_getBlockReceipts target receipt RLP must match eth_getTransactionReceipt"
        )
    source_event_digest = None
    if source_bridge_address is not None:
        source_event_digest = _source_event_digest_from_receipt(
            receipt,
            source_bridge_address=source_bridge_address,
            transaction_hash=transaction_hash,
            block_hash=block_hash,
            block_number=block_number,
        )
    return {
        "domain": domain,
        "chain": "eth" if domain == SCCP_DOMAIN_ETH else "bsc",
        "read_only": True,
        "evidence_mode": (
            "receipt_only" if source_event_digest is None else "sccp_source_event"
        ),
        "rpc_chain_id": chain_id,
        "transaction_hash": _hex(transaction_hash),
        "transaction_index": transaction_index,
        "receipt_status": "0x1",
        "receipt_type": receipt.get("type", "0x0"),
        "receipt_rlp": _hex(proof["receipt_rlp"]),
        "block_hash": _hex(block_hash),
        "block_number": block_number,
        "execution_receipts_root": _hex(block_receipts_root),
        "computed_receipts_root": _hex(proof["receipts_root"]),
        "receipt_root_verified": True,
        "receipt_trie_key": _hex(proof["receipt_trie_key"]),
        "receipt_trie_proof_nodes": [
            _hex(node) for node in proof["receipt_trie_proof_nodes"]
        ],
        "source_event_validated": source_event_digest is not None,
        "receipt_only_evidence": source_event_digest is None,
        **(
            {}
            if source_event_digest is None
            else {"source_event_digest": _hex(source_event_digest)}
        ),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Collect read-only SCCP EVM-family receipt trie proof evidence from "
            "an app/operator supplied JSON-RPC endpoint."
        )
    )
    parser.add_argument("--rpc-url", required=True, help="Ethereum/BSC JSON-RPC URL.")
    parser.add_argument("--domain", required=True, type=parse_domain, help="eth or bsc.")
    parser.add_argument(
        "--transaction-hash",
        required=True,
        type=lambda value: parse_hex32(value, label="transaction hash"),
        help="Transaction hash whose successful receipt should be proven.",
    )
    parser.add_argument(
        "--expected-rpc-chain-id",
        type=parse_rpc_chain_id,
        help="Expected canonical mainnet chain id for the selected domain.",
    )
    parser.add_argument(
        "--source-bridge-address",
        type=lambda value: parse_evm_address(value, label="source bridge address"),
        help=(
            "SCCP source bridge address. Required by default so the receipt "
            "must contain exactly one canonical source event log."
        ),
    )
    parser.add_argument(
        "--allow-receipt-only-evidence",
        action="store_true",
        help=(
            "Collect generic receipt-trie diagnostics without SCCP source-event "
            "validation. Do not use this mode as source proof material."
        ),
    )
    parser.add_argument("--timeout", type=float, default=15.0, help="HTTP timeout in seconds.")
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer ",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session=",
    "token=",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, OSError):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any((ord(ch) < 0x20 and ch not in "\n\t") or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.source_bridge_address is None and not args.allow_receipt_only_evidence:
        parser.error(
            "--source-bridge-address is required for SCCP source-event evidence; "
            "pass --allow-receipt-only-evidence only for generic "
            "receipt-trie diagnostics"
        )
    try:
        summary = collect_receipt_proof_evidence(
            args.rpc_url,
            domain=args.domain,
            transaction_hash=args.transaction_hash,
            expected_rpc_chain_id=args.expected_rpc_chain_id,
            source_bridge_address=args.source_bridge_address,
            allow_receipt_only_evidence=args.allow_receipt_only_evidence,
            timeout=args.timeout,
        )
    except (OSError, RuntimeError, TypeError, ValueError, argparse.ArgumentTypeError) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP EVM receipt proof evidence collection failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
