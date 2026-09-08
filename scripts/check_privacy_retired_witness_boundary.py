#!/usr/bin/env python3
"""Reject orphan confidential witness producers from every shipping SDK source."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys


RETIRED_PRODUCERS = (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyConfidentialWitness.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyConfidentialWitness.kt",
    "IrohaSwift/Sources/IrohaSwift/PrivacyConfidentialWitness.swift",
    "crates/connect_norito_bridge/src/privacy_production.rs",
)
PRODUCTION_ROOTS = (
    "java/iroha_android/src/main",
    "kotlin/core-jvm/src/main",
    "kotlin/client-android/src/main",
    "kotlin/kagemusha-wallet-android/src/main",
    "IrohaSwift/Sources",
    "javascript/iroha_js/src",
    "python/iroha_python/src/iroha_python",
    "python/iroha_native/src",
    "csharp/src",
    "crates/connect_norito_bridge/src",
)
SOURCE_SUFFIXES = frozenset((".java", ".kt", ".swift", ".js", ".ts", ".py", ".cs", ".rs", ".h"))
RETIRED_MARKERS = (
    "PrivacyConfidentialWitness",
    "PrivacyConfidentialNoteWitness",
    "PrivacyConfidentialTransferOutputWitness",
    "PrivacyConfidentialUnshieldChangeWitness",
    "PrivacyConfidentialMerklePathWitness",
    "privacyConfidentialWitnessV1WireName",
    "privacyConfidentialWitnessV2WireName",
    "connect_norito_bridge::privacy_production::",
)


def check(root: Path) -> tuple[str, ...]:
    """Check removal, including relocated producers and test fixtures copied into production."""
    errors: list[str] = []

    def walk_error(error: OSError) -> None:
        errors.append(f"production source traversal failed: {error.filename}")

    for relative in RETIRED_PRODUCERS:
        path = root / relative
        if path.exists() or path.is_symlink():
            errors.append(f"retired confidential witness producer must remain absent: {relative}")
    for relative in PRODUCTION_ROOTS:
        directory = root / relative
        if not directory.is_dir() or directory.is_symlink():
            errors.append(f"missing or symlinked production source root: {relative}")
            continue
        for parent, directories, names in os.walk(directory, followlinks=False, onerror=walk_error):
            for name in sorted(directories):
                path = Path(parent) / name
                if path.is_symlink():
                    errors.append(f"symlinked production source directory: {path.relative_to(root)}")
            for name in sorted(names):
                path = Path(parent) / name
                if path.suffix not in SOURCE_SUFFIXES:
                    continue
                if path.is_symlink():
                    errors.append(f"symlinked production source file: {path.relative_to(root)}")
                    continue
                try:
                    source = path.read_text(encoding="utf-8")
                except (OSError, UnicodeError):
                    errors.append(f"unreadable production source: {path.relative_to(root)}")
                    continue
                for marker in RETIRED_MARKERS:
                    if marker in source:
                        errors.append(f"retired confidential witness surface {marker}: {path.relative_to(root)}")
    return tuple(sorted(errors))


def main() -> int:
    """Check the requested source tree without loading an SDK or native library."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    errors = check(args.root)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("retired confidential witness production boundary passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
