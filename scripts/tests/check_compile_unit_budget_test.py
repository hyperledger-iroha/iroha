"""Unit tests for the Cargo compile-unit budget reporter."""

from __future__ import annotations

import importlib.util
import io
import json
from collections import Counter
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_compile_unit_budget.py"
SPEC = importlib.util.spec_from_file_location("check_compile_unit_budget", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def test_baseline_limit_uses_larger_of_three_or_two_percent() -> None:
    assert MODULE.baseline_limit(10) == 13
    assert MODULE.baseline_limit(200) == 204
    assert MODULE.baseline_limit(0) == 3


@pytest.mark.parametrize(
    ("baseline", "percent", "minimum"),
    [(-1, 2.0, 3), (1, -0.1, 3), (1, 2.0, -1)],
)
def test_baseline_limit_rejects_negative_inputs(
    baseline: int, percent: float, minimum: int
) -> None:
    with pytest.raises(ValueError):
        MODULE.baseline_limit(
            baseline,
            percent=percent,
            minimum_growth=minimum,
        )


def test_load_baseline_supports_root_and_keyed_reports(tmp_path: Path) -> None:
    root = tmp_path / "root.json"
    root.write_text('{"compile_units": 41}\n', encoding="utf-8")
    assert MODULE.load_baseline(root, None) == 41

    keyed = tmp_path / "keyed.json"
    keyed.write_text(
        '{"foundation": {"compile_units": 17}}\n',
        encoding="utf-8",
    )
    assert MODULE.load_baseline(keyed, "foundation") == 17


def test_load_baseline_rejects_invalid_values(tmp_path: Path) -> None:
    path = tmp_path / "invalid.json"
    path.write_text('{"compile_units": true}\n', encoding="utf-8")
    with pytest.raises(ValueError):
        MODULE.load_baseline(path, None)


def test_report_and_json_output_are_deterministic(tmp_path: Path) -> None:
    artifacts = {
        ("pkg-a", "alpha", ("lib",), "/src/a.rs"),
        ("pkg-b", "beta", ("test",), "/src/b.rs"),
    }
    report = MODULE.build_report(
        command=["cargo", "test"],
        artifact_scope="workspace",
        artifacts=artifacts,
        artifact_package_ids={"pkg-a", "pkg-b"},
        source_counts=Counter({"path": 1, "registry": 1}),
        package_artifacts=Counter({"beta": 1, "alpha": 1}),
        baseline=1,
        limit=4,
    )

    assert report["compile_units"] == 2
    assert report["artifact_scope"] == "workspace"
    assert report["within_budget"] is True
    assert report["package_sources"]["git"] == 0

    output = tmp_path / "report.json"
    MODULE.write_json_report(report, output)
    parsed = json.loads(output.read_text(encoding="utf-8"))
    assert parsed == report
    assert output.read_text(encoding="utf-8").endswith("\n")


def test_human_report_includes_budget_context() -> None:
    report = {
        "compile_units": 7,
        "artifact_packages": 4,
        "package_sources": {"registry": 2, "path": 2, "git": 0, "other": 0},
        "top_packages": [{"name": "iroha_data_model", "compile_units": 3}],
        "baseline_compile_units": 5,
        "limit_compile_units": 8,
    }
    output = io.StringIO()

    MODULE.write_human_report(report, output)

    assert output.getvalue() == (
        "compile_units=7\n"
        "artifact_packages=4\n"
        "registry_packages=2\n"
        "path_packages=2\n"
        "git_packages=0\n"
        "baseline_compile_units=5\n"
        "limit_compile_units=8\n"
        "top_packages:\n"
        "  iroha_data_model: 3\n"
    )


def test_report_sorts_equal_count_packages_by_name() -> None:
    report = MODULE.build_report(
        command=["cargo", "test"],
        artifact_scope="workspace",
        artifacts={
            ("pkg-z", "zeta", ("lib",), "/src/z.rs"),
            ("pkg-a", "alpha", ("lib",), "/src/a.rs"),
        },
        artifact_package_ids={"pkg-z", "pkg-a"},
        source_counts=Counter({"path": 2}),
        package_artifacts=Counter({"zeta": 1, "alpha": 1}),
        baseline=None,
        limit=None,
    )

    assert [entry["name"] for entry in report["top_packages"]] == [
        "alpha",
        "zeta",
    ]


def test_focused_command_is_locked_and_library_only() -> None:
    args = MODULE.argparse.Namespace(
        manifest_path=Path("Cargo.toml"),
        target_dir=None,
        allow_lock_update=False,
        workspace=False,
        package=["iroha_data_model"],
        lib=True,
    )

    assert MODULE.cargo_test_command(args) == [
        "cargo",
        "test",
        "--no-run",
        "--message-format=json",
        "--manifest-path",
        "Cargo.toml",
        "--locked",
        "-p",
        "iroha_data_model",
        "--lib",
    ]


def test_workspace_scope_excludes_registry_artifacts() -> None:
    workspace_members = {"path+file:///repo/crates/model#iroha_data_model@0.1.0"}
    member = next(iter(workspace_members))
    registry = "registry+https://github.com/rust-lang/crates.io-index#syn@2.0.0"

    assert MODULE.artifact_in_scope(member, "workspace", workspace_members)
    assert not MODULE.artifact_in_scope(registry, "workspace", workspace_members)
    assert MODULE.artifact_in_scope(registry, "all", workspace_members)
    with pytest.raises(ValueError, match="unsupported artifact scope"):
        MODULE.artifact_in_scope(member, "host-dependent", workspace_members)


def test_compiler_diagnostics_are_retained_for_failed_builds() -> None:
    message = {
        "reason": "compiler-message",
        "message": {
            "rendered": "error[E0001]: first line\n  --> src/lib.rs:2:3\n",
        },
    }

    assert MODULE.compiler_diagnostic_lines(message) == (
        "error[E0001]: first line",
        "  --> src/lib.rs:2:3",
    )
    assert MODULE.compiler_diagnostic_lines({"reason": "compiler-artifact"}) == ()
    assert (
        MODULE.compiler_diagnostic_lines(
            {"reason": "compiler-message", "message": {"rendered": None}}
        )
        == ()
    )
