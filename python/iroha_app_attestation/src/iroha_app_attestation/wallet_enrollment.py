"""Current E1 enrollment evidence using the retained real platform verifiers.

The private Native issuer selects the scope and operator policy. Public requests contain
raw platform evidence and an opaque Play Integrity token, never roots, policies, a decoded
Google verdict, an issuer key, or a success flag. These results are evidence records, not
signing authority or monetary grants. Native DATA must retain/consume the original E1 and
issuer result before exposure; credential signing remains the Enrollment-role Native owner.
"""
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .native_time_interval import NativeTimeInterval

from .attestation import (
    DurableAppleAssertionCounterStore, RawPlatformProof, android_patch_policy_met,
    require,
    verify_android_wallet_payment_key_raw, verify_apple_wallet_attestation_raw,
)
from .play_integrity import GooglePlayIntegrityVerifier
from .wallet_policy import (
    AndroidAppIdentityV1, AndroidEnrollmentPlatformV1, AppleAppIdentityV1,
    AppleEnrollmentPlatformV1, ConfiguredWalletEnrollmentPolicyV1,
)
from .revocation import verify_google_chain_not_revoked

_PREFIX = b"iroha:kagemusha:wallet:v1:"


def _digest(role: bytes, body: bytes) -> bytes:
    return hashlib.sha256(_PREFIX + role + b"\0" + len(body).to_bytes(8, "little") + body).digest()


@dataclass(frozen=True)
class WalletEnrollmentScope:
    """Native-selected exact E1 transcript and separately validated P256 payment public key.

    The transcript is LE16 version then six nonzero32 fields: scheme, asset, canonical
    account digest, app policy, enrollment policy and issuer nonce. The Native owner checks
    its account/current activation and authenticates its configured policy before selection.
    This parser by itself creates none of that authority.
    """
    challenge_transcript: bytes
    payment_key_sec1: bytes

    def validate(self) -> None:
        value = self.challenge_transcript
        require(type(value) is bytes and len(value) == 194 and value[:2] == b"\x01\0"
                and all(any(value[offset:offset + 32]) for offset in range(2, 194, 32)),
                "invalid current E1 transcript")
        require(type(self.payment_key_sec1) is bytes and len(self.payment_key_sec1) == 65
                and self.payment_key_sec1[0] == 4, "invalid current E1 payment key")

    def challenge_digest(self) -> bytes:
        self.validate()
        return _digest(b"enrollment-challenge", self.challenge_transcript)

    def enrollment_key_binding(self) -> bytes:
        return _digest(b"enrollment-key-binding", self.challenge_digest() + self.payment_key_sec1)


@dataclass(frozen=True)
class VerifiedWalletEnrollmentEvidence:
    """Projection plus original checked items; never an HTTP or signer capability.

    Android items are the leaf-first DER chain followed by the original Google decoder TLS
    response. Apple items are the original attestation object then original assertion. Native
    must rederive the exact model evidence digest and retain these originals before signing.
    The Android composition explicitly records enrollment-time PI; it creates no PI lease.
    """
    kind_tag: int
    time_ms: int
    facts: int
    os_patch_level: int
    vendor_patch_level: int
    boot_patch_level: int
    original_items: tuple[bytes, ...]
    app_attest_key_id: bytes | None = None
    app_attest_counter: int | None = None

    def evidence_digest(self) -> bytes:
        require(self.kind_tag in (1, 2, 3) and self.original_items
                and all(type(item) is bytes and 0 < len(item) < (1 << 32)
                        for item in self.original_items), "invalid verified evidence items")
        body = bytes([self.kind_tag]) + len(self.original_items).to_bytes(4, "little")
        for item in self.original_items:
            body += len(item).to_bytes(4, "little") + item
        return _digest(b"evidence", body)


def _trusted_time(time_ms: int) -> None:
    require(type(time_ms) is int and 0 < time_ms < (1 << 64), "trusted enrollment time absent")


def verify_android_wallet_enrollment(
    chain_der: list[bytes], opaque_play_integrity_token: str,
    scope: WalletEnrollmentScope, policy: ConfiguredWalletEnrollmentPolicyV1,
    google: GooglePlayIntegrityVerifier, trusted_time_ms: int, openssl_path: Path,
    *, challenge_created_at_ms: int, trusted_time_interval: Callable[[], NativeTimeInterval],
) -> VerifiedWalletEnrollmentEvidence:
    """Independent hardware, fresh Google revocation and server-decoded PI checks.

    PI requestHash is E1's payment-key binding, matching its exact account/challenge/key scope.
    Google endpoint/OAuth custody stay inside the retained server verifier. Outages retain
    retryable VerificationUnavailable semantics; no caller verdict or software key is used.
    """
    def current_time() -> int:
        interval = trusted_time_interval()
        require(type(interval) is NativeTimeInterval, "private enrollment clock absent")
        interval.check_both(lambda now: policy.validate_scope(
            scope.challenge_transcript, challenge_created_at_ms, now))
        return interval.upper_at_ms

    scope.validate()
    _trusted_time(trusted_time_ms)
    require(type(policy) is ConfiguredWalletEnrollmentPolicyV1, "configured policy absent")
    policy.validate_scope(scope.challenge_transcript, challenge_created_at_ms, trusted_time_ms)
    app = policy.app.identity
    selected = policy.enrollment.platform
    require(type(app) is AndroidAppIdentityV1 and type(selected) is AndroidEnrollmentPlatformV1,
            "configured Android platform absent")
    require(isinstance(google, GooglePlayIntegrityVerifier), "configured Google verifier absent")
    raw: RawPlatformProof = verify_android_wallet_payment_key_raw(
        chain_der, scope.challenge_digest(), app.package_name, app.package_version,
        app.app_signing_certificate_sha256, policy.attestation_root_der,
        selected.attestation_root_sha256, current_time(), openssl_path,
        allowed_security_levels=selected.allowed_security_levels())
    require(raw.attested_public_key_sec1 == scope.payment_key_sec1,
            "KeyMint attests another payment key")
    # This first producer supports configured Google roots only. Other vendor roots need
    # their actual adapter-owned revocation contract, never a fabricated Google-clear bit.
    current_time()
    verify_google_chain_not_revoked(chain_der)
    current_time()
    integrity = google.decode(opaque_play_integrity_token, policy.play_integrity_policy(),
                              scope.enrollment_key_binding(), current_time())
    now = current_time()
    require(integrity.proof.timestamp_ms <= now
            and now - integrity.proof.timestamp_ms <= selected.play_integrity_maximum_age_ms,
            "Google evidence expired during verification")
    require(raw.android_security_level in (1, 2) and raw.android_patch_levels is not None,
            "verified Android key facts absent")
    levels = raw.android_patch_levels
    facts = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 5) | (1 << 9) | (1 << 10)
    if raw.android_security_level == 2:
        facts |= 1 << 1
    if android_patch_policy_met(levels, selected.patch_floor_yyyymm):
        facts |= 1 << 4
    patches = tuple(0 if level is None else level for level in
                    (levels.os_patch_level, levels.vendor_patch_level, levels.boot_patch_level))
    require(all(type(level) is int and 0 <= level < (1 << 32) for level in patches),
            "Android patch fields exceed credential grammar")
    return VerifiedWalletEnrollmentEvidence(
        raw.android_security_level, trusted_time_ms, facts, *patches,
        tuple(chain_der) + (integrity.google_response,))


def verify_apple_wallet_enrollment(
    attestation_object: bytes, assertion_object: bytes, app_attest_key_id: bytes,
    scope: WalletEnrollmentScope, policy: ConfiguredWalletEnrollmentPolicyV1,
    counters: DurableAppleAssertionCounterStore, trusted_time_ms: int, openssl_path: Path,
    *, challenge_created_at_ms: int,
) -> VerifiedWalletEnrollmentEvidence:
    """Production App Attest plus fresh assertion binding the separate payment public key.

    The existing durable store preserves prior App Attest keys/counters and consumes the
    key-binding challenge once. The Native issuer must durably retain the exact result and
    credential; a lost original never authorizes repeating a consumed assertion. This does
    not independently attest Secure Enclave provenance, boot, patches or jailbreak absence.
    """
    scope.validate()
    _trusted_time(trusted_time_ms)
    require(type(policy) is ConfiguredWalletEnrollmentPolicyV1, "configured policy absent")
    policy.validate_scope(scope.challenge_transcript, challenge_created_at_ms, trusted_time_ms)
    app = policy.app.identity
    selected = policy.enrollment.platform
    require(type(app) is AppleAppIdentityV1 and type(selected) is AppleEnrollmentPlatformV1,
            "configured Apple platform absent")
    require(isinstance(counters, DurableAppleAssertionCounterStore),
            "durable configured Apple counter owner absent")
    raw = verify_apple_wallet_attestation_raw(
        attestation_object, app_attest_key_id, app.app_id, "production",
        scope.challenge_digest(), policy.attestation_root_der,
        selected.attestation_root_sha256, trusted_time_ms, openssl_path)
    counters.register_verified_key(raw, app.app_id, "production")
    assertion = counters.verify_wallet_enrollment_and_advance(
        assertion_object, scope.enrollment_key_binding(), app_attest_key_id,
        app.app_id, "production", openssl_path)
    return VerifiedWalletEnrollmentEvidence(
        3, trusted_time_ms, (1 << 6) | (1 << 7) | (1 << 8), 0, 0, 0,
        (attestation_object, assertion_object), app_attest_key_id, assertion.counter)
