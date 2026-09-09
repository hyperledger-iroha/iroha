"""Regression tests for the local Cargo acceleration wrapper."""

from __future__ import annotations

import json
import os
import shutil
import sys
import re
import subprocess
import textwrap
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "cargo_fast.sh"
CONTROLLED_ENV_VARS = (
    "CI",
    "CARGO_BUILD_JOBS",
    "CARGO_BUILD_TARGET",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_BUILD_BUILD_DIR",
    "CARGO_HOME",
    "CARGO_FAST_TEST_METADATA",
    "CARGO_FAST_RESOLUTION_CAPTURE",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_FAST_TARGET_ROOT",
    "CARGO_FAST_TEST_WORKSPACE_MANIFEST",
    "CARGO_INCREMENTAL",
    "CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_UNITS",
    "CARGO_PROFILE_BENCH_CODEGEN_UNITS",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS",
    "CARGO_PROFILE_DEV_CODEGEN_UNITS",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
    "CARGO_PROFILE_TEST_DEBUG",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_UNITS",
    "CARGO_PROFILE_TEST_CODEGEN_UNITS",
    "CARGO_TARGET_DIR",
    "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
    "CMAKE_BUILD_PARALLEL_LEVEL",
    "GITHUB_ACTIONS",
    "IROHA_GIT_COMMIT_HASH",
    "RUST_TEST_THREADS",
    "RUSTC_WRAPPER",
    "RUSTFLAGS",
    "SCCACHE_DIR",
    "VERGEN_GIT_SHA",
)

@pytest.fixture(autouse=True)
def hermetic_wrapper_checkout(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    checkout = tmp_path / "checkout"
    scripts = checkout / "scripts"
    scripts.mkdir(parents=True)
    for name in ("cargo_fast.sh", "check_cargo_target_owner.py"):
        shutil.copy2(REPO_ROOT / "scripts" / name, scripts / name)
    (checkout / "Cargo.toml").write_text("[workspace]\nmembers = []\n", encoding="utf-8")
    monkeypatch.setattr(sys.modules[__name__], "REPO_ROOT", checkout)
    monkeypatch.setattr(sys.modules[__name__], "SCRIPT", scripts / "cargo_fast.sh")


INHERITED_SINGLE_WORKER_FINGERPRINT = {
    "CARGO_BUILD_JOBS": "1",
    "CARGO_INCREMENTAL": "0",
    "CARGO_PROFILE_DEV_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_TEST_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_BENCH_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_UNITS": "1",
    "CMAKE_BUILD_PARALLEL_LEVEL": "1",
}


def _write_executable(path: Path, source: str) -> None:
    path.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
    path.chmod(0o755)


def _run_wrapper(
    tmp_path: Path,
    *arguments: str,
    extra_env: dict[str, str] | None = None,
    binaries: dict[str, str] | None = None,
) -> tuple[subprocess.CompletedProcess[str], dict[str, str], list[str]]:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir(exist_ok=True)
    capture = tmp_path / "cargo-capture.txt"
    _write_executable(
        fake_bin / "cargo",
        r"""
        #!/usr/bin/env python3
        import json, os, pathlib, sys
        args = sys.argv[1:]
        if "locate-project" in args:
            manifest = args[args.index("--manifest-path") + 1]
            print(os.environ.get("CARGO_FAST_TEST_WORKSPACE_MANIFEST", manifest))
        elif "metadata" in args:
            if os.environ.get("CARGO_FAST_RESOLUTION_CAPTURE"):
                pathlib.Path(os.environ["CARGO_FAST_RESOLUTION_CAPTURE"]).write_text(json.dumps(args))
            manifest = args[args.index("--manifest-path") + 1]
            root = pathlib.Path(os.environ.get("CARGO_FAST_TEST_WORKSPACE_MANIFEST", manifest)).parent
            target = os.environ.get("CARGO_TARGET_DIR", os.environ.get("CARGO_BUILD_TARGET_DIR", str(root / "target")))
            build = os.environ.get("CARGO_BUILD_BUILD_DIR")
            for index, argument in enumerate(args):
                if argument != "--config":
                    continue
                value = args[index + 1]
                if value.startswith("build.target-dir="):
                    target = json.loads(value.split("=", 1)[1])
                elif value.startswith("build.build-dir="):
                    build = json.loads(value.split("=", 1)[1])
            target = str(pathlib.Path(target).absolute())
            build = str(pathlib.Path(build).absolute()) if build else target
            print(os.environ.get("CARGO_FAST_TEST_METADATA", json.dumps({"version": 1, "workspace_root": str(root), "target_directory": target, "build_directory": build})))
        else:
            with open(os.environ["CARGO_FAST_CAPTURE"], "w") as stream:
                for name, value in os.environ.items():
                    print(name + "=" + value, file=stream)
                print("__CARGO_FAST_ARGS__", file=stream)
                print("\n".join(args), file=stream)
        """,
    )
    for name, source in (binaries or {}).items():
        _write_executable(fake_bin / name, source)

    environment = os.environ.copy()
    for name in CONTROLLED_ENV_VARS:
        environment.pop(name, None)
    environment.update(
        {
            "CARGO_FAST_CAPTURE": str(capture),
            "HOME": str(tmp_path / "home"),
            "PATH": os.pathsep.join((str(fake_bin), "/usr/bin", "/bin")),
        }
    )
    if extra_env:
        environment.update(extra_env)

    result = subprocess.run(
        ["/bin/bash", str(SCRIPT), *arguments],
        cwd=REPO_ROOT,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
    )

    if not capture.exists():
        return result, {}, []
    lines = capture.read_text(encoding="utf-8").splitlines()
    marker = lines.index("__CARGO_FAST_ARGS__")
    cargo_environment = dict(
        line.split("=", 1) for line in lines[:marker] if "=" in line
    )
    return result, cargo_environment, lines[marker + 1 :]


def test_default_preserves_cargo_arguments_and_profile_defaults(tmp_path: Path) -> None:
    result, environment, cargo_arguments = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--",
        "check",
        "-p",
        "iroha core",
        "--all-targets",
    )

    assert result.returncode == 0, result.stderr
    assert cargo_arguments == ["check", "-p", "iroha core", "--all-targets"]
    assert "CARGO_TARGET_DIR" not in environment
    assert "CARGO_BUILD_JOBS" not in environment
    assert "CARGO_INCREMENTAL" not in environment
    assert "VERGEN_GIT_SHA" not in environment
    assert "IROHA_GIT_COMMIT_HASH" not in environment
    assert "RUSTFLAGS" not in environment
    assert "CARGO_TARGET_DIR=workspace-default" in result.stdout
    assert "CARGO_BUILD_JOBS=cargo-default" in result.stdout
    assert "linker=system-default" in result.stdout


def test_cargo_replaces_wrapper_process_and_preserves_its_exit(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    pid_file = tmp_path / "cargo.pid"
    metadata = json.dumps({"version": 1, "workspace_root": str(REPO_ROOT),
                           "target_directory": str(REPO_ROOT / "target"),
                           "build_directory": str(REPO_ROOT / "target")})
    _write_executable(fake_bin / "cargo", f"""#!/bin/sh
if [ "$1" = metadata ]; then printf '%s\\n' '{metadata}'; exit 0; fi
if [ "$1" = locate-project ]; then printf '%s\\n' '{REPO_ROOT / "Cargo.toml"}'; exit 0; fi
printf '%s\\n' "$$" > "$PID_FILE"
exit 37
""")
    environment = os.environ.copy()
    for name in CONTROLLED_ENV_VARS:
        environment.pop(name, None)
    environment.update({"PATH": f"{fake_bin}:/usr/bin:/bin", "PID_FILE": str(pid_file)})
    process = subprocess.Popen(
        ["/bin/bash", str(SCRIPT), "--no-sccache", "--", "check"],
        cwd=REPO_ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    _, stderr = process.communicate(timeout=30)
    assert process.returncode == 37, stderr
    assert int(pid_file.read_text(encoding="utf-8").strip()) == process.pid


def test_default_clears_exact_local_inherited_single_worker_fingerprint(
    tmp_path: Path,
) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--",
        "check",
        extra_env={**INHERITED_SINGLE_WORKER_FINGERPRINT, "RUST_TEST_THREADS": "1"},
    )

    assert result.returncode == 0, result.stderr
    for name in INHERITED_SINGLE_WORKER_FINGERPRINT:
        assert name not in environment
    assert environment["RUST_TEST_THREADS"] == "1"
    assert "cleared inherited local single-worker build limits" in result.stdout
    assert "CARGO_BUILD_JOBS=cargo-default" in result.stdout


def test_preserve_build_limits_keeps_exact_inherited_fingerprint(
    tmp_path: Path,
) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--preserve-build-limits",
        "--",
        "check",
        extra_env=INHERITED_SINGLE_WORKER_FINGERPRINT,
    )

    assert result.returncode == 0, result.stderr
    for name, value in INHERITED_SINGLE_WORKER_FINGERPRINT.items():
        assert environment[name] == value
    assert "cleared inherited local single-worker build limits" not in result.stdout
    assert "CARGO_BUILD_JOBS=1" in result.stdout


def test_partial_inherited_build_limits_are_preserved(tmp_path: Path) -> None:
    partial_fingerprint = dict(INHERITED_SINGLE_WORKER_FINGERPRINT)
    partial_fingerprint.pop("CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_UNITS")
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--",
        "check",
        extra_env=partial_fingerprint,
    )

    assert result.returncode == 0, result.stderr
    for name, value in partial_fingerprint.items():
        assert environment[name] == value
    assert "cleared inherited local single-worker build limits" not in result.stdout


def test_ci_keeps_exact_inherited_build_limits(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--",
        "check",
        extra_env={**INHERITED_SINGLE_WORKER_FINGERPRINT, "CI": "true"},
    )

    assert result.returncode == 0, result.stderr
    for name, value in INHERITED_SINGLE_WORKER_FINGERPRINT.items():
        assert environment[name] == value
    assert environment["CI"] == "true"
    assert "cleared inherited local single-worker build limits" not in result.stdout


def test_explicit_wrapper_limits_replace_the_cleared_inherited_values(
    tmp_path: Path,
) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--jobs",
        "7",
        "--incremental",
        "--",
        "check",
        extra_env=INHERITED_SINGLE_WORKER_FINGERPRINT,
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_BUILD_JOBS"] == "7"
    assert environment["CARGO_INCREMENTAL"] == "1"
    for name in INHERITED_SINGLE_WORKER_FINGERPRINT:
        if name not in {"CARGO_BUILD_JOBS", "CARGO_INCREMENTAL"}:
            assert name not in environment
    assert "cleared inherited local single-worker build limits" in result.stdout


@pytest.mark.parametrize(
    ("flag", "expected"),
    (("--incremental", "1"), ("--no-incremental", "0")),
)
def test_incremental_mode_is_explicit(
    tmp_path: Path, flag: str, expected: str
) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", flag, "--", "test", "-p", "iroha_core"
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_INCREMENTAL"] == expected
    assert f"CARGO_INCREMENTAL={expected}" in result.stdout


def test_incremental_modes_are_mutually_exclusive(tmp_path: Path) -> None:
    result, _, _ = _run_wrapper(
        tmp_path,
        "--incremental",
        "--no-incremental",
        "--",
        "test",
    )

    assert result.returncode != 0
    assert "cannot be used together" in result.stderr


def test_target_slot_is_stable_and_repository_local(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--target-slot",
        "core-tests_1",
        "--",
        "test",
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_TARGET_DIR"] == str(
        REPO_ROOT / "target" / "cargo-fast" / "core-tests_1"
    )


@pytest.mark.parametrize("slot", ("", ".", "..", "../escape", "a/b", "two words"))
def test_target_slot_rejects_unsafe_names(tmp_path: Path, slot: str) -> None:
    result, _, _ = _run_wrapper(
        tmp_path, "--target-slot", slot, "--", "check"
    )

    assert result.returncode != 0
    assert "--target-slot must contain only" in result.stderr


def test_target_slot_and_target_dir_are_mutually_exclusive(tmp_path: Path) -> None:
    result, _, _ = _run_wrapper(
        tmp_path,
        "--target-slot",
        "core",
        "--target-dir",
        str(tmp_path / "target"),
        "--",
        "check",
    )

    assert result.returncode != 0
    assert "cannot be used together" in result.stderr


def test_target_slot_root_can_be_persisted_outside_the_checkout(tmp_path: Path) -> None:
    target_root = tmp_path / "persistent-targets"
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--target-slot",
        "routine",
        "--",
        "check",
        extra_env={"CARGO_FAST_TARGET_ROOT": str(target_root)},
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_TARGET_DIR"] == str(target_root / "routine")


@pytest.mark.parametrize("equal_form", (False, True))
def test_foreign_manifest_cannot_reuse_wrapper_target(tmp_path: Path, equal_form: bool) -> None:
    frozen = tmp_path / "frozen"
    frozen.mkdir()
    manifest = frozen / "Cargo.toml"
    manifest.write_text("[workspace]\nmembers = []\n", encoding="utf-8")
    flags = [f"--manifest-path={manifest}"] if equal_form else ["--manifest-path", str(manifest)]
    result, captured, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--print-env", "--", "check", *flags,
        "--target-dir", "target",
    )
    assert result.returncode != 0
    assert "belongs to another source tree" in result.stderr
    assert not captured


@pytest.mark.parametrize("flags", (("-C", "elsewhere"), ("-Celsewhere",)))
def test_cargo_directory_override_requires_selected_wrapper(tmp_path: Path, flags: tuple[str, ...]) -> None:
    result, captured, _ = _run_wrapper(tmp_path, "--no-sccache", "--", *flags, "check")
    assert result.returncode != 0
    assert "does not accept Cargo -C" in result.stderr
    assert not captured


@pytest.mark.parametrize("selection", ("environment", "wrapper", "cargo", "cargo-equals"))
def test_rejects_target_in_another_source_tree_before_cargo(
    tmp_path: Path, selection: str
) -> None:
    foreign = tmp_path / "other-checkout"
    foreign.mkdir()
    (foreign / "Cargo.toml").write_text("[workspace]\nmembers = []\n", encoding="utf-8")
    target = str(foreign / "target")
    arguments = ["--no-sccache"]
    environment = {}
    if selection == "environment":
        environment["CARGO_TARGET_DIR"] = target
    elif selection == "wrapper":
        arguments.extend(("--target-dir", target))
    arguments.extend(("--", "check"))
    if selection == "cargo":
        arguments.extend(("--target-dir", target))
    elif selection == "cargo-equals":
        arguments.append(f"--target-dir={target}")
    result, captured, _ = _run_wrapper(tmp_path, *arguments, extra_env=environment)
    assert result.returncode != 0
    assert "belongs to another source tree" in result.stderr
    assert not captured
    assert not Path(target).exists()  # This check never creates or cleans targets.


def test_forwarded_target_overrides_inherited_target_for_owner_check(tmp_path: Path) -> None:
    foreign = tmp_path / "other-checkout"
    foreign.mkdir()
    (foreign / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    result, _, arguments = _run_wrapper(
        tmp_path, "--no-sccache", "--", "check", "--target-dir", "target",
        extra_env={"CARGO_TARGET_DIR": str(foreign / "target")},
    )
    assert result.returncode == 0, result.stderr
    assert arguments == ["check", "--target-dir", "target"]


def test_program_target_argument_is_not_a_cargo_override(tmp_path: Path) -> None:
    foreign = tmp_path / "other-checkout"
    foreign.mkdir()
    (foreign / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    result, _, arguments = _run_wrapper(
        tmp_path, "--no-sccache", "--", "test", "--", "--target-dir", str(foreign),
    )
    assert result.returncode == 0, result.stderr
    assert arguments == ["test", "--", "--target-dir", str(foreign)]


def test_frozen_source_rejects_ancestor_target_and_symlink(tmp_path: Path) -> None:
    ancestor = tmp_path / "checkout"
    frozen = ancestor / "snapshots" / "frozen"
    frozen.mkdir(parents=True)
    (ancestor / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (frozen / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    target = ancestor / "target"
    target.mkdir()
    alias = frozen / "target-alias"
    alias.symlink_to(target, target_is_directory=True)
    sentinel = target / "retained-artifact"
    sentinel.write_bytes(b"unchanged warm artifact")
    for selected in (target, alias):
        result = subprocess.run(
            ["/usr/bin/env", "python3", str(REPO_ROOT / "scripts/check_cargo_target_owner.py"),
             "--source-root", str(frozen), "--target-dir", str(selected)],
            check=False, capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "belongs to another source tree" in result.stderr
        assert sentinel.read_bytes() == b"unchanged warm artifact"


def test_nested_workspace_is_distinct_but_member_target_has_same_owner(tmp_path: Path) -> None:
    source = tmp_path / "source"
    member = source / "member"
    nested = source / "snapshots" / "frozen"
    member.mkdir(parents=True)
    nested.mkdir(parents=True)
    (source / "Cargo.toml").write_text('[workspace]\nmembers = ["member"]\n', encoding="utf-8")
    (member / "Cargo.toml").write_text(
        '[package]\nname = "owner-test-member"\nversion = "0.1.0"\n[lib]\npath = "lib.rs"\n',
        encoding="utf-8",
    )
    (member / "lib.rs").write_text("", encoding="utf-8")
    (nested / "Cargo.toml").write_text("[workspace]\nmembers = []\n", encoding="utf-8")
    for target, expected in ((nested / "target", 1), (member / "target", 0), (source / "-cache", 0)):
        result = subprocess.run(
            ["/usr/bin/env", "python3", str(REPO_ROOT / "scripts/check_cargo_target_owner.py"),
             "--source-root", str(source), f"--target-dir={target}"],
            check=False, capture_output=True, text=True,
        )
        assert result.returncode == expected, result.stderr
        assert not target.exists()


def test_symlinked_manifest_preserves_cargos_distinct_source_directory(tmp_path: Path) -> None:
    original = tmp_path / "original"
    alias = tmp_path / "alias"
    original.mkdir()
    alias.mkdir()
    (original / "Cargo.toml").write_text(
        '[package]\nname = "manifest-link-owner"\nversion = "0.1.0"\n[lib]\npath = "lib.rs"\n',
        encoding="utf-8",
    )
    (original / "lib.rs").write_text("pub const VALUE: u8 = 1;\n", encoding="utf-8")
    (alias / "lib.rs").write_text("pub const VALUE: u8 = 2;\n", encoding="utf-8")
    (alias / "Cargo.toml").symlink_to(original / "Cargo.toml")
    result = subprocess.run(
        ["/usr/bin/env", "python3", str(REPO_ROOT / "scripts/check_cargo_target_owner.py"),
         "--source-root", str(original), "--manifest-path", str(alias / "Cargo.toml"),
         "--target-dir", str(original / "target")],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode != 0
    assert "belongs to another source tree" in result.stderr
    assert str(alias) in result.stderr
    assert not (original / "target").exists()


def test_jobs_override_is_forwarded_to_cargo(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--jobs", "6", "--", "build"
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_BUILD_JOBS"] == "6"
    assert "CARGO_BUILD_JOBS=6" in result.stdout
    assert "serializes compilation" not in result.stderr


def test_single_job_override_warns_about_serial_compilation(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--jobs", "1", "--", "build"
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_BUILD_JOBS"] == "1"
    assert "one Cargo job serializes compilation" in result.stderr


@pytest.mark.parametrize(
    "cargo_jobs",
    (("-j1",), ("-j", "1"), ("--jobs=1",), ("--jobs", "1")),
)
def test_forwarded_single_job_argument_warns_about_serial_compilation(
    tmp_path: Path, cargo_jobs: tuple[str, ...]
) -> None:
    result, _, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--", "build", *cargo_jobs
    )

    assert result.returncode == 0, result.stderr
    assert "one Cargo job serializes compilation" in result.stderr


def test_test_harness_job_argument_does_not_trigger_cargo_warning(
    tmp_path: Path,
) -> None:
    result, _, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--", "test", "--", "--jobs=1"
    )

    assert result.returncode == 0, result.stderr
    assert "serializes compilation" not in result.stderr


@pytest.mark.parametrize("jobs", ("", "0", "000", "-1", "1.5", "many"))
def test_jobs_override_requires_a_positive_integer(tmp_path: Path, jobs: str) -> None:
    result, _, _ = _run_wrapper(tmp_path, "--jobs", jobs, "--", "build")

    assert result.returncode != 0
    assert "--jobs must be a positive integer" in result.stderr


def test_explicit_jobs_override_replaces_inherited_limit(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--jobs",
        "8",
        "--",
        "check",
        extra_env={"CARGO_BUILD_JOBS": "1"},
    )

    assert result.returncode == 0, result.stderr
    assert environment["CARGO_BUILD_JOBS"] == "8"
    assert "serializes compilation" not in result.stderr


def test_explicit_development_metadata_has_no_release_marker(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--stable-local-metadata",
        "--",
        "build",
    )

    assert result.returncode == 0, result.stderr
    assert environment["VERGEN_GIT_SHA"] == "local-fast-build"
    assert "IROHA_GIT_COMMIT_HASH" not in environment


@pytest.mark.parametrize("sealed", ["a" * 40, "", "local-fast-build"])
def test_local_metadata_rejects_a_sealed_marker_before_cargo(
    tmp_path: Path, sealed: str
) -> None:
    result, environment, cargo_arguments = _run_wrapper(
        tmp_path, "--no-sccache", "--stable-local-metadata", "--", "build",
        extra_env={"IROHA_GIT_COMMIT_HASH": sealed},
    )
    assert result.returncode != 0
    assert "conflicts with IROHA_GIT_COMMIT_HASH" in result.stderr
    assert environment == {}
    assert cargo_arguments == []


def test_exact_build_preserves_both_source_markers(tmp_path: Path) -> None:
    commit = "a" * 40
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--", "build",
        extra_env={"IROHA_GIT_COMMIT_HASH": commit, "VERGEN_GIT_SHA": commit},
    )
    assert result.returncode == 0, result.stderr
    assert environment["IROHA_GIT_COMMIT_HASH"] == commit
    assert environment["VERGEN_GIT_SHA"] == commit


def test_default_linker_does_not_probe_installed_alternatives(tmp_path: Path) -> None:
    probe_log = tmp_path / "linker-probe.log"
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--",
        "build",
        extra_env={"LINKER_PROBE_LOG": str(probe_log)},
        binaries={
            "cc": "#!/bin/sh\nprintf 'probed\\n' > \"$LINKER_PROBE_LOG\"\n",
            "ld64.lld": "#!/bin/sh\nexit 0\n",
            "lld": "#!/bin/sh\nexit 0\n",
            "uname": "#!/bin/sh\nprintf 'Darwin\\n'\n",
        },
    )

    assert result.returncode == 0, result.stderr
    assert not probe_log.exists()
    assert "RUSTFLAGS" not in environment
    assert "linker=system-default" in result.stdout


def test_auto_linker_remains_an_explicit_opt_in(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--no-sccache",
        "--linker",
        "auto",
        "--",
        "build",
        binaries={
            "cc": "#!/bin/sh\nexit 0\n",
            "mold": "#!/bin/sh\nexit 0\n",
            "uname": "#!/bin/sh\nprintf 'Linux\\n'\n",
        },
    )

    assert result.returncode == 0, result.stderr
    expected_linker = tmp_path / "bin" / "mold"
    assert environment["RUSTFLAGS"] == (
        f"-Clinker={tmp_path / 'bin' / 'cc'} -Clink-arg=-fuse-ld={expected_linker}"
    )
    assert f"linker={expected_linker}" in result.stdout


def test_sccache_uses_its_default_without_restarting_the_daemon(tmp_path: Path) -> None:
    sccache_log = tmp_path / "sccache.log"
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--",
        "check",
        extra_env={"SCCACHE_LOG": str(sccache_log)},
        binaries={
            "sccache": """
                #!/bin/sh
                printf '%s\n' "$*" >> "$SCCACHE_LOG"
                exit 0
            """,
        },
    )

    assert result.returncode == 0, result.stderr
    assert environment["RUSTC_WRAPPER"] == str(tmp_path / "bin" / "sccache")
    assert "SCCACHE_DIR" not in environment
    assert not sccache_log.exists()
    assert "--stop-server" not in SCRIPT.read_text(encoding="utf-8")


def test_explicit_sccache_directory_is_forwarded(tmp_path: Path) -> None:
    cache_dir = tmp_path / "sccache"
    result, environment, _ = _run_wrapper(
        tmp_path,
        "--sccache-dir",
        str(cache_dir),
        "--",
        "check",
        binaries={"sccache": "#!/bin/sh\nexit 0\n"},
    )

    assert result.returncode == 0, result.stderr
    assert environment["SCCACHE_DIR"] == str(cache_dir)
    assert cache_dir.is_dir()


def test_wrapper_stays_compatible_with_stock_macos_bash() -> None:
    result = subprocess.run(
        ["/bin/bash", "-n", str(SCRIPT)],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr

    source = SCRIPT.read_text(encoding="utf-8")
    for pattern in (
        re.compile(r"\bdeclare\s+-A\b"),
        re.compile(r"\blocal\s+-n\b"),
        re.compile(r"\b(?:mapfile|readarray)\b"),
        re.compile(r"\$\{[^}\n]+(?:,,|\^\^)[^}\n]*\}"),
    ):
        assert pattern.search(source) is None, pattern.pattern


@pytest.mark.parametrize("inherited", [False, True])
def test_incremental_never_dispatches_sccache(tmp_path: Path, inherited: bool) -> None:
    extra = {"RUSTC_WRAPPER": "/fixed/sccache", "CARGO_INCREMENTAL": "1"} if inherited else {}
    result, environment, arguments = _run_wrapper(
        tmp_path, *(() if inherited else ("--incremental",)), "--", "check", "-p", "iroha_core",
        extra_env=extra, binaries={"sccache": "#!/bin/sh\nexit 99\n"},
    )
    assert result.returncode == 0, result.stderr
    assert environment["CARGO_INCREMENTAL"] == "1"
    assert "RUSTC_WRAPPER" not in environment
    assert arguments == ["check", "-p", "iroha_core"]


def test_incremental_retains_unrelated_compiler_wrapper(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path, "--incremental", "--", "check", extra_env={"RUSTC_WRAPPER": "/fixed/instrument-rustc"},
    )
    assert result.returncode == 0, result.stderr
    assert environment["RUSTC_WRAPPER"] == "/fixed/instrument-rustc"


def _lane_role(target: Path, role: str = "release", repo: Path | None = None) -> Path:
    marker = target / ".taira-build-lane/role.json"
    marker.parent.mkdir(parents=True)
    marker.write_text(json.dumps({"schema": "taira.cargo-lane.v1", "repo_root": str(repo or REPO_ROOT), "role": role}))
    marker.chmod(0o600)
    return marker


@pytest.mark.parametrize("selection", ("default", "environment", "build-environment", "wrapper", "cargo", "cargo-equals", "config", "config-equals"))
def test_authenticated_release_lane_rejected_before_build(tmp_path: Path, selection: str) -> None:
    target = REPO_ROOT / "target" if selection == "default" else tmp_path / "release-target"
    marker = _lane_role(target)
    before = marker.read_bytes()
    args = ["--no-sccache"]
    environment = {}
    if selection == "environment":
        environment["CARGO_TARGET_DIR"] = str(target)
    elif selection == "build-environment":
        environment["CARGO_BUILD_TARGET_DIR"] = str(target)
    elif selection == "wrapper":
        args += ["--target-dir", str(target)]
    args += ["--", "check"]
    if selection == "cargo":
        args += ["--target-dir", str(target)]
    elif selection == "cargo-equals":
        args += ["--target-dir=" + str(target)]
    elif selection == "config":
        args += ["--config", "build.target-dir=" + json.dumps(str(target))]
    elif selection == "config-equals":
        args += ["--config=build.target-dir=" + json.dumps(str(target))]
    result, captured, _ = _run_wrapper(tmp_path, *args, extra_env=environment)
    assert result.returncode != 0
    assert "authenticated release lane" in result.stderr
    assert "--target-slot <stable-name>" in result.stderr
    assert not captured
    assert marker.read_bytes() == before
    assert not (target / "debug").exists()


def test_named_child_development_lane_does_not_use_release_parent(tmp_path: Path) -> None:
    _lane_role(REPO_ROOT / "target")
    result, environment, _ = _run_wrapper(tmp_path, "--no-sccache", "--target-slot", "fee-check", "--", "check")
    assert result.returncode == 0, result.stderr
    assert environment["CARGO_TARGET_DIR"] == str(REPO_ROOT / "target/cargo-fast/fee-check")
    assert not (REPO_ROOT / "target/cargo-fast").exists()


@pytest.mark.parametrize("role", ("unassigned", "development"))
def test_external_stable_development_lane_preserves_jobserver(tmp_path: Path, role: str) -> None:
    target = tmp_path / "stable-target"
    if role == "development":
        _lane_role(target, role)
    result, environment, _ = _run_wrapper(tmp_path, "--no-sccache", "--target-dir", str(target), "--", "check")
    assert result.returncode == 0, result.stderr
    assert "CARGO_BUILD_JOBS" not in environment
    assert environment["CARGO_TARGET_DIR"] == str(target)


def test_cargo_resolves_config_file_and_independent_build_directory(tmp_path: Path) -> None:
    protected = tmp_path / "release"
    _lane_role(protected)
    config = tmp_path / "extra-config.toml"
    config.write_text("# Fake Cargo owns resolution; the guard does not parse this file.\n")
    metadata = {"version": 1, "workspace_root": str(REPO_ROOT), "target_directory": str(tmp_path / "ordinary"), "build_directory": str(protected)}
    result, captured, _ = _run_wrapper(tmp_path, "--no-sccache", "--", "check", "--config", str(config), extra_env={"CARGO_FAST_TEST_METADATA": json.dumps(metadata)})
    assert result.returncode != 0
    assert "authenticated release lane" in result.stderr
    assert not captured


def test_cli_target_wins_over_config_and_ambient_release_target(tmp_path: Path) -> None:
    release = tmp_path / "release"
    _lane_role(release)
    ordinary = tmp_path / "ordinary"
    result, _, args = _run_wrapper(tmp_path, "--no-sccache", "--", "check", "--target-dir", str(ordinary), "--config", "build.target-dir=" + json.dumps(str(release)), extra_env={"CARGO_TARGET_DIR": str(release)})
    assert result.returncode == 0, result.stderr
    assert args[-1] == "build.target-dir=" + json.dumps(str(release))
    assert not ordinary.exists()


@pytest.mark.parametrize("damage", ("malformed", "symlink", "wrong-mode", "foreign-development"))
def test_invalid_or_foreign_lane_role_fails_closed(tmp_path: Path, damage: str) -> None:
    target = tmp_path / "release"
    marker = _lane_role(target)
    if damage == "malformed":
        marker.write_text("not-json")
    elif damage == "symlink":
        stored = marker.with_name("saved.json")
        marker.rename(stored)
        marker.symlink_to(stored)
    elif damage == "wrong-mode":
        marker.chmod(0o644)
    else:
        marker.write_text(json.dumps({"schema": "taira.cargo-lane.v1", "repo_root": str(tmp_path / "another-repo"), "role": "development"}))
    result, captured, _ = _run_wrapper(tmp_path, "--no-sccache", "--target-dir", str(target), "--", "check")
    assert result.returncode != 0 and "--target-slot <stable-name>" in result.stderr
    assert not captured


def test_missing_metadata_build_directory_does_not_guess(tmp_path: Path) -> None:
    result, captured, _ = _run_wrapper(tmp_path, "--no-sccache", "--", "check", extra_env={"CARGO_FAST_TEST_METADATA": json.dumps({"version": 1, "workspace_root": str(REPO_ROOT), "target_directory": str(REPO_ROOT / "target")})})
    assert result.returncode != 0 and "cannot resolve build_directory" in result.stderr
    assert not captured


def test_unexpanded_alias_is_rejected_before_build(tmp_path: Path) -> None:
    result, captured, _ = _run_wrapper(tmp_path, "--no-sccache", "--", "hidden-target-alias")
    assert result.returncode != 0 and "hidden in a Cargo alias" in result.stderr
    assert not captured


def test_program_config_argument_cannot_select_release_lane(tmp_path: Path) -> None:
    release = tmp_path / "release"
    _lane_role(release)
    result, _, args = _run_wrapper(tmp_path, "--no-sccache", "--", "test", "--", "--config", "build.target-dir=" + json.dumps(str(release)))
    assert result.returncode == 0, result.stderr
    assert args[1:3] == ["--", "--config"]


def test_metadata_preserves_toolchain_config_order_and_cli_priority(tmp_path: Path) -> None:
    capture = tmp_path / "resolution.json"
    target = tmp_path / "explicit-target"
    first = "build.target-dir=" + json.dumps(str(tmp_path / "first"))
    second = "build.target-dir=" + json.dumps(str(tmp_path / "second"))
    result, _, _ = _run_wrapper(tmp_path, "--no-sccache", "--", "+1.93.1", "--config", first, "check", "--config=" + second, "--target-dir", str(target), extra_env={"CARGO_FAST_RESOLUTION_CAPTURE": str(capture)})
    assert result.returncode == 0, result.stderr
    query = json.loads(capture.read_text())
    assert query[:7] == ["+1.93.1", "--config", first, "--config", second, "--config", "build.target-dir=" + json.dumps(str(target))]
    assert query[7:12] == ["metadata", "--locked", "--offline", "--no-deps", "--format-version=1"]
    assert not target.exists()
