#!/usr/bin/env python3
"""Verify Python fixture parity with the canonical Android set."""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path
from typing import Iterable, List, Tuple

DEFAULT_SOURCE = Path("java/iroha_android/src/test/resources")
DEFAULT_TARGET = Path("python/iroha_python/tests/fixtures")


def collect_files(root: Path) -> List[Path]:
    if not root.exists():
        raise FileNotFoundError(f"missing directory: {root}")
    files = [p for p in root.rglob("*") if p.is_file()]
    files.sort()
    return files


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def compare(
    source: Path, target: Path
) -> Tuple[List[Path], List[Path], List[Tuple[Path, Path]]]:
    source_map = {p.relative_to(source): p for p in collect_files(source)}
    target_map = {p.relative_to(target): p for p in collect_files(target)}

    missing = sorted(rel for rel in source_map if rel not in target_map)
    extra = sorted(rel for rel in target_map if rel not in source_map)

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
        description="Check Python fixture parity with Android fixtures"
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
        print(f"[ok] Fixtures match between {args.source} and {args.target}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    sys.exit(main())
