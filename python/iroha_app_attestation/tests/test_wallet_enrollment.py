"""Current E1 vectors and genuinely signed synthetic X509/AppAttest regressions.

Synthetic roots and scripted server Google responses qualify component code only.
They are never physical-device, distributed app, production issuer or deployment evidence.
"""
import hashlib
from dataclasses import replace
import os
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import (
    ANDROID_KEY_DESCRIPTION_OID, APPLE_NONCE_OID, AttestationRejected,
    DurableAppleAssertionCounterStore, Selection,
    verify_android_wallet_payment_key_raw, verify_apple_wallet_attestation_raw,
    verify_apple_wallet_enrollment_assertion,
)
from iroha_app_attestation.play_integrity import (
    DecodedPlayIntegrityEvidence, GooglePlayIntegrityVerifier,
    PlayIntegrityEnrollmentPolicy, PlayIntegrityProof,
)
from iroha_app_attestation.native_time_interval import NativeTimeInterval
from iroha_app_attestation.wallet_enrollment import (
    VerifiedWalletEnrollmentEvidence, WalletEnrollmentScope,
    verify_android_wallet_enrollment, verify_apple_wallet_enrollment,
)
from iroha_app_attestation.wallet_policy import (
    AndroidAppIdentityV1, AndroidEnrollmentPlatformV1, AppleAppIdentityV1, AppleEnrollmentPlatformV1,
    ConfiguredWalletEnrollmentPolicyV1, RegulatoryPolicyV1, WalletAppPolicyV1, WalletEnrollmentPolicyV1,
)
from test_synthetic_platform_evidence import (
    SignedEnvelope, cbor, explicit, keymint_description, octets, sequence,
)

GENERATOR = bytes.fromhex(
    "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5")


def scope(point: bytes) -> WalletEnrollmentScope:
    return WalletEnrollmentScope(b"\1\0" + b"".join(bytes([i]) * 32 for i in range(1, 7)), point)


def configured_policy(root: bytes, apple: bool) -> ConfiguredWalletEnrollmentPolicyV1:
    """Synthetic component selection only; no operator approval or installed policy."""
    app = WalletAppPolicyV1(1, b"\1" * 32, AppleAppIdentityV1("TEAMID.example.app") if apple else
                           AndroidAppIdentityV1("org.example.wallet", 7, b"\x71" * 32))
    pin = hashlib.sha256(root).digest()
    platform = AppleEnrollmentPlatformV1(pin) if apple else AndroidEnrollmentPlatformV1(
        pin, 3, 202608, 120_000, True, True, 1)
    enrollment = WalletEnrollmentPolicyV1(1, b"\1" * 32, b"\2" * 32, app.policy_digest(),
        platform, RegulatoryPolicyV1(0, 0, 0), 120_000, 0)
    return ConfiguredWalletEnrollmentPolicyV1(app, enrollment, root)


def policy_scope(policy: ConfiguredWalletEnrollmentPolicyV1, point: bytes) -> WalletEnrollmentScope:
    return WalletEnrollmentScope(b"\1\0" + policy.enrollment.scheme_id + policy.enrollment.asset_digest
        + b"\3" * 32 + policy.app.policy_digest() + policy.enrollment.policy_digest() + b"\6" * 32, point)


def synthetic_root(fixture: SignedEnvelope) -> bytes:
    fixture.run("x509", "-in", "root.pem", "-outform", "DER", "-out", "root.der")
    return (fixture.directory / "root.der").read_bytes()


def assertion(fixture: SignedEnvelope, client_data_hash: bytes, counter: int) -> bytes:
    app_id = "TEAMID.example.app"
    auth = hashlib.sha256(app_id.encode()).digest() + b"\x40" + counter.to_bytes(4, "big")
    (fixture.directory / "assertion-message.bin").write_bytes(
        hashlib.sha256(auth + client_data_hash).digest())
    fixture.run("dgst", "-sha256", "-sign", "leaf.key", "-out", "assertion-signature.bin",
                "assertion-message.bin")
    return cbor({"signature": (fixture.directory / "assertion-signature.bin").read_bytes(),
                 "authenticatorData": auth})


def apple_object(fixture: SignedEnvelope, selected: WalletEnrollmentScope):
    app_id = "TEAMID.example.app"
    key_id = hashlib.sha256(fixture.point).digest()
    cose = cbor({1: 2, 3: -7, -1: 1, -2: fixture.point[1:33], -3: fixture.point[33:]})
    auth = (hashlib.sha256(app_id.encode()).digest() + b"\x40" + b"\0" * 4
            + b"appattest" + b"\0" * 7 + b"\0\x20" + key_id + cose)
    nonce = hashlib.sha256(auth + selected.challenge_digest()).digest()
    leaf, root = fixture.sign(APPLE_NONCE_OID, sequence(explicit(1, octets(nonce))))
    obj = cbor({"fmt": "apple-appattest",
                "attStmt": {"x5c": [leaf, root], "receipt": b"synthetic receipt"},
                "authData": auth})
    return obj, key_id, root


class WalletEnrollmentTests(unittest.TestCase):
    def setUp(self):
        binary = shutil.which("openssl")
        if binary is None:
            self.skipTest("OpenSSL3 unavailable")
        self.openssl = Path(binary).resolve()

    def test_e1_fixed_current_hashes_and_original_evidence_items(self):
        selected = scope(GENERATOR)
        self.assertEqual(selected.challenge_digest().hex(),
                         "9ee9b71a3026e333f8c69d1c5f338bdf8639fbd85644730be5bf5a3bdaf53682")
        self.assertEqual(selected.enrollment_key_binding().hex(),
                         "b4129d2396a72c9288160f28f26f1fc63f6935b8512a37fd722cc2aa5859454e")
        record = VerifiedWalletEnrollmentEvidence(
            1, 1, 0, 0, 0, 0, (b"leaf-der", b"root-der", b"actual-google-response"))
        self.assertEqual(record.evidence_digest().hex(),
                         "fa104a38766fc005595e4095c244607d1203f6bba3e66cbcfd60591a889d61e0")
        for value in (b"", selected.challenge_transcript + b"x",
                      b"\2\0" + selected.challenge_transcript[2:],
                      selected.challenge_transcript[:2] + b"\0" * 32 + selected.challenge_transcript[34:]):
            with self.subTest(value=value[:2]), self.assertRaises(AttestationRejected):
                WalletEnrollmentScope(value, GENERATOR).validate()

    def test_current_android_direct_challenge_preserves_tee_and_strongbox_checks(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            selected = scope(fixture.point)
            # Existing signed fixture builder only: replace its historical challenge octets
            # with current E1 before signing. Production current paths never use Selection.
            old = Selection(*[bytes([i]) * 32 for i in range(11, 17)])
            signer = b"\x71" * 32
            for level in (1, 2):
                description = keymint_description(
                    old, "org.example.wallet", 7, signer, security_level=level,
                    keymint_security_level=level, os_patch_level=202608,
                    vendor_patch_level=20260805)
                description = description.replace(hashlib.sha256(old.transcript()).digest(),
                                                  selected.challenge_digest())
                leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, description)
                args = ([leaf, root], selected.challenge_digest(), "org.example.wallet", 7,
                        signer, root, hashlib.sha256(root).digest(),
                        int(time.time() * 1000) + 60_000, self.openssl)
                proof = verify_android_wallet_payment_key_raw(
                    *args, allowed_security_levels=frozenset({1, 2}))
                self.assertEqual(proof.android_security_level, level)
                self.assertEqual(proof.attested_public_key_sec1, fixture.point)
                for wrong in (b"\0" * 32, hashlib.sha256(selected.challenge_digest()).digest()):
                    with self.assertRaises(AttestationRejected):
                        verify_android_wallet_payment_key_raw(
                            args[0], wrong, *args[2:], allowed_security_levels=frozenset({1, 2}))
                with self.assertRaises(AttestationRejected):
                    verify_android_wallet_payment_key_raw(*args, allowed_security_levels=frozenset({0}))

    def test_current_apple_uses_prehashed_e1_and_binding_without_extra_sha(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            selected = scope(GENERATOR)  # Separate payment key; App Attest key is fixture.point.
            obj, key_id, root = apple_object(fixture, selected)
            proof = verify_apple_wallet_attestation_raw(
                obj, key_id, "TEAMID.example.app", "production", selected.challenge_digest(),
                root, hashlib.sha256(root).digest(), int(time.time() * 1000) + 60_000, self.openssl)
            self.assertEqual(proof.attested_public_key_sec1, fixture.point)
            signed = assertion(fixture, selected.enrollment_key_binding(), 1)
            args = (signed, selected.enrollment_key_binding(), fixture.point, key_id,
                    "TEAMID.example.app", 0, self.openssl)
            verified = verify_apple_wallet_enrollment_assertion(*args)
            self.assertEqual(verified.counter, 1)
            self.assertEqual(verified.client_data_sha256, selected.enrollment_key_binding())
            with self.assertRaises(AttestationRejected):
                verify_apple_wallet_enrollment_assertion(
                    signed, hashlib.sha256(selected.enrollment_key_binding()).digest(), *args[2:])
            with self.assertRaises(AttestationRejected):
                verify_apple_wallet_attestation_raw(
                    obj, key_id, "TEAMID.example.app", "production",
                    hashlib.sha256(selected.challenge_digest()).digest(), root,
                    hashlib.sha256(root).digest(), int(time.time() * 1000) + 60_000, self.openssl)

    def test_current_apple_composition_preserves_counter_and_consumed_challenge_after_restart(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            os.chmod(directory, 0o700)
            fixture = SignedEnvelope(directory, self.openssl)
            policy = configured_policy(synthetic_root(fixture), True)
            selected = policy_scope(policy, GENERATOR)
            now = int(time.time() * 1000) + 60_000
            obj, key_id, root = apple_object(fixture, selected)
            store = DurableAppleAssertionCounterStore(directory / "counters.sqlite")
            signed = assertion(fixture, selected.enrollment_key_binding(), 1)
            record = verify_apple_wallet_enrollment(
                obj, signed, key_id, selected, policy, store,
                now, self.openssl, challenge_created_at_ms=now - 1)
            self.assertEqual(record.kind_tag, 3)
            self.assertEqual(record.facts, (1 << 6) | (1 << 7) | (1 << 8))
            self.assertEqual((record.os_patch_level, record.vendor_patch_level, record.boot_patch_level), (0, 0, 0))
            self.assertEqual(record.original_items, (obj, signed))
            store = DurableAppleAssertionCounterStore(directory / "counters.sqlite")
            with self.assertRaisesRegex(AttestationRejected, "counter did not advance"):
                verify_apple_wallet_enrollment(obj, signed, key_id, selected, policy, store,
                                              now, self.openssl, challenge_created_at_ms=now - 1)
            replay = assertion(fixture, selected.enrollment_key_binding(), 2)
            with self.assertRaisesRegex(AttestationRejected, "already consumed"):
                verify_apple_wallet_enrollment(obj, replay, key_id, selected, policy, store,
                                              now, self.openssl, challenge_created_at_ms=now - 1)

    def test_current_android_separate_google_boundary_binds_same_payment_key_and_original_response(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = SignedEnvelope(Path(temporary), self.openssl)
            policy = configured_policy(synthetic_root(fixture), False)
            selected = policy_scope(policy, fixture.point)
            old = Selection(*[bytes([i]) * 32 for i in range(11, 17)])
            signer = b"\x71" * 32
            description = keymint_description(old, "org.example.wallet", 7, signer,
                                             os_patch_level=202608, vendor_patch_level=20260805)
            description = description.replace(hashlib.sha256(old.transcript()).digest(), selected.challenge_digest())
            leaf, root = fixture.sign(ANDROID_KEY_DESCRIPTION_OID, description)
            now = int(time.time() * 1000) + 60_000
            pi = policy.play_integrity_policy()
            self.assertFalse(hasattr(pi, "maximum_refresh_interval_ms"))
            google = GooglePlayIntegrityVerifier(lambda: "synthetic-scoped-server-token")
            body = b'{"tokenPayloadExternal":"scripted-server-original-for-component-test"}'
            decoded = DecodedPlayIntegrityEvidence(body, PlayIntegrityProof(
                pi.policy_digest, selected.enrollment_key_binding(), now,
                b"\x31" * 32, hashlib.sha256(body).digest(), "PLAY_RECOGNIZED", "LICENSED",
                ("MEETS_DEVICE_INTEGRITY",)))
            with (patch("iroha_app_attestation.wallet_enrollment.verify_google_chain_not_revoked") as revocation,
                  patch.object(google, "decode", return_value=decoded) as decoder):
                record = verify_android_wallet_enrollment([leaf, root], "opaque-mobile-token", selected,
                                                         policy, google, now, self.openssl, challenge_created_at_ms=now - 1,
                                                         trusted_time_interval=lambda: NativeTimeInterval(now, now))
                revocation.assert_called_once_with([leaf, root])
                decoder.assert_called_once_with("opaque-mobile-token", pi, selected.enrollment_key_binding(), now)
                # Fresh interval is mandatory around each external stage. A response
                # can age out while its decoder is running without expiring the challenge.
                shorter = replace(policy, enrollment=replace(policy.enrollment,
                    platform=replace(policy.enrollment.platform, play_integrity_maximum_age_ms=1_000)))
                shorter_scope = policy_scope(shorter, fixture.point)
                # Scope changes bind the attestation, so exercise this temporal check
                # against the already selected raw verifier DATA in isolation.
                with patch("iroha_app_attestation.wallet_enrollment.verify_android_wallet_payment_key_raw") as raw:
                    raw.return_value.attested_public_key_sec1 = shorter_scope.payment_key_sec1
                    samples = iter([now, now, now, now, now + 1_001])
                    def fresh_interval():
                        current = next(samples)
                        return NativeTimeInterval(current, current)
                    with self.assertRaisesRegex(AttestationRejected, "expired during verification"):
                        verify_android_wallet_enrollment([leaf, root], "opaque-mobile-token", shorter_scope,
                            shorter, google, now, self.openssl, challenge_created_at_ms=now - 1,
                            trusted_time_interval=fresh_interval)
                self.assertEqual(record.original_items, (leaf, root, body))
                self.assertTrue(record.facts & (1 << 9))
                self.assertTrue(record.facts & (1 << 10))
                self.assertTrue(record.facts & (1 << 4))
                with self.assertRaisesRegex(AttestationRejected, "another payment key"):
                    verify_android_wallet_enrollment([leaf, root], "opaque-mobile-token", policy_scope(policy, GENERATOR),
                                                     policy, google, now, self.openssl, challenge_created_at_ms=now - 1,
                                                         trusted_time_interval=lambda: NativeTimeInterval(now, now))
