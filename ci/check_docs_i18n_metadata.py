#!/usr/bin/env python3
"""Audit translated Markdown metadata for source traceability."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
NULL_VALUES = {"", "null", "~", "none"}


def compute_file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_date(value: str) -> datetime | None:
    stripped = value.strip().strip('"').strip("'")
    if stripped.lower() in NULL_VALUES:
        return None
    try:
        if "t" not in stripped.lower():
            stripped = f"{stripped}T00:00:00+00:00"
        parsed = datetime.fromisoformat(stripped)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed


def parse_markdown_metadata(path: Path) -> dict[str, str] | None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        return None

    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        if not stripped:
            index += 1
            continue
        if stripped.startswith("<!--") and stripped.endswith("-->"):
            index += 1
            continue
        break

    if index >= len(lines) or lines[index].strip() != "---":
        return None

    metadata: dict[str, str] = {}
    for line in lines[index + 1 :]:
        stripped = line.strip()
        if stripped == "---":
            return metadata
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        metadata[key.strip().lower()] = value.strip().strip('"').strip("'")
    return None


def is_translation_metadata(metadata: dict[str, str]) -> bool:
    return (
        "source" in metadata
        or "source_hash" in metadata
        or "translation_last_reviewed" in metadata
    )


def iter_markdown(paths: list[Path]) -> list[Path]:
    files: list[Path] = []
    for path in paths:
        resolved = path if path.is_absolute() else REPO_ROOT / path
        if resolved.is_file() and resolved.suffix == ".md":
            files.append(resolved)
            continue
        if resolved.is_dir():
            files.extend(sorted(resolved.rglob("*.md")))
    return sorted(set(files))


def audit_file(path: Path) -> tuple[list[str], list[str], dict[str, Any] | None]:
    metadata = parse_markdown_metadata(path)
    if metadata is None or not is_translation_metadata(metadata):
        return [], [], None

    rel = path.relative_to(REPO_ROOT).as_posix()
    errors: list[str] = []
    warnings: list[str] = []
    source = metadata.get("source", "").strip()
    source_hash = metadata.get("source_hash", "").strip()
    reviewed_raw = metadata.get("translation_last_reviewed", "").strip()
    status = metadata.get("status", "").strip().lower()

    if not source:
        errors.append(f"{rel}: missing source metadata")
        source_path = None
    else:
        source_path = (REPO_ROOT / source).resolve()
        if not source_path.exists():
            errors.append(f"{rel}: source path does not exist: {source}")
            source_path = None

    if not source_hash:
        errors.append(f"{rel}: missing source_hash metadata")

    if "translation_last_reviewed" not in metadata:
        errors.append(f"{rel}: missing translation_last_reviewed metadata")
    elif status != "needs-translation" and parse_date(reviewed_raw) is None:
        errors.append(f"{rel}: invalid translation_last_reviewed value: {reviewed_raw}")

    current_hash = None
    if source_path is not None and source_path.is_file():
        current_hash = compute_file_hash(source_path)
        if source_hash and current_hash != source_hash:
            warnings.append(
                f"{rel}: source_hash is stale (expected {current_hash}, found {source_hash})"
            )

    report = {
        "path": rel,
        "source": source or None,
        "status": status or None,
        "source_hash": source_hash or None,
        "current_source_hash": current_hash,
        "translation_last_reviewed": reviewed_raw or None,
        "errors": errors,
        "warnings": warnings,
    }
    return errors, warnings, report


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit translated documentation front matter for source traceability."
    )
    parser.add_argument(
        "--paths",
        nargs="+",
        default=["docs/formal"],
        help="Files or directories to scan.",
    )
    parser.add_argument(
        "--require-current",
        action="store_true",
        help="Treat stale source_hash metadata as an error.",
    )
    parser.add_argument("--json-out", type=Path, help="Optional JSON report path.")
    parser.add_argument(
        "--max-messages",
        type=int,
        default=50,
        help="Maximum errors/warnings printed to stderr.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    paths = [Path(item) for item in args.paths]
    reports: list[dict[str, Any]] = []
    errors: list[str] = []
    warnings: list[str] = []

    for path in iter_markdown(paths):
        file_errors, file_warnings, report = audit_file(path)
        errors.extend(file_errors)
        warnings.extend(file_warnings)
        if report is not None:
            reports.append(report)

    if args.require_current:
        errors.extend(warnings)
        warnings = []

    payload = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "paths": [str(path) for path in paths],
        "translation_files": len(reports),
        "errors": errors,
        "warnings": warnings,
        "ok": not errors,
        "files": reports,
    }

    if args.json_out:
        output = args.json_out if args.json_out.is_absolute() else REPO_ROOT / args.json_out
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )

    for message in errors[: args.max_messages]:
        print(f"error: {message}", file=sys.stderr)
    for message in warnings[: args.max_messages]:
        print(f"warning: {message}", file=sys.stderr)

    omitted = max(0, len(errors) - args.max_messages) + max(
        0, len(warnings) - args.max_messages
    )
    if omitted:
        print(f"... omitted {omitted} additional messages", file=sys.stderr)

    if errors:
        print(
            f"docs i18n metadata audit failed: {len(errors)} error(s), {len(warnings)} warning(s)",
            file=sys.stderr,
        )
        return 1

    print(
        f"docs i18n metadata audit passed: {len(reports)} translation file(s), {len(warnings)} warning(s)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
