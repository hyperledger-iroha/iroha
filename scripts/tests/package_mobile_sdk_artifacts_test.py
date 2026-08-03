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
        shutil.copy2(PACKAGE_OWNER, scripts / PACKAGE_OWNER.name)
        shutil.copy2(LOCK_RUNNER, scripts / LOCK_RUNNER.name)
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
        **updates: str,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "/bin/bash",
                str(self.repository / "scripts/package_mobile_sdk_artifacts.sh"),
                "--root",
                str(self.repository),
                f"--{mode}",
                "--version",
                VERSION,
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
        bridge_manifest = b'{"schema_version":1}\n'
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
                arguments = parser.parse_args()
                source = Path(arguments.xcframework)
                output = Path(arguments.output)
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
                (output.parent / ".NoritoBridge.archive.lockfile").write_bytes(b"")
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

    def _assert_no_stage(self) -> None:
        stages = [
            path
            for path in self.output.parent.glob(".mobile-sdk.publish.*")
            if path.is_dir()
        ]
        self.assertEqual(stages, [])

    def test_held_parent_lock_rejects_without_touching_previous_release(self) -> None:
        sentinel = self._seed_previous_release()
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
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep the last good package\n")
        self._assert_no_stage()

    def test_late_validation_failure_preserves_previous_release(self) -> None:
        sentinel = self._seed_previous_release()
        result = self._package(PACKAGE_TEST_CHECK_FAIL="1")
        self.assertEqual(result.returncode, 91, result.stderr)
        self.assertIn("forced late package validation failure", result.stderr)
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "keep the last good package\n")
        self._assert_no_stage()

    def test_success_atomically_replaces_previous_release(self) -> None:
        sentinel = self._seed_previous_release()
        result = self._package()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(sentinel.exists())
        archive = self.output / f"iroha-mobile-sdk-android-{VERSION}.zip"
        manifest = self.output / f"mobile-sdk-android-{VERSION}.artifacts.json"
        checksums = self.output / f"SHA256SUMS-android-{VERSION}.txt"
        self.assertTrue(archive.is_file())
        self.assertTrue(manifest.is_file())
        self.assertTrue(checksums.is_file())
        self.assertFalse((self.output / ".NoritoBridge.archive.lockfile").exists())
        self._assert_no_stage()

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

    def test_apple_checker_and_archiver_share_authenticated_source_lock(self) -> None:
        seal_environment = self._write_fake_apple_owner()
        sentinel = self._seed_previous_release()
        result = self._package(
            mode="apple",
            SOURCE_DATE_EPOCH="1700000000",
            **seal_environment,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(sentinel.exists())
        self.assertTrue(
            (self.output / f"NoritoBridge-{VERSION}.xcframework.zip").is_file()
        )
        self.assertTrue(
            (self.output / f"NoritoBridge-{VERSION}.artifacts.json").is_file()
        )
        self.assertFalse((self.output / ".NoritoBridge.archive.lockfile").exists())
        self._assert_no_stage()

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
