#!/usr/bin/env python3
"""Collect read-only SCCP Solana destination evidence from JSON-RPC."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import sccp_solana_destination_evidence as evidence  # noqa: E402


Urlopen = Callable[..., Any]
SOLANA_JSON_RPC_MAX_RESPONSE_BYTES = 1024 * 1024
SOLANA_JSON_RPC_MAX_ERROR_BYTES = 4096

UPGRADEABLE_LOADER_ID = "BPFLoaderUpgradeab1e11111111111111111111111"
UPGRADEABLE_LOADER_PROGRAM_TAG = 2
UPGRADEABLE_LOADER_PROGRAMDATA_TAG = 3
PROGRAMDATA_METADATA_LEN = 45


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _summary_ready(summary: dict[str, Any], field: str, *, fallback: str | None = None) -> bool:
    if field in summary:
        return summary[field] is True
    return fallback is not None and summary.get(fallback) is True


def _parse_hex32(value: str, *, label: str) -> bytes:
    return evidence.parse_hex_bytes(value, label=label, byte_length=32)


def _parse_positive_u64(value: str, *, label: str) -> int:
    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    text = value
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    parsed = int(text, 10)
    if parsed <= 0 or parsed > 0xFFFF_FFFF_FFFF_FFFF:
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    return parsed


def _encode_solana_base58(raw: bytes) -> str:
    if not isinstance(raw, (bytes, bytearray)):
        raise ValueError("Solana public key bytes must be bytes")
    value = bytes(raw)
    if len(value) != 32:
        raise ValueError("Solana public key must be 32 bytes")
    numeric = int.from_bytes(value, "big")
    encoded = ""
    while numeric:
        numeric, remainder = divmod(numeric, 58)
        encoded = evidence.SOLANA_BASE58_ALPHABET[remainder] + encoded
    leading_zeros = len(value) - len(value.lstrip(b"\x00"))
    return "1" * leading_zeros + (encoded or "")


def _http_error_detail(exc: urllib.error.HTTPError) -> str:
    raw = exc.read(SOLANA_JSON_RPC_MAX_ERROR_BYTES + 1)
    truncated = len(raw) > SOLANA_JSON_RPC_MAX_ERROR_BYTES
    detail = raw[:SOLANA_JSON_RPC_MAX_ERROR_BYTES].decode("utf-8", "replace")
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
            raw = response.read(SOLANA_JSON_RPC_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        detail = _http_error_detail(exc)
        raise RuntimeError(
            f"JSON-RPC {method} failed with HTTP {exc.code}: {detail}"
        ) from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"JSON-RPC {method} request failed: {exc.reason}") from exc
    if len(raw) > SOLANA_JSON_RPC_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            f"JSON-RPC {method} response exceeds "
            f"{SOLANA_JSON_RPC_MAX_RESPONSE_BYTES} bytes"
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
    error = decoded.get("error")
    if error is not None:
        raise RuntimeError(f"JSON-RPC {method} error: {error}")
    if "result" not in decoded:
        raise RuntimeError(f"JSON-RPC {method} returned no result")
    return decoded["result"]


def _positive_context_slot(result: dict[str, Any], *, method: str) -> int:
    context = result.get("context")
    if not isinstance(context, dict):
        raise RuntimeError(f"{method} returned no context slot")
    slot = context.get("slot")
    if type(slot) is not int or slot <= 0:
        raise RuntimeError(f"{method} context slot must be a positive integer")
    return slot


def _account_info(
    rpc_url: str,
    address: str,
    *,
    commitment: str,
    opener: Urlopen,
    timeout: float,
) -> tuple[dict[str, Any], int]:
    result = _json_rpc(
        rpc_url,
        "getAccountInfo",
        [
            address,
            {
                "encoding": "base64",
                "commitment": commitment,
            },
        ],
        opener=opener,
        timeout=timeout,
    )
    if not isinstance(result, dict):
        raise RuntimeError("getAccountInfo returned a non-object result")
    context_slot = _positive_context_slot(result, method="getAccountInfo")
    value = result.get("value")
    if value is None:
        raise RuntimeError(f"Solana account {address} does not exist")
    if not isinstance(value, dict):
        raise RuntimeError("getAccountInfo value must be an object")
    return value, context_slot


def _account_data(account: dict[str, Any], *, label: str) -> bytes:
    data = account.get("data")
    if (
        not isinstance(data, list)
        or len(data) != 2
        or not isinstance(data[0], str)
        or data[1] != "base64"
    ):
        raise RuntimeError(f"{label} account data must use base64 encoding")
    try:
        raw = base64.b64decode(data[0], validate=True)
    except (ValueError, binascii.Error) as exc:
        raise RuntimeError(f"{label} account data is invalid base64") from exc
    if base64.b64encode(raw).decode("ascii") != data[0]:
        raise RuntimeError(f"{label} account data must be canonical base64")
    return raw


def _require_upgradeable_loader_account(
    account: dict[str, Any],
    *,
    label: str,
    executable: bool,
) -> bytes:
    owner = account.get("owner")
    if owner != UPGRADEABLE_LOADER_ID:
        raise RuntimeError(f"{label} account owner must be the BPF upgradeable loader")
    if account.get("executable") is not executable:
        expected = "executable" if executable else "not executable"
        raise RuntimeError(f"{label} account must be {expected}")
    return _account_data(account, label=label)


def _parse_upgradeable_program_account(data: bytes) -> str:
    if len(data) != 36:
        raise RuntimeError("Solana verifier program account must be exactly 36 bytes")
    tag = int.from_bytes(data[:4], "little")
    if tag != UPGRADEABLE_LOADER_PROGRAM_TAG:
        raise RuntimeError("Solana verifier account must be an upgradeable program")
    programdata_address = data[4:36]
    if not any(programdata_address):
        raise RuntimeError("Solana verifier programdata address must not be zero")
    return _encode_solana_base58(programdata_address)


def _parse_programdata_account(data: bytes) -> tuple[int, bytes | None, bytes, bytes]:
    if len(data) < PROGRAMDATA_METADATA_LEN:
        raise RuntimeError("Solana verifier programdata account is too short")
    tag = int.from_bytes(data[:4], "little")
    if tag != UPGRADEABLE_LOADER_PROGRAMDATA_TAG:
        raise RuntimeError("Solana verifier programdata account must be ProgramData")
    slot = int.from_bytes(data[4:12], "little")
    authority_tag = data[12]
    if authority_tag == 0:
        authority = None
    elif authority_tag == 1:
        authority = data[13:45]
        if not any(authority):
            raise RuntimeError("Solana verifier upgrade authority must not be zero when present")
    else:
        raise RuntimeError("Solana verifier programdata authority tag must be 0 or 1")
    if slot <= 0:
        raise RuntimeError("Solana verifier programdata slot must be positive")
    metadata = data[:PROGRAMDATA_METADATA_LEN]
    program_bytes = data[PROGRAMDATA_METADATA_LEN:]
    if not program_bytes or not any(program_bytes):
        raise RuntimeError("Solana verifier program bytes must not be empty or all zero")
    if not program_bytes.startswith(evidence.SOLANA_BPF_ELF_MAGIC):
        raise RuntimeError("Solana verifier program bytes must be a BPF ELF executable")
    return slot, authority, metadata, program_bytes


def collect_live_evidence(
    rpc_url: str,
    *,
    verifier_program_id: str,
    commitment: str = "finalized",
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    if commitment not in {"finalized", "confirmed"}:
        raise ValueError("Solana JSON-RPC commitment must be finalized or confirmed")
    program_id = evidence.normalize_solana_program_id(
        verifier_program_id,
        label="verifier program id",
    )
    program_account, program_context_slot = _account_info(
        rpc_url,
        program_id,
        commitment=commitment,
        opener=opener,
        timeout=timeout,
    )
    program_data = _require_upgradeable_loader_account(
        program_account,
        label="verifier program",
        executable=True,
    )
    programdata_address = _parse_upgradeable_program_account(program_data)
    if programdata_address == program_id:
        raise RuntimeError("Solana verifier ProgramData account must differ from program id")
    programdata_account, programdata_context_slot = _account_info(
        rpc_url,
        programdata_address,
        commitment=commitment,
        opener=opener,
        timeout=timeout,
    )
    programdata_data = _require_upgradeable_loader_account(
        programdata_account,
        label="verifier programdata",
        executable=False,
    )
    slot, upgrade_authority, programdata_metadata, program_bytes = (
        _parse_programdata_account(programdata_data)
    )
    if upgrade_authority is not None:
        raise RuntimeError("Solana verifier program must be immutable; upgrade authority is still set")
    if program_context_slot < slot:
        raise RuntimeError(
            "Solana verifier program read context slot must be at or after "
            "the ProgramData deployment slot"
        )
    if programdata_context_slot < slot:
        raise RuntimeError(
            "Solana verifier programdata read context slot must be at or after "
            "the ProgramData deployment slot"
        )
    code_hash = evidence.solana_verifier_program_code_hash(program_bytes)
    return {
        "verifier_program_id": program_id,
        "rpc_commitment": commitment,
        "programdata_address": programdata_address,
        "programdata_slot": str(slot),
        "program_account_context_slot": str(program_context_slot),
        "programdata_account_context_slot": str(programdata_context_slot),
        "program_owner": UPGRADEABLE_LOADER_ID,
        "programdata_owner": UPGRADEABLE_LOADER_ID,
        "program_immutable": True,
        "program_account_data_len": str(len(program_data)),
        "program_account_data_base64": base64.b64encode(program_data).decode(
            "ascii"
        ),
        "programdata_metadata_blake2b256": _hex(
            hashlib.blake2b(programdata_metadata, digest_size=32).digest()
        ),
        "programdata_metadata_base64": base64.b64encode(programdata_metadata).decode(
            "ascii"
        ),
        "program_bytes_len": len(program_bytes),
        "programdata_executable_base64": base64.b64encode(program_bytes).decode(
            "ascii"
        ),
        "verifier_code_hash": _hex(code_hash),
    }


def _live_positive_u64(live: dict[str, Any], field: str, *, label: str) -> int:
    value = live.get(field)
    if type(value) is int:
        parsed = value
    elif (
        isinstance(value, str)
        and value.isascii()
        and value.isdecimal()
        and (len(value) == 1 or not value.startswith("0"))
    ):
        parsed = int(value, 10)
    else:
        raise ValueError(f"{label} must be a positive decimal")
    if parsed <= 0 or parsed > 0xFFFF_FFFF_FFFF_FFFF:
        raise ValueError(f"{label} must be a positive decimal")
    return parsed


def _live_base64_bytes(live: dict[str, Any], field: str, *, label: str) -> bytes:
    value = live.get(field)
    if not isinstance(value, str) or not value or value != value.strip():
        raise ValueError(f"{label} must be present")
    try:
        raw = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise ValueError(f"{label} must be base64") from exc
    if base64.b64encode(raw).decode("ascii") != value:
        raise ValueError(f"{label} must be canonical base64")
    return raw


def _validate_live_evidence(live: dict[str, Any]) -> tuple[dict[str, Any], bytes, bytes]:
    if not isinstance(live, dict):
        raise ValueError("Solana live evidence must be an object")

    commitment = live.get("rpc_commitment")
    if commitment not in {"finalized", "confirmed"}:
        raise ValueError("Solana live RPC commitment metadata must be finalized or confirmed")

    try:
        program_id = evidence.normalize_solana_program_id(
            str(live.get("verifier_program_id", "")),
            label="verifier program id",
        )
        programdata_address = evidence.normalize_solana_program_id(
            str(live.get("programdata_address", "")),
            label="programdata address",
        )
        verifier_code_hash = _parse_hex32(
            str(live.get("verifier_code_hash", "")),
            label="verifier_code_hash",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc
    if programdata_address == program_id:
        raise ValueError("Solana verifier ProgramData account must differ from program id")

    executable_base64 = live.get("programdata_executable_base64")
    if (
        not isinstance(executable_base64, str)
        or not executable_base64
        or executable_base64 != executable_base64.strip()
    ):
        raise ValueError("Solana ProgramData executable base64 metadata must be present")
    try:
        program_bytes = evidence.parse_program_bytes_base64(
            executable_base64,
            label="Solana ProgramData executable base64 metadata",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc
    derived_code_hash = evidence.solana_verifier_program_code_hash(program_bytes)
    if derived_code_hash != verifier_code_hash:
        raise ValueError("Solana ProgramData executable bytes must match live verifier_code_hash")

    if live.get("program_owner") != UPGRADEABLE_LOADER_ID:
        raise ValueError("Solana verifier program owner must be the BPF upgradeable loader")
    if live.get("programdata_owner") != UPGRADEABLE_LOADER_ID:
        raise ValueError("Solana ProgramData owner must be the BPF upgradeable loader")
    if live.get("program_immutable") is not True:
        raise ValueError("Solana verifier program must be immutable")

    program_account_data_len = _live_positive_u64(
        live,
        "program_account_data_len",
        label="Solana Program account data length metadata",
    )
    if program_account_data_len != 36:
        raise ValueError("Solana Program account data length metadata must be 36 bytes")
    program_bytes_len = _live_positive_u64(
        live,
        "program_bytes_len",
        label="Solana ProgramData executable length metadata",
    )
    if program_bytes_len != len(program_bytes):
        raise ValueError("Solana ProgramData executable length metadata must match bytes")

    programdata_slot = _live_positive_u64(
        live,
        "programdata_slot",
        label="Solana ProgramData slot metadata",
    )
    program_context_slot = _live_positive_u64(
        live,
        "program_account_context_slot",
        label="Solana program account RPC context slot metadata",
    )
    programdata_context_slot = _live_positive_u64(
        live,
        "programdata_account_context_slot",
        label="Solana ProgramData account RPC context slot metadata",
    )
    if program_context_slot < programdata_slot:
        raise ValueError(
            "Solana program account RPC context slot must be at or after "
            "ProgramData deployment slot"
        )
    if programdata_context_slot < programdata_slot:
        raise ValueError(
            "Solana ProgramData account RPC context slot must be at or after "
            "ProgramData deployment slot"
        )

    program_account_data = _live_base64_bytes(
        live,
        "program_account_data_base64",
        label="Solana Program account data base64 metadata",
    )
    if len(program_account_data) != 36:
        raise ValueError(
            "Solana Program account data base64 metadata must decode to 36 bytes"
        )
    program_tag = int.from_bytes(program_account_data[:4], "little")
    if program_tag != UPGRADEABLE_LOADER_PROGRAM_TAG:
        raise ValueError("Solana Program account data metadata must be an upgradeable Program")
    try:
        expected_programdata_raw = evidence.decode_solana_base58(
            programdata_address,
            label="programdata address",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc
    if program_account_data[4:36] != expected_programdata_raw:
        raise ValueError(
            "Solana Program account data metadata must reference the ProgramData account"
        )

    programdata_metadata = _live_base64_bytes(
        live,
        "programdata_metadata_base64",
        label="Solana ProgramData metadata base64 metadata",
    )
    if len(programdata_metadata) != PROGRAMDATA_METADATA_LEN:
        raise ValueError(
            "Solana ProgramData metadata base64 metadata must decode to 45 bytes"
        )
    metadata_hash = _parse_hex32(
        str(live.get("programdata_metadata_blake2b256", "")),
        label="programdata_metadata_blake2b256",
    )
    derived_metadata_hash = hashlib.blake2b(
        programdata_metadata,
        digest_size=32,
    ).digest()
    if derived_metadata_hash != metadata_hash:
        raise ValueError("Solana ProgramData metadata bytes must match metadata hash")
    metadata_tag = int.from_bytes(programdata_metadata[:4], "little")
    if metadata_tag != UPGRADEABLE_LOADER_PROGRAMDATA_TAG:
        raise ValueError("Solana ProgramData metadata must be a ProgramData header")
    metadata_slot = int.from_bytes(programdata_metadata[4:12], "little")
    if metadata_slot != programdata_slot:
        raise ValueError("Solana ProgramData metadata slot must match ProgramData slot")
    if programdata_metadata[12] != 0 or any(programdata_metadata[13:45]):
        raise ValueError("Solana ProgramData metadata must encode no upgrade authority")

    normalized = {
        "verifier_program_id": program_id,
        "rpc_commitment": commitment,
        "programdata_address": programdata_address,
        "programdata_slot": str(programdata_slot),
        "program_account_context_slot": str(program_context_slot),
        "programdata_account_context_slot": str(programdata_context_slot),
        "program_owner": UPGRADEABLE_LOADER_ID,
        "programdata_owner": UPGRADEABLE_LOADER_ID,
        "program_immutable": True,
        "program_account_data_len": str(program_account_data_len),
        "program_account_data_base64": base64.b64encode(program_account_data).decode(
            "ascii"
        ),
        "programdata_metadata_blake2b256": _hex(metadata_hash),
        "programdata_metadata_base64": base64.b64encode(programdata_metadata).decode(
            "ascii"
        ),
        "program_bytes_len": program_bytes_len,
        "programdata_executable_base64": base64.b64encode(program_bytes).decode(
            "ascii"
        ),
        "verifier_code_hash": _hex(verifier_code_hash),
    }
    return normalized, verifier_code_hash, program_bytes


def _destination_args_from_validated_live(
    args: argparse.Namespace,
    live: dict[str, Any],
    verifier_code_hash: bytes,
    program_bytes: bytes,
) -> argparse.Namespace:
    return argparse.Namespace(
        verifier_program_id=live["verifier_program_id"],
        verifier_code_hash=verifier_code_hash,
        verifier_program_bytes_hex=None,
        verifier_program_bytes_base64=program_bytes,
        verifier_program_bytes_file=None,
        route_allowlist_hash=args.route_allowlist_hash,
        source_verifier_material_hash=args.source_verifier_material_hash,
        source_adapter_engine_deployment_hash=args.source_adapter_engine_deployment_hash,
        route_canary_evidence_hash=getattr(args, "route_canary_evidence_hash", None),
        expected_destination_binding_hash=args.expected_destination_binding_hash,
        programdata_address=live["programdata_address"],
        programdata_slot=int(str(live["programdata_slot"]), 10),
        program_account_context_slot=int(
            str(live["program_account_context_slot"]),
            10,
        ),
        programdata_account_context_slot=int(
            str(live["programdata_account_context_slot"]),
            10,
        ),
    )


def _destination_args_from_live(args: argparse.Namespace, live: dict[str, Any]) -> argparse.Namespace:
    validated_live, verifier_code_hash, program_bytes = _validate_live_evidence(live)
    return _destination_args_from_validated_live(
        args,
        validated_live,
        verifier_code_hash,
        program_bytes,
    )


def _summary(args: argparse.Namespace, live: dict[str, Any]) -> dict[str, Any]:
    live, verifier_code_hash, program_bytes = _validate_live_evidence(live)
    destination_args = _destination_args_from_validated_live(
        args,
        live,
        verifier_code_hash,
        program_bytes,
    )
    destination_binding_hash = evidence.solana_destination_binding_hash()
    expected_binding_matches = args.expected_destination_binding_hash == destination_binding_hash
    expected_code_hash = getattr(args, "expected_verifier_code_hash", None)
    if expected_code_hash is not None and expected_code_hash != verifier_code_hash:
        raise ValueError(
            "--expected-verifier-code-hash does not match live Solana verifier program bytes: "
            f"expected {_hex(expected_code_hash)}, got {live['verifier_code_hash']}"
        )
    expected_programdata = getattr(args, "expected_programdata_address", None)
    if expected_programdata is not None and expected_programdata != live["programdata_address"]:
        raise ValueError(
            "--expected-programdata-address does not match live Solana program account: "
            f"expected {expected_programdata}, got {live['programdata_address']}"
        )
    expected_programdata_slot = getattr(args, "expected_programdata_slot", None)
    if (
        expected_programdata_slot is not None
        and str(expected_programdata_slot) != live["programdata_slot"]
    ):
        raise ValueError(
            "--expected-programdata-slot does not match live Solana ProgramData account: "
            f"expected {expected_programdata_slot}, got {live['programdata_slot']}"
        )
    summary = evidence._json_summary(
        destination_args,
        destination_binding_hash,
        expected_binding_matches,
    )
    summary.update(live)
    summary["rpc_commitment_finalized"] = live.get("rpc_commitment") == "finalized"
    summary["expected_verifier_code_hash_matches"] = expected_code_hash is not None
    summary["expected_programdata_address_matches"] = expected_programdata is not None
    summary["expected_programdata_slot_matches"] = expected_programdata_slot is not None
    destination_toml_ready = _summary_ready(
        summary,
        "full_toml_ready",
        fallback="toml_ready",
    )
    summary["destination_toml_ready"] = destination_toml_ready
    full_toml_ready = bool(
        destination_toml_ready
        and summary["rpc_commitment_finalized"]
        and expected_code_hash is not None
        and expected_programdata is not None
        and expected_programdata_slot is not None
    )
    summary["full_toml_ready"] = full_toml_ready
    summary["toml_ready"] = full_toml_ready
    summary["offline_evidence_args"] = _offline_args_from_summary(args, live, summary)
    if summary["full_toml_ready"]:
        offline_toml = evidence.render_toml(destination_args, destination_binding_hash)
        summary["offline_toml_sha256"] = hashlib.sha256(
            offline_toml.encode("utf-8")
        ).hexdigest()
    return summary


def _offline_args_from_summary(
    args: argparse.Namespace,
    live: dict[str, Any],
    summary: dict[str, Any],
) -> list[str]:
    offline = [
        "--verifier-program-id",
        str(live["verifier_program_id"]),
        "--verifier-code-hash",
        str(live["verifier_code_hash"]),
        "--programdata-address",
        str(live["programdata_address"]),
        "--programdata-slot",
        str(live["programdata_slot"]),
        "--program-account-context-slot",
        str(live["program_account_context_slot"]),
        "--programdata-account-context-slot",
        str(live["programdata_account_context_slot"]),
        "--verifier-program-bytes-base64",
        str(live["programdata_executable_base64"]),
    ]
    if summary.get("expected_destination_binding_hash_matches") is True:
        offline.extend(
            [
                "--expected-destination-binding-hash",
                str(summary["destination_binding_hash"]),
            ]
        )
    if "route_allowlist_hash" in summary:
        offline.extend(
            [
                "--route-allowlist-hash",
                str(summary["route_allowlist_hash"]),
                "--source-verifier-material-hash",
                _hex(args.source_verifier_material_hash),
                "--source-adapter-engine-deployment-hash",
                _hex(args.source_adapter_engine_deployment_hash),
            ]
        )
        route_canary = summary.get("route_canary")
        if isinstance(route_canary, dict):
            offline.extend(
                [
                    "--route-canary-evidence-hash",
                    str(route_canary["evidence_hash"]),
                ]
            )
    return offline


def render_toml(args: argparse.Namespace, live: dict[str, Any]) -> str:
    if live.get("rpc_commitment") != "finalized":
        raise ValueError("--toml requires finalized Solana JSON-RPC commitment")
    if args.expected_verifier_code_hash is None:
        raise ValueError("--toml requires --expected-verifier-code-hash")
    if args.expected_programdata_address is None:
        raise ValueError("--toml requires --expected-programdata-address")
    if args.expected_programdata_slot is None:
        raise ValueError("--toml requires --expected-programdata-slot")
    if getattr(args, "route_canary_evidence_hash", None) is None:
        raise ValueError("--toml requires --route-canary-evidence-hash")
    summary = _summary(args, live)
    if summary.get("full_toml_ready") is not True:
        raise ValueError("Solana live evidence is not fully pinned for TOML output")
    live, verifier_code_hash, program_bytes = _validate_live_evidence(live)
    rendered = evidence.render_toml(
        _destination_args_from_validated_live(
            args,
            live,
            verifier_code_hash,
            program_bytes,
        ),
        evidence.solana_destination_binding_hash(),
    )
    comments = [
        "# sccp_solana_rpc_commitment = "
        + json.dumps(str(live["rpc_commitment"])),
        "# sccp_solana_program_owner = "
        + json.dumps(str(live["program_owner"])),
        "# sccp_solana_programdata_owner = "
        + json.dumps(str(live["programdata_owner"])),
        "# sccp_solana_program_immutable = "
        + json.dumps("true" if live["program_immutable"] is True else "false"),
        "# sccp_solana_program_account_data_len = "
        + json.dumps(str(live["program_account_data_len"])),
        "# sccp_solana_program_account_data_base64 = "
        + json.dumps(str(live["program_account_data_base64"])),
        "# sccp_solana_programdata_address = "
        + json.dumps(str(live["programdata_address"])),
        "# sccp_solana_programdata_slot = "
        + json.dumps(str(live["programdata_slot"])),
        "# sccp_solana_expected_programdata_slot = "
        + json.dumps(str(args.expected_programdata_slot)),
        "# sccp_solana_program_account_context_slot = "
        + json.dumps(str(live["program_account_context_slot"])),
        "# sccp_solana_programdata_account_context_slot = "
        + json.dumps(str(live["programdata_account_context_slot"])),
        "# sccp_solana_programdata_metadata_blake2b256 = "
        + json.dumps(str(live["programdata_metadata_blake2b256"])),
        "# sccp_solana_programdata_metadata_base64 = "
        + json.dumps(str(live["programdata_metadata_base64"])),
        "# sccp_solana_programdata_executable_blake2b256 = "
        + json.dumps(str(live["verifier_code_hash"])),
        "# sccp_solana_programdata_executable_base64 = "
        + json.dumps(str(live["programdata_executable_base64"])),
    ]
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


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Collect SCCP Solana destination rollout evidence from Solana JSON-RPC.",
    )
    parser.add_argument("--rpc-url", required=True, help="Solana JSON-RPC endpoint.")
    parser.add_argument(
        "--verifier-program-id",
        required=True,
        type=lambda value: evidence.normalize_solana_program_id(
            value,
            label="verifier program id",
        ),
        help="Deployed Solana verifier program id.",
    )
    parser.add_argument(
        "--commitment",
        default="finalized",
        choices=("finalized", "confirmed"),
        help="Read commitment for getAccountInfo calls. Defaults to finalized.",
    )
    parser.add_argument(
        "--expected-programdata-address",
        type=lambda value: evidence.normalize_solana_program_id(
            value,
            label="expected programdata address",
        ),
        help="Pinned ProgramData account address for the verifier program.",
    )
    parser.add_argument(
        "--expected-programdata-slot",
        type=lambda value: _parse_positive_u64(
            value,
            label="expected programdata slot",
        ),
        help="Pinned positive deployment slot for the verifier ProgramData account.",
    )
    parser.add_argument(
        "--expected-verifier-code-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="expected verifier code hash",
            byte_length=32,
        ),
        help="Pinned BLAKE2b-256 hash of the deployed ProgramData executable bytes.",
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help="Governed Solana route allowlist hash.",
    )
    parser.add_argument(
        "--source-verifier-material-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="source verifier material hash",
            byte_length=32,
        ),
        help="Source verifier material record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--source-adapter-engine-deployment-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="source adapter engine deployment hash",
            byte_length=32,
        ),
        help="Source adapter engine deployment record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="route canary evidence hash",
            byte_length=32,
        ),
        help="Post-deploy route canary evidence hash for all-lanes TOML metadata.",
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="expected destination binding hash",
            byte_length=32,
        ),
        help="Expected canonical SORA -> Solana destination binding hash.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=15.0,
        help="JSON-RPC timeout in seconds.",
    )
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Render production TOML after all expected live pins match.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        live = collect_live_evidence(
            args.rpc_url,
            verifier_program_id=args.verifier_program_id,
            commitment=args.commitment,
            timeout=args.timeout,
        )
        if args.toml:
            print(render_toml(args, live), end="")
        else:
            print(json.dumps(_summary(args, live), sort_keys=True, indent=2))
    except (RuntimeError, ValueError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
