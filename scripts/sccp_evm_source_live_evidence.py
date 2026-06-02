#!/usr/bin/env python3
"""Collect read-only SCCP EVM-family source bridge evidence from JSON-RPC."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from types import SimpleNamespace
from typing import Any, Callable


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))


Urlopen = Callable[..., Any]
EVM_SOURCE_JSON_RPC_MAX_RESPONSE_BYTES = 1024 * 1024
EVM_SOURCE_JSON_RPC_MAX_ERROR_BYTES = 4096

SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
EXPECTED_RPC_CHAIN_IDS = {
    SCCP_DOMAIN_ETH: 1,
    SCCP_DOMAIN_BSC: 56,
}
EVM_SOURCE_ALLOWED_BLOCK_TAGS = frozenset(("latest", "safe", "finalized"))


def _strip_lower_0x_hex(value: str, *, label: str) -> str:
    if value.startswith("0X"):
        raise argparse.ArgumentTypeError(f"{label} must use lowercase 0x prefix")
    if not value.startswith("0x"):
        raise argparse.ArgumentTypeError(f"{label} must be canonical lowercase 0x hex")
    text = value[2:]
    if text != text.lower():
        raise argparse.ArgumentTypeError(f"{label} must use lowercase hex")
    return text


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _load_evidence_module(domain: int) -> Any:
    filename = {
        SCCP_DOMAIN_ETH: "sccp_eth_source_bridge_evidence.py",
        SCCP_DOMAIN_BSC: "sccp_bsc_source_bridge_evidence.py",
    }.get(domain)
    if filename is None:
        raise ValueError("domain must be eth or bsc")
    path = SCRIPT_DIR / filename
    module_name = f"_sccp_evm_source_live_{path.stem}"
    cached = sys.modules.get(module_name)
    if cached is not None:
        return cached
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def parse_domain(value: str) -> int:
    """Parse an EVM-family source domain selector."""

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
    if parsed not in (SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC):
        raise argparse.ArgumentTypeError("domain must be eth, bsc, 1, or 2")
    return parsed


def _parse_hex_bytes(value: str, *, label: str, byte_length: int) -> bytes:
    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_lower_0x_hex(value, label=label)
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def _parse_hex32(value: str, *, label: str) -> bytes:
    return _parse_hex_bytes(value, label=label, byte_length=32)


def _summary_hex_bytes(
    record: dict[str, Any],
    field: str,
    *,
    label: str,
    byte_length: int,
) -> bytes:
    value = record.get(field)
    if not isinstance(value, str):
        raise ValueError(f"{label} must be an exact hex string")
    try:
        raw = _parse_hex_bytes(value, label=label, byte_length=byte_length)
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc
    if value != _hex(raw):
        raise ValueError(f"{label} must be canonical lowercase 0x hex")
    return raw


def _summary_hex32(record: dict[str, Any], field: str, *, label: str) -> bytes:
    return _summary_hex_bytes(record, field, label=label, byte_length=32)


def _summary_address(record: dict[str, Any], field: str, *, label: str) -> bytes:
    return _summary_hex_bytes(record, field, label=label, byte_length=20)


def _summary_runtime_bytes(
    evidence: Any,
    record: dict[str, Any],
    field: str,
    *,
    label: str,
) -> bytes:
    value = record.get(field)
    if not isinstance(value, str) or not value.startswith("0x"):
        raise ValueError(f"{label} must be exact 0x-prefixed hex")
    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise ValueError(f"{label} must not contain whitespace")
    try:
        raw = evidence.parse_runtime_bytecode_hex(value, label=label)
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc
    if value != _hex(raw):
        raise ValueError(f"{label} must be canonical lowercase 0x hex")
    return raw


def _parse_rpc_chain_id(value: str) -> int:
    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    text = value
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a canonical decimal integer"
        )
    parsed = int(text, 10)
    if parsed <= 0 or parsed > 0xFFFF_FFFF_FFFF_FFFF:
        raise argparse.ArgumentTypeError(
            "--expected-rpc-chain-id must be a positive u64 integer"
        )
    return parsed


def parse_block_tag(value: str) -> str:
    """Parse a stable/canonical JSON-RPC block tag for read-only evidence."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "--block-tag must not contain surrounding whitespace"
        )
    if value in EVM_SOURCE_ALLOWED_BLOCK_TAGS:
        return value
    if value.startswith("0x"):
        text = value[2:]
        if (
            not text
            or any(symbol not in "0123456789abcdef" for symbol in text)
            or (len(text) > 1 and text.startswith("0"))
        ):
            raise argparse.ArgumentTypeError(
                "--block-tag must be latest, safe, finalized, or a positive "
                "canonical lowercase 0x block number"
            )
        parsed = int(text, 16)
        if parsed <= 0:
            raise argparse.ArgumentTypeError(
                "--block-tag block number must be positive"
            )
        return "0x" + format(parsed, "x")
    raise argparse.ArgumentTypeError(
        "--block-tag must be latest, safe, finalized, or a positive canonical "
        "lowercase 0x block number"
    )


def _require_exact_positive_u64(value: object, *, label: str) -> int:
    if type(value) is not int or value <= 0 or value > 0xFFFF_FFFF_FFFF_FFFF:
        raise ValueError(f"{label} must be an exact positive u64")
    return value


def _require_exact_u32(value: object, *, label: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        raise ValueError(f"{label} must be an exact u32")
    return value


def _source_arg_bytes(
    args: Any,
    field: str,
    *,
    label: str,
    byte_length: int,
) -> bytes:
    value = getattr(args, field, None)
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be bytes")
    raw = bytes(value)
    if len(raw) != byte_length:
        raise ValueError(f"{label} must be {byte_length} bytes")
    if not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def _source_arg_optional_bytes(
    args: Any,
    field: str,
    *,
    label: str,
    byte_length: int,
) -> bytes | None:
    value = getattr(args, field, None)
    if value is None:
        return None
    return _source_arg_bytes(args, field, label=label, byte_length=byte_length)


def _source_arg_runtime_bytes(args: Any) -> bytes:
    value = getattr(args, "source_bridge_runtime_bytecode_hex", None)
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError("source bridge runtime bytecode argument must be bytes")
    raw = bytes(value)
    if not raw or not any(raw):
        raise ValueError("source bridge runtime bytecode argument must not be empty")
    return raw


def _http_error_detail(exc: urllib.error.HTTPError) -> str:
    raw = exc.read(EVM_SOURCE_JSON_RPC_MAX_ERROR_BYTES + 1)
    truncated = len(raw) > EVM_SOURCE_JSON_RPC_MAX_ERROR_BYTES
    detail = raw[:EVM_SOURCE_JSON_RPC_MAX_ERROR_BYTES].decode("utf-8", "replace")
    if truncated:
        detail += "...<truncated>"
    return detail


def _json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    decoded: dict[str, Any] = {}
    for key, value in pairs:
        if key in decoded:
            raise ValueError(f"JSON-RPC returned duplicate JSON key {key!r}")
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
            raw = response.read(EVM_SOURCE_JSON_RPC_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        detail = _http_error_detail(exc)
        raise RuntimeError(
            f"JSON-RPC {method} failed with HTTP {exc.code}: {detail}"
        ) from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"JSON-RPC {method} request failed: {exc.reason}") from exc
    if len(raw) > EVM_SOURCE_JSON_RPC_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            f"JSON-RPC {method} response exceeds "
            f"{EVM_SOURCE_JSON_RPC_MAX_RESPONSE_BYTES} bytes"
        )
    try:
        decoded = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_json_object_without_duplicate_keys,
        )
    except UnicodeDecodeError as exc:
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"JSON-RPC {method} returned invalid JSON") from exc
    except ValueError as exc:
        raise RuntimeError(str(exc)) from exc
    if not isinstance(decoded, dict):
        raise RuntimeError(f"JSON-RPC {method} returned a non-object response")
    if decoded.get("jsonrpc") != "2.0":
        raise RuntimeError(f"JSON-RPC {method} returned an invalid protocol version")
    if decoded.get("id") != 1:
        raise RuntimeError(f"JSON-RPC {method} returned a mismatched response id")
    error = decoded.get("error")
    if error is not None:
        raise RuntimeError(f"JSON-RPC {method} error: {error}")
    if "result" not in decoded:
        raise RuntimeError(f"JSON-RPC {method} returned no result")
    return decoded["result"]


def _rpc_quantity(result: Any, *, method: str) -> int:
    if not isinstance(result, str) or not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-quantity data")
    if result != result.strip():
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
    if not isinstance(result, str):
        raise RuntimeError(f"{method} returned non-string data")
    if result != result.strip():
        raise RuntimeError(f"{method} returned non-canonical hex data")
    if not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    text = result[2:]
    if len(text) % 2 != 0:
        raise RuntimeError(f"{method} returned odd-length hex")
    if any(symbol not in "0123456789abcdef" for symbol in text):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    return bytes.fromhex(text)


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


def _runtime_code_hash(
    evidence: Any,
    rpc_url: str,
    *,
    address: str,
    block_tag: str,
    opener: Urlopen,
    timeout: float,
) -> tuple[bytes, str]:
    result = _json_rpc(
        rpc_url,
        "eth_getCode",
        [address, block_tag],
        opener=opener,
        timeout=timeout,
    )
    runtime = _rpc_hex_data(result, method="eth_getCode source bridge")
    if not runtime or not any(runtime):
        raise RuntimeError("source bridge runtime bytecode is empty")
    return evidence.runtime_bytecode_hash(runtime), _hex(runtime)


def _receipt_summary(
    rpc_url: str,
    *,
    deployment_transaction_hash: bytes | None,
    bridge_address: str,
    opener: Urlopen,
    timeout: float,
) -> dict[str, Any]:
    if deployment_transaction_hash is None:
        return {}
    result = _json_rpc(
        rpc_url,
        "eth_getTransactionReceipt",
        [_hex(deployment_transaction_hash)],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(result, dict):
        raise RuntimeError("eth_getTransactionReceipt returned a non-object receipt")
    receipt_transaction_hash = result.get("transactionHash")
    try:
        parsed_receipt_transaction_hash = _rpc_fixed_hex_data(
            receipt_transaction_hash,
            method="eth_getTransactionReceipt transactionHash",
            byte_length=32,
        )
    except RuntimeError as exc:
        raise RuntimeError(
            "deployment receipt transactionHash must be a non-zero bytes32"
        ) from exc
    if parsed_receipt_transaction_hash != deployment_transaction_hash:
        raise RuntimeError(
            "deployment receipt transactionHash does not match requested "
            "deployment transaction"
        )
    status = result.get("status")
    if status != "0x1":
        raise RuntimeError("deployment transaction receipt status must be 0x1")
    contract_address = result.get("contractAddress")
    try:
        parsed_contract_address = _rpc_fixed_hex_data(
            contract_address,
            byte_length=20,
            method="eth_getTransactionReceipt contractAddress",
        )
    except RuntimeError as exc:
        raise RuntimeError(
            "deployment receipt contractAddress must be a non-zero 20-byte EVM address"
        ) from exc
    if _hex(parsed_contract_address).lower() != bridge_address.lower():
        raise RuntimeError(
            "deployment receipt contractAddress does not match source bridge"
        )
    block_hash = result.get("blockHash")
    try:
        parsed_block_hash = _rpc_fixed_hex_data(
            block_hash,
            method="eth_getTransactionReceipt blockHash",
            byte_length=32,
        )
    except RuntimeError as exc:
        raise RuntimeError(
            "deployment receipt blockHash must be a non-zero bytes32"
        ) from exc
    block_number = result.get("blockNumber")
    block_number_value = _rpc_quantity(
        block_number,
        method="eth_getTransactionReceipt blockNumber",
    )
    if block_number_value <= 0:
        raise RuntimeError("deployment receipt blockNumber must be non-zero")
    return {
        "deployment_transaction_hash": _hex(deployment_transaction_hash),
        "deployment_receipt_status": status,
        "deployment_receipt_contract_address": _hex(parsed_contract_address),
        "deployment_receipt_block_hash": _hex(parsed_block_hash),
        "deployment_receipt_block_number": block_number_value,
    }


def _verify_receipt_block_header(
    rpc_url: str,
    *,
    block_number: int,
    expected_block_hash: bytes,
    opener: Urlopen,
    timeout: float,
) -> bytes:
    result = _json_rpc(
        rpc_url,
        "eth_getBlockByNumber",
        [hex(block_number), False],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(result, dict):
        raise RuntimeError("eth_getBlockByNumber returned a non-object block")
    header_number = _rpc_quantity(
        result.get("number"),
        method="eth_getBlockByNumber number",
    )
    if header_number != block_number:
        raise RuntimeError(
            "deployment receipt block number does not match eth_getBlockByNumber"
        )
    header_hash = _rpc_fixed_hex_data(
        result.get("hash"),
        method="eth_getBlockByNumber hash",
        byte_length=32,
    )
    if header_hash != expected_block_hash:
        raise RuntimeError(
            "deployment receipt blockHash does not match eth_getBlockByNumber"
        )
    receipts_root = _rpc_fixed_hex_data(
        result.get("receiptsRoot"),
        method="eth_getBlockByNumber receiptsRoot",
        byte_length=32,
    )
    return receipts_root


def collect_source_bridge_evidence(
    rpc_url: str,
    *,
    domain: int,
    bridge_address: str,
    block_tag: str,
    deployment_transaction_hash: bytes | None = None,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Collect and verify read-only EVM-family source bridge evidence."""

    block_tag = parse_block_tag(block_tag)
    evidence = _load_evidence_module(domain)
    bridge = _hex(evidence.parse_evm_address(bridge_address, label="bridge address"))
    chain_id = _rpc_quantity(
        _json_rpc(
            rpc_url,
            "eth_chainId",
            [],
            opener=opener,
            timeout=timeout,
        ),
        method="eth_chainId",
    )
    bridge_code_hash, bridge_runtime_bytecode_hex = _runtime_code_hash(
        evidence,
        rpc_url,
        address=bridge,
        block_tag=block_tag,
        opener=opener,
        timeout=timeout,
    )
    receipt_summary = _receipt_summary(
        rpc_url,
        deployment_transaction_hash=deployment_transaction_hash,
        bridge_address=bridge,
        opener=opener,
        timeout=timeout,
    )
    if receipt_summary:
        receipt_block_tag = hex(receipt_summary["deployment_receipt_block_number"])
        receipt_block_hash = _parse_hex32(
            receipt_summary["deployment_receipt_block_hash"],
            label="deployment receipt block hash",
        )
        receipt_block_receipts_root = _verify_receipt_block_header(
            rpc_url,
            block_number=receipt_summary["deployment_receipt_block_number"],
            expected_block_hash=receipt_block_hash,
            opener=opener,
            timeout=timeout,
        )
        receipt_summary["deployment_receipt_block_hash_matches"] = True
        receipt_summary["deployment_receipt_block_receipts_root"] = _hex(
            receipt_block_receipts_root
        )
        receipt_summary["deployment_receipt_block_receipts_root_verified"] = True
        receipt_block_code_hash, receipt_block_runtime_bytecode_hex = _runtime_code_hash(
            evidence,
            rpc_url,
            address=bridge,
            block_tag=receipt_block_tag,
            opener=opener,
            timeout=timeout,
        )
        if receipt_block_code_hash != bridge_code_hash:
            raise RuntimeError(
                "source bridge code hash at deployment receipt block does not "
                "match selected block tag"
            )
        if receipt_block_runtime_bytecode_hex != bridge_runtime_bytecode_hex:
            raise RuntimeError(
                "source bridge runtime bytecode at deployment receipt block does "
                "not match selected block tag"
            )
        receipt_summary["deployment_receipt_block_code_hash_matches"] = True
    summary: dict[str, Any] = {
        "domain": domain,
        "chain": "eth" if domain == SCCP_DOMAIN_ETH else "bsc",
        "rpc_chain_id": chain_id,
        "bridge_address": bridge,
        "bridge_code_hash": _hex(bridge_code_hash),
        "bridge_runtime_bytecode_hex": bridge_runtime_bytecode_hex,
    }
    summary.update(receipt_summary)
    return summary


def _component_hash_args_present(args: argparse.Namespace) -> bool:
    return all(
        getattr(args, name, None) is not None
        for name in (
            "source_trust_anchor_hash",
            "consensus_verifier_hash",
            "message_inclusion_verifier_hash",
            "finality_policy_hash",
            "deployment_receipt_hash",
        )
    )


def _adapter_verifier_vk_hash(evidence: Any, domain: int) -> bytes:
    if domain == SCCP_DOMAIN_ETH:
        return evidence.eth_source_adapter_verifier_vk_hash()
    if domain == SCCP_DOMAIN_BSC:
        return evidence.bsc_source_adapter_verifier_vk_hash()
    raise ValueError("domain must be eth or bsc")


def _source_args(
    args: argparse.Namespace,
    source_bridge: dict[str, Any],
) -> SimpleNamespace:
    if not _component_hash_args_present(args):
        raise ValueError(
            "source TOML output requires source component hashes and "
            "--deployment-receipt-hash"
        )
    evidence = _load_evidence_module(args.domain)
    return SimpleNamespace(
        source_domain=args.domain,
        target_domain=SCCP_DOMAIN_SORA,
        bridge_address=evidence.parse_evm_address(
            source_bridge["bridge_address"],
            label="bridge address",
        ),
        source_trust_anchor_hash=args.source_trust_anchor_hash,
        consensus_verifier_hash=args.consensus_verifier_hash,
        message_inclusion_verifier_hash=args.message_inclusion_verifier_hash,
        source_bridge_emitter_code_hash=_parse_hex32(
            source_bridge["bridge_code_hash"],
            label="source bridge code hash",
        ),
        finality_policy_hash=args.finality_policy_hash,
        adapter_verifier_vk_hash=(
            args.adapter_verifier_vk_hash
            if args.adapter_verifier_vk_hash is not None
            else _adapter_verifier_vk_hash(evidence, args.domain)
        ),
        deployment_receipt_hash=args.deployment_receipt_hash,
        deployment_transaction_hash=(
            _parse_hex32(
                source_bridge["deployment_transaction_hash"],
                label="deployment transaction hash",
            )
            if source_bridge.get("deployment_transaction_hash") is not None
            else None
        ),
        deployment_receipt_contract_address=(
            evidence.parse_evm_address(
                source_bridge["deployment_receipt_contract_address"],
                label="deployment receipt contract address",
            )
            if source_bridge.get("deployment_receipt_contract_address") is not None
            else None
        ),
        deployment_receipt_block_hash=(
            _parse_hex32(
                source_bridge["deployment_receipt_block_hash"],
                label="deployment receipt block hash",
            )
            if source_bridge.get("deployment_receipt_block_hash") is not None
            else None
        ),
        deployment_receipt_block_number=(
            _require_exact_positive_u64(
                source_bridge["deployment_receipt_block_number"],
                label="deployment receipt block number",
            )
            if source_bridge.get("deployment_receipt_block_number") is not None
            else None
        ),
        deployment_receipt_block_receipts_root=(
            _parse_hex32(
                source_bridge["deployment_receipt_block_receipts_root"],
                label="deployment receipt block receiptsRoot",
            )
            if source_bridge.get("deployment_receipt_block_receipts_root") is not None
            else None
        ),
        expected_source_verifier_material_hash=args.expected_source_verifier_material_hash,
        expected_source_adapter_engine_deployment_hash=(
            args.expected_source_adapter_engine_deployment_hash
        ),
        source_bridge_runtime_bytecode_hex=evidence.parse_runtime_bytecode_hex(
            str(source_bridge["bridge_runtime_bytecode_hex"]),
            label="source bridge runtime bytecode",
        ),
        source_bridge_runtime_bytecode_file=None,
    )


def _source_record_hashes(evidence: Any, domain: int, args: SimpleNamespace) -> tuple[bytes, bytes]:
    if domain == SCCP_DOMAIN_ETH:
        return (
            evidence.eth_source_verifier_material_record_hash(args),
            evidence.eth_source_adapter_engine_deployment_record_hash(args),
        )
    if domain == SCCP_DOMAIN_BSC:
        return (
            evidence.bsc_source_verifier_material_record_hash(args),
            evidence.bsc_source_adapter_engine_deployment_record_hash(args),
        )
    raise ValueError("domain must be eth or bsc")


def _offline_args(args: argparse.Namespace, source_bridge: dict[str, Any]) -> list[str]:
    if not _component_hash_args_present(args):
        return []
    source_args = _source_args(args, source_bridge)
    rendered = [
        "--source-domain",
        str(args.domain),
        "--target-domain",
        str(SCCP_DOMAIN_SORA),
        "--bridge-address",
        source_bridge["bridge_address"],
        "--source-trust-anchor-hash",
        _hex(source_args.source_trust_anchor_hash),
        "--consensus-verifier-hash",
        _hex(source_args.consensus_verifier_hash),
        "--message-inclusion-verifier-hash",
        _hex(source_args.message_inclusion_verifier_hash),
        "--source-bridge-emitter-code-hash",
        source_bridge["bridge_code_hash"],
        "--source-bridge-runtime-bytecode-hex",
        source_bridge["bridge_runtime_bytecode_hex"],
        "--finality-policy-hash",
        _hex(source_args.finality_policy_hash),
        "--adapter-verifier-vk-hash",
        _hex(source_args.adapter_verifier_vk_hash),
        "--deployment-receipt-hash",
        _hex(source_args.deployment_receipt_hash),
    ]
    if source_args.deployment_transaction_hash is not None:
        rendered.extend(
            [
                "--deployment-transaction-hash",
                _hex(source_args.deployment_transaction_hash),
            ]
        )
    if source_args.deployment_receipt_contract_address is not None:
        rendered.extend(
            [
                "--deployment-receipt-contract-address",
                _hex(source_args.deployment_receipt_contract_address),
            ]
        )
    if source_args.deployment_receipt_block_hash is not None:
        rendered.extend(
            [
                "--deployment-receipt-block-hash",
                _hex(source_args.deployment_receipt_block_hash),
            ]
        )
    if source_args.deployment_receipt_block_number is not None:
        rendered.extend(
            [
                "--deployment-receipt-block-number",
                str(source_args.deployment_receipt_block_number),
            ]
        )
    if source_args.deployment_receipt_block_receipts_root is not None:
        rendered.extend(
            [
                "--deployment-receipt-block-receipts-root",
                _hex(source_args.deployment_receipt_block_receipts_root),
            ]
        )
    if source_args.expected_source_verifier_material_hash is not None:
        rendered.extend(
            [
                "--expected-source-verifier-material-hash",
                _hex(source_args.expected_source_verifier_material_hash),
            ]
        )
    if source_args.expected_source_adapter_engine_deployment_hash is not None:
        rendered.extend(
            [
                "--expected-source-adapter-engine-deployment-hash",
                _hex(source_args.expected_source_adapter_engine_deployment_hash),
            ]
        )
    return rendered


def _source_bridge_deployment_receipt_is_verified(source_bridge: dict[str, Any]) -> bool:
    if source_bridge.get("deployment_receipt_status") != "0x1":
        return False
    if source_bridge.get("deployment_receipt_block_hash_matches") is not True:
        return False
    if source_bridge.get("deployment_receipt_block_code_hash_matches") is not True:
        return False
    if source_bridge.get("deployment_receipt_block_receipts_root_verified") is not True:
        return False
    try:
        _parse_hex32(
            source_bridge.get("deployment_transaction_hash"),
            label="deployment transaction hash",
        )
        _parse_hex32(
            source_bridge.get("deployment_receipt_block_hash"),
            label="deployment receipt block hash",
        )
        _require_exact_positive_u64(
            source_bridge.get("deployment_receipt_block_number"),
            label="deployment receipt block number",
        )
        _parse_hex32(
            source_bridge.get("deployment_receipt_block_receipts_root"),
            label="deployment receipt block receiptsRoot",
        )
        chain = source_bridge.get("chain")
        if chain not in ("eth", "bsc"):
            return False
        evidence = _load_evidence_module(
            SCCP_DOMAIN_ETH if chain == "eth" else SCCP_DOMAIN_BSC
        )
        bridge_address = _hex(
            evidence.parse_evm_address(
                source_bridge.get("bridge_address"),
                label="source bridge address",
            )
        )
        receipt_address = _hex(
            evidence.parse_evm_address(
                source_bridge.get("deployment_receipt_contract_address"),
                label="deployment receipt contract address",
            )
        )
    except (argparse.ArgumentTypeError, AttributeError, TypeError, ValueError):
        return False
    return receipt_address == bridge_address


def _validate_source_summary(summary: dict[str, Any]) -> None:
    source_bridge = summary.get("source_bridge")
    if not isinstance(source_bridge, dict):
        raise ValueError("source bridge evidence is required")
    domain = source_bridge.get("domain")
    if type(domain) is not int or domain not in EXPECTED_RPC_CHAIN_IDS:
        raise ValueError("source domain must be an EVM-family SCCP lane")
    expected_chain = "eth" if domain == SCCP_DOMAIN_ETH else "bsc"
    if source_bridge.get("chain") != expected_chain:
        raise ValueError("source chain metadata must match domain")
    rpc_chain_id = source_bridge.get("rpc_chain_id")
    if type(rpc_chain_id) is not int:
        raise ValueError("EVM source RPC chain id metadata must be an integer")
    expected_rpc_chain_id = EXPECTED_RPC_CHAIN_IDS[domain]
    if rpc_chain_id != expected_rpc_chain_id:
        raise ValueError(
            f"EVM source RPC chain id metadata must be {expected_rpc_chain_id} "
            f"for {expected_chain}"
        )
    if source_bridge.get("expected_rpc_chain_id") != expected_rpc_chain_id:
        raise ValueError("expected source RPC chain id metadata must match the lane")

    evidence = _load_evidence_module(domain)
    bridge_address = _summary_address(
        source_bridge,
        "bridge_address",
        label="source bridge address",
    )
    bridge_code_hash = _summary_hex32(
        source_bridge,
        "bridge_code_hash",
        label="source bridge code hash",
    )
    bridge_runtime = _summary_runtime_bytes(
        evidence,
        source_bridge,
        "bridge_runtime_bytecode_hex",
        label="source bridge runtime bytecode",
    )
    if evidence.runtime_bytecode_hash(bridge_runtime) != bridge_code_hash:
        raise ValueError(
            "source bridge runtime bytecode hash must match bridge_code_hash"
        )
    if (
        source_bridge.get("expected_source_bridge_code_hash_matches") is True
        and source_bridge.get("expected_source_bridge_code_hash")
        != _hex(bridge_code_hash)
    ):
        raise ValueError(
            "expected source bridge code hash metadata must match bridge_code_hash"
        )

    if source_bridge.get("deployment_receipt_status") != "0x1":
        raise ValueError("source deployment receipt status must be 0x1")
    deployment_transaction_hash = _summary_hex32(
        source_bridge,
        "deployment_transaction_hash",
        label="deployment transaction hash",
    )
    deployment_receipt_contract_address = _summary_address(
        source_bridge,
        "deployment_receipt_contract_address",
        label="deployment receipt contract address",
    )
    if deployment_receipt_contract_address != bridge_address:
        raise ValueError(
            "deployment receipt contract address metadata must match source bridge"
        )
    if source_bridge.get("deployment_receipt_block_code_hash_matches") is not True:
        raise ValueError(
            "deployment receipt block code hash metadata must be verified"
        )
    if source_bridge.get("deployment_receipt_block_hash_matches") is not True:
        raise ValueError("deployment receipt block hash metadata must be verified")
    if source_bridge.get("deployment_receipt_block_receipts_root_verified") is not True:
        raise ValueError(
            "deployment receipt block receiptsRoot metadata must be verified"
        )
    deployment_receipt_block_hash = _summary_hex32(
        source_bridge,
        "deployment_receipt_block_hash",
        label="deployment receipt block hash",
    )
    deployment_receipt_block_number = _require_exact_positive_u64(
        source_bridge.get("deployment_receipt_block_number"),
        label="deployment receipt block number",
    )
    deployment_receipt_block_receipts_root = _summary_hex32(
        source_bridge,
        "deployment_receipt_block_receipts_root",
        label="deployment receipt block receiptsRoot",
    )

    source_args = summary.get("_source_args")
    if source_args is None:
        raise ValueError("source TOML arguments are required")
    if _require_exact_u32(
        getattr(source_args, "source_domain", None),
        label="source_domain",
    ) != domain:
        raise ValueError("source TOML domain metadata must match source bridge")
    if _require_exact_u32(
        getattr(source_args, "target_domain", None),
        label="target_domain",
    ) != SCCP_DOMAIN_SORA:
        raise ValueError("source TOML target domain metadata must be SORA")
    if (
        _source_arg_bytes(
            source_args,
            "bridge_address",
            label="source TOML bridge address",
            byte_length=20,
        )
        != bridge_address
    ):
        raise ValueError("source TOML bridge address must match source bridge")
    if (
        _source_arg_bytes(
            source_args,
            "source_bridge_emitter_code_hash",
            label="source TOML bridge code hash",
            byte_length=32,
        )
        != bridge_code_hash
    ):
        raise ValueError("source TOML bridge code hash must match source bridge")
    if _source_arg_runtime_bytes(source_args) != bridge_runtime:
        raise ValueError("source TOML bridge runtime bytecode must match source bridge")
    if (
        _source_arg_bytes(
            source_args,
            "deployment_transaction_hash",
            label="source TOML deployment transaction hash",
            byte_length=32,
        )
        != deployment_transaction_hash
    ):
        raise ValueError(
            "source TOML deployment transaction hash must match source bridge"
        )
    if (
        _source_arg_bytes(
            source_args,
            "deployment_receipt_contract_address",
            label="source TOML deployment receipt contract address",
            byte_length=20,
        )
        != deployment_receipt_contract_address
    ):
        raise ValueError(
            "source TOML deployment receipt contract address must match source bridge"
        )
    if (
        _source_arg_bytes(
            source_args,
            "deployment_receipt_block_hash",
            label="source TOML deployment receipt block hash",
            byte_length=32,
        )
        != deployment_receipt_block_hash
    ):
        raise ValueError(
            "source TOML deployment receipt block hash must match source bridge"
        )
    if (
        _require_exact_positive_u64(
            getattr(source_args, "deployment_receipt_block_number", None),
            label="source TOML deployment receipt block number",
        )
        != deployment_receipt_block_number
    ):
        raise ValueError(
            "source TOML deployment receipt block number must match source bridge"
        )
    if (
        _source_arg_bytes(
            source_args,
            "deployment_receipt_block_receipts_root",
            label="source TOML deployment receipt block receiptsRoot",
            byte_length=32,
        )
        != deployment_receipt_block_receipts_root
    ):
        raise ValueError(
            "source TOML deployment receipt block receiptsRoot must match source bridge"
        )

    source_records = summary.get("source_records")
    if not isinstance(source_records, dict):
        raise ValueError("source record evidence is required")
    material_hash = _summary_hex32(
        source_records,
        "source_verifier_material_hash",
        label="source verifier material hash",
    )
    deployment_hash = _summary_hex32(
        source_records,
        "source_adapter_engine_deployment_hash",
        label="source adapter engine deployment hash",
    )
    expected_material_hash, expected_deployment_hash = _source_record_hashes(
        evidence,
        domain,
        source_args,
    )
    if material_hash != expected_material_hash:
        raise ValueError(
            "source verifier material hash metadata must match canonical inputs"
        )
    if deployment_hash != expected_deployment_hash:
        raise ValueError(
            "source adapter engine deployment hash metadata must match canonical inputs"
        )

    source_args_expected_material_hash = _source_arg_optional_bytes(
        source_args,
        "expected_source_verifier_material_hash",
        label="expected source verifier material hash",
        byte_length=32,
    )
    source_args_expected_deployment_hash = _source_arg_optional_bytes(
        source_args,
        "expected_source_adapter_engine_deployment_hash",
        label="expected source adapter engine deployment hash",
        byte_length=32,
    )
    if source_records.get("expected_source_verifier_material_hash_matches") is True:
        if source_args_expected_material_hash != expected_material_hash:
            raise ValueError(
                "expected source verifier material hash argument must match "
                "canonical inputs"
            )
        if (
            source_records.get("expected_source_verifier_material_hash")
            != _hex(expected_material_hash)
        ):
            raise ValueError(
                "expected source verifier material hash metadata must match "
                "canonical inputs"
            )
    if (
        source_records.get("expected_source_adapter_engine_deployment_hash_matches")
        is True
    ):
        if source_args_expected_deployment_hash != expected_deployment_hash:
            raise ValueError(
                "expected source adapter engine deployment hash argument must match "
                "canonical inputs"
            )
        if (
            source_records.get("expected_source_adapter_engine_deployment_hash")
            != _hex(expected_deployment_hash)
        ):
            raise ValueError(
                "expected source adapter engine deployment hash metadata must match "
                "canonical inputs"
            )


def _toml_prerequisites(summary: dict[str, Any]) -> list[str]:
    source_bridge = summary.get("source_bridge")
    if not isinstance(source_bridge, dict):
        return ["source bridge evidence"]
    missing: list[str] = []
    if source_bridge.get("expected_rpc_chain_id_matches") is not True:
        missing.append("--expected-rpc-chain-id")
    if source_bridge.get("expected_source_bridge_code_hash_matches") is not True:
        missing.append("--expected-source-bridge-code-hash")
    if source_bridge.get("deployment_receipt_status") == "0x1":
        if source_bridge.get("deployment_receipt_block_hash_matches") is not True:
            missing.append("deployment receipt block hash verification")
        if source_bridge.get("deployment_receipt_block_receipts_root_verified") is not True:
            missing.append("deployment receipt block receiptsRoot verification")
        if source_bridge.get("deployment_receipt_block_code_hash_matches") is not True:
            missing.append("deployment receipt block code hash verification")
        if (
            source_bridge.get("deployment_receipt_block_hash_matches") is True
            and source_bridge.get("deployment_receipt_block_receipts_root_verified")
            is True
            and source_bridge.get("deployment_receipt_block_code_hash_matches") is True
            and not _source_bridge_deployment_receipt_is_verified(source_bridge)
        ):
            missing.append("--deployment-transaction-hash")
    elif not _source_bridge_deployment_receipt_is_verified(source_bridge):
        missing.append("--deployment-transaction-hash")
    source_records = summary.get("source_records")
    if not isinstance(source_records, dict):
        missing.append("source component hashes")
    elif source_records.get("expected_source_verifier_material_hash_matches") is not True:
        missing.append("--expected-source-verifier-material-hash")
    elif (
        source_records.get("expected_source_adapter_engine_deployment_hash_matches")
        is not True
    ):
        missing.append("--expected-source-adapter-engine-deployment-hash")
    return missing


def render_offline_toml(summary: dict[str, Any]) -> str:
    """Render source material and deployment TOML from pinned live evidence."""

    missing = _toml_prerequisites(summary)
    if missing:
        raise ValueError("TOML output requires " + ", ".join(missing))
    _validate_source_summary(summary)
    args = summary["_source_args"]
    evidence = _load_evidence_module(args.source_domain)
    rendered = evidence.render_toml(args)
    source_bridge = summary["source_bridge"]
    comments = [
        "# sccp_evm_source_rpc_chain_id = "
        + json.dumps(str(source_bridge["rpc_chain_id"])),
        "# sccp_evm_source_bridge_address = "
        + json.dumps(str(source_bridge["bridge_address"])),
        "# sccp_evm_source_bridge_runtime_code_hash = "
        + json.dumps(str(source_bridge["bridge_code_hash"])),
        "# sccp_evm_source_bridge_runtime_bytecode_hex = "
        + json.dumps(str(source_bridge["bridge_runtime_bytecode_hex"])),
    ]
    comments.extend(
        [
            "# sccp_evm_source_deployment_transaction_hash = "
            + json.dumps(str(source_bridge["deployment_transaction_hash"])),
            "# sccp_evm_source_deployment_receipt_status = "
            + json.dumps(str(source_bridge["deployment_receipt_status"])),
            "# sccp_evm_source_deployment_contract_address = "
            + json.dumps(str(source_bridge["deployment_receipt_contract_address"])),
            "# sccp_evm_source_deployment_block_hash = "
            + json.dumps(str(source_bridge["deployment_receipt_block_hash"])),
            "# sccp_evm_source_deployment_block_number = "
            + json.dumps(str(source_bridge["deployment_receipt_block_number"])),
            "# sccp_evm_source_deployment_block_receipts_root = "
            + json.dumps(str(source_bridge["deployment_receipt_block_receipts_root"])),
        ]
    )
    existing_keys = set()
    for line in rendered.splitlines():
        stripped = line.strip()
        if not stripped.startswith("#") or "=" not in stripped:
            continue
        key, _value = stripped[1:].split("=", 1)
        existing_keys.add(key.strip())
    missing_comments = [
        comment
        for comment in comments
        if comment[1:].split("=", 1)[0].strip() not in existing_keys
    ]
    if not missing_comments:
        return rendered
    return "\n".join([*missing_comments, rendered])


def collect_live_evidence(
    args: argparse.Namespace,
    *,
    opener: Urlopen = urllib.request.urlopen,
) -> dict[str, Any]:
    """Collect live EVM-family source evidence and return a JSON summary."""

    block_tag = parse_block_tag(args.block_tag)
    summary: dict[str, Any] = {
        "rpc_url": args.rpc_url,
        "read_only": True,
        "block_tag": block_tag,
    }
    source_bridge = collect_source_bridge_evidence(
        args.rpc_url,
        domain=args.domain,
        bridge_address=args.bridge_address,
        block_tag=block_tag,
        deployment_transaction_hash=args.deployment_transaction_hash,
        opener=opener,
        timeout=args.timeout,
    )
    canonical_rpc_chain_id = EXPECTED_RPC_CHAIN_IDS[args.domain]
    expected_rpc_chain_id = args.expected_rpc_chain_id
    if expected_rpc_chain_id is None:
        expected_rpc_chain_id = canonical_rpc_chain_id
    elif expected_rpc_chain_id != canonical_rpc_chain_id:
        raise ValueError(
            "--expected-rpc-chain-id must match the canonical "
            f"{source_bridge['chain']} mainnet chain id {canonical_rpc_chain_id}"
        )
    if expected_rpc_chain_id != source_bridge["rpc_chain_id"]:
        raise ValueError(
            "--expected-rpc-chain-id does not match eth_chainId for "
            f"{source_bridge['chain']} lane: expected {expected_rpc_chain_id}, "
            f"got {source_bridge['rpc_chain_id']}"
        )
    source_bridge["expected_rpc_chain_id"] = expected_rpc_chain_id
    source_bridge["expected_rpc_chain_id_matches"] = True

    if args.expected_source_bridge_code_hash is not None:
        if _hex(args.expected_source_bridge_code_hash) != source_bridge["bridge_code_hash"]:
            raise ValueError(
                "--expected-source-bridge-code-hash does not match live source "
                "bridge runtime bytecode: "
                f"expected {_hex(args.expected_source_bridge_code_hash)}, "
                f"got {source_bridge['bridge_code_hash']}"
            )
        source_bridge["expected_source_bridge_code_hash"] = _hex(
            args.expected_source_bridge_code_hash
        )
        source_bridge["expected_source_bridge_code_hash_matches"] = True
    summary["source_bridge"] = source_bridge

    if _component_hash_args_present(args):
        source_args = _source_args(args, source_bridge)
        evidence = _load_evidence_module(args.domain)
        material_hash, deployment_hash = _source_record_hashes(
            evidence,
            args.domain,
            source_args,
        )
        source_records = {
            "source_verifier_material_hash": _hex(material_hash),
            "source_adapter_engine_deployment_hash": _hex(deployment_hash),
            "expected_source_verifier_material_hash_matches": (
                args.expected_source_verifier_material_hash == material_hash
            ),
            "expected_source_adapter_engine_deployment_hash_matches": (
                args.expected_source_adapter_engine_deployment_hash == deployment_hash
            ),
        }
        if args.expected_source_verifier_material_hash is not None:
            if args.expected_source_verifier_material_hash != material_hash:
                raise ValueError(
                    "--expected-source-verifier-material-hash does not match "
                    "live source bridge evidence: "
                    f"expected {_hex(args.expected_source_verifier_material_hash)}, "
                    f"got {_hex(material_hash)}"
                )
            source_records["expected_source_verifier_material_hash"] = _hex(
                args.expected_source_verifier_material_hash
            )
        if args.expected_source_adapter_engine_deployment_hash is not None:
            if args.expected_source_adapter_engine_deployment_hash != deployment_hash:
                raise ValueError(
                    "--expected-source-adapter-engine-deployment-hash does not "
                    "match live source bridge evidence: "
                    f"expected {_hex(args.expected_source_adapter_engine_deployment_hash)}, "
                    f"got {_hex(deployment_hash)}"
                )
            source_records["expected_source_adapter_engine_deployment_hash"] = _hex(
                args.expected_source_adapter_engine_deployment_hash
            )
        summary["source_records"] = source_records
        summary["_source_args"] = source_args
    summary["offline_evidence_args"] = _offline_args(args, source_bridge)
    if not _toml_prerequisites(summary):
        offline_toml = render_offline_toml(summary)
        summary["offline_toml_sha256"] = hashlib.sha256(
            offline_toml.encode("utf-8")
        ).hexdigest()
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Read EVM-family source bridge bytecode through JSON-RPC and "
            "recompute ETH/BSC source material records."
        ),
    )
    parser.add_argument("--rpc-url", required=True, help="Ethereum/BSC JSON-RPC URL.")
    parser.add_argument("--domain", required=True, type=parse_domain, help="eth or bsc.")
    parser.add_argument("--bridge-address", required=True, help="Source bridge address.")
    parser.add_argument(
        "--expected-rpc-chain-id",
        type=_parse_rpc_chain_id,
        help=(
            "Expected eth_chainId. Defaults to the canonical mainnet id for "
            "--domain: eth=1, bsc=56; explicit values must match that id."
        ),
    )
    parser.add_argument(
        "--expected-source-bridge-code-hash",
        type=lambda value: _parse_hex32(value, label="expected source bridge code hash"),
        help="Expected non-zero deployed source bridge runtime code hash.",
    )
    parser.add_argument(
        "--deployment-transaction-hash",
        type=lambda value: _parse_hex32(value, label="deployment transaction hash"),
        help="Deployment transaction hash to read and verify as status 0x1; required for TOML.",
    )
    parser.add_argument(
        "--source-trust-anchor-hash",
        type=lambda value: _parse_hex32(value, label="source trust anchor hash"),
        help="Live source trust-anchor deployment hash for source TOML.",
    )
    parser.add_argument(
        "--consensus-verifier-hash",
        type=lambda value: _parse_hex32(value, label="consensus verifier hash"),
        help="Live source consensus-verifier deployment hash for source TOML.",
    )
    parser.add_argument(
        "--message-inclusion-verifier-hash",
        type=lambda value: _parse_hex32(value, label="message inclusion verifier hash"),
        help="Live source message-inclusion verifier hash for source TOML.",
    )
    parser.add_argument(
        "--finality-policy-hash",
        type=lambda value: _parse_hex32(value, label="finality policy hash"),
        help="Live source finality-policy deployment hash for source TOML.",
    )
    parser.add_argument(
        "--adapter-verifier-vk-hash",
        type=lambda value: _parse_hex32(value, label="adapter verifier vk hash"),
        help="Optional audit check for the canonical source-adapter verifier key hash.",
    )
    parser.add_argument(
        "--deployment-receipt-hash",
        type=lambda value: _parse_hex32(value, label="deployment receipt hash"),
        help="Governed source-adapter deployment receipt hash for source TOML.",
    )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected source verifier material hash",
        ),
        help="Expected governed source verifier material record hash.",
    )
    parser.add_argument(
        "--expected-source-adapter-engine-deployment-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected source adapter engine deployment hash",
        ),
        help="Expected governed source-adapter deployment record hash.",
    )
    parser.add_argument(
        "--block-tag",
        default="latest",
        type=parse_block_tag,
        help=(
            "JSON-RPC block tag for eth_getCode. Must be latest, safe, "
            "finalized, or a positive canonical lowercase 0x block number. "
            "Defaults to latest."
        ),
    )
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Print verified source material/deployment TOML instead of JSON.",
    )
    parser.add_argument("--timeout", type=float, default=15.0, help="HTTP timeout in seconds.")
    return parser


def _public_summary(summary: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in summary.items() if not key.startswith("_")}


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        summary = collect_live_evidence(args)
        if args.toml:
            sys.stdout.write(render_offline_toml(summary))
            return 0
    except (RuntimeError, ValueError, argparse.ArgumentTypeError) as exc:
        parser.error(str(exc))
    print(json.dumps(_public_summary(summary), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
