"""Opt-in interoperability with actual Rust-produced known-public fixture originals.

Set KAGEMUSHA_ORDINARY_TEST_GOLDEN, KAGEMUSHA_TEST_APP_ENCODER and
KAGEMUSHA_TEST_RAW_ENCODER to the held output of the actual current Rust gates.
The fixture contains inert raw attestation/Google data, never physical qualification.
Only its explicit known-public seed is passed to these real native encoder processes.
"""
import base64
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from iroha_app_attestation.attestation import AttestationRejected, RawPlatformProof, device_key_reference
from iroha_app_attestation.issuance import encode_ordinary_with_iroha, encode_refresh_with_iroha
from iroha_app_attestation.ordinary_enrollment import (
    CHALLENGE_BODY_BYTES, EnrollmentPossession, EVIDENCE_DOMAIN,
    authenticate_challenge_transport, credential_signing_request, decode_challenge_transport,
)
from iroha_app_attestation.play_integrity import PlayIntegrityPolicy, PlayIntegrityProof
from iroha_app_attestation.play_integrity_refresh import (
    authenticate_refresh_transport, verify_refresh_possession,
)


def decoded(value):
    result = base64.b64decode(value, validate=True)
    if base64.b64encode(result).decode("ascii") != value:
        raise ValueError("noncanonical fixture base64")
    return result


def file_identity(stat):
    # Reading and executing these held public originals legitimately changes access time.
    # Replacement, permissions and content timestamps must remain identical.
    return (stat.st_dev, stat.st_ino, stat.st_mode, stat.st_uid, stat.st_gid,
            stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns)


class NativeCodecCrossoverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        names = ("KAGEMUSHA_ORDINARY_TEST_GOLDEN", "KAGEMUSHA_TEST_APP_ENCODER",
                 "KAGEMUSHA_TEST_RAW_ENCODER")
        supplied = [os.environ.get(name) for name in names]
        if not any(supplied):
            raise unittest.SkipTest("requires actual current Rust golden and both built encoders")
        if not all(supplied):
            raise ValueError("native crossover requires all three held originals")
        cls.paths = [Path(value) for value in supplied]
        cls.originals = []
        for path in cls.paths:
            if not path.is_absolute() or path.is_symlink() or not path.is_file():
                raise ValueError("native crossover original must be an absolute regular file")
            before = file_identity(path.stat())
            data = path.read_bytes()
            if before != file_identity(path.stat()):
                raise ValueError("native crossover original changed during intake")
            cls.originals.append((before, hashlib.sha256(data).digest()))
        if len(cls.paths[0].read_bytes()) > 1024 * 1024:
            raise ValueError("native golden outside fixture bound")
        cls.document = json.loads(cls.paths[0].read_text())
        if (cls.document.get("schema") != "iroha.kagemusha.ordinary-app-enrollment-public-codec-fixture.v1"
                or cls.document.get("authority") is not False
                or cls.document.get("app_authority_seed_hex") != "3d" * 32
                or cls.document.get("wallet_issuer_seed_hex") != "3e" * 32
                or cls.document.get("platform_p256_secret_hex") != "07" * 32
                or [row["platform"] for row in cls.document["vectors"]]
                != ["android_keymint", "apple_app_attest"]):
            raise ValueError("not the actual explicit known-public model fixture")
        executable = shutil.which("openssl")
        if not executable:
            raise ValueError("native crossover requires the selected OpenSSL verifier")
        cls.openssl = Path(executable).resolve()

    @classmethod
    def tearDownClass(cls):
        for path, (before, digest) in zip(cls.paths, cls.originals):
            if before != file_identity(path.stat()) or hashlib.sha256(path.read_bytes()).digest() != digest:
                raise AssertionError("held native crossover original drifted")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="iroha-native-crossover-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        # This seed is public in the Rust fixture; it is never a deployed issuer seed.
        self.seed = bytes.fromhex(self.document["app_authority_seed_hex"])
        key = self.directory / "public-fixture-seed"
        self.key_fd = os.open(key, os.O_CREAT | os.O_EXCL | os.O_RDWR, 0o600)
        self.addCleanup(os.close, self.key_fd)
        os.write(self.key_fd, self.seed)
        # Actual encoder pread must preserve this deliberately nonzero inherited offset.
        os.lseek(self.key_fd, 13, os.SEEK_SET)
        der = self.directory / "public-fixture.der"
        der.write_bytes(bytes.fromhex("302e020100300506032b657004220420") + self.seed)
        result = subprocess.run([str(self.openssl), "pkey", "-inform", "DER", "-in", str(der),
                                 "-pubout", "-outform", "DER"], capture_output=True,
                                check=True, timeout=5)
        self.assertEqual(result.stdout[:12], bytes.fromhex("302a300506032b6570032100"))
        self.public = result.stdout[12:]
        self.assertEqual(len(self.public), 32)

    def test_rust_challenge_and_e371_match_python_without_an_enrollment_id_fallback(self):
        for row in self.document["vectors"]:
            with self.subTest(platform=row["platform"]):
                original = decoded(row["signed_preparation_base64"])
                challenge = decode_challenge_transport(original)
                self.assertEqual(len(original), 515)
                self.assertEqual(challenge.signing_bytes(), decoded(row["preparation_signing_message_base64"]))
                authenticate_challenge_transport(original, challenge, self.public, 300, self.openssl)
                self.assertEqual(challenge.attestation_challenge().hex(), row["operation_id"])
                self.assertNotEqual(challenge.attestation_challenge(), challenge.enrollment_id)
                self.assertNotEqual(challenge.attestation_challenge(), hashlib.sha256(original).digest())
                key = bytes.fromhex(row["attested_key_id_hex"])
                raw = decoded(row["raw_attestation_base64"])
                self.assertEqual(challenge.play_integrity_request_hash(key).hex(), row["play_integrity_request_hash_hex"])
                self.assertEqual(challenge.possession_message(key, hashlib.sha256(raw).digest(),
                                 challenge.issued_at_ms, challenge.expires_at_ms),
                                 decoded(row["enrollment_possession_signing_message_base64"]))
                with self.assertRaises(AttestationRejected):
                    authenticate_challenge_transport(original[:-1] + bytes([original[-1] ^ 1]),
                                                     challenge, self.public, 300, self.openssl)

    def test_actual_native_koac_is_the_complete_rust_original_on_both_platforms(self):
        for row in self.document["vectors"]:
            with self.subTest(platform=row["platform"]):
                challenge = decode_challenge_transport(decoded(row["signed_preparation_base64"]))
                circuit_issuer_point = decoded(row["ordinary_issuer_public_key_sec1_base64"])
                self.assertEqual(len(circuit_issuer_point), 65)
                self.assertEqual(circuit_issuer_point[0], 4)
                body = decoded(row["ordinary_credential_signing_body_base64"])
                self.assertEqual(len(body), 794)
                point, raw = decoded(row["app_public_key_sec1_base64"]), decoded(row["raw_attestation_base64"])
                pop_object = row["certificate_request"]["app_possession"]
                pop = decoded(pop_object["signature_der_base64"] if challenge.platform_class == 1
                              else pop_object["raw_assertion_base64"])
                evidence_digest = hashlib.sha256(EVIDENCE_DOMAIN + len(raw).to_bytes(8, "little") + raw
                                  + len(pop).to_bytes(8, "little") + pop).digest()
                floor = int.from_bytes(body[677:681], "little")
                proof = RawPlatformProof(hashlib.sha256(raw).digest(), point, device_key_reference(point),
                                         row["platform"], android_security_level=body[3] if challenge.platform_class == 1 else None)
                possession = EnrollmentPossession(hashlib.sha256(point).digest(), hashlib.sha256(raw).digest(),
                                                  hashlib.sha256(pop).digest(), evidence_digest, floor, body[4+12*32:4+13*32], None)
                # Fixture projections are inert data, not a real decoded Google verdict.
                policy = integrity = None
                if challenge.platform_class == 1:
                    slot = body[-113:]
                    self.assertEqual(slot[0], 1)
                    policy = PlayIntegrityPolicy(slot[65:97], "fixture.example.wallet", 1, bytes([2]) * 32,
                                                 1000, 1000, True, True, "MEETS_DEVICE_INTEGRITY")
                    integrity = PlayIntegrityProof(slot[65:97], slot[1:33], 200, bytes([59]) * 32,
                                                   slot[33:65], "PLAY_RECOGNIZED", "LICENSED", ("MEETS_DEVICE_INTEGRITY",))
                request = credential_signing_request(challenge, proof, possession, bytes([2]) * 32,
                          bytes([3]) * 32, self.public, 200, 10200, 10000, 20000,
                          frozenset({1, 2}) if challenge.platform_class == 1 else frozenset(), policy, integrity,
                          circuit_issuer_point)
                self.assertEqual(request, b"KOAC\x01" + body + self.public + circuit_issuer_point)
                expected = decoded(row["ordinary_credential_base64"])
                first = encode_ordinary_with_iroha(request, self.paths[1], self.originals[1][1], self.key_fd)
                second = encode_ordinary_with_iroha(request, self.paths[1], self.originals[1][1], self.key_fd)
                self.assertEqual(first, expected)
                self.assertEqual(second, expected)
                self.assertEqual(os.lseek(self.key_fd, 0, os.SEEK_CUR), 13)
                digest_domain = b"iroha:kagemusha:v1:ordinary-app-credential-original\0"
                self.assertEqual(hashlib.sha256(digest_domain + len(first).to_bytes(8, "little") + first).hexdigest(),
                                 row["ordinary_credential_digest_hex"])
                with self.assertRaises(AttestationRejected):
                    encode_ordinary_with_iroha(request[:-1] + bytes([request[-1] ^ 1]), self.paths[1],
                                              self.originals[1][1], self.key_fd)

    def test_actual_raw_encoder_preserves_original_signature_and_inherited_offset(self):
        for row in self.document["vectors"]:
            with self.subTest(platform=row["platform"]):
                expected = decoded(row["raw_response"]["raw_admission_base64"])
                self.assertEqual(len(expected), 314)
                request = b"KRAC01" + expected[:250] + self.public
                self.assertEqual(len(request), 288)
                for _ in range(2):
                    result = subprocess.run([str(self.paths[2]), "--key-fd", str(self.key_fd)],
                                            input=request, pass_fds=(self.key_fd,), capture_output=True,
                                            env={"PATH": "/usr/bin:/bin"}, timeout=10)
                    self.assertEqual(result.returncode, 0)
                    self.assertEqual(result.stderr, b"")
                    self.assertEqual(result.stdout, expected)
                    self.assertEqual(os.lseek(self.key_fd, 0, os.SEEK_CUR), 13)
                invalid = subprocess.run([str(self.paths[2]), "--key-fd", str(self.key_fd)],
                                         input=b"KOAC01" + request[6:], pass_fds=(self.key_fd,),
                                         capture_output=True, env={"PATH": "/usr/bin:/bin"}, timeout=10)
                self.assertNotEqual(invalid.returncode, 0)
                self.assertEqual(invalid.stdout, b"")

    def test_actual_native_periodic_lease_retains_both_issuer_signatures_and_exact_refresh(self):
        row = self.document["vectors"][0]
        refresh = row["integrity_refresh"]
        self.assertIsNone(self.document["vectors"][1]["integrity_refresh"])
        challenge = authenticate_refresh_transport(decoded(refresh["signed_refresh_challenge_base64"]),
            public_key=self.public, openssl_path=self.openssl)
        self.assertEqual(challenge.signing_bytes(), decoded(refresh["refresh_signing_message_base64"]))
        self.assertEqual(challenge.attempt_id().hex(), refresh["operation_id"])
        self.assertEqual(challenge.request_hash().hex(), refresh["play_integrity_request_hash_hex"])
        self.assertEqual(challenge.possession_message(), decoded(refresh["possession_signing_message_base64"]))
        point = decoded(row["app_public_key_sec1_base64"])
        original_der = decoded(refresh["possession_der_base64"])
        verify_refresh_possession(challenge, point, original_der, self.openssl)
        body = decoded(refresh["lease_signing_body_base64"])
        self.assertEqual(len(body), 402)
        request = (b"KRPI\x01" + body + len(original_der).to_bytes(2, "little") + original_der
                   + self.public + decoded(row["ordinary_issuer_public_key_sec1_base64"]))
        expected = decoded(refresh["lease_base64"])
        self.assertNotEqual(expected, decoded(refresh["lease_ed_original_base64"]))
        self.assertEqual(len(decoded(refresh["lease_issuer_admission_base64"])), 163)
        for _ in range(2):
            actual = encode_refresh_with_iroha(request, self.paths[1], self.originals[1][1], self.key_fd)
            self.assertEqual(actual, expected)
            self.assertEqual(os.lseek(self.key_fd, 0, os.SEEK_CUR), 13)
        wrong_point = request[:-1] + bytes([request[-1] ^ 1])
        with self.assertRaises(AttestationRejected):
            encode_refresh_with_iroha(wrong_point, self.paths[1], self.originals[1][1], self.key_fd)


if __name__ == "__main__":
    unittest.main()
