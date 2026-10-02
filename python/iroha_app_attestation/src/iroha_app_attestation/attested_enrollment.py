"""Attested-app enrollment and sync attestation checks for KAGEMUSHA (suite v1).

This module composes the existing, separately reviewed raw verifiers
(``attestation.py``, ``apple_receipt.py``, ``play_integrity.py`` and
``revocation.py``); it does not change them. What it adds:

* **Vendor-root registry.** Attestation roots are configuration, not code
  paths. Google's key-attestation roots (2025 EC, 2022 RSA, 2016 factory) and
  Apple's App Attestation and receipt roots ship embedded here and are checked
  against the pins in ``provider.py`` at import. Huawei, Xiaomi and Meizu roots
  use the same mechanism but are accepted only when configured, with an explicit
  provenance ``source``; their bytes must come from the vendor's official
  publication or from a real device chain verified against it, never invented.
  Known invented test roots from this repository are refused outright.
* **Minimum versionCode without editing the verifiers.** The attested
  ``attestationApplicationId`` is parsed first, ``versionCode >= min`` is
  required, and the exact-version verifier is then called with the attested
  versionCode. Play Integrity is pinned to that same versionCode, because the
  same APK produced both attestations.
* **Optional Play Integrity.** Enrollment succeeds on a sideloaded APK with a
  valid hardware key attestation. A supplied token is decoded at Google,
  verified and recorded; only a verified token lifts the no-Play-Integrity tier
  cap. A failed, unconfigured or unavailable decode is recorded and treated
  exactly like an absent token, so omitting the token never beats sending it.
* **Tier derivation** from the attestation root, security level, attestation
  version and attested patch levels.

Tier rules (Android; the weakest applicable cap wins, ``S > T > F``):

* base: StrongBox ``S``; TEE ``T``.
* root cap: the weakest ``tier_cap`` among configured roots sharing the presented
  root's *public key*. Google's 2016 factory and 2022 RSA root certificates carry
  the identical RSA key, so a chain can be re-terminated at either certificate;
  both therefore default to ``F``. Only the 2025 EC (RKP) root yields ``S``/``T``.
* attestation version 2 (Keymaster 3): ``F``.
* with a non-zero ``os_patch_floor`` (YYYYMM): a hardware-enforced osPatchLevel
  that is absent, unparsable or below the floor, or a present vendor/boot patch
  level below it: ``F``.
* no verified Play Integrity token: ``play_integrity.absent_tier_cap`` (``T``).

Apple: production App Attest gives ``A``; the development environment gives ``F``.

None of these checks claims a monotonic counter, a rollback-resistant journal,
a trusted clock or one-use keys. They admit a hardware-backed key that only the
authentic app can use; offline-value limits follow from the derived tier.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import re
import ssl
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, FrozenSet, List, Optional, Sequence, Tuple

from .apple_receipt import VerifiedAppleReceipt, verify_apple_receipt
from .attestation import (
    ANDROID_KEY_DESCRIPTION_OID, MAX_CERT, MAX_CHAIN, MAX_OBJECT, AttestationRejected,
    cbor_exact, certificate_key_extensions, certificate_spki_extensions, children,
    der_one, encode_android_chain, explicit_tags, fixed32, pem, positive_integer,
    primitive, require, verify_android_persistent_app_key_raw, verify_apple_assertion,
    verify_apple_raw,
)
from .attested_selection import (
    CERT_PLATFORM_ANDROID_STRONGBOX, CERT_PLATFORM_ANDROID_TEE,
    CERT_PLATFORM_APPLE_SECURE_ENCLAVE, AttestedSelectionV1, attested_device_id,
    enrollment_play_integrity_request_hash, require_p256_public_key, sha256,
    sync_client_data, sync_request_hash,
)
from .play_integrity import (
    MAX_RESPONSE_BYTES, MAX_TOKEN_BYTES, PlayIntegrityPolicy, PlayIntegrityProof,
    _verify_google_payload, request_hash_text,
)
from .provider import (
    APPLE_APP_ATTESTATION_ROOT_SHA256, APPLE_RECEIPT_ROOT_SHA256,
    GOOGLE_ATTESTATION_ROOT_SHA256,
)
from .revocation import certificate_serial, fetch_google_revocation_status


# --------------------------------------------------------------------------
# Errors. Every class is an AttestationRejected, so generic handlers fail closed.

class BelowMinimumVersion(AttestationRejected):
    """The attested app versionCode is below the scheme's minimum."""


class UntrustedVendorRoot(AttestationRejected):
    """The presented chain does not end at a configured vendor root."""


class AttestationRevoked(AttestationRejected):
    """A certificate of the chain is on the vendor's revocation list."""


class PlatformNotConfigured(AttestationRejected):
    """The scheme does not admit this platform."""


class VerificationUnavailable(AttestationRejected):
    """A live dependency (revocation list, Google decoder) could not answer.

    The operation fails closed and may be retried.
    """


class RevocationUnavailable(VerificationUnavailable):
    """The vendor revocation status could not be fetched or parsed."""


class PlayIntegrityUnavailable(VerificationUnavailable):
    """The Google Play Integrity decoder or its OAuth token was unavailable."""


def error_code(error: BaseException) -> Tuple[str, bool]:
    """Map an exception to a stable ``(code, retryable)`` pair for callers."""
    for kind, code, retryable in (
        (BelowMinimumVersion, "below_min_version", False),
        (UntrustedVendorRoot, "untrusted_root", False),
        (AttestationRevoked, "attestation_revoked", False),
        (PlatformNotConfigured, "platform_not_configured", False),
        (VerificationUnavailable, "verification_unavailable", True),
        (AttestationRejected, "attestation_rejected", False),
    ):
        if isinstance(error, kind):
            return code, retryable
    return "internal_error", False


# --------------------------------------------------------------------------
# Tiers.

TIERS = ("A", "S", "T", "F")
ANDROID_TIERS = ("S", "T", "F")
_TIER_RANK = {"A": 3, "S": 3, "T": 2, "F": 1}
SECURITY_LEVEL_TEE = 1
SECURITY_LEVEL_STRONGBOX = 2
_SECURITY_LEVEL_NAMES = {"tee": SECURITY_LEVEL_TEE, "strongbox": SECURITY_LEVEL_STRONGBOX}
_SECURITY_LEVEL_LABELS = {SECURITY_LEVEL_TEE: "tee", SECURITY_LEVEL_STRONGBOX: "strongbox"}


def weaker_tier(first: str, second: str) -> str:
    """Return the weaker of two Android tiers (``F`` is weakest)."""
    require(first in _TIER_RANK and second in _TIER_RANK, "invalid tier")
    return first if _TIER_RANK[first] <= _TIER_RANK[second] else second


def patch_level_yyyymm(value: int) -> Optional[int]:
    """Normalize a KeyMint patch level (YYYYMM or YYYYMMDD) to YYYYMM.

    Returns ``None`` for a value that has neither form. Zero is handled by the
    caller as "not reported".
    """
    if type(value) is not int:
        return None
    if 19000101 <= value <= 99991231:
        value //= 100
    if 190001 <= value <= 999912 and 1 <= value % 100 <= 12:
        return value
    return None


@dataclass(frozen=True)
class TierDecision:
    tier: str
    reasons: Tuple[str, ...]


def derive_android_tier(*, security_level: int, root_tier_cap: str, attestation_version: int,
                        os_patch_level: Optional[int], vendor_patch_level: Optional[int],
                        boot_patch_level: Optional[int], os_patch_floor: int,
                        play_integrity_verified: bool,
                        play_integrity_absent_cap: str) -> TierDecision:
    """Derive the Android tier and every condition that lowered it.

    Patch levels must be the hardware-enforced values from the verified
    KeyDescription; software-enforced copies are never passed here.
    """
    require(security_level in _SECURITY_LEVEL_LABELS, "invalid Android security level")
    require(root_tier_cap in ANDROID_TIERS and play_integrity_absent_cap in ANDROID_TIERS,
            "invalid Android tier cap")
    require(type(os_patch_floor) is int and (os_patch_floor == 0
            or patch_level_yyyymm(os_patch_floor) == os_patch_floor), "invalid patch floor")
    base = "S" if security_level == SECURITY_LEVEL_STRONGBOX else "T"
    caps: List[Tuple[str, str]] = [(root_tier_cap, "root_tier_cap")]
    if attestation_version == 2:
        caps.append(("F", "keymaster3_attestation"))
    if os_patch_floor:
        if not os_patch_level:
            caps.append(("F", "os_patch_level_absent"))
        for level in (os_patch_level, vendor_patch_level, boot_patch_level):
            if not level:
                continue
            normalized = patch_level_yyyymm(level)
            if normalized is None:
                caps.append(("F", "patch_level_unparsable"))
            elif normalized < os_patch_floor:
                caps.append(("F", "patch_level_below_floor"))
    if not play_integrity_verified:
        caps.append((play_integrity_absent_cap, "play_integrity_not_verified"))
    tier = base
    reasons: List[str] = []
    for cap, reason in caps:
        if _TIER_RANK[cap] < _TIER_RANK[base]:
            tier = weaker_tier(tier, cap)
            if reason not in reasons:
                reasons.append(reason)
    return TierDecision(tier, tuple(reasons))


# --------------------------------------------------------------------------
# Vendor-root registry.

ANDROID = "android"
APPLE = "apple"
ROLE_KEY_ATTESTATION = "key_attestation"
ROLE_APP_ATTEST = "app_attest"
ROLE_APP_ATTEST_RECEIPT = "app_attest_receipt"
REVOCATION_GOOGLE = "google_attestation_status"
REVOCATION_NONE = "none"
REVOCATION_NOT_APPLICABLE = "not_applicable"
CONFIGURABLE_VENDORS = frozenset({"huawei", "xiaomi", "meizu"})
_ROOT_ID = re.compile(r"[a-z0-9][a-z0-9.-]{2,63}\Z")
_PACKAGE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)+\Z")

# Invented roots that exist in this repository for mocks and demos. They look
# like vendor roots by name and must never be configured as trust anchors.
INVENTED_REPOSITORY_ROOT_SHA256 = frozenset(bytes.fromhex(value) for value in (
    "b01d535d9a470962a3ffc814fda8ea63fbc52a893d6bf39d865cb60a2c695a05",  # "Iroha HMS Safety Detect Root"
    "56be40cf19b693d4887cbf30d7265eae9fe267dd4698e1acf29530ffabf09e5e",  # "Iroha Play Integrity Root"
    "8425db5da2915e7c931ce32b96f60b35f3f325c2dc85d26394c83912196ca2c2",  # "Mock Huawei StrongBox Root"
    "bf25fa06eb409c2e022b263cfeb7fe168a9491f506fa78ab3fc7202f2842cf8b",  # "Mock OSP KeyMint Root"
))


def _spki_sha256(der: bytes) -> bytes:
    """Digest of a certificate's SubjectPublicKeyInfo content (key identity)."""
    spki, _ = certificate_spki_extensions(der)
    return sha256(spki.value)


@dataclass(frozen=True)
class VendorRoot:
    """One trust anchor. ``tier_cap`` bounds the tier any chain to it can earn."""

    root_id: str
    vendor: str
    platform: str
    role: str
    der: bytes
    sha256: bytes
    tier_cap: str
    revocation: str
    source: str

    def validate(self) -> None:
        require(type(self.root_id) is str and _ROOT_ID.fullmatch(self.root_id) is not None,
                "invalid vendor root ID")
        require(type(self.der) is bytes and 0 < len(self.der) <= MAX_CERT
                and type(self.sha256) is bytes and sha256(self.der) == self.sha256,
                "vendor root bytes differ from their pin")
        require(self.sha256 not in INVENTED_REPOSITORY_ROOT_SHA256,
                "invented repository root cannot be a vendor trust anchor")
        require(type(self.source) is str and 0 < len(self.source) <= 512,
                "vendor root provenance absent")
        certificate_spki_extensions(self.der)
        if self.platform == ANDROID:
            require(self.vendor in CONFIGURABLE_VENDORS | {"google"}
                    and self.role == ROLE_KEY_ATTESTATION
                    and self.tier_cap in ANDROID_TIERS
                    and self.revocation in (REVOCATION_GOOGLE, REVOCATION_NONE),
                    "invalid Android vendor root")
            require((self.vendor == "google") == (self.revocation == REVOCATION_GOOGLE),
                    "Google's status list covers only Google attestation roots")
        else:
            require(self.platform == APPLE and self.vendor == "apple"
                    and self.role in (ROLE_APP_ATTEST, ROLE_APP_ATTEST_RECEIPT)
                    and self.tier_cap == "A" and self.revocation == REVOCATION_NOT_APPLICABLE,
                    "invalid Apple vendor root")

    def describe(self) -> Dict[str, Any]:
        return {"id": self.root_id, "vendor": self.vendor, "platform": self.platform,
                "role": self.role, "sha256": self.sha256.hex(), "tier_cap": self.tier_cap,
                "revocation": self.revocation}


def _b64_der(text: str) -> bytes:
    return base64.b64decode("".join(text.split()), validate=True)


# Public roots, byte-identical to the official publications cited in ``source``
# (checked against https://android.googleapis.com/attestation/root and
# apple.com/certificateauthority on 2026-10-02) and to the pins in provider.py.
_GOOGLE_2022_RSA_DER = _b64_der("""
MIIFHDCCAwSgAwIBAgIJAPHBcqaZ6vUdMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAw
OWU4NTNiNmIwNDUwHhcNMjIwMzIwMTgwNzQ4WhcNNDIwMzE1MTgwNzQ4WjAbMRkwFwYDVQQFExBm
OTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHs
K7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfd
nJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL
/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04
T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+R
hhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1
Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgp
Zrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6
tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8
Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEw
HQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/
GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4IC
AQB8cMqTllHc8U+qCrOlg3H7174lmaCsbo/bJ0C17JEgMLb4kvrqsXZs01U3mB/qABg/1t5Pd5AO
RHARs1hhqGICW/nKMav574f9rZN4PC2ZlufGXb7sIdJpGiO9ctRhiLuYuly10JccUZGEHpHSYM2G
tkgYbZba6lsCPYAAP83cyDV+1aOkTf1RCp/lM0PKvmxYN10RYsK631jrleGdcdkxoSK//mSQbgcW
nmAEZrzHoF1/0gso1HZgIn0YLzVhLSA/iXCX4QT2h3J5z3znluKG1nv8NQdxei2DIIhASWfu804C
A96cQKTTlaae2fweqXjdN1/v2nqOhngNyz1361mFmr4XmaKH/ItTwOe72NI9ZcwS1lVaCvsIkTDC
EXdm9rCNPAY10iTunIHFXRh+7KPzlHGewCq/8TOohBRn0/NNfh7uRslOSZ/xKbN9tMBtw37Z8d2v
vnXq/YWdsm1+JLVwn6yYD/yacNJBlwpddla8eaVMjsF6nBnIgQOf9zKSe06nSTqvgwUHosgOECZJ
Z1EuzbH4yswbt02tKtKEFhx+v+OTge/06V+jGsqTWLsfrOCNLuA8H++z+pUENmpqnnHovaI47gC+
TNpkgYGkkBT6B/m/U01BuOBBTzhIlMEZq9qkDWuM2cA5kW5V3FJUcfHnw1IdYIg2Wxg7yHcQZemF
Qg==
""")
_GOOGLE_2025_EC_DER = _b64_der("""
MIICIjCCAaigAwIBAgIRAISp0Cl7DrWK5/8OgN52BgUwCgYIKoZIzj0EAwMwUjEcMBoGA1UEAwwT
S2V5IEF0dGVzdGF0aW9uIENBMTEQMA4GA1UECwwHQW5kcm9pZDETMBEGA1UECgwKR29vZ2xlIExM
QzELMAkGA1UEBhMCVVMwHhcNMjUwNzE3MjIzMjE4WhcNMzUwNzE1MjIzMjE4WjBSMRwwGgYDVQQD
DBNLZXkgQXR0ZXN0YXRpb24gQ0ExMRAwDgYDVQQLDAdBbmRyb2lkMRMwEQYDVQQKDApHb29nbGUg
TExDMQswCQYDVQQGEwJVUzB2MBAGByqGSM49AgEGBSuBBAAiA2IABCPaI3FO3z5bBQo8cuiEas4H
jqCtG/mLFfRT0MsIssPBEEU5Cfbt6sH5yOAxqEi5QagpU1yX4HwnGb7OtBYpDTB57uH5Eczm34A5
FNijV3s0/f0UPl7zbJcTx6xwqMIRq6NCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMC
AQYwHQYDVR0OBBYEFFIyuyz7RkOb3NaBqQ5lZuA0QepAMAoGCCqGSM49BAMDA2gAMGUCMETfjPO/
HwqReR2CS7p0ZWoD/LHs6hDi422opifHEUaYLxwGlT9SLdjkVpz0UUOR5wIxAIoGyxGKRHVTpqpG
RFiJtQEOOTp/+s1GcxeYuR2zh/80lQyu9vAFCj6E4AXc+osmRg==
""")
_GOOGLE_2016_FACTORY_DER = _b64_der("""
MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAw
OWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBm
OTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHs
K7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfd
nJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL
/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04
T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+R
hhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1
Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgp
Zrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6
tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8
Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCB
ozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0Rs
R/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOg
MYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZI
hvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfB
Pb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1t
wb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCU
JouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhu
Kug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsY
gBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ
79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYk
A3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZu
CuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Y
d6uuJOJENRaNVTzk
""")
_APPLE_APP_ATTESTATION_ROOT_DER = _b64_der("""
MIICITCCAaegAwIBAgIQC/O+DvHN0uD7jG5yH2IXmDAKBggqhkjOPQQDAzBSMSYwJAYDVQQDDB1B
cHBsZSBBcHAgQXR0ZXN0YXRpb24gUm9vdCBDQTETMBEGA1UECgwKQXBwbGUgSW5jLjETMBEGA1UE
CAwKQ2FsaWZvcm5pYTAeFw0yMDAzMTgxODMyNTNaFw00NTAzMTUwMDAwMDBaMFIxJjAkBgNVBAMM
HUFwcGxlIEFwcCBBdHRlc3RhdGlvbiBSb290IENBMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYD
VQQIDApDYWxpZm9ybmlhMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAERTHhmLW07ATaFQIEVwTtT4dy
ctdhNbJhFs/Ii2FdCgAHGbpphY3+d8qjuDngIN3WVhQUBHAoMeQ/cLiP1sOUtgjqK9auYen1mMEv
Rq9Sk3Jm5X8U62H+xTD3FE9TgS41o0IwQDAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBSskRBT
M72+aEH/pwyp5frq5eWKoTAOBgNVHQ8BAf8EBAMCAQYwCgYIKoZIzj0EAwMDaAAwZQIwQgFGnByv
siVbpTKwSga0kP0e8EeDS4+sQmTvb7vn53O5+FRXgeLhpJ06ysC5PrOyAjEAp5U4xDgEgllF7En3
VcE3iexZZtKeYnpqtijVoyFraWVIyd/dganmrduC1bmTBGwD
""")
_APPLE_ROOT_CA_G3_DER = _b64_der("""
MIICQzCCAcmgAwIBAgIILcX8iNLFS5UwCgYIKoZIzj0EAwMwZzEbMBkGA1UEAwwSQXBwbGUgUm9v
dCBDQSAtIEczMSYwJAYDVQQLDB1BcHBsZSBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTETMBEGA1UE
CgwKQXBwbGUgSW5jLjELMAkGA1UEBhMCVVMwHhcNMTQwNDMwMTgxOTA2WhcNMzkwNDMwMTgxOTA2
WjBnMRswGQYDVQQDDBJBcHBsZSBSb290IENBIC0gRzMxJjAkBgNVBAsMHUFwcGxlIENlcnRpZmlj
YXRpb24gQXV0aG9yaXR5MRMwEQYDVQQKDApBcHBsZSBJbmMuMQswCQYDVQQGEwJVUzB2MBAGByqG
SM49AgEGBSuBBAAiA2IABJjpLz1AcqTtkyJygRMc3RCV8cWjTnHcFBbZDuWmBSp3ZHtfTjjTuxxE
tX/1H7YyYl3J6YRbTzBPEVoA/VhYDKX1DyxNB0cTddqXl5dvMVztK517IDvYuVTZXpmkOlEKMaNC
MEAwHQYDVR0OBBYEFLuw3qFYM4iapIqZ3r6966/ayySrMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0P
AQH/BAQDAgEGMAoGCCqGSM49BAMDA2gAMGUCMQCD6cHEFl4aXTQY2e3v9GwOAEZLuN+yRhHFD/3m
eoyhpmvOwgPUnPWTxnS4at+qIxUCMG1mihDK1A3UT82NQz60imOlM27jbdoXt2QfyFMm+YhidDkL
F1vLUagM6BgD56KyKA==
""")

_GOOGLE_ROOT_SOURCE = "https://android.googleapis.com/attestation/root"
_GOOGLE_FACTORY_SOURCE = "https://developer.android.com/privacy-and-security/security-key-attestation"


def _shipped(root_id: str, vendor: str, platform: str, role: str, der: bytes,
             tier_cap: str, revocation: str, source: str) -> VendorRoot:
    return VendorRoot(root_id, vendor, platform, role, der, sha256(der), tier_cap,
                      revocation, source)


SHIPPED_VENDOR_ROOTS: Tuple[VendorRoot, ...] = (
    _shipped("google-key-attestation-2025-ec", "google", ANDROID, ROLE_KEY_ATTESTATION,
             _GOOGLE_2025_EC_DER, "S", REVOCATION_GOOGLE, _GOOGLE_ROOT_SOURCE),
    # Same RSA key as the 2016 factory certificate below; a chain verifies to
    # either certificate, so it cannot be the higher tier on its own.
    _shipped("google-key-attestation-2022-rsa", "google", ANDROID, ROLE_KEY_ATTESTATION,
             _GOOGLE_2022_RSA_DER, "F", REVOCATION_GOOGLE, _GOOGLE_ROOT_SOURCE),
    _shipped("google-key-attestation-2016-factory", "google", ANDROID, ROLE_KEY_ATTESTATION,
             _GOOGLE_2016_FACTORY_DER, "F", REVOCATION_GOOGLE, _GOOGLE_FACTORY_SOURCE),
    _shipped("apple-app-attestation-root", "apple", APPLE, ROLE_APP_ATTEST,
             _APPLE_APP_ATTESTATION_ROOT_DER, "A", REVOCATION_NOT_APPLICABLE,
             "https://www.apple.com/certificateauthority/Apple_App_Attestation_Root_CA.pem"),
    _shipped("apple-root-ca-g3", "apple", APPLE, ROLE_APP_ATTEST_RECEIPT,
             _APPLE_ROOT_CA_G3_DER, "A", REVOCATION_NOT_APPLICABLE,
             "https://www.apple.com/certificateauthority/AppleRootCA-G3.cer"),
)
SHIPPED_ROOTS_BY_ID: Dict[str, VendorRoot] = {root.root_id: root for root in SHIPPED_VENDOR_ROOTS}
_SHIPPED_PINS = frozenset(root.sha256 for root in SHIPPED_VENDOR_ROOTS)

# Bind the embedded bytes to the existing provider pins at import.
if (frozenset(root.sha256 for root in SHIPPED_VENDOR_ROOTS if root.vendor == "google")
        != GOOGLE_ATTESTATION_ROOT_SHA256
        or SHIPPED_ROOTS_BY_ID["apple-app-attestation-root"].sha256 != APPLE_APP_ATTESTATION_ROOT_SHA256
        or SHIPPED_ROOTS_BY_ID["apple-root-ca-g3"].sha256 != APPLE_RECEIPT_ROOT_SHA256):
    raise ImportError("shipped attestation roots differ from provider.py pins")
for _root in SHIPPED_VENDOR_ROOTS:
    _root.validate()
del _root


class VendorRootRegistry:
    """Configured trust anchors, indexed by exact DER and by public key."""

    def __init__(self, roots: Sequence[VendorRoot]) -> None:
        require(type(roots) in (list, tuple) and 0 < len(roots) <= 32, "vendor roots outside bound")
        ids: set = set()
        ders: set = set()
        for root in roots:
            require(type(root) is VendorRoot, "invalid vendor root")
            root.validate()
            require(root.root_id not in ids and root.der not in ders, "duplicate vendor root")
            ids.add(root.root_id)
            ders.add(root.der)
        self.roots: Tuple[VendorRoot, ...] = tuple(roots)
        self._android = {root.der: root for root in roots if root.platform == ANDROID}
        # Key equivalence: a chain verifies to every certificate of one key.
        key_caps: Dict[bytes, str] = {}
        for root in self._android.values():
            key = _spki_sha256(root.der)
            key_caps[key] = weaker_tier(key_caps.get(key, root.tier_cap), root.tier_cap)
        self._key_caps = key_caps
        apple = [root for root in roots if root.platform == APPLE]
        self._app_attest = [root for root in apple if root.role == ROLE_APP_ATTEST]
        self._receipt = [root for root in apple if root.role == ROLE_APP_ATTEST_RECEIPT]
        require(len(self._app_attest) <= 1 and len(self._receipt) <= 1,
                "ambiguous Apple root selection")

    @property
    def has_android(self) -> bool:
        return bool(self._android)

    @property
    def has_apple(self) -> bool:
        return bool(self._app_attest) and bool(self._receipt)

    def android_root(self, presented_root: bytes) -> VendorRoot:
        """Select the configured root equal to the chain's final certificate."""
        root = self._android.get(presented_root) if type(presented_root) is bytes else None
        if root is None:
            raise UntrustedVendorRoot("Android chain does not end at a configured vendor root")
        return root

    def effective_tier_cap(self, root: VendorRoot) -> str:
        """The weakest cap configured for any certificate of the same key."""
        return self._key_caps[_spki_sha256(root.der)]

    def apple_roots(self) -> Tuple[VendorRoot, VendorRoot]:
        if not self.has_apple:
            raise PlatformNotConfigured("Apple attestation roots are not configured")
        return self._app_attest[0], self._receipt[0]

    def describe(self) -> List[Dict[str, Any]]:
        result = []
        for root in self.roots:
            item = root.describe()
            if root.platform == ANDROID:
                item["effective_tier_cap"] = self.effective_tier_cap(root)
            result.append(item)
        return result


def _require_self_signed(der: bytes, openssl_path: Path) -> None:
    """Check that a configured root is self-issued and self-signed."""
    spki, _ = certificate_spki_extensions(der)
    del spki
    tbs = children(children(der_one(der))[0])
    start = 1 if tbs[0].tag_class == 2 and tbs[0].number == 0 else 0
    require(tbs[start + 2] == tbs[start + 4], "vendor root is not self-issued")
    with tempfile.TemporaryDirectory(prefix="kagemusha-attested-root-") as temporary:
        path = Path(temporary) / "root.pem"
        path.write_bytes(pem(der))
        result = subprocess.run(
            [str(openssl_path), "verify", "-no-CAfile", "-no-CApath", "-no-CAstore",
             "-check_ss_sig", "-no_check_time", "-trusted", str(path), str(path)],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False,
        )
    require(result.returncode == 0, "vendor root self-signature rejected")


def _exact_keys(value: Any, required: set, optional: set = frozenset(), *, name: str) -> Dict[str, Any]:
    require(type(value) is dict and required <= set(value) <= (required | set(optional)),
            f"invalid {name} layout")
    return value


def _hex_bytes(value: Any, size: int, name: str) -> bytes:
    require(type(value) is str and len(value) == 2 * size
            and re.fullmatch(r"[0-9a-f]*", value) is not None, f"invalid {name}")
    return bytes.fromhex(value)


def vendor_roots_from_config(entries: Any, openssl_path: Path) -> VendorRootRegistry:
    """Build the registry from configuration entries.

    ``{"id": <shipped id>}`` selects a shipped Google or Apple root; an Android
    shipped entry may override ``tier_cap``. A custom entry admits a Huawei,
    Xiaomi or Meizu key-attestation root and must state its exact DER, pin,
    revocation source (currently only ``"none"``) and provenance ``source``.
    Google and Apple roots cannot be added or substituted through custom entries.
    """
    require(type(entries) is list and 0 < len(entries) <= 32, "vendor roots outside bound")
    roots: List[VendorRoot] = []
    for entry in entries:
        require(type(entry) is dict and type(entry.get("id")) is str, "invalid vendor root entry")
        shipped = SHIPPED_ROOTS_BY_ID.get(entry["id"])
        if shipped is not None:
            _exact_keys(entry, {"id"}, {"tier_cap"}, name="shipped vendor root")
            if "tier_cap" in entry:
                require(shipped.platform == ANDROID and entry["tier_cap"] in ANDROID_TIERS,
                        "invalid shipped root tier cap")
                shipped = VendorRoot(shipped.root_id, shipped.vendor, shipped.platform,
                                     shipped.role, shipped.der, shipped.sha256,
                                     entry["tier_cap"], shipped.revocation, shipped.source)
            roots.append(shipped)
            continue
        _exact_keys(entry, {"id", "vendor", "platform", "der_base64", "sha256", "revocation",
                            "source"}, {"tier_cap"}, name="custom vendor root")
        require(entry["vendor"] in CONFIGURABLE_VENDORS and entry["platform"] == ANDROID,
                "only Huawei, Xiaomi or Meizu Android roots can be added")
        require(entry["revocation"] == REVOCATION_NONE,
                "unsupported custom vendor revocation source")
        tier_cap = entry.get("tier_cap", "F")
        require(tier_cap in ANDROID_TIERS, "invalid custom root tier cap")
        try:
            der = base64.b64decode(entry["der_base64"], validate=True) \
                if type(entry["der_base64"]) is str else b""
        except (binascii.Error, ValueError):
            raise AttestationRejected("invalid custom root DER") from None
        require(base64.b64encode(der).decode("ascii") == entry["der_base64"],
                "noncanonical custom root DER")
        pin = _hex_bytes(entry["sha256"], 32, "custom root pin")
        require(pin not in _SHIPPED_PINS, "a shipped root cannot be relabelled as another vendor")
        root = VendorRoot(entry["id"], entry["vendor"], ANDROID, ROLE_KEY_ATTESTATION, der,
                          pin, tier_cap, REVOCATION_NONE, entry["source"])
        root.validate()
        _require_self_signed(der, openssl_path)
        roots.append(root)
    return VendorRootRegistry(roots)


# --------------------------------------------------------------------------
# Scheme policy (projection of the root-signed descriptor held by the issuer).

PLAY_INTEGRITY_DEVICE_LEVELS = ("MEETS_DEVICE_INTEGRITY", "MEETS_STRONG_INTEGRITY")
MAX_REVOCATION_AGE_MS = 24 * 60 * 60 * 1000


@dataclass(frozen=True)
class AttestedPlayIntegrityPolicy:
    minimum_device_integrity: str
    require_play_recognized: bool
    require_licensed: bool
    maximum_evidence_age_ms: int
    absent_tier_cap: str

    def validate(self) -> None:
        require(self.minimum_device_integrity in PLAY_INTEGRITY_DEVICE_LEVELS
                and type(self.require_play_recognized) is bool
                and type(self.require_licensed) is bool
                and type(self.maximum_evidence_age_ms) is int
                and 0 < self.maximum_evidence_age_ms <= MAX_REVOCATION_AGE_MS
                and self.absent_tier_cap in ANDROID_TIERS,
                "invalid Play Integrity policy")


@dataclass(frozen=True)
class AttestedAndroidPolicy:
    package_name: str
    signer_sha256: Tuple[bytes, ...]
    min_version_code: int
    allowed_security_levels: FrozenSet[int]
    os_patch_floor: int
    play_integrity: AttestedPlayIntegrityPolicy

    def validate(self) -> None:
        require(type(self.package_name) is str and len(self.package_name) <= 255
                and _PACKAGE.fullmatch(self.package_name) is not None,
                "invalid Android package")
        require(type(self.signer_sha256) is tuple and 0 < len(self.signer_sha256) <= 8
                and len(set(self.signer_sha256)) == len(self.signer_sha256),
                "invalid Android signer set")
        for signer in self.signer_sha256:
            fixed32(signer, "Android signer")
        require(type(self.min_version_code) is int and 0 <= self.min_version_code < (1 << 63),
                "invalid minimum versionCode")
        require(type(self.allowed_security_levels) is frozenset
                and self.allowed_security_levels
                and self.allowed_security_levels <= {SECURITY_LEVEL_TEE, SECURITY_LEVEL_STRONGBOX},
                "invalid Android security levels")
        require(type(self.os_patch_floor) is int and (self.os_patch_floor == 0
                or patch_level_yyyymm(self.os_patch_floor) == self.os_patch_floor),
                "invalid Android patch floor")
        require(type(self.play_integrity) is AttestedPlayIntegrityPolicy,
                "invalid Play Integrity policy")
        self.play_integrity.validate()

    def projection(self) -> Dict[str, Any]:
        pi = self.play_integrity
        return {"package_name": self.package_name,
                "signer_sha256": [signer.hex() for signer in self.signer_sha256],
                "min_version_code": self.min_version_code,
                "allowed_security_levels": sorted(_SECURITY_LEVEL_LABELS[level]
                                                  for level in self.allowed_security_levels),
                "os_patch_floor": self.os_patch_floor,
                "play_integrity": {"minimum_device_integrity": pi.minimum_device_integrity,
                                   "require_play_recognized": pi.require_play_recognized,
                                   "require_licensed": pi.require_licensed,
                                   "maximum_evidence_age_ms": pi.maximum_evidence_age_ms,
                                   "absent_tier_cap": pi.absent_tier_cap}}


@dataclass(frozen=True)
class AttestedApplePolicy:
    app_id: str
    environment: str

    def validate(self) -> None:
        require(type(self.app_id) is str and 0 < len(self.app_id.encode("utf-8")) <= 255
                and re.fullmatch(r"[A-Z0-9]{10}\.[A-Za-z0-9.-]+", self.app_id) is not None,
                "invalid Apple App ID")
        require(self.environment in ("production", "development"),
                "invalid App Attest environment")


@dataclass(frozen=True)
class AttestedEnrollmentConfig:
    scheme_id: bytes
    roots: VendorRootRegistry
    android: Optional[AttestedAndroidPolicy]
    apple: Optional[AttestedApplePolicy]
    openssl_path: Path
    revocation_max_age_ms: int

    def validate(self) -> None:
        fixed32(self.scheme_id, "scheme ID")
        require(type(self.roots) is VendorRootRegistry, "invalid vendor root registry")
        require(self.android is not None or self.apple is not None, "no platform configured")
        if self.android is not None:
            require(type(self.android) is AttestedAndroidPolicy, "invalid Android policy")
            self.android.validate()
            require(self.roots.has_android, "Android policy without Android roots")
        if self.apple is not None:
            require(type(self.apple) is AttestedApplePolicy, "invalid Apple policy")
            self.apple.validate()
            require(self.roots.has_apple, "Apple policy without Apple roots")
        require(isinstance(self.openssl_path, Path) and self.openssl_path.is_absolute()
                and self.openssl_path.is_file(), "invalid OpenSSL executable")
        require(type(self.revocation_max_age_ms) is int
                and 0 < self.revocation_max_age_ms <= MAX_REVOCATION_AGE_MS,
                "invalid revocation freshness")

    @classmethod
    def from_json(cls, value: Any) -> "AttestedEnrollmentConfig":
        """Parse the strict JSON projection the issuer derives from its descriptor."""
        value = _exact_keys(value, {"scheme_id", "vendor_roots", "android", "apple",
                                    "openssl_path", "revocation_max_age_ms"},
                            name="attested enrollment configuration")
        openssl = value["openssl_path"]
        require(type(openssl) is str and openssl.startswith("/"), "invalid OpenSSL executable")
        openssl_path = Path(openssl)
        require(openssl_path.is_file(), "invalid OpenSSL executable")
        android = None
        if value["android"] is not None:
            item = _exact_keys(value["android"], {"package_name", "signer_sha256",
                               "min_version_code", "allowed_security_levels", "os_patch_floor",
                               "play_integrity"}, name="Android policy")
            pi = _exact_keys(item["play_integrity"], {"minimum_device_integrity",
                             "require_play_recognized", "require_licensed",
                             "maximum_evidence_age_ms", "absent_tier_cap"},
                             name="Play Integrity policy")
            signers = item["signer_sha256"]
            require(type(signers) is list, "invalid Android signer set")
            levels = item["allowed_security_levels"]
            require(type(levels) is list and all(type(level) is str and level in _SECURITY_LEVEL_NAMES
                                                for level in levels)
                    and len(set(levels)) == len(levels), "invalid Android security levels")
            android = AttestedAndroidPolicy(
                item["package_name"], tuple(_hex_bytes(s, 32, "Android signer") for s in signers),
                item["min_version_code"], frozenset(_SECURITY_LEVEL_NAMES[level] for level in levels),
                item["os_patch_floor"],
                AttestedPlayIntegrityPolicy(pi["minimum_device_integrity"],
                                            pi["require_play_recognized"], pi["require_licensed"],
                                            pi["maximum_evidence_age_ms"], pi["absent_tier_cap"]))
        apple = None
        if value["apple"] is not None:
            item = _exact_keys(value["apple"], {"app_id", "environment"}, name="Apple policy")
            apple = AttestedApplePolicy(item["app_id"], item["environment"])
        config = cls(_hex_bytes(value["scheme_id"], 32, "scheme ID"),
                     vendor_roots_from_config(value["vendor_roots"], openssl_path),
                     android, apple, openssl_path, value["revocation_max_age_ms"])
        config.validate()
        return config


# --------------------------------------------------------------------------
# Android KeyDescription facts.

@dataclass(frozen=True)
class AndroidKeyFacts:
    """Fields of the KeyDescription nearest the root (same selection as the verifier).

    Before ``verify_android_persistent_app_key_raw`` succeeds these are only
    candidates; afterwards they are signed attestation content. Patch levels are
    taken from the hardware-enforced list only.
    """

    attestation_version: int
    attestation_security_level: int
    keymint_version: int
    keymint_security_level: int
    package_name: str
    version_code: int
    signer_sha256: bytes
    os_version: Optional[int]
    os_patch_level: Optional[int]
    vendor_patch_level: Optional[int]
    boot_patch_level: Optional[int]

    def to_json(self) -> Dict[str, Any]:
        return {"attestation_version": self.attestation_version,
                "attestation_security_level": self.attestation_security_level,
                "keymint_version": self.keymint_version,
                "keymint_security_level": self.keymint_security_level,
                "package_name": self.package_name, "version_code": self.version_code,
                "signer_sha256": self.signer_sha256.hex(), "os_version": self.os_version,
                "os_patch_level": self.os_patch_level,
                "vendor_patch_level": self.vendor_patch_level,
                "boot_patch_level": self.boot_patch_level}


def android_key_facts(chain_der: Sequence[bytes], root_der: bytes) -> AndroidKeyFacts:
    """Parse the attestation application ID, versions and patch levels."""
    require(type(chain_der) in (list, tuple) and 2 <= len(chain_der) <= MAX_CHAIN
            and all(type(cert) is bytes and 0 < len(cert) <= MAX_CERT for cert in chain_der),
            "certificate chain outside bound")
    selected = next((certificate for certificate in reversed(chain_der)
                     if certificate != root_der
                     and ANDROID_KEY_DESCRIPTION_OID in certificate_spki_extensions(certificate)[1]),
                    None)
    require(selected is not None, "missing KeyMint extension")
    description = children(der_one(certificate_spki_extensions(selected)[1][ANDROID_KEY_DESCRIPTION_OID]))
    require(len(description) == 8, "invalid KeyMint key description")
    version = positive_integer(description[0])
    software = explicit_tags(description[6], ordinary_version=version)
    hardware = explicit_tags(description[7], ordinary_version=version)
    require(709 in software, "KeyMint app identity missing")
    app_id = children(der_one(primitive(software[709], 4)))
    require(len(app_id) == 2, "invalid KeyMint app ID")
    packages = children(app_id[0], number=17)
    signers = children(app_id[1], number=17)
    require(len(packages) == 1 and len(signers) == 1, "shared-UID or multi-signer app is not approved")
    package = children(packages[0])
    require(len(package) == 2, "invalid KeyMint package")
    try:
        package_name = primitive(package[0], 4).decode("utf-8")
    except UnicodeDecodeError:
        raise AttestationRejected("invalid KeyMint package name") from None
    signer = primitive(signers[0], 4)
    require(len(signer) == 32, "invalid KeyMint signer digest")

    def hardware_integer(tag: int) -> Optional[int]:
        return positive_integer(hardware[tag]) if tag in hardware else None

    return AndroidKeyFacts(
        version, positive_integer(description[1], 10), positive_integer(description[2]),
        positive_integer(description[3], 10), package_name, positive_integer(package[1]),
        signer, hardware_integer(705), hardware_integer(706), hardware_integer(718),
        hardware_integer(719))


# --------------------------------------------------------------------------
# Live dependencies.

def _wall_clock_ms() -> int:
    return time.time_ns() // 1_000_000


class GoogleRevocationCache:
    """Google's attestation status list, refreshed when older than ``max_age_ms``.

    A failed fetch never falls back to an older list: verification fails closed
    with ``RevocationUnavailable`` until a fresh list is available.
    """

    def __init__(self, *, clock: Callable[[], int], max_age_ms: int,
                 fetch: Optional[Callable[[], FrozenSet[int]]] = None) -> None:
        require(callable(clock) and type(max_age_ms) is int
                and 0 < max_age_ms <= MAX_REVOCATION_AGE_MS, "invalid revocation cache")
        self._clock = clock
        self._max_age_ms = max_age_ms
        self._fetch = fetch if fetch is not None else fetch_google_revocation_status
        self._serials: Optional[FrozenSet[int]] = None
        self._fetched_at_ms: Optional[int] = None

    @property
    def fetched_at_ms(self) -> Optional[int]:
        return self._fetched_at_ms

    def refresh(self) -> int:
        try:
            serials = self._fetch()
        except Exception:
            self._serials = None
            self._fetched_at_ms = None
            raise RevocationUnavailable("Android revocation status unavailable") from None
        require(type(serials) is frozenset and all(type(serial) is int for serial in serials),
                "invalid Android revocation status")
        self._serials = serials
        self._fetched_at_ms = self._clock()
        return self._fetched_at_ms

    def current(self) -> Tuple[FrozenSet[int], int]:
        now = self._clock()
        if (self._serials is None or self._fetched_at_ms is None or now < self._fetched_at_ms
                or now - self._fetched_at_ms > self._max_age_ms):
            self.refresh()
        assert self._serials is not None and self._fetched_at_ms is not None
        return self._serials, self._fetched_at_ms

    def require_not_revoked(self, chain_der: Sequence[bytes]) -> int:
        serials = {certificate_serial(certificate) for certificate in chain_der}
        listed, fetched_at = self.current()
        if listed.intersection(serials):
            raise AttestationRevoked("revoked Android attestation certificate")
        return fetched_at


PLAY_INTEGRITY_DECODE_URL = "https://playintegrity.googleapis.com/v1/{package}:decodeIntegrityToken"


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


def _require_opaque_token(token: Any) -> str:
    require(type(token) is str and 0 < len(token) <= MAX_TOKEN_BYTES
            and all(33 <= ord(char) <= 126 for char in token),
            "invalid opaque Play Integrity token")
    return token


class GooglePlayIntegrityDecoder:
    """Decode a Standard API token at Google's fixed endpoint; return the raw body.

    ``access_token`` supplies a ``playintegrity``-scoped OAuth token. Verdicts
    are never accepted from the device. HTTP 400 (malformed or foreign token)
    is a rejection; transport, OAuth and other HTTP failures are retryable.
    """

    def __init__(self, access_token: Callable[[], str], *, timeout_s: float = 5.0) -> None:
        require(callable(access_token), "Play Integrity access token source absent")
        self._access_token = access_token
        self._timeout_s = timeout_s

    def decode(self, opaque_token: str, package_name: str) -> bytes:
        _require_opaque_token(opaque_token)
        require(type(package_name) is str and _PACKAGE.fullmatch(package_name) is not None,
                "invalid Play Integrity package")
        try:
            access = self._access_token()
        except Exception:
            raise PlayIntegrityUnavailable("Play Integrity OAuth token unavailable") from None
        require(type(access) is str and 0 < len(access) <= 8192
                and all(33 <= ord(char) <= 126 for char in access),
                "Play Integrity OAuth token unavailable")
        url = PLAY_INTEGRITY_DECODE_URL.format(package=package_name)
        request = urllib.request.Request(
            url, data=json.dumps({"integrity_token": opaque_token},
                                 separators=(",", ":")).encode("ascii"),
            headers={"Authorization": f"Bearer {access}", "Content-Type": "application/json",
                     "Accept": "application/json"}, method="POST")
        opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}),
            urllib.request.HTTPSHandler(context=ssl.create_default_context()), _NoRedirect())
        try:
            with opener.open(request, timeout=self._timeout_s) as response:
                if not (response.status == 200 and response.geturl() == url
                        and response.headers.get("Content-Type", "").split(";", 1)[0]
                        .strip().lower() == "application/json"
                        and response.headers.get("Content-Encoding", "identity").lower()
                        == "identity"):
                    raise PlayIntegrityUnavailable("Play Integrity decoder response changed")
                body = response.read(MAX_RESPONSE_BYTES + 1)
        except urllib.error.HTTPError as error:
            code = error.code
            error.close()
            if code == 400:
                raise AttestationRejected("Play Integrity decoder rejected the token") from None
            raise PlayIntegrityUnavailable("Play Integrity decoder unavailable") from None
        except VerificationUnavailable:
            raise
        except Exception:
            raise PlayIntegrityUnavailable("Play Integrity decoder unavailable") from None
        require(0 < len(body) <= MAX_RESPONSE_BYTES, "Play Integrity response outside bound")
        return body


@dataclass(frozen=True)
class PlayIntegrityOutcome:
    """What happened to an optional Play Integrity token.

    ``status`` is one of ``absent``, ``not_configured``, ``unavailable``,
    ``rejected`` or ``verified``. Only ``verified`` can lift the tier cap.
    """

    status: str
    request_hash: bytes
    reason: Optional[str] = None
    token_sha256: Optional[bytes] = None
    google_response: Optional[bytes] = None
    proof: Optional[PlayIntegrityProof] = None
    version_code: Optional[int] = None
    signer_sha256: Optional[bytes] = None

    @property
    def verified(self) -> bool:
        return self.status == "verified"

    def to_json(self) -> Dict[str, Any]:
        result: Dict[str, Any] = {"status": self.status, "request_hash": self.request_hash.hex(),
                                  "request_hash_text": request_hash_text(self.request_hash)}
        if self.reason is not None:
            result["reason"] = self.reason
        if self.token_sha256 is not None:
            result["token_sha256"] = self.token_sha256.hex()
        if self.google_response is not None:
            result["google_response_base64"] = base64.b64encode(self.google_response).decode("ascii")
            result["google_response_sha256"] = sha256(self.google_response).hex()
        if self.proof is not None:
            result.update({"timestamp_ms": self.proof.timestamp_ms,
                           "app_recognition_verdict": self.proof.app_recognition_verdict,
                           "app_licensing_verdict": self.proof.app_licensing_verdict,
                           "device_integrity": list(self.proof.device_integrity),
                           "policy_digest": self.proof.policy_digest.hex()})
        if self.version_code is not None:
            result["version_code"] = self.version_code
        if self.signer_sha256 is not None:
            result["signer_sha256"] = self.signer_sha256.hex()
        return result


def _strict_json(raw: bytes) -> Any:
    def unique(pairs):
        result = {}
        for key, item in pairs:
            require(key not in result, "duplicate JSON member")
            result[key] = item
        return result

    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=unique,
                          parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (UnicodeDecodeError, ValueError, RecursionError):
        raise AttestationRejected("invalid JSON") from None


def _base64url_digest(text: Any) -> bytes:
    require(type(text) is str and len(text) == 43, "invalid base64url digest")
    try:
        raw = base64.urlsafe_b64decode(text + "=")
    except (binascii.Error, ValueError):
        raise AttestationRejected("invalid base64url digest") from None
    require(len(raw) == 32 and request_hash_text(raw) == text, "invalid base64url digest")
    return raw


def play_integrity_app_candidates(google_response: bytes) -> Tuple[int, bytes]:
    """Read the decoded versionCode and single signer digest before full verification."""
    value = _strict_json(google_response)
    require(type(value) is dict and type(value.get("tokenPayloadExternal")) is dict,
            "invalid Play Integrity decoder envelope")
    app = value["tokenPayloadExternal"].get("appIntegrity")
    require(type(app) is dict, "missing Play Integrity app verdict")
    version = app.get("versionCode")
    require(type(version) is str and re.fullmatch(r"(?:0|[1-9][0-9]{0,18})", version) is not None,
            "invalid Play Integrity app version")
    certificates = app.get("certificateSha256Digest")
    require(type(certificates) is list and len(certificates) == 1,
            "Play Integrity app-signing certificate differs")
    return int(version), _base64url_digest(certificates[0])


# --------------------------------------------------------------------------
# Results.

def _security_label(level: int) -> str:
    return _SECURITY_LEVEL_LABELS[level]


@dataclass(frozen=True)
class AndroidEnrollmentResult:
    transcript_sha256: bytes
    device_public_key: bytes
    device_id: bytes
    cert_platform: int
    tier: TierDecision
    security_level: int
    root: VendorRoot
    root_tier_cap: str
    revocation_fetched_at_ms: Optional[int]
    facts: AndroidKeyFacts
    evidence_sha256: bytes
    play_integrity: PlayIntegrityOutcome
    verified_at_ms: int

    def to_json(self) -> Dict[str, Any]:
        return {"platform": ANDROID, "transcript_sha256": self.transcript_sha256.hex(),
                "device_public_key": self.device_public_key.hex(),
                "device_id": self.device_id.hex(), "cert_platform": self.cert_platform,
                "tier": self.tier.tier, "tier_reasons": list(self.tier.reasons),
                "security_level": _security_label(self.security_level),
                "evidence_sha256": self.evidence_sha256.hex(),
                "verified_at_ms": self.verified_at_ms,
                "root": {**self.root.describe(), "effective_tier_cap": self.root_tier_cap,
                         "spki_sha256": _spki_sha256(self.root.der).hex()},
                "revocation": {"source": self.root.revocation,
                               "list_fetched_at_ms": self.revocation_fetched_at_ms},
                "key": self.facts.to_json(),
                "play_integrity": self.play_integrity.to_json()}


@dataclass(frozen=True)
class AppleEnrollmentResult:
    transcript_sha256: bytes
    device_public_key: bytes
    device_id: bytes
    cert_platform: int
    tier: TierDecision
    app_attest_key_id: bytes
    app_attest_public_key: bytes
    environment: str
    validation_category: Optional[int]
    bundle_version: Optional[str]
    receipt: VerifiedAppleReceipt
    evidence_sha256: bytes
    verified_at_ms: int

    def to_json(self) -> Dict[str, Any]:
        return {"platform": APPLE, "transcript_sha256": self.transcript_sha256.hex(),
                "device_public_key": self.device_public_key.hex(),
                "device_id": self.device_id.hex(), "cert_platform": self.cert_platform,
                "tier": self.tier.tier, "tier_reasons": list(self.tier.reasons),
                "app_attest_key_id": self.app_attest_key_id.hex(),
                "app_attest_public_key": self.app_attest_public_key.hex(),
                "app_attest_counter": 0, "environment": self.environment,
                "validation_category": self.validation_category,
                "bundle_version": self.bundle_version,
                "receipt": {"sha256": self.receipt.receipt_sha256.hex(),
                            "type": self.receipt.receipt_type,
                            "created_at_ms": self.receipt.creation_time_ms},
                "evidence_sha256": self.evidence_sha256.hex(),
                "verified_at_ms": self.verified_at_ms}


@dataclass(frozen=True)
class AndroidSyncResult:
    play_integrity: PlayIntegrityOutcome
    verified_at_ms: int

    def to_json(self) -> Dict[str, Any]:
        return {"platform": ANDROID, "play_integrity": self.play_integrity.to_json(),
                "verified_at_ms": self.verified_at_ms}


@dataclass(frozen=True)
class AppleSyncResult:
    counter: int
    assertion_sha256: bytes
    client_data_sha256: bytes
    validation_category: Optional[int]
    bundle_version: Optional[str]
    verified_at_ms: int

    def to_json(self) -> Dict[str, Any]:
        return {"platform": APPLE, "counter": self.counter,
                "assertion_sha256": self.assertion_sha256.hex(),
                "client_data_sha256": self.client_data_sha256.hex(),
                "validation_category": self.validation_category,
                "bundle_version": self.bundle_version, "verified_at_ms": self.verified_at_ms}


# --------------------------------------------------------------------------
# Verifier.

_PI_POLICY_DOMAIN = b"iroha.kagemusha.attested-worker.play-integrity-policy.v1\0"


class AttestedEnrollmentVerifier:
    """Stateless enrollment and sync attestation checks for one scheme.

    The issuer owns every durable record: challenges, account proofs, device
    certificates and App Attest counters. This object only answers whether
    platform evidence admits a key and at which tier.
    """

    def __init__(self, config: AttestedEnrollmentConfig, *,
                 clock: Optional[Callable[[], int]] = None,
                 revocation_fetch: Optional[Callable[[], FrozenSet[int]]] = None,
                 play_integrity_decoder: Optional[Any] = None) -> None:
        require(type(config) is AttestedEnrollmentConfig, "invalid attested configuration")
        config.validate()
        self.config = config
        self._clock = clock if clock is not None else _wall_clock_ms
        require(callable(self._clock), "invalid clock")
        self.revocation = GoogleRevocationCache(clock=self._clock,
                                                max_age_ms=config.revocation_max_age_ms,
                                                fetch=revocation_fetch)
        require(play_integrity_decoder is None or callable(getattr(play_integrity_decoder,
                                                                   "decode", None)),
                "invalid Play Integrity decoder")
        self._decoder = play_integrity_decoder
        self._pi_policy_digest = (
            sha256(_PI_POLICY_DOMAIN + json.dumps(config.android.projection(), sort_keys=True,
                   separators=(",", ":")).encode("utf-8"))
            if config.android is not None else None)

    @property
    def play_integrity_configured(self) -> bool:
        return self._decoder is not None

    def _now(self) -> int:
        now = self._clock()
        require(type(now) is int and 0 < now < (1 << 63), "invalid trusted time")
        return now

    def _android(self) -> AttestedAndroidPolicy:
        if self.config.android is None:
            raise PlatformNotConfigured("Android enrollment is not configured for this scheme")
        return self.config.android

    def _apple(self) -> AttestedApplePolicy:
        if self.config.apple is None:
            raise PlatformNotConfigured("Apple enrollment is not configured for this scheme")
        return self.config.apple

    def _play_integrity_policy(self, version_code: int, signer: bytes) -> PlayIntegrityPolicy:
        android = self._android()
        pi = android.play_integrity
        assert self._pi_policy_digest is not None
        policy = PlayIntegrityPolicy(
            self._pi_policy_digest, android.package_name, version_code, signer,
            pi.maximum_evidence_age_ms, pi.maximum_evidence_age_ms,
            pi.require_play_recognized, pi.require_licensed, pi.minimum_device_integrity)
        policy.validate()
        return policy

    def _optional_play_integrity(self, token: Optional[str], request_hash: bytes, *,
                                 version_code: Optional[int],
                                 signer: Optional[bytes]) -> PlayIntegrityOutcome:
        """Verify and record a token when present; never raises for token problems.

        With ``version_code`` given (enrollment) the decoded versionCode must equal
        it exactly. Without it (sync) the decoded versionCode must meet the
        minimum and the decoded signer must be an admitted signer.
        """
        android = self._android()
        if token is None:
            return PlayIntegrityOutcome("absent", request_hash)
        try:
            token_sha256 = sha256(_require_opaque_token(token).encode("ascii"))
        except AttestationRejected as error:
            return PlayIntegrityOutcome("rejected", request_hash, str(error))
        if self._decoder is None:
            return PlayIntegrityOutcome("not_configured", request_hash, token_sha256=token_sha256)
        try:
            body = self._decoder.decode(token, android.package_name)
            require(type(body) is bytes, "invalid Play Integrity decoder result")
        except VerificationUnavailable as error:
            return PlayIntegrityOutcome("unavailable", request_hash, str(error),
                                        token_sha256=token_sha256)
        except AttestationRejected as error:
            return PlayIntegrityOutcome("rejected", request_hash, str(error),
                                        token_sha256=token_sha256)
        except Exception:
            return PlayIntegrityOutcome("unavailable", request_hash,
                                        "Play Integrity decoder unavailable",
                                        token_sha256=token_sha256)
        try:
            if version_code is None:
                version_code, signer = play_integrity_app_candidates(body)
                require(version_code >= android.min_version_code,
                        "Play Integrity app version is below the scheme minimum")
                require(signer in android.signer_sha256,
                        "Play Integrity app-signing certificate is not admitted")
            assert signer is not None
            proof = _verify_google_payload(body, self._play_integrity_policy(version_code, signer),
                                           request_hash, self._now(), token_sha256)
        except AttestationRejected as error:
            return PlayIntegrityOutcome("rejected", request_hash, str(error),
                                        token_sha256=token_sha256, google_response=body)
        return PlayIntegrityOutcome("verified", request_hash, None, token_sha256, body, proof,
                                    version_code, signer)

    # -- enrollment ---------------------------------------------------------

    def enroll_android(self, *, server_nonce: bytes, client_nonce: bytes, account_digest: bytes,
                       certificate_chain: Sequence[bytes],
                       play_integrity_token: Optional[str] = None) -> AndroidEnrollmentResult:
        """Admit an Android hardware key generated with challenge ``H(E)``."""
        android = self._android()
        selection = AttestedSelectionV1.android(self.config.scheme_id, server_nonce,
                                                client_nonce, account_digest)
        transcript = selection.transcript()
        chain = list(certificate_chain) if type(certificate_chain) in (list, tuple) else None
        require(chain is not None and 2 <= len(chain) <= MAX_CHAIN
                and all(type(cert) is bytes and 0 < len(cert) <= MAX_CERT for cert in chain),
                "certificate chain outside bound")
        root = self.config.roots.android_root(chain[-1])
        # Minimum versionCode without touching the exact-version verifier:
        # select the attested values, check the minimum, then pin them.
        facts = android_key_facts(chain, root.der)
        require(facts.package_name == android.package_name, "Android package mismatch")
        require(facts.signer_sha256 in android.signer_sha256, "Android signing identity mismatch")
        if facts.version_code < android.min_version_code:
            raise BelowMinimumVersion("attested app versionCode is below the scheme minimum")
        proof = verify_android_persistent_app_key_raw(
            chain, selection, android.package_name, facts.version_code, facts.signer_sha256,
            root.der, root.sha256, self._now(), self.config.openssl_path,
            allowed_security_levels=android.allowed_security_levels)
        level = proof.android_security_level
        require(level in android.allowed_security_levels
                and level == facts.attestation_security_level == facts.keymint_security_level,
                "verified Android level differs from scheme policy")
        device_public_key = require_p256_public_key(proof.attested_public_key_sec1,
                                                    "attested public key")
        fetched_at = (self.revocation.require_not_revoked(chain)
                      if root.revocation == REVOCATION_GOOGLE else None)
        play_integrity = self._optional_play_integrity(
            play_integrity_token,
            enrollment_play_integrity_request_hash(transcript, device_public_key),
            version_code=facts.version_code, signer=facts.signer_sha256)
        root_cap = self.config.roots.effective_tier_cap(root)
        tier = derive_android_tier(
            security_level=level, root_tier_cap=root_cap,
            attestation_version=facts.attestation_version,
            os_patch_level=facts.os_patch_level, vendor_patch_level=facts.vendor_patch_level,
            boot_patch_level=facts.boot_patch_level, os_patch_floor=android.os_patch_floor,
            play_integrity_verified=play_integrity.verified,
            play_integrity_absent_cap=android.play_integrity.absent_tier_cap)
        return AndroidEnrollmentResult(
            sha256(transcript), device_public_key,
            attested_device_id(self.config.scheme_id, device_public_key),
            CERT_PLATFORM_ANDROID_STRONGBOX if level == SECURITY_LEVEL_STRONGBOX
            else CERT_PLATFORM_ANDROID_TEE,
            tier, level, root, root_cap, fetched_at, facts,
            sha256(encode_android_chain(chain)), play_integrity, self._now())

    def enroll_apple(self, *, server_nonce: bytes, client_nonce: bytes, account_digest: bytes,
                     key_id: bytes, signing_public_key: bytes,
                     attestation_object: bytes) -> AppleEnrollmentResult:
        """Admit an App Attest key and the Secure Enclave key it vouches for."""
        apple = self._apple()
        attest_root, receipt_root = self.config.roots.apple_roots()
        selection = AttestedSelectionV1.apple(self.config.scheme_id, server_nonce, client_nonce,
                                              account_digest, key_id, signing_public_key)
        transcript = selection.transcript()
        require(type(attestation_object) is bytes and 0 < len(attestation_object) <= MAX_OBJECT,
                "invalid App Attest object")
        decoded = cbor_exact(attestation_object)
        require(type(decoded) is dict and set(decoded) == {"fmt", "attStmt", "authData"}
                and type(decoded["attStmt"]) is dict
                and set(decoded["attStmt"]) == {"x5c", "receipt"}
                and type(decoded["attStmt"]["x5c"]) is list and decoded["attStmt"]["x5c"]
                and type(decoded["attStmt"]["x5c"][0]) is bytes
                and type(decoded["attStmt"]["receipt"]) is bytes,
                "invalid App Attest statement")
        leaf = decoded["attStmt"]["x5c"][0]
        leaf_point, _ = certificate_key_extensions(leaf)
        now = self._now()
        receipt = verify_apple_receipt(
            decoded["attStmt"]["receipt"], apple.app_id, leaf, leaf_point, receipt_root.der,
            receipt_root.sha256, now, self.config.openssl_path, expected_type="ATTEST")
        proof = verify_apple_raw(
            attestation_object, key_id, apple.app_id, apple.environment, selection,
            attest_root.der, attest_root.sha256, now, self.config.openssl_path,
            expected_validation_category=None, expected_bundle_version=None)
        require(proof.attested_public_key_sec1 == leaf_point
                and sha256(leaf_point) == key_id, "App Attest key mismatch")
        require(signing_public_key != leaf_point,
                "Secure Enclave key must differ from the App Attest key")
        tier = (TierDecision("A", ()) if apple.environment == "production"
                else TierDecision("F", ("app_attest_development_environment",)))
        return AppleEnrollmentResult(
            sha256(transcript), signing_public_key,
            attested_device_id(self.config.scheme_id, signing_public_key),
            CERT_PLATFORM_APPLE_SECURE_ENCLAVE, tier, key_id, leaf_point, apple.environment,
            proof.apple_validation_category, proof.apple_bundle_version, receipt,
            proof.evidence_sha256, now)

    # -- sync ---------------------------------------------------------------

    def sync_android(self, *, device_id: bytes, sync_nonce: bytes, head: bytes, seq: int,
                     play_integrity_token: Optional[str] = None) -> AndroidSyncResult:
        """Verify an optional sync-time Play Integrity refresh."""
        self._android()
        request_hash = sync_request_hash(device_id, sync_nonce, head, seq)
        outcome = self._optional_play_integrity(play_integrity_token, request_hash,
                                                version_code=None, signer=None)
        return AndroidSyncResult(outcome, self._now())

    def sync_apple(self, *, device_id: bytes, sync_nonce: bytes, head: bytes, seq: int,
                   assertion: bytes, app_attest_public_key: bytes,
                   previous_counter: int) -> AppleSyncResult:
        """Verify a sync-time App Attest assertion; the issuer persists ``counter``."""
        apple = self._apple()
        client_data = sync_client_data(device_id, sync_nonce, head, seq)
        require_p256_public_key(app_attest_public_key, "App Attest public key")
        checked = verify_apple_assertion(
            assertion, client_data, client_data, app_attest_public_key,
            sha256(app_attest_public_key), apple.app_id, previous_counter,
            self.config.openssl_path, expected_validation_category=None,
            expected_bundle_version=None)
        return AppleSyncResult(checked.counter, checked.assertion_sha256,
                               checked.client_data_sha256, checked.validation_category,
                               checked.bundle_version, self._now())
