#!/usr/bin/env python3
"""Produce the exact signed Musubi V1 fixture pair in an empty staging root.

The Rust owner only emits a deterministic JSON envelope.  This explicit writer
retains directory descriptors from the filesystem root through the staging
root, requires that root to be empty, and exclusively creates the private
output hierarchy and final files relative to retained descriptors.  It never
renames, replaces, removes, or cleans up a pathname.  A failed run deliberately
leaves incomplete private staging residue; a successful return is the only
signal that the closed pair is complete.  Publication into the repository is a
separate, explicitly reviewed operation and is not claimed to be atomic here.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import stat
import subprocess
from typing import Any, Callable, Iterable, Sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
OWNER_SCHEMA = "iroha.musubi.signed_fixtures.owner.v1"
OUTPUTS = (
    "fixtures/musubi/instructions_v1.json",
    "fixtures/musubi/sdk_v1.json",
)
OUTPUT_BASENAMES = tuple(Path(path).name for path in OUTPUTS)
LEGACY_KEYS = frozenset({"chain_id", "genesis_hash", "genesis_block_hash"})
MAX_FIXTURE_BYTES = 8 * 1024 * 1024
MAX_OWNER_ENVELOPE_BYTES = 2 * MAX_FIXTURE_BYTES + 1024 * 1024


@dataclass(frozen=True)
class RenderedOutput:
    """One validated path/content pair emitted by the typed Rust owner."""

    relative_path: str
    contents: bytes


@dataclass(frozen=True)
class _Identity:
    device: int
    inode: int
    file_type: int


@dataclass(frozen=True)
class _DirectoryLink:
    parent_fd: int
    name: str
    child_fd: int
    identity: _Identity


@dataclass(frozen=True)
class _WrittenFile:
    output_name: str
    descriptor: int
    identity: _Identity
    expected_contents: bytes


class _OpenedAbsoluteDirectory:
    def __init__(
        self,
        path: Path,
        descriptors: list[int],
        links: list[_DirectoryLink],
        base_identity: _Identity,
    ) -> None:
        self.path = path
        self.descriptors = descriptors
        self.links = links
        self.base_identity = base_identity

    @property
    def descriptor(self) -> int:
        return self.descriptors[-1]

    @property
    def identity(self) -> _Identity:
        if self.links:
            return self.links[-1].identity
        return self.base_identity

    @property
    def identities(self) -> frozenset[_Identity]:
        return frozenset([self.base_identity, *(link.identity for link in self.links)])

    def verify(self) -> None:
        if _identity(os.fstat(self.descriptors[0])) != self.base_identity:
            raise RuntimeError("filesystem-root descriptor identity changed")
        _verify_directory_links(self.links)

    def close(self) -> None:
        while self.descriptors:
            os.close(self.descriptors.pop())

    def __enter__(self) -> _OpenedAbsoluteDirectory:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-root",
        required=True,
        type=Path,
        help=("existing absolute, non-symbolic, empty private external staging root"),
    )
    return parser.parse_args(argv)


def owner_command() -> list[str]:
    """Return the argument-free typed-owner command."""

    return [
        "cargo",
        "run",
        "--locked",
        "--offline",
        "--jobs",
        "1",
        "-p",
        "iroha_data_model",
        "--features",
        "dev-tools,test-fixtures,json,transparent_api",
        "--bin",
        "musubi_fixtures",
    ]


def _paths_overlap(left: Path, right: Path) -> bool:
    return left == right or left in right.parents or right in left.parents


def resolve_owner_cargo_target_dir() -> Path:
    """Resolve the caller-supplied private external Cargo target directory."""

    configured = os.environ.get("CARGO_TARGET_DIR")
    if not configured:
        raise RuntimeError(
            "CARGO_TARGET_DIR must name an existing private external directory"
        )
    configured_path = Path(configured)
    if not configured_path.is_absolute():
        raise RuntimeError("CARGO_TARGET_DIR must be absolute")
    repository = REPO_ROOT.resolve(strict=True)
    try:
        metadata = configured_path.lstat()
    except OSError as error:
        raise RuntimeError(
            "CARGO_TARGET_DIR must name an existing private external directory"
        ) from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        raise RuntimeError(
            "CARGO_TARGET_DIR must be a private non-symbolic directory"
        )
    target = configured_path.resolve(strict=True)
    if _paths_overlap(target, repository):
        raise RuntimeError("Musubi fixture Cargo target overlaps the repository")
    return target


def run_owner(cargo_target_dir: Path) -> bytes:
    """Run the typed owner and return its bounded stdout envelope."""

    target = cargo_target_dir.resolve(strict=True)
    repository = REPO_ROOT.resolve(strict=True)
    if _paths_overlap(target, repository):
        raise RuntimeError("Musubi fixture Cargo target overlaps the repository")
    metadata = target.stat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        raise RuntimeError("Musubi fixture Cargo target must be a private directory")
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = os.fspath(target)
    process = subprocess.Popen(
        owner_command(),
        cwd=REPO_ROOT,
        env=environment,
        stdout=subprocess.PIPE,
    )
    if process.stdout is None:  # pragma: no cover - guaranteed by stdout=PIPE
        raise RuntimeError("failed to capture the Musubi fixture owner envelope")
    chunks: list[bytes] = []
    captured = 0
    exceeded_bound = False
    while chunk := process.stdout.read(64 * 1024):
        captured += len(chunk)
        if captured <= MAX_OWNER_ENVELOPE_BYTES:
            chunks.append(chunk)
        else:
            exceeded_bound = True
    process.stdout.close()
    return_code = process.wait()
    if return_code != 0:
        raise subprocess.CalledProcessError(return_code, owner_command())
    if exceeded_bound:
        raise RuntimeError("Musubi fixture owner envelope exceeds its byte bound")
    return b"".join(chunks)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeError(
                f"duplicate JSON key in Musubi fixture owner output: {key}"
            )
        result[key] = value
    return result


def _decode_json(raw: bytes, description: str) -> Any:
    try:
        return json.loads(raw, object_pairs_hook=_reject_duplicate_keys)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"invalid JSON in {description}: {error}") from error


def reject_legacy_keys(value: Any, location: str = "$") -> None:
    """Reject retired deployment-identity keys recursively."""

    if isinstance(value, dict):
        legacy = LEGACY_KEYS.intersection(value)
        if legacy:
            raise RuntimeError(
                f"legacy Musubi deployment keys at {location}: {sorted(legacy)}"
            )
        for key, child in value.items():
            reject_legacy_keys(child, f"{location}/{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            reject_legacy_keys(child, f"{location}/{index}")


def _validate_rendered_outputs(
    outputs: Sequence[RenderedOutput],
) -> tuple[RenderedOutput, ...]:
    if tuple(output.relative_path for output in outputs) != OUTPUTS:
        raise RuntimeError("Musubi fixture output set is not the closed V1 pair")
    validated = tuple(outputs)
    for output in validated:
        if not output.contents or len(output.contents) > MAX_FIXTURE_BYTES:
            raise RuntimeError(
                f"Musubi fixture exceeds its byte bound: {output.relative_path}"
            )
        if not output.contents.endswith(b"\n"):
            raise RuntimeError(
                "Musubi fixture lacks its canonical trailing newline: "
                f"{output.relative_path}"
            )
        document = _decode_json(output.contents, output.relative_path)
        reject_legacy_keys(document, output.relative_path)
    return validated


def parse_owner_envelope(raw: bytes) -> tuple[RenderedOutput, ...]:
    """Validate the typed owner's exact V1 envelope and closed output set."""

    if not raw or len(raw) > MAX_OWNER_ENVELOPE_BYTES:
        raise RuntimeError("Musubi fixture owner envelope has an invalid byte length")
    if not raw.endswith(b"\n") or b"\n" in raw[:-1]:
        raise RuntimeError(
            "Musubi fixture owner envelope must be one JSON line with a trailing newline"
        )
    envelope = _decode_json(raw, "Musubi fixture owner envelope")
    if not isinstance(envelope, dict) or set(envelope) != {"schema", "outputs"}:
        raise RuntimeError("Musubi fixture owner envelope has an unexpected shape")
    if envelope["schema"] != OWNER_SCHEMA:
        raise RuntimeError("Musubi fixture owner envelope has an unknown schema")
    encoded_outputs = envelope["outputs"]
    if not isinstance(encoded_outputs, list) or len(encoded_outputs) != len(OUTPUTS):
        raise RuntimeError("Musubi fixture owner output set is not the closed V1 pair")

    rendered: list[RenderedOutput] = []
    for expected_path, encoded in zip(OUTPUTS, encoded_outputs):
        if not isinstance(encoded, dict) or set(encoded) != {"path", "contents"}:
            raise RuntimeError("Musubi fixture owner output has an unexpected shape")
        if encoded["path"] != expected_path:
            raise RuntimeError(
                "Musubi fixture owner output set is not the closed V1 pair"
            )
        contents = encoded["contents"]
        if not isinstance(contents, str):
            raise RuntimeError(f"Musubi fixture contents are not text: {expected_path}")
        contents_bytes = contents.encode("utf-8")
        rendered.append(RenderedOutput(expected_path, contents_bytes))

    return _validate_rendered_outputs(rendered)


def _require_secure_unix_primitives() -> None:
    if os.name != "posix":
        raise RuntimeError("secure Musubi fixture staging is supported only on Unix")
    for name in ("O_NOFOLLOW", "O_DIRECTORY", "O_CLOEXEC", "O_NONBLOCK"):
        if not hasattr(os, name):
            raise RuntimeError(f"secure Musubi fixture staging requires os.{name}")
    for function in (os.open, os.mkdir, os.stat):
        if function not in os.supports_dir_fd:
            raise RuntimeError(
                f"secure Musubi fixture staging requires dir_fd for {function.__name__}"
            )
    if os.stat not in os.supports_follow_symlinks:
        raise RuntimeError("secure Musubi fixture staging requires no-follow stat")
    if os.listdir not in os.supports_fd:
        raise RuntimeError("secure Musubi fixture staging requires descriptor listdir")


def _identity(metadata: os.stat_result) -> _Identity:
    return _Identity(
        device=metadata.st_dev,
        inode=metadata.st_ino,
        file_type=stat.S_IFMT(metadata.st_mode),
    )


def _directory_flags() -> int:
    return os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def _read_flags() -> int:
    return os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC


def _absolute_components(path: Path) -> tuple[str, ...]:
    raw = os.fspath(path)
    if not os.path.isabs(raw):
        raise RuntimeError("Musubi fixture root must be absolute")
    components = raw.split(os.sep)
    if not components or components[0] != "":
        raise RuntimeError("Musubi fixture root has an invalid absolute path")
    if any(component in {"", ".", ".."} for component in components[1:]):
        raise RuntimeError(
            "Musubi fixture root may not contain empty, dot, or parent components"
        )
    if len(components) == 1:
        raise RuntimeError("the filesystem root is not a Musubi fixture staging root")
    return tuple(components[1:])


def _open_absolute_directory(path: Path) -> _OpenedAbsoluteDirectory:
    _require_secure_unix_primitives()
    components = _absolute_components(path)
    descriptors = [os.open(os.sep, _directory_flags())]
    base_identity = _identity(os.fstat(descriptors[0]))
    links: list[_DirectoryLink] = []
    try:
        for component in components:
            parent_fd = descriptors[-1]
            child_fd = os.open(component, _directory_flags(), dir_fd=parent_fd)
            metadata = os.fstat(child_fd)
            if not stat.S_ISDIR(metadata.st_mode):
                os.close(child_fd)
                raise RuntimeError(f"path component is not a directory: {component}")
            descriptors.append(child_fd)
            links.append(
                _DirectoryLink(
                    parent_fd=parent_fd,
                    name=component,
                    child_fd=child_fd,
                    identity=_identity(metadata),
                )
            )
        opened = _OpenedAbsoluteDirectory(path, descriptors, links, base_identity)
        opened.verify()
        return opened
    except BaseException:
        while descriptors:
            os.close(descriptors.pop())
        raise


def _verify_directory_links(links: Sequence[_DirectoryLink]) -> None:
    for link in links:
        try:
            named = os.stat(link.name, dir_fd=link.parent_fd, follow_symlinks=False)
        except OSError as error:
            raise RuntimeError(
                f"retained directory path disappeared or changed: {link.name}"
            ) from error
        if (
            _identity(named) != link.identity
            or _identity(os.fstat(link.child_fd)) != link.identity
        ):
            raise RuntimeError(f"retained directory path identity changed: {link.name}")


def _validate_private_directory(metadata: os.stat_result, description: str) -> None:
    if not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError(f"{description} is not a directory")
    if metadata.st_uid != os.geteuid():
        raise RuntimeError(f"{description} is not owned by the current effective user")
    if stat.S_IMODE(metadata.st_mode) & 0o077:
        raise RuntimeError(f"{description} must not grant group or other permissions")


def _validate_private_file(metadata: os.stat_result, description: str) -> None:
    if not stat.S_ISREG(metadata.st_mode):
        raise RuntimeError(f"{description} is not a regular file")
    if metadata.st_uid != os.geteuid():
        raise RuntimeError(f"{description} is not owned by the current effective user")
    if stat.S_IMODE(metadata.st_mode) & 0o077:
        raise RuntimeError(f"{description} must not grant group or other permissions")
    if metadata.st_nlink != 1:
        raise RuntimeError(f"{description} must have exactly one hard link")


def _validate_private_directory_links(links: Sequence[_DirectoryLink]) -> None:
    _verify_directory_links(links)
    for link in links:
        _validate_private_directory(
            os.fstat(link.child_fd), f"fixture output directory {link.name}"
        )


def _create_private_directory(parent_fd: int, name: str) -> tuple[int, _DirectoryLink]:
    try:
        os.mkdir(name, 0o700, dir_fd=parent_fd)
        os.fsync(parent_fd)
    except FileExistsError as error:
        raise RuntimeError(f"fixture output path already exists: {name}") from error
    try:
        descriptor = os.open(name, _directory_flags(), dir_fd=parent_fd)
    except OSError as error:
        raise RuntimeError(
            f"fixture output ancestor is not a no-follow directory: {name}"
        ) from error
    metadata = os.fstat(descriptor)
    try:
        _validate_private_directory(metadata, f"fixture output directory {name}")
    except BaseException:
        os.close(descriptor)
        raise
    return descriptor, _DirectoryLink(parent_fd, name, descriptor, _identity(metadata))


def _require_exact_names(
    directory_fd: int, expected: set[str], description: str
) -> None:
    names = set(os.listdir(directory_fd))
    if names != expected:
        raise RuntimeError(
            f"{description} does not contain its exact closed set: {sorted(names)}"
        )


def _require_empty_directory(directory_fd: int, description: str) -> None:
    _require_exact_names(directory_fd, set(), description)


def _read_regular_file(
    directory_fd: int,
    name: str,
    *,
    require_private: bool,
) -> bytes:
    try:
        descriptor = os.open(name, _read_flags(), dir_fd=directory_fd)
    except OSError as error:
        raise RuntimeError(
            f"fixture output is not a no-follow regular file: {name}"
        ) from error
    try:
        metadata = os.fstat(descriptor)
        if require_private:
            _validate_private_file(metadata, f"fixture output {name}")
        elif not stat.S_ISREG(metadata.st_mode):
            raise RuntimeError(f"fixture output is not a regular file: {name}")
        if metadata.st_size < 0 or metadata.st_size > MAX_FIXTURE_BYTES:
            raise RuntimeError(f"fixture output exceeds its byte bound: {name}")
        chunks: list[bytes] = []
        remaining = metadata.st_size
        while remaining:
            chunk = os.read(descriptor, min(remaining, 1024 * 1024))
            if not chunk:
                raise RuntimeError(
                    f"fixture output was truncated while reading: {name}"
                )
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise RuntimeError(f"fixture output grew while reading: {name}")
        if _identity(os.fstat(descriptor)) != _identity(metadata):
            raise RuntimeError(f"fixture output identity changed while reading: {name}")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _create_final_output(directory_fd: int, output: RenderedOutput) -> _WrittenFile:
    basename = Path(output.relative_path).name
    flags = (
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | os.O_NONBLOCK
        | os.O_NOFOLLOW
        | os.O_CLOEXEC
    )
    try:
        descriptor = os.open(basename, flags, 0o600, dir_fd=directory_fd)
    except OSError as error:
        raise RuntimeError(
            f"fixture final output could not be created exclusively: {basename}"
        ) from error
    try:
        metadata = os.fstat(descriptor)
        _validate_private_file(metadata, f"fixture final output {basename}")
        return _WrittenFile(
            output_name=basename,
            descriptor=descriptor,
            identity=_identity(metadata),
            expected_contents=output.contents,
        )
    except BaseException:
        os.close(descriptor)
        raise


def _write_all(written: _WrittenFile) -> None:
    view = memoryview(written.expected_contents)
    while view:
        count = os.write(written.descriptor, view)
        if count <= 0:
            raise RuntimeError(
                f"failed to write fixture final output: {written.output_name}"
            )
        view = view[count:]
    os.fsync(written.descriptor)


def _read_retained_output(written: _WrittenFile) -> bytes:
    os.lseek(written.descriptor, 0, os.SEEK_SET)
    chunks: list[bytes] = []
    remaining = len(written.expected_contents)
    while remaining:
        chunk = os.read(written.descriptor, min(remaining, 1024 * 1024))
        if not chunk:
            raise RuntimeError(
                f"fixture final output was truncated: {written.output_name}"
            )
        chunks.append(chunk)
        remaining -= len(chunk)
    if os.read(written.descriptor, 1):
        raise RuntimeError(f"fixture final output grew: {written.output_name}")
    return b"".join(chunks)


def _verify_written_file(directory_fd: int, written: _WrittenFile) -> None:
    try:
        named = os.stat(
            written.output_name,
            dir_fd=directory_fd,
            follow_symlinks=False,
        )
    except OSError as error:
        raise RuntimeError(
            f"fixture final output path changed: {written.output_name}"
        ) from error
    if (
        _identity(named) != written.identity
        or _identity(os.fstat(written.descriptor)) != written.identity
    ):
        raise RuntimeError(
            f"fixture final output identity changed: {written.output_name}"
        )
    _validate_private_file(named, f"fixture final output {written.output_name}")
    if named.st_size != len(written.expected_contents):
        raise RuntimeError(
            f"fixture final output length changed: {written.output_name}"
        )
    if _read_retained_output(written) != written.expected_contents:
        raise RuntimeError(f"fixture final output bytes differ: {written.output_name}")


def _reject_repository_staging(root: _OpenedAbsoluteDirectory) -> None:
    with _open_absolute_directory(REPO_ROOT) as repository:
        if (
            root.identity in repository.identities
            or repository.identity in root.identities
        ):
            raise RuntimeError(
                "Musubi fixture output root must be external to the Iroha repository"
            )


def write_outputs(
    output_root: Path,
    outputs: Sequence[RenderedOutput],
    *,
    after_root_open: Callable[[], None] | None = None,
    before_output_open: Callable[[int, str], None] | None = None,
    after_output_open: Callable[[int, str], None] | None = None,
) -> None:
    """Create the closed fixture pair beneath an empty private root descriptor.

    The optional callbacks are deterministic adversarial-test hooks.  Production
    callers do not supply them.
    """

    outputs = _validate_rendered_outputs(outputs)
    with _open_absolute_directory(output_root) as root:
        _validate_private_directory(
            os.fstat(root.descriptor), "Musubi fixture output root"
        )
        _reject_repository_staging(root)
        root.verify()
        _require_empty_directory(root.descriptor, "Musubi fixture output root")
        if after_root_open is not None:
            after_root_open()
        _validate_private_directory(
            os.fstat(root.descriptor), "Musubi fixture output root"
        )
        root.verify()
        _require_empty_directory(root.descriptor, "Musubi fixture output root")

        retained_output_links: list[_DirectoryLink] = []
        output_descriptors: list[int] = []
        written_files: list[_WrittenFile] = []
        try:
            fixtures_fd, fixtures_link = _create_private_directory(
                root.descriptor, "fixtures"
            )
            output_descriptors.append(fixtures_fd)
            retained_output_links.append(fixtures_link)
            musubi_fd, musubi_link = _create_private_directory(fixtures_fd, "musubi")
            output_descriptors.append(musubi_fd)
            retained_output_links.append(musubi_link)
            _validate_private_directory_links(retained_output_links)
            _require_empty_directory(musubi_fd, "Musubi fixture output directory")

            for output in outputs:
                _validate_private_directory_links(retained_output_links)
                basename = Path(output.relative_path).name
                if before_output_open is not None:
                    before_output_open(musubi_fd, basename)
                written = _create_final_output(musubi_fd, output)
                written_files.append(written)
                if after_output_open is not None:
                    after_output_open(musubi_fd, basename)
                _write_all(written)
                _verify_written_file(musubi_fd, written)
                os.fsync(musubi_fd)

            _require_exact_names(
                root.descriptor,
                {"fixtures"},
                "Musubi fixture output root",
            )
            _require_exact_names(
                fixtures_fd,
                {"musubi"},
                "Musubi fixture fixtures directory",
            )
            _require_exact_names(
                musubi_fd,
                set(OUTPUT_BASENAMES),
                "Musubi fixture output directory",
            )
            for written in written_files:
                _verify_written_file(musubi_fd, written)
            _validate_private_directory_links(retained_output_links)
            os.fsync(musubi_fd)
            os.fsync(fixtures_fd)
            os.fsync(root.descriptor)
            _validate_private_directory(
                os.fstat(root.descriptor), "Musubi fixture output root"
            )
            root.verify()
        finally:
            # A failed private staging write deliberately leaves every created
            # final-name file and directory for inspection.  Closing retained
            # descriptors is not pathname cleanup and does not mutate residue.
            while written_files:
                os.close(written_files.pop().descriptor)
            while output_descriptors:
                os.close(output_descriptors.pop())


def read_closed_outputs(root_path: Path) -> tuple[RenderedOutput, ...]:
    """Read the exact checked-in pair without pathname traversal or mutation."""

    with _open_absolute_directory(root_path) as root:
        retained_links: list[_DirectoryLink] = []
        descriptors: list[int] = []
        try:
            parent_fd = root.descriptor
            for name in ("fixtures", "musubi"):
                try:
                    descriptor = os.open(name, _directory_flags(), dir_fd=parent_fd)
                except OSError as error:
                    raise RuntimeError(
                        f"missing no-follow Musubi fixture directory: {name}"
                    ) from error
                metadata = os.fstat(descriptor)
                descriptors.append(descriptor)
                link = _DirectoryLink(parent_fd, name, descriptor, _identity(metadata))
                retained_links.append(link)
                parent_fd = descriptor

            _verify_directory_links(retained_links)
            _require_exact_names(
                parent_fd,
                set(OUTPUT_BASENAMES),
                "checked-in Musubi fixture directory",
            )
            rendered = tuple(
                RenderedOutput(
                    relative_path,
                    _read_regular_file(
                        parent_fd,
                        Path(relative_path).name,
                        require_private=False,
                    ),
                )
                for relative_path in OUTPUTS
            )
            _verify_directory_links(retained_links)
            root.verify()
            return rendered
        finally:
            while descriptors:
                os.close(descriptors.pop())


def main(argv: Iterable[str] | None = None) -> int:
    options = parse_args(argv)
    cargo_target_dir = resolve_owner_cargo_target_dir()
    outputs = parse_owner_envelope(run_owner(cargo_target_dir))
    write_outputs(options.output_root, outputs)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
