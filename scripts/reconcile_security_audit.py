#!/usr/bin/env python3
"""Reconcile the private security-audit Markdown ledger with its reports.

The tool is read-only.  It expects the private audit archive (or an exported
copy) to live inside the repository and validates checklist totals, local
report links, mandatory classification/status/evidence fields, and state
contradictions.  It never edits or closes a finding.

No environment variables are required.  Run ``--help`` for path and expected
count options.  The defaults describe the 2026-08-03 audit baseline; override
``--expected-checked``/``--expected-open`` with the release target when
reconciling a later source freeze.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from urllib.parse import unquote, urlsplit


CHECKBOX_RE = re.compile(r"^\s*[-*+]\s+\[([ xX])\]\s+(.+?)\s*$")
MARKDOWN_LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
REPORT_FIELD_RE = re.compile(
    r"^\s*(?:[-*+]\s+)?(?:\*\*)?"
    r"(audit\s+classification|classification|status|evidence)"
    r"(?:(?:\*\*)\s*:|:\s*(?:\*\*)?)\s*(.*?)\s*$",
    re.IGNORECASE,
)
REPORT_TABLE_FIELD_RE = re.compile(
    r"^\s*\|\s*(audit\s+classification|classification|status|evidence)"
    r"\s*\|\s*(.*?)\s*\|\s*$",
    re.IGNORECASE,
)
HEADING_RE = re.compile(r"^\s*#{1,6}\s+")
PLACEHOLDER_EVIDENCE_RE = re.compile(
    r"^\s*(?:todo|tbd|placeholder|none|n/?a|pending)\s*[.!]?\s*$",
    re.IGNORECASE,
)

CLASS_CONFIRMED_SOURCE = "confirmed-source-work"
CLASS_SOURCE_FIXED = "source-fixed-validation-pending"
CLASS_REJECTED = "rejected-with-code-path-proof"
CLASS_EXTERNAL = "externally-evidence-blocked"

CLASSIFICATION_ALIASES = {
    CLASS_CONFIRMED_SOURCE: CLASS_CONFIRMED_SOURCE,
    "confirmed-source": CLASS_CONFIRMED_SOURCE,
    "confirmed-source-work-required": CLASS_CONFIRMED_SOURCE,
    CLASS_SOURCE_FIXED: CLASS_SOURCE_FIXED,
    "source-fixed-validation-pending": CLASS_SOURCE_FIXED,
    "source-fixed-or-validation-pending": CLASS_SOURCE_FIXED,
    CLASS_REJECTED: CLASS_REJECTED,
    "rejected-code-path-proof": CLASS_REJECTED,
    CLASS_EXTERNAL: CLASS_EXTERNAL,
    "external-evidence-blocked": CLASS_EXTERNAL,
}

CLASSIFICATION_STATUSES = {
    CLASS_CONFIRMED_SOURCE: {
        "open",
        "confirmed",
        "in-progress",
        "source-work",
        "source-work-required",
    },
    CLASS_SOURCE_FIXED: {
        "source-fixed",
        "validation-pending",
        "fixed-pending-validation",
        "validated",
        "fixed-validated",
        "closed",
    },
    CLASS_REJECTED: {
        "rejected",
        "false-positive",
        "not-applicable",
        "closed",
    },
    CLASS_EXTERNAL: {
        "externally-evidence-blocked",
        "external-evidence-blocked",
        "evidence-blocked",
        "blocked-external",
    },
}

TERMINAL_STATUSES = {
    "validated",
    "fixed-validated",
    "closed",
    "rejected",
    "false-positive",
    "not-applicable",
}


def slug(value: str) -> str:
    """Normalize a human-readable field value for exact policy comparison."""

    return re.sub(r"[^a-z0-9]+", "-", value.strip().lower()).strip("-")


@dataclass(frozen=True)
class Finding:
    """One Markdown checklist row."""

    line: int
    checked: bool
    text: str
    reports: tuple[Path, ...]


@dataclass(frozen=True)
class ReportRecord:
    """Mandatory reconciliation fields read from one report."""

    path: Path
    classification: str
    status: str
    evidence: str


@dataclass(frozen=True)
class ReconciliationError:
    """One deterministic validation failure."""

    location: str
    message: str


@dataclass(frozen=True)
class ReconciliationSummary:
    """Machine-readable reconciliation totals."""

    total: int
    checked: int
    open: int
    reports: int
    errors: tuple[ReconciliationError, ...]


def _extract_link_target(raw: str) -> str:
    raw = raw.strip()
    if raw.startswith("<"):
        closing = raw.find(">")
        return raw[1:closing] if closing >= 0 else raw[1:]
    # Markdown titles follow the target after whitespace.  Audit report paths
    # use percent encoding for literal spaces, so the first token is exact.
    return raw.split(maxsplit=1)[0]


def _resolve_report_link(
    raw_target: str,
    ledger: Path,
    archive_root: Path,
) -> tuple[Path | None, str | None]:
    target = unquote(_extract_link_target(raw_target))
    split = urlsplit(target)
    if split.scheme or split.netloc:
        return None, None
    path_text = split.path
    if not path_text.lower().endswith(".md"):
        return None, None
    candidate = Path(path_text)
    if candidate.is_absolute():
        return None, "absolute report links are forbidden"
    resolved = (ledger.parent / candidate).resolve()
    if not resolved.is_relative_to(archive_root):
        return None, "report link escapes the configured audit archive"
    if resolved == ledger:
        return None, "checklist row links back to the ledger instead of a report"
    return resolved, None


def parse_findings(
    ledger: Path,
    archive_root: Path,
) -> tuple[list[Finding], list[ReconciliationError]]:
    """Parse checklist rows and resolve their local report links."""

    errors: list[ReconciliationError] = []
    findings: list[Finding] = []
    try:
        lines = ledger.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return [], [ReconciliationError(str(ledger), f"cannot read ledger: {error}")]

    for line_number, line in enumerate(lines, start=1):
        match = CHECKBOX_RE.match(line)
        if match is None:
            continue
        checked = match.group(1).lower() == "x"
        text = match.group(2)
        report_paths: list[Path] = []
        for raw_target in MARKDOWN_LINK_RE.findall(text):
            report, link_error = _resolve_report_link(
                raw_target, ledger, archive_root
            )
            if link_error is not None:
                errors.append(
                    ReconciliationError(
                        f"{ledger}:{line_number}",
                        f"invalid report link `{_extract_link_target(raw_target)}`: {link_error}",
                    )
                )
            elif report is not None:
                report_paths.append(report)
        unique_reports = tuple(dict.fromkeys(report_paths))
        if not unique_reports:
            errors.append(
                ReconciliationError(
                    f"{ledger}:{line_number}",
                    "checklist row has no local Markdown report link",
                )
            )
        findings.append(Finding(line_number, checked, text, unique_reports))

    if not findings:
        errors.append(ReconciliationError(str(ledger), "ledger contains no checklist rows"))
    return findings, errors


def _read_report_fields(
    path: Path,
) -> tuple[dict[str, str], list[ReconciliationError]]:
    errors: list[ReconciliationError] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return {}, [ReconciliationError(str(path), f"cannot read report: {error}")]

    fields: dict[str, str] = {}
    index = 0
    while index < len(lines):
        match = REPORT_FIELD_RE.match(lines[index]) or REPORT_TABLE_FIELD_RE.match(
            lines[index]
        )
        if match is None:
            index += 1
            continue
        raw_name = slug(match.group(1))
        name = "classification" if raw_name == "audit-classification" else raw_name
        chunks = [match.group(2).strip()] if match.group(2).strip() else []
        cursor = index + 1
        while cursor < len(lines):
            if (
                REPORT_FIELD_RE.match(lines[cursor])
                or REPORT_TABLE_FIELD_RE.match(lines[cursor])
                or HEADING_RE.match(lines[cursor])
            ):
                break
            stripped = lines[cursor].strip()
            if stripped:
                chunks.append(stripped.removeprefix("- ").removeprefix("* "))
            cursor += 1
        value = " ".join(chunks).strip()
        if name in fields:
            errors.append(
                ReconciliationError(
                    f"{path}:{index + 1}", f"report defines `{name}` more than once"
                )
            )
        else:
            fields[name] = value
        index = max(index + 1, cursor)
    return fields, errors


def parse_report(path: Path) -> tuple[ReportRecord | None, list[ReconciliationError]]:
    """Read and validate the mandatory fields in one report."""

    fields, errors = _read_report_fields(path)
    for field in ("classification", "status", "evidence"):
        if not fields.get(field, "").strip():
            errors.append(ReconciliationError(str(path), f"report is missing `{field}`"))
    if errors:
        return None, errors

    raw_classification = slug(fields["classification"])
    classification = CLASSIFICATION_ALIASES.get(raw_classification)
    if classification is None:
        allowed = ", ".join(
            (
                CLASS_CONFIRMED_SOURCE,
                CLASS_SOURCE_FIXED,
                CLASS_REJECTED,
                CLASS_EXTERNAL,
            )
        )
        errors.append(
            ReconciliationError(
                str(path),
                f"unknown classification `{fields['classification']}`; expected one of {allowed}",
            )
        )
        return None, errors

    status = slug(fields["status"])
    if status not in CLASSIFICATION_STATUSES[classification]:
        expected = ", ".join(sorted(CLASSIFICATION_STATUSES[classification]))
        errors.append(
            ReconciliationError(
                str(path),
                f"status `{fields['status']}` contradicts classification `{classification}`; expected one of {expected}",
            )
        )
    evidence = fields["evidence"].strip()
    if PLACEHOLDER_EVIDENCE_RE.fullmatch(evidence):
        errors.append(
            ReconciliationError(
                str(path), "evidence is a placeholder rather than reproducible proof"
            )
        )
    if errors:
        return None, errors
    return ReportRecord(path, classification, status, evidence), []


def _expected_count_error(
    label: str, actual: int, expected: int | None
) -> ReconciliationError | None:
    if expected is None or actual == expected:
        return None
    return ReconciliationError(
        "ledger", f"{label} count is {actual}, expected exactly {expected}"
    )


def reconcile(
    ledger: Path,
    archive_root: Path,
    *,
    reports_root: Path | None = None,
    expected_total: int | None = 561,
    expected_checked: int | None = 550,
    expected_open: int | None = 11,
    expected_reports: int | None = 303,
) -> ReconciliationSummary:
    """Reconcile one ledger/archive pair without mutating either."""

    ledger = ledger.resolve()
    archive_root = archive_root.resolve()
    errors: list[ReconciliationError] = []
    if not ledger.is_relative_to(archive_root):
        errors.append(
            ReconciliationError(
                str(ledger), "ledger must live inside the configured audit archive"
            )
        )
        return ReconciliationSummary(0, 0, 0, 0, tuple(errors))

    findings, finding_errors = parse_findings(ledger, archive_root)
    errors.extend(finding_errors)
    checked = sum(finding.checked for finding in findings)
    open_count = len(findings) - checked
    linked_reports = {
        report for finding in findings for report in finding.reports
    }

    for label, actual, expected in (
        ("total checklist", len(findings), expected_total),
        ("checked checklist", checked, expected_checked),
        ("open checklist", open_count, expected_open),
        ("linked report", len(linked_reports), expected_reports),
    ):
        error = _expected_count_error(label, actual, expected)
        if error is not None:
            errors.append(error)

    reports: dict[Path, ReportRecord] = {}
    for report_path in sorted(linked_reports):
        report, report_errors = parse_report(report_path)
        errors.extend(report_errors)
        if report is not None:
            reports[report_path] = report

    states_by_report: dict[Path, set[bool]] = {}
    for finding in findings:
        for report_path in finding.reports:
            states_by_report.setdefault(report_path, set()).add(finding.checked)
            report = reports.get(report_path)
            if report is None:
                continue
            terminal = report.status in TERMINAL_STATUSES
            if finding.checked and not terminal:
                errors.append(
                    ReconciliationError(
                        f"{ledger}:{finding.line}",
                        f"checked row links non-terminal report `{report_path.relative_to(archive_root)}` with status `{report.status}`",
                    )
                )
            elif not finding.checked and terminal:
                errors.append(
                    ReconciliationError(
                        f"{ledger}:{finding.line}",
                        f"open row links terminal report `{report_path.relative_to(archive_root)}` with status `{report.status}`",
                    )
                )
    for report_path, states in sorted(states_by_report.items()):
        if len(states) > 1:
            errors.append(
                ReconciliationError(
                    str(report_path),
                    "the same report is linked from both checked and open checklist rows",
                )
            )

    if reports_root is not None:
        reports_root = reports_root.resolve()
        if not reports_root.is_relative_to(archive_root):
            errors.append(
                ReconciliationError(
                    str(reports_root), "reports root escapes the configured audit archive"
                )
            )
        elif reports_root.exists():
            ignored_names = {"readme.md", "index.md"}
            archived_reports = {
                path.resolve()
                for path in reports_root.rglob("*.md")
                if path.name.lower() not in ignored_names
            }
            for orphan in sorted(archived_reports - linked_reports):
                errors.append(
                    ReconciliationError(
                        str(orphan), "report is not linked from any checklist row"
                    )
                )
        else:
            errors.append(
                ReconciliationError(str(reports_root), "reports root does not exist")
            )

    return ReconciliationSummary(
        len(findings), checked, open_count, len(linked_reports), tuple(errors)
    )


def _optional_count(value: str) -> int | None:
    if value.lower() in {"any", "none", "skip"}:
        return None
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("expected counts must be non-negative")
    return parsed


def build_parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ledger", required=True, type=Path, help="Markdown checklist")
    parser.add_argument(
        "--archive-root",
        type=Path,
        help="audit archive root (defaults to the ledger directory)",
    )
    parser.add_argument(
        "--reports-root",
        type=Path,
        help="optional report directory whose unlinked Markdown files are errors",
    )
    parser.add_argument("--expected-total", type=_optional_count, default=561)
    parser.add_argument("--expected-checked", type=_optional_count, default=550)
    parser.add_argument("--expected-open", type=_optional_count, default=11)
    parser.add_argument("--expected-reports", type=_optional_count, default=303)
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit the summary as JSON instead of human-readable text",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run reconciliation and return zero only for a fully consistent archive."""

    args = build_parser().parse_args(argv)
    ledger = args.ledger.resolve()
    archive_root = (args.archive_root or ledger.parent).resolve()
    reports_root = args.reports_root
    if reports_root is not None and not reports_root.is_absolute():
        reports_root = archive_root / reports_root
    summary = reconcile(
        ledger,
        archive_root,
        reports_root=reports_root,
        expected_total=args.expected_total,
        expected_checked=args.expected_checked,
        expected_open=args.expected_open,
        expected_reports=args.expected_reports,
    )
    if args.json:
        print(json.dumps(asdict(summary), sort_keys=True, indent=2))
    else:
        print(
            "audit reconciliation: "
            f"{summary.total} total, {summary.checked} checked, "
            f"{summary.open} open, {summary.reports} reports"
        )
        for error in summary.errors:
            print(f"ERROR {error.location}: {error.message}", file=sys.stderr)
    return 1 if summary.errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
