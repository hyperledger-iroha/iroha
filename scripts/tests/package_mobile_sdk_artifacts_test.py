#!/usr/bin/env python3
"""Race and failure-safety tests for the mobile SDK package publisher."""

from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
import zipfile


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_OWNER = REPOSITORY_ROOT / "scripts/package_mobile_sdk_artifacts.sh"
LOCK_RUNNER = REPOSITORY_ROOT / "scripts/exec_with_file_lock.py"
PODSPEC_RENDERER = REPOSITORY_ROOT / "scripts/render_norito_bridge_podspec.py"
PODSPEC_TEMPLATE = (
    REPOSITORY_ROOT
    / "crates/connect_norito_bridge/NoritoBridge.podspec.template"
)
VERSION = "1.0.0"


class MobileSdkPackagePublisherTests(unittest.TestCase):
    def setUp(self) -> None:
        if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
            self.fail("tests require isolated Python 3.12")
        self.temporary = tempfile.TemporaryDirectory(
            prefix="mobile-sdk-package-owner-test."
        )
        self.temporary_root = Path(self.temporary.name).resolve(strict=True)
        self.repository = self.temporary_root / "repo"
        self.artifacts = self.temporary_root / "android-artifacts"
        self.output = self.temporary_root / "package-output/mobile-sdk"
        self.output.parent.mkdir()
        self._write_fixture()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_fixture(self) -> None:
        scripts = self.repository / "scripts"
        scripts.mkdir(parents=True)
        package_owner = scripts / PACKAGE_OWNER.name
        shutil.copy2(PACKAGE_OWNER, package_owner)
        owner_source = package_owner.read_text(encoding="utf-8")
        publication = (
            'no_replace_flag = 0x4 if sys.platform == "darwin" else 0x1\n'
            "rename_with_flag(stage, final, no_replace_flag)"
        )
        fixture_publication = (
            'no_replace_flag = 0x4 if sys.platform == "darwin" else 0x1\n'
            'destination_race = final.parent / ".package-test-destination-race"\n'
            'stage_race = final.parent / ".package-test-stage-race"\n'
            "if destination_race.exists():\n"
            "    final.mkdir()\n"
            '    (final / "competitor.txt").write_bytes(b"late competitor\\n")\n'
            "elif stage_race.exists():\n"
            '    retained = stage.with_name(f"{stage.name}.owner-retained")\n'
            "    stage.rename(retained)\n"
            "    stage.mkdir()\n"
            '    (stage / "competitor.txt").write_bytes(b"late competitor\\n")\n'
            "rename_with_flag(stage, final, no_replace_flag)"
        )
        self.assertEqual(owner_source.count(publication), 1)
        package_owner.write_text(
            owner_source.replace(publication, fixture_publication),
            encoding="utf-8",
        )
        shutil.copy2(LOCK_RUNNER, scripts / LOCK_RUNNER.name)
        shutil.copy2(PODSPEC_RENDERER, scripts / PODSPEC_RENDERER.name)
        swift_root = self.repository / "IrohaSwift"
        swift_root.mkdir()
        (swift_root / "VERSION").write_text(f"{VERSION}\n", encoding="ascii")
        template_parent = self.repository / "crates/connect_norito_bridge"
        template_parent.mkdir(parents=True)
        shutil.copy2(PODSPEC_TEMPLATE, template_parent / PODSPEC_TEMPLATE.name)
        checker = scripts / "check_mobile_sdk_artifacts.sh"
        checker.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env bash
                set -euo pipefail
                if [[ "${PACKAGE_TEST_CHECK_FAIL:-0}" == "1" ]]; then
                  echo "forced late package validation failure" >&2
                  exit 91
                fi
                """
            ),
            encoding="utf-8",
        )
        checker.chmod(0o755)

        gradle = self.artifacts / "gradle-build/iroha_kotlin_sdk"
        core_jar = gradle / f"core-jvm/libs/core-jvm-{VERSION}.jar"
        client = gradle / "client-android"
        aar = client / "outputs/aar/client-android-release.aar"
        native_root = client / "generated/jniLibs/default"
        provenance = (
            client
            / "generated/nativeProvenance/default/iroha/native-build-provenance-v1.json"
        )
        core_jar.parent.mkdir(parents=True)
        aar.parent.mkdir(parents=True)
        provenance.parent.mkdir(parents=True)
        core_jar.write_bytes(b"canonical core fixture\n")
        provenance_payload = json.dumps(
            {"privacy_production_enabled": False},
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8") + b"\n"
        provenance.write_bytes(provenance_payload)
        for abi in ("arm64-v8a", "x86_64"):
            library = native_root / abi / "libconnect_norito_bridge.so"
            library.parent.mkdir(parents=True)
            library.write_bytes(f"canonical {abi} fixture\n".encode("ascii"))
        with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_STORED) as archive:
            archive.writestr(
                "assets/iroha/native-build-provenance-v1.json",
                provenance_payload,
            )

    def _environment(self, **updates: str) -> dict[str, str]:
        environment = os.environ.copy()
        environment.pop("MOBILE_SDK_PACKAGE_LOCK_FDS", None)
        environment.pop("NORITO_BRIDGE_OUTPUT_LOCK_FD", None)
        environment.update(
            {
                "MOBILE_SDK_PYTHON_BINARY": str(
                    Path(sys.executable).resolve(strict=True)
                ),
                "MOBILE_SDK_ANDROID_ARTIFACT_DIR": str(self.artifacts),
                "MOBILE_SDK_PACKAGE_OUT_DIR": str(self.output),
            }
        )
        environment.update(updates)
        return environment

    def _package(
        self,
        *,
        mode: str = "android",
        version: str = VERSION,
        **updates: str,
    ) -> subprocess.CompletedProcess[str]:
        platform_arguments = [] if mode == "all" else [f"--{mode}"]
        return subprocess.run(
            [
                "/bin/bash",
                str(self.repository / "scripts/package_mobile_sdk_artifacts.sh"),
                "--root",
                str(self.repository),
                *platform_arguments,
                "--version",
                version,
            ],
            env=self._environment(**updates),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def _write_fake_apple_owner(self) -> dict[str, str]:
        artifact_root = self.temporary_root / "apple-artifacts"
        xcframework = artifact_root / "NoritoBridge.xcframework"
        xcframework.mkdir(parents=True)
        (xcframework / "Info.plist").write_bytes(b"canonical plist fixture\n")
        bridge_manifest = (
            json.dumps(
                {"schema_version": 1, "version": VERSION},
                sort_keys=True,
                separators=(",", ":"),
            ).encode("utf-8")
            + b"\n"
        )
        (artifact_root / "NoritoBridge.artifacts.json").write_bytes(bridge_manifest)
        (xcframework / "NoritoBridge.artifacts.json").write_bytes(bridge_manifest)
        owner = self.repository / "scripts/archive_norito_xcframework.py"
        owner.write_text(
            textwrap.dedent(
                """\
                import argparse
                import fcntl
                import os
                from pathlib import Path
                import stat
                import zipfile

                parser = argparse.ArgumentParser()
                parser.add_argument("--xcframework", required=True)
                parser.add_argument("--output", required=True)
                parser.add_argument("--scratch-dir", required=True)
                arguments = parser.parse_args()
                source = Path(arguments.xcframework)
                output = Path(arguments.output)
                scratch = Path(arguments.scratch_dir)
                if scratch != output.parent.parent:
                    raise SystemExit("Apple archive scratch directory was not external")
                required_seal_environment = {
                    "NORITO_BRIDGE_SEAL_HOME",
                    "NORITO_BRIDGE_SEAL_CARGO_HOME",
                    "NORITO_BRIDGE_SEAL_RUSTUP_HOME",
                    "NORITO_BRIDGE_SEAL_TMPDIR",
                    "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR",
                    "NORITO_BRIDGE_SEAL_CARGO",
                    "NORITO_BRIDGE_SEAL_RUSTC",
                    "NORITO_BRIDGE_SEAL_RUSTDOC",
                    "NORITO_BRIDGE_SEAL_RUSTUP",
                    "NORITO_BRIDGE_SEAL_DEVELOPER_DIR",
                }
                missing = sorted(required_seal_environment - os.environ.keys())
                if missing:
                    raise SystemExit(f"Apple seal environment was not forwarded: {missing}")
                for name in required_seal_environment:
                    value = Path(os.environ[name])
                    if not value.is_absolute() or not value.exists():
                        raise SystemExit(f"Apple seal environment is invalid: {name}")
                raw_descriptor = os.environ.get("NORITO_BRIDGE_OUTPUT_LOCK_FD", "")
                if not raw_descriptor.isdecimal():
                    raise SystemExit("Apple source lock descriptor was not forwarded")
                descriptor = int(raw_descriptor, 10)
                descriptor_metadata = os.fstat(descriptor)
                path_metadata = (source.parent / ".NoritoBridge.publish.lockfile").lstat()
                if (
                    not stat.S_ISREG(descriptor_metadata.st_mode)
                    or (descriptor_metadata.st_dev, descriptor_metadata.st_ino)
                    != (path_metadata.st_dev, path_metadata.st_ino)
                ):
                    raise SystemExit("Apple source lock descriptor does not match its path")
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
                if os.environ.get("SOURCE_DATE_EPOCH") != "1700000000":
                    raise SystemExit("SOURCE_DATE_EPOCH was not forwarded")
                output.parent.mkdir(parents=True, exist_ok=True)
                payload = (source.parent / "NoritoBridge.artifacts.json").read_bytes()
                with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as archive:
                    archive.writestr(
                        "NoritoBridge.xcframework/NoritoBridge.artifacts.json",
                        payload,
                    )
                """
            ),
            encoding="utf-8",
        )
        seal_root = self.temporary_root / "apple-seal"
        directories = {
            "NORITO_BRIDGE_SEAL_HOME": seal_root / "home",
            "NORITO_BRIDGE_SEAL_CARGO_HOME": seal_root / "cargo-home",
            "NORITO_BRIDGE_SEAL_RUSTUP_HOME": seal_root / "rustup-home",
            "NORITO_BRIDGE_SEAL_TMPDIR": seal_root / "tmp",
            "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR": seal_root / "cargo-target",
            "NORITO_BRIDGE_SEAL_DEVELOPER_DIR": seal_root / "developer",
        }
        for directory in directories.values():
            directory.mkdir(parents=True)
        tools = {}
        tool_root = seal_root / "tools"
        tool_root.mkdir()
        for environment_name, filename in (
            ("NORITO_BRIDGE_SEAL_CARGO", "cargo"),
            ("NORITO_BRIDGE_SEAL_RUSTC", "rustc"),
            ("NORITO_BRIDGE_SEAL_RUSTDOC", "rustdoc"),
            ("NORITO_BRIDGE_SEAL_RUSTUP", "rustup"),
        ):
            tool = tool_root / filename
            tool.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            tool.chmod(0o755)
            tools[environment_name] = tool
        return {
            name: str(path.resolve(strict=True))
            for name, path in {**directories, **tools}.items()
        } | {"MOBILE_SDK_APPLE_ARTIFACT_DIR": str(artifact_root)}

    def _seed_previous_release(self) -> Path:
        self.output.mkdir(parents=True, exist_ok=True)
        sentinel = self.output / "last-good-release.txt"
        sentinel.write_text("keep the last good package\n", encoding="utf-8")
        return sentinel

    def _publish_stages(self) -> list[Path]:
        return [
            path
            for path in self.output.parent.glob(".mobile-sdk.publish.*")
            if path.is_dir()
        ]

    def _assert_no_publish_stage(self) -> None:
        self.assertEqual(self._publish_stages(), [])

    def _inject_extra_package_file(self) -> None:
        owner = self.repository / "scripts/package_mobile_sdk_artifacts.sh"
        source = owner.read_text(encoding="utf-8")
        marker = "# PACKAGE_TEST_BEFORE_STAGE_INVENTORY\n"
        self.assertEqual(source.count(marker), 1)
        owner.write_text(
            source.replace(
                marker,
                marker
                + '(stage / "unexpected.txt").write_text('
                + '"unexpected\\n", encoding="utf-8")\n',
                1,
            ),
            encoding="utf-8",
        )

    def test_held_parent_lock_rejects_before_creating_output(self) -> None:
        lock_path = self.output.parent / ".mobile-sdk.publish.lockfile"
        lock_path.parent.mkdir(parents=True, exist_ok=True)
        descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self._package()
        finally:
            os.close(descriptor)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("another process holds", result.stderr)
        self.assertFalse(self.output.exists())
        self._assert_no_publish_stage()

    def test_preexisting_destination_is_rejected_without_replacement(self) -> None:
        sentinel = self._seed_previous_release()
        result = self._package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not already exist", result.stderr)
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep the last good package\n")
        self._assert_no_publish_stage()

    def test_late_validation_failure_retains_stage_and_leaves_output_absent(self) -> None:
        result = self._package(PACKAGE_TEST_CHECK_FAIL="1")
        self.assertEqual(result.returncode, 91, result.stderr)
        self.assertIn("forced late package validation failure", result.stderr)
        self.assertFalse(self.output.exists())
        self.assertEqual(len(self._publish_stages()), 1)
        self.assertIn("retained failed package stage", result.stderr)

    def test_extra_package_file_is_rejected_before_publication(self) -> None:
        self._inject_extra_package_file()
        result = self._package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "package stage does not contain the exact android file set",
            result.stderr,
        )
        self.assertFalse(self.output.exists())

    def test_combined_package_extra_file_is_rejected_before_publication(self) -> None:
        self._inject_extra_package_file()
        seal_environment = self._write_fake_apple_owner()
        result = self._package(
            mode="all",
            version="pr-9-deadbeef",
            SOURCE_DATE_EPOCH="1700000000",
            **seal_environment,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "package stage does not contain the exact all file set",
            result.stderr,
        )
        self.assertFalse(self.output.exists())

    def test_success_publishes_only_to_absent_destination(self) -> None:
        result = self._package()
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.output / f"iroha-mobile-sdk-android-{VERSION}.zip"
        manifest = self.output / f"mobile-sdk-android-{VERSION}.artifacts.json"
        checksums = self.output / f"SHA256SUMS-android-{VERSION}.txt"
        self.assertTrue(archive.is_file())
        self.assertTrue(manifest.is_file())
        self.assertTrue(checksums.is_file())
        self.assertFalse((self.output / ".NoritoBridge.archive.lockfile").exists())
        self._assert_no_publish_stage()
        self.assertEqual(
            len(list(self.output.parent.glob(".iroha-mobile-sdk-android-*.stage.*"))),
            1,
        )
        self.assertIn("retained Android package stage", result.stderr)

        payload = json.loads(manifest.read_text(encoding="utf-8"))
        self.assertEqual(payload["version"], VERSION)
        self.assertEqual(payload["mode"], "android")
        self.assertEqual(
            [entry["kind"] for entry in payload["artifacts"]],
            ["android-sdk"],
        )
        for line in checksums.read_text(encoding="utf-8").splitlines():
            expected, relative = line.split("  ", 1)
            artifact = self.output / relative
            self.assertTrue(artifact.is_file(), relative)
            self.assertEqual(hashlib.sha256(artifact.read_bytes()).hexdigest(), expected)

    def test_late_destination_competitor_is_preserved(self) -> None:
        (self.output.parent / ".package-test-destination-race").write_bytes(b"")
        result = self._package()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            (self.output / "competitor.txt").read_bytes(),
            b"late competitor\n",
        )
        self.assertEqual(len(self._publish_stages()), 1)

    def test_late_stage_swap_is_detected_without_removing_foreign_output(self) -> None:
        (self.output.parent / ".package-test-stage-race").write_bytes(b"")
        result = self._package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match the authenticated stage inode", result.stderr)
        self.assertEqual(
            (self.output / "competitor.txt").read_bytes(),
            b"late competitor\n",
        )
        retained = list(
            self.output.parent.glob(".mobile-sdk.publish.*.owner-retained")
        )
        self.assertEqual(len(retained), 1)
        self.assertTrue(
            (retained[0] / f"mobile-sdk-android-{VERSION}.artifacts.json").is_file()
        )

    def test_apple_checker_and_archiver_share_authenticated_source_lock(self) -> None:
        diagnostic_version = "pr-7-deadbeef"
        seal_environment = self._write_fake_apple_owner()
        result = self._package(
            mode="apple",
            version=diagnostic_version,
            SOURCE_DATE_EPOCH="1700000000",
            **seal_environment,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.output / f"NoritoBridge-v{VERSION}.xcframework.zip"
        versioned_manifest = self.output / f"NoritoBridge-v{VERSION}.artifacts.json"
        podspec = self.output / f"NoritoBridge-{VERSION}.podspec"
        self.assertTrue(
            archive.is_file()
        )
        self.assertTrue(versioned_manifest.is_file())
        self.assertTrue(podspec.is_file())
        self.assertFalse((self.output / ".NoritoBridge.archive.lockfile").exists())
        self._assert_no_publish_stage()

        rendered = podspec.read_text(encoding="utf-8")
        archive_sha256 = hashlib.sha256(archive.read_bytes()).hexdigest()
        self.assertIn(f"s.version          = '{VERSION}'", rendered)
        self.assertIn(
            f"releases/download/v{VERSION}/NoritoBridge-v{VERSION}.xcframework.zip",
            rendered,
        )
        self.assertIn(f":sha256 => '{archive_sha256}'", rendered)

        package_manifest = (
            self.output / f"mobile-sdk-apple-{diagnostic_version}.artifacts.json"
        )
        package_payload = json.loads(package_manifest.read_text(encoding="utf-8"))
        self.assertEqual(package_payload["version"], diagnostic_version)
        self.assertEqual(package_payload["apple_sdk_semver"], VERSION)
        self.assertEqual(
            [entry["kind"] for entry in package_payload["artifacts"]],
            [
                "apple-xcframework",
                "apple-manifest",
                "apple-cocoapods-podspec",
            ],
        )
        self.assertEqual(
            {entry["name"] for entry in package_payload["artifacts"]},
            {archive.name, versioned_manifest.name, podspec.name},
        )
        checksums = self.output / f"SHA256SUMS-apple-{diagnostic_version}.txt"
        checksum_paths = {
            line.split("  ", 1)[1]
            for line in checksums.read_text(encoding="utf-8").splitlines()
        }
        self.assertEqual(
            checksum_paths,
            {archive.name, versioned_manifest.name, podspec.name, package_manifest.name},
        )

    def test_apple_manifest_version_must_match_pod_version(self) -> None:
        seal_environment = self._write_fake_apple_owner()
        artifact_root = Path(seal_environment["MOBILE_SDK_APPLE_ARTIFACT_DIR"])
        drifted = b'{"schema_version":1,"version":"9.9.9"}\n'
        (artifact_root / "NoritoBridge.artifacts.json").write_bytes(drifted)
        (artifact_root / "NoritoBridge.xcframework/NoritoBridge.artifacts.json").write_bytes(
            drifted
        )
        result = self._package(
            mode="apple",
            version="pr-8-deadbeef",
            SOURCE_DATE_EPOCH="1700000000",
            **seal_environment,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "embedded NoritoBridge manifest version must equal IrohaSwift/VERSION",
            result.stderr,
        )
        self.assertFalse(self.output.exists())

    def test_repository_local_and_implicit_outputs_are_rejected(self) -> None:
        implicit_environment = self._environment()
        implicit_environment.pop("MOBILE_SDK_PACKAGE_OUT_DIR")
        implicit = subprocess.run(
            [
                "/bin/bash",
                str(self.repository / "scripts/package_mobile_sdk_artifacts.sh"),
                "--root",
                str(self.repository),
                "--android",
                "--version",
                VERSION,
            ],
            env=implicit_environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertNotEqual(implicit.returncode, 0)
        self.assertIn("MOBILE_SDK_PACKAGE_OUT_DIR is required", implicit.stderr)

        repository_output = self.repository / "dist/mobile-sdk"
        repository_output.parent.mkdir(parents=True, exist_ok=True)
        confined = self._package(MOBILE_SDK_PACKAGE_OUT_DIR=str(repository_output))
        self.assertNotEqual(confined.returncode, 0)
        self.assertIn("outside the Iroha source tree", confined.stderr)
        self.assertFalse(repository_output.exists())

        real_parent = self.temporary_root / "canonical-package-parent/nested"
        real_parent.mkdir(parents=True)
        linked_ancestor = self.temporary_root / "linked-package-parent"
        linked_ancestor.symlink_to(real_parent.parent, target_is_directory=True)
        linked_output = linked_ancestor / "nested/mobile-sdk"
        no_follow = self._package(MOBILE_SDK_PACKAGE_OUT_DIR=str(linked_output))
        self.assertNotEqual(no_follow.returncode, 0)
        self.assertIn("must not traverse symbolic links", no_follow.stderr)
        self.assertFalse((real_parent / "mobile-sdk").exists())


if __name__ == "__main__":
    unittest.main()
