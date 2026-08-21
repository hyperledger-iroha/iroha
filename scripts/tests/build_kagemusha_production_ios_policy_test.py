"""Tests for canonical production App Attest policy construction."""

from __future__ import annotations

from contextlib import redirect_stderr
import io
import json
from pathlib import Path
import tempfile
import unittest

from scripts import build_kagemusha_production_ios_policy as builder
from scripts import kagemusha_candidate_ios_evidence as candidate_evidence
from scripts import kagemusha_production_ios_evidence as production_evidence


class ProductionIosPolicyBuilderTest(unittest.TestCase):
    def _argv(self, output: Path) -> list[str]:
        return [
            "--policy-id",
            "taira-kagemusha-production-ios-v1",
            "--app-id-prefix",
            "A1B2C3D4E5",
            "--bundle-version",
            "1",
            "--validation-category",
            "4",
            "--output",
            str(output),
        ]

    def test_builds_valid_policy_with_the_pinned_real_apple_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "production-policy-v1.json"
            self.assertEqual(builder.main(self._argv(output)), 0)
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            payload = output.read_bytes()
            value = candidate_evidence.parse_strict_json(payload, "built policy")
            errors: list[str] = []
            self.assertTrue(
                production_evidence._validate_policy(value, payload, errors)
            )
            self.assertEqual(errors, [])
            self.assertEqual(value["bundle_id"], builder.PRODUCTION_BUNDLE_ID)
            self.assertEqual(value["allowed_validation_categories"], [4])
            self.assertEqual(value["allowed_bundle_versions"], ["1"])

    def test_unsupported_category_fails_without_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "production-policy-v1.json"
            argv = self._argv(output)
            argv[argv.index("--validation-category") + 1] = "9"
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = builder.main(argv)
            self.assertEqual(result, 1)
            self.assertIn("allowed_validation_categories", stderr.getvalue())
            self.assertFalse(output.exists())

    def test_existing_policy_is_never_replaced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "production-policy-v1.json"
            output.write_text(json.dumps({"operator": "owned"}) + "\n")
            output.chmod(0o600)
            before = output.read_bytes()
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = builder.main(self._argv(output))
            self.assertEqual(result, 1)
            self.assertIn("already exists", stderr.getvalue())
            self.assertEqual(output.read_bytes(), before)

    def test_writable_root_input_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            certificate = root / "root.der"
            certificate.write_bytes(builder.DEFAULT_APPLE_ROOT.read_bytes())
            certificate.chmod(0o622)
            output = root / "production-policy-v1.json"
            argv = self._argv(output)
            argv[0:0] = ["--trusted-root-der", str(certificate)]
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = builder.main(argv)
            self.assertEqual(result, 1)
            self.assertIn("non-writable", stderr.getvalue())
            self.assertFalse(output.exists())

    def test_duplicate_operator_choices_are_rejected_not_normalized(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "production-policy-v1.json"
            argv = self._argv(output)
            argv.extend(["--bundle-version", "1"])
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = builder.main(argv)
            self.assertEqual(result, 1)
            self.assertIn("must not contain duplicates", stderr.getvalue())
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
