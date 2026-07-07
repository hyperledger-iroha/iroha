#!/usr/bin/env python3
"""Collect read-only SCCP TON destination evidence from TON Center v3."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import ipaddress
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from html import unescape as html_unescape
from pathlib import Path
from typing import Any, Callable
from urllib.parse import unquote


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from path_safety import first_symlinked_existing_path_component  # noqa: E402
import sccp_ton_destination_evidence as evidence  # noqa: E402


Urlopen = Callable[..., Any]
TON_ACCOUNT_STATES_MAX_RESPONSE_BYTES = 1024 * 1024
TON_ACCOUNT_STATES_MAX_ERROR_BYTES = 4096
PUBLIC_SUMMARY_FIELDS = (
    "source_domain",
    "domain",
    "chain",
    "verifier_plan",
    "verifier_identity",
    "verifier_code_hash",
    "anchor_id",
    "destination_binding_key",
    "destination_binding_hash",
    "expected_destination_binding_hash_matches",
    "code_boc_root_hash",
    "code_boc_hash_matches",
    "code_boc_base64",
    "code_boc_base64_sha256",
    "account_status",
    "source_verifier_material_hash",
    "source_adapter_engine_deployment_hash",
    "route_allowlist_id",
    "route_allowlist_hash",
    "expected_route_allowlist_hash",
    "expected_route_allowlist_hash_matches",
    "route_canary",
    "verifier_contract_address",
    "account_address",
    "account_state_hash",
    "last_transaction_lt",
    "last_transaction_hash",
    "code_boc_present",
    "expected_verifier_code_hash_matches",
    "expected_account_state_hash_matches",
    "destination_toml_ready",
    "full_toml_ready",
    "toml_ready",
    "offline_evidence_args",
    "offline_toml_sha256",
)


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _summary_ready(summary: dict[str, Any], field: str) -> bool:
    return summary.get(field) is True


def _parse_hex32(value: str, *, label: str) -> bytes:
    if type(value) is not str:
        raise argparse.ArgumentTypeError(f"{label} must be canonical lowercase 0x hex")
    return evidence.parse_hex_bytes(value, label=label, byte_length=32)


def _parse_ton_raw_address(value: str, *, label: str) -> str:
    if type(value) is not str:
        raise argparse.ArgumentTypeError(f"{label} must be workchain:account_hex")
    return evidence.normalize_ton_raw_address(value, label=label)


def _optional_live_bytes32_arg(
    args: argparse.Namespace,
    name: str,
    *,
    label: str,
) -> bytes | None:
    value = getattr(args, name, None)
    if value is None:
        return None
    if type(value) not in (bytes, bytearray):
        raise ValueError(f"{label} must be bytes")
    raw = bytes(value)
    if len(raw) != 32:
        raise ValueError(f"{label} must be 32 bytes")
    if not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def _decode_hash_text(value: Any, *, label: str) -> bytes:
    if type(value) is not str:
        raise RuntimeError(f"{label} must be a string")
    if value != value.strip():
        raise RuntimeError(f"{label} must not contain whitespace")
    text = value
    if not text:
        raise RuntimeError(f"{label} must be non-empty")
    if text.startswith("0X"):
        raise RuntimeError(f"{label} must use lowercase 0x prefix")
    has_hex_prefix = text.startswith("0x")
    hex_text = text[2:] if has_hex_prefix else text
    if has_hex_prefix and (
        len(hex_text) != 64
        or any(symbol not in "0123456789abcdefABCDEF" for symbol in hex_text)
    ):
        raise RuntimeError(f"{label} must be 32-byte hex")
    if has_hex_prefix and hex_text != hex_text.lower():
        raise RuntimeError(f"{label} must use lowercase hex")
    if len(hex_text) == 64 and all(symbol in "0123456789abcdef" for symbol in hex_text):
        raw = bytes.fromhex(hex_text)
    elif len(hex_text) == 64 and all(
        symbol in "0123456789abcdefABCDEF" for symbol in hex_text
    ):
        raise RuntimeError(f"{label} must use lowercase hex")
    else:
        padded = text + "=" * ((4 - len(text) % 4) % 4)
        try:
            raw = base64.b64decode(padded, altchars=b"-_", validate=True)
        except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, binascii.Error, ValueError):
            raise RuntimeError(f"{label} must be 32-byte hex or base64") from None
        canonical_base64 = base64.b64encode(raw).decode("ascii")
        canonical_base64url = base64.urlsafe_b64encode(raw).decode("ascii")
        if text not in {
            canonical_base64,
            canonical_base64.rstrip("="),
            canonical_base64url,
            canonical_base64url.rstrip("="),
        }:
            raise RuntimeError(f"{label} must be canonical base64 or base64url")
    if len(raw) != 32:
        raise RuntimeError(f"{label} must decode to 32 bytes")
    if not any(raw):
        raise RuntimeError(f"{label} must not be zero")
    return raw


def _account_states_url(api_url: str) -> str:
    if (
        type(api_url) is not str
        or api_url != api_url.strip()
        or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in api_url)
    ):
        raise ValueError("--api-url must be an exact http(s) URL")
    parsed = urllib.parse.urlparse(api_url)
    if parsed.scheme not in ("http", "https") or not parsed.netloc:
        raise ValueError("--api-url must be an http(s) URL")
    if parsed.username is not None or parsed.password is not None:
        raise ValueError(
            "--api-url must not include credentials; use --api-key or --api-key-file"
        )
    if parsed.params or parsed.query or parsed.fragment:
        raise ValueError("--api-url must not include params, query, or fragment")
    host = parsed.hostname
    if host is None:
        raise ValueError("--api-url must be an http(s) URL")
    if parsed.scheme == "http" and not _api_host_is_loopback(host):
        raise ValueError("--api-url must use HTTPS unless it is loopback HTTP")
    if parsed.scheme == "https" and _api_host_is_non_public_dns(host):
        raise ValueError("--api-url HTTPS host must use public DNS")
    path = parsed.path.rstrip("/")
    if path.endswith("/api/v3/accountStates"):
        return urllib.parse.urlunparse(parsed._replace(path=path))
    return urllib.parse.urlunparse(parsed._replace(path=path + "/api/v3/accountStates"))


def _api_host_is_loopback(host: str) -> bool:
    normalized = host.strip("[]").lower()
    try:
        return ipaddress.ip_address(normalized).is_loopback
    except ValueError:
        return normalized == "localhost" or normalized.endswith(".localhost")


def _api_host_is_non_public_dns(host: str) -> bool:
    normalized = host.strip("[]").lower()
    try:
        ipaddress.ip_address(normalized)
    except ValueError:
        pass
    else:
        return True
    labels = normalized.split(".")
    return (
        _api_host_is_loopback(normalized)
        or normalized.endswith(".local")
        or "." not in normalized
        or any(
            label == ""
            or not all(ch.isascii() for ch in label)
            or not label[0].isalnum()
            or not label[-1].isalnum()
            or len(label) > 63
            or any(not (ch.isalnum() or ch == "-") for ch in label)
            for label in labels
        )
    )


def _reject_runtime_file_symlink_path(path: Path) -> None:
    if first_symlinked_existing_path_component(path) is not None:
        raise ValueError("runtime file must not be a symlink")


def _read_runtime_text_file(path: Path, *, error_message: str) -> str:
    try:
        _reject_runtime_file_symlink_path(path)
    except (OSError, ValueError):
        raise ValueError(error_message) from None
    if not path.is_file():
        raise ValueError(error_message) from None
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        raise ValueError(error_message) from None


def _read_runtime_text_file_arg(value: Any, *, error_message: str) -> str:
    if type(value) is not str:
        raise ValueError(error_message) from None
    return _read_runtime_text_file(
        Path(value).expanduser(),
        error_message=error_message,
    )


def _read_api_key(args: argparse.Namespace) -> str | None:
    if args.api_key is not None and args.api_key_file is not None:
        raise ValueError("--api-key and --api-key-file cannot both be supplied")
    if args.api_key is not None:
        return _api_key_token(args.api_key, label="--api-key")
    if args.api_key_file is None:
        return None
    token = _read_runtime_text_file_arg(
        args.api_key_file,
        error_message="--api-key-file cannot be read",
    )
    return _api_key_token(
        token.rstrip("\r\n"),
        label="--api-key-file",
    )


def _api_key_token(value: Any, *, label: str) -> str:
    if type(value) is not str or not value:
        raise ValueError(f"{label} must contain a non-empty token")
    if not value.isascii():
        raise ValueError(f"{label} token must be ASCII")
    if value != value.strip() or any(
        ch.isspace() or ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value
    ):
        raise ValueError(
            f"{label} token must not contain whitespace or control characters"
        )
    return value


def _json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    decoded: dict[str, Any] = {}
    for key, value in pairs:
        if key in decoded:
            raise ValueError("TON accountStates returned duplicate JSON keys")
        decoded[key] = value
    return decoded


def _http_get_json(
    url: str,
    params: list[tuple[str, str]],
    *,
    opener: Urlopen,
    timeout: float,
    api_key: str | None,
) -> Any:
    query = urllib.parse.urlencode(params)
    request = urllib.request.Request(
        url + ("&" if "?" in url else "?") + query,
        headers={"Accept": "application/json"},
        method="GET",
    )
    if api_key is not None:
        api_key = _api_key_token(api_key, label="api_key")
        request.add_header("X-API-Key", api_key)
    try:
        with opener(request, timeout=timeout) as response:
            raw = response.read(TON_ACCOUNT_STATES_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(
            f"TON accountStates failed with HTTP {exc.code}"
        ) from None
    except urllib.error.URLError:
        raise RuntimeError("TON accountStates request failed") from None
    if len(raw) > TON_ACCOUNT_STATES_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            "TON accountStates response exceeds "
            f"{TON_ACCOUNT_STATES_MAX_RESPONSE_BYTES} bytes"
        )
    try:
        decoded = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_json_object_without_duplicate_keys,
        )
    except UnicodeDecodeError:
        raise RuntimeError("TON accountStates returned invalid JSON") from None
    except json.JSONDecodeError:
        raise RuntimeError("TON accountStates returned invalid JSON") from None
    except ValueError as exc:
        if str(exc) == "TON accountStates returned duplicate JSON keys":
            raise RuntimeError("TON accountStates returned duplicate JSON keys") from None
        raise RuntimeError("TON accountStates returned invalid JSON") from None
    if type(decoded) is not dict:
        raise RuntimeError("TON accountStates returned a non-object response")
    if decoded.get("error") is not None:
        raise RuntimeError("TON accountStates returned error response")
    ok = decoded.get("ok")
    if "ok" in decoded and (type(ok) is not bool or ok is not True):
        raise RuntimeError("TON accountStates returned unsuccessful response")
    return decoded


def _account_from_response(decoded: dict[str, Any]) -> dict[str, Any]:
    has_direct_accounts = "accounts" in decoded
    result = decoded.get("result")
    if result is not None and type(result) is not dict:
        raise RuntimeError("TON accountStates result must be an object")
    has_wrapped_accounts = type(result) is dict and "accounts" in result
    if has_direct_accounts and has_wrapped_accounts:
        raise RuntimeError("TON accountStates returned ambiguous accounts")
    accounts = decoded.get("accounts")
    if not has_direct_accounts and has_wrapped_accounts:
        accounts = result["accounts"]
    if type(accounts) is not list:
        raise RuntimeError("TON accountStates response must contain accounts")
    if len(accounts) != 1:
        raise RuntimeError("TON accountStates must return exactly one account")
    account = accounts[0]
    if type(account) is not dict:
        raise RuntimeError("TON accountStates account must be an object")
    return account


def _positive_decimal(value: Any, *, label: str) -> str:
    if (
        type(value) is not str
        or not value.isascii()
        or not value.isdecimal()
        or (len(value) > 1 and value.startswith("0"))
    ):
        raise RuntimeError(f"{label} must be a decimal string") from None
    if int(value, 10) <= 0:
        raise RuntimeError(f"{label} must be positive") from None
    return value


def _exact_live_string(live: dict[str, Any], field: str, *, label: str) -> str:
    value = live.get(field)
    if type(value) is not str or not value or value != value.strip():
        raise ValueError(f"{label} must be an exact non-empty string") from None
    return value


def collect_live_evidence(
    api_url: str,
    *,
    verifier_contract_address: str,
    api_key: str | None = None,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    verifier = evidence.normalize_ton_raw_address(
        verifier_contract_address,
        label="verifier contract address",
    )
    decoded = _http_get_json(
        _account_states_url(api_url),
        [("address", verifier), ("include_boc", "true")],
        opener=opener,
        timeout=timeout,
        api_key=api_key,
    )
    account = _account_from_response(decoded)
    account_address = account.get("address")
    if type(account_address) is not str or not account_address:
        raise RuntimeError("TON accountStates account address must be present")
    try:
        normalized_account_address = evidence.normalize_ton_raw_address(
            account_address,
            label="accountStates account address",
        )
    except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, ValueError):
        raise RuntimeError(
            "TON accountStates account address must be a canonical raw address"
        ) from None
    if normalized_account_address != verifier:
        raise RuntimeError("TON accountStates account address does not match verifier contract")
    status = account.get("status")
    if status != "active":
        raise RuntimeError("TON verifier contract account must be active")
    code_hash = _decode_hash_text(account.get("code_hash"), label="code_hash")
    account_state_hash = _decode_hash_text(
        account.get("account_state_hash"),
        label="account_state_hash",
    )
    last_transaction_hash = _decode_hash_text(
        account.get("last_transaction_hash"),
        label="last_transaction_hash",
    )
    last_transaction_lt = _positive_decimal(
        account.get("last_transaction_lt"),
        label="last_transaction_lt",
    )
    code_boc = account.get("code_boc")
    if type(code_boc) is not str or not code_boc or code_boc != code_boc.strip():
        raise RuntimeError("TON verifier account code_boc must be present")
    try:
        code_boc_bytes = evidence.parse_code_boc_base64(code_boc, label="code_boc")
        code_boc_hash = evidence.ton_boc_single_root_hash(code_boc_bytes)
    except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, ValueError):
        raise RuntimeError("TON verifier account code_boc is invalid") from None
    if code_boc_hash != code_hash:
        raise RuntimeError(
            "TON verifier account code_boc root hash does not match code_hash: "
            f"expected {_hex(code_hash)}, got {_hex(code_boc_hash)}"
        )
    return {
        "verifier_contract_address": verifier,
        "account_address": normalized_account_address,
        "account_status": status,
        "account_state_hash": _hex(account_state_hash),
        "last_transaction_lt": last_transaction_lt,
        "last_transaction_hash": _hex(last_transaction_hash),
        "code_boc_present": True,
        "code_boc_base64": base64.b64encode(code_boc_bytes).decode("ascii"),
        "code_boc_root_hash": _hex(code_boc_hash),
        "code_boc_hash_matches": True,
        "verifier_code_hash": _hex(code_hash),
    }


def _validate_live_evidence(
    live: dict[str, Any],
) -> tuple[dict[str, Any], bytes, bytes, bytes]:
    if type(live) is not dict:
        raise ValueError("TON live evidence must be an object")
    try:
        verifier = evidence.normalize_ton_raw_address(
            _exact_live_string(
                live,
                "verifier_contract_address",
                label="verifier_contract_address",
            ),
            label="verifier contract address",
        )
    except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, ValueError):
        raise ValueError("TON live verifier address metadata is invalid") from None
    try:
        account_address = evidence.normalize_ton_raw_address(
            _exact_live_string(
                live,
                "account_address",
                label="account_address",
            ),
            label="account address",
        )
    except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, ValueError):
        raise ValueError("TON live account address metadata is invalid") from None
    if account_address != verifier:
        raise ValueError("TON live account address must match verifier contract")
    if live.get("account_status") != "active":
        raise ValueError("TON live account status metadata must be active")
    if live.get("code_boc_present") is not True:
        raise ValueError("TON live code BoC presence metadata must be true")
    if live.get("code_boc_hash_matches") is not True:
        raise ValueError("TON live code BoC hash match metadata must be true")

    account_state_hash = _parse_hex32(
        _exact_live_string(live, "account_state_hash", label="account_state_hash"),
        label="account_state_hash",
    )
    last_transaction_hash = _parse_hex32(
        _exact_live_string(
            live,
            "last_transaction_hash",
            label="last_transaction_hash",
        ),
        label="last_transaction_hash",
    )
    try:
        last_transaction_lt = _positive_decimal(
            live.get("last_transaction_lt"),
            label="last_transaction_lt",
        )
    except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, ValueError):
        raise ValueError("TON live last_transaction_lt metadata is invalid") from None
    verifier_code_hash = _parse_hex32(
        _exact_live_string(live, "verifier_code_hash", label="verifier_code_hash"),
        label="verifier_code_hash",
    )
    code_boc_root_hash = _parse_hex32(
        _exact_live_string(live, "code_boc_root_hash", label="code_boc_root_hash"),
        label="code_boc_root_hash",
    )
    if code_boc_root_hash != verifier_code_hash:
        raise ValueError("TON live code BoC root hash must match verifier_code_hash")
    code_boc_base64 = _exact_live_string(
        live,
        "code_boc_base64",
        label="code_boc_base64",
    )
    try:
        code_boc_bytes = evidence.parse_code_boc_base64(
            code_boc_base64,
            label="code_boc_base64",
        )
        derived_code_boc_root_hash = evidence.ton_boc_single_root_hash(code_boc_bytes)
    except (argparse.ArgumentTypeError, SystemExit, RuntimeError, TypeError, ValueError):
        raise ValueError("TON live code BoC base64 metadata is invalid") from None
    if derived_code_boc_root_hash != code_boc_root_hash:
        raise ValueError("TON live code BoC bytes must match code_boc_root_hash")

    normalized = {
        "verifier_contract_address": verifier,
        "account_address": account_address,
        "account_status": "active",
        "account_state_hash": _hex(account_state_hash),
        "last_transaction_lt": last_transaction_lt,
        "last_transaction_hash": _hex(last_transaction_hash),
        "code_boc_present": True,
        "code_boc_base64": base64.b64encode(code_boc_bytes).decode("ascii"),
        "code_boc_root_hash": _hex(code_boc_root_hash),
        "code_boc_hash_matches": True,
        "verifier_code_hash": _hex(verifier_code_hash),
    }
    return normalized, verifier_code_hash, account_state_hash, code_boc_bytes


def _destination_args_from_validated_live(
    args: argparse.Namespace,
    live: dict[str, Any],
    verifier_code_hash: bytes,
    code_boc_bytes: bytes,
) -> argparse.Namespace:
    code_boc_base64 = _exact_live_string(
        live,
        "code_boc_base64",
        label="code_boc_base64",
    )
    return argparse.Namespace(
        verifier_contract_address=_exact_live_string(
            live,
            "verifier_contract_address",
            label="verifier_contract_address",
        ),
        verifier_code_hash=verifier_code_hash,
        route_allowlist_hash=args.route_allowlist_hash,
        source_verifier_material_hash=args.source_verifier_material_hash,
        source_adapter_engine_deployment_hash=args.source_adapter_engine_deployment_hash,
        route_canary_evidence_hash=getattr(args, "route_canary_evidence_hash", None),
        expected_destination_binding_hash=args.expected_destination_binding_hash,
        account_status=_exact_live_string(live, "account_status", label="account_status"),
        account_state_hash=_parse_hex32(
            _exact_live_string(live, "account_state_hash", label="account_state_hash"),
            label="account_state_hash",
        ),
        last_transaction_lt=_positive_decimal(
            live.get("last_transaction_lt"),
            label="last_transaction_lt",
        ),
        last_transaction_hash=_parse_hex32(
            _exact_live_string(
                live,
                "last_transaction_hash",
                label="last_transaction_hash",
            ),
            label="last_transaction_hash",
        ),
        verifier_code_boc_root_hash=_parse_hex32(
            _exact_live_string(live, "code_boc_root_hash", label="code_boc_root_hash"),
            label="code_boc_root_hash",
        ),
        verifier_code_boc_base64=code_boc_bytes,
        verifier_code_boc_base64_text=code_boc_base64,
        verifier_code_boc_hash_matches=True,
    )


def _destination_args_from_live(args: argparse.Namespace, live: dict[str, Any]) -> argparse.Namespace:
    live, verifier_code_hash, _account_state_hash, code_boc_bytes = (
        _validate_live_evidence(live)
    )
    return _destination_args_from_validated_live(
        args,
        live,
        verifier_code_hash,
        code_boc_bytes,
    )


def _offline_args_from_summary(
    args: argparse.Namespace,
    live: dict[str, Any],
    summary: dict[str, Any],
) -> list[str]:
    account_status = _exact_live_string(live, "account_status", label="account_status")
    if account_status != "active":
        raise ValueError("TON live account status metadata must be active") from None
    offline = [
        "--verifier-contract-address",
        _exact_live_string(
            live,
            "verifier_contract_address",
            label="verifier_contract_address",
        ),
        "--verifier-code-hash",
        _hex(
            _parse_hex32(
                _exact_live_string(
                    live,
                    "verifier_code_hash",
                    label="verifier_code_hash",
                ),
                label="verifier_code_hash",
            )
        ),
        "--verifier-code-boc-base64",
        _exact_live_string(live, "code_boc_base64", label="code_boc_base64"),
    ]
    offline.extend(
        [
            "--account-status",
            account_status,
            "--account-state-hash",
            _hex(
                _parse_hex32(
                    _exact_live_string(
                        live,
                        "account_state_hash",
                        label="account_state_hash",
                    ),
                    label="account_state_hash",
                )
            ),
            "--last-transaction-lt",
            _positive_decimal(
                live.get("last_transaction_lt"),
                label="last_transaction_lt",
            ),
            "--last-transaction-hash",
            _hex(
                _parse_hex32(
                    _exact_live_string(
                        live,
                        "last_transaction_hash",
                        label="last_transaction_hash",
                    ),
                    label="last_transaction_hash",
                )
            ),
        ]
    )
    if summary.get("expected_destination_binding_hash_matches") is True:
        offline.extend(
            [
                "--expected-destination-binding-hash",
                _hex(
                    _parse_hex32(
                        _exact_live_string(
                            summary,
                            "destination_binding_hash",
                            label="destination binding hash",
                        ),
                        label="destination binding hash",
                    )
                ),
            ]
        )
    if "route_allowlist_hash" in summary:
        offline.extend(
            [
                "--route-allowlist-hash",
                _hex(
                    _parse_hex32(
                        _exact_live_string(
                            summary,
                            "route_allowlist_hash",
                            label="route allowlist hash",
                        ),
                        label="route allowlist hash",
                    )
                ),
                "--source-verifier-material-hash",
                _hex(args.source_verifier_material_hash),
                "--source-adapter-engine-deployment-hash",
                _hex(args.source_adapter_engine_deployment_hash),
            ]
        )
        route_canary = summary.get("route_canary")
        if type(route_canary) is dict:
            offline.extend(
                [
                    "--route-canary-evidence-hash",
                    _hex(
                        _parse_hex32(
                            _exact_live_string(
                                route_canary,
                                "evidence_hash",
                                label="route canary evidence hash",
                            ),
                            label="route canary evidence hash",
                        )
                    ),
                ]
            )
    return offline


def _summary(args: argparse.Namespace, live: dict[str, Any]) -> dict[str, Any]:
    live, verifier_code_hash, account_state_hash, code_boc_bytes = (
        _validate_live_evidence(live)
    )
    destination_args = _destination_args_from_validated_live(
        args,
        live,
        verifier_code_hash,
        code_boc_bytes,
    )
    destination_binding_hash = evidence.ton_destination_binding_hash()
    expected_destination_binding_hash = _optional_live_bytes32_arg(
        args,
        "expected_destination_binding_hash",
        label="--expected-destination-binding-hash",
    )
    destination_args.expected_destination_binding_hash = expected_destination_binding_hash
    expected_binding_matches = (
        expected_destination_binding_hash == destination_binding_hash
    )
    expected_code_hash = _optional_live_bytes32_arg(
        args,
        "expected_verifier_code_hash",
        label="--expected-verifier-code-hash",
    )
    if expected_code_hash is not None and expected_code_hash != verifier_code_hash:
        raise ValueError(
            "--expected-verifier-code-hash does not match live TON verifier code hash: "
            f"expected {_hex(expected_code_hash)}, got {live['verifier_code_hash']}"
        )
    expected_state_hash = _optional_live_bytes32_arg(
        args,
        "expected_account_state_hash",
        label="--expected-account-state-hash",
    )
    if expected_state_hash is not None and expected_state_hash != account_state_hash:
        raise ValueError(
            "--expected-account-state-hash does not match live TON account state: "
            f"expected {_hex(expected_state_hash)}, got {live['account_state_hash']}"
        )
    summary = evidence._json_summary(
        destination_args,
        destination_binding_hash,
        expected_binding_matches,
    )
    summary.update(live)
    summary["expected_verifier_code_hash_matches"] = expected_code_hash is not None
    summary["expected_account_state_hash_matches"] = expected_state_hash is not None
    destination_toml_ready = _summary_ready(summary, "toml_ready")
    summary["destination_toml_ready"] = destination_toml_ready
    full_toml_ready = bool(
        destination_toml_ready
        and expected_code_hash is not None
        and expected_state_hash is not None
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


def render_toml(args: argparse.Namespace, live: dict[str, Any]) -> str:
    if args.expected_verifier_code_hash is None:
        raise ValueError("--toml requires --expected-verifier-code-hash")
    if args.expected_account_state_hash is None:
        raise ValueError("--toml requires --expected-account-state-hash")
    if getattr(args, "route_canary_evidence_hash", None) is None:
        raise ValueError("--toml requires --route-canary-evidence-hash")
    summary = _summary(args, live)
    if summary.get("toml_ready") is not True:
        raise ValueError("TON live evidence is not fully pinned for TOML output")
    live, verifier_code_hash, _account_state_hash, code_boc_bytes = (
        _validate_live_evidence(live)
    )
    rendered = evidence.render_toml(
        _destination_args_from_validated_live(
            args,
            live,
            verifier_code_hash,
            code_boc_bytes,
        ),
        evidence.ton_destination_binding_hash(),
    )
    account_state_hash = _hex(
        _parse_hex32(
            _exact_live_string(live, "account_state_hash", label="account_state_hash"),
            label="account_state_hash",
        )
    )
    last_transaction_lt = _positive_decimal(
        live.get("last_transaction_lt"),
        label="last_transaction_lt",
    )
    last_transaction_hash = _hex(
        _parse_hex32(
            _exact_live_string(
                live,
                "last_transaction_hash",
                label="last_transaction_hash",
            ),
            label="last_transaction_hash",
        )
    )
    verifier_code_hash_text = _hex(
        _parse_hex32(
            _exact_live_string(live, "verifier_code_hash", label="verifier_code_hash"),
            label="verifier_code_hash",
        )
    )
    code_boc_root_hash = _hex(
        _parse_hex32(
            _exact_live_string(live, "code_boc_root_hash", label="code_boc_root_hash"),
            label="code_boc_root_hash",
        )
    )
    comments = [
        "# sccp_ton_account_status = "
        + json.dumps(
            _exact_live_string(live, "account_status", label="account_status")
        ),
        "# sccp_ton_account_state_hash = " + json.dumps(account_state_hash),
        "# sccp_ton_last_transaction_lt = " + json.dumps(last_transaction_lt),
        "# sccp_ton_last_transaction_hash = " + json.dumps(last_transaction_hash),
        "# sccp_ton_code_hash = " + json.dumps(verifier_code_hash_text),
        "# sccp_ton_code_boc_root_hash = " + json.dumps(code_boc_root_hash),
        "# sccp_ton_code_boc_base64 = "
        + json.dumps(
            _exact_live_string(live, "code_boc_base64", label="code_boc_base64")
        ),
        "# sccp_ton_code_boc_hash_matches = "
        + json.dumps("true" if live.get("code_boc_hash_matches") is True else "false"),
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
        description="Collect SCCP TON destination rollout evidence from TON Center v3.",
    )
    parser.add_argument(
        "--api-url",
        required=True,
        help=(
            "TON Center v3 API root or accountStates endpoint. Must be HTTPS "
            "with a public DNS host or loopback HTTP, and must not include "
            "credentials, params, query, or fragment."
        ),
    )
    parser.add_argument(
        "--verifier-contract-address",
        required=True,
        type=lambda value: _parse_ton_raw_address(
            value,
            label="verifier contract address",
        ),
        help="Deployed TON verifier contract raw address.",
    )
    parser.add_argument("--api-key", help="Runtime-only TON Center API key.")
    parser.add_argument("--api-key-file", help="Runtime-only file containing the TON Center API key.")
    parser.add_argument(
        "--expected-account-state-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected account state hash",
        ),
        help="Pinned live TON account_state_hash for the verifier contract.",
    )
    parser.add_argument(
        "--expected-verifier-code-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected verifier code hash",
        ),
        help="Pinned live TON verifier code hash.",
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: _parse_hex32(
            value,
            label="route allowlist hash",
        ),
        help="Governed TON route allowlist hash.",
    )
    parser.add_argument(
        "--source-verifier-material-hash",
        type=lambda value: _parse_hex32(
            value,
            label="source verifier material hash",
        ),
        help="Source verifier material record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--source-adapter-engine-deployment-hash",
        type=lambda value: _parse_hex32(
            value,
            label="source adapter engine deployment hash",
        ),
        help="Source adapter engine deployment record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: _parse_hex32(
            value,
            label="route canary evidence hash",
        ),
        help="Post-deploy route canary evidence hash for all-lanes TOML metadata.",
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: _parse_hex32(
            value,
            label="expected destination binding hash",
        ),
        help="Expected canonical SORA -> TON destination binding hash.",
    )
    parser.add_argument("--timeout", type=float, default=15.0, help="HTTP timeout in seconds.")
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Render production TOML after all expected live pins match.",
    )
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "secret key",
    "secret-key",
    "secret_key",
    "private key",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer",
    "authorization",
    "access key",
    "access-key",
    "access_key",
    "api key",
    "api-key",
    "api_key",
    "client secret",
    "client-secret",
    "client_secret",
    "credential",
    "credentials",
    "auth header",
    "auth-header",
    "auth_header",
    "mnemonic",
    "recovery phrase",
    "recovery-phrase",
    "recovery_phrase",
    "seed phrase",
    "seed-phrase",
    "seed_phrase",
    "signing key",
    "signing-key",
    "signing_key",
    "session",
    "token",
)


def _decoded_public_blocker_text(value: str) -> str:
    decoded = value
    for _decode_pass in range(max(1, len(value))):
        next_decoded = unquote(html_unescape(decoded))
        if next_decoded == decoded:
            break
        decoded = next_decoded
    return decoded


def _decoded_cli_error_text_issue(value: str) -> bool:
    decoded = _decoded_public_blocker_text(value)
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in decoded):
        return True
    if not decoded.isascii():
        return True
    return any(character in "|`<>" for character in decoded)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, (OSError, SystemExit)):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    if not text.isascii():
        return fallback
    if _decoded_cli_error_text_issue(text):
        return fallback
    normalized_text = _decoded_public_blocker_text(text).casefold()
    if any(marker in normalized_text for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def _public_summary(summary: dict[str, Any]) -> dict[str, Any]:
    return {
        field: summary[field]
        for field in PUBLIC_SUMMARY_FIELDS
        if field in summary
    }


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        api_key = _read_api_key(args)
        live = collect_live_evidence(
            args.api_url,
            verifier_contract_address=args.verifier_contract_address,
            api_key=api_key,
            timeout=args.timeout,
        )
        if args.toml:
            print(render_toml(args, live), end="")
        else:
            print(
                json.dumps(
                    _public_summary(_summary(args, live)),
                    sort_keys=True,
                    indent=2,
                )
            )
    except (
        argparse.ArgumentTypeError,
        OSError,
        SystemExit,
        RuntimeError,
        TypeError,
        ValueError,
    ) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP TON live evidence collection failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
