#!/usr/bin/env python3
"""Regenerate the platform-independent current Kotodama artifact fixture.

The canonical fixture contains only source-bound semantic data.  A fresh koto
binary is copied into a private cache stage before it is executed; its local
identity is recorded in a cache-only attestation and never enters the checked-in
JSON.  The existing Rust admission test consumes the resulting fixture and is
the authoritative executable-policy oracle, so this owner does not compile an
ad-hoc verifier or accept rustc/rlib inputs.

Write mode creates the canonical JSON directly with O_EXCL in a caller-created,
owner-only cache stage.  It never publishes to the repository and never unlinks
residue or a file that another process may have replaced.
"""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import stat
import subprocess
import sys
from typing import Any, Iterable, Mapping, Sequence

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 CI fallback
    import tomli as tomllib


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIRECTORY = Path("javascript/iroha_js/test/fixtures")
SOURCE_PATH = FIXTURE_DIRECTORY / "current_rust_contract_artifact.ko"
FIXTURE_PATH = FIXTURE_DIRECTORY / "current_rust_contract_artifact.json"
GENERATOR_PATH = Path("scripts/regenerate_current_rust_contract_artifact.py")
ROOT_MANIFEST_PATH = Path("Cargo.toml")
ROOT_INPUTS = (
    ROOT_MANIFEST_PATH,
    Path("rust-toolchain.toml"),
    Path(".cargo/config.toml"),
    SOURCE_PATH,
    GENERATOR_PATH,
    Path("javascript/iroha_js/src/blake2b.js"),
    Path("javascript/iroha_js/src/ivmArtifact.js"),
    Path("javascript/iroha_js/src/kotodamaCompiler/normalize.js"),
)
ROOT_PACKAGES = (Path("crates/ivm"),)
# Package tests, examples, benches, fuzzers, and prose do not participate in a
# normal koto/verifier build. Excluding those developer-only trees keeps the
# semantic closure exact and prevents unrelated fixture churn.
NON_BUILD_PACKAGE_TREES = frozenset({"tests", "examples", "benches", "fuzz", "docs"})
SOURCE_CLOSURE_DOMAIN = b"iroha-current-rust-contract-artifact-source-v2\0"
CONTRACT_HASH_DOMAIN = b"iroha:ivm:contract-artifact:v1\0"
IVM_HEADER_BYTES = 49
MAX_ARTIFACT_BYTES = 4 * 1024 * 1024
HASH_LITERAL = re.compile(r"^hash:([0-9A-F]{64})#[0-9A-F]{4}$")
OUTPUT_STAGE_NAME = re.compile(
    r"^current-rust-contract-artifact\.(?!work\.)(?:[A-Za-z0-9][A-Za-z0-9_-]{5,63})$"
)
WORK_STAGE_PREFIX = "current-rust-contract-artifact.work."


class FixtureError(RuntimeError):
    """Raised when exact-current fixture generation cannot be proved."""


@dataclass(frozen=True)
class FileSnapshot:
    """Stable bytes and inode identity for one regular file."""

    path: Path
    label: str
    data: bytes
    sha256: str
    size: int
    device: int
    inode: int
    mode: int
    modified_ns: int
    changed_ns: int


@dataclass
class BoundDirectory:
    """A non-symbolic directory held open for relative no-follow operations."""

    path: Path
    label: str
    descriptor: int
    device: int
    inode: int
    mode: int
    owner: int

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _git_blob_id(data: bytes) -> str:
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data).hexdigest()


def _snapshot_file(path: Path, label: str, *, executable: bool = False) -> FileSnapshot:
    """Read a regular leaf through O_NOFOLLOW and prove the name stayed bound."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = -1
    try:
        named_before = path.lstat()
        if stat.S_ISLNK(named_before.st_mode) or not stat.S_ISREG(named_before.st_mode):
            raise FixtureError(f"{label} must be a non-symbolic regular file: {path}")
        if executable and named_before.st_mode & 0o111 == 0:
            raise FixtureError(f"{label} is not executable: {path}")
        descriptor = os.open(path, flags)
        opened_before = os.fstat(descriptor)
        if (opened_before.st_dev, opened_before.st_ino) != (
            named_before.st_dev,
            named_before.st_ino,
        ):
            raise FixtureError(f"{label} changed while it was opened: {path}")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
        opened_after = os.fstat(descriptor)
        named_after = path.lstat()
    except FixtureError:
        raise
    except OSError as error:
        raise FixtureError(f"failed to snapshot {label} {path}: {error}") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)

    def identity(metadata: os.stat_result) -> tuple[int, int, int, int, int, int]:
        return (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_size,
            stat.S_IMODE(metadata.st_mode),
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        )

    if identity(opened_before) != identity(opened_after) or identity(opened_after) != identity(named_after):
        raise FixtureError(f"{label} changed while it was read: {path}")
    data = b"".join(chunks)
    if len(data) != opened_after.st_size:
        raise FixtureError(f"{label} changed length while it was read: {path}")
    return FileSnapshot(
        path=path,
        label=label,
        data=data,
        sha256=_sha256(data),
        size=len(data),
        device=opened_after.st_dev,
        inode=opened_after.st_ino,
        mode=stat.S_IMODE(opened_after.st_mode),
        modified_ns=opened_after.st_mtime_ns,
        changed_ns=opened_after.st_ctime_ns,
    )


def _authenticate_file(snapshot: FileSnapshot) -> None:
    current = _snapshot_file(snapshot.path, snapshot.label)
    expected = (
        snapshot.sha256,
        snapshot.size,
        snapshot.device,
        snapshot.inode,
        snapshot.mode,
        snapshot.modified_ns,
        snapshot.changed_ns,
    )
    actual = (
        current.sha256,
        current.size,
        current.device,
        current.inode,
        current.mode,
        current.modified_ns,
        current.changed_ns,
    )
    if actual != expected:
        raise FixtureError(f"{snapshot.label} changed after it was snapshotted")


def _canonical_absolute(raw: Path, label: str) -> Path:
    if not raw.is_absolute():
        raise FixtureError(f"{label} must be absolute: {raw}")
    if raw != Path(os.path.normpath(raw)):
        raise FixtureError(f"{label} must be lexically canonical: {raw}")
    return raw


def _resolve_input_file(raw: Path, label: str, *, executable: bool = False) -> Path:
    candidate = raw if raw.is_absolute() else REPOSITORY_ROOT / raw
    candidate = Path(os.path.abspath(candidate))
    try:
        metadata = candidate.lstat()
    except OSError as error:
        raise FixtureError(f"{label} is unavailable: {candidate}: {error}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise FixtureError(f"{label} must be a non-symbolic regular file: {candidate}")
    if executable and metadata.st_mode & 0o111 == 0:
        raise FixtureError(f"{label} is not executable: {candidate}")
    return candidate


def _bind_directory(path: Path, label: str, *, exact_mode: int | None = None) -> BoundDirectory:
    path = _canonical_absolute(path, label)
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = -1
    try:
        named = path.lstat()
        if stat.S_ISLNK(named.st_mode) or not stat.S_ISDIR(named.st_mode):
            raise FixtureError(f"{label} must be a non-symbolic directory: {path}")
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino):
            raise FixtureError(f"{label} changed while it was opened")
        mode = stat.S_IMODE(opened.st_mode)
        if opened.st_uid != os.geteuid():
            raise FixtureError(f"{label} must be owned by the current user")
        if exact_mode is not None and mode != exact_mode:
            raise FixtureError(f"{label} must have exact mode {exact_mode:04o}")
        if mode & 0o022:
            raise FixtureError(f"{label} must not be group- or world-writable")
        return BoundDirectory(
            path=path,
            label=label,
            descriptor=descriptor,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=mode,
            owner=opened.st_uid,
        )
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise


def _authenticate_directory(directory: BoundDirectory) -> None:
    try:
        opened = os.fstat(directory.descriptor)
        named = directory.path.lstat()
    except OSError as error:
        raise FixtureError(f"{directory.label} became unavailable: {error}") from error
    expected = (directory.device, directory.inode, directory.mode, directory.owner)
    for metadata in (opened, named):
        actual = (
            metadata.st_dev,
            metadata.st_ino,
            stat.S_IMODE(metadata.st_mode),
            metadata.st_uid,
        )
        if (
            actual != expected
            or stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
        ):
            raise FixtureError(f"{directory.label} changed after it was bound")


def _create_work_stage(cache_root: BoundDirectory) -> BoundDirectory:
    """Create a unique private stage and deliberately leave all residue in cache."""

    for _attempt in range(64):
        name = f"{WORK_STAGE_PREFIX}{secrets.token_hex(12)}"
        try:
            os.mkdir(name, 0o700, dir_fd=cache_root.descriptor)
        except FileExistsError:
            continue
        stage = _bind_directory(cache_root.path / name, "generator work stage", exact_mode=0o700)
        _authenticate_directory(cache_root)
        return stage
    raise FixtureError("failed to allocate a unique generator work stage")


def _prepare_sealed_inputs(
    stage: BoundDirectory,
    koto_path: Path,
    git_path: Path,
) -> tuple[BoundDirectory, FileSnapshot, FileSnapshot, FileSnapshot]:
    """Copy every pathname input into a random read/execute-only directory."""

    koto = _snapshot_file(koto_path, "input koto", executable=True)
    git = _snapshot_file(git_path, "input Git", executable=True)
    source = _snapshot_file(REPOSITORY_ROOT / SOURCE_PATH, "input Kotodama source")
    os.mkdir("sealed-inputs", 0o700, dir_fd=stage.descriptor)
    writable = _bind_directory(
        stage.path / "sealed-inputs",
        "writable tool input stage",
        exact_mode=0o700,
    )
    try:
        source_copy = _write_new_file(
            writable,
            SOURCE_PATH.name,
            source.data,
            mode=0o400,
        )
        koto_copy = _write_new_file(writable, "koto", koto.data, mode=0o500)
        git_copy = _write_new_file(writable, "git", git.data, mode=0o500)
    finally:
        writable.close()
    try:
        os.chmod(stage.path / "sealed-inputs", 0o500, follow_symlinks=False)
    except OSError as error:
        raise FixtureError(f"failed to seal private tool input stage: {error}") from error
    sealed = _bind_directory(
        stage.path / "sealed-inputs",
        "sealed tool input stage",
        exact_mode=0o500,
    )
    for snapshot in (source_copy, koto_copy, git_copy):
        _authenticate_file(snapshot)
    return sealed, source_copy, koto_copy, git_copy


def _bind_output(cache_root: BoundDirectory, raw: Path) -> BoundDirectory:
    output = _canonical_absolute(raw, "write output")
    if output.name != FIXTURE_PATH.name:
        raise FixtureError(f"write output must use canonical name {FIXTURE_PATH.name}")
    if output.parent.parent != cache_root.path:
        raise FixtureError("write output must be directly below the explicit cache root")
    if OUTPUT_STAGE_NAME.fullmatch(output.parent.name) is None:
        raise FixtureError(
            "write output parent must be current-rust-contract-artifact.<unique>"
        )
    stage = _bind_directory(output.parent, "write output stage", exact_mode=0o700)
    try:
        _authenticate_directory(cache_root)
        try:
            os.stat(output.name, dir_fd=stage.descriptor, follow_symlinks=False)
        except FileNotFoundError:
            return stage
        raise FixtureError("write output already exists; use a fresh canonical file name")
    except BaseException:
        stage.close()
        raise


def _write_all(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise FixtureError("cache-staged write did not progress")
        view = view[written:]


def _write_new_file(
    directory: BoundDirectory,
    name: str,
    data: bytes,
    *,
    mode: int,
) -> FileSnapshot:
    """Create one final-name inode; on any race, fail without unlinking anything."""

    if not name or name in {".", ".."} or "/" in name or os.altsep and os.altsep in name:
        raise FixtureError(f"cache-staged file name is not canonical: {name!r}")
    _authenticate_directory(directory)
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = -1
    opened: os.stat_result | None = None
    try:
        descriptor = os.open(name, flags, mode, dir_fd=directory.descriptor)
        opened = os.fstat(descriptor)
        _write_all(descriptor, data)
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        opened = os.fstat(descriptor)
    except FileExistsError as error:
        raise FixtureError(f"cache-staged output already exists: {directory.path / name}") from error
    except FixtureError:
        raise
    except OSError as error:
        raise FixtureError(f"failed to create cache-staged file {name}: {error}") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)

    if opened is None:  # pragma: no cover - successful open invariant
        raise FixtureError("cache-staged output was not opened")
    _authenticate_directory(directory)
    snapshot = _snapshot_file(directory.path / name, f"cache-staged {name}")
    if (
        snapshot.device,
        snapshot.inode,
        snapshot.size,
        snapshot.mode,
        snapshot.sha256,
    ) != (
        opened.st_dev,
        opened.st_ino,
        len(data),
        mode,
        _sha256(data),
    ):
        raise FixtureError(
            f"cache-staged file {name} was replaced or changed during publication"
        )
    return snapshot


def _hermetic_environment(stage: BoundDirectory) -> dict[str, str]:
    """Return the complete, minimal environment inherited by external tools."""

    home = stage.path / "hermetic-home"
    temporary = stage.path / "hermetic-tmp"
    home.mkdir(mode=0o700)
    temporary.mkdir(mode=0o700)
    return {
        "HOME": os.fspath(home),
        "TMPDIR": os.fspath(temporary),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "PATH": "/usr/bin:/bin",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "RUST_BACKTRACE": "0",
        "SOURCE_DATE_EPOCH": "0",
    }


def _run(
    command: Sequence[os.PathLike[str] | str],
    *,
    environment: Mapping[str, str],
    pass_fds: Sequence[int] = (),
) -> subprocess.CompletedProcess[str]:
    rendered = [os.fspath(argument) for argument in command]
    try:
        return subprocess.run(
            rendered,
            cwd=REPOSITORY_ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=dict(environment),
            close_fds=True,
            pass_fds=tuple(pass_fds),
        )
    except subprocess.CalledProcessError as error:
        detail = error.stderr.strip() or error.stdout.strip() or "no diagnostic output"
        raise FixtureError(
            f"command failed ({error.returncode}): {' '.join(rendered)}\n{detail}"
        ) from error
    except OSError as error:
        raise FixtureError(f"failed to run {' '.join(rendered)}: {error}") from error


def _open_snapshot(snapshot: FileSnapshot) -> int:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(snapshot.path, flags)
        metadata = os.fstat(descriptor)
    except OSError as error:
        raise FixtureError(f"failed to open {snapshot.label}: {error}") from error
    if (metadata.st_dev, metadata.st_ino, metadata.st_size) != (
        snapshot.device,
        snapshot.inode,
        snapshot.size,
    ):
        os.close(descriptor)
        raise FixtureError(f"{snapshot.label} changed while its descriptor was opened")
    return descriptor


def _authenticate_descriptor(snapshot: FileSnapshot, descriptor: int) -> None:
    try:
        before = os.fstat(descriptor)
        digest = hashlib.sha256()
        offset = 0
        while offset < snapshot.size:
            chunk = os.pread(descriptor, min(1024 * 1024, snapshot.size - offset), offset)
            if not chunk:
                break
            digest.update(chunk)
            offset += len(chunk)
        after = os.fstat(descriptor)
    except OSError as error:
        raise FixtureError(f"failed to authenticate open {snapshot.label}: {error}") from error
    expected = (snapshot.device, snapshot.inode, snapshot.size, snapshot.mode)
    for metadata in (before, after):
        actual = (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_size,
            stat.S_IMODE(metadata.st_mode),
        )
        if actual != expected or not stat.S_ISREG(metadata.st_mode):
            raise FixtureError(f"open {snapshot.label} changed during execution")
    if offset != snapshot.size or digest.hexdigest() != snapshot.sha256:
        raise FixtureError(f"open {snapshot.label} content changed during execution")


def _run_bound_executable(
    executable: FileSnapshot,
    arguments: Sequence[os.PathLike[str] | str],
    *,
    environment: Mapping[str, str],
    inherited_fds: Sequence[int] = (),
) -> tuple[subprocess.CompletedProcess[str], str]:
    """Run a held executable FD on Linux or a sealed private path on Darwin."""

    descriptor = _open_snapshot(executable)
    try:
        _authenticate_descriptor(executable, descriptor)
        _authenticate_file(executable)
        pass_fds = [descriptor, *inherited_fds]
        if sys.platform.startswith("linux"):
            invocation = Path(f"/proc/self/fd/{descriptor}")
            binding = "linux-proc-self-fd"
        else:
            # Darwin exposes no executable fexecve/execveat API and rejects
            # executable /dev/fd paths. The random input directory is sealed
            # read/execute-only and its inode is authenticated around spawn.
            invocation = executable.path
            binding = "darwin-sealed-private-path"
        result = _run(
            (invocation, *arguments),
            environment=environment,
            pass_fds=pass_fds,
        )
        _authenticate_descriptor(executable, descriptor)
        _authenticate_file(executable)
        return result, binding
    finally:
        os.close(descriptor)


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise FixtureError(f"JSON contains duplicate object key {key!r}")
        result[key] = value
    return result


def _parse_json_snapshot(snapshot: FileSnapshot) -> tuple[Any, str]:
    """Decode and strict-parse the exact bytes held by one file snapshot."""

    try:
        text = snapshot.data.decode("utf-8")
        return json.loads(text, object_pairs_hook=_strict_object), text
    except FixtureError:
        raise
    except (UnicodeError, json.JSONDecodeError) as error:
        raise FixtureError(
            f"failed to parse {snapshot.label} {snapshot.path}: {error}"
        ) from error


def _load_json_strict(path: Path, label: str) -> Any:
    snapshot = _snapshot_file(path, label)
    value, _text = _parse_json_snapshot(snapshot)
    _authenticate_file(snapshot)
    return value


def _tracked_paths(
    git: FileSnapshot,
    environment: Mapping[str, str],
) -> tuple[frozenset[Path], str]:
    result, binding = _run_bound_executable(
        git,
        (
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "core.quotePath=false",
            "ls-files",
            "-z",
        ),
        environment=environment,
    )
    try:
        names = result.stdout.split("\0")
    except UnicodeError as error:
        raise FixtureError(f"tracked source path is not UTF-8: {error}") from error
    return frozenset(Path(name) for name in names if name), binding


def _toml_document(snapshot: FileSnapshot) -> dict[str, Any]:
    try:
        value = tomllib.loads(snapshot.data.decode("utf-8"))
    except (UnicodeError, tomllib.TOMLDecodeError) as error:
        raise FixtureError(f"failed to parse {snapshot.label}: {error}") from error
    if not isinstance(value, dict):  # pragma: no cover - tomllib invariant
        raise FixtureError(f"{snapshot.label} is not a TOML table")
    return value


def _dependency_tables(manifest: dict[str, Any]) -> Iterable[dict[str, Any]]:
    for name in ("dependencies", "build-dependencies"):
        table = manifest.get(name, {})
        if not isinstance(table, dict):
            raise FixtureError(f"Cargo manifest {name} must be a table")
        yield table
    target = manifest.get("target", {})
    if not isinstance(target, dict):
        raise FixtureError("Cargo manifest target must be a table")
    for target_value in target.values():
        if not isinstance(target_value, dict):
            raise FixtureError("Cargo manifest target entry must be a table")
        for name in ("dependencies", "build-dependencies"):
            table = target_value.get(name, {})
            if not isinstance(table, dict):
                raise FixtureError(f"Cargo target {name} must be a table")
            yield table


def _package_source_closure(tracked: frozenset[Path]) -> tuple[dict[Path, FileSnapshot], set[Path]]:
    """Resolve every local build dependency from Cargo manifests without Cargo."""

    root_manifest = _snapshot_file(REPOSITORY_ROOT / ROOT_MANIFEST_PATH, "root Cargo manifest")
    root = _toml_document(root_manifest)
    workspace = root.get("workspace")
    if not isinstance(workspace, dict):
        raise FixtureError("root Cargo.toml has no workspace table")
    workspace_dependencies = workspace.get("dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        raise FixtureError("workspace.dependencies must be a table")

    patch = root.get("patch", {})
    if not isinstance(patch, dict):
        raise FixtureError("root Cargo patch must be a table")
    patched_packages: list[Path] = []
    for registry, declarations in patch.items():
        if not isinstance(declarations, dict):
            raise FixtureError(f"Cargo patch registry {registry!r} must be a table")
        for package_name, declaration in declarations.items():
            if not isinstance(declaration, dict) or "path" not in declaration:
                continue
            raw_path = declaration["path"]
            if not isinstance(raw_path, str) or not raw_path:
                raise FixtureError(f"patched package {package_name!r} has invalid path")
            resolved = Path(os.path.normpath(raw_path))
            if resolved.is_absolute() or ".." in resolved.parts:
                raise FixtureError(f"patched package {package_name!r} escapes the repository")
            patched_packages.append(resolved)

    manifest_snapshots: dict[Path, FileSnapshot] = {ROOT_MANIFEST_PATH: root_manifest}
    package_directories: set[Path] = set()
    # Every local patch is included, not only the subset currently selected by
    # one host's Cargo resolution. This keeps the tracked source boundary
    # platform-independent while still following each patch's local deps.
    pending = [*ROOT_PACKAGES, *patched_packages]
    while pending:
        package = pending.pop()
        if package in package_directories:
            continue
        if package.is_absolute() or ".." in package.parts:
            raise FixtureError(f"local package escapes the repository: {package}")
        manifest_relative = package / "Cargo.toml"
        if manifest_relative not in tracked:
            raise FixtureError(f"local package manifest is not tracked: {manifest_relative}")
        snapshot = _snapshot_file(
            REPOSITORY_ROOT / manifest_relative,
            f"Cargo manifest {manifest_relative}",
        )
        manifest_snapshots[manifest_relative] = snapshot
        manifest = _toml_document(snapshot)
        package_directories.add(package)
        for table in _dependency_tables(manifest):
            for dependency_name, declaration in table.items():
                candidate = declaration
                base = package
                if isinstance(candidate, dict) and candidate.get("workspace") is True:
                    candidate = workspace_dependencies.get(dependency_name)
                    base = Path()
                    if candidate is None:
                        raise FixtureError(
                            f"workspace dependency {dependency_name!r} has no root declaration"
                        )
                if not isinstance(candidate, dict) or "path" not in candidate:
                    continue
                raw_path = candidate["path"]
                if not isinstance(raw_path, str) or not raw_path:
                    raise FixtureError(f"local dependency {dependency_name!r} has invalid path")
                resolved = Path(os.path.normpath(base / raw_path))
                if resolved.is_absolute() or ".." in resolved.parts:
                    raise FixtureError(
                        f"local dependency {dependency_name!r} escapes the repository"
                    )
                pending.append(resolved)
    return manifest_snapshots, package_directories


def _source_closure(
    git: FileSnapshot,
    environment: Mapping[str, str],
) -> tuple[dict[Path, FileSnapshot], str, str]:
    tracked_before, binding_before = _tracked_paths(git, environment)
    manifest_snapshots, package_directories = _package_source_closure(tracked_before)
    paths = set(ROOT_INPUTS)
    paths.update(manifest_snapshots)
    for relative in tracked_before:
        for package in package_directories:
            if relative == package or package not in relative.parents:
                continue
            package_relative = relative.relative_to(package)
            if package_relative.parts and package_relative.parts[0] in NON_BUILD_PACKAGE_TREES:
                continue
            paths.add(relative)
            break
    missing = sorted(path for path in paths if path not in tracked_before)
    if missing:
        raise FixtureError(
            "source closure contains untracked inputs: " + ", ".join(map(os.fspath, missing))
        )

    snapshots: dict[Path, FileSnapshot] = {}
    for relative in sorted(paths, key=os.fspath):
        snapshots[relative] = manifest_snapshots.get(relative) or _snapshot_file(
            REPOSITORY_ROOT / relative,
            f"source closure {relative}",
        )

    digest = hashlib.sha256(SOURCE_CLOSURE_DOMAIN)
    for relative, snapshot in snapshots.items():
        encoded_path = os.fspath(relative).encode("utf-8")
        digest.update(len(encoded_path).to_bytes(4, "big"))
        digest.update(encoded_path)
        digest.update(snapshot.size.to_bytes(8, "big"))
        digest.update(snapshot.data)

    tracked_after, binding_after = _tracked_paths(git, environment)
    if tracked_after != tracked_before:
        raise FixtureError("tracked source inventory changed during generation")
    if binding_after != binding_before:
        raise FixtureError("Git executable binding changed during generation")
    for snapshot in snapshots.values():
        _authenticate_file(snapshot)
    return snapshots, digest.hexdigest(), binding_before


def _manifest_hash(manifest: dict[str, Any], field: str) -> str:
    value = manifest.get(field)
    if not isinstance(value, str):
        raise FixtureError(f"compiler manifest has no string {field}")
    match = HASH_LITERAL.fullmatch(value)
    if match is None:
        raise FixtureError(f"compiler manifest has noncanonical {field}: {value!r}")
    return match.group(1).lower()


def _contract_hash(artifact: bytes) -> str:
    digest = bytearray(hashlib.blake2b(CONTRACT_HASH_DOMAIN + artifact, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()


def _u32_le(data: bytes, offset: int, label: str) -> int:
    end = offset + 4
    if end > len(data):
        raise FixtureError(f"generated artifact has truncated {label}")
    return int.from_bytes(data[offset:end], "little")


def _semantic_expectation(artifact: bytes, manifest: dict[str, Any]) -> dict[str, str | int]:
    """Derive deterministic expectations that the Rust fixture test must prove."""

    if not (IVM_HEADER_BYTES <= len(artifact) <= MAX_ARTIFACT_BYTES):
        raise FixtureError("generated artifact has an invalid byte length")
    if artifact[:4] != b"IVM\0" or artifact[4:6] != bytes((1, 1)):
        raise FixtureError("generated artifact is not a canonical IVM 1.1 program")
    if artifact[6] & ~0x03 or artifact[7] > 64 or artifact[16] != 1:
        raise FixtureError("generated artifact has unsupported execution metadata")

    abi_hash = _manifest_hash(manifest, "abi_hash")
    if artifact[17:IVM_HEADER_BYTES].hex() != abi_hash:
        raise FixtureError("generated artifact header and manifest ABI hashes differ")
    code_hash = _manifest_hash(manifest, "code_hash")
    if _contract_hash(artifact) != code_hash:
        raise FixtureError("generated artifact and manifest code hashes differ")

    offset = IVM_HEADER_BYTES
    if artifact[offset : offset + 4] != b"CNTR":
        raise FixtureError("generated artifact is missing the required CNTR section")
    interface_length = _u32_le(artifact, offset + 4, "CNTR length")
    if interface_length == 0:
        raise FixtureError("generated artifact has an empty CNTR section")
    offset += 8 + interface_length
    if offset > len(artifact):
        raise FixtureError("generated artifact CNTR section exceeds its bounds")
    if artifact[offset : offset + 4] == b"DBG1":
        raise FixtureError("deployable artifact must not contain DBG1 metadata")
    if artifact[offset : offset + 4] == b"LTLB":
        start = offset
        count = _u32_le(artifact, start + 4, "LTLB count")
        padding = _u32_le(artifact, start + 8, "LTLB padding")
        data_length = _u32_le(artifact, start + 12, "LTLB data length")
        if count > 0x10000 or padding > 3:
            raise FixtureError("generated artifact LTLB bounds are invalid")
        entries_length = count * 8
        data_start = start + 16 + entries_length
        data_end = data_start + data_length
        offset = data_end + padding
        expected_padding = (4 - ((start - IVM_HEADER_BYTES + 16 + entries_length + data_length) % 4)) % 4
        if offset > len(artifact) or padding != expected_padding:
            raise FixtureError("generated artifact LTLB layout is invalid")
        if any(artifact[data_end:offset]):
            raise FixtureError("generated artifact LTLB padding is non-zero")

    code_length = len(artifact) - offset
    if code_length <= 0 or code_length % 4:
        raise FixtureError("generated artifact instruction stream is not non-empty and aligned")
    entrypoints = manifest.get("entrypoints")
    if not isinstance(entrypoints, list) or not entrypoints:
        raise FixtureError("compiler manifest has no entrypoints")
    return {
        "code_hash_hex": code_hash,
        "abi_hash_hex": abi_hash,
        "header_len": IVM_HEADER_BYTES,
        "code_offset": offset,
        "entrypoint_count": len(entrypoints),
    }


def _build_artifact(
    koto: FileSnapshot,
    source: FileSnapshot,
    sealed_inputs: BoundDirectory,
    stage: BoundDirectory,
    environment: Mapping[str, str],
) -> tuple[bytes, dict[str, Any], str]:
    target_directory = stage.path / "kotodama-target"
    target_directory.mkdir(mode=0o700)
    artifact_path = stage.path / "current_rust_contract_artifact.to"
    manifest_path = stage.path / "current_rust_contract_artifact.manifest.json"
    source_descriptor = _open_snapshot(source)
    source_argument = (
        Path(f"/proc/self/fd/{source_descriptor}")
        if sys.platform.startswith("linux")
        else Path(f"/dev/fd/{source_descriptor}")
    )
    bindings: list[str] = []
    commands = (
        ("fmt", "--check", source_argument),
        (
            "build",
            "--profile",
            "release",
            "--target-dir",
            target_directory,
            "--out",
            artifact_path,
            "--manifest-out",
            manifest_path,
            source_argument,
        ),
    )
    try:
        for arguments in commands:
            _authenticate_directory(sealed_inputs)
            _authenticate_descriptor(source, source_descriptor)
            _authenticate_file(source)
            os.lseek(source_descriptor, 0, os.SEEK_SET)
            _result, binding = _run_bound_executable(
                koto,
                arguments,
                environment=environment,
                inherited_fds=(source_descriptor,),
            )
            bindings.append(binding)
            _authenticate_directory(sealed_inputs)
            _authenticate_descriptor(source, source_descriptor)
            _authenticate_file(source)
    finally:
        os.close(source_descriptor)
    if len(set(bindings)) != 1:
        raise FixtureError("koto executable binding changed during generation")

    artifact_snapshot = _snapshot_file(artifact_path, "generated Kotodama artifact")
    manifest_snapshot = _snapshot_file(manifest_path, "generated compiler manifest")
    manifest, _manifest_text = _parse_json_snapshot(manifest_snapshot)
    if not isinstance(manifest, dict):
        raise FixtureError("generated compiler manifest is not a JSON object")
    _authenticate_file(artifact_snapshot)
    _authenticate_file(manifest_snapshot)
    return artifact_snapshot.data, manifest, bindings[0]


def _source_provenance(
    sources: dict[Path, FileSnapshot], closure_sha256: str
) -> dict[str, str | int]:
    return {
        "scope": "tracked-semantic-source-closure-v1",
        "closure_algorithm": "sha256-framed-path-and-bytes-v1",
        "closure_sha256": closure_sha256,
        "file_count": len(sources),
        "contract_source_git_blob": _git_blob_id(sources[SOURCE_PATH].data),
        "artifact_generator_git_blob": _git_blob_id(sources[GENERATOR_PATH].data),
    }


def _fixture_document(
    artifact: bytes,
    manifest: dict[str, Any],
    provenance: dict[str, str | int],
) -> dict[str, Any]:
    return {
        "fixture_version": 2,
        "source": SOURCE_PATH.name,
        "artifact_base64": base64.b64encode(artifact).decode("ascii"),
        "artifact_length": len(artifact),
        "artifact_sha256": _sha256(artifact),
        "manifest": manifest,
        "artifact_semantics": _semantic_expectation(artifact, manifest),
        "source_provenance": provenance,
    }


def _render(value: dict[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def _attestation(
    koto: FileSnapshot,
    git: FileSnapshot,
    cargo_lock: FileSnapshot,
    executable_bindings: dict[str, str],
    fixture_text: str,
    fixture: dict[str, Any],
) -> str:
    provenance = fixture["source_provenance"]
    return _render(
        {
            "attestation_version": 1,
            "canonical_fixture_sha256": _sha256(fixture_text.encode("utf-8")),
            "artifact_sha256": fixture["artifact_sha256"],
            "koto_sha256": koto.sha256,
            "koto_size": koto.size,
            "git_sha256": git.sha256,
            "git_size": git.size,
            "cargo_lock_sha256": cargo_lock.sha256,
            "cargo_lock_size": cargo_lock.size,
            "executable_binding": executable_bindings,
            "darwin_executable_binding_limitation": (
                "macOS has no executable fexecve/execveat API and rejects /dev/fd execution; "
                "a malicious same-UID process that discovers the random sealed stage could chmod "
                "and swap the executable between authentication and exec"
                if any(value == "darwin-sealed-private-path" for value in executable_bindings.values())
                else None
            ),
            "source_closure_sha256": provenance["closure_sha256"],
            "source_file_count": provenance["file_count"],
        }
    )


def _generate(koto_path: Path, git_path: Path, stage: BoundDirectory) -> tuple[str, str]:
    environment = _hermetic_environment(stage)
    sealed_inputs, source, koto, git = _prepare_sealed_inputs(stage, koto_path, git_path)
    cargo_lock = _snapshot_file(REPOSITORY_ROOT / "Cargo.lock", "local Cargo.lock")
    try:
        sources, closure_sha256, git_binding = _source_closure(git, environment)
        if sources[SOURCE_PATH].sha256 != source.sha256:
            raise FixtureError("sealed Kotodama source differs from the tracked source closure")
        artifact, manifest, koto_binding = _build_artifact(
            koto,
            source,
            sealed_inputs,
            stage,
            environment,
        )
        for snapshot in sources.values():
            _authenticate_file(snapshot)
        _authenticate_file(cargo_lock)
        fixture = _fixture_document(
            artifact,
            manifest,
            _source_provenance(sources, closure_sha256),
        )
        fixture_text = _render(fixture)
        attestation_text = _attestation(
            koto,
            git,
            cargo_lock,
            {"git": git_binding, "koto": koto_binding},
            fixture_text,
            fixture,
        )
        _write_new_file(
            stage,
            "generation-attestation.json",
            attestation_text.encode("utf-8"),
            mode=0o600,
        )
        return fixture_text, attestation_text
    finally:
        sealed_inputs.close()


def _check(expected: str, path: Path) -> None:
    snapshot = _snapshot_file(path, "checked-in fixture")
    parsed, actual = _parse_json_snapshot(snapshot)
    if not isinstance(parsed, dict):
        raise FixtureError("checked-in fixture is not a JSON object")
    if actual == expected:
        _authenticate_file(snapshot)
        return
    diff = "".join(
        difflib.unified_diff(
            actual.splitlines(keepends=True),
            expected.splitlines(keepends=True),
            fromfile=os.fspath(path),
            tofile=f"{path} (regenerated)",
        )
    )
    _authenticate_file(snapshot)
    raise FixtureError(f"exact-current admission fixture is stale\n{diff}")


def _parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="verify the checked-in fixture")
    mode.add_argument("--write", action="store_true", help="create a cache-staged fixture")
    parser.add_argument("--koto", type=Path, required=True, help="fresh koto executable")
    parser.add_argument(
        "--git",
        type=Path,
        required=True,
        help="exact non-symbolic Git executable used to enumerate tracked inputs",
    )
    parser.add_argument(
        "--cache-root",
        type=Path,
        default=os.environ.get("IROHA_KOTODAMA_CACHE_ROOT"),
        help="absolute task cache root (or IROHA_KOTODAMA_CACHE_ROOT)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help=(
            "exact <cache-root>/current-rust-contract-artifact.<unique>/"
            f"{FIXTURE_PATH.name} path for --write"
        ),
    )
    args = parser.parse_args(argv)
    if args.cache_root is None:
        parser.error("--cache-root or IROHA_KOTODAMA_CACHE_ROOT is required")
    if args.write and args.output is None:
        parser.error("--write requires --output")
    if args.check and args.output is not None:
        parser.error("--output is only valid with --write")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    cache_root: BoundDirectory | None = None
    work_stage: BoundDirectory | None = None
    output_stage: BoundDirectory | None = None
    try:
        koto = _resolve_input_file(args.koto, "koto", executable=True)
        git = _resolve_input_file(args.git, "Git", executable=True)
        cache_root = _bind_directory(args.cache_root, "task cache root")
        if args.write:
            output_stage = _bind_output(cache_root, args.output)
        work_stage = _create_work_stage(cache_root)
        expected, _attestation_text = _generate(koto, git, work_stage)
        print(f"attestation {work_stage.path / 'generation-attestation.json'}")
        if args.write:
            if output_stage is None:  # pragma: no cover - argument invariant
                raise FixtureError("write output was not bound")
            _write_new_file(
                output_stage,
                FIXTURE_PATH.name,
                expected.encode("utf-8"),
                mode=0o644,
            )
            print(f"generated {args.output}")
        else:
            _check(expected, REPOSITORY_ROOT / FIXTURE_PATH)
            print(f"verified {FIXTURE_PATH}")
    except FixtureError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    finally:
        for directory in (output_stage, work_stage, cache_root):
            if directory is not None:
                directory.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
