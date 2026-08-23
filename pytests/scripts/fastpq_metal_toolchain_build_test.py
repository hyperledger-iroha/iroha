"""Regression tests for the FASTPQ macOS Metal toolchain bootstrap."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "crates" / "fastpq_prover"
BUILD_RS = CRATE / "build.rs"


def _write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)


@pytest.fixture(scope="module")
def build_script(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Compile build.rs against a no-op cc stub without invoking Cargo."""

    rustc = shutil.which("rustc")
    assert rustc is not None, "the Rust workspace test host must provide rustc"
    build_dir = tmp_path_factory.mktemp("fastpq-metal-build-script")
    cc_stub = build_dir / "cc_stub.rs"
    cc_stub.write_text(
        """
        use std::path::Path;

        pub struct Build;

        impl Build {
            pub fn new() -> Self { Self }
            pub fn cuda(&mut self, _: bool) -> &mut Self { self }
            pub fn debug(&mut self, _: bool) -> &mut Self { self }
            pub fn file<P: AsRef<Path>>(&mut self, _: P) -> &mut Self { self }
            pub fn flag<S: AsRef<str>>(&mut self, _: S) -> &mut Self { self }
            pub fn include<P: AsRef<Path>>(&mut self, _: P) -> &mut Self { self }
            pub fn ccbin(&mut self, _: bool) -> &mut Self { self }
            pub fn compile(&mut self, _: &str) {}
        }
        """,
        encoding="utf-8",
    )
    cc_rlib = build_dir / "libcc.rlib"
    subprocess.run(
        [
            rustc,
            "--edition=2024",
            "--crate-name=cc",
            "--crate-type=rlib",
            str(cc_stub),
            "-o",
            str(cc_rlib),
        ],
        check=True,
        cwd=ROOT,
    )
    executable = build_dir / "fastpq-build-script"
    subprocess.run(
        [
            rustc,
            "--edition=2024",
            str(BUILD_RS),
            "--extern",
            f"cc={cc_rlib}",
            "-o",
            str(executable),
        ],
        check=True,
        cwd=ROOT,
    )
    return executable


@pytest.fixture
def fake_toolchain(tmp_path: Path) -> tuple[Path, Path, Path]:
    """Create deterministic xcrun, xcodebuild, metal, and metallib stand-ins."""

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    log = tmp_path / "tool.log"
    ready = tmp_path / "toolchain-ready"
    _write_executable(
        fake_bin / "xcrun",
        """#!/bin/sh
printf 'xcrun' >> "$FASTPQ_TEST_TOOL_LOG"
for argument in "$@"; do printf '|%s' "$argument" >> "$FASTPQ_TEST_TOOL_LOG"; done
printf '\n' >> "$FASTPQ_TEST_TOOL_LOG"
if [ "$#" -eq 1 ] && [ "$1" = "--kill-cache" ]; then
    exit "${FASTPQ_TEST_XCRUN_CACHE_STATUS:-0}"
fi
if [ ! -f "$FASTPQ_TEST_TOOLCHAIN_READY" ]; then
    printf 'toolchain unavailable\n' >&2
    exit 1
fi
case "$*" in
    *"--find metal") printf '%s\n' "$FASTPQ_TEST_METAL" ;;
    *"--find metallib") printf '%s\n' "$FASTPQ_TEST_METALLIB" ;;
    *) printf 'unexpected xcrun arguments: %s\n' "$*" >&2; exit 2 ;;
esac
""",
    )
    _write_executable(
        fake_bin / "xcodebuild",
        """#!/bin/sh
printf 'xcodebuild' >> "$FASTPQ_TEST_TOOL_LOG"
for argument in "$@"; do printf '|%s' "$argument" >> "$FASTPQ_TEST_TOOL_LOG"; done
printf '\n' >> "$FASTPQ_TEST_TOOL_LOG"
if [ -n "${FASTPQ_TEST_XCODEBUILD_STATUS:-}" ]; then
    exit "$FASTPQ_TEST_XCODEBUILD_STATUS"
fi
: > "$FASTPQ_TEST_TOOLCHAIN_READY"
""",
    )
    _write_executable(
        fake_bin / "metal",
        """#!/bin/sh
printf 'metal' >> "$FASTPQ_TEST_TOOL_LOG"
for argument in "$@"; do printf '|%s' "$argument" >> "$FASTPQ_TEST_TOOL_LOG"; done
printf '\n' >> "$FASTPQ_TEST_TOOL_LOG"
if [ "$#" -eq 1 ] && [ "$1" = "-v" ]; then exit 0; fi
if [ -n "${FASTPQ_TEST_NO_TOOL_OUTPUT:-}" ]; then exit 0; fi
output=
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-o" ]; then shift; output=$1; fi
    shift
done
printf 'fake-air\n' > "$output"
""",
    )
    _write_executable(
        fake_bin / "metallib",
        """#!/bin/sh
printf 'metallib' >> "$FASTPQ_TEST_TOOL_LOG"
for argument in "$@"; do printf '|%s' "$argument" >> "$FASTPQ_TEST_TOOL_LOG"; done
printf '\n' >> "$FASTPQ_TEST_TOOL_LOG"
if [ "$#" -eq 1 ] && [ "$1" = "-v" ]; then
    if [ -n "${FASTPQ_TEST_BROKEN_METALLIB:-}" ]; then
        printf 'linker probe failed\n' >&2
        exit 7
    fi
    exit 0
fi
if [ -n "${FASTPQ_TEST_NO_TOOL_OUTPUT:-}" ]; then exit 0; fi
output=
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-o" ]; then shift; output=$1; fi
    shift
done
printf 'fake-metallib\n' > "$output"
""",
    )
    return fake_bin, log, ready


def _run_build_script(
    executable: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
    *,
    initially_ready: bool = False,
    prepopulate_outputs: bool = False,
    extra_env: dict[str, str] | None = None,
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    fake_bin, log, ready = fake_toolchain
    if initially_ready:
        ready.touch()
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    if prepopulate_outputs:
        for filename in (
            "ntt_stage.air",
            "poseidon2.air",
            "bn254.air",
            "fastpq.metallib",
        ):
            (out_dir / filename).write_text("stale\n", encoding="utf-8")
    environment = os.environ.copy()
    environment.update(
        {
            "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
            "CARGO_CFG_TARGET_OS": "macos",
            "CARGO_FEATURE_FASTPQ_GPU": "1",
            "OUT_DIR": str(out_dir),
            "FASTPQ_TEST_TOOL_LOG": str(log),
            "FASTPQ_TEST_TOOLCHAIN_READY": str(ready),
            "FASTPQ_TEST_METAL": str(fake_bin / "metal"),
            "FASTPQ_TEST_METALLIB": str(fake_bin / "metallib"),
        }
    )
    environment.pop("CARGO_FEATURE_CUDA", None)
    environment.pop("FASTPQ_SKIP_GPU_BUILD", None)
    if extra_env:
        environment.update(extra_env)
    completed = subprocess.run(
        [str(executable)],
        check=False,
        cwd=CRATE,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    lines = log.read_text(encoding="utf-8").splitlines() if log.exists() else []
    return completed, lines


def test_working_compiler_and_linker_do_not_trigger_download(
    build_script: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_build_script(
        build_script, fake_toolchain, tmp_path, initially_ready=True
    )

    assert completed.returncode == 0, completed.stderr
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert log.count("metal|-v") == 1
    assert log.count("metallib|-v") == 1
    assert "cargo:rustc-env=FASTPQ_METAL_LIB=" in completed.stdout
    assert "fastpq.metallib" in completed.stdout


def test_missing_toolchain_downloads_exact_component_then_redetects(
    build_script: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_build_script(build_script, fake_toolchain, tmp_path)

    assert completed.returncode == 0, completed.stderr
    download = "xcodebuild|-downloadComponent|MetalToolchain"
    assert log.count(download) == 1
    assert log.index(download) < log.index("xcrun|--kill-cache")
    assert log.index("xcrun|--kill-cache") < log.index("metal|-v")
    assert log.index("metal|-v") < log.index("metallib|-v")
    assert "fastpq.metallib" in completed.stdout


def test_broken_linker_triggers_download_and_actionable_redetection_error(
    build_script: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_build_script(
        build_script,
        fake_toolchain,
        tmp_path,
        initially_ready=True,
        extra_env={
            "FASTPQ_TEST_BROKEN_METALLIB": "1",
            "FASTPQ_TEST_XCRUN_CACHE_STATUS": "6",
        },
    )

    assert completed.returncode == 0, completed.stderr
    assert log.count("xcodebuild|-downloadComponent|MetalToolchain") == 1
    assert log.count("metallib|-v") == 2
    assert "compiler/linker redetection failed" in completed.stdout
    assert "`xcrun --kill-cache` exited with exit status: 6" in completed.stdout
    assert "xcode-select -p" in completed.stdout
    assert "xcodebuild -downloadComponent MetalToolchain" in completed.stdout
    assert "FASTPQ_SKIP_GPU_BUILD=1" in completed.stdout
    assert "cargo:rustc-env=FASTPQ_METAL_LIB=" in completed.stdout


def test_download_failure_reports_status_and_remediation(
    build_script: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_build_script(
        build_script,
        fake_toolchain,
        tmp_path,
        extra_env={"FASTPQ_TEST_XCODEBUILD_STATUS": "9"},
    )

    assert completed.returncode == 0, completed.stderr
    assert log.count("xcodebuild|-downloadComponent|MetalToolchain") == 1
    assert "exited with exit status: 9" in completed.stdout
    assert "xcode-select -p" in completed.stdout
    assert "FASTPQ_SKIP_GPU_BUILD=1" in completed.stdout


def test_success_without_fresh_compiler_output_rejects_stale_artifacts(
    build_script: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_build_script(
        build_script,
        fake_toolchain,
        tmp_path,
        initially_ready=True,
        prepopulate_outputs=True,
        extra_env={"FASTPQ_TEST_NO_TOOL_OUTPUT": "1"},
    )

    assert completed.returncode == 0, completed.stderr
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert "Metal AIR object was not produced" in completed.stdout
    assert "cargo:rustc-env=FASTPQ_METAL_LIB=" in completed.stdout.splitlines()
    assert "cargo:rustc-cfg=fastpq_metal_available" not in completed.stdout


def test_explicit_skip_never_probes_or_downloads(
    build_script: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_build_script(
        build_script,
        fake_toolchain,
        tmp_path,
        extra_env={"FASTPQ_SKIP_GPU_BUILD": "1"},
    )

    assert completed.returncode == 0, completed.stderr
    assert log == []
    assert "FASTPQ_SKIP_GPU_BUILD set" in completed.stdout
    assert "cargo:rustc-env=FASTPQ_METAL_LIB=" in completed.stdout
