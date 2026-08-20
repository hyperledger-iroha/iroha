#!/usr/bin/env python3
"""Run Kagemusha V4 generation under the applicable reviewed memory guard."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import secrets
import signal
import stat
import subprocess
import sys
import tempfile
import time
from typing import Sequence

# The reviewed source closure rejects generated cache paths. Prevent local
# imports below from invalidating the sealed checkout during generation.
sys.dont_write_bytecode = True

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.formal import run_sumeragi_v2_tlapm_guard as resource_guard


ABSOLUTE_MAX_MEMORY_BYTES = 64 * 1024 * 1024 * 1024
# On macOS each resource sample uses a process-group-scoped inventory; full-host
# inventories remain at admission and the final-success gate. Match the formal
# guard's bounded 4 Hz cadence; the kernel peak-RSS check remains an independent
# final gate.
SAMPLE_INTERVAL_SECONDS = 0.25
BYTES_PER_GIB = 1024 * 1024 * 1024
LOCK_PATH = Path("/tmp") / f"iroha-kagemusha-v4-{os.getuid()}.lock"
STAGING_ID_OPTION = "--staging-id"
STAGING_NAME_OPTION = "--staging-name"
OUTPUT_PARENT_FD_OPTION = "--output-parent-fd"
MEMORY_LIMIT_OPTION = "--memory-limit-bytes"
GENERATOR_BINARY_SHA256_OPTION = "--generator-binary-sha256"
SEALED_BUILD_REPORT_SHA256_OPTION = "--sealed-candidate-build-report-sha256"
SEALED_BUILD_REPORT_SCHEMA = "iroha.kagemusha.sealed_candidate_double_build_report.v1"
NATIVE_SEALED_BUILD_REPORT_SCHEMA = (
    "iroha.kagemusha.native_sealed_candidate_double_build_report.v2"
)
NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT = (
    "iroha.kagemusha.native-sealed-builder-launch.v1"
)
NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT = (
    "iroha.kagemusha.sealed-builder-exact-arguments.v1"
)
NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT = (
    "iroha.kagemusha.sealed-builder-exact-environment.v1"
)
NATIVE_SEALED_BUILDER_OS_TCB_CONTRACT = "iroha.kagemusha.macos-os-library-tcb.v1"
NATIVE_SEALED_BUILDER_REPORT_PUBLICATION_CONTRACT = (
    "iroha.kagemusha.native-no-replace-report-publication.v1"
)
NATIVE_SEALED_BUILDER_RUNTIME_DEPENDENCY_CONTRACT = (
    "iroha.kagemusha.symlink-free-macho-runtime-closure.v1"
)
MAX_SEALED_BUILD_REPORT_BYTES = 1024 * 1024
MEMORY_ENFORCEMENT_PROFILE = "self-physical-footprint-v1"
MEMORY_CAPACITY_OPERATION = "memory-capacity-v1"
MEMORY_CAPACITY_SCHEMA = "iroha.kagemusha.memory-capacity.v1"
MEMORY_CAPACITY_POLICY = "half-effective-physical-cap-absolute-v1"
MAX_MEMORY_CAPACITY_OUTCOME_BYTES = 256
FIXED_CANDIDATE_CHILD_PATH = "/usr/bin:/bin"
PUBLICATION_OUTCOME_SCHEMA = "iroha.kagemusha.publication_outcome.v1"
MAX_PUBLICATION_OUTCOME_BYTES = 16 * 1024
PUBLICATION_CONTROL_RECORD = "PUBLICATION"
PUBLICATION_CONTROL_DIGEST_HEX_LENGTH = hashlib.sha256().digest_size * 2
STAGING_ID_HEX_LENGTH = 32
STAGING_PREFIX = ".kagemusha-v4-staging-"
GUARDED_OUTPUT_SUFFIX = "unpublished"
BUNDLE_EXECUTABLE = "kagemusha_recursive_spend_v4_bundle"
JOURNAL_PREFIX = ".kagemusha-v4-guard-"
JOURNAL_SUFFIX = ".json"
JOURNAL_SCHEMA = "iroha.kagemusha.candidate_guard_journal.v1"
MAX_JOURNAL_BYTES = 4096
MAX_RECOVERABLE_JOURNALS = 64
CANDIDATE_SESSION_WRAPPER_FLAG = "--kagemusha-candidate-session-wrapper"
MINIMUM_OUTPUT_FREE_BYTES = 16 * 1024 * 1024 * 1024
DISK_BACKED_OUTPUT_FILESYSTEM_TYPES = frozenset(
    {
        "apfs",
        "btrfs",
        "ext2",
        "ext2/ext3",
        "ext3",
        "ext4",
        "f2fs",
        "fuseblk",
        "hfs",
        "hfsplus",
        "jfs",
        "reiserfs",
        "ufs",
        "xfs",
        "zfs",
    }
)


@dataclass(frozen=True)
class GenerationMemoryCapacityV1:
    """Authoritative memory-policy result returned by the pinned Rust bundle."""

    effective_physical_capacity_bytes: int
    safety_ceiling_bytes: int
    absolute_maximum_bytes: int
    enforcement_profile: str
    policy: str

    def report_context(self) -> dict[str, object]:
        """Return the exact Rust policy result propagated into the report."""

        return {
            "absolute_maximum_bytes": self.absolute_maximum_bytes,
            "effective_physical_capacity_bytes": (
                self.effective_physical_capacity_bytes
            ),
            "enforcement_profile": self.enforcement_profile,
            "policy": self.policy,
            "safety_ceiling_bytes": self.safety_ceiling_bytes,
            "schema": MEMORY_CAPACITY_SCHEMA,
        }


@dataclass
class ExecutionCopy:
    """A Darwin-safe private copy materialized only from the admitted fd."""

    directory_name: str
    file_name: str
    path: Path
    directory_descriptor: int
    file_descriptor: int
    directory_device: int
    directory_inode: int
    file_device: int
    file_inode: int
    mode: int
    size_bytes: int
    sha256: str

    def report_context(self) -> dict[str, object]:
        """Return the identity of the exact private execution copy."""

        return {
            "canonical_path": str(self.path),
            "directory_device": self.directory_device,
            "directory_inode": self.directory_inode,
            "directory_name": self.directory_name,
            "file_device": self.file_device,
            "file_inode": self.file_inode,
            "file_name": self.file_name,
            "method": "darwin_private_fd_copy",
            "mode": self.mode,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
        }


@dataclass
class ExecutableSnapshot:
    """Cryptographic and filesystem identity of one admitted executable."""

    path: Path
    sha256: str
    size_bytes: int
    device: int
    inode: int
    mode: int
    link_count: int
    owner_uid: int
    modified_ns: int
    changed_ns: int
    descriptor: int
    execution_copy: ExecutionCopy | None = None

    def report_context(self) -> dict[str, object]:
        """Return stable JSON evidence for this exact executable snapshot."""

        context: dict[str, object] = {
            "canonical_path": str(self.path),
            "build_profile": self.path.parent.name,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
            "stat_identity": {
                "changed_ns": self.changed_ns,
                "device": self.device,
                "inode": self.inode,
                "link_count": self.link_count,
                "mode": self.mode,
                "modified_ns": self.modified_ns,
                "owner_uid": self.owner_uid,
            },
        }
        if self.execution_copy is None:
            context["execution"] = {
                "descriptor_path": self.execution_path(),
                "method": "pinned_fd",
            }
        else:
            context["execution"] = self.execution_copy.report_context()
        return context

    def execution_path(self) -> str:
        """Return the inherited descriptor path used for both executions."""

        execution_descriptor = self.execution_descriptor()
        if execution_descriptor < 3:
            raise resource_guard.GuardError(
                "Kagemusha executable descriptor is unavailable"
            )
        if self.execution_copy is not None:
            return str(self.execution_copy.path)
        return f"/proc/self/fd/{execution_descriptor}"

    def execution_descriptor(self) -> int:
        """Return the fd which pins the bytes used by the next exec."""

        if self.execution_copy is not None:
            return self.execution_copy.file_descriptor
        return self.descriptor

    def close(self) -> None:
        """Close the executable descriptor retained for the full lifecycle."""

        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1
        if self.execution_copy is not None:
            for descriptor_name in ("file_descriptor", "directory_descriptor"):
                descriptor = getattr(self.execution_copy, descriptor_name)
                if descriptor >= 0:
                    os.close(descriptor)
                    setattr(self.execution_copy, descriptor_name, -1)


@dataclass
class PinnedSealedBuildReport:
    """Descriptor-pinned canonical report authenticating two sealed builds."""

    path: Path
    descriptor: int
    device: int
    inode: int
    mode: int
    owner_uid: int
    size_bytes: int
    modified_ns: int
    changed_ns: int
    sha256: str
    generator_sha256: str
    generator_size_bytes: int

    def validate(self) -> None:
        """Rehash the pinned bytes and reject pathname or metadata drift."""

        before = os.fstat(self.descriptor)
        current = os.stat(self.path, follow_symlinks=False)
        expected = (
            self.device,
            self.inode,
            self.mode,
            self.owner_uid,
            self.size_bytes,
            self.modified_ns,
            self.changed_ns,
        )
        observed = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        )
        pathname = (
            current.st_dev,
            current.st_ino,
            current.st_mode,
            current.st_uid,
            current.st_size,
            current.st_mtime_ns,
            current.st_ctime_ns,
        )
        if observed != expected or pathname != expected:
            raise resource_guard.GuardError("sealed build report identity changed")
        os.lseek(self.descriptor, 0, os.SEEK_SET)
        payload = bytearray()
        while len(payload) <= MAX_SEALED_BUILD_REPORT_BYTES:
            chunk = os.read(
                self.descriptor,
                min(64 * 1024, MAX_SEALED_BUILD_REPORT_BYTES + 1 - len(payload)),
            )
            if not chunk:
                break
            payload.extend(chunk)
        if (
            len(payload) != self.size_bytes
            or hashlib.sha256(payload).hexdigest() != self.sha256
            or os.fstat(self.descriptor).st_mtime_ns != self.modified_ns
            or os.fstat(self.descriptor).st_ctime_ns != self.changed_ns
        ):
            raise resource_guard.GuardError("sealed build report bytes changed")

    def close(self) -> None:
        """Close the report descriptor retained across generation."""

        resource_guard._close_descriptor(self.descriptor)
        self.descriptor = -1


@dataclass
class PinnedOutputParent:
    """An output parent held open by identity for cleanup and journaling."""

    path: Path
    descriptor: int
    device: int
    inode: int
    output_name: str
    filesystem_type: str
    free_bytes_at_admission: int

    def validate(self, *, require_path: bool = True) -> None:
        """Require the descriptor identity and, when requested, its original path."""

        try:
            opened = os.fstat(self.descriptor)
        except OSError as error:
            raise resource_guard.GuardError(
                "Kagemusha output parent identity is unavailable"
            ) from error
        if (
            not stat.S_ISDIR(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (self.device, self.inode)
        ):
            raise resource_guard.GuardError("Kagemusha output parent identity changed")
        if require_path:
            try:
                current = os.stat(self.path, follow_symlinks=False)
            except OSError as error:
                raise resource_guard.GuardError(
                    "Kagemusha output parent path is unavailable"
                ) from error
            if (
                not stat.S_ISDIR(current.st_mode)
                or (current.st_dev, current.st_ino) != (self.device, self.inode)
            ):
                raise resource_guard.GuardError(
                    "Kagemusha output parent path identity changed"
                )

    def report_context(self) -> dict[str, object]:
        """Return stable JSON evidence for the pinned output parent."""

        return {
            "canonical_path": str(self.path),
            "device": self.device,
            "filesystem_type": self.filesystem_type,
            "free_bytes_at_admission": self.free_bytes_at_admission,
            "inode": self.inode,
            "minimum_free_bytes": MINIMUM_OUTPUT_FREE_BYTES,
            "output_name": self.output_name,
        }

    def close(self) -> None:
        """Close the owned directory descriptor."""

        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


@dataclass(frozen=True)
class CandidatePublicationContract:
    """Immutable child-to-publisher arguments owned by the launcher."""

    requested_out_dir: str
    guarded_out_dir: str
    output_name: str
    output_parent_descriptor: int
    source_commit: str
    source_tree_sha256: str
    staging_id: str
    staging_name: str

    def validate(self, parent: PinnedOutputParent) -> None:
        """Require every publication argument to remain bound to the parent."""

        parent.validate()
        if (
            self.output_name != parent.output_name
            or self.output_parent_descriptor != parent.descriptor
            or self.requested_out_dir != str(parent.path / parent.output_name)
            or self.guarded_out_dir
            != str(parent.path / _guarded_output_name(self.staging_id))
            or self.staging_name != _staging_name(self.staging_id)
        ):
            raise resource_guard.GuardError(
                "Kagemusha child-to-publisher contract changed"
            )

    def report_context(self) -> dict[str, object]:
        """Return non-secret continuity evidence for this invocation."""

        return {
            "guarded_out_dir": self.guarded_out_dir,
            "output_name": self.output_name,
            "output_parent_descriptor": self.output_parent_descriptor,
            "requested_out_dir": self.requested_out_dir,
            "source_commit": self.source_commit,
            "source_tree_sha256": self.source_tree_sha256,
            "staging_id": self.staging_id,
            "staging_name": self.staging_name,
        }


@dataclass
class PinnedStagingDirectory:
    """Identity of the exact hidden directory admitted before generation."""

    staging_id: str
    name: str
    descriptor: int
    device: int
    inode: int

    def validate_named(
        self,
        parent: PinnedOutputParent,
        contract: CandidatePublicationContract,
    ) -> None:
        """Require the guarded name to still identify this exact directory."""

        contract.validate(parent)
        if self.staging_id != contract.staging_id or self.name != contract.staging_name:
            raise resource_guard.GuardError(
                "Kagemusha staging id or name changed before publication"
            )
        try:
            opened = os.fstat(self.descriptor)
            named = os.stat(
                self.name,
                dir_fd=parent.descriptor,
                follow_symlinks=False,
            )
        except OSError as error:
            raise resource_guard.GuardError(
                "Kagemusha guarded staging directory is unavailable"
            ) from error
        if (
            not stat.S_ISDIR(opened.st_mode)
            or not stat.S_ISDIR(named.st_mode)
            or (opened.st_dev, opened.st_ino) != (self.device, self.inode)
            or (named.st_dev, named.st_ino) != (self.device, self.inode)
            or opened.st_uid != os.geteuid()
            or named.st_uid != os.geteuid()
            or opened.st_mode & 0o077 != 0
            or named.st_mode & 0o077 != 0
        ):
            raise resource_guard.GuardError(
                "Kagemusha guarded staging directory identity changed"
            )

    def validate_published(
        self,
        parent: PinnedOutputParent,
        contract: CandidatePublicationContract,
    ) -> os.stat_result:
        """Require publication to rename this exact directory to the final leaf."""

        contract.validate(parent)
        opened = os.fstat(self.descriptor)
        try:
            published = os.stat(
                contract.output_name,
                dir_fd=parent.descriptor,
                follow_symlinks=False,
            )
        except OSError as error:
            raise resource_guard.GuardError(
                "Kagemusha publisher did not create the requested candidate directory"
            ) from error
        try:
            os.stat(
                contract.staging_name,
                dir_fd=parent.descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            pass
        else:
            raise resource_guard.GuardError(
                "Kagemusha publisher retained the guarded staging name"
            )
        if (
            not stat.S_ISDIR(opened.st_mode)
            or not stat.S_ISDIR(published.st_mode)
            or (opened.st_dev, opened.st_ino) != (self.device, self.inode)
            or (published.st_dev, published.st_ino) != (self.device, self.inode)
            or published.st_uid != os.geteuid()
            or published.st_mode & 0o077 != 0
        ):
            raise resource_guard.GuardError(
                "published Kagemusha candidate is not the guarded staging directory"
            )
        return published

    def close(self) -> None:
        """Release the staging identity descriptor."""

        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


def _executable_stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return every mutable stat field bound by an executable snapshot."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _validate_executable_metadata(metadata: os.stat_result) -> None:
    """Require one owner-controlled, single-link, non-empty executable file."""

    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_nlink != 1
        or metadata.st_size <= 0
        or metadata.st_mode & stat.S_IXUSR == 0
        or metadata.st_mode & 0o022 != 0
    ):
        raise resource_guard.GuardError(
            "Kagemusha executable has unsafe ownership, links, mode, or size"
        )


def _hash_executable_descriptor(
    descriptor: int,
) -> tuple[os.stat_result, str]:
    """Hash a pinned executable without changing its shared file offset."""

    before = os.fstat(descriptor)
    _validate_executable_metadata(before)
    digest = hashlib.sha256()
    offset = 0
    while offset < before.st_size:
        if hasattr(os, "pread"):
            chunk = os.pread(descriptor, min(1024 * 1024, before.st_size - offset), offset)
        else:  # pragma: no cover - every supported Unix platform exposes pread
            os.lseek(descriptor, offset, os.SEEK_SET)
            chunk = os.read(descriptor, min(1024 * 1024, before.st_size - offset))
        if not chunk:
            raise resource_guard.GuardError(
                "Kagemusha executable ended while it was being hashed"
            )
        digest.update(chunk)
        offset += len(chunk)
    after = os.fstat(descriptor)
    if _executable_stat_identity(before) != _executable_stat_identity(after):
        raise resource_guard.GuardError(
            "Kagemusha executable changed while it was being hashed"
        )
    return after, digest.hexdigest()


def _open_executable_identity(path: Path) -> tuple[int, os.stat_result, str]:
    """Open and hash one safe regular executable, retaining its descriptor."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise resource_guard.GuardError(
            f"Kagemusha executable cannot be opened safely: {error}"
        ) from error
    try:
        after, digest = _hash_executable_descriptor(descriptor)
        path_after = os.stat(path, follow_symlinks=False)
        if _executable_stat_identity(after) != _executable_stat_identity(path_after):
            raise resource_guard.GuardError(
                "Kagemusha executable changed while it was being admitted"
            )
        return descriptor, after, digest
    except BaseException:
        os.close(descriptor)
        raise


def _read_executable_identity(path: Path) -> tuple[os.stat_result, str]:
    """Hash one safe executable through a short-lived descriptor."""

    descriptor, metadata, digest = _open_executable_identity(path)
    os.close(descriptor)
    return metadata, digest


def _snapshot_executable(path_text: str, expected_name: str) -> ExecutableSnapshot:
    """Admit and cryptographically snapshot one exact prebuilt executable."""

    supplied = Path(path_text)
    try:
        supplied_metadata = supplied.lstat()
        resolved = supplied.resolve(strict=True)
    except OSError as error:
        raise resource_guard.GuardError(
            f"Kagemusha executable is unavailable: {error}"
        ) from error
    if stat.S_ISLNK(supplied_metadata.st_mode):
        raise resource_guard.GuardError("Kagemusha executable must not be a symlink")
    admitted_name = (
        resolved.name[:-4] if resolved.name.endswith(".exe") else resolved.name
    )
    if admitted_name != expected_name:
        raise resource_guard.GuardError(
            f"Kagemusha resource guard requires the prebuilt {expected_name} executable"
        )
    descriptor, metadata, sha256 = _open_executable_identity(resolved)
    return ExecutableSnapshot(
        path=resolved,
        sha256=sha256,
        size_bytes=metadata.st_size,
        device=metadata.st_dev,
        inode=metadata.st_ino,
        mode=metadata.st_mode,
        link_count=metadata.st_nlink,
        owner_uid=metadata.st_uid,
        modified_ns=metadata.st_mtime_ns,
        changed_ns=metadata.st_ctime_ns,
        descriptor=descriptor,
    )


def _strict_json_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    """Reject duplicate JSON object members at every nesting level."""

    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise resource_guard.GuardError("sealed build report has duplicate JSON members")
        result[key] = value
    return result


def _report_hex(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or not value.isascii()
        or any(character not in "0123456789abcdef" for character in value)
        or value == "0" * 64
    ):
        raise resource_guard.GuardError(f"sealed build report {label} is invalid")
    return value


def _unwrap_native_sealed_build_report(
    envelope: object,
) -> tuple[dict[str, object], dict[str, object]]:
    """Reject direct Python reports and unwrap one exact native V2 envelope."""

    if not isinstance(envelope, dict) or set(envelope) != {
        "builder_report_hex",
        "builder_report_sha256",
        "builder_report_size_bytes",
        "native_launch",
        "schema",
    }:
        raise resource_guard.GuardError(
            "sealed build report lacks its exact native-launch envelope"
        )
    if envelope.get("schema") != NATIVE_SEALED_BUILD_REPORT_SCHEMA:
        raise resource_guard.GuardError(
            "sealed build report was not published by the native launcher"
        )
    inner_hex = envelope.get("builder_report_hex")
    inner_size = envelope.get("builder_report_size_bytes")
    if (
        not isinstance(inner_hex, str)
        or not inner_hex
        or len(inner_hex) % 2
        or len(inner_hex) > 2 * MAX_SEALED_BUILD_REPORT_BYTES
        or any(character not in "0123456789abcdef" for character in inner_hex)
        or not isinstance(inner_size, int)
        or isinstance(inner_size, bool)
        or not 1 <= inner_size <= MAX_SEALED_BUILD_REPORT_BYTES
    ):
        raise resource_guard.GuardError(
            "native sealed-builder payload encoding is malformed"
        )
    inner = bytes.fromhex(inner_hex)
    if (
        len(inner) != inner_size
        or hashlib.sha256(inner).hexdigest()
        != _report_hex(envelope.get("builder_report_sha256"), "inner payload digest")
    ):
        raise resource_guard.GuardError(
            "native sealed-builder payload differs from its envelope binding"
        )
    launch = envelope.get("native_launch")
    exact_launch_fields = {
        "argument_contract",
        "argument_sha256",
        "builder_entrypoint_sha256",
        "contract",
        "controller_sha256",
        "environment_contract",
        "environment_sha256",
        "macos_build",
        "os_tcb_contract",
        "os_tcb_sha256",
        "python_interpreter_sha256",
        "python_runtime_tree_sha256",
        "report_publication_contract",
        "runtime_dependency_contract",
    }
    if (
        not isinstance(launch, dict)
        or set(launch) != exact_launch_fields
        or launch.get("contract") != NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT
        or launch.get("argument_contract")
        != NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT
        or launch.get("environment_contract")
        != NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT
        or launch.get("os_tcb_contract") != NATIVE_SEALED_BUILDER_OS_TCB_CONTRACT
        or launch.get("report_publication_contract")
        != NATIVE_SEALED_BUILDER_REPORT_PUBLICATION_CONTRACT
        or launch.get("runtime_dependency_contract")
        != NATIVE_SEALED_BUILDER_RUNTIME_DEPENDENCY_CONTRACT
        or not isinstance(launch.get("macos_build"), str)
        or not launch["macos_build"]
        or len(launch["macos_build"]) > 64
    ):
        raise resource_guard.GuardError(
            "native sealed-builder launch contract is not exact"
        )
    for field in (
        "argument_sha256",
        "builder_entrypoint_sha256",
        "controller_sha256",
        "environment_sha256",
        "os_tcb_sha256",
        "python_interpreter_sha256",
        "python_runtime_tree_sha256",
    ):
        _report_hex(launch.get(field), f"native launch {field}")
    try:
        report = json.loads(
            inner,
            object_pairs_hook=_strict_json_object,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                resource_guard.GuardError(
                    "sealed build report contains a non-finite number"
                )
            ),
        )
    except resource_guard.GuardError:
        raise
    except (UnicodeError, ValueError, json.JSONDecodeError) as error:
        raise resource_guard.GuardError(
            "native sealed-builder payload is not strict JSON"
        ) from error
    if not isinstance(report, dict) or resource_guard._canonical_json(report) != inner:
        raise resource_guard.GuardError(
            "native sealed-builder payload is not canonical JSON"
        )
    return report, launch


def _open_sealed_build_report(
    path: Path, expected_sha256: str
) -> PinnedSealedBuildReport:
    """Pin and validate the canonical report for both sealed generator builds."""

    expected_sha256 = _report_hex(expected_sha256, "digest")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
        metadata = os.fstat(descriptor)
        current = os.stat(path, follow_symlinks=False)
    except OSError as error:
        if "descriptor" in locals():
            os.close(descriptor)
        raise resource_guard.GuardError(
            f"sealed build report cannot be pinned: {error}"
        ) from error
    try:
        if (
            not stat.S_ISREG(metadata.st_mode)
            or not stat.S_ISREG(current.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_uid not in {0, os.geteuid()}
            or stat.S_IMODE(metadata.st_mode) & 0o022
            or not 1 <= metadata.st_size <= MAX_SEALED_BUILD_REPORT_BYTES
            or (metadata.st_dev, metadata.st_ino)
            != (current.st_dev, current.st_ino)
        ):
            raise resource_guard.GuardError("sealed build report metadata is unsafe")
        payload = bytearray()
        while len(payload) <= MAX_SEALED_BUILD_REPORT_BYTES:
            chunk = os.read(
                descriptor,
                min(64 * 1024, MAX_SEALED_BUILD_REPORT_BYTES + 1 - len(payload)),
            )
            if not chunk:
                break
            payload.extend(chunk)
        digest = hashlib.sha256(payload).hexdigest()
        if len(payload) != metadata.st_size or digest != expected_sha256:
            raise resource_guard.GuardError("sealed build report digest differs")
        try:
            report = json.loads(
                payload,
                object_pairs_hook=_strict_json_object,
                parse_constant=lambda _value: (_ for _ in ()).throw(
                    resource_guard.GuardError(
                        "sealed build report contains a non-finite number"
                    )
                ),
            )
        except resource_guard.GuardError:
            raise
        except (UnicodeError, ValueError, json.JSONDecodeError) as error:
            raise resource_guard.GuardError(
                "sealed build report is not strict JSON"
            ) from error
        if not isinstance(report, dict) or resource_guard._canonical_json(report) != payload:
            raise resource_guard.GuardError("sealed build report is not canonical JSON")
        report, _native_launch = _unwrap_native_sealed_build_report(report)
        expected_top = {
            "authenticated_source_seal_projection_sha256",
            "binary_path",
            "binary_sha256",
            "binary_size_bytes",
            "build_profile",
            "builds",
            "byte_equality",
            "candidate_generator",
            "minimum_build_physical_memory_bytes",
            "physical_memory_bytes_at_admission",
            "reproducible_build_count",
            "reviewed_cargo_binary_sha256",
            "reviewed_rustc_binary_sha256",
            "reviewed_source_closure",
            "reviewed_source_closure_descriptor_sha256",
            "schema",
            "source_commit",
            "source_date_epoch",
            "source_repo_dirty",
            "source_tree_sha256",
            "target_dir",
            "unit_graph_preflight",
            "verification_binary_path",
        }
        if set(report) != expected_top:
            raise resource_guard.GuardError("sealed build report fields are not exact")
        builds = report["builds"]
        if (
            report["schema"] != SEALED_BUILD_REPORT_SCHEMA
            or report["build_profile"] != "release"
            or report["reproducible_build_count"] != 2
            or report["source_repo_dirty"] is not False
            or not isinstance(builds, list)
            or len(builds) != 2
        ):
            raise resource_guard.GuardError("sealed build report policy is not exact")
        common_keys = {
            "authenticated_source_seal_projection_sha256",
            "build_inputs_sha256",
            "cargo_binary_sha256",
            "cargo_semantic_argv",
            "execution_policy_sha256",
            "normalized_unit_graph_sha256",
            "reviewed_source_closure_sha256",
            "runtime_gid",
            "runtime_uid",
            "rustc_binary_sha256",
            "source_commit",
            "source_date_epoch",
            "source_tree_sha256",
            "target",
        }
        outputs: list[tuple[str, int, str]] = []
        for index, raw_build in enumerate(builds, 1):
            if not isinstance(raw_build, dict) or set(raw_build) != {
                "identity",
                "identity_sha256",
                "output",
            }:
                raise resource_guard.GuardError("sealed build entry fields are not exact")
            identity = raw_build["identity"]
            output = raw_build["output"]
            if (
                not isinstance(identity, dict)
                or set(identity)
                != common_keys | {"ordinal", "source_snapshot_role", "target_role"}
                or identity["ordinal"] != index
                or not isinstance(output, dict)
                or set(output) != {"binary_path", "sha256", "size_bytes"}
                or not isinstance(output["binary_path"], str)
                or not isinstance(output["size_bytes"], int)
                or isinstance(output["size_bytes"], bool)
                or output["size_bytes"] <= 0
                or hashlib.sha256(resource_guard._canonical_json(identity)).hexdigest()
                != _report_hex(raw_build["identity_sha256"], "build identity digest")
            ):
                raise resource_guard.GuardError("sealed build identity is invalid")
            output_sha256 = _report_hex(output["sha256"], "build output digest")
            outputs.append((output_sha256, output["size_bytes"], output["binary_path"]))
        if outputs[0][:2] != outputs[1][:2] or outputs[0][2] == outputs[1][2]:
            raise resource_guard.GuardError("sealed builds are not independent and equal")
        equality = report["byte_equality"]
        generator = report["candidate_generator"]
        if (
            not isinstance(equality, dict)
            or set(equality) != {"algorithm", "equal", "sha256", "size_bytes"}
            or equality["algorithm"]
            != "sha256-size-and-final-descriptor-rehash-v1"
            or equality["equal"] is not True
            or (equality["sha256"], equality["size_bytes"]) != outputs[0][:2]
            or not isinstance(generator, dict)
            or set(generator) != {"selected_build_ordinal", "sha256", "size_bytes"}
            or generator["selected_build_ordinal"] != 1
            or (generator["sha256"], generator["size_bytes"]) != outputs[0][:2]
            or (report["binary_sha256"], report["binary_size_bytes"]) != outputs[0][:2]
            or report["binary_path"] != outputs[0][2]
            or report["verification_binary_path"] != outputs[1][2]
        ):
            raise resource_guard.GuardError("sealed build equality binding is invalid")
        pinned = PinnedSealedBuildReport(
            path=path.resolve(strict=True),
            descriptor=descriptor,
            device=metadata.st_dev,
            inode=metadata.st_ino,
            mode=metadata.st_mode,
            owner_uid=metadata.st_uid,
            size_bytes=metadata.st_size,
            modified_ns=metadata.st_mtime_ns,
            changed_ns=metadata.st_ctime_ns,
            sha256=digest,
            generator_sha256=_report_hex(generator["sha256"], "generator digest"),
            generator_size_bytes=generator["size_bytes"],
        )
        pinned.validate()
        return pinned
    except BaseException:
        os.close(descriptor)
        raise


def _validate_execution_copy(snapshot: ExecutableSnapshot) -> None:
    """Validate the private Darwin copy without rereading the source path."""

    if snapshot.execution_copy is not None:
        copy = snapshot.execution_copy
        copied_metadata, copied_sha256 = _hash_executable_descriptor(
            copy.file_descriptor
        )
        try:
            path_metadata = os.stat(copy.path, follow_symlinks=False)
            directory_metadata = os.fstat(copy.directory_descriptor)
            directory_path_metadata = os.stat(
                copy.path.parent, follow_symlinks=False
            )
        except OSError as error:
            raise resource_guard.GuardError(
                "Kagemusha private execution copy is unavailable"
            ) from error
        if (
            (copied_metadata.st_dev, copied_metadata.st_ino)
            != (copy.file_device, copy.file_inode)
            or (path_metadata.st_dev, path_metadata.st_ino)
            != (copy.file_device, copy.file_inode)
            or stat.S_IMODE(copied_metadata.st_mode) != 0o500
            or copied_metadata.st_size != snapshot.size_bytes
            or copied_sha256 != snapshot.sha256
            or (directory_metadata.st_dev, directory_metadata.st_ino)
            != (copy.directory_device, copy.directory_inode)
            or (directory_path_metadata.st_dev, directory_path_metadata.st_ino)
            != (copy.directory_device, copy.directory_inode)
            or stat.S_IMODE(directory_metadata.st_mode) != 0o500
        ):
            raise resource_guard.GuardError(
                "Kagemusha private execution copy changed after admission"
            )


def _validate_executable_unchanged(snapshot: ExecutableSnapshot) -> None:
    """Fail if the executable path, metadata, or bytes changed during the run."""

    pinned_metadata, pinned_sha256 = _hash_executable_descriptor(snapshot.descriptor)
    metadata, sha256 = _read_executable_identity(snapshot.path)
    expected_identity = (
        snapshot.device,
        snapshot.inode,
        snapshot.mode,
        snapshot.link_count,
        snapshot.owner_uid,
        snapshot.size_bytes,
        snapshot.modified_ns,
        snapshot.changed_ns,
    )
    if (
        _executable_stat_identity(pinned_metadata) != expected_identity
        or pinned_sha256 != snapshot.sha256
        or _executable_stat_identity(metadata) != expected_identity
        or sha256 != snapshot.sha256
    ):
        raise resource_guard.GuardError(
            "Kagemusha executable changed after admission"
        )
    _validate_execution_copy(snapshot)


def _execution_copy_name(staging_id: str) -> str:
    """Return the journal-bound private execution directory name."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    return f"{STAGING_PREFIX}{staging_id}-exec"


def _remove_directory_tree_at(parent_descriptor: int, name: str) -> None:
    """Remove one tree relative to a pinned parent without following symlinks."""

    if not name or name in {".", ".."} or Path(name).name != name:
        raise resource_guard.GuardError(
            "Kagemusha cleanup directory name is invalid"
        )
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

    def remove_contents(directory_descriptor: int) -> None:
        with os.scandir(directory_descriptor) as entries:
            children = list(entries)
        for entry in children:
            if not entry.is_dir(follow_symlinks=False):
                os.unlink(entry.name, dir_fd=directory_descriptor)
                continue
            before = entry.stat(follow_symlinks=False)
            child_descriptor = os.open(
                entry.name,
                directory_flags,
                dir_fd=directory_descriptor,
            )
            try:
                opened = os.fstat(child_descriptor)
                if (
                    not stat.S_ISDIR(opened.st_mode)
                    or not os.path.samestat(before, opened)
                    or opened.st_uid != os.geteuid()
                    or opened.st_mode & 0o077 != 0
                ):
                    raise resource_guard.GuardError(
                        "Kagemusha cleanup directory is untrusted or changed"
                    )
                os.fchmod(child_descriptor, 0o700)
                remove_contents(child_descriptor)
                current = os.stat(
                    entry.name,
                    dir_fd=directory_descriptor,
                    follow_symlinks=False,
                )
                if (
                    not stat.S_ISDIR(current.st_mode)
                    or not os.path.samestat(opened, current)
                ):
                    raise resource_guard.GuardError(
                        "Kagemusha cleanup directory identity changed"
                    )
                os.rmdir(entry.name, dir_fd=directory_descriptor)
            finally:
                os.close(child_descriptor)

    before = os.stat(name, dir_fd=parent_descriptor, follow_symlinks=False)
    if not stat.S_ISDIR(before.st_mode):
        raise resource_guard.GuardError(
            "Kagemusha cleanup target is not a directory"
        )
    descriptor = os.open(name, directory_flags, dir_fd=parent_descriptor)
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(opened.st_mode)
            or not os.path.samestat(before, opened)
            or opened.st_uid != os.geteuid()
            or opened.st_mode & 0o077 != 0
        ):
            raise resource_guard.GuardError(
                "Kagemusha cleanup directory is untrusted or changed"
            )
        os.fchmod(descriptor, 0o700)
        remove_contents(descriptor)
        current = os.stat(name, dir_fd=parent_descriptor, follow_symlinks=False)
        if (
            not stat.S_ISDIR(current.st_mode)
            or not os.path.samestat(opened, current)
        ):
            raise resource_guard.GuardError(
                "Kagemusha cleanup directory identity changed"
            )
        os.rmdir(name, dir_fd=parent_descriptor)
    finally:
        os.close(descriptor)


def _prepare_execution_copy(
    parent: PinnedOutputParent,
    snapshot: ExecutableSnapshot,
    staging_id: str,
    executable_name: str = BUNDLE_EXECUTABLE,
) -> None:
    """On Darwin, copy admitted fd bytes into a private disk-backed path."""

    if sys.platform != "darwin":
        return
    if snapshot.execution_copy is not None:
        raise resource_guard.GuardError("Kagemusha execution copy already exists")
    if (
        not executable_name
        or Path(executable_name).name != executable_name
        or executable_name in {".", ".."}
    ):
        raise resource_guard.GuardError("Kagemusha execution-copy name is invalid")
    parent.validate()
    directory_name = _execution_copy_name(staging_id)
    file_name = executable_name
    relative_file = f"{directory_name}/{file_name}"
    directory_descriptor = -1
    file_descriptor = -1
    created = False
    try:
        os.mkdir(directory_name, mode=0o700, dir_fd=parent.descriptor)
        created = True
        directory_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_DIRECTORY"):
            directory_flags |= os.O_DIRECTORY
        if hasattr(os, "O_NOFOLLOW"):
            directory_flags |= os.O_NOFOLLOW
        directory_descriptor = os.open(
            directory_name, directory_flags, dir_fd=parent.descriptor
        )
        directory_metadata = os.fstat(directory_descriptor)
        if (
            not stat.S_ISDIR(directory_metadata.st_mode)
            or directory_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(directory_metadata.st_mode) != 0o700
        ):
            raise resource_guard.GuardError(
                "Kagemusha execution-copy directory is unsafe"
            )

        write_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            write_flags |= os.O_NOFOLLOW
        writer = os.open(relative_file, write_flags, 0o600, dir_fd=parent.descriptor)
        try:
            offset = 0
            while offset < snapshot.size_bytes:
                chunk = os.pread(
                    snapshot.descriptor,
                    min(1024 * 1024, snapshot.size_bytes - offset),
                    offset,
                )
                if not chunk:
                    raise resource_guard.GuardError(
                        "admitted Kagemusha executable ended during private copy"
                    )
                resource_guard._write_all(writer, chunk)
                offset += len(chunk)
            os.fsync(writer)
            os.fchmod(writer, 0o500)
            os.fsync(writer)
        finally:
            os.close(writer)

        read_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            read_flags |= os.O_NOFOLLOW
        file_descriptor = os.open(
            relative_file, read_flags, dir_fd=parent.descriptor
        )
        copied_metadata, copied_sha256 = _hash_executable_descriptor(file_descriptor)
        path_metadata = os.stat(
            relative_file, dir_fd=parent.descriptor, follow_symlinks=False
        )
        if (
            _executable_stat_identity(copied_metadata)
            != _executable_stat_identity(path_metadata)
            or copied_metadata.st_size != snapshot.size_bytes
            or copied_sha256 != snapshot.sha256
            or stat.S_IMODE(copied_metadata.st_mode) != 0o500
        ):
            raise resource_guard.GuardError(
                "Kagemusha private execution copy does not match admission"
            )
        os.fchmod(directory_descriptor, 0o500)
        os.fsync(directory_descriptor)
        os.fsync(parent.descriptor)
        parent.validate()
        snapshot.execution_copy = ExecutionCopy(
            directory_name=directory_name,
            file_name=file_name,
            path=parent.path / directory_name / file_name,
            directory_descriptor=directory_descriptor,
            file_descriptor=file_descriptor,
            directory_device=directory_metadata.st_dev,
            directory_inode=directory_metadata.st_ino,
            file_device=copied_metadata.st_dev,
            file_inode=copied_metadata.st_ino,
            mode=copied_metadata.st_mode,
            size_bytes=copied_metadata.st_size,
            sha256=copied_sha256,
        )
        directory_descriptor = -1
        file_descriptor = -1
    except BaseException:
        for descriptor in (file_descriptor, directory_descriptor):
            if descriptor >= 0:
                os.close(descriptor)
        if created:
            try:
                _remove_directory_tree_at(parent.descriptor, directory_name)
                os.fsync(parent.descriptor)
            except BaseException:
                pass
        raise


def _release_execution_copy(
    parent: PinnedOutputParent, snapshot: ExecutableSnapshot
) -> None:
    """Validate and unlock a private copy so journal cleanup can remove it."""

    copy = snapshot.execution_copy
    if copy is None:
        return
    _validate_execution_copy(snapshot)
    parent.validate(require_path=False)
    os.fchmod(copy.directory_descriptor, 0o700)
    os.fsync(copy.directory_descriptor)
    for descriptor_name in ("file_descriptor", "directory_descriptor"):
        descriptor = getattr(copy, descriptor_name)
        if descriptor >= 0:
            os.close(descriptor)
            setattr(copy, descriptor_name, -1)
    snapshot.execution_copy = None


def _canonical_positive_decimal(value: str, label: str) -> int:
    """Parse one nonzero canonical unsigned decimal field."""

    if (
        not value
        or not value.isascii()
        or not value.isdigit()
        or value.startswith("0")
    ):
        raise resource_guard.GuardError(
            f"Kagemusha memory-capacity {label} is not canonical decimal"
        )
    parsed = int(value)
    if parsed <= 0 or parsed > (1 << 64) - 1:
        raise resource_guard.GuardError(
            f"Kagemusha memory-capacity {label} is outside u64"
        )
    return parsed


def _validate_memory_capacity_outcome(
    payload: bytes,
) -> GenerationMemoryCapacityV1:
    """Validate the pinned bundle's sole bounded memory-policy record."""

    if (
        not payload
        or len(payload) > MAX_MEMORY_CAPACITY_OUTCOME_BYTES
        or not payload.endswith(b"\n")
    ):
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity outcome is absent, oversized, or non-canonical"
        )
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity outcome is not ASCII"
        ) from error
    if text.count("\n") != 1:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity outcome must contain exactly one line"
        )
    fields = text[:-1].split(" ")
    if len(fields) != 6 or fields[0] != MEMORY_CAPACITY_SCHEMA:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity outcome schema is invalid"
        )
    expected_keys = ("physical", "ceiling", "absolute", "profile", "policy")
    values: dict[str, str] = {}
    for expected_key, field in zip(expected_keys, fields[1:]):
        key, separator, value = field.partition("=")
        if separator != "=" or key != expected_key or not value:
            raise resource_guard.GuardError(
                "Kagemusha memory-capacity outcome fields are invalid"
            )
        values[key] = value
    physical = _canonical_positive_decimal(values["physical"], "physical capacity")
    ceiling = _canonical_positive_decimal(values["ceiling"], "safety ceiling")
    absolute = _canonical_positive_decimal(values["absolute"], "absolute maximum")
    if absolute != ABSOLUTE_MAX_MEMORY_BYTES:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity absolute maximum differs from the launcher contract"
        )
    if ceiling > absolute or ceiling > physical:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity safety ceiling exceeds its admitted bounds"
        )
    if values["profile"] != MEMORY_ENFORCEMENT_PROFILE:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity enforcement profile is unsupported"
        )
    if values["policy"] != MEMORY_CAPACITY_POLICY:
        raise resource_guard.GuardError(
            "Kagemusha memory-capacity policy is unsupported"
        )
    return GenerationMemoryCapacityV1(
        effective_physical_capacity_bytes=physical,
        safety_ceiling_bytes=ceiling,
        absolute_maximum_bytes=absolute,
        enforcement_profile=values["profile"],
        policy=values["policy"],
    )


def _apply_optional_memory_limit_bytes(
    capacity: GenerationMemoryCapacityV1, requested_gib: float | None
) -> int:
    """Use the exact Rust ceiling, allowing only an explicit lower override."""

    ceiling = capacity.safety_ceiling_bytes
    if requested_gib is None:
        return ceiling
    if not math.isfinite(requested_gib) or requested_gib <= 0:
        raise resource_guard.GuardError("--max-memory-gib must be greater than zero")
    if requested_gib > ceiling / BYTES_PER_GIB:
        raise resource_guard.GuardError(
            "--max-memory-gib may lower but cannot raise the Kagemusha safety ceiling"
        )
    requested = int(requested_gib * BYTES_PER_GIB)
    if requested == 0:
        raise resource_guard.GuardError(
            "--max-memory-gib is too small to represent a positive byte limit"
        )
    return requested


def _physical_memory_bytes() -> int:
    """Return host memory for the non-shipping benchmark launcher only."""

    if sys.platform == "darwin":
        try:
            completed = subprocess.run(
                ["/usr/sbin/sysctl", "-n", "hw.memsize"],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                encoding="ascii",
                timeout=5,
            )
            if completed.returncode == 0:
                value = int(completed.stdout.strip())
                if value > 0:
                    return value
        except (OSError, ValueError, subprocess.TimeoutExpired):
            pass
    try:
        pages = int(os.sysconf("SC_PHYS_PAGES"))
        page_size = int(os.sysconf("SC_PAGE_SIZE"))
        value = pages * page_size
        if value > 0:
            return value
    except (OSError, ValueError, TypeError):
        pass
    return 0


def _candidate_child_environment(
    temporary_directory: Path | None = None,
) -> dict[str, str]:
    """Return the complete environment admitted into source-sealed code.

    In particular, loader injection, allocator overrides, Rust/Python runtime
    knobs, SDK discovery, and caller-controlled tool resolution never cross the
    evidence boundary. Generation spools use the already-admitted output
    filesystem; read-only control operations use the OS temporary directory.
    """

    temporary_path = Path("/tmp") if temporary_directory is None else temporary_directory
    temporary_text = os.fspath(temporary_path)
    if not temporary_path.is_absolute() or not temporary_text.isprintable():
        raise resource_guard.GuardError(
            "candidate temporary directory must be an absolute path without controls"
        )
    return {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": FIXED_CANDIDATE_CHILD_PATH,
        "TMPDIR": os.fspath(temporary_path),
    }


def _effective_memory_limit_bytes(requested_gib: float | None) -> int:
    """Derive the non-shipping benchmark launcher's external ceiling."""

    physical_memory = _physical_memory_bytes()
    if physical_memory <= 0:
        raise resource_guard.GuardError(
            "could not determine installed physical memory"
        )
    physical_half = max(1, physical_memory // 2)
    ceiling = min(ABSOLUTE_MAX_MEMORY_BYTES, physical_half)
    if requested_gib is None:
        return ceiling
    if not math.isfinite(requested_gib) or requested_gib <= 0:
        raise resource_guard.GuardError("--max-memory-gib must be greater than zero")
    if requested_gib > ceiling / BYTES_PER_GIB:
        raise resource_guard.GuardError(
            "--max-memory-gib may lower but cannot raise the Kagemusha safety ceiling"
        )
    requested = int(requested_gib * BYTES_PER_GIB)
    if requested == 0:
        raise resource_guard.GuardError(
            "--max-memory-gib is too small to represent a positive byte limit"
        )
    return requested


def _is_kagemusha_heavy_process(row: resource_guard.ProcessRow) -> bool:
    """Identify a V4 generator that is not owned by this supervisor."""

    if row.uid != os.getuid() or row.pid == os.getpid():
        return False
    name = Path(row.command).name.lower()
    return name == "kagemusha_recursive_spend_v4_bundle" or name.startswith(
        "kagemusha_recu"
    )


def _reject_foreign_kagemusha_jobs() -> None:
    """Fail closed instead of racing an unowned candidate generator."""

    jobs = [
        row for row in resource_guard._process_rows() if _is_kagemusha_heavy_process(row)
    ]
    if jobs:
        first = min(jobs, key=lambda row: row.pid)
        raise resource_guard.GuardError(
            "pre-existing Kagemusha V4 generator is outside this guard "
            f"(pid={first.pid}, pgid={first.process_group_id})"
        )


def _prepare_report_directory(path: Path) -> tuple[Path, Path]:
    """Create one new owner-private resource evidence directory."""

    path.mkdir(parents=True, mode=0o700, exist_ok=False)
    os.chmod(path, 0o700)
    return path / "kagemusha_resource.jsonl", path / "kagemusha_resource_summary.json"


def _validate_generation_command(command: Sequence[str]) -> None:
    """Require a prebuilt bundle generator, never a compiler or shell wrapper."""

    executable = Path(command[0]).name
    if executable.endswith(".exe"):
        executable = executable[:-4]
    if executable != BUNDLE_EXECUTABLE:
        raise resource_guard.GuardError(
            "Kagemusha resource guard requires the prebuilt "
            "kagemusha_recursive_spend_v4_bundle executable; build it before "
            "entering the reviewed generation guard"
        )
    if len(command) < 2 or command[1] != "generate-candidate":
        raise resource_guard.GuardError(
            "Kagemusha resource guard supervises only generate-candidate"
        )
    if any(
        option in command
        for option in (
            STAGING_ID_OPTION,
            STAGING_NAME_OPTION,
            OUTPUT_PARENT_FD_OPTION,
            MEMORY_LIMIT_OPTION,
            GENERATOR_BINARY_SHA256_OPTION,
            SEALED_BUILD_REPORT_SHA256_OPTION,
        )
    ):
        raise resource_guard.GuardError(
            "Kagemusha staging and output-parent options are reserved for the resource guard"
        )


def _required_option(command: Sequence[str], option: str) -> str:
    """Return one exact two-argument option from the bundle command."""

    positions = [index for index, value in enumerate(command) if value == option]
    if len(positions) != 1:
        raise resource_guard.GuardError(
            f"Kagemusha generation command requires exactly one {option}"
        )
    position = positions[0]
    if position + 1 >= len(command) or command[position + 1].startswith("--"):
        raise resource_guard.GuardError(
            f"Kagemusha generation command has no value for {option}"
        )
    return command[position + 1]


def _run_text_command(command: Sequence[str], description: str) -> str:
    """Run one fixed filesystem-inspection command with a short timeout."""

    try:
        completed = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=5,
            env=_candidate_child_environment(),
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise resource_guard.GuardError(f"could not inspect {description}") from error
    if completed.returncode != 0:
        detail = completed.stderr.strip() or f"exit status {completed.returncode}"
        raise resource_guard.GuardError(
            f"could not inspect {description}: {detail}"
        )
    return completed.stdout


def _filesystem_type(path: Path) -> str:
    """Return the normalized filesystem type containing an output parent."""

    if sys.platform.startswith("linux"):
        stat_command = next(
            (
                candidate
                for candidate in ("/usr/bin/stat", "/bin/stat")
                if Path(candidate).is_file()
            ),
            None,
        )
        if stat_command is None:
            raise resource_guard.GuardError("filesystem stat utility is unavailable")
        output = _run_text_command(
            [stat_command, "--file-system", "--format=%T", "--", str(path)],
            "Kagemusha output filesystem type",
        )
        filesystem_type = output.strip().lower()
    elif sys.platform == "darwin":
        df_output = _run_text_command(
            ["/bin/df", "-P", str(path)], "Kagemusha output filesystem device"
        )
        rows = [line.split() for line in df_output.splitlines() if line.strip()]
        if len(rows) < 2 or not rows[-1]:
            raise resource_guard.GuardError(
                "Kagemusha output filesystem device is malformed"
            )
        device = rows[-1][0]
        mount_output = _run_text_command(
            ["/sbin/mount"], "Kagemusha output filesystem mount table"
        )
        prefix = f"{device} on "
        matching = [line for line in mount_output.splitlines() if line.startswith(prefix)]
        if len(matching) != 1 or " (" not in matching[0]:
            raise resource_guard.GuardError(
                "Kagemusha output filesystem has no unique mount-table entry"
            )
        filesystem_type = matching[0].split(" (", 1)[1].split(",", 1)[0]
        filesystem_type = filesystem_type.rstrip(")").strip().lower()
    else:
        raise resource_guard.GuardError(
            "Kagemusha output filesystem validation is unsupported on this platform"
        )
    if not filesystem_type:
        raise resource_guard.GuardError("Kagemusha output filesystem type is empty")
    return filesystem_type


def _valid_output_leaf(output_name: str) -> bool:
    """Return whether an output name is one safe, non-reserved path leaf."""

    return (
        bool(output_name)
        and output_name not in {".", ".."}
        and Path(output_name).name == output_name
        and "/" not in output_name
        and (os.altsep is None or os.altsep not in output_name)
        and not output_name.startswith((STAGING_PREFIX, JOURNAL_PREFIX))
    )


def _staging_name(staging_id: str) -> str:
    """Return the sole candidate staging name for one launcher id."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    return f"{STAGING_PREFIX}{staging_id}-work"


def _guarded_output_name(staging_id: str) -> str:
    """Return the hidden decoy output leaf exposed to the generator child."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    return f"{STAGING_PREFIX}{staging_id}-{GUARDED_OUTPUT_SUFFIX}"


def _replace_required_option(
    command: Sequence[str], option: str, value: str
) -> list[str]:
    """Copy a command while replacing one already validated option value."""

    _required_option(command, option)
    replaced = list(command)
    position = replaced.index(option)
    replaced[position + 1] = value
    return replaced


def _prepare_guarded_command(
    command: Sequence[str],
) -> tuple[list[str], PinnedOutputParent, CandidatePublicationContract]:
    """Bind one unguessable staging prefix to this supervised invocation."""

    out_dir = Path(_required_option(command, "--out-dir"))
    source_commit = _required_option(command, "--source-commit")
    source_tree_sha256 = _required_option(command, "--source-tree-sha256")
    output_name = out_dir.name
    if not _valid_output_leaf(output_name):
        raise resource_guard.GuardError(
            "Kagemusha output path must end in one directory name"
        )
    parent = out_dir.parent if out_dir.parent != Path("") else Path(".")
    try:
        parent = parent.resolve(strict=True)
    except OSError as error:
        raise resource_guard.GuardError(
            f"Kagemusha output parent is unavailable: {error}"
        ) from error
    if not parent.is_dir():
        raise resource_guard.GuardError("Kagemusha output parent is not a directory")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(parent, flags)
        opened = os.fstat(descriptor)
        current = os.stat(parent, follow_symlinks=False)
    except OSError as error:
        if "descriptor" in locals():
            os.close(descriptor)
        raise resource_guard.GuardError(
            f"Kagemusha output parent cannot be pinned: {error}"
        ) from error
    if (
        not stat.S_ISDIR(opened.st_mode)
        or not stat.S_ISDIR(current.st_mode)
        or (opened.st_dev, opened.st_ino) != (current.st_dev, current.st_ino)
    ):
        os.close(descriptor)
        raise resource_guard.GuardError(
            "Kagemusha output parent changed while it was pinned"
        )
    try:
        filesystem_type = _filesystem_type(parent)
    except BaseException:
        os.close(descriptor)
        raise
    if filesystem_type not in DISK_BACKED_OUTPUT_FILESYSTEM_TYPES:
        os.close(descriptor)
        raise resource_guard.GuardError(
            "Kagemusha output parent is not on an admitted disk-backed filesystem "
            f"({filesystem_type})"
        )
    try:
        filesystem = os.fstatvfs(descriptor)
    except OSError as error:
        os.close(descriptor)
        raise resource_guard.GuardError(
            f"Kagemusha output free space is unavailable: {error}"
        ) from error
    free_bytes = filesystem.f_bavail * filesystem.f_frsize
    if free_bytes < MINIMUM_OUTPUT_FREE_BYTES:
        os.close(descriptor)
        raise resource_guard.GuardError(
            "Kagemusha output parent has less than 16 GiB available"
        )
    pinned = PinnedOutputParent(
        path=parent,
        descriptor=descriptor,
        device=opened.st_dev,
        inode=opened.st_ino,
        output_name=output_name,
        filesystem_type=filesystem_type,
        free_bytes_at_admission=free_bytes,
    )
    staging_id = secrets.token_hex(STAGING_ID_HEX_LENGTH // 2)
    staging_name = _staging_name(staging_id)
    guarded_out_dir = str(parent / _guarded_output_name(staging_id))
    contract = CandidatePublicationContract(
        requested_out_dir=str(parent / output_name),
        guarded_out_dir=guarded_out_dir,
        output_name=output_name,
        output_parent_descriptor=descriptor,
        source_commit=source_commit,
        source_tree_sha256=source_tree_sha256,
        staging_id=staging_id,
        staging_name=staging_name,
    )
    guarded_command = _replace_required_option(command, "--out-dir", guarded_out_dir)
    guarded_command.extend(
        [
            STAGING_ID_OPTION,
            staging_id,
            STAGING_NAME_OPTION,
            staging_name,
            OUTPUT_PARENT_FD_OPTION,
            str(descriptor),
        ]
    )
    contract.validate(pinned)
    return guarded_command, pinned, contract


def _validate_guarded_generation_command(
    command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    sealed_build_report: PinnedSealedBuildReport,
    parent: PinnedOutputParent,
    contract: CandidatePublicationContract,
    memory_limit_bytes: int,
) -> None:
    """Recheck every launcher-owned child argument after resource acceptance."""

    contract.validate(parent)
    if (
        len(command) < 2
        or command[0] != executable_snapshot.execution_path()
        or command[1] != "generate-candidate"
        or _required_option(command, "--out-dir") != contract.guarded_out_dir
        or _required_option(command, STAGING_ID_OPTION) != contract.staging_id
        or _required_option(command, STAGING_NAME_OPTION) != contract.staging_name
        or _required_option(command, OUTPUT_PARENT_FD_OPTION)
        != str(contract.output_parent_descriptor)
        or _required_option(command, "--source-commit") != contract.source_commit
        or _required_option(command, "--source-tree-sha256")
        != contract.source_tree_sha256
        or _required_option(command, MEMORY_LIMIT_OPTION) != str(memory_limit_bytes)
        or _required_option(command, GENERATOR_BINARY_SHA256_OPTION)
        != executable_snapshot.sha256
        or _required_option(command, SEALED_BUILD_REPORT_SHA256_OPTION)
        != sealed_build_report.sha256
    ):
        raise resource_guard.GuardError(
            "Kagemusha guarded generation command changed before publication"
        )


def _cleanup_staging(parent: PinnedOutputParent, staging_id: str) -> int:
    """Remove only residue carrying this guard's unguessable staging id."""

    if (
        len(staging_id) != STAGING_ID_HEX_LENGTH
        or not staging_id.isascii()
        or any(byte not in "0123456789abcdef" for byte in staging_id)
    ):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")

    parent.validate(require_path=False)
    prefix = f"{STAGING_PREFIX}{staging_id}-"
    removed = 0
    with os.scandir(parent.descriptor) as entries:
        for entry in entries:
            if not entry.name.startswith(prefix):
                continue
            metadata = entry.stat(follow_symlinks=False)
            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_uid != os.geteuid()
                or metadata.st_mode & 0o077 != 0
            ):
                raise resource_guard.GuardError(
                    "refusing to remove untrusted Kagemusha staging residue "
                    f"{entry.name}"
                )
            _remove_directory_tree_at(parent.descriptor, entry.name)
            removed += 1

    parent.validate(require_path=False)
    with os.scandir(parent.descriptor) as entries:
        if any(entry.name.startswith(prefix) for entry in entries):
            raise resource_guard.GuardError(
                "Kagemusha staging residue remains after guarded cleanup"
            )
    return removed


def _create_staging_directory(
    parent: PinnedOutputParent, staging_id: str
) -> PinnedStagingDirectory:
    """Create the exact hidden work directory relative to the pinned parent fd."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    parent.validate()
    name = _staging_name(staging_id)
    os.mkdir(name, mode=0o700, dir_fd=parent.descriptor)
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = -1
    try:
        named = os.stat(name, dir_fd=parent.descriptor, follow_symlinks=False)
        descriptor = os.open(name, flags, dir_fd=parent.descriptor)
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(named.st_mode)
            or not stat.S_ISDIR(opened.st_mode)
            or not os.path.samestat(named, opened)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != 0o700
        ):
            raise resource_guard.GuardError(
                "Kagemusha staging directory has unsafe metadata"
            )
        os.fsync(parent.descriptor)
        parent.validate()
        return PinnedStagingDirectory(
            staging_id=staging_id,
            name=name,
            descriptor=descriptor,
            device=opened.st_dev,
            inode=opened.st_ino,
        )
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise


def _valid_staging_id(staging_id: str) -> bool:
    """Return whether a staging id is canonical guard-generated lower hex."""

    return (
        len(staging_id) == STAGING_ID_HEX_LENGTH
        and staging_id.isascii()
        and all(byte in "0123456789abcdef" for byte in staging_id)
    )


def _journal_name(staging_id: str) -> str:
    """Return the reserved journal leaf for one validated staging id."""

    if not _valid_staging_id(staging_id):
        raise resource_guard.GuardError("Kagemusha staging id is invalid")
    return f"{JOURNAL_PREFIX}{staging_id}{JOURNAL_SUFFIX}"


def _journal_document(
    parent: PinnedOutputParent,
    staging_id: str,
    *,
    output_name: str | None = None,
) -> dict[str, object]:
    """Build the exact durable recovery record for one guarded invocation."""

    stored_output_name = parent.output_name if output_name is None else output_name
    if not _valid_output_leaf(stored_output_name):
        raise resource_guard.GuardError("Kagemusha journal output leaf is invalid")
    return {
        "execution_copy_name": _execution_copy_name(staging_id),
        "output_name": stored_output_name,
        "parent_device": parent.device,
        "parent_inode": parent.inode,
        "recovery_scope": "same_output_parent",
        "schema": JOURNAL_SCHEMA,
        "staging_id": staging_id,
        "staging_prefix": f"{STAGING_PREFIX}{staging_id}-",
    }


def _journal_stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return stable metadata used to bind one opened journal path."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _create_run_journal(parent: PinnedOutputParent, staging_id: str) -> None:
    """Durably create the recovery marker before the generator can spawn."""

    parent.validate()
    name = _journal_name(staging_id)
    payload = resource_guard._canonical_json(_journal_document(parent, staging_id))
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = -1
    created_identity: tuple[int, int] | None = None
    try:
        descriptor = os.open(name, flags, 0o600, dir_fd=parent.descriptor)
        metadata = os.fstat(descriptor)
        created_identity = (metadata.st_dev, metadata.st_ino)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise resource_guard.GuardError(
                "Kagemusha run journal has unsafe metadata"
            )
        resource_guard._write_all(descriptor, payload)
        os.fsync(descriptor)
        os.close(descriptor)
        descriptor = -1
        parent.validate(require_path=False)
        os.fsync(parent.descriptor)
    except BaseException as error:
        if descriptor >= 0:
            os.close(descriptor)
            descriptor = -1
        cleanup_error: BaseException | None = None
        if created_identity is not None:
            try:
                current = os.stat(
                    name, dir_fd=parent.descriptor, follow_symlinks=False
                )
                if (current.st_dev, current.st_ino) != created_identity:
                    raise resource_guard.GuardError(
                        "partial Kagemusha run journal identity changed"
                    )
                os.unlink(name, dir_fd=parent.descriptor)
                os.fsync(parent.descriptor)
            except FileNotFoundError:
                pass
            except BaseException as failure:
                cleanup_error = failure
        if cleanup_error is not None:
            raise resource_guard.GuardError(
                "could not remove a partial Kagemusha run journal"
            ) from cleanup_error
        raise error


def _read_run_journal(
    parent: PinnedOutputParent, name: str
) -> tuple[str, dict[str, object]]:
    """Read and strictly validate one marker relative to the pinned parent."""

    if not name.startswith(JOURNAL_PREFIX) or not name.endswith(JOURNAL_SUFFIX):
        raise resource_guard.GuardError("Kagemusha run journal name is malformed")
    staging_id = name[len(JOURNAL_PREFIX) : -len(JOURNAL_SUFFIX)]
    if name != _journal_name(staging_id):
        raise resource_guard.GuardError("Kagemusha run journal id is malformed")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=parent.descriptor)
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_uid != os.geteuid()
            or opened.st_nlink != 1
            or stat.S_IMODE(opened.st_mode) != 0o600
            or opened.st_size <= 0
            or opened.st_size > MAX_JOURNAL_BYTES
        ):
            raise resource_guard.GuardError(
                "Kagemusha run journal has unsafe metadata"
            )
        payload = bytearray()
        while len(payload) <= MAX_JOURNAL_BYTES:
            chunk = os.read(descriptor, MAX_JOURNAL_BYTES + 1 - len(payload))
            if not chunk:
                break
            payload.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    current = os.stat(name, dir_fd=parent.descriptor, follow_symlinks=False)
    if not (
        _journal_stat_identity(opened)
        == _journal_stat_identity(after)
        == _journal_stat_identity(current)
        and len(payload) == opened.st_size
    ):
        raise resource_guard.GuardError("Kagemusha run journal changed while read")
    try:
        document = json.loads(payload)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise resource_guard.GuardError("Kagemusha run journal is invalid JSON") from error
    if not isinstance(document, dict):
        raise resource_guard.GuardError("Kagemusha run journal is not an object")
    stored_output_name = document.get("output_name")
    if not isinstance(stored_output_name, str) or not _valid_output_leaf(
        stored_output_name
    ):
        raise resource_guard.GuardError(
            "Kagemusha run journal has an unsafe output leaf"
        )
    expected = _journal_document(
        parent, staging_id, output_name=stored_output_name
    )
    if document != expected or bytes(payload) != resource_guard._canonical_json(expected):
        raise resource_guard.GuardError("Kagemusha run journal is not canonical or bound")
    return staging_id, document


def _remove_run_journal(parent: PinnedOutputParent, staging_id: str) -> None:
    """Remove one already validated journal and durably sync its parent."""

    name = _journal_name(staging_id)
    recovered_id, _document = _read_run_journal(parent, name)
    if recovered_id != staging_id:
        raise resource_guard.GuardError("Kagemusha run journal id changed")
    os.unlink(name, dir_fd=parent.descriptor)
    os.fsync(parent.descriptor)
    parent.validate(require_path=False)


def _run_journal_exists(parent: PinnedOutputParent, staging_id: str) -> bool:
    """Return whether the exact marker name still has a directory entry."""

    parent.validate(require_path=False)
    try:
        os.stat(
            _journal_name(staging_id),
            dir_fd=parent.descriptor,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        return False
    return True


def _output_leaf_exists(parent: PinnedOutputParent, output_name: str) -> bool:
    """Return whether one validated output leaf is visible in the pinned parent."""

    if not _valid_output_leaf(output_name):
        raise resource_guard.GuardError("Kagemusha output leaf is invalid")
    try:
        os.stat(output_name, dir_fd=parent.descriptor, follow_symlinks=False)
    except FileNotFoundError:
        return False
    return True


def _cleanup_guarded_run(
    parent: PinnedOutputParent,
    staging_id: str,
    *,
    publication_confirmed: bool = False,
) -> int:
    """Clean staging, preserving recovery evidence for uncertain publication."""

    _recovered_id, document = _read_run_journal(parent, _journal_name(staging_id))
    output_name = document["output_name"]
    if not isinstance(output_name, str):  # guarded by _read_run_journal
        raise resource_guard.GuardError("Kagemusha journal output leaf is invalid")
    removed = _cleanup_staging(parent, staging_id)
    if _output_leaf_exists(parent, output_name) and not publication_confirmed:
        raise resource_guard.GuardError(
            "Kagemusha output became visible without confirmed publication; "
            "the recovery journal was retained for reconciliation"
        )
    _remove_run_journal(parent, staging_id)
    return removed


def _recover_stale_runs(parent: PinnedOutputParent) -> int:
    """Recover marker-bound residue in this same parent while holding the locks."""

    parent.validate()
    with os.scandir(parent.descriptor) as entries:
        names = sorted(
            entry.name for entry in entries if entry.name.startswith(JOURNAL_PREFIX)
        )
    if len(names) > MAX_RECOVERABLE_JOURNALS:
        raise resource_guard.GuardError("too many stale Kagemusha run journals")
    removed = 0
    for name in names:
        staging_id, document = _read_run_journal(parent, name)
        removed += _cleanup_staging(parent, staging_id)
        output_name = document["output_name"]
        if not isinstance(output_name, str):  # guarded by _read_run_journal
            raise resource_guard.GuardError(
                "Kagemusha journal output leaf is invalid"
            )
        if _output_leaf_exists(parent, output_name):
            raise resource_guard.GuardError(
                "stale Kagemusha journal records an already-visible output; "
                "manual reconciliation is required"
            )
        _remove_run_journal(parent, staging_id)
    with os.scandir(parent.descriptor) as entries:
        if any(entry.name.startswith(STAGING_PREFIX) for entry in entries):
            raise resource_guard.GuardError(
                "unjournaled Kagemusha staging residue exists in the output parent"
            )
    parent.validate()
    return removed


def _run_candidate_session_wrapper(argv: Sequence[str]) -> int:
    """Own a pinned-executable body and kill it when its supervisor disappears."""

    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--lifeline-fd", required=True, type=int)
    parser.add_argument("--control-fd", required=True, type=int)
    parser.add_argument("--executable-fd", required=True, type=int)
    parser.add_argument("--execution-path", required=True)
    parser.add_argument("--held-lock-fd", action="append", default=[], type=int)
    parser.add_argument("--child-directory-fd", action="append", default=[], type=int)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise resource_guard.GuardError("candidate session command is empty")
    descriptors = (
        args.lifeline_fd,
        args.control_fd,
        args.executable_fd,
        *args.held_lock_fd,
        *args.child_directory_fd,
    )
    if len(set(descriptors)) != len(descriptors):
        raise resource_guard.GuardError(
            "candidate session control descriptors overlap"
        )
    resource_guard._require_pipe_descriptor(args.lifeline_fd, "lifeline")
    resource_guard._require_pipe_descriptor(args.control_fd, "control")
    for descriptor in args.held_lock_fd:
        metadata = os.fstat(descriptor)
        if (
            descriptor < 3
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise resource_guard.GuardError("held lock descriptor is invalid")
    for descriptor in args.child_directory_fd:
        metadata = os.fstat(descriptor)
        if descriptor < 3 or not stat.S_ISDIR(metadata.st_mode):
            raise resource_guard.GuardError("child directory descriptor is invalid")
    executable_metadata = os.fstat(args.executable_fd)
    if args.executable_fd < 3:
        raise resource_guard.GuardError("pinned executable descriptor is invalid")
    _validate_executable_metadata(executable_metadata)
    if command[0] != args.execution_path:
        raise resource_guard.GuardError(
            "candidate session did not target its pinned executable descriptor"
        )
    execution_path_metadata = os.stat(args.execution_path, follow_symlinks=True)
    if _executable_stat_identity(execution_path_metadata) != _executable_stat_identity(
        executable_metadata
    ):
        raise resource_guard.GuardError(
            "candidate execution path does not identify its pinned bytes"
        )
    received_signal = 0

    def receive_signal(signum: int, _frame: object) -> None:
        nonlocal received_signal
        if received_signal == 0:
            received_signal = signum

    for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, receive_signal)

    operation = command[1] if len(command) >= 2 else ""
    capture_publication_outcome = operation == "publish-staged-candidate"
    capture_memory_capacity = operation == MEMORY_CAPACITY_OPERATION
    capture_control_outcome = capture_publication_outcome or capture_memory_capacity
    control_stdout = tempfile.TemporaryFile() if capture_control_outcome else None
    control_stderr = tempfile.TemporaryFile() if capture_control_outcome else None
    child: subprocess.Popen[bytes] | None = None
    try:
        if resource_guard._lifeline_closed(args.lifeline_fd, 0):
            return 1
        child = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(
                args.executable_fd,
                *args.child_directory_fd,
            ),
            start_new_session=True,
            env=_candidate_child_environment(
                Path(os.environ.get("TMPDIR", "/tmp"))
            ),
            stdout=control_stdout,
            stderr=control_stderr,
        )
        process_group_id = child.pid
        if process_group_id <= 1 or process_group_id == os.getpgrp():
            raise resource_guard.GuardError(
                "candidate body did not enter its own process group"
            )
        resource_guard._write_wrapper_control(
            args.control_fd, f"READY {process_group_id}"
        )

        completed: tuple[int, int] | None = None
        while completed is None:
            completed = resource_guard._wait4_nonblocking(child)
            if completed is not None:
                break
            if received_signal or resource_guard._lifeline_closed(
                args.lifeline_fd, 0.05
            ):
                resource_guard._terminate_owned_group(child, process_group_id)
                return 1
        if completed is None:
            raise resource_guard.GuardError(
                "candidate session lost the body return code"
            )
        returncode, kernel_peak_rss_bytes = completed
        lingering = resource_guard._process_group_exists(process_group_id)
        if lingering:
            resource_guard._terminate_owned_group(child, process_group_id)
        resource_guard._write_wrapper_control(
            args.control_fd,
            "EXIT "
            f"{returncode} 0 {1 if lingering else 0} 0 {kernel_peak_rss_bytes}",
        )
        if capture_control_outcome:
            if control_stdout is None or control_stderr is None:
                raise resource_guard.GuardError(
                    "bundle control outcome capture was not initialized"
                )
            captured: list[bytes] = []
            maximum_bytes = (
                MAX_PUBLICATION_OUTCOME_BYTES
                if capture_publication_outcome
                else MAX_MEMORY_CAPACITY_OUTCOME_BYTES
            )
            for stream in (control_stdout, control_stderr):
                stream.flush()
                stream.seek(0)
                payload = stream.read(maximum_bytes + 1)
                if len(payload) > maximum_bytes:
                    raise resource_guard.GuardError(
                        "bundle control outcome exceeded its fixed bound"
                    )
                captured.append(payload)
            if capture_publication_outcome:
                expected_final_path = _required_option(command, "--out-dir")
                resource_guard._write_wrapper_control(
                    args.control_fd,
                    _publication_control_record(
                        returncode,
                        captured[0],
                        captured[1],
                        expected_final_path=expected_final_path,
                    ),
                )
            elif returncode == 0:
                if captured[1]:
                    raise resource_guard.GuardError(
                        "successful memory-capacity query emitted stderr"
                    )
                _validate_memory_capacity_outcome(captured[0])
                resource_guard._write_wrapper_control(
                    args.control_fd,
                    captured[0].decode("ascii").removesuffix("\n"),
                )
        return 1 if lingering else resource_guard._exit_status(returncode)
    except BaseException as error:
        if child is not None:
            try:
                resource_guard._terminate_owned_group(child, child.pid)
            except BaseException:
                pass
        try:
            resource_guard._write_wrapper_control(args.control_fd, "ERROR")
        except BaseException:
            pass
        print(f"candidate session wrapper failed: {error}", file=sys.stderr)
        return 1
    finally:
        for stream in (control_stdout, control_stderr):
            if stream is not None:
                stream.close()
        for descriptor in descriptors:
            resource_guard._close_descriptor(descriptor)


def _spawn_pinned_guarded_session(
    command: Sequence[str],
    environment: dict[str, str],
    held_lock_descriptors: Sequence[int],
    child_directory_descriptors: Sequence[int],
    executable_snapshot: ExecutableSnapshot,
) -> resource_guard.GuardedSession:
    """Spawn a lifeline wrapper that inherits the admitted executable fd."""

    _validate_executable_unchanged(executable_snapshot)
    if not command or command[0] != executable_snapshot.execution_path():
        raise resource_guard.GuardError(
            "guarded command must execute the admitted descriptor path"
        )
    execution_descriptor = executable_snapshot.execution_descriptor()
    execution_path = executable_snapshot.execution_path()
    lifeline_reader, lifeline_writer = resource_guard._pipe()
    control_reader, control_writer = resource_guard._pipe()
    # The resource guard's spawner signature carries a caller environment, but
    # source-sealed execution accepts only this explicit projection. In
    # particular, never forward LD_*/DYLD_* loader hooks or an ambient PATH.
    child_environment = _candidate_child_environment(
        Path(environment.get("TMPDIR", "/tmp"))
    )
    wrapper_command = [
        sys.executable,
        str(Path(__file__).resolve()),
        CANDIDATE_SESSION_WRAPPER_FLAG,
        "--lifeline-fd",
        str(lifeline_reader),
        "--control-fd",
        str(control_writer),
        "--executable-fd",
        str(execution_descriptor),
        "--execution-path",
        execution_path,
    ]
    for descriptor in held_lock_descriptors:
        wrapper_command.extend(("--held-lock-fd", str(descriptor)))
    for descriptor in child_directory_descriptors:
        wrapper_command.extend(("--child-directory-fd", str(descriptor)))
    wrapper_command.extend(("--", *command))
    wrapper: subprocess.Popen[bytes] | None = None
    control: resource_guard.SessionControl | None = None
    try:
        wrapper = subprocess.Popen(
            wrapper_command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(
                lifeline_reader,
                control_writer,
                execution_descriptor,
                *held_lock_descriptors,
                *child_directory_descriptors,
            ),
            start_new_session=True,
            env=child_environment,
        )
        for descriptor in (lifeline_reader, control_writer):
            resource_guard._close_descriptor(descriptor)
        lifeline_reader = -1
        control_writer = -1
        control = resource_guard.SessionControl(control_reader)
        control_reader = -1
        ready = control.read_line(
            timeout=resource_guard.SESSION_READY_TIMEOUT_SECONDS,
            description="candidate lifeline wrapper readiness",
        )
        fields = ready.split()
        if len(fields) != 2 or fields[0] != "READY" or not fields[1].isdigit():
            raise resource_guard.GuardError(
                "candidate lifeline wrapper emitted invalid readiness"
            )
        process_group_id = int(fields[1])
        if process_group_id <= 1 or process_group_id == wrapper.pid:
            raise resource_guard.GuardError(
                "candidate lifeline wrapper reported an invalid body process group"
            )
        session = resource_guard.GuardedSession(
            wrapper, process_group_id, lifeline_writer, control
        )
        lifeline_writer = -1
        control = None
        return session
    except BaseException:
        resource_guard._close_descriptor(lifeline_writer)
        lifeline_writer = -1
        if wrapper is not None:
            try:
                wrapper.wait(timeout=resource_guard.TERM_GRACE_SECONDS * 2 + 1)
            except subprocess.TimeoutExpired:
                try:
                    resource_guard._terminate_owned_group(wrapper, wrapper.pid)
                except BaseException:
                    pass
        if control is not None:
            control.close()
        raise
    finally:
        for descriptor in (
            lifeline_reader,
            lifeline_writer,
            control_reader,
            control_writer,
        ):
            if descriptor >= 0:
                resource_guard._close_descriptor(descriptor)


def _run_guarded_with_pinned_executable(
    command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    **guard_options: object,
) -> int:
    """Run the resource guard with a session spawner that inherits the exec fd.

    Callers must put ``executable_snapshot.execution_path()`` in ``command[0]``
    and retain/close the snapshot around this call.
    """

    original_spawner = resource_guard._spawn_guarded_session

    def spawn(
        child_command: Sequence[str],
        environment: dict[str, str],
        held_lock_descriptors: Sequence[int],
        child_directory_descriptors: Sequence[int],
    ) -> resource_guard.GuardedSession:
        return _spawn_pinned_guarded_session(
            child_command,
            environment,
            held_lock_descriptors,
            child_directory_descriptors,
            executable_snapshot,
        )

    resource_guard._spawn_guarded_session = spawn
    try:
        return resource_guard._run_guarded(command, **guard_options)
    finally:
        resource_guard._spawn_guarded_session = original_spawner


def _run_pinned_bundle_command(
    command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    *,
    held_lock_descriptors: Sequence[int] = (),
    child_directory_descriptors: Sequence[int] = (),
    temporary_directory: Path | None = None,
) -> GenerationMemoryCapacityV1 | None:
    """Run one pinned bundle operation under a supervisor-death lifeline."""

    if len(command) < 2 or command[0] != executable_snapshot.execution_path():
        raise resource_guard.GuardError(
            "bundle control command must execute the admitted descriptor path"
        )
    operation = command[1]
    if operation == MEMORY_CAPACITY_OPERATION:
        if len(command) != 2 or child_directory_descriptors:
            raise resource_guard.GuardError(
                "memory-capacity query must be read-only and argument-free"
            )
        operation_description = "memory-capacity query"
        timeout_seconds = 30
    elif operation == "publish-staged-candidate":
        operation_description = "candidate publisher"
        timeout_seconds = 300
    else:
        raise resource_guard.GuardError(
            "bundle control command is not an admitted read-only query or publisher"
        )
    environment = _candidate_child_environment(temporary_directory)
    session = _spawn_pinned_guarded_session(
        command,
        environment,
        held_lock_descriptors,
        child_directory_descriptors,
        executable_snapshot,
    )
    received_signal = 0

    def receive_signal(signum: int, _frame: object) -> None:
        nonlocal received_signal
        if received_signal == 0:
            received_signal = signum

    watched_signals = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
    previous_handlers = {
        signum: signal.getsignal(signum) for signum in watched_signals
    }
    for signum in watched_signals:
        signal.signal(signum, receive_signal)
    interrupted = 0
    try:
        deadline = time.monotonic() + timeout_seconds
        while session.wrapper.poll() is None and time.monotonic() < deadline:
            if received_signal:
                interrupted = received_signal
                resource_guard._terminate_owned_group(
                    session.wrapper, session.process_group_id
                )
                break
            time.sleep(0.05)
        if interrupted:
            raise resource_guard.GuardError(
                f"Kagemusha {operation_description} interrupted by signal {interrupted}"
            )
        if session.wrapper.poll() is None:
            resource_guard._terminate_owned_group(
                session.wrapper, session.process_group_id
            )
            raise resource_guard.GuardError(
                f"timed out during Kagemusha {operation_description}"
            )
        wrapper_exit = session.control.read_line(
            timeout=resource_guard.CONTROL_RECORD_TIMEOUT_SECONDS,
            description=f"Kagemusha {operation_description} exit status",
        )
        fields = wrapper_exit.split()
        if (
            len(fields) != 6
            or fields[0] != "EXIT"
            or fields[2] not in {"0", "1"}
            or fields[3] not in {"0", "1"}
            or fields[4] not in {"0", "1"}
            or not fields[5].isdigit()
        ):
            raise resource_guard.GuardError(
                f"Kagemusha {operation_description} wrapper emitted invalid exit status"
            )
        try:
            returncode = int(fields[1])
        except ValueError as error:
            raise resource_guard.GuardError(
                f"Kagemusha {operation_description} emitted a non-integer status"
            ) from error
        if fields[2] == "1":
            raise resource_guard.GuardError(
                f"Kagemusha {operation_description} lost its supervisor lifeline"
            )
        if fields[3] == "1":
            raise resource_guard.GuardError(
                f"Kagemusha {operation_description} left a lingering process group"
            )
        if fields[4] == "1":
            raise resource_guard.GuardError(
                f"Kagemusha {operation_description} was cancelled"
            )
        if operation == MEMORY_CAPACITY_OPERATION:
            if returncode != 0:
                raise resource_guard.GuardError(
                    f"pinned Kagemusha memory-capacity query failed with status {returncode}"
                )
            capacity_record = session.control.read_line(
                timeout=resource_guard.CONTROL_RECORD_TIMEOUT_SECONDS,
                description="Kagemusha memory-capacity machine outcome",
            )
            return _validate_memory_capacity_outcome(
                f"{capacity_record}\n".encode("ascii")
            )
        publication_record = session.control.read_line(
            timeout=resource_guard.CONTROL_RECORD_TIMEOUT_SECONDS,
            description="candidate publisher machine outcome",
        )
        expected_final_path = _required_option(command, "--out-dir")
        publication_status = _validate_publication_control_record(
            publication_record,
            returncode=returncode,
            expected_final_path=expected_final_path,
        )
        if returncode == 0:
            if publication_status != "committed":
                raise resource_guard.GuardError(
                    "candidate publisher wrapper contradicted its successful exit"
                )
        if returncode != 0:
            if returncode == 75:
                if publication_status != "commit-uncertain":
                    raise resource_guard.GuardError(
                        "candidate publisher wrapper contradicted its commit-uncertain exit"
                    )
                raise resource_guard.GuardError(
                    "validated Kagemusha candidate publication reached an uncertain "
                    "post-rename durability boundary (status 75); retain the run journal "
                    "and reconcile the visible final leaf"
                )
            raise resource_guard.GuardError(
                "validated Kagemusha candidate publication failed with status "
                f"{returncode}"
            )
        return None
    finally:
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)
        session.close()
        if interrupted:
            prior = previous_handlers[interrupted]
            if callable(prior):
                prior(interrupted, None)


def _publication_control_record(
    returncode: int,
    stdout_payload: bytes,
    stderr_payload: bytes,
    *,
    expected_final_path: str,
) -> str:
    """Validate the child outcome and return one fixed-size wrapper record.

    The generic lifeline protocol intentionally caps every record at 255 bytes.
    Publication output can include a long canonical path, so the trusted wrapper
    validates the full bounded payload locally and sends only its status plus
    fixed-size path and payload digests to the supervisor.
    """

    if returncode == 0:
        if stderr_payload:
            raise resource_guard.GuardError(
                "committed candidate publisher emitted unexpected stderr"
            )
        _validate_publication_outcome(
            stdout_payload,
            expected_status="committed",
            expected_final_path=expected_final_path,
        )
        status = "committed"
        outcome_payload = stdout_payload
    elif returncode == 75:
        if stdout_payload:
            raise resource_guard.GuardError(
                "commit-uncertain candidate publisher emitted unexpected stdout"
            )
        _validate_publication_outcome(
            stderr_payload,
            expected_status="commit-uncertain",
            expected_final_path=expected_final_path,
        )
        status = "commit-uncertain"
        outcome_payload = stderr_payload
    else:
        status = "failed"
        outcome_payload = stdout_payload + b"\x00" + stderr_payload

    path_digest = hashlib.sha256(os.fsencode(expected_final_path)).hexdigest()
    outcome_digest = hashlib.sha256(outcome_payload).hexdigest()
    return (
        f"{PUBLICATION_CONTROL_RECORD} {status} "
        f"{path_digest} {outcome_digest}"
    )


def _validate_publication_control_record(
    record: str,
    *,
    returncode: int,
    expected_final_path: str,
) -> str:
    """Validate the wrapper's fixed-size, path-bound publication result."""

    fields = record.split()
    if len(fields) != 4 or fields[0] != PUBLICATION_CONTROL_RECORD:
        raise resource_guard.GuardError(
            "candidate publisher wrapper emitted an invalid machine outcome record"
        )
    status, path_digest, outcome_digest = fields[1:]
    expected_status = (
        "committed"
        if returncode == 0
        else "commit-uncertain"
        if returncode == 75
        else "failed"
    )
    if status != expected_status:
        raise resource_guard.GuardError(
            "candidate publisher wrapper outcome contradicts its exit status"
        )
    for label, digest in (
        ("path", path_digest),
        ("outcome", outcome_digest),
    ):
        if (
            len(digest) != PUBLICATION_CONTROL_DIGEST_HEX_LENGTH
            or any(character not in "0123456789abcdef" for character in digest)
        ):
            raise resource_guard.GuardError(
                f"candidate publisher wrapper {label} digest is invalid"
            )
    expected_path_digest = hashlib.sha256(
        os.fsencode(expected_final_path)
    ).hexdigest()
    if path_digest != expected_path_digest:
        raise resource_guard.GuardError(
            "candidate publisher wrapper outcome names the wrong final path"
        )
    return status


def _validate_publication_outcome(
    payload: bytes,
    *,
    expected_status: str,
    expected_final_path: str,
) -> None:
    """Validate the publisher's exact, path-bound machine outcome."""

    if not payload or len(payload) > MAX_PUBLICATION_OUTCOME_BYTES or not payload.endswith(b"\n"):
        raise resource_guard.GuardError(
            "candidate publisher machine outcome is absent, oversized, or non-canonical"
        )
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise resource_guard.GuardError(
            "candidate publisher machine outcome is not ASCII"
        ) from error
    if text.count("\n") != 1:
        raise resource_guard.GuardError(
            "candidate publisher machine outcome must contain exactly one line"
        )
    fields = text[:-1].split(" ")
    if len(fields) != 6 or fields[0] != PUBLICATION_OUTCOME_SCHEMA:
        raise resource_guard.GuardError(
            "candidate publisher machine outcome schema is invalid"
        )
    expected_keys = (
        "status",
        "final_path_encoding",
        "final_path_hex",
        "parent_directory_durable",
        "parent_sync_error_utf8_hex",
    )
    values: dict[str, str] = {}
    for expected_key, field in zip(expected_keys, fields[1:]):
        key, separator, value = field.partition("=")
        if separator != "=" or key != expected_key or not value:
            raise resource_guard.GuardError(
                "candidate publisher machine outcome fields are invalid"
            )
        values[key] = value
    final_path_hex = values["final_path_hex"]
    if (
        len(final_path_hex) % 2 != 0
        or any(character not in "0123456789abcdef" for character in final_path_hex)
        or bytes.fromhex(final_path_hex) != os.fsencode(expected_final_path)
    ):
        raise resource_guard.GuardError(
            "candidate publisher machine outcome names the wrong final path"
        )
    if values["status"] != expected_status or values["final_path_encoding"] != "bytes-hex":
        raise resource_guard.GuardError(
            "candidate publisher machine outcome status or path encoding is invalid"
        )
    sync_error = values["parent_sync_error_utf8_hex"]
    if expected_status == "committed":
        if values["parent_directory_durable"] != "1" or sync_error != "-":
            raise resource_guard.GuardError(
                "committed candidate publisher outcome is not durable"
            )
        return
    if values["parent_directory_durable"] != "0" or sync_error == "-":
        raise resource_guard.GuardError(
            "commit-uncertain candidate publisher outcome lacks its sync failure"
        )
    try:
        sync_error_bytes = bytes.fromhex(sync_error)
        sync_error_text = sync_error_bytes.decode("utf-8")
    except (ValueError, UnicodeDecodeError) as error:
        raise resource_guard.GuardError(
            "commit-uncertain candidate publisher sync failure is not UTF-8 hex"
        ) from error
    if not sync_error_text:
        raise resource_guard.GuardError(
            "commit-uncertain candidate publisher sync failure is empty"
        )


def _validate_staged_child_result(
    guarded_command: Sequence[str],
    executable_snapshot: ExecutableSnapshot,
    sealed_build_report: PinnedSealedBuildReport,
    output_parent: PinnedOutputParent,
    contract: CandidatePublicationContract,
    staging: PinnedStagingDirectory,
    memory_limit_bytes: int,
) -> None:
    """Prove the successful child left only its exact hidden staging directory."""

    _validate_executable_unchanged(executable_snapshot)
    _validate_guarded_generation_command(
        guarded_command,
        executable_snapshot,
        sealed_build_report,
        output_parent,
        contract,
        memory_limit_bytes,
    )
    staging.validate_named(output_parent, contract)
    if _output_leaf_exists(output_parent, contract.output_name):
        raise resource_guard.GuardError(
            "Kagemusha generator bypassed guarded publication"
        )
    try:
        os.stat(
            _guarded_output_name(contract.staging_id),
            dir_fd=output_parent.descriptor,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        pass
    else:
        raise resource_guard.GuardError(
            "Kagemusha generator directly renamed its staging directory"
        )


def _query_generation_memory_capacity(
    executable_snapshot: ExecutableSnapshot,
    *,
    held_lock_descriptors: Sequence[int] = (),
) -> GenerationMemoryCapacityV1:
    """Query memory policy through the exact pinned bytes used for generation."""

    _validate_executable_unchanged(executable_snapshot)
    outcome = _run_pinned_bundle_command(
        [executable_snapshot.execution_path(), MEMORY_CAPACITY_OPERATION],
        executable_snapshot,
        held_lock_descriptors=held_lock_descriptors,
    )
    _validate_executable_unchanged(executable_snapshot)
    if outcome is None:
        raise resource_guard.GuardError(
            "pinned Kagemusha memory-capacity query returned no policy"
        )
    return outcome


def _publish_staged_candidate(
    contract: CandidatePublicationContract,
    staging: PinnedStagingDirectory,
    executable_snapshot: ExecutableSnapshot,
    output_parent: PinnedOutputParent,
    memory_limit_bytes: int,
    held_lock_descriptors: Sequence[int] = (),
) -> int:
    """Authenticate and atomically publish staging only after the guard verdict."""

    _validate_executable_unchanged(executable_snapshot)
    contract.validate(output_parent)
    staging.validate_named(output_parent, contract)
    try:
        os.stat(
            contract.output_name,
            dir_fd=output_parent.descriptor,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        pass
    else:
        raise resource_guard.GuardError(
            "Kagemusha candidate output appeared before guarded publication"
        )
    publish_command = [
        executable_snapshot.execution_path(),
        "publish-staged-candidate",
        "--out-dir",
        contract.requested_out_dir,
        STAGING_ID_OPTION,
        contract.staging_id,
        STAGING_NAME_OPTION,
        contract.staging_name,
        OUTPUT_PARENT_FD_OPTION,
        str(contract.output_parent_descriptor),
        "--source-commit",
        contract.source_commit,
        "--source-tree-sha256",
        contract.source_tree_sha256,
        MEMORY_LIMIT_OPTION,
        str(memory_limit_bytes),
    ]
    outcome = _run_pinned_bundle_command(
        publish_command,
        executable_snapshot,
        held_lock_descriptors=held_lock_descriptors,
        child_directory_descriptors=(output_parent.descriptor,),
        temporary_directory=output_parent.path,
    )
    if outcome is not None:
        raise resource_guard.GuardError(
            "candidate publisher returned a memory-capacity result"
        )
    _validate_executable_unchanged(executable_snapshot)
    staging.validate_published(output_parent, contract)
    return 1


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Run Kagemusha V4 candidate generation beneath the non-raiseable "
            "lower of the 64 GiB production ceiling or half of physical RAM."
        )
    )
    parser.add_argument("--resource-report", required=True, type=Path)
    parser.add_argument("--sealed-build-report", required=True, type=Path)
    parser.add_argument("--sealed-build-report-sha256", required=True)
    parser.add_argument(
        "--max-memory-gib",
        type=float,
        help="optionally lower, but never raise, the production memory ceiling",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return parser


def _candidate_main(argv: Sequence[str] | None = None) -> int:
    """Acquire the global lock and supervise exactly one generation command."""

    args = _parser().parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        print("Kagemusha resource guard requires a command after --", file=sys.stderr)
        return 2
    output_parent: PinnedOutputParent | None = None
    executable_snapshot: ExecutableSnapshot | None = None
    sealed_build_report: PinnedSealedBuildReport | None = None
    staging: PinnedStagingDirectory | None = None
    try:
        _validate_generation_command(command)
        executable_snapshot = _snapshot_executable(command[0], BUNDLE_EXECUTABLE)
        sealed_build_report = _open_sealed_build_report(
            args.sealed_build_report, args.sealed_build_report_sha256
        )
        if (
            executable_snapshot.sha256 != sealed_build_report.generator_sha256
            or executable_snapshot.size_bytes
            != sealed_build_report.generator_size_bytes
        ):
            raise resource_guard.GuardError(
                "admitted generator differs from the sealed double-build report"
            )
        command.extend(
            (
                GENERATOR_BINARY_SHA256_OPTION,
                executable_snapshot.sha256,
                SEALED_BUILD_REPORT_SHA256_OPTION,
                sealed_build_report.sha256,
            )
        )
        command[0] = str(executable_snapshot.path)
        guarded_command, output_parent, contract = _prepare_guarded_command(command)
        with resource_guard._host_lock(
            resource_guard.HEAVY_JOB_LOCK_PATH, description="memory-heavy job"
        ) as heavy_lock:
            with resource_guard._host_lock(
                LOCK_PATH, description="Kagemusha generator"
            ) as kagemusha_lock:
                _reject_foreign_kagemusha_jobs()
                recovered_staging = _recover_stale_runs(output_parent)
                _create_run_journal(output_parent, contract.staging_id)
                publication_confirmed = False

                def cleanup_candidate() -> int:
                    if staging is not None:
                        staging.close()
                    _release_execution_copy(output_parent, executable_snapshot)
                    return _cleanup_guarded_run(
                        output_parent,
                        contract.staging_id,
                        publication_confirmed=publication_confirmed,
                    )

                try:
                    _prepare_execution_copy(
                        output_parent, executable_snapshot, contract.staging_id
                    )
                    memory_capacity = _query_generation_memory_capacity(
                        executable_snapshot,
                        held_lock_descriptors=(heavy_lock, kagemusha_lock),
                    )
                    memory_limit = _apply_optional_memory_limit_bytes(
                        memory_capacity, args.max_memory_gib
                    )
                    guarded_command.extend((MEMORY_LIMIT_OPTION, str(memory_limit)))
                    jsonl_path, summary_path = _prepare_report_directory(
                        args.resource_report
                    )

                    def publish_candidate() -> int:
                        nonlocal publication_confirmed
                        if staging is None:
                            raise resource_guard.GuardError(
                                "Kagemusha staging identity is unavailable"
                            )
                        sealed_build_report.validate()
                        result = _publish_staged_candidate(
                            contract,
                            staging,
                            executable_snapshot,
                            output_parent,
                            memory_limit,
                            held_lock_descriptors=(heavy_lock, kagemusha_lock),
                        )
                        publication_confirmed = True
                        return result

                    def validate_candidate() -> None:
                        if staging is None:
                            raise resource_guard.GuardError(
                                "Kagemusha staging identity is unavailable"
                            )
                        sealed_build_report.validate()
                        _validate_staged_child_result(
                            guarded_command,
                            executable_snapshot,
                            sealed_build_report,
                            output_parent,
                            contract,
                            staging,
                            memory_limit,
                        )

                    guarded_command[0] = executable_snapshot.execution_path()
                    staging = _create_staging_directory(
                        output_parent, contract.staging_id
                    )
                    return _run_guarded_with_pinned_executable(
                        guarded_command,
                        executable_snapshot,
                        report_path=jsonl_path,
                        summary_path=summary_path,
                        memory_limit_bytes=memory_limit,
                        maximum_memory_bytes=ABSOLUTE_MAX_MEMORY_BYTES,
                        absolute_memory_ceiling_bytes=ABSOLUTE_MAX_MEMORY_BYTES,
                        memory_enforcement_mode=(
                            resource_guard.MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT
                        ),
                        held_lock_descriptors=(heavy_lock, kagemusha_lock),
                        child_directory_descriptors=(output_parent.descriptor,),
                        sample_interval_seconds=SAMPLE_INTERVAL_SECONDS,
                        physical_footprint_interval_seconds=(
                            SAMPLE_INTERVAL_SECONDS
                        ),
                        post_run_cleanup=cleanup_candidate,
                        post_run_validation=validate_candidate,
                        post_success_finalize=publish_candidate,
                        report_context={
                            "executable_identity": (
                                executable_snapshot.report_context()
                            ),
                            "output_parent": output_parent.report_context(),
                            "same_parent_recovered_staging_directories": (
                                recovered_staging
                            ),
                            "publication_contract": contract.report_context(),
                            "generation_memory_enforcement_profile": (
                                MEMORY_ENFORCEMENT_PROFILE
                            ),
                            "generation_memory_capacity": (
                                memory_capacity.report_context()
                            ),
                            "generation_memory_limit_bytes": memory_limit,
                            "sealed_candidate_build_report": {
                                "generator_sha256": sealed_build_report.generator_sha256,
                                "generator_size_bytes": sealed_build_report.generator_size_bytes,
                                "sha256": sealed_build_report.sha256,
                                "size_bytes": sealed_build_report.size_bytes,
                            },
                            "staging_id": contract.staging_id,
                        },
                        child_environment=_candidate_child_environment(
                            output_parent.path
                        ),
                    )
                except BaseException:
                    if _run_journal_exists(output_parent, contract.staging_id):
                        cleanup_candidate()
                    raise
    except resource_guard.LockUnavailable as error:
        print(f"Kagemusha resource guard refused to start: {error}", file=sys.stderr)
        return resource_guard.LOCK_UNAVAILABLE_EXIT_CODE
    except (resource_guard.GuardError, OSError) as error:
        print(f"Kagemusha resource guard failed closed: {error}", file=sys.stderr)
        return 1
    finally:
        if staging is not None:
            staging.close()
        if output_parent is not None:
            output_parent.close()
        if executable_snapshot is not None:
            executable_snapshot.close()
        if sealed_build_report is not None:
            sealed_build_report.close()


def main(argv: Sequence[str] | None = None) -> int:
    """Dispatch only to the strict max-RSS-or-footprint candidate runner."""

    arguments = list(sys.argv[1:] if argv is None else argv)
    option_prefix = (
        arguments[: arguments.index("--")] if "--" in arguments else arguments
    )
    if "--report" in option_prefix:
        # TODO: Restore generic acceptance supervision only after it uses the
        # same scoped 250 ms max(RSS, physical-footprint) enforcement as the
        # strict candidate path. The retired RSS-only path caused host Jetsam.
        print(
            "Kagemusha V4 --report mode is retired because its RSS-only "
            "supervisor cannot bound Darwin physical footprint; use the "
            "strict --resource-report candidate workflow",
            file=sys.stderr,
        )
        return 2
    return _candidate_main(arguments)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == CANDIDATE_SESSION_WRAPPER_FLAG:
        raise SystemExit(_run_candidate_session_wrapper(sys.argv[2:]))
    raise SystemExit(main())
