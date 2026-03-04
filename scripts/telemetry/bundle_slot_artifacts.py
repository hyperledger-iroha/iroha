#!/usr/bin/env python3
"""Bundle slot-duration metrics + summary into a deterministic NX-18 evidence set."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Tuple


def sha256sum(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_artifact(source: Path, destination: Path) -> None:
    if source.resolve() == destination.resolve():
        return
    shutil.copy2(source, destination)


def parse_metadata(entries: Iterable[str]) -> Dict[str, str]:
    metadata: Dict[str, str] = {}
    for entry in entries:
        if "=" not in entry:
            raise ValueError(f"metadata entry '{entry}' must be key=value")
        key, raw_value = entry.split("=", 1)
        key = key.strip()
        value = raw_value.strip()
        if not key:
            raise ValueError(f"metadata entry '{entry}' has an empty key")
        metadata[key] = value
    return metadata


def build_manifest(records: List[Tuple[str, Path, Path]], metadata: Dict[str, str]) -> Dict[str, object]:
    artefacts: List[Dict[str, object]] = []
    for name, source, destination in records:
        size = destination.stat().st_size
        artefacts.append(
            {
                "name": name,
                "source": str(source),
                "path": str(destination),
                "size_bytes": size,
                "sha256": sha256sum(destination),
            }
        )
    manifest: Dict[str, object] = {
        "version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "artifacts": artefacts,
    }
    if metadata:
        manifest["metadata"] = metadata
    return manifest


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metrics", required=True, type=Path, help="Path to the Prometheus metrics file.")
    parser.add_argument("--summary", required=True, type=Path, help="Path to the slot summary JSON file.")
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("artifacts/nx18"),
        help="Directory where the bundled artifacts and manifest should be written.",
    )
    parser.add_argument(
        "--manifest-name",
        default="slot_bundle_manifest.json",
        help="Name of the manifest JSON file (default: slot_bundle_manifest.json).",
    )
    parser.add_argument(
        "--metadata",
        action="append",
        default=[],
        help="Optional metadata key=value entry to include in the manifest (repeatable).",
    )
    parser.add_argument(
        "--metrics-name",
        default="slot_metrics.prom",
        help="Filename to use when copying the metrics file into --out-dir.",
    )
    parser.add_argument(
        "--summary-name",
        default="slot_summary.json",
        help="Filename to use when copying the summary into --out-dir.",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    metrics = args.metrics.resolve()
    summary = args.summary.resolve()
    if not metrics.exists():
        raise SystemExit(f"[error] metrics file '{metrics}' does not exist")
    if not summary.exists():
        raise SystemExit(f"[error] summary file '{summary}' does not exist")
    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    metrics_dest = out_dir / args.metrics_name
    summary_dest = out_dir / args.summary_name
    copy_artifact(metrics, metrics_dest)
    copy_artifact(summary, summary_dest)

    metadata = parse_metadata(args.metadata)
    manifest = build_manifest(
        [
            ("metrics", metrics, metrics_dest),
            ("summary", summary, summary_dest),
        ],
        metadata,
    )
    manifest_path = out_dir / args.manifest_name
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"[nx18] bundled slot artifacts -> {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
