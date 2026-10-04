"""Process-policy controls for the sole explicit FastPQ Metal candidate producer.

Stand-in outputs are unqualified test bytes, never admitted libraries or hardware evidence."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
PRODUCER = ROOT / "scripts/build_fastpq_metal_bundle.py"


def _write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)


@pytest.fixture(scope="module")
def producer() -> Path:
    """Use the sole explicit producer, without compiling or invoking build.rs."""
    assert PRODUCER.is_file()
    return PRODUCER


@pytest.fixture
def fake_toolchain(tmp_path: Path) -> tuple[Path, Path, Path]:
    """Create deterministic xcrun, metal, and metallib stand-ins."""

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
        fake_bin / "metal",
        """#!/bin/sh
printf 'metal' >> "$FASTPQ_TEST_TOOL_LOG"
for argument in "$@"; do printf '|%s' "$argument" >> "$FASTPQ_TEST_TOOL_LOG"; done
printf '\n' >> "$FASTPQ_TEST_TOOL_LOG"
if [ "$#" -eq 1 ] && [ "$1" = "-v" ]; then
    if [ -n "${FASTPQ_TEST_BROKEN_METAL:-}" ]; then
        printf 'compiler probe failed\n' >&2
        exit 8
    fi
    exit 0
fi
if [ "$#" -eq 1 ] && [ "$1" = "-help" ]; then
    printf '%s\n' '-fno-fast-math'
    exit 0
fi
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


def _run_producer(
    executable: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
    *,
    initially_ready: bool = False,
    prepopulate_outputs: bool = False,
    extra_env: dict[str, str] | None = None,
    skip: bool = False,
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    fake_bin, log, ready = fake_toolchain
    if initially_ready:
        ready.touch()
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    if prepopulate_outputs:
        for filename in (
            "ntt_stage.air",
            "poseidon.air",
            "bn254.air",
            "fastpq.metallib",
        ):
            (out_dir / filename).write_text("stale\n", encoding="utf-8")
    environment = os.environ.copy()
    environment.update(
        {
            "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
            "FASTPQ_TEST_TOOL_LOG": str(log),
            "FASTPQ_TEST_TOOLCHAIN_READY": str(ready),
            "FASTPQ_TEST_METAL": str(fake_bin / "metal"),
            "FASTPQ_TEST_METALLIB": str(fake_bin / "metallib"),
        }
    )
    if extra_env:
        environment.update(extra_env)
    completed = subprocess.run(
        [sys.executable, str(executable), "--repo-root", str(ROOT),
         "--output", str(out_dir / "candidate"), "--target", "aarch64-apple-darwin",
         *(["--skip"] if skip else [])],
        check=False,
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    lines = log.read_text(encoding="utf-8").splitlines() if log.exists() else []
    return completed, lines


def test_working_compiler_and_linker_do_not_trigger_download(
    producer: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_producer(
        producer, fake_toolchain, tmp_path, initially_ready=True
    )

    assert completed.returncode == 0, completed.stderr
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert log.count("metal|-v") == 1
    assert log.count("metallib|-v") == 1
    record = json.loads((tmp_path / "out/candidate/generation.json").read_text())
    assert record["admission_authority"] is False
    assert record["signed_provenance"] is False
    assert record["hardware_qualification"] is False
    assert len(record["sources"]) == 8
    assert len(record["entry_points"]) == 16
    assert len([line for line in log if line.startswith("metal|") and "|-c|" in line]) == 6
    assert "unqualified" in completed.stdout
    assert "sha256=" in completed.stdout


def test_missing_toolchain_reports_manual_remediation_without_mutating_host(
    producer: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_producer(producer, fake_toolchain, tmp_path)

    assert completed.returncode == 1, completed.stderr
    assert log == [
        "xcrun|-sdk|macosx|--find|metal",
        "xcrun|--find|metal",
    ]
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert not any("--kill-cache" in line for line in log)
    assert "Metal compiler/linker is unavailable" in completed.stderr
    assert "xcodebuild -downloadComponent MetalToolchain" in completed.stderr
    assert "--skip" in completed.stderr
    assert not (tmp_path / "out/candidate").exists()


def test_broken_linker_reports_probe_error_without_redetection_or_host_mutation(
    producer: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_producer(
        producer,
        fake_toolchain,
        tmp_path,
        initially_ready=True,
        extra_env={"FASTPQ_TEST_BROKEN_METALLIB": "1"},
    )

    assert completed.returncode == 1, completed.stderr
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert not any("--kill-cache" in line for line in log)
    assert log.count("metal|-v") == 1
    assert log.count("metallib|-v") == 1
    assert "linker probe failed" in completed.stderr
    assert "xcode-select -p" in completed.stderr
    assert "xcodebuild -downloadComponent MetalToolchain" in completed.stderr
    assert "--skip" in completed.stderr
    assert not (tmp_path / "out/candidate").exists()


def test_broken_compiler_reports_probe_error_and_manual_remediation(
    producer: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_producer(
        producer,
        fake_toolchain,
        tmp_path,
        initially_ready=True,
        extra_env={"FASTPQ_TEST_BROKEN_METAL": "1"},
    )

    assert completed.returncode == 1, completed.stderr
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert not any("--kill-cache" in line for line in log)
    assert log.count("metal|-v") == 1
    assert "compiler probe failed" in completed.stderr
    assert "xcode-select -p" in completed.stderr
    assert "xcodebuild -downloadComponent MetalToolchain" in completed.stderr
    assert "--skip" in completed.stderr
    assert not (tmp_path / "out/candidate").exists()


def test_success_without_fresh_compiler_output_rejects_stale_artifacts(
    producer: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_producer(
        producer,
        fake_toolchain,
        tmp_path,
        initially_ready=True,
        prepopulate_outputs=True,
        extra_env={"FASTPQ_TEST_NO_TOOL_OUTPUT": "1"},
    )

    assert completed.returncode == 1, completed.stderr
    assert not any(line.startswith("xcodebuild|") for line in log)
    assert "Metal AIR object was not produced" in completed.stderr
    assert not (tmp_path / "out/candidate").exists()
    for filename in ("ntt_stage.air", "poseidon.air", "bn254.air", "fastpq.metallib"):
        assert (tmp_path / "out" / filename).read_text() == "stale\n"


def test_explicit_skip_never_probes_or_downloads(
    producer: Path,
    fake_toolchain: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    completed, log = _run_producer(
        producer,
        fake_toolchain,
        tmp_path,
        skip=True,
    )

    assert completed.returncode == 0, completed.stderr
    assert log == []
    assert "explicit --skip" in completed.stdout
    assert not (tmp_path / "out/candidate").exists()
