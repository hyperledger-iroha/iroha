"""Unit tests for the reproducible Cargo build profiler."""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import sys
from pathlib import Path

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "profile_cargo_build.py"
SPEC = importlib.util.spec_from_file_location("profile_cargo_build", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_normalized_cargo_args_adds_reproducible_defaults() -> None:
    """The profiler pins the lock, message stream, timings, and job count."""
    assert MODULE.normalized_cargo_args(["build", "--workspace"], 1) == [
        "build",
        "--locked",
        "--workspace",
        "--message-format",
        "json-render-diagnostics",
        "--timings",
        "--jobs",
        "1",
    ]


def test_normalized_cargo_args_preserves_explicit_controls() -> None:
    """Caller-supplied Cargo controls are not duplicated or replaced."""
    assert MODULE.normalized_cargo_args(
        [
            "--",
            "check",
            "--locked",
            "--message-format=json",
            "--timings=html",
            "-j2",
            "-p",
            "iroha_core",
        ],
        1,
    ) == [
        "check",
        "--locked",
        "--message-format=json",
        "--timings=html",
        "-j2",
        "-p",
        "iroha_core",
    ]


def test_normalized_cargo_args_precedes_test_harness_separator() -> None:
    """Profiler controls never leak into arguments consumed by a test binary."""
    assert MODULE.normalized_cargo_args(
        ["test", "-p", "iroha_core", "--", "--nocapture"],
        1,
    ) == [
        "test",
        "--locked",
        "-p",
        "iroha_core",
        "--message-format",
        "json-render-diagnostics",
        "--timings",
        "--jobs",
        "1",
        "--",
        "--nocapture",
    ]


def test_normalized_cargo_args_ignores_test_harness_locked_flag() -> None:
    """A test-binary flag cannot masquerade as Cargo's lockfile control."""
    assert MODULE.normalized_cargo_args(
        ["test", "-p", "iroha_core", "--", "--locked"],
        2,
    ) == [
        "test",
        "--locked",
        "-p",
        "iroha_core",
        "--message-format",
        "json-render-diagnostics",
        "--timings",
        "--jobs",
        "2",
        "--",
        "--locked",
    ]


def test_validate_paths_requires_external_outputs(tmp_path: Path) -> None:
    """Build products and reports cannot perturb the measured source tree."""
    root = tmp_path / "repo"
    root.mkdir()
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    external = tmp_path / "external"
    MODULE.validate_paths(root, external / "target", external / "report.json", False)
    with pytest.raises(ValueError, match="target-dir must be outside"):
        MODULE.validate_paths(
            root,
            root / "target-profile",
            external / "report.json",
            False,
        )
    with pytest.raises(ValueError, match="out must be outside"):
        MODULE.validate_paths(
            root,
            external / "target",
            root / "profile.json",
            False,
        )


def test_validate_paths_requires_explicit_warm_mode(tmp_path: Path) -> None:
    """An accidental warm cache cannot masquerade as a cold profile."""
    root = tmp_path / "repo"
    root.mkdir()
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    (target / "cached").write_text("present", encoding="utf-8")
    with pytest.raises(ValueError, match="non-empty"):
        MODULE.validate_paths(root, target, tmp_path / "report.json", False)
    MODULE.validate_paths(root, target, tmp_path / "report.json", True)


def test_source_fingerprint_is_order_independent_and_content_bound(tmp_path: Path) -> None:
    """The source identity binds path, mode, and content in sorted path order."""
    (tmp_path / "a.rs").write_text("fn a() {}\n", encoding="utf-8")
    (tmp_path / "b.rs").write_text("fn b() {}\n", encoding="utf-8")
    first = MODULE.source_fingerprint(tmp_path, ["a.rs", "b.rs"])
    assert first == MODULE.source_fingerprint(tmp_path, ["b.rs", "a.rs"])
    assert first.files == 2
    assert first.deleted == 0
    (tmp_path / "b.rs").write_text("fn changed() {}\n", encoding="utf-8")
    assert MODULE.source_fingerprint(tmp_path, ["a.rs", "b.rs"]).sha256 != first.sha256


def test_source_fingerprint_binds_tracked_deletions(tmp_path: Path) -> None:
    """A dirty tracked deletion is an input state, not a profiling race."""
    (tmp_path / "present.rs").write_text("fn present() {}\n", encoding="utf-8")
    with_deleted = MODULE.source_fingerprint(
        tmp_path,
        ["deleted.rs", "present.rs"],
    )
    without_deleted = MODULE.source_fingerprint(tmp_path, ["present.rs"])
    assert with_deleted.files == 1
    assert with_deleted.deleted == 1
    assert with_deleted.sha256 != without_deleted.sha256


def test_parse_cargo_messages_has_stable_unit_inventory() -> None:
    """Absolute artifact paths and message order do not affect unit identity."""
    artifact_a = {
        "reason": "compiler-artifact",
        "package_id": "path+file:///repo/crates/a#a@0.1.0",
        "target": {"name": "a", "kind": ["lib"], "crate_types": ["lib"]},
        "profile": {
            "opt_level": "0",
            "debuginfo": 2,
            "debug_assertions": True,
            "test": False,
        },
        "features": ["z", "a"],
        "filenames": ["/one/target/debug/liba.rlib"],
        "fresh": False,
    }
    artifact_b = {
        **artifact_a,
        "package_id": "registry+https://example.invalid#index#b@1.0.0",
        "target": {"name": "b", "kind": ["proc-macro"], "crate_types": ["proc-macro"]},
        "features": [],
        "filenames": ["/two/target/debug/libb.so"],
        "fresh": True,
    }
    lines = [
        "not json\n",
        MODULE.canonical_json_bytes(artifact_b).decode() + "\n",
        MODULE.canonical_json_bytes(artifact_a).decode() + "\n",
    ]
    units, fresh, compiled = MODULE.parse_cargo_messages(lines)
    assert [unit["name"] for unit in units] == ["a", "b"]
    assert units[0]["package_id"] == "workspace#a@0.1.0"
    assert units[0]["features"] == ["a", "z"]
    assert fresh == 1
    assert compiled == 1
    assert all("filenames" not in unit for unit in units)


@pytest.mark.parametrize(
    "field,replacement",
    (
        ("source", {"bytes": 2, "deleted": 0, "files": 1, "sha256": "bb"}),
        ("git_revision", "new-revision"),
        ("cargo_lock_sha256", "new-lock"),
        ("toolchain", {"cargo": "cargo changed", "rustc": "rustc changed"}),
        ("selected_env", {"RUSTFLAGS": "-Ctarget-cpu=generic"}),
    ),
)
def test_changed_input_fields_detects_profile_input_drift(
    field: str, replacement: object
) -> None:
    """Every mutable source, revision, lock, and toolchain input is rechecked."""
    before = {
        "cargo_args": ["build", "--locked"],
        "cargo_lock_sha256": "old-lock",
        "git_revision": "old-revision",
        "jobs": 1,
        "label": "test",
        "profile_mode": "cold",
        "selected_env": {},
        "source": {"bytes": 1, "deleted": 0, "files": 1, "sha256": "aa"},
        "toolchain": {"cargo": "cargo stable", "rustc": "rustc stable"},
    }
    after = copy.deepcopy(before)
    after[field] = replacement

    assert MODULE.changed_input_fields(before, after) == [field]
    assert MODULE.changed_input_fields(before, copy.deepcopy(before)) == []


def test_main_invalidates_successful_build_when_source_changes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A concurrent source edit makes an otherwise successful report unusable."""
    root = tmp_path / "repo"
    root.mkdir()
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
    source = root / "source.rs"
    source.write_text("fn before() {}\n", encoding="utf-8")
    external = tmp_path / "profile"
    report_path = external / "report.json"

    monkeypatch.setattr(
        MODULE,
        "tracked_and_untracked_paths",
        lambda _root: ["Cargo.lock", "Cargo.toml", "source.rs"],
    )

    def identity(command: list[str], _cwd: Path) -> str:
        return {
            ("git", "rev-parse", "HEAD"): "revision",
            ("cargo", "-Vv"): "cargo stable",
            ("rustc", "-Vv"): "rustc stable",
        }[tuple(command)]

    monkeypatch.setattr(MODULE, "command_output", identity)
    monkeypatch.setattr(MODULE.platform, "platform", lambda: "test-platform")

    class Process:
        stdout = io.StringIO("")

        @staticmethod
        def wait() -> int:
            source.write_text("fn after() {}\n", encoding="utf-8")
            return 0

    monkeypatch.setattr(
        MODULE.subprocess,
        "Popen",
        lambda *_args, **_kwargs: Process(),
    )

    returncode = MODULE.main(
        [
            "--root",
            str(root),
            "--target-dir",
            str(external / "target"),
            "--out",
            str(report_path),
            "--",
            "build",
        ]
    )

    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert returncode == MODULE.INPUT_DRIFT_EXIT_CODE
    assert report["schema_version"] == 2
    assert report["valid"] is False
    assert report["result"]["returncode"] == 0
    assert report["input_validation"]["stable"] is False
    assert report["input_validation"]["changed_fields"] == ["source"]
    assert report["input_sha256"] != report["input_validation"]["post_input_sha256"]
