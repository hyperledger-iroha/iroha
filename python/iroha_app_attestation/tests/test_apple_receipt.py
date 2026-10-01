"""Signed synthetic App Attest receipts, field bindings, and rejection cases."""

import hashlib
import shutil
import subprocess
import tempfile
import time
import unittest
from datetime import datetime, timezone
from pathlib import Path

from iroha_app_attestation.apple_receipt import (
    APPLE_FRAUD_RECEIPT_SIGNER_OID,
    MAX_RECEIPT,
    AttestationRejected,
    verify_apple_receipt,
)
from iroha_app_attestation.attestation import cbor_exact, certificate_key_extensions


def der(tag: int, value: bytes) -> bytes:
    size = len(value)
    length = (bytes([size]) if size < 128 else
              bytes([0x80 | ((size.bit_length() + 7) // 8)])
              + size.to_bytes((size.bit_length() + 7) // 8, "big"))
    return bytes([tag]) + length + value


def integer(value: int) -> bytes:
    raw = value.to_bytes(max(1, (value.bit_length() + 7) // 8), "big")
    return der(2, b"\0" + raw if raw[0] & 0x80 else raw)


def attribute(number: int, value: bytes) -> bytes:
    return der(0x30, integer(number) + integer(1) + der(4, value))


def timestamp(epoch_ms: int) -> bytes:
    instant = datetime.fromtimestamp(epoch_ms / 1000, tz=timezone.utc)
    return instant.strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3].encode() + b"Z"


class ReceiptFixture:
    def __init__(self, directory: Path, openssl: Path) -> None:
        self.directory = directory
        self.openssl = openssl
        self.run("req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                 "-nodes", "-keyout", "root.key", "-out", "root.pem", "-days", "2",
                 "-subj", "/CN=Synthetic Receipt Root",
                 "-addext", "basicConstraints=critical,CA:TRUE",
                 "-addext", "keyUsage=critical,keyCertSign,cRLSign")
        self.run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                 "-nodes", "-keyout", "attested.key", "-out", "attested.csr",
                 "-subj", "/CN=Synthetic Attested Key")
        self.run("x509", "-req", "-in", "attested.csr", "-CA", "root.pem",
                 "-CAkey", "root.key", "-CAcreateserial", "-out", "attested.pem", "-days", "2")
        self.run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                 "-nodes", "-keyout", "signer.key", "-out", "signer.csr",
                 "-subj", "/CN=Application Attestation Fraud Receipt Signing")
        (directory / "signer.cnf").write_text(
            "basicConstraints=critical,CA:FALSE\n"
            "keyUsage=critical,digitalSignature\n"
            f"{APPLE_FRAUD_RECEIPT_SIGNER_OID}=DER:05:00\n")
        self.run("x509", "-req", "-in", "signer.csr", "-CA", "root.pem",
                 "-CAkey", "root.key", "-CAcreateserial", "-out", "signer.pem",
                 "-days", "2", "-extfile", "signer.cnf")
        for name in ("root", "attested"):
            self.run("x509", "-in", name + ".pem", "-outform", "DER", "-out", name + ".der")
        self.root = (directory / "root.der").read_bytes()
        self.attested = (directory / "attested.der").read_bytes()
        self.point = certificate_key_extensions(self.attested)[0]

    def run(self, *arguments: str) -> None:
        subprocess.run([str(self.openssl), *arguments], cwd=self.directory,
                       capture_output=True, check=True)

    def receipt(self, values: dict[int, bytes], *, sorted_set: bool = True) -> bytes:
        parts = [(number, attribute(number, value)) for number, value in values.items()]
        ordered = sorted(parts) if sorted_set else parts
        (self.directory / "payload.der").write_bytes(der(0x31, b"".join(encoded for _, encoded in ordered)))
        self.run("cms", "-sign", "-binary", "-nodetach", "-nosmimecap",
                 "-in", "payload.der", "-signer", "signer.pem", "-inkey", "signer.key",
                 "-certfile", "root.pem", "-outform", "DER", "-out", "receipt.der")
        return (self.directory / "receipt.der").read_bytes()


class AppleReceiptTests(unittest.TestCase):
    def setUp(self) -> None:
        executable = shutil.which("openssl")
        if executable is None:
            self.skipTest("OpenSSL CLI unavailable")
        self.openssl = Path(executable).resolve()

    def _verify(self, fixture: ReceiptFixture, receipt: bytes, now_ms: int,
                *, app_id: str = "TEAMID.example.app", expected_type: str = "ATTEST"):
        return verify_apple_receipt(
            receipt, app_id, fixture.attested, fixture.point, fixture.root,
            hashlib.sha256(fixture.root).digest(), now_ms, self.openssl,
            expected_type=expected_type)

    def _fields(self, fixture: ReceiptFixture, now_ms: int) -> dict[int, bytes]:
        return {2: b"TEAMID.example.app", 3: fixture.attested,
                4: b"selected client challenge", 5: b"opaque token", 6: b"ATTEST",
                12: timestamp(now_ms - 1000), 21: timestamp(now_ms + 60_000)}

    def test_apple_published_receipt_uses_separate_g3_root(self) -> None:
        """Pin the real CMS chain from Apple's attestation-object guide.

        https://developer.apple.com/documentation/devicecheck/attestation-object-validation-guide
        """
        directory = Path(__file__).parent / "fixtures"
        sample = (directory / "apple_official_sample_attestation.cbor").read_bytes()
        self.assertEqual(hashlib.sha256(sample).hexdigest(),
                         "e4ca508153f6619a29d0887eb0ffe19540b9f15c5a6aeccdac26c49affd5f61a")
        statement = cbor_exact(sample)["attStmt"]
        receipt = statement["receipt"]
        leaf = statement["x5c"][0]
        point = certificate_key_extensions(leaf)[0]
        receipt_root = (directory / "apple_root_ca_g3.der").read_bytes()
        attestation_root = (directory / "apple_app_attestation_root.der").read_bytes()
        created_ms = 1_776_795_192_153
        checked = verify_apple_receipt(
            receipt, "1234567890.com.example.myapp", leaf, point,
            receipt_root, hashlib.sha256(receipt_root).digest(),
            created_ms + 1000, self.openssl)
        self.assertEqual(checked.creation_time_ms, created_ms)
        self.assertEqual(checked.receipt_type, "ATTEST")
        with self.assertRaises(AttestationRejected):
            verify_apple_receipt(
                receipt, "1234567890.com.example.myapp", leaf, point,
                attestation_root, hashlib.sha256(attestation_root).digest(),
                created_ms + 1000, self.openssl)

    def test_signed_receipt_binds_exact_app_certificate_and_freshness(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ReceiptFixture(Path(temporary), self.openssl)
            now = int(time.time()) * 1000
            values = self._fields(fixture, now)
            receipt = fixture.receipt(values)
            checked = self._verify(fixture, receipt, now)
            self.assertEqual(checked.receipt_type, "ATTEST")
            self.assertEqual(checked.creation_time_ms, now - 1000)
            self.assertEqual(checked.receipt_sha256, hashlib.sha256(receipt).digest())
            self.assertIsNone(checked.risk_metric)
            for changed in (
                {**values, 2: b"TEAMID.other.app"},
                {**values, 3: fixture.root},
                {**values, 6: b"RECEIPT"},
                {**values, 12: timestamp(now - 300_001)},
                {**values, 12: timestamp(now + 1)},
                {**values, 21: timestamp(now - 1)},
                {**values, 12: b"invalid"},
            ):
                with self.subTest(changed=list(set(changed.items()) ^ set(values.items()))):
                    with self.assertRaises(AttestationRejected):
                        self._verify(fixture, fixture.receipt(changed), now)

    def test_signature_root_pin_and_key_substitution_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ReceiptFixture(Path(temporary), self.openssl)
            now = int(time.time()) * 1000
            receipt = fixture.receipt(self._fields(fixture, now))
            mutated = bytearray(receipt)
            index = receipt.find(b"TEAMID.example.app")
            self.assertGreater(index, 0)
            mutated[index] ^= 1
            with self.assertRaises(AttestationRejected):
                self._verify(fixture, bytes(mutated), now)
            with self.assertRaises(AttestationRejected):
                verify_apple_receipt(receipt, "TEAMID.example.app", fixture.attested,
                                     fixture.point, fixture.root, b"\x55" * 32,
                                     now, self.openssl)
            with self.assertRaises(AttestationRejected):
                verify_apple_receipt(receipt, "TEAMID.example.app", fixture.attested,
                                     b"\x04" + b"\x33" * 64, fixture.root,
                                     hashlib.sha256(fixture.root).digest(), now, self.openssl)
            with self.assertRaises(AttestationRejected):
                self._verify(fixture, b"\0" * (MAX_RECEIPT + 1), now)
            unrelated = fixture.directory / "unrelated"
            unrelated.mkdir()
            other = ReceiptFixture(unrelated, self.openssl)
            with self.assertRaises(AttestationRejected):
                verify_apple_receipt(receipt, "TEAMID.example.app", fixture.attested,
                                     fixture.point, other.root,
                                     hashlib.sha256(other.root).digest(), now,
                                     self.openssl)

    def test_a_valid_chain_without_fraud_receipt_role_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ReceiptFixture(Path(temporary), self.openssl)
            now = int(time.time()) * 1000
            (fixture.directory / "signer-no-role.cnf").write_text(
                "basicConstraints=critical,CA:FALSE\n"
                "keyUsage=critical,digitalSignature\n")
            fixture.run("x509", "-req", "-in", "signer.csr", "-CA", "root.pem",
                        "-CAkey", "root.key", "-CAcreateserial", "-out", "signer.pem",
                        "-days", "2", "-extfile", "signer-no-role.cnf")
            with self.assertRaises(AttestationRejected):
                self._verify(fixture, fixture.receipt(self._fields(fixture, now)), now)

    def test_receipt_metric_and_duplicate_or_noncanonical_attribute_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ReceiptFixture(Path(temporary), self.openssl)
            now = int(time.time()) * 1000
            values = self._fields(fixture, now)
            values[6], values[17] = b"RECEIPT", b"5"
            self.assertEqual(self._verify(fixture, fixture.receipt(values), now,
                                          expected_type="RECEIPT").risk_metric, 5)
            for metric in (b"", b"-1", b"05", b"5.0"):
                with self.subTest(metric=metric), self.assertRaises(AttestationRejected):
                    self._verify(fixture, fixture.receipt({**values, 17: metric}),
                                 now, expected_type="RECEIPT")
            duplicate = [(number, attribute(number, value)) for number, value in values.items()]
            duplicate.append((2, attribute(2, values[2])))
            (fixture.directory / "payload.der").write_bytes(
                der(0x31, b"".join(encoded for _, encoded in sorted(duplicate))))
            fixture.run("cms", "-sign", "-binary", "-nodetach", "-nosmimecap",
                        "-in", "payload.der", "-signer", "signer.pem", "-inkey", "signer.key",
                        "-certfile", "root.pem", "-outform", "DER", "-out", "receipt.der")
            with self.assertRaises(AttestationRejected):
                self._verify(fixture, (fixture.directory / "receipt.der").read_bytes(),
                             now, expected_type="RECEIPT")
            with self.assertRaises(AttestationRejected):
                self._verify(fixture, fixture.receipt(values, sorted_set=False),
                             now, expected_type="RECEIPT")


if __name__ == "__main__":
    unittest.main()
