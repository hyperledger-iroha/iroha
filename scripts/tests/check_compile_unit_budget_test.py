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


def _args(**overrides: object) -> MODULE.argparse.Namespace:
    values: dict[str, object] = {
        "manifest_path": Path("Cargo.toml"),
        "target_dir": None,
        "allow_lock_update": False,
        "workspace": False,
        "package": ["iroha_data_model"],
        "lib": True,
        "artifact_scope": "workspace",
        "budget_percent": 2.0,
        "budget_min_growth": 3,
    }
    values.update(overrides)
    return MODULE.argparse.Namespace(**values)


def _contract(**overrides: object) -> dict[str, object]:
    contract = MODULE.measurement_contract(_args(), toolchain="1.93.1")
    contract.update(overrides)
    return contract


def _artifact_message(
    package_id: str,
    name: str,
    *,
    features: list[str] | None = None,
    profile: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "reason": "compiler-artifact",
        "package_id": package_id,
        "target": {
            "name": name,
            "kind": ["lib"],
            "crate_types": ["lib"],
            "src_path": f"/src/{name}.rs",
        },
        "features": features or [],
        "profile": profile or {"opt_level": "0", "test": False},
    }


def _artifact(package_id: str, name: str) -> MODULE.ArtifactIdentity:
    identity = MODULE.artifact_identity(_artifact_message(package_id, name))
    assert identity is not None
    return identity


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
    root.write_text(
        json.dumps(
            {
                "schema_version": MODULE.REPORT_SCHEMA_VERSION,
                **_contract(),
                "compile_units": 41,
            }
        ),
        encoding="utf-8",
    )
    assert MODULE.load_baseline(root, None)["compile_units"] == 41

    keyed = tmp_path / "keyed.json"
    keyed.write_text(
        json.dumps(
            {
                "schema_version": MODULE.REPORT_SCHEMA_VERSION,
                "foundation": {**_contract(), "compile_units": 17},
            }
        ),
        encoding="utf-8",
    )
    assert MODULE.load_baseline(keyed, "foundation")["compile_units"] == 17


def test_load_baseline_rejects_invalid_values(tmp_path: Path) -> None:
    path = tmp_path / "invalid.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": MODULE.REPORT_SCHEMA_VERSION,
                **_contract(),
                "compile_units": True,
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError):
        MODULE.load_baseline(path, None)


def test_load_baseline_rejects_legacy_identity_schema(tmp_path: Path) -> None:
    path = tmp_path / "legacy.json"
    path.write_text(
        '{"schema_version": 1, "compile_units": 41}\n', encoding="utf-8"
    )
    with pytest.raises(ValueError, match="schema_version 2"):
        MODULE.load_baseline(path, None)


def test_artifact_identity_distinguishes_features_and_profiles() -> None:
    base = _artifact_message(
        "pkg-a",
        "alpha",
        features=["std", "json"],
        profile={"test": False, "opt_level": "0"},
    )
    reordered = _artifact_message(
        "pkg-a",
        "alpha",
        features=["json", "std"],
        profile={"opt_level": "0", "test": False},
    )
    feature_variant = _artifact_message(
        "pkg-a",
        "alpha",
        features=["std"],
        profile={"test": False, "opt_level": "0"},
    )
    profile_variant = _artifact_message(
        "pkg-a",
        "alpha",
        features=["std", "json"],
        profile={"test": True, "opt_level": "0"},
    )

    assert MODULE.artifact_identity(base) == MODULE.artifact_identity(reordered)
    assert MODULE.artifact_identity(base) != MODULE.artifact_identity(
        feature_variant
    )
    assert MODULE.artifact_identity(base) != MODULE.artifact_identity(
        profile_variant
    )


def test_artifact_identity_rejects_incomplete_messages() -> None:
    message = _artifact_message("pkg-a", "alpha")
    del message["profile"]
    assert MODULE.artifact_identity(message) is None


def test_measurement_contract_rejects_toolchain_and_scope_drift() -> None:
    baseline = _contract()
    MODULE.validate_baseline_contract(baseline, _contract())

    with pytest.raises(ValueError, match="toolchain"):
        MODULE.validate_baseline_contract(baseline, _contract(toolchain="nightly"))
    with pytest.raises(ValueError, match="artifact_scope"):
        MODULE.validate_baseline_contract(
            baseline, _contract(artifact_scope="all")
        )
    incomplete = dict(baseline)
    del incomplete["artifact_identity"]
    with pytest.raises(ValueError, match="artifact_identity"):
        MODULE.validate_baseline_contract(incomplete, _contract())


@pytest.mark.parametrize(
    ("output", "expected"),
    [
        (
            "rustc 1.93.1 (01f7b7c28 2026-02-11)\n"
            "binary: rustc\ncommit-hash: 01f7b7c28\nrelease: 1.93.1\n",
            "1.93.1",
        ),
        ("rustc 1.94.0-nightly (abcdef 2026-03-01)\n", "1.94.0-nightly"),
    ],
)
def test_parse_rustc_release_requires_an_exact_version(
    output: str, expected: str
) -> None:
    assert MODULE.parse_rustc_release(output) == expected


def test_parse_rustc_release_rejects_unversioned_output() -> None:
    with pytest.raises(ValueError, match="exact release"):
        MODULE.parse_rustc_release("rustc development\n")


def test_rustc_release_honors_the_compiler_selected_for_cargo(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    observed: list[list[str]] = []

    def fake_check_output(command: list[str], *, text: bool) -> str:
        observed.append(command)
        assert text is True
        return "rustc 1.93.1 (01f7b7c28 2026-02-11)\n"

    monkeypatch.setenv("RUSTC", "/opt/reviewed/bin/rustc")
    monkeypatch.setattr(MODULE.subprocess, "check_output", fake_check_output)

    assert MODULE.rustc_release() == "1.93.1"
    assert observed == [["/opt/reviewed/bin/rustc", "--version", "--verbose"]]


def test_main_rejects_mismatched_baseline_before_cargo(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    baseline = tmp_path / "baseline.json"
    baseline.write_text(
        json.dumps(
            {
                "schema_version": MODULE.REPORT_SCHEMA_VERSION,
                "iroha_data_model_lib": {
                    **_contract(toolchain="1.92.0"),
                    "compile_units": 41,
                },
            }
        ),
        encoding="utf-8",
    )
    monkeypatch.setattr(MODULE, "rustc_release", lambda: "1.93.1")
    monkeypatch.setattr(
        MODULE,
        "cargo_metadata",
        lambda _args: pytest.fail("Cargo must not run for an incomparable baseline"),
    )
    monkeypatch.setattr(
        MODULE.sys,
        "argv",
        [
            "check_compile_unit_budget.py",
            "--locked",
            "--lib",
            "-p",
            "iroha_data_model",
            "--artifact-scope",
            "workspace",
            "--baseline",
            str(baseline),
            "--baseline-key",
            "iroha_data_model_lib",
            "--budget-percent",
            "2",
            "--budget-min-growth",
            "3",
        ],
    )

    assert MODULE.main() == 2
    assert "`toolchain` is '1.92.0', expected '1.93.1'" in capsys.readouterr().err


def test_report_and_json_output_are_deterministic(tmp_path: Path) -> None:
    artifacts = {
        _artifact("pkg-a", "alpha"),
        _artifact("pkg-b", "beta"),
    }
    report = MODULE.build_report(
        command=["cargo", "test"],
        artifacts=artifacts,
        artifact_package_ids={"pkg-a", "pkg-b"},
        source_counts=Counter({"path": 1, "registry": 1}),
        package_artifacts=Counter({"beta": 1, "alpha": 1}),
        baseline=1,
        limit=4,
        contract=_contract(),
    )

    assert report["compile_units"] == 2
    assert report["schema_version"] == 2
    assert report["artifact_identity"] == MODULE.ARTIFACT_IDENTITY
    assert report["toolchain"] == "1.93.1"
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
        "artifact_identity": MODULE.ARTIFACT_IDENTITY,
        "toolchain": "1.93.1",
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
        f"artifact_identity={MODULE.ARTIFACT_IDENTITY}\n"
        "toolchain=1.93.1\n"
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
        artifacts={
            _artifact("pkg-z", "zeta"),
            _artifact("pkg-a", "alpha"),
        },
        artifact_package_ids={"pkg-z", "pkg-a"},
        source_counts=Counter({"path": 2}),
        package_artifacts=Counter({"zeta": 1, "alpha": 1}),
        baseline=None,
        limit=None,
        contract=_contract(),
    )

    assert [entry["name"] for entry in report["top_packages"]] == [
        "alpha",
        "zeta",
    ]


def test_focused_command_is_locked_and_library_only() -> None:
    args = _args()

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
