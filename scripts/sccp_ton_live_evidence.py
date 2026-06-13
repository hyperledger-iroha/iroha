#!/usr/bin/env python3
"""Collect read-only SCCP TON destination evidence from TON Center v3."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import sccp_ton_destination_evidence as evidence  # noqa: E402


Urlopen = Callable[..., Any]
TON_ACCOUNT_STATES_MAX_RESPONSE_BYTES = 1024 * 1024
TON_ACCOUNT_STATES_MAX_ERROR_BYTES = 4096


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _summary_ready(summary: dict[str, Any], field: str) -> bool:
    return summary.get(field) is True


def _parse_hex32(value: str, *, label: str) -> bytes:
    return evidence.parse_hex_bytes(value, label=label, byte_length=32)


def _decode_hash_text(value: Any, *, label: str) -> bytes:
    if not isinstance(value, str):
        raise RuntimeError(f"{label} must be a string")
    if value != value.strip():
        raise RuntimeError(f"{label} must not contain whitespace")
    text = value
    if not text:
        raise RuntimeError(f"{label} must be non-empty")
    has_hex_prefix = text.lower().startswith("0x")
    hex_text = text[2:] if has_hex_prefix else text
    if has_hex_prefix and (
        len(hex_text) != 64
        or any(symbol not in "0123456789abcdefABCDEF" for symbol in hex_text)
    ):
        raise RuntimeError(f"{label} must be 32-byte hex")
    if len(hex_text) == 64 and all(symbol in "0123456789abcdefABCDEF" for symbol in hex_text):
        raw = bytes.fromhex(hex_text)
    else:
        padded = text + "=" * ((4 - len(text) % 4) % 4)
        try:
            raw = base64.b64decode(padded, altchars=b"-_", validate=True)
        except (TypeError, binascii.Error, ValueError):
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
    parsed = urllib.parse.urlparse(api_url)
    if parsed.scheme not in ("http", "https") or not parsed.netloc:
        raise ValueError("--api-url must be an http(s) URL")
    if parsed.username is not None or parsed.password is not None:
        raise ValueError(
            "--api-url must not include credentials; use --api-key or --api-key-file"
        )
    if parsed.params or parsed.query or parsed.fragment:
        raise ValueError("--api-url must not include params, query, or fragment")
    path = parsed.path.rstrip("/")
    if path.endswith("/api/v3/accountStates"):
        return urllib.parse.urlunparse(parsed._replace(path=path))
    return urllib.parse.urlunparse(parsed._replace(path=path + "/api/v3/accountStates"))


def _read_api_key(args: argparse.Namespace) -> str | None:
    if args.api_key is not None and args.api_key_file is not None:
        raise ValueError("--api-key and --api-key-file cannot both be supplied")
    if args.api_key is not None:
        return _api_key_token(args.api_key, label="--api-key")
    if args.api_key_file is None:
        return None
    try:
        token = Path(args.api_key_file).expanduser().read_text(encoding="utf-8")
    except OSError:
        raise ValueError("--api-key-file cannot be read") from None
    return _api_key_token(
        token.rstrip("\r\n"),
        label="--api-key-file",
    )


def _api_key_token(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or not value:
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
    if not isinstance(decoded, dict):
        raise RuntimeError("TON accountStates returned a non-object response")
    if decoded.get("error") is not None:
        raise RuntimeError("TON accountStates returned error response")
    return decoded


def _account_from_response(decoded: dict[str, Any]) -> dict[str, Any]:
    accounts = decoded.get("accounts")
    if accounts is None and isinstance(decoded.get("result"), dict):
        accounts = decoded["result"].get("accounts")
    if not isinstance(accounts, list):
        raise RuntimeError("TON accountStates response must contain accounts")
    if len(accounts) != 1:
        raise RuntimeError("TON accountStates must return exactly one account")
    account = accounts[0]
    if not isinstance(account, dict):
        raise RuntimeError("TON accountStates account must be an object")
    return account


def _positive_decimal(value: Any, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value.isascii()
        or not value.isdecimal()
        or (len(value) > 1 and value.startswith("0"))
    ):
        raise RuntimeError(f"{label} must be a decimal string")
    if int(value, 10) <= 0:
        raise RuntimeError(f"{label} must be positive")
    return value


def _exact_live_string(live: dict[str, Any], field: str, *, label: str) -> str:
    value = live.get(field)
    if not isinstance(value, str) or not value or value != value.strip():
        raise ValueError(f"{label} must be an exact non-empty string")
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
    if not isinstance(account_address, str) or not account_address:
        raise RuntimeError("TON accountStates account address must be present")
    try:
        normalized_account_address = evidence.normalize_ton_raw_address(
            account_address,
            label="accountStates account address",
        )
    except argparse.ArgumentTypeError as exc:
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
    if not isinstance(code_boc, str) or not code_boc or code_boc != code_boc.strip():
        raise RuntimeError("TON verifier account code_boc must be present")
    try:
        code_boc_bytes = evidence.parse_code_boc_base64(code_boc, label="code_boc")
        code_boc_hash = evidence.ton_boc_single_root_hash(code_boc_bytes)
    except (argparse.ArgumentTypeError, TypeError, ValueError):
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
    if not isinstance(live, dict):
        raise ValueError("TON live evidence must be an object")
    try:
        verifier = evidence.normalize_ton_raw_address(
            str(live.get("verifier_contract_address", "")),
            label="verifier contract address",
        )
    except argparse.ArgumentTypeError:
        raise ValueError("TON live verifier address metadata is invalid") from None
    try:
        account_address = evidence.normalize_ton_raw_address(
            str(live.get("account_address", "")),
            label="account address",
        )
    except argparse.ArgumentTypeError:
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
        str(live.get("account_state_hash", "")),
        label="account_state_hash",
    )
    last_transaction_hash = _parse_hex32(
        str(live.get("last_transaction_hash", "")),
        label="last_transaction_hash",
    )
    try:
        last_transaction_lt = _positive_decimal(
            live.get("last_transaction_lt"),
            label="last_transaction_lt",
        )
    except RuntimeError:
        raise ValueError("TON live last_transaction_lt metadata is invalid") from None
    verifier_code_hash = _parse_hex32(
        str(live.get("verifier_code_hash", "")),
        label="verifier_code_hash",
    )
    code_boc_root_hash = _parse_hex32(
        str(live.get("code_boc_root_hash", "")),
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
    except (argparse.ArgumentTypeError, TypeError, ValueError):
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
    code_boc_base64 = str(live["code_boc_base64"])
    return argparse.Namespace(
        verifier_contract_address=live["verifier_contract_address"],
        verifier_code_hash=verifier_code_hash,
        route_allowlist_hash=args.route_allowlist_hash,
        source_verifier_material_hash=args.source_verifier_material_hash,
        source_adapter_engine_deployment_hash=args.source_adapter_engine_deployment_hash,
        route_canary_evidence_hash=getattr(args, "route_canary_evidence_hash", None),
        expected_destination_binding_hash=args.expected_destination_binding_hash,
        account_status=str(live["account_status"]),
        account_state_hash=_parse_hex32(
            str(live["account_state_hash"]),
            label="account_state_hash",
        ),
        last_transaction_lt=str(live["last_transaction_lt"]),
        last_transaction_hash=_parse_hex32(
            str(live["last_transaction_hash"]),
            label="last_transaction_hash",
        ),
        verifier_code_boc_root_hash=_parse_hex32(
            str(live["code_boc_root_hash"]),
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
    offline = [
        "--verifier-contract-address",
        str(live["verifier_contract_address"]),
        "--verifier-code-hash",
        str(live["verifier_code_hash"]),
    ]
    code_boc_base64 = live.get("code_boc_base64")
    if isinstance(code_boc_base64, str) and code_boc_base64:
        if code_boc_base64 != code_boc_base64.strip():
            raise ValueError("code_boc_base64 must be an exact non-empty string")
        offline.extend(["--verifier-code-boc-base64", code_boc_base64])
    offline.extend(
        [
            "--account-status",
            str(live["account_status"]),
            "--account-state-hash",
            str(live["account_state_hash"]),
            "--last-transaction-lt",
            str(live["last_transaction_lt"]),
            "--last-transaction-hash",
            str(live["last_transaction_hash"]),
        ]
    )
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
    expected_binding_matches = args.expected_destination_binding_hash == destination_binding_hash
    expected_code_hash = getattr(args, "expected_verifier_code_hash", None)
    if expected_code_hash is not None and expected_code_hash != verifier_code_hash:
        raise ValueError(
            "--expected-verifier-code-hash does not match live TON verifier code hash: "
            f"expected {_hex(expected_code_hash)}, got {live['verifier_code_hash']}"
        )
    expected_state_hash = getattr(args, "expected_account_state_hash", None)
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
    comments = [
        "# sccp_ton_account_status = " + json.dumps(str(live["account_status"])),
        "# sccp_ton_account_state_hash = "
        + json.dumps(str(live["account_state_hash"])),
        "# sccp_ton_last_transaction_lt = "
        + json.dumps(str(live["last_transaction_lt"])),
        "# sccp_ton_last_transaction_hash = "
        + json.dumps(str(live["last_transaction_hash"])),
        "# sccp_ton_code_hash = " + json.dumps(str(live["verifier_code_hash"])),
        "# sccp_ton_code_boc_root_hash = "
        + json.dumps(str(live["code_boc_root_hash"])),
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
            "TON Center v3 API root or accountStates endpoint without credentials, "
            "params, query, or fragment."
        ),
    )
    parser.add_argument(
        "--verifier-contract-address",
        required=True,
        type=lambda value: evidence.normalize_ton_raw_address(
            value,
            label="verifier contract address",
        ),
        help="Deployed TON verifier contract raw address.",
    )
    parser.add_argument("--api-key", help="Runtime-only TON Center API key.")
    parser.add_argument("--api-key-file", help="Runtime-only file containing the TON Center API key.")
    parser.add_argument(
        "--expected-account-state-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="expected account state hash",
            byte_length=32,
        ),
        help="Pinned live TON account_state_hash for the verifier contract.",
    )
    parser.add_argument(
        "--expected-verifier-code-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="expected verifier code hash",
            byte_length=32,
        ),
        help="Pinned live TON verifier code hash.",
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help="Governed TON route allowlist hash.",
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
            print(json.dumps(_summary(args, live), sort_keys=True, indent=2))
    except (OSError, RuntimeError, TypeError, ValueError) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP TON live evidence collection failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
