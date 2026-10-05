"""Cryptographically signed synthetic platform envelopes; never production roots."""

import hashlib
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

from iroha_app_attestation.attestation import (
    ANDROID_KEY_DESCRIPTION_OID,
    APPLE_NONCE_OID,
    AndroidPatchLevels,
    AttestationRejected,
    Selection,
    certificate_key_extensions,
    children,
    device_key_reference,
    encode_android_chain,
    der_one,
    primitive,
    android_patch_policy_met,
    verify_android_raw,
    verify_android_persistent_app_key_raw,
    verify_apple_raw,
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


def cbor(value) -> bytes:
    if isinstance(value, int):
        major, length = (0, value) if value >= 0 else (1, -1 - value)
        payload = b""
    elif isinstance(value, bytes):
        major, length, payload = 2, len(value), value
    elif isinstance(value, str):
        payload = value.encode("utf-8")
        major, length = 3, len(payload)
    elif isinstance(value, list):
        payload = b"".join(map(cbor, value))
        major, length = 4, len(value)
    else:
        payload = b"".join(cbor(key) + cbor(item) for key, item in value.items())
        major, length = 5, len(value)
    if length < 24:
        return bytes([(major << 5) | length]) + payload
    if length < 256:
        return bytes([(major << 5) | 24, length]) + payload
    if length < 65536:
        return bytes([(major << 5) | 25]) + length.to_bytes(2, "big") + payload
    return bytes([(major << 5) | 26]) + length.to_bytes(4, "big") + payload


def selection(point: bytes) -> Selection:
    return Selection(b"\x01" * 32, b"\x02" * 32, b"\x03" * 32,
                     b"\x04" * 32, hashlib.sha256(point).digest(), b"\x06" * 32)


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


def keymint_description(selected: Selection, package_name: str,
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
    """Signed synthetic ordinary StrongBox evidence; no production trust."""
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
        octets(hashlib.sha256(selected.transcript()).digest()), octets(b""),
        software, hardware,
    )


class SyntheticPlatformTests(unittest.TestCase):
    def setUp(self) -> None:
        executable = shutil.which("openssl")
        if executable is None:
            self.skipTest("OpenSSL CLI unavailable")
        self.openssl = Path(executable).resolve()

    def test_signed_apple_attest_challenge_app_and_key_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            selected = selection(fixture.point)
            app_id = "TEAMID.example.app"
            key_id = hashlib.sha256(fixture.point).digest()
            cose = cbor({1: 2, 3: -7, -1: 1, -2: fixture.point[1:33], -3: fixture.point[33:]})
            auth = (hashlib.sha256(app_id.encode()).digest() + b"\x40" + b"\0" * 4
                    + b"appattest" + b"\0" * 7 + (32).to_bytes(2, "big") + key_id + cose)
            nonce = hashlib.sha256(auth + hashlib.sha256(selected.transcript()).digest()).digest()
            leaf, root = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(nonce))))
            obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [leaf, root], "receipt": b"synthetic receipt"}, "authData": auth})
            now = int(time.time() * 1000) + 60_000
            pin = hashlib.sha256(root).digest()
            result = verify_apple_raw(obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                      expected_validation_category=None, expected_bundle_version=None)
            self.assertEqual(result.attested_public_key_sec1, fixture.point)
            self.assertEqual(result.device_key_reference, device_key_reference(fixture.point))
            self.assertIsNone(result.apple_bundle_version)
            untagged_leaf, _ = fixture.sign(APPLE_NONCE_OID, sequence(octets(nonce)))
            untagged_obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [untagged_leaf, root],
                "receipt": b"synthetic receipt"}, "authData": auth})
            with self.assertRaisesRegex(AttestationRejected, "unexpected DER container"):
                verify_apple_raw(untagged_obj, key_id, app_id, "production", selected,
                                 root, pin, now, self.openssl,
                                 expected_validation_category=None, expected_bundle_version=None)
            distribution = {"apple_bundle_version_01": "27", "apple_validation_category_01": (2).to_bytes(4, "little")}
            # Apple's current sample carries this map with the ED flag clear.
            extended_auth = auth + cbor(distribution)
            extended_nonce = hashlib.sha256(extended_auth + hashlib.sha256(selected.transcript()).digest()).digest()
            extended_leaf, _ = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(extended_nonce))))
            self.assertIn(APPLE_NONCE_OID, certificate_key_extensions(extended_leaf)[1])
            extended_obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [extended_leaf, root], "receipt": b"synthetic receipt"}, "authData": extended_auth})
            extended_result = verify_apple_raw(extended_obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                               expected_validation_category=2, expected_bundle_version="27")
            self.assertEqual((extended_result.apple_validation_category, extended_result.apple_bundle_version), (2, "27"))
            flagged_auth = extended_auth[:32] + b"\xc0" + extended_auth[33:]
            flagged_nonce = hashlib.sha256(flagged_auth + hashlib.sha256(selected.transcript()).digest()).digest()
            flagged_leaf, _ = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(flagged_nonce))))
            flagged_obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [flagged_leaf, root], "receipt": b"synthetic receipt"}, "authData": flagged_auth})
            self.assertEqual(verify_apple_raw(flagged_obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                              expected_validation_category=2, expected_bundle_version="27").apple_bundle_version, "27")
            for category, version in ((3, "27"), (2, "26")):
                with self.subTest(category=category, version=version), self.assertRaises(AttestationRejected):
                    verify_apple_raw(extended_obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                     expected_validation_category=category, expected_bundle_version=version)
            with self.assertRaises(AttestationRejected):
                verify_apple_raw(obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                 expected_validation_category=2, expected_bundle_version="27")
            wrong_key_id = Selection(selected.client_nonce, selected.server_nonce, selected.release_id,
                                        selected.hardware_profile_id, b"\x09" * 32, selected.lane_id)
            wrong_nonce = hashlib.sha256(auth + hashlib.sha256(wrong_key_id.transcript()).digest()).digest()
            wrong_leaf, _ = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(wrong_nonce))))
            wrong_obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [wrong_leaf, root], "receipt": b"synthetic receipt"}, "authData": auth})
            with self.assertRaisesRegex(AttestationRejected, "prepared key ID"):
                verify_apple_raw(wrong_obj, key_id, app_id, "production", wrong_key_id, root, pin, now, self.openssl,
                                 expected_validation_category=None, expected_bundle_version=None)
            for wrong_extensions in (
                {"apple_bundle_version_01": "27", "apple_validation_category_01": b"\0\0\0\x02"},
                {"apple_bundle_version_01": "27", "apple_validation_category_01": (0).to_bytes(4, "little")},
                {"apple_bundle_version_01": "27"},
                {"apple_bundle_version_01": "27", "apple_validation_category_01": (2).to_bytes(4, "little"), "unknown": 1},
            ):
                wrong_auth = auth + cbor(wrong_extensions)
                wrong_nonce = hashlib.sha256(wrong_auth + hashlib.sha256(selected.transcript()).digest()).digest()
                wrong_leaf, _ = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(wrong_nonce))))
                wrong_obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [wrong_leaf, root], "receipt": b"synthetic receipt"}, "authData": wrong_auth})
                with self.subTest(wrong_extensions=wrong_extensions), self.assertRaises(AttestationRejected):
                    verify_apple_raw(wrong_obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                     expected_validation_category=2, expected_bundle_version="27")
            unknown_auth = auth + cbor({"unknown": 1})
            unknown_nonce = hashlib.sha256(unknown_auth + hashlib.sha256(selected.transcript()).digest()).digest()
            unknown_leaf, _ = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(unknown_nonce))))
            unknown_obj = cbor({"fmt": "apple-appattest", "attStmt": {"x5c": [unknown_leaf, root], "receipt": b"synthetic receipt"}, "authData": unknown_auth})
            with self.assertRaises(AttestationRejected):
                verify_apple_raw(unknown_obj, key_id, app_id, "production", selected, root, pin, now, self.openssl,
                                 expected_validation_category=None, expected_bundle_version=None)
            for changed in (
                (obj, key_id, "TEAMID.other.app", "production", selected, root, pin),
                (obj, key_id, app_id, "development", selected, root, pin),
                (obj, b"\x07" * 32, app_id, "production", selected, root, pin),
                (obj, key_id, app_id, "production", Selection(selected.client_nonce, b"\x08" * 32, *list(vars(selected).values())[2:]), root, pin),
                (obj, key_id, app_id, "production", selected, root, b"\x09" * 32),
            ):
                with self.subTest(changed=changed[2:4]), self.assertRaises(AttestationRejected):
                    verify_apple_raw(*changed, now, self.openssl,
                                     expected_validation_category=None, expected_bundle_version=None)

    def test_signed_android_strongbox_approval_boot_and_app_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce, original.release_id,
                                 original.hardware_profile_id, b"\0" * 32, original.lane_id)
            package_name, package_version = "org.example.wallet", 7
            signer = b"\x71" * 32
            description = keymint_description(selected, package_name, package_version, signer)
            leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, description)
            now = int(time.time() * 1000) + 60_000
            pin = hashlib.sha256(root).digest()
            arguments = ([leaf, root], selected, package_name, package_version, signer,
                         root, pin, now, self.openssl)
            result = verify_android_raw(*arguments, allowed_security_levels=frozenset({2}))
            self.assertEqual(result.platform, "android_keymint")
            self.assertEqual(result.android_security_level, 2)
            self.assertEqual(result.device_key_reference, device_key_reference(fixture.point))
            self.assertEqual(result.evidence_sha256,
                             hashlib.sha256(encode_android_chain([leaf, root])).digest())
            # Pixel 6's ordinary StrongBox key has neither rollback tag303 nor
            # usageCount tag405. The positive vector above deliberately omits
            # both and performs actual certificate signature verification.
            for changes, message in (
                ({"security_level": 1, "keymint_security_level": 1}, "authenticated hardware policy"),
                ({"security_level": 0, "keymint_security_level": 0}, "authenticated hardware policy"),
                ({"keymint_security_level": 1}, "authenticated hardware policy"),
                ({"usage_count": 1}, "persistent app approval"),
                ({"usage_count": 1000}, "persistent app approval"),
                ({"software_usage_count": 1}, "software-enforced"),
                ({"purpose": 3}, "P-256 SIGN/SHA-256"),
                ({"algorithm": 1}, "P-256 SIGN/SHA-256"),
                ({"key_size": 384}, "P-256 SIGN/SHA-256"),
                ({"digest": 0}, "P-256 SIGN/SHA-256"),
                ({"curve": 2}, "P-256 SIGN/SHA-256"),
                ({"origin": 2}, "P-256 SIGN/SHA-256"),
                ({"software_key_size": True}, "software-enforced"),
                ({"device_locked": False}, "boot state is not locked"),
                ({"verified_boot_state": 1}, "boot state is not locked"),
            ):
                changed = keymint_description(selected, package_name,
                                              package_version, signer, **changes)
                changed_leaf, _ = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, changed)
                with self.subTest(changes=changes), self.assertRaisesRegex(
                    AttestationRejected, message
                ):
                    verify_android_raw([changed_leaf, root], selected, package_name,
                                       package_version, signer, root, pin, now, self.openssl,
                                       allowed_security_levels=frozenset({2}))
            optional_rollback_leaf, _ = fixture.sign(
                ANDROID_KEY_DESCRIPTION_OID,
                keymint_description(selected, package_name, package_version,
                                    signer, rollback_resistant=True),
            )
            self.assertEqual(verify_android_raw(
                [optional_rollback_leaf, root], selected, package_name,
                package_version, signer, root, pin, now, self.openssl,
                allowed_security_levels=frozenset({2}),
            ).attested_public_key_sec1, fixture.point)
            for boot_hash in (b"\x52", b"\x52" * 31, b"\x52" * 33):
                malformed = keymint_description(
                    selected, package_name, package_version, signer,
                    verified_boot_hash=boot_hash,
                )
                malformed_leaf, _ = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, malformed)
                with self.subTest(boot_hash_length=len(boot_hash)), self.assertRaisesRegex(
                    AttestationRejected, "boot state is not locked and verified"
                ):
                    verify_android_raw(
                        [malformed_leaf, root], selected, package_name,
                        package_version, signer, root, pin, now, self.openssl,
                        allowed_security_levels=frozenset({2}),
                    )
            wrong_key_id = Selection(selected.client_nonce, selected.server_nonce, selected.release_id,
                                        selected.hardware_profile_id, b"\x09" * 32, selected.lane_id)
            wrong_arguments = ([leaf, root], wrong_key_id, package_name, package_version,
                               signer, root, pin, now, self.openssl)
            with self.assertRaisesRegex(AttestationRejected, "empty-key sentinel"):
                verify_android_raw(*wrong_arguments, allowed_security_levels=frozenset({2}))
            for index, changed in (
                (1, Selection(selected.client_nonce, b"\x08" * 32, *list(vars(selected).values())[2:])),
                (2, "org.example.other"),
                (3, package_version + 1),
                (4, b"\x72" * 32),
                (6, b"\x09" * 32),
            ):
                mutated = list(arguments)
                mutated[index] = changed
                with self.subTest(index=index), self.assertRaises(AttestationRejected):
                    verify_android_raw(*mutated, allowed_security_levels=frozenset({2}))

    def test_signed_tee_requires_explicit_hardware_policy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce,
                                 original.release_id, original.hardware_profile_id,
                                 b"\0" * 32, original.lane_id)
            package_name, package_version, signer = "org.example.wallet", 7, b"\x71" * 32
            now = int(time.time() * 1000) + 60_000
            for level in (1, 2, 0):
                leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID,
                    keymint_description(selected, package_name, package_version,
                                        signer, security_level=level,
                                        keymint_security_level=level))
                arguments = ([leaf, root], selected, package_name, package_version,
                             signer, root, hashlib.sha256(root).digest(), now, self.openssl)
                if level in (1, 2):
                    proof = verify_android_raw(*arguments,
                                               allowed_security_levels=frozenset({1, 2}))
                    self.assertEqual(proof.android_security_level, level)
                    self.assertEqual(proof.attested_public_key_sec1, fixture.point)
                else:
                    with self.assertRaisesRegex(AttestationRejected, "authenticated hardware policy"):
                        verify_android_raw(*arguments, allowed_security_levels=frozenset({1, 2}))
                if level == 1:
                    with self.assertRaisesRegex(AttestationRejected, "authenticated hardware policy"):
                        verify_android_raw(*arguments, allowed_security_levels=frozenset({2}))
                for invalid in (frozenset(), frozenset({0}), frozenset({3}),
                                frozenset({True}), {1, 2}, None):
                    with self.subTest(policy=invalid), self.assertRaisesRegex(
                        AttestationRejected, "security-level selection"
                    ):
                        verify_android_raw(*arguments, allowed_security_levels=invalid)

    def test_ordinary_persistent_android_accepts_original_keymaster_and_keymint_schemas(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce,
                                 original.release_id, original.hardware_profile_id,
                                 b"\0" * 32, original.lane_id)
            package_name, package_version, signer = "org.example.wallet", 7, b"\x71" * 32
            now = int(time.time() * 1000) + 60_000
            pairs = [(2, 3), (3, 4), (4, 41)] + [(version, version) for version in (100, 200, 300, 400, 500)]
            for version, keymaster in pairs:
                levels = (1,) if version == 2 else (1, 2)
                for level in levels:
                    with self.subTest(version=version, keymaster=keymaster, level=level):
                        description = keymint_description(
                            selected, package_name, package_version, signer,
                            attestation_version=version, keymaster_version=keymaster,
                            security_level=level, keymint_security_level=level,
                            verified_boot_hash=None if version == 2 else b"\x52" * 32,
                            legacy_rollback_resistant=version == 2,
                        )
                        leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, description)
                        proof = verify_android_persistent_app_key_raw(
                            [leaf, root], selected, package_name, package_version, signer,
                            root, hashlib.sha256(root).digest(), now, self.openssl,
                            allowed_security_levels=frozenset({1, 2}),
                        )
                        self.assertEqual(proof.android_security_level, level)
                        self.assertEqual(proof.android_patch_levels, AndroidPatchLevels(
                            version, None, None, 20260805 if version >= 3 else None, None))
                        self.assertEqual(proof.attested_public_key_sec1, fixture.point)
                        self.assertEqual(proof.evidence_sha256,
                                         hashlib.sha256(encode_android_chain([leaf, root])).digest())
                        if version < 100:
                            with self.assertRaisesRegex(AttestationRejected, "authenticated hardware policy"):
                                verify_android_raw(
                                    [leaf, root], selected, package_name, package_version, signer,
                                    root, hashlib.sha256(root).digest(), now, self.openssl,
                                    allowed_security_levels=frozenset({1, 2}),
                                )

    def test_signed_hardware_patch_levels_feed_the_patch_policy_fact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce,
                                 original.release_id, original.hardware_profile_id,
                                 b"\0" * 32, original.lane_id)
            package_name, package_version, signer = "org.example.wallet", 7, b"\x71" * 32
            now = int(time.time() * 1000) + 60_000

            def verified(**levels) -> AndroidPatchLevels:
                description = keymint_description(selected, package_name, package_version,
                                                  signer, **levels)
                leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, description)
                proof = verify_android_persistent_app_key_raw(
                    [leaf, root], selected, package_name, package_version, signer,
                    root, hashlib.sha256(root).digest(), now, self.openssl,
                    allowed_security_levels=frozenset({2}))
                return proof.android_patch_levels

            levels = verified(os_version=150000, os_patch_level=202609,
                              vendor_patch_level=20260905, boot_patch_level=20260901)
            self.assertEqual(levels, AndroidPatchLevels(300, 150000, 202609, 20260905, 20260901))
            self.assertIs(android_patch_policy_met(levels, 202609), True)
            self.assertIs(android_patch_policy_met(levels, 202610), False)
            stale_boot = verified(os_patch_level=202609, vendor_patch_level=20260905,
                                  boot_patch_level=20250101)
            self.assertIs(android_patch_policy_met(stale_boot, 202609), False)
            # A software-enforced OS patch level never stands in for the
            # hardware-enforced value.
            software_only = verified(software_os_patch_level=202609)
            self.assertEqual(software_only, AndroidPatchLevels(300, None, None, 20260805, None))
            self.assertIs(android_patch_policy_met(software_only, 202601), False)

    def test_ordinary_persistent_legacy_preserves_signed_identity_and_hardware_checks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce,
                                 original.release_id, original.hardware_profile_id,
                                 b"\0" * 32, original.lane_id)
            package_name, package_version, signer = "org.example.wallet", 7, b"\x71" * 32
            now = int(time.time() * 1000) + 60_000
            base = dict(attestation_version=2, keymaster_version=3,
                        security_level=1, keymint_security_level=1, verified_boot_hash=None)
            leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID,
                keymint_description(selected, package_name, package_version, signer, **base))
            arguments = ([leaf, root], selected, package_name, package_version, signer,
                         root, hashlib.sha256(root).digest(), now, self.openssl)
            with self.assertRaisesRegex(AttestationRejected, "authenticated hardware policy"):
                verify_android_persistent_app_key_raw(*arguments, allowed_security_levels=frozenset({2}))
            for changes in (
                {"device_locked": False}, {"verified_boot_state": 1},
                {"verified_boot_hash": b"\x52" * 32}, {"purpose": 3},
                {"algorithm": 1}, {"key_size": 384}, {"digest": 0},
                {"curve": 2}, {"origin": 2}, {"software_key_size": True},
                {"usage_count": 1}, {"software_usage_count": 1},
            ):
                with self.subTest(changes=changes):
                    changed_leaf, _ = fixture.sign(ANDROID_KEY_DESCRIPTION_OID,
                        keymint_description(selected, package_name, package_version, signer,
                                            **(base | changes)))
                    with self.assertRaises(AttestationRejected):
                        verify_android_persistent_app_key_raw(
                            [changed_leaf, root], *arguments[1:], allowed_security_levels=frozenset({1, 2}))
            for index, changed in (
                (1, Selection(selected.client_nonce, b"\x08" * 32, selected.release_id,
                              selected.hardware_profile_id, selected.attested_key_id, selected.lane_id)),
                (2, "org.example.other"), (3, package_version + 1),
                (4, b"\x72" * 32), (6, b"\x09" * 32),
                (0, [leaf[:-1] + bytes([leaf[-1] ^ 1]), root]),
            ):
                with self.subTest(index=index), self.assertRaises(AttestationRejected):
                    mutated = list(arguments)
                    mutated[index] = changed
                    verify_android_persistent_app_key_raw(*mutated, allowed_security_levels=frozenset({1, 2}))
            fields = children(der_one(keymint_description(
                selected, package_name, package_version, signer, **base)))
            no_app_binding = sequence(
                integer(2), integer(1, b"\x0a"), integer(3), integer(1, b"\x0a"),
                octets(hashlib.sha256(selected.transcript()).digest()), octets(b""),
                sequence(), der(b"\x30", fields[7].value),
            )
            missing_app_leaf, _ = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, no_app_binding)
            with self.assertRaises(AttestationRejected):
                verify_android_persistent_app_key_raw(
                    [missing_app_leaf, root], *arguments[1:], allowed_security_levels=frozenset({1, 2}))

    def test_ordinary_persistent_android_rejects_unknown_versions_and_wrong_boot_schema(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce,
                                 original.release_id, original.hardware_profile_id,
                                 b"\0" * 32, original.lane_id)
            package_name, package_version, signer = "org.example.wallet", 7, b"\x71" * 32
            now = int(time.time() * 1000) + 60_000
            rejected = [dict(attestation_version=v, keymaster_version=k)
                        for v, k in ((1, 2), (2, 4), (3, 3), (4, 4), (100, 4), (600, 600))]
            rejected.extend((
                dict(attestation_version=2, keymaster_version=3, security_level=2,
                     keymint_security_level=2, verified_boot_hash=None),
                dict(attestation_version=2, keymaster_version=3, security_level=1,
                     keymint_security_level=0, verified_boot_hash=None),
                dict(attestation_version=3, keymaster_version=4, verified_boot_hash=None),
                dict(attestation_version=4, keymaster_version=41, verified_boot_hash=b"\x52" * 31),
                dict(verified_boot_hash=b"\0" * 32),
            ))
            for changes in rejected:
                with self.subTest(changes=changes):
                    leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID,
                        keymint_description(selected, package_name, package_version, signer, **changes))
                    with self.assertRaises(AttestationRejected):
                        verify_android_persistent_app_key_raw(
                            [leaf, root], selected, package_name, package_version, signer,
                            root, hashlib.sha256(root).digest(), now, self.openssl,
                            allowed_security_levels=frozenset({1, 2}),
                        )

    def test_android_uses_root_nearest_extension_and_exact_chain_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            fixture = SignedEnvelope(directory, self.openssl)
            original = selection(fixture.point)
            selected = Selection(original.client_nonce, original.server_nonce,
                                 original.release_id, original.hardware_profile_id,
                                 b"\0" * 32, original.lane_id)
            package_name, package_version, signer = "org.example.wallet", 7, b"\x71" * 32
            description = keymint_description(selected, package_name, package_version,
                                              signer)
            extension_der = ":".join(f"{byte:02X}" for byte in description)
            (directory / "attested.cnf").write_text(
                "basicConstraints=critical,CA:TRUE\n"
                "keyUsage=critical,digitalSignature,keyCertSign,cRLSign\n"
                f"{ANDROID_KEY_DESCRIPTION_OID}=DER:{extension_der}\n"
            )
            fixture.run("x509", "-req", "-in", "leaf.csr", "-CA", "root.pem",
                        "-CAkey", "root.key", "-CAcreateserial", "-out", "attested.pem",
                        "-days", "2", "-extfile", "attested.cnf")
            fixture.run("x509", "-in", "attested.pem", "-outform", "DER",
                        "-out", "attested.der")
            fixture.run("x509", "-in", "root.pem", "-outform", "DER", "-out", "root.der")
            attested = (directory / "attested.der").read_bytes()
            root = (directory / "root.der").read_bytes()

            fixture.run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                        "-nodes", "-keyout", "forged.key", "-out", "forged.csr",
                        "-subj", "/CN=Attacker Added Leaf")
            (directory / "forged.cnf").write_text(
                "basicConstraints=critical,CA:FALSE\n"
                f"{ANDROID_KEY_DESCRIPTION_OID}=DER:04:03:62:61:64\n"
            )
            fixture.run("x509", "-req", "-in", "forged.csr", "-CA", "attested.pem",
                        "-CAkey", "leaf.key", "-CAcreateserial", "-out", "forged.pem",
                        "-days", "2", "-extfile", "forged.cnf")
            fixture.run("x509", "-in", "forged.pem", "-outform", "DER",
                        "-out", "forged.der")
            forged = (directory / "forged.der").read_bytes()
            self.assertNotEqual(certificate_key_extensions(forged)[0], fixture.point)
            now = int(time.time() * 1000) + 60_000
            pin = hashlib.sha256(root).digest()
            result = verify_android_raw(
                [attested, root], selected, package_name, package_version,
                signer, root, pin, now, self.openssl,
                allowed_security_levels=frozenset({2}),
            )
            self.assertEqual(result.attested_public_key_sec1, fixture.point)
            self.assertEqual(result.device_key_reference,
                             device_key_reference(fixture.point))
            for extended in ([forged, attested, root], [forged, attested]):
                with self.assertRaisesRegex(AttestationRejected,
                                            "app-controlled leaf key"):
                    verify_android_raw(
                        extended, selected, package_name, package_version,
                        signer, root, pin, now, self.openssl,
                        allowed_security_levels=frozenset({2}),
                    )

            # OpenSSL can build a valid path while ignoring an unrelated member
            # of its -untrusted pool. The verifier must reject that input list.
            fixture.run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256",
                        "-nodes", "-keyout", "extra.key", "-out", "extra.csr",
                        "-subj", "/CN=Unrelated Certificate")
            (directory / "extra.cnf").write_text(
                "basicConstraints=critical,CA:TRUE\n"
                f"{ANDROID_KEY_DESCRIPTION_OID}=DER:{extension_der}\n"
            )
            fixture.run("x509", "-req", "-in", "extra.csr", "-CA", "root.pem",
                        "-CAkey", "root.key", "-CAcreateserial", "-out", "extra.pem",
                        "-days", "2", "-extfile", "extra.cnf")
            fixture.run("x509", "-in", "extra.pem", "-outform", "DER",
                        "-out", "extra.der")
            extra = (directory / "extra.der").read_bytes()
            with self.assertRaisesRegex(AttestationRejected, "order or link"):
                verify_android_raw(
                    [forged, attested, extra, root], selected, package_name,
                    package_version, signer, root, pin, now, self.openssl,
                    allowed_security_levels=frozenset({2}),
                )
            with self.assertRaisesRegex(AttestationRejected, "misplaced certificate"):
                verify_android_raw(
                    [forged, attested, root, extra], selected, package_name,
                    package_version, signer, root, pin, now, self.openssl,
                    allowed_security_levels=frozenset({2}),
                )


if __name__ == "__main__":
    unittest.main()
