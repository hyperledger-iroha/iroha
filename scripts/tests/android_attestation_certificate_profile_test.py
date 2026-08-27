"""Focused Factory/RKP certificate-profile tests for the Android lab checker."""

from __future__ import annotations

import hashlib
from unittest import mock
import unittest

try:
    from scripts.tests import check_android_device_lab_slot_test as _slot_tests
except ModuleNotFoundError:
    import check_android_device_lab_slot_test as _slot_tests


device_lab = _slot_tests.device_lab
MODULE_PATH = _slot_tests.MODULE_PATH
android_attestation_metadata = _slot_tests.android_attestation_metadata
test_android_attestation_chain = _slot_tests.test_android_attestation_chain


def _der_tlv(tag: bytes, value: bytes) -> bytes:
    if len(value) < 0x80:
        length = bytes((len(value),))
    else:
        encoded = len(value).to_bytes((len(value).bit_length() + 7) // 8, "big")
        length = bytes((0x80 | len(encoded),)) + encoded
    return tag + length + value


def _der_sequence_parts(encoded: bytes) -> list[tuple[int, bool, int, bytes, bytes]]:
    reader_type = device_lab._android_x509._StrictDerReader
    wrapper = reader_type(encoded)
    sequence = wrapper.expect(0, True, 16, "test DER sequence")
    wrapper.finish("test DER sequence")
    reader = reader_type(sequence)
    parts = []
    while reader.remaining():
        parts.append(reader.read())
    return parts


class AndroidAttestationCertificateProfileTest(unittest.TestCase):
    """Exercise exact Factory/RKP classification and time semantics."""

    @classmethod
    def setUpClass(cls) -> None:
        _slot_tests.AndroidDeviceLabSlotTest.setUpClass()

    @classmethod
    def tearDownClass(cls) -> None:
        _slot_tests.AndroidDeviceLabSlotTest.tearDownClass()

    def test_android_factory_profile_ignores_only_target_leaf_expiration(self) -> None:
        metadata = android_attestation_metadata("pixel8-expired-target")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
            leaf_days=1,
        )
        certificates = device_lab._decode_attestation_certificate_chain(
            "chain.pem", chain
        )
        (leaf_not_before, leaf_not_after), _ = (
            device_lab._x509_certificate_validity_and_subject(certificates[0])
        )
        (root_not_before, root_not_after), _ = (
            device_lab._x509_certificate_validity_and_subject(certificates[-1])
        )
        evaluation_time_ms = leaf_not_after + 1
        self.assertGreaterEqual(evaluation_time_ms, leaf_not_before)
        self.assertGreaterEqual(evaluation_time_ms, root_not_before)
        self.assertLessEqual(evaluation_time_ms, root_not_after)
        errors: list[str] = []
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        receipt = authority["attestation_status_capture_receipt"]
        original_response_date = receipt["snapshot"]["response_date_ms"]
        original_fresh_until = receipt["payload"]["fresh_until_ms"]
        try:
            receipt["snapshot"]["response_date_ms"] = evaluation_time_ms - 1
            receipt["payload"]["fresh_until_ms"] = evaluation_time_ms + 1
            with mock.patch.object(
                device_lab.time,
                "time_ns",
                return_value=evaluation_time_ms * 1_000_000,
            ):
                count = device_lab._validate_android_attestation_certificate_chain(
                    "attestation/chain.pem", chain, metadata, errors
                )
        finally:
            receipt["snapshot"]["response_date_ms"] = original_response_date
            receipt["payload"]["fresh_until_ms"] = original_fresh_until
        self.assertEqual(count, 2, errors)
        self.assertEqual(errors, [])

    def test_android_factory_profile_rejects_not_yet_valid_non_target(self) -> None:
        metadata = android_attestation_metadata("pixel8-future-issuer")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        certificates = device_lab._decode_attestation_certificate_chain(
            "chain.pem", chain
        )
        (root_not_before, _), _ = device_lab._x509_certificate_validity_and_subject(
            certificates[-1]
        )
        self.assertEqual(
            device_lab._validate_android_attestation_certificate_time_profile(
                certificates,
                evaluation_time_ms=root_not_before,
            ),
            "factory",
        )
        errors: list[str] = []
        evaluation_time_ms = root_not_before - 1
        authority = device_lab._ANDROID_EVIDENCE_AUTHORITY
        assert authority is not None
        receipt = authority["attestation_status_capture_receipt"]
        original_response_date = receipt["snapshot"]["response_date_ms"]
        original_fresh_until = receipt["payload"]["fresh_until_ms"]
        try:
            receipt["snapshot"]["response_date_ms"] = evaluation_time_ms - 1
            receipt["payload"]["fresh_until_ms"] = evaluation_time_ms + 1
            with mock.patch.object(
                device_lab.time,
                "time_ns",
                return_value=evaluation_time_ms * 1_000_000,
            ):
                self.assertIsNone(
                    device_lab._validate_android_attestation_certificate_chain(
                        "attestation/chain.pem", chain, metadata, errors
                    )
                )
        finally:
            receipt["snapshot"]["response_date_ms"] = original_response_date
            receipt["payload"]["fresh_until_ms"] = original_fresh_until
        self.assertTrue(any("not yet valid" in error for error in errors), errors)

    def test_android_factory_expiration_exception_is_exactly_legacy_root(self) -> None:
        metadata = android_attestation_metadata("pixel8-factory-expiration")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        certificates = device_lab._decode_attestation_certificate_chain(
            "chain.pem", chain
        )
        (_, root_not_after), _ = device_lab._x509_certificate_validity_and_subject(
            certificates[-1]
        )
        self.assertEqual(
            device_lab._validate_android_attestation_certificate_time_profile(
                certificates,
                evaluation_time_ms=root_not_after,
            ),
            "factory",
        )
        with self.assertRaisesRegex(ValueError, "factory certificate is expired"):
            device_lab._validate_android_attestation_certificate_time_profile(
                certificates,
                evaluation_time_ms=root_not_after + 1,
            )

        legacy_root = (
            MODULE_PATH.parents[1] / "certs" / "google_attestation_root_rsa.der"
        ).read_bytes()
        (_, legacy_not_after), _ = device_lab._x509_certificate_validity_and_subject(
            legacy_root
        )
        self.assertEqual(
            hashlib.sha256(legacy_root).hexdigest(),
            device_lab.ANDROID_LEGACY_GOOGLE_ATTESTATION_ROOT_SHA256,
        )
        self.assertEqual(
            device_lab._validate_android_attestation_certificate_time_profile(
                [certificates[0], legacy_root],
                evaluation_time_ms=legacy_not_after + 1,
            ),
            "factory",
        )

    def test_android_leaf_rejects_x509_v1_and_v2_versions(self) -> None:
        metadata = android_attestation_metadata("pixel8-leaf-version")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        leaf = device_lab._decode_attestation_certificate_chain("chain.pem", chain)[0]
        version_3 = b"\xa0\x03\x02\x01\x02"
        self.assertEqual(leaf.count(version_3), 1)
        explicit_v2 = leaf.replace(
            version_3,
            b"\xa0\x03\x02\x01\x01",
            1,
        )
        outer = _der_sequence_parts(leaf)
        tbs = _der_sequence_parts(outer[0][4])
        omitted_v1_tbs = _der_tlv(
            b"\x30", b"".join(part[4] for part in tbs[1:])
        )
        omitted_v1 = _der_tlv(
            b"\x30", omitted_v1_tbs + outer[1][4] + outer[2][4]
        )
        for version, downgraded in (
            ("v1-omitted", omitted_v1),
            ("v2-explicit", explicit_v2),
        ):
            with self.subTest(version=version):
                with self.assertRaisesRegex(ValueError, "X.509 version 3"):
                    device_lab._x509_certificate_serial_and_attestation_extension(
                        downgraded
                    )

    def test_android_certificate_der_profile_rejects_security_ambiguities(self) -> None:
        metadata = android_attestation_metadata("pixel8-strict-certificate")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        leaf = device_lab._decode_attestation_certificate_chain("chain.pem", chain)[0]
        outer = _der_sequence_parts(leaf)
        self.assertEqual(len(outer), 3)
        tbs = _der_sequence_parts(outer[0][4])

        outer_algorithm = outer[1][4]
        sha256_oid = bytes.fromhex("06082a8648ce3d040302")
        self.assertIn(sha256_oid, outer_algorithm)
        mismatched_algorithm = outer_algorithm.replace(
            sha256_oid, bytes.fromhex("06082a8648ce3d040303"), 1
        )
        mismatched = _der_tlv(
            b"\x30", outer[0][4] + mismatched_algorithm + outer[2][4]
        )
        with self.assertRaisesRegex(ValueError, "must match exactly"):
            device_lab._x509_certificate_serial(mismatched)

        long_serial_tbs = _der_tlv(
            b"\x30",
            tbs[0][4]
            + _der_tlv(b"\x02", b"\x01" * 21)
            + b"".join(part[4] for part in tbs[2:]),
        )
        long_serial = _der_tlv(
            b"\x30", long_serial_tbs + outer_algorithm + outer[2][4]
        )
        with self.assertRaisesRegex(ValueError, "20-byte"):
            device_lab._x509_certificate_serial(long_serial)

        self.assertEqual(tbs[-1][:3], (2, True, 3))
        extensions = _der_sequence_parts(tbs[-1][3])
        duplicate_extensions = _der_tlv(
            b"\xa3",
            _der_tlv(
                b"\x30",
                b"".join(part[4] for part in extensions) + extensions[0][4],
            ),
        )
        duplicate_tbs = _der_tlv(
            b"\x30",
            b"".join(part[4] for part in tbs[:-1]) + duplicate_extensions,
        )
        duplicate = _der_tlv(
            b"\x30", duplicate_tbs + outer_algorithm + outer[2][4]
        )
        with self.assertRaisesRegex(ValueError, "duplicate .* extension OIDs"):
            device_lab._x509_certificate_serial_and_attestation_extension(duplicate)

    def test_android_rkp_profile_is_valid_at_the_evidence_validation_horizon(self) -> None:
        metadata = android_attestation_metadata("pixel8-rkp")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
            chain_kind="rkp",
        )
        certificates = device_lab._decode_attestation_certificate_chain(
            "chain.pem", chain
        )
        self.assertEqual(
            device_lab._classify_android_attestation_certificate_chain(
                certificates[-2]
            ),
            "rkp",
        )
        errors: list[str] = []
        self.assertEqual(
            device_lab._validate_android_attestation_certificate_chain(
                "attestation/chain.pem", chain, metadata, errors
            ),
            3,
        )
        self.assertEqual(errors, [])

    def test_android_rkp_profile_rejects_expired_non_target(self) -> None:
        metadata = android_attestation_metadata("pixel8-rkp-expired")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
            chain_kind="rkp",
        )
        certificates = device_lab._decode_attestation_certificate_chain(
            "chain.pem", chain
        )
        non_target_not_after = min(
            device_lab._x509_certificate_validity_and_subject(certificate)[0][1]
            for certificate in certificates[1:]
        )
        self.assertEqual(
            device_lab._validate_android_attestation_certificate_time_profile(
                certificates,
                evaluation_time_ms=non_target_not_after,
            ),
            "rkp",
        )
        with self.assertRaisesRegex(ValueError, "RKP certificate is not valid"):
            device_lab._validate_android_attestation_certificate_time_profile(
                certificates,
                evaluation_time_ms=non_target_not_after + 1,
            )

    def test_android_unknown_chain_profile_is_rejected(self) -> None:
        metadata = android_attestation_metadata("pixel8-unknown-chain")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
            chain_kind="unknown",
        )
        errors: list[str] = []
        self.assertIsNone(
            device_lab._validate_android_attestation_certificate_chain(
                "attestation/chain.pem", chain, metadata, errors
            )
        )
        self.assertTrue(
            any("classification is unknown" in error for error in errors), errors
        )

    def test_android_path_verification_uses_manual_time_profile(self) -> None:
        metadata = android_attestation_metadata("pixel8-manual-time")
        challenge = device_lab.derive_kagemusha_strongbox_challenge_v1(metadata)
        chain = test_android_attestation_chain(
            challenge,
            metadata["app_package_name"],
            bytes.fromhex(metadata["app_signing_certificate_sha256"]),
        )
        errors: list[str] = []
        with mock.patch.object(
            device_lab,
            "_run_pinned_openssl",
            wraps=device_lab._run_pinned_openssl,
        ) as run_openssl:
            count = device_lab._validate_android_attestation_certificate_chain(
                "attestation/chain.pem", chain, metadata, errors
            )
        self.assertEqual(count, 2, errors)
        self.assertTrue(run_openssl.call_args_list)
        for call in run_openssl.call_args_list:
            self.assertEqual(call.args[0][0], "verify")
            self.assertIn("-no_check_time", call.args[0])
