"""NEW first-release policy transcripts matching the Native wallet Model proposal.

These unsigned objects identify actual verifier inputs. Construction, validation and hashing
never authenticate operator approval. Native retains the approved canonical Norito policy
originals and supplies their typed projection; Python does not define another wire codec.
No default policy, root DER, approval, success flag, signer or disabled platform is provided.
"""
from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass

from .attestation import MAX_CERT, fixed32, require, validate_android_patch_floor
from .play_integrity import PlayIntegrityEnrollmentPolicy

_PREFIX = b"iroha:kagemusha:wallet:v1:"
_PACKAGE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)+\Z")


def _uint(value: int, bits: int, name: str, positive: bool = False) -> int:
    require(type(value) is int and (1 if positive else 0) <= value < (1 << bits), name)
    return value


def _binding(value: bytes, name: str) -> bytes:
    require(type(value) is bytes and any(fixed32(value, name)), name)
    return value


def _text(value: str, name: str) -> bytes:
    require(type(value) is str, name)
    try:
        raw = value.encode("utf-8")
    except UnicodeEncodeError:
        require(False, name)
    require(0 < len(raw) <= 255, name)
    return len(raw).to_bytes(4, "little") + raw


def _digest(role: bytes, body: bytes) -> bytes:
    return hashlib.sha256(_PREFIX + role + b"\0" + len(body).to_bytes(8, "little") + body).digest()


@dataclass(frozen=True)
class AndroidAppIdentityV1:
    package_name: str
    package_version: int
    app_signing_certificate_sha256: bytes

    def transcript(self) -> bytes:
        text = _text(self.package_name, "invalid Android package")
        require(_PACKAGE.fullmatch(self.package_name) is not None, "invalid Android package")
        return (b"\1" + text + _uint(self.package_version, 64, "invalid package version").to_bytes(8, "little")
                + _binding(self.app_signing_certificate_sha256, "invalid app signing pin"))


@dataclass(frozen=True)
class AppleAppIdentityV1:
    app_id: str

    def transcript(self) -> bytes:
        return b"\2" + _text(self.app_id, "invalid Apple App ID")


@dataclass(frozen=True)
class WalletAppPolicyV1:
    version: int
    scheme_id: bytes
    identity: AndroidAppIdentityV1 | AppleAppIdentityV1

    def transcript(self) -> bytes:
        require(type(self.version) is int and self.version == 1, "invalid app policy version")
        require(type(self.identity) in (AndroidAppIdentityV1, AppleAppIdentityV1), "invalid app platform")
        return b"\1\0" + _binding(self.scheme_id, "invalid app scheme") + self.identity.transcript()

    def policy_digest(self) -> bytes:
        """NEW app-policy domain; does not authenticate this policy."""
        return _digest(b"app-policy", self.transcript())


@dataclass(frozen=True)
class RegulatoryPolicyV1:
    permitted_controls: int
    blacklist_max_age_ms: int
    time_anchor_max_response_ms: int

    def transcript(self) -> bytes:
        controls = _uint(self.permitted_controls, 32, "invalid regulatory controls")
        age = _uint(self.blacklist_max_age_ms, 64, "invalid blacklist age")
        response = _uint(self.time_anchor_max_response_ms, 64, "invalid time anchor response")
        require(controls & ~7 == 0 and (age == 0 or controls & 1), "invalid regulatory policy")
        require((response > 0) == bool(controls & 6 or age > 0), "invalid regulatory time policy")
        return controls.to_bytes(4, "little") + age.to_bytes(8, "little") + response.to_bytes(8, "little")


@dataclass(frozen=True)
class AndroidEnrollmentPlatformV1:
    attestation_root_sha256: bytes
    hardware_tag: int
    patch_floor_yyyymm: int
    play_integrity_maximum_age_ms: int
    require_play_recognized: bool
    require_licensed: bool
    minimum_device_integrity_tag: int

    def transcript(self) -> bytes:
        require(type(self.hardware_tag) is int and self.hardware_tag in (1, 2, 3), "invalid hardware selector")
        validate_android_patch_floor(self.patch_floor_yyyymm)
        age = _uint(self.play_integrity_maximum_age_ms, 64, "invalid Google age", True)
        require(type(self.require_play_recognized) is bool and type(self.require_licensed) is bool,
                "invalid Google verdict selector")
        require(type(self.minimum_device_integrity_tag) is int
                and self.minimum_device_integrity_tag in (1, 2), "invalid Google device selector")
        return (b"\1" + _binding(self.attestation_root_sha256, "invalid Android root pin")
                + bytes([self.hardware_tag]) + self.patch_floor_yyyymm.to_bytes(4, "little")
                + age.to_bytes(8, "little") + bytes([self.require_play_recognized,
                    self.require_licensed, self.minimum_device_integrity_tag]))

    def allowed_security_levels(self) -> frozenset[int]:
        self.transcript()
        return {1: frozenset({1}), 2: frozenset({2}), 3: frozenset({1, 2})}[self.hardware_tag]


@dataclass(frozen=True)
class AppleEnrollmentPlatformV1:
    attestation_root_sha256: bytes

    def transcript(self) -> bytes:
        return b"\2" + _binding(self.attestation_root_sha256, "invalid Apple root pin")


@dataclass(frozen=True)
class WalletEnrollmentPolicyV1:
    version: int
    scheme_id: bytes
    asset_digest: bytes
    app_policy: bytes
    platform: AndroidEnrollmentPlatformV1 | AppleEnrollmentPlatformV1
    regulatory_policy: RegulatoryPolicyV1
    challenge_lifetime_ms: int
    attestation_lease_lifetime_ms: int

    def transcript(self) -> bytes:
        require(type(self.version) is int and self.version == 1, "invalid enrollment policy version")
        require(type(self.platform) in (AndroidEnrollmentPlatformV1, AppleEnrollmentPlatformV1), "invalid enrollment platform")
        require(type(self.regulatory_policy) is RegulatoryPolicyV1, "invalid regulator type")
        ttl = _uint(self.challenge_lifetime_ms, 64, "invalid challenge lifetime", True)
        lease = _uint(self.attestation_lease_lifetime_ms, 64, "invalid lease lifetime")
        regulator = self.regulatory_policy.transcript()
        require((lease > 0) == bool(self.regulatory_policy.permitted_controls & 4), "inconsistent lease permission")
        selected = self.platform.transcript()
        if type(self.platform) is AndroidEnrollmentPlatformV1:
            require(self.platform.play_integrity_maximum_age_ms <= ttl, "Google age exceeds challenge lifetime")
        return (b"\1\0" + _binding(self.scheme_id, "invalid enrollment scheme")
                + _binding(self.asset_digest, "invalid enrollment asset") + _binding(self.app_policy, "invalid enrollment app")
                + selected + regulator + ttl.to_bytes(8, "little") + lease.to_bytes(8, "little"))

    def policy_digest(self) -> bytes:
        """NEW enrollment-policy identity; existing opaque digests have no implied mapping."""
        return _digest(b"enrollment-policy", self.transcript())

    def validate_for_app(self, app: WalletAppPolicyV1) -> None:
        self.transcript()
        require(type(app) is WalletAppPolicyV1 and self.scheme_id == app.scheme_id
                and self.app_policy == app.policy_digest(), "policy app binding differs")
        require((type(app.identity) is AndroidAppIdentityV1) == (type(self.platform) is AndroidEnrollmentPlatformV1),
                "policy platform differs")

    def require_live_challenge(self, created_at_ms: int, now_ms: int) -> None:
        self.transcript()
        created = _uint(created_at_ms, 64, "invalid challenge creation", True)
        now = _uint(now_ms, 64, "invalid challenge time", True)
        expires = _uint(created + self.challenge_lifetime_ms, 64, "challenge expiry overflow", True)
        require(created <= now < expires, "challenge expired or not yet live")

    def lease_expires_at(self, issued_at_ms: int) -> int:
        self.transcript()
        issued = _uint(issued_at_ms, 64, "invalid issuance time", True)
        return (0 if self.attestation_lease_lifetime_ms == 0 else
                _uint(issued + self.attestation_lease_lifetime_ms, 64, "lease expiry overflow", True))


@dataclass(frozen=True)
class ConfiguredWalletEnrollmentPolicyV1:
    """Private projection of selected originals plus genuine root DER, never a public body.

    Native must authenticate operator selection and compare retained Model policy originals
    before creating this input. Root hashing verifies consistency, not external trust/approval.
    """
    app: WalletAppPolicyV1
    enrollment: WalletEnrollmentPolicyV1
    attestation_root_der: bytes

    def validate_scope(self, challenge_transcript: bytes, created_at_ms: int, now_ms: int) -> None:
        require(type(self.enrollment) is WalletEnrollmentPolicyV1, "configured policy absent")
        self.enrollment.validate_for_app(self.app)
        value = challenge_transcript
        require(type(value) is bytes and len(value) == 194 and value[:2] == b"\1\0"
                and all(any(value[offset:offset + 32]) for offset in range(2, 194, 32)), "invalid E1 transcript")
        require(value[2:34] == self.enrollment.scheme_id and value[34:66] == self.enrollment.asset_digest
                and value[98:130] == self.app.policy_digest()
                and value[130:162] == self.enrollment.policy_digest(), "E1 policy bindings differ")
        self.enrollment.require_live_challenge(created_at_ms, now_ms)
        require(type(self.attestation_root_der) is bytes and 0 < len(self.attestation_root_der) <= MAX_CERT
                and hashlib.sha256(self.attestation_root_der).digest()
                == self.enrollment.platform.attestation_root_sha256, "configured root original differs")

    def play_integrity_policy(self) -> PlayIntegrityEnrollmentPolicy:
        self.enrollment.validate_for_app(self.app)
        app = self.app.identity
        selected = self.enrollment.platform
        require(type(app) is AndroidAppIdentityV1 and type(selected) is AndroidEnrollmentPlatformV1,
                "Android policy absent")
        # Explicit NEW projection mapping, not a claim about any historical opaque digest.
        result = PlayIntegrityEnrollmentPolicy(
            self.enrollment.policy_digest(), app.package_name, app.package_version,
            app.app_signing_certificate_sha256, selected.play_integrity_maximum_age_ms,
            selected.require_play_recognized, selected.require_licensed,
            {1: "MEETS_DEVICE_INTEGRITY", 2: "MEETS_STRONG_INTEGRITY"}[selected.minimum_device_integrity_tag])
        result.validate()
        return result
