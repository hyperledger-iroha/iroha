#!/usr/bin/env python3
"""Generate and onboard a runtime-only Taira rollout canary config."""

from __future__ import annotations

import argparse
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

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from taira_faucet_canary import solve_puzzle  # noqa: E402


DEFAULT_CHAIN_ID = "809574f5-fee7-5e69-bfcf-52451e42d50f"
DEFAULT_CHAIN_DISCRIMINANT = 369
DEFAULT_DOMAIN = "wonderland.universal"
DEFAULT_ALIAS_PREFIX = "taira-rollout-canary"
DEFAULT_TIME_TO_LIVE_MS = 120_000
DEFAULT_STATUS_TIMEOUT_MS = 120_000
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


def run_command(cmd: list[str], *, input_text: str | None = None) -> str:
    proc = subprocess.run(
        cmd,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
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


def onboard_account(torii_root: str, alias: str, public_key_hex: str) -> dict[str, Any]:
    status, payload = _http_json(
        "POST",
        f"{torii_root.rstrip('/')}/v1/accounts/onboard",
        {
            "alias": alias,
            "public_key_hex": public_key_hex,
        },
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


def attempt_faucet(account_id: str, torii_root: str) -> dict[str, Any]:
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
        return {
            "status": "claimed",
            "response_status": claim_status,
            "request": claim_body,
            "response": claim,
        }

    rendered = claim if isinstance(claim, str) else json.dumps(claim, sort_keys=True)
    if claim_status == 400 and "positive faucet asset balance" in rendered:
        return {
            "status": "already-funded",
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


def write_config(
    path: Path,
    *,
    chain: str,
    torii_root: str,
    domain: str,
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
        chain = existing["chain"]
        chain_discriminant = int(existing["chain_discriminant"])
        domain = existing["domain"]

    alias = build_alias(args.alias_prefix, public_key, domain)
    account_id = None
    try:
        onboarding = onboard_account(args.torii_root, alias, public_key_raw_hex)
        response = onboarding.get("response")
        if isinstance(response, dict):
            value = response.get("account_id")
            if isinstance(value, str) and value:
                account_id = value
    except RuntimeError as onboarding_error:
        account_id = resolve_alias_account_id(args.torii_root, alias)
        onboarding = {
            "status": "existing",
            "response_status": 400,
            "response": str(onboarding_error),
        }
    if account_id is None:
        account_id = resolve_alias_account_id(args.torii_root, alias)
    faucet = (
        {"status": "skipped"}
        if args.skip_faucet
        else attempt_faucet(account_id, args.torii_root)
    )

    write_config(
        output_config,
        chain=chain,
        torii_root=args.torii_root,
        domain=domain,
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
