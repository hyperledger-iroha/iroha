"""Tests for the read-only security-audit reconciliation guard."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "reconcile_security_audit.py"
SPEC = importlib.util.spec_from_file_location("reconcile_security_audit", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
audit = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit
SPEC.loader.exec_module(audit)


def _write_report(
    path: Path,
    *,
    classification: str,
    status: str,
    evidence: str = "Code path crates/example.rs:42 and focused test passed.",
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            (
                "# Finding",
                "",
                f"Classification: {classification}",
                f"Status: {status}",
                f"Evidence: {evidence}",
                "",
            )
        ),
        encoding="utf-8",
    )


def _reconcile(root: Path, **overrides: object) -> audit.ReconciliationSummary:
    options = {
        "expected_total": 2,
        "expected_checked": 1,
        "expected_open": 1,
        "expected_reports": 2,
    }
    options.update(overrides)
    return audit.reconcile(root / "TODO.md", root, **options)


def test_valid_mixed_archive_reconciles(tmp_path: Path) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [x] Rejected issue ([report](reports/rejected.md))\n"
        "- [ ] External evidence ([report](reports/external.md))\n",
        encoding="utf-8",
    )
    _write_report(
        tmp_path / "reports/rejected.md",
        classification="rejected with code-path proof",
        status="rejected",
    )
    _write_report(
        tmp_path / "reports/external.md",
        classification="externally evidence-blocked",
        status="external evidence blocked",
        evidence="Production HSM attestation and independent reviewer seal are unavailable.",
    )

    summary = _reconcile(tmp_path, reports_root=tmp_path / "reports")

    assert (summary.total, summary.checked, summary.open, summary.reports) == (2, 1, 1, 2)
    assert summary.errors == ()


def test_checked_source_work_is_a_contradiction(tmp_path: Path) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [x] Residual source flaw ([report](reports/open.md))\n",
        encoding="utf-8",
    )
    _write_report(
        tmp_path / "reports/open.md",
        classification="confirmed source work",
        status="open",
    )

    summary = _reconcile(
        tmp_path,
        expected_total=1,
        expected_checked=1,
        expected_open=0,
        expected_reports=1,
    )

    assert any("checked row links non-terminal report" in error.message for error in summary.errors)


def test_open_terminal_report_is_a_contradiction(tmp_path: Path) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [ ] Fixed issue ([report](reports/fixed.md))\n",
        encoding="utf-8",
    )
    _write_report(
        tmp_path / "reports/fixed.md",
        classification="source-fixed/validation-pending",
        status="fixed validated",
    )

    summary = _reconcile(
        tmp_path,
        expected_total=1,
        expected_checked=0,
        expected_open=1,
        expected_reports=1,
    )

    assert any("open row links terminal report" in error.message for error in summary.errors)


def test_placeholder_evidence_and_missing_report_fail(tmp_path: Path) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [ ] Placeholder ([report](reports/placeholder.md))\n"
        "- [ ] Missing ([report](reports/missing.md))\n",
        encoding="utf-8",
    )
    _write_report(
        tmp_path / "reports/placeholder.md",
        classification="confirmed-source-work",
        status="open",
        evidence="TODO",
    )

    summary = _reconcile(
        tmp_path,
        expected_total=2,
        expected_checked=0,
        expected_open=2,
        expected_reports=2,
    )

    messages = [error.message for error in summary.errors]
    assert any("evidence is a placeholder" in message for message in messages)
    assert any("cannot read report" in message for message in messages)


def test_report_link_cannot_escape_archive(tmp_path: Path) -> None:
    outside = tmp_path.parent / "outside-audit-report.md"
    _write_report(
        outside,
        classification="rejected-with-code-path-proof",
        status="rejected",
    )
    (tmp_path / "TODO.md").write_text(
        f"- [ ] Escape ([report](../{outside.name}))\n",
        encoding="utf-8",
    )

    summary = _reconcile(
        tmp_path,
        expected_total=1,
        expected_checked=0,
        expected_open=1,
        expected_reports=0,
    )

    assert any("escapes the configured audit archive" in error.message for error in summary.errors)


def test_same_report_cannot_back_checked_and_open_rows(tmp_path: Path) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [x] First row ([report](reports/shared.md))\n"
        "- [ ] Second row ([report](reports/shared.md))\n",
        encoding="utf-8",
    )
    _write_report(
        tmp_path / "reports/shared.md",
        classification="rejected-with-code-path-proof",
        status="rejected",
    )

    summary = _reconcile(
        tmp_path,
        expected_total=2,
        expected_checked=1,
        expected_open=1,
        expected_reports=1,
    )

    assert any(
        "both checked and open checklist rows" in error.message for error in summary.errors
    )


def test_bold_and_table_report_fields_are_supported(tmp_path: Path) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [x] Bold report ([report](reports/bold.md))\n"
        "- [x] Table report ([report](reports/table.md))\n",
        encoding="utf-8",
    )
    (tmp_path / "reports").mkdir()
    (tmp_path / "reports/bold.md").write_text(
        "# Bold\n\n"
        "**Classification:** rejected-with-code-path-proof\n"
        "**Status**: rejected\n"
        "**Evidence:** exact negative test and code path.\n",
        encoding="utf-8",
    )
    (tmp_path / "reports/table.md").write_text(
        "# Table\n\n"
        "| Classification | source-fixed-validation-pending |\n"
        "| Status | validated |\n"
        "| Evidence | focused test command passed |\n",
        encoding="utf-8",
    )

    summary = _reconcile(
        tmp_path,
        expected_total=2,
        expected_checked=2,
        expected_open=0,
        expected_reports=2,
    )

    assert summary.errors == ()


def test_cli_json_exit_status_tracks_errors(tmp_path: Path, capsys: object) -> None:
    (tmp_path / "TODO.md").write_text(
        "- [x] Closed ([report](reports/closed.md))\n",
        encoding="utf-8",
    )
    _write_report(
        tmp_path / "reports/closed.md",
        classification="source-fixed-validation-pending",
        status="validated",
    )

    result = audit.main(
        [
            "--ledger",
            str(tmp_path / "TODO.md"),
            "--expected-total",
            "1",
            "--expected-checked",
            "1",
            "--expected-open",
            "0",
            "--expected-reports",
            "1",
            "--json",
        ]
    )

    assert result == 0
    output = capsys.readouterr().out
    assert '"errors": []' in output
