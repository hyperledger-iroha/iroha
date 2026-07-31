#!/usr/bin/env python3
"""Verify Python's generated Norito RPC descriptor mirrors."""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path
from typing import Iterable, List, Tuple

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


def compare(
    source: Path, target: Path
) -> Tuple[List[Path], List[Path], List[Tuple[Path, Path]]]:
    for root in (source, target):
        if not root.is_dir():
            raise FileNotFoundError(f"missing directory: {root}")

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
    except FileNotFoundError as exc:
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
