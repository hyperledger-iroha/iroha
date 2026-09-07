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


def test_run_jvm_parity_checks_skips_without_jdk_outside_strict_mode(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.delenv("NORITO_JVM_SKIP_TESTS", raising=False)
    monkeypatch.delenv("NORITO_JVM_STRICT", raising=False)
    monkeypatch.delenv("CI", raising=False)
    monkeypatch.setattr(MODULE, "ensure_java_tool", lambda _tool: None)

    MODULE.run_jvm_parity_checks()

    stderr = capsys.readouterr().err
    assert "skipping JVM parity checks outside strict mode" in stderr


def test_run_jvm_parity_checks_fails_without_jdk_in_strict_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("NORITO_JVM_SKIP_TESTS", raising=False)
    monkeypatch.setenv("NORITO_JVM_STRICT", "1")
    monkeypatch.delenv("CI", raising=False)
    monkeypatch.setattr(MODULE, "ensure_java_tool", lambda _tool: None)

    with pytest.raises(MODULE.CheckError, match="javac not found"):
        MODULE.run_jvm_parity_checks()


def test_run_jvm_parity_checks_rejects_skip_in_strict_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("NORITO_JVM_SKIP_TESTS", "1")
    monkeypatch.setenv("NORITO_JVM_STRICT", "1")

    with pytest.raises(MODULE.CheckError, match="forbidden in strict JVM parity mode"):
        MODULE.run_jvm_parity_checks()


def test_update_flags_marks_kotlin_binding_changes() -> None:
    flags = MODULE.PathFlags()

    MODULE.update_flags(
        flags,
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/norito/NoritoCodec.kt",
    )

    assert flags.kotlin_updated is True


def test_main_requires_kotlin_binding_updates_for_norito_changes(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setattr(MODULE, "determine_base_ref", lambda: "origin/main")
    monkeypatch.setattr(MODULE, "compute_merge_base", lambda _base_ref: "merge-base")
    monkeypatch.setattr(
        MODULE,
        "gather_flags",
        lambda _merge_base: MODULE.PathFlags(
            needs_reference_update=True,
            python_updated=True,
            java_affected=True,
            kotlin_updated=False,
        ),
    )

    assert MODULE.main() == 1

    stderr = capsys.readouterr().err
    assert "kotlin/core-jvm" in stderr


def test_main_force_all_runs_every_binding_lane_on_clean_tree(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[str] = []
    monkeypatch.setenv("NORITO_BINDINGS_CHECK_ALL", "1")
    monkeypatch.setattr(MODULE, "determine_base_ref", lambda: "origin/main")
    monkeypatch.setattr(MODULE, "compute_merge_base", lambda _base_ref: "merge-base")
    monkeypatch.setattr(MODULE, "gather_flags", lambda _merge_base: MODULE.PathFlags())
    monkeypatch.setattr(MODULE, "run_python_parity_checks", lambda: calls.append("python"))
    monkeypatch.setattr(MODULE, "run_jvm_parity_checks", lambda: calls.append("jvm"))

    assert MODULE.main() == 0
    assert calls == ["python", "jvm"]


def test_ci_binding_gate_is_forced_strict_and_workflow_owned() -> None:
    root = MODULE.REPO_ROOT
    gate = (root / "ci" / "check_norito_bindings_sync.sh").read_text(encoding="utf-8")
    workflow = (root / ".github" / "workflows" / "openapi.yml").read_text(
        encoding="utf-8"
    )

    assert 'export NORITO_BINDINGS_CHECK_ALL="1"' in gate
    assert 'export NORITO_JVM_STRICT="1"' in gate
    assert "bash ci/check_norito_bindings_sync.sh" in workflow


@pytest.mark.parametrize("ci_value", ["true", "1", "yes", "on"])
def test_jvm_checks_are_strict_in_ci(monkeypatch: pytest.MonkeyPatch, ci_value: str) -> None:
    monkeypatch.delenv("NORITO_JVM_STRICT", raising=False)
    monkeypatch.setenv("CI", ci_value)
    assert MODULE.jvm_checks_are_strict()


def test_jvm_lane_runs_java_consumers_and_kotlin_fixtures(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("NORITO_JVM_SKIP_TESTS", raising=False)
    monkeypatch.setattr(MODULE, "ensure_java_tool", lambda tool: f"/jdk/bin/{tool}")
    calls = []
    monkeypatch.setattr(MODULE, "run_command", lambda args, **kwargs: calls.append((args, kwargs)))
    MODULE.run_jvm_parity_checks()
    assert len(calls) == 1
    args, options = calls[0]
    assert args[0] == MODULE.REPO_ROOT / "kotlin" / "gradlew"
    assert options["cwd"] == MODULE.REPO_ROOT / "kotlin"
    assert "org.hyperledger.iroha.sdk.norito.*" in args
    assert "org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapterParityTest" in args
    assert "org.hyperledger.iroha.sdk.tx.norito.TransactionFixtureParityTest" in args


def test_java_source_changes_select_canonical_jvm_lane(monkeypatch: pytest.MonkeyPatch) -> None:
    calls = []
    flags = MODULE.PathFlags()
    MODULE.update_flags(flags, "java/norito_java/src/main/java/Codec.java")
    monkeypatch.delenv("NORITO_BINDINGS_CHECK_ALL", raising=False)
    monkeypatch.setattr(MODULE, "determine_base_ref", lambda: "origin/main")
    monkeypatch.setattr(MODULE, "compute_merge_base", lambda _: "base")
    monkeypatch.setattr(MODULE, "gather_flags", lambda _: flags)
    monkeypatch.setattr(MODULE, "run_jvm_parity_checks", lambda: calls.append("jvm"))
    assert MODULE.main() == 0
    assert calls == ["jvm"]


def test_rust_changes_require_only_canonical_binding_updates(monkeypatch: pytest.MonkeyPatch) -> None:
    calls = []
    monkeypatch.delenv("NORITO_BINDINGS_CHECK_ALL", raising=False)
    monkeypatch.setattr(MODULE, "determine_base_ref", lambda: "origin/main")
    monkeypatch.setattr(MODULE, "compute_merge_base", lambda _: "base")
    monkeypatch.setattr(MODULE, "gather_flags", lambda _: MODULE.PathFlags(
        needs_reference_update=True, python_updated=True, kotlin_updated=True))
    monkeypatch.setattr(MODULE, "run_python_parity_checks", lambda: calls.append("python"))
    monkeypatch.setattr(MODULE, "run_jvm_parity_checks", lambda: calls.append("jvm"))
    assert MODULE.main() == 0
    assert calls == ["python", "jvm"]
