"""Host-only checks for collecting an Android diagnostic fixture result."""
from pathlib import Path
import hashlib
import runpy
import tempfile
import unittest
from zipfile import ZipFile

reader = runpy.run_path(str(Path(__file__).with_name("read-fixture-result.py")))
PACKAGE = "org.hyperledger.iroha.kagemusha.enrollmentprobe.fixture"


class CollectResultTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.apks = []
        for version in (3, 4):
            apk = Path(self.scratch.name) / f"v{version}.apk"
            with ZipFile(apk, "w") as archive:
                archive.writestr("assets/spartan-artifacts/parameter-manifest.norito", bytes([version]))
            self.apks.append(apk)
        self.paths = reader["selected_paths"](self.apks[1])
        self.report = b'PASS: fixture result\n{"measured":true}'
        self.metrics = b'{"test":4}'
        self.fields = {
            **self.paths,
            "spartanRuntime": self.report.decode(),
            "spartanReportSha256": hashlib.sha256(self.report).hexdigest(),
            "spartanMeasurementsSha256": hashlib.sha256(self.metrics).hexdigest(),
        }
        self.files = {
            self.paths["spartanReportPath"]: self.report,
            self.paths["spartanCustodyPath"] + "/device-measurements.json": self.metrics,
        }

    def collect(self, apk, field, read):
        log = "\n".join(f"INSTRUMENTATION_STATUS: {key}={value}" for key, value in self.fields.items())
        return reader["collect"](apk, log, PACKAGE, "fixture-serial", field, read)

    def read(self, command):
        self.assertEqual(command[:-1], ["adb", "-s", "fixture-serial", "exec-out", "run-as", PACKAGE, "cat"])
        return self.files[command[-1]]

    def test_manifest_scopes_custody_and_report(self):
        older = reader["selected_paths"](self.apks[0])
        self.assertNotEqual(older["spartanCustodyPath"], self.paths["spartanCustodyPath"])
        self.assertNotEqual(older["spartanReportPath"], self.paths["spartanReportPath"])
        self.assertEqual(self.collect(self.apks[1], "report", self.read), self.report)
        self.assertEqual(self.collect(self.apks[1], "measurements", self.read), self.metrics)
        with self.assertRaisesRegex(ValueError, "selected APK"):
            self.collect(self.apks[0], "report", lambda _: self.fail("unexpected device read"))

    def test_pre_protected_result_for_the_same_manifest_is_refused(self):
        self.fields["spartanCustodyPath"] = self.paths["spartanCustodyPath"].removesuffix("/protected-debit-v1")
        with self.assertRaisesRegex(ValueError, "selected APK"):
            self.collect(self.apks[1], "report", lambda _: self.fail("unexpected device read"))

    def test_later_launch_cannot_replace_selected_report(self):
        self.files[self.paths["spartanReportPath"]] = self.report + b"modified"
        with self.assertRaisesRegex(ValueError, "retained report differs"):
            self.collect(self.apks[1], "report", self.read)

    def test_later_launch_cannot_replace_selected_measurements(self):
        self.files[self.paths["spartanCustodyPath"] + "/device-measurements.json"] = b'{"test":5}'
        with self.assertRaisesRegex(ValueError, "measurements differ"):
            self.collect(self.apks[1], "measurements", self.read)

    def test_legacy_fixed_path_is_refused_before_reading(self):
        self.fields["spartanReportPath"] = "files/spartan-native-runtime-report.txt"
        with self.assertRaisesRegex(ValueError, "selected APK"):
            self.collect(self.apks[1], "report", lambda _: self.fail("unexpected device read"))


if __name__ == "__main__":
    unittest.main()
