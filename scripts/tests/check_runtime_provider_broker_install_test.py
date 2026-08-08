#!/usr/bin/env python3
"""Focused tests for runtime-provider broker deployment assets."""

from __future__ import annotations

import contextlib
import hashlib
import io
import os
import plistlib
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import check_runtime_provider_broker_install as checker  # noqa: E402


ASSET_ROOT = REPO_ROOT / "configs" / "sorafs" / "runtime_provider_broker"
SYSTEMD_UNIT = (
    ASSET_ROOT / "systemd" / "iroha-runtime-provider-broker-v1.service"
)
TAIRA_DROP_IN = (
    ASSET_ROOT
    / "systemd"
    / "taira-irohad.service.d"
    / "20-runtime-provider-broker-v1.conf"
)
GOVERNANCE_DROP_IN = (
    ASSET_ROOT
    / "systemd"
    / "sorafs-governance-dag@.service.d"
    / "20-runtime-provider-broker-v1.conf"
)
LAUNCHD_PLIST = (
    ASSET_ROOT
    / "launchd"
    / "org.hyperledger.iroha.runtime-provider-broker-v1.plist"
)


class RuntimeProviderBrokerInstallCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.install_root = self.base / "root"
        self.install_root.mkdir(mode=0o700)
        self.expected_catalog = self.base / "expected-catalog.norito"
        self.catalog_bytes = b"canonical-public-runtime-provider-catalog-v1"
        self.expected_catalog.write_bytes(self.catalog_bytes)
        self.executable_bytes = b"deployment-owned-broker"
        self.expected_executable_sha256 = hashlib.sha256(
            self.executable_bytes
        ).hexdigest()
        self.service_uid = os.getuid()
        self.service_gid = os.getgid()

    def install_valid_layout(self, platform: str) -> checker.InstallLayout:
        layout = checker.LAYOUTS[platform]
        executable = checker._under_root(self.install_root, layout.executable)
        executable.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        if executable.exists():
            executable.chmod(0o755)
        executable.write_bytes(self.executable_bytes)
        executable.chmod(0o555)

        catalog = checker._under_root(self.install_root, layout.catalog)
        catalog.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        if catalog.exists():
            catalog.chmod(0o640)
        catalog.write_bytes(self.catalog_bytes)
        catalog.chmod(0o440)

        runtime_directory = checker._under_root(
            self.install_root, layout.runtime_directory
        )
        runtime_directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        runtime_directory.chmod(0o700)

        supervisor = checker._under_root(
            self.install_root, layout.supervisor_asset
        )
        supervisor.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        if supervisor.exists():
            supervisor.chmod(0o644)
        supervisor.write_bytes(
            checker._expected_supervisor_template(layout).read_bytes()
        )
        supervisor.chmod(0o444)
        for installed_asset, template_asset in layout.consumer_assets:
            installed = checker._under_root(self.install_root, installed_asset)
            installed.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
            if installed.exists():
                installed.chmod(0o644)
            installed.write_bytes(
                checker._expected_repository_asset(template_asset).read_bytes()
            )
            installed.chmod(0o444)
        return layout

    def validate(
        self,
        layout: checker.InstallLayout,
        *,
        check_runtime_directory: bool = True,
    ) -> None:
        checker.validate_install(
            layout=layout,
            install_root=self.install_root,
            expected_catalog=self.expected_catalog,
            expected_executable_sha256=self.expected_executable_sha256,
            service_uid=self.service_uid,
            service_gid=self.service_gid,
            trusted_artifact_owner_uid=self.service_uid,
            check_runtime_directory=check_runtime_directory,
        )

    def test_complete_linux_install_passes(self) -> None:
        self.validate(self.install_valid_layout("linux"))

    def test_complete_macos_install_passes(self) -> None:
        self.validate(self.install_valid_layout("macos"))

    def test_nonempty_catalog_requires_broker_executable(self) -> None:
        layout = self.install_valid_layout("linux")
        checker._under_root(self.install_root, layout.executable).unlink()
        with self.assertRaisesRegex(
            checker.InstallCheckError, "executable is not installed"
        ):
            self.validate(layout)

    def test_nonempty_catalog_requires_installed_catalog(self) -> None:
        layout = self.install_valid_layout("linux")
        checker._under_root(self.install_root, layout.catalog).unlink()
        with self.assertRaisesRegex(
            checker.InstallCheckError, "catalog is not installed"
        ):
            self.validate(layout)

    def test_installed_catalog_must_match_expected_bytes(self) -> None:
        layout = self.install_valid_layout("linux")
        catalog = checker._under_root(self.install_root, layout.catalog)
        catalog.chmod(0o600)
        catalog.write_bytes(b"different-public-catalog")
        catalog.chmod(0o440)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "differs from the expected canonical bytes"
        ):
            self.validate(layout)

    def test_empty_expected_catalog_is_not_a_disabled_marker(self) -> None:
        layout = self.install_valid_layout("linux")
        self.expected_catalog.write_bytes(b"")
        with self.assertRaisesRegex(
            checker.InstallCheckError, "expected runtime-provider catalog is empty"
        ):
            self.validate(layout)

    def test_oversized_expected_catalog_is_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        self.expected_catalog.write_bytes(
            b"x" * (checker.CATALOG_MAX_BYTES_V1 + 1)
        )
        with self.assertRaisesRegex(
            checker.InstallCheckError, "exceeds the V1 byte limit"
        ):
            self.validate(layout)

    def test_symlink_catalog_is_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        catalog = checker._under_root(self.install_root, layout.catalog)
        target = catalog.with_name("catalog-target.norito")
        catalog.rename(target)
        catalog.symlink_to(target.name)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "non-symlink regular file"
        ):
            self.validate(layout)

    def test_writable_executable_is_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        executable = checker._under_root(self.install_root, layout.executable)
        executable.chmod(0o755)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "unsafe mode bits"
        ):
            self.validate(layout)

    def test_executable_special_mode_bits_are_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        executable = checker._under_root(self.install_root, layout.executable)
        executable.chmod(0o4555)
        with self.assertRaisesRegex(checker.InstallCheckError, "unsafe mode bits"):
            self.validate(layout)

    def test_executable_digest_must_match_external_release_identity(self) -> None:
        layout = self.install_valid_layout("linux")
        self.expected_executable_sha256 = hashlib.sha256(b"different-binary").hexdigest()
        with self.assertRaisesRegex(
            checker.InstallCheckError, "externally verified release digest"
        ):
            self.validate(layout)

    def test_executable_digest_must_be_canonical_nonzero_lowercase(self) -> None:
        layout = self.install_valid_layout("linux")
        for invalid in ("0" * 64, "A" * 64, "abc"):
            with self.subTest(invalid=invalid):
                self.expected_executable_sha256 = invalid
                with self.assertRaisesRegex(
                    checker.InstallCheckError, "not canonical non-zero lowercase hex"
                ):
                    self.validate(layout)

    def test_executable_race_is_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        executable = checker._under_root(self.install_root, layout.executable)
        executable_inode = executable.stat().st_ino
        original_read = os.read
        mutated = False

        def racing_read(descriptor: int, count: int) -> bytes:
            nonlocal mutated
            chunk = original_read(descriptor, count)
            if not mutated and os.fstat(descriptor).st_ino == executable_inode:
                mutated = True
                executable.chmod(0o755)
                executable.write_bytes(b"changed-during-executable-read")
                executable.chmod(0o555)
            return chunk

        with mock.patch.object(checker.os, "read", side_effect=racing_read):
            with self.assertRaisesRegex(
                checker.InstallCheckError, "changed while it was checked"
            ):
                self.validate(layout)
        self.assertTrue(mutated)

    def test_catalog_must_be_readable_by_service_identity(self) -> None:
        layout = self.install_valid_layout("linux")
        catalog = checker._under_root(self.install_root, layout.catalog)
        catalog.chmod(0o000)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "cannot be opened safely"
        ):
            self.validate(layout)

    def test_catalog_owner_write_and_special_bits_are_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        catalog = checker._under_root(self.install_root, layout.catalog)
        for mode in (0o640, 0o444 | stat.S_ISGID):
            with self.subTest(mode=oct(mode)):
                catalog.chmod(mode)
                with self.assertRaisesRegex(
                    checker.InstallCheckError, "unsafe mode bits"
                ):
                    self.validate(layout)
        catalog.chmod(0o440)

    def test_runtime_directory_requires_exact_mode(self) -> None:
        layout = self.install_valid_layout("linux")
        runtime_directory = checker._under_root(
            self.install_root, layout.runtime_directory
        )
        runtime_directory.chmod(0o750)
        with self.assertRaisesRegex(checker.InstallCheckError, "mode is not 0700"):
            self.validate(layout)

    def test_macos_runtime_directory_is_unconditional(self) -> None:
        layout = self.install_valid_layout("macos")
        checker._under_root(
            self.install_root, layout.runtime_directory
        ).rmdir()
        with self.assertRaisesRegex(
            checker.InstallCheckError, "installation directory is missing"
        ):
            self.validate(layout, check_runtime_directory=False)

    def test_supervisor_asset_is_required(self) -> None:
        layout = self.install_valid_layout("linux")
        checker._under_root(self.install_root, layout.supervisor_asset).unlink()
        with self.assertRaisesRegex(
            checker.InstallCheckError, "supervisor asset is not installed"
        ):
            self.validate(layout)

    def test_supervisor_asset_must_not_be_a_symlink(self) -> None:
        layout = self.install_valid_layout("macos")
        supervisor = checker._under_root(
            self.install_root, layout.supervisor_asset
        )
        target = supervisor.with_name("supervisor-target.plist")
        supervisor.rename(target)
        supervisor.symlink_to(target.name)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "non-symlink regular file"
        ):
            self.validate(layout)

    def test_supervisor_asset_must_have_one_hard_link(self) -> None:
        layout = self.install_valid_layout("linux")
        supervisor = checker._under_root(
            self.install_root, layout.supervisor_asset
        )
        os.link(supervisor, supervisor.with_name("supervisor-alias.service"))
        with self.assertRaisesRegex(checker.InstallCheckError, "one hard link"):
            self.validate(layout)

    def test_supervisor_asset_rejects_unsafe_permissions(self) -> None:
        layout = self.install_valid_layout("linux")
        supervisor = checker._under_root(
            self.install_root, layout.supervisor_asset
        )
        supervisor.chmod(0o664)
        with self.assertRaisesRegex(checker.InstallCheckError, "unsafe mode bits"):
            self.validate(layout)

    def test_supervisor_asset_rejects_untrusted_owner(self) -> None:
        layout = self.install_valid_layout("linux")
        with self.assertRaisesRegex(checker.InstallCheckError, "untrusted owner"):
            checker.validate_install(
                layout=layout,
                install_root=self.install_root,
                expected_catalog=self.expected_catalog,
                expected_executable_sha256=self.expected_executable_sha256,
                service_uid=self.service_uid,
                service_gid=self.service_gid,
                trusted_artifact_owner_uid=self.service_uid + 1,
                check_runtime_directory=True,
            )

    def test_supervisor_asset_must_match_checked_in_template(self) -> None:
        layout = self.install_valid_layout("macos")
        supervisor = checker._under_root(
            self.install_root, layout.supervisor_asset
        )
        supervisor.chmod(0o644)
        supervisor.write_bytes(b"tampered-launchd-supervisor")
        supervisor.chmod(0o444)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "differs from the checked-in platform template"
        ):
            self.validate(layout)

    def test_supervisor_asset_race_is_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        supervisor = checker._under_root(
            self.install_root, layout.supervisor_asset
        )
        supervisor_inode = supervisor.stat().st_ino
        original_read = os.read
        mutated = False

        def racing_read(descriptor: int, count: int) -> bytes:
            nonlocal mutated
            chunk = original_read(descriptor, count)
            if not mutated and os.fstat(descriptor).st_ino == supervisor_inode:
                mutated = True
                supervisor.chmod(0o644)
                supervisor.write_bytes(b"changed-during-supervisor-read")
                supervisor.chmod(0o444)
            return chunk

        with mock.patch.object(checker.os, "read", side_effect=racing_read):
            with self.assertRaisesRegex(
                checker.InstallCheckError, "changed while it was checked"
            ):
                self.validate(layout)
        self.assertTrue(mutated)

    def test_consumer_drop_ins_are_required_and_exact(self) -> None:
        layout = self.install_valid_layout("linux")
        installed_asset, _ = layout.consumer_assets[0]
        installed = checker._under_root(self.install_root, installed_asset)
        installed.unlink()
        with self.assertRaisesRegex(
            checker.InstallCheckError, "consumer drop-in is not installed"
        ):
            self.validate(layout)

        layout = self.install_valid_layout("linux")
        installed_asset, _ = layout.consumer_assets[1]
        installed = checker._under_root(self.install_root, installed_asset)
        installed.chmod(0o644)
        with self.assertRaisesRegex(checker.InstallCheckError, "unsafe mode bits"):
            self.validate(layout)

        installed.chmod(0o644)
        installed.write_bytes(b"tampered-consumer-drop-in")
        installed.chmod(0o444)
        with self.assertRaisesRegex(
            checker.InstallCheckError, "differs from the checked-in platform template"
        ):
            self.validate(layout)

    def test_consumer_drop_in_hardlink_is_rejected(self) -> None:
        layout = self.install_valid_layout("linux")
        installed_asset, _ = layout.consumer_assets[0]
        installed = checker._under_root(self.install_root, installed_asset)
        os.link(installed, installed.with_name("consumer-drop-in-alias.conf"))
        with self.assertRaisesRegex(checker.InstallCheckError, "one hard link"):
            self.validate(layout)

    def test_enabled_cli_requires_external_executable_digest_before_identity_lookup(self) -> None:
        self.install_valid_layout("linux")
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            result = checker.main(
                [
                    "--platform",
                    "linux",
                    "--install-root",
                    str(self.install_root),
                    "--expected-catalog",
                    str(self.expected_catalog),
                ]
            )
        self.assertEqual(result, 1)
        self.assertIn("--expected-executable-sha256 is required", errors.getvalue())

    def test_disabled_cli_path_needs_no_service_account_or_files(self) -> None:
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = checker.main(
                [
                    "--platform",
                    "linux",
                    "--install-root",
                    str(self.install_root),
                    "--runtime-providers-disabled",
                ]
            )
        self.assertEqual(result, 0)
        self.assertIn("disabled", output.getvalue())

    def test_disabled_cli_path_rejects_installed_catalog(self) -> None:
        self.install_valid_layout("linux")
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            result = checker.main(
                [
                    "--platform",
                    "linux",
                    "--install-root",
                    str(self.install_root),
                    "--runtime-providers-disabled",
                ]
            )
        self.assertEqual(result, 1)
        self.assertIn("contains a fixed catalog artifact", errors.getvalue())

    def test_disabled_cli_path_rejects_missing_install_root(self) -> None:
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            result = checker.main(
                [
                    "--platform",
                    "linux",
                    "--install-root",
                    str(self.base / "missing-root"),
                    "--runtime-providers-disabled",
                ]
            )
        self.assertEqual(result, 1)
        self.assertIn("--install-root does not exist", errors.getvalue())

    def test_fixed_layouts_name_only_supported_authenticated_transports(self) -> None:
        self.assertEqual(set(checker.LAYOUTS), {"linux", "macos"})
        self.assertEqual(
            checker.LAYOUTS["linux"].socket,
            Path(
                "/run/iroha-runtime-provider-broker-v1/"
                "runtime-provider-broker-v1.sock"
            ),
        )
        self.assertEqual(
            checker.LAYOUTS["macos"].socket,
            Path("/private/var/iroha/run/runtime-provider-broker-v1.sock"),
        )


class RuntimeProviderBrokerSupervisorAssetTests(unittest.TestCase):
    def test_systemd_unit_uses_fixed_catalog_uid_and_runtime_directory(self) -> None:
        unit = SYSTEMD_UNIT.read_text(encoding="utf-8")
        for required in (
            "Type=notify",
            "NotifyAccess=main",
            "User=iroha",
            "Group=iroha",
            "RuntimeDirectory=iroha-runtime-provider-broker-v1",
            "RuntimeDirectoryMode=0700",
            "RuntimeDirectoryPreserve=no",
            "LimitCORE=0",
            "NoNewPrivileges=true",
            "ProtectSystem=strict",
            "PrivateTmp=true",
            (
                "ExecStart=/usr/local/libexec/iroha-runtime-provider-broker-v1 "
                "--catalog /etc/iroha/runtime-provider-broker/catalog.norito"
            ),
        ):
            self.assertIn(required, unit)
        self.assertNotIn("Environment=", unit)
        self.assertNotIn("EnvironmentFile=", unit)
        self.assertNotIn("PrivateUsers=true", unit)
        self.assertNotIn("--socket", unit)
        self.assertNotIn("--plugin", unit)
        self.assertNotIn("--private-key", unit)

    def test_consumer_drop_ins_require_and_order_after_broker(self) -> None:
        expected = (
            "[Unit]\n"
            "Requires=iroha-runtime-provider-broker-v1.service\n"
            "After=iroha-runtime-provider-broker-v1.service\n"
            "\n"
            "[Service]\n"
            "User=iroha\n"
            "Group=iroha\n"
            "ReadOnlyPaths=/run/iroha-runtime-provider-broker-v1\n"
        )
        self.assertEqual(TAIRA_DROP_IN.read_text(encoding="utf-8"), expected)
        self.assertEqual(GOVERNANCE_DROP_IN.read_text(encoding="utf-8"), expected)

    def test_launchd_plist_has_fixed_public_only_arguments(self) -> None:
        payload = plistlib.loads(LAUNCHD_PLIST.read_bytes())
        self.assertEqual(
            payload["Label"],
            "org.hyperledger.iroha.runtime-provider-broker-v1",
        )
        self.assertEqual(payload["UserName"], "iroha")
        self.assertEqual(payload["GroupName"], "iroha")
        self.assertEqual(payload["Umask"], 0o77)
        self.assertEqual(
            payload["ProgramArguments"],
            [
                "/usr/local/libexec/iroha-runtime-provider-broker-v1",
                "--catalog",
                "/private/etc/iroha/runtime-provider-broker/catalog.norito",
            ],
        )
        self.assertEqual(payload["KeepAlive"], {"SuccessfulExit": False})
        self.assertTrue(payload["RunAtLoad"])
        self.assertEqual(payload["SoftResourceLimits"]["Core"], 0)
        self.assertEqual(payload["HardResourceLimits"]["Core"], 0)
        self.assertNotIn("EnvironmentVariables", payload)
        joined_arguments = " ".join(payload["ProgramArguments"])
        for forbidden in ("--socket", "--plugin", "--private-key", "--credential"):
            self.assertNotIn(forbidden, joined_arguments)

    def test_docs_do_not_claim_concrete_provider_backends(self) -> None:
        docs = (ASSET_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn(
            "it does not supply a concrete HSM, KMS,", docs
        )
        self.assertIn("deployment-owned", docs)
        self.assertIn(
            "Both checked-in Linux consumer dependencies are mandatory", docs
        )
        self.assertIn("externally verified signed release provenance", docs)
        self.assertIn("Windows has no V1 authenticated runtime-provider transport", docs)


if __name__ == "__main__":
    unittest.main()
