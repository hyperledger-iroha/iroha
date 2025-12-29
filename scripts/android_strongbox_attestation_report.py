#!/usr/bin/env python3
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Summarise Android StrongBox attestation verification results."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


DEFAULT_BUNDLES_ROOT = Path("artifacts/android/attestation")


@dataclass(frozen=True)
class Report:
    """Rendered attestation summary."""

    lines: list[str]
    failures: list[str]

    def as_text(self) -> str:
        """Render the report as a string suitable for annotations."""
        if not self.lines:
            return ""
        return "\n".join(self.lines) + "\n"

    @property
    def has_failures(self) -> bool:
        """Return True when any bundle failed validation."""
        return bool(self.failures)


def collect_directories(root: Path) -> list[Path]:
    """Return bundle directories discovered under the attestation root."""
    result_dirs = {path.parent for path in root.rglob("result.json")}
    chain_dirs = {path.parent for path in root.rglob("chain.pem")}
    # Include all directories holding either attestation results or bundle inputs.
    candidates = result_dirs | chain_dirs
    return sorted(candidates, key=lambda path: path.relative_to(root).as_posix())


def load_result(path: Path) -> dict:
    """Load a result.json payload."""
    content = path.read_text(encoding="utf-8")
    return json.loads(content)


def build_report(root: Path) -> Report:
    """Produce a summary report for bundles located under root."""
    lines: list[str] = []
    failures: list[str] = []

    if not root.exists():
        lines.append(
            f"No attestation bundles directory found at {root}; skipping verification."
        )
        return Report(lines, failures)

    bundle_dirs = collect_directories(root)
    if not bundle_dirs:
        lines.append(
            "No attestation bundles discovered under "
            f"{root}; nothing to verify this run."
        )
        return Report(lines, failures)

    details: list[str] = []
    ok_count = 0

    for bundle_dir in bundle_dirs:
        rel = bundle_dir.relative_to(root).as_posix()
        result_path = bundle_dir / "result.json"
        if not result_path.exists():
            failures.append(
                f"{rel}: missing result.json (rerun scripts/android_strongbox_attestation_ci.sh)."
            )
            details.append(f"{rel}: result.json missing.")
            continue

        try:
            data = load_result(result_path)
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
            failures.append(f"{rel}: invalid result.json ({exc}).")
            details.append(f"{rel}: invalid result.json ({exc}).")
            continue

        alias = str(data.get("alias", "<unknown>"))
        att_level = str(data.get("attestation_security_level", "UNKNOWN"))
        key_level = str(data.get("keymaster_security_level", "UNKNOWN"))
        strongbox = data.get("strongbox_attestation")
        chain_length = data.get("chain_length", "UNKNOWN")

        details.append(
            f"{rel}: alias={alias} attestation={att_level} keymaster={key_level} "
            f"strongbox={strongbox} chain={chain_length}"
        )

        if (
            isinstance(strongbox, bool)
            and strongbox
            and att_level in ("STRONGBOX", "STRONG_BOX")
            and key_level in ("STRONGBOX", "STRONG_BOX")
        ):
            ok_count += 1
        else:
            failures.append(
                f"{rel}: expected STRONGBOX attestation/keymaster with "
                f"strongbox_attestation=true, got "
                f"attestation={att_level}, keymaster={key_level}, "
                f"strongbox_attestation={strongbox}"
            )

    summary = f"Verified {ok_count} of {len(bundle_dirs)} StrongBox attestation bundles."
    lines.append(summary)
    lines.extend(details)
    if failures:
        lines.append("")
        lines.append("Failure details:")
        lines.extend(failures)

    return Report(lines, failures)


def write_report(report: Report, path: Path) -> None:
    """Persist the report to disk."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report.as_text(), encoding="utf-8")


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Summarise Android StrongBox attestation verification results."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=DEFAULT_BUNDLES_ROOT,
        help="Directory containing attestation bundles (default: %(default)s).",
    )
    parser.add_argument(
        "--report-path",
        type=Path,
        required=True,
        help="Destination file for the generated summary.",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """Entry point."""
    args = parse_args(argv)
    report = build_report(args.root)
    write_report(report, args.report_path)
    if report.has_failures:
        for failure in report.failures:
            print(failure, file=sys.stderr, flush=True)
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
