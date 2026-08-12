"""Unit tests for the focused Cargo dependency-budget guard."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_dependency_budget.py"
SPEC = importlib.util.spec_from_file_location("check_dependency_budget", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

FIXTURE = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "dependency_budget_metadata.json"
)


def _write_package(root: Path, relative: str, manifest: str) -> None:
    directory = root / relative
    directory.mkdir(parents=True)
    (directory / "Cargo.toml").write_text(manifest, encoding="utf-8")


def _write_manifest_fixture(root: Path) -> None:
    (root / "Cargo.toml").write_text(
        """\
[workspace]
resolver = "2"
members = ["crates/*"]
exclude = ["crates/excluded"]

[workspace.dependencies]
model = { package = "model", path = "crates/model" }
codec = "1"

[patch.crates-io]
patched = { path = "vendor/patched" }
""",
        encoding="utf-8",
    )
    _write_package(
        root,
        "crates/model",
        """\
[package]
name = "model"
version = "0.1.0"

[dependencies]
codec = { workspace = true }
image = { version = "1", optional = true }
patched = "1"

[build-dependencies]
build-helper = { path = "../../vendor/build-helper" }

[target.'cfg(unix)'.dependencies]
unix-only = "1"
""",
    )
    _write_package(
        root,
        "crates/daemon",
        """\
[package]
name = "daemon"
version = "0.1.0"

[dependencies]
model = { workspace = true }
tokio = "1"

[dev-dependencies]
criterion = "1"
""",
    )
    _write_package(
        root,
        "crates/cli",
        """\
[package]
name = "cli"
version = "0.1.0"

[dependencies]
model = { workspace = true }
image = { version = "1", optional = true }
""",
    )
    _write_package(
        root,
        "crates/excluded",
        """\
[package]
name = "excluded"
version = "0.1.0"

[dependencies]
forbidden = "1"
""",
    )
    _write_package(
        root,
        "vendor/build-helper",
        """\
[package]
name = "build-helper"
version = "0.1.0"

[dependencies]
syn = "2"
""",
    )
    _write_package(
        root,
        "vendor/patched",
        """\
[package]
name = "patched"
version = "1.0.0"

[dependencies]
quote = "1"
""",
    )


def _scope(limits: dict[str, int]) -> dict[str, object]:
    return {
        "roots": ["model"],
        "include_root_dev_dependencies": False,
        "limits": limits,
    }


def _config(limits: dict[str, int], denied: list[str] | None = None) -> dict:
    return {
        "schema_version": 1,
        "measurement": {"kind": MODULE.MEASUREMENT_KIND},
        "denied_required_packages": denied or [],
        "scopes": {"model": _scope(limits)},
    }


def test_manifest_graph_measures_required_and_declared_closures(
    tmp_path: Path,
) -> None:
    _write_manifest_fixture(tmp_path)
    graph = MODULE.load_manifest_graph(tmp_path / "Cargo.toml")

    metrics, required, declared = MODULE.measure_scope(
        graph,
        roots=["model"],
        include_root_dev_dependencies=False,
    )

    assert metrics == {
        "required_local_packages": 3,
        "required_workspace_packages": 1,
        "required_path_packages": 2,
        "required_external_packages": 4,
        "required_dependency_edges": 6,
        "required_external_dependency_edges": 4,
        "declared_local_packages": 3,
        "declared_workspace_packages": 1,
        "declared_path_packages": 2,
        "declared_external_packages": 5,
        "declared_dependency_edges": 7,
        "declared_external_dependency_edges": 5,
    }
    assert "image" not in required.package_names
    assert "image" in declared.package_names
    assert "forbidden" not in declared.package_names


def test_workspace_scope_includes_dev_dependencies_only_for_workspace_roots(
    tmp_path: Path,
) -> None:
    _write_manifest_fixture(tmp_path)
    graph = MODULE.load_manifest_graph(tmp_path / "Cargo.toml")

    _metrics, required, _declared = MODULE.measure_scope(
        graph,
        roots=["*"],
        include_root_dev_dependencies=True,
    )

    assert "criterion" in required.package_names
    # Path dependencies are not workspace roots, so their own dev graphs would
    # remain excluded even if one were added to this fixture.
    assert "forbidden" not in required.package_names


def test_budget_is_an_exact_no_growth_ratchet(tmp_path: Path) -> None:
    _write_manifest_fixture(tmp_path)
    graph = MODULE.load_manifest_graph(tmp_path / "Cargo.toml")
    metrics, _required, _declared = MODULE.measure_scope(
        graph,
        roots=["model"],
        include_root_dev_dependencies=False,
    )

    report, violations = MODULE.build_source_report(graph, _config(metrics))
    assert violations == []
    assert report["within_budget"] is True

    lower_limits = dict(metrics)
    lower_limits["required_external_packages"] -= 1
    report, violations = MODULE.build_source_report(graph, _config(lower_limits))
    assert report["within_budget"] is False
    assert violations == [
        "model: required_external_packages 4 exceeds limit 3"
    ]


def test_denied_packages_apply_to_required_not_optional_closure(
    tmp_path: Path,
) -> None:
    _write_manifest_fixture(tmp_path)
    graph = MODULE.load_manifest_graph(tmp_path / "Cargo.toml")
    metrics, _required, _declared = MODULE.measure_scope(
        graph,
        roots=["model"],
        include_root_dev_dependencies=False,
    )

    _report, violations = MODULE.build_source_report(
        graph, _config(metrics, ["image"])
    )
    assert violations == []

    _report, violations = MODULE.build_source_report(
        graph, _config(metrics, ["codec"])
    )
    assert violations == ["model: denied package `codec` is required"]


def test_reviewed_manifest_fingerprint_fails_closed(tmp_path: Path) -> None:
    _write_manifest_fixture(tmp_path)
    graph = MODULE.load_manifest_graph(tmp_path / "Cargo.toml")
    metrics, _required, _declared = MODULE.measure_scope(
        graph,
        roots=["model"],
        include_root_dev_dependencies=False,
    )
    config = _config(metrics)
    config["baseline"] = {"manifest_fingerprint": "sha256:" + "00" * 32}

    report, violations = MODULE.build_source_report(graph, config)

    assert report["fingerprint_matches_baseline"] is False
    assert report["within_budget"] is False
    assert violations == [
        "manifest fingerprint differs from the reviewed dependency baseline: "
        f"{graph.manifest_fingerprint} != {'sha256:' + '00' * 32}"
    ]


def test_refresh_sets_observed_metrics_and_content_fingerprint(tmp_path: Path) -> None:
    _write_manifest_fixture(tmp_path)
    graph = MODULE.load_manifest_graph(tmp_path / "Cargo.toml")
    zero_limits = {metric: 0 for metric in MODULE.METRIC_KEYS}
    config = _config(zero_limits)
    report, _violations = MODULE.build_source_report(graph, config)

    refreshed = MODULE.refreshed_config(config, graph, report)

    assert refreshed["scopes"]["model"]["limits"] == report["scopes"]["model"][
        "metrics"
    ]
    assert refreshed["baseline"]["manifest_fingerprint"] == graph.manifest_fingerprint
    assert graph.manifest_fingerprint.startswith("sha256:")


def test_config_rejects_incomplete_metric_limits(tmp_path: Path) -> None:
    config_path = tmp_path / "budget.json"
    config_path.write_text(
        json.dumps(_config({"required_local_packages": 1})),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="missing"):
        MODULE.load_budget_config(config_path)


def test_captured_metadata_supports_focused_and_workspace_closures() -> None:
    metadata = json.loads(FIXTURE.read_text(encoding="utf-8"))

    model = MODULE.resolved_report(
        metadata,
        root_names=["iroha_data_model"],
        workspace=False,
        watched=["serde_json", "tokio"],
    )
    assert model["total_packages"] == 3
    assert model["package_sources"] == {
        "registry": 1,
        "path": 2,
        "git": 0,
        "other": 0,
    }
    assert model["watched_packages"] == {
        "serde_json": ["1.0.0"],
        "tokio": [],
    }

    workspace = MODULE.resolved_report(
        metadata,
        root_names=[],
        workspace=True,
        watched=[],
    )
    assert workspace["total_packages"] == 6


def test_resolved_metadata_command_is_locked_by_default() -> None:
    args = argparse.Namespace(
        manifest_path=Path("Cargo.toml"),
        allow_lock_update=False,
    )

    assert MODULE.cargo_metadata_command(args) == [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--manifest-path",
        "Cargo.toml",
        "--locked",
    ]


def test_stdout_json_is_not_mixed_with_human_diagnostics(capsys: pytest.CaptureFixture) -> None:
    result = MODULE.main(
        [
            "--metadata-json",
            str(FIXTURE),
            "-p",
            "iroha_data_model",
            "--json-out",
            "-",
        ]
    )

    captured = capsys.readouterr()
    assert result == 0
    assert json.loads(captured.out)["total_packages"] == 3
    assert "total_packages=3" in captured.err
