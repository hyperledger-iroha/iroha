#!/usr/bin/env python3
"""Verify Nexus lane registry bundles for CI/readiness (NX-2 roadmap task)."""

from __future__ import annotations

import argparse
import json
import sys
import tarfile
from dataclasses import dataclass
from hashlib import blake2b, sha256
from pathlib import Path
from typing import Dict, Iterable, List, Optional


class VerificationError(RuntimeError):
    """Raised when bundle verification fails."""


@dataclass(frozen=True)
class ManifestRecord:
    alias: str
    slug: str
    path: Path
    sha256_hex: str
    blake2b_hex: str
    privacy_commitments: List[Dict[str, str]]


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify Nexus lane registry bundles by checking digests and overlay metadata.",
    )
    parser.add_argument(
        "--bundle-dir",
        type=Path,
        help="Directory containing the bundle contents. Defaults to the summary parent directory.",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        help="Path to summary.json. Defaults to <bundle-dir>/summary.json.",
    )
    parser.add_argument(
        "--require-archive",
        action="store_true",
        help="Fail if the summary references bundle_path but the archive is missing.",
    )
    parser.add_argument(
        "--verify-archive",
        action="store_true",
        help="Inspect the tar archive (when present) to ensure it contains manifests/ and cache/ entries.",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def slugify_alias(alias: str) -> str:
    sanitized = "".join(ch.lower() if ch.isalnum() else "_" for ch in alias).strip("_")
    return sanitized or "lane"


def resolve_path(raw: Optional[str], base_dir: Path) -> Optional[Path]:
    if raw is None:
        return None
    candidate = Path(raw)
    if not candidate.is_absolute():
        candidate = (base_dir / candidate).resolve()
    return candidate


def compute_digests(path: Path) -> tuple[str, str]:
    data = path.read_bytes()
    return sha256(data).hexdigest(), blake2b(data, digest_size=32).hexdigest()


def extract_privacy_commitments(manifest: Dict) -> List[Dict[str, str]]:
    commits = []
    for entry in manifest.get("privacy_commitments") or []:
        cid = entry.get("id")
        scheme = entry.get("scheme")
        if cid is None or scheme is None:
            continue
        try:
            cid_int = int(cid)
        except (TypeError, ValueError):
            continue
        commits.append({"id": cid_int, "scheme": str(scheme)})
    commits.sort(key=lambda item: item["id"])
    return commits


def load_summary(summary_path: Path) -> Dict:
    try:
        return json.loads(summary_path.read_text(encoding="utf-8"))
    except FileNotFoundError as err:
        raise VerificationError(f"summary file {summary_path} does not exist") from err
    except json.JSONDecodeError as err:
        raise VerificationError(f"failed to parse {summary_path}: {err}") from err


def collect_manifests(summary: Dict, base_dir: Path) -> list[ManifestRecord]:
    manifests = summary.get("manifests")
    if not manifests:
        raise VerificationError("summary contains no manifest entries")
    records: list[ManifestRecord] = []
    for entry in manifests:
        alias = entry.get("alias")
        slug = entry.get("slug")
        path_raw = entry.get("path")
        sha_hex = entry.get("sha256")
        blake_hex = entry.get("blake2b")
        if not alias or not slug or not path_raw or not sha_hex or not blake_hex:
            raise VerificationError(f"incomplete manifest entry: {entry}")
        path = resolve_path(path_raw, base_dir)
        assert path is not None
        records.append(
            ManifestRecord(
                alias=str(alias),
                slug=str(slug),
                path=path,
                sha256_hex=str(sha_hex),
                blake2b_hex=str(blake_hex),
                privacy_commitments=[
                    {"id": int(entry.get("id")), "scheme": str(entry.get("scheme"))}
                    for entry in entry.get("privacy_commitments") or []
                    if entry.get("id") is not None and entry.get("scheme") is not None
                ],
            ),
        )
    return records


def verify_manifest(record: ManifestRecord) -> None:
    if not record.path.exists():
        raise VerificationError(f"manifest file {record.path} is missing")
    try:
        manifest = json.loads(record.path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise VerificationError(f"{record.path} is not valid JSON: {err}") from err
    alias = str(manifest.get("lane", "")).strip()
    if not alias:
        raise VerificationError(f"manifest {record.path} is missing the `lane` field")
    if record.alias != alias:
        raise VerificationError(
            f"alias mismatch for {record.path}: summary={record.alias!r}, manifest={alias!r}",
        )
    expected_slug = slugify_alias(alias)
    if record.slug != expected_slug:
        raise VerificationError(
            f"slug mismatch for {record.path}: summary={record.slug!r}, expected={expected_slug!r}",
        )
    sha_hex, blake_hex = compute_digests(record.path)
    if sha_hex != record.sha256_hex or blake_hex != record.blake2b_hex:
        raise VerificationError(
            f"digest mismatch for {record.path}: got sha256={sha_hex}, blake2b={blake_hex}",
        )
    expected_commits = extract_privacy_commitments(manifest)
    if record.privacy_commitments != expected_commits:
        raise VerificationError(
            f"privacy_commitments summary for {record.path} does not match manifest contents",
        )


def verify_overlay(summary: Dict, base_dir: Path) -> None:
    overlay_info = summary.get("governance_overlay") or {}
    overlay_path = resolve_path(overlay_info.get("path"), base_dir)
    modules = overlay_info.get("modules") or {}
    default_module = overlay_info.get("default_module")
    if overlay_path is None:
        if modules or default_module:
            raise VerificationError("overlay metadata present but `path` is missing")
        return
    if not overlay_path.exists():
        raise VerificationError(f"overlay file {overlay_path} is missing")
    try:
        overlay_payload = json.loads(overlay_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        raise VerificationError(f"{overlay_path} is not valid JSON: {err}") from err
    on_disk_modules = overlay_payload.get("modules") or {}
    if modules and modules != on_disk_modules:
        raise VerificationError(
            f"overlay module set in summary differs from {overlay_path}",
        )
    overlay_default = overlay_payload.get("default_module")
    if default_module != overlay_default:
        raise VerificationError(
            f"overlay default_module mismatch: summary={default_module!r}, file={overlay_default!r}",
        )


def verify_archive(summary: Dict, base_dir: Path, strict: bool, deep: bool) -> None:
    bundle_raw = summary.get("bundle_path")
    bundle_path = resolve_path(bundle_raw, base_dir)
    if bundle_raw and bundle_path is None:
        raise VerificationError("summary bundle_path is invalid")
    if bundle_path is None:
        if strict and bundle_raw:
            raise VerificationError(f"bundle archive {bundle_raw} not found")
        return
    if not bundle_path.exists():
        raise VerificationError(f"bundle archive {bundle_path} is missing")
    if deep:
        if not tarfile.is_tarfile(bundle_path):
            raise VerificationError(f"{bundle_path} is not a valid tar archive")
        with tarfile.open(bundle_path, "r:*") as tar:
            members = {member.name for member in tar.getmembers()}
        expects = {"manifests", "cache"}
        if not any(name.startswith("manifests/") for name in members):
            raise VerificationError(f"{bundle_path} is missing manifests/ entries")
        if not any(name.startswith("cache/") for name in members):
            raise VerificationError(f"{bundle_path} is missing cache/ entries")


def verify_bundle(args: argparse.Namespace) -> None:
    summary_path: Path
    if args.summary:
        summary_path = args.summary.expanduser().resolve()
    elif args.bundle_dir:
        summary_path = (args.bundle_dir / "summary.json").expanduser().resolve()
    else:
        raise VerificationError("must provide --summary or --bundle-dir")
    summary = load_summary(summary_path)
    base_dir = (args.bundle_dir or summary_path.parent).expanduser().resolve()
    manifests = collect_manifests(summary, base_dir)
    for record in manifests:
        verify_manifest(record)
    verify_overlay(summary, base_dir)
    verify_archive(summary, base_dir, args.require_archive, args.verify_archive)


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    try:
        verify_bundle(args)
    except VerificationError as err:
        print(f"[nexus] lane registry verification failed: {err}", file=sys.stderr)
        return 1
    print("[nexus] lane registry verification succeeded")
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
