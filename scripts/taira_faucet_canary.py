#!/usr/bin/env python3
"""Claim the Taira account faucet for a rollout canary account."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any
from urllib import error, request


FAUCET_POW_DOMAIN_SEPARATOR = b"iroha:accounts:faucet:pow:v2"


def leading_zero_bits(data: bytes) -> int:
    """Count leading zero bits in a byte string."""

    total = 0
    for byte in data:
        if byte == 0:
            total += 8
            continue
        total += 8 - byte.bit_length()
        break
    return total


def build_challenge(
    account_id: str,
    anchor_height: int,
    anchor_block_hash_hex: str,
    challenge_salt_hex: str | None,
) -> bytes:
    """Build the faucet PoW challenge digest."""

    hasher = hashlib.sha256()
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR)
    hasher.update(account_id.encode("utf-8"))
    hasher.update(anchor_height.to_bytes(8, byteorder="big", signed=False))
    hasher.update(bytes.fromhex(anchor_block_hash_hex))
    if challenge_salt_hex:
        hasher.update(bytes.fromhex(challenge_salt_hex))
    return hasher.digest()


def solve_puzzle(account_id: str, puzzle: dict[str, Any]) -> dict[str, Any]:
    """Solve the Taira faucet PoW challenge and build the request body."""

    difficulty_bits = int(puzzle["difficulty_bits"])
    body = {"account_id": account_id}
    if difficulty_bits <= 0:
        return body

    challenge = build_challenge(
        account_id=account_id,
        anchor_height=int(puzzle["anchor_height"]),
        anchor_block_hash_hex=str(puzzle["anchor_block_hash_hex"]),
        challenge_salt_hex=(
            str(puzzle["challenge_salt_hex"])
            if puzzle.get("challenge_salt_hex") is not None
            else None
        ),
    )
    n = 1 << int(puzzle["scrypt_log_n"])
    r = int(puzzle["scrypt_r"])
    p = int(puzzle["scrypt_p"])

    for nonce in range(0, 1 << 63):
        nonce_bytes = nonce.to_bytes(8, byteorder="big", signed=False)
        digest = hashlib.scrypt(
            nonce_bytes,
            salt=challenge,
            n=n,
            r=r,
            p=p,
            dklen=32,
        )
        if leading_zero_bits(digest) >= difficulty_bits:
            body["pow_anchor_height"] = int(puzzle["anchor_height"])
            body["pow_nonce_hex"] = nonce_bytes.hex()
            return body

    raise RuntimeError("faucet PoW nonce space exhausted")


def _http_json(method: str, url: str, payload: dict[str, Any] | None = None) -> tuple[int, Any]:
    data = None
    headers = {}
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
        parsed: Any
        try:
            parsed = json.loads(body) if body else None
        except json.JSONDecodeError:
            parsed = body
        return exc.code, parsed


def claim_faucet(account_id: str, torii_root: str) -> dict[str, Any]:
    """Fetch the current puzzle, solve it, and submit the faucet claim."""

    puzzle_status, puzzle = _http_json("GET", f"{torii_root.rstrip('/')}/v1/accounts/faucet/puzzle")
    if puzzle_status != 200 or not isinstance(puzzle, dict):
        raise RuntimeError(f"failed to fetch faucet puzzle: status={puzzle_status} body={puzzle!r}")
    body = solve_puzzle(account_id, puzzle)
    claim_status, claim = _http_json(
        "POST",
        f"{torii_root.rstrip('/')}/v1/accounts/faucet",
        body,
    )
    if claim_status not in (200, 202):
        raise RuntimeError(f"faucet claim failed: status={claim_status} body={claim!r}")
    return {
        "puzzle": puzzle,
        "request": body,
        "response_status": claim_status,
        "response": claim,
    }


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint."""

    parser = argparse.ArgumentParser(description="Claim the Taira faucet for a canary account.")
    parser.add_argument("--account-id", required=True, help="canonical I105 account id")
    parser.add_argument("--torii-root", required=True, help="Torii root URL, for example https://taira.sora.org")
    args = parser.parse_args(argv)

    result = claim_faucet(args.account_id, args.torii_root)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
