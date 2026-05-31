#!/usr/bin/env python3
"""Collect read-only SCCP Substrate-family destination evidence from JSON-RPC."""

from __future__ import annotations

import argparse
import base64
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

import sccp_substrate_destination_evidence as evidence  # noqa: E402


Urlopen = Callable[..., Any]
SUBSTRATE_JSON_RPC_MAX_RESPONSE_BYTES = 1024 * 1024
SUBSTRATE_JSON_RPC_MAX_ERROR_BYTES = 4096

RUNTIME_CODE_STORAGE_KEY = "0x3a636f6465"


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _summary_ready(summary: dict[str, Any], field: str) -> bool:
    return summary.get(field) is True


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def _parse_hex32(value: str, *, label: str) -> bytes:
    return evidence.parse_hex_bytes(value, label=label, byte_length=32)


def _parse_nonempty(value: str, *, label: str) -> str:
    if value != value.strip() or not value:
        raise argparse.ArgumentTypeError(f"{label} must be non-empty")
    return value


def _parse_nonnegative_u32(value: str, *, label: str) -> int:
    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a decimal u32")
    text = value
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} must be a decimal u32")
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} must be a decimal u32")
    parsed = int(text, 10)
    if parsed < 0 or parsed > 0xFFFF_FFFF:
        raise argparse.ArgumentTypeError(f"{label} must be a decimal u32")
    return parsed


def _http_error_detail(exc: urllib.error.HTTPError) -> str:
    raw = exc.read(SUBSTRATE_JSON_RPC_MAX_ERROR_BYTES + 1)
    truncated = len(raw) > SUBSTRATE_JSON_RPC_MAX_ERROR_BYTES
    detail = raw[:SUBSTRATE_JSON_RPC_MAX_ERROR_BYTES].decode("utf-8", "replace")
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
            raw = response.read(SUBSTRATE_JSON_RPC_MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as exc:
        detail = _http_error_detail(exc)
        raise RuntimeError(
            f"JSON-RPC {method} failed with HTTP {exc.code}: {detail}"
        ) from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"JSON-RPC {method} request failed: {exc.reason}") from exc
    if len(raw) > SUBSTRATE_JSON_RPC_MAX_RESPONSE_BYTES:
        raise RuntimeError(
            f"JSON-RPC {method} response exceeds "
            f"{SUBSTRATE_JSON_RPC_MAX_RESPONSE_BYTES} bytes"
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


def _rpc_hex_bytes(result: Any, *, method: str, byte_length: int | None = None) -> bytes:
    if not isinstance(result, str):
        raise RuntimeError(f"{method} returned non-string data")
    if result != result.strip():
        raise RuntimeError(f"{method} returned non-canonical hex data")
    if not result.startswith("0x"):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    text = result[2:]
    if any(symbol not in "0123456789abcdef" for symbol in text):
        raise RuntimeError(f"{method} returned non-canonical lowercase 0x hex data")
    if len(text) % 2 != 0:
        raise RuntimeError(f"{method} returned odd-length hex")
    if byte_length is not None and len(text) != byte_length * 2:
        raise RuntimeError(f"{method} must return {byte_length} bytes")
    raw = bytes.fromhex(text)
    if not raw or not any(raw):
        raise RuntimeError(f"{method} returned empty or all-zero data")
    return raw


def _runtime_version_value(value: Any, *, field: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        raise RuntimeError(f"state_getRuntimeVersion {field} must be a u32")
    return value


def _runtime_version(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        raise RuntimeError("state_getRuntimeVersion returned a non-object response")
    spec_name = result.get("specName")
    if (
        not isinstance(spec_name, str)
        or not spec_name
        or spec_name != spec_name.strip()
    ):
        raise RuntimeError("state_getRuntimeVersion specName must be non-empty")
    return {
        "spec_name": spec_name,
        "spec_version": _runtime_version_value(
            result.get("specVersion"),
            field="specVersion",
        ),
        "transaction_version": _runtime_version_value(
            result.get("transactionVersion"),
            field="transactionVersion",
        ),
    }


def runtime_code_hash(runtime_code: bytes) -> bytes:
    """Return the SCCP production hash for finalized Substrate runtime code."""

    if not isinstance(runtime_code, (bytes, bytearray)):
        raise ValueError("runtime code must be bytes")
    raw = bytes(runtime_code)
    if not raw or not any(raw):
        raise ValueError("runtime code must not be empty or all zero")
    return hashlib.blake2b(raw, digest_size=32).digest()


def collect_live_evidence(
    rpc_url: str,
    *,
    domain: int,
    opener: Urlopen = urllib.request.urlopen,
    timeout: float = 15.0,
) -> dict[str, Any]:
    """Collect finalized runtime identity and code evidence from Substrate JSON-RPC."""

    if domain not in evidence.SUBSTRATE_DESTINATION_PROFILES:
        raise ValueError("domain must be a supported Substrate-family SCCP lane")
    finalized_head = _rpc_hex_bytes(
        _json_rpc(
            rpc_url,
            "chain_getFinalizedHead",
            [],
            opener=opener,
            timeout=timeout,
        ),
        method="chain_getFinalizedHead",
        byte_length=32,
    )
    runtime = _runtime_version(
        _json_rpc(
            rpc_url,
            "state_getRuntimeVersion",
            [_hex(finalized_head)],
            opener=opener,
            timeout=timeout,
        )
    )
    profile = evidence.SUBSTRATE_DESTINATION_PROFILES[domain]
    if runtime["spec_name"] != profile["chain"]:
        raise RuntimeError(
            "state_getRuntimeVersion specName does not match selected "
            f"Substrate SCCP destination domain: expected {profile['chain']!r}, "
            f"got {runtime['spec_name']!r}"
        )
    runtime_code = _rpc_hex_bytes(
        _json_rpc(
            rpc_url,
            "state_getStorage",
            [RUNTIME_CODE_STORAGE_KEY, _hex(finalized_head)],
            opener=opener,
            timeout=timeout,
        ),
        method="state_getStorage :code",
    )
    return {
        "domain": domain,
        "chain": profile["chain"],
        "finalized_head": _hex(finalized_head),
        "runtime_code_storage_key": RUNTIME_CODE_STORAGE_KEY,
        "runtime_spec_name": runtime["spec_name"],
        "runtime_spec_version": runtime["spec_version"],
        "runtime_transaction_version": runtime["transaction_version"],
        "runtime_code_len": len(runtime_code),
        "runtime_code_base64": base64.b64encode(runtime_code).decode("ascii"),
        "runtime_code_hash_algorithm": "blake2b-256",
        "verifier_entrypoint": evidence.SUBSTRATE_RUNTIME_VERIFIER_ID,
        "verifier_code_hash": _hex(runtime_code_hash(runtime_code)),
    }


def _exact_live_string(live: dict[str, Any], field: str, *, label: str) -> str:
    value = live.get(field)
    if not isinstance(value, str) or not value or value != value.strip():
        raise ValueError(f"{label} must be an exact non-empty string")
    return value


def _live_hex32(live: dict[str, Any], field: str, *, label: str) -> bytes:
    return _parse_hex32(
        _exact_live_string(live, field, label=label),
        label=label,
    )


def _live_u32(live: dict[str, Any], field: str, *, label: str) -> int:
    value = live.get(field)
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        raise ValueError(f"{label} must be a u32 integer")
    return value


def _validate_live_evidence(live: dict[str, Any]) -> tuple[dict[str, Any], bytes, bytes]:
    if not isinstance(live, dict):
        raise ValueError("Substrate live evidence must be an object")
    domain = live.get("domain")
    if type(domain) is not int or domain not in evidence.SUBSTRATE_DESTINATION_PROFILES:
        raise ValueError("Substrate live domain must be a supported integer lane")
    profile = evidence.SUBSTRATE_DESTINATION_PROFILES[domain]
    if live.get("chain") != profile["chain"]:
        raise ValueError("Substrate live chain metadata must match domain")
    if live.get("runtime_code_storage_key") != RUNTIME_CODE_STORAGE_KEY:
        raise ValueError("Substrate live runtime code storage key metadata is invalid")
    if live.get("runtime_code_hash_algorithm") != "blake2b-256":
        raise ValueError("Substrate live runtime code hash algorithm must be blake2b-256")
    if live.get("verifier_entrypoint") != evidence.SUBSTRATE_RUNTIME_VERIFIER_ID:
        raise ValueError("Substrate live verifier entrypoint metadata is invalid")

    finalized_head = _live_hex32(live, "finalized_head", label="finalized_head")
    runtime_spec_name = _exact_live_string(
        live,
        "runtime_spec_name",
        label="runtime_spec_name",
    )
    if runtime_spec_name != profile["chain"]:
        raise ValueError("Substrate live runtime specName must match domain")
    runtime_spec_version = _live_u32(
        live,
        "runtime_spec_version",
        label="runtime_spec_version",
    )
    runtime_transaction_version = _live_u32(
        live,
        "runtime_transaction_version",
        label="runtime_transaction_version",
    )
    runtime_code_base64 = _exact_live_string(
        live,
        "runtime_code_base64",
        label="runtime_code_base64",
    )
    try:
        runtime_code = evidence.parse_runtime_code_base64(
            runtime_code_base64,
            label="runtime code",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc
    runtime_code_len = live.get("runtime_code_len")
    if type(runtime_code_len) is not int or runtime_code_len <= 0:
        raise ValueError("Substrate live runtime code length must be positive")
    if runtime_code_len != len(runtime_code):
        raise ValueError("Substrate live runtime code length must match bytes")
    verifier_code_hash = _live_hex32(
        live,
        "verifier_code_hash",
        label="verifier_code_hash",
    )
    derived_code_hash = runtime_code_hash(runtime_code)
    if derived_code_hash != verifier_code_hash:
        raise ValueError("Substrate live runtime code bytes must match verifier_code_hash")

    normalized = {
        "domain": domain,
        "chain": profile["chain"],
        "finalized_head": _hex(finalized_head),
        "runtime_code_storage_key": RUNTIME_CODE_STORAGE_KEY,
        "runtime_spec_name": runtime_spec_name,
        "runtime_spec_version": runtime_spec_version,
        "runtime_transaction_version": runtime_transaction_version,
        "runtime_code_len": len(runtime_code),
        "runtime_code_base64": base64.b64encode(runtime_code).decode("ascii"),
        "runtime_code_hash_algorithm": "blake2b-256",
        "verifier_entrypoint": evidence.SUBSTRATE_RUNTIME_VERIFIER_ID,
        "verifier_code_hash": _hex(verifier_code_hash),
    }
    return normalized, verifier_code_hash, runtime_code


def _destination_args_from_validated_live(
    args: argparse.Namespace,
    live: dict[str, Any],
    verifier_code_hash: bytes,
    runtime_code: bytes,
) -> argparse.Namespace:
    return argparse.Namespace(
        domain=live["domain"],
        verifier_entrypoint=evidence.SUBSTRATE_RUNTIME_VERIFIER_ID,
        verifier_code_hash=verifier_code_hash,
        runtime_code_hex=None,
        runtime_code_base64=runtime_code,
        runtime_code_file=None,
        route_allowlist_hash=args.route_allowlist_hash,
        source_verifier_material_hash=args.source_verifier_material_hash,
        source_adapter_engine_deployment_hash=args.source_adapter_engine_deployment_hash,
        route_canary_evidence_hash=getattr(args, "route_canary_evidence_hash", None),
        expected_destination_binding_hash=args.expected_destination_binding_hash,
        finalized_head=_parse_hex32(
            str(live["finalized_head"]),
            label="finalized_head",
        ),
        runtime_spec_name=str(live["runtime_spec_name"]),
        runtime_spec_version=int(live["runtime_spec_version"]),
        runtime_transaction_version=int(live["runtime_transaction_version"]),
    )


def _destination_args_from_live(
    args: argparse.Namespace,
    live: dict[str, Any],
) -> argparse.Namespace:
    live, verifier_code_hash, runtime_code = _validate_live_evidence(live)
    return _destination_args_from_validated_live(
        args,
        live,
        verifier_code_hash,
        runtime_code,
    )


def _offline_args_from_summary(
    args: argparse.Namespace,
    live: dict[str, Any],
    summary: dict[str, Any],
) -> list[str]:
    offline = [
        "--domain",
        str(live["chain"]),
        "--verifier-code-hash",
        str(live["verifier_code_hash"]),
        "--runtime-code-base64",
        str(live["runtime_code_base64"]),
        "--finalized-head",
        str(live["finalized_head"]),
        "--runtime-spec-name",
        str(live["runtime_spec_name"]),
        "--runtime-spec-version",
        str(live["runtime_spec_version"]),
        "--runtime-transaction-version",
        str(live["runtime_transaction_version"]),
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


def _summary(args: argparse.Namespace, live: dict[str, Any]) -> dict[str, Any]:
    live, verifier_code_hash, runtime_code = _validate_live_evidence(live)
    destination_args = _destination_args_from_validated_live(
        args,
        live,
        verifier_code_hash,
        runtime_code,
    )
    destination_binding_hash = evidence.substrate_destination_binding_hash(live["domain"])
    expected_binding_matches = args.expected_destination_binding_hash == destination_binding_hash

    expected_head = getattr(args, "expected_finalized_head", None)
    finalized_head = _parse_hex32(str(live["finalized_head"]), label="finalized_head")
    if expected_head is not None and expected_head != finalized_head:
        raise ValueError(
            "--expected-finalized-head does not match live finalized head: "
            f"expected {_hex(expected_head)}, got {live['finalized_head']}"
        )
    expected_code_hash = getattr(args, "expected_runtime_code_hash", None)
    if expected_code_hash is not None and expected_code_hash != verifier_code_hash:
        raise ValueError(
            "--expected-runtime-code-hash does not match live Substrate runtime "
            f":code: expected {_hex(expected_code_hash)}, got {live['verifier_code_hash']}"
        )
    expected_spec_name = getattr(args, "expected_spec_name", None)
    if expected_spec_name is not None and expected_spec_name != live["runtime_spec_name"]:
        raise ValueError(
            "--expected-spec-name does not match live runtime specName: "
            f"expected {expected_spec_name!r}, got {live['runtime_spec_name']!r}"
        )
    expected_spec_version = getattr(args, "expected_spec_version", None)
    if (
        expected_spec_version is not None
        and expected_spec_version != live["runtime_spec_version"]
    ):
        raise ValueError(
            "--expected-spec-version does not match live runtime specVersion: "
            f"expected {expected_spec_version}, got {live['runtime_spec_version']}"
        )
    expected_transaction_version = getattr(args, "expected_transaction_version", None)
    if (
        expected_transaction_version is not None
        and expected_transaction_version != live["runtime_transaction_version"]
    ):
        raise ValueError(
            "--expected-transaction-version does not match live runtime "
            f"transactionVersion: expected {expected_transaction_version}, "
            f"got {live['runtime_transaction_version']}"
        )

    summary = evidence._json_summary(
        destination_args,
        destination_binding_hash,
        expected_binding_matches,
    )
    summary.update(live)
    summary["expected_finalized_head_matches"] = expected_head is not None
    summary["expected_runtime_code_hash_matches"] = expected_code_hash is not None
    summary["expected_spec_name_matches"] = expected_spec_name is not None
    summary["expected_spec_version_matches"] = expected_spec_version is not None
    summary["expected_transaction_version_matches"] = (
        expected_transaction_version is not None
    )
    summary["toml_ready"] = bool(
        _summary_ready(summary, "toml_ready")
        and expected_head is not None
        and expected_code_hash is not None
        and expected_spec_name is not None
        and expected_spec_version is not None
        and expected_transaction_version is not None
    )
    summary["offline_evidence_args"] = _offline_args_from_summary(args, live, summary)
    if summary["toml_ready"]:
        offline_toml = evidence.render_toml(destination_args, destination_binding_hash)
        summary["offline_toml_sha256"] = hashlib.sha256(
            offline_toml.encode("utf-8")
        ).hexdigest()
    return summary


def render_toml(args: argparse.Namespace, live: dict[str, Any]) -> str:
    if args.expected_finalized_head is None:
        raise ValueError("--toml requires --expected-finalized-head")
    if args.expected_runtime_code_hash is None:
        raise ValueError("--toml requires --expected-runtime-code-hash")
    if args.expected_spec_name is None:
        raise ValueError("--toml requires --expected-spec-name")
    if args.expected_spec_version is None:
        raise ValueError("--toml requires --expected-spec-version")
    if args.expected_transaction_version is None:
        raise ValueError("--toml requires --expected-transaction-version")
    if getattr(args, "route_canary_evidence_hash", None) is None:
        raise ValueError("--toml requires --route-canary-evidence-hash")
    summary = _summary(args, live)
    if summary.get("toml_ready") is not True:
        raise ValueError("Substrate live evidence is not fully pinned for TOML output")
    live, verifier_code_hash, runtime_code = _validate_live_evidence(live)
    rendered = evidence.render_toml(
        _destination_args_from_validated_live(
            args,
            live,
            verifier_code_hash,
            runtime_code,
        ),
        evidence.substrate_destination_binding_hash(live["domain"]),
    )
    comments = [
        "# sccp_substrate_finalized_head = "
        + json.dumps(str(live["finalized_head"])),
        "# sccp_substrate_runtime_spec_name = "
        + json.dumps(str(live["runtime_spec_name"])),
        "# sccp_substrate_runtime_spec_version = "
        + json.dumps(str(live["runtime_spec_version"])),
        "# sccp_substrate_runtime_transaction_version = "
        + json.dumps(str(live["runtime_transaction_version"])),
        "# sccp_substrate_runtime_code_hash = "
        + json.dumps(str(live["verifier_code_hash"])),
        "# sccp_substrate_runtime_code_base64 = "
        + json.dumps(str(live["runtime_code_base64"])),
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
        description=(
            "Collect SCCP Substrate-family destination rollout evidence from "
            "finalized JSON-RPC state."
        ),
    )
    parser.add_argument("--rpc-url", required=True, help="Substrate JSON-RPC URL.")
    parser.add_argument(
        "--domain",
        required=True,
        type=evidence.parse_substrate_domain,
        help="Destination domain: sora-kusama, sora-polkadot, or sora2.",
    )
    parser.add_argument(
        "--expected-finalized-head",
        type=lambda value: _parse_hex32(value, label="expected finalized head"),
        help="Pinned finalized block hash used for runtime reads.",
    )
    parser.add_argument(
        "--expected-runtime-code-hash",
        type=lambda value: _parse_hex32(value, label="expected runtime code hash"),
        help="Pinned BLAKE2b-256 hash of finalized :code runtime bytes.",
    )
    parser.add_argument(
        "--expected-spec-name",
        type=lambda value: _parse_nonempty(value, label="expected spec name"),
        help="Pinned state_getRuntimeVersion specName.",
    )
    parser.add_argument(
        "--expected-spec-version",
        type=lambda value: _parse_nonnegative_u32(
            value,
            label="expected spec version",
        ),
        help="Pinned state_getRuntimeVersion specVersion.",
    )
    parser.add_argument(
        "--expected-transaction-version",
        type=lambda value: _parse_nonnegative_u32(
            value,
            label="expected transaction version",
        ),
        help="Pinned state_getRuntimeVersion transactionVersion.",
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: evidence.parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help="Governed Substrate route allowlist hash.",
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
        help="Expected canonical SORA -> Substrate destination binding hash.",
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
            domain=args.domain,
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
