"""Tests for scripts/check_norito_bindings_sync.py."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "check_norito_bindings_sync.py"
SPEC = importlib.util.spec_from_file_location("check_norito_bindings_sync", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_run_java_parity_checks_skips_without_jdk_outside_strict_mode(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.delenv("NORITO_JAVA_SKIP_TESTS", raising=False)
    monkeypatch.delenv("NORITO_JAVA_STRICT", raising=False)
    monkeypatch.delenv("CI", raising=False)
    monkeypatch.setattr(MODULE, "ensure_java_tool", lambda _tool: None)

    MODULE.run_java_parity_checks()

    stderr = capsys.readouterr().err
    assert "skipping JVM parity checks outside strict mode" in stderr


def test_run_java_parity_checks_fails_without_jdk_in_strict_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("NORITO_JAVA_SKIP_TESTS", raising=False)
    monkeypatch.setenv("NORITO_JAVA_STRICT", "1")
    monkeypatch.delenv("CI", raising=False)
    monkeypatch.setattr(MODULE, "ensure_java_tool", lambda _tool: None)

    with pytest.raises(MODULE.CheckError, match="javac not found"):
        MODULE.run_java_parity_checks()


def test_java_checks_are_strict_when_ci_true(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("NORITO_JAVA_STRICT", raising=False)
    monkeypatch.setenv("CI", "true")

    assert MODULE.java_checks_are_strict() is True
