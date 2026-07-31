#!/usr/bin/env python3
"""Admit one root-published Kagemusha generated candidate and its evidence."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
from typing import Any, Mapping

from scripts import kagemusha_root_published_build as published_build


RECEIPT_SCHEMA = "iroha.kagemusha.root_published_generated_candidate.v1"
PUBLICATION_PROTOCOL = "iroha.kagemusha.distinct_uid_root_atomic_publish.v1"
RECEIPT_FILE_NAME = "root-published-generated-candidate.json"
CANDIDATE_DIR_NAME = "candidate"
RESOURCE_REPORT_DIR_NAME = "resource-report"
WORKER_LAUNCH_RECEIPT_FILE_NAME = "generation-worker-launch.json"
WORKER_LAUNCH_RECEIPT_SCHEMA = "boi.taira.generation_worker_launch.v1"
GENERATION_JSONL_FILE_NAME = "kagemusha_resource.jsonl"
GENERATION_SUMMARY_FILE_NAME = "kagemusha_resource_summary.json"
PUBLICATION_STATUS = "root_published_boi_generation_output"
PROVISIONAL_PUBLICATION_STATUS = "provisional_boi_generation_worker_output"
PROVISIONAL_CROSS_STAGE_STATUS = (
    "blocked_pending_root_descriptor_copy_atomic_publication_receipt"
)
ARTIFACT_TREE_DOMAIN = (
    b"iroha.kagemusha.root-published-generated-candidate.v1\0"
)
SUBTREE_DOMAIN = b"iroha.kagemusha.root-published-generated-subtree.v1\0"
MAX_RECEIPT_BYTES = 64 * 1024
MAX_WORKER_LAUNCH_RECEIPT_BYTES = 64 * 1024
MAX_GENERATION_SUMMARY_BYTES = 16 * 1024 * 1024
MAX_GENERATION_JSONL_BYTES = 64 * 1024 * 1024
MAX_TREE_ENTRIES = 1024
MAX_GENERATION_MEMORY_BYTES = 256 * 1024 * 1024
GENERATION_SAMPLE_INTERVAL_SECONDS = 0.05
NORMALIZED_DIRECTORY_MODE = 0o555
NORMALIZED_FILE_MODES = frozenset({0o444, 0o555})
TRUSTED_LAUNCH_OWNER_GID = 0
GENERATION_STAGING_PREFIX = ".kagemusha-v4-staging-"
BUNDLE_EXECUTABLE_FILE_NAME = "kagemusha_recursive_spend_v4_bundle"
MINIMUM_STORAGE_AT_ADMISSION_BYTES = 64 * 1024**3
MINIMUM_POST_BUILD_STORAGE_RESERVE_BYTES = 16 * 1024**3
MAX_RECORDED_STORAGE_BYTES = 1024**6

RECEIPT_KEYS = frozenset(
    {
        "artifact_root",
        "artifact_tree_sha256",
        "build_uid",
        "build_user_name",
        "candidate_build_artifact_tree_sha256",
        "candidate_build_receipt_path",
        "candidate_build_receipt_sha256",
        "candidate_dir_path",
        "candidate_tree_sha256",
        "generation_resource_report_path",
        "generation_resource_report_tree_sha256",
        "generation_summary_path",
        "generation_summary_sha256",
        "production_closure_tree_sha256",
        "provisional_cross_stage_status",
        "provisional_generation_publication_status",
        "publication_protocol",
        "publication_status",
        "reviewed_source_closure_descriptor_sha256",
        "schema",
        "source_commit",
        "source_tree_sha256",
        "toolchain_provenance_sha256",
        "worker_launch_receipt_sha256",
    }
)
WORKER_LAUNCH_RECEIPT_KEYS = frozenset(
    {
        "build_uid",
        "build_user_name",
        "candidate_build_receipt_path",
        "candidate_build_receipt_sha256",
        "candidate_output_leaf",
        "generation_command_sha256",
        "resource_report_leaf",
        "schema",
        "storage_available_bytes_after_generation",
        "storage_available_bytes_at_admission",
        "storage_device",
        "storage_minimum_available_bytes",
        "storage_post_build_reserve_bytes",
        "worker_root",
        "worker_root_device",
        "worker_root_inode",
    }
)
GENERATION_SUMMARY_KEYS = frozenset(
    {
        "child_exit_code",
        "ended_utc",
        "event",
        "exit_reason",
        "exit_status",
        "evidence_peak_rss_bytes",
        "kernel_peak_rss_bytes",
        "kernel_peak_rss_method",
        "kernel_peak_rss_scope",
        "memory_limit_bytes",
        "peak_memory_bytes",
        "peak_physical_footprint_bytes",
        "peak_rss_bytes",
        "post_run_cleanup",
        "post_run_cleanup_removed",
        "post_run_validation",
        "post_success_finalize",
        "post_success_finalize_result",
        "report_context",
        "sample_count",
        "sample_interval_seconds",
        "schema_version",
        "started_utc",
        "supervisor_pid",
    }
)
GENERATION_START_KEYS = frozenset(
    {
        "event",
        "memory_limit_bytes",
        "report_context",
        "sample_interval_seconds",
        "schema_version",
        "started_utc",
        "supervisor_pid",
    }
)
GENERATION_SPAWN_KEYS = frozenset(
    {
        "event",
        "process_group_id",
        "schema_version",
        "timestamp_utc",
        "wrapper_pid",
    }
)
GENERATION_SAMPLE_KEYS = frozenset(
    {
        "accounting_method",
        "elapsed_seconds",
        "event",
        "memory_bytes",
        "memory_limit_bytes",
        "physical_footprint_bytes",
        "process_count",
        "process_group_id",
        "rss_bytes",
        "schema_version",
        "timestamp_utc",
    }
)
REPORT_CONTEXT_KEYS = frozenset(
    {
        "cross_stage_status",
        "executable_identity",
        "output_parent",
        "publication_status",
        "root_published_build",
        "same_parent_recovered_staging_directories",
        "staging_id",
    }
)
ROOT_PUBLISHED_BUILD_CONTEXT_KEYS = frozenset(
    {
        "artifact_root",
        "artifact_tree_sha256",
        "build_uid",
        "build_user_name",
        "receipt",
        "receipt_sha256",
    }
)
EXECUTABLE_IDENTITY_KEYS = frozenset(
    {
        "build_profile",
        "canonical_path",
        "execution",
        "sha256",
        "size_bytes",
        "stat_identity",
    }
)
EXECUTABLE_STAT_IDENTITY_KEYS = frozenset(
    {
        "changed_ns",
        "device",
        "inode",
        "link_count",
        "mode",
        "modified_ns",
        "owner_uid",
    }
)
PINNED_EXECUTION_CONTEXT_KEYS = frozenset({"descriptor_path", "method"})
DARWIN_EXECUTION_CONTEXT_KEYS = frozenset(
    {
        "canonical_path",
        "directory_device",
        "directory_inode",
        "directory_name",
        "file_device",
        "file_inode",
        "file_name",
        "method",
        "mode",
        "sha256",
        "size_bytes",
    }
)
OUTPUT_PARENT_CONTEXT_KEYS = frozenset(
    {
        "admission",
        "canonical_path",
        "device",
        "filesystem_type",
        "free_bytes_at_admission",
        "inode",
        "minimum_free_bytes",
        "output_name",
    }
)
CANDIDATE_FILE_NAMES = frozenset(
    {
        "candidate-manifest.json",
        "candidate-manifest.norito",
        "candidate-manifest.norito.sha256",
        "step-eq.params-ipa.krv4",
        "step-eq.proving-key.krv4",
        "step-eq.verifying-key.krv4",
        "step-eq.bootstrap-witness.krv4",
        "step-ep.params-ipa.krv4",
        "step-ep.proving-key.krv4",
        "step-ep.verifying-key.krv4",
        "step-ep.bootstrap-witness.krv4",
        "topup-finality-roster-v4.norito",
    }
)
RESOURCE_REPORT_FILE_NAMES = frozenset(
    {
        GENERATION_JSONL_FILE_NAME,
        GENERATION_SUMMARY_FILE_NAME,
    }
)


class PublishedGeneratedCandidateError(RuntimeError):
    """A root-published generated candidate failed closed."""


@dataclass(frozen=True)
class AdmittedPublishedGeneratedCandidate:
    """One immutable generated candidate and its root publication evidence."""

    receipt: Path
    receipt_sha256: str
    artifact_root: Path
    artifact_tree_sha256: str
    candidate_dir: Path
    candidate_tree_sha256: str
    generation_resource_report: Path
    generation_resource_report_tree_sha256: str
    generation_summary: Path
    generation_summary_sha256: str
    generation_jsonl: Path
    generation_jsonl_sha256: str
    worker_launch_receipt: Path
    candidate_build: published_build.AdmittedPublishedBuild
    worker_launch_receipt_sha256: str
    generation_command_sha256: str
    worker_root: Path
    worker_root_device: int
    worker_root_inode: int
    build_user_name: str
    build_uid: int
    production_closure_tree_sha256: str
    toolchain_provenance_sha256: str
    reviewed_source_closure_descriptor_sha256: str
    source_commit: str
    source_tree_sha256: str


def _lower_hex(value: object, length: int) -> bool:
    return (
        isinstance(value, str)
        and re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is not None
    )


def _nonzero_lower_hex(value: object, length: int) -> bool:
    return _lower_hex(value, length) and value != "0" * length


def _exact_int(value: object, *, minimum: int = 0) -> bool:
    return (
        not isinstance(value, bool)
        and isinstance(value, int)
        and value >= minimum
    )


def _canonical_lexical_absolute(value: object) -> bool:
    return (
        isinstance(value, str)
        and Path(value).is_absolute()
        and os.path.normpath(value) == value
    )


def _validate_normalized_root(root: Path) -> None:
    published_build._validate_root_and_parent_chain(root)
    metadata = root.lstat()
    if (
        stat.S_IMODE(metadata.st_mode) != NORMALIZED_DIRECTORY_MODE
        or metadata.st_mode & 0o7000 != 0
    ):
        raise PublishedGeneratedCandidateError(
            "published generated-candidate root mode is not normalized"
        )


def _direct_inventory(path: Path) -> dict[bytes, os.DirEntry[bytes]]:
    try:
        entries = list(os.scandir(os.fsencode(path)))
    except OSError as error:
        raise PublishedGeneratedCandidateError(
            "published generated-candidate inventory is unavailable"
        ) from error
    if len(entries) > MAX_TREE_ENTRIES:
        raise PublishedGeneratedCandidateError(
            "published generated-candidate inventory is oversized"
        )
    return {entry.name: entry for entry in entries}


def _tree_sha256(
    root: Path,
    *,
    domain: bytes,
    excluded: Path | None = None,
) -> str:
    """Hash one normalized tree using sorted relative D/F frames."""

    records: list[tuple[bytes, bytes, Path, os.stat_result]] = []
    stack: list[tuple[Path, bytes]] = [(root, b"")]
    while stack:
        directory, relative_directory = stack.pop()
        try:
            entries = sorted(
                os.scandir(os.fsencode(directory)),
                key=lambda entry: entry.name,
                reverse=True,
            )
        except OSError as error:
            raise PublishedGeneratedCandidateError(
                "published generated-candidate tree cannot be enumerated"
            ) from error
        for entry in entries:
            if len(records) >= MAX_TREE_ENTRIES:
                raise PublishedGeneratedCandidateError(
                    "published generated-candidate tree is oversized"
                )
            name = entry.name
            assert isinstance(name, bytes)
            relative = (
                name
                if not relative_directory
                else relative_directory + b"/" + name
            )
            path = Path(os.fsdecode(entry.path))
            metadata = entry.stat(follow_symlinks=False)
            if (
                metadata.st_uid != published_build.TRUSTED_OWNER_UID
                or metadata.st_mode & 0o7000 != 0
            ):
                raise PublishedGeneratedCandidateError(
                    "published generated-candidate entry has unsafe ownership"
                )
            if stat.S_ISDIR(metadata.st_mode):
                if stat.S_IMODE(metadata.st_mode) != NORMALIZED_DIRECTORY_MODE:
                    raise PublishedGeneratedCandidateError(
                        "published generated-candidate directory mode is not normalized"
                    )
                records.append((relative, b"D", path, metadata))
                stack.append((path, relative))
            elif stat.S_ISREG(metadata.st_mode):
                if (
                    metadata.st_nlink != 1
                    or stat.S_IMODE(metadata.st_mode) not in NORMALIZED_FILE_MODES
                ):
                    raise PublishedGeneratedCandidateError(
                        "published generated-candidate file metadata is unsafe"
                    )
                records.append((relative, b"F", path, metadata))
            else:
                raise PublishedGeneratedCandidateError(
                    "published generated-candidate tree has a special entry"
                )

    digest = hashlib.sha256()
    digest.update(domain)

    def frame(payload: bytes) -> None:
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)

    excluded_seen = excluded is None
    for relative, kind, path, metadata in sorted(
        records,
        key=lambda record: record[0],
    ):
        if excluded is not None and path == excluded:
            excluded_seen = True
            continue
        digest.update(kind)
        frame(relative)
        digest.update(stat.S_IMODE(metadata.st_mode).to_bytes(4, "big"))
        if kind == b"F":
            observed_sha256, size, _ = published_build._stable_regular_sha256(
                path
            )
            digest.update(size.to_bytes(8, "big"))
            digest.update(bytes.fromhex(observed_sha256))
    if not excluded_seen:
        raise PublishedGeneratedCandidateError(
            "generated-candidate receipt was not excluded from its tree digest"
        )
    return digest.hexdigest()


def _require_flat_inventory(path: Path, names: frozenset[str]) -> None:
    entries = _direct_inventory(path)
    if set(entries) != {os.fsencode(name) for name in names}:
        raise PublishedGeneratedCandidateError(
            "published generated-candidate subtree inventory is not exact"
        )
    if any(not entry.is_file(follow_symlinks=False) for entry in entries.values()):
        raise PublishedGeneratedCandidateError(
            "published generated-candidate subtree contains a non-file"
        )


def _validate_worker_launch_receipt(
    document: Mapping[str, Any],
    *,
    admitted_build: published_build.AdmittedPublishedBuild,
    output_parent: Mapping[str, Any],
) -> None:
    if (
        set(document) != set(WORKER_LAUNCH_RECEIPT_KEYS)
        or document.get("schema") != WORKER_LAUNCH_RECEIPT_SCHEMA
        or document.get("build_user_name") != published_build.BUILD_USER_NAME
        or not _exact_int(document.get("build_uid"), minimum=1)
        or document.get("build_uid") != admitted_build.build_uid
        or document.get("candidate_build_receipt_path")
        != str(admitted_build.receipt)
        or document.get("candidate_build_receipt_sha256")
        != admitted_build.receipt_sha256
        or document.get("candidate_output_leaf") != CANDIDATE_DIR_NAME
        or document.get("resource_report_leaf")
        != RESOURCE_REPORT_DIR_NAME
        or not _nonzero_lower_hex(
            document.get("generation_command_sha256"),
            64,
        )
    ):
        raise PublishedGeneratedCandidateError(
            "generation-worker launch receipt contract is not exact"
        )
    worker_root_text = document.get("worker_root")
    if not _canonical_lexical_absolute(worker_root_text):
        raise PublishedGeneratedCandidateError(
            "generation-worker root is not canonical"
        )
    worker_root = Path(worker_root_text)
    if (
        worker_root.name != "generation-output"
        or worker_root.parent.name != "worker"
        or re.fullmatch(r"run-[0-9a-f]{32}", worker_root.parent.parent.name)
        is None
        or worker_root_text != output_parent.get("canonical_path")
    ):
        raise PublishedGeneratedCandidateError(
            "generation-worker root does not match the guarded output root"
        )

    numeric_fields = (
        "storage_available_bytes_after_generation",
        "storage_available_bytes_at_admission",
        "storage_device",
        "storage_minimum_available_bytes",
        "storage_post_build_reserve_bytes",
        "worker_root_device",
        "worker_root_inode",
    )
    if any(
        not _exact_int(document.get(key), minimum=1)
        for key in numeric_fields
    ):
        raise PublishedGeneratedCandidateError(
            "generation-worker launch resource identity is malformed"
        )
    admission = document["storage_available_bytes_at_admission"]
    after_generation = document["storage_available_bytes_after_generation"]
    minimum = document["storage_minimum_available_bytes"]
    reserve = document["storage_post_build_reserve_bytes"]
    if (
        document["worker_root_device"] != output_parent.get("device")
        or document["worker_root_inode"] != output_parent.get("inode")
        or document["storage_device"] != document["worker_root_device"]
        or admission > MAX_RECORDED_STORAGE_BYTES
        or after_generation > MAX_RECORDED_STORAGE_BYTES
        or minimum > MAX_RECORDED_STORAGE_BYTES
        or reserve > MAX_RECORDED_STORAGE_BYTES
        or admission < minimum
        or after_generation < reserve
        or minimum < MINIMUM_STORAGE_AT_ADMISSION_BYTES
        or reserve < MINIMUM_POST_BUILD_STORAGE_RESERVE_BYTES
        or minimum <= reserve
    ):
        raise PublishedGeneratedCandidateError(
            "generation-worker launch storage bounds are not satisfied"
        )


def _validate_execution_context(
    execution: object,
    *,
    admitted_build: published_build.AdmittedPublishedBuild,
    output_parent: Mapping[str, Any],
    staging_id: str,
) -> None:
    if not isinstance(execution, dict):
        raise PublishedGeneratedCandidateError(
            "generation executable method is malformed"
        )
    method = execution.get("method")
    if method == "pinned_fd":
        if (
            set(execution) != set(PINNED_EXECUTION_CONTEXT_KEYS)
            or execution.get("descriptor_path")
            != str(admitted_build.executable)
        ):
            raise PublishedGeneratedCandidateError(
                "generation pinned executable method is not exact"
            )
        return
    if method != "darwin_private_fd_copy":
        raise PublishedGeneratedCandidateError(
            "generation executable method is not admitted"
        )

    expected_directory_name = (
        f"{GENERATION_STAGING_PREFIX}{staging_id}-exec"
    )
    expected_path = (
        Path(output_parent["canonical_path"])
        / expected_directory_name
        / BUNDLE_EXECUTABLE_FILE_NAME
    )
    mode = execution.get("mode")
    directory_device = execution.get("directory_device")
    directory_inode = execution.get("directory_inode")
    file_device = execution.get("file_device")
    file_inode = execution.get("file_inode")
    if (
        set(execution) != set(DARWIN_EXECUTION_CONTEXT_KEYS)
        or execution.get("canonical_path") != str(expected_path)
        or execution.get("directory_name") != expected_directory_name
        or execution.get("file_name") != BUNDLE_EXECUTABLE_FILE_NAME
        or directory_device != output_parent.get("device")
        or file_device != directory_device
        or not _exact_int(directory_inode, minimum=1)
        or not _exact_int(file_inode, minimum=1)
        or file_inode == directory_inode
        or isinstance(mode, bool)
        or not isinstance(mode, int)
        or not stat.S_ISREG(mode)
        or stat.S_IMODE(mode) != 0o500
        or execution.get("sha256") != admitted_build.executable_sha256
        or execution.get("size_bytes")
        != admitted_build.executable_size_bytes
    ):
        raise PublishedGeneratedCandidateError(
            "generation Darwin execution-copy closure is not exact"
        )


def _validate_summary(
    document: Mapping[str, Any],
    admitted_build: published_build.AdmittedPublishedBuild,
) -> None:
    if set(document) != set(GENERATION_SUMMARY_KEYS):
        raise PublishedGeneratedCandidateError(
            "generation summary field closure is not exact"
        )
    for key, expected in (
        ("child_exit_code", 0),
        ("exit_status", 0),
        ("post_success_finalize_result", 1),
        ("schema_version", 1),
    ):
        if (
            not _exact_int(document.get(key))
            or document.get(key) != expected
        ):
            raise PublishedGeneratedCandidateError(
                "generation summary status integers are not exact"
            )
    if (
        document.get("event") != "summary"
        or document.get("exit_reason") != "completed"
        or document.get("post_run_cleanup") != "completed"
        or document.get("post_run_validation") != "completed"
        or document.get("post_success_finalize") != "completed"
        or document.get("kernel_peak_rss_scope") != "direct_guarded_body"
    ):
        raise PublishedGeneratedCandidateError(
            "generation summary does not record a successful guarded publication"
        )
    for key in (
        "evidence_peak_rss_bytes",
        "kernel_peak_rss_bytes",
        "peak_physical_footprint_bytes",
    ):
        if not _exact_int(document.get(key)):
            raise PublishedGeneratedCandidateError(
                f"generation summary {key} is malformed"
            )
    if (
        not _exact_int(document.get("supervisor_pid"), minimum=1)
        or not _exact_int(document.get("memory_limit_bytes"), minimum=1)
        or not _exact_int(document.get("peak_memory_bytes"), minimum=1)
        or not _exact_int(document.get("peak_rss_bytes"), minimum=1)
        or not _exact_int(
            document.get("post_run_cleanup_removed"),
            minimum=1,
        )
        or not _exact_int(document.get("sample_count"), minimum=1)
        or not 0 < document["memory_limit_bytes"] <= MAX_GENERATION_MEMORY_BYTES
        or any(
            document[key] > document["memory_limit_bytes"]
            for key in (
                "evidence_peak_rss_bytes",
                "kernel_peak_rss_bytes",
                "peak_memory_bytes",
                "peak_physical_footprint_bytes",
                "peak_rss_bytes",
            )
        )
        or (
            document["kernel_peak_rss_bytes"] == 0
            and document.get("kernel_peak_rss_method") != "unavailable"
        )
        or (
            document["kernel_peak_rss_bytes"] > 0
            and document.get("kernel_peak_rss_method")
            != "wait4_ru_maxrss"
        )
    ):
        raise PublishedGeneratedCandidateError(
            "generation summary resource bounds are malformed"
        )
    sample_interval = document.get("sample_interval_seconds")
    if (
        isinstance(sample_interval, bool)
        or not isinstance(sample_interval, (int, float))
        or not math.isfinite(sample_interval)
        or sample_interval != GENERATION_SAMPLE_INTERVAL_SECONDS
    ):
        raise PublishedGeneratedCandidateError(
            "generation summary sample interval is malformed"
        )
    if not all(
        isinstance(document.get(key), str) and document.get(key)
        for key in (
            "ended_utc",
            "kernel_peak_rss_method",
            "started_utc",
        )
    ):
        raise PublishedGeneratedCandidateError(
            "generation summary text fields are malformed"
        )

    context = document.get("report_context")
    if not isinstance(context, dict) or set(context) != set(REPORT_CONTEXT_KEYS):
        raise PublishedGeneratedCandidateError(
            "generation report context field closure is not exact"
        )
    if (
        context.get("publication_status") != PROVISIONAL_PUBLICATION_STATUS
        or context.get("cross_stage_status") != PROVISIONAL_CROSS_STAGE_STATUS
        or not _exact_int(
            context.get("same_parent_recovered_staging_directories")
        )
        or context.get("same_parent_recovered_staging_directories") != 0
        or not _lower_hex(context.get("staging_id"), 32)
    ):
        raise PublishedGeneratedCandidateError(
            "generation report context does not identify provisional worker output"
        )

    build_context = context.get("root_published_build")
    expected_build_context = {
        "artifact_root": str(admitted_build.artifact_root),
        "artifact_tree_sha256": admitted_build.artifact_tree_sha256,
        "build_uid": admitted_build.build_uid,
        "build_user_name": admitted_build.build_user_name,
        "receipt": str(admitted_build.receipt),
        "receipt_sha256": admitted_build.receipt_sha256,
    }
    if (
        not isinstance(build_context, dict)
        or set(build_context) != set(ROOT_PUBLISHED_BUILD_CONTEXT_KEYS)
        or build_context != expected_build_context
    ):
        raise PublishedGeneratedCandidateError(
            "generation summary does not bind the admitted candidate build"
        )

    executable = context.get("executable_identity")
    executable_metadata = admitted_build.executable.lstat()
    if (
        not isinstance(executable, dict)
        or set(executable) != set(EXECUTABLE_IDENTITY_KEYS)
        or executable.get("canonical_path") != str(admitted_build.executable)
        or executable.get("sha256") != admitted_build.executable_sha256
        or executable.get("size_bytes") != admitted_build.executable_size_bytes
        or executable.get("build_profile") != admitted_build.executable.parent.name
    ):
        raise PublishedGeneratedCandidateError(
            "generation executable identity differs from the admitted build"
        )
    stat_identity = executable.get("stat_identity")
    expected_stat_identity = {
        "changed_ns": executable_metadata.st_ctime_ns,
        "device": executable_metadata.st_dev,
        "inode": executable_metadata.st_ino,
        "link_count": executable_metadata.st_nlink,
        "mode": executable_metadata.st_mode,
        "modified_ns": executable_metadata.st_mtime_ns,
        "owner_uid": executable_metadata.st_uid,
    }
    if (
        not isinstance(stat_identity, dict)
        or set(stat_identity) != set(EXECUTABLE_STAT_IDENTITY_KEYS)
        or stat_identity != expected_stat_identity
        or not _exact_int(stat_identity.get("device"), minimum=1)
        or not _exact_int(stat_identity.get("inode"), minimum=1)
        or not _exact_int(stat_identity.get("link_count"), minimum=1)
        or not _exact_int(stat_identity.get("mode"), minimum=1)
        or not _exact_int(stat_identity.get("modified_ns"), minimum=1)
        or not _exact_int(stat_identity.get("changed_ns"), minimum=1)
        or not _exact_int(stat_identity.get("owner_uid"))
    ):
        raise PublishedGeneratedCandidateError(
            "generation executable stat identity is not exact"
        )
    output_parent = context.get("output_parent")
    if (
        not isinstance(output_parent, dict)
        or set(output_parent) != set(OUTPUT_PARENT_CONTEXT_KEYS)
        or output_parent.get("admission")
        != "fresh_single_use_generation_worker_output_parent"
        or output_parent.get("output_name") != CANDIDATE_DIR_NAME
        or not _canonical_lexical_absolute(output_parent.get("canonical_path"))
    ):
        raise PublishedGeneratedCandidateError(
            "generation output-parent evidence is not exact"
        )
    for key in (
        "device",
        "free_bytes_at_admission",
        "inode",
        "minimum_free_bytes",
    ):
        if not _exact_int(output_parent.get(key), minimum=1):
            raise PublishedGeneratedCandidateError(
                "generation output-parent numeric evidence is malformed"
            )
    if (
        output_parent["free_bytes_at_admission"]
        < output_parent["minimum_free_bytes"]
        or not isinstance(output_parent.get("filesystem_type"), str)
        or not output_parent.get("filesystem_type")
        or (
            output_parent.get("device"),
            output_parent.get("inode"),
        )
        == (
            executable_metadata.st_dev,
            executable_metadata.st_ino,
        )
    ):
        raise PublishedGeneratedCandidateError(
            "generation output filesystem evidence is malformed"
        )
    _validate_execution_context(
        executable.get("execution"),
        admitted_build=admitted_build,
        output_parent=output_parent,
        staging_id=context["staging_id"],
    )


def _validate_jsonl(payload: bytes, summary: Mapping[str, Any]) -> None:
    if not payload.endswith(b"\n"):
        raise PublishedGeneratedCandidateError(
            "generation JSONL is not newline-terminated"
        )
    lines = payload.splitlines(keepends=True)
    if len(lines) < 3 or len(lines) > 1_000_000:
        raise PublishedGeneratedCandidateError(
            "generation JSONL event inventory is malformed"
        )
    events: list[Mapping[str, Any]] = []
    for line in lines:
        if not line.endswith(b"\n") or line in {b"\n", b"\r\n"}:
            raise PublishedGeneratedCandidateError(
                "generation JSONL contains an empty or partial record"
            )
        events.append(
            published_build._strict_canonical_json(
                line,
                "generation JSONL record",
            )
        )
    start = events[0]
    spawn = events[1]
    samples = events[2:-1]
    if (
        events[-1] != summary
        or set(start) != set(GENERATION_START_KEYS)
        or start.get("event") != "start"
        or not _exact_int(start.get("schema_version"))
        or start.get("schema_version") != 1
        or start.get("memory_limit_bytes") != summary.get("memory_limit_bytes")
        or start.get("sample_interval_seconds")
        != summary.get("sample_interval_seconds")
        or start.get("started_utc") != summary.get("started_utc")
        or start.get("supervisor_pid") != summary.get("supervisor_pid")
        or start.get("report_context") != summary.get("report_context")
        or set(spawn) != set(GENERATION_SPAWN_KEYS)
        or spawn.get("event") != "spawn"
        or not _exact_int(spawn.get("schema_version"))
        or spawn.get("schema_version") != 1
        or not _exact_int(spawn.get("process_group_id"), minimum=1)
        or not _exact_int(spawn.get("wrapper_pid"), minimum=1)
        or spawn.get("process_group_id") == spawn.get("wrapper_pid")
        or not isinstance(spawn.get("timestamp_utc"), str)
        or not spawn.get("timestamp_utc")
        or len(samples) != summary.get("sample_count")
    ):
        raise PublishedGeneratedCandidateError(
            "generation JSONL event closure is not exact"
        )
    memory_samples: list[int] = []
    footprint_samples: list[int] = []
    rss_samples: list[int] = []
    for sample in samples:
        elapsed = sample.get("elapsed_seconds")
        if (
            set(sample) != set(GENERATION_SAMPLE_KEYS)
            or sample.get("event") != "sample"
            or not _exact_int(sample.get("schema_version"))
            or sample.get("schema_version") != 1
            or sample.get("memory_limit_bytes")
            != summary.get("memory_limit_bytes")
            or sample.get("process_group_id") != spawn.get("process_group_id")
            or not isinstance(sample.get("accounting_method"), str)
            or not sample.get("accounting_method")
            or not isinstance(sample.get("timestamp_utc"), str)
            or not sample.get("timestamp_utc")
            or isinstance(elapsed, bool)
            or not isinstance(elapsed, (int, float))
            or not math.isfinite(elapsed)
            or elapsed < 0
        ):
            raise PublishedGeneratedCandidateError(
                "generation JSONL sample closure is not exact"
            )
        for key in (
            "memory_bytes",
            "physical_footprint_bytes",
            "rss_bytes",
        ):
            if not _exact_int(sample.get(key)):
                raise PublishedGeneratedCandidateError(
                    "generation JSONL sample resources are malformed"
                )
        if (
            not _exact_int(sample.get("process_count"), minimum=1)
            or sample["memory_bytes"] > summary["memory_limit_bytes"]
            or sample["physical_footprint_bytes"]
            > summary["memory_limit_bytes"]
            or sample["rss_bytes"] > summary["memory_limit_bytes"]
        ):
            raise PublishedGeneratedCandidateError(
                "generation JSONL sample resource bounds are malformed"
            )
        memory_samples.append(sample["memory_bytes"])
        footprint_samples.append(sample["physical_footprint_bytes"])
        rss_samples.append(sample["rss_bytes"])
    if (
        max(memory_samples, default=0) != summary.get("peak_memory_bytes")
        or max(footprint_samples, default=0)
        != summary.get("peak_physical_footprint_bytes")
        or max(rss_samples, default=0) != summary.get("peak_rss_bytes")
        or max(
            max(rss_samples, default=0),
            summary.get("kernel_peak_rss_bytes", 0),
        )
        != summary.get("evidence_peak_rss_bytes")
    ):
        raise PublishedGeneratedCandidateError(
            "generation JSONL samples do not bind the summary peaks"
        )


def admit_generated_candidate(
    receipt_path: Path,
    receipt_sha256: str,
) -> AdmittedPublishedGeneratedCandidate:
    """Admit one independently pinned root-published generated candidate."""

    if (
        not receipt_path.is_absolute()
        or os.path.normpath(os.fspath(receipt_path)) != os.fspath(receipt_path)
        or receipt_path.name != RECEIPT_FILE_NAME
        or not _nonzero_lower_hex(receipt_sha256, 64)
    ):
        raise PublishedGeneratedCandidateError(
            "generated-candidate receipt path or independent pin is malformed"
        )
    try:
        receipt = published_build._canonical_absolute_path(
            os.fspath(receipt_path)
        )
        receipt_payload, observed_receipt_sha256, receipt_metadata = (
            published_build._stable_regular_bytes(
                receipt,
                maximum_bytes=MAX_RECEIPT_BYTES,
            )
        )
    except published_build.PublishedBuildError as error:
        raise PublishedGeneratedCandidateError(str(error)) from error
    if (
        observed_receipt_sha256 != receipt_sha256
        or stat.S_IMODE(receipt_metadata.st_mode) not in NORMALIZED_FILE_MODES
    ):
        raise PublishedGeneratedCandidateError(
            "generated-candidate receipt differs from its independent pin"
        )
    try:
        document = published_build._strict_canonical_json(
            receipt_payload,
            "root-published generated-candidate receipt",
        )
    except published_build.PublishedBuildError as error:
        raise PublishedGeneratedCandidateError(str(error)) from error
    if (
        set(document) != set(RECEIPT_KEYS)
        or document.get("schema") != RECEIPT_SCHEMA
        or document.get("publication_protocol") != PUBLICATION_PROTOCOL
        or document.get("publication_status") != PUBLICATION_STATUS
        or document.get("provisional_generation_publication_status")
        != PROVISIONAL_PUBLICATION_STATUS
        or document.get("provisional_cross_stage_status")
        != PROVISIONAL_CROSS_STAGE_STATUS
        or document.get("build_user_name") != published_build.BUILD_USER_NAME
    ):
        raise PublishedGeneratedCandidateError(
            "generated-candidate receipt contract is not exact"
        )
    build_uid = document.get("build_uid")
    if not _exact_int(build_uid, minimum=1):
        raise PublishedGeneratedCandidateError(
            "generated-candidate build UID is malformed"
        )
    for key, length in (
        ("artifact_tree_sha256", 64),
        ("candidate_build_artifact_tree_sha256", 64),
        ("candidate_build_receipt_sha256", 64),
        ("candidate_tree_sha256", 64),
        ("generation_resource_report_tree_sha256", 64),
        ("generation_summary_sha256", 64),
        ("production_closure_tree_sha256", 64),
        ("reviewed_source_closure_descriptor_sha256", 64),
        ("source_commit", 40),
        ("source_tree_sha256", 64),
        ("toolchain_provenance_sha256", 64),
        ("worker_launch_receipt_sha256", 64),
    ):
        if not _nonzero_lower_hex(document.get(key), length):
            raise PublishedGeneratedCandidateError(
                f"generated-candidate {key} is malformed"
            )

    try:
        root = published_build._canonical_absolute_path(
            document.get("artifact_root")
        )
        candidate_dir = published_build._canonical_absolute_path(
            document.get("candidate_dir_path")
        )
        resource_report = published_build._canonical_absolute_path(
            document.get("generation_resource_report_path")
        )
        summary_path = published_build._canonical_absolute_path(
            document.get("generation_summary_path")
        )
        candidate_build_receipt = published_build._canonical_absolute_path(
            document.get("candidate_build_receipt_path")
        )
    except published_build.PublishedBuildError as error:
        raise PublishedGeneratedCandidateError(str(error)) from error

    expected_tree_sha256 = document["artifact_tree_sha256"]
    assert isinstance(expected_tree_sha256, str)
    if (
        root.name != expected_tree_sha256
        or receipt != root / RECEIPT_FILE_NAME
        or candidate_dir != root / CANDIDATE_DIR_NAME
        or resource_report != root / RESOURCE_REPORT_DIR_NAME
        or summary_path != resource_report / GENERATION_SUMMARY_FILE_NAME
    ):
        raise PublishedGeneratedCandidateError(
            "generated-candidate paths are not the exact content-addressed inventory"
        )
    _validate_normalized_root(root)
    direct = _direct_inventory(root)
    if set(direct) != {
        os.fsencode(RECEIPT_FILE_NAME),
        os.fsencode(CANDIDATE_DIR_NAME),
        os.fsencode(RESOURCE_REPORT_DIR_NAME),
        os.fsencode(WORKER_LAUNCH_RECEIPT_FILE_NAME),
    }:
        raise PublishedGeneratedCandidateError(
            "generated-candidate root inventory is not exact"
        )
    _require_flat_inventory(candidate_dir, CANDIDATE_FILE_NAMES)
    _require_flat_inventory(resource_report, RESOURCE_REPORT_FILE_NAMES)

    observed_tree_sha256 = _tree_sha256(
        root,
        domain=ARTIFACT_TREE_DOMAIN,
        excluded=receipt,
    )
    candidate_tree_sha256 = _tree_sha256(
        candidate_dir,
        domain=SUBTREE_DOMAIN,
    )
    resource_report_tree_sha256 = _tree_sha256(
        resource_report,
        domain=SUBTREE_DOMAIN,
    )
    if (
        observed_tree_sha256 != expected_tree_sha256
        or candidate_tree_sha256 != document["candidate_tree_sha256"]
        or resource_report_tree_sha256
        != document["generation_resource_report_tree_sha256"]
    ):
        raise PublishedGeneratedCandidateError(
            "generated-candidate tree differs from its content address"
        )

    try:
        admitted_build = published_build.admit_candidate(
            candidate_build_receipt,
            document["candidate_build_receipt_sha256"],
        )
    except published_build.PublishedBuildError as error:
        raise PublishedGeneratedCandidateError(
            "generated candidate does not bind an admitted candidate build"
        ) from error
    if (
        admitted_build.artifact_tree_sha256
        != document["candidate_build_artifact_tree_sha256"]
        or admitted_build.build_uid != build_uid
        or admitted_build.build_user_name != document["build_user_name"]
        or admitted_build.production_closure_tree_sha256
        != document["production_closure_tree_sha256"]
        or admitted_build.toolchain_provenance_sha256
        != document["toolchain_provenance_sha256"]
        or admitted_build.reviewed_source_closure_descriptor_sha256
        != document["reviewed_source_closure_descriptor_sha256"]
        or admitted_build.source_commit != document["source_commit"]
        or admitted_build.source_tree_sha256 != document["source_tree_sha256"]
    ):
        raise PublishedGeneratedCandidateError(
            "generated-candidate receipt and candidate-build receipt do not agree"
        )

    try:
        summary_payload, summary_sha256, summary_metadata = (
            published_build._stable_regular_bytes(
                summary_path,
                maximum_bytes=MAX_GENERATION_SUMMARY_BYTES,
            )
        )
        jsonl_path = resource_report / GENERATION_JSONL_FILE_NAME
        jsonl_payload, jsonl_sha256, jsonl_metadata = (
            published_build._stable_regular_bytes(
                jsonl_path,
                maximum_bytes=MAX_GENERATION_JSONL_BYTES,
            )
        )
        worker_launch_receipt = root / WORKER_LAUNCH_RECEIPT_FILE_NAME
        (
            worker_launch_payload,
            worker_launch_sha256,
            worker_launch_metadata,
        ) = published_build._stable_regular_bytes(
            worker_launch_receipt,
            maximum_bytes=MAX_WORKER_LAUNCH_RECEIPT_BYTES,
        )
    except published_build.PublishedBuildError as error:
        raise PublishedGeneratedCandidateError(str(error)) from error
    if (
        summary_sha256 != document["generation_summary_sha256"]
        or summary_metadata.st_mode & 0o111 != 0
        or jsonl_metadata.st_mode & 0o111 != 0
        or worker_launch_sha256
        != document["worker_launch_receipt_sha256"]
        or worker_launch_metadata.st_gid != TRUSTED_LAUNCH_OWNER_GID
        or stat.S_IMODE(worker_launch_metadata.st_mode) != 0o444
    ):
        raise PublishedGeneratedCandidateError(
            "generation report differs from its receipt"
        )
    try:
        summary_document = published_build._strict_canonical_json(
            summary_payload,
            "root-published generation summary",
        )
        worker_launch_document = published_build._strict_canonical_json(
            worker_launch_payload,
            "root-published generation-worker launch receipt",
        )
        _validate_summary(summary_document, admitted_build)
        _validate_jsonl(jsonl_payload, summary_document)
        _validate_worker_launch_receipt(
            worker_launch_document,
            admitted_build=admitted_build,
            output_parent=summary_document["report_context"][
                "output_parent"
            ],
        )
    except published_build.PublishedBuildError as error:
        raise PublishedGeneratedCandidateError(str(error)) from error

    return AdmittedPublishedGeneratedCandidate(
        receipt=receipt,
        receipt_sha256=receipt_sha256,
        artifact_root=root,
        artifact_tree_sha256=observed_tree_sha256,
        candidate_dir=candidate_dir,
        candidate_tree_sha256=candidate_tree_sha256,
        generation_resource_report=resource_report,
        generation_resource_report_tree_sha256=resource_report_tree_sha256,
        generation_summary=summary_path,
        generation_summary_sha256=summary_sha256,
        generation_jsonl=jsonl_path,
        generation_jsonl_sha256=jsonl_sha256,
        worker_launch_receipt=worker_launch_receipt,
        candidate_build=admitted_build,
        worker_launch_receipt_sha256=worker_launch_sha256,
        generation_command_sha256=worker_launch_document[
            "generation_command_sha256"
        ],
        worker_root=Path(worker_launch_document["worker_root"]),
        worker_root_device=worker_launch_document["worker_root_device"],
        worker_root_inode=worker_launch_document["worker_root_inode"],
        build_user_name=admitted_build.build_user_name,
        build_uid=admitted_build.build_uid,
        production_closure_tree_sha256=(
            admitted_build.production_closure_tree_sha256
        ),
        toolchain_provenance_sha256=(
            admitted_build.toolchain_provenance_sha256
        ),
        reviewed_source_closure_descriptor_sha256=(
            admitted_build.reviewed_source_closure_descriptor_sha256
        ),
        source_commit=admitted_build.source_commit,
        source_tree_sha256=admitted_build.source_tree_sha256,
    )
