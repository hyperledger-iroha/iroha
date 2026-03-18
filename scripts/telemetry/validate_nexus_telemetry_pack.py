#!/usr/bin/env python3
"""Validate Nexus telemetry rehearsal artefacts and emit a signed manifest.

The script ensures the expected evidence files exist inside a telemetry pack,
computes SHA-256 digests for each artefact, and writes a manifest JSON file
alongside a `.sha256` companion so governance can cite the hashes directly in
their runbooks.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Tuple


DEFAULT_ARTIFACTS: Tuple[str, ...] = (
    "prometheus.tgz",
    "otlp.ndjson",
    "torii_structured_logs.jsonl",
    "B4-RB-2026Q1.log",
)


def sha256sum(path: Path) -> str:
    """Compute the SHA-256 hex digest for *path*."""

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_metadata(values: Iterable[str]) -> Dict[str, object]:
    """Parse `key=value` pairs supplied on the CLI."""

    metadata: Dict[str, object] = {}
    for entry in values:
        if "=" not in entry:
            raise ValueError(f"metadata entry '{entry}' is missing '='")
        key, raw_value = entry.split("=", 1)
        key = key.strip()
        value = raw_value.strip()
        if not key:
            raise ValueError(f"metadata entry '{entry}' is missing a key")
        metadata[key] = value
    return metadata


def parse_slot_range(value: str) -> Tuple[str, int, int]:
    """Validate and normalise ``slot_range`` hints."""

    raw = value.strip()
    if not raw:
        raise ValueError("slot range hint must be non-empty")
    if "-" not in raw:
        raise ValueError("slot range hint must be formatted as <start>-<end>")
    start_str, end_str = raw.split("-", 1)
    try:
        start = int(start_str)
        end = int(end_str)
    except ValueError as exc:
        raise ValueError("slot range bounds must be integers") from exc
    if start < 0 or end < 0:
        raise ValueError("slot range bounds must be non-negative")
    if end < start:
        raise ValueError("slot range end must be greater than or equal to start")
    canonical = f"{start}-{end}"
    return canonical, start, end


def normalize_workload_seed(value: str) -> str:
    """Return a stripped workload seed or raise when missing."""

    stripped = value.strip()
    if not stripped:
        raise ValueError("workload seed must be non-empty")
    return stripped


def build_manifest(
    pack_dir: Path,
    artifacts: Iterable[str],
    metadata: Dict[str, object],
) -> Dict[str, object]:
    """Return the manifest JSON payload for *pack_dir*."""

    records: List[Dict[str, object]] = []
    for artifact in artifacts:
        candidate = pack_dir / artifact
        if not candidate.exists() or not candidate.is_file():
            raise FileNotFoundError(f"missing required artefact: {artifact}")
        size = candidate.stat().st_size
        if size == 0:
            raise ValueError(f"artefact '{artifact}' is empty")
        records.append(
            {
                "name": artifact,
                "path": str(candidate),
                "size_bytes": size,
                "sha256": sha256sum(candidate),
            }
        )

    manifest: Dict[str, object] = {
        "version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "pack_dir": str(pack_dir),
        "artifacts": records,
    }

    if metadata:
        manifest["metadata"] = metadata

    return manifest


def write_json(path: Path, payload: Dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_digest(manifest_path: Path) -> None:
    data = manifest_path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    digest_path = manifest_path.with_suffix(manifest_path.suffix + ".sha256")
    digest_path.write_text(digest + "\n", encoding="utf-8")


def main(argv: List[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pack_dir", type=Path, help="Directory containing telemetry artefacts")
    parser.add_argument(
        "--manifest-out",
        type=Path,
        help="Path for the manifest JSON (defaults to pack_dir/telemetry_manifest.json)",
    )
    parser.add_argument(
        "--expected",
        action="append",
        dest="expected",
        help="Required artefact name (can be repeated; defaults to the Nexus rehearsal set)",
    )
    parser.add_argument(
        "--metadata",
        action="append",
        default=[],
        help="Attach extra metadata as key=value (e.g. workload_seed=NEXUS-REH-2026Q1)",
    )
    parser.add_argument(
        "--slot-range",
        help="Optional slot range hint (e.g. 820-860); stored under metadata",
    )
    parser.add_argument(
        "--workload-seed",
        help="Optional workload seed hint; stored under metadata",
    )
    parser.add_argument(
        "--require-slot-range",
        action="store_true",
        help="fail if slot range metadata is missing",
    )
    parser.add_argument(
        "--require-workload-seed",
        action="store_true",
        help="fail if workload seed metadata is missing",
    )

    args = parser.parse_args(argv)
    pack_dir = args.pack_dir.resolve()
    if not pack_dir.is_dir():
        raise SystemExit(f"pack directory '{pack_dir}' does not exist")

    expected = args.expected if args.expected else list(DEFAULT_ARTIFACTS)
    metadata = parse_metadata(args.metadata)
    slot_range_meta: Tuple[str, int, int] | None = None
    if args.slot_range:
        slot_range_meta = parse_slot_range(args.slot_range)
    if args.require_slot_range and slot_range_meta is None:
        raise SystemExit("error: slot range metadata is required (pass --slot-range)")
    if slot_range_meta is not None:
        canonical, start, end = slot_range_meta
        metadata.setdefault("slot_range", canonical)
        metadata.setdefault("slot_range_start", start)
        metadata.setdefault("slot_range_end", end)

    workload_seed: str | None = None
    if args.workload_seed:
        workload_seed = normalize_workload_seed(args.workload_seed)
    if args.require_workload_seed and workload_seed is None:
        raise SystemExit("error: workload seed metadata is required (pass --workload-seed)")
    if workload_seed is not None:
        metadata.setdefault("workload_seed", workload_seed)

    manifest = build_manifest(pack_dir, expected, metadata)
    manifest_path = (
        args.manifest_out.resolve() if args.manifest_out else pack_dir / "telemetry_manifest.json"
    )
    write_json(manifest_path, manifest)
    write_digest(manifest_path)

    print(f"wrote manifest to {manifest_path}")
    print(f"wrote digest to {manifest_path}.sha256")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (FileNotFoundError, ValueError) as exc:
        raise SystemExit(f"error: {exc}")
