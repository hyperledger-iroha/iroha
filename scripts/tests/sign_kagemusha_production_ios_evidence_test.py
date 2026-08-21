"""Tests for fail-closed publication of production App Attest envelopes."""

from __future__ import annotations

import base64
from contextlib import redirect_stderr
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_DIRECTORY = ROOT / "scripts"
TEST_DIRECTORY = SCRIPT_DIRECTORY / "tests"
for directory in (SCRIPT_DIRECTORY, TEST_DIRECTORY):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

import check_kagemusha_candidate_ios_evidence_test as fixtures  # noqa: E402
import kagemusha_candidate_ios_evidence as candidate_evidence  # noqa: E402
import sign_kagemusha_production_ios_evidence as signer  # noqa: E402


def _write_key_pair(root: Path, label: str) -> tuple[Path, Path]:
    seed = hashlib.sha256(label.encode("ascii")).digest()
    public = candidate_evidence._ed25519_public_from_seed(seed)
    private_path = root / f"{label}.private.pem"
    public_path = root / f"{label}.public.pem"
    fixtures.write_private(
        private_path,
        b"-----BEGIN PRIVATE KEY-----\n"
        + base64.b64encode(candidate_evidence.ED25519_PKCS8_SEED_PREFIX + seed)
        + b"\n-----END PRIVATE KEY-----\n",
    )
    fixtures.write_private(
        public_path,
        b"-----BEGIN PUBLIC KEY-----\n"
        + base64.b64encode(candidate_evidence.ED25519_SPKI_PREFIX + public)
        + b"\n-----END PUBLIC KEY-----\n",
    )
    return private_path, public_path


class ProductionEnvelopeSignerTest(unittest.TestCase):
    """Exercise the exact production builder against the cryptographic fixture."""

    def _fixture(
        self, temporary: str
    ) -> tuple[fixtures.ProductionFixture, Path, bytes]:
        root = Path(temporary)
        key_root = root / "keys"
        key_root.mkdir(mode=0o700)
        lab_private, lab_public = _write_key_pair(key_root, "lab-signer")
        freshness_private, freshness_public = _write_key_pair(
            key_root, "freshness-signer"
        )
        fixture_root = root / "fixture"
        fixture_root.mkdir(mode=0o700)
        production = fixtures.ProductionFixture(
            fixtures.Fixture(fixture_root, lab_private, lab_public),
            freshness_private,
            freshness_public,
        )
        expected = production.evidence.read_bytes()
        platform = json.loads(expected.decode("ascii"))["platform_evidence"]
        platform_path = fixture_root / "signed/platform-evidence-v1.json"
        fixtures.write_json(platform_path, platform)
        production.evidence.unlink()
        return production, platform_path, expected

    @staticmethod
    def _argv(
        fixture: fixtures.ProductionFixture,
        platform: Path,
    ) -> list[str]:
        return [
            "--artifact-root",
            str(fixture.raw),
            "--platform-evidence",
            str(platform),
            "--production-policy",
            str(fixture.policy),
            "--release-manifest-sha256",
            fixture.release_manifest_sha256,
            "--private-key",
            str(fixture.private_key),
            "--public-key",
            str(fixture.public_key),
            "--signer-key-id",
            fixture.key_id,
            "--output",
            str(fixture.evidence.resolve()),
        ]

    def test_builds_exact_envelope_consumed_by_online_authority_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture, platform, expected = self._fixture(temporary)
            self.assertEqual(signer.main(self._argv(fixture, platform)), 0)
            self.assertEqual(fixture.evidence.read_bytes(), expected)
            self.assertEqual(fixture.evidence.stat().st_mode & 0o777, 0o600)
            self.assertEqual(fixture.errors(), [])

    def test_invalid_platform_is_rejected_without_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture, platform, _ = self._fixture(temporary)
            value = json.loads(platform.read_text(encoding="ascii"))
            value["capture_app_code_sign_measurements"]["cdhash"] = "0" * 40
            fixtures.write_json(platform, value)
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = signer.main(self._argv(fixture, platform))
            self.assertEqual(result, 1)
            self.assertIn("cdhash must be nonzero", stderr.getvalue())
            self.assertFalse(fixture.evidence.exists())

    def test_existing_output_is_never_replaced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture, platform, _ = self._fixture(temporary)
            fixtures.write_private(fixture.evidence, b"operator-owned\n")
            before = fixture.evidence.read_bytes()
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = signer.main(self._argv(fixture, platform))
            self.assertEqual(result, 1)
            self.assertIn("already exists", stderr.getvalue())
            self.assertEqual(fixture.evidence.read_bytes(), before)

    def test_zero_release_digest_is_rejected_without_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture, platform, _ = self._fixture(temporary)
            argv = self._argv(fixture, platform)
            index = argv.index("--release-manifest-sha256") + 1
            argv[index] = "0" * 64
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                result = signer.main(argv)
            self.assertEqual(result, 1)
            self.assertIn("nonzero lowercase SHA-256", stderr.getvalue())
            self.assertFalse(fixture.evidence.exists())


if __name__ == "__main__":
    unittest.main()
