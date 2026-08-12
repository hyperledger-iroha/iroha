#!/usr/bin/env python3
"""Measure reproducible Iroha build scenarios in an isolated target directory.

The profiler never runs ``cargo clean`` and refuses to reuse a non-empty target
directory unless ``--reuse`` is explicit. This keeps performance measurements
from destroying or contaminating a developer's normal Cargo cache.

Reported resource usage comes only from the completed Cargo child tree. The
profiler does not inspect the host process table.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import resource
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Sequence


_ISOLATION_PATH = Path(__file__).resolve().with_name("profile_cargo_build.py")
_ISOLATION_SPEC = importlib.util.spec_from_file_location(
    "_iroha_profile_cargo_build_isolation", _ISOLATION_PATH
)
assert _ISOLATION_SPEC is not None and _ISOLATION_SPEC.loader is not None
if _ISOLATION_SPEC.name in sys.modules:
    ISOLATION = sys.modules[_ISOLATION_SPEC.name]
else:
    ISOLATION = importlib.util.module_from_spec(_ISOLATION_SPEC)
    sys.modules[_ISOLATION_SPEC.name] = ISOLATION
    _previous_dont_write_bytecode = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        _ISOLATION_SPEC.loader.exec_module(ISOLATION)
    finally:
        sys.dont_write_bytecode = _previous_dont_write_bytecode


SCENARIOS: dict[str, tuple[str, ...]] = {
    "workspace": ("build", "--locked", "--offline", "--workspace", "--timings"),
    "data-model": (
        "build",
        "--locked",
        "--offline",
        "-p",
        "iroha_data_model",
        "--lib",
        "--timings",
    ),
    "daemon": (
        "build",
        "--locked",
        "--offline",
        "-p",
        "irohad",
        "--bin",
        "iroha3d",
        "--timings",
    ),
    "cli": (
        "build",
        "--locked",
        "--offline",
        "-p",
        "iroha_cli",
        "--bin",
        "iroha",
        "--timings",
    ),
}

_PASSTHROUGH_ENV_KEYS = (
    "PATH",
    "SYSTEMROOT",
)


@dataclass(frozen=True)
class Measurement:
    """One completed Cargo invocation and its child-rusage measurements."""

    scenario: str
    command: list[str]
    target_dir: str
    elapsed_seconds: float
    user_cpu_seconds: float
    system_cpu_seconds: float
    target_bytes: int
    return_code: int


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (default: inferred from this script).",
    )
    parser.add_argument(
        "scenario",
        choices=sorted(SCENARIOS),
        help="Build surface to measure.",
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        required=True,
        help="Dedicated Cargo target directory; it is never deleted.",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        help="Explicit Cargo job count; omit to measure Cargo's default jobserver.",
    )
    parser.add_argument(
        "--cargo-home",
        type=Path,
        required=True,
        help="Canonical external, caller-private Cargo cache root.",
    )
    parser.add_argument(
        "--rustup-home",
        type=Path,
        required=True,
        help="Canonical external, caller-private Rustup toolchain root.",
    )
    parser.add_argument(
        "--reuse",
        action="store_true",
        help="Allow a non-empty target directory for warm/no-op measurements.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Absent external path for the JSON result.",
    )
    return parser.parse_args(argv)


def resolve_inside(path: Path, parent: Path) -> bool:
    """Return whether ``path`` resolves inside ``parent``."""

    try:
        path.resolve().relative_to(parent.resolve())
    except ValueError:
        return False
    return True


def paths_overlap(left: Path, right: Path) -> bool:
    """Return whether either resolved path contains the other."""

    return resolve_inside(left, right) or resolve_inside(right, left)


def validate_target_dir(path: Path, *, root: Path, reuse: bool) -> Path:
    """Create and validate an isolated target directory without deleting data."""
    root = root.resolve()
    resolved = path.resolve()
    if resolved == Path(resolved.anchor):
        raise ValueError("target directory must not be a filesystem root")
    if paths_overlap(resolved, root):
        raise ValueError("target directory must be outside the repository")
    try:
        descriptor = ISOLATION._open_directory_anchored(resolved)
    except FileNotFoundError:
        descriptor = ISOLATION._open_directory_anchored(resolved, create=True)
    try:
        if ISOLATION._directory_names_fd(descriptor) and not reuse:
            raise ValueError(
                f"target directory is not empty: {resolved}; "
                "pass --reuse for a warm build"
            )
    finally:
        os.close(descriptor)
    return resolved


def validate_output_path(path: Path, root: Path, target_dir: Path) -> Path:
    """Return an absent external report/state bundle disjoint from the target."""

    unresolved = path.absolute()
    resolved = path.resolve()
    state = resolved.with_suffix(resolved.suffix + ".state")
    unresolved_state = unresolved.with_suffix(unresolved.suffix + ".state")
    if paths_overlap(resolved, root) or paths_overlap(state, root):
        raise ValueError("output path must be outside the repository")
    if paths_overlap(resolved, target_dir) or paths_overlap(state, target_dir):
        raise ValueError("output path must be outside the target directory")
    for candidate in (unresolved, unresolved_state):
        if os.path.lexists(candidate):
            raise ValueError(f"output path already exists: {candidate}")
    return resolved


def validate_private_roots(
    root: Path,
    target_dir: Path,
    cargo_home: Path,
    rustup_home: Path,
    forbidden_roots: Sequence[Path] = (),
) -> tuple[Path, Path]:
    """Validate explicit canonical cache/toolchain roots outside all inputs."""

    validated = []
    for label, path in (("--cargo-home", cargo_home), ("--rustup-home", rustup_home)):
        if not path.is_absolute() or path != path.resolve():
            raise ValueError(f"{label} must be an absolute canonical path")
        if not path.is_dir() or path.is_symlink():
            raise ValueError(f"{label} must be a non-symlink directory")
        metadata = path.stat()
        if stat.S_IMODE(metadata.st_mode) & 0o077:
            raise ValueError(f"{label} must be caller-private (mode 0700)")
        if hasattr(os, "geteuid") and metadata.st_uid != os.geteuid():
            raise ValueError(f"{label} must be owned by the current user")
        if (
            paths_overlap(path, root)
            or paths_overlap(path, target_dir)
            or any(paths_overlap(path, forbidden) for forbidden in forbidden_roots)
        ):
            raise ValueError(f"{label} must be external and disjoint")
        validated.append(path)
    if paths_overlap(*validated):
        raise ValueError("--cargo-home and --rustup-home must be disjoint")
    return validated[0], validated[1]


def resolve_tool(
    name: str,
    root: Path,
    search_path: str,
    environment: dict[str, str],
    forbidden_roots: Sequence[Path] = (),
) -> tuple[Path, Path, str, Path, str]:
    """Resolve and hash the actual executable used for one tool command."""

    found = shutil.which(name, path=search_path)
    if found is None:
        raise ValueError(f"required tool is not executable on PATH: {name}")
    discovered = Path(found).absolute()
    launcher = discovered.resolve(strict=True)
    launcher_metadata = launcher.stat()
    if (
        not stat.S_ISREG(launcher_metadata.st_mode)
        or not os.access(launcher, os.X_OK)
        or resolve_inside(launcher, root)
        or any(resolve_inside(launcher, path) for path in forbidden_roots)
    ):
        raise ValueError(f"{name} launcher must be an external regular executable")
    launcher_digest = hashlib.sha256(
        ISOLATION._read_regular_stable(launcher, launcher_metadata)
    ).hexdigest()
    executable = launcher
    if name in ("cargo", "rustc") and ISOLATION._is_rustup_proxy(
        discovered, launcher, search_path, launcher_digest
    ):
        try:
            safe_cwd = Path(environment["HOME"]).resolve(strict=True)
        except (KeyError, OSError) as error:
            raise ValueError(
                "tool identity resolution requires a private HOME"
            ) from error
        if not safe_cwd.is_dir() or resolve_inside(safe_cwd, root):
            raise ValueError("tool identity HOME must be an external directory")
        selected = subprocess.check_output(
            [str(launcher), "which", name],
            cwd=safe_cwd,
            env=environment,
            text=True,
        ).strip()
        if not selected:
            raise ValueError(f"rustup returned an empty executable path for {name}")
        selected_path = Path(selected)
        if not selected_path.is_absolute():
            raise ValueError(f"rustup returned a non-absolute path for {name}")
        executable = selected_path.resolve(strict=True)
    metadata = executable.stat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or not os.access(executable, os.X_OK)
        or resolve_inside(executable, root)
        or any(resolve_inside(executable, path) for path in forbidden_roots)
    ):
        raise ValueError(f"{name} must resolve to an external regular executable")
    digest = hashlib.sha256(
        ISOLATION._read_regular_stable(executable, metadata)
    ).hexdigest()
    return discovered, executable, digest, launcher, launcher_digest


def create_private_state(parent: Path) -> Path:
    """Create invocation-private HOME and temporary directories."""

    state = Path(tempfile.mkdtemp(prefix=".iroha-profile-state-", dir=parent))
    state.chmod(0o700)
    for name in ("home", "tmp"):
        directory = state / name
        directory.mkdir(mode=0o700)
    return state


def base_environment(
    root: Path,
    cargo_home: Path,
    rustup_home: Path,
    state_dir: Path,
) -> dict[str, str]:
    """Return the closed environment used to resolve and run profile tools."""

    return ISOLATION.base_environment(root, cargo_home, rustup_home, state_dir)


def minimal_environment(
    root: Path,
    target_dir: Path,
    cargo_home: Path,
    rustup_home: Path,
    state_dir: Path,
    rustc: Path,
) -> dict[str, str]:
    """Return the closed environment used by profiler child commands."""

    return ISOLATION.minimal_environment(
        root,
        root,
        target_dir,
        None,
        cargo_home,
        rustup_home,
        state_dir,
        rustc,
    )


def reserve_report(path: Path) -> int:
    """Reserve an absent report path before starting Cargo."""

    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    parent_fd, name = ISOLATION._open_parent_anchored(path, create=True)
    try:
        return os.open(name, flags, 0o600, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)


def directory_size(path: Path) -> int:
    """Return the total size of regular files below ``path`` without following links."""
    root = path.absolute()
    root_fd = ISOLATION._open_directory_anchored(root)

    def size(directory_fd: int) -> int:
        total = 0
        for name in ISOLATION._directory_names_fd(directory_fd):
            metadata = ISOLATION._lstat_at(directory_fd, name)
            if stat.S_ISREG(metadata.st_mode):
                total += metadata.st_size
            elif stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(
                metadata.st_mode
            ):
                child = ISOLATION._open_child_directory_at(
                    directory_fd, name, metadata, display=root / name
                )
                try:
                    total += size(child)
                finally:
                    os.close(child)
        return total

    try:
        return size(root_fd)
    finally:
        os.close(root_fd)


def _child_cpu_seconds() -> tuple[float, float]:
    """Return cumulative user and system CPU for reaped child processes."""
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime, usage.ru_stime


def measure(
    source_root: Path,
    scenario: str,
    target_dir: Path,
    jobs: int | None,
    cargo: Path,
    environment: dict[str, str],
) -> Measurement:
    """Run and measure one Cargo scenario."""
    command = [str(cargo), *SCENARIOS[scenario]]
    if jobs is not None:
        if jobs <= 0:
            raise ValueError("--jobs must be greater than zero")
        command.extend(("--jobs", str(jobs)))

    before_user_cpu, before_system_cpu = _child_cpu_seconds()
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=source_root, env=environment)
    return_code = process.wait()
    elapsed = time.monotonic() - started
    after_user_cpu, after_system_cpu = _child_cpu_seconds()

    return Measurement(
        scenario=scenario,
        command=command,
        target_dir=str(target_dir),
        elapsed_seconds=elapsed,
        user_cpu_seconds=max(0.0, after_user_cpu - before_user_cpu),
        system_cpu_seconds=max(0.0, after_system_cpu - before_system_cpu),
        target_bytes=directory_size(target_dir),
        return_code=return_code,
    )


def render_report(
    root: Path,
    measurement: Measurement,
    cargo_identity: tuple[Path, Path, str, Path, str] | dict[str, str],
    rustc_identity: tuple[Path, Path, str, Path, str] | dict[str, str],
    git_identity: tuple[Path, Path, str, Path, str] | dict[str, str],
    environment: dict[str, str],
    revision: str | None = None,
    rustc_version: str | None = None,
    isolation_input: dict[str, Any] | None = None,
    input_validation: dict[str, Any] | None = None,
) -> dict[str, object]:
    """Add reproducibility metadata to a measurement."""
    if isinstance(cargo_identity, dict):
        cargo_tool = ISOLATION.public_tool_identity(cargo_identity)
        rustc_tool = ISOLATION.public_tool_identity(rustc_identity)
        git_tool = ISOLATION.public_tool_identity(git_identity)
    else:
        cargo_tool = {
            "discovered_path": str(cargo_identity[0]),
            "resolved_path": str(cargo_identity[1]),
            "sha256": cargo_identity[2],
            "launcher_path": str(cargo_identity[3]),
            "launcher_sha256": cargo_identity[4],
        }
        assert not isinstance(rustc_identity, dict)
        assert not isinstance(git_identity, dict)
        rustc_tool = {
            "discovered_path": str(rustc_identity[0]),
            "resolved_path": str(rustc_identity[1]),
            "sha256": rustc_identity[2],
            "launcher_path": str(rustc_identity[3]),
            "launcher_sha256": rustc_identity[4],
        }
        git_tool = {
            "discovered_path": str(git_identity[0]),
            "resolved_path": str(git_identity[1]),
            "sha256": git_identity[2],
            "launcher_path": str(git_identity[3]),
            "launcher_sha256": git_identity[4],
        }
    if revision is None or rustc_version is None:
        raise ValueError("closed revision and rustc identities must be pre-captured")
    return {
        "schema_version": 2,
        "valid": measurement.return_code == 0
        and (input_validation is None or bool(input_validation.get("stable"))),
        "cargo_tool": cargo_tool,
        "git_revision": revision,
        "git_tool": git_tool,
        "input": isolation_input,
        "input_validation": input_validation,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "rustc": rustc_version,
        "rustc_tool": rustc_tool,
        "measurement": asdict(measurement),
    }


def _source_json(value: object) -> dict[str, object]:
    return {
        "bytes": value.bytes,
        "deleted": value.deleted,
        "files": value.files,
        "sha256": value.sha256,
    }


def _legacy_input_identity(
    *,
    scenario: str,
    jobs: int | None,
    environment: dict[str, str],
    source: object,
    revision: str,
    cargo_cache: object,
    rustup_tree: object,
    target_initial: object,
    execution_source: object,
    private_cargo_input: object,
    private_rustup_input: object,
    cargo_identity: dict[str, str],
    rustc_identity: dict[str, str],
    git_identity: dict[str, str],
    cargo_version: str,
    rustc_version: str,
    path_identity: str | None = None,
) -> dict[str, object]:
    """Return the legacy wrapper's closed, comparable input identity."""

    return {
        "cargo_args": [
            *SCENARIOS[scenario],
            *([] if jobs is None else ["--jobs", str(jobs)]),
        ],
        "cargo_cache": ISOLATION.tree_fingerprint_json(cargo_cache),
        "execution_source": ISOLATION.tree_fingerprint_json(execution_source),
        "git_revision": revision,
        "jobs": jobs,
        "path": path_identity if path_identity is not None else environment["PATH"],
        "private_cargo_input": ISOLATION.tree_fingerprint_json(private_cargo_input),
        "private_rustup_input": ISOLATION.tree_fingerprint_json(private_rustup_input),
        "rustup_tree": ISOLATION.tree_fingerprint_json(rustup_tree),
        "scenario": scenario,
        "source": _source_json(source),
        "target_initial": ISOLATION.tree_fingerprint_json(target_initial),
        "toolchain": {
            "cargo": {
                **ISOLATION.public_tool_identity(cargo_identity),
                "version": cargo_version,
            },
            "git": ISOLATION.public_tool_identity(git_identity),
            "rustc": {
                **ISOLATION.public_tool_identity(rustc_identity),
                "version": rustc_version,
            },
        },
    }


def main(argv: Sequence[str] | None = None) -> int:
    """Run the selected measurement and emit its JSON report."""
    args = parse_args(argv)
    root = args.root.resolve()
    output: Path | None = None
    state: object | None = None
    report_descriptor: int | None = None
    report_reserved = False
    report_complete = False
    report_identity: tuple[int, int] | None = None
    try:
        if not (root / "Cargo.toml").is_file() or not (root / "Cargo.lock").is_file():
            raise ValueError(f"repository root is incomplete: {root}")
        target_dir = validate_target_dir(args.target_dir, root=root, reuse=args.reuse)
        output = validate_output_path(args.output, root, target_dir)
        state_path = output.with_suffix(output.suffix + ".state")
        cargo_home, rustup_home = validate_private_roots(
            root,
            target_dir,
            args.cargo_home,
            args.rustup_home,
            (output, state_path),
        )
        caller_root_identity = ISOLATION._directory_identity(root)
        caller_cargo_identity = ISOLATION._directory_identity(cargo_home)
        caller_rustup_identity = ISOLATION._directory_identity(rustup_home)
        output_parent_descriptor = ISOLATION._open_directory_anchored(
            output.parent, create=True
        )
        os.close(output_parent_descriptor)
        state = ISOLATION.create_private_state(state_path)
        cargo_cache_input = ISOLATION.copy_bounded_tree(
            cargo_home,
            state.cargo_home,
            roots=("git", "registry"),
            expected_source_identity=caller_cargo_identity,
            reject_source_hardlinks=True,
        )
        rustup_input = ISOLATION.copy_bounded_tree(
            rustup_home,
            state.rustup_home,
            expected_source_identity=caller_rustup_identity,
            reject_source_hardlinks=True,
        )
        discovery_environment = ISOLATION.base_environment(
            root, state.cargo_home, state.rustup_home, state.root
        )
        search_path = discovery_environment["PATH"]
        ISOLATION.validate_search_path_disjoint(
            search_path,
            (root, cargo_home, rustup_home, target_dir, state.root),
        )
        forbidden_tools = (target_dir, state.root)
        git_identity = ISOLATION.resolve_isolated_tool(
            "git",
            root,
            search_path,
            discovery_environment,
            state,
            rustup_home,
            state.home,
            (*forbidden_tools, cargo_home, rustup_home),
        )
        ISOLATION.validate_git_worktree(
            root,
            discovery_environment,
            ISOLATION.tool_invocation_path(git_identity),
            state.home,
        )
        source_paths = ISOLATION.tracked_and_untracked_paths(
            root,
            discovery_environment,
            ISOLATION.tool_invocation_path(git_identity),
            state.home,
        )
        if not ISOLATION._path_still_names(root, caller_root_identity):
            raise ValueError("repository source root changed during Git inventory")
        source_input = ISOLATION.capture_source_snapshot(
            root,
            source_paths,
            state.source,
            expected_root_identity=caller_root_identity,
            reject_source_hardlinks=True,
        )
        revision = ISOLATION.closed_git_revision(
            root,
            discovery_environment,
            ISOLATION.tool_invocation_path(git_identity),
            state.home,
        )
        if not ISOLATION._path_still_names(root, caller_root_identity):
            raise ValueError(
                "repository source root changed during Git revision read"
            )
        cargo_identity = ISOLATION.resolve_isolated_tool(
            "cargo",
            root,
            search_path,
            discovery_environment,
            state,
            rustup_home,
            state.source,
            forbidden_tools,
        )
        rustc_identity = ISOLATION.resolve_isolated_tool(
            "rustc",
            root,
            search_path,
            discovery_environment,
            state,
            rustup_home,
            state.source,
            forbidden_tools,
        )
        cargo_version = ISOLATION.command_output(
            [ISOLATION.tool_invocation_path(cargo_identity), "-Vv"],
            state.home,
            discovery_environment,
        )
        rustc_version = ISOLATION.command_output(
            [ISOLATION.tool_invocation_path(rustc_identity), "-Vv"],
            state.home,
            discovery_environment,
        )
        environment = ISOLATION.minimal_environment(
            root,
            state.source,
            target_dir,
            args.jobs,
            state.cargo_home,
            state.rustup_home,
            state.root,
            Path(ISOLATION.tool_invocation_path(rustc_identity)),
        )
        environment["IROHA_GIT_COMMIT_HASH"] = revision
        environment["VERGEN_GIT_SHA"] = revision
        ISOLATION.expose_private_tools(
            state,
            {"cargo": cargo_identity, "git": git_identity, "rustc": rustc_identity},
            environment,
        )
        target_initial = ISOLATION.validate_writable_tree(
            target_dir, label="target directory"
        )
        private_cargo_input = ISOLATION.validate_writable_tree(
            state.cargo_home, label="private Cargo cache"
        )
        private_rustup_input = ISOLATION.validate_writable_tree(
            state.rustup_home, label="private Rustup tree"
        )
        execution_source_input = ISOLATION.bounded_tree_fingerprint(
            state.source, reject_hardlinks=True
        )
        ISOLATION.make_source_read_only(state.source)
        input_identity = _legacy_input_identity(
            scenario=args.scenario,
            jobs=args.jobs,
            environment=environment,
            source=source_input,
            revision=revision,
            cargo_cache=cargo_cache_input,
            rustup_tree=rustup_input,
            target_initial=target_initial,
            execution_source=execution_source_input,
            private_cargo_input=private_cargo_input,
            private_rustup_input=private_rustup_input,
            cargo_identity=cargo_identity,
            rustc_identity=rustc_identity,
            git_identity=git_identity,
            cargo_version=cargo_version,
            rustc_version=rustc_version,
            path_identity=search_path,
        )
        report_descriptor = reserve_report(output)
        report_reserved = True
        report_metadata = os.fstat(report_descriptor)
        report_identity = (report_metadata.st_dev, report_metadata.st_ino)
        measurement = measure(
            state.source,
            args.scenario,
            target_dir,
            args.jobs,
            Path(ISOLATION.tool_invocation_path(cargo_identity)),
            environment,
        )
        ISOLATION.verify_isolated_tool("cargo", cargo_identity, search_path)
        ISOLATION.verify_isolated_tool("rustc", rustc_identity, search_path)
        ISOLATION.verify_isolated_tool("git", git_identity, search_path)
        ISOLATION.validate_git_worktree(
            root,
            environment,
            ISOLATION.tool_invocation_path(git_identity),
            state.home,
        )
        post_source_paths = ISOLATION.tracked_and_untracked_paths(
            root,
            environment,
            ISOLATION.tool_invocation_path(git_identity),
            state.home,
        )
        post_source = ISOLATION.source_fingerprint(
            root,
            post_source_paths,
            expected_identity=caller_root_identity,
            reject_hardlinks=True,
        )
        post_revision = ISOLATION.closed_git_revision(
            root,
            environment,
            ISOLATION.tool_invocation_path(git_identity),
            state.home,
        )
        post_input = _legacy_input_identity(
            scenario=args.scenario,
            jobs=args.jobs,
            environment=environment,
            source=post_source,
            revision=post_revision,
            cargo_cache=ISOLATION.bounded_tree_fingerprint(
                cargo_home,
                ("git", "registry"),
                expected_identity=caller_cargo_identity,
                reject_hardlinks=True,
            ),
            rustup_tree=ISOLATION.bounded_tree_fingerprint(
                rustup_home,
                expected_identity=caller_rustup_identity,
                reject_hardlinks=True,
            ),
            target_initial=target_initial,
            execution_source=ISOLATION.bounded_tree_fingerprint(
                state.source, reject_hardlinks=True
            ),
            private_cargo_input=private_cargo_input,
            private_rustup_input=ISOLATION.validate_writable_tree(
                state.rustup_home, label="private Rustup tree"
            ),
            cargo_identity=cargo_identity,
            rustc_identity=rustc_identity,
            git_identity=git_identity,
            cargo_version=ISOLATION.command_output(
                [ISOLATION.tool_invocation_path(cargo_identity), "-Vv"],
                state.home,
                discovery_environment,
            ),
            rustc_version=ISOLATION.command_output(
                [ISOLATION.tool_invocation_path(rustc_identity), "-Vv"],
                state.home,
                discovery_environment,
            ),
            path_identity=search_path,
        )
        changed_fields = ISOLATION.changed_input_fields(input_identity, post_input)
        input_validation = {
            "changed_fields": changed_fields,
            "post_input": post_input,
            "stable": not changed_fields,
        }
        ISOLATION.remove_private_state(state)
        state = None
        report_measurement = Measurement(
            scenario=measurement.scenario,
            command=[cargo_identity["resolved_path"], *measurement.command[1:]],
            target_dir=measurement.target_dir,
            elapsed_seconds=measurement.elapsed_seconds,
            user_cpu_seconds=measurement.user_cpu_seconds,
            system_cpu_seconds=measurement.system_cpu_seconds,
            target_bytes=measurement.target_bytes,
            return_code=measurement.return_code,
        )
        report = render_report(
            root,
            report_measurement,
            cargo_identity,
            rustc_identity,
            git_identity,
            environment,
            revision,
            rustc_version,
            input_identity,
            input_validation,
        )
        rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
        assert report_descriptor is not None
        assert report_identity is not None
        if not ISOLATION._path_still_names(output, report_identity):
            raise ValueError("reserved output path was replaced")
        with os.fdopen(report_descriptor, "w", encoding="utf-8") as report_file:
            report_descriptor = None
            report_file.write(rendered)
        if not ISOLATION._path_still_names(output, report_identity):
            raise ValueError("reserved output path was replaced while written")
        report_complete = True
        print(f"wrote build profile to {output}")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        if report_descriptor is not None:
            os.close(report_descriptor)
            report_descriptor = None
        if report_reserved and not report_complete and output is not None:
            if report_identity is not None:
                try:
                    ISOLATION._remove_owned_path(output, report_identity)
                except (OSError, ValueError) as cleanup:
                    print(
                        f"ERROR: report cleanup failed: {cleanup}",
                        file=sys.stderr,
                    )
        if state is not None:
            try:
                ISOLATION.remove_private_state(state)
            except (OSError, ValueError) as cleanup:
                print(
                    f"ERROR: private state cleanup failed: {cleanup}",
                    file=sys.stderr,
                )
        print(f"ERROR: build profiling failed: {error}", file=sys.stderr)
        return 2
    finally:
        if report_descriptor is not None:
            os.close(report_descriptor)
        if state is not None:
            try:
                ISOLATION.remove_private_state(state)
            except (OSError, ValueError):
                pass
    if measurement.return_code == 0 and not input_validation["stable"]:
        return ISOLATION.INPUT_DRIFT_EXIT_CODE
    return measurement.return_code


if __name__ == "__main__":
    raise SystemExit(main())
