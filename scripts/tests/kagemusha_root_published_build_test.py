from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import kagemusha_root_published_build as published
from scripts import kagemusha_source_tree_seal as source_seal


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


class RootPublishedBuildTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.parent = Path(self.temporary.name).resolve()
        self.fixture_counter = 0

    def tearDown(self) -> None:
        for path in sorted(self.parent.rglob("*"), reverse=True):
            try:
                path.chmod(0o700 if path.is_dir() else 0o600)
            except OSError:
                pass
        self.temporary.cleanup()

    def _fixture(
        self,
        *,
        promotion_verifier: bool = False,
        report_mutator=None,
        report_encoder=None,
    ) -> tuple[Path, str, dict[str, object]]:
        self.fixture_counter += 1
        contract = (
            published._PROMOTION_VERIFIER_CONTRACT
            if promotion_verifier
            else published._CANDIDATE_CONTRACT
        )
        provisional_root = self.parent / f"{self.fixture_counter:064x}"
        provisional_root.mkdir(mode=0o700)
        executable = provisional_root / contract.executable_file_name
        executable.write_bytes(b"root-published executable fixture\n")
        executable.chmod(0o500)
        executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest()
        report = provisional_root / contract.report_file_name
        reviewed_source_closure, reviewed_source_closure_sha256 = (
            _reviewed_source_closure()
        )
        report_document = {
            contract.executable_path_key: (
                f"/worker/target/release/{contract.executable_file_name}"
            ),
            contract.executable_sha256_key: executable_sha256,
            contract.executable_size_key: executable.stat().st_size,
            "build_uid": 777,
            "build_user_name": published.BUILD_USER_NAME,
            "production_closure_tree_sha256": "1" * 64,
            "publication_status": "provisional_boi_build_worker_output",
            "reviewed_source_closure_descriptor_sha256": (
                reviewed_source_closure_sha256
            ),
            "schema": contract.report_schema,
            "source_commit": "3" * 40,
            "source_tree_sha256": "4" * 64,
            "toolchain_provenance_sha256": "5" * 64,
        }
        if not promotion_verifier:
            report_document.update(
                {
                    "apple_developer_dir_path": "/root/closure/apple",
                    "apple_sdk_path": "/root/closure/apple/sdk",
                    "build_profile": "release",
                    "cargo_home_path": "/root/closure/cargo-home",
                    "cargo_path": "/root/closure/bin/cargo",
                    "cargo_sha256": "6" * 64,
                    "cargo_vendor_path": "/root/closure/cargo-vendor",
                    "clang_resource_dir_path": (
                        "/root/closure/apple/clang-resource"
                    ),
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
                    "python_path": "/root/closure/bin/python3",
                    "python_sha256": "a" * 64,
                    "reviewed_source_closure": reviewed_source_closure,
                    "rustc_path": "/root/closure/bin/rustc",
                    "rustc_sha256": "b" * 64,
                    "rustc_sysroot_path": "/root/closure/rustc-sysroot",
                    "source_repo_dirty": False,
                    "source_signing_key_fingerprint": "C" * 40,
                    "target_dir": "/worker/target",
                }
            )
        if report_mutator is not None:
            report_mutator(report_document)
        elif report_encoder is None:
            self.assertEqual(set(report_document), set(contract.report_keys))
        report_payload = (
            report_encoder(report_document)
            if report_encoder is not None
            else (
                json.dumps(
                    report_document,
                    ensure_ascii=True,
                    separators=(",", ":"),
                    sort_keys=True,
                )
                + "\n"
            ).encode("ascii")
        )
        report.write_bytes(report_payload)
        report.chmod(0o400)
        report_sha256 = hashlib.sha256(report.read_bytes()).hexdigest()
        receipt = provisional_root / contract.receipt_file_name
        receipt.write_bytes(b"placeholder\n")
        receipt.chmod(0o400)
        with (
            mock.patch.object(
                published,
                "TRUSTED_OWNER_UID",
                os.geteuid(),
            ),
            mock.patch.object(
                published,
                "_validate_root_and_parent_chain",
                lambda _root: None,
            ),
        ):
            tree_sha256 = published._artifact_tree_sha256(
                provisional_root,
                receipt,
                executable,
                report,
            )
        final_root = provisional_root.with_name(tree_sha256)
        provisional_root.rename(final_root)
        executable = final_root / contract.executable_file_name
        report = final_root / contract.report_file_name
        receipt = final_root / contract.receipt_file_name
        receipt_document: dict[str, object] = {
            "artifact_root": str(final_root),
            "artifact_tree_sha256": tree_sha256,
            contract.executable_path_key: str(executable),
            contract.executable_sha256_key: executable_sha256,
            contract.executable_size_key: executable.stat().st_size,
            "build_uid": 777,
            "build_user_name": published.BUILD_USER_NAME,
            "production_closure_tree_sha256": "1" * 64,
            "publication_protocol": published.PUBLICATION_PROTOCOL,
            "reviewed_source_closure_descriptor_sha256": (
                reviewed_source_closure_sha256
            ),
            "schema": contract.receipt_schema,
            "sealed_build_report_path": str(report),
            "sealed_build_report_sha256": report_sha256,
            "source_commit": "3" * 40,
            "source_tree_sha256": "4" * 64,
            "toolchain_provenance_sha256": "5" * 64,
        }
        receipt.chmod(0o600)
        receipt.write_text(
            json.dumps(
                receipt_document,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n",
            encoding="ascii",
        )
        receipt.chmod(0o400)
        final_root.chmod(0o500)
        receipt_sha256 = hashlib.sha256(receipt.read_bytes()).hexdigest()
        return receipt, receipt_sha256, receipt_document

    def _admit_candidate(
        self,
        receipt: Path,
        receipt_sha256: str,
    ) -> published.AdmittedPublishedBuild:
        with (
            mock.patch.object(
                published,
                "TRUSTED_OWNER_UID",
                os.geteuid(),
            ),
            mock.patch.object(
                published,
                "_validate_root_and_parent_chain",
                lambda _root: None,
            ),
        ):
            return published.admit_candidate(receipt, receipt_sha256)

    def test_candidate_receipt_admits_exact_root_published_inventory(self) -> None:
        receipt, receipt_sha256, document = self._fixture()

        admitted = self._admit_candidate(receipt, receipt_sha256)

        self.assertEqual(admitted.executable, Path(document["binary_path"]))
        self.assertEqual(admitted.executable_sha256, document["binary_sha256"])
        self.assertEqual(admitted.build_user_name, published.BUILD_USER_NAME)
        self.assertEqual(admitted.build_uid, 777)

    def test_receipt_and_report_must_agree(self) -> None:
        receipt, _receipt_sha256, document = self._fixture()
        receipt.chmod(0o600)
        document["build_uid"] = 778
        receipt.write_text(
            json.dumps(
                document,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n",
            encoding="ascii",
        )
        receipt.chmod(0o400)
        receipt_sha256 = hashlib.sha256(receipt.read_bytes()).hexdigest()

        with self.assertRaisesRegex(
            published.PublishedBuildError,
            "do not agree",
        ):
            self._admit_candidate(receipt, receipt_sha256)

    def test_binary_mutation_breaks_content_address(self) -> None:
        receipt, receipt_sha256, document = self._fixture()
        executable = Path(document["binary_path"])
        executable.chmod(0o700)
        executable.write_bytes(b"same-UID poisoned output\n")
        executable.chmod(0o500)

        with self.assertRaisesRegex(
            published.PublishedBuildError,
            "content address",
        ):
            self._admit_candidate(receipt, receipt_sha256)

    def test_receipt_requires_independent_digest(self) -> None:
        receipt, _receipt_sha256, _document = self._fixture()

        with self.assertRaisesRegex(
            published.PublishedBuildError,
            "independent pin",
        ):
            self._admit_candidate(receipt, "f" * 64)

    def test_parallel_promotion_verifier_contract(self) -> None:
        receipt, receipt_sha256, document = self._fixture(
            promotion_verifier=True,
        )
        with (
            mock.patch.object(
                published,
                "TRUSTED_OWNER_UID",
                os.geteuid(),
            ),
            mock.patch.object(
                published,
                "_validate_root_and_parent_chain",
                lambda _root: None,
            ),
        ):
            admitted = published.admit_promotion_verifier(
                receipt,
                receipt_sha256,
            )

        self.assertEqual(
            admitted.executable,
            Path(document["promotion_verifier_path"]),
        )
        self.assertEqual(
            admitted.executable_sha256,
            document["promotion_verifier_sha256"],
        )

    def test_sealed_report_rejects_wrong_missing_and_extra_keys(self) -> None:
        mutations = (
            lambda report: report.__setitem__("schema", "wrong"),
            lambda report: report.pop("source_commit"),
            lambda report: report.__setitem__("unexpected", "field"),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    report_mutator=mutation
                )
                with self.assertRaisesRegex(
                    published.PublishedBuildError,
                    "contract is not exact",
                ):
                    self._admit_candidate(receipt, receipt_sha256)

    def test_sealed_report_rejects_duplicate_and_nonfinite_json(self) -> None:
        def duplicate(document: dict[str, object]) -> bytes:
            payload = (
                json.dumps(
                    document,
                    ensure_ascii=True,
                    separators=(",", ":"),
                    sort_keys=True,
                )
                + "\n"
            ).encode("ascii")
            return payload[:-2] + b',"schema":"duplicate"}\n'

        receipt, receipt_sha256, _ = self._fixture(report_encoder=duplicate)
        with self.assertRaisesRegex(
            published.PublishedBuildError,
            "strict JSON",
        ):
            self._admit_candidate(receipt, receipt_sha256)

        receipt, receipt_sha256, _ = self._fixture(
            report_mutator=lambda report: report.__setitem__(
                "physical_memory_bytes_at_admission",
                float("nan"),
            )
        )
        with self.assertRaisesRegex(
            published.PublishedBuildError,
            "strict JSON",
        ):
            self._admit_candidate(receipt, receipt_sha256)

    def test_candidate_report_rejects_semantic_substitutions(self) -> None:
        mutations = (
            lambda report: report.__setitem__(
                "physical_memory_bytes_at_admission",
                True,
            ),
            lambda report: report.__setitem__(
                "physical_memory_bytes_at_admission",
                23 * 1024**3,
            ),
            lambda report: report.__setitem__("source_repo_dirty", 0),
            lambda report: report.__setitem__("binary_path", "relative/binary"),
            lambda report: report.__setitem__("cargo_sha256", "A" * 64),
            lambda report: report.__setitem__("cargo_sha256", "0" * 64),
            lambda report: report.__setitem__(
                "source_signing_key_fingerprint",
                "0" * 40,
            ),
            lambda report: report.update(
                {
                    "binary_path": (
                        "/root/closure/target/release/"
                        "kagemusha_recursive_spend_v4_bundle"
                    ),
                    "target_dir": "/root/closure/target",
                }
            ),
            lambda report: report["reviewed_source_closure"].__setitem__(
                "source_tree_sha256",
                "f" * 64,
            ),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                receipt, receipt_sha256, _ = self._fixture(
                    report_mutator=mutation
                )
                with self.assertRaises(published.PublishedBuildError):
                    self._admit_candidate(receipt, receipt_sha256)


if __name__ == "__main__":
    unittest.main()
