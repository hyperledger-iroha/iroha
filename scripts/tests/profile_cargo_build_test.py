"""Unit tests for the reproducible Cargo build profiler."""

from __future__ import annotations

import copy
import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "profile_cargo_build.py"
SPEC = importlib.util.spec_from_file_location("profile_cargo_build", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
_PREVIOUS_DONT_WRITE_BYTECODE = sys.dont_write_bytecode
try:
    sys.dont_write_bytecode = True
    SPEC.loader.exec_module(MODULE)
finally:
    sys.dont_write_bytecode = _PREVIOUS_DONT_WRITE_BYTECODE


def _fake_profile_fixture(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    *,
    mutate_source_snapshot: bool = False,
) -> dict[str, Path]:
    """Create a Git/Cargo/rustc-free profiler fixture using executable fakes."""

    root = tmp_path / "repo"
    root.mkdir()
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    (root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
    (root / "source.rs").write_text("fn caller() {}\n", encoding="utf-8")

    cargo_home = tmp_path / "caller-cargo-home"
    (cargo_home / "registry").mkdir(parents=True)
    (cargo_home / "registry" / "seed").write_text("cache seed\n", encoding="utf-8")
    cargo_home.chmod(0o700)
    rustup_home = tmp_path / "caller-rustup-home"
    toolchain_bin = rustup_home / "toolchains" / "test" / "bin"
    toolchain_bin.mkdir(parents=True)
    rustup_home.chmod(0o700)

    source_action = (
        'chmod u+w "$PWD/source.rs"\nprintf "snapshot changed\\n" > "$PWD/source.rs"\n'
        if mutate_source_snapshot
        else ""
    )
    cargo = toolchain_bin / "cargo"
    cargo.write_text(
        "#!/bin/sh\n"
        'if [ "${1:-}" = "-Vv" ]; then printf "fake cargo 1.0\\n"; exit 0; fi\n'
        + source_action
        + 'printf "private cache write\\n" > "$CARGO_HOME/build-write"\n'
        + 'mkdir -p "$CARGO_TARGET_DIR/cargo-timings"\n'
        + 'printf "timing\\n" > "$CARGO_TARGET_DIR/cargo-timings/cargo-timing.html"\n'
        + "exit 0\n",
        encoding="utf-8",
    )
    rustc = toolchain_bin / "rustc"
    rustc.write_text(
        "#!/bin/sh\nprintf 'fake rustc 1.0\\n'\n",
        encoding="utf-8",
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
        "cargo_home": cargo_home,
        "rustup_home": rustup_home,
        "target": tmp_path / "target",
        "report": tmp_path / "reports" / "report.json",
    }


def test_normalized_cargo_args_adds_reproducible_defaults() -> None:
    """The profiler pins the lock, message stream, timings, and job count."""
    assert MODULE.normalized_cargo_args(["build", "--workspace"], 1) == [
        "build",
        "--locked",
        "--offline",
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
        "--offline",
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
        "--no-run",
        "--locked",
        "--offline",
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
        "--no-run",
        "--locked",
        "--offline",
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


def test_validate_paths_refuses_existing_or_target_nested_outputs(
    tmp_path: Path,
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    (root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    report = tmp_path / "report.json"
    report.write_text("retained\n", encoding="utf-8")
    with pytest.raises(ValueError, match="already exists"):
        MODULE.validate_paths(root, target, report, True)
    assert report.read_text(encoding="utf-8") == "retained\n"

    report.unlink()
    stderr_log = report.with_suffix(report.suffix + ".stderr.log")
    stderr_log.write_text("retained log\n", encoding="utf-8")
    with pytest.raises(ValueError, match="already exists"):
        MODULE.validate_paths(root, target, report, True)
    assert stderr_log.read_text(encoding="utf-8") == "retained log\n"

    stderr_log.unlink()
    with pytest.raises(ValueError, match="outside --target-dir"):
        MODULE.validate_paths(root, target, target / "report.json", True)


def test_validate_private_roots_requires_canonical_external_disjoint_directories(
    tmp_path: Path,
) -> None:
    root = tmp_path / "repo"
    target = tmp_path / "target"
    cargo_home = tmp_path / "cargo-home"
    rustup_home = tmp_path / "rustup-home"
    for directory in (root, target, cargo_home, rustup_home):
        directory.mkdir()
    cargo_home.chmod(0o700)
    rustup_home.chmod(0o700)
    out = tmp_path / "reports" / "report.json"
    assert MODULE.validate_private_roots(
        root, target, out, cargo_home, rustup_home
    ) == (cargo_home, rustup_home)
    nested_home = root / "cargo-home"
    nested_home.mkdir()
    nested_home.chmod(0o700)
    with pytest.raises(ValueError, match="external and disjoint"):
        MODULE.validate_private_roots(
            root, target, out, nested_home, rustup_home
        )


def test_report_bundle_reservation_never_replaces_existing_transcript(
    tmp_path: Path,
) -> None:
    out = tmp_path / "report.json"
    out.parent.mkdir(exist_ok=True)
    stderr_log = out.with_suffix(out.suffix + ".stderr.log")
    stderr_log.write_text("retained\n", encoding="utf-8")

    with pytest.raises(FileExistsError):
        MODULE.reserve_report_paths(out)

    assert stderr_log.read_text(encoding="utf-8") == "retained\n"
    assert not out.exists()
    assert not out.with_suffix(out.suffix + ".jsonl").exists()

    stderr_log.unlink()
    descriptors = MODULE.reserve_report_paths(out)
    identities = MODULE.reserved_report_identities(descriptors)
    for descriptor in descriptors:
        MODULE.os.close(descriptor)
    MODULE.remove_reserved_report_paths(out, identities)
    assert all(not path.exists() for path in MODULE.report_paths(out))

    state = MODULE.create_private_state(tmp_path / "private.state")
    (state.home / "owned").write_text("owned\n", encoding="utf-8")
    MODULE.remove_private_state(state)
    assert not state.root.exists()
    assert not any(
        path.name.startswith(".iroha-profile-cleanup-")
        for path in tmp_path.iterdir()
    )


@pytest.mark.parametrize(
    "cargo_args",
    (
        ["build", "--artifact-dir", "/tmp/escape"],
        ["build", "--lockfile-path=/tmp/escape.lock"],
        ["build", "--target-dir", "/tmp/escape"],
        ["build", "--target-dir=/tmp/escape"],
        ["build", "--config", "build.target-dir='/tmp/escape'"],
        ["build", "--config=build.target-dir='/tmp/escape'"],
    ),
)
def test_validate_cargo_controls_rejects_write_redirection(
    tmp_path: Path, cargo_args: list[str]
) -> None:
    with pytest.raises(ValueError, match="controlled by the profiler"):
        MODULE.validate_cargo_controls(
            MODULE.normalized_cargo_args(cargo_args, 1), tmp_path
        )


def test_validate_cargo_controls_confines_manifest_path(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    manifest = root / "member" / "Cargo.toml"
    manifest.parent.mkdir()
    manifest.write_text("[package]\nname='member'\n", encoding="utf-8")
    MODULE.validate_cargo_controls(
        MODULE.normalized_cargo_args(
            ["build", "--manifest-path", "member/Cargo.toml"], 1
        ),
        root,
    )
    with pytest.raises(ValueError, match="inside the repository"):
        MODULE.validate_cargo_controls(
            MODULE.normalized_cargo_args(
                ["build", "--manifest-path", str(tmp_path / "Cargo.toml")], 1
            ),
            root,
        )


def test_cargo_execution_args_maps_relative_manifest_into_snapshot(
    tmp_path: Path,
) -> None:
    root = tmp_path / "repo"
    manifest = root / "member" / "Cargo.toml"
    manifest.parent.mkdir(parents=True)
    manifest.write_text("[package]\nname='member'\n", encoding="utf-8")
    snapshot = tmp_path / "report.state" / "source"
    (snapshot / "member").mkdir(parents=True)
    (snapshot / "member" / "Cargo.toml").write_bytes(manifest.read_bytes())
    cargo_args = MODULE.normalized_cargo_args(
        ["build", "--manifest-path", "member/Cargo.toml"], 1
    )

    MODULE.validate_cargo_controls(cargo_args, root)
    execution = MODULE.cargo_execution_args(cargo_args, root, snapshot)

    value = execution[execution.index("--manifest-path") + 1]
    assert value == str(snapshot / "member" / "Cargo.toml")
    assert Path(value).is_relative_to(snapshot)
    assert not Path(value).is_relative_to(root)


@pytest.mark.parametrize(
    "target",
    (
        "../caller-target.json",
        "nested/spec.json",
        r"nested\spec.json",
        ".",
        "..",
    ),
)
def test_validate_cargo_controls_rejects_path_like_targets(
    tmp_path: Path, target: str
) -> None:
    with pytest.raises(ValueError, match="target triple"):
        MODULE.validate_cargo_controls(
            MODULE.normalized_cargo_args(["build", "--target", target], 1),
            tmp_path,
        )


@pytest.mark.parametrize("subcommand", ["clean", "install", "run", "vendor"])
def test_normalized_cargo_args_rejects_non_compile_subcommands(
    subcommand: str,
) -> None:
    with pytest.raises(ValueError, match="build, check, or test"):
        MODULE.normalized_cargo_args([subcommand], 1)


@pytest.mark.parametrize("option", ["--locked", "--offline"])
def test_validate_cargo_controls_requires_exact_single_policy_flag(
    tmp_path: Path, option: str
) -> None:
    args = MODULE.normalized_cargo_args(["build"], 1)
    args.insert(1, option)
    with pytest.raises(ValueError, match="exactly one"):
        MODULE.validate_cargo_controls(args, tmp_path)


def test_minimal_environment_is_closed_and_cargo_pinned(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    target = tmp_path / "target"
    cargo_home = tmp_path / "cargo-home"
    rustup_home = tmp_path / "rustup-home"
    state = tmp_path / "state"
    rustc = tmp_path / "rustc"
    for directory in (target, cargo_home, rustup_home, state / "home", state / "tmp"):
        directory.mkdir(parents=True)
    cargo_home.chmod(0o700)
    rustup_home.chmod(0o700)
    rustc.write_text("tool\n", encoding="utf-8")
    monkeypatch.setenv("UNREVIEWED_PROFILE_INPUT", "must-not-leak")
    monkeypatch.setenv("CARGO_TARGET_DIR", str(root / "escape"))
    monkeypatch.setenv("RUSTFLAGS", "must-not-leak")

    environment = MODULE.minimal_environment(
        root, root, target, 3, cargo_home, rustup_home, state, rustc
    )

    assert "UNREVIEWED_PROFILE_INPUT" not in environment
    assert "RUSTFLAGS" not in environment
    assert environment["CARGO_NET_OFFLINE"] == "true"
    assert environment["CARGO_TARGET_DIR"] == str(target)
    assert environment["CARGO_BUILD_JOBS"] == "3"
    assert environment["CARGO_HOME"] == str(cargo_home)
    assert environment["RUSTUP_HOME"] == str(rustup_home)
    assert environment["HOME"] == str(state / "home")
    assert environment["TMPDIR"] == str(state / "tmp")
    assert environment["RUSTC"] == str(rustc)
    assert environment["GIT_OPTIONAL_LOCKS"] == "0"
    assert environment["GIT_CONFIG_NOSYSTEM"] == "1"
    assert environment["GIT_CONFIG_GLOBAL"] == MODULE.os.devnull
    assert environment["GIT_CONFIG_COUNT"] == "5"
    assert environment["GIT_CONFIG_KEY_0"] == "core.fsmonitor"
    assert environment["GIT_CONFIG_VALUE_0"] == "false"
    assert environment["GIT_CONFIG_KEY_4"] == "core.pager"
    assert environment["GIT_CONFIG_VALUE_4"] == ""
    assert environment["GIT_PAGER"] == ""
    assert environment["GIT_TERMINAL_PROMPT"] == "0"


def test_source_fingerprint_is_order_independent_and_content_bound(
    tmp_path: Path,
) -> None:
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


def test_source_snapshot_is_inode_independent_and_preserves_dirty_deletion(
    tmp_path: Path,
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    dirty = root / "dirty.rs"
    dirty.write_text("dirty bytes\n", encoding="utf-8")
    snapshot = tmp_path / "snapshot"

    fingerprint = MODULE.capture_source_snapshot(
        root, ["deleted.rs", "dirty.rs"], snapshot
    )

    assert fingerprint.deleted == 1
    assert (snapshot / "dirty.rs").read_bytes() == dirty.read_bytes()
    assert (snapshot / "dirty.rs").stat().st_ino != dirty.stat().st_ino
    assert not (snapshot / "deleted.rs").exists()


def test_bounded_tree_copy_rejects_absolute_symlink(tmp_path: Path) -> None:
    source = tmp_path / "source"
    source.mkdir()
    outside = tmp_path / "outside"
    outside.write_text("outside\n", encoding="utf-8")
    (source / "escape").symlink_to(outside)

    with pytest.raises(ValueError, match="absolute target"):
        MODULE.copy_bounded_tree(source, tmp_path / "copy")


def test_bounded_tree_copy_rejects_relative_symlink_chain_escape(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    source.mkdir()
    outside = tmp_path / "outside"
    outside.mkdir()
    (outside / "payload").write_text("outside\n", encoding="utf-8")
    (source / "bridge").symlink_to("../outside", target_is_directory=True)
    (source / "escape").symlink_to("bridge/payload")

    with pytest.raises(ValueError, match="escapes its input root"):
        MODULE.copy_bounded_tree(source, tmp_path / "copy", roots=("escape",))


def test_bounded_tree_copy_never_writes_through_replaced_ancestor(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "source"
    (source / "nested").mkdir(parents=True)
    (source / "nested" / "payload").write_text("trusted\n", encoding="utf-8")
    destination = tmp_path / "copy"
    moved_destination = tmp_path / "moved-destination-child"
    outside = tmp_path / "outside"
    outside.mkdir()
    real_open = MODULE.os.open
    swapped = False

    def racing_open(path: object, flags: int, *args: object, **kwargs: object) -> int:
        nonlocal swapped
        if path == "payload" and flags & MODULE.os.O_WRONLY and not swapped:
            swapped = True
            (destination / "nested").rename(moved_destination)
            (destination / "nested").symlink_to(outside, target_is_directory=True)
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", racing_open)
    with pytest.raises((OSError, ValueError)):
        MODULE.copy_bounded_tree(source, destination)

    assert swapped is True
    assert not (outside / "payload").exists()
    assert (moved_destination / "payload").read_text(encoding="utf-8") == "trusted\n"


def test_source_snapshot_rejects_replaced_source_ancestor(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = tmp_path / "repo"
    (root / "src").mkdir(parents=True)
    (root / "src" / "lib.rs").write_text("fn trusted() {}\n", encoding="utf-8")
    moved_source = tmp_path / "moved-source"
    snapshot = tmp_path / "snapshot"
    real_read = MODULE._read_regular_stable_at
    reads = 0

    def racing_read(
        parent_fd: int,
        name: str,
        metadata: object,
        display: object,
    ) -> bytes:
        nonlocal reads
        if Path(display) == root / "src" / "lib.rs":
            reads += 1
            if reads == 2:
                (root / "src").rename(moved_source)
                (root / "src").symlink_to(
                    moved_source, target_is_directory=True
                )
        return real_read(parent_fd, name, metadata, display)

    monkeypatch.setattr(MODULE, "_read_regular_stable_at", racing_read)
    with pytest.raises((OSError, ValueError)):
        MODULE.capture_source_snapshot(root, ["src/lib.rs"], snapshot)

    assert reads >= 2
    assert not snapshot.exists()
    assert (moved_source / "lib.rs").read_text(encoding="utf-8") == (
        "fn trusted() {}\n"
    )


def test_failed_tree_copy_never_deletes_replacement_directory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "source"
    source.mkdir()
    (source / "payload").write_text("trusted\n", encoding="utf-8")
    destination = tmp_path / "copy"
    saved_owned_copy = tmp_path / "saved-owned-copy"
    caller_directory = tmp_path / "caller-directory"
    caller_directory.mkdir()
    (caller_directory / "sentinel").write_text("must survive\n", encoding="utf-8")
    real_read = MODULE._read_regular_stable_at
    reads = 0

    def racing_read(
        parent_fd: int,
        name: str,
        metadata: object,
        display: object,
    ) -> bytes:
        nonlocal reads
        if Path(display) == source / "payload":
            reads += 1
            if reads == 2:
                destination.rename(saved_owned_copy)
                caller_directory.rename(destination)
                raise ValueError("forced failure after destination replacement")
        return real_read(parent_fd, name, metadata, display)

    monkeypatch.setattr(MODULE, "_read_regular_stable_at", racing_read)
    with pytest.raises(ValueError, match="forced failure"):
        MODULE.copy_bounded_tree(source, destination)

    assert (destination / "sentinel").read_text(encoding="utf-8") == "must survive\n"
    assert saved_owned_copy.is_dir()


def test_report_and_state_cleanup_never_delete_replacements(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    out = tmp_path / "report.json"
    descriptors = MODULE.reserve_report_paths(out)
    identities = MODULE.reserved_report_identities(descriptors)
    for descriptor in descriptors:
        MODULE.os.close(descriptor)
    caller_file = tmp_path / "caller-file"
    caller_file.write_text("caller file survives\n", encoding="utf-8")
    saved_report = tmp_path / "saved-owned-report"

    state = MODULE.create_private_state(tmp_path / "report.state")
    (state.home / "private").write_text("state\n", encoding="utf-8")
    caller_directory = tmp_path / "caller-directory"
    caller_directory.mkdir()
    (caller_directory / "sentinel").write_text(
        "caller directory survives\n", encoding="utf-8"
    )
    saved_state = tmp_path / "saved-owned-state"
    real_rename = MODULE.os.rename
    real_rename_noreplace = MODULE._rename_noreplace_at
    report_swapped = False
    state_swapped = False
    rollback_collided = False

    def racing_rename(
        source: object, destination: object, *args: object, **kwargs: object
    ) -> None:
        nonlocal report_swapped, state_swapped
        if source == state.name and not state_swapped:
            state_swapped = True
            real_rename(state.root, saved_state)
            real_rename(caller_directory, state.root)
        real_rename(source, destination, *args, **kwargs)

    real_link = MODULE.os.link

    def racing_link(
        source: object, destination: object, *args: object, **kwargs: object
    ) -> None:
        nonlocal report_swapped
        if source == out.name and not report_swapped:
            report_swapped = True
            real_rename(out, saved_report)
            real_rename(caller_file, out)
        real_link(source, destination, *args, **kwargs)

    def racing_rename_noreplace(
        parent_fd: int, source: str, destination: str
    ) -> None:
        nonlocal rollback_collided
        if destination == state.name and not rollback_collided:
            rollback_collided = True
            MODULE.os.mkdir(destination, 0o700, dir_fd=parent_fd)
            collision = MODULE.os.open(
                destination, MODULE._DIRECTORY_FLAGS, dir_fd=parent_fd
            )
            try:
                sentinel = MODULE.os.open(
                    "collision-sentinel",
                    MODULE.os.O_WRONLY
                    | MODULE.os.O_CREAT
                    | MODULE.os.O_EXCL,
                    0o600,
                    dir_fd=collision,
                )
                MODULE.os.close(sentinel)
            finally:
                MODULE.os.close(collision)
        real_rename_noreplace(parent_fd, source, destination)

    monkeypatch.setattr(MODULE.os, "rename", racing_rename)
    monkeypatch.setattr(MODULE.os, "link", racing_link)
    monkeypatch.setattr(MODULE, "_rename_noreplace_at", racing_rename_noreplace)
    MODULE.remove_reserved_report_paths(out, identities)
    with pytest.raises(ValueError, match="replacement retained as") as caught:
        MODULE.remove_private_state(state)

    assert report_swapped is True
    assert state_swapped is True
    assert rollback_collided is True
    assert saved_report.is_file()
    assert saved_state.is_dir()
    assert out.read_text(encoding="utf-8") == "caller file survives\n"
    assert (state.root / "collision-sentinel").is_file()
    retained_name = (
        str(caught.value).split("retained as '", 1)[1].split("'", 1)[0]
    )
    retained = tmp_path / retained_name
    assert (retained / "sentinel").read_text(encoding="utf-8") == (
        "caller directory survives\n"
    )
    assert [
        path
        for path in tmp_path.iterdir()
        if path.name.startswith(".iroha-profile-cleanup-")
    ] == [retained]


def test_report_cleanup_rolls_back_a_swap_at_quarantine_rename(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    out = tmp_path / "report.json"
    descriptors = MODULE.reserve_report_paths(out)
    identities = MODULE.reserved_report_identities(descriptors)
    for descriptor in descriptors:
        MODULE.os.close(descriptor)
    caller_file = tmp_path / "caller-file"
    caller_file.write_text("caller survives at output\n", encoding="utf-8")
    saved_owned_report = tmp_path / "saved-owned-report"
    real_rename = MODULE.os.rename
    swapped = False

    def racing_rename(
        source: object, destination: object, *args: object, **kwargs: object
    ) -> None:
        nonlocal swapped
        if source == out.name and not swapped:
            swapped = True
            real_rename(out, saved_owned_report)
            real_rename(caller_file, out)
        real_rename(source, destination, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "rename", racing_rename)
    with pytest.raises(ValueError, match="replacement restored"):
        MODULE.remove_reserved_report_paths(out, identities)

    assert swapped is True
    assert out.read_text(encoding="utf-8") == "caller survives at output\n"
    assert saved_owned_report.is_file()
    assert not any(
        path.name.startswith(".iroha-profile-cleanup-")
        for path in tmp_path.iterdir()
    )


def test_report_cleanup_restores_swap_at_atomic_final_removal(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    out = tmp_path / "report.json"
    descriptors = MODULE.reserve_report_paths(out)
    identities = MODULE.reserved_report_identities(descriptors)
    for descriptor in descriptors:
        MODULE.os.close(descriptor)
    foreign = tmp_path / "foreign"
    foreign.write_text("must survive\n", encoding="utf-8")
    saved_owned = tmp_path / "saved-owned"
    real_swap = MODULE._rename_swap_at
    swapped = False

    def racing_swap(parent_fd: int, left: str, right: str) -> None:
        nonlocal swapped
        if not swapped:
            swapped = True
            MODULE.os.rename(
                left,
                saved_owned.name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
            MODULE.os.rename(
                foreign.name,
                left,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
        real_swap(parent_fd, left, right)

    monkeypatch.setattr(MODULE, "_rename_swap_at", racing_swap)
    with pytest.raises(ValueError, match="foreign entry restored"):
        MODULE._remove_owned_path(out, identities[0])

    assert swapped is True
    assert not foreign.exists()
    quarantines = [
        path
        for path in tmp_path.iterdir()
        if path.name.startswith(".iroha-profile-cleanup-")
    ]
    assert len(quarantines) == 1
    assert quarantines[0].read_text(encoding="utf-8") == "must survive\n"
    assert saved_owned.is_file()


def test_writable_tree_rejects_hardlinks_to_caller_inputs(tmp_path: Path) -> None:
    caller = tmp_path / "caller.rs"
    caller.write_text("caller\n", encoding="utf-8")
    target = tmp_path / "target"
    target.mkdir()
    target_link = target / "artifact"
    MODULE.os.link(caller, target_link)

    with pytest.raises(ValueError, match="hard-linked"):
        MODULE.validate_writable_tree(target, label="--target-dir")

    cache = tmp_path / "cargo-home"
    (cache / "registry").mkdir(parents=True)
    cache_link = cache / "registry" / "seed"
    MODULE.os.link(caller, cache_link)
    with pytest.raises(ValueError, match="hard-linked"):
        MODULE.copy_bounded_tree(
            cache,
            tmp_path / "private-cache",
            roots=("registry",),
            reject_source_hardlinks=True,
        )
    assert caller.read_text(encoding="utf-8") == "caller\n"


def test_cargo_controls_never_disclose_original_caller_paths(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    cargo_home = tmp_path / "cargo-home"
    rustup_home = tmp_path / "rustup-home"
    cargo_home.mkdir()
    rustup_home.mkdir()
    caller_target = root / "caller-target.json"
    caller_target.write_text("{}\n", encoding="utf-8")
    root_alias = tmp_path / "repo-alias"
    root_alias.symlink_to(root, target_is_directory=True)

    with pytest.raises(ValueError, match="target triple"):
        MODULE.validate_cargo_controls(
            MODULE.normalized_cargo_args(
                ["build", "--target", str(caller_target)], 1
            ),
            root,
            (cargo_home, rustup_home),
        )
    with pytest.raises(ValueError, match="must not disclose"):
        MODULE.validate_cargo_controls(
            MODULE.normalized_cargo_args(
                ["build", f"--features=leak={cargo_home / 'secret'}"], 1
            ),
            root,
            (cargo_home, rustup_home),
        )
    with pytest.raises(ValueError, match="must not disclose"):
        MODULE.validate_cargo_controls(
            MODULE.normalized_cargo_args(
                ["build", f"--features={root_alias / 'caller-target.json'}"],
                1,
            ),
            root,
            (cargo_home, rustup_home),
        )
    assert caller_target.exists()


def test_git_inventory_uses_closed_config_and_external_cwd(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = tmp_path / "repo"
    root.mkdir()
    safe_cwd = tmp_path / "safe"
    safe_cwd.mkdir()
    observed: dict[str, object] = {}

    def check_output(command: list[str], **kwargs: object) -> bytes:
        observed["command"] = command
        observed["cwd"] = kwargs["cwd"]
        return b"Cargo.toml\0"

    monkeypatch.setattr(MODULE.subprocess, "check_output", check_output)
    paths = MODULE.tracked_and_untracked_paths(
        root, {"PATH": "/bin"}, "/fake/git", safe_cwd
    )

    assert paths == ["Cargo.toml"]
    assert observed["cwd"] == safe_cwd
    command = observed["command"]
    assert "--no-pager" in command
    assert "--no-optional-locks" in command
    assert "core.fsmonitor=false" in command
    assert "core.hooksPath=/dev/null" in command
    assert "core.untrackedCache=false" in command
    assert f"core.excludesFile={MODULE.os.devnull}" in command
    assert command[command.index("-C") + 1] == str(root)


def test_verify_isolated_tool_binds_path_original_and_copy(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    original = tmp_path / "original"
    invoked = tmp_path / "private"
    original.write_bytes(b"tool-v1")
    invoked.write_bytes(b"tool-v1")
    original.chmod(0o700)
    invoked.chmod(0o700)
    digest = MODULE.sha256_bytes(b"tool-v1")
    identity = {
        "discovered_path": str(original),
        "launcher_path": str(original),
        "launcher_sha256": digest,
        "resolved_path": str(original),
        "invoked_path": str(invoked),
        "sha256": digest,
    }
    monkeypatch.setattr(MODULE.shutil, "which", lambda *_args, **_kwargs: str(original))

    MODULE.verify_isolated_tool("cargo", identity, str(tmp_path))
    original.write_bytes(b"tool-v2")
    with pytest.raises(ValueError, match="launcher changed"):
        MODULE.verify_isolated_tool("cargo", identity, str(tmp_path))


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
        ("selected_env", {"SOURCE_DATE_EPOCH": "1"}),
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


def test_main_isolates_source_cache_and_rustup_and_cleans_state(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A fake Cargo sees only copies; its source/cache writes cannot reach inputs."""

    fixture = _fake_profile_fixture(
        tmp_path, monkeypatch, mutate_source_snapshot=True
    )
    source = fixture["root"] / "source.rs"
    caller_source = source.read_bytes()
    caller_cache = MODULE.bounded_tree_fingerprint(fixture["cargo_home"])
    caller_rustup = MODULE.bounded_tree_fingerprint(fixture["rustup_home"])

    returncode = MODULE.main(
        [
            "--root",
            str(fixture["root"]),
            "--target-dir",
            str(fixture["target"]),
            "--out",
            str(fixture["report"]),
            "--cargo-home",
            str(fixture["cargo_home"]),
            "--rustup-home",
            str(fixture["rustup_home"]),
            "--",
            "build",
        ]
    )

    report = json.loads(fixture["report"].read_text(encoding="utf-8"))
    assert returncode == MODULE.INPUT_DRIFT_EXIT_CODE
    assert report["schema_version"] == 3
    assert report["valid"] is False
    assert report["result"]["returncode"] == 0
    assert report["input_validation"]["stable"] is False
    assert report["input_validation"]["changed_fields"] == ["execution_source"]
    assert report["input_sha256"] != report["input_validation"]["post_input_sha256"]
    assert source.read_bytes() == caller_source
    assert MODULE.bounded_tree_fingerprint(fixture["cargo_home"]) == caller_cache
    assert MODULE.bounded_tree_fingerprint(fixture["rustup_home"]) == caller_rustup
    assert not MODULE.private_state_path(fixture["report"]).exists()


def test_main_binds_warm_target_and_accepts_private_cache_writes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fake_profile_fixture(tmp_path, monkeypatch)
    fixture["target"].mkdir()
    (fixture["target"] / "warm-artifact").write_text("warm input\n", encoding="utf-8")
    warm = MODULE.bounded_tree_fingerprint(fixture["target"])
    caller_cache = MODULE.bounded_tree_fingerprint(fixture["cargo_home"])
    caller_rustup = MODULE.bounded_tree_fingerprint(fixture["rustup_home"])

    returncode = MODULE.main(
        [
            "--root",
            str(fixture["root"]),
            "--target-dir",
            str(fixture["target"]),
            "--reuse-target",
            "--out",
            str(fixture["report"]),
            "--cargo-home",
            str(fixture["cargo_home"]),
            "--rustup-home",
            str(fixture["rustup_home"]),
            "--",
            "build",
        ]
    )

    report = json.loads(fixture["report"].read_text(encoding="utf-8"))
    assert returncode == 0
    assert report["valid"] is True
    assert report["input"]["profile_mode"] == "warm"
    assert report["input"]["target_initial"] == MODULE.tree_fingerprint_json(warm)
    assert MODULE.bounded_tree_fingerprint(fixture["cargo_home"]) == caller_cache
    assert MODULE.bounded_tree_fingerprint(fixture["rustup_home"]) == caller_rustup
    assert not MODULE.private_state_path(fixture["report"]).exists()


def test_main_cleans_every_owned_path_when_late_manifest_resolution_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fake_profile_fixture(tmp_path, monkeypatch)
    manifest = fixture["root"] / "Cargo.toml"
    real_reserve = MODULE.reserve_report_paths

    def reserve_then_remove_manifest(out: Path) -> tuple[int, int, int]:
        descriptors = real_reserve(out)
        manifest.unlink()
        return descriptors

    monkeypatch.setattr(MODULE, "reserve_report_paths", reserve_then_remove_manifest)
    returncode = MODULE.main(
        [
            "--root",
            str(fixture["root"]),
            "--target-dir",
            str(fixture["target"]),
            "--out",
            str(fixture["report"]),
            "--cargo-home",
            str(fixture["cargo_home"]),
            "--rustup-home",
            str(fixture["rustup_home"]),
            "--",
            "build",
            "--manifest-path",
            str(manifest),
        ]
    )

    assert returncode == 2
    assert not MODULE.private_state_path(fixture["report"]).exists()
    assert all(not path.exists() for path in MODULE.report_paths(fixture["report"]))
