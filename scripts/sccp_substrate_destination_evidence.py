#!/usr/bin/env python3
"""Render SCCP Substrate-family destination rollout evidence.

Operators supply runtime verifier evidence to check the destination binding.
Production TOML additionally requires pinned source record hashes, the governed
route allowlist hash, and the expected destination binding hash collected from
independent governance or deployment records.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
from pathlib import Path
from typing import Iterable


SCCP_DOMAIN_SORA = 0
SCCP_STARK_FRI_PROOF_FAMILY = "stark-fri-v1"
SCCP_DESTINATION_BINDING_PREFIX = b"sccp:destination:binding:v1"
SCCP_ROUTE_ALLOWLIST_LABEL = b"sccp:route-allowlist:lane-evidence:v1"
SCCP_SUBSTRATE_ROUTE_CANARY_FINALIZED_RUNTIME_LABEL = (
    b"iroha:sccp:substrate-route-canary-finalized-runtime:v1"
)
SUBSTRATE_RUNTIME_BACKEND = "substrate-runtime-v1"
SUBSTRATE_RUNTIME_VERIFIER_TARGET_CODE = 5
SUBSTRATE_RUNTIME_BACKEND_FAMILY_CODE = 5
SUBSTRATE_RUNTIME_VERIFIER_ID = "SccpBridge.submit_message_proof"
SUBSTRATE_DESTINATION_PROFILES = {
    6: {
        "chain": "sora-kusama",
        "anchor_id": "sccp:sora-kusama:destination-anchor:runtime:v1",
        "route_allowlist_id": "sccp:sora-kusama:route-allowlist:runtime:v1",
    },
    7: {
        "chain": "sora-polkadot",
        "anchor_id": "sccp:sora-polkadot:destination-anchor:runtime:v1",
        "route_allowlist_id": "sccp:sora-polkadot:route-allowlist:runtime:v1",
    },
    8: {
        "chain": "sora2",
        "anchor_id": "sccp:sora2:destination-anchor:runtime:v1",
        "route_allowlist_id": "sccp:sora2:route-allowlist:runtime:v1",
    },
}
SUBSTRATE_DOMAIN_ALIASES = {
    "6": 6,
    "sora-kusama": 6,
    "kusama": 6,
    "7": 7,
    "sora-polkadot": 7,
    "polkadot": 7,
    "8": 8,
    "sora2": 8,
}


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def parse_hex_bytes(
    value: str,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    """Parse a fixed-width hex value."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_0x(value)
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if nonzero and not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def parse_runtime_code_hex(value: str, *, label: str) -> bytes:
    """Parse non-empty Substrate runtime code bytes from hex text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    text = _strip_0x(text)
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if len(text) % 2 != 0:
        raise argparse.ArgumentTypeError(f"{label} must have an even hex length")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    return raw


def parse_runtime_code_base64(value: str, *, label: str) -> bytes:
    """Parse non-empty Substrate runtime code bytes from base64 text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    try:
        raw = base64.b64decode(text, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise argparse.ArgumentTypeError(f"{label} must be base64") from exc
    if base64.b64encode(raw).decode("ascii") != text:
        raise argparse.ArgumentTypeError(f"{label} must be canonical base64")
    if not raw:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    return raw


def parse_runtime_code_file(value: str, *, label: str) -> bytes:
    """Parse non-empty Substrate runtime code bytes from a raw file."""

    path = Path(value).expanduser()
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise argparse.ArgumentTypeError(f"{label} file cannot be read") from exc
    if not raw:
        raise argparse.ArgumentTypeError(f"{label} file must not be empty")
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} file must not be all zero")
    return raw


def parse_substrate_domain(value: str) -> int:
    """Parse a supported Substrate-family SCCP destination domain."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "domain must be sora-kusama, sora-polkadot, or sora2"
        )
    domain = SUBSTRATE_DOMAIN_ALIASES.get(value.lower())
    if domain is None:
        raise argparse.ArgumentTypeError(
            "domain must be sora-kusama, sora-polkadot, or sora2"
        )
    return domain


def parse_runtime_entrypoint(value: str) -> str:
    """Parse the exact runtime verifier entrypoint expected by SCCP."""

    if value != SUBSTRATE_RUNTIME_VERIFIER_ID:
        raise argparse.ArgumentTypeError(
            f"verifier entrypoint must be {SUBSTRATE_RUNTIME_VERIFIER_ID}"
        )
    return value


def parse_nonempty_string(value: str, *, label: str) -> str:
    """Parse a non-empty string."""

    if value != value.strip() or not value:
        raise argparse.ArgumentTypeError(f"{label} must be non-empty")
    return value


def parse_decimal_u32(value: str, *, label: str) -> int:
    """Parse a decimal u32."""

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


def _require_fixed_bytes(
    value: bytes,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be {byte_length} bytes")
    raw = bytes(value)
    if len(raw) != byte_length:
        raise ValueError(f"{label} must be {byte_length} bytes")
    if nonzero and not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def _require_verifier_code_hash_role_separation(
    *,
    verifier_code_hash: bytes,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
) -> None:
    for role, raw in (
        ("route_allowlist_hash", route_allowlist_hash),
        ("destination_binding_hash", destination_binding_hash),
        ("source_verifier_material_hash", source_verifier_material_hash),
        (
            "source_adapter_engine_deployment_hash",
            source_adapter_engine_deployment_hash,
        ),
    ):
        if verifier_code_hash == raw:
            raise ValueError(f"verifier_code_hash must differ from {role}")


def _require_runtime_entrypoint(value: str) -> str:
    try:
        return parse_runtime_entrypoint(value)
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc


def _require_destination_evidence(args: argparse.Namespace) -> None:
    _profile(args.domain)
    args.verifier_entrypoint = _require_runtime_entrypoint(args.verifier_entrypoint)
    args.verifier_code_hash = _require_fixed_bytes(
        args.verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )


def _profile(domain: int) -> dict[str, str]:
    return SUBSTRATE_DESTINATION_PROFILES[domain]


def _hex(value: bytes) -> str:
    return "0x" + value.hex()


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little", signed=False))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


def substrate_runtime_code_hash(runtime_code: bytes) -> bytes:
    """Compute the deployed Substrate runtime code hash used in evidence."""

    if not isinstance(runtime_code, (bytes, bytearray)):
        raise ValueError("Substrate runtime code must be bytes")
    raw = bytes(runtime_code)
    if not raw or not any(raw):
        raise ValueError("Substrate runtime code must not be empty or all zero")
    return hashlib.blake2b(raw, digest_size=32).digest()


def apply_runtime_code_hash(args: argparse.Namespace) -> None:
    """Fill or verify the Substrate runtime code hash from runtime bytes."""

    runtime_hex = getattr(args, "runtime_code_hex", None)
    runtime_base64 = getattr(args, "runtime_code_base64", None)
    runtime_file = getattr(args, "runtime_code_file", None)
    supplied = [
        value
        for value in (runtime_hex, runtime_base64, runtime_file)
        if value is not None
    ]
    if len(supplied) > 1:
        raise ValueError(
            "--runtime-code-hex, --runtime-code-base64, and "
            "--runtime-code-file are mutually exclusive"
        )
    runtime_code = supplied[0] if supplied else None
    if runtime_code is None:
        if getattr(args, "verifier_code_hash", None) is None:
            raise ValueError(
                "--verifier-code-hash, --runtime-code-hex, or "
                "--runtime-code-file is required"
            )
        return
    derived_hash = substrate_runtime_code_hash(runtime_code)
    args.runtime_code_bytes = bytes(runtime_code)
    args.runtime_code_base64_text = base64.b64encode(bytes(runtime_code)).decode(
        "ascii"
    )
    if args.verifier_code_hash is not None and args.verifier_code_hash != derived_hash:
        raise ValueError(
            "--verifier-code-hash does not match Substrate runtime code: "
            f"expected {_hex(args.verifier_code_hash)}, got {_hex(derived_hash)}"
        )
    args.verifier_code_hash = derived_hash


def substrate_destination_binding_key(domain: int) -> str:
    """Return Rust's canonical SORA -> Substrate destination binding key."""

    profile = _profile(domain)
    return (
        f"sccp:{SCCP_DOMAIN_SORA}:{domain}:{profile['chain']}:"
        f"{SUBSTRATE_RUNTIME_BACKEND}:{SUBSTRATE_RUNTIME_VERIFIER_TARGET_CODE}"
    )


def substrate_destination_binding_hash(domain: int) -> bytes:
    """Compute Rust's canonical SORA -> Substrate destination binding hash."""

    profile = _profile(domain)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, domain)
    _push_u8(payload, 1)  # RecursiveZk
    _push_u8(payload, 1)  # CryptographicProof
    _push_u8(payload, SUBSTRATE_RUNTIME_VERIFIER_TARGET_CODE)
    _push_u8(payload, SUBSTRATE_RUNTIME_BACKEND_FAMILY_CODE)
    _push_vec(payload, substrate_destination_binding_key(domain).encode("utf-8"))
    _push_vec(
        payload,
        (
            "iroha:sccp:bridge-proof:message:stark-fri:v1:"
            + profile["chain"]
        ).encode("utf-8"),
    )
    _push_vec(payload, SCCP_STARK_FRI_PROOF_FAMILY.encode("utf-8"))
    _push_vec(payload, SUBSTRATE_RUNTIME_BACKEND.encode("utf-8"))
    return _prefixed_blake2b(SCCP_DESTINATION_BINDING_PREFIX, bytes(payload))


def substrate_route_allowlist_hash(
    *,
    domain: int,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    """Compute Rust's canonical SORA -> Substrate route allowlist hash."""

    profile = _profile(domain)
    source_verifier_material_hash = _require_fixed_bytes(
        source_verifier_material_hash,
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        source_adapter_engine_deployment_hash,
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, domain)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, profile["route_allowlist_id"].encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return _prefixed_blake2b(SCCP_ROUTE_ALLOWLIST_LABEL, payload)


def _require_u32_value(value: int, *, label: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFF_FFFF:
        raise ValueError(f"{label} must be a u32")
    return value


def _require_runtime_code(value: bytes, *, label: str) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be bytes")
    raw = bytes(value)
    if not raw or not any(raw):
        raise ValueError(f"{label} must not be empty or all zero")
    return raw


def substrate_route_canary_evidence_hash(
    *,
    domain: int,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    verifier_entrypoint: str,
    verifier_code_hash: bytes,
    finalized_head: bytes,
    runtime_spec_name: str,
    runtime_spec_version: int,
    runtime_transaction_version: int,
    runtime_code: bytes,
) -> bytes:
    """Hash finalized Substrate runtime metadata used by route-canary preflight."""

    profile = _profile(domain)
    route_allowlist_hash = _require_fixed_bytes(
        route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    source_verifier_material_hash = _require_fixed_bytes(
        source_verifier_material_hash,
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        source_adapter_engine_deployment_hash,
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    verifier_entrypoint = _require_runtime_entrypoint(verifier_entrypoint)
    verifier_code_hash = _require_fixed_bytes(
        verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )
    _require_verifier_code_hash_role_separation(
        verifier_code_hash=verifier_code_hash,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=(
            source_adapter_engine_deployment_hash
        ),
    )
    finalized_head = _require_fixed_bytes(
        finalized_head,
        label="finalized_head",
        byte_length=32,
    )
    if not isinstance(runtime_spec_name, str) or runtime_spec_name != profile["chain"]:
        raise ValueError("runtime_spec_name must match destination chain")
    runtime_spec_version = _require_u32_value(
        runtime_spec_version,
        label="runtime_spec_version",
    )
    runtime_transaction_version = _require_u32_value(
        runtime_transaction_version,
        label="runtime_transaction_version",
    )
    runtime_code = _require_runtime_code(runtime_code, label="runtime_code")
    runtime_code_hash = substrate_runtime_code_hash(runtime_code)
    if runtime_code_hash != verifier_code_hash:
        raise ValueError("runtime_code must match verifier_code_hash")

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, domain)
    payload.extend(route_allowlist_hash)
    payload.extend(destination_binding_hash)
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_vec(payload, verifier_entrypoint.encode("utf-8"))
    payload.extend(verifier_code_hash)
    payload.extend(finalized_head)
    _push_vec(payload, runtime_spec_name.encode("utf-8"))
    _push_u32(payload, runtime_spec_version)
    _push_u32(payload, runtime_transaction_version)
    _push_vec(payload, runtime_code)
    return _prefixed_blake2b(
        SCCP_SUBSTRATE_ROUTE_CANARY_FINALIZED_RUNTIME_LABEL,
        payload,
    )


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _toml_line(key: str, value: object) -> str:
    if isinstance(value, bool):
        rendered = "true" if value else "false"
    elif isinstance(value, int):
        rendered = str(value)
    elif isinstance(value, str):
        rendered = _toml_string(value)
    elif isinstance(value, list) and all(isinstance(item, str) for item in value):
        rendered = "[" + ", ".join(_toml_string(item) for item in value) + "]"
    else:
        raise TypeError(f"unsupported TOML value for {key}")
    return f"{key} = {rendered}"


def _destination_rollout_lines(args: argparse.Namespace) -> Iterable[str]:
    profile = _profile(args.domain)
    yield "[[zk.sccp_destination_rollouts]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", args.domain)
    yield _toml_line("chain", profile["chain"])
    yield _toml_line("verifier_plan", "SubstrateRuntimeNativeRecursive")
    yield _toml_line("immutable_verifier_ready", True)
    yield _toml_line("anchors_ready", True)
    yield _toml_line("verifier_identity", args.verifier_entrypoint)
    yield _toml_line("verifier_code_hash", _hex(args.verifier_code_hash))
    yield _toml_line("destination_binding_key", substrate_destination_binding_key(args.domain))
    yield _toml_line(
        "destination_binding_hash", _hex(substrate_destination_binding_hash(args.domain))
    )
    yield _toml_line("anchor_id", profile["anchor_id"])
    yield _toml_line("substrate_finalized_head", _hex(args.finalized_head))
    yield _toml_line("substrate_runtime_spec_name", str(args.runtime_spec_name))
    yield _toml_line("substrate_runtime_spec_version", str(args.runtime_spec_version))
    yield _toml_line(
        "substrate_runtime_transaction_version",
        str(args.runtime_transaction_version),
    )
    yield _toml_line("substrate_runtime_code_hash", _hex(args.verifier_code_hash))
    runtime_code_base64 = getattr(args, "runtime_code_base64_text", None)
    if runtime_code_base64 is not None:
        yield _toml_line("substrate_runtime_code_base64", runtime_code_base64)
    yield _toml_line("blockers", [])


def _route_allowlist_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> Iterable[str]:
    profile = _profile(args.domain)
    supplied_route_allowlist_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    if supplied_route_allowlist_hash != route_allowlist_hash:
        raise ValueError("route_allowlist_hash does not match validated lane evidence")
    yield "[[zk.sccp_route_allowlists]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", args.domain)
    yield _toml_line("chain", profile["chain"])
    yield _toml_line("activation_policy", "GovernanceAllowlist")
    yield _toml_line("route_allowlist_id", profile["route_allowlist_id"])
    yield _toml_line("route_allowlist_hash", _hex(route_allowlist_hash))
    yield from _route_canary_toml_lines(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    yield _toml_line("routes_allowlisted", True)
    yield _toml_line("blockers", [])


def _route_canary_toml_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return []
    return [
        _toml_line("route_canary_status", "passed"),
        _toml_line("route_canary_evidence_hash", _hex(canary_hash)),
        _toml_line("route_canary_route_allowlist_hash", _hex(route_allowlist_hash)),
        _toml_line(
            "route_canary_destination_binding_hash",
            _hex(destination_binding_hash),
        ),
    ]


def _route_canary_comment_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return []
    return [
        "# sccp_route_canary_status = " + json.dumps("passed"),
        "# sccp_route_canary_evidence_hash = " + json.dumps(_hex(canary_hash)),
        "# sccp_route_canary_route_allowlist_hash = "
        + json.dumps(_hex(route_allowlist_hash)),
        "# sccp_route_canary_destination_binding_hash = "
        + json.dumps(_hex(destination_binding_hash)),
    ]


def _route_canary_summary(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> dict[str, object] | None:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return None
    return {
        "status": "passed",
        "evidence_hash": _hex(canary_hash),
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "destination_binding_hash": _hex(destination_binding_hash),
        "evidence_bound": True,
    }


def _route_canary_evidence_hash(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes | None:
    canary_hash = getattr(args, "route_canary_evidence_hash", None)
    if canary_hash is None:
        return None
    canary_hash = _require_fixed_bytes(
        canary_hash,
        label="route_canary_evidence_hash",
        byte_length=32,
    )
    source_verifier_material_hash = _require_fixed_bytes(
        getattr(args, "source_verifier_material_hash", None),
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        getattr(args, "source_adapter_engine_deployment_hash", None),
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    if canary_hash in (
        route_allowlist_hash,
        destination_binding_hash,
        source_verifier_material_hash,
        source_adapter_engine_deployment_hash,
    ):
        raise ValueError(
            "route_canary_evidence_hash must be distinct from route_allowlist_hash, "
            "destination_binding_hash, source_verifier_material_hash, and "
            "source_adapter_engine_deployment_hash"
        )
    runtime_code = getattr(args, "runtime_code_bytes", None)
    if runtime_code is None:
        raise ValueError(
            "route_canary_evidence_hash requires runtime code bytes from "
            "--runtime-code-hex, --runtime-code-base64, or --runtime-code-file"
        )
    _require_toml_runtime_metadata(args, output="route-canary-evidence-hash")
    expected_hash = substrate_route_canary_evidence_hash(
        domain=args.domain,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        verifier_entrypoint=args.verifier_entrypoint,
        verifier_code_hash=args.verifier_code_hash,
        finalized_head=args.finalized_head,
        runtime_spec_name=args.runtime_spec_name,
        runtime_spec_version=args.runtime_spec_version,
        runtime_transaction_version=args.runtime_transaction_version,
        runtime_code=runtime_code,
    )
    if canary_hash != expected_hash:
        raise ValueError(
            "route_canary_evidence_hash must match finalized Substrate "
            "runtime metadata"
        )
    return canary_hash


def _route_allowlist_hash_from_args(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    return substrate_route_allowlist_hash(
        domain=args.domain,
        source_verifier_material_hash=getattr(
            args,
            "source_verifier_material_hash",
            None,
        ),
        source_adapter_engine_deployment_hash=(
            getattr(args, "source_adapter_engine_deployment_hash", None)
        ),
        destination_binding_hash=destination_binding_hash,
    )


def _require_expected_route_allowlist_hash(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    supplied_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    expected_hash = _route_allowlist_hash_from_args(args, destination_binding_hash)
    if supplied_hash != expected_hash:
        raise ValueError(
            "--route-allowlist-hash does not match canonical source, deployment, "
            "and destination evidence: "
            f"expected {_hex(expected_hash)}, got {_hex(supplied_hash)}"
        )
    return expected_hash


def _require_toml_runtime_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    finalized_head = getattr(args, "finalized_head", None)
    if finalized_head is None:
        raise ValueError(f"--{output} requires --finalized-head")
    _require_fixed_bytes(
        finalized_head,
        label="finalized_head",
        byte_length=32,
    )
    runtime_spec_name = getattr(args, "runtime_spec_name", None)
    if (
        not isinstance(runtime_spec_name, str)
        or not runtime_spec_name
        or runtime_spec_name != runtime_spec_name.strip()
    ):
        raise ValueError(f"--{output} requires --runtime-spec-name")
    expected_spec_name = _profile(args.domain)["chain"]
    if runtime_spec_name != expected_spec_name:
        raise ValueError(
            "runtime specName must match destination domain "
            f"{expected_spec_name}: got {runtime_spec_name!r}"
        )
    runtime_spec_version = getattr(args, "runtime_spec_version", None)
    if type(runtime_spec_version) is not int or runtime_spec_version < 0:
        raise ValueError(f"--{output} requires --runtime-spec-version")
    runtime_transaction_version = getattr(
        args,
        "runtime_transaction_version",
        None,
    )
    if (
        type(runtime_transaction_version) is not int
        or runtime_transaction_version < 0
    ):
        raise ValueError(f"--{output} requires --runtime-transaction-version")


def _toml_runtime_metadata_ready(args: argparse.Namespace) -> bool:
    try:
        _require_toml_runtime_metadata(args, output="toml")
    except ValueError:
        return False
    return True


def _missing_route_allowlist_args(args: argparse.Namespace) -> list[str]:
    return [
        name
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
        )
        if getattr(args, name, None) is None
    ]


def render_toml(
    args: argparse.Namespace,
    destination_binding_hash: bytes | None = None,
) -> str:
    """Render production Substrate destination rollout and route allowlist TOML."""

    apply_runtime_code_hash(args)
    _require_destination_evidence(args)
    expected_hash = substrate_destination_binding_hash(args.domain)
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is None:
        raise ValueError(
            "--expected-destination-binding-hash is required before rendering production TOML"
        )
    if expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match the canonical "
            f"SORA -> {_profile(args.domain)['chain']} binding: "
            f"expected {_hex(expected_pin)}, got {_hex(expected_hash)}"
        )
    if destination_binding_hash is None:
        destination_binding_hash = expected_pin
    elif destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> {_profile(args.domain)['chain']} binding: "
            f"expected {_hex(expected_hash)}, got {_hex(destination_binding_hash)}"
        )
    missing_route_args = _missing_route_allowlist_args(args)
    if missing_route_args:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing_route_args)
        raise ValueError(f"--toml requires {formatted}")
    route_allowlist_hash = _require_expected_route_allowlist_hash(
        args,
        destination_binding_hash,
    )
    if getattr(args, "route_canary_evidence_hash", None) is None:
        raise ValueError("--toml requires --route-canary-evidence-hash")
    _require_toml_runtime_metadata(args, output="toml")
    runtime_code_base64 = getattr(args, "runtime_code_base64_text", None)
    comments = [
        "# sccp_substrate_finalized_head = "
        + json.dumps(_hex(args.finalized_head)),
        "# sccp_substrate_runtime_spec_name = "
        + json.dumps(str(args.runtime_spec_name)),
        "# sccp_substrate_runtime_spec_version = "
        + json.dumps(str(args.runtime_spec_version)),
        "# sccp_substrate_runtime_transaction_version = "
        + json.dumps(str(args.runtime_transaction_version)),
        "# sccp_substrate_runtime_code_hash = "
        + json.dumps(_hex(args.verifier_code_hash)),
    ]
    if runtime_code_base64 is not None:
        comments.append(
            "# sccp_substrate_runtime_code_base64 = "
            + json.dumps(runtime_code_base64)
        )
    comments.extend(
        [
            "# sccp_substrate_destination_binding_hash = "
            + json.dumps(_hex(destination_binding_hash)),
            "# sccp_substrate_route_allowlist_hash = "
            + json.dumps(_hex(route_allowlist_hash)),
        ]
    )
    return "\n".join(
        [
            *comments,
            *_destination_rollout_lines(args),
            "",
            *_route_canary_comment_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
            ),
            *_route_allowlist_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
            ),
            "",
        ]
    )


def _json_summary(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
    expected_matches: bool,
) -> dict[str, object]:
    apply_runtime_code_hash(args)
    _require_destination_evidence(args)
    profile = _profile(args.domain)
    expected_hash = substrate_destination_binding_hash(args.domain)
    if destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> {profile['chain']} binding: "
            f"expected {_hex(expected_hash)}, got {_hex(destination_binding_hash)}"
        )
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is not None and expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match the canonical "
            f"SORA -> {profile['chain']} binding: "
            f"expected {_hex(expected_pin)}, got {_hex(expected_hash)}"
        )
    route_requested = any(
        getattr(args, name, None) is not None
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
            "route_canary_evidence_hash",
        )
    )
    summary = {
        "source_domain": SCCP_DOMAIN_SORA,
        "domain": args.domain,
        "chain": profile["chain"],
        "verifier_plan": "SubstrateRuntimeNativeRecursive",
        "verifier_identity": args.verifier_entrypoint,
        "verifier_code_hash": _hex(args.verifier_code_hash),
        "anchor_id": profile["anchor_id"],
        "destination_binding_key": substrate_destination_binding_key(args.domain),
        "destination_binding_hash": _hex(destination_binding_hash),
        "expected_destination_binding_hash_matches": expected_matches,
        "toml_ready": False,
    }
    runtime_code_base64 = getattr(args, "runtime_code_base64_text", None)
    if isinstance(runtime_code_base64, str) and runtime_code_base64.strip():
        summary["runtime_code_base64"] = runtime_code_base64
        summary["runtime_code_base64_sha256"] = hashlib.sha256(
            runtime_code_base64.encode("ascii")
        ).hexdigest()
    if route_requested:
        if expected_pin is None:
            raise ValueError(
                "--route-allowlist-hash requires "
                "--expected-destination-binding-hash"
            )
        missing_route_args = _missing_route_allowlist_args(args)
        if missing_route_args:
            formatted = ", ".join(
                f"--{name.replace('_', '-')}" for name in missing_route_args
            )
            raise ValueError("route allowlist evidence requires " + formatted)
        route_allowlist_hash = _require_fixed_bytes(
            args.route_allowlist_hash,
            label="route_allowlist_hash",
            byte_length=32,
        )
        expected_route_allowlist_hash = _require_expected_route_allowlist_hash(
            args,
            destination_binding_hash,
        )
        _require_toml_runtime_metadata(args, output="json")
        summary.update(
            {
                "source_verifier_material_hash": _hex(
                    args.source_verifier_material_hash
                ),
                "source_adapter_engine_deployment_hash": _hex(
                    args.source_adapter_engine_deployment_hash
                ),
                "route_allowlist_id": profile["route_allowlist_id"],
                "route_allowlist_hash": _hex(route_allowlist_hash),
                "expected_route_allowlist_hash": _hex(
                    expected_route_allowlist_hash
                ),
                "expected_route_allowlist_hash_matches": True,
                "toml_ready": (
                    expected_matches and _toml_runtime_metadata_ready(args)
                ),
            }
        )
        route_canary = _route_canary_summary(
            args,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
        )
        if route_canary is not None:
            summary["route_canary"] = route_canary
            summary["toml_ready"] = bool(summary["toml_ready"])
        else:
            summary["toml_ready"] = False
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP Substrate-family destination rollout evidence.",
    )
    parser.add_argument(
        "--domain",
        required=True,
        type=parse_substrate_domain,
        help="Destination domain: sora-kusama, sora-polkadot, or sora2.",
    )
    parser.add_argument(
        "--verifier-entrypoint",
        default=SUBSTRATE_RUNTIME_VERIFIER_ID,
        type=parse_runtime_entrypoint,
        help="Runtime verifier entrypoint. Defaults to the SCCP production entrypoint.",
    )
    parser.add_argument(
        "--verifier-code-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="verifier code hash",
            byte_length=32,
        ),
        help="Non-zero deployed runtime verifier code hash.",
    )
    parser.add_argument(
        "--runtime-code-hex",
        type=lambda value: parse_runtime_code_hex(
            value,
            label="runtime code",
        ),
        help=(
            "Hex-encoded deployed Substrate runtime code. When supplied, "
            "the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--runtime-code-base64",
        type=lambda value: parse_runtime_code_base64(
            value,
            label="runtime code",
        ),
        help=(
            "Base64-encoded deployed Substrate runtime code. When supplied, "
            "the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--runtime-code-file",
        type=lambda value: parse_runtime_code_file(
            value,
            label="runtime code",
        ),
        help=(
            "Raw file containing deployed Substrate runtime code. When "
            "supplied, the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help=(
            "Governed Substrate route allowlist hash. Must match the canonical "
            "source material, source adapter deployment, and destination "
            "binding tuple."
        ),
    )
    parser.add_argument(
        "--source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="source verifier material hash",
            byte_length=32,
        ),
        help="Source verifier material record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--source-adapter-engine-deployment-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="source adapter engine deployment hash",
            byte_length=32,
        ),
        help="Source adapter engine deployment record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary evidence hash",
            byte_length=32,
        ),
        help=(
            "Non-zero post-deploy route canary evidence hash. The helper "
            "recomputes it from finalized Substrate runtime metadata before "
            "emitting all-lanes preflight TOML."
        ),
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected destination binding hash",
            byte_length=32,
        ),
        help="Expected canonical SORA -> Substrate destination binding hash.",
    )
    parser.add_argument(
        "--finalized-head",
        type=lambda value: parse_hex_bytes(
            value,
            label="finalized head",
            byte_length=32,
        ),
        help="Audited finalized head used to read the runtime; required for TOML.",
    )
    parser.add_argument(
        "--runtime-spec-name",
        type=lambda value: parse_nonempty_string(
            value,
            label="runtime spec name",
        ),
        help="Audited runtime specName; required for TOML.",
    )
    parser.add_argument(
        "--runtime-spec-version",
        type=lambda value: parse_decimal_u32(
            value,
            label="runtime specVersion",
        ),
        help="Audited runtime specVersion; required for TOML.",
    )
    parser.add_argument(
        "--runtime-transaction-version",
        type=lambda value: parse_decimal_u32(
            value,
            label="runtime transactionVersion",
        ),
        help="Audited runtime transactionVersion; required for TOML.",
    )
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Render production TOML records instead of a compact JSON summary.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        apply_runtime_code_hash(args)
        destination_binding_hash = substrate_destination_binding_hash(args.domain)
        expected_matches = False
        if args.expected_destination_binding_hash is not None:
            if args.expected_destination_binding_hash != destination_binding_hash:
                raise ValueError(
                    "expected destination binding hash does not match the canonical "
                    "SORA -> Substrate binding: "
                    f"expected {_hex(args.expected_destination_binding_hash)}, "
                    f"got {_hex(destination_binding_hash)}"
                )
            expected_matches = True
        if args.toml:
            print(render_toml(args, destination_binding_hash), end="")
        else:
            print(
                json.dumps(
                    _json_summary(args, destination_binding_hash, expected_matches),
                    sort_keys=True,
                    indent=2,
                )
            )
    except ValueError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
