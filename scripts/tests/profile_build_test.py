"""Unit tests for the isolated Cargo build profiler."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "profile_build.py"
SPEC = importlib.util.spec_from_file_location("profile_build", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
_PREVIOUS_DONT_WRITE_BYTECODE = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    SPEC.loader.exec_module(MODULE)
finally:
    sys.dont_write_bytecode = _PREVIOUS_DONT_WRITE_BYTECODE


def _fake_legacy_fixture(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> dict[str, Path]:
    root = tmp_path / "repo"
    root.mkdir()
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
    (root / "source.rs").write_text("fn caller() {}\n", encoding="utf-8")
    cargo_home = tmp_path / "caller-cargo-home"
    (cargo_home / "registry").mkdir(parents=True)
    (cargo_home / "registry" / "seed").write_text("seed\n", encoding="utf-8")
    cargo_home.chmod(0o700)
    rustup_home = tmp_path / "caller-rustup-home"
    toolchain = rustup_home / "toolchains" / "test" / "bin"
    toolchain.mkdir(parents=True)
    rustup_home.chmod(0o700)
    cargo = toolchain / "cargo"
    cargo.write_text(
        "#!/bin/sh\n"
        'if [ "${1:-}" = "-Vv" ]; then printf "fake cargo 1.0\\n"; exit 0; fi\n'
        'printf "private\\n" > "$CARGO_HOME/build-write"\n'
        'mkdir -p "$CARGO_TARGET_DIR"\n'
        'printf "artifact\\n" > "$CARGO_TARGET_DIR/fake-artifact"\n'
        "exit 0\n",
        encoding="utf-8",
    )
    rustc = toolchain / "rustc"
    rustc.write_text(
        "#!/bin/sh\nprintf 'fake rustc 1.0\\n'\n", encoding="utf-8"
    )
    cargo.chmod(0o700)
    rustc.chmod(0o700)
    fake_bin = tmp_path / "fake-bin"
    fake_bin.mkdir()
    (fake_bin / "cargo").symlink_to(cargo)
    (fake_bin / "rustc").symlink_to(rustc)
    git = fake_bin / "git"
    git.write_text(
        "#!/bin/sh\n"
        "case \" $* \" in\n"
        "  *\" --show-toplevel \"*)\n"
        "    previous=\n"
        "    for argument in \"$@\"; do\n"
        "      if [ \"$previous\" = -C ]; then\n"
        "        printf '%s\\n' \"$argument\"; exit 0\n"
        "      fi\n"
        "      previous=$argument\n"
        "    done\n"
        "    ;;\n"
        "esac\n"
        "for argument in \"$@\"; do\n"
        "  if [ \"$argument\" = ls-files ]; then\n"
        "    printf 'Cargo.lock\\0Cargo.toml\\0source.rs\\0'\n"
        "    exit 0\n"
        "  fi\n"
        "  if [ \"$argument\" = rev-parse ]; then\n"
        "    printf 'fake-revision\\n'\n"
        "    exit 0\n"
        "  fi\n"
        "done\n"
        "exit 2\n",
        encoding="utf-8",
    )
    git.chmod(0o700)
    monkeypatch.setenv("PATH", f"{fake_bin}:/usr/bin:/bin")
    return {
        "root": root,
        "target": tmp_path / "target",
        "output": tmp_path / "reports" / "profile.json",
        "cargo_home": cargo_home,
        "rustup_home": rustup_home,
    }


def test_validate_target_dir_refuses_implicit_cache_reuse(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    target = tmp_path / "profile-target"
    target.mkdir()
    (target / "artifact").write_bytes(b"artifact")

    with pytest.raises(ValueError, match="pass --reuse"):
        MODULE.validate_target_dir(target, root=root, reuse=False)
    assert MODULE.validate_target_dir(target, root=root, reuse=True) == target.resolve()


def test_validate_target_dir_creates_isolated_directory(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    target = tmp_path / "new" / "target"
    resolved = MODULE.validate_target_dir(target, root=root, reuse=False)
    assert resolved.is_dir()
    assert not any(resolved.iterdir())


def test_legacy_warm_target_rejects_hardlink_to_caller_source(
    tmp_path: Path,
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    source = root / "source.rs"
    source.write_text("caller\n", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    MODULE.os.link(source, target / "artifact")

    resolved = MODULE.validate_target_dir(target, root=root, reuse=True)
    with pytest.raises(ValueError, match="hard-linked"):
        MODULE.ISOLATION.validate_writable_tree(
            resolved, label="target directory"
        )
    assert source.read_text(encoding="utf-8") == "caller\n"


def test_profile_paths_must_be_external_absent_and_disjoint(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    external_target = tmp_path / "target"
    with pytest.raises(ValueError, match="outside the repository"):
        MODULE.validate_target_dir(root / "target", root=root, reuse=False)

    external_target.mkdir()
    with pytest.raises(ValueError, match="outside the repository"):
        MODULE.validate_output_path(root / "profile.json", root, external_target)
    with pytest.raises(ValueError, match="outside the target"):
        MODULE.validate_output_path(
            external_target / "profile.json", root, external_target
        )

    report = tmp_path / "profile.json"
    report.write_text("retained\n", encoding="utf-8")
    with pytest.raises(ValueError, match="already exists"):
        MODULE.validate_output_path(report, root, external_target)
    assert report.read_text(encoding="utf-8") == "retained\n"


def test_minimal_environment_drops_ambient_inputs_and_pins_cargo(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    target = tmp_path / "target"
    cargo_home = tmp_path / "cargo-home"
    rustup_home = tmp_path / "rustup-home"
    state = tmp_path / "state"
    for directory in (target, cargo_home, rustup_home, state / "home", state / "tmp"):
        directory.mkdir(parents=True)
    cargo_home.chmod(0o700)
    rustup_home.chmod(0o700)
    rustc = tmp_path / "rustc"
    rustc.write_text("tool\n", encoding="utf-8")
    monkeypatch.setenv("UNREVIEWED_PROFILE_INPUT", "must-not-leak")
    monkeypatch.setenv("RUSTFLAGS", "must-not-leak")

    environment = MODULE.minimal_environment(
        root, target, cargo_home, rustup_home, state, rustc
    )

    assert "UNREVIEWED_PROFILE_INPUT" not in environment
    assert "RUSTFLAGS" not in environment
    assert environment["CARGO_NET_OFFLINE"] == "true"
    assert environment["CARGO_TARGET_DIR"] == str(target)
    assert environment["CARGO_HOME"] == str(cargo_home)
    assert environment["RUSTUP_HOME"] == str(rustup_home)
    assert environment["HOME"] == str(state / "home")
    assert environment["TMPDIR"] == str(state / "tmp")
    assert environment["RUSTC"] == str(rustc)
    assert environment["GIT_OPTIONAL_LOCKS"] == "0"
    assert all(
        "--locked" in scenario and "--offline" in scenario
        for scenario in MODULE.SCENARIOS.values()
    )


def test_report_is_reserved_exclusively_before_measurement(tmp_path: Path) -> None:
    report = tmp_path / "profile.json"
    descriptor = MODULE.reserve_report(report)
    try:
        with pytest.raises(FileExistsError):
            MODULE.reserve_report(report)
    finally:
        MODULE.os.close(descriptor)
    assert report.read_bytes() == b""


def test_private_roots_cannot_alias_the_candidate_or_target(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    target = tmp_path / "target"
    cargo_home = tmp_path / "cargo-home"
    rustup_home = tmp_path / "rustup-home"
    for directory in (root, target, cargo_home, rustup_home):
        directory.mkdir()
    cargo_home.chmod(0o700)
    rustup_home.chmod(0o700)
    assert MODULE.validate_private_roots(
        root, target, cargo_home, rustup_home
    ) == (cargo_home, rustup_home)
    nested = root / "cargo-home"
    nested.mkdir()
    nested.chmod(0o700)
    with pytest.raises(ValueError, match="external and disjoint"):
        MODULE.validate_private_roots(root, target, nested, rustup_home)


def test_legacy_profiler_has_no_process_table_observation() -> None:
    source = MODULE_PATH.read_text(encoding="utf-8")
    assert "process_tree" not in source
    assert '"ps"' not in source


def test_resolve_tool_uses_rustup_selected_executable(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    rustup = tmp_path / "rustup"
    cargo = tmp_path / "toolchain" / "cargo"
    cargo.parent.mkdir()
    rustup.write_bytes(b"rustup-launcher")
    cargo.write_bytes(b"authenticated-cargo")
    rustup.chmod(0o700)
    cargo.chmod(0o700)
    safe_home = tmp_path / "private-home"
    safe_home.mkdir()
    environment = {"HOME": str(safe_home), "PATH": "/authenticated/bin"}
    monkeypatch.setattr(MODULE.shutil, "which", lambda *_args, **_kwargs: str(rustup))
    monkeypatch.setattr(
        MODULE.subprocess,
        "check_output",
        lambda command, **kwargs: str(cargo) + "\n",
    )

    discovered, resolved, digest, launcher, launcher_digest = MODULE.resolve_tool(
        "cargo", root, environment["PATH"], environment
    )

    assert discovered == rustup
    assert resolved == cargo
    assert digest == MODULE.hashlib.sha256(cargo.read_bytes()).hexdigest()
    assert launcher == rustup
    assert launcher_digest == MODULE.hashlib.sha256(rustup.read_bytes()).hexdigest()


def test_directory_size_ignores_symlinks(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.mkdir()
    (target / "artifact").write_bytes(b"1234")
    (target / "artifact-link").symlink_to(target / "artifact")

    assert MODULE.directory_size(target) == 4


def test_parse_args_requires_positive_jobs_during_measurement(tmp_path: Path) -> None:
    cargo_home = tmp_path / "cargo-home"
    rustup_home = tmp_path / "rustup-home"
    args = MODULE.parse_args(
        [
            "workspace",
            "--target-dir",
            str(tmp_path / "target"),
            "--output",
            str(tmp_path / "report.json"),
            "--cargo-home",
            str(cargo_home),
            "--rustup-home",
            str(rustup_home),
            "--jobs",
            "0",
        ]
    )
    with pytest.raises(ValueError, match="greater than zero"):
        MODULE.measure(
            tmp_path,
            args.scenario,
            tmp_path,
            args.jobs,
            tmp_path / "cargo",
            {},
        )


def test_measure_uses_only_completed_child_rusage_without_process_observation(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    class Process:
        @staticmethod
        def wait() -> int:
            return 0

    cpu_samples = iter(((100.0, 50.0), (105.0, 52.0)))
    monotonic = iter((10.0, 12.0))
    popen_calls: list[tuple[list[str], dict[str, str]]] = []

    def popen(command: list[str], **kwargs: object) -> Process:
        popen_calls.append((command, kwargs["env"]))
        return Process()

    monkeypatch.setattr(MODULE.subprocess, "Popen", popen)
    monkeypatch.setattr(MODULE, "_child_cpu_seconds", lambda: next(cpu_samples))
    monkeypatch.setattr(MODULE.time, "monotonic", lambda: next(monotonic))

    environment = {"CARGO_TARGET_DIR": str(tmp_path)}
    cargo = tmp_path / "authenticated-cargo"
    measurement = MODULE.measure(
        tmp_path, "data-model", tmp_path, None, cargo, environment
    )

    assert measurement.user_cpu_seconds == pytest.approx(5.0)
    assert measurement.system_cpu_seconds == pytest.approx(2.0)
    assert popen_calls == [
        ([str(cargo), *MODULE.SCENARIOS["data-model"]], environment)
    ]
    assert "peak_process_tree_rss_bytes" not in MODULE.Measurement.__annotations__


def test_main_removes_private_state_after_successful_report(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fake_legacy_fixture(tmp_path, monkeypatch)
    fixture["target"].mkdir()
    (fixture["target"] / "warm-input").write_text("bound\n", encoding="utf-8")
    warm = MODULE.ISOLATION.bounded_tree_fingerprint(fixture["target"])
    source_before = (fixture["root"] / "source.rs").read_bytes()
    cache_before = MODULE.ISOLATION.bounded_tree_fingerprint(
        fixture["cargo_home"]
    )
    rustup_before = MODULE.ISOLATION.bounded_tree_fingerprint(
        fixture["rustup_home"]
    )

    returncode = MODULE.main(
        [
            "--root",
            str(fixture["root"]),
            "workspace",
            "--target-dir",
            str(fixture["target"]),
            "--reuse",
            "--output",
            str(fixture["output"]),
            "--cargo-home",
            str(fixture["cargo_home"]),
            "--rustup-home",
            str(fixture["rustup_home"]),
        ]
    )

    report = json.loads(fixture["output"].read_text(encoding="utf-8"))
    assert returncode == 0
    assert report["valid"] is True
    assert report["input"]["target_initial"] == MODULE.ISOLATION.tree_fingerprint_json(
        warm
    )
    assert (fixture["root"] / "source.rs").read_bytes() == source_before
    assert MODULE.ISOLATION.bounded_tree_fingerprint(
        fixture["cargo_home"]
    ) == cache_before
    assert MODULE.ISOLATION.bounded_tree_fingerprint(
        fixture["rustup_home"]
    ) == rustup_before
    assert not fixture["output"].with_suffix(".json.state").exists()
