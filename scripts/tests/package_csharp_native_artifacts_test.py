#!/usr/bin/env python3
"""Tests for deterministic, fail-closed C# ABI-23 native NuGet packaging."""

from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import check_native_sdk_abi22_artifact as artifact_checker  # noqa: E402
import package_csharp_native_artifacts as packager  # noqa: E402


SOURCE_COMMIT = "a" * 40
CONTRACT_ERRORS = (
    packager.CSharpNativePackageError,
    artifact_checker.ArtifactContractError,
)


def clean_source_state(_root: Path) -> tuple[str, bool]:
    """Return a deterministic clean source state for isolated package tests."""

    return SOURCE_COMMIT, True


def write_inputs(root: Path) -> dict[str, bytes]:
    """Write one exact synthetic five-target evidence inventory."""

    payloads: dict[str, bytes] = {}
    for index, asset in enumerate(packager.NATIVE_ASSETS):
        target_root = root / asset.target
        target_root.mkdir(parents=True)
        payload = f"native-{index}-{asset.target}\n".encode()
        payloads[asset.package_path] = payload
        artifact_path = target_root / asset.library_name
        artifact_path.write_bytes(payload)
        manifest = {
            "artifact_sha256": hashlib.sha256(payload).hexdigest(),
            "artifact_size": len(payload),
            "bridge_abi_version": artifact_checker.REQUIRED_BRIDGE_ABI_VERSION,
            "privacy_c_exports": list(
                artifact_checker.APPROVED_PRIVACY_C_EXPORTS
            ),
            "privacy_c_exports_inspected": True,
            "required_symbols": list(artifact_checker.REQUIRED_SYMBOLS["csharp"]),
            "schema": artifact_checker.SCHEMA,
            "sdk": "csharp",
            "source_commit": SOURCE_COMMIT,
            "source_tree_clean": True,
            "target": asset.target,
            "workspace_source_manifest_sha256": "b" * 64,
        }
        (target_root / packager.EVIDENCE_MANIFEST_NAME).write_bytes(
            artifact_checker.canonical_manifest_bytes(manifest)
        )
    return payloads


def snapshot(root: Path) -> dict[str, bytes]:
    """Return the byte-exact regular-file inventory below a test directory."""

    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def write_package(path: Path, payloads: dict[str, bytes], *, extra_runtime: bool = False) -> None:
    """Write a minimal NuGet-shaped ZIP containing the requested native bytes."""

    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("_rels/.rels", b"<Relationships />")
        for package_path, payload in payloads.items():
            archive.writestr(package_path, payload)
        if extra_runtime:
            archive.writestr("runtimes/linux-musl-x64/native/libconnect_norito_bridge.so", b"extra")


class CSharpNativePackageTests(unittest.TestCase):
    """Exercise inventory, provenance, deterministic staging, and package checks."""

    def test_exact_rust_target_to_nuget_rid_mapping(self) -> None:
        self.assertEqual(
            tuple(
                (asset.target, asset.rid, asset.library_name)
                for asset in packager.NATIVE_ASSETS
            ),
            (
                ("x86_64-unknown-linux-gnu", "linux-x64", "libconnect_norito_bridge.so"),
                ("aarch64-unknown-linux-gnu", "linux-arm64", "libconnect_norito_bridge.so"),
                ("x86_64-apple-darwin", "osx-x64", "libconnect_norito_bridge.dylib"),
                ("aarch64-apple-darwin", "osx-arm64", "libconnect_norito_bridge.dylib"),
                ("x86_64-pc-windows-msvc", "win-x64", "connect_norito_bridge.dll"),
            ),
        )

    def test_stage_is_byte_deterministic_and_verifiable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = root / "inputs"
            inputs.mkdir()
            write_inputs(inputs)
            first = root / "first"
            second = root / "second"
            first_manifest = packager.stage_package(
                inputs,
                first,
                root,
                source_state=clean_source_state,
            )
            second_manifest = packager.stage_package(
                inputs,
                second,
                root,
                source_state=clean_source_state,
            )
            self.assertEqual(first_manifest, second_manifest)
            self.assertEqual(snapshot(first), snapshot(second))
            self.assertEqual(
                packager.verify_stage(first, root, source_state=clean_source_state),
                first_manifest,
            )

    def test_missing_extra_and_stale_inputs_fail_closed(self) -> None:
        for mutation in ("missing", "extra", "stale"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                inputs = root / "inputs"
                inputs.mkdir()
                write_inputs(inputs)
                first_asset = packager.NATIVE_ASSETS[0]
                if mutation == "missing":
                    (
                        inputs
                        / first_asset.target
                        / packager.EVIDENCE_MANIFEST_NAME
                    ).unlink()
                elif mutation == "extra":
                    (inputs / "unreviewed-target").mkdir()
                else:
                    (
                        inputs
                        / first_asset.target
                        / first_asset.library_name
                    ).write_bytes(b"stale replacement")
                with self.assertRaises(CONTRACT_ERRORS):
                    packager.stage_package(
                        inputs,
                        root / "stage",
                        root,
                        source_state=clean_source_state,
                    )

    def test_wrong_abi_and_stale_commit_fail_closed(self) -> None:
        for field, value in (
            ("bridge_abi_version", 20),
            ("source_commit", "b" * 40),
        ):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                inputs = root / "inputs"
                inputs.mkdir()
                write_inputs(inputs)
                asset = packager.NATIVE_ASSETS[0]
                manifest_path = inputs / asset.target / packager.EVIDENCE_MANIFEST_NAME
                manifest = json.loads(manifest_path.read_text(encoding="ascii"))
                manifest[field] = value
                manifest_path.write_text(
                    json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
                    encoding="ascii",
                )
                with self.assertRaises(CONTRACT_ERRORS):
                    packager.stage_package(
                        inputs,
                        root / "stage",
                        root,
                        source_state=clean_source_state,
                    )

    def test_stage_rejects_extra_or_substituted_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = root / "inputs"
            inputs.mkdir()
            write_inputs(inputs)
            stage = root / "stage"
            packager.stage_package(inputs, stage, root, source_state=clean_source_state)
            (stage / "extra.bin").write_bytes(b"extra")
            with self.assertRaises(CONTRACT_ERRORS):
                packager.verify_stage(stage, root, source_state=clean_source_state)

    def test_package_requires_exact_runtime_inventory_and_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = root / "inputs"
            inputs.mkdir()
            payloads = write_inputs(inputs)
            stage = root / "stage"
            packager.stage_package(inputs, stage, root, source_state=clean_source_state)

            valid_package = root / "valid.nupkg"
            write_package(valid_package, payloads)
            packager.verify_package(
                valid_package,
                stage,
                root,
                source_state=clean_source_state,
            )

            extra_package = root / "extra.nupkg"
            write_package(extra_package, payloads, extra_runtime=True)
            with self.assertRaises(CONTRACT_ERRORS):
                packager.verify_package(
                    extra_package,
                    stage,
                    root,
                    source_state=clean_source_state,
                )

            stale_payloads = dict(payloads)
            stale_payloads[packager.NATIVE_ASSETS[0].package_path] = b"replacement"
            stale_package = root / "stale.nupkg"
            write_package(stale_package, stale_payloads)
            with self.assertRaises(CONTRACT_ERRORS):
                packager.verify_package(
                    stale_package,
                    stage,
                    root,
                    source_state=clean_source_state,
                )

    def test_repository_pack_and_workflow_contract(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/pr_csharp.yml").read_text(
            encoding="utf-8"
        )
        project = (
            REPO_ROOT
            / "csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj"
        ).read_text(encoding="utf-8-sig")
        consumer = (
            REPO_ROOT / "ci/check_csharp_sdk_package_consumer.sh"
        ).read_text(encoding="utf-8")

        reviewed_rows = (
            (
                "ubuntu-24.04",
                "x86_64-unknown-linux-gnu",
                "linux-x64",
                "libconnect_norito_bridge.so",
            ),
            (
                "ubuntu-24.04-arm",
                "aarch64-unknown-linux-gnu",
                "linux-arm64",
                "libconnect_norito_bridge.so",
            ),
            (
                "macos-15-intel",
                "x86_64-apple-darwin",
                "osx-x64",
                "libconnect_norito_bridge.dylib",
            ),
            (
                "macos-14",
                "aarch64-apple-darwin",
                "osx-arm64",
                "libconnect_norito_bridge.dylib",
            ),
            (
                "windows-latest",
                "x86_64-pc-windows-msvc",
                "win-x64",
                "connect_norito_bridge.dll",
            ),
        )
        native_job = workflow[
            workflow.index("  native-release-artifacts:")
            : workflow.index("  build-test-pack:")
        ]
        smoke_job = workflow[
            workflow.index("  native-package-smoke:")
            : workflow.index("  kagemusha-native-bridge-sdk:")
        ]
        for runner, target, rid, library_name in reviewed_rows:
            row = (
                f"          - os: {runner}\n"
                f"            target: {target}\n"
                f"            rid: {rid}\n"
                f"            library_name: {library_name}"
            )
            self.assertIn(row, native_job)
            self.assertIn(row, smoke_job)
            self.assertIn(f"runtimes/{rid}/native/{library_name}", project)

        self.assertEqual(native_job.count("check_native_sdk_abi22_artifact.py"), 2)
        self.assertIn("--sdk csharp", native_job)
        self.assertIn('GIT_CONFIG_COUNT: "1"', native_job)
        self.assertIn("GIT_CONFIG_KEY_0: core.autocrlf", native_job)
        self.assertIn('GIT_CONFIG_VALUE_0: "false"', native_job)
        self.assertIn("cache-targets: false", native_job)
        self.assertIn('if [[ "$host_target" != "$target" ]]', native_job)
        self.assertIn('if [[ -e target ]]', native_job)
        self.assertIn('cp "target/$target/release/$library_name" "$artifact"', native_job)
        self.assertIn("package_csharp_native_artifacts.py stage", workflow)
        self.assertIn("package_csharp_native_artifacts.py verify-package", workflow)
        self.assertIn(
            "python3 -I scripts/tests/package_csharp_native_artifacts_test.py",
            workflow,
        )
        self.assertIn("BeforeTargets=\"GenerateNuspec\"", project)
        self.assertIn("$(IrohaNativePackageScript)&quot; verify-stage", project)
        self.assertIn("SoraFsReferenceValidators.IsAppealFinanceAvailable()", consumer)
        self.assertIn("CSHARP_SDK_PACKAGE_CONSUMER_RUNTIME_IDENTIFIER", consumer)


if __name__ == "__main__":
    unittest.main()
