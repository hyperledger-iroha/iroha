from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock

from scripts import kagemusha_root_published_build as published_build
from scripts import kagemusha_source_tree_seal as source_seal
from scripts import (
    kagemusha_root_published_generated_candidate as generated,
)


def _canonical_json(document: object) -> bytes:
    return (
        json.dumps(
            document,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        )
        + "\n"
    ).encode("ascii")


def _reviewed_source_closure() -> tuple[dict[str, object], str]:
    manifest: list[object] = []
    manifest_sha256 = hashlib.sha256(
        source_seal._untracked_manifest_bytes(manifest)
    ).hexdigest()
    combined = hashlib.sha256()
    combined.update(source_seal.SOURCE_DIFF_DOMAIN)
    combined.update(source_seal.TRACKED_DIFF_DOMAIN)
    combined.update(bytes.fromhex(source_seal.EMPTY_SHA256))
    combined.update(source_seal.UNTRACKED_MANIFEST_DOMAIN)
    combined.update(bytes.fromhex(manifest_sha256))
    descriptor: dict[str, object] = {
        "base_commit": "3" * 40,
        "combined_source_fingerprint_sha256": combined.hexdigest(),
        "ignored_cargo_lock_sha256": "e" * 64,
        "ignored_cargo_lock_size_bytes": 1,
        "schema": source_seal.REVIEWED_SOURCE_CLOSURE_SCHEMA,
        "source_commit": "3" * 40,
        "source_repo_dirty": False,
        "source_tree_sha256": "4" * 64,
        "tracked_binary_diff_sha256": source_seal.EMPTY_SHA256,
        "untracked_file_count": 0,
        "untracked_path_mode_blob_oid_manifest": manifest,
        "untracked_path_mode_blob_oid_manifest_sha256": manifest_sha256,
    }
    digest = hashlib.sha256(
        source_seal._canonical_json_bytes(descriptor)
    ).hexdigest()
    return descriptor, digest


class RootPublishedGeneratedCandidateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.parent = Path(self.temporary.name).resolve()
        self.fixture_counter = 0
        self.build_receipt, self.build_receipt_sha256, self.build_document = (
            self._candidate_build_fixture()
        )

    def tearDown(self) -> None:
        for path in sorted(self.parent.rglob("*"), reverse=True):
            try:
                path.chmod(0o700 if path.is_dir() else 0o600)
            except OSError:
                pass
        self.temporary.cleanup()

    def _owner_patches(
        self,
    ) -> tuple[mock._patch, mock._patch, mock._patch]:
        return (
            mock.patch.object(
                published_build,
                "TRUSTED_OWNER_UID",
                os.geteuid(),
            ),
            mock.patch.object(
                published_build,
                "_validate_root_and_parent_chain",
                lambda _root: None,
            ),
            mock.patch.object(
                generated,
                "TRUSTED_LAUNCH_OWNER_GID",
                os.getegid(),
            ),
        )

    def _candidate_build_fixture(
        self,
    ) -> tuple[Path, str, dict[str, object]]:
        contract = published_build._CANDIDATE_CONTRACT
        root = self.parent / "candidate-build-provisional"
        root.mkdir()
        executable = root / contract.executable_file_name
        executable.write_bytes(b"root-published candidate executable\n")
        executable.chmod(0o500)
        executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest()
        report = root / contract.report_file_name
        reviewed_source_closure, reviewed_source_closure_sha256 = (
            _reviewed_source_closure()
        )
        report_document: dict[str, object] = {
            "apple_developer_dir_path": "/root/closure/apple",
            "apple_sdk_path": "/root/closure/apple/sdk",
            "binary_path": "/worker/target/release/kagemusha_recursive_spend_v4_bundle",
            "binary_sha256": executable_sha256,
            "binary_size_bytes": executable.stat().st_size,
            "build_profile": "release",
            "build_uid": 777,
            "build_user_name": published_build.BUILD_USER_NAME,
            "cargo_home_path": "/root/closure/cargo-home",
            "cargo_path": "/root/closure/bin/cargo",
            "cargo_sha256": "6" * 64,
            "cargo_vendor_path": "/root/closure/cargo-vendor",
            "clang_resource_dir_path": "/root/closure/apple/clang-resource",
            "git_exec_path": "/root/closure/git-core",
            "git_path": "/root/closure/bin/git",
            "git_sha256": "7" * 64,
            "gpg_path": "/root/closure/bin/gpg",
            "gpg_sha256": "8" * 64,
            "linker_path": "/root/closure/apple/bin/clang",
            "linker_sha256": "9" * 64,
            "minimum_build_physical_memory_bytes": 24 * 1024**3,
            "physical_memory_bytes_at_admission": 32 * 1024**3,
            "production_closure_root": "/root/closure",
            "production_closure_tree_sha256": "1" * 64,
            "publication_status": "provisional_boi_build_worker_output",
            "python_path": "/root/closure/bin/python3",
            "python_sha256": "a" * 64,
            "reviewed_source_closure": reviewed_source_closure,
            "reviewed_source_closure_descriptor_sha256": (
                reviewed_source_closure_sha256
            ),
            "rustc_path": "/root/closure/bin/rustc",
            "rustc_sha256": "b" * 64,
            "rustc_sysroot_path": "/root/closure/rustc-sysroot",
            "schema": published_build.CANDIDATE_REPORT_SCHEMA,
            "source_commit": "3" * 40,
            "source_repo_dirty": False,
            "source_signing_key_fingerprint": "C" * 40,
            "source_tree_sha256": "4" * 64,
            "target_dir": "/worker/target",
            "toolchain_provenance_sha256": "5" * 64,
        }
        self.assertEqual(
            set(report_document),
            set(published_build.CANDIDATE_REPORT_KEYS),
        )
        report.write_bytes(_canonical_json(report_document))
        report.chmod(0o400)
        report_sha256 = hashlib.sha256(report.read_bytes()).hexdigest()
        receipt = root / contract.receipt_file_name
        receipt.write_bytes(b"placeholder\n")
        receipt.chmod(0o400)
        first_patch, second_patch, third_patch = self._owner_patches()
        with first_patch, second_patch, third_patch:
            tree_sha256 = published_build._artifact_tree_sha256(
                root,
                receipt,
                executable,
                report,
            )
        final_root = root.with_name(tree_sha256)
        root.rename(final_root)
        executable = final_root / executable.name
        report = final_root / report.name
        receipt = final_root / receipt.name
        receipt_document: dict[str, object] = {
            "artifact_root": str(final_root),
            "artifact_tree_sha256": tree_sha256,
            "binary_path": str(executable),
            "binary_sha256": executable_sha256,
            "binary_size_bytes": executable.stat().st_size,
            "build_uid": 777,
            "build_user_name": published_build.BUILD_USER_NAME,
            "production_closure_tree_sha256": "1" * 64,
            "publication_protocol": published_build.PUBLICATION_PROTOCOL,
            "reviewed_source_closure_descriptor_sha256": (
                reviewed_source_closure_sha256
            ),
            "schema": published_build.CANDIDATE_RECEIPT_SCHEMA,
            "sealed_build_report_path": str(report),
            "sealed_build_report_sha256": report_sha256,
            "source_commit": "3" * 40,
            "source_tree_sha256": "4" * 64,
            "toolchain_provenance_sha256": "5" * 64,
        }
        receipt.chmod(0o600)
        receipt.write_bytes(_canonical_json(receipt_document))
        receipt.chmod(0o400)
        final_root.chmod(0o500)
        return (
            receipt,
            hashlib.sha256(receipt.read_bytes()).hexdigest(),
            receipt_document,
        )

    def _summary(
        self,
        staging_id: str,
        *,
        darwin_execution: bool = False,
    ) -> dict[str, object]:
        executable = Path(self.build_document["binary_path"])
        metadata = executable.lstat()
        output_parent_path = (
            self.parent
            / f"run-{staging_id}"
            / "worker"
            / "generation-output"
        )
        if darwin_execution:
            directory_name = (
                f"{generated.GENERATION_STAGING_PREFIX}{staging_id}-exec"
            )
            execution = {
                "canonical_path": str(
                    output_parent_path
                    / directory_name
                    / generated.BUNDLE_EXECUTABLE_FILE_NAME
                ),
                "directory_device": 11,
                "directory_inode": 13,
                "directory_name": directory_name,
                "file_device": 11,
                "file_inode": 14,
                "file_name": generated.BUNDLE_EXECUTABLE_FILE_NAME,
                "method": "darwin_private_fd_copy",
                "mode": stat.S_IFREG | 0o500,
                "sha256": self.build_document["binary_sha256"],
                "size_bytes": self.build_document["binary_size_bytes"],
            }
        else:
            execution = {
                "descriptor_path": str(executable),
                "method": "pinned_fd",
            }
        context = {
            "cross_stage_status": generated.PROVISIONAL_CROSS_STAGE_STATUS,
            "executable_identity": {
                "build_profile": executable.parent.name,
                "canonical_path": str(executable),
                "execution": execution,
                "sha256": self.build_document["binary_sha256"],
                "size_bytes": self.build_document["binary_size_bytes"],
                "stat_identity": {
                    "changed_ns": metadata.st_ctime_ns,
                    "device": metadata.st_dev,
                    "inode": metadata.st_ino,
                    "link_count": metadata.st_nlink,
                    "mode": metadata.st_mode,
                    "modified_ns": metadata.st_mtime_ns,
                    "owner_uid": os.geteuid(),
                },
            },
            "output_parent": {
                "admission": "fresh_single_use_generation_worker_output_parent",
                "canonical_path": str(output_parent_path),
                "device": 11,
                "filesystem_type": "apfs",
                "free_bytes_at_admission": 80 * 1024**3,
                "inode": 12,
                "minimum_free_bytes": 1024**3,
                "output_name": generated.CANDIDATE_DIR_NAME,
            },
            "publication_status": generated.PROVISIONAL_PUBLICATION_STATUS,
            "root_published_build": {
                "artifact_root": self.build_document["artifact_root"],
                "artifact_tree_sha256": self.build_document[
                    "artifact_tree_sha256"
                ],
                "build_uid": 777,
                "build_user_name": published_build.BUILD_USER_NAME,
                "receipt": str(self.build_receipt),
                "receipt_sha256": self.build_receipt_sha256,
            },
            "same_parent_recovered_staging_directories": 0,
            "staging_id": staging_id,
        }
        return {
            "child_exit_code": 0,
            "ended_utc": "2026-07-30T10:00:01.000Z",
            "event": "summary",
            "exit_reason": "completed",
            "exit_status": 0,
            "evidence_peak_rss_bytes": 1024,
            "kernel_peak_rss_bytes": 1024,
            "kernel_peak_rss_method": "wait4_ru_maxrss",
            "kernel_peak_rss_scope": "direct_guarded_body",
            "memory_limit_bytes": 256 * 1024**2,
            "peak_memory_bytes": 1024,
            "peak_physical_footprint_bytes": 1024,
            "peak_rss_bytes": 1024,
            "post_run_cleanup": "completed",
            "post_run_cleanup_removed": 1,
            "post_run_validation": "completed",
            "post_success_finalize": "completed",
            "post_success_finalize_result": 1,
            "report_context": context,
            "sample_count": 1,
            "sample_interval_seconds": 0.05,
            "schema_version": 1,
            "started_utc": "2026-07-30T10:00:00.000Z",
            "supervisor_pid": 1234,
        }

    def _fixture(
        self,
        *,
        receipt_mutator=None,
        summary_mutator=None,
        jsonl_mutator=None,
        launch_mutator=None,
        darwin_execution: bool = False,
    ) -> tuple[Path, str, dict[str, object]]:
        self.fixture_counter += 1
        staging_id = f"{self.fixture_counter:032x}"
        root = self.parent / f"generated-provisional-{self.fixture_counter}"
        candidate = root / generated.CANDIDATE_DIR_NAME
        report = root / generated.RESOURCE_REPORT_DIR_NAME
        candidate.mkdir(parents=True)
        report.mkdir()
        for index, name in enumerate(sorted(generated.CANDIDATE_FILE_NAMES)):
            path = candidate / name
            path.write_bytes(f"candidate artifact {index}: {name}\n".encode())
            path.chmod(0o444)
        summary = self._summary(
            staging_id,
            darwin_execution=darwin_execution,
        )
        if summary_mutator is not None:
            summary_mutator(summary)
        summary_path = report / generated.GENERATION_SUMMARY_FILE_NAME
        summary_path.write_bytes(_canonical_json(summary))
        summary_path.chmod(0o444)
        jsonl_path = report / generated.GENERATION_JSONL_FILE_NAME
        events = [
            {
                "event": "start",
                "memory_limit_bytes": summary["memory_limit_bytes"],
                "report_context": summary.get("report_context"),
                "sample_interval_seconds": summary[
                    "sample_interval_seconds"
                ],
                "schema_version": 1,
                "started_utc": summary["started_utc"],
                "supervisor_pid": summary["supervisor_pid"],
            },
            {
                "event": "spawn",
                "process_group_id": 1235,
                "schema_version": 1,
                "timestamp_utc": "2026-07-30T10:00:00.001Z",
                "wrapper_pid": 1236,
            },
            {
                "accounting_method": "procfs",
                "elapsed_seconds": 0.01,
                "event": "sample",
                "memory_bytes": 1024,
                "memory_limit_bytes": summary["memory_limit_bytes"],
                "physical_footprint_bytes": 1024,
                "process_count": 1,
                "process_group_id": 1235,
                "rss_bytes": 1024,
                "schema_version": 1,
                "timestamp_utc": "2026-07-30T10:00:00.010Z",
            },
            summary,
        ]
        if jsonl_mutator is not None:
            jsonl_mutator(events)
        jsonl_path.write_bytes(b"".join(_canonical_json(event) for event in events))
        jsonl_path.chmod(0o444)
        output_parent = summary["report_context"]["output_parent"]
        launch_document: dict[str, object] = {
            "build_uid": 777,
            "build_user_name": published_build.BUILD_USER_NAME,
            "candidate_build_receipt_path": str(self.build_receipt),
            "candidate_build_receipt_sha256": self.build_receipt_sha256,
            "candidate_output_leaf": generated.CANDIDATE_DIR_NAME,
            "generation_command_sha256": "c" * 64,
            "resource_report_leaf": generated.RESOURCE_REPORT_DIR_NAME,
            "schema": generated.WORKER_LAUNCH_RECEIPT_SCHEMA,
            "storage_available_bytes_after_generation": 20 * 1024**3,
            "storage_available_bytes_at_admission": 80 * 1024**3,
            "storage_device": output_parent["device"],
            "storage_minimum_available_bytes": 64 * 1024**3,
            "storage_post_build_reserve_bytes": 16 * 1024**3,
            "worker_root": output_parent["canonical_path"],
            "worker_root_device": output_parent["device"],
            "worker_root_inode": output_parent["inode"],
        }
        if launch_mutator is not None:
            launch_mutator(launch_document)
        launch_path = root / generated.WORKER_LAUNCH_RECEIPT_FILE_NAME
        launch_path.write_bytes(_canonical_json(launch_document))
        launch_path.chmod(0o444)
        launch_sha256 = hashlib.sha256(launch_path.read_bytes()).hexdigest()
        candidate.chmod(0o555)
        report.chmod(0o555)
        receipt = root / generated.RECEIPT_FILE_NAME
        receipt.write_bytes(b"placeholder\n")
        receipt.chmod(0o444)
        first_patch, second_patch, third_patch = self._owner_patches()
        with first_patch, second_patch, third_patch:
            candidate_tree_sha256 = generated._tree_sha256(
                candidate,
                domain=generated.SUBTREE_DOMAIN,
            )
            report_tree_sha256 = generated._tree_sha256(
                report,
                domain=generated.SUBTREE_DOMAIN,
            )
            root.chmod(0o555)
            tree_sha256 = generated._tree_sha256(
                root,
                domain=generated.ARTIFACT_TREE_DOMAIN,
                excluded=receipt,
            )
        final_root = root.with_name(tree_sha256)
        root.rename(final_root)
        candidate = final_root / candidate.name
        report = final_root / report.name
        summary_path = report / summary_path.name
        receipt = final_root / receipt.name
        document: dict[str, object] = {
            "artifact_root": str(final_root),
            "artifact_tree_sha256": tree_sha256,
            "build_uid": 777,
            "build_user_name": published_build.BUILD_USER_NAME,
            "candidate_build_artifact_tree_sha256": self.build_document[
                "artifact_tree_sha256"
            ],
            "candidate_build_receipt_path": str(self.build_receipt),
            "candidate_build_receipt_sha256": self.build_receipt_sha256,
            "candidate_dir_path": str(candidate),
            "candidate_tree_sha256": candidate_tree_sha256,
            "generation_resource_report_path": str(report),
            "generation_resource_report_tree_sha256": report_tree_sha256,
            "generation_summary_path": str(summary_path),
            "generation_summary_sha256": hashlib.sha256(
                summary_path.read_bytes()
            ).hexdigest(),
            "production_closure_tree_sha256": "1" * 64,
            "provisional_cross_stage_status": (
                generated.PROVISIONAL_CROSS_STAGE_STATUS
            ),
            "provisional_generation_publication_status": (
                generated.PROVISIONAL_PUBLICATION_STATUS
            ),
            "publication_protocol": generated.PUBLICATION_PROTOCOL,
            "publication_status": generated.PUBLICATION_STATUS,
            "reviewed_source_closure_descriptor_sha256": self.build_document[
                "reviewed_source_closure_descriptor_sha256"
            ],
            "schema": generated.RECEIPT_SCHEMA,
            "source_commit": "3" * 40,
            "source_tree_sha256": "4" * 64,
            "toolchain_provenance_sha256": "5" * 64,
            "worker_launch_receipt_sha256": launch_sha256,
        }
        if receipt_mutator is not None:
            receipt_mutator(document)
        final_root.chmod(0o755)
        receipt.chmod(0o644)
        receipt.write_bytes(_canonical_json(document))
        receipt.chmod(0o444)
        final_root.chmod(0o555)
        return receipt, hashlib.sha256(receipt.read_bytes()).hexdigest(), document

    def _admit(
        self,
        receipt: Path,
        receipt_sha256: str,
    ) -> generated.AdmittedPublishedGeneratedCandidate:
        first_patch, second_patch, third_patch = self._owner_patches()
        with first_patch, second_patch, third_patch:
            return generated.admit_generated_candidate(
                receipt,
                receipt_sha256,
            )

    def test_exact_root_published_generated_candidate_is_admitted(self) -> None:
        receipt, receipt_sha256, document = self._fixture()

        admitted = self._admit(receipt, receipt_sha256)

        self.assertEqual(admitted.candidate_dir, Path(document["candidate_dir_path"]))
        self.assertEqual(
            admitted.candidate_build.receipt,
            self.build_receipt,
        )
        self.assertEqual(
            admitted.worker_launch_receipt_sha256,
            document["worker_launch_receipt_sha256"],
        )
        self.assertEqual(admitted.generation_command_sha256, "c" * 64)
        self.assertEqual(admitted.build_uid, 777)

    def test_exact_darwin_private_execution_copy_is_admitted(self) -> None:
        receipt, receipt_sha256, _ = self._fixture(darwin_execution=True)

        admitted = self._admit(receipt, receipt_sha256)

        self.assertEqual(admitted.candidate_build.receipt, self.build_receipt)

    def test_darwin_execution_copy_rejects_wrong_missing_and_extra_keys(
        self,
    ) -> None:
        mutations = (
            lambda execution: execution.__setitem__("file_name", "wrong"),
            lambda execution: execution.pop("file_inode"),
            lambda execution: execution.__setitem__("unexpected", "field"),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    darwin_execution=True,
                    summary_mutator=lambda summary, mutation=mutation: mutation(
                        summary["report_context"]["executable_identity"][
                            "execution"
                        ]
                    ),
                )
                with self.assertRaisesRegex(
                    generated.PublishedGeneratedCandidateError,
                    "Darwin execution-copy closure is not exact",
                ):
                    self._admit(receipt, receipt_sha256)

    def test_launch_receipt_rejects_wrong_missing_and_extra_keys(self) -> None:
        mutations = (
            lambda launch: launch.__setitem__("schema", "wrong"),
            lambda launch: launch.pop("generation_command_sha256"),
            lambda launch: launch.__setitem__("unexpected", "field"),
            lambda launch: launch.__setitem__(
                "generation_command_sha256",
                "C" * 64,
            ),
            lambda launch: launch.__setitem__(
                "generation_command_sha256",
                False,
            ),
            lambda launch: launch.__setitem__(
                "generation_command_sha256",
                "0" * 64,
            ),
            lambda launch: launch.__setitem__(
                "candidate_build_receipt_sha256",
                "d" * 64,
            ),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    launch_mutator=mutation
                )
                with self.assertRaisesRegex(
                    generated.PublishedGeneratedCandidateError,
                    "launch receipt contract is not exact",
                ):
                    self._admit(receipt, receipt_sha256)

    def test_launch_receipt_rejects_worker_and_storage_substitutions(
        self,
    ) -> None:
        mutations = (
            lambda launch: launch.__setitem__("build_uid", True),
            lambda launch: launch.__setitem__(
                "worker_root",
                "/worker/provisional/generation-output",
            ),
            lambda launch: launch.__setitem__("worker_root_inode", 0),
            lambda launch: launch.__setitem__(
                "storage_available_bytes_at_admission",
                63 * 1024**3,
            ),
            lambda launch: launch.__setitem__(
                "storage_available_bytes_after_generation",
                15 * 1024**3,
            ),
            lambda launch: launch.__setitem__(
                "storage_post_build_reserve_bytes",
                True,
            ),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    launch_mutator=mutation
                )
                with self.assertRaises(
                    generated.PublishedGeneratedCandidateError
                ):
                    self._admit(receipt, receipt_sha256)

    def test_receipt_rejects_wrong_missing_and_extra_keys(self) -> None:
        mutations = (
            lambda document: document.__setitem__("schema", "wrong"),
            lambda document: document.pop("worker_launch_receipt_sha256"),
            lambda document: document.__setitem__("unexpected", "field"),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    receipt_mutator=mutation
                )
                with self.assertRaisesRegex(
                    generated.PublishedGeneratedCandidateError,
                    "contract is not exact",
                ):
                    self._admit(receipt, receipt_sha256)

    def test_receipt_rejects_a_substituted_launch_receipt_pin(self) -> None:
        receipt, receipt_sha256, _ = self._fixture(
            receipt_mutator=lambda document: document.__setitem__(
                "worker_launch_receipt_sha256",
                "d" * 64,
            )
        )
        with self.assertRaisesRegex(
            generated.PublishedGeneratedCandidateError,
            "generation report differs from its receipt",
        ):
            self._admit(receipt, receipt_sha256)

    def test_summary_rejects_wrong_missing_and_extra_keys(self) -> None:
        mutations = (
            lambda summary: summary.__setitem__("event", "provisional"),
            lambda summary: summary.pop("post_run_validation"),
            lambda summary: summary.__setitem__("unexpected", "field"),
        )
        patterns = (
            "successful guarded publication",
            "field closure is not exact",
            "field closure is not exact",
        )
        for mutation, pattern in zip(mutations, patterns):
            with self.subTest(pattern=pattern):
                receipt, receipt_sha256, _ = self._fixture(
                    summary_mutator=mutation
                )
                with self.assertRaisesRegex(
                    generated.PublishedGeneratedCandidateError,
                    pattern,
                ):
                    self._admit(receipt, receipt_sha256)

    def test_duplicate_and_nonfinite_json_are_rejected(self) -> None:
        receipt, _receipt_sha256, document = self._fixture()
        receipt.parent.chmod(0o755)
        receipt.chmod(0o644)
        payload = _canonical_json(document)
        duplicate = payload[:-2] + b',"schema":"duplicate"}\n'
        receipt.write_bytes(duplicate)
        receipt.chmod(0o444)
        receipt.parent.chmod(0o555)
        with self.assertRaisesRegex(
            generated.PublishedGeneratedCandidateError,
            "strict JSON",
        ):
            self._admit(
                receipt,
                hashlib.sha256(receipt.read_bytes()).hexdigest(),
            )

        receipt, _receipt_sha256, document = self._fixture()
        receipt.parent.chmod(0o755)
        receipt.chmod(0o644)
        document["build_uid"] = float("nan")
        receipt.write_bytes(_canonical_json(document))
        receipt.chmod(0o444)
        receipt.parent.chmod(0o555)
        with self.assertRaisesRegex(
            generated.PublishedGeneratedCandidateError,
            "strict JSON",
        ):
            self._admit(
                receipt,
                hashlib.sha256(receipt.read_bytes()).hexdigest(),
            )

    def test_summary_rejects_noninteger_and_unbounded_resource_evidence(
        self,
    ) -> None:
        executable_metadata = Path(
            self.build_document["binary_path"]
        ).lstat()
        mutations = (
            lambda summary: summary.__setitem__("child_exit_code", False),
            lambda summary: summary.__setitem__("sample_count", True),
            lambda summary: summary.__setitem__(
                "peak_memory_bytes",
                summary["memory_limit_bytes"] + 1,
            ),
            lambda summary: summary["report_context"].__setitem__(
                "same_parent_recovered_staging_directories",
                False,
            ),
            lambda summary: summary["report_context"]["output_parent"].__setitem__(
                "device",
                0,
            ),
            lambda summary: summary["report_context"]["output_parent"].update(
                {
                    "device": executable_metadata.st_dev,
                    "inode": executable_metadata.st_ino,
                }
            ),
            lambda summary: summary["report_context"]["output_parent"].__setitem__(
                "filesystem_type",
                "",
            ),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    summary_mutator=mutation
                )
                with self.assertRaises(
                    generated.PublishedGeneratedCandidateError
                ):
                    self._admit(receipt, receipt_sha256)

    def test_nonfinite_generation_summary_is_rejected(self) -> None:
        receipt, receipt_sha256, _ = self._fixture(
            summary_mutator=lambda summary: summary.__setitem__(
                "sample_interval_seconds",
                float("nan"),
            )
        )
        with self.assertRaisesRegex(
            generated.PublishedGeneratedCandidateError,
            "strict JSON",
        ):
            self._admit(receipt, receipt_sha256)

    def test_jsonl_rejects_wrong_missing_and_extra_event_keys(self) -> None:
        mutations = (
            lambda event: event.__setitem__("event", "provisional"),
            lambda event: event.pop("wrapper_pid"),
            lambda event: event.__setitem__("unexpected", "field"),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    jsonl_mutator=lambda events, mutation=mutation: mutation(
                        events[1]
                    )
                )
                with self.assertRaisesRegex(
                    generated.PublishedGeneratedCandidateError,
                    "event closure is not exact",
                ):
                    self._admit(receipt, receipt_sha256)

    def test_jsonl_rejects_pid_and_sample_type_substitutions(self) -> None:
        mutations = (
            lambda events: events[1].__setitem__("wrapper_pid", 1235),
            lambda events: events[0].__setitem__("schema_version", True),
            lambda events: events[1].__setitem__("schema_version", True),
            lambda events: events[2].__setitem__("schema_version", True),
            lambda events: events[2].__setitem__("process_count", False),
            lambda events: events[2].__setitem__(
                "memory_bytes",
                257 * 1024**2,
            ),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    jsonl_mutator=mutation
                )
                with self.assertRaises(
                    generated.PublishedGeneratedCandidateError
                ):
                    self._admit(receipt, receipt_sha256)

    def test_same_uid_candidate_mutation_breaks_content_address(self) -> None:
        receipt, receipt_sha256, document = self._fixture()
        candidate = Path(document["candidate_dir_path"])
        target = candidate / "candidate-manifest.norito"
        candidate.parent.chmod(0o755)
        candidate.chmod(0o755)
        target.chmod(0o644)
        target.write_bytes(b"same-UID poisoned candidate\n")
        target.chmod(0o444)
        candidate.chmod(0o555)
        candidate.parent.chmod(0o555)

        with self.assertRaisesRegex(
            generated.PublishedGeneratedCandidateError,
            "content address",
        ):
            self._admit(receipt, receipt_sha256)

    def test_candidate_and_report_inventories_are_exact(self) -> None:
        receipt, receipt_sha256, document = self._fixture()
        report = Path(document["generation_resource_report_path"])
        report.parent.chmod(0o755)
        report.chmod(0o755)
        extra = report / "same-uid-extra"
        extra.write_bytes(b"extra\n")
        extra.chmod(0o444)
        report.chmod(0o555)
        report.parent.chmod(0o555)
        with self.assertRaisesRegex(
            generated.PublishedGeneratedCandidateError,
            "inventory is not exact",
        ):
            self._admit(receipt, receipt_sha256)


if __name__ == "__main__":
    unittest.main()
