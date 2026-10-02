"""First-release ordinary app evidence verification under Native-selected policy.

Only authenticated deployment startup may supply policies and current-policy
rechecks. Constructing a policy does not authenticate it. This component never
accepts a release/root/decoded-Google-verdict in a mobile credential request and
never exposes signer custody to the evidence caller.
"""
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .apple_receipt import verify_apple_receipt
from .attestation import (RawPlatformProof, cbor_exact, decode_android_chain, fixed32,
                          require, verify_android_persistent_app_key_raw, verify_apple_raw)
from .ordinary_enrollment import (EnrollmentPossession, OrdinaryEnrollmentChallenge,
    OrdinaryPlatformEvidenceChallenge, authenticate_challenge_transport,
    credential_signing_request, decode_challenge_transport, verify_enrollment_possession)
from .play_integrity import (GooglePlayIntegrityVerifier, PlayIntegrityPolicy,
                             PlayIntegrityProof, _verify_google_payload)
from .provider import (AppleAppPolicy, GoogleKeyMintPolicy, OemKeyMintPolicy,
                       _apple_release_digest)
from .revocation import verify_google_chain_not_revoked


@dataclass(frozen=True)
class OrdinaryRawAttestationRequest:
    """Exact authenticated C/key/raw originals, before any possession or credential."""
    operation: str
    operation_id: bytes
    signed_preparation: bytes
    attested_public_key_sec1: bytes
    raw_attestation: bytes

    def validate(self) -> OrdinaryEnrollmentChallenge:
        require(self.operation in ("issue", "recover")
                and type(self.operation_id) is bytes and len(self.operation_id) == 32,
                "invalid raw admission operation")
        challenge = decode_challenge_transport(self.signed_preparation)
        require(self.operation_id == challenge.attestation_challenge(),
                "raw admission differs from signed C attempt")
        point = self.attested_public_key_sec1
        require(type(point) is bytes and len(point) == 65 and point[0] == 4
                and type(self.raw_attestation) is bytes
                and 0 < len(self.raw_attestation) <= 128 * 1024,
                "raw admission originals outside bound")
        return challenge

    def attempt_original(self) -> bytes:
        """Immutable raw attempt framing; these bytes are not an admission."""
        self.validate()
        return b"iroha:kagemusha:v1:ordinary-raw-issuer-attempt\0" + b"".join(
            len(field).to_bytes(8, "little") + field for field in
            (self.signed_preparation, self.attested_public_key_sec1, self.raw_attestation))


@dataclass(frozen=True)
class OrdinaryCredentialRequest:
    """Exact request originals from the authenticated Core facade."""
    operation: str
    operation_id: bytes
    signed_preparation: bytes
    attested_public_key_sec1: bytes
    raw_attestation: bytes
    raw_possession: bytes
    possession_platform: str
    play_integrity_token: str | None

    def raw_request(self) -> OrdinaryRawAttestationRequest:
        return OrdinaryRawAttestationRequest(self.operation, self.operation_id,
            self.signed_preparation, self.attested_public_key_sec1, self.raw_attestation)

    def validate(self) -> OrdinaryEnrollmentChallenge:
        require(self.operation in ("issue", "recover")
                and type(self.operation_id) is bytes and len(self.operation_id) == 32,
                "invalid ordinary credential request operation")
        challenge = decode_challenge_transport(self.signed_preparation)
        require(self.operation_id == challenge.attestation_challenge(),
                "ordinary operation differs from signed enrollment attempt")
        point = self.attested_public_key_sec1
        require(type(point) is bytes and len(point) == 65 and point[0] == 4
                and type(self.raw_attestation) is bytes and 0 < len(self.raw_attestation) <= 128 * 1024
                and type(self.raw_possession) is bytes and bool(self.raw_possession),
                "ordinary credential originals outside bound")
        if challenge.platform_class == 1:
            require(self.possession_platform == "android_keystore" and 8 <= len(self.raw_possession) <= 72,
                    "Android credential possession differs")
        else:
            require(self.possession_platform == "apple_app_attest" and len(self.raw_possession) <= 4096
                    and self.play_integrity_token is None, "Apple credential possession differs")
        require(self.play_integrity_token is None
                or (type(self.play_integrity_token) is str and 0 < len(self.play_integrity_token) <= 64 * 1024
                    and all(33 <= ord(char) <= 126 for char in self.play_integrity_token)),
                "invalid opaque Play Integrity token")
        return challenge

    def attempt_original(self) -> bytes:
        """Stable originals independent of issue/recover selection; not authority."""
        self.validate()
        token = b"" if self.play_integrity_token is None else self.play_integrity_token.encode("ascii")
        fields = (self.signed_preparation, self.attested_public_key_sec1,
                  self.raw_attestation, self.raw_possession, self.possession_platform.encode("ascii"), token)
        return b"iroha:kagemusha:v1:ordinary-app-issuer-attempt\0" + b"".join(
            len(field).to_bytes(8, "little") + field for field in fields)


@dataclass(frozen=True)
class OrdinaryReleasePolicy:
    """Selections projected by the genuine installed Native release owner.

    Account, lane and financial epoch are authorized by the actual Core signed
    preparation. They are not inferred from a public issuer configuration list.
    The provider's deployment must keep the original Native holder alive and
    supply its current-policy recheck before every issue and recovery.
    """
    release_id: bytes
    network_id: bytes
    hardware_profile_id: bytes
    suite_id: bytes
    trust_policy_digest: bytes
    app_authority_policy_digest: bytes
    issuer_policy_digest: bytes
    policy_epoch: int
    profile_valid_from_ms: int
    profile_expires_at_ms: int
    app_signing_identity_digest: bytes
    app_release_digest: bytes
    core_preparation_public_key: bytes
    authority_public_key: bytes
    circuit_issuer_public_key: bytes
    maximum_credential_lifetime_ms: int
    platform_policy: AppleAppPolicy | GoogleKeyMintPolicy | OemKeyMintPolicy
    play_integrity_policy: PlayIntegrityPolicy | None

    @property
    def platform_class(self) -> int:
        return 2 if type(self.platform_policy) is AppleAppPolicy else 1

    @property
    def allowed_android_levels(self) -> frozenset[int]:
        return (frozenset() if self.platform_class == 2
                else self.platform_policy.allowed_security_levels)

    def validate(self) -> None:
        for field in ("release_id", "network_id", "hardware_profile_id", "suite_id", "trust_policy_digest",
                      "app_authority_policy_digest", "issuer_policy_digest", "app_signing_identity_digest",
                      "app_release_digest", "core_preparation_public_key", "authority_public_key"):
            require(any(fixed32(getattr(self, field), field)), "empty ordinary policy selector")
        require(type(self.circuit_issuer_public_key) is bytes and len(self.circuit_issuer_public_key) == 65
                and self.circuit_issuer_public_key[0] == 4 and any(self.circuit_issuer_public_key[1:]),
                "actual governed ordinary issuer P256 key absent")
        require(type(self.policy_epoch) is int and 0 < self.policy_epoch < (1 << 64)
                and type(self.profile_valid_from_ms) is int and type(self.profile_expires_at_ms) is int
                and 0 < self.profile_valid_from_ms < self.profile_expires_at_ms < (1 << 64)
                and type(self.maximum_credential_lifetime_ms) is int
                and 0 < self.maximum_credential_lifetime_ms < (1 << 64),
                "invalid ordinary trust policy interval")
        require(type(self.platform_policy) in (AppleAppPolicy, GoogleKeyMintPolicy, OemKeyMintPolicy),
                "unsupported ordinary platform policy")
        self.platform_policy.validate()
        if self.platform_class == 2:
            require(self.play_integrity_policy is None and self.app_signing_identity_digest
                    == hashlib.sha256(self.platform_policy.app_id.encode("utf-8")).digest(),
                    "ordinary Apple identity or Android policy differs")
            if self.platform_policy.expected_validation_category is not None:
                require(self.app_release_digest == _apple_release_digest(
                    self.platform_policy.expected_validation_category, self.platform_policy.expected_bundle_version),
                    "ordinary Apple release differs from selected distribution")
        else:
            require(self.app_signing_identity_digest == self.platform_policy.signing_certificate_sha256,
                    "ordinary Android signing identity differs")
            if self.play_integrity_policy is not None:
                integrity = self.play_integrity_policy; integrity.validate()
                require(integrity.package_name == self.platform_policy.package_name
                        and integrity.package_version == self.platform_policy.package_version
                        and integrity.app_signing_certificate_sha256 == self.app_signing_identity_digest,
                        "ordinary Play Integrity app differs from key-attestation policy")

    def select(self, challenge: OrdinaryEnrollmentChallenge, now_ms: int) -> None:
        self.validate(); challenge.signing_bytes()
        require(self.profile_valid_from_ms <= now_ms < self.profile_expires_at_ms
                and challenge.platform_class == self.platform_class
                and challenge.policy_epoch == self.policy_epoch
                and all(getattr(challenge, field) == getattr(self, field) for field in
                    ("release_id", "network_id", "hardware_profile_id", "suite_id", "trust_policy_digest",
                     "app_authority_policy_digest", "issuer_policy_digest")),
                "ordinary preparation differs from current Native policy")


@dataclass(frozen=True)
class VerifiedOrdinaryRawEvidence:
    """Provider-checked originals; no final credential, E or monetary authority."""
    request: OrdinaryRawAttestationRequest
    challenge: OrdinaryEnrollmentChallenge
    policy: OrdinaryReleasePolicy
    raw_proof: RawPlatformProof
    trusted_time_ms: int


@dataclass(frozen=True)
class VerifiedOrdinaryEvidence:
    """Checked projection produced by the provider; parsing is not a signer grant."""
    request: OrdinaryCredentialRequest
    challenge: OrdinaryEnrollmentChallenge
    policy: OrdinaryReleasePolicy
    raw_proof: RawPlatformProof
    possession: EnrollmentPossession
    trusted_time_ms: int

    def signing_request(self, issued_at_ms: int, expires_at_ms: int,
                        integrity: PlayIntegrityProof | None) -> bytes:
        return credential_signing_request(self.challenge, self.raw_proof, self.possession,
            self.policy.app_signing_identity_digest, self.policy.app_release_digest,
            self.policy.authority_public_key, issued_at_ms, expires_at_ms,
            self.policy.maximum_credential_lifetime_ms, self.policy.profile_expires_at_ms,
            self.policy.allowed_android_levels, self.policy.play_integrity_policy, integrity,
            self.policy.circuit_issuer_public_key)


class GovernedOrdinaryEvidenceProvider:
    """Composition of exact Core preparation, platform originals, PoP and status.

    Current-policy callbacks are deployment-owned Native holder rechecks, never
    mobile fields or verdict callbacks. Google token decoding is deliberately
    separate so the durable issuer can save each actual response before signing.
    """
    def __init__(self, *, policies: tuple[OrdinaryReleasePolicy, ...],
                 trusted_time_ms: Callable[[], int], recheck_native_policy: Callable[[], None],
                 openssl_path: Path, play_integrity: GooglePlayIntegrityVerifier | None) -> None:
        require(type(policies) is tuple and 0 < len(policies) <= 64
                and callable(trusted_time_ms) and callable(recheck_native_policy)
                and openssl_path.is_absolute() and openssl_path.is_file(),
                "ordinary Native policy holder absent")
        self._policies = {}
        for policy in policies:
            require(type(policy) is OrdinaryReleasePolicy, "invalid ordinary release policy")
            policy.validate()
            key = (policy.release_id, policy.hardware_profile_id)
            require(key not in self._policies, "ambiguous ordinary Native profile")
            self._policies[key] = policy
        require(play_integrity is None or type(play_integrity) is GooglePlayIntegrityVerifier,
                "actual Google decoder is required")
        require(play_integrity is not None or all(p.play_integrity_policy is None for p in policies),
                "selected Play Integrity decoder absent")
        self._clock = trusted_time_ms
        self._recheck = recheck_native_policy
        self._openssl = openssl_path
        self._google = play_integrity

    def prepare_raw(self, request: OrdinaryRawAttestationRequest, *, fresh: bool) -> VerifiedOrdinaryRawEvidence:
        require(type(request) is OrdinaryRawAttestationRequest and type(fresh) is bool,
                "invalid raw admission request")
        challenge = request.validate()
        self._recheck()
        now = self._clock()
        require(type(now) is int and 0 < now < (1 << 64), "invalid ordinary trusted time")
        policy = self._policies.get((challenge.release_id, challenge.hardware_profile_id))
        require(policy is not None, "unapproved ordinary release/profile")
        policy.select(challenge, now)
        authenticate_challenge_transport(request.signed_preparation, challenge,
            policy.core_preparation_public_key, now, self._openssl, fresh=fresh)
        evidence_time = now if fresh else min(now, challenge.expires_at_ms - 1)
        key_id = hashlib.sha256(request.attested_public_key_sec1).digest()
        selected = OrdinaryPlatformEvidenceChallenge(challenge, key_id if challenge.platform_class == 2 else bytes(32))
        platform = policy.platform_policy
        if type(platform) is AppleAppPolicy:
            proof = verify_apple_raw(request.raw_attestation, key_id, platform.app_id,
                platform.environment, selected, platform.attestation_root_der,
                platform.attestation_root_sha256, evidence_time, self._openssl,
                expected_validation_category=platform.expected_validation_category,
                expected_bundle_version=platform.expected_bundle_version)
            original = cbor_exact(request.raw_attestation)["attStmt"]
            verify_apple_receipt(original["receipt"], platform.app_id, original["x5c"][0],
                proof.attested_public_key_sec1, platform.receipt_root_der, platform.receipt_root_sha256,
                evidence_time, self._openssl, expected_type="ATTEST")
            require((proof.apple_validation_category is None) == (proof.apple_bundle_version is None),
                    "incomplete ordinary Apple distribution")
            if proof.apple_validation_category is not None:
                require(policy.app_release_digest == _apple_release_digest(
                    proof.apple_validation_category, proof.apple_bundle_version),
                    "ordinary Apple evidence release differs")
        else:
            chain = decode_android_chain(request.raw_attestation)
            selected_root = (platform.root_for_chain(chain[-1]) if type(platform) is GoogleKeyMintPolicy
                             else platform.attestation_root_der)
            proof = verify_android_persistent_app_key_raw(chain, selected, platform.package_name, platform.package_version,
                platform.signing_certificate_sha256, selected_root,
                hashlib.sha256(selected_root).digest(), evidence_time, self._openssl,
                allowed_security_levels=platform.allowed_security_levels)
            if type(platform) is GoogleKeyMintPolicy:
                verify_google_chain_not_revoked(chain)
            else:
                require(platform.revocation_verifier(tuple(chain), now) is True,
                        "ordinary OEM revocation is not positively clear")
        require(proof.attested_public_key_sec1 == request.attested_public_key_sec1,
                "offered ordinary app key differs from actual attestation")
        self._recheck()
        return VerifiedOrdinaryRawEvidence(request, challenge, policy, proof, now)

    def prepare(self, request: OrdinaryCredentialRequest, *, fresh: bool) -> VerifiedOrdinaryEvidence:
        require(type(request) is OrdinaryCredentialRequest and type(fresh) is bool,
                "invalid ordinary credential request")
        request.validate()
        original = self.prepare_raw(request.raw_request(), fresh=fresh)
        challenge, policy, proof, now = (original.challenge, original.policy,
                                       original.raw_proof, original.trusted_time_ms)
        platform = policy.platform_policy
        possession = verify_enrollment_possession(challenge, proof, request.raw_attestation,
            request.raw_possession, self._openssl,
            apple_app_id=platform.app_id if type(platform) is AppleAppPolicy else None,
            app_release_digest=policy.app_release_digest,
            expected_validation_category=platform.expected_validation_category if type(platform) is AppleAppPolicy else None,
            expected_bundle_version=platform.expected_bundle_version if type(platform) is AppleAppPolicy else None,
            possession_issued_at_ms=challenge.issued_at_ms,
            possession_expires_at_ms=challenge.expires_at_ms)
        require((policy.play_integrity_policy is None) == (request.play_integrity_token is None),
                "opaque Play token differs from selected ordinary policy")
        self._recheck()
        return VerifiedOrdinaryEvidence(request, challenge, policy, proof, possession, now)

    def decode_integrity(self, evidence: VerifiedOrdinaryEvidence):
        require(type(evidence) is VerifiedOrdinaryEvidence, "ordinary evidence absent")
        policy = evidence.policy.play_integrity_policy
        if policy is None:
            return None
        require(self._google is not None, "selected Google decoder absent")
        self._recheck()
        now = self._clock()
        require(type(now) is int and evidence.trusted_time_ms <= now < evidence.challenge.expires_at_ms,
                "ordinary preparation expired before Google decode")
        result = self._google.decode(evidence.request.play_integrity_token, policy,
            evidence.challenge.play_integrity_request_hash(evidence.possession.attested_key_id), now)
        self._recheck()
        return result

    def retained_integrity(self, evidence: VerifiedOrdinaryEvidence, original_google_response: bytes,
                           *, verified_at_ms: int, fresh: bool) -> PlayIntegrityProof | None:
        policy = evidence.policy.play_integrity_policy
        if policy is None:
            require(original_google_response == b"", "unexpected retained Google response")
            return None
        require(type(verified_at_ms) is int and evidence.challenge.issued_at_ms <= verified_at_ms
                <= evidence.trusted_time_ms, "invalid retained Google verification time")
        # Stored originals are recovered only through the store's immutable
        # attempt match. No public/mobile decoded-verdict route reaches this.
        proof = _verify_google_payload(original_google_response, policy,
            evidence.challenge.play_integrity_request_hash(evidence.possession.attested_key_id),
            evidence.trusted_time_ms if fresh else verified_at_ms,
            hashlib.sha256(evidence.request.play_integrity_token.encode("ascii")).digest())
        return proof
