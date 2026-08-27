"""Tests for platform-scoped Kagemusha mobile production authorizations."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile


SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "kagemusha_mobile_production_authorization.py"
)
SPEC = importlib.util.spec_from_file_location("kagemusha_mobile_authorization", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
authorization = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(authorization)


class KagemushaMobileProductionAuthorizationTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.source_sha = "2" * 40
        self.reviewed_source = "3" * 64
        self.sealed_report = "4" * 64
        self.source_tree = "5" * 64
        self.candidate_sha = "9" * 64
        self.candidate_manifest = "a" * 64
        self.candidate_stage_manifest = "b" * 64
        self.candidate_inventory = "c" * 64
        self.release_generation = "generation-4"
        self.evidence = self.root / "platform-evidence.json"
        self.evidence.write_bytes(b'{"status":"verified"}\n')
        self.release_policy = self.root / "release-policy.toml"
        self.release_policy.write_bytes(b"generation = 4\n")
        self.artifact_root = self.root / "catalog"
        self.artifact_root.mkdir()
        metadata_payloads = {
            name: f"authenticated-{index}-{name}\n".encode("ascii")
            for index, name in enumerate(authorization.CATALOG_METADATA, start=1)
        }
        metadata_payloads["manifest.json"] = authorization._canonical_json(
            {
                "bridge_abi_version": 23,
                "generation": self.release_generation,
                "reviewed_source_closure_descriptor_sha256": self.reviewed_source,
                "sealed_candidate_build_report_sha256": self.sealed_report,
                "source_commit": self.source_sha,
                "source_repo_dirty": False,
                "source_tree_sha256": self.source_tree,
            }
        )
        self.artifact_manifest = hashlib.sha256(
            metadata_payloads["manifest.norito"]
        ).hexdigest()
        self.release = self.artifact_root / self.artifact_manifest
        self.release.mkdir()
        for name, payload in metadata_payloads.items():
            (self.release / name).write_bytes(payload)
        self.run_id = 12345
        self.run_attempt = 2
        self.promotion_id = authorization.promotion_id(
            authorization.EXPECTED_REPOSITORY,
            authorization.EXPECTED_WORKFLOW_REF,
            self.source_sha,
            self.run_id,
            self.run_attempt,
        )
        self.verification_report = self.root / "kagami-report.json"
        report = {field: "d" * 64 for field in authorization.KAGAMI_REPORT_FIELDS}
        report.update(
            {
                "artifacts": [],
                "asset_definition_id": "cash#sora",
                "asset_scale": 2,
                "bridge_abi_version": 23,
                "candidate_sha256": self.candidate_sha,
                "envelope_sha256": self.artifact_manifest,
                "generation": self.release_generation,
                "generation_memory_enforcement_profile": "bounded",
                "generation_memory_limit_bytes": 1024,
                "network_id": "iroha-test",
                "promotion_record_sha256": hashlib.sha256(
                    (self.release / "promotion-record-v4.norito").read_bytes()
                ).hexdigest(),
                "release_policy_sha256": hashlib.sha256(
                    self.release_policy.read_bytes()
                ).hexdigest(),
                "sealed_candidate_build_report_sha256": self.sealed_report,
                "status": "verified",
            }
        )
        self.verification_report.write_bytes(authorization._canonical_json(report))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def coordinates(self, platform: str) -> list[str]:
        return [
            "--platform",
            platform,
            "--repository",
            authorization.EXPECTED_REPOSITORY,
            "--workflow-ref",
            authorization.EXPECTED_WORKFLOW_REF,
            "--workflow-sha",
            self.source_sha,
            "--source-sha",
            self.source_sha,
            "--run-id",
            str(self.run_id),
            "--run-attempt",
            str(self.run_attempt),
            "--promotion-id",
            self.promotion_id,
            "--release-tag",
            "v1.2.3",
            "--reviewed-source-closure-sha256",
            self.reviewed_source,
            "--sealed-candidate-build-report-sha256",
            self.sealed_report,
            "--artifact-manifest-sha256",
            self.artifact_manifest,
        ]

    def issue(self, platform: str = "apple") -> Path:
        if platform == "apple":
            evidence = {
                "schema": (
                    "iroha.kagemusha.ios."
                    "app_attest_catalog_revalidation_receipt.v1"
                ),
                "version": 1,
                "promotion_id": self.promotion_id,
                "status": "catalog-revalidated-for-one-promotion",
                "release_statuses": [
                    {"release_manifest_sha256": self.artifact_manifest}
                ],
            }
        else:
            binding = {
                "required": True,
                "candidate_manifest_sha256": self.candidate_manifest,
                "candidate_stage_manifest_sha256": self.candidate_stage_manifest,
                "candidate_inventory_sha256": self.candidate_inventory,
                "candidate_record_sha256": self.candidate_sha,
                "candidate_source_commit": self.source_sha,
                "candidate_source_tree_sha256": self.source_tree,
                "candidate_generation": self.release_generation,
                "candidate_source_repo_dirty": False,
                "native_bridge_abi_version": 23,
                "production_capability_observed": False,
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "signed_evidence_artifact_sha256": "d" * 64,
                "signed_evidence_signer_public_key_sha256": "e" * 64,
            }
            evidence = {
                "schema_version": 1,
                "slots": [{"status": "ok", "kagemusha": binding}],
                "ok": 1,
                "failed": 0,
                "kagemusha": {
                    "production_evidence_required": True,
                    "standard_matrix_required": True,
                    "missing_device_families": [],
                    "missing_d2d_payment_transports": [],
                    "missing_d2d_payment_transport_pairs": [],
                    "duplicate_bindings": {},
                },
            }
        self.evidence.write_bytes(authorization._canonical_json(evidence))
        output = self.root / f"{platform}-authorization.json"
        status = authorization.main(
            [
                "issue",
                *self.coordinates(platform),
                "--evidence",
                str(self.evidence),
                "--release-policy",
                str(self.release_policy),
                "--artifact-root",
                str(self.artifact_root),
                "--release-verification-report",
                str(self.verification_report),
                "--output",
                str(output),
            ]
        )
        self.assertEqual(status, 0)
        return output

    def verify(self, path: Path, platform: str = "apple") -> int:
        return authorization.main(
            [
                "verify",
                *self.coordinates(platform),
                "--authorization",
                str(path),
            ]
        )

    @staticmethod
    def _artifact_record(kind: str, path: Path) -> dict[str, object]:
        payload = path.read_bytes()
        return {
            "kind": kind,
            "name": path.name,
            "path": path.name,
            "sha256": hashlib.sha256(payload).hexdigest(),
            "bytes": len(payload),
        }

    @staticmethod
    def _write_checksums(path: Path, artifacts: list[Path]) -> None:
        path.write_text(
            "".join(
                f"{hashlib.sha256(artifact.read_bytes()).hexdigest()}  "
                f"{artifact.name}\n"
                for artifact in artifacts
            ),
            encoding="ascii",
        )

    def _create_apple_package(
        self,
        root: Path,
        authorization_sha256: str,
        *,
        manifest_document: dict[str, object] | None = None,
    ) -> dict[str, Path]:
        root.mkdir(exist_ok=True)
        manifest = root / "NoritoBridge-v1.2.3.artifacts.json"
        document = manifest_document or {
            "version": "1.2.3",
            "native_bridge_abi_version": 23,
            "privacy_production_enabled": True,
            "cargo_features": ["privacy-production-enabled"],
            "source_commit": self.source_sha,
            "kagemusha_production_authorization_sha256": authorization_sha256,
        }
        manifest.write_bytes(authorization._canonical_json(document))
        archive = root / "NoritoBridge-v1.2.3.xcframework.zip"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr(
                "NoritoBridge.xcframework/NoritoBridge.artifacts.json",
                manifest.read_bytes(),
            )
            output.writestr(
                "NoritoBridge.xcframework/ios-arm64/NoritoBridge.framework/NoritoBridge",
                b"authenticated native bytes\n",
            )
        archive_sha256 = hashlib.sha256(archive.read_bytes()).hexdigest()
        podspec = root / "NoritoBridge-1.2.3.podspec"
        podspec.write_text(
            "s.version          = '1.2.3'\n"
            "s.source = { :http => "
            "'https://github.com/hyperledger-iroha/iroha/releases/download/"
            "v1.2.3/NoritoBridge-v1.2.3.xcframework.zip', "
            f":sha256 => '{archive_sha256}' }}\n",
            encoding="utf-8",
        )
        package_manifest = root / "mobile-sdk-apple-v1.2.3.artifacts.json"
        package_manifest.write_bytes(
            authorization._canonical_json(
                {
                    "version": "v1.2.3",
                    "apple_sdk_semver": "1.2.3",
                    "mode": "apple",
                    "artifacts": [
                        self._artifact_record("apple-xcframework", archive),
                        self._artifact_record("apple-manifest", manifest),
                        self._artifact_record("apple-cocoapods-podspec", podspec),
                    ],
                }
            )
        )
        checksums = root / "SHA256SUMS-apple-v1.2.3.txt"
        self._write_checksums(
            checksums, [archive, manifest, podspec, package_manifest]
        )
        return {
            "archive": archive,
            "manifest": manifest,
            "podspec": podspec,
            "checksums": checksums,
            "package_manifest": package_manifest,
        }

    def _create_android_package(
        self,
        root: Path,
        authorization_sha256: str,
        *,
        provenance_document: dict[str, object] | None = None,
    ) -> dict[str, Path]:
        root.mkdir(exist_ok=True)
        provenance = provenance_document or {
            "native_bridge_abi_version": 23,
            "privacy_production_enabled": True,
            "cargo_features": ["privacy-production-enabled"],
            "source_commit": self.source_sha,
            "kagemusha_production_authorization_sha256": authorization_sha256,
        }
        payloads = {
            "core-jvm/core-jvm-v1.2.3.jar": b"core jar\n",
            "client-android/client-android-release.aar": b"client aar\n",
            "native/arm64-v8a/libconnect_norito_bridge.so": b"arm64 native\n",
            "native/x86_64/libconnect_norito_bridge.so": b"x86 native\n",
            "native/native-build-provenance-v1.json": authorization._canonical_json(
                provenance
            ),
            "maven/org/hyperledger/iroha/core-jvm/1.2.3/core-jvm-1.2.3.pom": (
                b"<project/>\n"
            ),
        }
        internal_checksums = "".join(
            f"{hashlib.sha256(payload).hexdigest()}  {name}\n"
            for name, payload in payloads.items()
        ).encode("ascii")
        archive = root / "iroha-mobile-sdk-android-v1.2.3.zip"
        archive_root = "iroha-mobile-sdk-android-v1.2.3"
        with zipfile.ZipFile(archive, "w") as output:
            for name, payload in payloads.items():
                output.writestr(f"{archive_root}/{name}", payload)
            output.writestr(f"{archive_root}/SHA256SUMS.txt", internal_checksums)
        package_manifest = root / "mobile-sdk-android-v1.2.3.artifacts.json"
        package_manifest.write_bytes(
            authorization._canonical_json(
                {
                    "version": "v1.2.3",
                    "mode": "android",
                    "artifacts": [self._artifact_record("android-sdk", archive)],
                }
            )
        )
        checksums = root / "SHA256SUMS-android-v1.2.3.txt"
        self._write_checksums(checksums, [archive, package_manifest])
        return {
            "archive": archive,
            "checksums": checksums,
            "package_manifest": package_manifest,
        }

    @staticmethod
    def _artifact_arguments(paths: dict[str, Path]) -> list[str]:
        arguments: list[str] = []
        for name, path in paths.items():
            arguments.extend((f"--{name.replace('_', '-')}", str(path)))
        return arguments

    def test_issue_and_verify_exact_platform_authorizations(self) -> None:
        apple = self.issue("apple")
        android = self.issue("android")
        self.assertEqual(self.verify(apple, "apple"), 0)
        self.assertEqual(self.verify(android, "android"), 0)
        self.assertNotEqual(
            hashlib.sha256(apple.read_bytes()).digest(),
            hashlib.sha256(android.read_bytes()).digest(),
        )
        apple_document = json.loads(apple.read_bytes())
        self.assertEqual(apple_document["platform"], "apple")
        self.assertEqual(
            apple_document["target_triples"], authorization.PLATFORM_TARGETS["apple"]
        )
        self.assertEqual(
            apple_document["release_catalog_sha256"],
            authorization.release_catalog_sha256(self.artifact_root),
        )
        self.assertEqual(
            apple_document["artifact_manifest_sha256"], self.artifact_manifest
        )
        self.assertEqual(
            apple_document["promotion_record_sha256"],
            hashlib.sha256(
                (self.release / "promotion-record-v4.norito").read_bytes()
            ).hexdigest(),
        )

    def test_pair_requires_identical_release_bindings(self) -> None:
        apple = self.issue("apple")
        android = self.issue("android")
        pair_args = [
            value
            for index, value in enumerate(self.coordinates("apple"))
            if index not in (0, 1)
        ]
        self.assertEqual(
            authorization.main(
                [
                    "verify-pair",
                    *pair_args,
                    "--apple-authorization",
                    str(apple),
                    "--android-authorization",
                    str(android),
                    "--apple-release-verification-report",
                    str(self.verification_report),
                    "--android-release-verification-report",
                    str(self.verification_report),
                ]
            ),
            0,
        )
        document = json.loads(android.read_bytes())
        document["promotion_record_sha256"] = "7" * 64
        android.write_bytes(authorization._canonical_json(document))
        self.assertEqual(
            authorization.main(
                [
                    "verify-pair",
                    *pair_args,
                    "--apple-authorization",
                    str(apple),
                    "--android-authorization",
                    str(android),
                    "--apple-release-verification-report",
                    str(self.verification_report),
                    "--android-release-verification-report",
                    str(self.verification_report),
                ]
            ),
            1,
        )

    def test_cross_platform_substitution_is_rejected(self) -> None:
        apple = self.issue("apple")
        self.assertEqual(self.verify(apple, "android"), 1)

    def test_platform_evidence_must_bind_selected_release(self) -> None:
        self.issue("android")
        evidence = json.loads(self.evidence.read_bytes())
        evidence["slots"][0]["kagemusha"]["candidate_record_sha256"] = "f" * 64
        self.evidence.write_bytes(authorization._canonical_json(evidence))
        output = self.root / "android-mismatch.json"
        self.assertEqual(
            authorization.main(
                [
                    "issue",
                    *self.coordinates("android"),
                    "--evidence",
                    str(self.evidence),
                    "--release-policy",
                    str(self.release_policy),
                    "--artifact-root",
                    str(self.artifact_root),
                    "--release-verification-report",
                    str(self.verification_report),
                    "--output",
                    str(output),
                ]
            ),
            1,
        )

    def test_wrong_run_identity_and_zero_trust_digest_are_rejected(self) -> None:
        apple = self.issue("apple")
        original_promotion = self.promotion_id
        self.promotion_id = "5" * 64
        self.assertEqual(self.verify(apple), 1)
        self.promotion_id = original_promotion
        self.reviewed_source = "0" * 64
        self.assertEqual(self.verify(apple), 1)

    def test_noncanonical_duplicate_and_extra_json_members_are_rejected(self) -> None:
        apple = self.issue("apple")
        original = apple.read_text(encoding="utf-8")
        pretty = self.root / "pretty.json"
        pretty.write_text(json.dumps(json.loads(original), indent=2) + "\n", encoding="utf-8")
        self.assertEqual(self.verify(pretty), 1)

        duplicate = self.root / "duplicate.json"
        duplicate.write_text(
            original.replace(
                '"authorization":"privacy-production-enabled",',
                '"authorization":"privacy-production-enabled",'
                '"authorization":"privacy-production-enabled",',
                1,
            ),
            encoding="utf-8",
        )
        self.assertEqual(self.verify(duplicate), 1)

        extra_document = json.loads(original)
        extra_document["unexpected"] = True
        extra = self.root / "extra.json"
        extra.write_bytes(authorization._canonical_json(extra_document))
        self.assertEqual(self.verify(extra), 1)

    def test_issue_uses_exclusive_output_and_rejects_symlinked_evidence(self) -> None:
        output = self.issue("apple")
        self.assertEqual(
            authorization.main(
                [
                    "issue",
                    *self.coordinates("apple"),
                    "--evidence",
                    str(self.evidence),
                    "--release-policy",
                    str(self.release_policy),
                    "--artifact-root",
                    str(self.artifact_root),
                    "--release-verification-report",
                    str(self.verification_report),
                    "--output",
                    str(output),
                ]
            ),
            1,
        )
        alias = self.root / "evidence-alias.json"
        alias.symlink_to(self.evidence)
        second_output = self.root / "second.json"
        self.assertEqual(
            authorization.main(
                [
                    "issue",
                    *self.coordinates("apple"),
                    "--evidence",
                    str(alias),
                    "--release-policy",
                    str(self.release_policy),
                    "--artifact-root",
                    str(self.artifact_root),
                    "--release-verification-report",
                    str(self.verification_report),
                    "--output",
                    str(second_output),
                ]
            ),
            1,
        )

    def test_catalog_digest_changes_with_signed_metadata(self) -> None:
        before = authorization.release_catalog_sha256(self.artifact_root)
        record = self.release / "promotion-record-v4.norito"
        record.write_bytes(record.read_bytes() + b"changed\n")
        after = authorization.release_catalog_sha256(self.artifact_root)
        self.assertNotEqual(before, after)

    def test_selected_release_must_equal_manifest_digest(self) -> None:
        self.artifact_manifest = "8" * 64
        output = self.root / "wrong-release.json"
        self.assertEqual(
            authorization.main(
                [
                    "issue",
                    *self.coordinates("apple"),
                    "--evidence",
                    str(self.evidence),
                    "--release-policy",
                    str(self.release_policy),
                    "--artifact-root",
                    str(self.artifact_root),
                    "--release-verification-report",
                    str(self.verification_report),
                    "--output",
                    str(output),
                ]
            ),
            1,
        )

    def test_apple_artifact_requires_exact_production_binding(self) -> None:
        apple = self.issue("apple")
        digest = hashlib.sha256(apple.read_bytes()).hexdigest()
        package = self._create_apple_package(self.root / "apple-package", digest)
        self.assertEqual(
            authorization.main(
                [
                    "verify-apple-artifact",
                    *self._artifact_arguments(package),
                    "--release-tag",
                    "v1.2.3",
                    "--source-sha",
                    self.source_sha,
                    "--authorization-sha256",
                    digest,
                ]
            ),
            0,
        )
        with zipfile.ZipFile(package["archive"], "a") as output:
            output.writestr(
                "NoritoBridge.xcframework/ios-arm64/NoritoBridge.framework/Injected",
                b"substituted bytes\n",
            )
        self.assertEqual(
            authorization.main(
                [
                    "verify-apple-artifact",
                    *self._artifact_arguments(package),
                    "--release-tag",
                    "v1.2.3",
                    "--source-sha",
                    self.source_sha,
                    "--authorization-sha256",
                    digest,
                ]
            ),
            1,
        )

    def test_android_artifact_requires_exact_platform_binding(self) -> None:
        android = self.issue("android")
        digest = hashlib.sha256(android.read_bytes()).hexdigest()
        package = self._create_android_package(self.root / "android-package", digest)
        self.assertEqual(
            authorization.main(
                [
                    "verify-android-artifact",
                    *self._artifact_arguments(package),
                    "--release-tag",
                    "v1.2.3",
                    "--source-sha",
                    self.source_sha,
                    "--authorization-sha256",
                    digest,
                ]
            ),
            0,
        )
        archive_root = "iroha-mobile-sdk-android-v1.2.3"
        target = f"{archive_root}/native/arm64-v8a/libconnect_norito_bridge.so"
        with zipfile.ZipFile(package["archive"]) as original:
            members = [
                (info.filename, original.read(info))
                for info in original.infolist()
                if not info.is_dir()
            ]
        with zipfile.ZipFile(package["archive"], "w") as output:
            for name, payload in members:
                output.writestr(
                    name, b"substituted native bytes\n" if name == target else payload
                )
        self.assertEqual(
            authorization.main(
                [
                    "verify-android-artifact",
                    *self._artifact_arguments(package),
                    "--release-tag",
                    "v1.2.3",
                    "--source-sha",
                    self.source_sha,
                    "--authorization-sha256",
                    digest,
                ]
            ),
            1,
        )

    def test_artifacts_reject_wrong_authorization_provenance(self) -> None:
        apple = self.issue("apple")
        android = self.issue("android")
        wrong = "6" * 64
        apple_package = self._create_apple_package(
            self.root / "wrong-apple-package", wrong
        )
        android_package = self._create_android_package(
            self.root / "wrong-android-package", wrong
        )
        for command, package, expected in (
            ("verify-apple-artifact", apple_package, apple),
            ("verify-android-artifact", android_package, android),
        ):
            self.assertEqual(
                authorization.main(
                    [
                        command,
                        *self._artifact_arguments(package),
                        "--release-tag",
                        "v1.2.3",
                        "--source-sha",
                        self.source_sha,
                        "--authorization-sha256",
                        hashlib.sha256(expected.read_bytes()).hexdigest(),
                    ]
                ),
                1,
            )

    def test_android_artifact_rejects_unchecksummed_payload(self) -> None:
        android = self.issue("android")
        digest = hashlib.sha256(android.read_bytes()).hexdigest()
        package = self._create_android_package(
            self.root / "android-extra-package", digest
        )
        with zipfile.ZipFile(package["archive"], "a") as output:
            output.writestr(
                "iroha-mobile-sdk-android-v1.2.3/native/unlisted.so",
                b"unlisted bytes\n",
            )
        self.assertEqual(
            authorization.main(
                [
                    "verify-android-artifact",
                    *self._artifact_arguments(package),
                    "--release-tag",
                    "v1.2.3",
                    "--source-sha",
                    self.source_sha,
                    "--authorization-sha256",
                    digest,
                ]
            ),
            1,
        )

    def test_release_inventory_requires_exact_eight_and_fourteen_files(self) -> None:
        apple = self.issue("apple")
        android = self.issue("android")
        release = self.root / "release-assets"
        self._create_apple_package(
            release, hashlib.sha256(apple.read_bytes()).hexdigest()
        )
        self._create_android_package(
            release, hashlib.sha256(android.read_bytes()).hexdigest()
        )
        artifacts = argparse.Namespace(
            release_root=str(release), release_tag="v1.2.3", phase="artifacts"
        )
        artifacts_digest = authorization.verify_release_inventory(artifacts)
        self.assertRegex(artifacts_digest, r"^[0-9a-f]{64}$")
        for name in (
            "kagemusha-apple-production-authorization-v1.json",
            "kagemusha-apple-release-verification-report-v1.json",
            "kagemusha-apple-github-attestation-v1.json",
            "kagemusha-android-production-authorization-v1.json",
            "kagemusha-android-release-verification-report-v1.json",
            "kagemusha-android-github-attestation-v1.json",
        ):
            (release / name).write_bytes(f"authenticated {name}\n".encode("ascii"))
        final = argparse.Namespace(
            release_root=str(release), release_tag="v1.2.3", phase="final"
        )
        final_digest = authorization.verify_release_inventory(final)
        self.assertRegex(final_digest, r"^[0-9a-f]{64}$")
        self.assertNotEqual(artifacts_digest, final_digest)
        (release / "unexpected.txt").write_bytes(b"extra\n")
        with self.assertRaises(authorization.AuthorizationError):
            authorization.verify_release_inventory(final)


if __name__ == "__main__":
    unittest.main()
