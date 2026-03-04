#!/usr/bin/env python3
"""Replay security corpora and assert fingerprints stay stable."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import sys
from pathlib import Path

try:
    import resource
except ImportError:  # pragma: no cover - resource unavailable on Windows
    resource = None  # type: ignore


DEFAULT_CPU_SECS = int(os.environ.get("REPLAY_CPU_SECS", "120"))
DEFAULT_MEM_MB = int(os.environ.get("REPLAY_MEM_MB", "1024"))


def clamp_resources(cpu_secs: int, mem_mb: int) -> None:
    """Apply soft resource limits when supported."""

    if resource is None:
        return

    try:
        resource.setrlimit(resource.RLIMIT_CPU, (cpu_secs, cpu_secs))
    except (ValueError, OSError):
        pass

    try:
        limit_bytes = mem_mb * 1024 * 1024
        resource.setrlimit(resource.RLIMIT_AS, (limit_bytes, limit_bytes))
    except (ValueError, OSError):
        pass


def canonical_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def compute_fingerprint(entry: dict[str, object], fuzz_root: Path) -> str:
    rel_path = entry["path"]
    if not isinstance(rel_path, str):
        raise TypeError("metadata path must be a string")
    corpus_path = fuzz_root / rel_path
    if not corpus_path.exists():
        raise FileNotFoundError(f"corpus not found: {corpus_path}")

    suffix = corpus_path.suffix.lower()
    if suffix == ".json":
        data = json.loads(corpus_path.read_text(encoding="utf-8"))
        payload = canonical_json(data).encode("utf-8")
    elif suffix == ".b64":
        raw = corpus_path.read_bytes()
        try:
            payload = base64.b64decode(raw, validate=True)
        except binascii.Error:
            payload = base64.b64decode(raw, validate=False)
    else:
        payload = corpus_path.read_bytes()

    return hashlib.blake2b(payload, digest_size=32).hexdigest()


def load_metadata(path: Path) -> list[dict[str, object]]:
    with path.open("r", encoding="utf-8") as fh:
        metadata = json.load(fh)
    if not isinstance(metadata, list):
        raise ValueError("metadata must be a JSON array")
    for entry in metadata:
        if not isinstance(entry, dict):
            raise ValueError("metadata entries must be objects")
        if "expected_validation_fail" not in entry:
            raise ValueError(f"missing expected_validation_fail for {entry.get('path')}")
    return metadata  # type: ignore[return-value]


def save_metadata(path: Path, data: list[dict[str, object]]) -> None:
    path.write_text(json.dumps(data, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, default=None, help="Path to corpora metadata JSON")
    parser.add_argument("--update", action="store_true", help="Rewrite metadata with fresh fingerprints")
    args = parser.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[1]
    metadata_path = args.metadata or (repo_root / "fuzz" / "corpora.json")
    fuzz_root = metadata_path.parent

    clamp_resources(DEFAULT_CPU_SECS, DEFAULT_MEM_MB)

    entries = load_metadata(metadata_path)
    mismatches: list[str] = []

    for entry in entries:
        fingerprint = compute_fingerprint(entry, fuzz_root)
        stored = entry.get("fingerprint")
        if args.update:
            entry["fingerprint"] = fingerprint
        else:
            if stored is None or stored == "":
                mismatches.append(f"missing fingerprint for {entry.get('path')}")
            elif not isinstance(stored, str):
                mismatches.append(f"fingerprint for {entry.get('path')} must be a string")
            elif stored != fingerprint:
                mismatches.append(
                    f"fingerprint mismatch for {entry.get('path')}: expected {stored} got {fingerprint}"
                )

    if args.update:
        save_metadata(metadata_path, entries)
        print(f"Updated fingerprints for {len(entries)} corpora in {metadata_path}")
        return 0

    if mismatches:
        for line in mismatches:
            print(f"[security-replay] {line}", file=sys.stderr)
        return 1

    print(f"Verified {len(entries)} corpora fingerprints from {metadata_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
