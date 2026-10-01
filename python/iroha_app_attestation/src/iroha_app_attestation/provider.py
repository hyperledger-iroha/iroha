"""Deployment-selected app-certificate evidence policy and verification.

The mobile request selects a nonce, account and attested key. It cannot supply
release policy, app identity, roots, issuer keys, the clock or revocation rules.
An operator must authenticate the immutable policies before constructing this
provider; this module does not load or authenticate deployment configuration.
It never connects a signing key to the service.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence

from .apple_receipt import verify_apple_receipt
from .attestation import (
    AttestationRejected, RawPlatformProof, cbor_exact, decode_android_chain,
    app_attest_release_digest as _apple_release_digest,
    fixed32, require, verify_android_raw, verify_apple_raw,
    verify_issuer_preparation,
    GOOGLE_FACTORY_2016_ROOT_SHA256,
)
from .issuance import (
    MAX_ASSERTION_LIFETIME_MS, CertificateFields, GovernedIssuanceScope,
)
from .revocation import verify_google_chain_not_revoked
from .service import CertificateRequest


# Google's documented 2022 RSA, 2025 EC and expired-but-trusted 2016 factory anchors.
# https://developer.android.com/privacy-and-security/security-key-attestation
# https://android.googleapis.com/attestation/root
GOOGLE_ATTESTATION_ROOT_SHA256 = frozenset({
    bytes.fromhex("cedb1cb6dc896ae5ec797348bce9286753c2b38ee71ce0fbe34a9a1248800dfc"),
    bytes.fromhex("6d9db4ce6c5c0b293166d08986e05774a8776ceb525d9e4329520de12ba4bcc0"),
    GOOGLE_FACTORY_2016_ROOT_SHA256,
})
# https://www.apple.com/certificateauthority/Apple_App_Attestation_Root_CA.pem
APPLE_APP_ATTESTATION_ROOT_SHA256 = bytes.fromhex(
    "1cb9823ba28ba6ad2d33a006941de2ae4f513ef1d4e831b9f7e0fa7b6242c932"
)
# Apple's published attestation-object example carries a fraud receipt signed
# through Apple Application Integration CA 5 - G1 to Apple Root CA - G3. The
# receipt does not chain to the separate App Attestation Root CA.
# https://www.apple.com/certificateauthority/AppleRootCA-G3.cer
APPLE_RECEIPT_ROOT_SHA256 = bytes.fromhex(
    "63343abfb89a6a03ebb57e9b3f5fa7be7c4f5c756f3017b3a8c488c3653e9179"
)




@dataclass(frozen=True)
class AppleAppPolicy:
    """Operator-approved App ID, environment, distribution and root pins.

    A selected category/version requires those signed extensions in the raw
    attestation. If both are absent from policy, App Attest proves only the
    selected App ID/environment, not an exact app version or binary.
    """

    app_id: str
    environment: str
    attestation_root_der: bytes
    attestation_root_sha256: bytes
    receipt_root_der: bytes
    receipt_root_sha256: bytes
    expected_validation_category: int | None
    expected_bundle_version: str | None

    def validate(self) -> None:
        """Reject partial or substituted Apple trust configuration."""
        require(type(self.app_id) is str and 0 < len(self.app_id.encode("utf-8")) <= 255
                and self.environment in ("production", "development"),
                "invalid governed Apple App ID or environment")
        _pinned_root(self.attestation_root_der, self.attestation_root_sha256)
        require(self.attestation_root_sha256 == APPLE_APP_ATTESTATION_ROOT_SHA256,
                "Apple attestation root is not Apple's published trust anchor")
        _pinned_root(self.receipt_root_der, self.receipt_root_sha256)
        require(self.receipt_root_sha256 == APPLE_RECEIPT_ROOT_SHA256,
                "Apple receipt root is not Apple's published fraud-receipt trust anchor")
        require((self.expected_validation_category is None)
                == (self.expected_bundle_version is None),
                "incomplete governed Apple distribution selection")
        if self.expected_validation_category is not None:
            require(self.expected_validation_category in (2, 3, 4, 5)
                    and type(self.expected_bundle_version) is str
                    and 0 < len(self.expected_bundle_version.encode("utf-8")) <= 128,
                    "invalid governed Apple distribution selection")


@dataclass(frozen=True)
class GoogleKeyMintPolicy:
    """Operator-pinned Google-root KeyMint app and live status authority.

    The deployment must supply an authentic Google attestation root. Google's
    status list is not authoritative for a non-Google OEM root. Such a profile
    needs a separately implemented and reviewed revocation policy.
    """

    package_name: str
    package_version: int
    signing_certificate_sha256: bytes
    attestation_root_der: bytes
    attestation_root_sha256: bytes
    allowed_security_levels: frozenset[int]
    additional_attestation_roots_der: tuple[bytes, ...] = ()

    def validate(self) -> None:
        """Reject incomplete Android app or root configuration."""
        require(type(self.package_name) is str
                and 0 < len(self.package_name.encode("utf-8")) <= 255
                and type(self.package_version) is int and self.package_version >= 0,
                "invalid governed Android package")
        _android_security_levels(self.allowed_security_levels)
        fixed32(self.signing_certificate_sha256, "governed Android signer")
        _pinned_root(self.attestation_root_der, self.attestation_root_sha256)
        require(self.attestation_root_sha256 in GOOGLE_ATTESTATION_ROOT_SHA256,
                "Android root is not a published Google attestation root")
        require(type(self.additional_attestation_roots_der) is tuple
                and len(self.additional_attestation_roots_der) <= 2,
                "Google root selection outside bound")
        digests = {self.attestation_root_sha256}
        for root in self.additional_attestation_roots_der:
            digest = hashlib.sha256(root).digest() if type(root) is bytes else bytes(32)
            _pinned_root(root, digest)
            require(digest in GOOGLE_ATTESTATION_ROOT_SHA256 and digest not in digests,
                    "Google root selection differs from published originals")
            digests.add(digest)

    def root_for_chain(self, offered_root: bytes) -> bytes:
        """Select only an independently admitted, published complete root original."""
        self.validate()
        roots = (self.attestation_root_der, *self.additional_attestation_roots_der)
        require(type(offered_root) is bytes and offered_root in roots,
                "Android chain root is absent from admitted Google originals")
        return offered_root


@dataclass(frozen=True)
class OemKeyMintPolicy:
    """Operator-approved OEM root with its own live revocation authority.

    A Google attestation status response cannot authorize a Samsung, Huawei,
    Meizu or other OEM certificate. The deployment must authenticate the exact
    OEM root and install a checker which returns ``True`` only after a fresh
    authoritative revocation check of the complete verified chain. Device
    capability qualification remains a separate requirement.
    """

    package_name: str
    package_version: int
    signing_certificate_sha256: bytes
    attestation_root_der: bytes
    attestation_root_sha256: bytes
    revocation_verifier: Callable[[Sequence[bytes], int], bool]
    allowed_security_levels: frozenset[int]

    def validate(self) -> None:
        """Require an exact OEM anchor, app identity and active status checker."""
        require(type(self.package_name) is str
                and 0 < len(self.package_name.encode("utf-8")) <= 255
                and type(self.package_version) is int and self.package_version >= 0,
                "invalid governed OEM Android package")
        _android_security_levels(self.allowed_security_levels)
        fixed32(self.signing_certificate_sha256, "governed OEM Android signer")
        _pinned_root(self.attestation_root_der, self.attestation_root_sha256)
        require(self.attestation_root_sha256 not in GOOGLE_ATTESTATION_ROOT_SHA256
                and callable(self.revocation_verifier),
                "OEM profile needs a non-Google root and live revocation verifier")


@dataclass(frozen=True)
class GovernedReleasePolicy:
    """Immutable release/profile/lane scope from trusted deployment state.

    ``app_release_digest`` commits a governance-authorized distribution
    policy. Platform evidence does not measure an exact distributed binary.
    Accounts are authorized by their exact signed Core preparation, rather
    than a static server account list.
    """

    release_id: bytes
    hardware_profile_id: bytes
    lane_id: bytes
    app_signing_identity_digest: bytes
    app_release_digest: bytes
    issuer_policy_id: bytes
    issuer_public_spki_der: bytes
    issuer_public_spki_sha256: bytes
    authority_public_key: bytes
    max_lifetime_ms: int
    platform_policy: AppleAppPolicy | GoogleKeyMintPolicy | OemKeyMintPolicy

    @property
    def platform(self) -> str:
        """Select the sole platform accepted by this policy."""
        return ("apple_app_attest" if type(self.platform_policy) is AppleAppPolicy
                else "android_keymint")

    def validate(self) -> None:
        """Check complete immutable policy before accepting any request."""
        for name in ("release_id", "hardware_profile_id", "lane_id",
                     "app_signing_identity_digest", "app_release_digest",
                     "issuer_policy_id", "issuer_public_spki_sha256",
                     "authority_public_key"):
            fixed32(getattr(self, name), f"governed {name}")
        require(type(self.issuer_public_spki_der) is bytes
                and 0 < len(self.issuer_public_spki_der) <= 512
                and hashlib.sha256(self.issuer_public_spki_der).digest()
                == self.issuer_public_spki_sha256,
                "untrusted governed issuer key")
        require(type(self.max_lifetime_ms) is int
                and 0 < self.max_lifetime_ms <= MAX_ASSERTION_LIFETIME_MS,
                "invalid governed certificate lifetime")
        require(type(self.platform_policy) in (AppleAppPolicy, GoogleKeyMintPolicy, OemKeyMintPolicy),
                "unsupported governed platform policy")
        self.platform_policy.validate()
        if type(self.platform_policy) is AppleAppPolicy:
            apple = self.platform_policy
            require(self.app_signing_identity_digest
                    == hashlib.sha256(apple.app_id.encode("utf-8")).digest(),
                    "Apple policy signing identity differs from attested App ID")
            if apple.expected_validation_category is not None:
                require(self.app_release_digest == _apple_release_digest(
                    apple.expected_validation_category, apple.expected_bundle_version),
                        "Apple policy release digest differs from selected distribution")
        else:
            require(self.app_signing_identity_digest
                    == self.platform_policy.signing_certificate_sha256,
                    "Android policy signing identity differs from attested signer")


def _android_security_levels(levels: frozenset[int]) -> None:
    """Accept only an explicit authenticated TEE/StrongBox selection."""
    require(type(levels) is frozenset and bool(levels)
            and all(type(level) is int for level in levels)
            and levels <= {1, 2}, "invalid governed Android security levels")


def _pinned_root(root_der: bytes, root_sha256: bytes) -> None:
    """Require the configured root bytes to match an independent policy pin."""
    require(type(root_der) is bytes and 0 < len(root_der) <= 16 * 1024
            and hashlib.sha256(root_der).digest() == fixed32(root_sha256, "governed root pin"),
            "untrusted governed attestation root")


class GovernedEvidenceProvider:
    """Compose signed preparation, raw evidence and applicable live checks.

    Policies, clock and OpenSSL path are injected by the authenticated server
    deployment. A request may only match one exact configured policy. For
    recovery, historic Apple evidence is evaluated no later than the signed
    preparation's expiry; the current policy and issuer signature are still
    checked. New issuance always uses the current clock and a fresh token.
    Android revocation is fetched live for both operations.
    """

    def __init__(self, policies: tuple[GovernedReleasePolicy, ...],
                 trusted_time_ms: Callable[[], int], openssl_path: Path) -> None:
        require(type(policies) is tuple and bool(policies)
                and callable(trusted_time_ms)
                and isinstance(openssl_path, Path) and openssl_path.is_absolute()
                and openssl_path.is_file(), "invalid evidence-provider configuration")
        indexed: dict[tuple[str, bytes, bytes, bytes], GovernedReleasePolicy] = {}
        for policy in policies:
            require(type(policy) is GovernedReleasePolicy,
                    "invalid governed release policy")
            policy.validate()
            key = (policy.platform, policy.release_id,
                   policy.hardware_profile_id, policy.lane_id)
            require(key not in indexed, "ambiguous governed release policy")
            indexed[key] = policy
        self._policies = indexed
        self._trusted_time_ms = trusted_time_ms
        self._openssl_path = openssl_path

    def prepare(self, request: CertificateRequest, *, issue: bool) -> GovernedIssuanceScope:
        """Return a scope only after every selected evidence gate succeeds."""
        require(type(request) is CertificateRequest and type(issue) is bool
                and request.operation in ("issue", "recover")
                and (request.operation == "issue") == issue,
                "invalid app-certificate request")
        selected = request.selection
        selected.transcript()
        key = (request.platform, selected.release_id,
               selected.hardware_profile_id, selected.lane_id)
        policy = self._policies.get(key)
        require(policy is not None, "unapproved release or hardware profile")
        now_ms = self._trusted_time_ms()
        require(type(now_ms) is int and 0 < now_ms < (1 << 64),
                "invalid trusted evidence time")
        preparation = verify_issuer_preparation(
            request.signed_preparation, selected, request.account_canonical,
            policy.issuer_policy_id, policy.issuer_public_spki_der,
            policy.issuer_public_spki_sha256, now_ms, self._openssl_path,
            require_fresh=issue,
        )
        # A completed certificate can be recovered after receipt freshness has
        # lapsed. Cap historical evidence evaluation at the authenticated token
        # lifetime; no new certificate can be issued with that historical time.
        evidence_time_ms = (now_ms if issue else
                            min(now_ms, preparation.expires_at_ms - 1))
        if type(policy.platform_policy) is AppleAppPolicy:
            proof = self._verify_apple(request, policy.platform_policy,
                                       evidence_time_ms)
            require((proof.apple_validation_category is None)
                    == (proof.apple_bundle_version is None),
                    "incomplete Apple signed distribution")
            if proof.apple_validation_category is not None:
                require(proof.apple_bundle_version is not None and policy.app_release_digest
                        == _apple_release_digest(proof.apple_validation_category,
                                                 proof.apple_bundle_version),
                        "Apple signed release differs from governed digest")
        else:
            proof = self._verify_android(request, policy.platform_policy,
                                         evidence_time_ms, now_ms)
        fields = CertificateFields(
            selected.client_nonce, selected.server_nonce,
            policy.app_signing_identity_digest, policy.app_release_digest,
            proof.evidence_sha256, selected.release_id,
            selected.hardware_profile_id, proof.device_key_reference,
            hashlib.sha256(proof.attested_public_key_sec1).digest(),
            selected.lane_id, preparation.issued_at_ms,
            min(preparation.expires_at_ms,
                preparation.issued_at_ms + policy.max_lifetime_ms),
        )
        scope = GovernedIssuanceScope(
            selected, request.account_canonical, policy.issuer_policy_id,
            policy.issuer_public_spki_der, policy.issuer_public_spki_sha256,
            now_ms, self._openssl_path, request.platform, proof,
            policy.release_id, policy.hardware_profile_id, policy.lane_id,
            policy.app_signing_identity_digest, policy.app_release_digest,
            policy.max_lifetime_ms, policy.authority_public_key, fields,
        )
        scope.verified_request(request.signed_preparation, fresh=issue)
        return scope

    def _verify_apple(self, request: CertificateRequest, policy: AppleAppPolicy,
                      evidence_time_ms: int) -> RawPlatformProof:
        """Check the attested key and its independent Apple fraud receipt."""
        proof = verify_apple_raw(
            request.platform_evidence, request.selection.attested_key_id,
            policy.app_id, policy.environment, request.selection,
            policy.attestation_root_der, policy.attestation_root_sha256,
            evidence_time_ms, self._openssl_path,
            expected_validation_category=policy.expected_validation_category,
            expected_bundle_version=policy.expected_bundle_version,
        )
        decoded = cbor_exact(request.platform_evidence)
        statement = decoded["attStmt"]
        verify_apple_receipt(
            statement["receipt"], policy.app_id, statement["x5c"][0],
            proof.attested_public_key_sec1, policy.receipt_root_der,
            policy.receipt_root_sha256, evidence_time_ms, self._openssl_path,
            expected_type="ATTEST",
        )
        return proof

    def _verify_android(self, request: CertificateRequest,
                        policy: GoogleKeyMintPolicy | OemKeyMintPolicy,
                        evidence_time_ms: int,
                        current_time_ms: int) -> RawPlatformProof:
        """Check the exact pinned KeyMint path and fresh Google status."""
        chain = decode_android_chain(request.platform_evidence)
        proof = verify_android_raw(
            chain, request.selection, policy.package_name,
            policy.package_version, policy.signing_certificate_sha256,
            policy.attestation_root_der, policy.attestation_root_sha256,
            evidence_time_ms, self._openssl_path,
            allowed_security_levels=policy.allowed_security_levels,
        )
        require(proof.android_security_level in policy.allowed_security_levels,
                "verified Android level differs from governed policy")
        if type(policy) is GoogleKeyMintPolicy:
            verify_google_chain_not_revoked(chain)
        else:
            require(policy.revocation_verifier(tuple(chain), current_time_ms) is True,
                    "OEM Android attestation revocation is not positively clear")
        return proof
