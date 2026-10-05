"""Adversarial parsing and fail-closed checks for independent raw evidence."""

import unittest
import hashlib
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

from iroha_app_attestation.attestation import (
    AndroidPatchLevels,
    AttestationRejected,
    GOOGLE_FACTORY_2016_ROOT_SHA256,
    GOOGLE_FACTORY_2016_VERIFICATION_TIME_MS,
    Selection,
    _certificate_valid_at,
    certificate_key_extensions,
    certificate_spki_extensions,
    cbor_exact,
    der_one,
    decode_android_chain,
    encode_android_chain,
    explicit_tags,
    oid,
    android_patch_policy_met,
    patch_level_yyyymm,
    validate_android_patch_floor,
    verify_apple_raw,
    verify_android_raw,
    verify_pinned_chain,
)


def selection() -> Selection:
    return Selection(*[bytes([index]) * 32 for index in range(1, 7)])


class AttestationTests(unittest.TestCase):
    def test_challenge_binds_every_independently_selected_field(self) -> None:
        original = selection()
        transcript = original.transcript()
        self.assertTrue(transcript.startswith(b"iroha:kagemusha:v1:app-device-attestation-challenge\0"))
        for field in original.__dataclass_fields__:
            changed = dict(vars(original))
            changed[field] = bytes([changed[field][0] ^ 1]) + changed[field][1:]
            self.assertNotEqual(Selection(**changed).transcript(), transcript, field)
        with self.assertRaises(AttestationRejected):
            Selection(original.client_nonce, original.client_nonce, *list(vars(original).values())[2:]).transcript()
        android = Selection(b"\x01" * 32, b"\x02" * 32, b"\x03" * 32,
                            b"\x04" * 32, b"\0" * 32, b"\x05" * 32)
        self.assertEqual(hashlib.sha256(android.transcript()).hexdigest(),
                         "962093f79f952f77b79544f1c16f9d5dd176c6a12997959336be13bb799d12e8")

    def test_cbor_rejects_duplicate_fields_trailing_bytes_and_indefinite_lengths(self) -> None:
        self.assertEqual(cbor_exact(b"\xa1\x61a\x01"), {"a": 1})
        for raw in (b"\xa2\x61a\x01\x61a\x02", b"\xa1\x61a\x01\x00", b"\xbf\xff", b"\xa1\x61a"):
            with self.subTest(raw=raw), self.assertRaises(AttestationRejected):
                cbor_exact(raw)

    def test_der_rejects_trailing_and_nonminimal_lengths(self) -> None:
        self.assertEqual(der_one(b"\x04\x01x").value, b"x")
        for raw in (b"\x04\x01xx", b"\x04\x81\x01x", b"\x04\x82\x00\x01x", b"\x04\x02x"):
            with self.subTest(raw=raw), self.assertRaises(AttestationRejected):
                der_one(raw)
        self.assertEqual(oid(der_one(b"\x06\x03\x2a\x03\x04")), "1.2.3.4")
        for raw in (b"\x06\x02\x80\x2a", b"\x06\x03\x2a\x80\x03"):
            with self.subTest(raw=raw), self.assertRaises(AttestationRejected):
                oid(der_one(raw))

    def test_certificate_parser_rejects_ambiguous_extension_blocks_and_types(self) -> None:
        def tlv(tag: int, content: bytes) -> bytes:
            length = (bytes([len(content)]) if len(content) < 128 else
                      b"\x81" + bytes([len(content)]))
            return bytes([tag]) + length + content

        version = tlv(0xa0, b"\x02\x01\x02")
        mandatory = b"\x02\x01\x01" + b"\x30\x00" * 5
        oid_one, oid_two = b"\x06\x03\x2a\x03\x04", b"\x06\x03\x2a\x03\x05"

        def block(name: bytes, critical: bytes = b"") -> bytes:
            extension = tlv(0x30, name + critical + b"\x04\x01x")
            return tlv(0xa3, tlv(0x30, extension))

        def certificate(optionals: bytes) -> bytes:
            return tlv(0x30, tlv(0x30, version + mandatory + optionals)
                       + b"\x30\x00\x03\x01\x00")

        _, extensions = certificate_spki_extensions(certificate(block(oid_one)))
        self.assertEqual(extensions, {"1.2.3.4": b"x"})
        for malformed in (
            block(oid_one) + block(oid_two),
            block(oid_one) + b"\x81\x02\x00\x01",
            block(oid_one, b"\x02\x01\x01"),
            block(oid_one, b"\x01\x01\x00"),
        ):
            with self.subTest(malformed=malformed), self.assertRaises(AttestationRejected):
                certificate_spki_extensions(certificate(malformed))

    def test_keymint_accepts_documented_optional_tags_but_rejects_ambiguous_der(self) -> None:
        def wrap(tag: bytes, content: bytes) -> bytes:
            assert len(content) < 128
            return tag + bytes([len(content)]) + content

        def context(number: int, item: bytes) -> bytes:
            chunks = [number & 127]
            number >>= 7
            while number:
                chunks.insert(0, 128 | (number & 127))
                number >>= 7
            return wrap(b"\xbf" + bytes(chunks), item)

        integer = b"\x02\x04\x0c\x06\x7f\x01"
        vendor_patch = context(718, integer)
        brand = context(710, b"\x04\x05Pixel")
        early_boot = context(305, b"\x05\x00")
        valid = wrap(b"\x30", early_boot + brand + vendor_patch)
        self.assertEqual(set(explicit_tags(der_one(valid))), {305, 710, 718})
        for body in (
            vendor_patch + brand,  # unsorted
            brand + brand,  # duplicate
            context(725, b"\x05\x00"),  # unknown future semantics
            context(718, b"\x04\x01x"),  # wrong ASN.1 type
            context(305, b"\x05\x01\x00"),  # nonempty NULL
        ):
            with self.subTest(body=body), self.assertRaises(AttestationRejected):
                explicit_tags(der_one(wrap(b"\x30", body)))

    def test_android_chain_envelope_is_single_versioned_bounded_der_layout(self) -> None:
        chain = [b"\x30\x00", b"\x30\x03\x02\x01\x01"]
        encoded = encode_android_chain(chain)
        self.assertEqual(encoded, b"KMCA\x01\x02\0\0\0\x02\x30\0"
                         b"\0\0\0\x05\x30\x03\x02\x01\x01")
        self.assertEqual(decode_android_chain(encoded), chain)
        for malformed in (
            encoded[:-1], encoded + b"x", b"KMCA\x02" + encoded[5:],
            encoded[:5] + b"\x01" + encoded[6:],
            encoded[:6] + b"\0\0\0\x03" + encoded[10:],
        ):
            with self.subTest(malformed=malformed), self.assertRaises(AttestationRejected):
                decode_android_chain(malformed)

    def test_fake_apple_and_android_evidence_cannot_be_promoted(self) -> None:
        selected = selection()
        with self.assertRaises(AttestationRejected):
            verify_apple_raw(
                b"\xa1\x63fmt\x6fapple-appattest", b"\x01" * 32,
                "TEAMID.example.app", "production", selected,
                b"fake root", b"\x02" * 32, 1_000, Path("/usr/bin/openssl"),
                expected_validation_category=None, expected_bundle_version=None,
            )
        with self.assertRaises(AttestationRejected):
            verify_android_raw(
                [b"fake leaf", b"fake intermediate"], selected,
                "org.example.app", 1, b"\x03" * 32,
                b"fake root", b"\x02" * 32, 1_000, Path("/usr/bin/openssl"),
                allowed_security_levels=frozenset({2}),
            )

    def test_pinned_chain_checks_real_signatures_and_root_digest(self) -> None:
        executable = shutil.which("openssl")
        if executable is None:
            self.skipTest("OpenSSL CLI unavailable")
        openssl = Path(executable).resolve()
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            def run(*args: str) -> None:
                subprocess.run([str(openssl), *args], cwd=directory, capture_output=True, check=True)

            run("req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                "-nodes", "-keyout", "root.key", "-out", "root.pem", "-days", "2",
                "-subj", "/CN=Test Root", "-addext", "basicConstraints=critical,CA:TRUE",
                "-addext", "keyUsage=critical,keyCertSign,cRLSign")
            run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                "-nodes", "-keyout", "leaf.key", "-out", "leaf.csr", "-subj", "/CN=Test Leaf")
            run("x509", "-req", "-in", "leaf.csr", "-CA", "root.pem", "-CAkey", "root.key",
                "-CAcreateserial", "-out", "leaf.pem", "-days", "2")
            run("x509", "-in", "root.pem", "-outform", "DER", "-out", "root.der")
            run("x509", "-in", "leaf.pem", "-outform", "DER", "-out", "leaf.der")
            root, leaf = (directory / "root.der").read_bytes(), (directory / "leaf.der").read_bytes()
            point, _ = certificate_key_extensions(leaf)
            self.assertEqual(len(point), 65)
            current = int(time.time() * 1000)
            self.assertTrue(_certificate_valid_at(leaf, current))
            self.assertFalse(_certificate_valid_at(leaf, current + 4 * 86_400_000))
            verify_pinned_chain([leaf, root], root, hashlib.sha256(root).digest(), current, openssl)
            with self.assertRaises(AttestationRejected):
                verify_pinned_chain([leaf, root], root, b"\x01" * 32, current, openssl)
            with self.assertRaisesRegex(AttestationRejected, "exact 2016 root"):
                verify_pinned_chain(
                    [leaf, root], root, hashlib.sha256(root).digest(), current, openssl,
                    allow_google_factory_expired_root=True,
                )

    def test_published_factory_root_is_valid_before_expiry_only(self) -> None:
        root = (Path(__file__).parent / "fixtures" / "google_factory_root_2016.der").read_bytes()
        self.assertEqual(hashlib.sha256(root).digest(), GOOGLE_FACTORY_2016_ROOT_SHA256)
        self.assertTrue(_certificate_valid_at(root, GOOGLE_FACTORY_2016_VERIFICATION_TIME_MS))
        self.assertFalse(_certificate_valid_at(root, GOOGLE_FACTORY_2016_VERIFICATION_TIME_MS
                                                + 30 * 86_400_000))

    def test_patch_levels_normalize_only_documented_keymint_forms(self) -> None:
        for value, expected in ((202609, 202609), (20260905, 202609), (20260900, 202609),
                                (190001, 190001), (999912, 999912)):
            with self.subTest(value=value):
                self.assertEqual(patch_level_yyyymm(value), expected)
        for value in (0, 202600, 202613, 189912, 2026, 20261301, 100000000, True, "202609", None, 202609.0):
            with self.subTest(value=value):
                self.assertIsNone(patch_level_yyyymm(value))

    def test_patch_policy_fact_uses_every_reported_hardware_level(self) -> None:
        current = AndroidPatchLevels(300, 150000, 202609, 20260905, 20260901)
        self.assertIs(android_patch_policy_met(current, 202609), True)
        self.assertIs(android_patch_policy_met(current, 202601), True)
        # Vendor and boot levels that are absent or zero are "not reported".
        self.assertIs(android_patch_policy_met(AndroidPatchLevels(3, None, 202609, None, 0), 202609), True)
        # An unmet policy is the recorded PATCH_POLICY_MET fact, never an error.
        for levels in (
            AndroidPatchLevels(300, 150000, None, 20260905, 20260901),  # OS level absent
            AndroidPatchLevels(300, 150000, 0, 20260905, 20260901),     # OS level not reported
            AndroidPatchLevels(300, 150000, 202608, 20260905, 20260901),
            AndroidPatchLevels(300, 150000, 202609, 20260805, 20260901),
            AndroidPatchLevels(300, 150000, 202609, 20260905, 20260801),
            AndroidPatchLevels(300, 150000, 202613, 20260905, 20260901),  # unparsable OS
            AndroidPatchLevels(300, 150000, 202609, 2026, 20260901),      # unparsable vendor
            AndroidPatchLevels(300, 150000, 202609, 20260905, 20261301),  # unparsable boot
        ):
            with self.subTest(levels=levels):
                self.assertIs(android_patch_policy_met(levels, 202609), False)

    def test_bad_patch_floor_or_levels_raise_instead_of_reading_as_unmet(self) -> None:
        current = AndroidPatchLevels(300, 150000, 202609, 20260905, 20260901)
        self.assertEqual(validate_android_patch_floor(202609), 202609)
        for floor in (0, 20260901, 202613, 2026, True, "202609", None):
            with self.subTest(floor=floor):
                with self.assertRaisesRegex(AttestationRejected, "patch floor"):
                    validate_android_patch_floor(floor)
                # A misconfigured floor must not be recorded as policy-not-met.
                with self.assertRaisesRegex(AttestationRejected, "patch floor"):
                    android_patch_policy_met(current, floor)
        for levels in ((300, 150000, 202609, 20260905, 20260901), None,
                       AndroidPatchLevels(300, 150000, "202609", 20260905, 20260901),
                       AndroidPatchLevels(300, 150000, 202609, 20260905.0, 20260901)):
            with self.subTest(levels=levels), self.assertRaisesRegex(AttestationRejected, "patch levels absent"):
                android_patch_policy_met(levels, 202609)


if __name__ == "__main__":
    unittest.main()
