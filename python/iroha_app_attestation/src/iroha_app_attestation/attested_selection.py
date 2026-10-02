"""Enrollment transcript for the KAGEMUSHA ``attested-app`` suite (v1).

``AttestedSelectionV1`` is the attested-app counterpart of ``attestation.Selection``.
The existing raw verifiers in ``attestation.py`` read only two members of a
selection, ``attested_key_id`` and ``transcript()``, so this object plugs into
``verify_android_persistent_app_key_raw`` and ``verify_apple_raw`` unchanged.

The transcript is the exact byte string ``E``::

    E = D_enroll || scheme_id[32] || server_nonce[32] || client_nonce[32]
        || account_digest[32] || platform u8 || attested_key_id[32]
        || signing_public_key[65]

    D_enroll = b"iroha:kagemusha:v1:attested-app:enroll\\0"

* Android KeyMint fixes its attestation challenge when the key is generated, so
  the Android transcript carries the all-zero ``attested_key_id`` sentinel and an
  all-zero ``signing_public_key``. The KeyMint attestation challenge is ``H(E)``.
* Apple carries the App Attest key ID (SHA-256 of the App Attest P-256 point) and
  the Secure Enclave transfer key. The App Attest ``clientDataHash`` is ``H(E)``,
  so Apple's nonce is ``H(authData || H(E))``.

Every helper here is pure: no clock, network, key or store is involved. Hash and
domain choices follow the suite's normative layout; ``H`` is SHA-256.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass

from .attestation import fixed32, require

DOMAIN_PREFIX = b"iroha:kagemusha:v1:attested-app:"
DOMAIN_PURPOSES = frozenset({
    "scheme-id", "descriptor", "device-id", "cert", "transition", "request", "ack",
    "voucher", "crl", "crl-delta", "delivery", "enroll", "sync", "account-proof",
    "load-binding", "redemption-id",
})

# Enrollment platform byte inside ``E``. This is the enrollment flow (which
# attestation format follows), not the certificate platform below: Android does
# not know whether StrongBox or TEE generated its key until after it has used
# ``H(E)`` as the KeyMint challenge.
ENROLL_PLATFORM_ANDROID = 1
ENROLL_PLATFORM_APPLE = 2
ENROLL_PLATFORMS = frozenset({ENROLL_PLATFORM_ANDROID, ENROLL_PLATFORM_APPLE})

# ``KagemushaAttestedDeviceCertV1.platform``. There is deliberately no test value.
CERT_PLATFORM_ANDROID_STRONGBOX = 1
CERT_PLATFORM_ANDROID_TEE = 2
CERT_PLATFORM_APPLE_SECURE_ENCLAVE = 3

ZERO_KEY_ID = bytes(32)
ZERO_PUBLIC_KEY = bytes(65)

# NIST P-256 (secp256r1) curve parameters, SEC 2 section 2.4.2.
_P256_P = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
_P256_B = 0x5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B


def attested_domain(purpose: str) -> bytes:
    """Return ``iroha:kagemusha:v1:attested-app:<purpose>\\0`` for a suite purpose."""
    require(type(purpose) is str and purpose in DOMAIN_PURPOSES,
            "unknown attested-app domain purpose")
    return DOMAIN_PREFIX + purpose.encode("ascii") + b"\0"


ENROLL_DOMAIN = attested_domain("enroll")
SYNC_DOMAIN = attested_domain("sync")
ACCOUNT_PROOF_DOMAIN = attested_domain("account-proof")
DEVICE_ID_DOMAIN = attested_domain("device-id")


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def is_p256_public_key(point: bytes) -> bool:
    """Return whether ``point`` is an uncompressed SEC1 point on P-256."""
    if type(point) is not bytes or len(point) != 65 or point[0] != 4:
        return False
    x = int.from_bytes(point[1:33], "big")
    y = int.from_bytes(point[33:], "big")
    if x >= _P256_P or y >= _P256_P:
        return False
    return (y * y - (x * x * x - 3 * x + _P256_B)) % _P256_P == 0


def require_p256_public_key(point: bytes, name: str) -> bytes:
    require(is_p256_public_key(point), f"invalid {name}")
    return point


def _exact32(value: bytes, name: str) -> bytes:
    require(type(value) is bytes and len(value) == 32, f"invalid {name}")
    return value


@dataclass(frozen=True)
class AttestedSelectionV1:
    """Server-selected enrollment values; never decoded from platform evidence.

    ``scheme_id`` comes from the issuer's pinned descriptor, the nonces from the
    issuer challenge and the client request, and ``account_digest`` from the
    issuer's own account resolution. Only Apple supplies ``attested_key_id`` and
    ``signing_public_key`` from the device; both are bound by App Attest.
    """

    scheme_id: bytes
    server_nonce: bytes
    client_nonce: bytes
    account_digest: bytes
    platform: int
    attested_key_id: bytes
    signing_public_key: bytes

    @classmethod
    def android(cls, scheme_id: bytes, server_nonce: bytes, client_nonce: bytes,
                account_digest: bytes) -> "AttestedSelectionV1":
        """Android selection with the all-zero key ID and signing-key sentinels."""
        return cls(scheme_id, server_nonce, client_nonce, account_digest,
                   ENROLL_PLATFORM_ANDROID, ZERO_KEY_ID, ZERO_PUBLIC_KEY)

    @classmethod
    def apple(cls, scheme_id: bytes, server_nonce: bytes, client_nonce: bytes,
              account_digest: bytes, key_id: bytes,
              signing_public_key: bytes) -> "AttestedSelectionV1":
        """Apple selection binding the App Attest key ID and Secure Enclave key."""
        return cls(scheme_id, server_nonce, client_nonce, account_digest,
                   ENROLL_PLATFORM_APPLE, key_id, signing_public_key)

    def validate(self) -> None:
        fixed32(self.scheme_id, "scheme ID")
        fixed32(self.server_nonce, "server nonce")
        fixed32(self.client_nonce, "client nonce")
        fixed32(self.account_digest, "account digest")
        require(self.server_nonce != self.client_nonce, "repeated challenge nonce")
        require(type(self.platform) is int and self.platform in ENROLL_PLATFORMS,
                "invalid enrollment platform")
        _exact32(self.attested_key_id, "attested key ID")
        require(type(self.signing_public_key) is bytes and len(self.signing_public_key) == 65,
                "invalid signing public key")
        if self.platform == ENROLL_PLATFORM_ANDROID:
            require(self.attested_key_id == ZERO_KEY_ID
                    and self.signing_public_key == ZERO_PUBLIC_KEY,
                    "Android enrollment must use the zero key sentinels")
        else:
            require(any(self.attested_key_id), "invalid App Attest key ID")
            require_p256_public_key(self.signing_public_key,
                                    "Secure Enclave signing public key")

    def transcript(self) -> bytes:
        """Return the exact enrollment transcript ``E``."""
        self.validate()
        return (ENROLL_DOMAIN + self.scheme_id + self.server_nonce + self.client_nonce
                + self.account_digest + bytes([self.platform]) + self.attested_key_id
                + self.signing_public_key)

    def transcript_sha256(self) -> bytes:
        """``H(E)``: the KeyMint challenge and the App Attest ``clientDataHash``."""
        return sha256(self.transcript())


def enrollment_play_integrity_request_hash(transcript: bytes, attested_public_key: bytes) -> bytes:
    """``H(E || attested_pk)``, the Play Integrity ``requestHash`` at enrollment.

    The SDK sends ``play_integrity.request_hash_text`` of this digest (unpadded
    base64url) as the Standard API ``requestHash``.
    """
    require(type(transcript) is bytes and transcript.startswith(ENROLL_DOMAIN),
            "invalid enrollment transcript")
    require_p256_public_key(attested_public_key, "attested public key")
    return sha256(transcript + attested_public_key)


def account_proof_digest(transcript: bytes, attested_public_key: bytes | None = None) -> bytes:
    """Digest the account controller key signs during enrollment.

    Android: ``H(D_account-proof || E || attested_pk)``; Apple: ``H(D_account-proof || E)``.
    """
    require(type(transcript) is bytes and transcript.startswith(ENROLL_DOMAIN),
            "invalid enrollment transcript")
    suffix = b""
    if attested_public_key is not None:
        suffix = require_p256_public_key(attested_public_key, "attested public key")
    return sha256(ACCOUNT_PROOF_DOMAIN + transcript + suffix)


def attested_device_id(scheme_id: bytes, device_public_key: bytes) -> bytes:
    """``device_id = H(D_device-id || scheme_id || device_pk)``."""
    fixed32(scheme_id, "scheme ID")
    require_p256_public_key(device_public_key, "device public key")
    return sha256(DEVICE_ID_DOMAIN + scheme_id + device_public_key)


def sync_client_data(device_id: bytes, sync_nonce: bytes, head: bytes, seq: int) -> bytes:
    """``D_sync || device_id || sync_nonce || head || seq`` (seq as u64 little-endian).

    Its SHA-256 is both the Android Play Integrity ``requestHash`` and the iOS
    App Attest assertion ``clientDataHash`` for the attestation refresh at sync.
    """
    fixed32(device_id, "device ID")
    fixed32(sync_nonce, "sync nonce")
    _exact32(head, "acknowledged head")
    require(type(seq) is int and 0 <= seq < (1 << 64), "invalid sync sequence")
    return SYNC_DOMAIN + device_id + sync_nonce + head + seq.to_bytes(8, "little")


def sync_request_hash(device_id: bytes, sync_nonce: bytes, head: bytes, seq: int) -> bytes:
    """``H(D_sync || device_id || sync_nonce || head || seq)``."""
    return sha256(sync_client_data(device_id, sync_nonce, head, seq))
