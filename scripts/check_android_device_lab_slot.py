"""Validate Android device-lab slots for AND6 compliance evidence."""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
import sys
from typing import Iterable, List


EXPECTED_DIRS: tuple[str, ...] = ("telemetry", "attestation", "queue", "logs")


def scan_slot(slot_path: Path) -> dict:
    """Inspect a single slot directory and report any missing artefacts."""
    errors: list[str] = []
    present: dict[str, bool] = {}
    file_counts: dict[str, int] = {}

    for dirname in EXPECTED_DIRS:
        dir_path = slot_path / dirname
        exists = dir_path.is_dir()
        present[dirname] = exists
        if not exists:
            errors.append(f"missing {dirname}/ directory")
            continue
        count = sum(1 for entry in dir_path.rglob("*") if entry.is_file())
        file_counts[dirname] = count
        if count == 0:
            errors.append(f"{dirname}/ contains no files")

    sha_path = slot_path / "sha256sum.txt"
    present["sha256sum.txt"] = sha_path.is_file()
    if not sha_path.is_file():
        errors.append("missing sha256sum.txt")
    else:
        sha_text = sha_path.read_text(encoding="utf-8").strip()
        if not sha_text:
            errors.append("sha256sum.txt is empty")

    status = "ok" if not errors else "error"
    return {
        "slot": slot_path.name,
        "status": status,
        "errors": errors,
        "present": present,
        "file_counts": file_counts,
    }


def discover_slots(root: Path, slot_ids: Iterable[str] | None) -> List[Path]:
    """List slot directories under the given root."""
    if slot_ids:
        return [root / slot for slot in slot_ids]
    return [p for p in root.iterdir() if p.is_dir()]


def build_summary(root: Path, reports: list[dict]) -> dict:
    now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
    return {
        "schema_version": 1,
        "generated_at": now.isoformat().replace("+00:00", "Z"),
        "root": str(root),
        "slots": reports,
        "ok": sum(1 for r in reports if r["status"] == "ok"),
        "failed": sum(1 for r in reports if r["status"] != "ok"),
    }


def write_summary(path: Path, summary: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate Android device-lab slots for AND6 compliance."
    )
    parser.add_argument(
        "--root",
        default="artifacts/android/device_lab",
        help="Root directory containing device-lab slots.",
    )
    parser.add_argument(
        "--slot",
        action="append",
        dest="slots",
        default=None,
        help="Specific slot id(s) to validate. Defaults to all slots under --root.",
    )
    parser.add_argument(
        "--require-slot",
        action="store_true",
        help="Fail if no slot directories are found.",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional path to write a JSON summary.",
    )
    parser.add_argument(
        "--allow-missing-root",
        action="store_true",
        help="Treat a missing root directory as a skip instead of an error.",
    )
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    if not root.exists():
        if args.allow_missing_root:
            print(f"[device-lab] root {root} missing; skipping")
            return 0
        print(f"[device-lab] root {root} does not exist", file=sys.stderr)
        return 1

    slot_paths = discover_slots(root, args.slots)
    if not slot_paths:
        if args.require_slot:
            print(f"[device-lab] no slots found under {root}", file=sys.stderr)
            return 1
        print(f"[device-lab] no slots found under {root}; nothing to check")
        return 0

    reports: list[dict] = []
    failures = 0
    for slot_path in slot_paths:
        report = scan_slot(slot_path)
        reports.append(report)
        if report["status"] != "ok":
            failures += 1
            print(f"[device-lab] {slot_path.name}: {', '.join(report['errors'])}", file=sys.stderr)
        else:
            print(f"[device-lab] {slot_path.name}: ok")

    summary = build_summary(root, reports)
    if args.json_out:
        write_summary(Path(args.json_out), summary)
        print(f"[device-lab] wrote summary to {Path(args.json_out).resolve()}")

    return 1 if failures else 0


if __name__ == "__main__":
    import sys

    sys.exit(main())
