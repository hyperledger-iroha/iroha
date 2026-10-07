"""Existing actual current scope/primitive against Model vectors and real X509.

These component tests do not authorize a serving issuer, production app or device.
No current primitive or adapter implementation is copied from an obsolete donor.
"""
import hashlib
import json
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

from iroha_app_attestation.attestation import (
    ANDROID_KEY_DESCRIPTION_OID, AttestationRejected,
    children, der_one, primitive, verify_android_wallet_payment_key_raw,
)
from iroha_app_attestation.wallet_enrollment import (
    WalletEnrollmentScope, VerifiedWalletEnrollmentEvidence,
)

def der(tag: bytes, value: bytes) -> bytes:
    length = len(value)
    encoded = bytes([length]) if length < 128 else bytes([0x80 | ((length.bit_length() + 7) // 8)]) + length.to_bytes((length.bit_length() + 7) // 8, "big")
    return tag + encoded + value


def sequence(*parts: bytes) -> bytes:
    return der(b"\x30", b"".join(parts))


def set_of(*parts: bytes) -> bytes:
    return der(b"\x31", b"".join(parts))


def integer(value: int, tag: bytes = b"\x02") -> bytes:
    raw = value.to_bytes(max(1, (value.bit_length() + 7) // 8), "big")
    if raw[0] >= 128:
        raw = b"\0" + raw
    return der(tag, raw)


def octets(value: bytes) -> bytes:
    return der(b"\x04", value)


def explicit(number: int, value: bytes) -> bytes:
    if number < 31:
        return der(bytes([0xa0 | number]), value)
    components = [number & 127]
    number >>= 7
    while number:
        components.insert(0, 0x80 | (number & 127))
        number >>= 7
    return der(b"\xbf" + bytes(components), value)



class SignedEnvelope:
    def __init__(self, directory: Path, openssl: Path) -> None:
        self.directory = directory
        self.openssl = openssl
        self.run("req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                 "-nodes", "-keyout", "root.key", "-out", "root.pem", "-days", "2",
                 "-subj", "/CN=Synthetic Root", "-addext", "basicConstraints=critical,CA:TRUE",
                 "-addext", "keyUsage=critical,keyCertSign,cRLSign")
        self.run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                 "-nodes", "-keyout", "leaf.key", "-out", "leaf.csr", "-subj", "/CN=Synthetic Leaf")
        self.run("pkey", "-in", "leaf.key", "-pubout", "-outform", "DER", "-out", "leaf.spki")
        spki = children(der_one((directory / "leaf.spki").read_bytes()))
        self.point = primitive(spki[1], 3)[1:]

    def run(self, *arguments: str) -> None:
        subprocess.run([str(self.openssl), *arguments], cwd=self.directory,
                       capture_output=True, check=True)

    def sign(self, extension_oid: str, extension_der: bytes) -> tuple[bytes, bytes]:
        extension = self.directory / "extension.cnf"
        extension.write_text(extension_oid + "=DER:" + ":".join(f"{byte:02X}" for byte in extension_der) + "\n")
        self.run("x509", "-req", "-in", "leaf.csr", "-CA", "root.pem", "-CAkey", "root.key",
                 "-CAcreateserial", "-out", "leaf.pem", "-days", "2", "-extfile", "extension.cnf")
        self.run("x509", "-in", "root.pem", "-outform", "DER", "-out", "root.der")
        self.run("x509", "-in", "leaf.pem", "-outform", "DER", "-out", "leaf.der")
        return (self.directory / "leaf.der").read_bytes(), (self.directory / "root.der").read_bytes()


def keymint_description(expected_challenge: bytes, package_name: str,
                        package_version: int, signer: bytes, *,
                        verified_boot_hash: bytes | None = b"\x52" * 32,
                        attestation_version: int = 300, keymaster_version: int = 300,
                        security_level: int = 2, keymint_security_level: int = 2,
                        rollback_resistant: bool = False,
                        legacy_rollback_resistant: bool = False,
                        usage_count: int | None = None,
                        software_usage_count: int | None = None,
                        purpose: int = 2, algorithm: int = 3,
                        key_size: int = 256, digest: int = 4,
                        curve: int = 1, origin: int = 0,
                        device_locked: bool = True, verified_boot_state: int = 0,
                        software_key_size: bool = False,
                        os_version: int | None = None, os_patch_level: int | None = None,
                        vendor_patch_level: int | None = 20260805,
                        boot_patch_level: int | None = None,
                        software_os_patch_level: int | None = None) -> bytes:
    """Real signed synthetic KeyMint evidence for exact raw challenge bytes."""
    app_id = sequence(set_of(sequence(octets(package_name.encode()), integer(package_version))),
                      set_of(octets(signer)))
    software_fields = ([] if not software_key_size else [explicit(3, integer(key_size))])
    if software_usage_count is not None:
        software_fields.append(explicit(405, integer(software_usage_count)))
    if software_os_patch_level is not None:
        software_fields.append(explicit(706, integer(software_os_patch_level)))
    software = sequence(*software_fields, explicit(709, octets(app_id)))
    boot = sequence(octets(b"\x51" * 32),
                    der(b"\x01", b"\xff" if device_locked else b"\x00"),
                    integer(verified_boot_state, b"\x0a"),
                    *([] if verified_boot_hash is None else [octets(verified_boot_hash)]))
    hardware_fields = [explicit(1, set_of(integer(purpose))), explicit(2, integer(algorithm))]
    if not software_key_size:
        hardware_fields.append(explicit(3, integer(key_size)))
    hardware_fields.extend([explicit(5, set_of(integer(digest))), explicit(10, integer(curve))])
    if rollback_resistant:
        hardware_fields.append(explicit(303, der(b"\x05", b"")))
    if usage_count is not None:
        hardware_fields.append(explicit(405, integer(usage_count)))
    hardware_fields.append(explicit(702, integer(origin)))
    if legacy_rollback_resistant:
        hardware_fields.append(explicit(703, der(b"\x05", b"")))
    hardware_fields.append(explicit(704, boot))
    if os_version is not None:
        hardware_fields.append(explicit(705, integer(os_version)))
    if os_patch_level is not None:
        hardware_fields.append(explicit(706, integer(os_patch_level)))
    if attestation_version >= 3 and vendor_patch_level is not None:
        hardware_fields.append(explicit(718, integer(vendor_patch_level)))
    if attestation_version >= 3 and boot_patch_level is not None:
        hardware_fields.append(explicit(719, integer(boot_patch_level)))
    hardware = sequence(*hardware_fields)
    return sequence(
        integer(attestation_version), integer(security_level, b"\x0a"),
        integer(keymaster_version), integer(keymint_security_level, b"\x0a"),
        octets(expected_challenge), octets(b""),
        software, hardware,
    )


GENERATOR = bytes.fromhex("046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
                          "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5")


class CurrentWalletChallengeTests(unittest.TestCase):
    def setUp(self) -> None:
        executable = shutil.which("openssl")
        if executable is None:
            self.fail("real OpenSSL is required for this crypto control")
        self.openssl = Path(executable).resolve()
        vectors = json.loads((Path(__file__).resolve().parents[3]
                              / "fixtures/kagemusha/wallet_v1_vectors.json").read_text())
        self.vectors = {item["role"]: item for item in vectors["digests"]}
        self.challenge = WalletEnrollmentScope(
            bytes.fromhex(self.vectors["enrollment-challenge"]["body_hex"]), GENERATOR)
        self.package = "test.wallet.current"
        self.signer = b"\x73" * 32
        # Certificates are minted after setUp, potentially across a seconds boundary.
        # Keep fixture trusted time within their real two-day validity window.
        self.now = int(time.time() * 1000) + 60000

    def verify(self, fixture, leaf, root, **changes):
        selected = changes.pop("challenge", self.challenge)
        selected.validate()
        arguments = dict(
            chain_der=[leaf, root],
            challenge_digest=selected.challenge_digest(),
            package_name=self.package,
            package_version=7,
            signing_certificate_sha256=self.signer,
            root_der=root,
            root_sha256=hashlib.sha256(root).digest(),
            trusted_time_ms=self.now,
            openssl_path=self.openssl,
            allowed_security_levels=frozenset({1, 2}),
        )
        arguments.update(changes)
        return verify_android_wallet_payment_key_raw(**arguments)

    def signed(self, fixture, **changes):
        extension = keymint_description(self.challenge.challenge_digest(),
                                       self.package, 7, self.signer, **changes)
        return fixture.sign(ANDROID_KEY_DESCRIPTION_OID, extension)

    def test_exact_current_model_challenge_digest_and_preimage(self):
        vector = self.vectors["enrollment-challenge"]
        self.assertEqual(len(self.challenge.challenge_transcript), 194)
        self.assertEqual(self.challenge.challenge_digest().hex(), vector["digest_hex"])
        self.assertEqual(hashlib.sha256(bytes.fromhex(vector["preimage_hex"])).digest(),
                         self.challenge.challenge_digest())
        self.assertNotEqual(hashlib.sha256(self.challenge.challenge_transcript).digest(),
                            self.challenge.challenge_digest())
        self.assertNotEqual(hashlib.sha256(self.challenge.challenge_digest()).digest(),
                            self.challenge.challenge_digest())

    def test_exact_current_model_evidence_digest_preserves_original_item_order(self):
        vector = self.vectors["evidence"]
        items = (b"tee-leaf-der", b"tee-intermediate-der")
        self.assertEqual(VerifiedWalletEnrollmentEvidence(1, 1, 0, 0, 0, 0, items).evidence_digest().hex(), vector["digest_hex"])
        self.assertNotEqual(VerifiedWalletEnrollmentEvidence(1, 1, 0, 0, 0, 0, items[::-1]).evidence_digest(),
                            VerifiedWalletEnrollmentEvidence(1, 1, 0, 0, 0, 0, items).evidence_digest())
        self.assertNotEqual(VerifiedWalletEnrollmentEvidence(2, 1, 0, 0, 0, 0, items).evidence_digest(),
                            VerifiedWalletEnrollmentEvidence(1, 1, 0, 0, 0, 0, items).evidence_digest())

    def test_current_complete_android_digest_retains_issuer_response_and_key_binding(self):
        # Arbitrary codec items establish serialization only, never Google verification.
        items = (b"original-leaf", b"original-root", b"original-issuer-google-response")
        body = b"\1" + len(items).to_bytes(4, "little")
        body += b"".join(len(item).to_bytes(4, "little") + item for item in items)
        expected = hashlib.sha256(b"iroha:kagemusha:wallet:v1:evidence\0"
                                 + len(body).to_bytes(8, "little") + body).digest()
        current = VerifiedWalletEnrollmentEvidence(1, 1, 0, 0, 0, 0, items)
        self.assertEqual(current.evidence_digest(), expected)
        self.assertNotEqual(current.evidence_digest(),
                            VerifiedWalletEnrollmentEvidence(1, 1, 0, 0, 0, 0, items[:2]).evidence_digest())
        key_vector = self.vectors["enrollment-key-binding"]
        key_body = bytes.fromhex(key_vector["body_hex"])
        # Published role vectors use independent challenge-digest bodies; they do
        # not claim this role's body derives from another role's transcript vector.
        self.assertEqual(hashlib.sha256(bytes.fromhex(key_vector["preimage_hex"])).hexdigest(),
                         key_vector["digest_hex"])
        self.assertEqual(len(key_body), 97)
        selected = WalletEnrollmentScope(self.challenge.challenge_transcript, key_body[32:])
        binding_body = selected.challenge_digest() + selected.payment_key_sec1
        binding = hashlib.sha256(b"iroha:kagemusha:wallet:v1:enrollment-key-binding\0"
                                 + len(binding_body).to_bytes(8, "little") + binding_body).digest()
        self.assertEqual(selected.enrollment_key_binding(), binding)

    def test_real_signed_current_challenge_binds_tee_and_strongbox_payment_key(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            for version, keymaster, level, boot_hash in [
                    (2, 3, 1, None), (3, 4, 1, b"\x52" * 32),
                    (300, 300, 2, b"\x52" * 32)]:
                with self.subTest(version=version, level=level):
                    leaf, root = self.signed(fixture, attestation_version=version,
                        keymaster_version=keymaster, security_level=level,
                        keymint_security_level=level, verified_boot_hash=boot_hash,
                        os_patch_level=202609, vendor_patch_level=20260905,
                        boot_patch_level=20260901)
                    result = self.verify(fixture, leaf, root)
                    self.assertEqual(result.attested_public_key_sec1, fixture.point)
                    self.assertEqual(result.android_security_level, level)
                    self.assertEqual(result.android_patch_levels.os_patch_level, 202609)

    def test_each_current_challenge_binding_rejects_real_original_chain(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            leaf, root = self.signed(fixture)
            for offset in range(2, 194, 32):
                modified = bytearray(self.challenge.challenge_transcript)
                modified[offset] ^= 1
                with self.subTest(offset=offset), self.assertRaisesRegex(
                        AttestationRejected, "KeyMint challenge mismatch"):
                    self.verify(fixture, leaf, root,
                        challenge=WalletEnrollmentScope(bytes(modified), fixture.point))

    def test_raw_app_root_hardware_and_chain_substitutions_reject(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            leaf, root = self.signed(fixture)
            for change in [dict(package_name="other.app"),
                           dict(package_version=8),
                           dict(signing_certificate_sha256=b"\x74" * 32),
                           dict(root_sha256=b"\x75" * 32),
                           dict(allowed_security_levels=frozenset({1})),
                           dict(trusted_time_ms=self.now + 10 * 86400 * 1000),
                           dict(chain_der=[root, leaf]),
                           dict(chain_der=[leaf, leaf]),
                           dict(chain_der=[leaf[:-1] + bytes([leaf[-1] ^ 1]), root])]:
                with self.subTest(fields=list(change)), self.assertRaises(AttestationRejected):
                    self.verify(fixture, leaf, root, **change)

    def test_real_signed_bad_hardware_fields_reject_actual_current_primitive(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            for change in [dict(device_locked=False), dict(verified_boot_state=1),
                           dict(origin=2), dict(software_key_size=True),
                           dict(purpose=3), dict(usage_count=1),
                           dict(keymint_security_level=1)]:
                with self.subTest(fields=list(change)):
                    leaf, root = self.signed(fixture, **change)
                    with self.assertRaises(AttestationRejected):
                        self.verify(fixture, leaf, root)

    def test_noncurrent_transcripts_versions_zero_bindings_and_oversize_reject(self):
        invalid = [self.challenge.challenge_transcript[:-1], self.challenge.challenge_transcript + b"\0",
                   b"\x02\0" + self.challenge.challenge_transcript[2:],
                   b"\0\0" + self.challenge.challenge_transcript[2:],
                   bytearray(self.challenge.challenge_transcript)]
        for offset in range(2, 194, 32):
            invalid.append(self.challenge.challenge_transcript[:offset] + b"\0" * 32
                           + self.challenge.challenge_transcript[offset + 32:])
        for value in invalid:
            with self.subTest(length=len(value)), self.assertRaises(AttestationRejected):
                WalletEnrollmentScope(value, GENERATOR).validate()


if __name__ == "__main__":
    unittest.main()
