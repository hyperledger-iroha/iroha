"""NEW policy DATA vectors/controls. No policy approval, platform or issuer qualification."""
import hashlib
import json
import unittest
from dataclasses import replace
from pathlib import Path

from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.wallet_policy import (
    AndroidAppIdentityV1, AndroidEnrollmentPlatformV1, AppleAppIdentityV1, AppleEnrollmentPlatformV1,
    ConfiguredWalletEnrollmentPolicyV1, RegulatoryPolicyV1, WalletAppPolicyV1, WalletEnrollmentPolicyV1,
)


def policies(apple=False):
    app = WalletAppPolicyV1(1, b"\1" * 32, AppleAppIdentityV1("TEAMID.org.example.wallet") if apple else
                           AndroidAppIdentityV1("org.example.wallet", 7, b"\5" * 32))
    platform = AppleEnrollmentPlatformV1(b"\4" * 32) if apple else AndroidEnrollmentPlatformV1(
        b"\4" * 32, 3, 202608, 120000, True, True, 1)
    regulator = RegulatoryPolicyV1(4, 0, 5000) if apple else RegulatoryPolicyV1(0, 0, 0)
    enrollment = WalletEnrollmentPolicyV1(1, b"\1" * 32, b"\2" * 32, app.policy_digest(), platform,
        regulator, 120000, 3600000 if apple else 0)
    return app, enrollment


def challenge(app, enrollment):
    return (b"\1\0" + enrollment.scheme_id + enrollment.asset_digest + b"\3" * 32
            + app.policy_digest() + enrollment.policy_digest() + b"\6" * 32)


class WalletPolicyTests(unittest.TestCase):
    def test_shared_exact_new_transcripts_and_digests(self):
        vectors = Path(__file__).resolve().parents[3] / "fixtures/kagemusha/wallet_enrollment_policy_v1_vectors.json"
        document = json.loads(vectors.read_text())
        self.assertIn("unadmitted DATA", document["status"])
        self.assertEqual(len(document["cases"]), 2)
        for apple, row in zip((False, True), document["cases"]):
            app, enrollment = policies(apple)
            enrollment.validate_for_app(app)
            e1 = challenge(app, enrollment)
            for key, value in (
                ("app_policy_transcript_hex", app.transcript()), ("app_policy_digest_hex", app.policy_digest()),
                ("enrollment_policy_transcript_hex", enrollment.transcript()),
                ("enrollment_policy_digest_hex", enrollment.policy_digest()), ("challenge_transcript_hex", e1),
                ("challenge_digest_hex", hashlib.sha256(b"iroha:kagemusha:wallet:v1:enrollment-challenge\0"
                    + len(e1).to_bytes(8, "little") + e1).digest()),
            ):
                self.assertEqual(value.hex(), row[key])

    def test_policy_and_private_root_substitution_rejected_before_platform_call(self):
        app, enrollment = policies()
        raw = b"UNADMITTED DATA root consistency bytes; not a certificate or authority"
        selected = replace(enrollment.platform, attestation_root_sha256=hashlib.sha256(raw).digest())
        enrollment = replace(enrollment, platform=selected)
        configured = ConfiguredWalletEnrollmentPolicyV1(app, enrollment, raw)
        e1 = challenge(app, enrollment)
        configured.validate_scope(e1, 10, 10)
        for index in (2, 34, 98, 130):
            value = bytearray(e1); value[index] ^= 1
            with self.assertRaises(AttestationRejected):
                configured.validate_scope(bytes(value), 10, 10)
        for root in (b"", raw + b"x"):
            with self.assertRaises(AttestationRejected):
                replace(configured, attestation_root_der=root).validate_scope(e1, 10, 10)
        huge = b"x" * (16384 + 1)
        huge_enrollment = replace(enrollment, platform=replace(selected, attestation_root_sha256=hashlib.sha256(huge).digest()))
        with self.assertRaises(AttestationRejected):
            ConfiguredWalletEnrollmentPolicyV1(app, huge_enrollment, huge).validate_scope(
                challenge(app, huge_enrollment), 10, 10)
        with self.assertRaises(AttestationRejected):
            enrollment.validate_for_app(policies(True)[0])
        with self.assertRaises(AttestationRejected):
            replace(enrollment, platform=AppleEnrollmentPlatformV1(b"\4" * 32)).validate_for_app(app)

    def test_no_implicit_time_or_lease_and_all_overflow_boundaries(self):
        _, enrollment = policies()
        enrollment.require_live_challenge(10, 10)
        enrollment.require_live_challenge(10, 120009)
        for created, now in ((0, 1), (10, 9), (10, 120010), ((1 << 64) - 1, (1 << 64) - 1)):
            with self.assertRaises(AttestationRejected):
                enrollment.require_live_challenge(created, now)
        self.assertEqual(enrollment.lease_expires_at(10), 0)
        with self.assertRaises(AttestationRejected):
            enrollment.lease_expires_at(0)
        _, apple = policies(True)
        self.assertEqual(apple.lease_expires_at(10), 3600010)
        for candidate in (replace(apple, attestation_lease_lifetime_ms=0),
                          replace(enrollment, attestation_lease_lifetime_ms=1),
                          replace(enrollment, challenge_lifetime_ms=0),
                          replace(enrollment, challenge_lifetime_ms=119999),
                          replace(enrollment, regulatory_policy=RegulatoryPolicyV1(8, 0, 0))):
            with self.assertRaises(AttestationRejected):
                candidate.transcript()
        with self.assertRaises(AttestationRejected):
            apple.lease_expires_at((1 << 64) - 1)

    def test_current_app_byte_grammar_exact_without_normalization(self):
        app, _ = policies()
        for name in ("", "one", "a..b", "1a.b", "org.exa-mple", "org.é", "a." + "b" * 254):
            with self.assertRaises(AttestationRejected):
                replace(app, identity=replace(app.identity, package_name=name)).transcript()
        for version in (-1, 1 << 64, True):
            with self.assertRaises(AttestationRejected):
                replace(app, identity=replace(app.identity, package_version=version)).transcript()
        apple, _ = policies(True)
        for name in ("", "é" * 128, "\ud800"):
            with self.assertRaises(AttestationRejected):
                replace(apple, identity=AppleAppIdentityV1(name)).transcript()
        replace(apple, identity=AppleAppIdentityV1("é" * 127 + "a")).transcript()
        self.assertNotEqual(replace(apple, identity=AppleAppIdentityV1("é")).policy_digest(),
                            replace(apple, identity=AppleAppIdentityV1("e\u0301")).policy_digest())

    def test_current_google_projection_has_explicit_new_digest_and_finite_selectors(self):
        app, enrollment = policies()
        configured = ConfiguredWalletEnrollmentPolicyV1(app, enrollment, b"DATA only")
        projection = configured.play_integrity_policy()
        self.assertEqual(projection.policy_digest, enrollment.policy_digest())
        self.assertEqual(projection.package_name, app.identity.package_name)
        self.assertFalse(hasattr(projection, "maximum_refresh_interval_ms"))
        for tag, levels in ((1, {1}), (2, {2}), (3, {1, 2})):
            selected = replace(enrollment.platform, hardware_tag=tag, require_play_recognized=False,
                               require_licensed=False, minimum_device_integrity_tag=2)
            self.assertEqual(selected.allowed_security_levels(), frozenset(levels))
            changed = replace(configured, enrollment=replace(enrollment, platform=selected)).play_integrity_policy()
            self.assertEqual(changed.minimum_device_integrity, "MEETS_STRONG_INTEGRITY")
        for selected in (replace(enrollment.platform, hardware_tag=0),
                         replace(enrollment.platform, hardware_tag=True),
                         replace(enrollment.platform, minimum_device_integrity_tag=0),
                         replace(enrollment.platform, require_licensed=1),
                         replace(enrollment.platform, play_integrity_maximum_age_ms=0),
                         replace(enrollment.platform, patch_floor_yyyymm=202613)):
            with self.assertRaises(AttestationRejected):
                selected.transcript()
