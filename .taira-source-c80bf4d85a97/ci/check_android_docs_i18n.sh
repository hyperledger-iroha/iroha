#!/usr/bin/env python3
"""
Checks that required Android SDK localization files exist whenever the i18n plan
marks a locale as current or out of date. Intended to gate AND5 documentation.
Supports JSON status snapshots via --json-out for roadmap evidence bundles.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Optional

PLAN_PATH = Path("docs/source/sdk/android/i18n_plan.md")

DOC_PATHS: Dict[str, Path] = {
    "Quickstart": Path("docs/source/sdk/android/index.md"),
    "Key management": Path("docs/source/sdk/android/key_management.md"),
    "Offline signing": Path("docs/source/sdk/android/offline_signing.md"),
    "Networking": Path("docs/source/sdk/android/networking.md"),
    "Operator console sample": Path("docs/source/sdk/android/samples/operator_console.md"),
    "Retail wallet sample": Path("docs/source/sdk/android/samples/retail_wallet.md"),
}

LOCALE_SUFFIX = {
    "ja": ".ja.md",
    "he": ".he.md",
}


def parse_translation_table(plan_text: str) -> Dict[str, Dict[str, Dict[str, str]]]:
    rows: Dict[str, Dict[str, Dict[str, str]]] = {}
    capturing = False
    for line in plan_text.splitlines():
        if line.startswith("### 5.1"):
            capturing = True
            continue
        if capturing and line.startswith("### "):
            break
        if not capturing:
            continue
        stripped = line.strip()
        if not stripped.startswith("|"):
            continue
        if "Doc" in stripped and "English" in stripped:
            continue
        if set(stripped) <= {"|", "-", " "}:
            continue
        parts = [part.strip() for part in stripped.strip("|").split("|")]
        if len(parts) < 5:
            continue
        rows[parts[0]] = {
            "english_updated": parts[1],
            "statuses": {"ja": parts[2], "he": parts[3]},
        }
    return rows


def needs_locale(status: str) -> bool:
    return "✅" in status or "⚠️" in status


def localized_path(english_path: Path, suffix: str) -> Path:
    if english_path.suffix != ".md":
        return english_path.with_name(f"{english_path.name}{suffix}")
    return english_path.with_name(f"{english_path.stem}{suffix}")


def parse_front_matter_metadata(path: Path) -> Optional[Dict[str, str]]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    suffix = path.suffix.lower()
    metadata: Dict[str, str] = {}
    if suffix == ".md":
        lines = iter(text.splitlines())
        first = next(lines, "").strip()
        if first != "---":
            return None
        for line in lines:
            stripped = line.strip()
            if stripped == "---":
                break
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            metadata[key.strip().lower()] = value.strip()
        return metadata
    if suffix == ".org":
        for line in text.splitlines():
            if not line.startswith("#+"):
                continue
            content = line[2:]
            key, _, value = content.partition(":")
            if not value:
                continue
            metadata[key.strip().lower()] = value.strip()
        return metadata
    return None


def parse_date(value: str) -> Optional[datetime]:
    try:
        if not value or value.lower() in {"null", "~"}:
            return None
        if "t" in value.lower():
            return datetime.fromisoformat(value)
        return datetime.fromisoformat(value.strip() + "T00:00:00+00:00")
    except ValueError:
        return None


def compute_file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build_report(plan_path: Path, plan_text: str) -> Dict[str, Any]:
    table = parse_translation_table(plan_text)
    errors = []
    doc_reports = []

    def record_error(
        message: str,
        doc_report: Dict[str, Any],
        locale_report: Optional[Dict[str, Any]] = None,
    ) -> None:
        errors.append(message)
        doc_report.setdefault("errors", []).append(message)
        if locale_report is not None:
            locale_report.setdefault("errors", []).append(message)

    for doc_name, english_path in DOC_PATHS.items():
        status_entry = table.get(doc_name)
        doc_report: Dict[str, Any] = {
            "doc": doc_name,
            "english_path": str(english_path),
            "english_exists": english_path.exists(),
            "english_hash": None,
            "english_updated": None,
            "locales": {},
            "errors": [],
        }
        doc_reports.append(doc_report)

        if status_entry is None:
            record_error(
                f"Doc '{doc_name}' missing from translation status table.",
                doc_report,
            )
            continue
        if not english_path.exists():
            # English source not written yet; skip until doc lands but keep reporting.
            continue

        english_hash: Optional[str] = compute_file_hash(english_path)
        doc_report["english_hash"] = english_hash
        english_updated_raw = status_entry.get("english_updated", "").strip()
        if english_updated_raw:
            doc_report["english_updated"] = english_updated_raw
        english_updated = parse_date(english_updated_raw) if english_updated_raw else None

        for locale, suffix in LOCALE_SUFFIX.items():
            status = status_entry["statuses"].get(locale, "")
            required = needs_locale(status)
            localized = localized_path(english_path, suffix)
            metadata = parse_front_matter_metadata(localized) if localized.exists() else None
            locale_report: Dict[str, Any] = {
                "status": status,
                "required": required,
                "path": str(localized),
                "exists": localized.exists(),
                "metadata": metadata,
                "errors": [],
            }
            doc_report["locales"][locale] = locale_report

            if not required:
                continue
            if not localized.exists():
                record_error(
                    f"{doc_name}: {locale} marked as '{status}' but {localized} is missing.",
                    doc_report,
                    locale_report,
                )
                continue
            if metadata is None:
                record_error(
                    f"{doc_name}: {locale} marked as '{status}' but {localized} is missing front-matter metadata.",
                    doc_report,
                    locale_report,
                )
                continue
            fm_status = metadata.get("status", "")
            if fm_status.strip().lower() == "needs-translation":
                record_error(
                    f"{doc_name}: {locale} marked as '{status}' but {localized} metadata still declares status=needs-translation.",
                    doc_report,
                    locale_report,
                )
            source_hash = metadata.get("source_hash")
            if not source_hash:
                record_error(
                    f"{doc_name}: {locale} localization metadata missing source_hash.",
                    doc_report,
                    locale_report,
                )
            else:
                if english_hash != source_hash:
                    record_error(
                        f"{doc_name}: {locale} source_hash mismatch (expected {english_hash}, found {source_hash}).",
                        doc_report,
                        locale_report,
                    )
            if english_updated:
                reviewed_raw = metadata.get("translation_last_reviewed", "")
                reviewed = parse_date(reviewed_raw) if reviewed_raw else None
                if reviewed is None:
                    record_error(
                        f"{doc_name}: {locale} missing translation_last_reviewed metadata.",
                        doc_report,
                        locale_report,
                    )
                elif reviewed.astimezone(timezone.utc) < english_updated.astimezone(
                    timezone.utc
                ):
                    record_error(
                        f"{doc_name}: {locale} translation_last_reviewed ({reviewed_raw}) predates English update ({english_updated_raw}).",
                        doc_report,
                        locale_report,
                    )

    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "plan_path": str(plan_path),
        "docs": doc_reports,
        "errors": errors,
        "ok": not errors,
    }


def write_json_report(path: Path, payload: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Android SDK localization files against the i18n plan."
    )
    parser.add_argument(
        "--plan",
        type=Path,
        default=PLAN_PATH,
        help=f"Path to the localization plan (default: {PLAN_PATH})",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Optional path to write a JSON status snapshot.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    plan_path: Path = args.plan
    if not plan_path.exists():
        print(f"ERROR: missing plan file {plan_path}", file=sys.stderr)
        return 1

    report = build_report(plan_path, plan_path.read_text(encoding="utf-8"))

    if args.json_out:
        write_json_report(args.json_out, report)

    if not report["ok"]:
        print("Android docs localization check failed:", file=sys.stderr)
        for err in report["errors"]:
            print(f"  - {err}", file=sys.stderr)
        return 1

    print("Android docs localization check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
