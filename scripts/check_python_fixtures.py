#!/usr/bin/env python3
"""Verify Python's generated Norito RPC descriptor mirrors."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
from pathlib import Path
from typing import Iterable, List, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from norito_fixture_frame import (
    SIGNED_TRANSACTION_SCHEMA,
    TRANSACTION_PAYLOAD_SCHEMA,
    decode_canonical_norito_frame,
    iroha_hash_hex,
    signed_transaction_entrypoint_hash_hex,
    signed_transaction_payload,
)

DEFAULT_SOURCE = Path("fixtures/norito_rpc")
DEFAULT_TARGET = Path("python/iroha_python/tests/fixtures")
MANAGED_FIXTURES = (
    Path("transaction_payloads.json"),
    Path("transaction_fixtures.manifest.json"),
)


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def validate_canonical_frames(manifest_path: Path) -> None:
    document = json.loads(manifest_path.read_text(encoding="utf-8"))
    fixtures = document.get("fixtures") if isinstance(document, dict) else None
    if not isinstance(fixtures, list):
        raise ValueError(f"invalid fixture manifest: {manifest_path}")
    for entry in fixtures:
        if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
            raise ValueError(f"invalid fixture entry in {manifest_path}")
        name = entry["name"]
        try:
            payload_frame = base64.b64decode(entry["payload_base64"], validate=True)
            signed_frame = base64.b64decode(entry["signed_base64"], validate=True)
        except (KeyError, TypeError, ValueError) as exc:
            raise ValueError(f"{name}: invalid fixture base64") from exc
        if base64.b64encode(payload_frame).decode("ascii") != entry["payload_base64"]:
            raise ValueError(f"{name}: non-canonical payload base64")
        if base64.b64encode(signed_frame).decode("ascii") != entry["signed_base64"]:
            raise ValueError(f"{name}: non-canonical signed base64")
        payload_bare = decode_canonical_norito_frame(
            payload_frame,
            f"{name}.payload_base64",
            expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
        )
        signed_bare = decode_canonical_norito_frame(
            signed_frame,
            f"{name}.signed_base64",
            expected_schema=SIGNED_TRANSACTION_SCHEMA,
        )
        if iroha_hash_hex(payload_frame) != entry.get("payload_hash"):
            raise ValueError(f"{name}: payload_hash does not authenticate the framed bytes")
        if signed_transaction_payload(signed_bare) != payload_bare:
            raise ValueError(f"{name}: signed transaction does not contain its payload")
        if signed_transaction_entrypoint_hash_hex(signed_bare) != entry.get("signed_hash"):
            raise ValueError(f"{name}: signed_hash does not match compact External semantics")


def compare(
    source: Path, target: Path
) -> Tuple[List[Path], List[Path], List[Tuple[Path, Path]]]:
    for root in (source, target):
        if not root.is_dir():
            raise FileNotFoundError(f"missing directory: {root}")

    validate_canonical_frames(source / "transaction_fixtures.manifest.json")

    source_map = {}
    target_map = {}
    for relative in MANAGED_FIXTURES:
        source_path = source / relative
        if not source_path.is_file():
            raise FileNotFoundError(f"missing canonical fixture: {source_path}")
        source_map[relative] = source_path
        target_path = target / relative
        if target_path.is_file():
            target_map[relative] = target_path

    missing = sorted(rel for rel in source_map if rel not in target_map)
    extra = sorted(
        path.relative_to(target)
        for path in target.rglob("*.norito")
        if path.is_file()
    )

    diffs: List[Tuple[Path, Path]] = []
    for rel, src_path in source_map.items():
        tgt_path = target_map.get(rel)
        if tgt_path is None:
            continue
        if fingerprint(src_path) != fingerprint(tgt_path):
            diffs.append((src_path, tgt_path))
    return missing, extra, diffs


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check Python Norito RPC descriptor parity"
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_SOURCE,
        help=f"Canonical fixture directory (default: {DEFAULT_SOURCE})",
    )
    parser.add_argument(
        "--target",
        type=Path,
        default=DEFAULT_TARGET,
        help=f"Python fixture directory (default: {DEFAULT_TARGET})",
    )
    parser.add_argument(
        "--quiet", action="store_true", help="Suppress success output"
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        missing, extra, diffs = compare(args.source, args.target)
    except (FileNotFoundError, ValueError) as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    has_error = False
    if missing:
        has_error = True
        print("[error] missing files in target:")
        for rel in missing:
            print(f"    {rel}")
    if extra:
        has_error = True
        print("[error] unexpected files in target:")
        for rel in extra:
            print(f"    {rel}")
    if diffs:
        has_error = True
        print("[error] content mismatches:")
        for src, tgt in diffs:
            rel = tgt.relative_to(args.target)
            print(f"    {rel} (source={src}, target={tgt})")

    if has_error:
        return 1

    if not args.quiet:
        print(f"[ok] Norito RPC descriptors match between {args.source} and {args.target}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
