#!/usr/bin/env python3
"""Build and clean-smoke a deterministic SoraFS CLI platform archive."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
from typing import BinaryIO, NamedTuple

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier use the pinned backport.
    import tomli as tomllib

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_path_identity import resolve_path_identity  # noqa: E402


SCHEMA = "sorafs.cli.candidate-manifest.v1"
TARGET_SUFFIXES = {
    "x86_64-unknown-linux-gnu": "",
    "aarch64-unknown-linux-gnu": "",
    "x86_64-apple-darwin": "",
    "aarch64-apple-darwin": "",
    "x86_64-pc-windows-msvc": ".exe",
}
MAX_FILE_COUNT = 2_048
MAX_TOTAL_BYTES = 8 * 1024 * 1024 * 1024
MAX_VERSION_MAP_BYTES = 64 * 1024
SMOKE_TIMEOUT_SECONDS = 30
MAX_SMOKE_OUTPUT_BYTES = 1024 * 1024
VERSION_RE = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\Z"
)
SIGNER_BINARY = "sorafs_external_software_signer"
BROKER_ALIAS = "libexec/iroha-runtime-provider-broker-v1"
WINDOWS_SIGNER_POLICY = "WINDOWS-UNSUPPORTED-EXTERNAL-SOFTWARE-SIGNER.md"
SIGNER_ASSET_ROOT = "share/iroha/sorafs"
MACOS_SIGNER_ROLES = (
    "proof-outcome",
    "repair",
    "reserve",
    "orderbook",
    "governance-dag",
    "potr-gateway",
    "potr-provider",
    "billing",
    "evidence-viewer",
    "stream-token",
    "pop-credentials",
)


class CandidateError(ValueError):
    """Raised when candidate inputs or the generated archive are unsafe."""


class FileRecord(NamedTuple):
    relative: str
    size: int
    sha256: str
    mode: int
    device: int
    inode: int
    mtime_ns: int


class HashingReader:
    """Hash the exact bytes consumed by tarfile without buffering whole binaries."""

    def __init__(self, handle: BinaryIO) -> None:
        self._handle = handle
        self._digest = hashlib.sha256()
        self.bytes_read = 0

    def read(self, size: int = -1) -> bytes:
        chunk = self._handle.read(size)
        self._digest.update(chunk)
        self.bytes_read += len(chunk)
        return chunk

    @property
    def hexdigest(self) -> str:
        return self._digest.hexdigest()


def _fail(message: str) -> None:
    raise CandidateError(message)


def _resolved_identity(path: Path, *, label: str) -> Path:
    errors: list[str] = []
    resolved = resolve_path_identity(path, errors, label=label)
    if resolved is None:
        _fail(errors[0] if errors else f"{label} cannot be resolved")
    return resolved


def _absolute_without_resolving(path: Path) -> Path:
    return path if path.is_absolute() else Path.cwd() / path


def _reject_symlink_components(path: Path, *, label: str) -> None:
    absolute = _absolute_without_resolving(path)
    current = Path(absolute.anchor)
    for component in absolute.parts[1:]:
        current /= component
        try:
            current_stat = current.lstat()
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(current_stat.st_mode):
            _fail(f"{label} `{path}` must not contain symlink components")


def _validate_relative_path(relative: str) -> None:
    path = PurePosixPath(relative)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
        or "\\" in relative
        or ":" in relative
        or any(ord(character) < 32 or ord(character) == 127 for character in relative)
    ):
        _fail(f"candidate entry `{relative}` is not a canonical relative path")


def _open_read_no_follow(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return os.open(path, flags)


def _hash_descriptor(handle: BinaryIO) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    while True:
        chunk = handle.read(1024 * 1024)
        if not chunk:
            break
        digest.update(chunk)
        size += len(chunk)
    return digest.hexdigest(), size


def _normalized_mode(relative: str, suffix: str, source_mode: int) -> int:
    required_binaries = {
        f"sorafs_cli{suffix}",
        f"sorafs_fetch{suffix}",
        f"sorafs-validate{suffix}",
    }
    if relative in required_binaries or source_mode & 0o111:
        return 0o755
    return 0o644


def _signer_inventory(target: str) -> set[str]:
    if target.endswith("windows-msvc"):
        return {WINDOWS_SIGNER_POLICY}
    common = {
        SIGNER_BINARY,
        f"{SIGNER_BINARY}.help.txt",
        BROKER_ALIAS,
        "iroha-runtime-provider-broker-v1.help.txt",
        f"{SIGNER_ASSET_ROOT}/external_software_signer/README.md",
        f"{SIGNER_ASSET_ROOT}/runtime_provider_broker/README.md",
    }
    if target.endswith("linux-gnu"):
        return common | {
            f"{SIGNER_ASSET_ROOT}/external_software_signer/sorafs-external-software-signer@.service",
            f"{SIGNER_ASSET_ROOT}/external_software_signer/systemd/iroha-runtime-provider-broker-v1.service.d/20-external-software-signers.conf",
            f"{SIGNER_ASSET_ROOT}/runtime_provider_broker/systemd/iroha-runtime-provider-broker-v1.service",
            f"{SIGNER_ASSET_ROOT}/runtime_provider_broker/systemd/sorafs-governance-dag@.service.d/20-runtime-provider-broker-v1.conf",
        }
    launchd_assets = {
        f"{SIGNER_ASSET_ROOT}/external_software_signer/launchd/sorafs-external-software-signer-launchd-v1",
        f"{SIGNER_ASSET_ROOT}/runtime_provider_broker/launchd/org.hyperledger.iroha.runtime-provider-broker-v1.plist",
    }
    launchd_assets.update(
        f"{SIGNER_ASSET_ROOT}/external_software_signer/launchd/org.hyperledger.iroha.sorafs-signer-{role}.plist"
        for role in MACOS_SIGNER_ROLES
    )
    return common | launchd_assets


def _scan_candidate(input_dir: Path, *, version: str, target: str) -> list[FileRecord]:
    suffix = TARGET_SUFFIXES[target]
    _reject_symlink_components(input_dir, label="candidate input directory")
    try:
        root_stat = input_dir.lstat()
    except FileNotFoundError:
        _fail(f"candidate input directory `{input_dir}` is missing")
    if not stat.S_ISDIR(root_stat.st_mode):
        _fail(f"candidate input directory `{input_dir}` must be a regular directory")

    records: list[FileRecord] = []
    total_bytes = 0
    pending = [input_dir]
    while pending:
        directory = pending.pop()
        try:
            entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except OSError as error:
            _fail(f"failed to scan candidate directory `{directory}`: {error}")
        for entry in entries:
            path = Path(entry.path)
            relative = path.relative_to(input_dir).as_posix()
            _validate_relative_path(relative)
            try:
                entry_stat = entry.stat(follow_symlinks=False)
            except OSError as error:
                _fail(f"failed to inspect candidate entry `{relative}`: {error}")
            if stat.S_ISDIR(entry_stat.st_mode):
                pending.append(path)
                continue
            if not stat.S_ISREG(entry_stat.st_mode):
                _fail(f"candidate entry `{relative}` must be a regular file")
            if entry_stat.st_nlink != 1:
                _fail(f"candidate entry `{relative}` must have exactly one hard link")
            if (
                relative == "version-map.toml"
                and entry_stat.st_size > MAX_VERSION_MAP_BYTES
            ):
                _fail(
                    "candidate version-map.toml exceeds the byte ceiling of "
                    f"{MAX_VERSION_MAP_BYTES}"
                )
            if len(records) >= MAX_FILE_COUNT:
                _fail(f"candidate file count exceeds the ceiling of {MAX_FILE_COUNT}")
            total_bytes += entry_stat.st_size
            if total_bytes > MAX_TOTAL_BYTES:
                _fail(f"candidate byte size exceeds the ceiling of {MAX_TOTAL_BYTES}")

            descriptor = _open_read_no_follow(path)
            try:
                descriptor_stat = os.fstat(descriptor)
                if not stat.S_ISREG(descriptor_stat.st_mode):
                    _fail(f"candidate entry `{relative}` changed type while opening")
                if (descriptor_stat.st_dev, descriptor_stat.st_ino) != (
                    entry_stat.st_dev,
                    entry_stat.st_ino,
                ):
                    _fail(f"candidate entry `{relative}` changed while opening")
                with os.fdopen(descriptor, "rb", closefd=False) as handle:
                    digest, observed_size = _hash_descriptor(handle)
                final_stat = os.fstat(descriptor)
            finally:
                os.close(descriptor)
            if observed_size != entry_stat.st_size or (
                final_stat.st_dev,
                final_stat.st_ino,
                final_stat.st_size,
                final_stat.st_mtime_ns,
            ) != (
                entry_stat.st_dev,
                entry_stat.st_ino,
                entry_stat.st_size,
                entry_stat.st_mtime_ns,
            ):
                _fail(f"candidate entry `{relative}` changed while hashing")
            records.append(
                FileRecord(
                    relative=relative,
                    size=observed_size,
                    sha256=digest,
                    mode=_normalized_mode(relative, suffix, entry_stat.st_mode),
                    device=entry_stat.st_dev,
                    inode=entry_stat.st_ino,
                    mtime_ns=entry_stat.st_mtime_ns,
                )
            )

    records.sort(key=lambda record: record.relative)
    validator_name = f"sorafs-validate-{version}-{target}"
    required_files = {
        f"sorafs_cli{suffix}",
        f"sorafs_fetch{suffix}",
        f"sorafs-validate{suffix}",
        "sorafs_cli.help.txt",
        "sorafs_fetch.help.txt",
        "sorafs-validate.help.txt",
        "version-map.toml",
        "ROLLBACK-YANK.md",
        "CHANGELOG.md",
        "LICENSE",
        f"reference-validator/{validator_name}.sha256",
        f"reference-validator/{validator_name}.tar.gz",
        f"reference-validator/{validator_name}.tar.gz.sha256",
        f"reference-validator/{validator_name}.manifest.json",
        f"reference-validator/{validator_name}.manifest.json.sha256",
        f"reference-validator/{validator_name}/sorafs-validate{suffix}",
        f"reference-validator/{validator_name}/HELP.txt",
        f"reference-validator/{validator_name}/include/sorafs_reference.h",
        f"reference-validator/{validator_name}/smoke.advert.json",
        f"reference-validator/{validator_name}/smoke.bundle.json",
    }
    optional_files = {
        f"reference-validator/{validator_name}.manifest.json.sig"
    }
    required_files |= _signer_inventory(target)
    present = {record.relative for record in records}
    missing = sorted(required_files - present)
    if missing:
        _fail("candidate is missing required release files: " + ", ".join(missing))
    unexpected = sorted(present - required_files - optional_files)
    if unexpected:
        _fail("candidate contains unexpected release files: " + ", ".join(unexpected))
    empty_required = sorted(
        record.relative
        for record in records
        if record.size == 0
    )
    if empty_required:
        _fail("candidate release files must not be empty: " + ", ".join(empty_required))
    if not target.endswith("windows-msvc"):
        records_by_path = {record.relative: record for record in records}
        if records_by_path[SIGNER_BINARY].sha256 != records_by_path[BROKER_ALIAS].sha256:
            _fail("runtime-provider broker alias is not byte-identical to the signer")
    return records


def _validate_version_map(
    input_dir: Path, records: list[FileRecord], *, version: str
) -> None:
    """Require the bounded embedded version map to bind the candidate version."""

    record = next(
        record for record in records if record.relative == "version-map.toml"
    )
    source = input_dir / "version-map.toml"
    try:
        descriptor = _open_read_no_follow(source)
    except OSError:
        _fail("candidate version-map.toml could not be opened safely")
    try:
        opened_stat = os.fstat(descriptor)
        if not stat.S_ISREG(opened_stat.st_mode):
            _fail("candidate version-map.toml changed type before validation")
        if (
            opened_stat.st_dev,
            opened_stat.st_ino,
            opened_stat.st_size,
            opened_stat.st_mtime_ns,
        ) != (
            record.device,
            record.inode,
            record.size,
            record.mtime_ns,
        ):
            _fail("candidate version-map.toml changed before validation")
        with os.fdopen(descriptor, "rb", closefd=False) as handle:
            payload = handle.read(MAX_VERSION_MAP_BYTES + 1)
        final_stat = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        len(payload) != record.size
        or len(payload) > MAX_VERSION_MAP_BYTES
        or hashlib.sha256(payload).hexdigest() != record.sha256
        or (
            final_stat.st_dev,
            final_stat.st_ino,
            final_stat.st_size,
            final_stat.st_mtime_ns,
        )
        != (
            record.device,
            record.inode,
            record.size,
            record.mtime_ns,
        )
    ):
        _fail("candidate version-map.toml changed during validation")

    try:
        document = tomllib.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError):
        _fail("candidate version-map.toml must be valid UTF-8 TOML")
    if "release_version" not in document:
        _fail("candidate version-map.toml must declare top-level release_version")
    release_version = document["release_version"]
    if not isinstance(release_version, str):
        _fail("candidate version-map.toml top-level release_version must be a string")
    if release_version != version:
        _fail(
            "candidate version-map.toml top-level release_version does not match "
            "candidate --version"
        )


def _manifest_bytes(
    records: list[FileRecord], *, version: str, target: str
) -> bytes:
    payload = {
        "schema": SCHEMA,
        "package": "sorafs-cli",
        "version": version,
        "target": target,
        "external_software_signer": {
            "backend": "software" if not target.endswith("windows-msvc") else None,
            "broker_alias": BROKER_ALIAS if not target.endswith("windows-msvc") else None,
            "binary": SIGNER_BINARY if not target.endswith("windows-msvc") else None,
            "qualification": "software-key-qualified"
            if not target.endswith("windows-msvc")
            else "unsupported-windows",
            "windows_supported": False,
        },
        "payload_file_count": len(records),
        "payload_total_bytes": sum(record.size for record in records),
        "files": [
            {
                "mode": f"{record.mode:04o}",
                "path": record.relative,
                "sha256": record.sha256,
                "size": record.size,
            }
            for record in records
        ],
    }
    return (
        json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")


def _directory_members(package_name: str, records: list[FileRecord]) -> list[str]:
    directories = {package_name}
    for record in records:
        parent = PurePosixPath(package_name, record.relative).parent
        while str(parent) != ".":
            directories.add(parent.as_posix())
            if parent.as_posix() == package_name:
                break
            parent = parent.parent
    return sorted(directories, key=lambda item: (item.count("/"), item))


def _tar_info(name: str, *, mode: int, size: int = 0, directory: bool = False) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name + ("/" if directory else ""))
    info.type = tarfile.DIRTYPE if directory else tarfile.REGTYPE
    info.mode = mode
    info.size = size
    info.mtime = 0
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    return info


def _add_source_file(
    archive: tarfile.TarFile,
    *,
    input_dir: Path,
    package_name: str,
    record: FileRecord,
) -> None:
    source = input_dir / Path(*PurePosixPath(record.relative).parts)
    descriptor = _open_read_no_follow(source)
    try:
        opened_stat = os.fstat(descriptor)
        if (
            opened_stat.st_dev,
            opened_stat.st_ino,
            opened_stat.st_size,
            opened_stat.st_mtime_ns,
        ) != (
            record.device,
            record.inode,
            record.size,
            record.mtime_ns,
        ):
            _fail(f"candidate entry `{record.relative}` changed before archiving")
        with os.fdopen(descriptor, "rb", closefd=False) as handle:
            reader = HashingReader(handle)
            archive.addfile(
                _tar_info(
                    f"{package_name}/{record.relative}",
                    mode=record.mode,
                    size=record.size,
                ),
                reader,
            )
        final_stat = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        reader.bytes_read != record.size
        or reader.hexdigest != record.sha256
        or (
            final_stat.st_dev,
            final_stat.st_ino,
            final_stat.st_size,
            final_stat.st_mtime_ns,
        )
        != (
            record.device,
            record.inode,
            record.size,
            record.mtime_ns,
        )
    ):
        _fail(f"candidate entry `{record.relative}` changed while archiving")


def _write_archive(
    temporary_archive: Path,
    *,
    input_dir: Path,
    package_name: str,
    records: list[FileRecord],
    manifest_bytes: bytes,
) -> None:
    descriptor = os.open(
        temporary_archive,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as raw:
            with gzip.GzipFile(
                filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=0
            ) as compressed:
                with tarfile.open(
                    fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT
                ) as archive:
                    for directory in _directory_members(package_name, records):
                        archive.addfile(
                            _tar_info(directory, mode=0o755, directory=True)
                        )
                    for record in records:
                        _add_source_file(
                            archive,
                            input_dir=input_dir,
                            package_name=package_name,
                            record=record,
                        )
                    archive.addfile(
                        _tar_info(
                            f"{package_name}/PACKAGE-MANIFEST.json",
                            mode=0o644,
                            size=len(manifest_bytes),
                        ),
                        io.BytesIO(manifest_bytes),
                    )
            raw.flush()
            os.fsync(raw.fileno())
    finally:
        os.close(descriptor)


def _safe_extract_and_smoke(
    archive_path: Path,
    *,
    package_name: str,
    target: str,
    records: list[FileRecord],
    manifest_bytes: bytes,
    work_dir: Path,
) -> None:
    expected_files = {
        f"{package_name}/{record.relative}": (record.sha256, record.size, record.mode)
        for record in records
    }
    expected_files[f"{package_name}/PACKAGE-MANIFEST.json"] = (
        hashlib.sha256(manifest_bytes).hexdigest(),
        len(manifest_bytes),
        0o644,
    )
    with tempfile.TemporaryDirectory(prefix=".sorafs-clean-consumer-", dir=work_dir) as raw:
        extract_root = Path(raw)
        seen: set[str] = set()
        with tarfile.open(archive_path, mode="r:gz") as archive:
            for member in archive:
                name = member.name.rstrip("/")
                _validate_relative_path(name)
                if not (
                    name == package_name or name.startswith(f"{package_name}/")
                ):
                    _fail(f"archive member `{member.name}` escaped the package root")
                if name in seen:
                    _fail(f"archive contains duplicate member `{member.name}`")
                seen.add(name)
                destination = extract_root / Path(*PurePosixPath(name).parts)
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True)
                    destination.chmod(0o755)
                    continue
                if not member.isfile() or member.linkname:
                    _fail(f"archive member `{member.name}` must be a regular file")
                expected = expected_files.get(name)
                if expected is None:
                    _fail(f"archive contains unmanifested file `{member.name}`")
                source = archive.extractfile(member)
                if source is None:
                    _fail(f"archive file `{member.name}` has no payload")
                destination.parent.mkdir(parents=True, exist_ok=True)
                digest = hashlib.sha256()
                observed_size = 0
                with destination.open("xb") as output:
                    while True:
                        chunk = source.read(1024 * 1024)
                        if not chunk:
                            break
                        output.write(chunk)
                        digest.update(chunk)
                        observed_size += len(chunk)
                expected_digest, expected_size, expected_mode = expected
                if (
                    observed_size != expected_size
                    or digest.hexdigest() != expected_digest
                    or member.size != expected_size
                    or member.mode != expected_mode
                    or member.mtime != 0
                    or member.uid != 0
                    or member.gid != 0
                ):
                    _fail(f"archive file `{member.name}` failed manifest verification")
                destination.chmod(expected_mode)

        actual_files = {
            path.relative_to(extract_root).as_posix()
            for path in extract_root.rglob("*")
            if path.is_file()
        }
        if actual_files != set(expected_files):
            _fail("clean extraction does not match the exact manifest file inventory")
        extracted_manifest = extract_root / package_name / "PACKAGE-MANIFEST.json"
        if extracted_manifest.read_bytes() != manifest_bytes:
            _fail("clean extraction manifest bytes do not match the signed inventory")

        suffix = TARGET_SUFFIXES[target]
        package_root = extract_root / package_name
        smoke_binaries = [
            f"sorafs_cli{suffix}",
            f"sorafs_fetch{suffix}",
            f"sorafs-validate{suffix}",
        ]
        if not target.endswith("windows-msvc"):
            smoke_binaries.extend((SIGNER_BINARY, BROKER_ALIAS))
        for binary_name in smoke_binaries:
            binary = package_root / binary_name
            try:
                with tempfile.TemporaryFile() as smoke_output:
                    completed = subprocess.run(
                        [str(binary), "--help"],
                        cwd=package_root,
                        check=False,
                        stdout=smoke_output,
                        stderr=subprocess.STDOUT,
                        timeout=SMOKE_TIMEOUT_SECONDS,
                    )
                    output_size = smoke_output.tell()
            except (OSError, subprocess.TimeoutExpired) as error:
                _fail(f"clean-consumer smoke failed for `{binary_name}`: {error}")
            if (
                completed.returncode != 0
                or output_size <= 0
                or output_size > MAX_SMOKE_OUTPUT_BYTES
            ):
                _fail(
                    f"clean-consumer smoke failed for `{binary_name}` "
                    f"with exit code {completed.returncode} or invalid bounded output"
                )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def _write_exclusive(path: Path, payload: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o644,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError(f"failed to write `{path}`")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def package_candidate(
    *,
    input_dir: Path,
    output_dir: Path,
    version: str,
    target: str,
) -> dict[str, object]:
    if not VERSION_RE.fullmatch(version):
        _fail("release version must be canonical SemVer")
    if target not in TARGET_SUFFIXES:
        _fail(f"unsupported release target `{target}`")

    _reject_symlink_components(output_dir, label="candidate output directory")
    input_resolved = _resolved_identity(input_dir, label="candidate input directory")
    output_resolved = _resolved_identity(output_dir, label="candidate output directory")
    common = Path(os.path.commonpath((input_resolved, output_resolved)))
    if common in {input_resolved, output_resolved}:
        _fail("candidate input and output directories must not overlap")

    records = _scan_candidate(input_dir, version=version, target=target)
    _validate_version_map(input_dir, records, version=version)

    output_dir.mkdir(parents=True, exist_ok=True)
    _reject_symlink_components(output_dir, label="candidate output directory")
    if not output_dir.is_dir():
        _fail(f"candidate output directory `{output_dir}` must be a directory")
    output_resolved = _resolved_identity(output_dir, label="candidate output directory")
    common = Path(os.path.commonpath((input_resolved, output_resolved)))
    if common in {input_resolved, output_resolved}:
        _fail("candidate input and output directories must not overlap")

    manifest_bytes = _manifest_bytes(records, version=version, target=target)
    package_name = f"sorafs-cli-{version}-{target}"
    archive_path = output_dir / f"{package_name}.tar.gz"
    manifest_path = output_dir / f"{package_name}.manifest.json"
    archive_sha_path = output_dir / f"{package_name}.tar.gz.sha256"
    manifest_sha_path = output_dir / f"{package_name}.manifest.json.sha256"
    outputs = (archive_path, manifest_path, archive_sha_path, manifest_sha_path)
    existing = [str(path) for path in outputs if path.exists() or path.is_symlink()]
    if existing:
        _fail("candidate output files already exist: " + ", ".join(existing))

    temporary_archive = output_dir / f".{package_name}.{os.getpid()}.tmp"
    if temporary_archive.exists() or temporary_archive.is_symlink():
        _fail(f"temporary candidate archive `{temporary_archive}` already exists")
    try:
        _write_archive(
            temporary_archive,
            input_dir=input_dir,
            package_name=package_name,
            records=records,
            manifest_bytes=manifest_bytes,
        )
        _safe_extract_and_smoke(
            temporary_archive,
            package_name=package_name,
            target=target,
            records=records,
            manifest_bytes=manifest_bytes,
            work_dir=output_dir,
        )
        archive_digest = _sha256(temporary_archive)
        os.replace(temporary_archive, archive_path)
        archive_path.chmod(0o644)
        _write_exclusive(manifest_path, manifest_bytes)
        _write_exclusive(
            archive_sha_path,
            f"{archive_digest}  {archive_path.name}\n".encode("ascii"),
        )
        manifest_digest = hashlib.sha256(manifest_bytes).hexdigest()
        _write_exclusive(
            manifest_sha_path,
            f"{manifest_digest}  {manifest_path.name}\n".encode("ascii"),
        )
    finally:
        try:
            temporary_archive.unlink()
        except FileNotFoundError:
            pass

    summary: dict[str, object] = {
        "schema": SCHEMA,
        "status": "verified",
        "version": version,
        "target": target,
        "archive": archive_path.name,
        "archive_sha256": archive_digest,
        "manifest": manifest_path.name,
        "manifest_sha256": manifest_digest,
        "payload_file_count": len(records),
        "clean_smoke_binary_count": 3
        if target.endswith("windows-msvc")
        else 5,
    }
    return summary


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-dir", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    try:
        summary = package_candidate(
            input_dir=args.input_dir,
            output_dir=args.out_dir,
            version=args.version,
            target=args.target,
        )
    except (CandidateError, OSError, tarfile.TarError) as error:
        print(f"error: invalid SoraFS CLI release candidate: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
