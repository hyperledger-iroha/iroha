"""Tests for the Android telemetry redaction status helper."""

from __future__ import annotations

import io
import json
from pathlib import Path
import importlib.util
import sys

import pytest

MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "telemetry" / "check_redaction_status.py"
)
SPEC = importlib.util.spec_from_file_location("check_redaction_status", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)  # type: ignore[misc]


def make_summary(**kwargs):
    return checker.Summary(
        timestamp=kwargs.get("timestamp", "2026-03-01T12:00:00Z"),
        hashed=kwargs.get("hashed", 5),
        mismatched=kwargs.get("mismatched", 0),
        salt_epoch=kwargs.get("salt_epoch", "2026Q1"),
        salt_rotation=kwargs.get("salt_rotation", "salt-01"),
        failures=kwargs.get("failures", []),
        exporters=kwargs.get("exporters", []),
    )


def test_summary_to_payload_round_trip() -> None:
    summary = make_summary(
        failures=[{"authority": "demo", "reason": "test", "count": 1}],
        exporters=[{"backend": "otlp", "status": "ok", "backlog": 0}],
    )
    payload = checker.summary_to_payload(summary)

    assert payload["timestamp"] == summary.timestamp
    assert payload["hashed_authorities"] == summary.hashed
    assert payload["mismatched_authorities"] == summary.mismatched
    assert payload["salt"] == {"epoch": "2026Q1", "rotation_id": "salt-01"}
    assert payload["recent_failures"] == summary.failures
    assert payload["exporters"] == summary.exporters


def test_write_json_to_path_creates_parent(tmp_path: Path) -> None:
    payload = {"timestamp": "ts", "hashed_authorities": 1}
    target = tmp_path / "nested" / "status.json"

    checker.write_json_to_path(target, payload)

    assert target.exists()
    content = target.read_text()
    assert content.endswith("\n")
    assert json.loads(content) == payload


def test_print_summary_respects_custom_output() -> None:
    summary = make_summary(
        failures=[{"authority": "demo", "reason": "x", "count": 1}],
        exporters=[{"backend": "otlp", "status": "ok", "backlog": 0}],
    )
    buffer = io.StringIO()

    exit_code = checker.print_summary(
        summary,
        max_failures=5,
        warn_backlog=0.0,
        expected_salt_epoch=None,
        expected_salt_rotation=None,
        min_hashed=None,
        output=buffer,
    )

    output = buffer.getvalue()
    assert "STATUS" in output
    assert "EXPORT" in output
    assert exit_code == 0


def test_print_summary_handles_json_only_mode(capfd: pytest.CaptureFixture) -> None:
    summary = make_summary(mismatched=2, failures=[{"reason": "demo"}])
    buffer = io.StringIO()
    exit_code = checker.print_summary(
        summary,
        max_failures=5,
        warn_backlog=0.0,
        expected_salt_epoch=None,
        expected_salt_rotation=None,
        min_hashed=None,
        output=buffer,
    )
    assert exit_code == 2
    # ensure default stdout can be suppressed externally; nothing written to capfd
    captured = capfd.readouterr()
    assert captured.out == ""


def test_print_summary_enforces_min_hashed(capfd: pytest.CaptureFixture) -> None:
    summary = make_summary(hashed=98)
    exit_code = checker.print_summary(
        summary,
        max_failures=5,
        warn_backlog=0.0,
        expected_salt_epoch=None,
        expected_salt_rotation=None,
        min_hashed=120,
        output=None,
    )
    assert exit_code == 2
    captured = capfd.readouterr().out
    assert "android.telemetry.redaction.hashed_authorities" in captured
    assert "threshold=120" in captured


def test_print_summary_accepts_min_hashed_when_satisfied(capfd: pytest.CaptureFixture) -> None:
    summary = make_summary(hashed=150)
    exit_code = checker.print_summary(
        summary,
        max_failures=5,
        warn_backlog=0.0,
        expected_salt_epoch=None,
        expected_salt_rotation=None,
        min_hashed=120,
        output=None,
    )
    assert exit_code == 0
    captured = capfd.readouterr().out
    assert "hashed_authorities" in captured
    assert "ALERT   android.telemetry.redaction.hashed_authorities" not in captured
