#!/usr/bin/env python3
"""Generate and onboard a runtime-only Taira rollout canary config."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any
from urllib import error, request
from urllib.parse import urlparse

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

SCRIPT_DIR = Path(__file__).resolve().parent
IROHA_REPO_ROOT = SCRIPT_DIR.parent
IROHA_PYTHON_SRC = IROHA_REPO_ROOT / "python" / "iroha_python" / "src"
IROHA_PYTHON_ADDRESS = IROHA_PYTHON_SRC / "iroha_python" / "address.py"
DEFAULT_LOCALNET_CLIENT_TOML = IROHA_REPO_ROOT / "dist" / "taira-localnet" / "client.toml"
DEFAULT_IROHA_BIN_CANDIDATES = (
    IROHA_REPO_ROOT / "target" / "release" / "iroha",
    IROHA_REPO_ROOT / "target" / "debug" / "iroha",
)
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from taira_faucet_canary import solve_puzzle  # noqa: E402


AccountAddress = None
if IROHA_PYTHON_ADDRESS.exists():
    try:
        spec = importlib.util.spec_from_file_location(
            "iroha_python_address", IROHA_PYTHON_ADDRESS
        )
        if spec and spec.loader:
            module = importlib.util.module_from_spec(spec)
            sys.modules[spec.name] = module
            spec.loader.exec_module(module)
            AccountAddress = getattr(module, "AccountAddress", None)
    except Exception:  # pragma: no cover
        AccountAddress = None


DEFAULT_CHAIN_ID = "809574f5-fee7-5e69-bfcf-52451e42d50f"
DEFAULT_CHAIN_DISCRIMINANT = 369
DEFAULT_DOMAIN = "wonderland.universal"
DEFAULT_ALIAS_PREFIX = "taira-rollout-canary"
DEFAULT_TIME_TO_LIVE_MS = 120_000
DEFAULT_STATUS_TIMEOUT_MS = 120_000
DEFAULT_LOCAL_LOOPBACK_FUND_AMOUNT = os.environ.get(
    "TAIRA_LOCAL_LOOPBACK_FUND_AMOUNT", "1000000"
)
PUBLIC_ED25519_MULTICODEC = 0xED
PRIVATE_ED25519_MULTICODEC = 0x1300
HEX_BLOCK_RE = re.compile(r"^(?:[0-9a-f]{2}:)*[0-9a-f]{2}:?$", re.IGNORECASE)
PLACEHOLDER_VALUES = {
    "REPLACE_WITH_CANARY_PUBLIC_KEY",
    "REPLACE_WITH_CANARY_PRIVATE_KEY",
}


def encode_varint(value: int) -> bytes:
    out = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        if remaining:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def decode_varint(data: bytes, offset: int = 0) -> tuple[int, int]:
    shift = 0
    value = 0
    index = offset
    while index < len(data):
        byte = data[index]
        index += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, index
        shift += 7
    raise ValueError("truncated varint")


def format_multihash_hex(multicodec: int, payload: bytes) -> str:
    return (
        encode_varint(multicodec).hex()
        + encode_varint(len(payload)).hex()
        + payload.hex().upper()
    )


def decode_multihash_payload(multihash_hex: str, expected_multicodec: int) -> bytes:
    encoded = bytes.fromhex(multihash_hex.strip())
    multicodec, offset = decode_varint(encoded)
    if multicodec != expected_multicodec:
        raise ValueError(
            f"unexpected multicodec {multicodec:#x}; expected {expected_multicodec:#x}"
        )
    payload_len, offset = decode_varint(encoded, offset)
    payload = encoded[offset:]
    if len(payload) != payload_len:
        raise ValueError(
            f"payload length mismatch: expected {payload_len}, got {len(payload)}"
        )
    return payload


def _http_json(method: str, url: str, payload: dict[str, Any] | None = None) -> tuple[int, Any]:
    data = None
    headers = {"accept": "application/json"}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["content-type"] = "application/json"
    req = request.Request(url, data=data, headers=headers, method=method)
    try:
        with request.urlopen(req) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            return resp.status, json.loads(body) if body else None
    except error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        try:
            parsed: Any = json.loads(body) if body else None
        except json.JSONDecodeError:
            parsed = body
        return exc.code, parsed


def run_command(
    cmd: list[str],
    *,
    input_text: str | None = None,
    cwd: Path | None = None,
) -> str:
    proc = subprocess.run(
        cmd,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
        cwd=str(cwd) if cwd is not None else None,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        stderr = proc.stderr.strip()
        stdout = proc.stdout.strip()
        detail = stderr or stdout or f"exit code {proc.returncode}"
        raise RuntimeError(f"command failed: {' '.join(cmd)}: {detail}")
    return proc.stdout


def parse_openssl_hex_block(text: str, label: str) -> bytes:
    lines = text.splitlines()
    collecting = False
    parts: list[str] = []
    for line in lines:
        stripped = line.strip()
        if stripped == f"{label}:":
            collecting = True
            continue
        if not collecting:
            continue
        if not stripped:
            continue
        if HEX_BLOCK_RE.fullmatch(stripped):
            parts.append(stripped.replace(":", ""))
            continue
        if parts:
            break
    if not parts:
        raise RuntimeError(f"openssl output did not include `{label}:` bytes")
    return bytes.fromhex("".join(parts))


def generate_ed25519_keypair() -> tuple[str, str, str]:
    with tempfile.TemporaryDirectory() as tmpdir:
        key_path = Path(tmpdir) / "ed25519.pem"
        try:
            subprocess.run(
                ["openssl", "genpkey", "-algorithm", "Ed25519", "-out", str(key_path)],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                text=True,
            )
            rendered = run_command(["openssl", "pkey", "-in", str(key_path), "-text", "-noout"])
            private_seed = parse_openssl_hex_block(rendered, "priv")
            public_key_raw = parse_openssl_hex_block(rendered, "pub")
            public_key = format_multihash_hex(PUBLIC_ED25519_MULTICODEC, public_key_raw)
            private_key = format_multihash_hex(PRIVATE_ED25519_MULTICODEC, private_seed)
            return public_key, private_key, public_key_raw.hex()
        except (subprocess.CalledProcessError, RuntimeError):
            return generate_ed25519_keypair_via_iroha(repo_root=IROHA_REPO_ROOT)


def generate_ed25519_keypair_via_iroha(*, repo_root: Path) -> tuple[str, str, str]:
    rendered = run_command(
        ["cargo", "run", "-q", "-p", "iroha", "--example", "dev_key_material"],
        cwd=repo_root,
    )
    try:
        payload = json.loads(rendered)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"failed to parse Iroha keygen output: {rendered!r}") from exc

    public_key = payload.get("public_key")
    private_key = payload.get("private_key")
    public_key_raw_hex = payload.get("public_key_raw_hex")
    if not all(isinstance(value, str) and value.strip() for value in (public_key, private_key, public_key_raw_hex)):
        raise RuntimeError(f"Iroha keygen output is incomplete: {payload!r}")
    return public_key.strip(), private_key.strip(), public_key_raw_hex.strip().lower()


def shutil_which(name: str) -> str | None:
    path = os.environ.get("PATH", "")
    for entry in path.split(os.pathsep):
        if not entry:
            continue
        candidate = Path(entry) / name
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def normalize_domain(domain: str | None) -> str:
    if not domain:
        return DEFAULT_DOMAIN
    normalized = domain.strip()
    if not normalized:
        return DEFAULT_DOMAIN
    if "." not in normalized:
        return f"{normalized}.universal"
    return normalized


def load_existing_config(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    with path.open("rb") as handle:
        data = tomllib.load(handle)

    account = data.get("account") or {}
    public_key = account.get("public_key")
    private_key = account.get("private_key")
    account_id = account.get("id")
    if not isinstance(public_key, str) or not isinstance(private_key, str):
        return None
    if public_key in PLACEHOLDER_VALUES or private_key in PLACEHOLDER_VALUES:
        return None

    public_key_raw = decode_multihash_payload(public_key, PUBLIC_ED25519_MULTICODEC).hex()
    decode_multihash_payload(private_key, PRIVATE_ED25519_MULTICODEC)

    basic_auth = data.get("basic_auth")
    return {
        "chain": data.get("chain") or DEFAULT_CHAIN_ID,
        "domain": normalize_domain(account.get("domain")),
        "account_id": account_id.strip() if isinstance(account_id, str) else None,
        "public_key": public_key,
        "private_key": private_key,
        "public_key_raw_hex": public_key_raw,
        "chain_discriminant": account.get("chain_discriminant") or DEFAULT_CHAIN_DISCRIMINANT,
        "basic_auth": basic_auth if isinstance(basic_auth, dict) else None,
        "nonce": bool((data.get("transaction") or {}).get("nonce", False)),
    }


def build_alias(prefix: str, public_key: str, domain: str) -> str:
    label_prefix = re.sub(r"[^a-z0-9]", "", prefix.lower()) or "tairacanary"
    suffix = re.sub(r"[^a-z0-9]", "", public_key[-16:].lower())
    dataspace = normalize_domain(domain).lower().split(".")[-1]
    return f"{label_prefix}{suffix}@{dataspace}"


def is_loopback_torii_root(torii_root: str) -> bool:
    try:
        hostname = urlparse(torii_root).hostname
    except ValueError:
        return False
    return hostname in {"127.0.0.1", "localhost", "::1"}


def canonical_account_id_from_public_key(
    public_key_literal: str,
    chain_discriminant: int,
    iroha_bin: str | None,
) -> str | None:
    if not isinstance(public_key_literal, str) or not public_key_literal.strip():
        return None
    try:
        iroha_cli = resolve_iroha_bin(iroha_bin)
        public_key_bytes = decode_multihash_payload(
            public_key_literal.strip(), PUBLIC_ED25519_MULTICODEC
        )
        candidates = [
            format_multihash_hex(PUBLIC_ED25519_MULTICODEC, public_key_bytes),
            public_key_literal.strip(),
        ]
        for candidate in candidates:
            proc = subprocess.run(
                [
                    iroha_cli,
                    "tools",
                    "address",
                    "convert",
                    candidate,
                    "--network-prefix",
                    str(chain_discriminant),
                    "--format",
                    "i105",
                ],
                text=True,
                capture_output=True,
                check=False,
                encoding="utf-8",
                errors="replace",
            )
            if proc.returncode != 0:
                continue
            rendered = proc.stdout.strip()
            if not rendered:
                stderr_lines = [
                    line.strip()
                    for line in proc.stderr.splitlines()
                    if line.strip()
                    and not line.startswith("CLI started")
                    and not line.startswith("Build line:")
                    and not line.startswith("warning:")
                ]
                rendered = stderr_lines[-1] if stderr_lines else ""
            if rendered:
                return rendered
        return None
    except Exception:
        return None


def load_local_registrar(
    client_toml: Path, chain_discriminant: int, iroha_bin: str | None
) -> tuple[str, str]:
    with client_toml.open("rb") as handle:
        data = tomllib.load(handle)
    account = data.get("account")
    if not isinstance(account, dict):
        raise RuntimeError(f"local registrar config is missing [account]: {client_toml}")
    public_key = account.get("public_key")
    private_key = account.get("private_key")
    if not isinstance(public_key, str) or not public_key.strip():
        raise RuntimeError(f"local registrar config is missing account.public_key: {client_toml}")
    if not isinstance(private_key, str) or not private_key.strip():
        raise RuntimeError(f"local registrar config is missing account.private_key: {client_toml}")
    account_id = account.get("id")
    if not isinstance(account_id, str) or not account_id.strip():
        account_id = canonical_account_id_from_public_key(
            public_key.strip(),
            chain_discriminant,
            iroha_bin,
        )
    if not isinstance(account_id, str) or not account_id.strip():
        raise RuntimeError(f"failed to derive local registrar account id from {client_toml}")
    return account_id.strip(), private_key.strip()


def patch_account_id_in_toml(raw: str, account_id: str) -> str:
    lines = raw.splitlines()
    updated: list[str] = []
    in_account = False
    inserted = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            if in_account and not inserted:
                updated.append(f'id = "{account_id}"')
                inserted = True
            in_account = stripped == "[account]"
            updated.append(line)
            continue
        if in_account and stripped.startswith("id"):
            updated.append(f'id = "{account_id}"')
            inserted = True
            continue
        updated.append(line)
    if in_account and not inserted:
        updated.append(f'id = "{account_id}"')
        inserted = True
    if not inserted:
        raise RuntimeError("local registrar config is missing [account] section")
    return "\n".join(updated).rstrip() + "\n"


def write_cli_metadata_file(
    *,
    gas_asset_id: str | None,
    gas_limit: int | None,
    prefix: str,
) -> Path | None:
    metadata: dict[str, Any] = {}
    if isinstance(gas_asset_id, str) and gas_asset_id.strip():
        metadata["gas_asset_id"] = gas_asset_id.strip()
    if isinstance(gas_limit, int) and gas_limit > 0:
        metadata["gas_limit"] = gas_limit
    if not metadata:
        return None
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        prefix=prefix,
        suffix=".json",
        delete=False,
    ) as handle:
        json.dump(metadata, handle)
        handle.write("\n")
        return Path(handle.name)


def resolve_iroha_bin(explicit: str | None) -> str:
    candidate = explicit.strip() if isinstance(explicit, str) else ""
    if candidate:
        resolved = shutil_which(candidate)
        if resolved:
            return resolved
        if Path(candidate).is_file() and os.access(candidate, os.X_OK):
            return candidate
        raise RuntimeError(f"iroha binary not found: {candidate}")
    for path in DEFAULT_IROHA_BIN_CANDIDATES:
        if path.is_file() and os.access(path, os.X_OK):
            return str(path)
    resolved = shutil_which("iroha")
    if resolved:
        return resolved
    raise RuntimeError("iroha binary not found; pass --iroha-bin explicitly")


def register_account_with_local_registrar(
    *,
    client_toml: Path,
    iroha_bin: str | None,
    chain_discriminant: int,
    account_id: str,
    permissions: list[str],
    gas_asset_id: str | None = None,
    gas_limit: int | None = None,
) -> dict[str, Any]:
    registrar_account_id, _ = load_local_registrar(
        client_toml, chain_discriminant, iroha_bin
    )
    iroha_cli = resolve_iroha_bin(iroha_bin)
    metadata_path = write_cli_metadata_file(
        gas_asset_id=gas_asset_id,
        gas_limit=gas_limit,
        prefix="taira_local_registrar_metadata_",
    )
    iroha_cmd_prefix = [iroha_cli]
    if metadata_path is not None:
        iroha_cmd_prefix.extend(["--metadata", str(metadata_path)])
    iroha_cmd_prefix.extend(["-c", str(client_toml)])
    try:
        try:
            run_command(
                iroha_cmd_prefix
                + [
                    "ledger",
                    "account",
                    "register",
                    "--id",
                    account_id,
                ]
            )
            status = "created-local"
        except RuntimeError as exc:
            message = str(exc).lower()
            if "already exists" not in message and "409" not in message:
                raise
            status = "existing-local"

        for permission in normalize_permissions(permissions):
            try:
                run_command(
                    iroha_cmd_prefix
                    + [
                        "account",
                        "permission",
                        "grant",
                        "--id",
                        account_id,
                    ],
                    input_text=json.dumps({"name": permission, "payload": {}}),
                )
            except RuntimeError as exc:
                message = str(exc).lower()
                if "already exists" not in message and "duplicate" not in message:
                    raise
    finally:
        if metadata_path is not None:
            metadata_path.unlink(missing_ok=True)

    return {
        "status": status,
        "response_status": 200,
        "response": {
            "account_id": account_id,
            "registrar_account_id": registrar_account_id,
            "registrar_client_toml": str(client_toml),
        },
    }


def fund_account_with_local_registrar(
    *,
    client_toml: Path,
    iroha_bin: str | None,
    chain_discriminant: int,
    account_id: str,
    asset_definition_id: str,
    quantity: str,
    gas_asset_id: str | None = None,
    gas_limit: int | None = None,
) -> dict[str, Any]:
    registrar_account_id, _ = load_local_registrar(
        client_toml, chain_discriminant, iroha_bin
    )
    iroha_cli = resolve_iroha_bin(iroha_bin)
    metadata_path = write_cli_metadata_file(
        gas_asset_id=gas_asset_id,
        gas_limit=gas_limit,
        prefix="taira_local_funding_metadata_",
    )
    iroha_cmd_prefix = [iroha_cli]
    if metadata_path is not None:
        iroha_cmd_prefix.extend(["--metadata", str(metadata_path)])
    iroha_cmd_prefix.extend(["-c", str(client_toml)])
    try:
        run_command(
            iroha_cmd_prefix
            + [
                "ledger",
                "asset",
                "transfer",
                "--definition",
                asset_definition_id,
                "--account",
                registrar_account_id,
                "--to",
                account_id,
                "--quantity",
                quantity,
            ]
        )
    finally:
        if metadata_path is not None:
            metadata_path.unlink(missing_ok=True)
    return {
        "status": "funded-local",
        "response_status": 200,
        "response": {
            "account_id": account_id,
            "registrar_account_id": registrar_account_id,
            "asset_definition_id": asset_definition_id,
            "quantity": quantity,
        },
    }


def normalize_permissions(values: list[str]) -> list[str]:
    normalized: list[str] = []
    seen: set[str] = set()
    for raw in values:
        value = raw.strip()
        if not value or value in seen:
            continue
        seen.add(value)
        normalized.append(value)
    return normalized


def onboard_account(
    torii_root: str,
    alias: str,
    public_key_hex: str,
    *,
    permissions: list[str] | None = None,
    gas_asset_id: str | None = None,
    gas_limit: int | None = None,
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "alias": alias,
        "public_key_hex": public_key_hex,
        "uaid": derive_canary_uaid(public_key_hex),
    }
    requested_permissions = normalize_permissions(list(permissions or []))
    if requested_permissions:
        payload["permissions"] = requested_permissions
    if isinstance(gas_asset_id, str) and gas_asset_id.strip():
        payload["gas_asset_id"] = gas_asset_id.strip()
    if isinstance(gas_limit, int) and gas_limit > 0:
        payload["gas_limit"] = gas_limit
    status, payload = _http_json(
        "POST",
        f"{torii_root.rstrip('/')}/v1/accounts/onboard",
        payload,
    )
    if status in (200, 202):
        return {
            "status": "created",
            "response_status": status,
            "response": payload,
        }

    rendered = payload if isinstance(payload, str) else json.dumps(payload, sort_keys=True)
    if status == 400 and "account already exists" in rendered:
        return {
            "status": "existing",
            "response_status": status,
            "response": payload,
        }

    raise RuntimeError(f"account onboarding failed: status={status} body={payload!r}")


def derive_canary_uaid(public_key_hex: str) -> str:
    digest = bytearray(
        hashlib.blake2b(
            f"taira-canary-account:{public_key_hex.strip().lower()}".encode("utf-8"),
            digest_size=32,
        ).digest()
    )
    digest[-1] |= 1
    return f"uaid:{digest.hex()}"


def resolve_alias_account_id(torii_root: str, alias: str) -> str:
    status, payload = _http_json(
        "POST",
        f"{torii_root.rstrip('/')}/v1/aliases/resolve",
        {"alias": alias},
    )
    if status == 200 and isinstance(payload, dict):
        account_id = payload.get("account_id")
        if isinstance(account_id, str) and account_id:
            return account_id
    raise RuntimeError(f"alias resolve failed: status={status} body={payload!r}")


def transaction_status_kind(payload: Any) -> str | None:
    if not isinstance(payload, dict):
        return None
    status = payload.get("status")
    if isinstance(status, str) and status:
        return status
    if isinstance(status, dict):
        kind = status.get("kind")
        if isinstance(kind, str) and kind:
            return kind
    summary = payload.get("summary")
    if isinstance(summary, str) and summary:
        return summary
    return None


def wait_for_transaction_status(
    torii_root: str,
    tx_hash_hex: str,
    *,
    timeout_ms: int,
    poll_interval_ms: int = 1_000,
) -> dict[str, Any] | None:
    deadline = time.monotonic() + max(timeout_ms, 0) / 1000.0
    last_payload: Any = None
    while time.monotonic() < deadline:
        status, payload = _http_json(
            "GET",
            (
                f"{torii_root.rstrip('/')}/v1/pipeline/transactions/status"
                f"?hash={tx_hash_hex}&scope=global"
            ),
        )
        if status == 404:
            time.sleep(poll_interval_ms / 1000.0)
            continue
        last_payload = payload
        kind = transaction_status_kind(payload)
        if status in (200, 202) and kind in {"Applied", "Committed"}:
            return payload if isinstance(payload, dict) else None
        if kind in {"Rejected", "Expired"}:
            return payload if isinstance(payload, dict) else None
        time.sleep(poll_interval_ms / 1000.0)
    return last_payload if isinstance(last_payload, dict) else None


def attempt_faucet(
    account_id: str,
    torii_root: str,
    *,
    gas_asset_id: str | None = None,
    gas_limit: int | None = None,
    chain_discriminant: int = DEFAULT_CHAIN_DISCRIMINANT,
    iroha_bin: str | None = None,
    status_timeout_ms: int = DEFAULT_STATUS_TIMEOUT_MS,
) -> dict[str, Any]:
    puzzle_status, puzzle = _http_json(
        "GET",
        f"{torii_root.rstrip('/')}/v1/accounts/faucet/puzzle",
    )
    if puzzle_status != 200 or not isinstance(puzzle, dict):
        return {
            "status": "skipped",
            "response_status": puzzle_status,
            "response": puzzle,
        }

    claim_body = solve_puzzle(account_id, puzzle)
    claim_status, claim = _http_json(
        "POST",
        f"{torii_root.rstrip('/')}/v1/accounts/faucet",
        claim_body,
    )
    if claim_status in (200, 202):
        status = faucet_claim_status_kind(claim)
        if status != "Applied":
            tx_hash_hex = claim.get("tx_hash_hex") if isinstance(claim, dict) else None
            if isinstance(tx_hash_hex, str) and tx_hash_hex:
                final_status = wait_for_transaction_status(
                    torii_root,
                    tx_hash_hex,
                    timeout_ms=status_timeout_ms,
                )
                final_kind = transaction_status_kind(final_status)
                if final_kind in {"Applied", "Committed"}:
                    return {
                        "status": "claimed",
                        "response_status": claim_status,
                        "request": claim_body,
                        "response": claim,
                        "final_status": final_status,
                    }
                status = final_kind or status
            return {
                "status": "failed",
                "response_status": claim_status,
                "request": claim_body,
                "response": claim,
                "last_status": status or "not_observed",
            }
        return {
            "status": "claimed",
            "response_status": claim_status,
            "request": claim_body,
            "response": claim,
        }

    return {
        "status": "failed",
        "response_status": claim_status,
        "request": claim_body,
        "response": claim,
    }


def faucet_claim_status_kind(claim: Any) -> str | None:
    if not isinstance(claim, dict):
        return None
    status = claim.get("status")
    if isinstance(status, str) and status:
        return status
    if isinstance(status, dict):
        kind = status.get("kind")
        if isinstance(kind, str) and kind:
            return kind
    final_status = claim.get("final_status")
    if isinstance(final_status, dict):
        body = final_status.get("body")
        if isinstance(body, dict):
            status_obj = body.get("status")
            if isinstance(status_obj, dict):
                kind = status_obj.get("kind")
                if isinstance(kind, str) and kind:
                    return kind
    return None


def write_config(
    path: Path,
    *,
    chain: str,
    torii_root: str,
    domain: str,
    account_id: str,
    public_key: str,
    private_key: str,
    chain_discriminant: int,
    basic_auth: dict[str, Any] | None,
    time_to_live_ms: int,
    status_timeout_ms: int,
    nonce: bool,
) -> None:
    lines = [
        "# Auto-generated runtime-only rollout canary config.",
        f'chain = "{chain}"',
        f'torii_url = "{torii_root.rstrip("/")}/"',
    ]

    if basic_auth:
        web_login = basic_auth.get("web_login")
        password = basic_auth.get("password")
        if isinstance(web_login, str) and isinstance(password, str):
            lines.extend(
                [
                    "",
                    "[basic_auth]",
                    f'web_login = "{web_login}"',
                    f'password = "{password}"',
                ]
            )

    lines.extend(
        [
            "",
            "[account]",
            f'id = "{account_id}"',
            f'domain = "{domain}"',
            f'public_key = "{public_key}"',
            f'private_key = "{private_key}"',
            f"chain_discriminant = {chain_discriminant}",
            "",
            "[transaction]",
            f"time_to_live_ms = {time_to_live_ms}",
            f"status_timeout_ms = {status_timeout_ms}",
            f"nonce = {'true' if nonce else 'false'}",
            "",
        ]
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w",
        encoding="utf-8",
        dir=str(path.parent),
        delete=False,
    ) as handle:
        handle.write("\n".join(lines))
        temp_path = Path(handle.name)
    os.chmod(temp_path, 0o600)
    temp_path.replace(path)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate and onboard a runtime-only Taira rollout canary config."
    )
    parser.add_argument("--torii-root", required=True, help="Torii root URL")
    parser.add_argument(
        "--output-config",
        required=True,
        help="Runtime-only client config path to create or refresh",
    )
    parser.add_argument("--iroha-bin", help="Optional iroha binary or command name")
    parser.add_argument(
        "--chain-id",
        default=DEFAULT_CHAIN_ID,
        help=f"Chain id written into the generated config (default: {DEFAULT_CHAIN_ID})",
    )
    parser.add_argument(
        "--chain-discriminant",
        type=int,
        default=DEFAULT_CHAIN_DISCRIMINANT,
        help=f"I105 chain discriminant (default: {DEFAULT_CHAIN_DISCRIMINANT})",
    )
    parser.add_argument("--domain", default=DEFAULT_DOMAIN, help="Account domain")
    parser.add_argument(
        "--alias-prefix",
        default=DEFAULT_ALIAS_PREFIX,
        help="Alias prefix used when onboarding a new signer",
    )
    parser.add_argument(
        "--permission",
        dest="permissions",
        action="append",
        default=[],
        help="Permission token to request during onboarding (repeatable)",
    )
    parser.add_argument(
        "--time-to-live-ms",
        type=int,
        default=DEFAULT_TIME_TO_LIVE_MS,
        help=f"Rollout-specific transaction TTL (default: {DEFAULT_TIME_TO_LIVE_MS})",
    )
    parser.add_argument(
        "--status-timeout-ms",
        type=int,
        default=DEFAULT_STATUS_TIMEOUT_MS,
        help=f"Rollout-specific status timeout (default: {DEFAULT_STATUS_TIMEOUT_MS})",
    )
    parser.add_argument(
        "--skip-faucet",
        action="store_true",
        help="Only generate/onboard the signer and skip the initial faucet attempt",
    )
    parser.add_argument(
        "--gas-asset-id",
        default=None,
        help="Optional gas asset definition id to attach to onboarding transactions",
    )
    parser.add_argument(
        "--gas-limit",
        type=int,
        default=None,
        help="Optional gas limit to attach to onboarding transactions",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo_root = SCRIPT_DIR.parent
    output_config = Path(args.output_config).expanduser()
    existing = load_existing_config(output_config)

    if existing is None:
        public_key, private_key, public_key_raw_hex = generate_ed25519_keypair()
        generated_keys = True
        basic_auth = None
        nonce = False
        chain = args.chain_id
        chain_discriminant = args.chain_discriminant
        domain = normalize_domain(args.domain)
    else:
        public_key = existing["public_key"]
        private_key = existing["private_key"]
        public_key_raw_hex = existing["public_key_raw_hex"]
        generated_keys = False
        basic_auth = existing["basic_auth"]
        nonce = existing["nonce"]
        # Reuse the generated key material, but honor the current runtime arguments so a
        # redeployed network can refresh chain/domain settings without forcing a new signer.
        chain = args.chain_id
        chain_discriminant = int(args.chain_discriminant)
        domain = normalize_domain(args.domain)

    alias = build_alias(args.alias_prefix, public_key, domain)
    account_id = None
    try:
        onboarding = onboard_account(
            args.torii_root,
            alias,
            public_key_raw_hex,
            permissions=args.permissions,
            gas_asset_id=args.gas_asset_id,
            gas_limit=args.gas_limit,
        )
        response = onboarding.get("response")
        if isinstance(response, dict):
            value = response.get("account_id")
            if isinstance(value, str) and value:
                account_id = value
    except RuntimeError as onboarding_error:
        raise RuntimeError(
            "account onboarding failed; faucet funding was not attempted"
        ) from onboarding_error
    if account_id is None:
        account_id = resolve_alias_account_id(args.torii_root, alias)
    faucet = (
        {"status": "skipped"}
        if args.skip_faucet
        else attempt_faucet(
            account_id,
            args.torii_root,
            gas_asset_id=args.gas_asset_id,
            gas_limit=args.gas_limit,
            chain_discriminant=chain_discriminant,
            iroha_bin=args.iroha_bin,
            status_timeout_ms=args.status_timeout_ms,
        )
    )
    if not args.skip_faucet and faucet.get("status") != "claimed":
        raise RuntimeError(
            f"faucet funding did not reach Applied finality: {faucet!r}"
        )

    write_config(
        output_config,
        chain=chain,
        torii_root=args.torii_root,
        domain=domain,
        account_id=account_id,
        public_key=public_key,
        private_key=private_key,
        chain_discriminant=chain_discriminant,
        basic_auth=basic_auth,
        time_to_live_ms=args.time_to_live_ms,
        status_timeout_ms=args.status_timeout_ms,
        nonce=nonce,
    )

    print(
        json.dumps(
            {
                "config_path": str(output_config),
                "generated_keys": generated_keys,
                "account_id": account_id,
                "alias": alias,
                "public_key": public_key,
                "onboarding": onboarding,
                "faucet": faucet,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
