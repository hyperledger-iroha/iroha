"""Exercise SwiftPM artifact selection without moving frameworks or clearing caches."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class SwiftSpmValidationTests(unittest.TestCase):
    """Test the wrapper's positive and mandatory negative invocation boundaries."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="swift-spm-wrapper-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "scripts").mkdir()
        self.script = self.root / "scripts/check_swift_spm_validation.sh"
        self.script.write_bytes((ROOT / "scripts/check_swift_spm_validation.sh").read_bytes())
        (self.root / "IrohaSwift").mkdir()
        self.report = self.root / "report"
        self.report.mkdir()
        for name in ("scratch_with_bridge", "scratch_missing_bridge"):
            cache = self.report / name
            cache.mkdir()
            (cache / "warm-cache").write_text("retained", encoding="utf-8")
        self.calls = self.root / "swift-calls.jsonl"
        binary = self.root / "bin"
        binary.mkdir()
        swift = binary / "swift"
        swift.write_text(
            f"#!{sys.executable}\n"
            "import json, os, pathlib, sys\n"
            "local = os.environ.get('MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR')\n"
            "external = os.environ.get('MOBILE_SDK_APPLE_ARTIFACT_DIR')\n"
            "selected = local or external or os.environ['SWIFT_TEST_DEFAULT_ARTIFACT']\n"
            "with open(os.environ['SWIFT_TEST_CALLS'], 'a') as out:\n"
            "    out.write(json.dumps({'local': local, 'external': external, 'args': sys.argv[1:]}) + '\\n')\n"
            "if (pathlib.Path(selected) / 'NoritoBridge.xcframework').is_dir():\n"
            "    print('Build complete!')\n"
            "elif os.environ.get('SWIFT_TEST_ACCEPT_MISSING') == '1':\n"
            "    print('Unexpected missing-artifact acceptance')\n"
            "else:\n"
            "    print('error: NoritoBridge.xcframework is required at ' + selected)\n"
            "    sys.exit(65)\n",
            encoding="utf-8",
        )
        swift.chmod(0o755)
        self.environment = os.environ.copy()
        for key in (
            "MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR", "MOBILE_SDK_APPLE_ARTIFACT_DIR",
            "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT", "SWIFT_SPM_SUMMARY",
            "SWIFT_SPM_WITH_BRIDGE_LOG", "SWIFT_SPM_MISSING_BRIDGE_LOG",
            "SWIFT_SPM_MODULE_CACHE", "SWIFT_SPM_WITH_BRIDGE_SCRATCH",
            "SWIFT_SPM_MISSING_BRIDGE_SCRATCH",
        ):
            self.environment.pop(key, None)
        self.environment.update({
            "PATH": str(binary) + os.pathsep + os.environ["PATH"],
            "SWIFT_SPM_REPORT_DIR": str(self.report),
            "SWIFT_TEST_CALLS": str(self.calls),
            "SWIFT_TEST_DEFAULT_ARTIFACT": str(self.root / "dist"),
        })

    def framework(self, parent: Path) -> Path:
        """Create a marker framework consumed only by the mocked Swift process."""
        path = parent / "NoritoBridge.xcframework"
        path.mkdir(parents=True)
        (path / "marker").write_text("original framework", encoding="utf-8")
        return path

    def run_wrapper(self) -> subprocess.CompletedProcess[str]:
        """Run the copied wrapper with the isolated fake Swift executable."""
        return subprocess.run(
            ["/bin/bash", str(self.script)], env=self.environment,
            text=True, capture_output=True, check=False,
        )

    def assert_retained(self, framework: Path) -> None:
        """Check that positive artifacts and both existing caches survive."""
        self.assertEqual((framework / "marker").read_text(), "original framework")
        for name in ("scratch_with_bridge", "scratch_missing_bridge"):
            self.assertEqual((self.report / name / "warm-cache").read_text(), "retained")
        calls = [json.loads(line) for line in self.calls.read_text().splitlines()]
        self.assertEqual(len(calls), 2)
        self.assertIsNone(calls[1]["local"])
        negative_root = Path(calls[1]["external"])
        self.assertNotEqual(negative_root, framework.parent)
        self.assertFalse(negative_root.exists(), "empty negative directory was not removed")
        for call in calls:
            self.assertIn("--disable-automatic-resolution", call["args"])

    def test_default_framework_and_warm_caches_are_retained(self) -> None:
        framework = self.framework(self.root / "dist")
        inode = framework.stat().st_ino
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(framework.stat().st_ino, inode)
        self.assert_retained(framework)
        self.assertEqual(json.loads((self.report / "summary.json").read_text())["status"], "passed")

    def test_external_selector_works_without_checkout_dist(self) -> None:
        parent = self.root / "external artifacts"
        framework = self.framework(parent)
        self.environment["MOBILE_SDK_APPLE_ARTIFACT_DIR"] = str(parent)
        self.environment["MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"] = "1"
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_retained(framework)
        self.assertFalse((self.root / "dist").exists())

    def test_local_unit_selector_is_unset_only_for_negative_case(self) -> None:
        parent = self.root / "local unit"
        framework = self.framework(parent)
        self.environment["MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR"] = str(parent)
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_retained(framework)
        first = json.loads(self.calls.read_text().splitlines()[0])
        self.assertEqual(first["local"], str(parent))

    def test_missing_selected_framework_refuses_before_swift(self) -> None:
        self.environment["MOBILE_SDK_APPLE_ARTIFACT_DIR"] = str(self.root / "absent")
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 65)
        self.assertFalse(self.calls.exists())
        self.assertEqual(json.loads((self.report / "summary.json").read_text())["status"], "failed")

    def test_negative_case_acceptance_fails_the_wrapper(self) -> None:
        framework = self.framework(self.root / "dist")
        self.environment["SWIFT_TEST_ACCEPT_MISSING"] = "1"
        result = self.run_wrapper()
        self.assertEqual(result.returncode, 1)
        self.assert_retained(framework)
        summary = json.loads((self.report / "summary.json").read_text())
        self.assertEqual(summary["status"], "failed")
        self.assertFalse(summary["missing_bridge"]["required_error_present"])


if __name__ == "__main__":
    unittest.main()
