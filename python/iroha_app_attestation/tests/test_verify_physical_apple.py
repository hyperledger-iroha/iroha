"""Cross-language physical App Attest release-frame checks."""

import base64
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import AttestationRejected, RawPlatformProof
from iroha_app_attestation.verify_physical_apple import SELECTION_DOMAIN, _release_digest, _selection_identity, verify


class PhysicalAppleEvidenceTests(unittest.TestCase):
    def test_unverified_fraud_receipt_blocks_physical_acceptance(self) -> None:
        encoded = lambda value: base64.b64encode(value).decode("ascii")
        app_id = "TEAMID.example.app"
        app_hash = hashlib.sha256(app_id.encode("utf-8")).digest()
        key = b"\x66" * 32
        key_text = encoded(key)
        root = (Path(__file__).parent / "fixtures" / "apple_app_attestation_root.der").read_bytes()
        receipt_root = (Path(__file__).parent / "fixtures" / "apple_root_ca_g3.der").read_bytes()
        enrollment = {
            "keyID": key_text,
            "rawAttestationBase64": encoded(b"raw attestation checked by the stub"),
            "clientNonceBase64": encoded(b"\x11" * 32),
            "serverNonceBase64": encoded(b"\x22" * 32),
            "releaseIDBase64": encoded(b"\x33" * 32),
            "profileIDBase64": encoded(b"\x44" * 32),
            "attestedKeyIDBase64": key_text,
            "laneIDBase64": encoded(b"\x55" * 32),
            "expectedAppIDHashBase64": encoded(app_hash),
            "expectedReleaseDigestBase64": encoded(_release_digest("1")),
            "bundleVersion": "1",
        }
        assertions = {
            "keyID": key_text,
            "enrolledPublicKeyX963Base64": encoded(b"\x04" + b"\x77" * 64),
            "expectedAppIDHashBase64": encoded(app_hash),
            "firstSelectionBase64": encoded(b"selection"),
            "firstAssertionBase64": encoded(b"assertion"),
            "secondSelectionBase64": encoded(b"selection"),
            "secondAssertionBase64": encoded(b"assertion"),
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            enrollment_path = directory / "enrollment.json"
            assertions_path = directory / "assertions.json"
            root_path = directory / "root.der"
            receipt_root_path = directory / "receipt-root.der"
            enrollment_path.write_text(json.dumps(enrollment), encoding="utf-8")
            assertions_path.write_text(json.dumps(assertions), encoding="utf-8")
            root_path.write_bytes(root)
            receipt_root_path.write_bytes(receipt_root)
            proof = RawPlatformProof(b"\x01" * 32, b"\x04" + b"\x77" * 64,
                                     b"\x02" * 32, "apple_app_attest")
            with (patch("iroha_app_attestation.verify_physical_apple.verify_apple_raw", return_value=proof),
                  patch("iroha_app_attestation.verify_physical_apple.cbor_exact", return_value={
                      "attStmt": {"receipt": b"untrusted", "x5c": [b"credential"]}}),
                  patch("iroha_app_attestation.verify_physical_apple.verify_apple_receipt",
                        side_effect=AttestationRejected("invalid receipt")) as receipt,
                  patch("iroha_app_attestation.verify_physical_apple.verify_apple_assertion") as assertion):
                with self.assertRaisesRegex(AttestationRejected, "invalid receipt"):
                    verify(enrollment_path, assertions_path, root_path, receipt_root_path, app_id,
                           "development", "1", Path("/usr/bin/openssl"))
                self.assertEqual(receipt.call_count, 1)
                assertion.assert_not_called()

    def test_development_release_digest_and_invalid_versions(self) -> None:
        # V1 category 3 (development-signed app), bundle version "1".
        self.assertEqual(
            _release_digest("1").hex(),
            "e9078e09d73f3cd4d15efd91ea3a6e4d7eddbeb5be01a2003a157bc5582a548d",
        )
        for invalid in ("", "x" * 129, "1\0suffix", "\ud800"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    _release_digest(invalid)

    def test_selection_is_bound_to_enrollment_and_exact_next_counter(self) -> None:
        release, lane, profile = bytes([0x33]) * 32, bytes([0x55]) * 32, bytes([0x44]) * 32
        subject = bytearray(
            SELECTION_DOMAIN + (403).to_bytes(8, "little") + (1).to_bytes(2, "little")
            + release + bytes([0x21]) * 32 + bytes([0x22]) * 32
            + bytes([0x23]) * 32 + bytes([0x24]) * 32
            + lane + profile + (1).to_bytes(8, "little")
            + bytes([0x25]) * 32 + (1).to_bytes(8, "little")
            + b"\x01" + bytes([0x61]) * 32 + bytes(64)
            + (0).to_bytes(16, "little") + (1).to_bytes(16, "little")
        )
        self.assertEqual(len(subject), 460)
        self.assertEqual(_selection_identity(bytes(subject), release, profile, lane, 0),
                         bytes(subject[59:331]))
        for offset in (0, 49, 57, 59, 91, 123, 155, 187, 219, 251, 283,
                       291, 323, 331, 332, 364, 396, 428, 444):
            with self.subTest(offset=offset):
                changed = bytearray(subject)
                changed[offset] ^= 1
                with self.assertRaises(ValueError):
                    _selection_identity(bytes(changed), release, profile, lane, 0)


if __name__ == "__main__":
    unittest.main()
