#!/usr/bin/env python3
"""Validate the Android SDK Maven repository output before publishing/staging."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

DEFAULT_REPO_ROOT = Path("artifacts/android/maven")


def sha256_file(path: Path) -> str:
    """Compute the SHA-256 digest for ``path``."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(65536)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def load_checksums(path: Path) -> Dict[str, str]:
    """Parse ``checksums.txt`` into ``{relative_path: sha256}``."""
    contents = path.read_text(encoding="utf-8").splitlines()
    mapping: Dict[str, str] = {}
    for index, line in enumerate(contents):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            sha, rel_path = stripped.split(maxsplit=1)
        except ValueError as exc:  # pragma: no cover - defensive
            raise ValueError(f"invalid checksums.txt entry at line {index + 1}") from exc
        mapping[rel_path] = sha
    return mapping


@dataclass
class VerificationResult:
    summary: Optional[Dict[str, object]]
    errors: List[str]


def verify_repo(repo_dir: Path, *, expected_version: Optional[str], require_metadata: bool) -> VerificationResult:
    """Validate the Maven repo layout, file hashes, and metadata."""

    errors: List[str] = []
    summary_path = repo_dir / "publish_summary.json"
    summary: Optional[Dict[str, object]] = None

    if not summary_path.exists():
        errors.append(f"missing publish_summary.json at {summary_path}")
        return VerificationResult(summary=None, errors=errors)

    try:
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        errors.append(f"invalid JSON in {summary_path}: {exc}")
        return VerificationResult(summary=None, errors=errors)

    version = summary.get("version")
    if not isinstance(version, str):
        errors.append("publish_summary.json missing 'version' string")
    elif expected_version and version != expected_version:
        errors.append(
            f"version mismatch: expected {expected_version!r} but summary recorded {version!r}"
        )

    artifacts = summary.get("artifacts")
    if not isinstance(artifacts, list):
        errors.append("publish_summary.json missing 'artifacts' array")
        return VerificationResult(summary=summary, errors=errors)

    checksums_path = repo_dir / "checksums.txt"
    if not checksums_path.exists():
        errors.append(f"missing checksums.txt at {checksums_path}")
        return VerificationResult(summary=summary, errors=errors)

    try:
        checksum_map = load_checksums(checksums_path)
    except ValueError as exc:
        errors.append(str(exc))
        return VerificationResult(summary=summary, errors=errors)

    for entry in artifacts:
        if not isinstance(entry, dict):
            errors.append(f"artifact entry is not an object: {entry!r}")
            continue
        rel_path = entry.get("path")
        recorded_sha = entry.get("sha256")
        if not isinstance(rel_path, str) or not isinstance(recorded_sha, str):
            errors.append(f"artifact entry missing path/sha256: {entry}")
            continue
        artifact_path = repo_dir / rel_path
        if not artifact_path.exists():
            errors.append(f"artifact missing on disk: {artifact_path}")
            continue
        computed_sha = sha256_file(artifact_path)
        if recorded_sha != computed_sha:
            errors.append(f"SHA mismatch for {rel_path}: summary={recorded_sha} computed={computed_sha}")
        checksum_sha = checksum_map.get(rel_path)
        if checksum_sha is None:
            errors.append(f"checksums.txt missing entry for {rel_path}")
        elif checksum_sha != recorded_sha:
            errors.append(
                f"checksums.txt sha mismatch for {rel_path}: checksums.txt={checksum_sha} summary={recorded_sha}"
            )

    if require_metadata and version:
        artifact_ids = set()
        for entry in artifacts:
            rel_path = entry.get("path")
            if not isinstance(rel_path, str):
                continue
            parts = Path(rel_path).parts
            if len(parts) >= 5 and parts[0] == "org" and parts[1] == "hyperledger" and parts[2] == "iroha":
                artifact_ids.add(parts[3])
        for artifact_id in sorted(artifact_ids):
            metadata_path = repo_dir / "org" / "hyperledger" / "iroha" / artifact_id / "maven-metadata.xml"
            if not metadata_path.exists():
                errors.append(f"missing Maven metadata at {metadata_path}")
                continue
            try:
                tree = ET.parse(metadata_path)
            except ET.ParseError as exc:
                errors.append(f"invalid XML in {metadata_path}: {exc}")
                continue
            versions = [
                elem.text for elem in tree.findall(".//versioning/versions/version") if elem.text
            ]
            if version not in versions:
                errors.append(
                    f"{metadata_path} does not list version {version}; found {versions or 'no versions'}"
                )

    return VerificationResult(summary=summary, errors=errors)


def build_report(repo_dir: Path, result: VerificationResult) -> Dict[str, object]:
    """Produce a structured summary for CI pipelines."""
    artifact_count = 0
    version = None
    if result.summary:
        version = result.summary.get("version")
        artifacts = result.summary.get("artifacts")
        if isinstance(artifacts, list):
            artifact_count = len(artifacts)
    return {
        "repo_dir": str(repo_dir),
        "version": version,
        "artifact_count": artifact_count,
        "status": "ok" if not result.errors else "error",
        "errors": result.errors,
    }


def format_markdown_report(report: Dict[str, object]) -> str:
    """Render a human-readable Markdown summary suitable for evidence bundles."""
    lines = ["# Android Maven Repo Validation", ""]
    lines.append(f"- repo: `{report['repo_dir']}`")
    version = report.get("version")
    if version:
        lines.append(f"- version: `{version}`")
    lines.append(f"- artifacts: {report['artifact_count']}")
    lines.append(f"- status: {report['status']}")
    lines.append("")
    if report.get("errors"):
        lines.append("## Errors")
        for error in report["errors"]:
            lines.append(f"- {error}")
    else:
        lines.append("No validation errors detected.")
    lines.append("")
    return "\n".join(lines)


def parse_args(argv: Optional[Sequence[str]]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate Android SDK Maven repository output.")
    parser.add_argument(
        "--repo-dir",
        help="Local Maven repository root (defaults to artifacts/android/maven/<version> when --version is provided)",
    )
    parser.add_argument("--version", help="Expected SDK version; required when --repo-dir is omitted.")
    parser.add_argument(
        "--require-metadata",
        action="store_true",
        help="Ensure maven-metadata.xml contains the recorded version.",
    )
    parser.add_argument("--json-out", help="Optional path to write the JSON report.")
    parser.add_argument("--markdown-out", help="Optional path to write a Markdown summary for evidence bundles.")
    parser.add_argument("--quiet", action="store_true", help="Suppress human-readable output.")
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)
    if not args.repo_dir and not args.version:
        print("error: --version is required when --repo-dir is omitted", file=sys.stderr)
        return 2
    repo_dir = Path(args.repo_dir) if args.repo_dir else DEFAULT_REPO_ROOT / args.version
    repo_dir = repo_dir.resolve()

    result = verify_repo(repo_dir, expected_version=args.version, require_metadata=args.require_metadata)
    report = build_report(repo_dir, result)

    if args.json_out:
        output_path = Path(args.json_out)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if args.markdown_out:
        md_path = Path(args.markdown_out)
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(format_markdown_report(report), encoding="utf-8")

    if not args.quiet:
        if result.errors:
            for error in result.errors:
                print(f"[maven-check] {error}", file=sys.stderr)
        else:
            print(
                f"[maven-check] validated {report['artifact_count']} artifacts in {repo_dir}",
                file=sys.stdout,
            )

    return 0 if not result.errors else 1


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
