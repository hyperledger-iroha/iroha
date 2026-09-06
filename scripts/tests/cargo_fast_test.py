"""Regression tests for the local Cargo acceleration wrapper."""

from __future__ import annotations

import os
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
    "CARGO_FAST_TARGET_ROOT",
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
    "CMAKE_BUILD_PARALLEL_LEVEL",
    "GITHUB_ACTIONS",
    "IROHA_GIT_COMMIT_HASH",
    "RUST_TEST_THREADS",
    "RUSTC_WRAPPER",
    "RUSTFLAGS",
    "SCCACHE_DIR",
    "VERGEN_GIT_SHA",
)

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
        """
        #!/bin/sh
        {
          env
          printf '%s\n' __CARGO_FAST_ARGS__
          printf '%s\n' "$@"
        } > "$CARGO_FAST_CAPTURE"
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


def test_stable_metadata_only_sets_the_non_authoritative_sha(tmp_path: Path) -> None:
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
    assert "IROHA_GIT_COMMIT_HASH" not in SCRIPT.read_text(encoding="utf-8")


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
        f"-Clink-arg=-fuse-ld={expected_linker}"
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
