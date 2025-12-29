#!/usr/bin/env python3
"""Collect dashboards/logs/manifests for a red-team drill scenario."""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import platform
import shutil
import socket
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Mapping, MutableMapping, Sequence

from . import red_team_common


_CATEGORY_DIRS = {
    "dashboards": "dashboards",
    "alerts": "alerts",
    "logs": "logs",
    "manifests": "manifests",
    "evidence": "evidence",
}


@dataclass(frozen=True)
class FileEntry:
    """Manifest entry describing an copied file."""

    category: str
    destination: Path
    sha256: str
    source: str


@dataclass(frozen=True)
class ExportResult:
    """Outcome for an evidence export run."""

    manifest_path: Path
    entries: Sequence[FileEntry]


class GrafanaClient:
    """Minimal Grafana API client used for dashboard exports."""

    def __init__(self, base_url: str, token: str | None = None, timeout: int = 30) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout

    def fetch_dashboard(self, uid: str) -> bytes:
        """Fetch dashboard JSON for the provided UID."""

        url = f"{self.base_url}/api/dashboards/uid/{uid}"
        req = urllib.request.Request(url)
        req.add_header("Accept", "application/json")
        if self.token:
            req.add_header("Authorization", f"Bearer {self.token}")
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:  # noqa: S310
            return resp.read()


def _compute_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as src:
        for chunk in iter(lambda: src.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _copy_local_file(source: Path, dest_dir: Path, *, overwrite: bool) -> Path:
    if not source.is_file():
        raise FileNotFoundError(f"{source} is not a file")
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / source.name
    if dest.exists() and not overwrite:
        raise FileExistsError(f"{dest} already exists (use --overwrite to replace)")
    shutil.copy2(source, dest)
    return dest


def _sanitize_filename(name: str) -> str:
    return "".join(ch if ch.isalnum() or ch in ("-", "_", ".") else "_" for ch in name)


def _write_bytes(dest_dir: Path, filename: str, payload: bytes, *, overwrite: bool) -> Path:
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / filename
    if dest.exists() and not overwrite:
        raise FileExistsError(f"{dest} already exists (use --overwrite to replace)")
    dest.write_bytes(payload)
    return dest


def _ingest_dashboard_spec(
    spec: str,
    *,
    dest_dir: Path,
    grafana: GrafanaClient | None,
    overwrite: bool,
) -> FileEntry:
    if spec.startswith("grafana:"):
        if grafana is None:
            raise ValueError(
                "grafana dashboard export requested but no Grafana URL/token configured"
            )
        body = spec[len("grafana:") :]
        if ":" in body:
            uid, alias = body.split(":", 1)
            filename = _sanitize_filename(alias or uid)
        else:
            uid = body
            filename = f"dashboard_{uid}"
        payload = grafana.fetch_dashboard(uid)
        dest = _write_bytes(
            dest_dir,
            f"{filename}.json",
            payload,
            overwrite=overwrite,
        )
        return FileEntry(
            category="dashboards",
            destination=dest,
            sha256=_compute_sha256(dest),
            source=spec,
        )
    path = Path(spec)
    dest = _copy_local_file(path, dest_dir, overwrite=overwrite)
    return FileEntry(
        category="dashboards",
        destination=dest,
        sha256=_compute_sha256(dest),
        source=str(path),
    )


def _ingest_local_files(
    inputs: Iterable[str],
    *,
    category: str,
    dest_dir: Path,
    overwrite: bool,
) -> List[FileEntry]:
    entries: list[FileEntry] = []
    for raw in inputs:
        path = Path(raw)
        dest = _copy_local_file(path, dest_dir, overwrite=overwrite)
        entries.append(
            FileEntry(
                category=category,
                destination=dest,
                sha256=_compute_sha256(dest),
                source=str(path),
            )
        )
    return entries


def export_evidence(
    *,
    scenario_id: str,
    logs: Sequence[str],
    dashboards: Sequence[str],
    alerts: Sequence[str],
    manifests: Sequence[str],
    evidence_files: Sequence[str],
    artifacts_root: Path = red_team_common.DEFAULT_ARTIFACTS_DIR,
    metadata: Mapping[str, str] | None = None,
    summary_path: Path | None = None,
    overwrite: bool = False,
    grafana: GrafanaClient | None = None,
) -> ExportResult:
    """Copy evidence files into the scenario directory and emit a manifest."""

    if not any((logs, dashboards, alerts, manifests, evidence_files)):
        raise ValueError("at least one evidence input must be provided")

    paths = red_team_common.resolve_paths(scenario_id, artifacts_root=artifacts_root)
    category_dirs = {
        name: (paths.artifact_root / dirname)
        for name, dirname in _CATEGORY_DIRS.items()
    }

    entries: list[FileEntry] = []
    entries.extend(
        _ingest_local_files(
            logs,
            category="logs",
            dest_dir=category_dirs["logs"],
            overwrite=overwrite,
        )
    )
    entries.extend(
        _ingest_local_files(
            alerts,
            category="alerts",
            dest_dir=category_dirs["alerts"],
            overwrite=overwrite,
        )
    )
    entries.extend(
        _ingest_local_files(
            manifests,
            category="manifests",
            dest_dir=category_dirs["manifests"],
            overwrite=overwrite,
        )
    )
    entries.extend(
        _ingest_local_files(
            evidence_files,
            category="evidence",
            dest_dir=category_dirs["evidence"],
            overwrite=overwrite,
        )
    )
    for spec in dashboards:
        entries.append(
            _ingest_dashboard_spec(
                spec,
                dest_dir=category_dirs["dashboards"],
                grafana=grafana,
                overwrite=overwrite,
            )
        )

    manifest = {
        "scenario_id": scenario_id,
        "generated_at": _dt.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
        "host": socket.gethostname(),
        "platform": platform.platform(),
        "metadata": dict(metadata or {}),
        "files": [
            {
                "category": entry.category,
                "path": str(entry.destination.relative_to(paths.artifact_root)),
                "sha256": entry.sha256,
                "source": entry.source,
            }
            for entry in entries
        ],
    }

    target_path = summary_path or (paths.artifact_root / "evidence_manifest.json")
    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return ExportResult(manifest_path=target_path, entries=entries)


def _parse_metadata(entries: Sequence[str]) -> MutableMapping[str, str]:
    metadata: dict[str, str] = {}
    for raw in entries:
        if "=" not in raw:
            raise ValueError(f"metadata entry '{raw}' must use KEY=VALUE syntax")
        key, value = raw.split("=", 1)
        key = key.strip()
        if not key:
            raise ValueError("metadata keys must not be empty")
        metadata[key] = value
    return metadata


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario-id", required=True, help="Drill id YYYYMMDD-slug")
    parser.add_argument(
        "--log",
        action="append",
        default=[],
        help="Log file to copy into the scenario logs directory (repeatable).",
    )
    parser.add_argument(
        "--dashboard",
        action="append",
        default=[],
        help="Dashboard source (path or grafana:<uid>[:alias]) to archive (repeatable).",
    )
    parser.add_argument(
        "--alert",
        action="append",
        default=[],
        help="Alert bundle to copy into the alerts directory (repeatable).",
    )
    parser.add_argument(
        "--manifest",
        action="append",
        default=[],
        help="Norito manifest/proof to archive in the manifests directory (repeatable).",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        default=[],
        help="Additional evidence files (pcap, screenshots, etc.).",
    )
    parser.add_argument(
        "--metadata",
        action="append",
        default=[],
        help="Additional metadata entries recorded in the manifest (KEY=VALUE).",
    )
    parser.add_argument(
        "--artifacts-dir",
        type=Path,
        default=red_team_common.DEFAULT_ARTIFACTS_DIR,
        help=f"Root directory for scenario artefacts (default: {red_team_common.DEFAULT_ARTIFACTS_DIR}).",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        help="Optional path for the manifest JSON (defaults to evidence_manifest.json in the scenario).",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Allow overwriting files inside the scenario directories.",
    )
    parser.add_argument(
        "--grafana-url",
        default=os.environ.get("GRAFANA_API_URL"),
        help="Base URL for Grafana API (required when exporting grafana:<uid> dashboards).",
    )
    parser.add_argument(
        "--grafana-token",
        default=os.environ.get("GRAFANA_API_TOKEN"),
        help="Grafana API token used for Authorization header (optional).",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    metadata = _parse_metadata(args.metadata)
    grafana_client: GrafanaClient | None = None
    if any(spec.startswith("grafana:") for spec in args.dashboard):
        if not args.grafana_url:
            parser.error("--grafana-url is required when exporting grafana dashboards")
        grafana_client = GrafanaClient(args.grafana_url, args.grafana_token)
    export_evidence(
        scenario_id=args.scenario_id,
        logs=args.log,
        dashboards=args.dashboard,
        alerts=args.alert,
        manifests=args.manifest,
        evidence_files=args.evidence,
        artifacts_root=args.artifacts_dir,
        metadata=metadata,
        summary_path=args.summary,
        overwrite=args.overwrite,
        grafana=grafana_client,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
