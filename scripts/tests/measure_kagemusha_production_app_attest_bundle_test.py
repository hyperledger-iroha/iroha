"""Tests for production App Attest capture-app code-sign measurement."""

from __future__ import annotations

import json
import os
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest
from unittest import mock

from scripts import measure_kagemusha_production_app_attest_bundle as measure


class CaptureBundleMeasurementTest(unittest.TestCase):
    """Exercise strict inputs and canonical output without Apple credentials."""

    def test_private_regular_snapshot_is_exact_and_rejects_writable_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "input"
            path.write_bytes(b"bounded")
            path.chmod(0o600)
            self.assertEqual(
                measure._regular_bytes(path, "fixture", 7), b"bounded"
            )
            path.chmod(0o620)
            with self.assertRaisesRegex(
                measure.MeasurementError, "bounded, non-writable"
            ):
                measure._regular_bytes(path, "fixture", 7)

    def test_codesign_diagnostics_require_each_exact_identity(self) -> None:
        completed = mock.Mock(
            returncode=0,
            stderr=(
                b"Identifier=org.example.capture\n"
                b"TeamIdentifier=ABCDEFGHIJ\n"
                b"CDHash=0123456789abcdef0123456789abcdef01234567\n"
            ),
        )
        with mock.patch.object(measure.subprocess, "run", return_value=completed):
            self.assertEqual(
                measure._codesign_details(Path("/private/app"))["TeamIdentifier"],
                "ABCDEFGHIJ",
            )
        completed.stderr += b"CDHash=0123456789abcdef0123456789abcdef01234567\n"
        with mock.patch.object(measure.subprocess, "run", return_value=completed):
            with self.assertRaisesRegex(measure.MeasurementError, "repeated CDHash"):
                measure._codesign_details(Path("/private/app"))

    def test_measurement_binds_exact_signed_identity_and_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / "Capture.app"
            app.mkdir(mode=0o700)
            executable = app / "Capture"
            executable.write_bytes(b"reviewed-capture-executable")
            executable.chmod(0o700)
            (app / "Info.plist").write_bytes(
                plistlib.dumps(
                    {
                        "CFBundleExecutable": "Capture",
                        "CFBundleIdentifier": "org.example.capture",
                        "CFBundleVersion": "1",
                    }
                )
            )
            (app / "Info.plist").chmod(0o600)
            entitlements = root / "entitlements.plist"
            entitlements.write_bytes(
                plistlib.dumps(
                    {
                        "application-identifier": "ABCDEFGHIJ.org.example.capture",
                        "com.apple.developer.team-identifier": "ABCDEFGHIJ",
                        "com.apple.developer.devicecheck.appattest-environment": "production",
                    }
                )
            )
            entitlements.chmod(0o600)
            details = {
                "Identifier": "org.example.capture",
                "TeamIdentifier": "ABCDEFGHIJ",
                "CDHash": "0123456789abcdef0123456789abcdef01234567",
            }
            with mock.patch.object(measure, "_codesign_details", return_value=details):
                observed = measure.measure_bundle(
                    app, entitlements, "ABCDEFGHIJ", "org.example.capture"
                )
            self.assertEqual(observed["schema"], measure.SCHEMA)
            self.assertEqual(observed["bundle_version"], "1")
            self.assertEqual(observed["application_identifier"], "ABCDEFGHIJ.org.example.capture")
            self.assertRegex(str(observed["executable_sha256"]), r"^[0-9a-f]{64}$")

    def test_measurement_rejects_inputs_changed_during_codesign(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / "Capture.app"
            app.mkdir(mode=0o700)
            executable = app / "Capture"
            executable.write_bytes(b"reviewed-capture-executable")
            executable.chmod(0o700)
            (app / "Info.plist").write_bytes(
                plistlib.dumps(
                    {
                        "CFBundleExecutable": "Capture",
                        "CFBundleIdentifier": "org.example.capture",
                        "CFBundleVersion": "1",
                    }
                )
            )
            (app / "Info.plist").chmod(0o600)
            entitlements = root / "entitlements.plist"
            entitlements.write_bytes(
                plistlib.dumps(
                    {
                        "application-identifier": "ABCDEFGHIJ.org.example.capture",
                        "com.apple.developer.team-identifier": "ABCDEFGHIJ",
                        "com.apple.developer.devicecheck.appattest-environment": "production",
                    }
                )
            )
            entitlements.chmod(0o600)

            def codesign_details(_app: Path) -> dict[str, str]:
                entitlements.write_bytes(
                    plistlib.dumps(
                        {
                            "application-identifier": "ABCDEFGHIJ.org.example.capture",
                            "com.apple.developer.team-identifier": "ABCDEFGHIJ",
                            "com.apple.developer.devicecheck.appattest-environment": "development",
                        }
                    )
                )
                return {
                    "Identifier": "org.example.capture",
                    "TeamIdentifier": "ABCDEFGHIJ",
                    "CDHash": "0123456789abcdef0123456789abcdef01234567",
                }

            with (
                mock.patch.object(
                    measure, "_codesign_details", side_effect=codesign_details
                ),
                self.assertRaisesRegex(
                    measure.MeasurementError, "inputs changed during measurement"
                ),
            ):
                measure.measure_bundle(
                    app, entitlements, "ABCDEFGHIJ", "org.example.capture"
                )

    def test_measurement_rejects_app_swap_during_final_rereads(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / "Capture.app"
            replacement = root / "Replacement.app"
            info_payload = plistlib.dumps(
                {
                    "CFBundleExecutable": "Capture",
                    "CFBundleIdentifier": "org.example.capture",
                    "CFBundleVersion": "1",
                }
            )
            executable_payload = b"reviewed-capture-executable"
            for bundle in (app, replacement):
                bundle.mkdir(mode=0o700)
                (bundle / "Info.plist").write_bytes(info_payload)
                (bundle / "Info.plist").chmod(0o600)
                (bundle / "Capture").write_bytes(executable_payload)
                (bundle / "Capture").chmod(0o700)
            entitlements = root / "entitlements.plist"
            entitlements.write_bytes(
                plistlib.dumps(
                    {
                        "application-identifier": "ABCDEFGHIJ.org.example.capture",
                        "com.apple.developer.team-identifier": "ABCDEFGHIJ",
                        "com.apple.developer.devicecheck.appattest-environment": "production",
                    }
                )
            )
            entitlements.chmod(0o600)
            details = {
                "Identifier": "org.example.capture",
                "TeamIdentifier": "ABCDEFGHIJ",
                "CDHash": "0123456789abcdef0123456789abcdef01234567",
            }
            regular_bytes = measure._regular_bytes
            read_count = 0

            def swap_on_last_reread(path: Path, label: str, maximum: int) -> bytes:
                nonlocal read_count
                read_count += 1
                if read_count == 6:
                    app.rename(root / "Original.app")
                    replacement.rename(app)
                return regular_bytes(path, label, maximum)

            with (
                mock.patch.object(measure, "_codesign_details", return_value=details),
                mock.patch.object(
                    measure, "_regular_bytes", side_effect=swap_on_last_reread
                ),
                self.assertRaisesRegex(
                    measure.MeasurementError, "capture app changed during measurement"
                ),
            ):
                measure.measure_bundle(
                    app, entitlements, "ABCDEFGHIJ", "org.example.capture"
                )
            self.assertEqual(read_count, 6)

    def test_private_output_is_create_new_and_canonical(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "measurement.json"
            value = {"schema": measure.SCHEMA, "version": 1}
            measure._write_new_private_json(output, value)
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            self.assertEqual(
                output.read_bytes(),
                json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
                + b"\n",
            )
            with self.assertRaises(FileExistsError):
                measure._write_new_private_json(output, value)

    def test_private_output_zero_length_write_fails_without_partial_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "measurement.json"
            writer = mock.Mock(side_effect=[0, OSError("write loop continued")])
            with (
                mock.patch.object(measure.os, "write", writer),
                self.assertRaisesRegex(OSError, "short capture-app measurement write"),
            ):
                measure._write_new_private_json(
                    output,
                    {"schema": measure.SCHEMA, "version": 1},
                )
            self.assertEqual(writer.call_count, 1)
            self.assertFalse(output.exists())

    def test_private_output_close_failure_cleans_staging_file(self) -> None:
        value = {"schema": measure.SCHEMA, "version": 1}
        real_close = os.close
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "measurement.json"

            def close_then_fail(descriptor: int) -> None:
                real_close(descriptor)
                raise OSError("injected close failure")

            with (
                mock.patch.object(measure.os, "close", side_effect=close_then_fail),
                self.assertRaisesRegex(OSError, "injected close failure"),
            ):
                measure._write_new_private_json(output, value)
            self.assertFalse(output.exists())
            self.assertEqual(list(root.iterdir()), [])

    def test_private_output_post_link_replacement_is_preserved_as_uncertain(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "measurement.json"
            displaced = root / "displaced-created-file.json"
            raced_payload = b"raced replacement\n"
            real_link = os.link

            def replace_after_link(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                target: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *,
                follow_symlinks: bool = True,
            ) -> None:
                real_link(source, target, follow_symlinks=follow_symlinks)
                Path(target).rename(displaced)
                Path(target).write_bytes(raced_payload)
                Path(target).chmod(0o600)

            with (
                mock.patch.object(measure.os, "link", side_effect=replace_after_link),
                self.assertRaisesRegex(
                    measure.MeasurementPublicationUncertain,
                    "no final name was removed",
                ),
            ):
                measure._write_new_private_json(
                    output, {"schema": measure.SCHEMA, "version": 1}
                )
            self.assertEqual(output.read_bytes(), raced_payload)
            self.assertTrue(displaced.is_file())

    def test_private_output_same_inode_same_length_mutation_is_uncertain(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "measurement.json"
            value = {"kind": "original"}
            mutated_payload = json.dumps(
                {"kind": "tampered"},
                sort_keys=True,
                separators=(",", ":"),
            ).encode("ascii") + b"\n"
            expected_payload = json.dumps(
                value,
                sort_keys=True,
                separators=(",", ":"),
            ).encode("ascii") + b"\n"
            self.assertEqual(len(mutated_payload), len(expected_payload))
            real_link = os.link

            def mutate_after_link(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                target: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *,
                follow_symlinks: bool = True,
            ) -> None:
                real_link(source, target, follow_symlinks=follow_symlinks)
                Path(target).write_bytes(mutated_payload)
                Path(target).chmod(0o600)

            with (
                mock.patch.object(measure.os, "link", side_effect=mutate_after_link),
                self.assertRaisesRegex(
                    measure.MeasurementPublicationUncertain,
                    "no final name was removed",
                ),
            ):
                measure._write_new_private_json(output, value)
            self.assertEqual(output.read_bytes(), mutated_payload)

    def test_main_returns_temporary_failure_for_uncertain_publication(self) -> None:
        argv = [
            "measure_kagemusha_production_app_attest_bundle.py",
            "--app",
            "/unused/app",
            "--signed-entitlements",
            "/unused/entitlements.plist",
            "--development-team",
            "ABCDEFGHIJ",
            "--bundle-id",
            "org.example.capture",
            "--output",
            "/unused/output.json",
        ]
        with (
            mock.patch.object(sys, "argv", argv),
            mock.patch.object(measure, "measure_bundle", return_value={}),
            mock.patch.object(
                measure,
                "_write_new_private_json",
                side_effect=measure.MeasurementPublicationUncertain("uncertain"),
            ),
        ):
            self.assertEqual(measure.main(), 75)


if __name__ == "__main__":
    unittest.main()
