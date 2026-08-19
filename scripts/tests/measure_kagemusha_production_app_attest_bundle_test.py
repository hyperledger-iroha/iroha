"""Tests for production App Attest capture-app code-sign measurement."""

from __future__ import annotations

import json
from pathlib import Path
import plistlib
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


if __name__ == "__main__":
    unittest.main()
