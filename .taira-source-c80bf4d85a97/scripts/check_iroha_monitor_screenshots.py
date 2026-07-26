#!/usr/bin/env python3
"""
Verify or refresh iroha_monitor demo screenshot checksums.

The monitor docs ship four deterministic artefacts under
docs/source/images/iroha_monitor_demo/:

- iroha_monitor_demo_overview.ans
- iroha_monitor_demo_overview.svg
- iroha_monitor_demo_pipeline.ans
- iroha_monitor_demo_pipeline.svg

This helper checks that the files exist and match the recorded SHA-256 digests
or, when invoked with --update, rewrites the checksum manifest to reflect the
current artefacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Dict, List

DEFAULT_FILES = [
    "iroha_monitor_demo_overview.ans",
    "iroha_monitor_demo_overview.svg",
    "iroha_monitor_demo_pipeline.ans",
    "iroha_monitor_demo_pipeline.svg",
]


def compute_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_manifest(path: Path) -> Dict[str, str]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            data = json.load(handle)
    except FileNotFoundError as err:  # noqa: PERF203 - clarity over micro-optimisation
        raise SystemExit(f"checksum manifest missing: {path}") from err
    if not isinstance(data, dict) or "files" not in data:
        raise SystemExit(f"checksum manifest {path} must contain a `files` object")
    files = data["files"]
    if not isinstance(files, dict):
        raise SystemExit(f"checksum manifest {path} must map file names to digests")
    manifest: Dict[str, str] = {}
    for key, value in files.items():
        if not isinstance(key, str) or not isinstance(value, str):
            raise SystemExit(f"invalid checksum entry {key!r}: expected string digest")
        manifest[key] = value
    return manifest


def write_manifest(path: Path, digests: Dict[str, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {"files": digests}
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")


def resolve_expected(record_path: Path, explicit: List[str] | None) -> List[str]:
    if explicit:
        return explicit
    if record_path.exists():
        manifest = load_manifest(record_path)
        return list(manifest.keys())
    return list(DEFAULT_FILES)


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check iroha_monitor screenshot digests against a recorded manifest.",
    )
    parser.add_argument(
        "--dir",
        type=Path,
        default=Path("docs/source/images/iroha_monitor_demo"),
        help="Directory containing the screenshot artefacts",
    )
    parser.add_argument(
        "--record",
        type=Path,
        default=None,
        help="Path to the checksum manifest (defaults to <dir>/checksums.json)",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Rewrite the checksum manifest to reflect the current artefacts",
    )
    parser.add_argument(
        "--file",
        dest="files",
        action="append",
        default=None,
        help="Additional artefact to include (may be repeated); defaults to the four demo assets",
    )
    args = parser.parse_args(argv)

    artefact_dir = args.dir
    record_path = args.record or (artefact_dir / "checksums.json")
    expected_files = resolve_expected(record_path, args.files)

    digests: Dict[str, str] = {}
    missing: List[str] = []
    for name in expected_files:
        path = artefact_dir / name
        if not path.exists():
            missing.append(str(path))
            continue
        digests[name] = compute_sha256(path)

    if missing:
        for path in missing:
            print(f"missing artefact: {path}")
        return 1

    if args.update:
        write_manifest(record_path, digests)
        print(f"updated checksum manifest: {record_path}")
        return 0

    manifest = load_manifest(record_path)
    failures = 0
    for name, digest in digests.items():
        recorded = manifest.get(name)
        if recorded is None:
            print(f"checksum not recorded for {name}")
            failures += 1
            continue
        if recorded != digest:
            print(f"checksum mismatch for {name}: recorded {recorded}, actual {digest}")
            failures += 1

    for extra in sorted(set(manifest.keys()) - set(digests.keys())):
        print(f"recorded checksum has no matching artefact: {extra}")
        failures += 1

    if failures:
        return 1

    print(f"iroha_monitor screenshots verified against {record_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
