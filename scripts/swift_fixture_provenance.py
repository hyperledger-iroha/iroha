#!/usr/bin/env python3
"""Generate a provenance manifest for Swift Norito fixture regeneration."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional, Sequence


@dataclass(frozen=True)
class Provenance:
    """Provenance payload written to disk."""

    generated_at: str
    fixture_root: Path
    fixture_sha256: str
    state_file: Path
    state_sha256: str
    git_revision: Optional[str]
    approver: Optional[str]
    change_ticket: Optional[str]
    source_kind: Optional[str]
    archive_sha256: Optional[str]
    archive_kind: Optional[str]
    notes: Optional[str]

    def to_dict(self) -> dict:
        payload = {
            "generated_at": self.generated_at,
            "fixture_root": str(self.fixture_root),
            "fixture_sha256": self.fixture_sha256,
            "state_file": str(self.state_file),
            "state_sha256": self.state_sha256,
        }
        if self.git_revision:
            payload["git_revision"] = self.git_revision
        if self.approver:
            payload["approver"] = self.approver
        if self.change_ticket:
            payload["change_ticket"] = self.change_ticket
        if self.source_kind:
            payload["source_kind"] = self.source_kind
        if self.archive_sha256:
            payload["archive_sha256"] = self.archive_sha256
        if self.archive_kind:
            payload["archive_kind"] = self.archive_kind
        if self.notes:
            payload["notes"] = self.notes
        return payload


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def tree_digest(root: Path) -> str:
    """Return a deterministic SHA-256 digest for all files under ``root``.

    The digest includes relative paths to guard against renamed or missing
    fixtures while remaining stable across platforms and file ordering.
    """

    if not root.exists() or not root.is_dir():
        raise FileNotFoundError(f"fixture root missing or not a directory: {root}")

    digest = hashlib.sha256()
    files = [path for path in root.rglob("*") if path.is_file()]
    files.sort()
    for path in files:
        rel = path.relative_to(root).as_posix().encode("utf-8")
        digest.update(rel)
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def _git_revision(workdir: Path | None = None) -> Optional[str]:
    try:
        return (
            subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=workdir,
                check=True,
                capture_output=True,
                text=True,
            )
            .stdout.strip()
        )
    except Exception:
        return None


def build_provenance(
    fixture_root: Path,
    state_file: Path,
    *,
    git_ref: str | None = None,
    approver: str | None = None,
    change_ticket: str | None = None,
    source_kind: str | None = None,
    archive_sha256: str | None = None,
    archive_kind: str | None = None,
    notes: str | None = None,
) -> Provenance:
    fixture_hash = tree_digest(fixture_root)
    state_hash = _sha256_file(state_file)
    git_rev = git_ref or _git_revision(fixture_root.parent)
    generated_at = (
        datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    )
    return Provenance(
        generated_at=generated_at,
        fixture_root=fixture_root,
        fixture_sha256=fixture_hash,
        state_file=state_file,
        state_sha256=state_hash,
        git_revision=git_rev,
        approver=approver or None,
        change_ticket=change_ticket or None,
        source_kind=(source_kind or None),
        archive_sha256=archive_sha256 or None,
        archive_kind=archive_kind or None,
        notes=notes or None,
    )


def write_provenance(path: Path, payload: Provenance) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload.to_dict(), indent=2) + "\n", encoding="utf-8")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Build Swift fixture provenance manifest.")
    parser.add_argument("--root", required=True, type=Path, help="Fixture root directory.")
    parser.add_argument("--state-file", required=True, type=Path, help="Cadence state JSON file.")
    parser.add_argument("--out", required=True, type=Path, help="Output manifest path.")
    parser.add_argument("--git-ref", type=str, help="Optional git commit hash to record.")
    parser.add_argument("--approver", type=str, help="Change approver or on-call rotation.")
    parser.add_argument("--change-ticket", type=str, help="Change or governance ticket reference.")
    parser.add_argument("--source-kind", type=str, help="How fixtures were sourced (archive/directory).")
    parser.add_argument("--archive-sha", type=str, help="Archive SHA-256 when consuming bundles.")
    parser.add_argument("--archive-kind", type=str, help="Archive format when consuming bundles.")
    parser.add_argument("--notes", type=str, help="Optional notes for the provenance entry.")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    provenance = build_provenance(
        args.root,
        args.state_file,
        git_ref=args.git_ref,
        approver=args.approver,
        change_ticket=args.change_ticket,
        source_kind=args.source_kind,
        archive_sha256=args.archive_sha,
        archive_kind=args.archive_kind,
        notes=args.notes,
    )
    write_provenance(args.out, provenance)
    print(f"[swift-provenance] wrote manifest to {args.out}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
