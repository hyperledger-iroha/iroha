"""Unit tests for the focused Cargo dependency-budget guard."""

from __future__ import annotations

import argparse
import importlib.util
import json
import shutil
import subprocess
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


def _boundary_policy() -> dict:
    return {
        "schema_version": 1,
        "layers": {
            "sdk": ["sdk"],
            "wire": ["wire", "compiler"],
            "node": ["engine", "optional-engine", "dev-only"],
            "proof": ["proof"],
        },
        "configurations": {
            "sdk": {
                "package": "sdk", "default_features": False, "features": [],
                "target": "all", "forbidden_layers": ["node"],
                "forbidden_features": {"proof": ["full"]},
            },
        },
    }


@pytest.mark.parametrize("field,value", [
    ("package", "unowned"),
    ("default_features", None),
    ("target", ""),
    ("features", ["a", "a"]),
    ("forbidden_layers", ["missing"]),
    ("forbidden_layers", ["sdk"]),
    ("forbidden_features", {"unowned": ["full"]}),
])
def test_boundary_policy_rejects_ambiguous_selections(field: str, value: object) -> None:
    policy = _boundary_policy()
    policy["configurations"]["sdk"][field] = value
    with pytest.raises(ValueError):
        MODULE.validate_boundary_policy({"architecture": policy})


def test_boundary_policy_rejects_duplicate_ownership() -> None:
    policy = _boundary_policy()
    policy["layers"]["wire"].append("sdk")
    with pytest.raises(ValueError, match="multiple layer owners"):
        MODULE.validate_boundary_policy({"architecture": policy})


def test_boundary_command_selects_features_targets_and_dependency_kinds() -> None:
    policy = _boundary_policy()
    MODULE.validate_boundary_policy({"architecture": policy})
    selection = policy["configurations"]["sdk"]
    selection["features"] = ["tls", "sm"]
    command = MODULE.boundary_tree_command(Path("Cargo.toml"), selection, offline=True)
    assert command == [
        "cargo", "tree", "--manifest-path", "Cargo.toml", "--locked",
        "--package", "sdk", "--edges", "normal,build", "--target", "all",
        "--prefix", "depth", "--format", "|{p}|{f}", "--charset", "ascii",
        "--color", "never", "--no-default-features", "--features", "tls,sm",
        "--offline",
    ]
    selection["default_features"] = True
    assert "--no-default-features" not in MODULE.boundary_tree_command(
        Path("Cargo.toml"), selection, offline=False
    )


@pytest.mark.parametrize("tree", [
    "", "not a tree", "0|wrong v1.0.0|", "1|sdk v1.0.0|",
    "0|sdk v1.0.0|\n2|wire v1.0.0|", "0|sdk v1.0.0|\n0|sdk v1.0.0|",
    "0|sdk|", "0|sdk v1.0.0|\n[build-dependencies]",
])
def test_boundary_tree_rejects_incomplete_or_mismatched_output(tree: str) -> None:
    with pytest.raises(ValueError):
        MODULE.parse_boundary_tree(tree, "sdk")


def test_boundary_tree_reports_paths_and_feature_leaks_in_deduplicated_rows() -> None:
    policy = _boundary_policy()
    tree = """0|sdk v1.0.0 (/workspace/sdk)|default
1|wire v1.0.0 (/workspace/wire)|
2|compiler v1.0.0 (/workspace/compiler)|
3|engine v1.0.0 (/workspace/engine)|runtime
1|engine v1.0.0 (/workspace/engine)|runtime (*)
1|proof v1.0.0 (/workspace/proof)|model-primitives
1|proof v1.0.0 (/workspace/proof)|full,model-primitives (*)
"""
    report = MODULE.evaluate_boundary_tree(policy, policy["configurations"]["sdk"], tree)
    assert not report["within_boundary"]
    assert report["violations"] == [
        {"package": "engine", "forbidden_layer": "node", "path": ["sdk", "engine"]},
        {"package": "proof", "forbidden_feature": "full", "path": ["sdk", "proof"]},
    ]
    assert len(report["resolved_tree_sha256"]) == 64


def test_boundary_mode_fails_closed_on_cargo_failure(tmp_path, monkeypatch, capsys) -> None:
    config = _config({key: 100 for key in MODULE.METRIC_KEYS})
    config["architecture"] = _boundary_policy()
    path = tmp_path / "budget.json"
    path.write_text(json.dumps(config))
    monkeypatch.setattr(MODULE.subprocess, "run", lambda *a, **k: subprocess.CompletedProcess(
        a[0], 101, "", "lockfile needs to be updated"
    ))
    assert MODULE.main(["--config", str(path), "--check-boundaries"]) == 2
    assert "lockfile needs to be updated" in capsys.readouterr().err


def test_boundary_mode_emits_json_and_cannot_refresh_a_violation(tmp_path, monkeypatch, capsys) -> None:
    config = _config({key: 100 for key in MODULE.METRIC_KEYS})
    config["architecture"] = _boundary_policy()
    path = tmp_path / "budget.json"
    path.write_text(json.dumps(config))
    monkeypatch.setattr(MODULE.subprocess, "run", lambda *a, **k: subprocess.CompletedProcess(
        a[0], 0, "0|sdk v1.0.0|\n1|engine v1.0.0|", ""
    ))
    assert MODULE.main(["--config", str(path), "--check-boundaries", "--json-out", "-"]) == 1
    captured = capsys.readouterr()
    assert not json.loads(captured.out)["within_boundary"]
    assert "sdk -> engine" in captured.err
    before = path.read_bytes()
    assert MODULE.main(["--config", str(path), "--check-boundaries", "--write-baseline"]) == 2
    assert path.read_bytes() == before


@pytest.mark.skipif(shutil.which("cargo") is None, reason="Cargo required for feature resolution")
def test_boundary_resolution_covers_builds_excludes_dev_and_isolates_features(tmp_path: Path) -> None:
    """Exercise Cargo, including features enabled only by another workspace root."""

    (tmp_path / "Cargo.toml").write_text('[workspace]\nresolver="2"\nmembers=["crates/*"]\n')
    dependencies = {
        "sdk": '''[dependencies]
wire={path="../wire"}
optional-engine={path="../optional-engine", optional=true}
[build-dependencies]
compiler={path="../compiler"}
[dev-dependencies]
dev-only={path="../dev-only"}
[features]
execution=["dep:optional-engine"]
''',
        "wire": '''[dependencies]
proof={path="../proof", default-features=false}
[features]
execution=["proof/full"]
''',
        "compiler": '[build-dependencies]\nengine={path="../engine"}\n',
        "daemon": '[dependencies]\nwire={path="../wire", features=["execution"]}\n',
        "proof": '[features]\nfull=[]\n',
        "engine": "", "dev-only": "", "optional-engine": "",
    }
    for name, deps in dependencies.items():
        _write_package(tmp_path, f"crates/{name}", f'[package]\nname="{name}"\nversion="1.0.0"\n{deps}')
        source = tmp_path / "crates" / name / "src"
        source.mkdir()
        (source / "lib.rs").write_text("//! Dependency resolution fixture.\n")
    manifest = tmp_path / "Cargo.toml"
    subprocess.run(["cargo", "generate-lockfile", "--offline", "--manifest-path", str(manifest)], check=True, capture_output=True)
    policy = _boundary_policy()
    selection = policy["configurations"]["sdk"]
    tree = subprocess.check_output(MODULE.boundary_tree_command(manifest, selection, offline=True), text=True)
    report = MODULE.evaluate_boundary_tree(policy, selection, tree)
    assert report["violations"] == [{
        "package": "engine", "forbidden_layer": "node", "path": ["sdk", "compiler", "engine"],
    }]
    assert "dev-only" not in report["packages"]
    assert "optional-engine" not in report["packages"]
    selection["features"] = ["execution"]
    tree = subprocess.check_output(MODULE.boundary_tree_command(manifest, selection, offline=True), text=True)
    assert "optional-engine" in MODULE.evaluate_boundary_tree(policy, selection, tree)["packages"]


@pytest.mark.parametrize("value", [None, 0, 1, "true", "false", [], {}])
def test_boundary_policy_rejects_nonboolean_root_dev_selection(value: object) -> None:
    policy = _boundary_policy()
    MODULE.validate_boundary_policy({"architecture": policy})
    policy["configurations"]["sdk"]["include_root_dev_dependencies"] = value
    with pytest.raises(
        ValueError,
        match="^boundary `sdk` include_root_dev_dependencies must be boolean$",
    ):
        MODULE.validate_boundary_policy({"architecture": policy})


def test_boundary_root_dev_opt_in_preserves_shipping_command() -> None:
    policy = _boundary_policy()
    selection = policy["configurations"]["sdk"]
    shipping = MODULE.boundary_tree_command(Path("Cargo.toml"), selection, offline=True)
    selection["include_root_dev_dependencies"] = False
    MODULE.validate_boundary_policy({"architecture": policy})
    assert MODULE.boundary_tree_command(Path("Cargo.toml"), selection, offline=True) == shipping

    selection["include_root_dev_dependencies"] = True
    MODULE.validate_boundary_policy({"architecture": policy})
    expected = list(shipping)
    expected[expected.index("--edges") + 1] = "normal,build,dev"
    actual = MODULE.boundary_tree_command(Path("Cargo.toml"), selection, offline=True)
    assert actual == expected
    assert actual.count("--package") == 1
    assert "--workspace" not in actual
    assert "--all-features" not in actual
    assert "--prune" not in actual


def test_aggregate_model_test_boundary_retains_protocol_ownership_and_denials() -> None:
    path = Path(__file__).resolve().parents[2] / "ci" / "dependency_budget.json"
    policy = MODULE.validate_boundary_policy(json.loads(path.read_text()))
    shipping = policy["configurations"]["aggregate-model"]
    tests = policy["configurations"]["aggregate-model-tests"]
    assert tests == {
        **shipping,
        "default_features": True,
        "features": ["http"],
        "include_root_dev_dependencies": True,
    }
    assert tests["package"] == "iroha_data_model"
    assert tests["target"] == "all"
    assert set(tests["forbidden_layers"]) == {
        "node_execution", "node_configuration", "telemetry_runtime", "storage_runtime",
    }
    assert tests["forbidden_features"] == {
        "iroha_zkp_halo2": ["full", "parallel", "goldilocks_backend"],
    }
    assert "ivm_abi" in policy["layers"]["protocol"]
    assert "ivm" in policy["layers"]["node_execution"]
    for selection, expected_edges in [(shipping, "normal,build"), (tests, "normal,build,dev")]:
        command = MODULE.boundary_tree_command(Path("Cargo.toml"), selection, offline=True)
        assert command[command.index("--edges") + 1] == expected_edges
    baseline = "0|iroha_data_model v1.0.0|\n1|ivm_abi v1.0.0|\n"
    assert MODULE.evaluate_boundary_tree(policy, tests, baseline)["within_boundary"]
    with_engine = MODULE.evaluate_boundary_tree(policy, tests, baseline + "2|ivm v1.0.0|\n")
    assert with_engine["violations"] == [{
        "package": "ivm", "forbidden_layer": "node_execution",
        "path": ["iroha_data_model", "ivm_abi", "ivm"],
    }]


@pytest.mark.parametrize("owner", ["wire", "compiler", "test-support"])
def test_boundary_test_closure_rejects_forbidden_transitive_paths(owner: str) -> None:
    policy = _boundary_policy()
    selection = policy["configurations"]["sdk"]
    selection["include_root_dev_dependencies"] = True
    policy["layers"]["wire"].append("test-support")
    MODULE.validate_boundary_policy({"architecture": policy})
    baseline = f"0|sdk v1.0.0|\n1|{owner} v1.0.0|\n"
    assert MODULE.evaluate_boundary_tree(policy, selection, baseline)["within_boundary"]
    mutated = baseline + "2|engine v1.0.0|runtime\n"
    assert mutated != baseline
    report = MODULE.evaluate_boundary_tree(policy, selection, mutated)
    assert report["violations"] == [{
        "package": "engine", "forbidden_layer": "node", "path": ["sdk", owner, "engine"],
    }]


def test_boundary_mode_enforces_explicit_test_selection(tmp_path, monkeypatch, capsys) -> None:
    policy = _boundary_policy()
    shipping = policy["configurations"]["sdk"]
    policy["configurations"]["sdk-tests"] = {
        **shipping, "include_root_dev_dependencies": True,
    }
    policy["layers"]["wire"].append("test-support")
    config = _config({key: 100 for key in MODULE.METRIC_KEYS})
    config["architecture"] = policy
    path = tmp_path / "budget.json"
    path.write_text(json.dumps(config))
    commands = []

    def resolve(command, **kwargs):
        commands.append(command)
        tree = "0|sdk v1.0.0|\n1|wire v1.0.0|\n"
        if command[command.index("--edges") + 1] == "normal,build,dev":
            tree += "1|test-support v1.0.0|\n2|engine v1.0.0|runtime\n"
        return subprocess.CompletedProcess(command, 0, tree, "")

    monkeypatch.setattr(MODULE.subprocess, "run", resolve)
    assert MODULE.main(["--config", str(path), "--check-boundaries", "--json-out", "-"]) == 1
    output = capsys.readouterr()
    report = json.loads(output.out)
    assert report["measurement_kind"] == "cargo-feature-resolved-layer-boundaries-v1"
    assert report["configurations"]["sdk"]["within_boundary"]
    assert report["configurations"]["sdk-tests"]["violations"] == [{
        "package": "engine", "forbidden_layer": "node", "path": ["sdk", "test-support", "engine"],
    }]
    assert not report["within_boundary"]
    assert "sdk-tests: sdk -> test-support -> engine (node)" in output.err
    assert [command[command.index("--edges") + 1] for command in commands] == [
        "normal,build", "normal,build,dev",
    ]


@pytest.mark.skipif(shutil.which("cargo") is None, reason="Cargo required for feature resolution")
def test_boundary_resolution_includes_only_root_dev_dependencies(tmp_path: Path) -> None:
    """Qualify root-only dev resolution and normal/build closure of its test helpers."""

    (tmp_path / "Cargo.toml").write_text('[workspace]\nresolver="2"\nmembers=["crates/*"]\n')
    dependencies = {
        "sdk": '''[dependencies]
wire={path="../wire"}
[build-dependencies]
compiler={path="../compiler"}
[dev-dependencies]
test-support={path="../test-support"}
[features]
normal-leak=["wire/execution"]
build-leak=["compiler/execution"]
dev-leak=["test-support/execution"]
''',
        "wire": '''[dependencies]
proof={path="../proof", default-features=false}
engine={path="../engine", optional=true}
[dev-dependencies]
nonroot-dev-only={path="../nonroot-dev-only"}
[features]
execution=["dep:engine"]
''',
        "compiler": '''[build-dependencies]
engine={path="../engine", optional=true}
[features]
execution=["dep:engine"]
''',
        "test-support": '''[dependencies]
engine={path="../engine", optional=true}
[dev-dependencies]
nonroot-dev-only={path="../nonroot-dev-only"}
[features]
execution=["dep:engine"]
''',
        "daemon": '''[dependencies]
proof={path="../proof", features=["full"]}
[dev-dependencies]
workspace-dev-only={path="../workspace-dev-only"}
''',
        "proof": '[features]\nfull=[]\n',
        "engine": "", "nonroot-dev-only": "", "workspace-dev-only": "",
    }
    for name, deps in dependencies.items():
        _write_package(tmp_path, f"crates/{name}", f'[package]\nname="{name}"\nversion="1.0.0"\n{deps}')
        source = tmp_path / "crates" / name / "src"
        source.mkdir()
        (source / "lib.rs").write_text("//! Root test dependency boundary fixture.\n")
    manifest = tmp_path / "Cargo.toml"
    subprocess.run(["cargo", "generate-lockfile", "--offline", "--manifest-path", str(manifest)], check=True, capture_output=True)
    policy = _boundary_policy()
    policy["layers"]["wire"].append("test-support")
    policy["layers"]["node"].extend(["nonroot-dev-only", "workspace-dev-only"])
    selection = policy["configurations"]["sdk"]

    def inspect() -> dict:
        tree = subprocess.check_output(MODULE.boundary_tree_command(manifest, selection, offline=True), text=True)
        return MODULE.evaluate_boundary_tree(policy, selection, tree)

    shipping = inspect()
    assert shipping["within_boundary"]
    assert "test-support" not in shipping["packages"]
    selection["include_root_dev_dependencies"] = True
    baseline = inspect()
    assert baseline["within_boundary"]
    assert set(baseline["packages"]) == {"sdk", "wire", "proof", "compiler", "test-support"}
    assert "nonroot-dev-only" not in baseline["packages"]
    assert "workspace-dev-only" not in baseline["packages"]
    for feature, owner in [
        ("normal-leak", "wire"), ("build-leak", "compiler"), ("dev-leak", "test-support"),
    ]:
        selection["features"] = [feature]
        report = inspect()
        assert not report["within_boundary"]
        assert report["violations"] == [{
            "package": "engine", "forbidden_layer": "node", "path": ["sdk", owner, "engine"],
        }]
