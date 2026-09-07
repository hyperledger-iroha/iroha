"""Focused linker-choice regressions; fake Cargo never starts a build."""

from pathlib import Path

import pytest

from cargo_fast_test import _run_wrapper, _write_executable


GCC_DRIVER = r'''#!/bin/sh
printf '%s\n' "$*" >> "$LINKER_PROBE_LOG"
case "$1" in
  -print-prog-name=ld.lld|-print-prog-name=ld.mold)
    printf '%s\n' "$TEST_RESOLVED_LINKER"; exit 0 ;;
  -fuse-ld=lld|-fuse-ld=mold) exit 0 ;;
  *) printf 'gcc: unsupported absolute fuse-ld argument\n' >&2; exit 1 ;;
esac
'''


@pytest.mark.parametrize("mode,binary,named", [("lld", "ld.lld", "lld"), ("ld.lld", "ld.lld", "lld"), ("mold", "mold", "mold")])
@pytest.mark.parametrize("bare_resolution", [False, True])
def test_gcc_named_fallback_pins_the_probed_driver_and_same_linker(tmp_path: Path, mode: str, binary: str, named: str, bare_resolution: bool) -> None:
    resolved = binary if bare_resolution else str(tmp_path / "bin" / binary)
    result, environment, args = _run_wrapper(
        tmp_path, "--no-sccache", "--linker", mode, "--", "build", "-p", "example",
        extra_env={"LINKER_PROBE_LOG": str(tmp_path / "probe.log"), "TEST_RESOLVED_LINKER": resolved,
                   "RUSTFLAGS": "-Cdebuginfo=0", "CC": "/not-the-rustc-driver"},
        binaries={"cc": GCC_DRIVER, binary: "#!/bin/sh\nexit 0\n", "uname": "#!/bin/sh\necho Linux\n"},
    )
    assert result.returncode == 0, result.stderr
    assert args == ["build", "-p", "example"]
    assert environment["RUSTFLAGS"] == f"-Cdebuginfo=0 -Clinker={tmp_path / 'bin/cc'} -Clink-arg=-fuse-ld={named}"
    assert f"linker={tmp_path / 'bin' / binary}" in result.stdout
    probe = (tmp_path / "probe.log").read_text()
    assert f"-fuse-ld={tmp_path / 'bin' / binary}" in probe
    assert f"-fuse-ld={named} " in probe


def test_custom_path_never_falls_back_to_different_same_named_linker(tmp_path: Path) -> None:
    custom = tmp_path / "custom/ld.lld"
    custom.parent.mkdir()
    _write_executable(custom, "#!/bin/sh\nexit 0\n")
    result, captured, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--linker", str(custom), "--", "build",
        extra_env={"LINKER_PROBE_LOG": str(tmp_path / "probe.log"), "TEST_RESOLVED_LINKER": "ld.lld"},
        binaries={"cc": GCC_DRIVER, "ld.lld": "#!/bin/sh\nexit 0\n"},
    )
    assert result.returncode != 0
    assert not captured
    assert "cannot be honored" in result.stderr
    assert "not requested" in result.stderr
    assert "-fuse-ld=lld " not in (tmp_path / "probe.log").read_text()


def test_custom_symlink_to_exact_compiler_linker_can_use_named_form(tmp_path: Path) -> None:
    (tmp_path / "bin").mkdir()
    custom = tmp_path / "custom/ld.lld"
    custom.parent.mkdir()
    custom.symlink_to(tmp_path / "bin/ld.lld")
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--linker", str(custom), "--", "build",
        extra_env={"LINKER_PROBE_LOG": str(tmp_path / "probe.log"), "TEST_RESOLVED_LINKER": "ld.lld"},
        binaries={"cc": GCC_DRIVER, "ld.lld": "#!/bin/sh\nexit 0\n"},
    )
    assert result.returncode == 0, result.stderr
    assert environment["RUSTFLAGS"].endswith("-Clink-arg=-fuse-ld=lld")
    assert f"linker={custom}" in result.stdout


def test_clang_custom_path_remains_exact(tmp_path: Path) -> None:
    custom = tmp_path / "custom-linker"
    _write_executable(custom, "#!/bin/sh\nexit 0\n")
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--linker", str(custom), "--", "build",
        binaries={"cc": "#!/bin/sh\nexit 0\n"},
    )
    assert result.returncode == 0, result.stderr
    assert environment["RUSTFLAGS"] == f"-Clinker={tmp_path / 'bin/cc'} -Clink-arg=-fuse-ld={custom}"


def test_missing_explicit_linker_stops_before_cargo(tmp_path: Path) -> None:
    result, captured, _ = _run_wrapper(tmp_path, "--linker", str(tmp_path / "missing"), "--", "build")
    assert result.returncode != 0
    assert not captured
    assert "requested executable was not found" in result.stderr


@pytest.mark.parametrize("mode,success", [("auto", True), ("ld64.lld", False)])
def test_only_auto_may_fall_back_after_failed_link_probe(tmp_path: Path, mode: str, success: bool) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--linker", mode, "--", "build",
        binaries={"cc": "#!/bin/sh\necho diagnostic-probe-failure >&2\nexit 1\n", "ld64.lld": "#!/bin/sh\nexit 0\n", "uname": "#!/bin/sh\necho Darwin\n"},
    )
    assert (result.returncode == 0) == success
    assert "diagnostic-probe-failure" in result.stderr
    assert "RUSTFLAGS" not in environment
    if success:
        assert "linker=system-default" in result.stdout
    else:
        assert not environment


@pytest.mark.parametrize("override", [
    {"CARGO_ENCODED_RUSTFLAGS": ""},
    {"CARGO_ENCODED_RUSTFLAGS": "-Cdebuginfo=0"},
    {"CARGO_BUILD_TARGET": "aarch64-unknown-linux-gnu"},
    {"CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER": "/another/compiler"},
    {"RUSTFLAGS": "-C linker=/another/compiler"},
    {"RUSTFLAGS": "-Clink-arg=-fuse-ld=another"},
])
def test_explicit_inherited_driver_or_encoded_flags_do_not_get_silently_overridden(tmp_path: Path, override: dict[str, str]) -> None:
    result, captured, _ = _run_wrapper(tmp_path, "--linker", "lld", "--", "build", extra_env=override)
    assert result.returncode != 0
    assert not captured
    assert "cannot be honored" in result.stderr
    assert "--linker off" in result.stderr


@pytest.mark.parametrize("cargo_options", [("--target", "aarch64-unknown-linux-gnu"), ("--target=aarch64-unknown-linux-gnu",), ("--config", "target.x.linker=custom")])
def test_explicit_target_or_config_requires_caller_managed_linker(tmp_path: Path, cargo_options: tuple[str, ...]) -> None:
    result, captured, _ = _run_wrapper(tmp_path, "--linker", "lld", "--", "build", *cargo_options)
    assert result.returncode != 0
    assert not captured
    assert "Cargo --target/--config" in result.stderr


def test_off_preserves_external_linker_and_encoded_flags_without_probe(tmp_path: Path) -> None:
    result, environment, _ = _run_wrapper(
        tmp_path, "--no-sccache", "--linker", "off", "--", "build", "--target=custom",
        extra_env={"CARGO_ENCODED_RUSTFLAGS": "-Clinker=custom", "LINKER_PROBE_LOG": str(tmp_path / "probe.log")},
        binaries={"cc": GCC_DRIVER},
    )
    assert result.returncode == 0, result.stderr
    assert environment["CARGO_ENCODED_RUSTFLAGS"] == "-Clinker=custom"
    assert not (tmp_path / "probe.log").exists()
